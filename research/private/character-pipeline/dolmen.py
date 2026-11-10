"""Build a source-informed dolmen in Blender, without owner-data assets.

Run: blender --factory-startup -b --python-exit-code 2 --python dolmen.py -- --output NEW_DIR
The output directory must not exist. The .blend contains four separately editable
stones; the two FBX exports share that geometry, with the second block-sampled.
"""
import argparse
import math
import random
import sys
from pathlib import Path

import bmesh
import bpy
from mathutils import Vector, noise
from mathutils.bvhtree import BVHTree


def stone(name, center, radii, seed, slab=False):
    rng = random.Random(seed)
    # Rings preserve a broad cap and approximately flat feet, while individual
    # vertices and angular facets keep the stone naturally broken and uneven.
    sectors = 24
    levels = 9
    verts = []
    for j in range(levels):
        t = j / (levels - 1)
        profile = max(.13, math.sin(math.pi * (.09 + .82 * t)) ** .48)
        for i in range(sectors):
            angle = 2 * math.pi * i / sectors
            wobble = 1 + .075 * math.sin(3 * angle + seed) + .045 * rng.uniform(-1, 1)
            x = center[0] + radii[0] * profile * math.cos(angle) * wobble
            y = center[1] + radii[1] * profile * math.sin(angle) * wobble
            z = center[2] + radii[2] * (2 * t - 1)
            z += radii[2] * .10 * math.sin(5 * angle + 3 * t + seed) * (0.3 + 0.7 * math.sin(math.pi * t))
            if not slab and j == 0:
                z = 0
            verts.append((x, y, z))
    faces = []
    for j in range(levels - 1):
        for i in range(sectors):
            a = j * sectors + i
            b = j * sectors + (i + 1) % sectors
            c = (j + 1) * sectors + (i + 1) % sectors
            d = (j + 1) * sectors + i
            faces.extend(((a, b, c), (a, c, d)))
    faces.append(tuple(reversed(range(sectors))))
    faces.append(tuple((levels - 1) * sectors + i for i in range(sectors)))
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata(verts, [], faces)
    mesh.update()
    # Broken surfaces with a fine, stratified relief. Geometry remains the
    # editable source of both the Modern and block-sampled versions.
    bm = bmesh.new()
    bm.from_mesh(mesh)
    bmesh.ops.subdivide_edges(bm, edges=list(bm.edges), cuts=1, use_grid_fill=True)
    bm.normal_update()
    for v in bm.verts:
        original_z = v.co.z
        p = v.co
        grit = noise.noise_vector(Vector((p.x * 17 + seed, p.y * 17, p.z * 17))).x
        strata = math.sin(p.z * 62 + noise.noise_vector(p * 8).y * 1.4)
        v.co += v.normal * (.008 * grit + .006 * strata)
        if not slab and original_z == 0:
            v.co.z = 0
    bm.to_mesh(mesh)
    bm.free()
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    bpy.context.scene.collection.objects.link(obj)
    return obj


def voxelize(stones, size=1 / 32):
    trees = [BVHTree.FromObject(obj, bpy.context.evaluated_depsgraph_get()) for obj in stones]
    lo = (-.91, -.44, 0.0)
    dims = (59, 29, 34)
    solid = set()
    direction = Vector((0.0, 0.0, 1.0))
    # Pair all vertical intersections for each independent, closed stone.
    for x in range(dims[0]):
        for y in range(dims[1]):
            px, py = lo[0] + (x + .5) * size, lo[1] + (y + .5) * size
            spans = []
            for tree in trees:
                pos = Vector((px, py, -.1))
                hits = []
                for _ in range(8):
                    hit, _, _, _ = tree.ray_cast(pos, direction, 2.0)
                    if hit is None:
                        break
                    hits.append(hit.z)
                    pos = hit + direction * .0001
                spans.extend(zip(hits[::2], hits[1::2]))
            for z in range(dims[2]):
                pz = (z + .5) * size
                if any(a <= pz <= b for a, b in spans):
                    solid.add((x, y, z))
    vertices, faces = [], []
    sides = [((-1,0,0),((0,0,0),(0,0,1),(0,1,1),(0,1,0))),
             ((1,0,0),((1,0,0),(1,1,0),(1,1,1),(1,0,1))),
             ((0,-1,0),((0,0,0),(1,0,0),(1,0,1),(0,0,1))),
             ((0,1,0),((0,1,0),(0,1,1),(1,1,1),(1,1,0))),
             ((0,0,-1),((0,0,0),(0,1,0),(1,1,0),(1,0,0))),
             ((0,0,1),((0,0,1),(1,0,1),(1,1,1),(0,1,1)))]
    for x,y,z in sorted(solid):
        for (dx,dy,dz), corners in sides:
            if (x+dx,y+dy,z+dz) in solid:
                continue
            first = len(vertices)
            vertices.extend((lo[0]+(x+a)*size, lo[1]+(y+b)*size, (z+c)*size) for a,b,c in corners)
            faces.append(tuple(range(first, first+4)))
    mesh = bpy.data.meshes.new('Dolmen voxel surface')
    mesh.from_pydata(vertices, [], faces)
    mesh.update()
    obj = bpy.data.objects.new('Dolmen voxel surface', mesh)
    bpy.context.scene.collection.objects.link(obj)
    print(f'DOLMEN_VOXEL cells={len(solid)} surface_faces={len(faces)}')
    return obj


def export(path, objects):
    bpy.ops.object.select_all(action='DESELECT')
    for obj in objects:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    bpy.ops.export_scene.fbx(filepath=str(path), use_selection=True, object_types={'MESH'},
                             add_leaf_bones=False, axis_forward='-Z', axis_up='Y')


def stone_uv(obj):
    # Each face projects on its largest normal axis, avoiding stretched vertical
    # rock texture without depending on generated UV unwrap state.
    layer = obj.data.uv_layers.new(name='Stone')
    for poly in obj.data.polygons:
        axis = max(range(3), key=lambda n: abs(poly.normal[n]))
        for loop in poly.loop_indices:
            v = obj.data.vertices[obj.data.loops[loop].vertex_index].co
            layer.data[loop].uv = ((v.y, v.z) if axis == 0 else
                                   (v.x, v.z) if axis == 1 else (v.x, v.y))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args(sys.argv[sys.argv.index('--') + 1:])
    args.output.mkdir(parents=True, exist_ok=False)
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.object.delete(use_global=False)
    stones = [stone('Left upright',(-.50,-.08,.36),(.20,.23,.36),13),
              stone('Right upright',(.48,-.05,.37),(.23,.24,.37),23),
              stone('Rear upright',(.02,.23,.36),(.19,.18,.36),47),
              stone('Overhanging capstone',(0,0,.84),(.87,.37,.16),71,True)]
    mat = bpy.data.materials.new('Weathered brown grey stone')
    mat.diffuse_color = (.32,.30,.28,1)
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes.get('Principled BSDF')
    bsdf.inputs['Base Color'].default_value = mat.diffuse_color
    bsdf.inputs['Roughness'].default_value = .9
    for obj in stones:
        obj.data.materials.append(mat)
        stone_uv(obj)
    bpy.ops.wm.save_as_mainfile(filepath=str(args.output / 'Dolmen.blend'))
    export(args.output / 'Dolmen.fbx', stones)
    voxel = voxelize(stones)
    voxel.data.materials.append(mat)
    stone_uv(voxel)
    export(args.output / 'DolmenVoxel.fbx', [voxel])


if __name__ == '__main__':
    main()
