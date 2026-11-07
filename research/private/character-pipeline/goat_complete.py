"""Build a source-scaled goat gallop, reaction and death timeline.

The short contact phases allow a gallop to cover several body lengths without
the trip-trap cadence of the previous tiny walk. Playback is driven by actual
travel, not by changing simulation speed. Frame numbers are at 24 fps; gait
keys are sampled every quarter frame for contact preservation.
"""
import argparse
import json
import math
import sys
from pathlib import Path

import bpy
from mathutils import Matrix, Quaternion, Vector
sys.path.insert(0, str(Path(__file__).resolve().parent))
from character_textures import extract_packed


def neutral_standing(rig):
    """Place every hoof at its rest contact while retaining fixed leg lengths."""
    for bone in rig.pose.bones:
        bone.matrix_basis = Matrix.Identity(4)
    bob = -.010
    rig.pose.bones['body'].location = (0, 0, bob)
    bpy.context.view_layer.update()
    for name in ('hind_L', 'front_L', 'hind_R', 'front_R'):
        upper = rig.data.bones[name + '_upper']
        lower = rig.data.bones[name + '_lower']
        hip = upper.head_local + Vector((0, 0, bob))
        knee, ankle = lower.head_local.copy(), lower.tail_local.copy()
        a = (knee - upper.head_local).length
        b = (ankle - knee).length
        direction = ankle - hip
        distance = direction.length
        along = (a * a - b * b + distance * distance) / (2 * distance)
        height = math.sqrt(max(0, a * a - along * along))
        unit = direction.normalized()
        bend = Vector((0, unit.z, -unit.y))
        if name.startswith('front'):
            bend.negate()
        joint = hip + along * unit + height * bend
        for suffix, start, end in [('upper', hip, joint), ('lower', joint, ankle)]:
            pose = rig.pose.bones[name + '_' + suffix]
            rest = rig.data.bones[pose.name]
            rotation = (rest.tail_local - rest.head_local).rotation_difference(end - start)
            pose.matrix = Matrix.Translation(start) @ rotation.to_matrix().to_4x4() @ rest.matrix_local.to_3x3().to_4x4()
            bpy.context.view_layer.update()
        hoof = rig.pose.bones[name + '_hoof']
        rest = rig.data.bones[hoof.name]
        hoof.matrix = Matrix.Translation(ankle) @ rest.matrix_local.to_3x3().to_4x4()
        bpy.context.view_layer.update()
    return {b.name: b.matrix_basis.copy() for b in rig.pose.bones}


def gait_pose(rig, standing, t, stance):
    # Paired hind and fore contacts with a small left/right offset. At the
    # source sprite height (150/64), 0.16/stance model units per cycle is
    # multiplied by 150/64/0.95917 to obtain the real travel per cycle.
    phases = {'hind_L': 0.0, 'hind_R': .035, 'front_L': .48, 'front_R': .515}
    stride = .16
    for name, matrix in standing.items():
        rig.pose.bones[name].matrix_basis = matrix
    cycle = {name: (t + phase) % 1 for name, phase in phases.items()}
    # Body stays low at contact, lifting between contact groups.
    proximity = min(min(p, 1-p) for p in cycle.values())
    flight = min(1.0, max(0.0, (proximity - stance * .5) / .16))
    flight = flight * flight * (3 - 2 * flight)
    # Enough crouch to keep the longest rear reach inside the leg's actual
    # two-bone length throughout stance; otherwise the IK clamp slides.
    bob = -.06 + .045 * flight
    rig.pose.bones['body'].location = (0, 0, bob)
    bpy.context.view_layer.update()
    for name, phase in cycle.items():
        upper = rig.data.bones[name + '_upper']
        lower = rig.data.bones[name + '_lower']
        hip = upper.head_local + Vector((0, 0, bob))
        knee, ankle = lower.head_local.copy(), lower.tail_local.copy()
        if phase < stance:
            travel = stride * (.5 - phase / stance)
            lift = 0.0
        else:
            u = (phase - stance) / (1 - stance)
            eased = u * u * (3 - 2 * u)
            travel = stride * (-.5 + eased)
            lift = .10 * math.sin(math.pi * u) ** 2 + max(0, bob)
        target = ankle + Vector((0, travel, lift))
        a, b = (knee - upper.head_local).length, (ankle - knee).length
        direction = target - hip
        distance = direction.length
        if distance >= a + b:
            direction *= (a + b - 1e-6) / distance
            target = hip + direction
            distance = direction.length
        along = (a*a - b*b + distance*distance) / (2 * distance)
        height = math.sqrt(max(0, a*a - along*along))
        unit = direction.normalized()
        bend = Vector((0, unit.z, -unit.y))
        if name.startswith('front'):
            bend.negate()
        joint = hip + along * unit + height * bend
        for suffix, start, end in [('upper', hip, joint), ('lower', joint, target)]:
            bone = rig.pose.bones[name + '_' + suffix]
            rest = rig.data.bones[bone.name]
            rotation = (rest.tail_local - rest.head_local).rotation_difference(end - start)
            bone.matrix = Matrix.Translation(start) @ rotation.to_matrix().to_4x4() @ rest.matrix_local.to_3x3().to_4x4()
            bpy.context.view_layer.update()
        hoof = rig.pose.bones[name + '_hoof']
        rest = rig.data.bones[hoof.name]
        hoof.matrix = Matrix.Translation(target) @ rest.matrix_local.to_3x3().to_4x4()
        bpy.context.view_layer.update()


def key_pose(rig, frame):
    for bone in rig.pose.bones:
        bone.rotation_mode = 'QUATERNION'
        for path in ('location', 'rotation_quaternion', 'scale'):
            bone.keyframe_insert(data_path=path, frame=frame, group=bone.name)


def mesh_floor(scene):
    bpy.context.view_layer.update()
    deps = bpy.context.evaluated_depsgraph_get()
    bottoms = []
    for obj in scene.objects:
        if obj.type != 'MESH':
            continue
        evaluated = obj.evaluated_get(deps)
        mesh = evaluated.to_mesh()
        bottoms.extend((obj.matrix_world @ vertex.co).z for vertex in mesh.vertices)
        evaluated.to_mesh_clear()
    return min(bottoms)


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--output', type=Path, required=True)
    a = p.parse_args(sys.argv[sys.argv.index('--') + 1:])
    a.output.mkdir(parents=True, exist_ok=True)
    scene = bpy.context.scene
    rig = next(o for o in scene.objects if o.type == 'ARMATURE')
    rig.animation_data_clear()
    standing = neutral_standing(rig)
    standing_floor = mesh_floor(scene)
    scene.render.fps = 24
    scene.frame_start, scene.frame_end = 1, 146
    for frame in range(1, 50):
        for name, matrix in standing.items():
            rig.pose.bones[name].matrix_basis = matrix
        rig.pose.bones['head'].rotation_quaternion @= Quaternion((1, 0, 0), .018 * math.sin(2 * math.pi * (frame - 1) / 48))
        key_pose(rig, frame)
    for first, stance in [(50, .18), (76, .075)]:
        for sample in range(97):
            frame = first + sample / 4
            gait_pose(rig, standing, sample / 96, stance)
            key_pose(rig, frame)
    # Distinct, bounded non-looping reactions. Source death is reclaimed in
    # two visits, so the presentation owner retains the last pose briefly.
    for frame in range(102, 147):
        for name, matrix in standing.items():
            rig.pose.bones[name].matrix_basis = matrix
        if frame <= 112:
            u = (frame - 102) / 10
            rig.pose.bones['head'].rotation_quaternion @= Quaternion((1, 0, 0), -.22 * math.sin(math.pi * u))
        else:
            u = min(1, (frame - 113) / 20)
            u = u*u*(3-2*u)
            rig.pose.bones['body'].location.z -= .24 * u
            rig.pose.bones['body'].rotation_quaternion @= Quaternion((0, 1, 0), 1.45 * u)
            rig.pose.bones['head'].rotation_quaternion @= Quaternion((1, 0, 0), -.20 * u)
            for name in ('hind_L', 'hind_R', 'front_L', 'front_R'):
                bend = -1 if name.startswith('front') else 1
                rig.pose.bones[name + '_upper'].rotation_quaternion @= Quaternion((1, 0, 0), bend * .9 * u)
                rig.pose.bones[name + '_lower'].rotation_quaternion @= Quaternion((1, 0, 0), -bend * 1.65 * u)
            # Death may fold through the terrain. Project the deformed mesh's
            # lowest point back onto the original floor before recording it.
            floor_error = standing_floor - mesh_floor(scene)
            rig.pose.bones['body'].location.z += floor_error
            bpy.context.view_layer.update()
        key_pose(rig, frame)
    rig.animation_data.action.name = 'Goat_Timeline'
    scene.frame_set(1)
    # The source goat was authored around its torso. Present a ground-plane
    # root and match the archer's -Y authored forward without altering gait.
    floor = min((o.matrix_world @ v.co).z for o in scene.objects
                if o.type == 'MESH' for v in o.evaluated_get(
                    bpy.context.evaluated_depsgraph_get()).to_mesh().vertices)
    # Bake both the bind skeleton and its mesh into the new frame. A parent
    # transform on the armature is interpreted as a 100x translation by Unity's
    # FBX animation importer, even though Blender reimports it correctly.
    transform = Matrix.Rotation(math.pi, 4, 'Z') @ Matrix.Translation((0, 0, -floor))
    rig.data.transform(transform)
    for obj in scene.objects:
        if obj.type == 'MESH' and obj.parent == rig:
            obj.data.transform(transform)
    # Invalidate the evaluated pose after modifying the bind skeleton; the FBX
    # exporter otherwise bakes stale pre-transform matrices into animation.
    scene.frame_set(2)
    scene.frame_set(1)
    bpy.context.view_layer.update()
    images = list(bpy.data.images)
    extract_packed(a.output, {
        next(i.name for i in images if i.name.startswith('Color_')): 'body_albedo.jpg',
        next(i.name for i in images if i.name.startswith('NormalGL_')): 'body_normal.png',
        next(i.name for i in images if i.name.startswith('ORM_')): 'body_orm.jpg',
    })
    blend_path = (a.output / 'goat.blend').resolve()
    bpy.ops.wm.save_as_mainfile(filepath=str(blend_path))
    # Reopening refreshes Blender's cached bind matrices. Without this, the
    # exporter samples the pre-transform skeleton despite a correct viewport.
    bpy.ops.wm.open_mainfile(filepath=str(blend_path))
    bpy.ops.export_scene.fbx(filepath=str((a.output / 'goat.fbx').resolve()),
        axis_forward='-Z', axis_up='Y', add_leaf_bones=False,
        path_mode='COPY', embed_textures=True, use_mesh_modifiers=False,
        bake_anim=True, bake_anim_use_nla_strips=False,
        bake_anim_use_all_actions=False, bake_anim_simplify_factor=0.0,
        bake_anim_step=.25)
    (a.output / 'goat.clips.json').write_text(json.dumps({
        'fps': 24, 'forward': [0, -1, 0], 'coordinateSystem': 'Blender Z-up',
        'height': 0.95917, 'material': {
            'name': next(m.name for m in bpy.data.materials if m.name.startswith('tripo_material_')),
            'albedo': 'textures/body_albedo.jpg', 'normal': 'textures/body_normal.png',
            'orm': 'textures/body_orm.jpg', 'ormChannels': 'R=occlusion G=roughness B=metallic',
        }, 'clips': [
            {'name': 'Idle', 'firstFrame': 1, 'lastFrame': 49, 'loop': True},
            {'name': 'Walk', 'firstFrame': 50, 'lastFrame': 74, 'loop': True},
            {'name': 'Sprint', 'firstFrame': 76, 'lastFrame': 100, 'loop': True},
            {'name': 'Hit', 'firstFrame': 102, 'lastFrame': 112, 'loop': False},
            {'name': 'Death', 'firstFrame': 113, 'lastFrame': 146, 'loop': False},
        ]}, indent=2) + '\n')
    print('GOAT_TIMELINE_OK idle=1..49 walk=50..74 sprint=76..100 hit=102..112 death=113..146 fps=24', flush=True)


if __name__ == '__main__':
    main()
