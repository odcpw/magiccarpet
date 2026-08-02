#!/usr/bin/env python3
"""Build a deterministic, private Level 1 asset-dependency closure.

This tool consumes the initialized Level 1 export, the qualified owner asset
catalog, and two independent byte-identical final-entry frame traces. It
deliberately distinguishes load roots, scene reachability, observed frame use,
armed-empty categories, and dynamic dependencies. Exact frame use comes only
from the qualified render/audio/text choke points.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
from pathlib import Path
import sys
from typing import Any, Iterable


TRACE_TOOL_ROOT = Path(__file__).resolve().parents[1] / "remc2-frame-trace"
sys.path.insert(0, str(TRACE_TOOL_ROOT))
from trace_format import parse_trace, qualify_trace  # noqa: E402


SCHEMA = "mc2-level-asset-closure/v2"
BASELINE_STATIC_CLOSURE_SHA256 = (
    "04e8b237b166161b57d4b86fc5c50a5f96ea32e4ec9b20404ac1d57e4615e751"
)
REMC2_SPEC_COMMIT = "6806c7204d17220ecc0430759f784aea8ee2ddb2"
FINAL_TRACE_EVENT_COUNT = 76
REALMS = {
    0: {"name": "day", "letter": "D", "tmaps": 0, "tables": "TABLESD"},
    1: {"name": "night", "letter": "N", "tmaps": 1, "tables": "TABLESN"},
    2: {"name": "cave", "letter": "C", "tmaps": 2, "tables": "TABLESC"},
}
BUILD_ENTITY_CLASS = 10
BUILD_ENTITY_MODEL = 45
ENTITY_RECORD_BYTES = 176
ENTITY_ACTIVE_OFFSET = 2
ENTITY_CLASS_OFFSET = 67
ENTITY_MODEL_OFFSET = 68
ENTITY_BUILD_INDEX_OFFSET = 74
ENTITY_PARTICLE_PARAMETER_OFFSET = 94
OBJECTIVE_INDEX_BASES = (
    0x30, 0x36, 0x3C, 0x42, 0x45, 0x48, 0x4D, 0x4F, 0x56, 0x5C,
    0x61, 0x66, 0x69, 0x6E, 0x73, 0x76, 0x7C, 0x7E, 0x83, 0x85,
    0x88, 0x8C, 0x8F, 0x97, 0x9C,
)
D4750 = (
    0x00, 0x00, 0x02, 0x03, 0x04, 0x05, 0x01, 0x00, 0x06, 0x07,
    0x04, 0x05, 0x00, 0x01, 0x01, 0x02, 0x02, 0x03, 0x03, 0x04,
    0x04, 0x03, 0x03, 0x02, 0x02, 0x01, 0x01, 0x00, 0x00, 0x00,
    0x00, 0x01, 0x01, 0x01, 0x02, 0x02, 0x02, 0x02, 0x02, 0x01,
    0x01, 0x01, 0x00, 0x00, 0x03, 0x03, 0x03, 0x03, 0x04, 0x05,
    0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0C, 0x0C, 0x0C,
)


class ClosureError(ValueError):
    """Input or join failure that makes a deterministic closure unsafe."""


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ClosureError(f"cannot read JSON {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise ClosureError(f"JSON root must be an object: {path}")
    return value


def select_realm_assets(
    map_type: int, vga_resolution_selector: int, texture_size: int, terrain_flags: int
) -> list[str]:
    """Return the exact owner-data roots selected by remc2's realm loaders."""
    if map_type not in REALMS:
        raise ClosureError(f"unsupported map type: {map_type}")
    if texture_size not in (16, 32):
        raise ClosureError(f"owner-data closure supports texture sizes 16/32, got {texture_size}")
    realm = REALMS[map_type]
    letter = realm["letter"]
    sprite_prefix = "M" if vga_resolution_selector == 1 else "H"

    if map_type == 0:
        block = f"BLOCK{texture_size}.DAT"
        palette = "PALD-0.DAT"
        colors = "CLRD-0.DAT"
    elif map_type == 1:
        display_letter = "F" if terrain_flags & 2 else "N"
        block = f"BL{texture_size}{display_letter}0-0.DAT"
        palette = f"PAL{display_letter}-0.DAT"
        colors = "CLRN-0.DAT"
    else:
        block = f"BL{texture_size}C0-0.DAT"
        palette = "PALC-0.DAT"
        colors = "CLRC-0.DAT"

    roots = [
        f"DATA/TMAPS{realm['tmaps']}-0.DAT",
        f"DATA/TMAPS{realm['tmaps']}-0.TAB",
        f"DATA/{block}",
        f"DATA/{palette}",
        f"DATA/{colors}",
        f"DATA/{realm['tables']}.DAT",
        f"DATA/{sprite_prefix}SPR{letter}0-0.DAT",
        f"DATA/{sprite_prefix}SPR{letter}0-0.TAB",
        f"DATA/{sprite_prefix}WEB{letter}0-0.DAT",
        f"DATA/{sprite_prefix}WEB{letter}0-0.TAB",
    ]
    if texture_size == 32 and map_type in (0, 1):
        roots.append(f"DATA/SKY{letter}0-0.DAT")
    return sorted(roots)


def tmap_render_strategy(mode: int) -> str:
    """Classify every render-mode branch in GameRenderOriginal.cpp."""
    if mode in (0, 1):
        return "base"
    if 2 <= mode <= 16:
        return "animation_frame"
    if mode == 17:
        return "yaw_mirrored_16"
    if mode == 18:
        return "yaw_16"
    if mode == 19:
        return "yaw_lookup_D4750_12"
    if mode == 20:
        return "yaw_lookup_D4750_28"
    if mode == 21:
        return "base_transparent"
    if 22 <= mode <= 36:
        return "animation_frame_transparent"
    raise ClosureError(f"unknown TMAP render mode: {mode}")


def render_choice_index(
    base: int,
    mode: int,
    group_indices: list[int],
    *,
    animation_frame: int = 0,
    yaw_sector: int = 0,
) -> int:
    """Evaluate remc2's final TMAP-row formula with explicit bounds."""
    strategy = tmap_render_strategy(mode)
    if not 0 <= base < 504:
        raise ClosureError(f"TMAP base index out of bounds: {base}")
    if not group_indices or sorted(set(group_indices)) != group_indices:
        raise ClosureError("TMAP allocation group must be non-empty, unique, and sorted")
    if strategy in {"base", "base_transparent"}:
        candidate = base
    elif strategy in {"animation_frame", "animation_frame_transparent"}:
        if animation_frame < 0:
            raise ClosureError("animation frame must be non-negative")
        candidate = base + animation_frame
    else:
        if not 0 <= yaw_sector < 16:
            raise ClosureError(f"yaw sector out of bounds: {yaw_sector}")
        if strategy == "yaw_mirrored_16":
            offset = yaw_sector if yaw_sector < 8 else 15 - yaw_sector
        elif strategy == "yaw_16":
            offset = yaw_sector
        elif strategy == "yaw_lookup_D4750_12":
            offset = D4750[12 + yaw_sector]
        elif strategy == "yaw_lookup_D4750_28":
            offset = D4750[28 + yaw_sector]
        else:  # pragma: no cover - tmap_render_strategy is exhaustive
            raise ClosureError(f"unhandled render strategy: {strategy}")
        candidate = base + offset
    if candidate not in group_indices:
        raise ClosureError(
            f"render choice {candidate} is outside allocation group {group_indices}"
        )
    return candidate


def render_choice_candidates(base: int, mode: int, group_indices: list[int]) -> list[int]:
    strategy = tmap_render_strategy(mode)
    if strategy in {"base", "base_transparent"}:
        choices = [render_choice_index(base, mode, group_indices)]
    elif strategy in {"animation_frame", "animation_frame_transparent"}:
        choices = [
            render_choice_index(
                base, mode, group_indices, animation_frame=index - base
            )
            for index in group_indices
        ]
    else:
        choices = [
            render_choice_index(base, mode, group_indices, yaw_sector=sector)
            for sector in range(16)
        ]
    return sorted(set(choices))


def verify_export_identity(export_dir: Path, manifest: dict[str, Any]) -> None:
    """Verify all package members and the package identity before consumption."""
    files = manifest.get("files")
    if not isinstance(files, dict) or not files:
        raise ClosureError("level export manifest has no files identity map")
    identity_rows = []
    for name in sorted(files):
        entry = files[name]
        path = export_dir / name
        if not path.is_file():
            raise ClosureError(f"level export package member is missing: {name}")
        actual_size = path.stat().st_size
        actual_hash = sha256_file(path)
        if actual_size != entry.get("bytes") or actual_hash != entry.get("sha256"):
            raise ClosureError(f"level export member identity mismatch: {name}")
        identity_rows.append(f"{name}\0{actual_hash}\0{actual_size}\n")
    package_hash = sha256_bytes("".join(identity_rows).encode("utf-8"))
    if package_hash != manifest.get("package_sha256"):
        raise ClosureError("level export package_sha256 does not match its members")
    for consumed in ("entities.bin", "presentation.bin", "runtime.bin"):
        if consumed not in files:
            raise ClosureError(f"consumed export member is undeclared: {consumed}")


def parse_presentation(path: Path) -> dict[str, Any]:
    data = path.read_bytes()
    if len(data) < 12 or data[:4] != b"MCV1":
        raise ClosureError("presentation.bin has a bad or missing MCV1 header")
    version, reserved, particle_count = struct.unpack_from("<HHI", data, 4)
    if version != 1 or reserved != 0 or particle_count != 347:
        raise ClosureError("presentation.bin header/count mismatch")
    offset = 12
    particle_size = 14
    required = offset + particle_count * particle_size + 4
    if len(data) < required:
        raise ClosureError("presentation.bin is truncated in particle parameters")
    particles = []
    for index in range(particle_count):
        values = struct.unpack_from("<HHHHHBbBB", data, offset + index * particle_size)
        particles.append(
            {
                "index": index,
                "base_tmap": values[0],
                "word_2": values[1],
                "word_4": values[2],
                "speed": values[3],
                "rotation_speed": values[4],
                "byte_10": values[5],
                "byte_11": values[6],
                "render_mode": values[7],
                "byte_13": values[8],
            }
        )
    offset += particle_count * particle_size
    selector_count = struct.unpack_from("<I", data, offset)[0]
    offset += 4
    if selector_count != 504 or len(data) < offset + selector_count + 4:
        raise ClosureError("presentation.bin level-sprite selector section is malformed")
    selectors = list(data[offset : offset + selector_count])
    offset += selector_count
    parameter_count = struct.unpack_from("<I", data, offset)[0]
    offset += 4
    if parameter_count != 157 or len(data) != offset + parameter_count * 34:
        raise ClosureError("presentation.bin entity-parameter section is malformed")
    return {"particles": particles, "level_sprite_selectors": selectors}


def parse_entities(path: Path) -> dict[str, Any]:
    data = path.read_bytes()
    if len(data) < 12 or data[:4] != b"MCE1":
        raise ClosureError("entities.bin has a bad or missing MCE1 header")
    version, record_size, count = struct.unpack_from("<HHI", data, 4)
    if version != 1 or record_size != ENTITY_RECORD_BYTES:
        raise ClosureError("entities.bin header/record-size mismatch")
    if len(data) != 12 + count * record_size:
        raise ClosureError("entities.bin length does not match its declared records")
    particle_parameters: set[int] = set()
    build_indices: set[int] = set()
    for index in range(count):
        record = data[12 + index * record_size : 12 + (index + 1) * record_size]
        if record[ENTITY_ACTIVE_OFFSET] == 0:
            continue
        particle_parameters.add(
            struct.unpack_from("<H", record, ENTITY_PARTICLE_PARAMETER_OFFSET)[0]
        )
        if (
            record[ENTITY_CLASS_OFFSET] == BUILD_ENTITY_CLASS
            and record[ENTITY_MODEL_OFFSET] == BUILD_ENTITY_MODEL
        ):
            build_indices.add(record[ENTITY_BUILD_INDEX_OFFSET])
    return {
        "active_particle_parameters": sorted(particle_parameters),
        "active_build_indices": sorted(build_indices),
    }


def parse_build_footprints(owner_root: Path, indices: Iterable[int]) -> list[dict[str, Any]]:
    """Parse BUILD as terrain footprints, never as sprite RLE."""
    dat_path = owner_root / "DATA/BUILD0-0.DAT"
    tab_path = owner_root / "DATA/BUILD0-0.TAB"
    dat = dat_path.read_bytes()
    tab = tab_path.read_bytes()
    if len(tab) % 6:
        raise ClosureError("BUILD0-0.TAB length is not a multiple of its 6-byte records")
    records = [
        struct.unpack_from("<IBB", tab, offset) for offset in range(0, len(tab), 6)
    ]
    previous = -1
    for offset, width, height in records:
        if offset < previous or offset > len(dat):
            raise ClosureError("BUILD0-0.TAB contains non-monotonic/out-of-range offsets")
        previous = offset
        if width * height * 2 > len(dat) - offset:
            raise ClosureError("BUILD footprint exceeds BUILD0-0.DAT")
    result = []
    for index in sorted(set(indices)):
        if index >= len(records):
            raise ClosureError(f"BUILD footprint index out of bounds: {index}")
        offset, width, height = records[index]
        end = records[index + 1][0] if index + 1 < len(records) else len(dat)
        expected = width * height * 2
        if end - offset != expected:
            raise ClosureError(
                f"BUILD footprint {index} span is {end-offset}, expected {expected}"
            )
        cells = dat[offset:end]
        result.append(
            {
                "asset_id": f"owner_data:DATA/BUILD0-0.DAT#footprint-{index}",
                "kind": "terrain_modification_footprint",
                "index": index,
                "width": width,
                "height": height,
                "cell_stride_bytes": 2,
                "span": {"offset": offset, "length": len(cells)},
                "content_address": f"sha256:{sha256_bytes(cells)}",
                "schema_evidence": {
                    "loader": "EventsFunctions.cpp:46537-46570; Basic.cpp:271-273",
                    "consumer": "EventsFunctions.cpp:27648-28032",
                    "cell_0": "terrain/shading control byte",
                    "cell_1":
                        "target height offset from construction base or 0xff sentinel",
                    "raw_dimensions": "BUILD TAB <IBB> width and height",
                    "loader_resolution_rule":
                        "CreateIndexes doubles indexed DAT/TAB dimensions when resolution bit 0 is set",
                    "consumer_resolution_rule":
                        "castle consumers halve runtime dimensions when VGA selector is 1",
                    "effective_dimensions":
                        "for Level 1 selector 1 the loader doubling and consumer halving cancel",
                },
            }
        )
    return result


def parse_language_record(owner_root: Path, selector: int) -> dict[str, Any]:
    path = owner_root / "LANGUAGE/L1.TXT"
    data = path.read_bytes()
    if len(data) <= 4785:
        raise ClosureError("LANGUAGE/L1.TXT is shorter than its 4785-byte graphics prefix")
    records = data[4785:].split(b"\0")
    if selector >= len(records):
        raise ClosureError(f"language selector out of bounds: {selector}")
    raw = records[selector]
    if not raw:
        raise ClosureError(f"language selector {selector} resolves to an empty record")
    if (raw[:1] == b'"') != (raw[-1:] == b'"'):
        raise ClosureError(f"language selector {selector} has unmatched quote bytes")
    return {
        "asset_id": f"owner_data:LANGUAGE/L1.TXT#string-{selector}",
        "kind": "localization_record",
        "selector": selector,
        "encoding": "owner_8_bit",
        "byte_length": len(raw),
        "content_address": f"sha256:{sha256_bytes(raw)}",
        "glyph_mapping": "non-space owner byte + 1",
        "glyph_indices": sorted({byte + 1 for byte in raw if byte != 0x20}),
    }


def objective_language_selector(
    runtime: bytes, objective_layout: dict[str, Any], level_external: int
) -> int:
    if not 1 <= level_external <= len(OBJECTIVE_INDEX_BASES):
        raise ClosureError(f"objective level index out of bounds: {level_external}")
    try:
        offset = objective_layout["offset"]
        count = objective_layout["count"]
        record_bytes = objective_layout["record_bytes"]
    except (KeyError, TypeError) as exc:
        raise ClosureError("objective runtime layout is malformed") from exc
    if (
        not isinstance(offset, int)
        or not isinstance(count, int)
        or not isinstance(record_bytes, int)
        or offset < 0
        or count < 1
        or record_bytes < 2
        or offset + count * record_bytes > len(runtime)
    ):
        raise ClosureError("objective runtime layout is out of bounds")
    return OBJECTIVE_INDEX_BASES[level_external - 1] + runtime[offset + 1]


def resolve_glyph_assets(catalog: "Catalog", record: dict[str, Any]) -> list[dict[str, Any]]:
    result = []
    for index in record.get("glyph_indices", []):
        if not isinstance(index, int) or not 1 <= index <= 256:
            raise ClosureError(f"FONT1 glyph index out of owner-byte bounds: {index}")
        result.append(catalog.indexed_asset("DATA/FONT1.DAT", index))
    return result


class Catalog:
    def __init__(self, data: dict[str, Any]) -> None:
        self.data = data
        self.files = {
            item["path"]: item
            for item in data.get("files", [])
            if item.get("asset_id", "").startswith("owner_data:")
        }
        self.entries = {
            item["asset_id"]: item
            for item in data.get("entries", [])
            if item.get("asset_id", "").startswith("owner_data:")
        }
        self.tmaps: dict[int, dict[str, Any]] = {}

    def file_asset(self, path: str) -> dict[str, Any]:
        if path not in self.files:
            raise ClosureError(f"owner catalog has no file: {path}")
        item = self.files[path]
        return {
            "asset_id": item["asset_id"],
            "kind": "catalog_file",
            "path": path,
            "category": item.get("category"),
            "size": item.get("size"),
            "content_address": f"sha256:{item['sha256']}",
        }

    def verify_owner_file(self, owner_root: Path, path: str) -> None:
        if path not in self.files:
            raise ClosureError(f"owner catalog has no file: {path}")
        actual_path = owner_root / path
        if not actual_path.is_file():
            raise ClosureError(f"owner root has no file: {actual_path}")
        expected = self.files[path]["sha256"]
        actual = sha256_file(actual_path)
        if actual != expected:
            raise ClosureError(
                f"owner file/catalog hash mismatch for {path}: {actual} != {expected}"
            )

    def index_tmaps(self, pack_path: str) -> None:
        rows = [
            item
            for item in self.entries.values()
            if item.get("pack_path") == pack_path
        ]
        self.tmaps = {item["index"]: item for item in rows}
        if sorted(self.tmaps) != list(range(504)):
            raise ClosureError(f"{pack_path} does not have a complete 0..503 catalog")

    def tmap_group(self, index: int) -> list[dict[str, Any]]:
        if index not in self.tmaps:
            raise ClosureError(f"TMAP index out of bounds: {index}")
        group = self.tmaps[index].get("animation_or_allocation_group")
        members = [
            item
            for item in self.tmaps.values()
            if item.get("animation_or_allocation_group") == group
        ]
        if not members:
            raise ClosureError(f"TMAP allocation group {group} is empty")
        return sorted(members, key=lambda item: item["index"])

    def tmap_asset(self, item: dict[str, Any]) -> dict[str, Any]:
        return {
            "asset_id": item["asset_id"],
            "kind": "catalog_entry",
            "path": item["pack_path"],
            "index": item["index"],
            "allocation_group": item.get("animation_or_allocation_group"),
            "content_address": item["content_address"],
        }

    def indexed_asset(self, path: str, index: int) -> dict[str, Any]:
        asset_id = f"owner_data:{path}#{index}"
        if asset_id not in self.entries:
            raise ClosureError(f"owner catalog has no indexed asset: {asset_id}")
        item = self.entries[asset_id]
        return {
            "asset_id": asset_id,
            "kind": "catalog_entry",
            "path": path,
            "index": index,
            "content_address": item["content_address"],
        }


def edge(
    source: str, target: str, join: str, source_field: str, status: str, evidence: str
) -> dict[str, str]:
    if status not in {"proven", "dynamic", "unresolved"}:
        raise ClosureError(f"invalid edge status: {status}")
    return {
        "from": source,
        "to": target,
        "join": join,
        "source_field": source_field,
        "status": status,
        "evidence": evidence,
    }


def unique_assets(values: Iterable[dict[str, Any]]) -> list[dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    for value in values:
        asset_id = value["asset_id"]
        if asset_id in result and result[asset_id] != value:
            raise ClosureError(f"conflicting descriptions for asset {asset_id}")
        result[asset_id] = value
    return [result[key] for key in sorted(result)]


def require_sha256(value: Any, field: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ClosureError(f"{field} is not a lowercase SHA-256 digest")
    return value


def load_qualified_trace_pair(
    trace_a_dir: Path,
    trace_b_dir: Path,
    export_manifest: dict[str, Any],
    catalog_data: dict[str, Any],
) -> dict[str, Any]:
    trace_a = trace_a_dir / "trace.jsonl"
    trace_b = trace_b_dir / "trace.jsonl"
    if not trace_a.is_file() or not trace_b.is_file():
        raise ClosureError("both final frame traces must exist")
    bytes_a = trace_a.read_bytes()
    bytes_b = trace_b.read_bytes()
    if bytes_a != bytes_b:
        raise ClosureError("final frame traces are not byte-identical")

    try:
        qualification_a = qualify_trace(
            trace_a, require_complete_categories=True
        )
        qualification_b = qualify_trace(
            trace_b, require_complete_categories=True
        )
        header, events = parse_trace(trace_a)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        raise ClosureError(f"final frame trace qualification failed: {exc}") from exc
    if qualification_a != qualification_b:
        raise ClosureError("final frame qualification summaries differ")
    if qualification_a.get("events") != FINAL_TRACE_EVENT_COUNT:
        raise ClosureError(
            f"final frame must contain {FINAL_TRACE_EVENT_COUNT} canonical events"
        )

    for directory, computed in (
        (trace_a_dir, qualification_a),
        (trace_b_dir, qualification_b),
    ):
        stored = read_json(directory / "qualification.json")
        if stored != computed:
            raise ClosureError(
                f"stored frame qualification disagrees with trace: {directory}"
            )
    run_a = read_json(trace_a_dir / "run.json")
    run_b = read_json(trace_b_dir / "run.json")
    if run_a.get("status") != "PASS" or run_b.get("status") != "PASS":
        raise ClosureError("both final frame runs must have PASS status")
    if (
        not isinstance(run_a.get("run_uuid"), str)
        or not isinstance(run_b.get("run_uuid"), str)
        or run_a["run_uuid"] == run_b["run_uuid"]
    ):
        raise ClosureError("final frame runs are not independent")

    if header.get("scope") != "PRIVATE_TRACK_P_OWNER_DATA":
        raise ClosureError("frame trace is outside the private owner-data scope")
    bindings = header.get("bindings")
    required_bindings = {
        "level_export_package_sha256",
        "level_export_compiled_source_sha256",
        "static_closure_sha256",
        "owner_catalog_sha256",
        "upstream_commit",
        "upstream_diff_sha256",
        "trace_compiled_source_sha256",
        "observer_sha256",
        "trace_format_sha256",
        "binary_sha256",
        "build_manifest_sha256",
        "gdb_version",
        "build_type",
    }
    if not isinstance(bindings, dict) or set(bindings) != required_bindings:
        raise ClosureError("frame trace does not carry the complete binding set")
    for field in required_bindings - {"upstream_commit", "gdb_version", "build_type"}:
        require_sha256(bindings[field], f"frame bindings.{field}")
    if (
        not isinstance(bindings["upstream_commit"], str)
        or len(bindings["upstream_commit"]) != 40
        or bindings["build_type"] != "Debug"
        or not bindings["gdb_version"]
    ):
        raise ClosureError("frame trace build binding is malformed")
    compiled_source = export_manifest.get("source", {}).get(
        "compiled_source_sha256"
    )
    if bindings["level_export_package_sha256"] != export_manifest.get(
        "package_sha256"
    ):
        raise ClosureError("frame trace is bound to a different level package")
    if bindings["level_export_compiled_source_sha256"] != compiled_source:
        raise ClosureError("frame trace is bound to a different export source")
    if bindings["owner_catalog_sha256"] != catalog_data.get("catalog_sha256"):
        raise ClosureError("frame trace is bound to a different owner catalog")
    if bindings["static_closure_sha256"] != BASELINE_STATIC_CLOSURE_SHA256:
        raise ClosureError("frame trace is bound to an unknown static closure")

    invocation = header.get("invocation", {})
    if (
        invocation.get("recording")
        != {"mode": "none", "path": None, "sha256": None}
        or invocation.get("config", {}).get("sha256") is None
    ):
        raise ClosureError("frame trace invocation/recording binding is malformed")
    require_sha256(
        invocation["config"]["sha256"], "frame invocation.config.sha256"
    )
    settings = header.get("settings", {})
    selectors = export_manifest["presentation_dependencies"]
    expected_settings = {
        "map_type": export_manifest["level"]["map_type"]["id"],
        "terrain_graphics_flags": selectors["terrain_graphics_flags"],
        "vga_resolution_selector": selectors["vga_resolution_selector"],
        "texture_size": selectors["texture_size"],
    }
    if any(settings.get(field) != expected for field, expected in expected_settings.items()):
        raise ClosureError("frame trace settings disagree with the Level 1 export")
    camera = header.get("camera", {})
    reset = export_manifest.get("reset_projection", {}).get("expected_state", {})
    if (
        camera.get("entity_index") != reset.get("entity_index")
        or camera.get("position") != reset.get("position")
        or camera.get("yaw") != reset.get("yaw")
        or camera.get("pitch") != reset.get("display_pitch")
        or not isinstance(camera.get("roll"), int)
    ):
        raise ClosureError("frame camera disagrees with the Level 1 reset projection")

    empty = header.get("window", {}).get("empty_category_evidence")
    if not isinstance(empty, dict) or set(empty) != {"cursor", "sound", "music"}:
        raise ClosureError("frame trace must explain cursor, sound, and music as empty")
    observed_categories = {event.get("category") for event in events}
    if observed_categories & set(empty):
        raise ClosureError("armed-empty categories cannot also be frame events")

    trace_sha256 = sha256_bytes(bytes_a)
    event_set_sha256 = header.get("event_set", {}).get("sha256")
    require_sha256(trace_sha256, "frame trace sha256")
    require_sha256(event_set_sha256, "frame event-set sha256")
    return {
        "header": header,
        "events": events,
        "qualification": qualification_a,
        "trace_sha256": trace_sha256,
        "event_set_sha256": event_set_sha256,
        "run_uuids": [run_a["run_uuid"], run_b["run_uuid"]],
        "empty_category_evidence": empty,
    }


def frame_event_observations(
    trace: dict[str, Any],
    catalog: Catalog,
    owner_root: Path,
    selectors: dict[str, Any],
    objective: dict[str, Any],
    language_root: dict[str, Any],
) -> tuple[list[dict[str, Any]], list[dict[str, str]]]:
    block_path = f"DATA/{selectors['block_bank']}.DAT"
    sky_path = f"DATA/{selectors['sky_bank']}.DAT"
    sprite_path = f"DATA/{selectors['sprite_bank']}.DAT"
    tmap_path = f"DATA/{selectors['terrain_bank']}.DAT"
    for path in (block_path, sky_path, sprite_path, tmap_path, "DATA/FONT1.DAT"):
        catalog.verify_owner_file(owner_root, path)
    block_bytes = (owner_root / block_path).read_bytes()
    sky_bytes = (owner_root / sky_path).read_bytes()
    block_side = selectors["texture_size"]
    block_length = block_side * block_side
    objective_hash = objective["content_address"].removeprefix("sha256:")

    observations: list[dict[str, Any]] = []
    frame_edges: list[dict[str, str]] = []
    for ordinal, event in enumerate(trace["events"], 1):
        event_hash = sha256_bytes(canonical_json(event).encode("utf-8"))
        observation_id = f"frame_used:trace-event-{ordinal:03d}-{event_hash[:16]}"
        category = event.get("category")
        owner_ids: list[str]
        span: dict[str, int] | None = None
        observed_content_address: str | None = None

        if category == "terrain":
            if (
                event.get("bank") != selectors["block_bank"]
                or not isinstance(event.get("selector"), int)
            ):
                raise ClosureError(f"invalid terrain event {ordinal}")
            offset = event["selector"] * block_length
            payload = block_bytes[offset : offset + block_length]
            if len(payload) != block_length:
                raise ClosureError(f"terrain selector is out of bounds: {event}")
            span = {"offset": offset, "length": block_length}
            observed_content_address = f"sha256:{sha256_bytes(payload)}"
            owner_ids = [catalog.file_asset(block_path)["asset_id"]]
        elif category == "sky":
            if (
                event.get("bank") != selectors["sky_bank"]
                or event.get("selector") != "full_bank"
                or event.get("span_offset") != 0
                or event.get("span_length") != len(sky_bytes)
            ):
                raise ClosureError(f"invalid sky event {ordinal}")
            span = {"offset": 0, "length": len(sky_bytes)}
            observed_content_address = f"sha256:{sha256_bytes(sky_bytes)}"
            owner_ids = [catalog.file_asset(sky_path)["asset_id"]]
        elif category == "bitmap":
            if event.get("bank") != selectors["sprite_bank"]:
                raise ClosureError(f"bitmap event uses the wrong bank: {event}")
            owner_ids = [
                catalog.indexed_asset(sprite_path, event["selector"])["asset_id"]
            ]
        elif category == "glyph":
            if event.get("bank") != "FONT1":
                raise ClosureError(f"glyph event uses the wrong bank: {event}")
            owner_ids = [
                catalog.indexed_asset("DATA/FONT1.DAT", event["selector"])[
                    "asset_id"
                ]
            ]
        elif category == "tmap":
            if event.get("bank") != selectors["terrain_bank"]:
                raise ClosureError(f"TMAP event uses the wrong bank: {event}")
            owner_ids = [
                catalog.indexed_asset(tmap_path, event["selector"])["asset_id"]
            ]
        elif category == "text":
            raw = bytes.fromhex(event.get("bytes_hex", ""))
            if (
                len(raw) != event.get("byte_length")
                or sha256_bytes(raw) != event.get("content_sha256")
            ):
                raise ClosureError(f"text event bytes/hash disagree: {event}")
            if event["content_sha256"] == objective_hash:
                if raw.hex() != (
                    owner_root / "LANGUAGE/L1.TXT"
                ).read_bytes()[4785:].split(b"\0")[objective["selector"]].hex():
                    raise ClosureError("objective text hash matched but bytes did not")
                owner_ids = [objective["asset_id"], language_root["asset_id"]]
            else:
                glyph_indices = sorted({byte + 1 for byte in raw if byte != 0x20})
                if not glyph_indices:
                    raise ClosureError("raw text event has no drawable glyph")
                owner_ids = [
                    catalog.indexed_asset("DATA/FONT1.DAT", index)["asset_id"]
                    for index in glyph_indices
                ]
        else:
            raise ClosureError(f"unmapped final frame event category: {category}")

        observation = {
            "asset_id": observation_id,
            "kind": "observed_final_entry_asset",
            "trace_event_ordinal": ordinal,
            "trace_event_sha256": event_hash,
            "trace_event": event,
            "owner_asset_ids": sorted(set(owner_ids)),
            "resolution": "unique",
            "content_address": f"sha256:{event_hash}",
        }
        if span is not None:
            observation["owner_span"] = span
        if observed_content_address is not None:
            observation["observed_content_address"] = observed_content_address
        observations.append(observation)
        frame_edges.append(
            edge(
                f"trace.first_render_bearing_gameplay_present.event[{ordinal}]",
                observation_id,
                "canonical final-entry observation promoted to frame_used",
                f"event[{ordinal}].selector + event[{ordinal}].hook",
                "proven",
                f"{event['hook']}; trace sha256 {trace['trace_sha256']}",
            )
        )
        for owner_id in observation["owner_asset_ids"]:
            frame_edges.append(
                edge(
                    observation_id,
                    owner_id,
                    "observed bank/selector to owned catalog asset or entry",
                    f"event[{ordinal}].bank + event[{ordinal}].selector",
                    "proven",
                    f"{event['hook']}; resolution={event.get('resolution', 'bytes')}",
                )
            )
    if len(observations) != FINAL_TRACE_EVENT_COUNT:
        raise ClosureError("not every canonical trace event was promoted")
    if len({item["asset_id"] for item in observations}) != len(observations):
        raise ClosureError("frame-used observation identity collision")
    return observations, frame_edges


def _level2_common_paths() -> list[str]:
    """Return the common process roots shared by every realm profile."""
    return [
        "LEVELS/LEVELS.DAT",
        "LEVELS/LEVELS.TAB",
        "DATA/BUILD0-0.DAT",
        "DATA/BUILD0-0.TAB",
        "DATA/BLDGPRM.DAT",
        "DATA/POINTERS.DAT",
        "DATA/POINTERS.TAB",
        "DATA/SPELLS.DAT",
        "DATA/FONT1.DAT",
        "DATA/FONT1.TAB",
        "LANGUAGE/L1.TXT",
        "SOUND/SOUND.DAT",
        "SOUND/MUSIC.DAT",
        "DATA/TMAPS0-0.DAT",
        "DATA/TMAPS1-0.DAT",
        "DATA/TMAPS2-0.DAT",
    ]


def _level2_expected_selectors(manifest: dict[str, Any]) -> dict[str, Any]:
    """Validate and return the Day selector tuple from the export identity."""
    if manifest.get("schema") != "mc2-owner-level-export/v3":
        raise ClosureError("Level 2 closure requires owner-level-export/v3")
    level = manifest.get("level", {})
    map_type = level.get("map_type", {})
    if (level.get("external"), level.get("internal"), map_type.get("id"), map_type.get("name")) != (
        2,
        1,
        0,
        "Day",
    ):
        raise ClosureError("Level 2 closure requires external=2/internal=1/Day identity")
    if manifest.get("source", {}).get("upstream_commit") != REMC2_SPEC_COMMIT:
        raise ClosureError("Level 2 export is bound to an unknown remc2 specification")
    selectors = manifest.get("presentation_dependencies")
    expected = {
        "map_type": "Day",
        "vga_resolution_selector": 1,
        "texture_size": 32,
        "terrain_graphics_flags": 0,
        "terrain_graphics_flag_2": False,
        "terrain_bank": "TMAPS0-0",
        "sprite_bank": "MSPRD0-0",
        "web_bank": "MWEBD0-0",
        "block_bank": "BL32N0-0",
        "palette": "PALD-0",
        "sky_bank": "SKYD0-0",
        "sound_bank": 1,
        "cursor": 9,
    }
    if not isinstance(selectors, dict) or any(
        selectors.get(field) != value for field, value in expected.items()
    ):
        raise ClosureError("Level 2 presentation selector identity is not the qualified Day tuple")
    expected_paths = {
        "level_records": "owner:GAME/NETHERW/CLEVELS/LEVELS.DAT",
        "terrain_bank": "owner:GAME/NETHERW/CDATA/TMAPS0-0.DAT",
        "sprite_bank": "owner:CD_Files/DATA/MSPRD0-0.DAT",
        "web_bank": "owner:CD_Files/DATA/MWEBD0-0.DAT",
        "block_bank": "owner:CD_Files/DATA/BL32N0-0.DAT",
        "palette": "owner:CD_Files/DATA/PALD-0.DAT",
        "sky_bank": "owner:CD_Files/DATA/SKYD0-0.DAT",
        "building_bank": "owner:CD_Files/DATA/BUILD0-0.DAT",
        "spell_table": "owner:CD_Files/DATA/SPELLS.DAT",
        "font_bank": "owner:CD_Files/DATA/FONT1.DAT",
        "language": "owner:CD_Files/LANGUAGE/L1.TXT",
    }
    if selectors.get("resolved_files") != expected_paths:
        raise ClosureError("Level 2 resolved owner-file selector map is not qualified")
    return selectors


def build_level2_closure(
    export_dir: Path, catalog_path: Path, owner_root: Path
) -> dict[str, Any]:
    """Build the common Day closure without inventing an unobserved frame trace.

    The Level 2 export is a validated package and its owner-file selector tuple
    is qualified, but no final-entry frame pair exists yet.  The resulting
    ``frame_used`` partition therefore stays explicitly unqualified instead of
    claiming zero unresolved observations.
    """
    export_manifest = read_json(export_dir / "manifest.json")
    catalog_data = read_json(catalog_path)
    selectors = _level2_expected_selectors(export_manifest)
    verify_export_identity(export_dir, export_manifest)
    if catalog_data.get("schema_version") is None:
        raise ClosureError("owner catalog has no schema_version")
    catalog = Catalog(catalog_data)
    map_type = export_manifest["level"]["map_type"]["id"]
    resolution = selectors["vga_resolution_selector"]
    texture_size = selectors["texture_size"]
    terrain_flags = selectors["terrain_graphics_flags"]
    realm_paths = select_realm_assets(map_type, resolution, texture_size, terrain_flags)
    global_paths = _level2_common_paths()
    for path in sorted(set(realm_paths + global_paths)):
        catalog.verify_owner_file(owner_root, path)
    root_assets = [catalog.file_asset(path) for path in sorted(set(realm_paths + global_paths))]
    edges: list[dict[str, str]] = []
    for path in realm_paths:
        asset = catalog.file_asset(path)
        edges.append(
            edge(
                "level2.selected_realm",
                asset["asset_id"],
                "qualified Day realm/texture/resolution selector",
                "manifest.presentation_dependencies",
                "proven",
                "Level.cpp:878-1074; ReadAndDecompress.cpp:20-168",
            )
        )
    for path in global_paths:
        edges.append(
            edge(
                "process.global_open",
                catalog.file_asset(path)["asset_id"],
                "common startup file-array or bank open",
                "Level 2 manifest resolved_files + global loader root",
                "proven",
                "Basic.cpp:253-273; EventsFunctions.cpp:42837-43265; Sound.cpp:2299-2356",
            )
        )

    presentation = parse_presentation(export_dir / "presentation.bin")
    entities = parse_entities(export_dir / "entities.bin")
    tmaps_path = f"DATA/TMAPS{map_type}-0.DAT"
    catalog.index_tmaps(tmaps_path)
    scene_assets: list[dict[str, Any]] = []
    dynamic_assets: list[dict[str, Any]] = []
    for group_id, selected in enumerate(presentation["level_sprite_selectors"]):
        if not selected:
            continue
        for member in catalog.tmap_group(group_id):
            asset = catalog.tmap_asset(member)
            scene_assets.append(asset)
            edges.append(
                edge(
                    f"presentation.level_sprite_selectors[{group_id}]",
                    asset["asset_id"],
                    "TMAP allocation group expansion",
                    f"presentation.bin.level_sprite_selectors[{group_id}]",
                    "proven",
                    "TextureMaps.cpp:227-265; owner catalog animation_or_allocation_group",
                )
            )
    for parameter_index in entities["active_particle_parameters"]:
        if parameter_index >= len(presentation["particles"]):
            raise ClosureError(f"particle parameter index out of bounds: {parameter_index}")
        parameter = presentation["particles"][parameter_index]
        base = parameter["base_tmap"]
        mode = parameter["render_mode"]
        strategy = tmap_render_strategy(mode)
        members = catalog.tmap_group(base)
        group_indices = [member["index"] for member in members]
        for member in members:
            asset = catalog.tmap_asset(member)
            scene_assets.append(asset)
            edges.append(
                edge(
                    f"active_particle_parameter[{parameter_index}]",
                    asset["asset_id"],
                    "TMAP allocation group expansion",
                    f"presentation.bin.particles[{parameter_index}].word_0",
                    "proven",
                    "TextureMaps.cpp:227-265; Level 2 entity/presentation relation",
                )
            )
        if strategy not in {"base", "base_transparent"}:
            for choice in render_choice_candidates(base, mode, group_indices):
                dynamic_assets.append(catalog.tmap_asset(catalog.tmaps[choice]))
    footprints = parse_build_footprints(owner_root, entities["active_build_indices"])
    scene_assets.extend(footprints)
    for asset in footprints:
        edges.append(
            edge(
                f"active_build_entity[index={asset['index']}]",
                asset["asset_id"],
                "class=10/model=45 BUILD footprint relation",
                "entities.bin.class/model/build_index",
                "proven",
                "EventsFunctions.cpp:27077-27105; BUILD TAB/DAT spans",
            )
        )
    runtime = (export_dir / "runtime.bin").read_bytes()
    runtime_layout = export_manifest["record_layouts"]["runtime"]["objectives"]
    objective_selector = objective_language_selector(runtime, runtime_layout, 2)
    objective = parse_language_record(owner_root, objective_selector)
    language_root = catalog.file_asset("LANGUAGE/L1.TXT")
    scene_assets.append(objective)
    edges.append(
        edge(
            "runtime.objectives[0]",
            objective["asset_id"],
            "level external index + ObjectiveText",
            f"runtime.bin[{runtime_layout['offset'] + 1}]",
            "proven",
            "GameUI.cpp:20-28,532-580; LANGUAGE loader",
        )
    )
    edges.append(
        edge(
            objective["asset_id"],
            language_root["asset_id"],
            "record belongs to localization file",
            "language selector",
            "proven",
            "EventsFunctions.cpp:42837-42854",
        )
    )
    for glyph in resolve_glyph_assets(catalog, objective):
        scene_assets.append(glyph)
        edges.append(
            edge(
                objective["asset_id"],
                glyph["asset_id"],
                "owner 8-bit text byte + 1 to FONT1 glyph index",
                f"LANGUAGE/L1.TXT string {objective_selector}",
                "proven",
                "GameUI.cpp:579-614; FONT1 owner catalog indexed entries",
            )
        )
    cursor = catalog.indexed_asset("DATA/POINTERS.DAT", selectors["cursor"])
    scene_assets.append(cursor)
    edges.append(
        edge(
            "presentation.cursor",
            cursor["asset_id"],
            "POINTERS bank plus cursor index",
            "manifest.presentation_dependencies.cursor",
            "proven",
            "GameUI.cpp:660-671; EventsFunctions.cpp:8594-8658",
        )
    )
    dynamic_assets.append(
        catalog.indexed_asset(f"DATA/{selectors['sprite_bank']}.DAT", selectors["cursor"])
    )
    for path, reason in (
        (f"DATA/{selectors['sprite_bank']}.DAT", "Day HUD/spell/world sprite bank"),
        (f"DATA/{selectors['web_bank']}.DAT", "Day minimap web bank"),
        ("SOUND/SOUND.DAT", "sound bank 1 dynamic sample selection"),
        ("SOUND/MUSIC.DAT", "Day music track dynamic selection"),
    ):
        dynamic_assets.append(catalog.file_asset(path))
        edges.append(
            edge(
                "level2.runtime",
                catalog.file_asset(path)["asset_id"],
                reason,
                "runtime state and selected Day profile",
                "dynamic",
                "Level 2 closure does not claim an exact frame/audio observation",
            )
        )
    scene_assets = unique_assets(scene_assets)
    dynamic_assets = unique_assets(dynamic_assets)
    edges.sort(key=lambda item: (item["from"], item["to"], item["join"], item["source_field"], item["status"]))
    source_files = {
        name: {"sha256": sha256_file(export_dir / name), "size": (export_dir / name).stat().st_size}
        for name in ("manifest.json", "presentation.bin", "entities.bin", "runtime.bin")
    }
    document: dict[str, Any] = {
        "schema": SCHEMA,
        "scope": "PRIVATE_TRACK_P_OWNER_DATA",
        "profile": {
            "name": "level2-day",
            "external_level": 2,
            "internal_level": 1,
            "map_type": {"id": 0, "name": "day"},
            "spec_commit": export_manifest["source"]["upstream_commit"],
            "identity_status": "qualified",
        },
        "source": {
            "level_export": {
                "path": str(export_dir),
                "package_sha256": export_manifest["package_sha256"],
                "files": source_files,
            },
            "owner_catalog": {
                "path": str(catalog_path),
                "catalog_sha256": catalog_data.get("catalog_sha256"),
                "file_sha256": sha256_file(catalog_path),
            },
            "owner_root_identity": {
                path: sha256_file(owner_root / path)
                for path in sorted(set(realm_paths + global_paths))
            },
        },
        "selectors": {
            "level_external": 2,
            "level_internal": 1,
            "map_type": {"id": 0, "name": "day"},
            "vga_resolution_selector": resolution,
            "texture_size": texture_size,
            "terrain_graphics_flags": terrain_flags,
            "objective_language_selector": objective_selector,
            "active_particle_parameters": entities["active_particle_parameters"],
            "active_build_indices": entities["active_build_indices"],
            "audio": {"sound_bank": selectors["sound_bank"], "music_observation": "not_supplied"},
        },
        "sets": {
            "realm_load_roots": {
                "status": "qualified",
                "assets": unique_assets(root_assets),
                "selected_realm": sorted(catalog.file_asset(path)["asset_id"] for path in realm_paths),
                "global_open": sorted(catalog.file_asset(path)["asset_id"] for path in global_paths),
                "unresolved": [],
            },
            "scene_reachable": {"status": "qualified", "assets": scene_assets, "unresolved": []},
            "frame_used": {
                "status": "not_qualified_no_level2_frame_pair",
                "assets": [],
                "unresolved": ["Level 2 final-entry frame trace has not been supplied"],
                "armed_empty_evidence": {"cursor": "not_observed", "sound": "not_observed", "music": "not_observed"},
            },
            "dynamic_dependencies": {
                "status": "qualified_as_dynamic_not_exact",
                "assets": dynamic_assets,
                "unresolved": ["exact frame/audio selector use requires a qualified Level 2 trace"],
            },
        },
        "edges": edges,
        "qualification": {
            "realm_load_roots": "PASS",
            "scene_reachable": "PASS",
            "dynamic_dependencies": "PASS_AS_EXPLICIT_DYNAMIC_SET",
            "frame_used": "NOT_YET_QUALIFIED",
            "zero_unresolved_frame_used_claimed": False,
            "policy": "Common structure is qualified; exact frame-used claims remain fail-closed until a Level 2 trace pair exists.",
        },
    }
    document["closure_sha256"] = sha256_bytes(canonical_json(document).encode("utf-8"))
    return document


def build_closure(
    export_dir: Path,
    catalog_path: Path,
    owner_root: Path,
    trace_a_dir: Path,
    trace_b_dir: Path,
    profile: str = "level1",
) -> dict[str, Any]:
    export_manifest = read_json(export_dir / "manifest.json")
    catalog_data = read_json(catalog_path)
    if profile == "level2":
        return build_level2_closure(export_dir, catalog_path, owner_root)
    if profile != "level1":
        raise ClosureError(f"unsupported closure profile: {profile}")
    if export_manifest.get("schema") != "mc2-owner-level-export/v2":
        raise ClosureError("unsupported level-export schema")
    if catalog_data.get("schema_version") is None:
        raise ClosureError("owner catalog has no schema_version")
    verify_export_identity(export_dir, export_manifest)
    trace = load_qualified_trace_pair(
        trace_a_dir, trace_b_dir, export_manifest, catalog_data
    )

    selectors = export_manifest["presentation_dependencies"]
    map_type = export_manifest["level"]["map_type"]["id"]
    resolution = selectors["vga_resolution_selector"]
    texture_size = selectors["texture_size"]
    terrain_flags = selectors["terrain_graphics_flags"]
    if map_type != 1 or export_manifest["level"]["external"] != 1:
        raise ClosureError("this closure contract is specifically for initialized Level 1")

    catalog = Catalog(catalog_data)
    realm_paths = select_realm_assets(map_type, resolution, texture_size, terrain_flags)
    global_specs = [
        ("LEVELS/LEVELS.DAT", "compressed level record open", "level external index", "EventsFunctions.cpp:38235-38268"),
        ("LEVELS/LEVELS.TAB", "level offset-table open", "level external index", "EventsFunctions.cpp:38246-38263"),
        ("DATA/BUILD0-0.DAT", "startup file-array load", "psxadatabuild00dat", "Basic.cpp:271-273; EventsFunctions.cpp:43230-43255"),
        ("DATA/BUILD0-0.TAB", "startup file-array index load", "psxadatabuild00dat", "Basic.cpp:271-273; EventsFunctions.cpp:43230-43255"),
        ("DATA/BLDGPRM.DAT", "building-parameter table open", "str_D93C0_bldgprmbuffer", "EventsFunctions.cpp:38331-38340"),
        ("DATA/POINTERS.DAT", "startup pointer/cursor file-array load", "filearrayindex_POINTERSDATTAB", "Basic.cpp:265-267; EventsFunctions.cpp:42885-42898"),
        ("DATA/POINTERS.TAB", "startup pointer/cursor index load", "filearrayindex_POINTERSDATTAB", "Basic.cpp:265-267; EventsFunctions.cpp:42885-42898"),
        ("DATA/SPELLS.DAT", "startup spell-table load", "xadataspellsdatx", "EventsFunctions.cpp:42905-42916"),
        ("DATA/FONT1.DAT", "startup font file-array load", "xafonts1dat", "Basic.cpp:253-257; EventsFunctions.cpp:43230-43265"),
        ("DATA/FONT1.TAB", "startup font index load", "xafonts1tab", "Basic.cpp:253-257; EventsFunctions.cpp:43230-43265"),
        ("LANGUAGE/L1.TXT", "configured language-file open", "langIndex_4", "MenusAndIntros.cpp:695-713"),
        ("SOUND/SOUND.DAT", "sound-bank container open", "sound bank + SoundNumber_E37B6", "Sound.cpp:2299-2356"),
        ("SOUND/MUSIC.DAT", "music-bank container open", "music driver/channel", "Sound.cpp:5504-5530"),
        ("DATA/TMAPS0-0.DAT", "global TMAP file-handle open", "x_DWORD_DB740_tmaps00file", "TextureMaps.cpp:570-595"),
        ("DATA/TMAPS1-0.DAT", "global TMAP file-handle open", "x_DWORD_DB744_tmaps10file", "TextureMaps.cpp:570-595"),
        ("DATA/TMAPS2-0.DAT", "global TMAP file-handle open", "x_DWORD_DB748_tmaps20file", "TextureMaps.cpp:570-595"),
    ]
    for raw_join_path in (
        "DATA/BUILD0-0.DAT",
        "DATA/BUILD0-0.TAB",
        "LANGUAGE/L1.TXT",
    ):
        catalog.verify_owner_file(owner_root, raw_join_path)
    global_paths = [item[0] for item in global_specs]
    root_assets = [
        catalog.file_asset(path) for path in sorted(set(realm_paths + global_paths))
    ]
    edges: list[dict[str, str]] = []
    for path in realm_paths:
        asset = catalog.file_asset(path)
        if "TMAPS" in path and path.endswith(".DAT"):
            join = "selected current TMAP file handle"
            source_field = "level.map_type"
            evidence = "Level.cpp:1582-1610"
        elif "TMAPS" in path or "BL16" in path or "BL32" in path or "BLOCK" in path or "SKY" in path:
            join = "realm and texture-size graphics selection"
            source_field = "level.map_type + texture_size + terrain_graphics_flags"
            evidence = "ReadAndDecompress.cpp:20-138"
        elif "/PAL" in path:
            join = "realm/fog palette selection"
            source_field = "level.map_type + terrain_graphics_flags bit 2"
            evidence = "Level.cpp:878-905"
        elif "/CLR" in path:
            join = "realm color-lookup selection"
            source_field = "level.map_type"
            evidence = "EventsFunctions.cpp:31898-31936"
        elif "/TABLES" in path:
            join = "realm render-table selection"
            source_field = "level.map_type"
            evidence = "ReadAndDecompress.cpp:145-168"
        elif "SPR" in path:
            join = "realm and VGA-resolution sprite-bank selection"
            source_field = "level.map_type + vga_resolution_selector"
            evidence = "Level.cpp:1017-1074"
        elif "WEB" in path:
            join = "realm and VGA-resolution minimap-bank selection"
            source_field = "level.map_type + vga_resolution_selector"
            evidence = "Level.cpp:1410-1475"
        else:  # pragma: no cover - realm path matrix is closed
            raise ClosureError(f"no selected-realm loader evidence for {path}")
        edges.append(
            edge(
                "level1.selected_realm",
                asset["asset_id"],
                join,
                source_field,
                "proven",
                evidence,
            )
        )
    for path, join, source_field, evidence in global_specs:
        edges.append(
            edge(
                "process.global_open",
                catalog.file_asset(path)["asset_id"],
                join,
                source_field,
                "proven",
                evidence,
            )
        )

    presentation = parse_presentation(export_dir / "presentation.bin")
    entities = parse_entities(export_dir / "entities.bin")
    tmaps_path = f"DATA/TMAPS{map_type}-0.DAT"
    catalog.index_tmaps(tmaps_path)
    scene_assets: list[dict[str, Any]] = []
    dynamic_assets: list[dict[str, Any]] = []

    # The level sprite list is indexed by the TMAP allocation-group id.
    for group_id, selected in enumerate(presentation["level_sprite_selectors"]):
        if not selected:
            continue
        for member in catalog.tmap_group(group_id):
            asset = catalog.tmap_asset(member)
            scene_assets.append(asset)
            edges.append(
                edge(
                    f"presentation.level_sprite_selectors[{group_id}]",
                    asset["asset_id"],
                    "TMAP allocation group expansion",
                    f"presentation.bin.level_sprite_selectors[{group_id}]",
                    "proven",
                    "TextureMaps.cpp:227-265; owner catalog animation_or_allocation_group",
                )
            )

    for parameter_index in entities["active_particle_parameters"]:
        if parameter_index >= len(presentation["particles"]):
            raise ClosureError(f"particle parameter index out of bounds: {parameter_index}")
        parameter = presentation["particles"][parameter_index]
        base = parameter["base_tmap"]
        mode = parameter["render_mode"]
        strategy = tmap_render_strategy(mode)
        members = catalog.tmap_group(base)
        group_indices = [member["index"] for member in members]
        for member in members:
            asset = catalog.tmap_asset(member)
            scene_assets.append(asset)
            edges.append(
                edge(
                    f"active_particle_parameter[{parameter_index}]",
                    asset["asset_id"],
                    "TMAP allocation group expansion",
                    f"presentation.bin.particles[{parameter_index}].word_0",
                    "proven",
                    "TextureMaps.cpp:227-265; owner catalog "
                    "animation_or_allocation_group; active entities.bin word_0x5A",
                )
            )
        choices = render_choice_candidates(base, mode, group_indices)
        if strategy not in {"base", "base_transparent"}:
            for choice in choices:
                member = catalog.tmaps[choice]
                asset = catalog.tmap_asset(member)
                dynamic_assets.append(asset)
                edges.append(
                    edge(
                        f"active_particle_parameter[{parameter_index}]",
                        asset["asset_id"],
                        f"runtime render choice: {strategy}",
                        f"presentation.bin.particles[{parameter_index}].word_0/byte_12",
                        "dynamic",
                        "GameRenderOriginal.cpp:1959-2189; "
                        "GameRenderOriginal.h:48-55 D4750; animationFrame/yaw runtime state",
                    )
                )

    footprints = parse_build_footprints(owner_root, entities["active_build_indices"])
    scene_assets.extend(footprints)
    for asset in footprints:
        edges.append(
            edge(
                f"active_build_entity[index={asset['index']}]",
                asset["asset_id"],
                "class=10, model=45, byte_0x46 BUILD index",
                "entities.bin.class_0x3F/model_0x40/byte_0x46",
                "proven",
                "EventsFunctions.cpp:27077-27105; BUILD TAB <IBB> and DAT 2-byte cells",
            )
        )

    # BUILD index 1 is absent from the reset entity set: it is selected only
    # after the tier-zero Castle projectile creates a new class-3/model-2 root.
    # Keep that causal dependency explicit rather than pretending it was
    # observed in the frozen scene.
    castle_stage_one = parse_build_footprints(owner_root, [1])[0]
    dynamic_assets.append(castle_stage_one)
    edges.append(
        edge(
            "spell.castle.stage_0_to_1",
            castle_stage_one["asset_id"],
            "sub_60480 increments the castle level before BUILD index 1 terrain construction",
            "class=3, model=2, dword_0x10 stage",
            "dynamic",
            "EventsFunctions.cpp:61612-61652; "
            "EventsFunctions.cpp:27648-28032; BUILD TAB <IBB> and DAT 2-byte cells",
        )
    )

    # Level 1's current objective is base 48 plus ObjectiveText 0.
    runtime = (export_dir / "runtime.bin").read_bytes()
    divergences = export_manifest.get("reference", {}).get(
        "runtime_scalar_divergences"
    )
    if not isinstance(divergences, list) or len(divergences) != 1:
        raise ClosureError("expected exactly one documented runtime scalar divergence")
    music_divergence = divergences[0]
    expected_music_divergence = {
        "field": "maptypeMusic_0x235",
        "original_state_offset": "0x235",
        "runtime_bin_offset": 32,
        "exported_remc2_value": 1,
        "preserved_dos_frames": "0..19",
        "preserved_dos_value": 0,
        "status": "presentation_environment_divergence",
        "evidence": (
            "native remc2 selects the Night music bank when music is enabled; "
            "the oracle capture had music inactive"
        ),
    }
    if music_divergence != expected_music_divergence:
        raise ClosureError("unexpected or malformed maptypeMusic divergence contract")
    if len(runtime) < 36:
        raise ClosureError("runtime.bin is truncated before maptypeMusic")
    native_music_track = struct.unpack_from("<I", runtime, 32)[0]
    if native_music_track != music_divergence["exported_remc2_value"]:
        raise ClosureError("runtime maptypeMusic disagrees with documented divergence")
    runtime_layout = export_manifest["record_layouts"]["runtime"]["objectives"]
    objective_offset = runtime_layout["offset"]
    objective_selector = objective_language_selector(
        runtime, runtime_layout, export_manifest["level"]["external"]
    )
    objective = parse_language_record(owner_root, objective_selector)
    language_root = catalog.file_asset("LANGUAGE/L1.TXT")
    scene_assets.append(objective)
    edges.append(
        edge(
            "runtime.objectives[0]",
            objective["asset_id"],
            "IndexLevelText_DB4EE[level_external-1] + ObjectiveText_1",
            f"runtime.bin[{objective_offset + 1}]",
            "proven",
            "GameUI.cpp:20-28,532-580; LANGUAGE loader skips 4785-byte graphics prefix",
        )
    )
    edges.append(
        edge(
            objective["asset_id"],
            language_root["asset_id"],
            "record belongs to localization file",
            "language selector",
            "proven",
            "EventsFunctions.cpp:42837-42854",
        )
    )
    for glyph in resolve_glyph_assets(catalog, objective):
        glyph_index = glyph["index"]
        scene_assets.append(glyph)
        edges.append(
            edge(
                objective["asset_id"],
                glyph["asset_id"],
                "owner 8-bit text byte + 1 to FONT1 glyph index",
                f"LANGUAGE/L1.TXT string {objective_selector}",
                "proven",
                "GameUI.cpp:579-614; FONT1 owner catalog indexed entries",
            )
        )

    cursor = catalog.indexed_asset("DATA/POINTERS.DAT", selectors["cursor"])
    scene_assets.append(cursor)
    edges.append(
        edge(
            "presentation.cursor",
            cursor["asset_id"],
            "POINTERS bank plus cursor index",
            "manifest.presentation_dependencies.cursor",
            "proven",
            "Level 1 export manifest; GameUI.cpp:660-671; EventsFunctions.cpp:8594-8658",
        )
    )
    conditional_mspr_cursor = catalog.indexed_asset(
        f"DATA/{selectors['sprite_bank']}.DAT", selectors["cursor"]
    )
    dynamic_assets.append(conditional_mspr_cursor)
    edges.append(
        edge(
            "spell_menu.runtime",
            conditional_mspr_cursor["asset_id"],
            "conditional spell-menu bitmap index",
            "CURSOR_SPRITE_INDEX_D419E",
            "dynamic",
            "EventsFunctions.cpp:22686; this is not the OS/game cursor POINTERS bank",
        )
    )

    frame_assets, frame_edges = frame_event_observations(
        trace,
        catalog,
        owner_root,
        selectors,
        objective,
        language_root,
    )
    edges.extend(frame_edges)

    # These roots are valid future choices, while the trace proves this frame's exact use.
    for path, reason, field, evidence in (
        (
            "SOUND/SOUND.DAT",
            "sound bank 1 plus unresolved SoundNumber_E37B6 quality row selects sample data",
            "presentation.sound_bank=1 + SoundNumber_E37B6",
            "Sound.cpp:2299-2356",
        ),
        (
            "SOUND/MUSIC.DAT",
            "raw native music track 1 selects sequence; preserved DOS track was 0",
            "runtime.bin maptypeMusic offset 32 + reference.runtime_scalar_divergences",
            "EventsFunctions.cpp:31439-31454; Sound.cpp:869-901,6555; export divergence",
        ),
        (
            f"DATA/{selectors['sprite_bank']}.DAT",
            "HUD/spell/world sprite index selected at draw",
            "HUD and draw-call state",
            "EventsFunctions.cpp:21950-22686",
        ),
        (
            f"DATA/{selectors['web_bank']}.DAT",
            "minimap web tile selected by map/UI state",
            "map coordinates and UI state",
            "Level.cpp:1410-1475; final bitmap selection requires capture",
        ),
    ):
        asset = catalog.file_asset(path)
        dynamic_assets.append(asset)
        edges.append(
            edge(
                "level1.runtime",
                asset["asset_id"],
                reason,
                field,
                "dynamic",
                evidence,
            )
        )

    scene_assets = unique_assets(scene_assets)
    dynamic_assets = unique_assets(dynamic_assets)
    edges.sort(
        key=lambda item: (
            item["from"],
            item["to"],
            item["join"],
            item["source_field"],
            item["status"],
        )
    )
    source_files = {
        name: {
            "sha256": sha256_file(export_dir / name),
            "size": (export_dir / name).stat().st_size,
        }
        for name in ("manifest.json", "presentation.bin", "entities.bin", "runtime.bin")
    }
    document: dict[str, Any] = {
        "schema": SCHEMA,
        "scope": "PRIVATE_TRACK_P_OWNER_DATA",
        "source": {
            "level_export": {
                "path": "research/private/generated/level1-export-m",
                "package_sha256": export_manifest["package_sha256"],
                "files": source_files,
            },
            "owner_catalog": {
                "path": "research/private/remc2-atlas/semantics/generated/"
                "owner-asset-catalog.json",
                "catalog_sha256": catalog_data.get("catalog_sha256"),
                "file_sha256": sha256_file(catalog_path),
            },
            "frame_trace": {
                "paths": [
                    "research/private/generated/level1-frame-trace-final-v2-a",
                    "research/private/generated/level1-frame-trace-final-v2-b",
                ],
                "trace_sha256": trace["trace_sha256"],
                "event_set_sha256": trace["event_set_sha256"],
                "canonical_event_count": len(trace["events"]),
                "qualified_epoch": trace["qualification"]["qualified_epoch"],
                "skipped_presents": trace["qualification"]["skipped_presents"],
                "byte_identical": True,
                "independent_pass_runs": True,
                "bindings": trace["header"]["bindings"],
                "empty_category_evidence": trace["empty_category_evidence"],
            },
            "owner_root_identity": {
                "BUILD0-0.DAT": sha256_file(owner_root / "DATA/BUILD0-0.DAT"),
                "BUILD0-0.TAB": sha256_file(owner_root / "DATA/BUILD0-0.TAB"),
                "LANGUAGE/L1.TXT": sha256_file(owner_root / "LANGUAGE/L1.TXT"),
            },
        },
        "selectors": {
            "level_external": 1,
            "map_type": {"id": map_type, "name": REALMS[map_type]["name"]},
            "vga_resolution_selector": resolution,
            "texture_size": texture_size,
            "terrain_graphics_flags": terrain_flags,
            "objective_language_selector": objective_selector,
            "active_particle_parameters": entities["active_particle_parameters"],
            "active_build_indices": entities["active_build_indices"],
            "audio": {
                "sound_bank": selectors["sound_bank"],
                "sound_quality_selector": {
                    "value": "hq_44khz" if trace["header"]["settings"]["hq_sound"] else "native",
                    "status": "observed_frame_setting",
                    "source": "frame_trace.header.settings.hq_sound",
                },
                "music_raw_track": native_music_track,
                "music_presentation_divergence": {
                    "native_remc2": music_divergence["exported_remc2_value"],
                    "preserved_dos": music_divergence["preserved_dos_value"],
                    "dos_frames": music_divergence["preserved_dos_frames"],
                    "status": music_divergence["status"],
                },
            },
        },
        "sets": {
            "realm_load_roots": {
                "status": "qualified",
                "assets": unique_assets(root_assets),
                "selected_realm": sorted(
                    catalog.file_asset(path)["asset_id"] for path in realm_paths
                ),
                "global_open": sorted(
                    catalog.file_asset(path)["asset_id"] for path in global_paths
                ),
                "unresolved": [],
            },
            "scene_reachable": {
                "status": "qualified",
                "assets": scene_assets,
                "unresolved": [],
            },
            "frame_used": {
                "status": "qualified_final_entry_trace",
                "assets": frame_assets,
                "unresolved": [],
                "armed_empty_evidence": trace["empty_category_evidence"],
            },
            "dynamic_dependencies": {
                "status": "qualified_as_dynamic_not_exact",
                "assets": dynamic_assets,
                "unresolved": [],
            },
        },
        "edges": edges,
        "schema_corrections": {
            "BUILD0-0": {
                "catalog_interpretation_rejected": "indexed sprite/RLE",
                "qualified_interpretation": "width*height terrain grid, two bytes per cell",
                "stale_catalog_entry_hashes_used": False,
                "evidence": "EventsFunctions.cpp:27077-27105,28080-28125 and exact TAB/DAT spans",
            }
        },
        "qualification": {
            "realm_load_roots": "PASS",
            "scene_reachable": "PASS",
            "dynamic_dependencies": "PASS_AS_EXPLICIT_DYNAMIC_SET",
            "frame_used": "PASS_FINAL_ENTRY_TRACE",
            "frame_trace_pair": "PASS_BYTE_IDENTICAL_INDEPENDENT_RUNS",
            "canonical_frame_events_promoted": len(frame_assets),
            "ambiguous_frame_used": 0,
            "unresolved_frame_used": 0,
            "zero_unresolved_frame_used_claimed": True,
            "armed_empty_categories": sorted(trace["empty_category_evidence"]),
            "policy": (
                "Only canonical final-entry observations are promoted; "
                "armed-empty categories are evidence, not assets."
            ),
        },
    }
    document["closure_sha256"] = sha256_bytes(canonical_json(document).encode("utf-8"))
    return document


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--profile",
        choices=("level1", "level2"),
        default="level1",
        help="validated realm/profile closure to derive",
    )
    parser.add_argument(
        "--export-dir",
        type=Path,
        default=Path("research/private/generated/level1-export-m"),
    )
    parser.add_argument(
        "--catalog",
        type=Path,
        default=Path(
            "research/private/remc2-atlas/semantics/generated/owner-asset-catalog.json"
        ),
    )
    parser.add_argument(
        "--owner-root",
        type=Path,
        default=Path("research/private/remc2-native-runtime/CD_Files"),
    )
    parser.add_argument(
        "--trace-a",
        type=Path,
        default=Path(
            "research/private/generated/level1-frame-trace-final-v2-a"
        ),
    )
    parser.add_argument(
        "--trace-b",
        type=Path,
        default=Path(
            "research/private/generated/level1-frame-trace-final-v2-b"
        ),
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
    )
    args = parser.parse_args()
    export_dir = args.export_dir
    if args.profile == "level2" and export_dir == Path(
        "research/private/generated/level1-export-m"
    ):
        export_dir = Path("research/private/generated/l2-debug3.fJ3PaM/level2")
    output = args.output or Path(
        "research/private/generated/level2-asset-closure.json"
        if args.profile == "level2"
        else "research/private/generated/level1-asset-closure.json"
    )
    document = build_closure(
        export_dir,
        args.catalog,
        args.owner_root,
        args.trace_a,
        args.trace_b,
        args.profile,
    )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(
        json.dumps(
            {
                "output": str(output),
                "closure_sha256": document["closure_sha256"],
                "counts": {
                    name: len(value["assets"])
                    for name, value in document["sets"].items()
                },
                "qualification": document["qualification"],
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
