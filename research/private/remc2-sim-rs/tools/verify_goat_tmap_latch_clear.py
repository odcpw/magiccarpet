#!/usr/bin/env python3
"""Verify the pinned LE mapping and bytes for the MC2 TMAP latch clear."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import sys


def u32le(data: bytes, offset: int) -> int:
    return int.from_bytes(data[offset : offset + 4], "little")


def fail(message: str) -> None:
    raise SystemExit(f"MC_GOAT_TMAP_LATCH_CLEAR FAIL {message}")


def require(actual: object, expected: object, name: str) -> None:
    if actual != expected:
        fail(f"{name} expected={expected!r} actual={actual!r}")


def main() -> None:
    repo = Path(__file__).resolve().parents[4]
    evidence_path = (
        repo
        / "research/private/remc2-sim-rs/evidence/goat-tmap-latch-clear-0x715f0.json"
    )
    evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
    source = evidence["source_executable"]
    mapping = evidence["le_mapping"]
    window_claim = evidence["byte_window"]
    executable_path = repo / source["path"]
    executable = executable_path.read_bytes()

    require(len(executable), source["size"], "executable size")
    require(hashlib.sha256(executable).hexdigest(), source["sha256"], "executable sha256")

    header = u32le(executable, 0x3C)
    require(header, mapping["header_offset"], "LE header offset")
    require(executable[header : header + 2], b"LE", "LE signature")
    page_size = u32le(executable, header + 0x28)
    require(page_size, mapping["page_size"], "LE page size")

    object_table_relative = u32le(executable, header + 0x40)
    page_map_relative = u32le(executable, header + 0x48)
    data_pages_absolute = u32le(executable, header + 0x80)
    require(
        object_table_relative,
        mapping["object_table_relative_offset"],
        "object table relative offset",
    )
    require(
        page_map_relative,
        mapping["page_map_relative_offset"],
        "page map relative offset",
    )
    require(
        data_pages_absolute,
        mapping["data_pages_absolute_offset"],
        "data pages absolute offset",
    )

    object_table = header + object_table_relative
    require(
        object_table,
        mapping["object_table_absolute_offset"],
        "object table absolute offset",
    )
    object_offset = object_table + (mapping["object"] - 1) * 24
    relocation_base = u32le(executable, object_offset + 4)
    first_page = u32le(executable, object_offset + 12)
    page_count = u32le(executable, object_offset + 16)
    require(relocation_base, mapping["object_relocation_base"], "object base")
    require(first_page, mapping["object_first_page"], "object first page")
    require(page_count, mapping["object_page_count"], "object page count")

    virtual_address = mapping["target_virtual_address"]
    target_object_offset = virtual_address - relocation_base
    require(
        target_object_offset,
        mapping["target_object_offset"],
        "target object offset",
    )
    object_page_ordinal, in_page_offset = divmod(target_object_offset, page_size)
    require(
        object_page_ordinal,
        mapping["target_object_page_ordinal"],
        "object page ordinal",
    )
    require(in_page_offset, mapping["target_in_page_offset"], "in-page offset")
    if object_page_ordinal >= page_count:
        fail("target lies beyond object page count")

    page_map_entry = header + page_map_relative + (first_page - 1 + object_page_ordinal) * 4
    require(
        page_map_entry,
        mapping["target_page_map_entry_absolute_offset"],
        "page map entry offset",
    )
    page_map_bytes = executable[page_map_entry : page_map_entry + 4]
    require(page_map_bytes.hex(), mapping["target_page_map_entry_hex"], "page map entry")
    physical_page = int.from_bytes(page_map_bytes[:3], "big")
    require(physical_page, mapping["target_physical_page"], "physical page")
    require(page_map_bytes[3], 0, "page flags")

    file_offset = data_pages_absolute + (physical_page - 1) * page_size + in_page_offset
    require(file_offset, mapping["target_file_offset"], "target file offset")
    window = executable[file_offset : file_offset + window_claim["size"]]
    require(window.hex(), window_claim["hex"], "instruction byte window")
    require(hashlib.sha256(window).hexdigest(), window_claim["sha256"], "window sha256")

    decoded = evidence["decoded_instructions"]
    cursor = 0
    for instruction in decoded:
        encoded = bytes.fromhex(instruction["bytes"])
        require(
            virtual_address + cursor,
            instruction["virtual_address"],
            f"instruction {instruction['instruction']} address",
        )
        require(
            window[cursor : cursor + len(encoded)],
            encoded,
            f"instruction {instruction['instruction']} bytes",
        )
        cursor += len(encoded)

    print(
        "MC_GOAT_TMAP_LATCH_CLEAR PASS "
        f"exe_sha256={source['sha256']} "
        f"va=0x{virtual_address:X} store_va=0x{decoded[1]['virtual_address']:X} "
        f"file_offset=0x{file_offset:X} bytes={window[:cursor].hex()}"
    )


if __name__ == "__main__":
    try:
        main()
    except (KeyError, OSError, TypeError, ValueError) as error:
        fail(str(error))
