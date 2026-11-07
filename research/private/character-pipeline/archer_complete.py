"""Assemble synchronized archer body, bow and arrow into an FBX timeline.

Run with Blender 4.5 and --walk/--shot/--kneel editable blend inputs. The
single timeline is split into gameplay clips by the Unity model importer.
"""
import argparse
import json
import math
import sys
from pathlib import Path

import bpy
from mathutils import Matrix, Quaternion, Vector

sys.path.insert(0, str(Path(__file__).resolve().parent))
from archer_shot import solve_arm
from archer_kneel import _solve_leg, _rotate_segment
from character_textures import extract_packed


def smooth(t):
    t = max(0.0, min(1.0, t))
    return t * t * (3 - 2 * t)


def bow_tip_close(draw, x, z):
    """Keep both halves of the displaced string at their original length."""
    radial = x * x + (.5 * draw) ** 2
    low, high = 0.0, .58
    for _ in range(32):
        half_span = (low + high) / 2
        length = math.sqrt(radial + (half_span - z) ** 2) + math.sqrt(radial + (half_span + z) ** 2)
        if length < 1.16:
            low = half_span
        else:
            high = half_span
    return 1 - (low + high) / 1.16


def blend_matrix(a, b, t):
    al, aq, asc = a.decompose()
    bl, bq, bsc = b.decompose()
    return Matrix.LocRotScale(al.lerp(bl, t), aq.slerp(bq, t), asc.lerp(bsc, t))


def capture(path, frames):
    bpy.ops.wm.open_mainfile(filepath=str(path.resolve()))
    rig = next(o for o in bpy.context.scene.objects if o.type == 'ARMATURE')
    out = []
    for frame in frames:
        bpy.context.scene.frame_set(int(frame), subframe=frame % 1)
        bpy.context.view_layer.update()
        out.append(({b.name: b.matrix_basis.copy() for b in rig.pose.bones}, rig.location.copy()))
    return out


def interpolate(a, b, t):
    return ({n: blend_matrix(m, b[0][n], t) for n, m in a[0].items()}, a[1].lerp(b[1], t))


def main():
    p = argparse.ArgumentParser()
    for name in ('walk', 'shot', 'kneel', 'output'):
        p.add_argument('--' + name, type=Path, required=True)
    a = p.parse_args(sys.argv[sys.argv.index('--') + 1:])
    a.output.mkdir(parents=True, exist_ok=True)
    # The walk was authored at 96 Hz to hold sole contact between 24 Hz
    # presentation frames. Preserve every sample in the FBX timeline.
    walk = capture(a.walk, range(4, 101))
    kneel = capture(a.kneel, range(1, 20))
    shot = capture(a.shot, range(1, 20))
    scene = bpy.context.scene
    rig = next(o for o in scene.objects if o.type == 'ARMATURE')
    mesh = next(o for o in scene.objects if o.type == 'MESH' and o.parent == rig)
    control = bpy.data.objects['BowControl']
    arrow = bpy.data.objects['ArrowControl']
    strings = bpy.data.objects['Bowstring']
    limbs = [o for o in scene.objects if o.name.startswith('Bow limb')]
    tips = [o for o in scene.objects if o.name.startswith('Horn bow tip')]
    scene.frame_set(1)
    bpy.context.view_layer.update()
    # Near a relaxed nock the two hands almost coincide. Deriving direction
    # from their difference there produces unstable 180-degree bow rolls.
    aim_q = control.rotation_quaternion.copy()
    # The source pose carries a small upward pitch.  A level target needs a
    # level shaft and a level flight path; apply the same correction to the
    # entire bow frame before solving the string and hand contacts below.
    # This leaves target elevation to the runtime presenter instead of baking
    # one arbitrary target into the asset.
    aim_forward = aim_q @ Vector((0, 1, 0))
    level_forward = Vector((aim_forward.x, aim_forward.y, 0)).normalized()
    aim_q = aim_forward.rotation_difference(level_forward) @ aim_q
    # The source is an NLA animation. Clear it after sampling, including shape
    # animations, so the assembled timeline has a single authoritative writer.
    for obj in scene.objects:
        obj.animation_data_clear()
        if obj.type == 'MESH' and obj.data.shape_keys:
            obj.data.shape_keys.animation_data_clear()
    # Walk phase zero has a raised left shoe. The standing pose uses the
    # grounded shot pelvis and legs with the low-held-bow upper body.
    leg_names = {'Hips', 'LeftUpLeg', 'LeftLeg', 'LeftFoot', 'LeftToeBase',
                 'RightUpLeg', 'RightLeg', 'RightFoot', 'RightToeBase'}
    standing = ({name: shot[0][0][name].copy() if name in leg_names else matrix.copy()
                 for name, matrix in walk[0][0].items()}, shot[0][1].copy())
    for name, matrix in standing[0].items():
        rig.pose.bones[name].matrix_basis = matrix
    rig.location = standing[1]
    bpy.context.view_layer.update()
    death_feet = {side: (rig.matrix_world @ rig.pose.bones[side + 'Foot'].head).copy()
                  for side in ('Left', 'Right')}
    for name, matrix in kneel[0][0].items():
        rig.pose.bones[name].matrix_basis = matrix
    rig.location = kneel[0][1]
    bpy.context.view_layer.update()
    plant = rig.matrix_world @ rig.pose.bones['LeftFoot'].head
    # The original kneel put the ankle beyond standing leg reach. The closer
    # plant keeps the same planted foot reachable as the body rises.
    plant.y += .126
    plant.z += .0015
    hand_ids = {}
    for side, lo, hi in [('Left', .10, .14), ('Right', .13, .17)]:
        fore = rig.matrix_world @ rig.data.bones[side + 'ForeArm'].head_local
        wrist = rig.matrix_world @ rig.data.bones[side + 'Hand'].head_local
        axis = (wrist - fore).normalized()
        span = (wrist - fore).length / .237
        group = mesh.vertex_groups[side + 'Hand']
        hand_ids[side] = [v.index for v in mesh.data.vertices
                          if next((g.weight for g in v.groups if g.group == group.index), 0) > .5
                          and lo * span < (mesh.matrix_world @ v.co - wrist).dot(axis) < hi * span]
        if len(hand_ids[side]) < 5:
            raise ValueError('Missing hand surface: ' + side)
    carry_up = Vector((0, .40, .9165)).normalized()
    carry_fwd = Vector((0, carry_up.z, -carry_up.y))
    carry_q = Matrix((carry_fwd.cross(carry_up).normalized(), carry_fwd, carry_up)).transposed().to_quaternion()
    # Keep the held bow off the floor as the body falls face-down.
    carry_axis = carry_q @ Vector((0, 0, 1))
    horizontal_axis = Vector((carry_axis.x, carry_axis.y, 0)).normalized()
    fallen_carry_q = carry_axis.rotation_difference(horizontal_axis) @ carry_q
    scene.render.fps = 24
    scene.frame_start, scene.frame_end = 1, 386
    launch_location = launch_rotation = None
    previous_location = previous_rotation = None
    was_visible = False
    visible_world = visible_rotation = None

    def surface_hands():
        evaluated_obj = mesh.evaluated_get(bpy.context.evaluated_depsgraph_get())
        evaluated = evaluated_obj.to_mesh()
        result = {side: sum((evaluated_obj.matrix_world @ evaluated.vertices[i].co for i in ids), Vector()) / len(ids)
                  for side, ids in hand_ids.items()}
        evaluated_obj.to_mesh_clear()
        return result

    def body_floor():
        evaluated_obj = mesh.evaluated_get(bpy.context.evaluated_depsgraph_get())
        evaluated = evaluated_obj.to_mesh()
        floor = min((evaluated_obj.matrix_world @ vertex.co).z for vertex in evaluated.vertices)
        evaluated_obj.to_mesh_clear()
        return floor

    for f in range(1, 387):
        # The initial standing sequence includes an actual hand-to-quiver
        # reach, extraction, nocking, draw, release and return to idle.
        if f <= 49:
            pose, mode, t = standing, 'idle', 0
        elif f <= 74:
            pose, mode, t = walk[(f - 50) * 4], 'walk', 0
        elif f <= 195:
            t = (f - 75) / 24
            if t < .5:
                pose = standing
            elif t < 1.65:
                pose = interpolate(standing, shot[0], smooth((t - .5) / 1.15))
            elif t < 2.35:
                pose = interpolate(shot[0], shot[16], smooth((t - 1.65) / .70))
            elif t < 3.15:
                pose = shot[16]
            elif t < 3.32:
                pose = shot[min(18, 16 + int((t - 3.15) * 12))]
            else:
                pose = interpolate(shot[18], standing, smooth((t - 3.45) / 1.15))
            mode = 'shot'
        elif f <= 267:
            t = (f - 196) / 24
            if t < .5:
                pose = standing
            elif t < 1.65:
                pose = interpolate(standing, shot[0], smooth((t - .5) / 1.15))
            else:
                pose = interpolate(shot[0], kneel[0], smooth((t - 1.65) / 1.35))
            mode = 'kneel_enter'
        elif f <= 315:
            t = (f - 268) / 24
            if t < .65:
                pose = interpolate(kneel[0], kneel[16], smooth(t / .65))
            elif t < 1.05:
                pose = kneel[16]
            else:
                pose = interpolate(kneel[16], kneel[0], smooth((t - 1.15) / .8))
            mode = 'kneel_shot'
        elif f <= 340:
            pose = interpolate(kneel[0], standing, smooth((f - 316) / 24))
            mode, t = 'kneel_exit', (f - 316) / 24
        elif f <= 350:
            pose, mode, t = standing, 'hit', (f - 341) / 9
        else:
            pose, mode, t = standing, 'death', (f - 351) / 35
        for name, matrix in pose[0].items():
            bone = rig.pose.bones[name]
            bone.rotation_mode = 'QUATERNION'
            bone.matrix_basis = matrix
            if mode == 'idle' and name == 'Head':
                bone.rotation_quaternion @= Quaternion((1, 0, 0), .012 * math.sin(2 * math.pi * (f - 1) / 48))
        if mode == 'hit':
            rig.pose.bones['Spine02'].rotation_quaternion @= Quaternion((0, 0, 1), -.18 * math.sin(math.pi * t))
        elif mode == 'death':
            fall = smooth(t / .72)
            rig.pose.bones['Hips'].rotation_quaternion @= Quaternion((1, 0, 0), 1.50 * fall)
            rig.pose.bones['Spine02'].rotation_quaternion @= Quaternion((1, 0, 0), .18 * fall)
            rig.pose.bones['LeftUpLeg'].rotation_quaternion @= Quaternion((1, 0, 0), .28 * fall)
            rig.pose.bones['LeftLeg'].rotation_quaternion @= Quaternion((1, 0, 0), -.45 * fall)
            rig.pose.bones['RightArm'].rotation_quaternion @= Quaternion((1, 0, 0), -.30 * fall)
        rig.location = pose[1]
        bpy.context.view_layer.update()
        # Reach back over the shoulder and bring the extracted arrow to the
        # string. The fixed-length IK chain never scales the arm bones.
        if mode in {'shot', 'kneel_enter'} and .35 < t < 2.35:
            shoulder = rig.matrix_world @ rig.pose.bones['RightArm'].head
            current = rig.matrix_world @ rig.pose.bones['RightHand'].head
            quiver = shoulder + Vector((-.08, .13, .21))
            weight = smooth((t - .35) / .30) if t < .70 else (1 - smooth((t - 1.05) / .60))
            right_target = current.lerp(quiver, weight)
            left = rig.matrix_world @ rig.pose.bones['LeftHand'].head
            aiming = aim_q @ Vector((0, 1, 0))
            aiming.z = 0
            aiming.normalize()
            retract = smooth((t - 1.05) / .6) * (1 - smooth((t - 1.65) / .70))
            solve_arm(rig, 'LeftArm', 'LeftForeArm', 'LeftHand', left - aiming * (.23 * retract))
            left = rig.matrix_world @ rig.pose.bones['LeftHand'].head
            # At nocking the hands meet at the relaxed string; the bow arm
            # extends while the draw hand stays back, increasing tension.
            right_target = right_target.lerp(left - aiming * .10, retract)
            solve_arm(rig, 'RightArm', 'RightForeArm', 'RightHand', right_target)
            if retract > 0:
                for _ in range(3):
                    hands = surface_hands()
                    desired = hands['Left'] + aim_q @ Vector((0, -.095, 0))
                    wrist = rig.matrix_world @ rig.pose.bones['RightHand'].head
                    solve_arm(rig, 'RightArm', 'RightForeArm', 'RightHand',
                              wrist + (desired - hands['Right']) * retract)
        bpy.context.view_layer.update()
        if mode != 'walk':
            # Interpolating the standing and kneeling fixed-length chains can
            # sweep a shoe through the floor even when both endpoints stand on
            # it. Raise the shared root by the measured penetration, preserving
            # the skeletal pose and the planted lowest point.
            correction = .001 - body_floor()
            rig.location.z += correction if mode == 'death' else max(0, correction)
            bpy.context.view_layer.update()
        if mode == 'death':
            # Bend the legs against their planted feet before releasing them.
            free = smooth((t - .18) / .52)
            for side in ('Left', 'Right'):
                hip = rig.matrix_world @ rig.pose.bones[side + 'UpLeg'].head
                knee = rig.matrix_world @ rig.pose.bones[side + 'Leg'].head
                ankle = rig.matrix_world @ rig.pose.bones[side + 'Foot'].head
                target = death_feet[side].lerp(ankle, free)
                length = (knee - hip).length + (ankle - knee).length
                delta = target - hip
                if delta.length >= length - 1e-4:
                    target = hip + delta.normalized() * (length - 1e-4)
                _solve_leg(rig, side + 'UpLeg', side + 'Leg', side + 'Foot', target, Vector((0, -1, .2)))
            rig.location.z += .001 - body_floor()
            bpy.context.view_layer.update()
        # Maintain a stable planted front foot while the rear leg moves.
        if mode not in {'walk', 'death'}:
            hip = rig.matrix_world @ rig.pose.bones['LeftUpLeg'].head
            knee = rig.matrix_world @ rig.pose.bones['LeftLeg'].head
            ankle = rig.matrix_world @ rig.pose.bones['LeftFoot'].head
            length = (knee - hip).length + (ankle - knee).length
            direction = plant - hip
            target = plant if direction.length < length - 1e-4 else hip + direction.normalized() * (length - 1e-4)
            _solve_leg(rig, 'LeftUpLeg', 'LeftLeg', 'LeftFoot', target, Vector((0, -1, .2)))
            ankle = rig.matrix_world @ rig.pose.bones['LeftFoot'].head
            toe = rig.matrix_world @ rig.pose.bones['LeftToeBase'].head
            rest = rig.matrix_world.to_3x3() @ (rig.data.bones['LeftToeBase'].head_local - rig.data.bones['LeftFoot'].head_local)
            _rotate_segment(rig, 'LeftFoot', toe - ankle, rest)
            left_toe = rig.pose.bones['LeftToeBase']
            left_foot = rig.pose.bones['LeftFoot']
            left_toe.matrix = left_foot.matrix @ left_foot.bone.matrix_local.inverted() @ left_toe.bone.matrix_local
            bpy.context.view_layer.update()
        hands = surface_hands()
        if mode in {'idle', 'walk', 'hit', 'death'}:
            amount = 0
        elif mode == 'shot':
            amount = smooth((t - .7) / .95) * (1 - smooth((t - 3.45) / 1.15))
        elif mode == 'kneel_enter':
            amount = smooth((t - .7) / .95)
        elif mode == 'kneel_exit':
            amount = 1 - smooth(t)
        else:
            amount = 1
        control.location = hands['Left']
        control.rotation_mode = 'QUATERNION'
        control.rotation_quaternion = carry_q.slerp(aim_q, amount)
        if mode == 'death':
            control.rotation_quaternion = carry_q.slerp(fallen_carry_q, smooth(t / .72))
        bpy.context.view_layer.update()
        local = control.rotation_quaternion.inverted() @ (hands['Right'] - hands['Left'])
        engaged = ((mode == 'kneel_enter' and t >= 1.65) or
                   (mode == 'kneel_shot' and t < 1.05) or
                   (mode == 'shot' and 1.65 <= t < 3.15))
        tension = 1.0 if engaged else 0.0
        draw = max(0, min(1, -local.y - .095)) * tension
        xval = max(-3, min(3, local.x / .2)) * tension
        zval = max(-3, min(3, local.z / .2)) * tension
        close = bow_tip_close(draw, .2 * xval, .2 * zval)
        for obj in [strings, *limbs]:
            for name, value in [('TipBack', draw), ('TipClose', close), ('Draw', draw), ('NockX', xval), ('NockZ', zval)]:
                if name in obj.data.shape_keys.key_blocks:
                    shape = obj.data.shape_keys.key_blocks[name]
                    shape.value = value
                    shape.keyframe_insert(data_path='value', frame=f)
        for tip in tips:
            tip.location.y = -.5 * draw
            tip.location.z = (-.58 if tip.name.endswith('.001') else .58) * close
            tip.keyframe_insert(data_path='location', frame=f)
        arrow.rotation_mode = 'QUATERNION'
        arrow.rotation_quaternion = Quaternion()
        if mode in {'shot', 'kneel_enter'} and .70 <= t < 1.65:
            # During extraction the arrow follows the moving hand; its nock
            # reaches the string continuously as the hand approaches it.
            arrow.location = local + Vector((0, .11, 0))
            visible = True
        elif engaged:
            arrow.location = (.2 * xval, -.095 - draw + .11, .2 * zval)
            visible = True
        elif (mode == 'shot' and 3.15 <= t < 3.55) or (mode == 'kneel_shot' and 1.05 <= t < 1.45):
            launch_time = 3.15 if mode == 'shot' else 1.05
            if launch_location is None:
                launch_location, launch_rotation = previous_location.copy(), previous_rotation.copy()
            world = launch_location + (launch_rotation @ Vector((0, 1, 0))) * (8 * (t - launch_time))
            arrow.location = control.matrix_world.inverted() @ world
            arrow.rotation_quaternion = control.rotation_quaternion.inverted() @ launch_rotation
            visible = True
        else:
            if was_visible:
                # Shrink at the last flight location before moving the hidden
                # prop; sampled FBX interpolation must not show it flying back.
                arrow.location = control.matrix_world.inverted() @ visible_world
                arrow.rotation_quaternion = control.rotation_quaternion.inverted() @ visible_rotation
            elif mode in {'shot', 'kneel_enter'} and .35 <= t < .70:
                # Stage the invisible arrow at the approaching hand before it
                # becomes visible, avoiding an interpolated bow-to-quiver jump.
                arrow.location = local + Vector((0, .11, 0))
            else:
                arrow.location = (0, 0, 0)
            visible = False
            launch_location = launch_rotation = None
        if engaged:
            previous_location = control.matrix_world @ arrow.location
            previous_rotation = control.rotation_quaternion.copy()
        if visible:
            visible_world = control.matrix_world @ arrow.location
            visible_rotation = control.rotation_quaternion @ arrow.rotation_quaternion
        was_visible = visible
        arrow.scale = (1, 1, 1) if visible else (.001, .001, .001)
        for obj in (rig, control, arrow):
            for path in (('location',) if obj == rig else ('location', 'rotation_quaternion', 'scale')):
                obj.keyframe_insert(data_path=path, frame=f)
        for bone in rig.pose.bones:
            for path in ('location', 'rotation_quaternion', 'scale'):
                bone.keyframe_insert(data_path=path, frame=f, group=bone.name)
    # Fill the intervening gait samples on the same rig and bow control.
    for index, pose in enumerate(walk):
        frame = 50 + index / 4
        scene.frame_set(int(frame), subframe=frame % 1)
        rig.location = pose[1]
        for name, matrix in pose[0].items():
            bone = rig.pose.bones[name]
            bone.rotation_mode = 'QUATERNION'
            bone.matrix_basis = matrix
        bpy.context.view_layer.update()
        control.location = surface_hands()['Left']
        control.rotation_quaternion = carry_q
        rig.keyframe_insert(data_path='location', frame=frame)
        control.keyframe_insert(data_path='location', frame=frame)
        control.keyframe_insert(data_path='rotation_quaternion', frame=frame)
        for bone in rig.pose.bones:
            for path in ('location', 'rotation_quaternion', 'scale'):
                bone.keyframe_insert(data_path=path, frame=frame, group=bone.name)
    # Preserve contact between integer samples as the legs descend and rise.
    # The bow is independent of the skeleton, so apply exactly the same root
    # correction to its control at those subframes.
    for quarter in range(75 * 4 + 1, 386 * 4):
        if quarter % 4 == 0:
            continue
        frame = quarter / 4
        scene.frame_set(int(frame), subframe=frame % 1)
        delta = .001 - body_floor()
        if frame < 351:
            delta = max(0, delta)
        if abs(delta) > 1e-7:
            rig.location.z += delta
            control.location.z += delta
            rig.keyframe_insert(data_path='location', frame=frame)
            control.keyframe_insert(data_path='location', frame=frame)
    # Blender and Unity interpolate the skinned hand nonlinearly between the
    # authored 24 Hz poses. Re-solve the nock and grip at quarter frames, not
    # just the endpoints, so the actual imported animation preserves contact.
    engaged_ranges = ((75 + 1.65 * 24, 75 + 3.15 * 24),
                      (196 + 1.65 * 24, 268), (268, 268 + 1.05 * 24))
    for quarter in range(75 * 4, 316 * 4 + 1):
        frame = quarter / 4
        if not any(start <= frame < end for start, end in engaged_ranges):
            continue
        scene.frame_set(int(frame), subframe=frame % 1)
        hands = surface_hands()
        control.location = hands['Left']
        control.rotation_quaternion = aim_q
        bpy.context.view_layer.update()
        local = aim_q.inverted() @ (hands['Right'] - hands['Left'])
        draw = max(0, min(1, -local.y - .095))
        xval = max(-3, min(3, local.x / .2))
        zval = max(-3, min(3, local.z / .2))
        close = bow_tip_close(draw, .2 * xval, .2 * zval)
        control.keyframe_insert(data_path='location', frame=frame)
        control.keyframe_insert(data_path='rotation_quaternion', frame=frame)
        for obj in [strings, *limbs]:
            for name, value in [('TipBack', draw), ('TipClose', close), ('Draw', draw), ('NockX', xval), ('NockZ', zval)]:
                if name in obj.data.shape_keys.key_blocks:
                    shape = obj.data.shape_keys.key_blocks[name]
                    shape.value = value
                    shape.keyframe_insert(data_path='value', frame=frame)
        for tip in tips:
            tip.location.y = -.5 * draw
            tip.location.z = (-.58 if tip.name.endswith('.001') else .58) * close
            tip.keyframe_insert(data_path='location', frame=frame)
        arrow.location = (.2 * xval, -.095 - draw + .11, .2 * zval)
        arrow.keyframe_insert(data_path='location', frame=frame)
    # Arrow visibility is an event, not an interpolated tiny floating arrow.
    for layer in arrow.animation_data.action.layers:
        for strip in layer.strips:
            for bag in strip.channelbags:
                for curve in bag.fcurves:
                    if curve.data_path == 'scale':
                        for point in curve.keyframe_points:
                            point.interpolation = 'CONSTANT'
    scene.frame_set(1)
    extract_packed(a.output, {'Image_0': 'body_albedo.jpg', 'Image_1': 'body_orm.jpg',
                              'normal': 'body_normal.png'})
    bpy.ops.wm.save_as_mainfile(filepath=str((a.output / 'archer.blend').resolve()))
    bpy.ops.export_scene.fbx(filepath=str((a.output / 'archer.fbx').resolve()),
        axis_forward='-Z', axis_up='Y', add_leaf_bones=False,
        path_mode='COPY', embed_textures=True, use_mesh_modifiers=False,
        bake_anim=True, bake_anim_use_nla_strips=False,
        bake_anim_use_all_actions=False, bake_anim_step=.25, bake_anim_simplify_factor=0.0)
    (a.output / 'archer.clips.json').write_text(json.dumps({
        'fps': 24, 'forward': [0, -1, 0], 'coordinateSystem': 'Blender Z-up',
        'height': 1.80543, 'material': {
            'name': 'Material_0', 'albedo': 'textures/body_albedo.jpg',
            'normal': 'textures/body_normal.png', 'orm': 'textures/body_orm.jpg',
            'ormChannels': 'R=occlusion G=roughness B=metallic',
        }, 'clips': [
            {'name': 'Idle', 'firstFrame': 1, 'lastFrame': 49, 'loop': True},
            {'name': 'Walk', 'firstFrame': 50, 'lastFrame': 74, 'loop': True},
            {'name': 'StandingShot', 'firstFrame': 75, 'lastFrame': 195, 'loop': False},
            {'name': 'KneelEnter', 'firstFrame': 196, 'lastFrame': 268, 'loop': False},
            {'name': 'KneelingShot', 'firstFrame': 268, 'lastFrame': 316, 'loop': False},
            {'name': 'KneelExit', 'firstFrame': 316, 'lastFrame': 340, 'loop': False},
            {'name': 'Hit', 'firstFrame': 341, 'lastFrame': 350, 'loop': False},
            {'name': 'Death', 'firstFrame': 351, 'lastFrame': 386, 'loop': False},
        ]}, indent=2) + '\n')
    print('ARCHER_TIMELINE_OK fps=24 idle=1..49 walk=50..74 shot=75..195 kneel_enter=196..268 kneel_shot=268..316 kneel_exit=316..340 hit=341..350 death=351..386', flush=True)


if __name__ == '__main__':
    main()
