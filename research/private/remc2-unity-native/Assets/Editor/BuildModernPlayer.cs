using System;
using System.IO;
using UnityEditor;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace MagicCarpet.Native.Editor
{
    public static class BuildModernPlayer
    {
        // Author assets once; both presentation modes continue to use the same native session.
        public static void Prepare()
        {
            ModernCharacterImport.Validate();
            ModernNatureImport.Prepare();
            ModernVegetationMeshLods.Validate();
            ModernTerrainTextures.Prepare();
            foreach (var texturePath in Directory.GetFiles("Assets/Resources/ModernTextures", "*.jpg"))
            {
                var importer = (TextureImporter)AssetImporter.GetAtPath(texturePath);
                if (importer == null) continue;
                var colorTexture = texturePath.Contains("Albedo");
                var changed = importer.textureType != TextureImporterType.Default ||
                    importer.textureShape != TextureImporterShape.Texture2D ||
                    importer.sRGBTexture != colorTexture || !importer.mipmapEnabled ||
                    importer.anisoLevel != 8 || importer.maxTextureSize != 2048 ||
                    importer.textureCompression != TextureImporterCompression.CompressedHQ;
                importer.textureType = TextureImporterType.Default;
                importer.textureShape = TextureImporterShape.Texture2D;
                importer.sRGBTexture = colorTexture;
                importer.mipmapEnabled = true;
                importer.anisoLevel = 8;
                importer.maxTextureSize = 2048;
                importer.textureCompression = TextureImporterCompression.CompressedHQ;
                if (changed) importer.SaveAndReimport();
                if (AssetDatabase.LoadAssetAtPath<Texture2D>(texturePath) == null)
                    throw new InvalidOperationException($"Terrain texture failed 2D import: {texturePath}");
            }
            const string path = "Assets/Resources/ModernPipeline.asset";
            var pipeline = AssetDatabase.LoadAssetAtPath<UniversalRenderPipelineAsset>(path);
            if (pipeline == null)
            {
                var renderer = ScriptableObject.CreateInstance<UniversalRendererData>();
                renderer.renderingMode = RenderingMode.ForwardPlus;
                AssetDatabase.CreateAsset(renderer, "Assets/Resources/ModernPipelineRenderer.asset");
                pipeline = UniversalRenderPipelineAsset.Create(renderer);
                AssetDatabase.CreateAsset(pipeline, path);
            }
            var modernRenderer = AssetDatabase.LoadAssetAtPath<UniversalRendererData>(
                "Assets/Resources/ModernPipelineRenderer.asset");
            if (modernRenderer == null) throw new InvalidOperationException("Modern renderer asset is missing");
            // Water, soft spray and steam read scene depth in their transparent
            // passes. URP's default AfterTransparents supplies it too late.
            modernRenderer.copyDepthMode = CopyDepthMode.AfterOpaques;
            EditorUtility.SetDirty(modernRenderer);
            pipeline.supportsHDR = true;
            pipeline.supportsCameraDepthTexture = true;
            pipeline.supportsCameraOpaqueTexture = false;
            pipeline.msaaSampleCount = 4;
            pipeline.renderScale = 1f;
            pipeline.mainLightShadowmapResolution = 4096;
            pipeline.shadowDistance = 230f;
            pipeline.shadowCascadeCount = 4;
            pipeline.cascade4Split = new Vector3(.08f, .22f, .48f);
            var serialized = new SerializedObject(pipeline);
            serialized.FindProperty("m_MainLightShadowsSupported").boolValue = true;
            serialized.FindProperty("m_MainLightRenderingMode").intValue = (int)LightRenderingMode.PerPixel;
            serialized.FindProperty("m_AdditionalLightsRenderingMode").intValue = (int)LightRenderingMode.PerPixel;
            serialized.FindProperty("m_SoftShadowsSupported").boolValue = true;
            serialized.ApplyModifiedPropertiesWithoutUndo();
            pipeline.shadowDepthBias = .5f;
            pipeline.shadowNormalBias = .5f;
            const string globalsPath = "Assets/Resources/ModernPipelineGlobalSettings.asset";
            var globals = GraphicsSettings.GetSettingsForRenderPipeline<UniversalRenderPipeline>();
            if (globals != null && AssetDatabase.GetAssetPath(globals) != globalsPath)
            {
                var error = AssetDatabase.MoveAsset(AssetDatabase.GetAssetPath(globals), globalsPath);
                if (!string.IsNullOrEmpty(error)) throw new InvalidOperationException(error);
            }
            if (globals == null)
            {
                globals = AssetDatabase.LoadAssetAtPath<RenderPipelineGlobalSettings>(globalsPath);
                if (globals == null) throw new InvalidOperationException("Restore the authored ModernPipelineGlobalSettings asset");
                GraphicsSettings.RegisterRenderPipelineSettings<UniversalRenderPipeline>(globals);
            }
            EditorUtility.SetDirty(pipeline);
            AssetDatabase.SaveAssets();
            Debug.Log("MC_MODERN_PIPELINE prepared URP 17.3 desktop environment");
        }

        public static void BuildLinuxPlayer()
        {
            Prepare();
            var previous = GraphicsSettings.defaultRenderPipeline;
            var previousQuality = QualitySettings.renderPipeline;
            var previousDeformation = PlayerSettings.meshDeformation;
            try
            {
                // URP's build processors must see the asset to include its runtime resources.
                QualitySettings.renderPipeline = null;
                GraphicsSettings.defaultRenderPipeline = AssetDatabase.LoadAssetAtPath<UniversalRenderPipelineAsset>(
                    "Assets/Resources/ModernPipeline.asset");
                PlayerSettings.meshDeformation = MeshDeformation.GPUBatched;
                AssetDatabase.SaveAssets();
                BuildNativePlayer.BuildLinuxPlayer();
            }
            finally
            {
                GraphicsSettings.defaultRenderPipeline = previous;
                QualitySettings.renderPipeline = previousQuality;
                PlayerSettings.meshDeformation = previousDeformation;
                AssetDatabase.SaveAssets();
            }
        }
    }
}
