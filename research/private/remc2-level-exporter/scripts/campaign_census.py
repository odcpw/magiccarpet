#!/usr/bin/env python3
"""Build the bounded private campaign census for external Levels 3..24.

The exporter is the only producer used here.  This module validates the
pointer-free packages it produces, compares two complete directory trees, and
emits source-shaped observations only.  It never launches the original game,
DOSBox, a decompiler, or a simulation session.

The census deliberately stops at an evidence boundary.  It records initial
authored/reset selector tuples, the fixed objective sections, map identity,
owner/source facts, and a conservative enabled-table behavior closure.  It is
not a playability or simulation-completeness claim.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import struct
import subprocess
import sys
from collections.abc import Iterable, Mapping, Sequence
from pathlib import Path
from typing import Any

COMMIT = "6806c7204d17220ecc0430759f784aea8ee2ddb2"
SCHEMA = "mc2-campaign-census/v1"
PACKAGE_SCHEMA = "mc2-owner-level-export/v5"
AUTHORED_COUNT = 1200
AUTHORED_BYTES = 20
AUTHORED_OFFSET = 1091
ENTITY_COUNT = 1000
ENTITY_BYTES = 176
RUNTIME_BYTES = 2754
RUNTIME_SECTIONS = {
    "stages": (192, 8, 14),
    "objectives": (304, 8, 11),
    "stage_variables": (392, 11, 12),
    "events": (524, 50, 43),
    "player_inputs": (2674, 8, 10),
}
MAP_TYPES = {0: "Day", 1: "Night", 2: "Cave"}
DISPATCH_SITES = {
    "apply_events": 1,
    "update_entities": 2,
    "constructor": 3,
    "type_subtype": 4,
}
REQUIRED_PACKAGE_FILES = {
    "authored-level.bin",
    "collision-masks.bin",
    "entities-post-features.bin",
    "entities.bin",
    "feature-allocator.bin",
    "feature-construction-inputs.bin",
    "feature-end.bin",
    "feature-start.bin",
    "players.bin",
    "presentation.bin",
    "runtime.bin",
    "terrain-post-features.bin",
    "terrain-pre-features.bin",
    "terrain.bin",
}
HEX64 = set("0123456789abcdef")


class CensusError(ValueError):
    """A fail-closed census or package validation error."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise CensusError(message)


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")


def canonical_sha256(value: Any) -> str:
    return sha256_bytes(canonical_bytes(value))


def hex64(value: Any, label: str) -> str:
    require(isinstance(value, str) and len(value) == 64 and set(value) <= HEX64, f"{label} is not a lowercase SHA-256")
    return value


def signed8(value: int) -> int:
    return value - 256 if value >= 128 else value


def signed16(value: int) -> int:
    return value - 65536 if value >= 32768 else value


def repo_relative(path: Path, root: Path) -> str:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return path.resolve().as_posix()


def parse_int(value: str, label: str) -> int:
    try:
        parsed = int(value, 10)
    except ValueError as exc:
        raise CensusError(f"{label} is not a decimal integer: {value!r}") from exc
    return parsed


def level_list(spec: str) -> list[int]:
    """Parse a comma/range list, then require exactly external Levels 3..24."""

    levels: set[int] = set()
    for part in spec.split(","):
        part = part.strip()
        require(part, "empty level selector")
        if "-" in part:
            left, right = part.split("-", 1)
            start = parse_int(left, "level range start")
            end = parse_int(right, "level range end")
            require(start <= end, f"descending level range {part!r}")
            levels.update(range(start, end + 1))
        else:
            levels.add(parse_int(part, "level"))
    result = sorted(levels)
    require(result == list(range(3, 25)), f"census must cover exactly Levels 3..24, got {result}")
    return result


def member_identity(files: Mapping[str, Mapping[str, Any]]) -> str:
    """Match the exporter/native loader package identity algorithm."""

    chunks: list[bytes] = []
    for name in sorted(files):
        metadata = files[name]
        chunks.extend(
            (
                name.encode("utf-8"),
                b"\0",
                str(metadata["sha256"]).encode("ascii"),
                b"\0",
                str(metadata["bytes"]).encode("ascii"),
                b"\n",
            )
        )
    return sha256_bytes(b"".join(chunks))


def read_json(path: Path, label: str) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise CensusError(f"cannot read {label} {path}: {exc}") from exc


def bounded_indices(indices: Iterable[int]) -> dict[str, Any]:
    values = sorted(set(indices))
    encoded = canonical_bytes(values)
    return {
        "count": len(values),
        "first": values[:8],
        "last": values[-8:],
        "sha256": sha256_bytes(encoded),
    }


def tuple_key(values: Mapping[str, Any]) -> tuple[int, int, int, int]:
    return (int(values["class"]), int(values["action"]), int(values["model"]), int(values["subtype"]))


def tuple_text(key: Sequence[int]) -> str:
    return "/".join(str(value) for value in key)


def read_dispatch_atlas(path: Path) -> tuple[dict[int, list[dict[str, Any]]], dict[int, list[dict[str, Any]]], str]:
    """Read the generated source-table measurement without carrying addresses."""

    require(path.is_file(), f"dispatch atlas is missing: {path}")
    actions: dict[int, list[dict[str, Any]]] = {}
    constructors: dict[int, list[dict[str, Any]]] = {}
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            rows = csv.DictReader(handle)
            required = {"runtime_class", "table_kind", "selector_word_4", "enabled", "is_null_sentinel", "resolution_status"}
            require(required <= set(rows.fieldnames or ()), f"dispatch atlas lacks fields {sorted(required - set(rows.fieldnames or ())) }")
            for row in rows:
                if row.get("enabled") != "True" or row.get("is_null_sentinel") == "True":
                    continue
                runtime_class = parse_int(row["runtime_class"], "dispatch runtime class")
                selector = parse_int(row["selector_word_4"], "dispatch selector")
                require(0 <= runtime_class <= 255 and 0 <= selector <= 255, "dispatch atlas selector exceeds tuple width")
                item = {"selector": selector, "status": row.get("resolution_status", "") or "unknown"}
                target = actions if row["table_kind"] == "action" else constructors if row["table_kind"] == "constructor" else None
                require(target is not None, f"unknown dispatch table kind {row.get('table_kind')!r}")
                target.setdefault(runtime_class, []).append(item)
    except OSError as exc:
        raise CensusError(f"cannot read dispatch atlas {path}: {exc}") from exc
    for table in (actions, constructors):
        for values in table.values():
            values.sort(key=lambda item: (item["selector"], item["status"]))
    return actions, constructors, sha256_file(path)


def load_manifest(package: Path, expected_level: int) -> tuple[dict[str, Any], dict[str, Any]]:
    require(package.is_dir(), f"package directory is missing: {package}")
    manifest_path = package / "manifest.json"
    manifest = read_json(manifest_path, "package manifest")
    require(isinstance(manifest, dict), "package manifest root is not an object")
    require(manifest.get("schema") == PACKAGE_SCHEMA, f"Level {expected_level} is not a v5 package")
    level = manifest.get("level")
    require(isinstance(level, dict), "package level identity is malformed")
    require(level.get("external") == expected_level and level.get("internal") == expected_level - 1, "package level identity is malformed")
    map_type = level["map_type"]
    require(map_type.get("id") in MAP_TYPES and map_type.get("name") == MAP_TYPES[map_type["id"]], "package map type is not a pinned Day/Night/Cave identity")
    require(manifest.get("byte_order") == "little-endian", "package byte order is not little-endian")
    require(manifest.get("capture") == {"site": "0x002285ff", "frame": 0}, "package capture seam changed")
    counts = manifest.get("counts", {})
    require(counts.get("authored_features") == AUTHORED_COUNT, "authored feature denominator changed")
    require(counts.get("entity_slots") == ENTITY_COUNT, "entity slot denominator changed")
    require(counts.get("players") == 8, "player denominator changed")
    require(isinstance(counts.get("active_entities"), int) and 0 <= counts["active_entities"] <= ENTITY_COUNT, "active entity count is malformed")
    sizes = manifest.get("record_sizes", {})
    require(sizes.get("entity") == ENTITY_BYTES and isinstance(sizes.get("player"), int) and sizes["player"] > 0, "record sizes are not fixed-width")
    source = manifest.get("source")
    require(isinstance(source, dict), "package source provenance is missing")
    require(source.get("upstream_commit") == COMMIT, "package source commit is not pinned")
    for key in ("upstream_worktree_diff_sha256", "compiled_source_sha256"):
        hex64(source.get(key), f"manifest source {key}")
    owner_files = source.get("owner_files")
    require(isinstance(owner_files, list) and owner_files, "package owner-data profile is missing")
    for item in owner_files:
        require(isinstance(item, dict), "owner file identity is not an object")
        require(isinstance(item.get("logical_path"), str) and item["logical_path"], "owner file path is missing")
        require(not Path(item["logical_path"]).is_absolute() and ".." not in Path(item["logical_path"]).parts, "owner file path escapes its root")
        require(isinstance(item.get("bytes"), int) and item["bytes"] >= 0, "owner file size is malformed")
        hex64(item.get("sha256"), f"owner file {item.get('logical_path')} hash")
    files = manifest.get("files")
    require(isinstance(files, dict) and set(files) == REQUIRED_PACKAGE_FILES, "v5 package member set changed")
    actual = {entry.name for entry in package.iterdir() if entry.is_file()}
    require(actual == REQUIRED_PACKAGE_FILES | {"manifest.json"}, "package contains an unknown or missing regular file")
    require(all(entry.is_file() and not entry.is_symlink() for entry in package.iterdir()), "package contains a symlink or non-file member")
    actual_files: dict[str, dict[str, Any]] = {}
    for name, metadata in files.items():
        require(isinstance(metadata, dict), f"manifest member {name} metadata is malformed")
        member = package / name
        data = member.read_bytes()
        require(metadata.get("bytes") == len(data), f"package member {name} size differs from manifest")
        require(metadata.get("sha256") == sha256_bytes(data), f"package member {name} hash differs from manifest")
        actual_files[name] = {"bytes": len(data), "sha256": sha256_bytes(data)}
    require(manifest.get("package_sha256") == member_identity(files), "package identity differs from member hashes")
    return manifest, {"files": actual_files, "source": source}


def parse_authored(data: bytes, external_level: int) -> tuple[list[dict[str, Any]], dict[tuple[int, int, int, int], dict[str, Any]]]:
    expected_bytes = AUTHORED_OFFSET + AUTHORED_COUNT * AUTHORED_BYTES + 1 + 8 * 110 + 8 * 7 + 11 * 8
    require(len(data) == expected_bytes, f"authored-level.bin extent changed: {len(data)} != {expected_bytes}")
    rows: list[dict[str, Any]] = []
    tuples: dict[tuple[int, int, int, int], dict[str, Any]] = {}
    for index in range(AUTHORED_COUNT):
        offset = AUTHORED_OFFSET + index * AUTHORED_BYTES
        raw = data[offset : offset + AUTHORED_BYTES]
        kind, subtype_signed, axis_x, axis_y, dis_id, word10, stage_tag, par1, par2, par3 = struct.unpack("<HhHHhHhHHH", raw)
        if not any(raw):
            continue
        require(0 <= kind <= 255 and 0 <= par1 <= 65535, f"authored row {index} exceeds bounded source-field width")
        subtype = subtype_signed & 0xFF
        action = dis_id if dis_id >= 0 else 0xFF
        require(0 <= action <= 255, f"authored row {index} has an out-of-range DisId")
        row = {
            "authored_record": index,
            "class": kind,
            "action": action,
            "model": par1,
            "subtype": subtype,
            "initial_source_slot": None,
            "initial_dispatch_sites": [DISPATCH_SITES["type_subtype"]] if dis_id == -1 else [DISPATCH_SITES["apply_events"], DISPATCH_SITES["update_entities"]],
            "source_fields": {
                "type_0x30311": kind,
                "subtype_0x30311": subtype_signed,
                "DisId": dis_id,
                "word_10": word10,
                "stageTag_12": stage_tag,
                "par1_14": par1,
                "par2_16": par2,
                "par3_18": par3,
                "axis": {"x": axis_x, "y": axis_y},
            },
            "record_sha256": sha256_bytes(raw),
        }
        rows.append(row)
        key = (kind, action, par1, subtype)
        item = tuples.setdefault(key, {"class": kind, "action": action, "model": par1, "subtype": subtype, "authored_records": []})
        item["authored_records"].append(index)
    rows.sort(key=lambda row: row["authored_record"])
    for item in tuples.values():
        item["occurrences"] = bounded_indices(item.pop("authored_records"))
        item["domains"] = ["authored"]
    return rows, tuples


def parse_reset(data: bytes) -> tuple[list[dict[str, Any]], dict[tuple[int, int, int, int], dict[str, Any]], int]:
    require(data[:4] == b"MCE1", "entities.bin magic mismatch")
    version, record_bytes, count = struct.unpack_from("<HHI", data, 4)
    require((version, record_bytes, count) == (1, ENTITY_BYTES, ENTITY_COUNT), "entities.bin header changed")
    require(len(data) == 12 + ENTITY_COUNT * ENTITY_BYTES, "entities.bin extent changed")
    rows: list[dict[str, Any]] = []
    tuples: dict[tuple[int, int, int, int], dict[str, Any]] = {}
    active_count = 0
    for slot in range(ENTITY_COUNT):
        offset = 12 + slot * ENTITY_BYTES
        raw = data[offset : offset + ENTITY_BYTES]
        require(struct.unpack_from("<H", raw, 0)[0] == slot, f"entity slot {slot} index changed")
        active = bool(raw[2])
        if not active:
            require(raw[67] == 0, f"inactive entity slot {slot} carries a live class")
            continue
        active_count += 1
        class_id, model, action, subtype_signed = raw[67], raw[68], raw[73], signed8(raw[70])
        subtype = raw[70]
        row = {
            "slot": slot,
            "class": class_id,
            "action": action,
            "model": model,
            "subtype": subtype,
            "source_fields": {"class_offset": 67, "model_offset": 68, "action_offset": 73, "xsubtype_offset": 70, "xsubtype_signed": subtype_signed},
            "record_sha256": sha256_bytes(raw),
        }
        rows.append(row)
        key = (class_id, action, model, subtype)
        item = tuples.setdefault(key, {"class": class_id, "action": action, "model": model, "subtype": subtype, "reset_slots": []})
        item["reset_slots"].append(slot)
    for item in tuples.values():
        item["occurrences"] = bounded_indices(item.pop("reset_slots"))
        item["domains"] = ["reset"]
    return rows, tuples, active_count


def merge_tuples(authored: Mapping[tuple[int, int, int, int], Mapping[str, Any]], reset: Mapping[tuple[int, int, int, int], Mapping[str, Any]]) -> list[dict[str, Any]]:
    merged: dict[tuple[int, int, int, int], dict[str, Any]] = {}
    for domain, values in (("authored", authored), ("reset", reset)):
        for key, item in values.items():
            target = merged.setdefault(key, {"class": key[0], "action": key[1], "model": key[2], "subtype": key[3], "domains": [], "occurrences": {}})
            target["domains"].append(domain)
            target["occurrences"][domain] = item["occurrences"]
    return [merged[key] for key in sorted(merged)]


def parse_runtime(data: bytes) -> dict[str, Any]:
    require(data[:4] == b"MCR1" and len(data) == RUNTIME_BYTES, "runtime.bin layout changed")
    result: dict[str, Any] = {}
    for section, (offset, count, record_bytes) in RUNTIME_SECTIONS.items():
        rows: list[dict[str, Any]] = []
        for index in range(count):
            start = offset + index * record_bytes
            raw = data[start : start + record_bytes]
            if section == "stages":
                fields = {"type": raw[0], "flags": raw[1], "scalar": struct.unpack_from("<I", raw, 2)[0], "domain": raw[6], "relation": struct.unpack_from("<I", raw, 10)[0]}
            elif section == "objectives":
                fields = {"objective_type": raw[0], "flags": list(raw[1:8]), "relation": struct.unpack_from("<H", raw, 8)[0], "parameter": raw[10]}
            elif section == "stage_variables":
                fields = {"type": raw[0], "flags": raw[1], "selector": struct.unpack_from("<H", raw, 2)[0], "domain": raw[4], "relation": struct.unpack_from("<I", raw, 8)[0]}
            elif section == "events":
                fields = {"event_type": raw[0], "domain": raw[10], "relation": struct.unpack_from("<I", raw, 14)[0], "payload_sha256": sha256_bytes(raw[18:])}
            else:
                fields = {"enabled": raw[0], "payload_sha256": sha256_bytes(raw[1:])}
            rows.append({"index": index, "fields": fields, "record_sha256": sha256_bytes(raw)})
        result[section] = {"offset": offset, "count": count, "record_bytes": record_bytes, "rows": rows}
    return result


def parse_authored_stages(data: bytes) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    stages: list[dict[str, Any]] = []
    for index in range(8):
        offset = AUTHORED_OFFSET + AUTHORED_COUNT * AUTHORED_BYTES + 1 + 8 * 110 + index * 7
        raw = data[offset : offset + 7]
        stages.append({"index": index, "stage_index": signed8(raw[0]), "objective_selector": struct.unpack_from("<h", raw, 1)[0], "axis": {"x": struct.unpack_from("<h", raw, 3)[0], "y": struct.unpack_from("<h", raw, 5)[0]}, "record_sha256": sha256_bytes(raw)})
    variables: list[dict[str, Any]] = []
    offset_base = AUTHORED_OFFSET + AUTHORED_COUNT * AUTHORED_BYTES + 1 + 8 * 110 + 8 * 7
    for index in range(11):
        offset = offset_base + index * 8
        raw = data[offset : offset + 8]
        variables.append({"index": index, "index_value": signed8(raw[0]), "stage": signed8(raw[1]), "selector": struct.unpack_from("<H", raw, 2)[0], "axis_or_relation": {"x": struct.unpack_from("<H", raw, 4)[0], "y": struct.unpack_from("<H", raw, 6)[0]}, "record_sha256": sha256_bytes(raw)})
    return stages, variables


def parse_collision(
    manifest: Mapping[str, Any],
    package: Path,
    authored_rows: Sequence[Mapping[str, Any]],
    external_level: int,
) -> dict[str, Any]:
    metadata = manifest.get("collision_masks")
    require(isinstance(metadata, dict), f"external Level {external_level} collision-mask metadata is missing")
    require(metadata.get("file") == "collision-masks.bin" and metadata.get("magic") == "MCM1", "collision-mask metadata identity changed")
    member = package / "collision-masks.bin"
    require(member.is_file(), f"external Level {external_level} collision-mask member is absent")
    data = member.read_bytes()
    require(len(data) >= 20, "collision-masks.bin header is truncated")
    require(data[:4] == b"MCM1", "collision-masks.bin magic mismatch")
    version, header_bytes, vga, reserved, count, record_header = struct.unpack_from("<HHHHII", data, 4)
    require((version, header_bytes, vga, reserved, record_header) == (1, 20, metadata.get("vga_resolution_selector"), 0, 12), "collision-mask header changed")
    selectors: list[dict[str, Any]] = []
    offset = header_bytes
    previous_selector = -1
    for _ in range(count):
        require(offset + record_header <= len(data), "collision-mask record header is truncated")
        selector, width, height, reserved_record, pixel_bytes = struct.unpack_from("<HHHHI", data, offset)
        require(selector > previous_selector, "collision-mask selectors are not strictly ordered")
        require(width > 0 and height > 0, "collision-mask dimensions are empty")
        require(reserved_record == 0 and pixel_bytes == width * height * 2, "collision-mask record extent changed")
        payload = data[offset + record_header : offset + record_header + pixel_bytes]
        require(len(payload) == pixel_bytes, "collision-mask record is truncated")
        selectors.append({"selector": selector, "width": width, "height": height, "pixel_bytes": pixel_bytes, "sha256": sha256_bytes(payload)})
        offset += record_header + pixel_bytes
        previous_selector = selector
    require(offset == len(data), "collision-masks.bin has an unknown tail")
    declared = metadata.get("selectors")
    require(declared == selectors, "collision-mask metadata differs from decoded records")
    model_45_rows = [
        row
        for row in authored_rows
        if row["class"] == 10
        and row["subtype"] == 45
        and row["source_fields"]["DisId"] == -1
    ]
    expected_selectors = sorted({row["model"] for row in model_45_rows})
    require(
        [row["selector"] for row in selectors] == expected_selectors,
        f"external Level {external_level} collision-mask selectors differ from authored rows",
    )
    if external_level >= 3:
        recorded_count = metadata.get("authored_model_45_row_count")
        require(
            isinstance(recorded_count, int)
            and not isinstance(recorded_count, bool)
            and recorded_count >= 0,
            f"external Level {external_level} authored model-45 row count is missing",
        )
        require(
            recorded_count == len(model_45_rows),
            f"external Level {external_level} authored model-45 row count disagrees with authored data",
        )
    return {
        "member": "collision-masks.bin",
        "member_sha256": sha256_bytes(data),
        "selectors": selectors,
        "authored_model_45_row_count": len(model_45_rows),
        "source_files": metadata.get("source_files"),
        "selector_source": metadata.get("selector_source"),
    }


def parse_feature_inputs(manifest: Mapping[str, Any], package: Path) -> dict[str, Any]:
    descriptor = manifest.get("feature_generation_oracle", {}).get("construction_inputs")
    require(isinstance(descriptor, dict), "feature construction input descriptor is missing")
    require(descriptor.get("file") == "feature-construction-inputs.bin" and descriptor.get("magic") == "MCI3", "feature construction input identity changed")
    data = (package / "feature-construction-inputs.bin").read_bytes()
    require(data[:4] == b"MCI3", "feature-construction-inputs.bin magic mismatch")
    version, record_bytes, count = struct.unpack_from("<HHH", data, 4)
    require((version, record_bytes, count) == (1, 16, descriptor.get("record_count")), "feature construction input header changed")
    require(record_bytes == 16, "feature construction input record descriptor changed")
    selectors = []
    offset = 16
    for _ in range(count):
        selector, _scalar, _flags, _successor, width, height, extent = struct.unpack_from("<HHBBBBI", data, offset)
        require(width > 0 and height > 0 and extent == width * height * 2, "feature construction footprint extent changed")
        require(offset + 12 + extent <= len(data), "feature construction input record is truncated")
        selectors.append(selector)
        offset += 12 + extent
    require(offset == len(data), "feature-construction-inputs.bin has an unknown tail")
    require(sorted(selectors) == descriptor.get("selectors"), "feature construction selectors differ from manifest")
    return {"member": "feature-construction-inputs.bin", "member_sha256": sha256_bytes(data), "selectors": sorted(selectors), "source": descriptor.get("source")}


def owner_artifacts(manifest: Mapping[str, Any]) -> dict[str, dict[str, Any]]:
    return {item["logical_path"]: {"bytes": item["bytes"], "sha256": item["sha256"]} for item in manifest["source"]["owner_files"]}


def resolved_owner_path(value: Any) -> str | None:
    if not isinstance(value, str) or not value.startswith("owner:"):
        return None
    return value[6:]


def source_facts(manifest: Mapping[str, Any], package: Path, authored_rows: Sequence[Mapping[str, Any]], collision: Mapping[str, Any], construction: Mapping[str, Any]) -> list[dict[str, Any]]:
    owner = owner_artifacts(manifest)
    relevant: set[str] = set()
    for item in owner:
        base = Path(item).name
        if "LEVELS" in base or base in {"BUILD0-0.DAT", "BUILD0-0.TAB", "TABLESN.DAT", "SPELLS.DAT", "MSPRD0-0.DAT", "MSPRD0-0.TAB"}:
            relevant.add(item)
    presentation = manifest.get("presentation_dependencies", {})
    for value in (presentation.get("resolved_files", {}) if isinstance(presentation, dict) else {}).values():
        path = resolved_owner_path(value)
        if path is not None and path in owner:
            relevant.add(path)
    for item in collision.get("source_files", {}).values():
        if isinstance(item, dict) and item.get("logical_path") in owner:
            relevant.add(item["logical_path"])
    artifacts = [{"logical_path": path, **owner[path]} for path in sorted(relevant)]
    level_records = [item for item in artifacts if Path(item["logical_path"]).name.startswith("LEVELS.")]
    construction_artifacts = [item for item in artifacts if Path(item["logical_path"]).name in {"BUILD0-0.DAT", "BUILD0-0.TAB", "TABLESN.DAT", "SPELLS.DAT"}]
    mask_artifacts = [item for item in artifacts if Path(item["logical_path"]).name in {"MSPRD0-0.DAT", "MSPRD0-0.TAB"}]
    selector_provenance = []
    for selector in collision["selectors"]:
        matching = [row["authored_record"] for row in authored_rows if row["class"] == 10 and row["subtype"] == 45 and row["model"] == selector["selector"]]
        selector_provenance.append({"source_class": 10, "source_model": 45, "source_subtype": 45, "source_field": "par1_14", "selector_value": selector["selector"], "authored_records": matching})
    facts = [
        {"fact_id": "level-records", "kind": "authored_level_records", "version": 1, "member": "authored-level.bin", "member_sha256": sha256_file(package / "authored-level.bin"), "selector_provenance": [], "source_artifacts": level_records, "lifetime": "immutable-through-session"},
        {"fact_id": "feature-construction-inputs", "kind": "feature_construction_inputs", "version": 1, "member": construction["member"], "member_sha256": construction["member_sha256"], "selector_provenance": [{"source_field": "building_selector", "selector_value": selector} for selector in construction["selectors"]], "source_artifacts": construction_artifacts, "lifetime": "immutable-through-session"},
        {"fact_id": "collision-mask-bank", "kind": "collision_mask", "version": 1, "member": collision["member"], "member_sha256": collision["member_sha256"], "authored_model_45_row_count": collision["authored_model_45_row_count"], "selector_provenance": selector_provenance, "source_artifacts": mask_artifacts, "lifetime": "immutable-through-session"},
    ]
    for fact in facts:
        require(fact["source_artifacts"], f"source fact {fact['fact_id']} has no owner artifact")
    return facts


def behavior_closure(
    authored_rows: Sequence[Mapping[str, Any]],
    reset_rows: Sequence[Mapping[str, Any]],
    action_atlas: Mapping[int, Sequence[Mapping[str, Any]]],
    constructor_atlas: Mapping[int, Sequence[Mapping[str, Any]]],
) -> dict[str, Any]:
    classes = sorted({int(row["class"]) for row in authored_rows} | {int(row["class"]) for row in reset_rows})
    initial: set[tuple[int, str, int, int]] = set()
    for row in authored_rows:
        if row["source_fields"]["DisId"] == -1:
            initial.add((DISPATCH_SITES["type_subtype"], "TypeSubtype", row["class"], row["subtype"]))
    for row in reset_rows:
        initial.add((DISPATCH_SITES["apply_events"], "ClassAction", row["class"], row["action"]))
        initial.add((DISPATCH_SITES["update_entities"], "ClassAction", row["class"], row["action"]))
        initial.add((DISPATCH_SITES["constructor"], "ClassModel", row["class"], row["model"]))
    closure = set(initial)
    requirements: list[dict[str, Any]] = []
    for runtime_class in classes:
        for item in action_atlas.get(runtime_class, ()):
            selector = int(item["selector"])
            for site_name in ("apply_events", "update_entities"):
                key = (DISPATCH_SITES[site_name], "ClassAction", runtime_class, selector)
                closure.add(key)
                requirements.append({"dispatch_site": key[0], "selector": {"kind": key[1], "class": runtime_class, "action": selector}, "source_status": item["status"]})
        for item in constructor_atlas.get(runtime_class, ()):
            selector = int(item["selector"])
            key = (DISPATCH_SITES["constructor"], "ClassModel", runtime_class, selector)
            closure.add(key)
            requirements.append({"dispatch_site": key[0], "selector": {"kind": key[1], "class": runtime_class, "model": selector}, "source_status": item["status"]})
    for site, kind, class_id, value in sorted(closure):
        if kind == "TypeSubtype":
            selector = {"kind": kind, "type_id": class_id, "subtype": value}
        elif kind == "ClassAction":
            selector = {"kind": kind, "class": class_id, "action": value}
        else:
            selector = {"kind": kind, "class": class_id, "model": value}
        requirements.append({"dispatch_site": site, "selector": selector})
    unique: dict[str, dict[str, Any]] = {}
    for item in requirements:
        key = canonical_sha256({"dispatch_site": item["dispatch_site"], "selector": item["selector"]})
        unique.setdefault(key, item)
    ordered = sorted(unique.values(), key=lambda item: (item["dispatch_site"], json.dumps(item["selector"], sort_keys=True)))
    initial_keys = [{"dispatch_site": site, "selector": ({"kind": "TypeSubtype", "type_id": class_id, "subtype": value} if kind == "TypeSubtype" else {"kind": kind, "class": class_id, "action": value} if kind == "ClassAction" else {"kind": kind, "class": class_id, "model": value})} for site, kind, class_id, value in sorted(initial)]
    declared = [{"dispatch_site": item["dispatch_site"], "selector": item["selector"]} for item in ordered]
    return {
        "method": "conservative-enabled-table-row-union-per-observed-class",
        "initial": initial_keys,
        "closure": declared,
        "initial_key_count": len(initial_keys),
        "closure_key_count": len(declared),
        "unresolved_source_status_count": sum(1 for item in ordered if item.get("source_status") not in (None, "ENABLED_RESOLVED", "ENABLED_DISPATCH_CASE_WITHOUT_CALL")),
        "nonclaim": "This is a bounded source-table closure for observed classes; it does not claim that every dynamic predicate or transition has been simulated.",
    }


def closure_fixpoint(initial: Iterable[str], successors: Mapping[str, Iterable[str]]) -> list[str]:
    result = set(initial)
    pending = list(result)
    while pending:
        current = pending.pop()
        for successor in successors.get(current, ()):
            if successor not in result:
                result.add(successor)
                pending.append(successor)
    return sorted(result)


def validate_declared_closure(initial: Iterable[str], successors: Mapping[str, Iterable[str]], declared: Iterable[str]) -> None:
    expected = closure_fixpoint(initial, successors)
    actual = sorted(set(declared))
    require(actual == list(declared), "declared behavior closure is not sorted and unique")
    require(actual == expected, f"declared behavior closure differs from transitive fixpoint: expected {expected}, got {actual}")


def baseline_capability(census_path: Path, baseline_package: Path | None) -> dict[str, Any]:
    baseline = read_json(census_path, "accepted Level 2 census")
    require(isinstance(baseline, dict) and baseline.get("schema") == "mc2-level2-authored-reset-census/v1", "accepted Level 2 census schema is not pinned")
    tuple_set: set[tuple[int, int, int, int]] = set()
    for row in baseline.get("reset", {}).get("slots", []):
        if row.get("active"):
            tuple_set.add((row["class"], row["action"], row["model"], row["xsubtype"] & 0xFF))
    objective_selectors = sorted({row["objective_selector"] for row in baseline.get("stages", [])})
    runtime_stage_types = sorted({row["fields"]["type"] for row in baseline.get("runtime", {}).get("stages", {}).get("rows", [])})
    runtime_objective_types = sorted({row["fields"]["objective_type"] for row in baseline.get("runtime", {}).get("objectives", {}).get("rows", [])})
    stage_variable_selectors = sorted({row["fields"]["selector"] for row in baseline.get("runtime", {}).get("stage_variables", {}).get("rows", [])})
    owner_files = baseline.get("qualified_exports", [{}])[0].get("source", {}).get("owner_files", [])
    owner_map = {item.get("logical_path"): item for item in owner_files if isinstance(item, dict)}
    collision_selectors: list[int] = []
    construction_selectors: list[int] = []
    baseline_package_identity: str | None = None
    if baseline_package is not None and baseline_package.is_dir():
        manifest = read_json(baseline_package / "manifest.json", "accepted Level 2 package manifest")
        baseline_package_identity = manifest.get("package_sha256")
        collision_selectors = [item["selector"] for item in manifest.get("collision_masks", {}).get("selectors", [])]
        construction_selectors = list(manifest.get("feature_generation_oracle", {}).get("construction_inputs", {}).get("selectors", []))
    return {
        "source": {"census": census_path.as_posix(), "census_sha256": sha256_file(census_path), "baseline_package": baseline_package.as_posix() if baseline_package else None, "baseline_package_sha256": baseline_package_identity},
        "profiles": [{"external": 1, "internal": 0, "map_type_id": 1}, {"external": 2, "internal": 1, "map_type_id": 0}],
        "tuple_keys": [list(key) for key in sorted(tuple_set)],
        "objective_forms": {"authored_stage_objective_selectors": objective_selectors, "runtime_stage_types": runtime_stage_types, "runtime_objective_types": runtime_objective_types, "stage_variable_selectors": stage_variable_selectors},
        "source_fact_kinds": ["authored_level_records", "feature_construction_inputs", "collision_mask"],
        "source_fact_selectors": {"collision_mask": sorted(collision_selectors), "feature_construction_inputs": sorted(construction_selectors)},
        "owner_artifacts": [{"logical_path": path, "bytes": item.get("bytes"), "sha256": item.get("sha256")} for path, item in sorted(owner_map.items())],
        "nonclaim": "The Level 2 receipt is an accepted evidence baseline for tuple/form/fact comparison; it does not imply that a remaining realm is playable.",
    }


def diff_against_capability(
    level: Mapping[str, Any],
    tuples: Sequence[Mapping[str, Any]],
    runtime: Mapping[str, Any],
    authored_stages: Sequence[Mapping[str, Any]],
    facts: Sequence[Mapping[str, Any]],
    capability: Mapping[str, Any],
) -> dict[str, Any]:
    baseline_tuple_keys = {tuple(item) for item in capability["tuple_keys"]}
    observed_tuple_keys = {tuple_key(item) for item in tuples}
    observed_forms = {
        "authored_stage_objective_selectors": sorted({row["objective_selector"] for row in authored_stages}),
        "runtime_stage_types": sorted({row["fields"]["type"] for row in runtime["stages"]["rows"]}),
        "runtime_objective_types": sorted({row["fields"]["objective_type"] for row in runtime["objectives"]["rows"]}),
        "stage_variable_selectors": sorted({row["fields"]["selector"] for row in runtime["stage_variables"]["rows"]}),
    }
    baseline_forms = capability["objective_forms"]
    form_diff = {name: {"observed": values, "baseline": baseline_forms[name], "new_values": sorted(set(values) - set(baseline_forms[name])), "missing_from_observed": sorted(set(baseline_forms[name]) - set(values))} for name, values in observed_forms.items()}
    baseline_artifacts = {(item["logical_path"], item["sha256"]) for item in capability["owner_artifacts"]}
    fact_diff = []
    for fact in facts:
        baseline_values = capability["source_fact_selectors"].get(fact["kind"], [])
        values = sorted({item.get("selector_value") for item in fact["selector_provenance"] if isinstance(item.get("selector_value"), int)})
        artifacts = {(item["logical_path"], item["sha256"]) for item in fact["source_artifacts"]}
        fact_diff.append({"fact_id": fact["fact_id"], "kind": fact["kind"], "implementation_status": "known_kind" if fact["kind"] in capability["source_fact_kinds"] else "missing_kind", "baseline_selector_values": baseline_values, "observed_selector_values": values, "new_selector_values": sorted(set(values) - set(baseline_values)), "new_source_artifacts": [{"logical_path": path, "sha256": digest} for path, digest in sorted(artifacts - baseline_artifacts)]})
    profile_key = {"external": level["external"], "internal": level["internal"], "map_type_id": level["map_type"]["id"]}
    return {
        "profile": {"observed": profile_key, "status": "implemented_profile" if profile_key in capability["profiles"] else "missing_implemented_profile"},
        "tuples": {"baseline_count": len(baseline_tuple_keys), "observed_count": len(observed_tuple_keys), "known_baseline_tuples": [list(key) for key in sorted(observed_tuple_keys & baseline_tuple_keys)], "new_tuples": [list(key) for key in sorted(observed_tuple_keys - baseline_tuple_keys)]},
        "objective_forms": form_diff,
        "source_facts": fact_diff,
    }


def package_evidence(package: Path, expected_level: int, root: Path, atlas: tuple[dict[int, list[dict[str, Any]]], dict[int, list[dict[str, Any]]], str], capability: Mapping[str, Any]) -> dict[str, Any]:
    manifest, metadata = load_manifest(package, expected_level)
    authored_data = (package / "authored-level.bin").read_bytes()
    require(authored_data[6] == manifest["level"]["map_type"]["id"], "authored map type differs from manifest")
    authored_rows, authored_tuples = parse_authored(authored_data, expected_level)
    reset_rows, reset_tuples, active_count = parse_reset((package / "entities.bin").read_bytes())
    require(active_count == manifest["counts"]["active_entities"], "active entity count differs from manifest")
    post_data = (package / "entities-post-features.bin").read_bytes()
    require(post_data[:4] == b"MCE3", "entities-post-features.bin magic mismatch")
    players = (package / "players.bin").read_bytes()
    player_version, player_bytes, player_count = struct.unpack_from("<HHI", players, 4)
    require(players[:4] == b"MCP1" and player_count == 8 and player_version in (1, 2, 3, 4) and player_bytes == manifest["record_sizes"]["player"], "players.bin header changed")
    require(len(players) == 12 + player_count * player_bytes, "players.bin extent changed")
    runtime = parse_runtime((package / "runtime.bin").read_bytes())
    authored_stages, authored_stage_variables = parse_authored_stages(authored_data)
    collision = parse_collision(manifest, package, authored_rows, expected_level)
    construction = parse_feature_inputs(manifest, package)
    facts = source_facts(manifest, package, authored_rows, collision, construction)
    actions, constructors, atlas_sha = atlas
    merged = merge_tuples(authored_tuples, reset_tuples)
    behavior = behavior_closure(authored_rows, reset_rows, actions, constructors)
    observed = {
        "authored": {"nonzero_record_count": len(authored_rows), "tuple_count": len(authored_tuples), "rows": authored_rows, "tuples": [authored_tuples[key] for key in sorted(authored_tuples)]},
        "reset": {"active_count": active_count, "tuple_count": len(reset_tuples), "rows": reset_rows, "tuples": [reset_tuples[key] for key in sorted(reset_tuples)]},
        "class_action_model_subtype": merged,
        "objective_form": {"authored_stages": authored_stages, "authored_stage_variables": authored_stage_variables, "runtime": runtime},
        "map_type_gate": {"observed": manifest["level"]["map_type"], "accepted_map_type_ids": sorted(MAP_TYPES)},
        "required_source_facts": facts,
        "behavior_closure": behavior,
        "presentation": {key: manifest.get("presentation_dependencies", {}).get(key) for key in ("map_type", "terrain_bank", "sprite_bank", "web_bank", "palette", "sky_bank", "vga_resolution_selector", "texture_size", "resolved_files")},
    }
    file_hashes = {name: metadata["files"][name]["sha256"] for name in sorted(metadata["files"])}
    source = metadata["source"]
    identity = {
        "manifest_sha256": sha256_file(package / "manifest.json"),
        "package_sha256": manifest["package_sha256"],
        "file_sha256": file_hashes,
        "source_upstream_commit": source["upstream_commit"],
        "source_compiled_sha256": source["compiled_source_sha256"],
        "owner_files_sha256": canonical_sha256(sorted(source["owner_files"], key=lambda item: item["logical_path"])),
    }
    return {
        "external_level": expected_level,
        "package": repo_relative(package, root),
        "identity": identity,
        "source_owner_files": sorted(source["owner_files"], key=lambda item: item["logical_path"]),
        "map_type": manifest["level"]["map_type"],
        "counts": manifest["counts"],
        "record_sizes": manifest["record_sizes"],
        "atlas_sha256": atlas_sha,
        "observed": observed,
        "diff": diff_against_capability(manifest["level"], merged, runtime, authored_stages, facts, capability),
    }


def compare_twins(first: Mapping[str, Any], second: Mapping[str, Any]) -> None:
    require(first["identity"] == second["identity"], f"twin package identities differ for Level {first['external_level']}")
    require(first["source_owner_files"] == second["source_owner_files"], f"twin owner-data profiles differ for Level {first['external_level']}")
    require(canonical_bytes(first["observed"]) == canonical_bytes(second["observed"]), f"twin census observations differ for Level {first['external_level']}")


def validate_receipt(value: Mapping[str, Any]) -> None:
    require(value.get("schema") == SCHEMA, "census schema mismatch")
    scope = value.get("scope", {})
    require(scope.get("external_levels") == list(range(3, 25)), "census scope is not exactly Levels 3..24")
    require(scope.get("source_commit") == COMMIT and scope.get("original_executable_run") is False and scope.get("implementation_changes") is False, "census scope claims are malformed")
    levels = value.get("levels")
    require(isinstance(levels, list) and [item.get("external_level") for item in levels] == list(range(3, 25)), "census does not contain one ordered entry per remaining level")
    for item in levels:
        require(item.get("byte_identical_twins") is True, f"Level {item.get('external_level')} is not byte-identical across twins")
        for side in ("run_a", "run_b"):
            identity = item.get(side, {}).get("identity", {})
            hex64(identity.get("manifest_sha256"), f"Level {item.get('external_level')} {side} manifest hash")
            hex64(identity.get("package_sha256"), f"Level {item.get('external_level')} {side} package hash")
        closure = item.get("observed", {}).get("behavior_closure", {})
        declared = closure.get("closure", [])
        require(declared == sorted(declared, key=lambda row: (row["dispatch_site"], json.dumps(row["selector"], sort_keys=True))), f"Level {item.get('external_level')} behavior closure is not canonical")
        facts = item.get("observed", {}).get("required_source_facts", [])
        require(len({fact.get("fact_id") for fact in facts}) == len(facts), f"Level {item.get('external_level')} has duplicate source facts")
        for fact in facts:
            hex64(fact.get("member_sha256"), f"Level {item.get('external_level')} source fact member hash")
            for artifact in fact.get("source_artifacts", []):
                hex64(artifact.get("sha256"), f"Level {item.get('external_level')} source artifact hash")
    aggregate = value.get("aggregate", {})
    require(aggregate.get("level_count") == 22 and aggregate.get("byte_identical_level_count") == 22, "aggregate twin count is incomplete")


def build_receipt(args: argparse.Namespace, root: Path) -> dict[str, Any]:
    levels = level_list(args.levels)
    exporter = args.exporter.resolve(strict=True)
    owner_root = args.owner_root.resolve(strict=True)
    run_a = args.run_a.resolve()
    run_b = args.run_b.resolve()
    for output in (run_a, run_b):
        require(output != owner_root and output.is_relative_to(root / "research/private/generated"), f"generated output must stay below research/private/generated: {output}")
        require(not output.exists(), f"refusing to overwrite existing census output: {output}")
        output.mkdir(parents=True, exist_ok=False)
    baseline_census = args.baseline_census.resolve(strict=True)
    baseline_package = args.baseline_package.resolve() if args.baseline_package else None
    capability = baseline_capability(baseline_census, baseline_package)
    atlas = read_dispatch_atlas(args.dispatch_atlas.resolve(strict=True))
    entries: list[dict[str, Any]] = []
    for external_level in levels:
        package_a = run_a / f"level{external_level}"
        package_b = run_b / f"level{external_level}"
        for output in (package_a, package_b):
            command = [str(exporter), "--owner-root", str(owner_root), "--output", str(output), "--level", str(external_level)]
            try:
                result = subprocess.run(command, cwd=root, text=True, capture_output=True, check=False, timeout=300)
            except subprocess.TimeoutExpired as exc:
                raise CensusError(f"exporter timed out for Level {external_level} after 300 seconds") from exc
            if result.returncode != 0:
                raise CensusError(f"exporter failed for Level {external_level}:\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}")
            require(output.is_dir(), f"exporter did not create Level {external_level} package {output}")
        first = package_evidence(package_a, external_level, root, atlas, capability)
        second = package_evidence(package_b, external_level, root, atlas, capability)
        compare_twins(first, second)
        entries.append({"external_level": external_level, "byte_identical_twins": True, "map_type": first["map_type"], "run_a": first, "run_b": second, "observed": first["observed"], "diff": first["diff"]})
        print(f"census: Level {external_level} exported twice and matched", file=sys.stderr)
    aggregate = {
        "level_count": len(entries),
        "byte_identical_level_count": sum(1 for item in entries if item["byte_identical_twins"]),
        "active_entity_total": sum(item["observed"]["reset"]["active_count"] for item in entries),
        "authored_nonzero_total": sum(item["observed"]["authored"]["nonzero_record_count"] for item in entries),
        "distinct_tuple_total": sum(len(item["observed"]["class_action_model_subtype"]) for item in entries),
        "distinct_map_types": sorted({(item["map_type"]["id"], item["map_type"]["name"]) for item in entries}),
        "package_identity_sha256": canonical_sha256({item["external_level"]: item["run_a"]["identity"]["package_sha256"] for item in entries}),
    }
    receipt = {
        "schema": SCHEMA,
        "scope": {"external_levels": levels, "source_commit": COMMIT, "original_executable_run": False, "implementation_changes": False, "generated_packages": "private ignored output only"},
        "inputs": {"exporter": {"path": repo_relative(exporter, root), "sha256": sha256_file(exporter)}, "owner_root": {"path": repo_relative(owner_root, root), "required_directories": ["GAME/NETHERW", "CD_Files"], "profile": "remc2-native-runtime"}, "run_a": repo_relative(run_a, root), "run_b": repo_relative(run_b, root), "dispatch_atlas": {"path": repo_relative(args.dispatch_atlas, root), "sha256": atlas[2]}, "accepted_capability": capability},
        "implemented_capability": {"profiles": capability["profiles"], "accepted_tuple_key_count": len(capability["tuple_keys"]), "accepted_objective_forms": capability["objective_forms"], "accepted_source_fact_kinds": capability["source_fact_kinds"], "nonclaim": "A Level 2 evidence match is not a Level 3 implementation claim; every remaining profile is reported separately as unsupported until a later fold slice lands."},
        "levels": entries,
        "aggregate": aggregate,
        "tool": {"path": repo_relative(Path(__file__), root), "sha256": sha256_file(Path(__file__))},
    }
    validate_receipt(receipt)
    return receipt


def summary_markdown(receipt: Mapping[str, Any]) -> str:
    lines = [
        "# Remaining campaign census",
        "",
        "This is a source-shaped, bounded census of external Levels 3–24. Each package was produced twice by the pinned private exporter and the complete package trees matched byte-for-byte. No original executable, DOSBox session, decompiler, simulation implementation, or ABI change is part of this receipt.",
        "",
        f"- remc2 specification commit: `{receipt['scope']['source_commit']}`",
        f"- levels: `{receipt['scope']['external_levels'][0]}..{receipt['scope']['external_levels'][-1]}` ({receipt['aggregate']['level_count']} entries)",
        f"- byte-identical twin levels: `{receipt['aggregate']['byte_identical_level_count']}`",
        f"- generated roots (ignored/private): `{receipt['inputs']['run_a']}`, `{receipt['inputs']['run_b']}`",
        "- committed data is metadata only: package/member/source hashes, bounded tuples, objective forms, map gates, and source-fact diffs.",
        "",
        "## Per-level inventory",
        "",
        "| Level | Internal | Map | Active entities | Authored rows | Tuple keys | Closure keys | New tuple keys | New objective values | Source-fact deltas |",
        "|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for item in receipt["levels"]:
        diff = item["diff"]
        objective_new = sum(len(value["new_values"]) for value in diff["objective_forms"].values())
        source_new = sum(len(value["new_selector_values"]) + len(value["new_source_artifacts"]) for value in diff["source_facts"])
        lines.append(f"| {item['external_level']} | {item['external_level'] - 1} | {item['map_type']['name']} | {item['observed']['reset']['active_count']} | {item['observed']['authored']['nonzero_record_count']} | {len(item['observed']['class_action_model_subtype'])} | {item['observed']['behavior_closure']['closure_key_count']} | {len(diff['tuples']['new_tuples'])} | {objective_new} | {source_new} |")
    lines.extend(["", "## Bounded diff notes", "", "The tuple inventory uses the source-shaped order `class/action/model/subtype`. For authored rows those fields are `type_0x30311 / DisId (0xFF represents source -1) / par1_14 / subtype_0x30311`; reset rows use the captured entity `class / action / model / xsubtype` bytes. Both domains remain visible so a field-vocabulary collision is not silently normalized.", "", "The behavior closure is the union of enabled source action and constructor table rows for classes observed in the packages, plus authored type/subtype selectors. This is conservative and deterministic; it is not a claim that dynamic predicates, callback effects, or all transitive runtime transitions are implemented.", "", "Map gates are reported as exact exporter identities. Current implemented profile capability remains external Level 1/internal 0/Night and external Level 2/internal 1/Day; each Level 3–24 row therefore reports a missing implemented profile even when its map type or tuple shape matches the accepted Level 2 baseline.", "", "Required source facts include authored level records, feature-construction inputs, and collision-mask selectors. The diff distinguishes known fact kinds from newly observed selector values and owner-artifact identities. Presentation dependencies are recorded separately and are never treated as simulation authority.", ""])
    return "\n".join(lines)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exporter", type=Path, default=Path("/tmp/magiccarpet-remc2-level-exporter-build/remc2-level-exporter"))
    parser.add_argument("--owner-root", type=Path, default=Path("research/private/remc2-native-runtime"))
    parser.add_argument("--run-a", type=Path, default=Path("research/private/generated/campaign-census-run-a"))
    parser.add_argument("--run-b", type=Path, default=Path("research/private/generated/campaign-census-run-b"))
    parser.add_argument("--dispatch-atlas", type=Path, default=Path("research/private/remc2-atlas/semantics/generated/runtime-dispatch-rows.csv"))
    parser.add_argument("--baseline-census", type=Path, default=Path("research/private/remc2-sim-rs/level2-conformance/census/level2-authored-reset-census.json"))
    parser.add_argument("--baseline-package", type=Path, default=Path("research/private/generated/l2-debug3.fJ3PaM/level2"))
    parser.add_argument("--receipt", type=Path, default=Path("research/private/remc2-level-exporter/census/campaign-census.json"))
    parser.add_argument("--summary", type=Path, default=Path("research/private/generated/campaign-census.md"))
    parser.add_argument("--levels", default="3-24")
    parser.add_argument("--validate", type=Path, help="validate an existing committed receipt without exporting")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    root = Path(__file__).resolve().parents[4]
    try:
        if args.validate is not None:
            validate_receipt(read_json(args.validate.resolve(strict=True), "census receipt"))
            print(f"validated {args.validate}")
            return 0
        receipt = build_receipt(args, root)
        args.receipt.parent.mkdir(parents=True, exist_ok=True)
        args.summary.parent.mkdir(parents=True, exist_ok=True)
        args.receipt.write_bytes(canonical_bytes(receipt))
        args.summary.write_text(summary_markdown(receipt), encoding="utf-8")
        print(f"wrote {args.receipt}")
        print(f"wrote {args.summary}")
        return 0
    except CensusError as exc:
        print(f"campaign_census.py: error: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
