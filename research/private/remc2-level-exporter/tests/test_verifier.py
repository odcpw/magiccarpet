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

from verify_package import SCHEMA_V2, SCHEMA_V3, verify


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


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("package", type=Path)
    args = parser.parse_args()
    package = args.package.resolve(strict=True)
    verify(package)

    manifest = json.loads((package / "manifest.json").read_text(encoding="utf-8"))
    schema = manifest["schema"]

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

    if schema == SCHEMA_V3:
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

    if schema in {SCHEMA_V2, SCHEMA_V3}:
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
