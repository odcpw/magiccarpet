#!/usr/bin/env python3
"""Check an earned voxel-enabled Archer-to-Castle mana path through the native ABI.

The scenario names private owner resources and three actual semantic replays.
Only the recorded public step commands are submitted; the checker does not
place actors, grant spells, or modify health, terrain, or Castle state.
Stdout is one versioned JSON result, including typed failures.
"""

from __future__ import annotations

import argparse
import ctypes as c
import json
import sys
from pathlib import Path

from verify_neutral_game_session import NeutralClient, NeutralNativeError, sha256_file

CONTRACT = "magiccarpet-voxel-mana-e2e/v1"
SCENARIO_CONTRACT = "magiccarpet-voxel-mana-scenario/v1"
U8, U16, U32, U64 = c.c_uint8, c.c_uint16, c.c_uint32, c.c_uint64
I16, I32, PTR = c.c_int16, c.c_int32, c.c_void_p


class CheckError(Exception):
    def __init__(self, code: str, message: str, stage: str = "input") -> None:
        super().__init__(message)
        self.code, self.stage = code, stage


def require(condition: bool, message: str, stage: str = "verification") -> None:
    if not condition:
        raise CheckError("mismatch", message, stage)


class Parser(argparse.ArgumentParser):
    def error(self, message: str) -> None:
        raise CheckError("invalid_arguments", message)


class EconomyEvent(c.Structure):
    _fields_ = [
        ("abi", U32), ("size", U32), ("generation", U64), ("ordinal", U32),
        ("pass_no", U8), ("family", U8), ("code", U8), ("flags", U8),
        ("source_slot", U16), ("reserved0", U16), ("primary", U32),
        ("secondary", U32), ("entity0", U16), ("entity1", U16),
        ("value0", I32), ("value1", I32), ("reserved", U32 * 4),
    ]


class VoxelDeath(c.Structure):
    _fields_ = [
        ("abi", U32), ("size", U32), ("world_generation", U64),
        ("sequence", U64), ("slot_generation", U32), ("slot", U16),
        ("source_id", U16),
    ]


class Sphere(c.Structure):
    _fields_ = [
        ("abi", U32), ("size", U32), ("sphere_id", U32), ("slot", U16),
        ("owner", U16), ("authority", U8), ("active", U8), ("class_id", U8),
        ("model", U8), ("action", U8), ("phase", U8), ("render_mode", U8),
        ("reserved0", U8), ("x", U16), ("y", U16), ("z", I16), ("yaw", I16),
        ("vx", I16), ("vy", I16), ("vz", I16), ("render_parameter", U16),
        ("base_tmap", U16), ("logical_height", U16), ("mana", I32),
        ("carrier", U32), ("reserved", U32 * 3),
    ]


class Batch(c.Structure):
    _fields_ = [
        ("abi", U32), ("size", U32), ("generation", U64), ("tick", U64),
        ("live_hash", U64), ("start", U32), ("written", U32), ("total", U32),
        ("more", U8), ("pad", U8 * 3), ("reserved", U32 * 4),
    ]


class Entity(c.Structure):
    _fields_ = [
        ("abi", U32), ("size", U32), ("slot", U16), ("source_id", U16),
        ("next", U16), ("parameter", U16), ("authority", U8), ("active", U8),
        ("class_id", U8), ("model", U8), ("action", U8), ("phase", U8),
        ("x", U16), ("y", U16), ("z", I16), ("yaw", I16), ("pitch", I16),
        ("fov", I16), ("speed", I16), ("reserved", U32 * 4),
    ]


class Player(c.Structure):
    _fields_ = [
        ("abi", U32), ("size", U32), ("slot", U8), ("is_ai", U8),
        ("active", U8), ("pad0", U8), ("entity_index", U16),
        ("active_player_index", I16), ("turn", I32), ("left", I16),
        ("right", I16), ("left_sub", c.c_int8), ("right_sub", c.c_int8),
        ("selected", U8), ("selected_sub", c.c_int8), ("pad1", U8 * 2),
        ("reserved", U32 * 4),
    ]


class Presentation(c.Structure):
    _fields_ = [
        ("abi", U32), ("size", U32), ("generation", U64), ("tick", U64),
        ("slot", U16), ("source_id", U16), ("render_parameter", U16),
        ("base_tmap", U16), ("selected_tmap", U16), ("logical_height", U16),
        ("render_mode", U8), ("sector", U8), ("mirrored", U8), ("reserved0", U8),
        ("x", U16), ("y", U16), ("z", I16), ("yaw", I16), ("pixel_mode", U8),
        ("owner_color", U8), ("brightness", U16), ("draw_order", U32),
        ("control_flags", U32), ("depth", I16), ("reserved", U16),
    ]


def fields(record: c.Structure) -> dict:
    return {
        name: list(value) if isinstance(value, c.Array) else value
        for name, _ in record._fields_
        if (value := getattr(record, name)) is not None
    }


class Runtime:
    def __init__(self, library: Path, packages: list[dict], require_world: bool = False) -> None:
        self.client = NeutralClient(library, open_session=False)
        self.lib = self.client.lib
        self.handle = U64()
        require(c.sizeof(Presentation) == 64 and Presentation.generation.offset == 8
                and Presentation.tick.offset == 16 and Presentation.slot.offset == 24
                and Presentation.control_flags.offset == 56 and Presentation.depth.offset == 60,
                "presentation ABI layout differs", "abi")
        self._bind("mc_game_runtime_create_v1", [PTR])
        self._bind("mc_game_runtime_destroy_v1", [U64])
        self._bind("mc_game_runtime_load_replay_json_v1", [U64, U64, PTR, U32, PTR, U32])
        self._bind("mc_game_runtime_voxel_enable_json_v1", [U64, U64, U64, PTR, U32])
        self._bind("mc_native_campaign_voxel_deaths_copy", [U64, U64, U64, U64, PTR, U32, U32, PTR])
        self._bind("mc_native_campaign_economy_events", [U64, PTR, U32, U32, PTR])
        self._bind("mc_native_campaign_mana_spheres_copy", [U64, U64, U32, PTR, U32, U32, PTR, U32])
        self._bind("mc_native_campaign_entity_count", [U64, PTR])
        self._bind("mc_native_campaign_entities_copy", [U64, U32, PTR, U32, U32, PTR])
        self._bind("mc_native_campaign_players_copy", [U64, U32, PTR, U32, U32, PTR])
        if require_world:
            args = [U64, U64, U64, I16, PTR, U32, U32, PTR]
            try:
                self._bind("mc_native_campaign_world_entity_presentations_copy", args)
                self._bind("mc_native_campaign_entity_presentations_copy", args)
            except AttributeError as error:
                raise CheckError("unsupported_abi", f"required presentation query missing: {error}", "world_query") from error
        self.lib.mc_native_last_error_length.argtypes = [PTR]
        self.lib.mc_native_last_error_copy.argtypes = [PTR, U32, PTR]
        self.request = json.dumps({
            "request_contract": "carpet-flight-installed-package-set-bootstrap/v1",
            "packages": packages,
        }, separators=(",", ":")).encode()
        self.check(self.lib.mc_game_runtime_create_v1(c.byref(self.handle)), "create")
        self.client.handle = self.handle

    def _bind(self, name: str, args: list) -> None:
        fn = getattr(self.lib, name)
        fn.argtypes, fn.restype = args, c.c_int32

    def check(self, status: int, stage: str) -> None:
        if status == 0:
            return
        length = U32()
        self.lib.mc_native_last_error_length(c.byref(length))
        buf = c.create_string_buffer(length.value + 1)
        self.lib.mc_native_last_error_copy(buf, len(buf), c.byref(length))
        raise CheckError("native_call", f"status {status}: {buf.value.decode(errors='replace')}", stage)

    def close(self) -> None:
        if self.handle.value:
            self.check(self.lib.mc_game_runtime_destroy_v1(self.handle), "destroy")
            self.handle.value = 0

    def load(self, data: bytes) -> dict:
        self.check(self.lib.mc_game_runtime_load_replay_json_v1(
            self.handle, 1, self.request, len(self.request), data, len(data)), "load_replay")
        return self.client.snapshot()

    def enable(self, generation: int, tick: int, change: dict) -> None:
        seed = change["seed"]
        payload = json.dumps({"selected_slot": seed["selected_slot"],
                              "modern_heights": seed["modern_heights"]}, separators=(",", ":")).encode()
        self.check(self.lib.mc_game_runtime_voxel_enable_json_v1(
            self.handle, generation, tick, payload, len(payload)), "enable_voxels")

    def step(self, generation: int, tick: int, command: dict) -> None:
        self.check(self.client.step(generation, tick, command), "step")

    def economy(self) -> list[dict]:
        buf, written = (EconomyEvent * 4096)(), U32()
        self.check(self.lib.mc_native_campaign_economy_events(
            self.handle, buf, len(buf), c.sizeof(EconomyEvent), c.byref(written)), "economy_events")
        return [fields(buf[i]) for i in range(written.value)]

    def deaths(self, generation: int, tick: int, after: int) -> list[dict]:
        buf, written = (VoxelDeath * 1024)(), U32()
        self.check(self.lib.mc_native_campaign_voxel_deaths_copy(
            self.handle, generation, tick, after, buf, len(buf), c.sizeof(VoxelDeath),
            c.byref(written)), "voxel_deaths")
        return [fields(buf[i]) for i in range(written.value)]

    def spheres(self, generation: int) -> list[dict]:
        result, start, expected_tick = [], 0, None
        while True:
            buf, batch = (Sphere * 128)(), Batch()
            self.check(self.lib.mc_native_campaign_mana_spheres_copy(
                self.handle, generation, start, buf, len(buf), c.sizeof(Sphere),
                c.byref(batch), c.sizeof(Batch)), "mana_spheres")
            require(batch.generation == generation and batch.start == start
                    and batch.written <= len(buf) and batch.total <= 1000
                    and start + batch.written <= batch.total,
                    "invalid mana-sphere page metadata")
            require(expected_tick is None or batch.tick == expected_tick, "mana-sphere page tick changed")
            expected_tick = batch.tick
            result.extend(fields(buf[i]) for i in range(batch.written))
            start += batch.written
            if not batch.more:
                require(start == batch.total, "mana-sphere page is incomplete")
                return result
            require(batch.written > 0 and start < batch.total, "mana-sphere pagination made no progress")

    def entity(self, slot: int) -> dict:
        count = U32()
        self.check(self.lib.mc_native_campaign_entity_count(self.handle, c.byref(count)), "entity_count")
        require(0 <= slot < count.value, f"entity slot {slot} outside arena")
        buf, written = (Entity * 1)(), U32()
        self.check(self.lib.mc_native_campaign_entities_copy(
            self.handle, slot, buf, 1, c.sizeof(Entity), c.byref(written)), "entity")
        require(written.value == 1 and buf[0].slot == slot, "entity slot copy mismatch")
        return fields(buf[0])

    def entities(self) -> dict[int, dict]:
        count = U32()
        self.check(self.lib.mc_native_campaign_entity_count(self.handle, c.byref(count)), "entity_count")
        require(count.value <= 1000, "entity arena exceeds expected capacity", "world_query")
        buf, written = (Entity * count.value)(), U32()
        self.check(self.lib.mc_native_campaign_entities_copy(
            self.handle, 0, buf, len(buf), c.sizeof(Entity), c.byref(written)), "entities")
        require(written.value == count.value, "entity arena copy incomplete", "world_query")
        return {buf[i].slot: fields(buf[i]) for i in range(written.value)}

    def presentations(self, name: str, generation: int, tick: int, yaw: int) -> dict[int, bytes]:
        buf, written = (Presentation * 4000)(), U32()
        fn = getattr(self.lib, name)
        self.check(fn(self.handle, generation, tick, yaw, buf, len(buf),
                      c.sizeof(Presentation), c.byref(written)), name)
        require(written.value <= len(buf), "presentation count exceeds capacity", "world_query")
        rows = {buf[i].slot: bytes(buf[i]) for i in range(written.value)}
        require(len(rows) == written.value, "duplicate presentation slot", "world_query")
        return rows

    def player(self) -> dict:
        buf, written = (Player * 8)(), U32()
        self.check(self.lib.mc_native_campaign_players_copy(
            self.handle, 0, buf, len(buf), c.sizeof(Player), c.byref(written)), "players")
        require(written.value > 0, "no local player")
        return fields(buf[0])


def read_json(path: Path, label: str) -> tuple[bytes, dict]:
    if not path.is_file():
        raise CheckError("missing_fixture", f"missing {label}: {path}")
    data = path.read_bytes()
    try:
        value = json.loads(data)
    except (ValueError, UnicodeError) as error:
        raise CheckError("invalid_fixture", f"invalid {label}: {error}") from error
    if not isinstance(value, dict):
        raise CheckError("invalid_fixture", f"{label} must be a JSON object")
    return data, value


def check_saved(snapshot: dict, replay: dict, stage: str) -> dict:
    value = snapshot["value"]
    require(int(snapshot["identity"]["tick"]) == replay["expected_tick"], "tick differs from save", stage)
    for field in ("state_hash", "terrain_hash"):
        require(int(value[field], 16) == replay["expected_" + field], f"{field} differs from save", stage)
    require(int(value["effect_head"]) == replay["expected_effect_head"], "effect head differs from save", stage)
    require(value["effect_journal_sha256"] == replay["expected_effect_journal_sha256"],
            "effect journal differs from save", stage)
    return {"tick": snapshot["identity"]["tick"], "state_hash": value["state_hash"],
            "terrain_hash": value["terrain_hash"], "effect_head": value["effect_head"],
            "effect_journal_sha256": value["effect_journal_sha256"]}


def check_world_rows(runtime: Runtime, generation: int, tick: int, yaw: int) -> tuple[dict, dict, int]:
    world = runtime.presentations("mc_native_campaign_world_entity_presentations_copy", generation, tick, yaw)
    legacy = runtime.presentations("mc_native_campaign_entity_presentations_copy", generation, tick, yaw)
    require(all(slot in world and raw == world[slot] for slot, raw in legacy.items()),
            "legacy presentation rows are not a byte-identical subset of world rows", "world_query")
    entities = runtime.entities()
    mana_count = 0
    for slot, raw in world.items():
        entity = entities.get(slot)
        if entity and (entity["class_id"], entity["model"]) == (10, 39):
            mana_count += 1
            row = Presentation.from_buffer_copy(raw)
            require(row.control_flags & 0x421 == 0, f"returned mana slot {slot} has hidden flags", "world_query")
    return world, entities, mana_count


def check_world_boundaries(runtime: Runtime, generation: int, tick: int, yaw: int) -> dict:
    world, _, _ = check_world_rows(runtime, generation, tick, yaw)
    require(bool(world), "boundary check needs a nonempty world census", "world_query")
    before = runtime.client.snapshot()
    fn = runtime.lib.mc_native_campaign_world_entity_presentations_copy
    cases = [
        ("stale_generation", generation + 1, tick, 4000, 64, False, False, 12),
        ("stale_tick", generation, tick + 1, 4000, 64, False, False, 12),
        ("small_element", generation, tick, 4000, 63, False, False, 5),
        ("large_element", generation, tick, 4000, 65, False, False, 3),
        ("small_capacity", generation, tick, 0, 64, False, False, 10),
        ("null_buffer", generation, tick, 4000, 64, True, False, 1),
        ("null_count", generation, tick, 4000, 64, False, True, 1),
    ]
    result = {}
    for name, gen, when, capacity, size, null_buffer, null_count, expected_status in cases:
        # Error paths must return before touching either the count or this canary.
        # Allocate the full advertised capacity even when an invalid guard is
        # under test, so an unexpectedly successful implementation stays safe.
        buffer = (Presentation * 4000)()
        c.memset(c.addressof(buffer), 0xA5, c.sizeof(buffer))
        original = bytes(buffer)
        count = U32(0xA5A5A5A5)
        status = fn(runtime.handle, gen, when, yaw, None if null_buffer else buffer,
                    capacity, size, None if null_count else c.byref(count))
        require(status == expected_status, f"{name} returned status {status}, expected {expected_status}", "world_query")
        require(count.value == 0xA5A5A5A5 and bytes(buffer) == original,
                f"{name} changed count or buffer on error", "world_query")
        result[name] = {"status": status, "count_preserved": None if null_count else True,
                        "buffer_preserved": None if null_buffer else True}
    require(runtime.client.snapshot() == before, "invalid world queries changed session state", "world_query")
    return result


def run(library: Path, scenario_path: Path, require_world: bool = False) -> dict:
    if not library.is_file():
        raise CheckError("missing_fixture", f"missing native library: {library}")
    _, scenario = read_json(scenario_path, "scenario")
    if scenario.get("contract") != SCENARIO_CONTRACT:
        raise CheckError("invalid_fixture", "unsupported scenario contract")
    packages = scenario["packages"]
    if not isinstance(packages, list) or not packages:
        raise CheckError("invalid_fixture", "packages must be a nonempty list")
    for package in packages:
        for key in ("package_dir", "owner_cache_path", "presentation_cache_path"):
            if not Path(package[key]).is_dir():
                raise CheckError("missing_fixture", f"missing {key}: {package[key]}")
    replays, bytes_by_name = {}, {}
    for name in ("predeath", "precastle", "postcollection"):
        spec = scenario["replays"][name]
        path = Path(spec["path"])
        data, replay = read_json(path, name)
        require(sha256_file(path) == spec["sha256"], f"{name} SHA-256 differs", "fixture")
        replays[name], bytes_by_name[name] = replay, data
    first, before, after = (replays[k] for k in ("predeath", "precastle", "postcollection"))
    expected = scenario["expected"]
    require(expected == {"archer_slot": 273, "archer_generation": 13, "sphere_slot": 364,
                         "mana": 500, "player_slot": 152, "defender_slot": 372, "castle_slot": 113},
            "unexpected expected identities", "fixture")
    first_tick, before_tick, after_tick = (first["expected_tick"], before["expected_tick"], after["expected_tick"])
    require((first_tick, before_tick, after_tick) == (534, 1131, 1972), "unexpected scenario ticks", "fixture")
    operations = after["operations"]
    require(len(operations) == after_tick and first["operations"] == operations[:first_tick]
            and before["operations"] == operations[:before_tick], "replay histories are not prefixes", "fixture")
    changes = after["voxel_mode_changes"]
    require(len(changes) == 1 and changes[0]["before_operation"] == first_tick
            and before["voxel_mode_changes"] == changes, "voxel enable boundary differs", "fixture")
    require(changes[0]["parent_terrain_hash"] == first["expected_terrain_hash"],
            "voxel seed parent terrain differs", "fixture")
    require(changes[0]["seed"]["selected_slot"] == 38, "unexpected voxel building", "fixture")
    require(all(op.get("operation") == "step" and isinstance(op.get("command"), dict)
                for op in operations[first_tick:]), "unsupported replay operation", "fixture")

    runtime = Runtime(library, packages, require_world)
    try:
        start = runtime.load(bytes_by_name["predeath"])
        start_receipt = check_saved(start, first, "predeath")
        generation = int(start["identity"]["generation"])
        require(runtime.player()["active"] == 1 and runtime.player()["entity_index"] == 152,
                "local player is not entity 152")
        runtime.enable(generation, first_tick, changes[0])
        world_evidence = None
        if require_world:
            initial = runtime.client.snapshot()
            yaw = int(initial["value"]["source_camera_pose"]["yaw"])
            world_evidence = {"boundaries": check_world_boundaries(runtime, generation, first_tick, yaw),
                              "witness": None, "subset_checks": 0, "returned_mana_checks": 0,
                              "postcollection_checks": 0}
        death_cursor, deaths, births, claims, collections, arrivals = 0, [], [], [], [], []
        observations = {}
        for tick in range(first_tick, after_tick):
            runtime.step(generation, tick, operations[tick]["command"])
            now = tick + 1
            for death in runtime.deaths(generation, now, death_cursor):
                death_cursor = max(death_cursor, death["sequence"])
                if death["slot"] == 273 and death["source_id"] == 273 and death["slot_generation"] == 13:
                    deaths.append({"tick": now, **death})
            for event in runtime.economy():
                code = event["code"]
                if code == 1 and event["primary"] == 364 and event["entity0"] == 273:
                    births.append({"tick": now, **event})
                elif code == 2 and event["primary"] == 364:
                    claims.append({"tick": now, **event})
                elif code == 16 and event["primary"] == 372 and event["secondary"] == 364:
                    collections.append({"tick": now, **event})
                elif code == 7 and event["primary"] == 372 and event["entity0"] == 113 and event["value0"] != 0:
                    arrivals.append({"tick": now, **event})
            if require_world and ((539 <= now < 1574 and world_evidence["witness"] is None) or now >= 1574):
                snapshot = runtime.client.snapshot()
                yaw = int(snapshot["value"]["source_camera_pose"]["yaw"])
                world, entities, count = check_world_rows(runtime, generation, now, yaw)
                world_evidence["subset_checks"] += 1
                world_evidence["returned_mana_checks"] += count
                if now < 1574:
                    target = next((s for s in runtime.spheres(generation)
                                   if s["slot"] == 364 and s["active"]), None)
                    if target and 364 in world:
                        legacy = runtime.presentations("mc_native_campaign_entity_presentations_copy", generation, now, yaw)
                        if 364 not in legacy:
                            world_evidence["witness"] = {"tick": now, "camera_yaw": yaw,
                                "world_row": fields(Presentation.from_buffer_copy(world[364])),
                                "sphere": target, "world_rows": len(world), "legacy_rows": len(legacy)}
                else:
                    entity = entities.get(364)
                    require(364 not in world or not entity or (entity["class_id"], entity["model"]) != (10, 39),
                            f"collected/reclaimed sphere 364 is present at tick {now}", "world_query")
                    world_evidence["postcollection_checks"] += 1
            if now in (539, 543, 1574, 1575):
                sphere = next((s for s in runtime.spheres(generation) if s["slot"] == 364), None)
                observations[str(now)] = {"sphere": sphere, "entity": runtime.entity(364)}
            if now == before_tick:
                timeline_before = check_saved(runtime.client.snapshot(), before, "timeline_precastle")
        timeline_after = check_saved(runtime.client.snapshot(), after, "timeline_postcollection")
        for name, rows, wanted in (("death", deaths, 535), ("birth", births, 539),
                                   ("claim", claims, 543), ("collection", collections, 1574),
                                   ("arrival", arrivals, 1969)):
            require(len(rows) == 1 and rows[0]["tick"] == wanted, f"{name} receipt is not exactly once at tick {wanted}")
        require(births[0]["value0"] == 500 and claims[0]["entity0"] == 152
                and collections[0]["value0"] == 500 and arrivals[0]["value0"] == 500,
                "native event owner or mana differs")
        require(observations["539"]["sphere"]["active"] == 1 and observations["539"]["sphere"]["mana"] == 500
                and observations["539"]["sphere"]["owner"] == 0, "birth sphere differs")
        claimed = observations["543"]["sphere"]
        require(claimed["active"] == 1 and claimed["owner"] == 152 and claimed["reserved"][2] == 152
                and claimed["mana"] == 500, "claimed sphere differs")
        require(observations["1574"]["sphere"]["active"] == 0, "collected sphere did not hide")
        reclaimed = observations["1575"]
        require(reclaimed["sphere"] is None and reclaimed["entity"]["active"] == 0
                and reclaimed["entity"]["class_id"] == 0, "sphere was not reclaimed following pass")
        if require_world:
            require(world_evidence["witness"] is not None,
                    "no earned live sphere was visible in world census only", "world_query")
            require(world_evidence["postcollection_checks"] == after_tick - 1574 + 1,
                    "postcollection world check did not cover every step", "world_query")
    finally:
        runtime.close()

    cold = {}
    for name in ("precastle", "postcollection"):
        fresh = Runtime(library, packages, require_world)
        try:
            snapshot = fresh.load(bytes_by_name[name])
            cold[name] = check_saved(snapshot, replays[name], "cold_" + name)
            generation = int(snapshot["identity"]["generation"])
            sphere = next((s for s in fresh.spheres(generation) if s["slot"] == 364 and s["active"]), None)
            castle = fresh.entity(113)
            slot = fresh.entity(364)
            if name == "precastle":
                require(sphere is not None and sphere["owner"] == 152 and sphere["mana"] == 500,
                        "cold precast lost the claimed sphere")
                require(not (castle["active"] == 1 and (castle["class_id"], castle["model"]) == (3, 2)),
                        "cold precast unexpectedly has the later Castle")
            else:
                require(sphere is None and castle["active"] == 1 and (castle["class_id"], castle["model"]) == (3, 2),
                        "cold postcollection lost Castle or restored sphere")
                require(slot["active"] == 0 and slot["class_id"] == 0,
                        "cold postcollection restored the reclaimed source slot")
                if require_world:
                    yaw = int(snapshot["value"]["source_camera_pose"]["yaw"])
                    world, entities, _ = check_world_rows(fresh, generation, int(snapshot["identity"]["tick"]), yaw)
                    require(364 not in world or (entities[364]["class_id"], entities[364]["model"]) != (10, 39),
                            "cold postcollection world census restored sphere 364", "world_query")
            cold[name].update({"sphere364": sphere, "slot364": slot, "castle113": castle})
        finally:
            fresh.close()
    result = {"contract": CONTRACT, "ok": True, "native_sha256": sha256_file(library),
            "scenario_sha256": sha256_file(scenario_path), "fixture_sha256": {
                name: scenario["replays"][name]["sha256"] for name in replays},
            "predeath": start_receipt, "timeline_precastle": timeline_before,
            "timeline_postcollection": timeline_after, "cold_reloads": cold,
            "death": deaths[0], "birth": births[0], "claim": claims[0],
            "collection": collections[0], "arrival": arrivals[0], "observations": observations,
            "castle_balance_note": "ABI receipt proves a 500-mana arrival; common entity snapshot does not expose Castle balance"}
    if require_world:
        result["world_query"] = world_evidence
        result["world_query"]["scope"] = ("Only returned mana rows were checked for hide flags; "
                                           "no hidden or pending actor was synthetically injected.")
    return result


def main() -> int:
    parser = Parser(description=__doc__)
    parser.add_argument("--library", required=True, type=Path)
    parser.add_argument("--scenario", required=True, type=Path)
    parser.add_argument("--require-world-query", action="store_true",
                        help="require the additive world census and its live, lifecycle and boundary checks")
    parser.add_argument("--output", type=Path, help="new JSON receipt path; existing files are never overwritten")
    try:
        args = parser.parse_args()
        if args.output is not None and args.output.exists():
            raise CheckError("output_exists", f"receipt already exists: {args.output}", "output")
        result = run(args.library.resolve(), args.scenario.resolve(), args.require_world_query)
        if args.output is not None:
            with args.output.open("x", encoding="utf-8") as stream:
                json.dump(result, stream, sort_keys=True, indent=2)
                stream.write("\n")
        print(json.dumps(result, sort_keys=True, separators=(",", ":")))
        return 0
    except CheckError as error:
        result = {"contract": CONTRACT, "ok": False,
                  "error": {"code": error.code, "stage": error.stage, "message": str(error)}}
    except (KeyError, TypeError, ValueError, OSError, AttributeError, NeutralNativeError) as error:
        result = {"contract": CONTRACT, "ok": False,
                  "error": {"code": "invalid_input_or_native", "stage": "setup", "message": str(error)}}
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 1


if __name__ == "__main__":
    sys.exit(main())
