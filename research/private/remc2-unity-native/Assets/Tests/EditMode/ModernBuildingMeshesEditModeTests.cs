using System;
using System.IO;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernBuildingMeshesEditModeTests
    {
        private static ModernBuildingMeshes.Profile Sample()
        {
            return new ModernBuildingMeshes.Profile
            {
                build_index = 999, columns = 5, rows = 5,
                heights = new float[] {
                    0, 0, 0, 0, 0,
                    0, 8, 8, 8, 0,
                    0, 8, 10, 8, 0,
                    0, 8, 8, 8, 0,
                    0, 0, 0, 0, 0 }
            };
        }

        [Test]
        public void EveryLodKeepsEverySampleAndTheSameSourceFacets()
        {
            var profile = Sample();
            var meshes = new Mesh[3];
            try
            {
                int bodyVertices = (profile.columns - 1) * (profile.rows - 1) * 6;
                for (int lod = 0; lod < 3; lod++) meshes[lod] = ModernBuildingMeshes.Create(profile, lod);
                Assert.That(ModernBuildingMeshes.Footprint(profile), Is.EqualTo(new Vector2(16, 16)));
                for (int z = 0; z < profile.rows; z++)
                for (int x = 0; x < profile.columns; x++)
                {
                    var sample = new Vector3(x * 4 - 8, profile.heights[x + z * profile.columns], z * 4 - 8);
                    Assert.That(Array.IndexOf(meshes[0].vertices, sample), Is.InRange(0, bodyVertices - 1));
                }
                var body = meshes[0].vertices;
                for (int lod = 0; lod < 3; lod++)
                {
                    var vertices = meshes[lod].vertices;
                    var normals = meshes[lod].normals;
                    var palette = meshes[lod].uv2;
                    var triangles = meshes[lod].triangles;
                    Assert.That(meshes[lod].subMeshCount, Is.EqualTo(1));
                    Assert.That(normals.Length, Is.EqualTo(vertices.Length));
                    Assert.That(palette.Length, Is.EqualTo(vertices.Length));
                    Assert.That(meshes[lod].uv.Length, Is.EqualTo(vertices.Length));
                    Assert.That(meshes[lod].colors.Length, Is.EqualTo(vertices.Length));
                    for (int i = 0; i < bodyVertices; i++) Assert.That(vertices[i], Is.EqualTo(body[i]));
                    Assert.That(meshes[lod].bounds.min.x, Is.GreaterThanOrEqualTo(-8.001f));
                    Assert.That(meshes[lod].bounds.max.x, Is.LessThanOrEqualTo(8.001f));
                    Assert.That(meshes[lod].bounds.min.z, Is.GreaterThanOrEqualTo(-8.001f));
                    Assert.That(meshes[lod].bounds.max.z, Is.LessThanOrEqualTo(8.001f));
                    Assert.That(meshes[lod].bounds.min.y, Is.GreaterThanOrEqualTo(-.001f));
                    Assert.That(meshes[lod].bounds.max.y, Is.LessThanOrEqualTo(10.001f));
                    for (int i = 0; i < triangles.Length; i += 3)
                    {
                        int a = triangles[i], b = triangles[i + 1], c = triangles[i + 2];
                        var normal = Vector3.Cross(vertices[b] - vertices[a], vertices[c] - vertices[a]);
                        Assert.That(normal.sqrMagnitude, Is.GreaterThan(1e-12f));
                        Assert.That(Vector3.Dot(normal, normals[a]), Is.GreaterThan(0f));
                        Assert.That(normals[a].sqrMagnitude, Is.EqualTo(1f).Within(.001f));
                        Assert.That(palette[a].x, Is.EqualTo(Mathf.Round(palette[a].x)).And.InRange(0f, 6f));
                        if (i < bodyVertices) Assert.That(normals[a].y, Is.GreaterThan(0f));
                    }
                }
                Assert.That(meshes[0].triangles.Length, Is.GreaterThan(meshes[1].triangles.Length));
                Assert.That(meshes[1].triangles.Length, Is.GreaterThan(meshes[2].triangles.Length));
            }
            finally { foreach (var mesh in meshes) if (mesh != null) UnityEngine.Object.DestroyImmediate(mesh); }
        }

        [Test]
        public void SourceParitySelectsTheOtherDiagonalForNonPlanarCells()
        {
            var profile = new ModernBuildingMeshes.Profile { columns = 2, rows = 2, heights = new[] { 0f, 1f, 2f, 8f } };
            var even = ModernBuildingMeshes.Create(profile, 2);
            profile.diagonal_parity = 1;
            var odd = ModernBuildingMeshes.Create(profile, 2);
            try
            {
                CollectionAssert.AreEqual(new[] { new Vector3(-2, 0, -2), new Vector3(-2, 2, 2), new Vector3(2, 8, 2) },
                    new[] { even.vertices[0], even.vertices[1], even.vertices[2] });
                CollectionAssert.AreEqual(new[] { new Vector3(-2, 0, -2), new Vector3(-2, 2, 2), new Vector3(2, 1, -2) },
                    new[] { odd.vertices[0], odd.vertices[1], odd.vertices[2] });
            }
            finally { UnityEngine.Object.DestroyImmediate(even); UnityEngine.Object.DestroyImmediate(odd); }
        }

        [Test]
        public void WindowAnchorsMeetVisibleGlassOnTheirHostFacetsAtEveryLod()
        {
            var profile = Sample();
            var centers = ModernBuildingMeshes.WindowCenters(profile);
            var normals = ModernBuildingMeshes.WindowNormals(profile);
            Assert.That(centers.Length, Is.InRange(1, 6));
            Assert.That(normals.Length, Is.EqualTo(centers.Length));
            for (int lod = 0; lod < 3; lod++)
            {
                var mesh = ModernBuildingMeshes.Create(profile, lod);
                try
                {
                    for (int i = 0; i < centers.Length; i++)
                    {
                        Assert.That(normals[i].sqrMagnitude, Is.EqualTo(1f).Within(.001f));
                        float distance;
                        int material = FirstMaterial(mesh, centers[i] + normals[i], -normals[i], out distance);
                        Assert.That(material, Is.EqualTo(5), "anchor " + i + " LOD " + lod);
                        Assert.That(distance, Is.EqualTo(1f).Within(.001f));
                    }
                }
                finally { UnityEngine.Object.DestroyImmediate(mesh); }
            }
        }

        [Test]
        public void InvalidProfilesAndLodsAreRejected()
        {
            Assert.Throws<ArgumentNullException>(() => ModernBuildingMeshes.Create(null, 0));
            var p = Sample(); p.columns = 1;
            Assert.Throws<ArgumentException>(() => ModernBuildingMeshes.Footprint(p));
            p = Sample(); p.heights = new float[1];
            Assert.Throws<ArgumentException>(() => ModernBuildingMeshes.WindowCenters(p));
            p = Sample(); p.heights[2] = float.NaN;
            Assert.Throws<ArgumentException>(() => ModernBuildingMeshes.Create(p, 0));
            p = Sample(); p.heights[2] = float.PositiveInfinity;
            Assert.Throws<ArgumentException>(() => ModernBuildingMeshes.Create(p, 0));
            p = Sample(); p.diagonal_parity = 2;
            Assert.Throws<ArgumentException>(() => ModernBuildingMeshes.Create(p, 0));
            p = Sample();
            Assert.Throws<ArgumentOutOfRangeException>(() => ModernBuildingMeshes.Create(p, 3));
        }

        [Test]
        public void VoxelLodsUseExposedFacesRetainWindowsAndReduceGeometry()
        {
            var p = Sample(); var meshes = new Mesh[3];
            try
            {
                for (var lod = 0; lod < 3; lod++)
                {
                    var mesh = meshes[lod] = ModernBuildingMeshes.CreateVoxel(p, lod);
                    var vertices = mesh.vertices; var normals = mesh.normals;
                    var triangles = mesh.triangles; var palette = mesh.uv2;
                    Assert.That(mesh.subMeshCount, Is.EqualTo(1));
                    Assert.That(mesh.bounds.size.x, Is.LessThanOrEqualTo(16.001f));
                    Assert.That(mesh.bounds.size.z, Is.LessThanOrEqualTo(16.001f));
                    Assert.That(mesh.bounds.max.y, Is.LessThanOrEqualTo(10f + .5f * (1 << lod)));
                    Assert.That(Array.Exists(palette, material => material.x == 5), Is.True, "glowing glazing remains at every LOD");
                    var centers = ModernBuildingMeshes.VoxelWindowCenters(p, lod); var windowNormals = ModernBuildingMeshes.WindowNormals(p);
                    for (var w = 0; w < centers.Length; w++)
                    {
                        var anchor = centers[w];
                        var material = FirstMaterial(mesh, anchor + windowNormals[w], -windowNormals[w], out var distance);
                        Assert.That(material, Is.EqualTo(5), "glazing must remain visible outside the voxel surface");
                        Assert.That(distance, Is.EqualTo(1).Within(.001f));
                    }
                    for (var i = 0; i < triangles.Length; i += 3)
                    {
                        var a = triangles[i]; var b = triangles[i + 1]; var c = triangles[i + 2];
                        var cross = Vector3.Cross(vertices[b] - vertices[a], vertices[c] - vertices[a]);
                        Assert.That(cross.sqrMagnitude, Is.GreaterThan(1e-12f));
                        Assert.That(Vector3.Dot(cross, normals[a]), Is.GreaterThan(0));
                        Assert.That(normals[a].y, Is.GreaterThanOrEqualTo(-.001f), "no buried bottom faces");
                    }
                }
                Assert.That(meshes[0].triangles.Length, Is.GreaterThan(meshes[1].triangles.Length));
                Assert.That(meshes[1].triangles.Length, Is.GreaterThan(meshes[2].triangles.Length));
                Assert.That(meshes[0].triangles.Length / 3, Is.LessThan(30000));
                Assert.Throws<ArgumentOutOfRangeException>(() => ModernBuildingMeshes.CreateVoxel(p, 3));
            }
            finally { foreach (var mesh in meshes) if (mesh != null) UnityEngine.Object.DestroyImmediate(mesh); }
        }

        [Test]
        public void TerrainOnlyStampDoesNotGainArchitecturalWindows()
        {
            var profile = Sample(); profile.terrain_only = true;
            Assert.That(ModernBuildingMeshes.WindowCenters(profile), Is.Empty);
            var modern = ModernBuildingMeshes.Create(profile, 0);
            var voxel = ModernBuildingMeshes.CreateVoxel(profile, 0);
            try
            {
                Assert.That(Array.Exists(modern.uv2, p => p.x == 5), Is.False);
                Assert.That(Array.Exists(voxel.uv2, p => p.x == 5), Is.False);
            }
            finally { UnityEngine.Object.DestroyImmediate(modern); UnityEngine.Object.DestroyImmediate(voxel); }
        }

        [Serializable] private sealed class ArtLibrary { public ModernBuildingMeshes.Profile[] profiles, castle_profiles; }

        [Test]
        public void OptInBuildingLibraryNightContactSheets()
        {
            var output = Environment.GetEnvironmentVariable("MC_BUILDING_PREVIEW_DIR");
            if (string.IsNullOrEmpty(output)) return;
            var path = Environment.GetEnvironmentVariable("MC_BUILDING_ART_LIBRARY");
            Assert.That(path, Is.Not.Null.And.Not.Empty);
            Assert.That(SystemInfo.graphicsDeviceType, Is.Not.EqualTo(GraphicsDeviceType.Null));
            var library = JsonUtility.FromJson<ArtLibrary>(File.ReadAllText(path));
            Assert.That(library.profiles.Length, Is.EqualTo(17));
            Assert.That(library.castle_profiles.Length, Is.EqualTo(7));
            var previousPipeline = GraphicsSettings.defaultRenderPipeline; var previousQuality = QualitySettings.renderPipeline;
            var previousMode = RenderSettings.ambientMode; var previousSky = RenderSettings.ambientSkyColor;
            var previousEquator = RenderSettings.ambientEquatorColor; var previousGround = RenderSettings.ambientGroundColor;
            var previousSun = RenderSettings.sun; var previousFog = RenderSettings.fog; var previousTarget = RenderTexture.active;
            var node = new GameObject("Building art review"); var cameraObject = new GameObject("Building review camera");
            var moonObject = new GameObject("Building review moon");
            var target = new RenderTexture(320, 240, 24, RenderTextureFormat.ARGB32);
            var tile = new Texture2D(320, 240, TextureFormat.RGB24, false);
            var sheet = new Texture2D(1600, 960, TextureFormat.RGB24, false);
            Material material = null, terrainMaterial = null; Mesh mesh = null;
            try
            {
                var pipeline = Resources.Load<UniversalRenderPipelineAsset>("ModernPipeline");
                var shader = Resources.Load<Shader>("ModernBuilding");
                Assert.That(pipeline, Is.Not.Null); Assert.That(shader, Is.Not.Null);
                GraphicsSettings.defaultRenderPipeline = pipeline; QualitySettings.renderPipeline = null;
                RenderSettings.ambientMode = AmbientMode.Trilight;
                RenderSettings.ambientSkyColor = new Color(.18f, .24f, .34f);
                RenderSettings.ambientEquatorColor = new Color(.12f, .16f, .20f);
                RenderSettings.ambientGroundColor = new Color(.07f, .09f, .085f); RenderSettings.fog = false;
                var moon = moonObject.AddComponent<Light>(); moon.type = LightType.Directional; moon.cullingMask = 1 << 31;
                moon.color = new Color(.76f, .86f, 1f); moon.intensity = .8f;
                moon.transform.rotation = Quaternion.LookRotation(-new Vector3(-.38f, .48f, .79f).normalized); RenderSettings.sun = moon;
                material = new Material(shader); material.SetFloat("_WindowLight", .8f);
                material.SetTexture("_StoneMap", Resources.Load<Texture2D>("ModernTextures/RockAlbedoRoughness"));
                var terrainShader = Resources.Load<Shader>("VoxelSurface"); Assert.That(terrainShader, Is.Not.Null);
                terrainMaterial = new Material(terrainShader); terrainMaterial.SetColor("_BaseColor", new Color(.29f, .40f, .20f));
                node.layer = 31; var filter = node.AddComponent<MeshFilter>(); var renderer = node.AddComponent<MeshRenderer>();
                var camera = cameraObject.AddComponent<Camera>(); camera.orthographic = true; camera.cullingMask = 1 << 31;
                camera.clearFlags = CameraClearFlags.SolidColor; camera.backgroundColor = new Color(.04f, .05f, .06f);
                camera.targetTexture = target;
                Directory.CreateDirectory(output);
                for (int group = 0; group < 3; group++)
                {
                    bool night = group == 1;
                    var profiles = group == 2 ? library.castle_profiles : library.profiles;
                    RenderSettings.ambientSkyColor = night ? new Color(.18f, .24f, .34f) : new Color(.65f, .72f, .8f);
                    RenderSettings.ambientEquatorColor = night ? new Color(.12f, .16f, .20f) : new Color(.42f, .45f, .45f);
                    RenderSettings.ambientGroundColor = night ? new Color(.07f, .09f, .085f) : new Color(.28f, .30f, .25f);
                    moon.intensity = night ? .8f : 1.5f;
                    for (int style = 0; style < 2; style++)
                    {
                        var background = new Color32[1600 * 960];
                        for (var i = 0; i < background.Length; i++) background[i] = new Color32(10, 13, 15, 255);
                        sheet.SetPixels32(background);
                        for (int i = 0; i < profiles.Length; i++)
                        {
                        var profile = profiles[i]; renderer.sharedMaterial = profile.terrain_only ? terrainMaterial : material;
                        mesh = style == 0 ? ModernBuildingMeshes.Create(profile, 0) : ModernBuildingMeshes.CreateVoxel(profile, 0);
                        filter.sharedMesh = mesh;
                        var bounds = mesh.bounds; float extent = Mathf.Max(bounds.size.x, bounds.size.y, bounds.size.z);
                        Assert.That(extent, Is.GreaterThan(0));
                        camera.orthographicSize = extent * .75f;
                        camera.farClipPlane = extent * 8 + 10;
                        camera.transform.position = bounds.center + new Vector3(-1.25f, 1.05f, -1.6f) * extent;
                        camera.transform.LookAt(bounds.center);
                        camera.Render(); camera.Render(); RenderTexture.active = target;
                        tile.ReadPixels(new Rect(0, 0, 320, 240), 0, 0); tile.Apply();
                        sheet.SetPixels32((i % 5) * 320, (3 - i / 5) * 240, 320, 240, tile.GetPixels32());
                        Debug.Log($"MC_BUILDING_ART selector={profile.build_index} terrain_only={profile.terrain_only} style={style} triangles={mesh.triangles.Length / 3}");
                        filter.sharedMesh = null; UnityEngine.Object.DestroyImmediate(mesh); mesh = null;
                        }
                        sheet.Apply();
                        var groupName = group == 0 ? "buildings-day" : group == 1 ? "buildings-night" : "castle-day";
                        using var file = new FileStream(Path.Combine(output, (style == 0 ? "modern-" : "voxel-") + groupName + ".png"), FileMode.CreateNew, FileAccess.Write);
                        var data = sheet.EncodeToPNG(); file.Write(data, 0, data.Length);
                    }
                }
                RenderSettings.ambientSkyColor = new Color(.18f, .24f, .34f);
                RenderSettings.ambientEquatorColor = new Color(.12f, .16f, .20f);
                RenderSettings.ambientGroundColor = new Color(.07f, .09f, .085f); moon.intensity = .8f;
                var house = Array.Find(library.profiles, p => p.build_index == 26);
                var anchors = ModernBuildingMeshes.WindowCenters(house); var normals = ModernBuildingMeshes.WindowNormals(house);
                Assert.That(anchors.Length, Is.GreaterThan(0)); renderer.sharedMaterial = material;
                for (int style = 0; style < 2; style++)
                {
                    mesh = style == 0 ? ModernBuildingMeshes.Create(house, 0) : ModernBuildingMeshes.CreateVoxel(house, 0);
                    filter.sharedMesh = mesh;
                    var anchor = style == 0 ? anchors[0] : ModernBuildingMeshes.VoxelWindowCenters(house, 0)[0];
                    camera.orthographicSize = 2; camera.farClipPlane = 100;
                    camera.transform.position = anchor + normals[0] * 12; camera.transform.LookAt(anchor);
                    camera.Render(); camera.Render(); RenderTexture.active = target;
                    tile.ReadPixels(new Rect(0, 0, 320, 240), 0, 0); tile.Apply();
                    using var file = new FileStream(Path.Combine(output, style == 0 ? "modern-window-night.png" : "voxel-window-night.png"), FileMode.CreateNew, FileAccess.Write);
                    var data = tile.EncodeToPNG(); file.Write(data, 0, data.Length);
                    filter.sharedMesh = null; UnityEngine.Object.DestroyImmediate(mesh); mesh = null;
                }
            }
            finally
            {
                var camera = cameraObject.GetComponent<Camera>(); if (camera != null) camera.targetTexture = null;
                RenderTexture.active = previousTarget; RenderSettings.sun = previousSun; RenderSettings.ambientMode = previousMode;
                RenderSettings.ambientSkyColor = previousSky; RenderSettings.ambientEquatorColor = previousEquator;
                RenderSettings.ambientGroundColor = previousGround; RenderSettings.fog = previousFog;
                GraphicsSettings.defaultRenderPipeline = previousPipeline; QualitySettings.renderPipeline = previousQuality;
                target.Release(); UnityEngine.Object.DestroyImmediate(target); UnityEngine.Object.DestroyImmediate(tile); UnityEngine.Object.DestroyImmediate(sheet);
                if (mesh != null) UnityEngine.Object.DestroyImmediate(mesh); if (material != null) UnityEngine.Object.DestroyImmediate(material);
                if (terrainMaterial != null) UnityEngine.Object.DestroyImmediate(terrainMaterial);
                UnityEngine.Object.DestroyImmediate(node); UnityEngine.Object.DestroyImmediate(cameraObject); UnityEngine.Object.DestroyImmediate(moonObject);
            }
        }

        private static int FirstMaterial(Mesh mesh, Vector3 origin, Vector3 direction, out float nearest)
        {
            var vertices = mesh.vertices; var indices = mesh.triangles; var palette = mesh.uv2;
            nearest = float.PositiveInfinity; int material = -1;
            for (int i = 0; i < indices.Length; i += 3)
            {
                var a = vertices[indices[i]]; var e1 = vertices[indices[i + 1]] - a; var e2 = vertices[indices[i + 2]] - a;
                var p = Vector3.Cross(direction, e2); float det = Vector3.Dot(e1, p);
                if (Mathf.Abs(det) < 1e-7f) continue;
                var t = origin - a; float u = Vector3.Dot(t, p) / det; var q = Vector3.Cross(t, e1);
                float v = Vector3.Dot(direction, q) / det; float distance = Vector3.Dot(e2, q) / det;
                if (u >= 0 && v >= 0 && u + v <= 1 && distance >= 0 && distance < nearest)
                { nearest = distance; material = (int)palette[indices[i]].x; }
            }
            return material;
        }
    }
}
