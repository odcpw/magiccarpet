"""Production-path and discriminating tests for the Level 2 denominator."""

from __future__ import annotations

import json
import subprocess
import unittest
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[5]
BUILDER = HERE / "build_level2_denominator.py"
OUTPUT = HERE / "level2-dispatch-rng-audio-presentation.json"
COMMIT = "6806c7204d17220ecc0430759f784aea8ee2ddb2"


class Level2DenominatorTests(unittest.TestCase):
    def load(self) -> dict:
        return json.loads(OUTPUT.read_text())

    def test_production_builder_is_byte_stable_and_pinned(self) -> None:
        result = subprocess.run(
            ["python3", str(BUILDER), "--check"],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        data = self.load()
        self.assertEqual(data["spec"]["remc2_commit"], COMMIT)
        self.assertTrue(data["validation"]["all_rows_classified"])
        self.assertTrue(data["validation"]["stable_ids_unique"])

    def test_dispatch_join_preserves_active_and_excluded_slots(self) -> None:
        data = self.load()
        self.assertEqual(data["dispatch"]["denominator"], 845)
        rows = data["dispatch"]["rows"]
        self.assertEqual(len(rows), 845)
        self.assertEqual(sum(row["classification"] in {"reachable_level2", "reachable_no_call"} for row in rows), 58)
        # Discriminating values: an authored class-10/model-45 action is live,
        # while the nearby enabled class-10 action 48 is explicitly out of
        # Level 2 (action 46 is a disabled table slot).
        live = next(row for row in rows if row["runtime_class"] == 10 and row["table_kind"] == "action" and row["selector_word_4"] == 45)
        excluded = next(row for row in rows if row["runtime_class"] == 10 and row["table_kind"] == "action" and row["selector_word_4"] == 48)
        self.assertEqual(live["classification"], "reachable_level2")
        self.assertEqual(excluded["classification"], "mechanically_unreachable_level2")
        empty_case = next(row for row in rows if row["runtime_class"] == 14 and row["table_kind"] == "action" and row["selector_word_4"] == 3)
        self.assertEqual(empty_case["classification"], "reachable_no_call")
        self.assertEqual(empty_case["resolved_function"], "")
        self.assertEqual({row["classification"] for row in rows} & {"unclassified"}, set())

    def test_ai_modes_and_common_tail_order_are_explicit(self) -> None:
        ai = self.load()["ai_helper_ledger"]
        self.assertEqual(ai["root"], "sub_12910")
        self.assertEqual(len(ai["mode_cases"]), 14)
        self.assertEqual([row["mode"] for row in ai["mode_cases"]], [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 14])
        self.assertEqual(ai["mode_cases"][0]["tail_call_count"], 2)
        self.assertTrue(any(edge["caller"] == "sub_12910" and edge["callee"] == "sub_12A70" for edge in ai["edges"]))
        self.assertTrue(any(edge["caller"] == "sub_12910" and edge["callee"] == "sub_12E70" for edge in ai["edges"]))
        self.assertEqual(ai["counts"]["unclassified"], 0)
        self.assertFalse(any(edge["caller"] == edge["callee"] for edge in ai["edges"]))
        self.assertTrue(set(ai["required_roles"]).issubset(ai["role_counts"]))

    def test_audio_identity_and_rng_denominators_have_no_gaps(self) -> None:
        data = self.load()
        audio = data["audio"]
        self.assertEqual(audio["denominator"], 119)
        self.assertEqual(len({row["stable_occurrence_hash_sha256"] for row in audio["rows"]}), 119)
        self.assertEqual(audio["counts"]["reachable_level2"] + audio["counts"]["mechanically_unreachable_level2"], 119)
        self.assertTrue(all(row["classification"] == "mechanically_unreachable_level2" for row in audio["rows"] if row["branch_evidence"]["level1_exclusion_id"]))
        self.assertTrue(any(row["classification"] == "reachable_level2" and row["helper_path"] for row in audio["rows"]))
        self.assertTrue(all(row["profile"]["map_type"] == "Day" for row in audio["rows"]))
        rng = data["rng"]
        self.assertGreater(rng["denominator"], 100)
        self.assertEqual(rng["counts"].get("reachable_level2", 0) + rng["counts"].get("mechanically_unreachable_level2", 0), rng["denominator"])
        self.assertTrue(any(row["path"].endswith("Terrain.cpp") and row["rng_kind"] == "global_rng_read_or_transfer" for row in rng["rows"]))
        self.assertTrue(any(row["path"].endswith("Sound.cpp") and row["classification"] == "reachable_level2" for row in rng["rows"]))
        self.assertEqual(len({row["id"] for row in rng["rows"]}), len(rng["rows"]))
        self.assertTrue(all(row["classification"] == "mechanically_unreachable_level2" for row in rng["rows"] if row["path"].endswith("EventsFunctions.cpp") and row["line"] in {59842, 59843, 59844, 59847}))
        self.assertEqual(data["exclusions"]["unclassified"], 0)

    def test_presentation_cache_families_and_owner_split(self) -> None:
        whole = self.load()
        data = whole["presentation"]
        self.assertEqual(data["cache_denominator"], 1008)
        self.assertEqual({row["family"] for row in data["cache_rows"]}, {"entity_parameters", "particle_parameters", "sprite_selectors"})
        counts = {root["family"]: root["count"] for root in data["roots"]}
        self.assertEqual(counts, {"entity_parameters": 157, "particle_parameters": 347, "sprite_selectors": 504})
        sample = data["cache_rows"][0]["owner"]
        self.assertEqual(sample["native"], "remc2-sim-rs owner package")
        self.assertEqual(sample["unity"], "owner presentation bridge")
        self.assertGreater(data["source_reference_denominator"], 0)
        self.assertTrue(data["module_roots"]["EventsFunctions.cpp"])
        self.assertEqual(data["root_binding"]["source_references_field"], "source_references[].root_binding")
        self.assertTrue(all(not row["expression"].lstrip().startswith("#") for row in data["source_references"]))
        self.assertTrue(all(row["root_binding"] == (row["classification"] == "reachable_level2") for row in data["source_references"]))
        self.assertTrue(any(row["dependency_kind"] == "HUD" for row in data["source_references"]))
        self.assertTrue(any(row["dependency_kind"] == "music" for row in data["source_references"]))
        self.assertTrue(any(row["classification"] == "mechanically_unreachable_level2" for row in data["source_references"]))
        self.assertTrue(any(row["classification"] == "reachable_level2" for row in data["source_references"]))
        self.assertEqual(len({row["id"] for row in data["source_references"]}), len(data["source_references"]))
        for ledger in (whole["tuple_denominator"]["rows"], whole["dispatch"]["rows"], whole["audio"]["rows"], whole["ai_helper_ledger"]["rows"], data["cache_rows"]):
            self.assertEqual(len({row["id"] for row in ledger}), len(ledger))
        self.assertGreater(len(data["source_files"]), 0)
        tuples = self.load()["tuple_denominator"]["rows"]
        self.assertEqual(sum(row["binding_status"] == "exact_reset_relation" for row in tuples), 15)
        self.assertEqual(sum(row["binding_status"] == "unresolved_authored_event_no_reset_relation" for row in tuples), 358)
        self.assertGreater(len({tuple(row["presentation_binding"]) for row in tuples}), 1)


if __name__ == "__main__":
    unittest.main()
