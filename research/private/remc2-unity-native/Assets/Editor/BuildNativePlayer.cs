using System.IO;
using UnityEditor;
using UnityEditor.Build;
using UnityEditor.Build.Reporting;
using UnityEngine;
using System;
using System.Linq;

namespace MagicCarpet.Native.Editor
{
    public static class BuildNativePlayer
    {
        private const string ScenePath = "Assets/Scenes/Main.unity";

        [MenuItem("Magic Carpet/Build Linux Player")]
        public static void BuildLinuxPlayer()
        {
            var scenes = EditorBuildSettings.scenes.Where(scene => scene.enabled)
                .Select(scene => scene.path).ToArray();
            if (scenes.Length == 0 || scenes[0] != ScenePath)
                throw new BuildFailedException(
                    $"Enable {ScenePath} as the first scene in Build Settings; authored settings were preserved.");
            foreach (var scene in scenes)
                if (!File.Exists(scene) || AssetDatabase.LoadAssetAtPath<SceneAsset>(scene) == null)
                    throw new BuildFailedException(
                        $"Build scene is missing or invalid: {scene}. Restore or fix the authored scene.");
            if (!File.Exists("Assets/Plugins/x86_64/libsim_capi.so"))
                throw new BuildFailedException("Missing native plugin. Run build-player.sh first.");

            var outputDirectory = Environment.GetEnvironmentVariable("MC_PLAYER_BUILD_OUTPUT")
                ?? "Builds/Linux";
            Directory.CreateDirectory(outputDirectory);
            var report = BuildPipeline.BuildPlayer(new BuildPlayerOptions
            {
                scenes = scenes,
                locationPathName = Path.Combine(outputDirectory, "MagicCarpetNative.x86_64"),
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

    }
}
