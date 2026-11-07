"""Preserve the exact packed source maps for native engine import."""
from pathlib import Path

import bpy


def extract_packed(output: Path, names: dict[str, str]):
    directory = output / 'textures'
    directory.mkdir(parents=True, exist_ok=True)
    for source, target in names.items():
        image = bpy.data.images[source]
        if image.packed_file is None:
            raise ValueError(f'{source} is not packed')
        target_path = directory / target
        target_path.write_bytes(bytes(image.packed_file.data))
        # Empty names cause Blender's FBX exporter to alias multiple packed
        # images to the same embedded filename (including the normal map).
        image.filepath = str(target_path.resolve())
