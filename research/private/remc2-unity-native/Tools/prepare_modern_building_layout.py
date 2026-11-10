"""Extract a frozen Level 1 visual-only BUILD replacement descriptor.

Uses the qualified sandbox native and replay to read the public height plane.
Writes a new descriptor only after all selected cells and identities validate.
"""

import argparse
import collections
import ctypes as c
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
PROJECT = ROOT / 'research/private/remc2-unity-native/Builds/Modern'
OUTPUT = PROJECT / 'Builds/SandboxInputs/level1-building-source-layout.json'
ENTITIES = ROOT / 'research/private/generated/level1-export-m/entities.bin'
TERRAIN = ROOT / 'research/private/generated/level1-export-m/terrain.bin'
CLOSURE = ROOT / 'research/private/generated/level1-asset-closure.json'
BUILD = ROOT / 'research/private/remc2-native-runtime/CD_Files/DATA/BUILD0-0.DAT'
EXPECTED_BUILD_SHA = '3db5d9a763dedafbe3077eeeef47431da0b5d397e311f6ee46719a0fc51e8e5d'
EXPECTED_PLANE_SHA = 'e62920eb0d61c4a78b9173210f1611797cc714b8edee425b927fd9b8bd06dfbf'
ARCHITECTURAL_INVENTORY = {26: 9, 29: 11, 30: 8, 31: 3, 34: 1, 42: 4, 47: 2, 50: 3}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read_frozen_height(profile):
    library = PROJECT / 'Builds/Linux/MagicCarpetNative_Data/Plugins/x86_64/libsim_capi.so'
    library_bytes = library.read_bytes()
    replay = Path(profile['replay']).read_bytes()
    if sha(library_bytes) != profile['native_sha256'] or sha(replay) != profile['replay_sha256']:
        raise ValueError('installed native or replay does not match the sandbox profile')
    native = c.CDLL(str(library))
    u64, u32, ptr = c.c_uint64, c.c_uint32, c.c_void_p

    def bind(name, types):
        func = getattr(native, name)
        func.argtypes, func.restype = types, c.c_int
        return func

    create = bind('mc_game_runtime_create_v1', [c.POINTER(u64)])
    identity = bind('mc_game_runtime_identity_json_v1', [u64, ptr, u32, c.POINTER(u32)])
    load = bind('mc_game_runtime_load_replay_json_v1', [u64, u64, ptr, u32, ptr, u32])
    copy = bind('mc_native_campaign_terrain_plane_copy_u8', [u64, u64, u32, ptr, u32, c.POINTER(u32)])
    destroy = bind('mc_game_runtime_destroy_v1', [u64])
    error_length = bind('mc_native_last_error_length', [c.POINTER(u32)])
    error_copy = bind('mc_native_last_error_copy', [ptr, u32, c.POINTER(u32)])

    def check(status):
        if status == 0:
            return
        length = u32()
        error_length(c.byref(length))
        message = c.create_string_buffer(length.value + 1)
        error_copy(message, len(message), c.byref(length))
        raise RuntimeError(f'native status {status}: {message.value!r}')

    handle, length = u64(), u32()
    check(create(c.byref(handle)))
    buffer = c.create_string_buffer(8 * 1024 * 1024)
    try:
        check(identity(handle, buffer, len(buffer), c.byref(length)))
        initial = json.loads(buffer.raw[:length.value])
        packages = [{'package_dir': p['package'], 'owner_cache_path': p['owner'],
                     'presentation_cache_path': p['presentation']} for p in profile['packages']]
        request = json.dumps({'request_contract': 'carpet-flight-installed-package-set-bootstrap/v1',
                              'packages': packages}).encode()
        check(load(handle, int(initial['generation']), request, len(request), replay, len(replay)))
        check(identity(handle, buffer, len(buffer), c.byref(length)))
        session = json.loads(buffer.raw[:length.value])['session']
        if int(session['tick']) != 240:
            raise ValueError('unexpected sandbox replay tick')
        plane = (c.c_ubyte * 65536)()
        check(copy(handle, int(session['generation']), 1, plane, len(plane), c.byref(length)))
        if length.value != 65536:
            raise ValueError('unexpected terrain plane extent')
        result = bytes(plane)
        if sha(result) != EXPECTED_PLANE_SHA:
            raise ValueError('frozen terrain plane differs from the qualified snapshot')
        return result, sha(replay), sha(library_bytes)
    finally:
        check(destroy(handle))


def collect_footprints(value, found):
    if isinstance(value, dict):
        if value.get('kind') == 'terrain_modification_footprint':
            index = value['index']
            if index in found and found[index] != value:
                raise ValueError(f'conflicting BUILD closure rows for {index}')
            found[index] = value
        for child in value.values():
            collect_footprints(child, found)
    elif isinstance(value, list):
        for child in value:
            collect_footprints(child, found)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True, help='Fresh owner-local descriptor path; existing different output is preserved')
    args = parser.parse_args()
    profile = json.loads((PROJECT / 'sandbox-inputs.json').read_text())
    plane, replay_sha, native_sha = read_frozen_height(profile)
    entities, terrain, build, closure = (p.read_bytes() for p in (ENTITIES, TERRAIN, BUILD, CLOSURE))
    package = Path(profile['packages'][0]['package'])
    if entities != (package / 'entities.bin').read_bytes() or terrain != (package / 'terrain.bin').read_bytes():
        raise ValueError('sandbox level package differs from Level 1 reference export')
    if entities[:12] != bytes.fromhex('4d4345310100b000e8030000') or len(entities) != 176012:
        raise ValueError('unexpected entity arena format')
    if len(terrain) != 458752 or sha(build) != EXPECTED_BUILD_SHA:
        raise ValueError('unexpected terrain extent or BUILD data')
    reset = terrain[65536:131072]
    footprints = {}
    collect_footprints(json.loads(closure), footprints)
    buildings, positive_claims, preserved_claims, profiles, terrain_features = [], {}, {}, {}, []
    for slot in range(1000):
        record = entities[12 + slot * 176:12 + (slot + 1) * 176]
        if record[2] != 1 or tuple(record[67:69]) != (10, 45):
            continue
        if struct.unpack_from('<H', record)[0] != slot or record[73] != 52:
            raise ValueError(f'unexpected slot identity or action: {slot}')
        selector = record[74]
        x, y, z = struct.unpack_from('<HHh', record, 80)
        f = footprints[selector]
        w, h, offset = f['width'], f['height'], f['span']['offset']
        source = build[offset:offset + f['span']['length']]
        if len(source) != w * h * 2:
            raise ValueError(f'BUILD {selector} extent mismatch')
        # Some class10/model45 owners stamp water/terrain or nothing at all.
        # Source materials16..254 identify the architectural surface. Keep
        # other stamps in native terrain rather than inventing a house mesh.
        architectural = any(16 <= material < 255 for material in source[::2])
        if not architectural:
            terrain_features.append({'source_slot': slot, 'build_index': selector,
                                     'materials': sorted(set(source[::2])),
                                     'positive_height_cells': sum(v not in (0, 255) for v in source[1::2])})
            continue
        sx, sy = ((x + 128) >> 8) - w // 2, ((y + 128) >> 8) - h // 2
        positive, visible = [], []
        zero = missing = 0
        for j in range(h):
            for i in range(w):
                material, height = source[2 * (j * w + i):2 * (j * w + i) + 2]
                index = ((sy + j) & 255) * 256 + ((sx + i) & 255)
                if 16 <= material < 255:
                    visible.append((i, j))
                if height in (0, 255):
                    zero += height == 0
                    missing += height == 255
                    preserved_claims.setdefault(index, []).append(slot)
                    continue
                if index in positive_claims:
                    raise ValueError(f'positive height conflict at {index}: {slot}, {positive_claims[index]}')
                positive_claims[index] = slot
                positive.append((index, plane[index], height))
        if not positive or not visible:
            raise ValueError(f'empty positive or visible footprint: {slot}')
        bases = collections.Counter((actual - relative) & 255 for _, actual, relative in positive)
        if len(bases) != 1:
            raise ValueError(f'no unique base at slot {slot}: {bases}')
        base = next(iter(bases))
        if any(base + relative > 255 or actual != base + relative for _, actual, relative in positive):
            raise ValueError(f'height wrap or mismatch at slot {slot}')
        minx, maxx = min(i for i, _ in visible), max(i for i, _ in visible)
        miny, maxy = min(j for _, j in visible), max(j for _, j in visible)
        # BUILD material cells describe tiles; include the final row/column of
        # vertices to retain the original sloping envelope, not a bounding box.
        samples = []
        for j in range(miny, maxy + 2):
            for i in range(minx, maxx + 2):
                height = source[2 * (j * w + i) + 1] if i < w and j < h else 0
                samples.append(0 if height == 255 else height / 2)
        shape = {'build_index': selector, 'diagonal_parity': (sx + sy + minx + miny) & 1,
                 'columns': maxx - minx + 2,
                 'rows': maxy - miny + 2, 'heights': samples}
        if selector in profiles and profiles[selector] != shape:
            raise ValueError(f'inconsistent source profile {selector}')
        profiles[selector] = shape
        centerx = ((sx + (minx + maxx + 1) / 2) * 4) % 1024
        centerz = ((sy + (miny + maxy + 1) / 2) * 4) % 1024
        buildings.append({
            'source_slot': slot, 'build_index': selector, 'x': centerx, 'z': centerz,
            'base_height': base / 2, 'width': (maxx - minx + 1) * 4,
            'depth': (maxy - miny + 1) * 4, 'height': max(v for _, _, v in positive) / 2,
            'source_entity_position': {'x': x, 'y': y, 'z': z},
            'footprint_cells': {'width': w, 'height': h, 'start_x': sx & 255, 'start_y': sy & 255},
            'validation': {'positive_cells': len(positive), 'base_byte': base,
                           'matching_base_cells': bases[base], 'zero_height_cells_preserved': zero,
                           'ff_height_cells_preserved': missing,
                           'reset_equal_positive_cells': sum(plane[idx] == reset[idx] for idx, _, _ in positive),
                           'material_bbox_inclusive': [minx, miny, maxx, maxy],
                           'model_center_basis': 'center of material tile extents'},
            'cells': [{'index': idx, 'expected': actual, 'replacement': base} for idx, actual, _ in positive],
        })
    if collections.Counter(b['build_index'] for b in buildings) != ARCHITECTURAL_INVENTORY:
        raise ValueError('source architectural inventory changed')
    if len(terrain_features) != 6:
        raise ValueError('source non-architectural stamp inventory changed')
    overlap = positive_claims.keys() & preserved_claims.keys()
    if overlap:
        index = min(overlap)
        raise ValueError(f'positive patch touches another retained zero/FF cell at {index}')
    result = {
        'schema': 'mc-modern-building-layout/v2',
        'provenance': {
            'purpose': 'owner-local visual-only replacement of retained BUILD height stamps; not simulation, collision or live gameplay',
            'entity_path': str(ENTITIES.relative_to(ROOT)), 'entity_sha256': sha(entities),
            'build_path': str(BUILD.relative_to(ROOT)), 'build_sha256': sha(build),
            'closure_sha256': sha(closure), 'replay_path': profile['replay'],
            'replay_sha256': replay_sha, 'native_sha256': native_sha, 'snapshot_tick': 240,
            'source_height_plane_sha256': sha(plane), 'source_height_bytes': len(plane),
            'indices': 'index = (wrapped_source_tile_y * 256) + wrapped_source_tile_x',
            'replacement_scope': 'positive, finite BUILD height cells only; zero-offset skirt and 0xff cells preserved',
            'apply_rule': 'validate complete source snapshot hash and all expected bytes before changing a visual-only copy; all patches atomic, fail closed',
            'unit_conversion': 'source units /64; terrain cell = 4 Unity units; height byte = 0.5 Unity unit',
        },
        'profiles': [profiles[key] for key in sorted(profiles)],
        'buildings': buildings,
        'retained_terrain_features': terrain_features,
    }
    output = args.output.resolve()
    if output.exists():
        if json.loads(output.read_text()) != result:
            raise ValueError(f'existing descriptor differs; preserve and inspect {output}')
        disposition = 'verified existing descriptor'
    else:
        output.parent.mkdir(parents=True, exist_ok=True)
        with output.open('x') as stream:
            json.dump(result, stream, indent=2)
            stream.write('\n')
        disposition = 'created descriptor'
    print(json.dumps({'output': str(output), 'disposition': disposition, 'buildings': len(buildings),
                      'patched_cells': len(positive_claims), 'plane_sha256': sha(plane),
                      'zero_base_slots': [b['source_slot'] for b in buildings if b['base_height'] == 0]}))


if __name__ == '__main__':
    main()
