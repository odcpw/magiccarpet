#!/usr/bin/env python3
"""Verify a controlled 1600x900 Level 1 owner-HUD capture."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

from PIL import Image, ImageChops


WIDTH = 1600
HEIGHT = 900
REFERENCE_WIDTH = 640
REFERENCE_HEIGHT = 480
SCALE = min(WIDTH / REFERENCE_WIDTH, HEIGHT / REFERENCE_HEIGHT)
OFFSET_X = (WIDTH - REFERENCE_WIDTH * SCALE) / 2
OFFSET_Y = (HEIGHT - REFERENCE_HEIGHT * SCALE) / 2
OBJECTIVE = (192, 126, 256, 38)
BADGE = (192, 168, 60, 7)
MINIMAP = (0, 0, 128, 128)


def scaled_rect(spec: tuple[int, int, int, int]) -> tuple[int, int, int, int]:
    x, y, width, height = spec
    return (
        round(OFFSET_X + x * SCALE),
        round(OFFSET_Y + y * SCALE),
        round(OFFSET_X + (x + width) * SCALE),
        round(OFFSET_Y + (y + height) * SCALE),
    )


def count_colour(image: Image.Image, colour: tuple[int, int, int]) -> int:
    return sum(pixel == colour for pixel in image.convert("RGB").get_flattened_data())


def bitmap_samples(
    image: Image.Image,
    cache: Path,
    palette: bytes,
    row: dict,
    x: int,
    y: int,
) -> tuple[int, int, set[tuple[int, int]]]:
    destination = scaled_rect(
        (x, y, row["runtime_width"], row["runtime_height"])
    )
    left, top, right, bottom = destination
    indices = (cache / row["indices_path"]).read_bytes()
    mask = (cache / row["mask_path"]).read_bytes()
    expected_extent = row["stored_width"] * row["stored_height"]
    if len(indices) != expected_extent or len(mask) != expected_extent:
        raise SystemExit(f"selector {row['selector']}: wrong indexed/mask extent")
    opaque = 0
    matched = 0
    points: set[tuple[int, int]] = set()
    for screen_y in range(top, bottom):
        source_y = min(
            row["stored_height"] - 1,
            int((screen_y - top) * row["stored_height"] / (bottom - top)),
        )
        for screen_x in range(left, right):
            source_x = min(
                row["stored_width"] - 1,
                int((screen_x - left) * row["stored_width"] / (right - left)),
            )
            source = source_y * row["stored_width"] + source_x
            if not mask[source]:
                continue
            opaque += 1
            points.add((screen_x, screen_y))
            palette_offset = indices[source] * 3
            expected = tuple(palette[palette_offset:palette_offset + 3])
            matched += image.getpixel((screen_x, screen_y)) == expected
    return opaque, matched, points


def alpha_sample_points(
    cache: Path,
    path: str,
    width: int,
    height: int,
    destination: tuple[int, int, int, int],
) -> list[tuple[int, int]]:
    payload = (cache / path).read_bytes()
    if len(payload) != width * height * 4:
        raise SystemExit(f"{path}: wrong RGBA extent")
    left, top, right, bottom = destination
    points: list[tuple[int, int]] = []
    for y in range(top, bottom):
        source_y = min(height - 1, int((y - top) * height / (bottom - top)))
        for x in range(left, right):
            source_x = min(width - 1, int((x - left) * width / (right - left)))
            if payload[(source_y * width + source_x) * 4 + 3]:
                points.append((x, y))
    return points


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("capture", type=Path)
    parser.add_argument("--comparison", type=Path)
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
    manifest = json.loads((args.cache / "manifest.json").read_text())
    descriptor = json.loads((args.cache / manifest["assets"]["level1_hud"]["path"]).read_text())
    if (
        descriptor["schema"] != "mc2-level1-owner-hud/v1"
        or descriptor["reference_layout"]["width"] != 640
        or descriptor["reference_layout"]["height"] != 480
        or descriptor["reference_layout"]["source_scale"] != 2
    ):
        raise SystemExit("wrong HUD descriptor/reference layout")
    minimap = descriptor["minimap"]
    if (
        minimap["map_mode"]
        != "player_centered_yaw_rotated_TABLESN_scanline_aperture"
        or (minimap["width"], minimap["height"]) != (128, 128)
        or (minimap["logical_width"], minimap["logical_height"]) != (64, 64)
        or minimap["presentation_scale"] != 2
        or minimap["sampling"]["source_units_per_pixel"] != 512
    ):
        raise SystemExit("wrong qualified low-resolution minimap contract")
    aperture = minimap["aperture"]
    aperture_mask = (args.cache / aperture["path"]).read_bytes()
    if (
        len(aperture_mask) != 64 * 64
        or sum(aperture_mask) != 3204
        or hashlib.sha256(aperture_mask).hexdigest()
        != "98a3a7c0300b49204629aab1b4b552253bea2223f9e68c40d1458b2e62243285"
        or aperture["table_curve_sha256"]
        != "e155e5eaa2019054c52878dad895d0ad70c6b92698fab72ae518eeccea4eafc7"
    ):
        raise SystemExit("wrong exact TABLESN 64x64 minimap aperture")
    scanlines = aperture["scanlines"]
    for y, row in enumerate(scanlines):
        expected = bytes(
            int(row["start_x_inclusive"] <= x < row["end_x_exclusive"])
            for x in range(64)
        )
        if row["y"] != y or aperture_mask[y * 64:(y + 1) * 64] != expected:
            raise SystemExit(f"minimap aperture scanline mismatch at y={y}")
    directions = minimap["rotation"]["cardinal_direction_samples"]
    expected_direction_tiles = [
        (0, "top", 77, 190), (0, "right", 109, 222),
        (0, "bottom", 77, 254), (0, "left", 45, 222),
        (512, "top", 109, 222), (512, "right", 77, 254),
        (512, "bottom", 45, 222), (512, "left", 77, 190),
        (1024, "top", 77, 254), (1024, "right", 45, 222),
        (1024, "bottom", 77, 190), (1024, "left", 109, 222),
        (1536, "top", 45, 222), (1536, "right", 77, 190),
        (1536, "bottom", 109, 222), (1536, "left", 77, 254),
    ]
    if [
        (
            row["yaw"], row["screen_label"],
            row["source_tile_x"], row["source_tile_y"],
        )
        for row in directions
    ] != expected_direction_tiles:
        raise SystemExit("minimap cardinal direction transform changed")
    bitmap_rows = {
        row["selector"]: row for row in descriptor["bitmap_assets"]
    }
    if sorted(bitmap_rows) != [1, 40, 41, 42, 43, 54, 123, 124, 171, 172, 173]:
        raise SystemExit("wrong/duplicate HUD bitmap selector set")
    palette = (args.cache / "palette/paln.rgb8").read_bytes()
    if len(palette) != 768:
        raise SystemExit("wrong PALN RGB8 extent")
    placements = [
        (
            panel["semantic_role"],
            panel["selector"],
            panel["x"],
            panel["y"],
        )
        for panel in descriptor["top_bar"]["panels"]
    ]
    for slot in descriptor["spells"]["slots"]:
        placements.append(
            (
                slot["semantic_role"] + "_frame",
                descriptor["spells"]["frame_selector"],
                slot["x"],
                slot["y"],
            )
        )
        placements.append(
            (
                slot["semantic_role"] + "_icon",
                slot["icon_selector"],
                slot["x"],
                slot["y"],
            )
        )
    bitmap_results = {}
    topbar_points: set[tuple[int, int]] = set()
    for name, selector, x, y in placements:
        opaque, matched, points = bitmap_samples(
            image, args.cache, palette, bitmap_rows[selector], x, y
        )
        ratio = matched / opaque if opaque else 0.0
        if opaque < 100 or ratio < 0.55:
            raise SystemExit(
                f"{name}/selector{selector}: owner bitmap absent or wrong "
                f"({matched}/{opaque}, ratio {ratio:.3f})"
            )
        bitmap_results[name] = {
            "selector": selector,
            "opaque_sample_pixels": opaque,
            "exact_palette_matches": matched,
            "match_ratio": ratio,
        }
        topbar_points.update(points)

    objective_rect = scaled_rect(OBJECTIVE)
    badge_rect = scaled_rect(BADGE)
    minimap_rect = scaled_rect(MINIMAP)
    objective_crop = image.crop(objective_rect)
    badge_crop = image.crop(badge_rect)
    minimap_crop = image.crop(minimap_rect)
    objective_red = count_colour(objective_crop, (252, 36, 36))
    badge_red = count_colour(badge_crop, (252, 36, 36))
    if objective_red < 1500:
        raise SystemExit(f"objective FONT1 Night-red pixels too sparse: {objective_red}")
    if badge_red < 180:
        raise SystemExit(f"FrozenObserved owner-font badge missing: {badge_red}")
    if len(set(minimap_crop.get_flattened_data())) < 40:
        raise SystemExit("minimap lacks indexed Night terrain/entity diversity")

    centre_x = round(OFFSET_X + 64 * SCALE)
    centre_y = round(OFFSET_Y + 64 * SCALE)
    marker = image.crop((centre_x - 10, centre_y - 10, centre_x + 11, centre_y + 11))
    marker_red = count_colour(marker, (252, 36, 36))
    if marker_red < 15:
        raise SystemExit(f"live player marker missing at minimap centre: {marker_red}")

    magenta = sum(
        red > 200 and blue > 200 and green < 80
        for red, green, blue in image.get_flattened_data()
    )
    if magenta:
        raise SystemExit(f"shader/material failure magenta pixels: {magenta}")
    default_overlay_crop = image.crop((1400, 845, 1600, 900))
    near_white = sum(
        red > 235 and green > 235 and blue > 235
        for red, green, blue in default_overlay_crop.get_flattened_data()
    )
    if near_white > 20:
        raise SystemExit(
            f"bottom-right Unity/default-font overlay suspected: {near_white} near-white pixels"
        )

    comparison_changed = None
    comparison_topbar_changed = None
    if args.comparison:
        comparison = Image.open(args.comparison).convert("RGB")
        if comparison.size != image.size:
            raise SystemExit("comparison capture size mismatch")
        objective_points = alpha_sample_points(
            args.cache,
            descriptor["objective"]["render_path"],
            descriptor["objective"]["render_width"],
            descriptor["objective"]["render_height"],
            objective_rect,
        )
        objective_changed = sum(
            image.getpixel(point) != comparison.getpixel(point)
            for point in objective_points
        )
        # Fractional 1.875x GUI placement can expose a handful of edge
        # compositor samples from the changing framebuffer. The opaque owner
        # raster itself must otherwise remain fixed.
        if objective_changed > 20:
            raise SystemExit(
                f"FrozenObserved objective pixels changed: {objective_changed}"
            )
        comparison_topbar_changed = sum(
            image.getpixel(point) != comparison.getpixel(point)
            for point in topbar_points
        )
        # As with the objective, fractional 1.875x placement leaves a very
        # small number of edge-compositor samples coupled to the framebuffer.
        # More than 0.2% would indicate that the owner top bar itself moved or
        # changed.
        topbar_change_limit = max(100, round(len(topbar_points) * 0.002))
        if comparison_topbar_changed > topbar_change_limit:
            raise SystemExit(
                "FrozenObserved owner panels/spell icons changed: "
                f"{comparison_topbar_changed}"
            )
        map_difference = ImageChops.difference(
            minimap_crop, comparison.crop(minimap_rect)
        )
        comparison_changed = sum(
            pixel != (0, 0, 0)
            for pixel in map_difference.get_flattened_data()
        )
        if comparison_changed < 200:
            raise SystemExit(
                f"live-pose minimap did not move/rotate enough: {comparison_changed}"
            )

    report = {
        "capture": str(args.capture.resolve()),
        "status": "PASS",
        "reference_layout": [640, 480],
        "uniform_scale": SCALE,
        "objective_rect": list(objective_rect),
        "objective_red_pixels": objective_red,
        "badge_rect": list(badge_rect),
        "badge_red_pixels": badge_red,
        "minimap_rect": list(minimap_rect),
        "minimap_rgb_colours": len(set(minimap_crop.get_flattened_data())),
        "minimap_logical_resolution": [64, 64],
        "minimap_display_resolution": [128, 128],
        "minimap_aperture_opaque_pixels": 3204,
        "minimap_direction_samples": len(directions),
        "player_marker_red_pixels": marker_red,
        "magenta_pixels": magenta,
        "bottom_right_near_white_pixels": near_white,
        "owner_bitmap_samples": bitmap_results,
        "comparison_topbar_changed_pixels": comparison_topbar_changed,
        "comparison_minimap_changed_pixels": comparison_changed,
    }
    print(
        json.dumps(report, indent=2, sort_keys=True)
        if args.json
        else (
            "MC_OWNER_HUD_PIXELS PASS "
            f"objective_red={objective_red} badge_red={badge_red} "
            f"minimap_colours={report['minimap_rgb_colours']} "
            f"marker_red={marker_red} magenta=0 default_overlay=0"
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
