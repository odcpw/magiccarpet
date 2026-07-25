#!/usr/bin/env python3
"""Build a deterministic private catalogue of an owner's MC2 data.

The catalogue contains hashes, offsets, dimensions, palette bindings, and
decoder status. It never copies source bytes into the generated output.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import io
import json
import os
import re
import struct
import sys
import tempfile
import wave
from collections import Counter
from pathlib import Path
from typing import Any


HERE = Path(__file__).resolve().parent
LAB_ROOT = HERE.parents[2]
DEFAULT_OWNER_ROOT = LAB_ROOT / "research/private/remc2-native-runtime/CD_Files"
DEFAULT_ENHANCED_ROOT = LAB_ROOT / "upstream/magic-carpet-2-hd/enhancedassets"
DEFAULT_OUTPUT = HERE / "semantics/generated/owner-asset-catalog.json"
PRIVATE_ROOT = LAB_ROOT / "research/private"

SCHEMA_VERSION = 1
RNC_HEADER_SIZE = 18
RNC_SIGNATURES = {b"RNC\x01": 1, b"RNC\x02": 2}
TILE_TAB_RECORD = struct.Struct("<iih")
BITMAP_TAB_RECORD = struct.Struct("<IBB")

REALM_PALETTES = {
    "D": "DATA/PALD-0.DAT",
    "N": "DATA/PALN-0.DAT",
    "C": "DATA/PALC-0.DAT",
}
REALM_PALETTE_VARIANTS = {
    "D": ("DATA/PALD-0.DAT",),
    "N": ("DATA/PALN-0.DAT", "DATA/PALF-0.DAT"),
    "C": ("DATA/PALC-0.DAT",),
}

REQUIRED_GROUPS = {
    "levels": ("LEVELS/LEVELS.DAT", "LEVELS/LEVELS.TAB"),
    "sprites": tuple(
        f"DATA/{resolution}SPR{realm}0-0.{suffix}"
        for resolution in ("M", "H")
        for realm in ("D", "N", "C")
        for suffix in ("DAT", "TAB")
    ),
    "texture_maps": tuple(
        f"DATA/TMAPS{realm}-0.{suffix}"
        for realm in range(3)
        for suffix in ("DAT", "TAB")
    ),
    "palettes": (
        "DATA/PALD-0.DAT",
        "DATA/PALN-0.DAT",
        "DATA/PALC-0.DAT",
        "DATA/PALF-0.DAT",
    ),
    "skies": (
        "DATA/SKYD0-0.DAT",
        "DATA/SKYN0-0.DAT",
        "DATA/SKYC0-0.DAT",
    ),
    "blocks": (
        "DATA/BLOCK16.DAT",
        "DATA/BLOCK32.DAT",
        "DATA/BL16F0-0.DAT",
        "DATA/BL16N0-0.DAT",
        "DATA/BL16C0-0.DAT",
        "DATA/BL32F0-0.DAT",
        "DATA/BL32N0-0.DAT",
        "DATA/BL32C0-0.DAT",
    ),
    "buildings": (
        "DATA/BUILD0-0.DAT",
        "DATA/BUILD0-0.TAB",
        "DATA/BLDGPRM.DAT",
    ),
    "spells_and_tables": (
        "DATA/SPELLS.DAT",
        "DATA/TABLES.DAT",
        "DATA/TABLESD.DAT",
        "DATA/TABLESN.DAT",
        "DATA/TABLESC.DAT",
    ),
    "sound_and_music": ("SOUND/SOUND.DAT", "SOUND/MUSIC.DAT"),
    "web": tuple(
        f"DATA/{resolution}WEB{realm}0-0.{suffix}"
        for resolution in ("M", "H")
        for realm in ("D", "N", "C")
        for suffix in ("DAT", "TAB")
    ),
    "fonts": tuple(
        f"DATA/{stem}.{suffix}"
        for stem in (
            "FONT0",
            "FONT1",
            "FONT2",
            "DFONT0",
            "DFONT1",
            "SFONT1",
            "HFONT3",
        )
        for suffix in ("DAT", "TAB")
    ),
    "hud": (
        "DATA/BUTTON.DAT",
        "DATA/BUTTON.TAB",
        "DATA/POINTERS.DAT",
        "DATA/POINTERS.TAB",
        "DATA/SCREENS/HSCREEN0.DAT",
        "DATA/SMALTIT.DAT",
        "DATA/SMALTIT.PAL",
        "DATA/SMATITLE.DAT",
        "DATA/SMATITLE.PAL",
        "DATA/SMATITL2.DAT",
        "DATA/SMATITL2.PAL",
        "DATA/TITLE3.DAT",
        "DATA/TITBASF.DAT",
    ),
    "localization": (
        "LANGUAGE/D2.TXT",
        "LANGUAGE/L1.TXT",
        "DATA/ETEXT.DAT",
        "DATA/FTEXT.DAT",
        "DATA/GTEXT.DAT",
        "DATA/ITEXT.DAT",
        "DATA/SPTEXT.DAT",
        "DATA/SWTEXT.DAT",
        "DATA/FTDEFC.DAT",
        "DATA/FTDEFD.DAT",
        "DATA/FTDEFN.DAT",
        "DATA/GTDEFC.DAT",
        "DATA/GTDEFD.DAT",
        "DATA/GTDEFN.DAT",
        "DATA/GTD2.DAT",
    ),
}

ALL_REQUIRED_FILES = tuple(
    sorted({path for paths in REQUIRED_GROUPS.values() for path in paths})
)


class CatalogInputError(ValueError):
    """The configured private source cannot produce a complete catalogue."""


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")


def classify(relative: str, provenance: str) -> str:
    upper = relative.upper()
    name = Path(upper).name
    if provenance == "remc2_enhanced_assets":
        if "/TMAPS/" in upper:
            return "enhanced_texture_map"
        if "/FONTS/" in upper:
            return "enhanced_font"
        if "/MENU/" in upper or "/UI/" in upper:
            return "enhanced_hud"
        if "MUSIC" in upper:
            return "enhanced_music"
        if "SPEECH" in upper:
            return "enhanced_speech"
        if upper.startswith("LANGUAGES/"):
            return "enhanced_localization"
        return "enhanced_graphics"
    if upper.startswith("LEVELS/"):
        return "level_records"
    if name.startswith(("MSPR", "HSPR")):
        return "world_sprites"
    if name.startswith("TMAPS"):
        return "terrain_texture_maps"
    if name.startswith("PAL") or name.endswith(".PAL"):
        return "palette"
    if name.startswith("SKY"):
        return "sky"
    if name.startswith(("BLOCK", "BL16", "BL32")):
        return "terrain_blocks"
    if name.startswith(("BUILD", "BLDGPRM")):
        return "buildings"
    if "FONT" in name:
        return "font"
    if name.startswith(("MWEB", "HWEB")):
        return "hud_web"
    if name.startswith(("BUTTON", "POINTER", "SCREEN", "TITLE", "TITBAS")):
        return "hud_or_frontend"
    if (
        upper.startswith("LANGUAGE/")
        or "TEXT" in name
        or "TDEF" in name
        or name.startswith("GTD")
    ):
        return "localization"
    if name == "SPELLS.DAT":
        return "spell_parameters"
    if name.startswith("TABLES"):
        return "simulation_tables"
    if name == "SOUND.DAT":
        return "sound_bank"
    if name == "MUSIC.DAT":
        return "music_bank"
    if upper.startswith("INTRO/"):
        return "cinematic"
    if upper.startswith("SOUND/"):
        return "dos_audio_driver"
    return "other"


def png_dimensions(path: Path) -> tuple[int | None, int | None]:
    with path.open("rb") as handle:
        header = handle.read(24)
    if len(header) == 24 and header[:8] == b"\x89PNG\r\n\x1a\n":
        return struct.unpack(">II", header[16:24])
    return None, None


def wav_dimensions(path: Path) -> tuple[int | None, int | None, int | None]:
    try:
        with wave.open(str(path), "rb") as handle:
            return (
                handle.getnchannels(),
                handle.getframerate(),
                handle.getnframes(),
            )
    except (wave.Error, EOFError):
        return None, None, None


def rnc_header(data: bytes, offset: int = 0) -> dict[str, Any] | None:
    if offset < 0 or offset + RNC_HEADER_SIZE > len(data):
        return None
    signature = data[offset : offset + 4]
    if signature not in RNC_SIGNATURES:
        return None
    unpacked, packed = struct.unpack_from(">II", data, offset + 4)
    unpacked_crc, packed_crc = struct.unpack_from(">HH", data, offset + 12)
    leeway = data[offset + 16]
    chunks = data[offset + 17]
    return {
        "method": RNC_SIGNATURES[signature],
        "header_size": RNC_HEADER_SIZE,
        "unpacked_size": unpacked,
        "packed_payload_size": packed,
        "archive_size": packed + RNC_HEADER_SIZE,
        "unpacked_crc16": unpacked_crc,
        "packed_crc16": packed_crc,
        "leeway": leeway,
        "chunk_count": chunks,
    }


def base_file_record(path: Path, root: Path, provenance: str) -> dict[str, Any]:
    relative = path.relative_to(root).as_posix()
    suffix = path.suffix.casefold()
    width = height = None
    media: dict[str, Any] = {}
    decoder_status = "hashed_opaque"
    if suffix == ".png":
        width, height = png_dimensions(path)
        decoder_status = "png_header_valid" if width is not None else "png_invalid"
    elif suffix == ".wav":
        channels, rate, frames = wav_dimensions(path)
        media = {
            "channels": channels,
            "sample_rate_hz": rate,
            "sample_frames": frames,
        }
        decoder_status = "wav_header_valid" if channels is not None else "wav_invalid"
    record = {
        "asset_id": f"{provenance}:{relative}",
        "provenance": provenance,
        "path": relative,
        "category": classify(relative, provenance),
        "size": path.stat().st_size,
        "sha256": sha256_file(path),
        "decoder_status": decoder_status,
        "dimensions": {"width": width, "height": height},
    }
    if media:
        record["media"] = media
    record["content_address"] = f"sha256:{record['sha256']}"
    if provenance == "remc2_enhanced_assets":
        match = re.fullmatch(
            r"TMAPS2-(?P<realm>[012])-(?P<texture>\d{3})-(?P<frame>\d{2})\.data",
            path.name,
            re.IGNORECASE,
        )
        if match:
            record["logical_binding"] = {
                "realm": int(match.group("realm")),
                "texture_index": int(match.group("texture")),
                "animation_frame": int(match.group("frame")),
            }
            record["decoder_status"] = "remc2_enhanced_data_schema_external"
    return record


def source_files(root: Path) -> list[Path]:
    files: list[Path] = []
    for path in root.rglob("*"):
        if path.is_symlink():
            raise CatalogInputError(f"symlink is not allowed in source root: {path}")
        if not path.is_file():
            continue
        resolved = path.resolve()
        try:
            resolved.relative_to(root)
        except ValueError as error:
            raise CatalogInputError(
                f"source path escapes configured root: {path}"
            ) from error
        files.append(path)
    return sorted(files)


def validate_owner_root(owner_root: Path) -> Path:
    root = owner_root.expanduser().resolve()
    if not root.is_dir():
        raise CatalogInputError(f"owner root does not exist: {root}")
    missing = [relative for relative in ALL_REQUIRED_FILES if not (root / relative).is_file()]
    if missing:
        preview = ", ".join(missing[:8])
        suffix = f" (+{len(missing) - 8} more)" if len(missing) > 8 else ""
        raise CatalogInputError(
            f"owner root is missing {len(missing)} required sources: {preview}{suffix}"
        )
    return root


def decode_indexed_rle(
    dat: bytes, offset: int, width: int, height: int
) -> dict[str, Any]:
    if not width or not height:
        return {
            "status": "empty",
            "encoded_length": 0,
            "indexed_pixels_sha256": sha256_bytes(b""),
            "transparency_mask_sha256": sha256_bytes(b""),
        }
    cursor = offset
    pixels = bytearray(width * height)
    visible = bytearray(width * height)
    try:
        for y in range(height):
            x = 0
            while True:
                if cursor >= len(dat):
                    raise ValueError("RLE exceeds DAT")
                token = dat[cursor]
                cursor += 1
                if token == 0:
                    break
                if token & 0x80:
                    x += 256 - token
                    if x > width:
                        raise ValueError("transparent run exceeds row width")
                    continue
                if cursor + token > len(dat) or x + token > width:
                    raise ValueError("pixel run exceeds source or row width")
                start = y * width + x
                pixels[start : start + token] = dat[cursor : cursor + token]
                visible[start : start + token] = b"\x01" * token
                x += token
                cursor += token
        return {
            "status": "decoded_indexed_rle",
            "encoded_length": cursor - offset,
            "indexed_pixels_sha256": sha256_bytes(bytes(pixels)),
            "transparency_mask_sha256": sha256_bytes(bytes(visible)),
        }
    except ValueError as error:
        return {
            "status": "unresolved_decode_error",
            "encoded_length": max(cursor - offset, 0),
            "error": str(error),
            "indexed_pixels_sha256": None,
            "transparency_mask_sha256": None,
        }


def parse_bitmap_pair(
    dat_path: Path,
    tab_path: Path,
    owner_root: Path,
    palette: str | None,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    relative_dat = dat_path.relative_to(owner_root).as_posix()
    relative_tab = tab_path.relative_to(owner_root).as_posix()
    dat = dat_path.read_bytes()
    tab = tab_path.read_bytes()
    unresolved: list[dict[str, Any]] = []
    if len(tab) % BITMAP_TAB_RECORD.size:
        unresolved.append(
            {
                "asset_id": f"owner_data:{relative_tab}",
                "issue": "TAB size is not divisible by the known 6-byte <IBB schema",
                "next_step": "recover the pack-specific TAB prefix or record schema",
            }
        )
        return [], unresolved
    entries: list[dict[str, Any]] = []
    for index, (offset, width, height) in enumerate(
        BITMAP_TAB_RECORD.iter_unpack(tab)
    ):
        decoded = decode_indexed_rle(dat, offset, width, height)
        encoded_length = decoded["encoded_length"]
        encoded_end = min(offset + encoded_length, len(dat))
        entry = {
            "asset_id": f"owner_data:{relative_dat}#{index}",
            "provenance": "owner_data",
            "category": classify(relative_dat, "owner_data"),
            "pack_path": relative_dat,
            "index_path": relative_tab,
            "index": index,
            "offset": offset,
            "dimensions": {"width": width, "height": height},
            "palette_path": palette,
            "palette_variants": (
                list(REALM_PALETTE_VARIANTS["N"])
                if palette == REALM_PALETTES["N"]
                else ([palette] if palette else [])
            ),
            "pixel_identity": "8_bit_palette_index_plus_visibility_mask",
            "decoder_status": decoded["status"],
            "encoded_length": encoded_length,
            "encoded_sha256": (
                sha256_bytes(dat[offset:encoded_end])
                if 0 <= offset <= encoded_end <= len(dat)
                else None
            ),
            "indexed_pixels_sha256": decoded["indexed_pixels_sha256"],
            "transparency_mask_sha256": decoded["transparency_mask_sha256"],
        }
        if entry["encoded_sha256"] is not None:
            entry["content_address"] = f"sha256:{entry['encoded_sha256']}"
        if "error" in decoded:
            entry["decoder_error"] = decoded["error"]
            unresolved.append(
                {
                    "asset_id": entry["asset_id"],
                    "issue": decoded["error"],
                    "next_step": "compare pack decoder semantics with remc2",
                }
            )
        if palette is None:
            unresolved.append(
                {
                    "asset_id": entry["asset_id"],
                    "issue": "runtime palette binding is not yet explicit",
                    "next_step": "trace the pack's active palette consumer",
                }
            )
        entries.append(entry)
    return entries, unresolved


def parse_level_records(
    dat_path: Path, tab_path: Path, owner_root: Path
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    dat = dat_path.read_bytes()
    tab = tab_path.read_bytes()
    relative_dat = dat_path.relative_to(owner_root).as_posix()
    unresolved: list[dict[str, Any]] = []
    if len(tab) != 4000:
        return [], [
            {
                "asset_id": "owner_data:LEVELS/LEVELS.TAB",
                "issue": f"expected 4000 bytes, observed {len(tab)}",
                "next_step": "verify owner-data edition and level-table schema",
            }
        ]
    offsets = list(struct.unpack("<1000I", tab))
    records = []
    for index in range(len(offsets) - 1):
        start, end = offsets[index], offsets[index + 1]
        if end <= start:
            continue
        status = "rnc_header_valid"
        header = rnc_header(dat, start)
        if (
            header is None
            and end <= len(dat)
            and end - start == 0x6604
        ):
            status = "raw_uncompressed_level_ready"
        elif (
            end > len(dat)
            or header is None
            or start + header["archive_size"] > end
        ):
            status = "unresolved_container"
            unresolved.append(
                {
                    "asset_id": f"owner_data:{relative_dat}#{index}",
                    "issue": "invalid offset span or RNC header",
                    "next_step": "compare LEVELS.TAB span with remc2 LevelDecompress_533B0",
                }
            )
        packed_hash = sha256_bytes(dat[start:end]) if end <= len(dat) else None
        record = {
                "asset_id": f"owner_data:{relative_dat}#{index}",
                "provenance": "owner_data",
                "category": "level_record",
                "pack_path": relative_dat,
                "index_path": "LEVELS/LEVELS.TAB",
                "level_index": index,
                "offset": start,
                "end_offset": end,
                "packed_span_size": end - start,
                "packed_span_sha256": packed_hash,
                "dimensions": {"width": None, "height": None},
                "decoder_status": status,
                "rnc": header,
                "expected_unpacked_level_size": 0x6604,
            }
        if packed_hash is not None:
            record["content_address"] = f"sha256:{packed_hash}"
        records.append(record)
    return records, unresolved


def parse_tmap_records(
    dat_path: Path, tab_path: Path, owner_root: Path, realm: int
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    dat = dat_path.read_bytes()
    tab = tab_path.read_bytes()
    relative_dat = dat_path.relative_to(owner_root).as_posix()
    relative_tab = tab_path.relative_to(owner_root).as_posix()
    unresolved: list[dict[str, Any]] = []
    if len(tab) % TILE_TAB_RECORD.size:
        return [], [
            {
                "asset_id": f"owner_data:{relative_tab}",
                "issue": "TMAPS TAB is not divisible by 10-byte <iiH schema",
                "next_step": "verify owner-data edition",
            }
        ]
    raw_entries = list(TILE_TAB_RECORD.iter_unpack(tab))
    records = []
    for index, (metadata, start, group) in enumerate(raw_entries[:-1]):
        end = raw_entries[index + 1][1]
        header = rnc_header(dat, start)
        status = "rnc_header_valid_not_decoded"
        if end - start == 1 and metadata == 1:
            status = "raw_uncompressed_tmap_payload"
        elif (
            start < 0
            or end < start
            or end > len(dat)
            or header is None
            or start + header["archive_size"] > end
        ):
            status = "unresolved_container"
            unresolved.append(
                {
                    "asset_id": f"owner_data:{relative_dat}#{index}",
                    "issue": "invalid TMAP offset span or RNC header",
                    "next_step": "run the remc2 DataFileRNC decoder and compare metadata",
                }
            )
        packed_hash = (
            sha256_bytes(dat[start:end])
            if 0 <= start <= end <= len(dat)
            else None
        )
        record = {
                "asset_id": f"owner_data:{relative_dat}#{index}",
                "provenance": "owner_data",
                "category": "terrain_texture_map",
                "realm": realm,
                "pack_path": relative_dat,
                "index_path": relative_tab,
                "index": index,
                "offset": start,
                "end_offset": end,
                "metadata_dword_0": metadata,
                "animation_or_allocation_group": group,
                "dimensions": {"width": None, "height": None},
                "dimension_status": "requires_RNC_decode_and_texture_schema",
                "pixel_identity": "8_bit_palette_index_after_RNC_decode",
                "palette_path": (
                    "DATA/PALD-0.DAT",
                    "DATA/PALN-0.DAT",
                    "DATA/PALC-0.DAT",
                )[realm],
                "palette_variants": list(
                    (
                        REALM_PALETTE_VARIANTS["D"],
                        REALM_PALETTE_VARIANTS["N"],
                        REALM_PALETTE_VARIANTS["C"],
                    )[realm]
                ),
                "decoder_status": status,
                "packed_span_size": end - start,
                "packed_span_sha256": packed_hash,
                "rnc": header,
            }
        if packed_hash is not None:
            record["content_address"] = f"sha256:{packed_hash}"
        records.append(record)
    return records, unresolved


def annotate_known_files(files: list[dict[str, Any]]) -> None:
    for item in files:
        if item["provenance"] != "owner_data":
            continue
        path = item["path"]
        size = item["size"]
        if item["category"] == "palette":
            item["decoder_status"] = (
                "vga_256x_rgb6_ready" if size == 768 else "unresolved_palette_size"
            )
            item["dimensions"] = {"width": 256, "height": 1}
            item["color_identity"] = "256 ordered RGB6 entries; do not bake into pixels"
        elif item["category"] == "sky" and size == 65536:
            item["decoder_status"] = "raw_indexed_256x256_ready"
            item["dimensions"] = {"width": 256, "height": 256}
            realm = Path(path).name[3]
            item["palette_path"] = REALM_PALETTES.get(realm)
        elif item["category"] == "spell_parameters":
            item["decoder_status"] = (
                "26_families_x_80_bytes_ready"
                if size == 26 * 80
                else "unresolved_spell_table_size"
            )
            item["record_count"] = 26 if size == 26 * 80 else None
        elif item["category"] == "simulation_tables":
            item["decoder_status"] = "hashed_runtime_table_schema_pending"
        elif item["category"] in {"sound_bank", "music_bank"}:
            item["decoder_status"] = "hashed_bank_decoder_pending"
        elif item["category"] == "localization":
            item["decoder_status"] = "hashed_localization_schema_pending"


def mark_indexed_sources(
    files: list[dict[str, Any]], entries: list[dict[str, Any]]
) -> None:
    by_path = {
        row["path"]: row
        for row in files
        if row["provenance"] == "owner_data"
    }
    statuses: dict[str, list[str]] = {}
    for entry in entries:
        for field in ("pack_path", "index_path"):
            path = entry.get(field)
            if path:
                statuses.setdefault(path, []).append(entry["decoder_status"])
    for path, entry_statuses in statuses.items():
        file_row = by_path.get(path)
        if file_row is None:
            continue
        if all(status in {"rnc_header_valid", "decoded_indexed_rle", "empty"} for status in entry_statuses):
            file_row["decoder_status"] = "indexed_entries_ready"
        elif any(status.startswith("unresolved") for status in entry_statuses):
            file_row["decoder_status"] = "indexed_entries_ready_with_explicit_gaps"
        else:
            file_row["decoder_status"] = "indexed_schema_ready_payload_pending"
        file_row["indexed_entry_count"] = len(entry_statuses)


def coverage_matrix(
    files: list[dict[str, Any]],
    entries: list[dict[str, Any]],
    unresolved: list[dict[str, Any]],
) -> dict[str, dict[str, int]]:
    owner_files = {
        row["path"]: row
        for row in files
        if row["provenance"] == "owner_data"
    }
    issue_ids = {row["asset_id"] for row in unresolved}
    matrix: dict[str, dict[str, int]] = {}
    for group, expected_paths in REQUIRED_GROUPS.items():
        path_set = set(expected_paths)
        group_entries = [
            row
            for row in entries
            if row.get("pack_path") in path_set or row.get("index_path") in path_set
        ]
        matrix[group] = {
            "expected_files": len(expected_paths),
            "catalogued_files": sum(path in owner_files for path in expected_paths),
            "missing_files": sum(path not in owner_files for path in expected_paths),
            "decoded_or_structurally_indexed_files": sum(
                path in owner_files
                and owner_files[path]["decoder_status"]
                in {
                    "indexed_entries_ready",
                    "indexed_entries_ready_with_explicit_gaps",
                    "indexed_schema_ready_payload_pending",
                    "vga_256x_rgb6_ready",
                    "raw_indexed_256x256_ready",
                    "26_families_x_80_bytes_ready",
                    "png_header_valid",
                    "wav_header_valid",
                }
                for path in expected_paths
            ),
            "unresolved_files": sum(
                path not in owner_files
                or any(
                    token in owner_files[path]["decoder_status"]
                    for token in ("opaque", "pending", "unresolved", "gaps")
                )
                for path in expected_paths
            ),
            "entry_count": len(group_entries),
            "structurally_valid_entries": sum(
                not row["decoder_status"].startswith("unresolved")
                for row in group_entries
            ),
            "payload_decoded_entries": sum(
                row["decoder_status"] in {"decoded_indexed_rle", "empty"}
                for row in group_entries
            ),
            "unresolved_container_or_decode_entries": sum(
                row["decoder_status"].startswith("unresolved")
                for row in group_entries
            ),
            "entries_with_unresolved_issues": sum(
                row["asset_id"] in issue_ids for row in group_entries
            ),
        }
    return matrix
def build_catalog(owner_root: Path, enhanced_root: Path | None) -> dict[str, Any]:
    owner_root = validate_owner_root(owner_root)
    provenance = [
        {
            "id": "owner_data",
            "classification": "OWNED_BINARY",
            "root_locator": "configured_owner_root",
            "distribution": "private_only",
        }
    ]
    files = [
        base_file_record(path, owner_root, "owner_data")
        for path in source_files(owner_root)
    ]
    if enhanced_root is not None:
        enhanced_root = enhanced_root.expanduser().resolve()
        if not enhanced_root.is_dir():
            raise CatalogInputError(
                f"enhanced asset root does not exist: {enhanced_root}"
            )
        provenance.append(
            {
                "id": "remc2_enhanced_assets",
                "classification": "UPSTREAM_PROJECT_ASSET",
                "root_locator": "configured_enhanced_root",
                "distribution": "separate_provenance_review_required",
            }
        )
        files.extend(
            base_file_record(path, enhanced_root, "remc2_enhanced_assets")
            for path in source_files(enhanced_root)
        )
    annotate_known_files(files)

    entries: list[dict[str, Any]] = []
    unresolved: list[dict[str, Any]] = []
    level_entries, issues = parse_level_records(
        owner_root / "LEVELS/LEVELS.DAT",
        owner_root / "LEVELS/LEVELS.TAB",
        owner_root,
    )
    entries.extend(level_entries)
    unresolved.extend(issues)

    for realm in range(3):
        parsed, issues = parse_tmap_records(
            owner_root / f"DATA/TMAPS{realm}-0.DAT",
            owner_root / f"DATA/TMAPS{realm}-0.TAB",
            owner_root,
            realm,
        )
        entries.extend(parsed)
        unresolved.extend(issues)

    for realm_code in ("D", "N", "C"):
        for resolution in ("M", "H"):
            stem = f"{resolution}SPR{realm_code}0-0"
            parsed, issues = parse_bitmap_pair(
                owner_root / f"DATA/{stem}.DAT",
                owner_root / f"DATA/{stem}.TAB",
                owner_root,
                REALM_PALETTES[realm_code],
            )
            entries.extend(parsed)
            unresolved.extend(issues)

    bitmap_pairs = (
        "BUILD0-0",
        "BUTTON",
        "POINTERS",
        "MWEBD0-0",
        "MWEBN0-0",
        "MWEBC0-0",
        "HWEBD0-0",
        "HWEBN0-0",
        "HWEBC0-0",
        "DFONT0",
        "DFONT1",
        "SFONT1",
        "HFONT3",
        "FONT0",
        "FONT1",
        "FONT2",
    )
    for stem in bitmap_pairs:
        dat_path = owner_root / f"DATA/{stem}.DAT"
        tab_path = owner_root / f"DATA/{stem}.TAB"
        if not dat_path.is_file() or not tab_path.is_file():
            continue
        realm_match = re.search(r"([DNC])0-0$", stem)
        palette = REALM_PALETTES.get(realm_match.group(1)) if realm_match else None
        parsed, issues = parse_bitmap_pair(
            dat_path, tab_path, owner_root, palette
        )
        entries.extend(parsed)
        unresolved.extend(issues)

    mark_indexed_sources(files, entries)
    for item in files:
        if item["decoder_status"] in {
            "hashed_opaque",
            "hashed_bank_decoder_pending",
            "hashed_runtime_table_schema_pending",
            "hashed_localization_schema_pending",
            "remc2_enhanced_data_schema_external",
        }:
            unresolved.append(
                {
                    "asset_id": item["asset_id"],
                    "issue": f"decoder status is {item['decoder_status']}",
                    "next_step": "retain exact hash and resolve only when referenced by a level",
                }
            )

    files.sort(key=lambda row: (row["provenance"], row["path"]))
    entries.sort(key=lambda row: row["asset_id"])
    unresolved.sort(key=lambda row: (row["asset_id"], row["issue"]))
    coverage = coverage_matrix(files, entries, unresolved)
    catalog = {
        "schema_version": SCHEMA_VERSION,
        "scope": "PRIVATE_TRACK_P_OWNER_DATA",
        "provenance": provenance,
        "identity_contract": {
            "source_bytes": "referenced by path and SHA-256; never embedded",
            "indexed_pixels": "palette indices and visibility masks remain separate",
            "palettes": "ordered RGB6 source tables remain independent assets",
            "enhanced_assets": "never merged with owner-data provenance",
        },
        "realm_contract": {
            "0": "day",
            "1": "night",
            "2": "cave",
            "level_1_regression_map_type": 1,
            "level_1_regression_realm": "night",
        },
        "schema_evidence": {
            "bitmap_tab": "remc2/portability/bitmap_pos_struct.h:<IBB>",
            "texture_tab": "remc2/engine/Basic.h:<iih>",
            "texture_rnc": "remc2/engine/TextureMaps.cpp:LoadTexture_72A10",
            "level_offsets": "remc2/engine/EventsFunctions.cpp:LevelDecompress_533B0",
            "sprite_rle": "remc2-atlas/build_semantic_backbone.py:decode_sprite",
        },
        "coverage_matrix": coverage,
        "coverage_contract": {
            "files": "expected_files = catalogued_files + missing_files",
            "entry_structure": (
                "entry_count = structurally_valid_entries + "
                "unresolved_container_or_decode_entries"
            ),
            "payload_and_issues": (
                "payload_decoded_entries and entries_with_unresolved_issues "
                "are independent axes and may overlap"
            ),
        },
        "files": files,
        "entries": entries,
        "unresolved": unresolved,
        "counts": {
            "files": len(files),
            "entries": len(entries),
            "unresolved": len(unresolved),
            "files_by_category": dict(
                sorted(Counter(row["category"] for row in files).items())
            ),
            "entries_by_category": dict(
                sorted(Counter(row["category"] for row in entries).items())
            ),
            "decoder_status": dict(
                sorted(
                    Counter(
                        row["decoder_status"] for row in files + entries
                    ).items()
                )
            ),
        },
    }
    catalog["catalog_sha256"] = sha256_bytes(canonical_bytes(catalog))
    return catalog


def validate_private_output(output: Path) -> Path:
    resolved = output.expanduser().resolve()
    try:
        resolved.relative_to(PRIVATE_ROOT.resolve())
    except ValueError as error:
        raise CatalogInputError(
            f"catalog output must remain under {PRIVATE_ROOT}: {resolved}"
        ) from error
    return resolved


def atomic_write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        mode="w",
        encoding="utf-8",
        newline="",
        dir=path.parent,
        prefix=f".{path.name}.",
        suffix=".tmp",
        delete=False,
    ) as handle:
        temporary = Path(handle.name)
        handle.write(content)
        handle.flush()
        os.fsync(handle.fileno())
    temporary.replace(path)


def write_outputs(catalog: dict[str, Any], output: Path) -> None:
    output = validate_private_output(output)
    atomic_write_text(
        output,
        json.dumps(catalog, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
    )
    summary_path = output.with_name("owner-asset-catalog-summary.json")
    summary = {
        "schema_version": catalog["schema_version"],
        "scope": catalog["scope"],
        "catalog_sha256": catalog["catalog_sha256"],
        "counts": catalog["counts"],
        "provenance": catalog["provenance"],
        "realm_contract": catalog["realm_contract"],
        "coverage_matrix": catalog["coverage_matrix"],
        "coverage_contract": catalog["coverage_contract"],
    }
    atomic_write_text(
        summary_path,
        json.dumps(summary, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
    )
    unresolved_path = output.with_name("owner-asset-catalog-unresolved.csv")
    buffer = io.StringIO(newline="")
    writer = csv.DictWriter(
        buffer, fieldnames=("asset_id", "issue", "next_step")
    )
    writer.writeheader()
    writer.writerows(catalog["unresolved"])
    atomic_write_text(unresolved_path, buffer.getvalue())


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    configured_owner = os.environ.get("MC2_OWNER_DATA_ROOT")
    parser.add_argument(
        "--owner-root",
        type=Path,
        default=Path(configured_owner) if configured_owner else None,
        required=configured_owner is None,
        help="external/private owned installation root (or MC2_OWNER_DATA_ROOT)",
    )
    parser.add_argument(
        "--enhanced-root",
        type=Path,
        default=None,
        help="optional, separately catalogued remc2 enhanced assets",
    )
    parser.add_argument(
        "--no-enhanced",
        action="store_true",
        help="omit the optional enhanced-assets provenance",
    )
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        catalog = build_catalog(
            args.owner_root, None if args.no_enhanced else args.enhanced_root
        )
        write_outputs(catalog, args.output)
    except CatalogInputError as error:
        print(
            json.dumps(
                {"error": "catalog_input_error", "message": str(error)},
                sort_keys=True,
            ),
            file=sys.stderr,
        )
        return 2
    print(
        json.dumps(
            {
                "catalog": str(args.output),
                "catalog_sha256": catalog["catalog_sha256"],
                "counts": catalog["counts"],
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
