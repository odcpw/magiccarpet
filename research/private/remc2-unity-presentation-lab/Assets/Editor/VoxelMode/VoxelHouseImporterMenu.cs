#if UNITY_EDITOR
using System;
using System.IO;
using UnityEditor;
using UnityEngine;

namespace MagicCarpet.PresentationLab.VoxelMode
{
    /// <summary>
    /// Thin editor UI over the pure importer. It only selects an external
    /// owner-data root and a private editor output directory; no gameplay
    /// scene, Resources asset, or per-cell GameObject is created here.
    /// </summary>
    internal static class VoxelHouseImporterMenu
    {
        private const string ImportTitle = "MC2 Private/Voxel Mode/Import qualified BUILD-29 package";
        private const string ValidateTitle = "MC2 Private/Voxel Mode/Validate qualified BUILD-29 package";

        [MenuItem(ImportTitle)]
        private static void Import()
        {
            var ownerRoot = EditorUtility.OpenFolderPanel(
                "Select external owner-derived BUILD-29 package root",
                string.Empty,
                string.Empty);
            if (string.IsNullOrEmpty(ownerRoot)) return;

            try
            {
                var outputDirectory = Path.Combine(
                    Application.dataPath,
                    "Editor",
                    "VoxelMode",
                    "Generated");
                var result = VoxelHouseImporter.ImportQualifiedBuild29(new VoxelHouseImportRequest
                {
                    OwnerRoot = ownerRoot,
                    OutputDirectory = outputDirectory,
                });
                AssetDatabase.Refresh();
                Debug.Log(
                    "Imported qualified BUILD-29 voxel mapping without GameObjects: " +
                    result.MappingPath +
                    " (" + result.CellCount + " cells, sha256=" + result.MappingSha256 + ")");
            }
            catch (Exception error)
            {
                Debug.LogError("BUILD-29 voxel import rejected: " + error.Message);
            }
        }

        [MenuItem(ValidateTitle)]
        private static void ValidateOnly()
        {
            var ownerRoot = EditorUtility.OpenFolderPanel(
                "Select external owner-derived BUILD-29 package root",
                string.Empty,
                string.Empty);
            if (string.IsNullOrEmpty(ownerRoot)) return;

            try
            {
                VoxelHouseImporter.ValidateOnly(new VoxelHouseImportRequest
                {
                    OwnerRoot = ownerRoot,
                });
                Debug.Log("Qualified BUILD-29 voxel package validated; no output was written.");
            }
            catch (Exception error)
            {
                Debug.LogError("BUILD-29 voxel package rejected: " + error.Message);
            }
        }
    }
}
#endif
