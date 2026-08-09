import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "verify_unity_enhanced.py"
SPEC = importlib.util.spec_from_file_location("verify_unity_enhanced", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class UnityEnhancedVerificationTests(unittest.TestCase):
    def test_enhanced_lifecycle_shell_is_green(self):
        summary = MODULE.verify()
        self.assertEqual("PASS", summary["status"], summary)

    def test_enhanced_source_stays_outside_exact_and_voxel_paths(self):
        summary = MODULE.verify()
        forbidden = [check for check in summary["checks"] if check["name"] == "no gameplay or renderer-boundary imports"]
        self.assertEqual(1, len(forbidden))
        self.assertTrue(forbidden[0]["passed"], forbidden[0])


if __name__ == "__main__":
    unittest.main()
