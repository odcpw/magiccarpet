"""Receipt-bound source gameplay samples and native campaign audio journal."""
from __future__ import annotations

import ctypes as c
import hashlib
import io
import json
from pathlib import Path
import struct
import wave
from collections import deque

from original_assets import _VerifiedCache


class AudioCommand(c.Structure):
    _fields_ = [('abi_version', c.c_uint32), ('struct_size', c.c_uint32),
                ('generation', c.c_uint64), ('event_sequence', c.c_uint64),
                ('state_hash', c.c_uint64), ('asset_ref_index', c.c_uint32),
                ('relative_x', c.c_int32), ('relative_y', c.c_int32), ('relative_z', c.c_int32),
                ('playback_rate_hz', c.c_uint32), ('sound_id', c.c_uint16),
                ('entity_index', c.c_uint16), ('player_index', c.c_int16),
                ('sound_flags', c.c_int16), ('volume_q15', c.c_uint16), ('pan_q16', c.c_uint16),
                ('rate_delta_percent', c.c_int16), ('voice_index', c.c_uint16),
                ('play_type', c.c_uint8), ('loop_count', c.c_int8), ('action', c.c_uint8),
                ('pooled_flush_rank', c.c_uint8), ('reserved', c.c_uint32 * 4)]


if c.sizeof(AudioCommand) != 88:
    raise RuntimeError('unsupported native audio command ABI layout')


class GameplaySamples:
    """Resolve samples lazily from this mission's receipt-bound source bank."""
    def __init__(self, resource: dict):
        level = json.loads((Path(resource['package_dir']) / 'manifest.json').read_bytes())['level']
        self.samples = {}
        self.records = {}
        self.sources = {}
        for cache_path in (resource.get('presentation_cache_path'), resource.get('owner_cache_path')):
            if not cache_path:
                continue
            root = Path(cache_path)
            for prefix in ('', 'components/audio/'):
                if not (root / prefix / 'audio/descriptor.json').is_file():
                    continue
                cache = _VerifiedCache(root)
                descriptor = json.loads(cache.read(prefix + 'audio/descriptor.json'))
                profile = descriptor['profile']
                if (profile['external_level'], profile['internal_level'], profile['map_type']['id']) != (
                        level['external'], level['internal'], level['map_type']['id']):
                    continue
                if descriptor['sound']['bank'] != level['map_type']['id']:
                    raise ValueError('source sound bank does not match selected environment')
                records = descriptor['sound']['records']
                seen = set()
                for row in records:
                    sound_id = row['sound_id']
                    if (type(sound_id) is not int or not 1 <= sound_id <= 69 or sound_id in seen or
                            row['bank'] != descriptor['sound']['bank']):
                        raise ValueError('invalid source sound descriptor')
                    seen.add(sound_id)
                    if sound_id not in self.records:
                        self.records[sound_id] = row
                        self.sources[sound_id] = (cache, prefix)

    def available(self, sound_id: int) -> bool:
        row = self.records.get(sound_id)
        return bool(row and row.get('filename') != 'NULL.WAV' and row.get('asset_path') and row.get('wav'))

    def loop_target(self, sound_id: int) -> int:
        # Source direct ambience targets, as in OwnerAudioPresentation.
        return {1: 70, 2: 70, 5: 120, 31: 85}.get(sound_id, 127)

    def sample(self, sound_id: int) -> bytes | None:
        if not self.available(sound_id): return None
        if sound_id not in self.samples:
            row = self.records[sound_id]
            cache, prefix = self.sources[sound_id]
            data = cache.read(prefix + row['asset_path'])
            if hashlib.sha256(data).hexdigest() != row['asset_sha256']:
                raise ValueError('source gameplay sound hash differs')
            with wave.open(io.BytesIO(data), 'rb') as source:
                expected = row['wav']
                if (source.getnchannels(), source.getsampwidth(), source.getframerate(), source.getnframes()) != (
                        expected['channels'], expected['sample_width_bytes'], expected['sample_rate'], expected['frame_count']):
                    raise ValueError('source gameplay wave format differs')
            self.samples[sound_id] = data
        return self.samples[sound_id]


def event_head(packet: bytes) -> int:
    if len(packet) < 29 or packet[:4] != b'MCPO' or struct.unpack_from('<I', packet, 4)[0] != 5:
        raise ValueError('unsupported native audio observation')
    offset = 29
    for _ in range(3):
        if offset + 4 > len(packet): raise ValueError('truncated native audio identity')
        size, = struct.unpack_from('<I', packet, offset)
        offset += 4 + size
        if offset > len(packet): raise ValueError('truncated native audio identity')
    offset += 6 * 8
    if offset + 8 > len(packet): raise ValueError('truncated native audio head')
    return struct.unpack_from('<Q', packet, offset)[0]


class GameplayJournal:
    """Drain each native event once; native still owns voice arbitration."""
    def __init__(self, native):
        self.native = native
        self.generation = None
        self.cursor = 0
        self.waiting = set()
        self.voices = {}
        self.events = deque(maxlen=128)
        u64, u32, ptr = c.c_uint64, c.c_uint32, c.c_void_p
        for name, args in {
            'mc_game_session_events_json_v1': [u64, u64, u64, u64, u32, ptr, u32, ptr],
            'mc_game_runtime_audio_command_for_event_v1': [u64, u64, u64, ptr, u32],
            'mc_game_runtime_audio_command_release_v1': [u64, u64, u64],
            'mc_game_runtime_audio_command_discard_v1': [u64, u64, u64],
            'mc_game_runtime_audio_voice_complete_v1': [u64, u64, c.c_uint16, u64],
        }.items():
            fn = getattr(native.lib, name); fn.argtypes = args; fn.restype = c.c_int32

    def reset(self):
        self.generation = None
        self.waiting.clear()
        self.voices.clear()
        self.events.clear()

    def complete(self, generation: int, voice: int, sequence: int):
        if generation != self.generation or self.voices.get(voice) != sequence:
            return
        status = self.native.lib.mc_game_runtime_audio_voice_complete_v1(
            self.native.handle, generation, voice, sequence)
        # Pause, restart or source arbitration may already have retired it.
        if status not in (0, 10): self.native.check(status, 'complete gameplay voice')
        self.voices.pop(voice, None)

    def poll(self, observation: bytes, samples: GameplaySamples, playing: bool) -> list[dict]:
        _, generation, tick = self.native.counters()
        head = event_head(observation)
        if self.generation != generation:
            self.generation, self.cursor = generation, head
            self.waiting.clear(); self.voices.clear(); self.events.clear()
            return []
        if not playing:
            self.cursor = head; self.waiting.clear(); self.voices.clear(); self.events.clear()
            return []
        while self.cursor < head:
            page = self.native.read_json(self.native.lib.mc_game_session_events_json_v1,
                                         self.native.handle, generation, tick, self.cursor, 256)['value']
            next_cursor = int(page['next_cursor'])
            if next_cursor <= self.cursor or next_cursor > head:
                raise RuntimeError('native audio journal cursor did not advance')
            for row in page['events']:
                if row['event_id'] in ('audio.sound_prepared', 'audio.loop_ended'):
                    self.waiting.add(int(row['sequence']))
            self.cursor = next_cursor
        preferences = (self.native.identity_value.get('player_preferences') or {}).get('preferences') or {}
        enabled = preferences.get('sound_enabled') is True
        volume = preferences.get('sound_volume', 0)
        if type(volume) is not int or not 0 <= volume <= 127:
            raise RuntimeError('invalid native sound preference')
        if not enabled:
            self.voices.clear(); self.events.clear()
        for sequence in sorted(self.waiting.copy()):
            row = AudioCommand()
            status = self.native.lib.mc_game_runtime_audio_command_for_event_v1(
                self.native.handle, generation, sequence, c.byref(row), c.sizeof(row))
            if status == 10:  # muted, inaudible, or no source sidecar
                self.waiting.remove(sequence); continue
            self.native.check(status, 'read gameplay audio')
            if (row.abi_version, row.struct_size, row.generation, row.event_sequence) != (2, 88, generation, sequence):
                raise RuntimeError('stale native audio command')
            if row.action == 0: continue  # wait for native source-pool flush
            playable = enabled and row.action in (1, 2, 7, 8) and samples.available(row.sound_id)
            if playable:
                if row.voice_index >= 10 or row.loop_count not in (-1, 0) or row.volume_q15 > 32767:
                    raise RuntimeError('invalid native gameplay voice')
                if row.action in (1, 2): self.voices[row.voice_index] = sequence
                self.events.append({'sequence': str(sequence), 'generation': str(generation),
                    'voice': row.voice_index, 'sound': row.sound_id, 'rate': row.playback_rate_hz,
                    'volume_q15': row.volume_q15, 'pan_q16': row.pan_q16, 'sound_volume': volume,
                    'action': row.action, 'loop': row.loop_count == -1,
                    'target': samples.loop_target(row.sound_id), 'play_type': row.play_type})
                if row.action == 8 and row.loop_count == 0 and row.play_type == 4 and row.volume_q15 == 512:
                    self.voices.pop(row.voice_index, None)
                fn = self.native.lib.mc_game_runtime_audio_command_release_v1
            else:
                # Unimplemented sound families cannot retain a source voice.
                fn = self.native.lib.mc_game_runtime_audio_command_discard_v1
            self.native.check(fn(self.native.handle, generation, sequence), 'release gameplay audio')
            self.waiting.remove(sequence)
        return list(self.events)
