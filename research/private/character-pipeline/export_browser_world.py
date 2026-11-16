#!/usr/bin/env python3
"""Export owner-local Modern Level 1 art. Run with Blender --background --python.

The BUILD profiles are the approved Unity procedural mesh inputs, not baked
Unity resources. The browser builds those meshes using ModernBuildingMeshes'
facet and window rules. The frozen opening layout is a visual-only stamp
contract and never modifies native terrain. Outputs are create-once: remove or
move an obsolete pack explicitly before producing another one.
"""
import hashlib
import json
import os
from pathlib import Path
import struct
import tempfile
import subprocess

import bpy
import yaml
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[3]
RES = ROOT / 'research/private/remc2-unity-native/Assets/Resources'
GEN = ROOT / 'research/private/generated'
OUTPUT = GEN / 'browser-level1-assets/world'
LAYOUT = GEN / 'level1-assets-20261110/buildings/level1-source-layout.json'
LIBRARY = GEN / 'level1-assets-20261115/buildings/source-art-library-v2.json'
SOURCE_CACHE = GEN / 'level1-unity-asset-cache-plan28-cp4-correction3'
# These are the same accepted imports selected by the Unity presenters.
MODELS = {
    'spell-jar': 'ModernCastleSpellJar/CastleSpellJar.fbx',
    'experience-scroll': 'ModernExperienceScroll/ExperienceScroll.fbx',
    'exit-portal': 'ModernCharacters/ExitPortal/ExitPortal.fbx',
    'stone-statue': 'ModernScenery/StoneStatue.fbx',
    'dolmen': 'ModernScenery/Dolmen.fbx',
    'castle-head': 'ModernCharacters/CastleProjectile/CastleProjectile.fbx',
}
COLORS = {
    'Jar silver': ((.48,.51,.56,1), .68, .28),
    'Jar dark rim': ((.18,.19,.21,1), .60, .36),
    'Jar red lid': ((.62,.012,.018,1), .20, .30),
    'Scroll parchment': ((.97,.87,.72,1), 0, .85),
    'Scroll parchment overlay': ((1,.96,.86,1), 0, .85),
    'Scroll parchment lip': ((1,.98,.88,1), 0, .85),
    'Scroll violet binding': ((.32,.10,.30,1), 0, .85),
    'Scroll rolled edge': ((.44,.28,.14,1), 0, .85),
    'Blue basalt': ((.105,.16,.19,1), 0, .88),
    'Mouth darkness': ((.004,.003,.003,1), 0, 1),
    'Ember eyes': ((.75,.018,.009,1), 0, .4),
    'Weathered bronze': ((.21,.14,.075,1), 0, .8),
    'Blue grey weathered stone': ((.42,.51,.57,1), 0, .92),
    'Weathered brown grey stone': ((.84,.86,.91,1), 0, .92),
    'Skin': ((.53,.32,.22,1), 0, .8),
    'Wrap': ((.09,.28,.26,1), 0, .8),
    'Beard': ((.075,.032,.018,1), 0, .8),
    'Sclera': ((.72,.68,.58,1), 0, .8),
    'Pupils': ((.045,.025,.015,1), 0, .8),
}
TEXTURES = {
    'Scroll parchment': 'ModernExperienceScroll/ExperienceScrollPaper',
    'Scroll parchment overlay': 'ModernExperienceScroll/ExperienceScrollPaper',
    'Scroll parchment lip': 'ModernExperienceScroll/ExperienceScrollPaper',
    'Blue grey weathered stone': 'ModernScenery/StoneStatueAlbedo',
    'Weathered brown grey stone': 'ModernTextures/RockAlbedo',
    'Wrap': 'ModernCharacters/CastleProjectile/CastleProjectileFabric',
}

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def texture_path(stem):
    for ext in ('.png', '.jpg', '.jpeg'):
        p = RES / (stem + ext)
        if p.exists():
            return p
    raise ValueError('missing texture ' + stem)


def materials():
    for mat in bpy.data.materials:
        name = mat.name.split('.')[0]
        if name not in COLORS:
            raise ValueError('unrecognised source material: ' + mat.name)
        color, metallic, roughness = COLORS[name]
        mat.use_nodes = True
        shader = mat.node_tree.nodes.get('Principled BSDF')
        shader.inputs['Base Color'].default_value = color
        shader.inputs['Metallic'].default_value = metallic
        shader.inputs['Roughness'].default_value = roughness
        if name == 'Ember eyes':
            shader.inputs['Emission Color'].default_value = color
            shader.inputs['Emission Strength'].default_value = 1.2
        if name in TEXTURES:
            image = mat.node_tree.nodes.new('ShaderNodeTexImage')
            image.image = bpy.data.images.load(str(texture_path(TEXTURES[name])), check_existing=True)
            image.image.pack()
            # Preserve tint as a material factor in the exported asset.
            mat.node_tree.links.new(image.outputs['Color'], shader.inputs['Base Color'])


def export_model(source, destination):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.fbx(filepath=str(source))
    meshes = [obj for obj in bpy.context.scene.objects if obj.type == 'MESH']
    if not meshes or any(not obj.data.polygons for obj in meshes):
        raise ValueError('empty model ' + str(source))
    materials()
    bpy.ops.export_scene.gltf(filepath=str(destination), export_format='GLB',
                              export_animations=False, export_apply=False)


def export_tree(destination, scratch):
    """Reproduce ModernAuthoredTrees' shared mid meshes and root anchoring.

    Canopy rotation copies are instanced by the browser; the GLB stores one
    copy per part. Unity's saved mesh data is parsed, not reinterpreted as FBX.
    """
    yaml.SafeLoader.add_constructor('tag:unity3d.com,2011:43',
                                    lambda loader, node: loader.construct_mapping(node, deep=True))
    nature = RES / 'ModernNature'
    meshes = []
    for part in ('trunk', 'branches', 'leaves'):
        path = nature / ('tree_' + part + '_mid.asset')
        data = yaml.safe_load(path.read_text())['Mesh']
        vertex_data = data['m_VertexData']
        channels = vertex_data['m_Channels']
        if (data['m_IndexFormat'] != 0 or len(data['m_SubMeshes']) != 1 or
                [(c['offset'], c['format'], c['dimension']) for c in channels if c['dimension']] !=
                [(0, 0, 3), (12, 0, 3), (24, 0, 4), (40, 0, 4), (56, 0, 2)]):
            raise ValueError('unexpected Unity mesh format: ' + str(path))
        raw = bytes.fromhex(vertex_data['_typelessdata'])
        count = vertex_data['m_VertexCount']
        if len(raw) != count * 64:
            raise ValueError('unexpected Unity mesh stride')
        verts = [struct.unpack_from('<3f', raw, i*64) for i in range(count)]
        normals = [struct.unpack_from('<3f', raw, i*64+12) for i in range(count)]
        uv = [struct.unpack_from('<2f', raw, i*64+56) for i in range(count)]
        indices = bytes.fromhex(data['m_IndexBuffer'])
        indices = struct.unpack('<' + 'H'*(len(indices)//2), indices)
        sub = data['m_SubMeshes'][0]
        if sub['firstByte'] != 0 or sub['indexCount'] != len(indices) or len(indices)%3:
            raise ValueError('unexpected Unity mesh indices')
        meshes.append((part, path, verts, normals, uv, indices))
    trunk = meshes[0][2]
    minimum = min(v[1] for v in trunk)
    roots = [v for v in trunk if v[1] < minimum+.2]
    anchor_x = sum(v[0] for v in roots)/len(roots)
    anchor_z = sum(v[2] for v in roots)/len(roots)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    for part, path, verts, normals, uv, indices in meshes:
        converted, converted_normals = [], []
        for (x,y,z), (nx,ny,nz) in zip(verts, normals):
            x -= anchor_x
            z -= anchor_z
            if part == 'trunk':
                t = max(0, min(1, (y-minimum-.5)/1.8))
                scale = 3.2 + (1.15-3.2)*t
                derivative = (1.15-3.2)/1.8 if 0 < t < 1 else 0
                n = Vector((nx/scale, -nz/scale, ny-derivative/scale*(x*nx+z*nz)))
            else:
                scale = 1.15 if part == 'branches' else 1.2
                n = Vector((nx/scale,-nz/scale,ny))
            converted.append((x*scale, -z*scale, y))
            converted_normals.append(n.normalized())
        faces = [(indices[i],indices[i+1],indices[i+2]) for i in range(0,len(indices),3)]
        mesh = bpy.data.meshes.new('tree_' + part)
        mesh.from_pydata(converted, [], faces)
        mesh.update()
        for polygon in mesh.polygons: polygon.use_smooth = True
        mesh.normals_split_custom_set_from_vertices(converted_normals)
        layer = mesh.uv_layers.new(name='UVMap')
        for loop in mesh.loops:
            layer.data[loop.index].uv = uv[loop.vertex_index]
        obj = bpy.data.objects.new('tree_' + part, mesh)
        bpy.context.collection.objects.link(obj)
        mat = bpy.data.materials.new('tree_' + part)
        mat.use_nodes = True
        shader = mat.node_tree.nodes.get('Principled BSDF')
        shader.inputs['Roughness'].default_value = 1
        prefix = 'tree_small_02' + ('' if part == 'trunk' else '_branch' if part == 'branches' else '_leaves')
        diffuse = nature / (prefix + '_diff_1k.jpg')
        if part == 'leaves':
            rgba = scratch / 'tree-leaves.png'
            subprocess.run(['magick', str(diffuse), str(nature / (prefix + '_alpha_1k.png')),
                            '-alpha', 'off', '-compose', 'CopyOpacity', '-composite', str(rgba)], check=True, timeout=60)
            diffuse = rgba
            mat.surface_render_method = 'DITHERED'
            mat.use_transparency_overlap = False
        texture = mat.node_tree.nodes.new('ShaderNodeTexImage')
        texture.image = bpy.data.images.load(str(diffuse), check_existing=True)
        texture.image.pack()
        mat.node_tree.links.new(texture.outputs['Color'], shader.inputs['Base Color'])
        if part == 'leaves': mat.node_tree.links.new(texture.outputs['Alpha'], shader.inputs['Alpha'])
        mesh.materials.append(mat)
    bpy.ops.export_scene.gltf(filepath=str(destination), export_format='GLB', export_animations=False)
    return [path for _, path, _, _, _, _ in meshes]


def export_nature(destination, scratch):
    """Export Unity's seven prebuilt mid-detail vegetation meshes and albedo."""
    yaml.SafeLoader.add_constructor('tag:unity3d.com,2011:43',
                                    lambda loader, node: loader.construct_mapping(node, deep=True))
    nature = RES / 'ModernNature'
    parts = [('grass', 'grass_medium_01', True), ('fern', 'fern_02', True),
             ('shrub', 'shrub_02', True), ('tree_trunk', 'tree_small_02', False),
             ('rock', 'rock_moss_set_01', False), ('tree_branches', 'tree_small_02_branch', False),
             ('tree_leaves', 'tree_small_02_leaves', True)]
    bpy.ops.wm.read_factory_settings(use_empty=True)
    dependencies = []
    for part, texture_name, alpha in parts:
        path = nature / (part + '_mid.asset')
        dependencies.append(path)
        data = yaml.safe_load(path.read_text())['Mesh']
        v = data['m_VertexData']; raw = bytes.fromhex(v['_typelessdata']); count = v['m_VertexCount']
        channels = [(c['offset'], c['format'], c['dimension']) for c in v['m_Channels'] if c['dimension']]
        if data['m_IndexFormat'] != 0 or len(raw) != count*64 or channels != [(0,0,3),(12,0,3),(24,0,4),(40,0,4),(56,0,2)]:
            raise ValueError('unexpected nature mesh layout ' + str(path))
        verts = [struct.unpack_from('<3f', raw, i*64) for i in range(count)]
        normals = [struct.unpack_from('<3f', raw, i*64+12) for i in range(count)]
        uv = [struct.unpack_from('<2f', raw, i*64+56) for i in range(count)]
        raw_indices = bytes.fromhex(data['m_IndexBuffer'])
        indices = struct.unpack('<'+'H'*(len(raw_indices)//2), raw_indices)
        if len(data['m_SubMeshes']) != 1 or data['m_SubMeshes'][0]['indexCount'] != len(indices) or len(indices)%3:
            raise ValueError('unexpected nature index layout')
        mesh = bpy.data.meshes.new(part)
        mesh.from_pydata([(x,-z,y) for x,y,z in verts], [],
                         [(indices[i],indices[i+1],indices[i+2]) for i in range(0,len(indices),3)])
        mesh.update()
        for polygon in mesh.polygons: polygon.use_smooth = True
        mesh.normals_split_custom_set_from_vertices([(x,-z,y) for x,y,z in normals])
        layer = mesh.uv_layers.new(name='UVMap')
        for loop in mesh.loops: layer.data[loop.index].uv = uv[loop.vertex_index]
        obj = bpy.data.objects.new('nature_' + part, mesh); bpy.context.collection.objects.link(obj)
        mat = bpy.data.materials.new('nature_' + part); mat.use_nodes = True
        shader = mat.node_tree.nodes.get('Principled BSDF'); shader.inputs['Roughness'].default_value = .9
        diffuse = nature / (texture_name + '_diff_1k.jpg'); dependencies.append(diffuse)
        if alpha:
            alpha_path = nature / (texture_name + '_alpha_1k.png'); dependencies.append(alpha_path)
            rgba = scratch / (part + '.png')
            subprocess.run(['magick', str(diffuse), str(alpha_path), '-alpha', 'off',
                            '-compose', 'CopyOpacity', '-composite', str(rgba)], check=True, timeout=60)
            diffuse = rgba; mat.surface_render_method = 'DITHERED'; mat.use_transparency_overlap = False
        tex = mat.node_tree.nodes.new('ShaderNodeTexImage')
        tex.image = bpy.data.images.load(str(diffuse), check_existing=True); tex.image.pack()
        mat.node_tree.links.new(tex.outputs['Color'], shader.inputs['Base Color'])
        if alpha: mat.node_tree.links.new(tex.outputs['Alpha'], shader.inputs['Alpha'])
        mesh.materials.append(mat)
    bpy.ops.export_scene.gltf(filepath=str(destination), export_format='GLB', export_animations=False)
    return dependencies


def main():
    layout = json.loads(LAYOUT.read_text())
    library = json.loads(LIBRARY.read_text())
    if layout['schema'] != 'mc-modern-building-layout/v2' or len(layout['buildings']) != 41 or len(layout['profiles']) != 8:
        raise ValueError('unexpected qualified opening building layout')
    if library['schema'] != 'mc-source-building-art-library/v1' or len(library['profiles']) != 17 or len(library['castle_profiles']) != 7:
        raise ValueError('unexpected BUILD art library')
    if OUTPUT.exists():
        raise FileExistsError('refusing to overwrite existing asset pack: ' + str(OUTPUT))
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(tempfile.mkdtemp(prefix='world-staging-', dir=OUTPUT.parent))
    files = {}
    def record(name, source):
        path = staging / name
        files[name] = {'sha256': digest(path), 'bytes': path.stat().st_size,
                       'source': str(source.relative_to(ROOT)), 'source_sha256': digest(source)}
    for name, src in [('layout.json', LAYOUT), ('building-library.json', LIBRARY)]:
        with (staging / name).open('xb') as out:
            out.write(src.read_bytes())
        record(name, src)
    palette_path = SOURCE_CACHE / 'palette/paln.rgb8'
    palette = palette_path.read_bytes()
    if len(palette) != 768:
        raise ValueError('unexpected source palette')
    color_sources = []
    colors = {}
    for index in [58, *range(67,75)]:
        source = SOURCE_CACHE / f'terrain/tmaps1/{index:03}.indices.u8'
        color_sources.append(source)
        pixels = source.read_bytes()
        totals = [0, 0, 0]; weight_sum = 0
        for value in pixels:
            if not value: continue
            rgb = palette[value*3:value*3+3]; weight = max(rgb)
            for channel in range(3): totals[channel] += rgb[channel]*weight
            weight_sum += weight
        if not weight_sum: raise ValueError(f'empty mana color source {index}')
        colors[str(index)] = [v/weight_sum/255 for v in totals]
    with (staging / 'mana-colors.json').open('x') as out:
        json.dump({'schema':'mc-modern-mana-colors/v1','colors':colors}, out, sort_keys=True)
    record('mana-colors.json', palette_path)
    files['mana-colors.json']['additional_sources'] = [
        {'path': str(path.relative_to(ROOT)), 'sha256': digest(path)} for path in color_sources]
    for prefix in ('Ground', 'Rock'):
        for part in ('Albedo', 'Normal', 'Roughness'):
            src = RES / ('ModernTextures/' + prefix + part + '.jpg')
            name = (prefix + part + '.jpg').lower()
            subprocess.run(['magick', str(src), '-filter', 'Lanczos', '-resize', '1024x1024>',
                            '-strip', '-quality', '85', str(staging / name)], check=True, timeout=60)
            record(name, src)
            files[name]['derivation'] = 'ImageMagick Lanczos max 1024x1024, JPEG quality 85; source unchanged'
    for name, relative in MODELS.items():
        src = RES / relative
        export_model(src, staging / (name + '.glb'))
        record(name + '.glb', src)
        texture = {'experience-scroll':'ModernExperienceScroll/ExperienceScrollPaper',
                   'stone-statue':'ModernScenery/StoneStatueAlbedo',
                   'dolmen':'ModernTextures/RockAlbedo',
                   'castle-head':'ModernCharacters/CastleProjectile/CastleProjectileFabric'}.get(name)
        if texture:
            dependency = texture_path(texture)
            files[name + '.glb']['additional_sources'] = [
                {'path': str(dependency.relative_to(ROOT)), 'sha256': digest(dependency)}]
    with tempfile.TemporaryDirectory(prefix='mc-tree-') as temporary:
        sources = export_tree(staging / 'authored-tree.glb', Path(temporary))
    record('authored-tree.glb', sources[0])
    nature = RES / 'ModernNature'
    sources += [nature / name for name in ('tree_small_02_diff_1k.jpg',
                'tree_small_02_branch_diff_1k.jpg', 'tree_small_02_leaves_diff_1k.jpg',
                'tree_small_02_leaves_alpha_1k.png')]
    sources.append(ROOT / 'research/private/remc2-unity-native/Assets/Scripts/ModernAuthoredTrees.cs')
    files['authored-tree.glb']['additional_sources'] = [
        {'path': str(path.relative_to(ROOT)), 'sha256': digest(path)} for path in sources[1:]]
    with tempfile.TemporaryDirectory(prefix='mc-nature-') as temporary:
        nature_sources = export_nature(staging / 'nature.glb', Path(temporary))
    record('nature.glb', nature_sources[0])
    files['nature.glb']['additional_sources'] = [
        {'path': str(path.relative_to(ROOT)), 'sha256': digest(path)} for path in nature_sources[1:]]
    manifest = {'schema': 'mc-browser-modern-world/v1', 'files': files,
                'coordinates': 'glTF Y-up; native x,y,z map to world x,z,y after division by 64',
                'building_mesh': 'building-library.json profiles; 4 world units per source tile, Y-up',
                'layout': 'layout.json; visual-only, frozen terrain SHA-256 and per-cell preconditions',
                'model_scale': 'FBX original model units; consumer applies source logicalHeight/64',
                'limitations': ['Dynamic building construction beyond exact opening matches remains unimplemented']}
    with (staging / 'manifest.json').open('x') as out:
        json.dump(manifest, out, indent=2, sort_keys=True)
        out.write('\n')
    os.rename(staging, OUTPUT)
    print('WORLD_EXPORT ' + json.dumps({'path': str(OUTPUT), 'files': len(files),
                                        'bytes': sum(f['bytes'] for f in files.values())}))


if __name__ == '__main__':
    main()
