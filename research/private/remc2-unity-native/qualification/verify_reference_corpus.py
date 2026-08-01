#!/usr/bin/env python3
"""Verify the private Level 1 reference-corpus manifest against live files."""

from __future__ import annotations

import hashlib
import json
import subprocess
import tempfile
from pathlib import Path


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
MANIFEST_PATH = HERE / "level1-reference-corpus.json"


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def verify_runtime_pins(manifest: dict) -> None:
    pins = manifest["runtime_pins"]
    files = {
        "package_manifest_file_sha256":
            REPO / "research/private/generated/level1-export-m/manifest.json",
        "closure_file_sha256":
            REPO / "research/private/generated/level1-asset-closure.json",
        "owner_cache_manifest_file_sha256":
            REPO / "research/private/generated/level1-unity-asset-cache-plan29-final/manifest.json",
        "unity_player_file_sha256":
            REPO / "research/private/remc2-unity-native/Builds/Linux/MagicCarpetNative.x86_64",
        "unity_managed_assembly_sha256":
            REPO / "research/private/remc2-unity-native/Builds/Linux"
            / pins["unity_managed_assembly_path"],
        "unity_globalgamemanagers_sha256":
            REPO / (
                "research/private/remc2-unity-native/Builds/Linux/"
                "MagicCarpetNative_Data/globalgamemanagers"
            ),
        "native_plugin_file_sha256":
            REPO / "research/private/remc2-unity-native/Assets/Plugins/x86_64/libsim_capi.so",
        "built_native_plugin_file_sha256":
            REPO / (
                "research/private/remc2-unity-native/Builds/Linux/"
                "MagicCarpetNative_Data/Plugins/x86_64/libsim_capi.so"
            ),
    }
    for field, path in files.items():
        require(path.is_file() and not path.is_symlink(), f"missing or unsafe pin: {path}")
        require(sha256_file(path) == pins[field], f"runtime pin mismatch: {field}")
    require(
        pins["native_plugin_file_sha256"]
        == pins["built_native_plugin_file_sha256"],
        "source and built native plugin identities differ",
    )

    package = json.loads(files["package_manifest_file_sha256"].read_text())
    closure = json.loads(files["closure_file_sha256"].read_text())
    cache = json.loads(files["owner_cache_manifest_file_sha256"].read_text())
    require(
        package["package_sha256"] == pins["package_identity_sha256"],
        "package semantic identity mismatch",
    )
    require(
        closure["closure_sha256"] == pins["closure_identity_sha256"],
        "closure semantic identity mismatch",
    )
    require(
        cache["cache_content_sha256"] == pins["owner_cache_content_sha256"],
        "owner-cache semantic identity mismatch",
    )
    require(
        cache["bindings"]["level_export_package_sha256"]
        == pins["package_identity_sha256"],
        "owner cache package binding mismatch",
    )
    require(
        cache["bindings"]["level_asset_closure_sha256"]
        == pins["closure_identity_sha256"],
        "owner cache closure binding mismatch",
    )

    route_suite = next(
        item for item in manifest["suites"]
        if item["id"] == "unity-synthetic-presentation-route"
    )
    reset = route_suite["reset"]
    package_reset = package["reset_projection"]
    expected_state = package_reset["expected_state"]
    require(
        reset["state_hash_fnv1a64"] == package_reset["state_hash"]["hex"],
        "route reset state hash is not bound to the pinned package projection",
    )
    require(
        reset["package_reset_conformance_hash_fnv1a64"]
        == package_reset["conformance_hash"]["hex"],
        "route reset conformance hash is not bound to the pinned package projection",
    )
    require(
        reset["tick"] == expected_state["tick"]
        and reset["position"] == [
            expected_state["position"]["x"],
            expected_state["position"]["y"],
            expected_state["position"]["z"],
        ],
        "route reset state is not bound to the pinned package projection",
    )


def verify_idle_suite(manifest: dict) -> None:
    suite = next(
        item for item in manifest["suites"]
        if item["id"] == "level1-idle-full-frame"
    )
    fixtures = {item["role"]: item for item in suite["fixtures"]}
    fixture_bytes: dict[str, bytes] = {}
    for role, fixture in fixtures.items():
        path = REPO / fixture["path"]
        require(path.is_file() and not path.is_symlink(), f"missing fixture: {path}")
        data = path.read_bytes()
        fixture_bytes[role] = data
        require(sha256_bytes(data) == fixture["sha256"], f"{role} fixture hash mismatch")
        require(
            len(data) == fixture["frame_bytes"] * fixture["frame_count"],
            f"{role} fixture extent mismatch",
        )

    role_to_hash_field = {
        "main_state": "state_frame_sha256",
        "terrain_state": "terrain_frame_sha256",
        "complete_indexed_framebuffer": "original_framebuffer_sha256",
    }
    for checkpoint in suite["checkpoints"]:
        index = checkpoint["reference_frame"]
        for role, hash_field in role_to_hash_field.items():
            frame_bytes = fixtures[role]["frame_bytes"]
            start = index * frame_bytes
            frame = fixture_bytes[role][start:start + frame_bytes]
            require(len(frame) == frame_bytes, f"{role} frame {index} is truncated")
            require(
                sha256_bytes(frame) == checkpoint[hash_field],
                f"{role} frame {index} hash mismatch",
            )

    sim_root = REPO / "research/private/remc2-sim-rs"
    state_path = REPO / fixtures["main_state"]["path"]
    command = [
        "cargo", "run", "-q", "-p", "sim-cli", "--locked", "--",
        "inspect-regression", str(state_path),
    ]
    result = subprocess.run(
        command,
        cwd=sim_root,
        check=True,
        text=True,
        capture_output=True,
    )
    decoded = {
        row["frame"]: row
        for line in result.stdout.splitlines()
        if line.strip()
        for row in [json.loads(line)]
    }
    for checkpoint in suite["checkpoints"]:
        index = checkpoint["reference_frame"]
        row = decoded[index]
        flight = checkpoint["flight"]
        require(row["turn"] == checkpoint["source_turn"], f"turn mismatch at {index}")
        require(row["entity"] == flight["entity_index"], f"entity mismatch at {index}")
        require(row["position"] == flight["position"], f"position mismatch at {index}")
        require(row["yaw"] == flight["yaw"], f"yaw mismatch at {index}")
        require(
            row["display_pitch"] == flight["display_pitch"],
            f"display pitch mismatch at {index}",
        )
        require(
            row["effective_pitch"] == flight["effective_pitch"],
            f"effective pitch mismatch at {index}",
        )
        require(
            row["target_speed"] == flight["target_speed"]
            and row["actual_speed"] == flight["actual_speed"],
            f"speed mismatch at {index}",
        )
        require(
            row["state_hash"] == flight["state_hash_fnv1a64"],
            f"FlightState hash mismatch at {index}",
        )


def verify_supporting_corpus(manifest: dict) -> None:
    suite = next(
        item for item in manifest["suites"]
        if item["id"] == "afterloadtest2-recorded-flight-support"
    )
    for path_field, hash_field in (
        ("state_sequence_path", "state_sequence_sha256"),
        ("recording_path", "recording_sha256"),
    ):
        path = REPO / suite[path_field]
        require(path.is_file() and not path.is_symlink(), f"missing support file: {path}")
        require(sha256_file(path) == suite[hash_field], f"{path_field} hash mismatch")


def verify_active_forward_suite(manifest: dict) -> None:
    idle = next(
        item for item in manifest["suites"]
        if item["id"] == "level1-idle-full-frame"
    )
    active = next(
        item for item in manifest["suites"]
        if item["id"] == "level1-native-eight-forward"
    )
    fixture_paths = {
        item["role"]: REPO / item["path"]
        for item in idle["fixtures"]
    }
    command = [
        "cargo", "run", "-q", "-p", "sim-cli", "--locked", "--",
        "inspect-forward",
        str(fixture_paths["main_state"]),
        str(fixture_paths["terrain_state"]),
    ]
    result = subprocess.run(
        command,
        cwd=REPO / "research/private/remc2-sim-rs",
        check=True,
        text=True,
        capture_output=True,
    )
    rows = [json.loads(line) for line in result.stdout.splitlines() if line.strip()]
    require(len(rows) == len(active["checkpoints"]) == 8, "active checkpoint count")
    for expected, actual in zip(active["checkpoints"], rows, strict=True):
        require(
            expected["checkpoint_id"]
            == f"forward-pass-{actual['native_step_count']:02d}",
            "active checkpoint id mismatch",
        )
        for field in (
            "native_step_count",
            "position",
            "target_speed",
            "speed_changed_flag",
            "actual_speed",
            "entity_phase",
        ):
            require(
                expected[field] == actual[field],
                f"active checkpoint mismatch: {expected['checkpoint_id']} {field}",
            )
        require(
            expected["state_hash_fnv1a64"] == actual["state_hash"],
            f"active state hash mismatch: {expected['checkpoint_id']}",
        )


def verify_first_presented_frame(manifest: dict) -> None:
    suite = next(
        item for item in manifest["suites"]
        if item["id"] == "level1-first-presented-frame-trace"
    )
    checkpoint = suite["checkpoints"][0]
    provenance = checkpoint["provenance"]
    trace_paths = [REPO / provenance["trace_a"], REPO / provenance["trace_b"]]
    qualification_paths = [path.parent / "qualification.json" for path in trace_paths]
    for path in trace_paths:
        require(sha256_file(path) == provenance["trace_sha256"], "trace hash mismatch")
    for path in qualification_paths:
        require(
            sha256_file(path) == provenance["qualification_sha256"],
            "trace qualification hash mismatch",
        )
    require(trace_paths[0].read_bytes() == trace_paths[1].read_bytes(), "trace A/B differ")
    header = json.loads(trace_paths[0].read_text(encoding="utf-8").splitlines()[0])["header"]
    event = checkpoint["event_identity"]
    camera = checkpoint["camera_identity"]
    require(header["event_set"]["count"] == event["count"], "trace event count mismatch")
    require(header["event_set"]["sha256"] == event["sha256"], "trace event hash mismatch")
    require(
        [
            header["camera"]["position"]["x"],
            header["camera"]["position"]["y"],
            header["camera"]["position"]["z"],
        ] == camera["position"],
        "trace camera position mismatch",
    )
    for field in ("entity_index", "yaw", "pitch", "roll"):
        require(header["camera"][field] == camera[field], f"trace camera {field} mismatch")


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
    return bytes((0, 0, 0, roll & 0xff, pitch & 0xff, flags, 0, 0, 0, 0))


def run_native_route_qualification(manifest: dict) -> dict[int, dict]:
    pins = manifest["runtime_pins"]
    player = REPO / "research/private/remc2-unity-native/Builds/Linux/MagicCarpetNative.x86_64"
    project = REPO / "research/private/remc2-unity-native"
    with tempfile.NamedTemporaryFile(
        prefix="magiccarpet-route-corpus-", suffix=".log", delete=False
    ) as stream:
        log_path = Path(stream.name)
    command = [
        str(player),
        "-batchmode",
        "-nographics",
        "-mc-qualified",
        "-mc-self-test",
        "-mc-package",
        str(REPO / "research/private/generated/level1-export-m"),
        "-mc-closure",
        str(REPO / "research/private/generated/level1-asset-closure.json"),
        "-mc-asset-cache",
        str(REPO / "research/private/generated/level1-unity-asset-cache-plan29-final"),
        "-logFile",
        str(log_path),
    ]
    try:
        result = subprocess.run(command, cwd=project, text=True, capture_output=True)
        log = log_path.read_text(encoding="utf-8")
    finally:
        log_path.unlink(missing_ok=True)
    require(result.returncode == 0, f"native route qualification failed\n{result.stdout}")
    require(
        "MC_ROUTE_QUAL PASS checkpoints=5 steps=440" in log
        and "MC_SELF_TEST PASS abi=2" in log,
        "native route qualification markers missing",
    )
    idle_suite = next(
        item for item in manifest["suites"]
        if item["id"] == "level1-idle-full-frame"
    )
    native_idle = idle_suite["native_abi_idle_probe"]["native_step_hashes"]
    source_idle = idle_suite["native_abi_idle_probe"]["source_projection_hashes"]
    expected_idle_probe = (
        "MC_IDLE_QUAL PASS exactReset=true sourceProjectionExact=true "
        f"nativeHashes={native_idle['0']},{native_idle['8']},{native_idle['19']} "
        f"sourceHashes={source_idle['0']},{source_idle['8']},{source_idle['19']}"
    )
    require(expected_idle_probe in log, "native idle probe classification mismatch")
    reset = next(
        item for item in manifest["suites"]
        if item["id"] == "unity-synthetic-presentation-route"
    )["reset"]
    expected_reset = (
        "MC_ROUTE_RESET PASS tick=0 eventHead=0 "
        f"state={reset['state_hash_fnv1a64']} "
        f"sourceProjection={reset['state_hash_fnv1a64']} "
        f"packageConformance={reset['package_reset_conformance_hash_fnv1a64']} "
        f"sessionConformance={reset['live_session_conformance_hash_fnv1a64']} "
        f"sourceTurn={reset['source_turn']} "
        f"position={','.join(str(value) for value in reset['position'])}"
    )
    require(expected_reset in log, "route reset registry identity mismatch")
    records: dict[int, dict] = {}
    for line in log.splitlines():
        marker = "recordJson="
        if "MC_ROUTE_CHECKPOINT " not in line or marker not in line:
            continue
        record = json.loads(line.split(marker, 1)[1])
        if record["snapshot"]["generation"] != 1:
            continue
        tick = record["checkpoint_tick"]
        if tick in records:
            require(records[tick] == record, f"route session mismatch at {tick}")
        records[tick] = record
    require(len(records) == 5, "native route record count mismatch")
    require(
        pins["native_abi"] == 2
        and sha256_file(player) == pins["unity_player_file_sha256"],
        "qualified player identity changed during route run",
    )
    return records


def verify_unity_route(manifest: dict) -> None:
    suite = next(
        item for item in manifest["suites"]
        if item["id"] == "unity-synthetic-presentation-route"
    )
    route = suite["route"]
    stream = b"".join(autoplay_command(index) for index in range(route["final_step_count"]))
    require(
        sha256_bytes(stream) == route["full_command_stream_sha256"],
        "full Unity route command hash mismatch",
    )
    native_records = run_native_route_qualification(manifest)
    for checkpoint in suite["checkpoints"]:
        tick = checkpoint["unity_checkpoint_tick"]
        record = native_records[tick]
        require(
            checkpoint["checkpoint_id"] == f"unity-route-{tick}"
            and checkpoint["last_command_index"] == tick - 1,
            f"Unity checkpoint id/clock mismatch at {tick}",
        )
        input_identity = checkpoint["input_identity"]
        require(
            list(autoplay_command(tick - 1)) == input_identity["last_command_raw_10_bytes"],
            f"Unity last command mismatch at {tick}",
        )
        require(
            sha256_bytes(stream[:tick * 10]) == input_identity["prefix_sha256"]
            == record["command_prefix_sha256"],
            f"Unity command prefix mismatch at {tick}",
        )
        state = checkpoint["state_identity"]
        snapshot = record["snapshot"]
        state_mapping = {
            "generation": "generation",
            "tick": "tick",
            "event_head": "event_head",
            "state_hash_fnv1a64": "state_hash_fnv1a64",
            "conformance_hash_fnv1a64": "conformance_hash_fnv1a64",
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
        for expected_field, actual_field in state_mapping.items():
            require(
                state[expected_field] == snapshot[actual_field],
                f"Unity snapshot {expected_field} mismatch at {tick}",
            )
        require(
            state["position"] == [snapshot["x"], snapshot["y"], snapshot["z"]],
            f"Unity snapshot position mismatch at {tick}",
        )
        require(
            checkpoint["source_turn"] == snapshot["source_turn"],
            f"Unity source turn mismatch at {tick}",
        )
        events = checkpoint["event_identity"]
        require(
            events["generation"] == snapshot["generation"],
            f"event generation mismatch at {tick}",
        )
        for field in (
            "first_sequence", "last_sequence", "count",
            "content_schema", "content_sha256", "lifecycle_schema", "lifecycle_sha256",
        ):
            require(events[field] == record["events"][field], f"event {field} mismatch at {tick}")
        camera = checkpoint["camera_identity"]
        presentation = camera["presentation"]
        actual_presentation = record["presentation"]
        presentation_mapping = {
            "generation": "generation", "tick": "tick", "source_turn": "source_turn",
            "yaw": "camera_yaw", "pitch": "camera_pitch", "roll": "camera_roll",
            "fov": "camera_fov", "authority": "authority", "flags": "flags",
        }
        for expected_field, actual_field in presentation_mapping.items():
            require(
                presentation[expected_field] == actual_presentation[actual_field],
                f"presentation {expected_field} mismatch at {tick}",
            )
        require(
            presentation["position"] == [
                actual_presentation["camera_x"],
                actual_presentation["camera_y"],
                actual_presentation["camera_z"],
            ],
            f"presentation position mismatch at {tick}",
        )
        render = camera["render"]
        actual_render = record["render_camera"]
        render_mapping = {
            "generation": "generation", "tick": "tick", "source_turn": "source_turn",
            "eye_z": "eye_z", "yaw": "yaw", "pitch": "pitch", "roll": "roll",
            "fov": "fov", "authority": "authority", "flags": "flags",
        }
        for expected_field, actual_field in render_mapping.items():
            require(
                render[expected_field] == actual_render[actual_field],
                f"render camera {expected_field} mismatch at {tick}",
            )
        require(
            render["history"] == [actual_render["history_x"], actual_render["history_y"]]
            and render["slope_target"] == [
                actual_render["slope_target_x"], actual_render["slope_target_y"]
            ]
            and render["render_offset"] == [
                actual_render["render_offset_x"], actual_render["render_offset_y"]
            ],
            f"render camera vector mismatch at {tick}",
        )
        terrain = checkpoint["terrain_identity"]
        actual_terrain = record["terrain"]
        terrain_mapping = {
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
            "reflected": "reflected_triangles",
            "mode5": "primary_mode5_triangles",
            "mode26": "primary_mode26_triangles",
            "writes": "terrain_pixel_writes",
        }
        for expected_field, actual_field in terrain_mapping.items():
            require(
                terrain[expected_field] == actual_terrain[actual_field],
                f"terrain {expected_field} mismatch at {tick}",
            )
        require(
            checkpoint["presentation_scope_ref"] in manifest["presentation_scopes"],
            f"unknown presentation scope at {tick}",
        )


def verify_checkpoint_contracts(manifest: dict) -> None:
    seen: set[str] = set()
    for suite in manifest["suites"]:
        for checkpoint in suite.get("checkpoints", []):
            checkpoint_id = checkpoint.get("checkpoint_id")
            require(bool(checkpoint_id) and checkpoint_id not in seen, "checkpoint id contract")
            seen.add(checkpoint_id)
            for field in (
                "state_identity",
                "event_identity",
                "camera_identity",
                "presentation_scope_ref",
                "provenance",
            ):
                require(field in checkpoint, f"{checkpoint_id} missing {field}")
            require(
                checkpoint["presentation_scope_ref"] in manifest["presentation_scopes"],
                f"{checkpoint_id} presentation scope missing",
            )

    event_contract = manifest["event_hash_contract"]
    require(event_contract == {
        "algorithm": "SHA-256",
        "byte_order": "little-endian",
        "concatenation": (
            "events in strictly increasing sequence order within the checkpoint "
            "range, with no count or framing bytes"
        ),
        "content_schema": "mc2-native-event-logical-content-le/sha256-v1",
        "content_ordered_fields": [
            "sequence:u64",
            "state_hash:u64",
            "kind:u32",
            "code:u32",
            "entity_index:u16",
            "player_index:i16",
            "value0:i32",
            "value1:i32",
            "asset_ref_index:u32",
        ],
        "lifecycle_schema": "mc2-native-event-logical-lifecycle-le/sha256-v1",
        "lifecycle_rule": (
            "prepend generation:u64 to every event before the content fields"
        ),
        "excluded": ["abi_version", "struct_size", "reserved ABI bytes"],
    }, "event hash contract mismatch")


def run_focused_sim_test(test_name: str) -> None:
    sim_root = REPO / "research/private/remc2-sim-rs"
    result = subprocess.run(
        [
            "cargo", "test", "-q", "-p", "sim-core", "--locked",
            test_name, "--", "--exact",
        ],
        cwd=sim_root,
        text=True,
        capture_output=True,
    )
    require(
        result.returncode == 0,
        f"focused sim-core qualification failed: {test_name}\n"
        f"{result.stdout}{result.stderr}",
    )
    require(
        "1 passed" in result.stdout,
        f"focused sim-core qualification did not execute exactly one test: {test_name}",
    )


def main() -> None:
    manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    require(
        manifest.get("schema") == "mc2-private-level1-viewer-reference-corpus/v1",
        "reference-corpus schema mismatch",
    )
    require(
        manifest["reference_build"]["capture_hook"] == "0x002285ff",
        "capture hook mismatch",
    )
    verify_runtime_pins(manifest)
    verify_checkpoint_contracts(manifest)
    verify_idle_suite(manifest)
    verify_first_presented_frame(manifest)
    verify_active_forward_suite(manifest)
    verify_supporting_corpus(manifest)
    verify_unity_route(manifest)
    run_focused_sim_test(
        "regression_state::tests::"
        "owned_native_four_and_eight_pass_forward_traces_match_every_pass"
    )
    run_focused_sim_test(
        "regression_state::tests::"
        "owned_after_load_recording_matches_twenty_four_complete_flight_transitions"
    )
    print(
        "MC_LEVEL1_REFERENCE_CORPUS PASS "
        "idle=3 active_forward=8 supporting_transitions=24 "
        "unity_regression=5 classifications=explicit"
    )


if __name__ == "__main__":
    main()
