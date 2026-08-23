#!/usr/bin/env python3
"""CF-02 production exports and planted campaign-package-v1 negatives."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile

import pytest

from verify_package import (
    _campaign_decode_behavior_rows,
    _campaign_decode_collision,
    _campaign_required_behaviors,
    verify,
)


ROOT = Path(__file__).resolve().parents[4]


def canonical_bytes(value: object) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")


def tree_bytes(root: Path) -> dict[str, bytes]:
    return {
        path.relative_to(root).as_posix(): path.read_bytes()
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


def refresh_package_hash(package: Path, manifest: dict) -> None:
    identity = bytearray()
    for name in sorted(manifest["files"]):
        entry = manifest["files"][name]
        identity.extend(
            name.encode("ascii")
            + b"\0"
            + entry["sha256"].encode("ascii")
            + b"\0"
            + str(entry["bytes"]).encode("ascii")
            + b"\n"
        )
    manifest["package_sha256"] = hashlib.sha256(identity).hexdigest()
    (package / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")


def rewrite_definition(package: Path, mutate) -> None:
    definition_path = package / "campaign-definition.json"
    definition = json.loads(definition_path.read_text(encoding="utf-8"))
    mutate(definition)
    content = canonical_bytes(definition)
    definition_path.write_bytes(content)
    manifest = json.loads((package / "manifest.json").read_text(encoding="utf-8"))
    manifest["files"]["campaign-definition.json"] = {
        "bytes": len(content),
        "sha256": hashlib.sha256(content).hexdigest(),
    }
    refresh_package_hash(package, manifest)


def rewrite_member(package: Path, name: str, content: bytes, mutate_definition=lambda _value: None) -> None:
    (package / name).write_bytes(content)
    manifest = json.loads((package / "manifest.json").read_text(encoding="utf-8"))
    digest = hashlib.sha256(content).hexdigest()
    manifest["files"][name] = {"bytes": len(content), "sha256": digest}
    definition_path = package / "campaign-definition.json"
    definition = json.loads(definition_path.read_text(encoding="utf-8"))

    def replace_hash(value: object) -> None:
        if isinstance(value, dict):
            if value.get("member") == name and "sha256" in value:
                value["sha256"] = digest
            if value.get("member") == name and "member_sha256" in value:
                value["member_sha256"] = digest
            for child in value.values():
                replace_hash(child)
        elif isinstance(value, list):
            for child in value:
                replace_hash(child)

    replace_hash(definition)
    mutate_definition(definition)
    definition_content = canonical_bytes(definition)
    definition_path.write_bytes(definition_content)
    manifest["files"]["campaign-definition.json"] = {
        "bytes": len(definition_content),
        "sha256": hashlib.sha256(definition_content).hexdigest(),
    }
    refresh_package_hash(package, manifest)


@pytest.fixture(scope="module")
def campaign_twins() -> tuple[Path, Path]:
    exporter = Path(os.environ.get("MC2_LEVEL_EXPORTER_BIN", "/tmp/magiccarpet-remc2-level-exporter-build/remc2-level-exporter"))
    if not exporter.is_file():
        raise AssertionError("CF-02 tests require the clean built remc2-level-exporter")
    owner_root = Path(os.environ.get("MC2_OWNER_ROOT", str(ROOT / "research/private/remc2-native-runtime")))
    generated_root = ROOT / "research/private/generated"
    generated_root.mkdir(parents=True, exist_ok=True)
    scratch = Path(tempfile.mkdtemp(prefix="mc-cf02-package-v1-", dir=generated_root))
    first, second = scratch / "level2-a", scratch / "level2-b"
    command = [
        str(exporter),
        "--owner-root", str(owner_root),
        "--level", "2",
        "--campaign-package-v1",
    ]
    try:
        for output in (first, second):
            subprocess.run(
                command + ["--output", str(output)],
                cwd=ROOT,
                check=True,
                capture_output=True,
                text=True,
                timeout=120,
            )
        yield first, second
    finally:
        shutil.rmtree(scratch)


def test_campaign_package_v1_twins_are_byte_identical(campaign_twins: tuple[Path, Path]) -> None:
    first, second = campaign_twins
    verify(first)
    verify(second)
    assert tree_bytes(first) == tree_bytes(second)
    first_manifest = json.loads((first / "manifest.json").read_text(encoding="utf-8"))
    second_manifest = json.loads((second / "manifest.json").read_text(encoding="utf-8"))
    assert first_manifest["files"] == second_manifest["files"]
    assert first_manifest["package_sha256"] == second_manifest["package_sha256"]
    assert (first / "campaign-definition.json").read_bytes() == (
        second / "campaign-definition.json"
    ).read_bytes()


def expect_rejected(package: Path, mutate, expected: str, **verify_options) -> None:
    with tempfile.TemporaryDirectory(prefix="mc-cf02-negative-") as temporary:
        candidate = Path(temporary) / "package"
        shutil.copytree(package, candidate)
        mutate(candidate)
        with pytest.raises(AssertionError, match=expected):
            verify(candidate, **verify_options)


def test_campaign_package_v1_full_planted_negative_set(campaign_twins: tuple[Path, Path]) -> None:
    package = campaign_twins[0]

    expect_rejected(
        package,
        lambda candidate: rewrite_definition(candidate, lambda definition: definition.update({"unknown": 1})),
        "field set differs",
    )
    expect_rejected(
        package,
        lambda candidate: rewrite_definition(
            candidate,
            lambda definition: definition["source_facts"][0]["selector_provenance"][0].update({"selector_value": 999}),
        ),
        "selector provenance differs",
    )
    expect_rejected(
        package,
        lambda candidate: rewrite_definition(
            candidate,
            lambda definition: definition["source_facts"][0]["source_artifacts"][0].update({"logical_path": "../MSPRD0-0.DAT"}),
        ),
        "traverses|noncanonical",
    )

    authored = (package / "authored-level.bin").read_bytes()
    entities = (package / "entities.bin").read_bytes()
    _requirements, capabilities = _campaign_required_behaviors(
        _campaign_decode_behavior_rows(authored, entities)
    )
    missing_capability = set(capabilities)
    missing_capability.remove(sorted(missing_capability)[0])
    expect_rejected(
        package,
        lambda _candidate: None,
        "compiled capability is missing",
        campaign_capabilities=missing_capability,
    )
    expect_rejected(
        package,
        lambda candidate: rewrite_definition(
            candidate,
            lambda definition: definition["required_behaviors"].__delitem__(0),
        ),
        "required behavior roots",
    )


def test_campaign_package_v1_additional_closed_union_and_carrier_negatives(
    campaign_twins: tuple[Path, Path],
) -> None:
    package = campaign_twins[0]
    for mutate, expected in (
        (
            lambda definition: definition["source_facts"][0].update({"kind": 99}),
            "kind/version/lifetime/member differs",
        ),
        (
            lambda definition: definition["source_facts"][0].update({"version": 2}),
            "kind/version/lifetime/member differs",
        ),
        (
            lambda definition: definition["campaign_spec"]["objectives"][0].update({"operation_kind": 99}),
            "objective operations differ",
        ),
        (
            lambda definition: definition["campaign_spec"]["objectives"][0].update({"operation_version": 2}),
            "objective operations differ",
        ),
        (
            lambda definition: definition["campaign_spec"]["initial_activation"].append(
                {"operation_kind": 99, "operation_version": 1, "operands": {}}
            ),
            "initial-activation operation",
        ),
        (
            lambda definition: definition["source_facts"].reverse(),
            "exact sorted",
        ),
        (
            lambda definition: definition["source_facts"].__setitem__(
                2, definition["source_facts"][1]
            ),
            "exact sorted",
        ),
        (
            lambda definition: definition["identity"].update({"external_level": 1}),
            "selection mismatch",
        ),
        (
            lambda definition: definition["identity"].update({"internal_level": True}),
            "out of bounds",
        ),
        (
            lambda definition: definition["source_facts"][0].update({"member_sha256": "0" * 64}),
            "member hash differs",
        ),
        (
            lambda definition: definition["campaign_spec"]["presentation_recipe"].update(
                {"terrain_graphics_flags": 1}
            ),
            "terrain flags differ",
        ),
    ):
        expect_rejected(
            package,
            lambda candidate, mutate=mutate: rewrite_definition(candidate, mutate),
            expected,
        )

    expect_rejected(
        package,
        lambda candidate: (candidate / "collision-masks.bin").unlink(),
        "directory member set differs",
    )

    runtime_with_nonzero_relation_padding = bytearray(
        (package / "runtime.bin").read_bytes()
    )
    runtime_with_nonzero_relation_padding[192 + 7] = 1
    expect_rejected(
        package,
        lambda candidate: rewrite_member(
            candidate,
            "runtime.bin",
            bytes(runtime_with_nonzero_relation_padding),
        ),
        "relation padding is nonzero",
    )

    expect_rejected(
        package,
        lambda candidate: rewrite_definition(
            candidate,
            lambda definition: definition["source_facts"][0]["source_artifacts"][0].update(
                {"logical_path": "/CD_Files/DATA/MSPRD0-0.DAT"}
            ),
        ),
        "absolute|noncanonical",
    )

    empty_collision = (
        b"MCM1"
        + (1).to_bytes(2, "little")
        + (20).to_bytes(2, "little")
        + (1).to_bytes(2, "little")
        + (0).to_bytes(2, "little")
        + (0).to_bytes(4, "little")
        + (12).to_bytes(4, "little")
    )
    assert _campaign_decode_collision(empty_collision, []) == []
    expect_rejected(
        package,
        lambda candidate: rewrite_member(candidate, "collision-masks.bin", empty_collision),
        "selectors differ|empty/nonempty",
    )

    # The decoder also rejects the converse independently of package hashes:
    # a nonempty carrier cannot claim zero authored selector rows.
    with pytest.raises(AssertionError, match="selectors differ|empty/nonempty"):
        _campaign_decode_collision((package / "collision-masks.bin").read_bytes(), [])

    # The source stores subtype as signed i16, but schema v1 admits only u8.
    authored = bytearray((package / "authored-level.bin").read_bytes())
    first_authored = next(
        index
        for index in range(1200)
        if any(authored[1091 + index * 20 : 1091 + (index + 1) * 20])
    )
    struct.pack_into("<h", authored, 1091 + first_authored * 20 + 2, -2)
    with pytest.raises(AssertionError, match="exceeds schema-v1 width"):
        _campaign_decode_behavior_rows(
            bytes(authored), (package / "entities.bin").read_bytes()
        )


def test_campaign_package_v1_flag_is_level2_only(tmp_path: Path) -> None:
    exporter = Path(os.environ.get("MC2_LEVEL_EXPORTER_BIN", "/tmp/magiccarpet-remc2-level-exporter-build/remc2-level-exporter"))
    owner_root = Path(os.environ.get("MC2_OWNER_ROOT", str(ROOT / "research/private/remc2-native-runtime")))
    result = subprocess.run(
        [
            str(exporter), "--owner-root", str(owner_root), "--output", str(tmp_path / "level1"),
            "--level", "1", "--campaign-package-v1",
        ],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert result.returncode != 0
    assert "currently admits only external Level 2" in result.stderr
    assert not (tmp_path / "level1").exists()
