"""Argument-routing checks, not live rendering evidence."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


class PlayerLauncherTests(unittest.TestCase):
    def test_backend_selection_preserves_player_arguments(self):
        source = Path(__file__).resolve().parents[2] / "run-player.sh"
        with tempfile.TemporaryDirectory(prefix="mc-launcher-") as temporary:
            root = Path(temporary)
            shutil.copy2(source, root / "run-player.sh")
            binary = root / "Builds/Linux/MagicCarpetNative.x86_64"
            binary.parent.mkdir(parents=True)
            binary.write_text(
                "#!/usr/bin/env python3\nimport json,sys\nprint(json.dumps(sys.argv[1:]))\n"
            )
            binary.chmod(0o700)
            arguments = ["-logFile", "a path with spaces.log", "-mc-game-runtime"]
            cases = [
                ("auto", "wayland-0", ["-force-wayland"]),
                ("auto", "", []),
                ("default", "wayland-0", []),
                ("wayland", "", ["-force-wayland"]),
            ]
            for backend, display, prefix in cases:
                with self.subTest(backend=backend, display=display):
                    result = subprocess.run(
                        ["sh", str(root / "run-player.sh"), *arguments],
                        env={**os.environ, "MC_WINDOW_BACKEND": backend,
                             "WAYLAND_DISPLAY": display},
                        text=True, capture_output=True, check=True,
                    )
                    self.assertEqual(json.loads(result.stdout), prefix + arguments)
            invalid = subprocess.run(
                ["sh", str(root / "run-player.sh"), *arguments],
                env={**os.environ, "MC_WINDOW_BACKEND": "invalid"},
                text=True, capture_output=True,
            )
            self.assertEqual(invalid.returncode, 2)
            self.assertEqual(invalid.stdout, "")
            self.assertIn("MC_WINDOW_BACKEND", invalid.stderr)


if __name__ == "__main__":
    unittest.main()
