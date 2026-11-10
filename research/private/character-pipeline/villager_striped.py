"""Build Villager 242's editable striped costume from the accepted 241 master.

Run in Blender with --prepare --package --output FRESH_DIRECTORY. The reference master is
hash-checked, never modified, and the directory must not already exist. The
robe's authored cloth topology, weights, actions and underlying human are kept.
"""
import argparse
import hashlib
import itertools
import json
import math
import struct
import subprocess
import sys
import zlib
from pathlib import Path

import bpy
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[3]
MASTER = ROOT / 'research/private/generated/villager-modern-20261109/runtime-character/master/upper-prepared.blend'
MASTER_SHA = 'c36f1f0f8fcec31d9e0364faa5f8eacd4d45657d95aaf4796caf8dc52ab765c0'
NAMES = ['Human', 'Human.donitz_monk_robe', 'Walk skeletal lower cloth', 'Separate olive trousers',
         'Eye L', 'Iris L', 'Eye R', 'Iris R', 'Turban crown', 'Turban wrap 1',
         'Turban wrap 2', 'Turban wrap 3', 'Right turban drape', 'Left turban drape',
         'Nape wrap', 'Front neck wrap', 'Brown shoe L', 'Brown shoe R']
ACTIONS = ['Idle', 'Walk', 'Run', 'Hit', 'Death']


def write_stripe_png(path):
    # sRGB dyed warp bands with a subtle deterministic weave. U is circumference,
    # so bands run down the actual cloth rather than across the moving garment.
    width, height = 1024, 256
    bands = [(0, 110, 108, 124), (11, 110, 108, 124), (14, 165, 161, 182),
             (31, 165, 161, 182), (34, 133, 130, 150), (52, 133, 130, 150),
             (55, 110, 108, 124), (64, 110, 108, 124)]
    rows = bytearray()
    for y in range(height):
        rows.append(0)
        for x in range(width):
            p = x % 64
            for a, b in itertools.pairwise(bands):
                if a[0] <= p < b[0]:
                    t = (p - a[0]) / (b[0] - a[0])
                    rgb = [round(a[i] * (1-t) + b[i] * t) for i in (1, 2, 3)]
                    break
            # Restrained thread irregularity, periodic in both axes.
            d = (1 if x % 3 == 0 else 0) + (1 if y % 4 == 0 else -1)
            rows.extend([min(255, max(0, c + d)) for c in rgb] + [255])
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
    data = (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0))
            + chunk(b'sRGB', b'\0') + chunk(b'IDAT', zlib.compress(rows, 9)) + chunk(b'IEND', b''))
    path.write_bytes(data)


def assign_cylindrical_uv(obj):
    # Angle around the body aligns the two independent robe meshes at the waist.
    # Resolve the branch cut per polygon to prevent horizontal streaks.
    uv = obj.data.uv_layers.get('UVMap') or obj.data.uv_layers.new(name='UVMap')
    obj.data.uv_layers.active = uv
    uv.active_render = True
    for poly in obj.data.polygons:
        values = []
        for loop_id in poly.loop_indices:
            p = obj.matrix_world @ obj.data.vertices[obj.data.loops[loop_id].vertex_index].co
            values.append(((math.atan2(p.x, -p.y) / (2 * math.pi)) % 1, p.z / 1.8))
        if max(u for u, _ in values) - min(u for u, _ in values) > .5:
            values = [(u + 1 if u < .5 else u, v) for u, v in values]
        for loop_id, (u, v) in zip(poly.loop_indices, values):
            uv.data[loop_id].uv = (u, v)


def set_flat(material, color):
    shader = material.node_tree.nodes.get('Principled BSDF')
    shader.inputs['Base Color'].default_value = (*color, 1)
    material.diffuse_color = (*color, 1)


def prepare(output):
    if output.exists():
        raise FileExistsError(output)
    if hashlib.sha256(MASTER.read_bytes()).hexdigest() != MASTER_SHA:
        raise ValueError('Accepted five-action input master hash mismatch')
    output.mkdir(parents=True)
    bpy.ops.wm.open_mainfile(filepath=str(MASTER))
    rig = bpy.data.objects['VillagerRig']
    if len(rig.animation_data.drivers) != 162 or not all(bpy.data.actions.get(a) for a in ACTIONS):
        raise ValueError('Accepted rig/action structure is incomplete')
    if not all(bpy.data.objects[n].type == 'MESH' for n in NAMES):
        raise ValueError('Expected named costume meshes')
    robe = bpy.data.materials['Olive ochre robe']
    robe.name = 'Lavender grey striped robe'
    set_flat(robe, (.32, .30, .38))
    trouser = robe.copy()
    trouser.name = 'Muted grey trousers'
    set_flat(trouser, (.23, .22, .27))
    bpy.data.objects['Separate olive trousers'].data.materials[0] = trouser
    wrap = bpy.data.materials['Ivory woven wrap']
    wrap.name = 'Pale lavender headwrap'
    set_flat(wrap, (.67, .65, .78))
    side = wrap.copy()
    side.name = 'Dark grey side cloth'
    set_flat(side, (.16, .14, .18))
    # Keep the pale brow wrap and front necktie, with darker side and nape cloth.
    for name in ['Right turban drape', 'Left turban drape', 'Nape wrap']:
        bpy.data.objects[name].data.materials[0] = side
    for name in ['Human.donitz_monk_robe', 'Walk skeletal lower cloth']:
        assign_cylindrical_uv(bpy.data.objects[name])
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from character_textures import export_constants
    material_dir = output / 'export'
    report = export_constants(material_dir, NAMES)
    texture_rel = 'textures/Lavender_grey_striped_robe_woven.png'
    texture_path = material_dir / texture_rel
    write_stripe_png(texture_path)
    for material in report['materials']:
        if material['name'] == robe.name:
            material['albedo'] = texture_rel
            material['description'] = 'Authored periodic vertical warp bands, sRGB'
    report['schema'] = 'mc-character-materials/v1'
    (material_dir / 'materials.json').write_text(json.dumps(report, indent=2) + '\n')
    image = bpy.data.images.load(str(texture_path), check_existing=False)
    image.name = '242 striped woven cloth'
    image.pack()
    node = robe.node_tree.nodes.new('ShaderNodeTexImage')
    node.image = image
    uv_node = robe.node_tree.nodes.new('ShaderNodeUVMap')
    uv_node.uv_map = 'UVMap'
    robe.node_tree.links.new(uv_node.outputs['UV'], node.inputs['Vector'])
    robe.node_tree.links.new(node.outputs['Color'], robe.node_tree.nodes['Principled BSDF'].inputs['Base Color'])
    # The original body, action slots, armature and offline cloth remain editable.
    bpy.ops.wm.save_as_mainfile(filepath=str(output / 'VillagerStriped-master.blend'))
    print('PREPARED', output, flush=True)




def measure(output):
    """Sample every original frame against the ground using all worn meshes."""
    if output.exists():
        raise FileExistsError(output)
    rig = bpy.data.objects['VillagerRig']
    scene = bpy.context.scene
    meshes = [bpy.data.objects[name] for name in NAMES]
    if len(rig.animation_data.drivers) != 162 or any(not t.mute and len(t.strips) for t in rig.animation_data.nla_tracks):
        raise ValueError('Unexpected rig drivers or active NLA tracks')
    if any(o.type != 'MESH' or o.hide_render for o in meshes):
        raise ValueError('Expected every named costume mesh to be visible')
    report = {'meshes': NAMES, 'fps': scene.render.fps / scene.render.fps_base,
              'drivers': 162, 'clips': {}, 'materials': sorted({m.name for o in meshes for m in o.data.materials})}
    for name in ACTIONS:
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
                    heights = [(evaluated.matrix_world @ v.co).z for v in mesh.vertices]
                    if not heights or not all(math.isfinite(value) for value in heights):
                        raise ValueError('Non-finite or empty geometry: ' + obj.name)
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
    report['hitOverlayBones'] = ['DEF-spine.003', 'DEF-spine.004', 'DEF-spine.005', 'DEF-spine.006']
    if not all(name in rig.data.bones for name in report['hitOverlayBones']):
        raise ValueError('Missing hit overlay bone')
    output.write_text(json.dumps(report, indent=2) + '\n')


def finish(output):
    """Measure, export and describe a prepared costume without overwriting outputs."""
    master = output / 'VillagerStriped-master.blend'
    export = output / 'export'
    measure = output / 'costume.json'
    fbx = export / 'VillagerStriped.fbx'
    manifest_path = export / 'VillagerStriped.clips.json'
    if not master.is_file() or not (export / 'materials.json').is_file():
        raise ValueError('Expected a prepared master and material export')
    if manifest_path.exists():
        raise FileExistsError(manifest_path)
    pipeline = Path(__file__).resolve().parent
    blender = bpy.app.binary_path
    def run(script, arguments, log_name):
        command = [blender, '-b', str(master), '--python-exit-code', '2', '--python',
                   str(script), '--', *map(str, arguments)]
        with (output / log_name).open('x') as log:
            subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=3600)
    if not measure.exists():
        run(pipeline / 'villager_striped.py', ['--measure', '--output', measure], 'measure.log')
    if not fbx.exists():
        arguments = ['--output', fbx, '--animations', '--bake-rig', 'VillagerRig', '--join-skins']
        for name in NAMES:
            arguments += ['--mesh', name]
        for name in ACTIONS:
            arguments += ['--action', name]
        run(pipeline / 'export_character_fbx.py', arguments, 'export.log')
    report = json.loads(fbx.with_suffix('.export.json').read_text())
    measurement = json.loads(measure.read_text())
    maps = json.loads((export / 'materials.json').read_text())
    speeds = json.loads((ROOT / 'research/private/generated/human-figure-20261109/mpfb-motion02/measurements-audited.json').read_text())
    manifest = {'height': measurement['heightAboveGroundOrigin'], 'walkSpeed': speeds['walkSpeed'],
                'runSpeed': speeds['runSpeed'], 'hitOverlayBones': measurement['hitOverlayBones'],
                'minBoneWeight': 1e-7, 'clips': report['clips'],
                'materials': [{k: m[k] for k in ('name', 'albedo', 'orm')} for m in maps['materials']]}
    manifest_path.write_text(json.dumps(manifest, indent=2) + '\n')
    receipt = {'inputSha256': MASTER_SHA,
               'masterSha256': hashlib.sha256(master.read_bytes()).hexdigest(),
               'fbxSha256': hashlib.sha256(fbx.read_bytes()).hexdigest(),
               'deformBones': report['deformBones'],
               'sourceToGameMaxMeters': max(v['maxError'] for v in report['surfaceProof']),
               'blenderFbxMaxMeters': max(v['maxError'] for v in report['reimportSurfaceProof']),
               'costumeMinimumMeters': min(v['minimum'] for v in measurement['clips'].values()),
               'scope': 'Blender package; Unity, gameplay, Voxel and installation are not qualified'}
    (output / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print('PACKAGED', json.dumps(receipt), flush=True)


def render(output):
    if output.exists():
        raise FileExistsError(output)
    output.mkdir(parents=True)
    scene = bpy.context.scene
    rig = bpy.data.objects['VillagerRig']
    scene.render.engine = 'CYCLES'
    scene.cycles.samples = 12
    scene.cycles.use_denoising = True
    scene.render.threads_mode = 'FIXED'
    scene.render.threads = 4
    scene.render.resolution_x, scene.render.resolution_y = 480, 600
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = 'PNG'
    scene.world = bpy.data.worlds.new('242 inspection world')
    scene.world.use_nodes = True
    scene.world.node_tree.nodes['Background'].inputs[0].default_value = (.18, .18, .21, 1)
    scene.world.node_tree.nodes['Background'].inputs[1].default_value = .7
    center = Vector((0, 0, .9))
    for name, pos, power, size in [('Key', (3, -4, 6), 750, 4), ('Fill', (-4, -2, 3), 400, 5), ('Rim', (2, 4, 5), 600, 3)]:
        data = bpy.data.lights.new(name, 'AREA')
        data.energy, data.shape, data.size = power, 'DISK', size
        light = bpy.data.objects.new(name, data)
        scene.collection.objects.link(light)
        light.location = pos
        light.rotation_euler = (center - light.location).to_track_quat('-Z', 'Y').to_euler()
    camera_data = bpy.data.cameras.new('Review camera')
    camera = bpy.data.objects.new('Review camera', camera_data)
    scene.collection.objects.link(camera)
    scene.camera = camera
    camera.data.type = 'ORTHO'
    camera.data.ortho_scale = 2.3
    for action, frame in [('Idle', 1), ('Walk', 31), ('Run', 20), ('Hit', 30), ('Death', 264)]:
        rig.animation_data.action = bpy.data.actions[action]
        rig.animation_data.action_slot = bpy.data.actions[action].slots[0]
        scene.frame_set(frame)
        bpy.context.view_layer.update()
        deps = bpy.context.evaluated_depsgraph_get()
        bounds = [obj.evaluated_get(deps).matrix_world @ Vector(corner)
                  for obj in (bpy.data.objects[n] for n in NAMES)
                  for corner in obj.evaluated_get(deps).bound_box]
        low = Vector(tuple(min(p[i] for p in bounds) for i in range(3)))
        high = Vector(tuple(max(p[i] for p in bounds) for i in range(3)))
        center = (low + high) * .5
        camera.data.ortho_scale = max(2.3, (high-low).length * 1.12)
        for angle in (0, 90):
            r = math.radians(angle)
            camera.location = center + Vector((math.sin(r)*5, -math.cos(r)*5, 1.1))
            camera.rotation_euler = (center - camera.location).to_track_quat('-Z', 'Y').to_euler()
            scene.render.filepath = str(output / f'{action}-{frame:03d}-{angle:03d}.png')
            bpy.ops.render.render(write_still=True)
    print('RENDERED', output, flush=True)


def main():
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--prepare', action='store_true')
    group.add_argument('--render', action='store_true')
    group.add_argument('--finish', action='store_true')
    group.add_argument('--measure', action='store_true')
    parser.add_argument('--package', action='store_true', help='After preparation, also measure and export the complete package')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args(sys.argv[sys.argv.index('--')+1:])
    if args.prepare:
        prepare(args.output.resolve())
        if args.package:
            finish(args.output.resolve())
    elif args.finish:
        finish(args.output.resolve())
    elif args.measure:
        measure(args.output.resolve())
    else:
        render(args.output.resolve())


if __name__ == '__main__':
    main()
