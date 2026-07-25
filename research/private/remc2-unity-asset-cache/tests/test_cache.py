from __future__ import annotations

import hashlib
import importlib.util
import json
import shutil
import tempfile
import unittest
from pathlib import Path

TOOL = Path(__file__).resolve().parents[1] / "build_cache.py"
SPEC = importlib.util.spec_from_file_location("asset_cache", TOOL)
assert SPEC and SPEC.loader
cache = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(cache)


def tree_identity(root: Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(item for item in root.rglob("*") if item.is_file()):
        digest.update(path.relative_to(root).as_posix().encode("utf-8"))
        digest.update(b"\0")
        digest.update(path.read_bytes())
    return digest.hexdigest()


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
        sprites = manifest["assets"]["msprn"]
        self.assertEqual(
            [row["index"] for row in sprites],
            sorted(cache.FRAME_MSPRN + cache.DYNAMIC_MSPRN),
        )
        self.assertEqual(
            sum(row["inclusion"] == "qualified_frame_used" for row in sprites), 11
        )
        self.assertEqual(manifest["assets"]["font1"]["glyph_count"], 273)
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
            manifest["assets"]["font1"]["palette_status"],
            "mask_exact_runtime_tint_callsite_dynamic; Level1 objective CLRN3840 qualified",
        )
        self.assertEqual(
            manifest["assets"]["spell_icon_indices"]["status"],
            "family_mapping_with_Level1_reset_big_icons_materialized",
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
                "resolution_bank": "MSPRN0-0",
                "source_scale": 2,
                "bank_status":
                    "trace_qualified_Night_low_resolution_bank_scaled_2x",
                "high_resolution_bank_status":
                    "Deferred; no HSPRN parity claim in this cache",
            },
        )
        bitmap_assets = hud["bitmap_assets"]
        self.assertEqual(
            [row["selector"] for row in bitmap_assets],
            [1, 40, 41, 42, 43, 54, 123, 124, 171, 172, 173],
        )
        self.assertEqual(len({row["selector"] for row in bitmap_assets}), 11)
        self.assertTrue(
            all(
                row["bank"] == "MSPRN0-0"
                and row["source_scale"] == 2
                and row["runtime_width"] == row["stored_width"] * 2
                and row["runtime_height"] == row["stored_height"] * 2
                and row["authority"] == "FrozenObserved"
                and len(row["indices_sha256"]) == 64
                and len(row["mask_sha256"]) == 64
                for row in bitmap_assets
            )
        )
        self.assertEqual(
            {
                row["selector"]: (row["runtime_width"], row["runtime_height"])
                for row in bitmap_assets
            },
            {
                1: (64, 44),
                40: (124, 38),
                41: (128, 44),
                42: (16, 44),
                43: (34, 44),
                54: (128, 44),
                123: (62, 34),
                124: (62, 34),
                171: (16, 12),
                172: (16, 14),
                173: (32, 10),
            },
        )
        self.assertEqual(
            [row["selector"] for row in hud["top_bar"]["panels"]],
            [40, 54, 54, 41, 43, 42],
        )
        self.assertEqual(
            [row["authority"] for row in hud["top_bar"]["panels"]],
            ["FrozenObserved"] * 6,
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
                (153, 0, 123, "FrozenObserved", "SpellActivationLive"),
                (154, 1, 124, "FrozenObserved", "Deferred"),
            ],
        )
        objective = hud["objective"]
        self.assertEqual(objective["selector"], 48)
        self.assertEqual(objective["authority"], "FrozenObserved")
        self.assertEqual(objective["objective_textbox_ticks"], 33)
        self.assertEqual(objective["frame_selectors"], [171, 172, 173])
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
        self.assertEqual(
            hashlib.sha256(
                (self.first / objective["render_path"]).read_bytes()
            ).hexdigest(),
            "d1ec5a6eb550aec63cb491a3d072e4904c2c8f7157591f35e74ceebefc820cf9",
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
            (128, 128, 64, 64, 2, 512),
        )
        self.assertEqual(
            minimap["low_resolution_branch"],
            "MSPRN_effective_64x64_scaling512_then_nearest_2x",
        )
        aperture = minimap["aperture"]
        aperture_mask = (self.first / aperture["path"]).read_bytes()
        self.assertEqual(len(aperture_mask), 64 * 64)
        self.assertEqual(sum(aperture_mask), 3204)
        self.assertEqual(
            hashlib.sha256(aperture_mask).hexdigest(),
            "98a3a7c0300b49204629aab1b4b552253bea2223f9e68c40d1458b2e62243285",
        )
        self.assertEqual(aperture["opaque_pixels"], 3204)
        self.assertEqual(aperture["table_offset"], 0x14300)
        self.assertEqual(aperture["phase_step"], 0x80000)
        self.assertEqual(
            aperture["table_curve_sha256"],
            "e155e5eaa2019054c52878dad895d0ad70c6b92698fab72ae518eeccea4eafc7",
        )
        scanlines = aperture["scanlines"]
        self.assertEqual(len(scanlines), 64)
        self.assertEqual(
            [
                (
                    scanlines[index]["start_x_inclusive"],
                    scanlines[index]["end_x_exclusive"],
                )
                for index in (0, 1, 15, 31, 32, 48, 62, 63)
            ],
            [
                (25, 39), (21, 43), (5, 59), (1, 63),
                (1, 63), (5, 59), (21, 43), (25, 39),
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
            ("FrozenObserved", "FrozenObserved", "PlayerFlightLive"),
        )
        self.assertEqual(
            minimap["entity_overlay_mode"],
            "non_original_FrozenObserved_reset_reference_overlay",
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
            sound["materialized_sound_ids"], [1, 2, 5, 29, 31, 37, 39, 43, 46]
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
                5: "local_ambient",
                12: "explicit_null",
                29: "local_feedback",
                31: "local_ambient",
                37: "original_stereo_positional",
                39: "original_stereo_positional",
                43: "original_stereo_positional",
                46: "typed_goat_positional_request_pitch_deferred",
            },
        )
        expected_audio = {
            1: ("OCEAN.WAV", 151344, 151296,
                "d41a44e541f882e620a57316e020d40369372166f84ccbc07525848a395ec8dc"),
            2: ("CRICKET2.WAV", 135984, 135937,
                "f4636891ecc2736deba4aab46e7e5117d1b89022ea20fb93315ee0ab46de8d34"),
            5: ("FIRE.WAV", 44080, 44032,
                "008f25258a035546949ccca475ca954feb5f74cdfe32c76e6f612bf18575bf3a"),
            12: ("NULL.WAV", 64, 16,
                 "3a1a42d64f3295fa6bfa7cd321ebe66db702ec398fc958c16c5f9fe46bc325c4"),
            29: ("CANTUSE.WAV", 6976, 6925,
                 "4378349675288905df83dc81add0a1735f3fa44eabeb2d2000b9bfd459bc989d"),
            31: ("MARKET.WAV", 193072, 193024,
                 "d9c4a1e8e8decc67bdae1426b83b777902d68a2bdfc20e591b507722251ef839"),
            37: ("CYMERIAN.WAV", 39488, 39273,
                 "c3e81213dd5d3733a83d5f83b576feefe3e325c90087ab1b552c895bd1170b2b"),
            39: ("WYVERN.WAV", 74160, 74112,
                 "8f2eb4217c7f33afb922eda4be61194b94484a5afada83b969461027a9d8ad41"),
            43: ("FIREFLY1.WAV", 30064, 29830,
                 "1ec43d872d6dd9e07a8d625d70a85ce39488de6914d4062aad4d89a6d19c5604"),
            46: ("GOAT.WAV", 20768, 20545,
                 "6038ceffaa2c54f9d4f7a93494fc713bc6bc3b76bc9a8ecc7dfc7f3fe4445b18"),
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
            ["PrepareSound", "EndLoop", "UpdateMusic", "GoatSoundRequest"],
        )
        self.assertEqual(
            event_semantics["request_spell_activation_audio_status"],
            "NoMapping; must not infer Fireball audio",
        )
        self.assertEqual(event_semantics["cave_ids"]["exact_range"], [65, 69])
        self.assertEqual(
            event_semantics["positional_surfaces"]["command_required_sound_ids"],
            [37, 39, 43],
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
                    "Deferred_until_source_ordered_global_rng_is_qualified",
                "presentation_policy":
                    "Do_not_play_as_exact_without_numeric_pitch",
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
        self.assertEqual(materials["tmap_scene_reachable_selector_count"], 129)
        self.assertEqual(materials["tmap_decoded_texture_count"], 118)
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
        self.assertEqual(len(descriptor["tmaps1"]["scene_reachable_selectors"]), 129)
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

    def test_manifest_tamper_is_rejected(self) -> None:
        path = self.first / "manifest.json"
        manifest = json.loads(path.read_text())
        manifest["outputs"][0]["sha256"] = "0" * 64
        original = path.read_bytes()
        path.write_bytes(cache.canonical_json(manifest))
        with self.assertRaisesRegex(cache.CacheError, "hash mismatch"):
            cache.validate_cache(self.first)
        path.write_bytes(original)

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


if __name__ == "__main__":
    unittest.main()
