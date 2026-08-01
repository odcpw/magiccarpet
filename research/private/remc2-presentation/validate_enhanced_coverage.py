#!/usr/bin/env python3
"""Validate the closed Level 1 Unity Enhanced coverage audit."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


DEFAULT_MATRIX = Path(__file__).with_name("enhanced-coverage-matrix.json")
PINNED_REMC2 = "6806c7204d17220ecc0430759f784aea8ee2ddb2"
EXPECTED_FAMILIES = {
    "Reserved",
    "ImportedResetTuple",
    "AuthoredActivationTuple",
    "AuthoredTerrainFeature",
    "AuthoredPlayerMarker",
    "AuthoredSwitch",
    "PlayerFireball",
    "ArcherArrow",
    "FireflyFireball",
    "CastleBuild",
    "CastleRoot",
    "CastleConstruction",
    "CastleDefenseController",
    "CastleDefenseShot",
    "CastleAssociation",
    "Possession",
    "PossessionImpact",
    "GroundFire",
    "PossessionAttraction",
    "StrongPossessionImpact",
    "FireballImpact",
    "WaterSplash",
    "ManaSphere",
    "ManaCarrier",
    "DeathBurst",
    "PlayerDeathMarker",
    "SpeedTrail",
    "CheckpointSmoke",
    "SpellPickup",
    "Ambient",
    "MultipartSegment",
    "ResidentArcher",
    "ResidentBuilder",
    "ResidentVillager",
    "ResidentTrader",
    "Goat",
}
ALLOWED_ROUTES = {"existing-additive-v2", "new-gfx-q", "explicit-exclusion", "blocking-gap"}
WILDCARD_KEYS = {"*", "all", "any", "wildcard", "catch-all"}


def load_matrix(path: Path = DEFAULT_MATRIX) -> dict[str, Any]:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def validate_matrix(matrix: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if matrix.get("schema_version") != 1:
        errors.append("schema_version must be 1")
    if matrix.get("track") != "P-private-owner-data":
        errors.append("matrix must remain on Track P private owner-data")
    source = matrix.get("source_census", {})
    if source.get("pinned_source_commit") != PINNED_REMC2:
        errors.append("source census must remain pinned to remc2 commit 6806c720")
    if "dirty" in source.get("read_policy", "").lower() and "never" not in source.get("read_policy", "").lower():
        errors.append("source read policy must reject dirty upstream inference")
    authority = matrix.get("authority", {})
    if authority.get("gameplay") != "Rust simulation core":
        errors.append("Rust must remain gameplay authority")
    if authority.get("unity_feedback_to_gameplay") is not False:
        errors.append("Unity feedback into gameplay must remain false")
    if authority.get("owner_payloads_in_player_build") is not False:
        errors.append("owner payloads must not enter a player build")
    coverage = matrix.get("coverage", [])
    if not coverage:
        errors.append("coverage matrix must not be empty")
    ids: set[str] = set()
    family_rows: dict[str, dict[str, Any]] = {}
    for row in coverage:
        row_id = row.get("id")
        if not isinstance(row_id, str) or not row_id:
            errors.append("every coverage row needs a non-empty id")
        elif row_id in ids:
            errors.append(f"duplicate coverage id: {row_id}")
        else:
            ids.add(row_id)
        route = row.get("route")
        if route not in ALLOWED_ROUTES:
            errors.append(f"{row_id}: invalid route {route!r}")
        for key in ("domain", "source_owner", "current_rust_surface", "unity_consumer"):
            if not isinstance(row.get(key), str) or not row[key]:
                errors.append(f"{row_id}: missing {key}")
        fields = row.get("state_fields")
        if not isinstance(fields, list):
            errors.append(f"{row_id}: state_fields must be a list")
        if "family_key" in row:
            family = row["family_key"]
            if family in WILDCARD_KEYS:
                errors.append(f"{row_id}: wildcard family is forbidden")
            if family in family_rows:
                errors.append(f"duplicate family row: {family}")
            family_rows[family] = row
            if route == "explicit-exclusion":
                if not row.get("exclusion_reason"):
                    errors.append(f"{row_id}: explicit exclusion needs a reason")
            elif route != "new-gfx-q":
                errors.append(f"{row_id}: supported family must route to GFX-Q or explicit exclusion")
        elif route == "explicit-exclusion":
            errors.append(f"{row_id}: non-family exclusion is ambiguous")
        elif route == "blocking-gap" and not row.get("gap_reason"):
            errors.append(f"{row_id}: blocking gap needs a named reason")
    missing = EXPECTED_FAMILIES - set(family_rows)
    extra = set(family_rows) - EXPECTED_FAMILIES
    errors.extend(f"missing Level 1 family: {family}" for family in sorted(missing))
    errors.extend(f"unexpected family key: {family}" for family in sorted(extra))
    if not any(row.get("route") == "blocking-gap" for row in coverage):
        errors.append("matrix must retain at least one explicit blocking gap")
    failure_policy = matrix.get("failure_policy", [])
    if len(failure_policy) < 3:
        errors.append("failure policy must cover stale/unknown/duplicate behavior")
    return errors


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--matrix", type=Path, default=DEFAULT_MATRIX)
    parser.add_argument("--json", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        errors = validate_matrix(load_matrix(args.matrix))
    except (OSError, json.JSONDecodeError, KeyError) as error:
        errors = [str(error)]
    result = {"pass": not errors, "matrix": str(args.matrix), "errors": errors}
    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    elif errors:
        print("ENHANCED COVERAGE FAIL")
        for error in errors:
            print(f"  - {error}")
    else:
        print(f"ENHANCED COVERAGE PASS: {len(EXPECTED_FAMILIES)} typed Level 1 families plus explicit gaps")
    return 0 if not errors else 1


if __name__ == "__main__":
    sys.exit(main())
