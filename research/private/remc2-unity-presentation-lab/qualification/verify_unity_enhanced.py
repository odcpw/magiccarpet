#!/usr/bin/env python3
"""Verify the owner-free Unity Enhanced lifecycle shell without mutating it."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


PROJECT_ROOT = Path(__file__).resolve().parents[1]
EXACT_ROOT = PROJECT_ROOT.parent / "remc2-unity-native"


def _check(checks: list[dict[str, Any]], name: str, passed: bool, detail: str) -> None:
    checks.append({"name": name, "passed": passed, "detail": detail})


def verify(project_root: Path = PROJECT_ROOT) -> dict[str, Any]:
    checks: list[dict[str, Any]] = []
    script_root = project_root / "Assets" / "Scripts"
    test_path = project_root / "Assets" / "Tests" / "Editor" / "UnityEnhancedLifecycleEditModeTests.cs"
    scene_path = project_root / "Assets" / "Scenes" / "UnityEnhanced.unity"
    router_path = script_root / "PresentationModeRouter.cs"
    scripts = [
        script_root / "UnityEnhancedWorldRoot.cs",
        script_root / "UnityEnhancedCameraRig.cs",
        script_root / "UnityEnhancedInputAdapter.cs",
        script_root / "UnityEnhancedPresentationAdapters.cs",
        script_root / "UnityEnhancedTerrainPresentation.cs",
        script_root / "UnityEnhancedActorPresentation.cs",
        script_root / "UnityEnhancedNightLighting.cs",
        script_root / "UnityEnhancedWaterPresentation.cs",
        script_root / "UnityEnhancedVfxPresentation.cs",
        script_root / "UnityEnhancedQualityAndPost.cs",
        script_root / "UnityEnhancedHudPresentation.cs",
        script_root / "UnityEnhancedMinimapPresentation.cs",
        script_root / "UnityEnhancedDiagnostics.cs",
        script_root / "UnityEnhancedOwnerSessionSource.cs",
    ]

    _check(checks, "project exists", project_root.is_dir(), str(project_root))
    _check(checks, "Exact project remains a sibling", project_root != EXACT_ROOT and EXACT_ROOT.is_dir(), str(EXACT_ROOT))
    _check(checks, "Enhanced scene exists", scene_path.is_file(), str(scene_path))
    _check(checks, "Enhanced scripts and focused tests exist", all(path.is_file() for path in scripts + [test_path]), ", ".join(str(path) for path in scripts + [test_path]))

    scene_text = scene_path.read_text(encoding="utf-8") if scene_path.is_file() else ""
    root_meta = (script_root / "UnityEnhancedWorldRoot.cs.meta").read_text(encoding="utf-8") if (script_root / "UnityEnhancedWorldRoot.cs.meta").is_file() else ""
    root_guid = root_meta.split("guid:", 1)[1].strip().splitlines()[0] if "guid:" in root_meta else ""
    _check(checks, "scene contains the Enhanced root component", bool(root_guid) and root_guid in scene_text, str(scene_path))

    source_text = "\n".join(path.read_text(encoding="utf-8") for path in scripts if path.is_file())
    router_text = router_path.read_text(encoding="utf-8") if router_path.is_file() else ""
    _check(checks, "snapshot lifecycle is generation and tick bound", all(token in source_text for token in ("ApplySnapshot", "Generation", "LastTick", "ResetPresentation")), str(script_root))
    _check(checks, "identity pooling and stale cleanup are explicit", all(token in source_text for token in ("activeEntities", "pooledEntities", "StableIdentity", "Recycle")), str(scripts[0]))
    _check(checks, "camera and entity interpolation is presentation-only", all(token in source_text for token in ("ApplyPose", "Interpolate", "InterpolatePresentation", "SetTargetPose", "FieldOfView")), str(script_root))
    _check(checks, "input adapter is sequence-bound", all(token in source_text for token in ("Publish", "TryConsume", "lastPublishedSequence")), str(scripts[2]))
    _check(checks, "HUD/audio adapter is passive and event ordered", all(token in source_text for token in ("AcceptEvent", "ApplyTick", "lastEventSequence")), str(scripts[3]))
    _check(checks, "terrain consumes authoritative planes, dirty regions, and overlays", all(token in source_text for token in ("Heights", "TerrainTypes", "Shade", "Angles", "DirtyRegions", "RecalculateNormals", "WrapIndex", "DiagnosticOverlay")), str(script_root / "UnityEnhancedTerrainPresentation.cs"))
    _check(checks, "actor family coverage is generated and pooled", all(token in source_text for token in ("UnityEnhancedPresentationFamily", "Player", "Castle", "Possession", "GetMesh", "Release", "sortingOrder")), str(script_root / "UnityEnhancedActorPresentation.cs"))
    _check(checks, "Level 1 Night lighting is descriptor-driven and capped", all(token in source_text for token in ("Level1Night", "LocalLightCap", "DroppedLocalLightCount", "CompareRequests", "LightType.Directional")), str(script_root / "UnityEnhancedNightLighting.cs"))
    _check(checks, "water and shoreline reflection path is explicit", all(token in source_text for token in ("UnityEnhancedWaterReflectionMode", "ReflectionEnabled", "BuildShoreline", "ShorelineMask")), str(script_root / "UnityEnhancedWaterPresentation.cs"))
    _check(checks, "VFX events are ordered, replayable, pooled, and reduced-motion aware", all(token in source_text for token in ("UnityEnhancedVfxEvent", "ApplyReplay", "MaxActiveEffects", "DuplicateEffectCount", "ReducedMotion", "CameraComfortEnabled")), str(script_root / "UnityEnhancedVfxPresentation.cs"))
    _check(checks, "HUD consumes generation-bound mission state through the Enhanced camera", all(token in source_text for token in ("ApplySnapshot", "ScreenSpaceCamera", "PlayerLife", "PlayerMana", "never sends input or state back")), str(script_root / "UnityEnhancedHudPresentation.cs"))
    _check(checks, "minimap consumes the authoritative marks and command stream", all(token in source_text for token in ("UnityEnhancedMinimapSnapshot", "MinimapMarksSize", "mc_native_level1_minimap_marks", "mc_native_level1_minimap_entities_copy", "CommandHash", "DrawCommand")), str(script_root))
    _check(checks, "URP quality/post and spatial upscaling are gated", all(token in source_text for token in ("UniversalAdditionalCameraData", "Volume", "renderScale", "UpscalingFilterSelection", "TemporalMode", "TemporalUpscalingEnabled")), str(script_root / "UnityEnhancedQualityAndPost.cs"))
    _check(checks, "support diagnostics are path-free and measurable", all(token in source_text for token in ("UnityEnhancedPresentationMetrics", "BuildSupportReport", "LastFrameMilliseconds", "DroppedEffects", "DroppedLights", "MeetsFrameBudget")), str(script_root / "UnityEnhancedDiagnostics.cs"))
    _check(checks, "Boot router opens and closes Enhanced root", all(token in router_text for token in ("FindFirstObjectByType<UnityEnhancedWorldRoot>", "enhancedRoot.Open(SelectedMode, sessionLease)", "enhancedRoot.Close()")), str(router_path))
    owner_source_path = script_root / "UnityEnhancedOwnerSessionSource.cs"
    owner_source_text = owner_source_path.read_text(encoding="utf-8") if owner_source_path.is_file() else ""
    _check(checks, "owner-bound Enhanced source consumes native v2 observations", all(token in owner_source_text for token in ("UnityEnhancedOwnerSessionSource", "mc_native_session_open", "mc_native_session_step_with_camera", "mc_native_terrain_plane_copy_u8", "mc_native_entities_copy", "ReadObservations", "BuildSnapshot")), str(owner_source_path))
    _check(checks, "owner-bound Enhanced route is explicit and reset-safe", all(token in router_text for token in ("enhancedOwnerSession.Configure", "CaptureEnhancedFrame", "ResetEnhancedOwnerSession")), str(router_path))
    _check(checks, "no gameplay or renderer-boundary imports", not any(token in source_text for token in ("remc2-unity-native", "MagicCarpet.Native", "NativeSession", "VoxelMode")), str(script_root))
    _check(checks, "no physics authority is present", "Collider" not in source_text and "Rigidbody" not in source_text, str(script_root))

    forbidden_payload_extensions = {".dll", ".so", ".dylib", ".a", ".png", ".jpg", ".jpeg", ".wav", ".ogg", ".mp3", ".bundle", ".assetbundle"}
    payloads = [path for path in (project_root / "Assets").rglob("*") if path.is_file() and path.suffix.lower() in forbidden_payload_extensions]
    _check(checks, "no owner payload is embedded", not payloads, ", ".join(str(path) for path in payloads) or "none")

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
        print(summary["status"] + " Unity Enhanced verification")
        for check in summary["checks"]:
            print(("PASS" if check["passed"] else "FAIL") + " " + check["name"] + ": " + check["detail"])
    return 0 if summary["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
