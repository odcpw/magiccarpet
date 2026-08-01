#!/usr/bin/env python3
"""Strict verifier for private mc2-owner-level-export/v1, v2, and v3 packages."""

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
SCHEMAS = {SCHEMA_V1, SCHEMA_V2, SCHEMA_V3}
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


def verify_binary_structure(package: Path, manifest: dict) -> None:
    external_level = manifest["level"]["external"]
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
        2 if manifest["schema"] in {SCHEMA_V2, SCHEMA_V3} else 1,
        8,
        manifest["record_sizes"]["player"],
    )
    player_record_size = manifest["record_sizes"]["player"]
    expected_player_size = (
        573 if manifest["schema"] in {SCHEMA_V2, SCHEMA_V3} else 565
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
    if manifest["schema"] in {SCHEMA_V2, SCHEMA_V3}:
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
    else:
        assert external_level == LEVEL2_PROFILE["external"]
        require(
            divergences == [],
            "Level 2 runtime scalar divergences must be empty",
        )
        require(u32(runtime, 32) == 2, "Level 2 runtime music selector must be 2")
    if external_level == 1 and manifest["schema"] in {SCHEMA_V2, SCHEMA_V3}:
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
                    "Level 2 secondary cave must not claim stock comparison",
                )
                require(
                    plane["source_status"] == "live_plane",
                    "Level 2 secondary cave source status differs",
                )
                require(
                    plane["normalization"] == "preserved",
                    "Level 2 secondary cave normalization differs",
                )
            else:
                require(
                    plane["stock_regression_compared"] is True,
                    f"Level 2 {plane['name']} must claim stock comparison",
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
        require(
            manifest["normalization"]["volatile_host_state"] == [
                "player time_393 host-clock sample zeroed; secondary terrain plane preserved"
            ],
            "Level 2 host-state normalization policy differs",
        )
        feature_generation = manifest["feature_generation_oracle"]
        require(
            feature_generation["pre_terrain"]["inactive_secondary_plane"]
            == "preserved-live-plane",
            "Level 2 pre-feature secondary terrain policy differs",
        )
        require(
            feature_generation["post_terrain"]["inactive_secondary_plane"]
            == "preserved-live-plane",
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


def verify(
    package: Path,
    source_root: Path | None = None,
    upstream_root: Path | None = None,
    *,
    enforce_reference_hashes: bool = True,
) -> dict:
    package = package.resolve(strict=True)
    manifest_path = package / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    require(manifest["schema"] in SCHEMAS, "unsupported owner-package schema")
    level = manifest["level"]
    require(level.get("external") in {1, 2}, "unsupported owner-package level profile")
    if level["external"] == 1:
        assert level["internal"] == 0
        assert level["map_type"] == {"id": 1, "name": "Night"}
    else:
        require(level == {
            "external": LEVEL2_PROFILE["external"],
            "internal": LEVEL2_PROFILE["internal"],
            "map_type": LEVEL2_PROFILE["map_type"],
        }, "Level 2 profile identity mismatch (expected external 2/internal 1/Day)")
    assert manifest["capture"] == {"frame": 0, "site": "0x002285ff"}
    assert manifest["byte_order"] == "little-endian"
    assert manifest["counts"]["authored_features"] == 1200
    assert manifest["counts"]["entity_slots"] == 1000
    expected_active_entities = (
        177 if level["external"] == 1 else LEVEL2_PROFILE["active_entities"]
    )
    require(
        manifest["counts"]["active_entities"] == expected_active_entities,
        f"active entity count differs for external level {level['external']}",
    )
    assert manifest["dimensions"]["terrain"] == [256, 256]
    required_files = V3_FILES if manifest["schema"] == SCHEMA_V3 else V2_FILES
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
        if level["external"] == 1 and manifest["schema"] in {SCHEMA_V2, SCHEMA_V3}:
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
