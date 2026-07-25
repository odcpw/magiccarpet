#!/usr/bin/env python3
"""Build and validate the private, owner-local Level 1 Unity asset cache."""

from __future__ import annotations

import argparse
import csv
import hashlib
import importlib.util
import json
import os
import re
import shutil
import struct
import subprocess
import tempfile
import wave
from io import BytesIO
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parents[3]
PRIVATE = REPO / "research/private"
GENERATED = PRIVATE / "generated"
UNITY = PRIVATE / "remc2-unity-native"
DEFAULT_OWNER = PRIVATE / "remc2-native-runtime/CD_Files"
DEFAULT_PACKAGE = GENERATED / "level1-export-m"
DEFAULT_CLOSURE = GENERATED / "level1-asset-closure.json"
DEFAULT_CATALOG = PRIVATE / "remc2-atlas/semantics/generated/owner-asset-catalog.json"
DEFAULT_OUTPUT = GENERATED / "level1-unity-asset-cache"
PROVEN_DECODER = PRIVATE / "remc2-atlas/build_owner_asset_catalog.py"
PROVEN_CLOSURE = PRIVATE / "remc2-level-asset-closure/build_closure.py"
SPELL_TABLE = PRIVATE / "remc2-atlas/semantics/generated/spell-tiers.csv"
SPELL_GENERATOR = PRIVATE / "remc2-atlas/build_semantic_backbone.py"
RNC_HELPER = Path(__file__).with_name("rnc_decode.cpp")
XMI_HELPER = Path(__file__).with_name("xmi_convert.cpp")
ADL_RENDER_HELPER = Path(__file__).with_name("adl_render.cpp")
ADLMIDI_SOURCE = REPO / "upstream/magic-carpet-2-hd/libADLMIDI-master"
XMI_IMPLEMENTATION = (
    REPO / "upstream/magic-carpet-2-hd/remc2/portability/xmi2mid.cpp"
)
XMI_HEADER = REPO / "upstream/magic-carpet-2-hd/remc2/portability/xmi2mid.h"
RNC_DECODER_CPP = REPO / "upstream/magic-carpet-2-hd/remc2/utilities/DataFileRNC.cpp"
RNC_DECODER_HEADER = REPO / "upstream/magic-carpet-2-hd/remc2/utilities/DataFileRNC.h"
REMC2_TEXTURE_MAPPING = (
    REPO / "upstream/magic-carpet-2-hd/remc2/engine/EventsFunctions.cpp"
)
REMC2_UV_TABLE = REPO / "upstream/magic-carpet-2-hd/remc2/engine/UVTable_D4350.cpp"
REMC2_MATHS_TABLE = REPO / "upstream/magic-carpet-2-hd/remc2/utilities/Maths.cpp"

SCHEMA = "mc2-owner-unity-asset-cache/v1"
GENERATOR_VERSION = "1.7.0"
EXPECTED_PACKAGE = "730d2fa084265df839f3f2b55ed766ac167a8b152ca4631b5ded9d0bb9104ac8"
EXPECTED_CLOSURE = "1c3774ca5c032c1a7b8b1c48870840eee33011a2830d08c51f3104fb5c5bcc12"
FRAME_MSPRN = (1, 40, 41, 42, 43, 54, 123, 124, 171, 172, 173)
DYNAMIC_MSPRN = (9,)
FRAME_TMAPS = (40, 41, 54, 63)
LEVEL1_TERRAIN_SELECTORS = (
    0, 1, 2, 4, 5, 9, 22, 23, 24, 25, 26, 27, 36, 37, 38, 39, 44, 45,
    46, 47, 52, 53, 54, 55, 56, 57, 58, 59, 64, 65, 66, 67, 69, 70, 71,
    72, 73, 74, 75, 76, 84, 85, 86, 87, 88, 89, 97, 99, 101, 114, 115,
    116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 128, 131, 136, 139,
)
UNITY_TERRAIN_UV_GROUPS = (
    ((0, 1), (1, 1), (0, 0), (1, 0)),
    ((1, 1), (0, 1), (1, 0), (0, 0)),
    ((0, 0), (1, 0), (0, 1), (1, 1)),
    ((1, 0), (0, 0), (1, 1), (0, 1)),
    ((0, 1), (0, 0), (1, 1), (1, 0)),
    ((0, 0), (0, 1), (1, 0), (1, 1)),
    ((1, 1), (1, 0), (0, 1), (0, 0)),
    ((1, 0), (1, 1), (0, 0), (0, 1)),
)
SOURCE_PATHS = (
    "DATA/BL32N0-0.DAT",
    "DATA/CLRN-0.DAT",
    "DATA/MSPRN0-0.DAT",
    "DATA/MSPRN0-0.TAB",
    "DATA/PALN-0.DAT",
    "DATA/SKYN0-0.DAT",
    "DATA/SPELLS.DAT",
    "DATA/FONT1.DAT",
    "DATA/FONT1.TAB",
    "DATA/TMAPS1-0.DAT",
    "DATA/TMAPS1-0.TAB",
    "DATA/TABLESN.DAT",
    "LANGUAGE/D2.TXT",
    "LANGUAGE/L1.TXT",
    "SOUND/MUSIC.DAT",
    "SOUND/SOUND.DAT",
)
TAB_RECORD = struct.Struct("<IBB")
TMAP_TAB_RECORD = struct.Struct("<iiH")
TMAP_HEADER = struct.Struct("<HHH")
ENTITY_HEADER = struct.Struct("<4sHHI")
PRESENTATION_HEADER = struct.Struct("<4sHHI")
PARTICLE_RECORD = struct.Struct("<HHHHH BbBB")
ENTITY_ACTOR_SPECS = {
    (2, 2, 6, 59, 39): ("dolmen", 79, 0, 400),
    (5, 1, 15, 98, 238): ("goat", 152, 20, 150),
    (5, 13, 111, 100, 239): ("villager-a", 155, 20, 200),
    (5, 13, 111, 100, 241): ("villager-b", 180, 20, 250),
    (5, 13, 111, 100, 242): ("villager-c", 183, 20, 250),
    (5, 13, 111, 100, 271): ("villager-d", 199, 20, 300),
}
ENTITY_EFFECT_SPECS = {
    (10, 0, 0, 59, 7): ("fireball-impact", 38, 36, 250),
    (10, 1, 1, 59, 41): ("explosion-death", 54, 21, 250),
    (10, 14, 14, 59, 9): ("smoke-particle", 63, 21, 50),
}
EXPECTED_ACTOR_COUNTS = {
    "dolmen": 3,
    "goat": 33,
    "villager-a": 3,
    "villager-b": 6,
    "villager-c": 9,
    "villager-d": 10,
}
YAW_LOOKUP_D4750_28 = (0, 0, 0, 1, 1, 1, 2, 2, 2, 2, 2, 1, 1, 1, 0, 0)
BL32_TILE_SIDE = 32
BL32_TILE_BYTES = BL32_TILE_SIDE * BL32_TILE_SIDE
BL32_ATLAS_WIDTH = 256
BL32_ATLAS_COLUMNS = BL32_ATLAS_WIDTH // BL32_TILE_SIDE
LANGUAGE_PAYLOAD_OFFSET = 4785
LANGUAGE_RECORD_COUNT = 471
LEVEL1_OBJECTIVE_SELECTOR = 48
LEVEL1_OBJECTIVE_IDS = (48, 49, 50, 51, 52, 53)
LEVEL1_REFERENCE_WIDTH = 640
LEVEL1_REFERENCE_HEIGHT = 480
LEVEL1_MINIMAP_DISPLAY_SIDE = 128
LEVEL1_MINIMAP_LOGICAL_SIDE = 64
LEVEL1_MINIMAP_SOURCE_SCALE = 512
HUD_SOURCE_SCALE = 2
LEVEL1_SOUND_IDS = (1, 2, 5, 12, 29, 31, 37, 39, 43, 46)
LEVEL1_MATERIALIZED_SOUND_IDS = (1, 2, 5, 29, 31, 37, 39, 43, 46)
SOUND_QUALITY_ROWS = (1644, 1622, 1611, 822, 811, 800)


class CacheError(ValueError):
    pass


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_json(value: Any) -> bytes:
    return (
        json.dumps(value, sort_keys=True, indent=2, ensure_ascii=True) + "\n"
    ).encode("utf-8")


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise CacheError(f"cannot read canonical JSON {path}: {error}") from error


def confined_source(root: Path, relative: str) -> Path:
    candidate = root / relative
    if candidate.is_symlink():
        raise CacheError(f"owner source must not be a symlink: {relative}")
    resolved = candidate.resolve()
    try:
        resolved.relative_to(root)
    except ValueError as error:
        raise CacheError(f"owner source escapes root: {relative}") from error
    if not resolved.is_file():
        raise CacheError(f"owner source missing: {relative}")
    return resolved


def validate_output_path(output: Path) -> Path:
    resolved = output.expanduser().resolve()
    try:
        resolved.relative_to(PRIVATE.resolve())
    except ValueError as error:
        raise CacheError("cache output must remain under research/private") from error
    for forbidden in (
        UNITY / "Assets",
        UNITY / "StreamingAssets",
        UNITY / "Builds",
    ):
        try:
            resolved.relative_to(forbidden.resolve())
        except ValueError:
            continue
        raise CacheError(f"cache output is a distributable path: {resolved}")
    if resolved == PRIVATE.resolve():
        raise CacheError("cache output cannot be the private root")
    return resolved


def load_proven_decoder():
    spec = importlib.util.spec_from_file_location("owner_catalog_decoder", PROVEN_DECODER)
    if spec is None or spec.loader is None:
        raise CacheError("cannot load proven owner catalog decoder")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.decode_indexed_rle


def load_proven_export_verifier():
    spec = importlib.util.spec_from_file_location("level_asset_closure", PROVEN_CLOSURE)
    if spec is None or spec.loader is None:
        raise CacheError("cannot load proven package identity verifier")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.verify_export_identity


def materialize_indexed_rle(
    dat: bytes, offset: int, width: int, height: int
) -> tuple[bytes, bytes, int]:
    if width == 0 or height == 0:
        return b"", b"", 0
    cursor = offset
    pixels = bytearray(width * height)
    visible = bytearray(width * height)
    for y in range(height):
        x = 0
        while True:
            if cursor >= len(dat):
                raise CacheError("RLE exceeds DAT")
            token = dat[cursor]
            cursor += 1
            if token == 0:
                break
            if token & 0x80:
                x += 256 - token
                if x > width:
                    raise CacheError("transparent run exceeds row width")
                continue
            if cursor + token > len(dat) or x + token > width:
                raise CacheError("pixel run exceeds source or row width")
            start = y * width + x
            pixels[start : start + token] = dat[cursor : cursor + token]
            visible[start : start + token] = b"\x01" * token
            cursor += token
            x += token
    return bytes(pixels), bytes(visible), cursor - offset


def bitmap_records(tab: bytes) -> list[tuple[int, int, int]]:
    if len(tab) % TAB_RECORD.size:
        raise CacheError("TAB is truncated or not the proven six-byte <IBB schema")
    return list(TAB_RECORD.iter_unpack(tab))


def language_records(payload: bytes) -> list[bytes]:
    if len(payload) <= LANGUAGE_PAYLOAD_OFFSET:
        raise CacheError("owner localization payload is shorter than the proven prefix")
    records = payload[LANGUAGE_PAYLOAD_OFFSET:].split(b"\0")
    if len(records) < LANGUAGE_RECORD_COUNT:
        raise CacheError("owner localization payload has fewer than 471 records")
    result = records[:LANGUAGE_RECORD_COUNT]
    if any(b"\0" in value for value in result):
        raise CacheError("owner localization record retained a NUL byte")
    return result


def build_minimap_aperture(
    tables: bytes,
) -> tuple[bytes, list[dict[str, int]]]:
    if len(tables) != 83456:
        raise CacheError("TABLESN extent changed before minimap aperture decode")
    side = LEVEL1_MINIMAP_LOGICAL_SIDE
    half = side // 2
    phase = 0
    phase_step = 0x1000000 // half
    bounds: list[tuple[int, int] | None] = [None] * side
    for index in range(half):
        table_index = 0x14300 + (phase >> 16)
        half_span = (half * tables[table_index]) >> 8
        row = (half - half_span, half + half_span)
        bounds[half + index] = row
        bounds[half - 1 - index] = row
        phase += phase_step
    if any(row is None for row in bounds):
        raise CacheError("minimap aperture left an uninitialized scanline")
    scanlines = [
        {"y": y, "start_x_inclusive": row[0], "end_x_exclusive": row[1]}
        for y, row in enumerate(bounds)
        if row is not None
    ]
    mask = bytes(
        1 if row[0] <= x < row[1] else 0
        for row in bounds
        if row is not None
        for x in range(side)
    )
    if (
        len(mask) != side * side
        or sum(mask) != 3204
        or sha256_bytes(mask) !=
            "98a3a7c0300b49204629aab1b4b552253bea2223f9e68c40d1458b2e62243285"
    ):
        raise CacheError("qualified 64x64 TABLESN minimap aperture changed")
    return mask, scanlines


def load_remc2_sine_table() -> tuple[list[int], bytes]:
    text = REMC2_MATHS_TABLE.read_text(encoding="utf-8")
    declaration = "int32_t Maths::sin_DB750[2560]"
    start = text.index("{", text.index(declaration)) + 1
    end = text.index("};", start)
    tokens = re.findall(r"0x[0-9A-Fa-f]+|(?<![A-Za-z_])-?\d+", text[start:end])
    if len(tokens) != 2560:
        raise CacheError("remc2 Maths::sin_DB750 extent changed")
    values = []
    for token in tokens:
        value = int(token, 0)
        if value >= 0x80000000:
            value -= 0x100000000
        values.append(value)
    payload = struct.pack("<2560i", *values)
    if (
        values[0] != 0
        or values[512] != 65536
        or values[1024] != 0
        or values[1536] != -65536
        or values[2048] != 0
    ):
        raise CacheError("remc2 Maths::sin_DB750 cardinal values changed")
    return values, payload


def trunc_div(numerator: int, denominator: int) -> int:
    if denominator <= 0:
        raise CacheError("positive minimap denominator required")
    return (
        numerator // denominator
        if numerator >= 0
        else -((-numerator) // denominator)
    )


def minimap_source_fixed(
    player_x: int,
    player_y: int,
    yaw: int,
    screen_x: int,
    screen_y: int,
    sine_table: list[int],
) -> tuple[int, int]:
    width = LEVEL1_MINIMAP_LOGICAL_SIDE
    height = LEVEL1_MINIMAP_LOGICAL_SIDE
    scaling = LEVEL1_MINIMAP_SOURCE_SCALE
    angle = yaw & 0x7FF
    sine_scaled = (scaling * sine_table[angle]) >> 16
    cosine_scaled = (scaling * sine_table[angle + 0x200]) >> 16
    column_x = trunc_div(height * cosine_scaled, width)
    column_y = trunc_div(height * sine_scaled, width)
    origin_x = player_x - trunc_div(
        width * column_x - height * sine_scaled, 2
    )
    origin_y = player_y - trunc_div(
        height * cosine_scaled + width * column_y, 2
    )
    return (
        origin_x - screen_y * sine_scaled + screen_x * column_x,
        origin_y + screen_y * cosine_scaled + screen_x * column_y,
    )


def build_minimap_direction_samples(
    player_x: int,
    player_y: int,
    terrain_type: bytes,
    terrain_shade: bytes,
    tables: bytes,
    sine_table: list[int],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    points = (
        ("top", 32, 16),
        ("right", 48, 32),
        ("bottom", 32, 48),
        ("left", 16, 32),
    )
    rows = []
    for yaw in (0, 512, 1024, 1536):
        for label, screen_x, screen_y in points:
            fixed_x, fixed_y = minimap_source_fixed(
                player_x, player_y, yaw, screen_x, screen_y, sine_table
            )
            source_x = fixed_x & 0xFFFF
            source_y = fixed_y & 0xFFFF
            tile_x = source_x >> 8
            tile_y = source_y >> 8
            tile = tile_x + tile_y * 256
            terrain_base = tables[0x14000 + terrain_type[tile]]
            palette_index = tables[(terrain_shade[tile] << 8) | terrain_base]
            rows.append({
                "yaw": yaw,
                "screen_label": label,
                "screen_x": screen_x,
                "screen_y": screen_y,
                "source_fixed_x": source_x,
                "source_fixed_y": source_y,
                "source_tile_x": tile_x,
                "source_tile_y": tile_y,
                "terrain_type": terrain_type[tile],
                "terrain_shade": terrain_shade[tile],
                "terrain_base_index": terrain_base,
                "palette_index": palette_index,
            })
    probe_rows = []
    for yaw in range(0, 2048, 256):
        fixed_x, fixed_y = minimap_source_fixed(
            player_x, player_y, yaw, 42, 22, sine_table
        )
        probe_rows.append({
            "yaw": yaw,
            "screen_x": 42,
            "screen_y": 22,
            "source_fixed_x": fixed_x & 0xFFFF,
            "source_fixed_y": fixed_y & 0xFFFF,
            "source_tile_x": (fixed_x >> 8) & 255,
            "source_tile_y": (fixed_y >> 8) & 255,
        })
    return rows, probe_rows


def draw_mask_rgba(
    canvas: bytearray,
    canvas_width: int,
    canvas_height: int,
    source: bytes,
    width: int,
    height: int,
    x: int,
    y: int,
    scale: int,
    rgba: tuple[int, int, int, int],
) -> None:
    for source_y in range(height):
        for source_x in range(width):
            if not source[source_y * width + source_x]:
                continue
            for scale_y in range(scale):
                target_y = y + source_y * scale + scale_y
                if target_y < 0 or target_y >= canvas_height:
                    continue
                for scale_x in range(scale):
                    target_x = x + source_x * scale + scale_x
                    if target_x < 0 or target_x >= canvas_width:
                        continue
                    offset = (target_y * canvas_width + target_x) * 4
                    canvas[offset:offset + 4] = bytes(rgba)


def draw_indexed_rgba(
    canvas: bytearray,
    canvas_width: int,
    canvas_height: int,
    indices: bytes,
    mask: bytes,
    width: int,
    height: int,
    x: int,
    y: int,
    scale: int,
    palette_rgb8: bytes,
) -> None:
    for source_y in range(height):
        for source_x in range(width):
            source_offset = source_y * width + source_x
            if not mask[source_offset]:
                continue
            palette_offset = indices[source_offset] * 3
            rgba = (
                palette_rgb8[palette_offset],
                palette_rgb8[palette_offset + 1],
                palette_rgb8[palette_offset + 2],
                255,
            )
            draw_mask_rgba(
                canvas,
                canvas_width,
                canvas_height,
                b"\1",
                1,
                1,
                x + source_x * scale,
                y + source_y * scale,
                scale,
                rgba,
            )


def build_objective_rasters(
    text: bytes,
    glyph_rows: list[dict[str, Any]],
    font_blob: bytes,
    sprites: dict[int, tuple[bytes, bytes, int, int]],
    palette_rgb8: bytes,
    text_palette_index: int,
) -> tuple[bytes, bytes, dict[str, Any]]:
    objective_width = 256
    objective_height = 38
    objective = bytearray(objective_width * objective_height * 4)
    scale = HUD_SOURCE_SCALE
    corner = sprites[171]
    vertical = sprites[172]
    horizontal = sprites[173]

    for x in (0, 240):
        draw_indexed_rgba(
            objective, objective_width, objective_height,
            *corner, x, 0, scale, palette_rgb8
        )
        draw_indexed_rgba(
            objective, objective_width, objective_height,
            *corner, x, 26, scale, palette_rgb8
        )
    for y in (12,):
        draw_indexed_rgba(
            objective, objective_width, objective_height,
            *vertical, 0, y, scale, palette_rgb8
        )
        draw_indexed_rgba(
            objective, objective_width, objective_height,
            *vertical, 240, y, scale, palette_rgb8
        )
    for x in range(16, 240, 32):
        draw_indexed_rgba(
            objective, objective_width, objective_height,
            *horizontal, x, 0, scale, palette_rgb8
        )
        draw_indexed_rgba(
            objective, objective_width, objective_height,
            *horizontal, x, 28, scale, palette_rgb8
        )

    palette_offset = text_palette_index * 3
    text_rgba = (
        palette_rgb8[palette_offset],
        palette_rgb8[palette_offset + 1],
        palette_rgb8[palette_offset + 2],
        255,
    )
    cursor_x = 32
    glyph_sequence = []
    for value in text:
        glyph_index = value + 1
        row = glyph_rows[glyph_index]
        glyph_sequence.append(glyph_index)
        if value != 32:
            start = row["offset"]
            draw_mask_rgba(
                objective,
                objective_width,
                objective_height,
                font_blob[start:start + row["length"]],
                row["width"],
                row["height"],
                cursor_x,
                12,
                scale,
                text_rgba,
            )
        cursor_x += glyph_rows[33 if value == 32 else glyph_index]["width"] * scale
    if cursor_x != 224:
        raise CacheError("Level 1 objective FONT1 advance changed")
    objective_advance = cursor_x - 32

    badge_text = b"FROZEN OBSERVED"
    badge_width = sum(
        glyph_rows[33 if value == 32 else value + 1]["width"]
        for value in badge_text
    )
    badge_height = max(glyph_rows[value + 1]["height"] for value in badge_text if value != 32)
    badge = bytearray(badge_width * badge_height * 4)
    cursor_x = 0
    badge_glyphs = []
    for value in badge_text:
        glyph_index = value + 1
        badge_glyphs.append(glyph_index)
        row = glyph_rows[glyph_index]
        if value != 32:
            start = row["offset"]
            draw_mask_rgba(
                badge,
                badge_width,
                badge_height,
                font_blob[start:start + row["length"]],
                row["width"],
                row["height"],
                cursor_x,
                0,
                1,
                text_rgba,
            )
        cursor_x += glyph_rows[33 if value == 32 else glyph_index]["width"]
    return bytes(objective), bytes(badge), {
        "objective_width": objective_width,
        "objective_height": objective_height,
        "objective_text_origin": {"x": 32, "y": 12},
        "objective_glyph_indices": glyph_sequence,
        "objective_text_advance": objective_advance,
        "badge_text_ascii": badge_text.decode("ascii"),
        "badge_width": badge_width,
        "badge_height": badge_height,
        "badge_glyph_indices": badge_glyphs,
    }


def tmap_records(tab: bytes) -> list[tuple[int, int, int]]:
    if len(tab) % TMAP_TAB_RECORD.size:
        raise CacheError("TMAPS TAB is truncated or not the proven ten-byte <iiH schema")
    return list(TMAP_TAB_RECORD.iter_unpack(tab))


def build_entity_presentation_descriptor(
    entities_blob: bytes,
    presentation_blob: bytes,
    terrain_blob: bytes,
    tmap_rows: list[dict[str, Any]],
) -> dict[str, Any]:
    if len(entities_blob) < ENTITY_HEADER.size:
        raise CacheError("entities.bin header is truncated")
    magic, version, record_size, count = ENTITY_HEADER.unpack_from(entities_blob)
    if magic != b"MCE1" or version != 1 or record_size != 176 or count != 1000:
        raise CacheError("entities.bin is not the qualified MCE1/1 1000x176 form")
    if len(entities_blob) != ENTITY_HEADER.size + record_size * count:
        raise CacheError("entities.bin record extent changed")
    if len(terrain_blob) != 458752:
        raise CacheError("terrain.bin extent changed")
    terrain_heads = {
        value
        for value in struct.unpack_from("<65536h", terrain_blob, 5 * 65536)
        if value > 0
    }
    if len(terrain_heads) != 130:
        raise CacheError("qualified Level 1 terrain billboard head count changed")
    if len(presentation_blob) < PRESENTATION_HEADER.size:
        raise CacheError("presentation.bin header is truncated")
    pmagic, pversion, reserved, particle_count = PRESENTATION_HEADER.unpack_from(
        presentation_blob
    )
    if (
        pmagic != b"MCV1"
        or pversion != 1
        or reserved != 0
        or particle_count != 347
    ):
        raise CacheError("presentation.bin is not the qualified MCV1/1 form")
    particle_start = PRESENTATION_HEADER.size
    particle_end = particle_start + particle_count * PARTICLE_RECORD.size
    if particle_end + 4 > len(presentation_blob):
        raise CacheError("presentation particle table is truncated")
    particles = [
        PARTICLE_RECORD.unpack_from(
            presentation_blob, particle_start + index * PARTICLE_RECORD.size
        )
        for index in range(particle_count)
    ]
    selector_count = struct.unpack_from("<I", presentation_blob, particle_end)[0]
    selector_end = particle_end + 4 + selector_count
    if selector_count != 504 or selector_end + 4 > len(presentation_blob):
        raise CacheError("presentation selector table changed")
    parameter_count = struct.unpack_from("<I", presentation_blob, selector_end)[0]
    if parameter_count != 157 or selector_end + 4 + parameter_count * 34 != len(
        presentation_blob
    ):
        raise CacheError("presentation parameter table changed")

    texture_rows = {row["index"]: row for row in tmap_rows}
    grouped: dict[tuple[int, int, int, int | None, int, int, int], list[int]] = {}
    for index in range(count):
        start = ENTITY_HEADER.size + index * record_size
        record = entities_blob[start:start + record_size]
        if record[2] == 0:
            continue
        class_id = record[67]
        model = record[68]
        action = record[73]
        parameter_relation = struct.unpack_from("<I", record, 164)[0]
        parameter_index = parameter_relation - 1 if parameter_relation else None
        particle_index = struct.unpack_from("<h", record, 94)[0]
        animation_frame = struct.unpack_from("<b", record, 96)[0]
        control_flags = struct.unpack_from("<I", record, 16)[0]
        key = (
            class_id,
            model,
            action,
            parameter_index,
            particle_index,
            animation_frame,
            control_flags,
        )
        grouped.setdefault(key, []).append(index)
    if len(grouped) != 17 or sum(len(indices) for indices in grouped.values()) != 177:
        raise CacheError("qualified Level 1 active reset grouping changed")

    groups: list[dict[str, Any]] = []
    actor_total = 0
    effect_total = 0
    suppressed_head_total = 0
    off_chain_total = 0
    for key, indices in sorted(grouped.items(), key=lambda item: item[1][0]):
        (
            class_id,
            model,
            action,
            parameter_index,
            particle_index,
            animation_frame,
            control_flags,
        ) = key
        if particle_index < 0 or particle_index >= len(particles):
            raise CacheError(f"entity group has invalid particle index {particle_index}")
        (
            base_selector,
            word_2,
            word_4,
            speed,
            rotation_speed,
            raster_table_selector_offset,
            signed_byte_11,
            render_mode,
            byte_13,
        ) = particles[particle_index]
        actor_spec = ENTITY_ACTOR_SPECS.get(
            (class_id, model, action, parameter_index, particle_index)
        )
        effect_spec = ENTITY_EFFECT_SPECS.get(
            (class_id, model, action, parameter_index, particle_index)
        )
        presentation_spec = actor_spec or effect_spec
        terrain_head_indices = [index for index in indices if index in terrain_heads]
        visible_indices = (
            terrain_head_indices
            if control_flags & 0x21 == 0 and presentation_spec is not None
            else []
        )
        suppressed_head_indices = (
            terrain_head_indices if control_flags & 0x21 else []
        )
        off_chain_indices = [index for index in indices if index not in terrain_heads]
        suppressed_head_total += len(suppressed_head_indices)
        off_chain_total += len(off_chain_indices)
        qualified_view_indices: list[int] = []
        texture_animated = False
        texture_frame_count = 0
        authorities = {
            (
                "PlayerFlightLive"
                if index == 152
                else "SpellActivationLive"
                if index == 153
                else "FrozenObserved"
            )
            for index in indices
        }
        if len(authorities) != 1:
            raise CacheError("entity presentation group mixes authority domains")
        authority = next(iter(authorities))
        if presentation_spec is not None:
            family, expected_base, expected_mode, expected_rotation = presentation_spec
            if (
                base_selector != expected_base
                or render_mode != expected_mode
                or rotation_speed != expected_rotation
            ):
                raise CacheError(f"qualified billboard presentation changed for {family}")
            if render_mode == 20:
                selectors = [
                    base_selector + value
                    for value in sorted(set(YAW_LOOKUP_D4750_28))
                ]
            elif 2 <= render_mode <= 16 or 22 <= render_mode <= 36:
                selectors = [base_selector + animation_frame]
            else:
                selectors = [base_selector]
            for selector in selectors:
                texture = texture_rows.get(selector)
                if (
                    texture is None
                    or texture.get("status") != "decoded_indexed_texture"
                    or not texture.get("indices_path")
                ):
                    raise CacheError(
                        f"qualified actor selector {selector} is not materialized"
                    )
            selected_textures = [texture_rows[selector] for selector in selectors]
            animation_shapes = {
                (row["animated"], row["frame_count"]) for row in selected_textures
            }
            if len(animation_shapes) != 1:
                raise CacheError(f"selector animation family disagrees for {family}")
            texture_animated, texture_frame_count = next(iter(animation_shapes))
            if actor_spec is not None:
                if len(visible_indices) != len(indices):
                    raise CacheError(f"qualified actor left terrain render chain: {family}")
                actor_total += len(visible_indices)
                status = "qualified_owner_actor_billboard"
                qualified_view_indices = visible_indices
            else:
                effect_total += len(visible_indices)
                status = (
                    "mapped_reset_capture_site_effect_hidden_qualified_first_present_diverged"
                )
                qualified_view_indices = []
            hidden_reason = (
                None
                if len(visible_indices) == len(indices)
                else "same_reset_group_contains_off_chain_non_drawn_records"
            )
        else:
            family = None
            selectors = []
            status = "debug_unresolved_hidden_in_qualified_view"
            if class_id == 3:
                hidden_reason = "live_player_is_presented_by_the_carpet_flight_rig"
            elif class_id == 10:
                hidden_reason = (
                    "transient_effect_or_command_record_lifetime_not_live_in_frozen_snapshot"
                )
            elif class_id == 11:
                hidden_reason = "switch_trigger_or_checkpoint_control_record"
            elif class_id == 15:
                hidden_reason = "spell_holder_or_inventory_control_record"
            else:
                hidden_reason = "semantic_and_visibility_role_unresolved"
        groups.append(
            {
                "group_id": f"reset-{indices[0]:03d}",
                "entity_indices": indices,
                "entity_count": len(indices),
                "reset_chain_drawable_entity_indices": visible_indices,
                "reset_chain_drawable_count": len(visible_indices),
                "qualified_view_entity_indices": qualified_view_indices,
                "qualified_view_count": len(qualified_view_indices),
                "suppressed_terrain_head_entity_indices": suppressed_head_indices,
                "off_chain_entity_indices": off_chain_indices,
                "authority": authority,
                "key": {
                    "class_id": class_id,
                    "model": model,
                    "action": action,
                    "parameter_record_index": parameter_index,
                    "particle_parameter_index": particle_index,
                    "animation_frame": animation_frame,
                    "control_flags_u32": control_flags,
                },
                "semantic_family": family,
                "qualified_status": status,
                "qualified_hidden_reason": hidden_reason,
                "particle": {
                    "base_tmap_selector": base_selector,
                    "word_2": word_2,
                    "word_4": word_4,
                    "speed": speed,
                    "logical_height_source_units": rotation_speed,
                    "raw_raster_table_selector_offset": raster_table_selector_offset,
                    "signed_byte_11": signed_byte_11,
                    "render_mode": render_mode,
                    "byte_13": byte_13,
                },
                "selector": {
                    "bank": "TMAPS1-0",
                    "qualified_selectors": selectors,
                    "expression": (
                        "base_tmap_selector"
                        if render_mode in (0, 1, 21) and presentation_spec is not None
                        else (
                            "base_tmap_selector + D4750_28[yaw_relative_orientation_0_15]"
                            if render_mode == 20 and presentation_spec is not None
                            else (
                                "base_tmap_selector + animation_frame"
                                if presentation_spec is not None and
                                (2 <= render_mode <= 16 or 22 <= render_mode <= 36)
                                else "unresolved_in_qualified_view"
                            )
                        )
                    ),
                    "yaw_lookup_0_15": (
                        list(YAW_LOOKUP_D4750_28)
                        if render_mode == 20 and presentation_spec is not None
                        else []
                    ),
                    "mirror_when_orientation_gte_8": (
                        render_mode == 20 and presentation_spec is not None
                    ),
                    "yaw_sector_expression": (
                        "(((entity_yaw-camera_yaw)>>3)&0xF0)>>4"
                        if render_mode == 20 and presentation_spec is not None
                        else None
                    ),
                },
                "palette": {
                    "asset_id": "owner_data:DATA/PALN-0.DAT",
                    "path": "palette/paln.rgb8",
                    "binding": "exact_night_base_palette",
                },
                "billboard": {
                    "facing": "full_camera_facing_screen_plane",
                    "world_height": "logical_height_source_units / 64",
                    "world_width": "world_height * selected_tmap_width / selected_tmap_height",
                    "anchor": (
                        "viewer_bottom_center_at_entity_xyz_approximation"
                        if presentation_spec is not None
                        else "unresolved_in_qualified_view"
                    ),
                    "source_anchor_status": (
                        "deferred: original DrawSprite_41BD3 a1 offset depends on "
                        "(realHeight+realWidth)>>2 and camera sin/cos"
                        if presentation_spec is not None
                        else "unresolved_in_qualified_view"
                    ),
                    "hotspot": "no_per_sprite_hotspot_in_type_particle_str",
                    "depth": (
                        "viewer_world_depth_approximation_only; original terrain-cell "
                        "traversal, distance/fog gates, and software clipping are deferred"
                    ),
                    "compositing": (
                        "indexed_zero_transparent_standard_path"
                        if render_mode in (0, 20)
                        else (
                            "x_BYTE_F2CC6_alternate_transparency_path_approximated_by_index_zero_cutout"
                            if render_mode in (21, 36)
                            else "unresolved_in_qualified_view"
                        )
                    ),
                    "texture_animation": {
                        "animated": texture_animated,
                        "frame_count": texture_frame_count,
                        "scope": (
                            "selector_global_shared_TMAP_state_not_per_entity"
                            if texture_animated
                            else "static"
                            if presentation_spec is not None
                            else "unresolved_in_qualified_view"
                        ),
                        "cache_payload": (
                            "complete decoded base plus delta stream retained per selector"
                            if texture_animated
                            else "complete static decoded payload retained"
                        ),
                        "qualified_view": (
                            "base_frame_only; global used-TMAP update timing deferred"
                            if texture_animated
                            else "static_base_frame"
                            if presentation_spec is not None
                            else "unresolved_in_qualified_view"
                        ),
                    },
                },
            }
        )

    observed_counts = {
        row["semantic_family"]: row["entity_count"]
        for row in groups
        if row["qualified_status"] == "qualified_owner_actor_billboard"
    }
    if observed_counts != EXPECTED_ACTOR_COUNTS or actor_total != 64:
        raise CacheError("qualified Level 1 actor roster changed")
    if (
        effect_total != 19
        or suppressed_head_total != 47
        or off_chain_total != 47
        or actor_total + effect_total != 83
    ):
        raise CacheError("qualified Level 1 renderer disposition changed")
    authority_counts = {
        authority: sum(
            row["entity_count"] for row in groups if row["authority"] == authority
        )
        for authority in ("FrozenObserved", "PlayerFlightLive", "SpellActivationLive")
    }
    if authority_counts != {
        "FrozenObserved": 175,
        "PlayerFlightLive": 1,
        "SpellActivationLive": 1,
    } or any(
        row["authority"] != "FrozenObserved"
        for row in groups
        if row["reset_chain_drawable_count"]
    ):
        raise CacheError("qualified Level 1 presentation authority split changed")
    return {
        "schema": "mc2-level1-entity-presentations/v1",
        "authority_contract": {
            "reset_rows": {
                "FrozenObserved": 175,
                "PlayerFlightLive": 1,
                "SpellActivationLive": 1,
            },
            "qualified_view_rows": "64 stable FrozenObserved actors",
            "reset_chain_drawable_rows": "83 FrozenObserved rows",
            "transient_effect_boundary":
                "19 rows are exact exporter capture-site/reset-chain presentation, "
                "not the later first-presented-frame state after original update passes",
            "runtime_updates": "only ABI live-authority rows may mutate",
            "qualified_view":
                "64 stable owner actor billboards; no transient, suppressed, or off-chain proxies",
            "debug_view":
                "all other active reset rows are explicit unresolved diagnostics",
        },
        "source_contract": {
            "entity_record": "MCE1/v1 176-byte records",
            "presentation_record": "MCV1/v1 particle table",
            "selector_bank": "owner_data:DATA/TMAPS1-0.DAT",
            "palette": "owner_data:DATA/PALN-0.DAT",
            "msprn_status":
                "not_used_for_world_entities; closure MSPRN rows are HUD/bitmap path",
        },
        "qualified_actor_count": actor_total,
        "mapped_reset_effect_count": effect_total,
        "reset_chain_drawable_count": actor_total + effect_total,
        "qualified_view_count": actor_total,
        "suppressed_terrain_head_count": suppressed_head_total,
        "off_chain_active_count": off_chain_total,
        "qualified_group_count": len(EXPECTED_ACTOR_COUNTS),
        "active_reset_count": 177,
        "active_group_count": len(groups),
        "groups": groups,
    }


def compile_rnc_decoder(directory: Path) -> tuple[Path, dict[str, Any]]:
    executable = directory / "mc-rnc-decode"
    command = [
        "g++",
        "-std=c++17",
        "-O2",
        "-I",
        str(RNC_DECODER_HEADER.parent),
        str(RNC_HELPER),
        str(RNC_DECODER_CPP),
        "-o",
        str(executable),
    ]
    try:
        completed = subprocess.run(
            command, check=True, capture_output=True, text=True
        )
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", "") or str(error)
        raise CacheError(f"cannot build proven RNC decoder: {detail.strip()}") from error
    if completed.stderr:
        raise CacheError(f"RNC decoder compiler emitted diagnostics: {completed.stderr.strip()}")
    return executable, rnc_logical_identity()


def rnc_logical_identity(
    non_identity_compiler_metadata: str | None = None,
) -> dict[str, Any]:
    # The compiler executable is only a vehicle for the already pinned source.
    # Host version strings must not perturb the cache's logical identity.
    del non_identity_compiler_metadata
    return {
        "id": "remc2-DataFileRNC-owner-cache-adapter",
        "compile_flags": ["-std=c++17", "-O2"],
        "toolchain_identity_contract":
            "host compiler metadata is non-identity-bearing; adapter/implementation/"
            "header and every decoded output hash define logical cache truth",
        "adapter_sha256": sha256_file(RNC_HELPER),
        "implementation_sha256": sha256_file(RNC_DECODER_CPP),
        "header_sha256": sha256_file(RNC_DECODER_HEADER),
    }


def parse_source_uv_rows() -> list[list[list[int]]]:
    values = [
        int(value, 16)
        for value in re.findall(r"0x[0-9A-Fa-f]+", REMC2_UV_TABLE.read_text())
    ]
    if len(values) != 32 * 8 or any(value not in (0, 0x001FFFFF) for value in values):
        raise CacheError("remc2 UVTable_D4350 no longer has the qualified 32x8 binary form")
    rows: list[list[list[int]]] = []
    for row in range(32):
        values8 = values[row * 8:(row + 1) * 8]
        rows.append([
            [values8[index] // 0x001FFFFF, values8[index + 1] // 0x001FFFFF]
            for index in range(0, 8, 2)
        ])
    return rows


def decode_tmap_rnc(
    decoder: Path, packed: bytes, directory: Path, index: int
) -> bytes:
    source = directory / f"tmap-{index:03d}.rnc"
    target = directory / f"tmap-{index:03d}.decoded"
    source.write_bytes(packed)
    try:
        subprocess.run(
            [str(decoder), str(source), str(target)],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", "") or str(error)
        raise CacheError(f"TMAPS RNC decode failed at index {index}: {detail.strip()}") from error
    return target.read_bytes()


def decode_container_blob(
    decoder: Path,
    container: bytes,
    offset: int,
    unpacked_size: int,
    directory: Path,
    label: str,
) -> bytes:
    if offset < 0 or unpacked_size < 0 or offset >= len(container):
        raise CacheError(f"{label} descriptor is outside its container")
    if container[offset:offset + 3] != b"RNC":
        end = offset + unpacked_size
        if end > len(container):
            raise CacheError(f"{label} raw payload is truncated")
        return container[offset:end]
    if offset + 18 > len(container):
        raise CacheError(f"{label} RNC header is truncated")
    declared_unpacked = struct.unpack_from(">I", container, offset + 4)[0]
    packed_size = struct.unpack_from(">I", container, offset + 8)[0]
    if declared_unpacked != unpacked_size:
        raise CacheError(f"{label} RNC unpacked-size mismatch")
    end = offset + 18 + packed_size
    if end > len(container):
        raise CacheError(f"{label} RNC payload is truncated")
    source = directory / f"{label}.rnc"
    target = directory / f"{label}.decoded"
    source.write_bytes(container[offset:end])
    try:
        subprocess.run(
            [str(decoder), str(source), str(target)],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", "") or str(error)
        raise CacheError(f"{label} RNC decode failed: {detail.strip()}") from error
    decoded = target.read_bytes()
    if len(decoded) != unpacked_size:
        raise CacheError(f"{label} decoded extent changed")
    return decoded


def container_descriptor(
    container: bytes,
    *,
    count_rows: int,
    selector: int,
    row: int,
    label: str,
) -> tuple[list[int], dict[str, int]]:
    if len(container) < 4:
        raise CacheError(f"{label} container is truncated")
    trailer = struct.unpack_from("<i", container, len(container) - 4)[0]
    count_bytes = count_rows * 2
    record_bytes = count_rows * 16
    if trailer < 0 or trailer + count_bytes > len(container) - 4:
        raise CacheError(f"{label} trailer pointer is invalid")
    counts = list(struct.unpack_from(f"<{count_rows}h", container, trailer))
    if selector < 0 or selector >= counts[row]:
        raise CacheError(f"{label} selector is absent from descriptor row")
    record_offset = trailer + count_bytes + selector * record_bytes + row * 16
    if record_offset + 16 > len(container) - 4:
        raise CacheError(f"{label} descriptor record is truncated")
    metadata_offset, data_offset, metadata_size, data_size = struct.unpack_from(
        "<4i", container, record_offset
    )
    descriptor = {
        "container_trailer_offset": trailer,
        "descriptor_offset": record_offset,
        "metadata_offset": metadata_offset,
        "data_offset": data_offset,
        "metadata_size": metadata_size,
        "data_size": data_size,
    }
    for key, value in descriptor.items():
        if value < 0:
            raise CacheError(f"{label} descriptor has negative {key}")
    return counts, descriptor


def packed_audio_records(
    metadata: bytes,
    data: bytes,
    *,
    expected_count: int,
    label: str,
) -> list[dict[str, Any]]:
    expected_size = 32 + expected_count * 32
    if len(metadata) != expected_size:
        raise CacheError(f"{label} metadata extent changed")
    records = []
    for index in range(expected_count):
        record = metadata[32 + index * 32:64 + index * 32]
        filename_bytes = record[:18].split(b"\0", 1)[0]
        try:
            filename = filename_bytes.decode("ascii")
        except UnicodeDecodeError as error:
            raise CacheError(f"{label} filename is not ASCII") from error
        relative_offset = struct.unpack_from("<i", record, 18)[0]
        unknown = record[22:26]
        size, word = struct.unpack_from("<iH", record, 26)
        if (
            not filename
            or size < 0
            or relative_offset + size > len(data)
            or unknown != b"\0\0\0\0"
        ):
            raise CacheError(f"{label} record {index} is malformed or overlaps")
        records.append({
            "slot": index,
            "filename": filename,
            "relative_data_offset": relative_offset,
            "bytes": size,
            "word": word,
            "payload_sha256": sha256_bytes(
                data[relative_offset:relative_offset + size]
            ),
        })
    unique_spans = sorted({
        (row["relative_data_offset"], row["relative_data_offset"] + row["bytes"])
        for row in records
    })
    for previous, current in zip(unique_spans, unique_spans[1:]):
        if current[0] < previous[1]:
            raise CacheError(f"{label} contains overlapping non-identical records")
    return records


def wav_properties(payload: bytes, label: str) -> dict[str, Any]:
    if payload[:4] != b"RIFF" or payload[8:12] != b"WAVE":
        raise CacheError(f"{label} is not a RIFF/WAVE payload")
    declared = struct.unpack_from("<I", payload, 4)[0] + 8
    if declared > len(payload):
        raise CacheError(f"{label} RIFF extent exceeds its record")
    try:
        with wave.open(BytesIO(payload), "rb") as stream:
            channels = stream.getnchannels()
            sample_width = stream.getsampwidth()
            sample_rate = stream.getframerate()
            frame_count = stream.getnframes()
            compression = stream.getcomptype()
    except (EOFError, wave.Error) as error:
        raise CacheError(f"{label} WAVE parse failed: {error}") from error
    if compression != "NONE" or channels <= 0 or sample_width not in (1, 2):
        raise CacheError(f"{label} unsupported WAVE format")
    return {
        "riff_declared_bytes": declared,
        "channels": channels,
        "sample_width_bytes": sample_width,
        "sample_rate": sample_rate,
        "frame_count": frame_count,
        "duration_numerator_frames": frame_count,
        "duration_denominator_hz": sample_rate,
    }


def midi_dynamic_control_census(payload: bytes, label: str) -> dict[str, Any]:
    """Return the bounded XMIDI control events relevant to MC2 music dynamics."""
    if payload[:4] != b"MThd" or len(payload) < 14:
        raise CacheError(f"{label} is not standard MIDI")
    header_size = struct.unpack_from(">I", payload, 4)[0]
    if header_size < 6 or 8 + header_size > len(payload):
        raise CacheError(f"{label} MIDI header extent is invalid")
    midi_format, track_count, division = struct.unpack_from(">HHH", payload, 8)
    if midi_format != 0 or track_count != 1 or division == 0:
        raise CacheError(f"{label} MIDI format/track/division changed")

    def read_vlq(data: bytes, offset: int, extent: int) -> tuple[int, int]:
        value = 0
        for _ in range(4):
            if offset >= extent:
                raise CacheError(f"{label} truncated MIDI VLQ")
            byte = data[offset]
            offset += 1
            value = (value << 7) | (byte & 0x7F)
            if byte < 0x80:
                return value, offset
        raise CacheError(f"{label} MIDI VLQ exceeds four bytes")

    offset = 8 + header_size
    if payload[offset:offset + 4] != b"MTrk" or offset + 8 > len(payload):
        raise CacheError(f"{label} MIDI track chunk is missing")
    track_size = struct.unpack_from(">I", payload, offset + 4)[0]
    offset += 8
    track_end = offset + track_size
    if track_end > len(payload):
        raise CacheError(f"{label} MIDI track extent is invalid")

    tick = 0
    running_status: int | None = None
    relevant: list[dict[str, int]] = []
    saw_end = False
    while offset < track_end:
        delta, offset = read_vlq(payload, offset, track_end)
        tick += delta
        if offset >= track_end:
            raise CacheError(f"{label} MIDI event is truncated")
        status = payload[offset]
        if status < 0x80:
            if running_status is None:
                raise CacheError(f"{label} MIDI running status has no predecessor")
            status = running_status
        else:
            offset += 1
            if status < 0xF0:
                running_status = status

        if status == 0xFF:
            if offset >= track_end:
                raise CacheError(f"{label} MIDI meta event is truncated")
            meta_type = payload[offset]
            offset += 1
            size, offset = read_vlq(payload, offset, track_end)
            if offset + size > track_end:
                raise CacheError(f"{label} MIDI meta extent is invalid")
            offset += size
            if meta_type == 0x2F:
                saw_end = True
                break
            continue
        if status in (0xF0, 0xF7):
            size, offset = read_vlq(payload, offset, track_end)
            if offset + size > track_end:
                raise CacheError(f"{label} MIDI SysEx extent is invalid")
            offset += size
            running_status = None
            continue
        if status >= 0xF0:
            raise CacheError(f"{label} unsupported MIDI system event")

        event_type = status & 0xF0
        channel = status & 0x0F
        data_size = 1 if event_type in (0xC0, 0xD0) else 2
        if offset + data_size > track_end:
            raise CacheError(f"{label} MIDI channel event is truncated")
        first = payload[offset]
        second = payload[offset + 1] if data_size == 2 else 0
        offset += data_size
        if event_type == 0xB0 and first in (116, 117, 119):
            relevant.append({
                "tick": tick,
                "channel": channel,
                "controller": first,
                "value": second,
            })
    if not saw_end:
        raise CacheError(f"{label} MIDI track has no end marker")

    trigger_events = [
        event for event in relevant if event["controller"] == 119
    ]
    return {
        "midi_format": midi_format,
        "track_count": track_count,
        "division": division,
        "trigger_controller": 119,
        "trigger_events": trigger_events,
        "tagged_channels": sorted({
            event["channel"]
            for event in trigger_events
            if event["value"] == 0
        }),
        "loop_controller_events": [
            event for event in relevant if event["controller"] in (116, 117)
        ],
    }


def compile_xmi_converter(directory: Path) -> tuple[Path, dict[str, Any]]:
    executable = directory / "mc-xmi-to-midi"
    command = [
        os.environ.get("CXX", "c++"),
        "-std=c++17",
        "-O2",
        str(XMI_HELPER),
        str(XMI_IMPLEMENTATION),
        "-o",
        str(executable),
    ]
    try:
        completed = subprocess.run(
            command, check=True, capture_output=True, text=True
        )
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", "") or str(error)
        raise CacheError(f"cannot build XMI converter: {detail.strip()}") from error
    if completed.stderr:
        raise CacheError(f"XMI converter compiler emitted diagnostics: {completed.stderr}")
    return executable, {
        "id": "remc2-TranscodeXmiToMid-owner-cache-adapter",
        "adapter_sha256": sha256_file(XMI_HELPER),
        "implementation_sha256": sha256_file(XMI_IMPLEMENTATION),
        "header_sha256": sha256_file(XMI_HEADER),
        "compile_flags": ["-std=c++17", "-O2"],
        "host_compiler_metadata_identity_bearing": False,
    }


def transcode_xmi(
    executable: Path, payload: bytes, directory: Path, label: str
) -> bytes:
    source = directory / f"{label}.xmi"
    target = directory / f"{label}.mid"
    source.write_bytes(payload)
    try:
        subprocess.run(
            [str(executable), str(source), str(target)],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", "") or str(error)
        raise CacheError(f"{label} XMI conversion failed: {detail.strip()}") from error
    midi = target.read_bytes()
    if midi[:4] != b"MThd":
        raise CacheError(f"{label} converter did not produce standard MIDI")
    return midi


def compile_adl_renderer() -> tuple[Path, dict[str, Any]]:
    """Build a private deterministic MIDI-to-WAVE adapter outside the cache."""
    source_revision = subprocess.run(
        ["git", "-C", str(ADLMIDI_SOURCE), "rev-parse", "HEAD"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    if source_revision != "6806c7204d17220ecc0430759f784aea8ee2ddb2":
        raise CacheError("qualified remc2/libADLMIDI source revision changed")
    identity = sha256_bytes(
        canonical_json({
            "source_revision": source_revision,
            "adapter_sha256": sha256_file(ADL_RENDER_HELPER),
            "bank": 59,
            "chips": 2,
            "sample_rate": 44_100,
        })
    )[:20]
    build = Path(tempfile.gettempdir()) / f"mc-owner-adlmidi-{identity}"
    library = build / "libADLMIDI.a"
    executable = build / "mc-adl-render"
    if not library.is_file():
        build.mkdir(parents=True, exist_ok=True)
        configure = [
            "cmake",
            "-S", str(ADLMIDI_SOURCE),
            "-B", str(build),
            "-DCMAKE_BUILD_TYPE=Release",
            "-DADLMIDI_STATIC=ON",
            "-DWITH_MIDIPLAY=OFF",
            "-DWITH_GENADLDATA=OFF",
            "-DWITH_UNIT_TESTS=OFF",
            "-DWITH_VLC_PLUGIN=OFF",
        ]
        try:
            subprocess.run(
                configure, check=True, capture_output=True, text=True, timeout=180
            )
            subprocess.run(
                ["cmake", "--build", str(build), "--target", "ADLMIDI_static",
                 "--parallel", "2"],
                check=True,
                capture_output=True,
                text=True,
                timeout=300,
            )
        except (OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
            detail = getattr(error, "stderr", "") or str(error)
            raise CacheError(
                f"cannot build qualified private libADLMIDI: {detail.strip()}"
            ) from error
    if not executable.is_file():
        command = [
            os.environ.get("CXX", "c++"),
            "-std=c++17",
            "-O2",
            "-I", str(ADLMIDI_SOURCE / "include"),
            str(ADL_RENDER_HELPER),
            str(library),
            "-ldl", "-lpthread", "-lm",
            "-o", str(executable),
        ]
        try:
            subprocess.run(
                command, check=True, capture_output=True, text=True, timeout=180
            )
        except (OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
            detail = getattr(error, "stderr", "") or str(error)
            raise CacheError(f"cannot build ADLMIDI render adapter: {detail.strip()}") from error
    return executable, {
        "id": "libADLMIDI-private-owner-cache-render-adapter",
        "source_revision": source_revision,
        "source_license": "LGPL-2.1-or-later/GPL-3.0-or-later upstream dual license",
        "adapter_sha256": sha256_file(ADL_RENDER_HELPER),
        "compile_flags": ["-std=c++17", "-O2"],
        "bank": 59,
        "chips": 2,
        "sample_rate": 44_100,
        "host_tool_and_library_embedded_in_Unity": False,
    }


def render_midi_wave(
    executable: Path, midi: bytes, directory: Path, label: str
) -> bytes:
    source = directory / f"{label}-render.mid"
    target = directory / f"{label}-render.wav"
    source.write_bytes(midi)
    try:
        subprocess.run(
            [str(executable), str(source), str(target)],
            check=True,
            capture_output=True,
            text=True,
            timeout=180,
        )
    except (OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        detail = getattr(error, "stderr", "") or str(error)
        raise CacheError(f"{label} deterministic WAVE render failed: {detail.strip()}") from error
    payload = target.read_bytes()
    properties = wav_properties(payload, f"{label} rendered music")
    if (
        len(payload) != 77_889_580
        or sha256_bytes(payload) !=
            "03b3f951048504d8a6d30611d6c44a62ec1c463726b30c33c16e3cb69824605d"
        or properties != {
            "riff_declared_bytes": 77_889_580,
            "channels": 2,
            "sample_width_bytes": 2,
            "sample_rate": 44_100,
            "frame_count": 19_472_384,
            "duration_numerator_frames": 19_472_384,
            "duration_denominator_hz": 44_100,
        }
    ):
        raise CacheError("qualified C2GAME1 deterministic WAVE render changed")
    return payload


def write_output(
    stage: Path,
    outputs: list[dict[str, Any]],
    relative: str,
    data: bytes,
    *,
    kind: str,
    source_asset_ids: list[str],
    extra: dict[str, Any] | None = None,
) -> dict[str, Any]:
    target = stage / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(data)
    row: dict[str, Any] = {
        "path": relative,
        "kind": kind,
        "bytes": len(data),
        "sha256": sha256_bytes(data),
        "source_asset_ids": source_asset_ids,
    }
    if extra:
        row.update(extra)
    outputs.append(row)
    return row


def fsync_tree(root: Path) -> None:
    for path in sorted(item for item in root.rglob("*") if item.is_file()):
        with path.open("rb") as stream:
            os.fsync(stream.fileno())
    for path in sorted(
        (item for item in root.rglob("*") if item.is_dir()),
        key=lambda item: len(item.parts),
        reverse=True,
    ):
        descriptor = os.open(path, os.O_RDONLY)
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    descriptor = os.open(root, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def fsync_parent(path: Path) -> None:
    descriptor = os.open(path.parent, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def commit_stage(output: Path, stage: Path, failpoint: str | None = None) -> None:
    backup = output.with_name(output.name + ".previous")
    if not output.exists() and backup.exists():
        os.replace(backup, output)
        fsync_parent(output)
    if backup.exists():
        shutil.rmtree(backup)
        fsync_parent(output)
    if output.exists():
        os.replace(output, backup)
        fsync_parent(output)
    if failpoint == "after_backup":
        os.replace(backup, output)
        fsync_parent(output)
        raise CacheError("injected failure after backup rename")
    try:
        os.replace(stage, output)
        fsync_parent(output)
        if failpoint == "after_install":
            raise CacheError("injected failure after install rename")
    except Exception:
        if output.exists():
            shutil.rmtree(output)
        if backup.exists():
            os.replace(backup, output)
            fsync_parent(output)
        raise
    if backup.exists():
        shutil.rmtree(backup)
        fsync_parent(output)


def build_cache(
    owner_root: Path,
    package_dir: Path,
    closure_path: Path,
    catalog_path: Path,
    output: Path,
) -> dict[str, Any]:
    owner_root = owner_root.expanduser().resolve()
    package_dir = package_dir.expanduser().resolve()
    closure_path = closure_path.expanduser().resolve()
    catalog_path = catalog_path.expanduser().resolve()
    output = validate_output_path(output)

    package = load_json(package_dir / "manifest.json")
    closure = load_json(closure_path)
    catalog = load_json(catalog_path)
    try:
        load_proven_export_verifier()(package_dir, package)
    except ValueError as error:
        raise CacheError(f"package identity verification failed: {error}") from error
    if package.get("package_sha256") != EXPECTED_PACKAGE:
        raise CacheError("wrong Level 1 package hash")
    terrain_export = (package_dir / "terrain.bin").read_bytes()
    if len(terrain_export) < 65536:
        raise CacheError("Level 1 terrain export is truncated")
    level1_terrain_selectors = sorted(set(terrain_export[:65536]))
    if level1_terrain_selectors != list(LEVEL1_TERRAIN_SELECTORS):
        raise CacheError("oracle-matched Level 1 terrain selector set changed")
    closure_claim = closure.get("closure_sha256")
    closure_payload = dict(closure)
    closure_payload.pop("closure_sha256", None)
    if sha256_bytes(
        json.dumps(
            closure_payload, sort_keys=True, separators=(",", ":"), ensure_ascii=True
        ).encode("utf-8")
    ) != closure_claim:
        raise CacheError("closure canonical self-hash mismatch")
    if closure.get("closure_sha256") != EXPECTED_CLOSURE:
        raise CacheError("wrong Level 1 closure hash")
    closure_package = closure["source"]["level_export"]["package_sha256"]
    if closure_package != EXPECTED_PACKAGE:
        raise CacheError("closure is not bound to the exact package")
    catalog_binding = closure["source"]["owner_catalog"]["catalog_sha256"]
    catalog_claim = catalog.get("catalog_sha256")
    catalog_payload = dict(catalog)
    catalog_payload.pop("catalog_sha256", None)
    catalog_actual = sha256_bytes(
        json.dumps(
            catalog_payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False
        ).encode("utf-8")
    )
    if catalog_claim != catalog_actual or catalog_claim != catalog_binding:
        raise CacheError("owner catalog does not match closure binding")
    if sha256_file(catalog_path) != closure["source"]["owner_catalog"]["file_sha256"]:
        raise CacheError("owner catalog file hash does not match closure binding")

    file_rows = {row["path"]: row for row in catalog["files"]}
    entry_rows = {row["asset_id"]: row for row in catalog["entries"]}
    source_files: list[dict[str, Any]] = []
    source_bytes: dict[str, bytes] = {}
    for relative in SOURCE_PATHS:
        path = confined_source(owner_root, relative)
        data = path.read_bytes()
        row = file_rows.get(relative)
        if row is None or row.get("sha256") != sha256_bytes(data):
            raise CacheError(f"owner source hash differs from qualified catalog: {relative}")
        source_bytes[relative] = data
        source_files.append(
            {
                "asset_id": row["asset_id"],
                "path": relative,
                "bytes": len(data),
                "sha256": row["sha256"],
            }
        )

    frame_ids = {
        int(asset_id.rsplit("#", 1)[1])
        for event in closure["sets"]["frame_used"]["assets"]
        for asset_id in event.get("owner_asset_ids", [])
        if asset_id.startswith("owner_data:DATA/MSPRN0-0.DAT#")
    }
    if frame_ids != set(FRAME_MSPRN):
        raise CacheError("closure no longer contains the qualified 11 MSPRN frame indices")
    dynamic_ids = {
        int(row["asset_id"].rsplit("#", 1)[1])
        for row in closure["sets"]["dynamic_dependencies"]["assets"]
        if row["asset_id"].startswith("owner_data:DATA/MSPRN0-0.DAT#")
    }
    if not set(DYNAMIC_MSPRN) <= dynamic_ids:
        raise CacheError("qualified dynamic MSPRN dependency is missing")

    frame_tmaps = {
        int(asset_id.rsplit("#", 1)[1])
        for event in closure["sets"]["frame_used"]["assets"]
        for asset_id in event.get("owner_asset_ids", [])
        if asset_id.startswith("owner_data:DATA/TMAPS1-0.DAT#")
    }
    if frame_tmaps != set(FRAME_TMAPS):
        raise CacheError("closure no longer contains the qualified four TMAP selectors")
    scene_tmaps = {
        int(row["asset_id"].rsplit("#", 1)[1])
        for row in closure["sets"]["scene_reachable"]["assets"]
        if row["asset_id"].startswith("owner_data:DATA/TMAPS1-0.DAT#")
    }
    if not frame_tmaps <= scene_tmaps or len(scene_tmaps) != 129:
        raise CacheError("qualified Level 1 scene TMAP closure changed")

    stage = Path(tempfile.mkdtemp(prefix=f".{output.name}.", dir=output.parent))
    outputs: list[dict[str, Any]] = []
    try:
        rnc_work = stage / ".rnc-work"
        rnc_work.mkdir()
        decoder, rnc_provenance = compile_rnc_decoder(rnc_work)
        sound_container = source_bytes["SOUND/SOUND.DAT"]
        music_container = source_bytes["SOUND/MUSIC.DAT"]
        if (
            sha256_bytes(sound_container) !=
                "dba5958ae8f3aae53f3b9e3fa792bfef75a020b185e7884e344c7238ff2ee2a2"
            or sha256_bytes(music_container) !=
                "3549b18c75b1812618b775e8b76f1ead312acfe4d6f5c8029d8cb696a70dbeb5"
        ):
            raise CacheError("qualified owner audio container hash changed")

        sound_counts, sound_descriptor_raw = container_descriptor(
            sound_container,
            count_rows=6,
            selector=1,
            row=3,
            label="sound-bank-1-quality-822",
        )
        if (
            sound_counts != [0, 0, 0, 10, 10, 0]
            or sound_descriptor_raw != {
                "container_trailer_offset": 14864080,
                "descriptor_offset": 14864236,
                "metadata_offset": 1988320,
                "data_offset": 0,
                "metadata_size": 2080,
                "data_size": 1988320,
            }
        ):
            raise CacheError("qualified Night/822 SOUND descriptor changed")
        sound_metadata = decode_container_blob(
            decoder,
            sound_container,
            sound_descriptor_raw["metadata_offset"],
            sound_descriptor_raw["metadata_size"],
            rnc_work,
            "sound-bank-1-822-metadata",
        )
        sound_data = decode_container_blob(
            decoder,
            sound_container,
            sound_descriptor_raw["data_offset"],
            sound_descriptor_raw["data_size"],
            rnc_work,
            "sound-bank-1-822-data",
        )
        sound_records = packed_audio_records(
            sound_metadata,
            sound_data,
            expected_count=64,
            label="sound-bank-1-822",
        )
        sound_semantics = {
            1: ("ocean", "loop", 70, "local_ambient"),
            2: ("cricket2", "loop", 70, "local_ambient"),
            5: ("fire", "loop", 120, "local_ambient"),
            12: ("null", "explicit_null", 0, "explicit_null"),
            29: ("cantuse", "one_shot", 127, "local_feedback"),
            31: ("market", "loop", 85, "local_ambient"),
            37: ("cymerian", "one_shot", 127, "original_stereo_positional"),
            39: ("wyvern", "one_shot", 127, "original_stereo_positional"),
            43: ("firefly1", "one_shot", 127, "original_stereo_positional"),
            46: (
                "goat",
                "one_shot",
                127,
                "typed_goat_positional_request_pitch_deferred",
            ),
        }
        sound_assets = []
        for sound_id in LEVEL1_SOUND_IDS:
            record = sound_records[sound_id - 1]
            (
                semantic_name,
                play_class,
                target_volume,
                playback_scope,
            ) = sound_semantics[sound_id]
            payload = sound_data[
                record["relative_data_offset"]:
                record["relative_data_offset"] + record["bytes"]
            ]
            properties = wav_properties(payload, f"sound ID {sound_id}")
            if record["word"] != 90:
                raise CacheError(f"sound ID {sound_id} metadata word changed")
            asset_path = None
            asset_sha256 = None
            if sound_id in LEVEL1_MATERIALIZED_SOUND_IDS:
                output_row = write_output(
                    stage,
                    outputs,
                    f"audio/sfx/{sound_id:03d}-{semantic_name}.wav",
                    payload,
                    kind="owner_RIFF_WAVE_exact_record",
                    source_asset_ids=["owner_data:SOUND/SOUND.DAT"],
                    extra={
                        "bank": 1,
                        "quality": 822,
                        "quality_index": 3,
                        "sound_id": sound_id,
                        **properties,
                    },
                )
                asset_path = output_row["path"]
                asset_sha256 = output_row["sha256"]
            sound_assets.append({
                "bank": 1,
                "quality": 822,
                "quality_index": 3,
                "sound_id": sound_id,
                "metadata_slot": sound_id - 1,
                "filename": record["filename"],
                "relative_data_offset": record["relative_data_offset"],
                "record_bytes": record["bytes"],
                "record_sha256": record["payload_sha256"],
                "metadata_word": record["word"],
                "semantic_name": semantic_name,
                "play_class": play_class,
                "playback_scope": playback_scope,
                "loop_target_volume": target_volume,
                "asset_path": asset_path,
                "asset_sha256": asset_sha256,
                "mapping_status":
                    "explicit_null" if sound_id == 12 else "proven_Level1",
                "wav": properties,
            })
        expected_sound_names = {
            1: "OCEAN.WAV",
            2: "CRICKET2.WAV",
            5: "FIRE.WAV",
            12: "NULL.WAV",
            29: "CANTUSE.WAV",
            31: "MARKET.WAV",
            37: "CYMERIAN.WAV",
            39: "WYVERN.WAV",
            43: "FIREFLY1.WAV",
            46: "GOAT.WAV",
        }
        if {
            row["sound_id"]: row["filename"] for row in sound_assets
        } != expected_sound_names:
            raise CacheError("qualified Level 1 sound filenames changed")
        sound_descriptor = {
            "schema": "mc2-level1-sound-bank/v1",
            "source_asset_id": "owner_data:SOUND/SOUND.DAT",
            "source_sha256": sha256_bytes(sound_container),
            "bank": 1,
            "realm": "Night",
            "quality": 822,
            "quality_index": 3,
            "quality_rows": list(SOUND_QUALITY_ROWS),
            "bank_counts_by_quality_index": sound_counts,
            "container": sound_descriptor_raw,
            "runtime_indexing": "one_based_ID_with_inserted_null_slot_zero",
            "metadata_record_count": len(sound_records),
            "metadata_records": sound_records,
            "level1_assets": sound_assets,
            "materialized_sound_ids": list(LEVEL1_MATERIALIZED_SOUND_IDS),
            "explicit_null_sound_ids": [12],
            "other_rows_status": "Deferred",
        }
        sound_descriptor_output = write_output(
            stage,
            outputs,
            "audio/sound-bank-1-822.json",
            canonical_json(sound_descriptor),
            kind="canonical_json_Level1_sound_bank_descriptor",
            source_asset_ids=["owner_data:SOUND/SOUND.DAT"],
        )

        music_counts, music_descriptor_raw = container_descriptor(
            music_container,
            count_rows=4,
            selector=0,
            row=2,
            label="music-channel-0-driver-f",
        )
        if (
            music_counts != [2, 2, 2, 2]
            or music_descriptor_raw != {
                "container_trailer_offset": 643424,
                "descriptor_offset": 643464,
                "metadata_offset": 80656,
                "data_offset": 0,
                "metadata_size": 224,
                "data_size": 80656,
            }
        ):
            raise CacheError("qualified channel-0/f MUSIC descriptor changed")
        music_metadata = decode_container_blob(
            decoder,
            music_container,
            music_descriptor_raw["metadata_offset"],
            music_descriptor_raw["metadata_size"],
            rnc_work,
            "music-channel-0-f-metadata",
        )
        music_data = decode_container_blob(
            decoder,
            music_container,
            music_descriptor_raw["data_offset"],
            music_descriptor_raw["data_size"],
            rnc_work,
            "music-channel-0-f-data",
        )
        music_records = packed_audio_records(
            music_metadata,
            music_data,
            expected_count=6,
            label="music-channel-0-f",
        )
        expected_music_names = [
            "C2GAME1.XMI", "C2GAME2.XMI", "C2GAME3.XMI",
            "C2SETUP.XMI", "C2INTRO.XMI", "C2CUTS.XMI",
        ]
        if (
            [row["filename"] for row in music_records] != expected_music_names
            or any(row["word"] != 90 for row in music_records)
        ):
            raise CacheError("qualified channel-0/f MUSIC metadata changed")
        music_payload = music_data[:music_records[0]["bytes"]]
        if (
            music_payload[:4] != b"FORM"
            or music_payload[8:12] != b"XDIR"
        ):
            raise CacheError("C2GAME1 is not an XMIDI FORM/XDIR payload")
        music_xmi_output = write_output(
            stage,
            outputs,
            "audio/music/C2GAME1.xmi",
            music_payload,
            kind="owner_XMIDI_FORM_XDIR_exact_record",
            source_asset_ids=["owner_data:SOUND/MUSIC.DAT"],
            extra={
                "logical_track": 1,
                "playback_slot": 0,
                "loop_scan_pointer_slot": 1,
            },
        )
        xmi_converter, xmi_provenance = compile_xmi_converter(rnc_work)
        music_midi = transcode_xmi(
            xmi_converter, music_payload, rnc_work, "C2GAME1"
        )
        playback_control_census = midi_dynamic_control_census(
            music_midi, "C2GAME1 converted MIDI"
        )
        if (
            playback_control_census["trigger_events"] != []
            or playback_control_census["tagged_channels"] != []
            or playback_control_census["loop_controller_events"] != [
                {"tick": 0, "channel": 1, "controller": 116, "value": 0},
                {
                    "tick": 158265,
                    "channel": 1,
                    "controller": 117,
                    "value": 127,
                },
            ]
        ):
            raise CacheError("qualified C2GAME1 dynamic control census changed")
        scan_record = music_records[1]
        scan_offset = scan_record["relative_data_offset"]
        scan_payload = music_data[
            scan_offset:scan_offset + scan_record["bytes"]
        ]
        if scan_payload[:4] != b"FORM" or scan_payload[8:12] != b"XDIR":
            raise CacheError("C2GAME2 loop-scan pointer is not XMIDI")
        scan_midi = transcode_xmi(
            xmi_converter, scan_payload, rnc_work, "C2GAME2-census"
        )
        scan_control_census = midi_dynamic_control_census(
            scan_midi, "C2GAME2 loop-scan converted MIDI"
        )
        if (
            scan_control_census["trigger_events"] != [
                {"tick": 0, "channel": 7, "controller": 119, "value": 0},
                {"tick": 0, "channel": 8, "controller": 119, "value": 0},
            ]
            or scan_control_census["tagged_channels"] != [7, 8]
        ):
            raise CacheError("qualified C2GAME2 dynamic control census changed")
        music_midi_output = write_output(
            stage,
            outputs,
            "audio/music/C2GAME1.mid",
            music_midi,
            kind="standard_MIDI_deterministic_remc2_conversion",
            source_asset_ids=["owner_data:SOUND/MUSIC.DAT"],
            extra={
                "logical_track": 1,
                "playback_slot": 0,
                "conversion_status":
                    "deterministic_container_conversion_not_DOS_timbre",
            },
        )
        adl_renderer, adl_provenance = compile_adl_renderer()
        music_wave = render_midi_wave(
            adl_renderer, music_midi, rnc_work, "C2GAME1"
        )
        music_wave_properties = wav_properties(music_wave, "C2GAME1 rendered music")
        music_wave_output = write_output(
            stage,
            outputs,
            "audio/music/C2GAME1-adlmidi-bank59.wav",
            music_wave,
            kind="Unity_ready_PCM16_stereo_deterministic_libADLMIDI_render",
            source_asset_ids=["owner_data:SOUND/MUSIC.DAT"],
            extra={
                "logical_track": 1,
                "playback_slot": 0,
                "rendering": "libADLMIDI_bank59_two_chips",
                "presentation_status":
                    "deterministic_owner_local_adaptation_not_exact_DOS_hardware_timbre",
                **music_wave_properties,
            },
        )
        music_descriptor = {
            "schema": "mc2-level1-music-channel/v1",
            "source_asset_id": "owner_data:SOUND/MUSIC.DAT",
            "source_sha256": sha256_bytes(music_container),
            "channel": 0,
            "driver_type": "f",
            "driver_index": 2,
            "driver_config": "SBPRO2.MDI",
            "container": music_descriptor_raw,
            "channel_counts_by_driver_index": music_counts,
            "metadata_records": music_records,
            "logical_track": 1,
            "playback_slot": 0,
            "loop_scan_pointer_slot": 1,
            "payload_filename": "C2GAME1.XMI",
            "xmi_path": music_xmi_output["path"],
            "xmi_sha256": music_xmi_output["sha256"],
            "midi_path": music_midi_output["path"],
            "midi_sha256": music_midi_output["sha256"],
            "unity_wave_path": music_wave_output["path"],
            "unity_wave_sha256": music_wave_output["sha256"],
            "unity_wave": music_wave_properties,
            "unity_wave_renderer": adl_provenance,
            "dynamic_control": {
                "update_mode_field": "target_mode",
                "timer_multiplier": 3,
                "timer_hz_formula": "30 * timer_multiplier",
                "timer_hz": 90,
                "envelope_min": 0,
                "envelope_max": 127,
                "envelope_step_per_callback": 1,
                "playback_slot_census": playback_control_census,
                "loop_scan_pointer_census": scan_control_census,
                "audible_channel_resolution":
                    "Unresolved_playback_slot0_has_no_CC119_but_loop_scan_slot1_tags_channels7_8",
            },
            "initial_start_status":
                "SourceOrderQualified_remc2_local_gameTurn1_after_second_DrawAndEvents",
            "exact_DOS_hardware_timbre": "Deferred",
            "optional_random_OGG_path": "Excluded",
        }
        music_descriptor_output = write_output(
            stage,
            outputs,
            "audio/music-channel-0-f.json",
            canonical_json(music_descriptor),
            kind="canonical_json_Level1_music_channel_descriptor",
            source_asset_ids=["owner_data:SOUND/MUSIC.DAT"],
        )
        event_semantics = {
            "schema": "mc2-level1-audio-event-semantics/v1",
            "bank": 1,
            "quality": 822,
            "consume_event_kinds": [
                "PrepareSound", "EndLoop", "UpdateMusic", "GoatSoundRequest"
            ],
            "request_spell_activation_audio_status":
                "NoMapping; must not infer Fireball audio",
            "prepare_sound_ids": {
                "ambient_loops": [1, 2, 5, 31],
                "periodic": {
                    "effect_0x02": 12,
                    "effect_0x10": 39,
                    "effect_0x13": 43,
                    "effect_0x19": 37,
                },
                "insufficient_mana": 29,
            },
            "cave_ids": {
                "exact_range": [65, 69],
                "status": "Deferred_Cave_bank_not_Level1_Night",
            },
            "loop_targets": {"1": 70, "2": 70, "5": 120, "31": 85},
            "end_loop": {
                "fade_target": 0,
                "multiplier": 2,
                "callback_hz": 60,
                "volume_step_per_callback": 1,
            },
            "music_update": {
                "group": 3,
                "timer_hz": 90,
                "mode_1_target": 0,
                "mode_2_target": 127,
                "volume_step_per_callback": 1,
                "track_start_event_status":
                    "Absent_from_current_ABI; presentation adaptation",
            },
            "positional_surfaces": {
                "status":
                    "Ready_via_typed_McAudioCommandV2_original_integer_stereo",
                "command_required_sound_ids": [37, 39, 43],
                "missing_command_reason": "missing_positional_command",
                "attenuation_axis": "XY_only_original_helper_despite_XYZ_name",
                "unity_spatial_blend": 0,
                "unity_doppler": 0,
                "unity_default_rolloff": "Disabled",
            },
            "goat_sound_request": {
                "sound_id": 46,
                "event_kind": "GoatSoundRequest",
                "asset_status": "Ready_exact_owner_RIFF_WAVE",
                "position_source": "Rust_owned_live_Goat_snapshot",
                "play_type": "if_not_already_playing",
                "numeric_pitch_status":
                    "Deferred_until_source_ordered_global_rng_is_qualified",
                "presentation_policy":
                    "Do_not_play_as_exact_without_numeric_pitch",
            },
            "unknown_policy":
                "structured_deterministic_suppressed_log_no_fallback_clip",
        }
        event_semantics_output = write_output(
            stage,
            outputs,
            "audio/event-semantics.json",
            canonical_json(event_semantics),
            kind="canonical_json_Level1_audio_event_contract",
            source_asset_ids=[
                "owner_data:SOUND/SOUND.DAT",
                "owner_data:SOUND/MUSIC.DAT",
            ],
        )
        write_output(
            stage,
            outputs,
            "palette/paln.rgb6",
            source_bytes["DATA/PALN-0.DAT"],
            kind="ordered_rgb6_palette",
            source_asset_ids=["owner_data:DATA/PALN-0.DAT"],
            extra={
                "entries": 256,
                "palette_status": "exact_night_base_palette",
            },
        )
        paln_rgb6 = source_bytes["DATA/PALN-0.DAT"]
        if len(paln_rgb6) != 768 or max(paln_rgb6) > 63:
            raise CacheError("qualified PALN is not 256 RGB6 triplets")
        paln_rgb8 = bytes(value * 4 for value in paln_rgb6)
        paln_rgb8_output = write_output(
            stage,
            outputs,
            "palette/paln.rgb8",
            paln_rgb8,
            kind="ordered_rgb8_palette_exact_vga_scale",
            source_asset_ids=["owner_data:DATA/PALN-0.DAT"],
            extra={
                "entries": 256,
                "conversion": "channel_u8 = channel_rgb6 * 4",
                "maximum_channel": 252,
            },
        )
        tablesn = source_bytes["DATA/TABLESN.DAT"]
        if len(tablesn) != 83456:
            raise CacheError("qualified TABLESN byte count changed")
        tablesn_output = write_output(
            stage,
            outputs,
            "palette/tablesn.u8",
            tablesn,
            kind="remc2_night_renderer_tables_u8",
            source_asset_ids=["owner_data:DATA/TABLESN.DAT"],
            extra={
                "shade_lut_offset": 0,
                "shade_lut_bytes": 65536,
                "shade_lut_index":
                    "TABLESN[(brightness_u8 << 8) | source_palette_index_u8]",
            },
        )
        shade_lut_output = write_output(
            stage,
            outputs,
            "palette/tablesn.shade-lut.indices.u8",
            tablesn[:65536],
            kind="indexed_palette_shade_lut_256x256_u8",
            source_asset_ids=["owner_data:DATA/TABLESN.DAT"],
            extra={
                "width": 256,
                "height": 256,
                "x": "source_palette_index_u8",
                "y": "brightness_u8",
                "result": "PALN_palette_index_u8",
            },
        )
        clrn = source_bytes["DATA/CLRN-0.DAT"]
        if len(clrn) != 4096:
            raise CacheError("qualified CLRN byte count changed")
        clrn_output = write_output(
            stage,
            outputs,
            "palette/clrn.indices.u8",
            clrn,
            kind="night_rgb444_to_palette_index_lut_u8",
            source_asset_ids=["owner_data:DATA/CLRN-0.DAT"],
            extra={
                "entries": 4096,
                "index_semantics": "remc2 Night 12-bit colour lookup",
                "runtime_status":
                    "cached_and_hash_locked_not_required_by_static_terrain_shade_bridge",
            },
        )
        write_output(
            stage,
            outputs,
            "sky/skyn.indices.u8",
            source_bytes["DATA/SKYN0-0.DAT"],
            kind="indexed_image_u8",
            source_asset_ids=["owner_data:DATA/SKYN0-0.DAT"],
            extra={
                "width": 256,
                "height": 256,
                "palette_asset_id": "owner_data:DATA/PALN-0.DAT",
                "palette_status": "proven_night_binding",
            },
        )

        bl32_atlas = source_bytes["DATA/BL32N0-0.DAT"]
        if len(bl32_atlas) % (BL32_ATLAS_WIDTH * BL32_TILE_SIDE):
            raise CacheError("BL32N is not an exact 256-wide atlas of 32x32 tiles")
        bl32_count = len(bl32_atlas) // BL32_TILE_BYTES
        if bl32_count != 152:
            raise CacheError("qualified Night BL32N bank must contain exactly 152 tiles")
        bl32_tiles = bytearray()
        for tile in range(bl32_count):
            tile_x = (tile % BL32_ATLAS_COLUMNS) * BL32_TILE_SIDE
            tile_y = (tile // BL32_ATLAS_COLUMNS) * BL32_TILE_SIDE
            for row in range(BL32_TILE_SIDE):
                start = (tile_y + row) * BL32_ATLAS_WIDTH + tile_x
                bl32_tiles.extend(bl32_atlas[start:start + BL32_TILE_SIDE])
        if len(bl32_tiles) != len(bl32_atlas):
            raise CacheError("BL32N atlas repack changed byte count")
        used_tile_rows = []
        for selector in level1_terrain_selectors:
            start = selector * BL32_TILE_BYTES
            used_tile_rows.append({
                "selector": selector,
                "sha256": sha256_bytes(
                    bytes(bl32_tiles[start:start + BL32_TILE_BYTES])
                ),
            })
        bl32_atlas_output = write_output(
            stage,
            outputs,
            "terrain/bl32n.atlas.indices.u8",
            bl32_atlas,
            kind="indexed_atlas_u8",
            source_asset_ids=["owner_data:DATA/BL32N0-0.DAT"],
            extra={
                "width": BL32_ATLAS_WIDTH,
                "height": len(bl32_atlas) // BL32_ATLAS_WIDTH,
                "tile_width": BL32_TILE_SIDE,
                "tile_height": BL32_TILE_SIDE,
                "tile_columns": BL32_ATLAS_COLUMNS,
                "tile_count": bl32_count,
                "palette_asset_id": "owner_data:DATA/PALN-0.DAT",
            },
        )
        bl32_output = write_output(
            stage,
            outputs,
            "terrain/bl32n.indices.u8",
            bytes(bl32_tiles),
            kind="indexed_tile_bank_u8",
            source_asset_ids=["owner_data:DATA/BL32N0-0.DAT"],
            extra={
                "tile_width": BL32_TILE_SIDE,
                "tile_height": BL32_TILE_SIDE,
                "tile_count": bl32_count,
                "palette_asset_id": "owner_data:DATA/PALN-0.DAT",
            },
        )

        tmap_dat = source_bytes["DATA/TMAPS1-0.DAT"]
        tmap_tab = tmap_records(source_bytes["DATA/TMAPS1-0.TAB"])
        tmap_rows: list[dict[str, Any]] = []
        for index in sorted(scene_tmaps):
            if index + 1 >= len(tmap_tab):
                raise CacheError(f"TMAPS index {index} has no following offset sentinel")
            metadata, offset, group = tmap_tab[index]
            _, end_offset, _ = tmap_tab[index + 1]
            if offset < 0 or end_offset < offset or end_offset > len(tmap_dat):
                raise CacheError(f"TMAPS packed span is invalid at index {index}")
            packed = tmap_dat[offset:end_offset]
            asset_id = f"owner_data:DATA/TMAPS1-0.DAT#{index}"
            catalog_entry = entry_rows[asset_id]
            if (
                catalog_entry["offset"] != offset
                or catalog_entry["end_offset"] != end_offset
                or catalog_entry["metadata_dword_0"] != metadata
                or catalog_entry["animation_or_allocation_group"] != group
                or catalog_entry["packed_span_sha256"] != sha256_bytes(packed)
            ):
                raise CacheError(f"TMAPS TAB/catalog disagreement at index {index}")
            inclusion = (
                "qualified_frame_used"
                if index in frame_tmaps
                else "qualified_scene_reachable"
            )
            if catalog_entry["decoder_status"] == "raw_uncompressed_tmap_payload":
                if metadata != 1 or packed != b"0":
                    raise CacheError(f"unexpected raw TMAP payload at index {index}")
                tmap_rows.append(
                    {
                        "index": index,
                        "inclusion": inclusion,
                        "status": "raw_single_byte_placeholder_nondecoded",
                        "animation_or_allocation_group": group,
                        "source_asset_id": asset_id,
                    }
                )
                continue
            decoded = decode_tmap_rnc(decoder, packed, rnc_work, index)
            if len(decoded) < TMAP_HEADER.size:
                raise CacheError(f"decoded TMAP header is truncated at index {index}")
            header_word, width, height = TMAP_HEADER.unpack_from(decoded)
            first_frame_end = TMAP_HEADER.size + width * height
            if metadata != len(decoded) or not width or not height or len(decoded) < first_frame_end:
                raise CacheError(f"decoded TMAP dimensions disagree at index {index}")
            animated = (header_word & 1) != 0
            if animated:
                if len(decoded) <= first_frame_end or decoded[first_frame_end] == 0:
                    raise CacheError(f"animated TMAP has no frame stream at index {index}")
                frame_count = decoded[first_frame_end]
            else:
                if len(decoded) != first_frame_end:
                    raise CacheError(f"static TMAP has trailing data at index {index}")
                frame_count = 1
            decoded_output = write_output(
                stage,
                outputs,
                f"terrain/tmaps1/{index:03d}.decoded.bin",
                decoded,
                kind="remc2_type_particle_str_decoded_payload",
                source_asset_ids=[asset_id],
                extra={
                    "index": index,
                    "header_word": header_word,
                    "width": width,
                    "height": height,
                    "animated": animated,
                    "frame_count": frame_count,
                },
            )
            pixels = decoded[TMAP_HEADER.size:first_frame_end]
            pixel_output = write_output(
                stage,
                outputs,
                f"terrain/tmaps1/{index:03d}.indices.u8",
                pixels,
                kind="indexed_image_u8",
                source_asset_ids=[asset_id],
                extra={
                    "index": index,
                    "width": width,
                    "height": height,
                    "palette_asset_id": "owner_data:DATA/PALN-0.DAT",
                },
            )
            tmap_rows.append(
                {
                    "index": index,
                    "inclusion": inclusion,
                    "status": "decoded_indexed_texture",
                    "header_word": header_word,
                    "width": width,
                    "height": height,
                    "animated": animated,
                    "frame_count": frame_count,
                    "animation_or_allocation_group": group,
                    "indices_path": pixel_output["path"],
                    "decoded_payload_path": decoded_output["path"],
                    "source_asset_id": asset_id,
                    "row_order": "top_to_bottom",
                    "origin": "top_left",
                    "palette_asset_id": "owner_data:DATA/PALN-0.DAT",
                }
            )

        material_descriptor = {
            "schema": "mc2-level1-night-materials/v1",
            "palette": {
                "path": "palette/paln.rgb6",
                "encoding": "256_ordered_rgb_triplets_6_bit_channels",
                "rgb6_to_rgb8": "channel_u8 = channel_rgb6 * 4",
                "rgb8_path": paln_rgb8_output["path"],
                "tablesn_path": tablesn_output["path"],
                "shade_lut_path": shade_lut_output["path"],
                "clrn_path": clrn_output["path"],
            },
            "bl32n": {
                "indices_path": bl32_output["path"],
                "source_atlas_path": bl32_atlas_output["path"],
                "source_atlas_width": BL32_ATLAS_WIDTH,
                "source_atlas_height": len(bl32_atlas) // BL32_ATLAS_WIDTH,
                "source_atlas_columns": BL32_ATLAS_COLUMNS,
                "tile_width": BL32_TILE_SIDE,
                "tile_height": BL32_TILE_SIDE,
                "tile_count": bl32_count,
                "row_order": "top_to_bottom",
                "origin": "top_left",
                "terrain_selector": "terrain_type_plane_u8_direct_tile_index",
                "level1_used_selectors": level1_terrain_selectors,
                "level1_used_selector_count": len(level1_terrain_selectors),
                "level1_used_selector_max": max(level1_terrain_selectors),
                "level1_used_tile_sha256": used_tile_rows,
                "repack_provenance": {
                    "algorithm":
                        "remc2_sub_5B7A0_256_stride_32x32_address_grid",
                    "source_sha256": sha256_file(REMC2_TEXTURE_MAPPING),
                },
            },
            "terrain_uv": {
                "source_path":
                    "upstream/magic-carpet-2-hd/remc2/engine/UVTable_D4350.cpp",
                "source_sha256": sha256_file(REMC2_UV_TABLE),
                "source_rows_camera_relative": parse_source_uv_rows(),
                "source_row_count": 32,
                "persistent_group_expression": "(terrain_angle_u8 >> 4) & 7",
                "unity_vertex_order": [
                    "lower_left", "lower_right", "upper_left", "upper_right"
                ],
                "persistent_unity_groups": [
                    [list(point) for point in group]
                    for group in UNITY_TERRAIN_UV_GROUPS
                ],
                "persistent_unity_group_records": [
                    {
                        "group": index,
                        "points_flat": [
                            coordinate for point in group for coordinate in point
                        ],
                    }
                    for index, group in enumerate(UNITY_TERRAIN_UV_GROUPS)
                ],
                "triangle_diagonals": {
                    "even_x_plus_y": "lower_left_to_upper_right",
                    "odd_x_plus_y": "upper_left_to_lower_right",
                },
            },
            "tmaps1": {
                "frame_used_selectors": sorted(frame_tmaps),
                "scene_reachable_selectors": sorted(scene_tmaps),
                "textures": tmap_rows,
                "animation_semantics":
                    "TAB_group_preserved; equal group values identify source allocation/animation families",
            },
            "rendering_scope": {
                "qualified":
                    "indexed BL32N selection, D4 UV orientation, TABLESN static "
                    "terrain-shade lookup, PALN lookup",
                "not_yet_qualified":
                    "camera/distance pnt5 fog attenuation, water wave brightness, "
                    "full software-rasterizer interpolation",
                "claim": "indexed_night_material_and_static_shade_bridge",
            },
        }
        material_output = write_output(
            stage,
            outputs,
            "terrain/materials.json",
            canonical_json(material_descriptor),
            kind="canonical_json_material_descriptor",
            source_asset_ids=[
                "owner_data:DATA/BL32N0-0.DAT",
                "owner_data:DATA/TMAPS1-0.DAT",
                "owner_data:DATA/TMAPS1-0.TAB",
                "owner_data:DATA/PALN-0.DAT",
                "owner_data:DATA/TABLESN.DAT",
                "owner_data:DATA/CLRN-0.DAT",
            ],
            extra={
                "bl32_tile_count": bl32_count,
                "tmap_scene_selector_count": len(scene_tmaps),
                "tmap_frame_selector_count": len(frame_tmaps),
            },
        )
        entity_descriptor = build_entity_presentation_descriptor(
            (package_dir / "entities.bin").read_bytes(),
            (package_dir / "presentation.bin").read_bytes(),
            terrain_export,
            tmap_rows,
        )
        entity_output = write_output(
            stage,
            outputs,
            "entities/presentation.json",
            canonical_json(entity_descriptor),
            kind="canonical_json_level1_entity_presentation_descriptor",
            source_asset_ids=[
                "level_export:entities.bin",
                "level_export:presentation.bin",
                "owner_data:DATA/TMAPS1-0.DAT",
                "owner_data:DATA/TMAPS1-0.TAB",
                "owner_data:DATA/PALN-0.DAT",
            ],
            extra={
                "active_reset_count": entity_descriptor["active_reset_count"],
                "qualified_actor_count": entity_descriptor["qualified_actor_count"],
                "qualified_view_count":
                    entity_descriptor["qualified_view_count"],
                "reset_chain_drawable_count":
                    entity_descriptor["reset_chain_drawable_count"],
                "qualified_group_count": entity_descriptor["qualified_group_count"],
            },
        )
        shutil.rmtree(rnc_work)

        proven_decode = load_proven_decoder()
        mspr_dat = source_bytes["DATA/MSPRN0-0.DAT"]
        mspr_records = bitmap_records(source_bytes["DATA/MSPRN0-0.TAB"])
        sprite_rows: list[dict[str, Any]] = []
        sprite_materialized: dict[int, tuple[bytes, bytes, int, int]] = {}
        for index in sorted(set(FRAME_MSPRN + DYNAMIC_MSPRN)):
            offset, width, height = mspr_records[index]
            pixels, mask, encoded_length = materialize_indexed_rle(
                mspr_dat, offset, width, height
            )
            proven = proven_decode(mspr_dat, offset, width, height)
            catalog_entry = entry_rows[f"owner_data:DATA/MSPRN0-0.DAT#{index}"]
            if (
                proven["status"] != "decoded_indexed_rle"
                or proven["indexed_pixels_sha256"] != sha256_bytes(pixels)
                or proven["transparency_mask_sha256"] != sha256_bytes(mask)
                or catalog_entry["indexed_pixels_sha256"] != sha256_bytes(pixels)
                or catalog_entry["transparency_mask_sha256"] != sha256_bytes(mask)
                or proven["encoded_length"] != encoded_length
            ):
                raise CacheError(f"MSPRN decoder/catalog disagreement at index {index}")
            reason = (
                "qualified_frame_used"
                if index in FRAME_MSPRN
                else "qualified_dynamic_spell_menu_dependency"
            )
            index_row = write_output(
                stage,
                outputs,
                f"sprites/msprn/{index:03d}.indices.u8",
                pixels,
                kind="indexed_image_u8",
                source_asset_ids=[catalog_entry["asset_id"]],
                extra={"index": index, "width": width, "height": height},
            )
            mask_row = write_output(
                stage,
                outputs,
                f"sprites/msprn/{index:03d}.mask.u8",
                mask,
                kind="visibility_mask_u8_0_or_1",
                source_asset_ids=[catalog_entry["asset_id"]],
                extra={"index": index, "width": width, "height": height},
            )
            sprite_materialized[index] = (pixels, mask, width, height)
            sprite_rows.append(
                {
                    "index": index,
                    "inclusion": reason,
                    "width": width,
                    "height": height,
                    "indices_path": index_row["path"],
                    "mask_path": mask_row["path"],
                    "indices_sha256": index_row["sha256"],
                    "mask_sha256": mask_row["sha256"],
                    "palette_candidates": [
                        "owner_data:DATA/PALN-0.DAT",
                    ],
                    "palette_status": "PALN_materialized_fade_variant_unresolved_not_cached",
                    "stored_dimensions": {"width": width, "height": height},
                    "logical_scale": 2,
                    "runtime_dimensions": {"width": width * 2, "height": height * 2},
                    "row_order": "top_to_bottom",
                    "origin": "top_left",
                    "mask_semantics": "one_visible_zero_transparent",
                }
            )

        font_dat = source_bytes["DATA/FONT1.DAT"]
        font_records = bitmap_records(source_bytes["DATA/FONT1.TAB"])
        font_blob = bytearray()
        glyph_rows: list[dict[str, Any]] = []
        for index, (offset, width, height) in enumerate(font_records):
            pixels, mask, encoded_length = materialize_indexed_rle(
                font_dat, offset, width, height
            )
            proven = proven_decode(font_dat, offset, width, height)
            catalog_entry = entry_rows[f"owner_data:DATA/FONT1.DAT#{index}"]
            if (
                proven["indexed_pixels_sha256"] != sha256_bytes(pixels)
                or proven["transparency_mask_sha256"] != sha256_bytes(mask)
                or catalog_entry["transparency_mask_sha256"] != sha256_bytes(mask)
                or proven["encoded_length"] != encoded_length
            ):
                raise CacheError(f"FONT1 decoder/catalog disagreement at glyph {index}")
            blob_offset = len(font_blob)
            font_blob.extend(mask)
            glyph_rows.append(
                {
                    "index": index,
                    "offset": blob_offset,
                    "length": len(mask),
                    "width": width,
                    "height": height,
                    "mask_sha256": sha256_bytes(mask),
                    "source_asset_id": catalog_entry["asset_id"],
                    "palette_status":
                        "mask_exact_runtime_tint_callsite_dynamic",
                }
            )
        font_output = write_output(
            stage,
            outputs,
            "font/font1.masks.u8",
            bytes(font_blob),
            kind="concatenated_visibility_masks_u8_0_or_1",
            source_asset_ids=["owner_data:DATA/FONT1.DAT", "owner_data:DATA/FONT1.TAB"],
            extra={"glyph_count": len(glyph_rows)},
        )

        language_l1 = language_records(source_bytes["LANGUAGE/L1.TXT"])
        language_d2 = language_records(source_bytes["LANGUAGE/D2.TXT"])
        if language_l1 != language_d2:
            raise CacheError("owned L1 and D2 localization payloads are not byte-identical")
        expected_objective = b'"Fly towards my beacon."'
        if language_l1[LEVEL1_OBJECTIVE_SELECTOR] != expected_objective:
            raise CacheError("qualified Level 1 objective localization changed")
        if any(not language_l1[index] for index in LEVEL1_OBJECTIVE_IDS):
            raise CacheError("qualified Level 1 objective stage localization is empty")

        players_blob = (package_dir / "players.bin").read_bytes()
        if (
            len(players_blob) != 4596
            or players_blob[:4] != b"MCP1"
            or struct.unpack_from("<HHI", players_blob, 4) != (2, 573, 8)
        ):
            raise CacheError("players.bin is not the qualified MCP1/2 8x573 form")
        player0 = players_blob[12:12 + 573]
        enabled_holders = struct.unpack_from("<26H", player0, 268)
        left_slot = struct.unpack_from("<H", player0, 554)[0]
        right_slot = struct.unpack_from("<H", player0, 556)[0]
        if (
            left_slot != 0
            or right_slot != 1
            or enabled_holders[left_slot] != 153
            or enabled_holders[right_slot] != 154
        ):
            raise CacheError("qualified Level 1 reset spell slots changed")
        entities_blob = (package_dir / "entities.bin").read_bytes()
        if entities_blob[:4] != b"MCE1":
            raise CacheError("entities.bin is not the qualified MCE1 form")
        record_size = struct.unpack_from("<H", entities_blob, 6)[0]
        holder_models = []
        for entity_index in (153, 154):
            start = 12 + entity_index * record_size
            holder_models.append(entities_blob[start + 68])
        if holder_models != [0, 1]:
            raise CacheError("qualified reset spell holder models changed")
        player_start = 12 + 152 * record_size
        player_x = struct.unpack_from("<H", entities_blob, player_start + 80)[0]
        player_y = struct.unpack_from("<H", entities_blob, player_start + 82)[0]
        if (player_x, player_y) != (19840, 56960):
            raise CacheError("qualified Level 1 reset player position changed")

        minimap_mask, minimap_scanlines = build_minimap_aperture(tablesn)
        minimap_mask_output = write_output(
            stage,
            outputs,
            "hud/minimap-aperture-64.mask.u8",
            minimap_mask,
            kind="u8_TABLESN_scanline_aperture_mask",
            source_asset_ids=["owner_data:DATA/TABLESN.DAT"],
            extra={
                "logical_width": LEVEL1_MINIMAP_LOGICAL_SIDE,
                "logical_height": LEVEL1_MINIMAP_LOGICAL_SIDE,
                "opaque_pixels": sum(minimap_mask),
                "table_offset": 0x14300,
                "table_length": 256,
                "phase_step": 0x1000000 // (
                    LEVEL1_MINIMAP_LOGICAL_SIDE // 2
                ),
            },
        )
        sine_table, sine_payload = load_remc2_sine_table()
        minimap_sine_output = write_output(
            stage,
            outputs,
            "hud/remc2-sine-2560.i32le",
            sine_payload,
            kind="i32le_remc2_Maths_sin_DB750",
            source_asset_ids=[
                "remc2_source:utilities/Maths.cpp#Maths::sin_DB750"
            ],
            extra={
                "entry_count": len(sine_table),
                "angle_period": 2048,
                "fixed_one": 65536,
            },
        )
        minimap_direction_samples, minimap_octant_probe = (
            build_minimap_direction_samples(
                player_x,
                player_y,
                terrain_export[:65536],
                terrain_export[2 * 65536:3 * 65536],
                tablesn,
                sine_table,
            )
        )

        objective_text_index = source_bytes["DATA/CLRN-0.DAT"][3840]
        objective_background_index = source_bytes["DATA/CLRN-0.DAT"][0]
        if objective_text_index != 119 or objective_background_index != 61:
            raise CacheError("qualified Night objective CLRN selectors changed")
        objective_rgb_offset = objective_text_index * 3
        if tuple(paln_rgb8[objective_rgb_offset:objective_rgb_offset + 3]) != (
            252, 36, 36
        ):
            raise CacheError("qualified Night objective tint no longer matches PALN red")
        objective_rgba, badge_rgba, objective_render = build_objective_rasters(
            expected_objective,
            glyph_rows,
            bytes(font_blob),
            sprite_materialized,
            paln_rgb8,
            objective_text_index,
        )
        objective_output = write_output(
            stage,
            outputs,
            "hud/objective-reset.rgba8",
            objective_rgba,
            kind="rgba8_frozen_reset_objective_frame_and_FONT1_text",
            source_asset_ids=[
                "owner_data:LANGUAGE/L1.TXT",
                "owner_data:DATA/FONT1.DAT",
                "owner_data:DATA/MSPRN0-0.DAT#171",
                "owner_data:DATA/MSPRN0-0.DAT#172",
                "owner_data:DATA/MSPRN0-0.DAT#173",
                "owner_data:DATA/CLRN-0.DAT",
                "owner_data:DATA/PALN-0.DAT",
            ],
            extra={
                "width": objective_render["objective_width"],
                "height": objective_render["objective_height"],
                "authority": "FrozenObserved",
                "selector": LEVEL1_OBJECTIVE_SELECTOR,
            },
        )
        badge_output = write_output(
            stage,
            outputs,
            "hud/frozen-observed.rgba8",
            badge_rgba,
            kind="rgba8_FONT1_authority_badge",
            source_asset_ids=[
                "owner_data:DATA/FONT1.DAT",
                "owner_data:DATA/CLRN-0.DAT",
                "owner_data:DATA/PALN-0.DAT",
            ],
            extra={
                "width": objective_render["badge_width"],
                "height": objective_render["badge_height"],
                "authority": "FrozenObserved",
            },
        )
        localization_rows = [
            {
                "selector": selector,
                "bytes_hex": language_l1[selector].hex(),
                "byte_length": len(language_l1[selector]),
                "sha256": sha256_bytes(language_l1[selector]),
            }
            for selector in LEVEL1_OBJECTIVE_IDS
        ]
        hud_bitmap_selectors = (1, 40, 41, 42, 43, 54, 123, 124, 171, 172, 173)
        sprite_by_index = {row["index"]: row for row in sprite_rows}
        hud_bitmap_assets = []
        for selector in hud_bitmap_selectors:
            row = sprite_by_index[selector]
            hud_bitmap_assets.append({
                "selector": selector,
                "bank": "MSPRN0-0",
                "source_asset_id":
                    f"owner_data:DATA/MSPRN0-0.DAT#{selector}",
                "indices_path": row["indices_path"],
                "mask_path": row["mask_path"],
                "indices_sha256": row["indices_sha256"],
                "mask_sha256": row["mask_sha256"],
                "stored_width": row["width"],
                "stored_height": row["height"],
                "runtime_width": row["runtime_dimensions"]["width"],
                "runtime_height": row["runtime_dimensions"]["height"],
                "source_scale": row["logical_scale"],
                "palette": "PALN-0",
                "palette_effects": "base_palette_only; transient variants Deferred",
                "filter": "nearest",
                "authority": "FrozenObserved",
            })
        hud_descriptor = {
            "schema": "mc2-level1-owner-hud/v1",
            "bitmap_assets": hud_bitmap_assets,
            "reference_layout": {
                "width": LEVEL1_REFERENCE_WIDTH,
                "height": LEVEL1_REFERENCE_HEIGHT,
                "uniform_scale_expression":
                    "min(Screen.width / 640, Screen.height / 480)",
                "filter": "nearest",
                "resolution_bank": "MSPRN0-0",
                "source_scale": HUD_SOURCE_SCALE,
                "bank_status":
                    "trace_qualified_Night_low_resolution_bank_scaled_2x",
                "high_resolution_bank_status":
                    "Deferred; no HSPRN parity claim in this cache",
            },
            "top_bar": {
                "source_function": "DrawTopStatusBar_2D710@0x20e710",
                "panels": [
                    {
                        "semantic_role": "minimap_panel",
                        "selector": 40,
                        "x": 4,
                        "y": 4,
                        "authority": "FrozenObserved",
                    },
                    {
                        "semantic_role": "castle_empty_panel",
                        "selector": 54,
                        "x": 128,
                        "y": 4,
                        "authority": "FrozenObserved",
                    },
                    {
                        "semantic_role": "balloon_empty_panel",
                        "selector": 54,
                        "x": 256,
                        "y": 4,
                        "authority": "FrozenObserved",
                    },
                    {
                        "semantic_role": "player_health_mana_panel",
                        "selector": 41,
                        "x": 384,
                        "y": 4,
                        "authority": "FrozenObserved",
                    },
                    {
                        "semantic_role": "player_icon",
                        "selector": 43,
                        "x": 388,
                        "y": 4,
                        "authority": "FrozenObserved",
                    },
                    {
                        "semantic_role": "player_health_mana_icons",
                        "selector": 42,
                        "x": 460,
                        "y": 4,
                        "authority": "FrozenObserved",
                    },
                ],
                "health_mana_bars": {
                    "authority": "Deferred",
                    "reason": "ABI v2 exposes no live player health or general mana",
                },
            },
            "spells": {
                "source_function": "DrawSpellIcon_2E260@0x20f260",
                "frame_selector": 1,
                "slots": [
                    {
                        "semantic_role": "left_fireball",
                        "x": 510,
                        "y": 4,
                        "holder_entity": 153,
                        "model": 0,
                        "icon_selector": 123,
                        "authority": "FrozenObserved",
                        "activation_authority": "SpellActivationLive",
                    },
                    {
                        "semantic_role": "right_possess",
                        "x": 574,
                        "y": 4,
                        "holder_entity": 154,
                        "model": 1,
                        "icon_selector": 124,
                        "authority": "FrozenObserved",
                        "activation_authority": "Deferred",
                    },
                ],
                "glow_tier_charge": {
                    "authority": "Deferred",
                    "reason":
                        "holder timer, tier and charge fields are absent from ABI v2",
                },
            },
            "objective": {
                "source_function": "DrawCurrentObjectiveTextbox_30630@0x211630",
                "selector_expression": "IndexLevelText_DB4EE[0](48) + textSelector(0)",
                "selector": LEVEL1_OBJECTIVE_SELECTOR,
                "text_bytes_path": "hud/objective-text-048.bytes",
                "text_sha256": sha256_bytes(expected_objective),
                "text_encoding": "owner_bytes_ascii_subset_preserved",
                "font_bank": "FONT1",
                "glyph_expression": "byte + 1; space advances glyph 33 width",
                "glyph_indices": objective_render["objective_glyph_indices"],
                "tint": {
                    "active_map_palette_lut": "CLRN-0",
                    "lut_selector": 3840,
                    "palette_index": objective_text_index,
                    "paln_rgb8": [252, 36, 36],
                    "historical_pointer_name":
                        "xadataclrd0dat; PaletteChanges_47760 installs CLRN for Night",
                },
                "frame_selectors": [171, 172, 173],
                "render_path": objective_output["path"],
                "render_width": objective_render["objective_width"],
                "render_height": objective_render["objective_height"],
                "reference_x": 192,
                "reference_y": 126,
                "authority": "FrozenObserved",
                "objective_textbox_ticks": 33,
                "localization_records": localization_rows,
            },
            "authority_badge": {
                "text": objective_render["badge_text_ascii"],
                "render_path": badge_output["path"],
                "render_width": objective_render["badge_width"],
                "render_height": objective_render["badge_height"],
                "reference_x": 192,
                "reference_y": 168,
            },
            "minimap": {
                "source_functions": [
                    "DrawMinimap_63600@0x244600",
                    "DrawMinimapEntites_61880@0x242880",
                    "DrawMinimapMarks_644F0@0x2454f0",
                ],
                "x": 0,
                "y": 0,
                "width": LEVEL1_MINIMAP_DISPLAY_SIDE,
                "height": LEVEL1_MINIMAP_DISPLAY_SIDE,
                "logical_width": LEVEL1_MINIMAP_LOGICAL_SIDE,
                "logical_height": LEVEL1_MINIMAP_LOGICAL_SIDE,
                "presentation_scale": HUD_SOURCE_SCALE,
                "map_mode":
                    "player_centered_yaw_rotated_TABLESN_scanline_aperture",
                "low_resolution_branch":
                    "MSPRN_effective_64x64_scaling512_then_nearest_2x",
                "sampling": {
                    "source_units_per_pixel": LEVEL1_MINIMAP_SOURCE_SCALE,
                    "transform":
                        "fixed_integer_remc2_sub_63C90_standard_display_branch",
                    "screen_column_vector": "(cosine_scaled,sine_scaled)",
                    "screen_row_vector": "(-sine_scaled,cosine_scaled)",
                    "terrain_base_index": "TABLESN[0x14000 + terrain_type]",
                    "terrain_shade_index":
                        "TABLESN[(terrain_shade << 8) | terrain_base_index]",
                    "palette": "PALN-0",
                    "framebuffer_blend_status":
                        "Deferred; live texture uses new indexed colour directly",
                },
                "aperture": {
                    "path": minimap_mask_output["path"],
                    "sha256": minimap_mask_output["sha256"],
                    "opaque_pixels": sum(minimap_mask),
                    "table_offset": 0x14300,
                    "table_curve_sha256": sha256_bytes(
                        tablesn[0x14300:0x14400]
                    ),
                    "phase_step": 0x1000000 // (
                        LEVEL1_MINIMAP_LOGICAL_SIDE // 2
                    ),
                    "scanlines": minimap_scanlines,
                },
                "rotation": {
                    "sine_table_path": minimap_sine_output["path"],
                    "sine_table_sha256": minimap_sine_output["sha256"],
                    "cardinal_direction_samples": minimap_direction_samples,
                    "octant_probe_samples": minimap_octant_probe,
                },
                "terrain_authority": "FrozenObserved",
                "entity_positions_authority": "FrozenObserved",
                "entity_overlay_mode":
                    "non_original_FrozenObserved_reset_reference_overlay",
                "player_pose_authority": "PlayerFlightLive",
                "entity_colour_filter_status":
                    "Deferred; exact DrawMinimapEntities filter/colour pass not claimed",
                "objective_marks_status":
                    "Deferred; objective progression and mark pass absent from ABI v2",
            },
            "deferred": [
                "HSPRN high-resolution bank parity",
                "paletteMod and paletteSubMod transient fade/hit/flash palettes",
                "live health, mana, castle, balloon, tier and charge progression",
                "exact minimap entity colours, filtering, reveal and objective marks",
                "minimap TABLESN framebuffer blend against the live 3D view",
                "textbox background TABLESN blend against the live framebuffer",
            ],
        }
        objective_text_output = write_output(
            stage,
            outputs,
            "hud/objective-text-048.bytes",
            expected_objective,
            kind="owner_localization_record_bytes",
            source_asset_ids=["owner_data:LANGUAGE/L1.TXT"],
            extra={
                "selector": LEVEL1_OBJECTIVE_SELECTOR,
                "authority": "FrozenObserved",
            },
        )
        if objective_text_output["path"] != hud_descriptor["objective"]["text_bytes_path"]:
            raise CacheError("objective descriptor output path mismatch")
        hud_output = write_output(
            stage,
            outputs,
            "hud/level1.json",
            canonical_json(hud_descriptor),
            kind="canonical_json_Level1_HUD_descriptor",
            source_asset_ids=[
                "owner_data:DATA/MSPRN0-0.DAT",
                "owner_data:DATA/FONT1.DAT",
                "owner_data:LANGUAGE/L1.TXT",
                "owner_data:DATA/CLRN-0.DAT",
                "owner_data:DATA/PALN-0.DAT",
            ],
            extra={
                "authority_model": "Live_FrozenObserved_Deferred",
                "objective_selector": LEVEL1_OBJECTIVE_SELECTOR,
            },
        )

        with SPELL_TABLE.open(newline="", encoding="utf-8") as stream:
            spell_rows = list(csv.DictReader(stream))
        if len(spell_rows) != 78:
            raise CacheError("qualified spell tier table must have 78 rows")
        families = []
        for family in range(26):
            rows = [row for row in spell_rows if int(row["family_id"]) == family]
            rows.sort(key=lambda row: int(row["tier"]))
            if len(rows) != 3 or [int(row["tier"]) for row in rows] != [0, 1, 2]:
                raise CacheError(f"qualified spell family {family} is incomplete")
            small = {int(row["family_icon_small_index"]) for row in rows}
            big = {int(row["family_icon_big_index"]) for row in rows}
            tiers = [int(row["tier_icon_index"]) for row in rows]
            if small != {97 + family} or big != {123 + family} or tiers != [
                179 + family * 3 + tier for tier in range(3)
            ]:
                raise CacheError(f"qualified spell mapping mismatch at family {family}")
            families.append({
                "family_id": family,
                "family_small_index": next(iter(small)),
                "family_big_index": next(iter(big)),
                "tier_indices": tiers,
            })
        spell_indices = {
            "schema": "mc2-spell-icon-indices/v1",
            "bank": "MSPRN0-0",
            "status": "family_mapping_with_Level1_reset_big_icons_materialized",
            "source_asset_id": "owner_data:DATA/SPELLS.DAT",
            "provenance": {
                "family_indices": "code-derived qualified family mapping",
                "tier_indices": "SPELLS-derived qualified tier table",
                "qualified_table_sha256": sha256_file(SPELL_TABLE),
                "qualified_generator_sha256": sha256_file(SPELL_GENERATOR),
            },
            "families": families,
        }
        spell_output = write_output(
            stage,
            outputs,
            "spells/icon-indices.json",
            canonical_json(spell_indices),
            kind="canonical_json_index_mapping",
            source_asset_ids=["owner_data:DATA/SPELLS.DAT"],
            extra={
                "family_count": 26,
                "bitmap_status":
                    "Level1_reset_big_icons_123_and_124_materialized; other families deferred",
            },
        )

        outputs.sort(key=lambda row: row["path"])
        identity_input = b"".join(
            f"{row['path']}\0{row['sha256']}\0{row['bytes']}\n".encode("ascii")
            for row in outputs
        )
        manifest = {
            "schema": SCHEMA,
            "scope": "PRIVATE_TRACK_P_OWNER_DATA",
            "distribution": "owner_local_runtime_supplied_not_redistributable",
            "bindings": {
                "level_export_package_sha256": EXPECTED_PACKAGE,
                "level_asset_closure_sha256": EXPECTED_CLOSURE,
                "owner_catalog_sha256": catalog_binding,
            },
            "generator": {
                "id": "remc2-unity-asset-cache",
                "version": GENERATOR_VERSION,
                "source_sha256": sha256_file(Path(__file__)),
            },
            "decoder": {
                "id": "remc2-atlas-proven-indexed-rle",
                "version": "catalog-v1",
                "source_sha256": sha256_file(PROVEN_DECODER),
                "verification": "materialized bytes must match proven decoder and qualified catalog hashes",
            },
            "rnc_decoder": rnc_provenance,
            "xmi_converter": xmi_provenance,
            "music_renderer": adl_provenance,
            "source_files": sorted(source_files, key=lambda row: row["path"]),
            "assets": {
                "palette": {
                    "path": "palette/paln.rgb6",
                    "status": "exact_ordered_rgb6_multiply_by_4_not_baked",
                    "tablesn_path": shade_lut_output["path"],
                    "clrn_path": clrn_output["path"],
                },
                "sky": {
                    "path": "sky/skyn.indices.u8",
                    "palette_status": "proven_night_binding",
                },
                "terrain_materials": {
                    "path": material_output["path"],
                    "palette_path": "palette/paln.rgb6",
                    "bl32_indices_path": bl32_output["path"],
                    "bl32_source_atlas_path": bl32_atlas_output["path"],
                    "bl32_tile_count": bl32_count,
                    "tmap_frame_used_selectors": sorted(frame_tmaps),
                    "tmap_scene_reachable_selector_count": len(scene_tmaps),
                    "tmap_decoded_texture_count":
                        sum(row["status"] == "decoded_indexed_texture" for row in tmap_rows),
                    "tmap_raw_placeholder_count":
                        sum(row["status"] == "raw_single_byte_placeholder_nondecoded" for row in tmap_rows),
                    "palette_status":
                        "exact_night_TABLESN_static_shade_then_PALN_indexed_runtime_lookup",
                    "animation_status": "TAB_group_semantics_preserved_in_descriptor",
                },
                "msprn": sprite_rows,
                "entity_presentations": {
                    "path": entity_output["path"],
                    "status": "qualified_Level1_owner_actor_billboards",
                    "qualified_actor_count":
                        entity_descriptor["qualified_actor_count"],
                    "mapped_reset_effect_count":
                        entity_descriptor["mapped_reset_effect_count"],
                    "reset_chain_drawable_count":
                        entity_descriptor["reset_chain_drawable_count"],
                    "qualified_view_count":
                        entity_descriptor["qualified_view_count"],
                    "qualified_group_count":
                        entity_descriptor["qualified_group_count"],
                    "active_reset_count":
                        entity_descriptor["active_reset_count"],
                    "hidden_reset_count":
                        entity_descriptor["active_reset_count"]
                        - entity_descriptor["qualified_view_count"],
                    "suppressed_terrain_head_count":
                        entity_descriptor["suppressed_terrain_head_count"],
                    "off_chain_active_count":
                        entity_descriptor["off_chain_active_count"],
                },
                "font1": {
                    "mask_blob_path": font_output["path"],
                    "glyph_count": len(glyph_rows),
                    "glyphs": glyph_rows,
                    "palette_status":
                        "mask_exact_runtime_tint_callsite_dynamic; Level1 objective CLRN3840 qualified",
                },
                "spell_icon_indices": {
                    "path": spell_output["path"],
                    "status":
                        "family_mapping_with_Level1_reset_big_icons_materialized",
                },
                "level1_hud": {
                    "path": hud_output["path"],
                    "status":
                        "qualified_owner_assets_mixed_Live_FrozenObserved_Deferred",
                    "objective_selector": LEVEL1_OBJECTIVE_SELECTOR,
                    "objective_text_path": objective_text_output["path"],
                    "objective_render_path": objective_output["path"],
                    "authority_badge_path": badge_output["path"],
                    "minimap_mode":
                        "player_centered_yaw_rotated_TABLESN_scanline_aperture",
                    "minimap_logical_resolution": [64, 64],
                    "minimap_display_resolution": [128, 128],
                },
                "level1_audio": {
                    "sound_descriptor_path": sound_descriptor_output["path"],
                    "music_descriptor_path": music_descriptor_output["path"],
                    "event_semantics_path": event_semantics_output["path"],
                    "sound_bank": 1,
                    "sound_quality": 822,
                    "sound_quality_index": 3,
                    "materialized_sound_ids":
                        list(LEVEL1_MATERIALIZED_SOUND_IDS),
                    "explicit_null_sound_ids": [12],
                    "music_channel": 0,
                    "music_driver_type": "f",
                    "music_driver_index": 2,
                    "logical_track": 1,
                    "playback_slot": 0,
                    "loop_scan_pointer_slot": 1,
                    "music_xmi_path": music_xmi_output["path"],
                    "music_midi_path": music_midi_output["path"],
                    "music_unity_wave_path": music_wave_output["path"],
                    "music_playback_status":
                        "Ready_deterministic_owner_local_adaptation_not_exact_DOS_timbre",
                },
            },
            "outputs": outputs,
            "cache_content_sha256": sha256_bytes(identity_input),
        }
        manifest["manifest_payload_sha256"] = sha256_bytes(canonical_json(manifest))
        (stage / "manifest.json").write_bytes(canonical_json(manifest))
        validate_cache(stage, EXPECTED_PACKAGE, EXPECTED_CLOSURE)
        fsync_tree(stage)

        commit_stage(output, stage)
        return manifest
    except Exception:
        if stage.exists():
            shutil.rmtree(stage)
        raise


def validate_cache(
    cache_dir: Path,
    expected_package: str = EXPECTED_PACKAGE,
    expected_closure: str = EXPECTED_CLOSURE,
) -> dict[str, Any]:
    cache_dir = cache_dir.expanduser().resolve()
    manifest_path = cache_dir / "manifest.json"
    manifest = load_json(manifest_path)
    manifest_claim = manifest.get("manifest_payload_sha256")
    manifest_payload = dict(manifest)
    manifest_payload.pop("manifest_payload_sha256", None)
    if sha256_bytes(canonical_json(manifest_payload)) != manifest_claim:
        raise CacheError("cache manifest metadata hash mismatch")
    if manifest.get("schema") != SCHEMA:
        raise CacheError("unsupported cache schema")
    bindings = manifest.get("bindings", {})
    if bindings.get("level_export_package_sha256") != expected_package:
        raise CacheError("cache package binding mismatch")
    if bindings.get("level_asset_closure_sha256") != expected_closure:
        raise CacheError("cache closure binding mismatch")
    outputs = manifest.get("outputs")
    if not isinstance(outputs, list) or not outputs:
        raise CacheError("cache manifest has no outputs")
    seen: set[str] = set()
    identity_parts: list[bytes] = []
    for row in outputs:
        relative = row.get("path")
        if (
            not isinstance(relative, str)
            or relative in seen
            or relative.startswith("/")
            or ".." in Path(relative).parts
        ):
            raise CacheError("invalid or duplicate cache output path")
        seen.add(relative)
        path = cache_dir / relative
        if not path.is_file():
            raise CacheError(f"cache output missing: {relative}")
        size = path.stat().st_size
        digest = sha256_file(path)
        if size != row.get("bytes"):
            raise CacheError(f"cache output truncated: {relative}")
        if digest != row.get("sha256"):
            raise CacheError(f"cache output hash mismatch: {relative}")
        identity_parts.append(
            f"{relative}\0{digest}\0{size}\n".encode("ascii")
        )
    actual_identity = sha256_bytes(b"".join(identity_parts))
    if actual_identity != manifest.get("cache_content_sha256"):
        raise CacheError("cache content identity mismatch")
    expected_tree = {"manifest.json", *seen}
    actual_tree: set[str] = set()
    for path in cache_dir.rglob("*"):
        if path.is_symlink():
            raise CacheError("cache tree contains a symlink")
        if path.is_file():
            actual_tree.add(path.relative_to(cache_dir).as_posix())
    if actual_tree != expected_tree:
        raise CacheError("cache tree has missing or extra files")
    return manifest


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    build = subparsers.add_parser("build")
    build.add_argument("--owner-root", type=Path, default=DEFAULT_OWNER)
    build.add_argument("--package", type=Path, default=DEFAULT_PACKAGE)
    build.add_argument("--closure", type=Path, default=DEFAULT_CLOSURE)
    build.add_argument("--catalog", type=Path, default=DEFAULT_CATALOG)
    build.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    validate = subparsers.add_parser("validate")
    validate.add_argument("--cache", type=Path, default=DEFAULT_OUTPUT)
    validate.add_argument("--package-sha256", default=EXPECTED_PACKAGE)
    validate.add_argument("--closure-sha256", default=EXPECTED_CLOSURE)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.command == "build":
            manifest = build_cache(
                args.owner_root, args.package, args.closure, args.catalog, args.output
            )
            print(
                f"cache={args.output} outputs={len(manifest['outputs'])} "
                f"content_sha256={manifest['cache_content_sha256']}"
            )
        else:
            manifest = validate_cache(
                args.cache, args.package_sha256, args.closure_sha256
            )
            print(
                f"valid schema={manifest['schema']} "
                f"content_sha256={manifest['cache_content_sha256']}"
            )
        return 0
    except CacheError as error:
        print(f"ERROR: {error}")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
