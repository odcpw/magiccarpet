"""Emit real public C ABI MCPO packets and diagnostic projections at live ticks.

Run only against an explicitly supplied current library. The default scenario
references local, ignored owner packages; override it via MC_RENDER_SCENARIO
when reproducing elsewhere. No files are written.
"""
import argparse
import base64
import ctypes as c
import json
import os
import sys
from pathlib import Path

PRIVATE = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(PRIVATE / 'remc2-unity-native/qualification'))
from verify_voxel_mana_e2e import Runtime

MC_STATUS_BUFFER_TOO_SMALL = 11  # mc_sim.h / shared_types.rs


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--library', type=Path, required=True)
    parser.add_argument('--scenario', type=Path, default=Path(os.environ.get('MC_RENDER_SCENARIO', PRIVATE / 'generated/mc-voxel-combat-20261107/native-mana-scenario-01.json')))
    args = parser.parse_args()
    runtime = Runtime(args.library.resolve(), json.loads(args.scenario.read_text())['packages'])
    try:
        ptr, u32, u64 = c.c_void_p, c.c_uint32, c.c_uint64
        runtime._bind('mc_game_runtime_bootstrap_package_set_json_v1', [u64, u64, ptr, u32])
        runtime._bind('mc_game_runtime_open_level_v1', [u64, u64])
        runtime.check(runtime.lib.mc_game_runtime_bootstrap_package_set_json_v1(
            runtime.handle.value, 1, runtime.request, len(runtime.request)), 'bootstrap')
        runtime.check(runtime.lib.mc_game_runtime_open_level_v1(runtime.handle.value, 2), 'open level')
        for name in ('mc_game_runtime_identity_json_v1', 'mc_game_runtime_render_observation_json_v1'):
            runtime._bind(name, [u64, ptr, u32, ptr])
        runtime._bind('mc_game_runtime_render_observation_copy_v1', [u64, u64, u64, u64, ptr, u32, ptr])
        lib, handle = runtime.lib, runtime.handle.value
        def json_read(name):
            fn = getattr(lib, name)
            return runtime.client._read_json(lambda b, cap, n: fn(handle, b, cap, n))
        result = []
        for turn in range(2):
            diagnostic = json_read('mc_game_runtime_render_observation_json_v1')
            identity = json_read('mc_game_runtime_identity_json_v1')
            generation, tick = int(diagnostic['identity']['generation']), int(diagnostic['identity']['tick'])
            runtime_generation = int(identity['generation'])
            size = u32()
            status = lib.mc_game_runtime_render_observation_copy_v1(handle, runtime_generation, generation, tick, None, 0, c.byref(size))
            if status != MC_STATUS_BUFFER_TOO_SMALL or not 0 < size.value <= 16 * 1024 * 1024:
                raise RuntimeError(f'binary sizing failed: status={status}, size={size.value}')
            buf = (c.c_uint8 * size.value)()
            runtime.check(lib.mc_game_runtime_render_observation_copy_v1(handle, runtime_generation, generation, tick, buf, len(buf), c.byref(size)), 'binary copy')
            result.append({'binary': base64.b64encode(bytes(buf[:size.value])).decode('ascii'),
                           'diagnostic': diagnostic, 'runtime': identity})
            if not turn:
                runtime.step(generation, tick, {'longitudinal': 'coast', 'lateral': 'neutral', 'roll_axis': 0, 'pitch_axis': 0,
                    'cast_left': False, 'cast_right': False, 'cast_selected': False, 'elapsed_centiseconds': 3,
                    'select_spell': None, 'look_yaw': 0, 'look_pitch': 0})
        print(json.dumps(result, separators=(',', ':')))
    finally:
        runtime.close()

if __name__ == '__main__':
    main()
