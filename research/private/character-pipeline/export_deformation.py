"""Export ActionMesh deformations on an existing textured GLB using Blender.

Preserves the imported mesh's UV seams and materials, matches its bind vertices
geometrically, and verifies the first frame before creating linear shape keys.
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
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--vertices", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--fps", type=int, default=8)
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1:])
    if args.output.exists():
        raise FileExistsError(args.output)
    values = np.load(args.vertices)
    if values.ndim != 3 or values.shape[2] != 3 or not np.isfinite(values).all():
        raise ValueError("Expected finite [time, vertex, xyz] positions")
    # ActionMesh save_deformation writes (-GLB.z, GLB.x, GLB.y).
    # Blender's glTF importer uses (GLB.x, -GLB.z, GLB.y).
    values = values[:, :, [1, 0, 2]]
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(args.model.resolve()))
    objects = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    if len(objects) != 1:
        raise ValueError("This exporter requires one textured mesh")
    obj = objects[0]
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    tree = kdtree.KDTree(len(values[0]))
    for i, point in enumerate(values[0]):
        tree.insert(Vector(point), i)
    tree.balance()
    matches = [tree.find(v.co) for v in obj.data.vertices]
    indices = np.array([m[1] for m in matches])
    error = max(m[2] for m in matches)
    scale = np.ptp(values[0], axis=0).max()
    if error > scale * 1e-4:
        raise ValueError(f"Bind mismatch: {error} at model scale {scale}")
    obj.shape_key_add(name="Basis")
    # One source sample every 3 Blender frames gives 24 fps output from 8 Hz data.
    scene = bpy.context.scene
    scene.render.fps = 24
    step = 24 / args.fps
    for index, positions in enumerate(values):
        key = obj.shape_key_add(name=f"Walk_{index:03d}")
        key.data.foreach_set("co", positions[indices].ravel())
        frame = 1 + index * step
        for time, value in [(frame - step, 0), (frame, 1), (frame + step, 0)]:
            key.value = value
            key.keyframe_insert(data_path="value", frame=time)
    action = obj.data.shape_keys.animation_data.action
    for layer in action.layers:
        for strip in layer.strips:
            for bag in strip.channelbags:
                for curve in bag.fcurves:
                    for keyframe in curve.keyframe_points:
                        keyframe.interpolation = "LINEAR"
    scene.frame_start = 1
    scene.frame_end = round(1 + (len(values) - 1) * step)
    scene.frame_set(1)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.export_scene.gltf(filepath=str(args.output.resolve()), export_format="GLB",
                              export_frame_range=True, export_morph=True,
                              export_morph_normal=True, export_animations=True)
    report = {"frames": len(values), "fps": args.fps, "bind_max_error": error,
              "vertices": len(obj.data.vertices), "source_vertices": values.shape[1],
              "max_vertex_displacement": float(np.linalg.norm(values - values[0], axis=2).max()),
              "representation": "linear morph targets; not a skeletal rig"}
    args.output.with_suffix(".json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
