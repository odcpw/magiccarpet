#!/usr/bin/env python3
"""Reproduce and verify a complete file tree from a raw MODE1/2352 image.

The source image and inventory are opened read-only. The destination must not
already exist, is created with owner-only permissions, and is removed if any
file fails verification.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil

from list_mode1_2352_iso import LOGICAL_BLOCK_BYTES, RawMode1Iso


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def safe_relative_path(value: str) -> Path:
    path = Path(value)
    if path.is_absolute() or not path.parts or any(part in {"", ".", ".."} for part in path.parts):
        raise ValueError(f"unsafe inventory path: {value!r}")
    return path


def write_verified_extent(
    image: RawMode1Iso,
    destination: Path,
    extent_lba: int,
    size: int,
    expected_sha256: str,
) -> None:
    destination.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    descriptor = os.open(
        destination,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL,
        0o600,
    )
    digest = hashlib.sha256()
    try:
        with os.fdopen(descriptor, "wb") as output:
            remaining = size
            block = extent_lba
            while remaining:
                payload = image.read_block(block)
                take = min(remaining, LOGICAL_BLOCK_BYTES)
                chunk = payload[:take]
                output.write(chunk)
                digest.update(chunk)
                remaining -= take
                block += 1
            output.flush()
            os.fsync(output.fileno())
    except BaseException:
        destination.unlink(missing_ok=True)
        raise
    observed_sha256 = digest.hexdigest()
    if observed_sha256 != expected_sha256:
        destination.unlink(missing_ok=True)
        raise ValueError(
            f"extracted file SHA-256 mismatch for {destination}: "
            f"expected {expected_sha256}, observed {observed_sha256}"
        )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--image", required=True, type=Path)
    parser.add_argument("--inventory", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--expected-image-sha256", required=True)
    args = parser.parse_args()

    image_path = args.image.resolve()
    inventory_path = args.inventory.resolve()
    output_path = args.output.resolve()
    if output_path.exists():
        raise FileExistsError(f"destination already exists: {output_path}")

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

    entries = inventory["entries"]
    directory_entries = [entry for entry in entries if entry["kind"] == "directory"]
    file_entries = [entry for entry in entries if entry["kind"] == "file"]
    if any(not entry.get("sha256") for entry in file_entries):
        raise ValueError("inventory contains a file without a recorded SHA-256")

    output_path.mkdir(mode=0o700, parents=False)
    extracted_bytes = 0
    image = RawMode1Iso(image_path)
    try:
        for entry in directory_entries:
            (output_path / safe_relative_path(entry["path"])).mkdir(
                mode=0o700,
                parents=True,
                exist_ok=True,
            )
        for entry in file_entries:
            write_verified_extent(
                image,
                output_path / safe_relative_path(entry["path"]),
                int(entry["extent_lba"]),
                int(entry["size"]),
                entry["sha256"],
            )
            extracted_bytes += int(entry["size"])
    except BaseException:
        image.close()
        shutil.rmtree(output_path)
        raise
    else:
        image.close()

    result = {
        "image": str(image_path),
        "image_sha256": observed_image_sha256,
        "inventory": str(inventory_path),
        "output": str(output_path),
        "directory_count": len(directory_entries),
        "file_count": len(file_entries),
        "bytes": extracted_bytes,
        "all_file_hashes_verified": True,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
