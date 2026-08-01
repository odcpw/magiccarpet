#!/usr/bin/env python3
"""Mutation-negative tests for the Level 2 profile guard."""

from __future__ import annotations

import copy
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
WORKSPACE = TOOLS.parent
sys.path.insert(0, str(TOOLS))

import verify_level2_profile as guard  # noqa: E402


class Level2ProfileGuardTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.profile = guard.load_json(guard.PROFILE_PATH)

    def run_cli(self, profile: dict) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory(prefix="level2-profile-test-") as directory:
            path = Path(directory) / "profile.json"
            path.write_text(json.dumps(profile, indent=2) + "\n", encoding="utf-8")
            return subprocess.run(
                [sys.executable, str(TOOLS / "verify_level2_profile.py"), "--profile", str(path), "--no-current-tests", "--json"],
                cwd=WORKSPACE,
                check=False,
                capture_output=True,
                text=True,
            )

    def test_canonical_profile_and_sealed_history_pass_without_regeneration(self) -> None:
        result = guard.verify(run_tests=False)
        self.assertTrue(result["ok"])
        self.assertEqual(result["profile"], "Level 2 / external 2 / internal 1 / Day")
        self.assertTrue(result["historical_receipts"]["immutable"])
        self.assertEqual(result["current_checks"]["status"], "NOT_RUN")

    def test_external_level_mutation_is_rejected(self) -> None:
        profile = copy.deepcopy(self.profile)
        profile["profile"]["external_level"] = 3
        result = self.run_cli(profile)
        self.assertEqual(result.returncode, 1)
        self.assertIn("profile values changed", result.stdout)

    def test_map_type_mutation_is_rejected(self) -> None:
        profile = copy.deepcopy(self.profile)
        profile["profile"]["map_type"]["serialized_value"] = 1
        result = self.run_cli(profile)
        self.assertEqual(result.returncode, 1)
        self.assertIn("profile values changed", result.stdout)

    def test_sealed_matrix_hash_mutation_is_rejected(self) -> None:
        profile = copy.deepcopy(self.profile)
        profile["sealed_level1"]["historical_receipts"]["matrix"]["sha256"] = "0" * 64
        result = self.run_cli(profile)
        self.assertEqual(result.returncode, 1)
        self.assertIn("sealed Level 1 matrix hash changed", result.stdout)

    def test_owner_package_manifest_hash_mutation_is_rejected(self) -> None:
        profile = copy.deepcopy(self.profile)
        profile["sealed_level1"]["historical_receipts"]["owner_package"]["file_sha256"] = "0" * 64
        result = self.run_cli(profile)
        self.assertEqual(result.returncode, 1)
        self.assertIn("sealed owner package manifest hash changed", result.stdout)

    def test_asset_closure_hash_mutation_is_rejected(self) -> None:
        profile = copy.deepcopy(self.profile)
        profile["sealed_level1"]["historical_receipts"]["asset_closure"]["file_sha256"] = "0" * 64
        result = self.run_cli(profile)
        self.assertEqual(result.returncode, 1)
        self.assertIn("sealed asset closure hash changed", result.stdout)

    def test_abi_header_hash_mutation_is_rejected(self) -> None:
        profile = copy.deepcopy(self.profile)
        profile["sealed_level1"]["historical_receipts"]["abi"]["header_sha256"] = "0" * 64
        result = self.run_cli(profile)
        self.assertEqual(result.returncode, 1)
        self.assertIn("sealed native ABI header hash changed", result.stdout)

    def test_abi_event_code_witness_mutation_is_rejected(self) -> None:
        profile = copy.deepcopy(self.profile)
        profile["sealed_level1"]["historical_receipts"]["abi"]["event_code_range"]["last"] = 39
        result = self.run_cli(profile)
        self.assertEqual(result.returncode, 1)
        self.assertIn("sealed native ABI event-code range changed", result.stdout)

    def test_nonclaim_and_broadening_rules_are_required(self) -> None:
        profile = copy.deepcopy(self.profile)
        profile["scope"]["nonclaims"] = ["Level 2 is complete."]
        result = self.run_cli(profile)
        self.assertEqual(result.returncode, 1)
        self.assertIn("scope must state the Level 2 nonclaim", result.stdout)

    def test_source_pin_is_checked_against_checkout_not_profile_only(self) -> None:
        profile = copy.deepcopy(self.profile)
        profile["specification"]["remc2_commit"] = "0" * 40
        result = self.run_cli(profile)
        self.assertEqual(result.returncode, 1)
        self.assertIn("profile specification is not pinned", result.stdout)


if __name__ == "__main__":
    unittest.main()
