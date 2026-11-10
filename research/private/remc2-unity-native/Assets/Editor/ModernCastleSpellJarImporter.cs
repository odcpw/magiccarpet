using System;
using UnityEditor;

namespace MagicCarpet.Native.Editor
{
    // The single-mesh block export has a different Blender FBX root unit
    // conversion from the multi-object master.
    internal sealed class ModernCastleSpellJarImporter : AssetPostprocessor
    {
        private void OnPreprocessModel()
        {
            const string voxel = "Assets/Resources/ModernCastleSpellJar/CastleSpellJarVoxel.fbx";
            if (!assetPath.Equals(voxel, StringComparison.Ordinal)) return;
            ((ModelImporter)assetImporter).globalScale = 100f;
        }
    }
}
