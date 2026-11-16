"""Read generation-bound native terrain and source-selected 3D presentation rows."""
from __future__ import annotations

import ctypes as c
import gzip


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
