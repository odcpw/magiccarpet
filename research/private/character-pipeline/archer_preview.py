"""Render selected rigged character poses with bounded studio lighting."""
import argparse, math, sys
from pathlib import Path
import bpy
from mathutils import Vector
p=argparse.ArgumentParser();p.add_argument('--model',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--frames',type=int,nargs='+',required=True);p.add_argument('--views',type=int,nargs='+',default=[0,90]);p.add_argument('--overlay-bind',action='store_true');p.add_argument('--hand',choices=['Left','Right']);a=p.parse_args(sys.argv[sys.argv.index('--')+1:])
a.output.mkdir(parents=True,exist_ok=True)
bpy.ops.wm.read_factory_settings(use_empty=True);bpy.ops.import_scene.gltf(filepath=str(a.model.resolve()))
for o in list(bpy.context.scene.objects):
 if o.type=='MESH' and o.name=='Icosphere': bpy.data.objects.remove(o,do_unlink=True)
scene=bpy.context.scene
markers=[]
if a.overlay_bind:
 rig=next(o for o in scene.objects if o.type=='ARMATURE')
 rig.data.pose_position='REST'
 scene.frame_set(a.frames[0]);bpy.context.view_layer.update()
 marker=bpy.data.materials.new('Bone overlay green');marker.diffuse_color=(.08,1,.08,1);marker.use_nodes=True
 marker.node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value=(.08,1,.08,1)
 chain=[('LeftShoulder','LeftArm'),('LeftArm','LeftForeArm'),('LeftForeArm','LeftHand'),('RightShoulder','RightArm'),('RightArm','RightForeArm'),('RightForeArm','RightHand'),('Hips','LeftUpLeg'),('LeftUpLeg','LeftLeg'),('LeftLeg','LeftFoot'),('Hips','RightUpLeg'),('RightUpLeg','RightLeg'),('RightLeg','RightFoot')]
 for start,end in chain:
  v=rig.matrix_world@rig.data.bones[start].head_local;w=rig.matrix_world@rig.data.bones[end].head_local
  bpy.ops.mesh.primitive_uv_sphere_add(segments=12,ring_count=8,radius=.018,location=v);bpy.context.object.data.materials.append(marker);markers.append((bpy.context.object,bpy.context.object.location.copy()))
  dist=w-v;bpy.ops.mesh.primitive_cylinder_add(vertices=8,radius=.006,depth=dist.length,location=(v+w)/2);c=bpy.context.object;c.rotation_euler=dist.to_track_quat('Z','Y').to_euler();c.data.materials.append(marker);markers.append((c,c.location.copy()))
scene.render.engine='BLENDER_EEVEE_NEXT';scene.render.resolution_x=640;scene.render.resolution_y=800;scene.render.resolution_percentage=100;scene.render.image_settings.file_format='PNG';scene.world=bpy.data.worlds.new('Studio');scene.world.use_nodes=True;scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.16,.18,.22,1);scene.world.node_tree.nodes['Background'].inputs[1].default_value=.7
center=Vector((.05,0,.95));size=1.9
if a.hand:
 scene.frame_set(a.frames[0]);bpy.context.view_layer.update();rig=next(o for o in scene.objects if o.type=='ARMATURE');w=rig.matrix_world@rig.pose.bones[a.hand+'Hand'].head;e=rig.matrix_world@rig.pose.bones[a.hand+'ForeArm'].head;center=w+(w-e).normalized()*.09
bpy.ops.mesh.primitive_plane_add(size=100,location=(0,0,0));floor=bpy.context.object;floor.name='Review floor';mat=bpy.data.materials.new('Floor');mat.diffuse_color=(.13,.15,.18,1);floor.data.materials.append(mat)
for name,position,power,width in [('Key',(3,-4,6),900,4),('Fill',(-4,-2,3),450,5),('Rim',(2,4,5),750,3)]:
 data=bpy.data.lights.new(name,'AREA');data.energy=power;data.shape='DISK';data.size=width;o=bpy.data.objects.new(name,data);scene.collection.objects.link(o);o.location=position;o.rotation_euler=(center-o.location).to_track_quat('-Z','Y').to_euler()
bpy.ops.object.camera_add();cam=bpy.context.object;scene.camera=cam;cam.data.type='ORTHO';cam.data.ortho_scale=.52 if a.hand else 2.32
for angle in a.views:
 r=math.radians(angle);cam.location=center+Vector((math.sin(r)*5,-math.cos(r)*5,1.0));cam.rotation_euler=(center-cam.location).to_track_quat('-Z','Y').to_euler()
 for marker, base in markers: marker.location=base+(cam.location-center).normalized()*.8
 for frame in a.frames:
  scene.frame_set(frame);scene.render.filepath=str((a.output/f'view-{angle:03d}-frame-{frame:03d}.png').resolve());bpy.ops.render.render(write_still=True)
print('PREVIEW_OK',flush=True)
