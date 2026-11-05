using System;
using UnityEditor;
using UnityEngine;

namespace MagicCarpet.Native.Editor
{
    // Import source color and data maps explicitly. Keep FBX meshes readable
    // so the presentation can center each scanned variant at its base.
    internal sealed class ModernNatureImport : AssetPostprocessor
    {
        private const string Folder = "Assets/Resources/ModernNature/";
        [Serializable] private sealed class ImportedPart { public string name; public Vector3[] vertices; }
        [Serializable] private sealed class ImportedParts { public ImportedPart[] parts; }

        // Authoring validation: observe Unity's actual FBX transform rather than
        // assuming that the offline baker and importer share handedness.
        public static void ExportImportedMeshPositions()
        {
            var output = Environment.GetEnvironmentVariable("MC_FOLIAGE_BASIS_OUTPUT");
            if (string.IsNullOrWhiteSpace(output)) throw new InvalidOperationException("Set MC_FOLIAGE_BASIS_OUTPUT for the imported-mesh proof");
            var assets = new[] { "grass_medium_01_1k", "fern_02_1k", "shrub_02_1k", "tree_small_02_derived" };
            var names = new[] { "grass_medium_01_mid_b_LOD0", "fern_02_b", "shrub_02_b_LOD0", "tree_leaves" };
            var parts = new ImportedPart[assets.Length];
            for (var i = 0; i < parts.Length; i++)
            {
                var prefab = Resources.Load<GameObject>("ModernNature/" + assets[i]);
                if (prefab == null) throw new InvalidOperationException("Missing foliage basis resource: " + assets[i]);
                MeshFilter selected = null;
                foreach (var filter in prefab.GetComponentsInChildren<MeshFilter>(true)) if (filter.name == names[i]) selected = filter;
                if (selected == null || selected.sharedMesh == null) throw new InvalidOperationException("Missing imported foliage part: " + names[i]);
                var vertices = selected.sharedMesh.vertices;
                for (var v = 0; v < vertices.Length; v++) vertices[v] = selected.transform.localToWorldMatrix.MultiplyPoint3x4(vertices[v]);
                parts[i] = new ImportedPart { name = names[i], vertices = vertices };
            }
            System.IO.File.WriteAllText(output, JsonUtility.ToJson(new ImportedParts { parts = parts }));
            Debug.Log("MC_MODERN_FOLIAGE_BASIS exported " + parts.Length + " imported parts");
        }
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
            "tree_small_02_leaves_alpha_1k.png",
            "grass_lod_color.png", "fern_lod_color.png", "shrub_lod_color.png", "tree_leaves_lod_color.png",
            "grass_lod_normal_depth.png", "fern_lod_normal_depth.png", "shrub_lod_normal_depth.png", "tree_leaves_lod_normal_depth.png"
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
            var colorAtlas = path.EndsWith("_lod_color.png", StringComparison.Ordinal);
            var normalDepth = path.EndsWith("_lod_normal_depth.png", StringComparison.Ordinal);
            importer.sRGBTexture = path.Contains("_diff_") || colorAtlas;
            importer.mipmapEnabled = true;
            importer.textureCompression = normalDepth ? TextureImporterCompression.Uncompressed : TextureImporterCompression.CompressedHQ;
            importer.anisoLevel = 8;
            importer.maxTextureSize = 1024;
            if (colorAtlas || normalDepth)
            {
                importer.alphaSource = TextureImporterAlphaSource.FromInput;
                importer.alphaIsTransparency = !normalDepth;
                // Match the source cutout maps' coverage behavior. Preserving
                // an aggregate silhouette made distant fronds too solid.
                importer.mipMapsPreserveCoverage = false;
                importer.alphaTestReferenceValue = .4f;
                importer.npotScale = TextureImporterNPOTScale.None;
                importer.wrapMode = TextureWrapMode.Clamp;
                importer.filterMode = FilterMode.Bilinear;
            }
            importer.ClearPlatformTextureSettings("Standalone");
            if (colorAtlas)
            {
                var platform = importer.GetPlatformTextureSettings("Standalone");
                platform.name = "Standalone"; platform.overridden = true;
                platform.maxTextureSize = 1024; platform.format = TextureImporterFormat.BC7;
                platform.compressionQuality = 100;
                importer.SetPlatformTextureSettings(platform);
            }
        }

        // Called before the player build. Existing .meta files can predate this
        // postprocessor; reimport stale settings rather than trusting them.
        internal static void Prepare()
        {
            foreach (var name in new[] { "rock_moss_set_01_lod", "tree_small_02_lod" })
            {
                var path = Folder + name + ".fbx";
                var model = AssetImporter.GetAtPath(path) as ModelImporter;
                if (model == null) throw new InvalidOperationException("Missing generated nature model: " + path);
                if (!model.preserveHierarchy || !model.isReadable)
                {
                    model.preserveHierarchy = true;
                    model.isReadable = true;
                    model.SaveAndReimport();
                }
                var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(path);
                foreach (var filter in prefab.GetComponentsInChildren<MeshFilter>(true))
                    Debug.Log($"MC_MODERN_NATURE_MODEL asset={name} node={filter.name} mesh={filter.sharedMesh?.name}");
            }
            foreach (var name in RequiredMaps)
            {
                var path = Folder + name;
                var importer = AssetImporter.GetAtPath(path) as TextureImporter;
                if (importer == null) throw new InvalidOperationException("Missing nature texture importer: " + path);
                var colorAtlas = name.EndsWith("_lod_color.png", StringComparison.Ordinal);
                var normalDepth = name.EndsWith("_lod_normal_depth.png", StringComparison.Ordinal);
                var atlas = colorAtlas || normalDepth;
                var color = name.Contains("_diff_") || colorAtlas;
                var compression = normalDepth ? TextureImporterCompression.Uncompressed : TextureImporterCompression.CompressedHQ;
                var platform = importer.GetPlatformTextureSettings("Standalone");
                var stale = importer.textureType != TextureImporterType.Default ||
                    importer.textureShape != TextureImporterShape.Texture2D ||
                    importer.sRGBTexture != color || !importer.mipmapEnabled ||
                    importer.textureCompression != compression ||
                    importer.anisoLevel != 8 || importer.maxTextureSize != 1024 ||
                    platform.overridden != colorAtlas ||
                    (colorAtlas && (platform.format != TextureImporterFormat.BC7 || platform.maxTextureSize != 1024 || platform.compressionQuality != 100)) ||
                    (atlas && (importer.alphaSource != TextureImporterAlphaSource.FromInput ||
                        importer.alphaIsTransparency != colorAtlas || importer.mipMapsPreserveCoverage ||
                        importer.alphaTestReferenceValue != .4f || importer.npotScale != TextureImporterNPOTScale.None ||
                        importer.wrapMode != TextureWrapMode.Clamp || importer.filterMode != FilterMode.Bilinear)) ||
                    AssetDatabase.LoadAssetAtPath<Texture2D>(path) == null;
                if (stale)
                {
                    ConfigureTexture(importer, path);
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
            if (assetPath.EndsWith("_lod.fbx", StringComparison.Ordinal)) importer.preserveHierarchy = true;
        }
    }
}
