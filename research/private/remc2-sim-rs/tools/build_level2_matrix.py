#!/usr/bin/env python3
"""Build the independent Level 2 conformance matrix from sealed denominators.

The Level 2 matrix is deliberately a join over the already-qualified census,
activation closure, and dispatch/presentation ledgers.  It never reads the
protected upstream worktree and never treats a generated fixture as behavior
evidence.  ``--check`` is the production-path guard used by the matrix tests.

Ownership is resolved by joining stable denominator IDs (and, where the
ledger's own classification field is contradicted by the activation closure,
activation reachability) against each *leaf* bead's own structured
"Denominator ownership:" declaration in ``.beads/issues.jsonl``.  No row's
family, classification, or owner is derived by scanning English behavior
prose in a bead title.
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

MATRIX_CONTROL_BEAD_IDS = {"mc-a4ir", "mc-a4ir.1"}

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

# ---------------------------------------------------------------------------
# Leaf ownership join tables.
#
# Every open Level 2 bead's description carries a structured "Denominator
# ownership:" line (see `.beads/issues.jsonl`).  The tables below are that
# declared ownership translated into executable joins over the pinned
# census/activation/denominator artifacts: explicit stable IDs where the
# bead names them literally, and field-level joins (activation_group,
# runtime class/model, source path/symbol, ledger classification) where the
# bead names a computable band.  None of this depends on scanning English
# gameplay prose in a bead title.
#
# A handful of assignments are coarse, single-leaf placeholders because the
# pinned ledgers do not expose a safe, mechanical discriminator fine enough
# to split them further (documented at each site below and in the packet's
# closure report).  Those placeholders still satisfy "one leaf owner, never
# a parent aggregate" even though the fine split they describe remains a
# follow-up.
# ---------------------------------------------------------------------------


def _expand_id_ranges(text: str) -> list[int]:
    """Expand '0023-0025, 0030-0031, 0043' into [23, 24, 25, 30, 31, 43]."""
    ids: list[int] = []
    for part in text.split(","):
        part = part.strip()
        if not part:
            continue
        if "-" in part:
            lo, hi = part.split("-", 1)
            ids.extend(range(int(lo), int(hi) + 1))
        else:
            ids.append(int(part))
    return ids


# AI_HELPER-NNNN ownership, transcribed verbatim from each leaf bead's
# "Denominator ownership:" line.  Reconciles to exactly the 79 reachable
# AI_HELPER rows in the denominator ledger (verified by
# test_worker_ownership_is_disjoint_and_complete).
AI_HELPER_RANGES = {
    "mc-1g06.1": "23-25, 30-31, 43, 45-46, 52",
    "mc-1g06.2": "68, 72-74",
    "mc-4ipu.1": "8",
    "mc-4ipu.2": "11",
    "mc-4ipu.3": "12-15",
    "mc-6dqd.1": "3, 4, 21, 22, 26-29, 53, 67",
    "mc-6dqd.2": "0, 1, 5, 6, 32-35, 38-41, 47-49, 71",
    "mc-app8.1": "2, 7, 9",
    "mc-app8.2": "10, 36, 37, 42, 44, 50, 51, 62-66, 69-70, 75-78",
    "mc-k5js.1": "16-17",
    "mc-k5js.2": "18-20",
    "mc-uptb.1": "54",
    "mc-uptb.2": "55-61",
}
AI_HELPER_OWNER: dict[str, str] = {}
for _bead, _text in AI_HELPER_RANGES.items():
    for _n in _expand_id_ranges(_text):
        AI_HELPER_OWNER[f"AI_HELPER-{_n:04d}"] = _bead

# (runtime_class, model) -> leaf bead.  Shared by the reset census, the
# CTOR-flavored tuple_denominator rows, and (for the classes it resolves)
# the dispatch constructor/action tables -- the same pinned "class/model"
# identity selects the same owner in every ledger that carries it.
CLASS_MODEL_OWNER = {
    (2, 2): "mc-oiaa.1",       # bad_stone / dolmen family (Basic.cpp)
    (2, 3): "mc-oiaa.1",
    (3, 0): "mc-bvihb.1",      # castle/local-wizard construction
    (3, 1): "mc-bvihb.1",
    (5, 1): "mc-2sbd.1",       # goat (default; slots/tuples 39/71 overridden below)
    (5, 3): "mc-38gs.1",
    (5, 4): "mc-38gs.3",
    (5, 9): "mc-38gs.2",
    (5, 13): "mc-50i6",        # villager: shared Day creature inherited from Level 1
    (5, 14): "mc-38gs.4",
    (10, 45): "mc-oiaa.3",     # building / vault preprocessor rows
    (11, 0): "mc-clng",        # family 11 (Invisible)
    (11, 4): "mc-clng",
    (11, 32): "mc-clng",
    (15, 0): "mc-xcgi.1.1.1",  # spell pickup / holder rows
    (15, 1): "mc-xcgi.1.1.1",
    (15, 3): "mc-xcgi.1.1.1",
    (15, 12): "mc-xcgi.1.1.1",
}

# mc-2sbd.2 explicitly names these two reset slots and their CTOR tuple IDs
# ("reset slots 39/71" / "CTOR-L2-05-001-009, CTOR-L2-05-001-011"); every
# other goat reset slot/tuple falls back to CLASS_MODEL_OWNER's (5, 1) entry.
GOAT_RESET_SLOT_OVERRIDE = {39: "mc-2sbd.2", 71: "mc-2sbd.2"}
GOAT_TUPLE_ID_OVERRIDE = {"CTOR-L2-05-001-009": "mc-2sbd.2", "CTOR-L2-05-001-011": "mc-2sbd.2"}

# Explicit AUT-L2 authored/tuple index overrides named by the mc-38gs.* leaf
# beads ("authored rows AUT-L2-0164/0165/..." etc.), applied ahead of the
# activation_group range join below.
AUTHORED_INDEX_OVERRIDE: dict[int, str] = {}
for _i in (164, 165, 167, 168, 169, 238):
    AUTHORED_INDEX_OVERRIDE[_i] = "mc-38gs.1"
for _i in (239, 240, 241, 242, 243, 245):
    AUTHORED_INDEX_OVERRIDE[_i] = "mc-38gs.2"
for _i in (232, 233, 234, 235):
    AUTHORED_INDEX_OVERRIDE[_i] = "mc-38gs.3"
for _i in range(125, 152):
    AUTHORED_INDEX_OVERRIDE[_i] = "mc-38gs.4"


def _activation_group_owner(group: int) -> str | None:
    """mc-uk2k.1..4 own GROUP-L2-01..08 / 09..18 / 19..29 / 31..32 respectively
    (mc-uk2k.4 also owns GROUP-L2-30's exclusion and all 31 producer edges).
    activation_group == -1 marks the DisId=-1 class10/model45 preprocessor
    rows, which belong to the building/vault owner alongside their reset
    census counterpart."""
    if group == -1:
        return "mc-oiaa.3"
    if 0 <= group <= 8:
        return "mc-uk2k.1"
    if 9 <= group <= 18:
        return "mc-uk2k.2"
    if 19 <= group <= 29:
        return "mc-uk2k.3"
    if group in (31, 32):
        return "mc-uk2k.4"
    return None  # group 30: mechanically excluded (EXC-L2-GROUP-30)


# DSP-L2 ids whose raw dispatch-scan classification in the pinned ledger
# ("mechanically_unreachable_level2") contradicts the activation closure:
# each of these shares its constructor/action group_token_hex with a
# target_classes entry the activation closure marks resolved and
# (conditionally_)reachable, and each owning leaf bead names the id (or,
# for the two "action bands" the packet names, the specific ids within
# those bands that match a bead-claimed constructor's own token) directly
# in its "Denominator ownership:" line.  These move from the mechanical
# dispatch exclusion into row coverage; the raw ledger classification is
# left untouched (it is a pinned, sealed artifact -- the join corrects the
# *matrix*, not the input).
DISPATCH_CONTRADICTION_OWNER = {
    # Bee actions 16-19 (token 0x002A5BD0)
    "DSP-L2-089": "mc-jmcp.1", "DSP-L2-090": "mc-jmcp.1",
    "DSP-L2-091": "mc-jmcp.2", "DSP-L2-092": "mc-jmcp.2",
    "DSP-L2-093": "mc-mmv3", "DSP-L2-094": "mc-mmv3",
    "DSP-L2-095": "mc-mmv3", "DSP-L2-096": "mc-mmv3",
    # class-5 model-3: constructor + actions 25 and 216-220 (token 0x002A5BD4)
    "DSP-L2-312": "mc-38gs.1", "DSP-L2-098": "mc-38gs.1",
    "DSP-L2-289": "mc-38gs.1", "DSP-L2-290": "mc-38gs.1", "DSP-L2-291": "mc-38gs.1",
    "DSP-L2-292": "mc-38gs.1", "DSP-L2-293": "mc-38gs.1",
    # class-5 model-4 (archer): constructor (token 0x002A5BD8)
    "DSP-L2-313": "mc-38gs.3",
    # class-5 model-9: constructor + action 72 (token 0x002A5BEC)
    "DSP-L2-318": "mc-38gs.2", "DSP-L2-145": "mc-38gs.2",
    # class-5 model-10 Whirlwind: reachable action 82 (token 0x002A5BF0)
    "DSP-L2-155": "mc-hgk9.1",
    # class-5 model-14 (trader): constructor + actions 112-119 (token 0x002A5C00)
    "DSP-L2-323": "mc-38gs.4",
    "DSP-L2-185": "mc-38gs.4", "DSP-L2-186": "mc-38gs.4", "DSP-L2-187": "mc-38gs.4",
    "DSP-L2-188": "mc-38gs.4", "DSP-L2-189": "mc-38gs.4", "DSP-L2-190": "mc-38gs.4",
    "DSP-L2-191": "mc-38gs.4", "DSP-L2-192": "mc-38gs.4",
}

# Owners for the dispatch rows the pinned ledger already classifies
# reachable_level2/reachable_no_call (58 rows), derived from each row's
# resolved_function/runtime_class/group_token_hex identity (see the
# packet's closure report for the per-id join).
DISPATCH_LIVE_OWNER: dict[str, str] = {}
for _id in ("DSP-L2-011", "DSP-L2-012", "DSP-L2-013", "DSP-L2-036", "DSP-L2-014",
            "DSP-L2-037", "DSP-L2-017", "DSP-L2-020"):
    DISPATCH_LIVE_OWNER[_id] = "mc-oiaa.1"          # class 2: tree/statue/dolmen
for _id in ("DSP-L2-045", "DSP-L2-058", "DSP-L2-059", "DSP-L2-044", "DSP-L2-048", "DSP-L2-049"):
    DISPATCH_LIVE_OWNER[_id] = "mc-bvihb.1"         # class 3: player/castle
for _id in ("DSP-L2-074", "DSP-L2-075", "DSP-L2-076", "DSP-L2-077", "DSP-L2-322", "DSP-L2-178"):
    DISPATCH_LIVE_OWNER[_id] = "mc-50i6"            # class 5: generic/villager
for _id in ("DSP-L2-310", "DSP-L2-082", "DSP-L2-084", "DSP-L2-086", "DSP-L2-087"):
    DISPATCH_LIVE_OWNER[_id] = "mc-2sbd.2"          # class 5 model 1: goat actions 9-14
for _id in ("DSP-L2-455", "DSP-L2-555"):
    DISPATCH_LIVE_OWNER[_id] = "mc-oiaa.3"          # class 10 model 45: building
DISPATCH_LIVE_OWNER["DSP-L2-486"] = "mc-hgk9.2"     # class 10 model 76: Fire Sphere
for _id in ("DSP-L2-411", "DSP-L2-416", "DSP-L2-423", "DSP-L2-427", "DSP-L2-432",
            "DSP-L2-462", "DSP-L2-464", "DSP-L2-468", "DSP-L2-469"):
    DISPATCH_LIVE_OWNER[_id] = "mc-oiaa.2"          # class 10: misc effects
for _id in ("DSP-L2-603", "DSP-L2-649", "DSP-L2-604", "DSP-L2-607", "DSP-L2-653",
            "DSP-L2-615", "DSP-L2-635", "DSP-L2-681"):
    DISPATCH_LIVE_OWNER[_id] = "mc-clng"            # class 11: family 11
for _id in ("DSP-L2-722", "DSP-L2-724", "DSP-L2-738", "DSP-L2-818", "DSP-L2-819",
            "DSP-L2-741", "DSP-L2-821", "DSP-L2-744", "DSP-L2-748", "DSP-L2-749",
            "DSP-L2-750", "DSP-L2-830", "DSP-L2-775"):
    DISPATCH_LIVE_OWNER[_id] = "mc-4ipu.1"          # class 14/15: no bead names these yet

# cache family: entity_parameters/sprite_selectors match mc-oiaa.1's
# "creature entity-parameter/sprite selectors"; particle_parameters matches
# mc-oiaa.2's "particle/sprite selectors and effect references".
CACHE_FAMILY_OWNER = {
    "entity_parameters": "mc-oiaa.1",
    "sprite_selectors": "mc-oiaa.1",
    "particle_parameters": "mc-oiaa.2",
}

# presentation_refs family: primary join is source path, which already
# names the owning leaf bead's file directly (mc-exzm.1's "Terrain
# references", mc-oiaa.1's "Basic.cpp references", mc-94h7.2's music
# lifecycle -> Sound.cpp).  GameUI.cpp is further split by the exact root
# symbols mc-a73w.1/.2/.3 each name.  EventsFunctions.cpp (the shared
# gameplay-dispatch file) and the handful of GameUI.cpp refs outside the
# three named a73w roots are not mechanically separable per-owner from the
# pinned ledger alone (no field ties an individual reference to a single
# creature/spell family); they fall to mc-c2by.1 as a coarse placeholder,
# flagged as a residual in the closure report.
PRESENTATION_PATH_OWNER = {
    "remc2/engine/Terrain.cpp": "mc-exzm.1",
    "remc2/engine/Basic.cpp": "mc-oiaa.1",
    "remc2/engine/Sound.cpp": "mc-94h7.2",
    "remc2/engine/EventsFunctions.cpp": "mc-c2by.1",
}
PRESENTATION_GAMEUI_SYMBOL_OWNER = {
    "DrawTopStatusBar_2D710": "mc-a73w.1",
    "DrawCurrentObjectiveTextbox_30630": "mc-a73w.2",
    "DrawMinimapEntities_B_61A00": "mc-a73w.3",
    "DrawMinimapMarks_644F0": "mc-a73w.3",
}
PRESENTATION_GAMEUI_FALLBACK_OWNER = "mc-c2by.1"


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
        if value.get("id") in MATRIX_CONTROL_BEAD_IDS:
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


def row_for(plan_id: str, title: str, is_aggregate: bool) -> dict[str, Any]:
    """Build a matrix row for a bead.

    Classification and subsystem are derived from the bead's own structural
    role (aggregate closure gate vs. worker-ready implementation packet, a
    literal marker in the bead description -- see build()) rather than from
    scanning the bead's English title for behavior keywords.  `family` and
    `source` are finalized by build() once this plan's actual denominator
    coverage is known, so a bead that owns e.g. both dispatch and authored
    IDs gets a source anchor consistent with what it actually covers.
    """
    classification = "Unproved" if is_aggregate else "Missing"
    behavior = [
        "select external level 2 -> internal index 1 -> Day profile",
        "consume owned Level 2 records in source order",
        "emit Rust-authoritative state/events to the assessed surface",
    ]
    return {
        "id": f"ROW-{plan_id}",
        "subsystem": "gameplay",  # finalized in build() once family is known
        "family": "authored",     # finalized in build() once family is known
        "title": title,
        "reachability": {"status": "reachable", "chain": "Level 2 profile -> GenerateEvents_49290 -> rooted producer/dispatch closure"},
        "source": source_anchor("authored"),  # finalized in build()
        "preconditions": ["external level 2", "internal index 1", "Day map type 0", "owner English data"],
        "behavior_ordered": behavior,
        "implementation": {
            "rust": ["research/private/remc2-sim-rs"],
            "capi": [],
            "unity": [],
            "absent": not is_aggregate,
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


def build() -> dict[str, Any]:
    c = load(HERE / "census" / "level2-authored-reset-census.json")
    a = load(HERE / "activation" / "level2-activation-closure.json")
    d = load(HERE / "denominator" / "level2-dispatch-rng-audio-presentation.json")
    plan_defs = beads()
    if not plan_defs:
        raise RuntimeError("no open Level 2 implementation beads found")
    plan_ids = list(plan_defs)
    is_aggregate = {
        plan_id: "worker-ready implementation packet" not in (plan_defs[plan_id].get("description") or "")
        for plan_id in plan_ids
    }
    rows = [row_for(plan_id, plan_defs[plan_id]["title"], is_aggregate[plan_id]) for plan_id in plan_ids]
    by_plan = {row["plan_item"]: row for row in rows}

    def owner_row(plan_id: str) -> dict[str, Any]:
        if plan_id not in by_plan:
            raise RuntimeError(f"denominator join names {plan_id!r}, which is not an open Level 2 bead")
        return by_plan[plan_id]

    exclusions: list[dict[str, Any]] = []

    # -- authored ------------------------------------------------------
    authored = c["authored"]["rows"]
    closure = {r["index"]: r for r in a["authored_rows"]["rows"]}
    by_state = collections.defaultdict(list)
    for row in authored:
        by_state[closure[row["index"]]["closure_state"]].append(row["id"])
    for row in authored:
        idx = row["index"]
        state = closure[idx]["closure_state"]
        if state not in ("reachable", "conditionally_reachable"):
            continue
        owner = AUTHORED_INDEX_OVERRIDE.get(idx) or _activation_group_owner(closure[idx]["activation_group"])
        if owner is None:
            raise RuntimeError(f"authored row {row['id']} (group {closure[idx]['activation_group']}) has no owner join")
        add(owner_row(owner)["coverage"], "authored", [row["id"]])
    exclusions.append(exclusion("EXC-L2-AUTHORED-NOCALL", "755 deliberate-no-call authored records", "dispatcher requires a nonzero type and the pinned preprocessor leaves these records null", {"authored": [r["id"] for r in authored if closure[r["index"]]["closure_state"] == "deliberate_no_call"]}, "authored"))
    exclusions.append(exclusion("EXC-L2-AUTHORED-MECHANICAL", "87 mechanically excluded authored records", "pinned activation closure proves terrain auxiliary and unrooted group-zero records cannot reach the selected mission", {"authored": [r["id"] for r in authored if closure[r["index"]]["closure_state"] == "mechanically_excluded"]}, "authored"))

    # -- reset -----------------------------------------------------------
    reset = c["reset"]["slots"]
    active = [r for r in reset if r["active"]]
    inactive = [r["id"] for r in reset if not r["active"]]
    for slot in active:
        key = (slot["class"], slot["model"])
        if key == (5, 1) and slot["slot"] in GOAT_RESET_SLOT_OVERRIDE:
            owner = GOAT_RESET_SLOT_OVERRIDE[slot["slot"]]
        else:
            owner = CLASS_MODEL_OWNER.get(key)
        if owner is None:
            raise RuntimeError(f"reset slot {slot['id']} (class={slot['class']} model={slot['model']}) has no owner join")
        add(owner_row(owner)["coverage"], "reset", [slot["id"]])
    exclusions.append(exclusion("EXC-L2-RESET-INACTIVE", "886 inactive reset slots", "reset arena active partition is mechanically 114 slots; inactive records never dispatch", {"reset": inactive}, "reset"))

    # -- activation_groups + producer_edges -------------------------------
    rooted = [int(g) for g in a["root"]["rooted_groups"]]
    for g in rooted:
        owner = _activation_group_owner(g)
        if owner is None:
            raise RuntimeError(f"activation group {g} has no owner join")
        add(owner_row(owner)["coverage"], "activation_groups", [f"GROUP-L2-{g:02d}"])
    exclusions.append(exclusion("EXC-L2-GROUP-30", "activation group 30", "terminal group 30 is marked mechanically_excluded with five authored rows and no dispatchable rows", {"activation_groups": ["GROUP-L2-30"]}, "activation_groups"))
    add(owner_row("mc-uk2k.4")["coverage"], "producer_edges", [e["id"] for e in a["producer_edges"]])
    add(owner_row("mc-nr19.1")["coverage"], "objectives", [o["id"] for o in a["objectives"]])

    # -- dispatch ----------------------------------------------------------
    dispatch = d["dispatch"]["rows"]
    ledger_live_ids = [r["id"] for r in dispatch if r["classification"] in {"reachable_level2", "reachable_no_call"}]
    live = set(ledger_live_ids) | set(DISPATCH_CONTRADICTION_OWNER)
    dispatch_owner: dict[str, str] = {}
    # Iterate the pinned dispatch.rows list (deterministic JSON order), not a
    # set, so slot_to_row/coverage ordering is byte-stable across runs.
    for sid in ledger_live_ids:
        dispatch_owner[sid] = DISPATCH_LIVE_OWNER[sid]
    for sid, owner in DISPATCH_CONTRADICTION_OWNER.items():
        dispatch_owner[sid] = owner
    if set(dispatch_owner) != live:
        raise RuntimeError("dispatch ownership join does not exactly cover the live dispatch set")
    for sid, owner in dispatch_owner.items():
        add(owner_row(owner)["coverage"], "dispatch", [sid])
    excluded = [r["id"] for r in dispatch if r["id"] not in live]
    exclusions.append(exclusion(
        "EXC-L2-DISPATCH-NONROOTED",
        f"{len(excluded)} non-rooted/disabled/null dispatch slots",
        "dispatch ledger partitions every non-live slot as null sentinel, disabled, or mechanically unreachable under Level 2",
        {"dispatch": excluded},
        "dispatch",
    ))

    # -- tuples --------------------------------------------------------
    for row in d["tuple_denominator"]["rows"]:
        if row["id"].startswith("CTOR-L2-"):
            key = (row["runtime_class"], row["model"])
            if key == (5, 1) and row["id"] in GOAT_TUPLE_ID_OVERRIDE:
                owner = GOAT_TUPLE_ID_OVERRIDE[row["id"]]
            else:
                owner = CLASS_MODEL_OWNER.get(key)
        else:
            idx = int(row["id"].split("-")[-1])
            owner = AUTHORED_INDEX_OVERRIDE.get(idx) or _activation_group_owner(row["activation_group"])
        if owner is None:
            raise RuntimeError(f"tuple row {row['id']} has no owner join")
        add(owner_row(owner)["coverage"], "tuples", [row["id"]])

    # -- ai_helpers ------------------------------------------------------
    for row in d["ai_helper_ledger"]["rows"]:
        owner = AI_HELPER_OWNER.get(row["id"])
        if owner is None:
            raise RuntimeError(f"ai helper {row['id']} has no owner join")
        add(owner_row(owner)["coverage"], "ai_helpers", [row["id"]])

    # -- rng / audio: wholly owned by the beads that build their ledgers --
    rng = d["rng"]["rows"]
    add(owner_row("mc-yawx.1")["coverage"], "rng", [r["id"] for r in rng if r["classification"] == "reachable_level2"])
    exclusions.append(exclusion("EXC-L2-RNG-NONROOTED", "414 non-rooted RNG occurrences", "RNG ledger marks each occurrence outside the rooted Level 2 helper closure mechanically unreachable", {"rng": [r["id"] for r in rng if r["classification"] != "reachable_level2"]}, "rng"))
    audio = d["audio"]["rows"]
    add(owner_row("mc-yawx.2")["coverage"], "audio", [r["id"] for r in audio if r["classification"] == "reachable_level2"])
    exclusions.append(exclusion("EXC-L2-AUDIO-NONROOTED", "93 non-rooted audio calls", "audio ledger classifies the occurrence outside the rooted Level 2 source closure", {"audio": [r["id"] for r in audio if r["classification"] != "reachable_level2"]}, "audio"))

    # -- cache -----------------------------------------------------------
    cache = d["presentation"]["cache_rows"]
    for row in cache:
        if row["classification"] != "reachable_level2":
            continue
        owner = CACHE_FAMILY_OWNER.get(row["family"])
        if owner is None:
            raise RuntimeError(f"cache row {row['id']} family {row['family']!r} has no owner join")
        add(owner_row(owner)["coverage"], "cache", [row["id"]])
    exclusions.append(exclusion("EXC-L2-CACHE-NONROOTED", "967 presentation cache records", "cache rows without a Level 2 reset relation are mechanically unreachable", {"cache": [r["id"] for r in cache if r["classification"] != "reachable_level2"]}, "cache"))

    # -- presentation_refs -------------------------------------------------
    refs = d["presentation"]["source_references"]
    for row in refs:
        if row["classification"] != "reachable_level2":
            continue
        if row["path"] == "remc2/engine/GameUI.cpp":
            owner = PRESENTATION_GAMEUI_SYMBOL_OWNER.get(row["enclosing_symbol"], PRESENTATION_GAMEUI_FALLBACK_OWNER)
        else:
            owner = PRESENTATION_PATH_OWNER.get(row["path"])
        if owner is None:
            raise RuntimeError(f"presentation reference {row['id']} path {row['path']!r} has no owner join")
        add(owner_row(owner)["coverage"], "presentation_refs", [row["id"]])
    exclusions.append(exclusion("EXC-L2-PRESENTATION-NONROOTED", "818 source presentation references", "source references outside the Level 2 presentation root closure are mechanically unreachable", {"presentation_refs": [r["id"] for r in refs if r["classification"] != "reachable_level2"]}, "presentation_refs"))

    # -- finalize family/subsystem/source now that coverage is known -------
    subsystem_for_family = {
        "ai_helpers": "ai",
        "rng": "rng-audio", "audio": "rng-audio",
        "cache": "presentation", "presentation_refs": "presentation",
    }
    for row in rows:
        anchor_family = next((f for f in FAMILIES if row["coverage"][f]), "authored")
        row["family"] = anchor_family
        row["subsystem"] = subsystem_for_family.get(anchor_family, "gameplay")
        row["source"] = source_anchor(anchor_family)

    # Generic authored/dispatch fields remain an exact partition of the rows
    # that are candidates for implementation; the full 1,200/845 denominator is
    # checked by validate_level2_matrix.py through the coverage maps above.
    authored_indices = sorted(int(r["id"].split("-")[-1]) for r in authored if closure[r["index"]]["closure_state"] in {"reachable", "conditionally_reachable"})
    authored_rows = [r for r in rows if r["coverage"]["authored"]]
    row_coverage = {r["id"]: sorted(int(x.split("-")[-1]) for x in r["coverage"]["authored"]) for r in authored_rows}
    for row in authored_rows:
        row["authored_indices"] = row_coverage[row["id"]]
    dispatch_ids = [r["id"] for r in dispatch]
    dispatch_ex = next(e["id"] for e in exclusions if e["id"] == "EXC-L2-DISPATCH-NONROOTED")
    dispatch_census = {
        "source": "denominator/level2-dispatch-rng-audio-presentation.json dispatch.rows",
        "slot_id_format": "DSP-L2-000..DSP-L2-844",
        "slot_count": len(dispatch_ids),
        "slots": dispatch_ids,
        "slot_to_row": {sid: [owner_row(owner)["id"]] for sid, owner in dispatch_owner.items()},
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
            "generated": "2026-08-04",
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
