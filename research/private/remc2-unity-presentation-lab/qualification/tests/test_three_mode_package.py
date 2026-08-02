import copy
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


MATRIX_SPEC = importlib.util.spec_from_file_location(
    "validate_classic_hd_matrix",
    ROOT / "qualification" / "validate_classic_hd_matrix.py",
)
assert MATRIX_SPEC and MATRIX_SPEC.loader
MATRIX = importlib.util.module_from_spec(MATRIX_SPEC)
MATRIX_SPEC.loader.exec_module(MATRIX)


class ThreeModePackageTests(unittest.TestCase):
    def test_package_integrity_is_green_but_open_gates_are_explicit(self):
        summary = PACKAGE.verify()
        self.assertEqual("PASS", summary["status"], summary)
        self.assertEqual("open-gates", summary["qualification_state"])
        self.assertTrue(summary["open_gates"])
        route_gates = [gate for gate in summary["open_gates"] if "remaining owner route cases" in gate]
        self.assertEqual([], route_gates)
        self.assertEqual([], PACKAGE.validate_classic_hd_owner_completion_artifact())

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

    def test_owner_visual_captures_match_receipts_without_promoting_review(self):
        self.assertEqual([], PACKAGE.validate_classic_hd_owner_visual_capture_artifact())
        self.assertEqual([], PACKAGE.validate_unity_enhanced_owner_visual_capture_artifact())

    def test_resolution_matrix_records_rejection_without_leaving_budget_gate_open(self):
        self.assertEqual([], PACKAGE.validate_classic_hd_owner_performance_artifact())
        summary = PACKAGE.verify()
        self.assertFalse(any("performance remains over" in gate for gate in summary["open_gates"]))

    def test_owner_route_matrix_split_does_not_promote_gpu_timing(self):
        matrix = json.loads(
            (ROOT / "qualification" / "classic-hd-resolution-matrix.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual(
            "owner-route-qualified; gpu-timing-pending",
            matrix["unity_gpu_and_owner_route"],
        )
        self.assertEqual("not-captured", matrix["owner_route_gpu_timing"])
        summary = PACKAGE.verify()
        self.assertFalse(any("owner/GPU matrix remains pending" in gate for gate in summary["open_gates"]))
        self.assertTrue(any("GPU timing remains open" in gate for gate in summary["open_gates"]))

    def test_owner_route_matrix_split_fails_closed_when_gpu_status_is_missing(self):
        matrix_path = ROOT / "qualification" / "classic-hd-resolution-matrix.json"
        matrix = json.loads(matrix_path.read_text(encoding="utf-8"))
        candidate = copy.deepcopy(matrix)
        candidate.pop("owner_route_gpu_timing")
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "classic-hd-resolution-matrix.json"
            path.write_text(json.dumps(candidate), encoding="utf-8")
            errors = MATRIX.validate(path)
        self.assertIn(
            "qualified owner route must keep GPU timing explicitly not-captured",
            errors,
        )

    def test_os_window_lifecycle_records_both_real_player_cases_and_stays_open_for_review(self):
        self.assertEqual([], PACKAGE.validate_classic_hd_owner_os_window_lifecycle_artifact())
        self.assertEqual([], PACKAGE.validate_classic_hd_os_window_lifecycle_tooling())
        windowed = json.loads(
            (ROOT / "qualification" / "classic-hd-owner-os-window-lifecycle.json").read_text(
                encoding="utf-8"
            )
        )
        fullscreen = json.loads(
            (
                ROOT
                / "qualification"
                / "classic-hd-owner-os-window-lifecycle-fullscreen-window.json"
            ).read_text(encoding="utf-8")
        )
        self.assertEqual(["windowed"], windowed["captured_external_cases"])
        self.assertTrue(fullscreen["external_focus_transition_observed"])
        self.assertTrue(fullscreen["external_focus_return_observed"])
        self.assertTrue(fullscreen["post_regain_output_stable"])

    def test_standalone_player_build_path_is_documented(self):
        self.assertEqual([], PACKAGE.validate_standalone_player_build_tooling())


if __name__ == "__main__":
    unittest.main()
