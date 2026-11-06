"""Bind the approved textured goat to an articulated walking skeleton.

Run with Blender 4.5: blender -b -t 2 -P rig_goat.py -- --model model.glb --output DIR
The manually authored four-beat loop is inspired by the original four sprite
poses; it does not claim to recover their exact cadence or pixel motion.
"""
import argparse
import math
import sys
from pathlib import Path

import bpy
from mathutils import Matrix, Vector


def smooth(a, b, x):
    t = max(0.0, min(1.0, (x - a) / (b - a)))
    return t * t * (3.0 - 2.0 * t)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--model', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args(sys.argv[sys.argv.index('--') + 1:])
    args.output.mkdir(parents=True, exist_ok=True)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(args.model.resolve()))
    meshes = [obj for obj in bpy.context.scene.objects if obj.type == 'MESH']
    if len(meshes) != 1:
        raise ValueError('Expected the original single textured goat mesh')
    mesh = meshes[0]
    # The source node has a presentation yaw; remove it so forward is +Y.
    mesh.rotation_mode = 'XYZ'
    mesh.rotation_euler = (0, 0, 0)
    bpy.context.view_layer.objects.active = mesh
    mesh.select_set(True)
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    mesh.select_set(False)
    # Smooth two tiny reconstruction ridges on the rear neck and withers.
    # Weld equivalent positions only in the temporary adjacency, preserving UVs.
    vertices = mesh.data.vertices
    unique = {}
    reverse = []
    points = []
    for vertex in vertices:
        key = tuple(round(c, 5) for c in vertex.co)
        if key not in unique:
            unique[key] = len(points)
            points.append(vertex.co.copy())
        reverse.append(unique[key])
    neighbors = [set() for _ in points]
    for edge in mesh.data.edges:
        a, b = (reverse[i] for i in edge.vertices)
        if a != b:
            neighbors[a].add(b)
            neighbors[b].add(a)
    influences = []
    for point in points:
        effects = []
        for center, radius in [((0, .20, .28), (.075, .035, .040)),
                               ((0, .09, .18), (.075, .045, .035))]:
            distance = math.sqrt(sum(((point[i] - center[i]) / radius[i]) ** 2 for i in range(3)))
            effects.append(1 - smooth(.45, 1, distance))
        influences.append(max(effects))
    for _ in range(18):
        next_points = list(points)
        for index, adjacent in enumerate(neighbors):
            if adjacent and influences[index] > 0:
                average = sum((points[j] for j in adjacent), Vector()) / len(adjacent)
                next_points[index] = points[index].lerp(average, .65 * influences[index])
        points = next_points
    for vertex in vertices:
        vertex.co = points[reverse[vertex.index]]
    mesh.data.update()
    bpy.ops.object.armature_add()
    rig = bpy.context.object
    rig.name = 'GoatRig'
    bpy.ops.object.mode_set(mode='EDIT')
    bones = rig.data.edit_bones
    bones.remove(bones[0])
    body = bones.new('body')
    body.head, body.tail = (0, -.20, .02), (0, .13, .02)
    head = bones.new('head')
    head.head, head.tail, head.parent = (0, .22, .18), (0, .34, .27), body
    legs = {}
    for front, y in [(True, .125), (False, -.315)]:
        for side in [-1, 1]:
            key = ('front' if front else 'hind') + ('_L' if side > 0 else '_R')
            x = side * (.09 if front else .11)
            if front:
                ankle_y = .105 if side > 0 else .15
                hip_y = .13
                knee_y = .12 if side > 0 else .145
            else:
                ankle_y = -.365 if side > 0 else -.322
                hip_y = -.295
                knee_y = -.350 if side > 0 else -.330
            hip = Vector((x, hip_y, -.12))
            knee = Vector((x, knee_y, -.270))
            ankle = Vector((x, ankle_y, -.425))
            upper = bones.new(key + '_upper')
            upper.head, upper.tail, upper.parent = hip, knee, body
            lower = bones.new(key + '_lower')
            lower.head, lower.tail, lower.parent = knee, ankle, upper
            lower.use_connect = True
            foot = bones.new(key + '_hoof')
            foot.head, foot.tail, foot.parent = ankle, ankle + Vector((0, .06, 0)), lower
            foot.use_connect = True
            legs[key] = (hip, knee, ankle)
    bpy.ops.object.mode_set(mode='OBJECT')
    rig.show_in_front = True
    groups = {name: mesh.vertex_groups.new(name=name) for name in ['body', 'head'] + [k + '_' + part for k in legs for part in ['upper', 'lower', 'hoof']]}
    for vert in mesh.data.vertices:
        x, y, z = vert.co
        front = y > -.08
        side = 1 if x >= 0 else -1
        key = ('front' if front else 'hind') + ('_L' if side > 0 else '_R')
        # Above the belly, leg influence smoothly falls away into the torso.
        y_center = .125 if front else -.315
        attachment = (1 - smooth(-.17, -.015, z))
        if z > -.17:
            attachment *= 1 - smooth(.065, .18, abs(y - y_center))
            attachment *= smooth(.02, .085, abs(x))
        upper = smooth(-.315, -.225, z)
        hoof = 1 - smooth(-.445, -.405, z)
        head_weight = smooth(.10, .22, z) * smooth(.12, .24, y)
        values = {'body': (1 - attachment) * (1 - head_weight),
                  'head': (1 - attachment) * head_weight,
                  key + '_upper': attachment * upper,
                  key + '_lower': attachment * (1 - upper) * (1 - hoof),
                  key + '_hoof': attachment * (1 - upper) * hoof}
        for name, weight in values.items():
            if weight > .00001:
                groups[name].add([vert.index], weight, 'REPLACE')
    modifier = mesh.modifiers.new('Goat skeleton', 'ARMATURE')
    modifier.object = rig
    mesh.parent = rig
    scene = bpy.context.scene
    scene.render.fps = 24
    scene.frame_start, scene.frame_end = 1, 25
    # One walk cycle: rear-left, front-left, rear-right, front-right.
    phases = {'hind_L': 0, 'front_L': .25, 'hind_R': .5, 'front_R': .75}
    stance = .64
    stride = .095
    lift = .025
    for frame in range(1, 26):
        t = (frame - 1) / 24
        scene.frame_set(frame)
        bob = -.012 + .002 * math.cos(4 * math.pi * t)
        body_pose = rig.pose.bones['body']
        body_pose.location = (0, 0, bob)
        body_pose.keyframe_insert(data_path='location', frame=frame, group='body')
        bpy.context.view_layer.update()
        head_pose = rig.pose.bones['head']
        head_rest = rig.data.bones['head']
        head_pose.matrix = (Matrix.Translation(head_rest.head_local + Vector((0, 0, bob)))
                            @ Matrix.Rotation(math.radians(1.2) * math.sin(2 * math.pi * t), 4, 'X')
                            @ head_rest.matrix_local.to_3x3().to_4x4())
        head_pose.rotation_mode = 'QUATERNION'
        head_pose.keyframe_insert(data_path='rotation_quaternion', frame=frame, group='head')
        head_pose.keyframe_insert(data_path='location', frame=frame, group='head')
        bpy.context.view_layer.update()
        for name, (rest_hip, knee, ankle) in legs.items():
            hip = rest_hip + Vector((0, 0, bob))
            phase = (t + phases[name]) % 1.0
            if phase < stance:
                travel = stride * (.5 - phase / stance)
                rise = 0.0
            else:
                u = (phase - stance) / (1 - stance)
                travel = stride * (-.5 + smooth(0, 1, u))
                rise = lift * math.sin(math.pi * u) ** 2
            target = ankle + Vector((0, travel, rise))
            # Solve the planar two-segment chain exactly without scale or stretch.
            a, b = (knee - rest_hip).length, (ankle - knee).length
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
                desired = (rest.tail_local - rest.head_local).rotation_difference(end - start)
                bone.matrix = Matrix.Translation(start) @ desired.to_matrix().to_4x4() @ rest.matrix_local.to_3x3().to_4x4()
                bone.rotation_mode = 'QUATERNION'
                bone.keyframe_insert(data_path='rotation_quaternion', frame=frame, group=name)
                bone.keyframe_insert(data_path='location', frame=frame, group=name)
                bpy.context.view_layer.update()
            hoof = rig.pose.bones[name + '_hoof']
            rest = rig.data.bones[hoof.name]
            hoof.matrix = Matrix.Translation(target) @ rest.matrix_local.to_3x3().to_4x4()
            hoof.rotation_mode = 'QUATERNION'
            hoof.keyframe_insert(data_path='rotation_quaternion', frame=frame, group=name)
            hoof.keyframe_insert(data_path='location', frame=frame, group=name)
            bpy.context.view_layer.update()
    rig.animation_data.action.name = 'Goat Walk'
    scene.frame_set(1)
    bpy.ops.wm.save_as_mainfile(filepath=str((args.output / 'goat-rigged.blend').resolve()))
    bpy.ops.export_scene.gltf(filepath=str((args.output / 'goat-rigged.glb').resolve()), export_format='GLB',
                              export_animations=True, export_frame_range=True)


if __name__ == '__main__':
    main()
