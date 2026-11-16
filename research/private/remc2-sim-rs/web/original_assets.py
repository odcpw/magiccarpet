"""Allowlisted local owner assets and campaign HUD observations for Original browser.

No owner data is copied into source control. Construct once on the native owner
thread, and serve only ``manifest`` and ``blob`` through the loopback bridge.
"""
from __future__ import annotations

import ctypes as c
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2] / 'generated' / 'settlement-20261105-integration' / 'owner-closed-v6'


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


class OriginalAssets:
    """Bounded source sprite bundle; HTTP callers never supply filesystem paths."""
    def __init__(self, root: Path = ROOT):
        night = root / 'night-presentation-v6'
        environment = root / 'environment-complete' / 'cache-level1'
        source = json.loads((night / 'hud/level1.json').read_text())
        blob = bytearray()
        def add(data: bytes):
            offset = len(blob)
            blob.extend(data)
            return {'offset': offset, 'length': len(data)}
        tables = (environment / 'palette/tables.u8').read_bytes()
        if len(tables) != 83456:
            raise ValueError('invalid Night palette tables')
        normal = add(tables[0x4000:0x14000])
        aperture = add(tables[0x14300:0x14400])
        # Night uses TABLESN for history, including exit; GTD2 exists only for Day.
        rows = []
        for row in source['bitmap_assets']:
            width, height = row['stored_width'], row['stored_height']
            indices, mask = (night / row['indices_path']).read_bytes(), (night / row['mask_path']).read_bytes()
            if len(indices) != width * height or len(mask) != len(indices):
                raise ValueError(f"invalid source sprite {row['selector']}")
            rows.append({'selector': row['selector'], 'width': width, 'height': height,
                         'runtime_width': row['runtime_width'], 'runtime_height': row['runtime_height'],
                         'indices': add(indices), 'mask': add(mask)})
        objectives = []
        for row in source['objective']['rows']:
            data = (night / row['raster_path']).read_bytes()
            plane = row['raster_plane_bytes']
            if len(data) != plane * 2 or plane != row['render_width'] * row['render_height']:
                raise ValueError('invalid source objective raster')
            objectives.append({key: row[key] for key in ('selector', 'reference_x', 'reference_y',
                              'render_width', 'render_height', 'blend_x', 'blend_y', 'blend_width', 'blend_height')}
                              | {'indices': add(data[:plane]), 'mask': add(data[plane:])})
        tiers = []
        for row in source['spells']['tier_rows']:
            data = (night / row['render_path']).read_bytes()
            if len(data) != row['render_width'] * row['render_height'] * 4:
                raise ValueError('invalid source tier raster')
            tiers.append({'tier': row['tier'], 'width': row['render_width'], 'height': row['render_height'],
                          'color': row['palette_index'], 'rgba': add(data)})
        environment_descriptor = json.loads((environment / 'materials/descriptor.json').read_text())
        bank_name = environment_descriptor['profile']['sprites']
        bank = next(bank for bank in environment_descriptor['bitmap_banks'] if bank['bank'] == bank_name)
        symbols = []
        for row in bank['bitmaps']:
            if row.get('source_empty'):
                continue
            indices, mask = (environment / row['indices_path']).read_bytes(), (environment / row['mask_path']).read_bytes()
            if len(indices) != row['width'] * row['height'] or len(mask) != len(indices):
                raise ValueError(f"invalid minimap symbol {row['selector']}")
            symbols.append({'selector': row['selector'], 'width': row['width'], 'height': row['height'],
                            'indices': add(indices), 'mask': add(mask)})
        self.blob = bytes(blob)
        colors = (environment / 'palette/colors.u8').read_bytes()
        if len(colors) != 4096:
            raise ValueError('invalid Night color table')
        self.manifest = {'schema': 'mc-original-assets/v1', 'bytes': len(blob), 'normal': normal,
                         'objective_background': colors[0], 'map_cross_color': colors[4095],
                         'exit': normal, 'aperture': aperture, 'sprites': rows, 'symbols': symbols,
                         'objectives': objectives, 'tiers': tiers}

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
            'coverage': bytes(coverage), 'visibility_flags': row.visibility_flags}

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
    }
