"""Selected owner music and the native campaign music callback clock."""
from __future__ import annotations

import ctypes as c
import hashlib
import json
import os
from pathlib import Path
import stat
import wave

from original_assets import _VerifiedCache


class MusicSnapshot(c.Structure):
    _fields_ = [('abi_version', c.c_uint32), ('struct_size', c.c_uint32),
                ('generation', c.c_uint64), ('tick', c.c_uint64), ('asset_slot', c.c_uint32),
                ('start_count', c.c_uint32), ('update_count', c.c_uint32),
                ('fade_callback_count', c.c_uint32), ('stop_count', c.c_uint32), ('end_count', c.c_uint32),
                ('initialized', c.c_uint8), ('loaded', c.c_uint8), ('active', c.c_uint8),
                ('playing', c.c_uint8), ('track', c.c_uint8), ('volume', c.c_uint8),
                ('updates_enabled', c.c_uint8), ('timer_active', c.c_uint8), ('target_mode', c.c_uint8),
                ('direction', c.c_int8), ('current_units', c.c_uint8), ('timer_multiplier', c.c_uint8),
                ('timer_hz', c.c_uint16), ('reserved0', c.c_uint16), ('state_hash', c.c_uint64),
                ('reserved', c.c_uint32 * 2)]


if c.sizeof(MusicSnapshot) != 80:
    raise RuntimeError('unsupported campaign music snapshot ABI')


class CampaignMusicClock:
    """Advance the native fade only with completed source simulation steps."""
    def __init__(self, native):
        self.native = native
        self.remainder = 0
        self.previous = None
        ptr = c.POINTER(MusicSnapshot)
        for name, args in {
            'mc_native_campaign_day_music_state': [c.c_uint64, c.c_uint64, c.c_uint64, ptr, c.c_uint32],
            'mc_native_campaign_day_music_advance': [c.c_uint64, c.c_uint64, c.c_uint64, c.c_uint32, ptr, c.c_uint32],
        }.items():
            fn = getattr(native.lib, name)
            fn.argtypes, fn.restype = args, c.c_int32

    def reset(self):
        self.remainder = 0
        self.previous = None

    def state(self):
        _, generation, tick = self.native.counters()
        row = MusicSnapshot()
        status = self.native.lib.mc_native_campaign_day_music_state(
            self.native.handle, generation, tick, c.byref(row), c.sizeof(row))
        if status:
            try:
                self.native.check(status, 'campaign music state')
            except RuntimeError as error:
                if status == 8 and 'requires the qualified Level 2 owner cache' in str(error):
                    return None
                raise
        if (row.abi_version != 2 or row.struct_size != 80 or row.generation != generation or row.tick != tick or
                row.track not in (1, 2, 3) or row.volume > 127 or row.current_units > 127 or
                row.timer_active not in (0, 1) or row.playing not in (0, 1)):
            raise ValueError('invalid native campaign music state')
        return row

    def after_step(self):
        row = self.state()
        if row is None:
            self.reset()
            return
        current = (row.generation, row.start_count, row.target_mode)
        if not row.timer_active or current != self.previous:
            self.remainder = 0
        self.previous = current
        if row.timer_active:
            # Unity CampaignAudioPresenter uses fixedDeltaTime * timerHz.
            self.remainder += row.timer_hz
            callbacks, self.remainder = divmod(self.remainder, 30)
            if callbacks:
                result = MusicSnapshot()
                status = self.native.lib.mc_native_campaign_day_music_advance(
                    self.native.handle, row.generation, row.tick, callbacks, c.byref(result), c.sizeof(result))
                self.native.check(status, 'advance campaign music')

    @staticmethod
    def publication(row):
        return None if row is None else {
            'generation': str(row.generation), 'tick': str(row.tick), 'track': row.track,
            'playing': bool(row.playing), 'start_count': row.start_count,
            'volume': row.volume, 'current_units': row.current_units,
            'target_mode': row.target_mode, 'timer_active': bool(row.timer_active),
            'fade_callback_count': row.fade_callback_count,
        }


class CampaignMusicAsset:
    """A selected, receipt-bound owner WAV, streamed without buffering the track."""
    def __init__(self, resource):
        level = json.loads((Path(resource['package_dir']) / 'manifest.json').read_bytes())['level']
        expected_track = {1: 1, 0: 2, 2: 3}.get(level['map_type']['id'])
        if expected_track is None:
            raise ValueError('unsupported campaign music environment')
        self.root = self.relative = self.receipt = self.wave = None
        self.verified_identity = None
        for cache_path in (resource.get('presentation_cache_path'), resource.get('owner_cache_path')):
            if not cache_path:
                continue
            root = Path(cache_path)
            for prefix in ('', 'components/audio/'):
                if not (root / prefix / 'audio/descriptor.json').is_file():
                    continue
                cache = _VerifiedCache(root)
                descriptor = json.loads(cache.read(prefix + 'audio/descriptor.json'))
                profile, music = descriptor.get('profile') or {}, descriptor.get('music') or {}
                if (profile.get('external_level'), profile.get('internal_level'), (profile.get('map_type') or {}).get('id')) != (
                        level['external'], level['internal'], level['map_type']['id']):
                    continue
                if music.get('logical_track') != expected_track or music.get('maptype_music') != expected_track:
                    raise ValueError('campaign music track does not match its environment')
                relative = prefix + music.get('unity_wave_path', '')
                receipt, details = cache.receipts.get(relative), music.get('unity_wave') or {}
                if not receipt or receipt.get('sha256') != music.get('unity_wave_sha256'):
                    raise ValueError('campaign music wave has no matching owner receipt')
                if (details.get('channels'), details.get('sample_width_bytes'), details.get('sample_rate')) != (2, 2, 44100):
                    raise ValueError('unsupported campaign music PCM format')
                self.root, self.relative, self.receipt, self.wave = cache.root, relative, receipt, details
                self.track = expected_track
                return
        raise ValueError('selected campaign has no receipt-bound music wave')

    def open(self):
        path = (self.root / self.relative).resolve(strict=True)
        if not path.is_relative_to(self.root):
            raise ValueError('campaign music escapes selected cache')
        receipt = self.receipt
        size = receipt.get('bytes')
        if type(size) is not int or not 44 <= size <= 100_000_000:
            raise ValueError('invalid campaign music size')
        source = os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW), 'rb')
        try:
            info = os.fstat(source.fileno())
            if not stat.S_ISREG(info.st_mode) or info.st_size != size:
                raise ValueError('campaign music extent differs from its receipt')
            identity = (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns)
            if identity != self.verified_identity:
                with wave.open(source, 'rb') as wav:
                    if (wav.getnchannels(), wav.getsampwidth(), wav.getframerate(), wav.getnframes()) != (
                            2, 2, 44100, self.wave.get('frame_count')):
                        raise ValueError('campaign music wave differs from its descriptor')
                source.seek(0)
                digest = hashlib.sha256()
                while chunk := source.read(1024 * 1024):
                    digest.update(chunk)
                if digest.hexdigest() != receipt.get('sha256'):
                    raise ValueError('campaign music wave hash differs from its receipt')
                self.verified_identity = identity
            source.seek(0)
            return source, size
        except BaseException:
            source.close()
            raise


def byte_range(header, size):
    """Return an inclusive bounded HTTP byte range, or None for a full response."""
    if not header:
        return None
    if not header.startswith('bytes=') or ',' in header:
        raise ValueError('invalid music byte range')
    first, separator, last = header[6:].partition('-')
    if not separator or not (first.isdecimal() or last.isdecimal()):
        raise ValueError('invalid music byte range')
    if first:
        if not first.isdecimal() or (last and not last.isdecimal()): raise ValueError('invalid music byte range')
        start, end = int(first), min(size - 1, int(last)) if last else size - 1
    else:
        count = int(last)
        if count <= 0: raise ValueError('invalid music byte range')
        start, end = max(0, size - count), size - 1
    if start >= size or end < start:
        raise ValueError('unsatisfiable music byte range')
    return start, end
