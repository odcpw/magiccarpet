#!/usr/bin/env python3
"""Validate the private VR architecture record and Exact isolation guard."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[3]
GATE = Path(__file__).resolve().parent / "architecture-gate.json"
HEX40 = re.compile(r"^[0-9a-f]{40}$")
REQUIRED_PACKAGES = {
    "com.unity.render-pipelines.universal": "17.3.0",
    "com.unity.inputsystem": "1.14.2",
    "com.unity.xr.management": "4.7.0",
    "com.unity.xr.openxr": "1.17.1",
    "com.unity.xr.interaction.toolkit": "3.0.9",
    "com.unity.test-framework": "1.6.0",
}


def fail(message: str) -> None:
    raise ValueError(message)


def exact_negative_scan(root: Path, forbidden: list[str]) -> dict[str, object]:
    exact = root / "research" / "private" / "remc2-unity-native"
    manifest_path = exact / "Packages" / "manifest.json"
    if not manifest_path.is_file():
        fail(f"Exact manifest missing: {manifest_path}")
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    package_hits = [name for name in manifest.get("dependencies", {}) if name in forbidden[:3]]
    if package_hits:
        fail("Exact project contains forbidden XR packages: " + ", ".join(package_hits))

    text_hits: list[str] = []
    scan_roots = (exact / "Assets", exact / "Packages", exact / "ProjectSettings")
    for scan_root in scan_roots:
        if not scan_root.exists():
            continue
        for path in scan_root.rglob("*"):
            if not path.is_file() or path.suffix.lower() in {".png", ".jpg", ".wav", ".mp3", ".dll"}:
                continue
            text = path.read_text(encoding="utf-8", errors="ignore")
            for token in forbidden[3:]:
                if token in text:
                    text_hits.append(f"{path.relative_to(root)}:{token}")
    if text_hits:
        fail("Exact project contains forbidden XR object/token: " + ", ".join(text_hits[:8]))
    return {"package_hits": [], "token_hits": [], "status": "pass"}


def validate(root: Path, gate_path: Path = GATE) -> dict[str, object]:
    gate = json.loads(gate_path.read_text(encoding="utf-8"))
    if gate.get("schema_version") != 1 or gate.get("bead") != "mc-dl0r.19.1":
        fail("unexpected VR gate schema or bead")

    handoff = gate["gfx_handoff"]
    dependencies = gate.get("dependencies", {})
    for dependency in ("gfx_a_exact_guard", "gfx_b_mode_contract", "gfx_d_presentation_lab", "rust_presentation_abi"):
        if dependency not in dependencies:
            fail(f"missing explicit dependency: {dependency}")
    for key in ("exact_baseline_commit", "mode_contract_commit"):
        if not HEX40.fullmatch(handoff[key]):
            fail(f"{key} is not a full commit SHA")
    lab = handoff["presentation_lab"]
    if lab["project_path"] != "research/private/remc2-unity-presentation-lab":
        fail("presentation-lab handoff path drifted")
    if not HEX40.fullmatch(lab["handoff_commit"]):
        fail("presentation-lab handoff is not a full commit SHA")
    if lab["owner_payload_in_tree"] is not False:
        fail("presentation-lab handoff permits owner payload")

    unity = gate["unity_and_packages"]
    if unity["editor"] != "6000.3.20f1":
        fail("Unity editor pin drifted")
    for package, version in REQUIRED_PACKAGES.items():
        if unity["packages"].get(package) != version:
            fail(f"VR package pin drifted: {package}")

    targets = gate["target_matrix"]
    required_targets = {"pc_openxr_steamvr", "quest3_standalone", "quest2_standalone"}
    if set(targets) != required_targets:
        fail("target matrix is incomplete")
    pc = targets["pc_openxr_steamvr"]
    for key in ("host", "gpu", "runtime", "active_runtime", "runtime_id", "device_id", "admission_rule"):
        if not pc.get(key):
            fail(f"PC target missing {key}")

    controls = gate["transforms_and_controls"]
    if controls["coordinate_contract"]["tracked_head_pose"] != "local view offset only":
        fail("head pose is not presentation-local")
    if controls["right_hand_spell"]["hand_origin_throw"] != "not part of the first slice":
        fail("hand-origin throw was promoted into the first slice")

    budgets = gate["budgets"]
    if budgets["frame_time_ms"] != {"72": 13.89, "90": 11.11}:
        fail("refresh/frame budget mismatch")
    if budgets["allocations_after_warmup"] != 0:
        fail("steady-state allocation gate is not zero")
    if budgets["rust_tick_p99_ms"] > 4.0:
        fail("Rust tick budget is too loose")

    forbidden = gate["exact_negative_scan"]["forbidden_tokens"]
    scan = exact_negative_scan(root, forbidden)
    return {
        "status": "pass",
        "bead": gate["bead"],
        "handoff_commit": lab["handoff_commit"],
        "pc_runtime_status": pc["status"],
        "exact_negative_scan": scan,
        "runtime_qualification": "pending-editor-headset-build",
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true", dest="as_json")
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args(argv)
    try:
        result = validate(args.root.resolve())
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        if args.as_json:
            print(json.dumps({"status": "fail", "error": str(exc)}, sort_keys=True))
        else:
            print(f"FAIL: {exc}", file=sys.stderr)
        return 1
    if args.as_json:
        print(json.dumps(result, indent=2, sort_keys=True))
    else:
        print("PASS: private VR architecture gate")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
