using System;
using UnityEditor;
using UnityEngine;

namespace MagicCarpet.Native.Editor
{
    // The downloaded maps are data maps, not color images. Keep FBX meshes
    // readable so the presentation can center each scanned variant at its base.
    internal sealed class ModernNatureImport : AssetPostprocessor
    {
        private const string Folder = "Assets/Resources/ModernNature/";
        private static readonly string[] RequiredMaps =
        {
            "grass_medium_01_diff_1k.jpg", "grass_medium_01_nor_gl_1k.jpg",
            "grass_medium_01_rough_1k.jpg", "grass_medium_01_alpha_1k.png",
            "fern_02_diff_1k.jpg", "fern_02_nor_gl_1k.jpg",
            "fern_02_rough_1k.jpg", "fern_02_alpha_1k.png",
            "rock_moss_set_01_diff_1k.jpg", "rock_moss_set_01_nor_gl_1k.jpg",
            "rock_moss_set_01_rough_1k.jpg",
            "shrub_02_diff_1k.jpg", "shrub_02_nor_gl_1k.jpg",
            "shrub_02_rough_1k.jpg", "shrub_02_alpha_1k.png",
            "tree_small_02_diff_1k.jpg", "tree_small_02_nor_gl_1k.jpg", "tree_small_02_rough_1k.jpg",
            "tree_small_02_branch_diff_1k.jpg", "tree_small_02_branch_nor_gl_1k.jpg", "tree_small_02_branch_rough_1k.jpg",
            "tree_small_02_leaves_diff_1k.jpg", "tree_small_02_leaves_nor_gl_1k.jpg", "tree_small_02_leaves_rough_1k.jpg",
            "tree_small_02_leaves_alpha_1k.png"
        };

        private void OnPreprocessTexture()
        {
            if (!assetPath.StartsWith(Folder, System.StringComparison.Ordinal)) return;
            ConfigureTexture((TextureImporter)assetImporter, assetPath);
        }

        private static void ConfigureTexture(TextureImporter importer, string path)
        {
            importer.textureType = TextureImporterType.Default;
            importer.textureShape = TextureImporterShape.Texture2D;
            importer.sRGBTexture = path.Contains("_diff_");
            importer.mipmapEnabled = true;
            importer.textureCompression = TextureImporterCompression.CompressedHQ;
            importer.anisoLevel = 8;
            importer.maxTextureSize = 1024;
        }

        // Called before the player build. Existing .meta files can predate this
        // postprocessor; reimport stale settings rather than trusting them.
        internal static void Prepare()
        {
            foreach (var name in RequiredMaps)
            {
                var path = Folder + name;
                var importer = AssetImporter.GetAtPath(path) as TextureImporter;
                if (importer == null) throw new InvalidOperationException("Missing nature texture importer: " + path);
                var color = name.Contains("_diff_");
                var stale = importer.textureType != TextureImporterType.Default ||
                    importer.textureShape != TextureImporterShape.Texture2D ||
                    importer.sRGBTexture != color || !importer.mipmapEnabled ||
                    importer.textureCompression != TextureImporterCompression.CompressedHQ ||
                    importer.anisoLevel != 8 || importer.maxTextureSize != 1024 ||
                    importer.GetPlatformTextureSettings("Standalone").overridden ||
                    AssetDatabase.LoadAssetAtPath<Texture2D>(path) == null;
                if (stale)
                {
                    ConfigureTexture(importer, path);
                    importer.ClearPlatformTextureSettings("Standalone");
                    importer.SaveAndReimport();
                }
                var resourceName = "ModernNature/" + System.IO.Path.GetFileNameWithoutExtension(name);
                if (AssetDatabase.LoadAssetAtPath<Texture2D>(path) == null ||
                    Resources.Load<Texture2D>(resourceName) == null)
                    throw new InvalidOperationException("Nature texture is not a Texture2D resource: " + path);
            }
            Debug.Log("MC_MODERN_NATURE_IMPORT validated " + RequiredMaps.Length + " 2D texture resources");
        }

        private void OnPreprocessModel()
        {
            if (!assetPath.StartsWith(Folder, System.StringComparison.Ordinal)) return;
            var importer = (ModelImporter)assetImporter;
            importer.isReadable = true;
            importer.importAnimation = false;
            importer.materialImportMode = ModelImporterMaterialImportMode.None;
            importer.importTangents = ModelImporterTangents.CalculateMikk;
        }
    }
}
