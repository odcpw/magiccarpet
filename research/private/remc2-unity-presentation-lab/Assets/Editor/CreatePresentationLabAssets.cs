#if UNITY_EDITOR
using System;
using System.IO;
using UnityEditor;
using UnityEditor.Build.Reporting;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;
using UnityEngine.XR.Management;
using UnityEditor.XR.Management;
using UnityEditor.XR.Management.Metadata;

namespace MagicCarpet.PresentationLab.Editor
{
    public static class CreatePresentationLabAssets
    {
        private const string RendererPath = "Assets/Settings/PresentationLabRenderer.asset";
        private const string PipelinePath = "Assets/Settings/PresentationLabURP.asset";
        private const string XrSmokeScenePath = "Assets/Scenes/OpenXrSmoke.unity";
        private const string BuildPathArgument = "-mc-build-path";
        private const string XrGeneralSettingsPath = "Assets/XR/XRGeneralSettingsPerBuildTarget.asset";
        private const string OpenXrLoaderType = "UnityEngine.XR.OpenXR.OpenXRLoader";

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

        [MenuItem("Magic Carpet/Presentation Lab/Build OpenXR Smoke Player")]
        public static void BuildOpenXrSmokePlayer()
        {
            string outputPath = CommandLineValue(Environment.GetCommandLineArgs(), BuildPathArgument);
            if (string.IsNullOrWhiteSpace(outputPath))
            {
                throw new InvalidOperationException(
                    "OpenXR smoke build requires -mc-build-path <player-output-path>");
            }

            string projectRoot = Directory.GetParent(Application.dataPath).FullName;
            string sceneOnDisk = Path.Combine(projectRoot, XrSmokeScenePath.Replace('/', Path.DirectorySeparatorChar));
            if (!File.Exists(sceneOnDisk))
            {
                throw new InvalidOperationException("OpenXR smoke scene is missing: " + XrSmokeScenePath);
            }

            BuildTarget target = EditorUserBuildSettings.activeBuildTarget;
            if (target != BuildTarget.StandaloneLinux64 && target != BuildTarget.StandaloneWindows64)
            {
                throw new InvalidOperationException(
                    "OpenXR smoke player supports StandaloneLinux64 or StandaloneWindows64; active target was " + target);
            }

            string outputDirectory = Path.GetDirectoryName(Path.GetFullPath(outputPath));
            if (!string.IsNullOrWhiteSpace(outputDirectory))
            {
                Directory.CreateDirectory(outputDirectory);
            }

            EnsureStandaloneOpenXrSettings();

            BuildPlayerOptions options = new BuildPlayerOptions
            {
                scenes = new[] { XrSmokeScenePath },
                locationPathName = outputPath,
                target = target,
                options = BuildOptions.None,
            };
            BuildReport report = BuildPipeline.BuildPlayer(options);
            if (report.summary.result != BuildResult.Succeeded)
            {
                throw new InvalidOperationException(
                    "OpenXR smoke player build failed for " + target + ": " + report.summary.result);
            }

            Debug.Log("OpenXR smoke player built: target=" + target + ", scene=" + XrSmokeScenePath +
                      ", output=" + outputPath);
            if (Application.isBatchMode)
            {
                EditorApplication.Exit(0);
            }
        }

        private static void EnsureStandaloneOpenXrSettings()
        {
            XRGeneralSettingsPerBuildTarget generalSettings =
                AssetDatabase.LoadAssetAtPath<XRGeneralSettingsPerBuildTarget>(XrGeneralSettingsPath);
            if (generalSettings == null)
            {
                throw new InvalidOperationException("XR general settings asset is missing: " + XrGeneralSettingsPath);
            }

            if (!generalSettings.HasSettingsForBuildTarget(BuildTargetGroup.Standalone))
            {
                generalSettings.CreateDefaultSettingsForBuildTarget(BuildTargetGroup.Standalone);
            }

            if (!generalSettings.HasManagerSettingsForBuildTarget(BuildTargetGroup.Standalone))
            {
                generalSettings.CreateDefaultManagerSettingsForBuildTarget(BuildTargetGroup.Standalone);
            }

            XRManagerSettings manager = generalSettings.ManagerSettingsForBuildTarget(BuildTargetGroup.Standalone);
            if (manager == null)
            {
                throw new InvalidOperationException("Standalone XR manager settings could not be created");
            }

            if (!XRPackageMetadataStore.IsLoaderAssigned(OpenXrLoaderType, BuildTargetGroup.Standalone) &&
                !XRPackageMetadataStore.AssignLoader(manager, OpenXrLoaderType, BuildTargetGroup.Standalone))
            {
                throw new InvalidOperationException("Could not assign OpenXR loader to Standalone XR settings");
            }

            EditorBuildSettings.AddConfigObject(XRGeneralSettings.settingsKey, generalSettings, true);
            EditorUtility.SetDirty(generalSettings);
            EditorUtility.SetDirty(manager);
            AssetDatabase.SaveAssets();
            AssetDatabase.Refresh();
            Debug.Log("Standalone OpenXR manager configured: loader=" + OpenXrLoaderType);
        }

        private static string CommandLineValue(string[] arguments, string name)
        {
            for (int index = 0; index < (arguments == null ? 0 : arguments.Length); index++)
            {
                if (string.Equals(arguments[index], name, StringComparison.OrdinalIgnoreCase))
                {
                    return index + 1 < arguments.Length ? arguments[index + 1] : string.Empty;
                }

                string prefix = name + "=";
                if (arguments[index].StartsWith(prefix, StringComparison.OrdinalIgnoreCase))
                {
                    return arguments[index].Substring(prefix.Length);
                }
            }

            return string.Empty;
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
