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
import subprocess
import sys
import tempfile
from pathlib import Path

import build_cache as cache

SCHEMA = "mc2-owner-campaign-menu-cache/v2"
LEVEL_IDS = (
    "level.magic_carpet_2.night_spire",
    "level.magic_carpet_2.payahandra_ruins",
    "level.magic_carpet_2.revealed_way",
    "level.magic_carpet_2.spider_cave",
)


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
    # PresentLevelDescription_80C30 selects 23 + zero-based level index.
    return [dict(level_id=level_id, name=names[index], name_source_bytes=names[index].encode("ascii").hex(),
                 pause_profile=("night", "day", "day", "cave")[index],
                 briefing=records[23 + index].decode("ascii") if records[23 + index].isascii() else None,
                 briefing_text_id=f"text.mc2.record.{23 + index}",
                 viewport_x=portals[index][1], viewport_y=portals[index][2],
                 portal_x=portals[index][5], portal_y=portals[index][6],
                 portal_width=portals[index][3], portal_height=portals[index][4])
            for index, level_id in enumerate(LEVEL_IDS)]


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
        for profile, letter, building in (("night", "N", [51, 17, 59]),
                ("day", "D", [170, 99, 13]), ("cave", "C", [51, 136, 59])):
            pause_data = decoded_owner(f"DATA/HSPR{letter}0-0.DAT")
            pause_table = cache.bitmap_records(decoded_owner(f"DATA/HSPR{letter}0-0.TAB"))
            pause_palette = decoded_owner(f"DATA/PAL{letter}-0.DAT")
            pause_colours = decoded_owner(f"DATA/CLR{letter}-0.DAT")
            pause_tables = decoded_owner(f"DATA/TABLES{letter}.DAT")
            palette_path = f"pause-{profile}-palette.rgb6"
            write(palette_path, pause_palette)
            entries = []
            for selector in (*range(168, 171), *range(174, 179), 257, 258):
                entry = bitmap(pause_data, pause_table, pause_palette, selector,
                    f"pause-{profile}-{selector:03d}.rgba8")
                offset, width, height = pause_table[selector]
                if 174 <= selector <= 178:
                    indices, mask, _ = cache.materialize_indexed_rle(pause_data, offset, width, height)
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
    manifest = dict(schema=SCHEMA, source_commit=cache.PINNED_REMC2_COMMIT,
                    source_hscreen_sha256=hashlib.sha256(source).hexdigest(),
                    source_language_sha256=hashlib.sha256(language).hexdigest(),
                    language_cache=dict(path="language", binding_sha256=language_binding["binding_sha256"],
                        default_locale_id=language_binding["default_locale_id"]),
                    source_calls=["sub_7A110_load_hscreen(8,6)", "DrawNetGameMapBackground_85C8B", "PresentLevelDescription_80C30"],
                    width=1280, height=960, view_width=640, view_height=480,
                    missions=source_missions(language), portals=bindings, outputs=outputs,
                    startup=startup, pause=pause, portal_animation=portal_animation,
                    decoder=provenance)
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n")
    return manifest


def extend_missions(owner: Path, base: Path, output: Path) -> dict:
    """Retain validated map pixels while extending their source mission bindings."""
    manifest = json.loads((base / "manifest.json").read_text())
    if (manifest.get("schema") != "mc2-owner-campaign-menu-cache/v1"
            or manifest.get("source_commit") != cache.PINNED_REMC2_COMMIT):
        raise cache.CacheError("mission extension requires the existing owner map cache")
    language = cache.confined_source(owner.resolve(strict=True), "LANGUAGE/L1.TXT").read_bytes()
    if hashlib.sha256(language).hexdigest() != manifest.get("source_language_sha256"):
        raise cache.CacheError("campaign briefing source changed")
    fields = tuple(manifest["missions"][0])
    missions = [{key: row[key] for key in fields} for row in source_missions(language)]
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
