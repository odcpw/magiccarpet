import ctypes
import io
from pathlib import Path
import sys
import unittest
import wave

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from original_campaign_effects import CampaignEffect, EventBatch, EventQuery
from original_speech import SpeechCache, SpeechJournal


class SpeechTests(unittest.TestCase):
    def test_owner_track_segment_uses_source_second_seek_and_exact_duration(self):
        root = Path(__file__).resolve().parents[3] / 'generated/mc2-campaign-speech-current'
        cache = SpeechCache(root)
        output = cache.segment(1, 10999, 3999)
        with wave.open(io.BytesIO(output), 'rb') as actual, wave.open(str(root / 'TRACK01.wav'), 'rb') as owner:
            owner.setpos(10 * 44100)
            expected_frames = (3999 * 44100 + 999) // 1000
            self.assertEqual(actual.getnframes(), expected_frames)
            self.assertEqual(actual.readframes(expected_frames), owner.readframes(expected_frames))
        with self.assertRaises(ValueError): cache.segment(1, 9999999, 1000)
        with self.assertRaises(ValueError): cache.segment(28, 0, 1000)

    def test_raw_objective_effect_binds_only_native_segment(self):
        self.assertEqual((ctypes.sizeof(EventQuery), ctypes.sizeof(EventBatch), ctypes.sizeof(CampaignEffect)), (40, 56, 72))
        root = Path(__file__).resolve().parents[3] / 'generated/mc2-campaign-speech-current'
        journal = SpeechJournal(SpeechCache(root))
        effect = CampaignEffect(generation=7, sequence=22, kind=0x104,
                                code=(1 << 8) | (1 << 16) | 2, value0=10999, value1=3999)
        cues = journal.consume([effect]); self.assertEqual(len(cues), 1)
        self.assertEqual((cues[0]['track'], cues[0]['segment'], cues[0]['duck']), (1, 1, True))
        self.assertTrue(journal.wave(7, 22)); self.assertIsNone(journal.wave(7, 23))
        journal.complete(7, 22); self.assertIsNone(journal.wave(7, 22))
        self.assertEqual(journal.consume([]), [])


if __name__ == '__main__': unittest.main()
