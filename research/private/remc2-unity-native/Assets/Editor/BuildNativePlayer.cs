using System.IO;
using UnityEditor;
using UnityEditor.Build;
using UnityEditor.Build.Reporting;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;

namespace MagicCarpet.Native.Editor
{
    public static class BuildNativePlayer
    {
        private const string ScenePath = "Assets/Scenes/Main.unity";

        [MenuItem("Magic Carpet/Build Linux Player")]
        public static void BuildLinuxPlayer()
        {
            EnsureScene();
            PlayerSettings.productName = "Magic Carpet Native Harness";
            PlayerSettings.companyName = "Owner-local preservation lab";
            PlayerSettings.defaultScreenWidth = 1600;
            PlayerSettings.defaultScreenHeight = 900;
            PlayerSettings.fullScreenMode = FullScreenMode.FullScreenWindow;
            PlayerSettings.runInBackground = true;

            Directory.CreateDirectory("Builds/Linux");
            var report = BuildPipeline.BuildPlayer(new BuildPlayerOptions
            {
                scenes = new[] { ScenePath },
                locationPathName = "Builds/Linux/MagicCarpetNative.x86_64",
                target = BuildTarget.StandaloneLinux64,
                // The qualified owner presentation must not leak Unity's
                // default "Development Build" overlay into HUD captures.
                options = BuildOptions.None
            });

            if (report.summary.result != BuildResult.Succeeded)
            {
                throw new BuildFailedException(
                    $"Linux player build failed: {report.summary.result} " +
                    $"({report.summary.totalErrors} errors)");
            }

            Debug.Log(
                $"Built native harness: {report.summary.outputPath} " +
                $"({report.summary.totalSize} bytes)");
        }

        private static void EnsureScene()
        {
            Directory.CreateDirectory("Assets/Scenes");
            var scene = EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);
            scene.name = "Main";
            new GameObject("Magic Carpet Runtime").AddComponent<MagicCarpetRuntime>();
            EditorSceneManager.SaveScene(scene, ScenePath);
            EditorBuildSettings.scenes = new[]
            {
                new EditorBuildSettingsScene(ScenePath, true)
            };
        }
    }
}
