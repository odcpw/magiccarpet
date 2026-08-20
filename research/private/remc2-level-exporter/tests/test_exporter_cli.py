#!/usr/bin/env python3
"""Focused production-path tests for the owner-facing level selector.

The parser lives in ``src/level_selection.h`` and is included by the exporter
itself. This test compiles a tiny harness against that same header, then (when
the built exporter is available) checks that invalid CLI input fails before an
output or staging directory can appear.
"""

from __future__ import annotations

import os
import copy
import hashlib
from pathlib import Path
import shutil
import subprocess
import tempfile
import json
import sys


sys.path.insert(0, str(Path(__file__).resolve().parent))
from verify_package import (
    BANISHMENT_STATE_LAYOUT,
    LEVEL2_COLLISION_MASK_BANK_BYTES,
    LEVEL2_COLLISION_MASK_BANK_SHA256,
    LEVEL2_PLAYER_COMPONENT_VERSION,
    LEVEL2_PLAYER_RECORD_BYTES,
    LEVEL2_V4_PLAYER_COMPONENT_VERSION,
    LEVEL2_V4_PLAYER_RECORD_BYTES,
    SCHEMA_V3,
    SCHEMA_V4,
    SCHEMA_V5,
    TYPE_STR_164_AUXILIARY_LAYOUT,
    verify,
)


ROOT = Path(__file__).resolve().parents[4]
EXPORTER_SOURCE = ROOT / "research/private/remc2-level-exporter/src"


def compile_selector_harness(directory: Path) -> Path:
    source = directory / "selector_harness.cpp"
    source.write_text(
        """
#include \"level_selection.h\"
#include <iostream>
#include <string>

int main(int argc, char** argv) {
    for (int i = 1; i < argc; ++i) {
        try {
            const auto selection = mc2_exporter::parse_external_level(argv[i]);
            std::cout << selection.external << ":" << selection.internal << "\\n";
        } catch (const std::exception& error) {
            std::cerr << error.what() << "\\n";
            return 1;
        }
    }
}
""",
        encoding="utf-8",
    )
    binary = directory / "selector_harness"
    compiler = os.environ.get("CXX", "c++")
    subprocess.run(
        [compiler, "-std=c++17", "-Wall", "-Wextra", "-Werror", "-I", str(EXPORTER_SOURCE), str(source), "-o", str(binary)],
        check=True,
        capture_output=True,
        text=True,
    )
    return binary


def test_selector_mapping_and_rejection() -> None:
    with tempfile.TemporaryDirectory(prefix="mc2-exporter-selector-") as temporary:
        binary = compile_selector_harness(Path(temporary))
        mapped = subprocess.run(
            [str(binary), "1", "2", "25"],
            check=True,
            capture_output=True,
            text=True,
        )
        assert mapped.stdout.splitlines() == ["1:0", "2:1", "25:24"]

        for invalid in ("", "0", "26", "-1", "+1", "1x", " 2", "2 ", "999999999999999999999"):
            rejected = subprocess.run(
                [str(binary), invalid],
                capture_output=True,
                text=True,
            )
            assert rejected.returncode != 0, invalid


def test_invalid_cli_has_no_partial_output() -> None:
    exporter = Path(
        os.environ.get(
            "MC2_LEVEL_EXPORTER_BIN",
            "/tmp/magiccarpet-remc2-level-exporter-build/remc2-level-exporter",
        )
    )
    if not exporter.is_file():
        print(
            "SKIP production CLI negative tests: exporter binary is absent; "
            "set MC2_LEVEL_EXPORTER_BIN after a clean pinned build",
        )
        return
    owner_root = Path(
        os.environ.get(
            "MC2_OWNER_ROOT",
            str(ROOT / "research/private/remc2-native-runtime"),
        )
    )
    with tempfile.TemporaryDirectory(prefix="mc2-exporter-invalid-") as temporary:
        parent = Path(temporary)
        for invalid in ("0", "26", "-1", "1x"):
            output = parent / f"output-{invalid.replace('-', 'negative-')}"
            result = subprocess.run(
                [str(exporter), "--owner-root", str(owner_root), "--output", str(output), "--level", invalid],
                capture_output=True,
                text=True,
            )
            assert result.returncode != 0, (invalid, result.stdout, result.stderr)
            assert not output.exists(), invalid
            assert not Path(f"{output}.staging").exists(), invalid

        duplicate_output = parent / "duplicate"
        duplicate = subprocess.run(
            [
                str(exporter),
                "--owner-root",
                str(owner_root),
                "--output",
                str(duplicate_output),
                "--level",
                "1",
                "--level",
                "1",
            ],
            capture_output=True,
            text=True,
        )
        assert duplicate.returncode != 0
        assert not duplicate_output.exists()
        assert not Path(f"{duplicate_output}.staging").exists()


def _tree_bytes(root: Path) -> dict[str, bytes]:
    return {
        path.relative_to(root).as_posix(): path.read_bytes()
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


def _expect_level2_rejected(package: Path, mutate) -> None:
    with tempfile.TemporaryDirectory(prefix="mc2-level2-negative-") as temporary:
        candidate = Path(temporary) / "package"
        shutil.copytree(package, candidate)
        candidate.chmod(0o700)
        for path in candidate.iterdir():
            path.chmod(0o600)
        mutate(candidate)
        try:
            verify(candidate, enforce_reference_hashes=False)
        except (AssertionError, KeyError, OSError, json.JSONDecodeError, ValueError):
            return
        raise AssertionError("tampered Level 2 package was accepted")


def _mutate_manifest(candidate: Path, operation) -> None:
    path = candidate / "manifest.json"
    manifest = json.loads(path.read_text(encoding="utf-8"))
    operation(manifest)
    path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")


def _refresh_package_hash(candidate: Path) -> None:
    path = candidate / "manifest.json"
    manifest = json.loads(path.read_text(encoding="utf-8"))
    rows = []
    for name in sorted(manifest["files"]):
        entry = manifest["files"][name]
        rows.append(f"{name}\0{entry['sha256']}\0{entry['bytes']}\n")
    manifest["package_sha256"] = hashlib.sha256("".join(rows).encode()).hexdigest()
    path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")


def _stable_l2_manifest(manifest: dict) -> dict:
    stable = copy.deepcopy(manifest)
    # The v4/v5 extensions are explicit: only their schema, collision-mask
    # member, Type_str_164/banishment layouts and references, and player
    # extents are removed before comparing the stable v3 contract.
    stable["schema"] = SCHEMA_V3
    stable.pop("collision_masks", None)
    stable.pop("package_sha256", None)
    stable.pop("reset_projection", None)
    stable["record_sizes"]["player"] = 573
    stable["record_layouts"]["player"]["record_bytes"] = 573
    stable["record_layouts"]["player"].pop("type_str_164_auxiliary", None)
    stable["record_layouts"]["player"].pop("banishment_state_0x006", None)
    reference = stable.get("reference", {})
    reference.pop("type_str_164_auxiliary", None)
    reference.pop("banishment_state_0x006", None)
    reference.pop("terrain_frame0_sha256", None)
    reference.get("terrain_sequence", {}).pop("sha256", None)
    for plane in reference.get("terrain_planes", []):
        if plane["name"] == "secondary_cave":
            plane.pop("exported_sha256", None)
            plane.pop("source_status", None)
            plane.pop("normalization", None)
    stable.get("normalization", {}).pop("volatile_host_state", None)
    feature_generation = stable.get("feature_generation_oracle", {})
    feature_generation.get("pre_terrain", {}).pop("inactive_secondary_plane", None)
    feature_generation.get("post_terrain", {}).pop("inactive_secondary_plane", None)
    for name, entry in stable.get("files", {}).items():
        if name in {"terrain.bin", "terrain-pre-features.bin", "terrain-post-features.bin"}:
            entry.pop("sha256", None)
    stable.get("files", {}).pop("players.bin", None)
    stable.get("files", {}).pop("collision-masks.bin", None)
    source = stable.get("source", {})
    source.pop("compiled_source_sha256", None)
    source["compiled_files"] = [
        entry
        for entry in source.get("compiled_files", [])
        if entry["path"] != "private-exporter/src/exporter_main.cpp"
    ]
    source["owner_files"] = [
        entry
        for entry in source.get("owner_files", [])
        if entry["logical_path"]
        not in {"CD_Files/DATA/MSPRD0-0.DAT", "CD_Files/DATA/MSPRD0-0.TAB"}
    ]
    return stable


def _assert_legacy_player_prefixes(fresh: Path, preserved: Path) -> None:
    fresh_players = (fresh / "players.bin").read_bytes()
    preserved_players = (preserved / "players.bin").read_bytes()
    assert fresh_players[:4] == preserved_players[:4] == b"MCP1"
    assert int.from_bytes(fresh_players[8:12], "little") == 8
    assert int.from_bytes(preserved_players[8:12], "little") == 8
    assert int.from_bytes(fresh_players[4:6], "little") == LEVEL2_PLAYER_COMPONENT_VERSION
    assert int.from_bytes(fresh_players[6:8], "little") == LEVEL2_PLAYER_RECORD_BYTES
    assert int.from_bytes(preserved_players[4:6], "little") == 2
    assert int.from_bytes(preserved_players[6:8], "little") == 573
    for index in range(8):
        fresh_offset = 12 + index * LEVEL2_PLAYER_RECORD_BYTES
        preserved_offset = 12 + index * 573
        assert (
            fresh_players[fresh_offset : fresh_offset + 573]
            == preserved_players[preserved_offset : preserved_offset + 573]
        ), f"Level 2 player {index} legacy prefix differs"


def _assert_l2_v5_player_extension(fresh: Path, preserved_v4: Path) -> None:
    fresh_players = (fresh / "players.bin").read_bytes()
    v4_players = (preserved_v4 / "players.bin").read_bytes()
    assert fresh_players[:4] == v4_players[:4] == b"MCP1"
    assert int.from_bytes(fresh_players[4:6], "little") == LEVEL2_PLAYER_COMPONENT_VERSION
    assert int.from_bytes(fresh_players[6:8], "little") == LEVEL2_PLAYER_RECORD_BYTES
    assert int.from_bytes(v4_players[4:6], "little") == LEVEL2_V4_PLAYER_COMPONENT_VERSION
    assert int.from_bytes(v4_players[6:8], "little") == LEVEL2_V4_PLAYER_RECORD_BYTES
    assert int.from_bytes(fresh_players[8:12], "little") == int.from_bytes(v4_players[8:12], "little") == 8
    assert [
        fresh_players[12 + index * LEVEL2_PLAYER_RECORD_BYTES + 662]
        for index in range(8)
    ] == [1, 1, 0, 0, 0, 0, 0, 0]
    assert [
        int.from_bytes(
            fresh_players[
                12 + index * LEVEL2_PLAYER_RECORD_BYTES + 6 :
                12 + index * LEVEL2_PLAYER_RECORD_BYTES + 8
            ],
            "little",
            signed=True,
        )
        for index in range(8)
    ] == [31] * 8
    for index in range(8):
        fresh_offset = 12 + index * LEVEL2_PLAYER_RECORD_BYTES
        v4_offset = 12 + index * LEVEL2_V4_PLAYER_RECORD_BYTES
        assert (
            fresh_players[fresh_offset : fresh_offset + LEVEL2_V4_PLAYER_RECORD_BYTES]
            == v4_players[v4_offset : v4_offset + LEVEL2_V4_PLAYER_RECORD_BYTES]
        ), f"Level 2 player {index} changed before appended banishment byte"
        assert len(fresh_players[fresh_offset : fresh_offset + LEVEL2_PLAYER_RECORD_BYTES]) == 663


def _assert_l2_v5_upgrade(fresh: Path, preserved_v4: Path) -> None:
    fresh_manifest = json.loads((fresh / "manifest.json").read_text(encoding="utf-8"))
    v4_manifest = json.loads((preserved_v4 / "manifest.json").read_text(encoding="utf-8"))
    assert fresh_manifest["schema"] == SCHEMA_V5
    assert v4_manifest["schema"] == SCHEMA_V4
    assert fresh_manifest["record_layouts"]["player"]["type_str_164_auxiliary"] == TYPE_STR_164_AUXILIARY_LAYOUT
    assert fresh_manifest["record_layouts"]["player"]["banishment_state_0x006"] == BANISHMENT_STATE_LAYOUT
    assert fresh_manifest["record_layouts"]["player"]["banishment_state_0x006"]["record_offset"] == 662
    assert fresh_manifest["record_layouts"]["player"]["banishment_state_0x006"]["record_bytes"] == 1
    assert set(fresh_manifest["files"]) == set(v4_manifest["files"])
    for name in sorted(fresh_manifest["files"]):
        if name == "players.bin":
            continue
        assert (fresh / name).read_bytes() == (preserved_v4 / name).read_bytes(), (
            f"Level 2 v5 migration changed unrelated member {name}"
        )
    _assert_l2_v5_player_extension(fresh, preserved_v4)


def _assert_l2_v5_package_matches(fresh: Path, preserved: Path) -> None:
    """Keep Level 2 emitted bytes sealed while allowing honest source provenance."""

    fresh_manifest = json.loads((fresh / "manifest.json").read_text(encoding="utf-8"))
    preserved_manifest = json.loads((preserved / "manifest.json").read_text(encoding="utf-8"))
    assert fresh_manifest["files"] == preserved_manifest["files"]
    assert fresh_manifest["package_sha256"] == preserved_manifest["package_sha256"]
    for name in sorted(fresh_manifest["files"]):
        assert (fresh / name).read_bytes() == (preserved / name).read_bytes(), (
            f"Level 2 emitted member drifted: {name}"
        )

    fresh_source = copy.deepcopy(fresh_manifest["source"])
    preserved_source = copy.deepcopy(preserved_manifest["source"])
    fresh_compiled = {
        entry["path"]: entry["sha256"] for entry in fresh_source.pop("compiled_files")
    }
    preserved_compiled = {
        entry["path"]: entry["sha256"] for entry in preserved_source.pop("compiled_files")
    }
    fresh_source.pop("compiled_source_sha256")
    preserved_source.pop("compiled_source_sha256")
    assert fresh_source == preserved_source
    changed_path = "private-exporter/src/exporter_main.cpp"
    assert set(fresh_compiled) == set(preserved_compiled)
    assert {
        path: digest for path, digest in fresh_compiled.items() if path != changed_path
    } == {
        path: digest for path, digest in preserved_compiled.items() if path != changed_path
    }
    assert fresh_compiled[changed_path] != preserved_compiled[changed_path]


def _assert_legacy_terrain_semantics(fresh: Path, preserved: Path) -> None:
    for name in ("terrain.bin", "terrain-pre-features.bin", "terrain-post-features.bin"):
        fresh_terrain = (fresh / name).read_bytes()
        preserved_terrain = (preserved / name).read_bytes()
        assert len(fresh_terrain) == len(preserved_terrain) == 0x70000
        for start, end in ((0x00000, 0x10000), (0x10000, 0x20000),
                           (0x20000, 0x30000), (0x30000, 0x40000),
                           (0x50000, 0x70000)):
            assert fresh_terrain[start:end] == preserved_terrain[start:end], (
                f"{name} active terrain plane {start:#x}:{end:#x} differs"
            )
        assert fresh_terrain[0x40000:0x50000] == bytes(0x10000), (
            f"{name} non-Cave inactive terrain plane is not zero-normalized"
        )


def _assert_l2_corpus_matches(fresh: Path, preserved: Path) -> None:
    fresh_manifest = json.loads((fresh / "manifest.json").read_text(encoding="utf-8"))
    preserved_manifest = json.loads(
        (preserved / "manifest.json").read_text(encoding="utf-8")
    )
    assert preserved_manifest["schema"] == SCHEMA_V3
    assert preserved_manifest["level"] == {
        "external": 2,
        "internal": 1,
        "map_type": {"id": 0, "name": "Day"},
    }
    preserved_secondary = next(
        plane
        for plane in preserved_manifest["reference"]["terrain_planes"]
        if plane["name"] == "secondary_cave"
    )
    assert preserved_secondary["source_status"] == "live_plane"
    assert preserved_secondary["normalization"] == "preserved"
    if _stable_l2_manifest(fresh_manifest) != _stable_l2_manifest(preserved_manifest):
        raise AssertionError(f"Level 2 manifest differs from preserved corpus: {preserved}")
    fresh_files = set(fresh_manifest["files"]) - {"collision-masks.bin"}
    preserved_files = set(preserved_manifest["files"])
    if fresh_files != preserved_files:
        raise AssertionError(
            f"Level 2 member set differs from preserved corpus: {preserved}"
        )
    for name in sorted(fresh_files):
        if name == "players.bin":
            _assert_legacy_player_prefixes(fresh, preserved)
            continue
        if name in {"terrain.bin", "terrain-pre-features.bin", "terrain-post-features.bin"}:
            _assert_legacy_terrain_semantics(fresh, preserved)
            continue
        if (fresh / name).read_bytes() != (preserved / name).read_bytes():
            raise AssertionError(
                f"Level 2 stable payload differs for {name}: {preserved}"
            )


def test_deterministic_level_profiles() -> None:
    """Run the full production exporter gate only when explicitly requested.

    Four native captures are intentionally opt-in because each capture starts
    remc2 and can be long. The opt-in receipt compares the complete trees for
    default/explicit Level 1 and for two independent Level 2 exports.
    """

    exporter = Path(
        os.environ.get(
            "MC2_LEVEL_EXPORTER_BIN",
            "/tmp/magiccarpet-remc2-level-exporter-build/remc2-level-exporter",
        )
    )
    if os.environ.get("MC2_LEVEL_EXPORTER_RUN_FULL") != "1":
        print("SKIP full exporter determinism gate: set MC2_LEVEL_EXPORTER_RUN_FULL=1")
        return
    if not exporter.is_file():
        raise AssertionError(
            "MC2_LEVEL_EXPORTER_RUN_FULL=1 requires MC2_LEVEL_EXPORTER_BIN from a "
            "clean pinned remc2 build"
        )
    owner_root = Path(
        os.environ.get(
            "MC2_OWNER_ROOT",
            str(ROOT / "research/private/remc2-native-runtime"),
        )
    )
    generated_root = ROOT / "research/private/generated"
    generated_root.mkdir(parents=True, exist_ok=True)
    scratch = Path(tempfile.mkdtemp(prefix="mc2-exporter-cli-", dir=generated_root))
    try:
        default = scratch / "default"
        explicit_level_one = scratch / "explicit-level-one"
        level_two_a = scratch / "level-two-a"
        level_two_b = scratch / "level-two-b"

        def run_export(destination: Path, *extra: str) -> None:
            subprocess.run(
                [
                    str(exporter),
                    "--owner-root",
                    str(owner_root),
                    "--output",
                    str(destination),
                    *extra,
                ],
                check=True,
                capture_output=True,
                text=True,
            )

        run_export(default)
        run_export(explicit_level_one, "--level", "1")
        run_export(level_two_a, "--level", "2")
        run_export(level_two_b, "--level", "2")

        assert _tree_bytes(default) == _tree_bytes(explicit_level_one)
        assert _tree_bytes(level_two_a) == _tree_bytes(level_two_b)
        level_two_manifest = json.loads((level_two_a / "manifest.json").read_text())
        assert level_two_manifest["level"] == {
            "external": 2,
            "internal": 1,
            "map_type": {"id": 0, "name": "Day"},
        }
        assert level_two_manifest["schema"] == SCHEMA_V5
        collision_member = level_two_manifest["files"]["collision-masks.bin"]
        assert collision_member["bytes"] == LEVEL2_COLLISION_MASK_BANK_BYTES
        assert collision_member["sha256"] == LEVEL2_COLLISION_MASK_BANK_SHA256
        assert level_two_manifest["collision_masks"]["magic"] == "MCM1"
        assert level_two_manifest["collision_masks"]["vga_resolution_selector"] == 1
        assert level_two_manifest["collision_masks"]["selectors"]
        assert "reset_projection" not in level_two_manifest
        assert level_two_manifest["record_sizes"]["player"] == LEVEL2_PLAYER_RECORD_BYTES
        assert (
            level_two_manifest["record_layouts"]["player"]["type_str_164_auxiliary"]
            == TYPE_STR_164_AUXILIARY_LAYOUT
        )
        assert (
            level_two_manifest["record_layouts"]["player"]["banishment_state_0x006"]
            == BANISHMENT_STATE_LAYOUT
        )
        players = (level_two_a / "players.bin").read_bytes()
        assert int.from_bytes(players[4:6], "little") == LEVEL2_PLAYER_COMPONENT_VERSION
        assert int.from_bytes(players[6:8], "little") == LEVEL2_PLAYER_RECORD_BYTES
        assert len(players) == 12 + 8 * LEVEL2_PLAYER_RECORD_BYTES

        # Exercise the production verifier against both independent native
        # captures, then prove that profile relabelling and package-member
        # changes fail closed rather than relying on a fixture-only check.
        verify(level_two_a, enforce_reference_hashes=False)
        verify(level_two_b, enforce_reference_hashes=False)
        for preserved in (
            generated_root / "l2-debug3.fJ3PaM/level2",
            generated_root / "l2-debug3.fJ3PaM-v4/level2",
            generated_root / "mc2-l2-native2.Ibpz9b/out",
        ):
            if not preserved.is_dir():
                raise AssertionError(f"preserved Level 2 corpus is missing: {preserved}")
            preserved_manifest = json.loads((preserved / "manifest.json").read_text())
            if preserved_manifest["schema"] == SCHEMA_V3:
                _assert_l2_corpus_matches(level_two_a, preserved)
            elif preserved_manifest["schema"] == SCHEMA_V4:
                verify(preserved, enforce_reference_hashes=False)
                _assert_l2_v5_upgrade(level_two_a, preserved)
            elif preserved_manifest["schema"] == SCHEMA_V5:
                verify(preserved, enforce_reference_hashes=False)
                _assert_l2_v5_package_matches(level_two_a, preserved)
            else:
                raise AssertionError(
                    f"unsupported preserved Level 2 corpus schema: {preserved_manifest['schema']}"
                )
        def relabel_profile(candidate: Path) -> None:
            _mutate_manifest(
                candidate,
                lambda manifest: manifest["level"].update(
                    {"internal": 0, "map_type": {"id": 1, "name": "Night"}}
                ),
            )
            # package_sha256 is derived from the file identity rows rather than
            # the manifest itself; recompute it explicitly so this negative
            # proves the Level 2 profile check, not stale package metadata.
            _refresh_package_hash(candidate)

        _expect_level2_rejected(level_two_a, relabel_profile)
        def tamper_active_entity_count(candidate: Path) -> None:
            _mutate_manifest(
                candidate,
                lambda manifest: manifest["counts"].update({"active_entities": 113}),
            )
            _refresh_package_hash(candidate)

        _expect_level2_rejected(level_two_a, tamper_active_entity_count)

        def tamper_collision_mask_payload(candidate: Path) -> None:
            path = candidate / "collision-masks.bin"
            content = bytearray(path.read_bytes())
            content[-1] ^= 1
            path.write_bytes(content)
            path.chmod(0o600)
            _mutate_manifest(
                candidate,
                lambda manifest: manifest["files"]["collision-masks.bin"].update(
                    {"bytes": len(content), "sha256": hashlib.sha256(content).hexdigest()}
                ),
            )
            _refresh_package_hash(candidate)

        _expect_level2_rejected(level_two_a, tamper_collision_mask_payload)

        def tamper_selector_64_with_refreshed_identities(candidate: Path) -> None:
            manifest_path = candidate / "manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            path = candidate / "collision-masks.bin"
            content = bytearray(path.read_bytes())
            count = int.from_bytes(content[12:16], "little")
            cursor = 20
            found = False
            for row_index in range(count):
                selector = int.from_bytes(content[cursor : cursor + 2], "little")
                pixel_bytes = int.from_bytes(content[cursor + 8 : cursor + 12], "little")
                payload_start = cursor + 12
                payload_end = payload_start + pixel_bytes
                if selector == 64:
                    content[payload_end - 1] ^= 1
                    manifest["collision_masks"]["selectors"][row_index]["sha256"] = (
                        hashlib.sha256(content[payload_start:payload_end]).hexdigest()
                    )
                    found = True
                    break
                cursor = payload_end
            assert found, "qualified Level 2 carrier must contain selector 64"
            path.write_bytes(content)
            path.chmod(0o600)
            manifest["files"]["collision-masks.bin"].update(
                {"bytes": len(content), "sha256": hashlib.sha256(content).hexdigest()}
            )
            manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
            _refresh_package_hash(candidate)

        # Refresh the member, selector-row, and aggregate package identities.
        # Only the immutable source bank identity should reject this mutation.
        _expect_level2_rejected(level_two_a, tamper_selector_64_with_refreshed_identities)

        def tamper_collision_mask_header(candidate: Path) -> None:
            path = candidate / "collision-masks.bin"
            content = bytearray(path.read_bytes())
            content[8:10] = (2).to_bytes(2, "little")
            path.write_bytes(content)
            path.chmod(0o600)
            _mutate_manifest(
                candidate,
                lambda manifest: manifest["files"]["collision-masks.bin"].update(
                    {"bytes": len(content), "sha256": hashlib.sha256(content).hexdigest()}
                ),
            )
            _refresh_package_hash(candidate)

        _expect_level2_rejected(level_two_a, tamper_collision_mask_header)

        def tamper_collision_mask_extent(candidate: Path) -> None:
            path = candidate / "collision-masks.bin"
            content = bytearray(path.read_bytes())
            content[28:32] = (6).to_bytes(4, "little")
            path.write_bytes(content)
            path.chmod(0o600)
            _mutate_manifest(
                candidate,
                lambda manifest: manifest["files"]["collision-masks.bin"].update(
                    {"bytes": len(content), "sha256": hashlib.sha256(content).hexdigest()}
                ),
            )
            _refresh_package_hash(candidate)

        _expect_level2_rejected(level_two_a, tamper_collision_mask_extent)

        def tamper_collision_mask_metadata(candidate: Path) -> None:
            _mutate_manifest(
                candidate,
                lambda manifest: manifest["collision_masks"].update(
                    {"vga_resolution_selector": 2}
                ),
            )
            _refresh_package_hash(candidate)

        _expect_level2_rejected(level_two_a, tamper_collision_mask_metadata)

        def tamper_binary_u32(candidate: Path, name: str, offset: int, value: int) -> None:
            path = candidate / name
            content = bytearray(path.read_bytes())
            content[offset : offset + 4] = value.to_bytes(4, "little")
            path.write_bytes(content)
            _mutate_manifest(
                candidate,
                lambda manifest: manifest["files"][name].update(
                    {"bytes": len(content), "sha256": hashlib.sha256(content).hexdigest()}
                ),
            )
            _refresh_package_hash(candidate)

        # Refresh identities for each payload mutation so rejection exercises
        # the semantic relation checks rather than stale file hashes.
        _expect_level2_rejected(
            level_two_a,
            lambda candidate: tamper_binary_u32(candidate, "players.bin", 12 + 4, 1000),
        )
        _expect_level2_rejected(
            level_two_a,
            lambda candidate: tamper_binary_u32(candidate, "entities.bin", 12 + 4, 1001),
        )
        _expect_level2_rejected(
            level_two_a,
            lambda candidate: tamper_binary_u32(candidate, "runtime.bin", 206 + 10, 1001),
        )
        _expect_level2_rejected(
            level_two_a,
            lambda candidate: (candidate / "unexpected.bin").write_bytes(b"x"),
        )
        def tamper_terrain(candidate: Path) -> None:
            path = candidate / "terrain.bin"
            content = bytearray(path.read_bytes())
            content[0] ^= 1
            path.write_bytes(content)
            path.chmod(0o600)
            _mutate_manifest(
                candidate,
                lambda manifest: manifest["files"]["terrain.bin"].update(
                    {
                        "bytes": len(content),
                        "sha256": hashlib.sha256(content).hexdigest(),
                    }
                ),
            )
            _refresh_package_hash(candidate)
        _expect_level2_rejected(level_two_a, tamper_terrain)
    finally:
        shutil.rmtree(scratch)


if __name__ == "__main__":
    test_selector_mapping_and_rejection()
    test_invalid_cli_has_no_partial_output()
    test_deterministic_level_profiles()
