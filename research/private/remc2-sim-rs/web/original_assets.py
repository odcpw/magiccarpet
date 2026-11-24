"""Allowlisted local owner assets and campaign HUD observations for Original browser.

No owner data is copied into source control. Construct once on the native owner
thread, and serve only ``manifest`` and ``blob`` through the loopback bridge.
"""
from __future__ import annotations

import ctypes as c
import hashlib
import importlib.util
import json
from functools import cache
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2] / 'generated' / 'settlement-20261105-integration' / 'owner-closed-v6'
PRIVATE = Path(__file__).resolve().parents[2]
DEFAULT_MENU = PRIVATE / 'generated/mc2-campaign-menu-cache-current'
OBJECTIVE_FIRST = (48, 54, 60, 66, 69, 72, 77, 79, 86, 92, 97, 102, 105,
                   110, 115, 118, 124, 126, 131, 133, 136, 140, 143, 151, 156)
OBJECTIVE_END = (53, 59, 65, 68, 71, 76, 78, 85, 91, 96, 101, 104, 109,
                 114, 117, 123, 125, 130, 132, 135, 139, 142, 150, 156, 158)


@cache
def _objective_compositor():
    """Use the same source textbox compositor as the owner asset exporter."""
    path = PRIVATE / 'remc2-unity-asset-cache/build_cache.py'
    spec = importlib.util.spec_from_file_location('mc_owner_cache_objective', path)
    if spec is None or spec.loader is None: raise RuntimeError('source objective compositor unavailable')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.build_objective_rasters


class Slot(c.Structure):
    _fields_ = [('holder_entity_index', c.c_uint16), ('parent_entity_index', c.c_uint16),
                ('raw_spell_selector', c.c_int16), ('color_index', c.c_uint8), ('model', c.c_uint8),
                ('tier', c.c_int8), ('action', c.c_uint8), ('action_marker', c.c_int8),
                ('active_value', c.c_int16), ('activation_value', c.c_uint16), ('control_flags', c.c_uint32),
                ('max_mana', c.c_int32), ('mana_regen', c.c_int32), ('caster_mana', c.c_int32),
                ('frame_selector', c.c_uint16), ('icon_selector', c.c_uint16),
                ('level_text_selector', c.c_uint8), ('draw_admitted', c.c_uint8),
                ('help_unavailable', c.c_uint8), ('reserved_padding', c.c_uint8), ('reserved', c.c_uint32 * 2)]


class Hud(c.Structure):
    _fields_ = [('abi_version', c.c_uint32), ('struct_size', c.c_uint32), ('generation', c.c_uint64),
                ('tick', c.c_uint64), ('local_player_slot', c.c_uint8), ('local_player_active', c.c_uint8),
                ('local_player_color_index', c.c_uint8), ('hit_frame_time_pre_draw', c.c_uint8),
                ('local_player_entity_index', c.c_uint16), ('color_phase_1', c.c_uint8), ('color_phase_2', c.c_uint8),
                ('local_player_control_flags', c.c_uint32), ('local_player_life', c.c_int32),
                ('local_player_max_life', c.c_int32), ('local_player_mana', c.c_int32),
                ('local_player_max_mana', c.c_int32), ('global_mana_denominator', c.c_int32),
                ('minimap_panel_selector', c.c_uint16), ('castle_panel_selector', c.c_uint16),
                ('balloon_panel_selector', c.c_uint16), ('player_panel_selector', c.c_uint16),
                ('player_icon_selector', c.c_uint16), ('life_mana_icon_selector', c.c_uint16),
                ('life_palette_index', c.c_uint8), ('mana_palette_index', c.c_uint8),
                ('mana_max_palette_index', c.c_uint8), ('reserved_0', c.c_uint8),
                ('nyphur', c.c_uint64 * 13), ('left', Slot), ('right', Slot)]


if c.sizeof(Slot) != 48 or c.sizeof(Hud) != 272:
    raise RuntimeError('unsupported native HUD ABI layout')


def read_hud(lib, handle, generation: int, tick: int) -> dict:
    """Read generation/tick-bound native HUD while the caller holds the owner lock."""
    fn = lib.mc_native_campaign_day_hud_state
    fn.argtypes = [c.c_uint64, c.c_uint64, c.c_uint64, c.POINTER(Hud), c.c_uint32]
    fn.restype = c.c_int32
    row = Hud()
    status = fn(handle, generation, tick, c.byref(row), c.sizeof(row))
    if status != 0 or (row.abi_version, row.struct_size, row.generation, row.tick) != (2, 272, generation, tick):
        raise RuntimeError(f'native HUD rejected or stale: status {status}')
    def values(obj):
        return {name: int(getattr(obj, name)) for name, typ in obj._fields_
                if not name.startswith('reserved') and typ not in (Slot, c.c_uint64 * 13)}
    result = values(row)
    result.update(left=values(row.left), right=values(row.right))
    return result


class _VerifiedCache:
    """Read only receipt-bound files below one explicitly selected cache."""

    def __init__(self, root: Path):
        self.root = Path(root).resolve(strict=True)
        manifest_path = (self.root / 'manifest.json').resolve(strict=True)
        if not manifest_path.is_relative_to(self.root):
            raise ValueError('cache manifest escapes selected root')
        self.manifest = json.loads(manifest_path.read_bytes())
        rows = self.manifest.get('outputs')
        if not isinstance(rows, list):
            raise ValueError('cache has no output receipts')
        self.receipts = {row['path']: row for row in rows}
        if len(self.receipts) != len(rows):
            raise ValueError('duplicate cache output receipt')

    def read(self, relative: str) -> bytes:
        if not isinstance(relative, str) or not relative or Path(relative).is_absolute():
            raise ValueError('invalid cache output path')
        path = (self.root / relative).resolve(strict=True)
        if not path.is_relative_to(self.root):
            raise ValueError('cache output escapes selected root')
        receipt = self.receipts.get(relative)
        if receipt is None:
            raise ValueError(f'unlisted cache output: {relative}')
        if (not isinstance(receipt['bytes'], int) or not 0 <= receipt['bytes'] <= 16 * 1024 * 1024 or
                path.stat().st_size != receipt['bytes']):
            raise ValueError(f'invalid cache output size: {relative}')
        data = path.read_bytes()
        if len(data) != receipt['bytes'] or hashlib.sha256(data).hexdigest() != receipt['sha256']:
            raise ValueError(f'cache output receipt mismatch: {relative}')
        return data


class OriginalAssets:
    """Bounded source sprite bundle; HTTP callers never supply filesystem paths."""
    @classmethod
    def from_package(cls, resource_dict: dict, help_cache: Path | None = None):
        package = Path(resource_dict['package_dir']).resolve(strict=True)
        level = json.loads((package / 'manifest.json').read_bytes())['level']
        candidates = [resource_dict.get('presentation_cache_path'), resource_dict.get('owner_cache_path')]
        environment = next((Path(path) for path in candidates if path and
                            (Path(path) / 'materials/descriptor.json').is_file()), None)
        if environment is None:
            raise ValueError('selected package has no qualified environment cache')
        result = cls(environment=environment, help_cache=help_cache)
        profile = result._environment_profile
        if (profile['external_level'], profile['internal_level'], profile['map_type']['id']) != (
                level['external'], level['internal'], level['map_type']['id']):
            raise ValueError('environment cache does not match selected package')
        return result

    def __init__(self, root: Path = ROOT, *, environment: Path | None = None,
                 help_cache: Path | None = None):
        night = root / 'night-presentation-v6'
        environment = environment or root / 'environment-complete' / 'cache-level1'
        hud_cache, env_cache = _VerifiedCache(night), _VerifiedCache(environment)
        source = json.loads(hud_cache.read('hud/level1.json'))
        blob = bytearray()
        def add(data: bytes):
            offset = len(blob)
            blob.extend(data)
            return {'offset': offset, 'length': len(data)}
        tables = env_cache.read('palette/tables.u8')
        if len(tables) != 83456:
            raise ValueError('invalid Night palette tables')
        normal = add(tables[0x4000:0x14000])
        aperture = add(tables[0x14300:0x14400])
        environment_descriptor = json.loads(env_cache.read('materials/descriptor.json'))
        profile = environment_descriptor['profile']
        self._environment_profile = profile
        map_type = profile['map_type']['id']
        if map_type not in (0, 1, 2):
            raise ValueError('unsupported environment map profile')
        exit_table = normal
        if map_type == 0:
            exit_path = environment_descriptor.get('day_exit_tables', {}).get('path')
            if not exit_path:
                raise ValueError('Day environment lacks its source GTD2 table')
            exit_data = env_cache.read(exit_path)
            if len(exit_data) != 65536:
                raise ValueError('invalid Day exit table')
            exit_table = add(exit_data)
        rows = []
        for row in source['bitmap_assets']:
            width, height = row['stored_width'], row['stored_height']
            indices, mask = hud_cache.read(row['indices_path']), hud_cache.read(row['mask_path'])
            if len(indices) != width * height or len(mask) != len(indices):
                raise ValueError(f"invalid source sprite {row['selector']}")
            rows.append({'selector': row['selector'], 'width': width, 'height': height,
                         'runtime_width': row['runtime_width'], 'runtime_height': row['runtime_height'],
                         'indices': add(indices), 'mask': add(mask)})
        tiers = []
        for row in source['spells']['tier_rows']:
            data = hud_cache.read(row['render_path'])
            if len(data) != row['render_width'] * row['render_height'] * 4:
                raise ValueError('invalid source tier raster')
            tiers.append({'tier': row['tier'], 'width': row['render_width'], 'height': row['render_height'],
                          'color': row['palette_index'], 'rgba': add(data)})
        bank_name = profile['sprites']
        bank = next(bank for bank in environment_descriptor['bitmap_banks'] if bank['bank'] == bank_name)
        symbols = []
        for row in bank['bitmaps']:
            if row.get('source_empty'):
                continue
            indices, mask = env_cache.read(row['indices_path']), env_cache.read(row['mask_path'])
            if len(indices) != row['width'] * row['height'] or len(mask) != len(indices):
                raise ValueError(f"invalid minimap symbol {row['selector']}")
            symbols.append({'selector': row['selector'], 'width': row['width'], 'height': row['height'],
                            'indices': add(indices), 'mask': add(mask)})
        font_bank = next(bank for bank in environment_descriptor['bitmap_banks'] if bank['bank'] == 'FONT1')
        font1 = []
        for row in font_bank['bitmaps']:
            if row.get('source_empty'):
                continue
            mask = env_cache.read(row['mask_path'])
            if (row['width'] <= 0 or row['height'] <= 0 or
                    len(mask) != row['width'] * row['height'] or any(value not in (0, 1) for value in mask)):
                raise ValueError(f"invalid source FONT1 glyph {row['selector']}")
            font1.append({'selector': row['selector'], 'width': row['width'], 'height': row['height'],
                          'mask': add(mask)})
        if len({row['selector'] for row in font1}) != len(font1):
            raise ValueError('duplicate source FONT1 glyph')
        colors = env_cache.read('palette/colors.u8')
        if len(colors) != 4096:
            raise ValueError('invalid Night color table')
        objectives, objective_stages, objective_end, gameplay_font, ability_notification, participant_notices = self._objectives(
            help_cache or DEFAULT_MENU, profile, colors, add)
        help_rows = self._exit_help(help_cache, profile, colors, add) if help_cache else None
        self.blob = bytes(blob)
        self.manifest = {'schema': 'mc-original-assets/v1', 'bytes': len(blob), 'normal': normal,
                         'objective_background': colors[0], 'map_cross_color': colors[4095],
                         'exit': exit_table, 'aperture': aperture, 'sprites': rows, 'symbols': symbols,
                         'objectives': objectives, 'tiers': tiers, 'help': help_rows, 'font1': font1,
                         'gameplay_font': gameplay_font, 'ability_notification': ability_notification,
                         'participant_notices': participant_notices,
                         'objective_stage_records': objective_stages, 'objective_end_record': objective_end,
                         'map_profile': profile['map_type']['name'].lower(), 'hud_profile': 'night'}

    @staticmethod
    def _objectives(menu_path: Path, profile: dict, colors: bytes, add):
        # GameUI.cpp's level-indexed English strings; secret source levels
        # 30..34 use the fixed stage/end pair 51/101.
        internal = profile['internal_level']
        if type(internal) is not int: raise ValueError('invalid objective source level')
        if 0 <= internal < len(OBJECTIVE_FIRST):
            first, end = OBJECTIVE_FIRST[internal], OBJECTIVE_END[internal]
            stages = list(range(first, first + 8))
        elif 30 <= internal <= 34:
            stages, end = [51] * 8, 101
        else:
            raise ValueError('unsupported objective source level')
        menu = _VerifiedCache(menu_path)
        manifest = menu.manifest
        if (manifest.get('schema') != 'mc2-owner-campaign-menu-cache/v3' or
                manifest.get('source_commit') != '6806c7204d17220ecc0430759f784aea8ee2ddb2'):
            raise ValueError('unexpected source objective cache identity')
        language_root = (menu.root / manifest['language_cache']['path']).resolve(strict=True)
        if not language_root.is_relative_to(menu.root): raise ValueError('language cache escapes selected root')
        language = _VerifiedCache(language_root)
        lm = language.manifest
        if (lm.get('binding_sha256') != manifest['language_cache']['binding_sha256'] or
                lm.get('default_locale_id') != manifest['language_cache']['default_locale_id'] or
                lm.get('glyph_mapping') != 'mc2-byte-plus-one-controls/v1'):
            raise ValueError('source objective language binding differs')
        locale = next(row for row in lm['locales'] if row['id'] == lm['default_locale_id'])
        if 'LANGUAGE/L1.TXT' not in locale['aliases'] or locale['pack_sha256'] != manifest['source_language_sha256']:
            raise ValueError('source objective locale is not the selected English language')
        if manifest['objective']['font_id'] != 'hfont3' or 'hfont3' not in locale['font_ids']:
            raise ValueError('source objective font differs')
        font = next(row for row in lm['fonts'] if row['id'] == 'hfont3')
        masks = language.read(font['masks_path'])
        gameplay_font = []
        for glyph in font['glyphs']:
            offset, count = glyph['offset'], glyph['length']
            data = masks[offset:offset + count]
            if (offset < 0 or count != glyph['width'] * glyph['height'] or len(data) != count or
                    hashlib.sha256(data).hexdigest() != glyph['sha256'] or any(value not in (0, 1) for value in data)):
                raise ValueError('invalid source objective glyph')
            gameplay_font.append({'selector': glyph['index'], 'width': glyph['width'],
                                  'height': glyph['height'], 'mask': add(data)})
        name = profile['map_type']['name'].lower()
        source_profile = next(row for row in manifest['objective']['profiles'] if row['id'] == name)
        sprites = {}
        for sprite in source_profile['sprites']:
            indices, mask = menu.read(sprite['indices_path']), menu.read(sprite['mask_path'])
            size = sprite['width'] * sprite['height']
            if len(indices) != size or len(mask) != size or any(value not in (0, 1) for value in mask):
                raise ValueError('invalid source objective frame')
            sprites[sprite['selector']] = (indices, mask, sprite['width'], sprite['height'])
        if set(sprites) != {171, 172, 173}: raise ValueError('source objective frame selectors differ')
        palette = menu.read(source_profile['palette_path'])
        if len(palette) != 768 or any(value > 63 for value in palette):
            raise ValueError('invalid source objective palette')
        rgb = bytes(value * 4 for value in palette)
        records = language.read(locale['records_path'])
        rows = []
        for selector in sorted(set(stages + [end, 284, 285])):
            record = locale['records'][selector]
            start, length = record['offset'], record['bytes']
            text = records[start:start + length]
            if (record['selector'] != selector or start < 0 or not 0 < length <= 4096 or
                    len(text) != length or hashlib.sha256(text).hexdigest() != record['sha256'] or
                    any(value < 32 or value > 126 for value in text)):
                raise ValueError('invalid English objective record')
            data, _, geometry = _objective_compositor()(
                text, font['glyphs'], masks, sprites, rgb, colors[3840], 1)
            width, height = geometry['objective_width'], geometry['objective_height']
            plane = width * height
            blend = geometry['objective_blend_rect']
            if len(data) != plane * 2: raise ValueError('invalid source objective raster')
            rows.append({'selector': selector, 'text': text.decode('ascii'),
                         'reference_x': geometry['objective_x'], 'reference_y': geometry['objective_y'],
                         'render_width': width, 'render_height': height,
                         'blend_x': blend['x'], 'blend_y': blend['y'],
                         'blend_width': blend['width'], 'blend_height': blend['height'],
                         'indices': add(data[:plane]), 'mask': add(data[plane:])})
        ability_records = []
        for selector in range(159, 186):
            record = locale['records'][selector]
            start, length = record['offset'], record['bytes']
            text = records[start:start + length]
            if (record['selector'] != selector or start < 0 or not 0 < length <= 4096 or
                    len(text) != length or hashlib.sha256(text).hexdigest() != record['sha256'] or
                    any(value < 32 or value > 126 for value in text)):
                raise ValueError('invalid source spell notification language')
            ability_records.append(text.decode('ascii'))
        if ability_records[0].count('%s') != 1:
            raise ValueError('invalid source spell notification format')
        participant_notices = {}
        for kind, selector in (('participant.died', 374), ('participant.banished', 283),
                               ('participant.help_mode_on', 400), ('participant.help_mode_off', 401)):
            record = locale['records'][selector]
            start, length = record['offset'], record['bytes']
            text = records[start:start + length]
            if (record['selector'] != selector or start < 0 or not 0 < length <= 4096 or
                    len(text) != length or hashlib.sha256(text).hexdigest() != record['sha256'] or
                    any(value < 32 or value > 126 for value in text)):
                raise ValueError('invalid source participant notification language')
            participant_notices[kind] = text.decode('ascii')
        return rows, stages, end, gameplay_font, {'format': ability_records[0],
                                                   'families': ability_records[1:],
                                                   'color': colors[3840]}, participant_notices

    @staticmethod
    def _exit_help(help_cache: Path, profile: dict, colors: bytes, add) -> dict | None:
        menu = _VerifiedCache(help_cache)
        manifest = menu.manifest
        if (manifest.get('schema') != 'mc2-owner-campaign-menu-cache/v3' or
                manifest.get('source_commit') != '6806c7204d17220ecc0430759f784aea8ee2ddb2' or
                manifest.get('gameplay_help_path') != 'gameplay-help.json'):
            raise ValueError('unexpected source help cache identity')
        table = json.loads(menu.read('gameplay-help.json'))
        if (table.get('schema') != 'mc2-gameplay-help/v1' or table.get('source_sha256') !=
                'a5d1812f796c690cd0fc7ea7f46cdb64bd9b73d999c632b04f9ea1e2806f4386'):
            raise ValueError('unexpected source help table identity')
        expected = {
            19: dict(row=19, kind=2, flags=0x400, text_record=307, duration=50, x=0, y=0, age_threshold=160, priority=5),
            20: dict(row=20, kind=2, flags=0, text_record=308, duration=50, x=0, y=0, age_threshold=160, priority=5),
            83: dict(row=83, kind=8, flags=0x280, text_record=345, duration=50, x=0, y=0, age_threshold=100, priority=5),
        }
        rows = [row for row in table['rows'] if row['row'] in expected]
        if not rows:
            return None
        if len({row['row'] for row in rows}) != len(rows) or any(row != expected[row['row']] for row in rows):
            raise ValueError('source map help row differs')
        language_path = manifest['language_cache']['path']
        language_root = (menu.root / language_path).resolve(strict=True)
        if not language_root.is_relative_to(menu.root):
            raise ValueError('language cache escapes selected root')
        language = _VerifiedCache(language_root)
        lm = language.manifest
        if (lm.get('binding_sha256') != manifest['language_cache']['binding_sha256'] or
                lm.get('default_locale_id') != manifest['language_cache']['default_locale_id'] or
                lm.get('glyph_mapping') != 'mc2-byte-plus-one-controls/v1'):
            raise ValueError('source language binding differs')
        locale = next(row for row in lm['locales'] if row['id'] == lm['default_locale_id'])
        if 'LANGUAGE/L1.TXT' not in locale['aliases'] or locale['pack_sha256'] != manifest['source_language_sha256']:
            raise ValueError('source help locale is not the selected English language')
        records = language.read(locale['records_path'])
        font_id = manifest['objective']['font_id']
        if font_id != 'hfont3' or font_id not in locale['font_ids']:
            raise ValueError('source help font differs')
        font = next(row for row in lm['fonts'] if row['id'] == font_id)
        masks = language.read(font['masks_path'])
        glyphs = font['glyphs']
        name = profile['map_type']['name'].lower()
        source_profile = next(row for row in manifest['objective']['profiles'] if row['id'] == name)
        frame = []
        for sprite in source_profile['sprites']:
            indices, mask = menu.read(sprite['indices_path']), menu.read(sprite['mask_path'])
            size = sprite['width'] * sprite['height']
            if len(indices) != size or len(mask) != size or any(value not in (0, 1) for value in mask):
                raise ValueError('invalid source help frame sprite')
            frame.append(dict(selector=sprite['selector'], width=sprite['width'], height=sprite['height'],
                              indices=add(indices), mask=add(mask)))
        if [row['selector'] for row in frame] != [171, 172, 173]:
            raise ValueError('source help frame selectors differ')
        texts = {19: b'Destroy this for its mana', 20: b'Collect this spell by flying into it',
                 83: b'Fly here to exit the level'}
        result = {}
        for row in rows:
            selector = row['row']
            record = locale['records'][row['text_record']]
            offset, length = record['offset'], record['bytes']
            text = records[offset:offset + length]
            if (record['selector'] != row['text_record'] or offset < 0 or length > 4096 or len(text) != length or
                    hashlib.sha256(text).hexdigest() != record['sha256'] or text != texts[selector]):
                raise ValueError('source map help text differs')
            selected = [glyphs[33 if value in (9, 32) else value + 1] for value in text]
            advances = []
            for glyph in selected:
                start, count = glyph['offset'], glyph['length']
                data = masks[start:start + count]
                if (start < 0 or count != glyph['width'] * glyph['height'] or len(data) != count or
                        hashlib.sha256(data).hexdigest() != glyph['sha256'] or any(v not in (0, 1) for v in data)):
                    raise ValueError('invalid source help glyph')
                advances.append(glyph['width'])
            box_width = min(220, 8 * len(text))
            if 8 * len(text) > box_width:
                while box_width > 0 and (8 * len(text)) % box_width < box_width // 2:
                    box_width -= 8
            words = text.split(b' ')
            lines, line = [], b''
            for word in words:
                if not line: line = word
                elif len(line) + len(word) + 1 <= box_width // 8: line += b' ' + word
                else: lines.append(line); line = word
            if line: lines.append(line)
            width = max(sum(glyphs[33 if value == 32 else value + 1]['width'] for value in line) for line in lines)
            height = 14 * len(lines)
            if not (0 < width <= box_width <= 220 and 0 < height <= 64):
                raise ValueError('source help raster exceeds bounds')
            raster = bytearray(width * height)
            for line_number, line in enumerate(lines):
                x = 0
                for value in line:
                    glyph = glyphs[33 if value == 32 else value + 1]
                    if value != 32:
                        for y in range(glyph['height']):
                            start = glyph['offset'] + y * glyph['width']
                            at = (line_number * 14 + y) * width + x
                            raster[at:at + glyph['width']] = masks[start:start + glyph['width']]
                    x += glyph['width']
            result[str(selector)] = dict(text=text.decode('ascii'), text_record=row['text_record'], flags=row['flags'],
                duration=row['duration'], age_threshold=row['age_threshold'], priority=row['priority'], width=width,
                height=height, box_width=box_width, mask=add(raster), advances=advances, frame=frame,
                color=colors[4095], background=colors[0], line_color=colors[4095])
        return result

def read_missing_hint_entities(native, subjects):
    """Read missing map subjects on the owner thread; geometry carries the usual poses."""
    if not subjects:
        return []
    from original_world import CampaignEntity
    if c.sizeof(CampaignEntity) != 52 or any(type(slot) is not int or not 0 < slot < 65536 for slot in subjects):
        raise ValueError('invalid source help subject')
    lib = native.lib
    lib.mc_native_campaign_entity_count.argtypes = [c.c_uint64, c.c_void_p]
    lib.mc_native_campaign_entities_copy.argtypes = [c.c_uint64, c.c_uint32, c.c_void_p, c.c_uint32, c.c_uint32, c.c_void_p]
    lib.mc_native_campaign_entity_count.restype = lib.mc_native_campaign_entities_copy.restype = c.c_int32
    count = c.c_uint32()
    native.check(lib.mc_native_campaign_entity_count(native.handle, c.byref(count)), 'help entity count')
    first, last = min(subjects), max(subjects)
    if last >= count.value: raise ValueError('source help subject is outside entity table')
    rows = (CampaignEntity * (last - first + 1))()
    written = c.c_uint32()
    native.check(lib.mc_native_campaign_entities_copy(native.handle, first, rows, len(rows),
                 c.sizeof(CampaignEntity), c.byref(written)), 'help entity poses')
    if written.value != len(rows): raise ValueError('short source help entity table')
    result = []
    for slot in sorted(subjects):
        row = rows[slot - first]
        if (row.abi, row.size, row.entity_index, row.active) != (2, 52, slot, 1):
            raise ValueError('invalid source help entity pose')
        result.append({'entity': slot, 'x': row.x, 'y': row.y})
    return result


class MinimapState(c.Structure):
    _fields_ = [('abi_version', c.c_uint32), ('struct_size', c.c_uint32), ('generation', c.c_uint64), ('tick', c.c_uint64),
                ('local_player_entity_index', c.c_uint16), ('reserved_0', c.c_uint16), ('entity_count', c.c_uint32),
                ('mark_count', c.c_uint32), ('entity_hash', c.c_uint64), ('mark_hash', c.c_uint64),
                ('visibility_flags', c.c_uint32), ('reserved', c.c_uint32 * 3)]


class MinimapEntity(c.Structure):
    _fields_ = [('abi_version', c.c_uint32), ('struct_size', c.c_uint32), ('generation', c.c_uint64), ('tick', c.c_uint64),
                ('source_order', c.c_uint16), ('subject_entity_index', c.c_uint16), ('x', c.c_int16), ('y', c.c_int16),
                ('selector', c.c_uint16), ('palette_index', c.c_uint8), ('kind', c.c_uint8), ('size', c.c_uint8),
                ('anchor', c.c_uint8), ('help_selector', c.c_uint8), ('visible', c.c_uint8), ('reserved', c.c_uint32 * 3)]


class MinimapMark(c.Structure):
    _fields_ = [('abi_version', c.c_uint32), ('struct_size', c.c_uint32), ('generation', c.c_uint64), ('tick', c.c_uint64),
                ('source_order', c.c_uint16), ('subject_entity_index', c.c_uint16), ('x', c.c_int16), ('y', c.c_int16),
                ('palette_index', c.c_uint8), ('kind', c.c_uint8), ('help_selector', c.c_uint8), ('visible', c.c_uint8),
                ('reserved', c.c_uint32 * 3)]


def _read_map_labels(lib, handle, generation: int, tick: int, visibility_flags: int,
                     *, full_map: bool) -> list[dict]:
    # Both projections produce labels only when the same live Beyond Sight
    # bit is set. The map state already validates this generation and tick.
    if visibility_flags & 0x08 == 0:
        return []
    fn = (lib.mc_native_campaign_world_map_labels_json if full_map else
          lib.mc_native_campaign_minimap_labels_json)
    fn.argtypes = [c.c_uint64, c.c_uint64, c.c_uint64, c.POINTER(c.c_uint8),
                   c.c_uint32, c.POINTER(c.c_uint32)]
    fn.restype = c.c_int32
    required = c.c_uint32()
    status = fn(handle, generation, tick, None, 0, c.byref(required))
    if status != 10 or not 0 < required.value <= 8192:
        raise RuntimeError(f'native map labels sizing rejected: status {status}, bytes {required.value}')
    data = (c.c_uint8 * required.value)()
    written = c.c_uint32()
    status = fn(handle, generation, tick, data, len(data), c.byref(written))
    if status != 0 or written.value != len(data):
        raise RuntimeError(f'native map labels copy rejected or incomplete: status {status}')
    snapshot = json.loads(bytes(data).decode('utf-8'))
    if (not isinstance(snapshot, dict) or snapshot.get('schema') != 'mc2-campaign-minimap-labels/v1' or
            snapshot.get('generation') != generation or snapshot.get('tick') != tick or
            not isinstance(snapshot.get('labels'), list) or len(snapshot['labels']) > 8):
        raise RuntimeError('native map labels are stale or invalid')
    previous = -1
    for row in snapshot['labels']:
        if not isinstance(row, dict):
            raise RuntimeError('native map label is invalid')
        for key, lower, upper in (('source_order', 0, 65535), ('subject_entity_index', 0, 65535),
                                  ('x', -32768, 32767), ('y', -32768, 32767), ('palette_index', 0, 255)):
            if type(row.get(key)) is not int or not lower <= row[key] <= upper:
                raise RuntimeError(f'native map label {key} is invalid')
        text = row.get('text')
        if (row['source_order'] <= previous or not isinstance(text, str) or not 0 < len(text) <= 255 or
                any(not 0 < ord(value) <= 255 for value in text)):
            raise RuntimeError('native map label ordering or text is invalid')
        previous = row['source_order']
    return snapshot['labels']


def read_minimap(lib, handle, generation: int, tick: int) -> dict:
    """Fetch native minimap commands and terrain admission on the owner thread."""
    row = MinimapState()
    state = lib.mc_native_campaign_minimap_state
    state.argtypes = [c.c_uint64, c.c_uint64, c.c_uint64, c.POINTER(MinimapState), c.c_uint32]
    state.restype = c.c_int32
    status = state(handle, generation, tick, c.byref(row), c.sizeof(row))
    if status != 0 or (row.abi_version, row.struct_size, row.generation, row.tick) != (2, c.sizeof(row), generation, tick):
        raise RuntimeError(f'native minimap state rejected or stale: status {status}')
    def rows(symbol, typ, count):
        if count > 65536:
            raise ValueError('native minimap exceeds bounded entity capacity')
        if not count:
            return []
        fn = getattr(lib, symbol)
        fn.argtypes = [c.c_uint64, c.c_uint64, c.c_uint64, c.c_uint32, c.POINTER(typ), c.c_uint32, c.c_uint32, c.POINTER(c.c_uint32)]
        fn.restype = c.c_int32
        items, written = (typ * count)(), c.c_uint32()
        status = fn(handle, generation, tick, 0, items, count, c.sizeof(typ), c.byref(written))
        if status or written.value != count:
            raise RuntimeError(f'native minimap rows rejected or incomplete: status {status}')
        result = []
        for item in items:
            if (item.abi_version, item.struct_size, item.generation, item.tick) != (2, c.sizeof(typ), generation, tick):
                raise RuntimeError('native minimap row is stale')
            result.append({name: int(getattr(item, name)) for name, _ in typ._fields_ if not name.startswith('reserved')})
        return result
    coverage_fn = lib.mc_native_campaign_minimap_terrain_coverage_copy
    coverage_fn.argtypes = [c.c_uint64, c.c_uint64, c.c_uint64, c.POINTER(c.c_uint8), c.c_uint32, c.POINTER(c.c_uint32)]
    coverage_fn.restype = c.c_int32
    coverage, written = (c.c_uint8 * 4096)(), c.c_uint32()
    status = coverage_fn(handle, generation, tick, coverage, len(coverage), c.byref(written))
    if status or written.value != 4096:
        raise RuntimeError(f'native minimap coverage rejected or incomplete: status {status}')
    return {'entities': rows('mc_native_campaign_minimap_entities_copy', MinimapEntity, row.entity_count),
            'marks': rows('mc_native_campaign_minimap_marks_copy', MinimapMark, row.mark_count),
            'coverage': bytes(coverage), 'visibility_flags': row.visibility_flags,
            'labels': _read_map_labels(lib, handle, generation, tick, row.visibility_flags, full_map=False)}

class FrameQuery(c.Structure):
    _fields_ = [('abi_version', c.c_uint32), ('struct_size', c.c_uint32), ('generation', c.c_uint64),
                ('tick', c.c_uint64), ('presentation_revision', c.c_uint64),
                ('viewport_width', c.c_uint16), ('viewport_height', c.c_uint16), ('screen_stride', c.c_uint16),
                ('reserved_0', c.c_uint16), ('configured_blur_mode', c.c_uint8), ('blur_capability', c.c_uint8),
                ('high_speed_system', c.c_uint8), ('scratch_present', c.c_uint8), ('vga_resolution', c.c_uint8),
                ('spatial_smoothing', c.c_uint8), ('minimap_enabled', c.c_uint8), ('top_bar_enabled', c.c_uint8),
                ('menu_overlay', c.c_uint8), ('game_paused', c.c_uint8), ('level_end', c.c_uint8),
                ('objective_selector', c.c_uint8), ('objective_textbox', c.c_uint8), ('tutorial_or_briefing', c.c_uint8),
                ('help_popup', c.c_uint8), ('debug_text', c.c_uint8), ('reserved', c.c_uint32 * 2)]


class FrameInfo(c.Structure):
    _fields_ = [('abi_version', c.c_uint32), ('struct_size', c.c_uint32), ('generation', c.c_uint64), ('tick', c.c_uint64),
                ('presentation_revision', c.c_uint64), ('immutable_input_hash', c.c_uint64),
                ('width', c.c_uint16), ('height', c.c_uint16), ('stride', c.c_uint16), ('viewport_width', c.c_uint16),
                ('index_bytes', c.c_uint32), ('provenance_bytes', c.c_uint32), ('flags', c.c_uint32),
                ('effective_postprocess_mode', c.c_uint8), ('history_state', c.c_uint8), ('newly_committed', c.c_uint8),
                ('reserved_0', c.c_uint8), ('index_hash', c.c_uint64), ('provenance_hash', c.c_uint64),
                ('lighting_hash', c.c_uint64), ('source_arena_hash', c.c_uint64), ('overlay_pixels_written', c.c_uint32),
                ('overlay_stage_count', c.c_uint16), ('reserved_1', c.c_uint16), ('reserved', c.c_uint32 * 6)]


if c.sizeof(FrameQuery) != 64 or c.sizeof(FrameInfo) != 128:
    raise RuntimeError('unsupported native presentation frame ABI layout')


def read_minimap_underlay(lib, handle, generation: int, tick: int) -> bytes:
    """Return the 64×64 source minimap from the bound compatibility world frame."""
    query = FrameQuery(abi_version=2, struct_size=64, generation=generation, tick=tick,
                       presentation_revision=tick, viewport_width=320, viewport_height=200,
                       screen_stride=320, vga_resolution=1)
    info, pixels = FrameInfo(), (c.c_uint8 * 64000)()
    fn = lib.mc_native_campaign_presentation_frame_copy
    fn.argtypes = [c.c_uint64, c.POINTER(FrameQuery), c.POINTER(c.c_uint8), c.c_uint32,
                   c.c_void_p, c.c_uint32, c.POINTER(FrameInfo), c.c_uint32]
    fn.restype = c.c_int32
    status = fn(handle, c.byref(query), pixels, len(pixels), None, 0, c.byref(info), c.sizeof(info))
    if status or (info.abi_version, info.struct_size, info.generation, info.tick, info.width, info.height,
                  info.stride, info.index_bytes) != (2, 128, generation, tick, 320, 200, 320, 64000):
        raise RuntimeError(f'native minimap underlay rejected or stale: status {status}')
    return b''.join(bytes(pixels[y * 320:y * 320 + 64]) for y in range(64))

class Objective(c.Structure):
    _fields_ = [('abi_version', c.c_uint32), ('struct_size', c.c_uint32), ('generation', c.c_uint64),
                ('objective_slot', c.c_uint8), ('stage_slot', c.c_uint8), ('is_level_end', c.c_uint8),
                ('done', c.c_uint8), ('text_selector', c.c_uint8), ('stage_state', c.c_uint8),
                ('stage_kind', c.c_int8), ('stage_flags', c.c_int8), ('axis_x', c.c_int16), ('axis_y', c.c_int16),
                ('relation_domain', c.c_uint8), ('reserved_0', c.c_uint8), ('relation_value', c.c_uint16),
                ('reserved', c.c_uint32 * 4)]


def read_objective(lib, handle, generation: int, tick: int) -> dict:
    """Read the active source objective stage under the caller's frame lock."""
    del tick  # ABI is generation-bound; owner lock binds it to this frame.
    row = Objective()
    fn = lib.mc_native_campaign_objective_stage_state
    fn.argtypes = [c.c_uint64, c.c_uint64, c.c_uint8, c.c_uint8, c.POINTER(Objective), c.c_uint32]
    fn.restype = c.c_int32
    status = fn(handle, generation, 0, 0, c.byref(row), c.sizeof(row))
    if status or (row.abi_version, row.struct_size, row.generation, row.objective_slot, row.stage_slot) != (2, 48, generation, 0, 0):
        raise RuntimeError(f'native objective rejected or stale: status {status}')
    return {name: int(getattr(row, name)) for name, _ in row._fields_ if not name.startswith('reserved')}


def read_world_map(lib, handle, generation: int, tick: int) -> tuple[bytes, bytes, dict]:
    """Copy the bound original split-map image and source-ordered map commands."""
    indices, palette = (c.c_uint8 * 64000)(), (c.c_uint8 * 768)()
    entities, marks = (MinimapEntity * 1000)(), (MinimapMark * 64)()
    state = MinimapState()
    fn = lib.mc_native_campaign_world_map_copy
    fn.argtypes = [c.c_uint64, c.c_uint64, c.c_uint64, c.POINTER(c.c_uint8), c.c_uint32,
                   c.POINTER(c.c_uint8), c.c_uint32, c.POINTER(MinimapEntity), c.c_uint32,
                   c.POINTER(MinimapMark), c.c_uint32, c.POINTER(MinimapState), c.c_uint32]
    fn.restype = c.c_int32
    status = fn(handle, generation, tick, indices, len(indices), palette, len(palette),
                entities, len(entities), marks, len(marks), c.byref(state), c.sizeof(state))
    if status or (state.abi_version, state.struct_size, state.generation, state.tick) != (2, 72, generation, tick):
        raise RuntimeError(f'native world map rejected or stale: status {status}')
    if state.entity_count > len(entities) or state.mark_count > len(marks):
        raise RuntimeError('native world map exceeds its bounded buffers')
    def convert(rows, count):
        result = []
        for row in rows[:count]:
            if (row.abi_version, row.struct_size, row.generation, row.tick) != (2, c.sizeof(row), generation, tick):
                raise RuntimeError('native world map row is stale')
            result.append({name: int(getattr(row, name)) for name, _ in row._fields_ if not name.startswith('reserved')})
        return result
    return bytes(indices), bytes(palette), {
        'entities': convert(entities, state.entity_count), 'marks': convert(marks, state.mark_count),
        'visibility_flags': state.visibility_flags,
        'labels': _read_map_labels(lib, handle, generation, tick, state.visibility_flags, full_map=True),
    }
