import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONFIG = ROOT / "presentation-config.json"
sys.path.insert(0, str(ROOT))

from presentation_launcher import LauncherConfigError, build_command, load_config  # noqa: E402


class PresentationLauncherTests(unittest.TestCase):
    def test_config_defaults_to_exact(self):
        config = load_config(CONFIG)
        self.assertEqual(config["default_mode"], "exact")
        command = build_command(config, CONFIG)
        self.assertEqual(command[0], "Unity")
        self.assertIn("remc2-unity-native", command[2])
        self.assertNotIn("-mc-presentation-mode", command)

    def test_classic_mode_adds_only_external_owner_inputs(self):
        config = load_config(CONFIG)
        command = build_command(
            config,
            CONFIG,
            "classic-hd",
            "/owner/package",
            "/owner/closure.json",
            "/owner/cache",
        )
        self.assertIn("-mc-presentation-mode", command)
        self.assertIn("classic-hd", command)
        self.assertIn("-mc-package", command)
        self.assertNotIn("owner-data", " ".join(command))

    def test_optional_mode_requires_all_owner_inputs(self):
        config = load_config(CONFIG)
        with self.assertRaises(LauncherConfigError):
            build_command(config, CONFIG, "unity-enhanced", "/owner/package", None, "/owner/cache")

    def test_optional_mode_rejects_relative_owner_path(self):
        config = load_config(CONFIG)
        with self.assertRaises(LauncherConfigError):
            build_command(config, CONFIG, "classic-hd", "package", "/owner/closure", "/owner/cache")

    def test_config_round_trips_without_changing_mode_contract(self):
        config = load_config(CONFIG)
        with tempfile.TemporaryDirectory() as directory:
            copied = Path(directory) / "presentation-config.json"
            copied.write_text(json.dumps(config, indent=2), encoding="utf-8")
            round_tripped = load_config(copied)
        self.assertEqual(round_tripped["default_mode"], "exact")
        self.assertEqual(set(round_tripped["modes"]), {"exact", "classic-hd", "unity-enhanced"})

    def test_exact_cannot_become_optional_by_config_edit(self):
        config = load_config(CONFIG)
        config["modes"]["exact"]["arguments"] = ["-mc-presentation-mode", "classic-hd"]
        with self.assertRaises(LauncherConfigError):
            build_command(config, CONFIG)


if __name__ == "__main__":
    unittest.main()
