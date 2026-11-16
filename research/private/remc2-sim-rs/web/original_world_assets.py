"""Allowlisted Level 1 original indexed terrain and TMAP images for the browser.

Owner data remains in the ignored local cache; only the bounded manifest and
blob are served. TMAP selectors are source IDs, not dense atlas indices.
"""
from __future__ import annotations

import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2] / 'generated' / 'settlement-20261105-integration' / 'owner-closed-v6'


def _frames(payload: bytes, selector: int, width: int, height: int, animated: bool, count: int) -> list[bytes]:
    """Materialize selector-global SS2 transitions; see sim-ss2-tmap's decoder."""
    def fail(message):
        raise ValueError(f'TMAP {selector}: {message}')
    def unpack(fmt, offset, end):
        size = struct.calcsize(fmt)
        if offset < 0 or offset + size > end or end > len(payload):
            fail('truncated stream')
        return struct.unpack_from(fmt, payload, offset)

    if len(payload) < 6:
        fail('missing header')
    flags, w, h = struct.unpack_from('<HHH', payload)
    if (w, h, bool(flags & 1)) != (width, height, animated):
        fail('descriptor/header mismatch')
    base_end = 6 + width * height
    if base_end > len(payload):
        fail('truncated pixels')
    pixels = bytearray(payload[6:base_end])
    frames = [bytes(pixels)]
    if not animated:
        if count != 1 or base_end != len(payload):
            fail('invalid still image')
        return frames
    encoded_count, stream_size = unpack('<HI', base_end, len(payload))
    if encoded_count != count or count < 1 or base_end + 6 + stream_size != len(payload):
        fail('invalid animation header')
    position = base_end + 6
    for _ in range(count):
        outer = position
        size, tag, children = unpack('<IHH', outer, len(payload))
        end = outer + size
        if size < 16 or end > len(payload) or tag != 0xf1fa or payload[outer + 8:outer + 16] != bytes(8):
            fail('invalid SS2 outer record')
        if children == 0 and size == 16:
            position = end
            frames.append(bytes(pixels))
            continue
        if children != 1:
            fail('invalid SS2 child count')
        child = outer + 16
        child_size, child_type, lines = unpack('<IHh', child, end)
        if child_size < 8 or child + child_size != end or child_type != 7 or not 0 <= lines <= height:
            fail('invalid SS2 child record')
        position = child + 8
        destination = 0
        for _ in range(lines):
            while True:
                control, = unpack('<h', position, end)
                position += 2
                if control >= 0:
                    break
                if control & 0x4000:
                    destination += width * -control
                    if destination >= len(pixels):
                        fail('SS2 line skip exceeds image')
                else:
                    if destination + width > len(pixels):
                        fail('SS2 last pixel exceeds image')
                    pixels[destination + width - 1] = control & 255
            row_start, row_end = destination, destination + width
            if row_end > len(pixels):
                fail('SS2 row exceeds image')
            for _ in range(control):
                skip, span = unpack('<Bb', position, end)
                position += 2
                destination += skip
                write_end = destination + abs(span) * 2
                if span == 0 or destination < row_start or write_end > row_end:
                    fail('invalid SS2 packet')
                take = abs(span) * 2 if span > 0 else 2
                if position + take > end:
                    fail('truncated SS2 packet')
                data = payload[position:position + take]
                pixels[destination:write_end] = data if span > 0 else data * -span
                position += take
                destination = write_end
            destination = row_end
        if position != end:
            fail('SS2 record trailing bytes')
        frames.append(bytes(pixels))
    if position != len(payload):
        fail('animation trailing bytes')
    return frames


class OriginalWorldAssets:
    """Build a compact source-indexed bundle from a fixed owner-cache root."""
    def __init__(self, root: Path = ROOT):
        night = root / 'night-presentation-v6'
        environment = root / 'environment-complete' / 'cache-level1'
        d = json.loads((night / 'terrain/materials.json').read_text())
        env = json.loads((environment / 'materials/descriptor.json').read_text())
        if d.get('schema') != 'mc2-level1-night-materials/v1' or env['profile']['sprites'] != 'MSPRN0-0':
            raise ValueError('unexpected Level 1 Night materials')
        blob = bytearray()
        def add(data):
            result = {'offset': len(blob), 'length': len(data)}
            blob.extend(data)
            return result
        def read(relative, length):
            # Paths are descriptor-owned, not HTTP input. Refuse traversal even if a cache is tampered with.
            path = (night / relative).resolve()
            if not path.is_relative_to(night.resolve()):
                raise ValueError('asset outside owner cache')
            data = path.read_bytes()
            if len(data) != length:
                raise ValueError(f'invalid asset length: {relative}')
            return data
        bl = d['bl32n']
        if (bl['tile_width'], bl['tile_height'], bl['tile_count'], bl['terrain_selector']) != (32, 32, 152, 'terrain_type_plane_u8_direct_tile_index'):
            raise ValueError('unexpected terrain tile bank')
        terrain = {'width': 32, 'height': 32, 'count': 152,
                   'indices': add(read(bl['indices_path'], 152 * 32 * 32)),
                   'uv_groups': d['terrain_uv']['persistent_unity_groups']}
        palette = d['palette']
        rgb8 = read(palette['rgb8_path'], 768)
        if any(value % 4 for value in rgb8):
            raise ValueError('invalid source RGB6 palette')
        palette_out = {'rgb8': add(rgb8), 'shade_lut': add(read(palette['shade_lut_path'], 65536)),
                       'clrn': add(read(palette['clrn_path'], 4096))}
        tables = read(palette['tablesn_path'], 83456)
        if bytes(blob[palette_out['shade_lut']['offset']:palette_out['shade_lut']['offset'] + 65536]) != tables[:65536]:
            raise ValueError('TABLESN shade table mismatch')
        palette_out['blend_lut'] = add(tables[0x4000:0x14000])
        terrain['map_colors'] = add(tables[0x14000:0x14100])
        rows, unavailable, seen = [], [], set()
        for row in d['tmaps1']['textures']:
            selector = row['index']
            if selector in seen:
                raise ValueError('duplicate TMAP selector')
            seen.add(selector)
            if row['status'] == 'raw_single_byte_placeholder_nondecoded':
                unavailable.append(selector)
                continue
            if row['status'] != 'decoded_indexed_texture':
                raise ValueError(f'unknown TMAP status {selector}')
            path = (night / row['decoded_payload_path']).resolve()
            if not path.is_relative_to(night.resolve()):
                raise ValueError('TMAP outside owner cache')
            frames = _frames(path.read_bytes(), selector, row['width'], row['height'], row['animated'], row['frame_count'])
            if frames[0] != read(row['indices_path'], row['width'] * row['height']):
                raise ValueError(f'TMAP {selector} base mismatch')
            rows.append({'selector': selector, 'width': row['width'], 'height': row['height'],
                         'animated': row['animated'], 'frames': [add(frame) for frame in frames]})
        if seen != set(d['tmaps1']['scene_reachable_selectors']):
            raise ValueError('incomplete scene TMAP closure')
        self.blob = bytes(blob)
        self.manifest = {'schema': 'mc-original-world-assets/v1', 'bytes': len(blob),
                         'terrain': terrain, 'palette': palette_out, 'tmaps': rows,
                         'unavailable_selectors': unavailable}
