"""Construct the Level 1 night-bank statue as editable stone components.

The owner TMAP 078 is a frontal, blue-grey robed figure on a plinth. Its
unseen rear and depth are interpreted here; no owner pixels are distributed.
Run with Blender: --background --python-exit-code 2 --python stone_statue.py
-- --output NEW_DIRECTORY. Both exports use a one-unit total height and
grounded origin; the caller applies the native logical height.

The Unity material uses the Poly Haven CC0 RockAlbedo derivative described in
ModernTextures/README.md (source SHA-256 98ecbd1d19953433f5909b79958992b1da2b45addbee772d1d4c27287f368499).
Reproduce it from the Unity project root with:
ffmpeg -i Assets/Resources/ModernTextures/RockAlbedo.jpg -vf
'hue=s=0,eq=contrast=0.45:brightness=0.04,scale=1024:1024'
Assets/Resources/ModernScenery/StoneStatueAlbedo.png
"""
import argparse
import math
import sys
from pathlib import Path

import bpy
from mathutils import Vector, noise
from mathutils.bvhtree import BVHTree


def finish(name, location, scale, bevel=0):
    obj = bpy.context.object
    obj.name = name
    obj.location = location
    obj.scale = scale
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    if bevel:
        modifier = obj.modifiers.new('Soft carved edges', 'BEVEL')
        modifier.width = bevel
        modifier.segments = 2
        bpy.context.view_layer.objects.active = obj
        bpy.ops.object.modifier_apply(modifier=modifier.name)
    return obj


def block(name, location, halfsize, bevel=.006):
    bpy.ops.mesh.primitive_cube_add()
    return finish(name, location, halfsize, bevel)


def oval(name, location, radii):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=32, ring_count=20)
    obj = finish(name, location, radii)
    for polygon in obj.data.polygons:
        polygon.use_smooth = True
    return obj


def limb(name, a, b, radius):
    a, b = Vector(a), Vector(b)
    obj = oval(name, (a + b) / 2, (radius, radius, (b - a).length / 2 + radius))
    obj.rotation_euler = (b - a).to_track_quat('Z', 'Y').to_euler()
    return obj


def face():
    bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=40)
    obj = finish('Weathered human face', (0, -.002, .900), (.057, .049, .075))
    for polygon in obj.data.polygons:
        polygon.use_smooth = True
    # Work the facial planes directly into the head instead of attaching a
    # mask or geometric features. All displacements are deliberately shallow.
    def bell(x, center, width):
        return math.exp(-((x - center) / width) ** 2)
    for vertex in obj.data.vertices:
        p = vertex.co
        if p.y >= -.012:
            continue
        frontal = min(1.0, max(0.0, (-p.y - .012) / .025))
        nose = .014 * bell(p.x, 0, .011) * bell(p.z, -.002, .028)
        sockets = .006 * (bell(p.x, -.022, .012) + bell(p.x, .022, .012)) * bell(p.z, .014, .010)
        brow = .004 * bell(p.x, 0, .050) * bell(p.z, .028, .008)
        cheeks = .003 * (bell(p.x, -.031, .013) + bell(p.x, .031, .013)) * bell(p.z, -.010, .020)
        mouth = .003 * bell(p.x, 0, .024) * bell(p.z, -.035, .006)
        p.y += frontal * (-nose + sockets - brow - cheeks + mouth)
        if p.z < -.025:
            p.x *= 1 - .12 * min(1, (-p.z - .025) / .05)
    obj.data.update()
    return obj


def drape(name, rings, folds, seed):
    """Closed, softly folded stone garment, sampled from editable profiles."""
    sectors = 48
    vertices = []
    for z, rx, ry in rings:
        for i in range(sectors):
            angle = 2 * math.pi * i / sectors
            # Irregular near-vertical folds remain much broader than the
            # weathering so the drape reads at game-camera scale.
            ripple = (math.sin(folds * angle + seed + z * 2.3) +
                      .45 * math.sin((folds + 3) * angle - seed + z * 5))
            amount = .007 * ripple
            vertices.append(((rx + amount) * math.cos(angle),
                             (ry + amount) * math.sin(angle), z))
    faces = []
    for j in range(len(rings) - 1):
        for i in range(sectors):
            a = j * sectors + i
            b = j * sectors + (i + 1) % sectors
            faces.append((a, b, b + sectors, a + sectors))
    faces.append(tuple(reversed(range(sectors))))
    faces.append(tuple((len(rings) - 1) * sectors + i for i in range(sectors)))
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata(vertices, [], faces)
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    bpy.context.scene.collection.objects.link(obj)
    for polygon in mesh.polygons:
        polygon.use_smooth = len(polygon.vertices) == 4
    return obj


def weather(obj, seed):
    """Slight surface erosion; component meshes remain separately editable."""
    mesh = obj.data
    mesh.update()
    for vertex in mesh.vertices:
        point = obj.matrix_world @ vertex.co
        if point.z < .008:
            continue
        grain = noise.noise_vector(Vector((point.x * 32 + seed,
                                           point.y * 32, point.z * 32))).x
        weathering = noise.noise_vector(Vector((point.x * 11 + seed,
                                                point.y * 11, point.z * 11))).y
        vertex.co += vertex.normal * (.0015 * grain + .0014 * weathering)
    mesh.update()


def uv(obj):
    layer = obj.data.uv_layers.new(name='Stone')
    for polygon in obj.data.polygons:
        axis = max(range(3), key=lambda n: abs(polygon.normal[n]))
        for loop in polygon.loop_indices:
            v = obj.data.vertices[obj.data.loops[loop].vertex_index].co
            layer.data[loop].uv = ((v.y, v.z) if axis == 0 else
                                   (v.x, v.z) if axis == 1 else (v.x, v.y))


def voxelize(objects, step=1 / 64):
    # BVHs operate in world coordinates, including all component transforms.
    trees = []
    for obj in objects:
        mesh = obj.data.copy()
        mesh.transform(obj.matrix_world)
        trees.append(BVHTree.FromPolygons([v.co for v in mesh.vertices],
                                          [p.vertices[:] for p in mesh.polygons]))
        bpy.data.meshes.remove(mesh)
    lo = (-.25, -.17, 0.0)
    dims = (32, 22, 64)
    solid = set()
    direction = Vector((0, 0, 1))
    for x in range(dims[0]):
        for y in range(dims[1]):
            px, py = lo[0] + (x + .5) * step, lo[1] + (y + .5) * step
            spans = []
            for tree in trees:
                pos = Vector((px, py, -.1))
                hits = []
                for _ in range(24):
                    hit, _, _, _ = tree.ray_cast(pos, direction, 1.3)
                    if hit is None:
                        break
                    hits.append(hit.z)
                    pos = hit + direction * .0001
                spans.extend(zip(hits[::2], hits[1::2]))
            for z in range(dims[2]):
                pz = (z + .5) * step
                if any(a <= pz <= b for a, b in spans):
                    solid.add((x, y, z))
    sides = [((-1,0,0),((0,0,0),(0,0,1),(0,1,1),(0,1,0))),
             ((1,0,0),((1,0,0),(1,1,0),(1,1,1),(1,0,1))),
             ((0,-1,0),((0,0,0),(1,0,0),(1,0,1),(0,0,1))),
             ((0,1,0),((0,1,0),(0,1,1),(1,1,1),(1,1,0))),
             ((0,0,-1),((0,0,0),(0,1,0),(1,1,0),(1,0,0))),
             ((0,0,1),((0,0,1),(1,0,1),(1,1,1),(0,1,1)))]
    vertices, faces = [], []
    for x, y, z in sorted(solid):
        for (dx, dy, dz), corners in sides:
            if (x + dx, y + dy, z + dz) in solid:
                continue
            first = len(vertices)
            vertices.extend((lo[0] + (x+a)*step, lo[1] + (y+b)*step,
                             (z+c)*step) for a,b,c in corners)
            faces.append(tuple(range(first, first+4)))
    mesh = bpy.data.meshes.new('Statue block surface')
    mesh.from_pydata(vertices, [], faces)
    mesh.update()
    obj = bpy.data.objects.new('Statue block surface', mesh)
    bpy.context.scene.collection.objects.link(obj)
    print(f'STATUE_VOXEL cells={len(solid)} surface_faces={len(faces)}')
    return obj


def export(path, objects):
    bpy.ops.object.select_all(action='DESELECT')
    for obj in objects:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    bpy.ops.export_scene.fbx(filepath=str(path), use_selection=True, object_types={'MESH'},
                             add_leaf_bones=False, axis_forward='-Z', axis_up='Y')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args(sys.argv[sys.argv.index('--') + 1:])
    args.output.mkdir(parents=True, exist_ok=False)
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.object.delete(use_global=False)
    noise.seed_set(17)
    # Blender's -Y is the front. Source is a single image; the rounder human
    # form, stone drapery and head details are modeled interpretations.
    parts = [
        block('Single worn pedestal', (0,0,.085), (.155,.103,.085), .020),
        oval('Left foot', (-.055,-.018,.195), (.043,.071,.026)),
        oval('Right foot', (.055,-.018,.195), (.043,.071,.026)),
        limb('Left lower leg', (-.055,0,.215), (-.053,0,.365), .037),
        limb('Right lower leg', (.055,0,.215), (.053,0,.365), .037),
        drape('Folded lower robe', [(.235,.103,.060),(.260,.106,.061),
              (.285,.108,.061),(.310,.108,.061),(.340,.107,.060),(.365,.111,.061),
              (.390,.110,.061),(.420,.108,.059),(.450,.107,.058),
              (.480,.105,.057),(.510,.102,.057),(.540,.098,.057),
              (.570,.094,.055),(.600,.091,.053)], 7, 1.4),
        drape('Draped upper robe', [(.555,.090,.053),(.580,.091,.055),
              (.605,.094,.058),(.630,.099,.060),(.655,.105,.061),
              (.680,.111,.061),(.705,.116,.060),(.730,.121,.058),
              (.755,.116,.056),(.780,.101,.051),(.800,.070,.045),
              (.815,.044,.038)], 6, .5),
        limb('Left upper sleeve', (-.105,0,.755), (-.145,-.004,.660), .034),
        limb('Right upper sleeve', (.105,0,.755), (.145,-.004,.660), .034),
        limb('Left crossed forearm', (-.145,-.010,.655), (.045,-.095,.620), .025),
        limb('Right crossed forearm', (.145,-.012,.655), (-.045,-.100,.620), .025),
        oval('Left hand', (.044,-.095,.620), (.027,.020,.018)),
        oval('Right hand', (-.044,-.100,.620), (.027,.020,.018)),
        oval('Neck', (0,0,.827), (.035,.034,.042)),
        face(),
        oval('Left ear', (-.057,0,.899), (.009,.015,.021)),
        oval('Right ear', (.057,0,.899), (.009,.015,.021)),
        oval('Weathered hair', (0,.017,.939), (.057,.041,.036)),
    ]
    bpy.context.view_layer.update()
    mat = bpy.data.materials.new('Blue grey weathered stone')
    mat.diffuse_color = (.24,.31,.35,1)
    for index, obj in enumerate(parts):
        weather(obj, index * .7)
        obj.data.materials.append(mat)
        uv(obj)
    bpy.ops.wm.save_as_mainfile(filepath=str(args.output / 'StoneStatue.blend'))
    export(args.output / 'StoneStatue.fbx', parts)
    voxel = voxelize(parts)
    voxel.data.materials.append(mat)
    uv(voxel)
    export(args.output / 'StoneStatueVoxel.fbx', [voxel])


if __name__ == '__main__':
    main()
