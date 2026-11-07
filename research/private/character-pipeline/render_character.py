"""Blender inspection render: actual mesh, fixed lights and several viewpoints.

blender -b -P render_character.py -- --model goat.glb --output review
Use --animate to render every animation frame from the chosen viewpoint.
"""
import argparse
import json
import math
import sys
from pathlib import Path

import bpy
from mathutils import Vector


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--animate", action="store_true")
    parser.add_argument("--view", type=float, default=0)
    parser.add_argument("--frames", type=int, nargs="+")
    parser.add_argument("--engine", default="CYCLES")
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1:])
    args.output.mkdir(parents=True, exist_ok=True)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    if args.model.suffix.lower() == '.fbx':
        bpy.ops.import_scene.fbx(filepath=str(args.model.resolve()))
    else:
        bpy.ops.import_scene.gltf(filepath=str(args.model.resolve()))
    scene = bpy.context.scene
    meshes = [o for o in scene.objects if o.type == "MESH"]
    if not meshes:
        raise ValueError("Model contains no mesh")
    bounds = [o.matrix_world @ Vector(v) for o in meshes for v in o.bound_box]
    low = Vector(tuple(min(v[i] for v in bounds) for i in range(3)))
    high = Vector(tuple(max(v[i] for v in bounds) for i in range(3)))
    center = (low + high) / 2
    size = max(high - low)
    end = max([int(a.frame_range[1]) for a in bpy.data.actions] or [1])
    start = min([int(a.frame_range[0]) for a in bpy.data.actions] or [1])
    scene.frame_start, scene.frame_end = start, end
    scene.render.engine = args.engine
    if args.engine == "CYCLES":
        scene.cycles.samples = 24
        scene.cycles.use_denoising = True
    scene.render.resolution_x, scene.render.resolution_y = 768, 576
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = "PNG"
    scene.render.fps = 24
    scene.world = bpy.data.worlds.new("Studio")
    scene.world.use_nodes = True
    scene.world.node_tree.nodes["Background"].inputs[0].default_value = (0.15, 0.17, 0.2, 1)
    scene.world.node_tree.nodes["Background"].inputs[1].default_value = 0.7
    bpy.ops.mesh.primitive_plane_add(size=size * 200, location=(center.x, center.y, low.z))
    floor = bpy.context.object
    material = bpy.data.materials.new("Neutral floor")
    material.diffuse_color = (0.13, 0.15, 0.18, 1)
    floor.data.materials.append(material)
    for name, offset, power, width in [
        ("Key", (1, -2, 3), 450, 2.0),
        ("Fill", (-2, -1, 1.5), 180, 2.5),
        ("Rim", (1, 2, 2), 350, 1.5),
    ]:
        data = bpy.data.lights.new(name, "AREA")
        data.energy, data.shape, data.size = power * size * size, "DISK", width * size
        light = bpy.data.objects.new(name, data)
        scene.collection.objects.link(light)
        light.location = center + Vector(offset) * size
        light.rotation_euler = (center - light.location).to_track_quat("-Z", "Y").to_euler()
    bpy.ops.object.camera_add()
    camera = bpy.context.object
    scene.camera = camera
    camera.data.type, camera.data.ortho_scale = "ORTHO", size * 1.55
    report = {"model": str(args.model), "mesh_count": len(meshes),
              "vertices": sum(len(o.data.vertices) for o in meshes),
              "polygons": sum(len(o.data.polygons) for o in meshes),
              "shape_keys": sum(len(o.data.shape_keys.key_blocks) if o.data.shape_keys else 0 for o in meshes),
              "actions": len(bpy.data.actions), "frames": [start, end],
              "bounds": [list(low), list(high)]}
    (args.output / "inspection.json").write_text(json.dumps(report, indent=2) + "\n")
    for angle in ([args.view] if args.animate else [0, 90, 180, 270]):
        radians = math.radians(angle)
        camera.location = center + Vector((math.sin(radians) * 3, -math.cos(radians) * 3, 0.65)) * size
        camera.rotation_euler = (center - camera.location).to_track_quat("-Z", "Y").to_euler()
        frames = range(start, end + 1) if args.animate else (args.frames or sorted({start, (start + end) // 2, end}))
        for frame in frames:
            scene.frame_set(frame)
            scene.render.filepath = str((args.output / f"view-{int(angle):03d}-frame-{frame:04d}.png").resolve())
            bpy.ops.render.render(write_still=True)


if __name__ == "__main__":
    main()
