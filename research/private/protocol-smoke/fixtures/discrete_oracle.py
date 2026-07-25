#!/usr/bin/env python3
"""Known-source discrete oracle used only by the private protocol smoke suite."""

from __future__ import annotations

import argparse
import json


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--current", type=int, required=True)
    parser.add_argument("--increment", type=int, required=True)
    args = parser.parse_args()

    if not 0 <= args.current <= 99:
        print(json.dumps({"accepted": False, "error": "current_out_of_range"}, sort_keys=True))
        return 2
    if not 1 <= args.increment <= 100:
        print(
            json.dumps(
                {"accepted": False, "error": "increment_out_of_range"},
                sort_keys=True,
            )
        )
        return 2

    total = args.current + args.increment
    print(
        json.dumps(
            {
                "accepted": True,
                "current": total % 100,
                "wrapped": total >= 100,
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
