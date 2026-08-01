#!/usr/bin/env python3
from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
import verify_mode_contract  # noqa: E402


class ModeContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.contract = verify_mode_contract.load_contract(ROOT / "mode-contract.json")

    def test_contract_is_internally_consistent(self) -> None:
        self.assertEqual(verify_mode_contract.validate_contract(self.contract), [])

    def test_valid_mode_profiles(self) -> None:
        self.assertEqual(
            verify_mode_contract.validate_invocation(self.contract, mode="exact", internal_width=320, internal_height=200),
            [],
        )
        self.assertEqual(
            verify_mode_contract.validate_invocation(self.contract, mode="classic-hd", internal_width=640, internal_height=480),
            [],
        )
        self.assertEqual(verify_mode_contract.validate_invocation(self.contract, mode="unity-enhanced"), [])

    def test_invalid_combinations_are_rejected(self) -> None:
        cases = (
            {"mode": "off"},
            {"mode": "unity-enhanced", "exact_claim": True},
            {"mode": "exact", "widescreen_world": True},
            {"mode": "classic-hd", "internal_width": 1920, "internal_height": 1080},
            {"mode": "exact", "pipeline_switch": True},
            {"mode": "not-a-mode"},
        )
        for case in cases:
            with self.subTest(case=case):
                self.assertTrue(verify_mode_contract.validate_invocation(self.contract, **case))

    def test_owner_inputs_and_corpus_are_mode_scoped(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            missing = Path(directory) / "missing-owner-cache"
            errors = verify_mode_contract.validate_invocation(
                self.contract, mode="classic-hd", owner_inputs=missing
            )
            self.assertTrue(any("owner inputs" in error for error in errors))
        errors = verify_mode_contract.validate_invocation(
            self.contract,
            mode="unity-enhanced",
            corpus_root=self.contract["corpus_policy"]["exact_root"],
        )
        self.assertTrue(any("corpus root" in error for error in errors))

    def test_contract_is_json_and_does_not_claim_persistent_saves(self) -> None:
        with (ROOT / "mode-contract.json").open(encoding="utf-8") as handle:
            contract = json.load(handle)
        self.assertFalse(contract["invariants"]["persistent_save_support"])
        self.assertNotIn("research/private/remc2-unity-native", contract["modes"]["classic-hd"]["project"])


if __name__ == "__main__":
    unittest.main()
