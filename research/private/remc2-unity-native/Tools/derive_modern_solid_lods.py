"""Generate UV-preserving solid LODs from the original Poly Haven-derived FBX.
Run from the Unity project root:
  env -u DISPLAY -u WAYLAND_DISPLAY blender --factory-startup -b -t 4 --python Tools/derive_modern_solid_lods.py
"""
import bpy
from pathlib import Path
ROOT = Path(__file__).resolve().parent.parent / 'Assets/Resources/ModernNature'
for source, output, budgets in (
    ('rock_moss_set_01_1k.fbx', 'rock_moss_set_01_lod.fbx', {'rock_moss_set_01_rock03': 500}),
    ('tree_small_02_derived.fbx', 'tree_small_02_lod.fbx', {'tree_trunk': 300, 'tree_branches': 300}),
):
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.object.delete(use_global=False)
    bpy.ops.import_scene.fbx(filepath=str(ROOT / source))
    for obj in list(bpy.context.scene.objects):
        if obj.name not in budgets:
            bpy.data.objects.remove(obj, do_unlink=True)
            continue
        bpy.context.view_layer.objects.active = obj
        obj.select_set(True)
        mod = obj.modifiers.new('Bounded distant geometry', 'DECIMATE')
        mod.ratio = min(1.0, budgets[obj.name] / max(1, len(obj.data.polygons)))
        bpy.ops.object.modifier_apply(modifier=mod.name)
        print('LOD', obj.name, len(obj.data.polygons), flush=True)
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.export_scene.fbx(filepath=str(ROOT / output), use_selection=True,
                             object_types={'MESH'}, add_leaf_bones=False,
                             bake_anim=False, path_mode='AUTO')
    print('WROTE', output, flush=True)
