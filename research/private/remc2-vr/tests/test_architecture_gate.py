import json
from pathlib import Path
import subprocess
import sys
import unittest


ROOT = Path(__file__).resolve().parents[4]
CHECK = Path(__file__).resolve().parents[1] / "validate_architecture_gate.py"


class ArchitectureGateTests(unittest.TestCase):
    def run_check(self, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(CHECK), "--json", *args],
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=False,
        )

    def test_gate_and_exact_negative_scan_pass(self) -> None:
        result = self.run_check()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        payload = json.loads(result.stdout)
        self.assertEqual(payload["status"], "pass")
        self.assertEqual(payload["exact_negative_scan"]["status"], "pass")
        self.assertEqual(payload["runtime_qualification"], "pending-editor-headset-build")

    def test_missing_root_fails(self) -> None:
        result = self.run_check("--root", "/tmp/no-magic-carpet-root")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(json.loads(result.stdout)["status"], "fail")


if __name__ == "__main__":
    unittest.main()
