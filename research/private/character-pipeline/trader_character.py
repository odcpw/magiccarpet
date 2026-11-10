"""Author and package the source-inspired Level 1 Trader on the MPFB Rigify master.

Run --author FRESH_DIR on the accepted upper-prepared blend. Reopen the resulting
FRESH_DIR/master/Trader.blend for --audit, --gait, --contacts, --preview, and
finally --package, each using that same FRESH_DIR. Each stage refuses to
overwrite its outputs. Separate editable anatomy, clothing and load are
preserved; the portable package joins only at export time.
"""
import argparse
import hashlib
import json
import math
import subprocess
import sys
from pathlib import Path

import bmesh
import bpy
import numpy as np
from mathutils import Matrix, Quaternion, Vector
from mathutils.bvhtree import BVHTree

SOURCE_SHA = 'c36f1f0f8fcec31d9e0364faa5f8eacd4d45657d95aaf4796caf8dc52ab765c0'
SPEED_SHA = '554a18fe551c44adfa1da18363958a19d90ee7331f33a882f204540c14346fd5'
CLIPS = ('Idle', 'Walk', 'Run', 'Hit', 'Death')
BODY = ('Human', 'Separate olive trousers', 'Eye L', 'Iris L', 'Eye R', 'Iris R', 'Brown shoe L', 'Brown shoe R')
PACK_BONE = 'DEF-TraderPack'
PARENT = 'DEF-spine.004'


def action(rig, name):
    rig.animation_data.action = bpy.data.actions[name]
    rig.animation_data.action_slot = rig.animation_data.action.slots[0]


def material(name, color, roughness=.9):
    m = bpy.data.materials.new(name)
    m.diffuse_color = (*color, 1)
    m.use_nodes = True
    p = m.node_tree.nodes.get('Principled BSDF')
    p.inputs['Base Color'].default_value = (*color, 1)
    p.inputs['Roughness'].default_value = roughness
    p.inputs['Metallic'].default_value = 0
    return m


def skin(obj, rig, bone, mat):
    obj.data.materials.clear(); obj.data.materials.append(mat)
    g = obj.vertex_groups.new(name=bone)
    g.add(list(range(len(obj.data.vertices))), 1, 'REPLACE')
    mod = obj.modifiers.new('Trader rig', 'ARMATURE'); mod.object = rig
    for face in obj.data.polygons: face.use_smooth = True
    return obj


def ellipsoid(name, center, radii, rig, bone, mat):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=24, ring_count=12)
    obj = bpy.context.object; obj.name = name
    for v in obj.data.vertices:
        v.co = Vector((center[0]+v.co.x*radii[0], center[1]+v.co.y*radii[1], center[2]+v.co.z*radii[2]))
    return skin(obj, rig, bone, mat)


def rod(name, a, b, radius, rig, bone, mat):
    a, b = Vector(a), Vector(b)
    bpy.ops.mesh.primitive_cylinder_add(vertices=12, radius=radius, depth=(b-a).length)
    obj=bpy.context.object; obj.name=name
    q=(b-a).to_track_quat('Z','Y')
    for v in obj.data.vertices: v.co = q @ v.co + (a+b)/2
    return skin(obj, rig, bone, mat)


def band(name, points, width, rig, bone, mat):
    # Flat cloth/leather strip whose geometry remains visible in exported UV maps.
    verts=[]
    for x,y,z in points: verts.extend([(x-width/2,y,z),(x+width/2,y,z)])
    faces=[(2*i,2*i+1,2*i+3,2*i+2) for i in range(len(points)-1)]
    mesh=bpy.data.meshes.new(name); mesh.from_pydata(verts, [], faces); mesh.update()
    obj=bpy.data.objects.new(name,mesh); bpy.context.collection.objects.link(obj)
    return skin(obj,rig,bone,mat)


def linear_curves(action, match):
    for layer in action.layers:
        for strip in layer.strips:
            for bag in strip.channelbags:
                for curve in bag.fcurves:
                    if any(s in curve.data_path for s in match):
                        for key in curve.keyframe_points: key.interpolation='LINEAR'


def raise_arms(rig):
    # Override only FK arm controls, retaining the leg/root motion at 120 Hz.
    # The hands are brought in and up to grip straps in each standing action.
    scene=bpy.context.scene
    for name in ('Idle','Walk','Run','Hit'):
        action(rig,name)
        first,last=map(round,rig.animation_data.action.frame_range)
        for frame in range(first,last+1):
            scene.frame_set(frame)
            for side,sign in [('L',1),('R',-1)]:
                upper=rig.pose.bones['upper_arm_fk.'+side]
                upper.rotation_mode='QUATERNION'
                q=upper.bone.matrix_local.to_quaternion()
                upper.rotation_quaternion=q.inverted() @ Quaternion((1,0,0),-.25) @ Quaternion((0,1,0),sign*.90) @ q
                upper.keyframe_insert(data_path='rotation_quaternion',frame=frame,group=upper.name)
                bpy.context.view_layer.update()
                fore=rig.pose.bones['forearm_fk.'+side]; fore.rotation_mode='QUATERNION'
                q=fore.bone.matrix_local.to_quaternion()
                d=(fore.parent.matrix.to_3x3() @ fore.parent.bone.matrix_local.to_3x3().inverted()).to_quaternion()
                fore.rotation_quaternion=q.inverted() @ d.inverted() @ Quaternion((1,0,0),-1.48) @ d @ q
                fore.keyframe_insert(data_path='rotation_quaternion',frame=frame,group=fore.name)
            if frame in (first,last):
                for side in ('L','R'):
                    for finger in ('f_index','f_middle','f_ring','f_pinky'):
                        for segment in ('01','02','03'):
                            control=rig.pose.bones[f'{finger}.{segment}.{side}']
                            control.rotation_mode='XYZ';control.rotation_euler.x=.45
                            control.keyframe_insert(data_path='rotation_euler',frame=frame,group=control.name)
        linear_curves(rig.animation_data.action,['upper_arm_fk.', 'forearm_fk.'])


def author(out):
    if out.exists(): raise FileExistsError(out)
    if hashlib.sha256(Path(bpy.data.filepath).read_bytes()).hexdigest()!=SOURCE_SHA:
        raise ValueError('Expected accepted upper-prepared source')
    out.mkdir(parents=True)
    rig=bpy.data.objects['VillagerRig']; scene=bpy.context.scene
    for o in bpy.data.objects:
        if o.type=='MESH' and o.name not in BODY: o.hide_render=True; o.hide_viewport=True
    body=bpy.data.objects['Human']
    # Preserve anatomical skin previously hidden specifically by the long robe.
    m=body.modifiers['Delete.donitz_monk_robe']; m.show_render=False; m.show_viewport=False
    pale=material('Trader pale linen',(.65,.64,.62)); patch=material('Trader worn tan patches',(.40,.25,.14))
    green=material('Trader green vest',(.055,.24,.045)); leather=material('Trader worn leather',(.20,.095,.035))
    wood=material('Trader pack wood',(.18,.085,.033)); blue=material('Trader blue bedroll',(.08,.21,.38))
    cap=material('Trader slate blue cap',(.21,.26,.31)); sack=material('Trader tan sack',(.40,.27,.13))
    pants=bpy.data.objects['Separate olive trousers']; pants.data.materials.clear(); pants.data.materials.append(pale);pants.data.materials.append(patch)
    # Add fullness at the leg while preserving authored skin weights.
    for v in pants.data.vertices:
        if v.co.z < .86:
            center=math.copysign(.16,v.co.x) if abs(v.co.x)>.04 else 0
            fullness=1.16+.24*math.sin(math.pi*max(0,min(1,(v.co.z-.06)/.80)))
            fullness*=1+.035*math.sin(v.co.z*48)
            v.co.x=center+(v.co.x-center)*fullness
            v.co.y*=fullness
    # Contrasting irregular sewn patches on actual cloth faces, not separate decals.
    counts=0
    for poly in pants.data.polygons:
        c=poly.center
        if ((.09<c.x<.24 and -.17<c.y<-.045 and .38<c.z<.48) or
            (-.23<c.x<-.10 and -.17<c.y<-.04 and .60<c.z<.69)):
            poly.material_index=1; counts+=1
    if counts<4: raise ValueError(f'No trouser patches: {counts}')
    # Duplicate the separately skinned robe top, crop to a sleeveless short vest.
    base=bpy.data.objects['Human.donitz_monk_robe']; vest=base.copy(); vest.data=base.data.copy()
    bpy.context.collection.objects.link(vest);vest.name='Trader sleeveless vest';vest.hide_render=False;vest.hide_viewport=False
    bm=bmesh.new();bm.from_mesh(vest.data)
    remove=[f for f in bm.faces if f.calc_center_median().z < .985 or abs(f.calc_center_median().x)>.215]
    bmesh.ops.delete(bm,geom=remove,context='FACES')
    # The donor robe has a disconnected broad over-cowl; remove that component
    # while retaining the fitted torso and small shoulder-facing portions.
    pending=set(bm.faces);components=[]
    while pending:
        face=pending.pop(); stack=[face]; component=[face]
        while stack:
            for edge in stack.pop().edges:
                for neighbor in edge.link_faces:
                    if neighbor in pending:
                        pending.remove(neighbor);stack.append(neighbor);component.append(neighbor)
        components.append(component)
    components.sort(key=len,reverse=True)
    if len(components)<2 or not (1000<len(components[1])<2000):raise ValueError('Unexpected donor cowl topology')
    bmesh.ops.delete(bm,geom=components[1],context='FACES')
    bm.to_mesh(vest.data);bm.free();vest.data.update()
    vest.data.materials.clear();vest.data.materials.append(green)
    ellipsoid('Trader cap',(0,-.047,1.698),(.099,.109,.058),rig,'DEF-spine.006',cap)
    ellipsoid('Trader cap brim',(0,-.058,1.661),(.101,.115,.012),rig,'DEF-spine.006',cap)
    # New single rigid pack bone inherits the chest; its independent Death track
    # allows the load to settle beside the falling person instead of under them.
    bpy.ops.object.select_all(action='DESELECT');rig.select_set(True);bpy.context.view_layer.objects.active=rig
    bpy.ops.object.mode_set(mode='EDIT')
    b=rig.data.edit_bones.new(PACK_BONE); b.head=(0,.16,1.25);b.tail=(0,.16,1.45);b.parent=rig.data.edit_bones[PARENT];b.use_deform=True
    bpy.ops.object.mode_set(mode='OBJECT')
    pack=[]
    def add(o): pack.append(o);return o
    add(ellipsoid('Trader huge pack',(0,.31,1.35),(.235,.205,.36),rig,PACK_BONE,leather))
    add(ellipsoid('Trader sack left',(.215,.36,1.12),(.12,.13,.18),rig,PACK_BONE,sack))
    add(ellipsoid('Trader sack right',(-.215,.36,1.12),(.12,.13,.18),rig,PACK_BONE,sack))
    add(ellipsoid('Trader bundle low',(0,.48,1.17),(.14,.10,.13),rig,PACK_BONE,cap))
    add(ellipsoid('Trader bedroll left',(.155,.33,1.68),(.08,.17,.095),rig,PACK_BONE,blue))
    add(ellipsoid('Trader bedroll right',(-.155,.33,1.68),(.08,.17,.095),rig,PACK_BONE,blue))
    add(rod('Trader crossed pole A',(-.22,.38,1.52),(.23,.38,1.91),.019,rig,PACK_BONE,wood))
    add(rod('Trader crossed pole B',(.22,.42,1.52),(-.23,.42,1.91),.019,rig,PACK_BONE,wood))
    for sign in (-1,1):
        add(rod('Trader bedroll tie '+str(sign),(sign*.155,.18,1.67),(sign*.155,.49,1.67),.012,rig,PACK_BONE,leather))
        add(rod('Trader sack cord '+str(sign),(sign*.215,.25,1.23),(sign*.215,.48,1.23),.010,rig,PACK_BONE,wood))
        points=[]
        for i in range(13):
            z=1.07+i*.045; x=sign*.10
            y=.31+.205*math.sqrt(max(0,1-(x/.235)**2-((z-1.35)/.36)**2))+.01
            points.append((x,y,z))
        add(band('Trader pack binding '+str(sign),points,.028,rig,PACK_BONE,wood))
    add(rod('Trader load cross strap',(-.19,.49,1.42),(.19,.49,1.42),.014,rig,PACK_BONE,wood))
    for side,sign in [('left',1),('right',-1)]:
        band('Trader shoulder strap '+side,[(sign*.14,.10,1.47),(sign*.14,-.04,1.48),(sign*.115,-.27,1.405),(sign*.115,-.23,1.34),(sign*.10,-.20,1.08)],.045,rig,PARENT,leather)
    raise_arms(rig)
    # Author a released load in world space; collision support uses all pack vertices.
    action(rig,'Death'); scene.frame_set(10);bpy.context.view_layer.update()
    rest=rig.data.bones[PACK_BONE].matrix_local.copy();parentrest=rig.data.bones[PARENT].matrix_local.copy()
    parent=rig.evaluated_get(bpy.context.evaluated_depsgraph_get()).pose.bones[PARENT].matrix.copy()
    release=parent @ parentrest.inverted(); _,start_q,_=release.decompose()
    center=Vector((0,.31,1.35)); start_center=release@center
    end_q=Quaternion((0,1,0),math.radians(85)) @ start_q
    end_center=Vector((.88,.48,.28)); points=[v.co.copy() for o in pack for v in o.data.vertices]
    pb=rig.pose.bones[PACK_BONE];pb.rotation_mode='QUATERNION'
    for frame in range(1,265):
        scene.frame_set(frame);bpy.context.view_layer.update()
        parent=rig.evaluated_get(bpy.context.evaluated_depsgraph_get()).pose.bones[PARENT].matrix.copy()
        # The load follows the chest as the fall starts, then slips laterally
        # clear while descending; it must not sweep through the falling body.
        t=min(1,max(0,(frame-10)/80));ease=t*t*(3-2*t)
        lateral=min(1,max(0,(frame-10)/60));lateral=1-(1-lateral)**2
        q=start_q.slerp(end_q,ease)
        c=start_center.lerp(end_center,ease)
        c.x=start_center.x+(end_center.x-start_center.x)*lateral
        c.y=start_center.y+(end_center.y-start_center.y)*lateral
        seconds=max(0,frame-10)/(scene.render.fps/scene.render.fps_base)
        c.z=start_center.z-.5*9.81*seconds*seconds
        transform=(parent @ parentrest.inverted() if frame<=10 else
                   Matrix.Translation(c) @ q.to_matrix().to_4x4() @ Matrix.Translation(-center))
        low=min((transform@v).z for v in points)
        if low<.003:transform.translation.z+=.003-low
        desired=transform@rest
        pb.matrix_basis=pb.bone.convert_local_to_pose(desired,pb.bone.matrix_local,parent_matrix=parent,parent_matrix_local=parentrest,invert=True)
        for prop in ('location','rotation_quaternion','scale'):pb.keyframe_insert(data_path=prop,frame=frame,group=PACK_BONE)
    linear_curves(rig.animation_data.action,[PACK_BONE])
    # Separate actions must explicitly restore the attached load after Death.
    for name in ('Idle','Walk','Run','Hit'):
        action(rig,name); first,last=map(round,rig.animation_data.action.frame_range)
        for frame in (first,last):
            scene.frame_set(frame);pb.location=(0,0,0);pb.rotation_quaternion=(1,0,0,0);pb.scale=(1,1,1)
            for prop in ('location','rotation_quaternion','scale'):pb.keyframe_insert(data_path=prop,frame=frame,group=PACK_BONE)
        linear_curves(rig.animation_data.action,[PACK_BONE])
    action(rig,'Idle');scene.frame_set(1);bpy.context.view_layer.update()
    path=out/'master'/'Trader.blend';path.parent.mkdir()
    bpy.ops.wm.save_as_mainfile(filepath=str(path.resolve()),compress=True)
    print('AUTHORED',path,'patch faces',counts,flush=True)


def meshes():
    return [bpy.data.objects[n] for n in BODY]+[o for o in bpy.data.objects if o.type=='MESH' and o.name.startswith('Trader ') and not o.hide_render]


def require_master(out):
    if Path(bpy.data.filepath).resolve()!=(out/'master'/'Trader.blend').resolve():
        raise ValueError('Open the selected output master for this operation')


def preview(out):
    require_master(out)
    folder=out/'review';folder.mkdir(exist_ok=False)
    scene=bpy.context.scene;rig=bpy.data.objects['VillagerRig'];items=meshes()
    for obj in scene.objects:
        if obj.type=='MESH':obj.hide_render=obj not in items
    scene.render.engine='CYCLES';scene.cycles.device='CPU';scene.cycles.samples=8
    scene.cycles.use_denoising=True;scene.render.resolution_x=500;scene.render.resolution_y=600
    scene.render.resolution_percentage=100;scene.render.image_settings.file_format='PNG'
    world=bpy.data.worlds.new('Trader review');world.use_nodes=True
    world.node_tree.nodes['Background'].inputs[0].default_value=(.30,.30,.30,1)
    world.node_tree.nodes['Background'].inputs[1].default_value=.8;scene.world=world
    bpy.ops.mesh.primitive_plane_add(size=200,location=(0,0,-.002));floor=bpy.context.object
    floor.name='Review ground';floor.data.materials.append(material('Review ground grey',(.20,.20,.20)))
    for name,location,power,size in [('Key',(2,-4,5),550,4),('Fill',(-3,-1,3),350,4),('Rim',(1,3,4),450,3)]:
        data=bpy.data.lights.new(name,'AREA');data.energy=power;data.shape='DISK';data.size=size
        light=bpy.data.objects.new(name,data);scene.collection.objects.link(light);light.location=location
        light.rotation_euler=(Vector((0,0,1))-light.location).to_track_quat('-Z','Y').to_euler()
    bpy.ops.object.camera_add();camera=bpy.context.object;scene.camera=camera;camera.data.type='ORTHO'
    for clip,frame in [('Idle',1),('Walk',31),('Walk',91),('Run',1),('Run',20),('Run',38),('Run',58),('Hit',31),('Death',35),('Death',70),('Death',120),('Death',264)]:
        action(rig,clip);scene.frame_set(frame);bpy.context.view_layer.update();deps=bpy.context.evaluated_depsgraph_get();points=[]
        for obj in items:
            ev=obj.evaluated_get(deps);mesh=ev.to_mesh()
            try:points.extend(ev.matrix_world@v.co for v in mesh.vertices)
            finally:ev.to_mesh_clear()
        lo=Vector(tuple(min(p[i] for p in points) for i in range(3)))
        hi=Vector(tuple(max(p[i] for p in points) for i in range(3)));target=(lo+hi)/2
        for label,offset in [('front',(0,-5,1)),('side',(5,0,1)),('back',(0,5,1))]:
            width=(hi.x-lo.x) if label!='side' else (hi.y-lo.y)
            camera.data.ortho_scale=max(2.1,(hi.z-lo.z)*1.18,width*1.45)
            camera.location=target+Vector(offset);camera.rotation_euler=(target-camera.location).to_track_quat('-Z','Y').to_euler()
            scene.render.filepath=str((folder/f'{clip}-{frame:03d}-{label}.png').resolve())
            bpy.ops.render.render(write_still=True);print('RENDER',clip,frame,label,flush=True)


def audit(out):
    require_master(out)
    path=out/'costume.json'
    if path.exists():raise FileExistsError(path)
    rig=bpy.data.objects['VillagerRig'];scene=bpy.context.scene;items=meshes();surface_indices={}
    result={'meshes':[o.name for o in items],'clips':{},'fps':scene.render.fps/scene.render.fps_base,'hitOverlayBones':['DEF-spine.003','DEF-spine.004','DEF-spine.005','DEF-spine.006']}
    for name in CLIPS:
        action(rig,name);first,last=map(round,rig.animation_data.action.frame_range); samples=[]
        for step in range(first*4,last*4+1):
            frame=step/4
            scene.frame_set(int(frame),subframe=frame%1);bpy.context.view_layer.update();deps=bpy.context.evaluated_depsgraph_get()
            lo=math.inf;hi=-math.inf;lowest=''
            for obj in items:
                ev=obj.evaluated_get(deps);mesh=ev.to_mesh()
                try:
                    if obj.name not in surface_indices:
                        surface_indices[obj.name]=np.array(sorted({i for poly in mesh.polygons for i in poly.vertices}),dtype=np.int32)
                    used=surface_indices[obj.name]
                    coordinates=np.empty(len(mesh.vertices)*3,dtype=np.float32)
                    mesh.vertices.foreach_get('co',coordinates);coordinates=coordinates.reshape(-1,3)[used]
                    transform=np.array(ev.matrix_world,dtype=np.float64)
                    zs=(coordinates[:,0]*transform[2,0]+coordinates[:,1]*transform[2,1]+
                        coordinates[:,2]*transform[2,2]+transform[2,3])
                    if not len(zs) or not np.isfinite(zs).all():raise ValueError('Bad geometry '+obj.name)
                    obj_lo=float(zs.min());obj_hi=float(zs.max())
                    if obj_lo<lo:lo=obj_lo;lowest=obj.name
                    hi=max(hi,obj_hi)
                finally:ev.to_mesh_clear()
            if name=='Idle' and frame==first:result['heightAboveGroundOrigin']=hi
            samples.append({'frame':frame,'minimum':lo,'maximum':hi,'lowest':lowest})
        low=min(s['minimum'] for s in samples);result['clips'][name]={'first':first,'last':last,'minimum':low,'samples':samples}
        print('COSTUME',name,low,flush=True)
    path.write_text(json.dumps(result,indent=2)+'\n')
    if min(v['minimum'] for v in result['clips'].values())<-.003:raise ValueError('Whole costume ground penetration; see costume.json')


def gait(out):
    """Measure source-distance stance travel using this costume's actual shoe."""
    require_master(out)
    path=out/'gait.json'
    if path.exists():raise FileExistsError(path)
    scene=bpy.context.scene;rig=bpy.data.objects['VillagerRig'];shoe=bpy.data.objects['Brown shoe L']
    result={'method':'Actual evaluated left shoe; mean sole Y within 10mm of the lowest face-referenced point, 5-frame samples in initial stance; least-squares slope at authored FPS','clips':{}}
    fps=scene.render.fps/scene.render.fps_base
    for name,finish in [('Walk',60),('Run',30)]:
        action(rig,name);samples=[]
        for offset in range(0,finish+1,5):
            frame=1+offset;scene.frame_set(frame);bpy.context.view_layer.update();ev=shoe.evaluated_get(bpy.context.evaluated_depsgraph_get());mesh=ev.to_mesh()
            try:
                ids={i for poly in mesh.polygons for i in poly.vertices};pts=[ev.matrix_world@mesh.vertices[i].co for i in ids]
                low=min(p.z for p in pts);sole=[p for p in pts if p.z<low+.010]
                if low<.015:samples.append({'frame':frame,'z':low,'y':sum(p.y for p in sole)/len(sole)})
            finally:ev.to_mesh_clear()
        if len(samples)<4:raise ValueError('Too few planted shoe samples')
        times=[(s['frame']-1)/fps for s in samples];values=[s['y'] for s in samples]
        mt=sum(times)/len(times);my=sum(values)/len(values)
        speed=sum((t-mt)*(y-my) for t,y in zip(times,values))/sum((t-mt)**2 for t in times)
        result['clips'][name]={'speedMetersPerSecond':speed,'samples':samples}
    path.write_text(json.dumps(result,indent=2)+'\n');print('GAIT',json.dumps(result['clips']),flush=True)


def contacts(out):
    """Record sampled actual surface intersections; enclosed skin is not a collision proof."""
    require_master(out);path=out/'contacts.json'
    if path.exists():raise FileExistsError(path)
    rig=bpy.data.objects['VillagerRig'];scene=bpy.context.scene;items=meshes()
    body=bpy.data.objects['Human'];targets=[bpy.data.objects['Trader sleeveless vest'],bpy.data.objects['Separate olive trousers']]
    targets.extend(o for o in items if o.name.startswith(('Trader huge pack','Trader sack ', 'Trader bedroll ', 'Trader crossed pole', 'Trader bundle low')))
    def tree(obj,deps):
        ev=obj.evaluated_get(deps);mesh=ev.to_mesh()
        try:
            verts=[ev.matrix_world@v.co for v in mesh.vertices]
            return BVHTree.FromPolygons(verts,[tuple(p.vertices) for p in mesh.polygons],epsilon=0)
        finally:ev.to_mesh_clear()
    report={'method':'BVH overlaps of evaluated, face-referenced surfaces; counts are triangle-pair contacts and can include intended contact. Enclosure and distances are not established by this test.','samples':[]}
    for name in CLIPS:
        action(rig,name);first,last=map(round,rig.animation_data.action.frame_range)
        for i in range(5):
            frame=first+(last-first)*i/4;scene.frame_set(int(frame),subframe=frame%1);bpy.context.view_layer.update();deps=bpy.context.evaluated_depsgraph_get()
            reference=tree(body,deps);counts={o.name:len(reference.overlap(tree(o,deps))) for o in targets}
            hands={}
            if name!='Death':
                posed=rig.evaluated_get(deps)
                for side,label in [('L','left'),('R','right')]:
                    hand=posed.pose.bones['DEF-hand.'+side]
                    palm=posed.matrix_world @ ((hand.head+hand.tail)*.5)
                    nearest=tree(bpy.data.objects['Trader shoulder strap '+label],deps).find_nearest(palm)
                    hands[label]=nearest[3] if nearest else None
            report['samples'].append({'clip':name,'frame':frame,'surfaceTrianglePairs':counts,'palmToStrapMeters':hands})
            print('CONTACT',name,frame,counts,flush=True)
    path.write_text(json.dumps(report,indent=2)+'\n')


def package(out):
    require_master(out)
    export=out/'export'
    if export.exists():raise FileExistsError(export)
    audit_data=json.loads((out/'costume.json').read_text())
    if set(audit_data['clips'])!=set(CLIPS) or min(c['minimum'] for c in audit_data['clips'].values())<-.003:
        raise ValueError('Whole-costume audit did not pass')
    root=Path(__file__).resolve().parents[3]; pipeline=Path(__file__).resolve().parent
    speeds=root/'research/private/generated/human-figure-20261109/mpfb-motion02/measurements-audited.json'
    if hashlib.sha256(speeds.read_bytes()).hexdigest()!=SPEED_SHA:raise ValueError('Unexpected gait speed input')
    names=[o.name for o in meshes()];args=[v for n in names for v in ('--mesh',n)]
    master=out/'master'/'Trader.blend'
    def run(script,params,log):
        cmd=[bpy.app.binary_path,'-b',str(master.resolve()),'--python-exit-code','2','--python',str(pipeline/script),'--',*map(str,params)]
        with (out/log).open('x') as f:subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT,check=True,timeout=3600)
    run('character_textures.py',['--output',export,*args],'materials.log')
    fbx=export/'Trader.fbx'; run('export_character_fbx.py',['--output',fbx,'--animations','--bake-rig','VillagerRig','--join-skins',*args,*[v for n in CLIPS for v in ('--action',n)]],'export.log')
    data=json.loads(fbx.with_suffix('.export.json').read_text());maps=json.loads((export/'materials.json').read_text())
    measured=json.loads((out/'gait.json').read_text())['clips']
    if not all(math.isfinite(measured[n]['speedMetersPerSecond']) and measured[n]['speedMetersPerSecond']>0 for n in ('Walk','Run')):
        raise ValueError('Invalid measured costume gait')
    manifest={'height':audit_data['heightAboveGroundOrigin'],'walkSpeed':measured['Walk']['speedMetersPerSecond'],'runSpeed':measured['Run']['speedMetersPerSecond'],'hitOverlayBones':audit_data['hitOverlayBones'],'minBoneWeight':1e-7,'clips':data['clips'],'materials':[{k:m[k] for k in ('name','albedo','orm')} for m in maps['materials']]}
    (export/'Trader.clips.json').write_text(json.dumps(manifest,indent=2)+'\n')
    receipt={'sourceSha256':SOURCE_SHA,'masterSha256':hashlib.sha256(master.read_bytes()).hexdigest(),
             'fbxSha256':hashlib.sha256(fbx.read_bytes()).hexdigest(),
             'costumeMinimumMeters':min(c['minimum'] for c in audit_data['clips'].values()),
             'sourceToGameMaxMeters':max(v['maxError'] for v in data['surfaceProof']),
             'blenderFbxMaxMeters':max(v['maxError'] for v in data['reimportSurfaceProof']),
             'scope':'Blender package only; Unity, gameplay, Voxel and installation remain unverified'}
    (out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
    print('PACKAGED',json.dumps(receipt),flush=True)


def main():
    p=argparse.ArgumentParser();p.add_argument('--author',type=Path);p.add_argument('--audit',type=Path);p.add_argument('--package',type=Path);p.add_argument('--preview',type=Path);p.add_argument('--gait',type=Path);p.add_argument('--contacts',type=Path)
    a=p.parse_args(sys.argv[sys.argv.index('--')+1:])
    if sum(x is not None for x in (a.author,a.audit,a.package,a.preview,a.gait,a.contacts))!=1:p.error('Choose one action')
    if a.author:author(a.author)
    elif a.audit:audit(a.audit)
    elif a.package:package(a.package)
    elif a.preview:preview(a.preview)
    elif a.gait:gait(a.gait)
    else:contacts(a.contacts)


if __name__=='__main__':main()
