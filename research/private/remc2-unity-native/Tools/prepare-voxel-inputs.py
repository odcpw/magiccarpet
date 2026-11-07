#!/usr/bin/env python3
"""Export the verified frozen Level 1 plane and one actual BUILD house.

The height copy removes every known positive BUILD stamp using the existing
validated descriptor. It is a sandbox presentation input, not game state.
Existing different outputs and receipts are never overwritten.
"""

import collections
import hashlib
import importlib.util
import json
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
UNITY = TOOLS.parent
OUTPUT = UNITY / 'Assets/Resources/ModernCharacters/VoxelSandbox/Level1.json'
RECEIPT = Path('/mnt/pinokio-shared/magiccarpet-voxel-sandbox-20261107/level1-input-receipt.json')
SOURCE_SLOT = 38


def digest(data):
    return hashlib.sha256(data).hexdigest()


def encoded(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':')) + '\n').encode()


def existing_matches(path, content):
    if path.exists() and path.read_bytes() != content:
        raise ValueError(f'existing output differs; preserve and inspect {path}')


def create_or_verify(path, content):
    if path.exists():
        existing_matches(path, content)
        return 'verified existing'
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        with path.open('xb') as stream:
            stream.write(content)
    except FileExistsError:
        existing_matches(path, content)
        return 'verified existing'
    return 'created'


def main():
    spec = importlib.util.spec_from_file_location('modern_building_layout', TOOLS / 'prepare_modern_building_layout.py')
    extractor = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(extractor)
    sandbox_path = extractor.PROJECT / 'sandbox-inputs.json'
    sandbox_bytes = sandbox_path.read_bytes()
    sandbox = json.loads(sandbox_bytes)
    layout_path = Path(sandbox['building_layout'])
    layout_bytes = layout_path.read_bytes()
    if digest(layout_bytes) != sandbox['building_layout_sha256']:
        raise ValueError('building layout does not match the frozen sandbox profile')
    layout = json.loads(layout_bytes)
    if layout.get('schema') != 'mc-modern-building-layout/v2':
        raise ValueError('unexpected building layout schema')
    plane, replay_sha, native_sha = extractor.read_frozen_height(sandbox)
    source_sha = digest(plane)
    provenance = layout['provenance']
    if (provenance['source_height_plane_sha256'] != source_sha
            or provenance['replay_sha256'] != replay_sha
            or provenance['native_sha256'] != native_sha):
        raise ValueError('layout provenance differs from the verified frozen snapshot')
    buildings = layout['buildings']
    if len(buildings) != 32 or collections.Counter(b['build_index'] for b in buildings) != {26: 9, 29: 11, 30: 8, 42: 4}:
        raise ValueError('building inventory differs from the qualified layout')

    # Validate the entire patch set against the original plane before mutation.
    patches = {}
    for building in buildings:
        expected_base = building['validation']['base_byte']
        for cell in building['cells']:
            index, expected, replacement = cell['index'], cell['expected'], cell['replacement']
            if (type(index) is not int or not 0 <= index < len(plane)
                    or type(expected) is not int or not 0 <= expected <= 255
                    or type(replacement) is not int or not 0 <= replacement <= 255):
                raise ValueError(f'invalid patch values in slot {building["source_slot"]}')
            if index in patches or plane[index] != expected or replacement != expected_base:
                raise ValueError(f'conflicting or mismatched patch at {index}')
            patches[index] = replacement
    heights = bytearray(plane)
    for index, replacement in patches.items():
        heights[index] = replacement

    matches = [b for b in buildings if b['source_slot'] == SOURCE_SLOT and b['build_index'] == 26]
    if len(matches) != 1:
        raise ValueError('selected Level 1 house is missing or ambiguous')
    selected = matches[0]
    shapes = [p for p in layout['profiles'] if p['build_index'] == selected['build_index']]
    if len(shapes) != 1 or len(shapes[0]['heights']) != shapes[0]['columns'] * shapes[0]['rows']:
        raise ValueError('selected BUILD profile is missing, ambiguous, or malformed')
    if selected['base_height'] <= 0:
        raise ValueError('selected house is not on dry ground')
    placement = {key: selected[key] for key in ('source_slot', 'build_index', 'x', 'z', 'base_height', 'width', 'depth', 'height')}
    source = {
        'schema': 'mc-voxel-level1-input/v1',
        'source_height_sha256': source_sha,
        'stripped_height_sha256': digest(heights),
        'layout_sha256': digest(layout_bytes),
        'sandbox_profile_sha256': digest(sandbox_bytes),
        'replay_sha256': replay_sha,
        'native_sha256': native_sha,
        'snapshot_tick': 240,
        'patch_count': len(patches),
        'index_convention': 'z * 256 + x',
        'units': 'height byte = 0.5 source Unity units; terrain sample spacing = 4 source Unity units',
    }
    result = {'heights': list(heights), 'building': placement, 'profile': shapes[0], 'provenance': source}
    result_bytes = encoded(result)
    receipt_bytes = encoded({'output': str(OUTPUT), 'output_sha256': digest(result_bytes),
                             'selected_source_slot': SOURCE_SLOT, 'provenance': source})
    # Check both files before creating either, to avoid knowingly producing a partial pair.
    existing_matches(OUTPUT, result_bytes)
    existing_matches(RECEIPT, receipt_bytes)
    output_status = create_or_verify(OUTPUT, result_bytes)
    receipt_status = create_or_verify(RECEIPT, receipt_bytes)
    print(json.dumps({'output': str(OUTPUT), 'output_status': output_status, 'receipt': str(RECEIPT),
                      'receipt_status': receipt_status, 'output_sha256': digest(result_bytes),
                      'source_slot': SOURCE_SLOT, 'patch_count': len(patches), 'height_sha256': digest(heights)}))


if __name__ == '__main__':
    main()
