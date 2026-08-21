#!/usr/bin/env python3
"""Conformance and planted-failure tests for the CF-00 Omega seal."""

from __future__ import annotations

import copy
import hashlib
import json
import sys
import unittest
from pathlib import Path


HERE = Path(__file__).resolve().parent
if str(HERE) not in sys.path:
    sys.path.insert(0, str(HERE))

from build_campaign_fold_omega import (  # noqa: E402
    BASELINE_M1_JSON_SHA256,
    BASELINE_OMEGA_JSON_SHA256,
    BASELINE_SCHEMA_VERSION,
    EXPECTED_TRANSITION_OPEN_QUESTION_DELTAS,
    OBSERVATION_ALPHABET,
    REQUIRED_CAPABILITIES,
    SCHEMA_VERSION,
    STALE_M1_HASHES,
    validate_baseline_inventory,
    validate_inventory,
)


RECEIPT_V1 = HERE / "campaign-fold-omega-v1.json"
RECEIPT_V1_1 = HERE / "campaign-fold-omega-v1.1.json"
EXPECTED_V1_EVIDENCE_REVISION = "462b438d41f1d39ce4b4a1ae24f892dc3e04cafe"
EXPECTED_V1_1_M1_JSON_SHA256 = "7986b9a2935d160f40c22fae1250f7bfa9abe9ede81529832ed35ce148470383"
EXPECTED_V1_1_M1_MARKDOWN_SHA256 = "91b385359a70c78ab72ad011e0f6f5de827f334e1dcbe50283c1deb8565fc6ce"


def load_receipt(path: Path = RECEIPT_V1_1) -> dict:
    with path.open(encoding="utf-8") as stream:
        return json.load(stream)


def current_revision(receipt: dict) -> str:
    revision = receipt["evidence"]["superproject_revision"]
    assert revision == receipt["transition_inventory"]["regeneration"]["evidence_revision"]
    assert revision == receipt["transition_inventory"]["artifact"]["inputs"]["rust_revision"]
    return revision


class CampaignFoldOmegaTests(unittest.TestCase):
    def test_v1_audit_receipt_is_immutable(self) -> None:
        receipt = load_receipt(RECEIPT_V1)
        validate_baseline_inventory(receipt, expected_revision=EXPECTED_V1_EVIDENCE_REVISION)
        self.assertEqual(receipt["schema_version"], BASELINE_SCHEMA_VERSION)
        self.assertEqual(
            hashlib.sha256(RECEIPT_V1.read_bytes()).hexdigest(),
            BASELINE_OMEGA_JSON_SHA256,
        )
        regeneration = receipt["transition_inventory"]["regeneration"]
        self.assertEqual(regeneration["run_a"]["json_sha256"], BASELINE_M1_JSON_SHA256)
        self.assertEqual(len(receipt["transition_inventory"]["artifact"]["open_questions"]), 224)

    def test_v1_1_receipt_is_current_and_bounded(self) -> None:
        receipt = load_receipt()
        revision = current_revision(receipt)
        validate_inventory(receipt, expected_revision=revision)
        self.assertEqual(receipt["schema_version"], SCHEMA_VERSION)
        self.assertEqual(receipt["bead_id"], "mc-cf00-omega-reseal-3dxle")
        regeneration = receipt["transition_inventory"]["regeneration"]
        self.assertEqual(regeneration["run_a"]["json_sha256"], EXPECTED_V1_1_M1_JSON_SHA256)
        self.assertEqual(regeneration["run_a"]["markdown_sha256"], EXPECTED_V1_1_M1_MARKDOWN_SHA256)
        self.assertNotEqual(regeneration["run_a"]["label"], regeneration["run_b"]["label"])
        self.assertEqual(regeneration["run_a"]["json_sha256"], regeneration["run_b"]["json_sha256"])
        self.assertEqual(regeneration["run_a"]["markdown_sha256"], regeneration["run_b"]["markdown_sha256"])
        artifact = receipt["transition_inventory"]["artifact"]
        self.assertEqual(len(artifact["transitions"]), 54)
        self.assertEqual(len(artifact["route"]["route_kinds"]), 14)
        self.assertEqual(len(artifact["open_questions"]), 229)
        delta = receipt["v1_to_v1_1_delta"]
        self.assertEqual(delta["baseline_open_question_count"], 224)
        self.assertEqual(delta["current_open_question_count"], 229)
        self.assertEqual(delta["added_open_question_count"], 5)
        self.assertEqual(delta["unmapped_open_question_count"], 0)
        self.assertEqual(delta["transition_deltas"], list(EXPECTED_TRANSITION_OPEN_QUESTION_DELTAS))
        self.assertEqual(
            [level["external_level"] for level in receipt["remaining_realm_census"]["levels"]],
            list(range(3, 25)),
        )

    def test_every_omega_row_names_producer_and_following_consumer(self) -> None:
        receipt = load_receipt()
        for row in receipt["observation_alphabet"]:
            self.assertTrue(row["required"])
            self.assertTrue(row["producer"])
            self.assertTrue(row["following_pass_consumer"])
        self.assertEqual(
            [row["id"] for row in receipt["observation_alphabet"]],
            list(OBSERVATION_ALPHABET),
        )
        self.assertEqual(
            [row["id"] for row in receipt["capabilities"]],
            list(REQUIRED_CAPABILITIES),
        )
        for row in receipt["transition_receipts"]["receipts"]:
            self.assertRegex(row["commit"], r"^[0-9a-f]{40}$")
            self.assertTrue(row["ordinary_producer"])
            self.assertTrue(row["following_pass_consumer"])

    def test_every_new_question_has_named_landed_introduction_and_relocation(self) -> None:
        receipt = load_receipt()
        for row in receipt["v1_to_v1_1_delta"]["rows"]:
            self.assertRegex(row["introduced_by"]["commit"], r"^[0-9a-f]{40}$")
            self.assertTrue(row["introduced_by"]["bead_id"])
            self.assertTrue(row["introduced_by"]["present_at_evidence_revision"])
            self.assertRegex(row["relocated_by"]["commit"], r"^[0-9a-f]{40}$")
            self.assertEqual(
                row["relocated_by"]["bead_id"],
                "mc-cf01-pure-campaign-module-extraction-19t39",
            )
            self.assertTrue(row["relocated_by"]["present_at_evidence_revision"])

    def test_missing_state_cell_is_rejected(self) -> None:
        receipt = load_receipt()
        candidate = copy.deepcopy(receipt)
        candidate["transition_inventory"]["artifact"]["transitions"][0]["cells"] = []
        with self.assertRaisesRegex(ValueError, "state cell"):
            validate_inventory(candidate, expected_revision=current_revision(receipt))

    def test_missing_transition_is_rejected(self) -> None:
        receipt = load_receipt()
        candidate = copy.deepcopy(receipt)
        candidate["transition_inventory"]["artifact"]["transitions"].pop(10)
        with self.assertRaisesRegex(ValueError, "transition denominator"):
            validate_inventory(candidate, expected_revision=current_revision(receipt))

    def test_dropped_or_reordered_ordered_effect_is_rejected(self) -> None:
        receipt = load_receipt()
        candidate = copy.deepcopy(receipt)
        transition = next(
            row
            for row in candidate["transition_inventory"]["artifact"]["transitions"]
            if len(row["emissions"]) >= 2
        )
        transition["emissions"].reverse()
        with self.assertRaisesRegex(ValueError, "ordered effect/audio"):
            validate_inventory(candidate, expected_revision=current_revision(receipt))

    def test_missing_required_capability_is_rejected(self) -> None:
        receipt = load_receipt()
        candidate = copy.deepcopy(receipt)
        candidate["capabilities"].pop()
        with self.assertRaisesRegex(ValueError, "required capability"):
            validate_inventory(candidate, expected_revision=current_revision(receipt))

    def test_changed_evidence_revision_is_rejected(self) -> None:
        receipt = load_receipt()
        with self.assertRaisesRegex(ValueError, "evidence revision"):
            validate_inventory(receipt, expected_revision="0" * 40)

    def test_trace_on_difference_is_rejected(self) -> None:
        receipt = load_receipt()
        candidate = copy.deepcopy(receipt)
        candidate["trace_noninterference"]["trace_enabled_receipt"].pop()
        with self.assertRaisesRegex(ValueError, "trace"):
            validate_inventory(candidate, expected_revision=current_revision(receipt))

    def test_known_stale_m1_hash_is_rejected(self) -> None:
        receipt = load_receipt()
        candidate = copy.deepcopy(receipt)
        stale = sorted(STALE_M1_HASHES)[0]
        candidate["transition_inventory"]["regeneration"]["run_a"]["json_sha256"] = stale
        candidate["transition_inventory"]["regeneration"]["run_b"]["json_sha256"] = stale
        with self.assertRaisesRegex(ValueError, "stale M1"):
            validate_inventory(candidate, expected_revision=current_revision(receipt))

    def test_unmapped_open_question_drift_is_rejected(self) -> None:
        receipt = load_receipt()
        candidate = copy.deepcopy(receipt)
        candidate["transition_inventory"]["artifact"]["transitions"][0]["open_questions"].append(
            "unmapped planted open question"
        )
        with self.assertRaisesRegex(ValueError, "unmapped open-question drift"):
            validate_inventory(candidate, expected_revision=current_revision(receipt))


if __name__ == "__main__":
    unittest.main(verbosity=2)
