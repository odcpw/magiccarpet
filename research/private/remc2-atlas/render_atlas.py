#!/usr/bin/env python3
"""Render readable private matrices and graphs from the remc2 source atlas."""

from __future__ import annotations

import argparse
import csv
import json
import os
import re
import sqlite3
from collections import Counter, defaultdict
from pathlib import Path
from typing import Iterable, Mapping, Sequence


PRIVATE_MODE = 0o600


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--database",
        type=Path,
        default=Path(__file__).with_name("generated") / "atlas.sqlite",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path(__file__).with_name("generated"),
    )
    return parser.parse_args()


def primary_subsystem(file_name: str, line: int, name: str) -> str:
    path = file_name.lower()
    lowered = name.lower()

    if path.endswith("eventsfunctions.cpp"):
        if 31275 <= line <= 31885:
            return "frame_and_level_scheduler"
        if 37536 <= line <= 38093 or 59658 <= line <= 60912:
            return "player_flight_and_actions"
        if 39936 <= line <= 40193:
            return "entity_pool_and_dispatch"
        if 40197 <= line <= 40283:
            return "camera"
        if any(token in lowered for token in ("spell", "fireball")):
            return "spells_and_projectiles"
        if any(token in lowered for token in ("terrain", "height", "cave")):
            return "terrain_and_world_mutation"
        if any(token in lowered for token in ("castle", "mana", "balloon")):
            return "mana_castle_and_economy"
        if any(token in lowered for token in ("objective", "stage", "levelend")):
            return "objectives_and_progression"
        if any(token in lowered for token in ("sound", "music")):
            return "audio_and_music"
        if any(token in lowered for token in ("draw", "render", "bitmap")):
            return "world_and_hud_rendering"
        return "entity_ai_and_gameplay"

    rules: Sequence[tuple[tuple[str, ...], str]] = (
        (("playerinput.cpp", "player.cpp"), "player_flight_and_actions"),
        (("terrain.cpp", "basicterrain.cpp"), "terrain_and_world_mutation"),
        (("spells.cpp",), "spells_and_projectiles"),
        (("network.cpp", "port_net.cpp"), "network"),
        (("inputrecorder.cpp",), "replay_and_regression"),
        (
            ("compare.cpp", "comparemaps.cpp", "statemonitor.cpp", "renderertests.cpp"),
            "replay_and_regression",
        ),
        (("sound.cpp", "port_sdl_sound.cpp", "xmi", "adlmidi"), "audio_and_music"),
        (
            (
                "gamerender",
                "graphics.cpp",
                "projectionpolygon.cpp",
                "viewport.cpp",
                "texturemaps.cpp",
                "screenbuff.cpp",
                "gamebitmap.cpp",
                "stb_impl.cpp",
            ),
            "world_and_hud_rendering",
        ),
        (
            ("gameui.cpp", "menusandintros.cpp", "printmessages.cpp", "animation.cpp"),
            "menus_hud_and_frontend",
        ),
        (("events.cpp", "eventdispatcher.cpp"), "entity_pool_and_dispatch"),
        (
            (
                "level.cpp",
                "levelinit.cpp",
                "readanddecompress.cpp",
                "datafile",
                "findfirst",
                "fcaseopen.cpp",
                "port_filesystem.cpp",
            ),
            "level_data_and_persistence",
        ),
        (
            ("commandlineparser.cpp", "config.cpp", "read_config.cpp"),
            "configuration",
        ),
        (
            ("port_sdl_joystick.cpp", "port_sdl_vga_mouse.cpp"),
            "host_input_and_display",
        ),
        (
            ("port_time.cpp", "renderthread.cpp"),
            "host_timing_and_threads",
        ),
        (
            (
                "remc2.cpp",
                "sub_main.cpp",
                "basic.cpp",
                "basicmini.cpp",
                "engine_support.cpp",
                "engine_support_converts.cpp",
                "stdafx.cpp",
            ),
            "process_lifecycle_and_compatibility",
        ),
        (("maths.cpp",), "math_support"),
        (("/portability/", "port_outputs.cpp", "port_show_perifery.cpp"), "host_platform"),
        (("/utilities/",), "data_and_diagnostics_utilities"),
    )
    for fragments, subsystem in rules:
        if any(fragment in path for fragment in fragments):
            return subsystem
    return "recovered_types_tables_and_unclassified"


def state_region(target_name: str) -> str:
    lowered = target_name.lower()
    exact_rules = (
        ("maptypemap", "T0_terrain_type"),
        ("mapheightmap", "T1_terrain_height"),
        ("mapshading", "T2_terrain_shading"),
        ("mapangle", "T3_terrain_angle_water"),
        ("mapentityindex", "T4_terrain_entity_index"),
    )
    for token, region in exact_rules:
        if token in lowered:
            return region
    if any(token in lowered for token in ("cave", "ceiling")):
        return "T5_cave_secondary_terrain"
    if any(
        token in lowered
        for token in ("entities_", "entityarray", "entityindex", "str_0x6e8e")
    ):
        return "S3_entity_pool_and_entity_state"
    if any(token in lowered for token in ("input", "playeraction")):
        return "S2_player_input_commands"
    if any(
        token in lowered
        for token in ("player", "camera", "predictedaxis", "viewangle")
    ):
        return "S1_players_and_camera"
    if any(
        token in lowered
        for token in ("levelindex", "objective", "stage", "levelstate")
    ):
        return "S4_level_objectives_and_progression"
    if any(token in lowered for token in ("sound", "music", "xmi", "volume")):
        return "A0_audio_music_and_timer"
    if any(token in lowered for token in ("network", "socket", "packet")):
        return "N0_network"
    if any(
        token in lowered
        for token in ("viewport", "screen", "palette", "texture", "bitmap", "render")
    ):
        return "P0_presentation"
    if any(token in lowered for token in ("file", "path", "directory", "save")):
        return "F0_files_and_loaded_data"
    if any(token in lowered for token in ("config", "commandline", "resolution")):
        return "C0_configuration"
    return "U0_unclassified_or_shared_runtime"


def write_csv(path: Path, header: Sequence[str], rows: Iterable[Sequence[object]]) -> None:
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.writer(handle)
        writer.writerow(header)
        writer.writerows(rows)
    path.chmod(PRIVATE_MODE)


def scalar_counts(
    connection: sqlite3.Connection, table: str, key: str
) -> Mapping[str, int]:
    return {
        row[0]: row[1]
        for row in connection.execute(
            f"SELECT {key}, COUNT(*) FROM {table} GROUP BY {key}"
        )
    }


def access_counts(
    connection: sqlite3.Connection,
) -> Mapping[str, Counter[str]]:
    result: dict[str, Counter[str]] = defaultdict(Counter)
    for function_usr, access_kind, count in connection.execute(
        """
        SELECT function_usr, access_kind, COUNT(*)
        FROM state_accesses
        GROUP BY function_usr, access_kind
        """
    ):
        result[function_usr][access_kind] = count
    return result


def safe_id(value: str) -> str:
    return re.sub(r"[^a-zA-Z0-9_]", "_", value)


def quote_dot(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def main() -> int:
    os.umask(0o077)
    args = parse_args()
    database = args.database.resolve()
    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True, mode=0o700)
    output_dir.chmod(0o700)

    connection = sqlite3.connect(database)
    connection.row_factory = sqlite3.Row
    functions = list(
        connection.execute(
            """
            SELECT usr, name, qualified_name, kind, return_type, storage_class,
                   file, line, column, is_definition, is_inline
            FROM functions
            ORDER BY file, line, column, qualified_name
            """
        )
    )
    function_by_usr = {row["usr"]: row for row in functions}
    subsystem_by_usr = {
        row["usr"]: primary_subsystem(row["file"], row["line"], row["name"])
        for row in functions
    }

    calls_out = scalar_counts(connection, "calls", "caller_usr")
    calls_in = scalar_counts(connection, "calls", "callee_usr")
    literal_counts = scalar_counts(connection, "literals", "function_usr")
    bulk_counts = scalar_counts(connection, "bulk_operations", "function_usr")
    parameter_counts = scalar_counts(connection, "parameters", "function_usr")
    per_function_access = access_counts(connection)

    function_rows = []
    for row in functions:
        usr = row["usr"]
        access = per_function_access.get(usr, Counter())
        function_rows.append(
            (
                usr,
                row["qualified_name"],
                row["name"],
                subsystem_by_usr[usr],
                row["file"],
                row["line"],
                row["kind"],
                row["return_type"],
                parameter_counts.get(usr, 0),
                calls_out.get(usr, 0),
                calls_in.get(usr, 0),
                access.get("read", 0),
                access.get("write", 0),
                access.get("readwrite", 0),
                access.get("address_escape", 0)
                + access.get("argument_or_escape", 0),
                literal_counts.get(usr, 0),
                bulk_counts.get(usr, 0),
                row["is_definition"],
            )
        )
    write_csv(
        output_dir / "function-matrix.csv",
        (
            "function_usr",
            "qualified_name",
            "name",
            "primary_subsystem",
            "file",
            "line",
            "kind",
            "return_type",
            "parameter_count",
            "outgoing_call_edges",
            "incoming_call_edges",
            "read_edges",
            "write_edges",
            "readwrite_edges",
            "possible_pointer_or_escape_edges",
            "literal_count",
            "bulk_operation_count",
            "is_definition",
        ),
        function_rows,
    )

    call_rows = []
    subsystem_calls: Counter[tuple[str, str, str]] = Counter()
    for row in connection.execute(
        """
        SELECT caller_usr, callee_usr, callee_name, edge_kind, confidence,
               file, line, column
        FROM calls
        ORDER BY file, line, column, caller_usr, callee_name
        """
    ):
        caller = function_by_usr.get(row["caller_usr"])
        callee = function_by_usr.get(row["callee_usr"])
        caller_subsystem = subsystem_by_usr.get(
            row["caller_usr"], "unknown_caller"
        )
        callee_subsystem = subsystem_by_usr.get(
            row["callee_usr"],
            (
                "unresolved_or_indirect"
                if row["edge_kind"] == "calls_indirect_or_unresolved"
                else "external_library_or_host"
            ),
        )
        subsystem_calls[
            (caller_subsystem, callee_subsystem, row["edge_kind"])
        ] += 1
        call_rows.append(
            (
                row["caller_usr"],
                caller["qualified_name"] if caller else "",
                caller_subsystem,
                row["callee_usr"],
                callee["qualified_name"] if callee else row["callee_name"],
                callee_subsystem,
                row["edge_kind"],
                row["confidence"],
                row["file"],
                row["line"],
                row["column"],
            )
        )
    write_csv(
        output_dir / "call-edges.csv",
        (
            "caller_usr",
            "caller",
            "caller_subsystem",
            "callee_usr",
            "callee",
            "callee_subsystem",
            "edge_kind",
            "confidence",
            "file",
            "line",
            "column",
        ),
        call_rows,
    )
    write_csv(
        output_dir / "subsystem-call-matrix.csv",
        ("caller_subsystem", "callee_subsystem", "edge_kind", "edge_count"),
        (
            (*key, count)
            for key, count in sorted(
                subsystem_calls.items(),
                key=lambda item: (-item[1], item[0]),
            )
        ),
    )

    state_rows = []
    subsystem_state: Counter[tuple[str, str, str, str]] = Counter()
    target_summary: dict[
        tuple[str, str, str], Counter[str] | set[str]
    ] = {}
    for row in connection.execute(
        """
        SELECT function_usr, target_usr, target_kind, target_name,
               access_kind, confidence, file, line, column
        FROM state_accesses
        ORDER BY file, line, column, function_usr, target_name
        """
    ):
        function = function_by_usr.get(row["function_usr"])
        subsystem = subsystem_by_usr.get(
            row["function_usr"], "unknown_function"
        )
        region = state_region(row["target_name"])
        subsystem_state[
            (subsystem, region, row["access_kind"], row["confidence"])
        ] += 1
        key = (row["target_usr"], row["target_kind"], row["target_name"])
        if key not in target_summary:
            target_summary[key] = {
                "accesses": Counter(),
                "functions": set(),
                "subsystems": set(),
            }
        target_summary[key]["accesses"][row["access_kind"]] += 1
        target_summary[key]["functions"].add(row["function_usr"])
        target_summary[key]["subsystems"].add(subsystem)
        state_rows.append(
            (
                row["function_usr"],
                function["qualified_name"] if function else "",
                subsystem,
                row["target_usr"],
                row["target_kind"],
                row["target_name"],
                region,
                row["access_kind"],
                row["confidence"],
                row["file"],
                row["line"],
                row["column"],
            )
        )
    write_csv(
        output_dir / "state-access-edges.csv",
        (
            "function_usr",
            "function",
            "function_subsystem",
            "target_usr",
            "target_kind",
            "target_name",
            "state_region_seed",
            "access_kind",
            "confidence",
            "file",
            "line",
            "column",
        ),
        state_rows,
    )
    write_csv(
        output_dir / "subsystem-state-matrix.csv",
        (
            "function_subsystem",
            "state_region_seed",
            "access_kind",
            "confidence",
            "edge_count",
        ),
        (
            (*key, count)
            for key, count in sorted(
                subsystem_state.items(),
                key=lambda item: (-item[1], item[0]),
            )
        ),
    )

    target_rows = []
    for key, values in target_summary.items():
        target_usr, target_kind, target_name = key
        accesses = values["accesses"]
        target_rows.append(
            (
                target_usr,
                target_kind,
                target_name,
                state_region(target_name),
                sum(accesses.values()),
                accesses.get("read", 0),
                accesses.get("write", 0),
                accesses.get("readwrite", 0),
                accesses.get("address_escape", 0)
                + accesses.get("argument_or_escape", 0),
                len(values["functions"]),
                "|".join(sorted(values["subsystems"])),
            )
        )
    target_rows.sort(key=lambda row: (-row[4], row[2], row[0]))
    write_csv(
        output_dir / "state-targets.csv",
        (
            "target_usr",
            "target_kind",
            "target_name",
            "state_region_seed",
            "access_edge_count",
            "read_edges",
            "write_edges",
            "readwrite_edges",
            "possible_pointer_or_escape_edges",
            "touching_function_count",
            "touching_subsystems",
        ),
        target_rows,
    )

    subsystem_functions = Counter(subsystem_by_usr.values())
    subsystem_accesses = Counter()
    for (subsystem, _region, _kind, _confidence), count in subsystem_state.items():
        subsystem_accesses[subsystem] += count
    subsystem_out = Counter()
    subsystem_in = Counter()
    for (source, target, _kind), count in subsystem_calls.items():
        subsystem_out[source] += count
        subsystem_in[target] += count

    dot_lines = [
        "digraph remc2_subsystems {",
        "  rankdir=LR;",
        '  graph [label="remc2 exact-build subsystem call graph", labelloc=t];',
        '  node [shape=box, style="rounded,filled", fillcolor="#eef5ff"];',
    ]
    all_subsystems = sorted(
        set(subsystem_functions)
        | {key[0] for key in subsystem_calls}
        | {key[1] for key in subsystem_calls}
    )
    for subsystem in all_subsystems:
        label = (
            f"{subsystem}\\n"
            f"{subsystem_functions.get(subsystem, 0)} functions\\n"
            f"{subsystem_accesses.get(subsystem, 0)} state edges"
        )
        dot_lines.append(
            f"  {safe_id(subsystem)} [label={quote_dot(label)}];"
        )
    aggregate_pairs: Counter[tuple[str, str]] = Counter()
    for (source, target, _kind), count in subsystem_calls.items():
        aggregate_pairs[(source, target)] += count
    for (source, target), count in sorted(
        aggregate_pairs.items(), key=lambda item: (-item[1], item[0])
    ):
        if count < 3:
            continue
        penwidth = min(8.0, 1.0 + count ** 0.35)
        dot_lines.append(
            f"  {safe_id(source)} -> {safe_id(target)} "
            f"[label={quote_dot(str(count))}, penwidth={penwidth:.2f}];"
        )
    dot_lines.append("}")
    dot_path = output_dir / "subsystem-call-graph.dot"
    dot_path.write_text("\n".join(dot_lines) + "\n", encoding="utf-8")
    dot_path.chmod(PRIVATE_MODE)

    coverage_path = output_dir / "coverage.json"
    coverage = json.loads(coverage_path.read_text(encoding="utf-8"))
    top_hubs = sorted(
        function_rows,
        key=lambda row: (-(row[9] + row[10] + row[11] + row[12] + row[13]), row[1]),
    )[:20]

    markdown = [
        "# remc2 exact-build source atlas overview",
        "",
        "Classification: `PRIVATE_ANALYSIS`",
        "",
        "## Coverage",
        "",
        f"- Claim state: `{coverage['claim_state']}`",
        (
            "- Build translation units: "
            f"{coverage['compiled_translation_units_parsed_without_errors']}/"
            f"{coverage['compiled_translation_units_denominator']} parsed without errors"
        ),
        (
            "- Present but unbuilt translation units: "
            f"{coverage['present_unbuilt_translation_units']}"
        ),
        (
            "- Unresolved or indirect call edges: "
            f"{coverage['unresolved_direct_or_indirect_call_edges']}"
        ),
        (
            "- Possible pointer/argument state edges: "
            f"{coverage['possible_pointer_or_argument_access_edges']}"
        ),
        "",
        "The CSV files are sparse matrices. They preserve exact source locations and",
        "confidence labels; the subsystem and state-region assignments are navigation",
        "seeds, not semantic proof.",
        "",
        "## Subsystem census",
        "",
        "| Subsystem | Functions | Outgoing call edges | Incoming call edges | State edges |",
        "|---|---:|---:|---:|---:|",
    ]
    for subsystem, count in sorted(
        subsystem_functions.items(), key=lambda item: (-item[1], item[0])
    ):
        markdown.append(
            f"| `{subsystem}` | {count} | {subsystem_out[subsystem]} | "
            f"{subsystem_in[subsystem]} | {subsystem_accesses[subsystem]} |"
        )

    markdown.extend(
        [
            "",
            "## Highest-connectivity functions",
            "",
            "| Function | Subsystem | Source | Calls out | Calls in | Typed state edges |",
            "|---|---|---|---:|---:|---:|",
        ]
    )
    for row in top_hubs:
        typed_edges = row[11] + row[12] + row[13]
        markdown.append(
            f"| `{row[1]}` | `{row[3]}` | `{row[4]}:{row[5]}` | "
            f"{row[9]} | {row[10]} | {typed_edges} |"
        )

    markdown.extend(
        [
            "",
            "## Mechanical outputs",
            "",
            "- `function-matrix.csv`: one row per recovered function.",
            "- `call-edges.csv`: sparse function-to-function/reference graph.",
            "- `state-access-edges.csv`: sparse function-to-field/global graph.",
            "- `state-targets.csv`: state targets ranked by fan-in and access kind.",
            "- `subsystem-call-matrix.csv`: compressed subsystem graph.",
            "- `subsystem-state-matrix.csv`: subsystem-to-state-region matrix.",
            "- `subsystem-call-graph.dot` and `.svg`: readable system overview.",
            "",
            "## Interpretation boundary",
            "",
            "This is an excellent whole-program navigation map, not a declaration that",
            "every literal is a gameplay parameter or every static edge executes.",
            "Loaded-table lineage, indirect dispatch, aliases, and a small number of",
            "consequential runtime questions remain explicit follow-on work.",
        ]
    )
    overview_path = output_dir / "atlas-overview.md"
    overview_path.write_text("\n".join(markdown) + "\n", encoding="utf-8")
    overview_path.chmod(PRIVATE_MODE)
    connection.close()

    print(
        json.dumps(
            {
                "functions": len(function_rows),
                "call_edges": len(call_rows),
                "state_access_edges": len(state_rows),
                "state_targets": len(target_rows),
                "subsystems": len(subsystem_functions),
                "output_dir": str(output_dir),
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
