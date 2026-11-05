"""Pack source-thresholded multiview foliage captures (requires Pillow 13.0.0).

Run from the Unity project root:
  uv run --no-project --with pillow==13.0.0 python Tools/pack_modern_foliage.py
The RGB of the linear data atlas is object-space normal; A is view depth.
"""
from pathlib import Path
import os
from PIL import Image, ImageChops

project = Path(__file__).resolve().parent.parent
root = project / 'Assets/Resources/ModernNature'
frames = project.parent / 'generated/nightspire-rework-20261105/multiview-foliage'
tile, guard, azimuths, elevations = 128, 8, 8, 5
inner = tile - 2 * guard

def extend_data(image, coverage, steps):
    """Extend valid normal/depth into transparent pixels to prevent mip seams."""
    # Very low-alpha PNG pixels can contain saturated unpremultiplication
    # noise after filtering; propagate color from meaningful coverage instead.
    mask = coverage.point(lambda a: 255 if a >= 8 else 0)
    image = Image.composite(image, Image.new('RGBA', image.size), mask)
    for _ in range(steps):
        for dx, dy in ((-1, 0), (1, 0), (0, -1), (0, 1)):
            moved_mask = Image.new('L', mask.size)
            moved_mask.paste(mask, (dx, dy))
            fill = ImageChops.multiply(moved_mask, ImageChops.invert(mask))
            moved_image = Image.new('RGBA', image.size)
            moved_image.paste(image, (dx, dy))
            image.paste(moved_image, (0, 0), fill)
            mask = ImageChops.lighter(mask, moved_mask)
    return image

for name in ('grass_lod', 'fern_lod', 'shrub_lod', 'tree_leaves_lod'):
    if os.environ.get('MC_FOLIAGE_ASSET') and os.environ['MC_FOLIAGE_ASSET'] != name:
        continue
    color_atlas = Image.new('RGBA', (tile * azimuths, tile * elevations))
    data_atlas = Image.new('RGBA', (tile * azimuths, tile * elevations))
    for elevation in range(elevations):
        for azimuth in range(azimuths):
            base = frames / f'{name}_{elevation}_{azimuth}'
            color = Image.open(str(base) + '_color.png').convert('RGBA').resize((inner, inner), Image.Resampling.LANCZOS)
            normal = Image.open(str(base) + '_normal.png').convert('RGBA').resize((inner, inner), Image.Resampling.LANCZOS)
            depth = Image.open(str(base) + '_depth.png').convert('RGBA').resize((inner, inner), Image.Resampling.LANCZOS)
            coverage = color.getchannel('A')
            if coverage.getbbox() is None:
                raise ValueError(f'empty capture {base}')
            data = normal.copy()
            data.putalpha(depth.getchannel('R'))
            # The image is first placed in a full tile, so dilation fills the
            # guard as well as transparent holes adjacent to the silhouette.
            tile_data = Image.new('RGBA', (tile, tile))
            tile_data.paste(data, (guard, guard))
            tile_mask = Image.new('L', (tile, tile))
            tile_mask.paste(coverage, (guard, guard))
            tile_data = extend_data(tile_data, tile_mask, guard)
            tile_color = Image.new('RGBA', (tile, tile))
            tile_color.paste(color, (guard, guard))
            tile_color = extend_data(tile_color, tile_mask, guard)
            # RGB fills transparent borders, but their opacity remains zero.
            tile_color.putalpha(tile_mask)
            # Unity texture V=0 is the PNG's bottom row.
            x, y = azimuth * tile, (elevations - 1 - elevation) * tile
            color_atlas.paste(tile_color, (x, y))
            data_atlas.paste(tile_data, (x, y))
    color_atlas.save(root / f'{name}_color.png', optimize=True)
    data_atlas.save(root / f'{name}_normal_depth.png', optimize=True)
    print('PACKED', name, color_atlas.size, flush=True)
