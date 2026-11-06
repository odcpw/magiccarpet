"""Build a low-held-bow walk from aligned Meshy walking motion."""
import argparse,sys
from pathlib import Path
import bpy
from mathutils import Matrix,Vector
sys.path.insert(0,str(Path(__file__).resolve().parent))
from archer_shot import solve_arm,stash
from archer_props import rigid_quiver

def main():
 p=argparse.ArgumentParser();p.add_argument('--model',type=Path,required=True);p.add_argument('--bow',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args(sys.argv[sys.argv.index('--')+1:]);a.output.mkdir(parents=True,exist_ok=True)
 bpy.ops.wm.read_factory_settings(use_empty=True);bpy.ops.import_scene.gltf(filepath=str(a.model.resolve()));scene=bpy.context.scene;rig=next(o for o in scene.objects if o.type=='ARMATURE');mesh=next(o for o in scene.objects if o.type=='MESH' and o.parent==rig)
 rigid_quiver(mesh)
 if not rig.animation_data or not rig.animation_data.action:raise ValueError('Missing source walk action')
 source_start,source_end=rig.animation_data.action.frame_range
 samples=[]
 for frame in range(1,98):
  # Resample the provider's complete cycle to one second at 24 fps.
  source_frame=source_start+(frame-1)*(source_end-source_start)/96
  scene.frame_set(int(source_frame),subframe=source_frame%1);bpy.context.view_layer.update();hips=rig.matrix_world@rig.pose.bones['Hips'].head
  target=hips+Vector((.30,-.04,-.14))
  solve_arm(rig,'LeftArm','LeftForeArm','LeftHand',target)
  samples.append({b.name:b.matrix_basis.copy() for b in rig.pose.bones})
 rig.animation_data.action=None
 for track in list(rig.animation_data.nla_tracks):rig.animation_data.nla_tracks.remove(track)
 for o in list(scene.objects):
  if o.type=='MESH' and o.name=='Icosphere':bpy.data.objects.remove(o,do_unlink=True)
 # The grip is derived from the posed fingers rather than extrapolating from
 # the wrist, so the visible hand stays aligned with the bow handle.
 fore=rig.matrix_world@rig.data.bones['LeftForeArm'].head_local
 wrist=rig.matrix_world@rig.data.bones['LeftHand'].head_local
 axis=(wrist-fore).normalized();group=mesh.vertex_groups['LeftHand']
 grip_ids=[]
 for v in mesh.data.vertices:
  w=next((g.weight for g in v.groups if g.group==group.index),0)
  t=(mesh.matrix_world@v.co-wrist).dot(axis)
  if w>.5 and .10<t<.14:grip_ids.append(v.index)
 if len(grip_ids)<5:raise ValueError('Insufficient left-hand grip vertices')
 with bpy.data.libraries.load(str(a.bow.resolve()),link=False) as (src,dst):dst.objects=src.objects
 objects=[o for o in dst.objects if o and not o.name.startswith(('Arrow shaft','Arrowhead','Arrow fletching'))]
 for o in objects:scene.collection.objects.link(o)
 control=bpy.data.objects.new('BowControl',None);scene.collection.objects.link(control);control.rotation_mode='QUATERNION'
 for o in objects:
  old=o.matrix_world.copy();o.parent=control;o.matrix_world=old
 # Export a one-second 96 Hz curve so the soles remain grounded between
 # ordinary 24 fps presentation frames as well as at those frames.
 scene.render.fps=96;scene.frame_start=1;scene.frame_end=97
 for f in range(1,98):
  scene.frame_set(f)
  for n,m in samples[f-1].items():
   b=rig.pose.bones[n];b.rotation_mode='QUATERNION';b.matrix_basis=m
  bpy.context.view_layer.update()
  # Source walking feet hover above the floor. Lower the root by the actual
  # evaluated sole height, preserving all leg rotations and source stride.
  deps=bpy.context.evaluated_depsgraph_get();evaluated_obj=mesh.evaluated_get(deps);evaluated=evaluated_obj.to_mesh()
  bottom=min((evaluated_obj.matrix_world@v.co).z for v in evaluated.vertices)
  evaluated_obj.to_mesh_clear()
  hips=rig.pose.bones['Hips'];basis=rig.matrix_world.to_3x3()@hips.bone.matrix_local.to_3x3()
  hips.location+=basis.inverted()@Vector((0,0,-bottom+.001))
  bpy.context.view_layer.update()
  for n in samples[f-1]:
   b=rig.pose.bones[n]
   for path in ('location','rotation_quaternion','scale'):b.keyframe_insert(data_path=path,frame=f,group=n)
  evaluated_obj=mesh.evaluated_get(deps);evaluated=evaluated_obj.to_mesh()
  grip=sum((evaluated_obj.matrix_world@evaluated.vertices[i].co for i in grip_ids),Vector())/len(grip_ids)
  evaluated_obj.to_mesh_clear()
  # Carry in the sagittal plane outside the left hip; it does not cross the legs.
  z=Vector((0,.40,.9165)).normalized();forward=Vector((0,z.z,-z.y));x=forward.cross(z).normalized()
  control.location=grip;control.rotation_quaternion=Matrix((x,forward,z)).transposed().to_quaternion();control.keyframe_insert('location',frame=f);control.keyframe_insert('rotation_quaternion',frame=f)
 name='Archer_Walk';stash(rig,name);stash(control,name)
 # Place the one-second NLA strip at 1/24 s, so Blender's glTF reimport at
 # its default 24 fps exposes the familiar frames 1..25, not 0.25..24.25.
 for owner in (rig,control):
  strip=owner.animation_data.nla_tracks[-1].strips[0]
  strip.frame_start=4;strip.frame_end=100
 scene.frame_start=4;scene.frame_end=100
 bpy.ops.wm.save_as_mainfile(filepath=str((a.output/'walk.blend').resolve()));bpy.ops.export_scene.gltf(filepath=str((a.output/'walk.glb').resolve()),export_format='GLB',export_animation_mode='NLA_TRACKS',export_morph=True)
 print('WALK_OK',flush=True)
if __name__=='__main__':main()
