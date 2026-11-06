"""Generate a recurve wooden bow with a drawable string and removable arrow.

Run: blender -b -P archer_bow.py -- --output DIR
Bow grip is at the origin, limbs follow Z, arrow flight is local +Y. The
Bowstring uses Draw, TipBack and TipClose shape keys. Dimensions in metres
are provisional and must be fitted to the generated character's hand and rig.
"""
import argparse
import math
import sys
from pathlib import Path

import bpy


def mat(name, color, metallic=0, roughness=.55):
    m = bpy.data.materials.new(name)
    m.diffuse_color = (*color, 1)
    m.use_nodes = True
    bsdf = m.node_tree.nodes['Principled BSDF']
    bsdf.inputs['Base Color'].default_value = (*color, 1)
    bsdf.inputs['Metallic'].default_value = metallic
    bsdf.inputs['Roughness'].default_value = roughness
    return m


def tube(name, points, radius, material):
    curve = bpy.data.curves.new(name, 'CURVE')
    curve.dimensions = '3D'
    curve.resolution_u = 24
    curve.bevel_depth = radius
    curve.bevel_resolution = 4
    spline = curve.splines.new('POLY')
    spline.points.add(len(points)-1)
    for point, coordinate in zip(spline.points, points):
        point.co = (*coordinate, 1)
    obj = bpy.data.objects.new(name, curve)
    bpy.context.collection.objects.link(obj)
    obj.data.materials.append(material)
    return obj


def bow_limb(side, material):
    vertices, faces = [], []
    count = 24
    for i in range(count+1):
        t = i / count
        z = side * (.08 + .50*t)
        y = -.13 * math.sin(math.pi*t/2) + .035 * t**6
        half_width = .021 * (1 - .68*t)
        half_depth = .013 * (1 - .62*t)
        vertices += [(-half_width,y-half_depth,z), (half_width,y-half_depth,z),
                     (half_width,y+half_depth,z), (-half_width,y+half_depth,z)]
    for i in range(count):
        for j in range(4):
            faces.append((4*i+j,4*i+(j+1)%4,4*(i+1)+(j+1)%4,4*(i+1)+j))
    faces += [(3,2,1,0),tuple(range(4*count,4*count+4))]
    mesh=bpy.data.meshes.new('Recurved tapered wooden limb')
    mesh.from_pydata(vertices,[],faces)
    mesh.update()
    obj=bpy.data.objects.new('Bow limb upper' if side>0 else 'Bow limb lower',mesh)
    bpy.context.collection.objects.link(obj)
    mesh.materials.append(material)
    obj.shape_key_add(name='Basis')
    back=obj.shape_key_add(name='TipBack')
    close=obj.shape_key_add(name='TipClose')
    for i in range(count+1):
        for j in range(4):
            t=i/count
            curve=t*t*(3-2*t)
            back.data[4*i+j].co.y -= .5*curve
            close.data[4*i+j].co.z -= side*.58*i/count
    bevel=obj.modifiers.new('Softened wood edges','BEVEL')
    bevel.width=.004
    bevel.segments=3
    obj.modifiers.new('Wood normals','WEIGHTED_NORMAL')
    return obj


def string_mesh(material):
    # Rings remain anchored at the limb tips; the nock moves toward -Y.
    count=40
    ring=6
    vertices=[]
    for i in range(count+1):
        z=-.58+1.16*i/count
        for j in range(ring):
            angle=2*math.pi*j/ring
            vertices.append((.0024*math.cos(angle),-.095+.0024*math.sin(angle),z))
    faces=[]
    for i in range(count):
        for j in range(ring):
            faces.append((i*ring+j,i*ring+(j+1)%ring,(i+1)*ring+(j+1)%ring,(i+1)*ring+j))
    mesh=bpy.data.meshes.new('Bowstring mesh')
    mesh.from_pydata(vertices,[],faces)
    mesh.update()
    obj=bpy.data.objects.new('Bowstring',mesh)
    bpy.context.collection.objects.link(obj)
    mesh.materials.append(material)
    obj.shape_key_add(name='Basis')
    draw=obj.shape_key_add(name='Draw')
    back=obj.shape_key_add(name='TipBack')
    close=obj.shape_key_add(name='TipClose')
    nock_x=obj.shape_key_add(name='NockX')
    nock_z=obj.shape_key_add(name='NockZ')
    nock_x.slider_min=nock_z.slider_min=-3
    nock_x.slider_max=nock_z.slider_max=3
    for i in range(count+1):
        weight=1-abs(2*i/count-1)
        for j in range(ring):
            index=i*ring+j
            draw.data[index].co.y -= weight
            back.data[index].co.y -= .5*(1-weight)
            close.data[index].co.z -= math.copysign(.58*(1-weight), -1+2*i/count) if i != count//2 else 0
            nock_x.data[index].co.x += .2*weight
            nock_z.data[index].co.z += .2*weight
    return obj


def main():
    p=argparse.ArgumentParser()
    p.add_argument('--output',type=Path,required=True)
    args=p.parse_args(sys.argv[sys.argv.index('--')+1:])
    args.output.mkdir(parents=True,exist_ok=True)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    wood=mat('Warm polished bow wood',(.26,.105,.035),roughness=.52)
    grip=mat('Worn brown leather grip',(.12,.048,.02),roughness=.82)
    cord=mat('Pale flax bowstring',(.55,.47,.33),roughness=.84)
    dark=mat('Dark horn tips',(.045,.034,.027),roughness=.44)
    iron=mat('Forged arrowhead',(.16,.16,.14),metallic=.72,roughness=.35)
    feather=mat('Arrow feathers',(.68,.63,.51),roughness=.8)
    objects=[bow_limb(-1,wood),bow_limb(1,wood),string_mesh(cord)]
    objects.append(tube('Grip core',[(0,0,-.09),(0,0,.09)],.021,grip))
    for i in range(13):
        z=-.078+.013*i
        objects.append(tube('Grip binding',[(.021*math.cos(a),.021*math.sin(a),z+.002*a/(2*math.pi)) for a in [2*math.pi*j/20 for j in range(21)]],.0017,cord))
    for side in (-1,1):
        objects.append(tube('Horn bow tip',[(0,-.094,side*.55),(0,-.095,side*.583)],.006,dark))
    # Independent nocked arrow, +Y points toward the target.
    objects.append(tube('Arrow shaft',[(0,-.11,.008),(0,.90,.008)],.0043,wood))
    bpy.ops.mesh.primitive_cone_add(vertices=8,radius1=.011,radius2=0,depth=.043, location=(0,.918,.008),rotation=(-math.pi/2,0,0))
    tip=bpy.context.object
    tip.name='Arrowhead'
    tip.data.materials.append(iron)
    objects.append(tip)
    for side in (-1,1):
        mesh=bpy.data.meshes.new('Filled feather');mesh.from_pydata([(0,-.098,.008),(side*.018,-.075,.008),(side*.018,-.015,.008),(0,.005,.008)],[],[(0,1,2,3)]);mesh.update()
        obj=bpy.data.objects.new('Arrow fletching',mesh);bpy.context.collection.objects.link(obj);mesh.materials.append(feather);objects.append(obj)
    bpy.ops.object.select_all(action='DESELECT')
    for obj in objects: obj.select_set(True)
    bpy.context.view_layer.objects.active=objects[0]
    bpy.ops.wm.save_as_mainfile(filepath=str((args.output/'bow.blend').resolve()))
    bpy.ops.export_scene.gltf(filepath=str((args.output/'bow.glb').resolve()),export_format='GLB',use_selection=True,export_morph=True)
    print('BOW_OK',len(objects),'objects',flush=True)


if __name__=='__main__': main()
