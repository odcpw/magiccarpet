"""Build a periodic lower-cloth Walk target and fit it to rigid skin bones.

Inputs are actual monotonically sampled simulation vertices in metres. The
periodic target is explicitly authored; it is not a claim that physics looped.
"""
import argparse
import json
import sys
from pathlib import Path

import numpy as np


def extract_finite(cache_path, fixed_path, output, clip):
    """Extract the mapped simulation action and its real Death settle hold."""
    with np.load(fixed_path, allow_pickle=False) as data:
        faces = data['triangles'].copy()
        vertices = len(data['rest'])
    with np.load(cache_path, allow_pickle=False) as data:
        start, end, fps = int(data['action_start_frame']), int(data['action_end_frame']), int(data['fps'])
        stop = len(data['poses']) if clip == 'Death' else end
        poses = data['poses'][start-1:stop].astype(np.float64)
        source_frames = data['source_frames'][start-1:stop].copy()
        actions = data['source_actions'][start-1:stop]
        phases = data['phases'][start-1:stop].copy()
        times = data['times'][start-1:stop]
        triangles = data['triangles']
    action_frames = end-start+1
    expected = np.concatenate((np.arange(1, action_frames+1),
                               np.full(len(poses)-action_frames, action_frames)))
    if (fps != 120 or poses.shape[1:] != (vertices, 3) or not np.isfinite(poses).all()
            or not np.array_equal(triangles, faces) or not np.array_equal(source_frames, expected)
            or not np.all(actions == clip) or not np.allclose(np.diff(times), 1/120)
            or not np.all(phases[:action_frames] == 'action')
            or not np.all(phases[action_frames:] == 'final_hold')):
        raise ValueError('Finite simulation mapping, topology or time differs')
    raw = poses.copy()
    if clip == 'Idle':
        delta = raw[0]-raw[-1]
        phase = np.arange(len(raw))
        left = np.clip(phase/16, 0, 1)
        right = np.clip((phase-(len(raw)-17))/16, 0, 1)
        left = 3*left**2-2*left**3
        right = 3*right**2-2*right**3
        poses += (-.5*(1-left)+.5*right)[:, None, None]*delta
        poses[-1] = poses[0]
    output.mkdir(parents=True, exist_ok=False)
    np.savez_compressed(output/(clip+'.npz'), poses=poses, triangles=faces,
                        times=np.arange(len(poses))/120, source_frames=source_frames,
                        phases=phases, raw_source=raw)
    adjustment = np.linalg.norm(poses-raw, axis=2)
    report = {'schema_version': 1, 'kind': 'finite_target', 'clip': clip,
              'frames': len(poses), 'action_frames': action_frames, 'fps': fps,
              'adjustment_rms_m': float(np.sqrt(np.mean(adjustment**2))),
              'adjustment_max_m': float(adjustment.max())}
    (output/'target-report.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps(report), flush=True)


def polish_translation(fit_path, playback_path, output, mode):
    """Apply the measured Death floor or local exposed-leg correction."""
    with np.load(fit_path, allow_pickle=False) as data:
        arrays = {k: data[k] for k in data.files}
        weights = data['weights'].astype(np.float64)
        keys = data['key_transforms'].copy()
        target = data['target'].astype(np.float64)
    with np.load(playback_path, allow_pickle=False) as data:
        base = data['poses'].astype(np.float64)
    if (len(base) != len(keys) or base.shape != target.shape
            or weights.shape[0] != base.shape[1] or not np.isfinite(base).all()):
        raise ValueError('Expected actual playback at every finite source key')
    delta = np.zeros((len(base), weights.shape[1]))
    active = []
    for frame, points in enumerate(target):
        if mode == 'floor':
            active.append(np.arange(len(points)))
        else:
            mask = ((frame >= 98) & (frame <= 150) & (points[:, 0] > -.22)
                    & (points[:, 0] < -.12) & (points[:, 1] > .47)
                    & (points[:, 1] < .64) & (points[:, 2] > .20) & (points[:, 2] < .45))
            active.append(np.flatnonzero(mask & (points[:, 2]-base[frame, :, 2] > .001)))
    def project(frame):
        ids = active[frame]
        if not len(ids):
            return
        w = weights[ids]
        needed = (.0005-base[frame, ids, 2] if mode == 'floor'
                  else target[frame, ids, 2]-base[frame, ids, 2])
        d = delta[frame]
        for _ in range(200 if mode == 'floor' else 100):
            gaps = needed-w@d
            index = int(np.argmax(gaps))
            if gaps[index] < 1e-5:
                break
            row = w[index]
            denominator = row@row
            if denominator < 1e-12:
                raise ValueError('Unweighted correction vertex')
            d += gaps[index]/denominator*row
            np.clip(d, -.08 if mode == 'floor' else 0, .08 if mode == 'floor' else .03, out=d)
    for frame in range(len(base)):
        project(frame)
    for _ in range(3):
        old = delta.copy()
        delta = .8*old+.1*np.concatenate((old[:1], old[:-1]))+.1*np.concatenate((old[1:], old[-1:]))
        for frame in range(len(base)):
            project(frame)
    keys[:, :, 2, 3] += delta
    arrays['key_transforms'] = keys
    output.mkdir(parents=True, exist_ok=False)
    np.savez_compressed(output/'skinning.npz', **arrays)
    report = {'schema_version': 1, 'kind': 'translation_correction', 'mode': mode,
              'frames': len(base), 'max_bone_translation_m': float(np.abs(delta).max()),
              'rms_bone_translation_m': float(np.sqrt(np.mean(delta**2))),
              'minimum_predicted_ground_m': float(np.min(base[:, :, 2]+delta@weights.T))}
    (output/'correction-report.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps(report), flush=True)


def fit_finite_target(path, fixed_path, output, iterations, clip, seed_path=None):
    """Fit a mapped finite action without changing the shared garment weights."""
    from py_dem_bones.portable import _prepare_solver, _read_transformations, _pack_transformations
    with np.load(path,allow_pickle=False) as source:
        target=np.asarray(source['poses'],dtype=np.float64)
        faces=np.asarray(source['triangles'],dtype=np.int32)
    with np.load(fixed_path,allow_pickle=False) as source:
        rest=np.asarray(source['rest'],dtype=np.float64)
        fixed_faces=np.asarray(source['triangles'],dtype=np.int32)
        weights=np.asarray(source['weights'],dtype=np.float64)
        bind=np.asarray(source['bind_matrices'],dtype=np.float64)
    if target.ndim!=3 or target.shape[0]<2 or target.shape[1:]!=rest.shape or not np.isfinite(target).all():
        raise ValueError('Expected finite source-time poses with shared rest topology')
    if not np.array_equal(faces,fixed_faces) or weights.shape!=(len(rest),len(bind)):
        raise ValueError('Finite target and accepted Walk topology or weights differ')
    if clip=='Idle' and not np.array_equal(target[0],target[-1]):
        raise ValueError('Idle target must be explicitly authored to close before fitting')
    output.mkdir(parents=True,exist_ok=False)
    solver=_prepare_solver(rest,target,len(bind),faces=faces,max_influences=4,iterations=iterations)
    solver.set_weights(np.ascontiguousarray(weights.T))
    if seed_path is not None:
        with np.load(seed_path,allow_pickle=False) as source:
            if not np.array_equal(source['rest'],rest):raise ValueError('Finite transform seed rest differs')
            first=np.asarray(source['key_transforms'],dtype=np.float64)[0]
        if first.shape!=(len(bind),4,4):raise ValueError('Finite transform seed bone count differs')
        seed=np.repeat(first[None],len(target),axis=0)
        solver.set_transformations(_pack_transformations(seed,bone_count=len(bind)))
    solver.nTransIters=iterations;solver.computeTranformations()
    returned=np.asarray(solver.get_weights(),dtype=np.float64).T
    if solver.nB!=len(bind) or not np.array_equal(returned,weights):
        raise RuntimeError('Finite solve changed shared weights or bone identities')
    keys=_read_transformations(solver)
    if not np.isfinite(keys).all():raise RuntimeError('Finite solve returned non-finite transforms')
    rotations=keys[:,:,:3,:3];orth=float(np.max(np.abs(np.swapaxes(rotations,-1,-2)@rotations-np.eye(3))));det=float(np.max(np.abs(np.linalg.det(rotations)-1)))
    if max(orth,det)>1e-5:raise RuntimeError('Finite solve returned non-rigid transforms')
    endpoint_change=0.
    if clip=='Idle':
        endpoint_change=float(np.max(np.abs(keys[-1]-keys[0])));keys[-1]=keys[0]
    h=np.column_stack((rest,np.ones(len(rest))));predicted=np.einsum('vb,fbik,vk->fvi',weights,keys,h,optimize=True)[:,:,:3]
    e=np.linalg.norm(predicted-target,axis=2);vertex_rms=np.sqrt(np.mean(e**2,axis=0));frames=np.arange(1,len(target)+1,dtype=np.int32)
    np.savez_compressed(output/'skinning.npz',rest=rest,triangles=faces,weights=weights,
                        key_transforms=keys,bind_matrices=bind,key_source_frames=frames,
                        held_source_frames=np.array([],dtype=np.int32),reconstructed=predicted,target=target)
    report={'schema_version':1,'kind':'shared_weight_finite_fit','clip':clip,'bone_count':len(bind),
            'weight_max_difference_from_walk':0.,'source_fps':120,'frames':len(target),
            'rotation_orthogonality_max_error':orth,'rotation_determinant_max_error':det,
            'periodic':clip=='Idle','endpoint_matrix_forced_max_change':endpoint_change,
            'train':{'rms_m':float(np.sqrt(np.mean(e**2))),'per_vertex_rms_p95_m':float(np.quantile(vertex_rms,.95)),
                     'max_sample_m':float(e.max())},'held_out':None}
    (output/'fit-report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report),flush=True)


def fit_joint_targets(walk_path, run_path, output, bones, iterations, seed_path=None, seed_run_path=None):
    """Fit one shared rest/weights/bone set to the Walk and Run targets."""
    from py_dem_bones import solve_skinning
    with np.load(walk_path,allow_pickle=False) as source:
        walk=np.asarray(source['poses'],dtype=np.float64)
        faces=np.asarray(source['triangles'],dtype=np.int32)
    with np.load(run_path,allow_pickle=False) as source:
        run=np.asarray(source['poses'],dtype=np.float64)
        run_faces=np.asarray(source['triangles'],dtype=np.int32)
    if walk.shape[0]!=31 or run.shape[0]!=77 or walk.shape[1:]!=run.shape[1:] or not np.array_equal(faces,run_faces):
        raise ValueError('Expected compatible 30 Hz Walk and 120 Hz Run targets')
    if not np.isfinite(walk).all() or not np.isfinite(run).all() or not np.array_equal(walk[0],walk[-1]) or not np.array_equal(run[0],run[-1]):
        raise ValueError('Joint targets must be finite and periodic')
    output.mkdir(parents=True,exist_ok=False)
    wi=np.arange(0,31,2);ri=np.arange(0,77,4);rest=walk[0].copy()
    examples=np.concatenate((walk[wi],run[ri]))
    if seed_path is None:
        solution=solve_skinning(rest,examples,bones,faces=faces,max_influences=4,iterations=iterations)
        weights=np.asarray(solution.weights,dtype=np.float64).copy()
        matrices=np.asarray(solution.transforms,dtype=np.float64).copy()
    else:
        from py_dem_bones.portable import _prepare_solver,_read_transformations,_pack_transformations
        with np.load(seed_path,allow_pickle=False) as source:
            seed_weights=np.asarray(source['weights'],dtype=np.float64)
            seed_rest=np.asarray(source['rest'],dtype=np.float64)
            seed_faces=np.asarray(source['triangles'],dtype=np.int32)
            seed_walk=np.asarray(source['key_transforms'],dtype=np.float64)
        with np.load(seed_run_path,allow_pickle=False) as source:
            run_weights=np.asarray(source['weights'],dtype=np.float64)
            seed_run=np.asarray(source['key_transforms'],dtype=np.float64)
        if not np.array_equal(seed_rest,rest) or not np.array_equal(seed_faces,faces) or not np.array_equal(seed_weights,run_weights) or seed_weights.shape[1]!=bones:
            raise ValueError('Joint seed rest, topology, shared weights or bone count mismatch')
        seed_matrices=np.concatenate((seed_walk,seed_run))
        if seed_matrices.shape!=(len(examples),bones,4,4):
            raise ValueError('Joint seed frame or transform layout mismatch')
        solver=_prepare_solver(rest,examples,bones,faces=faces,max_influences=4,iterations=iterations)
        solver.set_weights(np.ascontiguousarray(seed_weights.T))
        solver.set_transformations(_pack_transformations(seed_matrices,bone_count=bones))
        solver.compute()
        weights=np.asarray(solver.get_weights(),dtype=np.float64).copy()
        matrices=_read_transformations(solver)
    if not np.isfinite(weights).all() or not np.isfinite(matrices).all() or weights.min() < -1e-12:
        raise RuntimeError('Joint solver returned invalid weights or transforms')
    weights[np.abs(weights)<1e-12]=0
    totals=weights.sum(axis=0)
    if np.any(totals<=0) or np.max(np.abs(totals-1))>1e-5 or np.any(np.count_nonzero(weights,axis=0)>4):
        raise RuntimeError('Joint solver returned invalid four-weight skinning')
    weights/=totals
    active=weights.sum(axis=1)>1e-12;weights=weights[active];matrices=matrices[:,active]
    r=matrices[:,:,:3,:3];orth=float(np.max(np.abs(np.swapaxes(r,-1,-2)@r-np.eye(3))));det=float(np.max(np.abs(np.linalg.det(r)-1)))
    if max(orth,det)>1e-5:raise RuntimeError('Joint solver returned non-rigid transforms')
    bind=np.repeat(np.eye(4)[None],len(weights),axis=0);bind[:,:3,3]=weights@rest/weights.sum(axis=1)[:,None]
    homogeneous=np.column_stack((rest,np.ones(len(rest))))
    def midpoint(a,b):
        u,s,vh=np.linalg.svd(a+b)
        if float(s.min())<1e-6:raise RuntimeError('Ambiguous near-180-degree joint interpolation')
        result=u@vh
        if np.any(np.linalg.det(result)<0):raise RuntimeError('Joint interpolation reflected a bone')
        return result
    def package(name,target,indices,keys,source_scale):
        key_change=float(np.max(np.abs(keys[-1]-keys[0])));keys=keys.copy();keys[-1]=keys[0]
        stride=int(indices[1]-indices[0]);left,right=keys[:-1],keys[1:];rhalf=midpoint(left[:,:,:3,:3],right[:,:,:3,:3])
        rotations={1:rhalf} if stride==2 else {1:midpoint(left[:,:,:3,:3],rhalf),2:rhalf,3:midpoint(rhalf,right[:,:,:3,:3])}
        center=bind[:,:3,3];lc=left[:,:,:3,3]+np.einsum('fbij,bj->fbi',left[:,:,:3,:3],center);rc=right[:,:,:3,3]+np.einsum('fbij,bj->fbi',right[:,:,:3,:3],center)
        interpolated=np.zeros((len(target),len(bind),4,4),dtype=np.float64);interpolated[indices]=keys
        for k,rotation in rotations.items():
            u=k/stride;interpolated[k:-1:stride,:,:3,:3]=rotation
            interpolated[k:-1:stride,:,:3,3]=(1-u)*lc+u*rc-np.einsum('fbij,bj->fbi',rotation,center)
            interpolated[k:-1:stride,:,3,3]=1
        predicted=np.einsum('bv,fbik,vk->fvi',weights,interpolated,homogeneous,optimize=True)[:,:,:3]
        held=np.array([i for i in range(len(target)) if i not in set(indices)],dtype=np.int32)
        def errors(ix):
            e=np.linalg.norm(predicted[ix]-target[ix],axis=2);v=np.sqrt(np.mean(e**2,axis=0))
            return {'rms_m':float(np.sqrt(np.mean(e**2))),'per_vertex_rms_p95_m':float(np.quantile(v,.95)),'max_sample_m':float(e.max())}
        np.savez_compressed(output/(name+'.npz'),rest=rest,triangles=faces,weights=weights.T,
                            transforms=interpolated,key_transforms=keys,bind_matrices=bind,
                            key_source_frames=1+indices*source_scale,held_source_frames=1+held*source_scale,
                            reconstructed=predicted,target=target)
        return {'train':errors(indices),'held_out':errors(held),'endpoint_matrix_forced_max_change':key_change}
    walk_report=package('Walk',walk,wi,matrices[:len(wi)],4)
    run_report=package('Run',run,ri,matrices[len(wi):],1)
    report={'schema_version':1,'kind':'joint_walk_run_fit','seeded':seed_path is not None,'requested_bones':bones,'returned_bones':len(weights),
            'max_influences':int(np.max(np.count_nonzero(weights,axis=0))),'minimum_weight':float(weights.min()),
            'max_weight_sum_error':float(np.max(np.abs(weights.sum(axis=0)-1))),
            'rotation_orthogonality_max_error':orth,'rotation_determinant_max_error':det,
            'walk':walk_report,'run':run_report,'source_fps':120}
    (output/'fit-report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report),flush=True)


def fit_fixed_target(path, fixed_path, output, iterations, key_step, transform_seed=None):
    """Fit another action using the accepted Walk rest, weights and bones."""
    from py_dem_bones.portable import _prepare_solver, _read_transformations, _pack_transformations

    with np.load(path, allow_pickle=False) as source:
        target = np.asarray(source['poses'], dtype=np.float64)
        faces = np.asarray(source['triangles'], dtype=np.int32)
    with np.load(fixed_path, allow_pickle=False) as source:
        rest = np.asarray(source['rest'], dtype=np.float64)
        fixed_faces = np.asarray(source['triangles'], dtype=np.int32)
        weights = np.asarray(source['weights'], dtype=np.float64)
        bind = np.asarray(source['bind_matrices'], dtype=np.float64)
    if target.ndim != 3 or target.shape[0] != 77 or target.shape[1:] != rest.shape:
        raise ValueError('Expected 77 source-time Run poses with the Walk topology')
    if not np.isfinite(target).all() or not np.array_equal(target[0],target[-1]):
        raise ValueError('Run target must be finite and periodic')
    if not np.array_equal(faces,fixed_faces) or weights.shape != (len(rest),len(bind)):
        raise ValueError('Run and accepted Walk topology or weights differ')
    output.mkdir(parents=True,exist_ok=False)
    train_index = np.arange(0,77,key_step)
    held_index = np.array([i for i in range(77) if i%key_step],dtype=np.int32)
    solver = _prepare_solver(rest,target[train_index],len(bind),faces=faces,
                             max_influences=4,iterations=iterations)
    solver.set_weights(np.ascontiguousarray(weights.T))
    if transform_seed is not None:
        with np.load(transform_seed,allow_pickle=False) as source:
            seed=np.asarray(source['transforms'],dtype=np.float64)
            if not np.array_equal(source['rest'],rest) or seed.shape!=(77,len(bind),4,4):
                raise ValueError('Run transform seed has mismatched rest or bone layout')
        solver.set_transformations(_pack_transformations(seed[train_index],bone_count=len(bind)))
    solver.nTransIters = iterations
    solver.computeTranformations()
    returned_weights = np.asarray(solver.get_weights(),dtype=np.float64).T
    weight_error = float(np.max(np.abs(returned_weights-weights)))
    if weight_error != 0 or solver.nB != len(bind):
        raise RuntimeError('Transform-only solve changed the shared Walk weights or bones')
    keys = _read_transformations(solver)
    if not np.isfinite(keys).all():
        raise RuntimeError('Non-finite fixed-weight solution')
    gram = np.swapaxes(keys[:,:,:3,:3],-1,-2) @ keys[:,:,:3,:3]
    orth_error = float(np.max(np.abs(gram-np.eye(3))))
    determinant_error = float(np.max(np.abs(np.linalg.det(keys[:,:,:3,:3])-1)))
    if max(orth_error,determinant_error)>1e-5:
        raise RuntimeError('Fixed-weight solution contains non-rigid transforms')
    endpoint_change = float(np.max(np.abs(keys[-1]-keys[0])))
    keys[-1] = keys[0]

    def midpoint(a,b):
        u,s,vh = np.linalg.svd(a+b)
        if float(s.min())<1e-6:
            raise RuntimeError('Ambiguous near-180-degree rotation interpolation')
        r = u@vh
        if np.any(np.linalg.det(r)<0):
            raise RuntimeError('Rotation interpolation reflected a bone')
        return r

    matrices=np.zeros((77,len(bind),4,4),dtype=np.float64);matrices[train_index]=keys
    if key_step>1:
        left,right=keys[:-1],keys[1:]
        rmid=midpoint(left[:,:,:3,:3],right[:,:,:3,:3])
        rotations={1:rmid} if key_step==2 else {1:midpoint(left[:,:,:3,:3],rmid),2:rmid,3:midpoint(rmid,right[:,:,:3,:3])}
        center=bind[:,:3,3]
        lc=left[:,:,:3,3]+np.einsum('fbij,bj->fbi',left[:,:,:3,:3],center)
        rc=right[:,:,:3,3]+np.einsum('fbij,bj->fbi',right[:,:,:3,:3],center)
        for j,r in rotations.items():
            fraction=j/key_step
            matrices[j:76:key_step,:,:3,:3]=r
            matrices[j:76:key_step,:,:3,3]=(1-fraction)*lc+fraction*rc-np.einsum('fbij,bj->fbi',r,center)
            matrices[j:76:key_step,:,3,3]=1
    h=np.column_stack((rest,np.ones(len(rest))))
    predicted=np.einsum('vb,fbik,vk->fvi',weights,matrices,h,optimize=True)[:,:,:3]
    def errors(indices):
        if len(indices)==0:return None
        e=np.linalg.norm(predicted[indices]-target[indices],axis=2)
        vertex_rms=np.sqrt(np.mean(e**2,axis=0))
        return {'all_samples_rms_m':float(np.sqrt(np.mean(e**2))),
                'per_vertex_rms_p95_m':float(np.quantile(vertex_rms,.95)),
                'per_vertex_rms_max_m':float(vertex_rms.max()),'max_sample_m':float(e.max())}
    np.savez_compressed(output/'skinning.npz',rest=rest,triangles=faces,weights=weights,
                        transforms=matrices,key_transforms=keys,bind_matrices=bind,
                        key_source_frames=1+train_index,held_source_frames=1+held_index,
                        reconstructed=predicted,target=target)
    report={'schema_version':1,'kind':'shared_weight_run_fit','bone_count':len(bind),
            'weight_max_difference_from_walk':weight_error,'transform_iterations':iterations,
            'rotation_orthogonality_max_error':orth_error,'rotation_determinant_max_error':determinant_error,
            'endpoint_matrix_forced_max_change':endpoint_change,'train':errors(train_index),
            'held_out':errors(held_index),'key_step':key_step,'source_fps':120,
            'training_frame_keys':(1+train_index).tolist(),'held_out_frame_keys':(1+held_index).tolist(),
            'coordinate_space':'Blender world metres; column-vector T maps rest points to posed points'}
    (output/'fit-report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report),flush=True)


def fit_target(path, output, bones, iterations):
    """Fit alternate samples; assess the intervening phases independently."""
    from py_dem_bones import solve_skinning

    with np.load(path, allow_pickle=False) as source:
        target = np.asarray(source['poses'], dtype=np.float64)
        faces = np.asarray(source['triangles'], dtype=np.int32)
    if target.ndim != 3 or target.shape[0] != 31 or target.shape[2] != 3:
        raise ValueError('Expected one periodic 30 Hz Walk with 31 samples')
    if not np.isfinite(target).all() or not np.array_equal(target[0], target[-1]):
        raise ValueError('Target must be finite with identical endpoints')
    if faces.ndim != 2 or faces.shape[1] != 3 or faces.min() < 0 or faces.max() >= target.shape[1]:
        raise ValueError('Expected valid triangle topology')
    if bones > target.shape[1]:
        raise ValueError('Requested bone count exceeds vertex count')
    output.mkdir(parents=True, exist_ok=False)
    rest = target[0].copy()
    train_index = np.arange(0, 31, 2)
    held_index = np.arange(1, 30, 2)
    result = solve_skinning(rest, target[train_index], bones, faces=faces,
                            max_influences=4, iterations=iterations)
    weights = np.asarray(result.weights, dtype=np.float64).copy()
    transforms = np.asarray(result.transforms, dtype=np.float64).copy()
    if weights.ndim != 2 or weights.shape[1] != len(rest) or transforms.shape != (16, weights.shape[0], 4, 4):
        raise RuntimeError('Unexpected solver output layout')
    if not np.isfinite(weights).all() or not np.isfinite(transforms).all():
        raise RuntimeError('Non-finite skinning solution')
    if weights.min() < -1e-12:
        raise RuntimeError(f'Negative solver weight: {weights.min()}')
    original_weights = weights.copy()
    weights[np.abs(weights) < 1e-12] = 0
    if np.any(np.count_nonzero(weights, axis=0) > 4):
        raise RuntimeError('Solver returned more than four nonzero weights')
    totals = weights.sum(axis=0)
    if np.any(totals <= 0) or np.max(np.abs(totals-1)) > 1e-5:
        raise RuntimeError('Solver returned invalid vertex weight sums')
    weights /= totals
    unused = np.sum(weights, axis=1) <= 1e-12
    weights = weights[~unused]
    transforms = transforms[:, ~unused]
    if not len(weights):
        raise RuntimeError('Solver returned no weighted bones')
    # Bone translation channels interpolate the animated bind-position, not
    # the translation component of the skinning matrix around world origin.
    bind = np.repeat(np.eye(4)[None], len(weights), axis=0)
    bind[:, :3, 3] = weights @ rest / weights.sum(axis=1)[:, None]
    rotation = transforms[:, :, :3, :3]
    gram = np.swapaxes(rotation, -1, -2) @ rotation
    rigid_error = float(np.max(np.abs(gram-np.eye(3))))
    determinant_error = float(np.max(np.abs(np.linalg.det(rotation)-1)))
    affine_error = float(np.max(np.abs(transforms[:, :, 3, :]-[0,0,0,1])))
    if max(rigid_error, determinant_error, affine_error) > 1e-5:
        raise RuntimeError('Solver returned scaling, shear, reflection, or non-affine data')

    # The same target shape occurs at both endpoints. Make the bone keys
    # themselves identical and measure the resulting endpoint error rather
    # than assuming that independently solved duplicate poses match.
    loop_matrix_change = float(np.max(np.abs(transforms[-1]-transforms[0])))
    transforms[-1] = transforms[0]
    mid = np.zeros((15, len(weights), 4, 4), dtype=np.float64)
    # Polar midpoint equals quaternion slerp at t=.5 for proper rotations whose
    # relative angle is below 180 degrees. Reject ambiguous antipodal pairs.
    u, singular, vh = np.linalg.svd(transforms[:-1, :, :3, :3] + transforms[1:, :, :3, :3])
    if float(singular.min()) < 1e-6:
        raise RuntimeError('Ambiguous near-180-degree held-out bone interpolation')
    midpoint_rotation = u @ vh
    if np.any(np.linalg.det(midpoint_rotation) < 0):
        raise RuntimeError('Held-out rotation interpolation reflected a bone')
    mid[:, :, :3, :3] = midpoint_rotation
    center = bind[:, :3, 3]
    left_center = transforms[:-1, :, :3, 3] + np.einsum('fbij,bj->fbi', transforms[:-1, :, :3, :3], center)
    right_center = transforms[1:, :, :3, 3] + np.einsum('fbij,bj->fbi', transforms[1:, :, :3, :3], center)
    mid[:, :, :3, 3] = (left_center+right_center)*.5 - np.einsum('fbij,bj->fbi', midpoint_rotation, center)
    mid[:, :, 3, 3] = 1
    homogeneous = np.column_stack((rest, np.ones(len(rest))))

    def reconstruct(matrices):
        return np.einsum('bv,fbik,vk->fvi', weights, matrices, homogeneous,
                         optimize=True)[:, :, :3]

    def errors(predicted, actual):
        e = np.linalg.norm(predicted-actual, axis=2)
        vertex_rms = np.sqrt(np.mean(e**2, axis=0))
        return {'all_samples_rms_m': float(np.sqrt(np.mean(e**2))),
                'per_vertex_rms_p95_m': float(np.quantile(vertex_rms,.95)),
                'per_vertex_rms_max_m': float(vertex_rms.max()),
                'max_sample_m': float(e.max())}

    train_pred = reconstruct(transforms)
    held_pred = reconstruct(mid)
    reconstructed = np.empty_like(target)
    reconstructed[train_index] = train_pred
    reconstructed[held_index] = held_pred
    transforms_30hz = np.empty((31, len(weights), 4, 4), dtype=np.float64)
    transforms_30hz[train_index] = transforms
    transforms_30hz[held_index] = mid
    np.savez_compressed(output/'skinning.npz', rest=rest, triangles=faces,
                        weights=weights.T, transforms=transforms_30hz,
                        key_transforms=transforms, bind_matrices=bind,
                        key_source_frames=1+train_index*4,
                        held_source_frames=1+held_index*4,
                        reconstructed=reconstructed, target=target)
    report = {'schema_version':1,'kind':'cloth_bone_fit','requested_bones':bones,
              'returned_bones':int(len(weights)),'iterations':iterations,
              'max_influences':int(np.max(np.count_nonzero(weights,axis=0))),
              'minimum_weight':float(weights.min()),'maximum_weight_sum_error':float(np.max(np.abs(weights.sum(axis=0)-1))),
              'weight_normalization_max_change':float(np.max(np.abs(weights-original_weights[~unused]))),
              'rotation_orthogonality_max_error':rigid_error,'rotation_determinant_max_error':determinant_error,
              'affine_last_row_max_error':affine_error,'endpoint_matrix_forced_max_change':loop_matrix_change,
              'train':errors(train_pred,target[train_index]),'held_out':errors(held_pred,target[held_index]),
              'coordinate_space':'Blender world metres; column-vector T maps rest points to posed points',
              'source_fps':120,'training_frame_keys':(1+train_index*4).tolist(),
              'held_out_frame_keys':(1+held_index*4).tolist()}
    (output/'fit-report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report),flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    inputs = parser.add_mutually_exclusive_group(required=True)
    inputs.add_argument('--cache', type=Path)
    inputs.add_argument('--target', type=Path)
    parser.add_argument('--fixed', type=Path,
                        help='Accepted Walk skinning arrays for a fixed-weight Run fit')
    parser.add_argument('--finite',action='store_true',
                        help='Fit a mapped Idle, Hit or Death target at all source frames')
    parser.add_argument('--extract-finite', action='store_true',
                        help='Extract mapped finite simulation poses; author an Idle loop')
    parser.add_argument('--correction', choices=('floor', 'exposure'),
                        help='Apply the accepted Death translation correction to --target fit')
    parser.add_argument('--playback', type=Path,
                        help='Actual Blender sampled positions required by --correction')
    parser.add_argument('--joint-run', type=Path,
                        help='Run target to fit jointly with the Walk target')
    parser.add_argument('--seed', type=Path, help='Accepted Walk fit used to initialize a joint solve')
    parser.add_argument('--seed-run', type=Path, help='Fixed-weight Run fit used to initialize a joint solve')
    parser.add_argument('--key-step',type=int,choices=(1,2,4),default=4,
                        help='Run transform key interval in 120 Hz frames')
    parser.add_argument('--transform-seed',type=Path,
                        help='Prior Run transforms used to initialize a fixed-weight solve')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--target-only', action='store_true')
    parser.add_argument('--bones', type=int, default=24)
    parser.add_argument('--iterations', type=int, default=30)
    parser.add_argument('--seam', type=int, default=85,
                        help='One-based first simulation frame of the selected loop')
    parser.add_argument('--period', type=int, default=30,
                        help='Loop period in 30 Hz simulation frames')
    parser.add_argument('--phase-origin', type=int, default=61,
                        help='Simulation frame corresponding to source action frame 1')
    parser.add_argument('--clip', choices=('Walk', 'Run', 'Idle', 'Hit', 'Death'), default='Walk')
    args = parser.parse_args()
    if args.bones < 1 or args.iterations < 1:
        parser.error('bones and iterations must be positive')
    if args.extract_finite:
        if args.cache is None or args.fixed is None or args.clip not in ('Idle', 'Hit', 'Death'):
            parser.error('--extract-finite requires --cache, --fixed and Idle, Hit or Death')
        extract_finite(args.cache, args.fixed, args.output, args.clip)
        return
    if args.correction is not None:
        if args.target is None or args.playback is None:
            parser.error('--correction requires --target fit and --playback')
        polish_translation(args.target, args.playback, args.output, args.correction)
        return
    if args.target is not None:
        if args.joint_run is not None:
            if args.fixed is not None:parser.error('--joint-run and --fixed cannot be combined')
            if (args.seed is None)!=(args.seed_run is None):parser.error('--seed and --seed-run must be supplied together')
            fit_joint_targets(args.target,args.joint_run,args.output,args.bones,args.iterations,args.seed,args.seed_run)
        elif args.fixed is not None and args.finite:
            if args.clip not in ('Idle','Hit','Death'):parser.error('--finite requires Idle, Hit or Death')
            fit_finite_target(args.target,args.fixed,args.output,args.iterations,args.clip,args.transform_seed)
        elif args.fixed is not None:
            fit_fixed_target(args.target,args.fixed,args.output,args.iterations,args.key_step,args.transform_seed)
        else:
            fit_target(args.target, args.output, args.bones, args.iterations)
        return
    with np.load(args.cache, allow_pickle=False) as source:
        poses = np.asarray(source['poses'], dtype=np.float64)
        triangles = np.asarray(source['triangles'], dtype=np.int32)
        times = np.asarray(source['times'], dtype=np.float64)
    if poses.ndim != 3 or poses.shape[2] != 3 or not np.isfinite(poses).all():
        raise ValueError('Expected finite 30 Hz simulation positions')
    if args.period < 8 or args.seam < 1 or args.seam+args.period > len(poses):
        raise ValueError('Selected seam and period are outside the simulation')
    if args.phase_origin < 1 or args.phase_origin+2*args.period > len(poses):
        raise ValueError('The simulation does not contain two audit cycles')
    if len(times) != len(poses) or len(times)<2:
        raise ValueError('Expected one time value per simulation pose')
    fps = round(1/float(times[1]-times[0]))
    if fps not in (30,120) or not np.allclose(np.diff(times),1/fps):
        raise ValueError('Input samples must be at a constant 30 Hz or 120 Hz')
    if not args.target_only and (fps != 30 or args.period != 30 or args.clip != 'Walk'):
        raise ValueError('Automatic fitting requires a 30 Hz Walk; use --target-only for other clips')
    if triangles.ndim != 2 or triangles.shape[1] != 3 or triangles.min() < 0 or triangles.max() >= poses.shape[1]:
        raise ValueError('Expected valid zero-based triangle indices')
    if args.bones > poses.shape[1]:
        raise ValueError('Requested bone count exceeds vertex count')
    args.output.mkdir(parents=True, exist_ok=False)

    # Blend only 1/7.5 seconds on either side of the selected seam.
    # The seam is chosen from actual geometry; it is not a converged simulation.
    raw = poses[args.seam-1:args.seam+args.period].copy()
    delta = raw[0] - raw[-1]
    phase = np.arange(args.period+1, dtype=np.float64)
    blend_frames = 4*(fps//30)
    left = np.clip(phase/blend_frames, 0, 1)
    right = np.clip((phase-(args.period-blend_frames))/blend_frames, 0, 1)
    left = 3*left**2 - 2*left**3
    right = 3*right**2 - 2*right**3
    correction = (-.5*(1-left)+.5*right)[:, None, None]*delta
    corrected = raw + correction
    ring = corrected[:-1]
    filtered = ring.copy()
    adjustment = np.linalg.norm(filtered-ring, axis=2)
    raw_adjustment = np.linalg.norm(filtered-raw[:-1], axis=2)
    offset = (args.phase_origin-args.seam) % args.period
    filtered = np.roll(filtered, -offset, axis=0)
    target = np.concatenate((filtered, filtered[:1]), axis=0)
    np.savez_compressed(args.output/'periodic-target.npz',
                        rest=target[0], poses=target, triangles=triangles,
                        times=np.arange(args.period+1, dtype=np.float64)/fps,
                        source_frames=np.arange(args.seam, args.seam+args.period+1, dtype=np.int32),
                        coordinate_space=np.array('Blender world metres, +Z up'))
    # This shape matches the independent Blender contact auditor's global
    # timeline: two repeats of the periodic target against the actual body.
    audit = poses.copy()
    audit_start = args.phase_origin-1
    audit[audit_start:audit_start+2*args.period+1] = np.concatenate((target[:-1], target[:-1], target[:1]), axis=0)
    np.savez_compressed(args.output/'periodic-audit.npz', poses=audit,
                        triangles=triangles, times=times)
    report = {'schema_version': 1, 'kind': 'authored_periodic_clip', 'clip':args.clip,
              'source_frames': [args.seam, args.seam+args.period], 'fps':fps,
              'seconds': args.period/fps, 'period_frames':args.period,
              'raw_endpoint_rms_m': float(np.sqrt(np.mean(delta**2)*3)),
              'raw_endpoint_max_m': float(np.max(np.linalg.norm(delta, axis=1))),
              'periodic_endpoint_max_m': float(np.max(np.abs(target[-1]-target[0]))),
              'authoring_adjustment_rms_m': float(np.sqrt(np.mean(raw_adjustment**2))),
              'authoring_adjustment_p95_m': float(np.quantile(raw_adjustment, .95)),
              'authoring_adjustment_max_m': float(raw_adjustment.max()),
              'smoothing_adjustment_max_m': float(adjustment.max()),
              'velocity_join_rms_m_per_frame': float(np.sqrt(np.mean(((target[1]-target[0])-(target[-1]-target[-2]))**2)*3))}
    (args.output/'target-report.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps(report), flush=True)
    if not args.target_only:
        fit_target(args.output/'periodic-target.npz', args.output/'fit', args.bones, args.iterations)


if __name__ == '__main__':
    try:
        main()
    except Exception as exc:
        print(json.dumps({'schema_version': 1, 'kind': 'error',
                          'error_type': type(exc).__name__, 'message': str(exc)[:500]}), file=sys.stderr)
        sys.exit(1)
