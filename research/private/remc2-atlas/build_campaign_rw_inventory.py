#!/usr/bin/env python3
"""Build a deterministic read/write/RNG/emission inventory for Level 2.

The inventory is intentionally source-backed and conservative. It follows the
ordinary Rust Level 2 source pass, joins each transition to the pinned remc2
scheduling anchors, and records unresolved packed or indirect accesses as open
questions instead of inventing field semantics.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


ROOT = Path(__file__).resolve().parents[3]
RUST_ROOT = ROOT / "research/private/remc2-sim-rs/crates/sim-core/src"
SOURCE_ROOT = ROOT / "upstream/magic-carpet-2-hd/remc2"
DEFAULT_ARTIFACT_DIR = ROOT / "research/private/remc2-atlas/artifacts"
DEFAULT_SUMMARY = ROOT / "docs/preservation-campaign-rw-inventory.md"
SCHEMA_VERSION = "campaign-rw-inventory-v1"

CLASSIFICATIONS = {
    "immutable_fact",
    "session_state",
    "derived_index",
    "presentation_only",
}

RUST_FILE_RE = re.compile(
    r"(?m)^[ \t]*(?:(?:pub(?:\([^)]*\))?)\s+)?"
    r"(?:unsafe\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)"
    r"(?:<[^{};]*>)?\s*\("
)

SOURCE_ANCHORS = {
    "outer_player_input": ["PlayerEvents_51BB0"],
    "source_dispatch": ["UpdateEntities_57730", "ApplyEvents_498A0"],
    "local_player_source_slot": [
        "UpdateEntities_57730",
        "AddPlayer03_00_5E010",
        "sub_693F0",
        "sub_6DCA0",
    ],
    "shared_pickup_source_slot": ["UpdateEntities_57730", "ApplyEvents_498A0"],
    "generic_spell_pickup_source_slot": [
        "UpdateEntities_57730",
        "NewEvent_4A050",
    ],
    "bad_stone_source_slot": ["UpdateEntities_57730", "ApplyEvents_498A0"],
    "house_source_slot": ["UpdateEntities_57730", "ApplyEvents_498A0"],
    "villager_source_slot": [
        "UpdateEntities_57730",
        "ApplyEvents_498A0",
        "sub_10C80",
    ],
    "dolmen_source_slot": ["UpdateEntities_57730", "ApplyEvents_498A0"],
    "nyphur_source_slot": ["UpdateEntities_57730", "sub_12910"],
    "goat_source_slot": ["UpdateEntities_57730", "ApplyEvents_498A0"],
    "castle_source_slot": ["UpdateEntities_57730", "ApplyEvents_498A0"],
    "dynamic_source_slot": [
        "UpdateEntities_57730",
        "ApplyEvents_498A0",
        "NewEvent_4A050",
    ],
    "fireball_source_slot": [
        "UpdateEntities_57730",
        "sub_693F0",
        "sub_6DCA0",
    ],
    "same_pass_cursor": ["ApplyEvents_498A0", "NewEvent_4A050"],
    "following_pass_cleanup": ["UpdateEntities_57730", "sub_57F20"],
    "objective_tail": ["sub_58F00_game_objectives"],
    "hud_tail": ["PresentObjective_59820", "PlayEntitySounds_6E150"],
}

SOURCE_CELL_RE = re.compile(
    r"\b(?:"
    r"x_[A-Za-z0-9_]+|dword_[A-Za-z0-9_]+|word_[A-Za-z0-9_]+|"
    r"byte_[A-Za-z0-9_]+|str_[A-Za-z0-9_]+|"
    r"map[A-Za-z0-9_]+|MSPRD|HSPR|TMAP|D41A0|"
    r"Entities_EA3E4|SPELLS_BEGIN_BUFFER_str"
    r")\b"
)

RNG_RE = re.compile(
    r"\b(?:rng|rand|random|advance_global_rng|draw|next_random|"
    r"GetRandom|Random)\w*\b",
    re.IGNORECASE,
)
EMISSION_RE = re.compile(
    r"\b(?:Level2[A-Za-z0-9_]*Event|GoatStepEvent|HouseStepEvent|"
    r"VillagerStepEffect|DispatchEvent|PrepareEventSound|"
    r"FlightEvent|PrepareSound|PlaySample|PlayEntitySounds|sound|audio)\b",
    re.IGNORECASE,
)
INDIRECT_RE = re.compile(
    r"(?:\b(?:memcpy|memmove|memset)\b|"
    r"\[[^\]]+\]\s*\(|"
    r"\(\s*\*[A-Za-z_][A-Za-z0-9_]*\s*\)|"
    r"\b(?:reinterpret_cast|static_cast|dynamic_cast)\b|"
    r"\b(?:void\s*\*|char\s*\*)\b)"
)
MUTATING_METHODS = {
    "append",
    "clear",
    "clone_from",
    "drain",
    "extend",
    "insert",
    "push",
    "remove",
    "retain",
    "resize",
    "sort",
    "sort_unstable",
    "swap",
    "truncate",
}

METHOD_PREFIXES = (
    "step_",
    "sync_",
    "rebuild_",
    "advance_",
    "activate_",
    "reclaim_",
    "append_",
    "refresh_",
    "set_",
    "finish_",
    "apply_",
    "clear_",
    "consume_",
    "spawn_",
    "dispatch_",
    "begin_",
    "synchronize_",
    "update_",
    "latch_",
    "resolve_",
    "take_",
    "free_",
    "record_",
    "reconcile_",
    "prepare_",
    "live_",
    "schedule_",
)

ROUTE_ARM_NAMES = {
    0: "shared_spell_pickup",
    1: "bad_stone",
    2: "ordinary_house",
    3: "ordinary_villager",
    4: "ordinary_dolmen",
    5: "castle",
    6: "dynamic_arena_row",
    7: "goat",
    8: "generic_spell_pickup",
    9: "local_player",
    10: "nyphur",
    11: "possession_holder",
    12: "castle_holder",
    13: "fireball_holder",
}

RUST_CELL_CLASS = {
    "profile": "immutable_fact",
    "mission": "session_state",
    "state": "session_state",
    "entities": "session_state",
    "players": "session_state",
    "runtime": "session_state",
    "context": "session_state",
    "terrain": "session_state",
    "economy": "session_state",
    "ordinary": "session_state",
    "goats": "session_state",
    "bad_stones": "session_state",
    "shared_spells": "session_state",
    "player_pending_damage": "session_state",
    "player_pending_attacker": "session_state",
    "player_life": "session_state",
    "dispatch_events": "presentation_only",
    "nyphur_dispatch_traces": "presentation_only",
    "level2_day_hud_color_phase_1": "presentation_only",
    "level2_day_hud_color_phase_2": "presentation_only",
    "level2_day_hud_hit_frame_time_pre_draw": "presentation_only",
    "level2_day_hud_player_panel_selector": "presentation_only",
    "evidence": "presentation_only",
}

DERIVED_WORDS = (
    "cache",
    "projection",
    "index",
    "cursor",
    "guidance",
    "target",
    "route",
    "activated",
    "pending",
    "deferred",
    "event_cursor",
    "source_slot",
    "owner_max_mana",
    "objective_stage",
)

PRESENTATION_WORDS = (
    "hud",
    "dispatch",
    "trace",
    "sound",
    "audio",
    "frame",
    "color_phase",
    "evidence",
)

SOURCE_IMMUTABLE_WORDS = (
    "mapterrain",
    "mapangle",
    "mapheight",
    "heightmap",
    "msprd",
    "hspr",
    "tmap",
    "spells_begin",
    "d4c48",
    "d4c52",
)

SOURCE_DERIVED_WORDS = (
    "cursor",
    "index",
    "target",
    "guidance",
    "38519",
    "d7bd6",
    "list",
    "next",
)


@dataclass(frozen=True)
class Slice:
    path: Path
    name: str
    start: int
    end: int
    start_line: int
    text: str


@dataclass(frozen=True)
class Segment:
    path: Path
    label: str
    start_line: int
    text: str


def rel(path: Path) -> str:
    return path.relative_to(ROOT).as_posix()


def line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def brace_end(text: str, open_index: int) -> int:
    depth = 0
    quote = ""
    escaped = False
    line_comment = False
    block_comment = False
    i = open_index
    while i < len(text):
        char = text[i]
        next_char = text[i + 1] if i + 1 < len(text) else ""
        if line_comment:
            if char == "\n":
                line_comment = False
            i += 1
            continue
        if block_comment:
            if char == "*" and next_char == "/":
                block_comment = False
                i += 2
            else:
                i += 1
            continue
        if quote:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == quote:
                quote = ""
            i += 1
            continue
        if char == "/" and next_char == "/":
            line_comment = True
            i += 2
            continue
        if char == "/" and next_char == "*":
            block_comment = True
            i += 2
            continue
        if char in "\"'":
            quote = char
            i += 1
            continue
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    return len(text)


def rust_functions(path: Path) -> list[Slice]:
    text = path.read_text(encoding="utf-8")
    result: list[Slice] = []
    for match in RUST_FILE_RE.finditer(text):
        open_index = text.find("{", match.end())
        if open_index < 0:
            continue
        semicolon = text.find(";", match.end(), open_index)
        if semicolon >= 0:
            continue
        end = brace_end(text, open_index)
        start = match.start()
        result.append(
            Slice(
                path=path,
                name=match.group(1),
                start=start,
                end=end,
                start_line=line_number(text, start),
                text=text[start:end],
            )
        )
    return result


def load_rust_functions() -> dict[str, list[Slice]]:
    index: dict[str, list[Slice]] = {}
    for path in sorted(RUST_ROOT.rglob("*.rs")):
        for function in rust_functions(path):
            index.setdefault(function.name, []).append(function)
    return index


def source_function(path: Path, name: str) -> Slice | None:
    text = path.read_text(encoding="utf-8", errors="replace")
    pattern = re.compile(
        rf"(?m)^[^;{{}}\n]*\b{re.escape(name)}\s*\([^;{{}}\n]*\)"
        rf"[^\n;{{}}]*\{{"
    )
    match = pattern.search(text)
    if match:
        open_index = text.find("{", match.start(), match.end())
        end = brace_end(text, open_index)
        return Slice(
            path=path,
            name=name,
            start=match.start(),
            end=end,
            start_line=line_number(text, match.start()),
            text=text[match.start():end],
        )
    for occurrence in re.finditer(rf"\b{re.escape(name)}\s*\(", text):
        tail = text[occurrence.end() : occurrence.end() + 600]
        brace = tail.find("{")
        semicolon = tail.find(";")
        if brace < 0 or (semicolon >= 0 and semicolon < brace):
            continue
        if tail[:brace].count("\n") > 3:
            continue
        open_index = occurrence.end() + brace
        start = text.rfind("\n", 0, occurrence.start()) + 1
        end = brace_end(text, open_index)
        return Slice(
            path=path,
            name=name,
            start=start,
            end=end,
            start_line=line_number(text, start),
            text=text[start:end],
        )
    return None


def source_index() -> dict[str, list[Slice]]:
    index: dict[str, list[Slice]] = {}
    paths = [
        SOURCE_ROOT / "engine/Events.cpp",
        SOURCE_ROOT / "engine/EventsFunctions.cpp",
    ]
    names = sorted(
        {
            name
            for values in SOURCE_ANCHORS.values()
            for name in values
        }
        | {
            "AddPlayer03_00_5E010",
            "sub_693F0",
            "sub_6DCA0",
            "sub_10C80",
            "sub_12910",
            "sub_57F20",
        }
    )
    for path in paths:
        for name in names:
            function = source_function(path, name)
            if function:
                index.setdefault(name, []).append(function)
    return index


def strip_line_comment(line: str) -> str:
    return line.split("//", 1)[0]


def method_calls(text: str) -> list[str]:
    calls: list[str] = []
    seen: set[str] = set()
    pattern = re.compile(
        r"\b(?:self|[A-Za-z_][A-Za-z0-9_]*)"
        r"(?:\s*\.\s*[A-Za-z_][A-Za-z0-9_]*){1,4}\s*\(\s*"
    )
    ignored = {
        "iter",
        "iter_mut",
        "get",
        "find",
        "filter",
        "map",
        "any",
        "all",
        "is_some",
        "is_none",
        "unwrap",
        "expect",
        "push",
        "clear",
        "insert",
        "remove",
        "entry",
        "or_default",
        "copied",
        "cloned",
        "collect",
        "sort_unstable_by_key",
        "into_iter",
        "chain",
        "extend",
        "is_empty",
        "len",
        "clone",
        "clone_from",
        "wrapping_add",
        "saturating_mul",
        "to_string",
    }
    for match in pattern.finditer(text):
        expression = match.group(0).split("(", 1)[0].strip()
        name = expression.split(".")[-1]
        if name in ignored or not name.startswith(METHOD_PREFIXES):
            continue
        if name not in seen:
            seen.add(name)
            calls.append(name)
    return calls


def function_segments(
    primary: Segment,
    rust_index: dict[str, list[Slice]],
    max_depth: int = 1,
) -> tuple[list[Segment], list[str]]:
    segments = [primary]
    seen = {(primary.path, primary.start_line, primary.label)}
    open_questions: list[str] = []
    frontier = [primary]
    for _ in range(max_depth):
        next_frontier: list[Segment] = []
        for segment in frontier:
            for name in method_calls(segment.text):
                candidates = rust_index.get(name, [])
                if not candidates:
                    continue
                if len(candidates) > 1:
                    open_questions.append(
                        f"Rust method {name} has {len(candidates)} definitions; "
                        "the inventory keeps all candidate bodies open."
                    )
                for candidate in candidates:
                    key = (candidate.path, candidate.start_line, candidate.name)
                    if key in seen:
                        continue
                    seen.add(key)
                    child = Segment(
                        path=candidate.path,
                        label=candidate.name,
                        start_line=candidate.start_line,
                        text=candidate.text,
                    )
                    segments.append(child)
                    next_frontier.append(child)
        frontier = next_frontier
    return segments, sorted(set(open_questions))


def classify_rust_cell(field: str) -> str:
    lower = field.lower()
    if lower in RUST_CELL_CLASS:
        return RUST_CELL_CLASS[lower]
    if any(word in lower for word in PRESENTATION_WORDS):
        return "presentation_only"
    if any(word in lower for word in DERIVED_WORDS):
        return "derived_index"
    if lower.startswith("baseline"):
        return "immutable_fact"
    return "session_state"


def classify_source_cell(cell: str) -> str | None:
    lower = cell.lower()
    if any(word in lower for word in SOURCE_IMMUTABLE_WORDS):
        return "immutable_fact"
    if any(word in lower for word in SOURCE_DERIVED_WORDS):
        return "derived_index"
    if any(word in lower for word in PRESENTATION_WORDS):
        return "presentation_only"
    if "d41a0" in lower or "entities_ea3e4" in lower:
        return "session_state"
    return None


def access_kind(code: str, token: str) -> str:
    escaped = re.escape(token)
    if re.search(
        rf"{escaped}(?:\[[^\]]+\])?\s*(?:\+=|-=|\*=|/=|&=|\|=|\^=)",
        code,
    ):
        return "read_write"
    if re.search(
        rf"{escaped}(?:\[[^\]]+\])?\s*(?:\+|-|\*|/|&|\||\^)?=(?!=)",
        code,
    ):
        return "write"
    if re.search(rf"{escaped}(?:\[[^\]]+\])?\s*(?:\+\+|--)", code):
        return "read_write"
    if re.search(rf"(?:memcpy|memmove|memset)\s*\([^)]*{escaped}", code):
        return "read_write"
    if re.search(rf"&\s*{escaped}", code):
        return "read_write"
    return "read"


def evidence(path: Path, line: int, symbol: str) -> dict[str, object]:
    return {"file": rel(path), "line": line, "symbol": symbol}


def rust_cells(
    segments: Iterable[Segment],
) -> tuple[dict[str, dict[str, object]], list[str]]:
    cells: dict[str, dict[str, object]] = {}
    opens: list[str] = []
    alias_seen: set[tuple[str, str, str]] = set()
    indirect_seen: set[tuple[str, str]] = set()
    pattern = re.compile(r"\bself(?:\.[A-Za-z_][A-Za-z0-9_]*)+")
    for segment in segments:
        for offset, raw_line in enumerate(segment.text.splitlines()):
            code = strip_line_comment(raw_line)
            if not code.strip():
                continue
            for match in pattern.finditer(code):
                original_path = match.group(0)
                path = original_path
                parts = path.split(".")
                called_method = (
                    parts[-1]
                    if len(parts) > 2
                    and re.match(r"\s*\(", code[match.end():])
                    else None
                )
                if len(parts) == 2 and re.match(r"\s*\(", code[match.end():]):
                    if (
                        parts[1].startswith(METHOD_PREFIXES)
                        or parts[1] in {"set_position", "set_parent"}
                    ):
                        continue
                if called_method is not None:
                    if called_method in MUTATING_METHODS:
                        path = ".".join(parts[:-1])
                    else:
                        path = ".".join(parts[:2])
                    parts = path.split(".")
                cell = f"rust:{path}"
                kind = access_kind(code, path)
                if called_method in MUTATING_METHODS:
                    kind = "write"
                elif called_method and (
                    called_method.startswith(METHOD_PREFIXES)
                    or called_method in {"set_position", "set_parent"}
                ):
                    kind = "read_write"
                item = cells.setdefault(
                    cell,
                    {
                        "cell": cell,
                        "access": set(),
                        "classification": classify_rust_cell(parts[1]),
                        "evidence": [],
                    },
                )
                item["access"].add(kind)
                item["evidence"].append(
                    evidence(
                        segment.path,
                        segment.start_line + offset,
                        segment.label,
                    )
                )
            if INDIRECT_RE.search(code):
                key = (rel(segment.path), segment.label)
                if key not in indirect_seen:
                    indirect_seen.add(key)
                    opens.append(
                        f"Indirect or packed Rust access at {rel(segment.path)}:"
                        f"{segment.start_line + offset} remains unresolved."
                    )
            alias_match = re.search(
                r"\b(?:session|feature|entity|row|holder|player|goat|"
                r"source|child|field)\s*\.",
                code,
            )
            if alias_match:
                alias = alias_match.group(0).strip().rstrip(".")
                key = (rel(segment.path), segment.label, alias)
                if key not in alias_seen:
                    alias_seen.add(key)
                    opens.append(
                        f"Mutable local or closure alias {alias} at "
                        f"{rel(segment.path)}:{segment.start_line + offset} "
                        "is not safely attributed to a root session cell by "
                        "static parsing."
                    )
    return cells, opens


def source_cells(
    segments: Iterable[Segment],
) -> tuple[dict[str, dict[str, object]], list[str]]:
    cells: dict[str, dict[str, object]] = {}
    opens: list[str] = []
    unknown_seen: set[str] = set()
    indirect_seen: set[tuple[str, str]] = set()
    for segment in segments:
        for offset, raw_line in enumerate(segment.text.splitlines()):
            code = strip_line_comment(raw_line)
            if not code.strip():
                continue
            for match in SOURCE_CELL_RE.finditer(code):
                token = match.group(0)
                cell = f"source:{token}"
                kind = access_kind(code, token)
                item = cells.setdefault(
                    cell,
                    {
                        "cell": cell,
                        "access": set(),
                        "classification": classify_source_cell(token),
                        "evidence": [],
                    },
                )
                item["access"].add(kind)
                item["evidence"].append(
                    evidence(
                        segment.path,
                        segment.start_line + offset,
                        segment.label,
                    )
                )
                if item["classification"] is None:
                    if cell not in unknown_seen:
                        unknown_seen.add(cell)
                        opens.append(
                            f"Source cell {cell} is visible but its storage "
                            f"class is ambiguous at {rel(segment.path)}:"
                            f"{segment.start_line + offset}."
                        )
            if INDIRECT_RE.search(code):
                key = (rel(segment.path), segment.label)
                if key not in indirect_seen:
                    indirect_seen.add(key)
                    opens.append(
                        f"Indirect or packed source access at {rel(segment.path)}:"
                        f"{segment.start_line + offset} remains unresolved."
                    )
    return cells, opens


def merge_cells(
    rust: dict[str, dict[str, object]],
    source: dict[str, dict[str, object]],
) -> list[dict[str, object]]:
    merged = {**rust, **source}
    output: list[dict[str, object]] = []
    for key in sorted(merged):
        item = merged[key]
        item["access"] = sorted(item["access"])
        item["evidence"] = sorted(
            {
                (entry["file"], entry["line"], entry["symbol"]): entry
                for entry in item["evidence"]
            }.values(),
            key=lambda entry: (entry["file"], entry["line"], entry["symbol"]),
        )
        if item["classification"] not in CLASSIFICATIONS:
            item["classification"] = None
            item["open_questions"] = [
                f"Classify {key} from a pinned runtime trace or a narrower "
                "packed-structure lineage before treating it as authoritative."
            ]
        output.append(item)
    return output


def ordered_events(
    segments: Iterable[Segment],
    matcher: re.Pattern[str],
    label: str,
) -> list[dict[str, object]]:
    result: list[dict[str, object]] = []
    seen: set[tuple[str, int, str]] = set()
    for segment in segments:
        for offset, raw_line in enumerate(segment.text.splitlines()):
            code = strip_line_comment(raw_line).strip()
            if not code or not matcher.search(code):
                continue
            line = segment.start_line + offset
            key = (rel(segment.path), line, code)
            if key in seen:
                continue
            seen.add(key)
            result.append(
                {
                    "ordinal": len(result),
                    "expression": code,
                    "evidence": evidence(segment.path, line, segment.label),
                    "kind": label,
                }
            )
    for ordinal, item in enumerate(result):
        item["ordinal"] = ordinal
    return result


def source_anchors(
    names: Iterable[str],
    source_functions: dict[str, list[Slice]],
    analyze_names: Iterable[str] | None = None,
) -> tuple[list[dict[str, object]], list[str], list[Segment]]:
    anchors: list[dict[str, object]] = []
    opens: list[str] = []
    segments: list[Segment] = []
    analyzed = set(names if analyze_names is None else analyze_names)
    for name in names:
        candidates = source_functions.get(name, [])
        if not candidates:
            opens.append(
                f"Pinned source symbol {name} was not resolved in the remc2 "
                "translation units; its handler lineage is open."
            )
            continue
        if len(candidates) > 1:
            opens.append(
                f"Pinned source symbol {name} has {len(candidates)} candidate "
                "definitions; all candidates remain in the evidence set."
            )
        for candidate in candidates:
            anchors.append(
                evidence(candidate.path, candidate.start_line, candidate.name)
            )
            if name in analyzed:
                segments.append(
                    Segment(
                        path=candidate.path,
                        label=candidate.name,
                        start_line=candidate.start_line,
                        text=candidate.text,
                    )
                )
    return anchors, sorted(set(opens)), segments


def make_transition(
    *,
    transition_id: str,
    name: str,
    schedule_position: str,
    rust_segments: list[Segment],
    source_names: list[str],
    rust_index: dict[str, list[Slice]],
    source_functions: dict[str, list[Slice]],
    source_role: str,
    source_analysis_names: list[str] | None = None,
    route_role: str | None = None,
    slot_kind: int | None = None,
    callsite: dict[str, object] | None = None,
) -> dict[str, object]:
    expanded, rust_opens = function_segments(rust_segments[0], rust_index)
    anchors, source_opens, source_segments = source_anchors(
        source_names, source_functions, source_analysis_names
    )
    rust_cells_found, rust_cell_opens = rust_cells(expanded)
    source_cells_found, source_cell_opens = source_cells(source_segments)
    cells = merge_cells(rust_cells_found, source_cells_found)
    opens = sorted(
        set(rust_opens + source_opens + rust_cell_opens + source_cell_opens)
    )
    if callsite:
        anchors.insert(0, callsite)
    reads = sorted(
        item["cell"]
        for item in cells
        if "read" in item["access"] or "read_write" in item["access"]
    )
    writes = sorted(
        item["cell"]
        for item in cells
        if "write" in item["access"] or "read_write" in item["access"]
    )
    return {
        "id": transition_id,
        "name": name,
        "schedule_position": schedule_position,
        "route_role": route_role,
        "slot_kind": slot_kind,
        "rust_handlers": [
            evidence(segment.path, segment.start_line, segment.label)
            for segment in expanded
        ],
        "source_handlers": anchors,
        "reads": reads,
        "writes": writes,
        "cells": cells,
        "rng_draws": ordered_events(expanded, RNG_RE, "rng"),
        "emissions": ordered_events(expanded, EMISSION_RE, "emission"),
        "open_questions": opens,
        "source_role": source_role,
    }


def callsite_segment(function: Slice, method: str, label: str) -> Segment | None:
    pattern = re.compile(rf"\b{re.escape(method)}\s*\(")
    match = pattern.search(function.text)
    if not match:
        return None
    line = function.start_line + function.text.count("\n", 0, match.start())
    return Segment(
        path=function.path,
        label=label,
        start_line=line,
        text=match.group(0),
    )


def top_level_route_arms(function: Slice) -> list[tuple[int, int, int, str]]:
    marker = function.text.find("match kind")
    if marker < 0:
        return []
    open_index = function.text.find("{", marker)
    if open_index < 0:
        return []
    close_index = brace_end(function.text, open_index)
    candidates = list(
        re.finditer(
            r"(?m)^([ \t]+)([0-9]+)\s*=>",
            function.text[open_index:close_index],
        )
    )
    if not candidates:
        return []
    minimum_indent = min(len(match.group(1).expandtabs(4)) for match in candidates)
    arms: list[tuple[int, int, int, str]] = []
    filtered = [
        match
        for match in candidates
        if len(match.group(1).expandtabs(4)) == minimum_indent
    ]
    for ordinal, match in enumerate(filtered):
        start = open_index + match.start()
        end = (
            open_index + filtered[ordinal + 1].start()
            if ordinal + 1 < len(filtered)
            else close_index
        )
        kind = int(match.group(2))
        arms.append(
            (
                kind,
                start,
                end,
                f"route_kind_{kind}",
            )
        )
    return arms


def direct_call_names(text: str) -> list[str]:
    return method_calls(text)


def make_call_transitions(
    *,
    prefix: str,
    function: Slice,
    text: str,
    schedule_position: str,
    source_role: str,
    source_names: list[str],
    rust_index: dict[str, list[Slice]],
    source_functions: dict[str, list[Slice]],
    starting_ordinal: int,
    source_analysis_names: list[str] | None = None,
    only_names: set[str] | None = None,
) -> list[dict[str, object]]:
    transitions: list[dict[str, object]] = []
    region_start = function.text.find(text) if text else -1
    if region_start < 0:
        region_start = 0
    names = [
        name
        for name in direct_call_names(text)
        if only_names is None or name in only_names
    ]
    for ordinal, name in enumerate(names, starting_ordinal):
        match = re.search(rf"\b{re.escape(name)}\s*\(", text)
        if match is None:
            continue
        absolute = region_start + match.start()
        line = function.start_line + function.text.count("\n", 0, absolute)
        callsite = Segment(
            path=function.path,
            label=name,
            start_line=line,
            text=match.group(0),
        )
        candidates = rust_index.get(name, [])
        if candidates:
            rust_segments = [
                Segment(
                    path=candidate.path,
                    label=candidate.name,
                    start_line=candidate.start_line,
                    text=candidate.text,
                )
                for candidate in candidates
            ]
        else:
            rust_segments = [callsite]
        transitions.append(
            make_transition(
                transition_id=f"{prefix}-{ordinal:03d}-{name}",
                name=name,
                schedule_position=schedule_position,
                rust_segments=rust_segments,
                source_names=source_names,
                rust_index=rust_index,
                source_functions=source_functions,
                source_role=source_role,
                source_analysis_names=source_analysis_names,
                callsite=evidence(function.path, callsite.start_line, name),
            )
        )
    return transitions


def git_revision(path: Path) -> str:
    try:
        return subprocess.run(
            ["git", "-C", str(path), "rev-parse", "HEAD"],
            check=True,
            capture_output=True,
            text=True,
            timeout=10,
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired):
        return "unresolved"


def input_hashes() -> list[dict[str, str]]:
    paths = sorted(RUST_ROOT.rglob("*.rs")) + sorted(
        path
        for path in SOURCE_ROOT.rglob("*.cpp")
        if path.name in {"Events.cpp", "EventsFunctions.cpp"}
    )
    return [
        {
            "file": rel(path),
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        }
        for path in paths
    ]


def build_inventory() -> dict[str, object]:
    rust_index = load_rust_functions()
    source_functions = source_index()
    source_pass_candidates = rust_index.get("step_level2_source_pass_with_player", [])
    outer_candidates = rust_index.get("step_outer_turn", [])
    if len(source_pass_candidates) != 1 or len(outer_candidates) != 1:
        raise RuntimeError(
            "expected exactly one Level 2 source-pass and outer-turn definition"
        )
    source_pass = source_pass_candidates[0]
    outer_turn = outer_candidates[0]
    transitions: list[dict[str, object]] = []

    loop_marker = outer_turn.text.find("for pass in")
    if loop_marker < 0:
        raise RuntimeError("step_outer_turn pass loop was not found")
    transitions.extend(
        make_call_transitions(
            prefix="outer-prepass",
            function=outer_turn,
            text=outer_turn.text[:loop_marker],
            schedule_position="prepass",
            source_role="outer turn before UpdateEntities_57730",
            source_names=SOURCE_ANCHORS["outer_player_input"],
            rust_index=rust_index,
            source_functions=source_functions,
            starting_ordinal=0,
            source_analysis_names=[],
        )
    )
    transitions.extend(
        make_call_transitions(
            prefix="following-pass-cleanup",
            function=outer_turn,
            text=outer_turn.text[
                loop_marker : outer_turn.text.find(
                    "self.step_level2_source_pass_with_player", loop_marker
                )
            ],
            schedule_position="following_pass_cleanup",
            source_role="UpdateEntities_57730 reclaim prologue",
            source_names=SOURCE_ANCHORS["following_pass_cleanup"],
            rust_index=rust_index,
            source_functions=source_functions,
            starting_ordinal=0,
            source_analysis_names=["sub_57F20"],
        )
    )

    route_marker = source_pass.text.find("let mut routes")
    if route_marker < 0:
        raise RuntimeError("Level 2 route construction was not found")
    prefix_text = source_pass.text[:route_marker]
    transitions.extend(
        make_call_transitions(
            prefix="source-prepass",
            function=source_pass,
            text=prefix_text,
            schedule_position="prepass",
            source_role="UpdateEntities_57730 source-pass prologue",
            source_names=SOURCE_ANCHORS["source_dispatch"],
            rust_index=rust_index,
            source_functions=source_functions,
            starting_ordinal=0,
            source_analysis_names=[],
        )
    )

    route_loop = source_pass.text.find("for (source_slot, kind)", route_marker)
    if route_loop < 0:
        raise RuntimeError("Level 2 source route loop was not found")
    route_line = source_pass.start_line + source_pass.text.count(
        "\n", 0, route_marker
    )
    route_segment = Segment(
        path=source_pass.path,
        label="route_construction",
        start_line=route_line,
        text=source_pass.text[route_marker:route_loop],
    )
    transitions.append(
        make_transition(
            transition_id="source-route-construction",
            name="source_slot_route_construction",
            schedule_position="slot_visit",
            rust_segments=[route_segment],
            source_names=SOURCE_ANCHORS["source_dispatch"],
            rust_index=rust_index,
            source_functions=source_functions,
            source_role="UpdateEntities_57730 source cursor and source-slot ordering",
            source_analysis_names=[],
            callsite=evidence(source_pass.path, route_line, "routes"),
        )
    )

    route_arms = top_level_route_arms(source_pass)
    for kind, start, end, label in route_arms:
        arm_text = source_pass.text[start:end]
        line = source_pass.start_line + source_pass.text.count("\n", 0, start)
        name = ROUTE_ARM_NAMES.get(kind, f"unclassified_kind_{kind}")
        role = f"ordinary Level 2 route kind {kind}"
        source_key = {
            0: "shared_pickup_source_slot",
            1: "bad_stone_source_slot",
            2: "house_source_slot",
            3: "villager_source_slot",
            4: "dolmen_source_slot",
            5: "castle_source_slot",
            6: "dynamic_source_slot",
            7: "goat_source_slot",
            8: "generic_spell_pickup_source_slot",
            9: "local_player_source_slot",
            10: "nyphur_source_slot",
            11: "dynamic_source_slot",
            12: "dynamic_source_slot",
            13: "fireball_source_slot",
        }.get(kind, "source_dispatch")
        source_analysis = {
            3: ["sub_10C80"],
            9: ["AddPlayer03_00_5E010", "sub_693F0", "sub_6DCA0"],
            8: ["NewEvent_4A050"],
            6: ["NewEvent_4A050"],
            13: ["sub_693F0", "sub_6DCA0"],
        }.get(kind, [])
        transitions.append(
            make_transition(
                transition_id=f"slot-kind-{kind:02d}",
                name=name,
                schedule_position="slot_visit",
                rust_segments=[
                    Segment(
                        path=source_pass.path,
                        label=label,
                        start_line=line,
                        text=arm_text,
                    )
                ],
                source_names=SOURCE_ANCHORS[source_key],
                rust_index=rust_index,
                source_functions=source_functions,
                source_role=role,
                source_analysis_names=source_analysis,
                route_role=role,
                slot_kind=kind,
                callsite=evidence(source_pass.path, line, f"match kind {kind}"),
            )
        )

    after_marker = source_pass.text.find(
        "self.activate_level2_authored_mana_attraction_fields"
    )
    if after_marker < 0:
        raise RuntimeError("same-pass Level 2 epilogue was not found")
    cursor_text = source_pass.text[route_loop:after_marker]
    for method, source_analysis in [
        ("live_arena_cursor_rows", []),
        ("step_level2_pending_possession_dynamic_row", ["NewEvent_4A050"]),
        ("schedule_new_possession_rows_in_source_order", []),
    ]:
        method_offset = cursor_text.find(method)
        if method_offset < 0:
            continue
        line_start = cursor_text.rfind("\n", 0, method_offset)
        line_end = cursor_text.find("\n", method_offset)
        if line_end < 0:
            line_end = len(cursor_text)
        transitions.extend(
            make_call_transitions(
                prefix=f"same-pass-cursor-{method}",
                function=source_pass,
                text=cursor_text[line_start:line_end],
                schedule_position="same_pass_epilogue",
                source_role="ApplyEvents_498A0 moving cursor and same-pass dynamic allocation",
                source_names=SOURCE_ANCHORS["same_pass_cursor"],
                rust_index=rust_index,
                source_functions=source_functions,
                starting_ordinal=0,
                source_analysis_names=source_analysis,
                only_names={method},
            )
        )

    transitions.extend(
        make_call_transitions(
            prefix="same-pass-epilogue",
            function=source_pass,
            text=source_pass.text[after_marker:],
            schedule_position="same_pass_epilogue",
            source_role="ApplyEvents_498A0 cursor follow-up and same-pass epilogue",
            source_names=SOURCE_ANCHORS["same_pass_cursor"],
            rust_index=rust_index,
            source_functions=source_functions,
            starting_ordinal=0,
            source_analysis_names=["NewEvent_4A050"],
        )
    )

    post_marker = outer_turn.text.find("self.finish_level2_day_hud_frame")
    transitions.extend(
        make_call_transitions(
            prefix="outer-postpass",
            function=outer_turn,
            text=outer_turn.text[post_marker:] if post_marker >= 0 else "",
            schedule_position="postpass",
            source_role="objective and presentation tail after UpdateEntities_57730",
            source_names=SOURCE_ANCHORS["objective_tail"]
            + SOURCE_ANCHORS["hud_tail"],
            rust_index=rust_index,
            source_functions=source_functions,
            starting_ordinal=0,
            source_analysis_names=[
                "sub_58F00_game_objectives",
                "PresentObjective_59820",
                "PlayEntitySounds_6E150",
            ],
        )
    )

    for ordinal, transition in enumerate(transitions):
        transition["transition_ordinal"] = ordinal
    open_questions = sorted(
        {
            question
            for transition in transitions
            for question in transition["open_questions"]
        }
    )
    known_unreached = [
        {
            "callback": "sub_58F00_game_objectives -> PresentObjective_59820 -> PlayEntitySounds_6E150",
            "status": "partially_unreached",
            "reason": "The Rust route reaches its objective and HUD tail, but the "
            "renderer and audio callbacks remain Unity presentation responsibilities "
            "and are not executed by sim-core.",
            "evidence": [
                {
                    "file": "upstream/magic-carpet-2-hd/remc2/engine/EventsFunctions.cpp",
                    "line": 40693,
                    "symbol": "sub_58F00_game_objectives",
                },
                {
                    "file": "upstream/magic-carpet-2-hd/remc2/engine/EventsFunctions.cpp",
                    "line": 40957,
                    "symbol": "PresentObjective_59820",
                },
                {
                    "file": "upstream/magic-carpet-2-hd/remc2/engine/EventsFunctions.cpp",
                    "line": 44343,
                    "symbol": "PlayEntitySounds_6E150",
                },
            ],
        },
        {
            "callback": "all disabled or unselected action and constructor table rows",
            "status": "unreached",
            "reason": "The ordinary route materializes only the Level 2 source-slot "
            "families and live dynamic rows; the complete 845-row dispatch "
            "denominator is outside this route inventory.",
            "evidence": [
                {
                    "file": "research/private/remc2-atlas/semantics/generated/runtime-dispatch-rows.csv",
                    "line": 1,
                    "symbol": "dispatch denominator",
                }
            ],
        },
        {
            "callback": "indirect callback and packed-byte aliases",
            "status": "open",
            "reason": "Static source parsing records the containing transition and "
            "the unresolved access, but does not guess an alias or callback target.",
            "evidence": [
                {
                    "file": "upstream/magic-carpet-2-hd/remc2/engine/Events.cpp",
                    "line": 410,
                    "symbol": "ApplyEvents_498A0",
                },
                {
                    "file": "upstream/magic-carpet-2-hd/remc2/engine/EventsFunctions.cpp",
                    "line": 39928,
                    "symbol": "UpdateEntities_57730",
                },
            ],
        },
    ]
    return {
        "schema_version": SCHEMA_VERSION,
        "route": {
            "name": "ordinary_level2_source_route",
            "entry": {
                "file": rel(outer_turn.path),
                "line": outer_turn.start_line,
                "symbol": outer_turn.name,
            },
            "source_pass": {
                "file": rel(source_pass.path),
                "line": source_pass.start_line,
                "symbol": source_pass.name,
            },
            "schedule_positions": [
                "prepass",
                "slot_visit",
                "same_pass_epilogue",
                "following_pass_cleanup",
                "postpass",
            ],
            "route_kinds": [
                {
                    "kind": kind,
                    "name": ROUTE_ARM_NAMES.get(
                        kind, f"unclassified_kind_{kind}"
                    ),
                }
                for kind in sorted({arm[0] for arm in route_arms})
            ],
            "semantics": "This is the ordinary Rust Level 2 route: one "
            "step_outer_turn, each selected entity pass for the requested "
            "speed index, source-slot ordering, same-pass allocation cursor, "
            "following-pass reclaim boundary, then the objective and HUD tail.",
        },
        "inputs": {
            "rust_revision": git_revision(ROOT),
            "remc2_revision": git_revision(SOURCE_ROOT),
            "files": input_hashes(),
        },
        "classification_vocabulary": sorted(CLASSIFICATIONS),
        "transitions": transitions,
        "coverage": {
            "reached_transition_count": len(transitions),
            "route_kind_count": len({arm[0] for arm in route_arms}),
            "known_unreached_callbacks": known_unreached,
            "open_question_count": len(open_questions),
            "statement": "Coverage is the static transition set reached by the "
            "current ordinary Level 2 Rust route shape. It includes all source "
            "pass prologue calls, route kinds 0 through 12, same-pass cursor "
            "follow-up calls, the following-pass reclaim call, and the outer "
            "objective and HUD tail. Runtime predicates inside those handlers and "
            "unresolved packed or indirect aliases remain open.",
        },
        "open_questions": open_questions,
        "tool": {
            "name": "build_campaign_rw_inventory.py",
            "version": "1",
            "source_backed": True,
            "generated_paths_are_root_relative": True,
        },
    }


def canonical_json(value: object) -> bytes:
    return (
        json.dumps(value, ensure_ascii=True, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def summary_markdown(inventory: dict[str, object]) -> str:
    route = inventory["route"]
    coverage = inventory["coverage"]
    transitions = inventory["transitions"]
    lines = [
        "# Level 2 read/write/RNG/emission inventory",
        "",
        "This document is a deterministic summary of the machine-readable "
        "artifact at research/private/remc2-atlas/artifacts/"
        "campaign-rw-inventory.json. It is a static, source-backed inventory; "
        "unresolved packed-byte aliases and indirect callbacks stay open.",
        "",
        "## Route and evidence",
        "",
        f"- Route: {route['name']}",
        f"- Rust entry: {route['entry']['file']}:{route['entry']['line']} "
        f"({route['entry']['symbol']})",
        f"- Rust source pass: {route['source_pass']['file']}:"
        f"{route['source_pass']['line']} ({route['source_pass']['symbol']})",
        "- Pinned remc2 schedule anchors: UpdateEntities_57730 "
        "(EventsFunctions.cpp:39928 in the current checkout) and "
        "ApplyEvents_498A0 (Events.cpp:410).",
        f"- Reached transition records: {coverage['reached_transition_count']}",
        f"- Open-question count: {coverage['open_question_count']}",
        "",
        "The route follows the player input prepass, source-pass prologue, "
        "source-slot route kinds, same-pass cursor and epilogue, following-pass "
        "reclaim boundary, and objective/HUD tail. Each record carries ordered "
        "RNG and emission observations from its Rust and source slices.",
        "",
        "## Transition schedule",
        "",
        "| ID | Name | Schedule | Slot kind | Reads | Writes | RNG | Emissions | Open |",
        "| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for transition in transitions:
        slot = "" if transition["slot_kind"] is None else str(transition["slot_kind"])
        lines.append(
            f"| {transition['id']} | {transition['name']} | "
            f"{transition['schedule_position']} | {slot} | "
            f"{len(transition['reads'])} | {len(transition['writes'])} | "
            f"{len(transition['rng_draws'])} | {len(transition['emissions'])} | "
            f"{len(transition['open_questions'])} |"
        )
    lines.extend(
        [
            "",
            "## Cell classification",
            "",
            "Every observed cell has a classification and file:line evidence. "
            "A null classification is deliberate: the cell is listed in the "
            "artifact with an open question instead of being guessed.",
            "",
            "- immutable_fact: decoded or compiled content that the transition "
            "does not author during the session.",
            "- session_state: mutable simulation or session carrier.",
            "- derived_index: cursor, projection, cache, route, or other "
            "recomputed index.",
            "- presentation_only: event, trace, HUD, audio, or evidence output.",
            "",
            "## Known unreached and open callbacks",
            "",
        ]
    )
    for item in coverage["known_unreached_callbacks"]:
        lines.append(
            f"- {item['status']} — {item['callback']}: {item['reason']}"
        )
    lines.extend(
        [
            "",
            "The full artifact records the exact evidence list and open "
            "questions for every transition. No sim-core, sim-capi, exporter, "
            "owner-data package, Unity, or pinned remc2 source file is modified "
            "by this measurement lane.",
            "",
        ]
    )
    return "\n".join(lines)


def write_outputs(
    inventory: dict[str, object],
    artifact_dir: Path,
    summary_path: Path,
) -> None:
    artifact_dir.mkdir(parents=True, exist_ok=True)
    summary_path.parent.mkdir(parents=True, exist_ok=True)
    payload = canonical_json(inventory)
    artifact_path = artifact_dir / "campaign-rw-inventory.json"
    hash_path = artifact_dir / "campaign-rw-inventory.sha256"
    artifact_path.write_bytes(payload)
    hash_path.write_text(
        hashlib.sha256(payload).hexdigest()
        + "  campaign-rw-inventory.json\n",
        encoding="utf-8",
    )
    summary_path.write_text(summary_markdown(inventory), encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=DEFAULT_ARTIFACT_DIR,
        help="directory for the JSON artifact and its SHA-256 sidecar",
    )
    parser.add_argument(
        "--summary",
        type=Path,
        default=DEFAULT_SUMMARY,
        help="Markdown summary path",
    )
    args = parser.parse_args()
    inventory = build_inventory()
    write_outputs(inventory, args.output_dir, args.summary)
    print(
        f"wrote {args.output_dir / 'campaign-rw-inventory.json'} "
        f"({len(inventory['transitions'])} transitions)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
