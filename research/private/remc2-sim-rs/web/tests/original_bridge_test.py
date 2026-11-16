#!/usr/bin/env python3
"""Real local Rust runtime -> HTTP scene -> controls/save/load regression."""
import http.cookiejar
import importlib.util
import json
import struct
import sys
import tempfile
import threading
import time
import unittest
import urllib.error
import urllib.request
from pathlib import Path

PATH = Path(__file__).resolve().parents[1] / "original_bridge.py"
sys.path.insert(0, str(PATH.parent))
spec = importlib.util.spec_from_file_location("original_bridge", PATH)
bridge_module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge_module)


class OriginalBridgeTest(unittest.TestCase):
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
            post("/original-api/control", {"sequence": 30, "action": "resume"})
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
