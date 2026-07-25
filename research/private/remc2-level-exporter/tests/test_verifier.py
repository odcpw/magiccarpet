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

from verify_package import verify


def expect_rejected(package: Path, mutate) -> None:
    with tempfile.TemporaryDirectory(prefix="mc2-exporter-negative-") as temporary:
        candidate = Path(temporary) / "package"
        shutil.copytree(package, candidate)
        candidate.chmod(0o700)
        for path in candidate.iterdir():
            path.chmod(0o600)
        mutate(candidate)
        try:
            verify(candidate)
        except (AssertionError, KeyError, OSError, json.JSONDecodeError):
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

    expect_rejected(
        package,
        lambda candidate: (candidate / "manifest.json").write_text(
            (candidate / "manifest.json")
            .read_text(encoding="utf-8")
            .replace("mc2-owner-level-export/v2", "tampered/v2"),
            encoding="utf-8",
        ),
    )

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
    expect_rejected(
        package,
        lambda candidate: mutate_u16(candidate, "players.bin", 12 + 4, 1000),
    )

    def make_related_entity_inactive(candidate: Path) -> None:
        path = candidate / "entities.bin"
        content = bytearray(path.read_bytes())
        content[12 + 152 * 176 + 2] = 0
        path.write_bytes(content)
        path.chmod(stat.S_IRUSR | stat.S_IWUSR)
        refresh_file_identity(candidate, "entities.bin")

    expect_rejected(package, make_related_entity_inactive)
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
        lambda candidate: mutate_manifest(
            candidate,
            lambda manifest: manifest.update(
                {"schema": "mc2-owner-level-export/v1"}
            ),
        ),
    )

    expect_rejected(
        package,
        lambda candidate: mutate_u16(candidate, "players.bin", 4, 1),
    )

    print("PASS: valid package accepted and 19 tampered packages rejected")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
