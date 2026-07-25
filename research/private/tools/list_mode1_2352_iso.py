#!/usr/bin/env python3
"""List and hash ISO-9660 files inside a raw 2352-byte Mode-1 data track.

This parser reads payload bytes in place. It never mounts or extracts the
image. It intentionally supports only the simple single-extent layout needed
for the owned MC2 disc; unsupported interleave or multi-extent records fail.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from dataclasses import asdict, dataclass
from pathlib import Path


RAW_SECTOR_BYTES = 2352
MODE1_DATA_OFFSET = 16
LOGICAL_BLOCK_BYTES = 2048
PVD_LBA = 16


@dataclass(frozen=True)
class Entry:
    path: str
    kind: str
    extent_lba: int
    size: int
    flags: int
    recorded_at: str
    sha256: str | None = None


class RawMode1Iso:
    def __init__(self, path: Path) -> None:
        self.path = path
        self.handle = path.open("rb")

    def close(self) -> None:
        self.handle.close()

    def read_block(self, lba: int) -> bytes:
        self.handle.seek(lba * RAW_SECTOR_BYTES + MODE1_DATA_OFFSET)
        data = self.handle.read(LOGICAL_BLOCK_BYTES)
        if len(data) != LOGICAL_BLOCK_BYTES:
            raise ValueError(f"short read at logical block {lba}")
        return data

    def read_extent(self, lba: int, size: int) -> bytes:
        output = bytearray()
        remaining = size
        block = lba
        while remaining:
            payload = self.read_block(block)
            take = min(remaining, LOGICAL_BLOCK_BYTES)
            output.extend(payload[:take])
            remaining -= take
            block += 1
        return bytes(output)

    def hash_extent(self, lba: int, size: int) -> str:
        digest = hashlib.sha256()
        remaining = size
        block = lba
        while remaining:
            payload = self.read_block(block)
            take = min(remaining, LOGICAL_BLOCK_BYTES)
            digest.update(payload[:take])
            remaining -= take
            block += 1
        return digest.hexdigest()


def both_endian_u32(raw: bytes, offset: int) -> int:
    little = int.from_bytes(raw[offset:offset + 4], "little")
    big = int.from_bytes(raw[offset + 4:offset + 8], "big")
    if little != big:
        raise ValueError(f"both-endian u32 mismatch at record offset {offset}")
    return little


def recorded_at(raw: bytes) -> str:
    year = 1900 + raw[0]
    zone_quarters = int.from_bytes(raw[6:7], "little", signed=True)
    sign = "+" if zone_quarters >= 0 else "-"
    minutes = abs(zone_quarters) * 15
    return (
        f"{year:04d}-{raw[1]:02d}-{raw[2]:02d}T"
        f"{raw[3]:02d}:{raw[4]:02d}:{raw[5]:02d}"
        f"{sign}{minutes // 60:02d}:{minutes % 60:02d}"
    )


def parse_record(raw: bytes, parent: str) -> tuple[Entry, str]:
    if len(raw) < 34 or raw[0] != len(raw):
        raise ValueError("invalid ISO directory record length")
    extent_lba = both_endian_u32(raw, 2)
    size = both_endian_u32(raw, 10)
    flags = raw[25]
    if flags & 0x80:
        raise ValueError("multi-extent ISO file is unsupported")
    if raw[26] != 0 or raw[27] != 0:
        raise ValueError("interleaved ISO file is unsupported")
    identifier_length = raw[32]
    identifier = raw[33:33 + identifier_length]
    if identifier == b"\x00":
        name = "."
    elif identifier == b"\x01":
        name = ".."
    else:
        name = identifier.decode("ascii")
        if ";" in name:
            name = name.rsplit(";", 1)[0]
    path = name if not parent else f"{parent}/{name}"
    kind = "directory" if flags & 0x02 else "file"
    return Entry(
        path=path,
        kind=kind,
        extent_lba=extent_lba,
        size=size,
        flags=flags,
        recorded_at=recorded_at(raw[18:25]),
    ), name


def directory_records(image: RawMode1Iso, entry: Entry) -> list[tuple[Entry, str]]:
    data = image.read_extent(entry.extent_lba, entry.size)
    records: list[tuple[Entry, str]] = []
    offset = 0
    parent = "" if entry.path == "." else entry.path
    while offset < len(data):
        length = data[offset]
        if length == 0:
            offset = ((offset // LOGICAL_BLOCK_BYTES) + 1) * LOGICAL_BLOCK_BYTES
            continue
        end = offset + length
        if end > len(data):
            raise ValueError(f"directory record exceeds extent at {entry.path}")
        records.append(parse_record(data[offset:end], parent))
        offset = end
    return records


def walk(image: RawMode1Iso, root: Entry, hash_files: bool) -> list[Entry]:
    output: list[Entry] = []
    queue = [root]
    visited_directories: set[tuple[int, int]] = set()
    while queue:
        directory = queue.pop(0)
        identity = (directory.extent_lba, directory.size)
        if identity in visited_directories:
            continue
        visited_directories.add(identity)
        for entry, name in directory_records(image, directory):
            if name in {".", ".."}:
                continue
            if entry.kind == "directory":
                output.append(entry)
                queue.append(entry)
            else:
                if hash_files:
                    entry = Entry(
                        **{
                            **asdict(entry),
                            "sha256": image.hash_extent(entry.extent_lba, entry.size),
                        }
                    )
                output.append(entry)
    return sorted(output, key=lambda item: item.path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("image", type=Path)
    parser.add_argument("--hash-files", action="store_true")
    args = parser.parse_args()

    image_path = args.image.resolve()
    image = RawMode1Iso(image_path)
    try:
        pvd = image.read_block(PVD_LBA)
        if pvd[:7] != b"\x01CD001\x01":
            raise ValueError("logical block 16 is not an ISO-9660 PVD")
        logical_block_size = int.from_bytes(pvd[128:130], "little")
        if logical_block_size != LOGICAL_BLOCK_BYTES:
            raise ValueError(
                f"unexpected ISO logical block size {logical_block_size}"
            )
        root_length = pvd[156]
        root, root_name = parse_record(pvd[156:156 + root_length], "")
        if root_name != "." or root.kind != "directory":
            raise ValueError("PVD root record is invalid")
        volume_id = pvd[40:72].decode("ascii").rstrip()
        records = walk(image, root, args.hash_files)
    finally:
        image.close()

    result = {
        "schema_version": 1,
        "image": str(image_path),
        "image_bytes": image_path.stat().st_size,
        "raw_sector_bytes": RAW_SECTOR_BYTES,
        "mode1_data_offset": MODE1_DATA_OFFSET,
        "logical_block_bytes": LOGICAL_BLOCK_BYTES,
        "volume_id": volume_id,
        "entry_count": len(records),
        "entries": [asdict(record) for record in records],
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
