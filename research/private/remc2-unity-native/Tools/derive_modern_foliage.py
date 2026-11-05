"""Bake source-faithful, relightable multiview vegetation impostors.

From the Unity project root:
  env -u DISPLAY -u WAYLAND_DISPLAY blender --factory-startup -b -t 4 --python Tools/derive_modern_foliage.py
  uv run --no-project --with pillow==13.0.0 python Tools/pack_modern_foliage.py

Frames are written below research/private/generated/nightspire-rework-20261105/
multiview-foliage. Each is an orthographic view of the actual selected CC0 mesh.
The source opacity is clipped at 0.4 per surface, as in ModernVegetation.shader.
Albedo is unlit; normals and view depth are linear data. No scene lighting is baked.
"""
import math
import os
import bpy
from mathutils import Matrix, Vector
from pathlib import Path

PROJECT = Path(__file__).resolve().parent.parent
ROOT = PROJECT / 'Assets/Resources/ModernNature'
OUT = Path(os.environ.get('MC_FOLIAGE_OUTPUT', str(PROJECT.parent / 'generated/nightspire-rework-20261105/multiview-foliage')))
OUT.mkdir(parents=True, exist_ok=True)
LIMIT = int(os.environ.get('MC_FOLIAGE_VIEW_LIMIT', '0'))
ONLY_ASSET = os.environ.get('MC_FOLIAGE_ASSET', '')
rendered = 0
SOURCES = (
    ('grass_medium_01_1k.fbx', 'grass_medium_01_mid_b_LOD0', 'grass_medium_01', 'grass_lod'),
    ('fern_02_1k.fbx', 'fern_02_b', 'fern_02', 'fern_lod'),
    ('shrub_02_1k.fbx', 'shrub_02_b_LOD0', 'shrub_02', 'shrub_lod'),
    ('tree_small_02_derived.fbx', 'tree_leaves', 'tree_small_02_leaves', 'tree_leaves_lod'),
)
AZIMUTHS = 8
ELEVATIONS = 5

for filename, part, texture, output in SOURCES:
    if ONLY_ASSET and output != ONLY_ASSET:
        continue
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.object.delete(use_global=False)
    bpy.ops.import_scene.fbx(filepath=str(ROOT / filename))
    obj = bpy.data.objects[part]
    for other in list(bpy.context.scene.objects):
        if other != obj:
            bpy.data.objects.remove(other, do_unlink=True)
    bpy.context.view_layer.update()
    # Match Unity's LoadScannedMesh: transform actual vertices first, then
    # recalculate tight bounds. Transforming a local AABB is not equivalent.
    points = [obj.matrix_world @ v.co for v in obj.data.vertices]
    lo = Vector(tuple(min(p[i] for p in points) for i in range(3)))
    hi = Vector(tuple(max(p[i] for p in points) for i in range(3)))
    center = (lo + hi) * .5
    # Sphere bounds every view. Unity computes this same radius from the
    # normalized source mesh rather than relying on a separate scale constant.
    radius = max((point - center).length for point in points) * 1.01
    diameter = 2 * radius

    mat = bpy.data.materials.new(output + '_capture')
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    nodes.clear()
    links = mat.node_tree.links
    diffuse = nodes.new('ShaderNodeTexImage')
    diffuse.image = bpy.data.images.load(str(ROOT / (texture + '_diff_1k.jpg')), check_existing=True)
    opacity = nodes.new('ShaderNodeTexImage')
    opacity.image = bpy.data.images.load(str(ROOT / (texture + '_alpha_1k.png')), check_existing=True)
    opacity.image.colorspace_settings.name = 'Non-Color'
    cutoff = nodes.new('ShaderNodeMath')
    cutoff.operation = 'GREATER_THAN'
    cutoff.inputs[1].default_value = .4
    links.new(opacity.outputs['Color'], cutoff.inputs[0])
    emission = nodes.new('ShaderNodeEmission')
    transparent = nodes.new('ShaderNodeBsdfTransparent')
    mix = nodes.new('ShaderNodeMixShader')
    links.new(cutoff.outputs[0], mix.inputs[0])
    links.new(transparent.outputs[0], mix.inputs[1])
    links.new(emission.outputs[0], mix.inputs[2])
    result = nodes.new('ShaderNodeOutputMaterial')
    links.new(mix.outputs[0], result.inputs['Surface'])
    obj.data.materials.clear()
    obj.data.materials.append(mat)

    # Tangent-space source normal map produces a world-space normal in Blender.
    # The observed Unity FBX basis is (-BlenderX, BlenderZ, -BlenderY).
    normal_texture = nodes.new('ShaderNodeTexImage')
    normal_texture.image = bpy.data.images.load(str(ROOT / (texture + '_nor_gl_1k.jpg')), check_existing=True)
    normal_texture.image.colorspace_settings.name = 'Non-Color'
    normal_map = nodes.new('ShaderNodeNormalMap')
    links.new(normal_texture.outputs['Color'], normal_map.inputs['Color'])
    separate = nodes.new('ShaderNodeSeparateXYZ')
    links.new(normal_map.outputs['Normal'], separate.inputs[0])
    normal_color = nodes.new('ShaderNodeCombineColor')
    for source_axis, target_axis, sign in (('X', 'Red', -1), ('Z', 'Green', 1), ('Y', 'Blue', -1)):
        multiply = nodes.new('ShaderNodeMath'); multiply.operation = 'MULTIPLY'; multiply.inputs[1].default_value = .5 * sign
        add = nodes.new('ShaderNodeMath'); add.operation = 'ADD'; add.inputs[1].default_value = .5
        links.new(separate.outputs[source_axis], multiply.inputs[0]); links.new(multiply.outputs[0], add.inputs[0])
        links.new(add.outputs[0], normal_color.inputs[target_axis])

    # Encode dot(surface-center, direction) / sphere diameter + 0.5.
    geometry = nodes.new('ShaderNodeNewGeometry')
    delta = nodes.new('ShaderNodeVectorMath'); delta.operation = 'SUBTRACT'; delta.inputs[1].default_value = center
    links.new(geometry.outputs['Position'], delta.inputs[0])
    dot = nodes.new('ShaderNodeVectorMath'); dot.operation = 'DOT_PRODUCT'
    links.new(delta.outputs['Vector'], dot.inputs[0])
    depth_scale = nodes.new('ShaderNodeMath'); depth_scale.operation = 'MULTIPLY'; depth_scale.inputs[1].default_value = 1 / diameter
    depth_bias = nodes.new('ShaderNodeMath'); depth_bias.operation = 'ADD'; depth_bias.inputs[1].default_value = .5
    links.new(dot.outputs['Value'], depth_scale.inputs[0]); links.new(depth_scale.outputs[0], depth_bias.inputs[0])

    camera_data = bpy.data.cameras.new(output + '_camera')
    camera = bpy.data.objects.new(output + '_camera', camera_data)
    bpy.context.scene.collection.objects.link(camera)
    camera_data.type = 'ORTHO'; camera_data.ortho_scale = diameter
    camera_data.clip_start = .001; camera_data.clip_end = diameter * 5
    scene = bpy.context.scene; scene.camera = camera; scene.render.engine = 'CYCLES'
    scene.cycles.device = 'CPU'; scene.cycles.samples = 16; scene.cycles.transparent_max_bounces = 64
    scene.render.resolution_x = 128; scene.render.resolution_y = 128; scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = 'PNG'; scene.render.image_settings.color_mode = 'RGBA'
    scene.render.film_transparent = True; scene.view_settings.look = 'None'
    print('SOURCE', output, 'center', tuple(center), 'radius', radius, flush=True)
    for elevation_index in range(ELEVATIONS):
        elevation = -math.pi / 2 + elevation_index * math.pi / (ELEVATIONS - 1)
        ce, se = math.cos(elevation), math.sin(elevation)
        for azimuth_index in range(AZIMUTHS):
            azimuth = azimuth_index * 2 * math.pi / AZIMUTHS
            ca, sa = math.cos(azimuth), math.sin(azimuth)
            # Unity view dir=(ce*ca,se,ce*sa). Convert to Blender basis;
            # calculate camera basis explicitly, including at the poles.
            direction = Vector((-ce*ca, -ce*sa, se))
            right = Vector((sa, -ca, 0))
            up = Vector((se*ca, se*sa, ce))
            camera.location = center + direction * (diameter * 2)
            camera.rotation_euler = Matrix((right, up, direction)).transposed().to_euler()
            dot.inputs[1].default_value = direction
            for channel, socket, transform in (('color', diffuse.outputs['Color'], 'Standard'),
                                                ('normal', normal_color.outputs['Color'], 'Raw'),
                                                ('depth', depth_bias.outputs[0], 'Raw')):
                links.new(socket, emission.inputs['Color'])
                scene.view_settings.view_transform = transform
                scene.render.filepath = str(OUT / f'{output}_{elevation_index}_{azimuth_index}_{channel}.png')
                bpy.ops.render.render(write_still=True)
            print('BAKED', output, elevation_index, azimuth_index, flush=True)
            rendered += 1
            if LIMIT and rendered >= LIMIT:
                break
        if LIMIT and rendered >= LIMIT:
            break
    if LIMIT and rendered >= LIMIT:
        break
