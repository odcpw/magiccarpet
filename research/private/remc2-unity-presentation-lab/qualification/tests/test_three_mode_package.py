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

    def test_candidate_profile_smoke_rejects_promoted_status(self):
        artifact_path = ROOT / "qualification" / "classic-hd-owner-route-960x720-smoke.json"
        artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
        artifact["qualification_status"] = "qualified"
        with tempfile.TemporaryDirectory() as temporary:
            candidate = Path(temporary) / "profile.json"
            candidate.write_text(json.dumps(artifact), encoding="utf-8")
            errors = PACKAGE.validate_profile_smoke_artifact(candidate, "classic-hd-960x720", 960, 720)
        self.assertIn("profile smoke must keep sustained qualification pending/open", errors)

    def test_profile_sequence_failure_must_remain_unpromoted(self):
        artifact_path = ROOT / "qualification" / "classic-hd-owner-route-960x720-sequence-failure.json"
        artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
        artifact["qualification_status"] = "qualified"
        with tempfile.TemporaryDirectory() as temporary:
            candidate = Path(temporary) / "profile-sequence-failure.json"
            candidate.write_text(json.dumps(artifact), encoding="utf-8")
            errors = PACKAGE.validate_profile_sequence_failure_artifact(candidate)
        self.assertIn("profile sequence failure artifact must remain explicitly unpromoted/open", errors)

    def test_visual_capture_tooling_is_present_without_promoting_corpus(self):
        self.assertEqual([], PACKAGE.validate_classic_hd_visual_capture_tooling())
        self.assertEqual([], PACKAGE.validate_unity_enhanced_visual_capture_tooling())


if __name__ == "__main__":
    unittest.main()
