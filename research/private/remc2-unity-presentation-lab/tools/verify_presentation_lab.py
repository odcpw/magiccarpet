#!/usr/bin/env python3
"""Static guard for the owner-free private presentation-lab scaffold."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys


EDITOR = "6000.3.20f1"
REQUIRED_PACKAGES = {
    "com.unity.render-pipelines.universal": "17.3.0",
    "com.unity.inputsystem": "1.14.2",
    "com.unity.test-framework": "1.6.0",
    "com.unity.xr.management": "4.7.0",
    "com.unity.xr.openxr": "1.17.1",
    "com.unity.xr.interaction.toolkit": "3.0.9",
}


def fail(message: str) -> None:
    raise ValueError(message)


def check(root: Path) -> dict[str, object]:
    manifest_path = root / "Packages" / "manifest.json"
    lock_path = root / "Packages" / "packages-lock.json"
    contract_path = root / "presentation-lab-contract.json"
    version_path = root / "ProjectSettings" / "ProjectVersion.txt"
    scene_path = root / "Assets" / "Scenes" / "PresentationLabBoot.unity"
    script_path = root / "Assets" / "Scripts" / "PresentationLabBootstrap.cs"
    editor_smoke_path = root / "Assets" / "Editor" / "PresentationLabEditorSmoke.cs"
    build_settings_path = root / "ProjectSettings" / "EditorBuildSettings.asset"

    for path in (
        manifest_path,
        lock_path,
        contract_path,
        version_path,
        scene_path,
        script_path,
        editor_smoke_path,
        build_settings_path,
    ):
        if not path.is_file():
            fail(f"required file missing: {path.relative_to(root)}")

    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    lock = json.loads(lock_path.read_text(encoding="utf-8"))
    contract = json.loads(contract_path.read_text(encoding="utf-8"))

    if EDITOR not in version_path.read_text(encoding="utf-8"):
        fail(f"ProjectVersion.txt is not pinned to {EDITOR}")

    manifest_dependencies = manifest.get("dependencies", {})
    lock_dependencies = lock.get("dependencies", {})
    for package, version in REQUIRED_PACKAGES.items():
        if manifest_dependencies.get(package) != version:
            fail(f"manifest pin mismatch for {package}")
        if lock_dependencies.get(package, {}).get("version") != version:
            fail(f"lock pin mismatch for {package}")
        if lock_dependencies.get(package, {}).get("depth") != 0:
            fail(f"package is not a direct dependency: {package}")

    if contract.get("renderer") != "universal-render-pipeline":
        fail("presentation contract does not select URP")
    modes = contract.get("modes", {})
    if set(modes) != {"classic-hd", "unity-enhanced"}:
        fail("presentation contract must contain exactly Classic HD and Unity Enhanced")
    startup = contract.get("startup", {})
    if startup.get("synthetic_no_owner_allowed") is not True:
        fail("owner-free synthetic boot is not enabled")
    if startup.get("mid_session_mode_swap") is not False:
        fail("mid-session mode switching must remain disabled")

    scene = scene_path.read_text(encoding="utf-8")
    script = script_path.read_text(encoding="utf-8")
    editor_smoke = editor_smoke_path.read_text(encoding="utf-8")
    build_settings = build_settings_path.read_text(encoding="utf-8")
    if "1f5c9e6b76e54e8e8bb8cbcc3c8dc1d6" not in scene:
        fail("boot scene does not reference PresentationLabBootstrap script GUID")
    for marker in ("--mc-mode=", "--mc-owner-root=", "--mc-require-owner-data", "synthetic/no-owner"):
        if marker not in script:
            fail(f"bootstrap is missing diagnostic marker: {marker}")
    if "PresentationLabEditorSmoke" not in editor_smoke or "EditorApplication.Exit(0)" not in editor_smoke:
        fail("editor smoke entry point is incomplete")
    if "Assets/Scenes/PresentationLabBoot.unity" not in build_settings:
        fail("boot scene is not registered in EditorBuildSettings")

    forbidden_dirs = (
        "Library",
        "Temp",
        "Build",
        "Builds",
        "Assets/OwnerData",
        "Assets/OwnerAssets",
        "Assets/StreamingAssets",
    )
    present_forbidden = [relative for relative in forbidden_dirs if (root / relative).exists()]
    if present_forbidden:
        fail("owner/build/cache paths must not be present: " + ", ".join(present_forbidden))

    forbidden_suffixes = {".unitypackage", ".bundle", ".pak"}
    payload_files = [
        path.relative_to(root).as_posix()
        for path in root.rglob("*")
        if path.is_file() and path.suffix.lower() in forbidden_suffixes
    ]
    if payload_files:
        fail("owner payload-like files are present: " + ", ".join(payload_files))

    return {
        "status": "pass",
        "project": root.name,
        "editor": EDITOR,
        "direct_package_pins": REQUIRED_PACKAGES,
        "owner_payload_files": [],
        "runtime_editor_verified": False,
        "notes": [
            "static separation and contract checks passed",
            "Unity editor/player smoke remains a separate gate",
        ],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true", dest="as_json")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args(argv)
    try:
        result = check(args.root.resolve())
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        if args.as_json:
            print(json.dumps({"status": "fail", "error": str(exc)}, sort_keys=True))
        else:
            print(f"FAIL: {exc}", file=sys.stderr)
        return 1
    if args.as_json:
        print(json.dumps(result, indent=2, sort_keys=True))
    else:
        print("PASS: presentation-lab static guard")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
