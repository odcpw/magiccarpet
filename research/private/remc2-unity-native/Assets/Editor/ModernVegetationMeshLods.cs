using System;
using System.Diagnostics;
using System.IO;
using UnityEditor;
using UnityEngine;

namespace MagicCarpet.Native.Editor
{
    // Explicit authoring step. It creates the exact normalized meshes used by
    // the qualified pilot; the player only loads the resulting Mesh assets.
    public static class ModernVegetationMeshLods
    {
        private const string Folder = "Assets/Resources/ModernNature/";
        private const int Count = 7;

        public static void Generate()
        {
            for (var i = 0; i < Count; i++)
            {
                var path = Folder + ModernVegetation.IntermediateResourceName(i) + ".asset";
                if (File.Exists(path) || File.Exists(path + ".meta") || AssetDatabase.LoadMainAssetAtPath(path) != null)
                    throw new InvalidOperationException("Refusing to overwrite existing vegetation asset: " + path);
            }
            var meshes = new Mesh[Count];
            try
            {
                for (var i = 0; i < Count; i++)
                {
                    var timer = Stopwatch.StartNew();
                    meshes[i] = ModernVegetation.CreateIntermediateForAuthoring(i);
                    meshes[i].name = ModernVegetation.IntermediateResourceName(i);
                    timer.Stop();
                    UnityEngine.Debug.Log($"MC_MODERN_MID_AUTHOR name={meshes[i].name} triangles={meshes[i].GetIndexCount(0) / 3} ms={timer.Elapsed.TotalMilliseconds:F3}");
                }
                for (var i = 0; i < Count; i++)
                    AssetDatabase.CreateAsset(meshes[i], Folder + meshes[i].name + ".asset");
                AssetDatabase.SaveAssets();
                AssetDatabase.Refresh();
                Validate();
            }
            finally
            {
                foreach (var mesh in meshes)
                    if (mesh != null && !AssetDatabase.Contains(mesh)) UnityEngine.Object.DestroyImmediate(mesh);
            }
        }

        internal static void Validate()
        {
            for (var i = 0; i < Count; i++)
            {
                var name = ModernVegetation.IntermediateResourceName(i);
                var path = Folder + name + ".asset";
                var asset = AssetDatabase.LoadAssetAtPath<Mesh>(path);
                var resource = Resources.Load<Mesh>("ModernNature/" + name);
                if (asset == null || resource == null || asset != resource || !asset.isReadable ||
                    asset.vertexCount == 0 || asset.subMeshCount == 0 || asset.GetIndexCount(0) == 0)
                    throw new InvalidOperationException("Missing or invalid authored vegetation mesh: " + path);
            }
            UnityEngine.Debug.Log("MC_MODERN_MID_AUTHOR validated " + Count + " prebuilt meshes");
        }
    }
}
