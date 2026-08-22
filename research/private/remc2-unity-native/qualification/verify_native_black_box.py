#!/usr/bin/env python3
"""Drive the current-source Level 1 C ABI as an external black-box consumer.

This module deliberately does not import Rust code or Unity assemblies.  It
loads the exact native plugin pinned by the private reference corpus, submits
the canonical 440-command route, and compares semantic observations with the
accepted corpus. A reset replay also covers lifecycle isolation and the fixed
logical audio-sidecar contract.
"""

from __future__ import annotations

import argparse
import ctypes as c
import copy
import hashlib
import json
import os
import struct
from dataclasses import dataclass
from pathlib import Path


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
MANIFEST_PATH = HERE / "level1-reference-corpus.json"

MC_STATUS_OK = 0
MC_STATUS_BOUNDS = 10
MC_STATUS_GENERATION_MISMATCH = 12
MC_NATIVE_ABI_VERSION = 2


class McCommandV2(c.Structure):
    _fields_ = [
        ("abi_version", c.c_uint32),
        ("struct_size", c.c_uint32),
        ("opcode", c.c_int8),
        ("argument_1", c.c_int8),
        ("argument_2", c.c_int8),
        ("roll", c.c_int8),
        ("pitch", c.c_int8),
        ("flags", c.c_int8),
        ("requested_spell", c.c_int16),
        ("reserved", c.c_uint32 * 4),
    ]


class McCameraInputV2(c.Structure):
    _fields_ = [
        ("abi_version", c.c_uint32),
        ("struct_size", c.c_uint32),
        ("look_yaw", c.c_uint16),
        ("look_pitch", c.c_uint16),
        ("reserved", c.c_uint32 * 4),
    ]


class McSessionSnapshotV2(c.Structure):
    _fields_ = [
        ("abi_version", c.c_uint32),
        ("struct_size", c.c_uint32),
        ("generation", c.c_uint64),
        ("tick", c.c_uint64),
        ("event_head", c.c_uint64),
        ("state_hash", c.c_uint64),
        ("conformance_hash", c.c_uint64),
        ("entity_index", c.c_uint16),
        ("x", c.c_uint16),
        ("y", c.c_uint16),
        ("z", c.c_int16),
        ("yaw", c.c_int16),
        ("display_pitch", c.c_int16),
        ("effective_pitch", c.c_int16),
        ("roll_accumulator", c.c_int16),
        ("target_speed", c.c_int16),
        ("actual_speed", c.c_int16),
        ("strafe_speed", c.c_int16),
        ("phase", c.c_uint8),
        ("last_resolution", c.c_uint8),
        ("profile", c.c_uint8),
        ("local_player_slot", c.c_uint8),
        ("left_spell_slot", c.c_uint8),
        ("right_spell_slot", c.c_uint8),
        ("selected_spell_ui_index", c.c_uint8),
        ("objective_textbox_ticks", c.c_uint8),
        ("objective_count", c.c_uint8),
        ("objectives_authority", c.c_uint8),
        ("reserved_1", c.c_uint8 * 2),
        ("left_spell_holder", c.c_uint16),
        ("right_spell_holder", c.c_uint16),
        ("selected_spell_holder", c.c_uint16),
        ("reserved_0", c.c_uint16),
        ("caster_mana", c.c_int32),
        ("maptype_music", c.c_int32),
        ("fire_sequence", c.c_uint32),
        ("reserved", c.c_uint32 * 4),
    ]


class McEventQueryV2(c.Structure):
    _fields_ = [
        ("abi_version", c.c_uint32),
        ("struct_size", c.c_uint32),
        ("generation", c.c_uint64),
        ("cursor", c.c_uint64),
        ("reserved", c.c_uint32 * 4),
    ]


class McEventBatchV2(c.Structure):
    _fields_ = [
        ("abi_version", c.c_uint32),
        ("struct_size", c.c_uint32),
        ("generation", c.c_uint64),
        ("next_cursor", c.c_uint64),
        ("event_head", c.c_uint64),
        ("written", c.c_uint32),
        ("has_more", c.c_uint8),
        ("reserved_0", c.c_uint8 * 3),
        ("reserved", c.c_uint32 * 4),
    ]


class McEventV2(c.Structure):
    _fields_ = [
        ("abi_version", c.c_uint32),
        ("struct_size", c.c_uint32),
        ("generation", c.c_uint64),
        ("sequence", c.c_uint64),
        ("state_hash", c.c_uint64),
        ("kind", c.c_uint32),
        ("code", c.c_uint32),
        ("entity_index", c.c_uint16),
        ("player_index", c.c_int16),
        ("value_0", c.c_int32),
        ("value_1", c.c_int32),
        ("asset_ref_index", c.c_uint32),
        ("reserved", c.c_uint32 * 4),
    ]


class McAudioCommandV2(c.Structure):
    _fields_ = [
        ("abi_version", c.c_uint32),
        ("struct_size", c.c_uint32),
        ("generation", c.c_uint64),
        ("event_sequence", c.c_uint64),
        ("state_hash", c.c_uint64),
        ("asset_ref_index", c.c_uint32),
        ("relative_x", c.c_int32),
        ("relative_y", c.c_int32),
        ("relative_z", c.c_int32),
        ("playback_rate_hz", c.c_uint32),
        ("sound_id", c.c_uint16),
        ("entity_index", c.c_uint16),
        ("player_index", c.c_int16),
        ("sound_flags", c.c_int16),
        ("volume_q15", c.c_uint16),
        ("pan_q16", c.c_uint16),
        ("rate_delta_percent", c.c_int16),
        ("voice_index", c.c_uint16),
        ("play_type", c.c_uint8),
        ("loop_count", c.c_int8),
        ("action", c.c_uint8),
        ("pooled_flush_rank", c.c_uint8),
        ("reserved", c.c_uint32 * 4),
    ]


class McPresentationStateV2(c.Structure):
    _fields_ = [
        ("abi_version", c.c_uint32),
        ("struct_size", c.c_uint32),
        ("generation", c.c_uint64),
        ("tick", c.c_uint64),
        ("source_turn", c.c_int32),
        ("camera_x", c.c_uint16),
        ("camera_y", c.c_uint16),
        ("camera_z", c.c_int16),
        ("camera_yaw", c.c_int16),
        ("camera_pitch", c.c_int16),
        ("camera_roll", c.c_int16),
        ("camera_fov", c.c_int16),
        ("camera_authority", c.c_uint8),
        ("camera_flags", c.c_uint8),
        ("reserved_0", c.c_uint32),
        ("reserved", c.c_uint32 * 4),
    ]


class McRenderCameraStateV2(c.Structure):
    _fields_ = [
        ("abi_version", c.c_uint32),
        ("struct_size", c.c_uint32),
        ("generation", c.c_uint64),
        ("tick", c.c_uint64),
        ("source_turn", c.c_int32),
        ("history_x", c.c_uint16),
        ("history_y", c.c_uint16),
        ("eye_z", c.c_int16),
        ("yaw", c.c_int16),
        ("pitch", c.c_int16),
        ("roll", c.c_int16),
        ("fov", c.c_int16),
        ("slope_target_x", c.c_int16),
        ("slope_target_y", c.c_int16),
        ("render_offset_x", c.c_int16),
        ("render_offset_y", c.c_int16),
        ("authority", c.c_uint8),
        ("flags", c.c_uint8),
        ("reserved_0", c.c_uint16),
        ("reserved", c.c_uint32 * 4),
    ]


EXPECTED_SIZES = {
    McCommandV2: 32,
    McCameraInputV2: 28,
    McSessionSnapshotV2: 120,
    McEventQueryV2: 40,
    McEventBatchV2: 56,
    McEventV2: 72,
    McAudioCommandV2: 88,
    McPresentationStateV2: 64,
    McRenderCameraStateV2: 72,
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def autoplay_command(index: int) -> bytes:
    flags = 1 if index < 360 else 0
    roll = 0
    pitch = 0
    if index < 145:
        roll = 38
    elif 180 <= index < 250:
        roll, pitch = -32, -22
    elif 250 <= index < 325:
        pitch = 24
    if 325 <= index < 365:
        flags |= 0x08
    if index in (160, 300):
        flags |= 0x10
    return bytes((0, 0, 0, roll & 0xFF, pitch & 0xFF, flags, 0, 0, 0, 0))


def signed_byte(value: int) -> int:
    return value if value < 128 else value - 256


def record(value: c.Structure, fields: tuple[str, ...]) -> dict[str, int]:
    return {field: int(getattr(value, field)) for field in fields}


SNAPSHOT_FIELDS = (
    "tick", "event_head", "state_hash", "conformance_hash", "entity_index",
    "x", "y", "z", "yaw", "display_pitch", "effective_pitch",
    "roll_accumulator", "target_speed", "actual_speed", "strafe_speed",
    "phase", "last_resolution", "profile", "fire_sequence",
)
PRESENTATION_FIELDS = (
    "tick", "source_turn", "camera_x", "camera_y", "camera_z", "camera_yaw",
    "camera_pitch", "camera_roll", "camera_fov", "camera_authority",
    "camera_flags",
)
RENDER_FIELDS = (
    "tick", "source_turn", "history_x", "history_y", "eye_z", "yaw", "pitch",
    "roll", "fov", "slope_target_x", "slope_target_y", "render_offset_x",
    "render_offset_y", "authority", "flags",
)
AUDIO_FIELDS = (
    "event_sequence", "state_hash", "asset_ref_index", "relative_x", "relative_y",
    "relative_z", "playback_rate_hz", "sound_id", "entity_index", "player_index",
    "sound_flags", "volume_q15", "pan_q16", "rate_delta_percent", "voice_index",
    "play_type", "loop_count", "action", "pooled_flush_rank", "pooled_flush_id",
)


def canonical_event(event: McEventV2, include_generation: bool) -> bytes:
    prefix = struct.pack("<Q", event.generation) if include_generation else b""
    return prefix + struct.pack(
        "<QQIIHhiiI",
        event.sequence,
        event.state_hash,
        event.kind,
        event.code,
        event.entity_index,
        event.player_index,
        event.value_0,
        event.value_1,
        event.asset_ref_index,
    )


def canonical_audio(value: dict[str, int], include_generation: bool) -> bytes:
    prefix = struct.pack("<Q", value["generation"]) if include_generation else b""
    return prefix + struct.pack(
        "<QQIiiiIHHhhHHhHBbBBQ",
        value["event_sequence"], value["state_hash"], value["asset_ref_index"],
        value["relative_x"], value["relative_y"], value["relative_z"],
        value["playback_rate_hz"], value["sound_id"], value["entity_index"],
        value["player_index"], value["sound_flags"], value["volume_q15"],
        value["pan_q16"], value["rate_delta_percent"], value["voice_index"],
        value["play_type"], value["loop_count"], value["action"],
        value["pooled_flush_rank"], value["pooled_flush_id"],
    )


@dataclass
class RouteCheckpoint:
    snapshot: dict[str, int]
    presentation: dict[str, int]
    render: dict[str, int]
    events: list[bytes]
    lifecycle_events: list[bytes]
    audio: list[dict[str, int]]


class NativeClient:
    def __init__(
        self,
        manifest: dict,
        plugin: Path | None = None,
        plugin_pin_field: str = "current_source_plugin_file_sha256",
    ) -> None:
        pins = manifest["runtime_pins"]
        self.package_dir = REPO / "research/private/generated/level1-export-m"
        self.closure_path = REPO / "research/private/generated/level1-asset-closure.json"
        self.cache_dir = (
            REPO / "research/private/generated/level1-unity-asset-cache-plan29-final"
        )
        self.plugin = plugin or (
            REPO / "research/private/remc2-sim-rs/target/release/libsim_capi.so"
        )
        require(self.plugin.is_file(), f"missing native plugin: {self.plugin}")
        require(
            sha256_file(self.plugin) == pins[plugin_pin_field],
            "native black-box plugin does not match the corpus pin",
        )
        identity_files = {
            "package_manifest_file_sha256": self.package_dir / "manifest.json",
            "closure_file_sha256": self.closure_path,
            "owner_cache_manifest_file_sha256": self.cache_dir / "manifest.json",
        }
        for field, path in identity_files.items():
            require(path.is_file(), f"missing owner input: {path}")
            require(sha256_file(path) == pins[field], f"native input pin mismatch: {field}")
        for cls, expected in EXPECTED_SIZES.items():
            require(c.sizeof(cls) == expected, f"ctypes layout drift: {cls.__name__}")
        os.environ["MC_ASSET_CACHE_PATH"] = str(self.cache_dir)
        self.lib = c.CDLL(str(self.plugin))
        self._bind()
        require(
            self.lib.mc_native_abi_version() == MC_NATIVE_ABI_VERSION,
            "native ABI version mismatch",
        )
        self.handle = c.c_uint64()
        package = str(self.package_dir).encode()
        closure = str(self.closure_path).encode()
        package_bytes = (c.c_uint8 * len(package)).from_buffer_copy(package)
        closure_bytes = (c.c_uint8 * len(closure)).from_buffer_copy(closure)
        status = self.lib.mc_native_session_open(
            package_bytes,
            len(package),
            closure_bytes,
            len(closure),
            c.byref(self.handle),
        )
        self.check(status, "open native session")

    def _bind(self) -> None:
        lib = self.lib
        lib.mc_native_abi_version.restype = c.c_uint32
        lib.mc_native_session_open.argtypes = [
            c.POINTER(c.c_uint8), c.c_uint32,
            c.POINTER(c.c_uint8), c.c_uint32,
            c.POINTER(c.c_uint64),
        ]
        lib.mc_native_session_open.restype = c.c_int32
        lib.mc_native_session_destroy.argtypes = [c.c_uint64]
        lib.mc_native_session_destroy.restype = c.c_int32
        lib.mc_native_session_snapshot.argtypes = [
            c.c_uint64, c.POINTER(McSessionSnapshotV2), c.c_uint32,
        ]
        lib.mc_native_session_snapshot.restype = c.c_int32
        lib.mc_native_session_reset.argtypes = [
            c.c_uint64, c.POINTER(McSessionSnapshotV2), c.c_uint32,
        ]
        lib.mc_native_session_reset.restype = c.c_int32
        lib.mc_native_session_step_with_camera.argtypes = [
            c.c_uint64,
            c.POINTER(McCommandV2),
            c.POINTER(McCameraInputV2),
            c.POINTER(McSessionSnapshotV2),
            c.c_uint32,
        ]
        lib.mc_native_session_step_with_camera.restype = c.c_int32
        lib.mc_native_events_copy_since.argtypes = [
            c.c_uint64,
            c.POINTER(McEventQueryV2),
            c.POINTER(McEventV2),
            c.c_uint32,
            c.c_uint32,
            c.POINTER(McEventBatchV2),
            c.c_uint32,
        ]
        lib.mc_native_events_copy_since.restype = c.c_int32
        lib.mc_native_audio_command_for_event.argtypes = [
            c.c_uint64,
            c.c_uint64,
            c.c_uint64,
            c.POINTER(McAudioCommandV2),
            c.c_uint32,
        ]
        lib.mc_native_audio_command_for_event.restype = c.c_int32
        lib.mc_native_presentation_state.argtypes = [
            c.c_uint64, c.POINTER(McPresentationStateV2), c.c_uint32,
        ]
        lib.mc_native_presentation_state.restype = c.c_int32
        lib.mc_native_render_camera_state.argtypes = [
            c.c_uint64, c.POINTER(McRenderCameraStateV2), c.c_uint32,
        ]
        lib.mc_native_render_camera_state.restype = c.c_int32

    @staticmethod
    def check(status: int, operation: str) -> None:
        require(status == MC_STATUS_OK, f"{operation} failed with status {status}")

    def close(self) -> None:
        if self.handle.value:
            self.check(
                self.lib.mc_native_session_destroy(self.handle.value),
                "destroy native session",
            )
            self.handle.value = 0

    def snapshot(self) -> McSessionSnapshotV2:
        value = McSessionSnapshotV2()
        self.check(
            self.lib.mc_native_session_snapshot(
                self.handle.value, c.byref(value), c.sizeof(value)
            ),
            "read snapshot",
        )
        return value

    def reset(self) -> McSessionSnapshotV2:
        value = McSessionSnapshotV2()
        self.check(
            self.lib.mc_native_session_reset(
                self.handle.value, c.byref(value), c.sizeof(value)
            ),
            "reset session",
        )
        return value

    def step(self, raw: bytes) -> McSessionSnapshotV2:
        command = McCommandV2(
            abi_version=MC_NATIVE_ABI_VERSION,
            struct_size=c.sizeof(McCommandV2),
            opcode=signed_byte(raw[0]),
            argument_1=signed_byte(raw[1]),
            argument_2=signed_byte(raw[2]),
            roll=signed_byte(raw[3]),
            pitch=signed_byte(raw[4]),
            flags=signed_byte(raw[5]),
            requested_spell=-1,
        )
        camera = McCameraInputV2(
            abi_version=MC_NATIVE_ABI_VERSION,
            struct_size=c.sizeof(McCameraInputV2),
        )
        value = McSessionSnapshotV2()
        self.check(
            self.lib.mc_native_session_step_with_camera(
                self.handle.value,
                c.byref(command),
                c.byref(camera),
                c.byref(value),
                c.sizeof(value),
            ),
            "step session",
        )
        return value

    def presentation(self) -> McPresentationStateV2:
        value = McPresentationStateV2()
        self.check(
            self.lib.mc_native_presentation_state(
                self.handle.value, c.byref(value), c.sizeof(value)
            ),
            "read presentation state",
        )
        return value

    def render_camera(self) -> McRenderCameraStateV2:
        value = McRenderCameraStateV2()
        self.check(
            self.lib.mc_native_render_camera_state(
                self.handle.value, c.byref(value), c.sizeof(value)
            ),
            "read render camera",
        )
        return value

    def events_since(
        self, generation: int, cursor: int
    ) -> tuple[list[McEventV2], int]:
        values: list[McEventV2] = []
        while True:
            query = McEventQueryV2(
                abi_version=MC_NATIVE_ABI_VERSION,
                struct_size=c.sizeof(McEventQueryV2),
                generation=generation,
                cursor=cursor,
            )
            buffer = (McEventV2 * 256)()
            batch = McEventBatchV2()
            self.check(
                self.lib.mc_native_events_copy_since(
                    self.handle.value,
                    c.byref(query),
                    buffer,
                    len(buffer),
                    c.sizeof(McEventV2),
                    c.byref(batch),
                    c.sizeof(batch),
                ),
                "copy event page",
            )
            require(batch.written > 0 or batch.has_more == 0, "empty event page loop")
            values.extend(buffer[index] for index in range(batch.written))
            cursor = batch.next_cursor
            if not batch.has_more:
                return values, cursor

    def audio_for(self, event: McEventV2) -> dict[str, int] | None:
        value = McAudioCommandV2()
        status = self.lib.mc_native_audio_command_for_event(
            self.handle.value,
            event.generation,
            event.sequence,
            c.byref(value),
            c.sizeof(value),
        )
        if status == MC_STATUS_BOUNDS:
            return None
        self.check(status, "read audio sidecar")
        require(value.generation == event.generation, "audio generation mismatch")
        result = record(value, AUDIO_FIELDS[:-1])
        result["pooled_flush_id"] = int(value.reserved[0]) | (int(value.reserved[1]) << 32)
        result["generation"] = int(value.generation)
        return result


def run_route(client: NativeClient, generation: int, checkpoints: set[int]) -> dict[int, RouteCheckpoint]:
    observed: dict[int, RouteCheckpoint] = {}
    cursor = 0
    for index in range(440):
        snapshot = client.step(autoplay_command(index))
        if snapshot.tick not in checkpoints:
            continue
        events, cursor = client.events_since(generation, cursor)
        observed[int(snapshot.tick)] = RouteCheckpoint(
            snapshot=record(snapshot, SNAPSHOT_FIELDS),
            presentation=record(client.presentation(), PRESENTATION_FIELDS),
            render=record(client.render_camera(), RENDER_FIELDS),
            events=[canonical_event(event, False) for event in events],
            lifecycle_events=[canonical_event(event, True) for event in events],
            audio=[audio for event in events if (audio := client.audio_for(event)) is not None],
        )
    require(set(observed) == checkpoints, "native black-box checkpoints missing")
    return observed


def verify_checkpoint(expected: dict, actual: RouteCheckpoint) -> None:
    tick = expected["unity_checkpoint_tick"]
    state = expected["state_identity"]
    fields = {
        "tick": "tick",
        "event_head": "event_head",
        "state_hash_fnv1a64": "state_hash",
        "conformance_hash_fnv1a64": "conformance_hash",
        "entity_index": "entity_index",
        "yaw": "yaw",
        "display_pitch": "display_pitch",
        "effective_pitch": "effective_pitch",
        "roll": "roll_accumulator",
        "target_speed": "target_speed",
        "actual_speed": "actual_speed",
        "strafe_speed": "strafe_speed",
        "phase": "phase",
        "last_resolution": "last_resolution",
        "profile": "profile",
        "fire_sequence": "fire_sequence",
    }
    for expected_name, actual_name in fields.items():
        expected_value = state[expected_name]
        if expected_name.endswith("fnv1a64"):
            expected_value = int(expected_value, 16)
        require(
            expected_value == actual.snapshot[actual_name],
            f"black-box snapshot {expected_name} mismatch at {tick}",
        )
    require(
        state["position"] == [
            actual.snapshot["x"], actual.snapshot["y"], actual.snapshot["z"]
        ],
        f"black-box position mismatch at {tick}",
    )
    event_identity = expected["event_identity"]
    require(len(actual.events) == event_identity["count"], f"event count mismatch at {tick}")
    require(
        hashlib.sha256(b"".join(actual.events)).hexdigest()
        == event_identity["content_sha256"],
        f"event content digest mismatch at {tick}",
    )
    require(
        hashlib.sha256(b"".join(actual.lifecycle_events)).hexdigest()
        == event_identity["lifecycle_sha256"],
        f"event lifecycle digest mismatch at {tick}",
    )
    audio_identity = expected["audio_identity"]
    actual_audio_hash = hashlib.sha256(
        b"".join(canonical_audio(item, False) for item in actual.audio)
    ).hexdigest()
    actual_audio_lifecycle_hash = hashlib.sha256(
        b"".join(canonical_audio(item, True) for item in actual.audio)
    ).hexdigest()
    require(len(actual.audio) == audio_identity["count"], f"audio count mismatch at {tick}")
    require(
        audio_identity["content_schema"] == "mc2-native-audio-logical-content-le/sha256-v1"
        and audio_identity["lifecycle_schema"]
        == "mc2-native-audio-logical-lifecycle-le/sha256-v1",
        f"audio schema mismatch at {tick}",
    )
    require(
        actual_audio_hash == audio_identity["content_sha256"],
        f"audio content digest mismatch at {tick}",
    )
    require(
        actual_audio_lifecycle_hash == audio_identity["lifecycle_sha256"],
        f"audio lifecycle digest mismatch at {tick}",
    )
    presentation = expected["camera_identity"]["presentation"]
    presentation_fields = {
        "tick": "tick", "source_turn": "source_turn", "yaw": "camera_yaw",
        "pitch": "camera_pitch", "roll": "camera_roll", "fov": "camera_fov",
        "authority": "camera_authority", "flags": "camera_flags",
    }
    for expected_name, actual_name in presentation_fields.items():
        require(
            presentation[expected_name] == actual.presentation[actual_name],
            f"black-box presentation {expected_name} mismatch at {tick}",
        )
    require(
        presentation["position"] == [
            actual.presentation["camera_x"],
            actual.presentation["camera_y"],
            actual.presentation["camera_z"],
        ],
        f"black-box presentation position mismatch at {tick}",
    )
    render = expected["camera_identity"]["render"]
    render_fields = (
        "tick", "source_turn", "eye_z", "yaw", "pitch", "roll", "fov",
        "authority", "flags",
    )
    for field in render_fields:
        require(render[field] == actual.render[field], f"black-box render {field} mismatch at {tick}")
    require(
        render["history"] == [actual.render["history_x"], actual.render["history_y"]]
        and render["slope_target"] == [
            actual.render["slope_target_x"], actual.render["slope_target_y"]
        ]
        and render["render_offset"] == [
            actual.render["render_offset_x"], actual.render["render_offset_y"]
        ],
        f"black-box render vectors mismatch at {tick}",
    )


def normalized_route(route: dict[int, RouteCheckpoint]) -> dict[int, dict]:
    return {
        tick: {
            "snapshot": value.snapshot,
            "presentation": value.presentation,
            "render": value.render,
            "events": value.events,
            "audio": [
                {key: field for key, field in item.items() if key != "generation"}
                for item in value.audio
            ],
        }
        for tick, value in route.items()
    }


def expect_rejection(operation, message: str) -> None:
    try:
        operation()
    except RuntimeError:
        return
    raise RuntimeError(message)


def verify_negative_controls(
    manifest: dict, expected: dict[int, dict], observed: dict[int, RouteCheckpoint]
) -> None:
    first_tick = min(expected)
    wrong_hash = copy.deepcopy(expected[first_tick])
    wrong_hash["state_identity"]["state_hash_fnv1a64"] = "0000000000000000"
    expect_rejection(
        lambda: verify_checkpoint(wrong_hash, observed[first_tick]),
        "planted expected hash was accepted",
    )

    wrong_order = copy.deepcopy(observed[first_tick])
    wrong_order.events.reverse()
    expect_rejection(
        lambda: verify_checkpoint(expected[first_tick], wrong_order),
        "planted event order mutation was accepted",
    )

    wrong_audio_order = copy.deepcopy(observed[first_tick])
    wrong_audio_order.audio.reverse()
    expect_rejection(
        lambda: verify_checkpoint(expected[first_tick], wrong_audio_order),
        "planted audio order mutation was accepted",
    )

    wrong_audio_field = copy.deepcopy(observed[first_tick])
    require(bool(wrong_audio_field.audio), "audio mutation control needs a non-empty trace")
    wrong_audio_field.audio[0]["pooled_flush_id"] ^= 1
    expect_rejection(
        lambda: verify_checkpoint(expected[first_tick], wrong_audio_field),
        "planted audio field mutation was accepted",
    )

    zero_audio = copy.deepcopy(observed[first_tick])
    zero_audio.audio.clear()
    expect_rejection(
        lambda: verify_checkpoint(expected[first_tick], zero_audio),
        "zero selected audio trace was accepted",
    )

    for field in (
        "current_source_plugin_file_sha256",
        "package_manifest_file_sha256",
        "closure_file_sha256",
        "owner_cache_manifest_file_sha256",
    ):
        wrong_identity = copy.deepcopy(manifest)
        wrong_identity["runtime_pins"][field] = "00" * 32
        expect_rejection(
            lambda candidate=wrong_identity: NativeClient(candidate),
            f"wrong {field} was accepted",
        )


def verify_native_black_box(
    manifest: dict,
    plugin: Path | None = None,
    plugin_pin_field: str = "current_source_plugin_file_sha256",
    run_negative_controls: bool = True,
) -> dict[int, dict[str, str | int]]:
    suite = next(
        item for item in manifest["suites"]
        if item["id"] == "unity-synthetic-presentation-route"
    )
    expected = {item["unity_checkpoint_tick"]: item for item in suite["checkpoints"]}
    client = NativeClient(manifest, plugin, plugin_pin_field)
    try:
        initial = client.snapshot()
        reset = suite["reset"]
        require(initial.generation == 1 and initial.tick == 0, "initial lifecycle mismatch")
        require(initial.state_hash == int(reset["state_hash_fnv1a64"], 16), "reset state mismatch")
        require(
            initial.conformance_hash == int(reset["live_session_conformance_hash_fnv1a64"], 16),
            "reset conformance mismatch",
        )
        first = run_route(client, initial.generation, set(expected))
        for tick, checkpoint in expected.items():
            verify_checkpoint(checkpoint, first[tick])
        if run_negative_controls:
            verify_negative_controls(manifest, expected, first)

        second_reset = client.reset()
        require(second_reset.generation == 2, "reset did not advance generation")
        stale_query = McEventQueryV2(
            abi_version=MC_NATIVE_ABI_VERSION,
            struct_size=c.sizeof(McEventQueryV2),
            generation=initial.generation,
            cursor=0,
        )
        stale_buffer = (McEventV2 * 1)()
        stale_batch = McEventBatchV2()
        status = client.lib.mc_native_events_copy_since(
            client.handle.value,
            c.byref(stale_query),
            stale_buffer,
            1,
            c.sizeof(McEventV2),
            c.byref(stale_batch),
            c.sizeof(stale_batch),
        )
        require(status == MC_STATUS_GENERATION_MISMATCH, "stale generation query was accepted")
        stale_audio = McAudioCommandV2()
        status = client.lib.mc_native_audio_command_for_event(
            client.handle.value,
            initial.generation,
            1,
            c.byref(stale_audio),
            c.sizeof(stale_audio),
        )
        require(status == MC_STATUS_GENERATION_MISMATCH, "stale audio generation was accepted")
        second = run_route(client, second_reset.generation, set(expected))
        require(
            normalized_route(first) == normalized_route(second),
            "reset replay changed normalized native observations",
        )
        return {
            tick: {
                "audio_count": len(value.audio),
                "audio_content_sha256": hashlib.sha256(
                    b"".join(canonical_audio(item, False) for item in value.audio)
                ).hexdigest(),
                "audio_lifecycle_sha256": hashlib.sha256(
                    b"".join(canonical_audio(item, True) for item in value.audio)
                ).hexdigest(),
            }
            for tick, value in first.items()
        }
    finally:
        client.close()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--print-candidate", action="store_true")
    args = parser.parse_args()
    manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    if args.print_candidate:
        suite = next(
            item for item in manifest["suites"]
            if item["id"] == "unity-synthetic-presentation-route"
        )
        ticks = {item["unity_checkpoint_tick"] for item in suite["checkpoints"]}
        client = NativeClient(manifest)
        try:
            route = run_route(client, client.snapshot().generation, ticks)
        finally:
            client.close()
        candidate = {}
        for tick, value in route.items():
            candidate[tick] = {
                "event_count": len(value.events),
                "event_content_sha256": hashlib.sha256(b"".join(value.events)).hexdigest(),
                "event_lifecycle_sha256": hashlib.sha256(
                    b"".join(value.lifecycle_events)
                ).hexdigest(),
                "audio_count": len(value.audio),
                "audio_content_sha256": hashlib.sha256(
                    b"".join(canonical_audio(item, False) for item in value.audio)
                ).hexdigest(),
                "audio_lifecycle_sha256": hashlib.sha256(
                    b"".join(canonical_audio(item, True) for item in value.audio)
                ).hexdigest(),
            }
        print(json.dumps(candidate, indent=2, sort_keys=True))
        return
    audio = verify_native_black_box(manifest)
    built_plugin = (
        REPO
        / "research/private/remc2-unity-native/Builds/Linux/"
        "MagicCarpetNative_Data/Plugins/x86_64/libsim_capi.so"
    )
    built_audio = verify_native_black_box(
        manifest,
        built_plugin,
        "built_native_plugin_file_sha256",
        run_negative_controls=False,
    )
    require(audio == built_audio, "current-source and built-player audio identities differ")
    print(
        "MC_LEVEL1_NATIVE_BLACK_BOX PASS "
        f"checkpoints={len(audio)} steps=440 current_source=exact built_player=exact "
        "replay=exact stale_generation=rejected "
        f"audio_identities={json.dumps(audio, sort_keys=True, separators=(',', ':'))} "
        "negative_controls=9"
    )


if __name__ == "__main__":
    main()
