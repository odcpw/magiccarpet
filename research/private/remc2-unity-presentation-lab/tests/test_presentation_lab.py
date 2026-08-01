import json
from pathlib import Path
import subprocess
import sys
import unittest


ROOT = Path(__file__).resolve().parents[1]
CHECK = ROOT / "tools" / "verify_presentation_lab.py"


class PresentationLabGuardTests(unittest.TestCase):
    def run_guard(self, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(CHECK), "--json", *args],
            cwd=ROOT.parents[3],
            text=True,
            capture_output=True,
            check=False,
        )

    def test_owner_free_scaffold_passes(self) -> None:
        result = self.run_guard()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        payload = json.loads(result.stdout)
        self.assertEqual(payload["status"], "pass")
        self.assertFalse(payload["runtime_editor_verified"])

    def test_wrong_root_fails(self) -> None:
        result = self.run_guard("--root", "/tmp/does-not-contain-a-presentation-lab")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(json.loads(result.stdout)["status"], "fail")


if __name__ == "__main__":
    unittest.main()
