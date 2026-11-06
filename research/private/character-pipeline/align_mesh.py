"""Bake a GLB yaw alignment in world coordinates, independent of rotation mode."""
import argparse
import json
import math
import sys
from pathlib import Path

import bpy
from mathutils import Matrix


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--yaw-degrees", required=True, type=float)
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1:])
    if args.output.exists():
        raise FileExistsError(args.output)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(args.model.resolve()))
    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    if len(meshes) != 1:
        raise ValueError("Expected one character mesh")
    obj = meshes[0]
    rotation = Matrix.Rotation(math.radians(args.yaw_degrees), 4, "Z")
    transform = rotation @ obj.matrix_world.copy()
    expected = [transform @ v.co for v in obj.data.vertices]
    obj.data.transform(transform)
    obj.parent = None
    obj.matrix_world = Matrix.Identity(4)
    obj.data.update()
    error = max((a - b.co).length for a, b in zip(expected, obj.data.vertices))
    if error > 1e-6:
        raise ValueError(f"Rotation bake mismatch: {error}")
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    args.output.parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.export_scene.gltf(filepath=str(args.output.resolve()), export_format="GLB", use_selection=True)
    args.output.with_suffix(".json").write_text(json.dumps({
        "source": str(args.model), "yaw_degrees": args.yaw_degrees,
        "max_bake_error": error, "matrix": [list(row) for row in transform],
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
