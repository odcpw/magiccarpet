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

ROOT = Path(__file__).resolve().parents[4]
PRIVATE = ROOT / "research/private"
LIBRARY = PRIVATE / "remc2-unity-native/Builds/Original/Builds/Linux/MagicCarpetNative_Data/Plugins/x86_64/libsim_capi.so"
PACKAGES = PRIVATE / "generated/mc-voxel-combat-20261107/native-mana-scenario-01.json"
PIXELS = 320 * 200
HEADER = struct.Struct("<4sHHHHIQQQQQ")
WORLD_HEADER = struct.Struct("<4sHHQQQQQIIII")
SOURCE_PALETTE = PRIVATE / "generated/settlement-20261105-integration/owner-closed-v6/environment-complete/cache-level1/palette/source.rgb8"
MODERN_ASSETS = PRIVATE / "generated/browser-level1-assets"


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
            "mc_native_campaign_day_indexed_frame_render": [u64, u64, u64, ptr, u32],
            "mc_native_campaign_presentation_frame_copy": [u64, ptr, ptr, u32, ptr, u32, ptr, u32],
            "mc_native_campaign_day_indexed_frame_copy": [u64, u64, u64, ptr, u32, ptr, u32, ptr, ptr],
            "mc_native_campaign_scene_frame_copy": [u64, u64, u64, ptr, u32, ptr, ptr, ptr, ptr, ptr],
            "mc_game_session_step_json_v1": [u64, u64, u64, ptr, u32],
            "mc_game_session_reset_v1": [u64, u64, u64],
            "mc_game_runtime_pause_v1": [u64, u64],
            "mc_game_runtime_resume_v1": [u64, u64],
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

    def counters(self):
        ident = self.identity_value
        return int(ident["generation"]), int(ident["session"]["generation"]), int(ident["session"]["tick"])

    def step(self, command):
        _, generation, tick = self.counters()
        payload = json.dumps(command, separators=(",", ":")).encode()
        self.check(self.lib.mc_game_session_step_json_v1(self.handle, generation, tick, payload, len(payload)), "step")
        self.identity_value["session"]["tick"] = str(tick + 1)

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
        outer, generation, tick = self.counters()
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
            except BaseException:
                self.check(self.lib.mc_game_runtime_destroy_v1(new_handle), "destroy failed load runtime")
                raise
            old = self.handle; self.handle = new_handle
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


class Bridge:
    def __init__(self, library, packages, save_path=None, renderer="indexed", modern=False):
        self.library, self.packages = library, packages
        if renderer not in ("indexed", "geometry"): raise ValueError("invalid renderer")
        self.renderer = renderer
        if modern and renderer != 'geometry': raise ValueError('modern observations require geometry renderer')
        self.modern = modern
        self.save_path = save_path
        self.has_save = bool(save_path and save_path.is_file())
        self.items = queue.Queue(maxsize=128)
        self.lock = threading.Lock()
        self.publications = deque(maxlen=32 if renderer == "geometry" else 8)
        self.ready = False; self.error = None; self.running = False; self.paused = False
        self.next_sequence = 0; self.last_reply = None; self.last_request = None
        self.last_contact = time.monotonic(); self.stop = threading.Event()
        self.assets = None
        self.modern_error = None
        self.thread = threading.Thread(target=self.run, name="original-native-owner", daemon=True)
        self.thread.start()

    def status(self):
        with self.lock:
            return {"ready": self.ready, "error": self.error, "running": self.running,
                    "paused": self.paused, "has_save": self.has_save, "next_sequence": self.next_sequence,
                    "renderer": self.renderer, "modern_error": self.modern_error}

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
                if self.error: raise ValueError(self.error)
                if time.monotonic() >= deadline: raise ValueError("request timeout")
        if not ok: raise ValueError(result)
        return result

    def run(self):
        native = None
        try:
            native = Native(self.library, self.packages)
            self.assets = OriginalAssets()
            world = OriginalWorld(native) if self.renderer == "geometry" else None
            modern = None
            if world and self.modern:
                try: modern = ModernWorld(native)
                except (AttributeError, RuntimeError) as exc: self.modern_error = str(exc)
            if world:
                from original_world_assets import OriginalWorldAssets
                self.world_assets = OriginalWorldAssets()
                palette = SOURCE_PALETTE.read_bytes()
                if len(palette) != 768: raise RuntimeError("invalid source palette")
            command, saved, sequence = dict(NEUTRAL), None, 0
            epoch = 0
            menu_state = None
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
                         "tick": tick, "player_pose": pose,
                         "source_menu_admission": menu_state,
                         "hud": read_hud(native.lib, native.handle.value, gen, tick),
                         "minimap": minimap, "objective": read_objective(native.lib, native.handle.value, gen, tick),
                         "paused": self.paused}
                if world:
                    state["terrain_revision"] = revision
                    state["palette_source"] = "static-owner-source-rgb8"
                    state["source_camera"] = camera
                world_map = None
                if not world and menu_state and menu_state["map_open"]:
                    indices, map_palette, map_rows = read_world_map(native.lib, native.handle.value, gen, tick)
                    world_map = HEADER.pack(b"MCM1", 1, HEADER.size, 320, 200, 0,
                                            outer, gen, tick, sequence, sequence) + indices + map_palette
                    state["world_map"] = map_rows
                with self.lock: self.publications.append((sequence, packet, state, observation, world_map, terrain,
                                                          world.terrain_gzip if world else None, modern_packet))
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
                            for key in ("cast_selected", "cast_left", "cast_right"):
                                next_command[key] |= command[key]
                            command = next_command
                        elif kind == "control":
                            action = value.get("action")
                            if action not in ("start", "pause", "resume", "reset", "save", "load", "map", "menu", "select_spell", "assign_spell"):
                                raise ValueError("invalid control action")
                            if action == "start":
                                if self.paused: native.control("resume", saved); self.paused = False
                                self.running = True; deadline = time.monotonic()
                                publish()
                            elif action == "pause":
                                if not self.paused: native.control("pause", saved); self.paused = True
                                self.running = False; command = dict(NEUTRAL); publish()
                            elif action == "resume":
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
                                native.step({**NEUTRAL, **extra, "elapsed_centiseconds": 3})
                                publish()
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
                                    self.running = self.running and not self.paused
                                    command = dict(NEUTRAL)
                                    menu_state = None
                                publish()
                        else: raise ValueError("invalid request kind")
                        self.next_sequence += 1
                        result = {"ok": True, "next_sequence": self.next_sequence,
                                  "running": self.running, "paused": self.paused}
                        self.last_request, self.last_reply = value, result
                        done.put((True, result))
                    except Exception as exc: done.put((False, str(exc)))
                now = time.monotonic()
                if self.running and now - self.last_contact > 1.5:
                    native.control("pause", saved); self.paused = True; self.running = False
                    command = dict(NEUTRAL); publish()
                if self.running and now >= deadline:
                    tick = native.counters()[2]
                    native.step({**command, "elapsed_centiseconds": 3 if tick % 3 else 4})
                    # Each submitted cast request survives until one native step.
                    command = {**command, "cast_selected": False, "cast_left": False, "cast_right": False}
                    publish()
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
        if target.path == "/original-api/assets/manifest":
            if not bridge.assets: return self.send_json(503, {"error": "loading"})
            return self.send_json(200, bridge.assets.manifest)
        if target.path == "/original-api/assets/blob":
            if not bridge.assets: return self.send_json(503, {"error": "loading"})
            return self.send_data(200, bridge.assets.blob, "application/octet-stream")
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
    packages = json.loads(args.packages.read_text())["packages"]
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
    bridge = Bridge(args.library, packages, args.save, args.renderer, args.modern)
    server = Server((args.host, args.port), bridge, args.web_root, args.allow_host)
    print(f"Original native bridge: http://{args.host}:{args.port}", flush=True)
    try: server.serve_forever(poll_interval=.2)
    except KeyboardInterrupt: pass
    finally: server.server_close(); bridge.close()


if __name__ == "__main__":
    main()
