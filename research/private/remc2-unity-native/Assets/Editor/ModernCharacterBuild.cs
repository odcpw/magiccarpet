using System;
using System.IO;
using UnityEditor;
using UnityEditor.Build;
using UnityEditor.Build.Reporting;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace MagicCarpet.Native.Editor
{
    public static class ModernCharacterBuild
    {
        public static void Build()
        {
            ModernCharacterImport.Prepare();
            BuildModernPlayer.Prepare();
            var output = Environment.GetEnvironmentVariable("MC_CHARACTER_PLAYER_OUTPUT");
            if (string.IsNullOrEmpty(output) || File.Exists(output)) throw new ArgumentException("Set a new MC_CHARACTER_PLAYER_OUTPUT");
            var scene = EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);
            new GameObject("Character inspection").AddComponent<ModernCharacterReview>();
            const string scenePath = "Assets/Scenes/CharacterReview.unity";
            EditorSceneManager.SaveScene(scene, scenePath);
            var previous = GraphicsSettings.defaultRenderPipeline;
            try
            {
                GraphicsSettings.defaultRenderPipeline = AssetDatabase.LoadAssetAtPath<UniversalRenderPipelineAsset>("Assets/Resources/ModernPipeline.asset");
                var report = BuildPipeline.BuildPlayer(new BuildPlayerOptions {
                    scenes = new[] { scenePath }, locationPathName = output,
                    target = BuildTarget.StandaloneLinux64, options = BuildOptions.None
                });
                if (report.summary.result != BuildResult.Succeeded) throw new BuildFailedException("Character player build failed");
                Debug.Log("MC_CHARACTER_PLAYER_BUILT " + output);
            }
            finally { GraphicsSettings.defaultRenderPipeline = previous; }
        }
    }
}
