#!/usr/bin/env python3
"""Focused conformance checks for the derived Level 2 activation closure."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path


HERE = Path(__file__).resolve().parent
BUILDER_PATH = HERE / "build_level2_activation_closure.py"
SPEC = importlib.util.spec_from_file_location("level2_activation_builder", BUILDER_PATH)
assert SPEC and SPEC.loader
BUILDER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILDER)


class Level2ActivationClosureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.path = HERE / "level2-activation-closure.json"
        cls.data = json.loads(cls.path.read_text())

    def test_committed_artifact_is_a_pinned_derivation(self) -> None:
        BUILDER.validate(self.path)
        self.assertEqual(self.data["spec"]["remc2_commit"], BUILDER.COMMIT)
        self.assertEqual(self.data["authored_rows"]["count"], 1200)
        self.assertEqual(self.data["unresolved"], [])

    def test_source_anchors_are_present_and_pinned(self) -> None:
        symbols = {source["symbol"] for source in self.data["sources"]}
        self.assertIn("LevelInitGame_56A30_root", symbols)
        self.assertIn("sub_4A1E0_group_filter", symbols)
        self.assertIn("sub_4A310_class10_model45", symbols)
        self.assertIn("sub_4A310_class11", symbols)
        self.assertIn("sub_58F00_game_objectives", symbols)
        self.assertIn("sub_6F100", symbols)
        self.assertIn("sub_6F0B0", symbols)
        self.assertIn("sub_6F300", symbols)
        self.assertIn("InitStages_58940", symbols)
        self.assertIn("sub_58F00_objective_switch", symbols)
        self.assertIn("IfSubtypeCallCreatingManaSphere_4A190", symbols)
        self.assertIn("sub_58F00_level_end_state", symbols)
        self.assertIn("sub_58F00_local_player_level_end", symbols)
        for source in self.data["sources"]:
            self.assertEqual(len(source["source_sha256"]), 64)
            self.assertGreater(source["line"], 0)

    def test_rooted_producer_graph_is_complete(self) -> None:
        self.assertEqual(
            self.data["root"]["rooted_groups"],
            list(range(0, 30)) + [31, 32],
        )
        edges = self.data["producer_edges"]
        self.assertEqual(len(edges), 31)
        edge_pairs = {(edge["source_group"], edge["target_group"]) for edge in edges}
        for pair in ((0, 1), (1, 2), (2, 3), (3, 4), (8, 22), (8, 27), (31, 32), (23, 24), (25, 26)):
            self.assertIn(pair, edge_pairs)
        self.assertNotIn((3, 0), edge_pairs)
        self.assertEqual(self.data["terminal"]["group_30"]["status"], "mechanically_excluded")
        self.assertEqual(self.data["terminal"]["group_30"]["authored_ids"], [f"AUT-L2-{index:04d}" for index in range(443, 448)])
        self.assertEqual(self.data["terminal"]["group_32"]["execution_status"], "conditionally_reachable")
        self.assertEqual(self.data["terminal"]["group_32"]["consumer_status"], "mechanically_noop")
        self.assertEqual(self.data["terminal"]["level_end"]["dispatch_call_id"], "DISPATCH-CALL-54383")

        groups = {group["group"]: group for group in self.data["groups"]}
        self.assertEqual(groups[30]["authored_row_count"], 5)
        self.assertEqual(groups[30]["dispatchable_row_count"], 0)
        self.assertEqual(groups[30]["consumer_status"], "mechanically_noop")
        self.assertIn("EDGE-AUT-L2-0209-stage-tag", groups[32]["incoming_edges"])
        self.assertEqual(groups[32]["execution_status"], "conditionally_reachable")
        self.assertEqual(groups[32]["consumer_status"], "mechanically_noop")

    def test_every_authored_row_has_one_of_four_explicit_outcomes(self) -> None:
        rows = self.data["authored_rows"]["rows"]
        self.assertEqual(len(rows), 1200)
        allowed = {"reachable", "conditionally_reachable", "deliberate_no_call", "mechanically_excluded"}
        self.assertTrue(all(row["closure_state"] in allowed for row in rows))
        counts = self.data["authored_rows"]["closure_state_counts"]
        self.assertEqual(counts, {
            "conditionally_reachable": 268,
            "deliberate_no_call": 755,
            "mechanically_excluded": 87,
            "reachable": 90,
        })
        self.assertEqual(sum(counts.values()), 1200)

    def test_every_row_has_exactly_one_planned_owner(self) -> None:
        rows = self.data["authored_rows"]["rows"]
        self.assertEqual(len({row["authored_id"] for row in rows}), 1200)
        owners = {row["planned_owner"] for row in rows}
        self.assertIn("root_group_zero_dispatch", owners)
        self.assertIn("GenerateEvents_49290_model45_preprocess", owners)
        self.assertIn("none", owners)
        for row in rows:
            if row["planned_owner"] == "none":
                self.assertEqual(row["incoming_edge_ids"], [])
            elif row["planned_owner"].startswith("group_"):
                self.assertTrue(row["incoming_edge_ids"], row)

    def test_dormant_candidate_classes_are_resolved(self) -> None:
        classes = {(entry["class"], entry["subtype"]): entry for entry in self.data["target_classes"]}
        for candidate in BUILDER.TARGET_CLASSES:
            self.assertIn(candidate, classes)
            self.assertTrue(classes[candidate]["resolved"], candidate)
            self.assertGreater(classes[candidate]["authored_count"], 0)
            self.assertEqual(classes[candidate]["constructor_anchor"], "IfSubtypeCallCreatingManaSphere_4A190")
            self.assertTrue(classes[candidate]["branch_evidence"]["marker"])
            self.assertGreater(classes[candidate]["branch_evidence"]["line"], 0)
        self.assertEqual(classes[(5, 9)]["branch_evidence"]["branch"], "class5.model9_sub12100_flag")
        for model in (2, 3, 4, 14):
            self.assertEqual(classes[(5, model)]["branch_evidence"]["branch"], "class5.generic")

    def test_objective_identity_and_semantics(self) -> None:
        objectives = self.data["objectives"]
        self.assertEqual([objective["objective_type"] for objective in objectives], [5, 3, 7, 5, 9])
        self.assertEqual(objectives[0]["target"], {"x": 2, "y": 152})
        self.assertEqual(objectives[1]["target"]["kind"], "player")
        self.assertEqual(objectives[1]["target"]["runtime_relation"], 113)
        self.assertEqual(objectives[2]["target"]["index"], 229)
        self.assertEqual(objectives[2]["target"]["authored_id"], "AUT-L2-0229")
        self.assertEqual(objectives[3]["target"], {"x": 154, "y": 229})
        self.assertEqual(objectives[4]["target"]["index"], 153)
        self.assertEqual(objectives[4]["target"]["authored_id"], "AUT-L2-0153")
        self.assertEqual(objectives[4]["target"]["chain_root"], 21)

    def test_all_production_dispatch_callsites_are_anchored(self) -> None:
        calls = self.data["dispatch_calls"]
        self.assertEqual(
            [call["line"] for call in calls],
            [28173, 39425, 39474, 44551, 44563, 54361, 54383, 54412, 54463, 54519, 54537],
        )
        self.assertEqual(calls[0]["context"], "building_action")
        self.assertEqual(calls[1]["context"], "LevelInitGame_56A30")
        self.assertEqual(calls[-2]["context"], "sub_6F300")
        self.assertIn("sub_4A1E0", calls[0]["expression"])

    def test_production_derivation_discriminates_root_chain_sentinel_and_objectives(self) -> None:
        command = [
            "python3",
            str(HERE / "build_level2_activation_closure.py"),
            "--check",
        ]
        subprocess.run(command, cwd=BUILDER.ROOT, check=True, capture_output=True, text=True)
        rows = {row["index"]: row for row in self.data["authored_rows"]["rows"]}
        self.assertEqual(rows[11]["planned_owner"], "root_group_zero_dispatch")
        self.assertEqual(rows[153]["activation_group"], 8)
        self.assertEqual(rows[153]["planned_owner"], "group_8_dispatch")
        self.assertEqual(rows[153]["closure_state"], "conditionally_reachable")
        self.assertEqual(rows[1]["route"], "GenerateEvents_49290_model45")
        self.assertIn("par1=55", rows[1]["proof"])
        self.assertEqual(rows[108]["producer_effect"], "no_op_guarded_xtype_zero")
        edge_pairs = {(edge["source_group"], edge["target_group"]) for edge in self.data["producer_edges"]}
        self.assertTrue({(0, 1), (1, 2), (8, 22), (8, 27), (31, 32)} <= edge_pairs)
        self.assertNotIn((3, 0), edge_pairs)
        self.assertEqual([objective["objective_type"] for objective in self.data["objectives"]], [5, 3, 7, 5, 9])

    def test_validator_rejects_a_tampered_artifact(self) -> None:
        tampered = json.loads(self.path.read_text())
        tampered["producer_edges"][0]["target_group"] += 1000
        # The shared machine's /tmp tmpfs is intentionally not part of this
        # conformance boundary; keep the negative fixture beside this test so
        # a full tmpfs cannot mask a validator failure.
        with tempfile.NamedTemporaryFile("w", suffix=".json", dir=HERE) as stream:
            json.dump(tampered, stream)
            stream.flush()
            with self.assertRaises(ValueError):
                BUILDER.validate(Path(stream.name))


if __name__ == "__main__":
    unittest.main()
