"""Export a prepared character scene for Unity without studio helpers.

The scene should contain only the character, its rig and its props.  Blender's
FBX exporter converts the scene from Z-up to Unity's Y-up convention.  Embedded
textures keep the handoff portable. With --animations the full scene timeline
is exported as one take; the consuming engine can split it using the manifest.
"""
import argparse
import json
import math
import struct
import sys
from pathlib import Path

import bpy
from mathutils import Matrix
from mathutils.kdtree import KDTree


def world_vertices(obj):
    evaluated = obj.evaluated_get(bpy.context.evaluated_depsgraph_get())
    mesh = evaluated.to_mesh()
    try:
        return [evaluated.matrix_world @ vertex.co for vertex in mesh.vertices]
    finally:
        evaluated.to_mesh_clear()


def bake_deform_rig(rig_name, mesh_names, action_names):
    """Freeze the rest shape and bake evaluated authoring controls to LBS bones.

    This deliberately rejects unsupported deformation instead of silently
    exporting a different body. The source rig and separate editable objects
    remain in memory and are not saved over their input file.
    """
    scene = bpy.context.scene
    source = bpy.data.objects[rig_name]
    if source.type != 'ARMATURE' or not action_names or not mesh_names:
        raise ValueError('Deform baking requires a rig, explicit meshes and named actions')
    if source.animation_data is None or any(not track.mute and len(track.strips) for track in source.animation_data.nla_tracks):
        raise ValueError('Named-action export requires animation data and muted authoring NLA tracks')
    meshes = [bpy.data.objects[name] for name in mesh_names]
    if any(obj.type != 'MESH' for obj in meshes):
        raise ValueError('Every explicit skin must be a mesh')
    source.animation_data.action = None
    source.data.pose_position = 'REST'
    bpy.context.view_layer.update()
    deps = bpy.context.evaluated_depsgraph_get()
    # Check the geometry that actually reaches the game. MakeHuman keeps
    # unweighted fitting helpers in the editable base, hidden by Mask modifiers.
    rest_meshes = {obj.name:bpy.data.meshes.new_from_object(obj.evaluated_get(deps),
                   preserve_all_data_layers=True, depsgraph=deps) for obj in meshes}
    deform_names = {bone.name for bone in source.data.bones if bone.use_deform}
    names = {obj.vertex_groups[group.group].name for obj in meshes for vertex in rest_meshes[obj.name].vertices
             for group in vertex.groups if group.weight > 0 and obj.vertex_groups[group.group].name in deform_names}
    bones = [bone for bone in source.data.bones if bone.name in names]
    for bone in bones:
        if bone.bbone_segments != 1:
            raise ValueError('Bake source still uses unsupported segmented B-bone: ' + bone.name)
    for obj in meshes:
        modifiers = [m for m in obj.modifiers if m.show_viewport and m.type == 'ARMATURE']
        if (obj.type != 'MESH' or len(modifiers) != 1 or modifiers[0].object != source or
                modifiers[0].use_deform_preserve_volume or modifiers[0].use_multi_modifier):
            raise ValueError('Bake source must use exactly one ordinary LBS armature: ' + obj.name)
        if any(m.show_viewport and m.type in {'CLOTH', 'CORRECTIVE_SMOOTH'} for m in obj.modifiers):
            raise ValueError('Bake source retains unsupported animated modifiers: ' + obj.name)
        for vertex in rest_meshes[obj.name].vertices:
            weights = [g.weight for g in vertex.groups if obj.vertex_groups[g.group].name in names and g.weight > 0]
            if not weights or len(weights) > 4 or abs(sum(weights) - 1) > .0001:
                raise ValueError('Bake source needs normalized one-to-four deform weights: ' + obj.name)

    data = bpy.data.armatures.new('Game deform skeleton')
    target = bpy.data.objects.new('GameRig', data)
    scene.collection.objects.link(target)
    target.matrix_world = source.matrix_world.copy()
    bpy.ops.object.select_all(action='DESELECT')
    target.select_set(True)
    bpy.context.view_layer.objects.active = target
    bpy.ops.object.mode_set(mode='EDIT')
    for bone in bones:
        edit = data.edit_bones.new(bone.name)
        # A new edit bone has zero length; setting its matrix first cannot
        # establish the source axis/roll. Give it length before orientation.
        edit.length = bone.length
        edit.matrix = bone.matrix_local.copy()
    for bone in bones:
        parent = bone.parent
        while parent is not None and parent.name not in names:
            parent = parent.parent
        if parent is not None:
            data.edit_bones[bone.name].parent = data.edit_bones[parent.name]
    bpy.ops.object.mode_set(mode='OBJECT')
    exported = []
    for obj in meshes:
        # Evaluated REST geometry includes the active anatomical morph mix and
        # static garment finish; its deform-weight custom data is retained.
        mesh = rest_meshes[obj.name]
        copy = obj.copy()
        copy.name = 'Game ' + obj.name
        copy.data = mesh
        copy.modifiers.clear()
        scene.collection.objects.link(copy)
        world = obj.matrix_world.copy()
        copy.parent = target
        copy.matrix_world = world
        armature = copy.modifiers.new('Game LBS', 'ARMATURE')
        armature.object = target
        exported.append(copy)
    bpy.context.view_layer.update()
    bind_proof = []
    for original, copied in zip(meshes, exported):
        before, after = world_vertices(original), world_vertices(copied)
        if len(before) != len(after):
            raise ValueError('Rest baking changed vertex count: ' + original.name)
        bind_proof.append({'mesh':original.name, 'maxError':max((a-b).length for a,b in zip(before,after))})
    rest_matrix_error = max(abs(source.data.bones[name].matrix_local[row][col]-target.data.bones[name].matrix_local[row][col])
                            for name in names for row in range(4) for col in range(4))
    # Short float32 bones can differ by a few microradians after reconstruction;
    # a mismatched inverse bind must never proceed to an apparently valid FBX.
    if rest_matrix_error > .0001:
        raise ValueError(f'Compact skeleton changed its inverse bind: {rest_matrix_error}')
    source.data.pose_position = 'POSE'
    target.animation_data_create()
    timeline = bpy.data.actions.new('GameTimeline')
    timeline.use_fake_user = True
    target.animation_data.action = timeline
    cursor, clips, proof, expected_samples = 1, [], [], {}
    # Bone order is irrelevant when matrix_basis is derived from the recorded
    # parent pose, rather than depending on an unevaluated previous assignment.
    identity = Matrix.Identity(4)
    for action_name in action_names:
        action = bpy.data.actions[action_name]
        source.animation_data.action = action
        source.animation_data.action_slot = action.slots[0]
        start, end = action.frame_range
        if abs(start-round(start)) > .0001 or abs(end-round(end)) > .0001:
            raise ValueError('Source action range must be integral at its authored FPS: ' + action_name)
        first, last = cursor, cursor + round(end-start)
        sample_frames = {round(start+(end-start)*fraction) for fraction in (0, .25, .5, .75, 1)}
        for frame in range(round(start), round(end)+1):
            scene.frame_set(frame)
            bpy.context.view_layer.update()
            evaluated = source.evaluated_get(bpy.context.evaluated_depsgraph_get())
            poses = {name: evaluated.pose.bones[name].matrix.copy() for name in names}
            expected = [world_vertices(obj) for obj in meshes] if frame in sample_frames else None
            destination_frame = first + frame-round(start)
            for pose in target.pose.bones:
                parent = pose.parent
                basis = pose.bone.convert_local_to_pose(
                    poses[pose.name], pose.bone.matrix_local,
                    parent_matrix=poses[parent.name] if parent else identity,
                    parent_matrix_local=parent.bone.matrix_local if parent else identity,
                    invert=True)
                pose.rotation_mode = 'QUATERNION'
                pose.matrix_basis = basis
                for prop in ('location', 'rotation_quaternion', 'scale'):
                    pose.keyframe_insert(data_path=prop, frame=destination_frame, group=pose.name)
            if expected is not None:
                # Evaluate the compact rig independently through its baked
                # timeline before comparing actual posed mesh surfaces.
                scene.frame_set(destination_frame)
                bpy.context.view_layer.update()
                for original, copied, reference in zip(meshes, exported, expected):
                    actual = world_vertices(copied)
                    expected_samples.setdefault(destination_frame, []).extend(actual)
                    if len(reference) != len(actual):
                        raise ValueError('Baking changed evaluated vertex count: ' + original.name)
                    errors = sorted((a-b).length for a,b in zip(actual,reference))
                    if not errors or not all(math.isfinite(error) for error in errors):
                        raise ValueError('Baking produced invalid surface geometry: ' + original.name)
                    proof.append({'clip':action_name, 'sourceFrame':frame, 'frame':destination_frame,
                                  'mesh':original.name, 'vertices':len(actual), 'maxError':errors[-1],
                                  'p95Error':errors[int(.95*(len(errors)-1))],
                                  'maxBoneMatrixError':max(abs(poses[name][row][col]-target.pose.bones[name].matrix[row][col])
                                                           for name in names for row in range(4) for col in range(4))})
        clips.append({'name':action_name, 'firstFrame':first, 'lastFrame':last,
                      'loop':action_name in {'Idle','Walk','Run','Sprint'}})
        cursor = last + 1
    for layer in timeline.layers:
        for strip in layer.strips:
            for bag in strip.channelbags:
                for curve in bag.fcurves:
                    for point in curve.keyframe_points:
                        point.interpolation = 'LINEAR'
    scene.frame_start, scene.frame_end = 1, cursor-1
    scene.frame_set(1)
    return [target, *exported], {'schema':'mc-game-skin-export/v1', 'fps':scene.render.fps/scene.render.fps_base, 'clips':clips,
                                 'deformBones':len(bones), 'bindSurfaceProof':bind_proof,
                                 'maxRestBoneMatrixError':rest_matrix_error, 'surfaceProof':proof,
                                 '_expectedSamples':expected_samples}


def verify_reimport(path, expected_samples):
    """Check real FBX consumer geometry at the recorded game-rig poses.

    Joining/export may reorder vertices. Bidirectional nearest-point comparison
    checks the sampled world-space surface without assuming source indices.
    """
    bpy.ops.wm.read_factory_settings(use_empty=True)
    # A scene timeline exports absolute frame times. Blender's default +1
    # import offset would compare every pose against its previous frame.
    bpy.ops.import_scene.fbx(filepath=str(path.resolve()), anim_offset=0)
    scene = bpy.context.scene
    # FBX has ordinary parented transforms, as does Unity. Blender's importer
    # invents connected-joint flags for coincident endpoints (import_fbx.py,
    # connect_child), suppressing valid animated local translations. The
    # compact source skeleton above intentionally has no connected bones.
    disconnected = []
    for armature in [obj for obj in scene.objects if obj.type == 'ARMATURE']:
        bpy.ops.object.select_all(action='DESELECT')
        armature.select_set(True)
        bpy.context.view_layer.objects.active = armature
        bpy.ops.object.mode_set(mode='EDIT')
        for bone in armature.data.edit_bones:
            if bone.use_connect:
                disconnected.append(bone.name)
                bone.use_connect = False
        bpy.ops.object.mode_set(mode='OBJECT')
    meshes = [obj for obj in scene.objects if obj.type == 'MESH']
    proof = []
    for frame, expected in expected_samples.items():
        scene.frame_set(frame)
        bpy.context.view_layer.update()
        actual = [point for obj in meshes for point in world_vertices(obj)]
        if len(actual) != len(expected):
            raise ValueError(f'FBX reimport changed vertex count at frame {frame}: {len(expected)} -> {len(actual)}')
        errors = []
        for source, destination in ((actual, expected), (expected, actual)):
            tree = KDTree(len(destination))
            for i, point in enumerate(destination):
                tree.insert(point, i)
            tree.balance()
            errors.extend(tree.find(point)[2] for point in source)
        errors.sort()
        if not errors or not all(math.isfinite(error) for error in errors):
            raise ValueError('FBX reimport has invalid sampled geometry')
        proof.append({'frame':frame, 'vertices':len(actual), 'maxError':errors[-1],
                      'p95Error':errors[int(.95*(len(errors)-1))]})
    return proof, disconnected


def write_surface_samples(path, samples, fps):
    """Little-endian MCSK v1 pose samples for an independent engine consumer."""
    with path.open('xb') as output:
        output.write(struct.pack('<4sIId', b'MCSK', 1, len(samples), fps))
        for frame, points in samples.items():
            output.write(struct.pack('<II', frame, len(points)))
            for point in points:
                output.write(struct.pack('<fff', *point))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--animations', action='store_true')
    parser.add_argument('--bake-rig', help='Bake this authoring armature into a compact ordinary-LBS game rig')
    parser.add_argument('--mesh', action='append', default=[], help='Explicit skinned mesh to bake; repeat for body, robe and props')
    parser.add_argument('--action', action='append', default=[], help='Named action to concatenate; repeat in desired clip order')
    parser.add_argument('--join-skins', action='store_true', help='Join disconnected skin components for one runtime renderer')
    args = parser.parse_args(sys.argv[sys.argv.index('--') + 1:])
    args.output.parent.mkdir(parents=True, exist_ok=True)
    report = None
    if args.bake_rig:
        if not args.animations:
            raise ValueError('--bake-rig requires --animations')
        if any(path.exists() for path in (args.output, args.output.with_suffix('.export.json'), args.output.with_suffix('.samples.bin'))):
            raise FileExistsError('Choose a fresh candidate export path')
        objects, report = bake_deform_rig(args.bake_rig, args.mesh, args.action)
    else:
        if args.mesh or args.action or args.join_skins:
            raise ValueError('Explicit skin/action options require --bake-rig')
        objects = [obj for obj in bpy.context.scene.objects if obj.type in {'ARMATURE', 'MESH', 'CURVE', 'EMPTY'}]
    if args.join_skins:
        skins = [obj for obj in objects if obj.type == 'MESH']
        bpy.ops.object.select_all(action='DESELECT')
        for obj in skins:
            obj.select_set(True)
        bpy.context.view_layer.objects.active = skins[0]
        if len(skins) > 1:
            bpy.ops.object.join()
        objects = [obj for obj in objects if obj.type == 'ARMATURE'] + [bpy.context.object]
    bpy.ops.object.select_all(action='DESELECT')
    for obj in objects:
        obj.select_set(True)
    bpy.ops.export_scene.fbx(
        filepath=str(args.output.resolve()), use_selection=True,
        object_types={'ARMATURE', 'MESH', 'EMPTY', 'OTHER'},
        axis_forward='-Z', axis_up='Y', apply_unit_scale=True,
        add_leaf_bones=False, path_mode='COPY', embed_textures=True,
        bake_anim=args.animations, bake_anim_use_nla_strips=False,
        bake_anim_use_all_actions=False, bake_anim_simplify_factor=0.0,
        use_mesh_modifiers=False,
    )
    if report is not None:
        samples = report.pop('_expectedSamples')
        sample_path = args.output.with_suffix('.samples.bin')
        write_surface_samples(sample_path, samples, report['fps'])
        report['surfaceSamples'] = sample_path.name
        report['reimportSurfaceProof'], report['reimportDisconnectedBones'] = verify_reimport(args.output, samples)
        args.output.with_suffix('.export.json').write_text(json.dumps(report, indent=2)+'\n')
        if max(sample['maxError'] for sample in report['reimportSurfaceProof']) > .0001:
            raise ValueError('FBX reimport changed a sampled surface by more than 0.1 mm; see export report')
    print('FBX_OK', args.output, flush=True)


if __name__ == '__main__':
    main()
