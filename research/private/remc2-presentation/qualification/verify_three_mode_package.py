#!/usr/bin/env python3
"""Verify the private three-mode presentation package and its open gates.

This is a package-integrity gate, not a player-facing release claim. It checks
that the Exact, Classic HD, and Unity Enhanced contracts remain separated, the
owner-bound Classic HD replay evidence is internally coherent, and every
existing validator is green. Placeholder visual corpora and pending owner/GPU
routes remain reported as open qualification state.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
from typing import Any, Callable


PRESENTATION_ROOT = Path(__file__).resolve().parents[1]
LAB_ROOT = PRESENTATION_ROOT.parent / "remc2-unity-presentation-lab"
SEQUENCE_ARTIFACT = Path(__file__).with_name("classic-hd-owner-route-sequence-smoke.json")
PROFILE_960_SEQUENCE_ARTIFACT = Path(__file__).with_name(
    "classic-hd-owner-route-960x720-sequence-smoke.json"
)
PROFILE_960_SEQUENCE_FAILURE_ARTIFACT = Path(__file__).with_name(
    "classic-hd-owner-route-960x720-sequence-failure.json"
)
ENHANCED_OWNER_SEQUENCE_ARTIFACT = Path(__file__).with_name(
    "unity-enhanced-owner-route-sequence-smoke.json"
)
ENHANCED_OWNER_SCENARIO_ARTIFACT = Path(__file__).with_name(
    "unity-enhanced-owner-route-scenarios.json"
)
CLASSIC_HD_OWNER_SCENARIO_ARTIFACT = Path(__file__).with_name(
    "classic-hd-owner-route-scenarios.json"
)
CLASSIC_HD_OWNER_SCENARIO_960_ARTIFACT = Path(__file__).with_name(
    "classic-hd-owner-route-960x720-scenarios.json"
)
CLASSIC_HD_OWNER_HIGH_SPEED_ARTIFACT = Path(__file__).with_name(
    "classic-hd-owner-high-speed-probe.json"
)
CLASSIC_HD_OWNER_PERFORMANCE_ARTIFACT = Path(__file__).with_name(
    "classic-hd-owner-performance-smoke.json"
)
CLASSIC_HD_OWNER_GPU_FENCE_ARTIFACT = Path(__file__).with_name(
    "classic-hd-owner-gpu-fence.json"
)
CLASSIC_HD_OWNER_ASSET_FALLBACK_ARTIFACT = Path(__file__).with_name(
    "classic-hd-owner-asset-fallback.json"
)
CLASSIC_HD_OWNER_FOCUS_PAUSE_ARTIFACT = Path(__file__).with_name(
    "classic-hd-owner-focus-pause.json"
)
CLASSIC_HD_OWNER_EXIT_SIDECAR_ARTIFACT = Path(__file__).with_name(
    "classic-hd-owner-exit-sidecar.json"
)
CLASSIC_HD_OWNER_COMPLETION_REPLAY_ARTIFACT = Path(__file__).with_name(
    "classic-hd-owner-completion-replay-smoke.json"
)
CLASSIC_HD_OWNER_EFFECTS_MATRIX_ARTIFACT = Path(__file__).with_name(
    "classic-hd-owner-effects-matrix.json"
)
CLASSIC_HD_OWNER_OS_WINDOW_LIFECYCLE_ARTIFACT = Path(__file__).with_name(
    "classic-hd-owner-os-window-lifecycle.json"
)
CLASSIC_HD_OWNER_OS_WINDOW_FULLSCREEN_ARTIFACT = Path(__file__).with_name(
    "classic-hd-owner-os-window-lifecycle-fullscreen-window.json"
)
CLASSIC_HD_OWNER_COMPLETION_ARTIFACT = Path(__file__).with_name(
    "classic-hd-owner-completion-route.json"
)
CLASSIC_HD_OWNER_VISUAL_CAPTURE_ARTIFACT = Path(__file__).with_name(
    "classic-hd-owner-visual-capture.json"
)
UNITY_ENHANCED_OWNER_VISUAL_CAPTURE_ARTIFACT = Path(__file__).with_name(
    "unity-enhanced-owner-visual-capture.json"
)
CLASSIC_HD_OWNER_VISUAL_CAPTURE_DIR = (
    Path(__file__).parent / "captures" / "classic-hd-owner-640x480"
)
UNITY_ENHANCED_OWNER_VISUAL_CAPTURE_DIR = (
    Path(__file__).parent / "captures" / "unity-enhanced-owner-1280x720"
)

OWNER_VISUAL_CAPTURE_ROUTES = {
    "town-approach": ("l1-night-town-approach.png", False),
    "town-overflight": ("l1-night-town-overflight.png", False),
    "fireball-fight": ("l1-night-fireball-fight.png", True),
    "mana-castle": ("l1-night-mana-castle.png", False),
    "reflective-water": ("l1-night-reflective-water.png", False),
    "fog-depth": ("l1-night-fog-depth.png", False),
    "high-speed-flight": ("l1-night-high-speed-flight.png", False),
}


def _load_module(name: str, path: Path) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise ImportError(f"cannot load validator module: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _check(checks: list[dict[str, Any]], name: str, passed: bool, detail: str) -> None:
    checks.append({"name": name, "passed": passed, "detail": detail})


def validate_sequence_artifact(
    path: Path = SEQUENCE_ARTIFACT,
    profile: str = "classic-hd-640x480",
    width: int = 640,
    height: int = 480,
) -> list[str]:
    errors: list[str] = []
    try:
        artifact = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"sequence artifact read failed: {error}"]

    if artifact.get("schema_version") != 1:
        errors.append("sequence artifact schema_version must be 1")
    if artifact.get("kind") != "classic-hd-owner-route-sequence-smoke/v1":
        errors.append("sequence artifact kind is wrong")
    if artifact.get("profile") != profile:
        errors.append(f"sequence artifact must target {profile}")
    if artifact.get("internal_width") != width or artifact.get("internal_height") != height:
        errors.append(f"sequence artifact extent must be {width}x{height}")
    if artifact.get("owner_payload_embedded") is not False:
        errors.append("sequence artifact must not embed owner payload")
    if artifact.get("owner_inputs_external") is not True:
        errors.append("sequence artifact must identify owner inputs as external")
    frames_per_pass = artifact.get("frames_per_pass")
    pass_count = artifact.get("pass_count")
    frame_count = artifact.get("frame_count")
    if not isinstance(frames_per_pass, int) or frames_per_pass <= 0:
        errors.append("sequence artifact frames_per_pass must be positive")
    if not isinstance(pass_count, int) or pass_count != 2:
        errors.append("sequence artifact must contain two replay passes")
    if isinstance(frames_per_pass, int) and isinstance(pass_count, int) and frame_count != frames_per_pass * pass_count:
        errors.append("sequence artifact frame_count does not match its passes")
    if frame_count != artifact.get("upload_count"):
        errors.append("every qualified frame must have one persistent upload")
    if artifact.get("allocation_epoch") != 1:
        errors.append("sequence artifact must prove one steady-state surface allocation")
    if artifact.get("reset_replay_match") is not True:
        errors.append("sequence reset replay did not match")
    for field in ("source_commit", "native_plugin_sha256", "log_sha256", "index_sequence_hash", "provenance_sequence_hash"):
        if not isinstance(artifact.get(field), str) or not artifact[field]:
            errors.append(f"sequence artifact is missing {field}")
    input_route = artifact.get("input_route")
    if not isinstance(input_route, list) or len(input_route) != 4:
        errors.append("sequence artifact must retain all four bounded input phases")
    if "GFX-O" not in str(artifact.get("qualification_status", "")):
        errors.append("sequence artifact must keep the broader GFX-O gate explicit")
    if artifact.get("failure_reason") is not None:
        errors.append("sequence artifact contains a failure reason")
    return errors


def validate_profile_smoke_artifact(
    path: Path,
    profile: str,
    width: int,
    height: int,
) -> list[str]:
    errors: list[str] = []
    try:
        artifact = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"profile smoke artifact read failed: {error}"]

    if artifact.get("schema_version") != 1:
        errors.append("profile smoke artifact schema_version must be 1")
    if artifact.get("kind") != "classic-hd-owner-route-profile-smoke/v1":
        errors.append("profile smoke artifact kind is wrong")
    if artifact.get("profile") != profile:
        errors.append(f"profile smoke artifact must target {profile}")
    if artifact.get("internal_width") != width or artifact.get("internal_height") != height:
        errors.append(f"profile smoke artifact extent must be {width}x{height}")
    if artifact.get("owner_payload_embedded") is not False or artifact.get("owner_inputs_external") is not True:
        errors.append("profile smoke artifact must keep owner inputs external")
    if artifact.get("frame_count") != 1 or artifact.get("upload_count") != 1:
        errors.append("profile smoke artifact must prove one frame and one upload")
    if artifact.get("profile_rerender_endpoint_reached") is not True:
        errors.append("profile smoke did not reach the rerender endpoint")
    for field in ("source_index_hash", "source_provenance_hash", "source_receipt_hash", "native_plugin_sha256", "log_sha256"):
        if not isinstance(artifact.get(field), str) or not artifact[field]:
            errors.append(f"profile smoke artifact is missing {field}")
    if artifact.get("failure_reason") is not None:
        errors.append("profile smoke artifact contains a failure reason")
    status = str(artifact.get("qualification_status", ""))
    if "pending" not in status and "open" not in status:
        errors.append("profile smoke must keep sustained qualification pending/open")
    return errors


def validate_enhanced_owner_sequence_artifact(
    path: Path = ENHANCED_OWNER_SEQUENCE_ARTIFACT,
) -> list[str]:
    errors: list[str] = []
    try:
        artifact = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"Unity Enhanced owner sequence artifact read failed: {error}"]

    if artifact.get("schema_version") != 1:
        errors.append("Unity Enhanced owner sequence schema_version must be 1")
    if artifact.get("kind") != "unity-enhanced-owner-route-sequence-smoke/v1":
        errors.append("Unity Enhanced owner sequence artifact kind is wrong")
    if artifact.get("mode") != "unity-enhanced":
        errors.append("Unity Enhanced owner sequence artifact mode is wrong")
    if artifact.get("native_abi") != "sim-capi-v2":
        errors.append("Unity Enhanced owner sequence must identify sim-capi-v2")
    if artifact.get("owner_payload_embedded") is not False or artifact.get("owner_inputs_external") is not True:
        errors.append("Unity Enhanced owner sequence must keep owner inputs external")
    if artifact.get("frames_per_pass") != 32 or artifact.get("pass_count") != 2:
        errors.append("Unity Enhanced owner sequence must retain two 32-frame passes")
    if artifact.get("frame_count") != 64 or artifact.get("reset_replay") is not True:
        errors.append("Unity Enhanced owner sequence must prove reset replay over 64 frames")
    if artifact.get("reset_tick") != 0:
        errors.append("Unity Enhanced owner sequence must apply an explicit tick-0 reset")
    if artifact.get("first_tick") != 32 or artifact.get("second_tick") != 32:
        errors.append("Unity Enhanced owner sequence must reach tick 32 in both passes")
    if artifact.get("active_entity_count", 0) <= 0 or artifact.get("active_visual_count", 0) <= 0:
        errors.append("Unity Enhanced owner sequence must apply active owner entities to Unity")
    if artifact.get("terrain_vertex_count", 0) <= 0 or artifact.get("shoreline_cell_count", 0) <= 0:
        errors.append("Unity Enhanced owner sequence must apply terrain and shoreline geometry")
    if artifact.get("night_lighting") is not True or artifact.get("reflection") is not True:
        errors.append("Unity Enhanced owner sequence must apply night lighting and reflection")
    for field in (
        "state_sequence_hash",
        "terrain_sequence_hash",
        "world_sequence_hash",
        "log_sha256",
    ):
        if not isinstance(artifact.get(field), str) or not artifact[field]:
            errors.append(f"Unity Enhanced owner sequence is missing {field}")
    status = str(artifact.get("qualification_status", ""))
    if "open" not in status:
        errors.append("Unity Enhanced owner sequence must keep broader gates open")
    return errors


def validate_enhanced_owner_scenario_artifact(
    path: Path = ENHANCED_OWNER_SCENARIO_ARTIFACT,
) -> list[str]:
    errors: list[str] = []
    try:
        artifact = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"Unity Enhanced owner scenario artifact read failed: {error}"]

    if artifact.get("schema_version") != 1:
        errors.append("Unity Enhanced owner scenario schema_version must be 1")
    if artifact.get("kind") != "unity-enhanced-owner-route-scenarios/v1":
        errors.append("Unity Enhanced owner scenario artifact kind is wrong")
    if artifact.get("mode") != "unity-enhanced" or artifact.get("native_abi") != "sim-capi-v2":
        errors.append("Unity Enhanced owner scenario must identify the Enhanced sim-capi-v2 route")
    if artifact.get("owner_payload_embedded") is not False or artifact.get("owner_inputs_external") is not True:
        errors.append("Unity Enhanced owner scenario must keep owner inputs external")
    if artifact.get("scenario_frame_count") != 49:
        errors.append("Unity Enhanced owner scenario must retain the 49-frame route receipt")

    routes = artifact.get("routes")
    required_routes = {"idle", "town", "combat", "high_speed_flight", "death_restart"}
    if not isinstance(routes, dict) or not required_routes.issubset(routes):
        errors.append("Unity Enhanced owner scenario is missing a required route")
    else:
        for route in sorted(required_routes):
            if routes[route].get("status") != "pass":
                errors.append(f"Unity Enhanced owner scenario route {route} is not pass")
        if routes["combat"].get("projectile_rows", 0) <= 0 or routes["combat"].get("impact_rows", 0) <= 0:
            errors.append("Unity Enhanced combat scenario must observe projectile and impact rows")
        if routes["high_speed_flight"].get("max_actual_speed", 0) <= 0:
            errors.append("Unity Enhanced flight scenario must observe positive authoritative speed")
        if routes["death_restart"].get("death_observed") is not True or routes["death_restart"].get("restart_tick") != 0:
            errors.append("Unity Enhanced death/restart scenario must observe death and tick-0 reset")

    if artifact.get("night_lighting") is not True or artifact.get("reflection") is not True:
        errors.append("Unity Enhanced owner scenario must retain night lighting and reflection")
    for field in ("state_route_hash", "event_route_hash", "native_plugin_sha256", "log_sha256"):
        if not isinstance(artifact.get(field), str) or not artifact[field]:
            errors.append(f"Unity Enhanced owner scenario is missing {field}")
    status = str(artifact.get("qualification_status", ""))
    if "remain open" not in status:
        errors.append("Unity Enhanced owner scenario must keep unqualified gates explicit")
    return errors


def validate_classic_hd_owner_completion_artifact(
    path: Path = CLASSIC_HD_OWNER_COMPLETION_ARTIFACT,
) -> list[str]:
    errors: list[str] = []
    try:
        artifact = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"Classic HD owner completion artifact read failed: {error}"]

    if artifact.get("schema_version") != 1:
        errors.append("Classic HD owner completion schema_version must be 1")
    if artifact.get("mode") != "classic-hd":
        errors.append("Classic HD owner completion mode is wrong")
    if artifact.get("route") != "level1-completion-postgame":
        errors.append("Classic HD owner completion route is wrong")
    if artifact.get("status") != "pass":
        errors.append("Classic HD owner completion must be a pass receipt")
    if artifact.get("evidence_authority") != "owner-external-input-journal":
        errors.append("Classic HD owner completion must identify the external owner journal")
    if artifact.get("owner_payload_embedded") is not False:
        errors.append("Classic HD owner completion must not embed owner payload")
    if artifact.get("owner_inputs_external") is not True:
        errors.append("Classic HD owner completion must identify owner inputs as external")
    if artifact.get("journal_schema") != "mc2-unity-playthrough/v1":
        errors.append("Classic HD owner completion journal schema is wrong")
    if artifact.get("journal_status") != "complete":
        errors.append("Classic HD owner completion journal must be complete")
    if artifact.get("journal_command_encoding") != "mc-native-command-10-byte-le/v1":
        errors.append("Classic HD owner completion journal encoding is wrong")

    command_count = artifact.get("journal_command_count")
    replayed_commands = artifact.get("journal_replayed_commands")
    if not isinstance(command_count, int) or command_count < 1_000:
        errors.append("Classic HD owner completion journal must contain at least 1,000 commands")
    if replayed_commands != command_count:
        errors.append("Classic HD owner completion must replay every journal command")
    if artifact.get("frame_count") != replayed_commands:
        errors.append("Classic HD owner completion frame_count must equal replayed commands")
    if artifact.get("upload_count") != artifact.get("frame_count"):
        errors.append("Classic HD owner completion must upload every replayed frame")
    if artifact.get("allocation_epoch") != 1:
        errors.append("Classic HD owner completion must keep one steady-state allocation")
    if artifact.get("initial_tick") != 0 or artifact.get("source_initial_tick") != 0:
        errors.append("Classic HD owner completion must start from tick zero")
    if artifact.get("generation_stable") is not True:
        errors.append("Classic HD owner completion generation must remain stable")
    if artifact.get("authoritative_generation") != artifact.get("generation"):
        errors.append("Classic HD owner completion authoritative/source generation must agree")
    if artifact.get("exit_sequence_active_observed") is not True:
        errors.append("Classic HD owner completion must observe the exit sequence")
    if not isinstance(artifact.get("exit_sequence_active"), bool):
        errors.append("Classic HD owner completion must record the terminal exit-active state")
    if artifact.get("exit_sequence_complete") is not True:
        errors.append("Classic HD owner completion must reach exit completion")
    if artifact.get("exit_result_code") != 10:
        errors.append("Classic HD owner completion must reach exit result 10")
    if not isinstance(artifact.get("tasks_completed"), int) or artifact["tasks_completed"] <= 0:
        errors.append("Classic HD owner completion must prove completed mission tasks")
    for field in (
        "journal_manifest_sha256",
        "journal_commands_sha256",
        "index_route_hash",
        "provenance_route_hash",
    ):
        value = artifact.get(field)
        if not isinstance(value, str) or not value:
            errors.append(f"Classic HD owner completion is missing {field}")
    if not isinstance(artifact.get("reason"), str) or not artifact["reason"]:
        errors.append("Classic HD owner completion is missing its reason")
    return errors


def validate_classic_hd_owner_scenario_artifact(
    path: Path = CLASSIC_HD_OWNER_SCENARIO_ARTIFACT,
    profile: str = "hd-640x480",
    width: int = 640,
    height: int = 480,
) -> list[str]:
    errors: list[str] = []
    try:
        artifact = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"Classic HD owner scenario artifact read failed: {error}"]

    if artifact.get("schema_version") != 1:
        errors.append("Classic HD owner scenario schema_version must be 1")
    if artifact.get("kind") != "classic-hd-owner-route-scenarios/v1":
        errors.append("Classic HD owner scenario artifact kind is wrong")
    if artifact.get("mode") != "classic-hd" or artifact.get("native_abi") != "sim-capi-v2":
        errors.append("Classic HD owner scenario must identify the sim-capi-v2 route")
    if artifact.get("profile") != profile:
        errors.append(f"Classic HD owner scenario must identify the {profile} profile")
    if artifact.get("internal_width") != width or artifact.get("internal_height") != height:
        errors.append(f"Classic HD owner scenario extent must be {width}x{height}")
    if artifact.get("owner_payload_embedded") is not False or artifact.get("owner_inputs_external") is not True:
        errors.append("Classic HD owner scenario must keep owner inputs external")
    if artifact.get("scenario_frame_count") != 49 or artifact.get("upload_count") != 49:
        errors.append("Classic HD owner scenario must retain 49 frame/upload receipts")

    routes = artifact.get("routes")
    required_routes = {"idle", "town", "combat", "high_speed_flight", "death_restart"}
    if not isinstance(routes, dict) or not required_routes.issubset(routes):
        errors.append("Classic HD owner scenario is missing a required route")
    else:
        for route in sorted(required_routes):
            if routes[route].get("status") != "pass":
                errors.append(f"Classic HD owner scenario route {route} is not pass")
        if routes["town"].get("authoritative_camera_moved") is not True:
            errors.append("Classic HD town scenario must observe authoritative camera movement")
        if routes["combat"].get("projectile_rows", 0) <= 0 or routes["combat"].get("impact_rows", 0) <= 0:
            errors.append("Classic HD combat scenario must observe projectile and impact rows")
        if routes["high_speed_flight"].get("max_actual_speed", 0) <= 0:
            errors.append("Classic HD flight scenario must observe positive authoritative speed")
        if routes["death_restart"].get("death_observed") is not True or routes["death_restart"].get("restart_tick") != 0:
            errors.append("Classic HD death/restart scenario must observe death and tick-0 reset")

    if artifact.get("night_lighting") is not True or artifact.get("reflection") is not True:
        errors.append("Classic HD owner scenario must retain night lighting and reflection")
    if artifact.get("high_speed_postprocess_status") != "automatic-mode-not-reached-open":
        errors.append("Classic HD owner scenario must distinguish flight motion from automatic high-speed postprocess")
    for field in ("index_route_hash", "provenance_route_hash", "last_receipt_hash", "native_plugin_sha256", "log_sha256"):
        if not isinstance(artifact.get(field), str) or not artifact[field]:
            errors.append(f"Classic HD owner scenario is missing {field}")
    status = str(artifact.get("qualification_status", ""))
    if "remain open" not in status:
        errors.append("Classic HD owner scenario must keep unqualified gates explicit")
    return errors


def validate_classic_hd_owner_asset_fallback_artifact(
    path: Path = CLASSIC_HD_OWNER_ASSET_FALLBACK_ARTIFACT,
) -> list[str]:
    errors: list[str] = []
    try:
        artifact = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"Classic HD owner asset fallback artifact read failed: {error}"]

    if artifact.get("schema_version") != 1:
        errors.append("Classic HD owner asset fallback schema_version must be 1")
    if artifact.get("kind") != "classic-hd-owner-asset-fallback-probe/v1":
        errors.append("Classic HD owner asset fallback artifact kind is wrong")
    if artifact.get("mode") != "classic-hd" or artifact.get("native_abi") != "sim-capi-v2":
        errors.append("Classic HD owner asset fallback must identify the sim-capi-v2 route")
    if artifact.get("profile") != "hd-960x720":
        errors.append("Classic HD owner asset fallback must target hd-960x720")
    if artifact.get("internal_width") != 960 or artifact.get("internal_height") != 720:
        errors.append("Classic HD owner asset fallback extent must be 960x720")
    if artifact.get("requested_asset_tier") != "optional-high-resolution":
        errors.append("Classic HD owner asset fallback must request the optional tier")
    if artifact.get("resolved_asset_tier") != "original-indexed":
        errors.append("Classic HD owner asset fallback must resolve to original-indexed")
    if artifact.get("fallback_applied") is not True or artifact.get("optional_pack_present") is not False:
        errors.append("Classic HD owner asset fallback must prove an absent-pack fallback")
    if "absent" not in str(artifact.get("fallback_reason", "")):
        errors.append("Classic HD owner asset fallback must record the absent-pack reason")
    if artifact.get("status") != "fallback-pass":
        errors.append("Classic HD owner asset fallback status must be fallback-pass")
    if artifact.get("owner_payload_embedded") is not False or artifact.get("owner_inputs_external") is not True:
        errors.append("Classic HD owner asset fallback must keep owner inputs external")
    if artifact.get("scenario_frame_count") != 49 or artifact.get("upload_count") != 49:
        errors.append("Classic HD owner asset fallback must retain 49 frame/upload receipts")
    routes = artifact.get("routes")
    required_routes = {"idle", "town", "combat", "high_speed_flight", "death_restart"}
    if not isinstance(routes, dict) or set(routes) != required_routes:
        errors.append("Classic HD owner asset fallback must retain exactly the five bounded route cases")
    elif any(routes[name] != "pass" for name in required_routes):
        errors.append("Classic HD owner asset fallback contains a non-pass route case")
    for field in (
        "source_revision",
        "index_route_hash",
        "provenance_route_hash",
        "last_receipt_hash",
        "native_plugin_sha256",
        "log_sha256",
        "log_path",
        "fallback_reason",
    ):
        if not isinstance(artifact.get(field), str) or not artifact[field]:
            errors.append(f"Classic HD owner asset fallback is missing {field}")
    qualification_status = str(artifact.get("qualification_status", ""))
    if "remain open" not in qualification_status:
        errors.append("Classic HD owner asset fallback must keep optional consumption and broader gates open")
    return errors


def validate_classic_hd_owner_focus_pause_artifact(
    path: Path = CLASSIC_HD_OWNER_FOCUS_PAUSE_ARTIFACT,
) -> list[str]:
    errors: list[str] = []
    try:
        artifact = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"Classic HD owner focus/pause artifact read failed: {error}"]

    if artifact.get("schema_version") != 1:
        errors.append("Classic HD owner focus/pause schema_version must be 1")
    if artifact.get("kind") != "classic-hd-owner-focus-pause-probe/v1":
        errors.append("Classic HD owner focus/pause artifact kind is wrong")
    if artifact.get("mode") != "classic-hd" or artifact.get("profile") != "hd-640x480":
        errors.append("Classic HD owner focus/pause must target the 640x480 Classic HD route")
    if artifact.get("internal_width") != 640 or artifact.get("internal_height") != 480:
        errors.append("Classic HD owner focus/pause extent must be 640x480")
    if artifact.get("native_abi") != "sim-capi-v2":
        errors.append("Classic HD owner focus/pause must identify sim-capi-v2")
    if artifact.get("owner_payload_embedded") is not False or artifact.get("owner_inputs_external") is not True:
        errors.append("Classic HD owner focus/pause must keep owner inputs external")
    expected = {
        "baseline_generation": 2,
        "baseline_authoritative_tick": 1,
        "baseline_presentation_tick": 0,
        "resumed_authoritative_tick": 2,
        "resumed_presentation_tick": 1,
        "baseline_uploads": 1,
        "uploads_after_resume": 2,
        "lifecycle_transitions": 4,
        "output_restore_requests": 2,
    }
    for field, value in expected.items():
        if artifact.get(field) != value:
            errors.append(f"Classic HD owner focus/pause field {field} must be {value}")
    if artifact.get("native_pause_contract") is not True or artifact.get("status") != "lifecycle-pass":
        errors.append("Classic HD owner focus/pause must prove the native pause lifecycle")
    if artifact.get("suspended_capture_contract") != "capture-refused-while-suspended; authoritative-generation-and-tick-unchanged":
        errors.append("Classic HD owner focus/pause must prove suspended capture is refused before stepping")
    if artifact.get("fullscreen_alt_tab_status") not in {
        "open-os-window-capture-required",
        "captured-windowed-and-fullscreen-window-open-human-review",
    }:
        errors.append("Classic HD owner focus/pause must identify the OS window capture state")
    for field in ("source_revision", "native_plugin_sha256", "log_sha256", "log_path", "qualification_status"):
        if not isinstance(artifact.get(field), str) or not artifact[field]:
            errors.append(f"Classic HD owner focus/pause is missing {field}")
    if "remains open" not in str(artifact.get("qualification_status", "")):
        errors.append("Classic HD owner focus/pause must keep the OS capture gate explicit")
    return errors


def validate_classic_hd_owner_exit_sidecar_artifact(
    path: Path = CLASSIC_HD_OWNER_EXIT_SIDECAR_ARTIFACT,
) -> list[str]:
    errors: list[str] = []
    try:
        artifact = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"Classic HD owner exit sidecar artifact read failed: {error}"]

    if artifact.get("schema_version") != 1:
        errors.append("Classic HD owner exit sidecar schema_version must be 1")
    if artifact.get("kind") != "classic-hd-owner-exit-sidecar-probe/v1":
        errors.append("Classic HD owner exit sidecar artifact kind is wrong")
    if artifact.get("mode") != "classic-hd" or artifact.get("profile") != "hd-640x480":
        errors.append("Classic HD owner exit sidecar must target the 640x480 Classic HD route")
    if artifact.get("internal_width") != 640 or artifact.get("internal_height") != 480:
        errors.append("Classic HD owner exit sidecar extent must be 640x480")
    if artifact.get("native_abi") != "sim-capi-v2":
        errors.append("Classic HD owner exit sidecar must identify sim-capi-v2")
    if artifact.get("owner_payload_embedded") is not False or artifact.get("owner_inputs_external") is not True:
        errors.append("Classic HD owner exit sidecar must keep owner inputs external")
    expected = {
        "initial_generation": 2,
        "initial_tick": 0,
        "reset_generation": 3,
        "reset_tick": 0,
        "reset_authoritative_generation": 3,
        "reset_authoritative_tick": 1,
        "tasks_completed": 0,
        "exit_active": 0,
        "exit_complete": 0,
        "exit_result": 0,
    }
    for field, value in expected.items():
        if artifact.get(field) != value:
            errors.append(f"Classic HD owner exit sidecar field {field} must be {value}")
    if artifact.get("status") != "sidecar-pass":
        errors.append("Classic HD owner exit sidecar status must be sidecar-pass")
    if artifact.get("full_completion_route_exercised") is not False:
        errors.append("Classic HD owner exit sidecar must not claim a full completion route")
    if not isinstance(artifact.get("native_plugin_sha256"), str) or not artifact["native_plugin_sha256"]:
        errors.append("Classic HD owner exit sidecar is missing native_plugin_sha256")
    if "open" not in str(artifact.get("authoritative_completion_status", "")):
        errors.append("Classic HD owner exit sidecar must keep full completion open")
    for field in ("source_revision", "log_sha256", "log_path", "qualification_status"):
        if not isinstance(artifact.get(field), str) or not artifact[field]:
            errors.append(f"Classic HD owner exit sidecar is missing {field}")
    if "remains open" not in str(artifact.get("qualification_status", "")):
        errors.append("Classic HD owner exit sidecar must keep the completion gate explicit")
    return errors


def validate_classic_hd_owner_completion_replay_artifact(
    path: Path = CLASSIC_HD_OWNER_COMPLETION_REPLAY_ARTIFACT,
) -> list[str]:
    errors: list[str] = []
    try:
        artifact = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"Classic HD owner completion replay artifact read failed: {error}"]

    if artifact.get("schema_version") != 1:
        errors.append("Classic HD owner completion replay schema_version must be 1")
    if artifact.get("kind") != "classic-hd-owner-completion-replay-smoke/v1":
        errors.append("Classic HD owner completion replay artifact kind is wrong")
    if artifact.get("mode") != "classic-hd" or artifact.get("profile") != "hd-640x480":
        errors.append("Classic HD owner completion replay must target the 640x480 route")
    if artifact.get("internal_width") != 640 or artifact.get("internal_height") != 480:
        errors.append("Classic HD owner completion replay extent must be 640x480")
    if artifact.get("route") != "level1-completion-postgame":
        errors.append("Classic HD owner completion replay route is wrong")
    if artifact.get("native_abi") != "sim-capi-v2":
        errors.append("Classic HD owner completion replay must identify sim-capi-v2")
    if artifact.get("status") != "open":
        errors.append("Classic HD owner completion replay smoke must remain open")
    if artifact.get("evidence_authority") != "owner-external-input-journal":
        errors.append("Classic HD owner completion replay authority is wrong")
    if artifact.get("owner_payload_embedded") is not False or artifact.get("owner_inputs_external") is not True:
        errors.append("Classic HD owner completion replay must keep owner inputs external")

    command_count = artifact.get("journal_command_count")
    if not isinstance(command_count, int) or command_count < 1:
        errors.append("Classic HD owner completion replay command count must be positive")
    else:
        if artifact.get("journal_replayed_commands") != command_count:
            errors.append("Classic HD owner completion replay did not consume the full journal")
        if artifact.get("frame_count") != command_count or artifact.get("upload_count") != command_count:
            errors.append("Classic HD owner completion replay frame/upload count is wrong")
        if artifact.get("final_tick") != command_count:
            errors.append("Classic HD owner completion replay authoritative final tick is wrong")
        if artifact.get("source_final_tick") != command_count - 1:
            errors.append("Classic HD owner completion replay presentation final tick is wrong")

    if artifact.get("journal_schema") != "mc2-unity-playthrough/v1":
        errors.append("Classic HD owner completion replay journal schema is wrong")
    if artifact.get("journal_status") != "complete":
        errors.append("Classic HD owner completion replay journal must be complete")
    if artifact.get("journal_command_encoding") != "mc-native-command-10-byte-le/v1":
        errors.append("Classic HD owner completion replay command encoding is wrong")
    if artifact.get("allocation_epoch") != 1 or artifact.get("generation_stable") is not True:
        errors.append("Classic HD owner completion replay must prove one allocation epoch and stable generation")
    if artifact.get("generation") != artifact.get("authoritative_generation"):
        errors.append("Classic HD owner completion replay source and authoritative generations differ")
    if artifact.get("exit_sequence_active") is not False or artifact.get("exit_sequence_complete") is not False:
        errors.append("Classic HD owner completion replay smoke must not claim authored exit completion")
    if artifact.get("exit_result_code") != 0 or artifact.get("tasks_completed") != 0:
        errors.append("Classic HD owner completion replay smoke terminal fields are inconsistent")
    for field in (
        "journal_manifest_sha256",
        "journal_commands_sha256",
        "index_route_hash",
        "provenance_route_hash",
        "native_plugin_sha256",
        "runner_log_path",
        "runner_log_sha256",
        "reason",
    ):
        if not isinstance(artifact.get(field), str) or not artifact[field]:
            errors.append(f"Classic HD owner completion replay is missing {field}")
    if "does not reach" not in str(artifact.get("reason", "")):
        errors.append("Classic HD owner completion replay must state why completion remains open")
    return errors


def validate_classic_hd_owner_effects_matrix_artifact(
    path: Path = CLASSIC_HD_OWNER_EFFECTS_MATRIX_ARTIFACT,
) -> list[str]:
    errors: list[str] = []
    try:
        artifact = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"Classic HD owner effects matrix artifact read failed: {error}"]

    if artifact.get("schema_version") != 1:
        errors.append("Classic HD owner effects matrix schema_version must be 1")
    if artifact.get("kind") != "classic-hd-owner-effects-matrix-probe/v1":
        errors.append("Classic HD owner effects matrix artifact kind is wrong")
    if artifact.get("mode") != "classic-hd" or artifact.get("profile") != "hd-640x480":
        errors.append("Classic HD owner effects matrix must target the 640x480 route")
    if artifact.get("internal_width") != 640 or artifact.get("internal_height") != 480:
        errors.append("Classic HD owner effects matrix extent must be 640x480")
    if artifact.get("native_abi") != "sim-capi-v2":
        errors.append("Classic HD owner effects matrix must identify sim-capi-v2")
    if artifact.get("owner_payload_embedded") is not False or artifact.get("owner_inputs_external") is not True:
        errors.append("Classic HD owner effects matrix must keep owner inputs external")
    expected_cases = {
        "off": ("6655b64aacd9c0dd", "2fc796aaa4ec61f8"),
        "spatial-2x2": ("97919bd7405795c1", "a7efb522f195a094"),
        "tablesn": ("6655b64aacd9c0dd", "2fc796aaa4ec61f8"),
    }
    cases = artifact.get("blur_cases")
    if not isinstance(cases, list) or len(cases) != len(expected_cases):
        errors.append("Classic HD owner effects matrix must contain all three blur cases")
    else:
        seen: set[str] = set()
        for case in cases:
            name = case.get("blur") if isinstance(case, dict) else None
            if name not in expected_cases:
                errors.append(f"Classic HD owner effects matrix has unexpected blur case {name!r}")
                continue
            seen.add(name)
            index_hash, provenance_hash = expected_cases[name]
            if case.get("reflections") is not True or case.get("dynamic_lighting") is not True:
                errors.append(f"Classic HD owner effects case {name} must keep reflections and lighting enabled")
            if case.get("status") != "pass" or case.get("uploads") != 1 or case.get("allocation_epoch") != 1:
                errors.append(f"Classic HD owner effects case {name} upload/allocation contract is wrong")
            if case.get("effective_postprocess_mode") != 0:
                errors.append(f"Classic HD owner effects case {name} effective postprocess mode is wrong")
            if case.get("index_hash") != index_hash or case.get("provenance_hash") != provenance_hash:
                errors.append(f"Classic HD owner effects case {name} hashes do not match the receipt")
        if seen != set(expected_cases):
            errors.append("Classic HD owner effects matrix must include exactly off, spatial-2x2, and tablesn")
    for field in ("reflection_off", "dynamic_lighting_off"):
        rejection = artifact.get(field)
        if not isinstance(rejection, dict) or rejection.get("status") != "rejected":
            errors.append(f"Classic HD owner effects matrix must reject {field}")
        elif "requires reflections" not in str(rejection.get("reason", "")):
            errors.append(f"Classic HD owner effects matrix {field} reason is not explicit")
    if artifact.get("visual_capture_status") != "open-corpus-capture-required":
        errors.append("Classic HD owner effects matrix must keep visual capture open")
    if artifact.get("status") != "effects-settings-pass":
        errors.append("Classic HD owner effects matrix status must be effects-settings-pass")
    for field in ("source_revision", "native_plugin_sha256", "log_sha256", "log_path", "qualification_status"):
        if not isinstance(artifact.get(field), str) or not artifact[field]:
            errors.append(f"Classic HD owner effects matrix is missing {field}")
    if "remain open" not in str(artifact.get("qualification_status", "")):
        errors.append("Classic HD owner effects matrix must keep visual/high-speed gates explicit")
    return errors


def validate_classic_hd_owner_high_speed_artifact(
    path: Path = CLASSIC_HD_OWNER_HIGH_SPEED_ARTIFACT,
) -> list[str]:
    errors: list[str] = []
    try:
        artifact = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"Classic HD owner high-speed artifact read failed: {error}"]

    if artifact.get("schema_version") != 1:
        errors.append("Classic HD owner high-speed schema_version must be 1")
    if artifact.get("kind") != "classic-hd-owner-high-speed-probe/v1":
        errors.append("Classic HD owner high-speed artifact kind is wrong")
    if artifact.get("native_abi") != "sim-capi-v2":
        errors.append("Classic HD owner high-speed artifact must identify sim-capi-v2")
    if artifact.get("owner_payload_embedded") is not False or artifact.get("owner_inputs_external") is not True:
        errors.append("Classic HD owner high-speed artifact must keep owner inputs external")
    if artifact.get("configured_blur") != "off":
        errors.append("Classic HD owner high-speed probe must run with blur off")
    if artifact.get("frames_per_pass") != 32 or artifact.get("pass_count") != 2:
        errors.append("Classic HD owner high-speed probe must retain two 32-frame passes")
    if artifact.get("upload_count") != 64 or artifact.get("allocation_epoch") != 1:
        errors.append("Classic HD owner high-speed probe must prove 64 uploads and one allocation epoch")
    if artifact.get("reset_replay_match") is not True:
        errors.append("Classic HD owner high-speed probe reset replay did not match")
    if artifact.get("gpu_readback_fence") != "not-captured":
        errors.append("Classic HD owner high-speed probe must state that GPU readback fencing is not captured")
    if artifact.get("status") != "open":
        errors.append("Classic HD owner high-speed probe must remain open")
    if not isinstance(artifact.get("failure_reason"), str) or not artifact["failure_reason"]:
        errors.append("Classic HD owner high-speed probe must record why automatic mode remains open")
    for field in ("source_revision", "native_plugin_sha256", "captured_at_utc"):
        if not isinstance(artifact.get(field), str) or not artifact[field]:
            errors.append(f"Classic HD owner high-speed artifact is missing {field}")

    profiles = artifact.get("profiles")
    expected_profiles = {
        "hd-640x480": (640, 480),
        "hd-960x720": (960, 720),
    }
    if not isinstance(profiles, list) or len(profiles) != len(expected_profiles):
        errors.append("Classic HD owner high-speed artifact must contain both qualified profiles")
    else:
        seen: set[str] = set()
        for profile in profiles:
            name = profile.get("profile") if isinstance(profile, dict) else None
            if name not in expected_profiles:
                errors.append(f"Classic HD owner high-speed artifact has unexpected profile {name!r}")
                continue
            seen.add(name)
            width, height = expected_profiles[name]
            if profile.get("internal_width") != width or profile.get("internal_height") != height:
                errors.append(f"Classic HD owner high-speed profile {name} extent is wrong")
            if profile.get("speed_spell_present") is not False:
                errors.append(f"Classic HD owner high-speed profile {name} must prove no synthetic Speed-jar activation")
            if profile.get("max_target_speed") != 80 or profile.get("max_actual_speed") != 80:
                errors.append(f"Classic HD owner high-speed profile {name} must record the observed speed limit of 80")
            if profile.get("max_effective_postprocess_mode") != 0:
                errors.append(f"Classic HD owner high-speed profile {name} unexpectedly reached automatic mode")
            if profile.get("automatic_high_speed_frames") != 0:
                errors.append(f"Classic HD owner high-speed profile {name} unexpectedly recorded high-speed frames")
            if profile.get("reset_replay_match") is not True or profile.get("status") != "open":
                errors.append(f"Classic HD owner high-speed profile {name} must retain an open deterministic result")
            for field in ("index_route_hash", "provenance_route_hash", "log_sha256", "log_path"):
                if not isinstance(profile.get(field), str) or not profile[field]:
                    errors.append(f"Classic HD owner high-speed profile {name} is missing {field}")
        if seen != set(expected_profiles):
            errors.append("Classic HD owner high-speed artifact must include exactly 640x480 and 960x720")
    return errors


def validate_classic_hd_owner_performance_artifact(
    path: Path = CLASSIC_HD_OWNER_PERFORMANCE_ARTIFACT,
) -> list[str]:
    errors: list[str] = []
    try:
        artifact = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"Classic HD owner performance artifact read failed: {error}"]

    if artifact.get("schema_version") != 1:
        errors.append("Classic HD owner performance schema_version must be 1")
    if artifact.get("kind") != "classic-hd-owner-performance-smoke/v1":
        errors.append("Classic HD owner performance artifact kind is wrong")
    if artifact.get("execution_context") != "UnityEditor-batchmode":
        errors.append("Classic HD owner performance context must be explicit")
    if artifact.get("native_abi") != "sim-capi-v2":
        errors.append("Classic HD owner performance must identify sim-capi-v2")
    if artifact.get("owner_payload_embedded") is not False or artifact.get("owner_inputs_external") is not True:
        errors.append("Classic HD owner performance must keep owner inputs external")
    if artifact.get("warmup_frames") != 8 or artifact.get("measurement_frames") != 64:
        errors.append("Classic HD owner performance must retain the bounded 8+64 frame sample")
    if artifact.get("upload_count_including_warmup") != 72 or artifact.get("allocation_epoch") != 1:
        errors.append("Classic HD owner performance must prove 72 uploads and one allocation epoch")
    if artifact.get("presentation_budget_ns") != 33333333:
        errors.append("Classic HD owner performance must identify the 33.3 ms budget")
    if artifact.get("gpu_timing") != "not-captured" or artifact.get("status") != "open":
        errors.append("Classic HD owner performance must keep GPU timing and promotion open")
    if not isinstance(artifact.get("failure_reason"), str) or not artifact["failure_reason"]:
        errors.append("Classic HD owner performance must record its open reason")
    for field in ("source_revision", "native_plugin_sha256", "captured_at_utc"):
        if not isinstance(artifact.get(field), str) or not artifact[field]:
            errors.append(f"Classic HD owner performance is missing {field}")

    profiles = artifact.get("profiles")
    expected_profiles = {"hd-640x480": (640, 480), "hd-960x720": (960, 720)}
    if not isinstance(profiles, list) or len(profiles) != len(expected_profiles):
        errors.append("Classic HD owner performance must contain both candidate profiles")
    else:
        seen: set[str] = set()
        for profile in profiles:
            name = profile.get("profile") if isinstance(profile, dict) else None
            if name not in expected_profiles:
                errors.append(f"Classic HD owner performance has unexpected profile {name!r}")
                continue
            seen.add(name)
            width, height = expected_profiles[name]
            if profile.get("internal_width") != width or profile.get("internal_height") != height:
                errors.append(f"Classic HD owner performance profile {name} extent is wrong")
            if profile.get("start_generation") != profile.get("end_generation"):
                errors.append(f"Classic HD owner performance profile {name} changed generation")
            if profile.get("start_tick") != 8 or profile.get("end_tick") != 71:
                errors.append(f"Classic HD owner performance profile {name} tick window is wrong")
            if profile.get("monotonic") is not True or profile.get("hashes_present") is not True:
                errors.append(f"Classic HD owner performance profile {name} route contract is not deterministic")
            if profile.get("uploads") != 72 or profile.get("allocation_epoch") != 1:
                errors.append(f"Classic HD owner performance profile {name} upload/allocation contract is wrong")
            if profile.get("steady_allocations") is not True or profile.get("route_contract") is not True:
                errors.append(f"Classic HD owner performance profile {name} steady route contract is not proven")
            budget_pass = profile.get("budget_pass")
            p95_total_ns = profile.get("p95_total_ns")
            if not isinstance(budget_pass, bool):
                errors.append(f"Classic HD owner performance profile {name} must state budget_pass")
            if not isinstance(p95_total_ns, int) or p95_total_ns <= 0:
                errors.append(f"Classic HD owner performance profile {name} must record a positive p95 total")
            elif budget_pass is True and p95_total_ns > artifact["presentation_budget_ns"]:
                errors.append(f"Classic HD owner performance profile {name} claims budget pass above the presentation budget")
            elif budget_pass is False and p95_total_ns <= artifact["presentation_budget_ns"]:
                errors.append(f"Classic HD owner performance profile {name} rejects a p95 total within the presentation budget")
            if profile.get("p95_allocation_bytes") != 0 or profile.get("max_allocation_bytes") != 0:
                errors.append(f"Classic HD owner performance profile {name} must prove zero measured steady allocations")
            for field in ("index_route_hash", "provenance_route_hash", "log_path", "log_sha256"):
                if not isinstance(profile.get(field), str) or not profile[field]:
                    errors.append(f"Classic HD owner performance profile {name} is missing {field}")
            expected_status = (
                "supported-owner-real-time-budget"
                if budget_pass is True
                else "rejected-by-owner-real-time-budget"
            )
            if profile.get("status") != expected_status:
                errors.append(
                    f"Classic HD owner performance profile {name} status must be {expected_status}"
                )
        if seen != set(expected_profiles):
            errors.append("Classic HD owner performance must include exactly 640x480 and 960x720")
    return errors


def validate_classic_hd_owner_gpu_fence_artifact(
    path: Path = CLASSIC_HD_OWNER_GPU_FENCE_ARTIFACT,
) -> list[str]:
    errors: list[str] = []
    try:
        artifact = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"Classic HD owner GPU fence artifact read failed: {error}"]

    if artifact.get("schema_version") != 1:
        errors.append("Classic HD owner GPU fence schema_version must be 1")
    if artifact.get("kind") != "classic-hd-owner-gpu-fence-probe/v1":
        errors.append("Classic HD owner GPU fence artifact kind is wrong")
    if artifact.get("execution_context") != "UnityEditor-batchmode":
        errors.append("Classic HD owner GPU fence context must be explicit")
    if artifact.get("native_abi") != "sim-capi-v2":
        errors.append("Classic HD owner GPU fence must identify sim-capi-v2")
    if artifact.get("owner_payload_embedded") is not False or artifact.get("owner_inputs_external") is not True:
        errors.append("Classic HD owner GPU fence must keep owner inputs external")
    if artifact.get("gpu_timing") != "not-captured":
        errors.append("Classic HD owner GPU fence must not claim GPU timing")
    if artifact.get("readback_status") != "pass" or artifact.get("fence_status") != "open-platform-unsupported":
        errors.append("Classic HD owner GPU fence must separate readback pass from open fence status")
    if artifact.get("status") != "open":
        errors.append("Classic HD owner GPU fence must remain open")
    if not isinstance(artifact.get("failure_reason"), str) or not artifact["failure_reason"]:
        errors.append("Classic HD owner GPU fence must record its open reason")
    for field in ("source_revision", "native_plugin_sha256", "captured_at_utc"):
        if not isinstance(artifact.get(field), str) or not artifact[field]:
            errors.append(f"Classic HD owner GPU fence is missing {field}")

    profiles = artifact.get("profiles")
    expected_profiles = {"hd-640x480": (640, 480), "hd-960x720": (960, 720)}
    if not isinstance(profiles, list) or len(profiles) != len(expected_profiles):
        errors.append("Classic HD owner GPU fence must contain both candidate profiles")
    else:
        seen: set[str] = set()
        for profile in profiles:
            name = profile.get("profile") if isinstance(profile, dict) else None
            if name not in expected_profiles:
                errors.append(f"Classic HD owner GPU fence has unexpected profile {name!r}")
                continue
            seen.add(name)
            width, height = expected_profiles[name]
            if profile.get("internal_width") != width or profile.get("internal_height") != height:
                errors.append(f"Classic HD owner GPU fence profile {name} extent is wrong")
            if profile.get("frames") != 2 or profile.get("uploads") != 2 or profile.get("allocation_epoch") != 1:
                errors.append(f"Classic HD owner GPU fence profile {name} upload contract is wrong")
            if profile.get("hashes_differ") is not True:
                errors.append(f"Classic HD owner GPU fence profile {name} must prove frame hashes differ")
            if profile.get("readback_error_a") is not False or profile.get("readback_error_b") is not False:
                errors.append(f"Classic HD owner GPU fence profile {name} has a readback error")
            if any(profile.get(field) is not False for field in ("fence_valid_a", "fence_passed_a", "fence_valid_b", "fence_passed_b")):
                errors.append(f"Classic HD owner GPU fence profile {name} must keep the unsupported fence open")
            if profile.get("status") != "readback-pass-fence-open":
                errors.append(f"Classic HD owner GPU fence profile {name} status is wrong")
            for field in ("raw_hash_a", "raw_hash_b", "resolved_hash_a", "resolved_hash_b", "log_path", "log_sha256"):
                if not isinstance(profile.get(field), str) or not profile[field]:
                    errors.append(f"Classic HD owner GPU fence profile {name} is missing {field}")
        if seen != set(expected_profiles):
            errors.append("Classic HD owner GPU fence must include exactly 640x480 and 960x720")
    return errors


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def validate_owner_visual_capture_artifact(
    path: Path,
    capture_dir: Path,
    mode: str,
    profile: str,
    width: int,
    height: int,
) -> list[str]:
    """Validate review-gated owner PNGs without promoting a presentation corpus."""
    errors: list[str] = []
    try:
        artifact = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"owner visual capture artifact read failed: {error}"]

    if artifact.get("schema_version") != 1:
        errors.append("owner visual capture schema_version must be 1")
    if artifact.get("mode") != mode:
        errors.append(f"owner visual capture mode must be {mode}")
    if artifact.get("profile") != profile:
        errors.append(f"owner visual capture profile must be {profile}")
    if artifact.get("capture_status") != "captured-for-human-review":
        errors.append("owner visual capture must remain captured-for-human-review")
    if artifact.get("visual_review_status") != "not-reviewed":
        errors.append("owner visual capture must remain not-reviewed")
    if artifact.get("owner_payload_embedded") is not False:
        errors.append("owner visual capture must not embed owner payload")
    if artifact.get("owner_inputs_external") is not True:
        errors.append("owner visual capture must identify owner inputs as external")
    for field in ("unity_editor", "graphics_api", "graphics_device", "graphics_driver"):
        if not isinstance(artifact.get(field), str) or not artifact[field]:
            errors.append(f"owner visual capture is missing {field}")

    captures = artifact.get("captures")
    if not isinstance(captures, list) or len(captures) != len(OWNER_VISUAL_CAPTURE_ROUTES):
        errors.append("owner visual capture must contain exactly seven Night routes")
        return errors

    seen_routes: set[str] = set()
    seen_files: set[str] = set()
    for entry in captures:
        if not isinstance(entry, dict):
            errors.append("owner visual capture contains a non-object route entry")
            continue
        route_id = entry.get("route_id")
        if route_id not in OWNER_VISUAL_CAPTURE_ROUTES:
            errors.append(f"owner visual capture has unexpected route {route_id!r}")
            continue
        if route_id in seen_routes:
            errors.append(f"owner visual capture repeats route {route_id}")
        seen_routes.add(route_id)
        expected_file, expected_cast = OWNER_VISUAL_CAPTURE_ROUTES[route_id]
        file_name = entry.get("file_name")
        if file_name != expected_file:
            errors.append(f"owner visual capture route {route_id} file name is wrong")
            continue
        if file_name in seen_files:
            errors.append(f"owner visual capture repeats file {file_name}")
        seen_files.add(file_name)
        if entry.get("capture_name") != Path(file_name).stem:
            errors.append(f"owner visual capture route {route_id} capture name is wrong")
        if entry.get("cast_observed") is not expected_cast:
            errors.append(f"owner visual capture route {route_id} cast observation is wrong")
        if entry.get("width") != width or entry.get("height") != height:
            errors.append(f"owner visual capture route {route_id} extent is wrong")
        if not isinstance(entry.get("frame_count"), int) or entry["frame_count"] <= 0:
            errors.append(f"owner visual capture route {route_id} frame count must be positive")
        for field in ("source_generation", "source_tick", "resolved_png_hash", "file_sha256"):
            if not isinstance(entry.get(field), str) or not entry[field]:
                errors.append(f"owner visual capture route {route_id} is missing {field}")
        resolved_hash = entry.get("resolved_png_hash")
        if (
            not isinstance(resolved_hash, str)
            or len(resolved_hash) != 16
            or any(character not in "0123456789abcdefABCDEF" for character in resolved_hash)
        ):
            errors.append(f"owner visual capture route {route_id} resolved hash is invalid")

        if mode == "classic-hd":
            if not isinstance(entry.get("upload_count"), int) or entry["upload_count"] < entry.get("frame_count", 0):
                errors.append(f"owner visual capture route {route_id} upload count is invalid")
        else:
            for field in ("active_entity_count", "active_visual_count"):
                if not isinstance(entry.get(field), int) or entry[field] <= 0:
                    errors.append(f"owner visual capture route {route_id} {field} must be positive")

        file_path = capture_dir / file_name
        if file_path.parent != capture_dir or not file_path.is_file():
            errors.append(f"owner visual capture route {route_id} PNG is missing")
            continue
        try:
            if file_path.read_bytes()[:8] != b"\x89PNG\r\n\x1a\n":
                errors.append(f"owner visual capture route {route_id} is not a PNG")
            if _sha256_file(file_path) != entry.get("file_sha256", "").lower():
                errors.append(f"owner visual capture route {route_id} PNG SHA-256 does not match receipt")
        except OSError as error:
            errors.append(f"owner visual capture route {route_id} PNG read failed: {error}")

    if seen_routes != set(OWNER_VISUAL_CAPTURE_ROUTES):
        errors.append("owner visual capture must include every expected Night route")
    return errors


def validate_classic_hd_owner_visual_capture_artifact(
    path: Path = CLASSIC_HD_OWNER_VISUAL_CAPTURE_ARTIFACT,
    capture_dir: Path = CLASSIC_HD_OWNER_VISUAL_CAPTURE_DIR,
) -> list[str]:
    return validate_owner_visual_capture_artifact(
        path, capture_dir, "classic-hd", "hd-640x480", 640, 480
    )


def _validate_classic_hd_owner_os_window_receipt(
    artifact: dict[str, Any],
    label: str,
    expected_window_case: str | None = None,
) -> list[str]:
    errors: list[str] = []
    if artifact.get("schema_version") != 1:
        errors.append(f"{label} schema_version must be 1")
    if artifact.get("kind") != "classic-hd-owner-os-window-lifecycle/v1":
        errors.append(f"{label} kind is wrong")
    if artifact.get("mode") != "classic-hd" or artifact.get("profile") != "hd-640x480":
        errors.append(f"{label} must target hd-640x480")
    if artifact.get("owner_payload_embedded") is not False or artifact.get("owner_inputs_external") is not True:
        errors.append(f"{label} must keep owner inputs external")
    if artifact.get("required_consecutive_stable_frames") != 2:
        errors.append(f"{label} must require two stable rendered frames")
    if expected_window_case is not None and artifact.get("window_case") != expected_window_case:
        errors.append(f"{label} must target {expected_window_case}")
    status = str(artifact.get("qualification_status", ""))
    if "open" not in status and "pending" not in status:
        errors.append(f"{label} must remain open until final review")
    if artifact.get("initial_output_stable") is True or artifact.get("post_regain_output_stable") is True:
        if artifact.get("actual_focus_callback_count", 0) <= 0:
            errors.append(f"{label} stable evidence requires an actual focus callback")

    events = artifact.get("events")
    if not isinstance(events, list):
        errors.append(f"{label} events must be a list")
    else:
        for event in events:
            if not isinstance(event, dict):
                errors.append(f"{label} contains a non-object event")
                continue
            if event.get("synthetic_qualification") is True:
                errors.append(f"{label} must reject synthetic events")
            if event.get("actual_os_callback") is True and not str(event.get("observation", "")).startswith(
                "unity-on-application-"
            ):
                errors.append(f"{label} actual callbacks must be named Unity application callbacks")
    return errors


def validate_classic_hd_owner_os_window_lifecycle_artifact(
    path: Path = CLASSIC_HD_OWNER_OS_WINDOW_LIFECYCLE_ARTIFACT,
) -> list[str]:
    """Validate the two real-player OS-window receipts without closing the gate."""
    errors: list[str] = []
    try:
        artifact = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"Classic HD OS window lifecycle artifact read failed: {error}"]

    errors.extend(_validate_classic_hd_owner_os_window_receipt(artifact, "Classic HD OS window lifecycle artifact"))

    required_cases = artifact.get("required_external_cases")
    if required_cases != ["windowed", "fullscreen-window"]:
        errors.append("Classic HD OS window lifecycle must require windowed and fullscreen-window cases")
    captured_cases = artifact.get("captured_external_cases")
    if not isinstance(captured_cases, list) or "windowed" not in captured_cases:
        errors.append("Classic HD OS window lifecycle must record the captured windowed case")

    try:
        fullscreen_artifact = json.loads(
            CLASSIC_HD_OWNER_OS_WINDOW_FULLSCREEN_ARTIFACT.read_text(encoding="utf-8")
        )
    except (OSError, json.JSONDecodeError) as error:
        errors.append(f"Classic HD fullscreen-window lifecycle artifact read failed: {error}")
    else:
        errors.extend(
            _validate_classic_hd_owner_os_window_receipt(
                fullscreen_artifact,
                "Classic HD fullscreen-window lifecycle artifact",
                "fullscreen-window",
            )
        )
    return errors


def validate_unity_enhanced_owner_visual_capture_artifact(
    path: Path = UNITY_ENHANCED_OWNER_VISUAL_CAPTURE_ARTIFACT,
    capture_dir: Path = UNITY_ENHANCED_OWNER_VISUAL_CAPTURE_DIR,
) -> list[str]:
    return validate_owner_visual_capture_artifact(
        path, capture_dir, "unity-enhanced", "level1-night-unity-enhanced", 1280, 720
    )


def validate_classic_hd_visual_capture_tooling() -> list[str]:
    """Keep the owner capture path present without promoting visual evidence."""
    errors: list[str] = []
    presentation_path = LAB_ROOT / "Assets" / "Scripts" / "ClassicHdPresentation.cs"
    editor_path = LAB_ROOT / "Assets" / "Editor" / "CreatePresentationLabAssets.cs"
    readme_path = LAB_ROOT / "README.md"
    required_snippets = {
        presentation_path: (
            "CaptureResolvedFramePng",
            "Graphics.Blit(IndexTexture, resolved, IndexedMaterial)",
            "EncodeToPNG",
            "screenshot cannot advance the authoritative Rust session",
        ),
        editor_path: (
            "RunOwnerClassicHdVisualCaptureProbe",
            "-mc-capture-dir",
            "captured-for-human-review",
            "visual_review_status=not-reviewed",
            "owner_inputs_external=true",
        ),
        readme_path: (
            "RunOwnerClassicHdVisualCaptureProbe",
            "classic-hd-owner-visual-capture-receipt.json",
            "placeholder corpus manifests",
        ),
    }
    for path, snippets in required_snippets.items():
        try:
            content = path.read_text(encoding="utf-8")
        except OSError as error:
            errors.append(f"visual capture tooling file read failed: {error}")
            continue
        for snippet in snippets:
            if snippet not in content:
                errors.append(f"visual capture tooling is missing {snippet!r} in {path.name}")
    return errors


def validate_unity_enhanced_visual_capture_tooling() -> list[str]:
    """Keep the owner Enhanced camera capture path separate and review-gated."""
    errors: list[str] = []
    root_path = LAB_ROOT / "Assets" / "Scripts" / "UnityEnhancedWorldRoot.cs"
    editor_path = LAB_ROOT / "Assets" / "Editor" / "CreatePresentationLabAssets.cs"
    readme_path = LAB_ROOT / "README.md"
    required_snippets = {
        root_path: (
            "CapturePresentationFramePng",
            "camera.Render()",
            "never advances or mutates the authoritative owner session",
        ),
        editor_path: (
            "RunOwnerUnityEnhancedVisualCaptureProbe",
            "unity-enhanced-owner-visual-capture-receipt.json",
            "captured-for-human-review",
            "visual_review_status=not-reviewed",
            "owner_inputs_external=true",
        ),
        readme_path: (
            "RunOwnerUnityEnhancedVisualCaptureProbe",
            "unity-enhanced-owner-visual-capture-receipt.json",
            "placeholder corpus manifests",
        ),
    }
    for path, snippets in required_snippets.items():
        try:
            content = path.read_text(encoding="utf-8")
        except OSError as error:
            errors.append(f"Unity Enhanced visual capture tooling file read failed: {error}")
            continue
        for snippet in snippets:
            if snippet not in content:
                errors.append(f"Unity Enhanced visual capture tooling is missing {snippet!r} in {path.name}")
    return errors


def validate_classic_hd_os_window_lifecycle_tooling() -> list[str]:
    """Ensure runtime OS callbacks have a distinct, review-gated trace path."""
    errors: list[str] = []
    router_path = LAB_ROOT / "Assets" / "Scripts" / "PresentationModeRouter.cs"
    recorder_path = LAB_ROOT / "Assets" / "Scripts" / "ClassicHdWindowLifecycleRecorder.cs"
    readme_path = LAB_ROOT / "README.md"
    required_snippets = {
        router_path: (
            "OnApplicationFocus",
            "RecordUnityLifecycleCallback",
            "-mc-lifecycle-receipt",
            "FullScreenMode.FullScreenWindow",
        ),
        recorder_path: (
            "actual_os_callback",
            "synthetic_qualification",
            "ObserveRenderedFrame",
            "RequiredStableFrames = 2",
            "File.WriteAllText",
            "classic-hd-owner-os-window-lifecycle/v1",
        ),
        readme_path: (
            "-mc-lifecycle-receipt",
            "classic-hd-owner-os-window-lifecycle.json",
            "external window-manager action",
        ),
    }
    for path, snippets in required_snippets.items():
        try:
            content = path.read_text(encoding="utf-8")
        except OSError as error:
            errors.append(f"OS window lifecycle tooling file read failed: {error}")
            continue
        for snippet in snippets:
            if snippet not in content:
                errors.append(f"OS window lifecycle tooling is missing {snippet!r} in {path.name}")
    return errors


def validate_standalone_player_build_tooling() -> list[str]:
    """Ensure the optional presentation lab has a reproducible player build path."""
    errors: list[str] = []
    editor_path = LAB_ROOT / "Assets" / "Editor" / "CreatePresentationLabAssets.cs"
    readme_path = LAB_ROOT / "README.md"
    required_snippets = {
        editor_path: (
            "BuildStandaloneLinuxPlayer",
            "BuildPipeline.BuildPlayer",
            "BuildTarget.StandaloneLinux64",
            "BuildOptions.StrictMode",
            "refusing to overwrite an existing standalone player output",
        ),
        readme_path: (
            "BuildStandaloneLinuxPlayer",
            "StandaloneLinux64",
            "owner package, closure, and asset-cache paths remain runtime arguments",
        ),
    }
    for path, snippets in required_snippets.items():
        try:
            content = path.read_text(encoding="utf-8")
        except OSError as error:
            errors.append(f"standalone player build tooling file read failed: {error}")
            continue
        for snippet in snippets:
            if snippet not in content:
                errors.append(f"standalone player build tooling is missing {snippet!r} in {path.name}")
    return errors


def validate_profile_sequence_failure_artifact(
    path: Path = PROFILE_960_SEQUENCE_FAILURE_ARTIFACT,
) -> list[str]:
    errors: list[str] = []
    try:
        artifact = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"profile sequence failure artifact read failed: {error}"]

    if artifact.get("schema_version") != 1:
        errors.append("profile sequence failure artifact schema_version must be 1")
    if artifact.get("kind") != "classic-hd-owner-route-profile-sequence-failure/v1":
        errors.append("profile sequence failure artifact kind is wrong")
    if artifact.get("profile") != "classic-hd-960x720":
        errors.append("profile sequence failure artifact must target 960x720")
    if artifact.get("internal_width") != 960 or artifact.get("internal_height") != 720:
        errors.append("profile sequence failure artifact extent must be 960x720")
    if artifact.get("owner_payload_embedded") is not False or artifact.get("owner_inputs_external") is not True:
        errors.append("profile sequence failure artifact must keep owner inputs external")
    if artifact.get("target_frame_count") != 64:
        errors.append("profile sequence failure artifact must describe the 64-frame two-pass probe")
    if artifact.get("observed_frame_receipts") != 0:
        errors.append("profile sequence failure artifact must not claim a completed frame receipt")
    if artifact.get("process_exit_status") != 137:
        errors.append("profile sequence failure artifact must retain the bounded-run termination status")
    if artifact.get("termination") != "explicitly-stopped-after-bounded-run":
        errors.append("profile sequence failure artifact termination is not explicit")
    if not isinstance(artifact.get("failure_reason"), str) or not artifact["failure_reason"]:
        errors.append("profile sequence failure artifact is missing its failure reason")
    for field in ("source_commit", "native_plugin_sha256", "log_sha256"):
        if not isinstance(artifact.get(field), str) or not artifact[field]:
            errors.append(f"profile sequence failure artifact is missing {field}")
    status = str(artifact.get("qualification_status", ""))
    if "superseded" in status:
        if not isinstance(artifact.get("superseded_by"), str) or not artifact["superseded_by"]:
            errors.append("superseded profile sequence failure artifact must name its replacement")
    elif "not-promoted" not in status or "open" not in status:
        errors.append("profile sequence failure artifact must remain explicitly unpromoted/open")
    return errors


def _validator_result(name: str, callback: Callable[[], Any], checks: list[dict[str, Any]]) -> None:
    try:
        result = callback()
    except (OSError, ValueError, KeyError, ImportError) as error:
        _check(checks, name, False, str(error))
        return

    if isinstance(result, list):
        _check(checks, name, not result, "; ".join(str(error) for error in result) or "green")
    elif isinstance(result, dict):
        status = result.get("status")
        passed = status == "PASS" or result.get("pass") is True
        _check(checks, name, passed, status or ("green" if passed else "failed"))
    else:
        _check(checks, name, bool(result), str(result))


def verify() -> dict[str, Any]:
    checks: list[dict[str, Any]] = []
    mode_contract = _load_module("mc_mode_contract", PRESENTATION_ROOT / "verify_mode_contract.py")
    corpora = _load_module("mc_presentation_corpora", PRESENTATION_ROOT / "verify_presentation_corpora.py")
    classic_spec = _load_module("mc_classic_hd_spec", PRESENTATION_ROOT / "validate_classic_hd_spec.py")
    asset_tiers = _load_module("mc_asset_tiers", PRESENTATION_ROOT / "validate_asset_tiers.py")
    enhanced_coverage = _load_module("mc_enhanced_coverage", PRESENTATION_ROOT / "validate_enhanced_coverage.py")
    classic_matrix = _load_module("mc_classic_hd_matrix", Path(__file__).with_name("validate_classic_hd_matrix.py"))
    lab = _load_module("mc_presentation_lab", LAB_ROOT / "qualification" / "verify_presentation_lab.py")
    enhanced = _load_module("mc_unity_enhanced", LAB_ROOT / "qualification" / "verify_unity_enhanced.py")

    contract_path = PRESENTATION_ROOT / "mode-contract.json"
    contract = mode_contract.load_contract(contract_path)
    _validator_result("three-mode contract schema", lambda: mode_contract.validate_contract(contract), checks)
    for mode, kwargs in (
        ("exact", {}),
        ("classic-hd", {"internal_width": 640, "internal_height": 480}),
        ("unity-enhanced", {}),
    ):
        _validator_result(
            f"{mode} invocation contract",
            lambda mode=mode, kwargs=kwargs: mode_contract.validate_invocation(contract, mode=mode, **kwargs),
            checks,
        )

    _validator_result("mode-separated presentation corpora", corpora.verify, checks)
    _validator_result("Classic HD specification", lambda: classic_spec.validate_spec(classic_spec.load_spec()), checks)
    _validator_result(
        "Classic HD asset tiers",
        lambda: asset_tiers.validate(
            json.loads((PRESENTATION_ROOT / "asset-tier-schema.json").read_text(encoding="utf-8")),
            json.loads((PRESENTATION_ROOT / "asset-tier-manifest.json").read_text(encoding="utf-8")),
        ),
        checks,
    )
    _validator_result(
        "Unity Enhanced coverage",
        lambda: enhanced_coverage.validate_matrix(enhanced_coverage.load_matrix()),
        checks,
    )
    _validator_result("Classic HD resolution matrix", classic_matrix.validate, checks)
    _validator_result("isolated Unity presentation lab", lambda: lab.verify(LAB_ROOT), checks)
    _validator_result("Unity Enhanced lifecycle shell", lambda: enhanced.verify(LAB_ROOT), checks)
    _validator_result("owner-bound Classic HD sequence artifact", validate_sequence_artifact, checks)
    _validator_result(
        "owner-bound Classic HD completion artifact",
        validate_classic_hd_owner_completion_artifact,
        checks,
    )
    _validator_result(
        "owner-bound Unity Enhanced sequence artifact",
        validate_enhanced_owner_sequence_artifact,
        checks,
    )
    _validator_result(
        "owner-bound Unity Enhanced scenario artifact",
        validate_enhanced_owner_scenario_artifact,
        checks,
    )
    _validator_result(
        "owner-bound Classic HD scenario artifact",
        validate_classic_hd_owner_scenario_artifact,
        checks,
    )
    _validator_result(
        "owner-bound Classic HD asset-tier fallback artifact",
        validate_classic_hd_owner_asset_fallback_artifact,
        checks,
    )
    _validator_result(
        "owner-bound Classic HD focus/pause artifact",
        validate_classic_hd_owner_focus_pause_artifact,
        checks,
    )
    _validator_result(
        "owner-bound Classic HD OS window lifecycle artifact",
        validate_classic_hd_owner_os_window_lifecycle_artifact,
        checks,
    )
    _validator_result(
        "owner-bound Classic HD exit-sidecar artifact",
        validate_classic_hd_owner_exit_sidecar_artifact,
        checks,
    )
    _validator_result(
        "owner-bound Classic HD completion replay artifact",
        validate_classic_hd_owner_completion_replay_artifact,
        checks,
    )
    _validator_result(
        "owner-bound Classic HD effects matrix artifact",
        validate_classic_hd_owner_effects_matrix_artifact,
        checks,
    )
    _validator_result(
        "Classic HD visual capture tooling",
        validate_classic_hd_visual_capture_tooling,
        checks,
    )
    _validator_result(
        "owner-bound Classic HD visual capture artifact",
        validate_classic_hd_owner_visual_capture_artifact,
        checks,
    )
    _validator_result(
        "Unity Enhanced visual capture tooling",
        validate_unity_enhanced_visual_capture_tooling,
        checks,
    )
    _validator_result(
        "Classic HD OS window lifecycle tooling",
        validate_classic_hd_os_window_lifecycle_tooling,
        checks,
    )
    _validator_result(
        "standalone presentation player build tooling",
        validate_standalone_player_build_tooling,
        checks,
    )
    _validator_result(
        "owner-bound Unity Enhanced visual capture artifact",
        validate_unity_enhanced_owner_visual_capture_artifact,
        checks,
    )
    _validator_result(
        "owner-bound Classic HD automatic high-speed probe",
        validate_classic_hd_owner_high_speed_artifact,
        checks,
    )
    _validator_result(
        "owner-bound Classic HD performance smoke",
        validate_classic_hd_owner_performance_artifact,
        checks,
    )
    _validator_result(
        "owner-bound Classic HD GPU readback/fence probe",
        validate_classic_hd_owner_gpu_fence_artifact,
        checks,
    )
    _validator_result(
        "owner-bound Classic HD 960x720 scenario artifact",
        lambda: validate_classic_hd_owner_scenario_artifact(
            CLASSIC_HD_OWNER_SCENARIO_960_ARTIFACT,
            "hd-960x720",
            960,
            720,
        ),
        checks,
    )
    _validator_result(
        "owner-bound Classic HD 960x720 sequence artifact",
        lambda: validate_sequence_artifact(PROFILE_960_SEQUENCE_ARTIFACT, "classic-hd-960x720", 960, 720),
        checks,
    )
    _validator_result(
        "owner-bound Classic HD 960x720 smoke artifact",
        lambda: validate_profile_smoke_artifact(
            Path(__file__).with_name("classic-hd-owner-route-960x720-smoke.json"),
            "classic-hd-960x720",
            960,
            720,
        ),
        checks,
    )
    _validator_result(
        "owner-bound Classic HD 960x720 sequence probe record",
        validate_profile_sequence_failure_artifact,
        checks,
    )

    matrix = json.loads((PRESENTATION_ROOT / "qualification" / "classic-hd-resolution-matrix.json").read_text(encoding="utf-8"))
    corpora_status = [
        json.loads((PRESENTATION_ROOT / "corpora" / directory / "manifest.json").read_text(encoding="utf-8"))
        for directory in ("exact", "classic-hd", "unity-enhanced")
    ]
    open_gates = []
    if any(manifest.get("capture_status") == "placeholder-not-captured" for manifest in corpora_status):
        open_gates.append("visual corpora are still placeholder-not-captured")
    if matrix.get("unity_gpu_and_owner_route") == "pending-owner-lab-capture":
        open_gates.append("owner/GPU matrix remains pending-owner-lab-capture")
    high_speed_probe = json.loads(CLASSIC_HD_OWNER_HIGH_SPEED_ARTIFACT.read_text(encoding="utf-8"))
    if high_speed_probe.get("status") == "open":
        open_gates.append("Classic HD automatic high-speed postprocess remains open; Speed-jar activation and GPU fencing are not qualified")
    performance_probe = json.loads(CLASSIC_HD_OWNER_PERFORMANCE_ARTIFACT.read_text(encoding="utf-8"))
    if performance_probe.get("status") == "open":
        over_budget = [
            profile.get("profile")
            for profile in performance_probe.get("profiles", [])
            if profile.get("budget_pass") is False
        ]
        if over_budget:
            open_gates.append(
                "Classic HD owner-bound performance remains over the 30 Hz presentation budget at: " +
                ", ".join(over_budget)
            )
    gpu_fence_probe = json.loads(CLASSIC_HD_OWNER_GPU_FENCE_ARTIFACT.read_text(encoding="utf-8"))
    if gpu_fence_probe.get("status") == "open":
        open_gates.append(
            "Classic HD GPU timing remains open; readbacks differ but the current OpenGLCore platform cannot report async graphics-fence status"
        )
    pending_route_cases = [
        name
        for name, value in matrix.get("route_cases", {}).items()
        if value == "pending-owner-lab-capture"
    ]
    if pending_route_cases:
        open_gates.append(
            "Classic HD remaining owner route cases remain pending: " + ", ".join(pending_route_cases)
        )
    completion_replay = json.loads(
        CLASSIC_HD_OWNER_COMPLETION_REPLAY_ARTIFACT.read_text(encoding="utf-8")
    )
    completion_route = json.loads(
        CLASSIC_HD_OWNER_COMPLETION_ARTIFACT.read_text(encoding="utf-8")
    )
    if completion_replay.get("status") == "open" and completion_route.get("status") != "pass":
        open_gates.append(
            "Classic HD owner command-journal replay is deterministic, but the authored completion/postgame route remains open"
        )

    status = "PASS" if all(check["passed"] for check in checks) else "FAIL"
    return {
        "status": status,
        "package": "private-three-mode-presentation",
        "qualification_state": "open-gates" if open_gates else "qualified",
        "open_gates": open_gates,
        "checks": checks,
        "failure_count": sum(not check["passed"] for check in checks),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    summary = verify()
    if args.json:
        print(json.dumps(summary, indent=2, sort_keys=True))
    else:
        print(
            f"{summary['status']} three-mode package integrity "
            f"({summary['qualification_state']})"
        )
        for check in summary["checks"]:
            print(("PASS" if check["passed"] else "FAIL") + " " + check["name"] + ": " + check["detail"])
        for gate in summary["open_gates"]:
            print("OPEN " + gate)
    return 0 if summary["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
