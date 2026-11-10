using System;
using UnityEditor;

namespace MagicCarpet.Native.Editor
{
    // Blender's single-object voxel FBX carries its unit conversion on the
    // root differently from the multi-object master. Normalize at import so
    // both prefabs expose the same model-space height to the presenter.
    internal sealed class ModernExperienceScrollImporter : AssetPostprocessor
    {
        private void OnPreprocessModel()
        {
            const string voxel = "Assets/Resources/ModernExperienceScroll/ExperienceScrollVoxel.fbx";
            if (!assetPath.Equals(voxel, StringComparison.Ordinal)) return;
            var model = (ModelImporter)assetImporter;
            model.globalScale = 100f;
        }
    }
}
