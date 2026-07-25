#!/usr/bin/env python3
"""CLI entry point for the deterministic private P1-P5 smoke suite."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from protocol_smoke import ScenarioError, run_all, verify_expected, verify_inputs


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser()
    result.add_argument(
        "--output-dir",
        required=True,
        type=Path,
        help="New owner-only directory under /tmp or this private package",
    )
    result.add_argument(
        "--skip-lock-check",
        action="store_true",
        help="Maintainer bootstrap only: emit results before expected.json exists",
    )
    return result


def main() -> int:
    args = parser().parse_args()
    try:
        if not args.skip_lock_check:
            verify_inputs()
        summary = run_all(args.output_dir)
        if not args.skip_lock_check:
            verify_expected(summary)
    except (OSError, ScenarioError, ValueError) as error:
        print(f"protocol smoke error: {error}", file=sys.stderr)
        return 2
    print(json.dumps(summary, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
