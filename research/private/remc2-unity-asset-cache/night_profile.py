"""Source-verified Night cache admission for the bank-bearing MC2 package.

The visual qualification is a reference, never the identity of the current
runtime. MCR1/v2 retains every v1 field and appends constructor counters; those
counters and the collision bank remain in the production package unchanged.
"""
from __future__ import annotations

import copy
import hashlib
import json
import struct
from pathlib import Path

SCHEMA = "mc2-owner-unity-asset-cache/v2"
BINDING_KEYS = tuple(sorted((
    "level_export_package_sha256", "campaign_package_sha256",
    "level_asset_closure_sha256", "owner_catalog_sha256",
    "owner_catalog_file_sha256", "qualified_runtime_capture_sha256",
    "qualified_runtime_capture_count", "qualification_level_export_package_sha256",
    "qualification_runtime_capture_sha256", "runtime_visual_projection_sha256",
    "runtime_visual_projection_schema", "goat_tmap_golden_file_sha256",
    "goat_tmap_oracle_manifest_sha256", "plan29_contract_file_sha256",
    "plan29_contract_self_sha256",
)))
VISUAL_MEMBERS = ("authored-level.bin", "entities.bin", "players.bin",
                  "presentation.bin", "terrain.bin")


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def visual_runtime(payload: bytes) -> bytes:
    """Decode the exact legacy visual field extent, without editing the carrier."""
    if len(payload) != 2783 or payload[:8] != b"MCR1\x02\x00\x00\x00":
        raise ValueError("Night production runtime requires MCR1/v2 and 29 counters")
    return payload[:4] + struct.pack("<HH", 1, 0) + payload[8:2754]


def metadata_sha256(manifest: dict) -> str:
    bindings = manifest["bindings"]
    if set(bindings) != set(BINDING_KEYS):
        raise ValueError("Night v2 binding field set differs")
    text = manifest["schema"] + "\n"
    for key in BINDING_KEYS:
        value = bindings[key]
        if key == "qualified_runtime_capture_count":
            if type(value) is not int or value != 8:
                raise ValueError("Night visual qualification requires eight captures")
        elif not isinstance(value, str) or not value.isascii():
            raise ValueError(f"Night binding {key} must be an ASCII string")
        text += key + "\0" + str(value) + "\n"
    text += "cache_content_sha256\0" + manifest["cache_content_sha256"] + "\n"
    return sha(text.encode("ascii"))


def _bank(package: Path, owner: dict, definition: dict, owner_root: Path) -> None:
    fact = next((f for f in definition["source_facts"]
                 if f["fact_id"] == "collision-mask-bank"), None)
    if fact is None or (fact["kind"], fact["version"], fact["member"], fact["lifetime"]) != (
        3, 2, "collision-masks.bin", "immutable-through-session"
    ):
        raise ValueError("Night production requires the declared collision SourceFact v2")
    feature = next(f for f in definition["source_facts"]
                   if f["fact_id"] == "feature-construction-inputs")
    if fact["selector_provenance"] != feature["selector_provenance"]:
        raise ValueError("Night collision selectors differ from authored constructors")
    metadata = owner["collision_masks"]
    if metadata["bank"] != "MSPRN0-0" or metadata["vga_resolution_selector"] != 1:
        raise ValueError("Night collision bank selects the wrong owner files")
    artifacts = []
    sources = {}
    for key, extension in (("tab", "TAB"), ("dat", "DAT")):
        relative = f"DATA/MSPRN0-0.{extension}"
        data = (owner_root / relative).read_bytes()
        identity = {"logical_path": "CD_Files/" + relative,
                    "bytes": len(data), "sha256": sha(data)}
        if metadata["source_files"][key] != identity:
            raise ValueError("Night collision source identity mismatch")
        artifacts.append(identity)
        sources[key] = data
    if fact["source_artifacts"] != sorted(artifacts, key=lambda a: a["logical_path"]):
        raise ValueError("Night collision fact does not bind the actual owner bytes")
    selectors = sorted({row["selector_value"] for row in fact["selector_provenance"]})
    result = bytearray(struct.pack("<4sHHHHII", b"MCM1", 1, 20, 1, 0, len(selectors), 12))
    rows = []
    for selector in selectors:
        if selector * 6 + 6 > len(sources["tab"]):
            raise ValueError("Night collision selector is outside the owner table")
        offset, width, height = struct.unpack_from("<IBB", sources["tab"], selector * 6)
        size = width * height * 2
        pixels = sources["dat"][offset:offset + size]
        if not size or len(pixels) != size:
            raise ValueError("Night collision source pixels are truncated")
        result.extend(struct.pack("<HHHHI", selector, width, height, 0, size))
        result.extend(pixels)
        rows.append({"selector": selector, "width": width, "height": height,
                     "pixel_bytes": size, "sha256": sha(pixels)})
    content = (package / "collision-masks.bin").read_bytes()
    if bytes(result) != content or rows != metadata["selectors"]:
        raise ValueError("Night collision carrier differs from actual owner pixels")
    if fact["member_sha256"] != sha(content):
        raise ValueError("Night collision SourceFact member hash mismatch")


def prepare(package: Path, qualified: Path, reference_campaign: Path,
            closure_path: Path, catalog: Path, owner_root: Path, *,
            expected_package: str, expected_runtime: str,
            verify_export, verify_campaign, closure_builder, private: Path) -> dict:
    """Validate current content and its independently retained visual qualification."""
    load = lambda p: json.loads(p.read_bytes())
    current = load(package / "manifest.json")
    owner = load(package / "owner-manifest.json")
    baseline = load(qualified / "manifest.json")
    legacy = load(reference_campaign / "manifest.json")
    verify_export(qualified, baseline)
    verify_export(package, owner)
    verify_export(package, current)
    verify_campaign(reference_campaign, legacy, expected_selection=(1, 0, 1))
    if baseline["package_sha256"] != expected_package:
        raise ValueError("Night qualification uses a different source export")
    if current["schema"] != "mc2-owner-campaign-package/v5" or current["level"] != baseline["level"]:
        raise ValueError("Night production requires the actual bank-bearing Level 1 package")
    if set(current["files"]) != set(legacy["files"]) | {"collision-masks.bin"}:
        raise ValueError("Night production campaign member set differs")
    if {p.name for p in package.iterdir()} != set(current["files"]) | {"manifest.json"}:
        raise ValueError("Night production campaign directory contains undeclared members")
    if current["owner_export_package_sha256"] != owner["package_sha256"]:
        raise ValueError("Night campaign does not bind its actual owner export")
    if any(current["files"].get(name) != identity for name, identity in owner["files"].items()):
        raise ValueError("Night campaign and owner export disagree on a member")
    for key in ("dimensions", "counts", "record_sizes", "presentation_dependencies"):
        if owner[key] != baseline[key]:
            raise ValueError(f"Night visual qualification differs at {key}")
    for name in VISUAL_MEMBERS:
        if (package / name).read_bytes() != (qualified / name).read_bytes():
            raise ValueError(f"Night visual qualification differs at {name}")
    runtime = (package / "runtime.bin").read_bytes()
    projection = visual_runtime(runtime)
    if projection != (qualified / "runtime.bin").read_bytes() or sha(projection) != expected_runtime:
        raise ValueError("Night runtime legacy visual fields differ from qualification")
    layout = owner["record_layouts"]["runtime"]
    if layout.get("constructor_counters") != {"offset": 2754, "count": 29, "record_bytes": 1}:
        raise ValueError("Night runtime constructor-counter layout differs")
    definition = load(package / "campaign-definition.json")
    reference = load(reference_campaign / "campaign-definition.json")
    if definition["schema"] != "mc2-campaign-definition/v5":
        raise ValueError("Night campaign definition schema differs")
    _bank(package, owner, definition, owner_root)
    # Only declared data-carrier upgrades may differ from the validated v3
    # behavior/asset definition; this does not change or publish either one.
    view = copy.deepcopy(definition)
    view["schema"] = reference["schema"]
    view["campaign_spec"]["reset_sources"]["runtime"] = reference["campaign_spec"]["reset_sources"]["runtime"]
    view["source_facts"] = [f for f in view["source_facts"] if f["fact_id"] != "collision-mask-bank"]
    for field, old in zip(view["campaign_spec"]["reset_sources"]["feature_boundary"],
                          reference["campaign_spec"]["reset_sources"]["feature_boundary"], strict=True):
        if field["member"] == "feature-construction-inputs.bin":
            if field["schema"] != "mc2-feature-construction-inputs/v2":
                raise ValueError("Night feature-construction carrier version differs")
            field.update(old)
    for fact in view["source_facts"]:
        if fact["fact_id"] == "feature-construction-inputs":
            fact["member_sha256"] = next(f["member_sha256"] for f in reference["source_facts"]
                                         if f["fact_id"] == fact["fact_id"])
    if view != reference:
        raise ValueError("Night behavior/asset definition differs from its visual qualification")
    closure = load(closure_path)
    rebuilt = closure_builder(qualified, catalog, owner_root,
        private / "generated/level1-frame-trace-final-v2-a",
        private / "generated/level1-frame-trace-final-v2-b")
    if closure != rebuilt:
        raise ValueError("Night visual closure differs from the canonical source/trace producer")
    return {"owner_manifest": owner,
            "campaign_package_sha256": current["package_sha256"],
            "runtime_capture_sha256": sha(runtime),
            "runtime_visual_projection_sha256": sha(projection),
            "qualification_level_export_package_sha256": baseline["package_sha256"],
            "closure_sha256": closure["closure_sha256"]}
