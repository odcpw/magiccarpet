#!/usr/bin/env python3
"""Known-source transfer classification fixture."""

from __future__ import annotations

import argparse
import json


VARIANTS = {
    "clean": {
        "contains_original_payload": False,
        "traffic_light": "GREEN",
    },
    "contaminated": {
        "contains_original_payload": True,
        "traffic_light": "RED",
    },
}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--variant", choices=sorted(VARIANTS), required=True)
    args = parser.parse_args()
    print(
        json.dumps(
            {"accepted": True, "variant": args.variant, **VARIANTS[args.variant]},
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
