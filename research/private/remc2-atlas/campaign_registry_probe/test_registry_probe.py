from __future__ import annotations

import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parents[3]
if str(HERE) not in sys.path:
    sys.path.insert(0, str(HERE))

import compare_registry
import extract_registry


class RegistryProbeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.inventory = extract_registry.extract(REPO_ROOT)

    def test_pinned_inventory_shape_and_row_counts(self) -> None:
        inventory = self.inventory
        self.assertEqual(inventory["source"]["commit"], extract_registry.PINNED_COMMIT)
        self.assertEqual(inventory["source"]["read_mode"], "git_show_object")
        self.assertEqual(inventory["source"]["class_table"]["definition_line"], 2060)
        self.assertEqual(inventory["class_count"], 17)
        self.assertEqual(
            inventory["summary"],
            {
                "action_callback_cases": 798,
                "action_non_sentinel_rows": 546,
                "action_rows_including_sentinels": 561,
                "class_rows_including_sentinel": 17,
                "class_sentinel_rows": 1,
                "constructor_callback_cases": 236,
                "constructor_non_sentinel_rows": 269,
                "constructor_rows_including_sentinels": 284,
                "source_behavior_key_count": 486,
            },
        )
        self.assertFalse(inventory["classes"][0]["is_null_sentinel"])
        self.assertTrue(inventory["classes"][16]["is_null_sentinel"])
        self.assertEqual(
            {
                item["class_index"]: (
                    item["action_declared_size"],
                    item["constructor_declared_size"],
                )
                for item in inventory["classes"]
                if item["class_index"] not in (0, 16)
            },
            {
                1: (1, 10),
                2: (23, 10),
                3: (14, 13),
                4: (1, 1),
                5: (236, 30),
                6: (1, 1),
                7: (1, 1),
                8: (1, 1),
                9: (33, 32),
                10: (100, 93),
                11: (46, 46),
                12: (1, 1),
                13: (11, 11),
                14: (12, 7),
                15: (80, 27),
            },
        )

    def test_rows_are_complete_and_selected_callbacks_are_resolved(self) -> None:
        rows = [
            row
            for item in self.inventory["classes"]
            for row in item["action_rows"] + item["constructor_rows"]
        ]
        self.assertEqual(len(rows), 561 + 284)
        self.assertTrue(
            all(
                row["source"]["file"] == extract_registry.TABLE_SOURCE_PATH
                and row["array_name"]
                and row["table_kind"] in {"action", "constructor"}
                for row in rows
            )
        )
        source_keys = {
            item["key"]: item for item in self.inventory["source_behavior_keys"]
        }
        self.assertEqual(source_keys["2:0"]["callback_identity"], "AddTree02_00_64E20")
        self.assertEqual(source_keys["3:0"]["callback_identity"], "AddPlayer03_00_5E010")
        self.assertNotIn("10:84", source_keys)
        disabled_84 = next(
            row
            for row in self.inventory["classes"][10]["action_rows"]
            if row["selector_word_4"] == 84
        )
        self.assertEqual(disabled_84["resolution_status"], "DISABLED_NULL_ADDRESS")

    def test_extraction_is_byte_deterministic(self) -> None:
        first = extract_registry.json_bytes(extract_registry.extract(REPO_ROOT))
        second = extract_registry.json_bytes(extract_registry.extract(REPO_ROOT))
        self.assertEqual(first, second)

    def test_checked_in_artifacts_and_diff_gate(self) -> None:
        inventory_path = HERE / "str-d4c48ar-inventory.json"
        report_path = HERE / "rust-diff.json"
        self.assertEqual(inventory_path.read_bytes(), extract_registry.json_bytes(self.inventory))
        report = compare_registry.build_report(REPO_ROOT)
        self.assertEqual(report_path.read_bytes(), compare_registry.json_bytes(report))
        self.assertEqual(
            report["summary"],
            {
                "implemented_missing_from_source_count": 0,
                "matching_key_count": 127,
                "source_not_yet_implemented_count": 359,
            },
        )
        self.assertEqual(report["registry"]["entry_count"], 127)
        self.assertEqual(report["registry"]["entry_group_count"], 24)


if __name__ == "__main__":
    unittest.main()
