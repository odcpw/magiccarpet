import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


CAPTURE = load("capture_manifest", ROOT / "capture_manifest.py")
VERIFY = load("verify_presentation_corpora", ROOT / "verify_presentation_corpora.py")


class PresentationCorpusTests(unittest.TestCase):
    def test_dry_run_corpora_are_green(self):
        summary = VERIFY.verify()
        self.assertEqual("PASS", summary["status"], summary)

    def test_manifest_builder_embeds_mode_profile_and_build_metadata(self):
        manifest = CAPTURE.build_manifest(
            mode="unity-enhanced",
            profile="level1-night-unity-enhanced-v1",
            source_commit="abc123",
            build_id="lab-build-1",
            unity_editor="6000.3.20f1",
            gpu="test-gpu",
            driver="test-driver",
        )
        self.assertEqual("unity-enhanced", manifest["mode"])
        self.assertEqual("level1-night-unity-enhanced-v1", manifest["profile"])
        self.assertEqual("lab-build-1", manifest["build_metadata"]["build_id"])
        self.assertTrue(manifest["build_metadata"]["metadata_complete"])

    def test_corpus_crossover_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "unity-enhanced" / "manifest.json"
            manifest = CAPTURE.build_manifest("classic-hd", "level1-night-classic-hd-4x3")
            with self.assertRaises(CAPTURE.ManifestError):
                CAPTURE.write_dry_run_manifest(output, manifest, "classic-hd")

    def test_existing_cross_mode_manifest_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "manifest.json"
            output.write_text(json.dumps({"mode": "exact", "corpus_directory": "exact"}), encoding="utf-8")
            original_root = CAPTURE.CORPORA_ROOT
            try:
                CAPTURE.CORPORA_ROOT = root
                with self.assertRaises(CAPTURE.ManifestError):
                    CAPTURE.write_dry_run_manifest(output, CAPTURE.build_manifest("classic-hd", "level1-night-classic-hd-4x3"), "classic-hd")
            finally:
                CAPTURE.CORPORA_ROOT = original_root


if __name__ == "__main__":
    unittest.main()
