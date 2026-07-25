#!/usr/bin/env python3
"""Known-source marker/function fixture with observable runtime tags."""

from __future__ import annotations

import argparse
import json
from collections.abc import Callable
from typing import Any


def synthetic_marker(name: str) -> Callable[[Callable[[], dict[str, Any]]], Callable[[], dict[str, Any]]]:
    def decorate(function: Callable[[], dict[str, Any]]) -> Callable[[], dict[str, Any]]:
        setattr(function, "synthetic_marker", name)
        return function

    return decorate


@synthetic_marker("ALPHA")
def alpha_handler() -> dict[str, Any]:
    return {"runtime_tag": "LIFT", "value": 3}


@synthetic_marker("OMEGA")
def omega_handler() -> dict[str, Any]:
    return {"runtime_tag": "DROP", "value": -2}


HANDLERS = {"ALPHA": alpha_handler, "OMEGA": omega_handler}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--marker", required=True)
    args = parser.parse_args()
    handler = HANDLERS.get(args.marker)
    if handler is None:
        print(json.dumps({"accepted": False, "error": "unknown_marker"}, sort_keys=True))
        return 2
    print(
        json.dumps(
            {"accepted": True, "marker": args.marker, **handler()},
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
