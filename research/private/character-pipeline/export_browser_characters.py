#!/usr/bin/env python3
"""Export the reviewed Unity character packages to self-contained browser GLBs.

Run from any directory with Python 3. The Blender worker is CPU-only and writes
into a new output directory; existing output is never overwritten. Owner-local
FBXs and textures are required and are not copied into the repository.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import math
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
PRIVATE = ROOT / 'research/private'
RESOURCES = PRIVATE / 'remc2-unity-native/Assets/Resources/ModernCharacters'
GENERATED = PRIVATE / 'generated'
BLENDER = Path('/home/oliver/Projects/fittimer-avatar-toolchain/blender-4.5.12-linux-x64/blender')
FAMILIES = ('Goat', 'Archer', 'Villager', 'VillagerBrown', 'VillagerStriped',
            'VillagerCarrier', 'Builder', 'Trader', 'Firefly', 'WormHead', 'WormSegment', 'Collector')


def package(name):
    if name == 'Firefly':
        return GENERATED / 'level1-assets-20261110/firefly/candidate03/unity-forward'
    if name.startswith('Worm'):
        return GENERATED / 'level1-assets-20261110/worm/portable-candidate08'
    return RESOURCES / name


def worker(name, destination):
    import bpy
    folder = package(name)
    metadata = json.loads((folder / (name + '.clips.json')).read_text())
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.fbx(filepath=str(folder / (name + '.fbx')), use_anim=True)
    # The FBX carries its own sampling rate (human packages use 120 Hz).
    fps = bpy.context.scene.render.fps / bpy.context.scene.render.fps_base
    animated = [o for o in bpy.context.scene.objects if o.animation_data and o.animation_data.action]
    if not animated:
        raise RuntimeError(f'{name}: imported FBX has no action')
    # FBX is a single accepted baked timeline; Unity splits that timeline using
    # these clip ranges. Mirror those ranges as separate NLA tracks for glTF.
    for obj in animated:
        animation = obj.animation_data
        action = animation.action
        animation.action = None
        for clip in metadata['clips']:
            track = animation.nla_tracks.new()
            track.name = clip['name']
            strip = track.strips.new(clip['name'], 0, action)
            # Blender's FBX importer shifts the baked Scene take by one frame.
            strip.action_frame_start = clip['firstFrame'] + 1
            strip.action_frame_end = clip['lastFrame'] + 1
            strip.frame_start = 0
            strip.frame_end = clip['lastFrame'] - clip['firstFrame']
    maps = metadata.get('materials', [metadata['material']] if 'material' in metadata else [])
    by_name = {m.name: m for m in bpy.data.materials}
    for mapspec in maps:
        material = by_name.get(mapspec['name'])
        if material is None:
            raise RuntimeError(f'{name}: missing material {mapspec["name"]!r}')
        material.use_nodes = True
        nodes = material.node_tree.nodes
        nodes.clear()
        output = nodes.new('ShaderNodeOutputMaterial')
        shader = nodes.new('ShaderNodeBsdfPrincipled')
        material.node_tree.links.new(shader.outputs['BSDF'], output.inputs['Surface'])
        def image_node(path, noncolor=False, *, nodes=nodes):
            node = nodes.new('ShaderNodeTexImage')
            original = folder / path
            # Render derivatives only: retain packed PBR channels, limit each
            # image to 1k, and leave the reviewed owner masters untouched.
            derivative_dir = destination.parent / ('.' + name + '-textures')
            derivative_dir.mkdir(exist_ok=True)
            extension = '.png' if noncolor else '.jpg'
            digest = hashlib.sha256(str(original).encode()).hexdigest()[:16]
            derivative = derivative_dir / (digest + extension)
            if not derivative.exists():
                command = ['magick', str(original), '-resize', '1024x1024>']
                if not noncolor: command += ['-quality', '88']
                subprocess.run(command + [str(derivative)], check=True, timeout=120)
            node.image = bpy.data.images.load(str(derivative), check_existing=True)
            if noncolor:
                node.image.colorspace_settings.name = 'Non-Color'
            return node
        albedo = image_node(mapspec['albedo'])
        material.node_tree.links.new(albedo.outputs['Color'], shader.inputs['Base Color'])
        if mapspec.get('orm'):
            orm = image_node(mapspec['orm'], True)
            sep = nodes.new('ShaderNodeSeparateColor')
            material.node_tree.links.new(orm.outputs['Color'], sep.inputs['Color'])
            material.node_tree.links.new(sep.outputs['Green'], shader.inputs['Roughness'])
            material.node_tree.links.new(sep.outputs['Blue'], shader.inputs['Metallic'])
            # Blender's glTF exporter reads ambient occlusion from its named
            # material-output group. Preserve the source ORM red channel too.
            from io_scene_gltf2.blender.com.material_helpers import create_settings_group, get_gltf_node_name
            group_name = get_gltf_node_name()
            group = bpy.data.node_groups.get(group_name) or create_settings_group(group_name)
            output = nodes.new('ShaderNodeGroup')
            output.node_tree = group
            material.node_tree.links.new(sep.outputs['Red'], output.inputs['Occlusion'])
        if mapspec.get('normal'):
            image = image_node(mapspec['normal'], True)
            normal = nodes.new('ShaderNodeNormalMap')
            material.node_tree.links.new(image.outputs['Color'], normal.inputs['Color'])
            material.node_tree.links.new(normal.outputs['Normal'], shader.inputs['Normal'])
        opacity = mapspec.get('opacity', 1) or 1
        shader.inputs['Alpha'].default_value = opacity
        if opacity < 1:
            material.surface_render_method = 'DITHERED'
            material.use_transparency_overlap = False
        strength = mapspec.get('emissionStrength', 0)
        if strength:
            shader.inputs['Emission Color'].default_value = mapspec['emissionColor']
            shader.inputs['Emission Strength'].default_value = strength
    bpy.ops.export_scene.gltf(filepath=str(destination), export_format='GLB',
                              export_animations=True, export_animation_mode='NLA_TRACKS',
                              export_frame_range=False, export_yup=True)
    destination.with_suffix('.export.json').write_text(json.dumps({'fps': fps}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=GENERATED / 'browser-level1-assets/characters')
    parser.add_argument('--blender', type=Path, default=BLENDER)
    parser.add_argument('--worker', choices=FAMILIES, help=argparse.SUPPRESS)
    args = parser.parse_args(sys.argv[sys.argv.index('--')+1:] if '--' in sys.argv else None)
    if args.worker:
        worker(args.worker, args.output)
        return
    if not shutil.which('magick'):
        raise RuntimeError('ImageMagick magick is required for bounded browser textures')
    # Fail before creating output if any reviewed package is unavailable.
    for name in FAMILIES:
        for suffix in ('.fbx', '.clips.json'):
            path = package(name) / (name + suffix)
            if not path.is_file():
                raise FileNotFoundError(path)
    args.output.mkdir(parents=True, exist_ok=False)
    manifest = {'schema': 'mc-browser-modern-characters/v1', 'assets': {}, 'files': {}}
    for name in FAMILIES:
        source = package(name)
        metadata = json.loads((source / (name + '.clips.json')).read_text())
        target = args.output / (name + '.glb')
        subprocess.run([str(args.blender), '-b', '-t', '2', '--python-exit-code', '1',
                        '--python', str(Path(__file__).resolve()), '--', '--worker', name,
                        '--output', str(target)], check=True, timeout=600,
                       env={**os.environ, 'OMP_NUM_THREADS':'2'})
        fps = json.loads(target.with_suffix('.export.json').read_text())['fps']
        digest = hashlib.sha256(target.read_bytes()).hexdigest()
        forward = metadata.get('forward', [0, -1, 0])
        if not isinstance(forward, list):
            raise TypeError(f'{name}: source forward is not explicit')
        yaw = math.atan2(-forward[0], forward[1])
        clips = [{'name': c['name'], 'duration': (c['lastFrame'] - c['firstFrame']) / fps,
                  'loop': c['loop']} for c in metadata['clips']]
        url = 'characters/' + target.name
        manifest['assets'][name] = {'url':url, 'sha256':digest, 'bytes':target.stat().st_size,
                                    'height':metadata['height'], 'forward_yaw':yaw,
                                    'walk_speed':metadata.get('walkSpeed', .14844 if name == 'Goat' else 1.65 if name == 'Archer' else 0),
                                    'run_speed':metadata.get('runSpeed', 0), 'clips':clips}
        manifest['files'][url] = {'sha256':digest, 'bytes':target.stat().st_size,
                                  'source':str((source / (name + '.fbx')).relative_to(ROOT))}
    (args.output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')


if __name__ == '__main__':
    main()
