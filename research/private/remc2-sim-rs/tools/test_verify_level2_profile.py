#!/usr/bin/env python3
"""Mutation-negative tests for the Level 2 profile guard."""

from __future__ import annotations

import copy
import hashlib
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

    def test_level1_header_stays_sealed_and_level2_header_is_additive(self) -> None:
        abi = self.profile["sealed_level1"]["historical_receipts"]["abi"]
        level1_header = WORKSPACE / abi["header_path"]
        self.assertEqual(
            hashlib.sha256(level1_header.read_bytes()).hexdigest(),
            "f822e3d343517469775ae53546ec827bd392ad37f866aec3d48bddc0504a3d6f",
        )
        level1_text = level1_header.read_text(encoding="utf-8")
        level2_only = (
            "McCampaignSelectionV2",
            "McCampaignStepInputV2",
            "McCampaignEventV2",
            "McCampaignStepSnapshotV2",
            "McCampaignWorldInfoV2",
            "McCampaignPlayerSnapshotV2",
            "McCampaignEntitySnapshotV2",
            "McCampaignEconomyEventV2",
            "mc_native_campaign_session_open",
            "mc_native_campaign_session_destroy",
            "mc_native_campaign_session_reset",
            "mc_native_campaign_session_step",
            "mc_native_campaign_world_info",
            "mc_native_campaign_player_count",
            "mc_native_campaign_objective_count",
            "mc_native_campaign_entity_count",
            "mc_native_campaign_players_copy",
            "mc_native_campaign_objectives_copy",
            "mc_native_campaign_entities_copy",
            "mc_native_campaign_economy_events",
        )
        for symbol in level2_only:
            self.assertNotIn(symbol, level1_text)

        level2_header = level1_header.with_name("mc_sim_campaign.h")
        level2_text = level2_header.read_text(encoding="utf-8")
        for symbol in level2_only:
            self.assertIn(symbol, level2_text)

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
