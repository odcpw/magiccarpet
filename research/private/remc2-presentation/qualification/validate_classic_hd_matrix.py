#!/usr/bin/env python3
"""Fail-closed validation for the private Classic HD resolution matrix."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


DEFAULT_MATRIX = Path(__file__).with_name("classic-hd-resolution-matrix.json")
BUDGET_NS = 33_333_333
REQUIRED = ("classic-hd-640x480", "classic-hd-960x720", "classic-hd-1280x960")


def validate(path: Path = DEFAULT_MATRIX) -> list[str]:
    errors: list[str] = []
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"matrix read failed: {error}"]

    if data.get("schema_version") != 1:
        errors.append("schema_version must be 1")
    if data.get("owner_payload_embedded") is not False:
        errors.append("owner_payload_embedded must be false")
    profiles = data.get("profiles")
    if not isinstance(profiles, list) or [item.get("profile") for item in profiles] != list(REQUIRED):
        errors.append("profiles must contain the mandatory rows in order")
        return errors

    for profile in profiles:
        if not isinstance(profile.get("p95_cpu_ns"), int) or profile["p95_cpu_ns"] <= 0:
            errors.append(f"{profile['profile']} has no positive p95_cpu_ns")
        if profile.get("deterministic_reset") is not True:
            errors.append(f"{profile['profile']} is not deterministic")
        if profile["profile"] == REQUIRED[0] and profile.get("status") != "supported-cpu-and-deterministic":
            errors.append("640x480 must be CPU-supported and deterministic")
        if profile["profile"] != REQUIRED[0] and profile["p95_cpu_ns"] <= BUDGET_NS and profile.get("status") not in {
            "candidate-awaiting-unity-gpu-and-owner-route-capture",
            "supported-cpu-and-deterministic",
            "rejected-by-owner-real-time-budget",
        }:
            errors.append(f"{profile['profile']} status contradicts its measured p95")
        if profile["profile"] != REQUIRED[0] and profile["p95_cpu_ns"] > BUDGET_NS and not str(profile.get("status", "")).startswith("rejected"):
            errors.append(f"{profile['profile']} over-budget row must remain rejected")

    if data.get("window_resolution_is_separate_from_internal_render_resolution") is not True:
        errors.append("window/internal resolution separation must be explicit")
    owner_gpu_status = data.get("unity_gpu_and_owner_route")
    if owner_gpu_status == "pending-owner-lab-capture":
        pass
    elif owner_gpu_status == "owner-route-qualified; gpu-timing-pending":
        if data.get("owner_route_qualification_status") != "qualified-owner-lab-capture":
            errors.append("qualified owner route must identify its qualification status")
        if data.get("owner_route_profiles") != ["classic-hd-640x480", "classic-hd-960x720"]:
            errors.append("qualified owner route must name the two captured profiles")
        if data.get("owner_route_owner_payload_embedded") is not False:
            errors.append("qualified owner route must keep owner payload external")
        if data.get("owner_route_owner_inputs_external") is not True:
            errors.append("qualified owner route must identify owner inputs as external")
        if data.get("owner_route_gpu_timing") != "not-captured":
            errors.append("qualified owner route must keep GPU timing explicitly not-captured")
    else:
        errors.append("owner/GPU route status must be pending or explicitly split")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("matrix", nargs="?", type=Path, default=DEFAULT_MATRIX)
    args = parser.parse_args()
    errors = validate(args.matrix)
    if errors:
        print("FAIL Classic HD matrix")
        for error in errors:
            print("- " + error)
        return 1
    print(f"PASS Classic HD matrix: {args.matrix}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
