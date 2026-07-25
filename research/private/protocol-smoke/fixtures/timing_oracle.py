#!/usr/bin/env python3
"""Known-source fixed-20-ms accumulator oracle."""

from __future__ import annotations

import argparse
import json


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--deltas-ms", required=True)
    args = parser.parse_args()

    try:
        deltas = [int(item) for item in args.deltas_ms.split(",")]
    except ValueError:
        print(json.dumps({"accepted": False, "error": "invalid_delta"}, sort_keys=True))
        return 2
    if not deltas or any(delta < 0 for delta in deltas):
        print(json.dumps({"accepted": False, "error": "invalid_delta"}, sort_keys=True))
        return 2

    elapsed = sum(deltas)
    print(
        json.dumps(
            {
                "accepted": True,
                "callback_count": len(deltas),
                "remainder_ms": elapsed % 20,
                "ticks": elapsed // 20,
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
