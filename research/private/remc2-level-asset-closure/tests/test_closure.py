#!/usr/bin/env python3
from __future__ import annotations

import copy
import importlib.util
import json
import shutil
import struct
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[4]
TOOL_PATH = ROOT / "research/private/remc2-level-asset-closure/build_closure.py"
SPEC = importlib.util.spec_from_file_location("build_closure", TOOL_PATH)
assert SPEC and SPEC.loader
closure = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(closure)

EXPORT = ROOT / "research/private/generated/level1-export-m"
CATALOG = (
    ROOT
    / "research/private/remc2-atlas/semantics/generated/owner-asset-catalog.json"
)
OWNER = ROOT / "research/private/remc2-native-runtime/CD_Files"
TRACE_A = ROOT / "research/private/generated/level1-frame-trace-final-v2-a"
TRACE_B = ROOT / "research/private/generated/level1-frame-trace-final-v2-b"
LEVEL2_EXPORT = ROOT / "research/private/generated/l2-debug3.fJ3PaM/level2"
LEVEL2_V4_EXPORT = ROOT / "research/private/generated/l2-debug3.fJ3PaM-v4/level2"


class RealmMatrixTests(unittest.TestCase):
    def test_all_owner_resolution_realm_and_fog_variants(self) -> None:
        for map_type in range(3):
            for resolution in (1, 8):
                for texture_size in (16, 32):
                    for flags in (0, 2):
                        roots = closure.select_realm_assets(
                            map_type, resolution, texture_size, flags
                        )
                        letter = "DNC"[map_type]
                        prefix = "M" if resolution == 1 else "H"
                        self.assertIn(f"DATA/{prefix}SPR{letter}0-0.DAT", roots)
                        self.assertIn(f"DATA/{prefix}WEB{letter}0-0.DAT", roots)
                        self.assertIn(f"DATA/TMAPS{map_type}-0.TAB", roots)
                        self.assertIn(f"DATA/TABLES{letter}.DAT", roots)
                        if map_type == 1:
                            display = "F" if flags & 2 else "N"
                            self.assertIn(
                                f"DATA/BL{texture_size}{display}0-0.DAT", roots
                            )
                            self.assertIn(f"DATA/PAL{display}-0.DAT", roots)


class RenderModeTests(unittest.TestCase):
    def test_every_known_tmap_render_mode_is_classified(self) -> None:
        strategies = [closure.tmap_render_strategy(mode) for mode in range(37)]
        self.assertEqual(strategies[0], "base")
        self.assertEqual(strategies[17], "yaw_mirrored_16")
        self.assertEqual(strategies[19], "yaw_lookup_D4750_12")
        self.assertEqual(strategies[36], "animation_frame_transparent")
        with self.assertRaises(closure.ClosureError):
            closure.tmap_render_strategy(37)

    def test_catalog_group_expansion(self) -> None:
        data = json.loads(CATALOG.read_text())
        catalog = closure.Catalog(data)
        catalog.index_tmaps("DATA/TMAPS1-0.DAT")
        self.assertEqual([x["index"] for x in catalog.tmap_group(38)], list(range(38, 54)))
        self.assertEqual([x["index"] for x in catalog.tmap_group(152)], [152, 153, 154])

    def test_yaw_formula_endpoints_and_d4750_tables(self) -> None:
        group16 = list(range(100, 116))
        self.assertEqual(
            closure.render_choice_index(100, 17, group16, yaw_sector=0), 100
        )
        self.assertEqual(
            closure.render_choice_index(100, 17, group16, yaw_sector=7), 107
        )
        self.assertEqual(
            closure.render_choice_index(100, 17, group16, yaw_sector=8), 107
        )
        self.assertEqual(
            closure.render_choice_index(100, 17, group16, yaw_sector=15), 100
        )
        self.assertEqual(
            closure.render_choice_candidates(100, 18, group16), group16
        )
        self.assertEqual(
            closure.render_choice_candidates(100, 19, group16),
            [100, 101, 102, 103, 104],
        )
        self.assertEqual(
            closure.render_choice_candidates(100, 20, group16),
            [100, 101, 102],
        )

    def test_animation_formula_endpoints_and_bounds(self) -> None:
        group = [38, 39, 40]
        self.assertEqual(
            closure.render_choice_index(38, 36, group, animation_frame=0), 38
        )
        self.assertEqual(
            closure.render_choice_index(38, 36, group, animation_frame=2), 40
        )
        self.assertEqual(closure.render_choice_candidates(38, 36, group), group)
        with self.assertRaises(closure.ClosureError):
            closure.render_choice_index(38, 36, group, animation_frame=3)
        with self.assertRaises(closure.ClosureError):
            closure.render_choice_index(100, 18, list(range(100, 115)), yaw_sector=15)
        with self.assertRaises(closure.ClosureError):
            closure.render_choice_index(100, 18, group, yaw_sector=16)


class BuildSchemaTests(unittest.TestCase):
    def test_level1_build_indices_and_two_byte_cell_spans(self) -> None:
        entities = closure.parse_entities(EXPORT / "entities.bin")
        expected = [17, 26, 29, 30, 31, 34, 42, 47, 50, 51, 52, 53]
        self.assertEqual(entities["active_build_indices"], expected)
        footprints = closure.parse_build_footprints(OWNER, expected)
        self.assertEqual([x["index"] for x in footprints], expected)
        for item in footprints:
            self.assertEqual(
                item["span"]["length"], item["width"] * item["height"] * 2
            )
            self.assertEqual(item["cell_stride_bytes"], 2)
            self.assertNotIn("pixel", item["kind"])

    def test_malformed_build_bounds_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "DATA").mkdir()
            (root / "DATA/BUILD0-0.DAT").write_bytes(b"\0" * 8)
            (root / "DATA/BUILD0-0.TAB").write_bytes(struct.pack("<IBB", 7, 2, 2))
            with self.assertRaises(closure.ClosureError):
                closure.parse_build_footprints(root, [0])


class ObjectiveTests(unittest.TestCase):
    def test_level1_objective_and_font_glyph_join(self) -> None:
        record = closure.parse_language_record(OWNER, 48)
        self.assertEqual(record["selector"], 48)
        self.assertEqual(record["byte_length"], len(b'"Fly towards my beacon."'))
        self.assertEqual(
            record["glyph_indices"],
            [35, 47, 71, 98, 99, 100, 101, 102, 109, 110, 111, 112, 115, 116, 117, 120, 122],
        )
        catalog = closure.Catalog(json.loads(CATALOG.read_text()))
        assets = closure.resolve_glyph_assets(catalog, record)
        self.assertEqual([asset["index"] for asset in assets], record["glyph_indices"])

    def test_malformed_objective_layout_is_rejected(self) -> None:
        runtime = (EXPORT / "runtime.bin").read_bytes()
        with self.assertRaises(closure.ClosureError):
            closure.objective_language_selector(
                runtime, {"offset": len(runtime), "count": 1, "record_bytes": 11}, 1
            )
        with self.assertRaises(closure.ClosureError):
            closure.objective_language_selector(
                runtime, {"offset": 304, "count": 8, "record_bytes": 11}, 26
            )

    def test_malformed_language_and_glyph_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "LANGUAGE").mkdir()
            (root / "LANGUAGE/L1.TXT").write_bytes(
                b"\0" * 4785 + b"\0" * 48 + b'"unmatched\0'
            )
            with self.assertRaises(closure.ClosureError):
                closure.parse_language_record(root, 48)
        catalog = closure.Catalog(json.loads(CATALOG.read_text()))
        with self.assertRaises(closure.ClosureError):
            closure.resolve_glyph_assets(catalog, {"glyph_indices": [300]})


class ClosureContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.document = closure.build_closure(
            EXPORT, CATALOG, OWNER, TRACE_A, TRACE_B
        )

    def test_repeatability(self) -> None:
        again = closure.build_closure(
            EXPORT, CATALOG, OWNER, TRACE_A, TRACE_B
        )
        self.assertEqual(
            closure.canonical_json(self.document), closure.canonical_json(again)
        )

    def test_export_package_hash_is_verified(self) -> None:
        manifest = json.loads((EXPORT / "manifest.json").read_text())
        closure.verify_export_identity(EXPORT, manifest)
        bad_member = copy.deepcopy(manifest)
        bad_member["files"]["entities.bin"]["sha256"] = "0" * 64
        with self.assertRaises(closure.ClosureError):
            closure.verify_export_identity(EXPORT, bad_member)
        manifest["package_sha256"] = "0" * 64
        with self.assertRaises(closure.ClosureError):
            closure.verify_export_identity(EXPORT, manifest)

    def test_four_sets_and_qualified_frame_policy(self) -> None:
        sets = self.document["sets"]
        self.assertEqual(
            set(sets),
            {
                "realm_load_roots",
                "scene_reachable",
                "frame_used",
                "dynamic_dependencies",
            },
        )
        self.assertEqual(sets["realm_load_roots"]["unresolved"], [])
        self.assertEqual(sets["scene_reachable"]["unresolved"], [])
        self.assertEqual(len(sets["frame_used"]["assets"]), 76)
        self.assertEqual(sets["frame_used"]["unresolved"], [])
        self.assertTrue(
            self.document["qualification"]["zero_unresolved_frame_used_claimed"]
        )
        self.assertEqual(
            self.document["qualification"]["frame_used"],
            "PASS_FINAL_ENTRY_TRACE",
        )
        self.assertEqual(
            set(sets["frame_used"]["armed_empty_evidence"]),
            {"cursor", "sound", "music"},
        )

    def test_every_canonical_trace_event_has_owned_causal_edges(self) -> None:
        assets = self.document["sets"]["frame_used"]["assets"]
        self.assertEqual(
            [asset["trace_event_ordinal"] for asset in assets],
            list(range(1, 77)),
        )
        edge_pairs = {
            (item["from"], item["to"]) for item in self.document["edges"]
        }
        for asset in assets:
            self.assertEqual(asset["resolution"], "unique")
            self.assertNotIn(
                asset["trace_event"]["category"], {"cursor", "sound", "music"}
            )
            self.assertTrue(asset["owner_asset_ids"])
            self.assertIn(
                (
                    f"trace.first_render_bearing_gameplay_present.event["
                    f"{asset['trace_event_ordinal']}]",
                    asset["asset_id"],
                ),
                edge_pairs,
            )
            for owner_id in asset["owner_asset_ids"]:
                self.assertTrue(owner_id.startswith("owner_data:"))
                self.assertIn((asset["asset_id"], owner_id), edge_pairs)

    def test_trace_pair_and_full_bindings_are_recorded(self) -> None:
        source = self.document["source"]["frame_trace"]
        self.assertTrue(source["byte_identical"])
        self.assertTrue(source["independent_pass_runs"])
        self.assertEqual(source["canonical_event_count"], 76)
        self.assertEqual(
            source["trace_sha256"],
            "12260e957f3fc1d13c81f71b63c2dc23f3121f8bfae4068e126af6826f7e3e3f",
        )
        self.assertEqual(
            source["event_set_sha256"],
            "945247ea42612fd448a27c85fca8f3e28c1574cb1d919a3798cd9e119d0aebd4",
        )
        self.assertIn("trace_format_sha256", source["bindings"])

    def test_pending_audio_hook_cannot_enter_closure(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            copies = []
            for name, source in (("a", TRACE_A), ("b", TRACE_B)):
                destination = root / name
                shutil.copytree(source, destination)
                trace_path = destination / "trace.jsonl"
                rows = trace_path.read_text(encoding="utf-8").splitlines()
                header_row = json.loads(rows[0])
                proof = header_row["header"]["window"][
                    "empty_category_evidence"
                ]["sound"]["resolved_hook_proof"]["close"]
                proof["breakpoint_pending"] = True
                rows[0] = closure.canonical_json(header_row)
                trace_path.write_text("\n".join(rows) + "\n", encoding="utf-8")
                copies.append(destination)
            with self.assertRaisesRegex(
                closure.ClosureError, "pending|qualification"
            ):
                closure.build_closure(
                    EXPORT, CATALOG, OWNER, copies[0], copies[1]
                )

    def test_every_edge_has_causal_contract(self) -> None:
        required = {"from", "to", "join", "source_field", "status", "evidence"}
        for item in self.document["edges"]:
            self.assertTrue(required.issubset(item))
            self.assertIn(item["status"], {"proven", "dynamic", "unresolved"})
            for field in required:
                self.assertTrue(item[field])

    def test_expected_particle_to_base_tmap_mapping(self) -> None:
        presentation = closure.parse_presentation(EXPORT / "presentation.bin")
        expected = {
            0: 8, 7: 38, 9: 63, 39: 79, 41: 54, 44: 0, 77: 59,
            177: 96, 238: 152, 239: 155, 241: 180, 242: 183, 271: 199,
        }
        self.assertEqual(
            {
                index: presentation["particles"][index]["base_tmap"]
                for index in expected
            },
            expected,
        )

    def test_build_catalog_sprite_interpretation_is_never_reused(self) -> None:
        correction = self.document["schema_corrections"]["BUILD0-0"]
        self.assertFalse(correction["stale_catalog_entry_hashes_used"])
        build_assets = [
            asset
            for asset in self.document["sets"]["scene_reachable"]["assets"]
            if asset["asset_id"].startswith(
                "owner_data:DATA/BUILD0-0.DAT#footprint-"
            )
        ]
        self.assertEqual(len(build_assets), 12)
        self.assertTrue(all(x["kind"] == "terrain_modification_footprint" for x in build_assets))

    def test_malformed_tmap_reference_is_rejected(self) -> None:
        presentation = closure.parse_presentation(EXPORT / "presentation.bin")
        broken = copy.deepcopy(presentation)
        broken["particles"][0]["base_tmap"] = 504
        catalog = closure.Catalog(json.loads(CATALOG.read_text()))
        catalog.index_tmaps("DATA/TMAPS1-0.DAT")
        with self.assertRaises(closure.ClosureError):
            catalog.tmap_group(broken["particles"][0]["base_tmap"])

    def test_raw_schema_joins_are_catalog_hash_qualified(self) -> None:
        catalog = closure.Catalog(json.loads(CATALOG.read_text()))
        for path in (
            "DATA/BUILD0-0.DAT",
            "DATA/BUILD0-0.TAB",
            "LANGUAGE/L1.TXT",
        ):
            catalog.verify_owner_file(OWNER, path)

    def test_cursor_selector_resolves_to_reachable_sprite(self) -> None:
        reachable = {
            asset["asset_id"]
            for asset in self.document["sets"]["scene_reachable"]["assets"]
        }
        dynamic = {
            asset["asset_id"]
            for asset in self.document["sets"]["dynamic_dependencies"]["assets"]
        }
        self.assertIn("owner_data:DATA/POINTERS.DAT#9", reachable)
        self.assertNotIn("owner_data:DATA/MSPRN0-0.DAT#9", reachable)
        self.assertIn("owner_data:DATA/MSPRN0-0.DAT#9", dynamic)

    def test_global_open_and_selected_realm_roots_are_distinct(self) -> None:
        roots = self.document["sets"]["realm_load_roots"]
        self.assertIn("owner_data:DATA/TMAPS1-0.DAT", roots["selected_realm"])
        for asset_id in (
            "owner_data:DATA/TMAPS0-0.DAT",
            "owner_data:DATA/TMAPS1-0.DAT",
            "owner_data:DATA/TMAPS2-0.DAT",
            "owner_data:LEVELS/LEVELS.TAB",
            "owner_data:DATA/BLDGPRM.DAT",
            "owner_data:DATA/POINTERS.DAT",
            "owner_data:DATA/POINTERS.TAB",
        ):
            self.assertIn(asset_id, roots["global_open"])

    def test_dynamic_render_choices_include_selectable_base_rows(self) -> None:
        dynamic = {
            asset["asset_id"]
            for asset in self.document["sets"]["dynamic_dependencies"]["assets"]
        }
        for index in (0, 8, 38, 152, 155, 180, 183, 199):
            self.assertIn(f"owner_data:DATA/TMAPS1-0.DAT#{index}", dynamic)

    def test_audio_scope_and_music_divergence(self) -> None:
        audio = self.document["selectors"]["audio"]
        self.assertEqual(audio["sound_bank"], 1)
        self.assertEqual(
            audio["sound_quality_selector"],
            {
                "value": "native",
                "status": "observed_frame_setting",
                "source": "frame_trace.header.settings.hq_sound",
            },
        )
        self.assertEqual(audio["music_raw_track"], 1)
        self.assertEqual(
            audio["music_presentation_divergence"],
            {
                "native_remc2": 1,
                "preserved_dos": 0,
                "dos_frames": "0..19",
                "status": "presentation_environment_divergence",
            },
        )


class Level2ClosureContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.document = closure.build_closure(
            LEVEL2_EXPORT, CATALOG, OWNER, TRACE_A, TRACE_B, "level2"
        )

    def test_day_identity_and_partitions_are_explicit(self) -> None:
        self.assertEqual(
            self.document["profile"],
            {
                "name": "level2-day",
                "external_level": 2,
                "internal_level": 1,
                "map_type": {"id": 0, "name": "day"},
                "spec_commit": "6806c7204d17220ecc0430759f784aea8ee2ddb2",
                "owner_package": {
                    "manifest_schema": "mc2-owner-level-export/v5",
                    "players_component": {
                        "magic": "MCP1",
                        "version": 4,
                        "record_bytes": 663,
                        "count": 8,
                    },
                },
                "identity_status": "qualified",
            },
        )
        roots = self.document["sets"]["realm_load_roots"]
        self.assertIn("owner_data:DATA/TMAPS0-0.DAT", roots["selected_realm"])
        self.assertIn("owner_data:DATA/MSPRD0-0.DAT", roots["selected_realm"])
        self.assertIn("owner_data:DATA/MWEBD0-0.DAT", roots["selected_realm"])
        self.assertNotIn("owner_data:DATA/TABLESN.DAT", roots["selected_realm"])
        self.assertEqual(roots["unresolved"], [])
        self.assertEqual(self.document["sets"]["scene_reachable"]["unresolved"], [])
        frame = self.document["sets"]["frame_used"]
        self.assertEqual(frame["status"], "not_qualified_no_level2_frame_pair")
        self.assertFalse(self.document["qualification"]["zero_unresolved_frame_used_claimed"])
        self.assertTrue(frame["unresolved"])

    def test_v4_inspection_is_separate_from_v5_closure_production(self) -> None:
        manifest = json.loads((LEVEL2_V4_EXPORT / "manifest.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["schema"], "mc2-owner-level-export/v4")
        selectors = closure._level2_expected_selectors(manifest)
        self.assertEqual(selectors["map_type"], "Day")
        self.assertEqual(selectors["vga_resolution_selector"], 1)
        self.assertEqual(selectors["texture_size"], 32)
        with self.assertRaisesRegex(closure.ClosureError, "requires owner-level-export/v5"):
            closure._level2_production_owner_identity(LEVEL2_V4_EXPORT, manifest)

        v5_manifest = json.loads((LEVEL2_EXPORT / "manifest.json").read_text(encoding="utf-8"))
        self.assertEqual(
            closure._level2_production_owner_identity(LEVEL2_EXPORT, v5_manifest),
            self.document["profile"]["owner_package"],
        )

    def test_v5_component_version_pair_is_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            export = Path(tmp) / "level2"
            shutil.copytree(LEVEL2_EXPORT, export)
            players = bytearray((export / "players.bin").read_bytes())
            players[4:6] = (3).to_bytes(2, "little")
            players[6:8] = (662).to_bytes(2, "little")
            (export / "players.bin").write_bytes(players)
            manifest = json.loads((export / "manifest.json").read_text(encoding="utf-8"))
            with self.assertRaisesRegex(closure.ClosureError, "component v4/663"):
                closure._level2_production_owner_identity(export, manifest)

        with tempfile.TemporaryDirectory() as tmp:
            export = Path(tmp) / "level2"
            shutil.copytree(LEVEL2_V4_EXPORT, export)
            players = bytearray((export / "players.bin").read_bytes())
            players[4:6] = (4).to_bytes(2, "little")
            players[6:8] = (663).to_bytes(2, "little")
            (export / "players.bin").write_bytes(players)
            manifest = json.loads((export / "manifest.json").read_text(encoding="utf-8"))
            with self.assertRaisesRegex(closure.ClosureError, "requires owner-level-export/v5"):
                closure._level2_production_owner_identity(export, manifest)

    def test_day_asset_ids_and_hashes_are_catalog_qualified(self) -> None:
        identity = self.document["source"]["owner_root_identity"]
        self.assertEqual(
            identity["DATA/TMAPS0-0.DAT"],
            "84af85f568e2b909a5a881a3beb7b0d4dfdbe7d3b7a89d572a49a35ff7413074",
        )
        self.assertEqual(
            identity["DATA/MSPRD0-0.DAT"],
            "a81783458f1b81da262d60bc1f4bc28012137437f2fcedb75af441a4fc777656",
        )
        dynamic = {
            item["asset_id"]
            for item in self.document["sets"]["dynamic_dependencies"]["assets"]
        }
        self.assertIn("owner_data:DATA/MSPRD0-0.DAT", dynamic)
        self.assertNotIn("owner_data:DATA/MSPRN0-0.DAT", dynamic)

    def test_day_closure_is_repeatable(self) -> None:
        again = closure.build_closure(
            LEVEL2_EXPORT, CATALOG, OWNER, TRACE_A, TRACE_B, "level2"
        )
        self.assertEqual(
            closure.canonical_json(self.document), closure.canonical_json(again)
        )

    def test_cross_level_relabel_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            export = Path(tmp) / "level2"
            shutil.copytree(LEVEL2_EXPORT, export)
            manifest_path = export / "manifest.json"
            manifest = json.loads(manifest_path.read_text())
            manifest["level"]["external"] = 1
            manifest_path.write_text(json.dumps(manifest))
            with self.assertRaisesRegex(closure.ClosureError, "external=2"):
                closure.build_closure(export, CATALOG, OWNER, TRACE_A, TRACE_B, "level2")

    def test_stale_catalog_asset_hash_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            catalog_path = Path(tmp) / "catalog.json"
            catalog = json.loads(CATALOG.read_text())
            for item in catalog["files"]:
                if item.get("path") == "DATA/MSPRD0-0.DAT":
                    item["sha256"] = "0" * 64
                    break
            else:
                self.fail("qualified Day sprite file missing from catalog")
            catalog_path.write_text(json.dumps(catalog))
            with self.assertRaisesRegex(closure.ClosureError, "catalog hash mismatch"):
                closure.build_closure(
                    LEVEL2_EXPORT, catalog_path, OWNER, TRACE_A, TRACE_B, "level2"
                )

    def test_missing_day_asset_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            catalog_path = Path(tmp) / "catalog.json"
            catalog = json.loads(CATALOG.read_text())
            catalog["files"] = [
                item for item in catalog["files"] if item.get("path") != "DATA/SKYD0-0.DAT"
            ]
            catalog_path.write_text(json.dumps(catalog))
            with self.assertRaisesRegex(closure.ClosureError, "owner catalog has no file"):
                closure.build_closure(
                    LEVEL2_EXPORT, catalog_path, OWNER, TRACE_A, TRACE_B, "level2"
                )


if __name__ == "__main__":
    unittest.main()
