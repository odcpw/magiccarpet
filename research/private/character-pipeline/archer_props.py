"""Keep the selected Meshy-grip archer's modeled quiver rigid on the back.

The quiver and body form one continuous, UV-split mesh. This bounded bind-space
mask is specific to the selected 1.78-unit Meshy-grip character, not a generic
segmentation method. A narrow transition joins the quiver to surrounding skin.
"""
import numpy as np

def smooth(a,b,x):
 t=max(0.,min(1.,(x-a)/(b-a)))
 return t*t*(3-2*t)


def rigid_quiver(mesh):
 spine=mesh.vertex_groups.get('Spine')
 if spine is None:raise ValueError('Missing Spine group')
 # The inner side of the upper tube lies close to pale neck cloth. Use the
 # existing base-color texture only for this narrow extension of the mask.
 images=[n.image for mat in mesh.data.materials if mat and mat.use_nodes
         for n in mat.node_tree.nodes if n.type=='TEX_IMAGE' and n.image
         and n.image.colorspace_settings.name=='sRGB']
 if not images or not mesh.data.uv_layers.active:raise ValueError('Missing quiver texture or UVs')
 image=images[0];width_px,height_px=image.size
 rgba=np.empty(width_px*height_px*4,dtype=np.float32);image.pixels.foreach_get(rgba)
 rgba=rgba.reshape((height_px,width_px,4));uv=mesh.data.uv_layers.active.data
 brown=set()
 for loop in mesh.data.loops:
  u,v=uv[loop.index].uv
  x=int((u%1)*(width_px-1));y=int((v%1)*(height_px-1))
  r,g,b=rgba[y,x,:3]
  if r<.65 and r-g>.07 and g-b>.008:brown.add(loop.vertex_index)
 factors={}
 for v in mesh.data.vertices:
  p=mesh.matrix_world@v.co
  if not 1.10<p.z<1.78:continue
  # The bind-space tube is tilted over the right shoulder.
  center_x=-.12-.29*(p.z-1.18)
  width=1-smooth(.050,.070,abs(p.x-center_x))
  # Fletching projects forward from the top of the rear tube.
  upper=smooth(1.44,1.52,p.z)
  rear=smooth(.095-.025*upper,.125-.030*upper,p.y)
  ends=smooth(1.10,1.14,p.z)*(1-smooth(1.74,1.78,p.z))
  factor=width*rear*ends
  if v.index in brown:
   extension=smooth(1.38,1.46,p.z)*smooth(.065,.095,p.y)
   extension*=1-smooth(.090,.110,abs(p.x-center_x))
   factor=max(factor,extension*ends)
  if factor>.001:
   # UV seams can duplicate the same geometric point with different texels.
   # All copies must share a mask or the seam tears when the head turns.
   position=tuple(round(float(x),6) for x in p)
   factors[position]=max(factor,factors.get(position,0.))
 changed=core=0
 for v in mesh.data.vertices:
  p=mesh.matrix_world@v.co
  factor=factors.get(tuple(round(float(x),6) for x in p),0.)
  if factor<.001:continue
  memberships={g.group:g.weight for g in v.groups}
  moved=0.
  for idx,weight in memberships.items():
   if idx==spine.index:continue
   remaining=weight*(1-factor)
   if remaining>1e-7:mesh.vertex_groups[idx].add([v.index],remaining,'REPLACE')
   else:mesh.vertex_groups[idx].remove([v.index])
   moved+=weight*factor
  if moved:
   spine.add([v.index],memberships.get(spine.index,0.)+moved,'REPLACE')
   # glTF permits four influences by default. Perform its top-four reduction
   # explicitly so the in-Blender and exported skins use the same weights.
   current=sorted(((g.group,g.weight) for g in v.groups if g.weight>1e-7),
                  key=lambda item:item[1],reverse=True)
   if len(current)>4:
    for idx,_ in current[4:]:mesh.vertex_groups[idx].remove([v.index])
    total=sum(weight for _,weight in current[:4])
    for idx,weight in current[:4]:mesh.vertex_groups[idx].add([v.index],weight/total,'REPLACE')
   changed+=1
   if factor>.99:core+=1
 if not 100<core<4000:raise ValueError(f'Unexpected quiver mask: {core} core vertices')
 print('RIGID_QUIVER',changed,'vertices;',core,'rigid core',flush=True)
 return changed,core
