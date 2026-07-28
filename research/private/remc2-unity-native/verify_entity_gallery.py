#!/usr/bin/env python3
"""Verify the controlled 1600x900 owner-sprite gallery capture."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path

from PIL import Image, ImageChops, ImageStat


WIDTH = 1600
HEIGHT = 900
DISTANCE = 18.0
# Match Level1ExitPresentationMath.UnityVerticalFov(128): Night first turns
# the source history FOV into its integer projection scale.  The 4:3 display
# adaptation is pillarboxed at 1600x900, so the active viewport retains the
# full 900-pixel height.
SOURCE_HISTORY_FOV = 128
SOURCE_PROJECTION_SCALE = 7 * 377 * SOURCE_HISTORY_FOV >> 11
FOCAL_PIXELS = HEIGHT * SOURCE_PROJECTION_SCALE / 240.0
BACKGROUND = (14, 17, 23)
CACHE_CONTENT_SHA256 = "d299990e300117d065eb3a75008f6a69911b1ba356e4e06452c0a826450a01e5"
PALN_RGB8_SHA256 = "7c4dfbbc6c0f68a99214fe3ca93d7a1cf0dd658e17a40ad7ecb302152264f507"
SPECS = {
    "dolmen": (-7.0, 0.6, 400, 110, 66),
    "goat_front": (-0.3, 0.6, 150, 22, 22),
    "goat_side": (4.2, 0.6, 150, 22, 22),
    "goat_rear_mirror": (8.7, 0.6, 150, 22, 22),
    "villager_a": (-7.0, -5.0, 200, 18, 40),
    "villager_b": (-7.0 + 14.0 / 3.0, -5.0, 250, 26, 56),
    "villager_c": (-7.0 + 28.0 / 3.0, -5.0, 250, 26, 56),
    "villager_d": (7.0, -5.0, 300, 24, 74),
}
OWNER_BINDINGS = {
    "dolmen": ("dolmen", 79, False),
    "goat_front": ("goat", 152, False),
    "goat_side": ("goat", 154, False),
    "goat_rear_mirror": ("goat", 154, True),
    "villager_a": ("villager-a", 155, False),
    "villager_b": ("villager-b", 180, False),
    "villager_c": ("villager-c", 183, False),
    "villager_d": ("villager-d", 199, False),
}


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def projected_rect(spec: tuple[float, float, int, int, int]) -> tuple[int, int, int, int]:
    x, baseline, logical_height, texture_width, texture_height = spec
    world_height = logical_height / 64.0
    world_width = world_height * texture_width / texture_height
    center_x = WIDTH / 2.0 + x * FOCAL_PIXELS / DISTANCE
    center_y_from_bottom = (
        HEIGHT / 2.0 + (baseline + world_height / 2.0) * FOCAL_PIXELS / DISTANCE
    )
    pixel_width = world_width * FOCAL_PIXELS / DISTANCE
    pixel_height = world_height * FOCAL_PIXELS / DISTANCE
    top = HEIGHT - center_y_from_bottom - pixel_height / 2.0
    return (
        math.floor(center_x - pixel_width / 2.0) - 2,
        math.floor(top) - 2,
        math.ceil(center_x + pixel_width / 2.0) + 2,
        math.ceil(top + pixel_height) + 2,
    )


def is_foreground(pixel: tuple[int, int, int]) -> bool:
    return max(abs(pixel[index] - BACKGROUND[index]) for index in range(3)) > 3


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("capture", type=Path)
    parser.add_argument(
        "--cache",
        type=Path,
        default=Path(__file__).resolve().parents[1]
        / "generated/level1-unity-asset-cache",
    )
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    image = Image.open(args.capture).convert("RGB")
    if image.size != (WIDTH, HEIGHT):
        raise SystemExit(f"expected {WIDTH}x{HEIGHT}, got {image.size}")

    cache_manifest = json.loads((args.cache / "manifest.json").read_text())
    if cache_manifest["cache_content_sha256"] != CACHE_CONTENT_SHA256:
        raise SystemExit("owner-cache identity mismatch")
    palette = (args.cache / "palette/paln.rgb8").read_bytes()
    if len(palette) != 768 or sha256_bytes(palette) != PALN_RGB8_SHA256:
        raise SystemExit("owner PALN RGB8 identity mismatch")
    palette_colours = {
        tuple(palette[offset:offset + 3]) for offset in range(0, len(palette), 3)
    }
    entity_descriptor = json.loads(
        (
            args.cache
            / cache_manifest["assets"]["entity_presentations"]["path"]
        ).read_text()
    )
    terrain_descriptor = json.loads(
        (
            args.cache
            / cache_manifest["assets"]["terrain_materials"]["path"]
        ).read_text()
    )
    groups = {
        group["semantic_family"]: group for group in entity_descriptor["groups"]
    }
    textures = {
        row["index"]: row for row in terrain_descriptor["tmaps1"]["textures"]
    }

    results = {}
    crops = {}
    for name, spec in SPECS.items():
        semantic_family, selector, mirrored = OWNER_BINDINGS[name]
        group = groups[semantic_family]
        if (
            group["qualified_status"] != "qualified_owner_actor_billboard"
            or group["authority"] != "FrozenObserved"
            or selector not in group["selector"]["qualified_selectors"]
        ):
            raise SystemExit(f"{name}: selector is not a qualified owner actor")
        if (
            group["billboard"]["hotspot"]
            != "no_per_sprite_hotspot_in_type_particle_str"
            or not group["billboard"]["source_anchor_status"].startswith("deferred:")
        ):
            raise SystemExit(f"{name}: hotspot/anchor classification changed")
        texture = textures[selector]
        if (
            texture["width"] != spec[3]
            or texture["height"] != spec[4]
            or texture["palette_asset_id"] != "owner_data:DATA/PALN-0.DAT"
            or texture["origin"] != "top_left"
            or texture["row_order"] != "top_to_bottom"
        ):
            raise SystemExit(f"{name}: owner TMAP descriptor mismatch")
        indices = (args.cache / texture["indices_path"]).read_bytes()
        if len(indices) != texture["width"] * texture["height"]:
            raise SystemExit(f"{name}: owner TMAP base-frame extent mismatch")

        rect = projected_rect(spec)
        crop = image.crop(rect)
        crops[name] = crop
        foreground = [
            (index % crop.width, index // crop.width, pixel)
            for index, pixel in enumerate(crop.get_flattened_data())
            if is_foreground(pixel)
        ]
        if not foreground:
            raise SystemExit(f"{name}: no visible owner-sprite pixels")
        xs = [row[0] for row in foreground]
        ys = [row[1] for row in foreground]
        bbox_width = max(xs) - min(xs) + 1
        bbox_height = max(ys) - min(ys) + 1
        expected_width = rect[2] - rect[0] - 4
        expected_height = rect[3] - rect[1] - 4
        if bbox_width > expected_width + 4 or bbox_height > expected_height + 4:
            raise SystemExit(
                f"{name}: foreground exceeds projected TMAP quad "
                f"{bbox_width}x{bbox_height} vs {expected_width}x{expected_height}"
            )
        if bbox_width < expected_width * 0.18 or bbox_height < expected_height * 0.35:
            raise SystemExit(
                f"{name}: implausibly sparse/flattened foreground "
                f"{bbox_width}x{bbox_height} vs {expected_width}x{expected_height}"
            )
        colors = {row[2] for row in foreground}
        if len(colors) < 4:
            raise SystemExit(f"{name}: insufficient PALN color diversity ({len(colors)})")
        palette_matches = sum(row[2] in palette_colours for row in foreground)
        palette_match_ratio = palette_matches / len(foreground)
        if palette_match_ratio < 0.95:
            raise SystemExit(
                f"{name}: rendered pixels are not owner PALN colours "
                f"({palette_match_ratio:.3f})"
            )

        source_match_ratio = None
        if not mirrored:
            expected_texture = Image.frombytes(
                "P", (texture["width"], texture["height"]), indices
            )
            expected_texture.putpalette(palette)
            expected_texture = expected_texture.convert("RGB").resize(
                (expected_width, expected_height), Image.Resampling.NEAREST
            )
            expected_mask = Image.frombytes(
                "L",
                (texture["width"], texture["height"]),
                bytes(255 if value else 0 for value in indices),
            ).resize((expected_width, expected_height), Image.Resampling.NEAREST)
            best_matches = 0
            expected_opaque = sum(
                value != 0 for value in expected_mask.get_flattened_data()
            )
            for offset_y in range(5):
                for offset_x in range(5):
                    matches = 0
                    for source_y in range(expected_height):
                        for source_x in range(expected_width):
                            if not expected_mask.getpixel((source_x, source_y)):
                                continue
                            matches += (
                                crop.getpixel((
                                    source_x + offset_x,
                                    source_y + offset_y,
                                ))
                                == expected_texture.getpixel((source_x, source_y))
                            )
                    best_matches = max(best_matches, matches)
            source_match_ratio = best_matches / expected_opaque
            if source_match_ratio < 0.70:
                raise SystemExit(
                    f"{name}: rendered pixels do not match owner TMAP "
                    f"({source_match_ratio:.3f})"
                )
        results[name] = {
            "semantic_family": semantic_family,
            "selector": selector,
            "owner_indices_sha256": sha256_bytes(indices),
            "projected_quad": list(rect),
            "expected_pixels": [expected_width, expected_height],
            "visible_bbox_pixels": [bbox_width, bbox_height],
            "visible_pixels": len(foreground),
            "rgb_color_count": len(colors),
            "paln_exact_match_ratio": palette_match_ratio,
            "owner_source_match_ratio": source_match_ratio,
        }

    side = crops["goat_side"]
    rear = crops["goat_rear_mirror"]
    if side.size != rear.size:
        raise SystemExit(f"goat mirror crop size mismatch: {side.size} vs {rear.size}")
    difference = ImageChops.difference(side.transpose(Image.Transpose.FLIP_LEFT_RIGHT), rear)
    mirror_mean_error = sum(ImageStat.Stat(difference).mean) / 3.0
    if mirror_mean_error > 6.0:
        raise SystemExit(f"goat rear UV mirror mismatch: mean RGB error {mirror_mean_error:.3f}")

    magenta_pixels = sum(
        1 for red, green, blue in image.get_flattened_data()
        if red > 200 and blue > 200 and green < 80
    )
    if magenta_pixels:
        raise SystemExit(f"shader/material failure magenta pixels: {magenta_pixels}")

    report = {
        "capture": str(args.capture.resolve()),
        "owner_cache_content_sha256": CACHE_CONTENT_SHA256,
        "palette_rgb8_sha256": PALN_RGB8_SHA256,
        "authority": "controlled-debug-gallery",
        "simulation_mutation": False,
        "status": "PASS",
        "sprites": results,
        "goat_side_rear_uv_mirror_mean_rgb_error": mirror_mean_error,
        "magenta_pixels": magenta_pixels,
    }
    print(json.dumps(report, indent=2, sort_keys=True) if args.json else (
        "MC_ENTITY_GALLERY_PIXELS PASS sprites=8 "
        f"mirror_mean_error={mirror_mean_error:.3f} magenta=0"
    ))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
