#!/usr/bin/env python3
"""Decode the owner campaign map and bind its first three semantic missions.

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
                 briefing=records[23 + index].decode("ascii") if records[23 + index].isascii() else None,
                 briefing_text_id=f"text.mc2.record.{23 + index}",
                 viewport_x=portals[index][1], viewport_y=portals[index][2],
                 portal_x=portals[index][5], portal_y=portals[index][6],
                 portal_width=portals[index][3], portal_height=portals[index][4])
            for index, level_id in enumerate(LEVEL_IDS)]


def build(owner: Path, output: Path, *, rnc_decoder: Path | None = None) -> dict:
    owner = owner.resolve(strict=True)
    # Never replace a cache or any work. A fresh output is reviewable/reusable.
    output.mkdir(parents=True, exist_ok=False)
    source = cache.confined_source(owner, "DATA/SCREENS/HSCREEN0.DAT").read_bytes()
    outputs = []

    def write(name: str, payload: bytes) -> None:
        (output / name).write_bytes(payload)
        outputs.append(dict(path=name, bytes=len(payload), sha256=hashlib.sha256(payload).hexdigest()))

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
        for kind, selector in [("completed", 37), ("available", 70)]:
            offset, width, height = cache.bitmap_records(table)[selector]
            indices, mask, _ = cache.materialize_indexed_rle(sprites, offset, width, height)
            rgba = bytes(channel for index, visible in zip(indices, mask, strict=True)
                         for channel in (*[component * 4 for component in palette[index * 3:index * 3 + 3]], 255 if visible else 0))
            path = f"portal-{kind}.rgba8"
            write(path, rgba)
            bindings.append(dict(kind=kind, path=path, width=width, height=height))
    manifest = dict(schema=SCHEMA, source_commit=cache.PINNED_REMC2_COMMIT,
                    source_hscreen_sha256=hashlib.sha256(source).hexdigest(),
                    source_language_sha256=hashlib.sha256(language).hexdigest(),
                    language_cache=dict(path="language", binding_sha256=language_binding["binding_sha256"],
                        default_locale_id=language_binding["default_locale_id"]),
                    source_calls=["sub_7A110_load_hscreen(8,6)", "DrawNetGameMapBackground_85C8B", "PresentLevelDescription_80C30"],
                    width=1280, height=960, view_width=640, view_height=480,
                    missions=source_missions(language), portals=bindings, outputs=outputs,
                    decoder=provenance)
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n")
    return manifest


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--owner-root", type=Path, default=cache.DEFAULT_OWNER)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--rnc-decoder", type=Path)
    args = parser.parse_args()
    try:
        result = build(args.owner_root, args.output, rnc_decoder=args.rnc_decoder)
    except (cache.CacheError, ValueError, OSError, subprocess.SubprocessError) as error:
        print(json.dumps(dict(schema="mc2-campaign-menu-import-result/v1", ok=False,
            error=dict(code="CAMPAIGN_MENU_IMPORT_FAILED", message=str(error)))), file=sys.stderr)
        raise SystemExit(2)
    print(json.dumps(dict(schema=result["schema"], output=str(args.output.resolve()),
                          missions=len(result["missions"]), outputs=len(result["outputs"]))))
