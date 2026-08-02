import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2].parent / "remc2-presentation"
MODULE_PATH = ROOT / "qualification" / "verify_three_mode_package.py"


def load_module():
    spec = importlib.util.spec_from_file_location("verify_three_mode_package", MODULE_PATH)
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


PACKAGE = load_module()


class ThreeModePackageTests(unittest.TestCase):
    def test_package_integrity_is_green_but_open_gates_are_explicit(self):
        summary = PACKAGE.verify()
        self.assertEqual("PASS", summary["status"], summary)
        self.assertEqual("open-gates", summary["qualification_state"])
        self.assertTrue(summary["open_gates"])

    def test_sequence_artifact_rejects_false_replay(self):
        artifact_path = ROOT / "qualification" / "classic-hd-owner-route-sequence-smoke.json"
        artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
        artifact["reset_replay_match"] = False
        with tempfile.TemporaryDirectory() as temporary:
            candidate = Path(temporary) / "sequence.json"
            candidate.write_text(json.dumps(artifact), encoding="utf-8")
            errors = PACKAGE.validate_sequence_artifact(candidate)
        self.assertIn("sequence reset replay did not match", errors)


if __name__ == "__main__":
    unittest.main()
