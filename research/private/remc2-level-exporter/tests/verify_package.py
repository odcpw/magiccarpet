#!/usr/bin/env python3
"""Strict verifier for private mc2-owner-level-export/v1 through v5 packages."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import stat
import struct
import sys

SCHEMA_V1 = "mc2-owner-level-export/v1"
SCHEMA_V2 = "mc2-owner-level-export/v2"
SCHEMA_V3 = "mc2-owner-level-export/v3"
SCHEMA_V4 = "mc2-owner-level-export/v4"
SCHEMA_V5 = "mc2-owner-level-export/v5"
CAMPAIGN_PACKAGE_SCHEMA = "mc2-owner-campaign-package/v1"
LEVEL2_CAMPAIGN_PACKAGE_SCHEMA = "mc2-owner-campaign-package/v2"
LEVEL1_ASSET_CAMPAIGN_PACKAGE_SCHEMA = "mc2-owner-campaign-package/v3"
CAMPAIGN_DEFINITION_SCHEMA = "mc2-campaign-definition/v1"
LEVEL2_CAMPAIGN_DEFINITION_SCHEMA = "mc2-campaign-definition/v2"
LEVEL1_ASSET_CAMPAIGN_DEFINITION_SCHEMA = "mc2-campaign-definition/v3"
SCHEMAS = {SCHEMA_V1, SCHEMA_V2, SCHEMA_V3, SCHEMA_V4, SCHEMA_V5}
STATE_SEQUENCE = (
    "remc2-regression-test/memimages/regressions/level1/"
    "sequence-002285FF-00356038.bin"
)
STATE_SEQUENCE_SHA256 = (
    "021ab6f582cae920546dc3bcf55b06d00937fea5ddaabfeae079c0368b314e7f"
)
TERRAIN_ORACLE_HASHES = {
    "type": "59a9d0c19d12fc068bbd5e8364a66db5519040ceacb2aa008de31de7c5538a73",
    "height": "50f3a45214f435ac3b8423254a69b7cc87b80a8039085c25a0fe2033eed1ecba",
    "shade": "6e15d0bebdbbd89ab82eae6dbb1c3e8f1fdc000449aa35eb99a11a1d9bca139c",
    "angle": "342e64a393dc8b35518d0e10753734cbc1d2006ce85dcff1562c8a34172ec02f",
    "secondary_cave": "944112ef6eb54e042889c9cc841ed1d7bdb129c38c1c61ce4c8b8e81589f466b",
    "entity_indices": "d4a35266bee1854dfb8b4fb6c9175549a12e0de2edafff667cc0872d4cb3e72f",
}
V2_FILES = {
    "authored-level.bin",
    "entities.bin",
    "players.bin",
    "presentation.bin",
    "runtime.bin",
    "terrain.bin",
}
V3_FILES = V2_FILES | {
    "entities-post-features.bin",
    "feature-allocator.bin",
    "feature-construction-inputs.bin",
    "feature-end.bin",
    "feature-start.bin",
    "terrain-post-features.bin",
    "terrain-pre-features.bin",
}
V4_FILES = V3_FILES | {"collision-masks.bin"}
V5_FILES = V4_FILES
CAMPAIGN_ENVELOPE_FILES = {"campaign-definition.json", "owner-manifest.json"}
CAMPAIGN_FILES = V5_FILES | {"campaign-definition.json"}
LEVEL2_CAMPAIGN_FILES = CAMPAIGN_FILES | {"building-parameters.bin"}
LEVEL1_ASSET_FILES = V3_FILES | CAMPAIGN_ENVELOPE_FILES | {
    "fireball-projectile-base.indexed-sprite.json",
    "fireball-projectile-fade.indexed-sprite.json",
}
REM_C2_COMMIT = "6806c7204d17220ecc0430759f784aea8ee2ddb2"
# Census-proven per-level campaign profile facts keyed by the outer selection
# (external, internal, map_type_id).  Level 2 values are the qualified Day
# reset census; Level 3 values are the mc-iol7n byte-identical twin exports.
# A selection without an entry fails closed before any member is decoded.
CAMPAIGN_PROFILES = {
    (1, 0, 1): {
        "package_schema": LEVEL1_ASSET_CAMPAIGN_PACKAGE_SCHEMA,
        "definition_schema": LEVEL1_ASSET_CAMPAIGN_DEFINITION_SCHEMA,
        "map_type_name": "Night",
        "map_type_id": 1,
        "active_entities": 177,
        "bound_player_slots": 1,
        "runtime_player_count": 1,
        "runtime_music_selector": 1,
        "objective_stage_forms": {0, 5, 7},
        "objective_count": 5,
        "player_schema": "mc2-player-arena/v2",
        "player_component_version": 2,
        "player_record_bytes": 573,
        "owner_manifest_schema": SCHEMA_V3,
        "files": LEVEL1_ASSET_FILES,
        "has_presentation_assets": True,
        "source_fact_ids": {
            "feature-construction-inputs",
            "level-records",
        },
    },
    (2, 1, 0): {
        "package_schema": LEVEL2_CAMPAIGN_PACKAGE_SCHEMA,
        "definition_schema": LEVEL2_CAMPAIGN_DEFINITION_SCHEMA,
        "map_type_name": "Day",
        "map_type_id": 0,
        "active_entities": 114,
        "collision_bank_bytes": 4638,
        "collision_bank_sha256": "cd438a156fc7738c8221ac403be2c5efc1a4ffd96dbad5d9a53b2a75036ca432",
        "bound_player_slots": 2,
        "runtime_player_count": 2,
        "runtime_music_selector": 2,
        "objective_stage_forms": {3, 5, 7, 9},
        "objective_count": 5,
        "player_schema": "mc2-player-arena/v4",
        "player_component_version": 4,
        "player_record_bytes": 663,
        "owner_manifest_schema": None,
        "files": LEVEL2_CAMPAIGN_FILES,
        "has_presentation_assets": False,
        "source_fact_ids": {
            "building-parameters",
            "collision-mask-bank",
            "feature-construction-inputs",
            "level-records",
        },
    },
    (3, 2, 0): {
        "package_schema": CAMPAIGN_PACKAGE_SCHEMA,
        "definition_schema": CAMPAIGN_DEFINITION_SCHEMA,
        "map_type_name": "Day",
        "map_type_id": 0,
        "active_entities": 252,
        "collision_bank_bytes": 3718,
        "collision_bank_sha256": "e1c2bd67c7ce43d0378d274431c252c9a427bf1a17bb638a344940433f06f5e6",
        "bound_player_slots": 1,
        "runtime_player_count": 1,
        "runtime_music_selector": 2,
        "objective_stage_forms": {0, 5, 7},
        "objective_count": 5,
        "player_schema": "mc2-player-arena/v4",
        "player_component_version": 4,
        "player_record_bytes": 663,
        "owner_manifest_schema": None,
        "files": CAMPAIGN_FILES,
        "has_presentation_assets": False,
        "source_fact_ids": {
            "collision-mask-bank",
            "feature-construction-inputs",
            "level-records",
        },
    },
}
CAMPAIGN_SOURCE_ARTIFACTS = {
    "CD_Files/DATA/BLDGPRM.DAT": (304, "1c4a7fdef5d4b811f871441f0380faaacd6e5e689e73fc4628bf8c5b51ea6f00"),
    "CD_Files/DATA/BUILD0-0.DAT": (80064, "3db5d9a763dedafbe3077eeeef47431da0b5d397e311f6ee46719a0fc51e8e5d"),
    "CD_Files/DATA/BUILD0-0.TAB": (462, "e064a1520e96f65c13fde9a05918d240ee4f708ab6aefc793ec8b391f220c05e"),
    "CD_Files/DATA/MSPRD0-0.DAT": (49202, "a81783458f1b81da262d60bc1f4bc28012137437f2fcedb75af441a4fc777656"),
    "CD_Files/DATA/MSPRD0-0.TAB": (1572, "47fe03bbd99e1975755a8e47401977965b5ef63fe427a755a2f481ed393117ea"),
    "CD_Files/DATA/SPELLS.DAT": (2080, "7b426f0104f282d91bf7da904ed978e809135e12e0dfebd845e4a582d52b51cc"),
    "CD_Files/DATA/TABLESN.DAT": (83456, "f88974baf68f728fb534a4dab35397b9cb3ca2f424019bbde36858eebbf97495"),
    "CD_Files/LEVELS/LEVELS.DAT": (3063856, "3b7fae5d7c350486a4f62ad4b2e9fa2e4e52023a216b8a7f043ce02439b50222"),
    "CD_Files/LEVELS/LEVELS.TAB": (4000, "179a65f04d4aa7bc147cfd56ffb63a0da400dfd85542eb0fbda10265d6bc1bea"),
    "GAME/NETHERW/CLEVELS/LEVELS.DAT": (3063856, "3b7fae5d7c350486a4f62ad4b2e9fa2e4e52023a216b8a7f043ce02439b50222"),
    "GAME/NETHERW/CLEVELS/LEVELS.TAB": (4000, "179a65f04d4aa7bc147cfd56ffb63a0da400dfd85542eb0fbda10265d6bc1bea"),
}
LEVEL1_PRESENTATION_ASSETS = {
    "presentation.fireball.projectile.base": {
        "member": "fireball-projectile-base.indexed-sprite.json",
        "sha256": "331df08c5fa8483d5dd75dc10abd9dd76cc26918d6447403b3f3199bc31f521d",
        "palette_path": "CD_Files/DATA/PALN-0.DAT",
        "palette_sha256": "6b55d0714b939e8bd25cef89466806299dbae933f2f2f96e0cf8402fc607d16f",
        "palette_rgb8_sha256": "7c4dfbbc6c0f68a99214fe3ca93d7a1cf0dd658e17a40ad7ecb302152264f507",
    },
    "presentation.fireball.projectile.fade": {
        "member": "fireball-projectile-fade.indexed-sprite.json",
        "sha256": "61059124cba22dd3086baab36bb7d12a9ab28ebf37d394827a49bea4f6c96784",
        "palette_path": "CD_Files/DATA/PALF-0.DAT",
        "palette_sha256": "c83f900a3d91b68e968f20cc484c8e0ca9a2578211c4d058a747d7f8db2880b4",
        "palette_rgb8_sha256": "f0091d61149264890604486b1d4aceadddf62c0cc90f0c0083a3cae881240b6f",
    },
}
MSPRD0_0_TAB_SHA256 = "47fe03bbd99e1975755a8e47401977965b5ef63fe427a755a2f481ed393117ea"
MSPRD0_0_DAT_SHA256 = "a81783458f1b81da262d60bc1f4bc28012137437f2fcedb75af441a4fc777656"
LEVEL2_COLLISION_MASK_BANK_BYTES = 4638
LEVEL2_COLLISION_MASK_BANK_SHA256 = (
    "cd438a156fc7738c8221ac403be2c5efc1a4ffd96dbad5d9a53b2a75036ca432"
)
LEVEL2_V4_PLAYER_COMPONENT_VERSION = 3
LEVEL2_V4_PLAYER_RECORD_BYTES = 662
LEVEL2_PLAYER_COMPONENT_VERSION = 4
LEVEL2_PLAYER_RECORD_BYTES = 663
TYPE_STR_164_AUXILIARY_BYTES = 89
TYPE_STR_164_AUXILIARY_LAYOUT = {
    "schema": "mc2-type-str-164-auxiliary/v1",
    "record_offset": 573,
    "record_bytes": TYPE_STR_164_AUXILIARY_BYTES,
    "source_struct": "Type_str_164",
    "source_struct_bytes": 1136,
    "descriptor_bounds": {
        "source_min_inclusive": 38,
        "source_max_exclusive": 580,
        "record_min_inclusive": 0,
        "record_max_exclusive": TYPE_STR_164_AUXILIARY_BYTES,
    },
    "fields": [
        {
            "name": "word_0x26_38",
            "source_offset": 38,
            "record_offset": 0,
            "count": 8,
            "element_bytes": 2,
            "bytes": 16,
            "encoding": "i16_le",
        },
        {
            "name": "byte_0x154_340",
            "source_offset": 340,
            "record_offset": 16,
            "count": 1,
            "element_bytes": 1,
            "bytes": 1,
            "encoding": "u8",
        },
        {
            "name": "dword_0x16D_365",
            "source_offset": 365,
            "record_offset": 17,
            "count": 1,
            "element_bytes": 4,
            "bytes": 4,
            "encoding": "i32_le",
        },
        {
            "name": "word_0x1A2_418",
            "source_offset": 418,
            "record_offset": 21,
            "count": 1,
            "element_bytes": 2,
            "bytes": 2,
            "encoding": "i16_le",
        },
        {
            "name": "array_0x1FC_508",
            "source_offset": 508,
            "record_offset": 23,
            "count": 32,
            "element_bytes": 2,
            "bytes": 64,
            "encoding": "u16_le",
        },
        {
            "name": "word_0x242_578",
            "source_offset": 578,
            "record_offset": 87,
            "count": 1,
            "element_bytes": 2,
            "bytes": 2,
            "encoding": "i16_le",
        },
    ],
}
LEVEL2_V4_PLAYER_LAYOUT = {
    "index_offset": 0,
    "normalized_host_time_offset": 50,
    "legacy_v1_bytes": 565,
    "ambient_loop_5_distance_offset": 565,
    "ambient_loop_31_distance_offset": 569,
    "type_str_164_auxiliary": TYPE_STR_164_AUXILIARY_LAYOUT,
    "record_bytes": LEVEL2_V4_PLAYER_RECORD_BYTES,
}
BANISHMENT_STATE_BYTES = 1
BANISHMENT_STATE_LAYOUT = {
    "schema": "mc2-player-banishment-state/v1",
    "record_offset": 662,
    "record_bytes": BANISHMENT_STATE_BYTES,
    "source_struct": "type_str_0x2BDE",
    "source_struct_bytes": 2124,
    "source_offset": 6,
    "source_field": "byte_0x006_2BE4_11236",
    "encoding": "u8",
}
LEVEL2_PLAYER_LAYOUT = {
    **LEVEL2_V4_PLAYER_LAYOUT,
    "banishment_state_0x006": BANISHMENT_STATE_LAYOUT,
    "record_bytes": LEVEL2_PLAYER_RECORD_BYTES,
}
V3_PACKAGE_SHA256 = "15f1ca196b1ebf227860f468e419732c1ba917d57de9097acc5e2f647bbfd835"
V3_SOURCE_IDENTITY = {
    "upstream_commit": "6806c7204d17220ecc0430759f784aea8ee2ddb2",
    "upstream_worktree_diff_sha256": (
        "73fcffd66baaeeeab443d65b1e7bc333eba1f7e106e7918a552c5ac10ad33258"
    ),
    "compiled_source_sha256": (
        "b8cfdd278a5a946fc340b118597fd8d65f0404600de9bdbcfabb6a8c212c5fb5"
    ),
}
V3_FILE_HASHES = {
    "entities-post-features.bin": (
        "689b25fd48beb9632f3d1dda1011e40adc9e03e52b0defa7eda46fb12ed996c9"
    ),
    "feature-allocator.bin": (
        "b72938b072b1ea1c0f68f416f6d9700e61175d6c02ff621c005ce3c1c8648f36"
    ),
    "feature-construction-inputs.bin": (
        "9b38d898793fe9223a711ce6ee0bae53bc1260c2370d2383676f3a7812abe7a7"
    ),
    "feature-end.bin": (
        "b9c1d1e5f271b1de6db4ba03e579b594015092d8b048cf1f36229c42427208b0"
    ),
    "feature-start.bin": (
        "26388bdd17e751df50804222cc49701248fb589494bde2e4955b38880aed654d"
    ),
    "terrain-post-features.bin": (
        "e1acc0e3b40c9f82efe9e7b74035f21d8d39174efd766380c36d7c8963a94140"
    ),
    "terrain-pre-features.bin": (
        "bf90fe6865a1b09e3ffa05b47ddf6e36e95f9efa598eed8904b98df638127c05"
    ),
}
V3_TERRAIN_PLANE_HASHES = {
    "terrain-pre-features.bin": (
        "a7fad153ce68d80ea0236ba87bcb27d78f0a713145ec037d91a960977c188dd5",
        "a5e06ac561c540c44f4db44a563735ab28552c275ca6767ee6bd4597a0cde525",
        "517dd198e17f956eb19b054c8c7efb7aa93504dda60d8faf17254f84316ffcb0",
        "acf21ffd82f162c4dfe9e32c7d8313ebc7351e636953f145d1559fbd8dc64f32",
        "de2f256064a0af797747c2b97505dc0b9f3df0de4f489eac731c23ae9ca9cc31",
        "fa43239bcee7b97ca62f007cc68487560a39e19f74f3dde7486db3f98df8e471",
    ),
    "terrain-post-features.bin": (
        "d545e90bf972783e9696c0c50bc976d5a5e3cd5cf4378d2abe0d1b524615ca19",
        "8886bba60e1ab577ae08eab78d01721820c9510c0b678a9794eb6af1ea067e89",
        "0fd52e8859197de782c15f715da8163b6b20ef5f17c1b7db04f8c960b33e402f",
        "a415ad23b13a6aba0376a3f0db992615acd31b601c1fbae3a0f402a531f4a4ef",
        "de2f256064a0af797747c2b97505dc0b9f3df0de4f489eac731c23ae9ca9cc31",
        "98a9c8a98491af66484087944ffbbcb62705682df0b24a4adcb8aeb96d9ab9bc",
    ),
}

# The native exporter intentionally emits an open Level 2 profile until the
# corresponding remc2 regression corpus is sealed.  These values are the
# profile contract, not hashes copied from a Level 1 receipt.
LEVEL2_PROFILE = {
    "external": 2,
    "internal": 1,
    "map_type": {"id": 0, "name": "Day"},
    "active_entities": 114,
    "number_of_players": 2,
}
LEVEL2_PRESENTATION = {
    "map_type": "Day",
    "terrain_bank": "TMAPS0-0",
    "sprite_bank": "MSPRD0-0",
    "web_bank": "MWEBD0-0",
    "palette": "PALD-0",
    "sky_bank": "SKYD0-0",
    "vga_resolution_selector": 1,
    "texture_size": 32,
    "sound_bank": 1,
    "cursor": 9,
}


def require(condition: bool, message: str) -> None:
    """Raise a useful first-mismatch diagnostic for profile checks."""
    if not condition:
        raise AssertionError(message)


TYPE_STR_164_REFERENCE = {
    "schema": "mc2-type-str-164-auxiliary/v1",
    "source_struct": "Type_str_164",
    "descriptor_table": "engine_support.cpp:665-707",
    "initialization": {
        "word_0x26_38": ["global_types.h:236", "engine_support_converts.cpp:59-61"],
        "byte_0x154_340": ["EventsFunctions.cpp:5424-5425"],
        "dword_0x16D_365": ["EventsFunctions.cpp:43760"],
        "word_0x1A2_418": ["EventsFunctions.cpp:6811-6815"],
        "array_0x1FC_508": ["EventsFunctions.cpp:43886-43897"],
        "word_0x242_578": ["EventsFunctions.cpp:43811"],
    },
    "consumers": {
        "word_0x26_38": ["engine_support.cpp:665-666"],
        "byte_0x154_340": [
            "EventsFunctions.cpp:55909-55910",
            "EventsFunctions.cpp:56193-56194",
        ],
        "dword_0x16D_365": ["EventsFunctions.cpp:5436-5437"],
        "word_0x1A2_418": [
            "EventsFunctions.cpp:5358-5359",
            "EventsFunctions.cpp:5956",
            "EventsFunctions.cpp:6805-6818",
        ],
        "array_0x1FC_508": [
            "EventsFunctions.cpp:5377-5392",
            "EventsFunctions.cpp:5968",
            "EventsFunctions.cpp:6201-6206",
        ],
        "word_0x242_578": [
            "EventsFunctions.cpp:5379",
            "EventsFunctions.cpp:6201-6206",
            "EventsFunctions.cpp:6263-6266",
        ],
    },
}

BANISHMENT_STATE_REFERENCE = {
    "schema": "mc2-player-banishment-state/v1",
    "source_struct": "type_str_0x2BDE",
    "source_field": "byte_0x006_2BE4_11236",
    "source_offset": 6,
    "descriptor_table": "engine_support.cpp:601-613",
    "serialization": "engine_support_converts.cpp:239-249",
    "consumer": "EventsFunctions.cpp:40781-40785",
}


def verify_type_str_164_carrier(manifest: dict, expected_layout: dict) -> None:
    layout = manifest["record_layouts"]["player"]
    require(
        layout == expected_layout,
        "Level 2 player layout does not exactly describe the Type_str_164 carrier",
    )
    require(
        manifest["reference"]["type_str_164_auxiliary"] == TYPE_STR_164_REFERENCE,
        "Type_str_164 source initialization/consumer pin changed",
    )


def verify_banishment_state_carrier(manifest: dict) -> None:
    layout = manifest["record_layouts"]["player"]
    require(
        layout["banishment_state_0x006"] == BANISHMENT_STATE_LAYOUT,
        "Level 2 player banishment-state descriptor changed",
    )
    require(
        manifest["reference"]["banishment_state_0x006"] == BANISHMENT_STATE_REFERENCE,
        "Level 2 banishment-state source pin changed",
    )


STATE_HASH_ORDER = [
    "entity_index:u16", "position.x:u16", "position.y:u16", "position.z:i16",
    "yaw:i16", "display_pitch:i16", "effective_pitch:i16",
    "roll_accumulator:i16", "pitch_accumulator:i16", "target_speed:i16",
    "speed_changed_flag:i16", "actual_speed:i16", "strafe_speed:i16",
    "roll_delta:i16", "pitch_delta:i16", "recoil_speed:i16",
    "recoil_yaw:i16", "impulse_x:i16", "impulse_y:i16", "impulse_z:i16",
    "entity_fov:i16", "water_counter:i8", "speed_modifier:u8",
    "speed_modifier_counter:i8", "immobilize_counter:i8",
    "immobilize_subcounter:i8", "tether_prior_distance:i32",
    "tether_target_index:u16", "tether_holder_index:u16", "tether_tier:u16",
    "speed_spell_holder_index:u16", "entity_phase:u8",
    "ambient_loop_5_distance:i32", "ambient_loop_31_distance:i32",
    "player_color_index:u8", "music_countdown:i16", "recovery_latch:u8",
    "skip_flight_once:u8", "tick:u64",
]


def expected_field_mappings() -> dict:
    mappings = {}
    player_fields = {
        "entity_index": (4, "u16_le"),
        "effective_pitch": (38, "i16_le"),
        "roll_accumulator": (46, "i16_le"),
        "pitch_accumulator": (48, "i16_le"),
        "target_speed": (20, "i16_le"),
        "strafe_speed": (22, "i16_le"),
        "roll_delta": (16, "i16_le"),
        "pitch_delta": (18, "i16_le"),
        "recoil_speed": (32, "i16_le"),
        "recoil_yaw": (34, "i16_le"),
        "impulse_x": (54, "i16_le"),
        "impulse_y": (56, "i16_le"),
        "impulse_z": (58, "i16_le"),
        "speed_modifier": (44, "u8"),
        "speed_modifier_counter": (45, "i8"),
        "ambient_loop_5_distance": (565, "i32_le"),
        "ambient_loop_31_distance": (569, "i32_le"),
        "player_color_index": (40, "u8_low_of_i16_le"),
    }
    for field, (offset, encoding) in player_fields.items():
        mappings[field] = {
            "kind": "exported_field",
            "file": "players.bin",
            "record": "local_player_slot",
            "offset": offset,
            "encoding": encoding,
        }
    mappings["ambient_loop_5_distance"]["source_field"] = "dword_0x19A_410"
    mappings["ambient_loop_31_distance"]["source_field"] = "maxDistance_0x19E_414"
    entity_fields = {
        "position.x": (80, "u16_le"),
        "position.y": (82, "u16_le"),
        "position.z": (84, "i16_le"),
        "yaw": (32, "i16_le"),
        "display_pitch": (34, "i16_le"),
        "actual_speed": (134, "i16_le"),
        "entity_fov": (38, "i16_le"),
        "entity_phase": (66, "u8"),
    }
    for field, (offset, encoding) in entity_fields.items():
        mappings[field] = {
            "kind": "entity_relation_field",
            "file": "entities.bin",
            "offset": offset,
            "encoding": encoding,
        }
    for field, value in {
        "speed_changed_flag": 0,
        "water_counter": 0,
        "immobilize_counter": 0,
        "immobilize_subcounter": 0,
        "tether_prior_distance": 0,
        "tether_target_index": 0,
        "tether_holder_index": 0,
        "tether_tier": 0,
        "speed_spell_holder_index": 0,
        "music_countdown": 0,
        "recovery_latch": False,
        "tick": 0,
    }.items():
        mappings[field] = {"kind": "zero_at_seam", "value": value}
    mappings["skip_flight_once"] = {
        "kind": "entity_relation_bit",
        "file": "entities.bin",
        "offset": 17,
        "bit": 3,
    }
    return mappings


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def u16(data: bytes, offset: int) -> int:
    return struct.unpack_from("<H", data, offset)[0]


def u32(data: bytes, offset: int) -> int:
    return struct.unpack_from("<I", data, offset)[0]


def decode(data: bytes, offset: int, encoding: str) -> int:
    formats = {
        "u8": "<B",
        "i8": "<b",
        "u16_le": "<H",
        "i16_le": "<h",
        "i32_le": "<i",
    }
    if encoding == "u8_low_of_i16_le":
        assert data[offset + 1] == 0
        return data[offset]
    return struct.unpack_from(formats[encoding], data, offset)[0]


def flatten_expected_state(state: dict) -> dict:
    result = dict(state)
    position = result.pop("position")
    result.update({f"position.{axis}": value for axis, value in position.items()})
    return result


def state_hash(state: dict) -> str:
    result = bytearray()

    def add(name: str, width: int, signed: bool = False) -> None:
        result.extend(int(state[name]).to_bytes(width, "little", signed=signed))

    add("entity_index", 2)
    add("position.x", 2)
    add("position.y", 2)
    add("position.z", 2, True)
    for name in (
        "yaw", "display_pitch", "effective_pitch", "roll_accumulator",
        "pitch_accumulator", "target_speed", "speed_changed_flag",
        "actual_speed", "strafe_speed", "roll_delta", "pitch_delta",
        "recoil_speed", "recoil_yaw", "impulse_x", "impulse_y",
        "impulse_z", "entity_fov",
    ):
        add(name, 2, True)
    add("water_counter", 1, True)
    add("speed_modifier", 1)
    add("speed_modifier_counter", 1, True)
    add("immobilize_counter", 1, True)
    add("immobilize_subcounter", 1, True)
    add("tether_prior_distance", 4, True)
    for name in (
        "tether_target_index", "tether_holder_index", "tether_tier",
        "speed_spell_holder_index",
    ):
        add(name, 2)
    add("entity_phase", 1)
    add("ambient_loop_5_distance", 4, True)
    add("ambient_loop_31_distance", 4, True)
    add("player_color_index", 1)
    add("music_countdown", 2, True)
    add("recovery_latch", 1)
    add("skip_flight_once", 1)
    add("tick", 8)
    value = 0xCBF29CE484222325
    for byte in result:
        value = ((value ^ byte) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return f"{value:016x}"


def verify_ambient_sequence(upstream_root: Path, manifest: dict) -> None:
    path = upstream_root / STATE_SEQUENCE
    assert path.is_file()
    assert sha256(path) == STATE_SEQUENCE_SHA256
    data = path.read_bytes()
    frame_bytes = 224_790
    assert len(data) == 20 * frame_bytes
    observations = []
    for frame_index in range(20):
        frame = data[frame_index * frame_bytes : (frame_index + 1) * frame_bytes]
        player_slot = u16(frame, 0x000C)
        transient = 0x2BDE + player_slot * 0x084C + 0x03E6
        observations.append(
            (
                player_slot,
                struct.unpack_from("<i", frame, transient + 0x019A)[0],
                struct.unpack_from("<i", frame, transient + 0x019E)[0],
            )
        )
    assert observations == [(0, 2048, 1746)] * 20
    assert manifest["reference"]["ambient_distance_sequence"] == {
        "capture_site": "0x002285ff",
        "frames": {"first": 0, "last": 19, "count": 20},
        "local_player_slot": 0,
        "dword_0x19A_410": {
            "semantic": "ambient_loop_5_distance",
            "value": 2048,
            "invariant": True,
        },
        "maxDistance_0x19E_414": {
            "semantic": "ambient_loop_31_distance",
            "value": 1746,
            "invariant": True,
        },
    }


def verify_feature_generation_input(package: Path, manifest: dict) -> None:
    descriptor = manifest["feature_generation_oracle"]
    assert descriptor == {
        "provenance": (
            "pristine authored Level 1 rerun after live frame-zero outputs were copied"
        ),
        "start_phase": "after-GenerateLevelMap_43830-before-sub_49F30-sub_49270",
        "end_phase": "after-sub_49270-before-group0",
        "vga_selector": 1,
        "global_rng_barriers": [
            0x1B712AE9,
            0x2C34E568,
            0x3D870B47,
            0xAF843886,
            0xFA1A8925,
            0x05F99924,
            0xDB828483,
        ],
        "start_record": {
            "file": "feature-start.bin",
            "magic": "MFS3",
            "version": 1,
            "bytes": 16,
            "terrain_rng": {"offset": 8, "encoding": "u16_le"},
            "global_rng": {"offset": 12, "encoding": "u32_le"},
        },
        "end_record": {
            "file": "feature-end.bin",
            "magic": "MFE3",
            "version": 1,
            "bytes": 16,
            "terrain_rng": {"offset": 8, "encoding": "u16_le"},
            "global_rng": {"offset": 12, "encoding": "u32_le"},
        },
        "pre_terrain": {
            "file": "terrain-pre-features.bin",
            "bytes": 0x70000,
            "inactive_secondary_plane": "zero-normalized-non-cave",
        },
        "post_terrain": {
            "file": "terrain-post-features.bin",
            "bytes": 0x70000,
            "inactive_secondary_plane": "zero-normalized-non-cave",
        },
        "post_entities": {
            "file": "entities-post-features.bin",
            "magic": "MCE3",
            "version": 1,
            "record_bytes": 176,
            "slots": 1000,
            "active": 47,
            "inactive_payload": "zero-normalized-after-index-active-prefix",
            "map_heads": "terrain-post-features.bin entity-index plane",
            "map_links": "oldMapEntity/nextEntity in complete active records",
        },
        "allocator": {
            "file": "feature-allocator.bin",
            "magic": "MCA3",
            "version": 1,
            "header_bytes": 32,
            "relation_count": 1000,
            "trace_record_bytes": 8,
            "trace_provenance": (
                "source-derived expected operations; counters and relations captured live"
            ),
        },
        "construction_inputs": {
            "file": "feature-construction-inputs.bin",
            "magic": "MCI3",
            "version": 1,
            "record_count": 12,
            "selectors": [17, 26, 29, 30, 31, 34, 42, 47, 50, 51, 52, 53],
            "source": "live pinned BLDGPRM rows plus BUILD footprint bytes",
        },
    }
    feature_start = (package / "feature-start.bin").read_bytes()
    assert len(feature_start) == 16
    assert feature_start[:4] == b"MFS3"
    assert u16(feature_start, 4) == 1
    assert u16(feature_start, 6) == 16
    assert u16(feature_start, 8) == 9775
    assert u16(feature_start, 10) == 0
    assert u32(feature_start, 12) == 49098

    feature_end = (package / "feature-end.bin").read_bytes()
    assert len(feature_end) == 16
    assert feature_end[:4] == b"MFE3"
    assert u16(feature_end, 4) == 1
    assert u16(feature_end, 6) == 16
    assert u16(feature_end, 8) == 44470
    assert u16(feature_end, 10) == 0
    assert u32(feature_end, 12) == 0xDB828483

    for name in ("terrain-pre-features.bin", "terrain-post-features.bin"):
        terrain = (package / name).read_bytes()
        assert len(terrain) == 0x70000
        assert terrain[0x40000:0x50000] == bytes(0x10000)
        planes = (
            terrain[0x00000:0x10000],
            terrain[0x10000:0x20000],
            terrain[0x20000:0x30000],
            terrain[0x30000:0x40000],
            terrain[0x40000:0x50000],
            terrain[0x50000:0x70000],
        )
        assert tuple(hashlib.sha256(plane).hexdigest() for plane in planes) == (
            V3_TERRAIN_PLANE_HASHES[name]
        ), f"feature terrain plane hashes: {name}"

    feature_entities = verify_record_file(
        package / "entities-post-features.bin", b"MCE3", 1, 1000, 176
    )
    construction = (package / "feature-construction-inputs.bin").read_bytes()
    assert construction[:4] == b"MCI3"
    assert u16(construction, 4) == 1
    assert u16(construction, 6) == 16
    assert u16(construction, 8) == 12
    assert u16(construction, 10) == 1
    assert u32(construction, 12) == 0
    parameters = {}
    offset = 16
    for _ in range(12):
        selector = u16(construction, offset)
        scalar = u16(construction, offset + 2)
        flags = construction[offset + 4]
        successor = construction[offset + 5]
        width = construction[offset + 6]
        height = construction[offset + 7]
        extent = u32(construction, offset + 8)
        assert width > 0 and height > 0 and extent == width * height * 2
        parameters[selector] = (scalar, flags, successor)
        offset += 12 + extent
    assert offset == len(construction)
    assert parameters == {
        17: (500, 0x18, 0),
        26: (20, 0x03, 0),
        29: (28, 0x03, 0),
        30: (25, 0x03, 0),
        31: (29, 0x03, 0),
        34: (118, 0x03, 0),
        42: (52, 0x03, 0),
        47: (34, 0x03, 0),
        50: (24, 0x03, 0),
        51: (30, 0x08, 0),
        52: (30, 0x08, 0),
        53: (30, 0x08, 0),
    }

    active = []
    for index in range(1000):
        offset = 12 + index * 176
        record = feature_entities[offset : offset + 176]
        assert u16(record, 0) == index
        if not record[2]:
            assert record[3:] == bytes(173)
            continue
        active.append(index)
        assert record[3] == 0
        assert record[67] == 10
        assert record[68] == 45, f"feature entity slot {index} model"
        assert record[69] == 0, f"feature entity slot {index} xtype"
        assert record[73] == 52
        selector = record[74]
        assert struct.unpack_from("<i", record, 12)[0] == parameters[selector][0] * 1000
        assert record[66] == (64 if index == 2 else index + 31)
        assert u16(record, 24) == (35109 + index if index == 2 else 39204 + index)
    assert active == list(range(1, 48))

    terrain_post = (package / "terrain-post-features.bin").read_bytes()
    linked = set()
    for cell in range(0x10000):
        slot = u16(terrain_post, 0x50000 + cell * 2)
        newer = 0
        while slot:
            assert 1 <= slot <= 47 and slot not in linked
            record = feature_entities[12 + slot * 176 : 12 + (slot + 1) * 176]
            assert u16(record, 28) == newer
            linked.add(slot)
            newer = slot
            slot = u16(record, 26)
    assert linked == set(range(1, 48))

    allocator = (package / "feature-allocator.bin").read_bytes()
    assert len(allocator) == 4440
    assert allocator[:4] == b"MCA3"
    assert u16(allocator, 4) == 1
    assert u16(allocator, 6) == 32
    assert struct.unpack_from("<i", allocator, 8)[0] == 951
    assert struct.unpack_from("<i", allocator, 12)[0] == -1
    assert u32(allocator, 16) == 952
    assert u32(allocator, 20) == 0
    assert u32(allocator, 24) == 51
    assert u16(allocator, 28) == 2
    assert u16(allocator, 30) == 8
    free_relations = struct.unpack_from("<1000H", allocator, 32)
    reclaim_relations = struct.unpack_from("<1000H", allocator, 2032)
    assert free_relations[:952] == tuple(
        range(999, 47, -1)
    ), "feature allocator free relations"
    assert free_relations[952:] == (0,) * 48
    assert reclaim_relations == (0,) * 1000

    trace = [
        struct.unpack_from("<BBHHH", allocator, 4032 + index * 8)
        for index in range(51)
    ]
    assert trace[:4] == [
        (1, 1, 1, 62, 0),
        (1, 1, 2, 63, 0),
        (2, 1, 1, 62, 0),
        (2, 1, 2, 63, 0),
    ]
    authored = (package / "authored-level.bin").read_bytes()
    houses = []
    for index in range(1, 1200):
        record = authored[1091 + index * 20 : 1091 + (index + 1) * 20]
        if u16(record, 0) == 10 and u16(record, 2) == 45:
            activation_group = struct.unpack_from("<h", record, 8)[0]
            if activation_group == -1:
                houses.append((index, u16(record, 14)))
                assert u16(record, 16) == 0
    special = [house for house in houses if parameters[house[1]][1] & 0x10]
    ordinary = [house for house in houses if not parameters[house[1]][1] & 0x10]
    assert special == [(3, 17)]
    assert len(ordinary) == 46 and ordinary[0][0] == 7
    expected_house_trace = [(1, 2, 2, 3, 0)]
    expected_house_trace.extend(
        (1, 3, 1 if order == 0 else order + 2, row, 0)
        for order, (row, _) in enumerate(ordinary)
    )
    assert trace[4:] == expected_house_trace


def verify_record_file(
    path: Path,
    magic: bytes,
    expected_version: int,
    expected_count: int,
    manifest_record_size: int,
) -> bytes:
    data = path.read_bytes()
    assert data[:4] == magic
    assert u16(data, 4) == expected_version
    record_size = u16(data, 6)
    count = u32(data, 8)
    assert record_size == manifest_record_size
    assert count == expected_count
    assert len(data) == 12 + record_size * count
    return data


def verify_reset_projection(
    manifest: dict, players: bytes, entities: bytes, runtime: bytes
) -> None:
    contract = manifest["reset_projection"]
    assert contract["schema"] == "mc2-flight-reset-projection/v1"
    assert contract["source"] == {
        "capture_frame": 0,
        "local_player_slot": {
            "file": "runtime.bin",
            "offset": 16,
            "encoding": "i16_le",
            "minimum": 0,
            "maximum": 7,
        },
        "entity_relation": (
            "players.bin local-player record offset 4 is the zero-based "
            "entities.bin record index"
        ),
    }
    mappings = contract["field_mappings"]
    assert mappings == expected_field_mappings()
    expected = flatten_expected_state(contract["expected_state"])
    assert set(mappings) == set(expected)
    player_size = manifest["record_sizes"]["player"]
    entity_size = manifest["record_sizes"]["entity"]
    local_player_slot = struct.unpack_from("<h", runtime, 16)[0]
    assert 0 <= local_player_slot <= 7
    player_start = 12 + local_player_slot * player_size
    entity_index = u16(players, player_start + 4)
    assert 0 <= entity_index < manifest["counts"]["entity_slots"]
    entity_start = 12 + entity_index * entity_size
    assert entities[entity_start + 2] == 1
    projected = {}
    encoding_widths = {
        "u8": 1, "i8": 1, "u16_le": 2, "i16_le": 2,
        "i32_le": 4, "u8_low_of_i16_le": 2,
    }
    for field, mapping in mappings.items():
        kind = mapping["kind"]
        if kind == "zero_at_seam":
            projected[field] = mapping["value"]
        elif kind == "exported_field":
            assert mapping["file"] == "players.bin"
            assert mapping["record"] == "local_player_slot"
            assert mapping["encoding"] in encoding_widths
            assert (
                0 <= mapping["offset"]
                <= player_size - encoding_widths[mapping["encoding"]]
            )
            projected[field] = decode(
                players, player_start + mapping["offset"], mapping["encoding"]
            )
        elif kind == "entity_relation_field":
            assert mapping["file"] == "entities.bin"
            assert mapping["encoding"] in encoding_widths
            assert (
                0 <= mapping["offset"]
                <= entity_size - encoding_widths[mapping["encoding"]]
            )
            projected[field] = decode(
                entities, entity_start + mapping["offset"], mapping["encoding"]
            )
        elif kind == "entity_relation_bit":
            assert mapping["file"] == "entities.bin"
            assert 0 <= mapping["offset"] < entity_size
            projected[field] = bool(
                entities[entity_start + mapping["offset"]] & (1 << mapping["bit"])
            )
        else:
            raise AssertionError(f"unknown reset projection kind {kind!r}")
    assert projected == expected
    assert contract["state_hash"] == {
        "api": "mc_sim_reset",
        "schema": "mc2-flight-state-hash/fnv1a64-v1",
        "algorithm": "FNV-1a 64-bit",
        "byte_order": "little-endian",
        "canonical_bytes": 81,
        "ordered_fields": STATE_HASH_ORDER,
        "hex": "9da4bc91af58f3ce",
    }
    assert state_hash(projected) == contract["state_hash"]["hex"]
    assert contract["conformance_hash"] == {
        "schema": "mc2-flight-conformance-hash/fnv1a64-v1",
        "context": "NativeSim::owner_level_one",
        "hex": "77b96b82492d6f4b",
        "derivation": (
            "sim-core conformance_hash over reset FlightState and sim-capi ABI "
            "v1 owner-level-one FlightContext"
        ),
    }


def authored_model_45_rows(authored: bytes) -> list[tuple[int, int]]:
    require(len(authored) == 0x6604, "authored-level.bin extent changed")
    rows = []
    for index in range(1, 1200):
        record = authored[1091 + index * 20 : 1091 + (index + 1) * 20]
        if (
            u16(record, 0) == 10
            and struct.unpack_from("<h", record, 2)[0] == 45
            and struct.unpack_from("<h", record, 8)[0] == -1
        ):
            rows.append((index, u16(record, 14)))
    return rows


def validate_collision_mask_contract(
    authored: bytes,
    content: bytes,
    metadata: dict,
    *,
    external_level: int,
) -> list[dict]:
    authored_rows = authored_model_45_rows(authored)
    declared_rows = metadata.get("selectors")
    require(isinstance(declared_rows, list), "collision-mask selector metadata is missing")
    require(len(content) >= 20, "collision-mask carrier header is truncated")
    require(content[:4] == b"MCM1", "collision-mask carrier magic mismatch")
    require(u16(content, 4) == 1, "collision-mask carrier version mismatch")
    require(u16(content, 6) == 20, "collision-mask carrier header extent mismatch")
    require(
        u16(content, 8) == metadata.get("vga_resolution_selector"),
        "collision-mask VGA selector mismatch",
    )
    require(u16(content, 10) == 0, "collision-mask header reserved field is non-zero")
    count = u32(content, 12)
    require(count == len(declared_rows), "collision-mask count disagrees with metadata")
    require(u32(content, 16) == 12, "collision-mask record header extent mismatch")

    cursor = 20
    previous_selector = -1
    parsed_rows = []
    for _ in range(count):
        require(cursor + 12 <= len(content), "collision-mask record header is truncated")
        selector = u16(content, cursor)
        width = u16(content, cursor + 2)
        height = u16(content, cursor + 4)
        require(u16(content, cursor + 6) == 0, "collision-mask record reserved field is non-zero")
        pixel_bytes = u32(content, cursor + 8)
        require(selector > previous_selector, "collision-mask selectors are not strictly ordered")
        require(width > 0 and height > 0, "collision-mask dimensions are empty")
        expected_bytes = width * height * 2
        require(pixel_bytes == expected_bytes, "collision-mask pixel extent mismatch")
        payload_start = cursor + 12
        payload_end = payload_start + pixel_bytes
        require(payload_end <= len(content), "collision-mask payload is truncated")
        payload = content[payload_start:payload_end]
        parsed_rows.append({
            "selector": selector,
            "width": width,
            "height": height,
            "pixel_bytes": pixel_bytes,
            "sha256": hashlib.sha256(payload).hexdigest(),
        })
        previous_selector = selector
        cursor = payload_end
    require(cursor == len(content), "collision-mask carrier has trailing bytes")
    require(parsed_rows == declared_rows, "collision-mask metadata differs from decoded records")

    expected_selectors = sorted({selector for _index, selector in authored_rows})
    actual_selectors = [row["selector"] for row in parsed_rows]
    require(
        actual_selectors == expected_selectors,
        f"external Level {external_level} collision-mask selectors differ from authored rows",
    )
    if external_level >= 3:
        recorded_count = metadata.get("authored_model_45_row_count")
        require(
            isinstance(recorded_count, int)
            and not isinstance(recorded_count, bool)
            and recorded_count >= 0,
            f"external Level {external_level} authored model-45 row count is missing",
        )
        require(
            recorded_count == len(authored_rows),
            f"external Level {external_level} authored model-45 row count disagrees with authored data",
        )
    return parsed_rows


def verify_collision_masks(package: Path, manifest: dict) -> None:
    external_level = manifest["level"]["external"]
    metadata = manifest.get("collision_masks")
    require(
        isinstance(metadata, dict),
        f"external Level {external_level} collision-mask metadata is missing",
    )
    base_metadata = {
        "file": "collision-masks.bin",
        "magic": "MCM1",
        "version": 1,
        "header_bytes": 20,
        "record_header_bytes": 12,
        "bank": "MSPRD0-0",
        "vga_resolution_selector": 1,
        "selector_source": "authored class-10/model-45 par1 selectors",
        "pixel_encoding": "signed i16 little-endian",
        "pixel_order": "row-major (y then x)",
        "transparent_value": -1,
    }
    if external_level == 2:
        assert metadata == {
            **base_metadata,
            "source_files": metadata["source_files"],
            "selectors": metadata["selectors"],
        }
    else:
        require(
            set(metadata) == set(base_metadata) | {"source_files", "selectors", "authored_model_45_row_count"},
            f"external Level {external_level} collision-mask metadata shape changed",
        )
        for key, expected in base_metadata.items():
            require(metadata.get(key) == expected, f"external Level {external_level} collision-mask metadata field {key} changed")
    source_files = metadata["source_files"]
    require(set(source_files) == {"tab", "dat"}, "collision-mask source file set differs")
    for key, logical_path, expected_bytes, expected_hash in (
        ("tab", "CD_Files/DATA/MSPRD0-0.TAB", 1572, MSPRD0_0_TAB_SHA256),
        ("dat", "CD_Files/DATA/MSPRD0-0.DAT", 49202, MSPRD0_0_DAT_SHA256),
    ):
        entry = source_files[key]
        assert entry == {
            "logical_path": logical_path,
            "bytes": expected_bytes,
            "sha256": expected_hash,
        }

    content_path = package / "collision-masks.bin"
    require(
        content_path.is_file(),
        f"external Level {external_level} collision-mask member is absent",
    )
    content = content_path.read_bytes()
    if external_level == 2:
        require(
            len(content) == LEVEL2_COLLISION_MASK_BANK_BYTES,
            "Level 2 collision-mask bank byte identity differs from the qualified source bank",
        )
        require(
            hashlib.sha256(content).hexdigest() == LEVEL2_COLLISION_MASK_BANK_SHA256,
            "Level 2 collision-mask bank SHA-256 differs from the qualified source bank",
        )
    validate_collision_mask_contract(
        (package / "authored-level.bin").read_bytes(),
        content,
        metadata,
        external_level=external_level,
    )


def verify_binary_structure(package: Path, manifest: dict) -> None:
    external_level = manifest["level"]["external"]
    level_two_v4 = external_level == 2 and manifest["schema"] == SCHEMA_V4
    level_two_v5 = external_level == 2 and manifest["schema"] == SCHEMA_V5
    extended_v5 = external_level >= 2 and manifest["schema"] == SCHEMA_V5
    authored = (package / "authored-level.bin").read_bytes()
    assert len(authored) == 0x6604
    require(
        authored[6] == manifest["level"]["map_type"]["id"],
        "authored-level.bin map type disagrees with manifest level identity",
    )

    entities = verify_record_file(
        package / "entities.bin",
        b"MCE1",
        1,
        1000,
        manifest["record_sizes"]["entity"],
    )
    entity_record_size = manifest["record_sizes"]["entity"]
    assert entity_record_size == 176
    active_entity_count = 0
    for index in range(1000):
        offset = 12 + index * entity_record_size
        assert u16(entities, offset) == index
        assert entities[offset + 2] in (0, 1)
        active_entity_count += entities[offset + 2]
        next_relation = u32(entities, offset + 4)
        require(
            0 <= next_relation <= 1000,
            f"entity slot {index} next relation is out of range",
        )
        assert 0 <= u32(entities, offset + 164) <= 157
        special_domain = entities[offset + 168]
        special_relation = u32(entities, offset + 172)
        assert special_domain in (0, 1, 2)
        if special_domain == 0:
            assert special_relation == 0
        elif special_domain == 1:
            assert 1 <= special_relation <= 8
        else:
            assert special_relation == 1
    require(
        active_entity_count == manifest["counts"]["active_entities"],
        "exported active entity count differs from manifest",
    )

    players = verify_record_file(
        package / "players.bin",
        b"MCP1",
        (
            LEVEL2_PLAYER_COMPONENT_VERSION
            if extended_v5
            else LEVEL2_V4_PLAYER_COMPONENT_VERSION
        )
        if level_two_v4 or extended_v5
        else (2 if manifest["schema"] in {SCHEMA_V2, SCHEMA_V3, SCHEMA_V4, SCHEMA_V5} else 1),
        8,
        manifest["record_sizes"]["player"],
    )
    player_record_size = manifest["record_sizes"]["player"]
    expected_player_size = (
        LEVEL2_PLAYER_RECORD_BYTES
        if extended_v5
        else (
            LEVEL2_V4_PLAYER_RECORD_BYTES
            if level_two_v4
            else (573 if manifest["schema"] in {SCHEMA_V2, SCHEMA_V3, SCHEMA_V4, SCHEMA_V5} else 565)
        )
    )
    assert player_record_size == expected_player_size
    for index in range(8):
        offset = 12 + index * player_record_size
        assert u16(players, offset) == index
        assert players[offset + 50 : offset + 54] == bytes(4)
        require(
            u16(players, offset + 4) < manifest["counts"]["entity_slots"],
            f"player slot {index} entity relation is out of range",
        )
    if extended_v5:
        verify_type_str_164_carrier(manifest, LEVEL2_PLAYER_LAYOUT)
        verify_banishment_state_carrier(manifest)
        layout = manifest["record_layouts"]["player"]
        auxiliary_offset = layout["type_str_164_auxiliary"]["record_offset"]
        auxiliary_end = auxiliary_offset + TYPE_STR_164_AUXILIARY_BYTES
        banishment_offset = layout["banishment_state_0x006"]["record_offset"]
        if level_two_v5:
            require(
                [
                    players[12 + index * player_record_size + banishment_offset]
                    for index in range(8)
                ]
                == [1, 1, 0, 0, 0, 0, 0, 0],
                "Level 2 v5 banishment-state values differ from the qualified source capture",
            )
            require(
                [
                    struct.unpack_from("<h", players, 12 + index * player_record_size + 6)[0]
                    for index in range(8)
                ]
                == [31] * 8,
                "Level 2 v5 compact ActPlayerIndex values differ from the qualified source capture",
            )
        for index in range(8):
            offset = 12 + index * player_record_size
            require(
                len(players[offset : offset + player_record_size]) >= auxiliary_end,
                f"external Level {external_level} player slot {index} Type_str_164 carrier is truncated",
            )
            require(
                len(players[offset : offset + player_record_size])
                >= banishment_offset + BANISHMENT_STATE_BYTES,
                f"external Level {external_level} player slot {index} banishment-state carrier is truncated",
            )
    elif level_two_v4:
        verify_type_str_164_carrier(manifest, LEVEL2_V4_PLAYER_LAYOUT)
        layout = manifest["record_layouts"]["player"]
        require(
            "banishment_state_0x006" not in layout
            and "banishment_state_0x006" not in manifest["reference"],
            "Level 2 v4 inspection layout must not contain the v5 banishment-state carrier",
        )
    elif manifest["schema"] in {SCHEMA_V2, SCHEMA_V3, SCHEMA_V4, SCHEMA_V5}:
        layout = manifest["record_layouts"]["player"]
        assert layout == {
            "index_offset": 0,
            "normalized_host_time_offset": 50,
            "legacy_v1_bytes": 565,
            "ambient_loop_5_distance_offset": 565,
            "ambient_loop_31_distance_offset": 569,
            "record_bytes": 573,
        }
        assert struct.unpack_from("<i", players, 12 + 565)[0] == 2048
        assert struct.unpack_from("<i", players, 12 + 569)[0] == (
            1746 if external_level == 1 else 2048
        )

    runtime = (package / "runtime.bin").read_bytes()
    assert runtime[:4] == b"MCR1"
    assert u16(runtime, 4) == 1
    assert u16(runtime, 6) == 0
    if external_level == 2:
        local_player_count = u16(runtime, 18)
        require(
            local_player_count == LEVEL2_PROFILE["number_of_players"],
            "Level 2 runtime local player count differs",
        )
        for slot in range(local_player_count):
            player_offset = 12 + slot * player_record_size
            entity_index = u16(players, player_offset + 4)
            entity_offset = 12 + entity_index * entity_record_size
            require(
                entities[entity_offset + 2] == 1,
                f"Level 2 player slot {slot} does not reference an active entity",
            )
    divergences = manifest["reference"]["runtime_scalar_divergences"]
    if external_level == 1:
        assert divergences == [
            {
                "field": "maptypeMusic_0x235",
                "original_state_offset": "0x235",
                "runtime_bin_offset": 32,
                "exported_remc2_value": 1,
                "preserved_dos_frames": "0..19",
                "preserved_dos_value": 0,
                "status": "presentation_environment_divergence",
                "evidence": (
                    "native remc2 selects the Night music bank when music is enabled; "
                    "the oracle capture had music inactive"
                ),
            }
        ]
        assert u32(runtime, 32) == 1
    elif external_level == 2:
        require(
            divergences == [],
            "Level 2 runtime scalar divergences must be empty",
        )
        require(u32(runtime, 32) == 2, "Level 2 runtime music selector must be 2")
    else:
        require(
            divergences == [],
            f"external Level {external_level} runtime scalar divergences must be empty",
        )
        expected_music_selector = {"Day": 2, "Night": 1, "Cave": 3}[manifest["level"]["map_type"]["name"]]
        require(
            u32(runtime, 32) == expected_music_selector,
            f"external Level {external_level} runtime music selector differs from map type",
        )
    if external_level == 1 and manifest["schema"] in {SCHEMA_V2, SCHEMA_V3, SCHEMA_V4}:
        verify_reset_projection(manifest, players, entities, runtime)
    elif external_level == 1:
        assert "reset_projection" not in manifest
    runtime_layout = manifest["record_layouts"]["runtime"]
    section_names = [
        "stages",
        "objectives",
        "stage_variables",
        "events",
        "player_inputs",
    ]
    assert runtime_layout["scalar_prefix_bytes"] == runtime_layout["stages"]["offset"]
    for current, following in zip(section_names, section_names[1:]):
        section = runtime_layout[current]
        assert (
            section["offset"] + section["count"] * section["record_bytes"]
            == runtime_layout[following]["offset"]
        )
    inputs = runtime_layout["player_inputs"]
    assert inputs["offset"] + inputs["count"] * inputs["record_bytes"] == len(runtime)
    assert runtime_layout["bytes"] == len(runtime)

    stages = runtime_layout["stages"]
    assert stages["record_bytes"] == 14
    for index in range(stages["count"]):
        offset = stages["offset"] + index * stages["record_bytes"]
        stage_type = runtime[offset]
        flags = runtime[offset + 1]
        domain = runtime[offset + 6]
        relation = u32(runtime, offset + 10)
        if stage_type in (1, 2, 4):
            assert domain == (2 if flags & 1 else 1)
        elif stage_type == 3:
            assert domain == (2 if flags & 1 else 0)
        elif stage_type == 6:
            assert domain == (3 if flags & 1 else 1)
        else:
            assert domain == 0
        if domain == 1:
            require(
                0 <= relation <= 1200,
                f"runtime stage {index} authored relation is out of range",
            )
        elif domain == 2:
            require(
                0 <= relation <= 1000,
                f"runtime stage {index} entity relation is out of range",
            )
        elif domain == 3:
            require(
                0 <= relation < 1000,
                f"runtime stage {index} special relation is out of range",
            )

    stage_vars = runtime_layout["stage_variables"]
    assert stage_vars["record_bytes"] == 12
    for index in range(stage_vars["count"]):
        offset = stage_vars["offset"] + index * stage_vars["record_bytes"]
        variable_type = runtime[offset]
        flags = runtime[offset + 1]
        domain = runtime[offset + 4]
        relation = u32(runtime, offset + 8)
        pointer_payload = (
            (3 <= variable_type <= 5 or 8 <= variable_type <= 9)
            and flags & 2 == 0
        )
        assert domain == (1 if pointer_payload else 0)
        if domain == 1:
            assert 0 <= relation <= 1000

    events = runtime_layout["events"]
    for index in range(events["count"]):
        offset = events["offset"] + index * events["record_bytes"]
        domain = runtime[offset + 10]
        relation = u32(runtime, offset + 14)
        assert domain == (1 if runtime[offset] != 0 else 0)
        if domain == 1:
            assert 0 <= relation <= 1000
        else:
            assert relation == 0

    presentation = (package / "presentation.bin").read_bytes()
    assert presentation[:4] == b"MCV1"
    assert u16(presentation, 4) == 1
    assert u16(presentation, 6) == 0
    assert u32(presentation, 8) == 347
    sprite_count_offset = 12 + 347 * 14
    assert u32(presentation, sprite_count_offset) == 504
    parameter_count_offset = sprite_count_offset + 4 + 504
    assert u32(presentation, parameter_count_offset) == 157
    assert len(presentation) == parameter_count_offset + 4 + 157 * 34

    terrain = (package / "terrain.bin").read_bytes()
    planes = manifest["reference"]["terrain_planes"]
    assert [plane["name"] for plane in planes] == [
        "type",
        "height",
        "shade",
        "angle",
        "secondary_cave",
        "entity_indices",
    ]
    non_cave_map = manifest["level"]["map_type"]["name"] != "Cave"
    historical_level_two_v3 = external_level == 2 and manifest["schema"] == SCHEMA_V3
    normalize_inactive_plane = non_cave_map and not historical_level_two_v3
    for plane in planes:
        start = plane["offset"]
        end = start + plane["bytes"]
        exported_hash = hashlib.sha256(terrain[start:end]).hexdigest()
        assert exported_hash == plane["exported_sha256"]
        if external_level == 1:
            assert plane["oracle_sha256"] == TERRAIN_ORACLE_HASHES[plane["name"]]
            if plane["name"] == "secondary_cave":
                assert plane["stock_regression_compared"] is False
                assert plane["source_status"] == "inactive_uninitialized_host_state"
                assert plane["normalization"] == "inactive_non_regression_compared_zeroed"
                assert exported_hash == hashlib.sha256(bytes(0x10000)).hexdigest()
                assert plane["match"] is False
            else:
                assert plane["stock_regression_compared"] is True
                assert plane["match"] is True
                assert exported_hash == plane["oracle_sha256"]
        else:
            require(
                exported_hash == plane["exported_sha256"],
                f"{plane['name']} content hash does not match manifest",
            )
            require(
                plane["oracle_sha256"] is None,
                f"Level 2 {plane['name']} oracle hash must be null",
            )
            require(
                plane["match"] is False,
                f"Level 2 {plane['name']} oracle match must be false",
            )
            if plane["name"] == "secondary_cave":
                require(
                    plane["stock_regression_compared"] is False,
                    "secondary cave plane must not claim stock comparison",
                )
                if normalize_inactive_plane:
                    require(
                        plane["source_status"] == "inactive_uninitialized_host_state",
                        "non-Cave secondary plane source status differs",
                    )
                    require(
                        plane["normalization"] == "inactive_non_regression_compared_zeroed",
                        "non-Cave secondary plane normalization differs",
                    )
                    require(
                        exported_hash == hashlib.sha256(bytes(0x10000)).hexdigest(),
                        "non-Cave secondary plane is not zero-normalized",
                    )
                    require(
                        terrain[start:end] == bytes(0x10000),
                        "non-Cave secondary plane is not zero-normalized",
                    )
                else:
                    require(
                        plane["source_status"] == "live_plane",
                        "Cave or historical v3 secondary plane source status differs",
                    )
                    require(
                        plane["normalization"] == "preserved",
                        "Cave or historical v3 secondary plane normalization differs",
                    )
            else:
                require(
                    plane["stock_regression_compared"] is True,
                    f"Level 2 {plane['name']} must claim stock comparison",
                )
    if normalize_inactive_plane:
        for name in ("terrain-pre-features.bin", "terrain-post-features.bin"):
            feature_terrain = (package / name).read_bytes()
            require(
                len(feature_terrain) == 0x70000,
                f"{name} terrain member has the wrong extent",
            )
            require(
                feature_terrain[0x40000:0x50000] == bytes(0x10000),
                "non-Cave secondary plane is not zero-normalized",
            )
    require(
        manifest["reference"]["terrain_full_match"] is (
            False if external_level == 1 else None
        ),
        "terrain full-match flag differs from level profile",
    )

    if external_level == 2:
        # The Level 2 package carries no Level 1 reset seal.  Older exploratory
        # captures did copy that object; it is deliberately ignored here so a
        # relabelled Level 1 state cannot pass the profile checks above.
        presentation = manifest["presentation_dependencies"]
        for key, expected in LEVEL2_PRESENTATION.items():
            require(
                presentation.get(key) == expected,
                f"Level 2 presentation field {key} differs from Day profile",
            )
        require(
            presentation.get("resolved_files", {}).get("language", "").startswith("owner:"),
            "Level 2 owner language asset is missing",
        )
        require(
            presentation["resolved_files"]["language"].endswith("/L1.TXT"),
            "Level 2 owner language asset is not English L1.TXT",
        )
        expected_volatile = (
            "player time_393 host-clock sample zeroed; secondary terrain plane preserved"
            if not normalize_inactive_plane
            else "inactive non-Cave secondary terrain plane zeroed; player time_393 host-clock sample zeroed"
        )
        require(
            manifest["normalization"]["volatile_host_state"] == [expected_volatile],
            "Level 2 host-state normalization policy differs",
        )
        feature_generation = manifest["feature_generation_oracle"]
        require(
            feature_generation["pre_terrain"]["inactive_secondary_plane"]
            == ("zero-normalized-non-cave" if normalize_inactive_plane else "preserved-live-plane"),
            "Level 2 pre-feature secondary terrain policy differs",
        )
        require(
            feature_generation["post_terrain"]["inactive_secondary_plane"]
            == ("zero-normalized-non-cave" if normalize_inactive_plane else "preserved-live-plane"),
            "Level 2 post-feature secondary terrain policy differs",
        )
        require(u16(runtime, 16) == 0, "Level 2 runtime local player slot must be 0")
        require(
            u16(runtime, 18) == LEVEL2_PROFILE["number_of_players"],
            "Level 2 runtime player count differs",
        )
        require(
            active_entity_count == LEVEL2_PROFILE["active_entities"],
            "Level 2 active entity count differs",
        )


def _exact_object(value: object, keys: set[str], label: str) -> dict:
    require(type(value) is dict, f"{label} is not an object")
    result = value
    require(set(result) == keys, f"{label} field set differs: {sorted(set(result) ^ keys)}")
    return result


def _bounded_integer(value: object, maximum: int, label: str) -> int:
    require(type(value) is int and 0 <= value <= maximum, f"{label} is out of bounds")
    return value


def _strict_equal(left: object, right: object) -> bool:
    if type(left) is not type(right):
        return False
    if isinstance(left, dict):
        return set(left) == set(right) and all(_strict_equal(left[key], right[key]) for key in left)
    if isinstance(left, list):
        return len(left) == len(right) and all(_strict_equal(a, b) for a, b in zip(left, right))
    return left == right


def _lower_hex(value: object, digits: int, label: str) -> str:
    require(
        isinstance(value, str)
        and len(value) == digits
        and all(character in "0123456789abcdef" for character in value),
        f"{label} is not {digits} lowercase hexadecimal digits",
    )
    return value


def _normalized_member_name(value: object, label: str) -> str:
    require(isinstance(value, str) and value, f"{label} is empty")
    require(value.isascii() and "/" not in value and "\\" not in value, f"{label} is not a flat normalized name")
    require(value not in {".", ".."} and "\0" not in value, f"{label} is reserved")
    return value


def _normalized_source_path(value: object, label: str) -> str:
    require(isinstance(value, str) and value and value.isascii(), f"{label} is not nonempty ASCII")
    require("\\" not in value and not value.startswith("/"), f"{label} is absolute or noncanonical")
    parts = value.split("/")
    require(all(part not in {"", ".", ".."} for part in parts), f"{label} traverses or is noncanonical")
    return value


def _canonical_json_bytes(value: object) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")


def _campaign_member_ref(value: object, files: dict, label: str, *, schema: str | None = None, count: int | None = None, record_bytes: int | None = None) -> dict:
    keys = {"member", "sha256"}
    if schema is not None:
        keys.add("schema")
    if count is not None:
        keys.add("count")
    if record_bytes is not None:
        keys.add("record_bytes")
    result = _exact_object(value, keys, label)
    member_name = _normalized_member_name(result["member"], f"{label}.member")
    require(member_name in files, f"{label} references an undeclared member")
    digest = _lower_hex(result["sha256"], 64, f"{label}.sha256")
    require(digest == files[member_name]["sha256"], f"{label} hash differs from outer file map")
    if schema is not None:
        require(result["schema"] == schema, f"{label} schema differs")
    if count is not None:
        require(_bounded_integer(result["count"], (1 << 64) - 1, f"{label}.count") == count, f"{label} count differs")
    if record_bytes is not None:
        require(_bounded_integer(result["record_bytes"], (1 << 64) - 1, f"{label}.record_bytes") == record_bytes, f"{label} record width differs")
    return result


def _campaign_decode_behavior_rows(
    authored: bytes,
    entities: bytes,
    expected_active_entities: int,
    expected_map_type_id: int = 0,
) -> list[dict]:
    require(
        len(authored) == 0x6604 and authored[6] == expected_map_type_id,
        "campaign authored member identity/layout changed",
    )
    rows: list[dict] = []
    for index in range(1200):
        offset = 1091 + index * 20
        record = authored[offset : offset + 20]
        if not any(record):
            continue
        class_id, subtype, _x, _y, action, _word10, _stage, model, _par2, _par3 = struct.unpack("<HhHHhHhHHH", record)
        require(
            class_id <= 255 and 0 <= subtype <= 255 and model <= 255 and -1 <= action <= 255,
            f"authored behavior row {index} exceeds schema-v1 width",
        )
        rows.append({
            "action": 255 if action == -1 else action,
            "authored_record": index,
            "class": class_id,
            "initial_dispatch_sites": [4] if action == -1 else [1, 2],
            "initial_source_slot": None,
            "model": model,
            "subtype": subtype,
        })

    require(len(entities) == 12 + 1000 * 176, "campaign entities extent changed")
    require(entities[:4] == b"MCE1" and u16(entities, 4) == 1 and u16(entities, 6) == 176 and u32(entities, 8) == 1000, "campaign entities header changed")
    active_count = 0
    for slot in range(1000):
        offset = 12 + slot * 176
        require(u16(entities, offset) == slot, f"entity slot/index mismatch at {slot}")
        active = entities[offset + 2]
        require(active in (0, 1), f"entity {slot} active flag is invalid")
        if not active:
            require(entities[offset + 67] == 0, f"inactive entity {slot} carries a live class")
            continue
        active_count += 1
        require(u16(entities, offset + 30) < 1200, f"entity {slot} source relation is out of bounds")
        require(u32(entities, offset + 4) <= 1000, f"entity {slot} next relation is out of bounds")
        require(u32(entities, offset + 164) <= 157, f"entity {slot} parameter relation is out of bounds")
        domain, relation = entities[offset + 168], u32(entities, offset + 172)
        require(
            (domain, relation) == (0, 0)
            or (domain == 1 and 1 <= relation <= 8)
            or (domain, relation) == (2, 1),
            f"entity {slot} special relation is invalid",
        )
        rows.append({
            "action": entities[offset + 73],
            "authored_record": u16(entities, offset + 30),
            "class": entities[offset + 67],
            "initial_dispatch_sites": [1, 2, 3],
            "initial_source_slot": slot,
            "model": entities[offset + 68],
            "subtype": entities[offset + 70],
        })
    require(active_count == expected_active_entities, "campaign active entity count differs from the census profile")
    rows.sort(key=lambda row: (row["authored_record"], row["initial_source_slot"] is not None, row["initial_source_slot"] or 0))
    return rows


def _campaign_selector(key: tuple[int, int, int, int]) -> dict:
    _site, kind, first, second = key
    if kind == 1:
        return {"action": second, "class": first, "kind": "ClassAction"}
    if kind == 2:
        return {"class": first, "kind": "ClassModel", "model": second}
    require(kind == 3, "unknown campaign behavior selector kind")
    return {"kind": "TypeSubtype", "subtype": second, "type_id": first}


def _campaign_behavior_key(site: int, row: dict) -> tuple[int, int, int, int]:
    if site == 3:
        return (site, 2, row["class"], row["model"])
    if site == 4:
        return (site, 3, row["class"], row["subtype"])
    require(site in (1, 2), "unknown campaign dispatch site")
    return (site, 1, row["class"], row["action"])


def _campaign_required_behaviors(rows: list[dict]) -> tuple[list[dict], set[tuple[int, int, int, int]]]:
    provenance: dict[tuple[int, int, int, int], dict] = {}
    for row in rows:
        # GOLDEN-CHANGE (operator-approved, 2026-08-23): campaign package v1
        # contains a post-GenerateEvents reset snapshot.  Only live reset rows
        # are runtime roots; dormant authored rows remain an inventory until a
        # later descriptor graph makes them reachable.
        if row["initial_source_slot"] is None:
            continue
        source = {
            "authored_record": row["authored_record"],
            "initial_source_slot": row["initial_source_slot"],
        }
        key = (2, 1, row["class"], row["action"])
        provenance.setdefault(key, source)
    requirements = []
    for key in sorted(provenance):
        requirements.append({
            "dispatch_site": key[0],
            "provenance": [provenance[key]],
            "selector": _campaign_selector(key),
        })
    return requirements, set(provenance)


def _campaign_validate_reset_members(package: Path, spec: dict, files: dict, profile: dict) -> tuple[bytes, bytes, bytes]:
    authored_descriptor = _campaign_member_ref(
        spec["authored_records"], files, "campaign_spec.authored_records",
        schema="mc2-authored-level/v1", count=1200, record_bytes=20,
    )
    authored = (package / authored_descriptor["member"]).read_bytes()
    entities_descriptor = _campaign_member_ref(
        spec["reset_sources"]["entities"], files, "campaign_spec.reset_sources.entities",
        schema="mc2-entity-arena/v1", count=1000, record_bytes=176,
    )
    entities = (package / entities_descriptor["member"]).read_bytes()
    expected_rows = _campaign_decode_behavior_rows(
        authored,
        entities,
        profile["active_entities"],
        profile["map_type_id"],
    )
    require(_strict_equal(spec["authored_behavior_rows"], expected_rows), "authored_behavior_rows differ from fresh authored/reset decode")

    players_descriptor = _campaign_member_ref(
        spec["reset_sources"]["players"], files, "campaign_spec.reset_sources.players",
        schema=profile["player_schema"],
        count=8,
        record_bytes=profile["player_record_bytes"],
    )
    players = (package / players_descriptor["member"]).read_bytes()
    player_record_bytes = profile["player_record_bytes"]
    require(
        len(players) == 12 + 8 * player_record_bytes and players[:4] == b"MCP1",
        "campaign players extent/magic changed",
    )
    require(
        (u16(players, 4), u16(players, 6), u32(players, 8))
        == (profile["player_component_version"], player_record_bytes, 8),
        "campaign players header changed",
    )
    for slot in range(8):
        offset = 12 + slot * player_record_bytes
        require(u16(players, offset) == slot, f"player slot/index mismatch at {slot}")
        require(players[offset + 2] in (0, 1) and players[offset + 3] == 0, f"player {slot} flags are invalid")
        require(u16(players, offset + 4) < 1000, f"player {slot} entity relation is out of bounds")
        if slot < profile["bound_player_slots"]:
            entity = u16(players, offset + 4)
            require(
                entities[12 + entity * 176 + 2] == 1,
                f"campaign active player {slot} does not reference an active entity",
            )
        require(players[offset + 50 : offset + 54] == bytes(4), f"player {slot} host time is not normalized")

    terrain_descriptor = _campaign_member_ref(
        spec["reset_sources"]["terrain"], files, "campaign_spec.reset_sources.terrain",
        schema="mc2-terrain/v1",
    )
    terrain = (package / terrain_descriptor["member"]).read_bytes()
    require(
        len(terrain) == 0x70000
        and terrain[0x40000:0x50000] == bytes(0x10000),
        "campaign terrain extent/non-Cave normalization changed",
    )

    runtime_descriptor = _campaign_member_ref(
        spec["reset_sources"]["runtime"], files, "campaign_spec.reset_sources.runtime",
        schema="mc2-runtime-state/v1",
    )
    runtime = (package / runtime_descriptor["member"]).read_bytes()
    require(len(runtime) == 2754 and runtime[:4] == b"MCR1" and (u16(runtime, 4), u16(runtime, 6)) == (1, 0), "campaign runtime header/extent changed")
    require(
        (u16(runtime, 16), u16(runtime, 18), u32(runtime, 32))
        == (0, profile["runtime_player_count"], profile["runtime_music_selector"]),
        "campaign runtime census profile scalars changed",
    )
    accepted_stage_types = {0, 3, 5, 7, 9}
    for index in range(8):
        offset = 192 + index * 14
        stage_type, flags = runtime[offset], runtime[offset + 1]
        require(stage_type in accepted_stage_types, f"runtime stage {index} has an unproven type")
        require(runtime[offset + 7 : offset + 10] == bytes(3), f"runtime stage {index} relation padding is nonzero")
        domain, relation = runtime[offset + 6], u32(runtime, offset + 10)
        expected_domain = 2 if stage_type == 3 and flags & 1 else 0
        require(domain == expected_domain, f"runtime stage {index} relation domain differs")
        require(
            domain == 0 or 1 <= relation <= 1000,
            f"runtime stage {index} relation is out of bounds",
        )
    for index in range(8):
        require(runtime[304 + index * 11] == 0, f"runtime objective {index} has an unproven type")
    for index in range(11):
        offset = 392 + index * 12
        variable_type, flags = runtime[offset], runtime[offset + 1]
        pointer_payload = (3 <= variable_type <= 5 or 8 <= variable_type <= 9) and flags & 2 == 0
        expected_domain = 1 if pointer_payload else 0
        require(runtime[offset + 5 : offset + 8] == bytes(3), f"runtime stage variable {index} relation padding is nonzero")
        domain, relation = runtime[offset + 4], u32(runtime, offset + 8)
        require(domain == expected_domain, f"runtime stage variable {index} relation domain differs")
        require(
            domain == 0 or 1 <= relation <= 1000,
            f"runtime stage variable {index} relation is out of bounds",
        )
    for index in range(50):
        offset = 524 + index * 43
        require(runtime[offset + 11 : offset + 14] == bytes(3), f"runtime event {index} relation padding is nonzero")
        domain, relation = runtime[offset + 10], u32(runtime, offset + 14)
        require(domain == (1 if runtime[offset] else 0), f"runtime event {index} relation domain differs")
        require((domain == 1 and 1 <= relation <= 1000) or (domain == 0 and relation == 0), f"runtime event {index} relation is out of bounds")

    expected_boundary = [
        ("feature-start.bin", "mc2-feature-boundary/v1"),
        ("terrain-pre-features.bin", "mc2-feature-terrain/v1"),
        ("feature-allocator.bin", "mc2-feature-allocator/v1"),
        ("feature-construction-inputs.bin", "mc2-feature-construction-inputs/v1"),
        ("entities-post-features.bin", "mc2-feature-entity-arena/v1"),
        ("terrain-post-features.bin", "mc2-feature-terrain/v1"),
        ("feature-end.bin", "mc2-feature-boundary/v1"),
    ]
    boundary = spec["reset_sources"]["feature_boundary"]
    require(isinstance(boundary, list) and len(boundary) == len(expected_boundary), "feature_boundary sequence differs")
    for index, (member_name, schema) in enumerate(expected_boundary):
        descriptor = _campaign_member_ref(boundary[index], files, f"feature_boundary[{index}]", schema=schema)
        require(descriptor["member"] == member_name, f"feature_boundary[{index}] member order differs")
    for name in ("feature-start.bin", "feature-end.bin"):
        content = (package / name).read_bytes()
        magic = b"MFS3" if name == "feature-start.bin" else b"MFE3"
        require(len(content) == 16 and content[:4] == magic and (u16(content, 4), u16(content, 6), u16(content, 10)) == (1, 16, 0), f"{name} layout changed")
    for name in ("terrain-pre-features.bin", "terrain-post-features.bin"):
        content = (package / name).read_bytes()
        require(
            len(content) == 0x70000
            and content[0x40000:0x50000] == bytes(0x10000),
            f"{name} layout/non-Cave normalization changed",
        )
    post_entities = (package / "entities-post-features.bin").read_bytes()
    require(len(post_entities) == 12 + 1000 * 176 and post_entities[:4] == b"MCE3" and (u16(post_entities, 4), u16(post_entities, 6), u32(post_entities, 8)) == (1, 176, 1000), "post-feature entity arena changed")
    allocator = (package / "feature-allocator.bin").read_bytes()
    require(
        len(allocator) >= 32
        and allocator[:4] == b"MCA3"
        and (u16(allocator, 4), u16(allocator, 6)) == (1, 32),
        "feature allocator layout changed",
    )
    trace_count = u32(allocator, 24)
    require(
        u32(allocator, 20) == 0
        and (u16(allocator, 28), u16(allocator, 30)) == (2, 8)
        and len(allocator) == 32 + 1000 * 4 + trace_count * 8,
        "feature allocator reserved/trace layout changed",
    )
    return authored, entities, runtime


def _campaign_decode_feature_inputs(content: bytes) -> list[int]:
    require(len(content) >= 16 and content[:4] == b"MCI3", "feature construction input header is truncated")
    require((u16(content, 4), u16(content, 6), u16(content, 10), u32(content, 12)) == (1, 16, 1, 0), "feature construction input header changed")
    count = u16(content, 8)
    selectors: list[int] = []
    cursor = 16
    for _ in range(count):
        require(cursor + 12 <= len(content), "feature construction input record is truncated")
        selector, width, height, extent = u16(content, cursor), content[cursor + 6], content[cursor + 7], u32(content, cursor + 8)
        require(not selectors or selector > selectors[-1], "feature construction selectors are not strictly ordered")
        require(width > 0 and height > 0 and extent == width * height * 2, "feature construction footprint extent changed")
        cursor += 12 + extent
        require(cursor <= len(content), "feature construction footprint is truncated")
        selectors.append(selector)
    require(cursor == len(content), "feature construction input member has trailing bytes")
    return selectors


def _campaign_decode_collision(content: bytes, authored_selectors: list[int]) -> list[int]:
    require(len(content) >= 20 and content[:4] == b"MCM1", "collision fact header is truncated")
    require((u16(content, 4), u16(content, 6), u16(content, 8), u16(content, 10), u32(content, 16)) == (1, 20, 1, 0, 12), "collision fact header changed")
    count, cursor, selectors = u32(content, 12), 20, []
    for _ in range(count):
        require(cursor + 12 <= len(content), "collision fact record is truncated")
        selector, width, height = u16(content, cursor), u16(content, cursor + 2), u16(content, cursor + 4)
        extent = u32(content, cursor + 8)
        require(u16(content, cursor + 6) == 0 and width > 0 and height > 0 and extent == width * height * 2, "collision fact record extent changed")
        require(not selectors or selector > selectors[-1], "collision fact selectors are not strictly ordered")
        cursor += 12 + extent
        require(cursor <= len(content), "collision fact payload is truncated")
        selectors.append(selector)
    require(cursor == len(content), "collision fact member has trailing bytes")
    expected = sorted(set(authored_selectors))
    require(selectors == expected, "collision fact selectors differ from authored class-10/subtype-45 rows")
    require((not expected and count == 0 and len(content) == 20) or (expected and count > 0), "collision fact empty/nonempty invariant differs")
    return selectors


def _campaign_decode_building_parameters(content: bytes) -> list[tuple[int, int, int]]:
    require(len(content) == 76 * 4, "building parameter owner extent changed")
    rows = [
        (u16(content, offset), content[offset + 2], content[offset + 3])
        for offset in range(0, len(content), 4)
    ]
    require(rows[21] == (6, 8, 54) and rows[54] == (8, 8, 0), "qualified building objective chain changed")
    selector, chain, seen = 21, [], set()
    while selector:
        require(len(chain) < 8 and selector not in seen and selector < len(rows), "building objective chain cycles or exceeds bounds")
        seen.add(selector)
        chain.append(selector)
        selector = rows[selector][2]
    require(chain == [21, 54], "qualified standing-building chain differs")
    return rows


def _campaign_validate_source_facts(package: Path, facts_value: object, files: dict, authored: bytes, behavior_rows: list[dict], profile: dict) -> None:
    require(isinstance(facts_value, list), "source_facts is not an array")
    facts = facts_value
    source_fact_ids = profile["source_fact_ids"]
    require(
        len(facts) == len(source_fact_ids),
        "campaign SourceFact count differs from the census profile",
    )
    authored_provenance = [
        {
            "authored_record": row["authored_record"],
            "selector_value": row["model"],
            "source_class": 10,
            "source_field": "par1_14",
            "source_model": row["model"],
            "source_subtype": 45,
        }
        for row in behavior_rows
        if row["initial_source_slot"] is None and row["class"] == 10 and row["subtype"] == 45 and row["action"] == 255
    ]
    stage, standing_selector, _axis_x, _axis_y = struct.unpack_from(
        "<bhhh", authored, 25972 + 4 * 7
    )
    if "building-parameters" in source_fact_ids:
        require(
            stage == 9 and standing_selector == 153,
            "standing-building objective operation differs from qualified Level 2",
        )
    else:
        standing_selector = None
    building_objective_provenance = [
        {
            "authored_record": row["authored_record"],
            "selector_value": row["model"],
            "source_class": row["class"],
            "source_field": "par1_14",
            "source_model": row["model"],
            "source_subtype": row["subtype"],
        }
        for row in behavior_rows
        if row["authored_record"] == standing_selector
        and row["initial_source_slot"] is None
        and row["class"] == 10
        and row["subtype"] == 45
    ]
    if "building-parameters" in source_fact_ids:
        require(
            building_objective_provenance == [{
                "authored_record": 153,
                "selector_value": 21,
                "source_class": 10,
                "source_field": "par1_14",
                "source_model": 21,
                "source_subtype": 45,
            }],
            "standing-building objective provenance differs from qualified Level 2",
        )
    all_expected = {
        "building-parameters": (4, "building-parameters.bin", building_objective_provenance, {"CD_Files/DATA/BLDGPRM.DAT"}),
        "collision-mask-bank": (3, "collision-masks.bin", authored_provenance, {"CD_Files/DATA/MSPRD0-0.DAT", "CD_Files/DATA/MSPRD0-0.TAB"}),
        "feature-construction-inputs": (2, "feature-construction-inputs.bin", authored_provenance, {"CD_Files/DATA/BUILD0-0.DAT", "CD_Files/DATA/BUILD0-0.TAB", "CD_Files/DATA/SPELLS.DAT", "CD_Files/DATA/TABLESN.DAT"}),
        "level-records": (1, "authored-level.bin", [], {"GAME/NETHERW/CLEVELS/LEVELS.DAT", "GAME/NETHERW/CLEVELS/LEVELS.TAB"}),
    }
    expected = {fact_id: all_expected[fact_id] for fact_id in source_fact_ids}
    fact_ids = [fact.get("fact_id") for fact in facts if isinstance(fact, dict)]
    require(
        fact_ids == sorted(expected),
        "source_facts are not the exact sorted census-profiled fact-id set",
    )
    member_claims: dict[str, str] = {}
    for fact_index, raw_fact in enumerate(facts):
        fact = _exact_object(raw_fact, {"fact_id", "kind", "version", "member", "member_sha256", "selector_provenance", "source_artifacts", "lifetime"}, f"source_facts[{fact_index}]")
        fact_id = fact["fact_id"]
        require(isinstance(fact_id, str) and fact_id and fact_id.isascii() and all(character.isalnum() or character in "-_" for character in fact_id), f"source fact {fact_index} id is invalid")
        require(fact_id in expected, f"unknown SourceFact id {fact_id}")
        kind, member_name, provenance, artifact_paths = expected[fact_id]
        fact_kind = _bounded_integer(fact["kind"], 0xFFFF, f"SourceFact {fact_id} kind")
        fact_version = _bounded_integer(fact["version"], 0xFFFF, f"SourceFact {fact_id} version")
        require(_strict_equal((fact_kind, fact_version, fact["lifetime"], fact["member"]), (kind, 1, "immutable-through-session", member_name)), f"SourceFact {fact_id} kind/version/lifetime/member differs")
        digest = _lower_hex(fact["member_sha256"], 64, f"SourceFact {fact_id} member hash")
        require(digest == files[member_name]["sha256"], f"SourceFact {fact_id} member hash differs from outer map")
        require(member_claims.setdefault(member_name, digest) == digest, f"SourceFact {fact_id} conflicts with another member claim")
        require(_strict_equal(fact["selector_provenance"], provenance), f"SourceFact {fact_id} selector provenance differs from authored decode")
        artifacts = fact["source_artifacts"]
        require(isinstance(artifacts, list), f"SourceFact {fact_id} artifacts are not an array")
        normalized_artifact_paths = []
        for artifact_index, raw_artifact in enumerate(artifacts):
            artifact = _exact_object(raw_artifact, {"logical_path", "bytes", "sha256"}, f"SourceFact {fact_id} artifact {artifact_index}")
            path = _normalized_source_path(artifact["logical_path"], f"SourceFact {fact_id} artifact path")
            normalized_artifact_paths.append(path)
            require(path in CAMPAIGN_SOURCE_ARTIFACTS, f"SourceFact {fact_id} has an unproven source artifact")
            expected_bytes, expected_hash = CAMPAIGN_SOURCE_ARTIFACTS[path]
            artifact_bytes = _bounded_integer(artifact["bytes"], (1 << 64) - 1, f"SourceFact {fact_id} artifact bytes")
            artifact_hash = _lower_hex(artifact["sha256"], 64, f"SourceFact {fact_id} artifact hash")
            require(_strict_equal((artifact_bytes, artifact_hash), (expected_bytes, expected_hash)), f"SourceFact {fact_id} artifact identity changed")
        require(normalized_artifact_paths == sorted(artifact_paths), f"SourceFact {fact_id} source artifacts are not exact/sorted")

    feature_selectors = _campaign_decode_feature_inputs((package / "feature-construction-inputs.bin").read_bytes())
    authored_selectors = [row["model"] for row in behavior_rows if row["initial_source_slot"] is None and row["class"] == 10 and row["subtype"] == 45 and row["action"] == 255]
    require(feature_selectors == sorted(set(authored_selectors)), "feature construction selectors differ from authored provenance")
    if "collision-mask-bank" in source_fact_ids:
        collision = (package / "collision-masks.bin").read_bytes()
        _campaign_decode_collision(collision, authored_selectors)
        require(
            len(collision) == profile["collision_bank_bytes"]
            and hashlib.sha256(collision).hexdigest()
            == profile["collision_bank_sha256"],
            "collision SourceFact differs from the census-qualified bytes",
        )
    if "building-parameters" in source_fact_ids:
        building_parameters = (package / "building-parameters.bin").read_bytes()
        require(
            hashlib.sha256(building_parameters).hexdigest()
            == "1c4a7fdef5d4b811f871441f0380faaacd6e5e689e73fc4628bf8c5b51ea6f00",
            "building parameter SourceFact differs from owner bytes",
        )
        _campaign_decode_building_parameters(building_parameters)


def _campaign_validate_operations(spec: dict, authored: bytes, files: dict, package: Path, profile: dict) -> list[dict]:
    require(spec["initial_activation"] == [], "schema-v1 has no proven initial-activation operation")
    expected_objectives = []
    inactive_seen = False
    for index in range(8):
        offset = 25972 + index * 7
        stage, selector, axis_x, axis_y = struct.unpack_from("<bhhh", authored, offset)
        if stage == -1:
            inactive_seen = True
            require((selector, axis_x, axis_y) == (0, 0, 0), f"inactive objective row {index} has payload")
            continue
        require(not inactive_seen and stage in profile["objective_stage_forms"], f"objective row {index} has an unproven operation form")
        expected_objectives.append({
            "operands": {"axis_x": axis_x, "axis_y": axis_y, "objective_selector": selector, "objective_slot": index, "stage_index": stage},
            "operation_kind": 1,
            "operation_version": 1,
        })
    require(len(expected_objectives) == profile["objective_count"] and _strict_equal(spec["objectives"], expected_objectives), "objective operations differ from the authored census-profiled rows")
    for index, operation in enumerate(spec["objectives"]):
        _exact_object(operation, {"operation_kind", "operation_version", "operands"}, f"objectives[{index}]")
        _exact_object(operation["operands"], {"axis_x", "axis_y", "objective_selector", "objective_slot", "stage_index"}, f"objectives[{index}].operands")

    rng = _exact_object(spec["initial_rng"], {"global", "terrain"}, "campaign_spec.initial_rng")
    for name, encoding, offset in (("global", "u32_le", 12), ("terrain", "u16_le", 8)):
        field = _exact_object(rng[name], {"encoding", "member", "offset", "sha256"}, f"initial_rng.{name}")
        field_offset = _bounded_integer(field["offset"], (1 << 64) - 1, f"initial_rng.{name}.offset")
        require(_strict_equal((field["encoding"], field["member"], field_offset, field["sha256"]), (encoding, "feature-end.bin", offset, files["feature-end.bin"]["sha256"])), f"initial_rng.{name} source field differs")
    feature_end = (package / "feature-end.bin").read_bytes()
    require(len(feature_end) == 16, "initial RNG member extent changed")
    return expected_objectives


def _campaign_validate_presentation(
    recipe_value: object,
    files: dict,
    *,
    owner_manifest: dict | None = None,
) -> None:
    keys = {"block_bank", "cursor", "map_type", "member", "palette", "resolved_files", "sky_bank", "sound_bank", "sprite_bank", "terrain_bank", "terrain_graphics_flag_2", "terrain_graphics_flags", "texture_size", "vga_resolution_selector", "web_bank"}
    recipe = _exact_object(recipe_value, keys, "campaign_spec.presentation_recipe")
    _campaign_member_ref(recipe["member"], files, "presentation_recipe.member", schema="mc2-presentation-snapshot/v1")
    if owner_manifest is not None:
        expected = owner_manifest["presentation_dependencies"]
        actual = {name: value for name, value in recipe.items() if name != "member"}
        require(
            _strict_equal(actual, expected),
            "presentation recipe differs from the bound owner manifest",
        )
        return
    expected = {
        "block_bank": "BL32N0-0", "cursor": 9, "map_type": "Day", "palette": "PALD-0",
        "sky_bank": "SKYD0-0", "sound_bank": 1, "sprite_bank": "MSPRD0-0",
        "terrain_bank": "TMAPS0-0", "texture_size": 32, "vga_resolution_selector": 1,
        "web_bank": "MWEBD0-0",
    }
    for name, value in expected.items():
        require(_strict_equal(recipe[name], value), f"presentation recipe {name} differs from Level 2")
    require(
        type(recipe["terrain_graphics_flags"]) is int
        and type(recipe["terrain_graphics_flag_2"]) is bool,
        "presentation terrain flags are malformed",
    )
    require(
        _strict_equal(
            (recipe["terrain_graphics_flags"], recipe["terrain_graphics_flag_2"]),
            (0, False),
        ),
        "presentation terrain flags differ from proven Level 2",
    )
    resolved = _exact_object(recipe["resolved_files"], {"level_records", "terrain_bank", "sprite_bank", "web_bank", "block_bank", "palette", "sky_bank", "building_bank", "spell_table", "font_bank", "language"}, "presentation_recipe.resolved_files")
    expected_resolved = {
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
    require(resolved == expected_resolved, "presentation recipe resolved asset identities differ")


def _campaign_package_identity(files: dict) -> str:
    identity = bytearray()
    for name in sorted(files):
        metadata = files[name]
        identity.extend(
            name.encode("ascii")
            + b"\0"
            + metadata["sha256"].encode("ascii")
            + b"\0"
            + str(metadata["bytes"]).encode("ascii")
            + b"\n"
        )
    return hashlib.sha256(identity).hexdigest()


def _campaign_validate_owner_manifest(
    package: Path,
    outer_manifest: dict,
    files: dict,
    outer_selection: tuple[int, int, int],
    profile: dict,
) -> dict:
    owner_manifest_bytes = (package / "owner-manifest.json").read_bytes()
    try:
        owner_manifest = json.loads(owner_manifest_bytes.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise AssertionError(f"owner manifest is not UTF-8 JSON: {error}") from error

    require(
        owner_manifest.get("schema") == profile["owner_manifest_schema"],
        "owner manifest schema differs from the campaign profile",
    )
    require(
        _strict_equal(owner_manifest.get("level"), outer_manifest["level"]),
        "owner manifest selection differs from the campaign envelope",
    )
    owner_files = _exact_object(
        owner_manifest.get("files"),
        V3_FILES,
        "owner manifest.files",
    )
    require(
        not (set(owner_files) & CAMPAIGN_ENVELOPE_FILES),
        "owner export identity includes campaign envelope members",
    )
    require(
        _strict_equal(owner_files, {name: files[name] for name in V3_FILES}),
        "owner manifest files differ from the outer campaign member map",
    )
    owner_identity = _campaign_package_identity(owner_files)
    require(
        owner_manifest.get("package_sha256") == owner_identity,
        "owner manifest package_sha256 includes unbound or envelope members",
    )
    require(
        outer_manifest["owner_export_package_sha256"] == owner_identity,
        "campaign owner_export_package_sha256 differs from the bound owner export",
    )

    owner_selection = (
        owner_manifest["level"]["external"],
        owner_manifest["level"]["internal"],
        owner_manifest["level"]["map_type"]["id"],
    )
    require(owner_selection == outer_selection, "owner manifest selection is cross-bound")
    require(owner_manifest["byte_order"] == "little-endian", "owner manifest byte order differs")
    require(owner_manifest["capture"] == {"frame": 0, "site": "0x002285ff"}, "owner manifest capture differs")
    require(owner_manifest["counts"]["active_entities"] == profile["active_entities"], "owner manifest active entity census differs")
    require(owner_manifest["record_sizes"]["player"] == profile["player_record_bytes"], "owner manifest player record width differs")
    require(owner_manifest["record_layouts"]["player"]["record_bytes"] == profile["player_record_bytes"], "owner manifest player layout width differs")
    require("collision_masks" not in owner_manifest, "Level 1 owner manifest carries a collision bank")
    require(
        owner_manifest["source"]["upstream_commit"] == REM_C2_COMMIT,
        "Level 1 owner source commit differs from pinned remc2",
    )
    _lower_hex(
        owner_manifest["source"]["upstream_worktree_diff_sha256"],
        64,
        "Level 1 owner worktree-diff identity",
    )
    _lower_hex(
        owner_manifest["source"]["compiled_source_sha256"],
        64,
        "Level 1 owner compiled-source identity",
    )
    require(str(package) not in owner_manifest_bytes.decode("utf-8"), "owner manifest embeds its output path")
    require("/home/" not in owner_manifest_bytes.decode("utf-8"), "owner manifest embeds a home path")
    require("\\\\" not in owner_manifest_bytes.decode("utf-8"), "owner manifest contains noncanonical separators")
    require("missing:" not in owner_manifest_bytes.decode("utf-8"), "owner manifest contains unresolved owner inputs")

    verify_feature_generation_input(package, owner_manifest)
    verify_binary_structure(package, owner_manifest)
    return owner_manifest


def _campaign_validate_presentation_assets(
    package: Path,
    raw_assets: object,
    files: dict,
) -> None:
    require(isinstance(raw_assets, list) and len(raw_assets) == 2, "campaign presentation asset set differs")
    observed: set[str] = set()
    for ordinal, raw in enumerate(raw_assets):
        row = _exact_object(raw, {"artifact_id", "member", "slot_id", "source"}, f"presentation asset {ordinal}")
        artifact_id = row["artifact_id"]
        require(isinstance(artifact_id, str) and artifact_id in LEVEL1_PRESENTATION_ASSETS, "unknown presentation artifact ID")
        require(artifact_id not in observed, "duplicate presentation artifact ID")
        observed.add(artifact_id)
        expected = LEVEL1_PRESENTATION_ASSETS[artifact_id]
        require(row["slot_id"] == "presentation.spell.fireball.projectile_asset", "presentation slot differs")
        member = _exact_object(row["member"], {"member", "schema", "sha256"}, f"presentation asset {artifact_id}.member")
        require(
            member == {
                "member": expected["member"],
                "schema": "carpet-flight-indexed-sprite/v1",
                "sha256": expected["sha256"],
            },
            "presentation member binding differs",
        )
        require(
            files[expected["member"]]["sha256"] == expected["sha256"],
            "presentation member hash is not profile-qualified",
        )
        source = _exact_object(
            row["source"],
            {"decoded_sha256", "frame_index", "indices_sha256", "owner_artifacts", "palette_rgb8_sha256", "selector", "transform"},
            f"presentation asset {artifact_id}.source",
        )
        require(source["decoded_sha256"] == "cf185f7e35f9103be4b4ecfa2573fc9332ac8ade346b564be3971cbe73fb35ec", "presentation decoded source differs")
        require(source["frame_index"] == 1 and source["selector"] == 463, "presentation source selector/frame differs")
        require(source["indices_sha256"] == "f6755ec0f9931b6748c32893cc11838fb398043dafdc962eb5e288105cf0c88d", "presentation base indices differ")
        require(source["palette_rgb8_sha256"] == expected["palette_rgb8_sha256"], "presentation RGB8 palette identity differs")
        require(source["transform"] == "base_frame_indices_plus_rgb6_times_4_palette_index_zero_transparent", "presentation transform differs")
        require(
            source["owner_artifacts"] == [
                {"bytes": 1354118, "logical_path": "CD_Files/DATA/TMAPS1-0.DAT", "sha256": "b091204ec21c0cf99716f3ae5c426a37113e57dda48a0d4d2eef98167e6ed0ff"},
                {"bytes": 5050, "logical_path": "CD_Files/DATA/TMAPS1-0.TAB", "sha256": "22afd08167ae415138548f99901ea037bdc2b48f56c6f3b2dbcfe9743d73c8f0"},
                {"bytes": 768, "logical_path": expected["palette_path"], "sha256": expected["palette_sha256"]},
            ],
            "presentation owner-source provenance differs",
        )

        payload_bytes = (package / expected["member"]).read_bytes()
        try:
            payload = json.loads(payload_bytes.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise AssertionError(f"presentation payload is not UTF-8 JSON: {error}") from error
        require(payload_bytes == _canonical_json_bytes(payload), "presentation payload is not canonical JSON")
        asset = _exact_object(
            payload,
            {"budgets", "color_contract", "coordinates", "dependencies", "frame_count", "frame_duration_ms", "height", "indices", "kind", "origin", "palette_rgba8", "runtime_format", "schema", "width"},
            f"presentation payload {artifact_id}",
        )
        require(asset["schema"] == "carpet-flight-indexed-sprite/v1" and asset["kind"] == "indexed_sprite", "presentation payload schema/kind differs")
        require(asset["runtime_format"] == "indexed8-rgba8/v1" and asset["color_contract"] == "srgb-straight-alpha-rgba8", "presentation runtime/color contract differs")
        require(asset["origin"] == "center" and asset["coordinates"] == "x_right_y_down", "presentation geometry contract differs")
        require((asset["width"], asset["height"], asset["frame_count"], asset["frame_duration_ms"]) == (34, 35, 1, 0), "presentation extent/timing differs")
        require(asset["dependencies"] == [], "presentation payload dependencies differ")
        require(asset["budgets"] == {"max_bytes": 32768, "max_frames": 1, "max_palette_colors": 256, "max_pixels": 1190}, "presentation budgets differ")
        indices = bytes(asset["indices"])
        require(len(indices) == 1190 and hashlib.sha256(indices).hexdigest() == source["indices_sha256"], "presentation indices differ from qualified base frame")
        palette = asset["palette_rgba8"]
        require(isinstance(palette, list) and len(palette) == 1024 and all(isinstance(value, int) and 0 <= value <= 255 for value in palette), "presentation RGBA palette is invalid")
        require(palette[3] == 0 and all(palette[index] == 255 for index in range(7, 1024, 4)), "presentation palette alpha contract differs")
        rgb8 = bytes(value for index, value in enumerate(palette) if index % 4 != 3)
        require(hashlib.sha256(rgb8).hexdigest() == expected["palette_rgb8_sha256"], "presentation RGB8 palette differs")
    require(observed == set(LEVEL1_PRESENTATION_ASSETS), "presentation artifact set is incomplete")


def verify_campaign_package(
    package: Path,
    manifest: dict,
    *,
    expected_selection: tuple[int, int, int] | None = None,
    campaign_capabilities: set[tuple[int, int, int, int]] | None = None,
) -> dict:
    # Stage 1: closed outer shape and normalized identities.
    level_one_envelope = manifest.get("level") == {
        "external": 1,
        "internal": 0,
        "map_type": {"id": 1, "name": "Night"},
    }
    manifest_keys = {"schema", "byte_order", "level", "files", "package_sha256"}
    if level_one_envelope:
        manifest_keys.add("owner_export_package_sha256")
    _exact_object(manifest, manifest_keys, "campaign manifest")
    require(manifest["byte_order"] == "little-endian", "campaign outer byte order differs")
    level = _exact_object(manifest["level"], {"external", "internal", "map_type"}, "campaign manifest.level")
    map_type = _exact_object(level["map_type"], {"id", "name"}, "campaign manifest.level.map_type")
    outer_selection = (
        _bounded_integer(level["external"], 0xFFFF, "external level"),
        _bounded_integer(level["internal"], 0xFFFF, "internal level"),
        _bounded_integer(map_type["id"], 0xFF, "map type id"),
    )
    profile = CAMPAIGN_PROFILES.get(outer_selection)
    require(profile is not None, "campaign outer selection has no census-proven profile")
    require(manifest["schema"] == profile["package_schema"], "campaign outer schema differs")
    require(
        (expected_selection is None or outer_selection == expected_selection)
        and map_type["name"] == profile["map_type_name"],
        "campaign caller/outer selection mismatch",
    )
    files = _exact_object(manifest["files"], profile["files"], "campaign manifest.files")
    for name, raw_metadata in files.items():
        _normalized_member_name(name, "campaign member name")
        metadata = _exact_object(raw_metadata, {"bytes", "sha256"}, f"campaign file {name}")
        _bounded_integer(metadata["bytes"], (1 << 64) - 1, f"campaign file {name} size")
        _lower_hex(metadata["sha256"], 64, f"campaign file {name} hash")
    _lower_hex(manifest["package_sha256"], 64, "campaign package hash")

    # Stage 2: exact regular directory set (symlinks and nested entries fail).
    actual_entries = {entry.name: entry for entry in package.iterdir()}
    require(
        set(actual_entries) == profile["files"] | {"manifest.json"},
        "campaign package directory member set differs",
    )
    for name, entry in actual_entries.items():
        require(entry.is_file() and not entry.is_symlink(), f"campaign package member {name} is not a regular file")

    # Stage 3: member and aggregate hashes.
    for name in sorted(files):
        content = (package / name).read_bytes()
        digest = hashlib.sha256(content).hexdigest()
        require((len(content), digest) == (files[name]["bytes"], files[name]["sha256"]), f"campaign member {name} size/hash mismatch")
    require(_campaign_package_identity(files) == manifest["package_sha256"], "campaign package_sha256 mismatch")

    owner_manifest = None
    if profile["owner_manifest_schema"] is not None:
        owner_manifest = _campaign_validate_owner_manifest(
            package,
            manifest,
            files,
            outer_selection,
            profile,
        )

    # Stage 4: canonical closed definition and caller/outer identity equality.
    definition_bytes = (package / "campaign-definition.json").read_bytes()
    try:
        definition = json.loads(definition_bytes.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise AssertionError(f"campaign definition is not canonical UTF-8 JSON: {error}") from error
    require(definition_bytes == _canonical_json_bytes(definition), "campaign definition bytes are not canonical")
    definition_fields = {"schema", "source", "identity", "campaign_spec", "source_facts", "required_behaviors"}
    if profile["has_presentation_assets"]:
        definition_fields.add("presentation_assets")
    root = _exact_object(definition, definition_fields, "campaign definition")
    require(root["schema"] == profile["definition_schema"], "campaign definition schema differs")
    source = _exact_object(root["source"], {"remc2_commit"}, "campaign definition.source")
    require(source["remc2_commit"] == REM_C2_COMMIT, "campaign source commit differs from pinned remc2")
    identity_value = _exact_object(root["identity"], {"external_level", "internal_level", "map_type_id"}, "campaign definition.identity")
    definition_selection = (
        _bounded_integer(identity_value["external_level"], 0xFFFF, "campaign definition external level"),
        _bounded_integer(identity_value["internal_level"], 0xFFFF, "campaign definition internal level"),
        _bounded_integer(identity_value["map_type_id"], 0xFF, "campaign definition map type id"),
    )
    require(definition_selection == outer_selection, "campaign definition/outer/caller selection mismatch")
    spec = _exact_object(root["campaign_spec"], {"authored_records", "authored_behavior_rows", "reset_sources", "initial_rng", "initial_activation", "objectives", "presentation_recipe"}, "campaign_spec")
    reset_sources = _exact_object(spec["reset_sources"], {"terrain", "entities", "players", "runtime", "feature_boundary"}, "campaign_spec.reset_sources")

    # Stage 5: all compiled member decoders and cross-member topology.
    authored, _entities, _runtime = _campaign_validate_reset_members(package, spec, files, profile)
    presentation = (package / "presentation.bin").read_bytes()
    require(len(presentation) == 10720 and presentation[:4] == b"MCV1" and (u16(presentation, 4), u16(presentation, 6), u32(presentation, 8)) == (1, 0, 347), "presentation snapshot layout changed")
    if profile["has_presentation_assets"]:
        _campaign_validate_presentation_assets(package, root["presentation_assets"], files)

    # Stage 6: immutable SourceFacts; presentation output is never consulted.
    behavior_rows = spec["authored_behavior_rows"]
    _campaign_validate_source_facts(
        package, root["source_facts"], files, authored, behavior_rows, profile
    )

    # Stage 7: reviewed operations must match the authored row that supplied
    # the Stage-6 SourceFact provenance.
    _campaign_validate_operations(spec, authored, files, package, profile)
    _campaign_validate_presentation(
        spec["presentation_recipe"],
        files,
        owner_manifest=owner_manifest,
    )

    # Stage 8: source-derived post-GenerateEvents runtime roots, then compiled
    # capabilities. Later selectors remain fail-closed at the source scheduler.
    expected_requirements, expected_keys = _campaign_required_behaviors(behavior_rows)
    require(_strict_equal(root["required_behaviors"], expected_requirements), "declared required behavior roots differ from the live reset selectors")
    capabilities = expected_keys if campaign_capabilities is None else campaign_capabilities
    missing = sorted(expected_keys - capabilities)
    require(not missing, f"campaign compiled capability is missing required behavior {missing[0] if missing else ''}")

    # Stage 9: a canonical reset receipt over already-validated reset members.
    reset_names = ["entities.bin", "feature-end.bin", "feature-start.bin", "players.bin", "runtime.bin", "terrain.bin"]
    reset_receipt = hashlib.sha256("".join(f"{name}\0{files[name]['sha256']}\n" for name in reset_names).encode("ascii")).hexdigest()
    require(len(reset_receipt) == 64, "campaign reset preflight receipt failed")
    return manifest


def verify(
    package: Path,
    source_root: Path | None = None,
    upstream_root: Path | None = None,
    *,
    enforce_reference_hashes: bool = True,
    expected_campaign_selection: tuple[int, int, int] | None = None,
    campaign_capabilities: set[tuple[int, int, int, int]] | None = None,
) -> dict:
    package = package.resolve(strict=True)
    manifest_path = package / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if manifest.get("schema") in {
        CAMPAIGN_PACKAGE_SCHEMA,
        LEVEL2_CAMPAIGN_PACKAGE_SCHEMA,
        LEVEL1_ASSET_CAMPAIGN_PACKAGE_SCHEMA,
    }:
        return verify_campaign_package(
            package,
            manifest,
            expected_selection=expected_campaign_selection,
            campaign_capabilities=campaign_capabilities,
        )
    require(manifest["schema"] in SCHEMAS, "unsupported owner-package schema")
    level = manifest["level"]
    external_level = level.get("external")
    require(
        isinstance(external_level, int)
        and not isinstance(external_level, bool)
        and 1 <= external_level <= 24,
        "unsupported owner-package level profile",
    )
    if external_level == 1:
        assert level["internal"] == 0
        assert level["map_type"] == {"id": 1, "name": "Night"}
        require(
            manifest["schema"] not in {SCHEMA_V4, SCHEMA_V5},
            "v4/v5 collision-mask schemas require an external level at least 2",
        )
    elif external_level == 2:
        require(level == {
            "external": LEVEL2_PROFILE["external"],
            "internal": LEVEL2_PROFILE["internal"],
            "map_type": LEVEL2_PROFILE["map_type"],
        }, "Level 2 profile identity mismatch (expected external 2/internal 1/Day)")
    else:
        require(
            manifest["schema"] == SCHEMA_V5,
            f"external Level {external_level} requires the v5 collision-mask schema",
        )
        require(
            level.get("internal") == external_level - 1,
            f"external Level {external_level} internal index differs from one-based identity",
        )
        require(
            level.get("map_type") in (
                {"id": 0, "name": "Day"},
                {"id": 1, "name": "Night"},
                {"id": 2, "name": "Cave"},
            ),
            f"external Level {external_level} map type identity is not pinned",
        )
    assert manifest["capture"] == {"frame": 0, "site": "0x002285ff"}
    assert manifest["byte_order"] == "little-endian"
    assert manifest["counts"]["authored_features"] == 1200
    assert manifest["counts"]["entity_slots"] == 1000
    expected_active_entities = {1: 177, 2: LEVEL2_PROFILE["active_entities"]}.get(external_level)
    if expected_active_entities is not None:
        require(
            manifest["counts"]["active_entities"] == expected_active_entities,
            f"active entity count differs for external level {external_level}",
        )
    assert manifest["dimensions"]["terrain"] == [256, 256]
    if manifest["schema"] == SCHEMA_V5:
        required_files = V5_FILES
    elif manifest["schema"] == SCHEMA_V4:
        required_files = V4_FILES
    elif manifest["schema"] == SCHEMA_V3:
        required_files = V3_FILES
    else:
        required_files = V2_FILES
    require(
        set(manifest["files"]) == required_files,
        f"manifest file set differs for schema {manifest['schema']}",
    )
    actual_members = {path.name for path in package.iterdir()}
    require(
        actual_members == required_files | {"manifest.json"},
        "package member set differs from schema contract",
    )
    assert stat.S_IMODE(package.stat().st_mode) == 0o700
    assert len(manifest["source"]["upstream_commit"]) == 40
    assert len(manifest["source"]["upstream_worktree_diff_sha256"]) == 64
    assert len(manifest["source"]["compiled_source_sha256"]) == 64
    if manifest["schema"] == SCHEMA_V3:
        if level["external"] == 1:
            assert {
                key: manifest["source"][key] for key in V3_SOURCE_IDENTITY
            } == V3_SOURCE_IDENTITY
        else:
            assert manifest["source"]["upstream_commit"] == (
                "6806c7204d17220ecc0430759f784aea8ee2ddb2"
            )
    if manifest["schema"] in {SCHEMA_V4, SCHEMA_V5}:
        require(
            external_level >= 2,
            f"v4/v5 collision-mask schemas require external Level at least 2 (got {external_level})",
        )
        require(
            "reset_projection" not in manifest,
            "Level 2 v4/v5 packages must not carry reset projection",
        )
    modified_hashes = {
        entry["path"]: entry["sha256"] for entry in manifest["source"]["modified_files"]
    }
    for path in manifest["source"]["modified_paths"]:
        assert path in modified_hashes
        assert len(modified_hashes[path]) == 64
    if source_root is not None or upstream_root is not None:
        assert source_root is not None and upstream_root is not None
        source_root = source_root.resolve(strict=True)
        upstream_root = upstream_root.resolve(strict=True)
        identity = []
        for entry in manifest["source"]["compiled_files"]:
            logical_path = entry["path"]
            if logical_path.startswith("private-exporter/"):
                path = source_root / logical_path.removeprefix("private-exporter/")
            else:
                path = upstream_root / logical_path
            assert path.is_file()
            actual = sha256(path)
            assert actual == entry["sha256"]
            identity.append(f"{logical_path}\\0{actual}\\n")
        assert identity == sorted(identity)
        aggregate = hashlib.sha256("".join(identity).encode()).hexdigest()
        assert aggregate == manifest["source"]["compiled_source_sha256"]
        if level["external"] == 1 and manifest["schema"] in {SCHEMA_V2, SCHEMA_V3, SCHEMA_V4, SCHEMA_V5}:
            verify_ambient_sequence(upstream_root, manifest)

    identity_rows = []
    for relative_name in sorted(required_files):
        entry = manifest["files"][relative_name]
        path = package / relative_name
        assert path.is_file()
        assert stat.S_IMODE(path.stat().st_mode) == 0o600
        assert path.stat().st_size == entry["bytes"]
        actual_hash = sha256(path)
        assert actual_hash == entry["sha256"]
        identity_rows.append(f"{relative_name}\0{actual_hash}\0{entry['bytes']}\n")

    expected_identity = hashlib.sha256("".join(identity_rows).encode()).hexdigest()
    assert manifest["package_sha256"] == expected_identity
    if (
        manifest["schema"] == SCHEMA_V3
        and level["external"] == 1
        and enforce_reference_hashes
    ):
        assert manifest["package_sha256"] == V3_PACKAGE_SHA256
        assert {
            name: manifest["files"][name]["sha256"] for name in V3_FILE_HASHES
        } == V3_FILE_HASHES
    assert (package / "terrain.bin").stat().st_size == 0x70000
    assert (package / "authored-level.bin").stat().st_size == 0x6604
    if manifest["schema"] == SCHEMA_V3 and level["external"] == 1:
        verify_feature_generation_input(package, manifest)
    if manifest["schema"] in {SCHEMA_V4, SCHEMA_V5}:
        verify_collision_masks(package, manifest)
    verify_binary_structure(package, manifest)

    raw_manifest = manifest_path.read_text(encoding="utf-8")
    assert str(package) not in raw_manifest
    assert "/home/" not in raw_manifest
    assert "\\\\" not in raw_manifest
    assert "missing:" not in raw_manifest
    return manifest


def compare(first: Path, second: Path) -> None:
    first_files = sorted(path.relative_to(first) for path in first.rglob("*") if path.is_file())
    second_files = sorted(path.relative_to(second) for path in second.rglob("*") if path.is_file())
    assert first_files == second_files
    for relative_name in first_files:
        assert (first / relative_name).read_bytes() == (second / relative_name).read_bytes()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("package", type=Path, nargs="?")
    parser.add_argument("--compare", type=Path, nargs=2, metavar=("FIRST", "SECOND"))
    parser.add_argument("--source-root", type=Path)
    parser.add_argument("--upstream-root", type=Path)
    args = parser.parse_args()
    try:
        if args.compare:
            first, second = (path.resolve(strict=True) for path in args.compare)
            verify(first, args.source_root, args.upstream_root)
            verify(second, args.source_root, args.upstream_root)
            compare(first, second)
            print("PASS: packages are structurally valid and byte-identical")
        elif args.package:
            verify(args.package, args.source_root, args.upstream_root)
            print("PASS: package is structurally valid")
        else:
            parser.error("provide PACKAGE or --compare FIRST SECOND")
    except (AssertionError, KeyError, OSError, json.JSONDecodeError, struct.error) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
