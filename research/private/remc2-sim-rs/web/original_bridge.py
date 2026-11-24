#!/usr/bin/env python3
"""Local Rust-backed Original Level 1 indexed scene bridge."""
from __future__ import annotations

import argparse
import ctypes as c
import hashlib
import hmac
import json
import mimetypes
import os
import queue
import re
import secrets
import stat
import struct
import tempfile
import threading
import time
from collections import deque
from http import cookies
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlsplit, parse_qs
from original_assets import (OriginalAssets, FrameQuery, FrameInfo as GenericFrameInfo,
                             read_hud, read_minimap, read_objective, read_world_map)
from original_world import ModernWorld, OriginalWorld
from original_media import OriginalMedia

ROOT = Path(__file__).resolve().parents[4]
PRIVATE = ROOT / "research/private"
LIBRARY = PRIVATE / "remc2-unity-native/Builds/Original/Builds/Linux/MagicCarpetNative_Data/Plugins/x86_64/libsim_capi.so"
PACKAGES = PRIVATE / "generated/mc-voxel-combat-20261107/native-mana-scenario-01.json"
PIXELS = 320 * 200
HEADER = struct.Struct("<4sHHHHIQQQQQ")
WORLD_HEADER = struct.Struct("<4sHHQQQQQIIII")
MODERN_ASSETS = PRIVATE / "generated/browser-level1-assets"
MEDIA_CACHE = PRIVATE / "generated/mc2-campaign-media-cache-decoded"


class FrameInfo(c.Structure):
    _fields_ = [("abi", c.c_uint32), ("size", c.c_uint32),
                ("generation", c.c_uint64), ("tick", c.c_uint64),
                ("width", c.c_uint16), ("height", c.c_uint16),
                ("stride", c.c_uint16), ("reserved0", c.c_uint16),
                ("index_bytes", c.c_uint32), ("palette_bytes", c.c_uint32),
                ("index_hash", c.c_uint64), ("palette_hash", c.c_uint64),
                ("terrain_input_hash", c.c_uint64), ("terrain_command_hash", c.c_uint64),
                ("package_sha", c.c_ubyte * 32), ("cache_sha", c.c_ubyte * 32),
                ("reserved", c.c_uint32 * 4)]


class Native:
    """All methods, including construction and destruction, run on one owner thread."""
    def __init__(self, library: Path, packages: list[dict]):
        self.lib = c.CDLL(str(library.resolve()))
        self.handle = c.c_uint64()
        self.language_cache = None
        self.media_installed = False
        self.observation_buffer = (c.c_ubyte * (2 * 1024 * 1024))()
        self.request = json.dumps({"request_contract": "carpet-flight-installed-package-set-bootstrap/v1",
                                   "packages": packages}, separators=(",", ":")).encode()
        u64, u32, ptr = c.c_uint64, c.c_uint32, c.c_void_p
        for name, args in {
            "mc_game_runtime_create_v1": [ptr],
            "mc_game_runtime_destroy_v1": [u64],
            "mc_game_runtime_identity_json_v1": [u64, ptr, u32, ptr],
            "mc_game_runtime_bootstrap_package_set_json_v1": [u64, u64, ptr, u32],
            "mc_game_runtime_open_level_v1": [u64, u64],
            "mc_game_runtime_campaign_menu_command_json_v1": [u64, u64, ptr, u32],
            "mc_game_runtime_media_command_json_v1": [u64, u64, ptr, u32],
            "mc_game_runtime_finish_mission_if_ready_result_v1": [u64, u64, u64, u64, ptr],
            "mc_native_campaign_day_indexed_frame_render": [u64, u64, u64, ptr, u32],
            "mc_native_campaign_presentation_frame_copy": [u64, ptr, ptr, u32, ptr, u32, ptr, u32],
            "mc_native_campaign_day_indexed_frame_copy": [u64, u64, u64, ptr, u32, ptr, u32, ptr, ptr],
            "mc_native_campaign_scene_frame_copy": [u64, u64, u64, ptr, u32, ptr, ptr, ptr, ptr, ptr],
            "mc_game_session_step_json_v1": [u64, u64, u64, ptr, u32],
            "mc_game_session_reset_v1": [u64, u64, u64],
            "mc_game_runtime_pause_v1": [u64, u64],
            "mc_game_runtime_resume_v1": [u64, u64],
            "mc_game_runtime_preferences_command_json_v1": [u64, u64, ptr, u32],
            "mc_game_runtime_toggle_icons_map_v1": [u64, u64, u64, u64],
            "mc_game_runtime_save_replay_json_v1": [u64, u64, u64, u64, ptr, u32, ptr],
            "mc_game_runtime_load_replay_from_runtime_json_v1": [u64, u64, u64, u64, ptr, u32, ptr, u32],
            "mc_game_runtime_render_observation_copy_v1": [u64, u64, u64, u64, ptr, u32, ptr],
            "mc_game_runtime_presentation_frame_v1": [u64, u64, u64, u64, c.c_uint8],
            "mc_native_last_error_length": [ptr], "mc_native_last_error_copy": [ptr, u32, ptr],
        }.items():
            fn = getattr(self.lib, name); fn.argtypes = args; fn.restype = c.c_int32
        if c.sizeof(FrameInfo) != 152:
            raise RuntimeError("native FrameInfo ABI layout mismatch")
        self.check(self.lib.mc_game_runtime_create_v1(c.byref(self.handle)), "create")
        try:
            self.check(self.lib.mc_game_runtime_bootstrap_package_set_json_v1(
                self.handle, 1, self.request, len(self.request)), "bootstrap")
            identity = self.identity()
            self.check(self.lib.mc_game_runtime_open_level_v1(self.handle, int(identity["generation"])), "open level")
            self.identity_value = self.identity()
        except BaseException:
            self.close(); raise

    def check(self, status: int, action: str):
        if status == 0: return
        length = c.c_uint32()
        self.lib.mc_native_last_error_length(c.byref(length))
        buf = c.create_string_buffer(length.value + 1)
        self.lib.mc_native_last_error_copy(buf, len(buf), c.byref(length))
        raise RuntimeError(f"{action}: native status {status}: {buf.value.decode(errors='replace')}")

    def read_json(self, fn, *args):
        length = c.c_uint32()
        status = fn(*args, None, 0, c.byref(length))
        if status != 10 or not 0 < length.value <= 16_000_000:
            self.check(status, "JSON size")
            raise RuntimeError("invalid native JSON length")
        data = (c.c_ubyte * length.value)()
        self.check(fn(*args, data, len(data), c.byref(length)), "JSON copy")
        return json.loads(bytes(data[:length.value]))

    def identity(self):
        return self.read_json(self.lib.mc_game_runtime_identity_json_v1, self.handle)

    def bind_language(self, handle):
        if self.language_cache is None:
            return
        def identity():
            return self.read_json(self.lib.mc_game_runtime_identity_json_v1, handle)
        state = identity()
        if state["mode"] not in ("playing", "paused"):
            return
        resume = state["mode"] != "paused"
        if resume:
            self.check(self.lib.mc_game_runtime_pause_v1(handle, int(state["generation"])), "pause for language")
        try:
            command = json.dumps({"operation": "install_languages", "cache_dir": str(self.language_cache)}).encode()
            self.check(self.lib.mc_game_runtime_preferences_command_json_v1(
                handle, int(identity()["generation"]), command, len(command)), "install selected language")
        finally:
            if resume:
                self.check(self.lib.mc_game_runtime_resume_v1(handle, int(identity()["generation"])), "resume after language")

    def counters(self):
        ident = self.identity_value
        return int(ident["generation"]), int(ident["session"]["generation"]), int(ident["session"]["tick"])

    def step(self, command):
        _, generation, tick = self.counters()
        payload = json.dumps(command, separators=(",", ":")).encode()
        self.check(self.lib.mc_game_session_step_json_v1(self.handle, generation, tick, payload, len(payload)), "step")
        # The source exit cinematic advances state while its gameplay tick is
        # frozen; source speed can also advance by more than one turn.
        self.identity_value = self.identity()

    def finish_if_ready(self):
        outer, generation, tick = self.counters()
        changed = c.c_uint8()
        self.check(self.lib.mc_game_runtime_finish_mission_if_ready_result_v1(
            self.handle, outer, generation, tick, c.byref(changed)), "finish mission")
        if changed.value not in (0, 1): raise RuntimeError("invalid mission transition flag")
        if changed.value: self.identity_value = self.identity()
        return bool(changed.value)

    def campaign_menu(self, action, level_id=None):
        command = {"action": action}
        if level_id is not None: command["level_id"] = level_id
        payload = json.dumps(command, separators=(",", ":")).encode()
        self.check(self.lib.mc_game_runtime_campaign_menu_command_json_v1(
            self.handle, int(self.identity_value["generation"]), payload, len(payload)), "campaign menu")
        self.identity_value = self.identity()

    def open_pending(self):
        self.check(self.lib.mc_game_runtime_open_level_v1(
            self.handle, int(self.identity_value["generation"])), "open selected mission")
        self.identity_value = self.identity()

    def media_command(self, command):
        payload = json.dumps(command, separators=(",", ":"), ensure_ascii=False).encode()
        self.check(self.lib.mc_game_runtime_media_command_json_v1(
            self.handle, int(self.identity_value["generation"]), payload, len(payload)), "frontend media")
        self.identity_value = self.identity()

    def observe(self):
        outer, generation, tick = self.counters()
        data, written = self.observation_buffer, c.c_uint32()
        self.check(self.lib.mc_game_runtime_render_observation_copy_v1(
            self.handle, outer, generation, tick, data, len(data), c.byref(written)), "copy render observation")
        if not 12 <= written.value <= len(data): raise RuntimeError("invalid render observation length")
        packet = bytes(data[:written.value])
        if packet[:4] != b"MCPO" or struct.unpack_from("<I", packet, 8)[0] != len(packet):
            raise RuntimeError("invalid render observation packet")
        return packet

    def presentation_frame(self):
        outer, generation, tick = self.counters()
        self.check(self.lib.mc_game_runtime_presentation_frame_v1(
            self.handle, outer, generation, tick, 1), "presentation frame")

    def control(self, action, saved):
        outer = int(self.identity_value["generation"])
        session = self.identity_value.get("session")
        generation = int(session["generation"]) if session else 0
        tick = int(session["tick"]) if session else 0
        if action in ("pause", "resume"):
            fn = self.lib.mc_game_runtime_pause_v1 if action == "pause" else self.lib.mc_game_runtime_resume_v1
            self.check(fn(self.handle, outer), action)
        elif action == "reset":
            self.check(self.lib.mc_game_session_reset_v1(self.handle, generation, tick), "reset")
        elif action == "map":
            self.check(self.lib.mc_game_runtime_toggle_icons_map_v1(self.handle, outer, generation, tick), "map")
        elif action == "save":
            length = c.c_uint32()
            fn = self.lib.mc_game_runtime_save_replay_json_v1
            status = fn(self.handle, outer, generation, tick, None, 0, c.byref(length))
            if status != 10 or not 0 < length.value <= 16_000_000:
                self.check(status, "save size"); raise RuntimeError("invalid native save size")
            data = (c.c_ubyte * length.value)()
            self.check(fn(self.handle, outer, generation, tick, data, len(data), c.byref(length)), "save")
            saved = bytes(data[:length.value])
        elif action == "load":
            if not saved: raise ValueError("no game has been saved")
            new_handle = c.c_uint64()
            self.check(self.lib.mc_game_runtime_create_v1(c.byref(new_handle)), "create load runtime")
            try:
                self.check(self.lib.mc_game_runtime_load_replay_from_runtime_json_v1(
                    new_handle, 1, self.handle, outer, self.request, len(self.request), saved, len(saved)), "load")
                self.bind_language(new_handle)
            except BaseException:
                self.check(self.lib.mc_game_runtime_destroy_v1(new_handle), "destroy failed load runtime")
                raise
            old = self.handle; self.handle = new_handle
            self.media_installed = False
            self.check(self.lib.mc_game_runtime_destroy_v1(old), "destroy previous runtime")
        self.identity_value = self.identity()
        return saved

    def frame(self, sequence: int):
        ident = self.identity_value
        generation, tick = int(ident["session"]["generation"]), int(ident["session"]["tick"])
        # The generic campaign endpoint renders the current generation/tick,
        # including a first presentation after pause or a loaded replay. The
        # retained Day adapter alone enforces a sequential initial tick.
        query = FrameQuery(abi_version=2, struct_size=64, generation=generation, tick=tick,
                           presentation_revision=tick, viewport_width=320, viewport_height=200,
                           screen_stride=320, vga_resolution=1)
        generic = (c.c_ubyte * PIXELS)(); generic_info = GenericFrameInfo()
        self.check(self.lib.mc_native_campaign_presentation_frame_copy(
            self.handle, c.byref(query), generic, len(generic), None, 0, c.byref(generic_info), c.sizeof(generic_info)), "render campaign presentation")
        if (generic_info.abi_version, generic_info.struct_size, generic_info.generation, generic_info.tick,
                generic_info.width, generic_info.height, generic_info.stride, generic_info.index_bytes) != (
                2, 128, generation, tick, 320, 200, 320, PIXELS):
            raise RuntimeError("native generic presentation metadata mismatch")
        self.minimap_terrain = b"".join(bytes(generic[y * 320:y * 320 + 64]) for y in range(64))
        info = FrameInfo()
        self.check(self.lib.mc_native_campaign_day_indexed_frame_render(
            self.handle, generation, tick, c.byref(info), c.sizeof(info)), "render indexed world")
        if (info.abi, info.size, info.generation, info.tick, info.width, info.height, info.stride,
                info.index_bytes, info.palette_bytes) != (2, 152, generation, tick, 320, 200, 320, PIXELS, 768):
            raise RuntimeError("native indexed world metadata mismatch")
        indices, palette = (c.c_ubyte * PIXELS)(), (c.c_ubyte * 768)()
        index_count, palette_count = c.c_uint32(), c.c_uint32()
        self.check(self.lib.mc_native_campaign_day_indexed_frame_copy(
            self.handle, generation, tick, indices, len(indices), palette, len(palette),
            c.byref(index_count), c.byref(palette_count)), "copy indexed world")
        if bytes(generic) != bytes(indices) or generic_info.index_hash != info.index_hash:
            raise RuntimeError("generic and Day indexed frame disagree")
        scene, written, revision = (c.c_ubyte * PIXELS)(), c.c_uint32(), c.c_uint64()
        blur, exit_tables, sprites = c.c_ubyte(), c.c_ubyte(), c.c_ubyte()
        self.check(self.lib.mc_native_campaign_scene_frame_copy(
            self.handle, generation, tick, scene, len(scene), c.byref(written), c.byref(revision),
            c.byref(blur), c.byref(exit_tables), c.byref(sprites)), "copy source scene")
        if index_count.value != PIXELS or palette_count.value != 768 or written.value != PIXELS or sprites.value != 1 or blur.value > 2 or exit_tables.value > 1:
            raise RuntimeError("native scene is incomplete or lacks source sprites")
        flags = sprites.value | (blur.value << 1) | (exit_tables.value << 3)
        return HEADER.pack(b"MCF1", 1, HEADER.size, 320, 200, flags,
                           int(ident["generation"]), generation, tick, sequence, revision.value) + bytes(scene) + bytes(palette)

    def close(self):
        if self.handle.value:
            self.check(self.lib.mc_game_runtime_destroy_v1(self.handle), "destroy")
            self.handle.value = 0


NEUTRAL = {"longitudinal": "coast", "lateral": "neutral", "roll_axis": 0,
           "pitch_axis": 0, "cast_selected": False, "cast_left": False, "cast_right": False}


def validate_command(value):
    if not isinstance(value, dict) or not set(NEUTRAL) <= set(value) or set(value) - set(NEUTRAL) - {"show_objective"}:
        raise ValueError("invalid command fields")
    if value["longitudinal"] not in ("coast", "accelerate", "decelerate") or value["lateral"] not in ("neutral", "negative", "positive"):
        raise ValueError("invalid flight control")
    if any(type(value[k]) is not int or not -64 <= value[k] <= 64 for k in ("roll_axis", "pitch_axis")):
        raise ValueError("invalid flight axis")
    if any(type(value[k]) is not bool for k in ("cast_selected", "cast_left", "cast_right")):
        raise ValueError("invalid casting control")
    if "show_objective" in value and type(value["show_objective"]) is not bool: raise ValueError("invalid objective control")
    return dict(value)


def pose_from_observation(packet):
    # MCPO v5: skip the three length-prefixed identity strings to reach the
    # fixed source player pose. Reject unknown versions instead of guessing.
    if struct.unpack_from("<I", packet, 4)[0] != 5: raise RuntimeError("unsupported native MCPO version")
    offset = 12 + 8 + 8 + 1
    for _ in range(3):
        size = struct.unpack_from("<I", packet, offset)[0]
        offset += 4 + size
        if offset > len(packet): raise RuntimeError("truncated MCPO identity")
    offset += 4 * 8 + 3 * 8
    x, y, z, yaw, display_pitch, effective_pitch, roll = struct.unpack_from("<HHhhhhh", packet, offset)
    return {"x": x, "y": y, "z": z, "yaw": yaw, "display_pitch": display_pitch,
            "effective_pitch": effective_pitch, "roll": roll}


def menu_from_observation(packet):
    offset = 12 + 8 + 8 + 1
    for _ in range(3):
        size = struct.unpack_from("<I", packet, offset)[0]
        offset += 4 + size
        if offset > len(packet): raise RuntimeError("truncated MCPO identity")
    offset += 4 * 8 + 3 * 8 + 14 + 6 + 16
    present = packet[offset]
    if present == 0: return None
    if present != 1: raise RuntimeError("invalid MCPO menu option")
    action, admitted, map_open, spells_open, info_open = struct.unpack_from("<5B", packet, offset + 1)
    return {"action": action, "admitted": bool(admitted), "map_open": bool(map_open),
            "spells_open": bool(spells_open), "player_information_open": bool(info_open)}


def camera_from_observation(packet):
    """Walk MCPO v5's variable scoreboard and notifications to its source camera."""
    if struct.unpack_from('<I', packet, 4)[0] != 5: raise RuntimeError('unsupported native MCPO version')
    offset = 29
    def string_end(position):
        length = struct.unpack_from('<I', packet, position)[0]
        end = position + 4 + length
        if end > len(packet): raise RuntimeError('truncated MCPO string')
        return end
    for _ in range(3): offset = string_end(offset)
    offset += 7 * 8 + 14 + 6 + 16
    present = packet[offset]; offset += 1
    if present == 1: offset += 13
    elif present != 0: raise RuntimeError('invalid MCPO menu option')
    offset += 1  # return-to-title
    count = struct.unpack_from('<I', packet, offset)[0]; offset += 4
    if count > len(packet) // 27: raise RuntimeError('invalid MCPO scoreboard count')
    for _ in range(count):
        offset = string_end(offset + 1) + 2 + 4 + 16
    present = packet[offset]; offset += 1
    if present == 1: offset = string_end(offset)
    elif present != 0: raise RuntimeError('invalid MCPO scoreboard option')
    count = struct.unpack_from('<I', packet, offset)[0]; offset += 4
    if count > len(packet) // 19: raise RuntimeError('invalid MCPO notification count')
    for _ in range(count):
        offset = string_end(offset + 9)
        offset = string_end(offset) + 2
    offset += 3  # wall contact and immobilization
    x, y, z, yaw, pitch, roll, fov = struct.unpack_from('<HHhhhhh', packet, offset)
    return {'x': x, 'y': y, 'z': z, 'yaw': yaw, 'pitch': pitch, 'roll': roll, 'fov': fov}


def replay_package(packages, replay):
    """Resolve the replay's content-addressed campaign package to its owner assets."""
    source = json.loads(replay)
    definition = source.get("definition") or source["state"]["definition"]
    hashes = [row["sha256"] for row in definition["artifacts"] if row["id"] == "campaign.package"]
    if len(hashes) != 1:
        raise ValueError("replay must identify one campaign package")
    matches = []
    for package in packages:
        manifest = json.loads((Path(package["package_dir"]) / "manifest.json").read_bytes())
        if manifest["package_sha256"] == hashes[0]:
            matches.append(package)
    if len(matches) != 1:
        raise ValueError("replay must match one supplied campaign package")
    return matches[0]


class Bridge:
    def __init__(self, library, packages, save_path=None, renderer="indexed", modern=False,
                 replay_path=None, help_cache=None):
        self.library, self.packages = library, packages
        if renderer not in ("indexed", "geometry"): raise ValueError("invalid renderer")
        self.renderer = renderer
        if modern and renderer != 'geometry': raise ValueError('modern observations require geometry renderer')
        self.modern = modern
        self.replay_path = replay_path
        self.help_cache = help_cache
        self.level_id = None
        self.save_path = save_path
        self.has_save = bool(save_path and save_path.is_file())
        self.items = queue.Queue(maxsize=128)
        self.lock = threading.Lock()
        self.publications = deque(maxlen=32 if renderer == "geometry" else 8)
        self.ready = False; self.error = None; self.running = False; self.paused = False
        self.next_sequence = 0; self.last_reply = None; self.last_request = None
        self.last_contact = time.monotonic(); self.stop = threading.Event()
        self.assets = None
        self.media = None; self.media_view = None
        self.phase = "loading"; self.campaign_menu = None; self.asset_revision = 0
        self.can_refresh_assets = False
        self.modern_error = None
        self.thread = threading.Thread(target=self.run, name="original-native-owner", daemon=True)
        self.thread.start()

    def status(self):
        with self.lock:
            return {"ready": self.ready, "error": self.error, "running": self.running,
                    "paused": self.paused, "has_save": self.has_save, "next_sequence": self.next_sequence,
                    "renderer": self.renderer, "modern_error": self.modern_error, "level_id": self.level_id,
                    "phase": self.phase, "campaign_menu": self.campaign_menu, "asset_revision": self.asset_revision,
                    "can_refresh_assets": self.can_refresh_assets, "media": self.media_view}

    def publication(self, sequence=None):
        with self.lock:
            if sequence is None: return self.publications[-1] if self.publications else None
            return next((p for p in self.publications if p[0] == sequence), None)

    def request(self, value):
        done = queue.Queue(maxsize=1)
        self.last_contact = time.monotonic()
        self.items.put((value, done), timeout=1)
        deadline = time.monotonic() + 30
        while True:
            try:
                ok, result = done.get(timeout=.1); break
            except queue.Empty:
                if self.error and not self.thread.is_alive(): raise ValueError(self.error)
                if time.monotonic() >= deadline: raise ValueError("request timeout")
        if not ok: raise ValueError(result)
        return result

    def run(self):
        native = None
        try:
            native = Native(self.library, self.packages)
            menu_cache = self.help_cache or PRIVATE / "generated/mc2-campaign-menu-cache-current"
            language_root = menu_cache / "language"
            if self.help_cache is not None or language_root.is_dir():
                native.language_cache = language_root.resolve(strict=True)
            if self.replay_path is not None:
                replay = self.replay_path.read_bytes()
                native.control("load", replay)
                self.paused = native.identity_value["mode"] == "paused"
            else:
                native.bind_language(native.handle)
                native.identity_value = native.identity()
                replay = native.control("save", None)
            package = None
            world = None
            modern = None
            palette = None
            command, saved, sequence = dict(NEUTRAL), None, 0
            epoch = 0
            menu_state = None
            frame_remainder = 0
            def elapsed_centiseconds():
                nonlocal frame_remainder
                # Source time advances per submitted 30 Hz frame, including
                # exit cinematics whose gameplay tick deliberately freezes.
                elapsed, frame_remainder = divmod(frame_remainder + 100, 30)
                return elapsed
            def update_phase():
                identity = native.identity_value
                menu = identity.get("campaign_menu")
                media = identity.get("frontend_media")
                pending_media = media and (media.get("campaign_return") or {}).get("blocking")
                phase = "media" if pending_media else (menu.get("phase") if menu else identity["mode"])
                if media and self.media:
                    media = dict(media)
                    media['actions'] = [dict(action, audio_index=self.media.audio_index(action.get('asset_path')))
                                        for action in media.get('actions', [])]
                with self.lock:
                    self.phase = phase
                    self.campaign_menu = menu
                    self.media_view = media if pending_media else None
                    if identity.get("session") is not None:
                        self.level_id = identity["active_runtime_flow"]["bound_level_id"]
                    if identity.get("session") is None:
                        self.publications.clear()

            def bind_package():
                nonlocal package, world, modern, palette, menu_state, epoch
                # Resolve from the native save definition, never a client path.
                current = replay_package(self.packages, native.control("save", None))
                assets = OriginalAssets.from_package(current, help_cache=self.help_cache)
                new_world = OriginalWorld(native) if self.renderer == "geometry" else None
                if new_world and world:
                    new_world.revision = world.revision
                new_world_assets = None
                if new_world:
                    from original_world_assets import OriginalWorldAssets
                    new_world_assets = OriginalWorldAssets.from_package(current)
                    if len(new_world_assets.palette) != 768: raise RuntimeError("invalid source palette")
                new_modern = None
                if new_world and self.modern:
                    try: new_modern = ModernWorld(native)
                    except (AttributeError, RuntimeError) as exc: self.modern_error = str(exc)
                package, world, modern = current, new_world, new_modern
                palette = new_world_assets.palette if new_world_assets else None
                epoch += 1; menu_state = None
                with self.lock:
                    self.publications.clear()
                    self.assets = assets
                    self.world_assets = new_world_assets
                    self.level_id = native.identity_value["active_runtime_flow"]["bound_level_id"]
                    self.asset_revision += 1

            def finish_if_ready():
                nonlocal command, frame_remainder
                if not native.finish_if_ready(): return False
                command = dict(NEUTRAL)
                frame_remainder = 0
                if native.identity_value.get("session") is None:
                    self.running = False; self.paused = False
                    update_phase()
                    return True
                # Secret entry constructs its destination session directly,
                # with no intervening campaign menu. Drop the old publication
                # before binding the native destination's package.
                binding_start = time.monotonic()
                with self.lock:
                    self.phase = "loading"
                    self.publications.clear()
                    self.assets = None; self.world_assets = None
                try:
                    native.bind_language(native.handle)
                    native.identity_value = native.identity()
                    bind_package(); update_phase(); publish()
                    now = time.monotonic()
                    self.last_contact = min(now, self.last_contact + (now - binding_start))
                except Exception as exc:
                    self.running = False; self.paused = native.identity_value["mode"] == "paused"
                    update_phase()
                    with self.lock:
                        self.assets = None; self.world_assets = None
                        self.ready = False; self.can_refresh_assets = True
                        self.error = f"{type(exc).__name__}: {exc}"
                return True

            def publish():
                nonlocal sequence, menu_state, modern
                native.presentation_frame()
                sequence += 1
                packet = None if world else native.frame(sequence)
                observation = native.observe()
                outer, gen, tick = native.counters()
                pose = pose_from_observation(observation)
                terrain = None
                modern_packet = None
                if world:
                    camera = camera_from_observation(observation)
                    revision, terrain, rows, row_count = world.read(gen, tick, camera["yaw"], epoch)
                    packet = WORLD_HEADER.pack(b"MCW1", 1, WORLD_HEADER.size, outer, gen, tick,
                                               sequence, revision, row_count, 64, 1, 0) + palette + rows
                    if modern:
                        try: modern_packet = modern.read(outer, gen, tick, sequence, epoch)
                        except (AttributeError, RuntimeError) as exc:
                            self.modern_error = str(exc)
                            modern = None
                menu = menu_from_observation(observation)
                if menu is not None: menu_state = menu
                minimap = read_minimap(native.lib, native.handle.value, gen, tick)
                minimap["terrain"] = None if world else list(native.minimap_terrain)
                minimap["coverage"] = list(minimap["coverage"])
                state = {"sequence": sequence, "runtime_generation": outer, "session_generation": gen,
                         "tick": tick, "asset_revision": self.asset_revision, "player_pose": pose,
                         "source_menu_admission": menu_state,
                         "hud": read_hud(native.lib, native.handle.value, gen, tick),
                         "minimap": minimap, "objective": read_objective(native.lib, native.handle.value, gen, tick),
                         "paused": self.paused}
                if world:
                    state["terrain_revision"] = revision
                    state["palette_source"] = "static-owner-source-rgb8"
                    state["source_camera"] = camera
                world_map = None
                if menu_state and menu_state["map_open"] and not modern:
                    indices, map_palette, map_rows = read_world_map(native.lib, native.handle.value, gen, tick)
                    if not world:
                        world_map = HEADER.pack(b"MCM1", 1, HEADER.size, 320, 200, 0,
                                                outer, gen, tick, sequence, sequence) + indices + map_palette
                    state["world_map"] = map_rows
                with self.lock: self.publications.append((sequence, packet, state, observation, world_map, terrain,
                                                          world.terrain_gzip if world else None, modern_packet))
            update_phase()
            if native.identity_value.get("session") is not None:
                bind_package()
                publish()
            with self.lock: self.ready = True
            deadline = time.monotonic()
            while not self.stop.is_set():
                try: item = self.items.get(timeout=max(0, min(.05, deadline-time.monotonic())) if self.running else .1)
                except queue.Empty: item = None
                if item:
                    value, done = item
                    try:
                        seq = value.get("sequence")
                        if type(seq) is not int or seq < 0: raise ValueError("invalid sequence")
                        if seq == self.next_sequence - 1 and value == self.last_request:
                            done.put((True, self.last_reply)); continue
                        if seq != self.next_sequence: raise ValueError(f"expected sequence {self.next_sequence}")
                        kind = value.get("kind")
                        if kind == "input":
                            next_command = validate_command(value.get("command"))
                            if native.identity_value.get("session") is not None:
                                for key in ("cast_selected", "cast_left", "cast_right"):
                                    next_command[key] |= command[key]
                                command = next_command
                        elif kind == "control":
                            action = value.get("action")
                            if action not in ("start", "pause", "resume", "reset", "save", "load", "map", "menu", "select_spell", "assign_spell", "dismiss_results", "select_mission", "start_mission", "refresh_assets", "media_frame"):
                                raise ValueError("invalid control action")
                            if action == "media_frame":
                                elapsed, media_input, escape = value.get('elapsed_us'), value.get('input'), value.get('escape')
                                if type(elapsed) is not int or not 0 <= elapsed <= 5_000_000 or type(media_input) is not bool or type(escape) is not bool:
                                    raise ValueError('invalid frontend media frame')
                                if self.phase != 'media': raise ValueError('no campaign media is active')
                                if self.media is None: self.media = OriginalMedia(MEDIA_CACHE)
                                if not native.media_installed:
                                    native.media_command({'operation': 'install', 'catalog': self.media.catalog,
                                                          'startup': False, 'audio_available': True})
                                    native.media_installed = True
                                native.media_command({'operation': 'frame', 'elapsed_us': elapsed,
                                                      'input': media_input, 'escape': escape})
                                update_phase()
                            elif action == "refresh_assets":
                                if native.identity_value.get("session") is None: raise ValueError("no active mission")
                                with self.lock: self.phase = "loading"
                                try:
                                    native.bind_language(native.handle)
                                    native.identity_value = native.identity()
                                    bind_package(); update_phase(); publish()
                                except Exception as exc:
                                    update_phase()
                                    with self.lock:
                                        self.assets = None; self.world_assets = None
                                        self.ready = False; self.can_refresh_assets = True
                                        self.error = f"{type(exc).__name__}: {exc}"
                                    raise
                                with self.lock:
                                    self.ready = True; self.error = None; self.can_refresh_assets = False
                            elif action in ("dismiss_results", "select_mission", "start_mission"):
                                level_id = value.get("level_id")
                                if action == "select_mission" and (not isinstance(level_id, str) or not 1 <= len(level_id) <= 160):
                                    raise ValueError("invalid level id")
                                if action != "start_mission" or native.identity_value["mode"] != "loading":
                                    native.campaign_menu(action, level_id if action == "select_mission" else None)
                                    update_phase()
                                if action == "start_mission":
                                    native.open_pending()
                                    self.running = False; self.paused = False; command = dict(NEUTRAL)
                                    frame_remainder = 0
                                    try:
                                        native.bind_language(native.handle)
                                        native.identity_value = native.identity()
                                        bind_package(); update_phase(); publish()
                                    except Exception as exc:
                                        update_phase()
                                        with self.lock:
                                            self.assets = None; self.world_assets = None
                                            self.ready = False; self.can_refresh_assets = True
                                            self.error = f"{type(exc).__name__}: {exc}"
                                        raise
                            elif action == "start":
                                if native.identity_value.get("session") is None: raise ValueError("no active mission")
                                if not self.ready: raise ValueError("mission assets are unavailable; retry refresh_assets")
                                if self.paused: native.control("resume", saved); self.paused = False
                                self.running = True; deadline = time.monotonic()
                                publish()
                            elif action == "pause":
                                if not self.paused: native.control("pause", saved); self.paused = True
                                self.running = False; command = dict(NEUTRAL); publish()
                            elif action == "resume":
                                if not self.ready: raise ValueError("mission assets are unavailable; retry refresh_assets")
                                if self.paused: native.control("resume", saved); self.paused = False
                                self.running = True; deadline = time.monotonic()
                                publish()
                            elif action in ("map", "menu", "select_spell", "assign_spell"):
                                if not self.running or self.paused: raise ValueError("game must be playing")
                                if action == "map": extra = {"abandon_action": "toggle_gameplay_map"}
                                elif action == "menu":
                                    menu_action = value.get("menu_action")
                                    if menu_action not in ("open_spell_menu", "close_spell_menu", "toggle_gameplay_map", "close_gameplay_menus"):
                                        raise ValueError("invalid menu action")
                                    extra = {"abandon_action": menu_action}
                                else:
                                    spell = value.get("spell_id")
                                    if not isinstance(spell, str) or not 1 <= len(spell) <= 160:
                                        raise ValueError("invalid spell id")
                                    if action == "select_spell": extra = {"select_spell": spell}
                                    else:
                                        tier, hand = value.get("tier"), value.get("hand")
                                        if type(tier) is not int or not 0 <= tier <= 2 or hand not in ("left", "right"):
                                            raise ValueError("invalid spell assignment")
                                        extra = {"assign_spell": {"spell_id": spell, "tier": tier, "hand": hand}}
                                native.step({**NEUTRAL, **extra, "elapsed_centiseconds": elapsed_centiseconds()})
                                if not finish_if_ready(): publish()
                            else:
                                if action == "load" and saved is None and self.save_path and self.save_path.is_file():
                                    saved = self.save_path.read_bytes()
                                if action == "reset" and self.paused:
                                    native.control("resume", saved)
                                    saved = native.control("reset", saved)
                                    native.control("pause", saved)
                                else:
                                    saved = native.control(action, saved)
                                if action == "save" and self.save_path:
                                    self.save_path.parent.mkdir(parents=True, exist_ok=True)
                                    with tempfile.NamedTemporaryFile(dir=self.save_path.parent, prefix=".original-save-", delete=False) as output:
                                        temporary = Path(output.name)
                                        output.write(saved); output.flush(); os.fsync(output.fileno())
                                    os.replace(temporary, self.save_path)
                                if action == "save": self.has_save = True
                                if action in ("reset", "load"):
                                    epoch += 1
                                    self.paused = native.identity_value["mode"] == "paused"
                                    self.running = self.running and not self.paused and native.identity_value.get("session") is not None
                                    command = dict(NEUTRAL)
                                    frame_remainder = 0
                                    menu_state = None
                                    if native.identity_value.get("session") is not None:
                                        with self.lock:
                                            self.phase = "loading"
                                            self.publications.clear()
                                        try: bind_package()
                                        except Exception as exc:
                                            update_phase()
                                            with self.lock:
                                                self.assets = None; self.world_assets = None
                                                self.ready = False; self.can_refresh_assets = True
                                                self.error = f"{type(exc).__name__}: {exc}"
                                            raise
                                    update_phase()
                                if native.identity_value.get("session") is not None: publish()
                        else: raise ValueError("invalid request kind")
                        self.next_sequence += 1
                        result = {"ok": True, "next_sequence": self.next_sequence,
                                  "running": self.running, "paused": self.paused,
                                  "phase": self.phase, "campaign_menu": self.campaign_menu,
                                  "level_id": self.level_id, "asset_revision": self.asset_revision,
                                  "media": self.media_view}
                        self.last_request, self.last_reply = value, result
                        done.put((True, result))
                    except Exception as exc: done.put((False, str(exc)))
                now = time.monotonic()
                if self.running and now - self.last_contact > 1.5:
                    native.control("pause", saved); self.paused = True; self.running = False
                    command = dict(NEUTRAL); publish()
                if self.running and now >= deadline:
                    native.step({**command, "elapsed_centiseconds": elapsed_centiseconds()})
                    # Each submitted cast request survives until one native step.
                    command = {**command, "cast_selected": False, "cast_left": False, "cast_right": False}
                    if not finish_if_ready(): publish()
                    deadline += 1/30
                    if deadline < time.monotonic() - .25: deadline = time.monotonic()
        except Exception as exc:
            with self.lock: self.error = f"{type(exc).__name__}: {exc}"; self.ready = False; self.running = False
        finally:
            if native is not None: native.close()

    def close(self):
        self.stop.set(); self.thread.join(timeout=30)


def host_alias(value):
    """Accept one explicit DNS/IPv4 host and port, never a pattern or URL."""
    match = re.fullmatch(r"([A-Za-z0-9](?:[A-Za-z0-9.-]*[A-Za-z0-9])?):([0-9]{1,5})", value)
    if not match or not 1 <= int(match.group(2)) <= 65535:
        raise argparse.ArgumentTypeError("host alias must be an explicit hostname:port")
    hostname = match.group(1)
    if any(not label or len(label) > 63 or label.startswith("-") or label.endswith("-")
           for label in hostname.split(".")) or len(hostname) > 253:
        raise argparse.ArgumentTypeError("host alias has an invalid hostname")
    return hostname.lower() + ":" + str(int(match.group(2)))


class Server(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, address, bridge, web_root, allowed_hosts=()):
        super().__init__(address, Handler)
        self.bridge, self.web_root = bridge, web_root.resolve() if web_root else None
        self.secret = secrets.token_bytes(32)
        self.controller = None
        self.controller_seen = 0.0
        self.controller_lock = threading.Lock()
        host, _ = address
        port = self.server_port
        self.allowed_hosts = {f"{host}:{port}", *(host_alias(alias) for alias in allowed_hosts)}
        if host in ("127.0.0.1", "localhost"):
            self.allowed_hosts.update({f"127.0.0.1:{port}", f"localhost:{port}",
                                       "127.0.0.1:8878", "localhost:8878"})


class Handler(BaseHTTPRequestHandler):
    server: Server

    def log_message(self, fmt, *args): pass

    def send_data(self, status, data, content_type="application/json", cookie=None, content_encoding=None):
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        if content_encoding: self.send_header("Content-Encoding", content_encoding)
        if cookie: self.send_header("Set-Cookie", f"mc_original={cookie}; HttpOnly; SameSite=Strict; Path=/")
        self.end_headers()
        try: self.wfile.write(data)
        except (BrokenPipeError, ConnectionResetError): pass

    def send_json(self, status, data, cookie=None):
        self.send_data(status, json.dumps(data, separators=(",", ":")).encode(), cookie=cookie)

    def host_allowed(self):
        return self.headers.get("Host", "") in self.server.allowed_hosts

    def controller_cookie(self):
        jar = cookies.SimpleCookie()
        try: jar.load(self.headers.get("Cookie", ""))
        except cookies.CookieError: return None
        if "mc_original" not in jar: return None
        value = jar["mc_original"].value
        nonce, separator, signature = value.partition(".")
        if not separator or len(nonce) > 64: return None
        expected = hmac.new(self.server.secret, nonce.encode(), hashlib.sha256).hexdigest()
        return value if secrets.compare_digest(signature, expected) else None

    def modern_asset(self, path):
        parts = path.split('/')
        if len(parts) != 5 or parts[:3] != ['', 'original-api', 'modern-assets']:
            return self.send_json(404, {'error': 'not found'})
        family, name = parts[3:]
        if family not in ('characters', 'world') or not re.fullmatch(r'[A-Za-z0-9_.-]+', name) or name in ('.', '..'):
            return self.send_json(404, {'error': 'not found'})
        root = (MODERN_ASSETS / family).resolve()
        if not root.is_relative_to(MODERN_ASSETS.resolve()): return self.send_json(404, {'error': 'not found'})
        manifest_path = root / 'manifest.json'
        try:
            with os.fdopen(os.open(manifest_path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), 'rb') as source:
                info = os.fstat(source.fileno())
                if not stat.S_ISREG(info.st_mode) or info.st_size > 1_000_000: raise ValueError('invalid manifest')
                raw = source.read(1_000_001)
                if len(raw) > 1_000_000: raise ValueError('invalid manifest')
            manifest = json.loads(raw)
            if manifest.get('schema') != f'mc-browser-modern-{family}/v1' or not isinstance(manifest.get('files'), dict):
                raise ValueError('invalid manifest')
            if name == 'manifest.json': return self.send_data(200, raw, 'application/json')
            entry = manifest['files'].get(name) or manifest['files'].get(f'{family}/{name}')
            if not isinstance(entry, dict): return self.send_json(404, {'error': 'asset not allowlisted'})
            expected_size, expected_hash = entry.get('bytes'), entry.get('sha256')
            if type(expected_size) is not int or not 0 <= expected_size <= 100_000_000 or not isinstance(expected_hash, str) or not re.fullmatch(r'[0-9a-f]{64}', expected_hash):
                raise ValueError('invalid asset receipt')
            asset = root / name
            with os.fdopen(os.open(asset, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), 'rb') as source:
                info = os.fstat(source.fileno())
                if not stat.S_ISREG(info.st_mode) or info.st_size != expected_size: raise ValueError('asset size mismatch')
                digest = hashlib.sha256()
                while chunk := source.read(1024 * 1024): digest.update(chunk)
                if digest.hexdigest() != expected_hash: raise ValueError('asset hash mismatch')
                source.seek(0)
                self.send_response(200)
                self.send_header('Content-Type', mimetypes.guess_type(name)[0] or 'application/octet-stream')
                self.send_header('Content-Length', str(expected_size))
                self.send_header('Cache-Control', 'no-store')
                self.send_header('X-Content-Type-Options', 'nosniff')
                self.end_headers()
                try:
                    while chunk := source.read(1024 * 1024): self.wfile.write(chunk)
                except (BrokenPipeError, ConnectionResetError): pass
        except (OSError, ValueError, TypeError, json.JSONDecodeError):
            return self.send_json(503, {'error': 'modern asset pack unavailable or invalid'})

    def do_GET(self):
        if not self.host_allowed(): return self.send_json(403, {"error": "invalid host"})
        target = urlsplit(self.path); bridge = self.server.bridge
        if target.path.startswith('/original-api/modern-assets/'):
            return self.modern_asset(target.path)
        if target.path == "/original-api/status":
            token = self.controller_cookie()
            if not token:
                nonce = secrets.token_urlsafe(24)
                token = nonce + "." + hmac.new(self.server.secret, nonce.encode(), hashlib.sha256).hexdigest()
            return self.send_json(200, bridge.status(), cookie=token)
        if target.path in ('/original-api/media/metadata', '/original-api/media/frame', '/original-api/media/audio', '/original-api/media/credits'):
            media = bridge.media
            if not media: return self.send_json(503, {'error': 'media is not bound'})
            try:
                params = parse_qs(target.query)
                if target.path.endswith('/metadata'): return self.send_json(200, media.metadata)
                if target.path.endswith('/audio'):
                    index = int(params['index'][0])
                    return self.send_data(200, media.audio_bytes(index), 'audio/wav')
                if target.path.endswith('/credits'):
                    with bridge.lock: view = bridge.media_view
                    if not view or (view.get('campaign_return') or {}).get('phase') != 'credits':
                        return self.send_json(410, {'error': 'credits are not active'})
                    return self.send_data(200, media.credits_frame(), 'application/octet-stream')
                epoch, frame = int(params['epoch'][0]), int(params['frame'][0])
                with bridge.lock: view = bridge.media_view
                if not view or (view['epoch'], view['frame_index']) != (epoch, frame):
                    return self.send_json(410, {'error': 'media frame is stale'})
                return self.send_data(200, media.frame(view), 'application/octet-stream')
            except (KeyError, ValueError, IndexError, OSError) as exc:
                return self.send_json(503, {'error': str(exc)})
        if target.path == "/original-api/assets/manifest":
            assets = bridge.assets
            if not assets: return self.send_json(503, {"error": "loading"})
            return self.send_json(200, assets.manifest)
        if target.path == "/original-api/assets/blob":
            assets = bridge.assets
            if not assets: return self.send_json(503, {"error": "loading"})
            return self.send_data(200, assets.blob, "application/octet-stream")
        if target.path in ("/original-api/world-assets/manifest", "/original-api/world-assets/blob"):
            assets = getattr(bridge, "world_assets", None)
            if not assets: return self.send_json(503, {"error": "world assets unavailable"})
            if target.path.endswith("/manifest"): return self.send_json(200, assets.manifest)
            return self.send_data(200, assets.blob, "application/octet-stream")
        if target.path in ("/original-api/frame", "/original-api/world-frame", "/original-api/modern-frame", "/original-api/terrain", "/original-api/state", "/original-api/observation", "/original-api/world-map"):
            params = parse_qs(target.query)
            try:
                requested = int(params["sequence"][0]) if "sequence" in params else None
                if requested is not None and requested < 0: raise ValueError()
            except (ValueError, IndexError): return self.send_json(400, {"error": "invalid publication sequence"})
            publication = bridge.publication(requested)
            if publication is None:
                if requested is None and bridge.ready and bridge.phase not in ("playing", "paused"):
                    return self.send_data(204, b"", "application/octet-stream")
                return self.send_json(410 if requested is not None else 503, {"error": "publication unavailable"})
            if target.path.endswith('/modern-frame'):
                if bridge.renderer != 'geometry': return self.send_json(404, {'error': 'renderer unavailable'})
                if publication[7] is None: return self.send_json(503, {'error': bridge.modern_error or 'modern observation unavailable'})
                return self.send_data(200, publication[7], 'application/octet-stream')
            if target.path in ("/original-api/frame", "/original-api/world-frame"):
                expected = "geometry" if target.path.endswith("/world-frame") else "indexed"
                if bridge.renderer != expected: return self.send_json(404, {"error": "renderer unavailable"})
                return self.send_data(200, publication[1], "application/octet-stream")
            if target.path.endswith("/terrain"):
                if publication[5] is None: return self.send_json(404, {"error": "terrain unavailable"})
                if any(part.strip().lower() == 'gzip' for part in self.headers.get('Accept-Encoding', '').split(',')):
                    return self.send_data(200, publication[6], "application/octet-stream", content_encoding="gzip")
                return self.send_data(200, publication[5], "application/octet-stream")
            if target.path.endswith("/observation"): return self.send_data(200, publication[3], "application/octet-stream")
            if target.path.endswith("/world-map"):
                if publication[4] is None: return self.send_json(404, {"error": "world map is closed"})
                return self.send_data(200, publication[4], "application/octet-stream")
            return self.send_json(200, publication[2])
        if self.server.web_root and (target.path in ("/", "/original.html") or target.path.startswith("/assets/")):
            name = "original.html" if target.path == "/" else target.path.lstrip("/")
            path = (self.server.web_root / name).resolve()
            if path.is_relative_to(self.server.web_root) and path.is_file():
                return self.send_data(200, path.read_bytes(), mimetypes.guess_type(path.name)[0] or "application/octet-stream")
        self.send_json(404, {"error": "not found"})

    def do_POST(self):
        if not self.host_allowed(): return self.send_json(403, {"error": "invalid host"})
        origin = urlsplit(self.headers.get("Origin", ""))
        if origin.scheme not in ("http", "https") or origin.netloc != self.headers.get("Host"):
            return self.send_json(403, {"error": "same-origin request required"})
        token = self.controller_cookie()
        if not token:
            return self.send_json(403, {"error": "controller cookie required"})
        with self.server.controller_lock:
            now = time.monotonic()
            if self.server.controller not in (None, token):
                if now - self.server.controller_seen <= 1.5 or self.server.bridge.running:
                    return self.send_json(409, {"error": "another controller is active"})
            self.server.controller = token
            self.server.controller_seen = now
        if self.path not in ("/original-api/input", "/original-api/control"):
            return self.send_json(404, {"error": "not found"})
        try:
            length = int(self.headers.get("Content-Length", "0"))
            if not 0 < length <= 4096: raise ValueError("invalid request length")
            data = json.loads(self.rfile.read(length))
            if not isinstance(data, dict): raise ValueError("request must be an object")
            kind = "input" if self.path.endswith("/input") else "control"
            control_fields = {"menu": {"menu_action"}, "select_spell": {"spell_id"},
                              "select_mission": {"level_id"},
                              "media_frame": {"elapsed_us", "input", "escape"},
                              "assign_spell": {"spell_id", "tier", "hand"}}
            expected = ({"sequence", "command"} if kind == "input" else
                        {"sequence", "action"} | control_fields.get(data.get("action"), set()))
            if set(data) != expected: raise ValueError("invalid request fields")
            data["kind"] = kind
            return self.send_json(200, self.server.bridge.request(data))
        except (ValueError, json.JSONDecodeError, queue.Full, queue.Empty) as exc:
            return self.send_json(400, {"error": str(exc) or "request timeout"})


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--library", type=Path, default=LIBRARY)
    parser.add_argument("--packages", type=Path, default=PACKAGES)
    parser.add_argument("--replay", type=Path, help="start at this read-only campaign replay")
    parser.add_argument("--help-cache", type=Path, help="use this owner-local gameplay help cache")
    parser.add_argument("--probe-frame", type=Path)
    parser.add_argument("--renderer", choices=("indexed", "geometry"), default="indexed")
    parser.add_argument("--modern", action="store_true", help="publish native Modern observations with geometry")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8881)
    parser.add_argument("--allow-host", type=host_alias, action="append", default=[],
                        help="allow an additional exact hostname:port (repeatable)")
    parser.add_argument("--web-root", type=Path)
    parser.add_argument("--save", type=Path, default=PRIVATE / "generated/original-web-20261116/save.json")
    args = parser.parse_args()
    resources = json.loads(args.packages.read_text())
    packages = resources["packages"] if isinstance(resources, dict) else resources
    if not isinstance(packages, list) or not packages:
        parser.error("the campaign package list is empty or invalid")
    if args.replay is not None and args.save.resolve() == args.replay.resolve():
        parser.error("--replay and the writable --save slot must be different files")
    if args.probe_frame:
        runtime = Native(args.library, packages)
        try:
            frame = runtime.frame(1)
            args.probe_frame.parent.mkdir(parents=True, exist_ok=True)
            args.probe_frame.write_bytes(frame)
            print(json.dumps({"identity": runtime.identity_value, "frame_bytes": len(frame),
                              "frame_sha256": hashlib.sha256(frame).hexdigest(), "file": str(args.probe_frame)}))
        finally: runtime.close()
        return
    bridge = Bridge(args.library, packages, args.save, args.renderer, args.modern, args.replay, args.help_cache)
    server = Server((args.host, args.port), bridge, args.web_root, args.allow_host)
    print(f"Original native bridge: http://{args.host}:{args.port}", flush=True)
    try: server.serve_forever(poll_interval=.2)
    except KeyboardInterrupt: pass
    finally: server.server_close(); bridge.close()


if __name__ == "__main__":
    main()
