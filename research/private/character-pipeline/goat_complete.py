"""Assemble the approved goat gait and a compatible breathing idle.

Output timeline (24 fps): idle 1..49; walk 50..74. The independent idle keeps
all four hooves planted; the walk keeps the existing four-beat gait. Both use
one bind skeleton and ground root for controller crossfades.
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


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--output', type=Path, required=True)
    a = p.parse_args(sys.argv[sys.argv.index('--') + 1:])
    a.output.mkdir(parents=True, exist_ok=True)
    scene = bpy.context.scene
    rig = next(o for o in scene.objects if o.type == 'ARMATURE')
    samples = []
    for frame in range(1, 26):
        scene.frame_set(frame)
        samples.append({b.name: b.matrix_basis.copy() for b in rig.pose.bones})
    rig.animation_data_clear()
    standing = neutral_standing(rig)
    scene.render.fps = 24
    scene.frame_start, scene.frame_end = 1, 74
    for frame in range(1, 75):
        if frame <= 49:
            pose = standing
        else:
            pose = samples[frame - 50]
        for name, matrix in pose.items():
            bone = rig.pose.bones[name]
            bone.rotation_mode = 'QUATERNION'
            bone.matrix_basis = matrix
            if frame <= 49 and name == 'head':
                # Two-second loop; endpoints and derivatives match exactly.
                t = (frame - 1) / 48
                bone.rotation_quaternion @= Quaternion((1, 0, 0), .018 * math.sin(2 * math.pi * t))
            for path in ('location', 'rotation_quaternion', 'scale'):
                bone.keyframe_insert(data_path=path, frame=frame, group=name)
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
        bake_anim_use_all_actions=False, bake_anim_simplify_factor=0.0)
    (a.output / 'goat.clips.json').write_text(json.dumps({
        'fps': 24, 'forward': [0, -1, 0], 'coordinateSystem': 'Blender Z-up',
        'height': 0.95917, 'material': {
            'name': next(m.name for m in bpy.data.materials if m.name.startswith('tripo_material_')),
            'albedo': 'textures/body_albedo.jpg', 'normal': 'textures/body_normal.png',
            'orm': 'textures/body_orm.jpg', 'ormChannels': 'R=occlusion G=roughness B=metallic',
        }, 'clips': [
            {'name': 'Idle', 'firstFrame': 1, 'lastFrame': 49, 'loop': True},
            {'name': 'Walk', 'firstFrame': 50, 'lastFrame': 74, 'loop': True},
        ]}, indent=2) + '\n')
    print('GOAT_TIMELINE_OK idle=1..49 walk=50..74 fps=24', flush=True)


if __name__ == '__main__':
    main()
