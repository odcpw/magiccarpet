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


if __name__ == '__main__':
    unittest.main()
