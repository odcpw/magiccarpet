"""Construct the Level 1 night-bank statue as editable stone components.

The owner TMAP 078 is a frontal, blue-grey armored figure on a plinth. Its
unseen rear and depth are interpreted here; no owner pixels are distributed.
Run with Blender: --background --python-exit-code 2 --python stone_statue.py
-- --output NEW_DIRECTORY. Both exports use a one-unit total height and
grounded origin; the caller applies the native logical height.
"""
import argparse
import sys
from pathlib import Path

import bpy
from mathutils import Vector
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
    bpy.ops.mesh.primitive_uv_sphere_add(segments=20, ring_count=12)
    return finish(name, location, radii)


def limb(name, a, b, radius):
    a, b = Vector(a), Vector(b)
    obj = oval(name, (a + b) / 2, (radius, radius, (b - a).length / 2 + radius))
    obj.rotation_euler = (b - a).to_track_quat('Z', 'Y').to_euler()
    return obj


def uv(obj):
    layer = obj.data.uv_layers.new(name='Stone')
    for polygon in obj.data.polygons:
        axis = max(range(3), key=lambda n: abs(polygon.normal[n]))
        for loop in polygon.loop_indices:
            v = obj.data.vertices[obj.data.loops[loop].vertex_index].co
            layer.data[loop].uv = ((v.y, v.z) if axis == 0 else
                                   (v.x, v.z) if axis == 1 else (v.x, v.y))


def voxelize(objects, step=1 / 48):
    # BVHs operate in world coordinates, including all component transforms.
    trees = []
    for obj in objects:
        mesh = obj.data.copy()
        mesh.transform(obj.matrix_world)
        trees.append(BVHTree.FromPolygons([v.co for v in mesh.vertices],
                                          [p.vertices[:] for p in mesh.polygons]))
        bpy.data.meshes.remove(mesh)
    lo = (-.25, -.16, 0.0)
    dims = (24, 16, 49)
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
    # Blender's -Y is the statue's front. The broad rectangular pedestal and
    # crossed arms reproduce the readable silhouette of the source frame.
    parts = [
        block('Plinth foot', (0,0,.025), (.165,.115,.025)),
        block('Plinth body', (0,0,.105), (.145,.100,.060)),
        block('Plinth cap', (0,0,.178), (.175,.118,.013)),
        block('Left boot', (-.055,-.005,.210), (.045,.075,.021)),
        block('Right boot', (.055,-.005,.210), (.045,.075,.021)),
        block('Left greave', (-.054,0,.292), (.043,.047,.065)),
        block('Right greave', (.054,0,.292), (.043,.047,.065)),
        block('Lower armor skirt', (0,0,.426), (.109,.053,.077), .012),
        block('Left skirt panel', (-.073,-.053,.433), (.028,.012,.069)),
        block('Center skirt panel', (0,-.057,.433), (.029,.012,.070)),
        block('Right skirt panel', (.073,-.053,.433), (.028,.012,.069)),
        block('Belt', (0,0,.513), (.110,.059,.013)),
        oval('Breastplate', (0,0,.620), (.109,.066,.108)),
        block('Breastplate band upper', (0,-.060,.656), (.088,.012,.010)),
        block('Breastplate band lower', (0,-.060,.627), (.094,.012,.009)),
        oval('Left pauldron', (-.130,0,.685), (.064,.062,.040)),
        oval('Right pauldron', (.130,0,.685), (.064,.062,.040)),
        limb('Left upper arm', (-.150,0,.664), (-.165,-.007,.581), .032),
        limb('Right upper arm', (.150,0,.664), (.165,-.007,.581), .032),
        limb('Left crossed forearm', (-.160,-.010,.573), (.035,-.078,.545), .028),
        limb('Right crossed forearm', (.160,-.010,.573), (-.035,-.080,.545), .028),
        oval('Left hand', (.033,-.080,.545), (.030,.024,.025)),
        oval('Right hand', (-.033,-.083,.545), (.030,.024,.025)),
        oval('Neck', (0,0,.740), (.040,.038,.034)),
        oval('Head', (0,0,.813), (.068,.052,.077)),
        block('Helmet brow', (0,-.040,.850), (.069,.017,.012)),
        oval('Helmet crown', (0,.005,.869), (.078,.057,.048)),
        block('Helmet left wing', (-.076,.005,.874), (.024,.045,.015)),
        block('Helmet right wing', (.076,.005,.874), (.024,.045,.015)),
    ]
    # Extend the source figure to normalized height one without altering its
    # aspect; the top of the helmet is the highest modeled point.
    for obj in parts:
        obj.location /= .917
        obj.scale /= .917
    bpy.context.view_layer.update()
    mat = bpy.data.materials.new('Blue grey weathered stone')
    mat.diffuse_color = (.24,.31,.35,1)
    for obj in parts:
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
