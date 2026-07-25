#!/usr/bin/env python3
"""Prepare the fixed private runtime config used by repeatable trace runs."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--owner-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    owner = args.owner_root.resolve(strict=True)
    config = json.loads((owner / "regression-config.json").read_text(encoding="utf-8"))
    settings = config["settings"][0]
    settings["paths"]["gameFolder"] = str(owner / "GAME/NETHERW")
    settings["paths"]["cdFolder"] = str(owner / "CD_Files")
    settings["sound"]["hqSound"] = False
    settings["sound"]["oggMusic"] = False
    # remc2 selects GameRenderHD only above the legacy 640x480 ceiling. The
    # final TMAP/terrain hooks below are GameRenderHD entry points, while the
    # original owner-data textures remain selected by useHighResGraphics=false.
    settings["graphics"]["windowResWidth"] = 800
    settings["graphics"]["windowResHeight"] = 600
    settings["graphics"]["gameDetail"]["gameResWidth"] = 800
    settings["graphics"]["gameDetail"]["gameResHeight"] = 600
    settings["graphics"]["gameDetail"]["useHighResGraphics"] = False
    settings["graphics"]["threading"]["isActive"] = False
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(config, sort_keys=True, indent=2) + "\n", encoding="utf-8"
    )
    args.output.chmod(0o600)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
