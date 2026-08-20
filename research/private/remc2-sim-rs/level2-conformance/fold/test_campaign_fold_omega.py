#!/usr/bin/env python3
"""Conformance and planted-failure tests for the CF-00 Omega seal."""

from __future__ import annotations

import copy
import json
import sys
import unittest
from pathlib import Path


HERE = Path(__file__).resolve().parent
if str(HERE) not in sys.path:
    sys.path.insert(0, str(HERE))

from build_campaign_fold_omega import (  # noqa: E402
    OBSERVATION_ALPHABET,
    REQUIRED_CAPABILITIES,
    SCHEMA_VERSION,
    STALE_M1_HASHES,
    validate_inventory,
)


RECEIPT = HERE / "campaign-fold-omega-v1.json"
EXPECTED_EVIDENCE_REVISION = "462b438d41f1d39ce4b4a1ae24f892dc3e04cafe"
EXPECTED_M1_SHA256 = "aad97175330bfdef21b09e3e164f054d2b6df59f66bd7c449f02e3f633514684"


def load_receipt() -> dict:
    with RECEIPT.open(encoding="utf-8") as stream:
        return json.load(stream)


class CampaignFoldOmegaTests(unittest.TestCase):
    def test_canonical_receipt_is_current_and_bounded(self) -> None:
        receipt = load_receipt()
        validate_inventory(receipt, expected_revision=EXPECTED_EVIDENCE_REVISION)
        self.assertEqual(receipt["schema_version"], SCHEMA_VERSION)
        regeneration = receipt["transition_inventory"]["regeneration"]
        self.assertEqual(regeneration["run_a"]["json_sha256"], EXPECTED_M1_SHA256)
        self.assertNotEqual(regeneration["run_a"]["label"], regeneration["run_b"]["label"])
        self.assertEqual(regeneration["run_a"]["json_sha256"], regeneration["run_b"]["json_sha256"])
        self.assertEqual(regeneration["run_a"]["markdown_sha256"], regeneration["run_b"]["markdown_sha256"])
        self.assertEqual(len(receipt["transition_inventory"]["artifact"]["transitions"]), 54)
        self.assertEqual(len(receipt["transition_inventory"]["artifact"]["route"]["route_kinds"]), 14)
        self.assertEqual(len(receipt["transition_inventory"]["artifact"]["open_questions"]), 224)
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

    def test_missing_state_cell_is_rejected(self) -> None:
        receipt = load_receipt()
        candidate = copy.deepcopy(receipt)
        candidate["transition_inventory"]["artifact"]["transitions"][0]["cells"] = []
        with self.assertRaisesRegex(ValueError, "state cell"):
            validate_inventory(candidate, expected_revision=EXPECTED_EVIDENCE_REVISION)

    def test_missing_transition_is_rejected(self) -> None:
        receipt = load_receipt()
        candidate = copy.deepcopy(receipt)
        candidate["transition_inventory"]["artifact"]["transitions"].pop(10)
        with self.assertRaisesRegex(ValueError, "transition denominator"):
            validate_inventory(candidate, expected_revision=EXPECTED_EVIDENCE_REVISION)

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
            validate_inventory(candidate, expected_revision=EXPECTED_EVIDENCE_REVISION)

    def test_missing_required_capability_is_rejected(self) -> None:
        receipt = load_receipt()
        candidate = copy.deepcopy(receipt)
        candidate["capabilities"].pop()
        with self.assertRaisesRegex(ValueError, "required capability"):
            validate_inventory(candidate, expected_revision=EXPECTED_EVIDENCE_REVISION)

    def test_changed_evidence_revision_is_rejected(self) -> None:
        receipt = load_receipt()
        with self.assertRaisesRegex(ValueError, "evidence revision"):
            validate_inventory(receipt, expected_revision="0" * 40)

    def test_trace_on_difference_is_rejected(self) -> None:
        receipt = load_receipt()
        candidate = copy.deepcopy(receipt)
        candidate["trace_noninterference"]["trace_enabled_receipt"].pop()
        with self.assertRaisesRegex(ValueError, "trace"):
            validate_inventory(candidate, expected_revision=EXPECTED_EVIDENCE_REVISION)

    def test_known_stale_m1_hash_is_rejected(self) -> None:
        receipt = load_receipt()
        candidate = copy.deepcopy(receipt)
        stale = sorted(STALE_M1_HASHES)[0]
        candidate["transition_inventory"]["regeneration"]["run_a"]["json_sha256"] = stale
        candidate["transition_inventory"]["regeneration"]["run_b"]["json_sha256"] = stale
        with self.assertRaisesRegex(ValueError, "stale M1"):
            validate_inventory(candidate, expected_revision=EXPECTED_EVIDENCE_REVISION)


if __name__ == "__main__":
    unittest.main(verbosity=2)
