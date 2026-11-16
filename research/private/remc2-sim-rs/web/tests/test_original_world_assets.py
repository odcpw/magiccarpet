"""Qualify the browser bundle against the real, ignored owner cache."""
import hashlib
import json
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


if __name__ == '__main__':
    unittest.main()
