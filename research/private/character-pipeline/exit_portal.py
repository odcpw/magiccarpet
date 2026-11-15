"""Source-informed MC2 exit target. Blender: -b --python exit_portal.py -- OUTPUT.

Editable master and two FBX exports use one mesh design. +Z up, -Y faces viewer,
height 1. The original 461 sprite is a static stone face with red slit eyes
and a black open mouth; gameplay owns its visibility and sequence.
"""
import math, sys
from pathlib import Path
import bpy
from mathutils import Vector

out=Path(sys.argv[sys.argv.index('--')+1]).resolve()
out.mkdir(parents=True, exist_ok=False)
bpy.ops.object.select_all(action='SELECT'); bpy.ops.object.delete(use_global=False)

def mat(name, color, roughness=.7, emission=0):
    m=bpy.data.materials.new(name); m.diffuse_color=(*color,1); m.use_nodes=True
    bs=m.node_tree.nodes.get('Principled BSDF'); bs.inputs['Base Color'].default_value=(*color,1); bs.inputs['Roughness'].default_value=roughness
    if emission:
        bs.inputs['Emission Color'].default_value=(*color,1); bs.inputs['Emission Strength'].default_value=emission
    return m
stone=mat('Blue basalt',(.105,.16,.19),.88); dark=mat('Mouth darkness',(.004,.003,.003),1); eye=mat('Ember eyes',(.75,.018,.009),.4,1.2); trim=mat('Weathered bronze',(.21,.14,.075),.8)
objects=[]
def mesh(name, verts, faces, material, smooth=False):
    data=bpy.data.meshes.new(name); data.from_pydata(verts,[],faces); data.update(); ob=bpy.data.objects.new(name,data); bpy.context.collection.objects.link(ob); ob.data.materials.append(material)
    for poly in data.polygons: poly.use_smooth=smooth
    objects.append(ob); return ob

def superellipse(a,b,t,n):
    c,s=math.cos(t),math.sin(t)
    return a*math.copysign(abs(c)**(2/n),c), b*math.copysign(abs(s)**(2/n),s)
# Ring around the open mouth; the massive upper cranium is extended above it.
N=96; rings=[]
for a,b,center,depth,n in [(.295,.295,.407,-.075,3.5),(.315,.32,.405,-.14,3.5),(.375,.38,.46,-.08,2.8),(.35,.39,.51,.045,2.8),(.295,.295,.407,.045,3.5)]:
    ring=[]
    for i in range(N):
        t=2*math.pi*i/N; x,z=superellipse(a,b,t,n); ring.append((x,depth,center+z))
    rings.append(ring)
verts=sum(rings,[]); faces=[]
for r in range(len(rings)):
    nxt=(r+1)%len(rings)
    for i in range(N): faces.append((r*N+i,r*N+(i+1)%N,nxt*N+(i+1)%N,nxt*N+i))
mesh('Carved stone face',verts,faces,stone,True)
# Black mouth sits behind the carved inner rim.
verts=[(0,-.055,.407)]+[(x,-.055,z) for x,_,z in rings[0]]
faces=[(0,1+i,1+(i+1)%N) for i in range(N)]; mesh('Deep mouth',verts,faces,dark)

def ellipsoid(name,loc,scale,material,rot=0):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=32,ring_count=16,location=loc); ob=bpy.context.object; ob.name=name; ob.scale=scale; ob.rotation_euler[1]=rot; bpy.ops.object.transform_apply(location=False,rotation=False,scale=True); ob.data.materials.append(material)
    for p in ob.data.polygons: p.use_smooth=True
    objects.append(ob)
# Sculpted forehead and bridge retain the source's strong horizontal brow.
ellipsoid('Cranium',(0,-.05,.835),(.35,.13,.165),stone)
for side in (-1,1):
    ellipsoid('Red eye '+str(side),(side*.175,-.184,.805),(.137,.025,.023),eye,side*.07)
    ellipsoid('Brow '+str(side),(side*.175,-.19,.845),(.16,.047,.035),stone,-side*.10)
    ellipsoid('Lower eyelid '+str(side),(side*.175,-.17,.772),(.15,.035,.022),trim,side*.04)
ellipsoid('Nose',(0,-.193,.735),(.07,.045,.03),trim)
ellipsoid('Lower lip',(0,-.13,.105),(.20,.047,.027),trim)
# Keep object transforms in the master for editing; FBX export bakes them.
bpy.ops.wm.save_as_mainfile(filepath=str(out/'ExitPortal.blend'))
def export(path, selected):
    bpy.ops.object.select_all(action='DESELECT')
    for ob in selected: ob.select_set(True)
    bpy.context.view_layer.objects.active=selected[0]
    bpy.ops.export_scene.fbx(filepath=str(path),use_selection=True,object_types={'MESH'},axis_forward='-Z',axis_up='Y',apply_unit_scale=True,add_leaf_bones=False,bake_anim=False,path_mode='AUTO')
export(out/'ExitPortal.fbx',objects)
# Voxel derivative: block remesh of each master part. Use separate copies so the
# editable original remains intact; no gameplay-time per-voxel objects.
voxel=[]
for src in objects:
    ob=src.copy(); ob.data=src.data.copy(); bpy.context.collection.objects.link(ob); ob.name='Voxel '+src.name
    # The mouth is a flat black backing, not solid stone. Block-remeshing an
    # open plane collapses its coverage; retain the master's exact disk.
    if src.name != 'Deep mouth':
        mod=ob.modifiers.new('Block voxel derivative','REMESH'); mod.mode='BLOCKS'; mod.octree_depth=6; mod.use_remove_disconnected=False
        bpy.context.view_layer.objects.active=ob; bpy.ops.object.select_all(action='DESELECT'); ob.select_set(True); bpy.ops.object.modifier_apply(modifier=mod.name)
    for p in ob.data.polygons: p.use_smooth=False
    voxel.append(ob)
export(out/'ExitPortalVoxel.fbx',voxel)
# Single preview, orthographic frontal view.
for ob in voxel: ob.hide_render=True
world=bpy.context.scene.world; world.color=(.09,.09,.09)
bpy.ops.object.light_add(type='AREA',location=(0,-3,3)); bpy.context.object.data.energy=500; bpy.context.object.data.shape='DISK'; bpy.context.object.data.size=4
bpy.ops.object.camera_add(location=(0,-4,.5),rotation=(math.pi/2,0,0)); cam=bpy.context.object; cam.data.type='ORTHO'; cam.data.ortho_scale=1.25; bpy.context.scene.camera=cam
scene=bpy.context.scene; scene.render.engine='CYCLES'; scene.cycles.samples=32; scene.render.resolution_x=640; scene.render.resolution_y=640; scene.render.resolution_percentage=100; scene.render.filepath=str(out/'preview.png')
if '--no-render' not in sys.argv: bpy.ops.render.render(write_still=True)
print('EXIT_PORTAL_MESHES',[(o.name,len(o.data.vertices),len(o.data.polygons)) for o in objects]); print('EXIT_PORTAL_VOXELS',[(o.name,len(o.data.vertices),len(o.data.polygons)) for o in voxel])
