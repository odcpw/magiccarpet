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
        "owner-bound Unity Enhanced sequence artifact",
        validate_enhanced_owner_sequence_artifact,
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
    if any(value == "pending-owner-lab-capture" for value in matrix.get("route_cases", {}).values()):
        open_gates.append("town/combat/flight/restart/window scenario routes remain pending")

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
