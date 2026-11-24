import ctypes
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
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
        self.assertEqual(manifest['objective_stage_records'], list(range(48, 56)))
        self.assertEqual(manifest['objective_end_record'], 53)
        menu = assets._VerifiedCache(assets.DEFAULT_MENU)
        language = assets._VerifiedCache(menu.root / menu.manifest['language_cache']['path'])
        lm = language.manifest
        locale = next(row for row in lm['locales'] if row['id'] == lm['default_locale_id'])
        records = language.read(locale['records_path'])
        def text(selector):
            record = locale['records'][selector]
            raw = records[record['offset']:record['offset'] + record['bytes']]
            self.assertEqual(hashlib.sha256(raw).hexdigest(), record['sha256'])
            return raw.decode('ascii')
        notification = manifest['ability_notification']
        self.assertEqual(notification['format'], text(159))
        self.assertEqual(notification['families'], [text(i) for i in range(160, 186)])
        self.assertEqual(manifest['participant_notices'], {
            'participant.died': text(374), 'participant.banished': text(283),
            'participant.help_mode_on': text(400), 'participant.help_mode_off': text(401),
            'participant.flight_assistance_on': text(398), 'participant.flight_assistance_off': text(399),
            'participant.game_speed_normal': text(381), 'participant.game_speed_fast': text(382),
            'participant.game_speed_super_fast': text(383)})
        colors = (assets.ROOT / 'environment-complete/cache-level1/palette/colors.u8').read_bytes()
        self.assertEqual(notification['color'], colors[3840])
        self.assertEqual([row['selector'] for row in manifest['objectives']], list(range(48, 56)) + [284, 285])
        # The new selected-language path preserves the prior NightSpire
        # objective images byte-for-byte, including frame and text.
        source = json.loads((assets.ROOT / 'night-presentation-v6/hud/level1.json').read_text())
        for row in source['objective']['rows']:
            current = next(item for item in manifest['objectives'] if item['selector'] == row['selector'])
            raster = (assets.ROOT / 'night-presentation-v6' / row['raster_path']).read_bytes()
            plane = row['raster_plane_bytes']
            self.assertEqual(self.span(bundle, current['indices']), raster[:plane])
            self.assertEqual(self.span(bundle, current['mask']), raster[plane:])
        self.assertIn(40, [row['selector'] for row in manifest['sprites']])
        self.assertGreater(len(manifest['symbols']), 200)
        self.assertEqual(ctypes.sizeof(assets.Hud), 272)
        self.assertEqual(ctypes.sizeof(assets.FrameQuery), 64)
        self.assertEqual(ctypes.sizeof(assets.FrameInfo), 128)

    def test_real_payahandra_and_secret_bind_source_objective_records(self):
        resources = json.loads((self.generated / 'continuity-20261115/installed-full-resources.json').read_text())
        if isinstance(resources, dict): resources = resources['packages']
        day = assets.OriginalAssets.from_package(resources[1])
        self.assertEqual(day.manifest['objective_stage_records'], list(range(54, 62)))
        self.assertEqual(day.manifest['objective_end_record'], 59)
        rows = {row['selector']: row for row in day.manifest['objectives']}
        self.assertEqual(rows[54]['text'], '"Seek my sign. There is a spell near there which will find out your enemies."')
        self.assertEqual(day.manifest['objective_stage_records'][3], 57)
        self.assertEqual(rows[57]['text'], '"Good work! Quickly, go to the ruins of Payahandra\'s tower."')
        self.assertEqual(rows[58]['text'], '"Payahandra\'s spells are hidden inside these vaults. Raze them to the ground."')
        self.assertTrue(any(self.span(day, rows[54]['mask'])))
        self.assertNotEqual(self.span(day, rows[54]['mask']), self.span(day, rows[55]['mask']))
        self.assertIn(284, rows)
        self.assertIn(285, rows)
        secret = assets.OriginalAssets.from_package(resources[25])
        self.assertEqual(secret.manifest['objective_stage_records'], [51] * 8)
        self.assertEqual(secret.manifest['objective_end_record'], 101)
        self.assertEqual({row['selector'] for row in secret.manifest['objectives']}, {51, 101, 284, 285})

    def test_real_minimap_subject_pose_matches_native_geometry(self):
        sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
        import original_bridge as bridge
        native = bridge.Native(bridge.LIBRARY, json.loads(bridge.PACKAGES.read_text())['packages'])
        try:
            native.presentation_frame()
            _, generation, tick = native.counters()
            minimap = assets.read_minimap(native.lib, native.handle.value, generation, tick)
            subjects = {row['subject_entity_index'] for row in minimap['entities']
                        if row['visible'] == 1 and row['help_selector'] in (19, 20) and row['subject_entity_index']}
            self.assertTrue(subjects, 'real owner map publishes creature or spell-pickup subjects')
            poses = assets.read_missing_hint_entities(native, subjects)
            self.assertEqual({row['entity'] for row in poses}, subjects)
            world = bridge.OriginalWorld(native)
            _, _, _, count = world.read(generation, tick, 0, 0)
            rendered = {row.entity_index: (row.x, row.y) for row in world.rows[:count]}
            shared = [row for row in poses if row['entity'] in rendered]
            self.assertTrue(shared, 'an actual marked subject is in the same native geometry publication')
            for row in shared:
                self.assertEqual((row['x'], row['y']), rendered[row['entity']])
        finally:
            native.close()

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
                for selector, text, record, flags, age, priority in (
                    (18, 'Possess This Mana', 306, 0x400, 160, 5),
                    (19, 'Destroy this for its mana', 307, 0x400, 160, 5),
                    (20, 'Collect this spell by flying into it', 308, 0, 160, 5),
                    (22, 'This stone restores your life', 310, 0, 320, 4),
                    (25, 'This is an enemy castle', 313, 0, 160, 5),
                    (26, 'This is an enemy balloon', 314, 0, 160, 5),
                    (27, 'Marker stone', 315, 0, 320, 3),
                    (29, 'Castled mana, current and capacity', 317, 544, 800, 2),
                    (30, 'Castle life', 318, 544, 800, 2),
                    (31, 'Current size of castle', 319, 544, 800, 2),
                    (32, 'Mana held by balloons', 320, 544, 800, 2),
                    (33, 'Number of balloons', 321, 544, 800, 2),
                    (34, 'Balloons life', 322, 544, 800, 2),
                    (35, 'Castle mana required', 323, 1568, 800, 2)):
                    row = m['help'][str(selector)]
                    self.assertEqual((row['text'], row['text_record'], row['flags'], row['duration'],
                                      row['age_threshold'], row['priority']), (text, record, flags, 50, age, priority))
                    self.assertEqual(len(self.span(bundle, row['mask'])), row['width'] * row['height'])
                    self.assertTrue(any(self.span(bundle, row['mask'])))
                for selector, xy in ((29, (214, 32)), (30, (214, 16)), (31, (130, 16)),
                                     (32, (320, 32)), (33, (260, 16)), (34, (320, 16)), (35, (0, 0))):
                    row = m['help'][str(selector)]
                    self.assertEqual((row['x'], row['y']), xy)
                self.assertEqual((m['help']['20']['box_width'], m['help']['20']['height']), (188, 28))
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
        self.assertEqual(set(assets.OriginalAssets(help_cache=old).manifest['help']), {'18', '19', '20', '22', '25', '26', '27', '29', '30', '31', '32', '33', '34', '35'})
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
