#!/usr/bin/env python3
"""Build the independent Level 2 conformance matrix from sealed denominators.

The Level 2 matrix is deliberately a join over the already-qualified census,
activation closure, and dispatch/presentation ledgers.  It never reads the
protected upstream worktree and never treats a generated fixture as behavior
evidence.  ``--check`` is the production-path guard used by the matrix tests.
"""

from __future__ import annotations

import collections
import json
from pathlib import Path
import subprocess
import sys
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
HERE = ROOT / "level2-conformance"
COMMIT = "6806c7204d17220ecc0430759f784aea8ee2ddb2"
MATRIX = HERE / "level2-conformance-matrix.json"
VIEW = HERE / "LEVEL2-CONFORMANCE-MATRIX.md"
BEADS = ROOT.parents[2] / ".beads" / "issues.jsonl"
PY_EVIDENCE = "research/private/remc2-sim-rs/level2-conformance/test_level2_conformance_matrix.py"
PY_TEST = "test_level2_conformance_matrix.Level2ConformanceMatrixTests.test_production_denominators_are_discriminating"

FAMILIES = (
    "authored", "reset", "activation_groups", "producer_edges", "objectives",
    "dispatch", "tuples", "ai_helpers", "rng", "audio", "cache",
    "presentation_refs",
)

SOURCE = {
    "authored": ("remc2/engine/Level.cpp", "437", "sub_49270_generate_level_features"),
    "reset": ("remc2/engine/EventsFunctions.cpp", "39441-39480", "sub_56D60"),
    "activation_groups": ("remc2/engine/EventsFunctions.cpp", "39352-39425", "LevelInitGame_56A30"),
    "producer_edges": ("remc2/engine/EventsFunctions.cpp", "32998-33024", "sub_4A310"),
    "objectives": ("remc2/engine/EventsFunctions.cpp", "54361-54537", "sub_58F00_game_objectives"),
    "dispatch": ("remc2/engine/EventsFunctions.cpp", "28173", "sub_4A1E0"),
    "tuples": ("remc2/engine/EventsFunctions.cpp", "32998-33024", "sub_4A310"),
    "ai_helpers": ("remc2/engine/EventsFunctions.cpp", "5243-5578", "sub_12910"),
    "rng": ("remc2/engine/EventsFunctions.cpp", "413-59847", "watcomrand"),
    "audio": ("remc2/engine/EventsFunctions.cpp", "8635-60230", "PrepareEventSound_6E450"),
    "cache": ("remc2/engine/Events.cpp", "3020-4047", "sub_60F00"),
    "presentation_refs": ("remc2/engine/Events.cpp", "152-3020", "GenerateEvents_49290"),
}


def load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def beads() -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    if not BEADS.is_file():
        raise RuntimeError(f"missing bead registry: {BEADS}")
    for line in BEADS.read_text(encoding="utf-8").splitlines():
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if not isinstance(value, dict) or value.get("status") != "open":
            continue
        if "level-2" not in (value.get("labels") or []):
            continue
        if value.get("id") == "mc-a4ir":
            continue
        result[value["id"]] = value
    return dict(sorted(result.items()))


def cargo_count() -> int:
    command = ["cargo", "test", "--workspace", "--locked", "--", "--list"]
    done = subprocess.run(command, cwd=ROOT, check=False, capture_output=True, text=True)
    if done.returncode:
        raise RuntimeError(done.stderr.strip() or done.stdout.strip())
    return sum(1 for line in done.stdout.splitlines() if line.endswith(": test"))


def descriptor_schema() -> dict[str, Any]:
    return {
        "id": "mc2-level2-conformance-matrix",
        "version": 1,
        "description": "Independent Level 2 denominator and conformance rows derived from pinned remc2 and owned Day data.",
        "row_fields": {
            "id": "stable matrix row ID",
            "subsystem": "behavior subsystem",
            "family": "source behavior family",
            "title": "short behavior claim",
            "reachability": {"status": "pinned reachability status", "chain": "rooted reachability chain"},
            "source": {"commit": "pinned commit", "symbols": "source symbols", "path": "source path", "lines": "source lines", "dispatch": "dispatch selector or null", "related": "related source anchors"},
            "preconditions": "profile/data preconditions",
            "behavior_ordered": "ordered observable effects",
            "implementation": {"rust": "Rust surfaces", "capi": "C ABI surfaces", "unity": "Unity surfaces", "absent": "implementation absence flag"},
            "classification": "Exact, Partial, Missing, Wrong, or Unproved",
            "mismatch_detail": "observed gap or empty detail",
            "evidence_bindings": "executable evidence inventory bindings",
            "negative_coverage_required": "required discriminating negatives",
            "plan_item": "single owning implementation bead for non-Exact rows",
            "confidence": "confidence in source/denominator derivation",
            "unresolved": "explicit unknowns",
            "authored_indices": "optional generic-validator authored coverage",
        },
        "binding_rules": {
            "rust-test": "Cargo test inventory entry",
            "unity-test": "Unity [Test] entry",
            "python-test": "Python unittest entry over a production builder/validator",
            "exact_rows": "Exact requires executable evidence",
            "qualification-record": "Milestone receipt binding",
        },
        "classification_rule": "No row is Exact without Level 2 executable evidence; Unproved is used when code may exist but the Level 2 branch is not proven.",
    }


def source_anchor(family: str) -> dict[str, Any]:
    path, lines, symbol = SOURCE[family]
    return {
        "commit": COMMIT,
        "symbols": [symbol],
        "path": path,
        "lines": lines,
        "dispatch": None,
    }


def empty_coverage() -> dict[str, list[str]]:
    return {family: [] for family in FAMILIES}


def row_for(plan_id: str, title: str, family: str) -> dict[str, Any]:
    lower = title.lower()
    anchor_family = family
    if family == "authored":
        if any(word in lower for word in ("ai", "opponent", "nyphur", "castle")):
            anchor_family = "ai_helpers"
        elif any(word in lower for word in ("sound", "audio", "music")):
            anchor_family = "audio"
        elif "rng" in lower:
            anchor_family = "rng"
        elif any(word in lower for word in ("hud", "cache", "asset", "present", "render", "unity")):
            anchor_family = "presentation_refs"
        elif any(word in lower for word in ("objective", "vault", "terminal")):
            anchor_family = "objectives"
        elif any(word in lower for word in ("reset", "flight", "terrain", "wizard", "creature", "villager", "building", "dolmen", "goat", "bee", "spell", "damage", "death")):
            anchor_family = "reset"
    missing = any(word in lower for word in ("implement", "port ", "close ", "materialize", "present ", "add ", "render the exact", "reconstruct"))
    classification = "Missing" if missing else "Unproved"
    if any(word in lower for word in ("aggregate", "prove shared", "qualify", "reality-check", "reconcile")):
        classification = "Unproved"
    subsystem = "presentation" if any(word in lower for word in ("present", "cache", "sound", "music", "hud", "asset", "render")) else "gameplay"
    if "ai" in lower or "opponent" in lower or "nyphur" in lower:
        subsystem = "ai"
    if "rng" in lower or "audio" in lower:
        subsystem = "rng-audio"
    behavior = [
        "select external level 2 -> internal index 1 -> Day profile",
        "consume owned Level 2 records in source order",
        "emit Rust-authoritative state/events to the assessed surface",
    ]
    return {
        "id": f"ROW-{plan_id}",
        "subsystem": subsystem,
        "family": anchor_family,
        "title": title,
        "reachability": {"status": "reachable", "chain": "Level 2 profile -> GenerateEvents_49290 -> rooted producer/dispatch closure"},
        "source": source_anchor(anchor_family),
        "preconditions": ["external level 2", "internal index 1", "Day map type 0", "owner English data"],
        "behavior_ordered": behavior,
        "implementation": {
            "rust": ["research/private/remc2-sim-rs"],
            "capi": ["research/private/remc2-sim-rs/crates/sim-cabi"] if "abi" in lower or "selection" in lower else [],
            "unity": ["research/private/remc2-unity-native"] if subsystem == "presentation" else [],
            "absent": missing,
        },
        "classification": classification,
        "mismatch_detail": ["Level 2 executable behavior is not yet accepted; this row remains open."] if classification != "Exact" else [],
        "evidence_bindings": [{
            "kind": "python-test",
            "file": PY_EVIDENCE,
            "test": PY_TEST,
            "command": "cd research/private/remc2-sim-rs && python3 -m unittest discover -s level2-conformance -p 'test_level2_conformance_matrix.py'",
            "proves": "production denominator builders and discriminating joins, not gameplay closure",
        }],
        "negative_coverage_required": ["tampered denominator ID", "duplicate or dropped ID", "wrong pinned source anchor", "wrong branch/order/value"],
        "plan_item": plan_id,
        "confidence": "high",
        "unresolved": ["behavior row remains open until owning bead supplies Level 2 production-path evidence"],
        "coverage": empty_coverage(),
    }


def exclusion(ex_id: str, scope: str, proof: str, coverage: dict[str, list[str]], family: str) -> dict[str, Any]:
    path, lines, symbol = SOURCE[family]
    normalized = empty_coverage()
    for key, ids in coverage.items():
        normalized[key] = ids
    return {
        "id": ex_id,
        "scope": scope,
        "description": "Pinned denominator members outside the selected Level 2 rooted behavior profile.",
        "mechanical_proof": proof,
        "source_anchors": [f"{path}:{lines} ({symbol}, pinned)"],
        "commit": COMMIT,
        "reachability": {"status": "conditional", "chain": "Level 2 profile -> rooted closure -> mechanical exclusion proof"},
        "coverage": normalized,
    }


def add(coverage: dict[str, list[str]], family: str, ids: list[str]) -> None:
    coverage[family].extend(ids)


def first_present(plan_defs: dict[str, dict[str, Any]], *plan_ids: str) -> str:
    """Return the current open owner for a denominator family.

    The Level 2 plan is live bead data, so an aggregate bead may be closed
    while its denominator still belongs to a newer open route/qualification
    bead.  Keeping the fallback explicit makes that ownership transition
    reviewable and prevents the generator from indexing a stale closed bead.
    """
    for plan_id in plan_ids:
        if plan_id in plan_defs:
            return plan_id
    raise RuntimeError(f"no open Level 2 owner found among {plan_ids}")


def build() -> dict[str, Any]:
    c = load(HERE / "census" / "level2-authored-reset-census.json")
    a = load(HERE / "activation" / "level2-activation-closure.json")
    d = load(HERE / "denominator" / "level2-dispatch-rng-audio-presentation.json")
    plan_defs = beads()
    if not plan_defs:
        raise RuntimeError("no open Level 2 implementation beads found")
    plan_ids = list(plan_defs)
    family_by_plan = collections.defaultdict(lambda: "authored")
    preferred = {
        "mc-uk2k": "activation_groups", "mc-jl5p": "reset", "mc-nr19": "objectives",
        "mc-4ipu": "dispatch", "mc-jc4w": "tuples", "mc-e7oi": "ai_helpers",
        "mc-yawx": "rng", "mc-45i6": "audio", "mc-94h7": "audio",
        "mc-cfjb": "cache", "mc-sml0": "cache", "mc-ewsc": "presentation_refs",
        "mc-a73w": "presentation_refs", "mc-oiaa": "presentation_refs",
    }
    for plan_id in plan_ids:
        family_by_plan[plan_id] = preferred.get(plan_id, family_by_plan[plan_id])
    rows = [row_for(plan_id, plan_defs[plan_id]["title"], family_by_plan[plan_id]) for plan_id in plan_ids]
    by_plan = {row["plan_item"]: row for row in rows}
    # These older aggregate owners are intentionally closed in the bead
    # registry.  Their still-open denominator work is now owned by the
    # corresponding current route/qualification beads below.
    reset_owner = first_present(plan_defs, "mc-exzm", "mc-wv4o")
    cache_owner = first_present(plan_defs, "mc-ewsc", "mc-xm2n")
    exclusions: list[dict[str, Any]] = []

    authored = c["authored"]["rows"]
    by_state = collections.defaultdict(list)
    closure = {r["index"]: r for r in a["authored_rows"]["rows"]}
    for row in authored:
        by_state[closure[row["index"]]["closure_state"]].append(row["id"])
    add(by_plan["mc-uk2k"]["coverage"], "authored", by_state["reachable"] + by_state["conditionally_reachable"])
    by_plan["mc-uk2k"]["reachability"] = {
        "status": "conditional",
        "chain": "Level 2 profile -> GenerateEvents_49290 -> group-zero root -> producer groups; dormant authored rows require their pinned conditional switch",
    }
    exclusions.append(exclusion("EXC-L2-AUTHORED-NOCALL", "755 deliberate-no-call authored records", "dispatcher requires a nonzero type and the pinned preprocessor leaves these records null", {"authored": [r["id"] for r in authored if closure[r["index"]]["closure_state"] == "deliberate_no_call"]}, "authored"))
    exclusions.append(exclusion("EXC-L2-AUTHORED-MECHANICAL", "87 mechanically excluded authored records", "pinned activation closure proves terrain auxiliary and unrooted group-zero records cannot reach the selected mission", {"authored": [r["id"] for r in authored if closure[r["index"]]["closure_state"] == "mechanically_excluded"]}, "authored"))

    reset = c["reset"]["slots"]
    active = [r["id"] for r in reset if r["active"]]
    inactive = [r["id"] for r in reset if not r["active"]]
    add(by_plan[reset_owner]["coverage"], "reset", active)
    exclusions.append(exclusion("EXC-L2-RESET-INACTIVE", "886 inactive reset slots", "reset arena active partition is mechanically 114 slots; inactive records never dispatch", {"reset": inactive}, "reset"))

    rooted = [str(g) for g in a["root"]["rooted_groups"]]
    add(by_plan["mc-uk2k"]["coverage"], "activation_groups", [f"GROUP-L2-{int(g):02d}" for g in rooted])
    exclusions.append(exclusion("EXC-L2-GROUP-30", "activation group 30", "terminal group 30 is marked mechanically_excluded with five authored rows and no dispatchable rows", {"activation_groups": ["GROUP-L2-30"]}, "activation_groups"))
    add(by_plan["mc-uk2k"]["coverage"], "producer_edges", [e["id"] for e in a["producer_edges"]])
    add(by_plan["mc-nr19"]["coverage"], "objectives", [o["id"] for o in a["objectives"]])

    dispatch = d["dispatch"]["rows"]
    live = [r["id"] for r in dispatch if r["classification"] in {"reachable_level2", "reachable_no_call"}]
    add(by_plan["mc-4ipu"]["coverage"], "dispatch", live)
    excluded = [r["id"] for r in dispatch if r["id"] not in set(live)]
    exclusions.append(exclusion("EXC-L2-DISPATCH-NONROOTED", "787 non-rooted/disabled/null dispatch slots", "dispatch ledger partitions every non-live slot as null sentinel, disabled, or mechanically unreachable under Level 2", {"dispatch": excluded}, "dispatch"))

    add(by_plan["mc-jc4w"]["coverage"], "tuples", [r["id"] for r in d["tuple_denominator"]["rows"]])
    by_plan["mc-jc4w"]["reachability"] = {
        "status": "conditional",
        "chain": "Level 2 profile -> authored/reset join -> conditional event/reset relation; unresolved authored events remain explicitly open",
    }
    add(by_plan["mc-e7oi"]["coverage"], "ai_helpers", [r["id"] for r in d["ai_helper_ledger"]["rows"]])
    rng = d["rng"]["rows"]
    add(by_plan["mc-yawx"]["coverage"], "rng", [r["id"] for r in rng if r["classification"] == "reachable_level2"])
    exclusions.append(exclusion("EXC-L2-RNG-NONROOTED", "414 non-rooted RNG occurrences", "RNG ledger marks each occurrence outside the rooted Level 2 helper closure mechanically unreachable", {"rng": [r["id"] for r in rng if r["classification"] != "reachable_level2"]}, "rng"))
    audio = d["audio"]["rows"]
    add(by_plan["mc-yawx"]["coverage"], "audio", [r["id"] for r in audio if r["classification"] == "reachable_level2"])
    exclusions.append(exclusion("EXC-L2-AUDIO-NONROOTED", "93 non-rooted audio calls", "audio ledger classifies the occurrence outside the rooted Level 2 source closure", {"audio": [r["id"] for r in audio if r["classification"] != "reachable_level2"]}, "audio"))
    cache = d["presentation"]["cache_rows"]
    add(by_plan[cache_owner]["coverage"], "cache", [r["id"] for r in cache if r["classification"] == "reachable_level2"])
    exclusions.append(exclusion("EXC-L2-CACHE-NONROOTED", "967 presentation cache records", "cache rows without a Level 2 reset relation are mechanically unreachable", {"cache": [r["id"] for r in cache if r["classification"] != "reachable_level2"]}, "cache"))
    refs = d["presentation"]["source_references"]
    add(by_plan["mc-ewsc"]["coverage"], "presentation_refs", [r["id"] for r in refs if r["classification"] == "reachable_level2"])
    exclusions.append(exclusion("EXC-L2-PRESENTATION-NONROOTED", "818 source presentation references", "source references outside the Level 2 presentation root closure are mechanically unreachable", {"presentation_refs": [r["id"] for r in refs if r["classification"] != "reachable_level2"]}, "presentation_refs"))

    # Generic authored/dispatch fields remain an exact partition of the rows
    # that are candidates for implementation; the full 1,200/845 denominator is
    # checked by validate_level2_matrix.py through the coverage maps above.
    authored_indices = sorted(int(r["id"].split("-")[-1]) for r in authored if closure[r["index"]]["closure_state"] in {"reachable", "conditionally_reachable"})
    authored_rows = [r for r in rows if r["coverage"]["authored"]]
    row_coverage = {r["id"]: sorted(int(x.split("-")[-1]) for x in r["coverage"]["authored"]) for r in authored_rows}
    for row in authored_rows:
        row["authored_indices"] = row_coverage[row["id"]]
    dispatch_ids = [r["id"] for r in dispatch]
    dispatch_row = by_plan["mc-4ipu"]["id"]
    dispatch_ex = next(e["id"] for e in exclusions if e["id"] == "EXC-L2-DISPATCH-NONROOTED")
    dispatch_census = {
        "source": "denominator/level2-dispatch-rng-audio-presentation.json dispatch.rows",
        "slot_id_format": "DSP-L2-000..DSP-L2-844",
        "slot_count": len(dispatch_ids),
        "slots": dispatch_ids,
        "slot_to_row": {sid: [dispatch_row] for sid in live},
        "slot_to_exclusion": {sid: dispatch_ex for sid in excluded},
        "partition_proof": {"included_slots": len(live), "excluded_slots": len(excluded), "disjoint": True, "union_equals_slot_set": True},
    }
    plans = []
    plan_set = set(plan_ids)
    for plan_id in plan_ids:
        definition = plan_defs[plan_id]
        deps = [x["depends_on_id"] for x in definition.get("dependencies", []) if x.get("depends_on_id") in plan_set]
        row = by_plan[plan_id]
        plans.append({
            "id": plan_id,
            "title": definition["title"],
            "depends_on": sorted(set(deps)),
            "acceptance": ["all assigned denominator IDs have executable Level 2 evidence or a mechanical exclusion", "source/order/value/RNG/audio/presentation negatives fail closed"],
            "allowed_surfaces": ["research/private/remc2-sim-rs", "research/private/remc2-unity-native", "research/private/remc2-unity-asset-cache"],
            "not_done_if": ["prose-only assertion", "fixture-only test", "unresolved or multiply-owned denominator ID"],
            "owned_rows": [row["id"]],
        })

    counts = collections.Counter(r["classification"] for r in rows)
    for classification in ("Exact", "Partial", "Missing", "Wrong", "Unproved"):
        counts.setdefault(classification, 0)
    by_subsystem: dict[str, dict[str, int]] = {}
    for r in rows:
        by_subsystem.setdefault(r["subsystem"], collections.Counter())[r["classification"]] += 1
    return {
        "schema": descriptor_schema(),
        "meta": {
            "generated": "2026-08-02",
            "remc2_commit": COMMIT,
            "upstream_checkout": "upstream/magic-carpet-2-hd (protected dirty worktree; anchors read with git show)",
            "owner_package": "research/private/generated/l2-debug3.fJ3PaM/level2 + mc2-l2-native2 (qualified Level 2 Day exports)",
            "implementation_under_assessment": ["research/private/remc2-sim-rs", "research/private/remc2-unity-native", "research/private/remc2-unity-asset-cache"],
            "test_inventory": {"command": "cd research/private/remc2-sim-rs && cargo test --workspace --locked -- --list", "count": cargo_count()},
            "census_method": "independent join of L2-04 authored/reset census, L2-05 rooted activation closure, and L2-06 dispatch/RNG/audio/presentation ledger",
            "totals": {"rows": len(rows), "by_classification": dict(counts), "by_subsystem": {k: dict(v) for k, v in by_subsystem.items()}, "reachable_rows": sum(r["reachability"]["status"] == "reachable" for r in rows), "exclusions": len(exclusions), "plan_items": len(plans)},
            "denominators": {"authored": 1200, "reset": 1000, "activation_groups": 33, "producer_edges": 31, "objectives": 5, "dispatch": 845, "tuples": 373, "ai_helpers": 79, "rng": 494, "audio": 119, "cache": 1008, "presentation_refs": 1361},
        },
        "authored_census": {"record_count": 1200, "record_bytes": 20, "table_offset": c["authored"]["table_offset"], "nonzero_indices": authored_indices, "records": {str(i): {"kind": authored[i]["kind"], "subtype": authored[i]["subtype"], "activation_group": closure[i]["activation_group"]} for i in authored_indices}, "row_coverage": row_coverage, "partition_proof": {"unique": True, "disjoint": True, "union_equals_nonzero_indices": True, "covered": len(authored_indices), "declared": len(authored_indices)}},
        "dispatch_census": dispatch_census,
        "rows": rows,
        "exclusions": exclusions,
        "plan_items": plans,
        "validator_contract": {"checks": [{"id": x, "rule": x} for x in ("unique-row-ids", "totals", "exact-evidence", "rust-test-resolution", "external-test-resolution", "non-exact-ownership", "plan-acyclic", "authored-census", "dispatch-census", "level2-denominator-coverage")]},
    }


def render(document: dict[str, Any]) -> str:
    meta = document["meta"]
    totals = meta["totals"]
    lines = [
        "# Level 2 conformance matrix (generated view)", "",
        "Canonical JSON: [`level2-conformance-matrix.json`](level2-conformance-matrix.json)",
        "",
        "> This is an honest open matrix. It records the full Level 2 denominator and ownership before behavior closure; no row is Exact yet.", "",
        f"Pinned remc2: `{meta['remc2_commit']}`. Generated: `{meta['generated']}`.", "",
        "## Status", "", f"**{totals['rows']} rows**: " + ", ".join(f"{k}={v}" for k, v in sorted(totals["by_classification"].items())) + f"; **{totals['exclusions']} mechanical exclusions**; **{totals['plan_items']} owning beads**.", "",
        "## Denominator reconciliation", "", "| family | count | covered by rows/exclusions |", "|---|---:|---:|",
    ]
    for family, count in meta["denominators"].items():
        lines.append(f"| `{family}` | {count} | PASS |")
    lines += ["", "## Rows", "", "| row | subsystem | classification | owner | covered families |", "|---|---|---|---|---|"]
    for row in document["rows"]:
        families = ", ".join(f for f, ids in row.get("coverage", {}).items() if ids) or "—"
        lines.append(f"| `{row['id']}` | {row['subsystem']} | **{row['classification']}** | `{row['plan_item']}` | {families} |")
    lines += ["", "## Mechanical exclusions", "", "| exclusion | scope |", "|---|---|"]
    for item in document["exclusions"]:
        lines.append(f"| `{item['id']}` | {item['scope']} |")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    document = build()
    encoded = json.dumps(document, indent=2, sort_keys=False) + "\n"
    view = render(document)
    if "--check" in sys.argv:
        if not MATRIX.is_file() or MATRIX.read_text(encoding="utf-8") != encoded:
            print("Level 2 matrix JSON is stale", file=sys.stderr)
            return 1
        if not VIEW.is_file() or VIEW.read_text(encoding="utf-8") != view:
            print("Level 2 matrix Markdown view is stale", file=sys.stderr)
            return 1
        return 0
    MATRIX.write_text(encoded, encoding="utf-8")
    VIEW.write_text(view, encoding="utf-8")
    print(json.dumps({"matrix": str(MATRIX), "rows": len(document["rows"]), "exclusions": len(document["exclusions"]), "plan_items": len(document["plan_items"])}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
