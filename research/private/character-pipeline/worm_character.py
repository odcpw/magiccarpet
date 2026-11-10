"""Build the source-informed MC2 Worm as separate, independently placed parts.

Run Blender 4.5 in background: blender -b -t 4 --python worm_character.py -- --output DIR
The local +Y axis is forward; local Z is up; origins are native entity centers.
A head armature animates only its two mandibles. Body motion is owned by Rust.
Original artwork stays in the private reference directory, never in exports.
"""
import argparse
import hashlib
import json
import math
import struct
import subprocess
import sys
import zlib
from pathlib import Path

import bpy
from mathutils import Vector
from mathutils.bvhtree import BVHTree
from mathutils.kdtree import KDTree


def mat(name, color, roughness=.7, metallic=0):
    m = bpy.data.materials.new(name)
    m.diffuse_color = (*color, 1)
    m.use_nodes = True
    p = m.node_tree.nodes.get('Principled BSDF')
    p.inputs['Base Color'].default_value = (*color, 1)
    p.inputs['Roughness'].default_value = roughness
    p.inputs['Metallic'].default_value = metallic
    return m


def chitin_maps(material, out):
    # Hand-authored, tileable fine mottling: an unobtrusive chitin texture,
    # independent of the original proprietary sprite pixels.
    width = 256
    images = {}
    for kind in ('albedo', 'roughness', 'normal'):
        image = bpy.data.images.new('Worm chitin ' + kind, width=width, height=width, alpha=True)
        pixels = []
        for y in range(width):
            v = 2 * math.pi * y / width
            for x in range(width):
                u = 2 * math.pi * x / width
                n = (math.sin(7*u+2*math.sin(3*v))*math.sin(9*v+math.sin(4*u))
                     + .4*math.sin(19*u+11*v) + .25*math.sin(37*u-23*v)) / 1.65
                if kind == 'albedo':
                    c = (max(0,.24+.045*n), max(0,.22+.043*n), max(0,.055+.017*n))
                elif kind == 'roughness':
                    c = (max(0,min(1,.68+.08*n)),)*3
                else:
                    c = (.5+.025*math.cos(19*u+11*v), .5+.025*math.cos(37*u-23*v), .998)
                pixels.extend((*c,1))
        image.pixels.foreach_set(pixels)
        image.filepath_raw = str(out / ('chitin-' + kind + '.png'))
        image.file_format = 'PNG'; image.save(); image.pack(); images[kind] = image
    tree = material.node_tree; shader = tree.nodes.get('Principled BSDF')
    for kind, target in [('albedo','Base Color'),('roughness','Roughness')]:
        node = tree.nodes.new('ShaderNodeTexImage'); node.image = images[kind]; node.label = kind
        if kind == 'roughness': node.image.colorspace_settings.name = 'Non-Color'
        tree.links.new(node.outputs['Color'], shader.inputs[target])
    node = tree.nodes.new('ShaderNodeTexImage'); node.image = images['normal']; node.label = 'normal'
    node.image.colorspace_settings.name = 'Non-Color'
    normal = tree.nodes.new('ShaderNodeNormalMap'); normal.inputs['Strength'].default_value = .35
    tree.links.new(node.outputs['Color'], normal.inputs['Color']); tree.links.new(normal.outputs['Normal'], shader.inputs['Normal'])


def segment_atlas(out):
    """One material/submesh per child, including olive, underside and spot."""
    atlas = mat('Shared segment atlas', (.24,.22,.055), .68)
    images = {}
    for kind in ('albedo','roughness','normal'):
        src = bpy.data.images['Worm chitin ' + kind]
        source = list(src.pixels); pixels = []
        if kind == 'albedo': dark, red = (.085,.072,.025), (.62,.012,.008)
        elif kind == 'roughness': dark, red = (.78,)*3, (.24,)*3
        else: dark, red = (.5,.5,1), (.5,.5,1)
        for y in range(256):
            for x in range(512):
                if x < 256:
                    i = (y*256+x)*4; pixels.extend(source[i:i+4])
                else: pixels.extend((*(dark if x < 384 else red),1))
        image = bpy.data.images.new('Segment '+kind, width=512, height=256, alpha=True)
        image.pixels.foreach_set(pixels);image.filepath_raw=str(out/('segment-'+kind+'.png'))
        image.file_format='PNG';image.save();image.pack();images[kind]=image
    tree=atlas.node_tree;shader=tree.nodes.get('Principled BSDF')
    for kind,target in [('albedo','Base Color'),('roughness','Roughness')]:
        node=tree.nodes.new('ShaderNodeTexImage');node.image=images[kind]
        if kind=='roughness':node.image.colorspace_settings.name='Non-Color'
        tree.links.new(node.outputs['Color'],shader.inputs[target])
    node=tree.nodes.new('ShaderNodeTexImage');node.image=images['normal'];node.image.colorspace_settings.name='Non-Color'
    normal=tree.nodes.new('ShaderNodeNormalMap');normal.inputs['Strength'].default_value=.35
    tree.links.new(node.outputs['Color'],normal.inputs['Color']);tree.links.new(normal.outputs['Normal'],shader.inputs['Normal'])
    return atlas


def sphere(name, location, scale, material, segments=24, rings=12):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=segments, ring_count=rings, location=location)
    o = bpy.context.object
    o.name = name
    o.scale = scale
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    o.data.materials.append(material)
    for p in o.data.polygons:
        p.use_smooth = True
    return o


def mandible(name, sign, material):
    # Long, tapered forward-facing arms curve inward, matching TMAP 34–37.
    points = [(sign*.44,.12,.03,.25), (sign*.66,.39,.01,.22),
              (sign*.82,.75,-.02,.17), (sign*.82,1.08,-.05,.12),
              (sign*.65,1.30,-.06,.065), (sign*.46,1.31,-.06,.012)]
    verts=[]; faces=[]; sides=12
    for i,(x,y,z,r) in enumerate(points):
        a=Vector(points[max(0,i-1)][:3]); b=Vector(points[min(len(points)-1,i+1)][:3])
        tangent=(b-a).normalized(); side=Vector((-tangent.y,tangent.x,0)).normalized(); up=tangent.cross(side).normalized()
        for j in range(sides):
            t=2*math.pi*j/sides; v=Vector((x,y,z))+r*(side*math.cos(t)+up*math.sin(t)*.72)
            verts.append(tuple(v))
    for i in range(len(points)-1):
        for j in range(sides):
            a=i*sides+j;b=i*sides+(j+1)%sides;faces.append((a,b,b+sides,a+sides))
    faces.extend([tuple(reversed(range(sides))),tuple((len(points)-1)*sides+j for j in range(sides))])
    mesh=bpy.data.meshes.new(name);mesh.from_pydata(verts,[],faces);mesh.update()
    o=bpy.data.objects.new(name,mesh);bpy.context.collection.objects.link(o);o.data.materials.append(material)
    for p in mesh.polygons:p.use_smooth=True
    # Deterministic usable UVs; no shader-only procedural dependency.
    uv=mesh.uv_layers.new(name='UVMap')
    for p in mesh.polygons:
        for li in p.loop_indices:
            v=mesh.loops[li].vertex_index;uv.data[li].uv=((v%sides)/sides,(v//sides)/(len(points)-1))
    return o


def skin(o, rig, bone):
    vg=o.vertex_groups.new(name=bone);vg.add(list(range(len(o.data.vertices))),1.0,'REPLACE')
    o.parent=rig;mod=o.modifiers.new('Mandible rig','ARMATURE');mod.object=rig


def action(rig, name, keys):
    a=bpy.data.actions.new(name);a.use_fake_user=True;rig.animation_data_create();rig.animation_data.action=a
    for frame,spread in keys:
        for side,sign in [('L',1),('R',-1)]:
            # Bone local Y follows world Z in the bind pose. Swing about that
            # axis so the mandibles close horizontally as in TMAP 34–37.
            bone=rig.pose.bones['Mandible.'+side];bone.rotation_mode='XYZ';bone.rotation_euler=(0,sign*spread,0);bone.keyframe_insert(data_path='rotation_euler',frame=frame,group=bone.name)
    return a


def select_export(path, objects, anim=False):
    bpy.ops.object.select_all(action='DESELECT')
    for o in objects:o.select_set(True)
    bpy.context.view_layer.objects.active=objects[0]
    bpy.ops.export_scene.fbx(filepath=str(path),use_selection=True,object_types={'MESH','ARMATURE'},
        add_leaf_bones=False,axis_forward='-Z',axis_up='Y',bake_anim=anim,
        bake_anim_use_all_actions=False,bake_anim_use_nla_strips=False,bake_anim_simplify_factor=0,
        path_mode='COPY',embed_textures=True)


def render(path, target, camera, ortho=4.6):
    scene=bpy.context.scene
    bpy.ops.object.camera_add(location=camera);cam=bpy.context.object;cam.data.type='ORTHO';cam.data.ortho_scale=ortho
    cam.rotation_euler=(Vector(target)-cam.location).to_track_quat('-Z','Y').to_euler();scene.camera=cam
    scene.render.engine='CYCLES';scene.cycles.samples=24;scene.cycles.device='CPU'
    scene.render.resolution_x=900;scene.render.resolution_y=720;scene.render.resolution_percentage=100
    scene.world.color=(.18,.18,.18);scene.render.image_settings.file_format='PNG';scene.render.filepath=str(path)
    bpy.ops.render.render(write_still=True);bpy.data.objects.remove(cam,do_unlink=True)


HEAD_MESHES = ('Head shell', 'Head underside', 'Eye socket', 'Single red eye',
               'Eye glint', 'Mandible L', 'Mandible R')
HEAD_ACTIONS = ('Idle', 'Attack', 'Hit', 'Death')
PIPELINE = Path(__file__).resolve().parent


def mandible_clearance():
    """Check the two independently animated mandible surfaces at every frame."""
    rig = bpy.data.objects['WormHeadRig']
    meshes = [bpy.data.objects['Mandible L'], bpy.data.objects['Mandible R']]
    results = []
    for name in HEAD_ACTIONS:
        action = bpy.data.actions[name]
        rig.animation_data.action = action; rig.animation_data.action_slot = action.slots[0]
        for frame in range(round(action.frame_range[0]), round(action.frame_range[1])+1):
            bpy.context.scene.frame_set(frame); bpy.context.view_layer.update()
            deps = bpy.context.evaluated_depsgraph_get(); points, trees = [], []
            for obj in meshes:
                evaluated = obj.evaluated_get(deps); mesh = evaluated.to_mesh()
                vertices = [evaluated.matrix_world @ v.co for v in mesh.vertices]
                faces = [list(p.vertices) for p in mesh.polygons]
                trees.append(BVHTree.FromPolygons(vertices, faces)); points.append(vertices)
                evaluated.to_mesh_clear()
            overlaps = len(trees[0].overlap(trees[1]))
            if overlaps:
                raise ValueError(f'Mandibles intersect in {name} frame {frame}')
            tree = KDTree(len(points[1]))
            for i, point in enumerate(points[1]): tree.insert(point, i)
            tree.balance()
            # Vertex distance is an upper bound, not an exact surface clearance.
            nearest = min(tree.find(point)[2] for point in points[0])
            results.append({'clip':name, 'frame':frame, 'surfaceIntersections':overlaps,
                            'nearestVertexDistanceUpperBound':nearest})
    rig.animation_data.action = bpy.data.actions['Idle']; bpy.context.scene.frame_set(1)
    return results


def write_map(path, width, height, values):
    """Write linear, non-color RGBA data without display transforms."""
    def chunk(tag, payload):
        return (struct.pack('>I', len(payload)) + tag + payload +
                struct.pack('>I', zlib.crc32(tag + payload)))
    rows = b''.join(b'\0' + values[y * width * 4:(y + 1) * width * 4]
                    for y in range(height))
    png = (b'\x89PNG\r\n\x1a\n' +
           chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0)) +
           chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b''))
    with path.open('xb') as stream:
        stream.write(png)


def portable_maps(output, prefix, image_prefix):
    """Preserve albedo; encode linear ORM and the authored normal strength."""
    rough = bpy.data.images[image_prefix + 'roughness']
    normal = bpy.data.images[image_prefix + 'normal']
    width, height = rough.size
    if tuple(normal.size) != (width, height):
        raise ValueError('Worm material maps have incompatible dimensions')
    r, n = list(rough.pixels), list(normal.pixels)
    orm, normals = bytearray(), bytearray()
    def byte(value):
        return round(max(0, min(1, value)) * 255)
    for i in range(0, len(r), 4):
        orm.extend((255, byte(r[i]), 0, 255))
        x, y = .35*(2*n[i]-1), .35*(2*n[i+1]-1)
        z = math.sqrt(max(0, 1-x*x-y*y))
        normals.extend((byte((x+1)/2), byte((y+1)/2), byte((z+1)/2), 255))
    write_map(output/'textures'/f'{prefix}_orm.png', width, height, orm)
    write_map(output/'textures'/f'{prefix}_normal.png', width, height, normals)


def augment_head_samples(out, report):
    """Add five independent source/game/FBX samples to the standard 20."""
    import export_character_fbx as exporter
    rig = bpy.data.objects['WormHeadRig']
    source_meshes = [bpy.data.objects[name] for name in HEAD_MESHES]
    game_objects, _ = exporter.bake_deform_rig('WormHeadRig', HEAD_MESHES, HEAD_ACTIONS)
    game_meshes = [obj for obj in game_objects if obj.type == 'MESH']
    extra = [('Idle', 4, 4), ('Idle', 9, 9), ('Attack', 4, 29),
             ('Attack', 8, 33), ('Hit', 3, 49)]
    samples, source_proof = {}, []
    for name, source_frame, game_frame in extra:
        action = bpy.data.actions[name]
        rig.animation_data.action = action; rig.animation_data.action_slot = action.slots[0]
        bpy.context.scene.frame_set(source_frame); bpy.context.view_layer.update()
        expected = [exporter.world_vertices(obj) for obj in source_meshes]
        bpy.context.scene.frame_set(game_frame); bpy.context.view_layer.update()
        actual = [exporter.world_vertices(obj) for obj in game_meshes]
        samples[game_frame] = []
        for source, dest, mesh in zip(expected, actual, source_meshes):
            if len(source) != len(dest):
                raise ValueError('Additional sample lost vertices: '+mesh.name)
            errors = [(a-b).length for a,b in zip(source,dest)]
            if not errors or not all(math.isfinite(e) for e in errors) or max(errors) > .0001:
                raise ValueError('Additional source/game surface mismatch: '+mesh.name)
            source_proof.append({'clip':name, 'sourceFrame':source_frame, 'frame':game_frame,
                                 'mesh':mesh.name, 'vertices':len(dest), 'maxError':max(errors)})
            samples[game_frame].extend(dest)
    proof, disconnected = exporter.verify_reimport(out/'WormHead.fbx', samples)
    if max(row['maxError'] for row in proof) > .0001:
        raise ValueError('Additional FBX sample exceeds 0.1 mm')
    sample_path = out/'WormHead.samples.bin'
    data = sample_path.read_bytes()
    magic, version, count, fps = struct.unpack_from('<4sIId', data)
    if (magic, version, count) != (b'MCSK', 1, 20):
        raise ValueError('Unexpected standard head sample file')
    offset = struct.calcsize('<4sIId'); combined = {}
    for _ in range(count):
        frame, n = struct.unpack_from('<II', data, offset); offset += 8
        combined[frame] = [struct.unpack_from('<fff', data, offset + 12*i) for i in range(n)]
        offset += 12*n
    if offset != len(data) or set(combined).intersection(samples):
        raise ValueError('Malformed or duplicated head samples')
    combined.update(samples)
    sample_path.rename(out/'WormHead.standard-20.samples.bin')
    exporter.write_surface_samples(sample_path, dict(sorted(combined.items())), fps)
    report['surfaceProof'].extend(source_proof)
    report['reimportSurfaceProof'].extend(proof)
    report['additionalSampleFrames'] = sorted(samples)
    report['additionalReimportDisconnectedBones'] = disconnected
    (out/'WormHead.export.json').write_text(json.dumps(report, indent=2)+'\n')
    return report


def package(out):
    """Package an already-open editable master through the shared exporter."""
    if Path(bpy.data.filepath).name != 'worm-master.blend':
        raise ValueError('Open the original worm-master.blend before --package')
    source_master_hash = hashlib.sha256(Path(bpy.data.filepath).read_bytes()).hexdigest()
    sys.path.insert(0, str(PIPELINE))
    from character_textures import export_constants, extract_packed
    records = export_constants(out, ['Head underside', 'Eye socket', 'Single red eye', 'Eye glint'])
    extract_packed(out, {
        'Worm chitin albedo': 'head_albedo.png',
        'Segment albedo': 'segment_albedo.png',
    })
    portable_maps(out, 'head', 'Worm chitin ')
    portable_maps(out, 'segment', 'Segment ')
    clearance = mandible_clearance()
    (out/'mandible-clearance.json').write_text(json.dumps({
        'scope': 'Mandible-to-mandible surface intersections; intentional shell hinge overlap excluded',
        'frames': clearance,
    }, indent=2)+'\n')

    segment = bpy.data.objects['WormSegment']
    segment.hide_viewport = False; segment.hide_render = False
    bpy.ops.object.armature_add(location=(0, 0, 0))
    rig = bpy.context.object; rig.name = 'WormSegmentRig'
    bpy.ops.object.mode_set(mode='EDIT')
    bone = rig.data.edit_bones[0]; bone.name = 'SegmentRoot'
    bone.head = (0, 0, 0); bone.tail = (0, 0, .3)
    bpy.ops.object.mode_set(mode='OBJECT')
    skin(segment, rig, 'SegmentRoot')
    rig.animation_data_create(); static = bpy.data.actions.new('SegmentStatic')
    static.use_fake_user = True; rig.animation_data.action = static
    pose = rig.pose.bones['SegmentRoot']; pose.rotation_mode = 'XYZ'
    for frame in (1, 2):
        pose.rotation_euler = (0, 0, 0); pose.keyframe_insert(data_path='rotation_euler', frame=frame)
    bpy.context.scene.frame_set(1)
    bpy.context.view_layer.update()
    def bounds(objects):
        coords = [obj.matrix_world @ vertex.co for obj in objects for vertex in obj.data.vertices]
        return {'min': [min(p[i] for p in coords) for i in range(3)],
                'max': [max(p[i] for p in coords) for i in range(3)]}
    head_bounds = bounds([bpy.data.objects[name] for name in HEAD_MESHES])
    segment_bounds = bounds([segment])
    master = out/'Worm-portable-master.blend'
    bpy.ops.wm.save_as_mainfile(filepath=str(master), compress=True)

    def run_export(name, rig_name, meshes, actions):
        fbx = out/(name+'.fbx')
        cmd = [bpy.app.binary_path, '-b', str(master), '--python-exit-code', '2',
               '--python', str(PIPELINE/'export_character_fbx.py'), '--', '--output', str(fbx),
               '--animations', '--bake-rig', rig_name, '--join-skins']
        cmd.extend(item for mesh in meshes for item in ('--mesh', mesh))
        cmd.extend(item for clip in actions for item in ('--action', clip))
        with (out/(name+'.export.log')).open('x') as log:
            subprocess.run(cmd, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=600)
        return json.loads(fbx.with_suffix('.export.json').read_text())

    head = run_export('WormHead', 'WormHeadRig', HEAD_MESHES, HEAD_ACTIONS)
    body = run_export('WormSegment', 'WormSegmentRig', ['WormSegment'], ['SegmentStatic'])
    if body['deformBones'] != 1:
        raise ValueError('The reusable segment must have one rigid deform bone')
    head = augment_head_samples(out, head)
    common = {'fps': head['fps'], 'forward': [0, 1, 0], 'coordinateSystem': 'Blender Z-up',
              'walkSpeed': 0, 'runSpeed': 0, 'minBoneWeight': 1e-7}
    head_maps = [{k: m[k] for k in ('name', 'albedo', 'orm')} for m in records['materials']]
    head_maps.append({'name': 'Olive chitin', 'albedo': 'textures/head_albedo.png',
                      'orm': 'textures/head_orm.png', 'normal': 'textures/head_normal.png'})
    segment_maps = [{'name': 'Shared segment atlas', 'albedo': 'textures/segment_albedo.png',
                     'orm': 'textures/segment_orm.png', 'normal': 'textures/segment_normal.png'}]
    for name, report, materials, extent in [('WormHead', head, head_maps, head_bounds),
                                             ('WormSegment', body, segment_maps, segment_bounds)]:
        manifest = {**common, 'height': extent['max'][2]-extent['min'][2],
                    'artBounds': extent, 'clips': report['clips'], 'materials': materials}
        (out/(name+'.clips.json')).write_text(json.dumps(manifest, indent=2)+'\n')
    (out/'package.json').write_text(json.dumps({
        'sourceMasterSha256': source_master_hash,
        'portableMasterSha256': hashlib.sha256(master.read_bytes()).hexdigest(),
        'generatorSha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'artifactSha256': {name: hashlib.sha256((out/name).read_bytes()).hexdigest() for name in (
            'WormHead.fbx', 'WormHead.samples.bin', 'WormHead.clips.json',
            'WormSegment.fbx', 'WormSegment.samples.bin', 'WormSegment.clips.json')},
        'axis': '+Y forward, +Z up; pivot is native entity center',
        'height': 'authored mesh extent only; native visual calibration remains pending',
        'sourceReferences': '../reference/sources.json',
        'headSurfaceSamples': 25, 'segmentDeformBones': body['deformBones'],
        'scope': 'Blender package; Unity, gameplay, Voxel and installation unverified'
    }, indent=2)+'\n')
    print('PORTABLE_PACKAGED', out, flush=True)


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--package', action='store_true')
    args=parser.parse_args(sys.argv[sys.argv.index('--')+1:]);out=args.output.resolve()
    if args.package:
        package(out)
        return
    out.mkdir(parents=True,exist_ok=True)
    bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
    olive=mat('Olive chitin',(.24,.22,.055),.63); dark=mat('Lower chitin',(.085,.072,.025),.78)
    chitin_maps(olive,out)
    red=mat('Ruby eye',(.62,.012,.008),.24,.08);glint=mat('Eye glint',(.95,.62,.43),.18)
    bpy.ops.object.armature_add(location=(0,0,0));rig=bpy.context.object;rig.name='WormHeadRig'
    bpy.ops.object.mode_set(mode='EDIT');root=rig.data.edit_bones[0];root.name='Root';root.head=(0,0,0);root.tail=(0,0,.3)
    for side,sign in [('L',1),('R',-1)]:
        b=rig.data.edit_bones.new('Mandible.'+side);b.head=(sign*.44,.12,.03);b.tail=(sign*.44,.12,.35);b.parent=root
    bpy.ops.object.mode_set(mode='OBJECT')
    head=[sphere('Head shell',(0,-.1,.05),(.77,.65,.37),olive),
          sphere('Head underside',(0,-.07,-.12),(.74,.61,.22),dark),
          sphere('Eye socket',(0,.03,.37),(.25,.23,.11),dark),
          sphere('Single red eye',(0,.04,.43),(.205,.19,.115),red),
          sphere('Eye glint',(-.055,.13,.515),(.045,.035,.014),glint,16,8)]
    for o in head:skin(o,rig,'Root')
    for side,sign in [('L',1),('R',-1)]:
        o=mandible('Mandible '+side,sign,olive);skin(o,rig,'Mandible.'+side);head.append(o)
    action(rig,'Idle',[(1,0),(13,.025),(25,0)])
    action(rig,'Attack',[(1,0),(5,.24),(10,.30),(15,.0),(21,0)])
    action(rig,'Hit',[(1,0),(4,.18),(9,-.04),(15,0)])
    action(rig,'Death',[(1,0),(7,.28),(15,.18),(25,.18)])
    rig.animation_data.action=bpy.data.actions['Idle'];bpy.context.scene.frame_set(1)
    # One reusable overlapping rounded child; all child scaling, placement and
    # yaw/pitch belong to the native child. No animated body chain is exported.
    seg=[sphere('Body segment',(0,0,0),(.53,.68,.40),olive,20,12),
         sphere('Body underside',(0,0,-.17),(.50,.63,.23),dark,20,12),
         sphere('Body red dorsal spot',(0,0,.385),(.19,.22,.065),red,16,8)]
    # Collapse body components into a single mesh / one renderer and retain three
    # simple PBR materials. Master retains all editable head components.
    bpy.ops.object.select_all(action='DESELECT')
    for o in seg:o.select_set(True)
    bpy.context.view_layer.objects.active=seg[0];bpy.ops.object.join();segment=bpy.context.object;segment.name='WormSegment'
    bpy.ops.object.origin_set(type='ORIGIN_CURSOR');segment.location=(0,0,0)
    # Pack the skin in the left atlas half and constant dark/red swatches on
    # the right. Every segment uses a single material, mesh and draw surface.
    atlas = segment_atlas(out); uv = segment.data.uv_layers.active.data
    for face in segment.data.polygons:
        for li in face.loop_indices:
            if face.material_index == 0: uv[li].uv.x *= .5
            elif face.material_index == 1: uv[li].uv = (.625,.5)
            elif face.material_index == 2: uv[li].uv = (.875,.5)
            else: raise ValueError('Unexpected segment material')
        face.material_index = 0
    segment.data.materials.clear();segment.data.materials.append(atlas)
    counts={o.name:{'vertices':len(o.data.vertices),'triangles':sum(len(p.vertices)-2 for p in o.data.polygons)} for o in head+[segment]}
    for o in head+[segment]:
        if not o.data.uv_layers:raise ValueError('missing UVs '+o.name)
        if any(not all(math.isfinite(c) for c in v.co) for v in o.data.vertices):raise ValueError('nonfinite mesh '+o.name)
        if o in head and any(abs(sum(g.weight for g in v.groups)-1)>1e-6 for v in o.data.vertices):raise ValueError('invalid weights '+o.name)
    segment.hide_render=True;segment.hide_viewport=True
    bpy.ops.wm.save_as_mainfile(filepath=str(out/'worm-master.blend'))
    # FBX's all-actions exporter does not reliably retarget Blender 4.5 action
    # slots. Bake one explicit timeline for the game and expose clip ranges.
    source_keys = {
        'Idle': [(1,0),(13,.025),(25,0)],
        'Attack': [(1,0),(5,.24),(10,.30),(15,0),(21,0)],
        'Hit': [(1,0),(4,.18),(9,-.04),(15,0)],
        'Death': [(1,0),(7,.28),(15,.18),(25,.18)],
    }
    clips = {}; timeline_keys = []; cursor = 1
    for name, keys in source_keys.items():
        end = keys[-1][0]; clips[name] = [cursor, cursor + end - 1]
        rig.animation_data.action = bpy.data.actions[name]
        for frame in range(1, end + 1):
            bpy.context.scene.frame_set(frame)
            timeline_keys.append((cursor + frame - 1, rig.pose.bones['Mandible.L'].rotation_euler.y))
        cursor += end
    action(rig,'GameTimeline',timeline_keys)
    bpy.context.scene.frame_start = 1
    bpy.context.scene.frame_end = cursor - 1
    select_export(out/'worm-head.fbx',[rig]+head,True)

    segment.hide_viewport=False;segment.hide_render=False
    select_export(out/'worm-segment.fbx',[segment],False)
    # Head isolated views, with CPU-only area lighting.
    segment.hide_render=True
    bpy.ops.object.light_add(type='AREA',location=(3,2,5));light=bpy.context.object;light.data.energy=650;light.data.shape='DISK';light.data.size=5
    render(out/'head-front.png',(0,.35,.0),(0,5,3.2));render(out/'head-side.png',(0,.35,0),(5,1,2));
    rig.animation_data.action=bpy.data.actions['Attack'];bpy.context.scene.frame_set(10);render(out/'head-attack.png',(0,.35,0),(3,4,3))
    rig.animation_data.action=bpy.data.actions['Hit'];bpy.context.scene.frame_set(4);render(out/'head-hit.png',(0,.35,0),(3,4,3))
    rig.animation_data.action=bpy.data.actions['Death'];bpy.context.scene.frame_set(20);render(out/'head-death.png',(0,.35,0),(3,4,3))
    # Source particle 89..104 nominal axis/spacing after loader derives the
    # missing width from the 32x26 TMAP66 and applies source integer arithmetic.
    axes = [119,131,147,159,167,167,159,151,135,127,119,104,87,71,55,47]
    spacing = [148]+axes[1:]
    # A bent source-spaced chain preview only; game children remain independent.
    rig.animation_data.action=bpy.data.actions['Idle'];bpy.context.scene.frame_set(1)
    segment.hide_render=False
    previous=Vector((0,0,0)); copies=[]
    for i,(axis,step) in enumerate(zip(axes,spacing)):
        heading=math.radians(min(i*7,70))
        previous += Vector((-math.sin(heading)*step/119, -math.cos(heading)*step/119, 0))
        obj=segment.copy();obj.data=segment.data;bpy.context.collection.objects.link(obj)
        obj.name=f'Preview child {i:02d}';obj.location=previous;obj.scale=(axis/119,)*3;obj.rotation_euler.z=-heading
        obj.hide_render=False;obj.hide_viewport=False;copies.append(obj)
    segment.hide_render=True
    render(out/'assembled-bend.png',(-6,-4.8,0),(-6,-4.8,20),ortho=22)
    for obj in copies:bpy.data.objects.remove(obj,do_unlink=True)
    report={'axis':'+Y forward, +Z up','pivot':'entity center','source':'TMAP30–37 head and 66 body, Day and Night; original remains owner-local',
       'sourceReferenceManifest':'../reference/sources.json','native':'action232 positions and orients every child from predecessor; do not animate the chain',
       'actions':clips,'fbxImporterFrameOffset':1,'nominalSourceBodyAxis':axes,'nominalSourceBodySpacing':spacing,'meshes':counts,
       'exports':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in [out/'worm-head.fbx',out/'worm-segment.fbx']}}
    (out/'manifest.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report))

if __name__=='__main__':main()
