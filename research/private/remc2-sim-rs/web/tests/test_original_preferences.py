import ctypes as c
import json
from pathlib import Path
import tempfile
import unittest

from original_bridge import Native
from original_preferences import install_profile


LIBRARY = Path(__file__).resolve().parents[3] / "generated/original-browser-native/libsim_capi.so"


@unittest.skipUnless(LIBRARY.is_file(), "installed browser native library is unavailable")
class NativeProfileTest(unittest.TestCase):
    def native(self):
        native = Native.__new__(Native)
        native.lib = c.CDLL(str(LIBRARY))
        native.handle = c.c_uint64()
        ptr, u64, u32 = c.c_void_p, c.c_uint64, c.c_uint32
        for name, args in {
            "mc_game_runtime_create_v1": [ptr],
            "mc_game_runtime_destroy_v1": [u64],
            "mc_game_runtime_identity_json_v1": [u64, ptr, u32, ptr],
            "mc_game_runtime_preferences_command_json_v1": [u64, u64, ptr, u32],
            "mc_native_last_error_length": [ptr],
            "mc_native_last_error_copy": [ptr, u32, ptr],
        }.items():
            fn = getattr(native.lib, name)
            fn.argtypes, fn.restype = args, c.c_int32
        native.check(native.lib.mc_game_runtime_create_v1(c.byref(native.handle)), "create")
        self.addCleanup(native.lib.mc_game_runtime_destroy_v1, native.handle)
        return native

    def test_profile_reload_and_invalid_fallback_do_not_modify_saves(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            profile = root / "profile"
            save = root / "campaign-save.json"
            save.write_bytes(b"existing campaign save")
            first = self.native()
            install_profile(first, first.handle, profile)
            prefs = first.identity()["player_preferences"]
            self.assertTrue(prefs["preferences"]["speech_enabled"])
            self.assertFalse(profile.exists())

            profile.mkdir()
            path = profile / "player-preferences.json"
            stored = prefs["preferences"]
            stored["speech_enabled"] = False
            path.write_text(json.dumps(stored))
            second = self.native()
            install_profile(second, second.handle, profile)
            self.assertFalse(second.identity()["player_preferences"]["preferences"]["speech_enabled"])
            self.assertEqual(save.read_bytes(), b"existing campaign save")

            invalid = b"not valid preferences"
            path.write_bytes(invalid)
            third = self.native()
            install_profile(third, third.handle, profile)
            fallback = third.identity()["player_preferences"]
            self.assertTrue(fallback["preferences"]["speech_enabled"])
            self.assertTrue(fallback["load_status"].startswith("defaults:"))
            self.assertEqual(path.read_bytes(), invalid)
            self.assertEqual(save.read_bytes(), b"existing campaign save")


if __name__ == "__main__":
    unittest.main()
