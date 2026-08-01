#!/usr/bin/env python3
"""Validate the pinned, source-grounded Classic HD rendering specification."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


DEFAULT_SPEC = Path(__file__).with_name("classic-hd-spec.json")
PINNED_REMC2 = "6806c7204d17220ecc0430759f784aea8ee2ddb2"
REQUIRED_STAGES = {"configuration", "buffer_and_history", "camera_and_world", "sky", "terrain_and_painter", "sprites_and_billboards", "ui_and_mouse", "display_and_selection", "asset_fallback"}
REQUIRED_PIPELINE = {"input", "buffer", "sky", "terrain", "typed-sprites", "postprocess", "hud-and-present"}


def load_spec(path: Path = DEFAULT_SPEC) -> dict[str, Any]:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def validate_spec(spec: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if spec.get("schema_version") != 1:
        errors.append("schema_version must be 1")
    source = spec.get("source", {})
    if source.get("commit") != PINNED_REMC2:
        errors.append("spec must be pinned to remc2 commit 6806c7204d17220ecc0430759f784aea8ee2ddb2")
    if source.get("read_policy") != "git object at pinned commit; never infer from a dirty upstream worktree":
        errors.append("source read policy must reject dirty upstream inference")
    policy = spec.get("milestone_1_policy", {})
    if policy.get("internal_aspect") != "4:3":
        errors.append("milestone 1 internal aspect must be 4:3")
    profiles = policy.get("supported_profiles", [])
    if not profiles:
        errors.append("at least one Classic HD profile is required")
    for profile in profiles:
        width, height = profile.get("game_resolution", [0, 0])
        if width * 3 != height * 4:
            errors.append(f"profile {profile.get('id')} is not 4:3")
        if profile.get("stride") != width:
            errors.append(f"profile {profile.get('id')} stride must equal width")
    stages = {anchor.get("stage") for anchor in spec.get("source_anchors", [])}
    errors.extend(f"missing source anchor stage: {stage}" for stage in sorted(REQUIRED_STAGES - stages))
    pipeline = {stage.get("id") for stage in spec.get("pipeline_contract", [])}
    errors.extend(f"missing pipeline stage: {stage}" for stage in sorted(REQUIRED_PIPELINE - pipeline))
    if spec.get("provenance", {}).get("original_binary_oracle") is not False:
        errors.append("original-binary oracle comparison must remain false")
    if spec.get("provenance", {}).get("dosbox") is not False:
        errors.append("DOSBox comparison must remain false")
    if not spec.get("unresolved_or_fail_closed"):
        errors.append("unresolved/fail-closed behavior must be explicit")
    for stage in spec.get("pipeline_contract", []):
        if not stage.get("test"):
            errors.append(f"pipeline stage {stage.get('id')} has no discriminating test requirement")
    return errors


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--spec", type=Path, default=DEFAULT_SPEC)
    parser.add_argument("--json", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        errors = validate_spec(load_spec(args.spec))
    except (OSError, json.JSONDecodeError, KeyError) as error:
        errors = [str(error)]
    result = {"pass": not errors, "spec": str(args.spec), "errors": errors}
    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    elif errors:
        print("CLASSIC HD SPEC FAIL")
        for error in errors:
            print(f"  - {error}")
    else:
        print(f"CLASSIC HD SPEC PASS: pinned remc2 {PINNED_REMC2}")
    return 0 if not errors else 1


if __name__ == "__main__":
    sys.exit(main())
