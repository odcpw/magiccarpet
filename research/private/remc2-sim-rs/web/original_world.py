"""Read generation-bound native terrain and source-selected 3D presentation rows."""
from __future__ import annotations

import ctypes as c
import gzip
import struct


class TerrainState(c.Structure):
    _fields_ = [('abi_version', c.c_uint32), ('struct_size', c.c_uint32),
                ('generation', c.c_uint64), ('tick', c.c_uint64),
                ('mutation_sequence', c.c_uint64), ('terrain_hash', c.c_uint64),
                ('width', c.c_uint16), ('height', c.c_uint16), ('plane_count', c.c_uint8),
                ('hash_schema', c.c_uint8), ('authority', c.c_uint8), ('reserved_0', c.c_uint8),
                ('reserved', c.c_uint32 * 4)]


class EntityPresentation(c.Structure):
    _fields_ = [('abi_version', c.c_uint32), ('struct_size', c.c_uint32),
                ('generation', c.c_uint64), ('tick', c.c_uint64),
                ('entity_index', c.c_uint16), ('source_id', c.c_uint16),
                ('render_parameter_index', c.c_uint16), ('base_tmap', c.c_uint16),
                ('selected_tmap', c.c_uint16), ('logical_height', c.c_uint16),
                ('render_mode', c.c_uint8), ('orientation_sector', c.c_uint8),
                ('mirrored', c.c_uint8), ('reserved_0', c.c_uint8),
                ('x', c.c_uint16), ('y', c.c_uint16), ('z', c.c_int16), ('yaw', c.c_int16),
                ('pixel_mode', c.c_uint8), ('owner_color', c.c_uint8), ('brightness', c.c_uint16),
                ('draw_order', c.c_uint32), ('control_flags', c.c_uint32),
                ('primary_depth_adjustment', c.c_int16), ('reserved', c.c_uint16)]


if c.sizeof(TerrainState) != 64 or c.sizeof(EntityPresentation) != 64:
    raise RuntimeError('unsupported native world ABI layout')


class OriginalWorld:
    PLANE_NAMES = ('types', 'height', 'shade', 'angle', 'secondary')

    def __init__(self, native):
        self.native = native
        self.cache_key = None
        self.terrain = None
        self.terrain_gzip = None
        self.revision = 0
        lib = native.lib
        for name, args in {
            'mc_native_campaign_terrain_state': [c.c_uint64, c.c_uint64, c.c_void_p, c.c_uint32],
            'mc_native_campaign_terrain_plane_length': [c.c_uint64, c.c_uint64, c.c_uint32, c.c_void_p],
            'mc_native_campaign_terrain_plane_copy_u8': [c.c_uint64, c.c_uint64, c.c_uint32, c.c_void_p, c.c_uint32, c.c_void_p],
            'mc_native_campaign_world_entity_presentations_copy': [c.c_uint64, c.c_uint64, c.c_uint64, c.c_int16, c.c_void_p, c.c_uint32, c.c_uint32, c.c_void_p],
        }.items():
            fn = getattr(lib, name); fn.argtypes = args; fn.restype = c.c_int32
        self.rows = (EntityPresentation * 65536)()

    def read(self, generation, tick, yaw, epoch):
        native = self.native
        state = TerrainState()
        native.check(native.lib.mc_native_campaign_terrain_state(native.handle, generation, c.byref(state), c.sizeof(state)), 'terrain state')
        if (state.abi_version, state.struct_size, state.generation, state.tick, state.width, state.height) != (2, 64, generation, tick, 256, 256):
            raise RuntimeError('stale or unsupported native terrain state')
        # A replay may create a new runtime whose native generation/hash repeats.
        key = (epoch, generation, state.mutation_sequence, state.terrain_hash)
        if key != self.cache_key:
            planes = []
            for kind in range(len(self.PLANE_NAMES)):
                length = c.c_uint32()
                native.check(native.lib.mc_native_campaign_terrain_plane_length(native.handle, generation, kind, c.byref(length)), 'terrain plane length')
                if length.value != 65536: raise RuntimeError('invalid native terrain plane extent')
                data, written = (c.c_uint8 * length.value)(), c.c_uint32()
                native.check(native.lib.mc_native_campaign_terrain_plane_copy_u8(native.handle, generation, kind, data, len(data), c.byref(written)), 'terrain plane copy')
                if written.value != length.value: raise RuntimeError('short native terrain plane')
                planes.append(bytes(data))
            self.terrain = b''.join(planes)
            self.terrain_gzip = gzip.compress(self.terrain, compresslevel=1, mtime=0)
            self.cache_key = key
            self.revision += 1
        written = c.c_uint32()
        native.check(native.lib.mc_native_campaign_world_entity_presentations_copy(native.handle, generation, tick, yaw, self.rows, len(self.rows), c.sizeof(EntityPresentation), c.byref(written)), 'world entity presentations')
        if written.value > len(self.rows): raise RuntimeError('invalid native entity count')
        for row in self.rows[:written.value]:
            if (row.abi_version, row.struct_size, row.generation, row.tick) != (2, 64, generation, tick):
                raise RuntimeError('stale native entity presentation')
        return self.revision, self.terrain, c.string_at(c.addressof(self.rows), written.value * c.sizeof(EntityPresentation)), written.value


class CampaignEntity(c.Structure):
    _fields_ = [('abi', c.c_uint32), ('size', c.c_uint32),
                ('entity_index', c.c_uint16), ('source_id', c.c_uint16),
                ('next_relation', c.c_uint16), ('parameter_relation', c.c_uint16),
                ('authority', c.c_uint8), ('active', c.c_uint8), ('class_id', c.c_uint8),
                ('model', c.c_uint8), ('action', c.c_uint8), ('phase', c.c_uint8),
                ('x', c.c_uint16), ('y', c.c_uint16), ('z', c.c_int16),
                ('yaw', c.c_int16), ('pitch', c.c_int16), ('fov', c.c_int16),
                ('speed', c.c_int16), ('reserved', c.c_uint32 * 4)]


class CharacterBase(c.Structure):
    _fields_ = [('abi', c.c_uint32), ('size', c.c_uint32), ('generation', c.c_uint64), ('tick', c.c_uint64),
                ('slot_generation', c.c_uint32), ('parent_generation', c.c_uint32),
                ('entity_index', c.c_uint16), ('source_id', c.c_uint16), ('parent_index', c.c_uint16), ('parent_id', c.c_uint16),
                ('class_id', c.c_uint8), ('model', c.c_uint8), ('action', c.c_uint8), ('phase', c.c_uint8),
                ('active', c.c_uint8), ('has_target', c.c_uint8), ('has_parent', c.c_uint8), ('stance', c.c_uint8),
                ('x', c.c_uint16), ('y', c.c_uint16), ('z', c.c_int16), ('yaw', c.c_int16),
                ('pitch', c.c_int16), ('speed', c.c_int16), ('life', c.c_int32),
                ('target_index', c.c_uint16), ('target_x', c.c_uint16), ('target_y', c.c_uint16), ('target_z', c.c_int16),
                ('control_flags', c.c_uint32), ('age_ticks', c.c_uint16), ('render_parameter', c.c_uint16)]


class Character(c.Structure):
    _fields_ = [('base', CharacterBase), ('roll', c.c_int16), ('source_half_width', c.c_uint16),
                ('render_height', c.c_uint16), ('previous_index', c.c_uint16), ('next_index', c.c_uint16),
                ('part_index', c.c_uint16), ('lifecycle_flags', c.c_uint32)]


class Building(c.Structure):
    _fields_ = [('abi', c.c_uint32), ('size', c.c_uint32), ('generation', c.c_uint64), ('tick', c.c_uint64),
                ('slot_generation', c.c_uint32), ('source_slot', c.c_uint16), ('source_id', c.c_uint16),
                ('building_index', c.c_uint8), ('action', c.c_uint8), ('active', c.c_uint8), ('drawing', c.c_uint8),
                ('x', c.c_uint16), ('y', c.c_uint16), ('z', c.c_int16), ('reserved', c.c_uint16 * 3)]


class CastleChild(c.Structure):
    _fields_ = [('abi', c.c_uint32), ('size', c.c_uint32), ('generation', c.c_uint64), ('tick', c.c_uint64),
                ('slot_generation', c.c_uint32), ('castle_generation', c.c_uint32), ('target_generation', c.c_uint32),
                ('entity_index', c.c_uint16), ('source_id', c.c_uint16), ('owner_entity_index', c.c_uint16),
                ('player_entity_index', c.c_uint16), ('castle_index', c.c_uint16), ('target_index', c.c_uint16),
                ('x', c.c_uint16), ('y', c.c_uint16), ('z', c.c_int16), ('yaw', c.c_int16), ('speed', c.c_int16),
                ('render_parameter', c.c_uint16), ('render_height', c.c_uint16), ('reserved', c.c_uint16),
                ('life', c.c_int32), ('max_life', c.c_int32), ('mana', c.c_int32), ('max_mana', c.c_int32),
                ('control_flags', c.c_uint32), ('class_id', c.c_uint8), ('model', c.c_uint8),
                ('action', c.c_uint8), ('phase', c.c_uint8), ('active', c.c_uint8), ('drawing', c.c_uint8),
                ('owner_player_color', c.c_uint8), ('relation_flags', c.c_uint8)]


class AuthoredTree(c.Structure):
    _fields_ = [('abi', c.c_uint32), ('size', c.c_uint32), ('generation', c.c_uint64), ('tick', c.c_uint64),
                ('slot_generation', c.c_uint32), ('entity_index', c.c_uint16), ('source_id', c.c_uint16),
                ('authored_index', c.c_uint16), ('render_parameter', c.c_uint16), ('render_height', c.c_uint16),
                ('x', c.c_uint16), ('y', c.c_uint16), ('z', c.c_int16), ('life', c.c_int32),
                ('control_flags', c.c_uint32), ('class_id', c.c_uint8), ('model', c.c_uint8),
                ('action', c.c_uint8), ('drawing', c.c_uint8)]


MODERN_HEADER = struct.Struct('<4sHHQQQQQIIIIIIIIII')
if tuple(map(c.sizeof, (CampaignEntity, CharacterBase, Character, Building, CastleChild, AuthoredTree))) != (52, 80, 96, 48, 96, 56):
    raise RuntimeError('unsupported modern native ABI layout')


class ModernWorld:
    """Copy all native projections on the owner thread at one immutable tick."""
    LIMIT = 65536

    def __init__(self, native):
        self.native = native
        u64, u32, ptr = c.c_uint64, c.c_uint32, c.c_void_p
        signatures = {
            'mc_native_campaign_entity_count': [u64, ptr],
            'mc_native_campaign_entities_copy': [u64, u32, ptr, u32, u32, ptr],
            'mc_native_campaign_character_states_copy_v2': [u64, u64, u64, ptr, u32, u32, ptr],
            'mc_game_runtime_buildings_copy_v1': [u64, u64, u64, ptr, u32, u32, ptr],
            'mc_native_campaign_castle_children_copy_v1': [u64, u64, u64, ptr, u32, u32, ptr],
            'mc_native_campaign_authored_trees_copy_v1': [u64, u64, u64, ptr, u32, u32, ptr],
        }
        for name, args in signatures.items():
            fn = getattr(native.lib, name); fn.argtypes = args; fn.restype = c.c_int32
        self.buffers = [(kind * self.LIMIT)() for kind in (CampaignEntity, Character, Building, CastleChild, AuthoredTree)]

    def read(self, outer, generation, tick, sequence, epoch):
        native = self.native
        count = c.c_uint32()
        native.check(native.lib.mc_native_campaign_entity_count(native.handle, c.byref(count)), 'modern entity count')
        if count.value > self.LIMIT: raise RuntimeError('modern entity count exceeds limit')
        written = c.c_uint32()
        native.check(native.lib.mc_native_campaign_entities_copy(native.handle, 0, self.buffers[0], count.value,
                     c.sizeof(CampaignEntity), c.byref(written)), 'modern entities')
        if written.value != count.value: raise RuntimeError('short modern entity copy')
        counts = [written.value]
        for name, buffer, kind in zip(('mc_native_campaign_character_states_copy_v2', 'mc_game_runtime_buildings_copy_v1',
                                       'mc_native_campaign_castle_children_copy_v1', 'mc_native_campaign_authored_trees_copy_v1'),
                                      self.buffers[1:], (Character, Building, CastleChild, AuthoredTree)):
            written = c.c_uint32()
            native.check(getattr(native.lib, name)(native.handle, generation, tick, buffer, self.LIMIT,
                         c.sizeof(kind), c.byref(written)), name)
            if written.value > self.LIMIT: raise RuntimeError('modern observation count exceeds limit')
            counts.append(written.value)
        for i, (buffer, count, kind) in enumerate(zip(self.buffers, counts, (CampaignEntity, Character, Building, CastleChild, AuthoredTree))):
            seen = set()
            for row in buffer[:count]:
                base = row.base if i == 1 else row
                if (base.abi, base.size) != (2, c.sizeof(kind)): raise RuntimeError('modern ABI mismatch')
                if i and (base.generation, base.tick) != (generation, tick): raise RuntimeError('stale modern observation')
                key = base.source_slot if i == 2 else base.entity_index
                if key in seen: raise RuntimeError('duplicate modern observation slot')
                seen.add(key)
        sizes = (52, 96, 48, 96, 56)
        # The C Castle-child struct ends at byte 92 and has four alignment
        # bytes. Native struct copies need not initialize padding; never expose it.
        for index in range(counts[3]):
            c.memset(c.addressof(self.buffers[3]) + index * 96 + 92, 0, 4)
        header = MODERN_HEADER.pack(b'MCD1', 1, MODERN_HEADER.size, outer, generation, tick, sequence, epoch,
                                    *(value for pair in zip(counts, sizes) for value in pair))
        return header + b''.join(c.string_at(c.addressof(buffer), count * size)
                                  for buffer, count, size in zip(self.buffers, counts, sizes))
