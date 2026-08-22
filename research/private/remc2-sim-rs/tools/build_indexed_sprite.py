#!/usr/bin/env python3
"""Deterministically compile a small authored indexed sprite to the F9 runtime schema."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def exact_object(value: object, fields: set[str], name: str) -> dict:
    if not isinstance(value, dict) or set(value) != fields:
        raise ValueError(f"{name} member set differs from {sorted(fields)}")
    return value


def compile_sprite(source: object, recipe: object, source_id: str, recipe_id: str) -> bytes:
    source = exact_object(
        source,
        {"height", "name", "palette", "rows", "schema", "width"},
        "source",
    )
    recipe = exact_object(
        recipe,
        {
            "budgets",
            "color_contract",
            "coordinates",
            "frame_duration_ms",
            "origin",
            "runtime_format",
            "schema",
        },
        "recipe",
    )
    if source["schema"] != "carpet-flight-indexed-source/v1":
        raise ValueError("unsupported source schema")
    if recipe["schema"] != "carpet-flight-indexed-recipe/v1":
        raise ValueError("unsupported recipe schema")
    width = source["width"]
    height = source["height"]
    rows = source["rows"]
    palette = source["palette"]
    if (
        isinstance(width, bool)
        or not isinstance(width, int)
        or isinstance(height, bool)
        or not isinstance(height, int)
        or width <= 0
        or height <= 0
        or not isinstance(rows, list)
        or len(rows) != height
        or not isinstance(palette, list)
        or not 1 <= len(palette) <= 256
    ):
        raise ValueError("invalid source dimensions, rows, or palette")
    if any(
        not isinstance(color, list)
        or len(color) != 4
        or any(isinstance(channel, bool) or not isinstance(channel, int) or not 0 <= channel <= 255 for channel in color)
        for color in palette
    ):
        raise ValueError("palette is not RGBA8")
    indices: list[int] = []
    for row in rows:
        if not isinstance(row, str) or len(row) != width:
            raise ValueError("source row width differs")
        for character in row:
            try:
                index = int(character, 16)
            except ValueError as error:
                raise ValueError("source row is not hexadecimal indices") from error
            if index >= len(palette):
                raise ValueError("source row references a missing palette color")
            indices.append(index)
    budgets = exact_object(
        recipe["budgets"],
        {"max_bytes", "max_frames", "max_palette_colors", "max_pixels"},
        "recipe.budgets",
    )
    runtime = {
        "budgets": budgets,
        "color_contract": recipe["color_contract"],
        "coordinates": recipe["coordinates"],
        "dependencies": [source_id, recipe_id],
        "frame_count": 1,
        "frame_duration_ms": recipe["frame_duration_ms"],
        "height": height,
        "indices": indices,
        "kind": "indexed_sprite",
        "origin": recipe["origin"],
        "palette_rgba8": [channel for color in palette for channel in color],
        "runtime_format": recipe["runtime_format"],
        "schema": "carpet-flight-indexed-sprite/v1",
        "width": width,
    }
    payload = (json.dumps(runtime, sort_keys=True, separators=(",", ":")) + "\n").encode()
    if len(payload) > budgets["max_bytes"]:
        raise ValueError("compiled payload exceeds byte budget")
    return payload


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--recipe", required=True, type=Path)
    parser.add_argument("--source-id", required=True)
    parser.add_argument("--recipe-id", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        payload = compile_sprite(
            json.loads(args.source.read_text(encoding="utf-8")),
            json.loads(args.recipe.read_text(encoding="utf-8")),
            args.source_id,
            args.recipe_id,
        )
    except (OSError, ValueError) as error:
        parser.error(str(error))
    if args.check:
        if args.output.read_bytes() != payload:
            raise SystemExit("runtime artifact differs from deterministic transform")
    else:
        args.output.write_bytes(payload)


if __name__ == "__main__":
    main()
