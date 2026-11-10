"""Create the source-inspired Level 1 villager 239 from the accepted rigged master.

Keeps the editable human and separately rigged gown; changes its pigments and
adds a small cap, hair, neckcloth and plum sleeve bands.  The bands are material
assignments on the existing skinned gown, so they inherit its animation exactly.
Use --package with --output FRESH_DIRECTORY to author, measure and export the
five-action package. Without --package, --output names the editable blend file.
"""
import argparse
import hashlib
import json
import math
import subprocess
import sys
from pathlib import Path

import bpy
from mathutils import Vector

MASTER_SHA = 'c36f1f0f8fcec31d9e0364faa5f8eacd4d45657d95aaf4796caf8dc52ab765c0'
SPEEDS_SHA = '554a18fe551c44adfa1da18363958a19d90ee7331f33a882f204540c14346fd5'
CLIPS = ('Idle', 'Walk', 'Run', 'Hit', 'Death')
HIDDEN = ('Turban crown', 'Turban wrap 1', 'Turban wrap 2', 'Turban wrap 3',
          'Right turban drape', 'Left turban drape', 'Nape wrap')
MESHES = ('Human', 'Human.donitz_monk_robe', 'Walk skeletal lower cloth',
          'Separate olive trousers', 'Eye L', 'Iris L', 'Eye R', 'Iris R',
          'Front neck wrap', 'Brown shoe L', 'Brown shoe R', 'Brown cap',
          'Brown cap brim', 'Dark hair')


def material(name, color, roughness):
    mat = bpy.data.materials.new(name)
    mat.diffuse_color = (*color, 1)
    mat.use_nodes = True
    shader = mat.node_tree.nodes.get('Principled BSDF')
    shader.inputs['Base Color'].default_value = (*color, 1)
    shader.inputs['Roughness'].default_value = roughness
    shader.inputs['Metallic'].default_value = 0
    return mat


def set_color(mat, color, roughness):
    mat.diffuse_color = (*color, 1)
    s = mat.node_tree.nodes.get('Principled BSDF')
    s.inputs['Base Color'].default_value = (*color, 1)
    s.inputs['Roughness'].default_value = roughness
    s.inputs['Metallic'].default_value = 0


def attach(obj, rig, mat):
    obj.data.materials.clear()
    obj.data.materials.append(mat)
    group = obj.vertex_groups.new(name='DEF-spine.006')
    group.add(list(range(len(obj.data.vertices))), 1.0, 'REPLACE')
    mod = obj.modifiers.new('Head skin', 'ARMATURE')
    mod.object = rig
    for poly in obj.data.polygons:
        poly.use_smooth = True


def ellipsoid(name, center, radius, rig, mat):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=32, ring_count=16, location=(0, 0, 0))
    obj = bpy.context.object
    obj.name = name
    for v in obj.data.vertices:
        v.co = Vector((center[0] + v.co.x * radius[0], center[1] + v.co.y * radius[1],
                       center[2] + v.co.z * radius[2]))
    attach(obj, rig, mat)
    return obj


def hair_mesh(rig, mat):
    # An open, conforming cap extends at the sides and nape; the face is open.
    # Each strip converges under the cap; a dark hairline remains visible at temples.
    vertices, faces = [], []
    count, rows = 48, 9
    for row in range(rows):
        u = row / (rows - 1)
        for j in range(count + 1):
            t = -2.16 + 4.32 * j / count  # back and sides, never across face
            lower = 1.525 + .055 * (abs(t) / 2.16) ** 2
            z = lower + u * (1.687 - lower)
            # Hair traces a softly tapered skull, wider above the nape.
            factor = .78 + .22 * math.sin(math.pi * u / 2)
            vertices.append((.090 * factor * math.sin(t), -.046 + .111 * factor * math.cos(t), z))
    for row in range(rows-1):
        for j in range(count):
            a = row*(count+1)+j
            faces.append((a, a+1, a+count+2, a+count+1))
    mesh = bpy.data.meshes.new('Hair surface'); mesh.from_pydata(vertices, [], faces); mesh.update()
    obj = bpy.data.objects.new('Dark hair', mesh); bpy.context.collection.objects.link(obj)
    attach(obj, rig, mat)


def author(master, output):
    if output.exists():
        raise FileExistsError(output)
    if hashlib.sha256(master.read_bytes()).hexdigest() != MASTER_SHA:
        raise ValueError('Unexpected master; refusing to modify another source')
    bpy.ops.wm.open_mainfile(filepath=str(master.resolve()))
    rig = bpy.data.objects['VillagerRig']
    if len(rig.animation_data.drivers) != 162 or not all(bpy.data.actions.get(name) for name in CLIPS):
        raise ValueError('Unexpected rig drivers or missing source actions')
    for name in HIDDEN:
        obj = bpy.data.objects[name]
        obj.hide_render = True
        obj.hide_viewport = True
    set_color(bpy.data.materials['Olive ochre robe'], (.20, .105, .050), .94)
    set_color(bpy.data.materials['Ivory woven wrap'], (.59, .57, .52), .94)
    plum = material('Plum woven cuffs', (.20, .045, .105), .95)
    cap = material('Brown cap cloth', (.135, .068, .030), .95)
    hair = material('Dark brown hair', (.035, .019, .012), .90)
    upper = bpy.data.objects['Human.donitz_monk_robe']
    upper.data.materials.append(plum)
    bands = [0, 0]
    for poly in upper.data.polygons:
        c = poly.center
        sign = 1 if c.x >= 0 else -1
        bone = rig.data.bones['DEF-forearm.' + ('L.001' if sign > 0 else 'R.001')]
        axis = bone.tail_local - bone.head_local
        along = (c - bone.head_local).dot(axis) / axis.length_squared
        # A band around the distal forearm, avoiding body-facing garment faces.
        if .67 <= along <= .94 and abs(c.x) > .40:
            poly.material_index = 1
            bands[sign < 0] += 1
    if min(bands) < 20:
        raise ValueError(f'Unexpected sleeve geometry: {bands}')
    ellipsoid('Brown cap', (0, -.047, 1.686), (.095, .107, .053), rig, cap)
    ellipsoid('Brown cap brim', (0, -.060, 1.657), (.099, .113, .014), rig, cap)
    hair_mesh(rig, hair)
    if not all(bpy.data.objects[n].type == 'MESH' and not bpy.data.objects[n].hide_render for n in MESHES):
        raise ValueError('Expected all export meshes to be visible')
    output.parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.wm.save_as_mainfile(filepath=str(output.resolve()), compress=True)
    print('BROWN_BANDS', bands, 'MESHES', list(MESHES), flush=True)


def measure(output):
    """Measure actual evaluated whole-costume ground contact on every frame."""
    if output.exists():
        raise FileExistsError(output)
    rig = bpy.data.objects['VillagerRig']
    scene = bpy.context.scene
    meshes = [bpy.data.objects[name] for name in MESHES]
    if len(rig.animation_data.drivers) != 162 or any(not t.mute and len(t.strips) for t in rig.animation_data.nla_tracks):
        raise ValueError('Unexpected rig drivers or active NLA tracks')
    if any(obj.type != 'MESH' or obj.hide_render for obj in meshes):
        raise ValueError('Expected all export meshes to be visible')
    report = {'meshes': MESHES, 'fps': scene.render.fps / scene.render.fps_base, 'clips': {},
              'hitOverlayBones': ['DEF-spine.003', 'DEF-spine.004', 'DEF-spine.005', 'DEF-spine.006']}
    if not all(name in rig.data.bones for name in report['hitOverlayBones']):
        raise ValueError('Missing hit overlay bone')
    for name in CLIPS:
        action = bpy.data.actions[name]
        rig.animation_data.action = action
        rig.animation_data.action_slot = action.slots[0]
        first, last = map(round, action.frame_range)
        samples = []
        for frame in range(first, last + 1):
            scene.frame_set(frame)
            bpy.context.view_layer.update()
            deps = bpy.context.evaluated_depsgraph_get()
            lo, hi = math.inf, -math.inf
            for obj in meshes:
                evaluated = obj.evaluated_get(deps)
                mesh = evaluated.to_mesh()
                try:
                    heights = [(evaluated.matrix_world @ vertex.co).z for vertex in mesh.vertices]
                    if not heights or not all(math.isfinite(z) for z in heights):
                        raise ValueError('Empty or non-finite costume geometry: ' + obj.name)
                    lo, hi = min(lo, min(heights)), max(hi, max(heights))
                finally:
                    evaluated.to_mesh_clear()
            samples.append({'frame': frame, 'minimum': lo, 'maximum': hi})
            if name == 'Idle' and frame == first:
                report['heightAboveGroundOrigin'] = hi
        minimum = min(sample['minimum'] for sample in samples)
        if minimum < -.003:
            raise ValueError(f'{name} costume penetrates ground: {minimum}')
        report['clips'][name] = {'first': first, 'last': last, 'minimum': minimum, 'samples': samples}
        print('CHECKED_COSTUME', name, len(samples), minimum, flush=True)
    output.write_text(json.dumps(report, indent=2) + '\n')
    return report


def package(master, output):
    """Author a fresh costume and export textures, FBX and import metadata."""
    if output.exists():
        raise FileExistsError(output)
    root = Path(__file__).resolve().parents[3]
    speeds_path = root / 'research/private/generated/human-figure-20261109/mpfb-motion02/measurements-audited.json'
    speeds_data = speeds_path.read_bytes()
    if hashlib.sha256(speeds_data).hexdigest() != SPEEDS_SHA:
        raise ValueError('Unexpected audited gait measurements')
    if hashlib.sha256(master.read_bytes()).hexdigest() != MASTER_SHA:
        raise ValueError('Unexpected master; refusing to modify another source')
    output.mkdir(parents=True, exist_ok=False)
    prepared = output / 'brown-master.blend'
    author(master, prepared)
    measurement = measure(output / 'costume.json')
    pipeline = Path(__file__).resolve().parent
    export = output / 'export'

    def run(script, arguments, log_name):
        command = [bpy.app.binary_path, '-b', str(prepared), '--python-exit-code', '2',
                   '--python', str(pipeline / script), '--', *map(str, arguments)]
        with (output / log_name).open('x') as log:
            subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=3600)

    mesh_args = [item for name in MESHES for item in ('--mesh', name)]
    run('character_textures.py', ['--output', export, *mesh_args], 'materials.log')
    fbx = export / 'VillagerBrown.fbx'
    action_args = [item for name in CLIPS for item in ('--action', name)]
    run('export_character_fbx.py', ['--output', fbx, '--animations', '--bake-rig', 'VillagerRig',
                                    '--join-skins', *mesh_args, *action_args], 'export.log')
    result = json.loads(fbx.with_suffix('.export.json').read_text())
    maps = json.loads((export / 'materials.json').read_text())
    speeds = json.loads(speeds_data)
    if any(material['metallic'] != 0 for material in maps['materials']):
        raise ValueError('Expected nonmetallic character materials')
    manifest = {'height': measurement['heightAboveGroundOrigin'], 'walkSpeed': speeds['walkSpeed'],
                'runSpeed': speeds['runSpeed'], 'hitOverlayBones': measurement['hitOverlayBones'],
                'minBoneWeight': 1e-7, 'clips': result['clips'],
                'materials': [{k: mat[k] for k in ('name', 'albedo', 'orm')} for mat in maps['materials']]}
    (export / 'VillagerBrown.clips.json').write_text(json.dumps(manifest, indent=2) + '\n')
    receipt = {'schema': 'mc-villager-brown-package/v1', 'inputSha256': MASTER_SHA,
               'speedMeasurementsSha256': SPEEDS_SHA,
               'masterSha256': hashlib.sha256(prepared.read_bytes()).hexdigest(),
               'fbxSha256': hashlib.sha256(fbx.read_bytes()).hexdigest(),
               'height': manifest['height'], 'deformBones': result['deformBones'],
               'sourceToGameMaxMeters': max(v['maxError'] for v in result['surfaceProof']),
               'blenderFbxMaxMeters': max(v['maxError'] for v in result['reimportSurfaceProof']),
               'costumeMinimumMeters': min(v['minimum'] for v in measurement['clips'].values()),
               'scope': 'Blender package; Unity, gameplay, Voxel and installation are not qualified'}
    (output / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print('PACKAGED', json.dumps(receipt), flush=True)


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--master', required=True, type=Path)
    p.add_argument('--output', required=True, type=Path)
    p.add_argument('--package', action='store_true', help='Author and export a complete package in a fresh directory')
    args = p.parse_args(sys.argv[sys.argv.index('--')+1:])
    if args.package:
        package(args.master.resolve(), args.output.resolve())
    else:
        author(args.master.resolve(), args.output.resolve())


if __name__ == '__main__':
    main()
