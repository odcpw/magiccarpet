#if UNITY_EDITOR
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace MagicCarpet.PresentationLab.Editor
{
    public static class CreatePresentationLabAssets
    {
        private const string RendererPath = "Assets/Settings/PresentationLabRenderer.asset";
        private const string PipelinePath = "Assets/Settings/PresentationLabURP.asset";

        [MenuItem("Magic Carpet/Presentation Lab/Create URP Assets")]
        public static void CreateRenderPipelineAssets()
        {
            EnsureFolder("Assets/Settings");

            UniversalRendererData renderer = AssetDatabase.LoadAssetAtPath<UniversalRendererData>(RendererPath);
            if (renderer == null)
            {
                renderer = ScriptableObject.CreateInstance<UniversalRendererData>();
                renderer.name = "PresentationLabRenderer";
                AssetDatabase.CreateAsset(renderer, RendererPath);
            }

            UniversalRenderPipelineAsset pipeline = AssetDatabase.LoadAssetAtPath<UniversalRenderPipelineAsset>(PipelinePath);
            if (pipeline == null)
            {
                pipeline = UniversalRenderPipelineAsset.Create(renderer);
                pipeline.name = "PresentationLabURP";
                AssetDatabase.CreateAsset(pipeline, PipelinePath);
            }

            GraphicsSettings.defaultRenderPipeline = pipeline;
            EditorUtility.SetDirty(renderer);
            EditorUtility.SetDirty(pipeline);
            AssetDatabase.SaveAssets();
            AssetDatabase.Refresh();
            Debug.Log("Presentation Lab URP assets created and assigned: " + PipelinePath);
        }

        [MenuItem("Magic Carpet/Presentation Lab/Validate Boot Scene")]
        public static void ValidateBootScene()
        {
            SceneAsset sceneAsset = AssetDatabase.LoadAssetAtPath<SceneAsset>("Assets/Scenes/Boot.unity");
            if (sceneAsset == null)
            {
                throw new System.InvalidOperationException("Boot scene asset is missing");
            }

            var scene = EditorSceneManager.OpenScene("Assets/Scenes/Boot.unity", OpenSceneMode.Single);
            if (!scene.IsValid() || scene.rootCount < 2)
            {
                throw new System.InvalidOperationException("Boot scene did not load with the router and camera roots");
            }

            Debug.Log("Presentation Lab Boot scene validated: " + scene.path);
            if (Application.isBatchMode)
            {
                EditorApplication.Exit(0);
            }
        }

        private static void EnsureFolder(string path)
        {
            if (AssetDatabase.IsValidFolder(path))
            {
                return;
            }

            string parent = System.IO.Path.GetDirectoryName(path).Replace('\\', '/');
            string name = System.IO.Path.GetFileName(path);
            AssetDatabase.CreateFolder(parent, name);
        }
    }
}
#endif
