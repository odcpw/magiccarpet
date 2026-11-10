"""Dress the matched MPFB human in source-inspired Villager 241 accessories.

The full anatomical mesh and official MakeHuman clothing mask remain editable.
This builds a static candidate; motions come from the same Rigify master.
"""
import argparse, hashlib, json, math, struct, sys
from pathlib import Path
import bpy,bmesh
from mathutils import Vector
from mathutils.bvhtree import BVHTree
from mathutils.kdtree import KDTree

EXPECTED_UPPER_FINGERPRINT='4241c1c88809795971533d26580bf4a082433faecb273023da70a5ed59283e7e'

def upper_fingerprint(obj):
 """Identify source positions, topology and named weights before conversion."""
 digest=hashlib.sha256()
 for vertex in obj.data.vertices:
  digest.update(struct.pack('<3fI',*vertex.co,len(vertex.groups)))
  for group in sorted(vertex.groups,key=lambda item:obj.vertex_groups[item.group].name):
   digest.update(obj.vertex_groups[group.group].name.encode()+b'\0');digest.update(struct.pack('<f',group.weight))
 for face in obj.data.polygons:
  digest.update(struct.pack('<I',len(face.vertices)))
  for index in face.vertices:digest.update(struct.pack('<I',index))
 return digest.hexdigest()

def prepare_upper_export(input_path, output_dir, use_render_subdiv=True, collar_clearance=False):
 """Freeze the rendered rest surface and retain the four strongest deform weights.

 The source object remains editable in the saved copy, hidden under a distinct
 name. This replaces only the upper mesh; it does not change the source file,
 skeleton, lower garment, actions, or Rigify drivers.
 """
 import numpy as np
 if output_dir.exists():raise FileExistsError('Choose a fresh output directory')
 bpy.ops.wm.open_mainfile(filepath=str(input_path.resolve()))
 scene=bpy.context.scene;rig=bpy.data.objects['VillagerRig'];source=bpy.data.objects['Human.donitz_monk_robe']
 if bpy.data.objects.get('Human.donitz_monk_robe [editable source]') is not None or upper_fingerprint(source)!=EXPECTED_UPPER_FINGERPRINT:
  raise ValueError('Unexpected or already-prepared upper mesh; no output written')
 drivers=len(rig.animation_data.drivers);prior_action=rig.animation_data.action;prior_slot=rig.animation_data.action_slot;prior_pose=rig.data.pose_position
 prior_frame,prior_subframe=scene.frame_current,scene.frame_subframe
 armatures=[m for m in source.modifiers if m.type=='ARMATURE' and m.show_viewport]
 subdivisions=[m for m in source.modifiers if m.type=='SUBSURF' and m.show_viewport]
 if (len(armatures)!=1 or armatures[0].object!=rig or len(subdivisions)!=1 or
     subdivisions[0].levels!=0 or subdivisions[0].render_levels!=1):raise ValueError('Unexpected upper garment modifier stack')
 if armatures[0].use_deform_preserve_volume or armatures[0].use_multi_modifier:raise ValueError('Upper garment must use ordinary LBS')
 output_dir.mkdir(parents=True,exist_ok=False)
 subdiv=subdivisions[0];previous_level=subdiv.levels;subdiv.levels=subdiv.render_levels if use_render_subdiv else previous_level
 rig.animation_data.action=None;rig.data.pose_position='REST';bpy.context.view_layer.update();deps=bpy.context.evaluated_depsgraph_get()
 mesh=bpy.data.meshes.new_from_object(source.evaluated_get(deps),preserve_all_data_layers=True,depsgraph=deps)
 candidate=source.copy();candidate.data=mesh;candidate.name='Prepared upper garment';bpy.context.collection.objects.link(candidate)
 candidate.modifiers.clear();mod=candidate.modifiers.new('Game compatible LBS','ARMATURE');mod.object=rig
 deform={b.name for b in rig.data.bones if b.use_deform};before={};changed=0;max_removed=0.
 for v in candidate.data.vertices:
  groups=[(candidate.vertex_groups[g.group],g.weight) for g in v.groups if candidate.vertex_groups[g.group].name in deform and g.weight>0]
  before[len(groups)]=before.get(len(groups),0)+1
  if not groups:raise ValueError(f'Unweighted evaluated garment vertex {v.index}')
  keep=sorted(groups,key=lambda pair:-pair[1])[:4];total=sum(w for _,w in keep);removed=sum(w for _,w in groups)-total
  max_removed=max(max_removed,removed)
  if len(groups)>4 or abs(total-1)>1e-7:
   for group,_ in groups:group.remove([v.index])
   for group,w in keep:group.add([v.index],w/total,'REPLACE')
   changed+=1
 collar_count=0
 if collar_clearance:
  # A 1.8mm visible skin protrusion was ray-measured at Walk31 on the
  # positive-X neckline. Shift only the symmetric local collar patches.
  for v in candidate.data.vertices:
   x,y,z=v.co
   if abs(x)<.035 or abs(x)>.12 or abs(y+.008)>.045 or abs(z-1.48)>.045:continue
   influence=math.exp(-((abs(x)-.076)/.025)**2-((y+.008)/.020)**2-((z-1.48)/.022)**2)
   if influence>.01:v.co.x+=math.copysign(.006*influence,x);collar_count+=1
  candidate.data.update()
 rig.data.pose_position='POSE';samples=[]
 def coordinates(obj):
  e=obj.evaluated_get(bpy.context.evaluated_depsgraph_get());m=e.to_mesh()
  try:
   data=np.empty(len(m.vertices)*3,dtype=np.float32);m.vertices.foreach_get('co',data);data=data.reshape((-1,3));mat=np.array(e.matrix_world,dtype=np.float64)
   return data@mat[:3,:3].T+mat[:3,3]
  finally:e.to_mesh_clear()
 for name in ('Walk','Run','Idle','Hit','Death'):
  action=bpy.data.actions.get(name)
  if action is None:raise ValueError('Missing action '+name)
  rig.animation_data.action=action;rig.animation_data.action_slot=action.slots[0];start,end=action.frame_range
  for i in range(9):
   time=start+(end-start)*i/8;frame=int(math.floor(time));scene.frame_set(frame,subframe=time-frame);bpy.context.view_layer.update()
   before_pts=coordinates(source);after_pts=coordinates(candidate)
   if before_pts.shape!=after_pts.shape:raise ValueError('Evaluated subdivision topology differs')
   err=np.linalg.norm(before_pts-after_pts,axis=1)
   if not np.isfinite(err).all():raise ValueError('Nonfinite garment error')
   samples.append({'clip':name,'sourceFrame':time,'vertices':len(err),'maxError':float(err.max()),'p95Error':float(np.quantile(err,.95)),'rmsError':float(np.sqrt(np.mean(err**2)))})
  print('UPPER_COMPARED',name,flush=True)
 if len(rig.animation_data.drivers)!=drivers:raise RuntimeError('Rigify drivers changed')
 report={'input':str(input_path.resolve()),'upperSource':'Human.donitz_monk_robe','subdivisionSourceViewportLevel':previous_level,'frozenLevel':subdiv.levels,'sourceRenderLevel':subdiv.render_levels,'vertices':len(mesh.vertices),'inputInfluenceHistogram':before,'verticesRenormalized':changed,'maxRemovedWeight':max_removed,'collarVerticesAdjusted':collar_count,'drivers':drivers,'samples':samples,'scope':'upper only; same-level, same-index comparison'}
 (output_dir/'upper-comparison.json').write_text(json.dumps(report,indent=2)+'\n')
 subdiv.levels=previous_level
 source.name='Human.donitz_monk_robe [editable source]';candidate.name='Human.donitz_monk_robe'
 source.hide_render=True;source.hide_viewport=True
 rig.animation_data.action=prior_action
 if prior_action is not None and prior_slot is not None:rig.animation_data.action_slot=prior_slot
 rig.data.pose_position=prior_pose;scene.frame_set(prior_frame,subframe=prior_subframe);bpy.context.view_layer.update()
 bpy.ops.wm.save_as_mainfile(filepath=str((output_dir/'upper-prepared.blend').resolve()))
 print('UPPER_PREPARED',json.dumps({'vertices':len(mesh.vertices),'changed':changed,'maxError':max(x['maxError'] for x in samples),'maxRemovedWeight':max_removed}),flush=True)

if '--prepare-upper' in sys.argv:
 parser=argparse.ArgumentParser();parser.add_argument('--prepare-upper',action='store_true');parser.add_argument('--upper-base',action='store_true');parser.add_argument('--collar-clearance',action='store_true');parser.add_argument('--input',type=Path,required=True);parser.add_argument('--output',type=Path,required=True)
 opts=parser.parse_args(sys.argv[sys.argv.index('--')+1:]);prepare_upper_export(opts.input,opts.output,not opts.upper_base,opts.collar_clearance);sys.exit(0)

p=argparse.ArgumentParser();p.add_argument('--input',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--sector-skirt',action='store_true');p.add_argument('--vent',action='store_true');p.add_argument('--remove-inner-skirt',action='store_true');p.add_argument('--fit-thighs',action='store_true');p.add_argument('--split-panels',action='store_true');p.add_argument('--leg-bind',action='store_true')
a=p.parse_args(sys.argv[sys.argv.index('--')+1:]);a.output.mkdir(parents=True,exist_ok=False)
bpy.ops.wm.open_mainfile(filepath=str(a.input.resolve()))
s=bpy.context.scene;rig=bpy.data.objects['VillagerRig'];human=bpy.data.objects['Human'];robe=bpy.data.objects['Human.donitz_monk_robe'];drivers=len(rig.animation_data.drivers)
# Contract the authored cape's broad lower edge into a modest shoulder yoke.
# All component topology and original interpolated rigging are retained.
bm=bmesh.new();bm.from_mesh(robe.data);remaining=set(bm.verts);parts=[]
while remaining:
 stack=[remaining.pop()];part=set()
 while stack:
  v=stack.pop();part.add(v)
  for e in v.link_edges:
   q=e.other_vert(v)
   if q in remaining:remaining.remove(q);stack.append(q)
 parts.append(part)
yoke=next(part for part in parts if len(part)==2304)
for v in yoke:
 v.co.x*=.93
 if v.co.z<1.43:v.co.z=1.43+(v.co.z-1.43)*.72
if a.fit_thighs:
 # The observed step exceeds front/back clearance by 25-34 mm around the
 # thighs. Fit that region only and taper the correction into the waist/hem.
 for v in max(parts,key=len):
  z=v.co.z;t=min(1.,max(0.,(z-.28)/.18),max(0.,(.95-z)/.12))
  v.co.y+=.04*t*max(-1.,min(1.,(v.co.y+.04)/.07))
vent_sides={}
panel_points=None
if a.split_panels:
 if not (a.sector_skirt or a.leg_bind):raise RuntimeError('Split panels require skirt or leg binding')
 main=max(parts,key=len);bm.verts.ensure_lookup_table();bm.verts.index_update()
 main_faces=[f for f in bm.faces if all(v in main for v in f.verts)]
 source=BVHTree.FromPolygons([v.co.copy() for v in bm.verts],[tuple(v.index for v in f.verts) for f in main_faces])
 panel_points={}
 for side,sgn in [('L',1),('R',-1)]:
  pts=[];rows=29;cols=41
  for j in range(rows):
   z=.10+(.93-.10)*j/(rows-1)
   for i in range(cols):
    angle=-math.pi/2+math.pi*i/(cols-1);direction=Vector((sgn*math.cos(angle),math.sin(angle),0));center=Vector((0,-.04,z))
    hit,_,_,_=source.ray_cast(center+direction, -direction,2.)
    if hit is None:raise RuntimeError(f'No authored surface at {side} row {j} column {i}')
    ease=.035*min(1.,max(0.,(.93-z)/.12));pts.append(hit+direction*ease)
  panel_points[side]=pts
 # Replace the old closed lower shell, including its inward liner. The new
 # panels overlap the upper robe by 7cm to form a covered waist seam.
 edges={e for v in main for e in v.link_edges};cut=bmesh.ops.bisect_plane(bm,geom=list(main)+list(edges)+main_faces,dist=.000001,plane_co=(0,0,.86),plane_no=(0,0,1),clear_inner=True,clear_outer=False)
if a.remove_inner_skirt:
 # Source robe has a closed inward-facing liner rising into the legs. Keep
 # the outer authored cloth, removing only the liner and its folded hem join.
 bm.normal_update();inner=[]
 for f in bm.faces:
  c=f.calc_center_median();radial=f.normal.x*c.x+f.normal.y*(c.y+.04)
  if c.z<.74 and (radial<-.0001 or max(v.co.z for v in f.verts)<.105):inner.append(f)
 if not inner:raise RuntimeError('No internal skirt lining found')
 bmesh.ops.delete(bm,geom=inner,context='FACES_ONLY')
 bmesh.ops.delete(bm,geom=[v for v in bm.verts if not v.link_faces],context='VERTS')
if a.vent:
 # Bisect the lower shell at its centre, then split the actual new edges.
 # No cloth strip is discarded, and each new free edge retains panel identity.
 vs=[v for v in bm.verts if v.co.z<.66]
 es=[e for e in bm.edges if all(v.co.z<.66 for v in e.verts)]
 fs=[f for f in bm.faces if all(v.co.z<.66 for v in f.verts)]
 cut=bmesh.ops.bisect_plane(bm,geom=vs+es+fs,dist=.000001,plane_co=(0,0,0),plane_no=(1,0,0),clear_inner=False,clear_outer=False)
 edges=[x for x in cut['geom_cut'] if isinstance(x,bmesh.types.BMEdge)]
 if not edges:raise RuntimeError('No vent edges found')
 bmesh.ops.split_edges(bm,edges=edges)
 bm.verts.index_update()
 for v in bm.verts:
  if v.is_boundary and v.co.z<.66 and abs(v.co.x)<.001:
   centers=[face.calc_center_median().x for face in v.link_faces]
   vent_sides[v.index]=1. if sum(centers)>=0 else 0.
bm.to_mesh(robe.data);bm.free();robe.data.update()
# A small three-level cage replaces the asset's foot/toe-bound hem. Independent
# mid-panel translations follow knee travel while the hem hangs from the pelvis.
# This is authored secondary motion, not a physics simulation.
if a.sector_skirt and not a.leg_bind:
 bpy.ops.object.mode_set(mode='OBJECT') if bpy.context.object and bpy.context.object.mode!='OBJECT' else None
 bpy.ops.object.select_all(action='DESELECT');rig.select_set(True);bpy.context.view_layer.objects.active=rig;bpy.ops.object.mode_set(mode='EDIT')
 for side,sgn in [('L',1),('R',-1)]:
  for panel,ys in [('Front',-1),('Back',1)]:
   for level,z in [('high',.90),('mid',.53),('hem',.12)]:
    bone=rig.data.edit_bones.new(f'SKIRT.{side}.{panel}.{level}');bone.head=(sgn*.09,ys*.08,z);bone.tail=(sgn*.09,ys*.08,z-.08);bone.parent=rig.data.edit_bones['DEF-spine'];bone.use_deform=True
 bpy.ops.object.mode_set(mode='OBJECT')
 for side in ('L','R'):
  for panel in ('Front','Back'):
   for segment in ('high','mid','hem'):robe.vertex_groups.new(name=f'SKIRT.{side}.{panel}.{segment}')
 old_deform={b.name for b in rig.data.bones if b.use_deform and not b.name.startswith('SKIRT.')}
 for v in ([] if a.split_panels else robe.data.vertices):
  z=v.co.z
  if z>=.94:continue
  blend=min(1.,max(0.,(.94-z)/.14));left=vent_sides.get(v.index,min(1.,max(0.,(v.co.x+.10)/.20)));panel='Front' if v.co.y<-.009 else 'Back'
  if z>=.53:
   mid=min(1.,max(0.,(.90-z)/.37));vertical=[('high',1-mid),('mid',mid)]
  else:
   hem=min(1.,max(0.,(.53-z)/.41));vertical=[('mid',1-hem),('hem',hem)]
  old=[(robe.vertex_groups[g.group],g.weight) for g in v.groups if robe.vertex_groups[g.group].name in old_deform and g.weight>0]
  for group,_ in old:group.remove([v.index])
  for group,weight in old:
   if weight*(1-blend)>0:group.add([v.index],weight*(1-blend),'REPLACE')
  for side,weight in [('L',left),('R',1-left)]:
   for segment,factor in vertical:
    w=blend*weight*factor
    if w>0:robe.vertex_groups[f'SKIRT.{side}.{panel}.{segment}'].add([v.index],w,'REPLACE')
 rest={side:(rig.data.bones[f'DEF-shin.{side}'].head_local-rig.data.bones['DEF-spine'].head_local).y for side in ('L','R')}
 for name in ('Walk','Run','Idle','Hit','Death'):
  action=bpy.data.actions.get(name)
  if action is None:continue
  rig.animation_data.action=action;end=int(round(action.frame_range[1]));frames=list(range(1,end+1,4))
  if frames[-1]!=end:frames.append(end)
  travel={}
  for frame in frames:
   s.frame_set(frame);bpy.context.view_layer.update();pelvis=rig.pose.bones['DEF-spine'].head
   travel[frame]={side:(rig.pose.bones[f'DEF-shin.{side}'].head-pelvis).y-rest[side] for side in ('L','R')}
  for frame in frames:
   s.frame_set(frame)
   for side in ('L','R'):
    displacement=max(-.28,min(.28,travel[frame][side]))
    for panel in ('Front','Back'):
     for segment,factor in [('high',0.),('mid',1.),('hem',0.)]:
      bone=rig.pose.bones[f'SKIRT.{side}.{panel}.{segment}'];local=bone.bone.matrix_local.to_3x3().inverted()@Vector((0,displacement*factor,0));bone.location=local;bone.keyframe_insert(data_path='location',frame=frame,group='Skirt '+side+' '+panel)
 rig.animation_data.action=bpy.data.actions.get('Walk');s.frame_set(1);bpy.context.view_layer.update()
# Match the single-LBS game limit without altering non-deform garment groups.
deform={b.name for b in rig.data.bones if b.use_deform};normalized=0
for v in robe.data.vertices:
 influences=[(robe.vertex_groups[g.group],g.weight) for g in v.groups if robe.vertex_groups[g.group].name in deform and g.weight>0]
 if not influences:continue
 keep=sorted(influences,key=lambda pair:-pair[1])[:4];total=sum(weight for _,weight in keep)
 for group,_ in influences:group.remove([v.index])
 for group,weight in keep:group.add([v.index],weight/total,'REPLACE')
 normalized+=1
def material(name,color,roughness):
 m=bpy.data.materials.new(name);m.diffuse_color=(*color,1);m.use_nodes=True;b=m.node_tree.nodes.get('Principled BSDF');b.inputs['Base Color'].default_value=(*color,1);b.inputs['Roughness'].default_value=roughness;return m
ivory=material('Ivory woven wrap',(.72,.70,.60),.95);leather=material('Brown leather footwear',(.16,.065,.026),.82)
eye_white=material('Eye white',(.66,.62,.54),.55);iris=material('Dark brown eyes',(.028,.012,.006),.48)
robe_mat=material('Olive ochre robe',(.24,.19,.048),.94);robe.data.materials.clear();robe.data.materials.append(robe_mat)
skin=material('Warm tan skin',(.36,.19,.105),.75);human.data.materials.clear();human.data.materials.append(skin)
# Separate fitted trouser geometry, not a material change on the human. Bake
# only the male shape in rest space, remove helper/upper-body regions, then
# offset the cloth surface from the legs and retain the body's rig weights.
states=[m.show_viewport for m in human.modifiers]
for m in human.modifiers:m.show_viewport=False
dg=bpy.context.evaluated_depsgraph_get();bpy.context.view_layer.update()
pants=human.copy();pants.data=bpy.data.meshes.new_from_object(human.evaluated_get(dg),preserve_all_data_layers=True,depsgraph=dg);pants.name='Separate olive trousers';bpy.context.collection.objects.link(pants)
for m,state in zip(human.modifiers,states):m.show_viewport=state
for m in list(pants.modifiers):pants.modifiers.remove(m)
pb=bmesh.new();pb.from_mesh(pants.data);pb.verts.ensure_lookup_table();deform_layer=pb.verts.layers.deform.active;body_group=human.vertex_groups['body'].index
remove=[v for v in pb.verts if v.co.z<.055 or v.co.z>.96 or abs(v.co.x)>.34 or not deform_layer or v[deform_layer].get(body_group,0)<.5]
bmesh.ops.delete(pb,geom=remove,context='VERTS');pb.normal_update()
for v in pb.verts:v.co+=v.normal*.014
pb.to_mesh(pants.data);pb.free();pants.data.update();pants.data.materials.clear();pants.data.materials.append(robe_mat)
for face in pants.data.polygons:face.use_smooth=True
pm=pants.modifiers.new('Villager rig','ARMATURE');pm.object=rig
def mesh_obj(name,verts,faces,mat,bone):
 me=bpy.data.meshes.new(name);me.from_pydata(verts,[],faces);me.update();ob=bpy.data.objects.new(name,me);bpy.context.collection.objects.link(ob);ob.data.materials.append(mat)
 for poly in me.polygons:poly.use_smooth=True
 vg=ob.vertex_groups.new(name=bone);vg.add(list(range(len(verts))),1.,'REPLACE');mod=ob.modifiers.new('Villager rig','ARMATURE');mod.object=rig
 return ob
if panel_points is not None:
 rows=29;cols=41
 def weighted_tree(obj,allowed):
  samples=[]
  for v in obj.data.vertices:
   w={obj.vertex_groups[g.group].name:g.weight for g in v.groups if obj.vertex_groups[g.group].name in allowed and g.weight>0}
   if w:samples.append((v.co.copy(),w))
  if not samples:raise RuntimeError('No weights for '+obj.name)
  tree=KDTree(len(samples))
  for index,(co,_) in enumerate(samples):tree.insert(co,index)
  tree.balance();return tree,samples
 def interpolate(tree,samples,co):
  out={}
  for _,index,distance in tree.find_n(co,min(8,len(samples))):
   factor=1/max(.01,distance)**2
   for name,w in samples[index][1].items():out[name]=out.get(name,0)+w*factor
  total=sum(out.values());return {n:w/total for n,w in out.items()}
 if a.leg_bind:
  leg_trees={side:weighted_tree(pants,{b.name for b in rig.data.bones if b.use_deform and (b.name.startswith(f'DEF-thigh.{side}') or b.name.startswith(f'DEF-shin.{side}'))}) for side in ('L','R')}
  bodice_tree,bodice_samples=weighted_tree(robe,{b.name for b in rig.data.bones if b.use_deform})
 for side,pts in panel_points.items():
  faces=[]
  for j in range(rows-1):
   for i in range(cols-1):
    q=j*cols+i;f=(q,q+1,q+1+cols,q+cols);faces.append(f if side=='L' else tuple(reversed(f)))
  ob=mesh_obj('Independent robe panel '+side,pts,faces,robe_mat,'DEF-spine' if a.leg_bind else f'SKIRT.{side}.Front.high')
  ob.vertex_groups.clear()
  if a.leg_bind:
   for name in {b.name for b in rig.data.bones if b.use_deform}:ob.vertex_groups.new(name=name)
  else:
   for panel in ('Front','Back'):
    for level in ('high','mid','hem'):ob.vertex_groups.new(name=f'SKIRT.{side}.{panel}.{level}')
  for j in range(rows):
   z=.10+(.93-.10)*j/(rows-1)
   if z>=.53:
    t=min(1.,max(0.,(.90-z)/.37));levels=[('high',1-t),('mid',t)]
   else:
    t=min(1.,max(0.,(.53-z)/.41));levels=[('mid',1-t),('hem',t)]
   for i in range(cols):
    panel='Front' if i<cols//2 else 'Back';idx=j*cols+i
    if a.leg_bind:
     tree,samples=leg_trees[side];leg=interpolate(tree,samples,pts[idx]);upper=interpolate(bodice_tree,bodice_samples,pts[idx]);blend=min(1.,max(0.,(z-.84)/.09));weights={}
     for name,w in leg.items():weights[name]=weights.get(name,0)+w*(1-blend)
     for name,w in upper.items():weights[name]=weights.get(name,0)+w*blend
     chosen=sorted(weights.items(),key=lambda item:-item[1])[:4];total=sum(w for _,w in chosen)
     for name,w in chosen:
      if w>0:ob.vertex_groups[name].add([idx],w/total,'REPLACE')
    else:
     for level,w in levels:
      if w>0:ob.vertex_groups[f'SKIRT.{side}.{panel}.{level}'].add([idx],w,'REPLACE')
  sub=ob.modifiers.new('Soft woven panels','SUBSURF');sub.levels=1;sub.render_levels=1
  thick=ob.modifiers.new('Hem and cloth thickness','SOLIDIFY');thick.thickness=.003
def ellipsoid(name,center,radii,mat,bone):
 verts=[];faces=[];n=48;rows=16
 for j in range(rows+1):
  theta=math.pi*j/rows
  for i in range(n):
   t=2*math.pi*i/n;verts.append((center[0]+radii[0]*math.sin(theta)*math.cos(t),center[1]+radii[1]*math.sin(theta)*math.sin(t),max(.003,center[2]+radii[2]*math.cos(theta))))
 for j in range(rows):
  for i in range(n):faces.append((j*n+i,j*n+(i+1)%n,(j+1)*n+(i+1)%n,(j+1)*n+i))
 return mesh_obj(name,verts,faces,mat,bone)
# MPFB masks its eyeball helpers; add visible eyes at their measured bind centers.
for side,label in [(1,'L'),(-1,'R')]:
 ellipsoid('Eye '+label,(side*.0294,-.1263,1.6205),(.0147,.0159,.014),eye_white,'DEF-spine.006')
 ellipsoid('Iris '+label,(side*.0294,-.1416,1.6205),(.0044,.0018,.0044),iris,'DEF-spine.006')
# Cloth cap plus overlapping soft elliptical wrap bands, independent from head.
ellipsoid('Turban crown',(0,-.048,1.705),(.097,.112,.083),ivory,'DEF-spine.006')
for band in range(3):
 verts=[];faces=[];n=64;k=10;z=1.666+band*.025
 for i in range(n):
  t=2*math.pi*i/n;center=Vector((.091*math.cos(t),-.048+.103*math.sin(t),z+.013*math.cos(t+band*.9)))
  outward=Vector((math.cos(t),math.sin(t),0))
  for j in range(k):
   u=2*math.pi*j/k;v=center+outward*(.007*math.cos(u))+Vector((0,0,.014*math.sin(u)));verts.append(v)
 for i in range(n):
  for j in range(k):faces.append((i*k+j,((i+1)%n)*k+j,((i+1)%n)*k+(j+1)%k,i*k+(j+1)%k))
 mesh_obj('Turban wrap %d'%(band+1),verts,faces,ivory,'DEF-spine.006')
# Two tapered cloth falls flank the face and drape to the upper chest.
for side,label in [(-1,'Right'),(1,'Left')]:
 verts=[];faces=[];rows=9;cols=6
 for j in range(rows):
  t=j/(rows-1);z=1.665-.215*t
  for i in range(cols):
   u=i/(cols-1);x=side*(.063-.031*t+.032*u);y=-.075-.045*t+.009*math.sin(math.pi*u)
   verts.append((x,y,z+.004*math.sin(u*math.pi)))
 for j in range(rows-1):
  for i in range(cols-1):q=j*cols+i;faces.append((q,q+1,q+1+cols,q+cols))
 ob=mesh_obj(label+' turban drape',verts,faces,ivory,'DEF-spine.006');mod=ob.modifiers.new('Cloth thickness','SOLIDIFY');mod.thickness=.003
# The back fall covers the nape; a small tapered front fall sits below the chin.
for name,back in [('Nape wrap',True),('Front neck wrap',False)]:
 verts=[];faces=[];rows=8;cols=9
 for j in range(rows):
  t=j/(rows-1)
  for i in range(cols):
   u=2*i/(cols-1)-1
   if back:
    x=u*(.086-.02*t);y=.045+.018*t+.006*(1-u*u);z=1.65-.20*t
   else:
    x=u*(.052-.025*t);y=-.117-.012*t;z=1.49-.085*t+.025*abs(u)
   verts.append((x,y,z))
 for j in range(rows-1):
  for i in range(cols-1):q=j*cols+i;faces.append((q,q+1,q+1+cols,q+cols))
 ob=mesh_obj(name,verts,faces,ivory,'DEF-spine.006');mod=ob.modifiers.new('Cloth thickness','SOLIDIFY');mod.thickness=.003
# Simple closed shoes, separately weighted to each foot; no garment-foot bridge.
for side,label in [(1,'L'),(-1,'R')]:
 verts=[];faces=[];n=48
 for z,scale in [(.003,1.),(.025,1.),(.050,.98),(.075,.78),(.09,.38)]:
  for i in range(n):
   t=2*math.pi*i/n;c=math.cos(t);q=math.sin(t)
   verts.append((side*.204+.058*scale*math.copysign(abs(c)**.7,c),-.080+.145*scale*math.copysign(abs(q)**.7,q),z))
 for row in range(4):
  for i in range(n):faces.append((row*n+i,row*n+(i+1)%n,(row+1)*n+(i+1)%n,(row+1)*n+i))
 faces.append(tuple(reversed(range(n))));faces.append(tuple(4*n+i for i in range(n)))
 mesh_obj('Brown shoe '+label,verts,faces,leather,'DEF-foot.'+label)
s.render.engine='BLENDER_WORKBENCH';s.display.shading.light='STUDIO';s.display.shading.color_type='MATERIAL';s.display.shading.show_cavity=True;s.render.resolution_x=640;s.render.resolution_y=800;s.render.resolution_percentage=100
cam=s.camera;center=Vector((0,0,.9))
if drivers!=len(rig.animation_data.drivers):raise RuntimeError('Rigify drivers changed during garment build')
bpy.ops.wm.save_as_mainfile(filepath=str((a.output/'dressed-master.blend').resolve()))
for angle,pos in [(0,(0,-5,1.5)),(90,(5,0,1.5)),(180,(0,5,1.5)),(270,(-5,0,1.5))]:
 cam.location=pos;cam.rotation_euler=(center-cam.location).to_track_quat('-Z','Y').to_euler();s.render.filepath=str((a.output/f'view-{angle:03d}.png').resolve());bpy.ops.render.render(write_still=True)
(a.output/'report.json').write_text(json.dumps({'input':str(a.input.resolve()),'rigDrivers':drivers,'mask':'Delete.donitz_monk_robe retained','garment':'CC0 Donitz robe, authored mask; fitted yoke','normalizedGarmentVertices':normalized,'maxGarmentInfluences':4,'status':'static accessory candidate, no motion acceptance'},indent=2)+'\n')
