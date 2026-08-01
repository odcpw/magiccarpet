#!/usr/bin/env python3
"""Validate the private three-mode presentation contract and CLI combinations."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


DEFAULT_CONTRACT = Path(__file__).with_name("mode-contract.json")
MODE_IDS = ("exact", "classic-hd", "unity-enhanced")


def load_contract(path: Path = DEFAULT_CONTRACT) -> dict[str, Any]:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def validate_contract(contract: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if contract.get("schema_version") != 1:
        errors.append("schema_version must be 1")
    modes = contract.get("modes")
    if not isinstance(modes, dict) or tuple(modes) != MODE_IDS:
        errors.append(f"modes must contain exactly {MODE_IDS} in order")
        return errors
    if contract.get("launcher", {}).get("default") != "exact":
        errors.append("launcher default must be exact")
    if contract.get("launcher", {}).get("selection_phase") != "before_session_open":
        errors.append("mode selection must happen before session open")
    if contract.get("authority", {}).get("pipeline_switching") != "forbidden during a presentation session":
        errors.append("mid-session pipeline switching must remain forbidden")
    if modes["exact"].get("renderer") != "built-in-exact-indexed":
        errors.append("Exact must use the sealed Built-in indexed renderer")
    if modes["exact"].get("internal_viewport") != {"width": 320, "height": 200, "format": "R8 indexed FINAL MAIN"}:
        errors.append("Exact internal viewport contract drifted")
    classic = modes["classic-hd"]
    if "indexed" not in str(classic.get("renderer")) or "upscale" in str(classic.get("renderer")):
        errors.append("Classic HD must be an indexed rerender, not an upscale renderer")
    for profile in classic.get("internal_profiles", []):
        if profile.get("width", 0) * 3 != profile.get("height", 0) * 4:
            errors.append(f"Classic HD profile {profile.get('id')} is not 4:3")
        if profile.get("stride") != profile.get("width"):
            errors.append(f"Classic HD profile {profile.get('id')} stride must equal width")
    for mode_id in MODE_IDS:
        if modes[mode_id].get("project") == modes["exact"].get("project") and mode_id != "exact":
            errors.append(f"{mode_id} cannot share the Exact Unity project")
        if modes[mode_id].get("corpus_root") == modes["exact"].get("corpus_root") and mode_id != "exact":
            errors.append(f"{mode_id} cannot write the Exact corpus")
    invariants = contract.get("invariants", {})
    for field in ("command_stream", "gameplay_event_stream", "gameplay_state_hash", "conformance_hash", "reset_and_restart"):
        if not invariants.get(field):
            errors.append(f"missing cross-mode invariant: {field}")
    if invariants.get("persistent_save_support") is not False:
        errors.append("persistent save support must be honest and false until implemented")
    if "VoxelMode" in json.dumps(contract):
        errors.append("VoxelMode must not be an Enhanced authority")
    return errors


def validate_invocation(
    contract: dict[str, Any],
    *,
    mode: str | None = None,
    internal_width: int | None = None,
    internal_height: int | None = None,
    pipeline_switch: bool = False,
    exact_claim: bool = False,
    widescreen_world: bool = False,
    owner_inputs: Path | None = None,
    corpus_root: str | None = None,
    mode_off: bool = False,
) -> list[str]:
    errors = validate_contract(contract)
    modes = contract["modes"]
    if mode_off or mode == "off":
        errors.append("presentation mode cannot be off; choose exact, classic-hd, or unity-enhanced")
    if mode is not None and mode not in MODE_IDS:
        errors.append(f"unknown presentation mode: {mode}")
        return errors
    if mode is None:
        return errors
    selected = modes[mode]
    if pipeline_switch:
        errors.append("pipeline family may not switch during a presentation session")
    if exact_claim and not selected["exact_claim"]:
        errors.append(f"{mode} cannot make an Exact claim")
    if widescreen_world and mode == "exact":
        errors.append("Exact may pillarbox a widescreen window but may not expand the world viewport")
    if internal_width is not None or internal_height is not None:
        width = internal_width or selected.get("internal_viewport", {}).get("width")
        height = internal_height or selected.get("internal_viewport", {}).get("height")
        if mode == "exact" and (width, height) != (320, 200):
            errors.append("Exact internal viewport is fixed at 320x200")
        if mode == "classic-hd" and (width is None or height is None or width * 3 != height * 4):
            errors.append("Classic HD internal viewport must be a 4:3 rerender profile")
    if owner_inputs is not None and mode != "exact" and not owner_inputs.exists():
        errors.append(f"required owner inputs are absent for {mode}: {owner_inputs}")
    if corpus_root is not None and corpus_root != selected.get("corpus_root"):
        errors.append(f"corpus root does not belong to {mode}: {corpus_root}")
    return errors


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--contract", type=Path, default=DEFAULT_CONTRACT)
    parser.add_argument("--mode")
    parser.add_argument("--internal-width", type=int)
    parser.add_argument("--internal-height", type=int)
    parser.add_argument("--pipeline-switch", action="store_true")
    parser.add_argument("--exact-claim", action="store_true")
    parser.add_argument("--widescreen-world", action="store_true")
    parser.add_argument("--owner-inputs", type=Path)
    parser.add_argument("--corpus-root")
    parser.add_argument("--mode-off", action="store_true")
    parser.add_argument("--json", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        contract = load_contract(args.contract)
        errors = validate_invocation(
            contract,
            mode=args.mode,
            internal_width=args.internal_width,
            internal_height=args.internal_height,
            pipeline_switch=args.pipeline_switch,
            exact_claim=args.exact_claim,
            widescreen_world=args.widescreen_world,
            owner_inputs=args.owner_inputs,
            corpus_root=args.corpus_root,
            mode_off=args.mode_off,
        )
    except (OSError, json.JSONDecodeError, KeyError) as error:
        errors = [str(error)]
    result = {"pass": not errors, "contract": str(args.contract), "mode": args.mode, "errors": errors}
    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    elif errors:
        print("PRESENTATION MODE CONTRACT FAIL")
        for error in errors:
            print(f"  - {error}")
    else:
        print(f"PRESENTATION MODE CONTRACT PASS: {args.mode or 'schema'}")
    return 0 if not errors else 1


if __name__ == "__main__":
    sys.exit(main())
