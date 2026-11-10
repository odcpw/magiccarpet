"""Build a source-informed Level 1 Firefly master and review images.

Run Blender 4.5 headless: blender -b --python firefly_character.py -- --output NEW_DIR
+Y is forward, Z up. The armature uses ordinary one-bone LBS per component.
Reference: private original Day/Night TMAP 279..286, six wing frames each.
"""
import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path

import bpy
from mathutils import Vector


def material(name, color, metallic=0.0, roughness=0.5, emission=0.0, alpha=1.0):
    m = bpy.data.materials.new(name)
    m.diffuse_color = (*color, alpha)
    m.use_nodes = True
    node = m.node_tree.nodes.get('Principled BSDF')
    node.inputs['Base Color'].default_value = (*color, 1)
    node.inputs['Metallic'].default_value = metallic
    node.inputs['Roughness'].default_value = roughness
    node.inputs['Alpha'].default_value = alpha
    if alpha < 1:
        m.surface_render_method = 'DITHERED'
    if emission:
        node.inputs['Emission Color'].default_value = (*color, 1)
        node.inputs['Emission Strength'].default_value = emission
    return m


def build(output):
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.object.delete(use_global=False)
    scene = bpy.context.scene
    scene.render.fps = 24
    mats = {
        'carapace': material('Dark olive chitin', (.055, .062, .022), .22, .42),
        'shell': material('Olive bronze wing covers', (.105, .095, .035), .30, .39),
        'under': material('Warm dark abdomen', (.095, .054, .022), .1, .52),
        'rim': material('Wing ribs and joints', (.13, .09, .035), .18, .45),
        'wing': material('Warm translucent flight membrane', (.30, .20, .095), .03, .48, alpha=.72),
        'eye': material('Blue violet compound eyes', (.115, .075, .42), .4, .2, .12),
        'glow': material('Amber luminous segments', (.92, .66, .09), .0, .38, .65),
    }
    arm = bpy.data.armatures.new('Firefly skeleton')
    rig = bpy.data.objects.new('FireflyRig', arm)
    scene.collection.objects.link(rig)
    bpy.context.view_layer.objects.active = rig
    rig.select_set(True)
    bpy.ops.object.mode_set(mode='EDIT')
    for name, head, tail, parent in [
        ('Body',(0,0,0),(0,0,.3),None),
        ('Wing.L',(.27,.12,.2),(.67,.12,.2),'Body'),
        ('Wing.R',(-.27,.12,.2),(-.67,.12,.2),'Body'),
    ]:
        bone = arm.edit_bones.new(name)
        bone.head, bone.tail = head, tail
        if parent:
            bone.parent = arm.edit_bones[parent]
    bpy.ops.object.mode_set(mode='OBJECT')
    rig.select_set(False)
    objects = []

    def bind(obj, name, mat, bone='Body'):
        obj.name = name
        bpy.context.view_layer.objects.active = obj
        bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
        obj.data.materials.append(mats[mat])
        for polygon in obj.data.polygons:
            polygon.use_smooth = True
        group = obj.vertex_groups.new(name=bone)
        group.add(list(range(len(obj.data.vertices))), 1.0, 'REPLACE')
        obj.parent = rig
        mod = obj.modifiers.new('Armature', 'ARMATURE')
        mod.object = rig
        objects.append(obj)
        return obj

    def ellipsoid(name, location, scale, mat, bone='Body', segments=24, rings=12):
        bpy.ops.mesh.primitive_uv_sphere_add(segments=segments, ring_count=rings, location=location)
        obj = bpy.context.object
        obj.scale = scale
        return bind(obj, name, mat, bone)

    def rod(name, a, b, radius, mat, bone='Body', radius2=None):
        delta = Vector(b) - Vector(a)
        center = (Vector(a) + Vector(b)) / 2
        bpy.ops.mesh.primitive_cone_add(vertices=8, radius1=radius, radius2=radius if radius2 is None else radius2, depth=delta.length, location=center)
        obj = bpy.context.object
        obj.rotation_mode = 'QUATERNION'
        obj.rotation_quaternion = delta.to_track_quat('Z', 'Y')
        return bind(obj, name, mat, bone)

    # Body is long along Y, the original's bright underside is split into plates.
    ellipsoid('Abdomen', (0,-.37,-.03), (.45,.77,.29), 'under', segments=32, rings=16)
    ellipsoid('Thorax', (0,.32,.08), (.38,.35,.27), 'carapace', segments=32, rings=16)
    ellipsoid('Head', (0,.65,.055), (.27,.24,.20), 'carapace')
    for side in (-1,1):
        # Sculpted tapered elytra, each half narrowing to the rear.
        ellipsoid('Elytron', (side*.215,-.34,.16), (.235,.70,.17), 'shell', segments=32, rings=16)
        ellipsoid('Compound eye', (side*.215,.72,.10), (.115,.115,.12), 'eye')
        rod('Antenna base', (side*.13,.78,.13), (side*.23,1.03,.24), .027, 'carapace', radius2=.018)
        rod('Antenna tip', (side*.23,1.03,.24), (side*.36,1.14,.20), .018, 'rim', radius2=.006)
        for j, y in enumerate((.28,.04,-.19)):
            shoulder=(side*.30,y,-.075); knee=(side*(.51+j*.035),y-.10,-.26); foot=(side*(.62+j*.045),y+.03,-.56)
            rod('Leg upper',shoulder,knee,.04,'carapace',radius2=.025)
            ellipsoid('Leg joint', knee, (.047,.047,.047), 'rim', segments=12, rings=8)
            rod('Leg lower',knee,foot,.026,'carapace',radius2=.008)
            rod('Foot',foot,(side*(.67+j*.045),y+.15,-.58),.012,'carapace',radius2=.004)
    for y in (-.56,-.35,-.14):
        ellipsoid('Lantern plate', (0,y,-.269), (.31,.13,.065), 'glow', segments=24,rings=10)
    # Outspread membranous wings, separate hinged bones, carefully modeled veins.
    for side, suffix in ((1,'L'),(-1,'R')):
        bone='Wing.'+suffix
        # Thin ellipsoid with tapered outline and opaque material for portable FBX.
        ellipsoid('Flight wing '+suffix,(side*.83,.02,.23),(.64,.31,.022),'wing',bone,segments=32,rings=12)
        root=(side*.29,.13,.25)
        for tip in ((side*1.40,.03,.25),(side*1.26,.24,.25),(side*1.19,-.22,.25)):
            rod('Wing vein',root,tip,.012,'rim',bone,radius2=.006)
        for x in (.70,1.05):
            rod('Wing cross vein',(side*x,-.17,.25),(side*(x+.10),.18,.25),.006,'rim',bone,radius2=.004)
    # Join disconnected skins into one renderer with exact rigid weights.
    bpy.ops.object.select_all(action='DESELECT')
    for obj in objects: obj.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    bpy.ops.object.join()
    mesh = bpy.context.object
    mesh.name='FireflySkin'
    bpy.ops.object.select_all(action='DESELECT')
    rig.select_set(True)
    bpy.context.view_layer.objects.active=rig
    rig.animation_data_create()
    def action(name, frames):
        act=bpy.data.actions.new(name)
        act.use_fake_user=True
        rig.animation_data.action=act
        for frame, body_z, body_pitch, left, right in frames:
            body=rig.pose.bones['Body']; body.rotation_mode='XYZ';body.location=(0,0,body_z);body.rotation_euler=(body_pitch,0,0)
            for wing, angle in [('Wing.L',left),('Wing.R',right)]:
                pb=rig.pose.bones[wing];pb.rotation_mode='XYZ';pb.rotation_euler=(angle if wing == 'Wing.L' else -angle,0,0)
            for pb in rig.pose.bones:
                pb.keyframe_insert(data_path='location',frame=frame,group=pb.name)
                pb.keyframe_insert(data_path='rotation_euler',frame=frame,group=pb.name)
        for layer in act.layers:
            for strip in layer.strips:
                for bag in strip.channelbags:
                    for curve in bag.fcurves:
                        for point in curve.keyframe_points: point.interpolation='LINEAR'
    # Wing bones point in opposite directions; their local X axes hinge both wings upward.
    action('Hover',[(1,0,0,0,0),(3,.018,0,.95,-.95),(5,0,0,0,0),(7,-.018,0,.95,-.95),(9,0,0,0,0)])
    action('Flight',[(1,0,.09,0,0),(3,.012,.09,1.05,-1.05),(5,0,.09,0,0),(7,-.012,.09,1.05,-1.05),(9,0,.09,0,0)])
    action('Attack',[(1,0,.02,0,0),(4,-.02,-.10,1,-1),(7,.05,.18,0,0),(10,0,.02,1,-1),(13,0,.02,0,0)])
    action('Hit',[(1,0,0,0,0),(3,.04,-.28,.9,-.9),(7,-.03,.12,.3,-.3),(12,0,0,0,0)])
    action('Death',[(1,0,0,0,0),(6,-.12,-.28,.7,-.7),(14,-.38,-.65,1.15,-1.15),(24,-.48,-1.2,.9,-.9)])
    rig.animation_data.action=bpy.data.actions['Hover']
    scene.frame_set(1)
    output.mkdir(parents=True,exist_ok=False)
    bpy.ops.wm.save_as_mainfile(filepath=str(output/'Firefly.blend'))
    triangles=sum(len(p.vertices)-2 for p in mesh.data.polygons)
    (output/'authoring.json').write_text(json.dumps({'axes':'+Y forward, Z up','source':{'class':5,'model':19,'renderIndex':287,'baseTmap':279,'renderMode':17,'logicalHeight':200,'actions':{'roam':153,'attack':154,'linked':155,'preKill':156,'kill':157,'stage':159}},'clips':['Hover','Flight','Attack','Hit','Death'],'triangles':triangles,'materials':list(mats),'scale':'unit authoring model; consumer must match native logical height 200/64','reference':'Original Night TMAPS1-0 and Day TMAPS0-0 selectors 279..286, six frames each'},indent=2)+'\n')
    # CPU-only studio; keeps visual review helpers out of the saved master.
    scene.render.engine='CYCLES'
    scene.cycles.device='CPU';scene.cycles.samples=24
    scene.render.resolution_x=800;scene.render.resolution_y=800;scene.render.resolution_percentage=100
    scene.world.color=(.12,.12,.12)
    def lamp(location, power, size):
        bpy.ops.object.light_add(type='AREA',location=location)
        obj=bpy.context.object;obj.data.energy=power;obj.data.shape='DISK';obj.data.size=size;obj.rotation_euler=(Vector((0,0,0))-obj.location).to_track_quat('-Z','Y').to_euler()
    lamp((4,2,5),700,5);lamp((-4,-2,3),400,4)
    bpy.ops.object.camera_add()
    camera=bpy.context.object;scene.camera=camera;camera.data.type='ORTHO';camera.data.ortho_scale=3.7
    for label,act,frame,position in [('front','Hover',1,(0,5,2)),('side','Hover',1,(5,0,2)),('flight','Flight',3,(3,4,3)),('flight-front','Flight',3,(0,5,2)),('attack','Attack',7,(3,4,2)),('death','Death',24,(3,4,2))]:
        rig.animation_data.action=bpy.data.actions[act];scene.frame_set(frame);camera.location=position;camera.rotation_euler=(Vector((0,0,-.05))-camera.location).to_track_quat('-Z','Y').to_euler();scene.render.filepath=str(output/(label+'.png'));bpy.ops.render.render(write_still=True)
    print('FIREFLY_MASTER_OK',triangles,output,flush=True)


def package(output):
    """Export an existing master, PBR constants, clip manifest and sampled proof."""
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from character_textures import export_constants
    master=output/'Firefly.blend'
    if not master.is_file():
        raise FileNotFoundError(master)
    # Export helper independently reopens the saved authoring master.
    target=output/'portable'
    target.mkdir(exist_ok=False)
    fbx=target/'Firefly.fbx'
    helper=Path(__file__).resolve().with_name('export_character_fbx.py')
    cmd=[bpy.app.binary_path,'-b',str(master),'--threads','8','--python-exit-code','2',
         '--python',str(helper),'--','--output',str(fbx),'--animations','--bake-rig',
         'FireflyRig','--mesh','FireflySkin']
    for name in ('Hover','Flight','Attack','Hit','Death'):
        cmd.extend(('--action',name))
    cmd.append('--join-skins')
    subprocess.run(cmd,check=True,timeout=600)
    # The shared material exporter deliberately rejects emission. Preserve the
    # intended emission separately, while exporting the same opaque PBR base.
    emissive={}
    opacity={}
    for mat in bpy.data.objects['FireflySkin'].data.materials:
        node=mat.node_tree.nodes.get('Principled BSDF')
        alpha=node.inputs['Alpha'].default_value
        if alpha < 1:
            opacity[mat.name]=alpha
            node.inputs['Alpha'].default_value=1
            mat.diffuse_color[3]=1
        strength=node.inputs['Emission Strength'].default_value
        if strength:
            emissive[mat.name]={'strength':strength,'linearColor':list(node.inputs['Emission Color'].default_value)}
            node.inputs['Emission Strength'].default_value=0
    material_report=export_constants(target/'pbr',['FireflySkin'])
    report=json.loads(fbx.with_suffix('.export.json').read_text())
    clips=report['clips']
    for clip in clips: clip['loop']=clip['name'] in {'Hover','Flight'}
    # The generic exporter knows only human locomotion loop names. Its final
    # report is authoritative too, so correct its Firefly loop metadata.
    fbx.with_suffix('.export.json').write_text(json.dumps(report,indent=2)+'\n')
    skin=bpy.data.objects['FireflySkin']
    bpy.context.scene.frame_set(1)
    bpy.context.view_layer.update()
    mesh=skin.evaluated_get(bpy.context.evaluated_depsgraph_get()).to_mesh()
    try:
        heights=[v.co.z for v in mesh.vertices]
        height=max(heights)-min(heights)
    finally: skin.evaluated_get(bpy.context.evaluated_depsgraph_get()).to_mesh_clear()
    manifest={'schema':'mc-firefly-clips/v1','fps':report['fps'],'height':height,
              'forward':[0,1,0], 'forwardConvention':'Blender source coordinates, Z up',
              'logicalHeight':200,'clips':clips,'materials':material_report['materials'],
              'materialDirectory':'pbr','emission':emissive,'opacity':opacity,
              'sourceToGameSurfaceMaxError':max(p['maxError'] for p in report['surfaceProof']),
              'fbxReimportSurfaceMaxError':max(p['maxError'] for p in report['reimportSurfaceProof'])}
    (target/'Firefly.clips.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print('FIREFLY_PACKAGE_OK',target,flush=True)


def normalize_for_unity(source, target):
    """Prepare the existing importer's relative paths and required Idle name."""
    target.mkdir(parents=True, exist_ok=False)
    for name in ('Firefly.fbx', 'Firefly.samples.bin'):
        shutil.copyfile(source / name, target / name)
    shutil.copytree(source / 'pbr/textures', target / 'textures')
    manifest = json.loads((source / 'Firefly.clips.json').read_text())
    report = json.loads((source / 'Firefly.export.json').read_text())
    for data in (manifest, report):
        for clip in data['clips']:
            if clip['name'] == 'Hover':
                clip['name'] = 'Idle'
    for proof in report['surfaceProof']:
        if proof['clip'] == 'Hover':
            proof['clip'] = 'Idle'
    if manifest['clips'] != report['clips']:
        raise ValueError('Clip manifests disagree')
    for material in manifest['materials']:
        name = material['name']
        material['opacity'] = manifest['opacity'].get(name, 1.0)
        emission = manifest['emission'].get(name, {})
        material['emissionStrength'] = emission.get('strength', 0)
        material['emissionColor'] = emission.get('linearColor', [0, 0, 0, 1])
    # The source authoring forward is +Y. Unlike legacy humans, Firefly
    # must not inherit their baked 180-degree importer fallback.
    manifest['forward'] = [0, 1, 0]
    manifest['forwardConvention'] = 'Blender source coordinates, Z up'
    manifest['minBoneWeight'] = 1e-7
    manifest.pop('materialDirectory', None)
    (target / 'Firefly.clips.json').write_text(json.dumps(manifest, indent=2) + '\n')
    (target / 'Firefly.export.json').write_text(json.dumps(report, indent=2) + '\n')
    (target / 'README.md').write_text(
        'Import-normalized copy: Hover is Idle because ModernCharacterInstance initializes Idle. '
        'Source frames, geometry and samples are identical. Paths are relative to this folder. '
        'Material entries expose opacity, emissionStrength and emissionColor; the existing '
        'importer ignores those fields until extended.\n')
    print('FIREFLY_UNITY_PACKAGE_OK', target, flush=True)


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,required=True)
    group=parser.add_mutually_exclusive_group()
    group.add_argument('--package',action='store_true')
    group.add_argument('--unity-output',type=Path,help='Create a fresh Unity importer package at this path')
    args=parser.parse_args(sys.argv[sys.argv.index('--')+1:])
    output=args.output.resolve()
    if args.unity_output:
        normalize_for_unity(output/'portable',args.unity_output.resolve())
    elif args.package:
        bpy.ops.wm.open_mainfile(filepath=str(output/'Firefly.blend'))
        package(output)
    else:
        build(output)

if __name__ == '__main__': main()
