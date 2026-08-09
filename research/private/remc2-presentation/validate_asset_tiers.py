#!/usr/bin/env python3
"""Validate the private Classic HD asset-tier contract without loading assets."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parent
SCHEMA = ROOT / "asset-tier-schema.json"
MANIFEST = ROOT / "asset-tier-manifest.json"


def validate(schema: dict[str, Any], manifest: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if schema.get("schema_id") != "mc-classic-hd-asset-tier-v1":
        errors.append("schema identity mismatch")
    if schema.get("schema_version") != 1:
        errors.append("schema version mismatch")
    if manifest.get("schema_id") != schema.get("schema_id"):
        errors.append("manifest schema identity mismatch")
    if manifest.get("schema_version") != schema.get("schema_version"):
        errors.append("manifest schema version mismatch")
    if manifest.get("owner_payload_embedded") is not False:
        errors.append("owner payload must never be embedded")
    policy = schema.get("selection_policy", {})
    if policy.get("select_before_session") is not True:
        errors.append("tier selection must happen before session")
    if policy.get("family_mixing") != "forbidden":
        errors.append("family mixing must be forbidden")
    statuses = set(schema.get("statuses", []))
    families = set(schema.get("families", {}))
    entries = manifest.get("entries")
    if not isinstance(entries, list) or not entries:
        errors.append("manifest entries must be a non-empty list")
        entries = []
    seen: set[tuple[str, str]] = set()
    required_fields = set(schema.get("required_entry_fields", []))
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            errors.append(f"entry {index} is not an object")
            continue
        missing = required_fields - set(entry)
        if missing:
            errors.append(f"entry {index} missing fields {sorted(missing)}")
        family = entry.get("family")
        tier = entry.get("tier")
        key = (str(family), str(tier))
        if key in seen:
            errors.append(f"duplicate entry {key}")
        seen.add(key)
        if family not in families:
            errors.append(f"entry {index} has unknown family {family!r}")
        if tier not in {"original-indexed", "optional-high-resolution"}:
            errors.append(f"entry {index} has unknown tier {tier!r}")
        if entry.get("status") not in statuses:
            errors.append(f"entry {index} has unknown status")
        if entry.get("sha256") is not None and (
            not isinstance(entry.get("sha256"), str)
            or len(entry["sha256"]) != 64
            or any(character not in "0123456789abcdef" for character in entry["sha256"])
        ):
            errors.append(f"entry {index} has invalid sha256")
        if entry.get("status") == "validated" and entry.get("sha256") is None:
            errors.append(f"entry {index} validated without sha256")
        if entry.get("status") in {"not-discovered", "fallback-original"} and not entry.get("fallback_reason"):
            errors.append(f"entry {index} missing fallback reason")
    for family in families:
        if (family, "original-indexed") not in seen:
            errors.append(f"family {family} lacks mandatory original entry")
    fallbacks = manifest.get("fallbacks")
    if not isinstance(fallbacks, dict) or set(fallbacks) != families:
        errors.append("fallbacks must cover every family exactly")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--schema", type=Path, default=SCHEMA)
    parser.add_argument("--manifest", type=Path, default=MANIFEST)
    args = parser.parse_args()
    schema = json.loads(args.schema.read_text(encoding="utf-8"))
    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    errors = validate(schema, manifest)
    if errors:
        for error in errors:
            print(f"FAIL {error}")
        return 1
    print(f"PASS asset-tier schema and manifest ({len(manifest['entries'])} entries)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
