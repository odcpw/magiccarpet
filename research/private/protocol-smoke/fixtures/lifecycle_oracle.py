#!/usr/bin/env python3
"""Known-source revision fixture for contradiction/supersession exercises."""

from __future__ import annotations

import argparse
import json


RULES = {
    1: {"step": 1, "scope": "legacy"},
    2: {"step": 2, "scope": "replacement"},
}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--revision", type=int, required=True)
    args = parser.parse_args()
    rule = RULES.get(args.revision)
    if rule is None:
        print(json.dumps({"accepted": False, "error": "unknown_revision"}, sort_keys=True))
        return 2
    print(json.dumps({"accepted": True, "revision": args.revision, **rule}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
