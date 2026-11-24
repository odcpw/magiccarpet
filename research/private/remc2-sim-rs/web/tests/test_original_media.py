import ctypes as c
import hashlib
import json
from pathlib import Path
import sys
import time
import unittest
from types import SimpleNamespace
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import original_bridge as bridge
from original_bridge import LIBRARY, MEDIA_CACHE
from original_media import OriginalMedia


class OwnerMediaTests(unittest.TestCase):
    def test_credits_use_receipt_bound_source_background_and_font(self):
        media = OriginalMedia(MEDIA_CACHE)
        art = media.catalog['credits_art']
        payload = media.credits_frame()
        self.assertEqual(len(payload), 640 * 480 + 768)
        self.assertEqual(hashlib.sha256(payload).hexdigest(), media.outputs[art['frames_path']]['sha256'])
        self.assertEqual(media.metadata['credits']['glyphs'], art['glyphs'])
        self.assertEqual(len(media.metadata['credits']['glyphs']), 273)
        import base64
        mask = base64.b64decode(media.metadata['credits']['mask'], validate=True)
        self.assertEqual(len(mask), 160 * 252)
        self.assertEqual(hashlib.sha256(mask).hexdigest(), media.outputs[art['font_path']]['sha256'])

    def test_bridge_keeps_results_blocked_until_native_media_releases(self):
        class Native:
            def __init__(self, *_):
                self.handle = SimpleNamespace(value=1); self.language_cache = None; self.lib = None
                self.media_installed = False; self.finished = False
                self.identity_value = self.playing()
            @staticmethod
            def playing():
                return {'mode': 'playing', 'generation': '1', 'session': {'generation': '1', 'tick': '0'},
                        'active_runtime_flow': {'bound_level_id': 'parent'}}
            def player_menu_state(self):
                return 0  # This return starts in ordinary flight.
            @staticmethod
            def result(blocking):
                return {'mode': 'campaign', 'generation': '2', 'session': None,
                        'campaign_menu': {'phase': 'results'},
                        'frontend_media': {'campaign_return': {'blocking': blocking, 'phase': 'cutscene'},
                                           'actions': [], 'active': blocking}}
            def identity(self): return self.identity_value
            def bind_language(self, *_): pass
            def control(self, *_): return b'{}'
            def counters(self): return 1, 1, 0
            def presentation_frame(self): pass
            def frame(self, _): self.minimap_terrain = b''; return b'frame'
            def observe(self): return b'observation'
            def step(self, _): pass
            def finish_if_ready(self):
                if self.finished: return False
                self.finished = True; self.identity_value = self.result(True); return True
            def media_command(self, command):
                if command['operation'] == 'frame': self.identity_value = self.result(False)
            def close(self): pass
        asset = SimpleNamespace(manifest={}, blob=b'')
        with mock.patch.object(bridge, 'Native', Native), \
             mock.patch.object(bridge, 'replay_package', return_value={}), \
             mock.patch.object(bridge.OriginalAssets, 'from_package', return_value=asset), \
             mock.patch.object(bridge, 'pose_from_observation', return_value={}), \
             mock.patch.object(bridge, 'menu_from_observation', return_value=None), \
             mock.patch.object(bridge, 'read_minimap', return_value={'coverage': b''}), \
             mock.patch.object(bridge, 'read_hud', return_value={}), \
             mock.patch.object(bridge, 'read_objective', return_value={}):
            owner = bridge.Bridge(None, [{}])
            try:
                for _ in range(200):
                    if owner.status()['ready']: break
                    time.sleep(.01)
                self.assertTrue(owner.status()['ready'], owner.status()['error'])
                owner.request({'kind': 'control', 'sequence': 0, 'action': 'start'})
                for _ in range(200):
                    if owner.status()['phase'] == 'media': break
                    time.sleep(.01)
                self.assertEqual(owner.status()['phase'], 'media')
                self.assertIsNone(owner.publication())
                reply = owner.request({'kind': 'control', 'sequence': 1, 'action': 'media_frame',
                                       'elapsed_us': 50_000, 'input': False, 'escape': False})
                self.assertEqual(reply['phase'], 'results')
                self.assertIsNone(reply['media'])
            finally:
                owner.close()

    def test_cut1_receipt_bounds_and_real_audio(self):
        media = OriginalMedia(MEDIA_CACHE)
        clip = media.clips['cut1']
        self.assertEqual((clip['width'], clip['height'], clip['frame_count']), (320, 200, 529))
        self.assertTrue(any(row['op'] == 'U' and row['value'] == 266 for row in clip['events']))
        view = dict(clip_id='cut1', frames_path=clip['frames_path'], width=320, height=200, frame_index=0)
        frame = media.frame(view)
        self.assertEqual(len(frame), 64768)
        with (media.root / clip['frames_path']).open('rb') as stream:
            self.assertEqual(frame, stream.read(64768))
        with self.assertRaises(ValueError): media.frame(view | {'frame_index': 529})
        with self.assertRaises(ValueError): media.frame(view | {'frames_path': '../media.json'})
        audio = next(row for row in media.audio if (row['kind'], row['bank'], row['id']) == ('sound', 5, 1))
        payload = media.audio_bytes(media.audio_index(audio['path']))
        receipt = media.outputs[audio['path']]
        self.assertEqual(hashlib.sha256(payload).hexdigest(), receipt['sha256'])
        self.assertEqual(payload[:4], b'RIFF')
        with self.assertRaises(ValueError): media.audio_index('../media.json')

    def test_installed_native_accepts_owner_catalog_and_advances_title_frame(self):
        media = OriginalMedia(MEDIA_CACHE)
        lib = c.CDLL(str(LIBRARY.resolve()))
        lib.mc_game_runtime_create_v1.argtypes = [c.c_void_p]
        lib.mc_game_runtime_destroy_v1.argtypes = [c.c_uint64]
        lib.mc_game_runtime_media_command_json_v1.argtypes = [c.c_uint64, c.c_uint64, c.c_void_p, c.c_uint32]
        lib.mc_game_runtime_identity_json_v1.argtypes = [c.c_uint64, c.c_void_p, c.c_uint32, c.c_void_p]
        for fn in (lib.mc_game_runtime_create_v1, lib.mc_game_runtime_destroy_v1,
                   lib.mc_game_runtime_media_command_json_v1, lib.mc_game_runtime_identity_json_v1):
            fn.restype = c.c_int32
        handle = c.c_uint64()
        self.assertEqual(lib.mc_game_runtime_create_v1(c.byref(handle)), 0)
        def send(command):
            payload = json.dumps(command, separators=(',', ':')).encode()
            self.assertEqual(lib.mc_game_runtime_media_command_json_v1(handle, 1, payload, len(payload)), 0)
        def identity():
            length = c.c_uint32()
            self.assertEqual(lib.mc_game_runtime_identity_json_v1(handle, None, 0, c.byref(length)), 10)
            data = (c.c_ubyte * length.value)()
            self.assertEqual(lib.mc_game_runtime_identity_json_v1(handle, data, len(data), c.byref(length)), 0)
            return json.loads(bytes(data[:length.value]))
        try:
            send({'operation': 'install', 'catalog': media.catalog, 'startup': True, 'audio_available': True})
            send({'operation': 'frame', 'elapsed_us': 50_000, 'input': False, 'escape': False})
            view = identity()['frontend_media']
            self.assertTrue(view['active'])
            self.assertEqual(view['clip_id'], 'welcome')
            self.assertEqual(len(media.frame(view)), 64768)
        finally:
            self.assertEqual(lib.mc_game_runtime_destroy_v1(handle), 0)


if __name__ == '__main__':
    unittest.main()
