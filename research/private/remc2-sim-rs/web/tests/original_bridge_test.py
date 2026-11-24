#!/usr/bin/env python3
"""Real local Rust runtime -> HTTP scene -> controls/save/load regression."""
import argparse
import ctypes as c
import gzip
import hashlib
import http.cookiejar
import http.client
import importlib.util
import json
import struct
import sys
import tempfile
import threading
import time
import unittest
from unittest import mock
import urllib.error
import urllib.request
from pathlib import Path

PATH = Path(__file__).resolve().parents[1] / "original_bridge.py"
sys.path.insert(0, str(PATH.parent))
spec = importlib.util.spec_from_file_location("original_bridge", PATH)
bridge_module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge_module)


class OriginalBridgeTest(unittest.TestCase):
    def test_source_speed_uses_live_guards_and_leaves_blocked_action_untouched(self):
        native = bridge_module.Native.__new__(bridge_module.Native)
        native.handle = c.c_uint64(7)
        native.lib = mock.Mock()
        native.identity_value = {'generation': 3, 'session': {'generation': 4, 'tick': 5}, 'source_speed_index': 0}
        native.identity = mock.Mock(return_value={'source_speed_index': 1})
        native.check = mock.Mock()
        fn = native.lib.mc_game_runtime_source_speed_v1
        fn.return_value = 16
        self.assertFalse(native.source_speed())
        native.identity.assert_not_called()
        native.check.assert_not_called()
        self.assertEqual(native.identity_value['source_speed_index'], 0)
        fn.return_value = 0
        self.assertTrue(native.source_speed())
        self.assertEqual(fn.call_args.args[1:], (3, 4, 5, 0))
        native.check.assert_called_once_with(0, 'source game speed')
        self.assertEqual(native.identity_value['source_speed_index'], 1)

    def test_tmap_snapshot_filters_siblings_and_checks_included_extent(self):
        m = bridge_module
        manifest = {'tmaps': [{'selector': 2, 'width': 2, 'height': 2}]}
        row = lambda selector, data: struct.pack('<HI', selector, len(data)) + data
        packet = struct.pack('<H', 2) + row(2, b'abcd') + row(3, b'sibling')
        self.assertEqual(m.decode_tmap_pixels(packet, manifest), [{'selector': 2, 'pixels': 'YWJjZA=='}])
        self.assertEqual(m.decode_tmap_pixels(b'\0\0', manifest), [])
        for invalid in (struct.pack('<H', 1) + row(2, b'bad'), packet[:-1],
                        struct.pack('<H', 2) + row(3, b'x') + row(2, b'abcd'), packet + b'x'):
            with self.assertRaises(ValueError): m.decode_tmap_pixels(invalid, manifest)

    def test_tmap_draw_applies_only_current_playing_or_paused_frame(self):
        m = bridge_module
        native = mock.Mock()
        native.handle = c.c_uint64(7)
        native.identity_value = {'mode': 'playing', 'generation': 3, 'session': {'generation': 4, 'tick': 5}}
        native.lib.mc_game_runtime_tmap_draw_v1.return_value = 0
        value = {'runtime_generation': '3', 'session_generation': '4', 'tick': '5', 'yaw': 22, 'selectors': [2, 3]}
        self.assertTrue(m.submit_tmap_draw(native, value, True))
        args = native.lib.mc_game_runtime_tmap_draw_v1.call_args.args
        self.assertEqual(args[1:5], (3, 4, 5, 22))
        self.assertEqual(list(args[5]), [2, 3])
        self.assertEqual(args[6], 2)
        native.identity_value['mode'] = 'paused'
        self.assertTrue(m.submit_tmap_draw(native, value, True))
        calls = native.lib.mc_game_runtime_tmap_draw_v1.call_count
        for key in ('runtime_generation', 'session_generation', 'tick'):
            self.assertFalse(m.submit_tmap_draw(native, dict(value, **{key: '0'}), True))
        self.assertFalse(m.submit_tmap_draw(native, value, False))
        native.identity_value['session'] = None
        self.assertFalse(m.submit_tmap_draw(native, value, True))
        self.assertEqual(native.lib.mc_game_runtime_tmap_draw_v1.call_count, calls)
        for change in ({'selectors': [3, 2]}, {'selectors': [2, 2]}, {'yaw': 32768}, {'tick': '-1'}):
            with self.assertRaises(ValueError): m.submit_tmap_draw(native, dict(value, **change), True)

    def test_recovery_edge_is_noop_alive_and_reaches_native_when_dead(self):
        m = bridge_module
        native = mock.Mock()
        native.counters.return_value = (3, 4, 5)
        native.handle = c.c_uint64(7)
        native.lib = mock.Mock()
        native.check.side_effect = lambda status, _: self.assertEqual(status, 0)
        command = dict(m.NEUTRAL, recover_player=True)
        self.assertEqual(m.validate_command(command), command)
        with self.assertRaises(ValueError):
            m.validate_command(dict(m.NEUTRAL, recover_player=1))
        hud = {'local_player_entity_index': 12, 'local_player_life': 10}
        with mock.patch.object(m, 'read_hud', return_value=hud):
            self.assertEqual(m.selected_cast_or_recovery(native, command),
                             dict(command, recover_player=False))
            native.lib.mc_native_campaign_entities_copy.assert_not_called()
            hud['local_player_life'] = -1
            self.assertEqual(m.selected_cast_or_recovery(native, command),
                             dict(command, cast_selected=False, recover_player=True))
            native.lib.mc_native_campaign_entities_copy.assert_not_called()
            # The pre-existing selected-cast JSON path still handles a dead
            # player as before; native admission checks the source action.
            self.assertEqual(m.selected_cast_or_recovery(native, dict(m.NEUTRAL, cast_selected=True)),
                             dict(m.NEUTRAL, recover_player=True))

    def test_native_terrain_revision_tracks_plane_bytes_and_reset(self):
        m = bridge_module
        native = m.Native(m.LIBRARY, json.loads(m.PACKAGES.read_text())['packages'])
        try:
            world = m.OriginalWorld(native)
            previous = None
            changes = stable = 0
            for _ in range(25):
                native.presentation_frame()
                _, generation, tick = native.counters()
                revision, terrain, _, _ = world.read(generation, tick, 0, 0)
                if previous:
                    prior_revision, prior_terrain, prior_gzip = previous
                    if terrain != prior_terrain:
                        changes += 1
                        self.assertEqual(revision, prior_revision + 1)
                        self.assertEqual(gzip.decompress(world.terrain_gzip), terrain)
                    else:
                        stable += 1
                        self.assertEqual(revision, prior_revision)
                        self.assertIs(world.terrain_gzip, prior_gzip)
                previous = revision, terrain, world.terrain_gzip
                native.step(dict(m.NEUTRAL, elapsed_centiseconds=3))
            self.assertGreater(changes, 0, 'real startup must mutate the published planes')
            self.assertGreater(stable, 0, 'real startup must contain an unchanged publication')
            native.control('reset', None)
            native.presentation_frame()
            _, generation, tick = native.counters()
            revision, terrain, _, _ = world.read(generation, tick, 0, 1)
            self.assertEqual(revision, previous[0] + 1)
            self.assertEqual(gzip.decompress(world.terrain_gzip), terrain)
            again, same, _, _ = world.read(generation, tick, 0, 1)
            self.assertEqual((again, same), (revision, terrain))
        finally:
            native.close()

    def test_geometry_retains_delayed_publications_until_bounded_eviction(self):
        m = bridge_module
        # Keep the real constructor, publication lookup and HTTP handlers, while
        # controlling publication timing independently of a native 30 Hz loop.
        with mock.patch.object(threading.Thread, 'start'):
            geometry = m.Bridge(None, [], renderer='geometry')
            indexed = m.Bridge(None, [], renderer='indexed')
        self.assertEqual(indexed.publications.maxlen, 8)
        server = m.Server(('127.0.0.1', 0), geometry, None)
        thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
        base = f'http://127.0.0.1:{server.server_port}'
        def append(sequence):
            geometry.publications.append((sequence, b'world-' + bytes([sequence]), {'sequence': sequence},
                                          b'observation-' + bytes([sequence]), None,
                                          b'terrain-' + bytes([sequence]), b'compressed'))
        def get(path):
            with urllib.request.urlopen(base + path, timeout=5) as response: return response.read()
        try:
            for sequence in range(1, 33): append(sequence)
            self.assertEqual(get('/original-api/world-frame?sequence=1'), b'world-\x01')
            self.assertEqual(json.loads(get('/original-api/state?sequence=1')), {'sequence': 1})
            self.assertEqual(get('/original-api/observation?sequence=1'), b'observation-\x01')
            self.assertEqual(get('/original-api/terrain?sequence=1'), b'terrain-\x01')
            append(33)
            for endpoint in ('world-frame', 'state', 'observation', 'terrain'):
                with self.assertRaises(urllib.error.HTTPError) as expired:
                    get(f'/original-api/{endpoint}?sequence=1')
                self.assertEqual(expired.exception.code, 410); expired.exception.close()
            self.assertEqual(get('/original-api/world-frame?sequence=2'), b'world-\x02')
        finally:
            server.shutdown(); server.server_close(); thread.join(timeout=5)

    def test_native_geometry_publication_and_replay(self):
        m = bridge_module
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(m.Native, 'frame', side_effect=AssertionError('geometry rasterized world')):
            bridge = m.Bridge(m.LIBRARY, json.loads(m.PACKAGES.read_text())['packages'], Path(temporary) / 'save.json', 'geometry', modern=True)
            server = m.Server(('127.0.0.1', 0), bridge, None)
            thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
            base = f'http://127.0.0.1:{server.server_port}'
            client = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
            def get(path):
                with client.open(base + path, timeout=30) as response: return response.read()
            def post(sequence, action):
                req = urllib.request.Request(base + '/original-api/control', json.dumps({'sequence': sequence, 'action': action}).encode(),
                                             headers={'Origin': base, 'Content-Type': 'application/json'}, method='POST')
                with client.open(req, timeout=30) as response: return json.loads(response.read())
            def publication():
                packet = get('/original-api/world-frame')
                magic, version, size, outer, generation, tick, sequence, revision, count, row_size, flags, reserved = m.WORLD_HEADER.unpack_from(packet)
                self.assertEqual((magic, version, size, row_size, flags, reserved), (b'MCW1', 1, 64, 64, 0, 0))
                self.assertGreater(count, 0)
                self.assertEqual(len(packet), 64 + 768 + 64 * count)
                state = json.loads(get(f'/original-api/state?sequence={sequence}'))
                self.assertEqual(state['palette_source'], 'native-campaign-rgb8')
                observation = get(f'/original-api/observation?sequence={sequence}')
                self.assertEqual((outer, generation, tick), (state['runtime_generation'], state['session_generation'], state['tick']))
                modern = get(f'/original-api/modern-frame?sequence={sequence}')
                from original_world import MODERN_HEADER
                header = MODERN_HEADER.unpack_from(modern)
                self.assertEqual(header[:8], (b'MCD1', 1, 96, outer, generation, tick, sequence, header[7]))
                self.assertEqual(header[9::2], (52, 96, 48, 96, 56, 64))
                self.assertEqual(len(modern), 96 + sum(header[i] * header[i + 1] for i in (8, 10, 12, 14, 16, 18)))
                self.assertGreater(header[10], 0)
                self.assertGreater(header[12], 0)
                state['modern_epoch'] = header[7]
                self.assertEqual(outer, struct.unpack_from('<Q', observation, 20)[0])
                offset = 29
                for _ in range(3):
                    length = struct.unpack_from('<I', observation, offset)[0]
                    offset += 4 + length
                self.assertEqual((generation, tick), struct.unpack_from('<QQ', observation, offset + 16))
                self.assertIsNone(state['minimap']['terrain'])
                terrain = get(f'/original-api/terrain?sequence={sequence}')
                self.assertEqual(len(terrain), 5 * 65536)
                req = urllib.request.Request(base + f'/original-api/terrain?sequence={sequence}', headers={'Accept-Encoding': 'gzip'})
                with client.open(req, timeout=30) as response:
                    self.assertEqual(response.headers['Content-Encoding'], 'gzip')
                    self.assertEqual(gzip.decompress(response.read()), terrain)
                self.assertEqual(state['source_camera'], m.camera_from_observation(observation))
                for offset in range(64 + 768, len(packet), 64):
                    self.assertEqual(struct.unpack_from('<IIQQ', packet, offset), (2, 64, generation, tick))
                return state, revision, terrain
            try:
                deadline = time.monotonic() + 60
                while True:
                    status = json.loads(get('/original-api/status'))
                    if status['error']: self.fail(status['error'])
                    if status['ready']: break
                    if time.monotonic() > deadline: self.fail('native startup timed out')
                    time.sleep(.1)
                self.assertEqual(status['renderer'], 'geometry')
                self.assertEqual(json.loads(get('/original-api/world-assets/manifest'))['schema'], 'mc-original-world-assets/v1')
                self.assertGreater(len(get('/original-api/world-assets/blob')), 1000)
                first, revision, terrain = publication()
                post(0, 'start')
                move = dict(m.NEUTRAL, longitudinal='accelerate', roll_axis=28, cast_left=True)
                req = urllib.request.Request(base + '/original-api/input', json.dumps({'sequence': 1, 'command': move}).encode(),
                                             headers={'Origin': base, 'Content-Type': 'application/json'}, method='POST')
                with client.open(req, timeout=30): pass
                time.sleep(.25)
                post(2, 'map')
                self.assertTrue(publication()[0]['source_menu_admission']['map_open'])
                post(3, 'map')
                self.assertFalse(publication()[0]['source_menu_admission']['map_open'])
                post(4, 'pause')
                stopped, rev2, same_terrain = publication()
                self.assertGreater(stopped['tick'], first['tick'])
                self.assertNotEqual(stopped['player_pose'], first['player_pose'])
                self.assertGreaterEqual(rev2, revision)
                if rev2 == revision: self.assertEqual(terrain, same_terrain)
                post(5, 'save'); post(6, 'resume'); time.sleep(.15); post(7, 'pause')
                post(8, 'load')
                loaded, rev3, _ = publication()
                self.assertEqual((loaded['tick'], loaded['player_pose']), (stopped['tick'], stopped['player_pose']))
                self.assertGreater(loaded['modern_epoch'], stopped['modern_epoch'])
                self.assertGreater(rev3, rev2)
                post(9, 'reset')
                reset, rev4, _ = publication()
                self.assertEqual(reset['tick'], 0)
                self.assertGreater(reset['modern_epoch'], loaded['modern_epoch'])
                self.assertGreater(rev4, rev3)
                with self.assertRaises(urllib.error.HTTPError) as rejected: get('/original-api/frame')
                self.assertEqual(rejected.exception.code, 404); rejected.exception.close()
            finally:
                server.shutdown(); server.server_close(); bridge.close(); thread.join(timeout=5)

    def test_modern_asset_allowlist_receipt_and_path_bounds(self):
        m = bridge_module
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); world = root / 'world'; world.mkdir()
            data = b'approved glb'; (world / 'house.glb').write_bytes(data)
            (world / 'secret.txt').write_bytes(b'secret')
            manifest = {'schema': 'mc-browser-modern-world/v1', 'files': {
                'house.glb': {'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}}}
            (world / 'manifest.json').write_text(json.dumps(manifest))
            with mock.patch.object(m, 'MODERN_ASSETS', root):
                server = m.Server(('127.0.0.1', 0), object(), None)
                thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
                base = f'http://127.0.0.1:{server.server_port}/original-api/modern-assets/'
                def get(path):
                    with urllib.request.urlopen(base + path, timeout=5) as response: return response.read()
                try:
                    self.assertEqual(get('world/house.glb'), data)
                    self.assertEqual(json.loads(get('world/manifest.json')), manifest)
                    for path, expected in [('world/secret.txt', 404), ('world/../secret.txt', 404),
                                           ('world/%2e%2e', 404), ('characters/manifest.json', 503)]:
                        with self.assertRaises(urllib.error.HTTPError) as error: get(path)
                        self.assertEqual(error.exception.code, expected); error.exception.close()
                    (world / 'house.glb').write_bytes(b'unapproved!')
                    with self.assertRaises(urllib.error.HTTPError) as error: get('world/house.glb')
                    self.assertEqual(error.exception.code, 503); error.exception.close()
                    (world / 'link.glb').symlink_to(world / 'secret.txt')
                    manifest['files']['link.glb'] = {'bytes': 6, 'sha256': hashlib.sha256(b'secret').hexdigest()}
                    (world / 'manifest.json').write_text(json.dumps(manifest))
                    with self.assertRaises(urllib.error.HTTPError) as error: get('world/link.glb')
                    self.assertEqual(error.exception.code, 503); error.exception.close()
                finally:
                    server.shutdown(); server.server_close(); thread.join(timeout=5)

    def test_missing_modern_native_api_keeps_original_available(self):
        m = bridge_module
        with mock.patch.object(m, 'ModernWorld', side_effect=AttributeError('missing modern API')):
            bridge = m.Bridge(m.LIBRARY, json.loads(m.PACKAGES.read_text())['packages'], renderer='geometry', modern=True)
            server = m.Server(('127.0.0.1', 0), bridge, None)
            thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
            base = f'http://127.0.0.1:{server.server_port}'
            try:
                deadline = time.monotonic() + 60
                while not bridge.status()['ready']:
                    if bridge.error: self.fail(bridge.error)
                    if time.monotonic() > deadline: self.fail('native startup timed out')
                    time.sleep(.1)
                self.assertIn('missing modern API', bridge.status()['modern_error'])
                with urllib.request.urlopen(base + '/original-api/world-frame', timeout=30) as response:
                    self.assertEqual(response.read(4), b'MCW1')
                with self.assertRaises(urllib.error.HTTPError) as error:
                    urllib.request.urlopen(base + '/original-api/modern-frame', timeout=30)
                self.assertEqual(error.exception.code, 503); error.exception.close()
            finally:
                server.shutdown(); server.server_close(); bridge.close(); thread.join(timeout=5)

    def test_explicit_host_alias_preserves_same_origin_guard(self):
        m = bridge_module
        class LocalBridge:
            running = False
            def status(self): return {"ready": True, "next_sequence": 0}
            def request(self, value): return {"ok": True, "received": value}

        server = m.Server(("127.0.0.1", 0), LocalBridge(), None, ["3090.bumblebee-hue.ts.net:8882"])
        thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
        conn = http.client.HTTPConnection("127.0.0.1", server.server_port, timeout=5)
        alias = "3090.bumblebee-hue.ts.net:8882"
        try:
            conn.request("GET", "/original-api/status", headers={"Host": "unlisted.example:8882"})
            response = conn.getresponse(); self.assertEqual(response.status, 403); response.read()
            conn.request("GET", "/original-api/status", headers={"Host": alias})
            response = conn.getresponse(); self.assertEqual(response.status, 200)
            controller = response.getheader("Set-Cookie").split(";", 1)[0]; response.read()
            payload = json.dumps({"sequence": 0, "action": "start"})
            conn.request("POST", "/original-api/control", body=payload,
                         headers={"Host": alias, "Origin": "http://evil.example", "Cookie": controller})
            response = conn.getresponse(); self.assertEqual(response.status, 403); response.read()
            conn.request("POST", "/original-api/control", body=payload,
                         headers={"Host": alias, "Origin": "http://" + alias, "Cookie": controller})
            response = conn.getresponse(); self.assertEqual(response.status, 200)
            self.assertEqual(json.loads(response.read())["received"]["action"], "start")
            for invalid in ("*.example:8882", "example", "example:0", "example:65536", "user@example:8882", "example:8882/path"):
                with self.assertRaises(argparse.ArgumentTypeError): m.host_alias(invalid)
        finally:
            conn.close(); server.shutdown(); server.server_close(); thread.join(timeout=5)

    def test_native_frame_controls_and_replay(self):
        m = bridge_module
        temporary = tempfile.TemporaryDirectory()
        save_path = Path(temporary.name) / "save.json"
        bridge = m.Bridge(m.LIBRARY, json.loads(m.PACKAGES.read_text())["packages"], save_path)
        server = m.Server(("127.0.0.1", 0), bridge, None)
        # Port zero is resolved after binding.
        server.allowed_hosts = {f"127.0.0.1:{server.server_port}"}
        thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
        base = f"http://127.0.0.1:{server.server_port}"
        client = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
        def get(path):
            with client.open(base + path, timeout=30) as response: return response.read()
        def post(path, value):
            request = urllib.request.Request(base + path, json.dumps(value).encode(), method="POST",
                                             headers={"Content-Type": "application/json", "Origin": base})
            try:
                with client.open(request, timeout=30) as response: return json.loads(response.read())
            except urllib.error.HTTPError as exc:
                raise AssertionError(f"{path}: {exc.code} {exc.read().decode()}") from exc
        try:
            deadline = time.monotonic() + 60
            while True:
                status = json.loads(get("/original-api/status"))
                if status["error"]: self.fail(status["error"])
                if status["ready"]: break
                if time.monotonic() > deadline: self.fail("native startup timed out")
                time.sleep(.1)
            frame = get("/original-api/frame")
            self.assertEqual(len(frame), 64824)
            magic, version, size, width, height, flags, _, generation, tick, publication, _ = m.HEADER.unpack_from(frame)
            self.assertEqual((magic, version, size, width, height, flags & 1), (b"MCF1", 1, 56, 320, 200, 1))
            state = json.loads(get(f"/original-api/state?sequence={publication}"))
            self.assertEqual((state["session_generation"], state["tick"]), (generation, tick))
            self.assertEqual(state["hud"]["generation"], generation)
            mcpo = get(f"/original-api/observation?sequence={publication}")
            self.assertEqual((mcpo[:4], struct.unpack_from("<I", mcpo, 4)[0]), (b"MCPO", 5))
            post("/original-api/control", {"sequence": 0, "action": "start"})
            stranger = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
            with stranger.open(base + "/original-api/status", timeout=30): pass
            foreign = urllib.request.Request(base + "/original-api/input", json.dumps({"sequence": 1, "command": m.NEUTRAL}).encode(),
                                             method="POST", headers={"Origin": base, "Content-Type": "application/json"})
            with self.assertRaises(urllib.error.HTTPError) as rejected: stranger.open(foreign, timeout=30)
            self.assertEqual(rejected.exception.code, 409); rejected.exception.close()
            move = dict(m.NEUTRAL, longitudinal="accelerate", roll_axis=28)
            for sequence in range(1, 13):
                post("/original-api/input", {"sequence": sequence, "command": move})
                time.sleep(.06)
            post("/original-api/control", {"sequence": 13, "action": "map"})
            map_state = json.loads(get("/original-api/state"))
            self.assertTrue(map_state["source_menu_admission"]["map_open"])
            map_packet = get(f"/original-api/world-map?sequence={map_state['sequence']}")
            self.assertEqual(len(map_packet), 64824)
            map_header = m.HEADER.unpack_from(map_packet)
            self.assertEqual((map_header[0], map_header[5], map_header[9]), (b"MCM1", 0, map_state["sequence"]))
            self.assertIn("entities", map_state["world_map"])
            time.sleep(.15)
            self.assertTrue(json.loads(get("/original-api/state"))["source_menu_admission"]["map_open"])
            post("/original-api/control", {"sequence": 14, "action": "map"})
            self.assertFalse(json.loads(get("/original-api/state"))["source_menu_admission"]["map_open"])
            post("/original-api/control", {"sequence": 15, "action": "menu", "menu_action": "open_spell_menu"})
            self.assertTrue(json.loads(get("/original-api/state"))["source_menu_admission"]["spells_open"])
            post("/original-api/control", {"sequence": 16, "action": "assign_spell", "spell_id": "spell.possession", "tier": 0, "hand": "left"})
            post("/original-api/control", {"sequence": 17, "action": "menu", "menu_action": "close_spell_menu"})
            self.assertFalse(json.loads(get("/original-api/state"))["source_menu_admission"]["spells_open"])
            post("/original-api/control", {"sequence": 18, "action": "pause"})
            stopped = json.loads(get("/original-api/state"))
            self.assertGreater(stopped["tick"], 0)
            self.assertNotEqual(stopped["player_pose"], state["player_pose"])
            time.sleep(.15)
            self.assertEqual(json.loads(get("/original-api/state"))["tick"], stopped["tick"])
            post("/original-api/control", {"sequence": 19, "action": "save"})
            self.assertTrue(save_path.is_file())
            self.assertTrue(json.loads(get("/original-api/status"))["has_save"])
            post("/original-api/control", {"sequence": 20, "action": "resume"})
            self.assertFalse(json.loads(get("/original-api/state"))["paused"],
                             "resume must publish unpaused state before acknowledgement")
            for sequence in range(21, 27):
                post("/original-api/input", {"sequence": sequence, "command": move})
                time.sleep(.06)
            post("/original-api/control", {"sequence": 27, "action": "pause"})
            moved = json.loads(get("/original-api/state"))
            self.assertNotEqual(moved["player_pose"], stopped["player_pose"])
            post("/original-api/control", {"sequence": 28, "action": "load"})
            restored = json.loads(get("/original-api/state"))
            self.assertEqual(restored["player_pose"], stopped["player_pose"])
            self.assertEqual(restored["tick"], stopped["tick"])
            post("/original-api/control", {"sequence": 29, "action": "reset"})
            self.assertEqual(json.loads(get("/original-api/state"))["tick"], 0)
            # Identical retry is acknowledged, not executed twice.
            retry = post("/original-api/control", {"sequence": 29, "action": "reset"})
            self.assertEqual(retry["next_sequence"], 30)
            post("/original-api/control", {"sequence": 30, "action": "start"})
            self.assertFalse(json.loads(get("/original-api/state"))["paused"],
                             "start from pause must publish unpaused state before acknowledgement")
            time.sleep(2)
            watchdog = json.loads(get("/original-api/status"))
            self.assertTrue(watchdog["paused"]); self.assertFalse(watchdog["running"])
            with self.assertRaises(urllib.error.HTTPError) as stale:
                get(f"/original-api/state?sequence={publication}")
            self.assertEqual(stale.exception.code, 410)
            stale.exception.close()
        finally:
            server.shutdown(); server.server_close(); bridge.close(); thread.join(timeout=5); temporary.cleanup()


if __name__ == "__main__": unittest.main()
