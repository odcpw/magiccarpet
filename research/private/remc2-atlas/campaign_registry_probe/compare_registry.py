#!/usr/bin/env python3
"""Diff the pinned remc2 action-key universe against the Rust registry.

The Rust registry is deliberately hand-authored: the extractor measures the
source dispatch table, while this tool checks that each claimed Rust behavior
key still exists in that pinned table.  Source keys with no Rust entry remain
visible as campaign coverage, not as an implied implementation requirement.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any, NoReturn

HERE = Path(__file__).resolve().parent
if str(HERE) not in sys.path:
    sys.path.insert(0, str(HERE))

from extract_registry import PINNED_COMMIT, extract, json_bytes
from extract_registry import SCHEMA as SOURCE_SCHEMA

REGISTRY_SCHEMA = "mc2-rust-behavior-registry/v1"
REPORT_SCHEMA = "mc2-str-d4c48ar-rust-diff/v1"


def fail(message: str) -> NoReturn:
    raise ValueError(message)


def repo_relative(path: Path, root: Path) -> str:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return str(path.resolve())


def load_registry(path: Path) -> tuple[dict[str, Any], str, dict[str, dict[str, Any]]]:
    try:
        raw_bytes = path.read_bytes()
        payload = json.loads(raw_bytes)  # ubs:ignore — JSONDecodeError is handled below
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"cannot read Rust registry {path}: {exc}")
    if not isinstance(payload, dict):
        fail("Rust registry root must be an object")
    if payload.get("schema") != REGISTRY_SCHEMA:
        fail(f"unexpected Rust registry schema: {payload.get('schema')!r}")
    if payload.get("source_commit") != PINNED_COMMIT:
        fail(
            "Rust registry source_commit does not match the pinned source: "
            f"{payload.get('source_commit')!r} != {PINNED_COMMIT}"
        )
    entries = payload.get("entries")
    if not isinstance(entries, list) or not entries:
        fail("Rust registry entries must be a non-empty list")

    seen_names: set[str] = set()
    expanded: dict[str, dict[str, Any]] = {}
    for ordinal, entry in enumerate(entries):
        if not isinstance(entry, dict):
            fail(f"Rust registry entry {ordinal} must be an object")
        name = entry.get("name")
        if not isinstance(name, str) or not name:
            fail(f"Rust registry entry {ordinal} needs a non-empty name")
        if name in seen_names:
            fail(f"Rust registry contains duplicate entry name {name!r}")
        seen_names.add(name)
        class_value = entry.get("class")
        if (
            isinstance(class_value, bool)
            or not isinstance(class_value, int)
            or not 0 <= class_value <= 255
        ):
            fail(f"Rust registry entry {ordinal} has invalid class {class_value!r}")
        models = entry.get("models")
        if not isinstance(models, list) or not models or not all(
            isinstance(model, int) and not isinstance(model, bool) and 0 <= model <= 255
            for model in models
        ):
            fail(f"Rust registry entry {name} has invalid models")
        scopes = entry.get("level_scopes")
        if not isinstance(scopes, dict) or not scopes:
            fail(f"Rust registry entry {name} needs level_scopes")
        entry_actions: set[int] = set()
        for level, actions in scopes.items():
            if level not in {"1", "2"}:
                fail(f"Rust registry entry {name} has invalid level {level!r}")
            if not isinstance(actions, list) or not actions:
                fail(f"Rust registry entry {name} has invalid actions for level {level}")
            if actions != sorted(set(actions)):
                fail(f"Rust registry entry {name} actions must be sorted and unique")
            if any(
                isinstance(action, bool) or not isinstance(action, int) or not 0 <= action <= 255
                for action in actions
            ):
                fail(f"Rust registry entry {name} has an invalid action")
            entry_actions.update(actions)
        for field in ("rust_paths", "evidence"):
            value = entry.get(field)
            if not isinstance(value, list) or not value or not all(
                isinstance(item, str) and item for item in value
            ):
                fail(f"Rust registry entry {name} has invalid {field}")
        if not isinstance(entry.get("behavior"), str) or not entry["behavior"]:
            fail(f"Rust registry entry {name} needs a behavior description")
        for action in sorted(entry_actions):
            expected_key = f"{class_value}:{action}"
            if expected_key in expanded:
                fail(f"Rust registry contains duplicate key {expected_key}")
            expanded[expected_key] = {
                **entry,
                "key": expected_key,
                "levels": sorted(scopes),
            }
    return payload, hashlib.sha256(raw_bytes).hexdigest(), expanded


def build_report(repo_root: Path, registry_path: Path | None = None) -> dict[str, Any]:
    root = repo_root.resolve()
    registry_file = registry_path or HERE / "rust_behavior_registry.json"
    registry, registry_sha256, registry_keys = load_registry(registry_file)
    inventory = extract(root)
    source_keys = {
        item["key"]: item for item in inventory["source_behavior_keys"]
    }
    matching_keys = sorted(source_keys.keys() & registry_keys.keys(), key=lambda key: tuple(map(int, key.split(":"))))
    missing_source_keys = sorted(
        registry_keys.keys() - source_keys.keys(), key=lambda key: tuple(map(int, key.split(":")))
    )
    unimplemented_source_keys = sorted(
        source_keys.keys() - registry_keys.keys(), key=lambda key: tuple(map(int, key.split(":")))
    )

    return {
        "schema": REPORT_SCHEMA,
        "source_schema": SOURCE_SCHEMA,
        "source": {
            "commit": inventory["source"]["commit"],
            "class_table": inventory["source"]["class_table"],
            "dispatch_source": inventory["source"]["dispatch_source"],
            "summary": inventory["summary"],
        },
        "registry": {
            "path": repo_relative(registry_file, root),
            "schema": registry["schema"],
            "sha256": registry_sha256,
            "entry_count": len(registry_keys),
            "entry_group_count": len(registry["entries"]),
        },
        "summary": {
            "matching_key_count": len(matching_keys),
            "implemented_missing_from_source_count": len(missing_source_keys),
            "source_not_yet_implemented_count": len(unimplemented_source_keys),
        },
        "matching_keys": [
            {
                "key": key,
                "registry": registry_keys[key],
                "source": source_keys[key],
            }
            for key in matching_keys
        ],
        "implemented_missing_from_source": [registry_keys[key] for key in missing_source_keys],
        "source_not_yet_implemented": [source_keys[key] for key in unimplemented_source_keys],
        "recommendation": {
            "decision": "keep_hand_authored_rust_registry_with_generated_source_cross_check",
            "rationale": [
                "The pinned table is a dispatch-slot measurement, not proof that a Rust behavior is implemented.",
                "A hand-authored entry carries the Rust path and evidence needed to review ownership.",
                "The generated diff catches registry keys that drift away from the pinned source while preserving the unimplemented source backlog.",
            ],
            "gate": {
                "fail_when": [
                    "implemented_missing_from_source_count is non-zero",
                    "the extractor cannot read the exact pinned commit or its row-count invariants fail",
                ],
                "review_when": [
                    "source_not_yet_implemented gains a key selected for the next campaign slice",
                    "a registry entry changes levels, Rust paths, evidence, or behavior wording",
                ],
            },
            "commands": [
                "python3 research/private/remc2-atlas/campaign_registry_probe/extract_registry.py --output /tmp/str-d4c48ar-inventory.json",
                "python3 research/private/remc2-atlas/campaign_registry_probe/compare_registry.py --output /tmp/str-d4c48ar-rust-diff.json",
            ],
            "non_claims": [
                "source_not_yet_implemented is not a claim that the Rust campaign is complete or incomplete by itself",
                "callback_identity is a best-effort identity recovered from the pinned dispatcher switch and is not a semantic equivalence proof",
            ],
        },
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=None)
    parser.add_argument("--registry", type=Path, default=HERE / "rust_behavior_registry.json")
    parser.add_argument("--output", type=Path, default=None)
    args = parser.parse_args(argv)
    root = args.repo_root.resolve() if args.repo_root else HERE.parents[3]
    try:
        payload = json_bytes(build_report(root, args.registry))
    except ValueError as exc:
        print(f"compare_registry.py: error: {exc}", file=sys.stderr)
        return 2
    if args.output is None:
        sys.stdout.buffer.write(payload)
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_bytes(payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
