#!/usr/bin/env python3
"""Measure real replay-loaded native CPU capacity, separately from rendering.

The two modes advance the same commands from the same checkpoint. Snapshot mode
uses one preallocated ABI copy per tick, as the client does, rather than a size
probe followed by a second expensive observation. No pacing or gameplay edits.
"""
from __future__ import annotations

import argparse
import ctypes as c
import json
import statistics
import sys
import time
from pathlib import Path

from verify_neutral_game_session import sha256_file
from verify_voxel_mana_e2e import Runtime


def distribution(values: list[float]) -> dict:
    ordered = sorted(values)
    return {"mean_ms": statistics.mean(ordered), **{
        name: ordered[int((len(ordered) - 1) * p)]
        for name, p in (("p50_ms", .5), ("p95_ms", .95), ("p99_ms", .99), ("max_ms", 1))}}


def measure(args, packages: list[dict], replay: bytes, mode: str) -> dict:
    runtime = Runtime(args.library, packages)
    try:
        initial = runtime.load(replay)
        generation, first = int(initial["identity"]["generation"]), int(initial["identity"]["tick"])
        # One MiB is a bounded capacity; overflow fails instead of measuring a
        # different number of expensive native observation calls.
        buffer, written = c.create_string_buffer(1024 * 1024), c.c_uint32()

        def snapshot() -> tuple[dict, float]:
            began = time.perf_counter_ns()
            runtime.check(runtime.lib.mc_game_session_snapshot_json_v1(
                runtime.handle, buffer, len(buffer), c.byref(written)), "snapshot")
            native_ms = (time.perf_counter_ns() - began) / 1e6
            if written.value > len(buffer):
                raise RuntimeError("Snapshot exceeded bounded buffer")
            return json.loads(buffer.raw[:written.value]), native_ms

        timings, step_ms, snapshot_ms, decode_ms = [], [], [], []
        start = None
        for index in range(args.ticks + args.warmup):
            command = {"longitudinal": "coast", "lateral": "neutral",
                       "roll_axis": 0, "pitch_axis": 0,
                       "cast_left": False, "cast_right": False, "cast_selected": False,
                       "look_yaw": 0, "look_pitch": 0,
                       "elapsed_centiseconds": 4 if index % 3 == 2 else 3}
            if index == args.warmup:
                start = time.perf_counter_ns()
            began = time.perf_counter_ns()
            runtime.step(generation, first + index, command)
            stepped = time.perf_counter_ns()
            projected_ms, decoded_ms = 0., 0.
            if mode == "step_and_snapshot":
                observation, projected_ms = snapshot()
                decoded_ms = (time.perf_counter_ns() - stepped) / 1e6 - projected_ms
                if (int(observation["identity"]["tick"]) != first + index + 1
                        or int(observation["identity"]["generation"]) != generation):
                    raise RuntimeError("Native observation did not advance exactly one tick")
            ended = time.perf_counter_ns()
            if index >= args.warmup:
                timings.append((ended - began) / 1e6)
                step_ms.append((stepped - began) / 1e6)
                snapshot_ms.append(projected_ms)
                decode_ms.append(decoded_ms)
        seconds = (time.perf_counter_ns() - start) / 1e9
        final, _ = snapshot()
        if int(final["identity"]["tick"]) != first + args.warmup + args.ticks:
            raise RuntimeError("Final tick did not match submitted commands")
        return {"mode": mode, "ticks": args.ticks, "warmup_ticks": args.warmup,
                "wall_seconds": seconds, "capacity_ticks_per_second": args.ticks / seconds,
                "whole_tick": distribution(timings), "step": distribution(step_ms),
                "snapshot_native": distribution(snapshot_ms), "snapshot_decode": distribution(decode_ms),
                "first_tick": first, "last_tick": int(final["identity"]["tick"]),
                "initial_snapshot": initial, "final_snapshot": final}
    finally:
        runtime.close()


class Parser(argparse.ArgumentParser):
    def error(self, message: str) -> None:
        raise ValueError(message)


def main() -> None:
    parser = Parser(description=__doc__)
    parser.add_argument("--library", required=True, type=Path)
    parser.add_argument("--scenario", required=True, type=Path)
    parser.add_argument("--replay", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--ticks", type=int, default=360)
    parser.add_argument("--warmup", type=int, default=30)
    parser.add_argument("--repetitions", type=int, default=3)
    args = parser.parse_args()
    if not (90 <= args.ticks <= 3600 and 0 <= args.warmup <= 300 and 1 <= args.repetitions <= 10):
        raise ValueError("Ticks, warmup or repetitions exceed bounds")
    args.output.mkdir(parents=True, exist_ok=False)
    packages = json.loads(args.scenario.read_text())["packages"]
    replay = args.replay.read_bytes()
    result = {"schema": "mc-native-cpu-benchmark/v1", "status": "running",
              "meaning": "Unpaced native CPU capacity; this is not rendered FPS or normal-speed gameplay",
              "native_sha256": sha256_file(args.library), "replay_sha256": sha256_file(args.replay),
              "command": sys.argv, "runs": []}
    destination = args.output / "report.json"
    try:
        for _ in range(args.repetitions):
            for mode in ("step_only", "step_and_snapshot"):
                result["runs"].append(measure(args, packages, replay, mode))
                destination.write_text(json.dumps(result, indent=2) + "\n")
        # The observation query must not change authoritative final state.
        finals = [run["final_snapshot"]["value"] for run in result["runs"]]
        if any(value != finals[0] for value in finals[1:]):
            raise RuntimeError("Authoritative final snapshots differ between modes or repetitions")
        result["status"] = "complete"
    except Exception as error:
        result.update(status="error", error={"type": type(error).__name__, "message": str(error)})
        raise
    finally:
        destination.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"schema": result["schema"], "status": result["status"], "report": str(destination),
                      "runs": [{key: run[key] for key in ("mode", "capacity_ticks_per_second", "whole_tick")}
                               for run in result["runs"]]}))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(json.dumps({"schema": "mc-native-cpu-benchmark/v1", "status": "error",
                          "error": {"type": type(error).__name__, "message": str(error)}}), file=sys.stderr)
        sys.exit(1)
