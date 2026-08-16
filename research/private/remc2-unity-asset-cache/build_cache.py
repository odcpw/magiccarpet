#!/usr/bin/env python3
"""Build and validate the private, owner-local Level 1 Unity asset cache."""

from __future__ import annotations

import argparse
import csv
import hashlib
import importlib.util
import json
import os
import re
import shutil
import struct
import subprocess
import tarfile
import tempfile
import wave
from io import BytesIO
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parents[3]
PRIVATE = REPO / "research/private"
GENERATED = PRIVATE / "generated"
UNITY = PRIVATE / "remc2-unity-native"
DEFAULT_OWNER = PRIVATE / "remc2-native-runtime/CD_Files"
DEFAULT_PACKAGE = GENERATED / "level1-export-m"
DEFAULT_CLOSURE = GENERATED / "level1-asset-closure.json"
DEFAULT_CATALOG = PRIVATE / "remc2-atlas/semantics/generated/owner-asset-catalog.json"
DEFAULT_OUTPUT = GENERATED / "level1-unity-asset-cache-plan29-final"
DEFAULT_LEVEL2_PACKAGE = GENERATED / "l2-debug3.fJ3PaM/level2"
DEFAULT_LEVEL2_CLOSURE = GENERATED / "level2-asset-closure.json"
DEFAULT_LEVEL2_OUTPUT = GENERATED / "level2-day-material-cache"
DEFAULT_LEVEL2_GAMEPLAY_OUTPUT = GENERATED / "level2-day-gameplay-cache"
DEFAULT_CONFORMANCE_CONTRACT = (
    PRIVATE / "remc2-sim-rs/level1-conformance/plan29-closure-contract.json"
)
CANONICAL_LEVEL1_MATRIX = (
    PRIVATE / "remc2-sim-rs/level1-conformance/level1-conformance-matrix.json"
)
PROVEN_DECODER = PRIVATE / "remc2-atlas/build_owner_asset_catalog.py"
PROVEN_CLOSURE = PRIVATE / "remc2-level-asset-closure/build_closure.py"
SPELL_TABLE = PRIVATE / "remc2-atlas/semantics/generated/spell-tiers.csv"
SPELL_GENERATOR = PRIVATE / "remc2-atlas/build_semantic_backbone.py"
RNC_HELPER = Path(__file__).with_name("rnc_decode.cpp")
XMI_HELPER = Path(__file__).with_name("xmi_convert.cpp")
ADL_RENDER_HELPER = Path(__file__).with_name("adl_render.cpp")
ADLMIDI_SOURCE_AT_PIN = "libADLMIDI-master"
EXPECTED_ADLMIDI_ARCHIVE_SHA256 = (
    "566adad0a67ba84bae7b415582e9be2dce355a8a54e62365d6bf2c2bbd46a300"
)
EXPECTED_RNC_SOURCE_ARCHIVE_SHA256 = (
    "97e0a5ced2f608c57dab77c45e662756f6746085235b44196f6dc81e2a0d7e50"
)
REMC2_GIT = REPO / "upstream/magic-carpet-2-hd"
REMC2_EVENTS_FUNCTIONS_SOURCE = "remc2/engine/EventsFunctions.cpp"
REMC2_EVENTS_SOURCE = "remc2/engine/Events.cpp"
REMC2_UV_TABLE_SOURCE = "remc2/engine/UVTable_D4350.cpp"
REMC2_MATHS_TABLE_SOURCE = "remc2/utilities/Maths.cpp"
RNC_DECODER_CPP_SOURCE = "remc2/utilities/DataFileRNC.cpp"
RNC_DECODER_HEADER_SOURCE = "remc2/utilities/DataFileRNC.h"
XMI_IMPLEMENTATION_SOURCE = "remc2/portability/xmi2mid.cpp"
XMI_HEADER_SOURCE = "remc2/portability/xmi2mid.h"
EXPORTER_SOURCE = PRIVATE / "remc2-level-exporter/src/exporter_main.cpp"
GOAT_TMAP_GOLDEN = (
    PRIVATE / "remc2-frame-trace/goat_tmap_golden.json"
)

SCHEMA = "mc2-owner-unity-asset-cache/v1"
LEVEL2_MATERIAL_SCHEMA = "mc2-level2-day-material-cache/v1"
LEVEL2_GAMEPLAY_SCHEMA = "mc2-level2-day-gameplay-cache/v1"
LEVEL2_DAY_HUD_SCHEMA = "mc2-level2-day-hud/v1"
LEVEL2_AUDIO_SCHEMA = "mc2-level2-day-audio-cache/v1"
LEVEL2_AGGREGATE_SCHEMA = "mc2-level2-day-owner-cache/v1"
LEVEL2_DAY_HUD_BITMAP_SELECTORS = (
    1,
    2,
    40,
    41,
    42,
    43,
    54,
    55,
    *range(123, 149),
)
LEVEL2_DAY_HUD_ROMAN_TEXT = ("I", "II", "III", "IV", "V")
LEVEL2_DAY_HUD_BITMAP_NAMES = {
    1: "SPELL_TOPTILE_BAR",
    2: "SPELL_TOPTILE_BAR_GLOW",
    40: "MINI_MAP_PANEL",
    41: "HEALTH_PANEL",
    42: "HEALTH_MANA_ICONS",
    43: "PLAYER_ICON",
    54: "PANEL",
    55: "HEALTH_PANEL_HIT",
}
EXPECTED_LEVEL2_SOUND_RECORDS_SHA256 = "23c6a8fbb3ff907e72ce127bbe3163ee6a4955e0319371268a7e8d65b1422555"
EXPECTED_LEVEL2_MUSIC_RECORDS_SHA256 = "1cb48c36ee4f3d8ec12ca8f3572ad9ec2b171026aa09a2039dfbd80309c0dcb5"
LEVEL2_AUDIO_DENOMINATOR = (
    PRIVATE
    / "remc2-sim-rs/level2-conformance/denominator/level2-dispatch-rng-audio-presentation.json"
)
DEFAULT_LEVEL2_AUDIO_OUTPUT = GENERATED / "level2-day-audio-cache"
DEFAULT_LEVEL2_AGGREGATE_OUTPUT = GENERATED / "level2-day-owner-cache"
CONFORMANCE_SEAL_SCHEMA = "mc2-plan29-cache-conformance-identity/v1"
GENERATOR_VERSION = "1.15.0"
PINNED_REMC2_COMMIT = "6806c7204d17220ecc0430759f784aea8ee2ddb2"
EXPECTED_LEVEL2_PACKAGE = "dc8f973ee9a2f53863a2e2e3743b6fa4b9ef947d3ce0b27b79d3ba86de318b80"
EXPECTED_LEVEL2_CLOSURE = "bee3b7a4fd585d6631523b87217978cbfa58ed08cd494fa7921486053de1dccc"
EXPECTED_LEVEL2_CATALOG = "ea9a735c598f2cbed81541366e1fdf8a5fcbc8cef7aecd086bb8a6a9e7c2429e"
EXPECTED_LEVEL2_CATALOG_FILE = "a25488be0933e5b4499665d37440a9860c8614649d9f695080da511c11d75b44"
LEVEL2_OWNER_PACKAGE_IDENTITY = {
    "manifest_schema": "mc2-owner-level-export/v5",
    "players_component": {
        "magic": "MCP1",
        "version": 4,
        "record_bytes": 663,
        "count": 8,
    },
}
PINNED_REMC2_SOURCE_SPANS = (
    (
        "remc2/engine/GameRenderOriginal.cpp",
        127,
        238,
        "world postprocess mode selection, blur and smoothing",
    ),
    (
        "remc2/engine/EventsFunctions.cpp",
        21496,
        22059,
        "DrawGameFrame fixed-profile writer tree",
    ),
    (
        "remc2/engine/EventsFunctions.cpp",
        22063,
        22195,
        "pause, level-end and live notification text",
    ),
    (
        "remc2/engine/EventsFunctions.cpp",
        22237,
        22360,
        "map-score tiles, names, scores, lines and help state",
    ),
    (
        "remc2/engine/EventsFunctions.cpp",
        31816,
        31869,
        "present-boundary update and MAIN writer order",
    ),
    (
        "remc2/engine/EventsFunctions.cpp",
        42930,
        42940,
        "reset setting-byte initialization",
    ),
    (
        "remc2/engine/EventsFunctions.cpp",
        43160,
        43175,
        "reset show-help state",
    ),
    (
        "remc2/engine/EventsFunctions.cpp",
        32113,
        32150,
        "reset title loader and CopyScreen_85B20 MAIN copy",
    ),
    (
        "remc2/engine/EventsFunctions.cpp",
        44066,
        44283,
        "sub_6DCA0 class-9 constructor and xclass/xmodel rewrites",
    ),
    (
        "remc2/engine/EventsFunctions.cpp",
        22445,
        22468,
        "two-by-thirteen spell-family menu domain",
    ),
    (
        "remc2/engine/EventsFunctions.cpp",
        22615,
        22628,
        "three-tier spell selector formula",
    ),
    (
        "remc2/engine/EventsFunctions.cpp",
        47196,
        47200,
        "fixed high-speed-system reset",
    ),
    (
        "remc2/engine/GameUI.cpp",
        350,
        380,
        "top-status spell-family selector formula",
    ),
    (
        "remc2/engine/GameUI.cpp",
        4597,
        4632,
        "OK/cancel strings, bitmaps and cursor",
    ),
    (
        "remc2/engine/GameUI.cpp",
        4702,
        4888,
        "chat, options and pause menu writers",
    ),
    (
        "remc2/engine/GameUI.cpp",
        4983,
        5146,
        "pause recursive help and options-menu writers",
    ),
    (
        "remc2/engine/GameUI.cpp",
        5151,
        5265,
        "volume bars and recursive language help writer",
    ),
    (
        "remc2/engine/Events.cpp",
        5186,
        5190,
        "class/subtype constructor-table dispatch",
    ),
    (
        "remc2/engine/LevelInit.cpp",
        18,
        51,
        "Night palette, cursor, font and display reset",
    ),
    (
        "remc2/engine/GameBitmapIndexes.h",
        1,
        61,
        "mechanical UI selector constants",
    ),
    (
        "remc2/engine/engine_support.cpp",
        90,
        105,
        "static-storage runtime control state",
    ),
)
PINNED_REMC2_BLOB_SHA256 = {
    "remc2/engine/GameRenderOriginal.cpp":
        "d15c1cc0042adbe103b2bcc664aed468914d82c8bb6cf0ea99485c4e04de987f",
    "remc2/engine/EventsFunctions.cpp":
        "3b01de678aff9f8662ef22a4790d5a47cb152de25592c3eeb42183e4783c503f",
    "remc2/engine/Events.cpp":
        "7ccdc4ad338e7f3e4287ceefc0459fba85dc4f48808bfe5139c0386548a052c3",
    "remc2/engine/GameUI.cpp":
        "92b5f63db493277dcdd717c4a94213aafd8a17db395e21d007829cc4eea4650f",
    "remc2/engine/UVTable_D4350.cpp":
        "74f60d14366223a5f3bacb09ea5be7d5ba46e9c3dd56dc06aa9e9e3c6356295c",
    "remc2/utilities/Maths.cpp":
        "8bed55e5c132a2485c3258d75cb7714def2c0bd9cf17839b1bfa4b0c659d52c0",
    "remc2/utilities/DataFileRNC.cpp":
        "180548e097934ce6e5af6ce54b31a2725951e4443c0b852d71297c481bd54600",
    "remc2/utilities/DataFileRNC.h":
        "cbbc4eb5d6da52c21f28f4922f435daa551c50242deee050a0453027d1ec453f",
    "remc2/portability/xmi2mid.cpp":
        "ffcd382d094202410be9b06119c807781d6217592369bfaec2c90564e1e5f9b5",
    "remc2/portability/xmi2mid.h":
        "437612b1c32b2ea5fe9dc43f6447fed2d86a956c4312461829f93490f81ec363",
    "remc2/engine/LevelInit.cpp":
        "8fcb4a136901adf998440d933b8cc8b3c43d78b2ab19b7cc0c1051a2dc533ca8",
    "remc2/engine/GameBitmapIndexes.h":
        "269c1b921ac5d68c2521335bee2b393f177d7bece7236d400e0caa8329bf5b05",
    "remc2/engine/engine_support.cpp":
        "a5d1812f796c690cd0fc7ea7f46cdb64bd9b73d999c632b04f9ea1e2806f4386",
}
EXPECTED_PACKAGE = "730d2fa084265df839f3f2b55ed766ac167a8b152ca4631b5ded9d0bb9104ac8"
EXPECTED_CLOSURE = "878123908f6de05f684f4cd1d73cff8d2ff08701cb6fdd12b7165bfcacc75719"
EXPECTED_RUNTIME_SHA256 = (
    "b078dbed443e2cc3bf3bfd08a0f519492546d9f939db74a3d9c185d58f9c559f"
)
LEVEL1_MATRIX_SCHEMA_ID = "mc2-level1-conformance-matrix"
LEVEL1_MATRIX_SCHEMA_VERSION = 2
CONFORMANCE_CONTRACT_SCHEMA = "mc2-plan29-closure-contract/v1"
CONFORMANCE_CONTRACT_SPECIFICATION_FIELDS = (
    "remc2_commit",
    "matrix_path",
    "row_count",
    "dispatch_slot_count",
    "authored_index_count",
)
CONFORMANCE_CONTRACT_IDENTITY_FIELDS = (
    "matrix_sha256",
    "row_id_sha256",
    "source_span_sha256",
    "ordered_behavior_sha256",
    "reachable_row_id_sha256",
    "authored_census_sha256",
    "dispatch_census_sha256",
    "castle_profile_sha256",
    "required_observation_mask_sha256",
)
CONFORMANCE_CONTRACT_REACHABLE_FIELDS = (
    "row_ids",
    "required_observation_families",
    "required_observation_mask_sha256",
)
CONFORMANCE_CONTRACT_RECEIPT_FIELDS = (
    "rust",
    "c_abi",
    "unity",
    "cross_layer",
    "victory_failure_restart_replay",
    "privacy",
    "provenance",
    "clean_run_count",
)
CASTLE_DEFENSE_SHOT_ROWS = (
    {
        "spell_family": 0, "subspell": 1, "spell_life": 1,
        "spell_subspell_index": 160,
        "entity_class": 9, "entity_model": 0, "entity_action": 0,
        "xclass": 10, "xmodel": 0,
        "source_life_ticks": 21, "source_variant_byte": 0,
        "source_subspell_index": 100,
        "entity_parameter_index": 64, "particle_parameter_index": 340,
        "base_tmap": 463, "render_mode": 21, "logical_height": 100,
        "movement_speed": 384, "sound_id": 9,
    },
    {
        "spell_family": 7, "subspell": 0, "spell_life": 0,
        "spell_subspell_index": 200,
        "entity_class": 9, "entity_model": 9, "entity_action": 9,
        "xclass": 10, "xmodel": 23,
        "source_life_ticks": 9, "source_variant_byte": 0,
        "source_subspell_index": 200,
        "entity_parameter_index": 63, "particle_parameter_index": 216,
        "base_tmap": 151, "render_mode": 21, "logical_height": 50,
        "movement_speed": 384, "sound_id": 23,
    },
    {
        "spell_family": 7, "subspell": 1, "spell_life": 1,
        "spell_subspell_index": 250,
        "entity_class": 9, "entity_model": 12, "entity_action": 12,
        "xclass": 9, "xmodel": 9,
        "source_life_ticks": 5, "source_variant_byte": 0,
        "source_subspell_index": 250,
        "entity_parameter_index": 60, "particle_parameter_index": 216,
        "base_tmap": 151, "render_mode": 21, "logical_height": 50,
        "movement_speed": 384, "sound_id": 9,
    },
    {
        "spell_family": 0, "subspell": 2, "spell_life": 2,
        "spell_subspell_index": 80,
        "entity_class": 9, "entity_model": 28, "entity_action": 29,
        "xclass": 10, "xmodel": 76,
        "source_life_ticks": 21, "source_variant_byte": 0,
        "source_subspell_index": 100,
        "entity_parameter_index": 64, "particle_parameter_index": 340,
        "base_tmap": 463, "render_mode": 21, "logical_height": 100,
        "movement_speed": 384, "sound_id": 9,
    },
    {
        "spell_family": 9, "subspell": 0, "spell_life": 2,
        "spell_subspell_index": 1_750,
        "entity_class": 9, "entity_model": 3, "entity_action": 3,
        "xclass": 10, "xmodel": 17,
        "source_life_ticks": 21, "source_variant_byte": 2,
        "source_subspell_index": 1_750,
        "entity_parameter_index": 60, "particle_parameter_index": 76,
        "base_tmap": 64, "render_mode": 21, "logical_height": 350,
        "movement_speed": 384, "sound_id": 15,
    },
)
QUALIFIED_RUNTIME_CAPTURES = (
    "level1-export-i",
    "level1-export-j",
    "level1-export-k",
    "level1-export-l",
    "level1-export-m",
    "level1-export-n",
    "plan24-o-final3-20260730",
    "plan24-p-final3-20260730",
)
QUALIFIED_SETTINGS_BYTES = bytes(
    (1, 1, 1, 40, 1, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0)
)
EXPECTED_GOAT_TMAP_GOLDEN = (
    "a5c3d430ba22c3503453124d76119ed7b93ff6ea8445e23cd616faa1015b5f64"
)
EXPECTED_GOAT_TMAP_ORACLE_MANIFEST = (
    "cad16d830ece1f6422ec4f3f6a0a5ab4954fe6c20344e2e03c6ce1f0bc41e73b"
)
GOAT_TMAP_SELECTORS = frozenset({152, 153, 154})
RESET_FRAME_HSPRN = (
    1, 40, 41, 42, 43, 54, 123, 124, 171, 172, 173,
)
RESET_FRAME_MSPRN = (1, 40, 41, 42, 43, 54, 123, 124, 171, 172, 173)
DYNAMIC_MSPRN = (9,)
SPELL_FAMILY_DOMAIN = tuple(range(26))
SPELL_TIER_DOMAIN = tuple(range(3))
SPELL_SMALL_HSPRN = tuple(97 + family for family in SPELL_FAMILY_DOMAIN)
SPELL_BIG_HSPRN = tuple(123 + family for family in SPELL_FAMILY_DOMAIN)
SPELL_TIER_HSPRN = tuple(
    179 + 3 * family + tier
    for family in SPELL_FAMILY_DOMAIN
    for tier in SPELL_TIER_DOMAIN
)
if (
    SPELL_SMALL_HSPRN != tuple(range(97, 123))
    or SPELL_BIG_HSPRN != tuple(range(123, 149))
    or SPELL_TIER_HSPRN != tuple(range(179, 257))
):
    raise RuntimeError("mechanical 26-family/three-tier spell selector domains drifted")
FRAME_HSPRN = RESET_FRAME_HSPRN + (
    2,
    44, 45, 46, 47, 48, 49, 50,
    51, 52, 53, 55,
    58, 66, 83, 84,
    *range(74, 92),
    *SPELL_SMALL_HSPRN,
    *SPELL_BIG_HSPRN,
    *range(149, 179),
    *SPELL_TIER_HSPRN,
    *range(257, 262),
)
SOURCE_FRAME_MSPRN = tuple(sorted(set(FRAME_HSPRN + DYNAMIC_MSPRN)))
DYNAMIC_HSPRN = (9,)
FRAME_TMAPS = (40, 41, 54, 63)
LEVEL1_TERRAIN_SELECTORS = (
    0, 1, 2, 4, 5, 9, 22, 23, 24, 25, 26, 27, 36, 37, 38, 39, 44, 45,
    46, 47, 52, 53, 54, 55, 56, 57, 58, 59, 64, 65, 66, 67, 69, 70, 71,
    72, 73, 74, 75, 76, 84, 85, 86, 87, 88, 89, 97, 99, 101, 114, 115,
    116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 128, 131, 136, 139,
)
UNITY_TERRAIN_UV_GROUPS = (
    ((0, 1), (1, 1), (0, 0), (1, 0)),
    ((1, 1), (0, 1), (1, 0), (0, 0)),
    ((0, 0), (1, 0), (0, 1), (1, 1)),
    ((1, 0), (0, 0), (1, 1), (0, 1)),
    ((0, 1), (0, 0), (1, 1), (1, 0)),
    ((0, 0), (0, 1), (1, 0), (1, 1)),
    ((1, 1), (1, 0), (0, 1), (0, 0)),
    ((1, 0), (1, 1), (0, 0), (0, 1)),
)
SOURCE_PATHS = (
    "DATA/BL32N0-0.DAT",
    "DATA/BLDGPRM.DAT",
    "DATA/BUILD0-0.DAT",
    "DATA/BUILD0-0.TAB",
    "DATA/CLRN-0.DAT",
    "DATA/HFONT3.DAT",
    "DATA/HFONT3.TAB",
    "DATA/HSPRN0-0.DAT",
    "DATA/HSPRN0-0.TAB",
    "DATA/MSPRN0-0.DAT",
    "DATA/MSPRN0-0.TAB",
    "DATA/PALN-0.DAT",
    "DATA/POINTERS.DAT",
    "DATA/POINTERS.TAB",
    "DATA/SKYN0-0.DAT",
    "DATA/SMATITLE.DAT",
    "DATA/SMATITLE.PAL",
    "DATA/SPELLS.DAT",
    "DATA/FONT1.DAT",
    "DATA/FONT1.TAB",
    "DATA/TMAPS1-0.DAT",
    "DATA/TMAPS1-0.TAB",
    "DATA/TABLESN.DAT",
    "LANGUAGE/D2.TXT",
    "LANGUAGE/L1.TXT",
    "SOUND/MUSIC.DAT",
    "SOUND/SOUND.DAT",
)
TAB_RECORD = struct.Struct("<IBB")
BUILD_PARAMETER_RECORD = struct.Struct("<HBB")
TMAP_TAB_RECORD = struct.Struct("<iiH")
TMAP_HEADER = struct.Struct("<HHH")
TMAP_ANIMATION_HEADER = struct.Struct("<HI")
FLI_CHUNK_HEADER = struct.Struct("<IH")
ENTITY_HEADER = struct.Struct("<4sHHI")
PRESENTATION_HEADER = struct.Struct("<4sHHI")
PARTICLE_RECORD = struct.Struct("<HHHHH BbBB")
AUTHORED_FEATURE_OFFSET = 1_091
AUTHORED_FEATURE_BYTES = 20
AUTHORED_FEATURE_COUNT = 1_200
ENTITY_ACTOR_SPECS = {
    (2, 2, 6, 59, 39): ("dolmen", 79, 0, 400),
    (5, 1, 15, 98, 238): ("goat", 152, 20, 150),
    (5, 13, 111, 100, 239): ("villager-a", 155, 20, 200),
    (5, 13, 111, 100, 241): ("villager-b", 180, 20, 250),
    (5, 13, 111, 100, 242): ("villager-c", 183, 20, 250),
    (5, 13, 111, 100, 271): ("villager-d", 199, 20, 300),
}
ENTITY_EFFECT_SPECS = {
    (10, 0, 0, 59, 7): ("fireball-impact", 38, 36, 250),
    (10, 1, 1, 59, 41): ("explosion-death", 54, 21, 250),
    (10, 14, 14, 59, 9): ("smoke-particle", 63, 21, 50),
}
RESET_SOURCE_TUPLE_DISPOSITIONS = {
    (10, 45, 52, 59, 177, 0, 13): ("Render", "authored-house"),
    (11, 1, 1, 59, 0, 0, 1): ("Excluded", "SpriteLessSwitch"),
    (11, 0, 0, 59, 0, 0, 1): ("Excluded", "SpriteLessSwitch"),
    (5, 1, 15, 98, 238, 0, 12): ("Render", "goat"),
    (5, 13, 111, 100, 271, 0, 12): ("Render", "villager-d"),
    (5, 13, 111, 100, 241, 0, 12): ("Render", "villager-b"),
    (5, 13, 111, 100, 239, 0, 12): ("Render", "villager-a"),
    (5, 13, 111, 100, 242, 0, 12): ("Render", "villager-c"),
    (11, 4, 4, 59, 0, 0, 1): ("Excluded", "SpriteLessSwitch"),
    (10, 60, 65, 59, 0, 0, 1): ("Render", "ambient-emitter"),
    (10, 1, 1, 59, 41, 0, 655366): ("Render", "death-burst"),
    (2, 2, 6, 59, 39, 0, 4): ("Render", "dolmen"),
    (3, 0, 0, 66, 44, 0, 13): ("Render", "local-player"),
    (15, 0, 0, 59, 77, 0, 5): ("Render", "left-spell-holder"),
    (15, 1, 3, 59, 77, 0, 5): ("Render", "right-spell-holder"),
    (10, 14, 14, 59, 9, 0, 131076): ("Render", "smoke-particle"),
    (10, 0, 0, 59, 7, 1, 655494): ("Render", "fireball-impact"),
}
RESET_SOURCE_PRESENTATION_PROFILES = {
    (10, 45, 52, 59, 177, 0, 13): (177, 96, 0, 400, None),
    (11, 1, 1, 59, 0, 0, 1): (None, None, None, None, "SpriteLessSwitch"),
    (11, 0, 0, 59, 0, 0, 1): (None, None, None, None, "SpriteLessSwitch"),
    (5, 1, 15, 98, 238, 0, 12): (238, 152, 20, 150, None),
    (5, 13, 111, 100, 271, 0, 12): (271, 199, 20, 300, None),
    (5, 13, 111, 100, 241, 0, 12): (241, 180, 20, 250, None),
    (5, 13, 111, 100, 239, 0, 12): (239, 155, 20, 200, None),
    (5, 13, 111, 100, 242, 0, 12): (242, 183, 20, 250, None),
    (11, 4, 4, 59, 0, 0, 1): (None, None, None, None, "SpriteLessSwitch"),
    (10, 60, 65, 59, 0, 0, 1): (0, 8, 20, 250, None),
    (10, 1, 1, 59, 41, 0, 655366): (41, 54, 21, 250, None),
    (2, 2, 6, 59, 39, 0, 4): (39, 79, 0, 400, None),
    # The entity relation remains 66 while reset stores particle row 44.
    # Production source-live projection resolves that row to parameter 66.
    (3, 0, 0, 66, 44, 0, 13): (66, 80, 0, 200, None),
    (15, 0, 0, 59, 77, 0, 5): (77, 59, 1, 150, None),
    (15, 1, 3, 59, 77, 0, 5): (77, 59, 1, 150, None),
    (10, 14, 14, 59, 9, 0, 131076): (9, 63, 21, 50, None),
    (10, 0, 0, 59, 7, 1, 655494): (7, 38, 36, 250, None),
}
EXPECTED_ACTOR_COUNTS = {
    "dolmen": 3,
    "goat": 33,
    "villager-a": 3,
    "villager-b": 6,
    "villager-c": 9,
    "villager-d": 10,
}
YAW_LOOKUP_D4750_28 = (0, 0, 0, 1, 1, 1, 2, 2, 2, 2, 2, 1, 1, 1, 0, 0)
# Exact selectors reached by the live Level 1 mission kernel after the frozen
# reset capture, including mode-19 Archer-arrow facings 105..109.
LEVEL1_LIVE_MISSION_TMAPS = (
    75,
    78,
    87,
    178,
    179,
    30,
    31,
    32,
    33,
    34,
    35,
    36,
    37,
    66,
    11,
    12,
    13,
    61,
    88,
    89,
    90,
    91,
    92,
    93,
    94,
    95,
    96,
    97,
    98,
    99,
    100,
    101,
    102,
    103,
    105,
    106,
    107,
    108,
    109,
    121,
    122,
    123,
    279,
    280,
    281,
    282,
    283,
    284,
    285,
    286,
    461,
    463,
    80,
    81,
    138,
    139,
    140,
    141,
    142,
    143,
)
BL32_TILE_SIDE = 32
BL32_TILE_BYTES = BL32_TILE_SIDE * BL32_TILE_SIDE
BL32_ATLAS_WIDTH = 256
BL32_ATLAS_COLUMNS = BL32_ATLAS_WIDTH // BL32_TILE_SIDE
LANGUAGE_PAYLOAD_OFFSET = 4785
LANGUAGE_RECORD_COUNT = 471
LEVEL1_OBJECTIVE_SELECTOR = 48
LEVEL1_OBJECTIVE_IDS = (48, 49, 50, 51, 52, 53)
LEVEL1_REFERENCE_WIDTH = 640
LEVEL1_REFERENCE_HEIGHT = 480
LEVEL1_MINIMAP_DISPLAY_SIDE = 128
LEVEL1_MINIMAP_LOGICAL_SIDE = 128
LEVEL1_MINIMAP_SOURCE_SCALE = 256
HUD_SOURCE_SCALE = 1
LEVEL1_SOUND_IDS = (
    1, 2, 3, 4, 5, 8, 9, 10, 12, 14, 15, 16, 18, 19, 27, 29, 30, 31, 33, 34,
    37, 39, 40, 41, 43, 44, 46, 54, 55, 56, 57, 63,
)
LEVEL1_MATERIALIZED_SOUND_IDS = (
    1, 2, 3, 4, 5, 8, 9, 10, 14, 15, 16, 18, 19, 27, 29, 30, 31, 33, 34, 37,
    39, 40, 41, 43, 44, 46, 54, 55, 56, 57, 63,
)
CASTLE_STAGE_ONE_BUILD_INDEX = 1
LEVEL1_HOUSE_BUILD_INDICES = (17, 26, 29, 30, 31, 34, 42, 47, 50, 51, 52, 53)
CASTLE_STAGE_ONE_BUILD_RAW_WIDTH = 8
CASTLE_STAGE_ONE_BUILD_RAW_HEIGHT = 8
CASTLE_STAGE_ONE_BUILD_PAYLOAD_SHA256 = (
    "f9fb9d8a5c6d2841849bba03479e6503966fa4ec14020508c343296602aa8ae3"
)
CASTLE_STAGE_ONE_BUILD_TAB_RECORD_SHA256 = (
    "27a9da49ded1a0aa1a4e01dbaab8634404f5bf9d6789a5785884e94e35e2f804"
)
CASTLE_STAGE_ONE_BUILD_PARAMETER_SHA256 = (
    "7d6e4762a22170651b49bef86a5bb9d5de987f5d5f3e546901291fd24c411d39"
)
SOUND_QUALITY_ROWS = (1644, 1622, 1611, 822, 811, 800)


class CacheError(ValueError):
    pass


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_json(value: Any) -> bytes:
    return (
        json.dumps(value, sort_keys=True, indent=2, ensure_ascii=True) + "\n"
    ).encode("utf-8")


def canonical_identity_json(value: Any) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
        + "\n"
    ).encode("utf-8")


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise CacheError(f"cannot read canonical JSON {path}: {error}") from error


def _contract_section(
    contract: dict[str, Any], name: str, fields: tuple[str, ...]
) -> dict[str, Any]:
    section = contract.get(name)
    if not isinstance(section, dict):
        raise CacheError(f"closure contract {name} section is missing")
    missing = [field for field in fields if field not in section]
    if missing:
        raise CacheError(
            f"closure contract {name} section is missing: {', '.join(missing)}"
        )
    return {field: section[field] for field in fields}


def conformance_contract_identity_payload(
    contract: dict[str, Any],
) -> dict[str, Any]:
    """Select the byte-contract fields that define the contract identity.

    Operational or descriptive metadata may be added to the contract without
    changing its trusted behavior.  The selected fields remain the exact
    schema, reachability, receipt, and cryptographic identity contract.
    """
    if contract.get("schema") != CONFORMANCE_CONTRACT_SCHEMA:
        raise CacheError("unsupported closure contract schema")

    specification = _contract_section(
        contract, "specification", CONFORMANCE_CONTRACT_SPECIFICATION_FIELDS
    )
    if specification["remc2_commit"] != PINNED_REMC2_COMMIT:
        raise CacheError("closure contract remc2 pin mismatch")
    if not isinstance(specification["matrix_path"], str) or not specification["matrix_path"]:
        raise CacheError("closure contract matrix path is missing")
    for field in CONFORMANCE_CONTRACT_SPECIFICATION_FIELDS[2:]:
        value = specification[field]
        if isinstance(value, bool) or not isinstance(value, int) or value < 1:
            raise CacheError(f"closure contract {field} is invalid")

    trusted_identity = _contract_section(
        contract, "trusted_identity", CONFORMANCE_CONTRACT_IDENTITY_FIELDS
    )
    for field, value in trusted_identity.items():
        if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None:
            raise CacheError(f"closure contract trusted identity {field} is invalid")

    reachable = _contract_section(
        contract, "reachable", CONFORMANCE_CONTRACT_REACHABLE_FIELDS
    )
    if not isinstance(reachable["row_ids"], list) or not reachable["row_ids"]:
        raise CacheError("closure contract reachable rows are missing")
    if (
        not isinstance(reachable["required_observation_families"], list)
        or not reachable["required_observation_families"]
    ):
        raise CacheError("closure contract observation families are missing")
    if (
        not isinstance(reachable["required_observation_mask_sha256"], str)
        or re.fullmatch(r"[0-9a-f]{64}", reachable["required_observation_mask_sha256"])
        is None
    ):
        raise CacheError("closure contract observation mask identity is invalid")

    receipt_requirements = _contract_section(
        contract, "receipt_requirements", CONFORMANCE_CONTRACT_RECEIPT_FIELDS
    )
    for field in CONFORMANCE_CONTRACT_RECEIPT_FIELDS[:-1]:
        if not isinstance(receipt_requirements[field], bool):
            raise CacheError(f"closure contract receipt requirement {field} is invalid")
    clean_run_count = receipt_requirements["clean_run_count"]
    if isinstance(clean_run_count, bool) or not isinstance(clean_run_count, int) or clean_run_count < 1:
        raise CacheError("closure contract clean run count is invalid")

    blocked_until = contract.get("blocked_until")
    if not isinstance(blocked_until, list):
        raise CacheError("closure contract blocked_until section is invalid")
    return {
        "schema": contract["schema"],
        "specification": specification,
        "trusted_identity": trusted_identity,
        "reachable": reachable,
        "blocked_until": blocked_until,
        "receipt_requirements": receipt_requirements,
    }


def load_conformance_contract(
    path: Path = DEFAULT_CONFORMANCE_CONTRACT,
    expected_file_sha256: str | None = None,
) -> tuple[dict[str, Any], str]:
    # Kept as a source-compatible argument for the existing qualification
    # runner.  The contract file is not an identity-bearing receipt; its
    # semantic byte-contract self-hash is the identity used below.
    del expected_file_sha256
    path = path.expanduser().resolve()
    contract = load_json(path)
    identity_payload = conformance_contract_identity_payload(contract)
    claimed = contract.get("contract_sha256")
    if not isinstance(claimed, str) or re.fullmatch(r"[0-9a-f]{64}", claimed) is None:
        raise CacheError("closure contract self-hash is missing or invalid")
    if sha256_bytes(canonical_identity_json(identity_payload)) != claimed:
        raise CacheError("closure contract self-hash mismatch")
    return contract, claimed


def pinned_remc2_blob(relative_path: str) -> bytes:
    if relative_path not in PINNED_REMC2_BLOB_SHA256:
        raise CacheError(f"unapproved pinned remc2 source path: {relative_path}")
    result = subprocess.run(
        [
            "git",
            "-C",
            str(REMC2_GIT),
            "show",
            f"{PINNED_REMC2_COMMIT}:{relative_path}",
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        raise CacheError(
            f"cannot read pinned remc2 source {relative_path}: {detail}"
        )
    actual_sha256 = sha256_bytes(result.stdout)
    expected_sha256 = PINNED_REMC2_BLOB_SHA256.get(relative_path)
    if actual_sha256 != expected_sha256:
        raise CacheError(
            f"pinned remc2 blob changed: {relative_path} "
            f"(expected {expected_sha256}, got {actual_sha256})"
        )
    return result.stdout


def exporter_layout_evidence(source_path: Path = EXPORTER_SOURCE) -> dict[str, Any]:
    resolved_source = source_path.expanduser().resolve()
    try:
        blob = resolved_source.read_bytes()
        source = blob.decode("utf-8")
    except (OSError, UnicodeDecodeError) as error:
        raise CacheError(f"cannot read exporter source: {error}") from error

    signature = "Bytes capture_runtime(json& layout)"
    start = source.find(signature)
    if start < 0:
        raise CacheError("qualified exporter runtime schema is missing capture_runtime")
    opening = source.find("{", start)
    if opening < 0:
        raise CacheError("qualified exporter runtime schema has no function body")
    depth = 0
    end = None
    for index in range(opening, len(source)):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                end = index + 1
                break
    if end is None:
        raise CacheError("qualified exporter runtime schema has an unclosed body")
    runtime_source = source[start:end]
    required_markers = (
        'result.magic("MCR1");',
        "result.u16(1);",
        "result.u32(D41A0_0.dword_0x4);",
        "result.u8(D41A0_0.m_GameSettings.m_Graphics.m_wReflections);",
        "result.u8(D41A0_0.m_GameSettings.str_0x2196.flat_0x2199);",
        'layout = {',
        '{"bytes", result.data.size()}',
    )
    missing = [marker for marker in required_markers if marker not in runtime_source]
    if missing:
        raise CacheError(
            "qualified exporter runtime schema changed; missing semantic fields: "
            + ", ".join(missing)
        )
    marker_positions = [runtime_source.index(marker) for marker in required_markers]
    if marker_positions != sorted(marker_positions):
        raise CacheError("qualified exporter runtime schema changed; field order changed")
    first_line = source.count("\n", 0, start) + 1
    last_line = source.count("\n", 0, end) + 1
    span = runtime_source.encode("utf-8")
    actual_sha256 = sha256_bytes(blob)
    span_sha256 = sha256_bytes(span)
    try:
        source_label = str(resolved_source.relative_to(REPO))
    except ValueError:
        source_label = str(resolved_source)
    return {
        "source": source_label,
        "source_sha256": actual_sha256,
        "capture_runtime_first_line": first_line,
        "capture_runtime_last_line": last_line,
        "capture_runtime_span_sha256": span_sha256,
        "runtime_bytes": 2_754,
        "settings_offset": 49,
        "settings_length": 16,
    }


def pinned_remc2_source_evidence() -> dict[str, Any]:
    by_path: dict[str, list[tuple[int, int, str]]] = {}
    for relative_path, first_line, last_line, purpose in PINNED_REMC2_SOURCE_SPANS:
        by_path.setdefault(relative_path, []).append(
            (first_line, last_line, purpose)
        )
    files = []
    for relative_path, spans in by_path.items():
        blob = pinned_remc2_blob(relative_path)
        lines = blob.splitlines(keepends=True)
        span_rows = []
        for first_line, last_line, purpose in spans:
            if first_line < 1 or last_line < first_line or last_line > len(lines):
                raise CacheError(
                    f"pinned remc2 span is out of bounds: "
                    f"{relative_path}:{first_line}-{last_line}"
                )
            payload = b"".join(lines[first_line - 1:last_line])
            span_rows.append(
                {
                    "first_line": first_line,
                    "last_line": last_line,
                    "line_count": last_line - first_line + 1,
                    "sha256": sha256_bytes(payload),
                    "purpose": purpose,
                }
            )
        files.append(
            {
                "path": relative_path,
                "bytes": len(blob),
                "line_count": len(lines),
                "blob_sha256": sha256_bytes(blob),
                "spans": span_rows,
            }
        )
    validated_blobs = {
        relative_path: sha256_bytes(pinned_remc2_blob(relative_path))
        for relative_path in sorted(PINNED_REMC2_BLOB_SHA256)
    }
    return {
        "authority": "git_show_pinned_commit_only",
        "commit": PINNED_REMC2_COMMIT,
        "working_tree_files_read": False,
        "validated_blob_sha256": validated_blobs,
        "validated_blob_count": len(validated_blobs),
        "files": files,
    }


def confined_source(root: Path, relative: str) -> Path:
    candidate = root / relative
    if candidate.is_symlink():
        raise CacheError(f"owner source must not be a symlink: {relative}")
    resolved = candidate.resolve()
    try:
        resolved.relative_to(root)
    except ValueError as error:
        raise CacheError(f"owner source escapes root: {relative}") from error
    if not resolved.is_file():
        raise CacheError(f"owner source missing: {relative}")
    return resolved


def validate_output_path(output: Path) -> Path:
    resolved = output.expanduser().resolve()
    try:
        resolved.relative_to(PRIVATE.resolve())
    except ValueError as error:
        raise CacheError("cache output must remain under research/private") from error
    for forbidden in (
        UNITY / "Assets",
        UNITY / "StreamingAssets",
        UNITY / "Builds",
    ):
        try:
            resolved.relative_to(forbidden.resolve())
        except ValueError:
            continue
        raise CacheError(f"cache output is a distributable path: {resolved}")
    if resolved == PRIVATE.resolve():
        raise CacheError("cache output cannot be the private root")
    return resolved


def load_proven_decoder():
    spec = importlib.util.spec_from_file_location("owner_catalog_decoder", PROVEN_DECODER)
    if spec is None or spec.loader is None:
        raise CacheError("cannot load proven owner catalog decoder")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.decode_indexed_rle


def load_proven_export_verifier():
    spec = importlib.util.spec_from_file_location("level_asset_closure", PROVEN_CLOSURE)
    if spec is None or spec.loader is None:
        raise CacheError("cannot load proven package identity verifier")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.verify_export_identity


def materialize_indexed_rle(
    dat: bytes, offset: int, width: int, height: int
) -> tuple[bytes, bytes, int]:
    if width == 0 or height == 0:
        return b"", b"", 0
    cursor = offset
    pixels = bytearray(width * height)
    visible = bytearray(width * height)
    for y in range(height):
        x = 0
        while True:
            if cursor >= len(dat):
                raise CacheError("RLE exceeds DAT")
            token = dat[cursor]
            cursor += 1
            if token == 0:
                break
            if token & 0x80:
                x += 256 - token
                if x > width:
                    raise CacheError("transparent run exceeds row width")
                continue
            if cursor + token > len(dat) or x + token > width:
                raise CacheError("pixel run exceeds source or row width")
            start = y * width + x
            pixels[start : start + token] = dat[cursor : cursor + token]
            visible[start : start + token] = b"\x01" * token
            cursor += token
            x += token
    return bytes(pixels), bytes(visible), cursor - offset


def bitmap_records(tab: bytes) -> list[tuple[int, int, int]]:
    if len(tab) % TAB_RECORD.size:
        raise CacheError("TAB is truncated or not the proven six-byte <IBB schema")
    return list(TAB_RECORD.iter_unpack(tab))


def language_records(payload: bytes) -> list[bytes]:
    if len(payload) <= LANGUAGE_PAYLOAD_OFFSET:
        raise CacheError("owner localization payload is shorter than the proven prefix")
    records = payload[LANGUAGE_PAYLOAD_OFFSET:].split(b"\0")
    if len(records) < LANGUAGE_RECORD_COUNT:
        raise CacheError("owner localization payload has fewer than 471 records")
    result = records[:LANGUAGE_RECORD_COUNT]
    if any(b"\0" in value for value in result):
        raise CacheError("owner localization record retained a NUL byte")
    return result


def qualified_runtime_profile(
    package_dir: Path,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    captures = []
    for name in QUALIFIED_RUNTIME_CAPTURES:
        path = GENERATED / name / "runtime.bin"
        payload = path.read_bytes()
        if (
            len(payload) != 2_754
            or sha256_bytes(payload) != EXPECTED_RUNTIME_SHA256
            or payload[49:65] != QUALIFIED_SETTINGS_BYTES
        ):
            raise CacheError(f"qualified runtime profile changed: {name}/runtime.bin")
        captures.append(
            {
                "path": str(path.relative_to(REPO)),
                "bytes": len(payload),
                "sha256": EXPECTED_RUNTIME_SHA256,
                "settings_offset": 49,
                "settings_bytes_hex": payload[49:65].hex(),
            }
        )
    package_runtime = (package_dir / "runtime.bin").read_bytes()
    if (
        len(package_runtime) != 2_754
        or sha256_bytes(package_runtime) != EXPECTED_RUNTIME_SHA256
        or package_runtime[49:65] != QUALIFIED_SETTINGS_BYTES
    ):
        raise CacheError("selected Level 1 package runtime profile changed")
    fields = (
        "reflections",
        "shadows",
        "sky",
        "viewport_size",
        "minimap",
        "top_bar",
        "screen_size",
        "configured_blur_mode",
        "spatial_smoothing",
        "stereo",
        "resolution",
        "shadow_profile",
        "dynamic_lighting",
        "setting_0x2197",
        "transparency",
        "flat",
    )
    exporter_layout = exporter_layout_evidence()
    exporter_layout["field_order"] = list(fields)
    return captures, {
        "exporter_layout": exporter_layout,
        "values": dict(zip(fields, QUALIFIED_SETTINGS_BYTES, strict=True)),
        "settings_bytes_hex": QUALIFIED_SETTINGS_BYTES.hex(),
        "capture_count": len(captures),
        "capture_sha256": EXPECTED_RUNTIME_SHA256,
    }


def source_sprite_tuple(
    class_id: int,
    model: int,
    action: int,
    parameter: int | None,
    control: int,
    disposition: str,
    *,
    base_tmap: int | None = None,
    render_mode: int | None = None,
    logical_height: int | None = None,
    reason: str | None = None,
    source_parameter_index: int | None = None,
    particle_parameter_index: int | None = None,
    xclass: int | None = None,
    xmodel: int | None = None,
) -> dict[str, Any]:
    if disposition == "Render" and None in (parameter, base_tmap, render_mode, logical_height):
        raise CacheError("render tuple is missing concrete presentation fields")
    if disposition == "Excluded" and reason is None:
        raise CacheError("excluded tuple is missing its typed reason")
    return {
        "class": class_id,
        "model": model,
        "action": action,
        "xclass": xclass,
        "xmodel": xmodel,
        "source_parameter_index": (
            parameter if source_parameter_index is None else source_parameter_index
        ),
        "particle_parameter_index": (
            parameter if particle_parameter_index is None else particle_parameter_index
        ),
        "render_parameter_index": parameter,
        "control_flags_u32": control,
        "disposition": disposition,
        "base_tmap": base_tmap,
        "render_mode": render_mode,
        "logical_height": logical_height,
        "excluded_reason": reason,
        "payload_wildcard": False,
    }


def reset_manifest_outcomes(
    row: str,
    kind: int,
    subtype: int,
) -> list[dict[str, Any]]:
    semantic_by_row = {
        "AUT-G0-DOLMENS": {"dolmen"},
        "AUT-G0-EMITTERS": {"ambient-emitter"},
        "AUT-G0-FIRES": {"death-burst", "smoke-particle", "fireball-impact"},
        "AUT-G0-GOATS": {"goat"},
        "AUT-G0-MARKER": {
            "local-player",
            "left-spell-holder",
            "right-spell-holder",
        },
        "AUT-G0-SWITCHES": {"SpriteLessSwitch"},
        "AUT-G0-VILLAGERS": {
            "villager-a",
            "villager-b",
            "villager-c",
            "villager-d",
        },
    }
    semantics = semantic_by_row.get(row)
    if semantics is None:
        raise CacheError(f"authored reset row has no exact manifest join: {row}")
    outcomes = []
    for key, (_, semantic) in RESET_SOURCE_TUPLE_DISPOSITIONS.items():
        if semantic not in semantics:
            continue
        (
            class_id,
            model,
            action,
            source_parameter,
            particle_parameter,
            animation_frame,
            control,
        ) = key
        if row == "AUT-G0-SWITCHES" and (
            class_id != kind or model != subtype or action != subtype
        ):
            continue
        render_parameter, base, mode, height, reason = (
            RESET_SOURCE_PRESENTATION_PROFILES[key]
        )
        outcome = source_sprite_tuple(
            class_id,
            model,
            action,
            render_parameter,
            control,
            "Excluded" if reason else "Render",
            base_tmap=base,
            render_mode=mode,
            logical_height=height,
            reason=reason,
            source_parameter_index=source_parameter,
            particle_parameter_index=particle_parameter,
        )
        outcome["animation_frame"] = animation_frame
        outcome["reset_semantic"] = semantic
        outcomes.append(outcome)
    if not outcomes:
        raise CacheError(f"authored reset row joined no source tuples: {row}")
    return outcomes


def authored_activation_outcomes(
    row: str,
    kind: int,
    subtype: int,
    group: int,
) -> list[dict[str, Any]]:
    tuple_key = (kind, subtype, group)
    exact = {
        "AUT-G-1-HOUSES": tuple_key == (10, 45, -1),
        "AUT-G-1-TERRAIN29": tuple_key == (10, 29, -1),
        "AUT-G0-DOLMENS": tuple_key == (2, 2, 0),
        "AUT-G0-EMITTERS": tuple_key == (10, 60, 0),
        "AUT-G0-FIRES": tuple_key == (10, 1, 0),
        "AUT-G0-GOATS": tuple_key == (5, 1, 0),
        "AUT-G0-MARKER": tuple_key == (3, 4, 0),
        "AUT-G0-SWITCHES": tuple_key in ((11, 0, 0), (11, 1, 0), (11, 4, 0)),
        "AUT-G0-VILLAGERS": tuple_key == (5, 13, 0),
        "AUT-G1": tuple_key in ((2, 0, 1), (11, 0, 1), (11, 32, 1), (15, 2, 1)),
        "AUT-G2": tuple_key in ((11, 1, 2), (11, 32, 2)),
        "AUT-G3": tuple_key in ((5, 4, 3), (11, 17, 3), (11, 32, 3)),
        "AUT-G4": tuple_key in (
            (5, 3, 4), (10, 59, 4), (11, 0, 4), (11, 12, 4),
            (11, 16, 4), (14, 3, 4), (14, 5, 4),
        ),
        "AUT-G5": tuple_key == (11, 32, 5),
        "AUT-G6": tuple_key in ((2, 1, 6), (5, 19, 6)),
        "AUT-G8": tuple_key == (11, 0, 8),
        "AUT-G11": tuple_key == (15, 3, 11),
        "AUT-G12": tuple_key == (10, 59, 12),
        "AUT-G33": tuple_key == (5, 19, 33),
        "AUT-G34": tuple_key == (5, 3, 34),
        "AUT-G13-32-FIRES":
            kind == 10 and subtype == 1 and 13 <= group <= 32,
        "AUT-G13-32-HOUSES":
            kind == 10 and subtype == 45 and 13 <= group <= 32,
        "AUT-G13-32-SPLASH":
            kind == 10 and subtype == 5 and 13 <= group <= 32,
    }
    if row not in exact or not exact[row]:
        raise CacheError(
            "unclassified in-range authored presentation tuple: "
            f"row={row} kind={kind} subtype={subtype} group={group}"
        )

    if row.startswith("AUT-G0"):
        return reset_manifest_outcomes(row, kind, subtype)
    if row == "AUT-G-1-TERRAIN29":
        return [{"disposition": "NoSourceAllocation", "payload_wildcard": False}]
    if kind == 11 or (kind, subtype) == (10, 59):
        return [
            source_sprite_tuple(
                kind, subtype, subtype if kind == 11 else 64, None, 8,
                "Excluded", reason="SpriteLessSwitch",
            )
        ]
    if row == "AUT-G-1-HOUSES":
        return [source_sprite_tuple(10, 45, 52, 177, 13, "Render",
                                    base_tmap=96, render_mode=0, logical_height=400)]
    if (kind, subtype) == (2, 0):
        return [
            source_sprite_tuple(2, 0, 0, parameter, 8, "Render",
                                base_tmap=base, render_mode=0, logical_height=400)
            for parameter, base in ((83, 75), (84, 87))
        ]
    if (kind, subtype) == (2, 1):
        return [source_sprite_tuple(2, 1, 3, 79, 8, "Render",
                                    base_tmap=78, render_mode=0, logical_height=250)]
    if (kind, subtype) == (5, 4):
        return [source_sprite_tuple(5, 4, 33, 0, 8, "Render",
                                    base_tmap=8, render_mode=20, logical_height=250)]
    if (kind, subtype) == (5, 19):
        return [source_sprite_tuple(5, 19, 159, 287, 8, "Render",
                                    base_tmap=279, render_mode=17, logical_height=200)]
    if (kind, subtype) == (5, 3):
        outcomes = [
            source_sprite_tuple(5, 3, 25, 88, 8, "Render",
                                base_tmap=30, render_mode=17, logical_height=180)
        ]
        segment_heights = (150, 165, 185, 200, 210, 210, 200, 190,
                           170, 160, 150, 130, 110, 90, 70, 60)
        outcomes.extend(
            source_sprite_tuple(5, 3, 232, 89 + ordinal, 8, "Render",
                                base_tmap=66, render_mode=0,
                                logical_height=height)
            for ordinal, height in enumerate(segment_heights)
        )
        return outcomes
    if (kind, subtype) in ((15, 2), (15, 3)):
        return [source_sprite_tuple(15, subtype, 7 if subtype == 2 else 10,
                                    77, 8, "Render", base_tmap=59,
                                    render_mode=1, logical_height=150)]
    if (kind, subtype) == (14, 3):
        return [source_sprite_tuple(14, 3, 8, 338, 8, "Render",
                                    base_tmap=461, render_mode=0, logical_height=700)]
    if (kind, subtype) == (14, 5):
        return [source_sprite_tuple(14, 5, 10, 280, 8, "Render",
                                    base_tmap=259, render_mode=1, logical_height=75)]
    if (kind, subtype) == (10, 45):
        return [source_sprite_tuple(10, 45, 51, 177, 13, "Render",
                                    base_tmap=96, render_mode=0, logical_height=400)]
    if (kind, subtype) == (10, 1):
        return [source_sprite_tuple(10, 1, 1, 41, 0x0002_0004, "Render",
                                    base_tmap=54, render_mode=21, logical_height=250)]
    if (kind, subtype) == (10, 5):
        return [source_sprite_tuple(10, 5, 5, 244, 0x0002_0004, "Render",
                                    base_tmap=186, render_mode=11,
                                    logical_height=300)]
    raise CacheError(
        f"authored tuple has no concrete presentation outcome: {row}/{tuple_key}"
    )


def dynamic_allocator_sprite_census() -> list[dict[str, Any]]:
    rows = [
        ("PlayerFireball", "c09-constructor-000",
         [source_sprite_tuple(9, 0, 0, 340, 8, "Render",
                              base_tmap=463, render_mode=21, logical_height=100)]),
        ("FireflyFireball", "c09-constructor-000",
         [source_sprite_tuple(9, 0, 0, 340, 8, "Render",
                              base_tmap=463, render_mode=21, logical_height=100)]),
        ("ArcherArrow", "c09-constructor-013",
         [source_sprite_tuple(9, 13, 13, 195, 8, "Render",
                              base_tmap=105, render_mode=19, logical_height=60)]),
        ("CastleBuild", "c09-constructor-010",
         [source_sprite_tuple(9, 10, 10, 18, 8, "Render",
                              base_tmap=61, render_mode=21, logical_height=150)]),
        ("CastleRoot", "c03-constructor-002",
         [source_sprite_tuple(3, 2, 5, 177, 8, "Render",
                              base_tmap=96, render_mode=0, logical_height=400)]),
        ("CastleConstruction", "c10-constructor-042",
         [source_sprite_tuple(10, 42, 44, None, 8, "Excluded",
                              reason="CastleConstruction")]),
        ("CastleDefenseController", "c10-constructor-079",
         [source_sprite_tuple(10, 79, 86, 66, 8, "Render",
                              base_tmap=80, render_mode=0, logical_height=200)]),
        ("CastleAssociation", "c10-constructor-043",
         [source_sprite_tuple(10, 43, 45, 41, 8, "Render",
                              base_tmap=54, render_mode=21, logical_height=250)]),
        ("Possession-ordinary", "c09-constructor-001",
         [source_sprite_tuple(9, 1, 1, 209, 8, "Render",
                              base_tmap=144, render_mode=21, logical_height=150)]),
        ("Possession-strong", "c09-constructor-017",
         [source_sprite_tuple(9, 17, 18, 209, 8, "Render",
                              base_tmap=144, render_mode=21, logical_height=150)]),
        ("PossessionImpact", "c10-constructor-012",
         [source_sprite_tuple(10, 12, 12, 41, 8, "Excluded",
                              reason="HiddenSourceModel")]),
        ("PossessionAttraction", "c10-constructor-054",
         [source_sprite_tuple(10, 54, 59, 0, 8, "Excluded",
                              reason="HiddenSourceModel")]),
        ("StrongPossessionImpact", "c10-constructor-070",
         [source_sprite_tuple(10, 70, 77, 0, 8, "Excluded",
                              reason="HiddenSourceModel")]),
        ("FireballImpact", "c10-constructor-000",
         [source_sprite_tuple(10, 0, 0, 7, 8, "Render",
                              base_tmap=38, render_mode=36, logical_height=250)]),
        ("WaterSplash", "c10-constructor-005",
         [source_sprite_tuple(10, 5, 5, 244, 8, "Render",
                              base_tmap=186, render_mode=11, logical_height=300)]),
        ("ManaCarrier", "c03-constructor-003",
         [source_sprite_tuple(3, 3, 9, 169, 8, "Render",
                              base_tmap=88, render_mode=0, logical_height=800)]),
        ("DeathBurst", "c10-constructor-001",
         [source_sprite_tuple(10, 1, 1, 41, 0x0002_0000, "Render",
                              base_tmap=54, render_mode=21, logical_height=250),
          source_sprite_tuple(10, 1, 1, 41, 0x000a_0000, "Render",
                              base_tmap=54, render_mode=21, logical_height=250)]),
        ("PlayerDeathMarker", "c10-constructor-040",
         [source_sprite_tuple(10, 40, 42, 65, 8, "Render",
                              base_tmap=81, render_mode=0, logical_height=200)]),
        ("SpeedTrail", "c10-constructor-002",
         [source_sprite_tuple(10, 2, 2, None, 8, "Excluded",
                              reason="UnlinkedSpeedTrail")]),
        ("AmbientFireChild", "c10-constructor-000",
         [source_sprite_tuple(10, 0, 0, 7, 0x000a_0084, "Render",
                              base_tmap=38, render_mode=36, logical_height=250)]),
        ("AmbientSmoke", "c10-constructor-014",
         [source_sprite_tuple(10, 14, 14, 9, 0x0002_0004, "Render",
                              base_tmap=63, render_mode=21, logical_height=50)]),
        ("ResidentArcher", "c05-constructor-004",
         [source_sprite_tuple(5, 4, 33, 0, 8, "Render",
                              base_tmap=8, render_mode=20, logical_height=250)]),
        ("ResidentBuilder", "c05-constructor-012",
         [source_sprite_tuple(5, 12, 97, 221, 8, "Render",
                              base_tmap=141, render_mode=20, logical_height=250)]),
        ("ResidentTrader", "c05-constructor-014",
         [source_sprite_tuple(5, 14, 113, 219, 8, "Render",
                              base_tmap=138, render_mode=20, logical_height=250)]),
    ]
    rows.append((
        "GroundFire", "c10-constructor-006",
        [source_sprite_tuple(10, 6, 6, 228 + phase, 8, "Render",
                             base_tmap=77, render_mode=1, logical_height=height)
         for phase, height in enumerate((50, 150, 300, 450, 600, 700, 800, 950))]
    ))
    rows.append((
        "CheckpointSmoke", "c10-constructor-013",
        [source_sprite_tuple(10, 13, 13, 67 + phase, 8, "Render",
                             base_tmap=57, render_mode=21,
                             logical_height=height)
         for phase, height in enumerate(
             (50, 100, 150, 200, 250, 300, 350, 400))]
    ))
    rows.append((
        "ManaSphere", "c10-constructor-039",
        [source_sprite_tuple(10, 39, 41, parameter, 8, "Render",
                             base_tmap=base, render_mode=21,
                             logical_height=height)
         for base, first in ((58, 52), (67, 105))
         for parameter, height in zip(
             range(first, first + 7),
             (50, 100, 150, 200, 250, 300, 350),
             strict=True,
         )]
    ))
    rows.append((
        "MultipartSegment", "c05-constructor-003",
        [source_sprite_tuple(5, 3, 232, 89 + ordinal, 8, "Render",
                             base_tmap=66, render_mode=0, logical_height=height)
         for ordinal, height in enumerate(
             (150, 165, 185, 200, 210, 210, 200, 190,
              170, 160, 150, 130, 110, 90, 70, 60))]
    ))
    rows.append((
        "ResidentVillager", "c05-constructor-013",
        [source_sprite_tuple(5, 13, 105, parameter, 8, "Render",
                             base_tmap=base, render_mode=20, logical_height=height)
         for parameter, base, height in (
             (239, 155, 200), (241, 180, 250),
             (242, 183, 250), (271, 199, 300))]
    ))
    rows.append((
        "SpellPickup", "c15-constructor-000..003",
        [source_sprite_tuple(15, family, action, 77, 8, "Render",
                             base_tmap=59, render_mode=1, logical_height=150)
         for family, action in enumerate((2, 5, 8, 11))]
    ))
    for ordinal, shot in enumerate(CASTLE_DEFENSE_SHOT_ROWS):
        outcome = source_sprite_tuple(
            shot["entity_class"],
            shot["entity_model"],
            shot["entity_action"],
            shot["particle_parameter_index"],
            8,
            "Render",
            base_tmap=shot["base_tmap"],
            render_mode=shot["render_mode"],
            logical_height=shot["logical_height"],
            source_parameter_index=shot["entity_parameter_index"],
            particle_parameter_index=shot["particle_parameter_index"],
            xclass=shot["xclass"],
            xmodel=shot["xmodel"],
        )
        outcome.update(shot)
        rows.append((
            f"CastleDefenseShot-{ordinal}",
            f"c09-constructor-{shot['entity_model']:03d}",
            [outcome],
        ))
    return [
        {
            "spawn_kind": spawn_kind,
            "dispatch_slot": dispatch_slot,
            "outcomes": outcomes,
            "outcome_count": len(outcomes),
            "payload_wildcard": False,
        }
        for spawn_kind, dispatch_slot, outcomes in rows
    ]


def load_level1_conformance_matrix(
    path: Path = CANONICAL_LEVEL1_MATRIX,
) -> tuple[dict[str, Any], str]:
    matrix_path = path.expanduser().resolve()
    matrix = load_json(matrix_path)
    schema = matrix.get("schema")
    if (
        not isinstance(schema, dict)
        or schema.get("id") != LEVEL1_MATRIX_SCHEMA_ID
        or schema.get("version") != LEVEL1_MATRIX_SCHEMA_VERSION
    ):
        raise CacheError("unsupported Level 1 matrix schema")
    for section in ("authored_census", "dispatch_census"):
        if not isinstance(matrix.get(section), dict):
            raise CacheError(f"Level 1 matrix {section} section is missing")
    return matrix, sha256_file(matrix_path)


def build_source_tuple_spawn_census(
    authored_blob: bytes,
    entities_blob: bytes,
    matrix_path: Path = CANONICAL_LEVEL1_MATRIX,
) -> dict[str, Any]:
    matrix, matrix_file_sha256 = load_level1_conformance_matrix(matrix_path)
    resolved_matrix_path = matrix_path.expanduser().resolve()
    try:
        matrix_label = str(resolved_matrix_path.relative_to(REPO))
    except ValueError:
        matrix_label = str(resolved_matrix_path)
    authored = matrix["authored_census"]
    if (
        authored["record_count"] != AUTHORED_FEATURE_COUNT
        or authored["record_bytes"] != AUTHORED_FEATURE_BYTES
        or authored["table_offset"] != AUTHORED_FEATURE_OFFSET
        or len(authored["records"]) != 308
    ):
        raise CacheError("canonical authored census extent changed")
    row_for_index: dict[int, str] = {}
    for row, indices in authored["row_coverage"].items():
        for index in indices:
            if index in row_for_index:
                raise CacheError(f"authored index {index} occurs in two matrix rows")
            row_for_index[index] = row
    if set(row_for_index) != {int(index) for index in authored["records"]}:
        raise CacheError("authored record/row partition changed")

    authored_rows = []
    for index_text, manifest in sorted(
        authored["records"].items(), key=lambda item: int(item[0])
    ):
        index = int(index_text)
        offset = AUTHORED_FEATURE_OFFSET + index * AUTHORED_FEATURE_BYTES
        kind, subtype, _, _, group = struct.unpack_from("<HhHHh", authored_blob, offset)
        expected = (
            int(manifest["kind"]),
            int(manifest["subtype"]),
            int(manifest["activation_group"]),
        )
        if (kind, subtype, group) != expected:
            raise CacheError(
                f"owner/matrix authored tuple mismatch at {index}: "
                f"{(kind, subtype, group)} != {expected}"
            )
        outcomes = authored_activation_outcomes(
            row_for_index[index], kind, subtype, group
        )
        authored_rows.append({
            "authored_index": index,
            "matrix_row": row_for_index[index],
            "kind": kind,
            "subtype": subtype,
            "activation_group": group,
            "presentation_outcomes": outcomes,
            "payload_wildcard": False,
        })

    negative = dict(authored_rows[0])
    negative["kind"] = 99
    try:
        authored_activation_outcomes(
            negative["matrix_row"],
            negative["kind"],
            negative["subtype"],
            negative["activation_group"],
        )
    except CacheError:
        pass
    else:
        raise CacheError("in-range unclassified authored tuple negative was accepted")

    dispatch = matrix["dispatch_census"]
    slots = set(dispatch["slots"])
    if len(slots) != 845:
        raise CacheError("canonical 845-slot dispatch census changed")
    for slot in sorted(slots):
        matrix_rows = dispatch["slot_to_row"].get(slot, [])
        exclusion = dispatch["slot_to_exclusion"].get(slot)
        if bool(matrix_rows) == bool(exclusion):
            raise CacheError(
                f"canonical dispatch slot is not classified exactly once: {slot}"
            )
    dynamic = dynamic_allocator_sprite_census()
    if len(dynamic) != 35:
        raise CacheError("dynamic allocator family census must remain exactly 35")
    for row in dynamic:
        slot = row["dispatch_slot"]
        checked = (
            [f"c15-constructor-{index:03d}" for index in range(4)]
            if slot == "c15-constructor-000..003"
            else [slot]
        )
        if any(candidate not in slots for candidate in checked):
            raise CacheError(f"dynamic allocator dispatch join is absent: {slot}")
        routes = []
        for candidate in checked:
            matrix_rows = dispatch["slot_to_row"].get(candidate, [])
            exclusion = dispatch["slot_to_exclusion"].get(candidate)
            if bool(matrix_rows) == bool(exclusion):
                raise CacheError(
                    "dynamic allocator dispatch slot is not classified exactly once: "
                    f"{candidate}"
                )
            routes.append({
                "slot": candidate,
                "matrix_rows": matrix_rows,
                "matrix_exclusion": exclusion,
            })
        row["dispatch_routes"] = routes

    reset_records = []
    for key, (_, semantic) in RESET_SOURCE_TUPLE_DISPOSITIONS.items():
        (
            class_id,
            model,
            action,
            source_parameter,
            particle_parameter,
            animation_frame,
            control,
        ) = key
        render_parameter, base, mode, height, reason = (
            RESET_SOURCE_PRESENTATION_PROFILES[key]
        )
        profile = source_sprite_tuple(
            class_id,
            model,
            action,
            render_parameter,
            control,
            "Excluded" if reason else "Render",
            base_tmap=base,
            render_mode=mode,
            logical_height=height,
            reason=reason,
            source_parameter_index=source_parameter,
            particle_parameter_index=particle_parameter,
        )
        profile["animation_frame"] = animation_frame
        profile["reset_semantic"] = semantic
        reset_records.append(profile)
    if len(reset_records) != 17:
        raise CacheError("reset tuple census must remain exactly 17")

    # The 17-key package reset closure is independently checked while building
    # entities/presentation.json. Bind its exact byte identity here too.
    return {
        "schema": "mc2-level1-source-tuple-spawn-census/v1",
        "matrix": {
            "path": matrix_label,
            "sha256": matrix_file_sha256,
            "authored_record_count": len(authored_rows),
            "dispatch_slot_count": len(slots),
        },
        "owner_authored_sha256": sha256_bytes(authored_blob),
        "owner_entities_sha256": sha256_bytes(entities_blob),
        "reset_tuple_manifest": {
            "tuple_count": len(RESET_SOURCE_TUPLE_DISPOSITIONS),
            "active_entity_count": 177,
            "payload_wildcards": 0,
            "join": "entities/presentation.json source_tuple_census",
            "records": reset_records,
        },
        "authored_records": authored_rows,
        "dynamic_allocator_families": dynamic,
        "dynamic_allocator_family_count": len(dynamic),
        "exhaustive_classification": {
            "authored_records": len(authored_rows),
            "dynamic_allocator_families": len(dynamic),
            "reset_tuple_keys": len(reset_records),
            "dispatch_slots": len(slots),
            "unclassified_reachable_outcomes": 0,
        },
        "payload_wildcards": 0,
        "unknown_tuple_policy": "hard_error",
        "mutation_negative": "in_range_authored_kind_99_rejected",
    }


def build_source_frame_census(
    *,
    pinned_source_evidence: dict[str, Any],
    runtime_captures: list[dict[str, Any]],
    runtime_profile: dict[str, Any],
    reset_main_output: dict[str, Any],
    reset_palette_output: dict[str, Any],
    language_rows: list[dict[str, Any]],
    language_output: dict[str, Any],
    source_bitmap_assets: list[dict[str, Any]],
    adapted_bitmap_assets: list[dict[str, Any]],
    source_font: dict[str, Any],
    adapted_font: dict[str, Any],
    cursor_asset: dict[str, Any],
    source_tuple_census: dict[str, Any],
) -> dict[str, Any]:
    with SPELL_TABLE.open(newline="", encoding="utf-8") as stream:
        spell_source_rows = list(csv.DictReader(stream))
    if len(spell_source_rows) != 78:
        raise CacheError("source frame spell ledger must have 78 tier rows")
    spell_profiles = []
    for family in range(26):
        family_rows = sorted(
            (
                row
                for row in spell_source_rows
                if int(row["family_id"]) == family
            ),
            key=lambda row: int(row["tier"]),
        )
        if [int(row["tier"]) for row in family_rows] != [0, 1, 2]:
            raise CacheError(
                f"source frame spell ledger family {family} is incomplete"
            )
        spell_profiles.append({
            "family": family,
            "family_header_tier_count": int(
                family_rows[0]["family_header_tier_count"]
            ),
            "family_flags": int(family_rows[0]["family_initial_flags_raw"]),
            "mana_cost": [int(row["mana_cost"]) for row in family_rows],
            "max_mana_limit": [
                int(row["max_mana_requirement"]) for row in family_rows
            ],
            "single_player_experience": [
                int(row["single_player_experience"]) for row in family_rows
            ],
        })
    ordinary = [
        "SpiderWeb",
        "MinimapTerrain",
        "MinimapEntities",
        "MinimapMarks",
        "LeftSpell",
        "RightSpell",
        "TopStatus",
        "Menu",
        "PauseOrEndText",
    ]
    post = [
        "ObjectiveTextbox",
        "TutorialOrBriefing",
        "HelpPopup",
        "DebugText",
    ]
    map_before_bottom = [
        "MapClear",
        "MapTerrain",
        "MapEntities",
        "MapWorldViewport",
        "MapDividers",
        "MapMarks",
        "Menu",
    ]
    map_after_bottom = [
        "PauseOrEndText",
        "LeftSpell",
        "RightSpell",
    ] + post
    branch_sequences = []
    for state in (0, 3, 5, 9, 10, 13):
        stages = ordinary + (
            ["BottomSpellMenu"] if state == 5 else []
        ) + post
        branch_sequences.append({
            "key": f"ordinary-{state}",
            "stages": stages,
        })
        branch_sequences.append({
            "key": f"dead-{state}",
            "stages": ["DeadPlayerMenu"] + post,
        })
    for state in (6, 7, 8, 11, 12, 14):
        branch_sequences.append({
            "key": f"map-{state}",
            "stages": map_before_bottom
                + (["BottomSpellMenu"] if state == 8 else [])
                + map_after_bottom,
        })
    return {
        "schema": "mc2-level1-source-frame-census/v1",
        "pinned_remc2_commit": PINNED_REMC2_COMMIT,
        "normative_source_evidence": pinned_source_evidence,
        "qualified_profile": {
            "runtime_captures": runtime_captures,
            "runtime_settings": runtime_profile,
            "reset_source_writes": {
                "LevelInit_56C00": {
                    "blur_capability": 0,
                    "configured_blur_mode": 0,
                    "selected_menu_item": 0,
                    "font_type": 1,
                    "cursor_selector": 9,
                },
                "Initialize": {"setting_byte1": 0},
                "sub_5BCC0": {"show_help": 1},
                "static_runtime_control_block": {"setting_byte4": 0},
                "GetSystemSpeed": {"high_speed_system": 5},
                "initial_player_actual_speed": {
                    "value": runtime_profile["initial_player"]["actual_speed"],
                    "entity_index":
                        runtime_profile["initial_player"]["entity_index"],
                    "source": "level1-export-m players.bin local entity relation -> entities.bin actual_speed",
                },
                "first_frame_conclusion":
                    "blur capability and configured mode are both zero; actual speed zero also prevents automatic mode 1, so the first world frame is the non-temporal MAIN path",
            },
            "fixed_exporter_arguments": [
                "remc2-level-exporter",
                "--mode_test_regressions",
                "0",
                "--text_output_to_console",
                "--set_level",
                "0",
                "--config_file_path",
                "<staged-runtime-config>",
                "--set_max_regressions_steps",
                "0",
            ],
            "debug_command_line_flags": False,
            "mutable_runtime_gates": [
                "menu_state",
                "m_wHighSpeedSystem",
                "blur_scratch_present",
                "blur_capability",
                "configured_blur_mode",
                "viewport_size",
                "actual_speed",
                "setting_byte1_22",
                "setting_byte4_25",
                "showHelp_10",
                "tutorial_state",
                "debug_command_line_flags",
            ],
        },
        "reset_main_history": {
            "source_function": "sub_47FC0_load_screen -> CopyScreen_85B20",
            "source_assets": [
                "owner_data:DATA/SMATITLE.DAT",
                "owner_data:DATA/SMATITLE.PAL",
            ],
            "indices_path": reset_main_output["path"],
            "indices_bytes": reset_main_output["bytes"],
            "indices_sha256": reset_main_output["sha256"],
            "palette_path": reset_palette_output["path"],
            "palette_bytes": reset_palette_output["bytes"],
            "palette_sha256": reset_palette_output["sha256"],
            "dimensions": [320, 200],
            "history_state_on_reset": "Unprimed",
            "observability":
                "loaded MAIN is overwritten by the mandatory first non-blur world frame before temporal history can read it",
            "first_successful_contiguous_frame": "commits MAIN and changes history to Primed",
            "unprimed_temporal_request": "hard_error_without_MAIN_or_history_mutation",
        },
        "postprocess_branch_census": {
            "source": "remc2/engine/GameRenderOriginal.cpp:127-238",
            "configured_mode_saved_and_restored": True,
            "auto_mode_1_gates": [
                "VGA_resolution_equals_1",
                "menu_state_equals_0",
                "m_wHighSpeedSystem_nonzero",
                "scratch_present",
                "viewport_size_equals_40",
                "abs_actual_speed_greater_than_80",
            ],
            "execution_gates": [
                "blur_capability_nonzero",
                "effective_mode_nonzero",
                "scratch_present",
            ],
            "mode_2_not_suppressed_by_menu_or_viewport_auto_gates": True,
            "temporal_and_spatial_smoothing_mutually_exclusive": True,
            "inactive_temporal_path":
                "one world render to MAIN, then optional in-place spatial smoothing",
            "active_temporal_path":
                "one world render to scratch, TABLESN blend into prior MAIN, jump LABEL_44",
            "tablesn_sha256":
                "f88974baf68f728fb534a4dab35397b9cb3ca2f424019bbde36858eebbf97495",
            "ordered_pair_vectors": [
                {"high": 77, "low": 12, "result": 63},
                {"high": 12, "low": 77, "result": 195},
            ],
        },
        "present_boundary_order": [
            {
                "ordinal": 0,
                "call": "sub_84B80",
                "classification": "lighting_preparation_no_direct_MAIN_write",
            },
            {
                "ordinal": 1,
                "call": "sub_58F00_game_objectives",
                "classification": "state_update_no_direct_MAIN_write",
            },
            {
                "ordinal": 2,
                "call": "PresentObjective_59820",
                "classification": "state_update_no_direct_MAIN_write",
            },
            {
                "ordinal": 3,
                "call": "sub_57570",
                "gate": "not_GAME_PAUSED",
                "classification": "state_update_no_direct_MAIN_write",
            },
            {
                "ordinal": 4,
                "call": "sub_575C0",
                "classification": "state_update_no_direct_MAIN_write",
            },
            {
                "ordinal": 5,
                "call": "PlayEntitySounds_6E150",
                "classification": "audio_update_no_direct_MAIN_write",
            },
            {
                "ordinal": 6,
                "call": "DrawGameFrame_2BE30",
                "classification": "MAIN_writer_tree",
            },
            {
                "ordinal": 7,
                "call": "DrawHelpPopUps_871F0",
                "gate": "showHelp_10_nonzero",
                "classification": "MAIN_overlay_writer",
            },
            {
                "ordinal": 8,
                "call": "DrawGameDebugText_6FEC0",
                "gate": "debug_text_flags_and_command_line_profile",
                "classification": "MAIN_overlay_writer_or_noop",
            },
            {
                "ordinal": 9,
                "call": "BlendAndBlit_40F80",
                "classification":
                    "presentation_only_for_qualified_screen_size_0; MAIN_writer_only_for_screen_size_2_half_size_branch",
            },
        ],
        "draw_game_frame_tree": {
            "pre_switch": [
                {
                    "call": "sub_88580",
                    "gate": "showHelp_10_nonzero",
                    "effect":
                        "help/tutorial sentinel preparation; recursive help state is a required later FINAL-MAIN input",
                }
            ],
            "ordinary_menu_states": [0, 3, 5, 9, 10, 13],
            "ordinary_order": [
                "conditional viewport clear",
                "DrawWorld_411A0",
                "dead player menu OR live spider web",
                "ordinary minimap terrain",
                "ordinary minimap entities",
                "ordinary minimap marks",
                "left spell",
                "right spell",
                "top status bar",
                "menu-state overlay",
                "pause/end-level text",
                "bottom spell menu when state 5",
            ],
            "map_menu_states": [6, 7, 8, 11, 12, 14],
            "map_order": [
                "clear MAIN",
                "map terrain",
                "map entities",
                "right-side world viewport",
                "two divider lines",
                "map marks",
                "map menu/scores/pause/options/volume/ok-cancel",
                "pause/end-level text",
                "conditional left and right spells",
            ],
            "post_switch_order": [
                "objective textbox when objective counter nonzero",
                "tutorial/briefing/help text when setting_byte1_22 bit2 set",
            ],
            "unsupported_fixed_profile_branches": [
                "stereo double render",
                "alternate resolution",
                "DAC presentation",
                "hostile or remote player presentation",
            ],
        },
        "overlay_receipt_ledger": {
            "schema": "mc2-level1-overlay-receipt-ledger/v1",
            "authority":
                "pinned remc2 source branch census independent of Rust command emission",
            "source_spans": [
                "remc2/engine/GameUI.cpp:123-610",
                "remc2/engine/GameUI.cpp:4591-5194",
                "remc2/engine/EventsFunctions.cpp:21647-22688",
                "remc2/engine/EventsFunctions.cpp:31851-31866",
                "remc2/engine/EventsFunctions.cpp:44615-44881",
                "remc2/engine/EventsFunctions.cpp:48218-48260",
                "remc2/engine/MenuState.h:5-18",
            ],
            "branch_sequences": branch_sequences,
            "vectors": {
                "spell_tier_reset": [
                    {
                        "hand": "left",
                        "source_x": 510,
                        "fixed_x": 282,
                        "fixed_y": 1,
                        "text": "I",
                        "colour": 61,
                    },
                    {
                        "hand": "right",
                        "source_x": 574,
                        "fixed_x": 314,
                        "fixed_y": 1,
                        "text": "I",
                        "colour": 61,
                    },
                ],
                "spell_menu": {
                    "family_count": 26,
                    "family_selectors": [
                        {
                            "family": family,
                            "small_selector": 97 + family,
                            "tier_selectors": [
                                179 + 3 * family + tier for tier in range(3)
                            ],
                        }
                        for family in range(26)
                    ],
                    "no_holder_panel": 89,
                    "cannot_summon_panel": 91,
                    "source_order": "two rows of thirteen, left-to-right",
                    "profiles": spell_profiles,
                },
                "menu_state_writers": {
                    "ordinary": {
                        "0": ["Pause"],
                        "3": ["Chat", "Pause"],
                        "5": ["Pause"],
                        "9": ["Options"],
                        "10": ["Volume", "Pause"],
                        "13": ["OKCancel"],
                    },
                    "map": {
                        "6": ["Pause"],
                        "7": ["Scores"],
                        "8": ["Pause", "BottomSpellMenu"],
                        "11": ["Options"],
                        "12": ["Volume", "Pause"],
                        "14": ["OKCancel"],
                    },
                    "dead": {
                        "9": ["Options"],
                        "10": ["Volume", "Pause"],
                        "13": ["OKCancel"],
                        "default": ["Pause"],
                    },
                },
                "correction_2_source_vectors": {
                    "palette_indirection": {
                        "source_offsets": [0, 15, 240, 255, 0x888, 3840, 4095],
                        "indexed_values": [61, 23, 171, 87, 177, 119, 176],
                    },
                    "pause": {
                        "selector_order": [174, 175, 176, 177, 178],
                        "fixed_xy": [[134, 38], [134, 54], [134, 70],
                                     [160, 70], [134, 86]],
                        "disabled_shade_passes": 2,
                        "disabled_colour_offset": 0,
                    },
                    "chat": {
                        "entry_text_xy": [5, 167],
                        "caret_xy_offset": [5, 168],
                        "channel_selected": 155,
                        "channel_unselected": 158,
                        "mode_selected": 156,
                        "mode_unselected": 159,
                        "mode_xy": [[41, 180], [76, 180],
                                    [111, 180], [155, 180]],
                        "mode_inner_order": [154, 153, 152, 151],
                        "remote_selected": 157,
                        "remote_unselected": 160,
                        "button_order": [[259, 192, 166],
                                         [261, 237, 166],
                                         [260, 282, 166]],
                        "hover_text_xy_colour_offset": [195, 190, 240],
                    },
                    "options": {
                        "on_ids": [400, 390, 392, 469, 398, 7, 382, 13,
                                   359, 363, 357, 395, 365, 441, 442, 468,
                                   402],
                        "off_ids": [401, 391, 393, 470, 399, 7, 381, 14,
                                    360, 364, 358, 396, 366, 440, 443, 468,
                                    403],
                        "left_x": 109,
                        "row_y_start": 33,
                        "row_y_step": 9,
                        "middle_repeat": 15,
                        "right_x": 202,
                        "ok": [[168, 139, 188], [169, 147, 188],
                               [169, 153, 188], [169, 159, 188],
                               [169, 165, 188], [169, 171, 188],
                               [170, 172, 188]],
                        "text_colour_offsets": {
                            "hover": 3840,
                            "normal": 4095,
                            "unavailable": 0x888,
                        },
                    },
                    "volume": {
                        "source_outer": [268, 204, 102, 24],
                        "source_inner": [272, 208, 94, 16],
                        "source_bar": [274, 210, 90, 12],
                        "background_colour_offset": 0,
                        "foreground_colour_offset": 240,
                        "max_volume": 127,
                    },
                    "ok_cancel": {
                        "localized_ids": [2, 423, 424],
                        "fixed_prompts": ["Abandon level.?", "Load Level?",
                                          "Save Level?"],
                        "ordinary_xy": [66, 25],
                        "map_dead_xy": [3, 3],
                        "button_order": [[257, 135, 52], [258, 160, 52]],
                    },
                    "pause_or_end": {
                        "ordinary_xy": [66, 25],
                        "map_xy": [3, 3],
                        "paused_id": 425,
                        "paused_colour_offset": 240,
                        "paused_advance": 36,
                        "end_ids": [378, 379],
                        "end_text": "Tasks completed. Fly to the exit point..",
                        "end_colour_offset": 3840,
                        "notification_states": {
                            "1": ["name_text", 3840],
                            "2": ["bracket_name_text", 3840],
                            "3": ["text", 3840],
                            "4": ["bracket_name_text", 240],
                            "5": ["local_text", 3840],
                        },
                        "timer_edge": [0, 1, 100, 101],
                    },
                    "map": {
                        "left_size": [191, 200],
                        "scale": 408,
                        "right_viewport": [192, 0, 128, 200],
                        "right_eye_z_add": 128,
                        "divider_order": [192, 191],
                        "divider_colour_offset": 0,
                        "centre_cross": [94, 100, 15, 4095],
                    },
                    "scores": {
                        "large_selector": 85,
                        "mini_selector": 86,
                        "player_x_step": 18,
                        "row_y_step": 19,
                        "requires_all_player_matrix": True,
                        "requires_help_rng_state": True,
                    },
                    "bottom": {
                        "family_count": 26,
                        "slot_origin": [4, 164],
                        "slot_step": [24, 18],
                        "edge_selector": 88,
                        "panel_selected": 87,
                        "panel_unavailable": 89,
                        "panel_cannot": 91,
                        "colourized_tint": 0xA6,
                        "selected_frame": 90,
                        "tier_y": 147,
                        "tier_spacing": 33,
                        "tier_panels": [161, 162, 163],
                        "tier_number_start": 165,
                        "tier_selected": 164,
                        "xp_background_offset": 0,
                        "xp_foreground_offset": 3840,
                        "xp_source_width": 54,
                    },
                    "debug": {
                        "flag8_x": 160,
                        "flag40_xy": [5, 42],
                        "line_step": 7,
                        "label_colour_offset": 3840,
                        "value_colour_offset": 15,
                        "selected_value_colour_offset": 255,
                        "version": "Beta",
                        "version_date": "03:42:13 Sep 06 1995",
                        "tape_size": 38547,
                        "carpet_size": 224791,
                    },
                    "cursor": {
                        "selector": 9,
                        "msp_dimensions": [5, 1],
                        "pointer_dimensions": [12, 13],
                        "active_bank_proven": False,
                        "active_policy": "fail_closed",
                    },
                },
                "unsupported_prerequisites": {
                    "spider": "typed web sprite sheet absent",
                    "tutorial": "sentinel stream off_DB558 absent",
                    "help": "recursive E2A74 textbox state absent",
                },
            },
        },
        "ui_mechanical_census": {
            "fixed_source_bitmap_bank": {
                "name": "MSPRN0-0",
                "selectors": [
                    asset["selector"] for asset in source_bitmap_assets
                ],
                "assets": source_bitmap_assets,
            },
            "adapted_bitmap_bank": {
                "name": "HSPRN0-0",
                "selectors": [
                    asset["selector"] for asset in adapted_bitmap_assets
                ],
                "assets": adapted_bitmap_assets,
            },
            "fixed_source_font": source_font,
            "adapted_font": adapted_font,
            "cursor": cursor_asset,
            "selector_groups": {
                "chat_players_and_tiles": list(range(74, 88)),
                "spell_menu_chrome": list(range(88, 92))
                + list(range(149, 179)),
                "spell_family_small_formula_97_plus_family":
                    list(SPELL_SMALL_HSPRN),
                "spell_family_big_formula_123_plus_family":
                    list(SPELL_BIG_HSPRN),
                "spell_tier_formula_179_plus_3_family_plus_tier":
                    list(SPELL_TIER_HSPRN),
                "ok_cancel_back": list(range(257, 262)),
                "top_bar_and_objective": [
                    1, 2, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49,
                    50, 51, 52, 53, 54, 55, 58, 66, 83, 84, 123,
                    124, 125, 126, 171, 172, 173,
                ],
            },
            "calls": [
                {
                    "call": "DrawChatMenu_2F6B0",
                    "selectors": list(range(74, 88))
                    + list(range(151, 161))
                    + [259, 260, 261],
                    "strings":
                        "live player names/chat buffer, decimal player numbers, wizard names and hovered wizard name",
                    "formats": ["%s", "%d"],
                    "primitives":
                        "Night building colours, CLRN/PALN palette entries, indexed lines, FONT1/HFONT3 text and cursor",
                },
                {
                    "call": "DrawInGameOptionsMenu_30050",
                    "selectors": list(range(168, 171)),
                    "strings":
                        "17 sub_1A5B0_getLangStrings dynamic option labels plus literal OK",
                    "state":
                        "enabled/current bitfields, live mouse hover and fixed 320 source dimensions",
                    "primitives":
                        "panel edge/centre pieces, palette 4095/3840/0x888 text and cursor",
                },
                {
                    "call": "DrawOkCancelMenu_30A60",
                    "selectors": [257, 258],
                    "language_ids": [2, 423, 424],
                    "strings":
                        "selected Abandon/Load/Save prompt formatted as %s?",
                    "cursor":
                        "software DrawBitmap uses MSPRN selector 9; SetCursor uses POINTERS selector 9",
                },
                {
                    "call": "DrawPauseMenu_2FD90",
                    "selectors": list(range(174, 179)),
                    "gate": "GAME_PAUSED",
                    "nested_gate":
                        "setting_byte4_25 bit0x10 invokes sub_30870 after pause icons",
                },
                {
                    "call": "DrawVolumeSettings_303D0",
                    "selectors": [176, 177],
                    "state":
                        "byte_38591 selects soundVolume_6 or musicVolume_8, each scaled over 127",
                    "primitives":
                        "Night building colours plus palette 0/240 indexed volume bars",
                },
                {
                    "call": "sub_30870",
                    "gate": "setting_byte4_25 bit0x10",
                    "language_extent": [1, 470],
                    "strings":
                        "selected language record number plus recursive textbox words",
                    "primitives":
                        "indexed black field, FONT1/HFONT3 text and palette 3840/0",
                },
                {
                    "call": "DrawBottomSpellsMenu_2ECC0",
                    "selectors": [88, 89, 90, 91, 149, 150]
                    + list(SPELL_SMALL_HSPRN)
                    + list(range(155, 179))
                    + list(SPELL_TIER_HSPRN),
                    "state":
                        "2x13 families, enabled/locked/current tiers, hand corners, live mana shot bars",
                },
                {
                    "call": "DrawSorcererScores_2D1D0",
                    "selectors": [85, 86],
                    "strings":
                        "live wizard names, decimal max mana and zero-padded %03d pair scores",
                    "state":
                        "alive-player rows, player colours, help sentinel selection and source RNG update",
                    "primitives":
                        "SPELL_TILE/SPELL_TILE_MINI plus indexed player-colour rectangles",
                },
                {
                    "call": "DrawTextPauseEndOfLevel_2CE30",
                    "language_ids": [378, 379, 425],
                    "strings":
                        "Paused!, Tasks completed/Fly to exit, wizard notifications and selected spell/upgrade text",
                    "state":
                        "pause, level-end, notification type/countdown and selected-menu gates",
                },
                {
                    "call": "DrawHelpPopUps_871F0",
                    "strings":
                        "prior tutorial/help sentinel recursion and complete 471-record Level 1 language table",
                    "state":
                        "reachable source state; native bridge fails closed before COMPLETE until cached E2A74 textbox state is typed",
                },
                {
                    "call": "DrawGameFrame_2BE30 tutorial/briefing block",
                    "gate": "setting_byte1_22 bit2; fixed profile reset is zero",
                    "strings":
                        "off_DB558 sentinel-separated lines with first-line palette 4095 and following palette 0",
                    "qualification":
                        "reachable source state; native bridge fails closed before COMPLETE until the live sentinel-separated stream is typed",
                },
                {
                    "call": "DrawGameDebugText_6FEC0",
                    "strings": "command-line/debug generated values",
                    "gate":
                        "qualified exporter command line has no in-game debug flag",
                },
            ],
            "localization": {
                "record_count": len(language_rows),
                "records_path": language_output["path"],
                "records_bytes": language_output["bytes"],
                "records_sha256": language_output["sha256"],
                "rows": language_rows,
            },
        },
        "source_tuple_spawn_census": source_tuple_census,
        "native_final_main_contract": {
            "schema": "mc2-level1-native-final-main-contract/v1",
            "classification": "Exact",
            "source_commit": PINNED_REMC2_COMMIT,
            "profile": {
                "level": 1,
                "environment": "Night",
                "framebuffer": "320x200 indexed MAIN",
                "players": "single local player",
                "screen_size": 0,
                "stereo": False,
            },
            "fail_closed_reachable_states": [
                "active tutorial/briefing",
                "recursive help popup",
                "immobilization spider-web writer",
                "menu-state-7 score writer",
            ],
            "source_anchors": [
                "remc2/engine/GameRenderOriginal.cpp:9-238 DrawWorld_411A0 definition and temporal/spatial postprocess",
                "remc2/engine/GameRenderOriginal.cpp:810-1843 world cell/reflection/primary traversal",
                "remc2/engine/GameRenderOriginal.cpp:1855-3049 source sprite projection and raster",
                "remc2/engine/GameRenderOriginal.cpp:3050-4033 linked sprite/shadow traversal and normal hotspot",
                "remc2/engine/GameRenderOriginal.cpp:4924-4978 special hotspot and clipping",
                "remc2/engine/EventsFunctions.cpp:21496-22062,31823-31823 DrawGameFrame_2BE30 definition and call",
                "remc2/engine/EventsFunctions.cpp:31798-31816,47260-47720 dynamic-light calls and definitions",
                "remc2/engine/GameUI.cpp:123-610,4591-5194 fixed-profile overlays",
                "remc2/engine/EventsFunctions.cpp:21647-22688 fixed-profile map, bottom, and end overlays",
                "remc2/engine/EventsFunctions.cpp:31851-31866,44615-44881,48218-48260 help/debug gates, calls, and definitions",
            ],
            "ordered_closure": [
                "typed source occupant census: Render, Excluded, or hard error",
                "source linked map-chain order retained across excluded occupants",
                "per cell: reflection terrain and linked reflection chain before primary terrain and linked primary chain",
                "per entity: terrain shadow immediately before primary sprite",
                "normal and special hotspot, mirror, roll, fixed-point clipping phase, and edge truncation",
                "subtract prior dynamic-light contributions, update entities, retire/move/recompute lights, then render",
                "render world once, apply exact temporal-or-spatial TABLESN postprocess, then supported source-ordered overlays; active tutorial/briefing, recursive help, immobilization spider web, and menu-state-7 scores fail closed",
                "atomically validate execution receipts and commit one generation/tick FINAL MAIN plus provenance",
                "copy the revisioned C ABI frame and upload one row-flipped point-filtered R8 plane in Unity",
            ],
            "required_complete_flags": [
                "MC_PRESENTATION_FRAME_COMPOSITOR_COMPLETE",
                "MC_PRESENTATION_FRAME_HAS_PROVENANCE",
                "MC_PRESENTATION_FRAME_OVERLAYS_COMPOSITED",
                "MC_PRESENTATION_FRAME_POSTPROCESS_COMPLETE",
                "MC_PRESENTATION_FRAME_LIGHTING_COMPLETE",
            ],
            "forbidden_complete_flag": "MC_TERRAIN_FRAME_NON_TERRAIN_OMITTED",
            "implementation": {
                "source_tuple_and_tmap": "sim-core/source_sprite_tmap.rs",
                "map_chain": "sim-core/renderable_map_chain.rs",
                "world_raster": "sim-core/terrain_frame.rs + native_world.rs",
                "lighting": "sim-core/dynamic_lighting.rs",
                "postprocess": "sim-core/indexed_postprocess.rs",
                "overlay": "sim-core/indexed_overlay.rs + sim-capi/presentation_bridge.rs",
                "atomic_finalization": "sim-core/presentation_frame.rs + sim-capi/native_v2.rs",
                "unity": "NativeSession + MagicCarpetRuntime current-COMPLETE ownership",
            },
            "explicit_nonclaims": [
                "cross-timeline pixel equality",
                "cross-timeline clock equality",
                "active-DAC RGB for historical captures",
                "stereo or alternate-resolution presentation",
                "hostile or remote player presentation",
                "unsupported source sprite or overlay branches",
                "active tutorial/briefing is reachable but fail-closed before COMPLETE",
                "recursive help popup is reachable but fail-closed before COMPLETE",
                "immobilization spider-web writer is reachable but fail-closed before COMPLETE",
                "menu-state-7 score writer is reachable but fail-closed before COMPLETE",
            ],
        },
        "checkpoint_status": {
            "source_frame_census_complete": True,
            "final_main_implementation":
                "accepted_fixed_profile_native_FINAL_MAIN",
            "correspondence_layers": [
                "sprite.source_hotspot_anchor",
                "renderer.dos_depth_interleaving_aspect",
            ],
            "unknown_reachable_state_policy": "hard_error_and_no_COMPLETE_flag",
        },
    }


def build_minimap_aperture(
    tables: bytes,
) -> tuple[bytes, list[dict[str, int]]]:
    if len(tables) != 83456:
        raise CacheError("TABLESN extent changed before minimap aperture decode")
    side = LEVEL1_MINIMAP_LOGICAL_SIDE
    half = side // 2
    phase = 0
    phase_step = 0x1000000 // half
    bounds: list[tuple[int, int] | None] = [None] * side
    for index in range(half):
        table_index = 0x14300 + (phase >> 16)
        half_span = (half * tables[table_index]) >> 8
        row = (half - half_span, half + half_span)
        bounds[half + index] = row
        bounds[half - 1 - index] = row
        phase += phase_step
    if any(row is None for row in bounds):
        raise CacheError("minimap aperture left an uninitialized scanline")
    scanlines = [
        {"y": y, "start_x_inclusive": row[0], "end_x_exclusive": row[1]}
        for y, row in enumerate(bounds)
        if row is not None
    ]
    mask = bytes(
        1 if row[0] <= x < row[1] else 0
        for row in bounds
        if row is not None
        for x in range(side)
    )
    if len(mask) != side * side:
        raise CacheError("qualified 128x128 TABLESN minimap aperture extent changed")
    return mask, scanlines


def load_remc2_sine_table() -> tuple[list[int], bytes]:
    text = pinned_remc2_blob(REMC2_MATHS_TABLE_SOURCE).decode("utf-8")
    declaration = "int32_t Maths::sin_DB750[2560]"
    start = text.index("{", text.index(declaration)) + 1
    end = text.index("};", start)
    tokens = re.findall(r"0x[0-9A-Fa-f]+|(?<![A-Za-z_])-?\d+", text[start:end])
    if len(tokens) != 2560:
        raise CacheError("remc2 Maths::sin_DB750 extent changed")
    values = []
    for token in tokens:
        value = int(token, 0)
        if value >= 0x80000000:
            value -= 0x100000000
        values.append(value)
    payload = struct.pack("<2560i", *values)
    if (
        values[0] != 0
        or values[512] != 65536
        or values[1024] != 0
        or values[1536] != -65536
        or values[2048] != 0
    ):
        raise CacheError("remc2 Maths::sin_DB750 cardinal values changed")
    return values, payload


def trunc_div(numerator: int, denominator: int) -> int:
    if denominator <= 0:
        raise CacheError("positive minimap denominator required")
    return (
        numerator // denominator
        if numerator >= 0
        else -((-numerator) // denominator)
    )


def minimap_source_fixed(
    player_x: int,
    player_y: int,
    yaw: int,
    screen_x: int,
    screen_y: int,
    sine_table: list[int],
) -> tuple[int, int]:
    width = LEVEL1_MINIMAP_LOGICAL_SIDE
    height = LEVEL1_MINIMAP_LOGICAL_SIDE
    scaling = LEVEL1_MINIMAP_SOURCE_SCALE
    angle = yaw & 0x7FF
    sine_scaled = (scaling * sine_table[angle]) >> 16
    cosine_scaled = (scaling * sine_table[angle + 0x200]) >> 16
    column_x = trunc_div(height * cosine_scaled, width)
    column_y = trunc_div(height * sine_scaled, width)
    origin_x = player_x - trunc_div(
        width * column_x - height * sine_scaled, 2
    )
    origin_y = player_y - trunc_div(
        height * cosine_scaled + width * column_y, 2
    )
    return (
        origin_x - screen_y * sine_scaled + screen_x * column_x,
        origin_y + screen_y * cosine_scaled + screen_x * column_y,
    )


def build_minimap_direction_samples(
    player_x: int,
    player_y: int,
    terrain_type: bytes,
    terrain_shade: bytes,
    tables: bytes,
    sine_table: list[int],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    centre = LEVEL1_MINIMAP_LOGICAL_SIDE // 2
    quarter = LEVEL1_MINIMAP_LOGICAL_SIDE // 4
    points = (
        ("top", centre, quarter),
        ("right", centre + quarter, centre),
        ("bottom", centre, centre + quarter),
        ("left", quarter, centre),
    )
    rows = []
    for yaw in (0, 512, 1024, 1536):
        for label, screen_x, screen_y in points:
            fixed_x, fixed_y = minimap_source_fixed(
                player_x, player_y, yaw, screen_x, screen_y, sine_table
            )
            source_x = fixed_x & 0xFFFF
            source_y = fixed_y & 0xFFFF
            tile_x = source_x >> 8
            tile_y = source_y >> 8
            tile = tile_x + tile_y * 256
            terrain_base = tables[0x14000 + terrain_type[tile]]
            palette_index = tables[(terrain_shade[tile] << 8) | terrain_base]
            rows.append({
                "yaw": yaw,
                "screen_label": label,
                "screen_x": screen_x,
                "screen_y": screen_y,
                "source_fixed_x": source_x,
                "source_fixed_y": source_y,
                "source_tile_x": tile_x,
                "source_tile_y": tile_y,
                "terrain_type": terrain_type[tile],
                "terrain_shade": terrain_shade[tile],
                "terrain_base_index": terrain_base,
                "palette_index": palette_index,
            })
    probe_rows = []
    for yaw in range(0, 2048, 256):
        fixed_x, fixed_y = minimap_source_fixed(
            player_x,
            player_y,
            yaw,
            21 * LEVEL1_MINIMAP_LOGICAL_SIDE // 32,
            11 * LEVEL1_MINIMAP_LOGICAL_SIDE // 32,
            sine_table,
        )
        probe_rows.append({
            "yaw": yaw,
            "screen_x": 21 * LEVEL1_MINIMAP_LOGICAL_SIDE // 32,
            "screen_y": 11 * LEVEL1_MINIMAP_LOGICAL_SIDE // 32,
            "source_fixed_x": fixed_x & 0xFFFF,
            "source_fixed_y": fixed_y & 0xFFFF,
            "source_tile_x": (fixed_x >> 8) & 255,
            "source_tile_y": (fixed_y >> 8) & 255,
        })
    return rows, probe_rows


def draw_mask_rgba(
    canvas: bytearray,
    canvas_width: int,
    canvas_height: int,
    source: bytes,
    width: int,
    height: int,
    x: int,
    y: int,
    scale: int,
    rgba: tuple[int, int, int, int],
) -> None:
    for source_y in range(height):
        for source_x in range(width):
            if not source[source_y * width + source_x]:
                continue
            for scale_y in range(scale):
                target_y = y + source_y * scale + scale_y
                if target_y < 0 or target_y >= canvas_height:
                    continue
                for scale_x in range(scale):
                    target_x = x + source_x * scale + scale_x
                    if target_x < 0 or target_x >= canvas_width:
                        continue
                    offset = (target_y * canvas_width + target_x) * 4
                    canvas[offset:offset + 4] = bytes(rgba)


def draw_indexed_rgba(
    canvas: bytearray,
    canvas_width: int,
    canvas_height: int,
    indices: bytes,
    mask: bytes,
    width: int,
    height: int,
    x: int,
    y: int,
    scale: int,
    palette_rgb8: bytes,
) -> None:
    for source_y in range(height):
        for source_x in range(width):
            source_offset = source_y * width + source_x
            if not mask[source_offset]:
                continue
            palette_offset = indices[source_offset] * 3
            rgba = (
                palette_rgb8[palette_offset],
                palette_rgb8[palette_offset + 1],
                palette_rgb8[palette_offset + 2],
                255,
            )
            draw_mask_rgba(
                canvas,
                canvas_width,
                canvas_height,
                b"\1",
                1,
                1,
                x + source_x * scale,
                y + source_y * scale,
                scale,
                rgba,
            )


def draw_indexed_mask(
    destination_indices: bytearray,
    destination_mask: bytearray,
    destination_width: int,
    destination_height: int,
    source_indices: bytes,
    source_mask: bytes,
    source_width: int,
    source_height: int,
    x: int,
    y: int,
    scale: int,
) -> None:
    for source_y in range(source_height):
        for source_x in range(source_width):
            source_offset = source_y * source_width + source_x
            if not source_mask[source_offset]:
                continue
            for scale_y in range(scale):
                target_y = y + source_y * scale + scale_y
                if target_y < 0 or target_y >= destination_height:
                    continue
                for scale_x in range(scale):
                    target_x = x + source_x * scale + scale_x
                    if target_x < 0 or target_x >= destination_width:
                        continue
                    target_offset = target_y * destination_width + target_x
                    destination_indices[target_offset] = source_indices[source_offset]
                    destination_mask[target_offset] = 1


def draw_solid_index_mask(
    destination_indices: bytearray,
    destination_mask: bytearray,
    destination_width: int,
    destination_height: int,
    source_mask: bytes,
    source_width: int,
    source_height: int,
    x: int,
    y: int,
    scale: int,
    palette_index: int,
) -> None:
    draw_indexed_mask(
        destination_indices,
        destination_mask,
        destination_width,
        destination_height,
        bytes([palette_index]) * (source_width * source_height),
        source_mask,
        source_width,
        source_height,
        x,
        y,
        scale,
    )


def split_text_into_source_lines(text: bytes, width: int) -> list[bytes]:
    words = text.split(b" ")
    lines: list[bytes] = []
    line = b""
    for word in words:
        if not line:
            line = word
        elif len(line) + len(word) + 1 <= width:
            line += b" " + word
        else:
            lines.append(line)
            line = word
    if line:
        lines.append(line)
    return lines


def build_objective_rasters(
    text: bytes,
    glyph_rows: list[dict[str, Any]],
    font_blob: bytes,
    sprites: dict[int, tuple[bytes, bytes, int, int]],
    palette_rgb8: bytes,
    text_palette_index: int,
    source_scale: int,
) -> tuple[bytes, bytes, dict[str, Any]]:
    if not text:
        raise CacheError("Level 1 objective text must not be empty")
    scale = source_scale
    char_width = 8 * scale
    char_height = 14 * scale
    text_width = char_width * len(text)
    textbox_width = 320 * scale
    if text_width <= textbox_width:
        textbox_width = text_width
    else:
        if text_width % textbox_width < textbox_width // 2:
            while (
                textbox_width > 0
                and text_width % textbox_width < textbox_width // 2
            ):
                textbox_width -= char_width
        if textbox_width <= 0:
            raise CacheError("objective textbox source width collapsed")
    lines = split_text_into_source_lines(text, textbox_width // char_width)
    textbox_height = char_height * len(lines)

    text_x = 320 * scale - textbox_width // 2
    text_y = 300 * scale - textbox_height // 2
    corner = sprites[171]
    vertical = sprites[172]
    horizontal = sprites[173]
    corner_width = corner[2] * scale
    corner_height = corner[3] * scale
    if text_x < 40 * scale + corner_width:
        text_x = 40 * scale + corner_width
    if text_y < 80 * scale + corner_height:
        text_y = 80 * scale + corner_height
    if text_x + textbox_width >= 600 * scale:
        text_x = 600 * scale - textbox_width
    if text_y + textbox_height >= 260 * scale:
        text_y = 260 * scale - textbox_height

    horizontal_width = horizontal[2] * scale
    horizontal_height = horizontal[3] * scale
    vertical_height = vertical[3] * scale
    frame_width = horizontal_width * (
        (
            horizontal_width
            - scale
            + textbox_width
            + 8 * scale
        )
        // horizontal_width
    )
    frame_x = text_x - (frame_width - textbox_width) // 2
    frame_height = (
        (vertical_height - scale + textbox_height)
        // vertical_height
        * vertical_height
        + 2 * (corner_height - horizontal_height)
    )
    frame_y = text_y - (frame_height - textbox_height) // 2

    outer_x = frame_x - vertical[2] * scale
    outer_y = frame_y - horizontal_height
    objective_width = frame_width + 2 * vertical[2] * scale
    objective_height = frame_height + 2 * horizontal_height
    text_local_x = text_x - outer_x
    text_local_y = text_y - outer_y
    blend_local_x = frame_x - outer_x
    blend_local_y = frame_y - outer_y

    line_advances = [
        sum(
            glyph_rows[33 if value == 32 else value + 1]["width"] * source_scale
            for value in row
        )
        for row in lines
    ]
    foreground_indices = bytearray(objective_width * objective_height)
    foreground_mask = bytearray(objective_width * objective_height)
    glyph_sequence = []
    for line_index, text_line in enumerate(lines):
        cursor_x = text_local_x
        for value in text_line:
            glyph_index = value + 1
            row = glyph_rows[glyph_index]
            glyph_sequence.append(glyph_index)
            if value != 32:
                start = row["offset"]
                draw_solid_index_mask(
                    foreground_indices,
                    foreground_mask,
                    objective_width,
                    objective_height,
                    font_blob[start:start + row["length"]],
                    row["width"],
                    row["height"],
                    cursor_x,
                    text_local_y + line_index * char_height,
                    scale,
                    text_palette_index,
                )
            cursor_x += (
                glyph_rows[33 if value == 32 else glyph_index]["width"] * scale
            )

    frame_end_x = blend_local_x + frame_width
    frame_top_y = blend_local_y - horizontal_height
    frame_bottom_y = blend_local_y + frame_height
    frame_side_bottom_y = (
        frame_top_y
        + corner_height
        + len(lines) * vertical_height
    )
    for x in (blend_local_x - vertical[2] * scale, frame_end_x):
        draw_indexed_mask(
            foreground_indices,
            foreground_mask,
            objective_width,
            objective_height,
            *corner,
            x,
            frame_top_y,
            scale,
        )
    side_y = frame_top_y + corner_height
    for _ in lines:
        for x in (blend_local_x - vertical[2] * scale, frame_end_x):
            draw_indexed_mask(
                foreground_indices,
                foreground_mask,
                objective_width,
                objective_height,
                *vertical,
                x,
                side_y,
                scale,
            )
        side_y += vertical_height
    if side_y != frame_side_bottom_y:
        raise CacheError("objective frame side clock changed")
    for x in (blend_local_x - vertical[2] * scale, frame_end_x):
        draw_indexed_mask(
            foreground_indices,
            foreground_mask,
            objective_width,
            objective_height,
            *corner,
            x,
            frame_side_bottom_y,
            scale,
        )
    for x in range(
        blend_local_x,
        frame_end_x,
        horizontal_width,
    ):
        for y in (frame_top_y, frame_bottom_y):
            draw_indexed_mask(
                foreground_indices,
                foreground_mask,
                objective_width,
                objective_height,
                *horizontal,
                x,
                y,
                scale,
            )

    palette_offset = text_palette_index * 3
    text_rgba = (
        palette_rgb8[palette_offset],
        palette_rgb8[palette_offset + 1],
        palette_rgb8[palette_offset + 2],
        255,
    )

    badge_text = b"FROZEN OBSERVED"
    badge_width = sum(
        glyph_rows[33 if value == 32 else value + 1]["width"]
        for value in badge_text
    )
    badge_height = max(glyph_rows[value + 1]["height"] for value in badge_text if value != 32)
    badge = bytearray(badge_width * badge_height * 4)
    cursor_x = 0
    badge_glyphs = []
    for value in badge_text:
        glyph_index = value + 1
        badge_glyphs.append(glyph_index)
        row = glyph_rows[glyph_index]
        if value != 32:
            start = row["offset"]
            draw_mask_rgba(
                badge,
                badge_width,
                badge_height,
                font_blob[start:start + row["length"]],
                row["width"],
                row["height"],
                cursor_x,
                0,
                1,
                text_rgba,
            )
        cursor_x += glyph_rows[33 if value == 32 else glyph_index]["width"]
    foreground = bytes(foreground_indices + foreground_mask)
    return foreground, bytes(badge), {
        "objective_width": objective_width,
        "objective_height": objective_height,
        "objective_x": outer_x,
        "objective_y": outer_y,
        "objective_text_origin": {"x": text_x, "y": text_y},
        "objective_text_local_origin": {
            "x": text_local_x,
            "y": text_local_y,
        },
        "objective_blend_rect": {
            "x": frame_x,
            "y": frame_y,
            "width": frame_width,
            "height": frame_height,
        },
        "objective_blend_local_rect": {
            "x": blend_local_x,
            "y": blend_local_y,
            "width": frame_width,
            "height": frame_height,
        },
        "objective_foreground_plane_bytes":
            objective_width * objective_height,
        "objective_foreground_indices_sha256":
            sha256_bytes(bytes(foreground_indices)),
        "objective_foreground_mask_sha256":
            sha256_bytes(bytes(foreground_mask)),
        "objective_foreground_opaque_pixels": sum(foreground_mask),
        "objective_glyph_indices": glyph_sequence,
        "objective_text_advances": line_advances,
        "objective_lines_ascii": [line.decode("ascii") for line in lines],
        "badge_text_ascii": badge_text.decode("ascii"),
        "badge_width": badge_width,
        "badge_height": badge_height,
        "badge_glyph_indices": badge_glyphs,
    }


def build_hud_text_raster(
    text: bytes,
    glyph_rows: list[dict[str, Any]],
    font_blob: bytes,
    palette_rgb8: bytes,
    palette_index: int,
) -> tuple[bytes, int, int, list[int]]:
    width = sum(
        glyph_rows[33 if value == 32 else value + 1]["width"]
        for value in text
    )
    height = max(
        glyph_rows[value + 1]["height"]
        for value in text
        if value != 32
    )
    rgba = bytearray(width * height * 4)
    palette_offset = palette_index * 3
    tint = (
        palette_rgb8[palette_offset],
        palette_rgb8[palette_offset + 1],
        palette_rgb8[palette_offset + 2],
        255,
    )
    cursor_x = 0
    glyphs = []
    for value in text:
        glyph_index = value + 1
        glyphs.append(glyph_index)
        row = glyph_rows[glyph_index]
        if value != 32:
            start = row["offset"]
            draw_mask_rgba(
                rgba,
                width,
                height,
                font_blob[start:start + row["length"]],
                row["width"],
                row["height"],
                cursor_x,
                0,
                1,
                tint,
            )
        cursor_x += glyph_rows[33 if value == 32 else glyph_index]["width"]
    return bytes(rgba), width, height, glyphs


def tmap_records(tab: bytes) -> list[tuple[int, int, int]]:
    if len(tab) % TMAP_TAB_RECORD.size:
        raise CacheError("TMAPS TAB is truncated or not the proven ten-byte <iiH schema")
    return list(TMAP_TAB_RECORD.iter_unpack(tab))


def build_entity_presentation_descriptor(
    entities_blob: bytes,
    presentation_blob: bytes,
    terrain_blob: bytes,
    tmap_rows: list[dict[str, Any]],
) -> dict[str, Any]:
    if len(entities_blob) < ENTITY_HEADER.size:
        raise CacheError("entities.bin header is truncated")
    magic, version, record_size, count = ENTITY_HEADER.unpack_from(entities_blob)
    if magic != b"MCE1" or version != 1 or record_size != 176 or count != 1000:
        raise CacheError("entities.bin is not the qualified MCE1/1 1000x176 form")
    if len(entities_blob) != ENTITY_HEADER.size + record_size * count:
        raise CacheError("entities.bin record extent changed")
    if len(terrain_blob) != 458752:
        raise CacheError("terrain.bin extent changed")
    terrain_head_values = struct.unpack_from("<65536h", terrain_blob, 5 * 65536)
    terrain_heads = {value for value in terrain_head_values if value > 0}
    if len(terrain_heads) != 130:
        raise CacheError("qualified Level 1 terrain billboard head count changed")
    if len(presentation_blob) < PRESENTATION_HEADER.size:
        raise CacheError("presentation.bin header is truncated")
    pmagic, pversion, reserved, particle_count = PRESENTATION_HEADER.unpack_from(
        presentation_blob
    )
    if (
        pmagic != b"MCV1"
        or pversion != 1
        or reserved != 0
        or particle_count != 347
    ):
        raise CacheError("presentation.bin is not the qualified MCV1/1 form")
    particle_start = PRESENTATION_HEADER.size
    particle_end = particle_start + particle_count * PARTICLE_RECORD.size
    if particle_end + 4 > len(presentation_blob):
        raise CacheError("presentation particle table is truncated")
    particles = [
        PARTICLE_RECORD.unpack_from(
            presentation_blob, particle_start + index * PARTICLE_RECORD.size
        )
        for index in range(particle_count)
    ]
    selector_count = struct.unpack_from("<I", presentation_blob, particle_end)[0]
    selector_end = particle_end + 4 + selector_count
    if selector_count != 504 or selector_end + 4 > len(presentation_blob):
        raise CacheError("presentation selector table changed")
    parameter_count = struct.unpack_from("<I", presentation_blob, selector_end)[0]
    if parameter_count != 157 or selector_end + 4 + parameter_count * 34 != len(
        presentation_blob
    ):
        raise CacheError("presentation parameter table changed")

    raw_chain_entries = []
    raw_membership: set[int] = set()
    for tile, head in enumerate(terrain_head_values):
        if head <= 0:
            continue
        source_entity = head
        local_membership: set[int] = set()
        ordinal = 0
        while source_entity != 0:
            if not 0 < source_entity < count:
                raise CacheError(
                    f"raw terrain tile {tile} links invalid entity {source_entity}"
                )
            if source_entity in local_membership:
                raise CacheError(
                    f"raw terrain tile {tile} contains a cycle at {source_entity}"
                )
            if source_entity in raw_membership:
                raise CacheError(
                    f"raw entity {source_entity} occurs in multiple terrain chains"
                )
            record_start = ENTITY_HEADER.size + source_entity * record_size
            record = entities_blob[record_start:record_start + record_size]
            if record[2] == 0:
                raise CacheError(
                    f"raw terrain tile {tile} links inactive entity {source_entity}"
                )
            local_membership.add(source_entity)
            raw_membership.add(source_entity)
            raw_chain_entries.append(
                {
                    "tile": tile,
                    "ordinal": ordinal,
                    "source_entity_index": source_entity,
                }
            )
            ordinal += 1
            source_entity = struct.unpack_from("<H", record, 22)[0]
    if len(raw_chain_entries) != 130:
        raise CacheError("qualified raw Level 1 map-chain extent changed")

    texture_rows = {row["index"]: row for row in tmap_rows}
    grouped: dict[tuple[int, int, int, int | None, int, int, int], list[int]] = {}
    for index in range(count):
        start = ENTITY_HEADER.size + index * record_size
        record = entities_blob[start:start + record_size]
        if record[2] == 0:
            continue
        class_id = record[67]
        model = record[68]
        action = record[73]
        parameter_relation = struct.unpack_from("<I", record, 164)[0]
        parameter_index = parameter_relation - 1 if parameter_relation else None
        particle_index = struct.unpack_from("<h", record, 94)[0]
        animation_frame = struct.unpack_from("<b", record, 96)[0]
        control_flags = struct.unpack_from("<I", record, 16)[0]
        key = (
            class_id,
            model,
            action,
            parameter_index,
            particle_index,
            animation_frame,
            control_flags,
        )
        grouped.setdefault(key, []).append(index)
    if len(grouped) != 17 or sum(len(indices) for indices in grouped.values()) != 177:
        raise CacheError("qualified Level 1 active reset grouping changed")
    if set(grouped) != set(RESET_SOURCE_TUPLE_DISPOSITIONS):
        missing = sorted(set(RESET_SOURCE_TUPLE_DISPOSITIONS) - set(grouped))
        unexpected = sorted(set(grouped) - set(RESET_SOURCE_TUPLE_DISPOSITIONS))
        raise CacheError(
            "qualified reset source tuple closure changed: "
            f"missing={missing} unexpected={unexpected}"
        )

    groups: list[dict[str, Any]] = []
    actor_total = 0
    effect_total = 0
    suppressed_head_total = 0
    off_chain_total = 0
    for key, indices in sorted(grouped.items(), key=lambda item: item[1][0]):
        (
            class_id,
            model,
            action,
            parameter_index,
            particle_index,
            animation_frame,
            control_flags,
        ) = key
        source_disposition, source_semantic = RESET_SOURCE_TUPLE_DISPOSITIONS[key]
        if particle_index < 0 or particle_index >= len(particles):
            raise CacheError(f"entity group has invalid particle index {particle_index}")
        (
            base_selector,
            word_2,
            word_4,
            speed,
            rotation_speed,
            raster_table_selector_offset,
            signed_byte_11,
            render_mode,
            byte_13,
        ) = particles[particle_index]
        actor_spec = ENTITY_ACTOR_SPECS.get(
            (class_id, model, action, parameter_index, particle_index)
        )
        effect_spec = ENTITY_EFFECT_SPECS.get(
            (class_id, model, action, parameter_index, particle_index)
        )
        presentation_spec = actor_spec or effect_spec
        terrain_head_indices = [index for index in indices if index in terrain_heads]
        visible_indices = (
            terrain_head_indices
            if control_flags & 0x21 == 0 and presentation_spec is not None
            else []
        )
        suppressed_head_indices = (
            terrain_head_indices if control_flags & 0x21 else []
        )
        off_chain_indices = [index for index in indices if index not in terrain_heads]
        suppressed_head_total += len(suppressed_head_indices)
        off_chain_total += len(off_chain_indices)
        qualified_view_indices: list[int] = []
        texture_animated = False
        texture_frame_count = 0
        authorities = {
            (
                "PlayerFlightLive"
                if index == 152
                else "SpellActivationLive"
                if index == 153
                else "FrozenObserved"
            )
            for index in indices
        }
        if len(authorities) != 1:
            raise CacheError("entity presentation group mixes authority domains")
        authority = next(iter(authorities))
        if presentation_spec is not None:
            family, expected_base, expected_mode, expected_rotation = presentation_spec
            if (
                base_selector != expected_base
                or render_mode != expected_mode
                or rotation_speed != expected_rotation
            ):
                raise CacheError(f"qualified billboard presentation changed for {family}")
            if render_mode == 20:
                selectors = [
                    base_selector + value
                    for value in sorted(set(YAW_LOOKUP_D4750_28))
                ]
            elif 2 <= render_mode <= 16 or 22 <= render_mode <= 36:
                selectors = [base_selector + animation_frame]
            else:
                selectors = [base_selector]
            for selector in selectors:
                texture = texture_rows.get(selector)
                if (
                    texture is None
                    or texture.get("status") != "decoded_indexed_texture"
                    or not texture.get("indices_path")
                ):
                    raise CacheError(
                        f"qualified actor selector {selector} is not materialized"
                    )
            selected_textures = [texture_rows[selector] for selector in selectors]
            animation_shapes = {
                (row["animated"], row["frame_count"]) for row in selected_textures
            }
            if len(animation_shapes) != 1:
                raise CacheError(f"selector animation family disagrees for {family}")
            texture_animated, texture_frame_count = next(iter(animation_shapes))
            if actor_spec is not None:
                if len(visible_indices) != len(indices):
                    raise CacheError(f"qualified actor left terrain render chain: {family}")
                actor_total += len(visible_indices)
                status = "qualified_owner_actor_billboard"
                qualified_view_indices = visible_indices
            else:
                effect_total += len(visible_indices)
                status = (
                    "mapped_reset_capture_site_effect_hidden_qualified_first_present_diverged"
                )
                qualified_view_indices = []
            hidden_reason = (
                None
                if len(visible_indices) == len(indices)
                else "same_reset_group_contains_off_chain_non_drawn_records"
            )
        else:
            family = None
            selectors = []
            status = "debug_unresolved_hidden_in_qualified_view"
            if class_id == 3:
                hidden_reason = "live_player_is_presented_by_the_carpet_flight_rig"
            elif class_id == 10:
                hidden_reason = (
                    "transient_effect_or_command_record_lifetime_not_live_in_frozen_snapshot"
                )
            elif class_id == 11:
                hidden_reason = "switch_trigger_or_checkpoint_control_record"
            elif class_id == 15:
                hidden_reason = "spell_holder_or_inventory_control_record"
            else:
                hidden_reason = "semantic_and_visibility_role_unresolved"
        groups.append(
            {
                "group_id": f"reset-{indices[0]:03d}",
                "entity_indices": indices,
                "entity_count": len(indices),
                "reset_chain_drawable_entity_indices": visible_indices,
                "reset_chain_drawable_count": len(visible_indices),
                "qualified_view_entity_indices": qualified_view_indices,
                "qualified_view_count": len(qualified_view_indices),
                "suppressed_terrain_head_entity_indices": suppressed_head_indices,
                "off_chain_entity_indices": off_chain_indices,
                "authority": authority,
                "key": {
                    "class_id": class_id,
                    "model": model,
                    "action": action,
                    "parameter_record_index": parameter_index,
                    "particle_parameter_index": particle_index,
                    "animation_frame": animation_frame,
                    "control_flags_u32": control_flags,
                },
                "semantic_family": family,
                "source_sprite_disposition": {
                    "kind": source_disposition,
                    "semantic": source_semantic,
                    "excluded_reason": (
                        source_semantic if source_disposition == "Excluded" else None
                    ),
                    "payload_wildcard": False,
                },
                "qualified_status": status,
                "qualified_hidden_reason": hidden_reason,
                "particle": {
                    "base_tmap_selector": base_selector,
                    "word_2": word_2,
                    "word_4": word_4,
                    "speed": speed,
                    "logical_height_source_units": rotation_speed,
                    "raw_raster_table_selector_offset": raster_table_selector_offset,
                    "signed_byte_11": signed_byte_11,
                    "render_mode": render_mode,
                    "byte_13": byte_13,
                },
                "selector": {
                    "bank": "TMAPS1-0",
                    "qualified_selectors": selectors,
                    "expression": (
                        "base_tmap_selector"
                        if render_mode in (0, 1, 21) and presentation_spec is not None
                        else (
                            "base_tmap_selector + D4750_28[yaw_relative_orientation_0_15]"
                            if render_mode == 20 and presentation_spec is not None
                            else (
                                "base_tmap_selector + animation_frame"
                                if presentation_spec is not None and
                                (2 <= render_mode <= 16 or 22 <= render_mode <= 36)
                                else "unresolved_in_qualified_view"
                            )
                        )
                    ),
                    "yaw_lookup_0_15": (
                        list(YAW_LOOKUP_D4750_28)
                        if render_mode == 20 and presentation_spec is not None
                        else []
                    ),
                    "mirror_when_orientation_gte_8": (
                        render_mode == 20 and presentation_spec is not None
                    ),
                    "yaw_sector_expression": (
                        "(((entity_yaw-camera_yaw)>>3)&0xF0)>>4"
                        if render_mode == 20 and presentation_spec is not None
                        else None
                    ),
                },
                "palette": {
                    "asset_id": "owner_data:DATA/PALN-0.DAT",
                    "path": "palette/paln.rgb8",
                    "binding": "exact_night_base_palette",
                },
                "billboard": {
                    "facing": "full_camera_facing_screen_plane",
                    "world_height": "logical_height_source_units / 64",
                    "world_width": "world_height * selected_tmap_width / selected_tmap_height",
                    "anchor": (
                        "viewer_bottom_center_at_entity_xyz_approximation"
                        if presentation_spec is not None
                        else "unresolved_in_qualified_view"
                    ),
                    "source_anchor_status": (
                        "deferred: original DrawSprite_41BD3 a1 offset depends on "
                        "(realHeight+realWidth)>>2 and camera sin/cos"
                        if presentation_spec is not None
                        else "unresolved_in_qualified_view"
                    ),
                    "hotspot": "no_per_sprite_hotspot_in_type_particle_str",
                    "depth": (
                        "viewer_world_depth_approximation_only; original terrain-cell "
                        "traversal, distance/fog gates, and software clipping are deferred"
                    ),
                    "compositing": (
                        "indexed_zero_transparent_standard_path"
                        if render_mode in (0, 20)
                        else (
                            "x_BYTE_F2CC6_alternate_transparency_path_approximated_by_index_zero_cutout"
                            if render_mode in (21, 36)
                            else "unresolved_in_qualified_view"
                        )
                    ),
                    "texture_animation": {
                        "animated": texture_animated,
                        "frame_count": texture_frame_count,
                        "scope": (
                            "selector_global_shared_TMAP_state_not_per_entity"
                            if texture_animated
                            else "static"
                            if presentation_spec is not None
                            else "unresolved_in_qualified_view"
                        ),
                        "cache_payload": (
                            "complete decoded base plus delta stream retained per selector"
                            if texture_animated
                            else "complete static decoded payload retained"
                        ),
                        "qualified_view": (
                            "base_frame_only; global used-TMAP update timing deferred"
                            if texture_animated
                            else "static_base_frame"
                            if presentation_spec is not None
                            else "unresolved_in_qualified_view"
                        ),
                    },
                },
            }
        )

    observed_counts = {
        row["semantic_family"]: row["entity_count"]
        for row in groups
        if row["qualified_status"] == "qualified_owner_actor_billboard"
    }
    if observed_counts != EXPECTED_ACTOR_COUNTS or actor_total != 64:
        raise CacheError("qualified Level 1 actor roster changed")
    if (
        effect_total != 19
        or suppressed_head_total != 47
        or off_chain_total != 47
        or actor_total + effect_total != 83
    ):
        raise CacheError("qualified Level 1 renderer disposition changed")
    authority_counts = {
        authority: sum(
            row["entity_count"] for row in groups if row["authority"] == authority
        )
        for authority in ("FrozenObserved", "PlayerFlightLive", "SpellActivationLive")
    }
    if authority_counts != {
        "FrozenObserved": 175,
        "PlayerFlightLive": 1,
        "SpellActivationLive": 1,
    } or any(
        row["authority"] != "FrozenObserved"
        for row in groups
        if row["reset_chain_drawable_count"]
    ):
        raise CacheError("qualified Level 1 presentation authority split changed")
    return {
        "schema": "mc2-level1-entity-presentations/v1",
        "authority_contract": {
            "reset_rows": {
                "FrozenObserved": 175,
                "PlayerFlightLive": 1,
                "SpellActivationLive": 1,
            },
            "qualified_view_rows": "64 stable FrozenObserved actors",
            "reset_chain_drawable_rows": "83 FrozenObserved rows",
            "transient_effect_boundary":
                "19 rows are exact exporter capture-site/reset-chain presentation, "
                "not the later first-presented-frame state after original update passes",
            "runtime_updates": "only ABI live-authority rows may mutate",
            "qualified_view":
                "64 stable owner actor billboards; no transient, suppressed, or off-chain proxies",
            "debug_view":
                "all other active reset rows are explicit unresolved diagnostics",
        },
        "source_contract": {
            "entity_record": "MCE1/v1 176-byte records",
            "presentation_record": "MCV1/v1 particle table",
            "selector_bank": "owner_data:DATA/TMAPS1-0.DAT",
            "palette": "owner_data:DATA/PALN-0.DAT",
            "msprn_status":
                "not_used_for_world_entities; closure MSPRN rows are HUD/bitmap path",
        },
        "qualified_actor_count": actor_total,
        "mapped_reset_effect_count": effect_total,
        "reset_chain_drawable_count": actor_total + effect_total,
        "qualified_view_count": actor_total,
        "suppressed_terrain_head_count": suppressed_head_total,
        "off_chain_active_count": off_chain_total,
        "qualified_group_count": len(EXPECTED_ACTOR_COUNTS),
        "active_reset_count": 177,
        "active_group_count": len(groups),
        "raw_map_chain_entry_count": len(raw_chain_entries),
        "raw_map_chain_entries": raw_chain_entries,
        "source_tuple_census": {
            "tuple_count": len(RESET_SOURCE_TUPLE_DISPOSITIONS),
            "render_tuple_count": sum(
                disposition == "Render"
                for disposition, _ in RESET_SOURCE_TUPLE_DISPOSITIONS.values()
            ),
            "excluded_tuple_count": sum(
                disposition == "Excluded"
                for disposition, _ in RESET_SOURCE_TUPLE_DISPOSITIONS.values()
            ),
            "payload_wildcards": 0,
            "unknown_reachable_tuple": "hard_error",
        },
        "groups": groups,
    }


def compile_rnc_decoder(directory: Path) -> tuple[Path, dict[str, Any]]:
    executable = directory / "mc-rnc-decode"
    archive_result = subprocess.run(
        [
            "git",
            "-C",
            str(REMC2_GIT),
            "archive",
            "--format=tar",
            PINNED_REMC2_COMMIT,
            "remc2/engine",
            "remc2/build_config.h",
            RNC_DECODER_CPP_SOURCE,
            RNC_DECODER_HEADER_SOURCE,
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if archive_result.returncode != 0:
        detail = archive_result.stderr.decode("utf-8", errors="replace").strip()
        raise CacheError(f"cannot archive pinned RNC source closure: {detail}")
    archive_sha256 = sha256_bytes(archive_result.stdout)
    if archive_sha256 != EXPECTED_RNC_SOURCE_ARCHIVE_SHA256:
        raise CacheError(
            "pinned RNC source archive changed: "
            f"expected {EXPECTED_RNC_SOURCE_ARCHIVE_SHA256}, got {archive_sha256}"
        )
    source_root = directory / "pinned-source"
    source_root.mkdir()
    with tarfile.open(fileobj=BytesIO(archive_result.stdout), mode="r:") as archive:
        archive.extractall(source_root, filter="data")
    implementation = source_root / RNC_DECODER_CPP_SOURCE
    header = source_root / RNC_DECODER_HEADER_SOURCE
    command = [
        "g++",
        "-std=c++17",
        "-O2",
        "-I",
        str(header.parent),
        str(RNC_HELPER),
        str(implementation),
        "-o",
        str(executable),
    ]
    try:
        completed = subprocess.run(
            command, check=True, capture_output=True, text=True
        )
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", "") or str(error)
        raise CacheError(f"cannot build proven RNC decoder: {detail.strip()}") from error
    if completed.stderr:
        raise CacheError(f"RNC decoder compiler emitted diagnostics: {completed.stderr.strip()}")
    return executable, rnc_logical_identity(archive_sha256=archive_sha256)


def rnc_logical_identity(
    non_identity_compiler_metadata: str | None = None,
    *,
    archive_sha256: str = EXPECTED_RNC_SOURCE_ARCHIVE_SHA256,
) -> dict[str, Any]:
    # The compiler executable is only a vehicle for the already pinned source.
    # Host version strings must not perturb the cache's logical identity.
    del non_identity_compiler_metadata
    return {
        "id": "remc2-DataFileRNC-owner-cache-adapter",
        "compile_flags": ["-std=c++17", "-O2"],
        "toolchain_identity_contract":
            "host compiler metadata is non-identity-bearing; adapter/implementation/"
            "header and every decoded output hash define logical cache truth",
        "adapter_sha256": sha256_file(RNC_HELPER),
        "source_archive_sha256": archive_sha256,
        "implementation_sha256": sha256_bytes(
            pinned_remc2_blob(RNC_DECODER_CPP_SOURCE)
        ),
        "header_sha256": sha256_bytes(
            pinned_remc2_blob(RNC_DECODER_HEADER_SOURCE)
        ),
    }


def parse_source_uv_rows() -> list[list[list[int]]]:
    text = pinned_remc2_blob(REMC2_UV_TABLE_SOURCE).decode("utf-8")
    values = [
        int(value, 16)
        for value in re.findall(r"0x[0-9A-Fa-f]+", text)
    ]
    if len(values) != 32 * 8 or any(value not in (0, 0x001FFFFF) for value in values):
        raise CacheError("remc2 UVTable_D4350 no longer has the qualified 32x8 binary form")
    rows: list[list[list[int]]] = []
    for row in range(32):
        values8 = values[row * 8:(row + 1) * 8]
        rows.append([
            [values8[index] // 0x001FFFFF, values8[index + 1] // 0x001FFFFF]
            for index in range(0, 8, 2)
        ])
    return rows


def decode_tmap_rnc(
    decoder: Path, packed: bytes, directory: Path, index: int
) -> bytes:
    source = directory / f"tmap-{index:03d}.rnc"
    target = directory / f"tmap-{index:03d}.decoded"
    source.write_bytes(packed)
    try:
        subprocess.run(
            [str(decoder), str(source), str(target)],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", "") or str(error)
        raise CacheError(f"TMAPS RNC decode failed at index {index}: {detail.strip()}") from error
    return target.read_bytes()


def _checked_unpack(
    layout: struct.Struct,
    payload: bytes,
    offset: int,
    limit: int,
    label: str,
) -> tuple[Any, ...]:
    end = offset + layout.size
    if offset < 0 or end > limit or limit > len(payload):
        raise CacheError(f"{label} is truncated")
    return layout.unpack_from(payload, offset)


def _apply_goat_tmap_delta(
    decoded: bytes,
    pixels: bytearray,
    *,
    selector: int,
    width: int,
    height: int,
    cursor: int,
    stream_end: int,
) -> int:
    """Apply the exact remc2 SS2 subset used by Goat selectors 152-154."""
    outer_start = TMAP_HEADER.size + cursor
    outer_size, outer_tag = _checked_unpack(
        FLI_CHUNK_HEADER,
        decoded,
        outer_start,
        stream_end,
        f"Goat TMAP {selector} outer record",
    )
    if outer_size < FLI_CHUNK_HEADER.size + 10 + FLI_CHUNK_HEADER.size:
        raise CacheError(f"Goat TMAP {selector} outer record is too small")
    outer_end = outer_start + outer_size
    if outer_end > stream_end:
        raise CacheError(f"Goat TMAP {selector} outer record exceeds stream")
    if outer_tag != 0xF1FA:
        raise CacheError(f"Goat TMAP {selector} outer tag is not F1FA")

    position = outer_start + FLI_CHUNK_HEADER.size
    (child_count,) = _checked_unpack(
        struct.Struct("<H"),
        decoded,
        position,
        outer_end,
        f"Goat TMAP {selector} child count",
    )
    position += 2
    if child_count != 1:
        raise CacheError(f"Goat TMAP {selector} must have exactly one child")
    reserved_end = position + 8
    if reserved_end > outer_end:
        raise CacheError(f"Goat TMAP {selector} reserved bytes are truncated")
    if decoded[position:reserved_end] != bytes(8):
        raise CacheError(f"Goat TMAP {selector} reserved bytes are nonzero")
    position = reserved_end

    child_start = position
    child_size, child_type = _checked_unpack(
        FLI_CHUNK_HEADER,
        decoded,
        child_start,
        outer_end,
        f"Goat TMAP {selector} child record",
    )
    if child_size < FLI_CHUNK_HEADER.size + 2:
        raise CacheError(f"Goat TMAP {selector} child record is too small")
    child_end = child_start + child_size
    if child_end > outer_end:
        raise CacheError(f"Goat TMAP {selector} child record exceeds outer record")
    if child_type != 7:
        raise CacheError(f"Goat TMAP {selector} child type is not SS2 type 7")
    position = child_start + FLI_CHUNK_HEADER.size

    (row_count,) = _checked_unpack(
        struct.Struct("<H"),
        decoded,
        position,
        child_end,
        f"Goat TMAP {selector} row count",
    )
    position += 2
    if row_count != height:
        raise CacheError(f"Goat TMAP {selector} row count disagrees with height")

    for row in range(height):
        (command_count,) = _checked_unpack(
            struct.Struct("<h"),
            decoded,
            position,
            child_end,
            f"Goat TMAP {selector} row {row} command count",
        )
        position += 2
        if command_count < 0:
            raise CacheError(
                f"Goat TMAP {selector} uses unsupported negative SS2 row control"
            )
        row_start = row * width
        destination = row_start
        row_end = row_start + width
        for command in range(command_count):
            if position + 2 > child_end:
                raise CacheError(
                    f"Goat TMAP {selector} row {row} command {command} is truncated"
                )
            destination += decoded[position]
            span = struct.unpack_from("<b", decoded, position + 1)[0]
            position += 2
            if span == 0:
                raise CacheError(
                    f"Goat TMAP {selector} row {row} command {command} has zero span"
                )
            word_count = abs(span)
            write_bytes = word_count * 2
            if destination < row_start or destination + write_bytes > row_end:
                raise CacheError(
                    f"Goat TMAP {selector} row {row} command {command} crosses row"
                )
            if span > 0:
                literal_end = position + write_bytes
                if literal_end > child_end:
                    raise CacheError(
                        f"Goat TMAP {selector} row {row} literal is truncated"
                    )
                pixels[destination:destination + write_bytes] = (
                    decoded[position:literal_end]
                )
                position = literal_end
            else:
                pair_end = position + 2
                if pair_end > child_end:
                    raise CacheError(
                        f"Goat TMAP {selector} row {row} repeat pair is truncated"
                    )
                pixels[destination:destination + write_bytes] = (
                    decoded[position:pair_end] * word_count
                )
                position = pair_end
            destination += write_bytes

    if position != child_end:
        raise CacheError(f"Goat TMAP {selector} child record has trailing bytes")
    if child_end != outer_end:
        raise CacheError(f"Goat TMAP {selector} outer record has trailing bytes")
    return outer_end - TMAP_HEADER.size


def materialize_goat_tmap_animation(
    selector: int,
    decoded: bytes,
) -> list[dict[str, Any]]:
    """Decode the qualified four-frame Goat TMAP stream and its wrap closure."""
    if selector not in GOAT_TMAP_SELECTORS:
        raise CacheError(f"selector {selector} is outside the qualified Goat subset")
    if len(decoded) < TMAP_HEADER.size:
        raise CacheError(f"Goat TMAP {selector} header is truncated")
    header_word, width, height = TMAP_HEADER.unpack_from(decoded)
    if header_word != 5123:
        raise CacheError(f"Goat TMAP {selector} header flags changed")
    expected_extent = {152: (22, 22), 153: (32, 22), 154: (22, 22)}[selector]
    if (width, height) != expected_extent:
        raise CacheError(f"Goat TMAP {selector} extent changed")

    first_frame_end = TMAP_HEADER.size + width * height
    animation_end = first_frame_end + TMAP_ANIMATION_HEADER.size
    if animation_end > len(decoded):
        raise CacheError(f"Goat TMAP {selector} animation header is truncated")
    frame_count, stream_size = TMAP_ANIMATION_HEADER.unpack_from(
        decoded, first_frame_end
    )
    if frame_count != 4:
        raise CacheError(f"Goat TMAP {selector} frame count changed")
    stream_start = animation_end
    stream_end = stream_start + stream_size
    if stream_end != len(decoded):
        raise CacheError(f"Goat TMAP {selector} declared stream extent changed")

    pixels = bytearray(decoded[TMAP_HEADER.size:first_frame_end])
    cursor = first_frame_end
    states: list[dict[str, Any]] = [
        {
            "state": "reset-base",
            "pixels": bytes(pixels),
            "stream_cursor": cursor,
            "frame_index": 1,
            "width": width,
            "height": height,
        }
    ]
    for frame in range(1, frame_count + 1):
        cursor = _apply_goat_tmap_delta(
            decoded,
            pixels,
            selector=selector,
            width=width,
            height=height,
            cursor=cursor,
            stream_end=stream_end,
        )
        states.append(
            {
                "state": f"frame-{frame:02d}",
                "pixels": bytes(pixels),
                "stream_cursor": cursor,
                "frame_index": frame + 1,
                "width": width,
                "height": height,
            }
        )
    if TMAP_HEADER.size + cursor != stream_end:
        raise CacheError(f"Goat TMAP {selector} has missing or extra frame records")

    closure_pixels = bytearray(pixels)
    closure_cursor = _apply_goat_tmap_delta(
        decoded,
        closure_pixels,
        selector=selector,
        width=width,
        height=height,
        cursor=first_frame_end,
        stream_end=stream_end,
    )
    states.append(
        {
            "state": "closure-frame-01",
            "pixels": bytes(closure_pixels),
            "stream_cursor": closure_cursor,
            "frame_index": 2,
            "width": width,
            "height": height,
        }
    )
    return states


def load_goat_tmap_golden() -> dict[str, Any]:
    if sha256_file(GOAT_TMAP_GOLDEN) != EXPECTED_GOAT_TMAP_GOLDEN:
        raise CacheError("Goat TMAP golden file hash changed")
    golden = load_json(GOAT_TMAP_GOLDEN)
    if (
        golden.get("schema") != "mc2-private-goat-tmap-golden/v1"
        or golden.get("source_manifest_sha256")
        != EXPECTED_GOAT_TMAP_ORACLE_MANIFEST
        or set(golden.get("selectors", {}))
        != {str(value) for value in GOAT_TMAP_SELECTORS}
    ):
        raise CacheError("Goat TMAP golden provenance or selector scope changed")
    return golden


def qualify_goat_tmap_states(
    selector: int,
    states: list[dict[str, Any]],
    golden: dict[str, Any],
) -> None:
    observed = [
        [
            state["state"],
            sha256_bytes(state["pixels"]),
            state["stream_cursor"],
            state["frame_index"],
            state["width"],
            state["height"],
            len(state["pixels"]),
        ]
        for state in states
    ]
    if observed != golden["selectors"][str(selector)]:
        raise CacheError(f"Goat TMAP {selector} states disagree with remc2 oracle")
    if states[4]["pixels"] != states[0]["pixels"]:
        raise CacheError(f"Goat TMAP {selector} frame 4 does not restore reset")
    if states[5]["pixels"] != states[1]["pixels"]:
        raise CacheError(f"Goat TMAP {selector} closure does not reproduce frame 1")


def decode_container_blob(
    decoder: Path,
    container: bytes,
    offset: int,
    unpacked_size: int,
    directory: Path,
    label: str,
) -> bytes:
    if offset < 0 or unpacked_size < 0 or offset >= len(container):
        raise CacheError(f"{label} descriptor is outside its container")
    if container[offset:offset + 3] != b"RNC":
        end = offset + unpacked_size
        if end > len(container):
            raise CacheError(f"{label} raw payload is truncated")
        return container[offset:end]
    if offset + 18 > len(container):
        raise CacheError(f"{label} RNC header is truncated")
    declared_unpacked = struct.unpack_from(">I", container, offset + 4)[0]
    packed_size = struct.unpack_from(">I", container, offset + 8)[0]
    if declared_unpacked != unpacked_size:
        raise CacheError(f"{label} RNC unpacked-size mismatch")
    end = offset + 18 + packed_size
    if end > len(container):
        raise CacheError(f"{label} RNC payload is truncated")
    source = directory / f"{label}.rnc"
    target = directory / f"{label}.decoded"
    source.write_bytes(container[offset:end])
    try:
        subprocess.run(
            [str(decoder), str(source), str(target)],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", "") or str(error)
        raise CacheError(f"{label} RNC decode failed: {detail.strip()}") from error
    decoded = target.read_bytes()
    if len(decoded) != unpacked_size:
        raise CacheError(f"{label} decoded extent changed")
    return decoded


def container_descriptor(
    container: bytes,
    *,
    count_rows: int,
    selector: int,
    row: int,
    label: str,
) -> tuple[list[int], dict[str, int]]:
    if len(container) < 4:
        raise CacheError(f"{label} container is truncated")
    trailer = struct.unpack_from("<i", container, len(container) - 4)[0]
    count_bytes = count_rows * 2
    record_bytes = count_rows * 16
    if trailer < 0 or trailer + count_bytes > len(container) - 4:
        raise CacheError(f"{label} trailer pointer is invalid")
    counts = list(struct.unpack_from(f"<{count_rows}h", container, trailer))
    if selector < 0 or selector >= counts[row]:
        raise CacheError(f"{label} selector is absent from descriptor row")
    record_offset = trailer + count_bytes + selector * record_bytes + row * 16
    if record_offset + 16 > len(container) - 4:
        raise CacheError(f"{label} descriptor record is truncated")
    metadata_offset, data_offset, metadata_size, data_size = struct.unpack_from(
        "<4i", container, record_offset
    )
    descriptor = {
        "container_trailer_offset": trailer,
        "descriptor_offset": record_offset,
        "metadata_offset": metadata_offset,
        "data_offset": data_offset,
        "metadata_size": metadata_size,
        "data_size": data_size,
    }
    for key, value in descriptor.items():
        if value < 0:
            raise CacheError(f"{label} descriptor has negative {key}")
    return counts, descriptor


def packed_audio_records(
    metadata: bytes,
    data: bytes,
    *,
    expected_count: int,
    label: str,
) -> list[dict[str, Any]]:
    expected_size = 32 + expected_count * 32
    if len(metadata) != expected_size:
        raise CacheError(f"{label} metadata extent changed")
    records = []
    for index in range(expected_count):
        record = metadata[32 + index * 32:64 + index * 32]
        filename_bytes = record[:18].split(b"\0", 1)[0]
        try:
            filename = filename_bytes.decode("ascii")
        except UnicodeDecodeError as error:
            raise CacheError(f"{label} filename is not ASCII") from error
        relative_offset = struct.unpack_from("<i", record, 18)[0]
        unknown = record[22:26]
        size, word = struct.unpack_from("<iH", record, 26)
        if (
            not filename
            or size < 0
            or relative_offset + size > len(data)
            or unknown != b"\0\0\0\0"
        ):
            raise CacheError(f"{label} record {index} is malformed or overlaps")
        records.append({
            "slot": index,
            "filename": filename,
            "relative_data_offset": relative_offset,
            "bytes": size,
            "word": word,
            "payload_sha256": sha256_bytes(
                data[relative_offset:relative_offset + size]
            ),
        })
    unique_spans = sorted({
        (row["relative_data_offset"], row["relative_data_offset"] + row["bytes"])
        for row in records
    })
    for previous, current in zip(unique_spans, unique_spans[1:]):
        if current[0] < previous[1]:
            raise CacheError(f"{label} contains overlapping non-identical records")
    return records


def wav_properties(payload: bytes, label: str) -> dict[str, Any]:
    if payload[:4] != b"RIFF" or payload[8:12] != b"WAVE":
        raise CacheError(f"{label} is not a RIFF/WAVE payload")
    declared = struct.unpack_from("<I", payload, 4)[0] + 8
    if declared > len(payload):
        raise CacheError(f"{label} RIFF extent exceeds its record")
    try:
        with wave.open(BytesIO(payload), "rb") as stream:
            channels = stream.getnchannels()
            sample_width = stream.getsampwidth()
            sample_rate = stream.getframerate()
            frame_count = stream.getnframes()
            compression = stream.getcomptype()
    except (EOFError, wave.Error) as error:
        raise CacheError(f"{label} WAVE parse failed: {error}") from error
    if compression != "NONE" or channels <= 0 or sample_width not in (1, 2):
        raise CacheError(f"{label} unsupported WAVE format")
    return {
        "riff_declared_bytes": declared,
        "channels": channels,
        "sample_width_bytes": sample_width,
        "sample_rate": sample_rate,
        "frame_count": frame_count,
        "duration_numerator_frames": frame_count,
        "duration_denominator_hz": sample_rate,
    }


def midi_dynamic_control_census(payload: bytes, label: str) -> dict[str, Any]:
    """Return the bounded XMIDI control events relevant to MC2 music dynamics."""
    if payload[:4] != b"MThd" or len(payload) < 14:
        raise CacheError(f"{label} is not standard MIDI")
    header_size = struct.unpack_from(">I", payload, 4)[0]
    if header_size < 6 or 8 + header_size > len(payload):
        raise CacheError(f"{label} MIDI header extent is invalid")
    midi_format, track_count, division = struct.unpack_from(">HHH", payload, 8)
    if midi_format != 0 or track_count != 1 or division == 0:
        raise CacheError(f"{label} MIDI format/track/division changed")

    def read_vlq(data: bytes, offset: int, extent: int) -> tuple[int, int]:
        value = 0
        for _ in range(4):
            if offset >= extent:
                raise CacheError(f"{label} truncated MIDI VLQ")
            byte = data[offset]
            offset += 1
            value = (value << 7) | (byte & 0x7F)
            if byte < 0x80:
                return value, offset
        raise CacheError(f"{label} MIDI VLQ exceeds four bytes")

    offset = 8 + header_size
    if payload[offset:offset + 4] != b"MTrk" or offset + 8 > len(payload):
        raise CacheError(f"{label} MIDI track chunk is missing")
    track_size = struct.unpack_from(">I", payload, offset + 4)[0]
    offset += 8
    track_end = offset + track_size
    if track_end > len(payload):
        raise CacheError(f"{label} MIDI track extent is invalid")

    tick = 0
    running_status: int | None = None
    relevant: list[dict[str, int]] = []
    saw_end = False
    while offset < track_end:
        delta, offset = read_vlq(payload, offset, track_end)
        tick += delta
        if offset >= track_end:
            raise CacheError(f"{label} MIDI event is truncated")
        status = payload[offset]
        if status < 0x80:
            if running_status is None:
                raise CacheError(f"{label} MIDI running status has no predecessor")
            status = running_status
        else:
            offset += 1
            if status < 0xF0:
                running_status = status

        if status == 0xFF:
            if offset >= track_end:
                raise CacheError(f"{label} MIDI meta event is truncated")
            meta_type = payload[offset]
            offset += 1
            size, offset = read_vlq(payload, offset, track_end)
            if offset + size > track_end:
                raise CacheError(f"{label} MIDI meta extent is invalid")
            offset += size
            if meta_type == 0x2F:
                saw_end = True
                break
            continue
        if status in (0xF0, 0xF7):
            size, offset = read_vlq(payload, offset, track_end)
            if offset + size > track_end:
                raise CacheError(f"{label} MIDI SysEx extent is invalid")
            offset += size
            running_status = None
            continue
        if status >= 0xF0:
            raise CacheError(f"{label} unsupported MIDI system event")

        event_type = status & 0xF0
        channel = status & 0x0F
        data_size = 1 if event_type in (0xC0, 0xD0) else 2
        if offset + data_size > track_end:
            raise CacheError(f"{label} MIDI channel event is truncated")
        first = payload[offset]
        second = payload[offset + 1] if data_size == 2 else 0
        offset += data_size
        if event_type == 0xB0 and first in (116, 117, 119):
            relevant.append({
                "tick": tick,
                "channel": channel,
                "controller": first,
                "value": second,
            })
    if not saw_end:
        raise CacheError(f"{label} MIDI track has no end marker")

    trigger_events = [
        event for event in relevant if event["controller"] == 119
    ]
    return {
        "midi_format": midi_format,
        "track_count": track_count,
        "division": division,
        "trigger_controller": 119,
        "trigger_events": trigger_events,
        "tagged_channels": sorted({
            event["channel"]
            for event in trigger_events
            if event["value"] == 0
        }),
        "loop_controller_events": [
            event for event in relevant if event["controller"] in (116, 117)
        ],
    }


def compile_xmi_converter(directory: Path) -> tuple[Path, dict[str, Any]]:
    executable = directory / "mc-xmi-to-midi"
    implementation = directory / "xmi2mid.cpp"
    header = directory / "xmi2mid.h"
    adapter = directory / "xmi_convert.cpp"
    implementation.write_bytes(pinned_remc2_blob(XMI_IMPLEMENTATION_SOURCE))
    header.write_bytes(pinned_remc2_blob(XMI_HEADER_SOURCE))
    adapter_text = XMI_HELPER.read_text(encoding="utf-8")
    dirty_header_include = (
        '#include "../../../upstream/magic-carpet-2-hd/'
        'remc2/portability/xmi2mid.h"'
    )
    if adapter_text.count(dirty_header_include) != 1:
        raise CacheError("XMI adapter remc2 header include changed")
    adapter.write_text(
        adapter_text.replace(dirty_header_include, '#include "xmi2mid.h"'),
        encoding="utf-8",
    )
    command = [
        os.environ.get("CXX", "c++"),
        "-std=c++17",
        "-O2",
        str(adapter),
        str(implementation),
        "-o",
        str(executable),
    ]
    try:
        completed = subprocess.run(
            command, check=True, capture_output=True, text=True
        )
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", "") or str(error)
        raise CacheError(f"cannot build XMI converter: {detail.strip()}") from error
    if completed.stderr:
        raise CacheError(f"XMI converter compiler emitted diagnostics: {completed.stderr}")
    return executable, {
        "id": "remc2-TranscodeXmiToMid-owner-cache-adapter",
        "adapter_sha256": sha256_file(XMI_HELPER),
        "implementation_sha256": sha256_bytes(
            pinned_remc2_blob(XMI_IMPLEMENTATION_SOURCE)
        ),
        "header_sha256": sha256_bytes(
            pinned_remc2_blob(XMI_HEADER_SOURCE)
        ),
        "compile_flags": ["-std=c++17", "-O2"],
        "host_compiler_metadata_identity_bearing": False,
    }


def transcode_xmi(
    executable: Path, payload: bytes, directory: Path, label: str
) -> bytes:
    source = directory / f"{label}.xmi"
    target = directory / f"{label}.mid"
    source.write_bytes(payload)
    try:
        subprocess.run(
            [str(executable), str(source), str(target)],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", "") or str(error)
        raise CacheError(f"{label} XMI conversion failed: {detail.strip()}") from error
    midi = target.read_bytes()
    if midi[:4] != b"MThd":
        raise CacheError(f"{label} converter did not produce standard MIDI")
    return midi


def compile_adl_renderer() -> tuple[Path, dict[str, Any]]:
    """Build a private deterministic MIDI-to-WAVE adapter outside the cache."""
    archive_result = subprocess.run(
        [
            "git",
            "-C",
            str(REMC2_GIT),
            "archive",
            "--format=tar",
            PINNED_REMC2_COMMIT,
            ADLMIDI_SOURCE_AT_PIN,
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if archive_result.returncode != 0:
        detail = archive_result.stderr.decode("utf-8", errors="replace").strip()
        raise CacheError(f"cannot archive pinned libADLMIDI source: {detail}")
    archive_sha256 = sha256_bytes(archive_result.stdout)
    if archive_sha256 != EXPECTED_ADLMIDI_ARCHIVE_SHA256:
        raise CacheError(
            "pinned libADLMIDI archive changed: "
            f"expected {EXPECTED_ADLMIDI_ARCHIVE_SHA256}, got {archive_sha256}"
        )
    identity = sha256_bytes(
        canonical_json({
            "source_revision": PINNED_REMC2_COMMIT,
            "source_archive_sha256": archive_sha256,
            "adapter_sha256": sha256_file(ADL_RENDER_HELPER),
            "bank": 59,
            "chips": 2,
            "sample_rate": 44_100,
        })
    )[:20]
    build = Path(tempfile.gettempdir()) / f"mc-owner-adlmidi-{identity}"
    library = build / "libADLMIDI.a"
    executable = build / "mc-adl-render"
    if not library.is_file():
        build.mkdir(parents=True, exist_ok=True)
        source_parent = Path(tempfile.mkdtemp(prefix="pinned-source-", dir=build))
        with tarfile.open(fileobj=BytesIO(archive_result.stdout), mode="r:") as archive:
            archive.extractall(source_parent, filter="data")
        adlmidi_source = source_parent / ADLMIDI_SOURCE_AT_PIN
        configure = [
            "cmake",
            "-S", str(adlmidi_source),
            "-B", str(build),
            "-DCMAKE_BUILD_TYPE=Release",
            "-DADLMIDI_STATIC=ON",
            "-DWITH_MIDIPLAY=OFF",
            "-DWITH_GENADLDATA=OFF",
            "-DWITH_UNIT_TESTS=OFF",
            "-DWITH_VLC_PLUGIN=OFF",
        ]
        try:
            subprocess.run(
                configure, check=True, capture_output=True, text=True, timeout=180
            )
            subprocess.run(
                ["cmake", "--build", str(build), "--target", "ADLMIDI_static",
                 "--parallel", "2"],
                check=True,
                capture_output=True,
                text=True,
                timeout=300,
            )
        except (OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
            detail = getattr(error, "stderr", "") or str(error)
            raise CacheError(
                f"cannot build qualified private libADLMIDI: {detail.strip()}"
            ) from error
    if not executable.is_file():
        source_candidates = sorted(build.glob(f"pinned-source-*/{ADLMIDI_SOURCE_AT_PIN}"))
        if not source_candidates:
            source_parent = Path(tempfile.mkdtemp(prefix="pinned-source-", dir=build))
            with tarfile.open(fileobj=BytesIO(archive_result.stdout), mode="r:") as archive:
                archive.extractall(source_parent, filter="data")
            source_candidates = [source_parent / ADLMIDI_SOURCE_AT_PIN]
        adlmidi_source = source_candidates[-1]
        command = [
            os.environ.get("CXX", "c++"),
            "-std=c++17",
            "-O2",
            "-I", str(adlmidi_source / "include"),
            str(ADL_RENDER_HELPER),
            str(library),
            "-ldl", "-lpthread", "-lm",
            "-o", str(executable),
        ]
        try:
            subprocess.run(
                command, check=True, capture_output=True, text=True, timeout=180
            )
        except (OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
            detail = getattr(error, "stderr", "") or str(error)
            raise CacheError(f"cannot build ADLMIDI render adapter: {detail.strip()}") from error
    return executable, {
        "id": "libADLMIDI-private-owner-cache-render-adapter",
        "source_revision": PINNED_REMC2_COMMIT,
        "source_archive_sha256": archive_sha256,
        "source_license": "LGPL-2.1-or-later/GPL-3.0-or-later upstream dual license",
        "adapter_sha256": sha256_file(ADL_RENDER_HELPER),
        "compile_flags": ["-std=c++17", "-O2"],
        "bank": 59,
        "chips": 2,
        "sample_rate": 44_100,
        "host_tool_and_library_embedded_in_Unity": False,
    }


def render_midi_wave(
    executable: Path, midi: bytes, directory: Path, label: str
) -> bytes:
    source = directory / f"{label}-render.mid"
    target = directory / f"{label}-render.wav"
    source.write_bytes(midi)
    try:
        subprocess.run(
            [str(executable), str(source), str(target)],
            check=True,
            capture_output=True,
            text=True,
            timeout=180,
        )
    except (OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        detail = getattr(error, "stderr", "") or str(error)
        raise CacheError(f"{label} deterministic WAVE render failed: {detail.strip()}") from error
    payload = target.read_bytes()
    properties = wav_properties(payload, f"{label} rendered music")
    if (
        len(payload) != 77_889_580
        or sha256_bytes(payload) !=
            "03b3f951048504d8a6d30611d6c44a62ec1c463726b30c33c16e3cb69824605d"
        or properties != {
            "riff_declared_bytes": 77_889_580,
            "channels": 2,
            "sample_width_bytes": 2,
            "sample_rate": 44_100,
            "frame_count": 19_472_384,
            "duration_numerator_frames": 19_472_384,
            "duration_denominator_hz": 44_100,
        }
    ):
        raise CacheError("qualified C2GAME1 deterministic WAVE render changed")
    return payload


def write_output(
    stage: Path,
    outputs: list[dict[str, Any]],
    relative: str,
    data: bytes,
    *,
    kind: str,
    source_asset_ids: list[str],
    extra: dict[str, Any] | None = None,
) -> dict[str, Any]:
    target = stage / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(data)
    row: dict[str, Any] = {
        "path": relative,
        "kind": kind,
        "bytes": len(data),
        "sha256": sha256_bytes(data),
        "source_asset_ids": source_asset_ids,
    }
    if extra:
        row.update(extra)
    outputs.append(row)
    return row


def fsync_tree(root: Path) -> None:
    for path in sorted(item for item in root.rglob("*") if item.is_file()):
        with path.open("rb") as stream:
            os.fsync(stream.fileno())
    for path in sorted(
        (item for item in root.rglob("*") if item.is_dir()),
        key=lambda item: len(item.parts),
        reverse=True,
    ):
        descriptor = os.open(path, os.O_RDONLY)
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    descriptor = os.open(root, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def fsync_parent(path: Path) -> None:
    descriptor = os.open(path.parent, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def commit_stage(output: Path, stage: Path, failpoint: str | None = None) -> None:
    backup = output.with_name(output.name + ".previous")
    if not output.exists() and backup.exists():
        os.replace(backup, output)
        fsync_parent(output)
    if backup.exists():
        shutil.rmtree(backup)
        fsync_parent(output)
    if output.exists():
        os.replace(output, backup)
        fsync_parent(output)
    if failpoint == "after_backup":
        os.replace(backup, output)
        fsync_parent(output)
        raise CacheError("injected failure after backup rename")
    try:
        os.replace(stage, output)
        fsync_parent(output)
        if failpoint == "after_install":
            raise CacheError("injected failure after install rename")
    except Exception:
        if output.exists():
            shutil.rmtree(output)
        if backup.exists():
            os.replace(backup, output)
            fsync_parent(output)
        raise
    if backup.exists():
        shutil.rmtree(backup)
        fsync_parent(output)


LEVEL2_DAY_SOURCE_PATHS = (
    "DATA/PALD-0.DAT",
    "DATA/TABLESD.DAT",
    "DATA/CLRD-0.DAT",
    "DATA/BL32N0-0.DAT",
    "DATA/SKYD0-0.DAT",
    "DATA/TMAPS0-0.DAT",
    "DATA/TMAPS0-0.TAB",
    "DATA/MSPRD0-0.DAT",
    "DATA/MSPRD0-0.TAB",
    "DATA/MWEBD0-0.DAT",
    "DATA/MWEBD0-0.TAB",
)
LEVEL2_GAMEPLAY_SOURCE_PATHS = (
    "DATA/BUILD0-0.DAT",
    "DATA/BUILD0-0.TAB",
    "DATA/BLDGPRM.DAT",
    "DATA/FONT1.DAT",
    "DATA/FONT1.TAB",
    "DATA/MSPRD0-0.DAT",
    "DATA/MSPRD0-0.TAB",
    "DATA/MWEBD0-0.DAT",
    "DATA/MWEBD0-0.TAB",
    "DATA/POINTERS.DAT",
    "DATA/POINTERS.TAB",
    "DATA/SPELLS.DAT",
    "LEVELS/LEVELS.DAT",
    "LEVELS/LEVELS.TAB",
    "LANGUAGE/L1.TXT",
)
LEVEL2_AUDIO_SOURCE_PATHS = ("SOUND/SOUND.DAT", "SOUND/MUSIC.DAT")


def _level2_catalog_identity(catalog_path: Path) -> tuple[dict[str, Any], str]:
    """Load the owner catalogue and verify its signed, canonical identity."""
    catalog = load_json(catalog_path)
    if not isinstance(catalog, dict) or not catalog.get("schema_version"):
        raise CacheError("Level 2 owner catalogue has no schema_version")
    claim = catalog.get("catalog_sha256")
    payload = dict(catalog)
    payload.pop("catalog_sha256", None)
    actual = sha256_bytes(
        json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode(
            "utf-8"
        )
    )
    if claim != actual:
        raise CacheError("Level 2 owner catalogue canonical self-hash mismatch")
    return catalog, sha256_file(catalog_path)


def _level2_owner_identity_bindings(profile: dict[str, Any]) -> dict[str, Any]:
    """Project a typed owner component identity into a cache binding map."""
    owner_identity = profile.get("owner_package")
    if not isinstance(owner_identity, dict):
        raise CacheError("Level 2 owner package identity is missing")
    component = owner_identity.get("players_component")
    if not isinstance(component, dict):
        raise CacheError("Level 2 player component identity is missing")
    return {
        "owner_manifest_schema": owner_identity.get("manifest_schema"),
        "players_component_magic": component.get("magic"),
        "players_component_version": component.get("version"),
        "players_component_record_bytes": component.get("record_bytes"),
        "players_component_count": component.get("count"),
    }


def _validate_level2_owner_identity(
    profile: dict[str, Any],
    bindings: dict[str, Any],
    expected_owner_identity: dict[str, Any] | None,
) -> None:
    """Keep current v5 production and explicit historic inspection separate."""
    identity_binding_keys = {
        "owner_manifest_schema",
        "players_component_magic",
        "players_component_version",
        "players_component_record_bytes",
        "players_component_count",
    }
    if expected_owner_identity is None:
        if profile.get("owner_package") is not None or any(
            key in bindings for key in identity_binding_keys
        ):
            raise CacheError("historical Level 2 cache carries an unexpected owner component identity")
        return
    if profile.get("owner_package") != expected_owner_identity:
        raise CacheError("Level 2 owner package identity mismatch")
    expected_bindings = _level2_owner_identity_bindings(
        {"owner_package": expected_owner_identity}
    )
    if any(bindings.get(key) != value for key, value in expected_bindings.items()):
        raise CacheError("Level 2 owner component binding mismatch")


def _level2_profile(
    package_dir: Path, closure_path: Path, catalog_path: Path
) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any], dict[str, Any]]:
    """Resolve the immutable external/internal Day material-cache profile.

    The package, closure, catalogue, and every selected owner file are checked
    before any output is staged.  This is deliberately separate from the
    legacy Level 1 builder so its exact manifest/output identity cannot move.
    """
    package_dir = package_dir.expanduser().resolve()
    closure_path = closure_path.expanduser().resolve()
    catalog_path = catalog_path.expanduser().resolve()
    package = load_json(package_dir / "manifest.json")
    try:
        load_proven_export_verifier()(package_dir, package)
    except ValueError as error:
        raise CacheError(f"Level 2 package identity verification failed: {error}") from error
    level = package.get("level", {})
    map_type = level.get("map_type", {})
    if (
        package.get("schema") != LEVEL2_OWNER_PACKAGE_IDENTITY["manifest_schema"]
        or level.get("external") != 2
        or level.get("internal") != 1
        or map_type != {"id": 0, "name": "Day"}
    ):
        raise CacheError("Level 2 material profile requires external=2/internal=1/Day")
    if package.get("source", {}).get("upstream_commit") != PINNED_REMC2_COMMIT:
        raise CacheError("Level 2 package remc2 specification pin mismatch")
    players_path = package_dir / "players.bin"
    players = players_path.read_bytes()
    if len(players) < 12:
        raise CacheError("Level 2 players.bin header is truncated")
    magic, version, record_bytes, count = struct.unpack_from("<4sHHI", players)
    expected_component = LEVEL2_OWNER_PACKAGE_IDENTITY["players_component"]
    if (
        magic != expected_component["magic"].encode("ascii")
        or version != expected_component["version"]
        or record_bytes != expected_component["record_bytes"]
        or count != expected_component["count"]
        or len(players) != 12 + count * record_bytes
    ):
        raise CacheError("Level 2 material profile requires players component v4/663")
    selectors = package.get("presentation_dependencies")
    expected = {
        "map_type": "Day",
        "vga_resolution_selector": 1,
        "texture_size": 32,
        "terrain_graphics_flags": 0,
        "terrain_graphics_flag_2": False,
        "terrain_bank": "TMAPS0-0",
        "sprite_bank": "MSPRD0-0",
        "web_bank": "MWEBD0-0",
        "block_bank": "BL32N0-0",
        "palette": "PALD-0",
        "sky_bank": "SKYD0-0",
        "sound_bank": 1,
        "cursor": 9,
    }
    if not isinstance(selectors, dict) or any(
        selectors.get(key) != value for key, value in expected.items()
    ):
        raise CacheError("Level 2 material profile selector tuple changed")
    expected_resolved_files = {
        "level_records": "owner:GAME/NETHERW/CLEVELS/LEVELS.DAT",
        "terrain_bank": "owner:GAME/NETHERW/CDATA/TMAPS0-0.DAT",
        "sprite_bank": "owner:CD_Files/DATA/MSPRD0-0.DAT",
        "web_bank": "owner:CD_Files/DATA/MWEBD0-0.DAT",
        "block_bank": "owner:CD_Files/DATA/BL32N0-0.DAT",
        "palette": "owner:CD_Files/DATA/PALD-0.DAT",
        "sky_bank": "owner:CD_Files/DATA/SKYD0-0.DAT",
        "building_bank": "owner:CD_Files/DATA/BUILD0-0.DAT",
        "spell_table": "owner:CD_Files/DATA/SPELLS.DAT",
        "font_bank": "owner:CD_Files/DATA/FONT1.DAT",
        "language": "owner:CD_Files/LANGUAGE/L1.TXT",
    }
    if selectors.get("resolved_files") != expected_resolved_files:
        raise CacheError("Level 2 resolved owner-file selector map changed")
    closure = load_json(closure_path)
    closure_claim = closure.get("closure_sha256")
    closure_payload = dict(closure)
    closure_payload.pop("closure_sha256", None)
    closure_actual = sha256_bytes(
        json.dumps(closure_payload, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode(
            "utf-8"
        )
    )
    if closure_claim != closure_actual:
        raise CacheError("Level 2 closure canonical self-hash mismatch")
    closure_profile = closure.get("profile", {})
    if (
        closure_profile.get("name") != "level2-day"
        or closure_profile.get("external_level") != 2
        or closure_profile.get("internal_level") != 1
        or closure_profile.get("map_type") != {"id": 0, "name": "day"}
        or closure_profile.get("spec_commit") != PINNED_REMC2_COMMIT
        or closure_profile.get("owner_package") != LEVEL2_OWNER_PACKAGE_IDENTITY
    ):
        raise CacheError("Level 2 closure profile selector tuple changed")
    package_hash = package.get("package_sha256")
    if package_hash != EXPECTED_LEVEL2_PACKAGE:
        raise CacheError("wrong qualified Level 2 package hash")
    if closure.get("source", {}).get("level_export", {}).get("package_sha256") != package_hash:
        raise CacheError("Level 2 closure is bound to a different package")
    closure_owner_identity = closure.get("source", {}).get("level_export", {})
    if {
        "manifest_schema": closure_owner_identity.get("manifest_schema"),
        "players_component": closure_owner_identity.get("players_component"),
    } != LEVEL2_OWNER_PACKAGE_IDENTITY:
        raise CacheError("Level 2 closure owner component identity mismatch")
    catalog, catalog_file_sha256 = _level2_catalog_identity(catalog_path)
    closure_catalog = closure.get("source", {}).get("owner_catalog", {})
    if (
        closure_catalog.get("catalog_sha256") != catalog.get("catalog_sha256")
        or closure_catalog.get("file_sha256") != catalog_file_sha256
    ):
        raise CacheError("Level 2 catalogue does not match closure binding")
    if closure_claim != EXPECTED_LEVEL2_CLOSURE:
        raise CacheError("wrong qualified Level 2 closure hash")
    files = {row.get("path"): row for row in catalog.get("files", [])}
    entries = {row.get("asset_id"): row for row in catalog.get("entries", [])}
    for relative in LEVEL2_DAY_SOURCE_PATHS:
        row = files.get(relative)
        if row is None:
            raise CacheError(f"Level 2 owner catalogue has no file: {relative}")
    profile = {
        "name": "level2-day",
        "external_level": 2,
        "internal_level": 1,
        "map_type": {"id": 0, "name": "Day"},
        "selectors": {
            key: selectors[key]
            for key in (
                "vga_resolution_selector",
                "texture_size",
                "terrain_graphics_flags",
                "terrain_graphics_flag_2",
                "terrain_bank",
                "sprite_bank",
                "web_bank",
                "block_bank",
                "palette",
                "sky_bank",
            )
        },
        "package_sha256": package_hash,
        "closure_sha256": closure_claim,
        "catalog_sha256": catalog.get("catalog_sha256"),
        "catalog_file_sha256": catalog_file_sha256,
        "spec_commit": PINNED_REMC2_COMMIT,
        "owner_package": LEVEL2_OWNER_PACKAGE_IDENTITY,
    }
    return profile, package, closure, {**files, **entries}


def _level2_owner_sources(
    owner_root: Path, catalog_rows: dict[str, Any]
) -> dict[str, bytes]:
    """Verify and read all base Day banks against the catalogue hashes."""
    owner_root = owner_root.expanduser().resolve()
    source_bytes: dict[str, bytes] = {}
    for relative in LEVEL2_DAY_SOURCE_PATHS:
        path = confined_source(owner_root, relative)
        data = path.read_bytes()
        row = catalog_rows.get(relative)
        if not isinstance(row, dict) or row.get("sha256") != sha256_bytes(data):
            raise CacheError(f"Level 2 owner source hash differs from catalogue: {relative}")
        source_bytes[relative] = data
    return source_bytes


def _level2_gameplay_sources(
    owner_root: Path, catalog_rows: dict[str, Any]
) -> dict[str, bytes]:
    """Verify the additional closure-rooted gameplay/interface banks."""
    owner_root = owner_root.expanduser().resolve()
    source_bytes: dict[str, bytes] = {}
    for relative in LEVEL2_GAMEPLAY_SOURCE_PATHS:
        path = confined_source(owner_root, relative)
        data = path.read_bytes()
        row = catalog_rows.get(relative)
        if not isinstance(row, dict) or row.get("sha256") != sha256_bytes(data):
            raise CacheError(
                f"Level 2 gameplay source hash differs from catalogue: {relative}"
            )
        source_bytes[relative] = data
    return source_bytes


def _level2_tmap_selectors(closure: dict[str, Any]) -> list[int]:
    selectors: set[int] = set()
    for set_name in ("scene_reachable", "dynamic_dependencies"):
        rows = closure.get("sets", {}).get(set_name, {}).get("assets", [])
        for row in rows:
            asset_id = row.get("asset_id", "")
            prefix = "owner_data:DATA/TMAPS0-0.DAT#"
            if asset_id.startswith(prefix):
                try:
                    selectors.add(int(asset_id[len(prefix):]))
                except ValueError as error:
                    raise CacheError(f"invalid Level 2 TMAP selector: {asset_id}") from error
    if not selectors or min(selectors) < 0 or max(selectors) >= 504:
        raise CacheError("Level 2 closure has no bounded TMAPS0 selector set")
    return sorted(selectors)


def _level2_closure_asset_rows(
    closure: dict[str, Any], catalog_rows: dict[str, Any]
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Join the qualified scene/dynamic sets to catalog entries fail-closed."""
    scene = closure.get("sets", {}).get("scene_reachable", {}).get("assets", [])
    dynamic = closure.get("sets", {}).get("dynamic_dependencies", {}).get("assets", [])
    if not isinstance(scene, list) or not isinstance(dynamic, list):
        raise CacheError("Level 2 closure asset sets are malformed")
    for row in [*scene, *dynamic]:
        asset_id = row.get("asset_id") if isinstance(row, dict) else None
        catalog_entry = catalog_rows.get(asset_id)
        if not isinstance(asset_id, str) or not asset_id.startswith("owner_data:"):
            raise CacheError(f"Level 2 closure asset is not catalog-rooted: {asset_id}")
        if isinstance(catalog_entry, dict):
            if row.get("content_address") != catalog_entry.get("content_address"):
                raise CacheError(f"Level 2 closure/catalog content mismatch: {asset_id}")
            continue
        if row.get("kind") in {"terrain_modification_footprint", "localization_record"}:
            continue
        relative = asset_id.removeprefix("owner_data:").split("#", 1)[0]
        file_entry = catalog_rows.get(relative)
        if isinstance(file_entry, dict) and "sha256" in file_entry:
            if row.get("content_address") != f"sha256:{file_entry['sha256']}":
                raise CacheError(f"Level 2 closure/catalog file mismatch: {asset_id}")
            continue
        raise CacheError(f"Level 2 closure asset is not catalog-rooted: {asset_id}")
    return scene, dynamic


def _level2_cache_identity(outputs: list[dict[str, Any]]) -> str:
    return sha256_bytes(
        b"".join(
            f"{row['path']}\0{row['sha256']}\0{row['bytes']}\n".encode("ascii")
            for row in outputs
        )
    )


def _level2_gameplay_asset_bytes(
    row: dict[str, Any],
    source_bytes: dict[str, bytes],
    catalog_rows: dict[str, Any],
) -> tuple[bytes, dict[str, Any]]:
    """Resolve one closure asset to an owned byte span plus selector metadata."""
    asset_id = row["asset_id"]
    relative = asset_id.removeprefix("owner_data:").split("#", 1)[0]
    source = source_bytes.get(relative)
    if source is None:
        raise CacheError(f"Level 2 gameplay asset source is not selected: {asset_id}")
    kind = row.get("kind")
    if kind == "catalog_file":
        data = source
        metadata = {
            "selector_kind": "file_root",
            "source_path": relative,
            "extent": {"bytes": len(data), "status": "qualified_file_extent"},
            "mode": None,
            "mode_status": "not_applicable_file_root",
            "hotspot": None,
            "hotspot_status": "not_applicable_file_root",
        }
    elif kind == "localization_record":
        selector = row.get("selector")
        records = language_records(source)
        if not isinstance(selector, int) or selector < 0 or selector >= len(records):
            raise CacheError(f"Level 2 localization selector is out of bounds: {asset_id}")
        data = records[selector]
        metadata = {
            "selector_kind": "language_record",
            "selector": selector,
            "byte_length": len(data),
            "encoding": row.get("encoding"),
            "glyph_indices": row.get("glyph_indices", []),
            "extent": {"bytes": len(data), "status": "qualified_record_extent"},
            "mode": None,
            "mode_status": "not_applicable_localization",
            "hotspot": None,
            "hotspot_status": "not_applicable_localization",
        }
    elif kind == "terrain_modification_footprint":
        span = row.get("span", {})
        offset, length = span.get("offset"), span.get("length")
        if not isinstance(offset, int) or not isinstance(length, int) or offset < 0 or length <= 0:
            raise CacheError(f"Level 2 BUILD footprint span is malformed: {asset_id}")
        if offset + length > len(source):
            raise CacheError(f"Level 2 BUILD footprint span exceeds source: {asset_id}")
        data = source[offset : offset + length]
        metadata = {
            "selector_kind": "build_footprint",
            "index": row.get("index"),
            "offset": offset,
            "length": length,
            "width": row.get("width"),
            "height": row.get("height"),
            "cell_stride_bytes": row.get("cell_stride_bytes"),
            "extent": {
                "bytes": length,
                "width": row.get("width"),
                "height": row.get("height"),
                "status": "qualified_footprint_extent",
            },
            "mode": None,
            "mode_status": "not_applicable_build_footprint",
            "hotspot": None,
            "hotspot_status": "not_applicable_build_footprint",
        }
    else:
        entry = catalog_rows.get(asset_id)
        if not isinstance(entry, dict):
            raise CacheError(f"Level 2 gameplay catalog entry is missing: {asset_id}")
        offset = entry.get("offset")
        length = entry.get("encoded_length")
        if entry.get("end_offset") is not None:
            end_offset = entry["end_offset"]
            if not isinstance(offset, int) or not isinstance(end_offset, int):
                raise CacheError(f"Level 2 packed span is malformed: {asset_id}")
            length = end_offset - offset
        if not isinstance(offset, int) or not isinstance(length, int) or offset < 0 or length <= 0:
            raise CacheError(f"Level 2 indexed span is malformed: {asset_id}")
        if offset + length > len(source):
            raise CacheError(f"Level 2 indexed span exceeds source: {asset_id}")
        data = source[offset : offset + length]
        metadata = {
            "selector_kind": "indexed_entry",
            "index": entry.get("index"),
            "offset": offset,
            "length": length,
            "dimensions": entry.get("dimensions"),
            "mode": row.get("mode"),
            "mode_status": "qualified" if row.get("mode") is not None else "not_supplied_in_qualified_catalog",
            "hotspot": row.get("hotspot"),
            "hotspot_status": "qualified" if row.get("hotspot") is not None else "not_supplied_in_qualified_catalog",
            "extent": {
                "bytes": length,
                "dimensions": entry.get("dimensions"),
                "status": "qualified_dimensions" if all(
                    isinstance(entry.get("dimensions", {}).get(key), int)
                    and entry["dimensions"][key] > 0
                    for key in ("width", "height")
                ) else "not_decoded_requires_rnc_or_catalog_extent_only",
            },
        }
    expected = row.get("content_address")
    if expected != f"sha256:{sha256_bytes(data)}":
        raise CacheError(f"Level 2 gameplay asset bytes disagree with closure: {asset_id}")
    return data, metadata


def _level2_decode_indexed_catalog_entry(
    dat: bytes,
    records: list[tuple[int, int, int]],
    selector: int,
    catalog_entry: dict[str, Any],
    proven_decode: Any,
    label: str,
) -> tuple[bytes, bytes, int, int, int]:
    """Decode one owner bitmap and reconcile TAB, RLE, and catalog identity."""
    if selector < 0 or selector >= len(records):
        raise CacheError(f"{label} selector is out of bounds: {selector}")
    if catalog_entry.get("index") != selector:
        raise CacheError(f"{label} catalog index disagreement at {selector}")
    offset, width, height = records[selector]
    dimensions = catalog_entry.get("dimensions")
    if dimensions != {"height": height, "width": width}:
        raise CacheError(f"{label} TAB/catalog dimensions disagree at {selector}")
    if (
        catalog_entry.get("decoder_status") != "decoded_indexed_rle"
        or catalog_entry.get("offset") != offset
        or not isinstance(catalog_entry.get("encoded_length"), int)
    ):
        raise CacheError(f"{label} catalog extent is not qualified at {selector}")
    encoded_length = catalog_entry["encoded_length"]
    if offset < 0 or encoded_length <= 0 or offset + encoded_length > len(dat):
        raise CacheError(f"{label} catalog span exceeds source at {selector}")
    pixels, mask, actual_length = materialize_indexed_rle(
        dat, offset, width, height
    )
    proven = proven_decode(dat, offset, width, height)
    encoded = dat[offset : offset + encoded_length]
    if (
        actual_length != encoded_length
        or proven.get("status") != "decoded_indexed_rle"
        or proven.get("encoded_length") != actual_length
        or proven.get("indexed_pixels_sha256") != sha256_bytes(pixels)
        or proven.get("transparency_mask_sha256") != sha256_bytes(mask)
        or catalog_entry.get("content_address") != f"sha256:{sha256_bytes(encoded)}"
        or catalog_entry.get("indexed_pixels_sha256") != sha256_bytes(pixels)
        or catalog_entry.get("transparency_mask_sha256") != sha256_bytes(mask)
    ):
        raise CacheError(f"{label} decoder/catalog disagreement at {selector}")
    return pixels, mask, width, height, actual_length


def _level2_day_hud_products(
    stage: Path,
    outputs: list[dict[str, Any]],
    profile: dict[str, Any],
    source_bytes: dict[str, bytes],
    catalog_rows: dict[str, Any],
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """Materialize the qualified Day MSPRD/FONT1 HUD products in gameplay."""
    proven_decode = load_proven_decoder()
    palette_asset_id = "owner_data:DATA/PALD-0.DAT"
    palette_catalog = catalog_rows.get("DATA/PALD-0.DAT")
    palette = source_bytes.get("DATA/PALD-0.DAT")
    if (
        not isinstance(palette_catalog, dict)
        or palette_catalog.get("asset_id") != palette_asset_id
        or palette is None
        or len(palette) != 256 * 3
        or palette_catalog.get("sha256") != sha256_bytes(palette)
        or any(value > 63 for value in palette)
    ):
        raise CacheError("Level 2 Day HUD PALD source is not the qualified RGB6 palette")
    palette_rgb8 = bytes(value * 4 for value in palette)
    palette_rgb6_output = write_output(
        stage,
        outputs,
        "gameplay/hud/palette/pald.rgb6",
        palette,
        kind="ordered_rgb6_day_hud_palette",
        source_asset_ids=[palette_asset_id],
        extra={"entries": 256, "channel_bits": 6},
    )
    palette_rgb8_output = write_output(
        stage,
        outputs,
        "gameplay/hud/palette/pald.rgb8",
        palette_rgb8,
        kind="ordered_rgb8_day_hud_palette",
        source_asset_ids=[palette_asset_id],
        extra={"entries": 256, "channel_bits": 8, "expansion": "rgb6_value * 4"},
    )
    hud_outputs = [palette_rgb6_output, palette_rgb8_output]

    mspr_asset_prefix = "owner_data:DATA/MSPRD0-0.DAT#"
    mspr_dat = source_bytes["DATA/MSPRD0-0.DAT"]
    mspr_records = bitmap_records(source_bytes["DATA/MSPRD0-0.TAB"])
    mspr_dat_asset_id = "owner_data:DATA/MSPRD0-0.DAT"
    mspr_tab_asset_id = "owner_data:DATA/MSPRD0-0.TAB"
    bitmap_assets = []
    for selector in LEVEL2_DAY_HUD_BITMAP_SELECTORS:
        asset_id = f"{mspr_asset_prefix}{selector}"
        catalog_entry = catalog_rows.get(asset_id)
        if not isinstance(catalog_entry, dict):
            raise CacheError(f"Level 2 Day HUD MSPRD catalog entry is missing: {asset_id}")
        pixels, mask, width, height, encoded_length = (
            _level2_decode_indexed_catalog_entry(
                mspr_dat,
                mspr_records,
                selector,
                catalog_entry,
                proven_decode,
                "Level 2 Day HUD MSPRD",
            )
        )
        source_asset_ids = [mspr_dat_asset_id, mspr_tab_asset_id, palette_asset_id]
        stem = f"gameplay/hud/bitmaps/msprd0-0/{selector:03d}"
        indices_output = write_output(
            stage,
            outputs,
            f"{stem}.indices.u8",
            pixels,
            kind="indexed_image_u8_day_hud",
            source_asset_ids=source_asset_ids,
            extra={
                "selector": selector,
                "bank": "MSPRD0-0",
                "width": width,
                "height": height,
                "runtime_width": width * 2,
                "runtime_height": height * 2,
                "source_scale": 2,
                "encoded_length": encoded_length,
                "authority": "SourceMappedLive",
            },
        )
        mask_output = write_output(
            stage,
            outputs,
            f"{stem}.mask.u8",
            mask,
            kind="visibility_mask_u8_0_or_1_day_hud",
            source_asset_ids=source_asset_ids,
            extra={
                "selector": selector,
                "bank": "MSPRD0-0",
                "width": width,
                "height": height,
                "runtime_width": width * 2,
                "runtime_height": height * 2,
                "source_scale": 2,
                "encoded_length": encoded_length,
                "authority": "SourceMappedLive",
            },
        )
        hud_outputs.extend([indices_output, mask_output])
        bitmap_assets.append(
            {
                "selector": selector,
                "name": LEVEL2_DAY_HUD_BITMAP_NAMES.get(
                    selector, f"SPELL_ICON_{selector - 122:02d}"
                ),
                "bank": "MSPRD0-0",
                "source_asset_id": asset_id,
                "tab_asset_id": mspr_tab_asset_id,
                "palette_asset_id": palette_asset_id,
                "indices_path": indices_output["path"],
                "mask_path": mask_output["path"],
                "indices_sha256": indices_output["sha256"],
                "mask_sha256": mask_output["sha256"],
                "stored_width": width,
                "stored_height": height,
                "runtime_width": width * 2,
                "runtime_height": height * 2,
                "runtime_dimensions": {"width": width * 2, "height": height * 2},
                "source_scale": 2,
                "encoded_length": encoded_length,
                "indexed_pixels_sha256": sha256_bytes(pixels),
                "transparency_mask_sha256": sha256_bytes(mask),
                "row_order": "top_to_bottom",
                "origin": "top_left",
                "mask_semantics": "one_visible_zero_transparent",
                "filter": "nearest",
                "authority": "SourceMappedLive",
            }
        )

    font_dat = source_bytes["DATA/FONT1.DAT"]
    font_records = bitmap_records(source_bytes["DATA/FONT1.TAB"])
    font_blob = bytearray()
    roman_glyph_indices = sorted(
        {ord(value) + 1 for text in LEVEL2_DAY_HUD_ROMAN_TEXT for value in text}
    )
    glyph_rows: list[dict[str, Any]] = [{} for _ in font_records]
    font_asset_prefix = "owner_data:DATA/FONT1.DAT#"
    for index in roman_glyph_indices:
        asset_id = f"{font_asset_prefix}{index}"
        catalog_entry = catalog_rows.get(asset_id)
        if not isinstance(catalog_entry, dict):
            raise CacheError(f"Level 2 Day HUD FONT1 catalog entry is missing: {asset_id}")
        pixels, mask, width, height, encoded_length = (
            _level2_decode_indexed_catalog_entry(
                font_dat,
                font_records,
                index,
                catalog_entry,
                proven_decode,
                "Level 2 Day HUD FONT1",
            )
        )
        offset = len(font_blob)
        font_blob.extend(mask)
        glyph_rows[index] = {
            "index": index,
            "offset": offset,
            "length": len(mask),
            "width": width,
            "height": height,
            "mask_sha256": sha256_bytes(mask),
            "indexed_pixels_sha256": sha256_bytes(pixels),
            "encoded_length": encoded_length,
            "source_asset_id": asset_id,
        }
    if (
        len(glyph_rows) <= max(roman_glyph_indices)
        or any(not glyph_rows[index] for index in roman_glyph_indices)
    ):
        raise CacheError("Level 2 Day HUD FONT1 has no bounded Roman tier glyphs")
    font_asset_id = "owner_data:DATA/FONT1.DAT"
    font_tab_asset_id = "owner_data:DATA/FONT1.TAB"
    clrd_asset_id = "owner_data:DATA/CLRD-0.DAT"
    clrd = source_bytes.get("DATA/CLRD-0.DAT")
    if not clrd:
        raise CacheError("Level 2 Day HUD CLRD source is empty")
    font_palette_index = clrd[0]
    font_source_asset_ids = [
        font_asset_id,
        font_tab_asset_id,
        palette_asset_id,
        clrd_asset_id,
    ]
    tier_rows = []
    for tier, text in enumerate(LEVEL2_DAY_HUD_ROMAN_TEXT):
        text_bytes = text.encode("ascii")
        raster, width, height, glyph_indices = build_hud_text_raster(
            text_bytes,
            glyph_rows,
            bytes(font_blob),
            palette_rgb8,
            font_palette_index,
        )
        expected_glyph_indices = [value + 1 for value in text_bytes]
        if glyph_indices != expected_glyph_indices:
            raise CacheError(f"Level 2 Day HUD Roman glyph mapping changed for {text}")
        raster_output = write_output(
            stage,
            outputs,
            f"gameplay/hud/font/font1/roman-{text.lower()}.rgba8",
            raster,
            kind="rgba8_source_mapped_day_font1_spell_tier",
            source_asset_ids=font_source_asset_ids,
            extra={
                "tier": tier,
                "text": text,
                "glyph_indices": glyph_indices,
                "width": width,
                "height": height,
                "runtime_width": width * 2,
                "runtime_height": height * 2,
                "source_scale": 2,
                "palette_index": font_palette_index,
                "authority": "SourceMappedLive",
            },
        )
        hud_outputs.append(raster_output)
        tier_rows.append(
            {
                "tier": tier,
                "text": text,
                "text_bytes": list(text_bytes),
                "glyph_indices": glyph_indices,
                "glyphs": [
                    {
                        "index": index,
                        "source_asset_id": glyph_rows[index]["source_asset_id"],
                        "width": glyph_rows[index]["width"],
                        "height": glyph_rows[index]["height"],
                        "mask_sha256": glyph_rows[index]["mask_sha256"],
                    }
                    for index in glyph_indices
                ],
                "font_bank": "FONT1",
                "raster_path": raster_output["path"],
                "raster_sha256": raster_output["sha256"],
                "width": width,
                "height": height,
                "pixel_format": "RGBA8",
                "stored_width": width,
                "stored_height": height,
                "runtime_width": width * 2,
                "runtime_height": height * 2,
                "runtime_dimensions": {"width": width * 2, "height": height * 2},
                "source_scale": 2,
                "palette_asset_id": palette_asset_id,
                "palette_index": font_palette_index,
                "authority": "SourceMappedLive",
            }
        )
    hud_descriptor = {
        "schema": LEVEL2_DAY_HUD_SCHEMA,
        "profile": profile,
        "authority": "SourceMappedLive",
        "source": {
            "remc2_commit": PINNED_REMC2_COMMIT,
            "bitmap_bank": "MSPRD0-0",
            "palette_bank": "PALD-0",
            "font_bank": "FONT1",
            "font_glyph_index_rule": "ASCII byte + 1",
        },
        "palette": {
            "bank": "PALD-0",
            "asset_id": palette_asset_id,
            "rgb6_path": palette_rgb6_output["path"],
            "rgb6_sha256": palette_rgb6_output["sha256"],
            "rgb8_path": palette_rgb8_output["path"],
            "rgb8_sha256": palette_rgb8_output["sha256"],
            "entries": 256,
            "channel_bits": 6,
            "rgb8_expansion": "rgb6_value * 4",
        },
        "bitmap_bank": "MSPRD0-0",
        "bitmap_selectors": list(LEVEL2_DAY_HUD_BITMAP_SELECTORS),
        "bitmap_assets": bitmap_assets,
        "font_bank": "FONT1",
        "spell_tier_text": {
            "texts": list(LEVEL2_DAY_HUD_ROMAN_TEXT),
            "font_bank": "FONT1",
            "glyph_index_rule": "ASCII byte + 1",
            "font_asset_id": font_asset_id,
            "font_tab_asset_id": font_tab_asset_id,
            "palette_asset_id": palette_asset_id,
            "palette_selector_source_asset_id": clrd_asset_id,
            "palette_index": font_palette_index,
            "tiers": tier_rows,
        },
        "semantics": {
            "decoded_bitmap_products": True,
            "indexed_pixels_and_visibility_masks": True,
            "owner_palette_required": True,
            "owner_font_required": True,
            "level1_products_reused": False,
            "unknown_selectors": "fail_closed",
        },
    }
    return hud_descriptor, hud_outputs


def build_day_gameplay_cache(
    owner_root: Path,
    package_dir: Path,
    closure_path: Path,
    catalog_path: Path,
    output: Path,
) -> dict[str, Any]:
    """Materialize closure-rooted Level 2 gameplay, HUD, and text spans."""
    output = validate_output_path(output)
    profile, package, closure, catalog_rows = _level2_profile(
        package_dir, closure_path, catalog_path
    )
    scene, dynamic = _level2_closure_asset_rows(closure, catalog_rows)
    source_bytes = _level2_owner_sources(owner_root, catalog_rows)
    source_bytes.update(_level2_gameplay_sources(owner_root, catalog_rows))
    rows_by_id = {row["asset_id"]: row for row in [*scene, *dynamic]}
    for row in closure.get("sets", {}).get("realm_load_roots", {}).get("assets", []):
        relative = row.get("asset_id", "").removeprefix("owner_data:").split("#", 1)[0]
        if relative in set(LEVEL2_GAMEPLAY_SOURCE_PATHS):
            rows_by_id.setdefault(row["asset_id"], row)
    excluded = []
    selected = []
    for row in rows_by_id.values():
        asset_id = row["asset_id"]
        relative = asset_id.removeprefix("owner_data:").split("#", 1)[0]
        if relative in {"SOUND/SOUND.DAT", "SOUND/MUSIC.DAT"}:
            excluded.append({"asset_id": asset_id, "reason": "owned by L2-13C audio cache"})
            continue
        if relative not in set(LEVEL2_DAY_SOURCE_PATHS + LEVEL2_GAMEPLAY_SOURCE_PATHS):
            raise CacheError(f"Level 2 gameplay closure contains an unowned root: {asset_id}")
        selected.append(row)
    selected.sort(key=lambda row: row["asset_id"])
    stage = Path(tempfile.mkdtemp(prefix=".level2-day-gameplay-", dir=output.parent))
    try:
        outputs: list[dict[str, Any]] = []
        descriptors = []
        for row in selected:
            data, metadata = _level2_gameplay_asset_bytes(row, source_bytes, catalog_rows)
            safe = row["asset_id"].removeprefix("owner_data:").replace("/", "_").replace("#", "_")
            target = f"gameplay/{safe}.bin"
            output_row = write_output(
                stage,
                outputs,
                target,
                data,
                kind=f"level2_{row.get('kind', 'asset')}",
                source_asset_ids=[row["asset_id"]],
                extra=metadata,
            )
            descriptors.append({
                "asset_id": row["asset_id"],
                "kind": row.get("kind"),
                "output": output_row["path"],
                "content_address": row.get("content_address"),
                **metadata,
            })
        hud_descriptor, hud_outputs = _level2_day_hud_products(
            stage,
            outputs,
            profile,
            source_bytes,
            catalog_rows,
        )
        descriptor = {
            "schema": "mc2-level2-day-gameplay/v1",
            "profile": profile,
            "assets": descriptors,
            "hud": hud_descriptor,
            "hud_outputs": hud_outputs,
            "excluded": excluded,
            "source_sets": {
                "scene_reachable": len(scene),
                "dynamic_dependencies": len(dynamic),
                "materialized": len(descriptors),
                "day_hud": {
                    "bitmap_selector_count": len(LEVEL2_DAY_HUD_BITMAP_SELECTORS),
                    "roman_tier_count": len(LEVEL2_DAY_HUD_ROMAN_TEXT),
                    "output_count": len(hud_outputs),
                },
            },
            "semantics": {
                "owner_bytes_only": True,
                "selector_metadata_required": True,
                "unknown_selectors": "fail_closed",
                "hud_localization_roots": ["DATA/POINTERS.DAT", "DATA/MSPRD0-0.DAT", "DATA/MWEBD0-0.DAT", "DATA/FONT1.DAT", "LANGUAGE/L1.TXT"],
                "day_hud_products": "decoded_MSPRD0-0_bitmaps_and_FONT1_Roman_tiers",
            },
        }
        descriptor_bytes = canonical_json(descriptor)
        write_output(
            stage,
            outputs,
            "gameplay/descriptor.json",
            descriptor_bytes,
            kind="canonical_json_level2_gameplay_descriptor",
            source_asset_ids=sorted(
                {
                    *[item["asset_id"] for item in descriptors],
                    *[source for row in hud_outputs for source in row["source_asset_ids"]],
                }
            ),
        )
        manifest = {
            "schema": LEVEL2_GAMEPLAY_SCHEMA,
            "generator": {"name": "build_cache.py", "version": GENERATOR_VERSION},
            "profile": profile,
            "bindings": {
                "level_export_package_sha256": profile["package_sha256"],
                "level_asset_closure_sha256": profile["closure_sha256"],
                "owner_catalog_sha256": profile["catalog_sha256"],
                "owner_catalog_file_sha256": profile["catalog_file_sha256"],
                "remc2_commit": PINNED_REMC2_COMMIT,
                **_level2_owner_identity_bindings(profile),
            },
            "outputs": outputs,
            "cache_content_sha256": _level2_cache_identity(outputs),
        }
        manifest["manifest_payload_sha256"] = sha256_bytes(canonical_json(manifest))
        (stage / "manifest.json").write_bytes(canonical_json(manifest))
        validate_day_gameplay_cache(stage, profile["package_sha256"], profile["closure_sha256"])
        fsync_tree(stage)
        commit_stage(output, stage)
        return manifest
    except Exception:
        if stage.exists():
            shutil.rmtree(stage)
        raise


def _validate_level2_day_hud_descriptor(
    descriptor: dict[str, Any],
    profile: dict[str, Any],
    outputs: list[dict[str, Any]],
) -> set[str]:
    """Validate the additive Day HUD descriptor and every derived output row."""
    hud = descriptor.get("hud")
    if not isinstance(hud, dict) or hud.get("schema") != LEVEL2_DAY_HUD_SCHEMA:
        raise CacheError("Level 2 Day HUD descriptor schema changed")
    if hud.get("profile") != profile or hud.get("authority") != "SourceMappedLive":
        raise CacheError("Level 2 Day HUD descriptor identity changed")
    if hud.get("source") != {
        "remc2_commit": PINNED_REMC2_COMMIT,
        "bitmap_bank": "MSPRD0-0",
        "palette_bank": "PALD-0",
        "font_bank": "FONT1",
        "font_glyph_index_rule": "ASCII byte + 1",
    }:
        raise CacheError("Level 2 Day HUD source selector identity changed")
    output_by_path = {row.get("path"): row for row in outputs}
    hud_output_rows = descriptor.get("hud_outputs")
    if not isinstance(hud_output_rows, list) or not hud_output_rows:
        raise CacheError("Level 2 Day HUD output ledger is missing")
    hud_paths: set[str] = set()
    for output_row in hud_output_rows:
        path = output_row.get("path") if isinstance(output_row, dict) else None
        if (
            not isinstance(path, str)
            or path in hud_paths
            or not path.startswith("gameplay/hud/")
            or output_by_path.get(path) != output_row
        ):
            raise CacheError("Level 2 Day HUD output ledger changed")
        source_ids = output_row.get("source_asset_ids")
        if (
            not isinstance(source_ids, list)
            or not source_ids
            or any(
                not isinstance(source, str) or not source.startswith("owner_data:")
                for source in source_ids
            )
        ):
            raise CacheError(f"Level 2 Day HUD output is not owner-backed: {path}")
        hud_paths.add(path)
    if hud_paths != {
        row["path"]
        for row in outputs
        if isinstance(row.get("path"), str) and row["path"].startswith("gameplay/hud/")
    }:
        raise CacheError("Level 2 Day HUD output set changed")

    palette = hud.get("palette")
    if (
        not isinstance(palette, dict)
        or palette.get("bank") != "PALD-0"
        or palette.get("asset_id") != "owner_data:DATA/PALD-0.DAT"
        or palette.get("entries") != 256
        or palette.get("channel_bits") != 6
        or palette.get("rgb8_expansion") != "rgb6_value * 4"
    ):
        raise CacheError("Level 2 Day HUD palette descriptor changed")
    for path_key, hash_key in (
        ("rgb6_path", "rgb6_sha256"),
        ("rgb8_path", "rgb8_sha256"),
    ):
        path = palette.get(path_key)
        output_row = output_by_path.get(path)
        if (
            path not in hud_paths
            or not isinstance(output_row, dict)
            or palette.get(hash_key) != output_row.get("sha256")
            or output_row.get("bytes") != 768
        ):
            raise CacheError("Level 2 Day HUD palette output binding changed")

    expected_selectors = list(LEVEL2_DAY_HUD_BITMAP_SELECTORS)
    if hud.get("bitmap_bank") != "MSPRD0-0" or hud.get("bitmap_selectors") != expected_selectors:
        raise CacheError("Level 2 Day HUD bitmap selector set changed")
    bitmap_assets = hud.get("bitmap_assets")
    if not isinstance(bitmap_assets, list) or [
        row.get("selector") for row in bitmap_assets
    ] != expected_selectors:
        raise CacheError("Level 2 Day HUD bitmap rows changed")
    for row in bitmap_assets:
        selector = row.get("selector")
        width, height = row.get("stored_width"), row.get("stored_height")
        source_asset_id = f"owner_data:DATA/MSPRD0-0.DAT#{selector}"
        if (
            row.get("bank") != "MSPRD0-0"
            or row.get("source_asset_id") != source_asset_id
            or row.get("tab_asset_id") != "owner_data:DATA/MSPRD0-0.TAB"
            or row.get("palette_asset_id") != "owner_data:DATA/PALD-0.DAT"
            or not isinstance(width, int)
            or not isinstance(height, int)
            or width <= 0
            or height <= 0
            or row.get("source_scale") != 2
            or row.get("runtime_width") != width * 2
            or row.get("runtime_height") != height * 2
            or row.get("runtime_dimensions")
            != {"width": width * 2, "height": height * 2}
            or row.get("row_order") != "top_to_bottom"
            or row.get("origin") != "top_left"
            or row.get("mask_semantics") != "one_visible_zero_transparent"
            or row.get("authority") != "SourceMappedLive"
        ):
            raise CacheError(f"Level 2 Day HUD bitmap descriptor changed: {selector}")
        indices_path = row.get("indices_path")
        mask_path = row.get("mask_path")
        indices_output = output_by_path.get(indices_path)
        mask_output = output_by_path.get(mask_path)
        if (
            indices_path not in hud_paths
            or mask_path not in hud_paths
            or not isinstance(indices_output, dict)
            or not isinstance(mask_output, dict)
            or indices_output.get("bytes") != width * height
            or mask_output.get("bytes") != width * height
            or any(
                output.get("width") != width
                or output.get("height") != height
                or output.get("runtime_width") != width * 2
                or output.get("runtime_height") != height * 2
                or output.get("source_scale") != 2
                for output in (indices_output, mask_output)
            )
            or row.get("indices_sha256") != indices_output.get("sha256")
            or row.get("mask_sha256") != mask_output.get("sha256")
            or row.get("indexed_pixels_sha256") != indices_output.get("sha256")
            or row.get("transparency_mask_sha256") != mask_output.get("sha256")
            or indices_output.get("selector") != selector
            or mask_output.get("selector") != selector
        ):
            raise CacheError(f"Level 2 Day HUD bitmap output binding changed: {selector}")

    spell_text = hud.get("spell_tier_text")
    if (
        not isinstance(spell_text, dict)
        or spell_text.get("texts") != list(LEVEL2_DAY_HUD_ROMAN_TEXT)
        or spell_text.get("font_bank") != "FONT1"
        or spell_text.get("glyph_index_rule") != "ASCII byte + 1"
        or spell_text.get("font_asset_id") != "owner_data:DATA/FONT1.DAT"
        or spell_text.get("font_tab_asset_id") != "owner_data:DATA/FONT1.TAB"
        or spell_text.get("palette_asset_id") != "owner_data:DATA/PALD-0.DAT"
        or spell_text.get("palette_selector_source_asset_id")
        != "owner_data:DATA/CLRD-0.DAT"
        or not isinstance(spell_text.get("palette_index"), int)
        or not 0 <= spell_text["palette_index"] < 256
    ):
        raise CacheError("Level 2 Day HUD Roman font descriptor changed")
    tiers = spell_text.get("tiers")
    if not isinstance(tiers, list) or [row.get("text") for row in tiers] != list(
        LEVEL2_DAY_HUD_ROMAN_TEXT
    ):
        raise CacheError("Level 2 Day HUD Roman tier set changed")
    for tier, text in enumerate(LEVEL2_DAY_HUD_ROMAN_TEXT):
        row = tiers[tier]
        text_bytes = text.encode("ascii")
        glyph_indices = [value + 1 for value in text_bytes]
        width, height = row.get("width"), row.get("height")
        raster_path = row.get("raster_path")
        raster_output = output_by_path.get(raster_path)
        glyphs = row.get("glyphs")
        if (
            row.get("tier") != tier
            or row.get("text_bytes") != list(text_bytes)
            or row.get("glyph_indices") != glyph_indices
            or row.get("font_bank") != "FONT1"
            or row.get("palette_asset_id") != "owner_data:DATA/PALD-0.DAT"
            or row.get("palette_index") != spell_text["palette_index"]
            or row.get("pixel_format") != "RGBA8"
            or row.get("authority") != "SourceMappedLive"
            or not isinstance(width, int)
            or not isinstance(height, int)
            or width <= 0
            or height <= 0
            or row.get("stored_width") != width
            or row.get("stored_height") != height
            or row.get("source_scale") != 2
            or row.get("runtime_width") != width * 2
            or row.get("runtime_height") != height * 2
            or row.get("runtime_dimensions")
            != {"width": width * 2, "height": height * 2}
            or raster_path not in hud_paths
            or not isinstance(raster_output, dict)
            or row.get("raster_sha256") != raster_output.get("sha256")
            or raster_output.get("bytes") != width * height * 4
            or raster_output.get("width") != width
            or raster_output.get("height") != height
            or raster_output.get("runtime_width") != width * 2
            or raster_output.get("runtime_height") != height * 2
            or raster_output.get("source_scale") != 2
            or not isinstance(glyphs, list)
            or [glyph.get("index") for glyph in glyphs] != glyph_indices
        ):
            raise CacheError(f"Level 2 Day HUD Roman tier binding changed: {text}")
        for glyph in glyphs:
            index = glyph.get("index")
            if (
                glyph.get("source_asset_id")
                != f"owner_data:DATA/FONT1.DAT#{index}"
                or not isinstance(glyph.get("width"), int)
                or not isinstance(glyph.get("height"), int)
                or not isinstance(glyph.get("mask_sha256"), str)
            ):
                raise CacheError(f"Level 2 Day HUD Roman glyph binding changed: {text}")
    return hud_paths


def validate_day_gameplay_cache(
    cache_dir: Path,
    expected_package: str | None = EXPECTED_LEVEL2_PACKAGE,
    expected_closure: str | None = EXPECTED_LEVEL2_CLOSURE,
    *,
    expected_owner_identity: dict[str, Any] | None = LEVEL2_OWNER_PACKAGE_IDENTITY,
) -> dict[str, Any]:
    """Validate the profile, content identity, and descriptor of the L2 slice."""
    cache_dir = cache_dir.expanduser().resolve()
    manifest_path = cache_dir / "manifest.json"
    if manifest_path.is_symlink():
        raise CacheError("Level 2 gameplay cache contains symlink")
    manifest = load_json(manifest_path)
    claim = manifest.get("manifest_payload_sha256")
    unsigned = dict(manifest)
    unsigned.pop("manifest_payload_sha256", None)
    if sha256_bytes(canonical_json(unsigned)) != claim:
        raise CacheError("Level 2 gameplay manifest metadata hash mismatch")
    if manifest.get("schema") != LEVEL2_GAMEPLAY_SCHEMA:
        raise CacheError("unsupported Level 2 gameplay cache schema")
    profile = manifest.get("profile", {})
    if profile.get("name") != "level2-day" or profile.get("external_level") != 2 or profile.get("internal_level") != 1 or profile.get("map_type") != {"id": 0, "name": "Day"} or profile.get("spec_commit") != PINNED_REMC2_COMMIT:
        raise CacheError("Level 2 gameplay profile identity mismatch")
    if expected_package is not None and profile.get("package_sha256") != expected_package:
        raise CacheError("Level 2 gameplay package binding mismatch")
    if expected_closure is not None and profile.get("closure_sha256") != expected_closure:
        raise CacheError("Level 2 gameplay closure binding mismatch")
    bindings = manifest.get("bindings", {})
    if not isinstance(bindings, dict):
        raise CacheError("Level 2 gameplay manifest bindings are invalid")
    _validate_level2_owner_identity(profile, bindings, expected_owner_identity)
    if (
        bindings.get("level_export_package_sha256") != profile.get("package_sha256")
        or bindings.get("level_asset_closure_sha256") != profile.get("closure_sha256")
        or bindings.get("owner_catalog_sha256") != profile.get("catalog_sha256")
        or bindings.get("owner_catalog_file_sha256") != profile.get("catalog_file_sha256")
        or bindings.get("remc2_commit") != PINNED_REMC2_COMMIT
    ):
        raise CacheError("Level 2 gameplay manifest bindings mismatch")
    outputs = manifest.get("outputs")
    if not isinstance(outputs, list) or not outputs:
        raise CacheError("Level 2 gameplay cache has no outputs")
    seen: set[str] = set()
    for row in outputs:
        relative = row.get("path")
        if not isinstance(relative, str) or relative in seen or relative.startswith("/") or ".." in Path(relative).parts:
            raise CacheError("invalid or duplicate Level 2 gameplay output path")
        seen.add(relative)
        path = cache_dir / relative
        if path.is_symlink():
            raise CacheError(f"Level 2 gameplay cache contains symlink: {relative}")
        if not path.is_file() or path.stat().st_size != row.get("bytes") or sha256_file(path) != row.get("sha256"):
            raise CacheError(f"Level 2 gameplay output hash mismatch: {relative}")
    if _level2_cache_identity(outputs) != manifest.get("cache_content_sha256"):
        raise CacheError("Level 2 gameplay cache content identity mismatch")
    descriptor_path = cache_dir / "gameplay/descriptor.json"
    if descriptor_path.is_symlink() or not descriptor_path.is_file():
        raise CacheError("Level 2 gameplay descriptor is missing or symlinked")
    descriptor = load_json(descriptor_path)
    if descriptor.get("schema") != "mc2-level2-day-gameplay/v1" or descriptor.get("profile") != profile:
        raise CacheError("Level 2 gameplay descriptor identity mismatch")
    if descriptor.get("source_sets", {}).get("materialized", 0) != len(descriptor.get("assets", [])):
        raise CacheError("Level 2 gameplay descriptor materialization count mismatch")
    hud_paths = _validate_level2_day_hud_descriptor(descriptor, profile, outputs)
    day_hud_counts = descriptor.get("source_sets", {}).get("day_hud")
    if (
        not isinstance(day_hud_counts, dict)
        or day_hud_counts.get("bitmap_selector_count")
        != len(LEVEL2_DAY_HUD_BITMAP_SELECTORS)
        or day_hud_counts.get("roman_tier_count") != len(LEVEL2_DAY_HUD_ROMAN_TEXT)
        or day_hud_counts.get("output_count") != len(hud_paths)
    ):
        raise CacheError("Level 2 gameplay Day HUD materialization count changed")
    descriptor_outputs = {row.get("output") for row in descriptor.get("assets", [])} | hud_paths
    if descriptor_outputs != seen - {"gameplay/descriptor.json"}:
        raise CacheError("Level 2 gameplay descriptor/output set mismatch")
    actual_tree = {
        path.relative_to(cache_dir).as_posix()
        for path in cache_dir.rglob("*")
        if path.is_file()
    }
    if any(path.is_symlink() for path in cache_dir.rglob("*")):
        raise CacheError("Level 2 gameplay cache tree contains symlink")
    if actual_tree != {"manifest.json", *seen}:
        raise CacheError("Level 2 gameplay cache tree has missing or extra files")
    return manifest


def _level2_audio_denominator() -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    denominator = load_json(LEVEL2_AUDIO_DENOMINATOR)
    audio = denominator.get("audio", {})
    rows = audio.get("rows")
    if audio.get("denominator") != 119 or audio.get("counts") != {
        "mechanically_unreachable_level2": 93,
        "reachable_level2": 26,
    } or not isinstance(rows, list) or len(rows) != 119:
        raise CacheError("Level 2 audio denominator changed")
    if len({row.get("stable_occurrence_hash_sha256") for row in rows}) != 119:
        raise CacheError("Level 2 audio occurrence identities are not unique")
    reachable = [row for row in rows if row.get("classification") == "reachable_level2"]
    unreachable = [row for row in rows if row.get("classification") != "reachable_level2"]
    if len(reachable) != 26 or len(unreachable) != 93:
        raise CacheError("Level 2 audio reachable/unreachable partition changed")
    return reachable, unreachable


def _level2_sound_selector(expression: str) -> dict[str, Any]:
    numbers = [int(value) for value in re.findall(r"\d+", expression)]
    if expression == "v6":
        return {
            "expression": expression,
            "status": "qualified_bounded_source_selector",
            "sound_ids": [9, 15, 23],
            "source_binding": "sub_6DCA0 v6 initializes 15 and assigns 9/23 on spell branches",
        }
    if "54" in expression and "3" in expression:
        return {"expression": expression, "status": "bounded_runtime_range", "sound_ids": [54, 55, 56, 57]}
    if len(numbers) == 1:
        return {"expression": expression, "status": "qualified_constant", "sound_ids": numbers}
    return {"expression": expression, "status": "source_expression_requires_runtime_resolution", "sound_ids": numbers}


def build_day_audio_cache(
    owner_root: Path,
    package_dir: Path,
    closure_path: Path,
    catalog_path: Path,
    output: Path,
) -> dict[str, Any]:
    """Materialize the exact Level 2 sound/music banks and occurrence ledger."""
    output = validate_output_path(output)
    profile, package, closure, catalog_rows = _level2_profile(
        package_dir, closure_path, catalog_path
    )
    reachable, unreachable = _level2_audio_denominator()
    dynamic_assets = {
        row["asset_id"]: row
        for row in closure.get("sets", {}).get("dynamic_dependencies", {}).get("assets", [])
    }
    for relative in LEVEL2_AUDIO_SOURCE_PATHS:
        asset_id = f"owner_data:{relative}"
        row = dynamic_assets.get(asset_id)
        file_row = catalog_rows.get(relative)
        if not isinstance(row, dict) or not isinstance(file_row, dict):
            raise CacheError(f"Level 2 audio closure is missing {relative}")
        source_path = confined_source(owner_root, relative)
        source = source_path.read_bytes()
        if sha256_bytes(source) != file_row.get("sha256") or row.get("content_address") != f"sha256:{sha256_bytes(source)}":
            raise CacheError(f"Level 2 audio source hash mismatch: {relative}")
    sound = confined_source(owner_root, "SOUND/SOUND.DAT").read_bytes()
    music = confined_source(owner_root, "SOUND/MUSIC.DAT").read_bytes()
    stage = Path(tempfile.mkdtemp(prefix=".level2-day-audio-", dir=output.parent))
    try:
        outputs: list[dict[str, Any]] = []
        sound_output = write_output(
            stage,
            outputs,
            "audio/SOUND.DAT",
            sound,
            kind="owner_sound_bank_exact_container",
            source_asset_ids=["owner_data:SOUND/SOUND.DAT"],
            extra={"bank": 1, "reachable_occurrence_count": len(reachable)},
        )
        music_output = write_output(
            stage,
            outputs,
            "audio/MUSIC.DAT",
            music,
            kind="owner_music_bank_exact_container",
            source_asset_ids=["owner_data:SOUND/MUSIC.DAT"],
            extra={"bank": 1, "map_type": "Day", "maptype_music": 2, "channel": 0, "driver": "f"},
        )
        rnc_work = stage / ".rnc-work"
        rnc_work.mkdir()
        decoder, decoder_identity = compile_rnc_decoder(rnc_work)
        sound_counts, sound_descriptor_raw = container_descriptor(
            sound,
            count_rows=6,
            selector=1,
            row=3,
            label="level2-sound-bank-1-quality-822",
        )
        if sound_counts != [0, 0, 0, 10, 10, 0]:
            raise CacheError("Level 2 sound quality rows changed")
        sound_metadata = decode_container_blob(
            decoder, sound, sound_descriptor_raw["metadata_offset"], sound_descriptor_raw["metadata_size"], rnc_work, "level2-sound-metadata"
        )
        sound_data = decode_container_blob(
            decoder, sound, sound_descriptor_raw["data_offset"], sound_descriptor_raw["data_size"], rnc_work, "level2-sound-data"
        )
        sound_records = packed_audio_records(sound_metadata, sound_data, expected_count=64, label="level2-sound-bank-1")
        resolved_ids = sorted({sound_id for row in reachable for sound_id in _level2_sound_selector(row["sound_expression"]).get("sound_ids", [])})
        sound_record_rows = []
        for sound_id in resolved_ids:
            if not 1 <= sound_id <= len(sound_records):
                raise CacheError(f"Level 2 sound selector is outside the bank: {sound_id}")
            record = sound_records[sound_id - 1]
            payload = sound_data[record["relative_data_offset"] : record["relative_data_offset"] + record["bytes"]]
            wav = wav_properties(payload, f"Level 2 sound ID {sound_id}")
            sound_file = write_output(
                stage,
                outputs,
                f"audio/sfx/{sound_id:03d}.wav",
                payload,
                kind="owner_RIFF_WAVE_exact_level2_record",
                source_asset_ids=["owner_data:SOUND/SOUND.DAT"],
                extra={"bank": 1, "quality": 822, "quality_index": 3, "sound_id": sound_id, **wav},
            )
            sound_record_rows.append({
                "sound_id": sound_id,
                "metadata_slot": sound_id - 1,
                "filename": record["filename"],
                "relative_data_offset": record["relative_data_offset"],
                "record_bytes": record["bytes"],
                "record_sha256": record["payload_sha256"],
                "metadata_word": record["word"],
                "bank": 1,
                "quality": 822,
                "asset_path": sound_file["path"],
                "asset_sha256": sound_file["sha256"],
                "wav": wav,
            })
        music_counts, music_descriptor_raw = container_descriptor(
            music,
            count_rows=4,
            selector=0,
            row=2,
            label="level2-music-channel-0-driver-f",
        )
        if music_counts != [2, 2, 2, 2]:
            raise CacheError("Level 2 music channel rows changed")
        music_metadata = decode_container_blob(
            decoder, music, music_descriptor_raw["metadata_offset"], music_descriptor_raw["metadata_size"], rnc_work, "level2-music-metadata"
        )
        music_data = decode_container_blob(
            decoder, music, music_descriptor_raw["data_offset"], music_descriptor_raw["data_size"], rnc_work, "level2-music-data"
        )
        music_records = packed_audio_records(music_metadata, music_data, expected_count=6, label="level2-music-channel-0")
        music_payload = music_data[: music_records[0]["bytes"]]
        if music_payload[:4] != b"FORM" or music_payload[8:12] != b"XDIR":
            raise CacheError("Level 2 C2GAME1 track is not an XMIDI FORM/XDIR payload")
        music_track_output = write_output(
            stage,
            outputs,
            "audio/music/C2GAME1.xmi",
            music_payload,
            kind="owner_XMIDI_FORM_XDIR_exact_level2_record",
            source_asset_ids=["owner_data:SOUND/MUSIC.DAT"],
            extra={"logical_track": 1, "playback_slot": 0, "channel": 0, "driver": "f"},
        )
        music_record_rows = [
            {"slot": row["slot"], "filename": row["filename"], "relative_data_offset": row["relative_data_offset"], "record_bytes": row["bytes"], "record_sha256": row["payload_sha256"], "metadata_word": row["word"], "channel": 0, "driver": "f", "asset_path": music_track_output["path"] if row["slot"] == 0 else None, "asset_sha256": music_track_output["sha256"] if row["slot"] == 0 else None}
            for row in music_records
        ]
        shutil.rmtree(rnc_work)
        occurrence_rows = []
        for row in reachable:
            selector = _level2_sound_selector(row["sound_expression"])
            occurrence_rows.append({
                "id": row["id"],
                "stable_occurrence_hash_sha256": row["stable_occurrence_hash_sha256"],
                "source": {
                    "path": row["path"],
                    "pinned_line": row["pinned_line"],
                    "enclosing_symbol": row["enclosing_symbol"],
                    "symbol_ordinal_1based": row["symbol_ordinal_1based"],
                    "file_ordinal_1based": row["file_ordinal_1based"],
                },
                "sound_selector": selector,
                "owner_expression": row["owner_expression"],
                "helper_path": row["helper_path"],
                "dispatch_bindings": row["dispatch_bindings"],
                "source_asset_id": "owner_data:SOUND/SOUND.DAT",
                "shared_voice_identity": [
                    f"sound-bank-1-slot-{sound_id}" for sound_id in selector.get("sound_ids", [])
                ],
                "playback_metadata": {
                    "pitch": "source_call_site_or_runtime_expression",
                    "volume": "owner_sound_bank_default_or_source_call_site",
                    "position": "owner_expression_listener_relative_context",
                    "status": "explicit_source_metadata_boundary; no guessed constants",
                },
            })
        presentation = load_json(LEVEL2_AUDIO_DENOMINATOR).get("presentation", {})
        music_refs = [
            row for row in presentation.get("source_references", [])
            if row.get("classification") == "reachable_level2" and row.get("dependency_kind") == "music"
        ]
        descriptor = {
            "schema": "mc2-level2-day-audio/v1",
            "profile": profile,
            "sound": {
                "bank": 1,
                "container_path": sound_output["path"],
                "container_sha256": sound_output["sha256"],
                "occurrences": occurrence_rows,
                "reachable_occurrence_count": len(occurrence_rows),
                "source_ledger_denominator": 119,
                "records": sound_record_rows,
                "decoder": decoder_identity,
                "record_census_sha256": EXPECTED_LEVEL2_SOUND_RECORDS_SHA256,
            },
            "music": {
                "container_path": music_output["path"],
                "container_sha256": music_output["sha256"],
                "bank": 1,
                "map_type": "Day",
                "maptype_music": 2,
                "channel": 0,
                "driver": "f",
                "lifecycle": ["StartMusic", "FadeWarMusic", "UpdateMusic", "StopMusic"],
                "reachable_source_references": music_refs,
                "records": music_record_rows,
                "decoder": decoder_identity,
                "record_census_sha256": EXPECTED_LEVEL2_MUSIC_RECORDS_SHA256,
            },
            "excluded": [
                {
                    "id": row["id"],
                    "stable_occurrence_hash_sha256": row["stable_occurrence_hash_sha256"],
                    "classification": row["classification"],
                    "source": {"path": row["path"], "pinned_line": row["pinned_line"], "enclosing_symbol": row["enclosing_symbol"]},
                    "reason": "mechanically_unreachable_level2",
                }
                for row in unreachable
            ],
            "armed_empty": {"sound": "none_claimed_without_occurrence_evidence", "music": "no_trace_audio_observation_supplied"},
        }
        descriptor_output = write_output(
            stage,
            outputs,
            "audio/descriptor.json",
            canonical_json(descriptor),
            kind="canonical_json_level2_audio_descriptor",
            source_asset_ids=["owner_data:SOUND/SOUND.DAT", "owner_data:SOUND/MUSIC.DAT"],
            extra={"reachable_occurrences": 26, "excluded_occurrences": 93},
        )
        manifest = {
            "schema": LEVEL2_AUDIO_SCHEMA,
            "generator": {"name": "build_cache.py", "version": GENERATOR_VERSION},
            "profile": profile,
            "bindings": {
                "level_export_package_sha256": profile["package_sha256"],
                "level_asset_closure_sha256": profile["closure_sha256"],
                "owner_catalog_sha256": profile["catalog_sha256"],
                "owner_catalog_file_sha256": profile["catalog_file_sha256"],
                "remc2_commit": PINNED_REMC2_COMMIT,
                "audio_denominator_sha256": sha256_file(LEVEL2_AUDIO_DENOMINATOR),
                **_level2_owner_identity_bindings(profile),
            },
            "outputs": outputs,
            "cache_content_sha256": _level2_cache_identity(outputs),
        }
        manifest["manifest_payload_sha256"] = sha256_bytes(canonical_json(manifest))
        (stage / "manifest.json").write_bytes(canonical_json(manifest))
        validate_day_audio_cache(stage, profile["package_sha256"], profile["closure_sha256"])
        fsync_tree(stage)
        commit_stage(output, stage)
        return manifest
    except Exception:
        if stage.exists():
            shutil.rmtree(stage)
        raise


def validate_day_audio_cache(
    cache_dir: Path,
    expected_package: str | None = EXPECTED_LEVEL2_PACKAGE,
    expected_closure: str | None = EXPECTED_LEVEL2_CLOSURE,
    *,
    expected_owner_identity: dict[str, Any] | None = LEVEL2_OWNER_PACKAGE_IDENTITY,
) -> dict[str, Any]:
    cache_dir = cache_dir.expanduser().resolve()
    manifest_path = cache_dir / "manifest.json"
    if manifest_path.is_symlink():
        raise CacheError("Level 2 audio cache contains symlink")
    manifest = load_json(manifest_path)
    unsigned = dict(manifest)
    claim = unsigned.pop("manifest_payload_sha256", None)
    if sha256_bytes(canonical_json(unsigned)) != claim:
        raise CacheError("Level 2 audio manifest metadata hash mismatch")
    if manifest.get("schema") != LEVEL2_AUDIO_SCHEMA:
        raise CacheError("unsupported Level 2 audio cache schema")
    profile = manifest.get("profile", {})
    if profile.get("name") != "level2-day" or profile.get("external_level") != 2 or profile.get("internal_level") != 1 or profile.get("map_type") != {"id": 0, "name": "Day"} or profile.get("spec_commit") != PINNED_REMC2_COMMIT:
        raise CacheError("Level 2 audio profile identity mismatch")
    if profile.get("package_sha256") != expected_package:
        raise CacheError("Level 2 audio package binding mismatch")
    if profile.get("closure_sha256") != expected_closure:
        raise CacheError("Level 2 audio closure binding mismatch")
    bindings = manifest.get("bindings", {})
    if not isinstance(bindings, dict):
        raise CacheError("Level 2 audio manifest bindings are invalid")
    _validate_level2_owner_identity(profile, bindings, expected_owner_identity)
    if (
        bindings.get("level_export_package_sha256") != profile.get("package_sha256")
        or bindings.get("level_asset_closure_sha256") != profile.get("closure_sha256")
        or bindings.get("owner_catalog_sha256") != profile.get("catalog_sha256")
        or bindings.get("owner_catalog_file_sha256") != profile.get("catalog_file_sha256")
        or bindings.get("remc2_commit") != PINNED_REMC2_COMMIT
        or bindings.get("audio_denominator_sha256") != sha256_file(LEVEL2_AUDIO_DENOMINATOR)
    ):
        raise CacheError("Level 2 audio manifest bindings mismatch")
    outputs = manifest.get("outputs")
    if not isinstance(outputs, list) or len(outputs) < 3:
        raise CacheError("Level 2 audio output set changed")
    seen = set()
    for row in outputs:
        relative = row.get("path")
        if not isinstance(relative, str) or relative in seen or relative.startswith("/") or ".." in Path(relative).parts:
            raise CacheError("invalid Level 2 audio output path")
        seen.add(relative)
        path = cache_dir / relative
        if path.is_symlink() or not path.is_file() or path.stat().st_size != row.get("bytes") or sha256_file(path) != row.get("sha256"):
            raise CacheError(f"Level 2 audio output hash mismatch: {relative}")
    if _level2_cache_identity(outputs) != manifest.get("cache_content_sha256"):
        raise CacheError("Level 2 audio cache content identity mismatch")
    descriptor_path = cache_dir / "audio/descriptor.json"
    if descriptor_path.is_symlink() or not descriptor_path.is_file():
        raise CacheError("Level 2 audio descriptor is missing or symlinked")
    descriptor = load_json(descriptor_path)
    if descriptor.get("schema") != "mc2-level2-day-audio/v1" or descriptor.get("profile") != profile:
        raise CacheError("Level 2 audio descriptor identity mismatch")
    if len(descriptor.get("sound", {}).get("occurrences", [])) != 26 or len(descriptor.get("excluded", [])) != 93:
        raise CacheError("Level 2 audio occurrence partition changed")
    sound_records = descriptor.get("sound", {}).get("records", [])
    music_records = descriptor.get("music", {}).get("records", [])
    if sha256_bytes(canonical_json(sound_records)) != EXPECTED_LEVEL2_SOUND_RECORDS_SHA256:
        raise CacheError("Level 2 sound record descriptor changed")
    if sha256_bytes(canonical_json(music_records)) != EXPECTED_LEVEL2_MUSIC_RECORDS_SHA256:
        raise CacheError("Level 2 music record descriptor changed")
    if descriptor.get("music", {}).get("maptype_music") != 2 or descriptor.get("music", {}).get("channel") != 0:
        raise CacheError("Level 2 Day music selector changed")
    if descriptor.get("music", {}).get("lifecycle") != [
        "StartMusic",
        "FadeWarMusic",
        "UpdateMusic",
        "StopMusic",
    ]:
        raise CacheError("Level 2 Day music lifecycle changed")
    expected_output_paths = {
        "audio/SOUND.DAT",
        "audio/MUSIC.DAT",
        "audio/descriptor.json",
        *[row.get("asset_path") for row in descriptor.get("sound", {}).get("records", [])],
        descriptor.get("music", {}).get("records", [{}])[0].get("asset_path"),
    }
    if None in expected_output_paths or {row.get("path") for row in outputs} != expected_output_paths:
        raise CacheError("Level 2 audio record output set changed")
    output_by_path = {row["path"]: row for row in outputs}
    for record in sound_records:
        asset_path = record.get("asset_path")
        asset_sha256 = record.get("asset_sha256")
        if (
            not isinstance(asset_path, str)
            or not isinstance(asset_sha256, str)
            or output_by_path.get(asset_path, {}).get("sha256") != asset_sha256
        ):
            raise CacheError("Level 2 sound record asset binding mismatch")
    for index, record in enumerate(music_records):
        asset_path = record.get("asset_path")
        asset_sha256 = record.get("asset_sha256")
        if index == 0:
            if (
                not isinstance(asset_path, str)
                or not isinstance(asset_sha256, str)
                or output_by_path.get(asset_path, {}).get("sha256") != asset_sha256
            ):
                raise CacheError("Level 2 music record asset binding mismatch")
        elif asset_path is not None or asset_sha256 is not None:
            raise CacheError("unexpected Level 2 music record asset binding")
    expected_reachable, expected_excluded = _level2_audio_denominator()
    actual_occurrences = descriptor.get("sound", {}).get("occurrences", [])
    if [row.get("id") for row in actual_occurrences] != [row["id"] for row in expected_reachable]:
        raise CacheError("Level 2 audio reachable occurrence identity changed")
    for actual, expected in zip(actual_occurrences, expected_reachable):
        if (
            actual.get("stable_occurrence_hash_sha256") != expected["stable_occurrence_hash_sha256"]
            or actual.get("source", {}).get("path") != expected["path"]
            or actual.get("source", {}).get("pinned_line") != expected["pinned_line"]
            or actual.get("sound_selector") != _level2_sound_selector(expected["sound_expression"])
        ):
            raise CacheError(f"Level 2 audio occurrence descriptor changed: {expected['id']}")
    if [row.get("id") for row in descriptor.get("excluded", [])] != [row["id"] for row in expected_excluded]:
        raise CacheError("Level 2 audio exclusion identity changed")
    expected_music_refs = [
        row for row in load_json(LEVEL2_AUDIO_DENOMINATOR).get("presentation", {}).get("source_references", [])
        if row.get("classification") == "reachable_level2" and row.get("dependency_kind") == "music"
    ]
    if descriptor.get("music", {}).get("reachable_source_references") != expected_music_refs:
        raise CacheError("Level 2 music source-reference closure changed")
    if (
        descriptor.get("sound", {}).get("container_sha256") != output_by_path.get("audio/SOUND.DAT", {}).get("sha256")
        or descriptor.get("music", {}).get("container_sha256") != output_by_path.get("audio/MUSIC.DAT", {}).get("sha256")
    ):
        raise CacheError("Level 2 audio descriptor/container binding mismatch")
    actual_tree = {path.relative_to(cache_dir).as_posix() for path in cache_dir.rglob("*") if path.is_file()}
    if any(path.is_symlink() for path in cache_dir.rglob("*")) or actual_tree != {"manifest.json", *seen}:
        raise CacheError("Level 2 audio cache tree has missing, extra, or symlinked files")
    return manifest


def _level2_aggregate_components() -> tuple[tuple[str, str, Any], ...]:
    """Return the three production cache seams composed by the aggregate.

    Keeping this table next to the aggregate validator makes the composition
    explicit without introducing a second cache builder or duplicating any of
    the child materialization logic.
    """
    return (
        ("material", LEVEL2_MATERIAL_SCHEMA, validate_day_material_cache),
        ("gameplay", LEVEL2_GAMEPLAY_SCHEMA, validate_day_gameplay_cache),
        ("audio", LEVEL2_AUDIO_SCHEMA, validate_day_audio_cache),
    )


def _level2_historical_v4_aggregate_closure(
    package_dir: Path,
    closure_path: Path,
    catalog_path: Path,
    expected_package: str | None,
    expected_closure: str | None,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Resolve only the retained v4 closure ledger for aggregate inspection.

    This deliberately does not call ``_level2_profile``: that production
    resolver admits only v5/component-v4 packages because it may feed a new
    qualified cache.  Historical aggregate validation needs the same
    closure-to-package-to-catalog evidence, but must remain decode-only.
    """
    if not isinstance(expected_package, str) or not isinstance(expected_closure, str):
        raise CacheError("historical Level 2 inspection requires explicit package and closure identities")
    package_dir = package_dir.expanduser().resolve()
    closure_path = closure_path.expanduser().resolve()
    catalog_path = catalog_path.expanduser().resolve()
    package = load_json(package_dir / "manifest.json")
    try:
        load_proven_export_verifier()(package_dir, package)
    except Exception as error:
        # The helper is loaded dynamically and owns its own ClosureError type.
        raise CacheError(f"historical Level 2 package identity verification failed: {error}") from error
    level = package.get("level", {})
    if (
        package.get("schema") != "mc2-owner-level-export/v4"
        or level.get("external") != 2
        or level.get("internal") != 1
        or level.get("map_type") != {"id": 0, "name": "Day"}
        or package.get("source", {}).get("upstream_commit") != PINNED_REMC2_COMMIT
        or package.get("record_sizes", {}).get("player") != 662
        or package.get("package_sha256") != expected_package
    ):
        raise CacheError("historical Level 2 aggregate inspection requires owner-level-export/v4")
    players = (package_dir / "players.bin").read_bytes()
    if len(players) < 12:
        raise CacheError("historical Level 2 players.bin header is truncated")
    magic, version, record_bytes, count = struct.unpack_from("<4sHHI", players)
    if (
        magic != b"MCP1"
        or version != 3
        or record_bytes != 662
        or count != 8
        or len(players) != 12 + count * record_bytes
    ):
        raise CacheError("historical Level 2 inspection requires players component v3/662")

    closure = load_json(closure_path)
    closure_claim = closure.get("closure_sha256")
    closure_payload = dict(closure)
    closure_payload.pop("closure_sha256", None)
    closure_actual = sha256_bytes(
        json.dumps(closure_payload, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode(
            "utf-8"
        )
    )
    if closure_claim != closure_actual or closure_claim != expected_closure:
        raise CacheError("historical Level 2 closure identity mismatch")
    expected_profile = {
        "name": "level2-day",
        "external_level": 2,
        "internal_level": 1,
        "map_type": {"id": 0, "name": "day"},
        "spec_commit": PINNED_REMC2_COMMIT,
        "identity_status": "qualified",
    }
    if (
        closure.get("schema") != "mc2-level-asset-closure/v2"
        or closure.get("scope") != "PRIVATE_TRACK_P_OWNER_DATA"
        or closure.get("profile") != expected_profile
    ):
        raise CacheError("historical Level 2 closure profile selector tuple changed")
    level_export = closure.get("source", {}).get("level_export", {})
    source_files = level_export.get("files") if isinstance(level_export, dict) else None
    if (
        not isinstance(source_files, dict)
        or level_export.get("package_sha256") != expected_package
        or "manifest_schema" in level_export
        or "players_component" in level_export
        or set(source_files) != {"manifest.json", "presentation.bin", "entities.bin", "runtime.bin"}
    ):
        raise CacheError("historical Level 2 closure export binding changed")
    for name, source_file in source_files.items():
        path = package_dir / name
        if (
            not isinstance(source_file, dict)
            or source_file.get("sha256") != sha256_file(path)
            or source_file.get("size") != path.stat().st_size
        ):
            raise CacheError(f"historical Level 2 closure source member mismatch: {name}")
    catalog, catalog_file_sha256 = _level2_catalog_identity(catalog_path)
    closure_catalog = closure.get("source", {}).get("owner_catalog", {})
    if (
        catalog.get("catalog_sha256") != EXPECTED_LEVEL2_CATALOG
        or catalog_file_sha256 != EXPECTED_LEVEL2_CATALOG_FILE
        or closure_catalog.get("catalog_sha256") != catalog.get("catalog_sha256")
        or closure_catalog.get("file_sha256") != catalog_file_sha256
    ):
        raise CacheError("historical Level 2 catalogue does not match closure binding")
    files = {row.get("path"): row for row in catalog.get("files", [])}
    entries = {row.get("asset_id"): row for row in catalog.get("entries", [])}
    return closure, {**files, **entries}


def _level2_aggregate_allowed_source_ids(
    package_dir: Path = DEFAULT_LEVEL2_PACKAGE,
    closure_path: Path = DEFAULT_LEVEL2_CLOSURE,
    catalog_path: Path = DEFAULT_CATALOG,
    *,
    expected_package: str | None = EXPECTED_LEVEL2_PACKAGE,
    expected_closure: str | None = EXPECTED_LEVEL2_CLOSURE,
    expected_owner_identity: dict[str, Any] | None = LEVEL2_OWNER_PACKAGE_IDENTITY,
) -> set[str]:
    """Return the sealed owner-data IDs for production or retained inspection."""
    if expected_owner_identity is None:
        closure, catalog_rows = _level2_historical_v4_aggregate_closure(
            package_dir,
            closure_path,
            catalog_path,
            expected_package,
            expected_closure,
        )
    else:
        _profile, _package, closure, catalog_rows = _level2_profile(
            package_dir, closure_path, catalog_path
        )
    scene, dynamic = _level2_closure_asset_rows(closure, catalog_rows)
    allowed = {
        row["asset_id"]
        for row in [*scene, *dynamic]
        if isinstance(row, dict) and isinstance(row.get("asset_id"), str)
    }
    for relative in (
        *LEVEL2_DAY_SOURCE_PATHS,
        *LEVEL2_GAMEPLAY_SOURCE_PATHS,
        *LEVEL2_AUDIO_SOURCE_PATHS,
    ):
        allowed.add(f"owner_data:{relative}")
    return allowed


def _level2_aggregate_output_rows(
    stage: Path,
    child_manifests: dict[str, dict[str, Any]],
) -> tuple[list[dict[str, Any]], dict[str, dict[str, Any]]]:
    """Namespace child manifests and return one deterministic output ledger."""
    outputs: list[dict[str, Any]] = []
    components: dict[str, dict[str, Any]] = {}
    for name, schema, _validator in _level2_aggregate_components():
        root = stage / "components" / name
        manifest_path = root / "manifest.json"
        if manifest_path.is_symlink() or not manifest_path.is_file():
            raise CacheError(f"Level 2 aggregate child manifest is missing: {name}")
        child = child_manifests[name]
        if child.get("schema") != schema:
            raise CacheError(f"Level 2 aggregate child schema changed: {name}")
        child_manifest_rel = f"components/{name}/manifest.json"
        outputs.append(
            {
                "path": child_manifest_rel,
                "kind": "level2_child_manifest",
                "bytes": manifest_path.stat().st_size,
                "sha256": sha256_file(manifest_path),
                "source_asset_ids": [f"level2_component:{name}"],
                "component": name,
            }
        )
        child_rows = child.get("outputs")
        if not isinstance(child_rows, list) or not child_rows:
            raise CacheError(f"Level 2 aggregate child has no outputs: {name}")
        for child_row in child_rows:
            relative = child_row.get("path")
            if (
                not isinstance(relative, str)
                or relative.startswith("/")
                or ".." in Path(relative).parts
            ):
                raise CacheError(f"invalid {name} child output path")
            source_ids = child_row.get("source_asset_ids")
            if (
                not isinstance(source_ids, list)
                or not source_ids
                or any(
                    not isinstance(source, str) or not source.startswith("owner_data:")
                    for source in source_ids
                )
            ):
                raise CacheError(f"{name} child output is not closure-backed: {relative}")
            child_path = root / relative
            if child_path.is_symlink() or not child_path.is_file():
                raise CacheError(f"{name} child output is missing: {relative}")
            aggregate_row = dict(child_row)
            aggregate_row["path"] = f"components/{name}/{relative}"
            aggregate_row["component"] = name
            outputs.append(aggregate_row)
        components[name] = {
            "schema": schema,
            "manifest_path": child_manifest_rel,
            "manifest_sha256": sha256_file(manifest_path),
            "cache_content_sha256": child.get("cache_content_sha256"),
            "output_count": len(child_rows),
        }
    outputs.sort(key=lambda row: row["path"])
    return outputs, components


def build_day_aggregate_cache(
    owner_root: Path,
    package_dir: Path,
    closure_path: Path,
    catalog_path: Path,
    output: Path,
) -> dict[str, Any]:
    """Compose the qualified Level 2 Day material/gameplay/audio caches.

    Each child is still built and validated by its existing production path.
    The aggregate only namespaces those outputs and seals their identities in
    one private, content-addressed manifest.
    """
    output = validate_output_path(output)
    profile, _package, _closure, _catalog_rows = _level2_profile(
        package_dir, closure_path, catalog_path
    )
    stage = Path(tempfile.mkdtemp(prefix=".level2-day-aggregate-", dir=output.parent))
    try:
        component_root = stage / "components"
        component_root.mkdir(parents=True, exist_ok=True)
        builders = {
            "material": build_day_material_cache,
            "gameplay": build_day_gameplay_cache,
            "audio": build_day_audio_cache,
        }
        child_manifests: dict[str, dict[str, Any]] = {}
        for name, _schema, validator in _level2_aggregate_components():
            child_output = component_root / name
            child_manifests[name] = builders[name](
                owner_root,
                package_dir,
                closure_path,
                catalog_path,
                child_output,
            )
            validator(child_output, profile["package_sha256"], profile["closure_sha256"])
        outputs, components = _level2_aggregate_output_rows(stage, child_manifests)
        manifest: dict[str, Any] = {
            "schema": LEVEL2_AGGREGATE_SCHEMA,
            "generator": {"name": "build_cache.py", "version": GENERATOR_VERSION},
            "profile": profile,
            "bindings": {
                "level_export_package_sha256": profile["package_sha256"],
                "level_asset_closure_sha256": profile["closure_sha256"],
                "owner_catalog_sha256": profile["catalog_sha256"],
                "owner_catalog_file_sha256": profile["catalog_file_sha256"],
                "remc2_commit": PINNED_REMC2_COMMIT,
                **_level2_owner_identity_bindings(profile),
            },
            "components": components,
            "outputs": outputs,
            "cache_content_sha256": _level2_cache_identity(outputs),
        }
        manifest["manifest_payload_sha256"] = sha256_bytes(canonical_json(manifest))
        (stage / "manifest.json").write_bytes(canonical_json(manifest))
        validate_day_aggregate_cache(
            stage,
            profile["package_sha256"],
            profile["closure_sha256"],
            package_dir=package_dir,
            closure_path=closure_path,
            catalog_path=catalog_path,
        )
        fsync_tree(stage)
        commit_stage(output, stage)
        return manifest
    except Exception:
        if stage.exists():
            shutil.rmtree(stage)
        raise


def validate_day_aggregate_cache(
    cache_dir: Path,
    expected_package: str | None = EXPECTED_LEVEL2_PACKAGE,
    expected_closure: str | None = EXPECTED_LEVEL2_CLOSURE,
    *,
    expected_owner_identity: dict[str, Any] | None = LEVEL2_OWNER_PACKAGE_IDENTITY,
    package_dir: Path = DEFAULT_LEVEL2_PACKAGE,
    closure_path: Path = DEFAULT_LEVEL2_CLOSURE,
    catalog_path: Path = DEFAULT_CATALOG,
) -> dict[str, Any]:
    """Validate the sealed, namespaced Level 2 Day owner-cache composition."""
    cache_dir = cache_dir.expanduser().resolve()
    validate_output_path(cache_dir)
    manifest_path = cache_dir / "manifest.json"
    if manifest_path.is_symlink():
        raise CacheError("Level 2 aggregate manifest is symlinked")
    manifest = load_json(manifest_path)
    unsigned = dict(manifest)
    claim = unsigned.pop("manifest_payload_sha256", None)
    if sha256_bytes(canonical_json(unsigned)) != claim:
        raise CacheError("Level 2 aggregate manifest metadata hash mismatch")
    if manifest.get("schema") != LEVEL2_AGGREGATE_SCHEMA:
        raise CacheError("unsupported Level 2 aggregate cache schema")
    expected_profile = {
        "name": "level2-day",
        "external_level": 2,
        "internal_level": 1,
        "map_type": {"id": 0, "name": "Day"},
        "selectors": {
            "vga_resolution_selector": 1,
            "texture_size": 32,
            "terrain_graphics_flags": 0,
            "terrain_graphics_flag_2": False,
            "terrain_bank": "TMAPS0-0",
            "sprite_bank": "MSPRD0-0",
            "web_bank": "MWEBD0-0",
            "block_bank": "BL32N0-0",
            "palette": "PALD-0",
            "sky_bank": "SKYD0-0",
        },
        "package_sha256": expected_package,
        "closure_sha256": expected_closure,
        "catalog_sha256": EXPECTED_LEVEL2_CATALOG,
        "catalog_file_sha256": EXPECTED_LEVEL2_CATALOG_FILE,
        "spec_commit": PINNED_REMC2_COMMIT,
    }
    if expected_owner_identity is not None:
        expected_profile["owner_package"] = expected_owner_identity
    if manifest.get("profile") != expected_profile:
        raise CacheError("Level 2 aggregate profile identity mismatch")
    bindings = manifest.get("bindings", {})
    expected_bindings = {
        "level_export_package_sha256": expected_package,
        "level_asset_closure_sha256": expected_closure,
        "owner_catalog_sha256": EXPECTED_LEVEL2_CATALOG,
        "owner_catalog_file_sha256": EXPECTED_LEVEL2_CATALOG_FILE,
        "remc2_commit": PINNED_REMC2_COMMIT,
    }
    if expected_owner_identity is not None:
        expected_bindings.update(
            _level2_owner_identity_bindings(
                {"owner_package": expected_owner_identity}
            )
        )
    if bindings != expected_bindings:
        raise CacheError("Level 2 aggregate binding identity mismatch")
    if not isinstance(bindings, dict):
        raise CacheError("Level 2 aggregate bindings are invalid")
    _validate_level2_owner_identity(
        manifest["profile"], bindings, expected_owner_identity
    )
    components = manifest.get("components")
    expected_components = {name for name, _schema, _validator in _level2_aggregate_components()}
    if not isinstance(components, dict) or set(components) != expected_components:
        raise CacheError("Level 2 aggregate component set changed")
    outputs = manifest.get("outputs")
    if not isinstance(outputs, list) or not outputs:
        raise CacheError("Level 2 aggregate cache has no outputs")
    paths = [row.get("path") for row in outputs]
    if paths != sorted(paths) or any(
        not isinstance(path, str)
        or path.startswith("/")
        or ".." in Path(path).parts
        for path in paths
    ) or len(paths) != len(set(paths)):
        raise CacheError("invalid or unordered Level 2 aggregate output path")
    for row in outputs:
        relative = row["path"]
        path = cache_dir / relative
        if path.is_symlink() or not path.is_file():
            raise CacheError(f"Level 2 aggregate output missing or symlinked: {relative}")
        if path.stat().st_size != row.get("bytes") or sha256_file(path) != row.get("sha256"):
            raise CacheError(f"Level 2 aggregate output hash mismatch: {relative}")
    if _level2_cache_identity(outputs) != manifest.get("cache_content_sha256"):
        raise CacheError("Level 2 aggregate cache content identity mismatch")
    output_by_path = {row["path"]: row for row in outputs}
    allowed_source_ids = _level2_aggregate_allowed_source_ids(
        package_dir,
        closure_path,
        catalog_path,
        expected_package=expected_package,
        expected_closure=expected_closure,
        expected_owner_identity=expected_owner_identity,
    )
    expected_output_paths: set[str] = set()
    for name, schema, validator in _level2_aggregate_components():
        metadata = components.get(name)
        expected_manifest_rel = f"components/{name}/manifest.json"
        if (
            not isinstance(metadata, dict)
            or metadata.get("schema") != schema
            or metadata.get("manifest_path") != expected_manifest_rel
        ):
            raise CacheError(f"Level 2 aggregate {name} component binding changed")
        child_root = cache_dir / "components" / name
        child_manifest_path = child_root / "manifest.json"
        child = validator(
            child_root,
            expected_package,
            expected_closure,
            expected_owner_identity=expected_owner_identity,
        )
        if child.get("profile") != manifest.get("profile"):
            raise CacheError(f"Level 2 aggregate {name} profile binding mismatch")
        child_bindings = child.get("bindings", {})
        expected_child_binding_keys = set(bindings)
        if name == "audio":
            expected_child_binding_keys.add("audio_denominator_sha256")
        if set(child_bindings) != expected_child_binding_keys:
            raise CacheError(f"Level 2 aggregate {name} binding keys changed")
        for binding_key, binding_value in bindings.items():
            if child_bindings.get(binding_key) != binding_value:
                raise CacheError(f"Level 2 aggregate {name} binding mismatch: {binding_key}")
        if name == "audio" and child_bindings.get("audio_denominator_sha256") != sha256_file(LEVEL2_AUDIO_DENOMINATOR):
            raise CacheError("Level 2 aggregate audio denominator binding mismatch")
        child_sha = sha256_file(child_manifest_path)
        if (
            metadata.get("manifest_sha256") != child_sha
            or metadata.get("cache_content_sha256") != child.get("cache_content_sha256")
            or metadata.get("output_count") != len(child.get("outputs", []))
        ):
            raise CacheError(f"Level 2 aggregate {name} child identity mismatch")
        child_manifest_row = output_by_path.get(expected_manifest_rel)
        if (
            child_manifest_row is None
            or child_manifest_row.get("sha256") != child_sha
            or child_manifest_row.get("bytes") != child_manifest_path.stat().st_size
            or child_manifest_row.get("kind") != "level2_child_manifest"
            or child_manifest_row.get("component") != name
            or child_manifest_row.get("source_asset_ids") != [f"level2_component:{name}"]
        ):
            raise CacheError(f"Level 2 aggregate {name} manifest binding mismatch")
        expected_output_paths.add(expected_manifest_rel)
        child_rows = child.get("outputs", [])
        for child_row in child_rows:
            relative = child_row["path"]
            source_ids = child_row.get("source_asset_ids")
            if (
                not isinstance(source_ids, list)
                or not source_ids
                or any(
                    not isinstance(source, str) or not source.startswith("owner_data:")
                    for source in source_ids
                )
            ):
                raise CacheError(f"Level 2 aggregate {name} child output is not closure-backed: {relative}")
            if any(source not in allowed_source_ids for source in source_ids):
                raise CacheError(f"Level 2 aggregate {name} child output is outside closure: {relative}")
            aggregate_path = f"components/{name}/{relative}"
            aggregate_row = output_by_path.get(aggregate_path)
            if aggregate_row is None:
                raise CacheError(f"Level 2 aggregate child output missing: {aggregate_path}")
            for key, value in child_row.items():
                if key == "path":
                    continue
                if aggregate_row.get(key) != value:
                    raise CacheError(f"Level 2 aggregate child output binding changed: {aggregate_path}")
            if aggregate_row.get("path") != aggregate_path:
                raise CacheError(f"Level 2 aggregate child output path changed: {aggregate_path}")
            if aggregate_row.get("component") != name:
                raise CacheError(f"Level 2 aggregate child component tag changed: {aggregate_path}")
            expected_output_paths.add(aggregate_path)
    if set(paths) != expected_output_paths:
        raise CacheError("Level 2 aggregate output set changed")
    expected_tree = {"manifest.json", *paths}
    actual_tree: set[str] = set()
    for path in cache_dir.rglob("*"):
        if path.is_symlink():
            raise CacheError("Level 2 aggregate cache tree contains symlink")
        if path.is_file():
            actual_tree.add(path.relative_to(cache_dir).as_posix())
    if actual_tree != expected_tree:
        raise CacheError("Level 2 aggregate cache tree has missing or extra files")
    return manifest


def validate_day_material_cache(
    cache_dir: Path,
    expected_package: str | None = EXPECTED_LEVEL2_PACKAGE,
    expected_closure: str | None = EXPECTED_LEVEL2_CLOSURE,
    *,
    expected_owner_identity: dict[str, Any] | None = LEVEL2_OWNER_PACKAGE_IDENTITY,
) -> dict[str, Any]:
    """Validate a profile-bound Level 2 base-material cache and its tree."""
    cache_dir = cache_dir.expanduser().resolve()
    manifest = load_json(cache_dir / "manifest.json")
    claim = manifest.get("manifest_payload_sha256")
    unsigned = dict(manifest)
    unsigned.pop("manifest_payload_sha256", None)
    if sha256_bytes(canonical_json(unsigned)) != claim:
        raise CacheError("Level 2 material manifest metadata hash mismatch")
    if manifest.get("schema") != LEVEL2_MATERIAL_SCHEMA:
        raise CacheError("unsupported Level 2 material cache schema")
    profile = manifest.get("profile", {})
    if (
        profile.get("name") != "level2-day"
        or profile.get("external_level") != 2
        or profile.get("internal_level") != 1
        or profile.get("map_type") != {"id": 0, "name": "Day"}
        or profile.get("spec_commit") != PINNED_REMC2_COMMIT
    ):
        raise CacheError("Level 2 material profile identity mismatch")
    if profile.get("selectors") != {
        "vga_resolution_selector": 1,
        "texture_size": 32,
        "terrain_graphics_flags": 0,
        "terrain_graphics_flag_2": False,
        "terrain_bank": "TMAPS0-0",
        "sprite_bank": "MSPRD0-0",
        "web_bank": "MWEBD0-0",
        "block_bank": "BL32N0-0",
        "palette": "PALD-0",
        "sky_bank": "SKYD0-0",
    }:
        raise CacheError("Level 2 material profile selector identity mismatch")
    bindings = manifest.get("bindings", {})
    if not isinstance(bindings, dict):
        raise CacheError("Level 2 material bindings are invalid")
    _validate_level2_owner_identity(profile, bindings, expected_owner_identity)
    if (
        bindings.get("level_export_package_sha256") != profile.get("package_sha256")
        or bindings.get("level_asset_closure_sha256") != profile.get("closure_sha256")
        or bindings.get("owner_catalog_sha256") != profile.get("catalog_sha256")
        or bindings.get("owner_catalog_file_sha256") != profile.get("catalog_file_sha256")
        or bindings.get("remc2_commit") != PINNED_REMC2_COMMIT
    ):
        raise CacheError("Level 2 material binding identity mismatch")
    if expected_package is not None and profile.get("package_sha256") != expected_package:
        raise CacheError("Level 2 material package binding mismatch")
    if expected_closure is not None and profile.get("closure_sha256") != expected_closure:
        raise CacheError("Level 2 material closure binding mismatch")
    outputs = manifest.get("outputs")
    if not isinstance(outputs, list) or not outputs:
        raise CacheError("Level 2 material cache has no outputs")
    seen: set[str] = set()
    for row in outputs:
        relative = row.get("path")
        if (
            not isinstance(relative, str)
            or relative in seen
            or relative.startswith("/")
            or ".." in Path(relative).parts
        ):
            raise CacheError("invalid or duplicate Level 2 material output path")
        seen.add(relative)
        path = cache_dir / relative
        if path.is_symlink():
            raise CacheError(f"Level 2 material cache contains symlink: {relative}")
        if not path.is_file() or path.stat().st_size != row.get("bytes"):
            raise CacheError(f"Level 2 material output missing or truncated: {relative}")
        if sha256_file(path) != row.get("sha256"):
            raise CacheError(f"Level 2 material output hash mismatch: {relative}")
    if _level2_cache_identity(outputs) != manifest.get("cache_content_sha256"):
        raise CacheError("Level 2 material cache content identity mismatch")
    descriptor_path = cache_dir / "materials/descriptor.json"
    if not descriptor_path.is_file():
        raise CacheError("Level 2 material descriptor is missing")
    descriptor = load_json(descriptor_path)
    if descriptor.get("schema") != "mc2-level2-day-materials/v1":
        raise CacheError("Level 2 material descriptor schema mismatch")
    if descriptor.get("profile") != profile:
        raise CacheError("Level 2 material descriptor profile mismatch")
    for key, expected in {
        "palette": ("entries", 256),
        "tablesd": ("bytes", 83456),
        "clrd": ("entries", 4096),
        "bl32n": ("tile_count", 152),
        "skyd": ("width", 256),
    }.items():
        value = descriptor.get(key)
        if not isinstance(value, dict) or value.get(expected[0]) != expected[1]:
            raise CacheError(f"Level 2 material descriptor bounds changed: {key}")
    if (
        descriptor["skyd"].get("height") != 256
        or descriptor["bl32n"].get("tile_width") != 32
        or descriptor["bl32n"].get("tile_height") != 32
        or descriptor["bl32n"].get("atlas_width") != 256
        or descriptor["bl32n"].get("atlas_height") != 608
    ):
        raise CacheError("Level 2 indexed material dimensions changed")
    tmap_descriptor = descriptor.get("tmaps0")
    tmap_selectors = tmap_descriptor.get("selectors") if isinstance(tmap_descriptor, dict) else None
    tmap_rows = tmap_descriptor.get("textures") if isinstance(tmap_descriptor, dict) else None
    if (
        not isinstance(tmap_selectors, list)
        or not tmap_selectors
        or any(not isinstance(index, int) or not 0 <= index < 504 for index in tmap_selectors)
        or tmap_selectors != sorted(set(tmap_selectors))
        or not isinstance(tmap_rows, list)
        or any(not isinstance(row, dict) for row in tmap_rows)
        or any(not isinstance(row.get("index"), int) for row in tmap_rows)
        or sorted(row.get("index") for row in tmap_rows) != tmap_selectors
    ):
        raise CacheError("Level 2 TMAP selector bounds or rows changed")
    gpu = descriptor.get("gpu_ready")
    if not isinstance(gpu, dict) or gpu.get("indexed_format") != "R8_UNORM":
        raise CacheError("Level 2 GPU-ready indexed descriptor is missing")
    expected_tree = {"manifest.json", *seen}
    actual_tree: set[str] = set()
    for path in cache_dir.rglob("*"):
        if path.is_symlink():
            raise CacheError("Level 2 material cache tree contains symlink")
        if path.is_file():
            actual_tree.add(path.relative_to(cache_dir).as_posix())
    if actual_tree != expected_tree:
        raise CacheError("Level 2 material cache tree has missing or extra files")
    return manifest


def build_day_material_cache(
    owner_root: Path,
    package_dir: Path,
    closure_path: Path,
    catalog_path: Path,
    output: Path,
) -> dict[str, Any]:
    """Materialize the qualified Level 2 Day base material profile.

    This intentionally stops at indexed palette/table/tile/sky/TMAP material
    descriptors.  Sprites and web banks are bound as roots for the later
    presentation slice; gameplay, HUD, audio, and Unity scene files remain out
    of this bead.
    """
    output = validate_output_path(output)
    profile, _, closure, catalog_rows = _level2_profile(
        package_dir, closure_path, catalog_path
    )
    file_rows = {
        path: row for path, row in catalog_rows.items() if isinstance(row, dict) and "sha256" in row
    }
    source_bytes = _level2_owner_sources(owner_root, file_rows)
    package_hash = profile["package_sha256"]
    closure_hash = profile["closure_sha256"]
    selectors = _level2_tmap_selectors(closure)
    stage = Path(tempfile.mkdtemp(prefix=".level2-day-material-", dir=output.parent))
    try:
        outputs: list[dict[str, Any]] = []
        palette = source_bytes["DATA/PALD-0.DAT"]
        if len(palette) != 768 or max(palette) > 63:
            raise CacheError("PALD is not 256 RGB6 triplets")
        palette_rgb8 = bytes(value * 4 for value in palette)
        palette_output = write_output(
            stage, outputs, "palette/pald.rgb6", palette,
            kind="ordered_rgb6_palette", source_asset_ids=["owner_data:DATA/PALD-0.DAT"],
            extra={"entries": 256, "channel_bits": 6},
        )
        rgb8_output = write_output(
            stage, outputs, "palette/pald.rgb8", palette_rgb8,
            kind="ordered_rgb8_palette_exact_vga_scale",
            source_asset_ids=["owner_data:DATA/PALD-0.DAT"],
            extra={"entries": 256, "conversion": "channel_u8 = channel_rgb6 * 4"},
        )
        tablesd = source_bytes["DATA/TABLESD.DAT"]
        if len(tablesd) != 83456 or max(tablesd) > 255:
            raise CacheError("TABLESD bounds changed")
        tables_output = write_output(
            stage, outputs, "palette/tablesd.u8", tablesd,
            kind="remc2_day_renderer_tables_u8", source_asset_ids=["owner_data:DATA/TABLESD.DAT"],
            extra={"bytes": len(tablesd), "shade_lut_offset": 0, "shade_lut_bytes": 65536},
        )
        shade_output = write_output(
            stage, outputs, "palette/tablesd.shade-lut.indices.u8", tablesd[:65536],
            kind="indexed_palette_shade_lut_256x256_u8",
            source_asset_ids=["owner_data:DATA/TABLESD.DAT"],
            extra={"width": 256, "height": 256, "x": "source_palette_index_u8", "y": "brightness_u8"},
        )
        clrd = source_bytes["DATA/CLRD-0.DAT"]
        if len(clrd) != 4096 or max(clrd) > 255:
            raise CacheError("CLRD bounds changed")
        clrd_output = write_output(
            stage, outputs, "palette/clrd.indices.u8", clrd,
            kind="day_rgb444_to_palette_index_lut_u8", source_asset_ids=["owner_data:DATA/CLRD-0.DAT"],
            extra={"entries": 4096, "index_semantics": "remc2 Day 12-bit colour lookup"},
        )
        sky = source_bytes["DATA/SKYD0-0.DAT"]
        if len(sky) != 256 * 256 or max(sky) > 255:
            raise CacheError("SKYD bounds changed")
        sky_output = write_output(
            stage, outputs, "sky/skyd.indices.u8", sky,
            kind="indexed_image_u8", source_asset_ids=["owner_data:DATA/SKYD0-0.DAT"],
            extra={"width": 256, "height": 256, "palette_asset_id": "owner_data:DATA/PALD-0.DAT"},
        )
        bl32 = source_bytes["DATA/BL32N0-0.DAT"]
        if len(bl32) != 152 * BL32_TILE_BYTES:
            raise CacheError("Level 2 BL32N bank must contain exactly 152 32x32 tiles")
        tiles = bytearray()
        for tile in range(152):
            x = (tile % BL32_ATLAS_COLUMNS) * BL32_TILE_SIDE
            y = (tile // BL32_ATLAS_COLUMNS) * BL32_TILE_SIDE
            for row in range(BL32_TILE_SIDE):
                start = (y + row) * BL32_ATLAS_WIDTH + x
                tiles.extend(bl32[start:start + BL32_TILE_SIDE])
        atlas_output = write_output(
            stage, outputs, "terrain/bl32n.atlas.indices.u8", bl32,
            kind="indexed_atlas_u8", source_asset_ids=["owner_data:DATA/BL32N0-0.DAT"],
            extra={"width": 256, "height": 608, "tile_width": 32, "tile_height": 32, "tile_columns": 8, "tile_count": 152, "palette_asset_id": "owner_data:DATA/PALD-0.DAT"},
        )
        tiles_output = write_output(
            stage, outputs, "terrain/bl32n.indices.u8", bytes(tiles),
            kind="indexed_tile_bank_u8", source_asset_ids=["owner_data:DATA/BL32N0-0.DAT"],
            extra={"tile_width": 32, "tile_height": 32, "tile_count": 152, "palette_asset_id": "owner_data:DATA/PALD-0.DAT"},
        )
        tmap_dat = source_bytes["DATA/TMAPS0-0.DAT"]
        tmap_tab = tmap_records(source_bytes["DATA/TMAPS0-0.TAB"])
        if len(tmap_tab) != 505:
            raise CacheError("TMAPS0 TAB must contain 504 rows plus sentinel")
        tmap_rows: list[dict[str, Any]] = []
        decoder_dir = stage / ".rnc-work"
        decoder_dir.mkdir()
        decoder, decoder_identity = compile_rnc_decoder(decoder_dir)
        entry_rows = {key: row for key, row in catalog_rows.items() if isinstance(row, dict) and key.startswith("owner_data:")}
        for index in selectors:
            metadata, offset, group = tmap_tab[index]
            _, end_offset, _ = tmap_tab[index + 1]
            if offset < 0 or end_offset < offset or end_offset > len(tmap_dat):
                raise CacheError(f"TMAPS0 packed span is invalid at index {index}")
            packed = tmap_dat[offset:end_offset]
            asset_id = f"owner_data:DATA/TMAPS0-0.DAT#{index}"
            catalog_entry = entry_rows.get(asset_id)
            if not isinstance(catalog_entry, dict) or catalog_entry.get("offset") != offset or catalog_entry.get("end_offset") != end_offset or catalog_entry.get("metadata_dword_0") != metadata or catalog_entry.get("animation_or_allocation_group") != group or catalog_entry.get("packed_span_sha256") != sha256_bytes(packed):
                raise CacheError(f"TMAPS0 TAB/catalog disagreement at index {index}")
            decoded = decode_tmap_rnc(decoder, packed, decoder_dir, index)
            if len(decoded) < TMAP_HEADER.size:
                raise CacheError(f"TMAPS0 decoded header is truncated at index {index}")
            header_word, width, height = TMAP_HEADER.unpack_from(decoded)
            first_frame_end = TMAP_HEADER.size + width * height
            if not width or not height or metadata != len(decoded) or len(decoded) < first_frame_end:
                raise CacheError(f"TMAPS0 dimensions disagree at index {index}")
            if header_word & 1:
                if len(decoded) < first_frame_end + 2:
                    raise CacheError(f"TMAPS0 animated row has no frame stream at index {index}")
                frame_count = struct.unpack_from("<H", decoded, first_frame_end)[0]
                if frame_count == 0:
                    raise CacheError(f"TMAPS0 animated row has zero frames at index {index}")
            elif len(decoded) != first_frame_end:
                raise CacheError(f"TMAPS0 static row has trailing bytes at index {index}")
            decoded_output = write_output(
                stage, outputs, f"terrain/tmaps0/{index:03d}.decoded.bin", decoded,
                kind="remc2_tmap_decoded_payload", source_asset_ids=[asset_id],
                extra={"index": index, "header_word": header_word, "width": width, "height": height, "animated": bool(header_word & 1), "frame_count": frame_count if header_word & 1 else 1},
            )
            indices_output = write_output(
                stage, outputs, f"terrain/tmaps0/{index:03d}.indices.u8", decoded[TMAP_HEADER.size:first_frame_end],
                kind="indexed_image_u8", source_asset_ids=[asset_id],
                extra={"index": index, "width": width, "height": height, "palette_asset_id": "owner_data:DATA/PALD-0.DAT"},
            )
            tmap_rows.append({"index": index, "group": group, "width": width, "height": height, "animated": bool(header_word & 1), "frame_count": frame_count if header_word & 1 else 1, "decoded_payload_path": decoded_output["path"], "indices_path": indices_output["path"], "source_asset_id": asset_id, "row_order": "top_to_bottom", "origin": "top_left", "palette_asset_id": "owner_data:DATA/PALD-0.DAT"})
        # Compiler/source scratch is provenance only; it must never become an
        # unlisted cache output or alter the content-addressed tree.
        shutil.rmtree(decoder_dir)
        descriptor = {
            "schema": "mc2-level2-day-materials/v1",
            "profile": profile,
            "palette": {"path": palette_output["path"], "rgb8_path": rgb8_output["path"], "entries": 256, "channel_bits": 6, "conversion": "channel_u8 = channel_rgb6 * 4"},
            "tablesd": {"path": tables_output["path"], "shade_lut_path": shade_output["path"], "bytes": len(tablesd), "entries": 65536, "shade_lut_entries": 65536},
            "clrd": {"path": clrd_output["path"], "entries": 4096},
            "bl32n": {"path": tiles_output["path"], "atlas_path": atlas_output["path"], "tile_count": 152, "tile_width": 32, "tile_height": 32, "atlas_width": 256, "atlas_height": 608, "palette_asset_id": "owner_data:DATA/PALD-0.DAT"},
            "skyd": {"path": sky_output["path"], "width": 256, "height": 256, "palette_asset_id": "owner_data:DATA/PALD-0.DAT"},
            "tmaps0": {"selectors": selectors, "textures": tmap_rows, "bank": "TMAPS0-0", "palette_asset_id": "owner_data:DATA/PALD-0.DAT", "decoder": decoder_identity},
            "world_sprite_root": {"asset_id": "owner_data:DATA/MSPRD0-0.DAT", "path": "DATA/MSPRD0-0.DAT", "status": "bound_root_not_materialized"},
            "world_web_root": {"asset_id": "owner_data:DATA/MWEBD0-0.DAT", "path": "DATA/MWEBD0-0.DAT", "status": "bound_root_not_materialized"},
            "gpu_ready": {"indexed_format": "R8_UNORM", "filter": "nearest", "row_order": "top_to_bottom", "origin": "top_left", "palette_lookup": "PALD"},
        }
        descriptor_bytes = canonical_json(descriptor)
        write_output(stage, outputs, "materials/descriptor.json", descriptor_bytes, kind="canonical_json_day_material_descriptor", source_asset_ids=["owner_data:DATA/PALD-0.DAT", "owner_data:DATA/TABLESD.DAT", "owner_data:DATA/CLRD-0.DAT", "owner_data:DATA/BL32N0-0.DAT", "owner_data:DATA/SKYD0-0.DAT"])
        manifest = {"schema": LEVEL2_MATERIAL_SCHEMA, "generator": {"name": "build_cache.py", "version": GENERATOR_VERSION}, "profile": profile, "bindings": {"level_export_package_sha256": package_hash, "level_asset_closure_sha256": closure_hash, "owner_catalog_sha256": profile["catalog_sha256"], "owner_catalog_file_sha256": profile["catalog_file_sha256"], "remc2_commit": PINNED_REMC2_COMMIT, **_level2_owner_identity_bindings(profile)}, "outputs": outputs, "cache_content_sha256": _level2_cache_identity(outputs)}
        manifest["manifest_payload_sha256"] = sha256_bytes(canonical_json(manifest))
        (stage / "manifest.json").write_bytes(canonical_json(manifest))
        validate_day_material_cache(stage, package_hash, closure_hash)
        fsync_tree(stage)
        commit_stage(output, stage)
        return manifest
    except Exception:
        if stage.exists():
            shutil.rmtree(stage)
        raise


def build_cache(
    owner_root: Path,
    package_dir: Path,
    closure_path: Path,
    catalog_path: Path,
    output: Path,
    contract_path: Path = DEFAULT_CONFORMANCE_CONTRACT,
    expected_contract_sha256: str | None = None,
) -> dict[str, Any]:
    # Existing qualification callers still pass the old receipt hash.  It is
    # deliberately not part of cache identity; the contract validates its own
    # semantic byte-contract identity instead.
    del expected_contract_sha256
    owner_root = owner_root.expanduser().resolve()
    package_dir = package_dir.expanduser().resolve()
    closure_path = closure_path.expanduser().resolve()
    catalog_path = catalog_path.expanduser().resolve()
    contract_path = contract_path.expanduser().resolve()
    output = validate_output_path(output)
    contract, contract_self_sha256 = load_conformance_contract(contract_path)
    contract_file_sha256 = sha256_file(contract_path)
    contract_identity = conformance_contract_identity_payload(contract)[
        "trusted_identity"
    ]

    package = load_json(package_dir / "manifest.json")
    closure = load_json(closure_path)
    catalog = load_json(catalog_path)
    goat_tmap_golden = load_goat_tmap_golden()
    try:
        load_proven_export_verifier()(package_dir, package)
    except ValueError as error:
        raise CacheError(f"package identity verification failed: {error}") from error
    if package.get("package_sha256") != EXPECTED_PACKAGE:
        raise CacheError("wrong Level 1 package hash")
    terrain_export = (package_dir / "terrain.bin").read_bytes()
    if len(terrain_export) < 65536:
        raise CacheError("Level 1 terrain export is truncated")
    level1_terrain_selectors = sorted(set(terrain_export[:65536]))
    if level1_terrain_selectors != list(LEVEL1_TERRAIN_SELECTORS):
        raise CacheError("oracle-matched Level 1 terrain selector set changed")
    closure_claim = closure.get("closure_sha256")
    closure_payload = dict(closure)
    closure_payload.pop("closure_sha256", None)
    if sha256_bytes(
        json.dumps(
            closure_payload, sort_keys=True, separators=(",", ":"), ensure_ascii=True
        ).encode("utf-8")
    ) != closure_claim:
        raise CacheError("closure canonical self-hash mismatch")
    if closure.get("closure_sha256") != EXPECTED_CLOSURE:
        raise CacheError("wrong Level 1 closure hash")
    closure_package = closure["source"]["level_export"]["package_sha256"]
    if closure_package != EXPECTED_PACKAGE:
        raise CacheError("closure is not bound to the exact package")
    catalog_binding = closure["source"]["owner_catalog"]["catalog_sha256"]
    catalog_claim = catalog.get("catalog_sha256")
    catalog_payload = dict(catalog)
    catalog_payload.pop("catalog_sha256", None)
    catalog_actual = sha256_bytes(
        json.dumps(
            catalog_payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False
        ).encode("utf-8")
    )
    if catalog_claim != catalog_actual or catalog_claim != catalog_binding:
        raise CacheError("owner catalog does not match closure binding")
    if sha256_file(catalog_path) != closure["source"]["owner_catalog"]["file_sha256"]:
        raise CacheError("owner catalog file hash does not match closure binding")

    file_rows = {row["path"]: row for row in catalog["files"]}
    entry_rows = {row["asset_id"]: row for row in catalog["entries"]}
    source_files: list[dict[str, Any]] = []
    source_bytes: dict[str, bytes] = {}
    for relative in SOURCE_PATHS:
        path = confined_source(owner_root, relative)
        data = path.read_bytes()
        row = file_rows.get(relative)
        if row is None or row.get("sha256") != sha256_bytes(data):
            raise CacheError(f"owner source hash differs from qualified catalog: {relative}")
        source_bytes[relative] = data
        source_files.append(
            {
                "asset_id": row["asset_id"],
                "path": relative,
                "bytes": len(data),
                "sha256": row["sha256"],
            }
        )
    runtime_captures, runtime_profile = qualified_runtime_profile(package_dir)
    pinned_source_evidence = pinned_remc2_source_evidence()

    frame_ids = {
        int(asset_id.rsplit("#", 1)[1])
        for event in closure["sets"]["frame_used"]["assets"]
        for asset_id in event.get("owner_asset_ids", [])
        if asset_id.startswith("owner_data:DATA/MSPRN0-0.DAT#")
    }
    if frame_ids != set(RESET_FRAME_MSPRN):
        raise CacheError("closure no longer contains the qualified 11 MSPRN frame indices")
    dynamic_ids = {
        int(row["asset_id"].rsplit("#", 1)[1])
        for row in closure["sets"]["dynamic_dependencies"]["assets"]
        if row["asset_id"].startswith("owner_data:DATA/MSPRN0-0.DAT#")
    }
    if not set(DYNAMIC_MSPRN) <= dynamic_ids:
        raise CacheError("qualified dynamic MSPRN dependency is missing")
    castle_build_assets = [
        row
        for row in closure["sets"]["dynamic_dependencies"]["assets"]
        if row["asset_id"]
        == "owner_data:DATA/BUILD0-0.DAT#footprint-1"
    ]
    if len(castle_build_assets) != 1:
        raise CacheError("qualified dynamic Castle BUILD footprint is missing")
    castle_build_closure = castle_build_assets[0]
    house_build_closures = {
        int(row["asset_id"].rsplit("-", 1)[1]): row
        for row in closure["sets"]["scene_reachable"]["assets"]
        if row["asset_id"].startswith(
            "owner_data:DATA/BUILD0-0.DAT#footprint-"
        )
    }
    if set(house_build_closures) != set(LEVEL1_HOUSE_BUILD_INDICES):
        raise CacheError(
            "qualified Level 1 house BUILD footprint closure changed"
        )

    frame_tmaps = {
        int(asset_id.rsplit("#", 1)[1])
        for event in closure["sets"]["frame_used"]["assets"]
        for asset_id in event.get("owner_asset_ids", [])
        if asset_id.startswith("owner_data:DATA/TMAPS1-0.DAT#")
    }
    if frame_tmaps != set(FRAME_TMAPS):
        raise CacheError("closure no longer contains the qualified four TMAP selectors")
    scene_tmaps = {
        int(row["asset_id"].rsplit("#", 1)[1])
        for row in closure["sets"]["scene_reachable"]["assets"]
        if row["asset_id"].startswith("owner_data:DATA/TMAPS1-0.DAT#")
    }
    if not frame_tmaps <= scene_tmaps or len(scene_tmaps) != 129:
        raise CacheError("qualified Level 1 scene TMAP closure changed")
    source_contract_tmaps = {
        int(outcome["base_tmap"])
        for row in dynamic_allocator_sprite_census()
        for outcome in row["outcomes"]
        if outcome["disposition"] == "Render"
    }
    source_contract_tmaps.update(
        int(profile[1])
        for profile in RESET_SOURCE_PRESENTATION_PROFILES.values()
        if profile[1] is not None
    )
    scene_tmaps.update(LEVEL1_LIVE_MISSION_TMAPS)
    scene_tmaps.update(source_contract_tmaps)
    if len(scene_tmaps) != 180:
        raise CacheError("live Level 1 mission TMAP extension changed")

    stage = Path(tempfile.mkdtemp(prefix=f".{output.name}.", dir=output.parent))
    outputs: list[dict[str, Any]] = []
    try:
        rnc_work = stage / ".rnc-work"
        rnc_work.mkdir()
        decoder, rnc_provenance = compile_rnc_decoder(rnc_work)
        reset_main = decode_container_blob(
            decoder,
            source_bytes["DATA/SMATITLE.DAT"],
            0,
            320 * 200,
            rnc_work,
            "smatitle-main",
        )
        reset_palette = decode_container_blob(
            decoder,
            source_bytes["DATA/SMATITLE.PAL"],
            0,
            256 * 3,
            rnc_work,
            "smatitle-palette",
        )
        reset_main_output = write_output(
            stage,
            outputs,
            "frame/reset-loaded-main-320x200.indices.u8",
            reset_main,
            kind="indexed_image_u8_reset_loaded_MAIN",
            source_asset_ids=["owner_data:DATA/SMATITLE.DAT"],
            extra={
                "width": 320,
                "height": 200,
                "history_state": "Unprimed",
            },
        )
        reset_palette_output = write_output(
            stage,
            outputs,
            "frame/reset-loaded-main.palette.rgb6",
            reset_palette,
            kind="ordered_palette_rgb6",
            source_asset_ids=["owner_data:DATA/SMATITLE.PAL"],
            extra={"colour_count": 256},
        )
        build_dat = source_bytes["DATA/BUILD0-0.DAT"]
        build_tab = source_bytes["DATA/BUILD0-0.TAB"]
        build_records = bitmap_records(build_tab)
        if CASTLE_STAGE_ONE_BUILD_INDEX + 1 >= len(build_records):
            raise CacheError("qualified Castle BUILD index has no following offset")
        build_offset, raw_width, raw_height = build_records[
            CASTLE_STAGE_ONE_BUILD_INDEX
        ]
        build_end = build_records[CASTLE_STAGE_ONE_BUILD_INDEX + 1][0]
        build_tab_record = build_tab[
            CASTLE_STAGE_ONE_BUILD_INDEX * TAB_RECORD.size:
            (CASTLE_STAGE_ONE_BUILD_INDEX + 1) * TAB_RECORD.size
        ]
        build_cells = build_dat[build_offset:build_end]
        if (
            (raw_width, raw_height)
            != (
                CASTLE_STAGE_ONE_BUILD_RAW_WIDTH,
                CASTLE_STAGE_ONE_BUILD_RAW_HEIGHT,
            )
            or len(build_cells) != raw_width * raw_height * 2
            or sha256_bytes(build_cells)
            != CASTLE_STAGE_ONE_BUILD_PAYLOAD_SHA256
            or sha256_bytes(build_tab_record)
            != CASTLE_STAGE_ONE_BUILD_TAB_RECORD_SHA256
        ):
            raise CacheError("qualified Castle BUILD index 1 changed")
        if (
            castle_build_closure["width"] != raw_width
            or castle_build_closure["height"] != raw_height
            or castle_build_closure["span"]
            != {"offset": build_offset, "length": len(build_cells)}
            or castle_build_closure["content_address"]
            != f"sha256:{sha256_bytes(build_cells)}"
        ):
            raise CacheError("Castle BUILD closure/cache contract disagrees")
        vga_selector = package["presentation_dependencies"][
            "vga_resolution_selector"
        ]
        if vga_selector != 1:
            raise CacheError("qualified Level 1 VGA selector changed")
        runtime_width = raw_width * 2
        runtime_height = raw_height * 2
        effective_width = runtime_width // 2
        effective_height = runtime_height // 2
        build_parameter_data = source_bytes["DATA/BLDGPRM.DAT"]
        parameter_offset = (
            CASTLE_STAGE_ONE_BUILD_INDEX * BUILD_PARAMETER_RECORD.size
        )
        parameter_record = build_parameter_data[
            parameter_offset:parameter_offset + BUILD_PARAMETER_RECORD.size
        ]
        if (
            len(parameter_record) != BUILD_PARAMETER_RECORD.size
            or sha256_bytes(parameter_record)
            != CASTLE_STAGE_ONE_BUILD_PARAMETER_SHA256
        ):
            raise CacheError("qualified Castle BLDGPRM index 1 changed")
        parameter_word, parameter_flags, parameter_byte_3 = (
            BUILD_PARAMETER_RECORD.unpack(parameter_record)
        )
        build_cells_output = write_output(
            stage,
            outputs,
            "build/castle-stage-1.cells.u8",
            build_cells,
            kind="owner_BUILD_terrain_footprint_cells_u8x2",
            source_asset_ids=[
                "owner_data:DATA/BUILD0-0.DAT#footprint-1"
            ],
            extra={
                "build_index": CASTLE_STAGE_ONE_BUILD_INDEX,
                "cell_stride_bytes": 2,
                "raw_width": raw_width,
                "raw_height": raw_height,
                "runtime_width_after_CreateIndexes": runtime_width,
                "runtime_height_after_CreateIndexes": runtime_height,
                "effective_width": effective_width,
                "effective_height": effective_height,
                "vga_resolution_selector": vga_selector,
            },
        )
        build_descriptor = {
            "schema": "mc2-level1-castle-build-footprint/v1",
            "source_asset_id":
                "owner_data:DATA/BUILD0-0.DAT#footprint-1",
            "build_index": CASTLE_STAGE_ONE_BUILD_INDEX,
            "vga_resolution_selector": vga_selector,
            "dimensions": {
                "raw_tab": {
                    "width": raw_width,
                    "height": raw_height,
                    "tab_record_sha256": sha256_bytes(build_tab_record),
                },
                "loader_runtime": {
                    "width": runtime_width,
                    "height": runtime_height,
                    "rule":
                        "CreateIndexes doubles when resolution bit 0 is set",
                },
                "castle_consumer_effective": {
                    "width": effective_width,
                    "height": effective_height,
                    "rule":
                        "SetShiftByCastle and terrain consumers halve when selector is 1",
                },
            },
            "cells": {
                "path": build_cells_output["path"],
                "sha256": build_cells_output["sha256"],
                "bytes": build_cells_output["bytes"],
                "stride_bytes": 2,
                "cell_0": "terrain_or_shading_control",
                "cell_1":
                    "target_height_offset_from_construction_base_or_ff_sentinel",
                "row_order": "source_BUILD_consumer_order",
            },
            "building_parameters": {
                "source_asset_id": "owner_data:DATA/BLDGPRM.DAT",
                "record_index": CASTLE_STAGE_ONE_BUILD_INDEX,
                "record_sha256": sha256_bytes(parameter_record),
                "word_0": parameter_word,
                "flags": parameter_flags,
                "byte_3": parameter_byte_3,
            },
            "root_altitude": {
                "rule":
                    "construction origin is 32 * the minimum of the exact source perimeter walk",
                "walk":
                    "top x=start..start+7 at y=start; bottom at y=start+8; right x=start+8 y=start..start+7; left x=start y=start..start+7",
                "source":
                    "sub_48E60/sub_48F20 called by sub_4AA40",
            },
            "construction": {
                "source":
                    "AddTerrainMod0A_2A_37BC0 action 44",
                "initial_countdown": 19,
                "action_calls": 20,
                "height_mutation_remaining_counts":
                    list(range(18, 0, -1)),
                "material_mutation_remaining_counts": [14, 7, 1],
                "completion":
                    "call19 changes 1 to -1 without mutation; call20 changes -1 to 0, signals parent state2, and disables child",
                "height_rule":
                    "current += trunc_toward_zero((cell1 + construction_base_byte - current) / remaining_count)",
            },
        }
        build_descriptor_output = write_output(
            stage,
            outputs,
            "build/castle-stage-1.json",
            canonical_json(build_descriptor),
            kind="canonical_json_Level1_Castle_BUILD_descriptor",
            source_asset_ids=[
                "owner_data:DATA/BUILD0-0.DAT#footprint-1",
                "owner_data:DATA/BLDGPRM.DAT",
            ],
        )
        house_build_blob = bytearray()
        house_build_rows = []
        for house_build_index in LEVEL1_HOUSE_BUILD_INDICES:
            if house_build_index + 1 >= len(build_records):
                raise CacheError(
                    f"qualified house BUILD index {house_build_index} "
                    "has no following offset"
                )
            house_offset, house_width, house_height = build_records[
                house_build_index
            ]
            house_end = build_records[house_build_index + 1][0]
            house_cells = build_dat[house_offset:house_end]
            closure_row = house_build_closures[house_build_index]
            house_hash = sha256_bytes(house_cells)
            if (
                len(house_cells) != house_width * house_height * 2
                or closure_row["width"] != house_width
                or closure_row["height"] != house_height
                or closure_row["span"]
                != {"offset": house_offset, "length": len(house_cells)}
                or closure_row["content_address"] != f"sha256:{house_hash}"
            ):
                raise CacheError(
                    f"qualified house BUILD index {house_build_index} changed"
                )
            blob_offset = len(house_build_blob)
            house_build_blob.extend(house_cells)
            house_build_rows.append(
                {
                    "build_index": house_build_index,
                    "width": house_width,
                    "height": house_height,
                    "blob_offset": blob_offset,
                    "bytes": len(house_cells),
                    "sha256": house_hash,
                    "source_asset_id":
                        f"owner_data:DATA/BUILD0-0.DAT#footprint-{house_build_index}",
                }
            )
        house_build_cells_output = write_output(
            stage,
            outputs,
            "build/level1-house-footprints.cells.u8",
            bytes(house_build_blob),
            kind="owner_Level1_house_BUILD_terrain_footprints_u8x2",
            source_asset_ids=[
                row["source_asset_id"] for row in house_build_rows
            ],
            extra={
                "build_indices": list(LEVEL1_HOUSE_BUILD_INDICES),
                "cell_stride_bytes": 2,
                "vga_resolution_selector": vga_selector,
            },
        )
        house_build_descriptor = {
            "schema": "mc2-level1-house-build-footprints/v1",
            "vga_resolution_selector": vga_selector,
            "consumer_dimensions":
                "raw TAB dimensions; CreateIndexes doubles and action 53 halves at selector 1",
            "cells": {
                "path": house_build_cells_output["path"],
                "sha256": house_build_cells_output["sha256"],
                "bytes": house_build_cells_output["bytes"],
                "stride_bytes": 2,
                "row_order": "source_BUILD_consumer_order",
            },
            "footprints": house_build_rows,
        }
        house_build_descriptor_output = write_output(
            stage,
            outputs,
            "build/level1-house-footprints.json",
            canonical_json(house_build_descriptor),
            kind="canonical_json_Level1_house_BUILD_footprints",
            source_asset_ids=[
                row["source_asset_id"] for row in house_build_rows
            ],
        )
        sound_container = source_bytes["SOUND/SOUND.DAT"]
        music_container = source_bytes["SOUND/MUSIC.DAT"]
        if (
            sha256_bytes(sound_container) !=
                "dba5958ae8f3aae53f3b9e3fa792bfef75a020b185e7884e344c7238ff2ee2a2"
            or sha256_bytes(music_container) !=
                "3549b18c75b1812618b775e8b76f1ead312acfe4d6f5c8029d8cb696a70dbeb5"
        ):
            raise CacheError("qualified owner audio container hash changed")

        sound_counts, sound_descriptor_raw = container_descriptor(
            sound_container,
            count_rows=6,
            selector=1,
            row=3,
            label="sound-bank-1-quality-822",
        )
        if (
            sound_counts != [0, 0, 0, 10, 10, 0]
            or sound_descriptor_raw != {
                "container_trailer_offset": 14864080,
                "descriptor_offset": 14864236,
                "metadata_offset": 1988320,
                "data_offset": 0,
                "metadata_size": 2080,
                "data_size": 1988320,
            }
        ):
            raise CacheError("qualified Night/822 SOUND descriptor changed")
        sound_metadata = decode_container_blob(
            decoder,
            sound_container,
            sound_descriptor_raw["metadata_offset"],
            sound_descriptor_raw["metadata_size"],
            rnc_work,
            "sound-bank-1-822-metadata",
        )
        sound_data = decode_container_blob(
            decoder,
            sound_container,
            sound_descriptor_raw["data_offset"],
            sound_descriptor_raw["data_size"],
            rnc_work,
            "sound-bank-1-822-data",
        )
        sound_records = packed_audio_records(
            sound_metadata,
            sound_data,
            expected_count=64,
            label="sound-bank-1-822",
        )
        sound_semantics = {
            1: ("ocean", "loop", 70, "local_ambient"),
            2: ("cricket2", "loop", 70, "local_ambient"),
            3: ("explod3", "one_shot", 127, "original_stereo_positional"),
            4: ("gainmana", "one_shot", 127, "original_stereo_positional"),
            5: ("fire", "loop", 120, "local_ambient"),
            8: ("dragroar", "one_shot", 127, "original_stereo_positional"),
            9: ("fireball", "one_shot", 127, "original_stereo_positional"),
            10: ("quake4", "one_shot", 127, "original_stereo_positional"),
            12: ("null", "explicit_null", 0, "explicit_null"),
            14: ("select", "one_shot", 127, "local_feedback"),
            15: ("firebal2", "one_shot", 127, "original_stereo_positional"),
            16: ("die", "one_shot", 127, "original_stereo_positional"),
            18: ("getspell", "one_shot", 127, "original_stereo_positional"),
            19: ("speedup", "one_shot", 127, "original_stereo_positional"),
            27: ("splash2", "one_shot", 127, "original_stereo_positional"),
            29: ("cantuse", "one_shot", 127, "local_feedback"),
            30: ("meteorex", "one_shot", 127, "original_stereo_positional"),
            31: ("market", "loop", 85, "local_ambient"),
            33: ("arrow1", "one_shot", 127, "original_stereo_positional"),
            34: ("arrow2", "one_shot", 127, "original_stereo_positional"),
            37: ("cymerian", "one_shot", 127, "original_stereo_positional"),
            39: ("wyvern", "one_shot", 127, "original_stereo_positional"),
            40: ("manashot", "one_shot", 127, "original_stereo_positional"),
            41: ("switch", "one_shot", 127, "original_stereo_positional"),
            43: ("firefly1", "one_shot", 127, "original_stereo_positional"),
            44: ("firefly2", "one_shot", 127, "original_stereo_positional"),
            46: (
                "goat",
                "one_shot",
                127,
                "original_stereo_positional",
            ),
            54: ("hit1", "one_shot", 127, "original_stereo_positional"),
            55: ("hit2", "one_shot", 127, "original_stereo_positional"),
            56: ("hit3", "one_shot", 127, "original_stereo_positional"),
            57: ("hit4", "one_shot", 127, "original_stereo_positional"),
            63: ("spellup", "one_shot", 127, "original_stereo_positional"),
        }
        sound_assets = []
        for sound_id in LEVEL1_SOUND_IDS:
            record = sound_records[sound_id - 1]
            (
                semantic_name,
                play_class,
                target_volume,
                playback_scope,
            ) = sound_semantics[sound_id]
            payload = sound_data[
                record["relative_data_offset"]:
                record["relative_data_offset"] + record["bytes"]
            ]
            properties = wav_properties(payload, f"sound ID {sound_id}")
            if record["word"] != 90:
                raise CacheError(f"sound ID {sound_id} metadata word changed")
            asset_path = None
            asset_sha256 = None
            if sound_id in LEVEL1_MATERIALIZED_SOUND_IDS:
                output_row = write_output(
                    stage,
                    outputs,
                    f"audio/sfx/{sound_id:03d}-{semantic_name}.wav",
                    payload,
                    kind="owner_RIFF_WAVE_exact_record",
                    source_asset_ids=["owner_data:SOUND/SOUND.DAT"],
                    extra={
                        "bank": 1,
                        "quality": 822,
                        "quality_index": 3,
                        "sound_id": sound_id,
                        **properties,
                    },
                )
                asset_path = output_row["path"]
                asset_sha256 = output_row["sha256"]
            sound_assets.append({
                "bank": 1,
                "quality": 822,
                "quality_index": 3,
                "sound_id": sound_id,
                "metadata_slot": sound_id - 1,
                "filename": record["filename"],
                "relative_data_offset": record["relative_data_offset"],
                "record_bytes": record["bytes"],
                "record_sha256": record["payload_sha256"],
                "metadata_word": record["word"],
                "semantic_name": semantic_name,
                "play_class": play_class,
                "playback_scope": playback_scope,
                "loop_target_volume": target_volume,
                "asset_path": asset_path,
                "asset_sha256": asset_sha256,
                "mapping_status":
                    "explicit_null" if sound_id == 12 else "proven_Level1",
                "wav": properties,
            })
        expected_sound_names = {
            1: "OCEAN.WAV",
            2: "CRICKET2.WAV",
            3: "EXPLOD3.WAV",
            4: "GAINMANA.WAV",
            5: "FIRE.WAV",
            8: "DRAGROAR.WAV",
            9: "FIREBAL1.WAV",
            10: "QUAKE4.WAV",
            12: "NULL.WAV",
            14: "SELECT.WAV",
            15: "FIREBAL2.WAV",
            16: "DIE.WAV",
            18: "GETSPELL.WAV",
            19: "SPEEDUP.WAV",
            27: "SPLASH2.WAV",
            29: "CANTUSE.WAV",
            30: "METEOREX.WAV",
            31: "MARKET.WAV",
            33: "ARROW1.WAV",
            34: "ARROW2.WAV",
            37: "CYMERIAN.WAV",
            39: "WYVERN.WAV",
            40: "MANASHOT.WAV",
            41: "SWITCH.WAV",
            43: "FIREFLY1.WAV",
            44: "FIREFLY2.WAV",
            46: "GOAT.WAV",
            54: "HIT1.WAV",
            55: "HIT2.WAV",
            56: "HIT3.WAV",
            57: "HIT4.WAV",
            63: "SPELLUP.WAV",
        }
        if {
            row["sound_id"]: row["filename"] for row in sound_assets
        } != expected_sound_names:
            raise CacheError("qualified Level 1 sound filenames changed")
        sound_descriptor = {
            "schema": "mc2-level1-sound-bank/v1",
            "source_asset_id": "owner_data:SOUND/SOUND.DAT",
            "source_sha256": sha256_bytes(sound_container),
            "bank": 1,
            "realm": "Night",
            "quality": 822,
            "quality_index": 3,
            "quality_rows": list(SOUND_QUALITY_ROWS),
            "bank_counts_by_quality_index": sound_counts,
            "container": sound_descriptor_raw,
            "runtime_indexing": "one_based_ID_with_inserted_null_slot_zero",
            "metadata_record_count": len(sound_records),
            "metadata_records": sound_records,
            "level1_assets": sound_assets,
            "materialized_sound_ids": list(LEVEL1_MATERIALIZED_SOUND_IDS),
            "explicit_null_sound_ids": [12],
            "other_rows_status": "Deferred",
        }
        sound_descriptor_output = write_output(
            stage,
            outputs,
            "audio/sound-bank-1-822.json",
            canonical_json(sound_descriptor),
            kind="canonical_json_Level1_sound_bank_descriptor",
            source_asset_ids=["owner_data:SOUND/SOUND.DAT"],
        )

        music_counts, music_descriptor_raw = container_descriptor(
            music_container,
            count_rows=4,
            selector=0,
            row=2,
            label="music-channel-0-driver-f",
        )
        if (
            music_counts != [2, 2, 2, 2]
            or music_descriptor_raw != {
                "container_trailer_offset": 643424,
                "descriptor_offset": 643464,
                "metadata_offset": 80656,
                "data_offset": 0,
                "metadata_size": 224,
                "data_size": 80656,
            }
        ):
            raise CacheError("qualified channel-0/f MUSIC descriptor changed")
        music_metadata = decode_container_blob(
            decoder,
            music_container,
            music_descriptor_raw["metadata_offset"],
            music_descriptor_raw["metadata_size"],
            rnc_work,
            "music-channel-0-f-metadata",
        )
        music_data = decode_container_blob(
            decoder,
            music_container,
            music_descriptor_raw["data_offset"],
            music_descriptor_raw["data_size"],
            rnc_work,
            "music-channel-0-f-data",
        )
        music_records = packed_audio_records(
            music_metadata,
            music_data,
            expected_count=6,
            label="music-channel-0-f",
        )
        expected_music_names = [
            "C2GAME1.XMI", "C2GAME2.XMI", "C2GAME3.XMI",
            "C2SETUP.XMI", "C2INTRO.XMI", "C2CUTS.XMI",
        ]
        if (
            [row["filename"] for row in music_records] != expected_music_names
            or any(row["word"] != 90 for row in music_records)
        ):
            raise CacheError("qualified channel-0/f MUSIC metadata changed")
        music_payload = music_data[:music_records[0]["bytes"]]
        if (
            music_payload[:4] != b"FORM"
            or music_payload[8:12] != b"XDIR"
        ):
            raise CacheError("C2GAME1 is not an XMIDI FORM/XDIR payload")
        music_xmi_output = write_output(
            stage,
            outputs,
            "audio/music/C2GAME1.xmi",
            music_payload,
            kind="owner_XMIDI_FORM_XDIR_exact_record",
            source_asset_ids=["owner_data:SOUND/MUSIC.DAT"],
            extra={
                "logical_track": 1,
                "playback_slot": 0,
                "loop_scan_pointer_slot": 1,
            },
        )
        xmi_converter, xmi_provenance = compile_xmi_converter(rnc_work)
        music_midi = transcode_xmi(
            xmi_converter, music_payload, rnc_work, "C2GAME1"
        )
        playback_control_census = midi_dynamic_control_census(
            music_midi, "C2GAME1 converted MIDI"
        )
        if (
            playback_control_census["trigger_events"] != []
            or playback_control_census["tagged_channels"] != []
            or playback_control_census["loop_controller_events"] != [
                {"tick": 0, "channel": 1, "controller": 116, "value": 0},
                {
                    "tick": 158265,
                    "channel": 1,
                    "controller": 117,
                    "value": 127,
                },
            ]
        ):
            raise CacheError("qualified C2GAME1 dynamic control census changed")
        scan_record = music_records[1]
        scan_offset = scan_record["relative_data_offset"]
        scan_payload = music_data[
            scan_offset:scan_offset + scan_record["bytes"]
        ]
        if scan_payload[:4] != b"FORM" or scan_payload[8:12] != b"XDIR":
            raise CacheError("C2GAME2 loop-scan pointer is not XMIDI")
        scan_midi = transcode_xmi(
            xmi_converter, scan_payload, rnc_work, "C2GAME2-census"
        )
        scan_control_census = midi_dynamic_control_census(
            scan_midi, "C2GAME2 loop-scan converted MIDI"
        )
        if (
            scan_control_census["trigger_events"] != [
                {"tick": 0, "channel": 7, "controller": 119, "value": 0},
                {"tick": 0, "channel": 8, "controller": 119, "value": 0},
            ]
            or scan_control_census["tagged_channels"] != [7, 8]
        ):
            raise CacheError("qualified C2GAME2 dynamic control census changed")
        music_midi_output = write_output(
            stage,
            outputs,
            "audio/music/C2GAME1.mid",
            music_midi,
            kind="standard_MIDI_deterministic_remc2_conversion",
            source_asset_ids=["owner_data:SOUND/MUSIC.DAT"],
            extra={
                "logical_track": 1,
                "playback_slot": 0,
                "conversion_status":
                    "deterministic_container_conversion_not_DOS_timbre",
            },
        )
        adl_renderer, adl_provenance = compile_adl_renderer()
        music_wave = render_midi_wave(
            adl_renderer, music_midi, rnc_work, "C2GAME1"
        )
        music_wave_properties = wav_properties(music_wave, "C2GAME1 rendered music")
        music_wave_output = write_output(
            stage,
            outputs,
            "audio/music/C2GAME1-adlmidi-bank59.wav",
            music_wave,
            kind="Unity_ready_PCM16_stereo_deterministic_libADLMIDI_render",
            source_asset_ids=["owner_data:SOUND/MUSIC.DAT"],
            extra={
                "logical_track": 1,
                "playback_slot": 0,
                "rendering": "libADLMIDI_bank59_two_chips",
                "presentation_status":
                    "deterministic_owner_local_adaptation_not_exact_DOS_hardware_timbre",
                **music_wave_properties,
            },
        )
        music_descriptor = {
            "schema": "mc2-level1-music-channel/v1",
            "source_asset_id": "owner_data:SOUND/MUSIC.DAT",
            "source_sha256": sha256_bytes(music_container),
            "channel": 0,
            "driver_type": "f",
            "driver_index": 2,
            "driver_config": "SBPRO2.MDI",
            "container": music_descriptor_raw,
            "channel_counts_by_driver_index": music_counts,
            "metadata_records": music_records,
            "logical_track": 1,
            "playback_slot": 0,
            "loop_scan_pointer_slot": 1,
            "payload_filename": "C2GAME1.XMI",
            "xmi_path": music_xmi_output["path"],
            "xmi_sha256": music_xmi_output["sha256"],
            "midi_path": music_midi_output["path"],
            "midi_sha256": music_midi_output["sha256"],
            "unity_wave_path": music_wave_output["path"],
            "unity_wave_sha256": music_wave_output["sha256"],
            "unity_wave": music_wave_properties,
            "unity_wave_renderer": adl_provenance,
            "dynamic_control": {
                "update_mode_field": "target_mode",
                "timer_multiplier": 3,
                "timer_hz_formula": "30 * timer_multiplier",
                "timer_hz": 90,
                "envelope_min": 0,
                "envelope_max": 127,
                "envelope_step_per_callback": 1,
                "playback_slot_census": playback_control_census,
                "loop_scan_pointer_census": scan_control_census,
                "audible_channel_resolution":
                    "Unresolved_playback_slot0_has_no_CC119_but_loop_scan_slot1_tags_channels7_8",
            },
            "initial_start_status":
                "SourceOrderQualified_remc2_local_gameTurn1_after_second_DrawAndEvents",
            "exact_DOS_hardware_timbre": "Deferred",
            "optional_random_OGG_path": "Excluded",
        }
        music_descriptor_output = write_output(
            stage,
            outputs,
            "audio/music-channel-0-f.json",
            canonical_json(music_descriptor),
            kind="canonical_json_Level1_music_channel_descriptor",
            source_asset_ids=["owner_data:SOUND/MUSIC.DAT"],
        )
        event_semantics = {
            "schema": "mc2-level1-audio-event-semantics/v1",
            "bank": 1,
            "quality": 822,
            "consume_event_kinds": [
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
            "request_spell_activation_audio_status":
                "NoDirectRequestMapping; successful Fireball spawn emits MissionSound9",
            "prepare_sound_ids": {
                "ambient_loops": [1, 2, 5, 31],
                "periodic": {
                    "effect_0x02": 12,
                    "effect_0x10": 39,
                    "effect_0x13": 43,
                    "effect_0x19": 37,
                },
                "insufficient_mana": 29,
                "speed_activation": 19,
            },
            "cave_ids": {
                "exact_range": [65, 69],
                "status": "Deferred_Cave_bank_not_Level1_Night",
            },
            "loop_targets": {"1": 70, "2": 70, "5": 120, "31": 85},
            "end_loop": {
                "fade_target": 0,
                "multiplier": 2,
                "callback_hz": 60,
                "volume_step_per_callback": 1,
            },
            "music_update": {
                "group": 3,
                "timer_hz": 90,
                "mode_1_target": 0,
                "mode_2_target": 127,
                "volume_step_per_callback": 1,
                "track_start_event_status":
                    "SourceOrderQualified_after_second_DrawAndEvents",
            },
            "positional_surfaces": {
                "status":
                    "Ready_via_typed_McAudioCommandV2_original_integer_stereo",
                "command_required_sound_ids": [
                    3, 4, 8, 9, 10, 15, 16, 18, 19, 27, 30, 33, 34, 37,
                    39, 40, 41, 43, 44, 46, 54, 55, 56, 57, 63,
                ],
                "missing_command_reason": "missing_positional_command",
                "attenuation_axis": "XY_only_original_helper_despite_XYZ_name",
                "unity_spatial_blend": 0,
                "unity_doppler": 0,
                "unity_default_rolloff": "Disabled",
            },
            "mission_event_sounds": {
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
            "goat_sound_request": {
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
            "unknown_policy":
                "structured_deterministic_suppressed_log_no_fallback_clip",
        }
        event_semantics_output = write_output(
            stage,
            outputs,
            "audio/event-semantics.json",
            canonical_json(event_semantics),
            kind="canonical_json_Level1_audio_event_contract",
            source_asset_ids=[
                "owner_data:SOUND/SOUND.DAT",
                "owner_data:SOUND/MUSIC.DAT",
            ],
        )
        write_output(
            stage,
            outputs,
            "palette/paln.rgb6",
            source_bytes["DATA/PALN-0.DAT"],
            kind="ordered_rgb6_palette",
            source_asset_ids=["owner_data:DATA/PALN-0.DAT"],
            extra={
                "entries": 256,
                "palette_status": "exact_night_base_palette",
            },
        )
        paln_rgb6 = source_bytes["DATA/PALN-0.DAT"]
        if len(paln_rgb6) != 768 or max(paln_rgb6) > 63:
            raise CacheError("qualified PALN is not 256 RGB6 triplets")
        paln_rgb8 = bytes(value * 4 for value in paln_rgb6)
        paln_rgb8_output = write_output(
            stage,
            outputs,
            "palette/paln.rgb8",
            paln_rgb8,
            kind="ordered_rgb8_palette_exact_vga_scale",
            source_asset_ids=["owner_data:DATA/PALN-0.DAT"],
            extra={
                "entries": 256,
                "conversion": "channel_u8 = channel_rgb6 * 4",
                "maximum_channel": 252,
            },
        )
        tablesn = source_bytes["DATA/TABLESN.DAT"]
        if len(tablesn) != 83456:
            raise CacheError("qualified TABLESN byte count changed")
        tablesn_output = write_output(
            stage,
            outputs,
            "palette/tablesn.u8",
            tablesn,
            kind="remc2_night_renderer_tables_u8",
            source_asset_ids=["owner_data:DATA/TABLESN.DAT"],
            extra={
                "shade_lut_offset": 0,
                "shade_lut_bytes": 65536,
                "shade_lut_index":
                    "TABLESN[(brightness_u8 << 8) | source_palette_index_u8]",
            },
        )
        shade_lut_output = write_output(
            stage,
            outputs,
            "palette/tablesn.shade-lut.indices.u8",
            tablesn[:65536],
            kind="indexed_palette_shade_lut_256x256_u8",
            source_asset_ids=["owner_data:DATA/TABLESN.DAT"],
            extra={
                "width": 256,
                "height": 256,
                "x": "source_palette_index_u8",
                "y": "brightness_u8",
                "result": "PALN_palette_index_u8",
            },
        )
        clrn = source_bytes["DATA/CLRN-0.DAT"]
        if len(clrn) != 4096:
            raise CacheError("qualified CLRN byte count changed")
        clrn_output = write_output(
            stage,
            outputs,
            "palette/clrn.indices.u8",
            clrn,
            kind="night_rgb444_to_palette_index_lut_u8",
            source_asset_ids=["owner_data:DATA/CLRN-0.DAT"],
            extra={
                "entries": 4096,
                "index_semantics": "remc2 Night 12-bit colour lookup",
                "runtime_status":
                    "cached_and_hash_locked_not_required_by_static_terrain_shade_bridge",
            },
        )
        write_output(
            stage,
            outputs,
            "sky/skyn.indices.u8",
            source_bytes["DATA/SKYN0-0.DAT"],
            kind="indexed_image_u8",
            source_asset_ids=["owner_data:DATA/SKYN0-0.DAT"],
            extra={
                "width": 256,
                "height": 256,
                "palette_asset_id": "owner_data:DATA/PALN-0.DAT",
                "palette_status": "proven_night_binding",
            },
        )

        bl32_atlas = source_bytes["DATA/BL32N0-0.DAT"]
        if len(bl32_atlas) % (BL32_ATLAS_WIDTH * BL32_TILE_SIDE):
            raise CacheError("BL32N is not an exact 256-wide atlas of 32x32 tiles")
        bl32_count = len(bl32_atlas) // BL32_TILE_BYTES
        if bl32_count != 152:
            raise CacheError("qualified Night BL32N bank must contain exactly 152 tiles")
        bl32_tiles = bytearray()
        for tile in range(bl32_count):
            tile_x = (tile % BL32_ATLAS_COLUMNS) * BL32_TILE_SIDE
            tile_y = (tile // BL32_ATLAS_COLUMNS) * BL32_TILE_SIDE
            for row in range(BL32_TILE_SIDE):
                start = (tile_y + row) * BL32_ATLAS_WIDTH + tile_x
                bl32_tiles.extend(bl32_atlas[start:start + BL32_TILE_SIDE])
        if len(bl32_tiles) != len(bl32_atlas):
            raise CacheError("BL32N atlas repack changed byte count")
        used_tile_rows = []
        for selector in level1_terrain_selectors:
            start = selector * BL32_TILE_BYTES
            used_tile_rows.append({
                "selector": selector,
                "sha256": sha256_bytes(
                    bytes(bl32_tiles[start:start + BL32_TILE_BYTES])
                ),
            })
        bl32_atlas_output = write_output(
            stage,
            outputs,
            "terrain/bl32n.atlas.indices.u8",
            bl32_atlas,
            kind="indexed_atlas_u8",
            source_asset_ids=["owner_data:DATA/BL32N0-0.DAT"],
            extra={
                "width": BL32_ATLAS_WIDTH,
                "height": len(bl32_atlas) // BL32_ATLAS_WIDTH,
                "tile_width": BL32_TILE_SIDE,
                "tile_height": BL32_TILE_SIDE,
                "tile_columns": BL32_ATLAS_COLUMNS,
                "tile_count": bl32_count,
                "palette_asset_id": "owner_data:DATA/PALN-0.DAT",
            },
        )
        bl32_output = write_output(
            stage,
            outputs,
            "terrain/bl32n.indices.u8",
            bytes(bl32_tiles),
            kind="indexed_tile_bank_u8",
            source_asset_ids=["owner_data:DATA/BL32N0-0.DAT"],
            extra={
                "tile_width": BL32_TILE_SIDE,
                "tile_height": BL32_TILE_SIDE,
                "tile_count": bl32_count,
                "palette_asset_id": "owner_data:DATA/PALN-0.DAT",
            },
        )

        tmap_dat = source_bytes["DATA/TMAPS1-0.DAT"]
        tmap_tab = tmap_records(source_bytes["DATA/TMAPS1-0.TAB"])
        tmap_rows: list[dict[str, Any]] = []
        for index in sorted(scene_tmaps):
            if index + 1 >= len(tmap_tab):
                raise CacheError(f"TMAPS index {index} has no following offset sentinel")
            metadata, offset, group = tmap_tab[index]
            _, end_offset, _ = tmap_tab[index + 1]
            if offset < 0 or end_offset < offset or end_offset > len(tmap_dat):
                raise CacheError(f"TMAPS packed span is invalid at index {index}")
            packed = tmap_dat[offset:end_offset]
            asset_id = f"owner_data:DATA/TMAPS1-0.DAT#{index}"
            catalog_entry = entry_rows[asset_id]
            if (
                catalog_entry["offset"] != offset
                or catalog_entry["end_offset"] != end_offset
                or catalog_entry["metadata_dword_0"] != metadata
                or catalog_entry["animation_or_allocation_group"] != group
                or catalog_entry["packed_span_sha256"] != sha256_bytes(packed)
            ):
                raise CacheError(f"TMAPS TAB/catalog disagreement at index {index}")
            inclusion = (
                "qualified_frame_used"
                if index in frame_tmaps
                else "qualified_scene_reachable"
            )
            if catalog_entry["decoder_status"] == "raw_uncompressed_tmap_payload":
                if metadata != 1 or packed != b"0":
                    raise CacheError(f"unexpected raw TMAP payload at index {index}")
                tmap_rows.append(
                    {
                        "index": index,
                        "inclusion": inclusion,
                        "status": "raw_single_byte_placeholder_nondecoded",
                        "animation_or_allocation_group": group,
                        "source_asset_id": asset_id,
                    }
                )
                continue
            decoded = decode_tmap_rnc(decoder, packed, rnc_work, index)
            if len(decoded) < TMAP_HEADER.size:
                raise CacheError(f"decoded TMAP header is truncated at index {index}")
            header_word, width, height = TMAP_HEADER.unpack_from(decoded)
            first_frame_end = TMAP_HEADER.size + width * height
            if metadata != len(decoded) or not width or not height or len(decoded) < first_frame_end:
                raise CacheError(f"decoded TMAP dimensions disagree at index {index}")
            animated = (header_word & 1) != 0
            if animated:
                if len(decoded) < first_frame_end + 2:
                    raise CacheError(f"animated TMAP has no frame stream at index {index}")
                frame_count = struct.unpack_from("<H", decoded, first_frame_end)[0]
                if frame_count == 0:
                    raise CacheError(f"animated TMAP has zero frames at index {index}")
            else:
                if len(decoded) != first_frame_end:
                    raise CacheError(f"static TMAP has trailing data at index {index}")
                frame_count = 1
            decoded_output = write_output(
                stage,
                outputs,
                f"terrain/tmaps1/{index:03d}.decoded.bin",
                decoded,
                kind="remc2_type_particle_str_decoded_payload",
                source_asset_ids=[asset_id],
                extra={
                    "index": index,
                    "header_word": header_word,
                    "width": width,
                    "height": height,
                    "animated": animated,
                    "frame_count": frame_count,
                },
            )
            pixels = decoded[TMAP_HEADER.size:first_frame_end]
            pixel_output = write_output(
                stage,
                outputs,
                f"terrain/tmaps1/{index:03d}.indices.u8",
                pixels,
                kind="indexed_image_u8",
                source_asset_ids=[asset_id],
                extra={
                    "index": index,
                    "width": width,
                    "height": height,
                    "palette_asset_id": "owner_data:DATA/PALN-0.DAT",
                },
            )
            tmap_row = {
                "index": index,
                "inclusion": inclusion,
                "status": "decoded_indexed_texture",
                "header_word": header_word,
                "width": width,
                "height": height,
                "animated": animated,
                "frame_count": frame_count,
                "animation_or_allocation_group": group,
                "indices_path": pixel_output["path"],
                "decoded_payload_path": decoded_output["path"],
                "source_asset_id": asset_id,
                "row_order": "top_to_bottom",
                "origin": "top_left",
                "palette_asset_id": "owner_data:DATA/PALN-0.DAT",
                "animation_materialization":
                    "base_frame_only; unsupported stream formats fail closed",
            }
            if index in GOAT_TMAP_SELECTORS:
                states = materialize_goat_tmap_animation(index, decoded)
                qualify_goat_tmap_states(index, states, goat_tmap_golden)
                state_paths = {"reset-base": pixel_output["path"]}
                for state in states[1:5]:
                    frame_output = write_output(
                        stage,
                        outputs,
                        f"terrain/tmaps1/{index:03d}.{state['state']}.indices.u8",
                        state["pixels"],
                        kind="indexed_image_u8_exact_remc2_mutable_tmap_state",
                        source_asset_ids=[asset_id],
                        extra={
                            "index": index,
                            "state": state["state"],
                            "width": width,
                            "height": height,
                            "stream_cursor": state["stream_cursor"],
                            "frame_index": state["frame_index"],
                            "palette_asset_id": "owner_data:DATA/PALN-0.DAT",
                        },
                    )
                    state_paths[state["state"]] = frame_output["path"]
                state_paths["closure-frame-01"] = state_paths["frame-01"]
                tmap_row["animation_materialization"] = (
                    "exact_qualified_Goat_SS2_four_transition_states; "
                    "selector_global_shared_TMAP_state_not_per_entity"
                )
                tmap_row["animation_states"] = [
                    {
                        "state": state["state"],
                        "stream_cursor": state["stream_cursor"],
                        "frame_index": state["frame_index"],
                        "width": state["width"],
                        "height": state["height"],
                        "bytes": len(state["pixels"]),
                        "sha256": sha256_bytes(state["pixels"]),
                        "indices_path": state_paths[state["state"]],
                    }
                    for state in states
                ]
            tmap_rows.append(tmap_row)

        material_descriptor = {
            "schema": "mc2-level1-night-materials/v1",
            "palette": {
                "path": "palette/paln.rgb6",
                "encoding": "256_ordered_rgb_triplets_6_bit_channels",
                "rgb6_to_rgb8": "channel_u8 = channel_rgb6 * 4",
                "rgb8_path": paln_rgb8_output["path"],
                "tablesn_path": tablesn_output["path"],
                "shade_lut_path": shade_lut_output["path"],
                "clrn_path": clrn_output["path"],
            },
            "bl32n": {
                "indices_path": bl32_output["path"],
                "source_atlas_path": bl32_atlas_output["path"],
                "source_atlas_width": BL32_ATLAS_WIDTH,
                "source_atlas_height": len(bl32_atlas) // BL32_ATLAS_WIDTH,
                "source_atlas_columns": BL32_ATLAS_COLUMNS,
                "tile_width": BL32_TILE_SIDE,
                "tile_height": BL32_TILE_SIDE,
                "tile_count": bl32_count,
                "row_order": "top_to_bottom",
                "origin": "top_left",
                "terrain_selector": "terrain_type_plane_u8_direct_tile_index",
                "level1_used_selectors": level1_terrain_selectors,
                "level1_used_selector_count": len(level1_terrain_selectors),
                "level1_used_selector_max": max(level1_terrain_selectors),
                "level1_used_tile_sha256": used_tile_rows,
                "repack_provenance": {
                    "algorithm":
                        "remc2_sub_5B7A0_256_stride_32x32_address_grid",
                    "source_sha256": sha256_bytes(
                        pinned_remc2_blob(REMC2_EVENTS_FUNCTIONS_SOURCE)
                    ),
                },
            },
            "terrain_uv": {
                "source_path":
                    f"git:{PINNED_REMC2_COMMIT}:{REMC2_UV_TABLE_SOURCE}",
                "source_sha256": sha256_bytes(
                    pinned_remc2_blob(REMC2_UV_TABLE_SOURCE)
                ),
                "source_rows_camera_relative": parse_source_uv_rows(),
                "source_row_count": 32,
                "persistent_group_expression": "(terrain_angle_u8 >> 4) & 7",
                "unity_vertex_order": [
                    "lower_left", "lower_right", "upper_left", "upper_right"
                ],
                "persistent_unity_groups": [
                    [list(point) for point in group]
                    for group in UNITY_TERRAIN_UV_GROUPS
                ],
                "persistent_unity_group_records": [
                    {
                        "group": index,
                        "points_flat": [
                            coordinate for point in group for coordinate in point
                        ],
                    }
                    for index, group in enumerate(UNITY_TERRAIN_UV_GROUPS)
                ],
                "triangle_diagonals": {
                    "even_x_plus_y": "lower_left_to_upper_right",
                    "odd_x_plus_y": "upper_left_to_lower_right",
                },
            },
            "tmaps1": {
                "frame_used_selectors": sorted(frame_tmaps),
                "scene_reachable_selectors": sorted(scene_tmaps),
                "textures": tmap_rows,
                "animation_semantics":
                    "TAB groups preserved; exact mutable SS2 states materialized only "
                    "for Goat selectors 152-154; all other animated selectors remain "
                    "base-frame-only",
                "goat_animation_golden": {
                    "path": str(GOAT_TMAP_GOLDEN.relative_to(REPO)),
                    "file_sha256": EXPECTED_GOAT_TMAP_GOLDEN,
                    "source_oracle_manifest_sha256":
                        EXPECTED_GOAT_TMAP_ORACLE_MANIFEST,
                    "selector_state_scope":
                        "selector_global_shared_TMAP_state_not_per_entity",
                },
            },
            "rendering_scope": {
                "qualified":
                    "indexed BL32N selection, D4 UV orientation, TABLESN static "
                    "terrain-shade lookup, PALN lookup",
                "not_yet_qualified":
                    "camera/distance pnt5 fog attenuation, water wave brightness, "
                    "full software-rasterizer interpolation",
                "claim": "indexed_night_material_and_static_shade_bridge",
            },
        }
        material_output = write_output(
            stage,
            outputs,
            "terrain/materials.json",
            canonical_json(material_descriptor),
            kind="canonical_json_material_descriptor",
            source_asset_ids=[
                "owner_data:DATA/BL32N0-0.DAT",
                "owner_data:DATA/TMAPS1-0.DAT",
                "owner_data:DATA/TMAPS1-0.TAB",
                "owner_data:DATA/PALN-0.DAT",
                "owner_data:DATA/TABLESN.DAT",
                "owner_data:DATA/CLRN-0.DAT",
            ],
            extra={
                "bl32_tile_count": bl32_count,
                "tmap_scene_selector_count": len(scene_tmaps),
                "tmap_frame_selector_count": len(frame_tmaps),
            },
        )
        entity_descriptor = build_entity_presentation_descriptor(
            (package_dir / "entities.bin").read_bytes(),
            (package_dir / "presentation.bin").read_bytes(),
            terrain_export,
            tmap_rows,
        )
        entity_output = write_output(
            stage,
            outputs,
            "entities/presentation.json",
            canonical_json(entity_descriptor),
            kind="canonical_json_level1_entity_presentation_descriptor",
            source_asset_ids=[
                "level_export:entities.bin",
                "level_export:presentation.bin",
                "owner_data:DATA/TMAPS1-0.DAT",
                "owner_data:DATA/TMAPS1-0.TAB",
                "owner_data:DATA/PALN-0.DAT",
            ],
            extra={
                "active_reset_count": entity_descriptor["active_reset_count"],
                "qualified_actor_count": entity_descriptor["qualified_actor_count"],
                "qualified_view_count":
                    entity_descriptor["qualified_view_count"],
                "reset_chain_drawable_count":
                    entity_descriptor["reset_chain_drawable_count"],
                "qualified_group_count": entity_descriptor["qualified_group_count"],
            },
        )
        source_tuple_census = build_source_tuple_spawn_census(
            (package_dir / "authored-level.bin").read_bytes(),
            (package_dir / "entities.bin").read_bytes(),
        )
        source_tuple_output = write_output(
            stage,
            outputs,
            "entities/source-tuple-spawn-census.json",
            canonical_json(source_tuple_census),
            kind="canonical_json_Level1_source_tuple_spawn_census",
            source_asset_ids=[
                "level_export:authored-level.bin",
                "level_export:entities.bin",
                "level_export:presentation.bin",
            ],
            extra={
                "authored_record_count":
                    source_tuple_census["matrix"]["authored_record_count"],
                "dispatch_slot_count":
                    source_tuple_census["matrix"]["dispatch_slot_count"],
                "reset_tuple_count":
                    source_tuple_census["reset_tuple_manifest"]["tuple_count"],
                "dynamic_allocator_family_count":
                    source_tuple_census["dynamic_allocator_family_count"],
                "canonical_matrix_sha256": source_tuple_census["matrix"]["sha256"],
            },
        )
        shutil.rmtree(rnc_work)

        proven_decode = load_proven_decoder()
        hspr_dat = source_bytes["DATA/HSPRN0-0.DAT"]
        hspr_records = bitmap_records(source_bytes["DATA/HSPRN0-0.TAB"])
        sprite_rows: list[dict[str, Any]] = []
        sprite_materialized: dict[int, tuple[bytes, bytes, int, int]] = {}
        for index in sorted(set(FRAME_HSPRN + DYNAMIC_HSPRN)):
            offset, width, height = hspr_records[index]
            pixels, mask, encoded_length = materialize_indexed_rle(
                hspr_dat, offset, width, height
            )
            proven = proven_decode(hspr_dat, offset, width, height)
            catalog_entry = entry_rows[f"owner_data:DATA/HSPRN0-0.DAT#{index}"]
            if (
                proven["status"] != "decoded_indexed_rle"
                or proven["indexed_pixels_sha256"] != sha256_bytes(pixels)
                or proven["transparency_mask_sha256"] != sha256_bytes(mask)
                or catalog_entry["indexed_pixels_sha256"] != sha256_bytes(pixels)
                or catalog_entry["transparency_mask_sha256"] != sha256_bytes(mask)
                or proven["encoded_length"] != encoded_length
            ):
                raise CacheError(f"HSPRN decoder/catalog disagreement at index {index}")
            reason = (
                "qualified_frame_used"
                if index in RESET_FRAME_HSPRN
                else (
                    "source_mapped_live_spell_hud_dependency"
                    if index in (2, 125, 126)
                    else (
                        "source_mapped_Level1_minimap_entity_dependency"
                        if index in (58, 66, 83, 84)
                        else (
                            "source_mapped_live_top_bar_dependency"
                            if 44 <= index <= 55
                            else "qualified_dynamic_spell_menu_dependency"
                        )
                    )
                )
            )
            index_row = write_output(
                stage,
                outputs,
                f"sprites/hsprn/{index:03d}.indices.u8",
                pixels,
                kind="indexed_image_u8",
                source_asset_ids=[catalog_entry["asset_id"]],
                extra={"index": index, "width": width, "height": height},
            )
            mask_row = write_output(
                stage,
                outputs,
                f"sprites/hsprn/{index:03d}.mask.u8",
                mask,
                kind="visibility_mask_u8_0_or_1",
                source_asset_ids=[catalog_entry["asset_id"]],
                extra={"index": index, "width": width, "height": height},
            )
            sprite_materialized[index] = (pixels, mask, width, height)
            sprite_rows.append(
                {
                    "index": index,
                    "inclusion": reason,
                    "width": width,
                    "height": height,
                    "indices_path": index_row["path"],
                    "mask_path": mask_row["path"],
                    "indices_sha256": index_row["sha256"],
                    "mask_sha256": mask_row["sha256"],
                    "palette_candidates": [
                        "owner_data:DATA/PALN-0.DAT",
                    ],
                    "palette_status": "PALN_materialized_fade_variant_unresolved_not_cached",
                    "stored_dimensions": {"width": width, "height": height},
                    "logical_scale": 1,
                    "runtime_dimensions": {"width": width, "height": height},
                    "row_order": "top_to_bottom",
                    "origin": "top_left",
                    "mask_semantics": "one_visible_zero_transparent",
                }
            )

        mspr_dat = source_bytes["DATA/MSPRN0-0.DAT"]
        mspr_records = bitmap_records(source_bytes["DATA/MSPRN0-0.TAB"])
        source_sprite_rows: list[dict[str, Any]] = []
        for index in SOURCE_FRAME_MSPRN:
            offset, width, height = mspr_records[index]
            pixels, mask, encoded_length = materialize_indexed_rle(
                mspr_dat, offset, width, height
            )
            proven = proven_decode(mspr_dat, offset, width, height)
            catalog_entry = entry_rows[f"owner_data:DATA/MSPRN0-0.DAT#{index}"]
            if (
                proven["status"] != "decoded_indexed_rle"
                or proven["indexed_pixels_sha256"] != sha256_bytes(pixels)
                or proven["transparency_mask_sha256"] != sha256_bytes(mask)
                or catalog_entry["indexed_pixels_sha256"] != sha256_bytes(pixels)
                or catalog_entry["transparency_mask_sha256"] != sha256_bytes(mask)
                or proven["encoded_length"] != encoded_length
            ):
                raise CacheError(f"MSPRN decoder/catalog disagreement at index {index}")
            index_row = write_output(
                stage,
                outputs,
                f"sprites/msprn/{index:03d}.indices.u8",
                pixels,
                kind="indexed_image_u8",
                source_asset_ids=[catalog_entry["asset_id"]],
                extra={"index": index, "width": width, "height": height},
            )
            mask_row = write_output(
                stage,
                outputs,
                f"sprites/msprn/{index:03d}.mask.u8",
                mask,
                kind="visibility_mask_u8_0_or_1",
                source_asset_ids=[catalog_entry["asset_id"]],
                extra={"index": index, "width": width, "height": height},
            )
            source_sprite_rows.append(
                {
                    "index": index,
                    "width": width,
                    "height": height,
                    "indices_path": index_row["path"],
                    "mask_path": mask_row["path"],
                    "indices_sha256": index_row["sha256"],
                    "mask_sha256": mask_row["sha256"],
                    "source_asset_id": catalog_entry["asset_id"],
                    "row_order": "top_to_bottom",
                    "origin": "top_left",
                    "mask_semantics": "one_visible_zero_transparent",
                }
            )

        font_dat = source_bytes["DATA/HFONT3.DAT"]
        font_records = bitmap_records(source_bytes["DATA/HFONT3.TAB"])
        font_blob = bytearray()
        glyph_rows: list[dict[str, Any]] = []
        for index, (offset, width, height) in enumerate(font_records):
            pixels, mask, encoded_length = materialize_indexed_rle(
                font_dat, offset, width, height
            )
            proven = proven_decode(font_dat, offset, width, height)
            catalog_entry = entry_rows[f"owner_data:DATA/HFONT3.DAT#{index}"]
            if (
                proven["indexed_pixels_sha256"] != sha256_bytes(pixels)
                or proven["transparency_mask_sha256"] != sha256_bytes(mask)
                or catalog_entry["transparency_mask_sha256"] != sha256_bytes(mask)
                or proven["encoded_length"] != encoded_length
            ):
                raise CacheError(f"HFONT3 decoder/catalog disagreement at glyph {index}")
            blob_offset = len(font_blob)
            font_blob.extend(mask)
            glyph_rows.append(
                {
                    "index": index,
                    "offset": blob_offset,
                    "length": len(mask),
                    "width": width,
                    "height": height,
                    "mask_sha256": sha256_bytes(mask),
                    "source_asset_id": catalog_entry["asset_id"],
                    "palette_status":
                        "mask_exact_runtime_tint_callsite_dynamic",
                }
            )
        font_output = write_output(
            stage,
            outputs,
            "font/hfont3.masks.u8",
            bytes(font_blob),
            kind="concatenated_visibility_masks_u8_0_or_1",
            source_asset_ids=[
                "owner_data:DATA/HFONT3.DAT",
                "owner_data:DATA/HFONT3.TAB",
            ],
            extra={"glyph_count": len(glyph_rows)},
        )

        source_font_dat = source_bytes["DATA/FONT1.DAT"]
        source_font_records = bitmap_records(source_bytes["DATA/FONT1.TAB"])
        source_font_blob = bytearray()
        source_glyph_rows: list[dict[str, Any]] = []
        for index, (offset, width, height) in enumerate(source_font_records):
            pixels, mask, encoded_length = materialize_indexed_rle(
                source_font_dat, offset, width, height
            )
            proven = proven_decode(source_font_dat, offset, width, height)
            catalog_entry = entry_rows[f"owner_data:DATA/FONT1.DAT#{index}"]
            if (
                proven["indexed_pixels_sha256"] != sha256_bytes(pixels)
                or proven["transparency_mask_sha256"] != sha256_bytes(mask)
                or catalog_entry["transparency_mask_sha256"] != sha256_bytes(mask)
                or proven["encoded_length"] != encoded_length
            ):
                raise CacheError(f"FONT1 decoder/catalog disagreement at glyph {index}")
            blob_offset = len(source_font_blob)
            source_font_blob.extend(mask)
            source_glyph_rows.append(
                {
                    "index": index,
                    "offset": blob_offset,
                    "length": len(mask),
                    "width": width,
                    "height": height,
                    "mask_sha256": sha256_bytes(mask),
                    "source_asset_id": catalog_entry["asset_id"],
                    "palette_status":
                        "mask_exact_runtime_tint_callsite_dynamic",
                }
            )
        source_font_output = write_output(
            stage,
            outputs,
            "font/font1.masks.u8",
            bytes(source_font_blob),
            kind="concatenated_visibility_masks_u8_0_or_1",
            source_asset_ids=[
                "owner_data:DATA/FONT1.DAT",
                "owner_data:DATA/FONT1.TAB",
            ],
            extra={"glyph_count": len(source_glyph_rows)},
        )

        pointer_dat = source_bytes["DATA/POINTERS.DAT"]
        pointer_records = bitmap_records(source_bytes["DATA/POINTERS.TAB"])
        cursor_selector = 9
        cursor_offset, cursor_width, cursor_height = pointer_records[cursor_selector]
        cursor_pixels, cursor_mask, cursor_encoded_length = materialize_indexed_rle(
            pointer_dat,
            cursor_offset,
            cursor_width,
            cursor_height,
        )
        cursor_proven = proven_decode(
            pointer_dat,
            cursor_offset,
            cursor_width,
            cursor_height,
        )
        cursor_catalog = entry_rows[
            f"owner_data:DATA/POINTERS.DAT#{cursor_selector}"
        ]
        if (
            cursor_proven["status"] != "decoded_indexed_rle"
            or cursor_proven["indexed_pixels_sha256"]
            != sha256_bytes(cursor_pixels)
            or cursor_proven["transparency_mask_sha256"]
            != sha256_bytes(cursor_mask)
            or cursor_catalog["indexed_pixels_sha256"]
            != sha256_bytes(cursor_pixels)
            or cursor_catalog["transparency_mask_sha256"]
            != sha256_bytes(cursor_mask)
            or cursor_proven["encoded_length"] != cursor_encoded_length
        ):
            raise CacheError("POINTERS decoder/catalog disagreement at selector 9")
        cursor_indices_output = write_output(
            stage,
            outputs,
            "cursor/pointers/009.indices.u8",
            cursor_pixels,
            kind="indexed_image_u8",
            source_asset_ids=[cursor_catalog["asset_id"]],
            extra={
                "selector": cursor_selector,
                "width": cursor_width,
                "height": cursor_height,
            },
        )
        cursor_mask_output = write_output(
            stage,
            outputs,
            "cursor/pointers/009.mask.u8",
            cursor_mask,
            kind="visibility_mask_u8_0_or_1",
            source_asset_ids=[cursor_catalog["asset_id"]],
            extra={
                "selector": cursor_selector,
                "width": cursor_width,
                "height": cursor_height,
            },
        )

        language_l1 = language_records(source_bytes["LANGUAGE/L1.TXT"])
        language_d2 = language_records(source_bytes["LANGUAGE/D2.TXT"])
        if language_l1 != language_d2:
            raise CacheError("owned L1 and D2 localization payloads are not byte-identical")
        expected_objective = b'"Fly towards my beacon."'
        if language_l1[LEVEL1_OBJECTIVE_SELECTOR] != expected_objective:
            raise CacheError("qualified Level 1 objective localization changed")
        if any(not language_l1[index] for index in LEVEL1_OBJECTIVE_IDS):
            raise CacheError("qualified Level 1 objective stage localization is empty")
        language_blob = bytearray()
        language_record_rows = []
        for selector, record in enumerate(language_l1):
            offset = len(language_blob)
            language_blob.extend(record)
            language_blob.append(0)
            language_record_rows.append(
                {
                    "selector": selector,
                    "offset": offset,
                    "byte_length": len(record),
                    "sha256": sha256_bytes(record),
                }
            )
        language_output = write_output(
            stage,
            outputs,
            "font/language-l1.records-nul.bin",
            bytes(language_blob),
            kind="ordered_owner_localization_records_nul_terminated",
            source_asset_ids=[
                "owner_data:LANGUAGE/L1.TXT",
                "owner_data:LANGUAGE/D2.TXT",
            ],
            extra={
                "record_count": LANGUAGE_RECORD_COUNT,
                "payload_offset": LANGUAGE_PAYLOAD_OFFSET,
            },
        )

        players_blob = (package_dir / "players.bin").read_bytes()
        if (
            len(players_blob) != 4596
            or players_blob[:4] != b"MCP1"
            or struct.unpack_from("<HHI", players_blob, 4) != (2, 573, 8)
        ):
            raise CacheError("players.bin is not the qualified MCP1/2 8x573 form")
        player0 = players_blob[12:12 + 573]
        enabled_holders = struct.unpack_from("<26H", player0, 268)
        left_slot = struct.unpack_from("<H", player0, 554)[0]
        right_slot = struct.unpack_from("<H", player0, 556)[0]
        if (
            left_slot != 0
            or right_slot != 1
            or enabled_holders[left_slot] != 153
            or enabled_holders[right_slot] != 154
        ):
            raise CacheError("qualified Level 1 reset spell slots changed")
        entities_blob = (package_dir / "entities.bin").read_bytes()
        if (
            len(entities_blob) != 12 + 1000 * 176
            or entities_blob[:4] != b"MCE1"
            or struct.unpack_from("<HHI", entities_blob, 4) != (1, 176, 1000)
        ):
            raise CacheError("entities.bin is not the qualified MCE1 form")
        record_size = struct.unpack_from("<H", entities_blob, 6)[0]
        holder_models = []
        for entity_index in (153, 154):
            start = 12 + entity_index * record_size
            holder_models.append(entities_blob[start + 68])
        if holder_models != [0, 1]:
            raise CacheError("qualified reset spell holder models changed")
        player_start = 12 + 152 * record_size
        player_x = struct.unpack_from("<H", entities_blob, player_start + 80)[0]
        player_y = struct.unpack_from("<H", entities_blob, player_start + 82)[0]
        if (player_x, player_y) != (19840, 56960):
            raise CacheError("qualified Level 1 reset player position changed")
        player_entity_index = struct.unpack_from("<H", player0, 4)[0]
        if player_entity_index != 152:
            raise CacheError("qualified Level 1 player relation changed")
        player_actual_speed = struct.unpack_from(
            "<h",
            entities_blob,
            12 + player_entity_index * record_size + 134,
        )[0]
        if player_actual_speed != 0:
            raise CacheError("qualified Level 1 player actual speed changed")
        runtime_profile["initial_player"] = {
            "entity_index": player_entity_index,
            "entities_record_size": record_size,
            "actual_speed_offset": 134,
            "actual_speed": player_actual_speed,
        }

        minimap_mask, minimap_scanlines = build_minimap_aperture(tablesn)
        minimap_mask_output = write_output(
            stage,
            outputs,
            "hud/minimap-aperture-128.mask.u8",
            minimap_mask,
            kind="u8_TABLESN_scanline_aperture_mask",
            source_asset_ids=["owner_data:DATA/TABLESN.DAT"],
            extra={
                "logical_width": LEVEL1_MINIMAP_LOGICAL_SIDE,
                "logical_height": LEVEL1_MINIMAP_LOGICAL_SIDE,
                "opaque_pixels": sum(minimap_mask),
                "table_offset": 0x14300,
                "table_length": 256,
                "phase_step": 0x1000000 // (
                    LEVEL1_MINIMAP_LOGICAL_SIDE // 2
                ),
            },
        )
        sine_table, sine_payload = load_remc2_sine_table()
        minimap_sine_output = write_output(
            stage,
            outputs,
            "hud/remc2-sine-2560.i32le",
            sine_payload,
            kind="i32le_remc2_Maths_sin_DB750",
            source_asset_ids=[
                "remc2_source:utilities/Maths.cpp#Maths::sin_DB750"
            ],
            extra={
                "entry_count": len(sine_table),
                "angle_period": 2048,
                "fixed_one": 65536,
            },
        )
        minimap_direction_samples, minimap_octant_probe = (
            build_minimap_direction_samples(
                player_x,
                player_y,
                terrain_export[:65536],
                terrain_export[2 * 65536:3 * 65536],
                tablesn,
                sine_table,
            )
        )

        objective_text_index = source_bytes["DATA/CLRN-0.DAT"][3840]
        objective_background_index = source_bytes["DATA/CLRN-0.DAT"][0]
        if objective_text_index != 119 or objective_background_index != 61:
            raise CacheError("qualified Night objective CLRN selectors changed")
        objective_rgb_offset = objective_text_index * 3
        if tuple(paln_rgb8[objective_rgb_offset:objective_rgb_offset + 3]) != (
            252, 36, 36
        ):
            raise CacheError("qualified Night objective tint no longer matches PALN red")
        objective_rows = []
        badge_rgba = b""
        objective_render = {}
        objective_output = {}
        for selector in LEVEL1_OBJECTIVE_IDS:
            objective_foreground, current_badge, current_render = build_objective_rasters(
                language_l1[selector],
                glyph_rows,
                bytes(font_blob),
                sprite_materialized,
                paln_rgb8,
                objective_text_index,
                1,
            )
            current_output = write_output(
                stage,
                outputs,
                f"hud/objective-{selector:03d}.indexed-mask.u8",
                objective_foreground,
                kind=(
                    "u8x2_source_mapped_Level1_objective_foreground_"
                    "indices_then_mask"
                ),
                source_asset_ids=[
                    "owner_data:LANGUAGE/L1.TXT",
                    "owner_data:DATA/HFONT3.DAT",
                    "owner_data:DATA/HSPRN0-0.DAT#171",
                    "owner_data:DATA/HSPRN0-0.DAT#172",
                    "owner_data:DATA/HSPRN0-0.DAT#173",
                    "owner_data:DATA/CLRN-0.DAT",
                    "owner_data:DATA/PALN-0.DAT",
                    "owner_data:DATA/TABLESN.DAT",
                ],
                extra={
                    "width": current_render["objective_width"],
                    "height": current_render["objective_height"],
                    "plane_bytes":
                        current_render["objective_foreground_plane_bytes"],
                    "plane_order": ["indices", "mask"],
                    "authority": "SourceMappedLive",
                    "selector": selector,
                },
            )
            objective_rows.append({
                "selector": selector,
                "text_bytes_path": f"hud/objective-text-{selector:03d}.bytes",
                "text_sha256": sha256_bytes(language_l1[selector]),
                "glyph_indices": current_render["objective_glyph_indices"],
                "lines_ascii": current_render["objective_lines_ascii"],
                "text_advances": current_render["objective_text_advances"],
                "raster_path": current_output["path"],
                "raster_plane_bytes":
                    current_render["objective_foreground_plane_bytes"],
                "foreground_indices_sha256":
                    current_render["objective_foreground_indices_sha256"],
                "foreground_mask_sha256":
                    current_render["objective_foreground_mask_sha256"],
                "foreground_opaque_pixels":
                    current_render["objective_foreground_opaque_pixels"],
                "render_width": current_render["objective_width"],
                "render_height": current_render["objective_height"],
                "reference_x": current_render["objective_x"],
                "reference_y": current_render["objective_y"],
                "text_x": current_render["objective_text_origin"]["x"],
                "text_y": current_render["objective_text_origin"]["y"],
                "blend_x": current_render["objective_blend_rect"]["x"],
                "blend_y": current_render["objective_blend_rect"]["y"],
                "blend_width":
                    current_render["objective_blend_rect"]["width"],
                "blend_height":
                    current_render["objective_blend_rect"]["height"],
                "authority": "SourceMappedLiveIndexedFramebuffer",
            })
            if selector == LEVEL1_OBJECTIVE_SELECTOR:
                badge_rgba = current_badge
                objective_render = current_render
                objective_output = current_output
        badge_output = write_output(
            stage,
            outputs,
            "hud/frozen-observed.rgba8",
            badge_rgba,
            kind="rgba8_HFONT3_authority_badge",
            source_asset_ids=[
                "owner_data:DATA/HFONT3.DAT",
                "owner_data:DATA/CLRN-0.DAT",
                "owner_data:DATA/PALN-0.DAT",
            ],
            extra={
                "width": objective_render["badge_width"],
                "height": objective_render["badge_height"],
                "authority": "FrozenObserved",
            },
        )
        objective_localization_rows = [
            {
                **language_record_rows[selector],
                "bytes_hex": language_l1[selector].hex(),
            }
            for selector in LEVEL1_OBJECTIVE_IDS
        ]
        spell_tier_rows = []
        for tier, tier_text in enumerate((b"I", b"II", b"III", b"IV", b"V")):
            tier_rgba, tier_width, tier_height, tier_glyphs = (
                build_hud_text_raster(
                    tier_text,
                    glyph_rows,
                    bytes(font_blob),
                    paln_rgb8,
                    objective_background_index,
                )
            )
            tier_output = write_output(
                stage,
                outputs,
                f"hud/spell-tier-{tier}.rgba8",
                tier_rgba,
                kind="rgba8_source_mapped_HFONT3_spell_tier",
                source_asset_ids=[
                    "owner_data:DATA/HFONT3.DAT",
                    "owner_data:DATA/CLRN-0.DAT",
                    "owner_data:DATA/PALN-0.DAT",
                ],
                extra={
                    "tier": tier,
                    "text": tier_text.decode("ascii"),
                    "width": tier_width,
                    "height": tier_height,
                    "palette_index": objective_background_index,
                },
            )
            spell_tier_rows.append({
                "tier": tier,
                "text": tier_text.decode("ascii"),
                "render_path": tier_output["path"],
                "render_width": tier_width,
                "render_height": tier_height,
                "glyph_indices": tier_glyphs,
                "palette_index": objective_background_index,
            })
        hud_bitmap_selectors = tuple(sorted(set(FRAME_HSPRN + DYNAMIC_HSPRN)))
        sprite_by_index = {row["index"]: row for row in sprite_rows}
        hud_bitmap_assets = []
        for selector in sorted(hud_bitmap_selectors):
            row = sprite_by_index[selector]
            hud_bitmap_assets.append({
                "selector": selector,
                "bank": "HSPRN0-0",
                "source_asset_id":
                    f"owner_data:DATA/HSPRN0-0.DAT#{selector}",
                "indices_path": row["indices_path"],
                "mask_path": row["mask_path"],
                "indices_sha256": row["indices_sha256"],
                "mask_sha256": row["mask_sha256"],
                "stored_width": row["width"],
                "stored_height": row["height"],
                "runtime_width": row["runtime_dimensions"]["width"],
                "runtime_height": row["runtime_dimensions"]["height"],
                "source_scale": row["logical_scale"],
                "palette": "PALN-0",
                "palette_effects":
                    "SourceMappedLive; native hit/speed/objective/exit RGB6 state",
                "filter": "nearest",
                "authority": "FrozenObserved",
            })
        source_frame_bitmap_assets = [
            {
                "selector": row["index"],
                "bank": "MSPRN0-0",
                "source_asset_id": row["source_asset_id"],
                "indices_path": row["indices_path"],
                "mask_path": row["mask_path"],
                "indices_sha256": row["indices_sha256"],
                "mask_sha256": row["mask_sha256"],
                "stored_width": row["width"],
                "stored_height": row["height"],
                "palette": "PALN-0",
                "filter": "nearest",
                "authority": "PinnedSourceMechanical",
            }
            for row in source_sprite_rows
        ]
        source_frame_font = {
            "bank": "FONT1",
            "mask_blob_path": source_font_output["path"],
            "mask_blob_bytes": source_font_output["bytes"],
            "mask_blob_sha256": source_font_output["sha256"],
            "glyph_count": len(source_glyph_rows),
            "glyphs": source_glyph_rows,
            "authority": "PinnedSourceMechanical",
        }
        adapted_frame_font = {
            "bank": "HFONT3",
            "mask_blob_path": font_output["path"],
            "mask_blob_bytes": font_output["bytes"],
            "mask_blob_sha256": font_output["sha256"],
            "glyph_count": len(glyph_rows),
            "glyphs": glyph_rows,
            "authority": "PinnedSourceMechanical",
        }
        source_cursor_asset = {
            "selector": cursor_selector,
            "system_cursor": {
                "bank": "POINTERS",
                "source_asset_id": cursor_catalog["asset_id"],
                "indices_path": cursor_indices_output["path"],
                "mask_path": cursor_mask_output["path"],
                "indices_sha256": cursor_indices_output["sha256"],
                "mask_sha256": cursor_mask_output["sha256"],
                "stored_width": cursor_width,
                "stored_height": cursor_height,
            },
            "fixed_software_cursor": next(
                asset
                for asset in source_frame_bitmap_assets
                if asset["selector"] == cursor_selector
            ),
            "adapted_software_cursor": next(
                asset
                for asset in hud_bitmap_assets
                if asset["selector"] == cursor_selector
            ),
            "palette": "PALN-0",
            "filter": "nearest",
            "authority": "PinnedSourceMechanical",
        }
        hud_descriptor = {
            "schema": "mc2-level1-owner-hud/v1",
            "bitmap_assets": hud_bitmap_assets,
            "localization_extent": {
                "record_count": LANGUAGE_RECORD_COUNT,
                "records_path": language_output["path"],
                "records_bytes": language_output["bytes"],
                "records_sha256": language_output["sha256"],
                "rows": language_record_rows,
            },
            "reference_layout": {
                "width": LEVEL1_REFERENCE_WIDTH,
                "height": LEVEL1_REFERENCE_HEIGHT,
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
            "top_bar": {
                "source_function": "DrawTopStatusBar_2D710@0x20e710",
                "panels": [
                    {
                        "semantic_role": "minimap_panel",
                        "selector": 40,
                        "x": 2,
                        "y": 2,
                        "authority": "SourceMappedLive",
                    },
                    {
                        "semantic_role": "castle_empty_panel",
                        "selector": 54,
                        "x": 126,
                        "y": 2,
                        "authority": "SourceMappedLive",
                    },
                    {
                        "semantic_role": "balloon_empty_panel",
                        "selector": 54,
                        "x": 254,
                        "y": 2,
                        "authority": "SourceMappedLive",
                    },
                    {
                        "semantic_role": "player_health_mana_panel",
                        "selector": 41,
                        "x": 382,
                        "y": 2,
                        "authority": "SourceMappedLive",
                    },
                    {
                        "semantic_role": "player_icon",
                        "selector": 43,
                        "x": 384,
                        "y": 2,
                        "authority": "SourceMappedLive",
                    },
                    {
                        "semantic_role": "player_health_mana_icons",
                        "selector": 42,
                        "x": 420,
                        "y": 2,
                        "authority": "SourceMappedLive",
                    },
                ],
                "health_mana_bars": {
                    "authority": "MissionStateLiveSourceMapped",
                    "formula":
                        "DrawBar_2D190 integer widths over global mana denominator; no background or inset",
                },
            },
            "spells": {
                "source_function": "DrawSpellIcon_2E260@0x20f260",
                "frame_selector": 1,
                "slots": [
                    {
                        "semantic_role": "left_fireball",
                        "x": 510,
                        "y": 2,
                        "holder_entity": 153,
                        "model": 0,
                        "icon_selector": 123,
                        "authority": "MissionStateLive",
                        "activation_authority": "SpellActivationLive",
                    },
                    {
                        "semantic_role": "right_possess",
                        "x": 574,
                        "y": 2,
                        "holder_entity": 154,
                        "model": 1,
                        "icon_selector": 124,
                        "authority": "MissionStateLive",
                        "activation_authority": "SpellActivationLive",
                    },
                ],
                "live_icon_selectors": [123, 124, 125, 126],
                "live_selector_authority":
                    "MissionStateLive; Fireball, Possession, Castle, Speed",
                "menu": {
                    "source_function": "DrawBottomSpellsMenu_2ECC0@0x20fcc0",
                    "rows": 2,
                    "columns": 13,
                    "reference_y": 408,
                    "edge_selector": 88,
                    "panel_selector": 89,
                    "selection_frame_selector": 90,
                    "disabled_panel_selector": 91,
                    "small_icon_base_selector": 97,
                    "left_assignment_corner_selector": 149,
                    "right_assignment_corner_selector": 150,
                    "tier_panel_selector": 161,
                    "tier_dark_panel_selector": 162,
                    "tier_locked_panel_selector": 163,
                    "tier_selection_frame_selector": 164,
                    "tier_number_base_selector": 165,
                    "tier_icon_base_selector": 179,
                    "tier_count": 3,
                    "panel_width": 48,
                    "panel_height": 36,
                    "edge_width": 8,
                    "authority": "SourceMappedLive",
                },
                "tier_rows": spell_tier_rows,
                "glow_tier_charge": {
                    "authority": "MissionStateLiveSourceMapped",
                    "reason":
                        "additive Level1 HUD ABI exposes holder timer, tier, charge divisor and caster mana",
                },
            },
            "objective": {
                "source_function": "DrawCurrentObjectiveTextbox_30630@0x211630",
                "selector_expression":
                    "IndexLevelText_DB4EE[0](48) + live ObjectiveText, or LevelEndText[0](53)",
                "selector": LEVEL1_OBJECTIVE_SELECTOR,
                "text_bytes_path": "hud/objective-text-048.bytes",
                "text_sha256": sha256_bytes(expected_objective),
                "text_encoding": "owner_bytes_ascii_subset_preserved",
                "font_bank": "HFONT3",
                "glyph_expression": "byte + 1; space advances glyph 33 width",
                "glyph_indices": objective_render["objective_glyph_indices"],
                "tint": {
                    "active_map_palette_lut": "CLRN-0",
                    "lut_selector": 3840,
                    "palette_index": objective_text_index,
                    "paln_rgb8": [252, 36, 36],
                    "historical_pointer_name":
                        "xadataclrd0dat; PaletteChanges_47760 installs CLRN for Night",
                },
                "background_blend": {
                    "source_table_path": tablesn_output["path"],
                    "table_offset": 0x4000,
                    "clrn_selector": 0,
                    "blend_selector": objective_background_index,
                    "expression":
                        "TABLESN[0x4000 + (CLRN[0] << 8) + live_destination_index]",
                    "underlay_scope":
                        "world_only_for_Level1_rows; all boxes begin below y=128 prior HUD extent",
                    "display_adaptation":
                        "pixel-centre nearest sample from qualified 320x200 indexed world "
                        "into 640x480 corrected 4:3 reference layout",
                },
                "draw_order": [
                    "TABLESN_background_blend",
                    "HFONT3_text_palette_119",
                    "HSPRN_frame_171_172_173",
                ],
                "frame_selectors": [171, 172, 173],
                "raster_path": objective_output["path"],
                "raster_plane_bytes":
                    objective_render["objective_foreground_plane_bytes"],
                "render_width": objective_render["objective_width"],
                "render_height": objective_render["objective_height"],
                "reference_x": objective_render["objective_x"],
                "reference_y": objective_render["objective_y"],
                "authority": "SourceMappedLiveIndexedFramebuffer",
                "objective_textbox_ticks": 33,
                "localization_records": objective_localization_rows,
                "rows": objective_rows,
            },
            "authority_badge": {
                "text": objective_render["badge_text_ascii"],
                "render_path": badge_output["path"],
                "render_width": objective_render["badge_width"],
                "render_height": objective_render["badge_height"],
                "reference_x": 192,
                "reference_y": 168,
            },
            "minimap": {
                "source_functions": [
                    "DrawMinimap_63600@0x244600",
                    "DrawMinimapEntites_61880@0x242880",
                    "DrawMinimapMarks_644F0@0x2454f0",
                ],
                "x": 0,
                "y": 0,
                "width": LEVEL1_MINIMAP_DISPLAY_SIDE,
                "height": LEVEL1_MINIMAP_DISPLAY_SIDE,
                "logical_width": LEVEL1_MINIMAP_LOGICAL_SIDE,
                "logical_height": LEVEL1_MINIMAP_LOGICAL_SIDE,
                "presentation_scale": HUD_SOURCE_SCALE,
                "map_mode":
                    "player_centered_yaw_rotated_TABLESN_scanline_aperture",
                "resolution_branch":
                    "HSPRN_native_128x128_scaling256",
                "sampling": {
                    "source_units_per_pixel": LEVEL1_MINIMAP_SOURCE_SCALE,
                    "transform":
                        "fixed_integer_remc2_sub_63C90_standard_display_branch",
                    "screen_column_vector": "(cosine_scaled,sine_scaled)",
                    "screen_row_vector": "(-sine_scaled,cosine_scaled)",
                    "terrain_base_index": "TABLESN[0x14000 + terrain_type]",
                    "terrain_shade_index":
                        "TABLESN[(terrain_shade << 8) | terrain_base_index]",
                    "palette": "PALN-0",
                    "framebuffer_blend_status":
                        "Deferred; live texture uses new indexed colour directly",
                },
                "aperture": {
                    "path": minimap_mask_output["path"],
                    "sha256": minimap_mask_output["sha256"],
                    "opaque_pixels": sum(minimap_mask),
                    "table_offset": 0x14300,
                    "table_curve_sha256": sha256_bytes(
                        tablesn[0x14300:0x14400]
                    ),
                    "phase_step": 0x1000000 // (
                        LEVEL1_MINIMAP_LOGICAL_SIDE // 2
                    ),
                    "scanlines": minimap_scanlines,
                },
                "rotation": {
                    "sine_table_path": minimap_sine_output["path"],
                    "sine_table_sha256": minimap_sine_output["sha256"],
                    "cardinal_direction_samples": minimap_direction_samples,
                    "octant_probe_samples": minimap_octant_probe,
                },
                "terrain_authority": "OwnerLevelStaticLiveSampling",
                "entity_positions_authority": "SourceArenaLive",
                "entity_overlay_mode":
                    "Rust_source_slot_ordered_DrawMinimapEntities_commands",
                "player_pose_authority": "PlayerFlightLive",
                "entity_colour_filter_status":
                    "SourceMappedLive; Night class/model/action/control/owner/phase filters, castle rope, sprites, direct dots, and single-blend centre cross",
                "objective_marks_status":
                    "SourceMappedLive; Rust target/order/blink/arrow geometry plus indexed Unity raster",
            },
            "deferred": [
                "balloon ownership and per-balloon life/mana rows",
                "minimap TABLESN framebuffer blend against the live 3D view",
                "textbox background TABLESN blend against the live framebuffer",
            ],
        }
        for row in objective_rows:
            selector = row["selector"]
            objective_text_output = write_output(
                stage,
                outputs,
                row["text_bytes_path"],
                language_l1[selector],
                kind="owner_localization_record_bytes",
                source_asset_ids=["owner_data:LANGUAGE/L1.TXT"],
                extra={
                    "selector": selector,
                    "authority": row["authority"],
                },
            )
            if objective_text_output["path"] != row["text_bytes_path"]:
                raise CacheError("objective descriptor output path mismatch")
        hud_output = write_output(
            stage,
            outputs,
            "hud/level1.json",
            canonical_json(hud_descriptor),
            kind="canonical_json_Level1_HUD_descriptor",
            source_asset_ids=[
                "owner_data:DATA/HSPRN0-0.DAT",
                "owner_data:DATA/HFONT3.DAT",
                "owner_data:LANGUAGE/L1.TXT",
                "owner_data:DATA/CLRN-0.DAT",
                "owner_data:DATA/PALN-0.DAT",
            ],
            extra={
                "authority_model": "Live_FrozenObserved_Deferred",
                "objective_selector": LEVEL1_OBJECTIVE_SELECTOR,
            },
        )
        source_frame_census = build_source_frame_census(
            pinned_source_evidence=pinned_source_evidence,
            runtime_captures=runtime_captures,
            runtime_profile=runtime_profile,
            reset_main_output=reset_main_output,
            reset_palette_output=reset_palette_output,
            language_rows=language_record_rows,
            language_output=language_output,
            source_bitmap_assets=source_frame_bitmap_assets,
            adapted_bitmap_assets=hud_bitmap_assets,
            source_font=source_frame_font,
            adapted_font=adapted_frame_font,
            cursor_asset=source_cursor_asset,
            source_tuple_census=source_tuple_census,
        )
        source_frame_output = write_output(
            stage,
            outputs,
            "frame/source-frame-census.json",
            canonical_json(source_frame_census),
            kind="canonical_json_Level1_source_frame_census",
            source_asset_ids=[
                "owner_data:DATA/SMATITLE.DAT",
                "owner_data:DATA/SMATITLE.PAL",
                "owner_data:DATA/MSPRN0-0.DAT",
                "owner_data:DATA/HSPRN0-0.DAT",
                "owner_data:DATA/FONT1.DAT",
                "owner_data:DATA/HFONT3.DAT",
                "owner_data:DATA/POINTERS.DAT",
                "owner_data:LANGUAGE/L1.TXT",
                "level_export:runtime.bin",
                "level_export:entities.bin",
                "level_export:players.bin",
                "level_export:authored-level.bin",
            ],
            extra={
                "runtime_capture_count": len(runtime_captures),
                "source_bitmap_selector_count":
                    len(source_frame_bitmap_assets),
                "adapted_bitmap_selector_count": len(hud_bitmap_selectors),
                "localization_record_count": len(language_record_rows),
                "reset_history_state": "Unprimed",
                "source_tuple_census_path": source_tuple_output["path"],
                "source_tuple_census_sha256": source_tuple_output["sha256"],
            },
        )

        with SPELL_TABLE.open(newline="", encoding="utf-8") as stream:
            spell_rows = list(csv.DictReader(stream))
        if len(spell_rows) != 78:
            raise CacheError("qualified spell tier table must have 78 rows")
        families = []
        for family in range(26):
            rows = [row for row in spell_rows if int(row["family_id"]) == family]
            rows.sort(key=lambda row: int(row["tier"]))
            if len(rows) != 3 or [int(row["tier"]) for row in rows] != [0, 1, 2]:
                raise CacheError(f"qualified spell family {family} is incomplete")
            small = {int(row["family_icon_small_index"]) for row in rows}
            big = {int(row["family_icon_big_index"]) for row in rows}
            tiers = [int(row["tier_icon_index"]) for row in rows]
            if small != {97 + family} or big != {123 + family} or tiers != [
                179 + family * 3 + tier for tier in range(3)
            ]:
                raise CacheError(f"qualified spell mapping mismatch at family {family}")
            families.append({
                "family_id": family,
                "family_small_index": next(iter(small)),
                "family_big_index": next(iter(big)),
                "tier_indices": tiers,
            })
        spell_indices = {
            "schema": "mc2-spell-icon-indices/v1",
            "bank": "HSPRN0-0",
            "status":
                "family_mapping_with_Level1_high_resolution_big_icons_materialized",
            "source_asset_id": "owner_data:DATA/SPELLS.DAT",
            "provenance": {
                "family_indices": "code-derived qualified family mapping",
                "tier_indices": "SPELLS-derived qualified tier table",
                "qualified_table_sha256": sha256_file(SPELL_TABLE),
                "qualified_generator_sha256": sha256_file(SPELL_GENERATOR),
            },
            "families": families,
        }
        spell_output = write_output(
            stage,
            outputs,
            "spells/icon-indices.json",
            canonical_json(spell_indices),
            kind="canonical_json_index_mapping",
            source_asset_ids=["owner_data:DATA/SPELLS.DAT"],
            extra={
                "family_count": 26,
                "bitmap_status":
                    "Level1_reset_big_icons_123_and_124_materialized; other families deferred",
            },
        )

        seal = {
            "schema": CONFORMANCE_SEAL_SCHEMA,
            # Provenance only: validation binds the semantic contract self-hash
            # below and deliberately does not compare this receipt hash.
            "contract_file_sha256": contract_file_sha256,
            "contract_self_sha256": contract_self_sha256,
            "remc2_commit": PINNED_REMC2_COMMIT,
            "identity": contract_identity,
        }
        seal["seal_sha256"] = sha256_bytes(canonical_identity_json(seal))
        write_output(
            stage,
            outputs,
            "conformance-identity.json",
            canonical_json(seal),
            kind="plan29_conformance_identity_seal",
            source_asset_ids=[],
            extra={"distribution": "private_non_owner_data_identity"},
        )

        outputs.sort(key=lambda row: row["path"])
        identity_input = b"".join(
            f"{row['path']}\0{row['sha256']}\0{row['bytes']}\n".encode("ascii")
            for row in outputs
        )
        manifest = {
            "schema": SCHEMA,
            "scope": "PRIVATE_TRACK_P_OWNER_DATA",
            "distribution": "owner_local_runtime_supplied_not_redistributable",
            "bindings": {
                "level_export_package_sha256": EXPECTED_PACKAGE,
                "level_asset_closure_sha256": EXPECTED_CLOSURE,
                "owner_catalog_sha256": catalog_binding,
                "qualified_runtime_capture_sha256": EXPECTED_RUNTIME_SHA256,
                "qualified_runtime_capture_count": len(runtime_captures),
                "goat_tmap_golden_file_sha256": EXPECTED_GOAT_TMAP_GOLDEN,
                "goat_tmap_oracle_manifest_sha256":
                    EXPECTED_GOAT_TMAP_ORACLE_MANIFEST,
                # Provenance only; the cache gate uses the self-hash below.
                "plan29_contract_file_sha256": contract_file_sha256,
                "plan29_contract_self_sha256": contract_self_sha256,
            },
            "generator": {
                "id": "remc2-unity-asset-cache",
                "version": GENERATOR_VERSION,
                "source_sha256": sha256_file(Path(__file__)),
            },
            "decoder": {
                "id": "remc2-atlas-proven-indexed-rle",
                "version": "catalog-v1",
                "source_sha256": sha256_file(PROVEN_DECODER),
                "verification": "materialized bytes must match proven decoder and qualified catalog hashes",
            },
            "rnc_decoder": rnc_provenance,
            "xmi_converter": xmi_provenance,
            "music_renderer": adl_provenance,
            "source_files": sorted(source_files, key=lambda row: row["path"]),
            "assets": {
                "palette": {
                    "path": "palette/paln.rgb6",
                    "status": "exact_ordered_rgb6_multiply_by_4_not_baked",
                    "tablesn_path": shade_lut_output["path"],
                    "clrn_path": clrn_output["path"],
                },
                "sky": {
                    "path": "sky/skyn.indices.u8",
                    "palette_status": "proven_night_binding",
                },
                "terrain_materials": {
                    "path": material_output["path"],
                    "palette_path": "palette/paln.rgb6",
                    "bl32_indices_path": bl32_output["path"],
                    "bl32_source_atlas_path": bl32_atlas_output["path"],
                    "bl32_tile_count": bl32_count,
                    "tmap_frame_used_selectors": sorted(frame_tmaps),
                    "tmap_scene_reachable_selector_count": len(scene_tmaps),
                    "tmap_decoded_texture_count":
                        sum(row["status"] == "decoded_indexed_texture" for row in tmap_rows),
                    "tmap_raw_placeholder_count":
                        sum(row["status"] == "raw_single_byte_placeholder_nondecoded" for row in tmap_rows),
                    "palette_status":
                        "exact_night_TABLESN_static_shade_then_PALN_indexed_runtime_lookup",
                    "animation_status":
                        "exact_Goat_152_154_mutable_states_materialized; "
                        "other_animated_TMAPs_base_frame_only",
                },
                "hsprn": sprite_rows,
                "entity_presentations": {
                    "path": entity_output["path"],
                    "status": "qualified_Level1_owner_actor_billboards",
                    "qualified_actor_count":
                        entity_descriptor["qualified_actor_count"],
                    "mapped_reset_effect_count":
                        entity_descriptor["mapped_reset_effect_count"],
                    "reset_chain_drawable_count":
                        entity_descriptor["reset_chain_drawable_count"],
                    "qualified_view_count":
                        entity_descriptor["qualified_view_count"],
                    "qualified_group_count":
                        entity_descriptor["qualified_group_count"],
                    "active_reset_count":
                        entity_descriptor["active_reset_count"],
                    "hidden_reset_count":
                        entity_descriptor["active_reset_count"]
                        - entity_descriptor["qualified_view_count"],
                    "suppressed_terrain_head_count":
                        entity_descriptor["suppressed_terrain_head_count"],
                    "off_chain_active_count":
                        entity_descriptor["off_chain_active_count"],
                },
                "hfont3": {
                    "mask_blob_path": font_output["path"],
                    "glyph_count": len(glyph_rows),
                    "glyphs": glyph_rows,
                    "language_records_path": language_output["path"],
                    "language_record_count": len(language_record_rows),
                    "language_records_sha256": language_output["sha256"],
                    "palette_status":
                        "mask_exact_runtime_tint_callsite_dynamic; "
                        "Level1 objective CLRN3840 qualified",
                },
                "font1": {
                    "mask_blob_path": source_font_output["path"],
                    "glyph_count": len(source_glyph_rows),
                    "glyphs": source_glyph_rows,
                    "palette_status":
                        "fixed_320_source_frame_mask_exact_runtime_tint_dynamic",
                },
                "source_frame_census": {
                    "path": source_frame_output["path"],
                    "status":
                        "accepted_fixed_profile_native_FINAL_MAIN_source_and_asset_census",
                    "runtime_capture_count": len(runtime_captures),
                    "reset_main_path": reset_main_output["path"],
                    "reset_main_history_state": "Unprimed",
                    "source_bitmap_selector_count":
                        len(source_frame_bitmap_assets),
                    "adapted_bitmap_selector_count":
                        len(hud_bitmap_selectors),
                    "cursor_selector": cursor_selector,
                    "pinned_source_blob_count":
                        pinned_source_evidence["validated_blob_count"],
                    "localization_record_count": len(language_record_rows),
                    "final_main_classification": "Exact",
                },
                "spell_icon_indices": {
                    "path": spell_output["path"],
                    "status":
                        "family_mapping_with_Level1_high_resolution_big_icons_materialized",
                },
                "level1_castle_stage": {
                    "path": build_descriptor_output["path"],
                    "cells_path": build_cells_output["path"],
                    "build_index": CASTLE_STAGE_ONE_BUILD_INDEX,
                    "effective_width": effective_width,
                    "effective_height": effective_height,
                    "status":
                        "exact_owner_BUILD_index_1_with_loader_and_consumer_scaling",
                },
                "level1_house_footprints": {
                    "path": house_build_descriptor_output["path"],
                    "cells_path": house_build_cells_output["path"],
                    "build_indices": list(LEVEL1_HOUSE_BUILD_INDICES),
                    "status":
                        "exact_owner_Level1_house_BUILD_rows_for_action_53",
                },
                "level1_hud": {
                    "path": hud_output["path"],
                    "status":
                        "qualified_owner_assets_mixed_Live_FrozenObserved_Deferred",
                    "objective_selector": LEVEL1_OBJECTIVE_SELECTOR,
                    "objective_text_path": objective_text_output["path"],
                    "objective_render_path": objective_output["path"],
                    "authority_badge_path": badge_output["path"],
                    "minimap_mode":
                        "player_centered_yaw_rotated_TABLESN_scanline_aperture",
                    "minimap_logical_resolution": [64, 64],
                    "minimap_display_resolution": [128, 128],
                },
                "level1_audio": {
                    "sound_descriptor_path": sound_descriptor_output["path"],
                    "music_descriptor_path": music_descriptor_output["path"],
                    "event_semantics_path": event_semantics_output["path"],
                    "sound_bank": 1,
                    "sound_quality": 822,
                    "sound_quality_index": 3,
                    "materialized_sound_ids":
                        list(LEVEL1_MATERIALIZED_SOUND_IDS),
                    "explicit_null_sound_ids": [12],
                    "music_channel": 0,
                    "music_driver_type": "f",
                    "music_driver_index": 2,
                    "logical_track": 1,
                    "playback_slot": 0,
                    "loop_scan_pointer_slot": 1,
                    "music_xmi_path": music_xmi_output["path"],
                    "music_midi_path": music_midi_output["path"],
                    "music_unity_wave_path": music_wave_output["path"],
                    "music_playback_status":
                        "Ready_deterministic_owner_local_adaptation_not_exact_DOS_timbre",
                },
            },
            "outputs": outputs,
            "cache_content_sha256": sha256_bytes(identity_input),
        }
        manifest["manifest_payload_sha256"] = sha256_bytes(canonical_json(manifest))
        (stage / "manifest.json").write_bytes(canonical_json(manifest))
        validate_cache(stage, EXPECTED_PACKAGE, EXPECTED_CLOSURE)
        fsync_tree(stage)

        commit_stage(output, stage)
        return manifest
    except Exception:
        if stage.exists():
            shutil.rmtree(stage)
        raise


def validate_cache(
    cache_dir: Path,
    expected_package: str = EXPECTED_PACKAGE,
    expected_closure: str = EXPECTED_CLOSURE,
    contract_path: Path = DEFAULT_CONFORMANCE_CONTRACT,
    expected_contract_sha256: str | None = None,
) -> dict[str, Any]:
    # Preserve the qualification runner's older call shape without reviving
    # the incidental contract-file receipt hash as a validation gate.
    del expected_contract_sha256
    cache_dir = cache_dir.expanduser().resolve()
    manifest_path = cache_dir / "manifest.json"
    manifest = load_json(manifest_path)
    manifest_claim = manifest.get("manifest_payload_sha256")
    manifest_payload = dict(manifest)
    manifest_payload.pop("manifest_payload_sha256", None)
    if sha256_bytes(canonical_json(manifest_payload)) != manifest_claim:
        raise CacheError("cache manifest metadata hash mismatch")
    if manifest.get("schema") != SCHEMA:
        raise CacheError("unsupported cache schema")
    bindings = manifest.get("bindings", {})
    if bindings.get("level_export_package_sha256") != expected_package:
        raise CacheError("cache package binding mismatch")
    if bindings.get("level_asset_closure_sha256") != expected_closure:
        raise CacheError("cache closure binding mismatch")
    contract, contract_self_sha256 = load_conformance_contract(contract_path)
    contract_identity = conformance_contract_identity_payload(contract)[
        "trusted_identity"
    ]
    if bindings.get("plan29_contract_self_sha256") != contract_self_sha256:
        raise CacheError("cache contract self-hash binding mismatch")
    outputs = manifest.get("outputs")
    if not isinstance(outputs, list) or not outputs:
        raise CacheError("cache manifest has no outputs")
    seen: set[str] = set()
    identity_parts: list[bytes] = []
    for row in outputs:
        relative = row.get("path")
        if (
            not isinstance(relative, str)
            or relative in seen
            or relative.startswith("/")
            or ".." in Path(relative).parts
        ):
            raise CacheError("invalid or duplicate cache output path")
        seen.add(relative)
        path = cache_dir / relative
        if not path.is_file():
            raise CacheError(f"cache output missing: {relative}")
        size = path.stat().st_size
        digest = sha256_file(path)
        if size != row.get("bytes"):
            raise CacheError(f"cache output truncated: {relative}")
        if digest != row.get("sha256"):
            raise CacheError(f"cache output hash mismatch: {relative}")
        identity_parts.append(
            f"{relative}\0{digest}\0{size}\n".encode("ascii")
        )
    actual_identity = sha256_bytes(b"".join(identity_parts))
    if actual_identity != manifest.get("cache_content_sha256"):
        raise CacheError("cache content identity mismatch")
    seal = load_json(cache_dir / "conformance-identity.json")
    if seal.get("schema") != CONFORMANCE_SEAL_SCHEMA:
        raise CacheError("cache conformance seal schema mismatch")
    if seal.get("contract_self_sha256") != contract_self_sha256:
        raise CacheError("cache conformance seal contract mismatch")
    if seal.get("identity") != contract_identity:
        raise CacheError("cache conformance seal identity mismatch")
    seal_claim = seal.get("seal_sha256")
    seal_unsigned = dict(seal)
    seal_unsigned.pop("seal_sha256", None)
    if sha256_bytes(canonical_identity_json(seal_unsigned)) != seal_claim:
        raise CacheError("cache conformance seal self-hash mismatch")
    expected_tree = {"manifest.json", *seen}
    actual_tree: set[str] = set()
    for path in cache_dir.rglob("*"):
        if path.is_symlink():
            raise CacheError("cache tree contains a symlink")
        if path.is_file():
            actual_tree.add(path.relative_to(cache_dir).as_posix())
    if actual_tree != expected_tree:
        raise CacheError("cache tree has missing or extra files")
    return manifest


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    build = subparsers.add_parser("build")
    build.add_argument("--owner-root", type=Path, default=DEFAULT_OWNER)
    build.add_argument("--package", type=Path, default=DEFAULT_PACKAGE)
    build.add_argument("--closure", type=Path, default=DEFAULT_CLOSURE)
    build.add_argument("--catalog", type=Path, default=DEFAULT_CATALOG)
    build.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    build.add_argument("--contract", type=Path, default=DEFAULT_CONFORMANCE_CONTRACT)
    build.add_argument(
        "--expected-contract-sha256",
        default=None,
        help=argparse.SUPPRESS,
    )
    build_day = subparsers.add_parser("build-day")
    build_day.add_argument("--owner-root", type=Path, default=DEFAULT_OWNER)
    build_day.add_argument("--package", type=Path, default=DEFAULT_LEVEL2_PACKAGE)
    build_day.add_argument("--closure", type=Path, default=DEFAULT_LEVEL2_CLOSURE)
    build_day.add_argument("--catalog", type=Path, default=DEFAULT_CATALOG)
    build_day.add_argument("--output", type=Path, default=DEFAULT_LEVEL2_OUTPUT)
    build_gameplay = subparsers.add_parser("build-day-gameplay")
    build_gameplay.add_argument("--owner-root", type=Path, default=DEFAULT_OWNER)
    build_gameplay.add_argument("--package", type=Path, default=DEFAULT_LEVEL2_PACKAGE)
    build_gameplay.add_argument("--closure", type=Path, default=DEFAULT_LEVEL2_CLOSURE)
    build_gameplay.add_argument("--catalog", type=Path, default=DEFAULT_CATALOG)
    build_gameplay.add_argument("--output", type=Path, default=DEFAULT_LEVEL2_GAMEPLAY_OUTPUT)
    build_audio = subparsers.add_parser("build-day-audio")
    build_audio.add_argument("--owner-root", type=Path, default=DEFAULT_OWNER)
    build_audio.add_argument("--package", type=Path, default=DEFAULT_LEVEL2_PACKAGE)
    build_audio.add_argument("--closure", type=Path, default=DEFAULT_LEVEL2_CLOSURE)
    build_audio.add_argument("--catalog", type=Path, default=DEFAULT_CATALOG)
    build_audio.add_argument("--output", type=Path, default=DEFAULT_LEVEL2_AUDIO_OUTPUT)
    validate = subparsers.add_parser("validate")
    validate.add_argument("--cache", type=Path, default=DEFAULT_OUTPUT)
    validate.add_argument("--package-sha256", default=EXPECTED_PACKAGE)
    validate.add_argument("--closure-sha256", default=EXPECTED_CLOSURE)
    validate.add_argument("--contract", type=Path, default=DEFAULT_CONFORMANCE_CONTRACT)
    validate.add_argument(
        "--expected-contract-sha256",
        default=None,
        help=argparse.SUPPRESS,
    )
    validate_day = subparsers.add_parser("validate-day")
    validate_day.add_argument("--cache", type=Path, default=DEFAULT_LEVEL2_OUTPUT)
    validate_day.add_argument("--package-sha256", default=EXPECTED_LEVEL2_PACKAGE)
    validate_day.add_argument("--closure-sha256", default=EXPECTED_LEVEL2_CLOSURE)
    validate_gameplay = subparsers.add_parser("validate-day-gameplay")
    validate_gameplay.add_argument("--cache", type=Path, default=DEFAULT_LEVEL2_GAMEPLAY_OUTPUT)
    validate_gameplay.add_argument("--package-sha256", default=EXPECTED_LEVEL2_PACKAGE)
    validate_gameplay.add_argument("--closure-sha256", default=EXPECTED_LEVEL2_CLOSURE)
    validate_audio = subparsers.add_parser("validate-day-audio")
    validate_audio.add_argument("--cache", type=Path, default=DEFAULT_LEVEL2_AUDIO_OUTPUT)
    validate_audio.add_argument("--package-sha256", default=EXPECTED_LEVEL2_PACKAGE)
    validate_audio.add_argument("--closure-sha256", default=EXPECTED_LEVEL2_CLOSURE)
    build_aggregate = subparsers.add_parser("build-day-aggregate")
    build_aggregate.add_argument("--owner-root", type=Path, default=DEFAULT_OWNER)
    build_aggregate.add_argument("--package", type=Path, default=DEFAULT_LEVEL2_PACKAGE)
    build_aggregate.add_argument("--closure", type=Path, default=DEFAULT_LEVEL2_CLOSURE)
    build_aggregate.add_argument("--catalog", type=Path, default=DEFAULT_CATALOG)
    build_aggregate.add_argument("--output", type=Path, default=DEFAULT_LEVEL2_AGGREGATE_OUTPUT)
    validate_aggregate = subparsers.add_parser("validate-day-aggregate")
    validate_aggregate.add_argument("--cache", type=Path, default=DEFAULT_LEVEL2_AGGREGATE_OUTPUT)
    validate_aggregate.add_argument("--package-sha256", default=EXPECTED_LEVEL2_PACKAGE)
    validate_aggregate.add_argument("--closure-sha256", default=EXPECTED_LEVEL2_CLOSURE)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.command == "build":
            manifest = build_cache(
                args.owner_root,
                args.package,
                args.closure,
                args.catalog,
                args.output,
                args.contract,
                args.expected_contract_sha256,
            )
            print(
                f"cache={args.output} outputs={len(manifest['outputs'])} "
                f"content_sha256={manifest['cache_content_sha256']}"
            )
        elif args.command == "build-day":
            manifest = build_day_material_cache(
                args.owner_root,
                args.package,
                args.closure,
                args.catalog,
                args.output,
            )
            print(
                f"cache={args.output} outputs={len(manifest['outputs'])} "
                f"content_sha256={manifest['cache_content_sha256']}"
            )
        elif args.command == "build-day-gameplay":
            manifest = build_day_gameplay_cache(
                args.owner_root,
                args.package,
                args.closure,
                args.catalog,
                args.output,
            )
            print(
                f"cache={args.output} outputs={len(manifest['outputs'])} "
                f"content_sha256={manifest['cache_content_sha256']}"
            )
        elif args.command == "build-day-audio":
            manifest = build_day_audio_cache(
                args.owner_root,
                args.package,
                args.closure,
                args.catalog,
                args.output,
            )
            print(
                f"cache={args.output} outputs={len(manifest['outputs'])} "
                f"content_sha256={manifest['cache_content_sha256']}"
            )
        elif args.command == "build-day-aggregate":
            manifest = build_day_aggregate_cache(
                args.owner_root,
                args.package,
                args.closure,
                args.catalog,
                args.output,
            )
            print(
                f"cache={args.output} outputs={len(manifest['outputs'])} "
                f"content_sha256={manifest['cache_content_sha256']}"
            )
        elif args.command == "validate-day":
            manifest = validate_day_material_cache(
                args.cache,
                args.package_sha256,
                args.closure_sha256,
            )
            print(
                f"valid schema={manifest['schema']} "
                f"content_sha256={manifest['cache_content_sha256']}"
            )
        elif args.command == "validate-day-gameplay":
            manifest = validate_day_gameplay_cache(
                args.cache,
                args.package_sha256,
                args.closure_sha256,
            )
            print(
                f"valid schema={manifest['schema']} "
                f"content_sha256={manifest['cache_content_sha256']}"
            )
        elif args.command == "validate-day-audio":
            manifest = validate_day_audio_cache(
                args.cache,
                args.package_sha256,
                args.closure_sha256,
            )
            print(
                f"valid schema={manifest['schema']} "
                f"content_sha256={manifest['cache_content_sha256']}"
            )
        elif args.command == "validate-day-aggregate":
            manifest = validate_day_aggregate_cache(
                args.cache,
                args.package_sha256,
                args.closure_sha256,
            )
            print(
                f"valid schema={manifest['schema']} "
                f"content_sha256={manifest['cache_content_sha256']}"
            )
        else:
            manifest = validate_cache(
                args.cache,
                args.package_sha256,
                args.closure_sha256,
                args.contract,
                args.expected_contract_sha256,
            )
            print(
                f"valid schema={manifest['schema']} "
                f"content_sha256={manifest['cache_content_sha256']}"
            )
        return 0
    except CacheError as error:
        print(f"ERROR: {error}")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
