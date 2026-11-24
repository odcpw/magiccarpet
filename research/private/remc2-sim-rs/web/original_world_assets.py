"""Source-indexed terrain and TMAP images for the browser.

Owner data remains in the ignored local cache; only the bounded manifest and
blob are served. TMAP selectors are source IDs, not dense atlas indices.
"""
from __future__ import annotations

import json
import hashlib
import struct
from functools import lru_cache
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2] / 'generated' / 'settlement-20261105-integration' / 'owner-closed-v6'


@lru_cache(maxsize=1)
def _source_sine() -> bytes:
    """The same pinned 16.16 sine table consumed by sim-terrain-grid."""
    source = (Path(__file__).resolve().parents[4] /
              'upstream/magic-carpet-2-hd/remc2/utilities/Maths.cpp').read_bytes()
    if hashlib.sha256(source).hexdigest() != '8bed55e5c132a2485c3258d75cb7714def2c0bd9cf17839b1bfa4b0c659d52c0':
        raise ValueError('pinned sine source changed')
    text = source.decode('utf-8')
    declaration = text.index('int32_t Maths::sin_DB750[2560]')
    start = text.index('{', declaration) + 1
    end = text.index('};', start)
    tokens = [token.split('//')[0].strip() for token in text[start:end].split(',')]
    values = [int(token, 16) for token in tokens if token]
    if (len(values) != 2560 or values[2048:] != values[:512]
            or [values[i] for i in (0, 512, 1024, 1536)] != [0, 65536, 0, 0xffff0000]):
        raise ValueError('pinned sine table is invalid')
    return struct.pack('<2048I', *values[:2048])


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
        position = outer + 16
        for _ in range(children):
            child = position
            child_size, child_type = unpack('<IH', child, end)
            child_end = child + child_size
            if child_size < 6 or child_end > end:
                fail('invalid SS2 child extent')
            if child_type == 4:
                position = child_end
                continue
            if child_type != 7 or child_size < 8:
                fail('unsupported SS2 child type')
            lines, = unpack('<h', child + 6, child_end)
            if not 0 <= lines <= height:
                fail('invalid SS2 line count')
            position = child + 8
            destination = 0
            for _ in range(lines):
                while True:
                    control, = unpack('<h', position, child_end)
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
                    skip, span = unpack('<Bb', position, child_end)
                    position += 2
                    destination += skip
                    write_end = destination + abs(span) * 2
                    if span == 0 or destination < row_start or write_end > row_end:
                        fail('invalid SS2 packet')
                    take = abs(span) * 2 if span > 0 else 2
                    if position + take > child_end:
                        fail('truncated SS2 packet')
                    data = payload[position:position + take]
                    pixels[destination:write_end] = data if span > 0 else data * -span
                    position += take
                    destination = write_end
                destination = row_end
            if position != child_end:
                fail('SS2 child trailing bytes')
        if position != end:
            fail('SS2 record child count or extent changed')
        frames.append(bytes(pixels))
    if position != len(payload):
        fail('animation trailing bytes')
    return frames


class OriginalWorldAssets:
    """Build a source-indexed bundle from the selected owner-cache root."""
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
                   'indices': add(read(bl['indices_path'], 152 * 32 * 32)), 'sine': add(_source_sine()),
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
        self.palette = rgb8
        self.manifest = {'schema': 'mc-original-world-assets/v1', 'bytes': len(blob), 'map_type': env['profile']['map_type']['id'],
                         'terrain': terrain, 'palette': palette_out, 'tmaps': rows,
                         'unavailable_selectors': unavailable}

    @classmethod
    def from_package(cls, resource: dict) -> 'OriginalWorldAssets':
        """Bind the supplied campaign package to its exported environment cache."""
        package = Path(resource['package_dir']).resolve(strict=True)
        environment = Path(resource['owner_cache_path']).resolve(strict=True)
        package_manifest = json.loads((package / 'manifest.json').read_text())
        descriptor = json.loads((environment / 'materials/descriptor.json').read_text())
        profile = descriptor['profile']
        if (descriptor.get('schema') != 'mc2-environment-materials/v1'
                or profile['campaign_package_sha256'] != package_manifest['package_sha256']
                or descriptor['tmaps']['bank'] != profile['tmaps']):
            raise ValueError('environment cache does not match campaign package')

        def read(relative, length=None):
            path = (environment / relative).resolve(strict=True)
            if not path.is_relative_to(environment):
                raise ValueError('asset outside owner cache')
            data = path.read_bytes()
            if length is not None and len(data) != length:
                raise ValueError(f'invalid asset length: {relative}')
            return data

        blob = bytearray()
        def add(data):
            span = {'offset': len(blob), 'length': len(data)}
            blob.extend(data)
            return span

        blocks = descriptor['blocks']
        size, count = blocks['tile_size'], blocks['tile_count']
        width, height = blocks['width'], blocks['height']
        if size != 32 or count <= 0 or width % size or height % size or count > width * height // (size * size):
            raise ValueError('invalid source terrain atlas dimensions')
        atlas = read(blocks['atlas_path'], width * height)
        columns = width // size
        tiles = b''.join(
            atlas[((tile // columns) * size + row) * width + (tile % columns) * size:
                  ((tile // columns) * size + row) * width + (tile % columns + 1) * size]
            for tile in range(count) for row in range(size)
        )
        # The orientation table is source-global, independent of mission art.
        uv = json.loads((ROOT / 'night-presentation-v6/terrain/materials.json').read_text())['terrain_uv']['persistent_unity_groups']
        terrain = {'width': size, 'height': size, 'count': count,
                   'indices': add(tiles), 'sine': add(_source_sine()), 'uv_groups': uv}
        rgb8 = read(descriptor['palette']['rgb8_path'], 768)
        if any(value % 4 for value in rgb8):
            raise ValueError('invalid source RGB6 palette')
        tables = read(descriptor['tables']['path'], 83456)
        palette = {'rgb8': add(rgb8), 'shade_lut': add(tables[:65536]),
                   'clrn': add(read(descriptor['colors']['path'], 4096)),
                   'blend_lut': add(tables[0x4000:0x14000])}
        terrain['map_colors'] = add(tables[0x14000:0x14100])
        rows, unavailable, seen = [], [], set()
        for row in descriptor['tmaps']['textures']:
            selector = row['index']
            if selector in seen:
                raise ValueError('duplicate TMAP selector')
            seen.add(selector)
            if row.get('source_empty'):
                if read(row['decoded_payload_path']) != b'0':
                    raise ValueError(f'TMAP {selector} invalid source placeholder')
                unavailable.append(selector)
                continue
            frames = _frames(read(row['decoded_payload_path']), selector, row['width'], row['height'],
                             row['animated'], row['frame_count'])
            if frames[0] != read(row['indices_path'], row['width'] * row['height']):
                raise ValueError(f'TMAP {selector} base mismatch')
            rows.append({'selector': selector, 'width': row['width'], 'height': row['height'],
                         'animated': row['animated'], 'frames': [add(frame) for frame in frames]})
        result = cls.__new__(cls)
        result.blob = bytes(blob)
        result.palette = rgb8
        result.manifest = {'schema': 'mc-original-world-assets/v1', 'bytes': len(blob), 'map_type': profile['map_type']['id'],
                           'terrain': terrain, 'palette': palette, 'tmaps': rows,
                           'unavailable_selectors': unavailable}
        return result
