#!/usr/bin/env python3
"""Canonical first-presented-frame trace format and qualification checks."""

from __future__ import annotations

from collections import Counter
import hashlib
import json
from pathlib import Path

SCHEMA = "mc2-first-presented-frame-trace/v1"
EVENT_SHA_SCHEMA = "mc2-frame-event-set/sha256-v1"


def canonical_json(value: dict) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def canonicalize_events(events: list[dict]) -> list[dict]:
    encoded = Counter(canonical_json(event) for event in events)
    result = []
    for event_json, count in sorted(encoded.items()):
        event = json.loads(event_json)
        event["count"] = count
        result.append(event)
    return sorted(result, key=canonical_json)


def event_set_sha256(events: list[dict]) -> str:
    payload = "".join(canonical_json(event) + "\n" for event in events).encode()
    return hashlib.sha256(payload).hexdigest()


def build_trace(header: dict, raw_events: list[dict]) -> str:
    events = canonicalize_events(raw_events)
    qualified = dict(header)
    qualified["schema"] = SCHEMA
    qualified["event_set"] = {
        "schema": EVENT_SHA_SCHEMA,
        "count": len(events),
        "sha256": event_set_sha256(events),
    }
    lines = [canonical_json({"header": qualified})]
    lines.extend(canonical_json({"event": event}) for event in events)
    return "\n".join(lines) + "\n"


def parse_trace(path: Path) -> tuple[dict, list[dict]]:
    lines = path.read_text(encoding="utf-8").splitlines()
    if not lines:
        raise ValueError("trace is empty")
    first = json.loads(lines[0])
    if set(first) != {"header"}:
        raise ValueError("first JSONL row must be the header")
    header = first["header"]
    events = []
    for line in lines[1:]:
        row = json.loads(line)
        if set(row) != {"event"}:
            raise ValueError("non-header JSONL rows must be events")
        events.append(row["event"])
    return header, events


def qualify_trace(path: Path, *, require_complete_categories: bool = False) -> dict:
    header, events = parse_trace(path)
    if header.get("schema") != SCHEMA:
        raise ValueError("wrong trace schema")
    if events != sorted(events, key=canonical_json):
        raise ValueError("events are not canonical-order")
    expected = header.get("event_set", {})
    if expected.get("schema") != EVENT_SHA_SCHEMA:
        raise ValueError("wrong event-set hash schema")
    if expected.get("count") != len(events):
        raise ValueError("event-set count mismatch")
    if expected.get("sha256") != event_set_sha256(events):
        raise ValueError("event-set hash mismatch")
    window = header.get("window", {})
    if window.get("name") != "first_render_bearing_gameplay_present":
        raise ValueError("wrong or missing qualified window")
    bounds = window.get("bounds", {})
    qualified_epoch = window.get("qualified_epoch")
    if not isinstance(qualified_epoch, int) or not 1 <= qualified_epoch <= bounds.get(
        "max_candidates", 0
    ):
        raise ValueError("qualified epoch is outside its declared bound")
    diagnostics = window.get("pre_window_diagnostics", [])
    if len(diagnostics) > bounds.get("max_skipped_presents", -1):
        raise ValueError("pre-window diagnostics exceed their declared bound")
    for ordinal, diagnostic in enumerate(diagnostics, 1):
        if set(diagnostic) != {
            "ordinal",
            "epoch",
            "source",
            "pointer_class",
            "caller_class",
            "caller_path",
            "final_tmap_count_at_entry",
            "subblit_entries",
            "return_proven",
        }:
            raise ValueError("invalid pre-window present diagnostic")
        if diagnostic["ordinal"] != ordinal:
            raise ValueError("pre-window diagnostic ordinals are not contiguous")
        if not 1 <= diagnostic["epoch"] <= qualified_epoch:
            raise ValueError("pre-window diagnostic has an invalid epoch")
        if diagnostic["source"] not in {"VGA_Blit", "direct_SubBlit"}:
            raise ValueError("pre-window diagnostic has an invalid source")
        if diagnostic["pointer_class"] not in {
            "null",
            "screen_buffer",
            "other_non_null",
            "not_applicable",
        }:
            raise ValueError("pre-window diagnostic has an invalid pointer class")
        if (
            not isinstance(diagnostic["caller_path"], list)
            or not diagnostic["caller_path"]
            or not all(isinstance(name, str) for name in diagnostic["caller_path"])
            or not isinstance(diagnostic["final_tmap_count_at_entry"], int)
            or diagnostic["final_tmap_count_at_entry"] < 0
            or diagnostic["subblit_entries"] != 1
            or diagnostic["return_proven"] is not True
        ):
            raise ValueError("invalid pre-window present proof")
    prequalified = window.get("prequalified_candidates", [])
    if len(prequalified) != qualified_epoch - 1:
        raise ValueError("prequalified candidate coverage is incomplete")
    for epoch, candidate in enumerate(prequalified, 1):
        expected_ordinals = [
            item["ordinal"] for item in diagnostics if item["epoch"] == epoch
        ]
        if candidate != {
            "epoch": epoch,
            "final_tmap_count": candidate.get("final_tmap_count"),
            "diagnostic_ordinals": expected_ordinals,
            "reason": "no_expected_gameplay_present_before_next_epoch",
        } or not isinstance(candidate["final_tmap_count"], int):
            raise ValueError("invalid prequalified candidate summary")
    transcript = window.get("lifecycle_transcript", [])
    if not transcript:
        raise ValueError("missing lifecycle transcript")
    if [row.get("seq") for row in transcript] != list(range(1, len(transcript) + 1)):
        raise ValueError("lifecycle transcript sequence is not contiguous")
    epochs = [row.get("epoch") for row in transcript]
    if any(not isinstance(epoch, int) or not 1 <= epoch <= qualified_epoch for epoch in epochs):
        raise ValueError("lifecycle transcript has an invalid epoch")
    if epochs != sorted(epochs):
        raise ValueError("lifecycle transcript epochs are not ordered")
    for epoch in range(1, qualified_epoch + 1):
        rows = [row for row in transcript if row["epoch"] == epoch]
        if not rows or rows[0]["kind"] != "candidate_begin":
            raise ValueError("each epoch must begin with candidate_begin")
        if sum(row["kind"] == "candidate_begin" for row in rows) != 1:
            raise ValueError("candidate_begin must occur exactly once per epoch")
        rejected = [row for row in rows if row["kind"] == "candidate_rejected"]
        if epoch < qualified_epoch:
            if len(rejected) != 1 or rows[-1] != rejected[0]:
                raise ValueError("prequalified epoch must end with candidate_rejected")
        elif rejected:
            raise ValueError("qualified epoch cannot be rejected")
    qualified_kinds = [
        "first_final_tmap",
        "qualified_vga_entry",
        "qualified_subblit",
        "qualified_vga_return",
        "closed",
    ]
    qualified_rows = [
        row for row in transcript if row["epoch"] == qualified_epoch
    ]
    positions = []
    for kind in qualified_kinds:
        matches = [
            index for index, row in enumerate(qualified_rows) if row["kind"] == kind
        ]
        if len(matches) != 1:
            raise ValueError(f"qualified transcript requires one {kind}")
        positions.append(matches[0])
    if positions != sorted(positions) or qualified_rows[-1]["kind"] != "closed":
        raise ValueError("qualified lifecycle transcript is out of order")
    qualified_entry = qualified_rows[positions[1]]
    if (
        qualified_entry.get("pointer_class") != "screen_buffer"
        or qualified_entry.get("caller_class") != "expected_gameplay_blit"
        or qualified_entry.get("final_tmap_count")
        != window.get("qualified_final_tmap_count")
        or qualified_rows[positions[2]].get("subblit_entries") != 1
        or qualified_rows[positions[3]].get("return_proven") is not True
        or qualified_rows[positions[4]].get("no_second_qualified_present") is not True
    ):
        raise ValueError("qualified lifecycle proof is incomplete")
    for diagnostic in diagnostics:
        ordinal = diagnostic["ordinal"]
        if diagnostic["source"] == "VGA_Blit":
            kinds = [
                "excluded_vga_entry",
                "excluded_vga_subblit",
                "excluded_vga_return",
            ]
        else:
            kinds = [
                "excluded_direct_subblit_entry",
                "excluded_direct_subblit_return",
            ]
        positions = [
            next(
                (
                    index
                    for index, row in enumerate(transcript)
                    if row["kind"] == kind
                    and row.get("diagnostic_ordinal") == ordinal
                ),
                -1,
            )
            for kind in kinds
        ]
        if -1 in positions or positions != sorted(positions):
            raise ValueError("pre-window diagnostic transcript is incomplete")
    if any(event.get("category") == "lifecycle" for event in events):
        raise ValueError("ordered lifecycle rows cannot enter the canonical asset event set")
    if any(event.get("resolution") != "unique" for event in events if "selector" in event):
        raise ValueError("unknown or ambiguous final selector")
    categories = {event.get("category") for event in events}
    if "tmap" not in categories:
        raise ValueError("trace has no final TMAP entry")
    tmap_count = sum(
        event["count"] for event in events if event.get("category") == "tmap"
    )
    if tmap_count != window.get("qualified_final_tmap_count"):
        raise ValueError("qualified TMAP count does not match the event set")
    empty_evidence = window.get("empty_category_evidence", {})
    if not isinstance(empty_evidence, dict):
        raise ValueError("empty-category evidence must be an object")
    for category, proof in empty_evidence.items():
        if category in categories:
            raise ValueError(
                f"{category} cannot have both asset events and empty-category evidence"
            )
        if (
            category not in {"cursor", "sound", "music"}
            or not isinstance(proof, dict)
            or proof.get("epoch") != qualified_epoch
            or proof.get("hook_armed") is not True
            or proof.get("actual_entry_count") != 0
            or not isinstance(proof.get("hook"), str)
            or not proof["hook"]
            or not isinstance(proof.get("reason"), str)
            or not proof["reason"]
        ):
            raise ValueError(f"invalid empty-category evidence for {category}")
        if category == "cursor":
            reason = proof["reason"]
            if reason not in {
                "cursor_outer_suppression_true",
                "cursor_draw_gate_false",
                "cursor_post_clip_extent_empty",
            }:
                raise ValueError("invalid empty cursor reason")
            if reason == "cursor_post_clip_extent_empty":
                state = proof.get("post_clip_state")
                if (
                    not isinstance(state, dict)
                    or state.get("post_clip_hook_reached") is not True
                    or state.get("resolution") != "unique"
                    or state.get("bank") != "POINTERS"
                    or not isinstance(state.get("source_selector"), int)
                    or state.get("nominal_selector")
                    != proof.get("active_selector_state")
                    or (
                        state.get("visible_width", 1) > 0
                        and state.get("visible_height", 1) > 0
                    )
                ):
                    raise ValueError("invalid post-clip empty cursor proof")
        if category in {"sound", "music"}:
            expected_line = 594 if category == "sound" else 104
            expected_function = (
                "SOUND_start_sample"
                if category == "sound"
                else "SOUND_start_sequence"
            )
            expected_source = (
                "upstream/magic-carpet-2-hd/remc2/portability/"
                "port_sdl_sound.cpp"
            )
            hook_proof = proof.get("resolved_hook_proof")
            if (
                not isinstance(hook_proof, dict)
                or set(hook_proof) != {"startup", "close"}
            ):
                raise ValueError(f"{category} empty evidence lacks resolved hook proof")
            for phase in ("startup", "close"):
                resolved = hook_proof[phase]
                if (
                    not isinstance(resolved, dict)
                    or set(resolved)
                    != {
                        "breakpoint_valid",
                        "breakpoint_enabled",
                        "breakpoint_pending",
                        "requested_source",
                        "requested_line",
                        "resolved_location_count",
                        "resolved_source",
                        "resolved_line",
                        "resolved_function",
                        "resolved_location_enabled",
                    }
                    or resolved["breakpoint_valid"] is not True
                    or resolved["breakpoint_enabled"] is not True
                    or resolved["breakpoint_pending"] is not False
                    or resolved["requested_source"] != expected_source
                    or resolved["requested_line"] != expected_line
                    or resolved["resolved_location_count"] != 1
                    or resolved["resolved_source"] != expected_source
                    or resolved["resolved_line"] != expected_line
                    or expected_function not in resolved["resolved_function"]
                    or resolved["resolved_location_enabled"] is not True
                ):
                    raise ValueError(
                        f"{category} {phase} hook is pending, unresolved, or wrong"
                    )
            if hook_proof["startup"] != hook_proof["close"]:
                raise ValueError(f"{category} hook changed between startup and close")
    if require_complete_categories:
        required = {"terrain", "sky", "bitmap", "cursor", "text", "glyph", "sound"}
        missing = required - categories - set(empty_evidence)
        if missing:
            raise ValueError(f"trace is missing required categories: {sorted(missing)!r}")
    return {
        "event_set_sha256": expected["sha256"],
        "events": len(events),
        "categories": sorted(category for category in categories if category),
        "music_starts": sum(
            event["count"] for event in events if event.get("category") == "music"
        ),
        "qualified_epoch": qualified_epoch,
        "skipped_presents": len(diagnostics),
    }
