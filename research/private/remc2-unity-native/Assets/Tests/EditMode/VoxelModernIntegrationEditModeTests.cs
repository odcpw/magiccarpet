using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using MagicCarpet.Native.Voxels;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native.Tests
{
    public sealed class VoxelModernIntegrationEditModeTests
    {
        [Serializable]
        private sealed class Level1Input { public ModernBuildingMeshes.Profile profile; }

        [Test]
        public void AlignedPatchCutsBothTerrainLodsAndKeepsTheirOutsideTriangles()
        {
            var landShader = Resources.Load<Shader>("ModernTerrain");
            var waterShader = Resources.Load<Shader>("ModernWater");
            Assert.That(landShader, Is.Not.Null);
            Assert.That(waterShader, Is.Not.Null);
            var land = new Material(landShader);
            var water = new Material(waterShader);
            var parent = new GameObject("Modern voxel terrain test");
            var terrain = new ModernTerrainMesh(parent.transform, land, water) { ViewDistance = 650f };
            try
            {
                Assert.That(land.FindPass("ShadowCaster"), Is.GreaterThanOrEqualTo(0));
                Assert.That(land.FindPass("DepthOnly"), Is.GreaterThanOrEqualTo(0));
                var heights = new byte[256 * 256]; Array.Fill(heights, (byte)40);
                var types = new byte[heights.Length]; Array.Fill(types, (byte)1);
                var angles = new byte[heights.Length];
                terrain.Update(heights, types, angles, 1, 0);
                var near = SectionMesh(terrain, "Mesh");
                var far = SectionMesh(terrain, "FarMesh");
                var baselineNear = near.GetTriangles(0);
                var baselineFar = far.GetTriangles(0);
                var patch = new Bounds(new Vector3(48, 0, 48), new Vector3(32, 100, 32));
                terrain.VoxelPatch = patch;
                // A new generation forces a rebuild after selecting the patch.
                terrain.Update(heights, types, angles, 2, 0);
                AssertCutAndPreserved(near, baselineNear, patch);
                AssertCutAndPreserved(far, baselineFar, patch);
                Assert.That(near.GetTriangles(1), Is.Empty);
                Assert.That(far.GetTriangles(1), Is.Empty);

                // The actual renderer uses these same cut meshes for its forward,
                // shadow-caster and depth passes, rather than a forward-only clip.
                terrain.Present(new Vector3(64, 20, 64));
                var copy = parent.transform.Find("Modern Terrain/Terrain 0,0 copy 4");
                Assert.That(copy, Is.Not.Null);
                var filter = copy.GetComponent<MeshFilter>();
                var renderer = copy.GetComponent<MeshRenderer>();
                Assert.That(renderer.sharedMaterials[0], Is.SameAs(land));
                Assert.That(renderer.shadowCastingMode, Is.EqualTo(ShadowCastingMode.On));
                Assert.That(filter.sharedMesh, Is.SameAs(near));
                terrain.Present(new Vector3(500, 20, 64));
                Assert.That(filter.sharedMesh, Is.SameAs(far));
            }
            finally
            {
                terrain.Dispose();
                UnityEngine.Object.DestroyImmediate(parent);
                UnityEngine.Object.DestroyImmediate(land);
                UnityEngine.Object.DestroyImmediate(water);
            }
        }

        private static Mesh SectionMesh(ModernTerrainMesh terrain, string field)
        {
            var fields = BindingFlags.Instance | BindingFlags.NonPublic | BindingFlags.Public;
            var sections = (Array)typeof(ModernTerrainMesh).GetField("sections", fields).GetValue(terrain);
            var section = sections.GetValue(0);
            return (Mesh)section.GetType().GetField(field, fields).GetValue(section);
        }

        private static void AssertCutAndPreserved(Mesh mesh, int[] baseline, Bounds patch)
        {
            var vertices = mesh.vertices;
            var expected = new List<int>();
            for (int i = 0; i < baseline.Length; i += 3)
            {
                var a = vertices[baseline[i]]; var b = vertices[baseline[i + 1]]; var c = vertices[baseline[i + 2]];
                if (OverlapsPatch(a, b, c, patch)) continue;
                expected.Add(baseline[i]); expected.Add(baseline[i + 1]); expected.Add(baseline[i + 2]);
            }
            var actual = mesh.GetTriangles(0);
            Assert.That(actual.Length, Is.GreaterThan(0).And.LessThan(baseline.Length));
            CollectionAssert.AreEqual(expected, actual, "The patch removes only triangles covering its interior.");
            float area = 0;
            for (int i = 0; i < actual.Length; i += 3)
            {
                var a = vertices[actual[i]]; var b = vertices[actual[i + 1]]; var c = vertices[actual[i + 2]];
                Assert.That(OverlapsPatch(a, b, c, patch), Is.False, "No rendered triangle may cross into the patch.");
                area += Mathf.Abs((b.x - a.x) * (c.z - a.z) - (b.z - a.z) * (c.x - a.x)) * .5f;
            }
            Assert.That(area, Is.EqualTo(128 * 128 - 32 * 32).Within(.001f));
        }

        private static bool OverlapsPatch(Vector3 a, Vector3 b, Vector3 c, Bounds patch)
        {
            float minX = Mathf.Min(a.x, Mathf.Min(b.x, c.x));
            float maxX = Mathf.Max(a.x, Mathf.Max(b.x, c.x));
            float minZ = Mathf.Min(a.z, Mathf.Min(b.z, c.z));
            float maxZ = Mathf.Max(a.z, Mathf.Max(b.z, c.z));
            return minX < patch.max.x && maxX > patch.min.x && minZ < patch.max.z && maxZ > patch.min.z;
        }

        [Test]
        public void FullScaleSourceHouseStartsSupportedWithBrickCells()
        {
            var asset = Resources.Load<TextAsset>("ModernCharacters/VoxelSandbox/Level1");
            Assert.That(asset, Is.Not.Null, "Run Tools/prepare-voxel-inputs.py first.");
            var profile = JsonUtility.FromJson<Level1Input>(asset.text).profile;
            Assert.That(profile.build_index, Is.EqualTo(26));
            const float cell = .25f, scale = 2f;
            var size = new Vector3Int(Mathf.RoundToInt((profile.columns - 1) * 2 * scale / cell),
                Mathf.CeilToInt(profile.heights.Max() * .5f * scale / cell),
                Mathf.RoundToInt((profile.rows - 1) * 2 * scale / cell));
            var shader = Shader.Find("MagicCarpet/VoxelSurface");
            Assert.That(shader, Is.Not.Null);
            var material = new Material(shader);
            var parent = new GameObject("Full-scale source house support test");
            try
            {
                using (var house = new VoxelVolume(parent.transform, Vector3.zero, size, cell, material))
                {
                    VoxelSandbox.PopulateHouse(house, profile, cell, scale);
                    house.Build();
                    Assert.That(parent.GetComponentsInChildren<MeshCollider>().Length, Is.GreaterThan(0));
                    Assert.That(house.DetachUnsupported(), Is.Empty);
                }
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(parent);
                UnityEngine.Object.DestroyImmediate(material);
            }
        }
    }
}
