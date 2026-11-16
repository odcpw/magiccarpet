"""Blender round-trip check for the owner-local world pack and its sources."""
import hashlib
import json
import math
import struct
from pathlib import Path

import bpy
import yaml
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[3]
PACK = ROOT / 'research/private/generated/browser-level1-assets/world'
RES = ROOT / 'research/private/remc2-unity-native/Assets/Resources'
SOURCES = {
    'spell-jar': 'ModernCastleSpellJar/CastleSpellJar.fbx',
    'experience-scroll': 'ModernExperienceScroll/ExperienceScroll.fbx',
    'exit-portal': 'ModernCharacters/ExitPortal/ExitPortal.fbx',
    'stone-statue': 'ModernScenery/StoneStatue.fbx',
    'dolmen': 'ModernScenery/Dolmen.fbx',
    'castle-head': 'ModernCharacters/CastleProjectile/CastleProjectile.fbx',
}


def bounds():
    points = [obj.matrix_world @ Vector(corner) for obj in bpy.context.scene.objects
              if obj.type == 'MESH' for corner in obj.bound_box]
    assert points and all(math.isfinite(v) for point in points for v in point)
    return [[min(p[i] for p in points), max(p[i] for p in points)] for i in range(3)]


manifest = json.loads((PACK / 'manifest.json').read_text())
assert manifest['schema'] == 'mc-browser-modern-world/v1'
for name, info in manifest['files'].items():
    data = (PACK / name).read_bytes()
    assert len(data) == info['bytes'] and hashlib.sha256(data).hexdigest() == info['sha256'], name
for name, source in SOURCES.items():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.fbx(filepath=str(RES / source))
    before = bounds()
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(PACK / (name + '.glb')))
    after = bounds()
    assert all(abs(a-b) < .0001 for axis1, axis2 in zip(before, after)
               for a, b in zip(axis1, axis2)), (name, before, after)
    materials = [m for m in bpy.data.materials if m.users]
    assert materials and all(m.use_nodes for m in materials), name
    if name in ('experience-scroll', 'stone-statue', 'dolmen'):
        assert any(n.type == 'TEX_IMAGE' and n.image for m in materials
                   for n in m.node_tree.nodes), name
    print('WORLD_ROUNDTRIP', name, 'bounds', after, 'materials', len(materials))

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=str(PACK / 'authored-tree.glb'))
parts = {o.name: o for o in bpy.context.scene.objects if o.type == 'MESH'}
assert set(parts) == {'tree_trunk', 'tree_branches', 'tree_leaves'}, parts.keys()
tree_bounds = bounds()
assert tree_bounds[2][1] > 1 and abs(tree_bounds[2][0]) < .01, tree_bounds
for part in parts.values():
    assert part.data.polygons and part.data.uv_layers and part.data.materials
    assert any(node.type == 'TEX_IMAGE' and node.image for node in part.data.materials[0].node_tree.nodes)
print('WORLD_TREE', tree_bounds, {name:len(obj.data.polygons) for name,obj in parts.items()})

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=str(PACK / 'nature.glb'))
parts = {o.name: o for o in bpy.context.scene.objects if o.type == 'MESH'}
assert set(parts) == {'nature_grass','nature_fern','nature_shrub','nature_tree_trunk',
                      'nature_rock','nature_tree_branches','nature_tree_leaves'}, parts.keys()
for part in parts.values():
    assert part.data.polygons and part.data.uv_layers and part.data.materials
    assert any(node.type == 'TEX_IMAGE' and node.image for node in part.data.materials[0].node_tree.nodes)
print('WORLD_NATURE', bounds(), {name:len(obj.data.polygons) for name,obj in parts.items()})
yaml.SafeLoader.add_constructor('tag:unity3d.com,2011:43',
                                lambda loader, node: loader.construct_mapping(node, deep=True))
source = yaml.safe_load((RES / 'ModernNature/rock_mid.asset').read_text())['Mesh']
raw = bytes.fromhex(source['m_VertexData']['_typelessdata'])
x,y,z = struct.unpack_from('<3f',raw)
expected = Vector((x,-z,y))
assert min((vertex.co-expected).length for vertex in parts['nature_rock'].data.vertices) < 1e-5
print('WORLD_NATURE_AXIS', tuple(expected))
