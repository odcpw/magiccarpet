#!/usr/bin/env python3
"""Static VR-B guard for package/profile/action-map exclusivity."""

from __future__ import annotations

import json
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[4]
PROJECT = ROOT / "research" / "private" / "remc2-unity-presentation-lab"
PINS = {
    "com.unity.inputsystem": "1.14.2",
    "com.unity.xr.management": "4.7.0",
    "com.unity.xr.openxr": "1.17.1",
    "com.unity.xr.interaction.toolkit": "3.0.9",
}


def fail(message: str) -> None:
    raise ValueError(message)


def validate() -> dict[str, object]:
    manifest_path = PROJECT / "Packages" / "manifest.json"
    lock_path = PROJECT / "Packages" / "packages-lock.json"
    profile_path = PROJECT / "qualification" / "xr-smoke-profile.json"
    action_path = PROJECT / "Assets" / "InputSystem" / "XrSmoke.inputactions"
    script_path = PROJECT / "Assets" / "Scripts" / "XrSmokeBootstrap.cs"
    for path in (manifest_path, lock_path, profile_path, action_path, script_path):
        if not path.is_file():
            fail(f"missing VR-B file: {path.relative_to(ROOT)}")

    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    lock = json.loads(lock_path.read_text(encoding="utf-8"))
    profile = json.loads(profile_path.read_text(encoding="utf-8"))
    actions = json.loads(action_path.read_text(encoding="utf-8"))
    for package, version in PINS.items():
        if manifest.get("dependencies", {}).get(package) != version:
            fail(f"manifest pin mismatch: {package}")
        entry = lock.get("dependencies", {}).get(package, {})
        if entry.get("version") != version or entry.get("depth") != 0:
            fail(f"lock pin mismatch or non-direct package: {package}")

    profiles = profile.get("profiles", {})
    if set(profiles) != {"flat-disabled", "pc-openxr"}:
        fail("VR-B must contain exactly flat-disabled and pc-openxr profiles")
    flat = profiles["flat-disabled"]
    pc = profiles["pc-openxr"]
    if flat["xr_loader"] != "none" or flat["xr_origin"] is not False:
        fail("flat-disabled profile can initialize XR")
    if pc["xr_loader"] != "com.unity.xr.openxr" or pc["xr_origin"] is not True:
        fail("pc-openxr profile is incomplete")
    for profile_name in profiles:
        if profiles[profile_name]["owner_data"] != "none" or profiles[profile_name]["simulation"] != "none":
            fail(f"profile is not owner-free: {profile_name}")

    action_map = actions.get("maps", [])
    if len(action_map) != 1 or action_map[0].get("name") != "XR Smoke":
        fail("XR Smoke action map is missing")
    action_names = {action["name"] for action in action_map[0].get("actions", [])}
    required_actions = set(profile["required_actions"])
    if action_names != required_actions:
        fail("action map names do not match required controls")
    binding_paths = {binding["path"] for binding in action_map[0].get("bindings", [])}
    for token in ("{LeftHand}", "{RightHand}", "thumbstick", "deviceRotation", "trigger", "grip"):
        if not any(token in path for path in binding_paths):
            fail(f"action map lacks binding token: {token}")

    script = script_path.read_text(encoding="utf-8")
    for marker in ("-mc-xr-profile", "XRGeneralSettings", "XR Origin", "XR UNAVAILABLE", "owner-data=none", "simulation=none"):
        if marker not in script:
            fail(f"XR bootstrap lacks marker: {marker}")
    for forbidden in ("NativeSession", "OwnerInputValidator", "PresentationSessionLease"):
        if forbidden in script:
            fail(f"XR smoke bootstrap has forbidden gameplay/owner coupling: {forbidden}")

    return {
        "status": "pass",
        "profiles": sorted(profiles),
        "actions": sorted(action_names),
        "packages": PINS,
        "runtime_verified": False,
    }


def main() -> int:
    try:
        result = validate()
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
