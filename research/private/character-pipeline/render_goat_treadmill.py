"""Render the exported goat on a moving checker floor for contact review.

blender -b -t 2 -P render_goat_treadmill.py -- --model goat-rigged.glb --output frames
The camera and goat remain fixed; the floor moves backward at the authored
stance speed. This intentionally shows three repetitions of an in-place cycle.
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
    parser.add_argument("--engine", default="BLENDER_EEVEE_NEXT")
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1:])
    args.output.mkdir(parents=True, exist_ok=True)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(args.model.resolve()))
    scene = bpy.context.scene
    meshes = [o for o in scene.objects if o.type == "MESH" and len(o.data.vertices) > 100]
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
    material.use_nodes = True
    nodes = material.node_tree.nodes
    bsdf = nodes.get('Principled BSDF')
    tex = nodes.new('ShaderNodeTexChecker')
    tex.inputs['Color1'].default_value = (.12, .14, .16, 1)
    tex.inputs['Color2'].default_value = (.22, .24, .26, 1)
    tex.inputs['Scale'].default_value = 5
    coord = nodes.new('ShaderNodeTexCoord')
    material.node_tree.links.new(coord.outputs['Object'], tex.inputs['Vector'])
    material.node_tree.links.new(tex.outputs['Color'], bsdf.inputs['Base Color'])
    floor_origin = floor.location.copy()
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
    camera.data.type, camera.data.ortho_scale = "ORTHO", size * 1.85
    report = {"model": str(args.model), "mesh_count": len(meshes),
              "vertices": sum(len(o.data.vertices) for o in meshes),
              "polygons": sum(len(o.data.polygons) for o in meshes),
              "shape_keys": sum(len(o.data.shape_keys.key_blocks) if o.data.shape_keys else 0 for o in meshes),
              "actions": len(bpy.data.actions), "frames": [start, end],
              "bounds": [list(low), list(high)]}
    (args.output / "inspection.json").write_text(json.dumps(report, indent=2) + "\n")
    for angle in [130]:
        radians = math.radians(angle)
        camera.location = center + Vector((math.sin(radians) * 3, -math.cos(radians) * 3, 0.65)) * size
        camera.rotation_euler = (center - camera.location).to_track_quat("-Z", "Y").to_euler()
        frames = range(1, 73)
        for frame in frames:
            scene.frame_set(1 + (frame - 1) % 24)
            floor.location.y = floor_origin.y - (frame - 1) * .095 / .64 / 24
            scene.render.filepath = str((args.output / f"view-{int(angle):03d}-frame-{frame:04d}.png").resolve())
            bpy.ops.render.render(write_still=True)


if __name__ == "__main__":
    main()
