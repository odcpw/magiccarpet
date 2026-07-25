#!/usr/bin/env python3
"""Deterministic, known-source P1-P5 registry smoke scenarios.

This module writes only to an explicit caller-supplied output directory. It
uses the adjacent private registry implementation and Python's standard
library. It contains no network code and no owned-game data.
"""

from __future__ import annotations

import ast
import copy
import hashlib
import json
import os
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable


BASE = Path(__file__).resolve().parent
FIXTURES = BASE / "fixtures"
PACKETS = BASE / "packets"
REGISTRY_SOURCE = BASE.parent / "registry"
EXPECTED_PATH = BASE / "expected.json"
PYTHON = Path(sys.executable).resolve()
OS_RELEASE = Path("/etc/os-release")

sys.path.insert(0, str(REGISTRY_SOURCE))
import registry  # noqa: E402


class ScenarioError(RuntimeError):
    """Raised when a preregistered synthetic observation does not match."""


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def bytes_sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def json_bytes(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=True, separators=(",", ":"), sort_keys=True)
        + "\n"
    ).encode("ascii")


def write_private_json(path: Path, value: Any) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "wb") as handle:
        handle.write(json_bytes(value))
        handle.flush()
        os.fsync(handle.fileno())


def load_packet(phase: str) -> dict[str, Any]:
    path = PACKETS / f"{phase.upper()}.json"
    with path.open("r", encoding="utf-8") as handle:
        packet = json.load(handle)
    if packet.get("phase") != phase.upper():
        raise ScenarioError(f"{path} has wrong phase")
    return packet


def require_file_hash(path: Path, expected: str) -> None:
    actual = file_sha256(path)
    if actual != expected:
        raise ScenarioError(
            f"hash mismatch for {path.name}: expected {expected}, actual {actual}"
        )


class FixedClock:
    """Allocate deterministic, distinct RFC3339 timestamps."""

    def __init__(self, phase_number: int) -> None:
        self.phase_number = phase_number
        self.sequence = 0

    def next(self) -> str:
        self.sequence += 1
        minute, second = divmod(self.sequence, 60)
        hour, minute = divmod(minute, 60)
        return (
            f"2026-07-{phase_number_to_day(self.phase_number):02d}"
            f"T{hour:02d}:{minute:02d}:{second:02d}Z"
        )


def phase_number_to_day(number: int) -> int:
    return 10 + number


def _default_detail_value(entity_type: str, entity_id: str, field_name: str) -> Any:
    description = (
        f"Known-source synthetic {field_name.replace('_', ' ')} for {entity_id}"
    )
    if field_name in registry.DETAIL_STRING_FIELDS:
        return description
    if field_name in registry.DETAIL_LIST_FIELDS:
        return [description]
    return {
        "description": description,
        "entity_type": entity_type,
        "synthetic_only": True,
    }


def detail_for(
    entity_type: str,
    entity_id: str,
    payload: dict[str, Any],
    *,
    overrides: dict[str, Any] | None = None,
) -> dict[str, Any]:
    manifest = {
        field_name: _default_detail_value(entity_type, entity_id, field_name)
        for field_name in registry.DETAIL_REQUIRED_FIELDS[entity_type]
    }
    manifest.update(overrides or {})

    if entity_type == "source":
        manifest.update(
            {
                "revision": "1",
                "origin_lineage": "Project-authored known-source synthetic fixture",
                "license": "Project-authored synthetic fixture",
                "license_risk_disposition": "GREEN",
                "evidence_grade": "KNOWN_SOURCE_SYNTHETIC",
                "approved_use": "Private protocol smoke qualification only",
                "exposure": {"execution": "local", "egress": "none"},
                "retention_disposition": {
                    "purpose": "protocol qualification",
                    "state": "private_or_temporary",
                },
                "modification": {"kind": "UNMODIFIED"},
            }
        )
    elif entity_type == "build_manifest":
        manifest["config_hashes"] = {
            "packet.json": next(iter(payload["artifact_hashes"].values()))
        }
        manifest["modification"] = {"kind": "SYNTHETIC_ORIGINAL"}
    elif entity_type == "tool":
        manifest["dependency_hashes"] = {
            "python": payload["artifact_hashes"]["python"]
        }
        manifest["qualification_status"] = "PINNED_FOR_SYNTHETIC_SMOKE"
    elif entity_type == "calibration":
        manifest["result_disposition"] = "PASS"
    elif entity_type == "campaign":
        manifest["calibration_ids"] = payload["calibration_ids"]
    elif entity_type == "hypothesis":
        manifest["question_id"] = payload["question_id"]
        manifest["evidence_route_ids"] = payload["evidence_ids"]
        manifest.setdefault("contradiction_ids", ["NONE"])
    elif entity_type == "finding":
        manifest["question_id"] = payload["question_id"]
        manifest["evidence_ids"] = payload["evidence_ids"]
        manifest.setdefault("contradiction_ids", ["NONE"])
    elif entity_type == "vector":
        manifest["finding_ids"] = payload["finding_ids"]
        manifest.setdefault(
            "no_original_payload_declaration",
            "ATTESTED_NO_ORIGINAL_PAYLOAD",
        )
    elif entity_type == "review":
        target_id = payload["target_id"]
        target_type = next(
            kind
            for kind, pattern in registry.ENTITY_PATTERNS.items()
            if kind != "review" and pattern.fullmatch(target_id)
        )
        manifest["target_type"] = target_type
        manifest["target_record_hash"] = payload["target_event_hash"]

    envelope = {
        "schema_id": f"mc2-behavioral-registry/{entity_type}/0.2",
        "artifact_id": f"DET-{entity_id}",
        "manifest": manifest,
    }
    envelope["manifest_sha256"] = registry.detail_manifest_sha256(manifest)
    return envelope


def create_spec(
    clock: FixedClock,
    entity_type: str,
    entity_id: str,
    status: str,
    payload: dict[str, Any],
    *,
    author: str = "analyst-a",
    classification: str = "SYNTHETIC_ONLY",
    detail_overrides: dict[str, Any] | None = None,
) -> dict[str, Any]:
    full_payload = copy.deepcopy(payload)
    full_payload["detail"] = detail_for(
        entity_type,
        entity_id,
        full_payload,
        overrides=detail_overrides,
    )
    return {
        "event_type": "CREATE",
        "entity_type": entity_type,
        "entity_id": entity_id,
        "author": author,
        "classification": classification,
        "status": status,
        "payload": full_payload,
        "timestamp": clock.next(),
    }


def transition_spec(
    clock: FixedClock,
    state: dict[str, Any],
    status: str,
    *,
    author: str = "analyst-a",
    reason: str,
    independence: str = "FRESH_NO_CONTEXT",
) -> dict[str, Any]:
    payload: dict[str, Any] = {"reason": reason}
    if state["entity_type"] in {"finding", "vector"} and status in {
        "REVIEWED",
        "ACCEPTED",
    }:
        payload["independence"] = independence
    return {
        "event_type": "TRANSITION",
        "entity_type": state["entity_type"],
        "entity_id": state["entity_id"],
        "author": author,
        "classification": state["classification"],
        "status": status,
        "payload": payload,
        "timestamp": clock.next(),
    }


def amend_spec(
    clock: FixedClock,
    state: dict[str, Any],
    changes: dict[str, Any],
    *,
    reason: str,
    author: str = "analyst-a",
) -> dict[str, Any]:
    return {
        "event_type": "AMEND",
        "entity_type": state["entity_type"],
        "entity_id": state["entity_id"],
        "author": author,
        "classification": state["classification"],
        "status": state["status"],
        "payload": {"reason": reason, "changes": changes},
        "timestamp": clock.next(),
    }


def link_spec(
    clock: FixedClock,
    state: dict[str, Any],
    target_id: str,
    relation: str,
    *,
    reason_author: str = "analyst-a",
) -> dict[str, Any]:
    return {
        "event_type": "LINK",
        "entity_type": state["entity_type"],
        "entity_id": state["entity_id"],
        "author": reason_author,
        "classification": state["classification"],
        "status": state["status"],
        "payload": {"target_id": target_id, "relation": relation},
        "timestamp": clock.next(),
    }


def review_payload(
    target: dict[str, Any],
    *,
    disposition: str = "APPROVE",
    independence: str = "FRESH_NO_CONTEXT",
    raw_context_inherited: bool = False,
) -> dict[str, Any]:
    return {
        "target_id": target["entity_id"],
        "target_event_hash": target["event_hash"],
        "disposition": disposition,
        "independence": independence,
        "review_checks": {
            "wording": True,
            "classification": True,
            "evidence": True,
            "uncertainty": True,
            "verification": True,
        },
        "review_context": {
            "context_source": "Fresh synthetic packet and exact target only",
            "raw_context_inherited": raw_context_inherited,
            "conflicts_disclosed": True,
        },
    }


def conformance_cases() -> list[dict[str, Any]]:
    return [
        {
            "kind": "POSITIVE",
            "inputs": {"value": 10},
            "expected": {"accepted": True},
            "units": "synthetic_units",
            "tolerance": 0,
        },
        {
            "kind": "BOUNDARY",
            "inputs": {"value": 99},
            "expected": {"accepted": True},
            "units": "synthetic_units",
            "tolerance": 0,
        },
        {
            "kind": "REJECTION",
            "inputs": {"value": -1},
            "expected": {"accepted": False},
            "units": "synthetic_units",
            "tolerance": 0,
        },
        {
            "kind": "INVARIANT",
            "inputs": {"value": 10},
            "expected": {"repeatable": True},
            "units": "synthetic_units",
            "tolerance": 0,
        },
    ]


@dataclass
class PhaseContext:
    phase: str
    packet: dict[str, Any]
    directory: Path
    registry_dir: Path
    clock: FixedClock
    negative_cases: list[dict[str, Any]] = field(default_factory=list)

    @property
    def events_path(self) -> Path:
        return self.registry_dir / registry.EVENTS_NAME

    def events(self) -> list[dict[str, Any]]:
        return registry.load_events(self.events_path)

    def states(self) -> dict[str, dict[str, Any]]:
        return registry.entity_state(self.events())

    def payloads(self) -> dict[str, dict[str, Any]]:
        return registry.entity_logical_payloads(self.events())

    def append(self, spec: dict[str, Any]) -> dict[str, Any]:
        return registry.append_spec(self.registry_dir, spec)

    def append_batch(self, specs: list[dict[str, Any]]) -> list[dict[str, Any]]:
        return registry.append_specs(self.registry_dir, specs)

    def root(self) -> str:
        events = self.events()
        return events[-1]["event_hash"] if events else "EMPTY"

    def expect_failure(
        self,
        case_id: str,
        operation: Callable[[], Any],
        expected_text: str,
    ) -> None:
        before = self.events_path.read_bytes()
        before_root = self.root()
        try:
            operation()
        except (registry.RegistryError, ScenarioError) as error:
            message = str(error)
            if expected_text not in message:
                raise ScenarioError(
                    f"{case_id} expected error containing {expected_text!r}, "
                    f"received {message!r}"
                ) from error
        else:
            raise ScenarioError(f"{case_id} unexpectedly succeeded")
        after = self.events_path.read_bytes()
        after_root = self.root()
        if after != before or after_root != before_root:
            raise ScenarioError(f"{case_id} changed registry bytes or root")
        self.negative_cases.append(
            {
                "case_id": case_id,
                "error_contains": expected_text,
                "events_sha256": bytes_sha256(after),
                "root_hash": after_root,
                "preserved": True,
            }
        )


def _mkdir_private(path: Path) -> None:
    os.mkdir(path, 0o700)
    os.chmod(path, 0o700)


def start_phase(output_root: Path, phase: str) -> PhaseContext:
    packet = load_packet(phase)
    phase_number = int(phase[1:])
    phase_dir = output_root / phase.lower()
    _mkdir_private(phase_dir)
    registry_dir = phase_dir / "registry"
    registry.initialize_registry(registry_dir, "protocol-smoke-owner")
    context = PhaseContext(
        phase=phase,
        packet=packet,
        directory=phase_dir,
        registry_dir=registry_dir,
        clock=FixedClock(phase_number),
    )

    fixture_hashes: dict[str, str] = {}
    for relative_name, expected_hash in packet["fixtures"].items():
        fixture_path = BASE / relative_name
        require_file_hash(fixture_path, expected_hash)
        fixture_hashes[fixture_path.name] = expected_hash

    suffix = phase
    source_id = f"SRC-SYNTH-{suffix}-0001"
    build_id = f"BLD-SYNTH-{suffix}-0001"
    tool_id = f"TOOL-SYNTH-{suffix}-0001"
    environment_id = f"ENV-SYNTH-{suffix}-0001"
    calibration_id = f"CAL-{environment_id}-001"
    packet_json = PACKETS / f"{phase}.json"
    packet_markdown = PACKETS / f"{phase}.md"
    packet_hashes = {
        packet_json.name: file_sha256(packet_json),
        packet_markdown.name: file_sha256(packet_markdown),
    }
    implementation_hashes = {
        "research/private/registry/registry.py": file_sha256(
            REGISTRY_SOURCE / "registry.py"
        ),
        "research/private/protocol-smoke/protocol_smoke.py": file_sha256(
            BASE / "protocol_smoke.py"
        ),
        "research/private/protocol-smoke/runner.py": file_sha256(
            BASE / "runner.py"
        ),
    }

    context.append(
        create_spec(
            context.clock,
            "source",
            source_id,
            "REGISTERED",
            {
                "title": f"{phase} known-source synthetic fixtures",
                "source_class": "SYNTHETIC",
                "artifact_hashes": fixture_hashes,
            },
        )
    )
    context.append(
        create_spec(
            context.clock,
            "build_manifest",
            build_id,
            "FROZEN",
            {
                "source_ids": [source_id],
                "artifact_hashes": packet_hashes,
            },
            detail_overrides={
                "launch_chain": [f"{PYTHON} -> {packet['packet_id']}"],
                "artifact_roles": {
                    "packet": "preregistered synthetic procedure",
                    "fixtures": "known-source inputs",
                },
                "install_version": "protocol-smoke-v1",
                "master_working_copy_status": "not applicable to synthetic fixture",
                "config_hashes": packet_hashes,
                "inner_executable": str(PYTHON),
                "modification": {"kind": "SYNTHETIC_ORIGINAL"},
            },
        )
    )
    python_hash = file_sha256(PYTHON)
    context.append(
        create_spec(
            context.clock,
            "tool",
            tool_id,
            "FROZEN",
            {
                "name": "CPython",
                "version": sys.version.split()[0],
                "origin": str(PYTHON),
                "artifact_hashes": {"python": python_hash},
            },
            detail_overrides={
                "revision": sys.version.split()[0],
                "dependency_hashes": {"python": python_hash},
                "licenses_notices": ["Python Software Foundation License"],
                "install_removal": {
                    "install": "pre-existing system interpreter",
                    "removal": "not modified by smoke suite",
                },
                "supported_format_disposition": {
                    "json": "standard-library json",
                    "python": "standard-library runtime",
                },
                "exposure": {"egress": "none", "execution": "local"},
                "synthetic_bakeoff": {
                    "fixture_count": len(packet["fixtures"]),
                    "result": "pinned before execution",
                },
                "qualification_status": "PINNED_FOR_SYNTHETIC_SMOKE",
            },
        )
    )
    os_hash = file_sha256(OS_RELEASE)
    context.append(
        create_spec(
            context.clock,
            "environment",
            environment_id,
            "FROZEN",
            {
                "tool_ids": [tool_id],
                "artifact_hashes": {"os-release": os_hash},
            },
            detail_overrides={
                "host": {"os_release_sha256": os_hash, "architecture": os.uname().machine},
                "display_audio": {"required": False, "reason": "CLI-only synthetic run"},
                "emulator": {"required": False},
                "game": {"present": False, "accessed": False},
                "capture": {"kind": "single canonical JSON stdout line"},
                "analyzer": {"kind": "Python AST and exact JSON comparison"},
                "multiplayer": {"required": False},
            },
        )
    )
    calibration = context.append(
        create_spec(
            context.clock,
            "calibration",
            calibration_id,
            "DRAFT",
            {
                "environment_id": environment_id,
                "metric": "Deterministic fixture self-check",
                "threshold": "exact JSON and exit-code match",
                "result": "pass",
                "artifact_hashes": packet_hashes,
            },
            detail_overrides={
                "condition": "Pinned known-source fixture and interpreter hashes",
                "warm_up": "No warm-up; one cold process per run",
                "threshold_kind": "EXACT",
                "result_disposition": "PASS",
                "baselines": {"packet_hashes": packet_hashes},
                "delivered_rates": {"sampling": "one deterministic result per run"},
                "instrumentation_delta": {"expected": 0, "units": "logical events"},
                "clock_uncertainty": {"applicable": False},
                "starvation_clock_checks": {"applicable": False},
            },
        )
    )
    context.append(
        transition_spec(
            context.clock,
            calibration,
            "PASSED",
            reason="All pinned synthetic calibration checks passed",
        )
    )
    campaign = context.append(
        create_spec(
            context.clock,
            "campaign",
            packet["campaign_id"],
            "DRAFT",
            {
                "bead_id": "mc-51m.5",
                "build_id": build_id,
                "environment_ids": [environment_id],
                "calibration_ids": [calibration_id],
                "question_scope": packet["outcome"],
                "denied_operations": packet["denied"],
            },
            detail_overrides={
                "subsystem": f"{phase} synthetic registry qualification",
                "insufficiency_rationale": "Synthetic smoke cannot establish any game fact",
                "source_risk_inputs": {"classification": "SYNTHETIC_ONLY"},
                "calibration_ids": [calibration_id],
                "approved_operations": ["local fixture execution", "private registry append"],
                "roles": {
                    "worker": "fresh packet executor",
                    "reviewer": "independent exact-target reviewer",
                },
                "implementer_isolation": "No owned-game or product implementation input",
                "exposure": {"egress": "none", "owned_game": "denied"},
                "outputs": {"registry": "private temporary", "summary": "sanitized"},
                "budgets": {"cpu": "one process per run", "retry": 0},
                "stop_escalation": ["hash mismatch", "unexpected output", "registry rejection"],
                "analyzer_project": {"present": False},
                "retention_policy": {"raw": "private or temporary only"},
                "closeout": {"requires": "root and terminal-state comparison"},
                "rerun_triggers": ["fixture hash change", "registry contract change"],
            },
        )
    )
    context.append(
        transition_spec(
            context.clock,
            campaign,
            "OPEN",
            reason="Preregistered packet and passed synthetic calibration are present",
        )
    )
    context.append(
        create_spec(
            context.clock,
            "question",
            packet["question_id"],
            "OPEN",
            {
                "campaign_id": packet["campaign_id"],
                "question": packet["outcome"],
                "falsifier": "Any prescribed exact result or registry state differs",
            },
            detail_overrides={
                "known_facts": "Only project-authored synthetic fixture behavior is known",
                "evidence_grade": "KNOWN_SOURCE_SYNTHETIC",
                "scope": {"phase": phase, "game_inference": False},
                "discriminating_prediction": packet["hypotheses"][0]["statement"],
                "hypothesis_plan": {
                    item["id"]: item["statement"] for item in packet["hypotheses"]
                },
                "answer_target": "Exact synthetic terminal state only",
            },
        )
    )
    context.append(
        create_spec(
            context.clock,
            "worker_packet",
            packet["packet_id"],
            "FROZEN",
            {
                "campaign_id": packet["campaign_id"],
                "allowed_operations": [
                    "read pinned packet and fixtures",
                    "execute prescribed local commands once",
                    "write caller-supplied private output",
                ],
                "denied_paths": packet["denied"],
            },
            detail_overrides={
                "question_experiment_ids": {
                    "question_id": packet["question_id"],
                    "experiment_id": packet["experiment_id"],
                },
                "outcome": packet["outcome"],
                "packet_identity": {
                    "json_sha256": packet_hashes[packet_json.name],
                    "markdown_sha256": packet_hashes[packet_markdown.name],
                },
                "cwd": str(BASE.parents[2]),
                "worker_reviewer_context": {
                    "worker": "fresh packet only",
                    "reviewer": "exact target with no raw inherited context",
                },
                "immutable_inputs": {
                    **packet_hashes,
                    **fixture_hashes,
                    **implementation_hashes,
                    "python": python_hash,
                    "os-release": os_hash,
                },
                "allowed_files_denied_services": {
                    "allowed": packet["allowed_reads"],
                    "denied": packet["denied"],
                },
                "ownership_lock": {"registry": "exclusive owner-only writer"},
                "hypothesis_plan": {
                    item["id"]: item["statement"] for item in packet["hypotheses"]
                },
                "procedure": {"runs": packet["runs"]},
                "measurement_plan": {"method": "exact JSON and exit status"},
                "output_contract": {"format": "sanitized deterministic JSON"},
                "budgets": {"retry": 0, "network": 0},
                "commands_runtime_bounds": {
                    "processes": len(packet["runs"]),
                    "timeout_seconds_each": 10,
                },
                "terminal_predicates": {"expected_lock": "exact match"},
                "cleanup_retry": {"cleanup": "caller controlled", "retry": False},
                "dependencies_downstream": {"bead": "mc-51m.5"},
                "return_manifest": {
                    "include": ["roots", "counts", "statuses", "negative errors"]
                },
            },
        )
    )
    return context


def add_hypotheses_and_experiment(
    context: PhaseContext,
    *,
    preferred_index: int = 0,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    source_id = f"SRC-SYNTH-{context.phase}-0001"
    hypotheses: list[dict[str, Any]] = []
    for item in context.packet["hypotheses"]:
        hypotheses.append(
            context.append(
                create_spec(
                    context.clock,
                    "hypothesis",
                    item["id"],
                    "OPEN",
                    {
                        "campaign_id": context.packet["campaign_id"],
                        "question_id": context.packet["question_id"],
                        "statement": item["statement"],
                        "alternative": "Competing preregistered synthetic law",
                        "falsifier": "A prescribed discriminating case disagrees",
                        "evidence_axes": {"O": 0, "R": 0, "T": 0, "A": 0, "C": 0},
                        "evidence_ids": [source_id],
                        "evidence_route_tags": ["ORIGINAL_DESIGN"],
                        "applicability": "Known-source synthetic fixture only",
                    },
                    detail_overrides={
                        "question_id": context.packet["question_id"],
                        "alternatives": {
                            "registered": [
                                hypothesis["id"]
                                for hypothesis in context.packet["hypotheses"]
                                if hypothesis["id"] != item["id"]
                            ]
                        },
                        "predictions": {
                            "packet": context.packet["packet_id"],
                            "statement": item["statement"],
                        },
                        "scope": {"synthetic_only": True, "phase": context.phase},
                        "evidence_route_ids": [source_id],
                        "contradiction_ids": ["NONE"],
                        "c1_resolution": "NONE",
                    },
                )
            )
        )
    experiment = context.append(
        create_spec(
            context.clock,
            "experiment",
            context.packet["experiment_id"],
            "DRAFT",
            {
                "campaign_id": context.packet["campaign_id"],
                "question_id": context.packet["question_id"],
                # The registry invalidates semantic consumers of a falsified
                # hypothesis. Bind the experiment to the preferred hypothesis;
                # the packet separately preregisters the alternative.
                "hypothesis_ids": [hypotheses[preferred_index]["entity_id"]],
                "reset_tier": context.packet["reset_tier"],
                "input_determinism": context.packet["input_determinism"],
                "stop_rule": context.packet["stopping_rule"],
            },
            detail_overrides={
                "primary_contrast": context.packet["outcome"],
                "variables": {"independent": "packet inputs", "dependent": "JSON output"},
                "setup_reset": {"tier": context.packet["reset_tier"]},
                "input_schedule": "Exact packet order",
                "controls": {"hashes": "verified before execution"},
                "exclusions": {"owned_game": True, "network": True},
                "run_order": {"order": [run["run_id"] for run in context.packet["runs"]]},
                "factorial_plan": {"kind": "bounded discriminating cases"},
                "sampling_plan": {"samples_per_case": 1, "deterministic": True},
                "independence_unit": "one cold process",
                "analysis_plan": {"comparison": "exact JSON and exit status"},
                "precision_target": "exact integer and Boolean equality",
                "units_tolerance": {"tolerance": 0},
                "decision_table": {"match": "pass", "mismatch": "failure"},
                "persistent_state": {"present": False},
                "calibration_ids": [f"CAL-ENV-SYNTH-{context.phase}-0001-001"],
            },
        )
    )
    experiment = context.append(
        transition_spec(
            context.clock,
            experiment,
            "PREREGISTERED",
            reason="Exact packet, fixtures, hypotheses, controls, and stopping rule frozen",
        )
    )
    return hypotheses, experiment


def execute_fixture(
    fixture_path: Path,
    arguments: list[str],
    *,
    expected_exit: int,
    expected_json: dict[str, Any],
) -> tuple[str, dict[str, Any], int]:
    command = [str(PYTHON), str(fixture_path), *arguments]
    completed = subprocess.run(
        command,
        check=False,
        capture_output=True,
        text=True,
        timeout=10,
        env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"},
    )
    stdout = completed.stdout.rstrip("\n")
    if completed.stderr:
        raise ScenarioError(f"unexpected stderr from {fixture_path.name}: {completed.stderr}")
    try:
        parsed = json.loads(stdout)
    except json.JSONDecodeError as error:
        raise ScenarioError(f"invalid JSON from {fixture_path.name}: {stdout!r}") from error
    if completed.returncode != expected_exit or parsed != expected_json:
        raise ScenarioError(
            f"{fixture_path.name} mismatch: exit={completed.returncode}, "
            f"json={parsed!r}"
        )
    return " ".join(command), parsed, completed.returncode


def add_runs(
    context: PhaseContext,
    experiment: dict[str, Any],
    fixture_name: str,
    *,
    defer_terminal_ids: set[str] | None = None,
) -> tuple[
    dict[str, dict[str, Any]],
    dict[str, dict[str, Any]],
    dict[str, dict[str, Any]],
    dict[str, dict[str, Any]],
]:
    defer_terminal_ids = defer_terminal_ids or set()
    fixture_path = FIXTURES / fixture_name
    runs: dict[str, dict[str, Any]] = {}
    observations: dict[str, dict[str, Any]] = {}
    measurements: dict[str, dict[str, Any]] = {}
    outputs: dict[str, dict[str, Any]] = {}

    for index, run in enumerate(context.packet["runs"], start=1):
        runs[run["run_id"]] = context.append(
            create_spec(
                context.clock,
                "run",
                run["run_id"],
                "PLANNED",
                {
                    "experiment_id": context.packet["experiment_id"],
                    "reset_tier": context.packet["reset_tier"],
                    "terminal_disposition": run["terminal_disposition"],
                },
                detail_overrides={
                    "build_environment_calibration": {
                        "build": f"BLD-SYNTH-{context.phase}-0001",
                        "environment": f"ENV-SYNTH-{context.phase}-0001",
                        "calibration": f"CAL-ENV-SYNTH-{context.phase}-0001-001",
                    },
                    "initial_state": {"stateless": True},
                    "input_events": {"arguments": run["arguments"]},
                    "geometry": {"applicable": False},
                    "run_order": {"index": index},
                    "rng_independence": {"rng": "unused"},
                    "output_capture_ids": {
                        "observation_id": f"OBS-{run['run_id']}-01",
                        "measurement_id": f"MEAS-{run['run_id']}-01",
                    },
                    "notes": "Known-source synthetic one-process run",
                    "setup_modifications": {"none": True},
                    "deviation_ids": [run.get("deviation_id", "NONE")],
                    "actual_procedure": (
                        f"{PYTHON} {fixture_path} {' '.join(run['arguments'])}"
                    ),
                    "validity_reason": (
                        "deliberate invalid-confirmatory control"
                        if run["terminal_disposition"] == "INVALID"
                        else "matches preregistered deterministic procedure"
                    ),
                    "clock_network_context": {
                        "network": "none",
                        "clock": "not used for fixture logic",
                    },
                    "exposure_retention": {
                        "exposure": "synthetic only",
                        "retention": "private temporary registry",
                    },
                },
            )
        )

    experiment = context.append(
        transition_spec(
            context.clock,
            experiment,
            "RUNNING",
            reason="All planned run records exist and fixture hashes match",
        )
    )

    for run in context.packet["runs"]:
        run_id = run["run_id"]
        command, parsed, exit_status = execute_fixture(
            fixture_path,
            run["arguments"],
            expected_exit=run["expected_exit"],
            expected_json=run["expected_json"],
        )
        outputs[run_id] = {
            "command": command,
            "exit_status": exit_status,
            "json": parsed,
        }
        observation_id = f"OBS-{run_id}-01"
        measurement_id = f"MEAS-{run_id}-01"
        observations[run_id] = context.append(
            create_spec(
                context.clock,
                "observation",
                observation_id,
                "RECORDED",
                {
                    "run_id": run_id,
                    "observed_fact": json.dumps(parsed, sort_keys=True),
                },
                detail_overrides={
                    "capture_ids": ["STDOUT-CANONICAL-JSON"],
                    "boundary_condition": (
                        "preregistered rejection"
                        if not parsed.get("accepted", False)
                        else "preregistered accepted input"
                    ),
                    "scope": {"fixture": fixture_name, "synthetic_only": True},
                    "timing_sampling_basis": "single deterministic process",
                    "observability_condition": "one complete JSON stdout line",
                },
            )
        )
        measurements[run_id] = context.append(
            create_spec(
                context.clock,
                "measurement",
                measurement_id,
                "RECORDED",
                {
                    "run_id": run_id,
                    "method": "Exact JSON parse and process exit status",
                    "units": "canonical JSON fields and integer exit status",
                    "uncertainty": 0,
                },
                detail_overrides={
                    "value_or_distribution": {
                        "exit_status": exit_status,
                        "value": parsed,
                    },
                    "capture_ids": [observation_id],
                    "sample_count": {"count": 1},
                    "independence_unit": "one cold process",
                    "extraction_identity": {
                        "parser": "Python standard-library json"
                    },
                    "precision_quantization_bounds": {"exact": True},
                    "timing_statistics": {"applicable": False},
                },
            )
        )
        if run_id not in defer_terminal_ids:
            runs[run_id] = context.append(
                transition_spec(
                    context.clock,
                    runs[run_id],
                    "VALID",
                    reason="Observed exit status and JSON matched preregistration exactly",
                )
            )
    return runs, observations, measurements, outputs


def amend_hypothesis_evidence(
    context: PhaseContext,
    hypothesis: dict[str, Any],
    evidence_ids: list[str],
    *,
    axes: dict[str, int],
    route_tags: list[str],
) -> dict[str, Any]:
    payload = dict(context.payloads()[hypothesis["entity_id"]])
    payload["evidence_ids"] = evidence_ids
    payload["evidence_axes"] = axes
    payload["evidence_route_tags"] = route_tags
    detail = detail_for(
        "hypothesis",
        hypothesis["entity_id"],
        payload,
        overrides={
            **payload["detail"]["manifest"],
            "question_id": payload["question_id"],
            "evidence_route_ids": evidence_ids,
        },
    )
    return context.append(
        amend_spec(
            context.clock,
            hypothesis,
            {
                "evidence_ids": evidence_ids,
                "evidence_axes": axes,
                "evidence_route_tags": route_tags,
                "detail": detail,
            },
            reason="Bind exact prescribed synthetic observations and measurements",
        )
    )


def accept_finding(
    context: PhaseContext,
    finding_id: str,
    *,
    evidence_ids: list[str],
    functional_rule: str,
    evidence_floor: str = "SIMPLE_BOUNDED",
    axes: dict[str, int] | None = None,
    route_tags: list[str] | None = None,
    rule_domains: list[str] | None = None,
    contradiction_resolution: str = "NONE",
    contradiction_ids: list[str] | None = None,
    classification: str = "SYNTHETIC_ONLY",
    reviewer: str = "reviewer-b",
    review_suffix: str = "01",
) -> tuple[dict[str, Any], dict[str, Any]]:
    axes = axes or {"O": 2, "R": 2, "T": 2, "A": 1, "C": 0}
    route_tags = route_tags or [
        "CONTROLLED_OBSERVATION",
        "SYNTHETIC_CONFORMANCE",
    ]
    rule_domains = rule_domains or ["DISCRETE"]
    payload = {
        "campaign_id": context.packet["campaign_id"],
        "question_id": context.packet["question_id"],
        "functional_rule": functional_rule,
        "fidelity_classification": "ORIGINAL_ADDITION",
        "evidence_floor": evidence_floor,
        "evidence_axes": axes,
        "evidence_route_tags": route_tags,
        "evidence_ids": evidence_ids,
        "rule_domains": rule_domains,
        "contradiction_resolution": contradiction_resolution,
        "applicability": "Known-source synthetic fixture only",
    }
    finding = context.append(
        create_spec(
            context.clock,
            "finding",
            finding_id,
            "DRAFT",
            payload,
            classification=classification,
            detail_overrides={
                "question_id": context.packet["question_id"],
                "scope": {"phase": context.phase, "synthetic_only": True},
                "functional_specification": {"rule": functional_rule},
                "unknowns_internal_freedom": {"none_required": True},
                "uncertainty": "Exact for the bounded synthetic cases only",
                "evidence_ids": evidence_ids,
                "contradiction_ids": contradiction_ids or ["NONE"],
                "reproduction": {"packet": context.packet["packet_id"]},
                "traffic_classification": {
                    "classification": classification,
                    "transferable": classification in registry.PUBLIC_TRANSFER_CLASSIFICATIONS,
                },
            },
        )
    )
    finding = context.append(
        transition_spec(
            context.clock,
            finding,
            "CORROBORATED",
            reason="Prescribed independent synthetic routes agree",
        )
    )
    finding = context.append(
        transition_spec(
            context.clock,
            finding,
            "REVIEWED",
            author=reviewer,
            reason="Non-author reviewer completed exact wording and evidence review",
        )
    )
    review_id = f"REV-{finding_id}-{review_suffix}"
    review = context.append(
        create_spec(
            context.clock,
            "review",
            review_id,
            "RECORDED",
            review_payload(finding),
            author=reviewer,
            classification=classification,
            detail_overrides={
                "target_type": "finding",
                "target_record_hash": finding["event_hash"],
                "reviewed_aspects": {
                    "wording": True,
                    "classification": True,
                    "evidence": True,
                    "uncertainty": True,
                    "verification": True,
                },
                "reasons": "Exact target is acceptable within synthetic scope",
            },
        )
    )
    finding = context.append(
        transition_spec(
            context.clock,
            finding,
            "ACCEPTED",
            author=reviewer,
            reason="Exact independent review approves bounded synthetic finding",
        )
    )
    return finding, review


def accept_vector(
    context: PhaseContext,
    vector_id: str,
    finding_ids: list[str],
    *,
    traffic_light: str = "GREEN",
    reviewer: str = "reviewer-b",
    review_suffix: str = "01",
) -> tuple[dict[str, Any], dict[str, Any]]:
    vector = context.append(
        create_spec(
            context.clock,
            "vector",
            vector_id,
            "DRAFT",
            {
                "campaign_id": context.packet["campaign_id"],
                "finding_ids": finding_ids,
                "traffic_light": traffic_light,
                "cases": conformance_cases(),
            },
            detail_overrides={
                "finding_ids": finding_ids,
                "branch_coverage": "positive,boundary,rejection,invariant",
                "synthesis_provenance": "Project-authored from accepted synthetic finding",
                "no_original_payload_declaration": "ATTESTED_NO_ORIGINAL_PAYLOAD",
                "case_schema": {
                    "required": ["kind", "inputs", "expected", "units", "tolerance"]
                },
            },
        )
    )
    vector = context.append(
        transition_spec(
            context.clock,
            vector,
            "REVIEWED",
            author=reviewer,
            reason="Non-author reviewer checked all four conformance case classes",
        )
    )
    review = context.append(
        create_spec(
            context.clock,
            "review",
            f"REV-{vector_id}-{review_suffix}",
            "RECORDED",
            review_payload(vector),
            author=reviewer,
            detail_overrides={
                "target_type": "vector",
                "target_record_hash": vector["event_hash"],
                "reviewed_aspects": {
                    "positive": True,
                    "boundary": True,
                    "rejection": True,
                    "invariant": True,
                },
                "reasons": "Vector is synthetic, complete, and contains no original payload",
            },
        )
    )
    vector = context.append(
        transition_spec(
            context.clock,
            vector,
            "ACCEPTED",
            author=reviewer,
            reason="Exact independent review approves Green synthetic vector",
        )
    )
    return vector, review


def finish_phase(
    context: PhaseContext,
    experiment: dict[str, Any],
) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    experiment = context.append(
        transition_spec(
            context.clock,
            experiment,
            "COMPLETE",
            reason="All prescribed runs and negative gates reached expected states",
        )
    )
    question = context.states()[context.packet["question_id"]]
    question = context.append(
        transition_spec(
            context.clock,
            question,
            "ANSWERED",
            reason="Bounded synthetic packet outcome determined",
        )
    )
    campaign = context.states()[context.packet["campaign_id"]]
    campaign = context.append(
        transition_spec(
            context.clock,
            campaign,
            "CLOSED",
            reason="Synthetic phase complete with deterministic terminal state",
        )
    )
    return experiment, question, campaign


def phase_summary(context: PhaseContext) -> dict[str, Any]:
    events = context.events()
    registry.validate_events(events)
    statuses = {
        entity_id: state["status"]
        for entity_id, state in sorted(registry.entity_state(events).items())
    }
    return {
        "phase": context.phase,
        "packet_id": context.packet["packet_id"],
        "packet_sha256": file_sha256(PACKETS / f"{context.phase}.json"),
        "event_count": len(events),
        "entity_count": len(statuses),
        "root_hash": events[-1]["event_hash"],
        "statuses": statuses,
        "negative_cases": context.negative_cases,
        "exposure": {
            "classification": "SYNTHETIC_ONLY",
            "network": "none",
            "remote_models": "none",
            "owned_mc2_paths_or_bytes": "none",
            "raw_registry_location": "caller-supplied private temporary output only",
        },
    }


def run_p1(output_root: Path) -> dict[str, Any]:
    context = start_phase(output_root, "P1")
    hypotheses, experiment = add_hypotheses_and_experiment(context)
    deferred = {"RUN-EXP-CAM-SYNTH-P1-0001-001-004"}
    runs, observations, measurements, _ = add_runs(
        context,
        experiment,
        "discrete_oracle.py",
        defer_terminal_ids=deferred,
    )
    invalid_id = next(iter(deferred))
    context.expect_failure(
        "P1-NEG-TERMINAL-MISMATCH",
        lambda: context.append(
            transition_spec(
                context.clock,
                runs[invalid_id],
                "VALID",
                reason="Deliberately incorrect terminal candidate",
            )
        ),
        "run status and terminal_disposition differ",
    )
    context.expect_failure(
        "P1-NEG-INCOMPLETE-INVALIDATION",
        lambda: context.append(
            transition_spec(
                context.clock,
                runs[invalid_id],
                "INVALID",
                reason="Deliberate packet input deviation",
            )
        ),
        "blocked/revoked/retested",
    )
    pdev_id = "PDEV-EXP-CAM-SYNTH-P1-0001-001-01"
    revoked_observation, revoked_measurement, invalid_run, _ = context.append_batch(
        [
            transition_spec(
                context.clock,
                observations[invalid_id],
                "REVOKED",
                reason="Observation preserved but excluded after protocol deviation",
            ),
            transition_spec(
                context.clock,
                measurements[invalid_id],
                "REVOKED",
                reason="Measurement preserved but excluded after protocol deviation",
            ),
            transition_spec(
                context.clock,
                runs[invalid_id],
                "INVALID",
                reason="Input differed from the confirmatory packet",
            ),
            create_spec(
                context.clock,
                "protocol_deviation",
                pdev_id,
                "RECORDED",
                {
                    "experiment_id": context.packet["experiment_id"],
                    "deviation": "Used current=96 in place of the wrapping contrast current=97",
                    "validity_disposition": "INVALID_CONFIRMATORY",
                },
                detail_overrides={
                    "run_ids": [invalid_id],
                    "planned_vs_actual": {
                        "planned_current": 97,
                        "actual_current": 96,
                    },
                    "affected_step": "P1 run 004 deliberate deviation control",
                    "occurrence_time": "during prescribed synthetic execution",
                    "downstream_effect": {
                        "hypothesis_support": "excluded",
                        "observation": f"OBS-{invalid_id}-01",
                    },
                    "review_context": {"disposition": "INVALID_CONFIRMATORY"},
                },
            ),
        ]
    )
    runs[invalid_id] = invalid_run
    evidence_ids = [
        observations[run_id]["entity_id"]
        for run_id in sorted(runs)
        if run_id != invalid_id
    ] + [
        measurements[run_id]["entity_id"]
        for run_id in sorted(runs)
        if run_id != invalid_id
    ]
    preferred = amend_hypothesis_evidence(
        context,
        hypotheses[0],
        evidence_ids,
        axes={"O": 3, "R": 3, "T": 2, "A": 1, "C": 0},
        route_tags=["CONTROLLED_OBSERVATION", "SYNTHETIC_CONFORMANCE"],
    )
    context.append(
        transition_spec(
            context.clock,
            preferred,
            "SUPPORTED",
            reason="All valid prescribed cases match modulo 100",
        )
    )
    alternative = amend_hypothesis_evidence(
        context,
        hypotheses[1],
        evidence_ids,
        axes={"O": 3, "R": 3, "T": 2, "A": 1, "C": 0},
        route_tags=["CONTROLLED_OBSERVATION", "SYNTHETIC_CONFORMANCE"],
    )
    context.append(
        transition_spec(
            context.clock,
            alternative,
            "FALSIFIED",
            reason="The 97+5 case yields 2 rather than saturation at 99",
        )
    )
    finish_phase(context, context.states()[experiment["entity_id"]])
    return phase_summary(context)


def run_p2(output_root: Path) -> dict[str, Any]:
    context = start_phase(output_root, "P2")
    hypotheses, experiment = add_hypotheses_and_experiment(context)
    runs, observations, measurements, _ = add_runs(
        context, experiment, "timing_oracle.py"
    )
    evidence_ids = [
        observations[run_id]["entity_id"] for run_id in sorted(runs)
    ] + [measurements[run_id]["entity_id"] for run_id in sorted(runs)]
    preferred = amend_hypothesis_evidence(
        context,
        hypotheses[0],
        evidence_ids,
        axes={"O": 3, "R": 3, "T": 2, "A": 1, "C": 0},
        route_tags=["CONTROLLED_OBSERVATION", "SYNTHETIC_CONFORMANCE"],
    )
    context.append(
        transition_spec(
            context.clock,
            preferred,
            "SUPPORTED",
            reason="55 ms and partitioned schedules match a 20 ms accumulator",
        )
    )
    alternative = amend_hypothesis_evidence(
        context,
        hypotheses[1],
        evidence_ids,
        axes={"O": 3, "R": 3, "T": 2, "A": 1, "C": 0},
        route_tags=["CONTROLLED_OBSERVATION", "SYNTHETIC_CONFORMANCE"],
    )
    context.append(
        transition_spec(
            context.clock,
            alternative,
            "FALSIFIED",
            reason="One 55 ms callback produces two ticks and three callbacks produce two",
        )
    )
    bad_finding_id = "FND-CAM-SYNTH-P2-0001-002"
    context.expect_failure(
        "P2-NEG-SIMPLE-FLOOR",
        lambda: context.append(
            create_spec(
                context.clock,
                "finding",
                bad_finding_id,
                "DRAFT",
                {
                    "campaign_id": context.packet["campaign_id"],
                    "question_id": context.packet["question_id"],
                    "functional_rule": "Fixed 20 ms accumulator",
                    "fidelity_classification": "ORIGINAL_ADDITION",
                    "evidence_floor": "SIMPLE_BOUNDED",
                    "evidence_axes": {"O": 3, "R": 3, "T": 2, "A": 1, "C": 0},
                    "evidence_route_tags": [
                        "CONTROLLED_OBSERVATION",
                        "SYNTHETIC_CONFORMANCE",
                    ],
                    "evidence_ids": evidence_ids,
                    "rule_domains": ["TIMING", "DETERMINISM"],
                    "contradiction_resolution": "NONE",
                    "applicability": "Known-source synthetic fixture only",
                },
            )
        ),
        "CRITICAL evidence floor",
    )
    accept_finding(
        context,
        "FND-CAM-SYNTH-P2-0001-001",
        evidence_ids=evidence_ids,
        functional_rule="Accumulate accepted elapsed milliseconds and emit floor(total/20) ticks",
        evidence_floor="CRITICAL",
        axes={"O": 3, "R": 3, "T": 2, "A": 1, "C": 0},
        rule_domains=["TIMING", "DETERMINISM"],
    )
    finish_phase(context, context.states()[experiment["entity_id"]])
    return phase_summary(context)


def static_correlation() -> dict[str, dict[str, Any]]:
    source_path = FIXTURES / "correlation_oracle.py"
    table_path = FIXTURES / "correlation_table.json"
    tree = ast.parse(source_path.read_text(encoding="utf-8"), filename=str(source_path))
    extracted: dict[str, dict[str, Any]] = {}
    for node in tree.body:
        if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            continue
        marker: str | None = None
        for decorator in node.decorator_list:
            if (
                isinstance(decorator, ast.Call)
                and isinstance(decorator.func, ast.Name)
                and decorator.func.id == "synthetic_marker"
                and len(decorator.args) == 1
                and isinstance(decorator.args[0], ast.Constant)
                and isinstance(decorator.args[0].value, str)
            ):
                marker = decorator.args[0].value
        if marker is None:
            continue
        returned = next(
            (
                statement.value
                for statement in node.body
                if isinstance(statement, ast.Return)
            ),
            None,
        )
        if returned is None:
            raise ScenarioError(f"{node.name} has no static return")
        value = ast.literal_eval(returned)
        extracted[marker] = {"function": node.name, **value}
    with table_path.open("r", encoding="utf-8") as handle:
        table = json.load(handle)
    if extracted != table:
        raise ScenarioError(
            f"static AST/table mismatch: extracted={extracted!r}, table={table!r}"
        )
    return extracted


def run_p3(output_root: Path) -> dict[str, Any]:
    context = start_phase(output_root, "P3")
    context.expect_failure(
        "P3-NEG-HASH-MISMATCH",
        lambda: require_file_hash(
            FIXTURES / "correlation_table.json",
            "0" * 64,
        ),
        "hash mismatch",
    )
    static_map = static_correlation()
    hypotheses, experiment = add_hypotheses_and_experiment(context)
    runs, observations, measurements, outputs = add_runs(
        context, experiment, "correlation_oracle.py"
    )
    for marker in ("ALPHA", "OMEGA"):
        run_id = next(
            run["run_id"]
            for run in context.packet["runs"]
            if run["arguments"] == ["--marker", marker]
        )
        actual = outputs[run_id]["json"]
        expected = static_map[marker]
        if (
            actual["runtime_tag"] != expected["runtime_tag"]
            or actual["value"] != expected["value"]
        ):
            raise ScenarioError(f"runtime correlation mismatch for {marker}")

    wrong_id = "OBS-RUN-EXP-CAM-SYNTH-P3-0001-001-999-02"
    real_run_id = "RUN-EXP-CAM-SYNTH-P3-0001-001-001"
    context.expect_failure(
        "P3-NEG-PARENT-MISMATCH",
        lambda: context.append(
            create_spec(
                context.clock,
                "observation",
                wrong_id,
                "RECORDED",
                {
                    "run_id": real_run_id,
                    "observed_fact": "Deliberately parent-mismatched observation",
                },
            )
        ),
        "does not embed its declared run_id",
    )
    captures: list[str] = []
    for index, run_id in enumerate(sorted(runs)[:2], start=1):
        capture_id = f"CAP-{run_id}-01"
        capture = context.append(
            create_spec(
                context.clock,
                "capture",
                capture_id,
                "FROZEN",
                {
                    "run_id": run_id,
                    "artifact_hashes": {
                        "correlation_oracle.py": file_sha256(
                            FIXTURES / "correlation_oracle.py"
                        ),
                        "correlation_table.json": file_sha256(
                            FIXTURES / "correlation_table.json"
                        ),
                    },
                },
                detail_overrides={
                    "artifact_manifest": {"marker": list(static_map)[index - 1]},
                    "artifact_roles": {
                        "python": "static marker/function source",
                        "json": "expected table",
                    },
                    "tool_configuration": {"parser": "Python ast"},
                    "rates_codecs_timestamps": {"codec": "UTF-8/JSON"},
                    "clock_mapping": {"applicable": False},
                    "measured_overhead": {"applicable": False},
                    "completeness": "Complete bounded synthetic fixture",
                    "logical_reference": run_id,
                    "retention_disposition": {
                        "state": "private temporary",
                        "reason": "protocol smoke",
                    },
                },
            )
        )
        captures.append(capture["entity_id"])
    evidence_ids = (
        [f"SRC-SYNTH-P3-0001", *captures]
        + [observations[run_id]["entity_id"] for run_id in sorted(runs)]
        + [measurements[run_id]["entity_id"] for run_id in sorted(runs)]
    )
    preferred = amend_hypothesis_evidence(
        context,
        hypotheses[0],
        evidence_ids,
        axes={"O": 3, "R": 3, "T": 3, "A": 2, "C": 0},
        route_tags=[
            "STATIC_ANALYSIS",
            "DYNAMIC_CORRELATION",
            "SYNTHETIC_CONFORMANCE",
        ],
    )
    context.append(
        transition_spec(
            context.clock,
            preferred,
            "SUPPORTED",
            reason="AST marker, table entry, and runtime tag/value agree",
        )
    )
    alternative = amend_hypothesis_evidence(
        context,
        hypotheses[1],
        evidence_ids,
        axes={"O": 3, "R": 3, "T": 3, "A": 2, "C": 0},
        route_tags=[
            "STATIC_ANALYSIS",
            "DYNAMIC_CORRELATION",
            "SYNTHETIC_CONFORMANCE",
        ],
    )
    context.append(
        transition_spec(
            context.clock,
            alternative,
            "FALSIFIED",
            reason="Runtime tags are LIFT/DROP rather than function names",
        )
    )
    mechanism_real = context.append(
        create_spec(
            context.clock,
            "mechanism",
            "MEC-CAM-SYNTH-P3-0001-001",
            "DRAFT",
            {
                "campaign_id": context.packet["campaign_id"],
                "statement": "Decorator marker selects a function whose return tag/value matches the static table",
                "dataflow_chain": [
                    "decorator marker",
                    "handler function",
                    "runtime JSON tag/value",
                ],
            },
            detail_overrides={
                "question_hypothesis_finding_ids": {
                    "question_id": context.packet["question_id"],
                    "hypothesis_id": hypotheses[0]["entity_id"],
                },
                "build_tool_project_config": {
                    "fixture": "correlation_oracle.py",
                    "table": "correlation_table.json",
                },
                "evidence_route_ids": evidence_ids,
                "loader_memory_map_qualification": {"applicable": False},
                "competing_mechanisms": {
                    "false": "function name is emitted tag"
                },
                "scope": {"synthetic_only": True},
                "runtime_probes": {"markers": ["ALPHA", "OMEGA"]},
                "ambiguity_stop_disposition": "No ambiguity in exact bounded cases",
            },
        )
    )
    context.append(
        transition_spec(
            context.clock,
            mechanism_real,
            "SUPPORTED",
            reason="Static and dynamic routes correlate exactly",
        )
    )
    mechanism_false = context.append(
        create_spec(
            context.clock,
            "mechanism",
            "MEC-CAM-SYNTH-P3-0001-002",
            "DRAFT",
            {
                "campaign_id": context.packet["campaign_id"],
                "statement": "Function name is emitted directly as runtime tag",
                "dataflow_chain": ["function name", "runtime JSON tag"],
            },
            detail_overrides={
                "question_hypothesis_finding_ids": {
                    "question_id": context.packet["question_id"],
                    "hypothesis_id": hypotheses[1]["entity_id"],
                },
                "build_tool_project_config": {
                    "fixture": "correlation_oracle.py"
                },
                "evidence_route_ids": evidence_ids,
                "loader_memory_map_qualification": {"applicable": False},
                "competing_mechanisms": {"preferred": mechanism_real["entity_id"]},
                "scope": {"synthetic_only": True},
                "runtime_probes": {"markers": ["ALPHA", "OMEGA"]},
                "ambiguity_stop_disposition": "Rejected by exact runtime tags",
            },
        )
    )
    context.append(
        transition_spec(
            context.clock,
            mechanism_false,
            "REJECTED",
            reason="Observed tags do not equal alpha_handler or omega_handler",
        )
    )
    finish_phase(context, context.states()[experiment["entity_id"]])
    return phase_summary(context)


def run_p4(output_root: Path) -> dict[str, Any]:
    context = start_phase(output_root, "P4")
    hypotheses, experiment = add_hypotheses_and_experiment(
        context, preferred_index=1
    )
    runs, observations, measurements, _ = add_runs(
        context, experiment, "lifecycle_oracle.py"
    )
    run_id = next(iter(runs))
    evidence_ids = [
        observations[run_id]["entity_id"],
        measurements[run_id]["entity_id"],
    ]
    old_finding, _ = accept_finding(
        context,
        "FND-CAM-SYNTH-P4-0001-001",
        evidence_ids=evidence_ids,
        functional_rule="Revision 1 uses a synthetic step of one",
    )
    old_vector, _ = accept_vector(
        context,
        "VEC-CAM-SYNTH-P4-0001-001",
        [old_finding["entity_id"]],
    )
    context.expect_failure(
        "P4-NEG-NONATOMIC-SUPERSESSION",
        lambda: context.append(
            transition_spec(
                context.clock,
                old_finding,
                "SUPERSEDED",
                reason="Deliberately omitted dependent vector invalidation",
            )
        ),
        "blocked/revoked/retested",
    )
    contradiction_id = "CTR-CAM-SYNTH-P4-0001-001"
    contradiction_spec = create_spec(
        context.clock,
        "contradiction",
        contradiction_id,
        "OPEN_BLOCKING",
        {
            "campaign_id": context.packet["campaign_id"],
            "affected_ids": [old_finding["entity_id"]],
            "disposition": "PENDING",
        },
        detail_overrides={
            "conflicting_evidence_ids": evidence_ids,
            "conflicting_statements": {
                "old": "revision 1 step is current",
                "new": "revision 2 step is current in replacement scope",
            },
            "scope_dimensions": {"revision": [1, 2]},
            "resolution": {"status": "pending"},
            "review_context": {"required": "exact independent review"},
            "evidence_axis_change": {"C": "0 to 1"},
            "invalidation_targets": [old_finding["entity_id"]],
        },
    )
    context.expect_failure(
        "P4-NEG-OPEN-CONTRADICTION",
        lambda: context.append(contradiction_spec),
        "blocked/revoked/retested",
    )
    contradiction_payload = dict(contradiction_spec["payload"])
    contradiction_payload["disposition"] = "HYPOTHESIS_FALSIFIED"
    contradiction_payload["detail"] = detail_for(
        "contradiction",
        contradiction_id,
        contradiction_payload,
        overrides={
            **contradiction_payload["detail"]["manifest"],
            "resolution": {
                "status": "HYPOTHESIS_FALSIFIED",
                "reason": "Revision-1 hypothesis is false in replacement scope",
            },
        },
    )
    (
        _contradiction_created,
        _contradiction_amended,
        contradiction,
        old_vector,
        old_finding,
    ) = context.append_batch(
        [
            contradiction_spec,
            amend_spec(
                context.clock,
                contradiction_spec,
                {
                    "disposition": "HYPOTHESIS_FALSIFIED",
                    "detail": contradiction_payload["detail"],
                },
                reason="Bind the exact scoped resolution before terminal transition",
            ),
            transition_spec(
                context.clock,
                contradiction_spec,
                "HYPOTHESIS_FALSIFIED",
                reason="The legacy hypothesis is false in the replacement scope",
            ),
            transition_spec(
                context.clock,
                old_vector,
                "RETEST_REQUIRED",
                reason="Source finding is blocked by resolved contradiction",
            ),
            transition_spec(
                context.clock,
                old_finding,
                "CONTRADICTED",
                reason="Revision-2 observation conflicts with revision-1 scope",
            ),
        ]
    )
    replacement_id = "FND-CAM-SYNTH-P4-0001-002"
    replacement_payload = {
        "campaign_id": context.packet["campaign_id"],
        "question_id": context.packet["question_id"],
        "functional_rule": "Revision 2 uses a synthetic step of two in replacement scope",
        "fidelity_classification": "ORIGINAL_ADDITION",
        "evidence_floor": "SIMPLE_BOUNDED",
        "evidence_axes": {"O": 2, "R": 2, "T": 2, "A": 1, "C": 1},
        "evidence_route_tags": [
            "CONTROLLED_OBSERVATION",
            "SYNTHETIC_CONFORMANCE",
        ],
        "evidence_ids": evidence_ids,
        "rule_domains": ["DISCRETE", "VERSION_MODE"],
        "contradiction_resolution": "Legacy hypothesis falsified in replacement scope",
        "applicability": "Known-source synthetic replacement scope only",
    }
    replacement = context.append(
        create_spec(
            context.clock,
            "finding",
            replacement_id,
            "DRAFT",
            replacement_payload,
            detail_overrides={
                "question_id": context.packet["question_id"],
                "scope": {"revision": 2, "synthetic_only": True},
                "functional_specification": {"step": 2},
                "unknowns_internal_freedom": {"none_required": True},
                "uncertainty": "C1 resolved by exact scoped contradiction record",
                "evidence_ids": evidence_ids,
                "contradiction_ids": [contradiction_id],
                "reproduction": {"packet": context.packet["packet_id"]},
                "traffic_classification": {"classification": "SYNTHETIC_ONLY"},
            },
        )
    )
    replacement = context.append(
        transition_spec(
            context.clock,
            replacement,
            "CORROBORATED",
            reason="Revision-2 fixture and exact measurement agree",
        )
    )
    replacement = context.append(
        transition_spec(
            context.clock,
            replacement,
            "REVIEWED",
            author="reviewer-c",
            reason="Fresh reviewer checked scoped replacement finding",
        )
    )
    context.append(
        create_spec(
            context.clock,
            "review",
            f"REV-{replacement_id}-01",
            "RECORDED",
            review_payload(replacement),
            author="reviewer-c",
            detail_overrides={
                "target_type": "finding",
                "target_record_hash": replacement["event_hash"],
                "reviewed_aspects": {
                    "scope": True,
                    "evidence": True,
                    "uncertainty": True,
                    "c1_resolution": True,
                },
                "reasons": "Replacement finding is exact and synthetic",
            },
        )
    )
    context.expect_failure(
        "P4-NEG-C1-WITHOUT-CONTRADICTION-REVIEW",
        lambda: context.append(
            transition_spec(
                context.clock,
                replacement,
                "ACCEPTED",
                author="reviewer-c",
                reason="Deliberately premature C1 acceptance",
            )
        ),
        "lacks an exact independent review",
    )
    context.append(
        create_spec(
            context.clock,
            "review",
            f"REV-{contradiction_id}-01",
            "RECORDED",
            review_payload(contradiction),
            author="reviewer-b",
            detail_overrides={
                "target_type": "contradiction",
                "target_record_hash": contradiction["event_hash"],
                "reviewed_aspects": {
                    "scope": True,
                    "conflicting_evidence": True,
                    "resolution": True,
                    "invalidation": True,
                },
                "reasons": "Resolved contradiction is exact and scope-bounded",
            },
        )
    )
    replacement = context.append(
        transition_spec(
            context.clock,
            replacement,
            "ACCEPTED",
            author="reviewer-c",
            reason="Finding and resolved contradiction both have exact independent review",
        )
    )
    replacement = context.append(
        link_spec(
            context.clock,
            replacement,
            old_finding["entity_id"],
            "supersedes",
        )
    )
    context.append(
        transition_spec(
            context.clock,
            old_finding,
            "SUPERSEDED",
            reason="Accepted revision-2 finding supersedes contradicted revision-1 finding",
        )
    )
    preferred = amend_hypothesis_evidence(
        context,
        hypotheses[1],
        evidence_ids,
        axes={"O": 2, "R": 2, "T": 2, "A": 1, "C": 1},
        route_tags=["CONTROLLED_OBSERVATION", "SYNTHETIC_CONFORMANCE"],
    )
    context.append(
        transition_spec(
            context.clock,
            preferred,
            "SUPPORTED",
            reason="Revision-2 replacement scope matches exact observation",
        )
    )
    legacy = amend_hypothesis_evidence(
        context,
        hypotheses[0],
        evidence_ids,
        axes={"O": 2, "R": 2, "T": 2, "A": 1, "C": 1},
        route_tags=["CONTROLLED_OBSERVATION", "SYNTHETIC_CONFORMANCE"],
    )
    context.append(
        transition_spec(
            context.clock,
            legacy,
            "FALSIFIED",
            reason="Revision-1 rule is false in replacement scope",
        )
    )
    finish_phase(context, context.states()[experiment["entity_id"]])
    return phase_summary(context)


def _receipt_detail(
    context: PhaseContext,
    receipt_id: str,
    transfer: dict[str, Any],
    implementation_hash: str,
) -> dict[str, Any]:
    payloads = context.payloads()
    states = context.states()
    transfer_payload = payloads[transfer["entity_id"]]
    payload = {
        "transfer_id": transfer["entity_id"],
        "transfer_event_hash": transfer["event_hash"],
        "destination_bead_id": transfer_payload["destination_bead_id"],
    }
    return detail_for(
        "receipt",
        receipt_id,
        payload,
        overrides={
            "vector_event_hashes": [
                states[vector_id]["event_hash"]
                for vector_id in transfer_payload["vector_ids"]
            ],
            "destination_files": transfer_payload["detail"]["manifest"][
                "destination_files"
            ],
            "implementation_artifact_hashes": {
                "transfer_oracle.py": implementation_hash
            },
            "worker_context": {
                "source": "fresh synthetic packet",
                "raw_context_inherited": False,
            },
            "stale_notification": {"required_on_revocation": True},
        },
    )


def run_p5(output_root: Path) -> dict[str, Any]:
    context = start_phase(output_root, "P5")
    hypotheses, experiment = add_hypotheses_and_experiment(context)
    runs, observations, measurements, _ = add_runs(
        context, experiment, "transfer_oracle.py"
    )
    clean_run = "RUN-EXP-CAM-SYNTH-P5-0001-001-001"
    contaminated_run = "RUN-EXP-CAM-SYNTH-P5-0001-001-002"
    clean_evidence = [
        observations[clean_run]["entity_id"],
        measurements[clean_run]["entity_id"],
    ]
    all_evidence = clean_evidence + [
        observations[contaminated_run]["entity_id"],
        measurements[contaminated_run]["entity_id"],
    ]
    clean_hypothesis = amend_hypothesis_evidence(
        context,
        hypotheses[0],
        clean_evidence,
        axes={"O": 3, "R": 3, "T": 2, "A": 1, "C": 0},
        route_tags=["CONTROLLED_OBSERVATION", "SYNTHETIC_CONFORMANCE"],
    )
    context.append(
        transition_spec(
            context.clock,
            clean_hypothesis,
            "SUPPORTED",
            reason="Clean fixture explicitly contains no original payload",
        )
    )
    contaminated_hypothesis = amend_hypothesis_evidence(
        context,
        hypotheses[1],
        all_evidence,
        axes={"O": 3, "R": 3, "T": 2, "A": 1, "C": 0},
        route_tags=["CONTROLLED_OBSERVATION", "SYNTHETIC_CONFORMANCE"],
    )
    context.append(
        transition_spec(
            context.clock,
            contaminated_hypothesis,
            "FALSIFIED",
            reason="Contaminated fixture declares original payload present and Red",
        )
    )
    finding, _ = accept_finding(
        context,
        "FND-CAM-SYNTH-P5-0001-001",
        evidence_ids=all_evidence,
        functional_rule="Only no-original-payload synthetic material is transferable",
    )
    clean_vector, _ = accept_vector(
        context,
        "VEC-CAM-SYNTH-P5-0001-001",
        [finding["entity_id"]],
    )

    contaminated_vector_id = "VEC-CAM-SYNTH-P5-0001-002"
    contaminated_vector = context.append(
        create_spec(
            context.clock,
            "vector",
            contaminated_vector_id,
            "DRAFT",
            {
                "campaign_id": context.packet["campaign_id"],
                "finding_ids": [finding["entity_id"]],
                "traffic_light": "GREEN",
                "cases": conformance_cases(),
            },
            detail_overrides={
                "finding_ids": [finding["entity_id"]],
                "branch_coverage": "positive,boundary,rejection,invariant",
                "synthesis_provenance": "Deliberately contaminated negative control",
                "no_original_payload_declaration": "DECLINED_CONTAINS_ORIGINAL_PAYLOAD",
                "case_schema": {
                    "required": ["kind", "inputs", "expected", "units", "tolerance"]
                },
            },
        )
    )
    contaminated_vector = context.append(
        transition_spec(
            context.clock,
            contaminated_vector,
            "REVIEWED",
            author="reviewer-b",
            reason="Contamination is explicitly identified",
        )
    )
    context.append(
        create_spec(
            context.clock,
            "review",
            f"REV-{contaminated_vector_id}-01",
            "RECORDED",
            review_payload(contaminated_vector, disposition="PRIVATE_ONLY"),
            author="reviewer-b",
            detail_overrides={
                "target_type": "vector",
                "target_record_hash": contaminated_vector["event_hash"],
                "reviewed_aspects": {
                    "contamination": True,
                    "classification": True,
                },
                "reasons": "Contains original payload; private only",
            },
        )
    )
    context.expect_failure(
        "P5-NEG-CONTAMINATED-VECTOR-ACCEPTANCE",
        lambda: context.append(
            transition_spec(
                context.clock,
                contaminated_vector,
                "ACCEPTED",
                author="reviewer-b",
                reason="Deliberately invalid acceptance attempt",
            )
        ),
        "lacks a no-original-payload attestation",
    )

    clean_transfer_id = "TRN-SYNTH-P5-CLEAN-0001"
    clean_transfer = context.append(
        create_spec(
            context.clock,
            "transfer",
            clean_transfer_id,
            "DRAFT",
            {
                "traffic_light": "GREEN",
                "destination_bead_id": "mc-51m.5",
                "finding_ids": [finding["entity_id"]],
                "vector_ids": [clean_vector["entity_id"]],
                "package_sha256": file_sha256(PACKETS / "P5.json"),
            },
            classification="PUBLIC_SAFE_CANDIDATE",
            detail_overrides={
                "package_schema_version": "1",
                "destination_files": ["synthetic/protocol-smoke-vector.json"],
                "applicability_uncertainties": {
                    "scope": "synthetic protocol qualification only"
                },
                "provisional_choices": {"none": True},
                "runtime_constraints": {"network": "none"},
                "required_behavior_internal_freedom": {
                    "required": "four conformance case classes",
                    "internal": "unconstrained",
                },
                "item_traffic_classification": {
                    clean_vector["entity_id"]: "GREEN"
                },
                "amber_disposition": {"present": False},
                "package_manifest": {
                    "finding_ids": [finding["entity_id"]],
                    "vector_ids": [clean_vector["entity_id"]],
                },
            },
        )
    )
    clean_transfer = context.append(
        transition_spec(
            context.clock,
            clean_transfer,
            "IN_REVIEW",
            reason="Clean transfer package is ready for exact review",
        )
    )
    correct_review_id = f"REV-{clean_transfer_id}-01"
    context.expect_failure(
        "P5-NEG-SELF-REVIEW",
        lambda: context.append(
            create_spec(
                context.clock,
                "review",
                correct_review_id,
                "RECORDED",
                review_payload(clean_transfer),
                author="analyst-a",
                classification="PUBLIC_SAFE_CANDIDATE",
            )
        ),
        "target author",
    )
    context.expect_failure(
        "P5-NEG-INHERITED-REVIEW",
        lambda: context.append(
            create_spec(
                context.clock,
                "review",
                correct_review_id,
                "RECORDED",
                review_payload(
                    clean_transfer,
                    independence="INHERITED_CONTEXT",
                    raw_context_inherited=True,
                ),
                author="reviewer-b",
                classification="PUBLIC_SAFE_CANDIDATE",
            )
        ),
        "inherited analyst context",
    )
    context.append(
        create_spec(
            context.clock,
            "review",
            correct_review_id,
            "RECORDED",
            review_payload(clean_transfer),
            author="reviewer-b",
            classification="PUBLIC_SAFE_CANDIDATE",
            detail_overrides={
                "target_type": "transfer",
                "target_record_hash": clean_transfer["event_hash"],
                "reviewed_aspects": {
                    "wording": True,
                    "classification": True,
                    "evidence": True,
                    "uncertainty": True,
                    "verification": True,
                },
                "reasons": "Exact clean transfer approved from packet-only context",
            },
        )
    )
    clean_transfer = context.append(
        transition_spec(
            context.clock,
            clean_transfer,
            "APPROVED",
            reason="Exact independent review and accepted Green evidence are present",
        )
    )
    receipt_id = f"RCPT-{clean_transfer_id}-01"
    fixture_hash = file_sha256(FIXTURES / "transfer_oracle.py")
    receipt = context.append(
        create_spec(
            context.clock,
            "receipt",
            receipt_id,
            "CURRENT",
            {
                "transfer_id": clean_transfer_id,
                "transfer_event_hash": clean_transfer["event_hash"],
                "destination_bead_id": "mc-51m.5",
            },
            author="implementer-c",
            classification="PUBLIC_SAFE_CANDIDATE",
            detail_overrides=_receipt_detail(
                context,
                receipt_id,
                clean_transfer,
                fixture_hash,
            )["manifest"],
        )
    )
    clean_transfer = context.append(
        transition_spec(
            context.clock,
            clean_transfer,
            "CONSUMED",
            reason="Exact current receipt binds transfer, vectors, destination, and implementation hash",
        )
    )
    context.expect_failure(
        "P5-NEG-NONATOMIC-REVOCATION",
        lambda: context.append(
            transition_spec(
                context.clock,
                clean_transfer,
                "REVOKED",
                reason="Deliberately omitted receipt staling",
            )
        ),
        "current for stale",
    )
    receipt, clean_transfer = context.append_batch(
        [
            transition_spec(
                context.clock,
                receipt,
                "STALE_REVOKED",
                reason="Transfer revocation invalidates exact current receipt",
            ),
            transition_spec(
                context.clock,
                clean_transfer,
                "REVOKED",
                reason="Synthetic revocation smoke control",
            ),
        ]
    )

    contaminated_transfer_id = "TRN-SYNTH-P5-CONTAM-0001"
    contaminated_transfer = context.append(
        create_spec(
            context.clock,
            "transfer",
            contaminated_transfer_id,
            "DRAFT",
            {
                "traffic_light": "GREEN",
                "destination_bead_id": "mc-51m.5",
                "finding_ids": [finding["entity_id"]],
                "vector_ids": [contaminated_vector["entity_id"]],
                "package_sha256": file_sha256(FIXTURES / "transfer_oracle.py"),
            },
            classification="PUBLIC_SAFE_CANDIDATE",
            detail_overrides={
                "package_schema_version": "1",
                "destination_files": ["synthetic/contaminated-negative-control.json"],
                "applicability_uncertainties": {"contamination": True},
                "provisional_choices": {"none": True},
                "runtime_constraints": {"network": "none"},
                "required_behavior_internal_freedom": {
                    "required": "must not approve",
                    "internal": "not applicable",
                },
                "item_traffic_classification": {
                    contaminated_vector["entity_id"]: "PRIVATE_ONLY"
                },
                "amber_disposition": {"present": False},
                "package_manifest": {
                    "finding_ids": [finding["entity_id"]],
                    "vector_ids": [contaminated_vector["entity_id"]],
                },
            },
        )
    )
    contaminated_transfer = context.append(
        transition_spec(
            context.clock,
            contaminated_transfer,
            "IN_REVIEW",
            reason="Contaminated negative-control transfer enters review",
        )
    )
    context.append(
        create_spec(
            context.clock,
            "review",
            f"REV-{contaminated_transfer_id}-01",
            "RECORDED",
            review_payload(contaminated_transfer, disposition="PRIVATE_ONLY"),
            author="reviewer-c",
            classification="PUBLIC_SAFE_CANDIDATE",
            detail_overrides={
                "target_type": "transfer",
                "target_record_hash": contaminated_transfer["event_hash"],
                "reviewed_aspects": {
                    "contamination": True,
                    "transferability": True,
                },
                "reasons": "Contaminated vector is not accepted or transferable",
            },
        )
    )
    context.expect_failure(
        "P5-NEG-CONTAMINATED-TRANSFER-APPROVAL",
        lambda: context.append(
            transition_spec(
                context.clock,
                contaminated_transfer,
                "APPROVED",
                reason="Deliberately invalid contaminated approval",
            )
        ),
        "vector",
    )
    context.append(
        transition_spec(
            context.clock,
            contaminated_transfer,
            "PRIVATE_ONLY",
            reason="Independent review rejects contaminated material from transfer",
        )
    )
    finish_phase(context, context.states()[experiment["entity_id"]])
    return phase_summary(context)


SCENARIOS: dict[str, Callable[[Path], dict[str, Any]]] = {
    "P1": run_p1,
    "P2": run_p2,
    "P3": run_p3,
    "P4": run_p4,
    "P5": run_p5,
}


def prepare_output(output_root: Path) -> Path:
    absolute = Path(os.path.abspath(os.fspath(output_root)))
    if absolute.exists():
        raise ScenarioError(f"output directory already exists: {absolute}")
    allowed_private_root = BASE
    allowed_tmp_root = Path("/tmp")
    if not (
        absolute.is_relative_to(allowed_private_root)
        or absolute.is_relative_to(allowed_tmp_root)
    ):
        raise ScenarioError(
            "output directory must be under /tmp or research/private/protocol-smoke"
        )
    if not absolute.parent.is_dir() or absolute.parent.is_symlink():
        raise ScenarioError("output parent must be an existing non-symlink directory")
    _mkdir_private(absolute)
    return absolute


def run_all(output_root: Path) -> dict[str, Any]:
    output_root = prepare_output(output_root)
    phases = {name: SCENARIOS[name](output_root) for name in sorted(SCENARIOS)}
    summary = {
        "schema_version": 1,
        "suite": "mc-51m.5-known-source-synthetic-protocol-smoke",
        "phases": phases,
        "exposure_declaration": {
            "owned_mc2_paths_or_bytes": "none",
            "network": "none",
            "remote_models": "none",
            "source_class": "project-authored known-source synthetic only",
            "raw_event_streams": "caller-supplied private temporary output only",
        },
    }
    write_private_json(output_root / "summary.json", summary)
    return summary


def comparable_summary(summary: dict[str, Any]) -> dict[str, Any]:
    return summary


def load_expected() -> dict[str, Any]:
    with EXPECTED_PATH.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def artifact_manifest() -> dict[str, str]:
    repo_root = BASE.parents[2]
    paths = [
        path
        for path in BASE.rglob("*")
        if path.is_file()
        and path != EXPECTED_PATH
        and path.suffix not in {".pyc", ".pyo"}
        and "__pycache__" not in path.parts
    ]
    paths.extend([REGISTRY_SOURCE / "registry.py", PYTHON, OS_RELEASE])
    manifest: dict[str, str] = {}
    for path in sorted(set(paths)):
        key = (
            str(path.relative_to(repo_root))
            if path.is_relative_to(repo_root)
            else str(path)
        )
        manifest[key] = file_sha256(path)
    return manifest


def verify_inputs() -> None:
    expected = load_expected()["artifact_sha256"]
    actual = artifact_manifest()
    if actual != expected:
        missing = sorted(set(expected) - set(actual))
        extra = sorted(set(actual) - set(expected))
        changed = sorted(
            key
            for key in set(actual) & set(expected)
            if actual[key] != expected[key]
        )
        raise ScenarioError(
            f"input manifest mismatch: missing={missing} extra={extra} "
            f"changed={changed}"
        )


def verify_expected(summary: dict[str, Any]) -> None:
    expected = load_expected()
    if comparable_summary(summary) != expected["summary"]:
        raise ScenarioError("actual deterministic summary differs from expected.json")
