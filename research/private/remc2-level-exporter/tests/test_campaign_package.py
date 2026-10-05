#!/usr/bin/env python3
"""Profile-versioned campaign exports and focused planted negatives."""

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
    _campaign_validate_presentation,
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


def package_identity(files: dict[str, dict[str, object]]) -> str:
    identity = bytearray()
    for name in sorted(files):
        entry = files[name]
        identity.extend(
            name.encode("ascii")
            + b"\0"
            + str(entry["sha256"]).encode("ascii")
            + b"\0"
            + str(entry["bytes"]).encode("ascii")
            + b"\n"
        )
    return hashlib.sha256(identity).hexdigest()


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
    scratch = Path(tempfile.mkdtemp(prefix="mc-cf02-campaign-package-", dir=generated_root))
    first, second = scratch / "level2-a", scratch / "level2-b"
    command = [
        str(exporter),
        "--owner-root", str(owner_root),
        "--level", "2",
        "--campaign-package",
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


def test_campaign_package_twins_are_byte_identical(campaign_twins: tuple[Path, Path]) -> None:
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


@pytest.mark.parametrize("fixture_name", ["campaign_twins", "level3_campaign_twins"])
def test_day_campaign_exports_source_selected_block_bank(request, fixture_name: str) -> None:
    package = request.getfixturevalue(fixture_name)[0]
    definition = json.loads((package / "campaign-definition.json").read_text(encoding="utf-8"))
    recipe = definition["campaign_spec"]["presentation_recipe"]
    assert recipe["schema"] == "mc2-presentation-recipe/v2"
    assert recipe["block_bank"] == "BLOCK32"
    assert recipe["resolved_files"]["block_bank"] == "owner:CD_Files/DATA/BLOCK32.DAT"
    assert recipe["block_bank"] != "BL32N0-0", "Night indices cannot use the Day palette"
    verify(package)
    manifest = json.loads((package / "manifest.json").read_text(encoding="utf-8"))
    recipe["block_bank"] = "BL32N0-0"
    recipe["resolved_files"]["block_bank"] = "owner:CD_Files/DATA/BL32N0-0.DAT"
    with pytest.raises(AssertionError, match="block_bank"):
        _campaign_validate_presentation(recipe, manifest["files"])


def test_level_two_v2_binds_the_standing_objective_to_owner_building_parameters(
    campaign_twins: tuple[Path, Path],
) -> None:
    package = campaign_twins[0]
    manifest = json.loads((package / "manifest.json").read_text(encoding="utf-8"))
    definition = json.loads(
        (package / "campaign-definition.json").read_text(encoding="utf-8")
    )
    member = (package / "building-parameters.bin").read_bytes()
    assert manifest["schema"] == "mc2-owner-campaign-package/v2"
    assert definition["schema"] == "mc2-campaign-definition/v2"
    assert len(member) == 304
    assert hashlib.sha256(member).hexdigest() == (
        "1c4a7fdef5d4b811f871441f0380faaacd6e5e689e73fc4628bf8c5b51ea6f00"
    )
    assert member[21 * 4 : 22 * 4] == bytes.fromhex("06000836")
    assert member[54 * 4 : 55 * 4] == bytes.fromhex("08000800")
    assert definition["source_facts"][0]["selector_provenance"] == [
        {
            "authored_record": 153,
            "source_class": 10,
            "source_field": "par1_14",
            "source_model": 21,
            "source_subtype": 45,
            "selector_value": 21,
        }
    ]

    authored = bytearray((package / "authored-level.bin").read_bytes())
    struct.pack_into("<h", authored, 1091 + 153 * 20 + 14, 22)

    def refresh_authored_row(definition: dict) -> None:
        row = next(
            row
            for row in definition["campaign_spec"]["authored_behavior_rows"]
            if row["authored_record"] == 153
        )
        row["model"] = 22

    expect_rejected(
        package,
        lambda candidate: rewrite_member(
            candidate,
            "authored-level.bin",
            bytes(authored),
            refresh_authored_row,
        ),
        "standing-building objective provenance differs",
    )

    authored_stage = bytearray((package / "authored-level.bin").read_bytes())
    struct.pack_into("<h", authored_stage, 25972 + 4 * 7 + 1, 163)

    def refresh_objective_selector(definition: dict) -> None:
        operation = next(
            operation
            for operation in definition["campaign_spec"]["objectives"]
            if operation["operands"]["objective_slot"] == 4
        )
        operation["operands"]["objective_selector"] = 163

    expect_rejected(
        package,
        lambda candidate: rewrite_member(
            candidate,
            "authored-level.bin",
            bytes(authored_stage),
            refresh_objective_selector,
        ),
        "standing-building objective operation differs",
    )


def expect_rejected(package: Path, mutate, expected: str, **verify_options) -> None:
    with tempfile.TemporaryDirectory(prefix="mc-cf02-negative-") as temporary:
        candidate = Path(temporary) / "package"
        shutil.copytree(package, candidate)
        mutate(candidate)
        with pytest.raises(AssertionError, match=expected):
            verify(candidate, **verify_options)


def test_campaign_package_full_planted_negative_set(campaign_twins: tuple[Path, Path]) -> None:
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
        _campaign_decode_behavior_rows(authored, entities, 114)
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


def test_campaign_package_additional_closed_union_and_carrier_negatives(
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
            bytes(authored), (package / "entities.bin").read_bytes(), 114
        )


def test_campaign_package_flag_is_census_profiled_only(tmp_path: Path) -> None:
    exporter = Path(os.environ.get("MC2_LEVEL_EXPORTER_BIN", "/tmp/magiccarpet-remc2-level-exporter-build/remc2-level-exporter"))
    owner_root = Path(os.environ.get("MC2_OWNER_ROOT", str(ROOT / "research/private/remc2-native-runtime")))
    for level in ("4",):
        output = tmp_path / f"level{level}"
        result = subprocess.run(
            [
                str(exporter), "--owner-root", str(owner_root), "--output", str(output),
                "--level", level, "--campaign-package",
            ],
            capture_output=True,
            text=True,
            timeout=30,
        )
        assert result.returncode != 0
        assert "admits only census-profiled external levels" in result.stderr
        assert not output.exists()


@pytest.fixture(scope="module")
def level1_campaign_exports() -> tuple[Path, Path, Path]:
    exporter = Path(os.environ.get("MC2_LEVEL_EXPORTER_BIN", "/tmp/magiccarpet-remc2-level-exporter-build/remc2-level-exporter"))
    if not exporter.is_file():
        raise AssertionError("CF-15 tests require the clean built remc2-level-exporter")
    owner_root = Path(os.environ.get("MC2_OWNER_ROOT", str(ROOT / "research/private/remc2-native-runtime")))
    generated_root = ROOT / "research/private/generated"
    generated_root.mkdir(parents=True, exist_ok=True)
    scratch = Path(tempfile.mkdtemp(prefix="mc-cf15-level1-package-v3-", dir=generated_root))
    ordinary = scratch / "ordinary"
    first, second = scratch / "campaign-a", scratch / "campaign-b"
    common = [
        str(exporter),
        "--owner-root", str(owner_root),
        "--level", "1",
    ]
    try:
        subprocess.run(
            common + ["--output", str(ordinary)],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
            timeout=120,
        )
        for output in (first, second):
            subprocess.run(
                common + ["--campaign-package", "--output", str(output)],
                cwd=ROOT,
                check=True,
                capture_output=True,
                text=True,
                timeout=120,
            )
        yield ordinary, first, second
    finally:
        shutil.rmtree(scratch)


def test_cf15_level1_campaign_preserves_owner_export_except_versioned_runtime_counters(
    level1_campaign_exports: tuple[Path, Path, Path],
) -> None:
    ordinary, first, second = level1_campaign_exports
    verify(first)
    verify(second)
    assert tree_bytes(first) == tree_bytes(second)

    outer = json.loads((first / "manifest.json").read_text(encoding="utf-8"))
    owner_manifest_bytes = (first / "owner-manifest.json").read_bytes()
    owner = json.loads(owner_manifest_bytes)
    definition = json.loads((first / "campaign-definition.json").read_text(encoding="utf-8"))

    # GOLDEN-CHANGE: only runtime v2 and its bound layout/hash metadata differ.
    # All other captured bytes and manifest fields remain exactly identical.
    ordinary_manifest = json.loads((ordinary / "manifest.json").read_bytes())
    expected_owner = json.loads(json.dumps(ordinary_manifest))
    runtime = (first / "runtime.bin").read_bytes()
    ordinary_runtime = (ordinary / "runtime.bin").read_bytes()
    assert len(runtime) == 2783 and len(ordinary_runtime) == 2754
    assert runtime[:4] == ordinary_runtime[:4] == b"MCR1"
    assert runtime[4:6] == b"\x02\x00" and ordinary_runtime[4:6] == b"\x01\x00"
    assert runtime[6:2754] == ordinary_runtime[6:]
    expected_owner["files"]["runtime.bin"] = {
        "bytes": 2783, "sha256": hashlib.sha256(runtime).hexdigest(),
    }
    expected_owner["record_layouts"]["runtime"]["bytes"] = 2783
    expected_owner["record_layouts"]["runtime"]["constructor_counters"] = {
        "offset": 2754, "count": 29, "record_bytes": 1,
    }
    expected_owner["package_sha256"] = package_identity(expected_owner["files"])
    assert owner == expected_owner
    assert owner["schema"] == "mc2-owner-level-export/v3"
    assert owner["level"] == {
        "external": 1,
        "internal": 0,
        "map_type": {"id": 1, "name": "Night"},
    }
    assert owner["counts"]["active_entities"] == 177
    assert owner["record_sizes"]["player"] == 573
    assert owner["record_layouts"]["player"]["record_bytes"] == 573
    assert "collision-masks.bin" not in owner["files"]
    assert "collision_masks" not in owner

    for name in owner["files"]:
        if name != "runtime.bin":
            assert (first / name).read_bytes() == (ordinary / name).read_bytes()
    assert owner["package_sha256"] == package_identity(owner["files"])

    assert set(outer) == {
        "schema",
        "byte_order",
        "level",
        "owner_export_package_sha256",
        "files",
        "package_sha256",
    }
    assert outer["schema"] == "mc2-owner-campaign-package/v3"
    assert outer["level"] == owner["level"]
    assert outer["owner_export_package_sha256"] == owner["package_sha256"]
    assert outer["package_sha256"] == package_identity(outer["files"])
    assert set(outer["files"]) == set(owner["files"]) | {
        "campaign-definition.json",
        "fireball-projectile-base.indexed-sprite.json",
        "fireball-projectile-fade.indexed-sprite.json",
        "owner-manifest.json",
    }
    assert outer["files"]["owner-manifest.json"] == {
        "bytes": len(owner_manifest_bytes),
        "sha256": hashlib.sha256(owner_manifest_bytes).hexdigest(),
    }
    assert "collision-masks.bin" not in outer["files"]
    assert not (first / "collision-masks.bin").exists()
    assert definition["schema"] == "mc2-campaign-definition/v3"

    assets = definition["presentation_assets"]
    assert [asset["artifact_id"] for asset in assets] == [
        "presentation.fireball.projectile.base",
        "presentation.fireball.projectile.fade",
    ]
    assert [asset["member"]["sha256"] for asset in assets] == [
        "331df08c5fa8483d5dd75dc10abd9dd76cc26918d6447403b3f3199bc31f521d",
        "61059124cba22dd3086baab36bb7d12a9ab28ebf37d394827a49bea4f6c96784",
    ]
    for asset in assets:
        payload = json.loads((first / asset["member"]["member"]).read_text(encoding="utf-8"))
        assert payload["schema"] == "carpet-flight-indexed-sprite/v1"
        assert (payload["width"], payload["height"], payload["frame_count"]) == (34, 35, 1)
        assert len(payload["indices"]) == 1190
        assert len(payload["palette_rgba8"]) == 1024

    mutated = json.loads(
        (first / "fireball-projectile-base.indexed-sprite.json").read_text(encoding="utf-8")
    )
    mutated["indices"][0] = (mutated["indices"][0] + 1) % 256
    expect_rejected(
        first,
        lambda candidate: rewrite_member(
            candidate,
            "fireball-projectile-base.indexed-sprite.json",
            canonical_bytes(mutated),
        ),
        "presentation member binding differs",
    )

    assert definition["identity"] == {
        "external_level": 1,
        "internal_level": 0,
        "map_type_id": 1,
    }
    players = definition["campaign_spec"]["reset_sources"]["players"]
    assert players["schema"] == "mc2-player-arena/v2"
    assert players["count"] == 8
    assert players["record_bytes"] == 573
    player_bytes = (first / "players.bin").read_bytes()
    assert player_bytes[:4] == b"MCP1"
    assert struct.unpack_from("<HHI", player_bytes, 4) == (2, 573, 8)
    runtime = (first / "runtime.bin").read_bytes()
    assert struct.unpack_from("<hh", runtime, 16) == (0, 1)

    objectives = definition["campaign_spec"]["objectives"]
    assert len(objectives) == 5
    assert {row["operands"]["stage_index"] for row in objectives} == {0, 5, 7}
    assert {
        row["fact_id"] for row in definition["source_facts"]
    } == {"feature-construction-inputs", "level-records"}
    assert sum(
        row["initial_source_slot"] is not None
        for row in definition["campaign_spec"]["authored_behavior_rows"]
    ) == 177


@pytest.fixture(scope="module")
def level3_campaign_twins() -> tuple[Path, Path]:
    exporter = Path(os.environ.get("MC2_LEVEL_EXPORTER_BIN", "/tmp/magiccarpet-remc2-level-exporter-build/remc2-level-exporter"))
    if not exporter.is_file():
        raise AssertionError("CF-14 tests require the clean built remc2-level-exporter")
    owner_root = Path(os.environ.get("MC2_OWNER_ROOT", str(ROOT / "research/private/remc2-native-runtime")))
    generated_root = ROOT / "research/private/generated"
    generated_root.mkdir(parents=True, exist_ok=True)
    scratch = Path(tempfile.mkdtemp(prefix="mc-cf14-package-v1-", dir=generated_root))
    first, second = scratch / "level3-a", scratch / "level3-b"
    command = [
        str(exporter),
        "--owner-root", str(owner_root),
        "--level", "3",
        "--campaign-package",
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


@pytest.mark.parametrize("mutation", ["truncated", "extra", "old_version", "unknown_version", "wrong_schema"])
def test_runtime_constructor_counter_contract_rejects_malformed_records(
    level3_campaign_twins: tuple[Path, Path], tmp_path: Path, mutation: str,
) -> None:
    package = tmp_path / "package"
    shutil.copytree(level3_campaign_twins[0], package)
    runtime = bytearray((package / "runtime.bin").read_bytes())
    assert len(runtime) == 2783 and runtime[4] == 2
    if mutation == "truncated":
        runtime.pop()
    elif mutation == "extra":
        runtime.append(0)
    elif mutation == "old_version":
        runtime[4] = 1
    elif mutation == "unknown_version":
        runtime[4] = 3
    rewrite_member(package, "runtime.bin", bytes(runtime))
    if mutation == "wrong_schema":
        rewrite_definition(package, lambda definition: definition["campaign_spec"]["reset_sources"]["runtime"].update(schema="mc2-runtime-state/v1"))
    with pytest.raises(AssertionError):
        verify(package)


def test_cf14_level3_campaign_package_uses_shared_schema(level3_campaign_twins: tuple[Path, Path]) -> None:
    first, second = level3_campaign_twins
    verify(first)
    verify(second)
    assert tree_bytes(first) == tree_bytes(second)
    manifest = json.loads((first / "manifest.json").read_text(encoding="utf-8"))
    assert manifest["schema"] == "mc2-owner-campaign-package/v1"
    assert manifest["level"] == {
        "external": 3,
        "internal": 2,
        "map_type": {"id": 0, "name": "Day"},
    }
    definition = json.loads((first / "campaign-definition.json").read_text(encoding="utf-8"))
    assert definition["schema"] == "mc2-campaign-definition/v1"
    assert definition["identity"] == {"external_level": 3, "internal_level": 2, "map_type_id": 0}
    runtime = (first / "runtime.bin").read_bytes()
    assert (struct.unpack_from("<H", runtime, 4)[0], len(runtime)) == (2, 2783)
    assert definition["campaign_spec"]["reset_sources"]["runtime"]["schema"] == "mc2-runtime-state/v2"
    assert runtime[2754 + 9] != 0, "source reset constructed Skeletons before capture"
    assert len(definition["campaign_spec"]["objectives"]) == 5
    live_pairs = sorted(
        (row["selector"]["class"], row["selector"]["action"])
        for row in definition["required_behaviors"]
    )
    # mc-iol7n census receipt: the fifteen live Level 3 reset selectors,
    # including the three census-delta capabilities (2,21), (11,16), (14,6)
    # that stay uncompiled until their shared-behavior leaves land.
    assert live_pairs == [
        (2, 0), (2, 3), (2, 21), (3, 0), (5, 31), (5, 72), (5, 232), (10, 52),
        (11, 0), (11, 4), (11, 16), (11, 32), (14, 6), (15, 19), (15, 34),
    ]
    # A caller pinning the wrong selection still fails closed.
    with pytest.raises(AssertionError, match="selection mismatch"):
        verify(first, expected_campaign_selection=(2, 1, 0))
