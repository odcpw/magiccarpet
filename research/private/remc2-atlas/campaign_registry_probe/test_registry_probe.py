from __future__ import annotations

import copy
import json
import sys
import tempfile
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
                "source_constructor_key_count": 238,
                "source_site_key_count": 1448,
                "source_site_key_counts": {
                    "ApplyEvents": 486,
                    "Constructor": 238,
                    "TypeSubtype": 238,
                    "UpdateEntities": 486,
                },
            },
        )
        self.assertEqual(
            {
                name: (site["function"], site["line"])
                for name, site in inventory["source"]["selection_sites"].items()
            },
            {
                "ApplyEvents": ("void ApplyEvents_498A0()", 410),
                "Constructor": (
                    "type_entity_0x6E8E* pre_sub_4A190_axis_3d",
                    4166,
                ),
                "TypeSubtype": (
                    "type_entity_0x6E8E* IfSubtypeCallCreatingManaSphere_4A190",
                    5186,
                ),
                "UpdateEntities": ("void UpdateEntities_57730()", 39928),
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
        site_keys = {item["key"] for item in self.inventory["source_site_keys"]}
        self.assertIn("ApplyEvents:ClassAction:5:9", site_keys)
        self.assertIn("UpdateEntities:ClassAction:5:9", site_keys)
        self.assertIn("Constructor:ClassModel:10:45", site_keys)
        self.assertIn("TypeSubtype:TypeSubtype:10:45", site_keys)

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
                "implemented_site_key_missing_from_source_count": 0,
                "matching_key_count": 127,
                "matching_site_key_count": 315,
                "source_not_yet_implemented_count": 359,
                "source_site_not_yet_implemented_count": 1133,
            },
        )
        self.assertEqual(report["registry"]["entry_count"], 127)
        self.assertEqual(report["registry"]["entry_group_count"], 24)
        self.assertEqual(report["registry"]["compiled_site_key_count"], 315)
        self.assertEqual(report["registry"]["constructor_key_count"], 60)
        self.assertEqual(report["registry"]["type_subtype_key_count"], 1)

    def write_registry(self, payload: dict[str, object], root: Path) -> Path:
        path = root / "registry.json"
        path.write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")
        return path

    def test_planted_registry_negatives_fail_closed(self) -> None:
        original = json.loads((HERE / "rust_behavior_registry.json").read_text())
        with tempfile.TemporaryDirectory(prefix="mc-cf03-negative-") as directory:
            root = Path(directory)

            # Missing pinned source key / extra Rust claim.
            extra = copy.deepcopy(original)
            extra["entries"][0]["level_scopes"]["1"].append(84)
            with self.assertRaisesRegex(ValueError, "absent from pinned source"):
                compare_registry.build_report(
                    REPO_ROOT, self.write_registry(extra, root)
                )

            # An otherwise identical class/action may not change site identity.
            wrong_site = copy.deepcopy(original)
            wrong_site["action_dispatch_sites"] = ["UpdateEntities", "ApplyEvents"]
            with self.assertRaisesRegex(ValueError, "preserve ApplyEvents"):
                compare_registry.load_registry(self.write_registry(wrong_site, root))

            # Constructor selection cannot be flattened into an action site.
            flattened = copy.deepcopy(original)
            flattened["constructor_dispatch_site"] = "UpdateEntities"
            with self.assertRaisesRegex(ValueError, "flattened"):
                compare_registry.load_registry(self.write_registry(flattened, root))

            # Declaration order is part of the deterministic compiled surface.
            permuted = copy.deepcopy(original)
            permuted["entries"][0], permuted["entries"][1] = (
                permuted["entries"][1],
                permuted["entries"][0],
            )
            with self.assertRaisesRegex(ValueError, "source-key order"):
                compare_registry.load_registry(self.write_registry(permuted, root))

            # Removing a pinned inventory key rejects the corresponding claim.
            source_keys = {item["key"] for item in self.inventory["source_site_keys"]}
            _, _, _, registry_keys = compare_registry.load_registry(
                HERE / "rust_behavior_registry.json"
            )
            source_keys.remove(next(iter(registry_keys)))
            with self.assertRaisesRegex(ValueError, "absent from pinned source"):
                compare_registry.require_implemented_keys_present(
                    source_keys, set(registry_keys), "planted source inventory"
                )

    def test_probe_is_inventory_only_and_registry_has_no_dynamic_registration(self) -> None:
        extractor_source = (HERE / "extract_registry.py").read_text(encoding="utf-8")
        dispatch_source = (
            REPO_ROOT
            / "research/private/remc2-sim-rs/crates/sim-core/src/campaign/dispatch.rs"
        ).read_text(encoding="utf-8")
        self.assertNotIn("emit_rust", extractor_source)
        self.assertNotIn("quote!", extractor_source)
        for forbidden in (
            "pub fn register",
            "fn register_handler",
            "HashMap<BehaviorKey",
            "Mutex<",
            "RwLock<",
            "OnceLock<",
        ):
            self.assertNotIn(forbidden, dispatch_source)


if __name__ == "__main__":
    unittest.main()
