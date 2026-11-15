#!/usr/bin/env python3
"""Decode the owner campaign map and bind its opening semantic missions.

This extends the existing owner asset pipeline with the previously absent
campaign-menu responsibility. It does not alter qualified gameplay caches.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

import build_cache as cache

SCHEMA = "mc2-owner-campaign-menu-cache/v3"
OPENING_LEVEL_IDS = (
    "level.magic_carpet_2.night_spire",
    "level.magic_carpet_2.payahandra_ruins",
    "level.magic_carpet_2.revealed_way",
    "level.magic_carpet_2.spider_cave",
)


def source_catalog() -> tuple[bytes, list[tuple[int, str, int]]]:
    path = cache.REPO / "research/private/remc2-sim-rs/crates/sim-core/src/campaign/backend/runtime_catalog.tsv"
    payload = path.read_bytes()
    rows = []
    for line in payload.decode("ascii").splitlines():
        if not line or line.startswith("#"):
            continue
        index, level_id, map_type = line.split("\t")
        if not re.fullmatch(r"level\.[a-z0-9_]+\.[a-z0-9_]+", level_id):
            raise cache.CacheError("source campaign catalog has an invalid semantic identity")
        rows.append((int(index), level_id, int(map_type)))
    if ([row[0] for row in rows] != [*range(25), *range(30, 35)]
            or tuple(row[1] for row in rows[:4]) != OPENING_LEVEL_IDS
            or len({row[1] for row in rows}) != 30
            or any(row[2] not in (0, 1, 2) for row in rows)):
        raise cache.CacheError("source campaign catalog does not bind the thirty source missions")
    return payload, rows


def source_missions(language: bytes) -> list[dict]:
    """Private MC2 selectors end here; the client receives semantic bindings."""
    names_source = cache.pinned_remc2_blob("remc2/engine/EventsFunctions.cpp").decode()
    names_body = names_source.split("const char* LevelsNames_D9204[61] = {", 1)[1].split("};", 1)[0]
    names = re.findall(r'"([^"\\]*)"', names_body)
    portal_blob = subprocess.run(
        ["git", "-C", str(cache.REMC2_GIT), "show", f"{cache.PINNED_REMC2_COMMIT}:remc2/engine/Type_MapScreenPortals_E17CC.cpp"],
        check=True, stdout=subprocess.PIPE, timeout=30,
    ).stdout
    if hashlib.sha256(portal_blob).hexdigest() != "fbf59d8c14616d665c221a53451002cbc1e8213784f86bcb934d374a472a9f5b":
        raise cache.CacheError("pinned campaign portal source changed")
    portal_source = portal_blob.decode()
    rows = re.findall(r"\{([^{}]+)\}", portal_source)
    portals = [[int(value.strip(), 0) for value in row.split(",")] for row in rows]
    records = cache.language_records(language)
    if len(names) != 61 or len(portals) != 26 or any(len(row) != 11 for row in portals):
        raise cache.CacheError("pinned campaign name/portal tables changed")
    secret_blob = subprocess.run(
        ["git", "-C", str(cache.REMC2_GIT), "show",
         f"{cache.PINNED_REMC2_COMMIT}:remc2/engine/Type_SecretMapScreenPortals_E2970.cpp"],
        check=True, stdout=subprocess.PIPE, timeout=30).stdout
    if hashlib.sha256(secret_blob).hexdigest() != "b8072edf2a27527340b721725edc5db75cde7c9c2759ae7218b08dbb1fe51cc7":
        raise cache.CacheError("pinned secret portal source changed")
    secret_source = secret_blob.decode()
    secrets = [[int(value.strip(), 0) for value in row.split(",")]
               for row in re.findall(r"\{([^{}]+)\}", secret_source)]
    if (len(secrets) != 6 or any(len(row) != 8 for row in secrets)
            or any(secrets[-1]) or [(row[1], row[2]) for row in secrets[:-1]]
            != [(4, 30), (7, 31), (11, 32), (17, 33), (19, 34)]):
        raise cache.CacheError("pinned secret portal associations changed")
    secret_by_index = {row[2]: row for row in secrets[:-1]}
    _, catalog = source_catalog()
    # PresentLevelDescription_80C30 selects 23 + zero-based level index.
    missions = []
    for index, level_id, map_type in catalog:
        secret = secret_by_index.get(index)
        origin = secret[1] if secret else index
        portal = portals[origin]
        missions.append(dict(level_id=level_id, name=names[index],
            name_source_bytes=names[index].encode("ascii").hex(),
            source_internal_index=index, source_ordinary_index=origin, is_secret=secret is not None,
            pause_profile=("day", "night", "cave")[map_type],
            briefing=records[23 + origin].decode("ascii") if records[23 + origin].isascii() else None,
            briefing_text_id=f"text.mc2.record.{23 + origin}",
            viewport_x=min(portal[1], 640), viewport_y=min(portal[2], 480),
            portal_x=secret[3] if secret else portal[5], portal_y=secret[4] if secret else portal[6],
            portal_width=40 if secret else portal[3], portal_height=40 if secret else portal[4]))
    return missions


def source_startup() -> dict:
    source_blob = subprocess.run(
        ["git", "-C", str(cache.REMC2_GIT), "show",
         f"{cache.PINNED_REMC2_COMMIT}:remc2/engine/MenusAndIntros.cpp"],
        check=True, stdout=subprocess.PIPE, timeout=30).stdout
    source = source_blob.decode()
    body = source.split("MainMenuAnimations_E1748x {{", 1)[1].split("}};", 1)[0]
    rows = [[int(value.strip(), 0) for value in row.split(",")]
            for row in re.findall(r"\{([^{}]+)\}", body)]
    if len(rows) != 4 or any(len(row) != 6 for row in rows):
        raise cache.CacheError("pinned startup animation table changed")
    buttons_body = source.split("str_E1BAC[10] = {", 1)[1].split("};", 1)[0]
    button_rows = re.findall(r"\{([^{}]+),\{[^{}]+\}\}", buttons_body)
    buttons = []
    for row in button_rows:
        values = [int(value.strip(), 0) for value in row.split(",")]
        if len(values) != 14:
            raise cache.CacheError("pinned startup button table changed")
        if not values[3]:
            break
        buttons.append(dict(source_action=values[0], x=values[3], y=values[4],
            width=values[5], height=values[6], text_selector=values[7],
            normal_selector=values[8], hover_selector=values[9], help_key=values[10]))
    if len(buttons) != 9:
        raise cache.CacheError("pinned startup button count changed")
    help_body = source.split("textBoxStr_E25DC[10] = {", 1)[1].split("};", 1)[0]
    help_rows = [[int(value.strip(), 0) for value in row.split(",")]
                 for row in re.findall(r"\{([^{}]+)\}", help_body)]
    if len(help_rows) != 10 or any(len(row) != 10 for row in help_rows) or any(help_rows[-1]):
        raise cache.CacheError("pinned startup help table changed")
    help_bindings = [dict(text_selector=row[0], x=row[1], y=row[2],
        line_x=row[3], line_y=row[4], line_end_x=row[6], line_end_y=row[7],
        help_key=row[9], border_selector=80, line_selector=89) for row in help_rows[:-1]]
    return dict(width=640, height=480, clock_hz=100, source_sha256=hashlib.sha256(source_blob).hexdigest(),
        background_path="startup-background.indices.u8", palette_path="startup-palette.rgb6",
        logo=dict(selector=66, x=185, y=232), buttons=buttons, help=help_bindings,
        animations=[dict(x=row[1], y=row[2], initial_selector=row[3],
            first_selector=row[4], last_selector=row[5], step_ticks=4) for row in rows])


def source_gameplay_help() -> bytes:
    """Bind source spell, objective-expiry and pause-menu help rows."""
    source = cache.pinned_remc2_blob("remc2/engine/engine_support.cpp")
    body = source.decode().split("type_array_str_E2A74 str_E2A74 {{", 1)[1].split("}};", 1)[0]
    rows = re.findall(r"^\{(0x[0-9A-Fa-f]+),\{([^{}]+)\},([^{}]+)\}", body, re.MULTILINE)
    if len(rows) != 105:
        raise cache.CacheError("pinned gameplay help table extent changed")
    bindings = []
    for index in (41, *range(72, 81), *range(86, 91)):
        kind, axes, tail = rows[index]
        values = [int(value.strip(), 0) for value in axes.split(",")]
        extra = [int(value.strip(), 0) for value in tail.split(",")]
        if len(values) != 5 or len(extra) != 6:
            raise cache.CacheError("pinned gameplay help row shape changed")
        kind = int(kind, 0)
        expected_kind = 5 if index == 41 else 7 if index <= 80 else 11
        if kind != expected_kind or (values[2] == 0) != (expected_kind == 11):
            raise cache.CacheError("pinned gameplay help row type or duration changed")
        bindings.append(dict(row=index, kind=kind, flags=values[0], text_record=values[1],
            duration=values[2], x=values[3], y=values[4], age_threshold=extra[3],
            priority=extra[4]))
    return (json.dumps(dict(schema="mc2-gameplay-help/v1", source_sha256=hashlib.sha256(source).hexdigest(),
        rows=bindings), sort_keys=True, separators=(",", ":")) + "\n").encode("ascii")


def source_campaign_controls() -> list[dict]:
    source = subprocess.run(["git", "-C", str(cache.REMC2_GIT), "show",
        f"{cache.PINNED_REMC2_COMMIT}:remc2/engine/MenusAndIntros.cpp"],
        check=True, stdout=subprocess.PIPE, timeout=30).stdout.decode()
    body = source.split("mapMenuButtons_E23E0[6] = {", 1)[1].split("};", 1)[0]
    result = []
    for row in re.findall(r"\{([^{}]+),\{[^{}]+\}\}", body):
        values = [int(value.strip(), 0) for value in row.split(",")]
        if len(values) != 14:
            raise cache.CacheError("pinned campaign control table changed")
        if not values[13]:
            break
        result.append(dict(source_action=values[0], x=values[3], y=values[4],
            width=values[5], height=values[6], text_selector=values[7],
            normal_selector=values[9], hover_selector=values[8], help_key=values[10]))
    if len(result) != 4:
        raise cache.CacheError("pinned campaign control count changed")
    return result


def source_map_motion() -> dict:
    source = subprocess.run(["git", "-C", str(cache.REMC2_GIT), "show",
        f"{cache.PINNED_REMC2_COMMIT}:remc2/engine/MenusAndIntros.cpp"],
        check=True, stdout=subprocess.PIPE, timeout=30).stdout.decode()
    body = source.split("x_BYTE_E26C8_str[16] = {", 1)[1].split("};", 1)[0]
    rows = [[int(value.strip(), 0) for value in row.split(",")]
            for row in re.findall(r"\{([^{}]+)\}", body)]
    if len(rows) != 16 or any(len(row) != 14 for row in rows) or any(rows[-1]):
        raise cache.CacheError("pinned campaign background animation table changed")
    signed = lambda value: value if value < 32768 else value - 65536
    background = [dict(x=row[2], y=row[3], first=row[4], last=row[5], selector=row[6],
        delay_seconds=row[7], phase=row[8], restart=row[9], sound_delay=signed(row[10]),
        sound=row[11], play_type=row[12], loops=signed(row[13])) for row in rows[:-1]]
    body = source.split("str_WORD_E20A4[12] = {", 1)[1].split("};", 1)[0]
    rows = re.findall(r"\{([^{}]+),\{([^{}]+)\},([^{}]+),\{([^{}]+)\}\}", body)
    decorations = []
    for header, movement, state, path in rows:
        header, movement, state, path = [[int(value.strip(), 0) for value in group.split(",")]
            for group in (header, movement, state, path)]
        if (len(header), len(movement), len(state), len(path)) != (13, 12, 3, 12):
            raise cache.CacheError("pinned dragon animation row changed")
        if not movement[2]:
            break
        decorations.append(dict(x=movement[2], y=movement[3], target_x=movement[4], target_y=movement[5],
            first=movement[6], last=movement[7], finish_first=movement[9], finish_last=movement[10],
            delay_seconds=movement[11], kind=state[0], frontier=state[2],
            start_sound=header[7], start_play_type=header[8], finish_sound=header[10], finish_play_type=header[11]))
    if len(decorations) != 11:
        raise cache.CacheError("pinned dragon animation table changed")
    return dict(background=background, decorations=decorations, presentation_hz=30,
        source_sha256=hashlib.sha256(source.encode()).hexdigest())


def source_map_border(data: bytes) -> tuple[bytes, bytes]:
    """Visible writes of sub_85CC3_draw_round_frame, including its source asymmetry."""
    indices, mask = bytearray(640 * 480), bytearray(640 * 480)
    cursor = 0
    left, bottom, right, bottom_right = 0, 306560, 640, 307200
    copied = skipped = 0
    def write(target: int, value: int) -> None:
        if 0 <= target < len(indices):
            indices[target], mask[target] = value, 1
    for remaining in range(240, 0, -1):
        while True:
            count = struct.unpack_from("<h", data, cursor)[0]
            cursor += 2
            if count > 0:
                copied = count
                pixels = data[cursor:cursor + count]
                if len(pixels) != count:
                    raise cache.CacheError("truncated source map border")
                for index, value in enumerate(pixels):
                    write(left + index, value); write(bottom + index, value)
                    write(right - index, value); write(bottom_right - index, value)
                cursor += count
                left += count; bottom += count; right -= count; bottom_right -= count
            elif count < 0:
                count = -count
                left += count; bottom += count; right -= count; bottom_right -= count
                skipped += copied + count
            else:
                break
        if remaining > 221:
            write(left, data[cursor - 3]); write(bottom, data[cursor - 3])
        left += -copied + 640 - skipped
        bottom += -copied - 640 - skipped
        right += skipped + copied + 640
        bottom_right += skipped + copied - 640
        skipped = 0
    return bytes(indices), bytes(mask)


def build(owner: Path, output: Path, *, rnc_decoder: Path | None = None) -> dict:
    owner = owner.resolve(strict=True)
    # Never replace a cache or any work. A fresh output is reviewable/reusable.
    output.mkdir(parents=True, exist_ok=False)
    source = cache.confined_source(owner, "DATA/SCREENS/HSCREEN0.DAT").read_bytes()
    outputs = []

    def write(name: str, payload: bytes) -> None:
        (output / name).write_bytes(payload)
        outputs.append(dict(path=name, bytes=len(payload), sha256=hashlib.sha256(payload).hexdigest()))

    def bitmap(data: bytes, table: list, palette: bytes, selector: int, path: str) -> dict:
        offset, width, height = table[selector]
        indices, mask, _ = cache.materialize_indexed_rle(data, offset, width, height)
        rgba = bytes(channel for index, visible in zip(indices, mask, strict=True)
                     for channel in (*[component * 4 for component in palette[index * 3:index * 3 + 3]],
                                     255 if visible else 0))
        write(path, rgba)
        return dict(selector=selector, path=path, width=width, height=height)

    with tempfile.TemporaryDirectory(prefix="mc-campaign-rnc-") as work:
        work = Path(work)
        if rnc_decoder is None:
            decoder, provenance = cache.compile_rnc_decoder(work)
        else:
            decoder, provenance = rnc_decoder.resolve(strict=True), cache.rnc_logical_identity()
        language_binding = cache.build_language_cache(owner, output / "language", rnc_decoder=decoder, work=work)
        default_locale = next(row for row in language_binding["locales"]
            if row["id"] == language_binding["default_locale_id"])
        source_language = next((alias for alias in default_locale["aliases"]
            if alias.upper() == "LANGUAGE/L1.TXT"), default_locale["aliases"][0])
        language = cache.confined_source(owner, source_language).read_bytes()
        # MainMenu_76FA0 -> sub_7A110_load_hscreen(8,4). The first
        # palette is raw; the following three reads are sequential RNC blocks.
        startup = source_startup()
        startup_palette = source[:768]
        if len(startup_palette) != 768 or max(startup_palette) > 63:
            raise cache.CacheError("startup palette is not RGB6")
        startup_background = cache.decode_container_blob(decoder, source, 0x300, 640 * 480,
            work, "startup-background")
        startup_sprites = cache.decode_container_blob(decoder, source, 0x29391, 224426,
            work, "startup-sprites")
        startup_table = cache.bitmap_records(cache.decode_container_blob(decoder, source,
            0x422D6, 666, work, "startup-table"))
        write(startup["background_path"], startup_background)
        write(startup["palette_path"], startup_palette)
        startup["bitmaps"] = [bitmap(startup_sprites, startup_table, startup_palette, selector,
            f"startup-{selector:03d}.rgba8") for selector, (_, width, height) in enumerate(startup_table)
            if width and height]
        # LanguageSettingDialog_779E0 loads HSCREEN recipe12, then draws
        # selector1 from the selected owner's own 4,785-byte language prefix.
        language_background = cache.decode_container_blob(decoder, source, 0x91C59,
            640 * 480, work, "language-screen-background")
        language_palette = source[0xB2947:0xB2947 + 768]
        if len(language_palette) != 768 or max(language_palette) > 63:
            raise cache.CacheError("source language screen palette is not RGB6")
        write("language-screen-background.indices.u8", language_background)
        write("language-screen-palette.rgb6", language_palette)
        language_screen = dict(width=640, height=480,
            background_path="language-screen-background.indices.u8",
            palette_path="language-screen-palette.rgb6",
            cursor=bitmap(startup_sprites, startup_table, language_palette, 37,
                "language-cursor-037.rgba8"), flags=[])
        language_outputs = {row["path"]: row for row in language_binding["outputs"]}
        for locale in language_binding["locales"]:
            locale_id = locale["id"]
            if not re.fullmatch(r"mc2-locale-[0-9a-f]{64}", locale_id):
                raise cache.CacheError("source language flag has an invalid locale identity")
            prefix_path = locale["prefix_path"]
            prefix = (output / "language" / prefix_path).read_bytes()
            receipt = language_outputs[prefix_path]
            if (len(prefix) != 4785 or receipt["bytes"] != len(prefix)
                    or receipt["sha256"] != hashlib.sha256(prefix).hexdigest()):
                raise cache.CacheError("source language flag prefix receipt mismatch")
            flag_table = cache.bitmap_records(prefix[4773:])
            if len(flag_table) != 2:
                raise cache.CacheError("source language flag table extent changed")
            offset, width, height = flag_table[1]
            indices, mask, _ = cache.materialize_indexed_rle(prefix[:4773], offset, width, height)
            if (not width or not height or len(indices) != width * height
                    or any(value not in (0, 1) for value in mask)):
                raise cache.CacheError("source language flag has invalid indexed planes")
            flag_prefix = "language-flag-" + locale_id.removeprefix("mc2-locale-")
            indices_path, mask_path = flag_prefix + ".indices.u8", flag_prefix + ".mask.u8"
            write(indices_path, bytes(indices))
            write(mask_path, bytes(mask))
            language_screen["flags"].append(dict(locale_id=locale_id, selector=1,
                width=width, height=height, indices_path=indices_path, mask_path=mask_path))
        # SetKeysDialog retains bank4 sprites but changes the screen/palette
        # through sub_7A110_load_hscreen case15.
        settings_indices = cache.decode_container_blob(decoder, source, 0x145210,
            640 * 480, work, "settings-background")
        settings_palette = source[0x163EFC:0x1641FC]
        if len(settings_palette) != 768 or max(settings_palette) > 63:
            raise cache.CacheError("settings palette is not RGB6")
        settings_rgba = bytes(channel for index in settings_indices
            for channel in (*[component * 4 for component in settings_palette[index * 3:index * 3 + 3]], 255))
        write("settings-background.rgba8", settings_rgba)
        settings = dict(width=640, height=480, background_path="settings-background.rgba8",
            bitmaps=[bitmap(startup_sprites, startup_table, settings_palette, selector,
                f"settings-{selector:03d}.rgba8") for selector in (107, 108, 110)])
        # sub_7A110_load_hscreen(8,4) loads its own coloured 7px font.
        # DrawText_7FB90 indexes this table by the original byte directly;
        # DATA/FONT1's byte+1 monochrome mapping is a different draw path.
        font_data = cache.decode_container_blob(decoder, source, 0x13ACCA, 6613, work, "startup-font")
        font_table_bytes = cache.decode_container_blob(decoder, source, 0x13B194, 1632,
            work, "startup-font-table")
        font_table = cache.bitmap_records(font_table_bytes)
        if len(font_table) != 272 or font_table[65][1:] != (7, 7):
            raise cache.CacheError("pinned startup font table changed")
        font_width, font_height = 16 * 8, 17 * 7
        font_rgba = bytearray(font_width * font_height * 4)
        glyphs = []
        for index, (offset, width, height) in enumerate(font_table):
            if width > 8 or height > 7:
                raise cache.CacheError("startup font exceeds its source atlas cell")
            indices, mask, _ = cache.materialize_indexed_rle(font_data, offset, width, height)
            x, y = index % 16 * 8, index // 16 * 7
            glyphs.append(dict(index=index, x=x, y=y, width=width, height=height))
            for local_y in range(height):
                for local_x in range(width):
                    pixel = local_y * width + local_x
                    if not mask[pixel]:
                        continue
                    palette_offset = indices[pixel] * 3
                    target = ((y + local_y) * font_width + x + local_x) * 4
                    font_rgba[target:target + 4] = bytes((*[channel * 4 for channel in
                        startup_palette[palette_offset:palette_offset + 3]], 255))
        font_path = "startup-font.rgba8"
        write(font_path, bytes(font_rgba))
        startup["font"] = dict(path=font_path, width=font_width, height=font_height,
            advance_x=7, advance_y=7, glyph_mapping="mc2-hscreen-bank4-byte-controls/v1", glyphs=glyphs,
            decoded_dat_sha256=hashlib.sha256(font_data).hexdigest(),
            decoded_tab_sha256=hashlib.sha256(font_table_bytes).hexdigest())
        def decoded_owner(relative: str) -> bytes:
            raw = cache.confined_source(owner, relative).read_bytes()
            return cache.decode_container_blob(decoder, raw, 0, int.from_bytes(raw[4:8], "big"),
                work, "pause-" + Path(relative).name.lower()) if raw.startswith(b"RNC\x01") else raw

        # The gameplay UI selects HFONT3 at its high-resolution draw boundary.
        # Retain white owner masks so each source CLRx colour remains selectable.
        pause_font_data = decoded_owner("DATA/HFONT3.DAT")
        pause_font_table = cache.bitmap_records(decoded_owner("DATA/HFONT3.TAB"))
        pause_font_width, pause_font_height = 160, 252
        pause_font_rgba = bytearray(pause_font_width * pause_font_height * 4)
        pause_glyphs = []
        for index, (offset, width, height) in enumerate(pause_font_table):
            if width > 10 or height > 14:
                raise cache.CacheError("source pause font exceeds its atlas cell")
            _, mask, _ = cache.materialize_indexed_rle(pause_font_data, offset, width, height)
            x, y = index % 16 * 10, index // 16 * 14
            pause_glyphs.append(dict(index=index, x=x, y=y, width=width, height=height))
            for local_y in range(height):
                for local_x in range(width):
                    if mask[local_y * width + local_x]:
                        target = ((y + local_y) * pause_font_width + x + local_x) * 4
                        pause_font_rgba[target:target + 4] = b"\xff\xff\xff\xff"
        pause_font_path = "pause-font.rgba8"
        write(pause_font_path, bytes(pause_font_rgba))
        pause = dict(font=dict(path=pause_font_path, width=pause_font_width, height=pause_font_height,
            advance_x=10, advance_y=14, glyph_mapping="mc2-byte-plus-one-controls/v1", glyphs=pause_glyphs),
            profiles=[])
        # Live gameplay objectives consume source indexed planes, not the
        # palette-baked pause sprites or the six pre-rendered Night strings.
        objective = dict(font_id="hfont3", table_offset=0x4000, profiles=[])
        for profile, letter, building in (("night", "N", [51, 17, 59]),
                ("day", "D", [170, 99, 13]), ("cave", "C", [51, 136, 59])):
            pause_data = decoded_owner(f"DATA/HSPR{letter}0-0.DAT")
            pause_table = cache.bitmap_records(decoded_owner(f"DATA/HSPR{letter}0-0.TAB"))
            pause_palette = decoded_owner(f"DATA/PAL{letter}-0.DAT")
            pause_colours = decoded_owner(f"DATA/CLR{letter}-0.DAT")
            pause_tables = decoded_owner(f"DATA/TABLES{letter}.DAT")
            palette_path = f"pause-{profile}-palette.rgb6"
            write(palette_path, pause_palette)
            if (len(pause_palette) != 768 or max(pause_palette) > 63
                    or len(pause_colours) != 4096 or len(pause_tables) != 83456):
                raise cache.CacheError(f"source {profile} objective palette/table extent changed")
            colours_path = f"objective-{profile}-colours.u8"
            tables_path = f"objective-{profile}-tables.u8"
            write(colours_path, pause_colours)
            write(tables_path, pause_tables)
            objective_sprites = []
            for selector in (171, 172, 173):
                offset, width, height = pause_table[selector]
                indices, mask, _ = cache.materialize_indexed_rle(pause_data, offset, width, height)
                if not width or not height or len(indices) != width * height or any(value not in (0, 1) for value in mask):
                    raise cache.CacheError(f"source {profile} objective sprite {selector} is invalid")
                indices_path = f"objective-{profile}-{selector:03d}.indices.u8"
                mask_path = f"objective-{profile}-{selector:03d}.mask.u8"
                write(indices_path, bytes(indices))
                write(mask_path, bytes(mask))
                objective_sprites.append(dict(selector=selector, width=width, height=height,
                    indices_path=indices_path, mask_path=mask_path))
            objective_profile = dict(id=profile, palette_path=palette_path,
                colours_path=colours_path, tables_path=tables_path, sprites=objective_sprites,
                pause_sprites=[])
            objective["profiles"].append(objective_profile)
            entries = []
            for selector in (85, 86, *range(168, 171), *range(174, 179), 257, 258):
                entry = bitmap(pause_data, pause_table, pause_palette, selector,
                    f"pause-{profile}-{selector:03d}.rgba8")
                offset, width, height = pause_table[selector]
                if 174 <= selector <= 178:
                    indices, mask, _ = cache.materialize_indexed_rle(pause_data, offset, width, height)
                    if (not width or not height or len(indices) != width * height or len(mask) != width * height
                            or any(value not in (0, 1) for value in mask)):
                        raise cache.CacheError(f"source {profile} pause sprite {selector} is invalid")
                    indices_path = f"pause-{profile}-{selector:03d}.indices.u8"
                    mask_path = f"pause-{profile}-{selector:03d}.mask.u8"
                    write(indices_path, bytes(indices))
                    write(mask_path, bytes(mask))
                    objective_profile["pause_sprites"].append(dict(selector=selector, width=width,
                        height=height, indices_path=indices_path, mask_path=mask_path))
                    shade = 0x4000 + 256 * pause_colours[0]
                    rgba = bytearray(width * height * 4)
                    for pixel, (index, visible) in enumerate(zip(indices, mask, strict=True)):
                        # ColorizeScreen runs twice over these opaque icons.
                        index = pause_tables[shade + pause_tables[shade + index]]
                        rgba[pixel * 4:pixel * 4 + 4] = bytes((*[channel * 4 for channel in
                            pause_palette[index * 3:index * 3 + 3]], 255 if visible else 0))
                    entry["disabled_path"] = f"pause-{profile}-{selector:03d}-disabled.rgba8"
                    write(entry["disabled_path"], bytes(rgba))
                entries.append(entry)
            pause["profiles"].append(dict(id=profile, palette_path=palette_path,
                colour_indices=[pause_colours[index] for index in (0, 240, 3840, 4095, 0x888)],
                building_colours=building, bitmaps=entries))
        # sub_7A110_load_hscreen(8, 6). Packed lengths are source reads;
        # decoded extents are the next heap offsets and the RNC headers.
        world = cache.decode_container_blob(decoder, source, 0xB2C47, 1280 * 960, work, "campaign-world")
        palette = cache.decode_container_blob(decoder, source, 0x13A9CA, 768, work, "campaign-palette")
        sprites = cache.decode_container_blob(decoder, source, 0x783BD, 301787, work, "campaign-sprites")
        table = cache.decode_container_blob(decoder, source, 0x91856, 1878, work, "campaign-table")
        if max(palette) > 63:
            raise cache.CacheError("campaign palette is not RGB6")
        write("world-map.indices.u8", world)
        write("palette.rgb6", palette)
        bindings = []
        portal_table = cache.bitmap_records(table)
        # Bank6 uses its own coloured font and shifts the bitmap table by one
        # before DrawText_7FB90. Reusing the bank4 title font changes both its
        # colours and 8x14 advances. Retain the actual source masks/indices for
        # the results sheet's palette-table shading over animated portals.
        campaign_font_data = cache.decode_container_blob(decoder, source,
            0x1641FC, 12754, work, "campaign-font")
        campaign_font_table_bytes = cache.decode_container_blob(decoder, source,
            0x1646BA, 1638, work, "campaign-font-table")
        campaign_font_table = cache.bitmap_records(campaign_font_table_bytes)
        if len(campaign_font_table) != 273 or campaign_font_table[66][1:] != (8, 14):
            raise cache.CacheError("pinned campaign font table changed")
        campaign_font_rgba = bytearray(160 * 252 * 4)
        campaign_glyphs = []
        for index, (offset, width, height) in enumerate(campaign_font_table):
            if width > 10 or height > 14:
                raise cache.CacheError("campaign font exceeds its source atlas cell")
            indices, mask, _ = cache.materialize_indexed_rle(campaign_font_data, offset, width, height)
            x, y = index % 16 * 10, index // 16 * 14
            campaign_glyphs.append(dict(index=index, x=x, y=y, width=width, height=height))
            for local_y in range(height):
                for local_x in range(width):
                    pixel = local_y * width + local_x
                    if mask[pixel]:
                        target = ((y + local_y) * 160 + x + local_x) * 4
                        campaign_font_rgba[target:target + 4] = bytes((*[channel * 4 for channel in
                            palette[indices[pixel] * 3:indices[pixel] * 3 + 3]], 255))
        write("campaign-font.rgba8", bytes(campaign_font_rgba))
        campaign_font = dict(path="campaign-font.rgba8", width=160, height=252,
            advance_x=8, advance_y=14, glyph_mapping="mc2-hscreen-bank6-byte-plus-one/v1",
            glyphs=campaign_glyphs, decoded_dat_sha256=hashlib.sha256(campaign_font_data).hexdigest(),
            decoded_tab_sha256=hashlib.sha256(campaign_font_table_bytes).hexdigest())
        campaign_colours = cache.decode_container_blob(decoder, source, 0x13B3B8,
            16384, work, "campaign-colours")
        write("campaign-colours.u8", campaign_colours)
        motion = source_map_motion()
        border = cache.decode_container_blob(decoder, source, 0x141E85, 24523, work, "campaign-border")
        border_indices, border_mask = source_map_border(border)
        write("map-border.indices.u8", border_indices)
        write("map-border.mask.u8", border_mask)
        motion.update(border_indices_path="map-border.indices.u8", border_mask_path="map-border.mask.u8")
        indexed_portals = []
        # DrawFrameAnim increments a selector equal to endIndex before drawing,
        # then wraps on the following source interval.
        for selector in (*range(1, 152), *range(206, 212), *range(270, 274), *range(277, 285), *range(297, 313)):
            offset, width, height = portal_table[selector]
            indices, mask, _ = cache.materialize_indexed_rle(sprites, offset, width, height)
            indices_path, mask_path = f"map-{selector:03d}.indices.u8", f"map-{selector:03d}.mask.u8"
            write(indices_path, bytes(indices))
            write(mask_path, bytes(mask))
            indexed_portals.append(dict(selector=selector, width=width, height=height,
                indices_path=indices_path, mask_path=mask_path))
        dialog_font_data = decoded_owner("DATA/FONT1.DAT")
        dialog_font_table = cache.bitmap_records(decoded_owner("DATA/FONT1.TAB"))
        if len(dialog_font_table) != 273 or dialog_font_table[33][1:] != (4, 7):
            raise cache.CacheError("pinned campaign dialog font table changed")
        dialog_rgba = bytearray(80 * 126 * 4)
        dialog_glyphs = []
        for index, (offset, width, height) in enumerate(dialog_font_table):
            if width > 5 or height > 7:
                raise cache.CacheError("campaign dialog font exceeds its source atlas cell")
            _, mask, _ = cache.materialize_indexed_rle(dialog_font_data, offset, width, height)
            x, y = index % 16 * 5, index // 16 * 7
            dialog_glyphs.append(dict(index=index, x=x, y=y, width=width, height=height))
            for local_y in range(height):
                for local_x in range(width):
                    if mask[local_y * width + local_x]:
                        target = ((y + local_y) * 80 + x + local_x) * 4
                        dialog_rgba[target:target + 4] = b"\xff\xff\xff\xff"
        write("campaign-dialog-font.rgba8", bytes(dialog_rgba))
        dialog_font = dict(path="campaign-dialog-font.rgba8", width=80, height=126,
            advance_x=4, advance_y=7, glyph_mapping="mc2-font1-byte-plus-one-controls/v1", glyphs=dialog_glyphs)
        campaign_presentation = dict(font=campaign_font, dialog_font=dialog_font, motion=motion,
            controls=source_campaign_controls(), colours_path="campaign-colours.u8",
            english_statistics=any(alias.upper() == "LANGUAGE/D2.TXT" for alias in default_locale["aliases"]),
            bitmaps=[bitmap(sprites, portal_table, palette, selector, f"map-{selector:03d}.rgba8")
                for selector in (*range(237, 240), *range(246, 269), 274, 275)], indexed_portals=indexed_portals)
        for kind, selector in [("completed", 37), ("available", 70)]:
            row = bitmap(sprites, portal_table, palette, selector, f"portal-{kind}.rgba8")
            bindings.append(dict(kind=kind, path=row["path"], width=row["width"], height=row["height"]))
        # DrawFrameAnim_7E5A0 advances before drawing at an eight-hundredth
        # gate, including end+1 for one interval before wrapping next time.
        portal_animation = dict(clock_hz=100, step_ticks=8, opening_first=70, opening_last=83,
            loop_first=33, loop_last=35, completed_first=37, completed_last=43,
            frames=[bitmap(sprites, portal_table, palette, selector, f"portal-{selector:03d}.rgba8")
                    for selector in (*range(33, 45), *range(70, 85))])
        sound = cache.confined_source(owner, "SOUND/SOUND.DAT").read_bytes()
        _, descriptor = cache.container_descriptor(sound, count_rows=6, selector=0, row=3,
            label="menu-sound-bank-0-quality-822")
        metadata = cache.decode_container_blob(decoder, sound, descriptor["metadata_offset"],
            descriptor["metadata_size"], work, "menu-sound-metadata")
        audio = cache.decode_container_blob(decoder, sound, descriptor["data_offset"],
            descriptor["data_size"], work, "menu-sound-data")
        records = cache.packed_audio_records(metadata, audio, expected_count=(len(metadata) - 32) // 32,
            label="menu-sound-bank-0")
        record = records[40]
        payload = audio[record["relative_data_offset"]:record["relative_data_offset"] + record["bytes"]]
        properties = cache.wav_properties(payload, "menu portal sample41")
        write("portal-opening-041.wav", payload)
        portal_animation["sound"] = dict(bank=0, id=41, path="portal-opening-041.wav", volume=127,
            pan=64, rate=100, if_not_playing=True, source_container_sha256=hashlib.sha256(sound).hexdigest(),
            source_record_sha256=record["payload_sha256"], source_filename=record["filename"], **properties)
        record = records[13]
        payload = audio[record["relative_data_offset"]:record["relative_data_offset"] + record["bytes"]]
        write("menu-click-014.wav", payload)
        click_sound = dict(bank=0, id=14, path="menu-click-014.wav", volume=127, pan=64, rate=100,
            if_not_playing=False, source_record_sha256=record["payload_sha256"],
            **cache.wav_properties(payload, "source menu click sample14"))
        sample_ids = {19}
        sample_ids.update(row["sound"] for row in motion["background"] if row["sound_delay"] != -1)
        sample_ids.update(row[key] for row in motion["decorations"] for key in ("start_sound", "finish_sound"))
        motion["sounds"] = []
        for sample_id in sorted(sample_ids):
            record = records[sample_id - 1]
            payload = audio[record["relative_data_offset"]:record["relative_data_offset"] + record["bytes"]]
            path = f"map-sample-{sample_id:03d}.wav"
            write(path, payload)
            motion["sounds"].append(dict(bank=0, id=sample_id, path=path, volume=127, pan=64, rate=100,
                if_not_playing=True, source_record_sha256=record["payload_sha256"],
                **cache.wav_properties(payload, f"source map sample{sample_id}")))
    catalog_payload, _ = source_catalog()
    write("source-runtime-catalog.tsv", catalog_payload)
    write("gameplay-help.json", source_gameplay_help())
    manifest = dict(schema=SCHEMA, source_commit=cache.PINNED_REMC2_COMMIT,
                    source_catalog_path="source-runtime-catalog.tsv", gameplay_help_path="gameplay-help.json",
                    source_hscreen_sha256=hashlib.sha256(source).hexdigest(),
                    source_language_sha256=hashlib.sha256(language).hexdigest(),
                    language_cache=dict(path="language", binding_sha256=language_binding["binding_sha256"],
                        default_locale_id=language_binding["default_locale_id"]),
                    source_calls=["sub_7A110_load_hscreen(8,6)", "DrawNetGameMapBackground_85C8B", "PresentLevelDescription_80C30"],
                    width=1280, height=960, view_width=640, view_height=480,
                    missions=source_missions(language), portals=bindings, outputs=outputs,
                    startup=startup, language_screen=language_screen, settings=settings, pause=pause,
                    objective=objective, portal_animation=portal_animation,
                    campaign=campaign_presentation, click_sound=click_sound,
                    decoder=provenance)
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n")
    return manifest


def extend_missions(owner: Path, base: Path, output: Path) -> dict:
    """Extend the legacy opening bindings without changing their four-mission schema."""
    manifest = json.loads((base / "manifest.json").read_text())
    if (manifest.get("schema") != "mc2-owner-campaign-menu-cache/v1"
            or manifest.get("source_commit") != cache.PINNED_REMC2_COMMIT):
        raise cache.CacheError("mission extension requires the existing owner map cache")
    language = cache.confined_source(owner.resolve(strict=True), "LANGUAGE/L1.TXT").read_bytes()
    if hashlib.sha256(language).hexdigest() != manifest.get("source_language_sha256"):
        raise cache.CacheError("campaign briefing source changed")
    fields = tuple(manifest["missions"][0])
    missions = [{key: row[key] for key in fields} for row in source_missions(language)[:4]]
    if manifest["missions"] != missions[:len(manifest["missions"])]:
        raise cache.CacheError("existing campaign mission bindings differ from source")
    payloads = []
    for row in manifest["outputs"]:
        relative = Path(row["path"])
        if relative.is_absolute() or ".." in relative.parts:
            raise cache.CacheError("invalid campaign output path")
        payload = (base / relative).read_bytes()
        if len(payload) != row["bytes"] or hashlib.sha256(payload).hexdigest() != row["sha256"]:
            raise cache.CacheError("existing campaign map output changed")
        payloads.append((relative, payload))
    output.mkdir(parents=True, exist_ok=False)
    for relative, payload in payloads:
        (output / relative).parent.mkdir(parents=True, exist_ok=True)
        (output / relative).write_bytes(payload)
    manifest["missions"] = missions
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n")
    return manifest


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--owner-root", type=Path, default=cache.DEFAULT_OWNER)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--rnc-decoder", type=Path)
    parser.add_argument("--base-cache", type=Path,
                        help="extend source missions without rebuilding existing map/language assets")
    args = parser.parse_args()
    try:
        result = (extend_missions(args.owner_root, args.base_cache, args.output)
                  if args.base_cache else build(args.owner_root, args.output, rnc_decoder=args.rnc_decoder))
    except (cache.CacheError, ValueError, OSError, subprocess.SubprocessError) as error:
        print(json.dumps(dict(schema="mc2-campaign-menu-import-result/v1", ok=False,
            error=dict(code="CAMPAIGN_MENU_IMPORT_FAILED", message=str(error)))), file=sys.stderr)
        raise SystemExit(2)
    print(json.dumps(dict(schema=result["schema"], output=str(args.output.resolve()),
                          missions=len(result["missions"]), outputs=len(result["outputs"]))))
