"""Measure evaluated rear-knee surface and both leg segment lengths in GLB."""
import argparse,json,sys
from pathlib import Path
import bpy
p=argparse.ArgumentParser();p.add_argument('--model',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--frames',type=int,nargs='+',required=True);a=p.parse_args(sys.argv[sys.argv.index('--')+1:]);bpy.ops.wm.read_factory_settings(use_empty=True);bpy.ops.import_scene.gltf(filepath=str(a.model.resolve()));scene=bpy.context.scene;rig=next(o for o in scene.objects if o.type=='ARMATURE');mesh=next(o for o in scene.objects if o.type=='MESH' and o.parent==rig);joint=rig.matrix_world@rig.data.bones['RightLeg'].head_local;group=mesh.vertex_groups['RightLeg'];ids=[v.index for v in mesh.data.vertices if (mesh.matrix_world@v.co-joint).length<.16 and any(g.group==group.index and g.weight>.5 for g in v.groups)]
if not ids:raise ValueError('No rear-knee surface vertices')
rows={}
for f in a.frames:
 scene.frame_set(f);bpy.context.view_layer.update();deps=bpy.context.evaluated_depsgraph_get();obj=mesh.evaluated_get(deps);em=obj.to_mesh();knee=min((obj.matrix_world@em.vertices[i].co).z for i in ids);obj.to_mesh_clear();pose=rig.evaluated_get(deps)
 def pt(n):return pose.matrix_world@pose.pose.bones[n].head
 rows[f]={'rear_knee_surface_z':knee,'left_thigh':(pt('LeftUpLeg')-pt('LeftLeg')).length,'left_shin':(pt('LeftLeg')-pt('LeftFoot')).length,'right_thigh':(pt('RightUpLeg')-pt('RightLeg')).length,'right_shin':(pt('RightLeg')-pt('RightFoot')).length}
a.output.write_text(json.dumps(rows,indent=2)+'\n');print('KNEE_SAMPLES',len(ids),rows,flush=True)
