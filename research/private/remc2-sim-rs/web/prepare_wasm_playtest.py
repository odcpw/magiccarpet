#!/usr/bin/env python3
"""Prepare owner-local inputs for the one-mission WASM playtest."""

import argparse
import ctypes as c
import gzip
import json
import shutil
import struct
from pathlib import Path

from original_assets import Hud, MinimapEntity, MinimapMark, MinimapState, Objective, OriginalAssets, Slot
from original_bridge import PRIVATE, campaign_packages
from original_world_assets import OriginalWorldAssets


def layout(record):
    formats = {c.c_uint8: 'u8', c.c_int8: 'i8', c.c_uint16: 'u16', c.c_int16: 'i16',
               c.c_uint32: 'u32', c.c_int32: 'i32', c.c_uint64: 'u64'}
    fields = {}
    for name, kind in record._fields_:
        if name.startswith('reserved'):
            continue
        if kind in formats:
            fields[name] = [getattr(record, name).offset, formats[kind]]
        elif kind is Slot:
            fields[name] = [getattr(record, name).offset, layout(kind)]
    return {'size': c.sizeof(record), 'fields': fields}


def add_tree(archive, root, prefix, members):
    root = Path(root).resolve(strict=True)
    for file in sorted(root.rglob('*')):
        if not file.is_file():
            continue
        if not file.resolve().is_relative_to(root):
            raise ValueError(f'package input escapes {root.name}: {file.relative_to(root)}')
        name = (prefix + '/' + file.relative_to(root).as_posix()).encode()
        info = file.stat()
        key = (info.st_dev, info.st_ino)
        if (prior := members.get(key)) is not None:
            archive.write(struct.pack('<HI', len(name), 0xffffffff))
            archive.write(name)
            archive.write(struct.pack('<I', prior))
        else:
            # A hard-linked WAV appears in both caches. Keep both paths with
            # a single copy of those immutable bytes.
            with file.open('rb') as source:
                archive.write(struct.pack('<HI', len(name), info.st_size))
                archive.write(name)
                shutil.copyfileobj(source, archive)
            members[key] = len(members)


def prepare(output, resource_set, help_cache):
    # The runtime requires the complete first three ordinary package manifests.
    # Level 1 needs both the verified owner cache and the presentation cache
    # selected by the installed launcher (including its map font binding).
    packages = campaign_packages(resource_set)[:3]
    if len(packages) != 3:
        raise ValueError('the installed ordinary package prefix is incomplete')
    output.mkdir(parents=True, exist_ok=False)
    with gzip.open(output / 'mission-files.gz', 'wb', compresslevel=3) as archive:
        members = {}
        for index, package in enumerate(packages):
            add_tree(archive, package['package_dir'], f'level{index + 1}', members)
        add_tree(archive, packages[0]['owner_cache_path'], 'owner', members)
        add_tree(archive, packages[0]['presentation_cache_path'], 'presentation', members)
        locale = (help_cache or PRIVATE / 'generated/mc2-campaign-menu-cache-current') / 'language'
        add_tree(archive, locale, 'language', members)
    descriptor = {
        'packages': [dict(package_dir=f'/level{index + 1}', **({
            'owner_cache_path': '/owner', 'presentation_cache_path': '/presentation',
        } if index == 0 else {}))
                     for index in range(3)],
        'layouts': {record.__name__: layout(record) for record in (Hud, MinimapEntity, MinimapMark, MinimapState, Objective)},
    }
    (output / 'mission.json').write_text(json.dumps(descriptor, separators=(',', ':')), encoding='utf-8')
    for name, assets in (('assets', OriginalAssets.from_package(packages[0], help_cache=help_cache)),
                         ('world-assets', OriginalWorldAssets.from_package(packages[0]))):
        (output / name).mkdir()
        (output / name / 'manifest').write_text(json.dumps(assets.manifest, separators=(',', ':')), encoding='utf-8')
        (output / name / 'blob').write_bytes(assets.blob)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--resources', type=Path)
    parser.add_argument('--help-cache', type=Path)
    args = parser.parse_args()
    prepare(args.output, args.resources, args.help_cache)
