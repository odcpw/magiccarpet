using System;
using NUnit.Framework;
using UnityEngine;

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
                types[Index(x, z)] = (byte)((x + 3 * z) % 7);
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
        public void SubdivisionUsesOriginalDiagonalsAndSamplingMatchesMesh()
        {
            var mesh = Section(terrain, parent, 0, 0);
            var vertices = mesh.vertices;
            Assert.That(vertices.Length, Is.EqualTo(Side * Side));
            var indices = mesh.GetTriangles(0);
            // The first two source cells have opposite diagonals; each has four quads.
            CollectionAssert.AreEqual(new[] { 0, 65, 66, 0, 66, 1 }, Slice(indices, 0, 6));
            CollectionAssert.AreEqual(new[] { 2, 67, 3, 67, 68, 3 }, Slice(indices, 12, 6));
            var hasRelief = false;
            for (var z = 0; z < Side; z++)
            for (var x = 0; x < Side; x++)
            {
                var sourceX = x / 2;
                var sourceZ = z / 2;
                var baseline = SourceHeight(sourceX, sourceZ, (x & 1) * 0.5f, (z & 1) * 0.5f);
                var difference = Mathf.Abs(vertices[x + z * Side].y - baseline);
                Assert.That(difference, Is.LessThanOrEqualTo(0.1501f));
                if ((x & 1) == 0 && (z & 1) == 0) Assert.That(difference, Is.LessThan(0.00001f));
                else if (difference > 0.001f) hasRelief = true;
            }
            Assert.That(hasRelief, Is.True, "the subdivision should add visible geometry");

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
        public void WaterAndShoreStayOnSourceSurfaceAndWrappedNormalsMatch()
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
        public void MutationsRefreshAffectedSectionsAcrossBothSeams()
        {
            // Instantiate the affected sections before the update, just as presentation does.
            var coordinates = new[,] { { 0, 0 }, { 7, 0 }, { 0, 7 }, { 7, 7 }, { 1, 1 } };
            var before = new Mesh[coordinates.GetLength(0)];
            for (var i = 0; i < before.Length; i++)
                before[i] = Section(terrain, parent, coordinates[i, 0], coordinates[i, 1]);
            var oldHeight = before[0].vertices[0].y;
            heights[Index(0, 0)] += 10;
            heights[Index(32, 32)] += 5;
            water[Index(255, 255)] = 0;
            water[Index(32, 31)] = 8;
            types[Index(32, 32)] += 1;
            terrain.Update(heights, types, water, 1, 1);
            Assert.That(before[0].vertices[0].y, Is.EqualTo(oldHeight + 5f).Within(0.00001f));
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

        private void AssertWaterCell(Mesh mesh, int x, int z)
        {
            var vertices = mesh.vertices;
            for (var dz = 0; dz <= 2; dz++)
            for (var dx = 0; dx <= 2; dx++)
            {
                var index = (x % 32 * 2 + dx) + (z % 32 * 2 + dz) * Side;
                Assert.That(vertices[index].y, Is.EqualTo(SourceHeight(x, z, dx * 0.5f, dz * 0.5f)).Within(0.00001f));
            }
            Assert.That(mesh.GetTriangles(1).Length, Is.GreaterThanOrEqualTo(24));
            Assert.That(terrain.SampleHeight(x * 4f + 1f, z * 4f + 3f),
                Is.EqualTo(SourceHeight(x, z, 0.25f, 0.75f)).Within(0.0001f));
        }

        private float SourceHeight(int x, int z, float fx, float fz)
        {
            var a = heights[Index(x, z)] * 0.5f;
            var b = heights[Index(x + 1, z)] * 0.5f;
            var c = heights[Index(x, z + 1)] * 0.5f;
            var d = heights[Index(x + 1, z + 1)] * 0.5f;
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
