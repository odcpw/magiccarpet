import json
import os
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from original_bridge import NEUTRAL, Native
from original_music import CampaignMusicAsset, CampaignMusicClock, byte_range


PRIVATE = Path(__file__).resolve().parents[3]
PACKAGES = PRIVATE / 'generated/mc-voxel-combat-20261107/native-mana-scenario-01.json'


class OriginalMusicTests(unittest.TestCase):
    def test_real_environment_waves_match_selected_owner_receipts(self):
        resources = json.loads(PACKAGES.read_text())['packages']
        for index, track in ((0, 1), (1, 2), (3, 3)):
            with self.subTest(track=track):
                asset = CampaignMusicAsset(resources[index])
                self.assertEqual(asset.track, track)
                source, size = asset.open()
                with source:
                    self.assertEqual(source.read(4), b'RIFF')
                    self.assertGreater(size, 40_000_000)
                # A second open reuses only a verified unchanged file identity.
                source, _ = asset.open()
                source.close()

    def test_music_ranges_are_bounded(self):
        self.assertIsNone(byte_range(None, 100))
        self.assertEqual(byte_range('bytes=0-9', 100), (0, 9))
        self.assertEqual(byte_range('bytes=90-', 100), (90, 99))
        self.assertEqual(byte_range('bytes=-10', 100), (90, 99))
        for text in ('bytes=100-', 'bytes=2-1', 'bytes=-0', 'bytes=0-1,4-5', 'bytes=x-y'):
            with self.subTest(text=text), self.assertRaises(ValueError):
                byte_range(text, 100)

    def test_real_night_music_reads_do_not_advance_native_state(self):
        resources = json.loads(PACKAGES.read_text())['packages']
        library = Path(os.environ.get('MC_MUSIC_NATIVE', PRIVATE / 'generated/original-browser-native/libsim_capi.so'))
        native = Native(library, resources)
        try:
            clock = CampaignMusicClock(native)
            first, again = clock.state(), clock.state()
            self.assertEqual((first.track, first.playing), (1, 1))
            self.assertEqual((first.state_hash, first.fade_callback_count), (again.state_hash, again.fade_callback_count))
            native.step({**NEUTRAL, 'elapsed_centiseconds': 3})
            clock.after_step()
            stepped = clock.state()
            self.assertEqual(stepped.tick, native.counters()[2])
            self.assertEqual(stepped.track, 1)
        finally:
            native.close()


if __name__ == '__main__':
    unittest.main()
