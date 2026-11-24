import ctypes
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('original_assets', Path(__file__).resolve().parents[1] / 'original_assets.py')
assets = importlib.util.module_from_spec(spec)
spec.loader.exec_module(assets)


class OwnerAssetTests(unittest.TestCase):
    generated = Path(__file__).resolve().parents[3] / 'generated'

    @staticmethod
    def span(bundle, row):
        return bundle.blob[row['offset']:row['offset'] + row['length']]

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

    def test_real_tklom_and_night_use_selected_map_assets_and_english_exit_help(self):
        help_cache = self.generated / 'exit-help-row83-20261124-candidate'
        resources = [
            ({'package_dir': self.generated / 'dragon-bee-20261105-integration/level5-owner-current/level5',
              'owner_cache_path': self.generated / 'secret-lifecycle-20261111-source4-environment',
              'presentation_cache_path': self.generated / 'secret-lifecycle-20261111-source4-environment'}, 'day'),
            ({'package_dir': assets.ROOT / 'level1',
              'owner_cache_path': assets.ROOT / 'environment-complete/cache-level1',
              'presentation_cache_path': assets.ROOT / 'night-presentation-v6'}, 'night'),
        ]
        menu = json.loads((help_cache / 'manifest.json').read_text())
        language = help_cache / 'language'
        lm = json.loads((language / 'manifest.json').read_text())
        font = next(row for row in lm['fonts'] if row['id'] == 'hfont3')
        glyph_masks = (language / font['masks_path']).read_bytes()
        for resource, profile in resources:
            with self.subTest(profile=profile):
                bundle = assets.OriginalAssets.from_package(resource, help_cache)
                m = bundle.manifest
                env = resource['owner_cache_path']
                descriptor = json.loads((env / 'materials/descriptor.json').read_text())
                bank = next(b for b in descriptor['bitmap_banks'] if b['bank'] == descriptor['profile']['sprites'])
                self.assertEqual([row['selector'] for row in m['symbols']],
                                 [row['selector'] for row in bank['bitmaps'] if not row.get('source_empty')])
                symbol = next(row for row in bank['bitmaps'] if row['selector'] == 84)
                actual = next(row for row in m['symbols'] if row['selector'] == 84)
                self.assertEqual((actual['width'], actual['height']), (symbol['width'], symbol['height']))
                self.assertEqual(self.span(bundle, actual['indices']), (env / symbol['indices_path']).read_bytes())
                self.assertEqual(self.span(bundle, actual['mask']), (env / symbol['mask_path']).read_bytes())
                tables = (env / 'palette/tables.u8').read_bytes()
                self.assertEqual(self.span(bundle, m['normal']), tables[0x4000:0x14000])
                self.assertEqual(self.span(bundle, m['aperture']), tables[0x14300:0x14400])
                expected_exit = ((env / 'palette/day-exit-gtd2.dat').read_bytes() if profile == 'day'
                                 else tables[0x4000:0x14000])
                self.assertEqual(self.span(bundle, m['exit']), expected_exit)
                self.assertEqual((m['map_profile'], m['hud_profile']), (profile, 'night'))
                hint = m['help']['83']
                self.assertEqual((hint['text'], hint['text_record'], hint['flags'], hint['duration'],
                                  hint['age_threshold'], hint['priority']),
                                 ('Fly here to exit the level', 345, 0x280, 50, 100, 5))
                self.assertEqual((hint['width'], hint['height']), (sum(hint['advances']), 14))
                self.assertEqual(len(self.span(bundle, hint['mask'])), hint['width'] * hint['height'])
                self.assertTrue(any(self.span(bundle, hint['mask'])))
                x = 0
                for value, advance in zip(hint['text'].encode('ascii'), hint['advances'], strict=True):
                    glyph = font['glyphs'][33 if value == 32 else value + 1]
                    self.assertEqual(advance, glyph['width'])
                    for y in range(glyph['height']):
                        source = glyph['offset'] + y * glyph['width']
                        result = y * hint['width'] + x
                        expected = bytes(glyph['width']) if value == 32 else glyph_masks[source:source + glyph['width']]
                        self.assertEqual(self.span(bundle, hint['mask'])[result:result + glyph['width']], expected)
                    x += advance
                colors = (env / 'palette/colors.u8').read_bytes()
                self.assertEqual((hint['color'], hint['background'], hint['line_color']),
                                 (colors[4095], colors[0], colors[4095]))
                source_frame = next(row for row in menu['objective']['profiles'] if row['id'] == profile)['sprites']
                for actual_frame, source in zip(hint['frame'], source_frame, strict=True):
                    self.assertEqual(actual_frame['selector'], source['selector'])
                    self.assertEqual(self.span(bundle, actual_frame['indices']),
                                     (help_cache / source['indices_path']).read_bytes())
                    self.assertEqual(self.span(bundle, actual_frame['mask']),
                                     (help_cache / source['mask_path']).read_bytes())

    def test_optional_help_and_confined_receipt_bound_paths(self):
        self.assertIsNone(assets.OriginalAssets().manifest['help'])
        old = self.generated / 'castle-status-help-20261122-candidate'
        self.assertIsNone(assets.OriginalAssets(help_cache=old).manifest['help'])
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / 'cache'
            root.mkdir()
            outside = root.parent / 'secret'
            outside.write_bytes(b'secret')
            (root / 'link').symlink_to(outside)
            digest = hashlib.sha256(b'secret').hexdigest()
            (root / 'manifest.json').write_text(json.dumps({'outputs': [
                {'path': '../secret', 'bytes': 6, 'sha256': digest},
                {'path': 'link', 'bytes': 6, 'sha256': digest}]}))
            cache = assets._VerifiedCache(root)
            for path in ('../secret', 'link'):
                with self.subTest(path=path), self.assertRaises(ValueError):
                    cache.read(path)


if __name__ == '__main__':
    unittest.main()
