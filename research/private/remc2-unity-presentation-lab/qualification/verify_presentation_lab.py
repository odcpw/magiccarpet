#!/usr/bin/env python3
"""Verify the isolated Unity presentation-lab project without mutating it."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any, Callable


PROJECT_ROOT = Path(__file__).resolve().parents[1]
EXACT_ROOT = PROJECT_ROOT.parent / "remc2-unity-native"
ROUTER_SCRIPT = PROJECT_ROOT / "Assets" / "Scripts" / "PresentationModeRouter.cs"
VALIDATOR_SCRIPT = PROJECT_ROOT / "Assets" / "Scripts" / "OwnerInputValidator.cs"
LEASE_SCRIPT = PROJECT_ROOT / "Assets" / "Scripts" / "PresentationSessionLease.cs"
BOOT_SCENE = PROJECT_ROOT / "Assets" / "Scenes" / "Boot.unity"
GRAPHICS_SETTINGS = PROJECT_ROOT / "ProjectSettings" / "GraphicsSettings.asset"


def _check(checks: list[dict[str, Any]], name: str, passed: bool, detail: str) -> None:
    checks.append({"name": name, "passed": passed, "detail": detail})


def _load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def verify(project_root: Path = PROJECT_ROOT) -> dict[str, Any]:
    checks: list[dict[str, Any]] = []
    manifest_path = project_root / "Packages" / "manifest.json"
    lock_path = project_root / "Packages" / "packages-lock.json"
    version_path = project_root / "ProjectSettings" / "ProjectVersion.txt"
    build_settings = project_root / "ProjectSettings" / "EditorBuildSettings.asset"
    router_path = project_root / "Assets" / "Scripts" / "PresentationModeRouter.cs"
    validator_path = project_root / "Assets" / "Scripts" / "OwnerInputValidator.cs"
    lease_path = project_root / "Assets" / "Scripts" / "PresentationSessionLease.cs"
    test_path = project_root / "Assets" / "Tests" / "Editor" / "PresentationLabEditModeTests.cs"
    classic_settings_path = project_root / "Assets" / "Scripts" / "ClassicHdPresentationSettings.cs"
    classic_abi_path = project_root / "Assets" / "Scripts" / "ClassicHdAbiContract.cs"
    classic_surface_path = project_root / "Assets" / "Scripts" / "ClassicHdR8Surface.cs"
    classic_presentation_path = project_root / "Assets" / "Scripts" / "ClassicHdPresentation.cs"
    classic_display_path = project_root / "Assets" / "Scripts" / "ClassicHdDisplayComposition.cs"
    classic_shader_path = project_root / "Assets" / "Resources" / "ClassicHdIndexedUnlit.shader"
    launcher_path = project_root / "launcher" / "presentation_launcher.py"
    launcher_config_path = project_root / "launcher" / "presentation-config.json"
    boot_scene = project_root / "Assets" / "Scenes" / "Boot.unity"
    graphics_settings = project_root / "ProjectSettings" / "GraphicsSettings.asset"
    global_settings = project_root / "Assets" / "UniversalRenderPipelineGlobalSettings.asset"

    _check(checks, "project exists", project_root.is_dir(), str(project_root))
    _check(checks, "Exact project is a sibling, not the lab", project_root != EXACT_ROOT and EXACT_ROOT.is_dir(), str(EXACT_ROOT))
    _check(checks, "Unity version is pinned", version_path.is_file() and "m_EditorVersion: 6000.3.20f1" in version_path.read_text(encoding="utf-8"), str(version_path))

    manifest_ok = False
    if manifest_path.is_file():
        manifest = _load_json(manifest_path)
        manifest_ok = manifest.get("dependencies", {}).get("com.unity.render-pipelines.universal") == "17.3.0"
    _check(checks, "URP package is explicit", manifest_ok, str(manifest_path))

    lock_ok = False
    if lock_path.is_file():
        lock = _load_json(lock_path)
        urp = lock.get("dependencies", {}).get("com.unity.render-pipelines.universal", {})
        lock_ok = urp.get("version") == "17.3.0" and urp.get("source") == "builtin"
    _check(checks, "URP lock entry is resolved", lock_ok, str(lock_path))

    required_files = [router_path, validator_path, lease_path, test_path, boot_scene]
    _check(checks, "mode shell files exist", all(path.is_file() for path in required_files), ", ".join(str(path) for path in required_files))
    classic_files = [
        classic_settings_path,
        classic_abi_path,
        classic_surface_path,
        classic_presentation_path,
        classic_display_path,
        classic_shader_path,
        launcher_path,
        launcher_config_path,
    ]
    _check(checks, "Classic HD integration files exist", all(path.is_file() for path in classic_files), ", ".join(str(path) for path in classic_files))
    _check(checks, "Boot scene is in build settings", build_settings.is_file() and "Assets/Scenes/Boot.unity" in build_settings.read_text(encoding="utf-8"), str(build_settings))

    graphics_text = graphics_settings.read_text(encoding="utf-8") if graphics_settings.is_file() else ""
    urp_asset = project_root / "Assets" / "Settings" / "PresentationLabURP.asset"
    _check(checks, "generated URP asset is assigned", urp_asset.is_file() and "m_CustomRenderPipeline: {fileID: 0}" not in graphics_text, str(urp_asset))
    _check(checks, "URP global settings are local", global_settings.is_file() and "UniversalRenderPipeline" in graphics_text, str(global_settings))

    router_text = router_path.read_text(encoding="utf-8") if router_path.is_file() else ""
    validator_text = validator_path.read_text(encoding="utf-8") if validator_path.is_file() else ""
    lease_text = lease_path.read_text(encoding="utf-8") if lease_path.is_file() else ""
    classic_text = "\n".join(
        path.read_text(encoding="utf-8")
        for path in (classic_settings_path, classic_abi_path, classic_surface_path, classic_presentation_path, classic_display_path)
        if path.is_file()
    )
    launcher_text = launcher_path.read_text(encoding="utf-8") if launcher_path.is_file() else ""
    launcher_config_text = launcher_config_path.read_text(encoding="utf-8") if launcher_config_path.is_file() else ""
    _check(checks, "router selects before opening", "PresentationSessionLease.Open" in router_text and "ownerValidation" in router_text, str(router_path))
    _check(checks, "missing inputs fail closed diagnostically", "NO OWNER INPUTS" in router_text and "return;" in router_text, str(router_path))
    _check(checks, "owner inputs stay external", "IsInside(projectRoot" in validator_text and "must remain outside" in validator_text, str(validator_path))
    _check(checks, "session lease is presentation-only", "does not load owner data" in lease_text and "IsOpen" in lease_text, str(lease_path))
    _check(checks, "Classic HD binds the additive C ABI", "mc_classic_hd_frame_copy_fixture" in classic_text and "NativeLibrary = \"sim_capi\"" in classic_text, str(classic_abi_path))
    _check(checks, "Classic HD uses a persistent R8 surface", "TextureFormat.R8" in classic_text and "UploadTopRowFirst" in classic_text and "new Texture2D" in classic_text, str(classic_surface_path))
    _check(checks, "launcher defaults to Exact", '"default_mode": "exact"' in launcher_config_text and '"default_mode"' in launcher_text, str(launcher_config_path))
    _check(checks, "launcher exposes all three modes", all(token in launcher_config_text for token in ('"exact"', '"classic-hd"', '"unity-enhanced"')), str(launcher_config_path))

    forbidden_tokens = ("remc2-unity-native", "MagicCarpet.Native", "NativeSession", "VoxelMode")
    all_text = "\n".join(text for text in (router_text, validator_text, lease_text, classic_text, launcher_text))
    _check(checks, "lab scripts do not reference Exact/native gameplay", not any(token in all_text for token in forbidden_tokens), str(project_root / "Assets"))

    forbidden_payload_extensions = {".dll", ".so", ".dylib", ".a", ".png", ".jpg", ".jpeg", ".wav", ".ogg", ".mp3", ".bundle", ".assetbundle"}
    payloads = [path for path in (project_root / "Assets").rglob("*") if path.is_file() and path.suffix.lower() in forbidden_payload_extensions]
    _check(checks, "no owner payload is embedded", not payloads, ", ".join(str(path) for path in payloads) or "none")
    _check(checks, "no VoxelMode dependency is embedded", "VoxelMode" not in all_text, str(project_root / "Assets"))

    status = "PASS" if all(check["passed"] for check in checks) else "FAIL"
    return {"status": status, "project": str(project_root), "checks": checks, "failure_count": sum(not check["passed"] for check in checks)}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true", help="emit a machine-readable summary")
    args = parser.parse_args()
    summary = verify()
    if args.json:
        print(json.dumps(summary, indent=2, sort_keys=True))
    else:
        print(summary["status"] + " presentation-lab verification")
        for check in summary["checks"]:
            print(("PASS" if check["passed"] else "FAIL") + " " + check["name"] + ": " + check["detail"])
    return 0 if summary["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
