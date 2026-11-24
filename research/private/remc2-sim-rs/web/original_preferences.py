"""Bind browser-owned preferences to the native runtime and source controls."""

import json


def install_profile(native, handle, profile_dir):
    """Install before bootstrap or replay load, without writing a profile."""
    identity = native.read_json(native.lib.mc_game_runtime_identity_json_v1, handle)
    payload = json.dumps({"operation": "install", "profile_dir": str(profile_dir)},
                         separators=(",", ":")).encode()
    native.check(native.lib.mc_game_runtime_preferences_command_json_v1(
        handle, int(identity["generation"]), payload, len(payload)), "install preferences")


def toggle_speech(native, available):
    """Run the native source toggle with the current guarded session identity."""
    identity = native.identity_value
    session = identity.get("session")
    if session is None:
        raise ValueError("speech settings require an active mission")
    if not available:
        raise ValueError("speech recordings are unavailable")
    payload = json.dumps({"operation": "source_audio", "action": "speech",
                          "speech_available": True,
                          "session_guard": {"session_id": session["session_id"],
                                            "generation": session["generation"],
                                            "expected_tick": session["tick"]}},
                         separators=(",", ":")).encode()
    native.check(native.lib.mc_game_runtime_preferences_command_json_v1(
        native.handle, int(identity["generation"]), payload, len(payload)), "toggle speech")
    native.identity_value = native.identity()
