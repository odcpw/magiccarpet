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
CORRECTION6_CACHE = (
    QUALIFICATION.parents[1]
    / "generated/level1-unity-asset-cache-plan28-cp4-correction6"
)


class PresentationContractTests(unittest.TestCase):
    def test_registered_contract_passes(self) -> None:
        VERIFIER.verify_contract(copy.deepcopy(CONTRACT))

    def test_native_final_main_composed_gate_binds_qualified_cache(self) -> None:
        contract = copy.deepcopy(CONTRACT)
        VERIFIER.verify_contract(contract)
        VERIFIER.verify_native_final_main_cache(contract, CORRECTION6_CACHE)
        VERIFIER.verify_no_qualified_placeholders(contract, CORRECTION6_CACHE)

    def test_native_final_main_cache_hash_substitutions_are_rejected(self) -> None:
        mutations = {
            "manifest_sha256": "manifest identity",
            "content_sha256": "content/count mismatch",
            "source_frame_census_sha256": "census identity mismatch",
        }
        for field, message in mutations.items():
            with self.subTest(field=field):
                mutated = copy.deepcopy(CONTRACT)
                mutated["native_final_main"]["cache"][field] = "0" * 64
                with self.assertRaisesRegex(SystemExit, message):
                    VERIFIER.verify_native_final_main_cache(
                        mutated, CORRECTION6_CACHE
                    )

    def test_native_final_main_shape_valid_unknown_rust_test_is_rejected(self) -> None:
        mutated = copy.deepcopy(CONTRACT)
        mutated["native_final_main"]["components"]["capi"]["tests"][0] = (
            "native_v2::tests::shape_valid_but_nonexistent"
        )
        with self.assertRaisesRegex(SystemExit, "capi test binding does not resolve"):
            VERIFIER.verify_contract(mutated)

    def test_native_final_main_shape_valid_unknown_unity_test_is_rejected(self) -> None:
        mutated = copy.deepcopy(CONTRACT)
        mutated["native_final_main"]["components"]["unity_one_plane"]["tests"][0] = (
            "MagicCarpet.Native.Tests.NativeSessionEditModeTests."
            "ShapeValidButNonexistent"
        )
        with self.assertRaisesRegex(
            SystemExit, "unity_one_plane test binding does not resolve"
        ):
            VERIFIER.verify_contract(mutated)

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

    def test_source_hotspot_exact_target_cannot_be_substituted(self) -> None:
        mutated = copy.deepcopy(CONTRACT)
        layer = next(
            row for row in mutated["layers"]
            if row["id"] == "sprite.source_hotspot_anchor"
        )
        layer["exact_against"] = "controlled gallery"
        with self.assertRaisesRegex(SystemExit, "exact target crosses"):
            VERIFIER.verify_contract(mutated)

    def test_native_final_main_source_anchor_cannot_move_symbols(self) -> None:
        mutated = copy.deepcopy(CONTRACT)
        mutated["native_final_main"]["source_anchors"][0]["symbols"].append(
            "DrawGameFrame_2BE30"
        )
        with self.assertRaisesRegex(SystemExit, "pinned source anchors"):
            VERIFIER.verify_contract(mutated)

    def test_native_final_main_requires_every_component(self) -> None:
        for component in VERIFIER.NATIVE_FINAL_MAIN_COMPONENTS:
            with self.subTest(component=component):
                mutated = copy.deepcopy(CONTRACT)
                del mutated["native_final_main"]["components"][component]
                with self.assertRaisesRegex(SystemExit, "component coverage"):
                    VERIFIER.verify_contract(mutated)

    def test_native_final_main_requires_complete_and_forbids_omission(self) -> None:
        mutated = copy.deepcopy(CONTRACT)
        mutated["native_final_main"]["complete_receipt"]["required_flags"].remove(
            "MC_PRESENTATION_FRAME_LIGHTING_COMPLETE"
        )
        with self.assertRaisesRegex(SystemExit, "COMPLETE/atomicity"):
            VERIFIER.verify_contract(mutated)
        mutated = copy.deepcopy(CONTRACT)
        mutated["native_final_main"]["complete_receipt"]["forbidden_flags"] = []
        with self.assertRaisesRegex(SystemExit, "COMPLETE/atomicity"):
            VERIFIER.verify_contract(mutated)

    def test_native_final_main_requires_atomic_failure_semantics(self) -> None:
        mutated = copy.deepcopy(CONTRACT)
        mutated["native_final_main"]["complete_receipt"]["failure_semantics"] = (
            "may commit partial output"
        )
        with self.assertRaisesRegex(SystemExit, "COMPLETE/atomicity"):
            VERIFIER.verify_contract(mutated)

    def test_native_final_main_requires_unity_one_plane_ownership(self) -> None:
        mutated = copy.deepcopy(CONTRACT)
        mutated["native_final_main"]["unity_one_plane"][
            "managed_duplicate_submissions"
        ] = 1
        with self.assertRaisesRegex(SystemExit, "one-plane"):
            VERIFIER.verify_contract(mutated)

    def test_native_final_main_requires_profile_exclusions(self) -> None:
        mutated = copy.deepcopy(CONTRACT)
        mutated["native_final_main"]["explicit_exclusions"].remove(
            "hostile or remote player presentation"
        )
        with self.assertRaisesRegex(SystemExit, "explicit exclusions"):
            VERIFIER.verify_contract(mutated)

    def test_native_final_main_cache_identity_is_paired(self) -> None:
        mutated = copy.deepcopy(CONTRACT)
        mutated["native_final_main"]["cache"]["output_count"] = 1247
        with self.assertRaisesRegex(SystemExit, "owner-cache identity"):
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
