"""Preserve packed maps or encode authored constant PBR values for game import."""
from pathlib import Path
import argparse
import json
import math
import struct
import sys
import zlib

import bpy


def extract_packed(output: Path, names: dict[str, str]):
    directory = output / 'textures'
    directory.mkdir(parents=True, exist_ok=True)
    for source, target in names.items():
        image = bpy.data.images[source]
        if image.packed_file is None:
            raise ValueError(f'{source} is not packed')
        target_path = directory / target
        target_path.write_bytes(bytes(image.packed_file.data))
        # Empty names cause Blender's FBX exporter to alias multiple packed
        # images to the same embedded filename (including the normal map).
        image.filepath = str(target_path.resolve())


def _constant_png(path, rgba, srgb=False):
    """Write numeric material data; encode linear pigment for an sRGB sampler."""
    def encode(value):
        return 12.92*value if value <= .0031308 else 1.055*value**(1/2.4)-.055
    values = [encode(value) if srgb and i < 3 else value for i,value in enumerate(rgba)]
    pixel = bytes(round(value*255) for value in values)
    def chunk(name, data):
        return struct.pack('>I', len(data))+name+data+struct.pack('>I', zlib.crc32(name+data))
    data = b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR', struct.pack('>IIBBBBB', 4, 4, 8, 6, 0, 0, 0))
    if srgb:
        data += chunk(b'sRGB', b'\0')
    data += chunk(b'IDAT', zlib.compress((b'\0'+pixel*4)*4))+chunk(b'IEND', b'')
    with path.open('xb') as output:
        output.write(data)


def export_constants(output: Path, mesh_names: list[str]):
    """Preserve flat Principled materials for the existing Unity PBR importer.

    These maps encode authored constants, not invented detail or baked AO.
    Textured/linked materials must use their actual maps instead of this path.
    """
    if not mesh_names:
        raise ValueError('At least one explicit mesh is required')
    materials = set()
    for name in mesh_names:
        obj = bpy.data.objects[name]
        if obj.type != 'MESH' or not obj.data.materials or any(material is None for material in obj.data.materials):
            raise ValueError('Expected a mesh with complete material slots: '+name)
        materials.update(obj.data.materials)
    records, filenames = [], set()
    for material in sorted(materials, key=lambda item:item.name):
        shaders = [node for node in material.node_tree.nodes if node.type == 'BSDF_PRINCIPLED'] if material.use_nodes else []
        if len(shaders) != 1:
            raise ValueError('Constant material export needs one Principled shader: '+material.name)
        shader = shaders[0]
        outputs = [node for node in material.node_tree.nodes if node.type == 'OUTPUT_MATERIAL' and node.is_active_output]
        if len(outputs) != 1 or len(outputs[0].inputs['Surface'].links) != 1 or outputs[0].inputs['Surface'].links[0].from_node != shader:
            raise ValueError('Expected the Principled shader directly connected to the active surface: '+material.name)
        if any(outputs[0].inputs[key].is_linked for key in ('Volume','Displacement')):
            raise ValueError('Volume/displacement requires a dedicated material bake: '+material.name)
        if any(shader.inputs[key].is_linked for key in ('Base Color','Metallic','Roughness','Alpha','Normal')):
            raise ValueError('Linked material inputs require their real source textures: '+material.name)
        if any(shader.inputs[key].is_linked or shader.inputs[key].default_value != 0
               for key in ('Subsurface Weight','Transmission Weight','Coat Weight','Sheen Weight')) or any(
               shader.inputs[key].is_linked for key in ('Emission Strength','Emission Color')) or (
               shader.inputs['Emission Strength'].default_value > 0 and any(shader.inputs['Emission Color'].default_value[:3])):
            raise ValueError('This path supports opaque base-color/metallic/roughness only: '+material.name)
        color = tuple(shader.inputs['Base Color'].default_value)
        metallic, roughness = (shader.inputs[key].default_value for key in ('Metallic','Roughness'))
        if not all(math.isfinite(value) and 0 <= value <= 1 for value in (*color,metallic,roughness)) or shader.inputs['Alpha'].default_value != 1 or color[3] != 1:
            raise ValueError('Expected opaque finite unit-range material values: '+material.name)
        safe = ''.join(char if char.isalnum() else '_' for char in material.name)
        if safe in filenames:
            raise ValueError('Sanitized material filename collision: '+safe)
        filenames.add(safe)
        records.append({'name':material.name, 'albedo':f'textures/{safe}_albedo.png',
                        'orm':f'textures/{safe}_orm.png', 'linearColor':color,
                        'roughness':roughness, 'metallic':metallic})
    output.mkdir(parents=True, exist_ok=False)
    (output/'textures').mkdir()
    for record in records:
        _constant_png(output/record['albedo'], record['linearColor'], srgb=True)
        _constant_png(output/record['orm'], (1,record['roughness'],record['metallic'],1))
    report = {'schema':'mc-character-constant-materials/v1','materials':records,
              'channels':'sRGB albedo; linear ORM: R=1 AO, G=roughness, B=metallic; no authored normal texture'}
    (output/'materials.json').write_text(json.dumps(report, indent=2)+'\n')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--mesh', action='append', required=True)
    args = parser.parse_args(sys.argv[sys.argv.index('--')+1:])
    print(json.dumps(export_constants(args.output.resolve(), args.mesh)), flush=True)
