"""Make the editable Level 1 Castle pickup vessel and block-surface derivative.

Inspired by owner-local Day/Night TMAP 59 (32 x 26), a silver double-handled
urn with a red cap. No original pixels are embedded. Axes: Z up; origin at
base; full height 1. Run with Blender's Python using --output NEW_DIRECTORY.
"""
import argparse
import math
import sys
from pathlib import Path

import bpy
from mathutils import Vector
from mathutils.bvhtree import BVHTree


def material(name, rgb, metallic, roughness):
    mat = bpy.data.materials.new(name)
    mat.diffuse_color = (*rgb, 1)
    mat.use_nodes = True
    node = mat.node_tree.nodes.get('Principled BSDF')
    node.inputs['Base Color'].default_value = (*rgb, 1)
    node.inputs['Metallic'].default_value = metallic
    node.inputs['Roughness'].default_value = roughness
    return mat


def lathe(name, profile, mat):
    count = 96
    vertices = [(r*math.cos(math.tau*i/count), r*math.sin(math.tau*i/count), z)
                for z, r in profile for i in range(count)]
    faces = []
    for j in range(len(profile)-1):
        for i in range(count):
            a = j*count+i
            b = j*count+(i+1)%count
            faces.append((a, b, b+count, a+count))
    faces += [tuple(reversed(range(count))), tuple((len(profile)-1)*count+i for i in range(count))]
    mesh = bpy.data.meshes.new(name); mesh.from_pydata(vertices, [], faces); mesh.update()
    obj = bpy.data.objects.new(name, mesh); bpy.context.scene.collection.objects.link(obj)
    mesh.materials.append(mat)
    for face in mesh.polygons: face.use_smooth = len(face.vertices) == 4
    return obj


def handle(name, side, mat):
    # A closed bowed lug sweeps out from each shoulder, then back to the belly.
    points = [(.32,.72),(.43,.77),(.53,.76),(.60,.69),(.61,.58),
              (.59,.48),(.53,.39),(.43,.36),(.34,.39)]
    curve = bpy.data.curves.new(name, 'CURVE'); curve.dimensions = '3D'
    curve.resolution_u = 18; curve.bevel_depth = .043; curve.bevel_resolution = 4
    spline = curve.splines.new('BEZIER'); spline.bezier_points.add(len(points)-1)
    for p, (x,z) in zip(spline.bezier_points, points):
        p.co=(side*x,0,z); p.handle_left_type='AUTO'; p.handle_right_type='AUTO'
    obj = bpy.data.objects.new(name, curve); bpy.context.scene.collection.objects.link(obj)
    obj.data.materials.append(mat)
    bpy.ops.object.select_all(action='DESELECT'); obj.select_set(True); bpy.context.view_layer.objects.active=obj
    bpy.ops.object.convert(target='MESH')
    # Close each tube end for solid voxel sampling.
    import bmesh
    bm=bmesh.new(); bm.from_mesh(obj.data)
    edges=[e for e in bm.edges if e.is_boundary]
    if edges: bmesh.ops.holes_fill(bm, edges=edges, sides=0)
    bm.to_mesh(obj.data); bm.free(); obj.data.update()
    for face in obj.data.polygons: face.use_smooth=True
    return obj


def export(path, objects):
    bpy.ops.object.select_all(action='DESELECT')
    for obj in objects: obj.select_set(True)
    bpy.context.view_layer.objects.active=objects[0]
    bpy.ops.export_scene.fbx(filepath=str(path), use_selection=True, object_types={'MESH'},
                             add_leaf_bones=False, axis_forward='-Z', axis_up='Y')


def voxelize(objects, mats):
    size=1/40; lo=(-.65,-.40,0); dims=(52,32,40)
    deps=bpy.context.evaluated_depsgraph_get()
    trees=[BVHTree.FromObject(obj,deps) for obj in objects]
    cells={}; direction=Vector((0,0,1))
    for x in range(dims[0]):
        for y in range(dims[1]):
            px=lo[0]+(x+.5)*size; py=lo[1]+(y+.5)*size
            for idx, tree in enumerate(trees):
                pos=Vector((px,py,-.05)); hits=[]
                for _ in range(24):
                    hit,_,_,_=tree.ray_cast(pos,direction,1.2)
                    if hit is None: break
                    hits.append(hit.z); pos=hit+direction*.00001
                spans=list(zip(hits[::2],hits[1::2]))
                for z in range(dims[2]):
                    pz=(z+.5)*size
                    if any(a <= pz <= b for a,b in spans): cells[x,y,z]=idx
    sides=[((-1,0,0),((0,0,0),(0,0,1),(0,1,1),(0,1,0))),((1,0,0),((1,0,0),(1,1,0),(1,1,1),(1,0,1))),((0,-1,0),((0,0,0),(1,0,0),(1,0,1),(0,0,1))),((0,1,0),((0,1,0),(0,1,1),(1,1,1),(1,1,0))),((0,0,-1),((0,0,0),(0,1,0),(1,1,0),(1,0,0))),((0,0,1),((0,0,1),(1,0,1),(1,1,1),(0,1,1)))]
    vertices=[]; faces=[]; indices=[]
    for (x,y,z),index in sorted(cells.items()):
        for (dx,dy,dz),corners in sides:
            if (x+dx,y+dy,z+dz) in cells: continue
            start=len(vertices)
            vertices.extend((lo[0]+(x+a)*size,lo[1]+(y+b)*size,(z+c)*size) for a,b,c in corners)
            faces.append(tuple(range(start,start+4))); indices.append(index)
    mesh=bpy.data.meshes.new('Jar block surface'); mesh.from_pydata(vertices,[],faces); mesh.update()
    obj=bpy.data.objects.new('Jar block surface',mesh); bpy.context.scene.collection.objects.link(obj)
    for mat in mats: mesh.materials.append(mat)
    for face,index in zip(mesh.polygons,indices): face.material_index=index
    print('JAR_VOXEL cells=%d faces=%d' % (len(cells),len(faces)))
    return obj


def main():
    parser=argparse.ArgumentParser(); parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args(sys.argv[sys.argv.index('--')+1:]); args.output.mkdir(parents=True,exist_ok=False)
    bpy.ops.object.select_all(action='SELECT'); bpy.ops.object.delete(use_global=False)
    silver=material('Jar silver',(.48,.51,.56),.68,.28)
    trim=material('Jar dark rim',(.18,.19,.21),.60,.36)
    red=material('Jar red lid',(.62,.012,.018),.20,.3)
    objects=[
        lathe('Silver urn',[(0,.15),(.04,.21),(.10,.28),(.20,.34),(.31,.37),(.42,.38),(.57,.37),(.70,.34),(.75,.34)],silver),
        handle('Left handle',-1,trim), handle('Right handle',1,trim),
        lathe('Shoulder rim',[(.70,.35),(.72,.38),(.76,.38),(.78,.35)],silver),
        lathe('Red lid',[(.76,.35),(.79,.36),(.82,.32),(.86,.27),(.88,.20),(.91,.16),(.94,.11),(.98,.08),(1,.05)],red),
    ]
    bpy.ops.wm.save_as_mainfile(filepath=str(args.output/'CastleSpellJar.blend'))
    export(args.output/'CastleSpellJar.fbx',objects)
    voxel=voxelize(objects,[silver,trim,trim,silver,red])
    export(args.output/'CastleSpellJarVoxel.fbx',[voxel])


if __name__ == '__main__': main()
