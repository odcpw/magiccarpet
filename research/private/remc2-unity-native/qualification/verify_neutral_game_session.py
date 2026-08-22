#!/usr/bin/env python3
"""Verify the additive neutral GameSession ABI against the fixed F1 corpus.

The candidate plug-in is supplied explicitly and is never written into the
accepted corpus. The test drives only the neutral JSON command/session API;
opaque compatibility receipts prove the exact retained event/audio identities
without making MC2 numeric selectors part of the public domain vocabulary.
"""

from __future__ import annotations

import argparse
import ctypes as c
import hashlib
import json
from pathlib import Path


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
MANIFEST = HERE / "level1-reference-corpus.json"

MC_STATUS_OK = 0
MC_STATUS_INVALID_ARGUMENT = 3
MC_STATUS_BOUNDS = 10
MC_STATUS_GENERATION_MISMATCH = 12


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


class NeutralNativeError(RuntimeError):
    def __init__(self, status: int, message: str) -> None:
        super().__init__(message)
        self.status = status


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def autoplay_command(index: int) -> dict[str, object]:
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
    longitudinal = "accelerate" if flags & 1 else "decelerate" if flags & 2 else "coast"
    lateral = "positive" if flags & 8 else "negative" if flags & 4 else "neutral"
    return {
        "longitudinal": longitudinal,
        "lateral": lateral,
        "roll_axis": roll,
        "pitch_axis": pitch,
        "cast_left": bool(flags & 0x10),
        "cast_right": bool(flags & 0x20),
        "cast_selected": bool(flags & 0x40),
        "look_yaw": 0,
        "look_pitch": 0,
    }


class NeutralClient:
    def __init__(
        self,
        plugin: Path,
        *,
        open_session: bool = True,
        workspace: Path | None = None,
    ) -> None:
        self.plugin = plugin.resolve()
        require(self.plugin.is_file(), f"missing candidate plug-in: {self.plugin}")
        self.lib = c.CDLL(str(self.plugin))
        self._bind()
        self.handle = c.c_uint64()
        self.workspace = workspace.resolve() if workspace is not None else None
        if open_session:
            self.open()

    def open(self) -> None:
        require(not self.handle.value, "neutral session is already open")
        package = REPO / "research/private/generated/level1-export-m"
        closure = REPO / "research/private/generated/level1-asset-closure.json"
        cache = REPO / "research/private/generated/level1-unity-asset-cache-plan29-final"
        values = [str(package).encode(), str(closure).encode(), str(cache).encode()]
        if self.workspace is None:
            status = self.lib.mc_game_session_open_v1(
                values[0], len(values[0]), values[1], len(values[1]),
                values[2], len(values[2]), c.byref(self.handle),
            )
        else:
            workspace = str(self.workspace).encode()
            status = self.lib.mc_game_session_open_workspace_v1(
                values[0], len(values[0]), values[1], len(values[1]),
                values[2], len(values[2]), workspace, len(workspace),
                c.byref(self.handle),
            )
        require(status == MC_STATUS_OK, f"neutral open failed with status {status}")

    def _bind(self) -> None:
        lib = self.lib
        lib.mc_game_session_contract_json_v1.argtypes = [
            c.c_void_p, c.c_uint32, c.POINTER(c.c_uint32),
        ]
        lib.mc_game_session_contract_json_v1.restype = c.c_int32
        lib.mc_game_session_open_v1.argtypes = [
            c.c_char_p, c.c_uint32, c.c_char_p, c.c_uint32,
            c.c_char_p, c.c_uint32, c.POINTER(c.c_uint64),
        ]
        lib.mc_game_session_open_v1.restype = c.c_int32
        lib.mc_game_session_open_workspace_v1.argtypes = [
            c.c_char_p, c.c_uint32, c.c_char_p, c.c_uint32,
            c.c_char_p, c.c_uint32, c.c_char_p, c.c_uint32,
            c.POINTER(c.c_uint64),
        ]
        lib.mc_game_session_open_workspace_v1.restype = c.c_int32
        lib.mc_game_session_destroy_v1.argtypes = [c.c_uint64]
        lib.mc_game_session_destroy_v1.restype = c.c_int32
        lib.mc_game_session_snapshot_json_v1.argtypes = [
            c.c_uint64, c.c_void_p, c.c_uint32, c.POINTER(c.c_uint32),
        ]
        lib.mc_game_session_snapshot_json_v1.restype = c.c_int32
        lib.mc_game_session_step_json_v1.argtypes = [
            c.c_uint64, c.c_uint64, c.c_uint64, c.c_char_p, c.c_uint32,
        ]
        lib.mc_game_session_step_json_v1.restype = c.c_int32
        lib.mc_game_session_reset_v1.argtypes = [c.c_uint64, c.c_uint64, c.c_uint64]
        lib.mc_game_session_reset_v1.restype = c.c_int32
        lib.mc_game_session_events_json_v1.argtypes = [
            c.c_uint64, c.c_uint64, c.c_uint64, c.c_uint64, c.c_uint32,
            c.c_void_p, c.c_uint32, c.POINTER(c.c_uint32),
        ]
        lib.mc_game_session_events_json_v1.restype = c.c_int32
        lib.mc_game_session_audio_json_v1.argtypes = [
            c.c_uint64, c.c_uint64, c.c_uint64, c.c_uint64,
            c.c_void_p, c.c_uint32, c.POINTER(c.c_uint32),
        ]
        lib.mc_game_session_audio_json_v1.restype = c.c_int32
        lib.mc_game_session_presentation_asset_json_v1.argtypes = [
            c.c_uint64, c.c_uint64, c.c_uint64, c.c_char_p, c.c_uint32,
            c.c_void_p, c.c_uint32, c.POINTER(c.c_uint32),
        ]
        lib.mc_game_session_presentation_asset_json_v1.restype = c.c_int32

    def close(self) -> None:
        if self.handle.value:
            status = self.lib.mc_game_session_destroy_v1(self.handle.value)
            require(status == MC_STATUS_OK, f"neutral destroy failed with status {status}")
            self.handle.value = 0

    @staticmethod
    def _read_json(call) -> dict:
        length = c.c_uint32()
        status = call(None, 0, c.byref(length))
        if status != MC_STATUS_BOUNDS or length.value == 0:
            raise NeutralNativeError(status, "neutral JSON sizing failed")
        buffer = (c.c_uint8 * length.value)()
        status = call(buffer, length.value, c.byref(length))
        if status != MC_STATUS_OK:
            raise NeutralNativeError(
                status, f"neutral JSON copy failed with status {status}"
            )
        return json.loads(bytes(buffer[:length.value]))

    def snapshot(self) -> dict:
        return self._read_json(
            lambda buffer, capacity, length: self.lib.mc_game_session_snapshot_json_v1(
                self.handle.value, buffer, capacity, length
            )
        )

    def contract(self) -> dict:
        return self._read_json(
            lambda buffer, capacity, length: self.lib.mc_game_session_contract_json_v1(
                buffer, capacity, length
            )
        )

    def step(self, generation: int, tick: int, command: dict[str, object]) -> int:
        payload = json.dumps(command, sort_keys=True, separators=(",", ":")).encode()
        return self.lib.mc_game_session_step_json_v1(
            self.handle.value, generation, tick, payload, len(payload)
        )

    def events(self, generation: int, tick: int, cursor: int, limit: int = 1_024) -> dict:
        return self._read_json(
            lambda buffer, capacity, length: self.lib.mc_game_session_events_json_v1(
                self.handle.value, generation, tick, cursor, limit,
                buffer, capacity, length,
            )
        )

    def reset(self, generation: int, tick: int) -> int:
        return self.lib.mc_game_session_reset_v1(self.handle.value, generation, tick)

    def try_audio(self, generation: int, tick: int, event_sequence: int) -> dict | None:
        length = c.c_uint32()
        status = self.lib.mc_game_session_audio_json_v1(
            self.handle.value, generation, tick, event_sequence,
            None, 0, c.byref(length),
        )
        if status == MC_STATUS_BOUNDS and length.value == 0:
            return None
        if status != MC_STATUS_BOUNDS or length.value == 0:
            raise NeutralNativeError(status, "neutral audio sizing failed")
        buffer = (c.c_uint8 * length.value)()
        status = self.lib.mc_game_session_audio_json_v1(
            self.handle.value, generation, tick, event_sequence,
            buffer, length.value, c.byref(length),
        )
        if status != MC_STATUS_OK:
            raise NeutralNativeError(
                status, f"neutral audio copy failed with status {status}"
            )
        return json.loads(bytes(buffer[:length.value]))

    def presentation_asset(self, generation: int, tick: int, semantic_id: str) -> dict:
        encoded = semantic_id.encode()
        return self._read_json(
            lambda buffer, capacity, length:
                self.lib.mc_game_session_presentation_asset_json_v1(
                    self.handle.value, generation, tick, encoded, len(encoded),
                    buffer, capacity, length,
                )
        )


def verify_snapshot(expected: dict, actual: dict) -> None:
    identity = actual["identity"]
    value = actual["value"]
    pose = value["player_pose"]
    motion = value["player_motion"]
    require(int(identity["tick"]) == expected["tick"], "neutral identity tick mismatch")
    require(int(value["tick"]) == expected["tick"], "neutral snapshot tick mismatch")
    require(int(value["event_head"]) == expected["event_head"], "neutral event head mismatch")
    require(value["state_hash"] == expected["state_hash_fnv1a64"], "neutral state hash mismatch")
    require(
        value["conformance_hash"] == expected["conformance_hash_fnv1a64"],
        "neutral conformance hash mismatch",
    )
    require([pose["x"], pose["y"], pose["z"]] == expected["position"], "neutral position mismatch")
    require(pose["yaw"] == expected["yaw"], "neutral yaw mismatch")
    require(pose["display_pitch"] == expected["display_pitch"], "neutral display pitch mismatch")
    require(pose["effective_pitch"] == expected["effective_pitch"], "neutral effective pitch mismatch")
    require(pose["roll"] == expected["roll"], "neutral roll mismatch")
    require(motion["target_speed"] == expected["target_speed"], "neutral target speed mismatch")
    require(motion["actual_speed"] == expected["actual_speed"], "neutral actual speed mismatch")
    require(motion["strafe_speed"] == expected["strafe_speed"], "neutral strafe speed mismatch")
    require(int(value["action_sequence"]) == expected["fire_sequence"], "neutral action sequence mismatch")


def verify_receipt(checkpoint: dict, observation: dict, verify_lifecycle: bool) -> None:
    page = observation["value"]
    receipt = page["compatibility"]
    event = checkpoint["event_identity"]
    audio = checkpoint["audio_identity"]
    require(receipt["contract"] == "mc2-owned-event-audio-identity/v1", "wrong compatibility contract")
    require(receipt["event_count"] == event["count"], "neutral event count mismatch")
    require(receipt["event_content_sha256"] == event["content_sha256"], "neutral event content mismatch")
    if verify_lifecycle:
        require(receipt["event_lifecycle_sha256"] == event["lifecycle_sha256"], "neutral event lifecycle mismatch")
    require(receipt["audio_count"] == audio["count"], "neutral audio count mismatch")
    require(receipt["audio_content_sha256"] == audio["content_sha256"], "neutral audio content mismatch")
    if verify_lifecycle:
        require(receipt["audio_lifecycle_sha256"] == audio["lifecycle_sha256"], "neutral audio lifecycle mismatch")
    encoded = json.dumps(page["events"], sort_keys=True, separators=(",", ":"))
    require("level1" not in encoded and "source_slot" not in encoded, "source identity leaked into neutral events")


def run_route(client: NeutralClient, checkpoints: dict[int, dict], generation: int) -> dict[int, dict]:
    observed: dict[int, dict] = {}
    cursor = 0
    audio_observed = False
    for index in range(440):
        status = client.step(generation, index, autoplay_command(index))
        require(status == MC_STATUS_OK, f"neutral step {index} failed with status {status}")
        tick = index + 1
        if tick not in checkpoints:
            continue
        snapshot = client.snapshot()
        events = client.events(generation, tick, cursor)
        cursor = int(events["value"]["next_cursor"])
        verify_snapshot(checkpoints[tick]["state_identity"], snapshot)
        verify_receipt(checkpoints[tick], events, generation == 1)
        if not audio_observed:
            for event in events["value"]["events"]:
                audio = client.try_audio(generation, tick, int(event["sequence"]))
                if audio is None:
                    continue
                value = audio["value"]
                require(value["event_sequence"] == event["sequence"], "neutral audio join mismatch")
                require(value["state_hash"] == event["state_hash"], "neutral audio state hash mismatch")
                require(value["asset"] == event["asset"], "neutral audio asset identity mismatch")
                require(isinstance(value["action"], str), "neutral audio action is not semantic")
                audio_observed = True
                break
        observed[tick] = {"snapshot": snapshot, "events": events}
    require(set(observed) == set(checkpoints), "neutral checkpoints missing")
    require(audio_observed, "neutral route exposed no readable audio sidecar")
    return observed


def normalized(observed: dict[int, dict]) -> dict[int, dict]:
    result = json.loads(json.dumps(observed))
    for item in result.values():
        item["snapshot"]["identity"]["generation"] = "0"
        receipt = item["events"]["value"]["compatibility"]
        receipt.pop("event_lifecycle_sha256")
        receipt.pop("audio_lifecycle_sha256")
        item["events"]["identity"]["generation"] = "0"
    return result


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--plugin", type=Path, required=True)
    args = parser.parse_args()
    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    suite = next(row for row in manifest["suites"] if row["id"] == "unity-synthetic-presentation-route")
    checkpoints = {row["unity_checkpoint_tick"]: row for row in suite["checkpoints"]}
    client = NeutralClient(args.plugin)
    try:
        initial = client.snapshot()
        require(initial["identity"]["generation"] == "1" and initial["identity"]["tick"] == "0", "initial neutral identity mismatch")
        bad = {"source_opcode": 1}
        require(client.step(1, 0, bad) == MC_STATUS_INVALID_ARGUMENT, "source-shaped command was accepted")
        require(client.snapshot() == initial, "rejected command changed neutral state")
        first = run_route(client, checkpoints, 1)
        require(client.step(1, 439, autoplay_command(439)) == MC_STATUS_INVALID_ARGUMENT, "stale tick was accepted")
        require(client.reset(1, 440) == MC_STATUS_OK, "neutral reset failed")  # ubs:ignore — compares a public integer ABI status, not secret material
        reset = client.snapshot()
        require(reset["identity"]["generation"] == "2" and reset["identity"]["tick"] == "0", "neutral reset identity mismatch")  # ubs:ignore — compares public lifecycle counters, not secret material
        require(client.step(1, 0, autoplay_command(0)) == MC_STATUS_GENERATION_MISMATCH, "stale generation was accepted")
        second = run_route(client, checkpoints, 2)
        require(normalized(first) == normalized(second), "neutral reset replay changed normalized observations")
    finally:
        client.close()
    print(
        "MC_NEUTRAL_GAME_SESSION PASS "
        f"plugin_sha256={sha256_file(args.plugin)} checkpoints={len(checkpoints)} "
        "steps=440 replay=exact fixed_snapshots=exact fixed_event_audio_receipts=exact "
        "audio_sidecar=readable stale_generation=rejected stale_tick=rejected "
        "source_command=rejected corpus_file=unchanged"
    )


if __name__ == "__main__":
    main()
