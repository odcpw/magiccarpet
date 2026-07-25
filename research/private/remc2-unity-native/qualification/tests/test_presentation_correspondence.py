#!/usr/bin/env python3
"""Negative-claim tests for Level 1 presentation correspondence."""

from __future__ import annotations

import copy
import importlib.util
import json
import unittest
from pathlib import Path


QUALIFICATION = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "verify_presentation_correspondence",
    QUALIFICATION / "verify_presentation_correspondence.py",
)
assert SPEC and SPEC.loader
VERIFIER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFIER)
CONTRACT = json.loads(
    (QUALIFICATION / "presentation-correspondence.json").read_text()
)


class PresentationContractTests(unittest.TestCase):
    def test_registered_contract_passes(self) -> None:
        VERIFIER.verify_contract(copy.deepcopy(CONTRACT))

    def test_cross_timeline_pixel_equality_is_rejected(self) -> None:
        mutated = copy.deepcopy(CONTRACT)
        mutated["comparisons"][0]["comparison_policy"] = "pixel-equality"
        with self.assertRaisesRegex(SystemExit, "cross-timeline equivalence"):
            VERIFIER.verify_contract(mutated)

    def test_cross_timeline_checkpoint_pair_is_rejected(self) -> None:
        mutated = copy.deepcopy(CONTRACT)
        mutated["comparisons"][0]["checkpoint_pairs"] = [
            {"left_clock": 0, "right_clock": 0}
        ]
        with self.assertRaisesRegex(SystemExit, "cross-timeline equivalence"):
            VERIFIER.verify_contract(mutated)

    def test_base_paln_review_decode_exact_rgb_claim_is_rejected(self) -> None:
        mutated = copy.deepcopy(CONTRACT)
        layer = next(
            row for row in mutated["layers"]
            if row["id"] == "original.review_decode.base_paln"
        )
        layer["classification"] = "exact"
        layer["exact_domain"] = "displayed RGB"
        layer["exact_against"] = "original active DAC"
        with self.assertRaisesRegex(SystemExit, "classification mismatch"):
            VERIFIER.verify_contract(mutated)

    def test_source_hotspot_exact_claim_is_rejected(self) -> None:
        mutated = copy.deepcopy(CONTRACT)
        layer = next(
            row for row in mutated["layers"]
            if row["id"] == "sprite.source_hotspot_anchor"
        )
        layer["classification"] = "exact"
        layer["present"] = True
        layer["exact_domain"] = "sprite hotspot"
        layer["exact_against"] = "owner cache"
        with self.assertRaisesRegex(SystemExit, "classification mismatch"):
            VERIFIER.verify_contract(mutated)

    def test_cross_domain_exact_target_is_rejected_and_counted(self) -> None:
        mutated = copy.deepcopy(CONTRACT)
        layer = next(
            row for row in mutated["layers"] if row["id"] == "unity.route.pngs"
        )
        layer["exact_against"] = "original remc2 framebuffer pixels"
        layer["not_claimed"] = []
        pixel_claims, clock_claims = VERIFIER.cross_timeline_claim_counts(mutated)
        self.assertEqual((pixel_claims, clock_claims), (1, 0))
        with self.assertRaisesRegex(SystemExit, "exact target crosses"):
            VERIFIER.verify_contract(mutated)

    def test_exact_cross_timeline_disclaimers_are_required(self) -> None:
        mutated = copy.deepcopy(CONTRACT)
        layer = next(
            row for row in mutated["layers"] if row["id"] == "unity.route.pngs"
        )
        layer["not_claimed"] = []
        with self.assertRaisesRegex(SystemExit, "claim limits are incomplete"):
            VERIFIER.verify_contract(mutated)

    def test_pairing_rule_equal_clock_claim_is_rejected_and_counted(self) -> None:
        mutated = copy.deepcopy(CONTRACT)
        mutated["pairing"]["rule"] = "Unity tick 145 equals remc2 frame 0"
        pixel_claims, clock_claims = VERIFIER.cross_timeline_claim_counts(mutated)
        self.assertEqual((pixel_claims, clock_claims), (0, 1))
        with self.assertRaisesRegex(SystemExit, "one-to-one"):
            VERIFIER.verify_contract(mutated)

    def test_layer_authority_domain_substitution_is_rejected(self) -> None:
        mutated = copy.deepcopy(CONTRACT)
        layer = next(
            row for row in mutated["layers"]
            if row["id"] == "original.framebuffer.indexed"
        )
        layer["authority_domain"] = "unity-synthetic-route"
        with self.assertRaisesRegex(SystemExit, "authority domain mismatch"):
            VERIFIER.verify_contract(mutated)

    def test_omitted_proxy_cannot_be_present(self) -> None:
        mutated = copy.deepcopy(CONTRACT)
        layer = next(
            row for row in mutated["layers"]
            if row["id"] == "projectile.debug_proxy"
        )
        layer["present"] = True
        with self.assertRaisesRegex(SystemExit, "omitted requires"):
            VERIFIER.verify_contract(mutated)

    def test_unknown_layer_is_rejected(self) -> None:
        mutated = copy.deepcopy(CONTRACT)
        mutated["layers"].append({
            "id": "unknown.presentation",
            "classification": "adapted",
            "present": True,
            "authority_domain": "unity-synthetic-route",
            "transform": "unknown",
            "not_equivalent_to": ["everything"],
            "evidence": [],
            "comparison_policy": "structural-correspondence",
            "not_claimed": [],
        })
        with self.assertRaisesRegex(SystemExit, "incomplete or duplicated"):
            VERIFIER.verify_contract(mutated)


if __name__ == "__main__":
    unittest.main()
