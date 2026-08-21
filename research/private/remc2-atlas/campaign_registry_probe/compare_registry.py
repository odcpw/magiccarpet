#!/usr/bin/env python3
"""Diff pinned remc2 dispatch sites against the compiled Rust registry.

The Rust registry is deliberately hand-authored: the extractor measures the
source dispatch table, while this tool checks that each claimed Rust behavior
key still exists at the same source site with the same selector shape.  Source
keys with no Rust entry remain campaign coverage, not an implied implementation
requirement.
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

REGISTRY_SCHEMA = "mc2-rust-behavior-registry/v2"
REPORT_SCHEMA = "mc2-str-d4c48ar-rust-diff/v2"

ACTION_SITES = ["ApplyEvents", "UpdateEntities"]
CONSTRUCTOR_SITE = "Constructor"


def fail(message: str) -> NoReturn:
    raise ValueError(message)


def repo_relative(path: Path, root: Path) -> str:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return str(path.resolve())


def exact_fields(value: dict[str, Any], expected: set[str], label: str) -> None:
    actual = set(value)
    if actual != expected:
        fail(
            f"{label} field set differs: missing={sorted(expected - actual)}, "
            f"unknown={sorted(actual - expected)}"
        )


def u8(value: Any, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= 255:
        fail(f"{label} is not a u8: {value!r}")
    return value


def sorted_unique_u8_list(value: Any, label: str, allow_empty: bool = False) -> list[int]:
    if not isinstance(value, list) or (not value and not allow_empty):
        fail(f"{label} must be {'a' if allow_empty else 'a non-'}empty list")
    items = [u8(item, f"{label}[]") for item in value]
    if items != sorted(set(items)):
        fail(f"{label} must be sorted and unique")
    return items


def action_key(site: str, runtime_class: int, action: int) -> str:
    return f"{site}:ClassAction:{runtime_class}:{action}"


def constructor_key(runtime_class: int, model: int) -> str:
    return f"Constructor:ClassModel:{runtime_class}:{model}"


def subtype_key(type_id: int, subtype: int) -> str:
    return f"TypeSubtype:TypeSubtype:{type_id}:{subtype}"


def require_implemented_keys_present(
    source_keys: set[str], registry_keys: set[str], label: str
) -> None:
    missing = sorted(registry_keys - source_keys)
    if missing:
        fail(f"{label} contains {len(missing)} key(s) absent from pinned source: {missing[:3]}")


def load_registry(
    path: Path,
) -> tuple[
    dict[str, Any],
    str,
    dict[str, dict[str, Any]],
    dict[str, dict[str, Any]],
]:
    try:
        raw_bytes = path.read_bytes()
        payload = json.loads(raw_bytes)  # ubs:ignore — JSONDecodeError is handled below
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"cannot read Rust registry {path}: {exc}")
    if not isinstance(payload, dict):
        fail("Rust registry root must be an object")
    exact_fields(
        payload,
        {
            "action_dispatch_sites",
            "constructor_dispatch_site",
            "description",
            "entries",
            "schema",
            "selection",
            "source_commit",
            "type_subtype_entries",
        },
        "Rust registry",
    )
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

    if payload["action_dispatch_sites"] != ACTION_SITES:
        fail("action dispatch sites must preserve ApplyEvents then UpdateEntities")
    if payload["constructor_dispatch_site"] != CONSTRUCTOR_SITE:
        fail("constructor dispatch site was flattened or renamed")
    for field in ("description", "selection"):
        if not isinstance(payload[field], str) or not payload[field]:
            fail(f"Rust registry {field} must be a non-empty string")

    seen_names: set[str] = set()
    expanded: dict[str, dict[str, Any]] = {}
    site_expanded: dict[str, dict[str, Any]] = {}
    constructor_owners: set[tuple[int, int]] = set()
    declaration_order: list[tuple[int, int, str]] = []
    for ordinal, entry in enumerate(entries):
        if not isinstance(entry, dict):
            fail(f"Rust registry entry {ordinal} must be an object")
        exact_fields(
            entry,
            {
                "behavior",
                "class",
                "constructor_models",
                "evidence",
                "level_scopes",
                "models",
                "name",
                "rust_paths",
            },
            f"Rust registry entry {ordinal}",
        )
        name = entry.get("name")
        if not isinstance(name, str) or not name:
            fail(f"Rust registry entry {ordinal} needs a non-empty name")
        if name in seen_names:
            fail(f"Rust registry contains duplicate entry name {name!r}")
        seen_names.add(name)
        class_value = u8(entry.get("class"), f"Rust registry entry {name}.class")
        models = sorted_unique_u8_list(entry.get("models"), f"Rust registry entry {name}.models")
        constructor_models = sorted_unique_u8_list(
            entry.get("constructor_models"),
            f"Rust registry entry {name}.constructor_models",
            allow_empty=True,
        )
        if not set(constructor_models) <= set(models):
            fail(f"Rust registry entry {name} constructor_models escape models")
        scopes = entry.get("level_scopes")
        if not isinstance(scopes, dict) or not scopes:
            fail(f"Rust registry entry {name} needs level_scopes")
        entry_actions: set[int] = set()
        for level, actions in scopes.items():
            if level not in {"1", "2"}:
                fail(f"Rust registry entry {name} has invalid level {level!r}")
            if not isinstance(actions, list) or not actions:
                fail(f"Rust registry entry {name} has invalid actions for level {level}")
            entry_actions.update(
                sorted_unique_u8_list(
                    actions, f"Rust registry entry {name}.level_scopes[{level}]"
                )
            )
        for field in ("rust_paths", "evidence"):
            value = entry.get(field)
            if not isinstance(value, list) or not value or not all(
                isinstance(item, str) and item for item in value
            ):
                fail(f"Rust registry entry {name} has invalid {field}")
        if not isinstance(entry.get("behavior"), str) or not entry["behavior"]:
            fail(f"Rust registry entry {name} needs a behavior description")
        actions = sorted(entry_actions)
        declaration_order.append((class_value, actions[0], name))
        for action in actions:
            expected_key = f"{class_value}:{action}"
            if expected_key in expanded:
                fail(f"Rust registry contains duplicate key {expected_key}")
            expanded[expected_key] = {
                **entry,
                "key": expected_key,
                "levels": sorted(scopes),
            }
            for site in ACTION_SITES:
                key = action_key(site, class_value, action)
                if key in site_expanded:
                    fail(f"Rust registry contains duplicate site key {key}")
                site_expanded[key] = {
                    **expanded[expected_key],
                    "dispatch_site": site,
                    "selector_kind": "ClassAction",
                    "reachable_successors": (
                        [action_key("UpdateEntities", class_value, action)]
                        if site == "ApplyEvents"
                        else []
                    ),
                }
        for model in constructor_models:
            owner = (class_value, model)
            if owner in constructor_owners:
                fail(f"Rust registry contains duplicate constructor {owner}")
            constructor_owners.add(owner)
            key = constructor_key(class_value, model)
            site_expanded[key] = {
                **entry,
                "key": key,
                "dispatch_site": CONSTRUCTOR_SITE,
                "selector_kind": "ClassModel",
                "reachable_successors": [],
            }

    if declaration_order != sorted(declaration_order):
        fail("Rust registry family declarations are not in source-key order")

    subtype_entries = payload["type_subtype_entries"]
    if not isinstance(subtype_entries, list) or not subtype_entries:
        fail("Rust registry type_subtype_entries must be a non-empty list")
    subtype_order: list[tuple[int, int]] = []
    for ordinal, entry in enumerate(subtype_entries):
        if not isinstance(entry, dict):
            fail(f"type_subtype entry {ordinal} must be an object")
        exact_fields(
            entry,
            {
                "evidence",
                "handler",
                "name",
                "reachable_constructor",
                "subtype",
                "type_id",
            },
            f"type_subtype entry {ordinal}",
        )
        type_id = u8(entry["type_id"], f"type_subtype entry {ordinal}.type_id")
        subtype = u8(entry["subtype"], f"type_subtype entry {ordinal}.subtype")
        if entry["handler"] not in seen_names:
            fail(f"type_subtype entry {ordinal} names an unknown handler")
        reachable = entry["reachable_constructor"]
        if not isinstance(reachable, dict):
            fail(f"type_subtype entry {ordinal}.reachable_constructor must be an object")
        exact_fields(reachable, {"class", "model"}, "reachable_constructor")
        if (reachable["class"], reachable["model"]) != (type_id, subtype):
            fail(f"type_subtype entry {ordinal} flattens its constructor successor")
        target = constructor_key(type_id, subtype)
        if target not in site_expanded:
            fail(f"type_subtype entry {ordinal} lacks compiled constructor {target}")
        for field in ("name",):
            if not isinstance(entry[field], str) or not entry[field]:
                fail(f"type_subtype entry {ordinal}.{field} must be non-empty")
        evidence = entry["evidence"]
        if not isinstance(evidence, list) or not evidence or not all(
            isinstance(item, str) and item for item in evidence
        ):
            fail(f"type_subtype entry {ordinal}.evidence must be non-empty strings")
        key = subtype_key(type_id, subtype)
        if key in site_expanded:
            fail(f"Rust registry contains duplicate site key {key}")
        site_expanded[key] = {
            **entry,
            "key": key,
            "dispatch_site": "TypeSubtype",
            "selector_kind": "TypeSubtype",
            "reachable_successors": [target],
        }
        subtype_order.append((type_id, subtype))
    if subtype_order != sorted(set(subtype_order)):
        fail("type_subtype entries must be sorted and unique")

    return payload, hashlib.sha256(raw_bytes).hexdigest(), expanded, site_expanded


def build_report(repo_root: Path, registry_path: Path | None = None) -> dict[str, Any]:
    root = repo_root.resolve()
    registry_file = registry_path or HERE / "rust_behavior_registry.json"
    registry, registry_sha256, registry_keys, registry_site_keys = load_registry(
        registry_file
    )
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
    source_site_keys = {item["key"]: item for item in inventory["source_site_keys"]}
    matching_site_keys = sorted(source_site_keys.keys() & registry_site_keys.keys())
    missing_source_site_keys = sorted(registry_site_keys.keys() - source_site_keys.keys())
    unimplemented_source_site_keys = sorted(source_site_keys.keys() - registry_site_keys.keys())
    require_implemented_keys_present(
        set(source_keys), set(registry_keys), "Rust selector registry"
    )
    require_implemented_keys_present(
        set(source_site_keys), set(registry_site_keys), "Rust dispatch-site registry"
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
            "compiled_site_key_count": len(registry_site_keys),
            "constructor_key_count": sum(
                item["dispatch_site"] == "Constructor"
                for item in registry_site_keys.values()
            ),
            "type_subtype_key_count": sum(
                item["dispatch_site"] == "TypeSubtype"
                for item in registry_site_keys.values()
            ),
        },
        "summary": {
            "matching_key_count": len(matching_keys),
            "implemented_missing_from_source_count": len(missing_source_keys),
            "source_not_yet_implemented_count": len(unimplemented_source_keys),
            "matching_site_key_count": len(matching_site_keys),
            "implemented_site_key_missing_from_source_count": len(
                missing_source_site_keys
            ),
            "source_site_not_yet_implemented_count": len(
                unimplemented_source_site_keys
            ),
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
        "matching_site_keys": matching_site_keys,
        "implemented_site_keys_missing_from_source": missing_source_site_keys,
        "source_site_keys_not_yet_implemented": unimplemented_source_site_keys,
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
                    "implemented_site_key_missing_from_source_count is non-zero",
                    "a dispatch site is flattened into another selector shape",
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
                "the extractor emits inventory only and never emits Rust declarations or handlers",
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
