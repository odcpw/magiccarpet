#!/usr/bin/env python3
"""Validate and print the private three-mode Magic Carpet launcher command.

This launcher never embeds owner data and never silently substitutes one
renderer for another.  Exact points at the sealed Built-in project; the two
optional modes point at the isolated URP lab and require explicit external
owner inputs.
"""

from __future__ import annotations

import argparse
import json
import shlex
import sys
from pathlib import Path
from typing import Any


MODES = ("exact", "classic-hd", "unity-enhanced")
SCHEMA_VERSION = 1


class LauncherConfigError(ValueError):
    pass


def load_config(path: Path) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise LauncherConfigError(f"launcher config could not be read: {exc}") from exc
    validate_config(data)
    return data


def validate_config(data: dict[str, Any]) -> None:
    if not isinstance(data, dict) or data.get("schema_version") != SCHEMA_VERSION:
        raise LauncherConfigError("unsupported launcher config schema version")
    if data.get("default_mode") not in MODES:
        raise LauncherConfigError("default_mode must be exact, classic-hd, or unity-enhanced")
    if not isinstance(data.get("unity_executable"), str) or not data["unity_executable"]:
        raise LauncherConfigError("unity_executable must be a non-empty command")
    modes = data.get("modes")
    if not isinstance(modes, dict):
        raise LauncherConfigError("modes must be an object")
    for mode in MODES:
        entry = modes.get(mode)
        if not isinstance(entry, dict):
            raise LauncherConfigError(f"missing launcher entry for {mode}")
        if not isinstance(entry.get("project"), str) or not entry["project"]:
            raise LauncherConfigError(f"{mode} project must be a non-empty path")
        if not isinstance(entry.get("arguments"), list) or not all(
            isinstance(argument, str) for argument in entry["arguments"]
        ):
            raise LauncherConfigError(f"{mode} arguments must be a string list")

    exact_args = data["modes"]["exact"]["arguments"]
    if any(argument.startswith("-mc-presentation-mode") for argument in exact_args):
        raise LauncherConfigError("Exact launcher entry must not select an optional presentation mode")

    classic = data.get("classic_hd_defaults")
    if not isinstance(classic, dict) or classic.get("profile") != "hd-640x480":
        raise LauncherConfigError("Classic HD defaults must select mandatory hd-640x480")
    if classic.get("output_filter") not in ("point", "bilinear"):
        raise LauncherConfigError("Classic HD output_filter is invalid")


def config_root(path: Path) -> Path:
    # launcher/ is one directory below the isolated Unity project.
    return path.resolve().parent.parent


def build_command(
    config: dict[str, Any],
    config_path: Path,
    mode: str | None = None,
    owner_package: str | None = None,
    owner_closure: str | None = None,
    owner_cache: str | None = None,
    settings_path: str | None = None,
    unity_executable: str | None = None,
) -> list[str]:
    validate_config(config)
    selected = mode or config["default_mode"]
    if selected not in MODES:
        raise LauncherConfigError(f"unsupported presentation mode: {selected}")

    root = config_root(config_path)
    entry = config["modes"][selected]
    project = (root / entry["project"]).resolve()
    command = [unity_executable or config["unity_executable"], "-projectPath", str(project)]
    command.extend(entry["arguments"])

    if selected != "exact":
        supplied = (owner_package, owner_closure, owner_cache)
        if any(value is None for value in supplied):
            raise LauncherConfigError("optional modes require package, closure, and asset-cache paths")
        for label, value in zip(("package", "closure", "asset-cache"), supplied):
            assert value is not None
            path = Path(value)
            if not path.is_absolute():
                raise LauncherConfigError(f"owner {label} path must be absolute")

        command.extend(["-mc-package", owner_package])
        command.extend(["-mc-closure", owner_closure])
        command.extend(["-mc-asset-cache", owner_cache])
        if settings_path is not None:
            settings = Path(settings_path)
            if not settings.is_absolute():
                raise LauncherConfigError("Classic HD settings path must be absolute")
            command.extend(["-mc-settings", str(settings)])

    return command


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, default=Path(__file__).with_name("presentation-config.json"))
    parser.add_argument("--mode", choices=MODES)
    parser.add_argument("--unity", help="override the Unity executable command")
    parser.add_argument("--owner-package")
    parser.add_argument("--owner-closure")
    parser.add_argument("--owner-cache")
    parser.add_argument("--settings")
    parser.add_argument("--print-json", action="store_true")
    parser.add_argument("--validate", action="store_true")
    args = parser.parse_args(argv)

    try:
        config = load_config(args.config)
        command = build_command(
            config,
            args.config,
            args.mode,
            args.owner_package,
            args.owner_closure,
            args.owner_cache,
            args.settings,
            args.unity,
        )
    except LauncherConfigError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2

    if args.validate:
        print("PASS presentation launcher config")
    elif args.print_json:
        print(json.dumps({"mode": args.mode or config["default_mode"], "command": command}, indent=2))
    else:
        print(shlex.join(command))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
