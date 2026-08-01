import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "verify_presentation_lab.py"
SPEC = importlib.util.spec_from_file_location("verify_presentation_lab", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class PresentationLabVerificationTests(unittest.TestCase):
    def test_project_boundary_and_urp_shell_are_green(self):
        summary = MODULE.verify()
        self.assertEqual("PASS", summary["status"], summary)

    def test_lab_is_not_the_exact_project(self):
        self.assertNotEqual(MODULE.PROJECT_ROOT, MODULE.EXACT_ROOT)
        self.assertTrue(MODULE.EXACT_ROOT.is_dir())


if __name__ == "__main__":
    unittest.main()
