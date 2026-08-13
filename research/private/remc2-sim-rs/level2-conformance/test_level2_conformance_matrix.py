#!/usr/bin/env python3
"""Production-bound and mutation-negative checks for the Level 2 matrix."""

from __future__ import annotations

import copy
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

import sys
TOOLS = Path(__file__).resolve().parents[1] / "tools"
sys.path.insert(0, str(TOOLS))
import build_level2_matrix as builder  # noqa: E402
import validate_level2_matrix as validator  # noqa: E402
import validate_level1_matrix as generic  # noqa: E402


class Level2ConformanceMatrixTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.matrix = json.loads(builder.MATRIX.read_text(encoding="utf-8"))

    def test_production_denominators_are_discriminating(self) -> None:
        """Join independent builders and exercise live/non-live branch values."""
        counts = validator.validate_coverage(self.matrix)
        self.assertEqual(counts, {
            "authored": 1200, "reset": 1000, "activation_groups": 33,
            "producer_edges": 31, "objectives": 5, "dispatch": 845,
            "tuples": 373, "ai_helpers": 79, "rng": 494, "audio": 119,
            "cache": 1008, "presentation_refs": 1361,
        })
        ledger = json.loads((builder.HERE / "denominator" / "level2-dispatch-rng-audio-presentation.json").read_text())
        self.assertEqual(
            {row["classification"] for row in ledger["dispatch"]["rows"] if row["runtime_class"] == 10 and row["table_kind"] == "action" and row["selector_word_4"] in {45, 48}},
            {"reachable_level2", "mechanically_unreachable_level2"},
        )
        self.assertEqual(ledger["audio"]["counts"]["reachable_level2"], 26)
        self.assertEqual(ledger["presentation"]["cache_denominator"], 1008)
        self.assertEqual(ledger["presentation"]["source_reference_denominator"], 1361)
        summary = validator.validate(builder.MATRIX)
        self.assertFalse(summary["closed"])
        # The owning plan is the live open-bead registry.  Closed aggregate
        # beads can transfer their still-open denominator work to a current
        # route/qualification bead, so this count must follow the generated
        # matrix rather than a stale historical plan total.
        self.assertEqual(summary["non_exact_rows"], len(self.matrix["rows"]))

    def test_mutation_negative_rejects_duplicate_denominator_owner(self) -> None:
        value = copy.deepcopy(self.matrix)
        first = next(row for row in value["rows"] if any(row["coverage"].values()))
        second = next(row for row in value["rows"] if row is not first)
        family = next(key for key, ids in first["coverage"].items() if ids)
        second["coverage"][family].append(first["coverage"][family][0])
        with self.assertRaisesRegex(ValueError, "multiply owned"):
            validator.validate_coverage(value)

    def test_worker_ownership_is_disjoint_and_complete(self) -> None:
        validator.validate_source_and_ownership(self.matrix)
        counts = validator.validate_coverage(self.matrix)
        self.assertEqual(
            [row["plan_item"] for row in self.matrix["rows"] if not any(row["coverage"].values())],
            [],
        )
        covered = {
            family: sum(
                len(entry["coverage"][family])
                for entry in self.matrix["rows"] + self.matrix["exclusions"]
            )
            for family in validator.FAMILIES
        }
        self.assertEqual(covered, counts)
        dispatch_exclusion = next(
            item for item in self.matrix["exclusions"]
            if item["id"] == "EXC-L2-DISPATCH-NONROOTED"
        )
        excluded_count = len(dispatch_exclusion["coverage"]["dispatch"])
        self.assertEqual(
            dispatch_exclusion["scope"],
            f"{excluded_count} non-rooted/disabled/null dispatch slots",
        )

    def test_owner_registry_is_invariant_under_tracker_status(self) -> None:
        statuses = ("open", "in_progress", "blocked", "deferred", "closed")
        records = [
            {
                "id": identifier,
                "title": identifier,
                "description": "worker-ready implementation packet",
                "status": statuses[index % len(statuses)],
            }
            for index, identifier in enumerate(sorted(builder.DENOMINATOR_OWNER_IDS))
        ]
        records.append({"id": "mc-unrelated", "title": "unrelated", "status": "open"})
        with tempfile.TemporaryDirectory() as directory:
            registry = Path(directory) / "issues.jsonl"
            registry.write_text(
                "".join(json.dumps(record) + "\n" for record in records),
                encoding="utf-8",
            )
            with mock.patch.object(builder, "BEADS", registry):
                self.assertEqual(set(builder.beads()), set(builder.DENOMINATOR_OWNER_IDS))

    def test_closed_spell_owner_row_retains_runtime_evidence_without_claiming_exact(self) -> None:
        row = next(
            row for row in self.matrix["rows"]
            if row["id"] == "ROW-mc-xcgi.1.1.1"
        )
        self.assertEqual(row["classification"], "Missing")
        self.assertFalse(row["implementation"]["absent"])
        self.assertEqual(row["source"]["symbols"], ["sub_68FF0", "sub_692A0"])
        tests = {
            binding["test"]
            for binding in row["evidence_bindings"]
            if binding["kind"] == "rust-test"
        }
        self.assertEqual(
            tests,
            {
                "native_world::tests::level_two_spell_owner_scan_and_source_gates_are_complete",
                "native_world::tests::level_two_outer_turn_family12_honors_remote_owner_and_return_hand_override",
            },
        )
        self.assertNotIn("allocate_level2_spell_replacement", json.dumps(row))

    def test_reset_and_constructor_owners_are_exact_stable_ids(self) -> None:
        census = json.loads(
            (builder.HERE / "census" / "level2-authored-reset-census.json").read_text()
        )
        ledger = json.loads(
            (builder.HERE / "denominator" / "level2-dispatch-rng-audio-presentation.json").read_text()
        )
        active_reset = {row["id"] for row in census["reset"]["slots"] if row["active"]}
        constructors = {
            row["id"]
            for row in ledger["tuple_denominator"]["rows"]
            if row["id"].startswith("CTOR-L2-")
        }
        self.assertEqual(set(builder.RESET_OWNER), active_reset)
        self.assertEqual(set(builder.CTOR_OWNER), constructors)
        self.assertNotIn("RST-L2-0115", builder.RESET_OWNER)
        self.assertNotIn("CTOR-L2-05-001-999", builder.CTOR_OWNER)

    def test_reachable_presentation_symbols_have_no_fallback_owner(self) -> None:
        ledger = json.loads(
            (builder.HERE / "denominator" / "level2-dispatch-rng-audio-presentation.json").read_text()
        )
        unresolved = [
            row["id"]
            for row in ledger["presentation"]["source_references"]
            if row["classification"] == "reachable_level2"
            and row["path"] not in builder.PRESENTATION_PATH_OWNER
            and (row["path"], row["enclosing_symbol"])
            not in builder.PRESENTATION_SYMBOL_OWNER
        ]
        self.assertEqual(unresolved, [])
        self.assertNotIn(
            ("remc2/engine/EventsFunctions.cpp", "new_reachable_symbol"),
            builder.PRESENTATION_SYMBOL_OWNER,
        )
        self.assertEqual(
            {
                symbol: builder.PRESENTATION_SYMBOL_OWNER[
                    ("remc2/engine/EventsFunctions.cpp", symbol)
                ]
                for symbol in ("sub_14530", "sub_15790", "sub_15910", "sub_5EFA0", "sub_5F660")
            },
            {
                "sub_14530": "mc-6dqd.1",
                "sub_15790": "mc-1g06.1",
                "sub_15910": "mc-1g06.1",
                "sub_5EFA0": "mc-6dqd.1",
                "sub_5F660": "mc-1g06.2",
            },
        )
        self.assertEqual(
            builder.PRESENTATION_SYMBOL_OWNER[
                ("remc2/engine/GameUI.cpp", "DrawSpellIcon_2E260")
            ],
            "mc-a73w.1",
        )

    def test_activation_reachable_dispatch_cannot_be_excluded(self) -> None:
        value = copy.deepcopy(self.matrix)
        identifier = "DSP-L2-089"
        owner = next(row for row in value["rows"] if identifier in row["coverage"]["dispatch"])
        exclusion = next(item for item in value["exclusions"] if item["id"] == "EXC-L2-DISPATCH-NONROOTED")
        owner["coverage"]["dispatch"].remove(identifier)
        exclusion["coverage"]["dispatch"].append(identifier)
        with self.assertRaisesRegex(ValueError, "activation-reachable dispatch"):
            validator.validate_coverage(value)

    def test_whirlwind_action82_uses_class10_model_gate_not_class5_dispatch(self) -> None:
        ledger = json.loads(
            (builder.HERE / "denominator" / "level2-dispatch-rng-audio-presentation.json").read_text()
        )
        rows = {row["id"]: row for row in ledger["dispatch"]["rows"]}
        self.assertEqual(
            (rows["DSP-L2-155"]["runtime_class"], rows["DSP-L2-155"]["selector_word_4"],
             rows["DSP-L2-155"]["resolved_function"]),
            (5, 82, "sub_22540"),
        )
        self.assertEqual(
            (rows["DSP-L2-492"]["runtime_class"], rows["DSP-L2-492"]["selector_word_4"],
             rows["DSP-L2-492"]["classification"]),
            (10, 82, "disabled_slot"),
        )
        self.assertNotIn("DSP-L2-155", builder.DISPATCH_CONTRADICTION_OWNER)
        exclusion = next(
            item for item in self.matrix["exclusions"]
            if item["id"] == "EXC-L2-DISPATCH-NONROOTED"
        )
        self.assertIn("DSP-L2-155", exclusion["coverage"]["dispatch"])
        self.assertIn("DSP-L2-492", exclusion["coverage"]["dispatch"])

    def test_mutation_negative_rejects_stale_source_and_missing_chain(self) -> None:
        value = copy.deepcopy(self.matrix)
        value["rows"][0]["source"]["commit"] = "0" * 40
        with self.assertRaisesRegex(ValueError, "pinned source anchor"):
            validator.validate_source_and_ownership(value)

    def test_sealed_level1_inventory_identity_cannot_be_rewritten(self) -> None:
        canonical = generic.load_matrix(
            generic.SIM_WORKSPACE_ROOT / "level1-conformance" / "level1-conformance-matrix.json"
        )
        canonical["meta"]["test_inventory"]["count"] = generic.EXPECTED_LEVEL1_CARGO_TESTS - 1
        profile = generic.level1_validation_profile()
        inventory = generic.capture_cargo_test_inventory(generic.SIM_WORKSPACE_ROOT)
        with self.assertRaisesRegex(generic.ValidationError, "historical inventory"):
            generic.validate_document(canonical, inventory, profile)
        value = copy.deepcopy(self.matrix)
        value["exclusions"][0]["reachability"]["chain"] = ""
        with self.assertRaisesRegex(ValueError, "reachability chain"):
            validator.validate_source_and_ownership(value)

    def test_builder_is_byte_stable(self) -> None:
        result = subprocess.run(
            [sys.executable, str(builder.HERE.parent / "tools" / "build_level2_matrix.py"), "--check"],
            cwd=builder.ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
