"""Bake accepted rigid cloth arrays into named actions of the existing rig.

Run with Blender 4.5: blender -b --python cloth_bake.py -- --help
Each --clip is ACTION=SKINNING_NPZ; input and output must be different files.
"""
import argparse
import json
import sys
from pathlib import Path

import bmesh
import bpy
import numpy as np
from mathutils import Matrix
from mathutils import Vector
from mathutils.bvhtree import BVHTree
from mathutils.geometry import intersect_ray_tri


def polish_contacts(model, fit, playback, output, action, previous=None, local=False):
    """Project fixed-weight bone translations against actual body contacts."""
    if output.exists():
        raise FileExistsError('Choose a fresh correction directory')
    bpy.ops.wm.open_mainfile(filepath=str(model.resolve()))
    with np.load(fit, allow_pickle=False) as source:
        arrays = {name: source[name] for name in source.files if name not in ('transforms', 'reconstructed')}
        weights = source['weights'].astype(float)
        faces = source['triangles'].astype(int)
        target = source['target'].astype(float)
        keys = source['key_transforms'].copy()
    with np.load(playback, allow_pickle=False) as source:
        base = source['poses'].astype(float)
    cyclic = action in ('Walk', 'Run', 'Idle')
    key_count = len(keys)-1 if cyclic else len(keys)
    if (base.ndim != 3 or base.shape[1:] != (len(weights), 3) or len(keys) < 2
            or len(base) < len(keys) or (len(base)-1) % (len(keys)-1)
            or not np.isfinite(base).all() or not np.isfinite(keys).all()
            or not np.isfinite(weights).all() or np.min(weights) < 0
            or np.max(np.abs(weights.sum(axis=1)-1)) > 1e-5
            or np.max(np.count_nonzero(weights, axis=1)) > 4):
        raise ValueError('Invalid contact fit or actual playback arrays')
    key_step = (len(base)-1)//(len(keys)-1)
    delta = np.zeros((key_count, weights.shape[1], 3))
    if previous is not None:
        with np.load(previous, allow_pickle=False) as source:
            delta = source['key_transforms'][:key_count, :, :3, 3]-keys[:key_count, :, :3, 3]
        if np.max(np.linalg.norm(delta, axis=2)) > .020001:
            raise ValueError('Previous correction exceeds the translation cap')
    scene = bpy.context.scene
    rig = bpy.data.objects['VillagerRig']
    rig.animation_data.action = bpy.data.actions[action]
    scene.render.fps = 120
    human = bpy.data.objects['Human']
    human.modifiers['Delete.donitz_monk_robe'].show_viewport = False
    names = ('Separate olive trousers', 'Brown shoe L', 'Brown shoe R') if action == 'Run' else ('Separate olive trousers', 'Human')
    objects = [bpy.data.objects[name] for name in names]

    def bary(point, a, b, c):
        v0, v1, v2 = b-a, c-a, point-a
        d00, d01, d11, d20, d21 = v0.dot(v0), v0.dot(v1), v1.dot(v1), v2.dot(v0), v2.dot(v1)
        denominator = d00*d11-d01*d01
        if abs(denominator) < 1e-20:
            return None
        v, w = (d11*d20-d01*d21)/denominator, (d00*d21-d01*d20)/denominator
        result = np.clip(np.array([1-v-w, v, w]), 0, 1)
        return result/result.sum() if result.sum() > 0 else None

    def intersection(a, b):
        for triangle, other in ((a, b), (b, a)):
            for index in range(3):
                origin = triangle[index]
                direction = triangle[(index+1)%3]-origin
                if direction.length_squared < 1e-20:
                    continue
                point = intersect_ray_tri(other[0], other[1], other[2], direction, origin, True)
                if point is not None:
                    t = (point-origin).dot(direction)/direction.length_squared
                    if -1e-5 <= t <= 1.00001:
                        return point
        return None

    def gather():
        rows, counts, frame_counts, self_count = [], {o.name: 0 for o in objects}, {o.name: 0 for o in objects}, 0
        for frame in range(len(base)-(1 if cyclic else 0)):
            scene.frame_set(frame+1)
            bpy.context.view_layer.update()
            left, alpha = frame//key_step, (frame%key_step)/key_step
            right = (left+1)%key_count if cyclic else min(left+1, key_count-1)
            movement = weights@((1-alpha)*delta[left]+alpha*delta[right])
            vertices = [Vector(v) for v in (base[frame]+movement)]
            cloth = BVHTree.FromPolygons(vertices, faces.tolist(), all_triangles=True)
            deps = bpy.context.evaluated_depsgraph_get()
            pairs = {(i, j) for i, j in cloth.overlap(cloth)
                     if i < j and set(faces[i]).isdisjoint(faces[j])}
            self_count += len(pairs)
            if pairs and action in ('Walk', 'Idle', 'Hit'):
                if len(target) == len(base):
                    reference = target[frame]
                else:
                    phase, fraction = frame//4, (frame%4)/4
                    reference = target[phase]*(1-fraction)+target[phase+1]*fraction
                for ci, bi in pairs:
                    ca, cb = [vertices[i] for i in faces[ci]], [vertices[i] for i in faces[bi]]
                    point = intersection(ca, cb)
                    if point is None:
                        continue
                    if local and not (-.16 < point.x < -.05 and point.y > .125 and .40 < point.z < .55):
                        continue
                    bc, bb = bary(point, *ca), bary(point, *cb)
                    if bc is None or bb is None:
                        continue
                    ra, rb = reference[faces[ci]], reference[faces[bi]]
                    normal = np.cross(rb[1]-rb[0], rb[2]-rb[0])
                    length = np.linalg.norm(normal)
                    if length < 1e-12:
                        continue
                    normal /= length
                    if np.dot(ra.mean(axis=0)-rb.mean(axis=0), normal) < 0:
                        normal = -normal
                    w = bc@weights[faces[ci]]-bb@weights[faces[bi]]
                    ids = np.flatnonzero(np.abs(w) > 1e-10)
                    if len(ids):
                        rows.append((left, right, alpha, ids, w[ids], normal, .002))
            for obj in objects:
                evaluated = obj.evaluated_get(deps)
                mesh = evaluated.to_mesh()
                mesh.calc_loop_triangles()
                points = [obj.matrix_world@v.co for v in mesh.vertices]
                triangles = [tuple(t.vertices) for t in mesh.loop_triangles]
                collider = BVHTree.FromPolygons(points, triangles, all_triangles=True)
                overlaps = cloth.overlap(collider)
                counts[obj.name] += len(overlaps)
                frame_counts[obj.name] += bool(overlaps)
                for ci, bi in overlaps:
                    ca, cb = [vertices[i] for i in faces[ci]], [points[i] for i in triangles[bi]]
                    normal = (cb[1]-cb[0]).cross(cb[2]-cb[0])
                    if normal.length < 1e-12:
                        continue
                    normal.normalize()
                    point = intersection(ca, cb)
                    if point is None:
                        continue
                    coeff = bary(point, *ca)
                    if coeff is None:
                        continue
                    w = coeff@weights[faces[ci]]
                    ids = np.flatnonzero(w > 1e-10)
                    rows.append((left, right, alpha, ids, w[ids], np.array(normal[:]), .003))
                    for vertex in faces[ci]:
                        hit, n, _, _ = collider.find_nearest(vertices[vertex], .025)
                        if hit is None:
                            continue
                        signed = (vertices[vertex]-hit).dot(n)
                        if signed < .003:
                            w = weights[vertex]
                            ids = np.flatnonzero(w > 1e-10)
                            rows.append((left, right, alpha, ids, w[ids], np.array(n[:]), min(.015, .003-signed)))
                evaluated.to_mesh_clear()
        return rows, counts, frame_counts, self_count

    history = []
    for iteration in range(4 if action in ('Run', 'Idle', 'Hit') else 12):
        rows, counts, frames, self_count = gather()
        anchor = delta.copy()
        base_count = sum(counts.values())
        entry = {'iteration': iteration, 'rows': len(rows), 'pairs': counts, 'frames': frames, 'self_pairs': self_count}
        history.append(entry)
        if not rows:
            break
        for _ in range(4):
            for left, right, alpha, ids, w, normal, minimum in rows:
                current = (1-alpha)*delta[left, ids]+alpha*delta[right, ids]
                old = (1-alpha)*anchor[left, ids]+alpha*anchor[right, ids]
                gap = minimum-np.sum(w[:, None]*(current-old)*normal)
                if gap > 0:
                    denominator = ((1-alpha)**2+alpha**2)*np.dot(w, w)+1e-10
                    step = min(.005, .15*gap/denominator)
                    delta[left, ids] += (1-alpha)*step*w[:, None]*normal
                    delta[right, ids] += alpha*step*w[:, None]*normal
            length = np.linalg.norm(delta, axis=2)
            delta *= np.minimum(1, .02/np.maximum(length, 1e-20))[:, :, None]
        if cyclic:
            candidate = .9*delta+.05*np.roll(delta, 1, axis=0)+.05*np.roll(delta, -1, axis=0)
        else:
            candidate = .9*delta+.05*np.concatenate((delta[:1], delta[:-1]))+.05*np.concatenate((delta[1:], delta[-1:]))
        accepted = False
        for fraction in (1., .5, .25, .125):
            delta = anchor+fraction*(candidate-anchor)
            _, next_counts, next_frames, next_self = gather()
            entry.setdefault('line_search', []).append({'fraction': fraction, 'pairs': next_counts,
                                                        'frames': next_frames, 'self_pairs': next_self})
            if (sum(next_counts.values()) <= base_count and next_self <= self_count
                    and sum(next_counts.values())+next_self < base_count+self_count):
                accepted = True
                break
        if not accepted:
            delta = anchor
            entry['stalled'] = True
            break
    keys[:key_count, :, :3, 3] += delta
    if cyclic:
        keys[key_count] = keys[0]
    arrays['key_transforms'] = keys
    output.mkdir(parents=True, exist_ok=False)
    np.savez_compressed(output/'skinning.npz', **arrays)
    report = {'schema_version': 1, 'kind': 'contact_correction', 'action': action, 'history': history,
              'max_bone_translation_m': float(np.linalg.norm(delta, axis=2).max()),
              'rms_bone_translation_m': float(np.sqrt(np.mean(np.sum(delta**2, axis=2)))),
              'cyclic': cyclic, 'local': local}
    (output/'correction-report.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps(report), flush=True)


def bake(model, output, clips, capture=None):
    if output.exists() or output.resolve() == model.resolve():
        raise FileExistsError('Choose a fresh output file distinct from the input')
    if capture is not None and (capture.exists() or capture.resolve() in {model.resolve(), output.resolve()} or len(clips) != 1):
        raise ValueError('--capture needs one clip and a fresh output path')
    bpy.ops.wm.open_mainfile(filepath=str(model.resolve()))
    scene = bpy.context.scene
    rig = bpy.data.objects['VillagerRig']
    drivers = len(rig.animation_data.drivers)
    if (drivers != 162 or not np.allclose(np.array(rig.matrix_world), np.eye(4), atol=1e-8)
            or any(not track.mute for track in rig.animation_data.nla_tracks)):
        raise ValueError('Unexpected rig drivers or world transform')
    scene.render.fps = 120
    reports = []
    for action_name, path in clips:
        with np.load(path, allow_pickle=False) as data:
            rest, faces, weights = data['rest'].copy(), data['triangles'].copy(), data['weights'].copy()
            keys, frames, bind = data['key_transforms'].copy(), data['key_source_frames'].copy(), data['bind_matrices'].copy()
        count = len(bind)
        if (rest.ndim != 2 or rest.shape[1] != 3 or not np.isfinite(rest).all()
                or faces.ndim != 2 or faces.shape[1] != 3 or faces.size == 0
                or faces.min() < 0 or faces.max() >= len(rest)
                or weights.shape != (len(rest), count) or count == 0 or not np.isfinite(bind).all()
                or keys.shape != (len(frames), count, 4, 4) or not np.isfinite(keys).all()
                or not np.allclose(keys[:, :, 3, :], [0, 0, 0, 1], atol=1e-8)
                or not np.isfinite(weights).all() or weights.min() < 0
                or np.max(np.abs(weights.sum(axis=1)-1)) > 1e-5
                or np.max(np.count_nonzero(weights, axis=1)) > 4
                or len(frames) < 2 or frames[0] != 1 or np.any(np.diff(frames) <= 0)):
            raise ValueError('Invalid shared cloth arrays: ' + str(path))
        rotation = keys[:, :, :3, :3]
        if (np.max(np.abs(np.swapaxes(rotation, -1, -2) @ rotation - np.eye(3))) > 1e-5
                or np.max(np.abs(np.linalg.det(rotation)-1)) > 1e-5):
            raise ValueError('Cloth keys contain scale, shear, or reflections')
        rig.animation_data.action = bpy.data.actions[action_name]
        names = [f'CLOTH.{i:03d}' for i in range(count)]
        present_names = {bone.name for bone in rig.data.bones if bone.name.startswith('CLOTH.')}
        if present_names and present_names != set(names):
            raise ValueError('Cloth bone identities differ')
        existing = [n in rig.data.bones for n in names]
        if any(existing) and not all(existing):
            raise ValueError('Partial cloth bone set')
        if all(existing):
            skirt = bpy.data.objects['Walk skeletal lower cloth']
            mesh = skirt.data
            actual = np.zeros_like(weights)
            for vertex in mesh.vertices:
                for influence in vertex.groups:
                    name = skirt.vertex_groups[influence.group].name
                    if name in names:
                        actual[vertex.index, names.index(name)] = influence.weight
            coordinates = np.empty(len(mesh.vertices)*3, dtype=np.float32)
            mesh.vertices.foreach_get('co', coordinates)
            if (len(mesh.vertices) != len(rest) or np.max(np.abs(actual-weights)) > 1e-6
                    or np.max(np.abs(coordinates.reshape(-1, 3)-rest)) > 1e-6
                    or [tuple(p.vertices) for p in mesh.polygons] != [tuple(f) for f in faces]):
                raise ValueError('Shared cloth rest or weights differ')
        else:
            robe = bpy.data.objects['Human.donitz_monk_robe']
            bm = bmesh.new()
            bm.from_mesh(robe.data)
            bmesh.ops.bisect_plane(bm, geom=list(bm.verts)+list(bm.edges)+list(bm.faces),
                                   dist=1e-6, plane_co=(0, 0, .83), plane_no=(0, 0, 1),
                                   clear_outer=False, clear_inner=True)
            bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context='VERTS')
            bm.to_mesh(robe.data)
            bm.free()
            robe.data.update()
            mesh = bpy.data.meshes.new('Walk skeletal lower cloth')
            mesh.from_pydata(rest.tolist(), [], faces.tolist())
            mesh.update()
            skirt = bpy.data.objects.new('Walk skeletal lower cloth', mesh)
            scene.collection.objects.link(skirt)
            mesh.materials.append(robe.data.materials[0])
            for poly in mesh.polygons:
                poly.use_smooth = True
            bpy.ops.object.select_all(action='DESELECT')
            rig.select_set(True)
            bpy.context.view_layer.objects.active = rig
            bpy.ops.object.mode_set(mode='EDIT')
            for name, matrix in zip(names, bind):
                bone = rig.data.edit_bones.new(name)
                bone.head = (0, 0, 0)
                bone.tail = (0, .08, 0)
                bone.matrix = Matrix(matrix.tolist())
                bone.use_deform = True
            bpy.ops.object.mode_set(mode='OBJECT')
            for index, name in enumerate(names):
                group = skirt.vertex_groups.new(name=name)
                for vertex in np.flatnonzero(weights[:, index]):
                    group.add([int(vertex)], float(weights[vertex, index]), 'REPLACE')
            modifier = skirt.modifiers.new('Skeletal cloth', 'ARMATURE')
            modifier.object = rig
            modifier.use_deform_preserve_volume = False
        bind_error = max(float(np.max(np.abs(np.array(rig.data.bones[n].matrix_local)-m)))
                         for n, m in zip(names, bind))
        if bind_error > 1e-6:
            raise ValueError('Cloth bind matrices differ from solver convention')
        previous, first = {}, {}
        for index, frame in enumerate(frames):
            frame = int(frame)
            scene.frame_set(frame)
            for bone_index, name in enumerate(names):
                bone = rig.pose.bones[name]
                bone.rotation_mode = 'QUATERNION'
                bone.matrix = Matrix(keys[index, bone_index].tolist()) @ rig.data.bones[name].matrix_local
                q = bone.rotation_quaternion.copy()
                if name in previous and previous[name].dot(q) < 0:
                    q.negate()
                bone.rotation_quaternion = q
                previous[name] = q.copy()
                if name not in first:
                    first[name] = q.copy()
                for property_name in ('location', 'rotation_quaternion', 'scale'):
                    bone.keyframe_insert(data_path=property_name, frame=frame, group=name)
        winding = [n for n in names if action_name in ('Idle', 'Walk', 'Run')
                   and first[n].dot(previous[n]) < 0]
        if winding:
            raise ValueError('Looping cloth quaternion winding: ' + ', '.join(winding))
        for layer in rig.animation_data.action.layers:
            for strip in layer.strips:
                for bag in strip.channelbags:
                    for curve in bag.fcurves:
                        if curve.data_path.startswith('pose.bones["CLOTH.'):
                            for key in curve.keyframe_points:
                                key.interpolation = 'LINEAR'
        reports.append({'action': action_name, 'keys': len(frames), 'end_frame': int(frames[-1]),
                        'bones': count, 'bind_max_axis_error_m': bind_error})
    if len(rig.animation_data.drivers) != drivers:
        raise RuntimeError('Rigify drivers changed')
    scene.frame_set(1)
    bpy.context.view_layer.update()
    output.parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.wm.save_as_mainfile(filepath=str(output.resolve()))
    if capture is not None:
        poses = []
        for frame in range(1, int(frames[-1])+1):
            scene.frame_set(frame)
            bpy.context.view_layer.update()
            evaluated = skirt.evaluated_get(bpy.context.evaluated_depsgraph_get())
            mesh = evaluated.to_mesh()
            coordinates = np.empty(len(mesh.vertices)*3, dtype=np.float32)
            mesh.vertices.foreach_get('co', coordinates)
            poses.append(coordinates.reshape(-1, 3).copy())
            evaluated.to_mesh_clear()
        capture.parent.mkdir(parents=True, exist_ok=True)
        with capture.open('xb') as stream:
            np.savez_compressed(stream, poses=np.array(poses), triangles=faces,
                                times=np.arange(len(poses))/120)
    print(json.dumps({'schema_version': 1, 'kind': 'cloth_bake', 'output': str(output),
                      'drivers': drivers, 'fps': 120, 'clips': reports}), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--model', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--clip', action='append', metavar='ACTION=NPZ')
    parser.add_argument('--capture', type=Path, help='Capture actual Blender playback for a single clip')
    parser.add_argument('--contact-fit', type=Path, help='Correct this fit using actual contact geometry')
    parser.add_argument('--playback', type=Path, help='Actual sampled Blender positions for contact correction')
    parser.add_argument('--action', choices=('Walk', 'Run', 'Idle', 'Hit'))
    parser.add_argument('--previous', type=Path, help='Prior correction from the same unmodified fit')
    parser.add_argument('--local', action='store_true', help='Restrict standing self correction to the measured rear notch')
    args = parser.parse_args(sys.argv[sys.argv.index('--')+1:])
    if args.contact_fit is not None:
        if args.playback is None or args.action is None or args.clip or args.capture:
            parser.error('--contact-fit requires --playback and --action, without --clip/--capture')
        if args.local and args.action not in ('Idle', 'Hit'):
            parser.error('--local is only valid for the standing actions')
        polish_contacts(args.model, args.contact_fit, args.playback, args.output,
                        args.action, args.previous, args.local)
        return
    if not args.clip or args.playback or args.action or args.previous or args.local:
        parser.error('Bake requires --clip; contact options require --contact-fit')
    clips = []
    for item in args.clip:
        name, separator, path = item.partition('=')
        if not separator or name not in ('Idle', 'Walk', 'Run', 'Hit', 'Death') or not path:
            parser.error('--clip must be one of Idle, Walk, Run, Hit, Death followed by =NPZ')
        clips.append((name, Path(path)))
    if len({name for name, _ in clips}) != len(clips):
        parser.error('Do not repeat an action')
    bake(args.model, args.output, clips, args.capture)


if __name__ == '__main__':
    try:
        main()
    except Exception as exc:
        print(json.dumps({'schema_version': 1, 'kind': 'error',
                          'error_type': type(exc).__name__, 'message': str(exc)[:500]}), file=sys.stderr)
        sys.exit(1)
