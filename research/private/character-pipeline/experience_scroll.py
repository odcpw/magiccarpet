"""Build an editable, source-informed MC2 experience scroll and voxel derivative.

The TMAP 259 reference is a static 32x12 horizontal rolled parchment with a
violet center binding. No owner data is used or embedded in these exports.
Run: blender -b --python-exit-code 2 --python experience_scroll.py -- --output NEW_DIR
"""
import argparse
import math
import random
import sys
from pathlib import Path

import bpy
from mathutils import Vector
from mathutils.bvhtree import BVHTree


def material(name, color):
    m = bpy.data.materials.new(name)
    m.diffuse_color = (*color, 1)
    m.use_nodes = True
    p = m.node_tree.nodes.get('Principled BSDF')
    p.inputs['Base Color'].default_value = (*color, 1)
    p.inputs['Roughness'].default_value = .8
    return m


def tube(name, x0, x1, radius, mat, segments=96, rings=24):
    verts, faces = [], []
    for j in range(rings + 1):
        t = j / rings
        x = x0 + (x1-x0)*t
        for i in range(segments):
            a = math.tau*i/segments
            # Subtle irregularity gives rolled material a less machined edge.
            r = radius*(1 + .012*math.sin(3*a+4*t) + .007*math.sin(11*a+13*t))
            verts.append((x, r*math.cos(a), .5+r*math.sin(a)))
    for j in range(rings):
        for i in range(segments):
            a=j*segments+i; b=j*segments+(i+1)%segments
            faces.append((a,b,b+segments,a+segments))
    faces.extend((tuple(reversed(range(segments))),tuple(rings*segments+i for i in range(segments))))
    mesh=bpy.data.meshes.new(name); mesh.from_pydata(verts,[],faces); mesh.update()
    obj=bpy.data.objects.new(name,mesh); bpy.context.scene.collection.objects.link(obj)
    obj.data.materials.append(mat)
    for p in mesh.polygons: p.use_smooth = len(p.vertices)==4
    scroll_uv(mesh)
    return obj


def scroll_uv(mesh):
    uv = mesh.uv_layers.new(name='Parchment')
    for poly in mesh.polygons:
        coords = []
        for index in poly.loop_indices:
            v = mesh.vertices[mesh.loops[index].vertex_index].co
            coords.append(((v.x + 1.36) / 2.72, (math.atan2(v.z - .5, v.y) / math.tau) % 1))
        # Unwrap the cylinder seam within each face.
        if max(c[1] for c in coords) - min(c[1] for c in coords) > .5:
            coords = [(u, v + 1 if v < .5 else v) for u, v in coords]
        for index, coordinate in zip(poly.loop_indices, coords): uv.data[index].uv = coordinate


def parchment_texture(path):
    # Deterministic, tileable fine fiber and broad mottling; neutral white is
    # multiplied by the parchment material in the runtime shader.
    rng = random.Random(259)
    width, height = 1024, 512
    image = bpy.data.images.new('Parchment fibers', width=width, height=height, alpha=True)
    pixels = []
    for y in range(height):
        for x in range(width):
            coarse = .04*math.sin(x*math.tau/71)*math.sin(y*math.tau/97)
            fiber = .025*math.sin(y*math.tau/13 + math.sin(x*math.tau/101))
            n = rng.uniform(-.035,.035)
            value = max(.65,min(1,.88+coarse+fiber+n))
            pixels.extend((value, value*.97, value*.91, 1))
    image.pixels.foreach_set(pixels); image.filepath_raw=str(path); image.file_format='PNG'; image.save()
    image.pack()
    return image


def export(path, objects):
    bpy.ops.object.select_all(action='DESELECT')
    for obj in objects: obj.select_set(True)
    bpy.context.view_layer.objects.active=objects[0]
    bpy.ops.export_scene.fbx(filepath=str(path), use_selection=True, object_types={'MESH'},
                             add_leaf_bones=False, axis_forward='-Z', axis_up='Y')


def voxelize(objects, materials, size=1/28):
    # Independently sample the closed pieces, in material priority order.
    trees=[BVHTree.FromObject(o,bpy.context.evaluated_depsgraph_get()) for o in objects]
    lo=(-1.36,-.5,0); dims=(77,28,28); cells={}
    direction=Vector((0,0,1))
    for x in range(dims[0]):
        for y in range(dims[1]):
            px=lo[0]+(x+.5)*size; py=lo[1]+(y+.5)*size
            for idx, tree in enumerate(trees):
                pos=Vector((px,py,-.1)); hits=[]
                for _ in range(12):
                    hit,_,_,_=tree.ray_cast(pos,direction,2)
                    if hit is None: break
                    hits.append(hit.z); pos=hit+direction*.0001
                spans=list(zip(hits[::2],hits[1::2]))
                for z in range(dims[2]):
                    pz=(z+.5)*size
                    if any(a<=pz<=b for a,b in spans): cells[(x,y,z)]=idx
    sides=[((-1,0,0),((0,0,0),(0,0,1),(0,1,1),(0,1,0))),((1,0,0),((1,0,0),(1,1,0),(1,1,1),(1,0,1))),((0,-1,0),((0,0,0),(1,0,0),(1,0,1),(0,0,1))),((0,1,0),((0,1,0),(0,1,1),(1,1,1),(1,1,0))),((0,0,-1),((0,0,0),(0,1,0),(1,1,0),(1,0,0))),((0,0,1),((0,0,1),(1,0,1),(1,1,1),(0,1,1)))]
    vertices=[]; faces=[]; indices=[]
    for (x,y,z), mat in sorted(cells.items()):
        for (dx,dy,dz),corners in sides:
            if (x+dx,y+dy,z+dz) in cells: continue
            start=len(vertices); vertices.extend((lo[0]+(x+a)*size,lo[1]+(y+b)*size,(z+c)*size) for a,b,c in corners)
            faces.append(tuple(range(start,start+4))); indices.append(mat)
    mesh=bpy.data.meshes.new('Scroll block surface'); mesh.from_pydata(vertices,[],faces); mesh.update()
    obj=bpy.data.objects.new('Scroll block surface',mesh); bpy.context.scene.collection.objects.link(obj)
    for m in materials: mesh.materials.append(m)
    for face, index in zip(mesh.polygons,indices): face.material_index=index
    scroll_uv(mesh)
    print(f'SCROLL_VOXEL cells={len(cells)} faces={len(faces)}')
    return obj


def main():
    parser=argparse.ArgumentParser(); parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args(sys.argv[sys.argv.index('--')+1:]); args.output.mkdir(parents=True,exist_ok=False)
    bpy.ops.object.select_all(action='SELECT'); bpy.ops.object.delete(use_global=False)
    parchment=material('Scroll parchment',(0.56,.35,.18))
    binding=material('Scroll violet binding',(.32,.10,.30))
    edge=material('Scroll rolled edge',(.32,.19,.09))
    # A short deep recessed end makes the rolled concentric edge legible.
    objects=[tube('Parchment body',-1.30,1.30,.43,parchment),
             tube('Rolled end',1.27,1.34,.32,edge),
             tube('Inner parchment curl',1.32,1.36,.22,parchment),
             tube('Central violet binding',-.20,.04,.452,binding)]
    texture = parchment_texture(args.output/'ExperienceScrollAlbedo.png')
    nodes = parchment.node_tree.nodes
    image_node = nodes.new('ShaderNodeTexImage'); image_node.image = texture
    multiply = nodes.new('ShaderNodeMixRGB'); multiply.blend_type = 'MULTIPLY'
    multiply.inputs[0].default_value = 1
    multiply.inputs[2].default_value = parchment.diffuse_color
    links = parchment.node_tree.links
    links.new(image_node.outputs['Color'], multiply.inputs[1])
    links.new(multiply.outputs['Color'], nodes.get('Principled BSDF').inputs['Base Color'])
    bpy.ops.wm.save_as_mainfile(filepath=str(args.output/'ExperienceScroll.blend'))
    export(args.output/'ExperienceScroll.fbx',objects)
    voxel=voxelize(objects,[parchment,edge,parchment,binding])
    export(args.output/'ExperienceScrollVoxel.fbx',[voxel])


if __name__=='__main__': main()
