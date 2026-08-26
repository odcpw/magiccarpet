#!/usr/bin/env python3
"""Positive and negative smoke tests for verify_package.py."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import stat
import tempfile

from verify_package import (
    CAMPAIGN_PACKAGE_SCHEMA,
    LEVEL2_PLAYER_RECORD_BYTES,
    LEVEL2_V4_PLAYER_RECORD_BYTES,
    SCHEMA_V2,
    SCHEMA_V3,
    SCHEMA_V4,
    SCHEMA_V5,
    _campaign_package_identity,
    _canonical_json_bytes,
    verify,
)


def expect_rejected(
    package: Path,
    mutate,
    *,
    semantic: bool = False,
    expected_error: str | None = None,
) -> None:
    with tempfile.TemporaryDirectory(prefix="mc2-exporter-negative-") as temporary:
        candidate = Path(temporary) / "package"
        shutil.copytree(package, candidate)
        candidate.chmod(0o700)
        for path in candidate.iterdir():
            path.chmod(0o600)
        mutate(candidate)
        try:
            verify(candidate, enforce_reference_hashes=not semantic)
        except (AssertionError, KeyError, OSError, json.JSONDecodeError) as error:
            if expected_error is not None and expected_error not in str(error):
                raise AssertionError(
                    f"wrong rejection: expected {expected_error!r}, got {error!r}"
                ) from error
            return
        raise AssertionError("tampered package was accepted")


def refresh_file_identity(candidate: Path, relative_name: str) -> None:
    manifest_path = candidate / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    path = candidate / relative_name
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    manifest["files"][relative_name] = {
        "bytes": path.stat().st_size,
        "sha256": digest,
    }
    rows = []
    for name in sorted(manifest["files"]):
        entry = manifest["files"][name]
        rows.append(f"{name}\0{entry['sha256']}\0{entry['bytes']}\n")
    manifest["package_sha256"] = hashlib.sha256("".join(rows).encode()).hexdigest()
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")


def mutate_manifest(candidate: Path, operation) -> None:
    path = candidate / "manifest.json"
    manifest = json.loads(path.read_text(encoding="utf-8"))
    operation(manifest)
    path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")


def mutate_i16(candidate: Path, name: str, offset: int, value: int) -> None:
    path = candidate / name
    content = bytearray(path.read_bytes())
    content[offset : offset + 2] = value.to_bytes(2, "little", signed=True)
    path.write_bytes(content)
    path.chmod(stat.S_IRUSR | stat.S_IWUSR)
    refresh_file_identity(candidate, name)


def mutate_u16(candidate: Path, name: str, offset: int, value: int) -> None:
    path = candidate / name
    content = bytearray(path.read_bytes())
    content[offset : offset + 2] = value.to_bytes(2, "little")
    path.write_bytes(content)
    path.chmod(stat.S_IRUSR | stat.S_IWUSR)
    refresh_file_identity(candidate, name)


def rewrite_campaign_definition(candidate: Path, operation) -> None:
    path = candidate / "campaign-definition.json"
    definition = json.loads(path.read_text(encoding="utf-8"))
    operation(definition)
    path.write_bytes(_canonical_json_bytes(definition))
    refresh_file_identity(candidate, "campaign-definition.json")


def cross_bind_owner_manifest(candidate: Path) -> None:
    path = candidate / "owner-manifest.json"
    owner_manifest = json.loads(path.read_text(encoding="utf-8"))
    owner_manifest["files"]["players.bin"]["sha256"] = "0" * 64
    owner_manifest["package_sha256"] = _campaign_package_identity(
        owner_manifest["files"]
    )
    path.write_text(json.dumps(owner_manifest, indent=2) + "\n", encoding="utf-8")
    outer_manifest_path = candidate / "manifest.json"
    outer_manifest = json.loads(outer_manifest_path.read_text(encoding="utf-8"))
    outer_manifest["owner_export_package_sha256"] = owner_manifest["package_sha256"]
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    outer_manifest["files"]["owner-manifest.json"] = {
        "bytes": path.stat().st_size,
        "sha256": digest,
    }
    outer_manifest["package_sha256"] = _campaign_package_identity(
        outer_manifest["files"]
    )
    outer_manifest_path.write_text(
        json.dumps(outer_manifest, indent=2) + "\n",
        encoding="utf-8",
    )


def verify_level1_campaign_negatives(package: Path) -> None:
    expect_rejected(
        package,
        lambda candidate: rewrite_campaign_definition(
            candidate,
            lambda definition: definition["campaign_spec"]["reset_sources"][
                "players"
            ].update({"schema": "mc2-player-arena/v4"}),
        ),
        expected_error="schema differs",
    )
    expect_rejected(
        package,
        lambda candidate: rewrite_campaign_definition(
            candidate,
            lambda definition: definition["campaign_spec"]["reset_sources"][
                "players"
            ].update({"record_bytes": LEVEL2_PLAYER_RECORD_BYTES}),
        ),
        expected_error="record width differs",
    )

    def tamper_owner_manifest(candidate: Path) -> None:
        path = candidate / "owner-manifest.json"
        path.write_bytes(path.read_bytes() + b" ")

    expect_rejected(
        package,
        tamper_owner_manifest,
        expected_error="campaign member owner-manifest.json size/hash mismatch",
    )
    expect_rejected(
        package,
        cross_bind_owner_manifest,
        expected_error="owner manifest files differ from the outer campaign member map",
    )

    def inject_collision_member(candidate: Path) -> None:
        (candidate / "collision-masks.bin").write_bytes(b"MCM1")

    expect_rejected(
        package,
        inject_collision_member,
        expected_error="campaign package directory member set differs",
    )
    expect_rejected(
        package,
        lambda candidate: rewrite_campaign_definition(
            candidate,
            lambda definition: definition["source_facts"].append(
                {
                    "fact_id": "collision-mask-bank",
                    "kind": 3,
                    "lifetime": "immutable-through-session",
                    "member": "collision-masks.bin",
                    "member_sha256": "0" * 64,
                    "selector_provenance": [],
                    "source_artifacts": [],
                    "version": 1,
                }
            ),
        ),
        expected_error="SourceFact count differs from the census profile",
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("package", type=Path)
    args = parser.parse_args()
    package = args.package.resolve(strict=True)
    verify(package)

    manifest = json.loads((package / "manifest.json").read_text(encoding="utf-8"))
    schema = manifest["schema"]

    if schema == CAMPAIGN_PACKAGE_SCHEMA:
        if manifest["level"]["external"] == 1:
            verify_level1_campaign_negatives(package)
        print(f"PASS: valid {schema} package and schema-aware tamper suite rejected")
        return 0

    if schema == SCHEMA_V5 and manifest["level"]["external"] == 2:
        players = (package / "players.bin").read_bytes()
        expected_banishment_states = [1, 1, 0, 0, 0, 0, 0, 0]
        assert [
            players[12 + slot * LEVEL2_PLAYER_RECORD_BYTES + 662]
            for slot in range(8)
        ] == expected_banishment_states
        assert [
            int.from_bytes(
                players[
                    12 + slot * LEVEL2_PLAYER_RECORD_BYTES + 6 :
                    12 + slot * LEVEL2_PLAYER_RECORD_BYTES + 8
                ],
                "little",
                signed=True,
            )
            for slot in range(8)
        ] == [31] * 8

    def corrupt_schema(candidate: Path) -> None:
        """Change the actual schema revision, so v1/v2 are not a no-op test."""
        path = candidate / "manifest.json"
        candidate_manifest = json.loads(path.read_text(encoding="utf-8"))
        revision = candidate_manifest["schema"].rsplit("/", 1)[-1]
        candidate_manifest["schema"] = f"tampered/{revision}"
        path.write_text(
            json.dumps(candidate_manifest, indent=2) + "\n", encoding="utf-8"
        )

    # Exercise schema validation for every supported revision.  The old test
    # replaced the literal v3 string, which silently did nothing for v1/v2.
    expect_rejected(package, corrupt_schema)

    def corrupt_entity_relation(candidate: Path) -> None:
        path = candidate / "entities.bin"
        content = bytearray(path.read_bytes())
        content[12 + 4 : 12 + 8] = (1001).to_bytes(4, "little")
        path.write_bytes(content)
        path.chmod(stat.S_IRUSR | stat.S_IWUSR)

    expect_rejected(package, corrupt_entity_relation)

    def corrupt_authoritative_terrain(candidate: Path) -> None:
        path = candidate / "terrain.bin"
        content = bytearray(path.read_bytes())
        content[0] ^= 0xFF
        path.write_bytes(content)
        path.chmod(stat.S_IRUSR | stat.S_IWUSR)

    expect_rejected(package, corrupt_authoritative_terrain)

    if schema == SCHEMA_V3 and manifest["level"]["external"] == 1:
        def corrupt_pre_feature_terrain_with_valid_hashes(candidate: Path) -> None:
            path = candidate / "terrain-pre-features.bin"
            content = bytearray(path.read_bytes())
            content[0x40000] ^= 0xFF
            path.write_bytes(content)
            path.chmod(stat.S_IRUSR | stat.S_IWUSR)
            refresh_file_identity(candidate, "terrain-pre-features.bin")

        expect_rejected(package, corrupt_pre_feature_terrain_with_valid_hashes)
        expect_rejected(
            package,
            lambda candidate: mutate_u16(candidate, "feature-start.bin", 10, 1),
        )
        expect_rejected(
            package,
            lambda candidate: mutate_u16(candidate, "feature-end.bin", 6, 15),
        )
        expect_rejected(
            package,
            lambda candidate: mutate_u16(candidate, "feature-start.bin", 8, 9776),
        )

        def corrupt_post_feature_terrain_with_valid_hashes(candidate: Path) -> None:
            path = candidate / "terrain-post-features.bin"
            content = bytearray(path.read_bytes())
            content[174 + 212 * 256] ^= 1
            path.write_bytes(content)
            path.chmod(stat.S_IRUSR | stat.S_IWUSR)
            refresh_file_identity(candidate, "terrain-post-features.bin")

        expect_rejected(
            package,
            corrupt_post_feature_terrain_with_valid_hashes,
            semantic=True,
            expected_error="feature terrain plane hashes: terrain-post-features.bin",
        )

        def corrupt_feature_entity_xtype_with_valid_hashes(candidate: Path) -> None:
            path = candidate / "entities-post-features.bin"
            content = bytearray(path.read_bytes())
            content[12 + 2 * 176 + 69] = 1
            path.write_bytes(content)
            path.chmod(stat.S_IRUSR | stat.S_IWUSR)
            refresh_file_identity(candidate, "entities-post-features.bin")

        expect_rejected(
            package,
            corrupt_feature_entity_xtype_with_valid_hashes,
            semantic=True,
            expected_error="feature entity slot 2 xtype",
        )
        expect_rejected(
            package,
            lambda candidate: mutate_u16(candidate, "feature-allocator.bin", 32, 998),
            semantic=True,
            expected_error="feature allocator free relations",
        )
        expect_rejected(
            package,
            lambda candidate: mutate_u16(
                candidate, "feature-construction-inputs.bin", 16, 18
            ),
        )

    if schema == SCHEMA_V4 and manifest["level"]["external"] == 2:
        def remove_type_str_164_carrier_descriptor(candidate: Path) -> None:
            mutate_manifest(
                candidate,
                lambda value: value["record_layouts"]["player"].pop(
                    "type_str_164_auxiliary"
                ),
            )

        expect_rejected(package, remove_type_str_164_carrier_descriptor)

        def duplicate_type_str_164_descriptor(candidate: Path) -> None:
            def mutate(value: dict) -> None:
                fields = value["record_layouts"]["player"]["type_str_164_auxiliary"][
                    "fields"
                ]
                fields.append(dict(fields[0]))

            mutate_manifest(candidate, mutate)

        expect_rejected(package, duplicate_type_str_164_descriptor)

        def move_type_str_164_descriptor_out_of_bounds(candidate: Path) -> None:
            mutate_manifest(
                candidate,
                lambda value: value["record_layouts"]["player"][
                    "type_str_164_auxiliary"
                ]["fields"][0].update({"source_offset": 1135}),
            )

        expect_rejected(package, move_type_str_164_descriptor_out_of_bounds)

        def tamper_type_str_164_source_reference(candidate: Path) -> None:
            mutate_manifest(
                candidate,
                lambda value: value["reference"]["type_str_164_auxiliary"].update(
                    {"consumer": "EventsFunctions.cpp:0"}
                ),
            )

        expect_rejected(package, tamper_type_str_164_source_reference)

        def use_legacy_player_component_version(candidate: Path) -> None:
            mutate_u16(candidate, "players.bin", 4, 2)

        expect_rejected(package, use_legacy_player_component_version)

        def use_v4_663_player_component(candidate: Path) -> None:
            path = candidate / "players.bin"
            content = path.read_bytes()
            count = int.from_bytes(content[8:12], "little")
            upgraded = bytearray(b"MCP1")
            upgraded.extend((4).to_bytes(2, "little"))
            upgraded.extend(LEVEL2_PLAYER_RECORD_BYTES.to_bytes(2, "little"))
            upgraded.extend(count.to_bytes(4, "little"))
            for slot in range(count):
                offset = 12 + slot * LEVEL2_V4_PLAYER_RECORD_BYTES
                upgraded.extend(content[offset : offset + LEVEL2_V4_PLAYER_RECORD_BYTES])
                upgraded.append(0)
            path.write_bytes(upgraded)
            path.chmod(stat.S_IRUSR | stat.S_IWUSR)
            refresh_file_identity(candidate, "players.bin")

        # A v4 manifest cannot be upgraded merely by appending a byte and
        # relabelling MCP1: the v5 manifest/layout route is required too.
        expect_rejected(package, use_v4_663_player_component)

        def truncate_type_str_164_carrier(candidate: Path) -> None:
            path = candidate / "players.bin"
            path.write_bytes(path.read_bytes()[:-1])
            path.chmod(stat.S_IRUSR | stat.S_IWUSR)
            refresh_file_identity(candidate, "players.bin")

        expect_rejected(package, truncate_type_str_164_carrier)

    if schema == SCHEMA_V5 and manifest["level"]["external"] == 2:
        def remove_banishment_state_descriptor(candidate: Path) -> None:
            mutate_manifest(
                candidate,
                lambda value: value["record_layouts"]["player"].pop(
                    "banishment_state_0x006"
                ),
            )

        expect_rejected(package, remove_banishment_state_descriptor)

        def remove_banishment_state_reference(candidate: Path) -> None:
            mutate_manifest(
                candidate,
                lambda value: value["reference"].pop("banishment_state_0x006"),
            )

        expect_rejected(package, remove_banishment_state_reference)

        def use_v3_662_player_component(candidate: Path) -> None:
            path = candidate / "players.bin"
            content = path.read_bytes()
            count = int.from_bytes(content[8:12], "little")
            downgraded = bytearray(b"MCP1")
            downgraded.extend((3).to_bytes(2, "little"))
            downgraded.extend(LEVEL2_V4_PLAYER_RECORD_BYTES.to_bytes(2, "little"))
            downgraded.extend(count.to_bytes(4, "little"))
            for slot in range(count):
                offset = 12 + slot * LEVEL2_PLAYER_RECORD_BYTES
                downgraded.extend(content[offset : offset + LEVEL2_V4_PLAYER_RECORD_BYTES])
            path.write_bytes(downgraded)
            path.chmod(stat.S_IRUSR | stat.S_IWUSR)
            refresh_file_identity(candidate, "players.bin")

        # Refresh the member and aggregate identities first: this proves the
        # v5/component-v3+662 pair itself is rejected rather than a stale hash.
        expect_rejected(package, use_v3_662_player_component)

    if schema in {SCHEMA_V2, SCHEMA_V3} and manifest["level"]["external"] == 1:
        def corrupt_ambient_distance_with_valid_hashes(candidate: Path) -> None:
            path = candidate / "players.bin"
            content = bytearray(path.read_bytes())
            content[12 + 565 : 12 + 569] = (2047).to_bytes(4, "little")
            path.write_bytes(content)
            path.chmod(stat.S_IRUSR | stat.S_IWUSR)
            refresh_file_identity(candidate, "players.bin")

        expect_rejected(package, corrupt_ambient_distance_with_valid_hashes)

        def corrupt_second_ambient_distance(candidate: Path) -> None:
            path = candidate / "players.bin"
            content = bytearray(path.read_bytes())
            content[12 + 569 : 12 + 573] = (1745).to_bytes(4, "little")
            path.write_bytes(content)
            path.chmod(stat.S_IRUSR | stat.S_IWUSR)
            refresh_file_identity(candidate, "players.bin")

        expect_rejected(package, corrupt_second_ambient_distance)
        expect_rejected(
            package,
            lambda candidate: mutate_i16(candidate, "players.bin", 12 + 20, 1),
        )
        expect_rejected(
            package,
            lambda candidate: mutate_i16(candidate, "players.bin", 12 + 40, 256),
        )
        expect_rejected(
            package,
            lambda candidate: mutate_i16(
                candidate, "entities.bin", 12 + 152 * 176 + 80, 19841
            ),
        )
        expect_rejected(
            package,
            lambda candidate: mutate_i16(candidate, "runtime.bin", 16, 8),
        )

        def make_related_entity_inactive(candidate: Path) -> None:
            path = candidate / "entities.bin"
            content = bytearray(path.read_bytes())
            content[12 + 152 * 176 + 2] = 0
            path.write_bytes(content)
            path.chmod(stat.S_IRUSR | stat.S_IWUSR)
            refresh_file_identity(candidate, "entities.bin")

        expect_rejected(package, make_related_entity_inactive)

        # Reset projection is introduced by v2 and remains part of v3.
        expect_rejected(
            package,
            lambda candidate: mutate_manifest(
                candidate,
                lambda manifest: manifest["reset_projection"]["field_mappings"].pop(
                    "target_speed"
                ),
            ),
        )
        expect_rejected(
            package,
            lambda candidate: mutate_manifest(
                candidate,
                lambda manifest: manifest["reset_projection"]["field_mappings"][
                    "target_speed"
                ].update({"kind": "zero_at_seam", "value": 0}),
            ),
        )
        expect_rejected(
            package,
            lambda candidate: mutate_manifest(
                candidate,
                lambda manifest: manifest["reset_projection"]["field_mappings"][
                    "target_speed"
                ].update({"offset": 573}),
            ),
        )
        expect_rejected(
            package,
            lambda candidate: mutate_manifest(
                candidate,
                lambda manifest: manifest["reset_projection"]["field_mappings"][
                    "target_speed"
                ].update({"encoding": "u32_le"}),
            ),
        )
        expect_rejected(
            package,
            lambda candidate: mutate_manifest(
                candidate,
                lambda manifest: manifest["reset_projection"]["state_hash"].update(
                    {"hex": "0000000000000000"}
                ),
            ),
        )
        expect_rejected(
            package,
            lambda candidate: mutate_manifest(
                candidate,
                lambda manifest: manifest["reset_projection"][
                    "conformance_hash"
                ].update({"hex": "0000000000000000"}),
            ),
        )

    if schema in {SCHEMA_V4, SCHEMA_V5}:
        assert manifest["level"]["map_type"]["name"] == "Day"

        def corrupt_inactive_secondary_plane_with_valid_hashes(candidate: Path) -> None:
            path = candidate / "terrain-pre-features.bin"
            content = bytearray(path.read_bytes())
            content[0x40000] ^= 1
            path.write_bytes(content)
            path.chmod(stat.S_IRUSR | stat.S_IWUSR)
            refresh_file_identity(candidate, "terrain-pre-features.bin")

        expect_rejected(
            package,
            corrupt_inactive_secondary_plane_with_valid_hashes,
            semantic=True,
            expected_error="non-Cave secondary plane is not zero-normalized",
        )

    expect_rejected(
        package,
        lambda candidate: mutate_u16(
            candidate,
            "players.bin",
            4,
            3 if schema in {SCHEMA_V2, SCHEMA_V3} else 2,
        ),
    )

    print(f"PASS: valid {schema} package and schema-aware tamper suite rejected")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
