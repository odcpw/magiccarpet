"""Measure rendered hand, bow grip, nock and arrow positions after GLB reimport."""
import argparse,json,sys
from pathlib import Path
import bpy
from mathutils import Vector
p=argparse.ArgumentParser();p.add_argument('--model',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--frames',type=int,nargs='+',required=True);a=p.parse_args(sys.argv[sys.argv.index('--')+1:]);bpy.ops.wm.read_factory_settings(use_empty=True);bpy.ops.import_scene.gltf(filepath=str(a.model.resolve()));scene=bpy.context.scene;rig=next(o for o in scene.objects if o.type=='ARMATURE');string=bpy.data.objects['Bowstring'];bow=bpy.data.objects['BowControl'];arrow=bpy.data.objects['ArrowControl'];rows={}
body=next(o for o in scene.objects if o.type=='MESH' and o.parent==rig)
hand_ids={}
for side,lower,upper in [('Left',.10,.14),('Right',.13,.17)]:
 fore=rig.matrix_world@rig.data.bones[side+'ForeArm'].head_local;wrist=rig.matrix_world@rig.data.bones[side+'Hand'].head_local;span=(wrist-fore).length;axis=(wrist-fore).normalized();lower*=span/.237;upper*=span/.237;group=body.vertex_groups[side+'Hand'];hand_ids[side]=[v.index for v in body.data.vertices if any(g.group==group.index and g.weight>.5 for g in v.groups) and lower<(body.matrix_world@v.co-wrist).dot(axis)<upper]

for frame in a.frames:
 scene.frame_set(frame);bpy.context.view_layer.update();dep=bpy.context.evaluated_depsgraph_get();string_ev=string.evaluated_get(dep);m=string_ev.to_mesh();verts=[string_ev.matrix_world@v.co for v in m.vertices];base=[bow.matrix_world.inverted()@string.matrix_world@v.co for v in string.data.vertices];nearest=min(abs(v.z) for v in base);centers=[verts[i] for i,v in enumerate(base) if abs(abs(v.z)-nearest)<.0001];nock=sum(centers,Vector())/len(centers)
 rings={}
 for i,v in enumerate(base):rings.setdefault(round(v.z,4),[]).append(verts[i])
 ring_centers=[sum(points,Vector())/len(points) for z,points in sorted(rings.items())]
 length=sum((a-b).length for a,b in zip(ring_centers,ring_centers[1:]));string_ev.to_mesh_clear();rig_ev=rig.evaluated_get(dep)
 def hand(n):return rig_ev.matrix_world@rig_ev.pose.bones[n].head
 body_ev=body.evaluated_get(dep);bm=body_ev.to_mesh()
 fingers={side:sum((body_ev.matrix_world@bm.vertices[i].co for i in ids),Vector())/len(ids) for side,ids in hand_ids.items()}
 body_ev.to_mesh_clear()
 rows[frame]={'string_length':length,'string_ring_count':len(ring_centers),'left_wrist':list(hand('LeftHand')),'right_wrist':list(hand('RightHand')),'grip':list(bow.matrix_world.translation),'nock':list(nock),'arrow_control':list(arrow.matrix_world.translation),'right_finger_nock_dist':(nock-fingers['Right']).length,'left_finger_grip_dist':(bow.matrix_world.translation-fingers['Left']).length,'finger_gap':(fingers['Left']-fingers['Right']).length,'right_nock_dist':(nock-hand('RightHand')).length,'left_grip_dist':(bow.matrix_world.translation-hand('LeftHand')).length}
a.output.write_text(json.dumps(rows,indent=2)+'\n')
for frame,r in rows.items():print(frame,'string',round(r['string_length'],5),'rings',r['string_ring_count'],'fingernock',round(r['right_finger_nock_dist'],3),'fingergrip',round(r['left_finger_grip_dist'],3),'gap',round(r['finger_gap'],3),'nockdist',round(r['right_nock_dist'],3),'gripdist',round(r['left_grip_dist'],3), 'nock',[round(x,3) for x in r['nock']],'right',[round(x,3) for x in r['right_wrist']])
