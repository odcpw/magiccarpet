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
        for script in ("run-player.sh", "build-player.sh", "run-campaign.sh", "run-modern.sh"):
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

    def test_modern_pointer_selects_qualified_project_and_preserves_arguments(self):
        generated = self.campaign_resources()
        qualified = Path(self.temporary.name) / "qualified modern project"
        qualified.mkdir()
        launcher = qualified / "run-player.sh"
        launcher.write_text("#!/bin/sh\nprintf 'QUALIFIED_PLAYER\\n'\nprintf '%s\\n' \"$@\"\n")
        launcher.chmod(0o700)
        (self.root / "Builds/Modern").symlink_to(qualified, target_is_directory=True)
        save = generated / "a path with spaces.json"
        arguments = ["--new-game", "--save", str(save), "--", "-force-vulkan"]
        original_payload = self.api["payload"](self.binary.parent)
        result = subprocess.run([str(self.root / "run-modern.sh"), *arguments],
                                text=True, capture_output=True, check=False, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        lines = result.stdout.splitlines()
        forwarded = lines[lines.index("QUALIFIED_PLAYER") + 1:]
        self.assertIn("-mc-modern", forwarded)
        self.assertEqual(forwarded[-1], "-force-vulkan")
        self.assertEqual(forwarded[forwarded.index("-mc-runtime-replay-save") + 1], str(save))
        self.assertEqual(forwarded[forwarded.index("-mc-runtime-owner-cache") + 1],
                         str(generated / "settlement-20261105-integration/owner-closed-v6/environment-complete/cache-level1"))
        self.assertEqual(original_payload, self.api["payload"](self.binary.parent))

    def test_broken_modern_pointer_fails_without_falling_back_to_original(self):
        (self.root / "Builds/Modern").symlink_to(self.root / "absent project", target_is_directory=True)
        result = subprocess.run([str(self.root / "run-modern.sh"), "--help"],
                                text=True, capture_output=True, check=False, timeout=10)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, "")
        self.assertIn("Modern installation is unavailable", result.stderr)

    def test_original_pointer_preserves_resources_and_explicit_modern_selection(self):
        generated = self.campaign_resources()
        qualified = Path(self.temporary.name) / "qualified original project"
        qualified.mkdir()
        launcher = qualified / "run-player.sh"
        launcher.write_text("#!/bin/sh\nprintf 'QUALIFIED_ORIGINAL\\n'\nprintf '%s\\n' \"$@\"\n")
        launcher.chmod(0o700)
        (self.root / "Builds/Original").symlink_to(qualified, target_is_directory=True)
        save = generated / "original save.json"
        result = subprocess.run([str(self.root / "run-campaign.sh"), "--new-game", "--save", str(save)],
                                text=True, capture_output=True, check=False, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        lines = result.stdout.splitlines()
        forwarded = lines[lines.index("QUALIFIED_ORIGINAL") + 1:]
        self.assertNotIn("-mc-modern", forwarded)
        self.assertEqual(forwarded[forwarded.index("-mc-runtime-owner-cache") + 1],
                         str(generated / "settlement-20261105-integration/owner-closed-v6/environment-complete/cache-level1"))
        # The modern selector must win even while an original pointer exists.
        self.test_modern_pointer_selects_qualified_project_and_preserves_arguments()

    def test_broken_original_pointer_fails_without_using_an_unqualified_player(self):
        self.campaign_resources()
        (self.root / "Builds/Original").symlink_to(self.root / "absent original", target_is_directory=True)
        result = subprocess.run([str(self.root / "run-campaign.sh"), "--new-game"],
                                text=True, capture_output=True, check=False, timeout=10)
        self.assertEqual(result.returncode, 64)
        self.assertIn("Missing qualified player launcher", result.stderr)
        self.assertNotIn("Native plugin SHA256", result.stdout)

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

    def test_editor_test_edits_reuse_player_but_assembly_contract_changes_do_not(self):
        tests = self.root / "Assets/Tests/EditMode"
        tests.mkdir(parents=True)
        definition = tests / "MagicCarpet.Native.EditModeTests.asmdef"
        definition.write_text(json.dumps({"includePlatforms": ["Editor"]}))
        source = tests / "ExampleTests.cs"
        source.write_text("// original Editor-only test\n")
        self.install_receipt()
        source.write_text("// changed Editor-only test\n")
        self.assertEqual(self.launch().returncode, 0)
        definition.write_text(json.dumps({"includePlatforms": []}))
        result = self.launch()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("native contract changed", result.stderr)
        # With no Editor-only contract, ordinary C# remains a player input.
        self.install_receipt()
        source.write_text("// changed runtime source\n")
        self.assertNotEqual(self.launch().returncode, 0)

    def test_command_failure_keeps_log_and_existing_player(self):
        before = self.api["payload"](self.binary.parent)
        log = self.root / "failure.log"
        with contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(RuntimeError, "Command failed.*Read"):
                self.api["command"](["sh", "-c", "echo concrete failure >&2; exit 17"], self.root, log)
        self.assertIn("concrete failure", log.read_text())
        self.assertEqual(before, self.api["payload"](self.binary.parent))

    def build_with_rust_edit(self, phase, source_root=None, native_only=False):
        sim = source_root or self.header.parents[3]
        for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml"):
            (sim / name).write_text("fixture build input\n")
        artifact = self.root / "captured-native.so"
        artifact.write_bytes(b"captured fixture native bytes")
        editor = self.root / "fixture-editor"
        editor.write_text("#!/bin/sh\nexit 0\n")
        editor.chmod(0o700)
        player_bytes = self.binary.read_bytes()
        changed = sim / "crates/after_capture.rs"
        self.build_commands = []
        self.build_directories = []

        def command(arguments, cwd, log, environment=None, diagnostic_log=None):
            self.build_commands.append(arguments[0])
            self.build_directories.append(cwd)
            if arguments[0] == "cargo":
                log.write_text(json.dumps({"reason": "compiler-artifact",
                    "target": {"name": "sim_capi"},
                    "filenames": [str(artifact.with_name("libsim_capi.so"))]}) + "\n")
                shutil.copy2(artifact, artifact.with_name("libsim_capi.so"))
                if phase == "cargo":
                    changed.write_text("Rust changed during Cargo\n")
            else:
                stage = Path(environment["MC_PLAYER_BUILD_OUTPUT"])
                stage.mkdir()
                (stage / self.binary.name).write_bytes(player_bytes)
                (stage / self.binary.name).chmod(0o700)
                plugin = stage / self.api["PLUGIN"]
                plugin.parent.mkdir(parents=True)
                shutil.copy2(artifact, plugin)
                changed.write_text("Rust changed after native capture\n")
                log.write_text("fixture Unity build\n")

        arguments = ["-", str(self.root)]
        if source_root is not None:
            arguments += ["--native-source-root", str(source_root)]
        if native_only:
            arguments += ["--native-only"]
        with patch.dict(self.api["main"].__globals__, {"command": command}), \
             patch.object(self.api["ctypes"], "CDLL") as library, \
             patch.object(sys, "argv", arguments), \
             patch.dict(os.environ, {"MC_UNITY_EDITOR": str(editor)}), \
             contextlib.redirect_stdout(io.StringIO()):
            library.return_value.mc_native_abi_version.return_value = 2
            self.api["main"]()
        return self.api["check_install"]()

    def test_completed_native_capture_allows_later_rust_edits(self):
        receipt = self.build_with_rust_edit("unity")
        self.assertEqual(len(self.build_commands), 2)
        self.assertNotIn("crates/after_capture.rs", receipt["rust_inputs"])
        self.assertIn("crates/after_capture.rs", self.api["rust_inputs"]())
        self.assertEqual(self.plugin.read_bytes(), b"captured fixture native bytes")
        self.assertEqual(receipt["native_sha256"], self.api["digest"](self.plugin))

    def test_explicit_native_source_build_capture_refresh_and_launch_share_one_root(self):
        selected = Path(self.temporary.name) / "published Rust workspace"
        header = selected / "crates/sim-capi/include/mc_sim.h"
        header.parent.mkdir(parents=True)
        header.write_text(self.header.read_text())
        source = selected / "crates/selected.rs"
        source.write_text("published source fixture\n")
        sibling = self.header.parents[3] / "crates/sibling-only.rs"
        sibling.write_text("separate Modern source fixture\n")
        receipt = self.build_with_rust_edit("unity", source_root=selected)
        self.assertEqual(self.build_directories[0], selected.resolve())
        self.assertEqual(self.build_directories[1], self.root)
        self.assertEqual(receipt["native_source_root"], str(selected.resolve()))
        self.assertEqual(receipt["native_contract"], self.api["native_contract"](selected))
        self.assertEqual(receipt["rust_inputs"]["crates/selected.rs"], self.api["digest"](source))
        self.assertNotIn("crates/sibling-only.rs", receipt["rust_inputs"])
        self.assertNotIn("crates/after_capture.rs", receipt["rust_inputs"])
        # An ordinary launch validates the recorded source's header contract;
        # the unused sibling must no longer control this Original install.
        self.header.write_text("different Modern header\n")
        self.assertEqual(self.launch().returncode, 0)
        refreshed = self.build_with_rust_edit("none", source_root=selected, native_only=True)
        self.assertEqual(self.build_commands, ["cargo"])
        self.assertEqual(self.build_directories, [selected.resolve()])
        self.assertEqual(refreshed["native_source_root"], str(selected.resolve()))
        self.assertEqual(refreshed["rust_inputs"], self.api["rust_inputs"](selected))
        # Subsequent ordinary native-only refreshes retain the recorded root,
        # even without the flag and despite the incompatible unused sibling.
        repeated = self.build_with_rust_edit("none", native_only=True)
        self.assertEqual(self.build_commands, ["cargo"])
        self.assertEqual(self.build_directories, [selected.resolve()])
        self.assertEqual(repeated["native_source_root"], str(selected.resolve()))
        self.assertEqual(repeated["rust_inputs"], self.api["rust_inputs"](selected))
        header.write_text("changed selected native contract\n")
        result = self.launch()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("native contract changed", result.stderr)

    def test_explicit_native_source_failures_preserve_installed_payload(self):
        before = self.api["payload"](self.binary.parent)
        receipt_before = (self.binary.parent / "build-install.json").read_bytes()
        selected = Path(self.temporary.name) / "selected Rust workspace"
        header = selected / "crates/sim-capi/include/mc_sim.h"
        header.parent.mkdir(parents=True)
        header.write_text(self.header.read_text())
        with self.assertRaisesRegex(RuntimeError, "Build inputs changed during Cargo"):
            self.build_with_rust_edit("cargo", source_root=selected, native_only=True)
        self.assertEqual(self.build_directories, [selected.resolve()])
        self.assertEqual(before, self.api["payload"](self.binary.parent))
        self.assertEqual(receipt_before, (self.binary.parent / "build-install.json").read_bytes())
        # A new source selection cannot bypass native-only compatibility.
        header.write_text("incompatible selected header\n")
        with self.assertRaisesRegex(RuntimeError, "native contract changed"):
            self.build_with_rust_edit("none", source_root=selected, native_only=True)
        self.assertEqual(self.build_commands, [])
        result = subprocess.run([str(self.root / "build-player.sh"), "--native-source-root", str(self.root)],
                                text=True, capture_output=True, check=False, timeout=10)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Invalid native source workspace", result.stderr)
        self.assertEqual(before, self.api["payload"](self.binary.parent))
        self.assertEqual(receipt_before, (self.binary.parent / "build-install.json").read_bytes())

    def test_rust_edits_during_cargo_are_still_rejected(self):
        before = self.api["payload"](self.binary.parent)
        with self.assertRaisesRegex(RuntimeError, "Build inputs changed during Cargo"):
            self.build_with_rust_edit("cargo")
        self.assertEqual(self.build_commands, ["cargo"])
        self.assertEqual(before, self.api["payload"](self.binary.parent))

    def campaign_resources(self):
        generated = self.root.parent / "generated"
        launcher = (self.root / "run-campaign.sh").read_text()
        python = launcher.split("<<'PY'\n", 1)[1].rsplit("\nPY", 1)[0]
        helper = Path(self.temporary.name) / "campaign_helpers.py"
        helper.write_text(python)
        with patch.object(sys, "argv", ["-", str(self.root)]):
            namespace = runpy.run_path(str(helper), run_name="campaign_helpers")
        resources = list(namespace["default_campaign_resources"]())
        packages = [row["package"] for row in resources]
        packages.extend(generated / name for name in
                        ("mc-bhq8-level1-collision-v5-night", "level3-day-block32-campaign-recipe-v2"))
        for package in packages:
            package.mkdir(parents=True, exist_ok=True)
            # Only argument routing is under test; Rust owns content validation.
            (package / "manifest.json").write_text("{}")
        for row in resources:
            for kind in ("owner", "presentation"):
                row[kind].mkdir(parents=True, exist_ok=True)
        for name in ("mc2-level1-night-cache-v2", "mc2-campaign-menu-cache-current",
                     "mc-b9s2-cursor-level1-20261029", "mc-b9s2-cursor-level2-vault-20261029",
                     "mc-b9s2-cursor-level3-20261029", "mc-b9s2-cursor-level4-20261029",
                     "level2-vault-successor-owner-cache-20261025",
                     "level3-day-spell-owner-cache-audio-v4", "mc-shared-source-map-motion-20261026",
                     "settlement-20261105-integration/owner-closed-v6/environment-complete/cache-level1",
                     "settlement-20261105-integration/owner-closed-v6/environment-complete/cache-level3",
                     "settlement-20261105-integration/owner-closed-v6/night-presentation-v6"):
            (generated / name).mkdir(parents=True, exist_ok=True)
        return generated

    def campaign(self, *arguments, environment=None):
        env = {**os.environ, "MC_WINDOW_BACKEND": "default"}
        for name in ("MC_CAMPAIGN_REPLAY", "MC_CAMPAIGN_SAVE", "MC_PLAYER_PROJECT"):
            env.pop(name, None)
        env.update(environment or {})
        return subprocess.run(["sh", str(self.root / "run-campaign.sh"), *arguments],
                              env=env, text=True, capture_output=True, check=False, timeout=10)

    def test_campaign_loading_assets_recovery_path_is_forwarded(self):
        generated = self.campaign_resources()
        missing = generated / "selected missing loading artwork"
        for option in ("--loading-assets", "-mc-runtime-loading-assets"):
            with self.subTest(option=option):
                result = self.campaign("--new-game", option, str(missing))
                self.assertEqual(result.returncode, 0, result.stderr)
                arguments = json.loads(result.stdout.splitlines()[-1])
                self.assertEqual(arguments.count("-mc-runtime-loading-assets"), 1)
                self.assertEqual(arguments[arguments.index("-mc-runtime-loading-assets") + 1],
                                 str(missing))
        self.assertFalse(missing.exists(), "the runtime owns missing-artwork recovery")

    def test_campaign_defaults_and_new_game_do_not_require_canned_replays(self):
        generated = self.campaign_resources()
        for options, menu in (([], True), (["--new-game"], False)):
            with self.subTest(options=options):
                result = self.campaign(*options)
                self.assertEqual(result.returncode, 0, result.stderr)
                arguments = json.loads(result.stdout.splitlines()[-1])
                packages = [arguments[i + 1] for i, flag in enumerate(arguments)
                            if flag == "-mc-runtime-package"]
                self.assertEqual(len(packages), 30)
                self.assertEqual(len(set(packages)), 30)
                self.assertEqual(packages[0], str(generated / "settlement-20261105-integration/owner-closed-v6/level1"))
                self.assertEqual(packages[24], str(generated / "mc-bhq10-owner-v3-level25-20261029"))
                self.assertEqual(packages[25], str(generated / "mc-paj2-source30-campaign-package"))
                self.assertEqual(packages[-1], str(generated / "mc-paj2-source-secret-34-campaign-package"))
                resource_flags = [flag for flag in arguments if flag in (
                    "-mc-runtime-package", "-mc-runtime-owner-cache", "-mc-runtime-presentation-cache")]
                self.assertEqual(resource_flags, [
                    "-mc-runtime-package", "-mc-runtime-owner-cache", "-mc-runtime-presentation-cache"] * 30)
                self.assertEqual("-mc-runtime-menu" in arguments, menu)
                self.assertNotIn("-mc-runtime-replay-load", arguments)
                self.assertEqual(arguments[arguments.index("-mc-runtime-replay-save") + 1],
                                 str(generated / "campaign-save.json"))
        self.assertFalse(list(generated.glob("opened-*.json")))

    def test_campaign_uses_installed_ending_media_unless_overridden(self):
        generated = self.campaign_resources()
        installed = generated / "mc2-campaign-media-cache-decoded"
        custom = generated / "other media"
        for cache in (installed, custom):
            cache.mkdir()
        for options, expected in (([], installed), (["--media-cache", str(custom)], custom)):
            with self.subTest(options=options):
                result = self.campaign(*options)
                self.assertEqual(result.returncode, 0, result.stderr)
                arguments = json.loads(result.stdout.splitlines()[-1])
                self.assertEqual(arguments.count("-mc-runtime-media-cache"), 1)
                self.assertEqual(arguments[arguments.index("-mc-runtime-media-cache") + 1],
                                 str(expected))

    def test_campaign_complete_override_preserves_whitespace_and_unordered_resources(self):
        generated = self.campaign_resources()
        loading = generated / "loading cache with spaces"
        loading.mkdir()
        media = generated / "original intro media"
        media.mkdir()
        menu = generated / "original campaign map"
        menu.mkdir()
        options = ["--new-game", "--loading-cache", str(loading), "--media-cache", str(media),
                   "--menu-cache", str(menu)]
        packages = []
        for index in (4, 2, 1, 3):
            package = generated / f"explicit mission {index}"
            package.mkdir()
            (package / "manifest.json").write_text("{}")
            owner, presentation = package / "owner cache", package / "presentation cache"
            owner.mkdir()
            presentation.mkdir()
            packages.append(str(package))
            options.extend(("--package", str(package), "--owner-cache", str(owner),
                            "--presentation-cache", str(presentation)))
        save = generated / "fresh player save.json"
        result = self.campaign(*options, "--save", str(save), "-screen-width", "1280")
        self.assertEqual(result.returncode, 0, result.stderr)
        arguments = json.loads(result.stdout.splitlines()[-1])
        actual = [arguments[index + 1] for index, value in enumerate(arguments)
                  if value == "-mc-runtime-package"]
        self.assertEqual(actual, packages)
        self.assertEqual(arguments.count("-mc-runtime-owner-cache"), 4)
        self.assertEqual(arguments.count("-mc-runtime-presentation-cache"), 4)
        self.assertNotIn(str(generated / "mc-bhq8-level1-collision-v5-night"), arguments)
        self.assertNotIn(str(generated / "settlement-20261105-integration/owner-closed-v6/level1"), arguments)
        self.assertNotIn("-mc-runtime-menu", arguments)
        self.assertEqual(arguments[arguments.index("-mc-runtime-replay-save") + 1], str(save))
        self.assertEqual(arguments[arguments.index("-mc-runtime-media-cache") + 1], str(media))
        self.assertEqual(arguments[arguments.index("-mc-runtime-menu-cache") + 1], str(menu))
        self.assertEqual(arguments[-2:], ["-screen-width", "1280"])

    def test_campaign_current_save_and_native_arguments_use_the_same_route(self):
        generated = self.campaign_resources()
        save = generated / "current player save.json"
        save.write_text("routing fixture, not a runtime save")
        for option in ("--load-save", "-mc-runtime-replay-load"):
            result = self.campaign(option, str(save), "-mc-runtime-replay-save", str(save))
            self.assertEqual(result.returncode, 0, result.stderr)
            arguments = json.loads(result.stdout.splitlines()[-1])
            self.assertNotIn("-mc-runtime-menu", arguments)
            self.assertEqual(arguments[arguments.index("-mc-runtime-replay-load") + 1], str(save))
        result = self.campaign("--new-game", environment={"MC_CAMPAIGN_REPLAY": "/missing"})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("-mc-runtime-replay-load", json.loads(result.stdout.splitlines()[-1]))

    def test_campaign_rejects_missing_conflicting_and_duplicate_inputs_before_player(self):
        generated = self.campaign_resources()
        package = str(generated / "mc-bhq8-level1-collision-v5-night")
        cache = str(generated / "mc2-level1-night-cache-v2")
        cases = [
            (["--package"], "requires a path"),
            (["--package", ""], "nonblank path"),
            (["--package", "/missing/owner/package"], "Missing campaign package"),
            (["--package", package], "requires --loading-cache"),
            (["--owner-cache", cache], "must follow a --package"),
            (["--package", package, "--owner-cache", "/missing/cache"], "Missing owner cache"),
            (["--package", package, "--owner-cache", cache, "--owner-cache", cache],
             "Duplicate owner resource"),
            (["--package", package, "--package", package + "/."], "Duplicate campaign package"),
            (["--load-save", "/missing/save.json"], "Missing runtime save"),
            (["--new-game", "--load-save", "/missing/save.json"], "Choose only one"),
            (["--menu", "--new-game"], "Choose only one"),
            (["-mc-runtime-menu", "-mc-runtime-replay-load", "/missing/save.json"], "Choose only one"),
            (["--new-game", "--", "-mc-runtime-replay-load", "/missing/save.json"],
             "Campaign options must precede --"),
            (["--save", str(generated / "missing parent/save.json")], "Invalid save destination"),
            (["--level", "2"], "--new-game"),
            (["--packge", package], "Unknown campaign option"),
        ]
        for options, message in cases:
            with self.subTest(options=options):
                result = self.campaign(*options)
                self.assertEqual(result.returncode, 64, result.stderr)
                self.assertIn(message, result.stderr)
                self.assertNotIn("Native plugin SHA256", result.stdout)


if __name__ == "__main__":
    unittest.main()
