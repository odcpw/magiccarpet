using System;
using System.IO;
using MagicCarpet.Native.Voxels;
using UnityEditor;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace MagicCarpet.Native.Editor
{
    // Explicit, owner-local authoring step. The player only loads these shared
    // meshes; texture readback and surface sampling never run during play.
    public static class VoxelVegetationBaker
    {
        private const string Folder = "Assets/Resources/ModernNature/Voxels";

        public static void Generate()
        {
            if (SystemInfo.graphicsDeviceType == GraphicsDeviceType.Null)
                throw new InvalidOperationException("Vegetation authoring needs texture readback on a graphics device");
            for (int kind = 0; kind < VoxelVegetationMesh.KindCount; kind++)
            for (int lod = 0; lod < VoxelVegetationMesh.LodCount; lod++)
            {
                var path = Folder + "/" + VoxelVegetationMesh.ResourceName(kind, lod) + ".asset";
                if (File.Exists(path) || File.Exists(path + ".meta") || AssetDatabase.LoadMainAssetAtPath(path) != null)
                    throw new InvalidOperationException("Refusing to overwrite a vegetation mesh: " + path);
            }
            var built = new Mesh[VoxelVegetationMesh.KindCount, VoxelVegetationMesh.LodCount];
            try
            {
                for (int kind = 0; kind < VoxelVegetationMesh.KindCount; kind++)
                {
                    var source = Resources.Load<Mesh>("ModernNature/" + ModernVegetation.IntermediateResourceName(kind));
                    var texture = VoxelVegetationMesh.TextureName(kind);
                    if (source == null) throw new InvalidOperationException("Missing vegetation source for kind " + kind);
                    var albedo = Read(Resources.Load<Texture2D>("ModernNature/" + texture + "_diff_1k"));
                    var alpha = VoxelVegetationMesh.UsesAlpha(kind)
                        ? Read(Resources.Load<Texture2D>("ModernNature/" + texture + "_alpha_1k")) : null;
                    for (int lod = 0; lod < VoxelVegetationMesh.LodCount; lod++)
                    {
                        var result = VoxelVegetationMesh.Build(source, VoxelVegetationMesh.Resolution(lod), albedo, alpha);
                        built[kind, lod] = result.Mesh;
                        result.Mesh.name = VoxelVegetationMesh.ResourceName(kind, lod);
                        Debug.Log($"MC_VOXEL_NATURE_BAKE kind={kind} lod={lod} cells={result.Cells} faces={result.Faces} triangles={result.Faces*2} alpha_rejected={result.AlphaRejected} interior_faces_culled={result.InteriorFacesCulled} cell={result.CellSize}");
                    }
                }
                if (!AssetDatabase.IsValidFolder(Folder)) AssetDatabase.CreateFolder("Assets/Resources/ModernNature", "Voxels");
                for (int kind = 0; kind < VoxelVegetationMesh.KindCount; kind++)
                for (int lod = 0; lod < VoxelVegetationMesh.LodCount; lod++)
                    AssetDatabase.CreateAsset(built[kind, lod], Folder + "/" + VoxelVegetationMesh.ResourceName(kind, lod) + ".asset");
                AssetDatabase.SaveAssets(); AssetDatabase.Refresh();
            }
            finally
            {
                foreach (var mesh in built)
                    if (mesh != null && !AssetDatabase.Contains(mesh)) UnityEngine.Object.DestroyImmediate(mesh);
            }
        }

        // Bake the near voxel geometry into the same 8x5 relightable atlas
        // contract as ModernVegetationImpostor. Four foliage kinds retain the
        // existing two-triangle distant draw, with voxel-derived silhouettes.
        public static void GenerateImpostors()
        {
            if (SystemInfo.graphicsDeviceType == GraphicsDeviceType.Null)
                throw new InvalidOperationException("Voxel impostor authoring needs a graphics device");
            if (!AssetDatabase.IsValidFolder(Folder)) throw new InvalidOperationException("Bake voxel meshes first");
            foreach (int kind in new[] { 0, 1, 2, 6 })
                foreach (var suffix in new[] { "_color.png", "_normal_depth.png" })
                {
                    var path = Folder + "/" + VoxelVegetationMesh.ImpostorName(kind) + suffix;
                    if (File.Exists(path) || File.Exists(path + ".meta"))
                        throw new InvalidOperationException("Refusing to overwrite an impostor: " + path);
                }
            var shader = Resources.Load<Shader>("VoxelVegetationImpostorBake");
            var pipeline = Resources.Load<UniversalRenderPipelineAsset>("ModernPipeline");
            if (shader == null || pipeline == null) throw new InvalidOperationException("Missing voxel impostor bake shader or pipeline");
            var previousPipeline = GraphicsSettings.defaultRenderPipeline; var previousQuality = QualitySettings.renderPipeline;
            var previousTarget = RenderTexture.active;
            var cameraObject = new GameObject("Voxel foliage bake camera"); var camera = cameraObject.AddComponent<Camera>();
            var obj = new GameObject("Voxel foliage bake mesh"); obj.layer = 31;
            var filter = obj.AddComponent<MeshFilter>(); var renderer = obj.AddComponent<MeshRenderer>();
            var material = new Material(shader); renderer.sharedMaterial = material;
            var colorTarget = new RenderTexture(112,112,24,RenderTextureFormat.ARGB32,RenderTextureReadWrite.sRGB);
            var dataTarget = new RenderTexture(112,112,24,RenderTextureFormat.ARGB32,RenderTextureReadWrite.Linear);
            var colorTile = new Texture2D(112,112,TextureFormat.RGBA32,false,false);
            var dataTile = new Texture2D(112,112,TextureFormat.RGBA32,false,true);
            try
            {
                GraphicsSettings.defaultRenderPipeline = pipeline; QualitySettings.renderPipeline = null;
                camera.enabled = false; camera.cullingMask = 1 << 31; camera.orthographic = true;
                camera.clearFlags = CameraClearFlags.SolidColor; camera.backgroundColor = Color.clear;
                camera.allowHDR = false; camera.allowMSAA = false;
                foreach (int kind in new[] { 0, 1, 2, 6 })
                {
                    var mesh = Resources.Load<Mesh>("ModernNature/Voxels/" + VoxelVegetationMesh.ResourceName(kind, 0));
                    if (mesh == null) throw new InvalidOperationException("Missing near voxel mesh for impostor " + kind);
                    filter.sharedMesh = mesh;
                    var center = mesh.bounds.center; float radiusSquared = 0;
                    foreach (var p in mesh.vertices) radiusSquared = Mathf.Max(radiusSquared, (p-center).sqrMagnitude);
                    float diameter = Mathf.Sqrt(radiusSquared) * 2.02f;
                    if (diameter <= 0) throw new InvalidOperationException("Empty voxel impostor geometry");
                    camera.orthographicSize = diameter*.5f; camera.nearClipPlane = diameter*.01f; camera.farClipPlane = diameter*5;
                    material.SetVector("_BakeCenter", center); material.SetFloat("_BakeDiameter", diameter);
                    var colorAtlas = new Texture2D(1024,640,TextureFormat.RGBA32,false,false);
                    var dataAtlas = new Texture2D(1024,640,TextureFormat.RGBA32,false,true);
                    try
                    {
                        for (int elevation = 0; elevation < 5; elevation++)
                        for (int azimuth = 0; azimuth < 8; azimuth++)
                        {
                            float e = -Mathf.PI*.5f + elevation*Mathf.PI/4f, a = azimuth*Mathf.PI/4f;
                            float ce = Mathf.Cos(e), se = Mathf.Sin(e), ca = Mathf.Cos(a), sa = Mathf.Sin(a);
                            var direction = new Vector3(ce*ca,se,ce*sa);
                            var up = new Vector3(-se*ca,ce,-se*sa);
                            camera.transform.position = center + direction*diameter*2;
                            camera.transform.rotation = Quaternion.LookRotation(-direction, up);
                            material.SetVector("_BakeDirection", direction);
                            Color32[] Capture(RenderTexture target, Texture2D tile, float data)
                            {
                                material.SetFloat("_CaptureData", data); camera.targetTexture = target;
                                camera.Render(); RenderTexture.active = target;
                                tile.ReadPixels(new Rect(0,0,112,112),0,0); tile.Apply();
                                return tile.GetPixels32();
                            }
                            var color = Capture(colorTarget, colorTile, 0); var data = Capture(dataTarget, dataTile, 1);
                            var packed = Pad(color, data);
                            colorAtlas.SetPixels32(azimuth*128,elevation*128,128,128,packed.color);
                            dataAtlas.SetPixels32(azimuth*128,elevation*128,128,128,packed.data);
                        }
                        colorAtlas.Apply(); dataAtlas.Apply();
                        var prefix = Folder + "/" + VoxelVegetationMesh.ImpostorName(kind);
                        File.WriteAllBytes(prefix + "_color.png", colorAtlas.EncodeToPNG());
                        File.WriteAllBytes(prefix + "_normal_depth.png", dataAtlas.EncodeToPNG());
                        ImportAtlas(prefix + "_color.png", true); ImportAtlas(prefix + "_normal_depth.png", false);
                        Debug.Log($"MC_VOXEL_NATURE_IMPOSTOR kind={kind} frames=40 triangles=2 diameter={diameter}");
                    }
                    finally { UnityEngine.Object.DestroyImmediate(colorAtlas); UnityEngine.Object.DestroyImmediate(dataAtlas); }
                }
                AssetDatabase.SaveAssets(); AssetDatabase.Refresh();
            }
            finally
            {
                camera.targetTexture = null; RenderTexture.active = previousTarget;
                GraphicsSettings.defaultRenderPipeline = previousPipeline; QualitySettings.renderPipeline = previousQuality;
                colorTarget.Release(); dataTarget.Release();
                UnityEngine.Object.DestroyImmediate(colorTarget); UnityEngine.Object.DestroyImmediate(dataTarget);
                UnityEngine.Object.DestroyImmediate(colorTile); UnityEngine.Object.DestroyImmediate(dataTile);
                UnityEngine.Object.DestroyImmediate(material); UnityEngine.Object.DestroyImmediate(obj); UnityEngine.Object.DestroyImmediate(cameraObject);
            }
        }

        private static (Color32[] color, Color32[] data) Pad(Color32[] color, Color32[] data)
        {
            var c = new Color32[128*128]; var d = new Color32[c.Length]; var occupied = new bool[c.Length];
            var coverage = new byte[c.Length];
            for (int y = 0; y < 112; y++) for (int x = 0; x < 112; x++)
            {
                int source = x+y*112, target = x+8+(y+8)*128;
                c[target] = color[source]; d[target] = data[source]; coverage[target] = color[source].a;
                occupied[target] = coverage[target] >= 8;
            }
            for (int pass = 0; pass < 8; pass++)
            {
                var next = (bool[])occupied.Clone(); var nextC = (Color32[])c.Clone(); var nextD = (Color32[])d.Clone();
                for (int y = 0; y < 128; y++) for (int x = 0; x < 128; x++)
                {
                    int i = x+y*128; if (occupied[i]) continue;
                    int neighbour = x > 0 && occupied[i-1] ? i-1 : x < 127 && occupied[i+1] ? i+1 :
                        y > 0 && occupied[i-128] ? i-128 : y < 127 && occupied[i+128] ? i+128 : -1;
                    if (neighbour < 0) continue;
                    next[i] = true; nextC[i] = c[neighbour]; nextD[i] = d[neighbour];
                }
                occupied = next; c = nextC; d = nextD;
            }
            for (int i = 0; i < c.Length; i++) c[i].a = coverage[i];
            return (c,d);
        }

        private static void ImportAtlas(string path, bool srgb)
        {
            AssetDatabase.ImportAsset(path, ImportAssetOptions.ForceSynchronousImport);
            var importer = (TextureImporter)AssetImporter.GetAtPath(path);
            importer.textureType = TextureImporterType.Default; importer.sRGBTexture = srgb;
            importer.alphaIsTransparency = false; importer.mipmapEnabled = true;
            importer.wrapMode = TextureWrapMode.Clamp; importer.filterMode = FilterMode.Trilinear;
            importer.textureCompression = TextureImporterCompression.Uncompressed;
            importer.SaveAndReimport();
        }

        private static VoxelVegetationMesh.Samples Read(Texture2D texture)
        {
            if (texture == null) throw new InvalidOperationException("Missing vegetation source texture");
            const int size = 256;
            var target = RenderTexture.GetTemporary(size, size, 0, RenderTextureFormat.ARGB32);
            var previous = RenderTexture.active;
            Texture2D readable = null;
            try
            {
                Graphics.Blit(texture, target); RenderTexture.active = target;
                readable = new Texture2D(size, size, TextureFormat.RGBA32, false);
                readable.ReadPixels(new Rect(0, 0, size, size), 0, 0); readable.Apply();
                return new VoxelVegetationMesh.Samples(readable.GetPixels32(), size, size);
            }
            finally
            {
                RenderTexture.active = previous; RenderTexture.ReleaseTemporary(target);
                if (readable != null) UnityEngine.Object.DestroyImmediate(readable);
            }
        }
    }
}
