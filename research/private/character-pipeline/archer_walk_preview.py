"""Render a three-cycle, three-quarter treadmill review of an archer walk.

Run: blender -b -t 4 --python-exit-code 1 -P archer_walk_preview.py -- --model walk.glb --output frames
The fixed character is animated from the exported GLB; the checker floor moves
at the observed approximately 1.65 model units/second stance speed. This is a
visual reference, not a proof that every point on a rolling foot is locked.
"""
import argparse
import math
import sys
from pathlib import Path
import bpy
from mathutils import Vector


def main():
 parser=argparse.ArgumentParser(description=__doc__)
 parser.add_argument('--model',type=Path,required=True)
 parser.add_argument('--output',type=Path,required=True)
 args=parser.parse_args(sys.argv[sys.argv.index('--')+1:])
 args.output.mkdir(parents=True,exist_ok=True)
 bpy.ops.wm.read_factory_settings(use_empty=True)
 bpy.ops.import_scene.gltf(filepath=str(args.model.resolve()))
 scene=bpy.context.scene
 scene.render.engine='BLENDER_EEVEE_NEXT'
 scene.render.resolution_x=640;scene.render.resolution_y=800
 scene.render.resolution_percentage=100;scene.render.image_settings.file_format='PNG'
 scene.world=bpy.data.worlds.new('Studio');scene.world.use_nodes=True
 scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.16,.18,.22,1)
 scene.world.node_tree.nodes['Background'].inputs[1].default_value=.7
 center=Vector((.05,0,.95))
 bpy.ops.mesh.primitive_plane_add(size=100,location=(0,0,0));floor=bpy.context.object
 material=bpy.data.materials.new('Reference checker floor');material.use_nodes=True
 nodes=material.node_tree.nodes;bsdf=nodes.get('Principled BSDF');checker=nodes.new('ShaderNodeTexChecker')
 checker.inputs['Color1'].default_value=(.12,.14,.16,1);checker.inputs['Color2'].default_value=(.22,.24,.26,1)
 checker.inputs['Scale'].default_value=2.5
 coord=nodes.new('ShaderNodeTexCoord');material.node_tree.links.new(coord.outputs['Object'],checker.inputs['Vector'])
 material.node_tree.links.new(checker.outputs['Color'],bsdf.inputs['Base Color']);floor.data.materials.append(material)
 for name,position,power,width in [('Key',(3,-4,6),900,4),('Fill',(-4,-2,3),450,5),('Rim',(2,4,5),750,3)]:
  data=bpy.data.lights.new(name,'AREA');data.energy=power;data.shape='DISK';data.size=width
  light=bpy.data.objects.new(name,data);scene.collection.objects.link(light);light.location=position
  light.rotation_euler=(center-light.location).to_track_quat('-Z','Y').to_euler()
 bpy.ops.object.camera_add();camera=bpy.context.object;scene.camera=camera
 camera.data.type='ORTHO';camera.data.ortho_scale=2.32
 angle=math.radians(45);camera.location=center+Vector((math.sin(angle)*5,-math.cos(angle)*5,1))
 camera.rotation_euler=(center-camera.location).to_track_quat('-Z','Y').to_euler()
 for frame in range(1,73):
  scene.frame_set(1+(frame-1)%24);floor.location.y=(frame-1)*.0688
  scene.render.filepath=str((args.output/f'frame-{frame:03d}.png').resolve())
  bpy.ops.render.render(write_still=True)
 print('ARCHER_TREADMILL_OK 72 frames at 24 fps',flush=True)


if __name__=='__main__':main()
