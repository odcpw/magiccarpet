from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import shutil
import tempfile
import unittest
from pathlib import Path

TOOL = Path(__file__).resolve().parents[1] / "build_cache.py"
SPEC = importlib.util.spec_from_file_location("asset_cache", TOOL)
assert SPEC and SPEC.loader
cache = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(cache)


LEVEL2_V4_PACKAGE = cache.GENERATED / "l2-debug3.fJ3PaM-v4/level2"
LEVEL2_V4_CLOSURE = cache.GENERATED / "level2-asset-closure-v4.json"
LEVEL2_V4_MATERIAL = cache.GENERATED / "level2-day-material-cache-v4"
LEVEL2_V4_GAMEPLAY = cache.GENERATED / "level2-day-gameplay-cache-v4"
LEVEL2_V4_AGGREGATE = cache.GENERATED / "level2-day-owner-cache-v4"
LEVEL2_V4_PACKAGE_SHA256 = (
    "1d73e81bc897933d62621b53a1c8c1cf890bd5dffe17f77de43f8d38f3854488"
)
LEVEL2_V4_CLOSURE_SHA256 = (
    "66505e6c3691cce6c730682047c7d8634c682172daa017d4840d1a4a5be4661c"
)


def tree_identity(root: Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(item for item in root.rglob("*") if item.is_file()):
        digest.update(path.relative_to(root).as_posix().encode("utf-8"))
        digest.update(b"\0")
        digest.update(path.read_bytes())
    return digest.hexdigest()


class InstalledLanguageTests(unittest.TestCase):
    def test_identical_installed_packs_are_one_content_not_two_languages(self):
        language = cache.language_importer()
        packs = language.installed_packs(cache.DEFAULT_OWNER)
        l1 = next(row for row in packs if "LANGUAGE/L1.TXT" in row["aliases"])
        d2 = next(row for row in packs if "LANGUAGE/D2.TXT" in row["aliases"])
        self.assertIs(l1, d2)
        self.assertEqual(len(l1["records"]), 471)
        self.assertEqual(l1["payload"], (cache.DEFAULT_OWNER / "LANGUAGE/L1.TXT").read_bytes())
        self.assertEqual(l1["records"], cache.language_records(l1["payload"]))

    def test_language_records_require_prefix_and_each_nul_terminator(self):
        language = cache.language_importer()
        payload = (cache.DEFAULT_OWNER / "LANGUAGE/L1.TXT").read_bytes()
        records = language.language_records(payload)
        truncated = payload[:4785] + b"\0".join(records[:470]) + b"\0"
        with self.assertRaisesRegex(language.LanguageCacheError, "record 470.*NUL"):
            language.language_records(truncated)
        # Tutorial 0xff sentinel streams are not language records or Unicode.
        with self.assertRaisesRegex(language.LanguageCacheError, "prefix"):
            language.language_records(b"First tutorial line\xffNext line\xff\x00")

    def test_real_source_glyph_mapping_and_missing_glyph_are_discriminated(self):
        language = cache.language_importer()
        data = (cache.DEFAULT_OWNER / "DATA/FONT1.DAT").read_bytes()
        table = cache.bitmap_records((cache.DEFAULT_OWNER / "DATA/FONT1.TAB").read_bytes())
        masks = bytearray()
        glyphs = []
        for index, (offset, width, height) in enumerate(table):
            _, mask, _ = cache.materialize_indexed_rle(data, offset, width, height)
            glyphs.append(dict(index=index, offset=len(masks), length=len(mask), width=width,
                height=height, sha256=language.digest(mask)))
            masks.extend(mask)
        records = language.language_records((cache.DEFAULT_OWNER / "LANGUAGE/L1.TXT").read_bytes())
        language.validate_glyphs(records, glyphs, bytes(masks))
        self.assertEqual([language.glyph_index(value) for value in (9, 10, 13, 32, 65, 233)],
            [33, 34, None, 33, 66, 234])
        with self.assertRaisesRegex(language.LanguageCacheError, "requires missing glyph"):
            language.validate_glyphs(records, glyphs[:64], bytes(masks))

    def test_real_complete_cache_rejects_missing_font_before_binding(self):
        language = cache.language_importer()
        with tempfile.TemporaryDirectory(prefix="mc-language-cache-") as temporary:
            output = Path(temporary) / "locale"
            manifest = cache.build_language_cache(cache.DEFAULT_OWNER, output,
                rnc_decoder=os.environ.get("MC_RNC_DECODER"))
            observed = language.validate(output)
            self.assertEqual(observed["binding_sha256"], manifest["binding_sha256"])
            self.assertEqual(len(observed["locales"]), 1)
            self.assertEqual(observed["locales"][0]["aliases"], ["LANGUAGE/D2.TXT", "LANGUAGE/L1.TXT"])
            (output / "fonts/font1.masks.u8").unlink()
            with self.assertRaises(FileNotFoundError):
                language.validate(output)


class EnvironmentMaterialTests(unittest.TestCase):
    def test_real_secret_environment_audio_binds_complete_selected_banks(self):
        environment = cache.environment_importer()
        for source, bank, expected in [(31, 1, (32, "SPIDER.WAV")),
                                       (34, 0, (49, "TORNADO.WAV"))]:
            with self.subTest(source=source):
                root = cache.GENERATED / f"mc-oi68-source{source}-fullbank-v2-20261029"
                manifest = environment.validate(root)
                descriptor = json.loads((root / "audio/descriptor.json").read_bytes())
                self.assertEqual(descriptor["sound"]["bank"], bank)
                records = descriptor["sound"]["records"]
                self.assertEqual([row["sound_id"] for row in records], list(range(1, 65)))
                self.assertEqual(records[expected[0] - 1]["filename"], expected[1])
                self.assertEqual(manifest["profile"]["internal_level"], source)

        # Rehashing a descriptor cannot conceal an omitted source slot.
        source = cache.GENERATED / "mc-oi68-source31-fullbank-v2-20261029"
        with tempfile.TemporaryDirectory(prefix="mc-audio-bank-missing-") as temporary:
            root = Path(temporary) / "cache"
            shutil.copytree(source, root)
            descriptor_path = root / "audio/descriptor.json"
            descriptor = json.loads(descriptor_path.read_bytes())
            descriptor["sound"]["records"].pop(31)
            descriptor["sound"]["record_census_sha256"] = environment.digest(
                environment.canonical(descriptor["sound"]["records"]))
            descriptor_path.write_bytes(environment.canonical(descriptor))
            manifest = json.loads((root / "manifest.json").read_bytes())
            row = next(row for row in manifest["outputs"] if row["path"] == "audio/descriptor.json")
            row["sha256"] = environment.digest(descriptor_path.read_bytes())
            row["bytes"] = descriptor_path.stat().st_size
            manifest["cache_content_sha256"] = environment.content_identity(manifest["outputs"])
            manifest.pop("manifest_payload_sha256")
            manifest["manifest_payload_sha256"] = environment.digest(environment.canonical(manifest))
            (root / "manifest.json").write_bytes(environment.canonical(manifest))
            with self.assertRaisesRegex(environment.EnvironmentMaterialError, "membership/order"):
                environment.validate(root)

    def test_real_package_profiles_validate_selected_resources(self):
        environment = cache.environment_importer()
        for level, palette, blocks, sky in [
            (1, "PALN-0", "BL32N0-0", "SKYN0-0"),
            (2, "PALD-0", "BLOCK32", "SKYD0-0"),
            (4, "PALC-0", "BL32C0-0", None),
            (25, "PALF-0", "BL32F0-0", "SKYN0-0"),
        ]:
            with self.subTest(level=level):
                root = cache.GENERATED / f"mc-oi62-environment-level{level}-v2"
                manifest = environment.validate(root)
                profile = manifest["profile"]
                self.assertEqual((profile["palette"], profile["blocks"], profile["sky"]), (palette, blocks, sky))
                self.assertEqual((root / "terrain/blocks.atlas.u8").read_bytes(), (cache.DEFAULT_OWNER / f"DATA/{blocks}.DAT").read_bytes())
                self.assertEqual((root / "palette/tables.u8").read_bytes(), (cache.DEFAULT_OWNER / f"DATA/{profile['tables']}.DAT").read_bytes())
                if level == 4:
                    self.assertFalse((root / "sky").exists())

    def test_rehashed_wrong_palette_keeps_source_bank_guard(self):
        environment = cache.environment_importer()
        source = cache.GENERATED / "mc-oi62-environment-level1-v2"
        with tempfile.TemporaryDirectory(prefix="mc-environment-wrong-bank-") as temporary:
            root = Path(temporary) / "cache"
            shutil.copytree(source, root)
            data = (cache.DEFAULT_OWNER / "DATA/PALD-0.DAT").read_bytes()
            (root / "palette/source.rgb6").write_bytes(data)
            manifest = json.loads((root / "manifest.json").read_bytes())
            row = next(row for row in manifest["outputs"] if row["path"] == "palette/source.rgb6")
            row["sha256"] = environment.digest(data)
            manifest["cache_content_sha256"] = environment.content_identity(manifest["outputs"])
            manifest.pop("manifest_payload_sha256")
            manifest["manifest_payload_sha256"] = environment.digest(environment.canonical(manifest))
            (root / "manifest.json").write_bytes(environment.canonical(manifest))
            with self.assertRaisesRegex(environment.EnvironmentMaterialError, "selected owner bank identity differs: PALN-0"):
                environment.validate(root)


class SemanticIdentityTests(unittest.TestCase):
    """Discriminate semantic controls from incidental source/receipt pins."""

    def test_contract_metadata_does_not_change_byte_contract_identity(self) -> None:
        with tempfile.TemporaryDirectory(prefix="contract-metadata-test.") as root:
            path = Path(root) / "contract.json"
            contract = json.loads(cache.DEFAULT_CONFORMANCE_CONTRACT.read_text())
            contract["metadata"] = {"review_note": "equivalent formatting metadata"}
            contract["specification"]["metadata"] = {"generated_by": "test"}
            contract["trusted_identity"]["metadata_note"] = "not an identity field"
            contract["reachable"]["metadata"] = {"display": "stable"}
            contract["receipt_requirements"]["metadata"] = {"owner": "private"}
            path.write_bytes(cache.canonical_json(contract))

            loaded, identity = cache.load_conformance_contract(path, "0" * 64)

        self.assertEqual(loaded["schema"], cache.CONFORMANCE_CONTRACT_SCHEMA)
        self.assertEqual(identity, contract["contract_sha256"])

    def test_wrong_contract_schema_is_rejected_before_identity_check(self) -> None:
        with tempfile.TemporaryDirectory(prefix="contract-schema-test.") as root:
            path = Path(root) / "contract.json"
            contract = json.loads(cache.DEFAULT_CONFORMANCE_CONTRACT.read_text())
            contract["schema"] = "mc2-plan29-closure-contract/v999"
            path.write_bytes(cache.canonical_json(contract))

            with self.assertRaisesRegex(cache.CacheError, "unsupported closure contract schema"):
                cache.load_conformance_contract(path)

    def test_exporter_comment_change_preserves_runtime_schema(self) -> None:
        with tempfile.TemporaryDirectory(prefix="exporter-metadata-test.") as root:
            path = Path(root) / "exporter_main.cpp"
            path.write_bytes(
                b"// harmless source metadata\n"
                + cache.EXPORTER_SOURCE.read_bytes()
            )

            evidence = cache.exporter_layout_evidence(path)

        self.assertEqual(evidence["source"], str(path))
        self.assertGreater(evidence["capture_runtime_last_line"], evidence["capture_runtime_first_line"])
        self.assertEqual(evidence["runtime_bytes"], 2_754)

    def test_exporter_schema_marker_change_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory(prefix="exporter-schema-test.") as root:
            path = Path(root) / "exporter_main.cpp"
            source = cache.EXPORTER_SOURCE.read_bytes()
            original = b'result.magic("MCR1");\n    result.u16(include_constructor_counters ? 2 : 1);'
            replacement = b'result.magic("MCR1");\n    result.u16(include_constructor_counters ? 2 : 2);'
            self.assertEqual(source.count(original), 1)
            path.write_bytes(source.replace(original, replacement))

            with self.assertRaisesRegex(cache.CacheError, "runtime schema changed"):
                cache.exporter_layout_evidence(path)

    def test_matrix_metadata_change_preserves_schema_contract(self) -> None:
        with tempfile.TemporaryDirectory(prefix="matrix-metadata-test.") as root:
            path = Path(root) / "matrix.json"
            matrix = json.loads(cache.CANONICAL_LEVEL1_MATRIX.read_text())
            matrix["meta"]["generated"] = "equivalent metadata"
            matrix["metadata"] = {"review_note": "not a semantic row change"}
            path.write_bytes(cache.canonical_json(matrix))

            loaded, file_identity = cache.load_level1_conformance_matrix(path)
            expected_file_identity = cache.sha256_file(path)

        self.assertEqual(
            loaded["schema"]["id"], cache.LEVEL1_MATRIX_SCHEMA_ID
        )
        self.assertEqual(loaded["schema"]["version"], cache.LEVEL1_MATRIX_SCHEMA_VERSION)
        self.assertEqual(file_identity, expected_file_identity)

    def test_wrong_matrix_schema_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory(prefix="matrix-schema-test.") as root:
            path = Path(root) / "matrix.json"
            matrix = json.loads(cache.CANONICAL_LEVEL1_MATRIX.read_text())
            matrix["schema"]["version"] += 1
            path.write_bytes(cache.canonical_json(matrix))

            with self.assertRaisesRegex(cache.CacheError, "unsupported Level 1 matrix schema"):
                cache.load_level1_conformance_matrix(path)


class NightProductionCacheTests(unittest.TestCase):
    """Exercise the actual bank-bearing owner contract without rebuilding caches."""

    @classmethod
    def setUpClass(cls) -> None:
        spec = importlib.util.spec_from_file_location("night_profile_tests", TOOL.with_name("night_profile.py"))
        assert spec and spec.loader
        cls.night = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.night)
        cls.package = cache.GENERATED / "mc-bhq8-level1-collision-v5-night"
        cls.output = cache.GENERATED / "mc2-level1-night-cache-v2"
        if not cls.package.is_dir() or not cls.output.is_dir():
            raise unittest.SkipTest("private Night package/cache unavailable")

    def test_canonical_import_binds_actual_runtime_and_retained_visual_view(self) -> None:
        runtime = (self.package / "runtime.bin").read_bytes()
        projection = self.night.visual_runtime(runtime)
        manifest = cache.validate_cache(self.output,
            "61d2d4962aeafa6325d2d0da3de8d0bcfe5f5d2e6859bdaa5b849cea596eac42",
            cache.EXPECTED_CLOSURE)
        bindings = manifest["bindings"]
        self.assertEqual(bindings["qualified_runtime_capture_sha256"], cache.sha256_bytes(runtime))
        self.assertNotEqual(cache.sha256_bytes(runtime), cache.EXPECTED_RUNTIME_SHA256)
        self.assertEqual(projection, (cache.DEFAULT_PACKAGE / "runtime.bin").read_bytes())
        self.assertEqual(bindings["runtime_visual_projection_sha256"], cache.sha256_bytes(projection))
        self.assertEqual(bindings["qualification_runtime_capture_sha256"], cache.EXPECTED_RUNTIME_SHA256)
        self.assertEqual(manifest["night_metadata_sha256"], self.night.metadata_sha256(manifest))

    def test_runtime_projection_rejects_wrong_version_or_truncated_counters(self) -> None:
        runtime = (self.package / "runtime.bin").read_bytes()
        for payload in (runtime[:-1], runtime[:4] + b"\x01\x00" + runtime[6:]):
            with self.assertRaisesRegex(ValueError, "MCR1/v2"):
                self.night.visual_runtime(payload)

    def test_scalar_digest_binds_raw_runtime_separately_from_qualification(self) -> None:
        manifest = json.loads((self.output / "manifest.json").read_bytes())
        original = self.night.metadata_sha256(manifest)
        manifest["bindings"]["qualified_runtime_capture_sha256"] = cache.EXPECTED_RUNTIME_SHA256
        self.assertNotEqual(self.night.metadata_sha256(manifest), original)
        manifest["bindings"]["undeclared_alias"] = cache.EXPECTED_RUNTIME_SHA256
        with self.assertRaisesRegex(ValueError, "field set"):
            self.night.metadata_sha256(manifest)

    def test_collision_bank_matches_owner_pixels_and_rejects_changed_pixel(self) -> None:
        owner = json.loads((self.package / "owner-manifest.json").read_bytes())
        definition = json.loads((self.package / "campaign-definition.json").read_bytes())
        self.night._bank(self.package, owner, definition, cache.DEFAULT_OWNER)
        bank = bytearray((self.package / "collision-masks.bin").read_bytes())
        bank[-1] ^= 1
        with tempfile.TemporaryDirectory(prefix="night-collision-pixel.") as directory:
            root = Path(directory)
            (root / "collision-masks.bin").write_bytes(bank)
            with self.assertRaisesRegex(ValueError, "owner pixels"):
                self.night._bank(root, owner, definition, cache.DEFAULT_OWNER)


class OwnerCacheTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.temp_root = Path(
            tempfile.mkdtemp(prefix="asset-cache-test.", dir=cache.GENERATED)
        )
        cls.first = cls.temp_root / "first"
        cls.second = cls.temp_root / "second"
        cache.build_cache(
            cache.DEFAULT_OWNER,
            cache.DEFAULT_PACKAGE,
            cache.DEFAULT_CLOSURE,
            cache.DEFAULT_CATALOG,
            cls.first,
        )
        cache.build_cache(
            cache.DEFAULT_OWNER,
            cache.DEFAULT_PACKAGE,
            cache.DEFAULT_CLOSURE,
            cache.DEFAULT_CATALOG,
            cls.second,
        )

    @classmethod
    def tearDownClass(cls) -> None:
        shutil.rmtree(cls.temp_root)

    def test_byte_identical_rebuild_and_exact_scope(self) -> None:
        self.assertEqual(tree_identity(self.first), tree_identity(self.second))
        manifest = cache.validate_cache(self.first)
        outputs = {row["path"]: row for row in manifest["outputs"]}
        self.assertEqual(
            {
                path: (outputs[path]["bytes"], outputs[path]["sha256"])
                for path in (
                    "terrain/tmaps1/152.decoded.bin",
                    "terrain/tmaps1/152.indices.u8",
                    "terrain/tmaps1/153.decoded.bin",
                    "terrain/tmaps1/153.indices.u8",
                    "terrain/tmaps1/154.decoded.bin",
                    "terrain/tmaps1/154.indices.u8",
                )
            },
            {
                "terrain/tmaps1/152.decoded.bin": (
                    1818, "a5660e5a142d4179c1b3bb4328b4875c6621daf45c6c8dfcaa9a7a2fd3cc74db"),
                "terrain/tmaps1/152.indices.u8": (
                    484, "cd5e773ab7c4e4e88536c5a690a8a1794a1b67361b402ab8365e9df3e59a1e9d"),
                "terrain/tmaps1/153.decoded.bin": (
                    2730, "13c357f4d824561f23c134cdd5b40d44d1eeac3a374f3b244c890a097b6cb122"),
                "terrain/tmaps1/153.indices.u8": (
                    704, "acea2351981a1fd9aefaf7a2713bc00c4c2b2461dd087b9a2621e7fa3fb819a3"),
                "terrain/tmaps1/154.decoded.bin": (
                    1822, "64bcc3b7a53355174d7833f3d16ef2384fa03fc9fc4c561c3aea0651ce6fe053"),
                "terrain/tmaps1/154.indices.u8": (
                    484, "7d081998c98ba0aa2bb57d4956926f3b8c80b31ae6ee481f8c9174551d6db722"),
            },
        )
        sprites = manifest["assets"]["hsprn"]
        self.assertEqual(
            [row["index"] for row in sprites],
            sorted(set(cache.FRAME_HSPRN + cache.DYNAMIC_HSPRN)),
        )
        self.assertEqual(
            sum(row["inclusion"] == "qualified_frame_used" for row in sprites), 11
        )
        self.assertEqual(manifest["assets"]["hfont3"]["glyph_count"], 273)
        castle = manifest["assets"]["level1_castle_stage"]
        self.assertEqual(castle["build_index"], 1)
        self.assertEqual(
            (castle["effective_width"], castle["effective_height"]),
            (8, 8),
        )
        castle_descriptor = json.loads(
            (self.first / castle["path"]).read_text(encoding="utf-8")
        )
        self.assertEqual(
            castle_descriptor["dimensions"],
            {
                "raw_tab": {
                    "height": 8,
                    "tab_record_sha256":
                        cache.CASTLE_STAGE_ONE_BUILD_TAB_RECORD_SHA256,
                    "width": 8,
                },
                "loader_runtime": {
                    "height": 16,
                    "rule":
                        "CreateIndexes doubles when resolution bit 0 is set",
                    "width": 16,
                },
                "castle_consumer_effective": {
                    "height": 8,
                    "rule":
                        "SetShiftByCastle and terrain consumers halve when selector is 1",
                    "width": 8,
                },
            },
        )
        castle_cells = (
            self.first / castle_descriptor["cells"]["path"]
        ).read_bytes()
        self.assertEqual(len(castle_cells), 128)
        self.assertEqual(
            hashlib.sha256(castle_cells).hexdigest(),
            cache.CASTLE_STAGE_ONE_BUILD_PAYLOAD_SHA256,
        )
        self.assertEqual(
            castle_descriptor["building_parameters"],
            {
                "byte_3": 0,
                "flags": 3,
                "record_index": 1,
                "record_sha256":
                    cache.CASTLE_STAGE_ONE_BUILD_PARAMETER_SHA256,
                "source_asset_id": "owner_data:DATA/BLDGPRM.DAT",
                "word_0": 2,
            },
        )
        houses = manifest["assets"]["level1_house_footprints"]
        self.assertEqual(
            houses["build_indices"],
            list(cache.LEVEL1_HOUSE_BUILD_INDICES),
        )
        house_descriptor = json.loads(
            (self.first / houses["path"]).read_text(encoding="utf-8")
        )
        self.assertEqual(
            house_descriptor["schema"],
            "mc2-level1-house-build-footprints/v1",
        )
        house_cells = (
            self.first / house_descriptor["cells"]["path"]
        ).read_bytes()
        self.assertEqual(len(house_cells), 7858)
        self.assertEqual(
            hashlib.sha256(house_cells).hexdigest(),
            "902f81508af1d7b8c8a65658a114aefeec79c94c5cb350fc5b5544ac42c06784",
        )
        self.assertEqual(
            [
                (row["build_index"], row["width"], row["height"])
                for row in house_descriptor["footprints"]
            ],
            [
                (17, 48, 48),
                (26, 9, 11),
                (29, 13, 11),
                (30, 13, 15),
                (31, 23, 11),
                (34, 22, 22),
                (42, 11, 11),
                (47, 11, 11),
                (50, 7, 7),
                (51, 4, 12),
                (52, 12, 4),
                (53, 8, 8),
            ],
        )
        entity_asset = manifest["assets"]["entity_presentations"]
        self.assertEqual(entity_asset["qualified_actor_count"], 64)
        self.assertEqual(entity_asset["mapped_reset_effect_count"], 19)
        self.assertEqual(entity_asset["reset_chain_drawable_count"], 83)
        self.assertEqual(entity_asset["qualified_view_count"], 64)
        self.assertEqual(entity_asset["qualified_group_count"], 6)
        self.assertEqual(entity_asset["active_reset_count"], 177)
        self.assertEqual(entity_asset["hidden_reset_count"], 113)
        self.assertEqual(entity_asset["suppressed_terrain_head_count"], 47)
        self.assertEqual(entity_asset["off_chain_active_count"], 47)
        entity_descriptor = json.loads(
            (self.first / entity_asset["path"]).read_text()
        )
        self.assertEqual(
            entity_descriptor["source_contract"]["msprn_status"],
            "not_used_for_world_entities; closure MSPRN rows are HUD/bitmap path",
        )
        actor_groups = {
            row["semantic_family"]: row
            for row in entity_descriptor["groups"]
            if row["qualified_status"] == "qualified_owner_actor_billboard"
        }
        self.assertEqual(
            {family: row["entity_count"] for family, row in actor_groups.items()},
            cache.EXPECTED_ACTOR_COUNTS,
        )
        self.assertEqual(
            actor_groups["dolmen"]["selector"]["qualified_selectors"], [79]
        )
        self.assertEqual(
            actor_groups["goat"]["selector"]["qualified_selectors"], [152, 153, 154]
        )
        self.assertEqual(
            actor_groups["villager-a"]["selector"]["qualified_selectors"],
            [155, 156, 157],
        )
        self.assertEqual(
            actor_groups["villager-b"]["selector"]["qualified_selectors"],
            [180, 181, 182],
        )
        self.assertEqual(
            actor_groups["villager-c"]["selector"]["qualified_selectors"],
            [183, 184, 185],
        )
        self.assertEqual(
            actor_groups["villager-d"]["selector"]["qualified_selectors"],
            [199, 200, 201],
        )
        self.assertEqual(
            actor_groups["goat"]["selector"]["yaw_lookup_0_15"],
            list(cache.YAW_LOOKUP_D4750_28),
        )
        self.assertTrue(
            actor_groups["goat"]["selector"]["mirror_when_orientation_gte_8"]
        )
        self.assertEqual(
            actor_groups["dolmen"]["billboard"]["texture_animation"]["qualified_view"],
            "static_base_frame",
        )
        self.assertTrue(
            all(
                row["billboard"]["anchor"] ==
                    "viewer_bottom_center_at_entity_xyz_approximation"
                and row["billboard"]["source_anchor_status"].startswith(
                    "deferred: original DrawSprite_41BD3"
                )
                for row in actor_groups.values()
            )
        )
        for family in (
            "goat", "villager-a", "villager-b", "villager-c", "villager-d"
        ):
            animation = actor_groups[family]["billboard"]["texture_animation"]
            self.assertTrue(animation["animated"])
            self.assertEqual(animation["frame_count"], 4)
            self.assertEqual(
                animation["scope"],
                "selector_global_shared_TMAP_state_not_per_entity",
            )
            self.assertEqual(
                animation["qualified_view"],
                "base_frame_only; global used-TMAP update timing deferred",
            )
        self.assertEqual(
            sum(
                row["reset_chain_drawable_count"]
                for row in entity_descriptor["groups"]
            ),
            83,
        )
        self.assertEqual(
            sum(row["qualified_view_count"] for row in entity_descriptor["groups"]),
            64,
        )
        self.assertEqual(
            {
                authority: sum(
                    row["entity_count"]
                    for row in entity_descriptor["groups"]
                    if row["authority"] == authority
                )
                for authority in (
                    "FrozenObserved", "PlayerFlightLive", "SpellActivationLive"
                )
            },
            {
                "FrozenObserved": 175,
                "PlayerFlightLive": 1,
                "SpellActivationLive": 1,
            },
        )
        self.assertTrue(
            all(
                row["authority"] == "FrozenObserved"
                for row in entity_descriptor["groups"]
                if row["reset_chain_drawable_count"]
            )
        )
        effects = {
            row["semantic_family"]: row
            for row in entity_descriptor["groups"]
            if row["qualified_status"] ==
                "mapped_reset_capture_site_effect_hidden_qualified_first_present_diverged"
        }
        self.assertEqual(
            {
                family: row["reset_chain_drawable_count"]
                for family, row in effects.items()
            },
            {"fireball-impact": 12, "explosion-death": 1, "smoke-particle": 6},
        )
        self.assertTrue(
            all(row["qualified_view_count"] == 0 for row in effects.values())
        )
        self.assertEqual(effects["fireball-impact"]["selector"]["qualified_selectors"], [39])
        self.assertEqual(effects["explosion-death"]["selector"]["qualified_selectors"], [54])
        self.assertEqual(effects["smoke-particle"]["selector"]["qualified_selectors"], [63])
        self.assertEqual(
            effects["fireball-impact"]["billboard"]["compositing"],
            "x_BYTE_F2CC6_alternate_transparency_path_approximated_by_index_zero_cutout",
        )
        self.assertTrue(
            all(
                row["qualified_hidden_reason"]
                for row in entity_descriptor["groups"]
                if row["qualified_view_count"] != row["entity_count"]
            )
        )
        self.assertEqual(
            manifest["assets"]["hfont3"]["palette_status"],
            "mask_exact_runtime_tint_callsite_dynamic; Level1 objective CLRN3840 qualified",
        )
        self.assertEqual(
            manifest["assets"]["spell_icon_indices"]["status"],
            "family_mapping_with_Level1_high_resolution_big_icons_materialized",
        )
        hud_asset = manifest["assets"]["level1_hud"]
        self.assertEqual(
            hud_asset["status"],
            "qualified_owner_assets_mixed_Live_FrozenObserved_Deferred",
        )
        hud = json.loads((self.first / hud_asset["path"]).read_text())
        self.assertEqual(hud["schema"], "mc2-level1-owner-hud/v1")
        self.assertEqual(
            hud["reference_layout"],
            {
                "width": 640,
                "height": 480,
                "uniform_scale_expression":
                    "min(Screen.width / 640, Screen.height / 480)",
                "filter": "nearest",
                "resolution_bank": "HSPRN0-0",
                "source_scale": 1,
                "bank_status":
                    "source_mapped_Night_high_resolution_bank_native",
                "high_resolution_bank_status":
                    "Qualified through remc2 LoadSpr_47160 high-resolution branch",
            },
        )
        bitmap_assets = hud["bitmap_assets"]
        self.assertEqual(
            [row["selector"] for row in bitmap_assets],
            [1, 2, 9]
            + list(range(40, 56))
            + [58, 66]
            + list(range(74, 92))
            + list(range(97, 262)),
        )
        self.assertEqual(len({row["selector"] for row in bitmap_assets}), 204)
        self.assertTrue(
            all(
                row["bank"] == "HSPRN0-0"
                and row["source_scale"] == 1
                and row["runtime_width"] == row["stored_width"]
                and row["runtime_height"] == row["stored_height"]
                and row["authority"] == "FrozenObserved"
                and len(row["indices_sha256"]) == 64
                and len(row["mask_sha256"]) == 64
                for row in bitmap_assets
            )
        )
        legacy_hud_selectors = {
            1, 2, *range(40, 56), 58, 66, 83, 84, *range(88, 92),
            *range(97, 127), 149, 150, *range(161, 168),
            *range(171, 174), *range(179, 191),
        }
        self.assertEqual(
            {
                row["selector"]: (row["runtime_width"], row["runtime_height"])
                for row in bitmap_assets
                if row["selector"] in legacy_hud_selectors
            },
            {
                1: (64, 44),
                2: (64, 44),
                40: (124, 44),
                41: (128, 44),
                42: (16, 44),
                43: (34, 44),
                44: (34, 44),
                45: (34, 44),
                46: (34, 44),
                47: (34, 44),
                48: (34, 44),
                49: (34, 44),
                50: (34, 44),
                51: (34, 44),
                52: (34, 44),
                53: (34, 44),
                54: (128, 44),
                55: (128, 44),
                58: (8, 10),
                66: (18, 23),
                83: (5, 5),
                84: (9, 9),
                88: (8, 72),
                89: (48, 36),
                90: (48, 36),
                91: (48, 36),
                97: (41, 23),
                98: (41, 23),
                99: (41, 23),
                100: (41, 23),
                101: (41, 23),
                102: (41, 23),
                103: (41, 23),
                104: (41, 23),
                105: (41, 23),
                106: (41, 23),
                107: (41, 23),
                108: (41, 23),
                109: (41, 23),
                110: (41, 23),
                111: (41, 23),
                112: (41, 23),
                113: (41, 23),
                114: (41, 23),
                115: (41, 23),
                116: (41, 23),
                117: (41, 23),
                118: (41, 23),
                119: (41, 23),
                120: (41, 23),
                121: (41, 23),
                122: (41, 23),
                123: (62, 34),
                124: (62, 34),
                125: (62, 34),
                126: (62, 34),
                149: (12, 12),
                150: (12, 12),
                161: (66, 34),
                162: (66, 34),
                163: (66, 34),
                164: (66, 34),
                165: (6, 12),
                166: (10, 12),
                167: (14, 12),
                171: (16, 12),
                172: (16, 14),
                173: (32, 10),
                179: (40, 22),
                180: (40, 22),
                181: (40, 22),
                182: (40, 22),
                183: (40, 22),
                184: (40, 22),
                185: (40, 22),
                186: (40, 22),
                187: (40, 22),
                188: (40, 22),
                189: (40, 22),
                190: (40, 22),
            },
        )
        self.assertEqual(
            [row["selector"] for row in hud["top_bar"]["panels"]],
            [40, 54, 54, 41, 43, 42],
        )
        self.assertEqual(
            [row["authority"] for row in hud["top_bar"]["panels"]],
            ["SourceMappedLive"] * 6,
        )
        self.assertEqual(
            [
                (
                    row["holder_entity"],
                    row["model"],
                    row["icon_selector"],
                    row["authority"],
                    row["activation_authority"],
                )
                for row in hud["spells"]["slots"]
            ],
            [
                (153, 0, 123, "MissionStateLive", "SpellActivationLive"),
                (154, 1, 124, "MissionStateLive", "SpellActivationLive"),
            ],
        )
        objective = hud["objective"]
        self.assertEqual(objective["selector"], 48)
        self.assertEqual(
            objective["authority"],
            "SourceMappedLiveIndexedFramebuffer",
        )
        self.assertEqual(objective["objective_textbox_ticks"], 33)
        self.assertEqual(objective["frame_selectors"], [171, 172, 173])
        self.assertEqual(
            objective["draw_order"],
            [
                "TABLESN_background_blend",
                "HFONT3_text_palette_119",
                "HSPRN_frame_171_172_173",
            ],
        )
        self.assertEqual(
            objective["background_blend"],
            {
                "source_table_path": "palette/tablesn.u8",
                "table_offset": 0x4000,
                "clrn_selector": 0,
                "blend_selector": 61,
                "expression":
                    "TABLESN[0x4000 + (CLRN[0] << 8) + live_destination_index]",
                "underlay_scope":
                    "world_only_for_Level1_rows; all boxes begin below y=128 prior HUD extent",
                "display_adaptation":
                    "pixel-centre nearest sample from qualified 320x200 indexed world "
                    "into 640x480 corrected 4:3 reference layout",
            },
        )
        self.assertEqual(
            objective["glyph_indices"],
            [35, 71, 109, 122, 33, 117, 112, 120, 98, 115, 101, 116,
             33, 110, 122, 33, 99, 102, 98, 100, 112, 111, 47, 35],
        )
        self.assertEqual(
            objective["tint"],
            {
                "active_map_palette_lut": "CLRN-0",
                "lut_selector": 3840,
                "palette_index": 119,
                "paln_rgb8": [252, 36, 36],
                "historical_pointer_name":
                    "xadataclrd0dat; PaletteChanges_47760 installs CLRN for Night",
            },
        )
        self.assertEqual(
            (self.first / objective["text_bytes_path"]).read_bytes(),
            b'"Fly towards my beacon."',
        )
        expected_geometry = {
            48: (192, 234, 256, 38, 224, 246, 208, 244, 224, 18),
            49: (128, 220, 384, 52, 160, 232, 144, 230, 352, 32),
            50: (144, 220, 352, 52, 164, 232, 160, 230, 320, 32),
            51: (144, 220, 352, 52, 164, 232, 160, 230, 320, 32),
            52: (176, 220, 288, 52, 208, 232, 192, 230, 256, 32),
            53: (128, 220, 384, 52, 160, 232, 144, 230, 352, 32),
        }
        for row in objective["rows"]:
            self.assertEqual(row["authority"], "SourceMappedLiveIndexedFramebuffer")
            self.assertEqual(
                (
                    row["reference_x"],
                    row["reference_y"],
                    row["render_width"],
                    row["render_height"],
                    row["text_x"],
                    row["text_y"],
                    row["blend_x"],
                    row["blend_y"],
                    row["blend_width"],
                    row["blend_height"],
                ),
                expected_geometry[row["selector"]],
            )
            plane_bytes = row["render_width"] * row["render_height"]
            self.assertEqual(row["raster_plane_bytes"], plane_bytes)
            raster = (self.first / row["raster_path"]).read_bytes()
            self.assertEqual(len(raster), plane_bytes * 2)
            indices = raster[:plane_bytes]
            mask = raster[plane_bytes:]
            self.assertEqual(
                hashlib.sha256(indices).hexdigest(),
                row["foreground_indices_sha256"],
            )
            self.assertEqual(
                hashlib.sha256(mask).hexdigest(),
                row["foreground_mask_sha256"],
            )
            self.assertEqual(sum(mask), row["foreground_opaque_pixels"])
            self.assertEqual(set(mask), {0, 1})
            self.assertIn(119, indices)
        self.assertEqual(
            hashlib.sha256(
                (self.first / objective["raster_path"]).read_bytes()
            ).hexdigest(),
            next(
                row["sha256"]
                for row in json.loads(
                    (self.first / "manifest.json").read_text(encoding="utf-8")
                )["outputs"]
                if row["path"] == objective["raster_path"]
            ),
        )
        self.assertEqual(
            hud["authority_badge"]["text"], "FROZEN OBSERVED"
        )
        self.assertEqual(
            hud["minimap"]["map_mode"],
            "player_centered_yaw_rotated_TABLESN_scanline_aperture",
        )
        minimap = hud["minimap"]
        self.assertEqual(
            (
                minimap["width"],
                minimap["height"],
                minimap["logical_width"],
                minimap["logical_height"],
                minimap["presentation_scale"],
                minimap["sampling"]["source_units_per_pixel"],
            ),
            (128, 128, 128, 128, 1, 256),
        )
        self.assertEqual(
            minimap["resolution_branch"],
            "HSPRN_native_128x128_scaling256",
        )
        aperture = minimap["aperture"]
        aperture_mask = (self.first / aperture["path"]).read_bytes()
        self.assertEqual(len(aperture_mask), 128 * 128)
        self.assertEqual(sum(aperture_mask), 12848)
        self.assertEqual(
            hashlib.sha256(aperture_mask).hexdigest(),
            "1d4ce7e417ace6af7eb78a29b83a6f8a0b2558002d36079ea2fa291f7a6df98b",
        )
        self.assertEqual(aperture["opaque_pixels"], 12848)
        self.assertEqual(aperture["table_offset"], 0x14300)
        self.assertEqual(aperture["phase_step"], 0x40000)
        self.assertEqual(
            aperture["table_curve_sha256"],
            "e155e5eaa2019054c52878dad895d0ad70c6b92698fab72ae518eeccea4eafc7",
        )
        scanlines = aperture["scanlines"]
        self.assertEqual(len(scanlines), 128)
        self.assertEqual(
            [
                (
                    scanlines[index]["start_x_inclusive"],
                    scanlines[index]["end_x_exclusive"],
                )
                for index in (0, 1, 31, 63, 64, 96, 126, 127)
            ],
            [
                (53, 75), (49, 79), (9, 119), (1, 127),
                (1, 127), (9, 119), (49, 79), (53, 75),
            ],
        )
        sine_payload = (
            self.first / minimap["rotation"]["sine_table_path"]
        ).read_bytes()
        self.assertEqual(len(sine_payload), 2560 * 4)
        self.assertEqual(
            hashlib.sha256(sine_payload).hexdigest(),
            "9df35597f7825cd0143fcd66827a399c4a73822fc5a769c5accd9323f3210ce1",
        )
        direction_rows = minimap["rotation"]["cardinal_direction_samples"]
        self.assertEqual(len(direction_rows), 16)
        self.assertEqual(
            [
                (
                    row["yaw"],
                    row["screen_label"],
                    row["source_tile_x"],
                    row["source_tile_y"],
                    row["palette_index"],
                )
                for row in direction_rows
            ],
            [
                (0, "top", 77, 190, 99),
                (0, "right", 109, 222, 52),
                (0, "bottom", 77, 254, 56),
                (0, "left", 45, 222, 209),
                (512, "top", 109, 222, 52),
                (512, "right", 77, 254, 56),
                (512, "bottom", 45, 222, 209),
                (512, "left", 77, 190, 99),
                (1024, "top", 77, 254, 56),
                (1024, "right", 45, 222, 209),
                (1024, "bottom", 77, 190, 99),
                (1024, "left", 109, 222, 52),
                (1536, "top", 45, 222, 209),
                (1536, "right", 77, 190, 99),
                (1536, "bottom", 109, 222, 52),
                (1536, "left", 77, 254, 56),
            ],
        )
        self.assertEqual(
            [
                (row["yaw"], row["source_tile_x"], row["source_tile_y"])
                for row in minimap["rotation"]["octant_probe_samples"]
            ],
            [
                (0, 97, 202), (256, 105, 222), (512, 97, 242),
                (768, 77, 250), (1024, 57, 242), (1280, 49, 222),
                (1536, 57, 202), (1792, 77, 194),
            ],
        )
        self.assertEqual(
            (
                minimap["terrain_authority"],
                minimap["entity_positions_authority"],
                minimap["player_pose_authority"],
            ),
            (
                "OwnerLevelStaticLiveSampling",
                "SourceArenaLive",
                "PlayerFlightLive",
            ),
        )
        self.assertEqual(
            minimap["entity_overlay_mode"],
            "Rust_source_slot_ordered_DrawMinimapEntities_commands",
        )
        self.assertTrue(
            minimap["entity_colour_filter_status"].startswith("SourceMappedLive")
        )
        audio_asset = manifest["assets"]["level1_audio"]
        self.assertEqual(
            (
                audio_asset["sound_bank"],
                audio_asset["sound_quality"],
                audio_asset["sound_quality_index"],
                audio_asset["music_channel"],
                audio_asset["music_driver_type"],
                audio_asset["music_driver_index"],
                audio_asset["logical_track"],
                audio_asset["playback_slot"],
                audio_asset["loop_scan_pointer_slot"],
            ),
            (1, 822, 3, 0, "f", 2, 1, 0, 1),
        )
        sound = json.loads(
            (self.first / audio_asset["sound_descriptor_path"]).read_text()
        )
        self.assertEqual(sound["schema"], "mc2-level1-sound-bank/v1")
        self.assertEqual(sound["bank_counts_by_quality_index"], [0, 0, 0, 10, 10, 0])
        self.assertEqual(
            sound["container"],
            {
                "container_trailer_offset": 14864080,
                "descriptor_offset": 14864236,
                "metadata_offset": 1988320,
                "data_offset": 0,
                "metadata_size": 2080,
                "data_size": 1988320,
            },
        )
        self.assertEqual(len(sound["metadata_records"]), 64)
        self.assertEqual(
            hashlib.sha256(
                "\n".join(
                    row["filename"] for row in sound["metadata_records"]
                ).encode()
            ).hexdigest(),
            "b59b907e19cefde10c544fa64c73456fcea591dbb9491bdb7afe2c42890fb06c",
        )
        self.assertEqual(
            sound["materialized_sound_ids"],
            [
                1, 2, 3, 4, 5, 8, 9, 10, 14, 15, 16, 18, 19, 27, 29, 30,
                31, 33, 34, 37, 39, 40, 41, 43, 44, 46, 54, 55, 56, 57, 63,
            ],
        )
        self.assertEqual(sound["explicit_null_sound_ids"], [12])
        self.assertEqual(
            {
                row["sound_id"]: row["playback_scope"]
                for row in sound["level1_assets"]
            },
            {
                1: "local_ambient",
                2: "local_ambient",
                3: "original_stereo_positional",
                4: "original_stereo_positional",
                5: "local_ambient",
                8: "original_stereo_positional",
                9: "original_stereo_positional",
                10: "original_stereo_positional",
                12: "explicit_null",
                14: "local_feedback",
                15: "original_stereo_positional",
                16: "original_stereo_positional",
                18: "original_stereo_positional",
                19: "original_stereo_positional",
                27: "original_stereo_positional",
                29: "local_feedback",
                30: "original_stereo_positional",
                31: "local_ambient",
                33: "original_stereo_positional",
                34: "original_stereo_positional",
                37: "original_stereo_positional",
                39: "original_stereo_positional",
                40: "original_stereo_positional",
                41: "original_stereo_positional",
                43: "original_stereo_positional",
                44: "original_stereo_positional",
                46: "original_stereo_positional",
                54: "original_stereo_positional",
                55: "original_stereo_positional",
                56: "original_stereo_positional",
                57: "original_stereo_positional",
                63: "original_stereo_positional",
            },
        )
        expected_audio = {
            1: ("OCEAN.WAV", 151344, 151296,
                "d41a44e541f882e620a57316e020d40369372166f84ccbc07525848a395ec8dc"),
            2: ("CRICKET2.WAV", 135984, 135937,
                "f4636891ecc2736deba4aab46e7e5117d1b89022ea20fb93315ee0ab46de8d34"),
            3: ("EXPLOD3.WAV", 30192, 30144,
                "18f40a58188d26caf20c4dd2f01cf6e383c6867fb58a2c2d9aeb5c4ab480a2b0"),
            4: ("GAINMANA.WAV", 22688, 22640,
                "ec5b853e1834f05e2d47a58e5dcf4db32cd8f3253a69ce75205531d1c2d785e2"),
            5: ("FIRE.WAV", 44080, 44032,
                "008f25258a035546949ccca475ca954feb5f74cdfe32c76e6f612bf18575bf3a"),
            8: ("DRAGROAR.WAV", 62384, 62336,
                "121e3c3bca9874214f334ad48151bd79b90a64f76b809c01e66398507c57e1d1"),
            9: ("FIREBAL1.WAV", 26992, 26944,
                "8bacc231f15be4c688d7abada4bb2531ae4b8a87160633fdb1bbba4c6bad9153"),
            10: ("QUAKE4.WAV", 84592, 84535,
                 "1629b0015e5eba8f1c8b9d0504ae9c67280efa9bb5f742a450b346f38ca6bd53"),
            12: ("NULL.WAV", 64, 16,
                 "3a1a42d64f3295fa6bfa7cd321ebe66db702ec398fc958c16c5f9fe46bc325c4"),
            14: ("SELECT.WAV", 17264, 17217,
                 "3323d19462e645fae51ecefafdee7f256c76acb62bd1d6497833e1120f73fef6"),
            15: ("FIREBAL2.WAV", 34032, 33984,
                 "69122af4883939c44437cd5fbf6aac4696f97e238adc2c091f2b97a3deb1a0f2"),
            16: ("DIE.WAV", 34416, 34368,
                 "c86a379357ec7461ff209a08084f0701a56b6d37f6907f6dfa14126752bc8aed"),
            18: ("GETSPELL.WAV", 15120, 15072,
                 "580bce7d3576fb1dd4cea32a5880603f5abf265f4ae7580cc7d56dff119169c9"),
            19: ("SPEEDUP.WAV", 38032, 37975,
                 "86cdd986d164d95d21c52233f7c3e739e28343a26c8c2051a4027e96eb7b444f"),
            27: ("SPLASH2.WAV", 36000, 35945,
                 "15c2412f13d916a66a26877f763d72ca51e63a9db51096915236b0501dd157db"),
            29: ("CANTUSE.WAV", 6976, 6925,
                 "4378349675288905df83dc81add0a1735f3fa44eabeb2d2000b9bfd459bc989d"),
            30: ("METEOREX.WAV", 76272, 76223,
                 "162c06dbb4a9cdb10062bb25972681c036430476cc56906ce9c34b91a924d575"),
            31: ("MARKET.WAV", 193072, 193024,
                 "d9c4a1e8e8decc67bdae1426b83b777902d68a2bdfc20e591b507722251ef839"),
            33: ("ARROW1.WAV", 4144, 4096,
                 "e33d523c2e770a42d7fe3a02695e40f61923a0b75416b2b849a53463377b5c2f"),
            34: ("ARROW2.WAV", 6080, 6032,
                 "e7bfad156a297a29ad4c6d10fc0453333168606b69eb00f1f8f5a49c4529539c"),
            37: ("CYMERIAN.WAV", 39488, 39273,
                 "c3e81213dd5d3733a83d5f83b576feefe3e325c90087ab1b552c895bd1170b2b"),
            39: ("WYVERN.WAV", 74160, 74112,
                 "8f2eb4217c7f33afb922eda4be61194b94484a5afada83b969461027a9d8ad41"),
            40: ("MANASHOT.WAV", 18512, 18465,
                 "b7ef46837b325b2bacd6b83367a63070ac9c30a66dd1b09e4b095382c6f1989b"),
            41: ("SWITCH.WAV", 17264, 17216,
                 "39cf0c73c13dadac62bd3c5b132a5490adda3c54ef0c08acb9357d389e4281bc"),
            43: ("FIREFLY1.WAV", 30064, 29830,
                 "1ec43d872d6dd9e07a8d625d70a85ce39488de6914d4062aad4d89a6d19c5604"),
            44: ("FIREFLY2.WAV", 33440, 33203,
                 "591b2ed29b3501e75ecea2e90944a40610a751842ac3473b6933de05808c8c38"),
            46: ("GOAT.WAV", 20768, 20545,
                 "6038ceffaa2c54f9d4f7a93494fc713bc6bc3b76bc9a8ecc7dfc7f3fe4445b18"),
            54: ("HIT1.WAV", 5184, 4953,
                 "141d2a5552083a059affba266fe982231b309dcef97194b638aefe8497726d1a"),
            55: ("HIT2.WAV", 5440, 5217,
                 "de575886d3a96c1e2ba0a45011982dc9ef658c821313929dc88516bb1d1a2726"),
            56: ("HIT3.WAV", 5968, 5737,
                 "c53ec7e9c9fe7c81e1ab9e1975d22df64d78a88155e67ecec5d42765d944ff7a"),
            57: ("HIT4.WAV", 5024, 4801,
                 "c8235415ee99d0d4eeab7c1ed5ffdc672cda7de5c6074d65c9f713b5fcb12f2e"),
            63: ("SPELLUP.WAV", 25824, 25633,
                 "5291fd4907c8cf31c38b2b8c4f93ead6f69ea5f602365db6236437a91af21209"),
        }
        self.assertEqual(
            {
                row["sound_id"]: (
                    row["filename"],
                    row["record_bytes"],
                    row["wav"]["frame_count"],
                    row["record_sha256"],
                )
                for row in sound["level1_assets"]
            },
            expected_audio,
        )
        goat = next(
            row for row in sound["level1_assets"] if row["sound_id"] == 46
        )
        self.assertEqual(
            (
                goat["metadata_slot"],
                goat["relative_data_offset"],
                goat["metadata_word"],
                goat["asset_path"],
                goat["asset_sha256"],
            ),
            (
                45,
                1587168,
                90,
                "audio/sfx/046-goat.wav",
                "6038ceffaa2c54f9d4f7a93494fc713bc6bc3b76bc9a8ecc7dfc7f3fe4445b18",
            ),
        )
        self.assertEqual(sound["metadata_records"][46]["filename"], "DOORC2.WAV")
        self.assertTrue(
            all(
                row["wav"]["channels"] == 1
                and row["wav"]["sample_width_bytes"] == 1
                and row["wav"]["sample_rate"] == 22050
                for row in sound["level1_assets"]
            )
        )
        null_row = next(
            row for row in sound["level1_assets"] if row["sound_id"] == 12
        )
        self.assertIsNone(null_row["asset_path"])
        self.assertEqual(null_row["mapping_status"], "explicit_null")
        for row in sound["level1_assets"]:
            if row["sound_id"] == 12:
                continue
            payload = (self.first / row["asset_path"]).read_bytes()
            self.assertEqual(payload[:4], b"RIFF")
            self.assertEqual(hashlib.sha256(payload).hexdigest(), row["asset_sha256"])

        music = json.loads(
            (self.first / audio_asset["music_descriptor_path"]).read_text()
        )
        self.assertEqual(music["schema"], "mc2-level1-music-channel/v1")
        self.assertEqual(music["channel_counts_by_driver_index"], [2, 2, 2, 2])
        self.assertEqual(
            music["container"],
            {
                "container_trailer_offset": 643424,
                "descriptor_offset": 643464,
                "metadata_offset": 80656,
                "data_offset": 0,
                "metadata_size": 224,
                "data_size": 80656,
            },
        )
        self.assertEqual(
            [row["filename"] for row in music["metadata_records"]],
            [
                "C2GAME1.XMI", "C2GAME2.XMI", "C2GAME3.XMI",
                "C2SETUP.XMI", "C2INTRO.XMI", "C2CUTS.XMI",
            ],
        )
        self.assertEqual(
            (
                music["logical_track"],
                music["playback_slot"],
                music["loop_scan_pointer_slot"],
                music["payload_filename"],
            ),
            (1, 0, 1, "C2GAME1.XMI"),
        )
        self.assertEqual(
            music["xmi_sha256"],
            "0f8081d8ef6ad609f406638862a573b9d1d014f940f0f3194e4d5687f724bd8d",
        )
        self.assertEqual(
            music["midi_sha256"],
            "7e145e80b6acf1d3760511f649a4e4c7bc16639b3884797e09ae35e0f8186c12",
        )
        self.assertEqual(
            music["dynamic_control"],
            {
                "update_mode_field": "target_mode",
                "timer_multiplier": 3,
                "timer_hz_formula": "30 * timer_multiplier",
                "timer_hz": 90,
                "envelope_min": 0,
                "envelope_max": 127,
                "envelope_step_per_callback": 1,
                "playback_slot_census": {
                    "midi_format": 0,
                    "track_count": 1,
                    "division": 201,
                    "trigger_controller": 119,
                    "trigger_events": [],
                    "tagged_channels": [],
                    "loop_controller_events": [
                        {
                            "tick": 0,
                            "channel": 1,
                            "controller": 116,
                            "value": 0,
                        },
                        {
                            "tick": 158265,
                            "channel": 1,
                            "controller": 117,
                            "value": 127,
                        },
                    ],
                },
                "loop_scan_pointer_census": {
                    "midi_format": 0,
                    "track_count": 1,
                    "division": 167,
                    "trigger_controller": 119,
                    "trigger_events": [
                        {
                            "tick": 0,
                            "channel": 7,
                            "controller": 119,
                            "value": 0,
                        },
                        {
                            "tick": 0,
                            "channel": 8,
                            "controller": 119,
                            "value": 0,
                        },
                    ],
                    "tagged_channels": [7, 8],
                    "loop_controller_events": [
                        {
                            "tick": 0,
                            "channel": 2,
                            "controller": 116,
                            "value": 0,
                        },
                    ],
                },
                "audible_channel_resolution":
                    "Unresolved_playback_slot0_has_no_CC119_but_"
                    "loop_scan_slot1_tags_channels7_8",
            },
        )
        self.assertEqual((self.first / music["xmi_path"]).read_bytes()[:12],
                         b"FORM\x00\x00\x00\x0eXDIR")
        self.assertEqual((self.first / music["midi_path"]).read_bytes()[:4], b"MThd")
        self.assertEqual(
            music["unity_wave_sha256"],
            "03b3f951048504d8a6d30611d6c44a62ec1c463726b30c33c16e3cb69824605d",
        )
        self.assertEqual(
            music["unity_wave"],
            {
                "riff_declared_bytes": 77889580,
                "channels": 2,
                "sample_width_bytes": 2,
                "sample_rate": 44100,
                "frame_count": 19472384,
                "duration_numerator_frames": 19472384,
                "duration_denominator_hz": 44100,
            },
        )
        rendered_music = (self.first / music["unity_wave_path"]).read_bytes()
        self.assertEqual(len(rendered_music), 77889580)
        self.assertEqual(
            hashlib.sha256(rendered_music).hexdigest(),
            music["unity_wave_sha256"],
        )
        self.assertEqual(
            audio_asset["music_playback_status"],
            "Ready_deterministic_owner_local_adaptation_not_exact_DOS_timbre",
        )
        event_semantics = json.loads(
            (self.first / audio_asset["event_semantics_path"]).read_text()
        )
        self.assertEqual(
            event_semantics["consume_event_kinds"],
            [
                "PrepareSound",
                "EndLoop",
                "UpdateMusic",
                "StartMusic",
                "GoatSoundRequest",
                "ArcherArrowSound",
                "FireflyChargeSound",
                "FireballImpacted",
                "WaterSplashed",
                "CastleProjectileSpawned",
                "CastleConstructionSound",
                "ManaSphereClaimed",
                "MissionSound",
            ],
        )
        self.assertEqual(
            event_semantics["request_spell_activation_audio_status"],
            "NoDirectRequestMapping; successful Fireball spawn emits MissionSound9",
        )
        self.assertEqual(event_semantics["cave_ids"]["exact_range"], [65, 69])
        self.assertEqual(
            event_semantics["positional_surfaces"]["command_required_sound_ids"],
            [
                3, 4, 8, 9, 10, 15, 16, 18, 19, 27, 30, 33, 34, 37, 39,
                40, 41, 43, 44, 46, 54, 55, 56, 57, 63,
            ],
        )
        self.assertEqual(
            event_semantics["positional_surfaces"]["missing_command_reason"],
            "missing_positional_command",
        )
        self.assertEqual(
            event_semantics["positional_surfaces"]["attenuation_axis"],
            "XY_only_original_helper_despite_XYZ_name",
        )
        self.assertEqual(
            event_semantics["mission_event_sounds"],
            {
                "ArcherArrowSound": [33, 34],
                "FireflyChargeSound": [43, 44],
                "CastleProjectileSpawned": [15],
                "CastleConstructionSound": [10],
                "ManaSphereClaimed": [4],
                "FireballImpacted": [3],
                "WaterSplashed": [27],
                "MissionSound": [
                    8, 9, 16, 18, 19, 30, 40, 41, 54, 55, 56, 57, 63,
                ],
                "position_status":
                    "Ready_typed_source_slot_XYZ_integer_stereo_sidecar",
            },
        )
        self.assertEqual(
            event_semantics["positional_surfaces"]["unity_spatial_blend"], 0
        )
        self.assertEqual(
            event_semantics["goat_sound_request"],
            {
                "sound_id": 46,
                "event_kind": "GoatSoundRequest",
                "asset_status": "Ready_exact_owner_RIFF_WAVE",
                "position_source": "Rust_owned_live_Goat_snapshot",
                "play_type": "if_not_already_playing",
                "numeric_pitch_status":
                    "Ready_source_ordered_global_rng_and_action_branch",
                "presentation_policy":
                    "Play_only_from_authoritative_audio_sidecar",
            },
        )
        self.assertTrue(
            all(
                row["selector"] == expected
                for row, expected in zip(
                    objective["localization_records"], range(48, 54)
                )
            )
        )
        materials = manifest["assets"]["terrain_materials"]
        self.assertEqual(materials["bl32_tile_count"], 152)
        self.assertEqual(materials["tmap_frame_used_selectors"], [40, 41, 54, 63])
        self.assertEqual(materials["tmap_scene_reachable_selector_count"], 180)
        self.assertEqual(materials["tmap_decoded_texture_count"], 169)
        self.assertEqual(materials["tmap_raw_placeholder_count"], 11)
        descriptor = json.loads((self.first / materials["path"]).read_text())
        self.assertEqual(
            descriptor["bl32n"]["level1_used_selectors"],
            list(cache.LEVEL1_TERRAIN_SELECTORS),
        )
        self.assertEqual(descriptor["bl32n"]["level1_used_selector_count"], 65)
        self.assertEqual(descriptor["bl32n"]["level1_used_selector_max"], 139)
        tile_hashes = {
            row["selector"]: row["sha256"]
            for row in descriptor["bl32n"]["level1_used_tile_sha256"]
        }
        self.assertEqual(
            tile_hashes[5],
            "69c91a1e4673dc23a95e51632102a5ddd518ff1c7cfbd2d0e5b64e4fc2cd3ca6",
        )
        self.assertEqual(
            descriptor["palette"]["rgb6_to_rgb8"],
            "channel_u8 = channel_rgb6 * 4",
        )
        rgb6 = (self.first / descriptor["palette"]["path"]).read_bytes()
        rgb8 = (self.first / descriptor["palette"]["rgb8_path"]).read_bytes()
        self.assertEqual(rgb8, bytes(value * 4 for value in rgb6))
        self.assertEqual(max(rgb6), 63)
        self.assertEqual(max(rgb8), 252)
        self.assertEqual(descriptor["tmaps1"]["frame_used_selectors"], [40, 41, 54, 63])
        self.assertEqual(len(descriptor["tmaps1"]["scene_reachable_selectors"]), 180)
        self.assertTrue(
            set(cache.LEVEL1_LIVE_MISSION_TMAPS)
            <= set(descriptor["tmaps1"]["scene_reachable_selectors"])
        )
        frame_rows = {
            row["index"]: row
            for row in descriptor["tmaps1"]["textures"]
            if row["index"] in cache.FRAME_TMAPS
        }
        self.assertEqual(
            {index: (row["width"], row["height"]) for index, row in frame_rows.items()},
            {40: (40, 39), 41: (40, 39), 54: (36, 34), 63: (64, 53)},
        )
        self.assertTrue(all(row["frame_count"] == 1 for row in frame_rows.values()))
        placeholders = [
            row for row in descriptor["tmaps1"]["textures"]
            if row["status"] == "raw_single_byte_placeholder_nondecoded"
        ]
        self.assertEqual([row["index"] for row in placeholders], list(range(426, 437)))
        bl32 = self.first / materials["bl32_indices_path"]
        self.assertEqual(bl32.stat().st_size, 152 * 32 * 32)
        self.assertEqual(
            hashlib.sha256(bl32.read_bytes()).hexdigest(),
            "4ef5c6f6971dc470e9992e4cf3000a32e93312a9676ec2f94f4e809702ba3b5e",
        )
        shade_lut = self.first / descriptor["palette"]["shade_lut_path"]
        self.assertEqual(
            hashlib.sha256(shade_lut.read_bytes()).hexdigest(),
            "0772cd986b5725dd3224520f17d762aa2c317727bfddd953fae07b6150233f81",
        )
        shade = shade_lut.read_bytes()
        for brightness, source_index in ((0, 0), (17, 5), (40, 128), (63, 255)):
            self.assertEqual(
                shade[brightness * 256 + source_index],
                cache.DEFAULT_OWNER.joinpath("DATA/TABLESN.DAT").read_bytes()[
                    (brightness << 8) | source_index
                ],
            )

    def test_uv_source_lock_all_rows_groups_and_diagonals(self) -> None:
        rows = cache.parse_source_uv_rows()
        self.assertEqual(len(rows), 32)
        self.assertTrue(all(len(row) == 4 for row in rows))
        self.assertEqual(
            hashlib.sha256(cache.canonical_json(rows)).hexdigest(),
            "7f4af4c2a40950c06c512d672812f45329670c0334c830578e50ab53005953c0",
        )
        groups = cache.UNITY_TERRAIN_UV_GROUPS
        self.assertEqual(len(groups), 8)
        self.assertEqual(len(set(groups)), 8)
        self.assertEqual(
            groups,
            (
                ((0, 1), (1, 1), (0, 0), (1, 0)),
                ((1, 1), (0, 1), (1, 0), (0, 0)),
                ((0, 0), (1, 0), (0, 1), (1, 1)),
                ((1, 0), (0, 0), (1, 1), (0, 1)),
                ((0, 1), (0, 0), (1, 1), (1, 0)),
                ((0, 0), (0, 1), (1, 0), (1, 1)),
                ((1, 1), (1, 0), (0, 1), (0, 0)),
                ((1, 0), (1, 1), (0, 0), (0, 1)),
            ),
        )
        even = ((0, 2, 3), (0, 3, 1))
        odd = ((0, 2, 1), (2, 3, 1))
        self.assertEqual({frozenset((0, 3))}, {
            frozenset(set(even[0]) & set(even[1]))
        })
        self.assertEqual({frozenset((1, 2))}, {
            frozenset(set(odd[0]) & set(odd[1]))
        })

    def test_rnc_identity_ignores_host_compiler_metadata(self) -> None:
        self.assertEqual(
            cache.rnc_logical_identity("gcc 14 on host A"),
            cache.rnc_logical_identity("clang 19 on host B"),
        )
        self.assertNotIn("compiler", cache.rnc_logical_identity())

    def test_goat_tmap_states_match_exact_remc2_golden(self) -> None:
        golden = cache.load_goat_tmap_golden()
        manifest = cache.validate_cache(self.first)
        descriptor = json.loads(
            (
                self.first
                / manifest["assets"]["terrain_materials"]["path"]
            ).read_text()
        )
        textures = {
            row["index"]: row for row in descriptor["tmaps1"]["textures"]
        }
        exact_selectors = {
            index
            for index, row in textures.items()
            if row.get("animation_materialization", "").startswith(
                "exact_qualified_Goat_SS2"
            )
        }
        self.assertEqual(exact_selectors, cache.GOAT_TMAP_SELECTORS)
        for selector in sorted(cache.GOAT_TMAP_SELECTORS):
            decoded = (
                self.first / f"terrain/tmaps1/{selector:03d}.decoded.bin"
            ).read_bytes()
            states = cache.materialize_goat_tmap_animation(selector, decoded)
            cache.qualify_goat_tmap_states(selector, states, golden)
            rows = textures[selector]["animation_states"]
            self.assertEqual(
                [
                    [
                        row["state"],
                        row["sha256"],
                        row["stream_cursor"],
                        row["frame_index"],
                        row["width"],
                        row["height"],
                        row["bytes"],
                    ]
                    for row in rows
                ],
                golden["selectors"][str(selector)],
            )
            self.assertEqual(
                (self.first / rows[4]["indices_path"]).read_bytes(),
                (self.first / rows[0]["indices_path"]).read_bytes(),
            )
            self.assertEqual(
                (self.first / rows[5]["indices_path"]).read_bytes(),
                (self.first / rows[1]["indices_path"]).read_bytes(),
            )
        self.assertTrue(
            all(
                row["animation_materialization"].startswith("base_frame_only")
                for index, row in textures.items()
                if row.get("animated") and index not in cache.GOAT_TMAP_SELECTORS
            )
        )

    def test_goat_tmap_decoder_rejects_every_truncation(self) -> None:
        for selector in sorted(cache.GOAT_TMAP_SELECTORS):
            decoded = (
                self.first / f"terrain/tmaps1/{selector:03d}.decoded.bin"
            ).read_bytes()
            for end in range(len(decoded)):
                with self.assertRaises(cache.CacheError):
                    cache.materialize_goat_tmap_animation(selector, decoded[:end])

    def test_goat_tmap_decoder_rejects_malformed_qualified_subset(self) -> None:
        selector = 152
        original = (
            self.first / f"terrain/tmaps1/{selector:03d}.decoded.bin"
        ).read_bytes()
        _, width, height = cache.TMAP_HEADER.unpack_from(original)
        animation_header = cache.TMAP_HEADER.size + width * height
        stream_start = animation_header + cache.TMAP_ANIMATION_HEADER.size

        mutations: list[bytearray] = []

        def changed(offset: int, payload: bytes) -> None:
            value = bytearray(original)
            value[offset:offset + len(payload)] = payload
            mutations.append(value)

        changed(0, b"\x02\x14")  # wrong qualified flags
        changed(2, b"\x17\x00")  # wrong width
        changed(animation_header, b"\x03\x00")  # wrong frame count
        changed(animation_header + 2, b"\x2b\x05\x00\x00")  # wrong stream size
        changed(stream_start, b"\x05\x00\x00\x00")  # undersized outer
        changed(stream_start + 4, b"\x00\x00")  # wrong outer tag
        changed(stream_start + 6, b"\x02\x00")  # multiple children
        changed(stream_start + 8, b"\x01")  # nonzero reserved byte
        changed(stream_start + 16, b"\x05\x00\x00\x00")  # undersized child
        changed(stream_start + 20, b"\x0f\x00")  # unsupported child type
        changed(stream_start + 22, (height - 1).to_bytes(2, "little"))
        changed(stream_start + 24, b"\xff\xff")  # negative row control
        changed(stream_start + 27, b"\x00")  # zero SS2 span
        changed(stream_start + 26, bytes([width]))  # write crosses row

        trailing = bytearray(original + b"\x00")
        declared = len(original) - (animation_header + 6) + 1
        trailing[animation_header + 2:animation_header + 6] = (
            declared.to_bytes(4, "little")
        )
        mutations.append(trailing)

        for malformed in mutations:
            with self.assertRaises(cache.CacheError):
                cache.materialize_goat_tmap_animation(selector, bytes(malformed))

    def test_goat_tmap_materialized_frame_tamper_is_rejected(self) -> None:
        target = self.first / "terrain/tmaps1/152.frame-02.indices.u8"
        original = target.read_bytes()
        target.write_bytes(original[:10] + bytes([original[10] ^ 1]) + original[11:])
        with self.assertRaisesRegex(cache.CacheError, "hash mismatch"):
            cache.validate_cache(self.first)
        target.write_bytes(original)
        target.write_bytes(original[:-1])
        with self.assertRaisesRegex(cache.CacheError, "truncated"):
            cache.validate_cache(self.first)
        target.write_bytes(original)

    def test_tampered_output_is_rejected(self) -> None:
        target = self.first / "sky/skyn.indices.u8"
        original = target.read_bytes()
        target.write_bytes(bytes([original[0] ^ 1]) + original[1:])
        with self.assertRaisesRegex(cache.CacheError, "hash mismatch"):
            cache.validate_cache(self.first)
        target.write_bytes(original)

    def test_truncated_output_is_rejected(self) -> None:
        target = self.first / "palette/paln.rgb6"
        original = target.read_bytes()
        target.write_bytes(original[:-1])
        with self.assertRaisesRegex(cache.CacheError, "truncated"):
            cache.validate_cache(self.first)
        target.write_bytes(original)

    def test_goat_sound_tamper_and_truncation_are_rejected(self) -> None:
        target = self.first / "audio/sfx/046-goat.wav"
        original = target.read_bytes()
        target.write_bytes(original[:128] + bytes([original[128] ^ 1]) + original[129:])
        with self.assertRaisesRegex(cache.CacheError, "hash mismatch"):
            cache.validate_cache(self.first)
        target.write_bytes(original[:-1])
        with self.assertRaisesRegex(cache.CacheError, "truncated"):
            cache.validate_cache(self.first)
        target.write_bytes(original)

    def test_wrong_binding_is_rejected(self) -> None:
        with self.assertRaisesRegex(cache.CacheError, "package binding mismatch"):
            cache.validate_cache(self.first, "0" * 64, cache.EXPECTED_CLOSURE)
        with self.assertRaisesRegex(cache.CacheError, "closure binding mismatch"):
            cache.validate_cache(self.first, cache.EXPECTED_PACKAGE, "0" * 64)

    def test_source_frame_census_binds_accepted_native_final_main(self) -> None:
        manifest = cache.validate_cache(self.first)
        self.assertEqual(len(manifest["outputs"]), 1249)
        asset = manifest["assets"]["source_frame_census"]
        self.assertEqual(
            asset["status"],
            "accepted_fixed_profile_native_FINAL_MAIN_source_and_asset_census",
        )
        self.assertEqual(asset["final_main_classification"], "Exact")
        census = json.loads((self.first / asset["path"]).read_text())
        self.assertEqual(
            census["checkpoint_status"],
            {
                "source_frame_census_complete": True,
                "final_main_implementation":
                    "accepted_fixed_profile_native_FINAL_MAIN",
                "correspondence_layers": [
                    "sprite.source_hotspot_anchor",
                    "renderer.dos_depth_interleaving_aspect",
                ],
                "unknown_reachable_state_policy":
                    "hard_error_and_no_COMPLETE_flag",
            },
        )
        contract = census["native_final_main_contract"]
        self.assertEqual(contract["classification"], "Exact")
        self.assertEqual(contract["source_commit"], cache.PINNED_REMC2_COMMIT)
        self.assertEqual(contract["profile"]["framebuffer"], "320x200 indexed MAIN")
        self.assertEqual(
            contract["fail_closed_reachable_states"],
            [
                "active tutorial/briefing",
                "recursive help popup",
                "immobilization spider-web writer",
                "menu-state-7 score writer",
            ],
        )
        self.assertEqual(len(contract["source_anchors"]), 10)
        self.assertEqual(len(contract["ordered_closure"]), 9)
        self.assertEqual(
            contract["required_complete_flags"],
            [
                "MC_PRESENTATION_FRAME_COMPOSITOR_COMPLETE",
                "MC_PRESENTATION_FRAME_HAS_PROVENANCE",
                "MC_PRESENTATION_FRAME_OVERLAYS_COMPOSITED",
                "MC_PRESENTATION_FRAME_POSTPROCESS_COMPLETE",
                "MC_PRESENTATION_FRAME_LIGHTING_COMPLETE",
            ],
        )
        self.assertEqual(
            contract["forbidden_complete_flag"],
            "MC_TERRAIN_FRAME_NON_TERRAIN_OMITTED",
        )
        self.assertEqual(
            set(contract["implementation"]),
            {
                "source_tuple_and_tmap",
                "map_chain",
                "world_raster",
                "lighting",
                "postprocess",
                "overlay",
                "atomic_finalization",
                "unity",
            },
        )
        self.assertIn(
            "cross-timeline pixel equality", contract["explicit_nonclaims"]
        )
        self.assertIn(
            "stereo or alternate-resolution presentation",
            contract["explicit_nonclaims"],
        )
        self.assertTrue(
            all(
                "fail-closed before COMPLETE" in claim
                for claim in contract["explicit_nonclaims"][-4:]
            )
        )

    def test_manifest_tamper_is_rejected(self) -> None:
        path = self.first / "manifest.json"
        manifest = json.loads(path.read_text())
        manifest["outputs"][0]["sha256"] = "0" * 64
        original = path.read_bytes()
        path.write_bytes(cache.canonical_json(manifest))
        with self.assertRaisesRegex(cache.CacheError, "hash mismatch"):
            cache.validate_cache(self.first)
        path.write_bytes(original)

    def test_plan29_conformance_seal_is_contract_bound_and_tamper_evident(self) -> None:
        manifest = cache.validate_cache(self.first)
        seal_path = self.first / "conformance-identity.json"
        seal = json.loads(seal_path.read_text(encoding="utf-8"))
        contract, contract_self_sha256 = cache.load_conformance_contract()
        self.assertEqual(seal["schema"], cache.CONFORMANCE_SEAL_SCHEMA)
        self.assertEqual(
            seal["contract_file_sha256"],
            cache.sha256_file(cache.DEFAULT_CONFORMANCE_CONTRACT),
        )
        self.assertEqual(seal["contract_self_sha256"], contract_self_sha256)
        self.assertEqual(seal["identity"], contract["trusted_identity"])
        self.assertEqual(
            manifest["bindings"]["plan29_contract_file_sha256"],
            cache.sha256_file(cache.DEFAULT_CONFORMANCE_CONTRACT),
        )
        self.assertEqual(
            manifest["bindings"]["plan29_contract_self_sha256"],
            contract_self_sha256,
        )

        original = seal_path.read_bytes()
        seal["identity"]["row_id_sha256"] = "0" * 64
        seal_path.write_bytes(cache.canonical_json(seal))
        with self.assertRaisesRegex(cache.CacheError, "output hash mismatch"):
            cache.validate_cache(self.first)
        seal_path.write_bytes(original)

        self.assertIsNotNone(
            cache.validate_cache(
                self.first,
                contract_path=cache.DEFAULT_CONFORMANCE_CONTRACT,
                expected_contract_sha256="0" * 64,
            )
        )

    def test_extra_file_and_symlink_are_rejected(self) -> None:
        extra = self.first / "extra.bin"
        extra.write_bytes(b"x")
        with self.assertRaisesRegex(cache.CacheError, "extra files"):
            cache.validate_cache(self.first)
        extra.unlink()
        link = self.first / "extra-link"
        link.symlink_to(self.first / "palette/paln.rgb6")
        with self.assertRaisesRegex(cache.CacheError, "symlink"):
            cache.validate_cache(self.first)
        link.unlink()

    def test_malformed_rle_is_rejected(self) -> None:
        with self.assertRaisesRegex(cache.CacheError, "exceeds"):
            cache.materialize_indexed_rle(b"\x02\x01", 0, 1, 1)

    def test_owner_source_symlink_is_rejected(self) -> None:
        source_root = self.temp_root / "owner"
        (source_root / "DATA").mkdir(parents=True)
        target = source_root / "real"
        target.write_bytes(b"x")
        (source_root / "DATA/source").symlink_to(target)
        with self.assertRaisesRegex(cache.CacheError, "symlink"):
            cache.confined_source(source_root.resolve(), "DATA/source")

    def test_transaction_rolls_back_both_failure_points_and_recovers_startup(self) -> None:
        root = self.temp_root / "transaction"
        root.mkdir()
        output = root / "cache"
        output.mkdir()
        (output / "old").write_bytes(b"old")
        for point in ("after_backup", "after_install"):
            stage = root / ("stage-" + point)
            stage.mkdir()
            (stage / "new").write_bytes(b"new")
            with self.assertRaisesRegex(cache.CacheError, "injected"):
                cache.commit_stage(output, stage, point)
            self.assertEqual((output / "old").read_bytes(), b"old")
        backup = root / "cache.previous"
        output.rename(backup)
        stage = root / "stage-recovery"
        stage.mkdir()
        (stage / "new").write_bytes(b"new")
        cache.commit_stage(output, stage)
        self.assertEqual((output / "new").read_bytes(), b"new")

    def test_distributable_output_paths_are_rejected(self) -> None:
        with self.assertRaisesRegex(cache.CacheError, "distributable path"):
            cache.validate_output_path(cache.UNITY / "Assets/OwnerCache")
        with self.assertRaisesRegex(cache.CacheError, "distributable path"):
            cache.validate_output_path(cache.UNITY / "Builds/OwnerCache")


class Level2DayMaterialTests(unittest.TestCase):
    """Focused acceptance coverage for the additive Level 2 Day profile seam."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.temp_root = Path(
            tempfile.mkdtemp(prefix="level2-day-material-test.", dir=cache.GENERATED)
        )
        cls.first = cls.temp_root / "first"
        cls.second = cls.temp_root / "second"
        cache.build_day_material_cache(
            cache.DEFAULT_OWNER,
            cache.DEFAULT_LEVEL2_PACKAGE,
            cache.DEFAULT_LEVEL2_CLOSURE,
            cache.DEFAULT_CATALOG,
            cls.first,
        )
        cache.build_day_material_cache(
            cache.DEFAULT_OWNER,
            cache.DEFAULT_LEVEL2_PACKAGE,
            cache.DEFAULT_LEVEL2_CLOSURE,
            cache.DEFAULT_CATALOG,
            cls.second,
        )

    @classmethod
    def tearDownClass(cls) -> None:
        shutil.rmtree(cls.temp_root)

    def test_byte_identical_profile_bound_rebuild_and_gpu_bounds(self) -> None:
        self.assertEqual(tree_identity(self.first), tree_identity(self.second))
        manifest = cache.validate_day_material_cache(
            self.first,
            cache.EXPECTED_LEVEL2_PACKAGE,
            cache.EXPECTED_LEVEL2_CLOSURE,
        )
        package = json.loads(
            (cache.DEFAULT_LEVEL2_PACKAGE / "manifest.json").read_text(encoding="utf-8")
        )
        self.assertEqual(package["schema"], "mc2-owner-level-export/v5")
        self.assertEqual(package["package_sha256"], cache.EXPECTED_LEVEL2_PACKAGE)
        self.assertEqual(
            manifest["profile"]["owner_package"], cache.LEVEL2_OWNER_PACKAGE_IDENTITY
        )
        self.assertEqual(manifest["profile"]["external_level"], 2)
        self.assertEqual(manifest["profile"]["internal_level"], 1)
        self.assertEqual(manifest["profile"]["map_type"], {"id": 0, "name": "Day"})
        descriptor = json.loads(
            (self.first / "materials/descriptor.json").read_text(encoding="utf-8")
        )
        self.assertEqual(descriptor["palette"]["entries"], 256)
        self.assertEqual(descriptor["tablesd"]["bytes"], 83456)
        self.assertEqual(descriptor["clrd"]["entries"], 4096)
        self.assertEqual(descriptor["bl32n"]["tile_count"], 152)
        self.assertEqual(len(descriptor["tmaps0"]["selectors"]), 149)
        self.assertTrue(
            {
                *range(8, 11),
                *range(14, 22),
                *range(30, 38),
                *range(138, 141),
                158,
            }.issubset(descriptor["tmaps0"]["selectors"])
        )
        self.assertEqual(descriptor["gpu_ready"]["indexed_format"], "R8_UNORM")
        with self.assertRaisesRegex(cache.CacheError, "closure binding mismatch"):
            cache.validate_day_material_cache(
                self.first,
                cache.EXPECTED_LEVEL2_PACKAGE,
                "bee3b7a4fd585d6631523b87217978cbfa58ed08cd494fa7921486053de1dccc",
            )

    def test_material_manifest_tamper_is_rejected(self) -> None:
        path = self.first / "manifest.json"
        original = path.read_bytes()
        manifest = json.loads(original)
        manifest["outputs"][0]["sha256"] = "0" * 64
        path.write_bytes(cache.canonical_json(manifest))
        with self.assertRaisesRegex(cache.CacheError, "metadata hash mismatch"):
            cache.validate_day_material_cache(self.first)
        path.write_bytes(original)

    def test_owner_component_identity_tamper_is_rejected(self) -> None:
        path = self.first / "manifest.json"
        original = path.read_bytes()
        manifest = json.loads(original)
        manifest["profile"]["owner_package"]["players_component"]["version"] = 3
        manifest["bindings"]["players_component_version"] = 3
        unsigned = dict(manifest)
        unsigned.pop("manifest_payload_sha256", None)
        manifest["manifest_payload_sha256"] = cache.sha256_bytes(
            cache.canonical_json(unsigned)
        )
        path.write_bytes(cache.canonical_json(manifest))
        with self.assertRaisesRegex(cache.CacheError, "owner package identity mismatch"):
            cache.validate_day_material_cache(self.first)
        path.write_bytes(original)

    def test_cross_profile_and_closure_tamper_fail_before_materialization(self) -> None:
        with self.assertRaisesRegex(cache.CacheError, "requires external=2"):
            cache.build_day_material_cache(
                cache.DEFAULT_OWNER,
                cache.DEFAULT_PACKAGE,
                cache.DEFAULT_CLOSURE,
                cache.DEFAULT_CATALOG,
                self.temp_root / "wrong-profile",
            )
        package_copy = self.temp_root / "package-selector-tampered"
        shutil.copytree(cache.DEFAULT_LEVEL2_PACKAGE, package_copy)
        package_manifest_path = package_copy / "manifest.json"
        package_manifest = json.loads(package_manifest_path.read_text(encoding="utf-8"))
        package_manifest["presentation_dependencies"]["resolved_files"]["block_bank"] = (
            "owner:CD_Files/DATA/BL32F0-0.DAT"
        )
        package_manifest_path.write_bytes(cache.canonical_json(package_manifest))
        with self.assertRaisesRegex(cache.CacheError, "resolved owner-file selector map"):
            cache.build_day_material_cache(
                cache.DEFAULT_OWNER,
                package_copy,
                cache.DEFAULT_LEVEL2_CLOSURE,
                cache.DEFAULT_CATALOG,
                self.temp_root / "tampered-package-selector",
            )
        closure_copy = self.temp_root / "closure.json"
        closure = json.loads(cache.DEFAULT_LEVEL2_CLOSURE.read_text(encoding="utf-8"))
        closure["sets"]["scene_reachable"]["assets"] = []
        unsigned_closure = dict(closure)
        unsigned_closure.pop("closure_sha256", None)
        closure["closure_sha256"] = cache.sha256_bytes(
            json.dumps(
                unsigned_closure,
                sort_keys=True,
                separators=(",", ":"),
                ensure_ascii=True,
            ).encode("utf-8")
        )
        closure_copy.write_bytes(cache.canonical_json(closure))
        with self.assertRaisesRegex(cache.CacheError, "wrong qualified Level 2 closure hash"):
            cache.build_day_material_cache(
                cache.DEFAULT_OWNER,
                cache.DEFAULT_LEVEL2_PACKAGE,
                closure_copy,
                cache.DEFAULT_CATALOG,
                self.temp_root / "tampered-closure",
            )

    def test_material_symlink_is_rejected(self) -> None:
        path = self.first / "palette/pald.rgb6"
        original = path.read_bytes()
        path.unlink()
        path.symlink_to(self.second / "palette/pald.rgb6")
        with self.assertRaisesRegex(cache.CacheError, "contains symlink"):
            cache.validate_day_material_cache(self.first)
        path.unlink()
        path.write_bytes(original)


class CorrectedDayMaterialTests(unittest.TestCase):
    def test_v2_aggregate_binds_corrected_material_and_rejects_version_downgrade(self) -> None:
        source = cache.GENERATED / "level3-day-block32-owner-cache-v2"
        inputs = {
            "owner_export_dir": cache.GENERATED / "level3-day-block32-owner-v2",
            "campaign_package_dir": cache.GENERATED / "level3-day-block32-campaign-recipe-v2",
            "closure_path": cache.GENERATED / "day-material-v2.j6oRrc/closure-runtime-v2.json",
        }
        manifest = cache.validate_campaign_day_aggregate_cache(source, **inputs)
        self.assertEqual(manifest["schema"], cache.CAMPAIGN_DAY_AGGREGATE_SCHEMA_V2)
        self.assertEqual(manifest["components"]["material"]["schema"], cache.DAY_MATERIAL_SCHEMA_V2)
        with tempfile.TemporaryDirectory(prefix="day-aggregate-v2-negative.", dir=cache.GENERATED) as directory:
            target = Path(directory) / "cache"
            shutil.copytree(source, target)
            manifest["schema"] = cache.CAMPAIGN_DAY_AGGREGATE_SCHEMA
            manifest.pop("manifest_payload_sha256")
            manifest["manifest_payload_sha256"] = cache.sha256_bytes(cache.canonical_json(manifest))
            (target / "manifest.json").write_bytes(cache.canonical_json(manifest))
            with self.assertRaisesRegex(cache.CacheError, "unsupported campaign Day aggregate schema"):
                cache.validate_campaign_day_aggregate_cache(target, **inputs)

    def test_v2_requires_day_bank_even_after_manifest_rehash(self) -> None:
        source = cache.GENERATED / "level3-day-block32-material-cache-v2"
        manifest = cache.validate_campaign_day_material_cache(source)
        self.assertEqual(manifest["schema"], cache.DAY_MATERIAL_SCHEMA_V2)
        with tempfile.TemporaryDirectory(prefix="day-v2-negative.") as directory:
            target = Path(directory) / "cache"
            shutil.copytree(source, target)
            manifest["profile"]["selectors"]["block_bank"] = "BL32N0-0"
            manifest.pop("manifest_payload_sha256")
            manifest["manifest_payload_sha256"] = cache.sha256_bytes(cache.canonical_json(manifest))
            (target / "manifest.json").write_bytes(cache.canonical_json(manifest))
            with self.assertRaisesRegex(cache.CacheError, "selector identity"):
                cache.validate_campaign_day_material_cache(target)


class CampaignDayMaterialTests(unittest.TestCase):
    """Additive campaign-Day dialect without a Level-N runtime branch."""

    @classmethod
    def setUpClass(cls) -> None:
        package_value = os.environ.get("MC_CAMPAIGN_PACKAGE_LEVEL3_V1")
        if not package_value:
            raise unittest.SkipTest("MC_CAMPAIGN_PACKAGE_LEVEL3_V1 is not set")
        cls.package = Path(package_value)
        cls.temp_root = Path(
            tempfile.mkdtemp(prefix="campaign-day-material-test.", dir=cache.GENERATED)
        )
        cls.first = cls.temp_root / "first"
        cls.second = cls.temp_root / "second"
        for output in (cls.first, cls.second):
            cache.build_campaign_day_material_cache(
                cache.DEFAULT_OWNER,
                cache.DEFAULT_CAMPAIGN_DAY_OWNER_EXPORT,
                cls.package,
                cache.DEFAULT_CAMPAIGN_DAY_CLOSURE,
                cache.DEFAULT_CATALOG,
                output,
            )

    @classmethod
    def tearDownClass(cls) -> None:
        if hasattr(cls, "temp_root"):
            shutil.rmtree(cls.temp_root)

    def test_neutral_cache_is_repeatable_and_binds_both_packages(self) -> None:
        self.assertEqual(tree_identity(self.first), tree_identity(self.second))
        campaign = json.loads((self.package / "manifest.json").read_text())
        owner = json.loads(
            (cache.DEFAULT_CAMPAIGN_DAY_OWNER_EXPORT / "manifest.json").read_text()
        )
        closure = json.loads(cache.DEFAULT_CAMPAIGN_DAY_CLOSURE.read_text())
        manifest = cache.validate_campaign_day_material_cache(
            self.first,
            owner["package_sha256"],
            campaign["package_sha256"],
            closure["closure_sha256"],
        )
        self.assertEqual(manifest["schema"], cache.DAY_MATERIAL_SCHEMA)
        self.assertEqual(manifest["profile"]["name"], "day-material")
        self.assertEqual(len(manifest["outputs"]), 333)
        descriptor = json.loads(
            (self.first / "materials/descriptor.json").read_text()
        )
        self.assertEqual(descriptor["schema"], cache.DAY_MATERIAL_DESCRIPTOR_SCHEMA)
        self.assertEqual(len(descriptor["tmaps0"]["selectors"]), 162)
        self.assertTrue(set(range(14, 30)).issubset(descriptor["tmaps0"]["selectors"]))
        self.assertTrue(set(range(110, 116)).issubset(descriptor["tmaps0"]["selectors"]))
        self.assertTrue({152, 153, 154}.issubset(descriptor["tmaps0"]["selectors"]))
        self.assertTrue(
            {138, 139, 140}.isdisjoint(descriptor["tmaps0"]["selectors"])
        )

    def test_neutral_cache_rejects_cross_binding_and_tree_tamper(self) -> None:
        with self.assertRaisesRegex(cache.CacheError, "campaign package binding mismatch"):
            cache.validate_campaign_day_material_cache(
                self.first,
                expected_campaign_package="0" * 64,
            )
        extra = self.first / "unexpected.bin"
        extra.write_bytes(b"not declared")
        with self.assertRaisesRegex(cache.CacheError, "missing or extra files"):
            cache.validate_campaign_day_material_cache(self.first)
        extra.unlink()

    def test_neutral_cache_rejects_missing_model9_runtime_selector(self) -> None:
        selector = self.first / "terrain/tmaps0/112.indices.u8"
        selector.rename(selector.with_suffix(".missing"))
        try:
            with self.assertRaisesRegex(
                cache.CacheError,
                "output missing or truncated: terrain/tmaps0/112.indices.u8",
            ):
                cache.validate_campaign_day_material_cache(self.first)
        finally:
            selector.with_suffix(".missing").rename(selector)


class CampaignDayGameplayAggregateTests(unittest.TestCase):
    """Level 3 package-present materials plus gameplay/HUD presentation."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.temp_root = Path(
            tempfile.mkdtemp(prefix="campaign-day-owner-cache-test.", dir=cache.GENERATED)
        )
        cls.first = cls.temp_root / "first"
        cls.second = cls.temp_root / "second"
        for output in (cls.first, cls.second):
            cache.build_campaign_day_aggregate_cache(
                cache.DEFAULT_OWNER,
                cache.DEFAULT_CAMPAIGN_DAY_OWNER_EXPORT,
                cache.DEFAULT_CAMPAIGN_DAY_PACKAGE,
                cache.DEFAULT_CAMPAIGN_DAY_CLOSURE,
                cache.DEFAULT_CATALOG,
                output,
            )

    @classmethod
    def tearDownClass(cls) -> None:
        shutil.rmtree(cls.temp_root)

    def test_repeatable_exact_level3_package_present_membership(self) -> None:
        self.assertEqual(tree_identity(self.first), tree_identity(self.second))
        manifest = cache.validate_campaign_day_aggregate_cache(self.first)
        self.assertEqual(manifest["schema"], cache.CAMPAIGN_DAY_AGGREGATE_SCHEMA)
        self.assertEqual(manifest["profile"]["external_level"], 3)
        self.assertEqual(manifest["profile"]["internal_level"], 2)
        self.assertEqual(set(manifest["components"]), {"material", "gameplay", "audio"})
        # GOLDEN-CHANGE: six source-owned model-9 TMAP selectors add twelve
        # decoded material outputs and six closure-bound gameplay outputs.
        self.assertEqual(len(manifest["outputs"]), 662)
        self.assertEqual(
            manifest["presentation_scope"]["audio"],
            "materialized_from_source_qualified_level3_occurrences",
        )

        audio = json.loads(
            (self.first / "components/audio/audio/descriptor.json").read_text()
        )
        self.assertEqual(len(audio["sound"]["occurrences"]), 34)
        self.assertEqual(len(audio["excluded"]), 85)
        self.assertEqual(
            [row["sound_id"] for row in audio["sound"]["records"]],
            [1, 2, 3, 5, 8, 9, 10, 12, 13, 14, 15, 16, 18, 19, 23, 29, 30, 31, 40, 41, 43, 44, 47, 54, 55, 56, 57, 60, 61],
        )
        self.assertEqual(audio["music"]["maptype_music"], 2)
        self.assertEqual(audio["music"]["lifecycle"], ["StartMusic", "FadeWarMusic", "UpdateMusic", "StopMusic"])
        self.assertEqual(
            audio["music"]["unity_wave_sha256"],
            "03b3f951048504d8a6d30611d6c44a62ec1c463726b30c33c16e3cb69824605d",
        )
        self.assertEqual(
            audio["music"]["unity_wave"],
            {
                "riff_declared_bytes": 77889580,
                "channels": 2,
                "sample_width_bytes": 2,
                "sample_rate": 44100,
                "frame_count": 19472384,
                "duration_numerator_frames": 19472384,
                "duration_denominator_hz": 44100,
            },
        )
        self.assertEqual(
            audio["music"]["dynamic_control"]["playback_slot_census"]["division"],
            201,
        )
        self.assertEqual(
            audio["music"]["dynamic_control"]["loop_scan_pointer_census"]["tagged_channels"],
            [7, 8],
        )
        self.assertEqual(
            (self.first / "components/audio" / audio["music"]["midi_path"]).read_bytes()[:4],
            b"MThd",
        )

        descriptor = json.loads(
            (
                self.first / "components/gameplay/gameplay/descriptor.json"
            ).read_text(encoding="utf-8")
        )
        closure = json.loads(cache.DEFAULT_CAMPAIGN_DAY_CLOSURE.read_text())
        catalog, _ = cache._level2_catalog_identity(cache.DEFAULT_CATALOG)
        rows = {
            row.get("path"): row for row in catalog.get("files", [])
        }
        rows.update(
            {row.get("asset_id"): row for row in catalog.get("entries", [])}
        )
        scene, dynamic, selected, excluded = cache._day_gameplay_selected_rows(
            closure, rows
        )
        self.assertEqual(
            [row["asset_id"] for row in descriptor["assets"]],
            [row["asset_id"] for row in selected],
        )
        self.assertEqual(descriptor["excluded"], excluded)
        self.assertEqual(len(scene), 174)
        # GOLDEN-CHANGE: model-9 particles 201/202 contribute the six exact
        # movement/reaction TMAP rows selected by their source render modes.
        self.assertEqual(len(dynamic), 52)
        self.assertEqual(len(selected), 215)
        self.assertEqual(descriptor["source_sets"]["day_hud"]["output_count"], 75)
        self.assertEqual(
            manifest["bindings"]["campaign_package_sha256"],
            "6f17fe1c33c5852796b08acba2c44e5073939b947975b4f3b330d2c45b62d18d",
        )
        self.assertEqual(
            manifest["bindings"]["level_asset_closure_sha256"],
            "d6e909d65437a37fed699429efa968175f39d6253c350537a1c82ce9729d5871",
        )

    def test_tamper_symlink_and_campaign_package_mismatch_fail_closed(self) -> None:
        descriptor = self.first / "components/gameplay/gameplay/descriptor.json"
        original = descriptor.read_bytes()
        descriptor.write_bytes(original + b"tamper")
        with self.assertRaisesRegex(cache.CacheError, "output hash mismatch"):
            cache.validate_campaign_day_aggregate_cache(self.first)
        descriptor.write_bytes(original)

        victim = self.first / "components/gameplay/gameplay/hud/palette/pald.rgb6"
        victim_original = victim.read_bytes()
        victim.unlink()
        victim.symlink_to(
            self.second / "components/gameplay/gameplay/hud/palette/pald.rgb6"
        )
        with self.assertRaisesRegex(cache.CacheError, "missing or symlinked"):
            cache.validate_campaign_day_aggregate_cache(self.first)
        victim.unlink()
        victim.write_bytes(victim_original)

        wav = self.first / "components/audio/audio/sfx/012.wav"
        wav_original = wav.read_bytes()
        wav.write_bytes(wav_original + b"tamper")
        with self.assertRaisesRegex(cache.CacheError, "output hash mismatch"):
            cache.validate_campaign_day_aggregate_cache(self.first)
        wav.write_bytes(wav_original)

        music_wave = self.first / "components/audio/audio/music/C2GAME1-adlmidi-bank59.wav"
        with music_wave.open("r+b") as stream:
            stream.seek(-1, os.SEEK_END)
            last = stream.read(1)
            stream.seek(-1, os.SEEK_END)
            stream.write(bytes([last[0] ^ 1]))
        with self.assertRaisesRegex(cache.CacheError, "output hash mismatch"):
            cache.validate_campaign_day_aggregate_cache(self.first)
        with music_wave.open("r+b") as stream:
            stream.seek(-1, os.SEEK_END)
            last = stream.read(1)
            stream.seek(-1, os.SEEK_END)
            stream.write(bytes([last[0] ^ 1]))

        wrong_package = self.temp_root / "wrong-campaign-package"
        shutil.copytree(cache.DEFAULT_CAMPAIGN_DAY_PACKAGE, wrong_package)
        wrong_manifest = wrong_package / "manifest.json"
        package = json.loads(wrong_manifest.read_text(encoding="utf-8"))
        package["package_sha256"] = "0" * 64
        wrong_manifest.write_bytes(cache.canonical_json(package))
        with self.assertRaisesRegex(
            cache.CacheError, "campaign Day package identity verification failed"
        ):
            cache.validate_campaign_day_aggregate_cache(
                self.first, campaign_package_dir=wrong_package
            )

    def test_level3_audio_denominator_rebuilds_exactly(self) -> None:
        builder_path = (
            cache.PRIVATE
            / "remc2-sim-rs/level3-conformance/audio/build_level3_audio_denominator.py"
        )
        spec = importlib.util.spec_from_file_location("level3_audio_denominator", builder_path)
        self.assertIsNotNone(spec)
        assert spec and spec.loader
        builder = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(builder)
        expected = cache.CAMPAIGN_DAY_AUDIO_DENOMINATOR.read_bytes()
        self.assertEqual(builder.canonical(builder.build()), expected)
        denominator = json.loads(expected)
        reachable = [
            row for row in denominator["audio"]["rows"]
            if row["classification"] == "reachable_level3"
        ]
        self.assertEqual(len(reachable), 34)
        self.assertEqual(len({row["stable_occurrence_hash_sha256"] for row in reachable}), 34)

    def test_level3_audio_rejects_unsupported_occurrence_selector(self) -> None:
        tampered = self.temp_root / "tampered-audio-selector"
        shutil.copytree(self.first / "components/audio", tampered)
        descriptor_path = tampered / "audio/descriptor.json"
        descriptor = json.loads(descriptor_path.read_text())
        descriptor["sound"]["occurrences"][0]["sound_selector"]["sound_ids"] = [64]
        descriptor_path.write_bytes(cache.canonical_json(descriptor))
        manifest_path = tampered / "manifest.json"
        manifest = json.loads(manifest_path.read_text())
        for row in manifest["outputs"]:
            if row["path"] == "audio/descriptor.json":
                row["bytes"] = descriptor_path.stat().st_size
                row["sha256"] = cache.sha256_file(descriptor_path)
        manifest["cache_content_sha256"] = cache._level2_cache_identity(manifest["outputs"])
        unsigned = dict(manifest)
        unsigned.pop("manifest_payload_sha256", None)
        manifest["manifest_payload_sha256"] = cache.sha256_bytes(cache.canonical_json(unsigned))
        manifest_path.write_bytes(cache.canonical_json(manifest))
        with self.assertRaisesRegex(cache.CacheError, "occurrence descriptor changed"):
            cache.validate_campaign_day_audio_cache(tampered)


class Level2HistoricalV4InspectionTests(unittest.TestCase):
    """Retained v4 artifacts remain inspectable but cannot re-enter production."""

    def test_v4_cache_and_package_are_inspectable_only(self) -> None:
        package = json.loads((LEVEL2_V4_PACKAGE / "manifest.json").read_text())
        closure = json.loads(LEVEL2_V4_CLOSURE.read_text())
        self.assertEqual(package["schema"], "mc2-owner-level-export/v4")
        self.assertNotIn("owner_package", closure["profile"])
        self.assertEqual(
            cache.validate_day_material_cache(
                LEVEL2_V4_MATERIAL,
                LEVEL2_V4_PACKAGE_SHA256,
                LEVEL2_V4_CLOSURE_SHA256,
                expected_owner_identity=None,
            )["profile"]["package_sha256"],
            LEVEL2_V4_PACKAGE_SHA256,
        )
        self.assertEqual(
            cache.validate_day_gameplay_cache(
                LEVEL2_V4_GAMEPLAY,
                LEVEL2_V4_PACKAGE_SHA256,
                LEVEL2_V4_CLOSURE_SHA256,
                expected_owner_identity=None,
            )["profile"]["closure_sha256"],
            LEVEL2_V4_CLOSURE_SHA256,
        )
        self.assertEqual(
            cache.validate_day_aggregate_cache(
                LEVEL2_V4_AGGREGATE,
                LEVEL2_V4_PACKAGE_SHA256,
                LEVEL2_V4_CLOSURE_SHA256,
                expected_owner_identity=None,
                package_dir=LEVEL2_V4_PACKAGE,
                closure_path=LEVEL2_V4_CLOSURE,
                catalog_path=cache.DEFAULT_CATALOG,
            )["profile"]["package_sha256"],
            LEVEL2_V4_PACKAGE_SHA256,
        )
        # The retained closure deliberately preserves its historical lowercase
        # map tuple, so the production resolver rejects it at its profile gate
        # before it would inspect the v5-only owner-component descriptor.
        with self.assertRaisesRegex(
            cache.CacheError, "requires external=2/internal=1/Day"
        ):
            cache._level2_profile(
                LEVEL2_V4_PACKAGE,
                LEVEL2_V4_CLOSURE,
                cache.DEFAULT_CATALOG,
            )
        # The aggregate source ledger derives from the retained package and
        # closure, without admitting v4 through the qualified v5 resolver.
        allowed = cache._level2_aggregate_allowed_source_ids(
            LEVEL2_V4_PACKAGE,
            LEVEL2_V4_CLOSURE,
            cache.DEFAULT_CATALOG,
            expected_package=LEVEL2_V4_PACKAGE_SHA256,
            expected_closure=LEVEL2_V4_CLOSURE_SHA256,
            expected_owner_identity=None,
        )
        self.assertIn("owner_data:DATA/PALD-0.DAT", allowed)


class Level2DayGameplayTests(unittest.TestCase):
    """Production-path coverage for closure-rooted gameplay/HUD/text spans."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.temp_root = Path(
            tempfile.mkdtemp(prefix="level2-day-gameplay-test.", dir=cache.GENERATED)
        )
        cls.first = cls.temp_root / "first"
        cls.second = cls.temp_root / "second"
        for output in (cls.first, cls.second):
            cache.build_day_gameplay_cache(
                cache.DEFAULT_OWNER,
                cache.DEFAULT_LEVEL2_PACKAGE,
                cache.DEFAULT_LEVEL2_CLOSURE,
                cache.DEFAULT_CATALOG,
                output,
            )

    @classmethod
    def tearDownClass(cls) -> None:
        shutil.rmtree(cls.temp_root)

    def test_deterministic_content_and_selector_metadata(self) -> None:
        self.assertEqual(tree_identity(self.first), tree_identity(self.second))
        manifest = cache.validate_day_gameplay_cache(self.first)
        self.assertGreaterEqual(len(manifest["outputs"]), 160)
        descriptor = json.loads(
            (self.first / "gameplay/descriptor.json").read_text(encoding="utf-8")
        )
        assets = {row["asset_id"]: row for row in descriptor["assets"]}
        self.assertIn("owner_data:DATA/BUILD0-0.DAT#footprint-26", assets)
        self.assertIn("owner_data:DATA/FONT1.DAT#100", assets)
        self.assertIn("owner_data:LANGUAGE/L1.TXT#string-54", assets)
        self.assertIn("owner_data:DATA/POINTERS.DAT#9", assets)
        self.assertEqual(descriptor["source_sets"]["materialized"], len(assets))
        self.assertTrue(descriptor["semantics"]["selector_metadata_required"])
        for row in assets.values():
            self.assertTrue(row["selector_kind"])
            self.assertTrue(row["extent"]["status"])
            self.assertTrue(row["mode_status"])
            self.assertTrue(row["hotspot_status"])

    def test_day_hud_decodes_exact_day_selectors_and_roman_tiers(self) -> None:
        expected_selectors = [
            1,
            2,
            40,
            41,
            42,
            43,
            54,
            55,
            *range(123, 149),
        ]
        self.assertEqual(tree_identity(self.first), tree_identity(self.second))
        manifest = cache.validate_day_gameplay_cache(self.first)
        descriptor = json.loads(
            (self.first / "gameplay/descriptor.json").read_text(encoding="utf-8")
        )
        hud = descriptor["hud"]
        self.assertEqual(hud["schema"], "mc2-level2-day-hud/v1")
        self.assertEqual(hud["source"]["bitmap_bank"], "MSPRD0-0")
        self.assertEqual(hud["source"]["palette_bank"], "PALD-0")
        self.assertEqual(hud["source"]["font_bank"], "FONT1")
        self.assertEqual(hud["bitmap_selectors"], expected_selectors)
        self.assertEqual(
            [row["selector"] for row in hud["bitmap_assets"]], expected_selectors
        )
        self.assertEqual(
            hud["spell_tier_text"]["texts"], ["I", "II", "III", "IV", "V"]
        )
        self.assertEqual(len(descriptor["hud_outputs"]), 75)
        self.assertEqual(descriptor["source_sets"]["day_hud"]["output_count"], 75)
        self.assertNotIn("HSPRN", json.dumps(hud, sort_keys=True))
        output_by_path = {row["path"]: row for row in manifest["outputs"]}
        for row in hud["bitmap_assets"]:
            self.assertEqual(
                row["source_asset_id"],
                f"owner_data:DATA/MSPRD0-0.DAT#{row['selector']}",
            )
            self.assertEqual(row["tab_asset_id"], "owner_data:DATA/MSPRD0-0.TAB")
            self.assertEqual(row["palette_asset_id"], "owner_data:DATA/PALD-0.DAT")
            expected_bytes = row["stored_width"] * row["stored_height"]
            self.assertEqual(row["source_scale"], 2)
            self.assertEqual(row["runtime_width"], row["stored_width"] * 2)
            self.assertEqual(row["runtime_height"], row["stored_height"] * 2)
            self.assertEqual(
                row["runtime_dimensions"],
                {"width": row["runtime_width"], "height": row["runtime_height"]},
            )
            indices_output = output_by_path[row["indices_path"]]
            mask_output = output_by_path[row["mask_path"]]
            self.assertEqual(indices_output["bytes"], expected_bytes)
            self.assertEqual(mask_output["bytes"], expected_bytes)
            self.assertTrue(
                set((self.first / row["mask_path"]).read_bytes()).issubset({0, 1})
            )
            self.assertEqual(indices_output["source_asset_ids"], [
                "owner_data:DATA/MSPRD0-0.DAT",
                "owner_data:DATA/MSPRD0-0.TAB",
                "owner_data:DATA/PALD-0.DAT",
            ])
        for text, tier in zip(
            ["I", "II", "III", "IV", "V"], hud["spell_tier_text"]["tiers"]
        ):
            self.assertEqual(tier["text"], text)
            self.assertEqual(tier["glyph_indices"], [ord(value) + 1 for value in text])
            self.assertEqual(tier["font_bank"], "FONT1")
            self.assertEqual(tier["stored_width"], tier["width"])
            self.assertEqual(tier["stored_height"], tier["height"])
            self.assertEqual(tier["source_scale"], 2)
            self.assertEqual(tier["runtime_width"], tier["width"] * 2)
            self.assertEqual(tier["runtime_height"], tier["height"] * 2)
            self.assertEqual(
                tier["runtime_dimensions"],
                {"width": tier["runtime_width"], "height": tier["runtime_height"]},
            )
            self.assertEqual(
                output_by_path[tier["raster_path"]]["bytes"],
                tier["width"] * tier["height"] * 4,
            )
            self.assertTrue(all(
                glyph["source_asset_id"].startswith("owner_data:DATA/FONT1.DAT#")
                for glyph in tier["glyphs"]
            ))

    def test_day_hud_unknown_selector_and_missing_outputs_fail_closed(self) -> None:
        tampered = self.temp_root / "hud-selector-tamper"
        shutil.copytree(self.first, tampered)
        descriptor_path = tampered / "gameplay/descriptor.json"
        descriptor = json.loads(descriptor_path.read_text(encoding="utf-8"))
        descriptor["hud"]["bitmap_assets"][0]["selector"] = 999
        descriptor_path.write_bytes(cache.canonical_json(descriptor))
        manifest_path = tampered / "manifest.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        for output in manifest["outputs"]:
            if output["path"] == "gameplay/descriptor.json":
                output["bytes"] = descriptor_path.stat().st_size
                output["sha256"] = cache.sha256_file(descriptor_path)
        manifest["cache_content_sha256"] = cache._level2_cache_identity(
            manifest["outputs"]
        )
        unsigned = dict(manifest)
        unsigned.pop("manifest_payload_sha256", None)
        manifest["manifest_payload_sha256"] = cache.sha256_bytes(
            cache.canonical_json(unsigned)
        )
        manifest_path.write_bytes(cache.canonical_json(manifest))
        with self.assertRaisesRegex(cache.CacheError, "bitmap rows changed"):
            cache.validate_day_gameplay_cache(tampered)

        missing = self.temp_root / "hud-output-missing"
        shutil.copytree(self.first, missing)
        victim = next((missing / "gameplay/hud").rglob("*.u8"))
        victim.unlink()
        with self.assertRaisesRegex(cache.CacheError, "output hash mismatch"):
            cache.validate_day_gameplay_cache(missing)

        descriptor_missing = self.temp_root / "hud-descriptor-missing"
        shutil.copytree(self.first, descriptor_missing)
        (descriptor_missing / "gameplay/descriptor.json").unlink()
        with self.assertRaisesRegex(cache.CacheError, "output hash mismatch"):
            cache.validate_day_gameplay_cache(descriptor_missing)

    def test_cross_level_and_unknown_closure_inputs_fail_closed(self) -> None:
        with self.assertRaisesRegex(cache.CacheError, "requires external=2"):
            cache.build_day_gameplay_cache(
                cache.DEFAULT_OWNER,
                cache.DEFAULT_PACKAGE,
                cache.DEFAULT_CLOSURE,
                cache.DEFAULT_CATALOG,
                self.temp_root / "wrong-profile",
            )
        closure_copy = self.temp_root / "closure.json"
        closure = json.loads(cache.DEFAULT_LEVEL2_CLOSURE.read_text(encoding="utf-8"))
        closure["sets"]["scene_reachable"]["assets"].append(
            {
                "asset_id": "owner_data:DATA/NOPE.DAT#1",
                "content_address": "sha256:" + "0" * 64,
                "kind": "catalog_entry",
            }
        )
        closure_copy.write_bytes(cache.canonical_json(closure))
        with self.assertRaisesRegex(cache.CacheError, "canonical self-hash mismatch"):
            cache.build_day_gameplay_cache(
                cache.DEFAULT_OWNER,
                cache.DEFAULT_LEVEL2_PACKAGE,
                closure_copy,
                cache.DEFAULT_CATALOG,
                self.temp_root / "unknown-selector",
            )

    def test_output_tamper_and_symlink_are_rejected(self) -> None:
        path = self.first / "gameplay/descriptor.json"
        original = path.read_bytes()
        path.write_bytes(original + b"tamper")
        with self.assertRaisesRegex(cache.CacheError, "hash mismatch"):
            cache.validate_day_gameplay_cache(self.first)
        path.write_bytes(original)
        link = self.first / "gameplay/symlink.bin"
        link.symlink_to(self.second / "gameplay/descriptor.json")
        with self.assertRaisesRegex(cache.CacheError, "symlink"):
            cache.validate_day_gameplay_cache(self.first)
        link.unlink()

    def test_default_validator_rejects_self_consistent_wrong_profile_binding(self) -> None:
        tampered = self.temp_root / "tampered-binding"
        shutil.copytree(self.first, tampered)
        path = tampered / "manifest.json"
        manifest = json.loads(path.read_text(encoding="utf-8"))
        manifest["profile"]["package_sha256"] = "0" * 64
        manifest["bindings"]["level_export_package_sha256"] = "0" * 64
        unsigned = dict(manifest)
        unsigned.pop("manifest_payload_sha256", None)
        manifest["manifest_payload_sha256"] = cache.sha256_bytes(
            cache.canonical_json(unsigned)
        )
        path.write_bytes(cache.canonical_json(manifest))
        with self.assertRaisesRegex(cache.CacheError, "package binding mismatch"):
            cache.validate_day_gameplay_cache(tampered)


class Level2DayAudioTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.temp_root = Path(
            tempfile.mkdtemp(prefix="level2-day-audio-test.", dir=cache.GENERATED)
        )
        cls.first = cls.temp_root / "first"
        cls.second = cls.temp_root / "second"
        for output in (cls.first, cls.second):
            cache.build_day_audio_cache(
                cache.DEFAULT_OWNER,
                cache.DEFAULT_LEVEL2_PACKAGE,
                cache.DEFAULT_LEVEL2_CLOSURE,
                cache.DEFAULT_CATALOG,
                output,
            )

    @classmethod
    def tearDownClass(cls) -> None:
        shutil.rmtree(cls.temp_root)

    def test_deterministic_banks_occurrences_and_day_music_selector(self) -> None:
        self.assertEqual(tree_identity(self.first), tree_identity(self.second))
        manifest = cache.validate_day_audio_cache(self.first)
        self.assertEqual(len(manifest["outputs"]), 27)
        descriptor = json.loads(
            (self.first / "audio/descriptor.json").read_text(encoding="utf-8")
        )
        self.assertEqual(len(descriptor["sound"]["occurrences"]), 26)
        self.assertEqual(len(descriptor["excluded"]), 93)
        self.assertEqual(
            [row["sound_id"] for row in descriptor["sound"]["records"]],
            [1, 2, 3, 4, 5, 8, 9, 10, 14, 15, 16, 18, 23, 29, 31, 40, 41, 46, 54, 55, 56, 57, 61],
        )
        self.assertEqual(
            [row["filename"] for row in descriptor["music"]["records"]][:3],
            ["C2GAME1.XMI", "C2GAME2.XMI", "C2GAME3.XMI"],
        )
        self.assertEqual(
            len({row["stable_occurrence_hash_sha256"] for row in descriptor["sound"]["occurrences"]}),
            26,
        )
        self.assertEqual(descriptor["music"]["maptype_music"], 2)
        self.assertEqual(descriptor["music"]["channel"], 0)
        self.assertEqual(descriptor["music"]["driver"], "f")

    def test_audio_output_and_binding_tamper_fail_closed(self) -> None:
        path = self.first / "audio/SOUND.DAT"
        original = path.read_bytes()
        path.write_bytes(original + b"tamper")
        with self.assertRaisesRegex(cache.CacheError, "output hash mismatch"):
            cache.validate_day_audio_cache(self.first)
        path.write_bytes(original)
        manifest_path = self.first / "manifest.json"
        manifest_original = manifest_path.read_bytes()
        manifest = json.loads(manifest_original)
        manifest["profile"]["package_sha256"] = "0" * 64
        manifest_path.write_bytes(cache.canonical_json(manifest))
        with self.assertRaisesRegex(cache.CacheError, "metadata hash mismatch"):
            cache.validate_day_audio_cache(self.first)
        manifest_path.write_bytes(manifest_original)
        descriptor_path = self.first / "audio/descriptor.json"
        descriptor_original = descriptor_path.read_bytes()
        descriptor = json.loads(descriptor_original)
        descriptor["sound"]["occurrences"][0]["sound_selector"]["sound_ids"] = [99]
        descriptor_path.write_bytes(cache.canonical_json(descriptor))
        manifest = json.loads(manifest_original)
        for row in manifest["outputs"]:
            if row["path"] == "audio/descriptor.json":
                row["bytes"] = descriptor_path.stat().st_size
                row["sha256"] = cache.sha256_file(descriptor_path)
        manifest["cache_content_sha256"] = cache._level2_cache_identity(manifest["outputs"])
        unsigned = dict(manifest)
        unsigned.pop("manifest_payload_sha256", None)
        manifest["manifest_payload_sha256"] = cache.sha256_bytes(cache.canonical_json(unsigned))
        manifest_path.write_bytes(cache.canonical_json(manifest))
        with self.assertRaisesRegex(cache.CacheError, "occurrence descriptor changed"):
            cache.validate_day_audio_cache(self.first)
        descriptor_path.write_bytes(descriptor_original)
        manifest_path.write_bytes(manifest_original)
        descriptor = json.loads(descriptor_original)
        descriptor["sound"]["records"][0]["filename"] = "EVIL.WAV"
        descriptor_path.write_bytes(cache.canonical_json(descriptor))
        manifest = json.loads(manifest_original)
        for row in manifest["outputs"]:
            if row["path"] == "audio/descriptor.json":
                row["bytes"] = descriptor_path.stat().st_size
                row["sha256"] = cache.sha256_file(descriptor_path)
        manifest["cache_content_sha256"] = cache._level2_cache_identity(manifest["outputs"])
        unsigned = dict(manifest)
        unsigned.pop("manifest_payload_sha256", None)
        manifest["manifest_payload_sha256"] = cache.sha256_bytes(cache.canonical_json(unsigned))
        manifest_path.write_bytes(cache.canonical_json(manifest))
        with self.assertRaisesRegex(cache.CacheError, "sound record descriptor changed"):
            cache.validate_day_audio_cache(self.first)
        descriptor_path.write_bytes(descriptor_original)
        manifest_path.write_bytes(manifest_original)
        wav_path = self.first / "audio/sfx/001.wav"
        wav_original = wav_path.read_bytes()
        wav_path.write_bytes(wav_original + b"tamper")
        manifest = json.loads(manifest_original)
        for row in manifest["outputs"]:
            if row["path"] == "audio/sfx/001.wav":
                row["bytes"] = wav_path.stat().st_size
                row["sha256"] = cache.sha256_file(wav_path)
        manifest["cache_content_sha256"] = cache._level2_cache_identity(manifest["outputs"])
        unsigned = dict(manifest)
        unsigned.pop("manifest_payload_sha256", None)
        manifest["manifest_payload_sha256"] = cache.sha256_bytes(cache.canonical_json(unsigned))
        manifest_path.write_bytes(cache.canonical_json(manifest))
        with self.assertRaisesRegex(cache.CacheError, "sound record asset binding mismatch"):
            cache.validate_day_audio_cache(self.first)
        wav_path.write_bytes(wav_original)
        manifest_path.write_bytes(manifest_original)
        descriptor = json.loads(descriptor_original)
        descriptor["music"]["lifecycle"] = ["StartMusic"]
        descriptor_path.write_bytes(cache.canonical_json(descriptor))
        manifest = json.loads(manifest_original)
        for row in manifest["outputs"]:
            if row["path"] == "audio/descriptor.json":
                row["bytes"] = descriptor_path.stat().st_size
                row["sha256"] = cache.sha256_file(descriptor_path)
        manifest["cache_content_sha256"] = cache._level2_cache_identity(manifest["outputs"])
        unsigned = dict(manifest)
        unsigned.pop("manifest_payload_sha256", None)
        manifest["manifest_payload_sha256"] = cache.sha256_bytes(cache.canonical_json(unsigned))
        manifest_path.write_bytes(cache.canonical_json(manifest))
        with self.assertRaisesRegex(cache.CacheError, "music lifecycle changed"):
            cache.validate_day_audio_cache(self.first)
        descriptor_path.write_bytes(descriptor_original)
        manifest_path.write_bytes(manifest_original)

    def test_audio_cross_level_input_fails_before_materialization(self) -> None:
        with self.assertRaisesRegex(cache.CacheError, "requires external=2"):
            cache.build_day_audio_cache(
                cache.DEFAULT_OWNER,
                cache.DEFAULT_PACKAGE,
                cache.DEFAULT_CLOSURE,
                cache.DEFAULT_CATALOG,
                self.temp_root / "wrong-profile",
            )


class Level2DayAggregateTests(unittest.TestCase):
    """Acceptance coverage for the combined private Level 2 owner cache."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.temp_root = Path(
            tempfile.mkdtemp(prefix="level2-day-aggregate-test.", dir=cache.GENERATED)
        )
        cls.first = cls.temp_root / "first"
        cls.second = cls.temp_root / "second"
        for output in (cls.first, cls.second):
            cache.build_day_aggregate_cache(
                cache.DEFAULT_OWNER,
                cache.DEFAULT_LEVEL2_PACKAGE,
                cache.DEFAULT_LEVEL2_CLOSURE,
                cache.DEFAULT_CATALOG,
                output,
            )

    @classmethod
    def tearDownClass(cls) -> None:
        shutil.rmtree(cls.temp_root)

    def test_deterministic_composition_and_level1_seal(self) -> None:
        self.assertEqual(tree_identity(self.first), tree_identity(self.second))
        manifest = cache.validate_day_aggregate_cache(self.first)
        self.assertEqual(set(manifest["components"]), {"material", "gameplay", "audio"})
        self.assertEqual(manifest["profile"]["external_level"], 2)
        self.assertEqual(manifest["profile"]["internal_level"], 1)
        self.assertEqual(manifest["profile"]["map_type"], {"id": 0, "name": "Day"})
        self.assertEqual(
            len(manifest["outputs"]),
            sum(row["output_count"] + 1 for row in manifest["components"].values()),
        )
        gameplay_hud_paths = [
            row["path"]
            for row in manifest["outputs"]
            if row["path"].startswith("components/gameplay/gameplay/hud/")
        ]
        self.assertEqual(len(gameplay_hud_paths), 75)
        gameplay_descriptor = json.loads(
            (
                self.first
                / "components/gameplay/gameplay/descriptor.json"
            ).read_text(encoding="utf-8")
        )
        self.assertEqual(
            gameplay_descriptor["hud"]["bitmap_selectors"],
            [1, 2, 40, 41, 42, 43, 54, 55, *range(123, 149)],
        )
        for row in manifest["outputs"]:
            if row["kind"] != "level2_child_manifest":
                self.assertTrue(
                    all(source.startswith("owner_data:") for source in row["source_asset_ids"])
                )
        level1 = cache.validate_cache(cache.DEFAULT_OUTPUT)
        self.assertEqual(
            level1["cache_content_sha256"],
            "b664ad00c94e3f9393bc92ecac91a821673222cd365ed47617c4fab63db733bf",
        )

    def test_missing_output_and_source_ledger_tamper_fail_closed(self) -> None:
        missing = self.temp_root / "missing"
        shutil.copytree(self.first, missing)
        victim = next(
            path
            for path in (missing / "components").rglob("*")
            if path.is_file() and path.name != "manifest.json"
        )
        victim.unlink()
        with self.assertRaisesRegex(cache.CacheError, "output missing"):
            cache.validate_day_aggregate_cache(missing)

        tampered = self.temp_root / "source-ledger-tamper"
        shutil.copytree(self.first, tampered)
        child_path = tampered / "components/material/manifest.json"
        child = json.loads(child_path.read_text(encoding="utf-8"))
        child["outputs"][0]["source_asset_ids"] = ["owner_data:FAKE.DAT"]
        unsigned_child = dict(child)
        unsigned_child.pop("manifest_payload_sha256", None)
        child["manifest_payload_sha256"] = cache.sha256_bytes(cache.canonical_json(unsigned_child))
        child_path.write_bytes(cache.canonical_json(child))
        aggregate_path = tampered / "manifest.json"
        aggregate = json.loads(aggregate_path.read_text(encoding="utf-8"))
        aggregate["components"]["material"]["manifest_sha256"] = cache.sha256_file(child_path)
        for row in aggregate["outputs"]:
            if row["path"] == "components/material/manifest.json":
                row["bytes"] = child_path.stat().st_size
                row["sha256"] = cache.sha256_file(child_path)
            if row["path"] == "components/material/" + child["outputs"][0]["path"]:
                row["source_asset_ids"] = ["owner_data:FAKE.DAT"]
        aggregate["cache_content_sha256"] = cache._level2_cache_identity(aggregate["outputs"])
        unsigned_aggregate = dict(aggregate)
        unsigned_aggregate.pop("manifest_payload_sha256", None)
        aggregate["manifest_payload_sha256"] = cache.sha256_bytes(
            cache.canonical_json(unsigned_aggregate)
        )
        aggregate_path.write_bytes(cache.canonical_json(aggregate))
        with self.assertRaisesRegex(cache.CacheError, "outside closure"):
            cache.validate_day_aggregate_cache(tampered)

        extra = self.temp_root / "extra-output"
        shutil.copytree(self.first, extra)
        extra_file = extra / "components/extra.bin"
        extra_file.write_bytes(b"unexpected")
        aggregate_path = extra / "manifest.json"
        aggregate = json.loads(aggregate_path.read_text(encoding="utf-8"))
        aggregate["outputs"].append(
            {
                "path": "components/extra.bin",
                "kind": "unexpected",
                "bytes": extra_file.stat().st_size,
                "sha256": cache.sha256_file(extra_file),
                "source_asset_ids": ["owner_data:DATA/PALD-0.DAT"],
                "component": "aggregate",
            }
        )
        aggregate["outputs"].sort(key=lambda row: row["path"])
        aggregate["cache_content_sha256"] = cache._level2_cache_identity(aggregate["outputs"])
        unsigned_aggregate = dict(aggregate)
        unsigned_aggregate.pop("manifest_payload_sha256", None)
        aggregate["manifest_payload_sha256"] = cache.sha256_bytes(
            cache.canonical_json(unsigned_aggregate)
        )
        aggregate_path.write_bytes(cache.canonical_json(aggregate))
        with self.assertRaisesRegex(cache.CacheError, "output set changed"):
            cache.validate_day_aggregate_cache(extra)

    def test_component_profile_tamper_cross_level_missing_and_privacy_fail_closed(self) -> None:
        tampered = self.temp_root / "component-profile-tamper"
        shutil.copytree(self.first, tampered)
        child_path = tampered / "components/gameplay/manifest.json"
        child = json.loads(child_path.read_text(encoding="utf-8"))
        child["profile"]["catalog_sha256"] = "0" * 64
        child["bindings"]["owner_catalog_sha256"] = "0" * 64
        unsigned_child = dict(child)
        unsigned_child.pop("manifest_payload_sha256", None)
        child["manifest_payload_sha256"] = cache.sha256_bytes(cache.canonical_json(unsigned_child))
        child_path.write_bytes(cache.canonical_json(child))
        aggregate_path = tampered / "manifest.json"
        aggregate = json.loads(aggregate_path.read_text(encoding="utf-8"))
        aggregate["components"]["gameplay"]["manifest_sha256"] = cache.sha256_file(child_path)
        for row in aggregate["outputs"]:
            if row["path"] == "components/gameplay/manifest.json":
                row["bytes"] = child_path.stat().st_size
                row["sha256"] = cache.sha256_file(child_path)
        aggregate["cache_content_sha256"] = cache._level2_cache_identity(aggregate["outputs"])
        unsigned_aggregate = dict(aggregate)
        unsigned_aggregate.pop("manifest_payload_sha256", None)
        aggregate["manifest_payload_sha256"] = cache.sha256_bytes(
            cache.canonical_json(unsigned_aggregate)
        )
        aggregate_path.write_bytes(cache.canonical_json(aggregate))
        with self.assertRaisesRegex(cache.CacheError, "descriptor identity mismatch"):
            cache.validate_day_aggregate_cache(tampered)

        binding_tampered = self.temp_root / "component-binding-key-tamper"
        shutil.copytree(self.first, binding_tampered)
        child_path = binding_tampered / "components/material/manifest.json"
        child = json.loads(child_path.read_text(encoding="utf-8"))
        child["bindings"]["unexpected"] = "tampered"
        unsigned_child = dict(child)
        unsigned_child.pop("manifest_payload_sha256", None)
        child["manifest_payload_sha256"] = cache.sha256_bytes(cache.canonical_json(unsigned_child))
        child_path.write_bytes(cache.canonical_json(child))
        aggregate_path = binding_tampered / "manifest.json"
        aggregate = json.loads(aggregate_path.read_text(encoding="utf-8"))
        aggregate["components"]["material"]["manifest_sha256"] = cache.sha256_file(child_path)
        for row in aggregate["outputs"]:
            if row["path"] == "components/material/manifest.json":
                row["bytes"] = child_path.stat().st_size
                row["sha256"] = cache.sha256_file(child_path)
        aggregate["cache_content_sha256"] = cache._level2_cache_identity(aggregate["outputs"])
        unsigned_aggregate = dict(aggregate)
        unsigned_aggregate.pop("manifest_payload_sha256", None)
        aggregate["manifest_payload_sha256"] = cache.sha256_bytes(
            cache.canonical_json(unsigned_aggregate)
        )
        aggregate_path.write_bytes(cache.canonical_json(aggregate))
        with self.assertRaisesRegex(cache.CacheError, "binding keys changed"):
            cache.validate_day_aggregate_cache(binding_tampered)

        with self.assertRaisesRegex(cache.CacheError, "requires external=2"):
            cache.build_day_aggregate_cache(
                cache.DEFAULT_OWNER,
                cache.DEFAULT_PACKAGE,
                cache.DEFAULT_CLOSURE,
                cache.DEFAULT_CATALOG,
                self.temp_root / "wrong-profile",
            )
        with self.assertRaisesRegex(cache.CacheError, "cannot read canonical JSON"):
            cache.build_day_aggregate_cache(
                cache.DEFAULT_OWNER,
                cache.DEFAULT_LEVEL2_PACKAGE,
                self.temp_root / "missing-closure.json",
                cache.DEFAULT_CATALOG,
                self.temp_root / "missing-input",
            )
        with self.assertRaisesRegex(cache.CacheError, "distributable path"):
            cache.build_day_aggregate_cache(
                cache.DEFAULT_OWNER,
                cache.DEFAULT_LEVEL2_PACKAGE,
                cache.DEFAULT_LEVEL2_CLOSURE,
                cache.DEFAULT_CATALOG,
                cache.UNITY / "Assets/Level2OwnerCache",
            )


if __name__ == "__main__":
    unittest.main()
