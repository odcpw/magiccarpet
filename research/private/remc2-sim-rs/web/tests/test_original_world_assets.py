"""Qualify the browser bundle against the real, ignored owner cache."""
import hashlib
import json
import struct
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from original_world_assets import ROOT, OriginalWorldAssets, _frames


@unittest.skipUnless((ROOT / 'night-presentation-v6/terrain/materials.json').is_file(), 'local owner cache unavailable')
class WorldAssetsTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.bundle = OriginalWorldAssets()
        cls.m = cls.bundle.manifest
        cls.night = ROOT / 'night-presentation-v6'
        cls.source = json.loads((cls.night / 'terrain/materials.json').read_text())

    def get(self, ref):
        self.assertGreaterEqual(ref['offset'], 0)
        self.assertGreater(ref['length'], 0)
        self.assertLessEqual(ref['offset'] + ref['length'], len(self.bundle.blob))
        return self.bundle.blob[ref['offset']:ref['offset'] + ref['length']]

    def test_terrain_palette_and_orientation(self):
        m = self.m
        self.assertEqual(m['bytes'], len(self.bundle.blob))
        self.assertLess(m['bytes'], 2_000_000)
        t = m['terrain']
        self.assertEqual(m['map_type'], 1)
        sine = struct.unpack('<2048i', self.get(t['sine']))
        self.assertEqual([sine[i] for i in (0, 512, 1024, 1536)], [0, 65536, 0, -65536])
        self.assertEqual((t['count'], t['width'], t['height']), (152, 32, 32))
        tiles = self.get(t['indices'])
        self.assertEqual(tiles, (self.night / self.source['bl32n']['indices_path']).read_bytes())
        for row in self.source['bl32n']['level1_used_tile_sha256']:
            start = row['selector'] * 1024
            self.assertEqual(hashlib.sha256(tiles[start:start + 1024]).hexdigest(), row['sha256'])
        groups = t['uv_groups']
        self.assertEqual(groups, self.source['terrain_uv']['persistent_unity_groups'])
        self.assertEqual(groups[0], [[0, 1], [1, 1], [0, 0], [1, 0]])
        self.assertEqual(len({tuple(map(tuple, g)) for g in groups}), 8)
        for group in groups:
            self.assertEqual(set(map(tuple, group)), {(0, 0), (0, 1), (1, 0), (1, 1)})
        p = m['palette']
        raw = (self.night / self.source['palette']['tablesn_path']).read_bytes()
        self.assertEqual(self.get(p['shade_lut']), raw[:65536])
        self.assertEqual(self.get(p['blend_lut']), raw[0x4000:0x14000])
        self.assertEqual(len(self.get(t['map_colors'])), 256)
        self.assertEqual(self.get(t['map_colors']), raw[0x14000:0x14100])
        self.assertEqual(len(self.get(p['rgb8'])), 768)
        self.assertEqual(len(self.get(p['clrn'])), 4096)

    def test_all_real_selector_frames_and_qualified_goat_states(self):
        rows = {r['selector']: r for r in self.m['tmaps']}
        source_rows = {r['index']: r for r in self.source['tmaps1']['textures']}
        self.assertEqual(set(rows) | set(self.m['unavailable_selectors']), set(self.source['tmaps1']['scene_reachable_selectors']))
        self.assertFalse(set(rows) & set(self.m['unavailable_selectors']))
        self.assertTrue({40, 41, 54, 63, 152, 153, 154, 279, 286, 463}.issubset(rows))
        for selector, row in rows.items():
            original = source_rows[selector]
            self.assertEqual(len(row['frames']), original['frame_count'] + 1 if row['animated'] else 1)
            for ref in row['frames']:
                self.assertEqual(len(self.get(ref)), row['width'] * row['height'])
            self.assertEqual(self.get(row['frames'][0]), (self.night / original['indices_path']).read_bytes())
            if row['animated']:
                self.assertEqual(self.get(row['frames'][0]), self.get(row['frames'][-1]))
        # Independently qualified by the existing native-source Goat golden.
        for selector in (152, 153, 154):
            source = source_rows[selector]
            states = source['animation_states']
            for i, state in enumerate(states[:5]):
                self.assertEqual(hashlib.sha256(self.get(rows[selector]['frames'][i])).hexdigest(), state['sha256'])
        self.assertNotEqual(self.get(rows[463]['frames'][0]), self.get(rows[463]['frames'][1]))

    def test_corrupt_stream_fails_closed(self):
        row = next(r for r in self.source['tmaps1']['textures'] if r['index'] == 152)
        payload = (self.night / row['decoded_payload_path']).read_bytes()
        with self.assertRaises(ValueError):
            _frames(payload[:-1], 152, row['width'], row['height'], True, row['frame_count'])


@unittest.skipUnless((ROOT.parent.parent / 'continuity-20261115/installed-full-resources.json').is_file(),
                     'local campaign resources unavailable')
class MissionWorldAssetsTest(unittest.TestCase):
    def test_package_night_keeps_source_empty_selectors(self):
        resources = json.loads((ROOT.parent.parent / 'continuity-20261115/installed-full-resources.json').read_text())
        resource = resources[0]
        descriptor = json.loads((Path(resource['owner_cache_path']) / 'materials/descriptor.json').read_text())
        bundle = OriginalWorldAssets.from_package(resource)
        self.assertEqual(bundle.manifest['map_type'], 1)
        self.assertEqual(set(bundle.manifest['unavailable_selectors']),
                         {row['index'] for row in descriptor['tmaps']['textures'] if row.get('source_empty')})
        self.assertEqual(bundle.palette,
                         (Path(resource['owner_cache_path']) / descriptor['palette']['rgb8_path']).read_bytes())

    def test_tklom_owner_bundle_uses_real_day_resources(self):
        resources = json.loads((ROOT.parent.parent / 'continuity-20261115/installed-full-resources.json').read_text())
        resource = resources[4]
        root = Path(resource['owner_cache_path'])
        descriptor = json.loads((root / 'materials/descriptor.json').read_text())
        bundle = OriginalWorldAssets.from_package(resource)
        def get(span):
            return bundle.blob[span['offset']:span['offset'] + span['length']]
        self.assertEqual(descriptor['profile']['map_type']['name'], 'Day')
        self.assertEqual(bundle.manifest['map_type'], descriptor['profile']['map_type']['id'])
        sine = struct.unpack('<2048i', get(bundle.manifest['terrain']['sine']))
        self.assertEqual([sine[i] for i in (0, 512, 1024, 1536)], [0, 65536, 0, -65536])
        self.assertEqual(bundle.palette, (root / descriptor['palette']['rgb8_path']).read_bytes())
        self.assertEqual(bundle.palette, get(bundle.manifest['palette']['rgb8']))
        self.assertEqual(get(bundle.manifest['palette']['blend_lut']),
                         (root / descriptor['tables']['path']).read_bytes()[0x4000:0x14000])
        blocks = descriptor['blocks']
        atlas = (root / blocks['atlas_path']).read_bytes()
        tiles = get(bundle.manifest['terrain']['indices'])
        size, width = blocks['tile_size'], blocks['width']
        for tile in (0, 1, 8, blocks['tile_count'] - 1):
            x, y = (tile % (width // size)) * size, (tile // (width // size)) * size
            expected = b''.join(atlas[(y + row) * width + x:(y + row) * width + x + size]
                                for row in range(size))
            self.assertEqual(tiles[tile * size * size:(tile + 1) * size * size], expected)
        rows = {row['selector']: row for row in bundle.manifest['tmaps']}
        source = {row['index']: row for row in descriptor['tmaps']['textures']}
        self.assertEqual(set(rows), set(source))
        self.assertEqual(bundle.manifest['unavailable_selectors'], [])
        for selector, row in rows.items():
            self.assertEqual(get(row['frames'][0]), (root / source[selector]['indices_path']).read_bytes())
        # Day selector 34 has a real multi-child transition, including a type-4
        # palette child which the gameplay decoder deliberately skips.
        source34 = source[34]
        payload = (root / source34['decoded_payload_path']).read_bytes()
        outer = 6 + source34['width'] * source34['height'] + 6
        children = struct.unpack_from('<H', payload, outer + 6)[0]
        self.assertGreater(children, 1)
        child = outer + 16
        types = []
        for _ in range(children):
            length, kind = struct.unpack_from('<IH', payload, child)
            types.append(kind)
            child += length
        self.assertIn(4, types)
        self.assertEqual(get(rows[34]['frames'][0]), get(rows[34]['frames'][-1]))
        wrong = dict(resource, package_dir=resources[0]['package_dir'])
        with self.assertRaises(ValueError):
            OriginalWorldAssets.from_package(wrong)


if __name__ == '__main__':
    unittest.main()
