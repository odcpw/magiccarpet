#!/usr/bin/env python3
"""Verify private exact-tick qualified captures, optionally across two runs."""

from __future__ import annotations

import hashlib
import json
import sys
import tempfile
from pathlib import Path

from PIL import Image


CHECKPOINTS = (145, 163, 220, 303, 400)
SCHEMA = "mc2-private-qualified-full-frame-v2"
PACKAGE_SHA256 = "730d2fa084265df839f3f2b55ed766ac167a8b152ca4631b5ded9d0bb9104ac8"
CLOSURE_SHA256 = "1c3774ca5c032c1a7b8b1c48870840eee33011a2830d08c51f3104fb5c5bcc12"
CACHE_SHA256 = "a5fdd9961ebd61566b0230e2dc1b468219dae74536279b51d89d9c2d6cc0ce81"
PLUGIN_SHA256 = "dc1f27e8f36736b37c046113faacead948413c90fc8d2baca0313921f2904e7b"
CORPUS_PATH = (
    Path(__file__).resolve().parent
    / "qualification"
    / "level1-reference-corpus.json"
)
CORPUS = json.loads(CORPUS_PATH.read_text(encoding="utf-8"))
ROUTE = next(
    suite for suite in CORPUS["suites"]
    if suite["id"] == "unity-synthetic-presentation-route"
)
ROUTE_CHECKPOINTS = {
    checkpoint["unity_checkpoint_tick"]: checkpoint
    for checkpoint in ROUTE["checkpoints"]
}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def resolve_private_directory(raw: str) -> Path:
    directory = Path(raw).resolve(strict=True)
    temporary_root = Path(tempfile.gettempdir()).resolve(strict=True)
    if directory == temporary_root or temporary_root not in directory.parents:
        raise SystemExit(f"capture directory must be below {temporary_root}: {directory}")
    if not directory.is_dir() or directory.is_symlink():
        raise SystemExit(f"capture directory is not a real directory: {directory}")
    return directory


def verify_run(directory: Path) -> dict[int, tuple[str, str]]:
    identities: dict[int, tuple[str, str]] = {}
    expected_autoplay_names: set[str] = set()
    for checkpoint in CHECKPOINTS:
        stem = f"magiccarpet-autoplay-{checkpoint:03d}"
        png_path = directory / f"{stem}.png"
        manifest_path = directory / f"{stem}.json"
        expected_autoplay_names.update((png_path.name, manifest_path.name))
        if not png_path.is_file() or png_path.is_symlink():
            raise SystemExit(f"missing or unsafe PNG: {png_path}")
        if not manifest_path.is_file() or manifest_path.is_symlink():
            raise SystemExit(f"missing or unsafe manifest: {manifest_path}")

        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        png_hash = sha256(png_path)
        required = {
            "schema": SCHEMA,
            "presentation_mode": "qualified",
            "evidence_authority": "synthetic-native-unity-regression",
            "oracle_status": "no-dynamic-remc2-route-capture",
            "checkpoint_tick": checkpoint,
            "last_command_index": checkpoint - 1,
            "native_render_tick": checkpoint - 1,
            "exact_checkpoint": True,
            "command_encoding": "mc2-player-command-10-byte-le/v1",
            "width": 1600,
            "height": 900,
            "package_sha256": PACKAGE_SHA256,
            "closure_sha256": CLOSURE_SHA256,
            "owner_cache_content_sha256": CACHE_SHA256,
            "native_plugin_sha256": PLUGIN_SHA256,
            "unity_version": CORPUS["runtime_pins"]["unity_version"],
            "unity_build_guid": CORPUS["runtime_pins"]["unity_build_guid"],
            "png_sha256": png_hash,
            "magenta_pixels": 0,
            "magenta_gate_pass": True,
        }
        for key, expected in required.items():
            if manifest.get(key) != expected:
                raise SystemExit(
                    f"{manifest_path.name}: {key}={manifest.get(key)!r}, "
                    f"expected {expected!r}"
                )
        if not manifest.get("unity_build_guid"):
            raise SystemExit(f"{manifest_path.name}: missing Unity build identity")
        if not manifest.get("graphics_api") or not manifest.get("graphics_device"):
            raise SystemExit(f"{manifest_path.name}: missing graphics identity")

        expected = ROUTE_CHECKPOINTS[checkpoint]
        expected_frame = expected["unity_frame_identity"]
        if (
            expected_frame["schema"] != SCHEMA
            or expected_frame["png_sha256"] != png_hash
        ):
            raise SystemExit(f"{manifest_path.name}: registered PNG identity mismatch")
        if manifest.get("command_prefix_sha256") != expected["input_identity"]["prefix_sha256"]:
            raise SystemExit(f"{manifest_path.name}: command prefix mismatch")
        snapshot = manifest.get("snapshot") or {}
        expected_state = expected["state_identity"]
        state_fields = {
            "generation": "generation",
            "tick": "tick",
            "event_head": "event_head",
            "state_hash_fnv1a64": "state_hash_fnv1a64",
            "conformance_hash_fnv1a64": "conformance_hash_fnv1a64",
            "entity_index": "entity_index",
            "yaw": "yaw",
            "display_pitch": "display_pitch",
            "effective_pitch": "effective_pitch",
            "roll_accumulator": "roll",
            "target_speed": "target_speed",
            "actual_speed": "actual_speed",
            "strafe_speed": "strafe_speed",
            "phase": "phase",
            "last_resolution": "last_resolution",
            "profile": "profile",
            "source_turn": None,
            "fire_sequence": "fire_sequence",
        }
        for actual_field, expected_field in state_fields.items():
            expected_value = (
                expected["source_turn"]
                if expected_field is None
                else expected_state[expected_field]
            )
            if snapshot.get(actual_field) != expected_value:
                raise SystemExit(
                    f"{manifest_path.name}: snapshot {actual_field} mismatch"
                )
        if [snapshot.get("x"), snapshot.get("y"), snapshot.get("z")] != expected_state["position"]:
            raise SystemExit(f"{manifest_path.name}: snapshot position mismatch")

        events = manifest.get("events") or {}
        expected_events = expected["event_identity"]
        for field in (
            "first_sequence",
            "last_sequence",
            "count",
            "content_schema",
            "content_sha256",
            "lifecycle_schema",
            "lifecycle_sha256",
        ):
            if events.get(field) != expected_events[field]:
                raise SystemExit(f"{manifest_path.name}: event {field} mismatch")
        if events.get("cursor_equals_head") is not True:
            raise SystemExit(f"{manifest_path.name}: event cursor did not reach head")

        presentation = manifest.get("presentation") or {}
        expected_presentation = expected["camera_identity"]["presentation"]
        presentation_fields = {
            "generation": "generation",
            "tick": "tick",
            "source_turn": "source_turn",
            "camera_yaw": "yaw",
            "camera_pitch": "pitch",
            "camera_roll": "roll",
            "camera_fov": "fov",
            "authority": "authority",
            "flags": "flags",
        }
        for actual_field, expected_field in presentation_fields.items():
            if presentation.get(actual_field) != expected_presentation[expected_field]:
                raise SystemExit(
                    f"{manifest_path.name}: presentation {actual_field} mismatch"
                )
        if [
            presentation.get("camera_x"),
            presentation.get("camera_y"),
            presentation.get("camera_z"),
        ] != expected_presentation["position"]:
            raise SystemExit(f"{manifest_path.name}: presentation position mismatch")

        render = manifest.get("render_camera") or {}
        expected_render = expected["camera_identity"]["render"]
        render_fields = {
            "generation": "generation",
            "tick": "tick",
            "source_turn": "source_turn",
            "eye_z": "eye_z",
            "yaw": "yaw",
            "pitch": "pitch",
            "roll": "roll",
            "fov": "fov",
            "authority": "authority",
            "flags": "flags",
        }
        for actual_field, expected_field in render_fields.items():
            if render.get(actual_field) != expected_render[expected_field]:
                raise SystemExit(
                    f"{manifest_path.name}: render camera {actual_field} mismatch"
                )
        if [
            render.get("history_x"),
            render.get("history_y"),
        ] != expected_render["history"]:
            raise SystemExit(f"{manifest_path.name}: render camera history mismatch")
        if [
            render.get("slope_target_x"),
            render.get("slope_target_y"),
        ] != expected_render["slope_target"]:
            raise SystemExit(f"{manifest_path.name}: render camera slope mismatch")
        if [
            render.get("render_offset_x"),
            render.get("render_offset_y"),
        ] != expected_render["render_offset"]:
            raise SystemExit(f"{manifest_path.name}: render camera offset mismatch")

        terrain = manifest.get("terrain") or {}
        expected_terrain = expected["terrain_identity"]
        terrain_fields = {
            "generation": "generation",
            "tick": "tick",
            "width": "width",
            "height": "height",
            "stride": "stride",
            "viewport_x": "viewport_x",
            "viewport_y": "viewport_y",
            "viewport_width": "viewport_width",
            "viewport_height": "viewport_height",
            "index_hash_fnv1a64": "index_hash_fnv1a64",
            "coverage_hash_fnv1a64": "coverage_hash_fnv1a64",
            "reflected_triangles": "reflected",
            "primary_mode5_triangles": "mode5",
            "primary_mode26_triangles": "mode26",
            "terrain_pixel_writes": "writes",
        }
        for actual_field, expected_field in terrain_fields.items():
            if terrain.get(actual_field) != expected_terrain[expected_field]:
                raise SystemExit(f"{manifest_path.name}: terrain {actual_field} mismatch")
        if (
            manifest.get("terrain_gpu_bottom_up_fnv1a64")
            != expected_terrain["gpu_bottom_up_fnv1a64"]
            or manifest.get("terrain_logical_top_down_fnv1a64")
            != expected_terrain["logical_top_down_fnv1a64"]
        ):
            raise SystemExit(f"{manifest_path.name}: terrain GPU identity mismatch")

        with Image.open(png_path) as image:
            rgba = image.convert("RGBA")
            if rgba.size != (1600, 900):
                raise SystemExit(f"{png_path.name}: unexpected size {rgba.size}")
            raw_unity_rgba = rgba.transpose(
                Image.Transpose.FLIP_TOP_BOTTOM
            ).tobytes()
            rgba_hash = hashlib.sha256(raw_unity_rgba).hexdigest()
            if manifest.get("rgba_sha256") != rgba_hash:
                raise SystemExit(
                    f"{manifest_path.name}: raw RGBA SHA-256 mismatch"
                )
            if expected_frame["raw_rgba_sha256"] != rgba_hash:
                raise SystemExit(
                    f"{manifest_path.name}: registered raw RGBA identity mismatch"
                )
            magenta = sum(
                1 for red, green, blue, _alpha in rgba.get_flattened_data()
                if red == 255 and green == 0 and blue == 255
            )
            if magenta:
                raise SystemExit(f"{png_path.name}: {magenta} magenta pixels")
        identities[checkpoint] = (
            png_hash,
            hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
        )

    observed_autoplay_names = {
        path.name for path in directory.glob("magiccarpet-autoplay-*")
    }
    if observed_autoplay_names != expected_autoplay_names:
        raise SystemExit(
            "unexpected autoplay artifacts: "
            f"{sorted(observed_autoplay_names ^ expected_autoplay_names)}"
        )
    return identities


def main() -> None:
    if len(sys.argv) not in (2, 3):
        raise SystemExit(
            "usage: verify_qualified_full_frames.py RUN_A [RUN_B]"
        )
    first_directory = resolve_private_directory(sys.argv[1])
    first = verify_run(first_directory)
    if len(sys.argv) == 3:
        second_directory = resolve_private_directory(sys.argv[2])
        second = verify_run(second_directory)
        if first != second:
            raise SystemExit("qualified full-frame runs are not byte-identical")
        comparison = " byte-identical-runs=2"
    else:
        comparison = ""
    print(
        "MC_QUALIFIED_FULL_FRAMES PASS "
        f"checkpoints={len(CHECKPOINTS)} exact=5 magenta=0{comparison}"
    )


if __name__ == "__main__":
    main()
