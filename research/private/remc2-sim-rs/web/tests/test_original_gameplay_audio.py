import hashlib
import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from original_gameplay_audio import AudioCommand, GameplaySamples


class GameplayAudioTests(unittest.TestCase):
    def test_selected_night_and_day_use_real_receipted_fireball_waves(self):
        import ctypes
        self.assertEqual(ctypes.sizeof(AudioCommand), 88)
        resources = json.loads((Path(__file__).resolve().parents[3] /
            'generated/continuity-20261115/installed-full-resources.json').read_text())
        resources = resources['packages'] if isinstance(resources, dict) else resources
        night, day = GameplaySamples(resources[0]), GameplaySamples(resources[1])
        self.assertEqual(hashlib.sha256(night.sample(9)).hexdigest(),
                         '8bacc231f15be4c688d7abada4bb2531ae4b8a87160633fdb1bbba4c6bad9153')
        self.assertEqual(hashlib.sha256(day.sample(9)).hexdigest(),
                         'cb9c1b97f49089f7b2eee7e097a50ec7cbd896e8d376fb31e301e59e14f3882c')
        self.assertNotEqual(night.sample(3), day.sample(3))
        self.assertTrue(day.available(2))
        self.assertEqual(day.loop_target(2), 70)
        self.assertGreater(len(day.sample(2)), 0)
        self.assertIsNone(day.sample(70))

    def test_payahandra_scroll_uses_missing_receipted_owner_sample(self):
        resources = json.loads((Path(__file__).resolve().parents[3] /
            'generated/continuity-20261115/installed-full-resources.json').read_text())
        resources = resources['packages'] if isinstance(resources, dict) else resources
        resource = resources[1]
        samples = GameplaySamples(resource)
        presentation = Path(resource['presentation_cache_path']) / 'components/audio/audio/descriptor.json'
        owner = Path(resource['owner_cache_path']) / 'audio/descriptor.json'
        presentation_rows = json.loads(presentation.read_text())['sound']['records']
        owner_rows = json.loads(owner.read_text())['sound']['records']
        self.assertNotIn(63, {row['sound_id'] for row in presentation_rows})
        self.assertEqual(next(row for row in owner_rows if row['sound_id'] == 63)['filename'], 'SPELLUP.WAV')
        self.assertEqual(hashlib.sha256(samples.sample(63)).hexdigest(),
                         '3517142ac9fddc3673f5bfbeb6e32af687284078746407d57da39c82281e0e35')
        # The original presentation descriptor remains authoritative when
        # both receipts contain the same sample ID.
        row = next(row for row in presentation_rows if row['sound_id'] == 61)
        self.assertEqual(hashlib.sha256(samples.sample(61)).hexdigest(), row['asset_sha256'])
        self.assertEqual(samples.sources[61][0].root, Path(resource['presentation_cache_path']).resolve())
        self.assertEqual(samples.sources[63][0].root, Path(resource['owner_cache_path']).resolve())
        wrong_bank = dict(resource, owner_cache_path=resources[0]['owner_cache_path'])
        self.assertFalse(GameplaySamples(wrong_bank).available(63),
                         'a Night owner sample must not fill a Day descriptor')


if __name__ == '__main__':
    unittest.main()
