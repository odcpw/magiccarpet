#!/usr/bin/env python3
"""Fail-closed validator for the independent Level 2 conformance matrix."""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
from typing import Any

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
MATRIX = ROOT / "level2-conformance" / "level2-conformance-matrix.json"
COMMIT = "6806c7204d17220ecc0430759f784aea8ee2ddb2"
FAMILIES = (
    "authored", "reset", "activation_groups", "producer_edges", "objectives",
    "dispatch", "tuples", "ai_helpers", "rng", "audio", "cache",
    "presentation_refs",
)

# These dispatch rows are mechanically classified as unreachable by the
# sealed dispatch scan, but the independent activation closure proves their
# class/model routes reachable. L2-08A explicitly moved this bounded set into
# worker ownership, so the validator must prevent them drifting back into an
# exclusion.
ACTIVATION_REACHABLE_DISPATCH_IDS = frozenset({
    "DSP-L2-089", "DSP-L2-090", "DSP-L2-091", "DSP-L2-092",
    "DSP-L2-093", "DSP-L2-094", "DSP-L2-095", "DSP-L2-096",
    "DSP-L2-098", "DSP-L2-145", "DSP-L2-185", "DSP-L2-186",
    "DSP-L2-187", "DSP-L2-188", "DSP-L2-189", "DSP-L2-190",
    "DSP-L2-191", "DSP-L2-192", "DSP-L2-289", "DSP-L2-290",
    "DSP-L2-291", "DSP-L2-292", "DSP-L2-293", "DSP-L2-312",
    "DSP-L2-313", "DSP-L2-318", "DSP-L2-323",
})
MATRIX_CONTROL_BEAD_IDS = {"mc-a4ir", "mc-a4ir.1"}


class Level2MatrixError(ValueError):
    pass


def load(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise Level2MatrixError(f"{path}: cannot load JSON: {error}") from error


def expected_denominators() -> dict[str, set[str]]:
    census = load(ROOT / "level2-conformance" / "census" / "level2-authored-reset-census.json")
    activation = load(ROOT / "level2-conformance" / "activation" / "level2-activation-closure.json")
    ledger = load(ROOT / "level2-conformance" / "denominator" / "level2-dispatch-rng-audio-presentation.json")
    for name, artifact in (("census", census), ("activation", activation), ("denominator", ledger)):
        if artifact.get("spec", {}).get("remc2_commit") != COMMIT:
            raise Level2MatrixError(f"{name} artifact is not derived from the pinned remc2 commit")
    return {
        "authored": {row["id"] for row in census["authored"]["rows"]},
        "reset": {row["id"] for row in census["reset"]["slots"]},
        "activation_groups": {f"GROUP-L2-{index:02d}" for index in range(len(activation["groups"]))},
        "producer_edges": {row["id"] for row in activation["producer_edges"]},
        "objectives": {row["id"] for row in activation["objectives"]},
        "dispatch": {row["id"] for row in ledger["dispatch"]["rows"]},
        "tuples": {row["id"] for row in ledger["tuple_denominator"]["rows"]},
        "ai_helpers": {row["id"] for row in ledger["ai_helper_ledger"]["rows"]},
        "rng": {row["id"] for row in ledger["rng"]["rows"]},
        "audio": {row["id"] for row in ledger["audio"]["rows"]},
        "cache": {row["id"] for row in ledger["presentation"]["cache_rows"]},
        "presentation_refs": {row["id"] for row in ledger["presentation"]["source_references"]},
    }


def open_bead_ids() -> set[str]:
    path = ROOT.parents[2] / ".beads" / "issues.jsonl"
    result: set[str] = set()
    for line in path.read_text(encoding="utf-8").splitlines():
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if not isinstance(value, dict) or value.get("status") != "open":
            continue
        if "level-2" in (value.get("labels") or []) and value.get("id") not in MATRIX_CONTROL_BEAD_IDS:
            result.add(value["id"])
    return result


def validate_coverage(document: dict[str, Any]) -> dict[str, int]:
    expected = expected_denominators()
    rows = document["rows"]
    exclusions = document["exclusions"]
    row_ids = {row["id"] for row in rows}
    exclusion_ids = {item["id"] for item in exclusions}
    if len(row_ids) != len(rows):
        raise Level2MatrixError("duplicate Level 2 row ID")
    if len(exclusion_ids) != len(exclusions):
        raise Level2MatrixError("duplicate Level 2 exclusion ID")
    seen: dict[str, dict[str, str]] = {family: {} for family in FAMILIES}
    for owner_kind, entries in (("row", rows), ("exclusion", exclusions)):
        for entry in entries:
            coverage = entry.get("coverage")
            if not isinstance(coverage, dict):
                raise Level2MatrixError(f"{owner_kind} {entry['id']}: coverage map is missing")
            if set(coverage) != set(FAMILIES):
                raise Level2MatrixError(f"{owner_kind} {entry['id']}: coverage families do not match registry")
            for family in FAMILIES:
                ids = coverage[family]
                if not isinstance(ids, list) or len(ids) != len(set(ids)):
                    raise Level2MatrixError(f"{owner_kind} {entry['id']}.{family}: IDs must be unique array")
                for identifier in ids:
                    if not isinstance(identifier, str) or identifier not in expected[family]:
                        raise Level2MatrixError(f"{owner_kind} {entry['id']}.{family}: unknown denominator ID {identifier!r}")
                    if identifier in seen[family]:
                        prior = seen[family][identifier]
                        raise Level2MatrixError(f"{family} denominator {identifier} is multiply owned by {prior} and {entry['id']}")
                    seen[family][identifier] = f"{owner_kind}:{entry['id']}"
    for family in FAMILIES:
        missing = sorted(expected[family] - set(seen[family]))
        if missing:
            raise Level2MatrixError(f"{family}: uncovered denominator IDs (first {missing[0]})")
    for identifier in sorted(ACTIVATION_REACHABLE_DISPATCH_IDS):
        if identifier not in expected["dispatch"]:
            raise Level2MatrixError(f"activation-reachable dispatch ID {identifier} is absent from the sealed ledger")
        owner = seen["dispatch"][identifier]
        if not owner.startswith("row:"):
            raise Level2MatrixError(f"activation-reachable dispatch ID {identifier} is assigned to {owner}")
    declared = document["meta"].get("denominators")
    if declared != {family: len(expected[family]) for family in FAMILIES}:
        raise Level2MatrixError("meta.denominators does not match the sealed artifacts")
    return {family: len(expected[family]) for family in FAMILIES}


def validate_source_and_ownership(document: dict[str, Any]) -> None:
    if document.get("schema", {}).get("id") != "mc2-level2-conformance-matrix":
        raise Level2MatrixError("wrong Level 2 matrix schema")
    if document["meta"].get("remc2_commit") != COMMIT:
        raise Level2MatrixError("matrix meta uses the wrong pinned commit")
    plans = {item["id"]: item for item in document["plan_items"]}
    expected_beads = open_bead_ids()
    if set(plans) != expected_beads:
        raise Level2MatrixError(f"plan/bead mismatch: missing={sorted(expected_beads-set(plans))}, extra={sorted(set(plans)-expected_beads)}")
    owned: dict[str, str] = {}
    for row in document["rows"]:
        if row["classification"] == "Exact":
            raise Level2MatrixError("Level 2 matrix must not claim Exact before behavior closure")
        plan = row.get("plan_item")
        if plan not in plans:
            raise Level2MatrixError(f"row {row['id']} has dangling owner {plan!r}")
        if row["id"] in owned:
            raise Level2MatrixError(f"row {row['id']} has duplicate owner")
        owned[row["id"]] = plan
        source = row.get("source", {})
        if source.get("commit") != COMMIT or not source.get("path") or not source.get("lines") or not source.get("symbols"):
            raise Level2MatrixError(f"row {row['id']} has incomplete pinned source anchor")
        if not row.get("reachability", {}).get("chain"):
            raise Level2MatrixError(f"row {row['id']} has no reachability chain")
        if not row.get("evidence_bindings"):
            raise Level2MatrixError(f"row {row['id']} has no executable evidence binding")
        try:
            pinned = subprocess.run(
                ["git", "-C", str(ROOT.parents[2] / "upstream" / "magic-carpet-2-hd"), "show", f"{COMMIT}:{source['path']}"],
                check=True, capture_output=True, text=True,
            ).stdout
        except (OSError, subprocess.CalledProcessError) as error:
            raise Level2MatrixError(f"row {row['id']} source path is not readable at pinned commit") from error
        for symbol in source["symbols"]:
            if symbol not in pinned:
                raise Level2MatrixError(f"row {row['id']} symbol {symbol!r} is absent from pinned source")
    listed: dict[str, str] = {}
    for plan_id, plan in plans.items():
        for row_id in plan.get("owned_rows", []):
            if row_id in listed:
                raise Level2MatrixError(f"row {row_id} multiply owned by {listed[row_id]} and {plan_id}")
            listed[row_id] = plan_id
    if listed != owned:
        raise Level2MatrixError(f"plan ownership is not a bijection: missing={sorted(set(owned)-set(listed))}, extra={sorted(set(listed)-set(owned))}")
    for item in document["exclusions"]:
        if item.get("commit") != COMMIT or not item.get("source_anchors"):
            raise Level2MatrixError(f"exclusion {item['id']} lacks pinned source proof")
        if not item.get("reachability", {}).get("chain"):
            raise Level2MatrixError(f"exclusion {item['id']} lacks a reachability chain")


def validate(document_path: Path = MATRIX) -> dict[str, Any]:
    document = load(document_path)
    validate_source_and_ownership(document)
    denominator_counts = validate_coverage(document)
    # Reuse the Level 1 structural validator with an explicit open profile. It
    # checks row shape, Python evidence resolution, authored/dispatch maps, and
    # the acyclic plan graph without borrowing Level 1 closure totals.
    sys.path.insert(0, str(HERE))
    import validate_level1_matrix as generic  # type: ignore

    profile = generic.open_matrix_validation_profile(
        schema_id="mc2-level2-conformance-matrix",
        schema_version=1,
        pinned_commit=COMMIT,
        expected_authored_indices=len(document["authored_census"]["nonzero_indices"]),
        expected_dispatch_slots=845,
        inventory_command="cd research/private/remc2-sim-rs && cargo test --workspace --locked -- --list",
    )
    inventory = generic.capture_cargo_test_inventory(ROOT)
    summary = generic.validate_file(document_path, inventory, profile)
    if summary["rows"] != len(document["rows"]):
        raise Level2MatrixError("generic validator row count mismatch")
    return {**summary, "denominators": denominator_counts, "closed": False}


def main() -> int:
    try:
        summary = validate(Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else MATRIX)
    except (Level2MatrixError, ValueError, OSError, subprocess.SubprocessError) as error:
        print(json.dumps({"ok": False, "closed": False, "error": str(error)}, sort_keys=True))
        return 1
    print(json.dumps({"ok": True, **summary}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
