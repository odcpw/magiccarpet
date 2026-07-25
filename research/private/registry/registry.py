#!/usr/bin/env python3
"""Append-only private evidence registry for MC2 behavioral research.

The JSONL event stream is authoritative. Every line is canonical JSON, every
event is globally hash-chained, and every entity has its own predecessor
chain. This tool has no network or third-party dependency.
"""

from __future__ import annotations

import argparse
import datetime as dt
import fcntl
import hashlib
import json
import math
import os
import re
import stat
import sys
import tempfile
from collections import defaultdict, deque
from pathlib import Path
from typing import Any, Iterable


SCHEMA_VERSION = 1
EVENTS_NAME = "events.jsonl"
LOCK_NAME = "writer.lock"
META_NAME = "registry-meta.json"

EVENT_TYPES = {"CREATE", "TRANSITION", "AMEND", "LINK"}
CLASSIFICATIONS = {
    "PRIVATE_OWNED_REFERENCE",
    "PRIVATE_ANALYSIS",
    "PUBLIC_SAFE_CANDIDATE",
    "SYNTHETIC_ONLY",
}
RELATIONS = {
    "supports",
    "contradicts",
    "falsifies",
    "refines",
    "supersedes",
    "rejects",
    "revokes",
    "verifies",
    "implements",
    "invalidates",
    "requires_retest",
}
ACYCLIC_RELATIONS = {
    "supports",
    "refines",
    "supersedes",
    "verifies",
    "implements",
}
FANOUT_RELATIONS = {"supports", "refines", "verifies", "implements"}
HEX_256 = re.compile(r"^[0-9a-f]{64}$")
EVENT_ID = re.compile(r"^EVT-[0-9]{12}$")


ENTITY_PATTERNS = {
    "source": re.compile(r"^SRC-[A-Z0-9][A-Z0-9-]*-[0-9]{4}$"),
    "build_manifest": re.compile(r"^BLD-[A-Z0-9][A-Z0-9-]*-[0-9]{4}$"),
    "environment": re.compile(r"^ENV-[A-Z0-9][A-Z0-9-]*-[0-9]{4}$"),
    "calibration": re.compile(r"^CAL-[A-Z0-9][A-Z0-9-]*-[0-9]{3}$"),
    "tool": re.compile(r"^TOOL-[A-Z0-9][A-Z0-9-]*-[0-9]{4}$"),
    "campaign": re.compile(r"^CAM-[A-Z0-9][A-Z0-9-]*-[0-9]{4}$"),
    "question": re.compile(r"^Q-CAM-[A-Z0-9][A-Z0-9-]*-[0-9]{4}-[0-9]{2}$"),
    "experiment": re.compile(
        r"^EXP-CAM-[A-Z0-9][A-Z0-9-]*-[0-9]{4}-[0-9]{3}$"
    ),
    "worker_packet": re.compile(r"^PKT-[A-Z0-9][A-Z0-9-]*-[0-9]{4}$"),
    "run": re.compile(
        r"^RUN-EXP-CAM-[A-Z0-9][A-Z0-9-]*-[0-9]{4}-[0-9]{3}-[0-9]{3}$"
    ),
    "capture": re.compile(
        r"^CAP-RUN-EXP-CAM-[A-Z0-9][A-Z0-9-]*-[0-9]{4}-"
        r"[0-9]{3}-[0-9]{3}-[0-9]{2}$"
    ),
    "observation": re.compile(
        r"^OBS-RUN-EXP-CAM-[A-Z0-9][A-Z0-9-]*-[0-9]{4}-"
        r"[0-9]{3}-[0-9]{3}-[0-9]{2}$"
    ),
    "measurement": re.compile(
        r"^MEAS-RUN-EXP-CAM-[A-Z0-9][A-Z0-9-]*-[0-9]{4}-"
        r"[0-9]{3}-[0-9]{3}-[0-9]{2}$"
    ),
    "hypothesis": re.compile(
        r"^HYP-CAM-[A-Z0-9][A-Z0-9-]*-[0-9]{4}-[0-9]{3}$"
    ),
    "mechanism": re.compile(
        r"^MEC-CAM-[A-Z0-9][A-Z0-9-]*-[0-9]{4}-[0-9]{3}$"
    ),
    "finding": re.compile(
        r"^FND-CAM-[A-Z0-9][A-Z0-9-]*-[0-9]{4}-[0-9]{3}$"
    ),
    "contradiction": re.compile(
        r"^CTR-CAM-[A-Z0-9][A-Z0-9-]*-[0-9]{4}-[0-9]{3}$"
    ),
    "protocol_deviation": re.compile(
        r"^PDEV-EXP-CAM-[A-Z0-9][A-Z0-9-]*-[0-9]{4}-"
        r"[0-9]{3}-[0-9]{2}$"
    ),
    "product_deviation": re.compile(
        r"^DDEV-[A-Z0-9][A-Z0-9-]*-[0-9]{3}$"
    ),
    "vector": re.compile(
        r"^VEC-CAM-[A-Z0-9][A-Z0-9-]*-[0-9]{4}-[0-9]{3}$"
    ),
    "transfer": re.compile(r"^TRN-[A-Z0-9][A-Z0-9-]*-[0-9]{4}$"),
    "review": re.compile(r"^REV-[A-Z0-9][A-Z0-9-]*-[0-9]{2}$"),
    "receipt": re.compile(
        r"^RCPT-TRN-[A-Z0-9][A-Z0-9-]*-[0-9]{4}-[0-9]{2}$"
    ),
}


INITIAL_STATUSES = {
    "source": {"REGISTERED"},
    "build_manifest": {"FROZEN"},
    "environment": {"FROZEN"},
    "calibration": {"DRAFT"},
    "tool": {"FROZEN"},
    "campaign": {"DRAFT", "OPEN"},
    "question": {"OPEN"},
    "experiment": {"DRAFT"},
    "worker_packet": {"FROZEN"},
    "run": {"PLANNED"},
    "capture": {"FROZEN"},
    "observation": {"RECORDED"},
    "measurement": {"DRAFT", "RECORDED"},
    "hypothesis": {"OPEN"},
    "mechanism": {"DRAFT"},
    "finding": {"DRAFT"},
    "contradiction": {"OPEN_BLOCKING"},
    "protocol_deviation": {"RECORDED"},
    "product_deviation": {"DRAFT"},
    "vector": {"DRAFT"},
    "transfer": {"DRAFT"},
    "review": {"RECORDED"},
    "receipt": {"CURRENT"},
}


TRANSITIONS = {
    "source": {
        "REGISTERED": {"SUPERSEDED", "REVOKED", "RETEST_REQUIRED"},
    },
    "build_manifest": {
        "FROZEN": {"SUPERSEDED", "REVOKED", "RETEST_REQUIRED"},
    },
    "environment": {
        "FROZEN": {"SUPERSEDED", "REVOKED", "RETEST_REQUIRED"},
    },
    "calibration": {
        "DRAFT": {"PASSED", "FAILED"},
        "PASSED": {"SUPERSEDED", "REVOKED", "RETEST_REQUIRED"},
        "FAILED": {"SUPERSEDED"},
        "RETEST_REQUIRED": {"PASSED", "FAILED", "SUPERSEDED"},
    },
    "tool": {
        "FROZEN": {"SUPERSEDED", "REVOKED", "RETEST_REQUIRED"},
    },
    "campaign": {
        "DRAFT": {"OPEN", "ABANDONED"},
        "OPEN": {"PAUSED", "CLOSED", "ABANDONED", "RETEST_REQUIRED"},
        "PAUSED": {"OPEN", "ABANDONED", "RETEST_REQUIRED"},
        "RETEST_REQUIRED": {"OPEN", "ABANDONED"},
    },
    "question": {
        "OPEN": {"ANSWERED", "DEFERRED"},
        "DEFERRED": {"OPEN"},
        "ANSWERED": {"RETEST_REQUIRED"},
        "RETEST_REQUIRED": {"OPEN", "DEFERRED"},
    },
    "experiment": {
        "DRAFT": {"PREREGISTERED", "EXPLORATORY", "INVALID"},
        "PREREGISTERED": {"RUNNING", "INVALID", "EXPLORATORY"},
        "RUNNING": {"COMPLETE", "INVALID", "EXPLORATORY"},
        "COMPLETE": {"RETEST_REQUIRED"},
        "RETEST_REQUIRED": {"PREREGISTERED", "INVALID"},
    },
    "worker_packet": {
        "FROZEN": {"SUPERSEDED", "REVOKED", "RETEST_REQUIRED"},
    },
    "run": {
        "PLANNED": {"VALID", "INVALID", "EXCLUDED", "ABORTED"},
        "VALID": {"RETEST_REQUIRED"},
    },
    "capture": {
        "FROZEN": {"SUPERSEDED", "REVOKED", "RETEST_REQUIRED"},
    },
    "observation": {
        "RECORDED": {"SUPERSEDED", "REVOKED", "RETEST_REQUIRED"},
    },
    "measurement": {
        "DRAFT": {"RECORDED", "REJECTED"},
        "RECORDED": {"SUPERSEDED", "REVOKED", "RETEST_REQUIRED"},
    },
    "hypothesis": {
        "OPEN": {
            "SUPPORTED",
            "FALSIFIED",
            "UNRESOLVED",
            "SCOPE_SPLIT",
            "SUPERSEDED",
            "ABANDONED",
        },
        "SUPPORTED": {
            "CORROBORATED",
            "FALSIFIED",
            "UNRESOLVED",
            "SCOPE_SPLIT",
            "SUPERSEDED",
            "RETEST_REQUIRED",
        },
        "CORROBORATED": {
            "FALSIFIED",
            "SCOPE_SPLIT",
            "SUPERSEDED",
            "RETEST_REQUIRED",
        },
        "UNRESOLVED": {
            "SUPPORTED",
            "FALSIFIED",
            "SCOPE_SPLIT",
            "SUPERSEDED",
            "ABANDONED",
        },
        "RETEST_REQUIRED": {
            "OPEN",
            "SUPPORTED",
            "FALSIFIED",
            "UNRESOLVED",
        },
    },
    "mechanism": {
        "DRAFT": {"SUPPORTED", "UNRESOLVED", "FALSIFIED", "REJECTED"},
        "SUPPORTED": {
            "CORROBORATED",
            "FALSIFIED",
            "UNRESOLVED",
            "SUPERSEDED",
            "RETEST_REQUIRED",
        },
        "CORROBORATED": {
            "FALSIFIED",
            "SUPERSEDED",
            "REVOKED",
            "RETEST_REQUIRED",
        },
        "RETEST_REQUIRED": {"DRAFT", "SUPPORTED", "FALSIFIED"},
    },
    "finding": {
        "DRAFT": {"CORROBORATED", "CONTRADICTED", "REJECTED"},
        "CORROBORATED": {
            "REVIEWED",
            "CONTRADICTED",
            "REJECTED",
            "SUPERSEDED",
            "RETEST_REQUIRED",
        },
        "REVIEWED": {
            "ACCEPTED",
            "CONTRADICTED",
            "REJECTED",
            "SUPERSEDED",
            "REVOKED",
            "RETEST_REQUIRED",
        },
        "ACCEPTED": {
            "CONTRADICTED",
            "SUPERSEDED",
            "REVOKED",
            "RETEST_REQUIRED",
        },
        "CONTRADICTED": {
            "CORROBORATED",
            "REJECTED",
            "SUPERSEDED",
            "REVOKED",
        },
        "RETEST_REQUIRED": {
            "DRAFT",
            "CORROBORATED",
            "CONTRADICTED",
            "REJECTED",
        },
    },
    "contradiction": {
        "OPEN_BLOCKING": {
            "NOT_OVERLAPPING_SCOPE",
            "VERSION_OR_MODE_SPLIT",
            "MEASUREMENT_ERROR",
            "HYPOTHESIS_FALSIFIED",
            "UNRESOLVED_BLOCKING",
        },
        "UNRESOLVED_BLOCKING": {
            "NOT_OVERLAPPING_SCOPE",
            "VERSION_OR_MODE_SPLIT",
            "MEASUREMENT_ERROR",
            "HYPOTHESIS_FALSIFIED",
        },
    },
    "product_deviation": {
        "DRAFT": {"APPROVED", "REJECTED"},
        "APPROVED": {"SUPERSEDED", "REVOKED", "RETEST_REQUIRED"},
    },
    "vector": {
        "DRAFT": {"REVIEWED", "REJECTED"},
        "REVIEWED": {"ACCEPTED", "REJECTED"},
        "ACCEPTED": {"SUPERSEDED", "REVOKED", "RETEST_REQUIRED"},
    },
    "transfer": {
        "DRAFT": {"IN_REVIEW", "PRIVATE_ONLY", "LEGAL_REVIEW", "REVISE"},
        "REVISE": {"DRAFT", "PRIVATE_ONLY", "LEGAL_REVIEW"},
        "IN_REVIEW": {
            "APPROVED",
            "REVISE",
            "PRIVATE_ONLY",
            "LEGAL_REVIEW",
        },
        "APPROVED": {
            "CONSUMED",
            "SUPERSEDED",
            "REVOKED",
            "RETEST_REQUIRED",
        },
        "CONSUMED": {"SUPERSEDED", "REVOKED", "RETEST_REQUIRED"},
        "RETEST_REQUIRED": {"DRAFT", "REVOKED", "SUPERSEDED"},
    },
    "receipt": {
        "CURRENT": {"STALE_RETEST", "STALE_SUPERSEDED", "STALE_REVOKED"},
    },
}


INVALIDATING_STATUSES = {
    "FALSIFIED",
    "SUPERSEDED",
    "REVOKED",
    "RETEST_REQUIRED",
    "CONTRADICTED",
    "UNRESOLVED_BLOCKING",
    "INVALID",
}
SAFE_DEPENDENT_STATUSES = {
    "FALSIFIED",
    "SUPERSEDED",
    "REVOKED",
    "RETEST_REQUIRED",
    "CONTRADICTED",
    "REJECTED",
    "INVALID",
    "PRIVATE_ONLY",
    "LEGAL_REVIEW",
    "STALE_RETEST",
    "STALE_SUPERSEDED",
    "STALE_REVOKED",
}
STALE_RECEIPT_STATUSES = {
    "STALE_RETEST",
    "STALE_SUPERSEDED",
    "STALE_REVOKED",
}
REVIEW_DISPOSITIONS = {"APPROVE", "REVISE", "PRIVATE_ONLY", "LEGAL_REVIEW"}
INDEPENDENCE = {"FRESH_NO_CONTEXT", "INDEPENDENT_ACCESS", "INHERITED_CONTEXT"}


REQUIRED_PAYLOAD = {
    "source": {"title", "source_class", "artifact_hashes"},
    "build_manifest": {"source_ids", "artifact_hashes"},
    "environment": {"tool_ids", "artifact_hashes"},
    "calibration": {
        "environment_id",
        "metric",
        "threshold",
        "result",
        "artifact_hashes",
    },
    "tool": {"name", "version", "origin", "artifact_hashes"},
    "campaign": {
        "bead_id",
        "build_id",
        "environment_ids",
        "calibration_ids",
        "question_scope",
        "denied_operations",
    },
    "question": {"campaign_id", "question", "falsifier"},
    "experiment": {
        "campaign_id",
        "question_id",
        "hypothesis_ids",
        "reset_tier",
        "input_determinism",
        "stop_rule",
    },
    "worker_packet": {"campaign_id", "allowed_operations", "denied_paths"},
    "run": {"experiment_id", "reset_tier", "terminal_disposition"},
    "capture": {"run_id", "artifact_hashes"},
    "observation": {"run_id", "observed_fact"},
    "measurement": {"run_id", "method", "units", "uncertainty"},
    "hypothesis": {
        "campaign_id",
        "question_id",
        "statement",
        "alternative",
        "falsifier",
        "evidence_axes",
        "evidence_ids",
        "evidence_route_tags",
        "applicability",
    },
    "mechanism": {"campaign_id", "statement", "dataflow_chain"},
    "finding": {
        "campaign_id",
        "question_id",
        "functional_rule",
        "fidelity_classification",
        "evidence_floor",
        "evidence_axes",
        "evidence_route_tags",
        "evidence_ids",
        "rule_domains",
        "contradiction_resolution",
        "applicability",
    },
    "contradiction": {"campaign_id", "affected_ids", "disposition"},
    "protocol_deviation": {
        "experiment_id",
        "deviation",
        "validity_disposition",
    },
    "product_deviation": {"scope", "reference_delta", "rationale"},
    "vector": {"campaign_id", "finding_ids", "traffic_light", "cases"},
    "transfer": {
        "traffic_light",
        "destination_bead_id",
        "finding_ids",
        "vector_ids",
        "package_sha256",
    },
    "review": {
        "target_id",
        "target_event_hash",
        "disposition",
        "independence",
        "review_checks",
        "review_context",
    },
    "receipt": {
        "transfer_id",
        "transfer_event_hash",
        "destination_bead_id",
    },
}

COMMON_REQUIRED_PAYLOAD = {"detail"}

FIDELITY_CLASSIFICATIONS = {
    "REFERENCE_FACT",
    "REFERENCE_HYPOTHESIS",
    "PROVISIONAL_REPLICA_RULE",
    "DELIBERATE_MODERNIZATION",
    "ORIGINAL_ADDITION",
    "OMITTED",
}
EVIDENCE_FLOORS = {
    "SIMPLE_BOUNDED": {"O": 2, "R": 2, "T": 2, "A": 1},
    "CRITICAL": {"O": 3, "R": 3, "T": 2, "A": 1},
}
EVIDENCE_ROUTE_TAGS = {
    "DOCUMENTATION",
    "CONTROLLED_OBSERVATION",
    "STATIC_ANALYSIS",
    "DYNAMIC_CORRELATION",
    "INDEPENDENT_REPRODUCTION",
    "SYNTHETIC_CONFORMANCE",
    "ORIGINAL_DESIGN",
}
RULE_DOMAINS = {
    "DISCRETE",
    "CONTINUOUS",
    "STOCHASTIC",
    "VERSION_MODE",
    "PRESENTATION",
    "TIMING",
    "DETERMINISM",
    "NETWORK",
    "ECONOMY",
    "ORDERING",
}
CRITICAL_RULE_DOMAINS = {
    "TIMING",
    "DETERMINISM",
    "NETWORK",
    "ECONOMY",
    "ORDERING",
}
PUBLIC_TRANSFER_CLASSIFICATIONS = {"PUBLIC_SAFE_CANDIDATE", "SYNTHETIC_ONLY"}
SOURCE_CLASSES = {
    "OWNED",
    "PUBLIC",
    "SYNTHETIC",
    "DOCUMENTATION",
    "MODIFIED_DIAGNOSTIC",
}
RESET_TIERS = {"COLD_LAUNCH", "LEVEL_RESTART", "CHECKPOINT", "SAVESTATE"}
INPUT_DETERMINISM_GRADES = {
    "D0_MANUAL_UNTIMED",
    "D1_MANUAL_TIMED",
    "D2_REPLAYED_HOST",
    "D3_GUEST_CORRELATED",
}
RUN_TERMINAL_DISPOSITIONS = {
    "PENDING",
    "SUCCESS",
    "INVALID",
    "EXCLUDED",
    "ABORTED",
}
CONTRADICTION_DISPOSITIONS = {
    "PENDING",
    "NOT_OVERLAPPING_SCOPE",
    "VERSION_OR_MODE_SPLIT",
    "MEASUREMENT_ERROR",
    "HYPOTHESIS_FALSIFIED",
    "UNRESOLVED_BLOCKING",
}
PROTOCOL_DEVIATION_DISPOSITIONS = {
    "WITHIN_PREREGISTERED_TOLERANCE",
    "INVALID_CONFIRMATORY",
    "EXCLUDED",
    "EXPLORATORY",
}
CALIBRATION_RESULTS = {"PENDING", "PASS", "FAIL", "EQUIVALENT"}

STRING_LIST_FIELDS = {
    "source_ids",
    "tool_ids",
    "environment_ids",
    "calibration_ids",
    "denied_operations",
    "hypothesis_ids",
    "allowed_operations",
    "denied_paths",
    "dataflow_chain",
    "affected_ids",
    "finding_ids",
    "vector_ids",
    "evidence_route_tags",
    "evidence_ids",
    "rule_domains",
    "references",
}
LIST_FIELDS = {"cases"}
OBJECT_FIELDS = {
    "artifact_hashes",
    "evidence_axes",
    "detail",
    "review_checks",
    "review_context",
}
SCALAR_FIELDS = {"threshold", "result", "uncertainty"}
STRING_FIELDS = {
    field
    for fields in REQUIRED_PAYLOAD.values()
    for field in fields
    if field
    not in STRING_LIST_FIELDS | LIST_FIELDS | OBJECT_FIELDS | SCALAR_FIELDS
}
ALLOWED_CREATE_FIELDS = {
    entity_type: required | COMMON_REQUIRED_PAYLOAD | {"references"}
    for entity_type, required in REQUIRED_PAYLOAD.items()
}

# Type-specific inline detail manifests make the full protocol record
# representable without weakening the closed CREATE schemas. They are hashed
# independently so the same manifest can later be extracted to an owner-only
# artifact while retaining exact identity.
DETAIL_REQUIRED_FIELDS: dict[str, set[str]] = {
    "source": {
        "revision",
        "origin_lineage",
        "license",
        "license_risk_disposition",
        "evidence_grade",
        "approved_use",
        "exposure",
        "retention_disposition",
        "modification",
    },
    "build_manifest": {
        "launch_chain",
        "artifact_roles",
        "install_version",
        "master_working_copy_status",
        "config_hashes",
        "inner_executable",
        "modification",
    },
    "tool": {
        "revision",
        "dependency_hashes",
        "licenses_notices",
        "install_removal",
        "supported_format_disposition",
        "exposure",
        "synthetic_bakeoff",
        "qualification_status",
    },
    "environment": {
        "host",
        "display_audio",
        "emulator",
        "game",
        "capture",
        "analyzer",
        "multiplayer",
    },
    "calibration": {
        "condition",
        "warm_up",
        "threshold_kind",
        "result_disposition",
        "baselines",
        "delivered_rates",
        "instrumentation_delta",
        "clock_uncertainty",
        "starvation_clock_checks",
    },
    "campaign": {
        "subsystem",
        "insufficiency_rationale",
        "source_risk_inputs",
        "calibration_ids",
        "approved_operations",
        "roles",
        "implementer_isolation",
        "exposure",
        "outputs",
        "budgets",
        "stop_escalation",
        "analyzer_project",
        "retention_policy",
        "closeout",
        "rerun_triggers",
    },
    "question": {
        "known_facts",
        "evidence_grade",
        "scope",
        "discriminating_prediction",
        "hypothesis_plan",
        "answer_target",
    },
    "experiment": {
        "primary_contrast",
        "variables",
        "setup_reset",
        "input_schedule",
        "controls",
        "exclusions",
        "run_order",
        "factorial_plan",
        "sampling_plan",
        "independence_unit",
        "analysis_plan",
        "precision_target",
        "units_tolerance",
        "decision_table",
        "persistent_state",
        "calibration_ids",
    },
    "worker_packet": {
        "question_experiment_ids",
        "outcome",
        "packet_identity",
        "cwd",
        "worker_reviewer_context",
        "immutable_inputs",
        "allowed_files_denied_services",
        "ownership_lock",
        "hypothesis_plan",
        "procedure",
        "measurement_plan",
        "output_contract",
        "budgets",
        "commands_runtime_bounds",
        "terminal_predicates",
        "cleanup_retry",
        "dependencies_downstream",
        "return_manifest",
    },
    "run": {
        "build_environment_calibration",
        "initial_state",
        "input_events",
        "geometry",
        "run_order",
        "rng_independence",
        "output_capture_ids",
        "notes",
        "setup_modifications",
        "deviation_ids",
        "actual_procedure",
        "validity_reason",
        "clock_network_context",
        "exposure_retention",
    },
    "capture": {
        "artifact_manifest",
        "artifact_roles",
        "tool_configuration",
        "rates_codecs_timestamps",
        "clock_mapping",
        "measured_overhead",
        "completeness",
        "logical_reference",
        "retention_disposition",
    },
    "observation": {
        "capture_ids",
        "boundary_condition",
        "scope",
        "timing_sampling_basis",
        "observability_condition",
    },
    "measurement": {
        "value_or_distribution",
        "capture_ids",
        "sample_count",
        "independence_unit",
        "extraction_identity",
        "precision_quantization_bounds",
        "timing_statistics",
    },
    "hypothesis": {
        "question_id",
        "alternatives",
        "predictions",
        "scope",
        "evidence_route_ids",
        "contradiction_ids",
        "c1_resolution",
    },
    "mechanism": {
        "question_hypothesis_finding_ids",
        "build_tool_project_config",
        "evidence_route_ids",
        "loader_memory_map_qualification",
        "competing_mechanisms",
        "scope",
        "runtime_probes",
        "ambiguity_stop_disposition",
    },
    "finding": {
        "question_id",
        "scope",
        "functional_specification",
        "unknowns_internal_freedom",
        "uncertainty",
        "evidence_ids",
        "contradiction_ids",
        "reproduction",
        "traffic_classification",
    },
    "contradiction": {
        "conflicting_evidence_ids",
        "conflicting_statements",
        "scope_dimensions",
        "resolution",
        "review_context",
        "evidence_axis_change",
        "invalidation_targets",
    },
    "protocol_deviation": {
        "run_ids",
        "planned_vs_actual",
        "affected_step",
        "occurrence_time",
        "downstream_effect",
        "review_context",
    },
    "product_deviation": {
        "reference_finding_ids",
        "replacement_behavior",
        "applicability",
        "approval_context",
        "public_safe_declaration",
        "destination",
    },
    "vector": {
        "finding_ids",
        "branch_coverage",
        "synthesis_provenance",
        "no_original_payload_declaration",
        "case_schema",
    },
    "transfer": {
        "package_schema_version",
        "destination_files",
        "applicability_uncertainties",
        "provisional_choices",
        "runtime_constraints",
        "required_behavior_internal_freedom",
        "item_traffic_classification",
        "amber_disposition",
        "package_manifest",
    },
    "review": {
        "target_type",
        "target_record_hash",
        "reviewed_aspects",
        "reasons",
    },
    "receipt": {
        "vector_event_hashes",
        "destination_files",
        "implementation_artifact_hashes",
        "worker_context",
        "stale_notification",
    },
}

DETAIL_LIST_FIELDS = {
    "launch_chain",
    "licenses_notices",
    "calibration_ids",
    "approved_operations",
    "stop_escalation",
    "rerun_triggers",
    "capture_ids",
    "contradiction_ids",
    "deviation_ids",
    "evidence_route_ids",
    "evidence_ids",
    "vector_ids",
    "finding_ids",
    "conflicting_evidence_ids",
    "invalidation_targets",
    "run_ids",
    "reference_finding_ids",
    "destination_files",
    "vector_event_hashes",
}

DETAIL_STRING_FIELDS = {
    "revision",
    "origin_lineage",
    "license",
    "license_risk_disposition",
    "evidence_grade",
    "approved_use",
    "install_version",
    "master_working_copy_status",
    "inner_executable",
    "qualification_status",
    "condition",
    "warm_up",
    "threshold_kind",
    "result_disposition",
    "subsystem",
    "insufficiency_rationale",
    "implementer_isolation",
    "known_facts",
    "discriminating_prediction",
    "answer_target",
    "primary_contrast",
    "input_schedule",
    "independence_unit",
    "precision_target",
    "outcome",
    "cwd",
    "notes",
    "actual_procedure",
    "validity_reason",
    "completeness",
    "logical_reference",
    "boundary_condition",
    "timing_sampling_basis",
    "observability_condition",
    "question_id",
    "c1_resolution",
    "ambiguity_stop_disposition",
    "uncertainty",
    "occurrence_time",
    "affected_step",
    "replacement_behavior",
    "applicability",
    "public_safe_declaration",
    "branch_coverage",
    "synthesis_provenance",
    "no_original_payload_declaration",
    "package_schema_version",
    "target_type",
    "target_record_hash",
    "reasons",
}

# Every semantic ID field is a real dependency, even if the author omits the
# optional human-readable `references` list.
DEPENDENCY_FIELDS: dict[str, dict[str, set[str] | None]] = {
    "build_manifest": {"source_ids": {"source"}},
    "environment": {"tool_ids": {"tool"}},
    "calibration": {"environment_id": {"environment"}},
    "campaign": {
        "build_id": {"build_manifest"},
        "environment_ids": {"environment"},
        "calibration_ids": {"calibration"},
    },
    "question": {"campaign_id": {"campaign"}},
    "experiment": {
        "campaign_id": {"campaign"},
        "question_id": {"question"},
        "hypothesis_ids": {"hypothesis"},
    },
    "worker_packet": {"campaign_id": {"campaign"}},
    "run": {"experiment_id": {"experiment"}},
    "capture": {"run_id": {"run"}},
    "observation": {"run_id": {"run"}},
    "measurement": {"run_id": {"run"}},
    "hypothesis": {
        "campaign_id": {"campaign"},
        "question_id": {"question"},
        "evidence_ids": {
            "source",
            "capture",
            "observation",
            "measurement",
            "hypothesis",
            "mechanism",
        },
    },
    "mechanism": {"campaign_id": {"campaign"}},
    "finding": {
        "campaign_id": {"campaign"},
        "question_id": {"question"},
        "evidence_ids": {
            "source",
            "capture",
            "observation",
            "measurement",
            "hypothesis",
            "mechanism",
        },
    },
    "contradiction": {
        "campaign_id": {"campaign"},
        "affected_ids": None,
    },
    "protocol_deviation": {"experiment_id": {"experiment"}},
    "vector": {
        "campaign_id": {"campaign"},
        "finding_ids": {"finding"},
    },
    "transfer": {
        "finding_ids": {"finding"},
        "vector_ids": {"vector"},
    },
    "review": {"target_id": None},
    "receipt": {"transfer_id": {"transfer"}},
}

NESTED_PARENT_IDS: dict[str, tuple[str, str]] = {
    "calibration": ("environment_id", "CAL-"),
    "question": ("campaign_id", "Q-"),
    "experiment": ("campaign_id", "EXP-"),
    "run": ("experiment_id", "RUN-"),
    "capture": ("run_id", "CAP-"),
    "observation": ("run_id", "OBS-"),
    "measurement": ("run_id", "MEAS-"),
    "hypothesis": ("campaign_id", "HYP-"),
    "mechanism": ("campaign_id", "MEC-"),
    "finding": ("campaign_id", "FND-"),
    "contradiction": ("campaign_id", "CTR-"),
    "protocol_deviation": ("experiment_id", "PDEV-"),
    "vector": ("campaign_id", "VEC-"),
    "review": ("target_id", "REV-"),
    "receipt": ("transfer_id", "RCPT-"),
}

LINK_IMMUTABLE_ENTITY_TYPES = {"transfer", "review", "receipt"}

AMENDABLE_STATUSES = {
    "calibration": {"DRAFT", "FAILED", "RETEST_REQUIRED"},
    "campaign": {"DRAFT", "OPEN", "PAUSED"},
    "question": {"OPEN", "DEFERRED", "RETEST_REQUIRED"},
    "experiment": {"DRAFT", "EXPLORATORY", "RETEST_REQUIRED"},
    "hypothesis": {"OPEN", "SUPPORTED", "UNRESOLVED", "RETEST_REQUIRED"},
    "mechanism": {"DRAFT", "SUPPORTED", "UNRESOLVED", "RETEST_REQUIRED"},
    "finding": {"DRAFT", "CORROBORATED", "CONTRADICTED", "RETEST_REQUIRED"},
    "contradiction": {"OPEN_BLOCKING", "UNRESOLVED_BLOCKING"},
    "product_deviation": {"DRAFT"},
    "vector": {"DRAFT"},
}


class RegistryError(ValueError):
    """Raised when registry content is not valid."""


def canonical_bytes(value: dict[str, Any]) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        + "\n"
    ).encode("ascii")


def hash_event(event_without_hash: dict[str, Any]) -> str:
    return hashlib.sha256(canonical_bytes(event_without_hash)).hexdigest()


def detail_manifest_sha256(manifest: dict[str, Any]) -> str:
    return hashlib.sha256(canonical_bytes(manifest)).hexdigest()


def utc_now() -> str:
    return (
        dt.datetime.now(dt.timezone.utc)
        .replace(microsecond=0)
        .isoformat()
        .replace("+00:00", "Z")
    )


def parse_timestamp(value: str) -> None:
    if not isinstance(value, str) or not value.endswith("Z"):
        raise RegistryError("timestamp must be an RFC3339 UTC value ending in Z")
    try:
        parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise RegistryError(f"invalid timestamp {value!r}") from error
    if parsed.tzinfo is None:
        raise RegistryError("timestamp has no timezone")


def ensure_owner_only(path: Path, directory: bool) -> None:
    file_mode = path.lstat().st_mode
    if stat.S_ISLNK(file_mode):
        raise RegistryError(f"symlink is forbidden in registry storage: {path}")
    if directory and not stat.S_ISDIR(file_mode):
        raise RegistryError(f"registry directory is not a directory: {path}")
    if not directory and not stat.S_ISREG(file_mode):
        raise RegistryError(f"registry file is not a regular file: {path}")
    mode = stat.S_IMODE(file_mode)
    forbidden = mode & 0o077
    if forbidden:
        kind = "directory" if directory else "file"
        raise RegistryError(
            f"{kind} is accessible by group/other: {path} mode={mode:04o}"
        )


def absolute_without_resolving(path: Path) -> Path:
    return Path(os.path.abspath(os.fspath(path)))


def reject_symlink_components(path: Path) -> None:
    absolute = absolute_without_resolving(path)
    current = Path(absolute.anchor)
    for part in absolute.parts[1:]:
        current /= part
        try:
            mode = current.lstat().st_mode
        except FileNotFoundError:
            break
        if stat.S_ISLNK(mode):
            raise RegistryError(f"symlink component is forbidden: {current}")


def check_sha256_values(value: Any, location: str = "payload") -> None:
    if isinstance(value, dict):
        for key, item in value.items():
            child = f"{location}.{key}"
            if (
                key.endswith("_sha256")
                or key == "event_hash"
                or key.endswith("_event_hash")
            ):
                if not isinstance(item, str) or not HEX_256.fullmatch(item):
                    raise RegistryError(f"{child} is not a lowercase SHA-256")
            if key == "artifact_hashes":
                if not isinstance(item, dict) or not item:
                    raise RegistryError(f"{child} must be a non-empty object")
                for artifact, digest in item.items():
                    if not isinstance(artifact, str) or not artifact:
                        raise RegistryError(f"{child} contains an invalid artifact name")
                    if not isinstance(digest, str) or not HEX_256.fullmatch(digest):
                        raise RegistryError(
                            f"{child}.{artifact} is not a lowercase SHA-256"
                        )
            check_sha256_values(item, child)
    elif isinstance(value, list):
        for index, item in enumerate(value):
            check_sha256_values(item, f"{location}[{index}]")
    elif isinstance(value, float) and not math.isfinite(value):
        raise RegistryError(f"{location} contains a non-finite number")


def validate_evidence_axes(value: Any, entity_id: str) -> None:
    if not isinstance(value, dict) or set(value) != {"O", "R", "T", "A", "C"}:
        raise RegistryError(f"{entity_id} evidence_axes must contain O/R/T/A/C")
    for axis, score in value.items():
        if not isinstance(score, int) or not 0 <= score <= 3:
            raise RegistryError(f"{entity_id} axis {axis} must be an integer 0..3")


def status_transition_allowed(entity_type: str, old: str, new: str) -> bool:
    return new in TRANSITIONS.get(entity_type, {}).get(old, set())


def entity_state(events: Iterable[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    return {event["entity_id"]: event for event in events}


def entity_authors(events: Iterable[dict[str, Any]]) -> dict[str, str]:
    authors: dict[str, str] = {}
    for event in events:
        authors.setdefault(event["entity_id"], event["author"])
    return authors


def entity_create_payloads(
    events: Iterable[dict[str, Any]],
) -> dict[str, dict[str, Any]]:
    payloads: dict[str, dict[str, Any]] = {}
    for event in events:
        if event["event_type"] == "CREATE":
            payloads[event["entity_id"]] = event["payload"]
    return payloads


def entity_logical_payloads(
    events: Iterable[dict[str, Any]],
) -> dict[str, dict[str, Any]]:
    payloads: dict[str, dict[str, Any]] = {}
    for event in events:
        entity_id = event["entity_id"]
        if event["event_type"] == "CREATE":
            payloads[entity_id] = dict(event["payload"])
        elif event["event_type"] == "AMEND":
            payloads[entity_id] = {
                **payloads[entity_id],
                **event["payload"]["changes"],
            }
    return payloads


def event_by_hash(events: Iterable[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    return {event["event_hash"]: event for event in events}


def references_in_payload(payload: dict[str, Any]) -> set[str]:
    references = payload.get("references", [])
    if not isinstance(references, list):
        raise RegistryError("payload.references must be an array")
    if not all(isinstance(item, str) and item for item in references):
        raise RegistryError("payload.references contains an invalid ID")
    return set(references)


def validate_string(value: Any, location: str) -> None:
    if not isinstance(value, str) or not value:
        raise RegistryError(f"{location} must be a non-empty string")


def validate_string_list(value: Any, location: str) -> None:
    if not isinstance(value, list) or not all(
        isinstance(item, str) and item for item in value
    ):
        raise RegistryError(f"{location} must be an array of non-empty strings")
    if len(value) != len(set(value)):
        raise RegistryError(f"{location} contains duplicate IDs or values")


def validate_payload_types(
    entity_type: str,
    entity_id: str,
    payload: dict[str, Any],
) -> None:
    allowed = ALLOWED_CREATE_FIELDS[entity_type]
    missing = (REQUIRED_PAYLOAD[entity_type] | COMMON_REQUIRED_PAYLOAD) - set(
        payload
    )
    extra = set(payload) - allowed
    if missing or extra:
        raise RegistryError(
            f"{entity_id} payload fields differ; "
            f"missing={sorted(missing)} extra={sorted(extra)}"
        )
    for field, value in payload.items():
        location = f"{entity_id}.{field}"
        if field in STRING_FIELDS:
            validate_string(value, location)
        elif field in STRING_LIST_FIELDS:
            validate_string_list(value, location)
        elif field in LIST_FIELDS:
            if not isinstance(value, list):
                raise RegistryError(f"{location} must be an array")
        elif field in OBJECT_FIELDS:
            if not isinstance(value, dict):
                raise RegistryError(f"{location} must be an object")
        elif field in SCALAR_FIELDS:
            if isinstance(value, (dict, list)) or value is None:
                raise RegistryError(f"{location} must be a scalar value")


def validate_detail_manifest(
    entity_type: str, entity_id: str, detail: Any
) -> None:
    if not isinstance(detail, dict) or set(detail) != {
        "schema_id",
        "artifact_id",
        "manifest",
        "manifest_sha256",
    }:
        raise RegistryError(f"{entity_id}.detail has invalid envelope fields")
    expected_schema = f"mc2-behavioral-registry/{entity_type}/0.2"
    if detail["schema_id"] != expected_schema:
        raise RegistryError(
            f"{entity_id}.detail schema_id must be {expected_schema}"
        )
    if detail["artifact_id"] != f"DET-{entity_id}":
        raise RegistryError(f"{entity_id}.detail artifact_id is not bound to entity")
    manifest = detail["manifest"]
    if not isinstance(manifest, dict):
        raise RegistryError(f"{entity_id}.detail.manifest must be an object")
    required = DETAIL_REQUIRED_FIELDS[entity_type]
    if set(manifest) != required:
        raise RegistryError(
            f"{entity_id}.detail.manifest fields differ; "
            f"missing={sorted(required - set(manifest))} "
            f"extra={sorted(set(manifest) - required)}"
        )
    for field, value in manifest.items():
        location = f"{entity_id}.detail.manifest.{field}"
        if field in DETAIL_STRING_FIELDS:
            validate_string(value, location)
        elif field in DETAIL_LIST_FIELDS:
            validate_string_list(value, location)
        elif not isinstance(value, dict) or not value:
            raise RegistryError(f"{location} must be a non-empty object")
        validate_detail_tree(value, location)
        if field in {
            "config_hashes",
            "dependency_hashes",
            "implementation_artifact_hashes",
        }:
            validate_named_hash_map(value, location)
    if (
        not isinstance(detail["manifest_sha256"], str)
        or not HEX_256.fullmatch(detail["manifest_sha256"])
        or detail["manifest_sha256"] != detail_manifest_sha256(manifest)
    ):
        raise RegistryError(f"{entity_id}.detail manifest_sha256 mismatch")


def validate_named_hash_map(value: Any, location: str) -> None:
    if not isinstance(value, dict) or not value:
        raise RegistryError(f"{location} must be a non-empty hash map")
    for name, digest in value.items():
        if (
            not isinstance(name, str)
            or not name
            or not isinstance(digest, str)
            or not HEX_256.fullmatch(digest)
        ):
            raise RegistryError(f"{location}.{name} is not a lowercase SHA-256")


def validate_detail_tree(value: Any, location: str) -> None:
    if isinstance(value, dict):
        if not value:
            raise RegistryError(f"{location} must not be empty")
        for key, child in value.items():
            if not isinstance(key, str) or not key:
                raise RegistryError(f"{location} has an invalid key")
            validate_detail_tree(child, f"{location}.{key}")
    elif isinstance(value, list):
        if not value:
            raise RegistryError(f"{location} must not be empty")
        for index, child in enumerate(value):
            validate_detail_tree(child, f"{location}[{index}]")
    elif isinstance(value, str):
        if not value:
            raise RegistryError(f"{location} must not be empty")
    elif isinstance(value, float):
        if not math.isfinite(value):
            raise RegistryError(f"{location} must be finite")
    elif value is None:
        raise RegistryError(f"{location} must not be null")
    elif not isinstance(value, (bool, int)):
        raise RegistryError(f"{location} has an unsupported value type")


def dependency_values(payload: dict[str, Any], field: str) -> list[str]:
    value = payload[field]
    return value if isinstance(value, list) else [value]


def validate_dependencies(
    entity_type: str,
    entity_id: str,
    payload: dict[str, Any],
    states: dict[str, dict[str, Any]],
) -> None:
    dependency_rules = dict(DEPENDENCY_FIELDS.get(entity_type, {}))
    for reference in references_in_payload(payload):
        dependency_rules.setdefault("references", None)
        if reference not in states:
            raise RegistryError(f"{entity_id} references unknown entity {reference}")

    for field, expected_types in dependency_rules.items():
        if field == "references":
            values = list(references_in_payload(payload))
        else:
            values = dependency_values(payload, field)
        for dependency_id in values:
            state = states.get(dependency_id)
            if state is None:
                raise RegistryError(
                    f"{entity_id}.{field} references unknown entity {dependency_id}"
                )
            if expected_types is not None and state["entity_type"] not in expected_types:
                raise RegistryError(
                    f"{entity_id}.{field} expects {sorted(expected_types)}, "
                    f"found {state['entity_type']} for {dependency_id}"
                )
            if state["status"] in INVALIDATING_STATUSES:
                raise RegistryError(
                    f"{entity_id}.{field} references invalidated entity "
                    f"{dependency_id} ({state['status']})"
                )


def validate_nested_parent_id(
    entity_type: str, entity_id: str, payload: dict[str, Any]
) -> None:
    binding = NESTED_PARENT_IDS.get(entity_type)
    if binding is None:
        return
    parent_field, prefix = binding
    expected_prefix = f"{prefix}{payload[parent_field]}-"
    if not entity_id.startswith(expected_prefix):
        raise RegistryError(
            f"{entity_id} does not embed its declared {parent_field} "
            f"{payload[parent_field]}"
        )


def validate_finding_for_acceptance(
    entity_id: str,
    classification: str,
    payload: dict[str, Any],
) -> None:
    if classification not in PUBLIC_TRANSFER_CLASSIFICATIONS:
        raise RegistryError(
            f"{entity_id} accepted finding has private transfer classification"
        )
    floor_name = payload["evidence_floor"]
    if floor_name not in EVIDENCE_FLOORS:
        raise RegistryError(f"{entity_id} has invalid evidence_floor")
    axes = payload["evidence_axes"]
    if axes["C"] > 1:
        raise RegistryError(f"{entity_id} C2/C3 blocks acceptance and transfer")
    if axes["C"] == 1 and payload["contradiction_resolution"] == "NONE":
        raise RegistryError(
            f"{entity_id} C1 requires an explicit reviewed contradiction resolution"
        )
    route_tags = payload["evidence_route_tags"]
    if len(route_tags) < axes["T"]:
        raise RegistryError(
            f"{entity_id} T{axes['T']} requires at least {axes['T']} "
            "distinct evidence routes"
        )
    for axis, floor in EVIDENCE_FLOORS[floor_name].items():
        if axes[axis] < floor:
            raise RegistryError(
                f"{entity_id} does not meet {floor_name} {axis}{floor} floor"
            )


def validate_vector_for_acceptance(
    entity_id: str,
    classification: str,
    payload: dict[str, Any],
    states: dict[str, dict[str, Any]],
    logical_payloads: dict[str, dict[str, Any]],
) -> None:
    if classification not in PUBLIC_TRANSFER_CLASSIFICATIONS:
        raise RegistryError(
            f"{entity_id} accepted vector has private transfer classification"
        )
    if payload["traffic_light"] not in {"GREEN", "AMBER"}:
        raise RegistryError(f"{entity_id} vector is not transferable")
    if not payload["cases"]:
        raise RegistryError(f"{entity_id} accepted vector must contain cases")
    required_case_kinds = {"POSITIVE", "BOUNDARY", "REJECTION", "INVARIANT"}
    case_kinds: set[str] = set()
    for index, case in enumerate(payload["cases"]):
        if not isinstance(case, dict) or set(case) != {
            "kind",
            "inputs",
            "expected",
            "units",
            "tolerance",
        }:
            raise RegistryError(
                f"{entity_id} case {index} does not match the conformance schema"
            )
        if case["kind"] not in required_case_kinds:
            raise RegistryError(f"{entity_id} case {index} has invalid kind")
        if not isinstance(case["inputs"], dict) or not case["inputs"]:
            raise RegistryError(f"{entity_id} case {index} inputs are empty")
        if not isinstance(case["expected"], dict) or not case["expected"]:
            raise RegistryError(f"{entity_id} case {index} expected result is empty")
        if not isinstance(case["units"], str) or not case["units"]:
            raise RegistryError(f"{entity_id} case {index} units are required")
        if isinstance(case["tolerance"], (dict, list)) or case["tolerance"] is None:
            raise RegistryError(f"{entity_id} case {index} tolerance is invalid")
        case_kinds.add(case["kind"])
    if case_kinds != required_case_kinds:
        raise RegistryError(
            f"{entity_id} must cover positive, boundary, rejection and invariant cases"
        )
    detail = payload["detail"]["manifest"]
    if (
        detail["no_original_payload_declaration"]
        != "ATTESTED_NO_ORIGINAL_PAYLOAD"
    ):
        raise RegistryError(
            f"{entity_id} lacks a no-original-payload attestation"
        )
    for finding_id in payload["finding_ids"]:
        finding = states[finding_id]
        if finding["status"] != "ACCEPTED":
            raise RegistryError(
                f"{entity_id} source finding {finding_id} is not ACCEPTED"
            )
        if logical_payloads[finding_id]["campaign_id"] != payload["campaign_id"]:
            raise RegistryError(
                f"{entity_id} source finding {finding_id} is from another campaign"
            )


def validate_transfer_for_approval(
    entity_id: str,
    classification: str,
    payload: dict[str, Any],
    states: dict[str, dict[str, Any]],
    logical_payloads: dict[str, dict[str, Any]],
) -> None:
    if classification != "PUBLIC_SAFE_CANDIDATE":
        raise RegistryError(f"{entity_id} transfer must be PUBLIC_SAFE_CANDIDATE")
    if payload["traffic_light"] != "GREEN":
        raise RegistryError(
            f"{entity_id} Amber transfer requires explicit legal review"
        )
    if not payload["finding_ids"] or not payload["vector_ids"]:
        raise RegistryError(
            f"{entity_id} approved transfer requires findings and vectors"
        )
    campaigns: set[str] = set()
    for finding_id in payload["finding_ids"]:
        state = states[finding_id]
        if state["status"] != "ACCEPTED":
            raise RegistryError(f"{entity_id} finding {finding_id} is not ACCEPTED")
        finding_payload = logical_payloads[finding_id]
        validate_finding_for_acceptance(
            finding_id, state["classification"], finding_payload
        )
        campaigns.add(finding_payload["campaign_id"])
    for vector_id in payload["vector_ids"]:
        state = states[vector_id]
        if state["status"] != "ACCEPTED":
            raise RegistryError(f"{entity_id} vector {vector_id} is not ACCEPTED")
        vector_payload = logical_payloads[vector_id]
        if vector_payload["traffic_light"] != "GREEN":
            raise RegistryError(
                f"{entity_id} cannot include Amber vector {vector_id}"
            )
        validate_vector_for_acceptance(
            vector_id,
            state["classification"],
            vector_payload,
            states,
            logical_payloads,
        )
        campaigns.add(vector_payload["campaign_id"])
    if len(campaigns) != 1:
        raise RegistryError(f"{entity_id} transfer evidence spans multiple campaigns")


def validate_entity_payload(
    event: dict[str, Any],
    payload: dict[str, Any],
    states: dict[str, dict[str, Any]],
    hash_index: dict[str, dict[str, Any]],
    logical_payloads: dict[str, dict[str, Any]],
    authors: dict[str, str],
) -> None:
    entity_type = event["entity_type"]
    entity_id = event["entity_id"]
    validate_payload_types(entity_type, entity_id, payload)
    validate_detail_manifest(entity_type, entity_id, payload["detail"])

    if "evidence_axes" in payload:
        validate_evidence_axes(payload["evidence_axes"], entity_id)
    if entity_type == "source" and payload["source_class"] not in SOURCE_CLASSES:
        raise RegistryError(f"{entity_id} has invalid source_class")
    if entity_type == "experiment":
        if payload["reset_tier"] not in RESET_TIERS:
            raise RegistryError(f"{entity_id} has invalid reset_tier")
        if payload["input_determinism"] not in INPUT_DETERMINISM_GRADES:
            raise RegistryError(f"{entity_id} has invalid input_determinism")
    if entity_type == "run":
        if payload["reset_tier"] not in RESET_TIERS:
            raise RegistryError(f"{entity_id} has invalid reset_tier")
        if payload["terminal_disposition"] not in RUN_TERMINAL_DISPOSITIONS:
            raise RegistryError(f"{entity_id} has invalid terminal_disposition")
    if (
        entity_type == "calibration"
        and payload["detail"]["manifest"]["result_disposition"]
        not in CALIBRATION_RESULTS
    ):
        raise RegistryError(f"{entity_id} has invalid calibration result")
    if (
        entity_type == "contradiction"
        and payload["disposition"] not in CONTRADICTION_DISPOSITIONS
    ):
        raise RegistryError(f"{entity_id} has invalid contradiction disposition")
    if (
        entity_type == "protocol_deviation"
        and payload["validity_disposition"]
        not in PROTOCOL_DEVIATION_DISPOSITIONS
    ):
        raise RegistryError(
            f"{entity_id} has invalid protocol-deviation disposition"
        )
    if entity_type == "finding":
        if payload["fidelity_classification"] not in FIDELITY_CLASSIFICATIONS:
            raise RegistryError(f"{entity_id} has invalid fidelity_classification")
        if payload["evidence_floor"] not in EVIDENCE_FLOORS:
            raise RegistryError(f"{entity_id} has invalid evidence_floor")
        route_tags = payload["evidence_route_tags"]
        if not route_tags or not set(route_tags) <= EVIDENCE_ROUTE_TAGS:
            raise RegistryError(f"{entity_id} has invalid evidence_route_tags")
        rule_domains = payload["rule_domains"]
        if not rule_domains or not set(rule_domains) <= RULE_DOMAINS:
            raise RegistryError(f"{entity_id} has invalid rule_domains")
        if (
            set(rule_domains) & CRITICAL_RULE_DOMAINS
            and payload["evidence_floor"] != "CRITICAL"
        ):
            raise RegistryError(
                f"{entity_id} timing/determinism/network/economy/ordering "
                "rules require the CRITICAL evidence floor"
            )
    if entity_type == "hypothesis":
        route_tags = payload["evidence_route_tags"]
        if not route_tags or not set(route_tags) <= EVIDENCE_ROUTE_TAGS:
            raise RegistryError(f"{entity_id} has invalid evidence_route_tags")

    if entity_type == "vector" and payload["traffic_light"] == "RED":
        raise RegistryError(f"{entity_id} is a prohibited Red synthetic vector")
    if entity_type in {"vector", "transfer"} and payload["traffic_light"] not in {
        "GREEN",
        "AMBER",
    }:
        raise RegistryError(f"{entity_id} traffic_light must be GREEN or AMBER")

    validate_dependencies(entity_type, entity_id, payload, states)
    validate_nested_parent_id(entity_type, entity_id, payload)
    if entity_type == "campaign" and event["status"] == "OPEN":
        for calibration_id in payload["calibration_ids"]:
            if states[calibration_id]["status"] != "PASSED":
                raise RegistryError(
                    f"{entity_id} calibration {calibration_id} is not PASSED"
                )

    detail_manifest = payload["detail"]["manifest"]
    if (
        entity_type == "campaign"
        and detail_manifest["calibration_ids"] != payload["calibration_ids"]
    ):
        raise RegistryError(f"{entity_id} detail calibration_ids mismatch")
    if entity_type in {"hypothesis", "finding"}:
        if detail_manifest["question_id"] != payload["question_id"]:
            raise RegistryError(f"{entity_id} detail question_id mismatch")
        detail_evidence_field = (
            "evidence_route_ids" if entity_type == "hypothesis" else "evidence_ids"
        )
        if detail_manifest[detail_evidence_field] != payload["evidence_ids"]:
            raise RegistryError(f"{entity_id} detail evidence IDs mismatch")
    if (
        entity_type == "vector"
        and detail_manifest["finding_ids"] != payload["finding_ids"]
    ):
        raise RegistryError(f"{entity_id} detail finding_ids mismatch")

    if entity_type == "review":
        if payload["disposition"] not in REVIEW_DISPOSITIONS:
            raise RegistryError(f"{entity_id} has invalid review disposition")
        if payload["independence"] not in INDEPENDENCE:
            raise RegistryError(f"{entity_id} has invalid independence value")
        if (
            payload["disposition"] == "APPROVE"
            and payload["independence"] == "INHERITED_CONTEXT"
        ):
            raise RegistryError(
                f"{entity_id} inherited analyst context cannot approve a record"
            )
        if (
            not isinstance(payload["review_checks"], dict)
            or set(payload["review_checks"])
            != {"wording", "classification", "evidence", "uncertainty", "verification"}
            or not all(
                isinstance(value, bool)
                for value in payload["review_checks"].values()
            )
        ):
            raise RegistryError(f"{entity_id} review_checks are invalid")
        if (
            payload["disposition"] == "APPROVE"
            and not all(payload["review_checks"].values())
        ):
            raise RegistryError(f"{entity_id} cannot approve failed review checks")
        if (
            not isinstance(payload["review_context"], dict)
            or set(payload["review_context"])
            != {"context_source", "raw_context_inherited", "conflicts_disclosed"}
            or not isinstance(payload["review_context"]["context_source"], str)
            or not payload["review_context"]["context_source"]
            or not isinstance(payload["review_context"]["raw_context_inherited"], bool)
            or not isinstance(payload["review_context"]["conflicts_disclosed"], bool)
        ):
            raise RegistryError(f"{entity_id} review_context is invalid")
        if (
            payload["disposition"] == "APPROVE"
            and payload["review_context"]["raw_context_inherited"]
        ):
            raise RegistryError(
                f"{entity_id} inherited raw analyst context cannot approve"
            )
        referenced = hash_index.get(payload["target_event_hash"])
        if referenced is None:
            raise RegistryError(f"{entity_id} references unknown target event hash")
        if (
            referenced["entity_id"] != payload["target_id"]
            or referenced["event_hash"]
            != states[payload["target_id"]]["event_hash"]
        ):
            raise RegistryError(
                f"{entity_id} must review the exact latest target event"
            )
        reviewable_statuses = {
            "transfer": {"IN_REVIEW"},
            "finding": {"REVIEWED"},
            "vector": {"REVIEWED"},
            "product_deviation": {"DRAFT"},
            "tool": {"FROZEN"},
            "contradiction": {
                "NOT_OVERLAPPING_SCOPE",
                "VERSION_OR_MODE_SPLIT",
                "MEASUREMENT_ERROR",
                "HYPOTHESIS_FALSIFIED",
            },
        }
        if referenced["entity_type"] not in reviewable_statuses:
            raise RegistryError(
                f"{entity_id} cannot review {referenced['entity_type']}"
            )
        if referenced["status"] not in reviewable_statuses[referenced["entity_type"]]:
            raise RegistryError(
                f"{entity_id} target {payload['target_id']} is not reviewable "
                f"in {referenced['status']}"
            )
        target_state = states[payload["target_id"]]
        if event["classification"] != target_state["classification"]:
            raise RegistryError(f"{entity_id} review classification mismatch")
        if event["author"] == authors[payload["target_id"]]:
            raise RegistryError(f"{entity_id} reviewer is the target author")
        if (
            payload["detail"]["manifest"]["target_type"]
            != referenced["entity_type"]
            or payload["detail"]["manifest"]["target_record_hash"]
            != referenced["event_hash"]
        ):
            raise RegistryError(f"{entity_id} detail does not bind the review target")

    if entity_type == "receipt":
        referenced = hash_index.get(payload["transfer_event_hash"])
        if referenced is None:
            raise RegistryError(f"{entity_id} references unknown transfer event hash")
        if (
            referenced["entity_id"] != payload["transfer_id"]
            or referenced["entity_type"] != "transfer"
            or referenced["status"] != "APPROVED"
            or referenced["event_hash"]
            != states[payload["transfer_id"]]["event_hash"]
        ):
            raise RegistryError(
                f"{entity_id} must consume the exact latest approved transfer"
            )
        transfer_payload = logical_payloads[payload["transfer_id"]]
        if payload["destination_bead_id"] != transfer_payload["destination_bead_id"]:
            raise RegistryError(f"{entity_id} receipt destination mismatch")
        if event["classification"] != referenced["classification"]:
            raise RegistryError(f"{entity_id} receipt classification mismatch")
        receipt_detail = payload["detail"]["manifest"]
        transfer_detail = transfer_payload["detail"]["manifest"]
        expected_vector_hashes = sorted(
            states[vector_id]["event_hash"]
            for vector_id in transfer_payload["vector_ids"]
        )
        if sorted(receipt_detail["vector_event_hashes"]) != expected_vector_hashes:
            raise RegistryError(f"{entity_id} vector hashes do not match transfer")
        if receipt_detail["destination_files"] != transfer_detail[
            "destination_files"
        ]:
            raise RegistryError(f"{entity_id} destination files mismatch")
        implementation_hashes = receipt_detail["implementation_artifact_hashes"]
        if not isinstance(implementation_hashes, dict) or not implementation_hashes:
            raise RegistryError(
                f"{entity_id} implementation artifact hashes are required"
            )
        for name, digest in implementation_hashes.items():
            if (
                not isinstance(name, str)
                or not name
                or not isinstance(digest, str)
                or not HEX_256.fullmatch(digest)
            ):
                raise RegistryError(
                    f"{entity_id} has invalid implementation artifact hash"
                )


def validate_event_shape(event: dict[str, Any], line_number: int) -> None:
    required = {
        "schema_version",
        "event_id",
        "event_type",
        "entity_type",
        "entity_id",
        "entity_version",
        "timestamp",
        "author",
        "classification",
        "status",
        "previous_event_hash",
        "entity_previous_hash",
        "payload",
        "event_hash",
    }
    if set(event) != required:
        missing = required - set(event)
        extra = set(event) - required
        raise RegistryError(
            f"line {line_number}: event keys differ; "
            f"missing={sorted(missing)} extra={sorted(extra)}"
        )
    if event["schema_version"] != SCHEMA_VERSION:
        raise RegistryError(f"line {line_number}: unsupported schema version")
    if not isinstance(event["event_id"], str) or not EVENT_ID.fullmatch(
        event["event_id"]
    ):
        raise RegistryError(f"line {line_number}: invalid event_id")
    if event["event_type"] not in EVENT_TYPES:
        raise RegistryError(f"line {line_number}: invalid event_type")
    if event["entity_type"] not in ENTITY_PATTERNS:
        raise RegistryError(f"line {line_number}: invalid entity_type")
    pattern = ENTITY_PATTERNS[event["entity_type"]]
    if not isinstance(event["entity_id"], str) or not pattern.fullmatch(
        event["entity_id"]
    ):
        raise RegistryError(
            f"line {line_number}: invalid {event['entity_type']} ID "
            f"{event['entity_id']!r}"
        )
    if not isinstance(event["entity_version"], int) or event["entity_version"] < 1:
        raise RegistryError(f"line {line_number}: invalid entity_version")
    parse_timestamp(event["timestamp"])
    if not isinstance(event["author"], str) or not event["author"].strip():
        raise RegistryError(f"line {line_number}: author is required")
    if event["classification"] not in CLASSIFICATIONS:
        raise RegistryError(f"line {line_number}: invalid classification")
    if not isinstance(event["status"], str) or not re.fullmatch(
        r"[A-Z][A-Z0-9_]*", event["status"]
    ):
        raise RegistryError(f"line {line_number}: invalid status")
    for key in ("previous_event_hash", "entity_previous_hash"):
        value = event[key]
        if value is not None and (
            not isinstance(value, str) or not HEX_256.fullmatch(value)
        ):
            raise RegistryError(f"line {line_number}: invalid {key}")
    if not isinstance(event["payload"], dict):
        raise RegistryError(f"line {line_number}: payload must be an object")
    if not isinstance(event["event_hash"], str) or not HEX_256.fullmatch(
        event["event_hash"]
    ):
        raise RegistryError(f"line {line_number}: invalid event_hash")
    check_sha256_values(event["payload"])


def exact_independent_approvals(
    target_id: str,
    target_event_hash: str,
    states: dict[str, dict[str, Any]],
    logical_payloads: dict[str, dict[str, Any]],
    authors: dict[str, str],
) -> list[dict[str, Any]]:
    return [
        state
        for review_id, state in states.items()
        if state["entity_type"] == "review"
        and logical_payloads[review_id]["target_id"] == target_id
        and logical_payloads[review_id]["target_event_hash"] == target_event_hash
        and logical_payloads[review_id]["disposition"] == "APPROVE"
        and logical_payloads[review_id]["independence"] != "INHERITED_CONTEXT"
        and not logical_payloads[review_id]["review_context"][
            "raw_context_inherited"
        ]
        and authors[review_id] != authors[target_id]
    ]


def validate_reviewed_c1_resolution(
    entity_id: str,
    payload: dict[str, Any],
    states: dict[str, dict[str, Any]],
    logical_payloads: dict[str, dict[str, Any]],
    authors: dict[str, str],
) -> None:
    if payload["evidence_axes"]["C"] != 1:
        return
    contradiction_ids = payload["detail"]["manifest"]["contradiction_ids"]
    if not contradiction_ids:
        raise RegistryError(
            f"{entity_id} C1 requires a linked contradiction resolution"
        )
    resolved_statuses = {
        "NOT_OVERLAPPING_SCOPE",
        "VERSION_OR_MODE_SPLIT",
        "MEASUREMENT_ERROR",
        "HYPOTHESIS_FALSIFIED",
    }
    for contradiction_id in contradiction_ids:
        contradiction = states.get(contradiction_id)
        if (
            contradiction is None
            or contradiction["entity_type"] != "contradiction"
            or contradiction["status"] not in resolved_statuses
        ):
            raise RegistryError(
                f"{entity_id} C1 contradiction {contradiction_id} is not resolved"
            )
        if not exact_independent_approvals(
            contradiction_id,
            contradiction["event_hash"],
            states,
            logical_payloads,
            authors,
        ):
            raise RegistryError(
                f"{entity_id} C1 contradiction {contradiction_id} lacks an "
                "exact independent review"
            )


def find_cycles(graph: dict[str, set[str]]) -> list[list[str]]:
    cycles: list[list[str]] = []
    visiting: set[str] = set()
    visited: set[str] = set()
    stack: list[str] = []

    def visit(node: str) -> None:
        if node in visiting:
            start = stack.index(node)
            cycles.append(stack[start:] + [node])
            return
        if node in visited:
            return
        visiting.add(node)
        stack.append(node)
        for target in sorted(graph.get(node, set())):
            visit(target)
        stack.pop()
        visiting.remove(node)
        visited.add(node)

    for node in sorted(graph):
        visit(node)
    return cycles


def validate_final_graph(events: list[dict[str, Any]]) -> None:
    states = entity_state(events)
    authors = entity_authors(events)
    create_payloads = entity_create_payloads(events)
    logical_payloads = entity_logical_payloads(events)
    links: list[tuple[str, str, str]] = []
    acyclic: dict[str, set[str]] = defaultdict(set)
    fanout: dict[str, set[str]] = defaultdict(set)

    for event in events:
        if event["event_type"] != "LINK":
            continue
        source = event["entity_id"]
        target = event["payload"]["target_id"]
        relation = event["payload"]["relation"]
        links.append((source, target, relation))
        if relation in ACYCLIC_RELATIONS:
            acyclic[source].add(target)
        if relation in FANOUT_RELATIONS:
            fanout[source].add(target)

    for entity_id, state in states.items():
        payload = logical_payloads[entity_id]
        entity_type = state["entity_type"]
        for field in DEPENDENCY_FIELDS.get(entity_type, {}):
            # affected_ids names claims blocked by a contradiction; those are
            # invalidation targets, not evidence on which the contradiction
            # depends. Reviews are immutable historical dispositions rather
            # than downstream claims. Receipts are deliberately included.
            if field == "affected_ids" or entity_type == "review":
                continue
            for dependency_id in dependency_values(payload, field):
                fanout[dependency_id].add(entity_id)
        for dependency_id in references_in_payload(payload):
            fanout[dependency_id].add(entity_id)

    cycles = find_cycles(acyclic)
    if cycles:
        raise RegistryError(f"cyclic evidence lineage: {cycles[0]}")

    reviews_by_target: dict[str, list[dict[str, Any]]] = defaultdict(list)
    receipts_by_transfer: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for state in states.values():
        if state["entity_type"] == "review":
            payload = create_payloads[state["entity_id"]]
            reviews_by_target[payload["target_id"]].append(state)
        elif state["entity_type"] == "receipt":
            payload = create_payloads[state["entity_id"]]
            receipts_by_transfer[payload["transfer_id"]].append(state)

    for transfer_id, transfer in states.items():
        if transfer["entity_type"] != "transfer":
            continue
        if transfer["status"] in {"APPROVED", "CONSUMED"}:
            approvals = [
                review
                for review in reviews_by_target.get(transfer_id, [])
                if create_payloads[review["entity_id"]]["disposition"] == "APPROVE"
            ]
            if not approvals:
                raise RegistryError(f"{transfer_id} is approved without a review")
            for review in approvals:
                if authors[review["entity_id"]] == authors[transfer_id]:
                    raise RegistryError(f"{transfer_id} was self-approved")
        if transfer["status"] == "CONSUMED":
            current = [
                receipt
                for receipt in receipts_by_transfer.get(transfer_id, [])
                if receipt["status"] == "CURRENT"
                and create_payloads[receipt["entity_id"]]["destination_bead_id"]
                == logical_payloads[transfer_id]["destination_bead_id"]
            ]
            if not current:
                raise RegistryError(f"{transfer_id} is consumed without a receipt")
        if transfer["status"] in {"REVOKED", "SUPERSEDED", "RETEST_REQUIRED"}:
            for receipt in receipts_by_transfer.get(transfer_id, []):
                if receipt["status"] == "CURRENT":
                    raise RegistryError(
                        f"{receipt['entity_id']} is current for stale {transfer_id}"
                    )

    directly_invalidated: set[str] = set()
    for contradiction_id, contradiction in states.items():
        if (
            contradiction["entity_type"] == "contradiction"
            and contradiction["status"] in {"OPEN_BLOCKING", "UNRESOLVED_BLOCKING"}
        ):
            directly_invalidated.update(
                logical_payloads[contradiction_id]["affected_ids"]
            )
    for source, target, relation in links:
        source_state = states[source]["status"]
        if relation in {"invalidates", "revokes", "falsifies", "requires_retest"}:
            directly_invalidated.add(target)
        if relation == "contradicts" and source_state in {
            "OPEN_BLOCKING",
            "UNRESOLVED_BLOCKING",
        }:
            directly_invalidated.add(target)

    invalid_roots = {
        entity_id
        for entity_id, state in states.items()
        if state["status"] in INVALIDATING_STATUSES
    } | directly_invalidated

    affected: set[str] = set()
    queue: deque[str] = deque(invalid_roots)
    while queue:
        source = queue.popleft()
        for target in fanout.get(source, set()):
            if target not in affected:
                affected.add(target)
                queue.append(target)

    for entity_id in sorted(affected | directly_invalidated):
        state = states[entity_id]
        if state["entity_type"] == "receipt":
            if state["status"] not in STALE_RECEIPT_STATUSES:
                raise RegistryError(f"{entity_id} must be stale after invalidation")
        elif state["status"] not in SAFE_DEPENDENT_STATUSES:
            raise RegistryError(
                f"{entity_id} must be blocked/revoked/retested after upstream "
                "invalidation"
            )


def validate_events(events: list[dict[str, Any]]) -> None:
    previous_global: str | None = None
    entity_latest: dict[str, dict[str, Any]] = {}
    logical_payloads: dict[str, dict[str, Any]] = {}
    authors: dict[str, str] = {}
    event_ids: set[str] = set()
    event_hashes: set[str] = set()
    hash_index: dict[str, dict[str, Any]] = {}

    for index, event in enumerate(events, start=1):
        validate_event_shape(event, index)
        expected_event_id = f"EVT-{index:012d}"
        if event["event_id"] != expected_event_id:
            raise RegistryError(
                f"line {index}: expected event_id {expected_event_id}, "
                f"found {event['event_id']}"
            )
        if event["event_id"] in event_ids:
            raise RegistryError(f"line {index}: reused event_id")
        if event["event_hash"] in event_hashes:
            raise RegistryError(f"line {index}: reused event_hash")
        if event["previous_event_hash"] != previous_global:
            raise RegistryError(f"line {index}: broken global prior hash")

        without_hash = {key: value for key, value in event.items() if key != "event_hash"}
        expected_hash = hash_event(without_hash)
        if event["event_hash"] != expected_hash:
            raise RegistryError(f"line {index}: event content hash mismatch")

        entity_id = event["entity_id"]
        previous_entity = entity_latest.get(entity_id)
        if event["event_type"] == "CREATE":
            if previous_entity is not None:
                raise RegistryError(f"line {index}: reused entity ID {entity_id}")
            if event["entity_version"] != 1 or event["entity_previous_hash"] is not None:
                raise RegistryError(f"line {index}: invalid CREATE predecessor")
            if event["status"] not in INITIAL_STATUSES[event["entity_type"]]:
                raise RegistryError(
                    f"line {index}: invalid initial status for {event['entity_type']}"
                )
            validate_entity_payload(
                event,
                event["payload"],
                entity_latest,
                hash_index,
                logical_payloads,
                authors,
            )
            logical_payloads[entity_id] = dict(event["payload"])
            authors[entity_id] = event["author"]
        else:
            if previous_entity is None:
                raise RegistryError(f"line {index}: event precedes CREATE for {entity_id}")
            if event["entity_type"] != previous_entity["entity_type"]:
                raise RegistryError(f"line {index}: entity type changed for {entity_id}")
            if event["classification"] != previous_entity["classification"]:
                raise RegistryError(f"line {index}: classification changed for {entity_id}")
            if event["entity_version"] != previous_entity["entity_version"] + 1:
                raise RegistryError(f"line {index}: entity version is not sequential")
            if event["entity_previous_hash"] != previous_entity["event_hash"]:
                raise RegistryError(f"line {index}: broken entity prior hash")
            if event["event_type"] == "TRANSITION":
                if not status_transition_allowed(
                    event["entity_type"],
                    previous_entity["status"],
                    event["status"],
                ):
                    raise RegistryError(
                        f"line {index}: invalid transition "
                        f"{previous_entity['status']} -> {event['status']}"
                    )
                review_transition = (
                    event["entity_type"] in {"finding", "vector"}
                    and event["status"] in {"REVIEWED", "ACCEPTED"}
                )
                required_transition_fields = (
                    {"reason", "independence"}
                    if review_transition
                    else {"reason"}
                )
                if (
                    set(event["payload"]) != required_transition_fields
                    or not isinstance(event["payload"]["reason"], str)
                    or not event["payload"]["reason"]
                ):
                    raise RegistryError(
                        f"line {index}: transition payload fields differ or "
                        "reason is missing"
                    )
                if review_transition:
                    if event["payload"]["independence"] not in INDEPENDENCE:
                        raise RegistryError(
                            f"line {index}: invalid reviewer independence"
                        )
                    if event["payload"]["independence"] == "INHERITED_CONTEXT":
                        raise RegistryError(
                            f"line {index}: inherited context cannot independently "
                            f"{event['status'].lower()} {event['entity_type']}"
                        )
                    if event["author"] == authors[entity_id]:
                        raise RegistryError(
                            f"line {index}: {event['entity_type']} author cannot "
                            f"self-{event['status'].lower()}"
                        )
                if event["entity_type"] == "run" and event["status"] in {
                    "VALID",
                    "INVALID",
                    "EXCLUDED",
                    "ABORTED",
                }:
                    expected_terminal = {
                        "VALID": "SUCCESS",
                        "INVALID": "INVALID",
                        "EXCLUDED": "EXCLUDED",
                        "ABORTED": "ABORTED",
                    }[event["status"]]
                    if (
                        logical_payloads[entity_id]["terminal_disposition"]
                        != expected_terminal
                    ):
                        raise RegistryError(
                            f"line {index}: run status and terminal_disposition "
                            "differ"
                        )
                if (
                    event["entity_type"] == "calibration"
                    and event["status"] in {"PASSED", "FAILED"}
                ):
                    result_disposition = logical_payloads[entity_id]["detail"][
                        "manifest"
                    ]["result_disposition"]
                    if (
                        event["status"] == "PASSED"
                        and result_disposition not in {"PASS", "EQUIVALENT"}
                    ) or (
                        event["status"] == "FAILED"
                        and result_disposition != "FAIL"
                    ):
                        raise RegistryError(
                            f"line {index}: calibration status and result differ"
                        )
                if (
                    event["entity_type"] == "contradiction"
                    and event["status"]
                    in CONTRADICTION_DISPOSITIONS - {"PENDING"}
                    and logical_payloads[entity_id]["disposition"]
                    != event["status"]
                ):
                    raise RegistryError(
                        f"line {index}: contradiction status and disposition differ"
                    )
                if (
                    event["entity_type"] == "finding"
                    and event["status"] == "ACCEPTED"
                ):
                    validate_finding_for_acceptance(
                        entity_id,
                        event["classification"],
                        logical_payloads[entity_id],
                    )
                    validate_reviewed_c1_resolution(
                        entity_id,
                        logical_payloads[entity_id],
                        entity_latest,
                        logical_payloads,
                        authors,
                    )
                    if not exact_independent_approvals(
                        entity_id,
                        previous_entity["event_hash"],
                        entity_latest,
                        logical_payloads,
                        authors,
                    ):
                        raise RegistryError(
                            f"{entity_id} acceptance lacks an exact independent "
                            "review"
                        )
                elif (
                    event["entity_type"] == "vector"
                    and event["status"] == "ACCEPTED"
                ):
                    validate_vector_for_acceptance(
                        entity_id,
                        event["classification"],
                        logical_payloads[entity_id],
                        entity_latest,
                        logical_payloads,
                    )
                    if not exact_independent_approvals(
                        entity_id,
                        previous_entity["event_hash"],
                        entity_latest,
                        logical_payloads,
                        authors,
                    ):
                        raise RegistryError(
                            f"{entity_id} acceptance lacks an exact independent "
                            "review"
                        )
                elif (
                    event["entity_type"] == "product_deviation"
                    and event["status"] == "APPROVED"
                ):
                    if not exact_independent_approvals(
                        entity_id,
                        previous_entity["event_hash"],
                        entity_latest,
                        logical_payloads,
                        authors,
                    ):
                        raise RegistryError(
                            f"{entity_id} approval lacks an exact independent "
                            "review"
                        )
                elif (
                    event["entity_type"] == "transfer"
                    and event["status"] == "APPROVED"
                ):
                    validate_transfer_for_approval(
                        entity_id,
                        event["classification"],
                        logical_payloads[entity_id],
                        entity_latest,
                        logical_payloads,
                    )
                    approvals = exact_independent_approvals(
                        entity_id,
                        previous_entity["event_hash"],
                        entity_latest,
                        logical_payloads,
                        authors,
                    )
                    if not approvals:
                        raise RegistryError(
                            f"{entity_id} approval lacks an exact independent review"
                        )
                elif (
                    event["entity_type"] == "transfer"
                    and event["status"] == "CONSUMED"
                ):
                    transfer_payload = logical_payloads[entity_id]
                    receipts = [
                        state
                        for receipt_id, state in entity_latest.items()
                        if state["entity_type"] == "receipt"
                        and state["status"] == "CURRENT"
                        and logical_payloads[receipt_id]["transfer_id"] == entity_id
                        and logical_payloads[receipt_id]["transfer_event_hash"]
                        == previous_entity["event_hash"]
                        and logical_payloads[receipt_id]["destination_bead_id"]
                        == transfer_payload["destination_bead_id"]
                    ]
                    if not receipts:
                        raise RegistryError(
                            f"{entity_id} consumption lacks an exact current receipt"
                        )
            elif event["event_type"] == "AMEND":
                if event["status"] != previous_entity["status"]:
                    raise RegistryError(f"line {index}: AMEND changed status")
                if (
                    set(event["payload"]) != {"reason", "changes"}
                    or not isinstance(event["payload"]["reason"], str)
                    or not event["payload"]["reason"]
                    or not isinstance(event["payload"]["changes"], dict)
                    or not event["payload"]["changes"]
                ):
                    raise RegistryError(f"line {index}: amendment reason is required")
                if previous_entity["status"] not in AMENDABLE_STATUSES.get(
                    event["entity_type"], set()
                ):
                    raise RegistryError(
                        f"line {index}: {event['entity_type']} is immutable in "
                        f"{previous_entity['status']}"
                    )
                merged_payload = {
                    **logical_payloads[entity_id],
                    **event["payload"]["changes"],
                }
                validate_entity_payload(
                    event,
                    merged_payload,
                    entity_latest,
                    hash_index,
                    logical_payloads,
                    authors,
                )
                logical_payloads[entity_id] = merged_payload
            elif event["event_type"] == "LINK":
                if event["status"] != previous_entity["status"]:
                    raise RegistryError(f"line {index}: LINK changed status")
                if event["entity_type"] in LINK_IMMUTABLE_ENTITY_TYPES:
                    raise RegistryError(
                        f"line {index}: LINK cannot version immutable "
                        f"{event['entity_type']}"
                    )
                target = event["payload"].get("target_id")
                relation = event["payload"].get("relation")
                if not isinstance(target, str) or target not in entity_latest:
                    raise RegistryError(f"line {index}: LINK target is unknown")
                if target == entity_id:
                    raise RegistryError(f"line {index}: self-link is forbidden")
                if relation not in RELATIONS:
                    raise RegistryError(f"line {index}: invalid relation")
                if set(event["payload"]) != {"target_id", "relation"}:
                    raise RegistryError(f"line {index}: LINK payload fields differ")

        event_ids.add(event["event_id"])
        event_hashes.add(event["event_hash"])
        hash_index[event["event_hash"]] = event
        entity_latest[entity_id] = event
        previous_global = event["event_hash"]

    validate_final_graph(events)


def load_events(path: Path, check_canonical: bool = True) -> list[dict[str, Any]]:
    if not path.exists():
        raise RegistryError(f"missing registry event stream: {path}")
    events: list[dict[str, Any]] = []
    with path.open("rb") as handle:
        for line_number, raw in enumerate(handle, start=1):
            if not raw.endswith(b"\n"):
                raise RegistryError(f"line {line_number}: partial JSONL record")
            try:
                event = json.loads(raw)
            except json.JSONDecodeError as error:
                raise RegistryError(f"line {line_number}: invalid JSON") from error
            if not isinstance(event, dict):
                raise RegistryError(f"line {line_number}: event is not an object")
            if check_canonical and raw != canonical_bytes(event):
                raise RegistryError(f"line {line_number}: event is not canonical JSON")
            events.append(event)
    return events


def validate_storage(registry_dir: Path) -> None:
    registry_dir = absolute_without_resolving(registry_dir)
    reject_symlink_components(registry_dir)
    if not registry_dir.is_dir():
        raise RegistryError(f"registry directory does not exist: {registry_dir}")
    ensure_owner_only(registry_dir, directory=True)
    for path in registry_dir.iterdir():
        ensure_owner_only(path, directory=path.is_dir())
    for name in (EVENTS_NAME, LOCK_NAME, META_NAME):
        ensure_owner_only(registry_dir / name, directory=False)


def initialize_registry(registry_dir: Path, author: str) -> None:
    registry_dir = absolute_without_resolving(registry_dir)
    reject_symlink_components(registry_dir.parent)
    registry_dir.mkdir(mode=0o700, parents=True, exist_ok=False)
    os.chmod(registry_dir, 0o700)
    metadata = {
        "schema_version": SCHEMA_VERSION,
        "created_at": utc_now(),
        "created_by": author,
        "authority": EVENTS_NAME,
        "classification": "PRIVATE_ANALYSIS",
    }
    for name, content in (
        (META_NAME, canonical_bytes(metadata)),
        (EVENTS_NAME, b""),
        (LOCK_NAME, b""),
    ):
        descriptor = os.open(
            registry_dir / name,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL,
            0o600,
        )
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())


def next_event(
    events: list[dict[str, Any]],
    spec: dict[str, Any],
) -> dict[str, Any]:
    allowed = {
        "event_type",
        "entity_type",
        "entity_id",
        "author",
        "classification",
        "status",
        "payload",
        "timestamp",
    }
    if not isinstance(spec, dict) or not set(spec) <= allowed:
        raise RegistryError(
            f"event spec has unsupported keys: {sorted(set(spec) - allowed)}"
        )
    required = allowed - {"timestamp"}
    missing = required - set(spec)
    if missing:
        raise RegistryError(f"event spec missing fields: {sorted(missing)}")

    latest = entity_state(events).get(spec["entity_id"])
    event_type = spec["event_type"]
    if event_type == "CREATE":
        entity_version = 1
        entity_previous_hash = None
    else:
        if latest is None:
            raise RegistryError("non-CREATE event has no existing entity")
        entity_version = latest["entity_version"] + 1
        entity_previous_hash = latest["event_hash"]

    event_without_hash = {
        "schema_version": SCHEMA_VERSION,
        "event_id": f"EVT-{len(events) + 1:012d}",
        "event_type": event_type,
        "entity_type": spec["entity_type"],
        "entity_id": spec["entity_id"],
        "entity_version": entity_version,
        "timestamp": spec.get("timestamp", utc_now()),
        "author": spec["author"],
        "classification": spec["classification"],
        "status": spec["status"],
        "previous_event_hash": events[-1]["event_hash"] if events else None,
        "entity_previous_hash": entity_previous_hash,
        "payload": spec["payload"],
    }
    return {
        **event_without_hash,
        "event_hash": hash_event(event_without_hash),
    }


def replace_event_stream(
    registry_dir: Path,
    events: list[dict[str, Any]],
) -> None:
    events_path = registry_dir / EVENTS_NAME
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{EVENTS_NAME}.",
        suffix=".staged",
        dir=registry_dir,
    )
    temporary_path = Path(temporary_name)
    replaced = False
    try:
        with os.fdopen(descriptor, "wb", buffering=0) as stream:
            for event in events:
                content = canonical_bytes(event)
                written = 0
                while written < len(content):
                    count = stream.write(content[written:])
                    if count is None or count <= 0:
                        raise OSError("short write while staging registry")
                    written += count
            os.fsync(stream.fileno())
        os.replace(temporary_path, events_path)
        replaced = True
        directory_descriptor = os.open(
            registry_dir,
            os.O_RDONLY | getattr(os, "O_DIRECTORY", 0),
        )
        try:
            os.fsync(directory_descriptor)
        finally:
            os.close(directory_descriptor)
    finally:
        if not replaced:
            try:
                temporary_path.unlink()
            except FileNotFoundError:
                pass


def append_spec(registry_dir: Path, spec: dict[str, Any]) -> dict[str, Any]:
    return append_specs(registry_dir, [spec])[0]


def append_specs(
    registry_dir: Path, specs: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    if not isinstance(specs, list) or not specs:
        raise RegistryError("append batch must contain at least one event spec")
    registry_dir = absolute_without_resolving(registry_dir)
    validate_storage(registry_dir)
    lock_path = registry_dir / LOCK_NAME
    events_path = registry_dir / EVENTS_NAME
    with lock_path.open("r+b", buffering=0) as lock:
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
        events = load_events(events_path)
        if events:
            validate_events(events)
        appended: list[dict[str, Any]] = []
        candidate = list(events)
        for spec in specs:
            event = next_event(candidate, spec)
            candidate.append(event)
            appended.append(event)
        validate_events(candidate)
        replace_event_stream(registry_dir, candidate)
        fcntl.flock(lock.fileno(), fcntl.LOCK_UN)
    return appended


def render_markdown(events: list[dict[str, Any]]) -> str:
    states = entity_state(events)
    root = events[-1]["event_hash"] if events else "EMPTY"
    lines = [
        "# Private evidence registry",
        "",
        f"- Events: {len(events)}",
        f"- Entities: {len(states)}",
        f"- Root hash: `{root}`",
        "",
        "| Entity | Type | Version | Status | Classification |",
        "|---|---|---:|---|---|",
    ]
    for entity_id in sorted(states):
        event = states[entity_id]
        lines.append(
            f"| `{entity_id}` | {event['entity_type']} | "
            f"{event['entity_version']} | {event['status']} | "
            f"{event['classification']} |"
        )
    lines.append("")
    return "\n".join(lines)


def read_spec(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise RegistryError("event spec must be a JSON object")
    return value


def read_specs(path: Path) -> list[dict[str, Any]]:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, list) or not all(
        isinstance(item, dict) for item in value
    ):
        raise RegistryError("event batch must be a non-empty JSON array of objects")
    if not value:
        raise RegistryError("event batch must contain at least one event spec")
    return value


def command_init(args: argparse.Namespace) -> int:
    initialize_registry(args.registry, args.author)
    print(
        json.dumps(
            {
                "registry": str(args.registry.resolve()),
                "schema_version": SCHEMA_VERSION,
                "status": "initialized",
            },
            sort_keys=True,
        )
    )
    return 0


def command_append(args: argparse.Namespace) -> int:
    event = append_spec(args.registry, read_spec(args.spec))
    print(json.dumps(event, sort_keys=True))
    return 0


def command_append_batch(args: argparse.Namespace) -> int:
    events = append_specs(args.registry, read_specs(args.specs))
    print(json.dumps(events, sort_keys=True))
    return 0


def command_validate(args: argparse.Namespace) -> int:
    validate_storage(args.registry)
    events = load_events(args.registry / EVENTS_NAME)
    validate_events(events)
    print(
        json.dumps(
            {
                "registry": str(args.registry.resolve()),
                "events": len(events),
                "entities": len(entity_state(events)),
                "root_hash": events[-1]["event_hash"] if events else None,
                "status": "valid",
            },
            sort_keys=True,
        )
    )
    return 0


def command_root(args: argparse.Namespace) -> int:
    events = load_events(args.registry / EVENTS_NAME)
    validate_events(events)
    print(events[-1]["event_hash"] if events else "EMPTY")
    return 0


def command_render(args: argparse.Namespace) -> int:
    events = load_events(args.registry / EVENTS_NAME)
    validate_events(events)
    if args.format == "json":
        states = entity_state(events)
        print(
            json.dumps(
                {
                    "root_hash": events[-1]["event_hash"] if events else None,
                    "entities": states,
                },
                indent=2,
                sort_keys=True,
            )
        )
    else:
        print(render_markdown(events), end="")
    return 0


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser()
    subparsers = result.add_subparsers(dest="command", required=True)

    init_parser = subparsers.add_parser("init")
    init_parser.add_argument("--registry", required=True, type=Path)
    init_parser.add_argument("--author", required=True)
    init_parser.set_defaults(function=command_init)

    append_parser = subparsers.add_parser("append")
    append_parser.add_argument("--registry", required=True, type=Path)
    append_parser.add_argument("--spec", required=True, type=Path)
    append_parser.set_defaults(function=command_append)

    append_batch_parser = subparsers.add_parser("append-batch")
    append_batch_parser.add_argument("--registry", required=True, type=Path)
    append_batch_parser.add_argument("--specs", required=True, type=Path)
    append_batch_parser.set_defaults(function=command_append_batch)

    validate_parser = subparsers.add_parser("validate")
    validate_parser.add_argument("--registry", required=True, type=Path)
    validate_parser.set_defaults(function=command_validate)

    root_parser = subparsers.add_parser("root")
    root_parser.add_argument("--registry", required=True, type=Path)
    root_parser.set_defaults(function=command_root)

    render_parser = subparsers.add_parser("render")
    render_parser.add_argument("--registry", required=True, type=Path)
    render_parser.add_argument("--format", choices=("markdown", "json"), default="markdown")
    render_parser.set_defaults(function=command_render)

    return result


def main() -> int:
    args = parser().parse_args()
    try:
        return args.function(args)
    except (OSError, RegistryError, json.JSONDecodeError) as error:
        print(f"registry error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
