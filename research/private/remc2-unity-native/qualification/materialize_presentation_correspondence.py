#!/usr/bin/env python3
"""Materialize private Level 1 presentation evidence under a temporary path."""

from __future__ import annotations

import argparse
import hashlib
import html
import json
import shutil
import tempfile
from pathlib import Path

from PIL import Image


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
CORPUS_PATH = HERE / "level1-reference-corpus.json"
CONTRACT_PATH = HERE / "presentation-correspondence.json"


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def resolve_output(raw: Path) -> Path:
    require(not raw.is_symlink(), f"unsafe output symlink: {raw}")
    output = raw.resolve()
    temporary_root = Path(tempfile.gettempdir()).resolve()
    require(
        output != temporary_root and temporary_root in output.parents,
        f"output must be a dedicated child of {temporary_root}",
    )
    require(not output.is_symlink(), f"unsafe output symlink: {output}")
    output.mkdir(parents=True, exist_ok=True)
    require(not any(output.iterdir()), f"output directory is not empty: {output}")
    return output


def resolve_capture_directory(raw: Path) -> Path:
    require(not raw.is_symlink(), f"unsafe capture symlink: {raw}")
    directory = raw.resolve(strict=True)
    temporary_root = Path(tempfile.gettempdir()).resolve()
    require(
        directory != temporary_root and temporary_root in directory.parents,
        f"Unity capture directory must be below {temporary_root}",
    )
    require(directory.is_dir() and not directory.is_symlink(), "unsafe capture directory")
    return directory


def render_html(report: dict) -> str:
    original_cards = []
    for row in report["original_timeline"]:
        original_cards.append(
            "<figure>"
            f"<img src='{html.escape(row['png_file'])}' "
            f"alt='{html.escape(row['checkpoint_id'])}'>"
            f"<figcaption>{html.escape(row['checkpoint_id'])}"
            f"<small>reference frame {row['reference_frame']} · "
            f"source turn {row['source_turn']} · base-PALN structural decode</small>"
            "</figcaption></figure>"
        )
    unity_cards = []
    for row in report["unity_timeline"]:
        unity_cards.append(
            "<figure>"
            f"<img src='{html.escape(row['png_file'])}' "
            f"alt='{html.escape(row['checkpoint_id'])}'>"
            f"<figcaption>{html.escape(row['checkpoint_id'])}"
            f"<small>Unity checkpoint {row['unity_checkpoint_tick']} · "
            f"native render tick {row['native_render_tick']} · synthetic route</small>"
            "</figcaption></figure>"
        )
    matrix_rows = []
    for surface in report["surfaces"]:
        cells = []
        for classification in ("exact", "frozen", "adapted", "omitted", "deferred"):
            values = surface[classification]
            cells.append(
                "<td><ul>"
                + "".join(f"<li>{html.escape(value)}</li>" for value in values)
                + "</ul></td>"
            )
        matrix_rows.append(
            f"<tr><th>{html.escape(surface['id'])}</th>{''.join(cells)}</tr>"
        )
    return """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Private MC2 Level 1 presentation correspondence</title>
<style>
:root { color-scheme: dark; font-family: system-ui, sans-serif; background:#090b11; color:#edf1ff; }
body { margin:0; padding:28px; }
h1,h2 { margin:.25rem 0 1rem; }
.warning { border:2px solid #ffcf5a; background:#352a0b; padding:14px; font-weight:700; }
.timeline { display:grid; grid-template-columns:repeat(auto-fit,minmax(260px,1fr)); gap:18px; }
figure { margin:0; padding:12px; background:#141925; border:1px solid #30394f; }
img { width:100%; height:auto; image-rendering:pixelated; background:#000; }
figcaption { padding-top:8px; font-weight:700; }
small { display:block; color:#aeb9d1; font-weight:400; margin-top:4px; }
.original img { aspect-ratio:4/3; object-fit:fill; }
table { border-collapse:collapse; width:100%; font-size:.9rem; }
th,td { vertical-align:top; border:1px solid #30394f; padding:8px; }
thead th { background:#20283a; }
ul { margin:0; padding-left:1.1rem; }
</style>
</head>
<body>
<h1>Private MC2 Level 1 presentation correspondence</h1>
<p class="warning">Independent clocks. No one-to-one frame pairing, active-DAC RGB claim, or remc2/Unity pixel-equality claim.</p>
<h2>Original remc2 timeline</h2>
<div class="timeline original">""" + "".join(original_cards) + """</div>
<h2>Unity synthetic regression timeline</h2>
<div class="timeline">""" + "".join(unity_cards) + """</div>
<h2>Structural correspondence matrix</h2>
<table><thead><tr><th>Surface</th><th>Exact</th><th>Frozen</th><th>Adapted</th><th>Omitted</th><th>Deferred</th></tr></thead>
<tbody>""" + "".join(matrix_rows) + """</tbody></table>
</body></html>
"""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    parser.add_argument("--unity-captures", required=True, type=Path)
    args = parser.parse_args()

    output = resolve_output(args.output)
    unity_captures = resolve_capture_directory(args.unity_captures)
    corpus = json.loads(CORPUS_PATH.read_text())
    contract = json.loads(CONTRACT_PATH.read_text())
    original_suite = next(
        item for item in corpus["suites"] if item["id"] == contract["reference_suite"]
    )
    unity_suite = next(
        item for item in corpus["suites"] if item["id"] == contract["unity_suite"]
    )

    fixture = next(
        item for item in original_suite["fixtures"]
        if item["role"] == "complete_indexed_framebuffer"
    )
    fixture_path = REPO / fixture["path"]
    framebuffer = fixture_path.read_bytes()
    require(sha256_bytes(framebuffer) == fixture["sha256"], "framebuffer fixture hash mismatch")
    require(
        len(framebuffer) == fixture["frame_bytes"] * fixture["frame_count"],
        "framebuffer fixture extent mismatch",
    )

    decode = contract["original_timeline"]["display_decode"]
    palette_rgb6_path = REPO / decode["palette_rgb6_path"]
    palette_rgb8_path = REPO / decode["palette_rgb8_path"]
    palette_rgb6 = palette_rgb6_path.read_bytes()
    palette_rgb8 = palette_rgb8_path.read_bytes()
    require(
        len(palette_rgb6) == 768
        and max(palette_rgb6) <= 63
        and sha256_bytes(palette_rgb6) == decode["palette_rgb6_sha256"],
        "PALN RGB6 identity mismatch",
    )
    require(
        palette_rgb8 == bytes(value * 4 for value in palette_rgb6)
        and sha256_bytes(palette_rgb8) == decode["palette_rgb8_sha256"],
        "PALN RGB8 conversion/identity mismatch",
    )

    original_by_id = {
        item["checkpoint_id"]: item for item in original_suite["checkpoints"]
    }
    original_rows = []
    for checkpoint_id in contract["original_timeline"]["checkpoint_ids"]:
        checkpoint = original_by_id[checkpoint_id]
        reference_frame = checkpoint["reference_frame"]
        start = reference_frame * fixture["frame_bytes"]
        indices = framebuffer[start:start + fixture["frame_bytes"]]
        require(
            sha256_bytes(indices) == checkpoint["original_framebuffer_sha256"],
            f"{checkpoint_id}: exact indexed framebuffer mismatch",
        )
        rgb = bytes(
            channel
            for palette_index in indices
            for channel in palette_rgb8[palette_index * 3:palette_index * 3 + 3]
        )
        image = Image.frombytes("RGB", (fixture["width"], fixture["height"]), rgb)
        png_name = f"{checkpoint_id}-base-paln.png"
        png_path = output / png_name
        image.save(png_path, format="PNG", optimize=False, compress_level=9)
        png_hash = sha256_file(png_path)
        require(
            png_hash
            == contract["original_timeline"]["derived_png_sha256"][checkpoint_id],
            f"{checkpoint_id}: derived base-PALN PNG identity mismatch",
        )
        original_rows.append({
            "checkpoint_id": checkpoint_id,
            "reference_frame": reference_frame,
            "source_turn": checkpoint["source_turn"],
            "indices_sha256": sha256_bytes(indices),
            "png_file": png_name,
            "png_sha256": png_hash,
            "decode_authority": decode["authority"],
        })

    unity_by_id = {
        item["checkpoint_id"]: item for item in unity_suite["checkpoints"]
    }
    unity_rows = []
    for checkpoint_id in contract["unity_timeline"]["checkpoint_ids"]:
        checkpoint = unity_by_id[checkpoint_id]
        tick = checkpoint["unity_checkpoint_tick"]
        manifest_path = unity_captures / f"magiccarpet-autoplay-{tick:03d}.json"
        png_source = unity_captures / f"magiccarpet-autoplay-{tick:03d}.png"
        manifest = json.loads(manifest_path.read_text())
        expected_frame = checkpoint["unity_frame_identity"]
        require(
            manifest["checkpoint_tick"] == tick
            and manifest["evidence_authority"] == contract["unity_timeline"]["authority"],
            f"{checkpoint_id}: Unity capture authority/clock mismatch",
        )
        require(
            sha256_file(png_source) == manifest["png_sha256"]
            == expected_frame["png_sha256"],
            f"{checkpoint_id}: Unity PNG identity mismatch",
        )
        png_name = f"{checkpoint_id}.png"
        shutil.copyfile(png_source, output / png_name)
        unity_rows.append({
            "checkpoint_id": checkpoint_id,
            "unity_checkpoint_tick": tick,
            "native_render_tick": checkpoint["camera_identity"]["render"]["tick"],
            "png_file": png_name,
            "png_sha256": expected_frame["png_sha256"],
            "authority": contract["unity_timeline"]["authority"],
        })

    report = {
        "schema": contract["schema"],
        "comparison_mode": contract["comparison_mode"],
        "corpus_sha256": sha256_file(CORPUS_PATH),
        "contract_sha256": sha256_file(CONTRACT_PATH),
        "package_sha256": corpus["runtime_pins"]["package_identity_sha256"],
        "closure_sha256": corpus["runtime_pins"]["closure_identity_sha256"],
        "owner_cache_content_sha256": corpus["runtime_pins"]["owner_cache_content_sha256"],
        "unity_build_guid": corpus["runtime_pins"]["unity_build_guid"],
        "original_timeline": original_rows,
        "unity_timeline": unity_rows,
        "controlled_gallery": contract["controlled_gallery"],
        "one_to_one_pairs": contract["pairing"]["one_to_one_pairs"],
        "timelines": contract["timelines"],
        "layers": contract["layers"],
        "comparisons": contract["comparisons"],
        "forbidden_claims": contract["forbidden_claims"],
        "surfaces": contract["surfaces"],
        "claim_limits": {
            "original": decode["claim_limit"],
            "unity": contract["unity_timeline"]["claim_limit"],
        },
    }
    (output / "presentation-correspondence-evidence.json").write_text(
        json.dumps(report, indent=2) + "\n"
    )
    (output / "presentation-correspondence.html").write_text(render_html(report))
    print(
        "MC_PRESENTATION_MATERIALIZE PASS "
        f"original={len(original_rows)} unity={len(unity_rows)} "
        f"mode={contract['comparison_mode']} output={output}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
