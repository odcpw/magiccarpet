#if UNITY_EDITOR
using System;
using UnityEditor;
using UnityEngine;
using UnityEngine.Rendering.Universal;

namespace MagicCarpet.PresentationLab.Editor
{
    /// <summary>
    /// Creates the serialized renderer variant needed by URP player builds.
    /// The default renderer remains the Exact/Classic HD renderer; the
    /// Enhanced camera selects this second renderer only while Whole Hog or
    /// High quality is active.
    /// </summary>
    public static class EnsureEnhancedRendererAsset
    {
        private const string DefaultRendererPath = "Assets/Settings/PresentationLabRenderer.asset";
        private const string EnhancedRendererPath = "Assets/Settings/PresentationLabEnhancedRenderer.asset";
        private const string PipelinePath = "Assets/Settings/PresentationLabURP.asset";
        private const string EnhancedRendererName = "PresentationLabEnhancedRenderer";

        [MenuItem("Magic Carpet/Presentation Lab/Ensure Enhanced Renderer Asset")]
        public static void Ensure()
        {
            UniversalRendererData defaultRenderer =
                AssetDatabase.LoadAssetAtPath<UniversalRendererData>(DefaultRendererPath);
            UniversalRenderPipelineAsset pipeline =
                AssetDatabase.LoadAssetAtPath<UniversalRenderPipelineAsset>(PipelinePath);
            if (defaultRenderer == null || pipeline == null)
            {
                throw new InvalidOperationException(
                    "Presentation Lab URP assets must exist before creating the Enhanced renderer");
            }

            UniversalRendererData enhancedRenderer =
                AssetDatabase.LoadAssetAtPath<UniversalRendererData>(EnhancedRendererPath);
            if (enhancedRenderer == null)
            {
                enhancedRenderer = UnityEngine.Object.Instantiate(defaultRenderer);
                enhancedRenderer.name = EnhancedRendererName;
                AssetDatabase.CreateAsset(enhancedRenderer, EnhancedRendererPath);
            }

            ScreenSpaceAmbientOcclusion ambientOcclusion = null;
            for (int index = 0; index < enhancedRenderer.rendererFeatures.Count; index++)
            {
                if (enhancedRenderer.rendererFeatures[index] is ScreenSpaceAmbientOcclusion existing)
                {
                    ambientOcclusion = existing;
                    break;
                }
            }

            if (ambientOcclusion == null)
            {
                ambientOcclusion = ScriptableObject.CreateInstance<ScreenSpaceAmbientOcclusion>();
                ambientOcclusion.name = "Enhanced Screen Space Ambient Occlusion";
                enhancedRenderer.rendererFeatures.Add(ambientOcclusion);
                AssetDatabase.AddObjectToAsset(ambientOcclusion, enhancedRenderer);
            }

            ambientOcclusion.SetActive(true);
            ConfigureAmbientOcclusion(ambientOcclusion);
            enhancedRenderer.SetDirty();
            EnsurePipelineRendererReference(pipeline, enhancedRenderer);
            EditorUtility.SetDirty(enhancedRenderer);
            EditorUtility.SetDirty(pipeline);
            AssetDatabase.SaveAssets();
            AssetDatabase.Refresh();
            Debug.Log(
                "Enhanced renderer asset ensured: " + EnhancedRendererPath +
                " with active URP ScreenSpaceAmbientOcclusion (AfterOpaque, Depth)");
        }

        private static void ConfigureAmbientOcclusion(ScreenSpaceAmbientOcclusion feature)
        {
            SerializedObject serializedFeature = new SerializedObject(feature);
            SerializedProperty settings = serializedFeature.FindProperty("m_Settings");
            if (settings == null)
            {
                throw new InvalidOperationException("URP SSAO settings property is missing");
            }

            SetBool(settings, "AfterOpaque", true);
            SetBool(settings, "Downsample", false);
            SetEnum(settings, "AOMethod", 1); // Interleaved Gradient; no blue-noise dependency.
            SetEnum(settings, "Source", 0); // Depth; avoids a normals prepass on the generated mesh.
            SetEnum(settings, "NormalSamples", 1);
            SetFloat(settings, "Intensity", 2.0f);
            SetFloat(settings, "DirectLightingStrength", 0.25f);
            SetFloat(settings, "Radius", 0.035f);
            SetEnum(settings, "Samples", 1);
            SetEnum(settings, "BlurQuality", 0);
            SetFloat(settings, "Falloff", 100f);
            serializedFeature.ApplyModifiedPropertiesWithoutUndo();
        }

        private static void EnsurePipelineRendererReference(
            UniversalRenderPipelineAsset pipeline,
            UniversalRendererData enhancedRenderer)
        {
            SerializedObject serializedPipeline = new SerializedObject(pipeline);
            SerializedProperty rendererDataList = serializedPipeline.FindProperty("m_RendererDataList");
            if (rendererDataList == null)
            {
                throw new InvalidOperationException("URP renderer data list is missing");
            }

            for (int index = 0; index < rendererDataList.arraySize; index++)
            {
                if (rendererDataList.GetArrayElementAtIndex(index).objectReferenceValue == enhancedRenderer)
                {
                    serializedPipeline.ApplyModifiedPropertiesWithoutUndo();
                    return;
                }
            }

            int newIndex = rendererDataList.arraySize;
            rendererDataList.InsertArrayElementAtIndex(newIndex);
            rendererDataList.GetArrayElementAtIndex(newIndex).objectReferenceValue = enhancedRenderer;
            serializedPipeline.ApplyModifiedPropertiesWithoutUndo();
        }

        private static void SetBool(SerializedProperty parent, string name, bool value)
        {
            SerializedProperty property = parent.FindPropertyRelative(name);
            if (property == null) throw new InvalidOperationException("SSAO property is missing: " + name);
            property.boolValue = value;
        }

        private static void SetEnum(SerializedProperty parent, string name, int value)
        {
            SerializedProperty property = parent.FindPropertyRelative(name);
            if (property == null) throw new InvalidOperationException("SSAO property is missing: " + name);
            property.enumValueIndex = value;
        }

        private static void SetFloat(SerializedProperty parent, string name, float value)
        {
            SerializedProperty property = parent.FindPropertyRelative(name);
            if (property == null) throw new InvalidOperationException("SSAO property is missing: " + name);
            property.floatValue = value;
        }
    }
}
#endif
