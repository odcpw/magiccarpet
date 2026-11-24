"""The browser and Original launcher share the installed campaign resources."""

import http.cookiejar
import importlib.util
import json
import os
from pathlib import Path
import runpy
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from unittest import mock
import urllib.request

WEB = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(WEB))
spec = importlib.util.spec_from_file_location('original_bridge', WEB / 'original_bridge.py')
bridge_module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge_module)


class OriginalCampaignResourcesTest(unittest.TestCase):
    def test_default_matches_launcher_and_explicit_override(self):
        m = bridge_module
        launcher = m.CAMPAIGN_LAUNCHER
        script = launcher.read_text().split("<<'PY'\n", 1)[1].rsplit('\nPY', 1)[0]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            helper = root / 'launcher.py'
            helper.write_text(script)
            with mock.patch.object(sys, 'argv', ['-', str(launcher.parent)]):
                namespace = runpy.run_path(str(helper), run_name='campaign_resources_test')
            expected = [{
                'package_dir': str(row['package']),
                'owner_cache_path': str(row['owner']),
                'presentation_cache_path': str(row['presentation']),
            } for row in namespace['default_campaign_resources']()]
            save = root / 'must-not-exist.json'
            env = dict(os.environ, MC_CAMPAIGN_SAVE=str(save), MC_PLAYER_PROJECT=str(root / 'no-player'))
            printed = json.loads(subprocess.check_output([str(launcher), '--print-resources'],
                                                         env=env, text=True, timeout=10))
            self.assertEqual(printed['packages'], expected)
            self.assertFalse(save.exists())
            self.assertEqual(m.campaign_packages(None), expected)
            self.assertEqual(len(expected), 30)
            self.assertTrue(all(set(row) == {'package_dir', 'owner_cache_path', 'presentation_cache_path'}
                                for row in expected))
            self.assertTrue(all(Path(path).is_dir() for row in expected for path in row.values()))
            identities = [json.loads((Path(row['package_dir']) / 'manifest.json').read_text())['level']['internal']
                          for row in expected]
            self.assertEqual(identities, list(range(25)) + list(range(30, 35)))
            explicit = root / 'resources.json'
            explicit.write_text(json.dumps({'packages': expected[:2]}))
            with mock.patch.object(m.subprocess, 'check_output', side_effect=AssertionError('default resolver called')):
                self.assertEqual(m.campaign_packages(explicit), expected[:2])

    def test_fresh_browser_opens_nightspire_with_no_completed_levels(self):
        m = bridge_module
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / 'save.json'
            bridge = m.Bridge(m.LIBRARY, m.campaign_packages(None), save, 'geometry')
            server = m.Server(('127.0.0.1', 0), bridge, None)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            base = f'http://127.0.0.1:{server.server_port}'
            client = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
            def get(path):
                with client.open(base + path, timeout=30) as response:
                    return response.read()
            def post(sequence, action):
                request = urllib.request.Request(base + '/original-api/control',
                    json.dumps({'sequence': sequence, 'action': action}).encode(),
                    headers={'Origin': base, 'Content-Type': 'application/json'}, method='POST')
                with client.open(request, timeout=30) as response:
                    return json.loads(response.read())
            try:
                deadline = time.monotonic() + 180
                while True:
                    status = json.loads(get('/original-api/status'))
                    if status['error']:
                        self.fail(status['error'])
                    if status['ready']:
                        break
                    if time.monotonic() > deadline:
                        self.fail('native campaign startup timed out')
                    time.sleep(.1)
                self.assertEqual(status['level_id'], 'level.magic_carpet_2.night_spire')
                self.assertFalse(status['has_save'])
                self.assertGreater(len(get('/original-api/world-frame')), 64 + 768)
                post(0, 'start')
                post(1, 'pause')
                post(2, 'save')
                self.assertEqual(json.loads(save.read_text())['completed_levels'], [])
            finally:
                server.shutdown()
                server.server_close()
                bridge.close()
                thread.join(timeout=5)


if __name__ == '__main__':
    unittest.main()
