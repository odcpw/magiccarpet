#!/usr/bin/env python3
"""Fail-closed working-tree contamination scanner for a public candidate."""

from __future__ import annotations

import argparse
import codecs
import hashlib
import json
import os
import re
import stat
import sys
from pathlib import Path
from typing import Any, NamedTuple


class FileInspection(NamedTuple):
    digest: str
    prefix: bytes
    is_text: bool
    forbidden_fragments: tuple[str, ...]


def inspect_file(path: Path, fragments: list[str]) -> FileInspection:
    """Hash and classify a file without size cutoffs or whole-file reads."""
    digest = hashlib.sha256()
    decoder = codecs.getincrementaldecoder("utf-8")("strict")
    is_text = True
    prefix = bytearray()
    encoded_fragments = {
        fragment: fragment.encode("utf-8") for fragment in fragments
    }
    found: set[str] = set()
    max_fragment = max((len(value) for value in encoded_fragments.values()), default=1)
    tail = b""

    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
            if len(prefix) < 4096:
                prefix.extend(chunk[:4096 - len(prefix)])
            search = tail + chunk
            for fragment, encoded in encoded_fragments.items():
                if fragment not in found and encoded in search:
                    found.add(fragment)
            tail = search[-(max_fragment - 1):] if max_fragment > 1 else b""
            if is_text:
                try:
                    decoded = decoder.decode(chunk)
                except UnicodeDecodeError:
                    is_text = False
                else:
                    if any(
                        not (character.isprintable() or character in "\t\n\r")
                        for character in decoded
                    ):
                        is_text = False

    if is_text:
        try:
            decoded = decoder.decode(b"", final=True)
        except UnicodeDecodeError:
            is_text = False
        else:
            if any(
                not (character.isprintable() or character in "\t\n\r")
                for character in decoded
            ):
                is_text = False
    return FileInspection(
        digest.hexdigest(), bytes(prefix), is_text, tuple(sorted(found))
    )


def parse_lfs_pointer(raw: bytes, size: int) -> tuple[bool, str | None, str | None]:
    """Recognize canonical and extension-bearing Git LFS pointer text."""
    version = "version https://git-lfs.github.com/spec/v1"
    if not raw.startswith(version.encode("ascii")):
        return False, None, None
    if size > 4096:
        return True, None, "pointer exceeds 4096-byte safety limit"
    try:
        lines = raw.decode("ascii").splitlines()
    except UnicodeDecodeError:
        return False, None, None
    if not lines or lines[0] != version:
        return False, None, None

    oid: str | None = None
    declared_size: int | None = None
    for line in lines[1:]:
        if line.startswith("ext-"):
            if re.fullmatch(r"ext-[A-Za-z0-9][A-Za-z0-9.-]* .+", line) is None:
                return True, None, "invalid extension line"
        elif line.startswith("oid sha256:"):
            candidate = line.removeprefix("oid sha256:")
            if oid is not None or re.fullmatch(r"[0-9a-f]{64}", candidate) is None:
                return True, None, "invalid or duplicate oid"
            oid = candidate
        elif line.startswith("size "):
            candidate = line.removeprefix("size ")
            if declared_size is not None or not candidate.isdecimal():
                return True, None, "invalid or duplicate size"
            declared_size = int(candidate)
        else:
            return True, None, "unknown pointer line"
    if oid is None or declared_size is None:
        return True, None, "missing oid or size"
    return True, oid, None


def detect_magic(prefix: bytes) -> str | None:
    if prefix.startswith(b"MZ"):
        return "pe"
    if prefix.startswith(b"\x7fELF"):
        return "elf"
    if prefix.startswith(b"\x00asm"):
        return "wasm"
    if prefix.startswith((b"\x4c\x01", b"\x64\x86", b"\xc4\x01", b"\x64\xaa")):
        return "coff"
    if prefix.startswith((b"\xfe\xed\xfa\xce", b"\xce\xfa\xed\xfe",
                          b"\xfe\xed\xfa\xcf", b"\xcf\xfa\xed\xfe",
                          b"\xca\xfe\xba\xbe")):
        return "macho"
    if prefix.startswith(b"!<arch>\n"):
        return "ar"
    if prefix.startswith((b"PK\x03\x04", b"PK\x05\x06", b"PK\x07\x08")):
        return "zip"
    if prefix.startswith(b"Rar!\x1a\x07"):
        return "rar"
    if prefix.startswith(b"7z\xbc\xaf\x27\x1c"):
        return "seven_zip"
    if prefix.startswith(b"\x1f\x8b"):
        return "gzip"
    if prefix.startswith(b"BZh"):
        return "bzip2"
    if prefix.startswith(b"\xfd7zXZ\x00"):
        return "xz"
    if prefix.startswith(b"\x28\xb5\x2f\xfd"):
        return "zstd"
    if prefix.startswith(b"\x04\x22\x4d\x18"):
        return "lz4"
    if prefix.startswith(b"MSCF"):
        return "cab"
    if len(prefix) >= 265 and prefix[257:262] == b"ustar":
        return "tar"
    if prefix.startswith(b"%PDF-"):
        return "pdf"
    if prefix.startswith(b"\x89PNG\r\n\x1a\n"):
        return "png"
    if prefix.startswith(b"\xff\xd8\xff"):
        return "jpeg"
    if prefix.startswith((b"GIF87a", b"GIF89a")):
        return "gif"
    if prefix.startswith(b"OggS"):
        return "ogg"
    if prefix.startswith(b"fLaC"):
        return "flac"
    if prefix.startswith((b"ID3", b"\xff\xfb", b"\xff\xf3", b"\xff\xf2")):
        return "mp3"
    if len(prefix) >= 12 and prefix[4:8] == b"ftyp":
        return "iso_bmff"
    if prefix.startswith(b"\x1aE\xdf\xa3"):
        return "matroska"
    if prefix.startswith(b"RIFF"):
        if prefix[8:12] == b"WEBP":
            return "webp"
        return "riff"
    if prefix.startswith(b"DDS "):
        return "dds"
    if prefix.startswith((b"\x00\x01\x00\x00", b"OTTO")):
        return "truetype"
    if prefix.startswith((b"wOFF", b"wOF2")):
        return "woff"
    if prefix.startswith((b"II*\x00", b"MM\x00*")):
        return "tiff"
    if prefix.startswith(b"8BPS"):
        return "psd"
    if prefix.startswith(b"glTF"):
        return "glb"
    if prefix.startswith(b"BLENDER"):
        return "blend"
    return None


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ValueError(f"{path}: top level must be an object")
    return value


def load_assets(
    path: Path | None,
    required: set[str],
    allowed_classes: set[str],
    forbidden_fragments: list[str],
) -> tuple[dict[str, dict[str, Any]], list[str]]:
    if path is None:
        return {}, []
    data = load_json(path)
    errors: list[str] = []
    result: dict[str, dict[str, Any]] = {}
    unexpected_top_level = sorted(set(data) - {"schema_version", "assets"})
    if data.get("schema_version") != 1:
        errors.append(f"{path}: schema_version must equal 1")
    if unexpected_top_level:
        errors.append(
            f"{path}: unexpected top-level fields "
            f"{', '.join(unexpected_top_level)}"
        )
    assets = data.get("assets")
    if not isinstance(assets, list):
        return {}, [f"{path}: assets must be an array"]
    for index, item in enumerate(assets):
        if not isinstance(item, dict):
            errors.append(f"{path}: assets[{index}] must be an object")
            continue
        missing = sorted(required - item.keys())
        unexpected = sorted(set(item) - required)
        if missing:
            errors.append(f"{path}: assets[{index}] missing {', '.join(missing)}")
            continue
        if unexpected:
            errors.append(
                f"{path}: assets[{index}] has unexpected fields "
                f"{', '.join(unexpected)}"
            )
            continue
        rel = item.get("path")
        digest = item.get("sha256")
        if (
            not isinstance(rel, str)
            or not rel.strip()
            or Path(rel).is_absolute()
            or ".." in Path(rel).parts
            or "\\" in rel
            or Path(rel).as_posix() != rel
        ):
            errors.append(f"{path}: assets[{index}] has unsafe path")
            continue
        if not isinstance(digest, str) or re.fullmatch(r"[0-9a-f]{64}", digest) is None:
            errors.append(f"{path}: assets[{index}] has invalid sha256")
            continue
        if item.get("redistribution") is not True:
            errors.append(f"{path}: assets[{index}] is not approved for redistribution")
            continue
        string_fields = required - {"redistribution"}
        empty_fields = sorted(
            field for field in string_fields
            if not isinstance(item.get(field), str) or not item[field].strip()
        )
        if empty_fields:
            errors.append(
                f"{path}: assets[{index}] has empty/non-string "
                f"{', '.join(empty_fields)}"
            )
            continue
        if item["class"] not in allowed_classes:
            errors.append(
                f"{path}: assets[{index}] has disallowed class {item['class']!r}"
            )
            continue
        contaminated_fields = sorted(
            field
            for field, value in item.items()
            if isinstance(value, str)
            and any(
                fragment.casefold() in value.casefold()
                for fragment in forbidden_fragments
            )
        )
        if contaminated_fields:
            errors.append(
                f"{path}: assets[{index}] contains forbidden private marker in "
                f"{', '.join(contaminated_fields)}"
            )
            continue
        if rel in result:
            errors.append(f"{path}: duplicate asset path {rel}")
            continue
        result[rel] = item
    return result, errors


def scan(root: Path, policy_path: Path, manifest_path: Path | None) -> list[str]:
    policy = load_json(policy_path)
    required = set(policy["required_asset_fields"])
    allowed_classes = set(policy["allowed_asset_classes"])
    forbidden_text = list(policy["forbidden_text_fragments"])
    assets, errors = load_assets(
        manifest_path, required, allowed_classes, forbidden_text
    )
    forbidden_hashes = {
        item["sha256"]: item.get("source_id", "unknown")
        for item in policy["forbidden_sha256"]
    }
    forbidden_suffixes = {value.lower() for value in policy["forbidden_suffixes"]}
    forbidden_directory_suffixes = {
        value.lower() for value in policy["forbidden_directory_suffixes"]
    }
    conditional_suffixes = {
        value.lower() for value in policy["manifest_required_suffixes"]
    }
    forbidden_magic = set(policy["forbidden_magic"])
    conditional_magic = set(policy["manifest_required_magic"])
    forbidden_paths = [
        value.replace("\\", "/").casefold()
        for value in policy["forbidden_path_fragments"]
    ]

    seen_assets: set[str] = set()
    for current, dirs, files in os.walk(root, topdown=True, followlinks=False):
        current_path = Path(current)
        dirs[:] = sorted(dirs)
        files.sort()
        for name in list(dirs):
            path = current_path / name
            rel = path.relative_to(root).as_posix()
            folded_rel = rel.casefold()
            git_like = name.casefold().rstrip(" .") == ".git"
            if path.is_symlink():
                errors.append(f"SYMLINK {rel}")
                dirs.remove(name)
            elif name == ".git" and current_path == root:
                dirs.remove(name)
            elif git_like:
                errors.append(f"NESTED_GIT_DIRECTORY {rel}/")
                dirs.remove(name)
            elif any(name.lower().endswith(suffix)
                     for suffix in forbidden_directory_suffixes):
                errors.append(f"FORBIDDEN_DIRECTORY_SUFFIX {rel}/")
                dirs.remove(name)
            elif any(fragment in folded_rel for fragment in forbidden_paths):
                errors.append(f"FORBIDDEN_PATH {rel}/")
                dirs.remove(name)

        for name in files:
            path = current_path / name
            rel = path.relative_to(root).as_posix()
            folded_rel = rel.casefold()
            if name == ".git" and current_path == root:
                continue
            if name.casefold().rstrip(" .") == ".git":
                errors.append(f"NESTED_GIT_FILE {rel}")
                continue
            try:
                mode = path.lstat().st_mode
            except FileNotFoundError:
                errors.append(f"RACE_DISAPPEARED {rel}")
                continue
            if stat.S_ISLNK(mode):
                errors.append(f"SYMLINK {rel}")
                continue
            if not stat.S_ISREG(mode):
                errors.append(f"NON_REGULAR {rel}")
                continue
            if any(fragment in folded_rel for fragment in forbidden_paths):
                errors.append(f"FORBIDDEN_PATH {rel}")
                continue

            resolved_path = path.resolve()
            text_control_file = (
                resolved_path == policy_path
            )
            inspection = inspect_file(
                path, [] if text_control_file else forbidden_text
            )
            lfs, lfs_oid, lfs_error = parse_lfs_pointer(
                inspection.prefix, path.stat().st_size
            )
            if lfs and lfs_error is not None:
                errors.append(f"INVALID_LFS_POINTER {rel} {lfs_error}")
            content_hash = lfs_oid if lfs_oid is not None else inspection.digest

            if content_hash in forbidden_hashes:
                errors.append(
                    f"FORBIDDEN_HASH {rel} {content_hash} {forbidden_hashes[content_hash]}"
                )

            suffix = path.suffix.lower()
            magic = detect_magic(inspection.prefix)
            if suffix in forbidden_suffixes:
                errors.append(f"FORBIDDEN_SUFFIX {rel} {suffix}")
            if magic in forbidden_magic:
                errors.append(f"FORBIDDEN_MAGIC {rel} {magic}")

            binary_payload = not inspection.is_text and not lfs
            known_asset_kind = (
                suffix in conditional_suffixes
                or magic in conditional_magic
                or lfs
            )
            conditional = (
                known_asset_kind
                or binary_payload
            )
            entry = assets.get(rel)
            if entry is not None:
                if entry["sha256"] != content_hash:
                    errors.append(
                        f"ASSET_HASH_MISMATCH {rel} "
                        f"expected={entry['sha256']} actual={content_hash}"
                    )
                else:
                    seen_assets.add(rel)
            if conditional:
                if entry is None:
                    kind = "ASSET" if known_asset_kind else "BINARY"
                    errors.append(f"UNMANIFESTED_{kind} {rel}")

            for fragment in inspection.forbidden_fragments:
                errors.append(f"FORBIDDEN_TEXT {rel} {fragment!r}")

    for rel in sorted(set(assets) - seen_assets):
        errors.append(f"MANIFEST_ENTRY_MISSING_FILE {rel}")
    return sorted(set(errors))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=Path)
    parser.add_argument("--policy", type=Path, required=True)
    parser.add_argument("--asset-manifest", type=Path)
    args = parser.parse_args()

    root = args.root.resolve()
    if not root.is_dir():
        parser.error(f"root is not a directory: {root}")
    errors = scan(root, args.policy.resolve(),
                  args.asset_manifest.resolve() if args.asset_manifest else None)
    if errors:
        for error in errors:
            print(error)
        print(f"PUBLIC_BOUNDARY_FAIL count={len(errors)}", file=sys.stderr)
        return 1
    print("PUBLIC_BOUNDARY_PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
