import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from verify_xr_smoke import validate


class XrSmokeTests(unittest.TestCase):
    def test_profiles_and_actions_are_static_and_owner_free(self) -> None:
        result = validate()
        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["profiles"], ["flat-disabled", "pc-openxr"])
        self.assertFalse(result["runtime_verified"])
        self.assertEqual(result["build_method"], "BuildOpenXrSmokePlayer")


if __name__ == "__main__":
    unittest.main()
