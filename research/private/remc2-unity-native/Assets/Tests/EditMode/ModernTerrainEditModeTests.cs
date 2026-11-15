using System;
using System.Collections.Generic;
using System.IO;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernTerrainEditModeTests
    {
        private const int Side = 65;
        private const int MapCells = 256;
        private byte[] heights;
        private byte[] types;
        private byte[] water;
        private GameObject parent;
        private Material material;
        private ModernTerrainMesh terrain;

        [SetUp]
        public void SetUp()
        {
            heights = new byte[MapCells * MapCells];
            types = new byte[heights.Length];
            water = new byte[heights.Length];
            for (var z = 0; z < MapCells; z++)
            for (var x = 0; x < MapCells; x++)
            {
                heights[Index(x, z)] = (byte)((x * 17 + z * 31 + x * z) & 63);
                types[Index(x, z)] = (byte)(1+(x + 3 * z) % 6);
            }
            water[Index(31, 31)] = 8;
            water[Index(255, 255)] = 8;
            parent = new GameObject("Terrain test parent");
            material = new Material(Shader.Find("Hidden/InternalErrorShader"));
            terrain = new ModernTerrainMesh(parent.transform, material, material) { ViewDistance = 0f };
            terrain.Update(heights, types, water, 1, 0);
        }

        [TearDown]
        public void TearDown()
        {
            terrain?.Dispose();
            UnityEngine.Object.DestroyImmediate(parent);
            UnityEngine.Object.DestroyImmediate(material);
        }

        [Test]
        public void WarmContactSamplesTrackHeightMaterialWaterAndGenerationChanges()
        {
            var points=new[]{new Vector2(127.25f,126.75f),new Vector2(-.75f,1.25f),new Vector2(1023.25f,1025.25f)};
            foreach(var p in points)terrain.SampleHeight(p.x,p.y);
            for(int turn=1;turn<=4;turn++)
            {
                foreach(int index in new[]{Index(31,31),Index(0,0),Index(255,0)})
                {
                    if(turn==1)heights[index]=(byte)(heights[index]+20);
                    if(turn==2)types[index]=0;
                    if(turn==3){types[index]=1;water[index]^=8;}
                }
                terrain.Update(heights,types,water,turn==4?2UL:1UL,(ulong)turn);
                using var fresh=new ModernTerrainMesh(parent.transform,material,material);
                fresh.Update(heights,types,water,1,0);
                foreach(var p in points)
                {
                    Assert.That(terrain.SampleHeight(p.x,p.y),Is.EqualTo(fresh.SampleHeight(p.x,p.y)).Within(.00001f));
                    Assert.That(terrain.SampleHeight(p.x+1024,p.y-1024),Is.EqualTo(fresh.SampleHeight(p.x,p.y)).Within(.0001f));
                }
            }
        }

        [Test]
        public void ShoreCoverageTracksOrientationOnlyMutationAndLowersItsBed()
        {
            var atlas = new byte[256*608];
            Array.Fill(atlas,(byte)12);
            for(var v=0;v<32;v++) for(var u=0;u<16;u++)
                atlas[(44/8*32+v)*256+44%8*32+u]=0;
            using var mask=new ModernShoreMask(atlas);
            terrain.Dispose();
            terrain=new ModernTerrainMesh(parent.transform,material,material,mask);
            Array.Clear(heights,0,heights.Length);
            Array.Clear(water,0,water.Length);
            Array.Fill(types,(byte)1);
            types[0]=44;
            terrain.Update(heights,types,water,1,0);
            Assert.That(terrain.IsWater(.5f,1.5f),Is.True);
            Assert.That(terrain.IsWater(3.5f,1.5f),Is.True,"sea-level mixed tiles form a submerged bank");
            Assert.That(mask.SampleWorld(.5f,1.5f),Is.EqualTo(1));
            Assert.That(mask.SampleWorld(3.5f,1.5f),Is.Zero);
            Assert.That(terrain.SampleHeight(.5f,1.5f),Is.LessThan(-.3f));
            Assert.That(Section(terrain,parent,0,0).GetTriangles(1).Length,Is.EqualTo(24));
            var revision=terrain.Revision;
            water[0]=0x10;
            terrain.Update(heights,types,water,1,1);
            Assert.That(terrain.Revision,Is.GreaterThan(revision));
            Assert.That(mask.Revision,Is.EqualTo(terrain.Revision));
            Assert.That(mask.SampleWorld(.5f,1.5f),Is.Zero);
            Assert.That(mask.SampleWorld(3.5f,1.5f),Is.EqualTo(1));
            Assert.That(terrain.IsWater(3.5f,1.5f),Is.True);
            heights[0]=2;
            terrain.Update(heights,types,water,1,2);
            Assert.That(terrain.IsWater(.5f,1.5f),Is.False,"elevated transition material stays land");
            Assert.That(Section(terrain,parent,0,0).GetTriangles(1),Is.Empty);
            Assert.That(mask.SampleWorld(3.5f,1.5f),Is.Zero);
        }

        [Test]
        public void SubdivisionUsesOriginalDiagonalsAndSamplingMatchesMesh()
        {
            var mesh = Section(terrain, parent, 0, 0);
            var vertices = mesh.vertices;
            Assert.That(vertices.Length, Is.EqualTo(Side * Side));
            var indices = mesh.GetTriangles(0);
            // The first two source cells have opposite diagonals; each has four quads.
            CollectionAssert.AreEqual(new[] { 0, 65, 66, 0, 66, 1 }, Slice(indices, 0, 6));
            CollectionAssert.AreEqual(new[] { 2, 67, 3, 67, 68, 3 }, Slice(indices, 12, 6));
            var hasReconstruction = false;
            for (var z = 0; z < Side; z++)
            for (var x = 0; x < Side; x++)
            {
                var sourceX = x / 2;
                var sourceZ = z / 2;
                var baseline = SourceHeight(sourceX, sourceZ, (x & 1) * 0.5f, (z & 1) * 0.5f);
                var difference = Mathf.Abs(vertices[x + z * Side].y - baseline);
                // Dry source vertices now undergo visual reconstruction. The
                // normalized filter stays within the source range plus relief.
                Assert.That(vertices[x+z*Side].y, Is.InRange(-.60f,31.65f));
                if (difference > .15f) hasReconstruction = true;
            }
            Assert.That(hasReconstruction, Is.True, "dry ridges should be reconstructed");

            // Interpolate actual emitted triangles, including both diagonal parities.
            for (var i = 0; i < indices.Length; i += 6 * 137)
            {
                var a = vertices[indices[i]];
                var b = vertices[indices[i + 1]];
                var c = vertices[indices[i + 2]];
                var point = a * 0.2f + b * 0.3f + c * 0.5f;
                Assert.That(terrain.SampleHeight(point.x, point.z), Is.EqualTo(point.y).Within(0.0005f));
            }
        }

        [Test]
        public void WaterAndShoreShareBedSurfaceAndWrappedNormalsMatch()
        {
            var first = Section(terrain, parent, 0, 0);
            var lastX = Section(terrain, parent, 7, 0);
            var lastZ = Section(terrain, parent, 0, 7);
            var lastBoth = Section(terrain, parent, 7, 7);
            AssertWaterCell(first, 31, 31);
            AssertWaterCell(lastBoth, 255, 255);
            Assert.That(terrain.IsWater(31 * 4 + 1, 31 * 4 + 1), Is.True);
            Assert.That(terrain.IsWater(30 * 4 + 1, 31 * 4 + 1), Is.False);
            var firstVertices = first.vertices;
            var firstNormals = first.normals;
            var xVertices = lastX.vertices;
            var xNormals = lastX.normals;
            var zVertices = lastZ.vertices;
            var zNormals = lastZ.normals;
            for (var i = 0; i < Side; i++)
            {
                Assert.That(xVertices[64 + i * Side].y, Is.EqualTo(firstVertices[i * Side].y).Within(0.00001f));
                Assert.That(Vector3.Distance(xNormals[64 + i * Side], firstNormals[i * Side]), Is.LessThan(0.00001f));
                Assert.That(zVertices[i + 64 * Side].y, Is.EqualTo(firstVertices[i].y).Within(0.00001f));
                Assert.That(Vector3.Distance(zNormals[i + 64 * Side], firstNormals[i]), Is.LessThan(0.00001f));
            }
            Assert.That(terrain.SampleHeight(-1.25f, 129.5f),
                Is.EqualTo(terrain.SampleHeight(1022.75f, -894.5f)).Within(0.0001f));
        }

        [Test]
        public void VoxelSectionsCoverMapAndRetainSourceSamplingWaterAndLodEdges()
        {
            var other = new GameObject("Voxel terrain test");
            try
            {
                using var voxel = new ModernTerrainMesh(other.transform, material, material, voxel: true) { ViewDistance = 0f };
                voxel.Update(heights, types, water, 1, 0);
                foreach (var p in new[] { new Vector2(.2f, .3f), new Vector2(127.2f, 128.8f), new Vector2(-.5f, 1024.5f) })
                {
                    Assert.That(voxel.SampleHeight(p.x, p.y), Is.EqualTo(terrain.SampleHeight(p.x, p.y)).Within(.00001f));
                    Assert.That(voxel.GroundType(p.x, p.y), Is.EqualTo(terrain.GroundType(p.x, p.y)));
                    Assert.That(voxel.IsWater(p.x, p.y), Is.EqualTo(terrain.IsWater(p.x, p.y)));
                }
                // Visit all 64 canonical sections in far LOD, including the
                // periodic edges. No section is replaced by a smooth fallback.
                for (int z = 0; z < 8; z++) for (int x = 0; x < 8; x++)
                {
                    voxel.Present(new Vector3(x * 128 + 64, 500, z * 128 + 64));
                    var filter = other.transform.Find($"Modern Terrain/Terrain {x},{z} copy 4").GetComponent<MeshFilter>();
                    Assert.That(filter.sharedMesh.name, Does.StartWith("Voxel terrain section"));
                    Assert.That(filter.sharedMesh.name, Does.EndWith("far"));
                    Assert.That(filter.sharedMesh.GetIndexCount(0) + filter.sharedMesh.GetIndexCount(1), Is.GreaterThan(0));
                    Assert.That(filter.sharedMesh.vertexCount,
                        Is.LessThanOrEqualTo(VoxelVertexBound(Voxels.VoxelTerrainSurface.FarCell)));
                }
                voxel.Present(new Vector3(64, 0, 64));
                var near = other.transform.Find("Modern Terrain/Terrain 0,0 copy 4").GetComponent<MeshFilter>().sharedMesh;
                Assert.That(near.name, Does.EndWith("near"));
                Assert.That(near.vertexCount,
                    Is.LessThanOrEqualTo(VoxelVertexBound(Voxels.VoxelTerrainSurface.NearCell)));
                Assert.That(near.GetIndexCount(0) / 3, Is.GreaterThan(8000));
                var hasVertical = false;
                foreach (var n in near.normals) if (Mathf.Abs(n.y) < .001f) { hasVertical = true; break; }
                Assert.That(hasVertical, Is.True, "voxel terrain contains exposed vertical block faces");
                var baseline = Section(terrain, parent, 0, 0);
                CollectionAssert.AreEqual(baseline.GetTriangles(1), near.GetTriangles(1), "water retains the original geometry");
                voxel.Present(new Vector3(64, 500, 64));
                var far = other.transform.Find("Modern Terrain/Terrain 0,0 copy 4").GetComponent<MeshFilter>().sharedMesh;
                Assert.That(far.GetIndexCount(0), Is.LessThan(near.GetIndexCount(0) / 2),
                    "2m distant columns must still substantially reduce geometry");
                CollectionAssert.AreEqual(TopEdge(near, 128), TopEdge(far, 128), "near/far meet at section edges");
                CollectionAssert.AreEqual(TopEdge(near, 0), TopEdge(far, 0), "near/far meet at the wrapped map edge");
                TestContext.WriteLine($"voxel section near={near.GetIndexCount(0)/3} land triangles far={far.GetIndexCount(0)/3} nearVertices={near.vertexCount} farVertices={far.vertexCount}");
            }
            finally { UnityEngine.Object.DestroyImmediate(other); }
        }

        // n² top quads and at most one wall per interior/boundary interface:
        // 2n(n-1)+4n = 2n²+2n walls. Each quad contributes at most four new
        // vertices, plus the retained 65²-vertex smooth water/collar prefix.
        private static int VoxelVertexBound(float cell)
        {
            int n = Mathf.RoundToInt(128f / cell);
            return Side * Side + 12 * n * n + 8 * n;
        }

        [Test]
        public void VoxelSurfaceKeepsTheExistingNativePatchOpen()
        {
            var other = new GameObject("Voxel patch terrain test");
            try
            {
                using var voxel = new ModernTerrainMesh(other.transform, material, material, voxel: true)
                    { ViewDistance = 0f, VoxelPatch = new Bounds(new Vector3(64, 0, 64), new Vector3(32, 200, 40)) };
                voxel.Update(heights, types, water, 1, 0);
                voxel.Present(new Vector3(64, 500, 64));
                var mesh = other.transform.Find("Modern Terrain/Terrain 0,0 copy 4").GetComponent<MeshFilter>().sharedMesh;
                Assert.That(mesh.name, Does.EndWith("near"), "patch collar uses the fine surface at every distance");
                var v = mesh.vertices; var triangles = mesh.GetTriangles(0);
                for (int i = 0; i < triangles.Length; i += 3)
                {
                    var p = (v[triangles[i]] + v[triangles[i+1]] + v[triangles[i+2]]) / 3f;
                    Assert.That(p.x > 48 && p.x < 80 && p.z > 44 && p.z < 84, Is.False, "no terrain caps the native patch");
                }
            }
            finally { UnityEngine.Object.DestroyImmediate(other); }
        }

        [Test]
        public void VoxelMixedLodWallsSealSectionAndPeriodicBoundaries()
        {
            var smooth = new Mesh { subMeshCount = 2 };
            var near = new Mesh(); var far = new Mesh(); var wrapNear = new Mesh(); var wrapFar = new Mesh();
            var zNear = new Mesh(); var zFar = new Mesh(); var zWrapNear = new Mesh(); var zWrapFar = new Mesh();
            try
            {
                float Height(float x, float z) => Mathf.Repeat(x, 1024f) * .4f + Mathf.Repeat(z, 1024f) * .02f;
                bool Water(float x, float z) => false;
                Color Tint(float x, float z) => Color.white;
                Voxels.VoxelTerrainSurface.Build(near, smooth, Vector2.zero, 1, Height, Water, Tint, null);
                Voxels.VoxelTerrainSurface.Build(far, smooth, new Vector2(128, 0), Voxels.VoxelTerrainSurface.FarCell, Height, Water, Tint, null);
                AssertSealed(near, 128, far, 0);
                Voxels.VoxelTerrainSurface.Build(wrapNear, smooth, new Vector2(896, 0), 1, Height, Water, Tint, null);
                Voxels.VoxelTerrainSurface.Build(wrapFar, smooth, Vector2.zero, Voxels.VoxelTerrainSurface.FarCell, Height, Water, Tint, null);
                AssertSealed(wrapNear, 128, wrapFar, 0);
                Voxels.VoxelTerrainSurface.Build(zNear, smooth, Vector2.zero, 1, Height, Water, Tint, null);
                Voxels.VoxelTerrainSurface.Build(zFar, smooth, new Vector2(0,128), Voxels.VoxelTerrainSurface.FarCell, Height, Water, Tint, null);
                AssertSealed(zNear, 128, zFar, 0, true);
                Voxels.VoxelTerrainSurface.Build(zWrapNear, smooth, new Vector2(0,896), 1, Height, Water, Tint, null);
                Voxels.VoxelTerrainSurface.Build(zWrapFar, smooth, Vector2.zero, Voxels.VoxelTerrainSurface.FarCell, Height, Water, Tint, null);
                AssertSealed(zWrapNear, 128, zWrapFar, 0, true);
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(smooth); UnityEngine.Object.DestroyImmediate(near);
                UnityEngine.Object.DestroyImmediate(far); UnityEngine.Object.DestroyImmediate(wrapNear);
                UnityEngine.Object.DestroyImmediate(wrapFar);
                UnityEngine.Object.DestroyImmediate(zNear); UnityEngine.Object.DestroyImmediate(zFar);
                UnityEngine.Object.DestroyImmediate(zWrapNear); UnityEngine.Object.DestroyImmediate(zWrapFar);
            }
        }

        [Test]
        public void VoxelVisualHeightSamplesTheActuallyPresentedLodAndWrap()
        {
            var other = new GameObject("Voxel visual height test");
            try
            {
                using var voxel = new ModernTerrainMesh(other.transform, material, material, voxel: true) { ViewDistance = 0f };
                voxel.Update(heights, types, water, 1, 0);
                float x = 60.25f, z = 60.35f;
                float source = voxel.SampleHeight(x, z);
                Assert.That(voxel.SampleVisualHeight(x, z), Is.EqualTo(source), "before presentation preserve the source sampler");
                void Check(float cameraX, float cameraY, float pointX, string lod)
                {
                    voxel.Present(new Vector3(cameraX, cameraY, 64));
                    var mesh = other.transform.Find("Modern Terrain/Terrain 0,0 copy 4").GetComponent<MeshFilter>().sharedMesh;
                    Assert.That(mesh.name, Does.EndWith(lod));
                    var actual = TopHeightAt(mesh, 60.25f, z);
                    Assert.That(voxel.SampleVisualHeight(pointX, z), Is.EqualTo(actual).Within(.0001f));
                }
                Check(64, 0, x, "near");
                Check(64, 500, x, "far");
                Check(1088, 0, x + 1024, "near");
                Assert.That(voxel.SampleHeight(x, z), Is.EqualTo(source).Within(.00001f), "source sampling remains unchanged");
                heights[Index(15, 15)] += 20;
                voxel.Update(heights, types, water, 1, 1);
                var updatedHeight = voxel.SampleVisualHeight(x + 1024, z);
                var updatedMesh = other.transform.Find("Modern Terrain/Terrain 0,0 copy 4").GetComponent<MeshFilter>().sharedMesh;
                Assert.That(updatedHeight, Is.EqualTo(TopHeightAt(updatedMesh, x, z)).Within(.0001f),
                    "sampling after an update refreshes the selected mesh before Present");
                Check(64, 0, x, "near");
            }
            finally { UnityEngine.Object.DestroyImmediate(other); }
        }

        private static float TopHeightAt(Mesh mesh, float x, float z)
        {
            var v = mesh.vertices; var n = mesh.normals; var triangles = mesh.GetTriangles(0);
            for (int i = 0; i < triangles.Length; i += 3)
            {
                int a = triangles[i], b = triangles[i+1], c = triangles[i+2];
                if (n[a].y < .999f || n[b].y < .999f || n[c].y < .999f) continue;
                if (x >= Mathf.Min(v[a].x,v[b].x,v[c].x) && x <= Mathf.Max(v[a].x,v[b].x,v[c].x) &&
                    z >= Mathf.Min(v[a].z,v[b].z,v[c].z) && z <= Mathf.Max(v[a].z,v[b].z,v[c].z)) return v[a].y;
            }
            Assert.Fail($"No block top at {x},{z}"); return 0;
        }

        private static void AssertSealed(Mesh left, float leftEdge, Mesh right, float rightEdge, bool zAxis = false)
        {
            var a = TopEdge(left, leftEdge, zAxis); var b = TopEdge(right, rightEdge, zAxis);
            for (int z = 0; z < 128; z++)
            {
                Assert.That(float.IsNaN(a[z]) || float.IsNaN(b[z]), Is.False, $"missing top at edge {z}");
                if (Mathf.Abs(a[z] - b[z]) < .00001f) continue;
                var high = a[z] > b[z] ? left : right; var edge = a[z] > b[z] ? leftEdge : rightEdge;
                Assert.That(HasWall(high, edge, z + .5f, Mathf.Min(a[z], b[z]), Mathf.Max(a[z], b[z]), zAxis), Is.True,
                    $"unsealed edge at z={z} between heights {a[z]} and {b[z]}");
            }
        }

        private static bool HasWall(Mesh mesh, float edgeX, float z, float low, float high, bool zAxis)
        {
            var v = mesh.vertices; var n = mesh.normals; var triangles = mesh.GetTriangles(0);
            for (int i = 0; i < triangles.Length; i += 3)
            {
                int a = triangles[i], b = triangles[i+1], c = triangles[i+2];
                float Axis(Vector3 p) => zAxis ? p.z : p.x;
                float Along(Vector3 p) => zAxis ? p.x : p.z;
                if (Mathf.Abs(Axis(n[a])) < .99f || Mathf.Abs(Axis(v[a])-edgeX) > .001f ||
                    Mathf.Abs(Axis(v[b])-edgeX) > .001f || Mathf.Abs(Axis(v[c])-edgeX) > .001f) continue;
                var minZ = Mathf.Min(Along(v[a]), Along(v[b]), Along(v[c])); var maxZ = Mathf.Max(Along(v[a]), Along(v[b]), Along(v[c]));
                var minY = Mathf.Min(v[a].y, v[b].y, v[c].y); var maxY = Mathf.Max(v[a].y, v[b].y, v[c].y);
                if (z >= minZ && z <= maxZ && minY <= low + .001f && maxY >= high - .001f) return true;
            }
            return false;
        }

        [Test]
        public void VoxelLevel1GalleryWhenRequested()
        {
            var directory = Environment.GetEnvironmentVariable("MC_TERRAIN_PREVIEW_DIR");
            var terrainPath = Environment.GetEnvironmentVariable("MC_TERRAIN_LEVEL1_BIN");
            var shorePath = Environment.GetEnvironmentVariable("MC_TERRAIN_LEVEL1_SHORE");
            if (string.IsNullOrEmpty(directory) || string.IsNullOrEmpty(terrainPath) || string.IsNullOrEmpty(shorePath))
                Assert.Ignore("Owner-local Level 1 gallery is opt-in");
            Assert.That(SystemInfo.graphicsDeviceType, Is.Not.EqualTo(GraphicsDeviceType.Null));
            var bytes = File.ReadAllBytes(terrainPath);
            Assert.That(bytes.Length, Is.EqualTo(7 * MapCells * MapCells));
            var sourceTypes = new byte[65536]; var sourceHeights = new byte[65536]; var sourceAngles = new byte[65536];
            Array.Copy(bytes, 0, sourceTypes, 0, 65536);
            Array.Copy(bytes, 65536, sourceHeights, 0, 65536);
            Array.Copy(bytes, 3 * 65536, sourceAngles, 0, 65536);
            var reviewRoot = new GameObject("Level 1 terrain gallery");
            var camera = new GameObject("Terrain review camera").AddComponent<Camera>();
            var keyObject = new GameObject("Terrain review key");
            var target = new RenderTexture(960, 640, 24, RenderTextureFormat.ARGB32);
            var pixels = new Texture2D(960, 640, TextureFormat.RGB24, false);
            var previousPipeline = GraphicsSettings.defaultRenderPipeline; var previousQuality = QualitySettings.renderPipeline;
            var previousTarget = RenderTexture.active; var previousSkybox = RenderSettings.skybox;
            var previousSun = RenderSettings.sun; var previousMode = RenderSettings.ambientMode;
            var previousSky = RenderSettings.ambientSkyColor; var previousEquator = RenderSettings.ambientEquatorColor;
            var previousGround = RenderSettings.ambientGroundColor; var previousFog = RenderSettings.fog;
            var previousFogColor = RenderSettings.fogColor; var previousFogDensity = RenderSettings.fogDensity;
            var previousFogMode = RenderSettings.fogMode;
            var land = new Material(Resources.Load<Shader>("ModernTerrain"));
            var waterMaterial = new Material(Resources.Load<Shader>("ModernWater"));
            var sky = new Material(Resources.Load<Shader>("ModernSky"));
            try
            {
                var pipeline = Resources.Load<UniversalRenderPipelineAsset>("ModernPipeline"); Assert.That(pipeline, Is.Not.Null);
                GraphicsSettings.defaultRenderPipeline = pipeline; QualitySettings.renderPipeline = null;
                foreach (var layer in new[] { "Rock", "Ground" })
                    foreach (var channel in new[] { "AlbedoRoughness", "Normal" })
                        land.SetTexture("_" + layer + channel, Resources.Load<Texture2D>("ModernTextures/" + layer + channel));
                land.SetFloat("_UseTextures", 1); land.SetFloat("_UsePackedRoughness", 1);
                land.SetFloat("_UseSparseProjection", 1); land.SetFloat("_TextureScale", .20f);
                land.SetFloat("_UseMaterialEndpoints", 1);
                using var shore = new ModernShoreMask(File.ReadAllBytes(shorePath));
                Shader.SetGlobalTexture("_ModernShoreMask", shore.Texture); Shader.SetGlobalFloat("_ModernShoreMaskReady", 1);
                using var world = new ModernTerrainMesh(reviewRoot.transform, land, waterMaterial, shore, voxel: true) { ViewDistance = 320 };
                world.Update(sourceHeights, sourceTypes, sourceAngles, 1, 0);
                using var night = new ModernNightLighting(reviewRoot.transform);
                using var waterSystem = new ModernWaterSystem(reviewRoot.transform, world, night, land);
                var key = keyObject.AddComponent<Light>(); key.type = LightType.Directional; RenderSettings.sun = key;
                RenderSettings.skybox = sky; RenderSettings.ambientMode = AmbientMode.Trilight;
                RenderSettings.fog = true; RenderSettings.fogMode = FogMode.ExponentialSquared;
                camera.clearFlags = CameraClearFlags.Skybox; camera.fieldOfView = 67; camera.farClipPlane = 900;
                camera.allowHDR = true; camera.targetTexture = target;
                Directory.CreateDirectory(directory);
                foreach (var place in new[] { new Vector2(220, 888), new Vector2(140, 68) })
                {
                    string label = place.x == 220 ? "hill" : "shore";
                    var center = new Vector3(place.x, world.SampleHeight(place.x, place.y), place.y);
                    camera.transform.position = center + new Vector3(-42, 30, -56);
                    camera.transform.position = new Vector3(camera.transform.position.x,
                        Mathf.Max(camera.transform.position.y, world.SampleHeight(camera.transform.position.x, camera.transform.position.z) + 10),
                        camera.transform.position.z);
                    camera.transform.LookAt(center + Vector3.up * 3);
                    world.Present(camera.transform.position); waterSystem.Update(camera);
                    foreach (var mode in new[] { 0, 2 })
                    {
                        ModernEnvironmentSandbox.ApplyLighting(mode, key, sky, .58f);
                        camera.Render(); camera.Render(); RenderTexture.active = target;
                        pixels.ReadPixels(new Rect(0, 0, 960, 640), 0, 0); pixels.Apply();
                        var path = Path.Combine(directory, label + (mode == 0 ? "-day.png" : "-night.png"));
                        using var output = new FileStream(path, FileMode.CreateNew, FileAccess.Write);
                        var encoded = pixels.EncodeToPNG(); output.Write(encoded, 0, encoded.Length);
                        TestContext.WriteLine($"{label} mode={mode} camera={camera.transform.position} detailCopies={world.VisibleDetailCopies} farCopies={world.VisibleFarCopies} triangles={world.VisibleTriangleCount}");
                    }
                }
            }
            finally
            {
                camera.targetTexture = null; RenderTexture.active = previousTarget;
                RenderSettings.skybox = previousSkybox; RenderSettings.sun = previousSun; RenderSettings.ambientMode = previousMode;
                RenderSettings.ambientSkyColor = previousSky; RenderSettings.ambientEquatorColor = previousEquator;
                RenderSettings.ambientGroundColor = previousGround; RenderSettings.fog = previousFog;
                RenderSettings.fogColor = previousFogColor; RenderSettings.fogDensity = previousFogDensity; RenderSettings.fogMode = previousFogMode;
                GraphicsSettings.defaultRenderPipeline = previousPipeline; QualitySettings.renderPipeline = previousQuality;
                target.Release(); UnityEngine.Object.DestroyImmediate(target); UnityEngine.Object.DestroyImmediate(pixels);
                UnityEngine.Object.DestroyImmediate(camera.gameObject); UnityEngine.Object.DestroyImmediate(keyObject);
                UnityEngine.Object.DestroyImmediate(reviewRoot); UnityEngine.Object.DestroyImmediate(land);
                UnityEngine.Object.DestroyImmediate(waterMaterial); UnityEngine.Object.DestroyImmediate(sky);
            }
        }

        private static float[] TopEdge(Mesh mesh, float edgeX, bool zAxis = false)
        {
            var result = new float[128]; Array.Fill(result, float.NaN);
            var vertices = mesh.vertices; var normals = mesh.normals; var triangles = mesh.GetTriangles(0);
            for (int i = 0; i < triangles.Length; i += 3)
            {
                int a = triangles[i], b = triangles[i+1], c = triangles[i+2];
                if (normals[a].y < .999f || normals[b].y < .999f || normals[c].y < .999f) continue;
                int[] points = { a, b, c };
                for (int j = 0; j < 3; j++)
                {
                    var p = vertices[points[j]]; var q = vertices[points[(j+1)%3]];
                    if (Mathf.Abs((zAxis ? p.z : p.x)-edgeX) > .001f || Mathf.Abs((zAxis ? q.z : q.x)-edgeX) > .001f) continue;
                    int low = Mathf.RoundToInt(Mathf.Min(zAxis ? p.x : p.z, zAxis ? q.x : q.z));
                    int high = Mathf.RoundToInt(Mathf.Max(zAxis ? p.x : p.z, zAxis ? q.x : q.z));
                    for (int k = Mathf.Max(0, low); k < Mathf.Min(128, high); k++) result[k] = p.y;
                }
            }
            return result;
        }

        [Test]
        public void MutationsRefreshAffectedSectionsAcrossBothSeams()
        {
            // Instantiate the affected sections before the update, just as presentation does.
            var coordinates = new[,] { { 0, 0 }, { 7, 0 }, { 0, 7 }, { 7, 7 }, { 1, 1 }, { 0, 2 }, { 1, 2 } };
            var before = new Mesh[coordinates.GetLength(0)];
            for (var i = 0; i < before.Length; i++)
                before[i] = Section(terrain, parent, coordinates[i, 0], coordinates[i, 1]);
            var oldHeight = before[0].vertices[0].y;
            heights[Index(0, 0)] += 10;
            heights[Index(32, 32)] += 5;
            heights[Index(29, 64)] += 17; // filter/normal halo crosses the next section
            water[Index(255, 255)] = 0;
            water[Index(32, 31)] = 8;
            types[Index(32, 32)] += 1;
            terrain.Update(heights, types, water, 1, 1);
            Assert.That(before[0].vertices[0].y, Is.GreaterThan(oldHeight));
            Assert.That(terrain.IsWater(1023f, 1023f), Is.False);
            Assert.That(terrain.IsWater(129f, 125f), Is.True);

            var otherParent = new GameObject("Fresh terrain test parent");
            var fresh = new ModernTerrainMesh(otherParent.transform, material, material) { ViewDistance = 0f };
            try
            {
                fresh.Update(heights, types, water, 2, 1);
                for (var i = 0; i < before.Length; i++)
                {
                    var rebuilt = before[i];
                    var expected = Section(fresh, otherParent, coordinates[i, 0], coordinates[i, 1]);
                    var actualVertices = rebuilt.vertices;
                    var expectedVertices = expected.vertices;
                    var actualNormals = rebuilt.normals;
                    var expectedNormals = expected.normals;
                    var actualColors = rebuilt.colors;
                    var expectedColors = expected.colors;
                    for (var v = 0; v < actualVertices.Length; v++)
                    {
                        Assert.That(Vector3.Distance(actualVertices[v], expectedVertices[v]), Is.LessThan(0.00001f));
                        Assert.That(Vector3.Distance(actualNormals[v], expectedNormals[v]), Is.LessThan(0.00001f));
                        Assert.That(actualColors[v], Is.EqualTo(expectedColors[v]));
                    }
                    CollectionAssert.AreEqual(expected.GetTriangles(0), rebuilt.GetTriangles(0));
                    CollectionAssert.AreEqual(expected.GetTriangles(1), rebuilt.GetTriangles(1));
                }
            }
            finally
            {
                fresh.Dispose();
                UnityEngine.Object.DestroyImmediate(otherParent);
            }
        }

        [Test]
        public void DistantLodStitchesLandWaterAndChunkEdgesAndTracksDeformation()
        {
            // A large lake supplies both shoreline transitions and homogeneous
            // interior water; the wrapped water cell from SetUp is retained.
            for (var z = 4; z < 28; z++)
            for (var x = 4; x < 28; x++) water[Index(x, z)] = 8;
            terrain.Update(heights, types, water, 1, 1);
            terrain.ViewDistance = 650f;
            terrain.Present(new Vector3(64f, 0f, 64f));
            var copy = parent.transform.Find("Modern Terrain/Terrain 0,0 copy 4");
            Assert.That(copy, Is.Not.Null);
            var filter = copy.GetComponent<MeshFilter>();
            var near = filter.sharedMesh;
            terrain.Present(new Vector3(500f, 0f, 64f));
            var far = filter.sharedMesh;
            Assert.That(far, Is.Not.SameAs(near));
            Assert.That(terrain.VisibleDetailCopies, Is.GreaterThan(0));
            Assert.That(terrain.VisibleFarCopies, Is.GreaterThan(0));
            Assert.That(terrain.VisibleTriangleCount, Is.GreaterThan(0));
            Assert.That(far.GetTriangles(0).Length, Is.LessThan(near.GetTriangles(0).Length));
            Assert.That(far.GetTriangles(1).Length, Is.LessThan(near.GetTriangles(1).Length / 2));
            CollectionAssert.AreEqual(near.vertices, far.vertices);
            AssertWaterCell(near, 12, 12);
            AssertMaterialCoverage(near);
            AssertMaterialCoverage(far);
            CollectionAssert.AreEquivalent(ShoreEdges(near), ShoreEdges(far));
            Assert.That(ShoreEdges(far).Count, Is.GreaterThan(0));
            AssertClosedInteriorEdges(far);
            // A copy in the hysteresis band keeps its previous LOD.
            terrain.Present(new Vector3(338f, 0f, 64f));
            Assert.That(filter.sharedMesh, Is.SameAs(far));
            terrain.Present(new Vector3(64f, 0f, 64f));
            Assert.That(filter.sharedMesh, Is.SameAs(near));
            terrain.Present(new Vector3(338f, 0f, 64f));
            Assert.That(filter.sharedMesh, Is.SameAs(near));
            terrain.Present(new Vector3(500f, 0f, 64f));
            Assert.That(filter.sharedMesh, Is.SameAs(far));
            terrain.Present(new Vector3(64f, 500f, 64f));
            Assert.That(copy.gameObject.activeSelf, Is.True, "altitude must not alter horizontal residency");
            Assert.That(filter.sharedMesh, Is.SameAs(far));
            terrain.Present(new Vector3(64f, 0f, 64f));
            Assert.That(filter.sharedMesh, Is.SameAs(near));
            terrain.Present(new Vector3(500f, 0f, 64f));

            // Rebuild both retained variants after a source-height and water change.
            heights[Index(10, 10)] += 10;
            water[Index(12, 12)] = 0;
            terrain.Update(heights, types, water, 1, 2);
            Assert.That(far.vertices[20 + 20 * Side].y,
                Is.EqualTo(terrain.SampleHeight(40,40)).Within(.00001f));
            CollectionAssert.AreEqual(near.vertices, far.vertices);
            AssertMaterialCoverage(near);
            AssertMaterialCoverage(far);
            CollectionAssert.AreEquivalent(ShoreEdges(near), ShoreEdges(far));
            AssertClosedInteriorEdges(far);
            terrain.Present(new Vector3(64f, 0f, 64f));
            Assert.That(filter.sharedMesh, Is.SameAs(near));
        }

        [Test]
        public void LandformReconstructionRoundsRidgesWithoutMovingWetAnchorsOrFlatGround()
        {
            Array.Fill(heights,(byte)16); Array.Clear(water,0,water.Length);
            heights[Index(16,16)]=144;
            terrain.Update(heights,types,water,3,0);
            // A 64 m impulse's central gain is .12 + .88*(6*6/256).
            Assert.That(terrain.SampleHeight(64,64), Is.EqualTo(23.6f).Within(.0001f));
            Assert.That(terrain.SampleHeight(48,64), Is.EqualTo(8).Within(.0001f));
            Assert.That(terrain.SampleHeight(60,64), Is.GreaterThan(8));
            Assert.That(heights[Index(16,16)], Is.EqualTo(144), "source data stays untouched");
            water[Index(15,15)]=8;
            terrain.Update(heights,types,water,3,1);
            Assert.That(terrain.SampleHeight(64,64), Is.EqualTo(72).Within(.0001f), "wet corner stays exact");
        }

        [Test]
        public void WaterOverlapCoversOnlyAdjacentDryCellsAndClearsAfterMutation()
        {
            Array.Fill(heights,(byte)40);Array.Clear(water,0,water.Length);
            water[Index(16,16)]=8;terrain.Update(heights,types,water,4,0);
            Section(terrain,parent,0,0);
            var node=parent.transform.Find("Modern Terrain/Terrain 0,0 copy 4/Shore overlap");
            var mesh=node.GetComponent<MeshFilter>().sharedMesh;
            var vertices=mesh.vertices;var triangles=mesh.triangles;
            Assert.That(triangles.Length,Is.EqualTo(8*4*6));
            Assert.That(mesh.bounds.min.y,Is.LessThanOrEqualTo(.02f));
            for(var i=0;i<triangles.Length;i+=3)
            {
                var center=(vertices[triangles[i]]+vertices[triangles[i+1]]+vertices[triangles[i+2]])/3;
                Assert.That(terrain.IsWater(center.x,center.z),Is.False,"the apron must not double-render wet cells");
                Assert.That(Mathf.Abs(Mathf.FloorToInt(center.x/4)-16),Is.LessThanOrEqualTo(1));
                Assert.That(Mathf.Abs(Mathf.FloorToInt(center.z/4)-16),Is.LessThanOrEqualTo(1));
            }
            water[Index(16,16)]=0;terrain.Update(heights,types,water,4,1);Section(terrain,parent,0,0);
            Assert.That(mesh.triangles,Is.Empty);Assert.That(node.GetComponent<MeshRenderer>().enabled,Is.False);
        }

        [Test]
        public void CalmSeaLevelWaterUsesMaterialWithoutInteriorWaveFlag()
        {
            Array.Fill(heights,(byte)0);Array.Fill(types,(byte)1);Array.Clear(water,0,water.Length);
            types[Index(16,16)]=0;terrain.Update(heights,types,water,5,0);
            Assert.That(terrain.IsWater(65,65),Is.True);
            Assert.That(terrain.IsWater(61,65),Is.False);
            Assert.That(Section(terrain,parent,0,0).GetTriangles(1).Length,Is.EqualTo(24));
            types[Index(16,16)]=1;terrain.Update(heights,types,water,5,1);
            Assert.That(terrain.IsWater(65,65),Is.False);
            Assert.That(Section(terrain,parent,0,0).GetTriangles(1),Is.Empty);
            types[Index(16,16)]=0;heights[Index(16,16)]=10;terrain.Update(heights,types,water,5,2);
            Assert.That(terrain.IsWater(65,65),Is.False,"raised dry type-0 ground is not flooded by the presentation rule");
        }

        [Test]
        public void WaterVisibilityUsesCurrentWaterAndWrappedActiveCopies()
        {
            Array.Clear(water, 0, water.Length);
            water[Index(255, 255)] = 8;
            terrain.Update(heights, types, water, 1, 1);
            terrain.ViewDistance = 100f;
            terrain.Present(new Vector3(-2f, 200f, -2f));
            var cameraObject = new GameObject("Water visibility camera");
            try
            {
                var camera = cameraObject.AddComponent<Camera>();
                camera.orthographic = true;
                camera.orthographicSize = 8f;
                camera.farClipPlane = 400f;
                camera.transform.position = new Vector3(-2f, 200f, -2f);
                camera.transform.rotation = Quaternion.Euler(90f, 0f, 0f);
                Assert.That(terrain.WaterVisible(camera), Is.True);
                camera.transform.rotation = Quaternion.Euler(-90f, 0f, 0f);
                Assert.That(terrain.WaterVisible(camera), Is.False);
                // High land in the same chunk must not enlarge the water bound.
                for (var i = 0; i < heights.Length; i++) heights[i] = 120;
                heights[Index(255, 255)] = heights[Index(0, 255)] = 0;
                heights[Index(255, 0)] = heights[Index(0, 0)] = 0;
                terrain.Update(heights, types, water, 1, 2);
                camera.transform.position = new Vector3(-2f, 25f, -50f);
                camera.transform.rotation = Quaternion.identity;
                Assert.That(terrain.WaterVisible(camera), Is.False);
                camera.transform.position = new Vector3(-2f, 200f, -2f);
                camera.transform.rotation = Quaternion.Euler(90f, 0f, 0f);
                Assert.That(terrain.WaterVisible(camera), Is.True);
                water[Index(255, 255)] = 0;
                terrain.Update(heights, types, water, 1, 3);
                Assert.That(terrain.WaterVisible(camera), Is.False);
            }
            finally { UnityEngine.Object.DestroyImmediate(cameraObject); }
        }

        private static void AssertClosedInteriorEdges(Mesh mesh)
        {
            var edges = new Dictionary<long, int>();
            for (var submesh = 0; submesh < 2; submesh++)
            {
                var triangles = mesh.GetTriangles(submesh);
                for (var i = 0; i < triangles.Length; i += 3)
                {
                    CountEdge(edges, triangles[i], triangles[i + 1]);
                    CountEdge(edges, triangles[i + 1], triangles[i + 2]);
                    CountEdge(edges, triangles[i + 2], triangles[i]);
                }
            }
            foreach (var edge in edges)
            {
                var a = (int)(edge.Key >> 32);
                var b = (int)edge.Key;
                var boundary = (a % Side == 0 && b % Side == 0)
                    || (a % Side == Side - 1 && b % Side == Side - 1)
                    || (a / Side == 0 && b / Side == 0)
                    || (a / Side == Side - 1 && b / Side == Side - 1);
                Assert.That(edge.Value, Is.EqualTo(boundary ? 1 : 2), $"edge {a}--{b}");
            }
        }

        private void AssertMaterialCoverage(Mesh mesh)
        {
            var vertices = mesh.vertices;
            var areas = new float[2, 32 * 32];
            for (var submesh = 0; submesh < 2; submesh++)
            {
                var triangles = mesh.GetTriangles(submesh);
                for (var i = 0; i < triangles.Length; i += 3)
                {
                    var a = vertices[triangles[i]];
                    var b = vertices[triangles[i + 1]];
                    var c = vertices[triangles[i + 2]];
                    var x = Mathf.FloorToInt((a.x + b.x + c.x) / 12f);
                    var z = Mathf.FloorToInt((a.z + b.z + c.z) / 12f);
                    Assert.That(x, Is.InRange(0, 31));
                    Assert.That(z, Is.InRange(0, 31));
                    Assert.That((water[Index(x, z)] & 8) != 0, Is.EqualTo(submesh == 1), $"cell {x},{z}");
                    var area = Mathf.Abs((b.x - a.x) * (c.z - a.z) - (b.z - a.z) * (c.x - a.x)) * 0.5f;
                    Assert.That(area, Is.GreaterThan(0));
                    areas[submesh, x + z * 32] += area;
                }
            }
            for (var z = 0; z < 32; z++)
            for (var x = 0; x < 32; x++)
            {
                var submesh = (water[Index(x, z)] & 8) != 0 ? 1 : 0;
                Assert.That(areas[submesh, x + z * 32], Is.EqualTo(16f).Within(0.0001f), $"cell {x},{z}");
            }
        }

        private static HashSet<long> ShoreEdges(Mesh mesh)
        {
            var landEdges = new Dictionary<long, int>();
            var waterEdges = new Dictionary<long, int>();
            for (var submesh = 0; submesh < 2; submesh++)
            {
                var edges = submesh == 0 ? landEdges : waterEdges;
                var triangles = mesh.GetTriangles(submesh);
                for (var i = 0; i < triangles.Length; i += 3)
                {
                    CountEdge(edges, triangles[i], triangles[i + 1]);
                    CountEdge(edges, triangles[i + 1], triangles[i + 2]);
                    CountEdge(edges, triangles[i + 2], triangles[i]);
                }
            }
            var shore = new HashSet<long>();
            foreach (var edge in waterEdges)
                if (landEdges.ContainsKey(edge.Key)) shore.Add(edge.Key);
            return shore;
        }

        private static void CountEdge(Dictionary<long, int> edges, int a, int b)
        {
            var key = ((long)Math.Min(a, b) << 32) | (uint)Math.Max(a, b);
            edges.TryGetValue(key, out var count);
            edges[key] = count + 1;
        }

        private void AssertWaterCell(Mesh mesh, int x, int z)
        {
            var vertices = mesh.vertices;
            for (var dz = 0; dz <= 2; dz++)
            for (var dx = 0; dx <= 2; dx++)
            {
                var index = (x % 32 * 2 + dx) + (z % 32 * 2 + dz) * Side;
                Assert.That(vertices[index].y, Is.EqualTo(SourceHeight(x, z, dx * 0.5f, dz * 0.5f,true)).Within(0.00001f));
            }
            Assert.That(mesh.GetTriangles(1).Length, Is.GreaterThanOrEqualTo(24));
            Assert.That(terrain.SampleHeight(x * 4f + 1f, z * 4f + 3f),
                Is.EqualTo(SourceHeight(x, z, 0.25f, 0.75f,true)).Within(0.0001f));
        }

        private float SourceHeight(int x, int z, float fx, float fz,bool coastal=false)
        {
            var a = heights[Index(x, z)] * 0.5f;
            var b = heights[Index(x + 1, z)] * 0.5f;
            var c = heights[Index(x, z + 1)] * 0.5f;
            var d = heights[Index(x + 1, z + 1)] * 0.5f;
            if(coastal)
            {
                float Bank(float h) => h-.45f*(1-Mathf.SmoothStep(0,1,Mathf.InverseLerp(1,3,h)));
                a=Bank(a);b=Bank(b);c=Bank(c);d=Bank(d);
            }
            if (((x + z) & 1) == 0)
                return fx <= fz ? a + fz * (c - a) + fx * (d - c) : a + fz * (d - b) + fx * (b - a);
            return fx + fz >= 1f ? c + (1f - fz) * (b - d) + fx * (d - c) : a + fz * (c - a) + fx * (b - a);
        }

        private static int Index(int x, int z) => (x & 255) + (z & 255) * MapCells;

        private static Mesh Section(ModernTerrainMesh owner, GameObject root, int x, int z)
        {
            owner.Present(new Vector3(x * 128f + 64f, 0f, z * 128f + 64f));
            var name = $"Modern terrain section {x + z * 8}";
            foreach (var filter in root.GetComponentsInChildren<MeshFilter>(true))
                if (filter.sharedMesh != null && filter.sharedMesh.name == name) return filter.sharedMesh;
            Assert.Fail($"Terrain section {x},{z} was not presented");
            return null;
        }

        private static int[] Slice(int[] values, int offset, int count)
        {
            var result = new int[count];
            Array.Copy(values, offset, result, 0, count);
            return result;
        }
    }
}
