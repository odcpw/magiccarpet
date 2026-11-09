"""Extract owner-local original villager views without recoloring the source.

Decodes original SS2 frame packets and records input hashes. Outputs
are reference material, not distributable game assets.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

from PIL import Image, ImageDraw


def apply_ss2(data, pixels, width, height, cursor, limit):
    """FLC SS2 delta: row skips, last-pixel controls and word packets."""
    start = 6 + cursor
    size, tag = struct.unpack_from('<IH', data, start)
    end = start + size
    if tag != 0xf1fa or end > limit or size < 24:
        raise ValueError('Invalid original animation frame')
    children = struct.unpack_from('<H', data, start + 6)[0]
    position = start + 16
    if children != 1:
        raise ValueError('Expected one SS2 child')
    child_size, kind = struct.unpack_from('<IH', data, position)
    child_end = position + child_size
    if kind != 7 or child_end != end:
        raise ValueError('Invalid SS2 child extent/type')
    position += 6
    rows = struct.unpack_from('<H', data, position)[0]
    position += 2
    y, done = 0, 0
    while done < rows:
        if position + 2 > end:
            raise ValueError('Truncated row control')
        op = struct.unpack_from('<h', data, position)[0]
        position += 2
        if op < 0:
            if op & 0x4000:
                y += -op
            else:
                if y >= height:
                    raise ValueError('Last-pixel row exceeds image')
                pixels[(y + 1) * width - 1] = op & 255
            continue
        if y >= height:
            raise ValueError('SS2 row exceeds image')
        x = 0
        for _ in range(op):
            if position + 2 > end:
                raise ValueError('Truncated SS2 packet')
            skip, span = struct.unpack_from('<Bb', data, position)
            position += 2
            x += skip
            length = 2 * abs(span)
            source_size = length if span > 0 else 2
            if span == 0 or x + length > width or position + source_size > end:
                raise ValueError('Invalid SS2 packet extent')
            payload = data[position:position + source_size]
            pixels[y * width + x:y * width + x + length] = payload if span > 0 else payload * abs(span)
            position += source_size
            x += length
        y += 1
        done += 1
    if position != child_end:
        raise ValueError('Unconsumed SS2 bytes')
    return end - 6


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--world', choices=['night', 'day'], default='night')
    parser.add_argument('--appearance', type=int, choices=[239, 241, 242, 271], nargs='+', default=[239, 241, 242, 271])
    args = parser.parse_args()
    palette_path = args.cache / ('palette/paln.rgb8' if args.world == 'night' else 'palette/pald.rgb8')
    palette = palette_path.read_bytes()
    if len(palette) != 768:
        raise ValueError('Expected the original 256-color palette')
    output = args.output
    output.mkdir(parents=True, exist_ok=False)
    records = []
    for base, parameter in [(155, 239), (180, 241), (183, 242), (199, 271)]:
        if parameter not in args.appearance:
            continue
        sheet = Image.new('RGB', (1280, 1080), (40, 43, 49))
        draw = ImageDraw.Draw(sheet)
        for view in range(3):
            selector = base + view
            bank = 'tmaps1' if args.world == 'night' else 'tmaps0'
            path = args.cache / f'terrain/{bank}/{selector:03}.decoded.bin'
            data = path.read_bytes()
            flags, width, height = struct.unpack_from('<HHH', data)
            cursor = 6 + width * height
            pixels = bytearray(data[6:cursor])
            frames = [bytes(pixels)]
            if flags & 1:
                count, size = struct.unpack_from('<HI', data, cursor)
                end = cursor + 6 + size
                if end != len(data):
                    raise ValueError(f'TMAP {selector} animation extent mismatch')
                frames = []
                for _ in range(count):
                    cursor = apply_ss2(data, pixels, width, height, cursor, end)
                    frames.append(bytes(pixels))
            records.append(dict(selector=selector, renderParameter=parameter, width=width,
                height=height, frames=len(frames), sha256=hashlib.sha256(data).hexdigest()))
            for phase, indices in enumerate(frames[:4]):
                rgba = bytes(channel for index in indices for channel in
                    (*palette[index * 3:index * 3 + 3], 0 if index == 0 else 255))
                image = Image.frombytes('RGBA', (width, height), rgba)
                image.save(output / f'tmap-{selector}-{phase}.png')
                scale = min(8, 300 // width, 300 // height)
                enlarged = image.resize((width * scale, height * scale), Image.Resampling.NEAREST)
                left, top = phase * 320 + (320 - enlarged.width) // 2, view * 360 + 45
                sheet.paste(enlarged, (left, top), enlarged)
                draw.text((phase * 320 + 12, view * 360 + 12),
                    f'TMAP {selector}, phase {phase}, parameter {parameter}', fill='white')
        sheet.save(output / f'villager-{parameter}-original.png')
    (output / 'source.json').write_text(json.dumps(dict(paletteSha256=
        hashlib.sha256(palette).hexdigest(), records=records), indent=2) + '\n')
    print(json.dumps(dict(output=str(output.resolve()), sourceRecords=len(records))))


if __name__ == '__main__':
    main()
