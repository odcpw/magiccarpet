#!/usr/bin/env python3
"""Verify two independent canonical frame traces are byte-identical."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

TRACE_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TRACE_ROOT))
from trace_format import qualify_trace  # noqa: E402


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("first", type=Path)
    parser.add_argument("second", type=Path)
    parser.add_argument("--require-complete-categories", action="store_true")
    args = parser.parse_args()
    first = args.first.resolve(strict=True)
    second = args.second.resolve(strict=True)
    first_result = qualify_trace(
        first / "trace.jsonl",
        require_complete_categories=args.require_complete_categories,
    )
    second_result = qualify_trace(
        second / "trace.jsonl",
        require_complete_categories=args.require_complete_categories,
    )
    if (first / "trace.jsonl").read_bytes() != (second / "trace.jsonl").read_bytes():
        raise SystemExit("canonical traces differ")
    if first_result != second_result:
        raise SystemExit("qualification summaries differ")
    first_run = json.loads((first / "run.json").read_text(encoding="utf-8"))
    second_run = json.loads((second / "run.json").read_text(encoding="utf-8"))
    if first_run["run_uuid"] == second_run["run_uuid"]:
        raise SystemExit("run UUIDs are not independent")
    print(
        "PASS: canonical traces are byte-identical; "
        f"event_set_sha256={first_result['event_set_sha256']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
