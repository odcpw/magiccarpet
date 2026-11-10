"""Create actions for the matched MPFB human and its separate robe.

Run Blender 4.5.12 in background with the source .blend open, then this script.
No source file is overwritten. The procedural gait is not claimed as mocap.
"""
import argparse
import json
import math
import sys
from pathlib import Path

import bpy
from mathutils import Quaternion, Vector

parser=argparse.ArgumentParser()
parser.add_argument('--output',type=Path,required=True)
OUT=parser.parse_args(sys.argv[sys.argv.index('--')+1:]).output.resolve()
OUT.mkdir(parents=True,exist_ok=False)
scene=bpy.context.scene
source_path=bpy.data.filepath
rig=bpy.data.objects['VillagerRig'];body=bpy.data.objects['Human'];robe=bpy.data.objects['Human.donitz_monk_robe']
if body.parent!=rig or robe.parent!=rig or len(body.data.vertices)!=19158:
    raise RuntimeError('Unexpected source body or rig')
if tuple(round(x,6) for x in rig.scale)!=(1,1,1):
    raise RuntimeError('Source rig is not unit scale')
# Unity LBS does not reproduce Blender's segmented B-bone skinning. Preserve
# this file's original input and constrain weighted deform bones in the output.
weighted_names={g.name for obj in (body,robe) for g in obj.vertex_groups}
collapsed_bones={b.name:b.bbone_segments for b in rig.data.bones if b.name in weighted_names and b.bbone_segments>1}
for name in collapsed_bones:rig.data.bones[name].bbone_segments=1
# Unity's target mesh allows four deform influences. Prune by weight and
# normalize only deform groups; retain all unrelated masks unchanged.
deform_groups={g.index:g for g in body.vertex_groups if g.name in rig.data.bones and rig.data.bones[g.name].use_deform}
pruned_vertices=0;unweighted_raw=[]
for vertex in body.data.vertices:
    influences=sorted(((x.group,x.weight) for x in vertex.groups if x.group in deform_groups and x.weight>0),key=lambda x:x[1],reverse=True)
    if not influences:
        unweighted_raw.append(vertex.index)
        continue
    if len(influences)>4:pruned_vertices+=1
    kept=influences[:4];total=sum(w for _,w in kept)
    for group,_ in influences[4:]:deform_groups[group].remove([vertex.index])
    for group,weight in kept:deform_groups[group].add([vertex.index],weight/total,'REPLACE')
# Rigify's IK/FK and constraint drivers live in animation_data as well as the
# action. Clearing animation_data destroys the drivers and freezes the skin.
if rig.animation_data: rig.animation_data.action=None
rig.data.pose_position='POSE'
for side in ('L','R'):
    for limb in ('thigh_parent','upper_arm_parent'):
        rig.pose.bones[f'{limb}.{side}']['IK_FK']=1.0
for o in bpy.data.objects:
    if o not in (body,robe,rig):o.hide_render=True
for mod in body.modifiers:
    if mod.type=='PARTICLE_SYSTEM':mod.show_render=False;mod.show_viewport=False
    if mod.name in ('charmorph_rig_pv','charmorph_corrective_smooth'):
        mod.show_render=False;mod.show_viewport=False
bpy.context.view_layer.update()
# Body masks remove helpers and robe-covered skin, changing evaluated indices.
# Select evaluated foot-weighted vertices per sample rather than indexing raw.
foot_groups={g.index for g in body.vertex_groups if g.name.startswith(('DEF-foot.','DEF-toe'))}
deps=bpy.context.evaluated_depsgraph_get();ev=body.evaluated_get(deps)
visible=ev.to_mesh(preserve_all_data_layers=True,depsgraph=deps)
try:
    visible_weights=[sum(x.weight for x in v.groups if x.group in deform_groups and x.weight>0) for v in visible.vertices]
    if not visible_weights or min(visible_weights)<.9999 or max(visible_weights)>1.0001:
        raise RuntimeError('Visible masked body has unnormalized or missing deform weights')
    foot_count=sum(any(x.group in foot_groups and x.weight>.05 for x in v.groups) for v in visible.vertices)
    bind_min=min((ev.matrix_world@v.co).z for v in visible.vertices)
    if foot_count<100 or abs(bind_min)>.003:raise RuntimeError('Source feet or bind ground are unexpected')
finally:ev.to_mesh_clear()

def rot_world(name,angle,adduction=0,compensate_parent=False):
    p=rig.pose.bones[name]; p.rotation_mode='QUATERNION'
    q=p.bone.matrix_local.to_quaternion()
    d=Quaternion()
    if compensate_parent and p.parent:
        d=(p.parent.matrix.to_3x3() @ p.parent.bone.matrix_local.to_3x3().inverted()).to_quaternion()
    p.rotation_quaternion=q.inverted() @ d.inverted() @ Quaternion((1,0,0),angle) @ Quaternion((0,1,0),adduction) @ d @ q
    p.keyframe_insert(data_path='rotation_quaternion',group=name)

def clear_pose():
    for p in rig.pose.bones:
        p.location=(0,0,0);p.rotation_mode='QUATERNION';p.rotation_quaternion=(1,0,0,0);p.scale=(1,1,1)
    bpy.context.view_layer.update()

def min_sole():
    deps=bpy.context.evaluated_depsgraph_get();ev=body.evaluated_get(deps);mesh=ev.to_mesh(preserve_all_data_layers=True,depsgraph=deps)
    try:return min((ev.matrix_world@v.co).z for v in mesh.vertices if any(x.group in foot_groups and x.weight>.05 for x in v.groups))
    finally:ev.to_mesh_clear()

def min_body():
    deps=bpy.context.evaluated_depsgraph_get();ev=body.evaluated_get(deps);mesh=ev.to_mesh()
    try:return min((ev.matrix_world@v.co).z for v in mesh.vertices)
    finally:ev.to_mesh_clear()

def relax_fingers(frame):
    for side in ('L','R'):
        for finger in ('f_index','f_middle','f_ring','f_pinky'):
            for segment in ('01','02','03'):
                p=rig.pose.bones[f'{finger}.{segment}.{side}']
                p.rotation_mode='XYZ';p.rotation_euler.x=.25
                p.keyframe_insert(data_path='rotation_euler',frame=frame,group=p.name)

def relaxed_arms(swing=0,elbow=.20):
    for side,sign in [('L',1),('R',-1)]:
        rot_world('upper_arm_fk.'+side,sign*swing,sign*.55)
        bpy.context.view_layer.update()
        rot_world('forearm_fk.'+side,elbow,compensate_parent=True)

# Angles are world-X swings through each control's own rest basis. The source
# uses ordinary LBS with its second volume-preserving modifier disabled.
# Gait is 120-Hz sampled, with periodic endpoints and no forward translation.
clips={}
for kind,steps,hip_amp,knee_amp,arm_amp in [('Walk',120,.43,.58,.30),('Run',76,.58,1.05,.55)]:
    rig.animation_data.action=None;clear_pose();act=bpy.data.actions.new(kind);act.use_fake_user=True;rig.animation_data_create();rig.animation_data.action=act
    for i in range(steps+1):
        frame=i+1;scene.frame_set(frame)
        for side,sign in [('L',1),('R',-1)]:
            t=(i/steps+(0 if side=='L' else .5))%1
            stance=.50 if kind=='Walk' else .40
            if t<stance:
                # Plant at forward extent and move the planted sole steadily
                # backwards relative to the advancing character.
                hip=-hip_amp+2*hip_amp*t/stance;bend=0
            else:
                u=(t-stance)/(1-stance)
                hip=hip_amp*math.cos(math.pi*u)
                bend=knee_amp*math.sin(math.pi*u)
            rot_world('thigh_fk.'+side,hip)
            rot_world('shin_fk.'+side,bend)
            rot_world('foot_fk.'+side,-hip-bend*.75)
            rot_world('upper_arm_fk.'+side,-hip*arm_amp/hip_amp,sign*.55)
            bpy.context.view_layer.update()
            # MPFB rest forearms already angle forward substantially; these
            # values give about 145 degrees in Walk and 89 in Run.
            rot_world('forearm_fk.'+side, .20 if kind=='Walk' else -.80,compensate_parent=True)
        if i in (0,steps):
            relax_fingers(frame)
            rot_world('chest',0);rot_world('root',0)
        # Actual surface soles determine correction, with explicit airborne
        # intervals in Run; interpolation is verified independently below.
        root=rig.pose.bones['root'];root.location=(0,0,0)
        bpy.context.view_layer.update(); z=min_sole()
        if kind=='Run':
            # Flight between alternating stance intervals, symmetric in each
            # half-cycle. Preserve a true airborne phase rather than pinning.
            half=(i/steps)%0.5
            airborne=.055*math.sin(math.pi*(half-.40)/.10) if half>.40 else 0
        else:airborne=0
        root.location.z=airborne-z;root.keyframe_insert(data_path='location',frame=frame,group='root')
    # Linear sampling avoids Bezier overshoot between densely sampled contacts.
    for layer in act.layers:
        for strip in layer.strips:
            for bag in strip.channelbags:
                for curve in bag.fcurves:
                    for kp in curve.keyframe_points:kp.interpolation='LINEAR'
    scene.frame_set(1);bpy.context.view_layer.update()
    neutral_knee=rig.pose.bones['DEF-thigh.L'].tail.copy()
    scene.frame_set(1+steps//4);bpy.context.view_layer.update()
    moved_knee=rig.pose.bones['DEF-thigh.L'].tail.copy()
    displacement=(moved_knee-neutral_knee).length
    print(kind,'deform knee displacement',displacement,flush=True)
    if displacement<=.03:raise RuntimeError('FK keys did not reach the actual deform rig')
    clips[kind]={'start':1,'end':steps+1,'fps':120,'duration_seconds':steps/120,'sole_samples':{}}
    for i in range(steps*4+1):
        frame=1+i/4;scene.frame_set(int(frame),subframe=frame%1);bpy.context.view_layer.update()
        z=min_sole();clips[kind]['sole_samples'][str(frame)]=round(z,6)
    vals=list(clips[kind]['sole_samples'].values())
    clips[kind]['sole_min']=min(vals);clips[kind]['sole_max']=max(vals)
    print(kind,'sole',min(vals),max(vals),flush=True)
    if min(vals)<=-.003:raise RuntimeError('A sole penetrated the ground')
    if kind=='Run' and max(vals)<=.04:raise RuntimeError('Run lost airborne phases')

# Additional actions share the same deform rig and bind. Hit leaves the lower
# body still. Death rotates the rig about its ground-level root and uses the
# full evaluated skin for floor clearance as the back reaches the floor.
for kind,steps in [('Idle',120),('Hit',60),('Death',144)]:
    rig.animation_data.action=None;clear_pose()
    act=bpy.data.actions.new(kind);act.use_fake_user=True;rig.animation_data.action=act
    for i in range(steps+1):
        frame=i+1;scene.frame_set(frame);t=i/steps
        if kind=='Idle':
            relaxed_arms(.025*math.sin(2*math.pi*t))
            rot_world('chest',.012*math.sin(2*math.pi*t))
        elif kind=='Hit':
            pulse=math.sin(math.pi*min(1,t/.65)) if t<.65 else 0
            relaxed_arms(.16*pulse)
            rot_world('chest',-.30*pulse)
        else:
            fall=min(1,t/.72);fall=fall*fall*(3-2*fall)
            relaxed_arms(.42*math.sin(math.pi*fall)+.12*fall,.20-.35*fall)
            rot_world('chest',-.10*math.sin(math.pi*fall))
            for side in ('L','R'):
                rot_world('thigh_fk.'+side,-.35*fall)
                rot_world('shin_fk.'+side,.90*math.sin(math.pi*fall*.75))
            rot_world('root',-math.pi*.5*fall)
            root=rig.pose.bones['root'];root.location=(0,0,0)
            bpy.context.view_layer.update();root.location.z=-min_body()
            root.keyframe_insert(data_path='location',frame=frame,group='root')
        if i in (0,steps):
            relax_fingers(frame)
            for side in ('L','R'):
                rot_world('foot_fk.'+side,0)
                if kind!='Death':
                    rot_world('thigh_fk.'+side,0);rot_world('shin_fk.'+side,0)
            if kind!='Death':
                rot_world('root',0)
                root=rig.pose.bones['root'];root.location=(0,0,0)
                root.keyframe_insert(data_path='location',frame=frame,group='root')
    for layer in act.layers:
        for strip in layer.strips:
            for bag in strip.channelbags:
                for curve in bag.fcurves:
                    for kp in curve.keyframe_points:kp.interpolation='LINEAR'
    clips[kind]={'start':1,'end':steps+1,'fps':120,'duration_seconds':steps/120}
    if kind=='Death':
        heights=[]
        for i in range(steps*4+1):
            frame=1+i/4;scene.frame_set(int(frame),subframe=frame%1);heights.append(min_body())
        clips[kind]['body_floor_min']=min(heights);clips[kind]['body_floor_max']=max(heights)
        print('Death full-body floor',min(heights),max(heights),flush=True)
        if min(heights)<-.003:raise RuntimeError('Death body penetrated the ground')

rig.animation_data.action=bpy.data.actions['Walk'];scene.render.fps=120;scene.frame_start=1;scene.frame_end=121;scene.frame_set(1)
bpy.context.view_layer.update()
landmarks={}
for name in ['torso','shoulder.L','upper_arm_fk.L','forearm_fk.L','hand_fk.L','shoulder.R','upper_arm_fk.R','forearm_fk.R','hand_fk.R','thigh_fk.L','shin_fk.L','foot_fk.L','thigh_fk.R','shin_fk.R','foot_fk.R','neck','head','DEF-spine','DEF-spine.001','DEF-spine.002','DEF-spine.003','DEF-spine.004','DEF-spine.005','DEF-spine.006','DEF-shoulder.L','DEF-upper_arm.L','DEF-forearm.L','DEF-hand.L','DEF-shoulder.R','DEF-upper_arm.R','DEF-forearm.R','DEF-hand.R','DEF-thigh.L','DEF-shin.L','DEF-foot.L','DEF-thigh.R','DEF-shin.R','DEF-foot.R']:
    b=rig.data.bones[name];landmarks[name]={'head_world':list(rig.matrix_world@b.head_local),'tail_world':list(rig.matrix_world@b.tail_local),'rest_world_matrix':[list(row) for row in rig.matrix_world@b.matrix_local]}
metadata={'source':source_path,'axis':'+Z up, -Y front; metres','mesh':body.name,'robe':robe.name,'rig':rig.name,'source_rest':'Original MPFB A-pose Rigify rest; top-four normalized human deform weights','body_vertices_raw':len(body.data.vertices),'unweighted_raw_helper_vertices':len(unweighted_raw),'body_vertices_visible':len(visible_weights),'pruned_body_vertices':pruned_vertices,'evaluated_foot_weighted_vertices':foot_count,'bind_min':bind_min,'landmarks':landmarks,'clips':clips,'motion':'Procedural FK actions sampled at 120 Hz. Walk and Run root correction uses the actual evaluated masked LBS feet; Run has explicit airborne intervals. Death correction uses the entire evaluated masked human. Robe clearance remains the garment owner\'s task.','deformation':'Ordinary LBS only; weighted B-bones use one segment. The original body mask modifiers and separate robe are preserved. The robe weights have not been pruned.','collapsed_weighted_bones_original_segments':collapsed_bones}
(OUT/'measurements.json').write_text(json.dumps(metadata,indent=2))
# Review renders retain the authored body, mask and garment materials.
scene.render.engine='BLENDER_WORKBENCH';scene.display.shading.light='STUDIO';scene.display.shading.color_type='MATERIAL';scene.display.shading.show_shadows=True
scene.render.resolution_x=512;scene.render.resolution_y=512;scene.render.resolution_percentage=100
camera=bpy.data.objects.get('Human review camera')
if camera is None:
    data=bpy.data.cameras.new('Human review camera');camera=bpy.data.objects.new('Human review camera',data);scene.collection.objects.link(camera)
camera.hide_render=False;camera.data.type='ORTHO';camera.data.ortho_scale=2.25;scene.camera=camera
for kind,frames in [('Walk',[1,31,61,91]),('Run',[1,20,39,58]),('Idle',[1,31]),('Hit',[1,20,40,61]),('Death',[1,37,73,109,145])]:
    rig.animation_data.action=bpy.data.actions[kind]
    for frame in frames:
        scene.frame_set(frame)
        bpy.context.view_layer.update();ev=body.evaluated_get(bpy.context.evaluated_depsgraph_get());mesh=ev.to_mesh()
        points=[ev.matrix_world@v.co for v in mesh.vertices];ev.to_mesh_clear()
        center=Vector(tuple((min(p[k] for p in points)+max(p[k] for p in points))/2 for k in range(3)))
        for label,offset in [('front',(0,-4,.2)),('side',(4,-.1,.2))]:
            camera.location=center+Vector(offset);camera.rotation_euler=(center-camera.location).to_track_quat('-Z','Y').to_euler()
            scene.render.filepath=str(OUT/f'{kind.lower()}-{frame:03d}-{label}.png');bpy.ops.render.render(write_still=True)
rig.animation_data.action=bpy.data.actions['Walk'];scene.frame_set(1)
bpy.ops.wm.save_as_mainfile(filepath=str(OUT/'human-master.blend'))
print('DONE',OUT,flush=True)
