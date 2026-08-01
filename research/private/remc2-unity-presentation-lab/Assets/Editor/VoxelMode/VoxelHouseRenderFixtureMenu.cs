#if UNITY_EDITOR
using System;
using System.IO;
using UnityEditor;
using UnityEngine;

namespace MagicCarpet.PresentationLab.VoxelMode
{
    internal static class VoxelHouseRenderFixtureMenu
    {
        [MenuItem("MC2 Private/Voxel Mode/Build deterministic BUILD-29 state fixture report")]
        private static void BuildReport()
        {
            var mappingPath = EditorUtility.OpenFilePanel(
                "Select voxel-house-build29.mapping.json",
                string.Empty,
                "json");
            if (string.IsNullOrEmpty(mappingPath)) return;

            try
            {
                var fixture = VoxelHouseRenderFixture.LoadFromMapping(mappingPath);
                var outputDirectory = Path.Combine(Application.dataPath, "Editor", "VoxelMode", "Generated");
                Directory.CreateDirectory(outputDirectory);
                var outputPath = Path.Combine(outputDirectory, "voxel-house-build29.fixture-report.json");
                File.WriteAllText(outputPath, fixture.BuildReportJson(), new System.Text.UTF8Encoding(false));
                AssetDatabase.Refresh();
                Debug.Log("Wrote deterministic BUILD-29 state fixture report: " + outputPath);
            }
            catch (Exception error)
            {
                Debug.LogError("BUILD-29 fixture report rejected: " + error.Message);
            }
        }
    }
}
#endif
