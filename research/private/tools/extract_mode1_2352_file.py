#!/usr/bin/env python3
"""Reproduce one file from a hashed raw MODE1/2352 ISO inventory.

The source image is opened read-only. The destination is created exclusively
with owner-only permissions and is never overwritten.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path

from list_mode1_2352_iso import (
    LOGICAL_BLOCK_BYTES,
    RawMode1Iso,
)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def selected_entry(inventory: dict[str, object], iso_path: str) -> dict[str, object]:
    matches = [
        entry
        for entry in inventory["entries"]
        if entry["path"].casefold() == iso_path.casefold()
    ]
    if len(matches) != 1:
        raise ValueError(
            f"expected exactly one inventory entry for {iso_path!r}; "
            f"found {len(matches)}"
        )
    entry = matches[0]
    if entry["kind"] != "file":
        raise ValueError(f"{entry['path']!r} is not a file")
    if not entry.get("sha256"):
        raise ValueError(f"{entry['path']!r} has no recorded SHA-256")
    return entry


def write_extent_exclusively(
    image: RawMode1Iso,
    output_path: Path,
    extent_lba: int,
    size: int,
) -> None:
    output_path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    descriptor = os.open(
        output_path,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL,
        0o600,
    )
    try:
        with os.fdopen(descriptor, "wb") as output:
            remaining = size
            block = extent_lba
            while remaining:
                payload = image.read_block(block)
                take = min(remaining, LOGICAL_BLOCK_BYTES)
                output.write(payload[:take])
                remaining -= take
                block += 1
            output.flush()
            os.fsync(output.fileno())
    except BaseException:
        output_path.unlink(missing_ok=True)
        raise


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--image", required=True, type=Path)
    parser.add_argument("--inventory", required=True, type=Path)
    parser.add_argument("--iso-path", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--expected-image-sha256", required=True)
    args = parser.parse_args()

    image_path = args.image.resolve()
    inventory_path = args.inventory.resolve()
    output_path = args.output.resolve()

    observed_image_sha256 = sha256_file(image_path)
    if observed_image_sha256 != args.expected_image_sha256:
        raise ValueError(
            "source image SHA-256 mismatch: "
            f"expected {args.expected_image_sha256}, "
            f"observed {observed_image_sha256}"
        )

    with inventory_path.open("r", encoding="utf-8") as handle:
        inventory = json.load(handle)
    if inventory["image_bytes"] != image_path.stat().st_size:
        raise ValueError("source image size differs from the inventory")

    entry = selected_entry(inventory, args.iso_path)
    image = RawMode1Iso(image_path)
    try:
        write_extent_exclusively(
            image,
            output_path,
            int(entry["extent_lba"]),
            int(entry["size"]),
        )
    finally:
        image.close()

    observed_output_sha256 = sha256_file(output_path)
    if observed_output_sha256 != entry["sha256"]:
        output_path.unlink(missing_ok=True)
        raise ValueError(
            "extracted file SHA-256 mismatch: "
            f"expected {entry['sha256']}, observed {observed_output_sha256}"
        )

    print(
        json.dumps(
            {
                "image": str(image_path),
                "image_sha256": observed_image_sha256,
                "inventory": str(inventory_path),
                "iso_path": entry["path"],
                "extent_lba": entry["extent_lba"],
                "bytes": entry["size"],
                "output": str(output_path),
                "output_sha256": observed_output_sha256,
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
