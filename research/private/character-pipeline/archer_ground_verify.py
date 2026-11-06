"""Measure evaluated per-foot minima after importing an animated GLB."""
import argparse,json,sys
from pathlib import Path
import bpy
p=argparse.ArgumentParser();p.add_argument('--model',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--frames',type=int,nargs='+',required=True);a=p.parse_args(sys.argv[sys.argv.index('--')+1:]);bpy.ops.wm.read_factory_settings(use_empty=True);bpy.ops.import_scene.gltf(filepath=str(a.model.resolve()));scene=bpy.context.scene;rig=next(o for o in scene.objects if o.type=='ARMATURE');mesh=next(o for o in scene.objects if o.type=='MESH' and o.parent==rig);ids={}
for side in ('Left','Right'):
 names={side+'Foot',side+'ToeBase'};groups={g.index for g in mesh.vertex_groups if g.name in names};ids[side]=[v.index for v in mesh.data.vertices if sum(x.weight for x in v.groups if x.group in groups)>.5]
 if not ids[side]:raise ValueError('No weighted foot vertices')
rows={}
for f in a.frames:
 scene.frame_set(f);bpy.context.view_layer.update();obj=mesh.evaluated_get(bpy.context.evaluated_depsgraph_get());em=obj.to_mesh();rows[f]={side:min((obj.matrix_world@em.vertices[i].co).z for i in ix) for side,ix in ids.items()};obj.to_mesh_clear()
a.output.write_text(json.dumps(rows,indent=2)+'\n')
for f,r in rows.items():print(f,*(side+':'+str(round(z,4)) for side,z in r.items()))
