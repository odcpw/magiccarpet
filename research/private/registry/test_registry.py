from __future__ import annotations

import copy
import json
import os
import stat
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import registry


HASH_A = "a" * 64
HASH_B = "b" * 64
HASH_C = "c" * 64


def detail_for(entity_type: str, entity_id: str, payload: dict) -> dict:
    manifest: dict = {}
    for field in registry.DETAIL_REQUIRED_FIELDS[entity_type]:
        if field in registry.DETAIL_STRING_FIELDS:
            manifest[field] = "synthetic"
        elif field in registry.DETAIL_LIST_FIELDS:
            manifest[field] = ["synthetic"]
        else:
            manifest[field] = {"synthetic": True}
    if entity_type == "calibration":
        manifest["result_disposition"] = "PASS"
    if entity_type == "build_manifest":
        manifest["config_hashes"] = {"synthetic.cfg": HASH_A}
    elif entity_type == "tool":
        manifest["dependency_hashes"] = {"runtime": HASH_A}
    elif entity_type == "campaign":
        manifest["calibration_ids"] = payload["calibration_ids"]
    elif entity_type == "hypothesis":
        manifest["question_id"] = payload["question_id"]
        manifest["evidence_route_ids"] = payload["evidence_ids"]
        manifest["contradiction_ids"] = ["NONE"]
    elif entity_type == "finding":
        manifest["question_id"] = payload["question_id"]
        manifest["evidence_ids"] = payload["evidence_ids"]
        manifest["contradiction_ids"] = ["NONE"]
    elif entity_type == "vector":
        manifest["finding_ids"] = payload["finding_ids"]
        manifest["no_original_payload_declaration"] = (
            "ATTESTED_NO_ORIGINAL_PAYLOAD"
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
    return {
        "schema_id": f"mc2-behavioral-registry/{entity_type}/0.2",
        "artifact_id": f"DET-{entity_id}",
        "manifest": manifest,
        "manifest_sha256": registry.detail_manifest_sha256(manifest),
    }


def create_spec(
    entity_type: str,
    entity_id: str,
    status: str,
    payload: dict,
    author: str = "analyst-a",
    classification: str = "SYNTHETIC_ONLY",
) -> dict:
    payload = copy.deepcopy(payload)
    payload.setdefault("detail", detail_for(entity_type, entity_id, payload))
    return {
        "event_type": "CREATE",
        "entity_type": entity_type,
        "entity_id": entity_id,
        "author": author,
        "classification": classification,
        "status": status,
        "payload": payload,
    }


def transition_spec(
    state: dict,
    status: str,
    author: str = "analyst-a",
    reason: str = "test transition",
    independence: str = "FRESH_NO_CONTEXT",
) -> dict:
    payload = {"reason": reason}
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
    }


def link_spec(
    state: dict,
    target_id: str,
    relation: str,
    author: str = "analyst-a",
) -> dict:
    return {
        "event_type": "LINK",
        "entity_type": state["entity_type"],
        "entity_id": state["entity_id"],
        "author": author,
        "classification": state["classification"],
        "status": state["status"],
        "payload": {"target_id": target_id, "relation": relation},
    }


def amend_spec(
    state: dict,
    changes: dict,
    author: str = "analyst-a",
    reason: str = "test amendment",
) -> dict:
    return {
        "event_type": "AMEND",
        "entity_type": state["entity_type"],
        "entity_id": state["entity_id"],
        "author": author,
        "classification": state["classification"],
        "status": state["status"],
        "payload": {"reason": reason, "changes": changes},
    }


def review_payload(
    target: dict,
    disposition: str = "APPROVE",
    independence: str = "FRESH_NO_CONTEXT",
    raw_context_inherited: bool = False,
) -> dict:
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
            "context_source": "independent synthetic packet",
            "raw_context_inherited": raw_context_inherited,
            "conflicts_disclosed": True,
        },
    }


def conformance_cases() -> list[dict]:
    return [
        {
            "kind": kind,
            "inputs": {"value": index},
            "expected": {"accepted": kind != "REJECTION"},
            "units": "synthetic_units",
            "tolerance": 0,
        }
        for index, kind in enumerate(
            ["POSITIVE", "BOUNDARY", "REJECTION", "INVARIANT"]
        )
    ]


def seed_campaign(registry_dir: Path) -> dict:
    append = lambda spec: registry.append_spec(registry_dir, spec)
    append(
        create_spec(
            "source",
            "SRC-SYNTH-0001",
            "REGISTERED",
            {
                "title": "Synthetic source",
                "source_class": "SYNTHETIC",
                "artifact_hashes": {"fixture.bin": HASH_A},
            },
        )
    )
    append(
        create_spec(
            "build_manifest",
            "BLD-SYNTH-0001",
            "FROZEN",
            {
                "source_ids": ["SRC-SYNTH-0001"],
                "artifact_hashes": {"inner.exe": HASH_B},
            },
        )
    )
    append(
        create_spec(
            "tool",
            "TOOL-SYNTH-0001",
            "FROZEN",
            {
                "name": "Synthetic tool",
                "version": "1",
                "origin": "known source",
                "artifact_hashes": {"tool.py": HASH_C},
            },
        )
    )
    append(
        create_spec(
            "environment",
            "ENV-SYNTH-0001",
            "FROZEN",
            {
                "tool_ids": ["TOOL-SYNTH-0001"],
                "artifact_hashes": {"environment.txt": HASH_A},
            },
        )
    )
    calibration = append(
        create_spec(
            "calibration",
            "CAL-ENV-SYNTH-0001-001",
            "DRAFT",
            {
                "environment_id": "ENV-SYNTH-0001",
                "metric": "Synthetic clock",
                "threshold": "exact",
                "result": "pass",
                "artifact_hashes": {"calibration.json": HASH_C},
            },
        )
    )
    append(transition_spec(calibration, "PASSED"))
    campaign = append(
        create_spec(
            "campaign",
            "CAM-SYNTH-PROTOCOL-0001",
            "OPEN",
            {
                "bead_id": "mc-synthetic",
                "build_id": "BLD-SYNTH-0001",
                "environment_ids": ["ENV-SYNTH-0001"],
                "calibration_ids": ["CAL-ENV-SYNTH-0001-001"],
                "question_scope": "Synthetic protocol tests",
                "denied_operations": ["owned game access"],
            },
        )
    )
    append(
        create_spec(
            "question",
            "Q-CAM-SYNTH-PROTOCOL-0001-01",
            "OPEN",
            {
                "campaign_id": "CAM-SYNTH-PROTOCOL-0001",
                "question": "Does the synthetic rule hold?",
                "falsifier": "A controlled counterexample",
            },
        )
    )
    return campaign


def seed_accepted_transfer_evidence(registry_dir: Path) -> tuple[dict, dict]:
    seed_campaign(registry_dir)
    finding = registry.append_spec(
        registry_dir,
        create_spec(
            "finding",
            "FND-CAM-SYNTH-PROTOCOL-0001-001",
            "DRAFT",
            {
                "campaign_id": "CAM-SYNTH-PROTOCOL-0001",
                "question_id": "Q-CAM-SYNTH-PROTOCOL-0001-01",
                "evidence_ids": ["SRC-SYNTH-0001"],
                "functional_rule": "Synthetic bounded rule",
                "fidelity_classification": "ORIGINAL_ADDITION",
                "evidence_floor": "SIMPLE_BOUNDED",
                "evidence_axes": {"O": 2, "R": 2, "T": 2, "A": 1, "C": 0},
                "evidence_route_tags": [
                    "CONTROLLED_OBSERVATION",
                    "SYNTHETIC_CONFORMANCE",
                ],
                "rule_domains": ["DISCRETE"],
                "contradiction_resolution": "NONE",
                "applicability": "Synthetic only",
            },
        ),
    )
    finding = registry.append_spec(
        registry_dir, transition_spec(finding, "CORROBORATED")
    )
    finding = registry.append_spec(
        registry_dir, transition_spec(finding, "REVIEWED", author="reviewer-b")
    )
    registry.append_spec(
        registry_dir,
        create_spec(
            "review",
            "REV-FND-CAM-SYNTH-PROTOCOL-0001-001-01",
            "RECORDED",
            review_payload(finding),
            author="reviewer-b",
        ),
    )
    finding = registry.append_spec(
        registry_dir, transition_spec(finding, "ACCEPTED", author="reviewer-b")
    )
    vector = registry.append_spec(
        registry_dir,
        create_spec(
            "vector",
            "VEC-CAM-SYNTH-PROTOCOL-0001-001",
            "DRAFT",
            {
                "campaign_id": "CAM-SYNTH-PROTOCOL-0001",
                "finding_ids": ["FND-CAM-SYNTH-PROTOCOL-0001-001"],
                "traffic_light": "GREEN",
                "cases": conformance_cases(),
            },
        ),
    )
    vector = registry.append_spec(
        registry_dir, transition_spec(vector, "REVIEWED", author="reviewer-b")
    )
    registry.append_spec(
        registry_dir,
        create_spec(
            "review",
            "REV-VEC-CAM-SYNTH-PROTOCOL-0001-001-01",
            "RECORDED",
            review_payload(vector),
            author="reviewer-b",
        ),
    )
    vector = registry.append_spec(
        registry_dir, transition_spec(vector, "ACCEPTED", author="reviewer-b")
    )
    return finding, vector


class RegistryTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.registry_dir = Path(self.temp.name) / "registry"
        registry.initialize_registry(self.registry_dir, "test-owner")

    def tearDown(self) -> None:
        self.temp.cleanup()

    def events(self) -> list[dict]:
        return registry.load_events(self.registry_dir / registry.EVENTS_NAME)

    def append(self, spec: dict) -> dict:
        return registry.append_spec(self.registry_dir, spec)

    def receipt_detail(self, receipt_id: str, transfer_id: str) -> dict:
        events = self.events()
        states = registry.entity_state(events)
        payloads = registry.entity_logical_payloads(events)
        transfer_payload = payloads[transfer_id]
        detail = detail_for(
            "receipt",
            receipt_id,
            {
                "transfer_id": transfer_id,
                "transfer_event_hash": states[transfer_id]["event_hash"],
                "destination_bead_id": transfer_payload["destination_bead_id"],
            },
        )
        manifest = detail["manifest"]
        manifest["vector_event_hashes"] = [
            states[vector_id]["event_hash"]
            for vector_id in transfer_payload["vector_ids"]
        ]
        manifest["destination_files"] = transfer_payload["detail"]["manifest"][
            "destination_files"
        ]
        manifest["implementation_artifact_hashes"] = {
            "synthetic-output": HASH_A
        }
        detail["manifest_sha256"] = registry.detail_manifest_sha256(manifest)
        return detail

    def source(self) -> dict:
        return self.append(
            create_spec(
                "source",
                "SRC-SYNTH-0001",
                "REGISTERED",
                {
                    "title": "Synthetic source",
                    "source_class": "SYNTHETIC",
                    "artifact_hashes": {"fixture.bin": HASH_A},
                },
            )
        )

    def build(self) -> dict:
        self.source()
        return self.append(
            create_spec(
                "build_manifest",
                "BLD-SYNTH-0001",
                "FROZEN",
                {
                    "source_ids": ["SRC-SYNTH-0001"],
                    "artifact_hashes": {"inner.exe": HASH_B},
                    "references": ["SRC-SYNTH-0001"],
                },
            )
        )

    def transfer_chain(self) -> tuple[dict, dict, dict]:
        finding, vector = seed_accepted_transfer_evidence(self.registry_dir)
        transfer = self.append(
            create_spec(
                "transfer",
                "TRN-FLIGHT-0001",
                "DRAFT",
                {
                    "traffic_light": "GREEN",
                    "destination_bead_id": "mc-synthetic",
                    "finding_ids": [finding["entity_id"]],
                    "vector_ids": [vector["entity_id"]],
                    "package_sha256": HASH_C,
                },
                author="analyst-a",
                classification="PUBLIC_SAFE_CANDIDATE",
            )
        )
        in_review = self.append(transition_spec(transfer, "IN_REVIEW"))
        review = self.append(
            create_spec(
                "review",
                "REV-TRN-FLIGHT-0001-01",
                "RECORDED",
                review_payload(in_review),
                author="reviewer-b",
                classification="PUBLIC_SAFE_CANDIDATE",
            )
        )
        approved = self.append(transition_spec(in_review, "APPROVED"))
        return approved, review, in_review

    def test_owner_only_storage_and_round_trip(self) -> None:
        self.build()
        registry.validate_storage(self.registry_dir)
        registry.validate_events(self.events())
        for path in self.registry_dir.iterdir():
            mode = stat.S_IMODE(path.stat().st_mode)
            self.assertEqual(mode & 0o077, 0)
        rendered = registry.render_markdown(self.events())
        self.assertIn("BLD-SYNTH-0001", rendered)

    def test_tamper_and_broken_prior_hash_are_rejected(self) -> None:
        self.build()
        events = self.events()
        tampered = copy.deepcopy(events)
        tampered[0]["payload"]["title"] = "changed"
        with self.assertRaisesRegex(registry.RegistryError, "content hash mismatch"):
            registry.validate_events(tampered)

        broken = copy.deepcopy(events)
        broken[1]["previous_event_hash"] = HASH_C
        without_hash = {
            key: value for key, value in broken[1].items() if key != "event_hash"
        }
        broken[1]["event_hash"] = registry.hash_event(without_hash)
        with self.assertRaisesRegex(registry.RegistryError, "broken global prior hash"):
            registry.validate_events(broken)

    def test_reused_entity_and_missing_reference_are_rejected(self) -> None:
        self.source()
        with self.assertRaisesRegex(registry.RegistryError, "reused entity ID"):
            self.append(
                create_spec(
                    "source",
                    "SRC-SYNTH-0001",
                    "REGISTERED",
                    {
                        "title": "Duplicate",
                        "source_class": "SYNTHETIC",
                        "artifact_hashes": {"fixture.bin": HASH_A},
                    },
                )
            )

    def test_semantic_dependencies_are_typed_and_mandatory(self) -> None:
        source = self.source()
        with self.assertRaisesRegex(registry.RegistryError, "unknown entity"):
            self.append(
                create_spec(
                    "build_manifest",
                    "BLD-SYNTH-9999",
                    "FROZEN",
                    {
                        "source_ids": ["SRC-SYNTH-9999"],
                        "artifact_hashes": {"inner.exe": HASH_B},
                    },
                )
            )
        with self.assertRaisesRegex(registry.RegistryError, "expects.*tool"):
            self.append(
                create_spec(
                    "environment",
                    "ENV-SYNTH-0001",
                    "FROZEN",
                    {
                        "tool_ids": [source["entity_id"]],
                        "artifact_hashes": {"environment.txt": HASH_A},
                    },
                )
            )

    def test_nested_ids_are_bound_to_declared_parents(self) -> None:
        seed_campaign(self.registry_dir)
        self.append(
            create_spec(
                "campaign",
                "CAM-SYNTH-OTHER-0002",
                "OPEN",
                {
                    "bead_id": "mc-other",
                    "build_id": "BLD-SYNTH-0001",
                    "environment_ids": ["ENV-SYNTH-0001"],
                    "calibration_ids": ["CAL-ENV-SYNTH-0001-001"],
                    "question_scope": "Other synthetic campaign",
                    "denied_operations": ["owned game access"],
                },
            )
        )
        with self.assertRaisesRegex(registry.RegistryError, "does not embed"):
            self.append(
                create_spec(
                    "question",
                    "Q-CAM-SYNTH-PROTOCOL-0001-02",
                    "OPEN",
                    {
                        "campaign_id": "CAM-SYNTH-OTHER-0002",
                        "question": "Mismatched parent?",
                        "falsifier": "Any result",
                    },
                )
            )

    def test_create_payload_fields_and_types_are_closed(self) -> None:
        with self.assertRaisesRegex(registry.RegistryError, "non-empty string"):
            self.append(
                create_spec(
                    "source",
                    "SRC-SYNTH-0001",
                    "REGISTERED",
                    {
                        "title": 7,
                        "source_class": "SYNTHETIC",
                        "artifact_hashes": {"fixture.bin": HASH_A},
                    },
                )
            )
        with self.assertRaisesRegex(registry.RegistryError, "extra=.*secret"):
            self.append(
                create_spec(
                    "source",
                    "SRC-SYNTH-0001",
                    "REGISTERED",
                    {
                        "title": "Synthetic source",
                        "source_class": "SYNTHETIC",
                        "artifact_hashes": {"fixture.bin": HASH_A},
                        "secret": "not schema-backed",
                    },
                )
            )

    def test_typed_detail_manifest_is_required_and_hash_bound(self) -> None:
        spec = create_spec(
            "source",
            "SRC-SYNTH-0001",
            "REGISTERED",
            {
                "title": "Synthetic source",
                "source_class": "SYNTHETIC",
                "artifact_hashes": {"fixture.bin": HASH_A},
            },
        )
        missing = copy.deepcopy(spec)
        del missing["payload"]["detail"]
        with self.assertRaisesRegex(registry.RegistryError, "missing=.*detail"):
            self.append(missing)

        mismatched = copy.deepcopy(spec)
        mismatched["payload"]["detail"]["manifest"]["revision"] = "changed"
        with self.assertRaisesRegex(registry.RegistryError, "manifest_sha256 mismatch"):
            self.append(mismatched)

        incomplete = copy.deepcopy(spec)
        del incomplete["payload"]["detail"]["manifest"]["approved_use"]
        incomplete["payload"]["detail"]["manifest_sha256"] = (
            registry.detail_manifest_sha256(
                incomplete["payload"]["detail"]["manifest"]
            )
        )
        with self.assertRaisesRegex(registry.RegistryError, "manifest fields differ"):
            self.append(incomplete)

        source = self.append(spec)
        malformed_hashes = create_spec(
            "build_manifest",
            "BLD-SYNTH-0001",
            "FROZEN",
            {
                "source_ids": [source["entity_id"]],
                "artifact_hashes": {"inner.exe": HASH_B},
            },
        )
        detail = malformed_hashes["payload"]["detail"]
        detail["manifest"]["config_hashes"] = {"synthetic.cfg": "not-a-hash"}
        detail["manifest_sha256"] = registry.detail_manifest_sha256(
            detail["manifest"]
        )
        with self.assertRaisesRegex(registry.RegistryError, "lowercase SHA-256"):
            self.append(malformed_hashes)

    def test_detail_ids_are_bound_to_registered_dependencies(self) -> None:
        seed_campaign(self.registry_dir)
        campaign = create_spec(
            "campaign",
            "CAM-SYNTH-OTHER-0002",
            "OPEN",
            {
                "bead_id": "mc-other",
                "build_id": "BLD-SYNTH-0001",
                "environment_ids": ["ENV-SYNTH-0001"],
                "calibration_ids": ["CAL-ENV-SYNTH-0001-001"],
                "question_scope": "Other synthetic campaign",
                "denied_operations": ["owned game access"],
            },
        )
        detail = campaign["payload"]["detail"]
        detail["manifest"]["calibration_ids"] = ["CAL-ENV-SYNTH-9999-001"]
        detail["manifest_sha256"] = registry.detail_manifest_sha256(
            detail["manifest"]
        )
        with self.assertRaisesRegex(registry.RegistryError, "calibration_ids mismatch"):
            self.append(campaign)

        with self.assertRaisesRegex(registry.RegistryError, "unknown entity"):
            self.append(
                create_spec(
                    "finding",
                    "FND-CAM-SYNTH-PROTOCOL-0001-099",
                    "DRAFT",
                    {
                        "campaign_id": "CAM-SYNTH-PROTOCOL-0001",
                        "question_id": "Q-CAM-SYNTH-PROTOCOL-0001-01",
                        "evidence_ids": [
                            "OBS-RUN-EXP-CAM-SYNTH-PROTOCOL-0001-001-001-01"
                        ],
                        "functional_rule": "Unregistered evidence must fail",
                        "fidelity_classification": "REFERENCE_HYPOTHESIS",
                        "evidence_floor": "SIMPLE_BOUNDED",
                        "evidence_axes": {
                            "O": 1,
                            "R": 0,
                            "T": 0,
                            "A": 1,
                            "C": 0,
                        },
                        "evidence_route_tags": ["CONTROLLED_OBSERVATION"],
                        "rule_domains": ["DISCRETE"],
                        "contradiction_resolution": "NONE",
                        "applicability": "Synthetic only",
                    },
                )
            )

        with self.assertRaisesRegex(registry.RegistryError, "unknown entity"):
            self.append(
                create_spec(
                    "build_manifest",
                    "BLD-SYNTH-9998",
                    "FROZEN",
                    {
                        "source_ids": ["SRC-SYNTH-9999"],
                        "artifact_hashes": {"inner.exe": HASH_B},
                        "references": ["SRC-SYNTH-9999"],
                    },
                )
            )

    def test_invalid_lifecycle_is_rejected(self) -> None:
        source = self.source()
        with self.assertRaisesRegex(registry.RegistryError, "invalid transition"):
            self.append(transition_spec(source, "APPROVED"))

    def test_red_vector_is_rejected(self) -> None:
        with self.assertRaisesRegex(registry.RegistryError, "prohibited Red"):
            self.append(
                create_spec(
                    "vector",
                    "VEC-CAM-SYNTH-PROTOCOL-0001-001",
                    "DRAFT",
                    {
                        "campaign_id": "CAM-SYNTH-PROTOCOL-0001",
                        "finding_ids": ["FND-CAM-SYNTH-PROTOCOL-0001-001"],
                        "traffic_light": "RED",
                        "cases": [],
                    },
                )
            )

    def test_self_review_and_inherited_context_are_rejected(self) -> None:
        finding, vector = seed_accepted_transfer_evidence(self.registry_dir)
        transfer = self.append(
            create_spec(
                "transfer",
                "TRN-FLIGHT-0001",
                "DRAFT",
                {
                    "traffic_light": "GREEN",
                    "destination_bead_id": "mc-synthetic",
                    "finding_ids": [finding["entity_id"]],
                    "vector_ids": [vector["entity_id"]],
                    "package_sha256": HASH_C,
                },
                author="analyst-a",
                classification="PUBLIC_SAFE_CANDIDATE",
            )
        )
        in_review = self.append(transition_spec(transfer, "IN_REVIEW"))
        with self.assertRaisesRegex(registry.RegistryError, "target author"):
            self.append(
                create_spec(
                    "review",
                    "REV-TRN-FLIGHT-0001-01",
                    "RECORDED",
                    review_payload(in_review),
                    author="analyst-a",
                    classification="PUBLIC_SAFE_CANDIDATE",
                )
            )

    def test_finding_and_vector_require_exact_independent_review(self) -> None:
        seed_campaign(self.registry_dir)
        finding = self.append(
            create_spec(
                "finding",
                "FND-CAM-SYNTH-PROTOCOL-0001-001",
                "DRAFT",
                {
                    "campaign_id": "CAM-SYNTH-PROTOCOL-0001",
                    "question_id": "Q-CAM-SYNTH-PROTOCOL-0001-01",
                    "evidence_ids": ["SRC-SYNTH-0001"],
                    "functional_rule": "Synthetic bounded rule",
                    "fidelity_classification": "ORIGINAL_ADDITION",
                    "evidence_floor": "SIMPLE_BOUNDED",
                    "evidence_axes": {
                        "O": 2,
                        "R": 2,
                        "T": 2,
                        "A": 1,
                        "C": 0,
                    },
                    "evidence_route_tags": [
                        "CONTROLLED_OBSERVATION",
                        "SYNTHETIC_CONFORMANCE",
                    ],
                    "rule_domains": ["DISCRETE"],
                    "contradiction_resolution": "NONE",
                    "applicability": "Synthetic only",
                },
            )
        )
        finding = self.append(transition_spec(finding, "CORROBORATED"))
        with self.assertRaisesRegex(registry.RegistryError, "self-reviewed"):
            self.append(transition_spec(finding, "REVIEWED", author="analyst-a"))
        finding = self.append(
            transition_spec(finding, "REVIEWED", author="reviewer-b")
        )
        with self.assertRaisesRegex(registry.RegistryError, "exact independent review"):
            self.append(
                transition_spec(finding, "ACCEPTED", author="reviewer-b")
            )
        with self.assertRaisesRegex(registry.RegistryError, "target author"):
            self.append(
                create_spec(
                    "review",
                    "REV-FND-CAM-SYNTH-PROTOCOL-0001-001-01",
                    "RECORDED",
                    review_payload(finding),
                    author="analyst-a",
                )
            )

        vector = self.append(
            create_spec(
                "vector",
                "VEC-CAM-SYNTH-PROTOCOL-0001-001",
                "DRAFT",
                {
                    "campaign_id": "CAM-SYNTH-PROTOCOL-0001",
                    "finding_ids": ["FND-CAM-SYNTH-PROTOCOL-0001-001"],
                    "traffic_light": "GREEN",
                    "cases": conformance_cases(),
                },
            )
        )
        with self.assertRaisesRegex(registry.RegistryError, "self-reviewed"):
            self.append(transition_spec(vector, "REVIEWED", author="analyst-a"))

    def test_review_binds_exact_transfer_version_and_is_immutable(self) -> None:
        finding, vector = seed_accepted_transfer_evidence(self.registry_dir)
        transfer = self.append(
            create_spec(
                "transfer",
                "TRN-FLIGHT-0001",
                "DRAFT",
                {
                    "traffic_light": "GREEN",
                    "destination_bead_id": "mc-synthetic",
                    "finding_ids": [finding["entity_id"]],
                    "vector_ids": [vector["entity_id"]],
                    "package_sha256": HASH_C,
                },
                classification="PUBLIC_SAFE_CANDIDATE",
            )
        )
        with self.assertRaisesRegex(registry.RegistryError, "immutable"):
            self.append(amend_spec(transfer, {"package_sha256": HASH_B}))
        in_review = self.append(transition_spec(transfer, "IN_REVIEW"))
        review = self.append(
            create_spec(
                "review",
                "REV-TRN-FLIGHT-0001-01",
                "RECORDED",
                review_payload(in_review),
                author="reviewer-b",
                classification="PUBLIC_SAFE_CANDIDATE",
            )
        )
        with self.assertRaisesRegex(registry.RegistryError, "immutable"):
            self.append(amend_spec(review, {"disposition": "APPROVE"}))
        with self.assertRaisesRegex(registry.RegistryError, "immutable"):
            self.append(link_spec(in_review, finding["entity_id"], "rejects"))
        revised = self.append(transition_spec(in_review, "REVISE"))
        draft = self.append(transition_spec(revised, "DRAFT"))
        second_in_review = self.append(transition_spec(draft, "IN_REVIEW"))
        with self.assertRaisesRegex(registry.RegistryError, "exact independent review"):
            self.append(transition_spec(second_in_review, "APPROVED"))

    def test_private_and_c2_c3_evidence_cannot_be_accepted(self) -> None:
        seed_campaign(self.registry_dir)
        private_finding = self.append(
            create_spec(
                "finding",
                "FND-CAM-SYNTH-PROTOCOL-0001-002",
                "DRAFT",
                {
                    "campaign_id": "CAM-SYNTH-PROTOCOL-0001",
                    "question_id": "Q-CAM-SYNTH-PROTOCOL-0001-01",
                    "evidence_ids": ["SRC-SYNTH-0001"],
                    "functional_rule": "Private finding",
                    "fidelity_classification": "REFERENCE_FACT",
                    "evidence_floor": "SIMPLE_BOUNDED",
                    "evidence_axes": {
                        "O": 3,
                        "R": 3,
                        "T": 3,
                        "A": 3,
                        "C": 0,
                    },
                    "evidence_route_tags": [
                        "CONTROLLED_OBSERVATION",
                        "STATIC_ANALYSIS",
                        "INDEPENDENT_REPRODUCTION",
                    ],
                    "rule_domains": ["DISCRETE"],
                    "contradiction_resolution": "NONE",
                    "applicability": "Synthetic only",
                },
                classification="PRIVATE_OWNED_REFERENCE",
            )
        )
        private_finding = self.append(
            transition_spec(private_finding, "CORROBORATED")
        )
        private_finding = self.append(
            transition_spec(private_finding, "REVIEWED", author="reviewer-b")
        )
        with self.assertRaisesRegex(registry.RegistryError, "private"):
            self.append(
                transition_spec(private_finding, "ACCEPTED", author="reviewer-b")
            )

    def test_critical_rule_domain_cannot_select_simple_floor(self) -> None:
        seed_campaign(self.registry_dir)
        with self.assertRaisesRegex(registry.RegistryError, "CRITICAL evidence floor"):
            self.append(
                create_spec(
                    "finding",
                    "FND-CAM-SYNTH-PROTOCOL-0001-001",
                    "DRAFT",
                    {
                        "campaign_id": "CAM-SYNTH-PROTOCOL-0001",
                        "question_id": "Q-CAM-SYNTH-PROTOCOL-0001-01",
                        "evidence_ids": ["SRC-SYNTH-0001"],
                        "functional_rule": "Synthetic network ordering rule",
                        "fidelity_classification": "ORIGINAL_ADDITION",
                        "evidence_floor": "SIMPLE_BOUNDED",
                        "evidence_axes": {
                            "O": 3,
                            "R": 3,
                            "T": 2,
                            "A": 1,
                            "C": 0,
                        },
                        "evidence_route_tags": [
                            "CONTROLLED_OBSERVATION",
                            "SYNTHETIC_CONFORMANCE",
                        ],
                        "rule_domains": ["NETWORK", "ORDERING"],
                        "contradiction_resolution": "NONE",
                        "applicability": "Synthetic only",
                    },
                )
            )

    def test_c1_acceptance_requires_reviewed_contradiction_resolution(self) -> None:
        seed_campaign(self.registry_dir)
        finding = self.append(
            create_spec(
                "finding",
                "FND-CAM-SYNTH-PROTOCOL-0001-001",
                "DRAFT",
                {
                    "campaign_id": "CAM-SYNTH-PROTOCOL-0001",
                    "question_id": "Q-CAM-SYNTH-PROTOCOL-0001-01",
                    "evidence_ids": ["SRC-SYNTH-0001"],
                    "functional_rule": "Synthetic scoped rule",
                    "fidelity_classification": "ORIGINAL_ADDITION",
                    "evidence_floor": "SIMPLE_BOUNDED",
                    "evidence_axes": {
                        "O": 2,
                        "R": 2,
                        "T": 2,
                        "A": 1,
                        "C": 1,
                    },
                    "evidence_route_tags": [
                        "CONTROLLED_OBSERVATION",
                        "SYNTHETIC_CONFORMANCE",
                    ],
                    "rule_domains": ["DISCRETE"],
                    "contradiction_resolution": "Claimed scope separation",
                    "applicability": "Synthetic only",
                },
            )
        )
        finding = self.append(transition_spec(finding, "CORROBORATED"))
        finding = self.append(
            transition_spec(finding, "REVIEWED", author="reviewer-b")
        )
        self.append(
            create_spec(
                "review",
                "REV-FND-CAM-SYNTH-PROTOCOL-0001-001-01",
                "RECORDED",
                review_payload(finding),
                author="reviewer-b",
            )
        )
        with self.assertRaisesRegex(registry.RegistryError, "is not resolved"):
            self.append(
                transition_spec(finding, "ACCEPTED", author="reviewer-b")
            )

    def test_amber_vector_cannot_be_laundered_through_green_transfer(self) -> None:
        finding, _ = seed_accepted_transfer_evidence(self.registry_dir)
        vector = self.append(
            create_spec(
                "vector",
                "VEC-CAM-SYNTH-PROTOCOL-0001-002",
                "DRAFT",
                {
                    "campaign_id": "CAM-SYNTH-PROTOCOL-0001",
                    "finding_ids": ["FND-CAM-SYNTH-PROTOCOL-0001-001"],
                    "traffic_light": "AMBER",
                    "cases": conformance_cases(),
                },
            )
        )
        vector = self.append(
            transition_spec(vector, "REVIEWED", author="reviewer-b")
        )
        self.append(
            create_spec(
                "review",
                "REV-VEC-CAM-SYNTH-PROTOCOL-0001-002-01",
                "RECORDED",
                review_payload(vector),
                author="reviewer-b",
            )
        )
        vector = self.append(
            transition_spec(vector, "ACCEPTED", author="reviewer-b")
        )
        transfer = self.append(
            create_spec(
                "transfer",
                "TRN-AMBER-0001",
                "DRAFT",
                {
                    "traffic_light": "GREEN",
                    "destination_bead_id": "mc-synthetic",
                    "finding_ids": [finding["entity_id"]],
                    "vector_ids": [vector["entity_id"]],
                    "package_sha256": HASH_C,
                },
                classification="PUBLIC_SAFE_CANDIDATE",
            )
        )
        transfer = self.append(transition_spec(transfer, "IN_REVIEW"))
        self.append(
            create_spec(
                "review",
                "REV-TRN-AMBER-0001-01",
                "RECORDED",
                review_payload(transfer),
                author="reviewer-b",
                classification="PUBLIC_SAFE_CANDIDATE",
            )
        )
        with self.assertRaisesRegex(registry.RegistryError, "Amber vector"):
            self.append(transition_spec(transfer, "APPROVED"))

        contradicted = self.append(
            create_spec(
                "finding",
                "FND-CAM-SYNTH-PROTOCOL-0001-003",
                "DRAFT",
                {
                    "campaign_id": "CAM-SYNTH-PROTOCOL-0001",
                    "question_id": "Q-CAM-SYNTH-PROTOCOL-0001-01",
                    "evidence_ids": ["SRC-SYNTH-0001"],
                    "functional_rule": "Contradicted finding",
                    "fidelity_classification": "REFERENCE_HYPOTHESIS",
                    "evidence_floor": "SIMPLE_BOUNDED",
                    "evidence_axes": {
                        "O": 3,
                        "R": 3,
                        "T": 3,
                        "A": 3,
                        "C": 3,
                    },
                    "evidence_route_tags": [
                        "CONTROLLED_OBSERVATION",
                        "STATIC_ANALYSIS",
                        "INDEPENDENT_REPRODUCTION",
                    ],
                    "rule_domains": ["DISCRETE"],
                    "contradiction_resolution": "Unresolved material contradiction",
                    "applicability": "Synthetic only",
                },
            )
        )
        contradicted = self.append(transition_spec(contradicted, "CORROBORATED"))
        contradicted = self.append(
            transition_spec(contradicted, "REVIEWED", author="reviewer-b")
        )
        with self.assertRaisesRegex(registry.RegistryError, "C2/C3"):
            self.append(
                transition_spec(contradicted, "ACCEPTED", author="reviewer-b")
            )

        inherited_dir = Path(self.temp.name) / "inherited"
        registry.initialize_registry(inherited_dir, "owner")
        inherited_finding, inherited_vector = seed_accepted_transfer_evidence(
            inherited_dir
        )
        first = registry.append_spec(
            inherited_dir,
            create_spec(
                "transfer",
                "TRN-TERRAIN-0001",
                "DRAFT",
                {
                    "traffic_light": "GREEN",
                    "destination_bead_id": "mc-synthetic",
                    "finding_ids": [inherited_finding["entity_id"]],
                    "vector_ids": [inherited_vector["entity_id"]],
                    "package_sha256": HASH_C,
                },
                classification="PUBLIC_SAFE_CANDIDATE",
            ),
        )
        second = registry.append_spec(
            inherited_dir, transition_spec(first, "IN_REVIEW")
        )
        with self.assertRaisesRegex(registry.RegistryError, "inherited analyst"):
            registry.append_spec(
                inherited_dir,
                create_spec(
                    "review",
                    "REV-TRN-TERRAIN-0001-01",
                    "RECORDED",
                    review_payload(
                        second,
                        independence="INHERITED_CONTEXT",
                        raw_context_inherited=True,
                    ),
                    author="reviewer-b",
                    classification="PUBLIC_SAFE_CANDIDATE",
                ),
            )

    def test_exact_hash_receipt_and_revocation_must_stale(self) -> None:
        approved, _, _ = self.transfer_chain()
        receipt = self.append(
            create_spec(
                "receipt",
                "RCPT-TRN-FLIGHT-0001-01",
                "CURRENT",
                {
                    "transfer_id": "TRN-FLIGHT-0001",
                    "transfer_event_hash": approved["event_hash"],
                    "destination_bead_id": "mc-synthetic",
                    "detail": self.receipt_detail(
                        "RCPT-TRN-FLIGHT-0001-01", "TRN-FLIGHT-0001"
                    ),
                },
                author="implementer-c",
                classification="PUBLIC_SAFE_CANDIDATE",
            )
        )
        consumed = self.append(transition_spec(approved, "CONSUMED"))
        registry.validate_events(self.events())
        with self.assertRaisesRegex(registry.RegistryError, "current for stale"):
            self.append(transition_spec(consumed, "REVOKED"))

        stale, revoked = registry.append_specs(
            self.registry_dir,
            [
                transition_spec(receipt, "STALE_REVOKED"),
                transition_spec(consumed, "REVOKED"),
            ],
        )
        self.assertEqual(stale["status"], "STALE_REVOKED")
        self.assertEqual(revoked["status"], "REVOKED")

    def test_receipt_binds_latest_hash_and_destination(self) -> None:
        approved, _, _ = self.transfer_chain()
        with self.assertRaisesRegex(registry.RegistryError, "destination mismatch"):
            self.append(
                create_spec(
                    "receipt",
                    "RCPT-TRN-FLIGHT-0001-01",
                    "CURRENT",
                    {
                        "transfer_id": approved["entity_id"],
                        "transfer_event_hash": approved["event_hash"],
                        "destination_bead_id": "mc-wrong",
                        "detail": self.receipt_detail(
                            "RCPT-TRN-FLIGHT-0001-01", approved["entity_id"]
                        ),
                    },
                    author="implementer-c",
                    classification="PUBLIC_SAFE_CANDIDATE",
                )
            )
        with self.assertRaisesRegex(registry.RegistryError, "immutable"):
            self.append(
                link_spec(
                    approved,
                    "FND-CAM-SYNTH-PROTOCOL-0001-001",
                    "rejects",
                )
            )
        receipt = self.append(
            create_spec(
                "receipt",
                "RCPT-TRN-FLIGHT-0001-02",
                "CURRENT",
                {
                    "transfer_id": approved["entity_id"],
                    "transfer_event_hash": approved["event_hash"],
                    "destination_bead_id": "mc-synthetic",
                    "detail": self.receipt_detail(
                        "RCPT-TRN-FLIGHT-0001-02", approved["entity_id"]
                    ),
                },
                author="implementer-c",
                classification="PUBLIC_SAFE_CANDIDATE",
            )
        )
        consumed = self.append(transition_spec(approved, "CONSUMED"))
        with self.assertRaisesRegex(registry.RegistryError, "exact latest"):
            self.append(
                create_spec(
                    "receipt",
                    "RCPT-TRN-FLIGHT-0001-03",
                    "CURRENT",
                    {
                        "transfer_id": approved["entity_id"],
                        "transfer_event_hash": approved["event_hash"],
                        "destination_bead_id": "mc-synthetic",
                        "detail": self.receipt_detail(
                            "RCPT-TRN-FLIGHT-0001-03", approved["entity_id"]
                        ),
                    },
                    author="implementer-c",
                    classification="PUBLIC_SAFE_CANDIDATE",
                )
            )
        self.assertEqual(receipt["status"], "CURRENT")
        self.assertEqual(consumed["status"], "CONSUMED")

    def test_semantic_transfer_dependencies_invalidate_consumers(self) -> None:
        approved, _, _ = self.transfer_chain()
        receipt = self.append(
            create_spec(
                "receipt",
                "RCPT-TRN-FLIGHT-0001-01",
                "CURRENT",
                {
                    "transfer_id": approved["entity_id"],
                    "transfer_event_hash": approved["event_hash"],
                    "destination_bead_id": "mc-synthetic",
                    "detail": self.receipt_detail(
                        "RCPT-TRN-FLIGHT-0001-01", approved["entity_id"]
                    ),
                },
                author="implementer-c",
                classification="PUBLIC_SAFE_CANDIDATE",
            )
        )
        consumed = self.append(transition_spec(approved, "CONSUMED"))
        finding = registry.entity_state(self.events())[
            "FND-CAM-SYNTH-PROTOCOL-0001-001"
        ]
        vector = registry.entity_state(self.events())[
            "VEC-CAM-SYNTH-PROTOCOL-0001-001"
        ]
        with self.assertRaisesRegex(
            registry.RegistryError, "stale after invalidation|blocked/revoked/retested"
        ):
            self.append(transition_spec(finding, "REVOKED"))
        stale, revoked_transfer, revoked_vector, revoked_finding = (
            registry.append_specs(
            self.registry_dir,
            [
                transition_spec(receipt, "STALE_REVOKED"),
                transition_spec(consumed, "REVOKED"),
                transition_spec(vector, "REVOKED"),
                transition_spec(finding, "REVOKED"),
            ],
            )
        )
        self.assertEqual(stale["status"], "STALE_REVOKED")
        self.assertEqual(revoked_transfer["status"], "REVOKED")
        self.assertEqual(revoked_vector["status"], "REVOKED")
        self.assertEqual(revoked_finding["status"], "REVOKED")

    def test_invalid_batch_is_not_partially_appended(self) -> None:
        campaign = seed_campaign(self.registry_dir)
        before = (self.registry_dir / registry.EVENTS_NAME).read_bytes()
        with self.assertRaisesRegex(registry.RegistryError, "invalid transition"):
            registry.append_specs(
                self.registry_dir,
                [
                    create_spec(
                        "finding",
                        "FND-CAM-SYNTH-PROTOCOL-0001-002",
                        "DRAFT",
                        {
                            "campaign_id": "CAM-SYNTH-PROTOCOL-0001",
                            "question_id": "Q-CAM-SYNTH-PROTOCOL-0001-01",
                            "evidence_ids": ["SRC-SYNTH-0001"],
                            "functional_rule": "Synthetic rule",
                            "fidelity_classification": "REFERENCE_HYPOTHESIS",
                            "evidence_floor": "SIMPLE_BOUNDED",
                            "evidence_axes": {
                                "O": 1,
                                "R": 0,
                                "T": 0,
                                "A": 1,
                                "C": 0,
                            },
                            "evidence_route_tags": ["ORIGINAL_DESIGN"],
                            "rule_domains": ["DISCRETE"],
                            "contradiction_resolution": "NONE",
                            "applicability": "Synthetic only",
                        },
                    ),
                    transition_spec(campaign, "OPEN"),
                ],
            )
        self.assertEqual(
            (self.registry_dir / registry.EVENTS_NAME).read_bytes(), before
        )

    def test_staged_commit_failure_preserves_authoritative_stream(self) -> None:
        self.source()
        before = (self.registry_dir / registry.EVENTS_NAME).read_bytes()
        with mock.patch.object(
            registry.os, "fsync", side_effect=OSError("injected fsync failure")
        ):
            with self.assertRaisesRegex(OSError, "injected fsync failure"):
                self.append(
                    create_spec(
                        "build_manifest",
                        "BLD-SYNTH-0001",
                        "FROZEN",
                        {
                            "source_ids": ["SRC-SYNTH-0001"],
                            "artifact_hashes": {"inner.exe": HASH_B},
                        },
                    )
                )
        self.assertEqual(
            (self.registry_dir / registry.EVENTS_NAME).read_bytes(), before
        )
        self.assertFalse(
            any(
                path.name.endswith(".staged")
                for path in self.registry_dir.iterdir()
            )
        )

    def test_registry_root_symlink_is_rejected(self) -> None:
        alias = Path(self.temp.name) / "registry-alias"
        alias.symlink_to(self.registry_dir, target_is_directory=True)
        with self.assertRaisesRegex(registry.RegistryError, "symlink component"):
            registry.validate_storage(alias)

    def test_invalidation_fanout_is_fail_closed(self) -> None:
        seed_campaign(self.registry_dir)
        hypothesis = self.append(
            create_spec(
                "hypothesis",
                "HYP-CAM-SYNTH-PROTOCOL-0001-001",
                "OPEN",
                {
                    "campaign_id": "CAM-SYNTH-PROTOCOL-0001",
                    "question_id": "Q-CAM-SYNTH-PROTOCOL-0001-01",
                    "statement": "Synthetic hypothesis",
                    "alternative": "Synthetic alternative",
                    "falsifier": "Synthetic falsifier",
                    "evidence_axes": {"O": 0, "R": 0, "T": 0, "A": 0, "C": 0},
                    "evidence_ids": ["SRC-SYNTH-0001"],
                    "evidence_route_tags": ["CONTROLLED_OBSERVATION"],
                    "applicability": "Synthetic only",
                },
            )
        )
        finding = self.append(
            create_spec(
                "finding",
                "FND-CAM-SYNTH-PROTOCOL-0001-001",
                "DRAFT",
                {
                    "campaign_id": "CAM-SYNTH-PROTOCOL-0001",
                    "question_id": "Q-CAM-SYNTH-PROTOCOL-0001-01",
                    "evidence_ids": ["SRC-SYNTH-0001"],
                    "functional_rule": "Synthetic rule",
                    "fidelity_classification": "REFERENCE_HYPOTHESIS",
                    "evidence_floor": "SIMPLE_BOUNDED",
                    "evidence_axes": {"O": 1, "R": 0, "T": 0, "A": 1, "C": 0},
                    "evidence_route_tags": ["CONTROLLED_OBSERVATION"],
                    "rule_domains": ["DISCRETE"],
                    "contradiction_resolution": "NONE",
                    "applicability": "Synthetic only",
                },
            )
        )
        hypothesis = self.append(
            link_spec(hypothesis, finding["entity_id"], "supports")
        )
        with self.assertRaisesRegex(
            registry.RegistryError, "blocked/revoked/retested"
        ):
            self.append(transition_spec(hypothesis, "FALSIFIED"))

        finding = self.append(transition_spec(finding, "CONTRADICTED"))
        falsified = self.append(transition_spec(hypothesis, "FALSIFIED"))
        self.assertEqual(falsified["status"], "FALSIFIED")
        self.assertEqual(finding["status"], "CONTRADICTED")

    def test_semantic_dependency_and_affected_id_fanout_are_complete(self) -> None:
        seed_campaign(self.registry_dir)
        source = registry.entity_state(self.events())["SRC-SYNTH-0001"]
        with self.assertRaisesRegex(
            registry.RegistryError, "blocked/revoked/retested"
        ):
            self.append(transition_spec(source, "REVOKED"))

        finding = self.append(
            create_spec(
                "finding",
                "FND-CAM-SYNTH-PROTOCOL-0001-001",
                "DRAFT",
                {
                    "campaign_id": "CAM-SYNTH-PROTOCOL-0001",
                    "question_id": "Q-CAM-SYNTH-PROTOCOL-0001-01",
                    "evidence_ids": ["SRC-SYNTH-0001"],
                    "functional_rule": "Synthetic rule",
                    "fidelity_classification": "REFERENCE_HYPOTHESIS",
                    "evidence_floor": "SIMPLE_BOUNDED",
                    "evidence_axes": {
                        "O": 1,
                        "R": 0,
                        "T": 0,
                        "A": 1,
                        "C": 0,
                    },
                    "evidence_route_tags": ["CONTROLLED_OBSERVATION"],
                    "rule_domains": ["DISCRETE"],
                    "contradiction_resolution": "NONE",
                    "applicability": "Synthetic only",
                },
            )
        )
        contradiction_spec = create_spec(
            "contradiction",
            "CTR-CAM-SYNTH-PROTOCOL-0001-001",
            "OPEN_BLOCKING",
            {
                "campaign_id": "CAM-SYNTH-PROTOCOL-0001",
                "affected_ids": [finding["entity_id"]],
                "disposition": "PENDING",
            },
        )
        with self.assertRaisesRegex(
            registry.RegistryError, "blocked/revoked/retested"
        ):
            self.append(contradiction_spec)
        contradiction, contradicted = registry.append_specs(
            self.registry_dir,
            [
                contradiction_spec,
                transition_spec(finding, "CONTRADICTED"),
            ],
        )
        self.assertEqual(contradiction["status"], "OPEN_BLOCKING")
        self.assertEqual(contradicted["status"], "CONTRADICTED")

    def test_restore_preserves_root_hash(self) -> None:
        self.build()
        events_path = self.registry_dir / registry.EVENTS_NAME
        original = events_path.read_bytes()
        original_root = self.events()[-1]["event_hash"]

        restored_dir = Path(self.temp.name) / "restored"
        registry.initialize_registry(restored_dir, "restore-owner")
        restored_events = restored_dir / registry.EVENTS_NAME
        restored_events.write_bytes(original)
        os.chmod(restored_events, 0o600)
        loaded = registry.load_events(restored_events)
        registry.validate_events(loaded)
        self.assertEqual(loaded[-1]["event_hash"], original_root)

    def test_noncanonical_jsonl_is_rejected(self) -> None:
        self.source()
        path = self.registry_dir / registry.EVENTS_NAME
        value = json.loads(path.read_text(encoding="ascii"))
        path.write_text(
            json.dumps(value, sort_keys=False, separators=(", ", ": ")) + "\n",
            encoding="ascii",
        )
        os.chmod(path, 0o600)
        with self.assertRaisesRegex(registry.RegistryError, "not canonical"):
            registry.load_events(path)


if __name__ == "__main__":
    unittest.main()
