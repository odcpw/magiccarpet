"""Verify a reimported goat GLB's skin, loop and treadmill stance.

blender -b -t 2 -P verify_goat_rig.py -- --model goat-rigged.glb --output report.json
All coordinates are model units; the imported goat is roughly one unit tall.
The contact check tests this rig's authored 0.095-unit stride and 64% stance,
not the timing or motion of the original game sprites.
"""
import argparse
import json
import sys
from pathlib import Path

import bpy
import numpy as np


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--model', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--start-frame', type=int, default=1)
    parser.add_argument('--ground-baked', action='store_true', help='Check the ground-root, reversed-forward FBX')
    args = parser.parse_args(sys.argv[sys.argv.index('--') + 1:])
    if args.output.exists():
        raise FileExistsError(args.output)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    if args.model.suffix.lower() == '.fbx':
        bpy.ops.import_scene.fbx(filepath=str(args.model.resolve()))
    else:
        bpy.ops.import_scene.gltf(filepath=str(args.model.resolve()))
    scene = bpy.context.scene
    rig = next(o for o in scene.objects if o.type == 'ARMATURE')
    obj = max((o for o in scene.objects if o.type == 'MESH'), key=lambda o: len(o.data.vertices))
    deps = bpy.context.evaluated_depsgraph_get()
    phases = {'hind_L': 0, 'front_L': .25, 'hind_R': .5, 'front_R': .75}
    regions = {}
    for name in ['head'] + [n + '_hoof' for n in phases]:
        group = obj.vertex_groups[name]
        regions[name] = [v.index for v in obj.data.vertices
                         if (name == 'head' or v.co.z < (.03 if args.ground_baked else -.46))
                         and any(g.group == group.index and g.weight > .999 for g in v.groups)]
        if len(regions[name]) < 10:
            raise ValueError(f'{name}: not enough rigid-region vertices')
    positions, lengths = [], []
    for frame in range(args.start_frame, args.start_frame + 25):
        scene.frame_set(frame)
        evaluated = obj.evaluated_get(deps)
        mesh = evaluated.to_mesh()
        positions.append(np.array([v.co[:] for v in mesh.vertices]))
        evaluated.to_mesh_clear()
        lengths.append([bone.length for bone in rig.pose.bones])
    positions = np.array(positions)
    lengths = np.array(lengths)
    if not np.isfinite(positions).all() or not np.isfinite(lengths).all():
        raise ValueError('Non-finite mesh position or bone length')
    rigid = {}
    for name, ids in regions.items():
        sampled = np.linspace(0, len(ids) - 1, 12, dtype=int)
        errors = []
        for a, b in zip(sampled[:-1], sampled[1:]):
            distances = np.linalg.norm(positions[:, ids[a]] - positions[:, ids[b]], axis=1)
            errors.append(float(np.ptp(distances)))
        rigid[name] = max(errors)
    observed = {n: {} for n in phases}
    for frame in range(48):
        t = frame / 24
        p = positions[frame % 24]
        for name, offset in phases.items():
            if (t + offset) % 1 < .64:
                floor_y = (1 if args.ground_baked else -1) * t * .095 / .64
                indices = regions[name + '_hoof']
                sample = (float(p[indices, 1].mean() - floor_y), float(p[indices, 2].min()))
                observed[name].setdefault(int(t + offset), []).append(sample)
    contact = {}
    for name, intervals in observed.items():
        valid = [v for v in intervals.values() if len(v) >= 10]
        if not valid:
            raise ValueError(f'{name}: no complete sampled stance')
        drifts = [np.ptp(np.asarray(v)[:, 0]) for v in valid]
        bottoms = [point[1] for interval in valid for point in interval]
        contact[name] = {'samples': sum(map(len, valid)),
                         'max_fore_aft_drift': float(max(drifts)),
                         'bottom_range': [float(min(bottoms)), float(max(bottoms))]}
    weights = [sum(g.weight for g in v.groups) for v in obj.data.vertices]
    if not np.isfinite(weights).all():
        raise ValueError('Non-finite vertex weight sum')
    report = {'coordinate_units': 'model units', 'vertices': len(obj.data.vertices),
              'bones': len(rig.data.bones), 'actions': len(bpy.data.actions),
              'loop_max_vertex_error': float(np.linalg.norm(positions[0] - positions[-1], axis=1).max()),
              'max_bone_length_variation': float(np.ptp(lengths, axis=0).max()),
              'rigid_region_sample_pair_variation': rigid,
              'weight_sum_range': [float(min(weights)), float(max(weights))],
              'stance_contact': contact}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))
    if (report['loop_max_vertex_error'] > 1e-5 or report['max_bone_length_variation'] > 1e-5
            or max(rigid.values()) > 1e-5 or min(weights) < .9999 or max(weights) > 1.0001
            or max(v['max_fore_aft_drift'] for v in contact.values()) > 1e-4):
        raise ValueError('Exported goat failed a loop, skin or stance check')


if __name__ == '__main__':
    main()
