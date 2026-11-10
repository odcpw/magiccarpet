"""Reproducible source-informed Castle collector. Blender 4.5, CPU rendering.

+Y forward, Z up, origin at bottom of basket. TMAP 88..95 are static owner
colors, not flight frames. Idle is a static binding clip; native position and
sphere actors carry the actual movement and collection, not invented animation.
Run: blender -b --threads 8 --python-exit-code 2 --python collector_balloon.py -- --output NEW_DIR
Run again with --package to export an existing master and surface samples.
"""
import argparse
import json
import math
import shutil
import subprocess
import sys
from pathlib import Path

import bpy
from mathutils import Vector


def material(name, color, metallic=0.0, roughness=0.5):
    mat = bpy.data.materials.new(name)
    mat.diffuse_color = (*color, 1)
    mat.use_nodes = True
    node = mat.node_tree.nodes.get('Principled BSDF')
    node.inputs['Base Color'].default_value = (*color, 1)
    node.inputs['Metallic'].default_value = metallic
    node.inputs['Roughness'].default_value = roughness
    return mat


def build(output):
    output.mkdir(parents=True, exist_ok=False)
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.object.delete(use_global=False)
    scene = bpy.context.scene
    scene.render.fps = 24
    mats = {
        'crown': material('Owner crown', (.56, .57, .61), roughness=.78),
        'cloth': material('Owner fabric', (.56, .57, .61), roughness=.78),
        'band': material('Owner band', (.38, .40, .44), roughness=.72),
        'seam': material('Fabric seams', (.20, .20, .22), roughness=.8),
        'gold': material('Aged brass', (.55, .30, .035), metallic=.75, roughness=.32),
        'rope': material('Dark suspension rope', (.14, .11, .075), roughness=.9),
        'wood': material('Gondola wood', (.19, .095, .038), roughness=.75),
        'wood_dark': material('Gondola iron and dark wood', (.065, .040, .023), metallic=.15, roughness=.72),
    }
    arm = bpy.data.armatures.new('Collector skeleton')
    rig = bpy.data.objects.new('CollectorRig', arm)
    scene.collection.objects.link(rig)
    bpy.context.view_layer.objects.active = rig
    rig.select_set(True)
    bpy.ops.object.mode_set(mode='EDIT')
    bone = arm.edit_bones.new('Body')
    bone.head=(0,0,0); bone.tail=(0,0,.4)
    bpy.ops.object.mode_set(mode='OBJECT')
    rig.select_set(False)
    objects=[]

    def bind(obj, name, mat, smooth=True):
        obj.name=name
        bpy.context.view_layer.objects.active=obj
        bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
        obj.data.materials.append(mats[mat])
        for poly in obj.data.polygons: poly.use_smooth=smooth
        group=obj.vertex_groups.new(name='Body')
        group.add(list(range(len(obj.data.vertices))),1.0,'REPLACE')
        obj.parent=rig
        mod=obj.modifiers.new('Armature','ARMATURE'); mod.object=rig
        objects.append(obj)
        return obj

    def rod(name,a,b,r,mat,r2=None,vertices=8):
        d=Vector(b)-Vector(a)
        bpy.ops.mesh.primitive_cone_add(vertices=vertices, radius1=r, radius2=r if r2 is None else r2, depth=d.length, location=(Vector(a)+Vector(b))/2)
        obj=bpy.context.object;obj.rotation_mode='QUATERNION';obj.rotation_quaternion=d.to_track_quat('Z','Y')
        return bind(obj,name,mat)

    def lathe(name, profile, mat, n=48, cap_bottom=True, cap_top=True):
        verts=[(r*math.cos(i*2*math.pi/n),r*math.sin(i*2*math.pi/n),z) for z,r in profile for i in range(n)]
        faces=[(j*n+i,j*n+(i+1)%n,(j+1)*n+(i+1)%n,(j+1)*n+i) for j in range(len(profile)-1) for i in range(n)]
        if cap_bottom: faces.append(tuple(reversed(tuple(range(n)))))
        if cap_top: faces.append(tuple((len(profile)-1)*n+i for i in range(n)))
        mesh=bpy.data.meshes.new(name); mesh.from_pydata(verts,[],faces);mesh.update()
        obj=bpy.data.objects.new(name,mesh);scene.collection.objects.link(obj)
        return bind(obj,name,mat)

    def ring(name,z,r,tube,mat):
        bpy.ops.mesh.primitive_torus_add(major_segments=48,minor_segments=8,location=(0,0,z),major_radius=r,minor_radius=tube)
        return bind(bpy.context.object,name,mat)

    # The source envelope is pavilion-shaped, not a round hot-air balloon:
    # pointed crown, a broad shoulder, band, and a long taper behind the rigging.
    profile=[(.58,.23),(.72,.34),(.96,.51),(1.16,.65),(1.35,.76),(1.43,.79),(1.74,.81),(1.88,.79),(1.94,.75),(2.02,.70),(2.16,.52),(2.30,.30),(2.42,.055)]
    # Source Night palettes keep the crown blue across owners while the
    # lower fabric and band vary; Day tints both. Separate materials preserve
    # that capability without baking lighting into the neutral master.
    lathe('Shaped lower textile envelope',profile[:9],'cloth',cap_top=False)
    lathe('Crown textile envelope',profile[8:],'crown',cap_bottom=False)
    lathe('Broad patterned middle band',[(1.70,.815),(1.74,.825),(1.91,.805),(1.94,.775)],'band')
    for z,r in [(1.70,.825),(1.94,.786)]: ring('Band welt',z,r,.012,'seam')
    # Stitched vertical gores follow the actual envelope profile.
    for i in range(12):
        a=i*2*math.pi/12
        for (z0,r0),(z1,r1) in zip(profile,profile[1:]):
            rod('Envelope gore',((r0+.004)*math.cos(a),(r0+.004)*math.sin(a),z0),((r1+.004)*math.cos(a),(r1+.004)*math.sin(a),z1),.006,'seam',vertices=6)
        # Short angled inset across the band, matching the striped source rim.
        b=a+.17
        rod('Diagonal band stitch',(.827*math.cos(a),.827*math.sin(a),1.72),(.802*math.cos(b),.802*math.sin(b),1.91),.008,'seam',vertices=6)
    ring('Brass suspension hoop',1.40,.77,.035,'gold')
    ring('Hoop lower wire',1.355,.75,.009,'wood_dark')
    # Alternating brass studs on the visible hoop.
    for i in range(24):
        a=i*2*math.pi/24
        bpy.ops.mesh.primitive_uv_sphere_add(segments=8,ring_count=4,radius=.035,location=(.77*math.cos(a),.77*math.sin(a),1.40))
        bind(bpy.context.object,'Hoop stud','gold')
    # Wooden tapered gondola. Slats and rims keep its scale visible at distance.
    lathe('Tapered wooden gondola',[(0,.22),(.05,.28),(.38,.36),(.45,.39)],'wood',n=16)
    ring('Basket upper rim',.44,.385,.035,'wood_dark')
    ring('Basket lower rim',.055,.28,.025,'wood_dark')
    for i in range(12):
        a=i*2*math.pi/12
        rod('Basket stave',(.27*math.cos(a),.27*math.sin(a),.06),(.37*math.cos(a),.37*math.sin(a),.43),.016,'wood_dark',vertices=6)
    # Six radial stay cables, plus shorter stays to the neck of the fabric.
    for i in range(8):
        a=i*2*math.pi/8
        rod('Suspension cable',(.36*math.cos(a),.36*math.sin(a),.43),(.76*math.cos(a),.76*math.sin(a),1.39),.012,'rope',vertices=6)
        rod('Inner shroud',(.30*math.cos(a),.30*math.sin(a),.43),(.34*math.cos(a),.34*math.sin(a),.78),.009,'rope',vertices=6)
    rod('Crown mast',(0,0,2.40),(0,0,2.70),.023,'gold',vertices=12)
    rod('Finial crossbar',(-.06,0,2.65),(.06,0,2.65),.021,'gold',vertices=8)
    bpy.ops.mesh.primitive_uv_sphere_add(segments=12,ring_count=8,radius=.044,location=(0,0,2.72));bind(bpy.context.object,'Finial','gold')
    bpy.ops.object.select_all(action='DESELECT')
    for obj in objects: obj.select_set(True)
    bpy.context.view_layer.objects.active=objects[0];bpy.ops.object.join()
    skin=bpy.context.object;skin.name='CollectorSkin'
    # Source Level 1 render row is 545 wide / 800 high. Widen uniformly in
    # the horizontal plane so the orthographic model bounds match that ratio,
    # while keeping the basket's bottom anchor and the source height intact.
    horizontal_scale=(545.0/800.0)*2.764/1.674
    for vertex in skin.data.vertices:
        vertex.co.x*=horizontal_scale
        vertex.co.y*=horizontal_scale
    bpy.ops.object.select_all(action='DESELECT');rig.select_set(True);bpy.context.view_layer.objects.active=rig
    rig.animation_data_create();act=bpy.data.actions.new('Idle');act.use_fake_user=True;rig.animation_data.action=act
    pb=rig.pose.bones['Body'];pb.rotation_mode='XYZ'
    for frame in (1,25):
        pb.keyframe_insert(data_path='location',frame=frame,group='Body')
        pb.keyframe_insert(data_path='rotation_euler',frame=frame,group='Body')
    scene.frame_set(1)
    bpy.ops.wm.save_as_mainfile(filepath=str(output/'Collector.blend'))
    triangles=sum(len(poly.vertices)-2 for poly in skin.data.polygons)
    (output/'authoring.json').write_text(json.dumps({'axes':'+Y forward, Z up; origin at basket bottom','source':{'class':3,'model':3,'action':9,'renderIndex':169,'baseTmap':88,'logicalHeight':800,'ownerColorSelectors':list(range(88,96))},'clips':['Idle'], 'triangles':triangles,'materials':list(mats),'ownerTintMaterials':['Owner crown','Owner fabric','Owner band'],'reference':'Private Day/Night TMAP 88..95, each static; source movement and mana are authoritative'},indent=2)+'\n')
    scene.render.engine='CYCLES';scene.cycles.device='CPU';scene.cycles.samples=16
    scene.render.resolution_x=600;scene.render.resolution_y=600;scene.render.resolution_percentage=100
    scene.world.color=(.13,.13,.13)
    def lamp(loc,power,size):
        bpy.ops.object.light_add(type='AREA',location=loc);o=bpy.context.object;o.data.energy=power;o.data.shape='DISK';o.data.size=size;o.rotation_euler=(Vector((0,0,1.4))-o.location).to_track_quat('-Z','Y').to_euler()
    lamp((4,3,5),600,5);lamp((-4,-2,3),350,4)
    bpy.ops.object.camera_add();cam=bpy.context.object;scene.camera=cam;cam.data.type='ORTHO';cam.data.ortho_scale=3.35
    for name,pos in [('front',(0,6,1.6)),('three-quarter',(4,5,3)),('low',(3,5,.6))]:
        cam.location=pos;cam.rotation_euler=(Vector((0,0,1.35))-cam.location).to_track_quat('-Z','Y').to_euler();scene.render.filepath=str(output/(name+'.png'));bpy.ops.render.render(write_still=True)
    print('COLLECTOR_MASTER_OK',triangles,flush=True)


def package(output):
    sys.path.insert(0,str(Path(__file__).resolve().parent))
    from character_textures import export_constants
    target=output/'portable';target.mkdir(exist_ok=False)
    fbx=target/'Collector.fbx'; helper=Path(__file__).resolve().with_name('export_character_fbx.py')
    subprocess.run([bpy.app.binary_path,'-b',str(output/'Collector.blend'),'--threads','8','--python-exit-code','2','--python',str(helper),'--','--output',str(fbx),'--animations','--bake-rig','CollectorRig','--mesh','CollectorSkin','--action','Idle','--join-skins'],check=True,timeout=600)
    materials=export_constants(target/'textures-pbr',['CollectorSkin'])
    report=json.loads(fbx.with_suffix('.export.json').read_text())
    manifest={'schema':'mc-collector-clips/v1','fps':report['fps'],'height':2.764,'forward':[0,1,0],'forwardConvention':'Blender source coordinates, Z up','logicalHeight':800,'clips':report['clips'],'materials':materials['materials'],'materialDirectory':'textures-pbr','ownerTintMaterials':['Owner crown','Owner fabric','Owner band'],'sourceToGameSurfaceMaxError':max(p['maxError'] for p in report['surfaceProof']),'fbxReimportSurfaceMaxError':max(p['maxError'] for p in report['reimportSurfaceProof'])}
    (target/'Collector.clips.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print('COLLECTOR_PACKAGE_OK',target,flush=True)


def unity_package(output, target):
    """Normalize paths for the shared character importer without changing samples."""
    source=output/'portable'
    target.mkdir(parents=True,exist_ok=False)
    for name in ('Collector.fbx','Collector.samples.bin'):
        shutil.copyfile(source/name,target/name)
    shutil.copytree(source/'textures-pbr/textures',target/'textures')
    manifest=json.loads((source/'Collector.clips.json').read_text())
    report=json.loads((source/'Collector.export.json').read_text())
    if manifest['clips'] != report['clips']:
        raise ValueError('Clip manifests disagree')
    for mat in manifest['materials']:
        mat['opacity']=1.0
        mat['emissionStrength']=0
        mat['emissionColor']=[0,0,0,1]
    manifest['minBoneWeight']=1e-7
    manifest.pop('materialDirectory',None)
    (target/'Collector.clips.json').write_text(json.dumps(manifest,indent=2)+'\n')
    (target/'Collector.export.json').write_text(json.dumps(report,indent=2)+'\n')
    print('COLLECTOR_UNITY_PACKAGE_OK',target,flush=True)


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,required=True)
    group=parser.add_mutually_exclusive_group();group.add_argument('--package',action='store_true');group.add_argument('--unity-output',type=Path)
    args=parser.parse_args(sys.argv[sys.argv.index('--')+1:]);out=args.output.resolve()
    if args.unity_output:
        unity_package(out,args.unity_output.resolve())
    elif args.package:
        bpy.ops.wm.open_mainfile(filepath=str(out/'Collector.blend'));package(out)
    else: build(out)

if __name__=='__main__': main()
