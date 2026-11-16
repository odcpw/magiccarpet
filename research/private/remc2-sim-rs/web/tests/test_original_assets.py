import ctypes
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('original_assets', Path(__file__).resolve().parents[1] / 'original_assets.py')
assets = importlib.util.module_from_spec(spec)
spec.loader.exec_module(assets)


class OwnerAssetTests(unittest.TestCase):
    def test_bundle_contains_exact_source_palette_table_and_rasters(self):
        bundle = assets.OriginalAssets()
        manifest = bundle.manifest
        span = manifest['normal']
        table = (assets.ROOT / 'environment-complete/cache-level1/palette/tables.u8').read_bytes()
        self.assertEqual(bundle.blob[span['offset']:span['offset'] + span['length']], table[0x4000:0x14000])
        self.assertEqual(manifest['exit'], span)
        self.assertEqual(manifest['objective_background'], 61)
        self.assertEqual([row['selector'] for row in manifest['objectives']], list(range(48, 54)))
        self.assertIn(40, [row['selector'] for row in manifest['sprites']])
        self.assertGreater(len(manifest['symbols']), 200)
        self.assertEqual(ctypes.sizeof(assets.Hud), 272)
        self.assertEqual(ctypes.sizeof(assets.FrameQuery), 64)
        self.assertEqual(ctypes.sizeof(assets.FrameInfo), 128)


if __name__ == '__main__':
    unittest.main()
