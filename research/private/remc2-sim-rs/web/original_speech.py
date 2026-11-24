"""Owner-local CD speech segments and the native campaign effect cursor."""
from __future__ import annotations

import hashlib
import io
import json
from collections import OrderedDict
from pathlib import Path
import wave

from original_campaign_effects import CampaignEffect


class SpeechCache:
    """The selected installed cache owns every permitted track and segment."""
    def __init__(self, directory: Path):
        self.root = Path(directory).resolve(strict=True)
        catalog_path = (self.root / 'speech.json').resolve(strict=True)
        if not catalog_path.is_relative_to(self.root): raise ValueError('speech catalog escapes its cache')
        catalog = json.loads(catalog_path.read_bytes())
        if (catalog.get('schema'), catalog.get('sample_rate'), catalog.get('channels'), catalog.get('sample_width')) != (
                'mc2-campaign-speech/v1', 44100, 2, 2):
            raise ValueError('invalid source speech catalog')
        rows = catalog.get('tracks')
        if not isinstance(rows, list) or len(rows) != 27: raise ValueError('incomplete source speech catalog')
        self.tracks = {}
        self.verified = OrderedDict()
        for row in rows:
            number = row.get('track')
            if (type(number) is not int or not 1 <= number <= 27 or number in self.tracks or
                    row.get('path') != f'TRACK{number:02d}.wav' or type(row.get('frames')) is not int or
                    row['frames'] <= 0 or type(row.get('bytes')) is not int or row['bytes'] <= 0 or
                    not isinstance(row.get('sha256'), str) or len(row['sha256']) != 64):
                raise ValueError('invalid source speech track')
            self.tracks[number] = row

    def segment(self, track: int, start_ms: int, length_ms: int) -> bytes:
        row = self.tracks.get(track)
        if row is None or start_ms < 0 or length_ms <= 0 or length_ms > 120000:
            raise ValueError('invalid native speech segment')
        if track not in self.verified:
            path = (self.root / row['path']).resolve(strict=True)
            if not path.is_relative_to(self.root): raise ValueError('speech track escapes its cache')
            data = path.read_bytes()
            if len(data) != row['bytes'] or hashlib.sha256(data).hexdigest() != row['sha256']:
                raise ValueError('source speech track receipt mismatch')
            with wave.open(io.BytesIO(data), 'rb') as source:
                if (source.getnchannels(), source.getsampwidth(), source.getframerate(), source.getnframes()) != (
                        2, 2, 44100, row['frames']):
                    raise ValueError('source speech PCM differs from catalog')
            self.verified[track] = data
            while len(self.verified) > 2: self.verified.popitem(last=False)
        else:
            self.verified.move_to_end(track)
        # Unity retains remc2's integer-millisecond seek truncation.
        start = (start_ms // 1000) * 44100
        frames = (length_ms * 44100 + 999) // 1000
        if start >= row['frames'] or start + frames > row['frames']:
            raise ValueError('native speech segment extends beyond track')
        with wave.open(io.BytesIO(self.verified[track]), 'rb') as source:
            source.setpos(start); pcm = source.readframes(frames)
        if len(pcm) != frames * 4: raise ValueError('truncated native speech segment')
        result = io.BytesIO()
        with wave.open(result, 'wb') as output:
            output.setnchannels(2); output.setsampwidth(2); output.setframerate(44100); output.writeframes(pcm)
        return result.getvalue()


class SpeechJournal:
    """Bind only speech segments selected by actual objective effects."""
    def __init__(self, cache: SpeechCache | None):
        self.cache = cache
        self.events = OrderedDict()
        self.error = None if cache else 'source speech cache unavailable'

    def reset(self):
        self.events.clear()

    def consume(self, effects: list[CampaignEffect]) -> list[dict]:
        for row in effects:
            if row.kind != 0x104: continue
            track, segment = (row.code >> 8) & 255, (row.code >> 16) & 255
            if not track or self.cache is None: continue
            try:
                data = self.cache.segment(track, row.value0, row.value1)
            except (OSError, EOFError, ValueError, wave.Error) as exc:
                self.error = f'{type(exc).__name__}: {exc}'
                self.cache = None
                continue
            key = (row.generation, row.sequence)
            self.events[key] = {'generation': str(row.generation), 'sequence': str(row.sequence),
                'track': track, 'segment': segment, 'start_ms': row.value0, 'length_ms': row.value1,
                'duck': bool(row.code & 2), 'wave': data}
            while len(self.events) > 8: self.events.popitem(last=False)
        return [{key: value for key, value in row.items() if key != 'wave'} for row in self.events.values()]

    def wave(self, generation: int, sequence: int) -> bytes | None:
        row = self.events.get((generation, sequence))
        return row['wave'] if row else None

    def complete(self, generation: int, sequence: int) -> None:
        self.events.pop((generation, sequence), None)
