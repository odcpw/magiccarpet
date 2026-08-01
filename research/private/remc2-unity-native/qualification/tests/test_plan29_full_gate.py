from __future__ import annotations

import copy
import importlib.util
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

TOOL = Path(__file__).resolve().parents[1] / "run_plan29_full_gate.py"
SPEC = importlib.util.spec_from_file_location("plan29_gate", TOOL)
assert SPEC and SPEC.loader
gate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gate)


def run_receipt(label: str) -> dict:
    digests = {
        key: "1" * 64
        for key in (
            "reset", "victory", "failure", "restart", "replay",
            "observation_mask", "unity_checkpoints", "full_run",
        )
    }
    receipt = {
        "schema": "mc2-plan29-clean-run/v1",
        "run_id": label,
        "run_root": f"/private/{label}",
        "clean": {
            "cargo_target_isolated": True,
            "unity_library_isolated": True,
            "cache_isolated": True,
            "preexisting_artifacts_rejected": True,
        },
        "inputs": {"cache_manifest": "2" * 64},
        "gates": {key: "PASS" for key in gate.REQUIRED_GATES},
        "identity": {},
        "semantic_digests": digests,
        "status": "PASS",
    }
    receipt["receipt_sha256"] = gate.sha256_bytes(gate.canonical_json(receipt))
    return receipt


def aggregate() -> dict:
    runs = [run_receipt("run-a")]
    return {
        "schema": "mc2-plan29-full-gate/v1",
        "qualification_policy_sha256": gate.EXPECTED_POLICY_SHA256,
        "exact_run_count": 1,
        "runs": runs,
        "run_count_policy": {
            "required_runs": 1,
            "completed_runs": 1,
            "run_b_required": False,
            "run_b_permitted": False,
        },
        "status": "PASS",
    }


class Plan29FullGateTests(unittest.TestCase):
    def test_exact_one_aggregate_passes(self) -> None:
        gate.validate_aggregate(aggregate())

    def test_zero_or_two_runs_fail(self) -> None:
        for count in (0, 2):
            document = aggregate()
            document["exact_run_count"] = count
            document["runs"] = [run_receipt(f"run-{index}") for index in range(count)]
            with self.subTest(count=count), self.assertRaises(gate.GateError):
                gate.validate_aggregate(document)

    def test_missing_scenario_or_wrong_policy_fails(self) -> None:
        missing = aggregate()
        del missing["runs"][0]["gates"]["failure"]
        missing["runs"][0]["receipt_sha256"] = gate.sha256_bytes(
            gate.canonical_json({key: value for key, value in missing["runs"][0].items()
                                 if key != "receipt_sha256"})
        )
        with self.assertRaises(gate.GateError):
            gate.validate_aggregate(missing)
        divergent = aggregate()
        divergent["qualification_policy_sha256"] = "0" * 64
        with self.assertRaises(gate.GateError):
            gate.validate_aggregate(divergent)

    def test_forbidden_original_binary_commands_fail(self) -> None:
        for command in (["DOSBox", "game.exe"], ["run", "NETHERW.EXE"], ["ghidra"]):
            with self.subTest(command=command), self.assertRaises(gate.GateError):
                gate.validate_command_allowlist(command)

    def test_cli_rejects_run_counts_other_than_one_before_execution(self) -> None:
        for count in (0, 2):
            with tempfile.TemporaryDirectory() as directory:
                output = Path(directory) / "out"
                result = subprocess.run(
                    [sys.executable, str(TOOL), "--exact-runs", str(count), "--output-root", str(output)],
                    text=True,
                    capture_output=True,
                )
                self.assertEqual(result.returncode, 2)
                self.assertIn("must be exactly 1", result.stderr)
                self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main(verbosity=2)
