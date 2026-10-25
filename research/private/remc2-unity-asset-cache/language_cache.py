"""MC2 installed language records and source bitmap-font bindings.

Language files have a bitmap prefix followed by 471 NUL-terminated records.
They are not the game's separate tutorial sentinel streams. Text stays bytes;
the source glyph mapping, rather than a host Unicode font, renders those bytes.
"""
from __future__ import annotations

import hashlib
import json
import re
import struct
from pathlib import Path
from typing import Any, Callable, NamedTuple

SCHEMA = "mc2-installed-language-cache/v1"
SOURCE_COMMIT = "6806c7204d17220ecc0430759f784aea8ee2ddb2"
PREFIX_BYTES = 4785
RECORD_COUNT = 471
FONT_NAMES = ("FONT0", "FONT1", "FONT2", "HFONT3")
PACK_NAME = re.compile(r"[LD][0-9]+\.TXT", re.IGNORECASE)


class LanguageCacheError(ValueError):
    pass


class ImportCodecs(NamedTuple):
    confined_source: Callable[..., Path]
    decode_container_blob: Callable[..., bytes]
    bitmap_records: Callable[..., Any]
    materialize_indexed_rle: Callable[..., Any]
    build_objective_rasters: Callable[..., Any]
    decoder_identity: dict


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def canonical(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()


def language_records(payload: bytes) -> list[bytes]:
    if len(payload) <= PREFIX_BYTES:
        raise LanguageCacheError("language pack is shorter than its bitmap/table prefix")
    pictures = list(struct.iter_unpack("<IBB", payload[4773:PREFIX_BYTES]))
    if not any(width and height for _, width, height in pictures) or any(
            bool(width) != bool(height) or offset >= 4773 for offset, width, height in pictures):
        raise LanguageCacheError("language bitmap/table prefix is malformed")
    records = []
    cursor = PREFIX_BYTES
    for index in range(RECORD_COUNT):
        end = payload.find(b"\0", cursor)
        if end < 0:
            raise LanguageCacheError(f"language record {index} has no NUL terminator")
        records.append(payload[cursor:end])
        cursor = end + 1
    return records


def installed_packs(owner: Path) -> list[dict]:
    """Group identical owner files; a numeric filename is not a language name."""
    root = owner.resolve(strict=True)
    directory = root / "LANGUAGE"
    grouped: dict[str, dict] = {}
    for path in sorted(directory.iterdir(), key=lambda item: item.name.upper()):
        if not PACK_NAME.fullmatch(path.name):
            continue
        if not path.is_file() or not path.resolve(strict=True).is_relative_to(root):
            raise LanguageCacheError("installed language pack is not a confined owner file")
        payload = path.read_bytes()
        records = language_records(payload)
        identity = digest(payload)
        row = grouped.setdefault(identity, dict(pack_sha256=identity, payload=payload,
            records=records, aliases=[]))
        row["aliases"].append(f"LANGUAGE/{path.name}")
    if not grouped:
        raise LanguageCacheError("no installed source language pack is available")
    return list(grouped.values())


def glyph_index(byte: int) -> int | None:
    if not 0 <= byte <= 255:
        raise LanguageCacheError("source text values must be bytes")
    if byte in (0, 13):
        return None
    return 33 if byte in (9, 32) else 34 if byte == 10 else byte + 1


def validate_glyphs(records: list[bytes], glyphs: list[dict], masks: bytes) -> None:
    for index, row in enumerate(glyphs):
        if row["index"] != index or row["length"] != row["width"] * row["height"] or row["offset"] < 0 or \
                row["offset"] + row["length"] > len(masks):
            raise LanguageCacheError(f"source glyph {index} has an invalid mask extent")
        if digest(masks[row["offset"]:row["offset"] + row["length"]]) != row["sha256"]:
            raise LanguageCacheError(f"source glyph {index} mask identity mismatch")
    if any(value not in (0, 1) for value in masks):
        raise LanguageCacheError("source font masks must be binary")
    for selector, record in enumerate(records):
        for byte in record:
            index = glyph_index(byte)
            if index is not None and index >= len(glyphs):
                raise LanguageCacheError(f"language record {selector} requires missing glyph {index}")


def locale_identity(pack_sha256: str, text_sha256: str, prefix_sha256: str,
                    fonts: list[dict], payloads: dict[str, bytes]) -> str:
    return "mc2-locale-" + digest(canonical(dict(pack_sha256=pack_sha256,
        text_sha256=text_sha256, prefix_sha256=prefix_sha256,
        glyph_mapping="mc2-byte-plus-one-controls/v1",
        fonts=[dict(id=font["id"], glyphs=font["glyphs"],
                    masks_sha256=digest(payloads[font["masks_path"]])) for font in fonts])))


def build(owner: Path, output: Path, codecs: ImportCodecs, *, rnc_decoder: Path, work: Path) -> dict:
    """Use the existing owner's decoder; never compile a second native tool."""
    cache = codecs
    owner = owner.resolve(strict=True)
    if output.exists():
        raise LanguageCacheError("language cache output already exists")
    packs = installed_packs(owner)
    for pack in packs:
        for offset, width, height in codecs.bitmap_records(pack["payload"][4773:PREFIX_BYTES]):
            codecs.materialize_indexed_rle(pack["payload"][:4773], offset, width, height)
    payloads: dict[str, bytes] = {}
    sources = []
    fonts = []
    def source(relative: str) -> bytes:
        raw = cache.confined_source(owner, relative).read_bytes()
        sources.append(dict(path=relative, bytes=len(raw), sha256=digest(raw)))
        if not raw.startswith(b"RNC"):
            return raw
        if len(raw) < 18 or raw[:4] != b"RNC\x01":
            raise LanguageCacheError(f"unsupported packed source resource: {relative}")
        expected = int.from_bytes(raw[4:8], "big")
        if expected > 4_194_304:
            raise LanguageCacheError("source language resource exceeds its bounded size")
        return cache.decode_container_blob(rnc_decoder, raw, 0, expected, work,
            "language-" + Path(relative).name.lower().replace(".", "-"))

    for name in FONT_NAMES:
        decoded = {}
        for extension in ("DAT", "TAB"):
            relative = f"DATA/{name}.{extension}"
            decoded[extension] = source(relative)
        glyphs = []
        masks = bytearray()
        for index, (offset, width, height) in enumerate(cache.bitmap_records(decoded["TAB"])):
            _, mask, _ = cache.materialize_indexed_rle(decoded["DAT"], offset, width, height)
            glyphs.append(dict(index=index, offset=len(masks), length=len(mask),
                width=width, height=height, sha256=digest(mask)))
            masks.extend(mask)
        for pack in packs:
            validate_glyphs(pack["records"], glyphs, bytes(masks))
        relative = f"fonts/{name.lower()}.masks.u8"
        payloads[relative] = bytes(masks)
        fonts.append(dict(id=name.lower(), masks_path=relative, glyphs=glyphs,
            decoded_dat_sha256=digest(decoded["DAT"]), decoded_tab_sha256=digest(decoded["TAB"])))

    sprite_data = source("DATA/HSPRN0-0.DAT")
    sprite_table = cache.bitmap_records(source("DATA/HSPRN0-0.TAB"))
    sprites = {index: (*cache.materialize_indexed_rle(sprite_data, *sprite_table[index])[:2],
                       *sprite_table[index][1:]) for index in (171, 172, 173)}
    clrn, palette = source("DATA/CLRN-0.DAT"), source("DATA/PALN-0.DAT")
    if len(clrn) != 4096 or len(palette) != 768 or max(palette) > 63:
        raise LanguageCacheError("source objective palette binding is malformed")
    hfont = next(font for font in fonts if font["id"] == "hfont3")
    locales = []
    for pack in packs:
        blob = bytearray()
        rows = []
        for selector, record in enumerate(pack["records"]):
            rows.append(dict(id=f"text.mc2.record.{selector}", selector=selector,
                offset=len(blob), bytes=len(record), sha256=digest(record)))
            blob.extend(record + b"\0")
        locale_id = locale_identity(pack["pack_sha256"], digest(bytes(blob)),
            digest(pack["payload"][:PREFIX_BYTES]), fonts, payloads)
        identity = locale_id.removeprefix("mc2-locale-")
        path = f"text/{identity}.records-nul.u8"
        payloads[path] = bytes(blob)
        prefix_path = f"text/{identity}.prefix.u8"
        payloads[prefix_path] = pack["payload"][:PREFIX_BYTES]
        objectives = []
        for selector in range(48, 54):
            foreground, _, geometry = cache.build_objective_rasters(pack["records"][selector],
                hfont["glyphs"], payloads[hfont["masks_path"]], sprites,
                bytes(component * 4 for component in palette), clrn[3840], 1)
            raster_path = f"text/{identity}.objective-{selector}.indexed-mask.u8"
            payloads[raster_path] = foreground
            objectives.append(dict(selector=selector, text_id=f"text.mc2.record.{selector}",
                text_sha256=digest(pack["records"][selector]), raster_path=raster_path,
                reference_x=geometry["objective_x"], reference_y=geometry["objective_y"],
                width=geometry["objective_width"], height=geometry["objective_height"]))
        locales.append(dict(id=locale_id, label="Installed source language", aliases=pack["aliases"],
            pack_sha256=pack["pack_sha256"], records_path=path, prefix_path=prefix_path,
            text_sha256=digest(bytes(blob)), records=rows, objectives=objectives,
            font_ids=[font["id"] for font in fonts],
            non_ascii_record_count=sum(any(value >= 128 for value in row) for row in pack["records"])))
    # The installed source default is L1 when present; aliases have no invented
    # ISO locale or translated label. Other filenames remain owner provenance.
    default = next((row["id"] for row in locales if any(alias.upper() == "LANGUAGE/L1.TXT"
        for alias in row["aliases"])), locales[0]["id"])
    for pack in packs:
        for relative in pack["aliases"]:
            sources.append(dict(path=relative, bytes=len(pack["payload"]), sha256=pack["pack_sha256"]))
    outputs = [dict(path=path, bytes=len(data), sha256=digest(data)) for path, data in sorted(payloads.items())]
    manifest = dict(schema=SCHEMA, source_commit=SOURCE_COMMIT, decoder=codecs.decoder_identity,
        default_locale_id=default, glyph_mapping="mc2-byte-plus-one-controls/v1",
        language_prefix_bytes=PREFIX_BYTES, language_record_count=RECORD_COUNT,
        sources=sources, fonts=fonts, locales=locales, outputs=outputs)
    manifest["binding_sha256"] = digest(canonical(manifest))
    # Complete validation precedes publication. A missing manifest can never be
    # discovered as an installed locale, even if an I/O failure leaves outputs.
    output.mkdir(parents=True, exist_ok=False)
    for relative, data in payloads.items():
        path = output / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    (output / "manifest.json").write_bytes(canonical(manifest) + b"\n")
    return manifest


def validate(root: Path) -> dict:
    try:
        return _validate(root)
    except (KeyError, IndexError, TypeError) as error:
        raise LanguageCacheError(f"language cache manifest is incomplete: {error}") from error


def _validate(root: Path) -> dict:
    root = root.resolve(strict=True)
    manifest_path = root / "manifest.json"
    if manifest_path.is_symlink():
        raise LanguageCacheError("language manifest must be a regular file")
    manifest = json.loads(manifest_path.read_bytes())
    binding = manifest.pop("binding_sha256", None)
    if manifest.get("schema") != SCHEMA or manifest.get("source_commit") != SOURCE_COMMIT or \
            manifest.get("glyph_mapping") != "mc2-byte-plus-one-controls/v1" or \
            manifest.get("language_prefix_bytes") != PREFIX_BYTES or \
            manifest.get("language_record_count") != RECORD_COUNT or digest(canonical(manifest)) != binding:
        raise LanguageCacheError("language cache binding identity mismatch")
    manifest["binding_sha256"] = binding
    payloads = {}
    for row in manifest["outputs"]:
        relative = Path(row["path"])
        if relative.is_absolute() or ".." in relative.parts:
            raise LanguageCacheError("language output path is not confined")
        path = root / relative
        if any((root / Path(*relative.parts[:i])).is_symlink() for i in range(1, len(relative.parts) + 1)):
            raise LanguageCacheError("language output must be a regular file")
        data = path.read_bytes()
        if len(data) != row["bytes"] or digest(data) != row["sha256"]:
            raise LanguageCacheError(f"language output identity mismatch: {relative}")
        if row["path"] in payloads:
            raise LanguageCacheError("language output identity is duplicated")
        payloads[row["path"]] = data
    if manifest["default_locale_id"] not in {row["id"] for row in manifest["locales"]}:
        raise LanguageCacheError("installed default locale is missing")
    fonts = {row["id"]: row for row in manifest["fonts"]}
    if len(fonts) != len(FONT_NAMES) or set(fonts) != {name.lower() for name in FONT_NAMES}:
        raise LanguageCacheError("source font resource set is incomplete")
    for locale in manifest["locales"]:
        blob = payloads[locale["records_path"]]
        records = []
        cursor = 0
        for selector, row in enumerate(locale["records"]):
            offset, size = row["offset"], row["bytes"]
            if row["selector"] != selector or row["id"] != f"text.mc2.record.{selector}" or \
                    offset != cursor or size < 0 or offset + size >= len(blob) or blob[offset + size] != 0:
                raise LanguageCacheError("language record extent is invalid")
            record = blob[offset:offset + size]
            if b"\0" in record or digest(record) != row["sha256"]:
                raise LanguageCacheError("language record identity mismatch")
            records.append(record)
            cursor = offset + size + 1
        if len(records) != RECORD_COUNT or cursor != len(blob) or digest(blob) != locale["text_sha256"]:
            raise LanguageCacheError("language record set is incomplete")
        prefix = payloads[locale["prefix_path"]]
        if len(prefix) != PREFIX_BYTES or locale["font_ids"] != [name.lower() for name in FONT_NAMES] or \
                locale["id"] != locale_identity(locale["pack_sha256"], digest(blob), digest(prefix),
                                                manifest["fonts"], payloads):
            raise LanguageCacheError("locale text/font binding identity mismatch")
        for font_id in locale["font_ids"]:
            font = fonts[font_id]
            validate_glyphs(records, font["glyphs"], payloads[font["masks_path"]])
        if [row["selector"] for row in locale["objectives"]] != list(range(48, 54)):
            raise LanguageCacheError("current objective resource set is incomplete")
        for objective in locale["objectives"]:
            selector = objective["selector"]
            width, height = objective["width"], objective["height"]
            raster = payloads[objective["raster_path"]]
            if objective["text_id"] != f"text.mc2.record.{selector}" or \
                    objective["text_sha256"] != digest(records[selector]) or \
                    min(width, height) <= 0 or width > 640 or height > 480 or \
                    any(value < 0 or value % 2 for value in (width, height,
                        objective["reference_x"], objective["reference_y"])) or \
                    len(raster) != width * height * 2 or \
                    any(value not in (0, 1) for value in raster[width * height:]):
                raise LanguageCacheError("objective text/font raster binding is malformed")
    return manifest
