#!/usr/bin/env python3
"""Verify two private Goat TMAP oracle runs against the pinned golden."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import stat


ROOT = Path(__file__).resolve().parent
GOLDEN_PATH = ROOT / "goat_tmap_golden.json"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_run(path: Path, golden: dict) -> tuple[dict, bytes]:
    path = path.resolve(strict=True)
    if not path.is_dir() or stat.S_IMODE(path.stat().st_mode) != 0o700:
        raise ValueError(f"oracle run must be a 0700 directory: {path}")
    manifest_path = path / "manifest.json"
    manifest_bytes = manifest_path.read_bytes()
    if hashlib.sha256(manifest_bytes).hexdigest() != golden[
        "source_manifest_sha256"
    ]:
        raise ValueError(f"oracle manifest is not the pinned golden: {path}")
    manifest = json.loads(manifest_bytes)
    if (
        manifest.get("schema") != "mc2-private-goat-tmap-oracle/v1"
        or manifest.get("status") != "PASS"
        or manifest.get("decoder") != "compiled-remc2-sub_72350"
        or manifest.get("big_sprites") is not False
    ):
        raise ValueError(f"oracle authority fields changed: {path}")
    if set(item.name for item in path.iterdir()) != {
        "manifest.json",
        *[row["file"] for row in manifest["states"]],
    }:
        raise ValueError(f"oracle run has missing or extra artifacts: {path}")
    projected: dict[str, list[list[object]]] = {}
    for row in manifest["states"]:
        artifact = path / row["file"]
        if stat.S_IMODE(artifact.stat().st_mode) != 0o600:
            raise ValueError(f"oracle artifact must be 0600: {artifact}")
        if artifact.stat().st_size != row["bytes"] or sha256(artifact) != row["sha256"]:
            raise ValueError(f"oracle artifact identity mismatch: {artifact}")
        projected.setdefault(str(row["selector"]), []).append(
            [
                row["state"],
                row["sha256"],
                row["stream_cursor"],
                row["frame_index"],
                row["width"],
                row["height"],
                row["bytes"],
            ]
        )
    if projected != golden["selectors"]:
        raise ValueError(f"oracle state matrix differs from golden: {path}")
    for selector, rows in projected.items():
        by_state = {row[0]: row for row in rows}
        if (
            by_state["reset-base"][1] != by_state["frame-04"][1]
            or by_state["closure-frame-01"][1:] != by_state["frame-01"][1:]
        ):
            raise ValueError(f"selector {selector} cycle closure changed")
    if stat.S_IMODE(manifest_path.stat().st_mode) != 0o600:
        raise ValueError(f"oracle manifest must be 0600: {manifest_path}")
    return manifest, manifest_bytes


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("run_a", type=Path)
    parser.add_argument("run_b", type=Path)
    args = parser.parse_args()
    golden = json.loads(GOLDEN_PATH.read_text(encoding="utf-8"))
    if golden.get("schema") != "mc2-private-goat-tmap-golden/v1":
        raise ValueError("unsupported Goat TMAP golden schema")
    manifest_a, bytes_a = load_run(args.run_a, golden)
    manifest_b, bytes_b = load_run(args.run_b, golden)
    if bytes_a != bytes_b:
        raise ValueError("oracle manifests are not deterministic")
    for row in manifest_a["states"]:
        if (args.run_a / row["file"]).read_bytes() != (
            args.run_b / row["file"]
        ).read_bytes():
            raise ValueError(f"oracle payload differs across runs: {row['file']}")
    print(
        "MC_GOAT_TMAP_GOLDEN PASS "
        f"selectors={len(golden['selectors'])} "
        f"states={len(manifest_a['states'])} "
        f"manifest={golden['source_manifest_sha256']}"
    )


if __name__ == "__main__":
    main()
