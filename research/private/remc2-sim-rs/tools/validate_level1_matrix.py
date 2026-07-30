#!/usr/bin/env python3
"""Fail-closed validator for the canonical Level 1 conformance matrix."""

from __future__ import annotations

import argparse
import collections
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

SCHEMA_ID = "mc2-level1-conformance-matrix"
SCHEMA_VERSION = 2
PINNED_REMC2_COMMIT = "6806c7204d17220ecc0430759f784aea8ee2ddb2"
EXPECTED_AUTHORED_INDICES = 308
EXPECTED_DISPATCH_SLOTS = 845
EXPECTED_CHECK_IDS = {
    "unique-row-ids",
    "totals",
    "exact-evidence",
    "rust-test-resolution",
    "non-exact-ownership",
    "plan-acyclic",
    "authored-census",
    "dispatch-census",
}
CLASSIFICATIONS = {"Exact", "Partial", "Missing", "Wrong", "Unproved"}
CONFIDENCE_LEVELS = {"high", "medium", "low"}
REACHABILITY_STATUSES = {"reachable", "conditional"}
SLOT_ID_RE = re.compile(r"^c\d{2}-(?:action|constructor)-\d{3}$")
TEST_LINE_RE = re.compile(r"^(.+): test$")

ROOT_KEYS = {
    "schema",
    "meta",
    "authored_census",
    "dispatch_census",
    "rows",
    "exclusions",
    "plan_items",
    "validator_contract",
}
ROW_KEYS = {
    "id",
    "subsystem",
    "family",
    "title",
    "reachability",
    "source",
    "preconditions",
    "behavior_ordered",
    "implementation",
    "classification",
    "mismatch_detail",
    "evidence_bindings",
    "negative_coverage_required",
    "plan_item",
    "confidence",
    "unresolved",
}
ROW_DESCRIPTOR_KEYS = ROW_KEYS | {"authored_indices"}


class ValidationError(Exception):
    """The matrix or its live evidence inventory violates the v2 contract."""


def fail(message: str) -> None:
    raise ValidationError(message)


def expect_object(value: Any, path: str) -> dict[str, Any]:
    if type(value) is not dict:
        fail(f"{path}: expected object, got {type(value).__name__}")
    return value


def expect_array(value: Any, path: str) -> list[Any]:
    if type(value) is not list:
        fail(f"{path}: expected array, got {type(value).__name__}")
    return value


def expect_string(value: Any, path: str) -> str:
    if type(value) is not str or not value:
        fail(f"{path}: expected non-empty string")
    return value


def expect_int(value: Any, path: str, *, minimum: int | None = None) -> int:
    if type(value) is not int:
        fail(f"{path}: expected integer, got {type(value).__name__}")
    if minimum is not None and value < minimum:
        fail(f"{path}: expected integer >= {minimum}, got {value}")
    return value


def expect_bool(value: Any, path: str) -> bool:
    if type(value) is not bool:
        fail(f"{path}: expected boolean, got {type(value).__name__}")
    return value


def expect_keys(
    value: Any,
    path: str,
    required: set[str],
    optional: set[str] | None = None,
) -> dict[str, Any]:
    obj = expect_object(value, path)
    optional = optional or set()
    missing = sorted(required - obj.keys())
    unknown = sorted(obj.keys() - required - optional)
    if missing:
        fail(f"{path}: missing fields: {', '.join(missing)}")
    if unknown:
        fail(f"{path}: unknown fields: {', '.join(unknown)}")
    return obj


def string_array(value: Any, path: str, *, unique: bool = False) -> list[str]:
    items = expect_array(value, path)
    result = [expect_string(item, f"{path}[{index}]") for index, item in enumerate(items)]
    if unique and len(result) != len(set(result)):
        fail(f"{path}: duplicate values are not allowed")
    return result


def int_array(value: Any, path: str, *, unique: bool = False) -> list[int]:
    items = expect_array(value, path)
    result = [expect_int(item, f"{path}[{index}]") for index, item in enumerate(items)]
    if unique and len(result) != len(set(result)):
        fail(f"{path}: duplicate values are not allowed")
    return result


def duplicate_rejecting_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            fail(f"JSON object contains duplicate field {key!r}")
        result[key] = value
    return result


def reject_json_constant(value: str) -> None:
    fail(f"invalid JSON numeric constant {value}")


def load_matrix(path: Path) -> dict[str, Any]:
    try:
        with path.open("r", encoding="utf-8") as handle:
            value = json.load(
                handle,
                object_pairs_hook=duplicate_rejecting_object,
                parse_constant=reject_json_constant,
            )
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        fail(f"{path}: cannot load canonical JSON: {error}")
    return expect_object(value, "$")


def capture_cargo_test_inventory(workspace_root: Path) -> collections.Counter[str]:
    command = ["cargo", "test", "--workspace", "--locked", "--", "--list"]
    environment = dict(os.environ)
    environment["CARGO_TERM_COLOR"] = "never"
    try:
        completed = subprocess.run(
            command,
            cwd=workspace_root,
            env=environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
    except OSError as error:
        fail(f"cargo test inventory command could not start: {error}")
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        fail(
            "cargo test --workspace --locked -- --list failed "
            f"with exit {completed.returncode}: {detail}"
        )
    inventory: collections.Counter[str] = collections.Counter()
    for line in completed.stdout.splitlines():
        match = TEST_LINE_RE.fullmatch(line)
        if match:
            inventory[match.group(1)] += 1
    if not inventory:
        fail("cargo test inventory contained no test names")
    duplicates = sorted(name for name, count in inventory.items() if count != 1)
    if duplicates:
        fail(f"cargo test inventory contains ambiguous duplicate name {duplicates[0]!r}")
    return inventory


def validate_schema(value: Any) -> None:
    schema = expect_keys(
        value,
        "$.schema",
        {
            "id",
            "version",
            "description",
            "row_fields",
            "binding_rules",
            "classification_rule",
        },
    )
    if schema["id"] != SCHEMA_ID:
        fail(f"$.schema.id: expected {SCHEMA_ID!r}")
    if type(schema["version"]) is not int or schema["version"] != SCHEMA_VERSION:
        fail(f"$.schema.version: expected integer {SCHEMA_VERSION}")
    expect_string(schema["description"], "$.schema.description")
    row_fields = expect_keys(schema["row_fields"], "$.schema.row_fields", ROW_DESCRIPTOR_KEYS)
    for key in ROW_DESCRIPTOR_KEYS - {"reachability", "source", "implementation"}:
        expect_string(row_fields[key], f"$.schema.row_fields.{key}")
    for field, keys in {
        "reachability": {"status", "chain"},
        "source": {"commit", "symbols", "path", "lines", "dispatch", "related"},
        "implementation": {"rust", "capi", "unity", "absent"},
    }.items():
        descriptor = expect_keys(
            row_fields[field], f"$.schema.row_fields.{field}", keys
        )
        for key in keys:
            expect_string(descriptor[key], f"$.schema.row_fields.{field}.{key}")
    rules = expect_keys(
        schema["binding_rules"],
        "$.schema.binding_rules",
        {"rust-test", "exact_rows", "qualification-record"},
    )
    for key, rule in rules.items():
        expect_string(rule, f"$.schema.binding_rules.{key}")
    expect_string(schema["classification_rule"], "$.schema.classification_rule")


def validate_evidence_binding(
    value: Any,
    path: str,
    row_id: str,
    classification: str,
    inventory: collections.Counter[str],
) -> str:
    common_keys = {"kind", "file", "test", "command", "proves"}
    initial = expect_object(value, path)
    kind = expect_string(initial.get("kind"), f"{path}.kind")
    if kind == "rust-test":
        binding = expect_keys(initial, path, common_keys)
    elif kind == "qualification-record":
        binding = expect_keys(initial, path, common_keys | {"non_executable"})
        if not expect_bool(binding["non_executable"], f"{path}.non_executable"):
            fail(f"{path}.non_executable: qualification records must be true")
        if classification == "Exact":
            fail(f"row {row_id}: Exact rows cannot use non-executable evidence")
    else:
        fail(f"{path}.kind: unknown evidence kind {kind!r}")
    for key in common_keys:
        expect_string(binding[key], f"{path}.{key}")
    if kind == "rust-test":
        test_name = binding["test"]
        if inventory.get(test_name, 0) != 1:
            fail(
                f"row {row_id}: rust-test binding {test_name!r} "
                "does not resolve exactly once"
            )
    return kind


def validate_row(
    value: Any,
    index: int,
    inventory: collections.Counter[str],
) -> dict[str, Any]:
    path = f"$.rows[{index}]"
    row = expect_keys(value, path, ROW_KEYS, {"authored_indices"})
    row_id = expect_string(row["id"], f"{path}.id")
    expect_string(row["subsystem"], f"{path}.subsystem")
    expect_string(row["family"], f"{path}.family")
    expect_string(row["title"], f"{path}.title")

    reachability = expect_keys(
        row["reachability"], f"{path}.reachability", {"status", "chain"}
    )
    status = expect_string(reachability["status"], f"{path}.reachability.status")
    if status not in REACHABILITY_STATUSES:
        fail(f"{path}.reachability.status: unknown value {status!r}")
    expect_string(reachability["chain"], f"{path}.reachability.chain")

    source = expect_keys(
        row["source"],
        f"{path}.source",
        {"commit", "symbols", "path", "lines", "dispatch"},
        {"related"},
    )
    commit = expect_string(source["commit"], f"{path}.source.commit")
    if commit != PINNED_REMC2_COMMIT:
        fail(f"{path}.source.commit: expected pinned remc2 commit")
    string_array(source["symbols"], f"{path}.source.symbols", unique=True)
    expect_string(source["path"], f"{path}.source.path")
    expect_string(source["lines"], f"{path}.source.lines")
    if source["dispatch"] is not None:
        dispatch = expect_keys(
            source["dispatch"],
            f"{path}.source.dispatch",
            {"class", "model", "action_slot"},
        )
        expect_int(dispatch["class"], f"{path}.source.dispatch.class", minimum=0)
        for key in ("model", "action_slot"):
            if dispatch[key] is not None:
                expect_int(
                    dispatch[key], f"{path}.source.dispatch.{key}", minimum=0
                )
    related_sources = (
        expect_array(source["related"], f"{path}.source.related")
        if "related" in source
        else []
    )
    for related_index, related_value in enumerate(related_sources):
        related_path = f"{path}.source.related[{related_index}]"
        related = expect_keys(
            related_value,
            related_path,
            {"commit", "symbols", "path", "lines"},
        )
        related_commit = expect_string(related["commit"], f"{related_path}.commit")
        if related_commit != PINNED_REMC2_COMMIT:
            fail(f"{related_path}.commit: expected pinned remc2 commit")
        string_array(related["symbols"], f"{related_path}.symbols", unique=True)
        expect_string(related["path"], f"{related_path}.path")
        expect_string(related["lines"], f"{related_path}.lines")

    string_array(row["preconditions"], f"{path}.preconditions")
    string_array(row["behavior_ordered"], f"{path}.behavior_ordered")
    implementation = expect_keys(
        row["implementation"],
        f"{path}.implementation",
        {"rust", "capi", "unity", "absent"},
    )
    for key in ("rust", "capi", "unity"):
        string_array(implementation[key], f"{path}.implementation.{key}", unique=True)
    expect_bool(implementation["absent"], f"{path}.implementation.absent")

    classification = expect_string(row["classification"], f"{path}.classification")
    if classification not in CLASSIFICATIONS:
        fail(f"{path}.classification: unknown value {classification!r}")
    string_array(row["mismatch_detail"], f"{path}.mismatch_detail")
    bindings = expect_array(row["evidence_bindings"], f"{path}.evidence_bindings")
    binding_kinds = [
        validate_evidence_binding(
            binding,
            f"{path}.evidence_bindings[{binding_index}]",
            row_id,
            classification,
            inventory,
        )
        for binding_index, binding in enumerate(bindings)
    ]
    if classification == "Exact" and "rust-test" not in binding_kinds:
        fail(f"row {row_id}: Exact row has no resolving executable rust-test evidence")
    string_array(
        row["negative_coverage_required"], f"{path}.negative_coverage_required"
    )
    if classification == "Exact":
        if row["plan_item"] is not None:
            fail(f"row {row_id}: Exact row must not name a plan item")
    else:
        expect_string(row["plan_item"], f"{path}.plan_item")
    confidence = expect_string(row["confidence"], f"{path}.confidence")
    if confidence not in CONFIDENCE_LEVELS:
        fail(f"{path}.confidence: unknown value {confidence!r}")
    string_array(row["unresolved"], f"{path}.unresolved")
    if "authored_indices" in row:
        int_array(row["authored_indices"], f"{path}.authored_indices", unique=True)
    return row


def validate_exclusion(value: Any, index: int) -> dict[str, Any]:
    path = f"$.exclusions[{index}]"
    exclusion = expect_keys(
        value,
        path,
        {"id", "scope", "description", "mechanical_proof", "source_anchors", "commit"},
    )
    for key in ("id", "scope", "description", "mechanical_proof"):
        expect_string(exclusion[key], f"{path}.{key}")
    anchors = string_array(
        exclusion["source_anchors"], f"{path}.source_anchors", unique=True
    )
    if not anchors:
        fail(f"{path}.source_anchors: expected at least one source anchor")
    if exclusion["commit"] != PINNED_REMC2_COMMIT:
        fail(f"{path}.commit: expected pinned remc2 commit")
    return exclusion


def validate_plan(value: Any, index: int) -> dict[str, Any]:
    path = f"$.plan_items[{index}]"
    plan = expect_keys(
        value,
        path,
        {
            "id",
            "title",
            "depends_on",
            "acceptance",
            "allowed_surfaces",
            "not_done_if",
            "owned_rows",
        },
    )
    expect_string(plan["id"], f"{path}.id")
    expect_string(plan["title"], f"{path}.title")
    for key in ("depends_on", "acceptance", "allowed_surfaces", "not_done_if", "owned_rows"):
        string_array(plan[key], f"{path}.{key}", unique=True)
    return plan


def unique_id_map(items: list[dict[str, Any]], path: str) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    for index, item in enumerate(items):
        item_id = item["id"]
        if item_id in result:
            fail(f"{path}[{index}].id: duplicate ID {item_id!r}")
        result[item_id] = item
    return result


def validate_plan_graph(plans: dict[str, dict[str, Any]]) -> None:
    for plan_id, plan in plans.items():
        for dependency in plan["depends_on"]:
            if dependency not in plans:
                fail(f"plan {plan_id}: dangling depends_on reference {dependency!r}")

    state: dict[str, int] = {}
    stack: list[str] = []

    def visit(plan_id: str) -> None:
        marker = state.get(plan_id, 0)
        if marker == 1:
            cycle_start = stack.index(plan_id)
            cycle = stack[cycle_start:] + [plan_id]
            fail(f"plan dependency cycle: {' -> '.join(cycle)}")
        if marker == 2:
            return
        state[plan_id] = 1
        stack.append(plan_id)
        for dependency in plans[plan_id]["depends_on"]:
            visit(dependency)
        stack.pop()
        state[plan_id] = 2

    for plan_id in plans:
        visit(plan_id)


def validate_ownership(
    rows: dict[str, dict[str, Any]],
    plans: dict[str, dict[str, Any]],
) -> None:
    expected: dict[str, str] = {}
    for row_id, row in rows.items():
        if row["classification"] == "Exact":
            continue
        plan_id = row["plan_item"]
        if plan_id not in plans:
            fail(f"row {row_id}: dangling plan_item reference {plan_id!r}")
        expected[row_id] = plan_id

    actual: dict[str, str] = {}
    for plan_id, plan in plans.items():
        for row_id in plan["owned_rows"]:
            if row_id not in rows:
                fail(f"plan {plan_id}: owned_rows references unknown row {row_id!r}")
            if rows[row_id]["classification"] == "Exact":
                fail(f"plan {plan_id}: Exact row {row_id} must not be owned")
            if row_id in actual:
                fail(
                    f"row {row_id}: duplicate ownership by "
                    f"{actual[row_id]} and {plan_id}"
                )
            actual[row_id] = plan_id
    if expected != actual:
        missing = sorted(set(expected) - set(actual))
        extra = sorted(set(actual) - set(expected))
        wrong = sorted(
            row_id
            for row_id in set(expected) & set(actual)
            if expected[row_id] != actual[row_id]
        )
        details = []
        if missing:
            details.append(f"missing={missing}")
        if extra:
            details.append(f"extra={extra}")
        if wrong:
            details.append(
                "wrong="
                + repr(
                    [
                        (row_id, expected[row_id], actual[row_id])
                        for row_id in wrong
                    ]
                )
            )
        fail("plan owned_rows is not a bijection over non-Exact rows: " + "; ".join(details))


def validate_authored_census(
    value: Any,
    rows: dict[str, dict[str, Any]],
) -> None:
    path = "$.authored_census"
    census = expect_keys(
        value,
        path,
        {
            "record_count",
            "record_bytes",
            "table_offset",
            "nonzero_indices",
            "records",
            "row_coverage",
            "partition_proof",
        },
    )
    record_count = expect_int(census["record_count"], f"{path}.record_count", minimum=1)
    expect_int(census["record_bytes"], f"{path}.record_bytes", minimum=1)
    expect_int(census["table_offset"], f"{path}.table_offset", minimum=0)
    nonzero = int_array(
        census["nonzero_indices"], f"{path}.nonzero_indices", unique=True
    )
    if len(nonzero) != EXPECTED_AUTHORED_INDICES:
        fail(
            f"{path}.nonzero_indices: expected exactly "
            f"{EXPECTED_AUTHORED_INDICES} indices, got {len(nonzero)}"
        )
    for index, authored_index in enumerate(nonzero):
        if authored_index <= 0 or authored_index >= record_count:
            fail(
                f"{path}.nonzero_indices[{index}]: index {authored_index} "
                f"outside 1..{record_count - 1}"
            )

    records = expect_object(census["records"], f"{path}.records")
    expected_record_keys = {str(index) for index in nonzero}
    if set(records) != expected_record_keys:
        fail(f"{path}.records: keys must equal nonzero_indices exactly")
    for key, record_value in records.items():
        record = expect_keys(
            record_value,
            f"{path}.records.{key}",
            {"kind", "subtype", "activation_group"},
        )
        for field in ("kind", "subtype", "activation_group"):
            expect_int(record[field], f"{path}.records.{key}.{field}")

    coverage = expect_object(census["row_coverage"], f"{path}.row_coverage")
    seen: dict[int, str] = {}
    for row_id, indices_value in coverage.items():
        expect_string(row_id, f"{path}.row_coverage key")
        if row_id not in rows:
            fail(f"{path}.row_coverage: unknown row {row_id!r}")
        indices = int_array(
            indices_value, f"{path}.row_coverage.{row_id}", unique=True
        )
        if rows[row_id].get("authored_indices") != indices:
            fail(
                f"row {row_id}: authored_indices does not exactly equal "
                "authored_census.row_coverage"
            )
        for authored_index in indices:
            if authored_index in seen:
                fail(
                    f"{path}.row_coverage: index {authored_index} appears in "
                    f"both {seen[authored_index]} and {row_id}"
                )
            seen[authored_index] = row_id
    rows_with_authored = {
        row_id for row_id, row in rows.items() if "authored_indices" in row
    }
    if rows_with_authored != set(coverage):
        fail(
            f"{path}.row_coverage: rows do not exactly match rows carrying "
            "authored_indices"
        )
    if set(seen) != set(nonzero):
        missing = sorted(set(nonzero) - set(seen))
        extra = sorted(set(seen) - set(nonzero))
        fail(
            f"{path}.row_coverage: union does not equal nonzero_indices "
            f"(missing={missing}, extra={extra})"
        )

    proof = expect_keys(
        census["partition_proof"],
        f"{path}.partition_proof",
        {"unique", "disjoint", "union_equals_nonzero_indices", "covered", "declared"},
    )
    if not expect_bool(proof["unique"], f"{path}.partition_proof.unique"):
        fail(f"{path}.partition_proof.unique: must be true")
    if not expect_bool(proof["disjoint"], f"{path}.partition_proof.disjoint"):
        fail(f"{path}.partition_proof.disjoint: must be true")
    if not expect_bool(
        proof["union_equals_nonzero_indices"],
        f"{path}.partition_proof.union_equals_nonzero_indices",
    ):
        fail(f"{path}.partition_proof.union_equals_nonzero_indices: must be true")
    if expect_int(proof["covered"], f"{path}.partition_proof.covered") != len(seen):
        fail(f"{path}.partition_proof.covered: inconsistent count")
    if expect_int(proof["declared"], f"{path}.partition_proof.declared") != len(nonzero):
        fail(f"{path}.partition_proof.declared: inconsistent count")


def validate_dispatch_census(
    value: Any,
    rows: dict[str, dict[str, Any]],
    exclusions: dict[str, dict[str, Any]],
) -> None:
    path = "$.dispatch_census"
    census = expect_keys(
        value,
        path,
        {
            "source",
            "slot_id_format",
            "slot_count",
            "slots",
            "slot_to_row",
            "slot_to_exclusion",
            "partition_proof",
        },
    )
    expect_string(census["source"], f"{path}.source")
    expect_string(census["slot_id_format"], f"{path}.slot_id_format")
    slot_count = expect_int(census["slot_count"], f"{path}.slot_count", minimum=1)
    if slot_count != EXPECTED_DISPATCH_SLOTS:
        fail(
            f"{path}.slot_count: expected exactly "
            f"{EXPECTED_DISPATCH_SLOTS}, got {slot_count}"
        )
    slots = string_array(census["slots"], f"{path}.slots", unique=True)
    if len(slots) != slot_count:
        fail(f"{path}.slots: length does not equal slot_count")
    for index, slot_id in enumerate(slots):
        if not SLOT_ID_RE.fullmatch(slot_id):
            fail(f"{path}.slots[{index}]: malformed typed slot ID {slot_id!r}")
    slot_set = set(slots)

    slot_to_row = expect_object(census["slot_to_row"], f"{path}.slot_to_row")
    slot_to_exclusion = expect_object(
        census["slot_to_exclusion"], f"{path}.slot_to_exclusion"
    )
    overlap = sorted(set(slot_to_row) & set(slot_to_exclusion))
    if overlap:
        fail(f"{path}: slot maps overlap at {overlap[0]!r}")
    union = set(slot_to_row) | set(slot_to_exclusion)
    if union != slot_set:
        missing = sorted(slot_set - union)
        extra = sorted(union - slot_set)
        fail(
            f"{path}: slot maps do not exactly partition slots "
            f"(missing={missing}, extra={extra})"
        )

    for slot_id, row_ids_value in slot_to_row.items():
        if not SLOT_ID_RE.fullmatch(slot_id):
            fail(f"{path}.slot_to_row: malformed slot key {slot_id!r}")
        row_ids = string_array(
            row_ids_value, f"{path}.slot_to_row.{slot_id}", unique=True
        )
        if not row_ids:
            fail(f"{path}.slot_to_row.{slot_id}: expected at least one row ID")
        for row_id in row_ids:
            if row_id not in rows:
                fail(f"{path}.slot_to_row.{slot_id}: unknown row {row_id!r}")
    for slot_id, exclusion_id_value in slot_to_exclusion.items():
        if not SLOT_ID_RE.fullmatch(slot_id):
            fail(f"{path}.slot_to_exclusion: malformed slot key {slot_id!r}")
        exclusion_id = expect_string(
            exclusion_id_value, f"{path}.slot_to_exclusion.{slot_id}"
        )
        if exclusion_id not in exclusions:
            fail(
                f"{path}.slot_to_exclusion.{slot_id}: "
                f"unknown exclusion {exclusion_id!r}"
            )

    proof = expect_keys(
        census["partition_proof"],
        f"{path}.partition_proof",
        {"included_slots", "excluded_slots", "disjoint", "union_equals_slot_set"},
    )
    if expect_int(
        proof["included_slots"], f"{path}.partition_proof.included_slots"
    ) != len(slot_to_row):
        fail(f"{path}.partition_proof.included_slots: inconsistent count")
    if expect_int(
        proof["excluded_slots"], f"{path}.partition_proof.excluded_slots"
    ) != len(slot_to_exclusion):
        fail(f"{path}.partition_proof.excluded_slots: inconsistent count")
    if not expect_bool(proof["disjoint"], f"{path}.partition_proof.disjoint"):
        fail(f"{path}.partition_proof.disjoint: must be true")
    if not expect_bool(
        proof["union_equals_slot_set"],
        f"{path}.partition_proof.union_equals_slot_set",
    ):
        fail(f"{path}.partition_proof.union_equals_slot_set: must be true")


def validate_meta(
    value: Any,
    rows: list[dict[str, Any]],
    exclusion_count: int,
    plan_count: int,
    inventory: collections.Counter[str],
) -> None:
    path = "$.meta"
    meta = expect_keys(
        value,
        path,
        {
            "generated",
            "remc2_commit",
            "upstream_checkout",
            "owner_package",
            "implementation_under_assessment",
            "test_inventory",
            "census_method",
            "totals",
        },
    )
    expect_string(meta["generated"], f"{path}.generated")
    if meta["remc2_commit"] != PINNED_REMC2_COMMIT:
        fail(f"{path}.remc2_commit: expected pinned remc2 commit")
    for key in ("upstream_checkout", "owner_package", "census_method"):
        expect_string(meta[key], f"{path}.{key}")
    string_array(
        meta["implementation_under_assessment"],
        f"{path}.implementation_under_assessment",
        unique=True,
    )
    inventory_meta = expect_keys(
        meta["test_inventory"], f"{path}.test_inventory", {"command", "count"}
    )
    expected_command = (
        "cd research/private/remc2-sim-rs && "
        "cargo test --workspace --locked -- --list"
    )
    if inventory_meta["command"] != expected_command:
        fail(f"{path}.test_inventory.command: unexpected inventory command")
    actual_inventory_count = sum(inventory.values())
    declared_inventory_count = expect_int(
        inventory_meta["count"], f"{path}.test_inventory.count"
    )
    if declared_inventory_count != actual_inventory_count:
        fail(
            f"{path}.test_inventory.count: expected live Cargo inventory "
            f"count {actual_inventory_count}"
        )

    totals = expect_keys(
        meta["totals"],
        f"{path}.totals",
        {
            "rows",
            "by_classification",
            "by_subsystem",
            "reachable_rows",
            "exclusions",
            "plan_items",
        },
    )
    expected_scalars = {
        "rows": len(rows),
        "reachable_rows": sum(
            row["reachability"]["status"] == "reachable" for row in rows
        ),
        "exclusions": exclusion_count,
        "plan_items": plan_count,
    }
    for key, expected in expected_scalars.items():
        if expect_int(totals[key], f"{path}.totals.{key}") != expected:
            fail(f"{path}.totals.{key}: expected recomputed value {expected}")

    expected_by_classification = {
        classification: sum(row["classification"] == classification for row in rows)
        for classification in CLASSIFICATIONS
    }
    by_classification = expect_keys(
        totals["by_classification"],
        f"{path}.totals.by_classification",
        CLASSIFICATIONS,
    )
    for classification, expected in expected_by_classification.items():
        if expect_int(
            by_classification[classification],
            f"{path}.totals.by_classification.{classification}",
        ) != expected:
            fail(
                f"{path}.totals.by_classification.{classification}: "
                f"expected recomputed value {expected}"
            )

    expected_by_subsystem: dict[str, collections.Counter[str]] = {}
    for row in rows:
        expected_by_subsystem.setdefault(
            row["subsystem"], collections.Counter()
        )[row["classification"]] += 1
    by_subsystem = expect_object(totals["by_subsystem"], f"{path}.totals.by_subsystem")
    if set(by_subsystem) != set(expected_by_subsystem):
        fail(f"{path}.totals.by_subsystem: subsystem keys do not match rows")
    for subsystem, expected_counts in expected_by_subsystem.items():
        actual_counts = expect_keys(
            by_subsystem[subsystem],
            f"{path}.totals.by_subsystem.{subsystem}",
            set(expected_counts),
        )
        for classification, expected in expected_counts.items():
            if expect_int(
                actual_counts[classification],
                f"{path}.totals.by_subsystem.{subsystem}.{classification}",
            ) != expected:
                fail(
                    f"{path}.totals.by_subsystem.{subsystem}.{classification}: "
                    f"expected recomputed value {expected}"
                )


def validate_validator_contract(value: Any) -> None:
    contract = expect_keys(value, "$.validator_contract", {"checks"})
    checks = expect_array(contract["checks"], "$.validator_contract.checks")
    check_ids: list[str] = []
    for index, check_value in enumerate(checks):
        path = f"$.validator_contract.checks[{index}]"
        check = expect_keys(check_value, path, {"id", "rule"})
        check_ids.append(expect_string(check["id"], f"{path}.id"))
        expect_string(check["rule"], f"{path}.rule")
    if len(check_ids) != len(set(check_ids)):
        fail("$.validator_contract.checks: duplicate check IDs")
    if set(check_ids) != EXPECTED_CHECK_IDS:
        fail(
            "$.validator_contract.checks: IDs do not exactly match the "
            "schema-v2 validator contract"
        )


def validate_document(
    document: Any,
    inventory: collections.Counter[str],
) -> dict[str, int | bool]:
    root = expect_keys(document, "$", ROOT_KEYS)
    validate_schema(root["schema"])

    row_values = expect_array(root["rows"], "$.rows")
    rows_list = [
        validate_row(row, index, inventory) for index, row in enumerate(row_values)
    ]
    rows = unique_id_map(rows_list, "$.rows")

    exclusion_values = expect_array(root["exclusions"], "$.exclusions")
    exclusions_list = [
        validate_exclusion(exclusion, index)
        for index, exclusion in enumerate(exclusion_values)
    ]
    exclusions = unique_id_map(exclusions_list, "$.exclusions")

    plan_values = expect_array(root["plan_items"], "$.plan_items")
    plans_list = [
        validate_plan(plan, index) for index, plan in enumerate(plan_values)
    ]
    plans = unique_id_map(plans_list, "$.plan_items")

    all_ids = list(rows) + list(exclusions) + list(plans)
    if len(all_ids) != len(set(all_ids)):
        fail("row, exclusion, and plan IDs must also be globally unique")

    validate_plan_graph(plans)
    validate_ownership(rows, plans)
    validate_authored_census(root["authored_census"], rows)
    validate_dispatch_census(root["dispatch_census"], rows, exclusions)
    validate_meta(
        root["meta"],
        rows_list,
        len(exclusions),
        len(plans),
        inventory,
    )
    validate_validator_contract(root["validator_contract"])
    return {
        "ok": True,
        "rows": len(rows),
        "exact_rows": sum(row["classification"] == "Exact" for row in rows_list),
        "non_exact_rows": sum(
            row["classification"] != "Exact" for row in rows_list
        ),
        "exclusions": len(exclusions),
        "plan_items": len(plans),
        "authored_indices": EXPECTED_AUTHORED_INDICES,
        "dispatch_slots": EXPECTED_DISPATCH_SLOTS,
        "cargo_tests": sum(inventory.values()),
    }


def validate_file(
    matrix_path: Path,
    inventory: collections.Counter[str],
) -> dict[str, int | bool]:
    return validate_document(load_matrix(matrix_path), inventory)


def parse_args(arguments: list[str]) -> argparse.Namespace:
    workspace_root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(
        description="Validate the canonical Level 1 conformance matrix fail-closed."
    )
    parser.add_argument(
        "matrix",
        nargs="?",
        type=Path,
        default=workspace_root
        / "level1-conformance"
        / "level1-conformance-matrix.json",
        help="matrix JSON (default: canonical repository matrix)",
    )
    return parser.parse_args(arguments)


def main(arguments: list[str] | None = None) -> int:
    args = parse_args(arguments if arguments is not None else sys.argv[1:])
    workspace_root = Path(__file__).resolve().parent.parent
    matrix_path = args.matrix.resolve()
    try:
        inventory = capture_cargo_test_inventory(workspace_root)
        summary = validate_file(matrix_path, inventory)
    except ValidationError as error:
        print(
            json.dumps({"ok": False, "error": str(error)}, sort_keys=True),
            file=sys.stderr,
        )
        return 1
    print(json.dumps({"matrix": str(matrix_path), **summary}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
