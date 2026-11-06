"""Check minimum world-space height of the bow in reimported animation."""
import argparse,json,sys
from pathlib import Path
import bpy
p=argparse.ArgumentParser();p.add_argument('--model',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--frames',type=int,nargs='+',required=True);a=p.parse_args(sys.argv[sys.argv.index('--')+1:]);bpy.ops.wm.read_factory_settings(use_empty=True);bpy.ops.import_scene.gltf(filepath=str(a.model.resolve()));scene=bpy.context.scene;parts=[o for o in scene.objects if o.type=='MESH' and o.name.startswith(('Bow limb','Bowstring','Horn bow tip','Grip core','Grip binding'))];assert parts;rows={}
for f in a.frames:
 scene.frame_set(f);bpy.context.view_layer.update();deps=bpy.context.evaluated_depsgraph_get();lowest=float('inf')
 for obj in parts:
  e=obj.evaluated_get(deps);m=e.to_mesh();lowest=min(lowest,min((e.matrix_world@v.co).z for v in m.vertices));e.to_mesh_clear()
 rows[f]=lowest
print('BOW_CLEARANCE',rows,flush=True);a.output.write_text(json.dumps(rows,indent=2)+'\n')
