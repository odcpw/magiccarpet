"""Check the selected archer's exported one-second walk and rigid quiver.

Run in Blender with --python-exit-code 1, then -- --model walk.glb --output report.json.
This checks sampled flat-ground contact and three quiver probes, not continuous
collision clearance or arbitrary characters. Coordinates are model units.
"""
import argparse
import json
import sys
from pathlib import Path

import bpy
import numpy as np
from mathutils import Vector, kdtree


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1:])
    if args.output.exists():
        raise FileExistsError(args.output)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(args.model.resolve()))
    scene = bpy.context.scene
    obj = max((o for o in scene.objects if o.type == "MESH"),
              key=lambda o: len(o.data.vertices))
    tree = kdtree.KDTree(len(obj.data.vertices))
    for vertex in obj.data.vertices:
        tree.insert(obj.matrix_world @ vertex.co, vertex.index)
    tree.balance()
    probes = []
    for point in [(-.120, .191, 1.188), (-.215, .168, 1.438), (-.207, .119, 1.609)]:
        _, index, error = tree.find(Vector(point))
        if error > .035:
            raise ValueError("Selected character's quiver probe is absent")
        probes.append(index)
    weights = np.array([sum(g.weight for g in v.groups) for v in obj.data.vertices])
    if not np.isfinite(weights).all():
        raise ValueError("Non-finite skin weights")
    deps = bpy.context.evaluated_depsgraph_get()
    first = None
    movement = 0.0
    floors, distances = [], []
    for sample in range(97):
        frame = 1 + sample / 4
        scene.frame_set(int(frame), subframe=frame % 1)
        evaluated_obj = obj.evaluated_get(deps)
        evaluated = evaluated_obj.to_mesh()
        positions = np.array([evaluated_obj.matrix_world @ v.co for v in evaluated.vertices])
        evaluated_obj.to_mesh_clear()
        if not np.isfinite(positions).all():
            raise ValueError(f"Non-finite geometry at frame {frame}")
        if first is None:
            first = positions.copy()
        movement = max(movement, float(np.linalg.norm(positions - first, axis=1).max()))
        floors.append(float(positions[:, 2].min()))
        p = positions[probes]
        distances.append([np.linalg.norm(p[0] - p[1]), np.linalg.norm(p[1] - p[2]),
                          np.linalg.norm(p[0] - p[2])])
    seam = float(np.linalg.norm(first - positions, axis=1).max())
    rigidity = float(np.ptp(np.asarray(distances), axis=0).max())
    failures = []
    if seam >= 1e-5:
        failures.append("loop seam")
    if movement < .001:
        failures.append("missing character motion")
    ranges = [tuple(action.frame_range) for action in bpy.data.actions]
    if not ranges or abs(min(r[0] for r in ranges) - 1) > .001 or abs(max(r[1] for r in ranges) - 25) > .001:
        failures.append("one-second exported animation range")
    if min(floors) <= -.001 or max(floors) >= .003:
        failures.append("ground contact")
    if rigidity >= 1e-5:
        failures.append("quiver probe rigidity")
    if weights.min() < .9999 or weights.max() > 1.0001:
        failures.append("skin weight normalization")
    report = {"version": 1, "model": str(args.model), "coordinate_units": "model units",
              "status": "failed" if failures else "passed", "failures": failures,
              "vertices": len(obj.data.vertices), "quarter_frame_samples": len(floors),
              "sole_minimum": min(floors), "sole_maximum": max(floors),
              "loop_max_vertex_error": seam, "maximum_vertex_motion": movement,
              "animation_frame_ranges": ranges, "quiver_probe_distance_variation": rigidity,
              "weight_sum_range": [float(weights.min()), float(weights.max())]}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report))
    if failures:
        raise ValueError(", ".join(failures))


if __name__ == "__main__":
    main()
