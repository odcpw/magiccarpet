"""Package-selected original material banks and their immutable cache products.

Import codecs are passed explicitly from the existing owner importer. No game
callbacks or gameplay admission are implemented in this module.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import shutil
import struct
import tempfile
from typing import Any, Callable, NamedTuple


SCHEMA = "mc2-environment-material-cache/v1"
DESCRIPTOR_SCHEMA = "mc2-environment-materials/v1"


class EnvironmentMaterialError(ValueError):
    pass


class ImportCodecs(NamedTuple):
    closure: Callable[..., dict[str, Any]]
    write: Callable[..., dict[str, Any]]
    compile_rnc: Callable[..., Any]
    decode_blob: Callable[..., bytes]
    bitmap_records: Callable[..., Any]
    decode_bitmap: Callable[..., Any]
    proven_bitmap: Callable[..., Any]
    fsync: Callable[..., None]
    output_path: Callable[..., Path]


def canonical(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False) + "\n").encode("utf-8")


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def content_identity(outputs: list[dict[str, Any]]) -> str:
    return digest("".join(f"{row['path']}\0{row['sha256']}\0{row['bytes']}\n" for row in sorted(outputs, key=lambda row: row["path"])).encode("utf-8"))


def build(owner_root: Path, package: Path, catalog_path: Path, output: Path, codecs: ImportCodecs) -> dict[str, Any]:
    owner_root = owner_root.expanduser().resolve(strict=True)
    package = package.expanduser().resolve(strict=True)
    output = codecs.output_path(output)
    # A live generation's cache is immutable. Never replace an existing tree.
    if output.exists():
        raise EnvironmentMaterialError("environment cache output already exists")
    closure = codecs.closure(package, catalog_path, owner_root)
    profile = closure["profile"]
    catalog = json.loads(catalog_path.read_bytes())
    entries = {row["asset_id"]: row for row in catalog["entries"]}
    sources = {}
    for row in closure["owner_files"]:
        path = owner_root / row["path"]
        if path.is_symlink() or not path.resolve(strict=True).is_relative_to(owner_root):
            raise EnvironmentMaterialError("environment source escapes owner root")
        data = path.read_bytes()
        if len(data) != row["size"] or "sha256:" + digest(data) != row["content_address"]:
            raise EnvironmentMaterialError(f"owner source changed: {row['path']}")
        sources[row["path"]] = data
    output.parent.mkdir(parents=True, exist_ok=True)
    stage = Path(tempfile.mkdtemp(prefix="environment-material-", dir=output.parent))
    outputs: list[dict[str, Any]] = []

    def emit(path: str, data: bytes, kind: str, asset: str, **metadata: Any) -> dict[str, Any]:
        return codecs.write(stage, outputs, path, data, kind=kind, source_asset_ids=[asset], extra=metadata or None)

    def raw(bank: str) -> bytes:
        return sources[f"DATA/{bank}.DAT"]

    palette_id = f"owner_data:DATA/{profile['palette']}.DAT"
    palette = raw(profile["palette"])
    if len(palette) != 768 or max(palette) > 63:
        raise EnvironmentMaterialError("selected palette is not 256 RGB6 triplets")
    palette_row = emit("palette/source.rgb6", palette, "ordered_rgb6_palette", palette_id)
    rgb = emit("palette/source.rgb8", bytes(value * 4 for value in palette), "ordered_rgb8_palette_exact_vga_scale", palette_id)
    tables = raw(profile["tables"])
    colors = raw(profile["colors"])
    if len(tables) != 83_456 or len(colors) != 4_096:
        raise EnvironmentMaterialError("selected shade/blend or colour table extent differs")
    tables_row = emit("palette/tables.u8", tables, "original_renderer_tables_u8", f"owner_data:DATA/{profile['tables']}.DAT")
    colors_row = emit("palette/colors.u8", colors, "original_rgb444_lookup_u8", f"owner_data:DATA/{profile['colors']}.DAT")
    atlas = raw(profile["blocks"])
    if len(atlas) != 152 * 32 * 32:
        raise EnvironmentMaterialError("selected source32 block atlas extent differs")
    atlas_row = emit("terrain/blocks.atlas.u8", atlas, "indexed_atlas_u8", f"owner_data:DATA/{profile['blocks']}.DAT", width=256, height=608)
    variant = emit("terrain/source16.dat", raw(profile["dormant_blocks16"]), "dormant_original_texture16_bank", f"owner_data:DATA/{profile['dormant_blocks16']}.DAT")
    sky_row = None
    if profile["sky"] is not None:
        sky = raw(profile["sky"])
        if len(sky) != 65_536:
            raise EnvironmentMaterialError("selected source32 sky extent differs")
        sky_row = emit("sky/source.u8", sky, "indexed_image_u8", f"owner_data:DATA/{profile['sky']}.DAT", width=256, height=256)
    exit_tables = None
    if profile["map_type"]["id"] == 0:
        exit_tables = emit("palette/day-exit-gtd2.dat", raw("GTD2"), "original_day_exit_tables", "owner_data:DATA/GTD2.DAT")
    scratch = stage / "decoder"
    scratch.mkdir()
    decoder, decoder_identity = codecs.compile_rnc(scratch)
    bank = profile["tmaps"]
    tmap_data = raw(bank)
    tab = sources[f"DATA/{bank}.TAB"]
    if len(tab) != 505 * 10:
        raise EnvironmentMaterialError("selected TMAP table requires 504 rows and adjacent sentinel")
    rows = list(struct.iter_unpack("<iiH", tab))
    textures = []
    for index, (decoded_size, offset, group) in enumerate(rows[:-1]):
        end = rows[index + 1][1]
        asset_id = f"owner_data:DATA/{bank}.DAT#{index}"
        entry = entries.get(asset_id, {})
        if not 0 <= offset < end <= len(tmap_data) or entry.get("offset") != offset or entry.get("end_offset") != end or entry.get("metadata_dword_0") != decoded_size or entry.get("animation_or_allocation_group") != group or entry.get("packed_span_sha256") != digest(tmap_data[offset:end]):
            raise EnvironmentMaterialError(f"TMAP adjacent span/catalog mismatch: {asset_id}")
        packed = tmap_data[offset:end]
        decoded = codecs.decode_blob(decoder, packed, 0, decoded_size, scratch, f"tmap-{index}")
        if decoded == b"0":
            payload = emit(f"textures/{index:03d}.decoded", decoded, "original_empty_tmap_payload", asset_id)
            textures.append({"index": index, "group": group, "source_empty": True,
                "decoded_payload_path": payload["path"], "source_asset_id": asset_id})
            continue
        if len(decoded) < 6:
            raise EnvironmentMaterialError(f"TMAP header truncated: {asset_id}")
        header, width, height = struct.unpack_from("<HHH", decoded)
        frame_end = 6 + width * height
        if not width or not height or frame_end > len(decoded):
            raise EnvironmentMaterialError(f"TMAP dimensions exceed decoded payload: {asset_id}")
        frame_count = 1
        if header & 1:
            if frame_end + 2 > len(decoded):
                raise EnvironmentMaterialError(f"animated TMAP has no frame stream: {asset_id}")
            frame_count = struct.unpack_from("<H", decoded, frame_end)[0]
            if not frame_count:
                raise EnvironmentMaterialError(f"animated TMAP has zero frames: {asset_id}")
        elif frame_end != len(decoded):
            raise EnvironmentMaterialError(f"static TMAP has trailing bytes: {asset_id}")
        payload = emit(f"textures/{index:03d}.decoded", decoded, "original_tmap_decoded_payload", asset_id)
        pixels = emit(f"textures/{index:03d}.indices.u8", decoded[6:frame_end], "indexed_image_u8", asset_id)
        textures.append({"index": index, "group": group, "width": width, "height": height, "animated": bool(header & 1), "frame_count": frame_count, "decoded_payload_path": payload["path"], "indices_path": pixels["path"], "source_asset_id": asset_id})
    shutil.rmtree(scratch)  # Known disposable codec scratch, never imported data.
    bitmap_banks = []
    proven_bitmap = codecs.proven_bitmap()
    for bank in (profile["sprites"], profile["web"], "FONT1"):
        data = raw(bank)
        records = codecs.bitmap_records(sources[f"DATA/{bank}.TAB"])
        bitmaps = []
        for selector, (offset, width, height) in enumerate(records):
            row = {"selector": selector, "width": width, "height": height}
            if not width or not height:
                row["source_empty"] = True
            else:
                asset = f"owner_data:DATA/{bank}.DAT#{selector}"
                pixels, mask, width, height, _ = codecs.decode_bitmap(data, records, selector, entries.get(asset, {}), proven_bitmap, bank)
                row["indices_path"] = emit(f"bitmaps/{bank.lower()}/{selector:03d}.indices.u8", pixels, "indexed_bitmap_u8", asset)["path"]
                row["mask_path"] = emit(f"bitmaps/{bank.lower()}/{selector:03d}.mask.u8", mask, "source_opaque_coverage_u8", asset)["path"]
            bitmaps.append(row)
        bitmap_banks.append({"bank": bank, "bitmaps": bitmaps})
    descriptor = {
        "schema": DESCRIPTOR_SCHEMA, "profile": profile,
        "palette": {"path": palette_row["path"], "rgb8_path": rgb["path"]},
        "tables": {"path": tables_row["path"], "key_color_1": profile["key_color_1"], "key_color_2": profile["key_color_2"]},
        "colors": {"path": colors_row["path"]},
        "blocks": {"atlas_path": atlas_row["path"], "width": 256, "height": 608, "tile_size": 32, "tile_count": 152},
        "source16": {"path": variant["path"], "activation": "dormant-source-variant"},
        "sky": None if sky_row is None else {"path": sky_row["path"], "width": 256, "height": 256},
        "day_exit_tables": None if exit_tables is None else {"path": exit_tables["path"]},
        "tmaps": {"bank": profile["tmaps"], "textures": textures}, "bitmap_banks": bitmap_banks,
    }
    emit("materials/descriptor.json", canonical(descriptor), "canonical_environment_material_descriptor", "owner_data:DATA/" + profile["tables"] + ".DAT")
    emit("materials/closure.json", canonical(closure), "canonical_environment_asset_closure", "owner_data:DATA/" + profile["tmaps"] + ".DAT")
    manifest = {"schema": SCHEMA, "profile": profile, "closure_sha256": closure["closure_sha256"], "source_files": closure["owner_files"], "decoder": decoder_identity, "outputs": outputs, "cache_content_sha256": content_identity(outputs)}
    manifest["manifest_payload_sha256"] = digest(canonical(manifest))
    (stage / "manifest.json").write_bytes(canonical(manifest))
    validate(stage)
    codecs.fsync(stage)
    stage.rename(output)
    return manifest


def validate(root: Path) -> dict[str, Any]:
    manifest = json.loads((root / "manifest.json").read_bytes())
    unsigned = dict(manifest)
    claim = unsigned.pop("manifest_payload_sha256", None)
    if manifest.get("schema") != SCHEMA or claim != digest(canonical(unsigned)):
        raise EnvironmentMaterialError("environment manifest schema/metadata identity differs")
    outputs = manifest.get("outputs", [])
    if content_identity(outputs) != manifest.get("cache_content_sha256"):
        raise EnvironmentMaterialError("environment content identity differs")
    seen = set()
    for row in outputs:
        relative = Path(row["path"])
        path = root / relative
        if relative.is_absolute() or ".." in relative.parts or row["path"] in seen or path.is_symlink() or not path.resolve(strict=True).is_relative_to(root.resolve()):
            raise EnvironmentMaterialError("environment output path is invalid/duplicate")
        seen.add(row["path"])
        payload = path.read_bytes()
        if len(payload) != row["bytes"] or digest(payload) != row["sha256"]:
            raise EnvironmentMaterialError(f"environment output identity differs: {row['path']}")
    actual = {path.relative_to(root).as_posix() for path in root.rglob("*") if path.is_file()}
    if actual != seen | {"manifest.json"}:
        raise EnvironmentMaterialError("environment cache has unlisted or missing outputs")
    descriptor = json.loads((root / "materials/descriptor.json").read_bytes())
    if descriptor.get("schema") != DESCRIPTOR_SCHEMA or descriptor.get("profile") != manifest["profile"]:
        raise EnvironmentMaterialError("environment descriptor/profile identity differs")
    if (descriptor["sky"] is None) != (manifest["profile"]["map_type"]["id"] == 2):
        raise EnvironmentMaterialError("Cave sky policy differs from source")
    profile = manifest["profile"]
    map_id = profile["map_type"]["id"]
    letter = {0: "D", 1: "N", 2: "C"}[map_id]
    display = "F" if map_id == 1 and profile["terrain_graphics_flags"] & 2 else letter
    prefix = "M" if profile["vga_resolution_selector"] == 1 else "H"
    expected = {"palette": f"PAL{display}-0", "tables": f"TABLES{letter}",
        "colors": f"CLR{letter}-0", "blocks": "BLOCK32" if map_id == 0 else f"BL32{display}0-0",
        "dormant_blocks16": "BLOCK16" if map_id == 0 else f"BL16{display}0-0",
        "sky": None if map_id == 2 else f"SKY{letter}0-0", "tmaps": f"TMAPS{map_id}-0",
        "sprites": f"{prefix}SPR{letter}0-0", "web": f"{prefix}WEB{letter}0-0",
        "key_color_1": 0 if map_id == 1 else 254, "key_color_2": 0 if map_id == 0 else 255,
        "texture_size": 32}
    if profile["vga_resolution_selector"] not in (0, 1) or any(profile.get(key) != value for key, value in expected.items()):
        raise EnvironmentMaterialError("environment selectors differ from source")
    by_path = {row["path"]: row for row in outputs}
    def payload(path: str) -> bytes:
        if path not in by_path:
            raise EnvironmentMaterialError("environment descriptor references an unlisted output")
        return (root / path).read_bytes()
    source_ids = {row["asset_id"] for row in manifest["source_files"]}
    source_rows = {row["asset_id"]: row for row in manifest["source_files"]}
    for row in outputs:
        if not row["source_asset_ids"] or any(asset.split("#")[0] not in source_ids for asset in row["source_asset_ids"]):
            raise EnvironmentMaterialError("environment product references unretained source")
    raw_products = {"palette/source.rgb6": profile["palette"], "palette/tables.u8": profile["tables"],
        "palette/colors.u8": profile["colors"], "terrain/blocks.atlas.u8": profile["blocks"],
        "terrain/source16.dat": profile["dormant_blocks16"]}
    if profile["sky"] is not None:
        raw_products["sky/source.u8"] = profile["sky"]
    if map_id == 0:
        raw_products["palette/day-exit-gtd2.dat"] = "GTD2"
    for path, bank in raw_products.items():
        source_id = f"owner_data:DATA/{bank}.DAT"
        row, source = by_path.get(path, {}), source_rows.get(source_id, {})
        if row.get("source_asset_ids") != [source_id] or source.get("content_address") != "sha256:" + row.get("sha256", "") or source.get("size") != row.get("bytes"):
            raise EnvironmentMaterialError(f"selected owner bank identity differs: {bank}")
    for path, extent in {"palette/source.rgb6": 768, "palette/source.rgb8": 768,
            "palette/tables.u8": 83456, "palette/colors.u8": 4096, "terrain/blocks.atlas.u8": 155648}.items():
        if by_path.get(path, {}).get("bytes") != extent:
            raise EnvironmentMaterialError(f"environment resource extent differs: {path}")
    six = (root / "palette/source.rgb6").read_bytes()
    if max(six) > 63 or bytes(value * 4 for value in six) != (root / "palette/source.rgb8").read_bytes():
        raise EnvironmentMaterialError("environment palette conversion differs")
    textures = descriptor["tmaps"]["textures"]
    if descriptor["tmaps"]["bank"] != profile["tmaps"] or [row["index"] for row in textures] != list(range(504)):
        raise EnvironmentMaterialError("environment TMAP table is incomplete")
    for row in textures:
        data = payload(row["decoded_payload_path"])
        if row.get("source_empty"):
            if data != b"0" or "indices_path" in row:
                raise EnvironmentMaterialError("environment empty TMAP differs")
        else:
            header, width, height = struct.unpack_from("<HHH", data)
            if width != row["width"] or height != row["height"] or not width * height or data[6:6 + width * height] != payload(row["indices_path"]):
                raise EnvironmentMaterialError("environment TMAP dimensions/pixels differ")
    if {bank["bank"] for bank in descriptor["bitmap_banks"]} != {profile["sprites"], profile["web"], "FONT1"}:
        raise EnvironmentMaterialError("environment bitmap banks differ")
    for bank in descriptor["bitmap_banks"]:
        if [row["selector"] for row in bank["bitmaps"]] != list(range(len(bank["bitmaps"]))):
            raise EnvironmentMaterialError("environment bitmap order differs")
        for row in bank["bitmaps"]:
            if row.get("source_empty"):
                if row["width"] and row["height"]:
                    raise EnvironmentMaterialError("nonempty bitmap marked empty")
                continue
            pixels = payload(row["indices_path"])
            mask = payload(row["mask_path"])
            if len(pixels) != row["width"] * row["height"] or len(mask) != len(pixels) or max(mask) > 1:
                raise EnvironmentMaterialError("environment bitmap extent/coverage differs")
    return manifest
