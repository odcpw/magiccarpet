"""Build/install regressions; fixtures are not live rendering evidence."""

import contextlib
import io
import json
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


SOURCE = Path(__file__).resolve().parents[2]


class PlayerLauncherTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="mc-launcher-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / "unity project"
        self.root.mkdir()
        for script in ("run-player.sh", "build-player.sh"):
            shutil.copy2(SOURCE / script, self.root / script)
        for directory in ("Assets/Scenes", "Packages", "ProjectSettings"):
            (self.root / directory).mkdir(parents=True)
        (self.root / "Assets/Scenes/Main.unity").write_text("authored scene\n")
        (self.root / "ProjectSettings/ProjectVersion.txt").write_text("m_EditorVersion: 6000.3.20f1\n")
        self.header = self.root.parent / "remc2-sim-rs/crates/sim-capi/include/mc_sim.h"
        self.header.parent.mkdir(parents=True)
        self.header.write_text("unsigned mc_native_abi_version(void);\n")
        self.binary = self.root / "Builds/Linux/MagicCarpetNative.x86_64"
        self.binary.parent.mkdir(parents=True)
        self.binary.write_text(
            "#!/usr/bin/env python3\nimport json,sys\nprint(json.dumps(sys.argv[1:]))\n"
        )
        self.binary.chmod(0o700)
        # Load helpers without running the build command.
        code = (self.root / "build-player.sh").read_text().split("<<'PY'\n", 1)[1].rsplit("\nPY", 1)[0]
        helper = Path(self.temporary.name) / "build_helpers.py"
        helper.write_text(code)
        with patch.object(sys, "argv", ["-", str(self.root)]):
            self.api = runpy.run_path(str(helper), run_name="build_helpers")
        self.plugin = self.binary.parent / self.api["PLUGIN"]
        self.plugin.parent.mkdir(parents=True)
        self.plugin.write_bytes(b"fixture plugin, not executable native code")
        asset = self.root / "Assets/Plugins/x86_64/libsim_capi.so"
        asset.parent.mkdir(parents=True)
        shutil.copy2(self.plugin, asset)
        self.install_receipt()

    def install_receipt(self):
        self.api["write_receipt"](self.binary.parent, {
            "schema": "mc-player-install/v1",
            "unity_inputs": self.api["unity_inputs"](),
            "native_contract": self.api["native_contract"](),
            "player_files": self.api["payload"](self.binary.parent),
            "native_sha256": self.api["digest"](self.plugin),
        })

    def launch(self, *arguments, backend="default", display=""):
        return subprocess.run(
            ["sh", str(self.root / "run-player.sh"), *arguments],
            env={**os.environ, "MC_WINDOW_BACKEND": backend, "WAYLAND_DISPLAY": display},
            text=True, capture_output=True, check=False, timeout=10,
        )

    def test_backend_selection_preserves_player_arguments(self):
        arguments = ["-logFile", "a path with spaces.log", "-mc-game-runtime", "--help"]
        cases = [
            ("auto", "wayland-0", ["-force-wayland"]),
            ("auto", "", []),
            ("default", "wayland-0", []),
            ("wayland", "", ["-force-wayland"]),
        ]
        for backend, display, prefix in cases:
            with self.subTest(backend=backend, display=display):
                result = self.launch(*arguments, backend=backend, display=display)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(json.loads(result.stdout.splitlines()[-1]), prefix + arguments)
        invalid = self.launch(*arguments, backend="invalid")
        self.assertEqual(invalid.returncode, 2)
        self.assertEqual(invalid.stdout, "")
        self.assertIn("MC_WINDOW_BACKEND", invalid.stderr)

    def test_missing_and_tampered_install_are_rejected(self):
        self.plugin.write_bytes(b"different plugin")
        result = self.launch()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("installed native plugin changed", result.stderr)
        self.assertIn("full build", result.stderr)
        self.assertNotIn("Native plugin SHA256", result.stdout)
        (self.binary.parent / "build-install.json").rename(self.root / "saved-install.json")
        result = self.launch()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Missing or incompatible", result.stderr)

    def test_native_refresh_rejects_changed_unity_inputs_and_contract_before_cargo(self):
        for path in (self.root / "Assets/Scenes/Main.unity", self.header):
            with self.subTest(path=path):
                original = path.read_bytes()
                path.write_bytes(original + b"changed\n")
                result = subprocess.run([str(self.root / "build-player.sh"), "--native-only"],
                                        capture_output=True, text=True, check=False, timeout=10)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("native contract changed", result.stderr)
                self.assertEqual(self.plugin.read_bytes(), b"fixture plugin, not executable native code")
                path.write_bytes(original)
        # Rust artifact bytes alone do not invalidate the Unity input identity.
        asset = self.root / "Assets/Plugins/x86_64/libsim_capi.so"
        asset.parent.mkdir(parents=True, exist_ok=True)
        asset.write_bytes(b"Rust-only artifact")
        self.api["check_install"]()

    def test_launch_holds_install_lock_until_player_exits(self):
        self.binary.write_text("#!/bin/sh\necho ready\nexec sleep 30\n")
        self.install_receipt()
        player = subprocess.Popen(["sh", str(self.root / "run-player.sh")], stdout=subprocess.PIPE,
                                  stderr=subprocess.PIPE, text=True,
                                  env={**os.environ, "MC_WINDOW_BACKEND": "default"})
        try:
            self.assertIn("Native plugin SHA256", player.stdout.readline())
            self.assertEqual(player.stdout.readline().strip(), "ready")
            result = subprocess.run([str(self.root / "build-player.sh"), "--native-only"],
                                    text=True, capture_output=True, check=False, timeout=10)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Player or another build is active", result.stderr)
        finally:
            player.terminate()
            player.communicate(timeout=5)

    def test_command_failure_keeps_log_and_existing_player(self):
        before = self.api["payload"](self.binary.parent)
        log = self.root / "failure.log"
        with contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(RuntimeError, "Command failed.*Read"):
                self.api["command"](["sh", "-c", "echo concrete failure >&2; exit 17"], self.root, log)
        self.assertIn("concrete failure", log.read_text())
        self.assertEqual(before, self.api["payload"](self.binary.parent))


if __name__ == "__main__":
    unittest.main()
