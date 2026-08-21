#!/usr/bin/env python3
"""Fail-closed validator for the canonical Level 1 conformance matrix."""

from __future__ import annotations

import argparse
import ast
import collections
from dataclasses import dataclass
import hashlib
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
EXPECTED_LEVEL1_CARGO_TESTS = 481
EXPECTED_AUTHORED_INDICES = 308
EXPECTED_DISPATCH_SLOTS = 845
EXPECTED_MATRIX_ROWS = 182
FINAL_CONTRACT_SCHEMA = "mc2-plan29-closure-contract/v1"
FINAL_POLICY_SCHEMA = "mc2-plan29-final-qualification-policy/v1"
FINAL_RECEIPT_SCHEMA = "mc2-plan29-closure-receipts/v1"
FINAL_CACHE_SEAL_SCHEMA = "mc2-plan29-cache-conformance-identity/v1"
REQUIRED_RESIDENT_FAMILIES = {
    "archer": 0x01,
    "builder": 0x02,
    "villager": 0x04,
    "trader": 0x08,
}
EXPECTED_CHECK_IDS = {
    "unique-row-ids",
    "totals",
    "exact-evidence",
    "rust-test-resolution",
    "external-test-resolution",
    "non-exact-ownership",
    "plan-acyclic",
    "authored-census",
    "dispatch-census",
    "audio-call-ledger",
}
CLASSIFICATIONS = {"Exact", "Partial", "Missing", "Wrong", "Unproved"}
CONFIDENCE_LEVELS = {"high", "medium", "low"}
REACHABILITY_STATUSES = {"reachable", "conditional"}
SLOT_ID_RE = re.compile(r"^c\d{2}-(?:action|constructor)-\d{3}$")
OPEN_SLOT_ID_RE = re.compile(r"^DSP-L2-\d{3}$")
TEST_LINE_RE = re.compile(r"^(.+): test$")
CSHARP_NAMESPACE_RE = re.compile(r"\bnamespace\s+([A-Za-z_][A-Za-z0-9_.]*)")
CSHARP_CLASS_RE = re.compile(r"\bclass\s+([A-Za-z_][A-Za-z0-9_]*)")
CSHARP_TEST_RE = re.compile(
    r"\[Test\](?:\s*\[[^\]]+\])*\s*(?:public\s+)?(?:unsafe\s+)?void\s+"
    r"([A-Za-z_][A-Za-z0-9_]*)\s*\(",
    re.MULTILINE,
)
REPO_ROOT = Path(__file__).resolve().parents[4]
SIM_WORKSPACE_ROOT = Path(__file__).resolve().parent.parent
AUDIO_LEDGER_PATH = (
    SIM_WORKSPACE_ROOT / "level1-conformance" / "remc2-audio-call-ledger.json"
)
AUDIO_SOURCE_PATHS = (
    "remc2/engine/EventsFunctions.cpp",
    "remc2/engine/GameUI.cpp",
    "remc2/engine/Events.cpp",
)
AUDIO_TARGET = "PrepareEventSound_6E450"

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


@dataclass(frozen=True)
class MatrixValidationProfile:
    """Explicit denominator/evidence policy for a conformance matrix.

    The Level 1 wrapper below supplies the sealed historical policy.  Other
    matrices must opt into their own schema and denominators; ``None`` means
    that a denominator is recomputed from the document rather than silently
    borrowing Level 1's counts.
    """

    schema_id: str = SCHEMA_ID
    schema_version: int = SCHEMA_VERSION
    pinned_commit: str = PINNED_REMC2_COMMIT
    expected_rows: int | None = None
    expected_authored_indices: int | None = None
    expected_dispatch_slots: int | None = None
    expected_validator_checks: frozenset[str] | None = None
    audio_ledger_path: Path | None = None
    inventory_command: str | None = None


def level1_validation_profile() -> MatrixValidationProfile:
    """Return the sealed Level 1 policy used by legacy entry points."""

    return MatrixValidationProfile(
        expected_rows=EXPECTED_MATRIX_ROWS,
        expected_authored_indices=EXPECTED_AUTHORED_INDICES,
        expected_dispatch_slots=EXPECTED_DISPATCH_SLOTS,
        expected_validator_checks=frozenset(EXPECTED_CHECK_IDS),
        audio_ledger_path=AUDIO_LEDGER_PATH,
        inventory_command=(
            "cd research/private/remc2-sim-rs && "
            "cargo test --workspace --locked -- --list"
        ),
    )


def open_matrix_validation_profile(
    *,
    schema_id: str,
    schema_version: int,
    pinned_commit: str = PINNED_REMC2_COMMIT,
    expected_rows: int | None = None,
    expected_authored_indices: int | None = None,
    expected_dispatch_slots: int | None = None,
    inventory_command: str | None = None,
) -> MatrixValidationProfile:
    """Build an open profile without inheriting Level 1 closure policy.

    This is intentionally a small public constructor so a Level 2 matrix can
    declare its own schema and denominators while still using all structural
    checks (duplicate IDs, partitions, ownership, evidence resolution, and
    dependency acyclicity).
    """

    if not schema_id or schema_id == SCHEMA_ID:
        raise ValueError("an open profile must name a non-Level-1 schema")
    if schema_version < 1:
        raise ValueError("schema_version must be positive")
    return MatrixValidationProfile(
        schema_id=schema_id,
        schema_version=schema_version,
        pinned_commit=pinned_commit,
        expected_rows=expected_rows,
        expected_authored_indices=expected_authored_indices,
        expected_dispatch_slots=expected_dispatch_slots,
        expected_validator_checks=None,
        audio_ledger_path=None,
        inventory_command=inventory_command,
    )


def profile_from_json(path: Path) -> MatrixValidationProfile:
    """Load an explicit validator profile from a small JSON contract.

    The accepted shape is either the object itself or ``{"matrix_validation":
    {...}}``.  No Level 1 values are filled in implicitly.
    """

    value = load_matrix(path)
    value = value.get("matrix_validation", value)
    profile = expect_keys(
        value,
        "$.matrix_validation",
        {"schema_id", "schema_version", "pinned_commit"},
        {
            "expected_rows",
            "expected_authored_indices",
            "expected_dispatch_slots",
            "inventory_command",
        },
    )
    return open_matrix_validation_profile(
        schema_id=expect_string(profile["schema_id"], "profile.schema_id"),
        schema_version=expect_int(profile["schema_version"], "profile.schema_version", minimum=1),
        pinned_commit=expect_string(profile["pinned_commit"], "profile.pinned_commit"),
        expected_rows=(
            None
            if "expected_rows" not in profile or profile["expected_rows"] is None
            else expect_int(profile["expected_rows"], "profile.expected_rows", minimum=0)
        ),
        expected_authored_indices=(
            None
            if "expected_authored_indices" not in profile or profile["expected_authored_indices"] is None
            else expect_int(profile["expected_authored_indices"], "profile.expected_authored_indices", minimum=0)
        ),
        expected_dispatch_slots=(
            None
            if "expected_dispatch_slots" not in profile or profile["expected_dispatch_slots"] is None
            else expect_int(profile["expected_dispatch_slots"], "profile.expected_dispatch_slots", minimum=0)
        ),
        inventory_command=(
            None
            if "inventory_command" not in profile or profile["inventory_command"] is None
            else expect_string(profile["inventory_command"], "profile.inventory_command")
        ),
    )


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


def canonical_json_bytes(value: Any) -> bytes:
    """Canonical JSON used by every PLAN29 identity and receipt seal."""
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
        + "\n"
    ).encode("utf-8")


def sha256_hex(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def expect_sha256(value: Any, path: str) -> str:
    digest = expect_string(value, path)
    if re.fullmatch(r"[0-9a-f]{64}", digest) is None:
        fail(f"{path}: expected lowercase SHA-256")
    return digest


def matrix_row_id_digest(matrix: dict[str, Any]) -> str:
    rows = expect_array(matrix.get("rows"), "$.rows")
    row_ids = sorted(expect_string(row.get("id"), "$.rows[].id") for row in rows)
    if len(row_ids) != EXPECTED_MATRIX_ROWS or len(set(row_ids)) != len(row_ids):
        fail(f"closure identity: expected {EXPECTED_MATRIX_ROWS} unique row IDs")
    return sha256_hex(("\n".join(row_ids) + "\n").encode("utf-8"))


def matrix_source_span_digest(matrix: dict[str, Any]) -> str:
    descriptors = []
    for row in sorted(matrix["rows"], key=lambda item: item["id"]):
        source = expect_object(row.get("source"), f"row {row.get('id')}.source")
        if source.get("commit") != PINNED_REMC2_COMMIT:
            fail(f"row {row.get('id')}: source commit is not the pinned remc2 commit")
        descriptors.append(
            {
                "id": row["id"],
                "path": source.get("path"),
                "symbols": source.get("symbols"),
                "lines": source.get("lines"),
                "dispatch": source.get("dispatch"),
                "related": source.get("related", []),
            }
        )
    return sha256_hex(canonical_json_bytes(descriptors))


def matrix_ordered_behavior_digest(matrix: dict[str, Any]) -> str:
    behaviors = []
    for row in sorted(matrix["rows"], key=lambda item: item["id"]):
        behaviors.append(
            {
                "id": row["id"],
                "reachability": row.get("reachability"),
                "preconditions": row.get("preconditions"),
                "dispatch": row.get("source", {}).get("dispatch"),
                "behavior_ordered": row.get("behavior_ordered"),
                "negative_coverage_required": row.get("negative_coverage_required"),
                "implementation_layers": sorted(
                    key
                    for key, value in row.get("implementation", {}).items()
                    if key != "absent" and value
                ),
            }
        )
    return sha256_hex(canonical_json_bytes(behaviors))


def matrix_reachable_row_id_digest(matrix: dict[str, Any]) -> str:
    row_ids = sorted(
        row["id"]
        for row in matrix["rows"]
        if row["reachability"]["status"] == "reachable"
    )
    return sha256_hex(("\n".join(row_ids) + "\n").encode("utf-8"))


def matrix_identity(matrix: dict[str, Any], matrix_path: Path) -> dict[str, str]:
    castle_rows = [
        {
            "id": row["id"],
            "source": row["source"],
            "behavior_ordered": row["behavior_ordered"],
        }
        for row in sorted(matrix["rows"], key=lambda item: item["id"])
        if "castle" in canonical_json_bytes(row).decode("utf-8").lower()
    ]
    required_mask = {
        "families": REQUIRED_RESIDENT_FAMILIES,
        "required_mask": sum(REQUIRED_RESIDENT_FAMILIES.values()),
    }
    return {
        "matrix_sha256": sha256_hex(matrix_path.read_bytes()),
        "row_id_sha256": matrix_row_id_digest(matrix),
        "source_span_sha256": matrix_source_span_digest(matrix),
        "ordered_behavior_sha256": matrix_ordered_behavior_digest(matrix),
        "reachable_row_id_sha256": matrix_reachable_row_id_digest(matrix),
        "authored_census_sha256": sha256_hex(
            canonical_json_bytes(matrix["authored_census"])
        ),
        "dispatch_census_sha256": sha256_hex(
            canonical_json_bytes(matrix["dispatch_census"])
        ),
        "castle_profile_sha256": sha256_hex(canonical_json_bytes(castle_rows)),
        "required_observation_mask_sha256": sha256_hex(
            canonical_json_bytes(required_mask)
        ),
    }


def load_trusted_closure_contract(
    path: Path, expected_sha256: str
) -> dict[str, Any]:
    expected_sha256 = expect_sha256(expected_sha256, "expected contract SHA-256")
    raw = path.read_bytes()
    actual_file_sha256 = sha256_hex(raw)
    if actual_file_sha256 != expected_sha256:
        fail("closure contract file SHA-256 does not match reviewed expected hash")
    contract = load_matrix(path)
    if contract.get("schema") != FINAL_CONTRACT_SCHEMA:
        fail(f"closure contract: expected schema {FINAL_CONTRACT_SCHEMA}")
    specification = expect_object(contract.get("specification"), "contract.specification")
    if specification.get("remc2_commit") != PINNED_REMC2_COMMIT:
        fail("closure contract: wrong pinned remc2 commit")
    if specification.get("row_count") != EXPECTED_MATRIX_ROWS:
        fail(f"closure contract: expected {EXPECTED_MATRIX_ROWS} rows")
    if specification.get("dispatch_slot_count") != EXPECTED_DISPATCH_SLOTS:
        fail(f"closure contract: expected {EXPECTED_DISPATCH_SLOTS} dispatch slots")
    if specification.get("authored_index_count") != EXPECTED_AUTHORED_INDICES:
        fail(f"closure contract: expected {EXPECTED_AUTHORED_INDICES} authored indices")
    claimed = expect_sha256(contract.get("contract_sha256"), "contract.contract_sha256")
    unsigned = dict(contract)
    del unsigned["contract_sha256"]
    if sha256_hex(canonical_json_bytes(unsigned)) != claimed:
        fail("closure contract self-hash mismatch")
    if contract.get("blocked_until") != []:
        fail("closure contract retains blocked rows")
    return contract


def _validate_identity(
    actual: dict[str, str], expected: Any, path: str
) -> None:
    expected_object = expect_object(expected, path)
    if expected_object != actual:
        for key, value in actual.items():
            if expected_object.get(key) != value:
                fail(f"{path}.{key}: identity mismatch")
        fail(f"{path}: identity mismatch")


def load_executed_receipts(path: Path) -> dict[str, Any]:
    receipts = load_matrix(path)
    if receipts.get("schema") != FINAL_RECEIPT_SCHEMA:
        fail(f"closure receipts: expected schema {FINAL_RECEIPT_SCHEMA}")
    if receipts.get("remc2_commit") != PINNED_REMC2_COMMIT:
        fail("closure receipts: wrong source commit")
    return receipts


def validate_receipt_set(
    matrix: dict[str, Any], contract: dict[str, Any], receipts: dict[str, Any]
) -> dict[str, Any]:
    identity = expect_object(contract.get("trusted_identity"), "contract.trusted_identity")
    _validate_identity(identity, receipts.get("identity"), "receipts.identity")
    reachable = sorted(
        row["id"]
        for row in matrix["rows"]
        if row["reachability"]["status"] == "reachable"
    )
    execution_ids: set[str] = set()
    receipt_ids: list[str] = []
    layers = expect_object(receipts.get("layers"), "receipts.layers")
    for layer in ("rust", "c_abi", "unity", "cross_layer"):
        receipt = expect_object(layers.get(layer), f"receipts.layers.{layer}")
        if receipt.get("status") != "PASS" or receipt.get("exit_code") != 0:
            fail(f"receipts.layers.{layer}: requires executed PASS with exit code zero")
        execution_id = expect_string(
            receipt.get("execution_id"), f"receipts.layers.{layer}.execution_id"
        )
        if execution_id in execution_ids:
            fail("closure receipts: duplicate execution ID")
        execution_ids.add(execution_id)
        receipt_ids.append(execution_id)
        expect_sha256(receipt.get("command_sha256"), f"receipts.layers.{layer}.command_sha256")
        expect_sha256(receipt.get("output_sha256"), f"receipts.layers.{layer}.output_sha256")
        if receipt.get("source_commit") != PINNED_REMC2_COMMIT:
            fail(f"receipts.layers.{layer}: wrong source commit")
        if receipt.get("ordered_behavior_sha256") != identity.get(
            "ordered_behavior_sha256"
        ):
            fail(f"receipts.layers.{layer}: ordered behavior digest mismatch")
        if sorted(receipt.get("row_ids", [])) != reachable:
            fail(f"receipts.layers.{layer}: reachable row coverage mismatch")
    scenarios = expect_object(receipts.get("scenarios"), "receipts.scenarios")
    for scenario in ("victory", "failure", "restart", "replay", "privacy", "provenance"):
        item = expect_object(scenarios.get(scenario), f"receipts.scenarios.{scenario}")
        if item.get("status") != "PASS":
            fail(f"receipts.scenarios.{scenario}: required PASS receipt")
        expect_sha256(item.get("digest"), f"receipts.scenarios.{scenario}.digest")
    return {"receipt_ids": receipt_ids, "reachable_rows": len(reachable)}


def validate_required_observation_receipts(receipts: dict[str, Any]) -> int:
    observation = expect_object(
        receipts.get("required_observation"), "receipts.required_observation"
    )
    required = expect_int(observation.get("required_mask"), "required_mask")
    observed = expect_int(observation.get("observed_mask"), "observed_mask")
    remaining = expect_int(observation.get("remaining_mask"), "remaining_mask")
    expected = sum(REQUIRED_RESIDENT_FAMILIES.values())
    if required != expected or required == 0:
        fail("required observation mask must start from the nonzero pinned family mask")
    if observed != required or remaining != (required & ~observed) or remaining != 0:
        fail("required observation mask was not fully observed through production dispatch")
    families = expect_object(observation.get("family_receipts"), "family_receipts")
    if set(families) != set(REQUIRED_RESIDENT_FAMILIES):
        fail("required observation family receipts are incomplete")
    for family, bit in REQUIRED_RESIDENT_FAMILIES.items():
        receipt = expect_object(families[family], f"family_receipts.{family}")
        if receipt.get("bit") != bit or receipt.get("status") != "PASS":
            fail(f"family_receipts.{family}: invalid production observation")
        expect_sha256(receipt.get("dispatch_digest"), f"family_receipts.{family}.dispatch_digest")
    return remaining


def validate_cache_conformance_identity(
    cache_path: Path, contract_sha256: str, identity: dict[str, str]
) -> str:
    seal_path = cache_path / "conformance-identity.json"
    seal = load_matrix(seal_path)
    if seal.get("schema") != FINAL_CACHE_SEAL_SCHEMA:
        fail(f"cache seal: expected schema {FINAL_CACHE_SEAL_SCHEMA}")
    if seal.get("contract_file_sha256") != contract_sha256:
        fail("cache seal: contract hash mismatch")
    _validate_identity(identity, seal.get("identity"), "cache seal.identity")
    claimed = expect_sha256(seal.get("seal_sha256"), "cache seal.seal_sha256")
    unsigned = dict(seal)
    del unsigned["seal_sha256"]
    if sha256_hex(canonical_json_bytes(unsigned)) != claimed:
        fail("cache seal self-hash mismatch")
    return claimed


def validate_qualification_policy(
    policy_path: Path, contract_file_sha256: str, contract_self_sha256: str
) -> str:
    policy = load_matrix(policy_path)
    if policy.get("schema") != FINAL_POLICY_SCHEMA:
        fail(f"qualification policy: expected schema {FINAL_POLICY_SCHEMA}")
    behavior = expect_object(
        policy.get("behavior_contract"), "qualification_policy.behavior_contract"
    )
    if (
        behavior.get("file_sha256") != contract_file_sha256
        or behavior.get("self_sha256") != contract_self_sha256
    ):
        fail("qualification policy: behavior-contract binding mismatch")
    acceptance = expect_object(
        policy.get("acceptance"), "qualification_policy.acceptance"
    )
    if (
        acceptance.get("required_clean_runs") != 1
        or acceptance.get("required_run_labels") != ["run-a"]
        or acceptance.get("run_b_required") is not False
        or acceptance.get("run_b_permitted") is not False
        or policy.get("override_scope") != ["receipt_requirements.clean_run_count"]
    ):
        fail("qualification policy: exactly run A must be required and permitted")
    claimed = expect_sha256(
        policy.get("policy_sha256"), "qualification_policy.policy_sha256"
    )
    unsigned = dict(policy)
    del unsigned["policy_sha256"]
    if sha256_hex(canonical_json_bytes(unsigned)) != claimed:
        fail("qualification policy: self-hash mismatch")
    return sha256_hex(policy_path.read_bytes())


def validate_clean_run(
    receipts: dict[str, Any], identity: dict[str, str], policy_sha256: str
) -> list[str]:
    aggregate = expect_object(receipts.get("clean_runs"), "receipts.clean_runs")
    if aggregate.get("schema") != "mc2-plan29-full-gate/v1":
        fail("clean runs: wrong aggregate schema")
    runs = expect_array(aggregate.get("runs"), "clean_runs.runs")
    if aggregate.get("exact_run_count") != 1 or len(runs) != 1:
        fail("clean runs: exactly one run is required")
    if (
        aggregate.get("qualification_policy_sha256") != policy_sha256
        or receipts.get("qualification_policy_sha256") != policy_sha256
    ):
        fail("clean runs: qualification-policy binding mismatch")
    run_policy = expect_object(
        aggregate.get("run_count_policy"), "clean_runs.run_count_policy"
    )
    if (
        run_policy.get("required_runs") != 1
        or run_policy.get("completed_runs") != 1
        or run_policy.get("run_b_required") is not False
        or run_policy.get("run_b_permitted") is not False
    ):
        fail("clean runs: run-count policy is not 1/1")
    run_ids = [expect_string(run.get("run_id"), "clean_runs.runs[].run_id") for run in runs]
    if not run_ids[0].startswith("run-a"):
        fail("clean runs: the required run is not run A")
    for run in runs:
        expect_string(run.get("run_root"), "clean_runs.runs[].run_root")
        if run.get("status") != "PASS" or run.get("identity") != identity:
            fail("clean runs: each run must PASS with the trusted identity")
        claimed = expect_sha256(
            run.get("receipt_sha256"), "clean_runs.runs[].receipt_sha256"
        )
        unsigned = dict(run)
        del unsigned["receipt_sha256"]
        if sha256_hex(canonical_json_bytes(unsigned)) != claimed:
            fail("clean runs: run receipt self-hash mismatch")
        clean = expect_object(run.get("clean"), "clean_runs.runs[].clean")
        for key in (
            "cargo_target_isolated",
            "unity_library_isolated",
            "cache_isolated",
            "preexisting_artifacts_rejected",
        ):
            if clean.get(key) is not True:
                fail(f"clean runs: missing isolation proof {key}")
        gates = expect_object(run.get("gates"), "clean_runs.runs[].gates")
        required = {
            "rust", "c_abi", "sim_cli", "matrix_closed", "cache",
            "unity_edit_mode", "unity_play_mode", "victory", "failure",
            "restart", "replay", "privacy", "provenance", "presentation",
        }
        if set(gates) != required or any(value != "PASS" for value in gates.values()):
            fail("clean runs: required gate set is incomplete")
        semantics = expect_object(
            run.get("semantic_digests"), "clean_runs.runs[].semantic_digests"
        )
        for key in (
            "reset", "victory", "failure", "restart", "replay",
            "observation_mask", "unity_checkpoints", "full_run",
        ):
            expect_sha256(semantics.get(key), f"clean_runs.runs[].semantic_digests.{key}")
    if aggregate.get("status") != "PASS":
        fail("clean runs: aggregate status is not PASS")
    return run_ids


def level1_matrix_closed(
    matrix_path: Path,
    contract_path: Path,
    policy_path: Path,
    receipt_path: Path,
    cache_path: Path,
    expected_contract_sha256: str,
    inventory: collections.Counter[str] | None = None,
) -> dict[str, Any]:
    """Strict final predicate. Ordinary matrix validation never implies closure."""
    if inventory is None:
        inventory = capture_cargo_test_inventory(SIM_WORKSPACE_ROOT)
    matrix = load_matrix(matrix_path)
    ordinary = validate_document(matrix, inventory)
    ordinary.update(validate_audio_call_ledger(matrix))
    if ordinary["rows"] != EXPECTED_MATRIX_ROWS or ordinary["non_exact_rows"] != 0:
        fail("final closure requires exactly 182 Exact rows")
    identity = matrix_identity(matrix, matrix_path)
    contract = load_trusted_closure_contract(contract_path, expected_contract_sha256)
    contract_self_sha256 = expect_sha256(
        contract.get("contract_sha256"), "contract.contract_sha256"
    )
    policy_sha256 = validate_qualification_policy(
        policy_path, expected_contract_sha256, contract_self_sha256
    )
    _validate_identity(identity, contract.get("trusted_identity"), "contract.trusted_identity")
    expected_reachable = sorted(
        row["id"]
        for row in matrix["rows"]
        if row["reachability"]["status"] == "reachable"
    )
    if contract.get("reachable", {}).get("row_ids") != expected_reachable:
        fail("closure contract reachable row set mismatch")
    receipts = load_executed_receipts(receipt_path)
    receipt_summary = validate_receipt_set(matrix, contract, receipts)
    remaining_mask = validate_required_observation_receipts(receipts)
    cache_seal = validate_cache_conformance_identity(
        cache_path, expected_contract_sha256, identity
    )
    run_ids = validate_clean_run(receipts, identity, policy_sha256)
    return {
        **ordinary,
        "closed": True,
        "trusted_contract_sha256": expected_contract_sha256,
        "qualification_policy_sha256": policy_sha256,
        "identity": identity,
        "receipt_ids": receipt_summary["receipt_ids"],
        "remaining_observation_mask": remaining_mask,
        "cache_seal_sha256": cache_seal,
        "clean_run_ids": run_ids,
    }


def mask_cpp_noncode(text: str) -> str:
    """Preserve offsets/newlines while masking C++ comments and literals."""
    output = list(text)
    index = 0
    state = "code"
    while index < len(text):
        char = text[index]
        following = text[index + 1] if index + 1 < len(text) else ""
        if state == "code":
            if char == "/" and following == "/":
                output[index] = output[index + 1] = " "
                index += 2
                state = "line"
                continue
            if char == "/" and following == "*":
                output[index] = output[index + 1] = " "
                index += 2
                state = "block"
                continue
            if char == '"':
                output[index] = " "
                state = "string"
            elif char == "'":
                output[index] = " "
                state = "char"
        elif state == "line":
            if char == "\n":
                state = "code"
            else:
                output[index] = " "
        elif state == "block":
            if char == "*" and following == "/":
                output[index] = output[index + 1] = " "
                index += 2
                state = "code"
                continue
            if char != "\n":
                output[index] = " "
        else:
            quote = '"' if state == "string" else "'"
            if char == "\\":
                output[index] = " "
                if index + 1 < len(text):
                    if text[index + 1] != "\n":
                        output[index + 1] = " "
                    index += 2
                    continue
            if char == quote:
                state = "code"
            if char != "\n":
                output[index] = " "
        index += 1
    return "".join(output)


def matching_cpp_paren(masked: str, open_offset: int) -> int:
    depth = 0
    for index in range(open_offset, len(masked)):
        if masked[index] == "(":
            depth += 1
        elif masked[index] == ")":
            depth -= 1
            if depth == 0:
                return index
    fail(f"pinned remc2 source has an unclosed parenthesis at offset {open_offset}")


def cpp_function_starts(masked: str) -> list[tuple[int, str]]:
    """Find real top-level definitions, including functions lacking IDA banners."""
    starts: list[tuple[int, str]] = []
    candidate = re.compile(
        r"(?m)^(?:[A-Za-z_][\w:<>,]*[ \t*&]+)+"
        r"(?P<name>[A-Za-z_~]\w*(?:::\w+)*)[ \t]*\("
    )
    for match in candidate.finditer(masked):
        open_offset = masked.find("(", match.start(), match.end())
        close_offset = matching_cpp_paren(masked, open_offset)
        cursor = close_offset + 1
        while cursor < len(masked) and masked[cursor].isspace():
            cursor += 1
        if masked.startswith("const", cursor):
            cursor += len("const")
            while cursor < len(masked) and masked[cursor].isspace():
                cursor += 1
        if cursor < len(masked) and masked[cursor] == "{":
            starts.append((match.start(), match.group("name")))
    return starts


def enclosing_cpp_function(starts: list[tuple[int, str]], offset: int) -> str:
    enclosing = "<file-scope-or-unknown>"
    for start, symbol in starts:
        if start > offset:
            break
        enclosing = symbol
    return enclosing


def normalize_cpp_expression(expression: str) -> str:
    clean = mask_cpp_noncode(expression)
    clean = re.sub(r"\s+", " ", clean).strip()
    clean = re.sub(r"\s*([(),;\[\]])\s*", r"\1", clean)
    return re.sub(r"\s*(->|::)\s*", r"\1", clean)


def split_cpp_arguments(masked: str, raw: str) -> list[str]:
    cuts: list[int] = []
    paren = bracket = brace = 0
    for index, char in enumerate(masked):
        if char == "(":
            paren += 1
        elif char == ")":
            paren -= 1
        elif char == "[":
            bracket += 1
        elif char == "]":
            bracket -= 1
        elif char == "{":
            brace += 1
        elif char == "}":
            brace -= 1
        elif char == "," and paren == bracket == brace == 0:
            cuts.append(index)
    bounds = [-1, *cuts, len(raw)]
    return [raw[bounds[i] + 1 : bounds[i + 1]].strip() for i in range(len(bounds) - 1)]


def cpp_preprocessor_contexts(text: str) -> dict[int, list[dict[str, str]]]:
    result: dict[int, list[dict[str, str]]] = {}
    stack: list[dict[str, str]] = []
    for line_number, line in enumerate(text.splitlines(), 1):
        stripped = line.lstrip()
        if stripped.startswith(("#if ", "#ifdef ", "#ifndef ")):
            kind, _, condition = stripped[1:].partition(" ")
            stack.append(
                {
                    "kind": kind,
                    "condition": condition.split("//", 1)[0].strip(),
                    "branch": "if",
                }
            )
        elif stripped.startswith("#elif ") and stack:
            stack[-1] = {
                "kind": "elif",
                "condition": stripped[6:].split("//", 1)[0].strip(),
                "branch": "elif",
            }
        elif stripped.startswith("#else") and stack:
            stack[-1] = {**stack[-1], "branch": "else"}
        elif stripped.startswith("#endif") and stack:
            stack.pop()
        result[line_number] = [dict(item) for item in stack]
    return result


def pinned_audio_source(path: str) -> str:
    upstream = REPO_ROOT / "upstream" / "magic-carpet-2-hd"
    command = [
        "git",
        "-C",
        str(upstream),
        "show",
        f"{PINNED_REMC2_COMMIT}:{path}",
    ]
    try:
        completed = subprocess.run(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
    except OSError as error:
        fail(f"cannot read pinned remc2 audio source {path}: {error}")
    if completed.returncode != 0:
        fail(f"cannot read pinned remc2 audio source {path}: {completed.stderr.strip()}")
    return completed.stdout


def extract_pinned_audio_calls() -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for path in AUDIO_SOURCE_PATHS:
        text = pinned_audio_source(path)
        masked = mask_cpp_noncode(text)
        functions = cpp_function_starts(masked)
        preprocessor = cpp_preprocessor_contexts(text)
        file_ordinal = 0
        cursor = 0
        while True:
            start = masked.find(AUDIO_TARGET, cursor)
            if start < 0:
                break
            cursor = start + len(AUDIO_TARGET)
            open_offset = cursor
            while open_offset < len(masked) and masked[open_offset].isspace():
                open_offset += 1
            if open_offset >= len(masked) or masked[open_offset] != "(":
                continue
            close_offset = matching_cpp_paren(masked, open_offset)
            file_ordinal += 1
            line = text.count("\n", 0, start) + 1
            symbol = enclosing_cpp_function(functions, start)
            normalized = normalize_cpp_expression(text[start : close_offset + 1])
            raw_inside = text[open_offset + 1 : close_offset]
            masked_inside = masked[open_offset + 1 : close_offset]
            arguments = split_cpp_arguments(masked_inside, raw_inside)
            if len(arguments) != 3:
                fail(f"{path}:{line}: expected three PrepareEventSound arguments")
            records.append(
                {
                    "path": path,
                    "enclosing_symbol": symbol,
                    "file_ordinal_1based": file_ordinal,
                    "symbol_ordinal_1based": 0,
                    "pinned_line": line,
                    "normalized_call_expression": normalized,
                    "stable_occurrence_hash_sha256": "",
                    "preprocessor_context": preprocessor[line],
                    "entity_owner_expression": normalize_cpp_expression(arguments[0]),
                    "auxiliary_owner_expression": normalize_cpp_expression(arguments[1]),
                    "sound_expression": normalize_cpp_expression(arguments[2]),
                }
            )
    ordinals: collections.Counter[tuple[str, str]] = collections.Counter()
    for record in records:
        key = (record["path"], record["enclosing_symbol"])
        ordinals[key] += 1
        record["symbol_ordinal_1based"] = ordinals[key]
        identity = "\n".join(
            (
                record["path"],
                record["enclosing_symbol"],
                str(record["symbol_ordinal_1based"]),
                record["normalized_call_expression"],
            )
        )
        record["stable_occurrence_hash_sha256"] = hashlib.sha256(
            identity.encode()
        ).hexdigest()
    return records


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


def load_external_evidence_inventory(path: Path) -> collections.Counter[str]:
    """Load a reviewed evidence inventory without running a test suite.

    The file is either a JSON object mapping fully-qualified test names to
    integer occurrence counts, or an array of names (which must be unique).
    This keeps synthetic/open matrix validation deterministic while making the
    inventory an explicit input rather than an accidental Level 1 workspace
    dependency.
    """

    try:
        with path.open("r", encoding="utf-8") as handle:
            value = json.load(
                handle,
                object_pairs_hook=duplicate_rejecting_object,
                parse_constant=reject_json_constant,
            )
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        fail(f"{path}: cannot load evidence inventory: {error}")
    if isinstance(value, dict) and "tests" in value:
        value = value["tests"]
    if type(value) is dict:
        inventory: collections.Counter[str] = collections.Counter()
        for name, count in value.items():
            expect_string(name, f"{path}.tests key")
            inventory[name] = expect_int(count, f"{path}.tests.{name}", minimum=0)
    elif type(value) is list:
        names = [expect_string(item, f"{path}.tests[{index}]") for index, item in enumerate(value)]
        if len(names) != len(set(names)):
            fail(f"{path}: duplicate evidence inventory names are ambiguous")
        inventory = collections.Counter(names)
    else:
        fail(f"{path}: evidence inventory must be an object or array")
    if not inventory:
        fail(f"{path}: evidence inventory is empty")
    return inventory


def capture_unity_test_inventory(source_path: Path) -> collections.Counter[str]:
    try:
        source = source_path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        fail(f"cannot read Unity test source {source_path}: {error}")
    namespace_match = CSHARP_NAMESPACE_RE.search(source)
    class_match = CSHARP_CLASS_RE.search(source)
    if namespace_match is None or class_match is None:
        fail(f"Unity test source {source_path} has no namespace/class")
    prefix = f"{namespace_match.group(1)}.{class_match.group(1)}"
    inventory = collections.Counter(
        f"{prefix}.{method}" for method in CSHARP_TEST_RE.findall(source)
    )
    if not inventory:
        fail(f"Unity test source {source_path} contains no [Test] methods")
    duplicates = sorted(name for name, count in inventory.items() if count != 1)
    if duplicates:
        fail(f"Unity test source contains ambiguous duplicate {duplicates[0]!r}")
    return inventory


def capture_python_test_inventory(source_path: Path) -> collections.Counter[str]:
    try:
        tree = ast.parse(source_path.read_text(encoding="utf-8"), source_path)
    except (OSError, UnicodeError, SyntaxError) as error:
        fail(f"cannot parse Python test source {source_path}: {error}")
    inventory: collections.Counter[str] = collections.Counter()
    for node in tree.body:
        if not isinstance(node, ast.ClassDef):
            continue
        for member in node.body:
            if isinstance(member, (ast.FunctionDef, ast.AsyncFunctionDef)) and member.name.startswith(
                "test_"
            ):
                inventory[f"{source_path.stem}.{node.name}.{member.name}"] += 1
    if not inventory:
        fail(f"Python test source {source_path} contains no test methods")
    duplicates = sorted(name for name, count in inventory.items() if count != 1)
    if duplicates:
        fail(f"Python test source contains ambiguous duplicate {duplicates[0]!r}")
    return inventory


def resolve_binding_source(file_value: str, kind: str) -> Path:
    candidate = REPO_ROOT / file_value
    if candidate.is_symlink():
        fail(f"{kind} binding file cannot be a symlink: {file_value!r}")
    source = candidate.resolve()
    try:
        source.relative_to(REPO_ROOT)
    except ValueError:
        fail(f"{kind} binding file escapes the repository: {file_value!r}")
    if not source.is_file():
        fail(f"{kind} binding file is not a regular repository file: {file_value!r}")
    return source


def validate_schema(
    value: Any, profile: MatrixValidationProfile | None = None
) -> None:
    profile = profile or level1_validation_profile()
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
    if schema["id"] != profile.schema_id:
        fail(f"$.schema.id: expected {profile.schema_id!r}")
    if type(schema["version"]) is not int or schema["version"] != profile.schema_version:
        fail(f"$.schema.version: expected integer {profile.schema_version}")
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
        {
            "rust-test",
            "unity-test",
            "python-test",
            "exact_rows",
            "qualification-record",
        },
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
    if kind in {"rust-test", "unity-test", "python-test"}:
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
    elif kind == "unity-test":
        source = resolve_binding_source(binding["file"], kind)
        test_name = binding["test"]
        if capture_unity_test_inventory(source).get(test_name, 0) != 1:
            fail(
                f"row {row_id}: unity-test binding {test_name!r} "
                "does not resolve exactly once"
            )
    elif kind == "python-test":
        source = resolve_binding_source(binding["file"], kind)
        test_name = binding["test"]
        if capture_python_test_inventory(source).get(test_name, 0) != 1:
            fail(
                f"row {row_id}: python-test binding {test_name!r} "
                "does not resolve exactly once"
            )
    return kind


def validate_row(
    value: Any,
    index: int,
    inventory: collections.Counter[str],
    profile: MatrixValidationProfile | None = None,
) -> dict[str, Any]:
    profile = profile or level1_validation_profile()
    path = f"$.rows[{index}]"
    # Open Level 2 matrices carry a machine-readable denominator coverage map
    # alongside the stable Level 1 row fields.  Keep it optional so the sealed
    # Level 1 schema and identity remain byte-for-byte unchanged.
    row = expect_keys(value, path, ROW_KEYS, {"authored_indices", "coverage"})
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
    if commit != profile.pinned_commit:
        fail(f"{path}.source.commit: expected pinned remc2 commit {profile.pinned_commit}")
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
        if related_commit != profile.pinned_commit:
            fail(f"{related_path}.commit: expected pinned remc2 commit {profile.pinned_commit}")
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


def validate_exclusion(
    value: Any,
    index: int,
    profile: MatrixValidationProfile | None = None,
) -> dict[str, Any]:
    profile = profile or level1_validation_profile()
    path = f"$.exclusions[{index}]"
    exclusion = expect_keys(
        value,
        path,
        {"id", "scope", "description", "mechanical_proof", "source_anchors", "commit"},
        {"coverage", "reachability"},
    )
    for key in ("id", "scope", "description", "mechanical_proof"):
        expect_string(exclusion[key], f"{path}.{key}")
    anchors = string_array(
        exclusion["source_anchors"], f"{path}.source_anchors", unique=True
    )
    if not anchors:
        fail(f"{path}.source_anchors: expected at least one source anchor")
    if exclusion["commit"] != profile.pinned_commit:
        fail(f"{path}.commit: expected pinned remc2 commit {profile.pinned_commit}")
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
    profile: MatrixValidationProfile | None = None,
) -> None:
    profile = profile or level1_validation_profile()
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
    if (
        profile.expected_authored_indices is not None
        and len(nonzero) != profile.expected_authored_indices
    ):
        fail(
            f"{path}.nonzero_indices: expected exactly "
            f"{profile.expected_authored_indices} indices, got {len(nonzero)}"
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
    profile: MatrixValidationProfile | None = None,
) -> None:
    profile = profile or level1_validation_profile()
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
    if (
        profile.expected_dispatch_slots is not None
        and slot_count != profile.expected_dispatch_slots
    ):
        fail(
            f"{path}.slot_count: expected exactly "
            f"{profile.expected_dispatch_slots}, got {slot_count}"
        )
    slots = string_array(census["slots"], f"{path}.slots", unique=True)
    if len(slots) != slot_count:
        fail(f"{path}.slots: length does not equal slot_count")
    # Explicit open profiles may reuse the historical cXX slots in synthetic
    # tests or declare the Level 2 DSP-L2 slots.  The sealed Level 1 profile
    # remains strict and only accepts its original cXX form.
    slot_pattern = SLOT_ID_RE
    for index, slot_id in enumerate(slots):
        if not slot_pattern.fullmatch(slot_id) and not (
            profile.schema_id == "mc2-level2-conformance-matrix" and OPEN_SLOT_ID_RE.fullmatch(slot_id)
        ):
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
        if not slot_pattern.fullmatch(slot_id) and not (
            profile.schema_id == "mc2-level2-conformance-matrix" and OPEN_SLOT_ID_RE.fullmatch(slot_id)
        ):
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
        if not slot_pattern.fullmatch(slot_id) and not (
            profile.schema_id == "mc2-level2-conformance-matrix" and OPEN_SLOT_ID_RE.fullmatch(slot_id)
        ):
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
    profile: MatrixValidationProfile | None = None,
) -> None:
    profile = profile or level1_validation_profile()
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
        {"denominators"},
    )
    expect_string(meta["generated"], f"{path}.generated")
    if meta["remc2_commit"] != profile.pinned_commit:
        fail(f"{path}.remc2_commit: expected pinned remc2 commit {profile.pinned_commit}")
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
    expect_string(inventory_meta["command"], f"{path}.test_inventory.command")
    if (
        profile.inventory_command is not None
        and inventory_meta["command"] != profile.inventory_command
    ):
        fail(f"{path}.test_inventory.command: unexpected inventory command")
    actual_inventory_count = sum(inventory.values())
    declared_inventory_count = expect_int(
        inventory_meta["count"], f"{path}.test_inventory.count"
    )
    if profile.schema_id == SCHEMA_ID and declared_inventory_count != EXPECTED_LEVEL1_CARGO_TESTS:
        fail(
            f"{path}.test_inventory.count: sealed Level 1 historical inventory "
            f"must remain {EXPECTED_LEVEL1_CARGO_TESTS}"
        )
    if declared_inventory_count > actual_inventory_count:
        fail(
            f"{path}.test_inventory.count: expected at least "
            f"{declared_inventory_count} live Cargo tests, got {actual_inventory_count}"
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
        {"denominators"},
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


def validate_validator_contract(
    value: Any, profile: MatrixValidationProfile | None = None
) -> None:
    profile = profile or level1_validation_profile()
    contract = expect_keys(value, "$.validator_contract", {"checks"})
    checks = expect_array(contract["checks"], "$.validator_contract.checks")
    check_ids: list[str] = []
    for index, check_value in enumerate(checks):
        path = f"$.validator_contract.checks[{index}]"
        check = expect_keys(check_value, path, {"id", "rule"})
        check_ids.append(expect_string(check["id"], f"{path}.id"))
        expect_string(check["rule"], f"{path}.rule")
    if not check_ids:
        fail("$.validator_contract.checks: at least one declared check is required")
    if len(check_ids) != len(set(check_ids)):
        fail("$.validator_contract.checks: duplicate check IDs")
    expected_checks = profile.expected_validator_checks
    if expected_checks is not None and set(check_ids) != expected_checks:
        fail(
            "$.validator_contract.checks: IDs do not exactly match the "
            "declared validator contract"
        )


def validate_audio_call_ledger(
    matrix: dict[str, Any],
    profile: MatrixValidationProfile | None = None,
) -> dict[str, int]:
    profile = profile or level1_validation_profile()
    exclusions = {exclusion["id"]: exclusion for exclusion in matrix["exclusions"]}
    exclusion_ids = set(exclusions)
    row_ids = {row["id"] for row in matrix["rows"]}
    slot_to_row = matrix["dispatch_census"]["slot_to_row"]
    slot_to_exclusion = matrix["dispatch_census"]["slot_to_exclusion"]
    ledger_path = profile.audio_ledger_path or AUDIO_LEDGER_PATH
    ledger = load_matrix(ledger_path)
    expect_keys(
        ledger,
        "audio ledger",
        {
            "schema",
            "pinned_commit",
            "target",
            "ordinal_convention",
            "hash_convention",
            "counts",
            "records",
            "validation",
            "classification_scope_profile",
        },
    )
    for field in ("ordinal_convention", "hash_convention"):
        expect_string(ledger[field], f"audio ledger.{field}")
    if ledger.get("schema") != "plan26-remc2-prepare-event-sound-ledger-v1":
        fail("audio ledger: unexpected schema")
    if ledger.get("pinned_commit") != profile.pinned_commit:
        fail("audio ledger: pinned commit differs from the matrix")
    if ledger.get("target") != AUDIO_TARGET:
        fail("audio ledger: unexpected source-call target")
    scope_profile = expect_object(
        ledger.get("classification_scope_profile"),
        "audio ledger.classification_scope_profile",
    )
    expect_keys(
        scope_profile,
        "audio ledger.classification_scope_profile",
        {"mission", "players", "map", "speech", "objective_kind", "host", "status"},
    )
    for field in scope_profile:
        expect_string(scope_profile[field], f"audio ledger.classification_scope_profile.{field}")
    if scope_profile.get("status") != (
        "accepted after independent writer/reviewer reconciliation; "
        "extraction fields are mechanical"
    ):
        fail("audio ledger: classifications are not accepted closure evidence")
    if scope_profile.get("objective_kind") != "ordinary / non-secret":
        fail("audio ledger: selected profile does not close the objective-kind axis")
    if "selected normatively" not in str(scope_profile.get("speech", "")):
        fail("audio ledger: speech/CD profile must be selected normatively")
    if ledger_path.stat().st_mode & 0o777 != 0o644:
        fail("audio ledger: repository permissions must be 0644")

    records = expect_array(ledger.get("records"), "audio ledger.records")
    record_keys = {
        "path",
        "enclosing_symbol",
        "file_ordinal_1based",
        "symbol_ordinal_1based",
        "pinned_line",
        "normalized_call_expression",
        "stable_occurrence_hash_sha256",
        "preprocessor_context",
        "entity_owner_expression",
        "auxiliary_owner_expression",
        "sound_expression",
        "classification",
        "dispatch_bindings",
        "exclusion_registry_evidence",
        "canonical_linux_x64_preprocessor_state",
    }
    for index, record_value in enumerate(records):
        path = f"audio ledger.records[{index}]"
        record = expect_keys(record_value, path, record_keys)
        for field in (
            "path",
            "enclosing_symbol",
            "normalized_call_expression",
            "entity_owner_expression",
            "auxiliary_owner_expression",
            "sound_expression",
            "canonical_linux_x64_preprocessor_state",
        ):
            expect_string(record[field], f"{path}.{field}")
        for field in ("file_ordinal_1based", "symbol_ordinal_1based", "pinned_line"):
            expect_int(record[field], f"{path}.{field}", minimum=1)
        occurrence_hash = expect_string(
            record["stable_occurrence_hash_sha256"],
            f"{path}.stable_occurrence_hash_sha256",
        )
        if re.fullmatch(r"[0-9a-f]{64}", occurrence_hash) is None:
            fail(f"{path}.stable_occurrence_hash_sha256: expected lowercase SHA-256")
        for context_index, context_value in enumerate(
            expect_array(record["preprocessor_context"], f"{path}.preprocessor_context")
        ):
            context_path = f"{path}.preprocessor_context[{context_index}]"
            context = expect_keys(context_value, context_path, {"kind", "condition", "branch"})
            for field in context:
                expect_string(context[field], f"{context_path}.{field}")
        classification = expect_keys(
            record["classification"],
            f"{path}.classification",
            {
                "disposition",
                "proposed_distinct_production_tag",
                "current_rust_tag",
                "exclusion_id",
                "producer_or_profile_proof",
            },
            {"mapping_warning"},
        )
        expect_string(
            classification["producer_or_profile_proof"],
            f"{path}.classification.producer_or_profile_proof",
        )
        disposition = expect_string(
            classification["disposition"], f"{path}.classification.disposition"
        )
        if disposition not in {"reachable", "excluded"}:
            fail(f"{path}.classification.disposition: expected reachable or excluded")
        for field in ("proposed_distinct_production_tag", "exclusion_id"):
            if classification[field] is not None:
                expect_string(classification[field], f"{path}.classification.{field}")
        current_tag = classification["current_rust_tag"]
        if current_tag is not None:
            if isinstance(current_tag, str):
                expect_string(current_tag, f"{path}.classification.current_rust_tag")
            else:
                string_array(
                    current_tag,
                    f"{path}.classification.current_rust_tag",
                    unique=True,
                )
                if not current_tag:
                    fail(f"{path}.classification.current_rust_tag: array cannot be empty")
        if "mapping_warning" in classification:
            expect_string(
                classification["mapping_warning"],
                f"{path}.classification.mapping_warning",
            )
        for binding_index, binding_value in enumerate(
            expect_array(record["dispatch_bindings"], f"{path}.dispatch_bindings")
        ):
            binding_path = f"{path}.dispatch_bindings[{binding_index}]"
            binding = expect_keys(
                binding_value,
                binding_path,
                {"slot", "rows", "exclusion", "semantic_label"},
            )
            expect_string(binding["slot"], f"{binding_path}.slot")
            binding_rows = string_array(
                binding["rows"], f"{binding_path}.rows", unique=True
            )
            if any(row_id not in row_ids for row_id in binding_rows):
                fail(f"{binding_path}.rows: unknown matrix row")
            if binding["exclusion"] is not None:
                expect_string(binding["exclusion"], f"{binding_path}.exclusion")
                if binding["exclusion"] not in exclusion_ids:
                    fail(f"{binding_path}.exclusion: unknown exclusion")
            expect_string(binding["semantic_label"], f"{binding_path}.semantic_label")
            slot = binding["slot"]
            if slot not in slot_to_row and slot not in slot_to_exclusion:
                fail(f"{binding_path}.slot: unknown dispatch-census slot")
            if binding_rows != slot_to_row.get(slot, []):
                fail(f"{binding_path}.rows: disagrees with dispatch census")
            if binding["exclusion"] != slot_to_exclusion.get(slot):
                fail(f"{binding_path}.exclusion: disagrees with dispatch census")
        evidence = record["exclusion_registry_evidence"]
        if evidence is not None:
            evidence_path = f"{path}.exclusion_registry_evidence"
            evidence = expect_keys(
                evidence,
                evidence_path,
                {"id", "scope", "description", "mechanical_proof", "source_anchors", "commit"},
            )
            for field in ("id", "scope", "description", "mechanical_proof", "commit"):
                expect_string(evidence[field], f"{evidence_path}.{field}")
            string_array(evidence["source_anchors"], f"{evidence_path}.source_anchors")
            if evidence["commit"] != PINNED_REMC2_COMMIT:
                fail(f"{evidence_path}.commit: expected pinned remc2 commit")
            if evidence["id"] != classification["exclusion_id"]:
                fail(f"{evidence_path}.id: does not match the record exclusion")
            if evidence != exclusions.get(evidence["id"]):
                fail(f"{evidence_path}: does not exactly match the matrix registry")

    validation = expect_keys(
        ledger["validation"],
        "audio ledger.validation",
        {
            "expected_total",
            "expected_by_path",
            "fresh_extraction_set_equality_basis",
            "all_classified",
            "unclassified_keys",
        },
    )
    if validation["expected_total"] != 119:
        fail("audio ledger.validation.expected_total: expected 119")
    if validation["expected_by_path"] != {
        "remc2/engine/EventsFunctions.cpp": 117,
        "remc2/engine/GameUI.cpp": 1,
        "remc2/engine/Events.cpp": 1,
    }:
        fail("audio ledger.validation.expected_by_path: expected 117+1+1")
    expect_string(
        validation["fresh_extraction_set_equality_basis"],
        "audio ledger.validation.fresh_extraction_set_equality_basis",
    )
    if validation["all_classified"] is not True or validation["unclassified_keys"] != []:
        fail("audio ledger.validation: accepted ledger must have zero unclassified calls")
    fresh = extract_pinned_audio_calls()
    mechanical_fields = tuple(fresh[0])
    if len(records) != len(fresh):
        fail(f"audio ledger: expected {len(fresh)} source calls, got {len(records)}")
    for index, (record, expected) in enumerate(zip(records, fresh, strict=True)):
        actual_mechanical = {field: record.get(field) for field in mechanical_fields}
        if actual_mechanical != expected:
            fail(
                "audio ledger: fresh pinned extraction disagrees at record "
                f"{index + 1} ({expected['path']}:{expected['pinned_line']})"
            )

    expected_by_path = collections.Counter(record["path"] for record in fresh)
    if expected_by_path != collections.Counter(
        {
            "remc2/engine/EventsFunctions.cpp": 117,
            "remc2/engine/GameUI.cpp": 1,
            "remc2/engine/Events.cpp": 1,
        }
    ):
        fail("audio ledger: pinned denominator changed from 117+1+1")
    dispositions = collections.Counter(
        expect_object(record.get("classification"), f"audio ledger.records[{index}]")
        .get("disposition")
        for index, record in enumerate(records)
    )
    if dispositions != {"reachable": 46, "excluded": 73}:
        fail(
            "audio ledger: accepted classification partition must be "
            "46 reachable / 73 excluded"
        )
    reachable_tags: list[str] = []
    for index, record in enumerate(records):
        classification = expect_object(
            record["classification"], f"audio ledger.records[{index}].classification"
        )
        if classification["disposition"] == "reachable":
            if record["exclusion_registry_evidence"] is not None:
                fail(f"audio ledger: reachable record {index + 1} claims exclusion evidence")
            tag = classification.get("proposed_distinct_production_tag")
            if not isinstance(tag, str) or not tag.startswith("SourceAudioCallTag::"):
                fail(f"audio ledger: reachable record {index + 1} has no source-call tag")
            if classification.get("exclusion_id") is not None:
                fail(f"audio ledger: reachable record {index + 1} names an exclusion")
            reachable_tags.append(tag.removeprefix("SourceAudioCallTag::"))
        else:
            if record["exclusion_registry_evidence"] is None:
                fail(f"audio ledger: excluded record {index + 1} lacks registry evidence")
            exclusion_id = classification.get("exclusion_id")
            if exclusion_id not in exclusion_ids:
                fail(
                    f"audio ledger: excluded record {index + 1} has unknown "
                    f"exclusion {exclusion_id!r}"
                )
            if classification.get("proposed_distinct_production_tag") is not None:
                fail(f"audio ledger: excluded record {index + 1} retains a source tag")
        expected_platform_state = (
            "active (no local conditional directive)"
            if not record["preprocessor_context"]
            else "inactive (#else of defined(__linux__) || defined(__APPLE__))"
        )
        if record["canonical_linux_x64_preprocessor_state"] != expected_platform_state:
            fail(
                f"audio ledger: record {index + 1} platform state disagrees with "
                "its pinned preprocessor context"
            )
    if len(reachable_tags) != len(set(reachable_tags)):
        fail("audio ledger: reachable source-call tags are not one-to-one")

    core_root = SIM_WORKSPACE_ROOT / "crates" / "sim-core" / "src"
    lib_source = (core_root / "lib.rs").read_text(encoding="utf-8")
    enum_match = re.search(
        r"pub enum SourceAudioCallTag\s*\{(?P<body>.*?)\n\}",
        lib_source,
        re.DOTALL,
    )
    if enum_match is None:
        fail("audio ledger: SourceAudioCallTag enum is missing")
    enum_tags = [
        line.strip().removesuffix(",")
        for line in enum_match.group("body").splitlines()
        if line.strip()
    ]
    if set(reachable_tags) != set(enum_tags) or len(enum_tags) != 46:
        fail("audio ledger: ledger tags and SourceAudioCallTag enum are not bijective")
    binding_match = re.search(
        r"pub const SOURCE_AUDIO_CALL_BINDINGS:.*?=\s*\[(?P<body>.*?)\n\];",
        lib_source,
        re.DOTALL,
    )
    if binding_match is None:
        fail("audio ledger: compiled source occurrence binding table is missing")
    compiled_bindings = dict(
        re.findall(
            r"SourceAudioCallTag::(\w+)\s*,\s*\"([0-9a-f]{64})\"",
            binding_match.group("body"),
        )
    )
    ledger_bindings = {
        record["classification"]["proposed_distinct_production_tag"].removeprefix(
            "SourceAudioCallTag::"
        ): record["stable_occurrence_hash_sha256"]
        for record in records
        if record["classification"]["disposition"] == "reachable"
    }
    if compiled_bindings != ledger_bindings or len(compiled_bindings) != 46:
        fail("audio ledger: compiled tag-to-occurrence ownership map disagrees")
    all_impl = re.compile(
        r"#\[cfg\(test\)\]\s*impl SourceAudioCallTag\s*\{.*?\n\}", re.DOTALL
    )
    production_lib = all_impl.sub("", lib_source)
    production_lib = production_lib.replace(binding_match.group(0), "")
    production_sources = [production_lib]
    for filename in ("goat.rs", "level1_mission.rs", "spell_activation.rs"):
        source = (core_root / filename).read_text(encoding="utf-8")
        test_module = re.search(
            r"(?m)^#\[cfg\(test\)\]\s*\nmod tests\s*\{",
            source,
        )
        production_sources.append(
            source if test_module is None else source[: test_module.start()]
        )
    production_text = "\n".join(production_sources)
    for tag in enum_tags:
        count = len(re.findall(rf"\bSourceAudioCallTag::{re.escape(tag)}\b", production_text))
        if count != 1:
            fail(
                f"audio ledger: source-call tag {tag!r} has {count} production bindings, "
                "expected exactly one"
            )
    counts = expect_object(ledger.get("counts"), "audio ledger.counts")
    expected_counts = {
        "total": 119,
        "by_path": dict(expected_by_path),
        "unique_occurrence_hashes": 119,
        "unique_keys": 119,
        "reachable": 46,
        "excluded": 73,
        "unclassified": 0,
        "classification_ids_unique_per_reachable_occurrence": 46,
    }
    if counts != expected_counts:
        fail("audio ledger: stored counts do not equal the accepted recomputed counts")
    return {"audio_calls": 119, "audio_reachable_calls": 46, "audio_excluded_calls": 73}


def validate_document(
    document: Any,
    inventory: collections.Counter[str],
    profile: MatrixValidationProfile | None = None,
) -> dict[str, int | bool]:
    profile = profile or level1_validation_profile()
    root = expect_keys(document, "$", ROOT_KEYS)
    validate_schema(root["schema"], profile)

    row_values = expect_array(root["rows"], "$.rows")
    rows_list = [
        validate_row(row, index, inventory, profile) for index, row in enumerate(row_values)
    ]
    rows = unique_id_map(rows_list, "$.rows")
    if profile.expected_rows is not None and len(rows) != profile.expected_rows:
        fail(
            f"$.rows: expected exactly {profile.expected_rows} rows, got {len(rows)}"
        )

    exclusion_values = expect_array(root["exclusions"], "$.exclusions")
    exclusions_list = [
        validate_exclusion(exclusion, index, profile)
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
    validate_authored_census(root["authored_census"], rows, profile)
    validate_dispatch_census(root["dispatch_census"], rows, exclusions, profile)
    validate_meta(
        root["meta"],
        rows_list,
        len(exclusions),
        len(plans),
        inventory,
        profile,
    )
    validate_validator_contract(root["validator_contract"], profile)
    return {
        "ok": True,
        "rows": len(rows),
        "exact_rows": sum(row["classification"] == "Exact" for row in rows_list),
        "non_exact_rows": sum(
            row["classification"] != "Exact" for row in rows_list
        ),
        "exclusions": len(exclusions),
        "plan_items": len(plans),
        "authored_indices": len(root["authored_census"]["nonzero_indices"]),
        "dispatch_slots": len(root["dispatch_census"]["slots"]),
        "cargo_tests": (
            root["meta"]["test_inventory"]["count"]
            if profile.schema_id == SCHEMA_ID
            else sum(inventory.values())
        ),
    }


def validate_file(
    matrix_path: Path,
    inventory: collections.Counter[str],
    profile: MatrixValidationProfile | None = None,
) -> dict[str, int | bool]:
    profile = profile or level1_validation_profile()
    document = load_matrix(matrix_path)
    summary = validate_document(document, inventory, profile)
    if profile.audio_ledger_path is not None:
        summary.update(validate_audio_call_ledger(document, profile))
    return summary


def validate_matrix(
    matrix_path: Path,
    inventory: collections.Counter[str],
    profile: MatrixValidationProfile,
) -> dict[str, int | bool]:
    """Explicit generic entry point; unlike the legacy name, never implies closure."""

    return validate_file(matrix_path, inventory, profile)


def require_closed(
    matrix_path: Path,
    inventory: collections.Counter[str] | None = None,
    profile: MatrixValidationProfile | None = None,
) -> dict[str, Any]:
    """Require the canonical matrix itself to be closed, without implying final qualification."""
    profile = profile or level1_validation_profile()
    if inventory is None:
        inventory = capture_cargo_test_inventory(SIM_WORKSPACE_ROOT)
    document = load_matrix(matrix_path)
    summary = validate_file(matrix_path, inventory, profile)
    if profile.expected_rows is not None and summary["rows"] != profile.expected_rows:
        fail(f"closed matrix requires exactly {profile.expected_rows} rows")
    if summary["non_exact_rows"] != 0 or summary["exact_rows"] != summary["rows"]:
        if profile.expected_rows == EXPECTED_MATRIX_ROWS:
            fail("closed matrix requires every one of the 182 rows to be Exact")
        fail("closed matrix requires every row to be Exact")
    if document["meta"]["remc2_commit"] != profile.pinned_commit:
        fail("closed matrix uses the wrong pinned remc2 commit")
    return {**summary, "closed": True}


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
    parser.add_argument(
        "--require-closed",
        action="store_true",
        help="require every validated row to be Exact (Level 1 additionally requires its sealed denominator)",
    )
    parser.add_argument(
        "--profile",
        type=Path,
        help="explicit JSON matrix-validation profile; without it use sealed Level 1 policy",
    )
    parser.add_argument(
        "--evidence-inventory",
        type=Path,
        help="reviewed JSON inventory of external executable evidence names",
    )
    parser.add_argument("--final-closure", action="store_true")
    parser.add_argument("--closure-contract", type=Path)
    parser.add_argument("--qualification-policy", type=Path)
    parser.add_argument("--receipts", type=Path)
    parser.add_argument("--cache", type=Path)
    parser.add_argument("--expected-contract-sha256")
    parser.add_argument("--json", action="store_true")
    return parser.parse_args(arguments)


def main(arguments: list[str] | None = None) -> int:
    args = parse_args(arguments if arguments is not None else sys.argv[1:])
    workspace_root = Path(__file__).resolve().parent.parent
    matrix_path = args.matrix.resolve()
    try:
        profile = (
            profile_from_json(args.profile.resolve())
            if args.profile is not None
            else level1_validation_profile()
        )
        inventory = (
            load_external_evidence_inventory(args.evidence_inventory.resolve())
            if args.evidence_inventory is not None
            else capture_cargo_test_inventory(workspace_root)
        )
        if args.final_closure:
            missing = [
                name
                for name, value in (
                    ("--closure-contract", args.closure_contract),
                    ("--qualification-policy", args.qualification_policy),
                    ("--receipts", args.receipts),
                    ("--cache", args.cache),
                    ("--expected-contract-sha256", args.expected_contract_sha256),
                )
                if value is None
            ]
            if missing:
                fail("--final-closure requires " + ", ".join(missing))
            summary = level1_matrix_closed(
                matrix_path,
                args.closure_contract.resolve(),
                args.qualification_policy.resolve(),
                args.receipts.resolve(),
                args.cache.resolve(),
                args.expected_contract_sha256,
                inventory,
            )
        elif args.require_closed:
            summary = require_closed(matrix_path, inventory, profile)
        else:
            summary = validate_file(matrix_path, inventory, profile)
    except ValidationError as error:
        print(json.dumps({"ok": False, "closed": False, "error": str(error)}, sort_keys=True))
        return 1
    print(json.dumps({"matrix": str(matrix_path), **summary}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
