from __future__ import annotations

import importlib.util
import hashlib
import struct
import sys
from pathlib import Path

import pytest


TEST_ROOT = Path(__file__).parent
sys.path.insert(0, str(TEST_ROOT))
import verify_package as verifier  # noqa: E402


CENSUS_PATH = TEST_ROOT.parent / "scripts" / "campaign_census.py"
CENSUS_SPEC = importlib.util.spec_from_file_location("campaign_census_no_collision", CENSUS_PATH)
assert CENSUS_SPEC and CENSUS_SPEC.loader
census = importlib.util.module_from_spec(CENSUS_SPEC)
CENSUS_SPEC.loader.exec_module(census)


def authored_level(*, model_45_rows: list[int]) -> bytes:
    data = bytearray(0x6604)
    for row_index, model in enumerate(model_45_rows, start=1):
        struct.pack_into(
            "<HhHHhHhHHH",
            data,
            1091 + row_index * 20,
            10,
            45,
            0,
            0,
            -1,
            0,
            0,
            model,
            0,
            0,
        )
    return bytes(data)


def collision_member(selectors: list[int]) -> bytes:
    content = bytearray(struct.pack("<4sHHHHII", b"MCM1", 1, 20, 1, 0, len(selectors), 12))
    for selector in selectors:
        payload = b"\0\0"
        content.extend(struct.pack("<HHHHI", selector, 1, 1, 0, len(payload)))
        content.extend(payload)
    return bytes(content)


def selector_rows(selectors: list[int]) -> list[dict[str, object]]:
    return [
        {
            "selector": selector,
            "width": 1,
            "height": 1,
            "pixel_bytes": 2,
            "sha256": hashlib.sha256(b"\0\0").hexdigest(),
        }
        for selector in selectors
    ]


def collision_metadata(selectors: list[int], row_count: int) -> dict[str, object]:
    return {
        "file": "collision-masks.bin",
        "magic": "MCM1",
        "version": 1,
        "header_bytes": 20,
        "record_header_bytes": 12,
        "bank": "MSPRD0-0",
        "vga_resolution_selector": 1,
        "selector_source": "authored class-10/model-45 par1 selectors",
        "pixel_encoding": "signed i16 little-endian",
        "pixel_order": "row-major (y then x)",
        "transparent_value": -1,
        "source_files": {
            "tab": {
                "logical_path": "CD_Files/DATA/MSPRD0-0.TAB",
                "bytes": 1572,
                "sha256": verifier.MSPRD0_0_TAB_SHA256,
            },
            "dat": {
                "logical_path": "CD_Files/DATA/MSPRD0-0.DAT",
                "bytes": 49202,
                "sha256": verifier.MSPRD0_0_DAT_SHA256,
            },
        },
        "selectors": selector_rows(selectors),
        "authored_model_45_row_count": row_count,
    }


def census_rows(model_45_rows: list[int]) -> list[dict[str, object]]:
    return [
        {
            "class": 10,
            "subtype": 45,
            "model": model,
            "source_fields": {"DisId": -1},
        }
        for model in model_45_rows
    ]


def test_qualified_empty_member_is_admitted_and_counted() -> None:
    authored = authored_level(model_45_rows=[])
    content = collision_member([])
    metadata = collision_metadata([], 0)

    assert verifier.validate_collision_mask_contract(
        authored,
        content,
        metadata,
        external_level=8,
    ) == []


@pytest.mark.parametrize(
    ("authored_models", "member_selectors", "row_count", "message"),
    [
        ([17], [], 1, "selectors differ"),
        ([], [17], 0, "selectors differ"),
    ],
)
def test_zero_count_cross_check_rejects_planted_mismatch(
    authored_models: list[int],
    member_selectors: list[int],
    row_count: int,
    message: str,
) -> None:
    with pytest.raises(AssertionError, match=message):
        verifier.validate_collision_mask_contract(
            authored_level(model_45_rows=authored_models),
            collision_member(member_selectors),
            collision_metadata(member_selectors, row_count),
            external_level=8,
        )


def test_validator_rejects_absent_typed_member(tmp_path: Path) -> None:
    package = tmp_path / "package"
    package.mkdir()
    (package / "authored-level.bin").write_bytes(authored_level(model_45_rows=[]))
    manifest = {
        "schema": "mc2-owner-level-export/v5",
        "level": {"external": 8},
        "collision_masks": collision_metadata([], 0),
    }

    with pytest.raises(AssertionError, match="member is absent"):
        verifier.verify_collision_masks(package, manifest)


def test_census_repeats_the_empty_member_cross_check(tmp_path: Path) -> None:
    package = tmp_path / "package"
    package.mkdir()
    (package / "collision-masks.bin").write_bytes(collision_member([]))
    manifest = {"collision_masks": collision_metadata([], 1)}

    with pytest.raises(census.CensusError, match="selectors differ"):
        census.parse_collision(manifest, package, census_rows([17]), 8)
