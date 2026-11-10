"""Author and package the source-inspired Builder on the accepted MPFB rig.

Run Blender --background --python builder_character.py -- --prepare --package
--output FRESH_DIRECTORY. Inputs remain untouched; editable anatomy and garments
are retained as separate objects. No paid generation or foreign animation.
"""
import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path

import bmesh
import bpy
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[3]
PIPELINE = Path(__file__).resolve().parent
MASTER = ROOT / 'research/private/generated/villager-modern-20261109/runtime-character/master/upper-prepared.blend'
MASTER_SHA = 'c36f1f0f8fcec31d9e0364faa5f8eacd4d45657d95aaf4796caf8dc52ab765c0'
SPEEDS = ROOT / 'research/private/generated/human-figure-20261109/mpfb-motion02/measurements-audited.json'
SPEEDS_SHA = '554a18fe551c44adfa1da18363958a19d90ee7331f33a882f204540c14346fd5'
ACTIONS = ('Idle', 'Walk', 'Run', 'Hit', 'Death')
MESHES = ('Human', 'Builder vest', 'Builder waistband', 'Separate olive trousers', 'Eye L', 'Iris L', 'Eye R', 'Iris R',
          'Brown shoe L', 'Brown shoe R', 'Brown cap', 'Brown cap brim', 'Dark hair', 'Builder beard')
HIDE = ('Human.donitz_monk_robe', 'Walk skeletal lower cloth', 'Turban crown', 'Turban wrap 1',
        'Turban wrap 2', 'Turban wrap 3', 'Right turban drape', 'Left turban drape',
        'Nape wrap', 'Front neck wrap')


def material(name, rgb, roughness=.94):
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    mat.diffuse_color = (*rgb, 1)
    shader = mat.node_tree.nodes.get('Principled BSDF')
    shader.inputs['Base Color'].default_value = (*rgb, 1)
    shader.inputs['Roughness'].default_value = roughness
    return mat


def stockier_body(human, rig):
    # Expand the existing rest-shape anatomy around its own arm axes, without
    # changing the Rigify rest skeleton or any of its actions.
    arm_groups = [g.index for g in human.vertex_groups if g.name.startswith('DEF-upper_arm.')]
    if not arm_groups or not human.data.shape_keys:
        raise ValueError('Expected editable human shape keys and upper-arm weights')
    bones = {side: (rig.data.bones['DEF-upper_arm.' + side].head_local.copy(),
                    rig.data.bones['DEF-upper_arm.' + side + '.001'].tail_local.copy())
             for side in ('L', 'R')}
    for v in human.data.vertices:
        weight = min(1.0, sum(g.weight for g in v.groups if g.group in arm_groups))
        for shape in human.data.shape_keys.key_blocks:
            point = shape.data[v.index].co
            # Broadening tapers at the waist and neck, and retains the same
            # baseline at the shoulders so attached head/arms remain aligned.
            chest = max(0, min(1, (point.z-1.04)/.16, (1.46-point.z)/.13))
            chest *= max(0, min(1, (.27-abs(point.x))/.09))
            point.x *= 1.0 + .09 * chest * (1-weight)
            if weight > .001:
                start, end = bones['L' if point.x >= 0 else 'R']
                axis = end-start
                t = max(0, min(1, (point-start).dot(axis)/axis.length_squared))
                center = start + axis*t
                point += (point-center) * (.14 * weight * min(1, t/.12, (1-t)/.12))
    human.data.update()


def beard_from_human(human, rig):
    # A conforming skinned chin beard and moustache follow the same face/head
    # weights, leaving the mouth uncovered. This is geometry, not a decal.
    states = [m.show_viewport for m in human.modifiers]
    for modifier in human.modifiers:
        modifier.show_viewport = False
    bpy.context.view_layer.update()
    deps = bpy.context.evaluated_depsgraph_get()
    beard = human.copy()
    beard.data = bpy.data.meshes.new_from_object(human.evaluated_get(deps), preserve_all_data_layers=True, depsgraph=deps)
    beard.name = 'Builder beard'
    bpy.context.collection.objects.link(beard)
    for modifier, state in zip(human.modifiers, states):
        modifier.show_viewport = state
    for modifier in list(beard.modifiers):
        beard.modifiers.remove(modifier)
    bm = bmesh.new()
    bm.from_mesh(beard.data)
    deform = bm.verts.layers.deform.active
    body_index = human.vertex_groups['body'].index
    discard = []
    for v in bm.verts:
        x, y, z = v.co
        chin = 1.488 < z < 1.547 and abs(x) < .060 and y < -.105
        moustache = 1.562 < z < 1.579 and abs(x) < .040 and y < -.13
        if not deform or v[deform].get(body_index, 0) < .5 or not (chin or moustache):
            discard.append(v)
    bmesh.ops.delete(bm, geom=discard, context='VERTS')
    boundary = [v for v in bm.verts if v.is_boundary]
    for _ in range(2):
        positions = {}
        for v in boundary:
            neighbours = [e.other_vert(v) for e in v.link_edges if e.is_boundary]
            if len(neighbours) == 2:
                positions[v] = v.co * .6 + (neighbours[0].co + neighbours[1].co) * .2
        for v, position in positions.items():
            v.co = position
    bm.normal_update()
    for v in bm.verts:
        v.co += v.normal * .0025
    bm.to_mesh(beard.data)
    bm.free()
    beard.data.update()
    if len(beard.data.polygons) < 40:
        raise ValueError('Unexpectedly small beard')
    beard.data.materials.clear()
    beard.data.materials.append(material('Builder beard hair', (.045, .022, .010), .9))
    for face in beard.data.polygons:
        face.use_smooth = True
    mod = beard.modifiers.new('Villager rig', 'ARMATURE')
    mod.object = rig
    return beard


def vest_from_human(human, rig):
    # Freeze the existing body shape in bind space, retaining actual body
    # topology and skin weights. An open front leaves the chest visible.
    states = [m.show_viewport for m in human.modifiers]
    for modifier in human.modifiers:
        modifier.show_viewport = False
    bpy.context.view_layer.update()
    deps = bpy.context.evaluated_depsgraph_get()
    vest = human.copy()
    vest.data = bpy.data.meshes.new_from_object(human.evaluated_get(deps), preserve_all_data_layers=True, depsgraph=deps)
    vest.name = 'Builder vest'
    bpy.context.collection.objects.link(vest)
    for modifier, state in zip(human.modifiers, states):
        modifier.show_viewport = state
    for modifier in list(vest.modifiers):
        vest.modifiers.remove(modifier)
    bm = bmesh.new()
    bm.from_mesh(vest.data)
    deform = bm.verts.layers.deform.active
    body_index = human.vertex_groups['body'].index
    remove = []
    for v in bm.verts:
        x, y, z = v.co
        # Open front, low armholes, and a short hem. Front panels widen at
        # the upper shoulder; the centre of the chest stays exposed.
        open_front = y < -.03 and abs(x) < (.065 if z > 1.36 else .090)
        if (not deform or v[deform].get(body_index, 0) < .5 or z < 1.035 or z > 1.445
                or abs(x) > (.205 if z > 1.32 else .17) or open_front):
            remove.append(v)
    bmesh.ops.delete(bm, geom=remove, context='VERTS')
    # Smooth the cropped seams before offsetting the fabric: body topology is
    # irregular, so raw selected-vertex boundaries form sharp square tabs.
    boundary = [v for v in bm.verts if v.is_boundary]
    for _ in range(4):
        positions = {}
        for v in boundary:
            neighbours = [edge.other_vert(v) for edge in v.link_edges if edge.is_boundary]
            if len(neighbours) == 2:
                positions[v] = v.co * .5 + (neighbours[0].co + neighbours[1].co) * .25
        for v, position in positions.items():
            v.co = position
    bm.normal_update()
    for v in bm.verts:
        v.co += v.normal * .010
    bm.to_mesh(vest.data)
    bm.free()
    vest.data.update()
    if len(vest.data.polygons) < 300:
        raise ValueError('Unexpectedly small vest surface')
    vest.data.materials.clear()
    vest.data.materials.append(material('Olive vest weave', (.20, .18, .045)))
    vest.data.materials.append(material('Ochre vest pattern', (.28, .24, .070)))
    for poly in vest.data.polygons:
        c = poly.center
        # Small broken woven squares; geometric material assignments survive
        # export without relying on unsupported procedural shader nodes.
        if (int((c.x + .3) * 55) + int((c.z - 1) * 55)) % 7 == 0:
            poly.material_index = 1
        poly.use_smooth = True
    mod = vest.modifiers.new('Villager rig', 'ARMATURE')
    mod.object = rig
    return vest


def waistband_from_human(human, rig, linen):
    # The hidden-under-robe source trousers have an uneven upper cut. A small
    # separately skinned linen waistband overlaps that edge without changing
    # the anatomy or needing a second skeleton.
    states = [m.show_viewport for m in human.modifiers]
    for modifier in human.modifiers:
        modifier.show_viewport = False
    bpy.context.view_layer.update()
    deps = bpy.context.evaluated_depsgraph_get()
    band = human.copy()
    band.data = bpy.data.meshes.new_from_object(human.evaluated_get(deps), preserve_all_data_layers=True, depsgraph=deps)
    band.name = 'Builder waistband'
    bpy.context.collection.objects.link(band)
    for modifier, state in zip(human.modifiers, states):
        modifier.show_viewport = state
    for modifier in list(band.modifiers):
        band.modifiers.remove(modifier)
    bm = bmesh.new()
    bm.from_mesh(band.data)
    deform = bm.verts.layers.deform.active
    body_index = human.vertex_groups['body'].index
    discard = [v for v in bm.verts if not deform or v[deform].get(body_index, 0) < .5
               or v.co.z < .91 or v.co.z > .985 or abs(v.co.x) > .22]
    bmesh.ops.delete(bm, geom=discard, context='VERTS')
    boundary = [v for v in bm.verts if v.is_boundary]
    for _ in range(5):
        positions = {}
        for v in boundary:
            neighbours = [e.other_vert(v) for e in v.link_edges if e.is_boundary]
            if len(neighbours) == 2:
                positions[v] = v.co * .5 + (neighbours[0].co + neighbours[1].co) * .25
        for v, position in positions.items():
            v.co = position
    bm.normal_update()
    for v in bm.verts:
        v.co += v.normal * .012
    bm.to_mesh(band.data)
    bm.free()
    band.data.update()
    band.data.materials.clear()
    band.data.materials.append(linen)
    for poly in band.data.polygons:
        poly.use_smooth = True
    mod = band.modifiers.new('Villager rig', 'ARMATURE')
    mod.object = rig
    if len(band.data.polygons) < 30:
        raise ValueError('Unexpectedly small waistband')
    return band


def prepare(output):
    if output.exists():
        raise FileExistsError(output)
    if hashlib.sha256(MASTER.read_bytes()).hexdigest() != MASTER_SHA:
        raise ValueError('Unexpected accepted source master')
    output.mkdir(parents=True)
    bpy.ops.wm.open_mainfile(filepath=str(MASTER))
    rig = bpy.data.objects['VillagerRig']
    if len(rig.animation_data.drivers) != 162 or not all(bpy.data.actions.get(a) for a in ACTIONS):
        raise ValueError('Source rig or actions differ from accepted master')
    for name in HIDE:
        obj = bpy.data.objects[name]
        obj.hide_render = obj.hide_viewport = True
    human = bpy.data.objects['Human']
    stockier_body(human, rig)
    beard_from_human(human, rig)
    human.modifiers['Delete.donitz_monk_robe'].show_viewport = False
    human.modifiers['Delete.donitz_monk_robe'].show_render = False
    vest_from_human(human, rig)
    # Hide only the anatomy inside trousers; the full body data remains intact.
    group = human.vertex_groups.new(name='Covered by Builder trousers')
    covered = [v.index for v in human.data.vertices if v.co.z < .80]
    group.add(covered, 1, 'REPLACE')
    mask = human.modifiers.new('Builder trousers skin coverage', 'MASK')
    mask.vertex_group = group.name
    mask.invert_vertex_group = True
    pants = bpy.data.objects['Separate olive trousers']
    pants.data.materials.clear()
    pants.data.materials.append(material('Pale linen trousers', (.61, .60, .65)))
    pants.data.materials.append(material('Green trouser bands', (.16, .30, .13)))
    waistband_from_human(human, rig, pants.data.materials[0])
    # Existing skinned trousers are fitted to the same body. Add fullness by
    # expanding around each leg, tapering to the waistband and ankle.
    for v in pants.data.vertices:
        x, y, z = v.co
        if z < .86:
            fullness = .022 * min(1, max(0, (z - .09) / .12)) * min(1, max(0, (.90-z) / .12))
            axis_x = .16 if x > 0 else -.16
            radial = Vector((x-axis_x, y+.025, 0))
            if radial.length > .0001:
                v.co += radial.normalized() * fullness
    pants.data.update()
    for poly in pants.data.polygons:
        z = poly.center.z
        if .13 <= z <= .155 or .185 <= z <= .210:
            poly.material_index = 1
    # The prior brown variant provides a small, separately skinned cap/hair.
    sys.path.insert(0, str(PIPELINE))
    from villager_brown import ellipsoid, hair_mesh
    cap = material('Builder brown cap', (.135, .068, .030))
    hair = material('Builder dark hair', (.035, .019, .012), .90)
    ellipsoid('Brown cap', (0, -.047, 1.686), (.095, .107, .053), rig, cap)
    ellipsoid('Brown cap brim', (0, -.060, 1.657), (.099, .113, .014), rig, cap)
    hair_mesh(rig, hair)
    if any(bpy.data.objects[n].hide_render for n in MESHES):
        raise ValueError('Missing visible costume mesh')
    bpy.ops.wm.save_as_mainfile(filepath=str(output / 'Builder-master.blend'), compress=True)
    print('PREPARED', output, flush=True)


def measure(path):
    if path.exists():
        raise FileExistsError(path)
    # Reuse the accepted all-frame whole-costume ground check unchanged.
    sys.path.insert(0, str(PIPELINE))
    import villager_brown
    villager_brown.MESHES = MESHES
    villager_brown.measure(path)


def package(output):
    master = output / 'Builder-master.blend'
    if not master.is_file():
        raise ValueError('Prepare the master first')
    if hashlib.sha256(SPEEDS.read_bytes()).hexdigest() != SPEEDS_SHA:
        raise ValueError('Unexpected gait measurements')
    export = output / 'export'
    def run(script, args, name):
        cmd = [bpy.app.binary_path, '-b', str(master), '--python-exit-code', '2', '--python',
               str(PIPELINE / script), '--', *map(str, args)]
        with (output / name).open('x') as log:
            subprocess.run(cmd, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=3600)
    run('builder_character.py', ['--measure', '--output', output / 'costume.json'], 'measure.log')
    meshargs = [item for n in MESHES for item in ('--mesh', n)]
    run('character_textures.py', ['--output', export, *meshargs], 'materials.log')
    fbx = export / 'Builder.fbx'
    run('export_character_fbx.py', ['--output', fbx, '--animations', '--bake-rig', 'VillagerRig',
                                   '--join-skins', *meshargs,
                                   *[item for a in ACTIONS for item in ('--action', a)]], 'export.log')
    report = json.loads(fbx.with_suffix('.export.json').read_text())
    measurement = json.loads((output / 'costume.json').read_text())
    maps = json.loads((export / 'materials.json').read_text())
    speeds = json.loads(SPEEDS.read_text())
    manifest = {'height': measurement['heightAboveGroundOrigin'], 'walkSpeed': speeds['walkSpeed'],
                'runSpeed': speeds['runSpeed'], 'hitOverlayBones': measurement['hitOverlayBones'],
                'minBoneWeight': 1e-7, 'clips': report['clips'],
                'materials': [{k: m[k] for k in ('name', 'albedo', 'orm')} for m in maps['materials']]}
    (export / 'Builder.clips.json').write_text(json.dumps(manifest, indent=2) + '\n')
    receipt = {'inputSha256': MASTER_SHA, 'masterSha256': hashlib.sha256(master.read_bytes()).hexdigest(),
               'fbxSha256': hashlib.sha256(fbx.read_bytes()).hexdigest(), 'deformBones': report['deformBones'],
               'sourceToGameMaxMeters': max(v['maxError'] for v in report['surfaceProof']),
               'blenderFbxMaxMeters': max(v['maxError'] for v in report['reimportSurfaceProof']),
               'costumeMinimumMeters': min(v['minimum'] for v in measurement['clips'].values()),
               'scope': 'Blender package only; Unity, gameplay, Voxel and installation unqualified'}
    (output / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print('PACKAGED', json.dumps(receipt), flush=True)


def render(output):
    # The sibling's renderer uses these names and samples the actual five poses.
    sys.path.insert(0, str(PIPELINE))
    import villager_striped
    villager_striped.NAMES = MESHES
    villager_striped.render(output)


def main():
    parser = argparse.ArgumentParser()
    op = parser.add_mutually_exclusive_group(required=True)
    op.add_argument('--prepare', action='store_true')
    op.add_argument('--measure', action='store_true')
    op.add_argument('--render', action='store_true')
    parser.add_argument('--package', action='store_true')
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args(sys.argv[sys.argv.index('--')+1:])
    output = args.output.resolve()
    if args.prepare:
        prepare(output)
        if args.package:
            package(output)
    elif args.measure:
        measure(output)
    else:
        render(output)


if __name__ == '__main__':
    main()
