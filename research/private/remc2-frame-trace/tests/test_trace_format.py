from __future__ import annotations

import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from trace_format import build_trace, parse_trace, qualify_trace


HEADER = {
    "bindings": {"package_sha256": "a" * 64},
    "window": {
        "name": "first_render_bearing_gameplay_present",
        "qualified_epoch": 2,
        "qualified_final_tmap_count": 1,
        "bounds": {"max_candidates": 64, "max_skipped_presents": 256},
        "prequalified_candidates": [
            {
                "epoch": 1,
                "final_tmap_count": 0,
                "diagnostic_ordinals": [1],
                "reason": "no_expected_gameplay_present_before_next_epoch",
            }
        ],
        "pre_window_diagnostics": [
            {
                "ordinal": 1,
                "epoch": 1,
                "source": "VGA_Blit",
                "pointer_class": "null",
                "caller_class": "other",
                "caller_path": ["fix_sub_9A0FC_wait_to_screen_beam"],
                "final_tmap_count_at_entry": 0,
                "subblit_entries": 1,
                "return_proven": True,
            }
        ],
        "lifecycle_transcript": [
            {"seq": 1, "epoch": 1, "kind": "candidate_begin"},
            {
                "seq": 2, "epoch": 1, "kind": "excluded_vga_entry",
                "diagnostic_ordinal": 1, "pointer_class": "null",
                "caller_class": "other", "final_tmap_count": 0,
            },
            {
                "seq": 3, "epoch": 1, "kind": "excluded_vga_subblit",
                "diagnostic_ordinal": 1, "subblit_entries": 1,
            },
            {
                "seq": 4, "epoch": 1, "kind": "excluded_vga_return",
                "diagnostic_ordinal": 1, "return_proven": True,
            },
            {
                "seq": 5, "epoch": 1, "kind": "candidate_rejected",
                "final_tmap_count": 0, "diagnostic_ordinals": [1],
                "reason": "no_expected_gameplay_present_before_next_epoch",
            },
            {"seq": 6, "epoch": 2, "kind": "candidate_begin"},
            {"seq": 7, "epoch": 2, "kind": "first_final_tmap", "selector": 7},
            {
                "seq": 8, "epoch": 2, "kind": "qualified_vga_entry",
                "pointer_class": "screen_buffer",
                "caller_class": "expected_gameplay_blit",
                "caller_path": [
                    "sub_90478_VGA_Blit320", "BlendAndBlit_40F80",
                    "DrawAndEventsInGame_47560",
                ],
                "final_tmap_count": 1,
            },
            {
                "seq": 9, "epoch": 2, "kind": "qualified_subblit",
                "subblit_entries": 1,
            },
            {
                "seq": 10, "epoch": 2, "kind": "qualified_vga_return",
                "return_proven": True,
            },
            {
                "seq": 11, "epoch": 2, "kind": "closed",
                "no_second_qualified_present": True,
            },
        ],
    },
}
EVENTS = [
    {"category": "tmap", "selector": 7, "resolution": "unique", "bank": "TMAPS1-0"},
]


def resolved_hook_proof(category: str) -> dict:
    line = 594 if category == "sound" else 104
    function = (
        "_Z18SOUND_start_sampleP7_SAMPLE"
        if category == "sound"
        else "_Z20SOUND_start_sequenceP8SEQUENCE"
    )
    source = (
        "upstream/magic-carpet-2-hd/remc2/portability/port_sdl_sound.cpp"
    )
    phase = {
        "breakpoint_valid": True,
        "breakpoint_enabled": True,
        "breakpoint_pending": False,
        "requested_source": source,
        "requested_line": line,
        "resolved_location_count": 1,
        "resolved_source": source,
        "resolved_line": line,
        "resolved_function": function,
        "resolved_location_enabled": True,
    }
    return {"startup": dict(phase), "close": dict(phase)}


class TraceFormatTests(unittest.TestCase):
    def write(self, text: str) -> Path:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        path = Path(directory.name) / "trace.jsonl"
        path.write_text(text, encoding="utf-8")
        return path

    def test_canonical_trace_is_order_independent(self) -> None:
        header = json.loads(json.dumps(HEADER))
        header["window"]["qualified_final_tmap_count"] = 2
        header["window"]["lifecycle_transcript"][7]["final_tmap_count"] = 2
        first = build_trace(header, EVENTS + [EVENTS[0]])
        second = build_trace(header, list(reversed(EVENTS)) + [EVENTS[0]])
        self.assertEqual(first, second)
        path = self.write(first)
        result = qualify_trace(path)
        self.assertEqual(result["music_starts"], 0)
        self.assertEqual(result["qualified_epoch"], 2)
        self.assertEqual(result["skipped_presents"], 1)
        _, events = parse_trace(path)
        tmap = next(event for event in events if event["category"] == "tmap")
        self.assertEqual(tmap["count"], 2)

    def test_tampered_event_hash_is_rejected(self) -> None:
        text = build_trace(HEADER, EVENTS).replace('"selector":7', '"selector":8')
        with self.assertRaisesRegex(ValueError, "hash mismatch"):
            qualify_trace(self.write(text))

    def test_ambiguous_selector_is_rejected(self) -> None:
        events = [dict(event) for event in EVENTS]
        events[0]["resolution"] = "ambiguous"
        with self.assertRaisesRegex(ValueError, "ambiguous"):
            qualify_trace(self.write(build_trace(HEADER, events)))

    def test_missing_present_is_rejected(self) -> None:
        header = json.loads(json.dumps(HEADER))
        transcript = header["window"]["lifecycle_transcript"]
        transcript[:] = [
            row for row in transcript if row["kind"] != "qualified_subblit"
        ]
        for seq, row in enumerate(transcript, 1):
            row["seq"] = seq
        with self.assertRaisesRegex(ValueError, "qualified_subblit"):
            qualify_trace(self.write(build_trace(header, EVENTS)))

    def test_lifecycle_cannot_enter_canonical_asset_set(self) -> None:
        events = EVENTS + [{"category": "lifecycle", "phase": "closed"}]
        with self.assertRaisesRegex(ValueError, "lifecycle"):
            qualify_trace(self.write(build_trace(HEADER, events)))

    def test_armed_empty_sound_and_cursor_complete_categories(self) -> None:
        header = json.loads(json.dumps(HEADER))
        header["window"]["empty_category_evidence"] = {
            "cursor": {
                "epoch": 2,
                "hook": "DrawCursor_8C635:post_clip_visible_extent",
                "hook_armed": True,
                "actual_entry_count": 0,
                "reason": "cursor_draw_gate_false",
                "active_selector_state": 9,
            },
            "sound": {
                "epoch": 2,
                "hook": "SOUND_start_sample:after_unit_test_guard",
                "hook_armed": True,
                "actual_entry_count": 0,
                "reason": "no_actual_sample_start_in_qualified_epoch",
                "resolved_hook_proof": resolved_hook_proof("sound"),
            },
        }
        events = EVENTS + [
            {"category": category}
            for category in ("terrain", "sky", "bitmap", "text", "glyph")
        ]
        result = qualify_trace(
            self.write(build_trace(header, events)),
            require_complete_categories=True,
        )
        self.assertNotIn("cursor", result["categories"])
        self.assertNotIn("sound", result["categories"])

    def test_pending_empty_audio_hook_is_rejected(self) -> None:
        header = json.loads(json.dumps(HEADER))
        proof = resolved_hook_proof("sound")
        proof["close"]["breakpoint_pending"] = True
        header["window"]["empty_category_evidence"] = {
            "sound": {
                "epoch": 2,
                "hook": "SOUND_start_sample:after_unit_test_guard",
                "hook_armed": True,
                "actual_entry_count": 0,
                "reason": "no_actual_sample_start_in_qualified_epoch",
                "resolved_hook_proof": proof,
            }
        }
        with self.assertRaisesRegex(ValueError, "pending"):
            qualify_trace(
                self.write(build_trace(header, EVENTS)),
                require_complete_categories=True,
            )

    def test_empty_evidence_cannot_mask_an_observed_category(self) -> None:
        header = json.loads(json.dumps(HEADER))
        header["window"]["empty_category_evidence"] = {
            "sound": {
                "epoch": 2,
                "hook": "SOUND_start_sample:after_unit_test_guard",
                "hook_armed": True,
                "actual_entry_count": 0,
                "reason": "none",
            }
        }
        events = EVENTS + [{"category": "sound"}]
        with self.assertRaisesRegex(ValueError, "both asset events"):
            qualify_trace(self.write(build_trace(header, events)))

    def test_empty_cursor_post_clip_proof_requires_exact_source(self) -> None:
        header = json.loads(json.dumps(HEADER))
        header["window"]["empty_category_evidence"] = {
            "cursor": {
                "epoch": 2,
                "hook": "DrawCursor_8C635:post_clip_visible_extent",
                "hook_armed": True,
                "actual_entry_count": 0,
                "reason": "cursor_post_clip_extent_empty",
                "active_selector_state": 9,
                "post_clip_state": {
                    "bank": "POINTERS",
                    "nominal_selector": 9,
                    "source_selector": 0,
                    "resolution": "ambiguous",
                    "visible_width": 0,
                    "visible_height": 0,
                    "post_clip_hook_reached": True,
                },
            },
        }
        with self.assertRaisesRegex(ValueError, "post-clip"):
            qualify_trace(self.write(build_trace(header, EVENTS)))

    def test_non_event_jsonl_row_is_rejected(self) -> None:
        lines = build_trace(HEADER, EVENTS).splitlines()
        lines[1] = json.dumps({"wrong": {}})
        with self.assertRaisesRegex(ValueError, "must be events"):
            qualify_trace(self.write("\n".join(lines) + "\n"))

    def test_skipped_present_must_prove_return(self) -> None:
        header = json.loads(json.dumps(HEADER))
        header["window"]["pre_window_diagnostics"][0]["return_proven"] = False
        with self.assertRaisesRegex(ValueError, "present proof"):
            qualify_trace(self.write(build_trace(header, EVENTS)))

    def test_direct_palette_present_is_accepted(self) -> None:
        header = json.loads(json.dumps(HEADER))
        diagnostic = header["window"]["pre_window_diagnostics"][0]
        diagnostic["source"] = "direct_SubBlit"
        diagnostic["pointer_class"] = "not_applicable"
        transcript = header["window"]["lifecycle_transcript"]
        transcript.pop(2)
        transcript[1]["kind"] = "excluded_direct_subblit_entry"
        transcript[2]["kind"] = "excluded_direct_subblit_return"
        for seq, row in enumerate(transcript, 1):
            row["seq"] = seq
        result = qualify_trace(self.write(build_trace(header, EVENTS)))
        self.assertEqual(result["skipped_presents"], 1)

    def test_non_null_pre_render_present_is_accepted(self) -> None:
        header = json.loads(json.dumps(HEADER))
        diagnostic = header["window"]["pre_window_diagnostics"][0]
        diagnostic["pointer_class"] = "other_non_null"
        header["window"]["lifecycle_transcript"][1][
            "pointer_class"
        ] = "other_non_null"
        result = qualify_trace(self.write(build_trace(header, EVENTS)))
        self.assertEqual(result["skipped_presents"], 1)

    def test_out_of_order_qualified_chain_is_rejected(self) -> None:
        header = json.loads(json.dumps(HEADER))
        transcript = header["window"]["lifecycle_transcript"]
        transcript[8], transcript[9] = transcript[9], transcript[8]
        transcript[8]["seq"], transcript[9]["seq"] = 9, 10
        with self.assertRaisesRegex(ValueError, "out of order"):
            qualify_trace(self.write(build_trace(header, EVENTS)))


if __name__ == "__main__":
    unittest.main()
