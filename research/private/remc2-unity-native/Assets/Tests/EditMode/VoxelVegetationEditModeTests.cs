using System;
using System.IO;
using System.Linq;
using System.Reflection;
using MagicCarpet.Native.Voxels;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;
using Object = UnityEngine.Object;

namespace MagicCarpet.Native.Tests
{
    public sealed class VoxelVegetationEditModeTests
    {
        [Test]
        public void SurfaceSamplerHonorsCutoutAndCullsInteriorFaces()
        {
            var source = new Mesh();
            source.vertices = new[] { new Vector3(0,0,0), new Vector3(2,0,0), new Vector3(0,0,1), new Vector3(2,0,1) };
            source.uv = new[] { new Vector2(0,0), new Vector2(.999f,0), new Vector2(0,.999f), new Vector2(.999f,.999f) };
            source.triangles = new[] { 0,2,1, 1,2,3 }; source.RecalculateBounds();
            var white = new VoxelVegetationMesh.Samples(new[] { new Color32(255,255,255,255) }, 1, 1);
            var cutout = new VoxelVegetationMesh.Samples(new[] { new Color32(255,0,0,255), new Color32(0,0,0,255) }, 2, 1);
            var result = VoxelVegetationMesh.Build(source, 8, white, cutout);
            try
            {
                Assert.That(result.AlphaRejected, Is.GreaterThan(0));
                Assert.That(result.InteriorFacesCulled, Is.GreaterThan(0));
                Assert.That(result.Faces, Is.LessThan(result.Cells * 6));
                Assert.That(result.Mesh.bounds.max.x, Is.LessThanOrEqualTo(1.25f), "transparent half does not become a solid card");
                Assert.That(result.Mesh.vertexCount, Is.EqualTo(result.Faces * 4));
            }
            finally { Object.DestroyImmediate(result.Mesh); Object.DestroyImmediate(source); }
        }

        [Test]
        public void AllSevenKindsUseBoundedSharedVoxelLodsAndModernRemainsAvailable()
        {
            var root = new GameObject("Vegetation derivative test");
            var material = new Material(Shader.Find("Hidden/InternalErrorShader"));
            try
            {
                using var terrain = new ModernTerrainMesh(root.transform, material, material);
                using var modern = new ModernVegetation(terrain);
                using var voxel = new ModernVegetation(terrain, true);
                var near = Field<Mesh[]>(voxel, "meshes"); var mid = Field<Mesh[]>(voxel, "intermediateMeshes");
                var far = Field<Mesh[]>(voxel, "distantMeshes"); var modernNear = Field<Mesh[]>(modern, "meshes");
                var materials = Field<Material[]>(voxel, "materials"); var farMaterials = Field<Material[]>(voxel, "distantMaterials");
                for (int kind = 0; kind < VoxelVegetationMesh.KindCount; kind++)
                {
                    Assert.That(modernNear[kind], Is.Not.Null);
                    Assert.That(near[kind], Is.Not.SameAs(modernNear[kind]));
                    foreach (var mesh in new[] { near[kind], mid[kind], far[kind] })
                    {
                        Assert.That(mesh.vertexCount, Is.GreaterThan(0));
                        Assert.That(mesh.colors32.Length, Is.EqualTo(mesh.vertexCount));
                        Assert.That(mesh.normals.Length, Is.EqualTo(mesh.vertexCount));
                        Assert.That(mesh.normals.All(n => float.IsFinite(n.x) && float.IsFinite(n.y) && float.IsFinite(n.z) && Mathf.Abs(n.magnitude-1) < .001f), Is.True);
                        Assert.That(float.IsFinite(mesh.bounds.size.x) && float.IsFinite(mesh.bounds.size.y) && float.IsFinite(mesh.bounds.size.z), Is.True);
                    }
                    var nearCount = near[kind].GetIndexCount(0) / 3;
                    var midCount = mid[kind].GetIndexCount(0) / 3;
                    var farCount = far[kind].GetIndexCount(0) / 3;
                    TestContext.WriteLine($"kind={kind} near={nearCount} mid={midCount} far={farCount} nearBounds={near[kind].bounds}");
                    Assert.That(midCount, Is.LessThan(nearCount), "middle LOD reduces geometry");
                    Assert.That(farCount, Is.LessThan(midCount), "far LOD reduces geometry");
                    Assert.That(farCount, Is.LessThan(nearCount / 2), "far LOD substantially reduces geometry");
                    Assert.That(materials[kind].enableInstancing && farMaterials[kind].enableInstancing, Is.True);
                    Assert.That(materials[kind].shader.name, Is.EqualTo("MagicCarpet/ModernVegetation"));
                    Assert.That(materials[kind].GetFloat("_UseTextures"), Is.Zero);
                    Assert.That(farMaterials[kind].GetFloat("_LodThreshold"), Is.Zero);
                    if (VoxelVegetationMesh.UsesAlpha(kind))
                    {
                        Assert.That(farCount, Is.EqualTo(2), "voxel-derived foliage impostors keep the existing far draw cost");
                        Assert.That(farMaterials[kind].shader.name, Is.EqualTo("MagicCarpet/ModernVegetationImpostor"));
                        var prefix = "ModernNature/Voxels/" + VoxelVegetationMesh.ImpostorName(kind);
                        Assert.That(farMaterials[kind].GetTexture("_BaseMap"), Is.SameAs(Resources.Load<Texture2D>(prefix + "_color")));
                        Assert.That(farMaterials[kind].GetTexture("_ImpostorData"), Is.SameAs(Resources.Load<Texture2D>(prefix + "_normal_depth")));
                        var center = near[kind].bounds.center;
                        var bakedCenter = farMaterials[kind].GetVector("_ImpostorCenter");
                        Assert.That(Vector3.Distance(center, new Vector3(bakedCenter.x,bakedCenter.y,bakedCenter.z)), Is.LessThan(.0001f));
                        float radiusSquared = 0;
                        foreach (var vertex in near[kind].vertices) radiusSquared = Mathf.Max(radiusSquared, (vertex-center).sqrMagnitude);
                        Assert.That(farMaterials[kind].GetFloat("_ImpostorDiameter"), Is.EqualTo(Mathf.Sqrt(radiusSquared)*2.02f).Within(.0001f));
                    }
                }
            }
            finally { Object.DestroyImmediate(material); Object.DestroyImmediate(root); }
        }

        [Test]
        public void GalleryWhenRequested()
        {
            var directory = Environment.GetEnvironmentVariable("MC_VOXEL_NATURE_PREVIEW_DIR");
            if (string.IsNullOrEmpty(directory)) Assert.Ignore("Voxel vegetation gallery is opt-in");
            Assert.That(SystemInfo.graphicsDeviceType, Is.Not.EqualTo(GraphicsDeviceType.Null));
            var root = new GameObject("Vegetation review");
            var terrainMaterial = new Material(Shader.Find("Hidden/InternalErrorShader"));
            var cameraObject = new GameObject("Vegetation review camera"); var camera = cameraObject.AddComponent<Camera>();
            var lightObject = new GameObject("Vegetation review light"); var light = lightObject.AddComponent<Light>();
            var preview = new GameObject("Vegetation preview mesh"); preview.layer = 31;
            var filter = preview.AddComponent<MeshFilter>(); var renderer = preview.AddComponent<MeshRenderer>();
            var target = new RenderTexture(256, 256, 24, RenderTextureFormat.ARGB32);
            var tile = new Texture2D(256, 256, TextureFormat.RGB24, false);
            var sheet = new Texture2D(1024, 512, TextureFormat.RGB24, false);
            var oldPipeline = GraphicsSettings.defaultRenderPipeline; var oldQuality = QualitySettings.renderPipeline;
            var oldTarget = RenderTexture.active; var oldSun = RenderSettings.sun; var oldMode = RenderSettings.ambientMode;
            var oldAmbient = RenderSettings.ambientLight; var oldFog = RenderSettings.fog;
            try
            {
                using var terrain = new ModernTerrainMesh(root.transform, terrainMaterial, terrainMaterial);
                using var modern = new ModernVegetation(terrain);
                using var voxel = new ModernVegetation(terrain, true);
                var pipeline = Resources.Load<UniversalRenderPipelineAsset>("ModernPipeline"); Assert.That(pipeline, Is.Not.Null);
                GraphicsSettings.defaultRenderPipeline = pipeline; QualitySettings.renderPipeline = null;
                light.type = LightType.Directional; light.cullingMask = 1 << 31; light.transform.rotation = Quaternion.Euler(35,155,0);
                RenderSettings.sun = light; RenderSettings.ambientMode = AmbientMode.Flat; RenderSettings.fog = false;
                camera.cullingMask = 1 << 31; camera.clearFlags = CameraClearFlags.SolidColor; camera.backgroundColor = new Color(.08f,.09f,.10f);
                camera.orthographic = true; camera.targetTexture = target;
                Directory.CreateDirectory(directory);
                foreach (var mode in new[] { "modern", "voxel-near", "voxel-far" })
                foreach (var night in new[] { false, true })
                {
                    var source = mode == "modern" ? modern : voxel;
                    var meshes = Field<Mesh[]>(source, mode == "voxel-far" ? "distantMeshes" : "meshes");
                    var materials = Field<Material[]>(source, mode == "voxel-far" ? "distantMaterials" : "materials");
                    var framing = Field<Mesh[]>(source, "meshes");
                    RenderSettings.ambientLight = night ? new Color(.16f,.20f,.25f) : new Color(.32f,.32f,.32f);
                    light.color = night ? new Color(.76f,.86f,1) : Color.white; light.intensity = night ? .8f : 1.1f;
                    for (int kind = 0; kind < VoxelVegetationMesh.KindCount; kind++)
                    {
                        filter.sharedMesh = meshes[kind]; renderer.sharedMaterial = materials[kind];
                        // Frame all LODs with the near shape, not the expanded
                        // billboard culling bounds (which include wind padding).
                        var bounds = framing[kind].bounds; var size = Mathf.Max(bounds.size.x,bounds.size.y,bounds.size.z);
                        camera.orthographicSize = Mathf.Max(.05f,size*.72f);
                        camera.transform.position = bounds.center + new Vector3(1.5f,1f,-2.5f)*size;
                        camera.transform.LookAt(bounds.center);
                        camera.Render(); camera.Render(); RenderTexture.active = target;
                        tile.ReadPixels(new Rect(0,0,256,256),0,0); tile.Apply();
                        var image = tile.GetPixels32();
                        Assert.That(image.Count(p => p.r > 200 && p.b > 200 && p.g < 50), Is.LessThan(10), "no shader errors");
                        sheet.SetPixels32((kind % 4)*256, (1-kind/4)*256, 256,256,image);
                    }
                    sheet.Apply();
                    var path = Path.Combine(directory, mode + (night ? "-night.png" : "-day.png"));
                    using var output = new FileStream(path, FileMode.CreateNew, FileAccess.Write);
                    var encoded = sheet.EncodeToPNG(); output.Write(encoded,0,encoded.Length);
                }
            }
            finally
            {
                camera.targetTexture = null; RenderTexture.active = oldTarget; RenderSettings.sun = oldSun;
                RenderSettings.ambientMode = oldMode; RenderSettings.ambientLight = oldAmbient; RenderSettings.fog = oldFog;
                GraphicsSettings.defaultRenderPipeline = oldPipeline; QualitySettings.renderPipeline = oldQuality;
                target.Release(); Object.DestroyImmediate(target); Object.DestroyImmediate(tile); Object.DestroyImmediate(sheet);
                Object.DestroyImmediate(preview); Object.DestroyImmediate(cameraObject); Object.DestroyImmediate(lightObject);
                Object.DestroyImmediate(terrainMaterial); Object.DestroyImmediate(root);
            }
        }

        private static T Field<T>(object owner, string name) => (T)typeof(ModernVegetation)
            .GetField(name, BindingFlags.NonPublic | BindingFlags.Instance).GetValue(owner);
    }
}
