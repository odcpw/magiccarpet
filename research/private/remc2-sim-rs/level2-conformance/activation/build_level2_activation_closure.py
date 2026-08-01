#!/usr/bin/env python3
"""Derive the Level 2 activation/producer closure from the pinned remc2 source.

The authored/reset census is the data boundary for this report.  The control
flow claims are checked against source text from the pinned remc2 commit with
``git show``; a dirty upstream worktree is therefore never used as the spec.
The resulting JSON is intentionally a derivation, not a hand-maintained story
about which groups happen to look interesting.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from collections import Counter, defaultdict, deque
from pathlib import Path
from typing import Iterable


COMMIT = "6806c7204d17220ecc0430759f784aea8ee2ddb2"
ROOT = Path(__file__).resolve().parents[5]
CENSUS = ROOT / "research/private/remc2-sim-rs/level2-conformance/census/level2-authored-reset-census.json"
OUTPUT = ROOT / "research/private/remc2-sim-rs/level2-conformance/activation/level2-activation-closure.json"
UPSTREAM = ROOT / "upstream/magic-carpet-2-hd"

TARGET_CLASSES: tuple[tuple[int, int], ...] = (
    (5, 2), (5, 3), (5, 4), (5, 9), (5, 14),
    (10, 1), (10, 6), (10, 13), (10, 17), (10, 22), (10, 39),
    (10, 45), (10, 54), (10, 58), (10, 59), (10, 76),
    (11, 0), (11, 1), (11, 12), (11, 32),
    (14, 5), (15, 6), (15, 11),
)

TARGET_BRANCHES: dict[tuple[int, int], tuple[str, str]] = {
    **{(5, model): ("class5.generic", "case 5:\n\t\tif (indexx->model_0x40_64 == 0x16)") for model in (2, 3, 4, 9, 14)},
    (5, 9): ("class5.model9_sub12100_flag", "sub_12100(entity, v3x, v3x->model_0x40_64 == 0x9)"),
    **{(10, model): ("class10.early_generic", "if (v4 < 0x22u)") for model in (1, 6, 13)},
    (10, 17): ("class10.spell_path", "v3x->subSpellIndex_0x2A_42 ="),
    (10, 22): ("class10.coordinate_path", "if (v4 <= 0x22)"),
    (10, 39): ("class10.generic_v4_lt_0x43", "if (v4 < 0x43u)"),
    (10, 45): ("class10.model45_par2_producer", "if (v4 == 0x2D)"),
    (10, 54): ("class10.model54_maxlife", "else if (v4 <= 0x36u)"),
    (10, 58): ("class10.generic_v4_lt_0x3d", "if (v4 < 0x3Du)"),
    (10, 59): ("class10.generic_v4_lt_0x3d", "if (v4 < 0x3Du)"),
    (10, 76): ("class10.spell_path", "v3x->subSpellIndex_0x2A_42 ="),
    **{(11, model): ("class11.stageTag", "case 0xB:\n\t\tindexx->id_0x1A_26 = entity->stageTag_12;") for model in (0, 1, 12, 32)},
    (14, 5): ("class14.default_model5", "case 0xE:\n\t\tv10 = indexx->model_0x40_64;"),
    **{(15, model): ("class15.action_stageTag", "case 0xF:\n\t\tindexx->actionIndex_0x45_69 += entity->stageTag_12;") for model in (6, 11)},
}


def fail(message: str) -> None:
    raise ValueError(message)


def source_file(path: str) -> str:
    try:
        result = subprocess.run(
            ["git", "-C", str(UPSTREAM), "show", f"{COMMIT}:{path}"],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as exc:
        fail(f"cannot read pinned source {path}: {exc}")
    return result.stdout


def source_anchor(path: str, symbol: str, marker: str, claim: str) -> dict:
    text = source_file(path)
    if marker not in text:
        fail(f"pinned source marker missing for {symbol}: {marker!r}")
    line = text[: text.index(marker)].count("\n") + 1
    return {
        "path": path,
        "symbol": symbol,
        "line": line,
        "marker": marker,
        "claim": claim,
        "source_sha256": hashlib.sha256(text.encode()).hexdigest(),
    }


def required_sources() -> list[dict]:
    return [
        source_anchor(
            "remc2/engine/Level.cpp",
            "sub_49270_generate_level_features",
            "GenerateEvents_49290(terrain, 1, 640, 480);",
            "level construction runs the authored-event preprocessor before root activation",
        ),
        source_anchor(
            "remc2/engine/Events.cpp",
            "GenerateEvents_49290",
            "if (terrain->entity_0x30311[ix].DisId == -1 && terrain->entity_0x30311[ix].type_0x30311 == 0x000a && terrain->entity_0x30311[ix].subtype_0x30311 == 0x52)",
            "the authored-event preprocessor scans DisId=-1 rows before group activation",
        ),
        source_anchor(
            "remc2/engine/Events.cpp",
            "GenerateEvents_49290_model45",
            "if (terrain->entity_0x30311[nx].DisId == -1 && terrain->entity_0x30311[nx].type_0x30311 == 0x000a && terrain->entity_0x30311[nx].subtype_0x30311 == 0x002d && str_D93C0_bldgprmbuffer[terrain->entity_0x30311[nx].par1_14].byte_2 & 0x10)",
            "sentinel class10/model45 rows are prepared in the building-flag branch",
        ),
        source_anchor(
            "remc2/engine/Events.cpp",
            "GenerateEvents_49290_model45_clear",
            "if (terrain->entity_0x30311[ox].DisId == -1 && terrain->entity_0x30311[ox].type_0x30311 == 0x000a && terrain->entity_0x30311[ox].subtype_0x30311 == 0x002d && !(str_D93C0_bldgprmbuffer[terrain->entity_0x30311[ox].par1_14].byte_2 & 0x10))",
            "sentinel class10/model45 rows are prepared in the complementary building-flag branch",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "sub_4A1E0",
            "void sub_4A1E0(int a1, char a2)",
            "group activation scans authored rows and dispatches rows whose type is nonzero and DisId matches",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "LevelInitGame_56A30_root",
            "sub_4A1E0(0, 1);",
            "LevelInitGame_56A30 seeds the closure with group zero after initialization",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "LevelInitGame_56A30_definition",
            "void LevelInitGame_56A30(int16_t level, std::string customLevelPath)",
            "the root call is in the production level-load function",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "sub_56D60_reload_root",
            "void sub_56D60(unsigned int a1, char a2)",
            "the reload path has its own group-zero root call",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "sub_4A1E0_group_filter",
            "D41A0_0.terrain_2FECE.entity_0x30311[iy].type_0x30311 && D41A0_0.terrain_2FECE.entity_0x30311[iy].DisId == a1",
            "the dispatcher is mechanically gated by nonzero type and exact activation group",
        ),
        source_anchor(
            "remc2/engine/Events.cpp",
            "IfSubtypeCallCreatingManaSphere_4A190",
            "type_entity_0x6E8E* IfSubtypeCallCreatingManaSphere_4A190(axis_3d* position, int type, int subtype)",
            "constructor dispatch resolves each authored kind/subtype before sub_4A310 applies class-specific fields",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "sub_4A310_class10_model45",
            "v3x->xtype_0x41_65 = entity->par2_16;",
            "class 10 model 45 stores par2 as a later group producer",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "sub_4A310_class11",
            "indexx->id_0x1A_26 = entity->stageTag_12;",
            "class 11 rows carry their next activation group in stageTag",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "AddSwitch0B_00_6F030",
            "sub_4A1E0(event->id_0x1A_26, 1);",
            "successful switch/checkpoint and terminal actions activate a producer id",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "sub_4A1E0_building_consumer",
            "if (event->xtype_0x41_65)\n\t\t\tsub_4A1E0(event->xtype_0x41_65, 1);",
            "completed class10/model45 building action consumes its par2/xtype producer edge",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "AddSwitch0B_04_6F150",
            "type_entity_0x6E8E* AddSwitch0B_04_6F150(type_entity_0x6E8E* a1x)",
            "level-end switch is a terminal producer consumer",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "sub_6F100",
            "sub_4A1E0(a1x->id_0x1A_26, 0);",
            "switch return action can conditionally reactivate a producer id",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "sub_6F0B0",
            "if (InitSwitchChainZaxisAndSound_6F850(a1x, 1))",
            "reverse switch completion conditionally activates a producer id",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "sub_6F0B0_dispatch_call",
            "if (InitSwitchChainZaxisAndSound_6F850(a1x, 1))\n\t\t{\n\t\t\tsub_4A1E0(a1x->id_0x1A_26, 0);",
            "reverse switch completion dispatches the exact producer id",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "sub_6F300",
            "PrepareEventSound_6E450(a1x - D41A0_0.struct_0x6E8E, -1, 41);",
            "interval/programmable switch completion activates a producer id",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "sub_6F300_dispatch_call",
            "PrepareEventSound_6E450(a1x - D41A0_0.struct_0x6E8E, -1, 41);\n\t\t\tsub_4A1E0(a1x->id_0x1A_26, 1);",
            "programmable switch completion dispatches the exact producer id",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "AddSwitch0B_20_6F1C0",
            "D41A0_0.struct_0x3659C[resultx].substr_3659C.ObjectiveDone_2 = 1;\n\t\t\t\tsub_4A1E0(a1x->id_0x1A_26, 1);",
            "objective completion activates the next producer id",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "sub_58F00_game_objectives",
            "void sub_58F00_game_objectives()",
            "the stage objective state machine owns Level 2 objective completion",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "InitStages_58940",
            "void InitStages_58940()",
            "stage records are copied into runtime objective slots before objective evaluation",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "sub_58F00_objective_switch",
            "switch (D41A0_0.stages_0x3654C[v3].stages_3654C_byte0)",
            "objective semantics dispatch on the initialized stage type",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "objective_type_5_fly_to_coordinate",
            "case 5:\n\t\t\t\t\t\t\tif (v3 == D41A0_0.struct_0x3659C[v0x].substr_3659C.ObjectiveText_1)",
            "objective type 5 checks player distance to the authored stage coordinate",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "objective_type_3_banish_player",
            "case 3:\n\t\t\t\t\t\t\tif (!D41A0_0.array_0x2BDE",
            "objective type 3 checks the target player's banished state",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "objective_type_7_kill_authored_row",
            "case 7:\n\t\t\t\t\t\t\tif (v3 == D41A0_0.struct_0x3659C[v0x].substr_3659C.ObjectiveText_1 && !x_D41A0_BYTEARRAY_4_struct.bytearray_38403x",
            "objective type 7 checks the authored creature/object bytearray by selector",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "objective_type_9_building_chain",
            "case 9:\n\t\t\t\t\t\t\tif (v3 == D41A0_0.struct_0x3659C[v0x].substr_3659C.ObjectiveText_1",
            "objective type 9 walks the authored building-chain selector",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "sub_58F00_level_end_state",
            "D41A0_0.struct_0x3659C[v0x].substr_3659C.IsLevelEnd_0 = v14;\n\t\t\t\t\tif (v23 || v14)",
            "the objective state machine marks the player stage complete before terminal switch processing",
        ),
        source_anchor(
            "remc2/engine/EventsFunctions.cpp",
            "sub_58F00_local_player_level_end",
            "if ((x_WORD)v24 == D41A0_0.LevelIndex_0xc)\n\t\t\t\t\t\t\tD41A0_0.byte_0x36E02 = 1;",
            "the local player's objective completion sets the level-end flag",
        ),
    ]


def direct_dispatch_calls() -> list[dict]:
    """Enumerate every direct production call into the group dispatcher.

    The pinned source has two initialization roots, one building producer,
    switch/checkpoint completions, and programmable switch variants.  Keeping
    the exact line/expression list in the artifact makes an omitted producer
    callsite visible instead of silently relying on a hand-picked subset.
    """
    path = "remc2/engine/EventsFunctions.cpp"
    text = source_file(path)
    expected = {
        28173: ("building_action", "class10.model45 building completion forwards xtype/par2"),
        39425: ("LevelInitGame_56A30", "initial Level 2 load seeds group zero"),
        39474: ("sub_56D60", "reload path seeds group zero"),
        44551: ("AddSwitch0B_00_6F030", "successful switch completion activates id"),
        44563: ("CheckpointArrived_6F070", "checkpoint arrival activates id"),
        54361: ("sub_6F100", "switch proximity return conditionally reactivates id"),
        54383: ("AddSwitch0B_04_6F150", "level-end switch activates id"),
        54412: ("AddSwitch0B_20_6F1C0", "objective stage completion activates id"),
        54463: ("sub_6F0B0", "reverse switch completion conditionally reactivates id"),
        54519: ("sub_6F300", "programmable switch interval completion activates id"),
        54537: ("sub_6F300", "programmable switch explicit completion activates id"),
    }
    calls = []
    for number, line in enumerate(text.splitlines(), 1):
        if "sub_4A1E0(" not in line or number == 32950:
            continue
        if number not in expected:
            fail(f"unexpected or unclassified direct sub_4A1E0 callsite at {path}:{number}")
        context, purpose = expected[number]
        calls.append({
            "id": f"DISPATCH-CALL-{number}",
            "path": path,
            "line": number,
            "expression": line.strip(),
            "context": context,
            "purpose": purpose,
            "source_sha256": hashlib.sha256(text.encode()).hexdigest(),
        })
    if [call["line"] for call in calls] != sorted(expected):
        fail(f"direct dispatcher callsite set changed: {[call['line'] for call in calls]}")
    return calls


def branch_evidence(kind: int, subtype: int) -> dict:
    key = (kind, subtype)
    if key not in TARGET_BRANCHES:
        fail(f"no pinned sub_4A310 branch mapping for target tuple {key}")
    branch, marker = TARGET_BRANCHES[key]
    path = "remc2/engine/EventsFunctions.cpp"
    text = source_file(path)
    start = text.index("void sub_4A310(type_entity_0x30311* entity)")
    end = text.find("\n//-----", start + 1)
    body = text[start:] if end < 0 else text[start:end]
    if marker not in body:
        fail(f"pinned sub_4A310 branch marker missing for {key}: {marker!r}")
    absolute = start + body.index(marker)
    return {
        "branch": branch,
        "marker": marker,
        "path": path,
        "line": text[:absolute].count("\n") + 1,
        "source_anchor": "sub_4A310",
    }


def load_census() -> dict:
    data = json.loads(CENSUS.read_text())
    if data.get("schema") != "mc2-level2-authored-reset-census/v1":
        fail("unexpected Level 2 census schema")
    if data.get("spec", {}).get("remc2_commit") != COMMIT:
        fail("census and closure use different remc2 commits")
    rows = data.get("authored", {}).get("rows")
    if not isinstance(rows, list) or len(rows) != 1200:
        fail("census authored row denominator changed")
    return data


def identity(row: dict) -> str:
    return row["id"]


def derive_edges(rows: list[dict], dispatch_calls: list[dict]) -> list[dict]:
    edges: list[dict] = []
    for row in rows:
        group = row["activation_group"]
        if group < 0:
            continue
        kind, subtype = row["kind"], row["subtype"]
        if kind == 11 and row["stage_tag"] > 0:
            edges.append({
                "id": f"EDGE-{identity(row)}-stage-tag",
                "producer_row": identity(row),
                "source_group": group,
                "target_group": row["stage_tag"],
                "producer": "class11.stageTag",
                "dispatch_modes": ["switch_or_objective_completion"],
                "source_anchor": "sub_4A310_class11",
                "consumer_call_ids": [call["id"] for call in dispatch_calls if call["line"] in (44551, 44563, 54383, 54412, 54519, 54537)],
            })
        if kind == 10 and subtype == 45 and row["parameter_selectors"]["par2"] > 0:
            edges.append({
                "id": f"EDGE-{identity(row)}-par2",
                "producer_row": identity(row),
                "source_group": group,
                "target_group": row["parameter_selectors"]["par2"],
                "producer": "class10.model45.par2",
                "dispatch_modes": ["building_action"],
                "source_anchor": "sub_4A310_class10_model45",
                "consumer_call_ids": ["DISPATCH-CALL-28173"],
            })
    return sorted(edges, key=lambda edge: (edge["source_group"], edge["target_group"], edge["producer_row"]))


def rooted_groups(edges: list[dict]) -> list[int]:
    adjacency: dict[int, set[int]] = defaultdict(set)
    for edge in edges:
        adjacency[edge["source_group"]].add(edge["target_group"])
    seen = {0}
    queue = deque([0])
    while queue:
        source = queue.popleft()
        for target in sorted(adjacency[source]):
            if target not in seen:
                seen.add(target)
                queue.append(target)
    return sorted(seen)


def classify_rows(rows: list[dict], rooted: set[int], edges: list[dict]) -> list[dict]:
    classified = []
    incoming = defaultdict(list)
    for edge in edges:
        incoming[edge["target_group"]].append(edge["id"])
    for row in rows:
        state = row["state"]
        group = row["activation_group"]
        if state == "null":
            closure, route, proof = (
                "deliberate_no_call",
                "none",
                "record bytes are all zero; dispatcher requires a nonzero type",
            )
            proof_sources = ["sub_4A1E0_group_filter"]
        elif row["kind"] == 0:
            closure, route, proof = (
                "mechanically_excluded",
                "none",
                f"authored row kind/type=0 (subtype={row['subtype']}, group={group}); sub_4A1E0's nonzero-type gate cannot dispatch it",
            )
            proof_sources = ["sub_4A1E0_group_filter"]
        elif state == "unassigned_sentinel":
            closure, route, proof = (
                "conditionally_reachable",
                "GenerateEvents_49290_model45",
                f"DisId=-1 class10/model45 row par1={row['parameter_selectors']['par1']} is accepted by exactly one of the pinned model45 building-flag branches (&0x10 or its complement)",
            )
            proof_sources = ["GenerateEvents_49290_model45", "GenerateEvents_49290_model45_clear"]
        elif group == 0:
            closure, route, proof = (
                "reachable",
                "sub_4A1E0(0,1)",
                "LevelInitGame_56A30 reaches the root group activation after feature generation",
            )
            proof_sources = ["LevelInitGame_56A30_root", "sub_4A1E0_group_filter"]
        elif group in rooted:
            closure, route, proof = (
                "conditionally_reachable",
                f"producer_group_{group}",
                "a rooted producer edge reaches this activation group; switch/objective/building conditions still gate execution",
            )
            proof_sources = ["sub_4A1E0_group_filter"] + sorted({edge["source_anchor"] for edge in edges if edge["target_group"] == group})
        else:
            closure, route, proof = (
                "mechanically_excluded",
                "none",
                "no rooted producer edge reaches this activation group",
            )
            proof_sources = ["sub_4A1E0_group_filter"]
        if closure == "reachable":
            owner = "root_group_zero_dispatch"
        elif closure == "conditionally_reachable" and route == "GenerateEvents_49290_model45":
            owner = "GenerateEvents_49290_model45_preprocess"
        elif closure == "conditionally_reachable":
            owner = f"group_{group}_dispatch"
        else:
            owner = "none"
        if row["kind"] == 10 and row["subtype"] == 45:
            if row["parameter_selectors"]["par2"]:
                producer_effect = "par2_group_producer"
            else:
                producer_effect = "no_op_guarded_xtype_zero"
                if "sub_4A1E0_building_consumer" not in proof_sources:
                    proof_sources.append("sub_4A1E0_building_consumer")
        else:
            producer_effect = "none"
        classified.append({
            "authored_id": identity(row),
            "index": row["index"],
            "kind": row["kind"],
            "subtype": row["subtype"],
            "activation_group": group,
            "closure_state": closure,
            "planned_owner": owner,
            "route": route,
            "producer_effect": producer_effect,
            "proof": proof,
            "proof_source_anchors": sorted(set(proof_sources)),
            "incoming_edge_ids": sorted(incoming.get(group, [])) if owner.startswith("group_") else [],
        })
    return classified


def target_class_report(rows: list[dict], classified: list[dict]) -> list[dict]:
    by_tuple: dict[tuple[int, int], list[dict]] = defaultdict(list)
    by_id = {entry["authored_id"]: entry for entry in classified}
    for row in rows:
        if (row["kind"], row["subtype"]) in TARGET_CLASSES:
            by_tuple[(row["kind"], row["subtype"])].append({
                "authored_id": row["id"],
                "index": row["index"],
                "activation_group": row["activation_group"],
                "closure_state": by_id[row["id"]]["closure_state"],
            })
    report = []
    for kind, subtype in TARGET_CLASSES:
        members = by_tuple[(kind, subtype)]
        report.append({
            "class": kind,
            "subtype": subtype,
            "authored_count": len(members),
            "rows": members,
            "producer_row_ids": [entry["authored_id"] for entry in members if by_id[entry["authored_id"]]["producer_effect"] == "par2_group_producer"],
            "no_op_row_ids": [entry["authored_id"] for entry in members if by_id[entry["authored_id"]]["producer_effect"] == "no_op_guarded_xtype_zero"],
            "dispatch_anchor": "sub_4A310",
            "branch_evidence": branch_evidence(kind, subtype),
            "constructor_anchor": "IfSubtypeCallCreatingManaSphere_4A190",
            "constructor_dispatch": "sub_4A310",
            "resolved": bool(members),
        })
    return report


def objectives(census: dict) -> list[dict]:
    authored = [stage for stage in census["stages"] if stage["stage_index"] >= 0]
    runtime = {row["index"]: row for row in census["runtime"]["stages"]["rows"]}
    rows = {row["index"]: row for row in census["authored"]["rows"]}
    result = []
    for stage in authored:
        index, kind, selector = stage["index"], stage["stage_index"], stage["objective_selector"]
        runtime_fields = runtime[index]["fields"]
        if kind == 5:
            target = {"x": stage["axis"]["x"], "y": stage["axis"]["y"]}
            identity_text = f"stage_axis({target['x']},{target['y']})"
            anchor = "objective_type_5_fly_to_coordinate"
        elif kind == 3:
            target = {"kind": "player", "selector": selector, "runtime_relation": runtime_fields["relation"]}
            identity_text = "player_state"
            anchor = "objective_type_3_banish_player"
        elif kind == 7:
            target_row = rows[selector]
            target = {"authored_id": target_row["id"], "index": selector, "kind": target_row["kind"], "subtype": target_row["subtype"], "runtime_relation": runtime_fields["relation"], "role": "Bee"}
            identity_text = target_row["id"]
            anchor = "objective_type_7_kill_authored_row"
        elif kind == 9:
            target_row = rows[selector]
            target = {"authored_id": target_row["id"], "index": selector, "chain_root": target_row["parameter_selectors"]["par1"], "runtime_relation": runtime_fields["relation"], "kind": target_row["kind"], "subtype": target_row["subtype"], "role": "building_chain_root"}
            identity_text = target_row["id"]
            anchor = "objective_type_9_building_chain"
        else:
            fail(f"unexpected active Level 2 objective type {kind}")
        result.append({
            "id": f"OBJ-L2-{index:02d}",
            "stage_record_id": stage["id"],
            "stage_index": index,
            "objective_type": kind,
            "objective_selector": selector,
            "runtime_stage_fields": runtime_fields,
            "target": target,
            "authored_identity": identity_text,
            "source_anchor": anchor,
        })
    return result


def build() -> dict:
    census = load_census()
    rows = census["authored"]["rows"]
    sources = required_sources()
    dispatch_calls = direct_dispatch_calls()
    edges = derive_edges(rows, dispatch_calls)
    rooted = rooted_groups(edges)
    classified = classify_rows(rows, set(rooted), edges)
    classes = target_class_report(rows, classified)
    group_rows: dict[int, list[str]] = defaultdict(list)
    all_group_rows: dict[int, list[str]] = defaultdict(list)
    for row in rows:
        if row["activation_group"] >= 0:
            all_group_rows[row["activation_group"]].append(row["id"])
            if row["kind"]:
                group_rows[row["activation_group"]].append(row["id"])
    group_ids = sorted({row["activation_group"] for row in rows if row["activation_group"] >= 0} | {edge["target_group"] for edge in edges})
    groups = []
    for group in group_ids:
        groups.append({
            "group": group,
            "authored_row_count": len(all_group_rows.get(group, [])),
            "dispatchable_row_count": len(group_rows.get(group, [])),
            "authored_ids": sorted(all_group_rows.get(group, [])),
            "root_status": "root" if group == 0 else "conditionally_reachable" if group in rooted else "mechanically_excluded",
            "execution_status": "root" if group == 0 else "conditionally_reachable" if group in rooted else "mechanically_excluded",
            "incoming_edges": [edge["id"] for edge in edges if edge["target_group"] == group],
            "outgoing_edges": [edge["id"] for edge in edges if edge["source_group"] == group],
            "consumer_status": "authored_rows" if group_rows.get(group) else "mechanically_noop" if all_group_rows.get(group) or group in rooted else "producer_target_without_authored_rows",
        })
    state_counts = Counter(entry["closure_state"] for entry in classified)
    unresolved = [entry for entry in classes if not entry["resolved"]]
    return {
        "schema": "mc2-level2-activation-closure/v1",
        "spec": {"remc2_commit": COMMIT, "census": str(CENSUS.relative_to(ROOT)), "scope": "Level 2 authored activation, producers, objectives, and terminal closure"},
        "root": {"activation_group": 0, "source_anchor": "sub_4A1E0", "call_site": "LevelInitGame_56A30", "rooted_groups": rooted},
        "sources": sources,
        "dispatch_calls": dispatch_calls,
        "producer_edges": edges,
        "groups": groups,
        "authored_rows": {"count": len(classified), "closure_state_counts": dict(sorted(state_counts.items())), "rows": classified},
        "target_classes": classes,
        "objectives": objectives(census),
        "terminal": {
            "group_30": {"status": "mechanically_excluded", "authored_ids": ["AUT-L2-0443", "AUT-L2-0444", "AUT-L2-0445", "AUT-L2-0446", "AUT-L2-0447"], "proof": "five authored group-30 rows are kind/type zero; sub_4A1E0's nonzero-type gate makes the group mechanically no-op"},
            "group_32": {"execution_status": "conditionally_reachable", "consumer_status": "mechanically_noop", "incoming_edge": "EDGE-AUT-L2-0209-stage-tag", "proof": "class11 stageTag producer row AUT-L2-0209 targets 32, but no authored consumer row has activation_group 32"},
            "level_end": {
                "source_anchor": "AddSwitch0B_04_6F150",
                "dispatch_call_id": "DISPATCH-CALL-54383",
                "objective_state_anchors": ["sub_58F00_level_end_state", "sub_58F00_local_player_level_end"],
                "proof": "sub_58F00 marks IsLevelEnd and the local-player level-end flag; AddSwitch0B_04_6F150 then calls sub_4A1E0(id,1) for the terminal producer path",
            },
        },
        "unresolved": unresolved,
    }


def canonical(value: object) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def validate(path: Path) -> None:
    expected = build()
    actual = json.loads(path.read_text())
    if canonical(actual) != canonical(expected):
        fail(f"activation closure is stale or hand-edited: {path}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true", help="write the derived closure JSON")
    parser.add_argument("--check", action="store_true", help="rederive and compare the existing closure JSON")
    parser.add_argument("--output", type=Path, default=OUTPUT)
    args = parser.parse_args()
    if args.write:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(canonical(build()))
        return 0
    if args.check:
        validate(args.output)
        return 0
    parser.error("choose --write or --check")


if __name__ == "__main__":
    main()
