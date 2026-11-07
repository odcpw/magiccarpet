using System;
using System.IO;
using MagicCarpet.Native.Voxels;
using UnityEditor;
using UnityEditor.Build;
using UnityEditor.Build.Reporting;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;
namespace MagicCarpet.Native.Editor
{
    public static class BuildVoxelPlayer
    {
        public static void Build()
        {
            VoxelGoatImport.Prepare();
            var output = Environment.GetEnvironmentVariable("MC_VOXEL_PLAYER_OUTPUT");
            if (string.IsNullOrEmpty(output) || File.Exists(output)) throw new ArgumentException("Set a new MC_VOXEL_PLAYER_OUTPUT");
            if (Resources.Load<TextAsset>("ModernCharacters/VoxelSandbox/Level1") == null) throw new BuildFailedException("Run prepare-voxel-inputs.py first");
            var scene = EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);
            new GameObject("Level 1 voxel sandbox").AddComponent<VoxelSandbox>();
            const string path = "Assets/Scenes/VoxelSandbox.unity";
            EditorSceneManager.SaveScene(scene, path);
            var previous = GraphicsSettings.defaultRenderPipeline;
            var previousDeformation = PlayerSettings.meshDeformation;
            try {
                GraphicsSettings.defaultRenderPipeline = Resources.Load<UniversalRenderPipelineAsset>("ModernPipeline");
                // Fine voxel skins reuse the approved rigs but contain many
                // more vertices. Deform them on the GPU in this experimental
                // player without changing the normal campaign build settings.
                PlayerSettings.meshDeformation = MeshDeformation.GPUBatched;
                var report = BuildPipeline.BuildPlayer(new BuildPlayerOptions { scenes = new[]{path}, locationPathName=output, target=BuildTarget.StandaloneLinux64, options=BuildOptions.None });
                if(report.summary.result != BuildResult.Succeeded) throw new BuildFailedException("Voxel player build failed");
                Debug.Log("MC_VOXEL_PLAYER_BUILT " + output);
            } finally {
                GraphicsSettings.defaultRenderPipeline=previous;
                PlayerSettings.meshDeformation=previousDeformation;
            }
        }
    }
}
