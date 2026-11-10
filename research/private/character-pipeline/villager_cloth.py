"""Offline lower-robe physics trial over the matched MPFB villager.

Sample the continuous physics timeline before rendering frozen mesh samples.
This experiment does not modify the input master or clear Rigify drivers.
"""
import argparse
import hashlib
import json
import math
import re
import struct
import sys
from pathlib import Path

import bmesh
import bpy
import numpy as np
from mathutils import Quaternion, Vector
from mathutils.bvhtree import BVHTree

DEATH_SOURCE_FINGERPRINT = '4df2e8675b3be7704428fda6cef92c53c280ff01def473e91b6e8271d0d9bfc2'
DEATH_SUPPORTED_FINGERPRINT = '8884ca92388318e63efb315c652c5e143267c1475a718141e343edb3ee5afef8'
DEATH_CONTROL_REST_FINGERPRINT = '27a6418b1bdb4556d90aade3a85f01e7ea59f0ca196a7fd4c9525b2a69469d98'
RUN_SOURCE_FINGERPRINT = 'b53b5037f529096ba64e4ea9ce6a9f98752a12453b9d663d49fbda851593ad5e'
RUN_SUPPORTED_FINGERPRINT = '98227dc8c18342a65dd12000affac2655906eadc5b366a4a5961fd4bb8c7632b'
RUN_CONTROL_REST_FINGERPRINT = 'b980750a7fc29d4358642eb1a2f13f75d6dbaf5638dbef726a134f7d59112e5b'
FOOTWEAR_MASK_FINGERPRINT = 'feed34e79f47f30aafb1df45a052c11d0d1b4e15666005f6528aeb833c816ebf'
SHARED_REST_SHA256 = 'eb72b9cd971b394543d13f42a8420eec3efd3b76a31d214da3bae2d3c973ef27'
SHARED_TRIANGLES_SHA256 = '5e6f1ca21f47823c355badd2117aaa47606778d6c5dd55ccfb59b82e455c8cf1'
FOOT_SOURCE_FINGERPRINTS = {
    'Human': '5a1891faf3aed881a63aa97fe7215873b5017f7430ee221dd168ba969c1b3af4',
    'Brown shoe L': '868d5526ff3228f6975704755237c6943654ce0eedfc440843678f51f072ca54',
    'Brown shoe R': 'ccd5c94b740b630f1441ccc0338b4210b371893d8ff1decc9e3df979bd150df6',
}

def mesh_fingerprint(obj):
    """Hash authored positions, topology and named groups without modifiers."""
    digest = hashlib.sha256()
    for vertex in obj.data.vertices:
        digest.update(struct.pack('<3fI',*vertex.co,len(vertex.groups)))
        for group in sorted(vertex.groups,key=lambda item:obj.vertex_groups[item.group].name):
            digest.update(obj.vertex_groups[group.group].name.encode()+b'\0')
            digest.update(struct.pack('<f',group.weight))
    for face in obj.data.polygons:
        digest.update(struct.pack('<I',len(face.vertices)))
        for index in face.vertices:
            digest.update(struct.pack('<I',index))
    return digest.hexdigest()

def footwear_mask_fingerprint(human):
    group=human.vertex_groups.get('Covered by footwear')
    modifier=human.modifiers.get('Footwear skin coverage')
    if (group is None or modifier is None or modifier.type!='MASK' or modifier.vertex_group!=group.name or
            not modifier.invert_vertex_group or modifier.threshold!=0 or not modifier.show_viewport or not modifier.show_render):
        return None
    digest=hashlib.sha256()
    for vertex in human.data.vertices:
        for weight in vertex.groups:
            if weight.group==group.index and weight.weight>0:
                digest.update(struct.pack('<If',vertex.index,weight.weight))
    return digest.hexdigest()

def merge_footwear_mask(model, output):
    """Mask only covered foot skin, retaining the full editable anatomy."""
    if output.exists() or output.resolve() == model.resolve():
        raise FileExistsError('Choose a fresh output file')
    bpy.ops.wm.open_mainfile(filepath=str(model.resolve()))
    rig=bpy.data.objects['VillagerRig'];human=bpy.data.objects['Human'];scene=bpy.context.scene
    if (any(mesh_fingerprint(bpy.data.objects[name]) != expected for name,expected in FOOT_SOURCE_FINGERPRINTS.items())
            or human.vertex_groups.get('Covered by footwear') is not None
            or human.modifiers.get('Footwear skin coverage') is not None
            or len(rig.animation_data.drivers) != 162):
        raise ValueError('Human, shoes, or driver state differ from the checked source; no output written')
    prior_pose=rig.data.pose_position;prior_action=rig.animation_data.action;prior_slot=rig.animation_data.action_slot
    prior_frame,prior_subframe=scene.frame_current,scene.frame_subframe
    masks=[(m,m.show_viewport) for m in human.modifiers if m.type=='MASK']
    for modifier,_ in masks:modifier.show_viewport=False
    rig.animation_data.action=None;rig.data.pose_position='REST';bpy.context.view_layer.update()
    deps=bpy.context.evaluated_depsgraph_get();evaluated=human.evaluated_get(deps)
    mesh=evaluated.to_mesh(preserve_all_data_layers=True,depsgraph=deps)
    if len(mesh.vertices)!=len(human.data.vertices):
        evaluated.to_mesh_clear();raise ValueError('Foot mask requires stable unmasked Human vertex indices')
    selected={}
    for side in ('L','R'):
        shoe=bpy.data.objects['Brown shoe '+side];se=shoe.evaluated_get(deps);sm=se.to_mesh()
        try:
            tree=BVHTree.FromPolygons([se.matrix_world@v.co for v in sm.vertices],
                                     [tuple(p.vertices) for p in sm.polygons])
        finally:se.to_mesh_clear()
        groups={g.index for g in human.vertex_groups if g.name.startswith('DEF-foot.'+side)
                or (g.name.startswith('DEF-toe') and g.name.endswith('.'+side))}
        if not groups:raise ValueError('Missing anatomical foot groups: '+side)
        ids=[]
        for vertex in mesh.vertices:
            if sum(g.weight for g in vertex.groups if g.group in groups)<.25:continue
            point=evaluated.matrix_world@vertex.co;hit,normal,_,distance=tree.find_nearest(point)
            # Signed surface distance admits covered skin and a small measured
            # shoe-surface margin, including soles that protrude a few mm.
            if hit is not None and (point-hit).dot(normal)<=.008:
                ids.append(vertex.index)
        if not ids:raise ValueError('No covered foot vertices for '+side)
        selected[side]=ids
    evaluated.to_mesh_clear()
    group=human.vertex_groups.new(name='Covered by footwear')
    group.add(sorted(set(selected['L'])|set(selected['R'])),1.,'REPLACE')
    modifier=human.modifiers.new('Footwear skin coverage','MASK')
    modifier.vertex_group=group.name;modifier.invert_vertex_group=True;modifier.threshold=0
    for old,state in masks:old.show_viewport=state
    rig.animation_data.action=prior_action
    if prior_action is not None and prior_slot is not None:rig.animation_data.action_slot=prior_slot
    rig.data.pose_position=prior_pose;scene.frame_set(prior_frame,subframe=prior_subframe);bpy.context.view_layer.update()
    output.parent.mkdir(parents=True,exist_ok=True)
    bpy.ops.wm.save_as_mainfile(filepath=str(output.resolve()))
    print('FOOTWEAR_MASK',json.dumps({'left':len(selected['L']),'right':len(selected['R']),
          'method':'foot/toe anatomical weights and signed nearest distance to closed bind-pose shoe surface; 8mm margin',
          'drivers':len(rig.animation_data.drivers)}),flush=True)

def action_fingerprint(action):
    """Fingerprint all existing Death F-curves, including interpolation handles."""
    digest = hashlib.sha256()
    for layer in action.layers:
        for strip in layer.strips:
            for bag in strip.channelbags:
                for curve in sorted(bag.fcurves, key=lambda c:(c.data_path,c.array_index)):
                    digest.update(curve.data_path.encode()+b'\0')
                    digest.update(struct.pack('<iI', curve.array_index, len(curve.keyframe_points)))
                    for point in curve.keyframe_points:
                        digest.update(struct.pack('<6f', *point.co, *point.handle_left, *point.handle_right))
                        digest.update(point.interpolation.encode()+b'\0')
    return digest.hexdigest()

def death_control_rest_fingerprint(rig, action):
    """Check referenced control bones and their rest ancestors, ignoring cloth bones."""
    names = {'head'}
    for layer in action.layers:
        for strip in layer.strips:
            for bag in strip.channelbags:
                for curve in bag.fcurves:
                    match = re.match(r'pose.bones\["([^"]+)"\]', curve.data_path)
                    if match:
                        names.add(match.group(1))
    for name in list(names):
        parent = rig.data.bones[name].parent
        while parent:
            names.add(parent.name);parent = parent.parent
    digest = hashlib.sha256()
    for name in sorted(names):
        bone = rig.data.bones[name]
        digest.update(name.encode()+b'\0')
        digest.update((bone.parent.name if bone.parent else '').encode()+b'\0')
        for row in bone.matrix_local:
            digest.update(struct.pack('<4f',*row))
    return digest.hexdigest()

def prepare_run_support(model, output):
    """Copy Run and add only the measured world-Z support to body and cloth."""
    if output.exists():raise FileExistsError('Choose a fresh output directory')
    bpy.ops.wm.open_mainfile(filepath=str(model.resolve()))
    scene=bpy.context.scene;rig=bpy.data.objects['VillagerRig'];source=bpy.data.actions['Run']
    human=bpy.data.objects['Human']
    if (bpy.data.actions.get('Run [original]') or action_fingerprint(source)!=RUN_SOURCE_FINGERPRINT or
            death_control_rest_fingerprint(rig,source)!=RUN_CONTROL_REST_FINGERPRINT or
            footwear_mask_fingerprint(human)!=FOOTWEAR_MASK_FINGERPRINT or len(rig.animation_data.drivers)!=162 or
            tuple(source.frame_range)!=(1.,77.)):
        raise ValueError('Run action, footwear mask, or rig differs from the checked source')
    cloth=[bone for bone in rig.data.bones if bone.name.startswith('CLOTH.') and bone.use_deform]
    root=rig.data.bones['root']
    if len(cloth)!=48 or root.parent or any(bone.parent for bone in cloth):
        raise ValueError('Expected one root and 48 independently animated root-level cloth bones')
    visible=[obj for obj in scene.objects if obj.type=='MESH' and not obj.hide_render
             and not obj.name.startswith('WGT-')]
    if not visible:raise ValueError('No visible costume geometry')
    prior_action=rig.animation_data.action;prior_slot=rig.animation_data.action_slot
    prior_frame,prior_subframe=scene.frame_current,scene.frame_subframe
    prior_pose=rig.data.pose_position
    source.name='Run [original]';source.use_fake_user=True
    action=source.copy();action.name='Run';action.use_fake_user=True
    rig.animation_data.action=action;rig.animation_data.action_slot=action.slots[0]
    rig.data.pose_position='POSE';world_inverse=rig.matrix_world.to_3x3().inverted()
    def points(obj):
        evaluated=obj.evaluated_get(bpy.context.evaluated_depsgraph_get());mesh=evaluated.to_mesh()
        try:
            data=np.empty(len(mesh.vertices)*3,dtype=np.float32);mesh.vertices.foreach_get('co',data)
            matrix=np.array(evaluated.matrix_world,dtype=np.float64)
            return data.reshape((-1,3))@matrix[:3,:3].T+matrix[:3,3]
        finally:evaluated.to_mesh_clear()
    rows=[];endpoint_surfaces={}
    for frame in range(1,78):
        scene.frame_set(frame);bpy.context.view_layer.update()
        before={obj.name:points(obj) for obj in visible}
        before_bones={bone.name:rig.pose.bones[bone.name].matrix.copy() for bone in cloth}
        minima={name:float(values[:,2].min()) for name,values in before.items()}
        low=min(minima,key=minima.get);delta=max(0.,.0005-minima[low]);world_shift=Vector((0,0,delta))
        for bone in [root,*cloth]:
            pose=rig.pose.bones[bone.name];local=bone.matrix_local.to_3x3().inverted()@world_inverse@world_shift
            pose.location+=local
            for index in range(3):pose.keyframe_insert(data_path='location',index=index,frame=frame,group=bone.name)
        scene.frame_set(frame);bpy.context.view_layer.update()
        after={obj.name:points(obj) for obj in visible};difference=np.array((0.,0.,delta))
        surface_error=max(float(np.linalg.norm(after[name]-before[name]-difference,axis=1).max()) for name in before)
        bone_error=max((rig.pose.bones[bone.name].matrix.translation-before_bones[bone.name].translation-
                        world_inverse@world_shift).length for bone in cloth)
        if surface_error>.00001 or bone_error>.00001:
            raise RuntimeError(f'Run support changed relative surfaces at frame {frame}: {surface_error}, {bone_error}')
        rows.append({'source_frame':frame,'lowest_object':low,'source_min_z':minima[low],
                     'world_z_added':delta,'output_min_z':min(float(v[:,2].min()) for v in after.values()),
                     'surface_translation_max_error_m':surface_error,'cloth_bone_translation_max_error_m':bone_error})
        if frame in (1,77):endpoint_surfaces[frame]=after
    endpoint_error=max(float(np.linalg.norm(endpoint_surfaces[1][n]-endpoint_surfaces[77][n],axis=1).max())
                       for n in endpoint_surfaces[1])
    if endpoint_error>.000001 or rows[0]['world_z_added']!=rows[-1]['world_z_added']:
        raise RuntimeError(f'Run support changed the loop endpoints: {endpoint_error}')
    for layer in action.layers:
        for strip in layer.strips:
            for bag in strip.channelbags:
                for curve in bag.fcurves:
                    if curve.data_path.endswith('.location') and (curve.data_path.startswith('pose.bones["root"]') or
                            curve.data_path.startswith('pose.bones["CLOTH.')):
                        for point in curve.keyframe_points:point.interpolation='LINEAR'
    # Half-frame audit does not add any unmeasured support or alter the profile.
    half=[]
    for index in range(153):
        frame=1+index/2;scene.frame_set(int(frame),subframe=frame%1);bpy.context.view_layer.update()
        values={obj.name:float(points(obj)[:,2].min()) for obj in visible};name=min(values,key=values.get)
        half.append({'source_frame':frame,'lowest_object':name,'min_z':values[name]})
    report={'fps':120,'source_action_fingerprint':RUN_SOURCE_FINGERPRINT,'supported_action_fingerprint':action_fingerprint(action),
            'method':'minimum world-Z correction to put the evaluated visible costume at or above +0.5mm at each source frame; identical translation applied to all 48 independent cloth bones',
            'source_frames':rows,'half_frame_audit':half,'endpoint_surface_max_error_m':endpoint_error,
            'original_action_retained':'Run [original]','drivers':len(rig.animation_data.drivers)}
    output.mkdir(parents=True,exist_ok=False);(output/'run-support.json').write_text(json.dumps(report,indent=2)+'\n')
    if prior_action==source:rig.animation_data.action=action;rig.animation_data.action_slot=action.slots[0]
    else:
        rig.animation_data.action=prior_action
        if prior_action is not None and prior_slot is not None:rig.animation_data.action_slot=prior_slot
    rig.data.pose_position=prior_pose;scene.frame_set(prior_frame,subframe=prior_subframe);bpy.context.view_layer.update()
    bpy.ops.wm.save_as_mainfile(filepath=str((output/'run-supported.blend').resolve()))
    print('RUN_SUPPORT',json.dumps({'max_lift_m':max(x['world_z_added'] for x in rows),
          'min_output_m':min(x['output_min_z'] for x in rows),'min_half_frame_m':min(x['min_z'] for x in half),
          'max_surface_error_m':max(x['surface_translation_max_error_m'] for x in rows),
          'max_cloth_bone_error_m':max(x['cloth_bone_translation_max_error_m'] for x in rows),
          'endpoint_error_m':endpoint_error,'supported_fingerprint':report['supported_action_fingerprint']}),flush=True)

def merge_run_support(model, donor, output):
    """Apply the reviewed Run-only support to a fresh, already-masked master."""
    report_path=output.with_suffix('.run-support.json')
    if output.exists() or report_path.exists() or output.resolve() in {model.resolve(),donor.resolve()}:
        raise FileExistsError('Choose a fresh output and report path')
    bpy.ops.wm.open_mainfile(filepath=str(model.resolve()))
    scene=bpy.context.scene;rig=bpy.data.objects['VillagerRig'];original=bpy.data.actions['Run'];human=bpy.data.objects['Human']
    if (bpy.data.actions.get('Run [original]') or action_fingerprint(original)!=RUN_SOURCE_FINGERPRINT or
            death_control_rest_fingerprint(rig,original)!=RUN_CONTROL_REST_FINGERPRINT or
            footwear_mask_fingerprint(human)!=FOOTWEAR_MASK_FINGERPRINT or len(rig.animation_data.drivers)!=162):
        raise ValueError('Run action, control rest, footwear mask, or drivers differ; no output written')
    visible=[obj for obj in scene.objects if obj.type=='MESH' and not obj.hide_render and not obj.name.startswith('WGT-')]
    if not {'Human','Brown shoe L','Brown shoe R','Walk skeletal lower cloth'}.issubset({o.name for o in visible}):
        raise ValueError('Required visible body, footwear, or lower cloth missing')
    prior_action=rig.animation_data.action;prior_slot=rig.animation_data.action_slot
    prior_frame,prior_subframe=scene.frame_current,scene.frame_subframe;prior_pose=rig.data.pose_position
    with bpy.data.libraries.load(str(donor.resolve()),link=False) as (source,target):
        if 'Run' not in source.actions:raise ValueError('Supported action library has no Run action')
        target.actions=['Run']
    supported=target.actions[0]
    if supported is None or action_fingerprint(supported)!=RUN_SUPPORTED_FINGERPRINT:
        raise ValueError('Supported Run action differs from the reviewed candidate')
    original.name='Run [original]';original.use_fake_user=True;supported.name='Run';supported.use_fake_user=True
    rig.animation_data.action=supported;rig.animation_data.action_slot=supported.slots[0];rig.data.pose_position='POSE'
    floors=[]
    for index in range(153):
        frame=1+index/2;scene.frame_set(int(frame),subframe=frame%1);bpy.context.view_layer.update();deps=bpy.context.evaluated_depsgraph_get();mins={}
        for obj in visible:
            evaluated=obj.evaluated_get(deps);mesh=evaluated.to_mesh()
            try:mins[obj.name]=min((evaluated.matrix_world@v.co).z for v in mesh.vertices)
            finally:evaluated.to_mesh_clear()
        lowest=min(mins,key=mins.get);floors.append({'source_frame':frame,'lowest_object':lowest,'min_z':mins[lowest]})
    if min(row['min_z'] for row in floors if row['source_frame'].is_integer())<.000499:
        raise ValueError('Supported Run no longer meets the measured 0.5mm integer-frame clearance')
    if min(row['min_z'] for row in floors)<-1e-6:
        raise ValueError('Supported Run penetrates ground between source frames')
    if prior_action==original:rig.animation_data.action=supported;rig.animation_data.action_slot=supported.slots[0]
    else:
        rig.animation_data.action=prior_action
        if prior_action is not None and prior_slot is not None:rig.animation_data.action_slot=prior_slot
    rig.data.pose_position=prior_pose;scene.frame_set(prior_frame,subframe=prior_subframe);bpy.context.view_layer.update()
    output.parent.mkdir(parents=True,exist_ok=True)
    report_path.write_text(json.dumps({'scope':'actual visible target costume; 120Hz and 240Hz floor audit',
                                      'source_action':RUN_SOURCE_FINGERPRINT,'supported_action':RUN_SUPPORTED_FINGERPRINT,
                                      'frames':floors},indent=2)+'\n')
    bpy.ops.wm.save_as_mainfile(filepath=str(output.resolve()))
    print('RUN_SUPPORT_MERGED',str(output),'min_z',min(row['min_z'] for row in floors),flush=True)

def merge_supported_death(model, donor, output):
    """Replace only the checked Death action in a fresh copy of a common master."""
    if output.exists() or output.resolve() in {model.resolve(),donor.resolve()}:
        raise FileExistsError('Choose a fresh output file')
    bpy.ops.wm.open_mainfile(filepath=str(model.resolve()))
    rig = bpy.data.objects['VillagerRig'];original = bpy.data.actions['Death']
    if (bpy.data.actions.get('Death [original]') or action_fingerprint(original) != DEATH_SOURCE_FINGERPRINT or
            death_control_rest_fingerprint(rig,original) != DEATH_CONTROL_REST_FINGERPRINT or
            len(rig.animation_data.drivers) != 162):
        raise ValueError('Death source action, rig rest controls, or drivers do not match; no output written')
    previous_action = rig.animation_data.action
    with bpy.data.libraries.load(str(donor.resolve()),link=False) as (source,target):
        if 'Death' not in source.actions:
            raise ValueError('Supported action library has no Death action')
        target.actions = ['Death']
    supported = target.actions[0]
    if supported is None or action_fingerprint(supported) != DEATH_SUPPORTED_FINGERPRINT:
        raise ValueError('Supported Death action does not match the reviewed candidate')
    original.name = 'Death [original]';original.use_fake_user = True
    supported.name = 'Death';supported.use_fake_user = True
    if previous_action == original:
        rig.animation_data.action = supported;rig.animation_data.action_slot = supported.slots[0]
    if len(rig.animation_data.drivers) != 162:
        raise RuntimeError('Rigify drivers changed')
    output.parent.mkdir(parents=True,exist_ok=True)
    bpy.ops.wm.save_as_mainfile(filepath=str(output.resolve()))
    print('DEATH_MERGED',str(output),flush=True)

def prepare_supported_death(model, output, reference_cache):
    """Author a Death-only, costume-supported action without changing other actions."""
    if output.exists():
        raise FileExistsError('Choose a fresh output directory')
    bpy.ops.wm.open_mainfile(filepath=str(model.resolve()))
    scene = bpy.context.scene
    rig = bpy.data.objects['VillagerRig']
    original = bpy.data.actions['Death']
    if (bpy.data.actions.get('Death [original]') or action_fingerprint(original) != DEATH_SOURCE_FINGERPRINT or
            death_control_rest_fingerprint(rig,original) != DEATH_CONTROL_REST_FINGERPRINT):
        raise ValueError('Death action differs from the checked source or was already prepared')
    if len(rig.animation_data.drivers) != 162:
        raise ValueError('Unexpected Rigify driver count')
    reference = np.load(reference_cache)
    if (reference['poses'].shape[1:] != (1920,3) or reference['triangles'].shape != (3712,3) or
            hashlib.sha256(reference['poses'][0].tobytes()).hexdigest() != SHARED_REST_SHA256 or
            hashlib.sha256(reference['triangles'].tobytes()).hexdigest() != SHARED_TRIANGLES_SHA256):
        raise ValueError('Unexpected shared lower-robe reference')
    prior_action = rig.animation_data.action
    prior_slot = rig.animation_data.action_slot
    prior_frame, prior_subframe = scene.frame_current, scene.frame_subframe
    prior_pose = rig.data.pose_position
    # A temporary in-memory copy matches the cut actually used by the cloth
    # producer. The editable source upper and every other mesh stay untouched.
    upper = bpy.data.objects['Human.donitz_monk_robe']
    upper_hidden = upper.hide_render
    preview_upper = upper.copy()
    preview_upper.data = upper.data.copy()
    preview_upper.name = 'Temporary supported-Death upper measurement'
    scene.collection.objects.link(preview_upper)
    bm = bmesh.new(); bm.from_mesh(preview_upper.data)
    bmesh.ops.bisect_plane(bm,geom=list(bm.verts)+list(bm.edges)+list(bm.faces),dist=1e-6,
                           plane_co=(0,0,.83),plane_no=(0,0,1),clear_outer=False,clear_inner=True)
    bmesh.ops.delete(bm,geom=[v for v in bm.verts if not v.link_faces],context='VERTS')
    bm.to_mesh(preview_upper.data);bm.free();preview_upper.data.update()
    upper.hide_render = True
    visible = [obj for obj in scene.objects if obj.type == 'MESH' and not obj.hide_render
               and not obj.name.startswith('WGT-')]
    # Recover the shared pinned-rest coordinates from the actual Idle frame.
    idle = bpy.data.actions['Idle'];rig.animation_data.action = idle;rig.animation_data.action_slot = idle.slots[0]
    rig.data.pose_position = 'POSE';scene.frame_set(1);bpy.context.view_layer.update()
    def skin_matrix():
        bone = rig.pose.bones['DEF-spine']
        return rig.matrix_world @ bone.matrix @ bone.bone.matrix_local.inverted() @ rig.matrix_world.inverted()
    initial_inverse = skin_matrix().inverted()
    pin_rest = [initial_inverse @ Vector(point) for point in reference['poses'][0,:192]]
    original.name = 'Death [original]'; original.use_fake_user = True
    action = original.copy();action.name = 'Death';action.use_fake_user = True
    rig.animation_data.action = action;rig.animation_data.action_slot = action.slots[0]
    root = rig.pose.bones['root']
    def pose_world_x(name, angle, compensate_parent=False, adduction=0.):
        pose = rig.pose.bones[name]; q = pose.bone.matrix_local.to_quaternion(); d = Quaternion()
        if compensate_parent and pose.parent:
            d = (pose.parent.matrix.to_3x3() @ pose.parent.bone.matrix_local.to_3x3().inverted()).to_quaternion()
        pose.rotation_mode = 'QUATERNION'
        pose.rotation_quaternion = (q.inverted() @ d.inverted() @ Quaternion((1,0,0),angle)
                                    @ Quaternion((0,1,0),adduction) @ d @ q)
    def object_minima(objects):
        deps = bpy.context.evaluated_depsgraph_get(); minima = {}
        for obj in objects:
            evaluated = obj.evaluated_get(deps); mesh = evaluated.to_mesh()
            try:
                minima[obj.name] = min((evaluated.matrix_world @ v.co).z for v in mesh.vertices)
            finally:
                evaluated.to_mesh_clear()
        return minima
    shoes = [bpy.data.objects['Brown shoe L'],bpy.data.objects['Brown shoe R']]
    headwear = [bpy.data.objects['Turban crown'],bpy.data.objects['Nape wrap']]
    checks = []
    for frame in range(1,146):
        scene.frame_set(frame);bpy.context.view_layer.update()
        t=(frame-1)/144;fall=min(1.,t/.72);fall=fall*fall*(3-2*fall)
        late=max(0.,min(1.,(fall-.80)/.20));late=late*late*(3-2*late)
        arm_settle=max(0.,min(1.,(frame-105)/40));arm_settle=arm_settle*arm_settle*(3-2*arm_settle)
        if arm_settle:
            # Retain the protective lift throughout impact, then let both
            # hands settle beside the torso during the held final pose.
            for side,sign in [('L',1),('R',-1)]:
                pose_world_x('upper_arm_fk.'+side,sign*.12*(1-arm_settle),adduction=sign*.55)
            bpy.context.view_layer.update()
            for side in ('L','R'):
                pose_world_x('forearm_fk.'+side,-.15*(1-arm_settle)+arm_settle,compensate_parent=True)
        # Let the head settle onto its wrap late in the fall. The head and
        # both thigh controls are the only local body changes.
        pose_world_x('head', -.50*late, compensate_parent=True)
        bpy.context.view_layer.update()
        minima = object_minima(visible)
        pin_min = min((skin_matrix() @ point).z for point in pin_rest)
        lift = max(0., -min(minima.values()))
        root.location.z += lift
        bpy.context.view_layer.update()
        # Lower both shoes smoothly as the supported body reaches its final
        # pose. This uses the measured shoe surface, not a whole-body lift.
        shoe_min = min(object_minima(shoes).values())
        thigh_delta = min(.14, max(0.,(shoe_min-.003)/.83))*late
        for side in ('L','R'):
            pose_world_x('thigh_fk.'+side, -.35*fall+thigh_delta)
        bpy.context.view_layer.update()
        minima = object_minima(visible)
        pin_min = min((skin_matrix() @ point).z for point in pin_rest)
        extra = max(0., -min(minima.values()))
        root.location.z += extra
        bpy.context.view_layer.update()
        minima = object_minima(visible)
        pin_min = min((skin_matrix() @ point).z for point in pin_rest)
        root.keyframe_insert(data_path='location',frame=frame,group='root')
        for name in ('head','thigh_fk.L','thigh_fk.R'):
            rig.pose.bones[name].keyframe_insert(data_path='rotation_quaternion',frame=frame,group=name)
        if arm_settle:
            for name in ('upper_arm_fk.L','upper_arm_fk.R','forearm_fk.L','forearm_fk.R'):
                rig.pose.bones[name].keyframe_insert(data_path='rotation_quaternion',frame=frame,group=name)
        checks.append({'source_frame':frame,'min_z':minima,'waist_pin_goal_min_z':pin_min,
                       'root_support_lift':lift+extra,'head_world_x_angle':-.50*late,
                       'thigh_world_x_delta':thigh_delta,'arm_settle':arm_settle})
        if frame%30==0:print('DEATH_SUPPORT_FRAME',frame,flush=True)
    for layer in action.layers:
        for strip in layer.strips:
            for bag in strip.channelbags:
                for curve in bag.fcurves:
                    for point in curve.keyframe_points:point.interpolation = 'LINEAR'
    if len(rig.animation_data.drivers) != 162:
        raise RuntimeError('Rigify drivers changed')
    output.mkdir(parents=True,exist_ok=False)
    (output/'death-source-audit.json').write_text(json.dumps({'fps':120,'source_death_fingerprint':DEATH_SOURCE_FINGERPRINT,
        'method':'one copied procedural Death action; minimal support lift for measured costume; localized late head and thigh adjustments; waist pin clearance reported separately',
        'frames':checks},indent=2)+'\n')
    # Save without the temporary cut upper. The original full upper stays in
    # the master and the new Death action alone is the integration deliverable.
    upper.hide_render = upper_hidden
    bpy.data.objects.remove(preview_upper,do_unlink=True)
    if prior_action == original:
        rig.animation_data.action = action;rig.animation_data.action_slot = action.slots[0]
    else:
        rig.animation_data.action = prior_action
        if prior_action is not None and prior_slot is not None:rig.animation_data.action_slot = prior_slot
    rig.data.pose_position = prior_pose;scene.frame_set(prior_frame,subframe=prior_subframe)
    bpy.context.view_layer.update()
    bpy.ops.wm.save_as_mainfile(filepath=str((output/'death-supported.blend').resolve()))
    print('DEATH_SUPPORTED',json.dumps({'frames':len(checks),'worst_costume_min_z':min(min(c['min_z'].values()) for c in checks),
          'worst_waist_pin_goal_z':min(c['waist_pin_goal_min_z'] for c in checks),'max_root_lift':max(c['root_support_lift'] for c in checks)}),flush=True)

if '--prepare-death' in sys.argv:
    death_parser = argparse.ArgumentParser();death_parser.add_argument('--prepare-death',action='store_true')
    death_parser.add_argument('--model',type=Path,required=True);death_parser.add_argument('--output',type=Path,required=True)
    death_parser.add_argument('--reference-cache',type=Path,required=True)
    death_args=death_parser.parse_args(sys.argv[sys.argv.index('--')+1:])
    prepare_supported_death(death_args.model,death_args.output,death_args.reference_cache)
    sys.exit(0)

if '--merge-death' in sys.argv:
    merge_parser = argparse.ArgumentParser();merge_parser.add_argument('--merge-death',action='store_true')
    merge_parser.add_argument('--model',type=Path,required=True);merge_parser.add_argument('--donor',type=Path,required=True)
    merge_parser.add_argument('--output',type=Path,required=True)
    merge_args=merge_parser.parse_args(sys.argv[sys.argv.index('--')+1:])
    merge_supported_death(merge_args.model,merge_args.donor,merge_args.output)
    sys.exit(0)

if '--merge-shoe-mask' in sys.argv:
    shoe_parser = argparse.ArgumentParser();shoe_parser.add_argument('--merge-shoe-mask',action='store_true')
    shoe_parser.add_argument('--model',type=Path,required=True);shoe_parser.add_argument('--output',type=Path,required=True)
    shoe_args=shoe_parser.parse_args(sys.argv[sys.argv.index('--')+1:])
    merge_footwear_mask(shoe_args.model,shoe_args.output)
    sys.exit(0)

if '--prepare-run-support' in sys.argv:
    run_parser = argparse.ArgumentParser();run_parser.add_argument('--prepare-run-support',action='store_true')
    run_parser.add_argument('--model',type=Path,required=True);run_parser.add_argument('--output',type=Path,required=True)
    run_args=run_parser.parse_args(sys.argv[sys.argv.index('--')+1:])
    prepare_run_support(run_args.model,run_args.output)
    sys.exit(0)

if '--merge-run-support' in sys.argv:
    run_merge_parser = argparse.ArgumentParser();run_merge_parser.add_argument('--merge-run-support',action='store_true')
    run_merge_parser.add_argument('--model',type=Path,required=True);run_merge_parser.add_argument('--donor',type=Path,required=True)
    run_merge_parser.add_argument('--output',type=Path,required=True)
    run_merge_args=run_merge_parser.parse_args(sys.argv[sys.argv.index('--')+1:])
    merge_run_support(run_merge_args.model,run_merge_args.donor,run_merge_args.output)
    sys.exit(0)

parser = argparse.ArgumentParser()
parser.add_argument('--model', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--action', choices=('Walk', 'Run', 'Idle', 'Hit', 'Death'), default='Walk')
parser.add_argument('--simulation-fps', choices=(30, 120), type=int, default=30)
parser.add_argument('--reference-cache', type=Path,
                    help='Walk/Run raw cache proving the finite-action rest topology and frame 1')
args = parser.parse_args(sys.argv[sys.argv.index('--') + 1:])
args.output.mkdir(parents=True, exist_ok=False)
bpy.ops.wm.open_mainfile(filepath=str(args.model.resolve()))
scene = bpy.context.scene
rig = bpy.data.objects['VillagerRig']
robe = bpy.data.objects['Human.donitz_monk_robe']
trousers = bpy.data.objects['Separate olive trousers']
drivers_before = len(rig.animation_data.drivers)
if drivers_before != 162:
    raise RuntimeError(f'Expected 162 Rigify drivers, found {drivers_before}')

# Blend a held Idle pose into the original cyclic action. The source is sampled
# at 120 Hz; .25 strip scale at 30 Hz preserves its exact source-time period.
fps = args.simulation_fps
finite = args.action in {'Idle', 'Hit', 'Death'}
if finite and (fps != 120 or args.reference_cache is None):
    raise ValueError('Finite actions require 120 Hz and an explicit shared reference cache')
source_action = bpy.data.actions[args.action]
source_start, source_end = source_action.frame_range
if finite and (source_start != 1 or source_end != round(source_end)):
    raise ValueError('Finite source actions must start at frame 1 and end on an integer frame')
period = (round(source_end-source_start) if finite else
          (30 if args.action == 'Walk' else 19)*(fps//30))
repeats = 1 if finite else (4 if args.action == 'Walk' or fps == 120 else 6)
end_frame = fps + period*repeats + (fps if finite else 0)
action_start = fps+1
review_start = action_start if finite else action_start + period
review_end = end_frame if finite else review_start + 2*period
rig.animation_data.action = None
idle_track = rig.animation_data.nla_tracks.new()
idle = idle_track.strips.new('Held Idle', 1, bpy.data.actions['Idle'])
idle.frame_end = end_frame + 1
idle.use_animated_time = True
idle.strip_time = 1
idle.keyframe_insert(data_path='strip_time', frame=1)
idle.keyframe_insert(data_path='strip_time', frame=end_frame + 1)
walk_track = rig.animation_data.nla_tracks.new()
walk = walk_track.strips.new(args.action+' at source speed', action_start, bpy.data.actions[args.action])
walk.scale = fps/120
walk.repeat = repeats
walk.blend_type = 'REPLACE'
walk.blend_in = 0 if finite else period
walk.extrapolation = 'HOLD_FORWARD' if finite else 'NOTHING'
scene.render.fps = fps
scene.frame_start, scene.frame_end = 1, end_frame

# Check action timing before creating any physics modifier or cache.
scene.frame_set(review_start)
bpy.context.view_layer.update()
tracked = sorted({group.name for obj in scene.objects if obj.type == 'MESH' and not obj.hide_render
                  for group in obj.vertex_groups if group.name in rig.data.bones
                  and rig.data.bones[group.name].use_deform})
if not tracked:
    raise RuntimeError('No character deform bones found for source-pose comparison')
nla_pose = {name: rig.pose.bones[name].matrix.copy() for name in tracked}
walk_track.mute = True
idle_track.mute = True
rig.animation_data.action = bpy.data.actions[args.action]
scene.frame_set(1)
bpy.context.view_layer.update()
source_pose = {name: rig.pose.bones[name].matrix.copy() for name in tracked}
pose_error = max(abs(nla_pose[n][i][j] - source_pose[n][i][j]) for n in tracked for i in range(4) for j in range(4))
rig.animation_data.action = None
walk_track.mute = False
idle_track.mute = False
if pose_error > .0001:
    raise RuntimeError(f'NLA gait does not match source: matrix max error {pose_error}')
if len(rig.animation_data.drivers) != drivers_before:
    raise RuntimeError('Rigify drivers changed')

# Prove the finite NLA plays the source once and holds its final pose, rather
# than shifting source time or silently blending back to Idle.
finite_pose_checks = []
if finite:
    for source_frame in range(1, round(source_end)+1):
        sim_frame = action_start + source_frame-1
        for current in ([sim_frame, end_frame] if source_frame == source_end else [sim_frame]):
            scene.frame_set(current)
            bpy.context.view_layer.update()
            nla = {name: rig.pose.bones[name].matrix.copy() for name in tracked}
            walk_track.mute = True
            idle_track.mute = True
            rig.animation_data.action = source_action
            scene.frame_set(source_frame)
            bpy.context.view_layer.update()
            error = max(abs(nla[n][i][j] - rig.pose.bones[n].matrix[i][j])
                        for n in tracked for i in range(4) for j in range(4))
            finite_pose_checks.append({'simulation_frame': current, 'source_frame': source_frame,
                                       'matrix_max_error': error})
            rig.animation_data.action = None
            walk_track.mute = False
            idle_track.mute = False
    if max(check['matrix_max_error'] for check in finite_pose_checks) > .0001:
        raise RuntimeError('Finite action or final hold differs from its original source pose')

def cut(obj, height, keep_below):
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    geom = list(bm.verts) + list(bm.edges) + list(bm.faces)
    bmesh.ops.bisect_plane(bm, geom=geom, dist=1e-6,
                           plane_co=(0, 0, height), plane_no=(0, 0, 1),
                           clear_outer=keep_below, clear_inner=not keep_below)
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context='VERTS')
    bm.to_mesh(obj.data)
    bm.free()
    obj.data.update()

cut(robe, .83, False)

# Build one continuous uncapped cloth shell in the held Idle pose. The original
# split shell contains jagged disconnected edges and starts inside the legs.
# Invert the actual pelvis skin transform so the generated rest coordinates
# reproduce these measured-space positions under the existing armature.
scene.frame_set(1)
bpy.context.view_layer.update()
bone = rig.pose.bones['DEF-spine']
skin = rig.matrix_world @ bone.matrix @ bone.bone.matrix_local.inverted() @ rig.matrix_world.inverted()
inverse_skin = skin.inverted()
columns, rows = 64, 30
vertices, faces = [], []
for j in range(rows):
    t = j / (rows - 1)
    # Keep the hem above the shoes and provide fabric for the measured Walk
    # sweep, which reaches y=-.35..+.445 m between heights .2 and .3 m.
    z = .98 - .78*t
    waist_blend = min(1., j/4.)
    rx = .156 * (1-waist_blend) + (.22 + .12*t)*waist_blend
    ry = .125 * (1-waist_blend) + (.17 + .27*t)*waist_blend
    for i in range(columns):
        angle = 2*math.pi*i/columns
        p = Vector((rx*math.cos(angle), -.02 + ry*math.sin(angle), z))
        vertices.append(inverse_skin @ p)
for j in range(rows-1):
    for i in range(columns):
        a = j*columns+i
        b = j*columns+(i+1)%columns
        faces.append((a, a+columns, b+columns, b))
mesh = bpy.data.meshes.new('Continuous lower cloth')
mesh.from_pydata(vertices, [], faces)
mesh.update()
lower = bpy.data.objects.new('Simulated lower robe', mesh)
scene.collection.objects.link(lower)
lower.data.materials.append(robe.data.materials[0])
for face in lower.data.polygons:
    face.use_smooth = True
pelvis = lower.vertex_groups.new(name='DEF-spine')
pelvis.add(list(range(len(lower.data.vertices))), 1.0, 'REPLACE')
pins = lower.vertex_groups.new(name='Cloth waist pin')
for j in range(rows):
    weight = max(0., 1. - j / 3.)
    if weight:
        pins.add(list(range(j*columns, (j+1)*columns)), weight, 'REPLACE')
armature = lower.modifiers.new('Pelvis support', 'ARMATURE')
armature.object = rig
# In Death the uncorrected pelvis-pinned waist goals rotate up to 86 mm below
# z=0, where a Collision plane cannot displace fully pinned vertices. Animate
# only the three anchored rows' pre-armature goals against that plane; frame 1
# remains exactly the shared raw rest. Free fabric still uses real collisions.
floor_support = {'enabled': args.action == 'Death', 'corrected_source_frames': 0,
                 'max_world_correction_m': 0.}
if args.action == 'Death':
    rest = [v.co.copy() for v in lower.data.vertices]
    lower.shape_key_add(name='Basis', from_mix=False)
    for source_frame in range(1, round(source_end)+1):
        frame = action_start+source_frame-1
        scene.frame_set(frame)
        bpy.context.view_layer.update()
        pelvis_bone = rig.pose.bones['DEF-spine']
        pose_skin = (rig.matrix_world @ pelvis_bone.matrix @ pelvis_bone.bone.matrix_local.inverted()
                     @ rig.matrix_world.inverted())
        inverse_linear = pose_skin.to_3x3().inverted()
        correction = {}
        for index in range(3*columns):
            goal = pose_skin @ rest[index]
            height = max(0., .006-goal.z)
            if height:
                correction[index] = inverse_linear @ Vector((0,0,height))
                floor_support['max_world_correction_m'] = max(floor_support['max_world_correction_m'], height)
        if not correction:
            continue
        key = lower.shape_key_add(name=f'Floor support {source_frame:03d}', from_mix=False)
        for index, delta in correction.items():
            key.data[index].co = rest[index]+delta
        key.value = 0
        key.keyframe_insert(data_path='value', frame=1)
        key.keyframe_insert(data_path='value', frame=frame-1)
        key.value = 1
        key.keyframe_insert(data_path='value', frame=frame)
        if source_frame != round(source_end):
            key.value = 0
            key.keyframe_insert(data_path='value', frame=frame+1)
        floor_support['corrected_source_frames'] += 1
    if lower.data.shape_keys.animation_data:
        for layer in lower.data.shape_keys.animation_data.action.layers:
            for strip in layer.strips:
                for bag in strip.channelbags:
                    for curve in bag.fcurves:
                        for point in curve.keyframe_points:
                            point.interpolation = 'LINEAR'
    scene.frame_set(1)
    bpy.context.view_layer.update()
# The skinned continuous trouser surface is the actual moving leg collider.
trousers.modifiers.new('Offline garment collision', 'COLLISION')
trousers.collision.thickness_outer = .006
trousers.collision.cloth_friction = 5
# A separate full-anatomy animated collider includes the hands and forearms.
# The visible Human retains its authored covered-body mask unchanged.
human_collider = bpy.data.objects['Human'].copy()
human_collider.name = 'Offline full-human collider'
scene.collection.objects.link(human_collider)
human_collider.modifiers.remove(human_collider.modifiers['Delete.donitz_monk_robe'])
if human_collider.modifiers.get('Footwear skin coverage'):
    human_collider.modifiers.remove(human_collider.modifiers['Footwear skin coverage'])
human_collider.hide_render = True
human_collider.modifiers.new('Offline garment collision', 'COLLISION')
human_collider.collision.thickness_outer = .006
human_collider.collision.cloth_friction = 5
for name in ('Brown shoe L', 'Brown shoe R'):
    shoe = bpy.data.objects[name]
    shoe.modifiers.new('Offline garment collision', 'COLLISION')
    shoe.collision.thickness_outer = .006
if args.action == 'Death':
    floor_mesh = bpy.data.meshes.new('Offline ground')
    floor_mesh.from_pydata([(-10,-10,0),(10,-10,0),(10,10,0),(-10,10,0)], [], [(0,1,2,3)])
    floor_mesh.update()
    floor = bpy.data.objects.new('Offline ground', floor_mesh)
    scene.collection.objects.link(floor)
    floor.modifiers.new('Offline ground collision', 'COLLISION')
    floor.collision.thickness_outer = .002
cloth = lower.modifiers.new('Offline lower robe', 'CLOTH')
cloth.settings.quality = 8
cloth.settings.vertex_group_mass = pins.name
# Blender's native reference mass and air drag are used as solver parameters,
# not as a calibrated total fabric mass. Other force coefficients remain native.
area = sum(p.area for p in lower.data.polygons)
cloth.settings.mass = .3
cloth.settings.air_damping = 1.0
cloth.settings.tension_stiffness = 15
cloth.settings.compression_stiffness = 15
cloth.settings.shear_stiffness = 5
cloth.settings.bending_stiffness = .5
cloth.collision_settings.distance_min = .006
cloth.collision_settings.use_self_collision = True
cloth.collision_settings.self_distance_min = .004
cloth.point_cache.frame_start, cloth.point_cache.frame_end = 1, end_frame

# Initial nearest-surface measurements are diagnostic, not a proof of no
# intersections; signed nearest normals are ambiguous at open garment edges.
physics_frame_sets = [1]
scene.frame_set(1)
bpy.context.view_layer.update()
dg = bpy.context.evaluated_depsgraph_get()
leg_eval = trousers.evaluated_get(dg)
leg_mesh = leg_eval.to_mesh()
leg_points = [trousers.matrix_world @ v.co for v in leg_mesh.vertices]
leg_bvh = BVHTree.FromPolygons(leg_points, [tuple(p.vertices) for p in leg_mesh.polygons])
lower_eval = lower.evaluated_get(dg)
lower_mesh = lower_eval.to_mesh()
nearest = []
for v in lower_mesh.vertices:
    p = lower.matrix_world @ v.co
    hit, normal, _, distance = leg_bvh.find_nearest(p)
    if hit is not None:
        nearest.append((distance, (p-hit).dot(normal)))
skirt_points = [lower.matrix_world @ v.co for v in lower_mesh.vertices]
skirt_bvh = BVHTree.FromPolygons(skirt_points, [tuple(p.vertices) for p in lower_mesh.polygons])
lower.data.calc_loop_triangles()
triangles = np.array([tuple(p.vertices) for p in lower.data.loop_triangles], dtype=np.int32)
if finite:
    reference = np.load(args.reference_cache)
    initial = np.array([tuple(v.co) for v in lower_mesh.vertices], dtype=np.float32)
    if not np.array_equal(triangles, reference['triangles']) or not np.array_equal(initial, reference['poses'][0]):
        raise RuntimeError('Finite action changed the shared raw rest topology or initial positions')
    if not np.allclose(np.array(lower.matrix_world), np.eye(4), atol=0, rtol=0):
        raise RuntimeError('Cloth cache requires identity/world-coordinate object transform')
overlap_count = len(skirt_bvh.overlap(leg_bvh))
human = bpy.data.objects['Human']
body_mask = human.modifiers['Delete.donitz_monk_robe']
mask_state = body_mask.show_viewport
body_mask.show_viewport = False
bpy.context.view_layer.update()
body_eval = human.evaluated_get(bpy.context.evaluated_depsgraph_get())
body_mesh = body_eval.to_mesh()
body_tree = BVHTree.FromPolygons([human.matrix_world @ v.co for v in body_mesh.vertices],
                                [tuple(p.vertices) for p in body_mesh.polygons])
body_overlap = skirt_bvh.overlap(body_tree)
body_overlap_below_support = sum(
    1 for skirt_face, _ in body_overlap
    if sum(skirt_points[i].z for i in lower_mesh.polygons[skirt_face].vertices)
       / len(lower_mesh.polygons[skirt_face].vertices) < .87)
body_eval.to_mesh_clear()
body_mask.show_viewport = mask_state
bpy.context.view_layer.update()
shoe_overlap = 0
for name in ('Brown shoe L', 'Brown shoe R'):
    shoe = bpy.data.objects[name]
    evaluated = shoe.evaluated_get(dg)
    shoe_mesh = evaluated.to_mesh()
    shoe_tree = BVHTree.FromPolygons([shoe.matrix_world @ v.co for v in shoe_mesh.vertices],
                                    [tuple(p.vertices) for p in shoe_mesh.polygons])
    shoe_overlap += len(skirt_bvh.overlap(shoe_tree))
    evaluated.to_mesh_clear()
leg_eval.to_mesh_clear()
lower_eval.to_mesh_clear()
print('SETUP', json.dumps({'vertices': len(lower.data.vertices), 'faces': len(lower.data.polygons),
      'area_m2': area, 'solver_mass_per_vertex': cloth.settings.mass,
      'air_damping': cloth.settings.air_damping, 'initial_nearest_m': min(x[0] for x in nearest),
      'initial_signed_negative_count': sum(x[1] < 0 for x in nearest),
      'initial_overlapping_face_pairs': overlap_count,
      'initial_shoe_overlapping_face_pairs': shoe_overlap,
      'initial_human_overlaps_below_support': body_overlap_below_support,
      'floor_support':floor_support,
      'gait_pose_matrix_max_error': pose_error, 'drivers': drivers_before}), flush=True)
if overlap_count or shoe_overlap or body_overlap_below_support:
    raise RuntimeError('Initial lower garment intersects the trousers, shoes, or exposed lower human')

# Capture every simulation frame in monotonic global order before any render.
frames = []
intersection_counts = {}
for frame in range(1, end_frame+1):
    physics_frame_sets.append(frame)
    scene.frame_set(frame)
    bpy.context.view_layer.update()
    evaluated = lower.evaluated_get(bpy.context.evaluated_depsgraph_get())
    mesh = evaluated.to_mesh()
    if len(mesh.vertices) != len(lower.data.vertices):
        raise RuntimeError('Cloth vertex topology changed')
    co = np.empty(len(mesh.vertices)*3, dtype=np.float32)
    mesh.vertices.foreach_get('co', co)
    frames.append(co.reshape(-1,3))
    if review_start <= frame <= review_end:
        skirt_tree = BVHTree.FromPolygons([lower.matrix_world @ v.co for v in mesh.vertices],
                                          [tuple(p.vertices) for p in mesh.polygons])
        leg_eval = trousers.evaluated_get(bpy.context.evaluated_depsgraph_get())
        leg_mesh = leg_eval.to_mesh()
        leg_tree = BVHTree.FromPolygons([trousers.matrix_world @ v.co for v in leg_mesh.vertices],
                                        [tuple(p.vertices) for p in leg_mesh.polygons])
        intersection_counts[frame] = len(skirt_tree.overlap(leg_tree))
        leg_eval.to_mesh_clear()
    evaluated.to_mesh_clear()
    if frame % 15 == 0:
        print('SIM_FRAME', frame, flush=True)
frames = np.stack(frames)
if not np.isfinite(frames).all():
    raise RuntimeError('Non-finite cloth simulation')
lower.data.calc_loop_triangles()
if finite:
    sim_frames = np.arange(1, end_frame+1, dtype=np.int32)
    source_frames = np.minimum(np.maximum(sim_frames-action_start+1, 1), round(source_end)).astype(np.int32)
    source_actions = np.where(sim_frames < action_start, 'Idle', args.action)
    phases = np.where(sim_frames < action_start, 'idle_settle',
                      np.where(sim_frames <= action_start+period, 'action', 'final_hold'))
    np.savez_compressed(args.output/'cloth-cache.npz', poses=frames, triangles=triangles,
                        times=(sim_frames-1)/fps, simulation_frames=sim_frames,
                        source_frames=source_frames, source_actions=source_actions, phases=phases,
                        action=np.array(args.action), fps=np.int32(fps),
                        action_start_frame=np.int32(action_start),
                        action_end_frame=np.int32(action_start+period),
                        coordinate_space=np.array('identity object transform; Blender world metres; +Z up'))
    (args.output/'body-pose-checks.json').write_text(json.dumps({
        'action': args.action, 'fps': fps, 'checks': finite_pose_checks,
        'checked_deform_bones': tracked,
        'initial_matches_reference_cache': str(args.reference_cache.resolve()),
        'coordinates': 'identity object transform; Blender world metres; +Z up',
        'mapping': 'simulation_frames and times index poses; source_actions and source_frames specify body pose; final_hold clamps to action endpoint',
        'floor_support': floor_support,
        'note': 'Raw simulation, including Idle; no authored loop correction applied.'}, indent=2)+'\n')
else:
    np.savez_compressed(args.output/'cloth-cache.npz', poses=frames, triangles=triangles,
                        times=np.arange(len(frames))/fps,
                        action=np.array(args.action), period_frames=np.int32(period),
                        source_phase_start=np.int32(review_start))

# Freeze modifier results and prove render evaluation uses those exact samples.
review_lower = lower
if finite and lower.data.shape_keys is not None:
    # A plain duplicate prevents the animated floor-support keys from being
    # re-evaluated on top of the exact cached coordinates during rendering.
    review_mesh = bpy.data.meshes.new('Exact cached cloth review')
    review_mesh.from_pydata([tuple(v.co) for v in lower.data.vertices], [],
                            [tuple(p.vertices) for p in lower.data.polygons])
    review_mesh.update()
    review_lower = bpy.data.objects.new('Exact cached cloth review', review_mesh)
    scene.collection.objects.link(review_lower)
    review_lower.data.materials.append(lower.data.materials[0])
    for face in review_lower.data.polygons:
        face.use_smooth = True
    lower.hide_viewport = True
    lower.hide_render = True
else:
    for mod in lower.modifiers:
        mod.show_viewport = False
        mod.show_render = False
scene.render.engine = 'BLENDER_WORKBENCH'
scene.display.shading.light = 'STUDIO'
scene.display.shading.color_type = 'MATERIAL'
scene.display.shading.show_shadows = True
scene.display.shading.show_cavity = True
scene.display.shading.show_specular_highlight = False
scene.render.resolution_x, scene.render.resolution_y, scene.render.resolution_percentage = (
    (1024, 1280, 100) if finite else (480, 640, 100))
scene.render.image_settings.file_format = 'PNG'
center = Vector((0, 0, .9))
cam_data = bpy.data.cameras.new('Offline review camera')
cam = bpy.data.objects.new('Offline review camera', cam_data)
scene.collection.objects.link(cam)
scene.camera = cam
cam_data.type = 'ORTHO'
cam_data.ortho_scale = 2.2
parity = 0.
if finite:
    review_frames = sorted({1, fps, action_start, action_start+period, end_frame,
                            *(action_start+round(period*i/12) for i in range(13)),
                            *(min(end_frame, action_start+period+round(fps*i/4)) for i in range(1,5))})
else:
    review_frames = range(review_start, review_end+1, fps//30)
review_floor = []
for frame in review_frames:
    scene.frame_set(frame)
    review_lower.data.vertices.foreach_set('co', frames[frame-1].ravel())
    review_lower.data.update()
    bpy.context.view_layer.update()
    evaluated = review_lower.evaluated_get(bpy.context.evaluated_depsgraph_get())
    mesh = evaluated.to_mesh()
    check = np.empty(len(mesh.vertices)*3, dtype=np.float32)
    mesh.vertices.foreach_get('co', check)
    evaluated.to_mesh_clear()
    parity = max(parity, float(np.max(np.abs(check.reshape(-1,3)-frames[frame-1]))))
    if finite:
        body_eval = human.evaluated_get(bpy.context.evaluated_depsgraph_get())
        body_mesh = body_eval.to_mesh()
        points = [body_eval.matrix_world @ v.co for v in body_mesh.vertices]
        body_eval.to_mesh_clear()
        center = Vector(tuple((min(p[i] for p in points)+max(p[i] for p in points))/2 for i in range(3)))
        cam_data.ortho_scale = 2.5
        shoe_minima = {}
        for name in ('Brown shoe L', 'Brown shoe R'):
            obj = bpy.data.objects[name]
            shoe_eval = obj.evaluated_get(bpy.context.evaluated_depsgraph_get())
            shoe_mesh = shoe_eval.to_mesh()
            shoe_minima[name] = min((shoe_eval.matrix_world @ v.co).z for v in shoe_mesh.vertices)
            shoe_eval.to_mesh_clear()
        review_floor.append({'simulation_frame':frame, 'source_frame':int(source_frames[frame-1]),
                             'cloth_min_z':float(frames[frame-1,:,2].min()),
                             'visible_human_min_z':min(p.z for p in points), 'shoe_min_z':shoe_minima})
    for label, position in [('front',(0,-5,1.1)),('side',(5,0,1.1))]:
        cam.location = center+Vector(position) if finite else position
        cam.rotation_euler = (center-cam.location).to_track_quat('-Z','Y').to_euler()
        scene.render.filepath = str((args.output/f'{label}-{frame:03d}.png').resolve())
        bpy.ops.render.render(write_still=True)
if finite:
    (args.output/'review-metrics.json').write_text(json.dumps({
        'cache_render_max_error_m':parity, 'review_frames':review_floor,
        'cloth_min_z_all_frames':float(frames[:,:,2].min()),
        'trouser_overlap_face_pairs':intersection_counts,
        'drivers_before':drivers_before, 'drivers_after':len(rig.animation_data.drivers)}, indent=2)+'\n')
print('RESULT', json.dumps({'frames': len(frames), 'render_sample_max_error_m': parity,
      'sampled_trouser_overlapping_face_pairs':intersection_counts,
      'physics_frame_set_sequence': physics_frame_sets,
      'drivers':len(rig.animation_data.drivers), 'fps':fps, 'action':args.action,
      'period_frames':period, 'review_start':review_start}), flush=True)
