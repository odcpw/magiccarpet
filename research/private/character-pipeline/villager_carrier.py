"""Author and audit the Level 1 vessel carrier from the accepted villager master.

Run inside Blender against the verified upper-prepared.blend with --author OUT;
then reopen OUT/master/VillagerCarrier.blend and run --audit OUT. Both modes
refuse to replace their outputs. The pot is a separate editable skinned mesh;
its dedicated bone follows the head except for the authored Death release.
"""
import argparse
import hashlib
import json
import math
import sys
from pathlib import Path

import bpy
from mathutils import Matrix, Quaternion, Vector

SOURCE_SHA = 'c36f1f0f8fcec31d9e0364faa5f8eacd4d45657d95aaf4796caf8dc52ab765c0'
ACTIONS = ('Idle', 'Walk', 'Run', 'Hit', 'Death')
MESHES = ['Human', 'Human.donitz_monk_robe', 'Walk skeletal lower cloth',
          'Separate olive trousers', 'Eye L', 'Iris L', 'Eye R', 'Iris R',
          'Turban crown', 'Turban wrap 1', 'Turban wrap 2', 'Turban wrap 3',
          'Right turban drape', 'Left turban drape', 'Nape wrap',
          'Brown shoe L', 'Brown shoe R', 'Carrier veil', 'Carrier vessel']
BONE = 'Carrier vessel bone'
HEAD = 'DEF-spine.006'


def vertices(obj):
    ev = obj.evaluated_get(bpy.context.evaluated_depsgraph_get())
    mesh = ev.to_mesh()
    try:
        used = {i for p in mesh.polygons for i in p.vertices}
        return [ev.matrix_world @ mesh.vertices[i].co for i in sorted(used)]
    finally:
        ev.to_mesh_clear()


def action(rig, name):
    a = bpy.data.actions[name]
    rig.animation_data.action = a
    rig.animation_data.action_slot = a.slots[0]
    return a


def create_pot(rig):
    # Closed lathed pottery profile, with an inset mouth and a thick lip. z is
    # the original rig's world-space rest height. Its foot sits on the wrap.
    profile = [(0.0, 1.781), (.085, 1.781), (.13, 1.800), (.17, 1.831),
               (.203, 1.882), (.218, 1.935), (.215, 1.985), (.19, 2.032),
               (.15, 2.070), (.12, 2.089), (.12, 2.110), (.145, 2.117),
               (.145, 2.130), (.094, 2.130), (.091, 2.115), (.091, 2.100),
               (.0, 2.100)]
    n = 48
    vs = [(r*math.cos(2*math.pi*j/n), -.04+r*math.sin(2*math.pi*j/n), z)
          for r,z in profile for j in range(n)]
    faces = [(i*n+j, i*n+(j+1)%n, (i+1)*n+(j+1)%n, (i+1)*n+j)
             for i in range(len(profile)-1) for j in range(n)]
    mesh = bpy.data.meshes.new('Hand-thrown carrier vessel mesh')
    mesh.from_pydata(vs, [], faces); mesh.update()
    obj = bpy.data.objects.new('Carrier vessel', mesh)
    bpy.context.scene.collection.objects.link(obj)
    for p in mesh.polygons: p.use_smooth = True
    mat = bpy.data.materials.new('Warm tan earthenware'); mat.use_nodes = True
    shader = mat.node_tree.nodes.get('Principled BSDF')
    shader.inputs['Base Color'].default_value = (.39, .205, .085, 1)
    shader.inputs['Roughness'].default_value = .86
    obj.data.materials.append(mat)
    group = obj.vertex_groups.new(name=BONE); group.add(list(range(len(vs))), 1.0, 'REPLACE')
    mod = obj.modifiers.new('Carrier LBS', 'ARMATURE'); mod.object = rig
    return obj



def create_veil(rig):
    # A separate front drape covers the lower face and opens at the eyes,
    # widening gently over the neck as in the daylight source sheet.
    cols, rows = 17, 7
    vs=[]
    for row in range(rows):
        t=row/(rows-1)
        z=1.603 - .165*t
        width=.079 + .027*t
        for col in range(cols):
            u=2*col/(cols-1)-1
            x=width*u
            y=-.190 + max(0.0, 1.54-z)*.95 + .055*(abs(u)**1.7)
            zfold=z + .004*math.cos(u*math.pi*2)*(t*.7+.3)
            vs.append((x,y,zfold))
    faces=[]
    for row in range(rows-1):
        for col in range(cols-1):
            i=row*cols+col; faces.append((i,i+1,i+1+cols,i+cols))
    mesh=bpy.data.meshes.new('Carrier veil mesh'); mesh.from_pydata(vs,[],faces); mesh.update()
    obj=bpy.data.objects.new('Carrier veil',mesh); bpy.context.scene.collection.objects.link(obj)
    for poly in mesh.polygons: poly.use_smooth=True
    obj.data.materials.append(bpy.data.materials['Ivory woven wrap'])
    g=obj.vertex_groups.new(name=HEAD); g.add(list(range(len(vs))),1,'REPLACE')
    mod=obj.modifiers.new('Veil LBS','ARMATURE'); mod.object=rig

def author(out):
    source = Path(bpy.data.filepath)
    if hashlib.sha256(source.read_bytes()).hexdigest() != SOURCE_SHA:
        raise ValueError('Not the accepted upper-prepared master')
    if out.exists(): raise FileExistsError(out)
    (out/'master').mkdir(parents=True)
    rig = bpy.data.objects['VillagerRig']; scene = bpy.context.scene
    bpy.data.objects['Front neck wrap'].hide_render = True
    # Desaturated warm brown from the daylight sheet; preserve cloth geometry.
    mat = bpy.data.materials['Olive ochre robe']
    mat.name = 'Deep brown carrier robe'
    mat.node_tree.nodes.get('Principled BSDF').inputs['Base Color'].default_value = (.052, .027, .020, 1)
    mat.node_tree.nodes.get('Principled BSDF').inputs['Roughness'].default_value = .92
    wrap = bpy.data.materials['Ivory woven wrap']
    wrap.node_tree.nodes.get('Principled BSDF').inputs['Base Color'].default_value = (.56, .57, .66, 1)
    wrap.node_tree.nodes.get('Principled BSDF').inputs['Roughness'].default_value = .9
    bpy.ops.object.select_all(action='DESELECT'); rig.select_set(True)
    bpy.context.view_layer.objects.active = rig
    bpy.ops.object.mode_set(mode='EDIT')
    bone = rig.data.edit_bones.new(BONE); bone.head = (0, -.04, 1.781); bone.tail = (0, -.04, 2.0)
    bone.parent = rig.data.edit_bones[HEAD]; bone.use_deform = True
    bpy.ops.object.mode_set(mode='OBJECT')
    pot = create_pot(rig)
    create_veil(rig)
    rest = rig.data.bones[BONE].matrix_local.copy()
    headrest = rig.data.bones[HEAD].matrix_local.copy()
    scene.frame_set(1)
    # Each source action retains its own original control/cloth curves. The
    # pot bone has neutral pose by default; only Death needs independent keys.
    action(rig, 'Death')
    scene.frame_set(56); bpy.context.view_layer.update()
    head = rig.evaluated_get(bpy.context.evaluated_depsgraph_get()).pose.bones[HEAD].matrix.copy()
    release = head @ headrest.inverted()
    _, start_q, _ = release.decompose()
    # Settle at the body's right side: the pot rotates onto its side, and its
    # exact transformed mesh support is raised over z=0 at every authored key.
    end_q = Quaternion((0, 1, 0), math.radians(88)) @ start_q
    source_points = [v.co.copy() for v in pot.data.vertices]
    center = Vector((0, -.04, 1.95))
    start_center = release @ center
    end_center = Vector((start_center.x + .69, start_center.y - .10, .19))
    pb = rig.pose.bones[BONE]; pb.rotation_mode = 'QUATERNION'
    for frame in range(56, 265):
        scene.frame_set(frame); bpy.context.view_layer.update()
        head_pose = rig.evaluated_get(bpy.context.evaluated_depsgraph_get()).pose.bones[HEAD].matrix.copy()
        if frame == 56:
            desired = release @ rest
        else:
            t = min(1.0, (frame-56)/92)
            ease = t*t*(3-2*t)
            q = start_q.slerp(end_q, ease)
            # Lateral travel starts with the release; accelerated downward arc.
            c = start_center.lerp(end_center, ease)
            m = Matrix.Translation(c) @ q.to_matrix().to_4x4() @ Matrix.Translation(-center)
            low = min((m @ v).z for v in source_points)
            if low < .003: m.translation.z += .003-low
            desired = m @ rest
        # Convert absolute desired pose to a local animation transform under the
        # currently evaluated head, keeping exportable ordinary LBS semantics.
        basis = pb.bone.convert_local_to_pose(desired, pb.bone.matrix_local,
            parent_matrix=head_pose, parent_matrix_local=rig.data.bones[HEAD].matrix_local, invert=True)
        pb.matrix_basis = basis
        for prop in ('location', 'rotation_quaternion', 'scale'):
            pb.keyframe_insert(data_path=prop, frame=frame, group=BONE)
    # Neutral keys at the beginning preserve the same supported pot at the
    # initial Death pose; linear keys between 1 and 56 remain neutral.
    for frame in (1, 55):
        pb.location = (0,0,0); pb.rotation_quaternion = (1,0,0,0); pb.scale = (1,1,1)
        for prop in ('location', 'rotation_quaternion', 'scale'):
            pb.keyframe_insert(data_path=prop, frame=frame, group=BONE)
    for layer in bpy.data.actions['Death'].layers:
        for strip in layer.strips:
            for bag in strip.channelbags:
                for curve in bag.fcurves:
                    if BONE in curve.data_path:
                        for point in curve.keyframe_points: point.interpolation = 'LINEAR'
    action(rig, 'Idle'); scene.frame_set(1)
    bpy.ops.wm.save_as_mainfile(filepath=str((out/'master/VillagerCarrier.blend').resolve()))
    print('CARRIER_MASTER', out/'master/VillagerCarrier.blend', flush=True)


def audit(out):
    path = out/'costume.json'
    if path.exists(): raise FileExistsError(path)
    scene = bpy.context.scene; rig = bpy.data.objects['VillagerRig']
    meshes = [bpy.data.objects[n] for n in MESHES]
    report = {'meshes': MESHES, 'fps':scene.render.fps/scene.render.fps_base,
              'clips':{}, 'hitOverlayBones':['DEF-spine.003','DEF-spine.004','DEF-spine.005','DEF-spine.006']}
    for name in ACTIONS:
        a = action(rig, name); first,last=map(round,a.frame_range); lo=math.inf; potlo=math.inf
        worst = None
        for step in range(first*2, last*2+1):
            scene.frame_set(step//2, subframe=(step%2)*.5); bpy.context.view_layer.update()
            lower=min(p.z for obj in meshes for p in vertices(obj))
            pmin=min(p.z for p in vertices(meshes[-1]))
            if lower < lo: lo=lower; worst=step/2
            potlo=min(potlo,pmin)
            if name == 'Idle' and step == first*2:
                report['heightAboveGroundOrigin']=max(p.z for obj in meshes for p in vertices(obj))
        report['clips'][name]={'first':first, 'last':last,'minimum':lo, 'vesselMinimum':potlo,'worstFrame':worst,'sampleStep':.5}
        print('COSTUME',name,report['clips'][name],flush=True)
        if lo < -.003: raise ValueError('Floor penetration: '+name)
    path.write_text(json.dumps(report,indent=2)+'\n')



def preview(out):
    folder=out/'review-framed'
    folder.mkdir(exist_ok=False)
    scene=bpy.context.scene; rig=bpy.data.objects['VillagerRig']
    for obj in scene.objects:
        if obj.type == 'MESH': obj.hide_render = obj.name not in MESHES
    for mat in bpy.data.materials:
        if mat.use_nodes:
            node=mat.node_tree.nodes.get('Principled BSDF')
            if node: mat.diffuse_color=node.inputs['Base Color'].default_value
    scene.render.engine='BLENDER_WORKBENCH'
    scene.display.shading.light='STUDIO'
    scene.display.shading.color_type='MATERIAL'
    scene.display.shading.show_shadows=True
    scene.display.shading.show_cavity=True
    scene.render.resolution_x=500; scene.render.resolution_y=640; scene.render.resolution_percentage=100
    scene.render.image_settings.file_format='PNG'
    bpy.ops.object.camera_add(); camera=bpy.context.object; scene.camera=camera
    camera.data.type='ORTHO'; camera.data.ortho_scale=2.7
    for clip, frame in [('Idle',1),('Walk',31),('Run',20),('Hit',31),('Death',56),('Death',100),('Death',148),('Death',264)]:
        action(rig,clip); scene.frame_set(frame); bpy.context.view_layer.update()
        points=[point for obj in (bpy.data.objects[n] for n in MESHES) for point in vertices(obj)]
        low=Vector(tuple(min(p[i] for p in points) for i in range(3)))
        high=Vector(tuple(max(p[i] for p in points) for i in range(3)))
        target=(low+high)*.5
        for label, offset in [('front',(0,-5,0)),('side',(5,0,0))]:
            width=(high.x-low.x) if label=='front' else (high.y-low.y)
            camera.data.ortho_scale=max(2.5, (high.z-low.z)*1.2, width*1.65)
            camera.location=target+Vector(offset)
            camera.rotation_euler=(target-camera.location).to_track_quat('-Z','Y').to_euler()
            scene.render.filepath=str((folder/(clip+str(frame)+'-'+label+'.png')).resolve())
            bpy.ops.render.render(write_still=True)
            print('RENDER',clip,frame,label,flush=True)


def package(out):
    import importlib.util
    pipeline=Path(__file__).resolve().parent
    def module(name):
        spec=importlib.util.spec_from_file_location(name,pipeline/(name+'.py'))
        mod=importlib.util.module_from_spec(spec); spec.loader.exec_module(mod); return mod
    runtime=out/'runtime'
    if Path(bpy.data.filepath).resolve() != (out/'master/VillagerCarrier.blend').resolve():
        raise ValueError('Package requires the selected output master')
    measure=json.loads((out/'costume.json').read_text())
    # export_constants atomically refuses an existing directory.
    maps=module('character_textures').export_constants(runtime, MESHES)
    destination=runtime/'VillagerCarrier.fbx'
    sys.argv=['export_character_fbx.py','--','--output',str(destination),'--animations',
              '--bake-rig','VillagerRig','--join-skins']
    for name in MESHES: sys.argv += ['--mesh', name]
    for name in ACTIONS: sys.argv += ['--action', name]
    module('export_character_fbx').main()
    report=json.loads(destination.with_suffix('.export.json').read_text())
    root=pipeline.parents[2]
    speeds=json.loads((root/'research/private/generated/human-figure-20261109/mpfb-motion02/measurements-audited.json').read_text())
    manifest={'height':measure['heightAboveGroundOrigin'], 'walkSpeed':speeds['walkSpeed'],
              'runSpeed':speeds['runSpeed'], 'hitOverlayBones':measure['hitOverlayBones'],
              'minBoneWeight':1e-7, 'clips':report['clips'],
              'materials':[{k:m[k] for k in ('name','albedo','orm')} for m in maps['materials']]}
    (runtime/'VillagerCarrier.clips.json').write_text(json.dumps(manifest,indent=2)+'\n')
    receipt={'schema':'mc-villager-carrier-package/v1', 'inputSha256':SOURCE_SHA,
             'masterSha256':hashlib.sha256((out/'master/VillagerCarrier.blend').read_bytes()).hexdigest(),
             'fbxSha256':hashlib.sha256(destination.read_bytes()).hexdigest(),
             'heightIncludingVessel':manifest['height'], 'deformBones':report['deformBones'],
             'sourceToGameMaxMeters':max(x['maxError'] for x in report['surfaceProof']),
             'blenderFbxMaxMeters':max(x['maxError'] for x in report['reimportSurfaceProof']),
             'costumeMinimumMeters':min(x['minimum'] for x in measure['clips'].values()),
             'scope':'Blender authoring and FBX reimport; Unity and gameplay not yet verified'}
    (out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
    print('CARRIER_PACKAGE',json.dumps(receipt),flush=True)

def main():
    p=argparse.ArgumentParser(); p.add_argument('--author',type=Path); p.add_argument('--audit',type=Path); p.add_argument('--preview',type=Path); p.add_argument('--package',type=Path)
    args=p.parse_args(sys.argv[sys.argv.index('--')+1:])
    if sum(bool(x) for x in (args.author,args.audit,args.preview,args.package)) != 1: p.error('choose exactly one mode')
    if args.author: author(args.author.resolve())
    elif args.audit: audit(args.audit.resolve())
    elif args.preview: preview(args.preview.resolve())
    else: package(args.package.resolve())

if __name__=='__main__': main()
