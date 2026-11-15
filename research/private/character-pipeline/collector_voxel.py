"""Build exterior voxel meshes from the accepted, static CollectorSkin master.

Blender: -b -t 2 --python-exit-code 1 --python collector_voxel.py --
    --master Collector.blend --output NEW_DIR

The static Idle rig has one bone and no motion. Each occupied surface cell
gets a source material role; only exposed faces are emitted. Nothing fills
the envelope interior and no per-cell object is produced. Material names are
retained so the Unity presenter can reuse the accepted owner-tinted materials.
"""
import argparse
import json
import math
import sys
from collections import Counter
from pathlib import Path

import bpy
from mathutils import Vector


def build(source, output, resolution):
    mesh = source.data
    mesh.calc_loop_triangles()
    points = [source.matrix_world @ v.co for v in mesh.vertices]
    bottom = min(p.z for p in points)
    height = max(p.z for p in points) - bottom
    size = height / resolution
    cells = {}
    # Keep narrow rigging, seams and embroidery visible when surfaces meet
    # the cloth in one cell, without expanding them into filled volumes.
    priority = {'Owner embroidery': 3, 'Fabric seams': 2,
                'Dark suspension rope': 2, 'Aged brass': 2,
                'Gondola iron and dark wood': 2}
    roles = [m.name for m in mesh.materials]
    for tri in mesh.loop_triangles:
        a, b, c = (points[i] for i in tri.vertices)
        steps = max(1, math.ceil(max((a-b).length, (b-c).length, (c-a).length) / size))
        role = tri.material_index
        rank = priority.get(roles[role], 0)
        for u in range(steps + 1):
            for v in range(steps + 1 - u):
                p = a + (b-a) * (u / steps) + (c-a) * (v / steps)
                key = (math.floor(p.x/size), math.floor(p.y/size), math.floor((p.z-bottom)/size))
                center = Vector(((key[0]+.5)*size, (key[1]+.5)*size, bottom+(key[2]+.5)*size))
                distance = (p-center).length_squared
                previous = cells.get(key)
                if previous is None or rank > previous[0] or rank == previous[0] and distance < previous[1]:
                    cells[key] = (rank, distance, role)
    directions = [(0,0,-1), (0,0,1), (-1,0,0), (1,0,0), (0,-1,0), (0,1,0)]
    corners = [(0,0,0),(1,0,0),(1,1,0),(0,1,0),(0,0,1),(1,0,1),(1,1,1),(0,1,1)]
    faces = [(0,3,2,1),(4,5,6,7),(0,4,7,3),(1,2,6,5),(0,1,5,4),(3,7,6,2)]
    vertices = []
    polygons = []
    material_indices = []
    for (x,y,z), (_,_,role) in sorted(cells.items()):
        for direction, face in zip(directions, faces):
            if (x+direction[0],y+direction[1],z+direction[2]) in cells:
                continue
            first = len(vertices)
            vertices.extend(((x+corners[i][0])*size, (y+corners[i][1])*size,
                             bottom+(z+corners[i][2])*size) for i in face)
            polygons.append((first,first+1,first+2,first+3))
            material_indices.append(role)
    data = bpy.data.meshes.new(f'Collector voxel {resolution}')
    data.from_pydata(vertices, [], polygons)
    data.update()
    for material in mesh.materials:
        data.materials.append(material)
    for polygon, role in zip(data.polygons, material_indices):
        polygon.material_index = role
    obj = bpy.data.objects.new(f'CollectorVoxel{resolution}', data)
    bpy.context.scene.collection.objects.link(obj)
    bpy.ops.object.select_all(action='DESELECT')
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    path = output / f'CollectorVoxel{resolution}.fbx'
    bpy.ops.export_scene.fbx(filepath=str(path), use_selection=True, object_types={'MESH'},
                             axis_forward='-Z', axis_up='Y', apply_unit_scale=True,
                             add_leaf_bones=False, bake_anim=False)
    counts = Counter(roles[index] for index in material_indices)
    report = {'resolution': resolution, 'cellSize': size, 'cells': len(cells),
              'faces': len(polygons), 'triangles': len(polygons)*2,
              'materialFaces': dict(sorted(counts.items()))}
    bpy.data.objects.remove(obj, do_unlink=True)
    bpy.data.meshes.remove(data)
    return report


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--master', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args(sys.argv[sys.argv.index('--')+1:])
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    bpy.ops.wm.open_mainfile(filepath=str(args.master.resolve()))
    source = bpy.data.objects.get('CollectorSkin')
    if source is None or source.type != 'MESH':
        raise ValueError('Accepted CollectorSkin master missing')
    report = [build(source, output, resolution) for resolution in (96,48)]
    (output/'collector-voxel.json').write_text(json.dumps(report, indent=2)+'\n')
    print('COLLECTOR_VOXEL_OK', json.dumps(report), flush=True)


if __name__ == '__main__':
    main()
