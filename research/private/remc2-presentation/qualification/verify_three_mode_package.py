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


def _load_module(name: str, path: Path) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise ImportError(f"cannot load validator module: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _check(checks: list[dict[str, Any]], name: str, passed: bool, detail: str) -> None:
    checks.append({"name": name, "passed": passed, "detail": detail})


def validate_sequence_artifact(path: Path = SEQUENCE_ARTIFACT) -> list[str]:
    errors: list[str] = []
    try:
        artifact = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"sequence artifact read failed: {error}"]

    if artifact.get("schema_version") != 1:
        errors.append("sequence artifact schema_version must be 1")
    if artifact.get("kind") != "classic-hd-owner-route-sequence-smoke/v1":
        errors.append("sequence artifact kind is wrong")
    if artifact.get("profile") != "classic-hd-640x480":
        errors.append("sequence artifact must be the qualified 640x480 profile")
    if artifact.get("internal_width") != 640 or artifact.get("internal_height") != 480:
        errors.append("sequence artifact extent must be 640x480")
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
