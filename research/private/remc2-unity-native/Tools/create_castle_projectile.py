"""Build a source-inspired Castle projectile from the local human master.

Run `blender -b --factory-startup --python <script> -- <empty-output-dir>`.
The source human and its approved player assets remain untouched. The .blend
is an editable local master; both FBX files derive from the same head.
"""
import math
import sys
from pathlib import Path

import bmesh
import bpy
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[4]
HUMAN = ROOT / 'research/private/generated/human-figure-20261109/final05/human-master.blend'
out = Path(sys.argv[sys.argv.index('--') + 1]).resolve()
out.mkdir(parents=True, exist_ok=True)
master = out / 'CastleProjectile-master.blend'
modern, voxel = out / 'CastleProjectile.fbx', out / 'CastleProjectileVoxel.fbx'
fabric = out / 'CastleProjectileFabric.png'
if any(path.exists() for path in (master, modern, voxel, fabric)):
    raise RuntimeError('output exists; preserve it and use a new directory')

bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
with bpy.data.libraries.load(str(HUMAN), link=False) as (source, target):
    if 'cm_vitruvian' not in source.meshes:
        raise RuntimeError('human master mesh is absent')
    target.meshes = ['cm_vitruvian']
human = target.meshes[0]


def new_material(name, color):
    material = bpy.data.materials.new(name)
    material.diffuse_color = (*color, 1)
    return material

skin_mat = new_material('Skin', (.53, .32, .22))
beard_mat = new_material('Beard', (.075, .032, .018))
wrap_mat = new_material('Wrap', (.09, .28, .26))
white_mat = new_material('Sclera', (.72, .68, .58))
eye_mat = new_material('Pupils', (.045, .025, .015))


def create_head():
    mesh = human.copy()
    obj = bpy.data.objects.new('Face', mesh)
    bpy.context.scene.collection.objects.link(obj)
    if obj.data.shape_keys: obj.shape_key_clear()
    bm = bmesh.new(); bm.from_mesh(mesh)
    bmesh.ops.bisect_plane(bm, geom=list(bm.verts) + list(bm.edges) + list(bm.faces),
                           dist=.00001, plane_co=(0, 0, 1.53), plane_no=(0, 0, 1), clear_inner=True)
    # Close the cut as a tapered underside rather than an open flat neck.
    boundary = [edge for edge in bm.edges if edge.is_boundary and all(abs(v.co.z - 1.53) < .0001 for v in edge.verts)]
    if not boundary:
        raise RuntimeError('expected neck boundary was not found')
    extruded = bmesh.ops.extrude_edge_only(bm, edges=boundary)['geom']
    newverts = [v for v in extruded if isinstance(v, bmesh.types.BMVert)]
    newedges = [e for e in extruded if isinstance(e, bmesh.types.BMEdge) and all(v in newverts for v in e.verts)]
    for vert in newverts:
        vert.co.x *= .65
        vert.co.y = .025 + (vert.co.y - .025) * .65
        vert.co.z -= .012
    bmesh.ops.holes_fill(bm, edges=newedges, sides=0)
    bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
    bm.to_mesh(mesh); bm.free()
    mesh.materials.clear(); mesh.materials.append(skin_mat)
    for polygon in mesh.polygons: polygon.use_smooth = True
    return obj


face = create_head()
# Beard follows the actual jaw surface. A thin solidified shell closes its
# boundary, with higher sideburns and a small moustache above the lips.
beard = bpy.data.objects.new('Beard', face.data.copy())
bpy.context.scene.collection.objects.link(beard)
bm = bmesh.new(); bm.from_mesh(beard.data)
remove = []
for polygon in bm.faces:
    c = polygon.calc_center_median()
    outer = polygon.normal.y < -.08 or abs(c.x) > .055
    jaw = outer and c.y < .025 and (c.z < 1.578 or (abs(c.x) > .055 and c.z < 1.63))
    moustache = outer and c.y < -.060 and abs(c.x) < .031 and 1.586 < c.z < 1.594
    if not (jaw or moustache): remove.append(polygon)
bmesh.ops.delete(bm, geom=remove, context='FACES')
bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context='VERTS')
for vert in bm.verts: vert.co += vert.normal * .0015
bm.to_mesh(beard.data); bm.free()
beard.data.materials.clear(); beard.data.materials.append(beard_mat)
for polygon in beard.data.polygons: polygon.use_smooth = True
bpy.context.view_layer.objects.active = beard
modifier = beard.modifiers.new('Closed beard shell', 'SOLIDIFY'); modifier.thickness = .002; modifier.offset = 0
bpy.ops.object.modifier_apply(modifier=modifier.name)
modifier = beard.modifiers.new('Compact beard shell', 'DECIMATE'); modifier.ratio = .11
bpy.ops.object.modifier_apply(modifier=modifier.name)
bpy.ops.object.select_all(action='DESELECT'); face.select_set(True); bpy.context.view_layer.objects.active = face
modifier = face.modifiers.new('Compact anatomical surface', 'DECIMATE'); modifier.ratio = .24
bpy.ops.object.modifier_apply(modifier=modifier.name)


def sphere(name, location, scale, material, segments=32, rings=16):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=segments, ring_count=rings, location=location)
    obj = bpy.context.object; obj.name = name; obj.scale = scale
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    obj.data.materials.append(material)
    for polygon in obj.data.polygons: polygon.use_smooth = True
    return obj


# A low, rounded cap gives the original teal silhouette without a pointed hat.
wrap = sphere('Wrap', (0, .025, 1.685), (.088, .105, .071), wrap_mat, 40, 20)


def pair(name, material, positions, scale):
    parts = [sphere(name, position, scale, material, 16, 8) for position in positions]
    bpy.ops.object.select_all(action='DESELECT')
    for obj in parts: obj.select_set(True)
    bpy.context.view_layer.objects.active = parts[0]; bpy.ops.object.join()
    parts[0].name = name
    return parts[0]


# Seat shallow globes just outside the actual socket surface. A fixed depth
# would bury them if the anatomical mesh or its reduction changed.
eye_positions = []
for x in (-.030, .030):
    hit, point, _, _ = face.ray_cast(Vector((x, -1, 1.627)), Vector((0, 1, 0)))
    if not hit:
        raise RuntimeError('eye socket surface is absent')
    eye_positions.append((x, point.y - .00025, 1.627))
sclera = pair('Sclera', white_mat, eye_positions, (.010, .0015, .0035))
pupils = pair('Pupils', eye_mat, [(x, y - .0015, z) for x, y, z in eye_positions], (.0028, .001, .0028))
brows = pair('Brows', beard_mat, [(-.030, -.061, 1.643), (.030, -.061, 1.643)], (.016, .0025, .0035))
parts = (face, beard, wrap, sclera, pupils, brows)
bpy.ops.wm.save_as_mainfile(filepath=str(master))

# Small, neutral woven texture modulates the teal tint without color patches.
image = bpy.data.images.new('Subtle woven cloth', width=128, height=128, alpha=False)
values = []
for y in range(128):
    for x in range(128):
        value = .91 + .035 * math.sin(math.tau * x / 8) * math.sin(math.tau * y / 8)
        values.extend((value, value, value, 1))
image.pixels[:] = values; image.filepath_raw = str(fabric); image.file_format = 'PNG'; image.save()


def export(path, objects):
    bpy.ops.object.select_all(action='DESELECT')
    for obj in objects: obj.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    bpy.ops.export_scene.fbx(filepath=str(path), use_selection=True, object_types={'MESH'},
                             add_leaf_bones=False, bake_anim=False, axis_forward='-Z', axis_up='Y')


export(modern, parts)
voxel_parts = []
for original in parts:
    obj = original.copy(); obj.data = original.data.copy(); obj.name = original.name + ' Voxel'
    bpy.context.scene.collection.objects.link(obj)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.select_all(action='DESELECT'); obj.select_set(True)
    modifier = obj.modifiers.new('Fine block surface', 'REMESH')
    modifier.mode = 'BLOCKS'; modifier.octree_depth = 6 if original in (face, beard, wrap) else 4
    modifier.scale = .95; modifier.use_remove_disconnected = False
    bpy.ops.object.modifier_apply(modifier=modifier.name)
    voxel_parts.append(obj)
export(voxel, voxel_parts)
for obj in (*parts, *voxel_parts):
    print(f'CASTLE_PROJECTILE_PART name={obj.name} vertices={len(obj.data.vertices)} faces={len(obj.data.polygons)}')
