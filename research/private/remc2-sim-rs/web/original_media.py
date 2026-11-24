"""Confined owner-media assets for the Rust frontend media clock."""
from __future__ import annotations

import base64
import hashlib
import json
from pathlib import Path
import threading


class OriginalMedia:
    def __init__(self, root: Path):
        self.root = root.resolve(strict=True)
        self.raw_catalog = (self.root / 'media.json').read_bytes()
        if not 0 < len(self.raw_catalog) <= 240_000:
            raise ValueError('invalid owner media catalog size')
        self.catalog = json.loads(self.raw_catalog)
        if self.catalog.get('schema') != 'mc2-frontend-media/v1':
            raise ValueError('invalid owner media catalog')
        self.outputs = {row['path']: row for row in self.catalog['outputs']}
        if len(self.outputs) != len(self.catalog['outputs']):
            raise ValueError('duplicate owner media receipt')
        self.audio = self.catalog['audio']
        if any(row['path'] not in self.outputs for row in self.audio):
            raise ValueError('unlisted owner media audio')
        self.clips = {row['id']: row for row in self.catalog['clips']}
        if len(self.clips) != len(self.catalog['clips']):
            raise ValueError('duplicate owner media clip')
        self.lock = threading.Lock()
        self.verified = set()
        glyphs = []
        for glyph in self.catalog['glyphs']:
            width, height = glyph['width'], glyph['height']
            data = self.read(glyph['path'])
            if len(data) != width * height or any(value not in (0, 1) for value in data):
                raise ValueError('invalid owner media subtitle glyph')
            glyphs.append({'index': glyph['index'], 'width': width, 'height': height,
                           'mask': base64.b64encode(data).decode('ascii')})
        if len({glyph['index'] for glyph in glyphs}) != len(glyphs):
            raise ValueError('duplicate owner media glyph')
        self.metadata = {'glyphs': glyphs}
        art = self.catalog.get('credits_art')
        if art is not None:
            if (art['width'], art['height'], art['font_width'], art['font_height'],
                    art['advance_x'], art['advance_y']) != (640, 480, 160, 252, 8, 14):
                raise ValueError('invalid owner credits art')
            credits_glyphs = art['glyphs']
            if len(credits_glyphs) != 273:
                raise ValueError('invalid owner credits glyph count')
            for index, glyph in enumerate(credits_glyphs):
                width, height = glyph['width'], glyph['height']
                if (glyph['index'] != index or type(width) is not int or type(height) is not int or
                        not 0 <= width <= 10 or not 0 <= height <= 14 or (width == 0) != (height == 0)):
                    raise ValueError('invalid owner credits glyph')
            mask = self.read(art['font_path'])
            if len(mask) != 160 * 252 or any(value not in (0, 1) for value in mask):
                raise ValueError('invalid owner credits font')
            self._path(art['frames_path'])
            self.metadata['credits'] = {'width': 640, 'height': 480, 'font_width': 160,
                                        'advance_x': 8, 'advance_y': 14, 'glyphs': credits_glyphs,
                                        'mask': base64.b64encode(mask).decode('ascii')}

    def _path(self, relative):
        if not isinstance(relative, str) or not relative or '\\' in relative:
            raise ValueError('invalid owner media path')
        path = Path(relative)
        if path.is_absolute() or any(part in ('.', '..') for part in path.parts):
            raise ValueError('invalid owner media path')
        receipt = self.outputs.get(relative)
        if receipt is None: raise ValueError('unlisted owner media path')
        resolved = (self.root / path).resolve(strict=True)
        if not resolved.is_relative_to(self.root): raise ValueError('owner media path escapes cache')
        return resolved, receipt

    def _verify(self, relative):
        path, receipt = self._path(relative)
        size = receipt['bytes']
        if type(size) is not int or not 0 <= size <= 128_000_000:
            raise ValueError('invalid owner media extent')
        if relative not in self.verified:
            digest = hashlib.sha256()
            with path.open('rb') as stream:
                while chunk := stream.read(1024 * 1024): digest.update(chunk)
                if stream.tell() != size or digest.hexdigest() != receipt['sha256']:
                    raise ValueError(f'owner media receipt mismatch: {relative}')
            self.verified.add(relative)
        return path, size

    def read(self, relative):
        with self.lock:
            path, size = self._verify(relative)
            data = path.read_bytes()
            if len(data) != size: raise ValueError('owner media extent changed')
            return data

    def frame(self, snapshot):
        clip = self.clips.get(snapshot['clip_id'])
        if not clip or snapshot['frames_path'] != clip['frames_path']:
            raise ValueError('unbound native media clip')
        width, height, frame = snapshot['width'], snapshot['height'], snapshot['frame_index']
        if (width, height) != (clip['width'], clip['height']) or (width, height) != (320, 200):
            raise ValueError('unsupported owner media frame extent')
        if type(frame) is not int or not 0 <= frame < clip['frame_count']:
            raise ValueError('invalid owner media frame index')
        stride = width * height + 768
        with self.lock:
            path, size = self._verify(clip['frames_path'])
            if size != stride * clip['frame_count']: raise ValueError('invalid owner media frame count')
            with path.open('rb') as stream:
                stream.seek(stride * frame)
                data = stream.read(stride)
            if len(data) != stride: raise ValueError('truncated owner media frame')
            return data

    def audio_index(self, path):
        if path is None: return None
        matches = [index for index, row in enumerate(self.audio) if row['path'] == path]
        if len(matches) != 1: raise ValueError('unbound native media sound')
        return matches[0]

    def audio_bytes(self, index):
        if type(index) is not int or not 0 <= index < len(self.audio):
            raise ValueError('invalid owner media sound')
        return self.read(self.audio[index]['path'])

    def credits_frame(self):
        art = self.catalog.get('credits_art')
        if not art or 'credits' not in self.metadata: raise ValueError('owner credits art is unavailable')
        data = self.read(art['frames_path'])
        if len(data) != 640 * 480 + 768: raise ValueError('invalid owner credits background')
        return data
