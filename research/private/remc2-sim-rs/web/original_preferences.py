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
    if not available:
        raise ValueError("speech recordings are unavailable")
    apply_source_preference(native, {"operation": "source_audio", "action": "speech",
                                     "speech_available": True})


def apply_source_preference(native, command):
    """Let the current native session apply and persist the source control."""
    identity = native.identity_value
    session = identity.get("session")
    if session is None:
        raise ValueError("source settings require an active mission")
    payload = json.dumps({**command,
                          "session_guard": {"session_id": session["session_id"],
                                            "generation": session["generation"],
                                            "expected_tick": session["tick"]}},
                         separators=(",", ":")).encode()
    native.check(native.lib.mc_game_runtime_preferences_command_json_v1(
        native.handle, int(identity["generation"]), payload, len(payload)), command["operation"])
    native.identity_value = native.identity()
