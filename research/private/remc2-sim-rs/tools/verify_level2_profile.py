#!/usr/bin/env python3
"""Fail-closed Level 2 runtime-profile guard.

The active check owns only the small profile consumed by the runtime selection
seam and an independent focused ``sim-core`` behavior test.  The historical
Level 1 receipts and the generated Level 2 census/activation/denominator
matrix remain available for explicit archival review; they are deliberately
not read by this per-slice command.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parent.parent
PROFILE_PATH = ROOT / "level2-conformance" / "level2-profile.json"
PROFILE_SCHEMA = "mc2-level2-profile-contract/v1"

DIRECT_RUNTIME_CHECKS = (
    (
        "sim-core-profile-selection",
        [
            "cargo",
            "test",
            "-p",
            "sim-core",
            "--lib",
            "--locked",
            "profile_selection_is_typed_and_does_not_fallback",
        ],
    ),
)


class GuardError(Exception):
    """A runtime profile or direct behavior check violates the contract."""


def fail(message: str) -> None:
    raise GuardError(message)


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"invalid JSON {path}: {exc}")
    require(isinstance(value, dict), f"{path}: root must be an object")
    return value


def require_value(container: dict[str, Any], key: str, expected: Any, label: str) -> None:
    actual = container.get(key)
    same_type = isinstance(actual, type(expected))
    if isinstance(expected, bool):
        same_type = isinstance(actual, bool)
    elif isinstance(expected, int):
        same_type = isinstance(actual, int) and not isinstance(actual, bool)
    require(
        same_type and actual == expected,
        f"invalid runtime profile: {label} must be {expected!r}",
    )


def assert_profile(profile: dict[str, Any]) -> None:
    """Validate runtime-selected values without consulting archival metadata."""

    require(profile.get("schema") == PROFILE_SCHEMA, "wrong Level 2 profile schema")
    actual = profile.get("profile")
    require(isinstance(actual, dict), "profile.profile must be an object")

    # These values are the runtime selector contract.  They are checked as
    # individual fields rather than by copying a second nested dictionary that
    # could become a self-referential profile oracle.
    require_value(actual, "external_level", 2, "external level")
    require_value(actual, "internal_level_index", 1, "internal level index")

    map_type = actual.get("map_type")
    require(isinstance(map_type, dict), "invalid runtime profile: map_type must be an object")
    require_value(map_type, "name", "Day", "map type name")
    require_value(map_type, "serialized_value", 0, "Day map serialized value")

    require_value(actual, "campaign", "offline", "campaign")

    owner_data = actual.get("owner_data")
    require(isinstance(owner_data, dict), "invalid runtime profile: owner_data must be an object")
    require_value(owner_data, "language", "English", "owner-data language")
    require_value(owner_data, "required", True, "owner-data requirement")

    players = actual.get("players")
    require(isinstance(players, dict), "invalid runtime profile: players must be an object")
    require_value(players, "local_player_indices", [0], "local player indices")
    require_value(players, "ai_player_indices", [1], "AI player indices")
    require_value(players, "total_players", 2, "total player count")
    require_value(players, "network_multiplayer", False, "network multiplayer")

    desktop = actual.get("desktop")
    require(isinstance(desktop, dict), "invalid runtime profile: desktop must be an object")
    require_value(desktop, "width", 320, "desktop width")
    require_value(desktop, "height", 200, "desktop height")
    require_value(desktop, "screen_size", 0, "desktop screen size")
    require_value(desktop, "stereo", False, "desktop stereo")

    # The profile format has no fallback mode.  Reject an explicit opt-in if a
    # caller tries to widen it, while allowing unrelated archival fields to be
    # edited without changing runtime qualification.
    for field in ("fallback", "fallback_enabled", "allow_fallback"):
        if field in actual:
            require_value(actual, field, False, f"{field} must remain disabled")


def _command_text(command: Sequence[str]) -> str:
    return " ".join(command)


def _test_count(output: str) -> int:
    return sum(int(value) for value in re.findall(r"running\s+(\d+)\s+tests?", output))


def run_runtime_checks() -> list[dict[str, Any]]:
    """Run direct runtime checks and reject successful zero-test selection."""

    results: list[dict[str, Any]] = []
    for check_id, command in DIRECT_RUNTIME_CHECKS:
        try:
            process = subprocess.run(
                command,
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
                timeout=300,
            )
        except subprocess.TimeoutExpired as exc:
            detail = (exc.stdout or "") + (exc.stderr or "")
            fail(f"direct runtime check {check_id} timed out after 300 seconds: {detail[-1200:]}")
        except OSError as exc:
            fail(f"direct runtime check {check_id} could not start: {exc}")

        output = process.stdout + process.stderr
        test_count = _test_count(output)
        passed = process.returncode == 0 and test_count > 0
        result = {
            "id": check_id,
            "status": "PASS" if passed else "FAIL",
            "command": _command_text(command),
            "tests": test_count,
        }
        results.append(result)
        if not passed:
            # Keep compiler/test diagnostics on stderr so a JSON caller still
            # receives machine-readable stdout and no evidence is discarded.
            detail = output.strip() or "<no output>"
            print(detail, file=sys.stderr)
            if process.returncode == 0 and test_count == 0:
                fail(f"direct runtime check {check_id} selected zero tests")
            fail(f"direct runtime check {check_id} failed: {detail[-1200:]}")
    return results


def verify(*, profile_path: Path | None = None, run_tests: bool = True) -> dict[str, Any]:
    path = (profile_path or PROFILE_PATH).resolve()
    profile = load_json(path)
    assert_profile(profile)
    checks = run_runtime_checks() if run_tests else []
    return {
        "ok": True,
        "profile": "Level 2 / external 2 / internal 1 / Day",
        "runtime_checks": {
            "status": "PASS" if run_tests else "NOT_RUN",
            "checks": checks,
        },
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", type=Path, default=PROFILE_PATH)
    parser.add_argument(
        "--no-runtime-tests",
        "--no-current-tests",
        dest="no_runtime_tests",
        action="store_true",
        help="validate only the runtime profile schema/value contract",
    )
    parser.add_argument("--json", action="store_true", help="emit one machine-readable result")
    args = parser.parse_args(argv)
    try:
        result = verify(profile_path=args.profile, run_tests=not args.no_runtime_tests)
    except GuardError as exc:
        result = {"ok": False, "error": str(exc)}
        if args.json:
            print(json.dumps(result, sort_keys=True))
        else:
            print(f"LEVEL2 PROFILE FAIL: {exc}", file=sys.stderr)
        return 1
    if args.json:
        print(json.dumps(result, sort_keys=True))
    else:
        print("LEVEL2 PROFILE PASS: external level 2 -> internal index 1, Day, offline, 320x200")
        if not args.no_runtime_tests:
            print("  direct sim-core profile behavior: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
