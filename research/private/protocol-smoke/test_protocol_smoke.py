from __future__ import annotations

import os
import stat
import tempfile
import unittest
from pathlib import Path

import protocol_smoke


EXPECTED_NEGATIVE_CASES = {
    "P1": {
        "P1-NEG-TERMINAL-MISMATCH",
        "P1-NEG-INCOMPLETE-INVALIDATION",
    },
    "P2": {"P2-NEG-SIMPLE-FLOOR"},
    "P3": {
        "P3-NEG-HASH-MISMATCH",
        "P3-NEG-PARENT-MISMATCH",
    },
    "P4": {
        "P4-NEG-NONATOMIC-SUPERSESSION",
        "P4-NEG-OPEN-CONTRADICTION",
        "P4-NEG-C1-WITHOUT-CONTRADICTION-REVIEW",
    },
    "P5": {
        "P5-NEG-CONTAMINATED-VECTOR-ACCEPTANCE",
        "P5-NEG-SELF-REVIEW",
        "P5-NEG-INHERITED-REVIEW",
        "P5-NEG-NONATOMIC-REVOCATION",
        "P5-NEG-CONTAMINATED-TRANSFER-APPROVAL",
    },
}


class ProtocolSmokeTest(unittest.TestCase):
    def test_all_phases_match_expected_lock(self) -> None:
        protocol_smoke.verify_inputs()
        with tempfile.TemporaryDirectory(prefix="mc-protocol-smoke-", dir="/tmp") as root:
            output = Path(root) / "run"
            summary = protocol_smoke.run_all(output)
            protocol_smoke.verify_expected(summary)

    def test_repeated_runs_have_identical_roots_and_terminal_states(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mc-protocol-smoke-", dir="/tmp") as root:
            first = protocol_smoke.run_all(Path(root) / "first")
            second = protocol_smoke.run_all(Path(root) / "second")
            for phase in sorted(protocol_smoke.SCENARIOS):
                self.assertEqual(
                    first["phases"][phase]["root_hash"],
                    second["phases"][phase]["root_hash"],
                )
                self.assertEqual(
                    first["phases"][phase]["statuses"],
                    second["phases"][phase]["statuses"],
                )
                self.assertEqual(
                    first["phases"][phase]["negative_cases"],
                    second["phases"][phase]["negative_cases"],
                )

    def test_negative_matrix_is_complete_and_preserved(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mc-protocol-smoke-", dir="/tmp") as root:
            summary = protocol_smoke.run_all(Path(root) / "run")
            for phase, expected_case_ids in EXPECTED_NEGATIVE_CASES.items():
                cases = summary["phases"][phase]["negative_cases"]
                self.assertEqual(
                    {case["case_id"] for case in cases},
                    expected_case_ids,
                )
                self.assertTrue(all(case["preserved"] for case in cases))
                self.assertTrue(
                    all(len(case["events_sha256"]) == 64 for case in cases)
                )
                self.assertTrue(all(len(case["root_hash"]) == 64 for case in cases))

    def test_generated_storage_is_owner_only(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mc-protocol-smoke-", dir="/tmp") as root:
            output = Path(root) / "run"
            protocol_smoke.run_all(output)
            for path in [output, *output.rglob("*")]:
                mode = stat.S_IMODE(path.lstat().st_mode)
                if path.is_dir():
                    self.assertEqual(mode, 0o700, path)
                else:
                    self.assertEqual(mode, 0o600, path)

    def test_static_ast_table_is_exact(self) -> None:
        self.assertEqual(
            protocol_smoke.static_correlation(),
            {
                "ALPHA": {
                    "function": "alpha_handler",
                    "runtime_tag": "LIFT",
                    "value": 3,
                },
                "OMEGA": {
                    "function": "omega_handler",
                    "runtime_tag": "DROP",
                    "value": -2,
                },
            },
        )

    def test_output_outside_private_or_tmp_is_rejected_without_creation(self) -> None:
        candidate = Path("/var/tmp") / f"mc-protocol-smoke-denied-{os.getpid()}"
        self.assertFalse(candidate.exists())
        with self.assertRaisesRegex(
            protocol_smoke.ScenarioError,
            "must be under /tmp or research/private/protocol-smoke",
        ):
            protocol_smoke.prepare_output(candidate)
        self.assertFalse(candidate.exists())


if __name__ == "__main__":
    unittest.main()
