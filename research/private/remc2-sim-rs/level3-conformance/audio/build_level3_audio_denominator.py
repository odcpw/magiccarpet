#!/usr/bin/env python3
"""Build the package- and source-qualified Level 3 audio denominator."""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
from pathlib import Path


COMMIT = "6806c7204d17220ecc0430759f784aea8ee2ddb2"
ROOT = Path(__file__).resolve().parents[5]
UPSTREAM = ROOT / "upstream/magic-carpet-2-hd"
PACKAGE = ROOT / "research/private/generated/mc-cf14-campaign-package-v1-a/qualified-level3"
LEDGER = ROOT / "research/private/remc2-sim-rs/level1-conformance/remc2-audio-call-ledger.json"
RUST = ROOT / "research/private/remc2-sim-rs/crates/sim-core/src/lib.rs"
OUTPUT = Path(__file__).with_name("level3-audio-denominator.json")
PACKAGE_SHA256 = "6f17fe1c33c5852796b08acba2c44e5073939b947975b4f3b330d2c45b62d18d"

# Exact source occurrences exercised by the qualified package, its dynamic
# creature/spell/castle lifecycle, and shared Day player/UI lifecycle.  These
# are source-ledger ordinals, not Level 2's reachability result.
REACHABLE_ORDINALS = (
    1, 2, 8, 9, 10, 18, 35, 73, 74, 78, 79, 80, 81, 82, 85, 86, 87, 88,
    89, 90, 91, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116,
    118,
)


def canonical(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n").encode()


def source(path: str) -> str:
    return subprocess.run(
        ["git", "-C", str(UPSTREAM), "show", f"{COMMIT}:{path}"],
        check=True, capture_output=True, text=True,
    ).stdout


def build() -> dict[str, object]:
    manifest = json.loads((PACKAGE / "manifest.json").read_text())
    definition = json.loads((PACKAGE / "campaign-definition.json").read_text())
    if manifest.get("package_sha256") != PACKAGE_SHA256 or manifest.get("level") != {
        "external": 3, "internal": 2, "map_type": {"id": 0, "name": "Day"}
    }:
        raise ValueError("qualified Level 3 package identity changed")
    if hashlib.sha256((PACKAGE / "campaign-definition.json").read_bytes()).hexdigest() != manifest["files"]["campaign-definition.json"]["sha256"]:
        raise ValueError("qualified Level 3 campaign definition changed")
    models = sorted({row["model"] for row in definition["campaign_spec"]["authored_behavior_rows"] if row["class"] == 5})
    if models != [0, 1, 3, 4, 9]:
        raise ValueError("Level 3 creature-family census changed")

    ledger = json.loads(LEDGER.read_text())
    records = ledger.get("records")
    if not isinstance(records, list) or len(records) != 119:
        raise ValueError("source audio ledger denominator changed")
    rust = RUST.read_text()
    binding_block = rust.split("pub const SOURCE_AUDIO_CALL_BINDINGS", 1)[1].split("];", 1)[0]
    binding_rows = re.findall(
        r"SourceAudioCallTag::([A-Za-z0-9_]+),\s*\"([0-9a-f]{64})\"",
        binding_block,
    )
    if len(binding_rows) != 53 or len({digest for _, digest in binding_rows}) != 53:
        raise ValueError("Rust source-audio binding table changed")
    binding_by_hash = {digest: tag for tag, digest in binding_rows}
    pinned_sources: dict[str, str] = {}
    rows: list[dict[str, object]] = []
    selected = set(REACHABLE_ORDINALS)
    for ordinal, record in enumerate(records, 1):
        path = record["path"]
        text = pinned_sources.setdefault(path, source(path))
        lines = text.splitlines()
        line = int(record["pinned_line"])
        if not 1 <= line <= len(lines) or "PrepareEventSound_6E450" not in lines[line - 1]:
            raise ValueError(f"source occurrence moved: {ordinal}")
        occurrence_hash = record["stable_occurrence_hash_sha256"]
        reachable = ordinal in selected
        if reachable and occurrence_hash not in binding_by_hash:
            raise ValueError(f"reachable occurrence lacks Rust binding: {ordinal}")
        rows.append({
            "id": f"AUD-L3-{ordinal:03d}",
            "ledger_ordinal_1based": ordinal,
            "path": path,
            "enclosing_symbol": record["enclosing_symbol"],
            "file_ordinal_1based": record["file_ordinal_1based"],
            "symbol_ordinal_1based": record["symbol_ordinal_1based"],
            "pinned_line": line,
            "normalized_call_expression": record["normalized_call_expression"],
            "stable_occurrence_hash_sha256": occurrence_hash,
            "owner_expression": record["entity_owner_expression"],
            "auxiliary_owner_expression": record["auxiliary_owner_expression"],
            "sound_expression": record["sound_expression"],
            "classification": "reachable_level3" if reachable else "mechanically_unreachable_level3",
            "runtime_binding_tag": binding_by_hash.get(occurrence_hash) if reachable else None,
            "profile": {"external": 3, "internal": 2, "map_type": "Day"},
            "proof": "qualified Level 3 package/shared lifecycle and compiler-checked source binding" if reachable else "not bound by the qualified Level 3 package/shared lifecycle",
        })
    music_source = source("remc2/engine/Sound.cpp")
    music_symbols = ["InitMusic_8D970", "StartMusic_8E160", "StopMusic_8E020", "EndMusic_99C90", "UpdateMusic_99970", "FadeWarMusic_99830"]
    music_refs = []
    for symbol in music_symbols:
        match = re.search(rf"\b{re.escape(symbol)}\s*\(", music_source)
        if not match:
            raise ValueError(f"music lifecycle symbol moved: {symbol}")
        music_refs.append({"symbol": symbol, "path": "remc2/engine/Sound.cpp", "pinned_line": music_source.count("\n", 0, match.start()) + 1, "source_sha256": hashlib.sha256(music_source.encode()).hexdigest()})
    return {
        "schema": "mc2-level3-audio-denominator/v1",
        "pinned_commit": COMMIT,
        "package": {"path": str(PACKAGE.relative_to(ROOT)), "package_sha256": PACKAGE_SHA256, "campaign_definition_sha256": manifest["files"]["campaign-definition.json"]["sha256"], "class5_models": models},
        "audio": {"denominator": 119, "counts": {"reachable_level3": len(selected), "mechanically_unreachable_level3": 119 - len(selected)}, "rows": rows},
        "music": {"map_type": "Day", "maptype_music": 2, "channel": 0, "driver": "f", "logical_track": 1, "playback_slot": 0, "lifecycle": ["StartMusic", "FadeWarMusic", "UpdateMusic", "StopMusic"], "source_references": music_refs},
    }


if __name__ == "__main__":
    OUTPUT.write_bytes(canonical(build()))
