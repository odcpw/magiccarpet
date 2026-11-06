"""Build a one-second archer shot from the aligned Meshy rig and a bow.

This is a candidate motion: the source game has static standing/kneeling attack
sprites, so the transition and the separate nock/release are authored.
"""
import argparse,json,math,struct,sys
from pathlib import Path
import bpy
from mathutils import Matrix,Vector,Quaternion


def stash(owner,name):
    anim=owner.animation_data
    if not anim or not anim.action: return
    action=anim.action
    track=anim.nla_tracks.new();track.name=name
    strip=track.strips.new(name,1,action)
    if hasattr(strip,'action_slot') and anim.action_slot: strip.action_slot=anim.action_slot
    anim.action=None


def key(obj,path,frame): obj.keyframe_insert(data_path=path,frame=frame)


def set_arrow_scale_step(path):
    # Blender's NLA exporter bakes constant object keys as LINEAR. Keep the
    # short re-nock invisible between frames in the actual glTF consumer.
    raw=path.read_bytes()
    magic,version,total=struct.unpack_from('<4sII',raw,0)
    if magic!=b'glTF' or version!=2 or total!=len(raw): raise ValueError('Invalid GLB header')
    length,kind=struct.unpack_from('<II',raw,12)
    if kind!=0x4E4F534A: raise ValueError('Missing GLB JSON chunk')
    data=json.loads(raw[20:20+length]);changed=0
    for animation in data.get('animations',[]):
        channels=animation['channels'];samplers=animation['samplers']
        for channel in channels:
            target=channel['target'];node=data['nodes'][target['node']]
            if node.get('name')=='ArrowControl' and target['path']=='scale':
                index=channel['sampler']
                if sum(c['sampler']==index for c in channels)>1:
                    samplers.append(dict(samplers[index]));index=len(samplers)-1;channel['sampler']=index
                samplers[index]['interpolation']='STEP';changed+=1
    if changed!=1: raise ValueError(f'Expected one arrow scale channel, got {changed}')
    encoded=json.dumps(data,separators=(',',':')).encode('utf-8')
    encoded+=b' '*((-len(encoded))%4)
    remainder=raw[20+length:]
    result=struct.pack('<4sII',magic,version,20+len(encoded)+len(remainder))+struct.pack('<II',len(encoded),kind)+encoded+remainder
    path.write_bytes(result)




def solve_arm(rig, upper_name, fore_name, hand_name, target, pole=None):
    def point(name): return rig.matrix_world @ rig.pose.bones[name].head
    a,b,c=(point(n) for n in (upper_name,fore_name,hand_name))
    length1=(b-a).length; length2=(c-b).length
    direction=target-a; distance=direction.length
    distance=max(abs(length1-length2)+1e-5,min(distance,length1+length2-1e-5))
    unit=direction.normalized(); along=(length1*length1-length2*length2+distance*distance)/(2*distance)
    height=math.sqrt(max(0,length1*length1-along*along))
    # Retain the original elbow plane with a mild downward bias.
    bend=(pole if pole is not None else b-a+Vector((0,0,-.035)))
    bend-=unit*bend.dot(unit)
    if bend.length<1e-5: bend=unit.cross(Vector((0,1,0)))
    elbow=a+unit*along+bend.normalized()*height
    def rotate(name, current, desired):
        bone=rig.pose.bones[name];rot=current.rotation_difference(desired)
        basis=rig.matrix_world.to_quaternion();rot=basis.inverted()@rot@basis
        head=bone.matrix.translation
        bone.matrix=Matrix.Translation(head)@rot.to_matrix().to_4x4()@Matrix.Translation(-head)@bone.matrix
        bpy.context.view_layer.update()
    rotate(upper_name,b-a,elbow-a)
    b=point(fore_name);c=point(hand_name)
    rotate(fore_name,c-b,target-b)


def rotate_bone_world(rig,name,rotation):
    bone=rig.pose.bones[name]
    basis=rig.matrix_world.to_quaternion()
    rotation=basis.inverted()@rotation@basis
    head=bone.matrix.translation.copy()
    bone.matrix=Matrix.Translation(head)@rotation.to_matrix().to_4x4()@Matrix.Translation(-head)@bone.matrix
    bpy.context.view_layer.update()


def lower_extended_bow_arm(rig,amount):
    shoulder=rig.matrix_world@rig.pose.bones['LeftArm'].head
    wrist=rig.matrix_world@rig.pose.bones['LeftHand'].head
    current=wrist-shoulder
    desired=current+Vector((0,0,-amount))
    rotate_bone_world(rig,'LeftArm',current.rotation_difference(desired))


def face_target(rig):
    head=rig.matrix_world@rig.pose.bones['Head'].head
    front=rig.matrix_world@rig.pose.bones['headfront'].head
    target=rig.matrix_world@rig.pose.bones['LeftHand'].head
    a=Vector((front.x-head.x,front.y-head.y,0))
    b=Vector((target.x-head.x,target.y-head.y,0))
    if a.length<1e-6 or b.length<1e-6:return
    angle=math.atan2(a.x*b.y-a.y*b.x,a.dot(b))
    angle=max(-.70,min(.70,angle))
    rotate_bone_world(rig,'neck',Quaternion(Vector((0,0,1)),angle*.25))
    rotate_bone_world(rig,'Head',Quaternion(Vector((0,0,1)),angle*.75))


def main():
 p=argparse.ArgumentParser();p.add_argument('--model',type=Path,required=True);p.add_argument('--bow',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--kneel',action='store_true');p.add_argument('--rigid-quiver',action='store_true');p.add_argument('--aim-drop',type=float,default=.07);a=p.parse_args(sys.argv[sys.argv.index('--')+1:]);a.output.mkdir(parents=True,exist_ok=True)
 bpy.ops.wm.read_factory_settings(use_empty=True)
 bpy.ops.import_scene.gltf(filepath=str(a.model.resolve()))
 rig=next(o for o in bpy.context.scene.objects if o.type=='ARMATURE');scene=bpy.context.scene
 sys.path.insert(0,str(Path(__file__).resolve().parent))
 source_mesh=next(o for o in scene.objects if o.type=='MESH' and o.parent==rig)
 if a.rigid_quiver:
  from archer_props import rigid_quiver
  rigid_quiver(source_mesh)
 mesh=next(o for o in scene.objects if o.type=='MESH' and o.parent==rig)
 hand_vertices={}
 for side,lower,upper in [('Left',.10,.14),('Right',.13,.17)]:
  fore=rig.matrix_world@rig.data.bones[side+'ForeArm'].head_local
  wrist=rig.matrix_world@rig.data.bones[side+'Hand'].head_local
  span=(wrist-fore).length;axis=(wrist-fore).normalized();group=mesh.vertex_groups[side+'Hand'];ids=[]
  lower*=span/.237;upper*=span/.237
  for v in mesh.data.vertices:
   w=next((g.weight for g in v.groups if g.group==group.index),0)
   t=(mesh.matrix_world@v.co-wrist).dot(axis)
   if w>.5 and lower<t<upper: ids.append(v.index)
  if len(ids)<5: raise ValueError(f'Insufficient {side} hand vertices for grip')
  hand_vertices[side]=ids
 base_location=rig.location.copy()
 footgroups={g.index for g in source_mesh.vertex_groups if g.name in {'LeftFoot','LeftToeBase','RightFoot','RightToeBase'}}
 foot_ids=[v.index for v in source_mesh.data.vertices if sum(q.weight for q in v.groups if q.group in footgroups)>.5]
 if not foot_ids: raise ValueError('No weighted foot vertices')
 # Capture source pose matrices at a useful held/draw/release span, then bake
 # one second with a short eased return to the initial pose.
 samples=[]
 for frame in range(1,26):
  f=61+(frame-1)*1.5
  rig.location=base_location
  scene.frame_set(int(f),subframe=f%1);bpy.context.view_layer.update()
  if not a.kneel:
   obj=source_mesh.evaluated_get(bpy.context.evaluated_depsgraph_get());data=obj.to_mesh()
   floor=min((obj.matrix_world@data.vertices[i].co).z for i in foot_ids)
   obj.to_mesh_clear();rig.location.z-=floor;bpy.context.view_layer.update()
  right=rig.matrix_world@rig.pose.bones['RightHand'].head
  lower=Vector((0,0,-a.aim_drop))
  lower_extended_bow_arm(rig,a.aim_drop)
  solve_arm(rig,'RightArm','RightForeArm','RightHand',right+lower)
  face_target(rig)
  if a.kneel:
   from archer_kneel import apply_kneel
   apply_kneel(rig,source_mesh)
  samples.append(({b.name:b.matrix_basis.copy() for b in rig.pose.bones},rig.location.copy()))
 for f in range(21,26):
  t=(f-21)/4;t=t*t*(3-2*t)
  bones,loc=samples[f-1];first_bones,first_loc=samples[0]
  for name,m in bones.items():
   l,q,s=m.decompose();l0,q0,s0=first_bones[name].decompose()
   bones[name]=Matrix.LocRotScale(l.lerp(l0,t),q.slerp(q0,t),s.lerp(s0,t))
  samples[f-1]=(bones,loc.lerp(first_loc,t))
 rig.animation_data.action=None
 for track in list(rig.animation_data.nla_tracks): rig.animation_data.nla_tracks.remove(track)
 for obj in list(scene.objects):
  if obj.type=='MESH' and obj.name=='Icosphere': bpy.data.objects.remove(obj,do_unlink=True)
 with bpy.data.libraries.load(str(a.bow.resolve()),link=False) as (source,dest): dest.objects=source.objects
 bow_objects=[obj for obj in dest.objects if obj]
 for obj in bow_objects: scene.collection.objects.link(obj)
 control=bpy.data.objects.new('BowControl',None);scene.collection.objects.link(control);control.rotation_mode='QUATERNION'
 arrow=bpy.data.objects.new('ArrowControl',None);scene.collection.objects.link(arrow);arrow.parent=control
 for obj in bow_objects:
  old=obj.matrix_world.copy();obj.parent=arrow if obj.name.startswith(('Arrow shaft','Arrowhead','Arrow fletching')) else control;obj.matrix_world=old
 strings=next(o for o in bow_objects if o.name=='Bowstring')
 limbs=[o for o in bow_objects if o.name.startswith('Bow limb')]
 tips=[o for o in bow_objects if o.name.startswith('Horn bow tip')]
 scene.render.fps=24;scene.frame_start=1;scene.frame_end=25
 for frame in range(1,26):
  scene.frame_set(frame)
  bones,location=samples[frame-1]
  rig.location=location;rig.keyframe_insert(data_path='location',frame=frame)
  for name,m in bones.items():
   bone=rig.pose.bones[name];bone.rotation_mode='QUATERNION';bone.matrix_basis=m
   for path in ('location','rotation_quaternion','scale'): bone.keyframe_insert(data_path=path,frame=frame,group=name)
  bpy.context.view_layer.update()
  eval_obj=mesh.evaluated_get(bpy.context.evaluated_depsgraph_get());evaluated=eval_obj.to_mesh()
  def hand(side):
   ids=hand_vertices[side]
   return sum((eval_obj.matrix_world@evaluated.vertices[i].co for i in ids),Vector())/len(ids)
  grip=hand('Left');drawhand=hand('Right')
  eval_obj.to_mesh_clear()
  direction=grip-drawhand;forward=direction.normalized();up=Vector((0,0,1));x=forward.cross(up).normalized();z=x.cross(forward).normalized()
  control.location=grip;control.rotation_quaternion=Matrix((x,forward,z)).transposed().to_quaternion()
  key(control,'location',frame);key(control,'rotation_quaternion',frame)
  local=control.rotation_quaternion.inverted()@(drawhand-grip)
  tension=1 if frame<=18 else (max(0,(20-frame)/2) if frame<=20 else (max(0,(frame-23)/2) if frame>=24 else 0))
  draw=max(0,min(1,(-local.y-.095)))*tension
  zval=max(-3,min(3,local.z/.2))*tension
  xval=max(-3,min(3,local.x/.2))*tension
  close=(.58-math.sqrt(max(0,.58**2-(.5*draw)**2)))/.58
  for obj in [strings,*limbs]:
   shapes=obj.data.shape_keys.key_blocks
   for name,val in [('TipBack',draw),('TipClose',close),('Draw',draw),('NockX',xval),('NockZ',zval)]:
    if name in shapes:
     shapes[name].value=val;shapes[name].keyframe_insert(data_path='value',frame=frame)
  for tip in tips:
   tip.location.y=-.5*draw
   tip.location.z=(-.58 if tip.name.endswith('.001') else .58)*close
   key(tip,'location',frame)
  arrow.location=(.2*xval,-.095-draw+.11,.2*zval)
  if 19<=frame<=22: arrow.location.y+=(frame-18)*.3
  arrow.scale=(0.001,)*3 if 21<=frame<=24 else (1,)*3
  key(arrow,'location',frame);key(arrow,'scale',frame)
 # Keep the re-nock visibility discrete; do not fade a detached arrow in.
 if arrow.animation_data and arrow.animation_data.action:
  for layer in arrow.animation_data.action.layers:
   for strip in layer.strips:
    for bag in strip.channelbags:
     for curve in bag.fcurves:
      if curve.data_path=='scale':
       for point in curve.keyframe_points: point.interpolation='CONSTANT'
 # Merge each body's, prop's and morph's animation into the same glTF clip.
 name='Archer_Kneeling_Fire' if a.kneel else 'Archer_Standing_Fire'
 stem='kneel' if a.kneel else 'standing'
 for obj in [rig,control,arrow,*tips]: stash(obj,name)
 for obj in [strings,*limbs]: stash(obj.data.shape_keys,name)
 bpy.context.view_layer.update()
 bpy.ops.wm.save_as_mainfile(filepath=str((a.output/(stem+'.blend')).resolve()))
 bpy.ops.export_scene.gltf(filepath=str((a.output/(stem+'.glb')).resolve()),export_format='GLB',export_animation_mode='NLA_TRACKS',export_morph=True)
 set_arrow_scale_step(a.output/(stem+'.glb'))
 print('SHOT_OK frames 1..25 at 24fps release near frame19',flush=True)

if __name__=='__main__': main()
