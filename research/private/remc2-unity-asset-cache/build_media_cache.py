#!/usr/bin/env python3
"""Owner-local MC2 FMV import. No original media is shipped with the client."""
from __future__ import annotations
import hashlib
import json
import re
import struct
import subprocess
import tempfile
from pathlib import Path
import build_cache as cache

SCHEMA = 'mc2-frontend-media/v1'

class Reader:
    def __init__(self, data: bytes):
        self.data, self.pos = data, 0
    def take(self, n: int) -> bytes:
        if n < 0 or self.pos + n > len(self.data):
            raise cache.CacheError(f'truncated FMV chunk at byte {self.pos}, need {n}')
        value = self.data[self.pos:self.pos+n]
        self.pos += n
        return value
    def number(self, fmt: str) -> int:
        return struct.unpack(fmt, self.take(struct.calcsize(fmt)))[0]

def decode_frames(payload: bytes):
    """Persistent indexed pixels/palette; all reads and writes are bounded.

    The source's height_17DB48 is the scanline width (header field at 8).
    Normal ReadFrame accepts F1FA only; the final ring frame is not played.
    """
    r = Reader(payload)
    size, magic, count, width, height = struct.unpack('<I4H', r.take(12))
    if size != 12 or magic != 0xAF12 or not 1 < count <= 20000 or not (0 < width <= 640 and 0 < height <= 480):
        raise cache.CacheError('invalid MC2 FMV 12-byte header')
    pixels, palette = bytearray(width*height), bytearray(768)
    def put(y, x, data):
        if y < 0 or y >= height or x < 0 or x + len(data) > width:
            raise cache.CacheError(f'FMV write outside {width}x{height} at {x},{y}')
        pixels[y*width+x:y*width+x+len(data)] = data
    for frame in range(count-1):
        extent, kind, chunks = struct.unpack('<IHH', r.take(8))
        r.take(8)
        if kind != 0xF1FA or extent < 16:
            raise cache.CacheError(f'frame {frame}: invalid frame type {kind:#x} or size {extent}')
        body = Reader(r.take(extent-16))
        try:
            for _ in range(chunks):
                length, kind = struct.unpack('<IH', body.take(6))
                if length < 6:
                    raise cache.CacheError('chunk size smaller than header')
                c = Reader(body.take(length-6))
                if kind in (4, 11):
                    packets, index = c.number('<H'), 0
                    for _ in range(packets):
                        index += c.number('<B')
                        n = c.number('<B') or 256
                        if index+n > 256:
                            raise cache.CacheError('palette packet exceeds 256 colors')
                        palette[index*3:(index+n)*3] = c.take(n*3)
                        index += n
                elif kind == 13:
                    pixels[:] = bytes(len(pixels))
                elif kind == 16:
                    pixels[:] = c.take(len(pixels))
                elif kind == 15:
                    for y in range(height):
                        c.take(1)  # obsolete packet count; stop at scanline width
                        x = 0
                        while x < width:
                            n = c.number('<b')
                            if n == 0: raise cache.CacheError('zero BRUN run')
                            data = c.take(1)*n if n > 0 else c.take(-n)
                            put(y, x, data)
                            x += len(data)
                elif kind == 12:
                    y, lines = c.number('<H'), c.number('<H')
                    for y in range(y, y+lines):
                        x = 0
                        for _ in range(c.number('<B')):
                            x += c.number('<B')
                            n = c.number('<b')
                            data = c.take(n) if n >= 0 else c.take(1)*(-n)
                            put(y, x, data)
                            x += len(data)
                elif kind == 7:
                    lines, y = c.number('<H'), 0
                    while lines:
                        op = c.number('<h')
                        if op < 0:
                            if op & 0x4000: y += -op
                            else: put(y, width-1, bytes([op & 255]))
                            continue
                        x = 0
                        for _ in range(op):
                            x += c.number('<B')
                            n = c.number('<b')
                            data = c.take(n*2) if n >= 0 else c.take(2)*(-n)
                            put(y, x, data)
                            x += len(data)
                        y += 1
                        lines -= 1
                # PSTAMP and unknown sized chunks are deliberately skipped.
        except cache.CacheError as e:
            raise cache.CacheError(f'frame {frame}, chunk {kind}: {e}') from e
        # remc2 installs both palette packet forms as VGA channels, without
        # converting COLOUR256. Preserve that arithmetic at the presentation edge.
        yield width, height, bytes(pixels), bytes(palette)

def event_table(name: str) -> list[dict]:
    source = subprocess.run(['git','-C',str(cache.REMC2_GIT),'show',
        f'{cache.PINNED_REMC2_COMMIT}:remc2/engine/MenusAndIntros.cpp'],
        check=True,stdout=subprocess.PIPE,timeout=30).stdout.decode()
    body = source.split(f'Type_SoundEvent_E17CC {name}[', 1)[1].split('};', 1)[0]
    rows = re.findall(r'\{\s*(0x[\da-fA-F]+),\s*(0x[\da-fA-F]+),\s*(0x[\da-fA-F]+)\s*\}', body)
    result, last = [], 0
    for a,b,c in rows:
        frame, op, value = int(a,16), int(b,16), int(c,16)
        # The second source table contains unreachable tail garbage after frame96.
        # The real event cursor stops at its next frame0 row, never sorting it.
        if value == 0xFFFFFFFF or frame < last: break
        result.append(dict(frame=frame, op=chr(op), value=value))
        last = frame
    return result

def build(owner: Path, output: Path, language: int = 1) -> dict:
    owner = owner.resolve(strict=True)
    output.mkdir(parents=True, exist_ok=False)
    outputs = []
    def write(path, data):
        target = output/path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        outputs.append(dict(path=path, bytes=len(data), sha256=hashlib.sha256(data).hexdigest()))
    clips = []
    for clip_id, name, table in [('intro1','INTRO.DAT','str_E17CC_0'),('intro2','INTRO2.DAT','str_E17CC_0x160')]:
        source = cache.confined_source(owner, f'INTRO/{name}').read_bytes()
        frames_path = f'{clip_id}.frames'
        digest = hashlib.sha256()
        count = 0
        with (output/frames_path).open('xb') as target:
            for width,height,pixels,palette in decode_frames(source):
                data = pixels+palette
                digest.update(data)
                target.write(data)
                count += 1
        outputs.append(dict(path=frames_path,bytes=(width*height+768)*count,sha256=digest.hexdigest()))
        clips.append(dict(id=clip_id,width=width,height=height,frame_count=count,frames_path=frames_path,
                          source_sha256=hashlib.sha256(source).hexdigest(),events=event_table(table)))
    with tempfile.TemporaryDirectory(prefix='mc-media-') as temp:
        work = Path(temp)
        decoder, _ = cache.compile_rnc_decoder(work)
        screens = cache.confined_source(owner,'DATA/SCREENS/HSCREEN0.DAT').read_bytes()
        welcome = cache.decode_container_blob(decoder,screens,0x178E5F,64000,work,'welcome')
        pal = cache.decode_container_blob(decoder,screens,0x17C118,768,work,'welcome-pal')
        if len(welcome) != 64000 or len(pal) != 768:
            raise cache.CacheError('welcome screen extent is not 320x200 + 768')
        write('welcome.frames',welcome+pal)
        clips.insert(0,dict(id='welcome',width=320,height=200,frame_count=1,frames_path='welcome.frames',events=[]))
        glyphdata = cache.decode_container_blob(decoder,screens,0x164FCD,struct.unpack_from('>I',screens,0x164FCD+4)[0],work,'intro-font')
        glyphtable = cache.decode_container_blob(decoder,screens,0x165329,struct.unpack_from('>I',screens,0x165329+4)[0],work,'intro-table')
        glyphs=[]
        for index,(offset,width,height) in enumerate(cache.bitmap_records(glyphtable)):
            pixels,mask,_ = cache.materialize_indexed_rle(glyphdata,offset,width,height)
            path=f'glyph-{index}.mask'
            write(path,mask)
            glyphs.append(dict(index=index,width=width,height=height,path=path))
        language_bytes=cache.confined_source(owner,f'LANGUAGE/L{language}.TXT').read_bytes()
        subtitles=[row.decode('cp1252') for row in cache.language_records(language_bytes)]
        audio=[]
        for filename,selector,rows,row,slots in [('SOUND',4,6,3,range(1,20)),('MUSIC',0,4,0,[4,5])]:
            container=cache.confined_source(owner,f'SOUND/{filename}.DAT').read_bytes()
            _,desc=cache.container_descriptor(container,count_rows=rows,selector=selector,row=row,label=filename)
            meta=cache.decode_container_blob(decoder,container,desc['metadata_offset'],desc['metadata_size'],work,filename+'-meta')
            data=cache.decode_container_blob(decoder,container,desc['data_offset'],desc['data_size'],work,filename+'-data')
            records=cache.packed_audio_records(meta,data,expected_count=(len(meta)-32)//32,label=filename)
            if filename=='MUSIC':
                converter,_=cache.compile_xmi_converter(work)
                renderer,_=cache.compile_adl_renderer()
            for slot in slots:
                record=records[slot-1]
                payload=data[record['relative_data_offset']:record['relative_data_offset']+record['bytes']]
                path=f'audio/{filename.lower()}-{slot}.wav'
                if filename=='MUSIC':
                    midi=cache.transcode_xmi(converter,payload,work,f'media-{slot}')
                    source=work/f'media-{slot}.mid'
                    source.write_bytes(midi)
                    target=work/f'media-{slot}.wav'
                    subprocess.run([str(renderer),str(source),str(target)],check=True,timeout=180)
                    payload=target.read_bytes()
                cache.wav_properties(payload,path)
                write(path,payload)
                audio.append(dict(kind=filename.lower(),bank=selector,id=slot,path=path,filename=record['filename']))
    result=dict(schema=SCHEMA,clips=clips,audio=audio,glyphs=glyphs,subtitles=subtitles,
                subtitles_enabled=language!=2,language=language,outputs=outputs,
                music_renderer='libADLMIDI bank59, 2 chips, 44100Hz; host adaptation, not DOS hardware timbre')
    (output/'media.json').write_bytes(cache.canonical_json(result))
    return result
