#!/usr/bin/env python3
"""Direct mutation-negative tests for the active Level 2 runtime profile.

The filename is retained for command compatibility with the former generic
matrix-profile suite.  These tests intentionally do not load a census,
denominator, generated matrix, or Beads registry; those artifacts are
historical/reference evidence rather than ordinary Level 2 development proof.
"""

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

import verify_level2_profile as guard


class RuntimeProfileTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.profile = guard.load_json(guard.PROFILE_PATH)

    def run_cli(self, profile: dict, *, run_runtime_tests: bool = False) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory(prefix="level2-runtime-profile-") as directory:
            path = Path(directory) / "profile.json"
            path.write_text(json.dumps(profile, indent=2) + "\n", encoding="utf-8")
            command = [
                sys.executable,
                str(TOOLS / "verify_level2_profile.py"),
                "--profile",
                str(path),
                "--json",
            ]
            if not run_runtime_tests:
                command.append("--no-runtime-tests")
            return subprocess.run(
                command,
                cwd=WORKSPACE,
                check=False,
                capture_output=True,
                text=True,
                timeout=300,
            )

    def assert_profile_rejected(self, profile: dict, message: str) -> None:
        result = self.run_cli(profile)
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(message, result.stdout + result.stderr)

    def test_runtime_profile_accepts_without_archival_inputs(self) -> None:
        profile = copy.deepcopy(self.profile)
        profile.pop("specification", None)
        profile.pop("scope", None)
        profile.pop("sealed_level1", None)
        result = self.run_cli(profile)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        report = json.loads(result.stdout)
        self.assertTrue(report["ok"])
        self.assertEqual(report["runtime_checks"]["status"], "NOT_RUN")

    def test_wrong_external_level_is_rejected(self) -> None:
        profile = copy.deepcopy(self.profile)
        profile["profile"]["external_level"] = 3
        self.assert_profile_rejected(profile, "external level")

    def test_wrong_internal_level_mapping_is_rejected(self) -> None:
        profile = copy.deepcopy(self.profile)
        profile["profile"]["internal_level_index"] = 0
        self.assert_profile_rejected(profile, "internal level index")

    def test_wrong_day_name_is_rejected(self) -> None:
        profile = copy.deepcopy(self.profile)
        profile["profile"]["map_type"]["name"] = "Night"
        self.assert_profile_rejected(profile, "map type name")

    def test_wrong_day_serialized_value_is_rejected(self) -> None:
        profile = copy.deepcopy(self.profile)
        profile["profile"]["map_type"]["serialized_value"] = 1
        self.assert_profile_rejected(profile, "Day map serialized value")

    def test_enabled_fallback_is_rejected(self) -> None:
        profile = copy.deepcopy(self.profile)
        profile["profile"]["fallback_enabled"] = True
        self.assert_profile_rejected(profile, "fallback_enabled")

    def test_historical_tracker_and_matrix_metadata_are_not_runtime_inputs(self) -> None:
        profile = copy.deepcopy(self.profile)
        profile["sealed_level1"]["historical_receipts"]["matrix"]["sha256"] = "0" * 64
        profile["sealed_level1"]["historical_receipts"]["matrix"]["rows"] = -1
        profile["sealed_level1"]["current_checks"]["commands"] = ["edited tracker history"]
        profile["beads"] = {"mc-gsqrl.4": {"description": "edited historical tracker text"}}
        result = self.run_cli(profile)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_direct_runtime_check_is_a_nonempty_selector_test(self) -> None:
        check_id, command = guard.DIRECT_RUNTIME_CHECKS[0]
        self.assertEqual(check_id, "sim-core-profile-selection")
        self.assertIn("profile_selection_is_typed_and_does_not_fallback", command)
        self.assertGreater(guard._test_count("running 1 test"), 0)
        self.assertEqual(guard._test_count("running 0 tests"), 0)


if __name__ == "__main__":
    unittest.main()
