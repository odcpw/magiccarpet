using System;
using System.Collections;
using System.Collections.Generic;
using System.Reflection;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernVegetationEditModeTests
    {
        private GameObject root;
        private Material material;
        private ModernTerrainMesh terrain;
        private ModernVegetation vegetation;
        private byte[] heights;
        private byte[] types;
        private byte[] water;

        [SetUp]
        public void SetUp()
        {
            heights = new byte[256 * 256];
            types = new byte[heights.Length];
            water = new byte[heights.Length];
            root = new GameObject("Vegetation test terrain");
            material = new Material(Shader.Find("Hidden/InternalErrorShader"));
            terrain = new ModernTerrainMesh(root.transform, material, material);
            terrain.Update(heights, types, water, 1, 0);
            vegetation = new ModernVegetation(terrain);
        }

        [TearDown]
        public void TearDown()
        {
            vegetation?.Dispose();
            terrain?.Dispose();
            UnityEngine.Object.DestroyImmediate(root);
            UnityEngine.Object.DestroyImmediate(material);
        }

        [Test]
        public void DistantScannedMeshesRetainUvAndHaveValidTriangleIndices()
        {
            var original = Field<Mesh[]>(vegetation, "meshes");
            var distant = Field<Mesh[]>(vegetation, "distantMeshes");
            long originalTriangles = 0;
            long distantTriangles = 0;
            for (var kind = 0; kind < original.Length; kind++)
            {
                CollectionAssert.AreEqual(original[kind].vertices, distant[kind].vertices);
                CollectionAssert.AreEqual(original[kind].uv, distant[kind].uv);
                for (var sub = 0; sub < original[kind].subMeshCount; sub++)
                {
                    originalTriangles += original[kind].GetTriangles(sub).Length / 3;
                    var indices = distant[kind].GetTriangles(sub);
                    distantTriangles += indices.Length / 3;
                    Assert.That(indices.Length % 3, Is.Zero);
                    for (var i = 0; i < indices.Length; i += 3)
                    {
                        var a = indices[i]; var b = indices[i + 1]; var c = indices[i + 2];
                        if (a < 0 || b < 0 || c < 0 || a >= distant[kind].vertexCount ||
                            b >= distant[kind].vertexCount || c >= distant[kind].vertexCount ||
                            a == b || b == c || c == a)
                            Assert.Fail($"invalid triangle in kind {kind}, submesh {sub}, offset {i}: {a}, {b}, {c}");
                    }
                }
            }
            Assert.That(distantTriangles, Is.GreaterThan(0).And.LessThan(originalTriangles),
                "The actual imported meshes must produce a smaller distant geometry set.");
            Assert.That(distant[5].GetIndexCount(0), Is.LessThan(original[5].GetIndexCount(0) * .8f), "tree branches");
            Assert.That(distant[6].GetIndexCount(0), Is.LessThan(original[6].GetIndexCount(0) * .75f), "tree leaf cards");
            AssertWholeLeafCards(original[6], distant[6]);
            var originalBounds = ReferencedBounds(original[6]);
            var distantBounds = ReferencedBounds(distant[6]);
            Assert.That(Vector3.Distance(originalBounds.min, distantBounds.min), Is.LessThan(.0001f));
            Assert.That(Vector3.Distance(originalBounds.max, distantBounds.max), Is.LessThan(.0001f));
        }

        private static void AssertWholeLeafCards(Mesh original, Mesh distant)
        {
            var vertices = original.vertices;
            var source = original.GetTriangles(0);
            var selected = distant.GetTriangles(0);
            var present = new HashSet<(int, int, int)>();
            for (var i = 0; i < selected.Length; i += 3)
                present.Add((selected[i], selected[i + 1], selected[i + 2]));
            var positionIds = new Dictionary<Vector3, int>();
            var vertexIds = new int[vertices.Length];
            for (var i = 0; i < vertices.Length; i++)
            {
                if (!positionIds.TryGetValue(vertices[i], out var id))
                {
                    id = positionIds.Count;
                    positionIds.Add(vertices[i], id);
                }
                vertexIds[i] = id;
            }
            var edges = new Dictionary<(int, int), int>();
            var paired = new bool[source.Length / 3];
            var pairCount = 0;
            for (var tri = 0; tri < paired.Length; tri++)
            for (var edge = 0; edge < 3 && !paired[tri]; edge++)
            {
                var a = vertexIds[source[tri * 3 + edge]];
                var b = vertexIds[source[tri * 3 + (edge + 1) % 3]];
                if (a == b) continue;
                var key = a < b ? (a, b) : (b, a);
                if (edges.TryGetValue(key, out var other) && !paired[other] &&
                    Vector3.Dot(Normal(vertices, source, tri), Normal(vertices, source, other)) > .995f)
                {
                    paired[tri] = paired[other] = true;
                    pairCount++;
                    var first = (source[tri * 3], source[tri * 3 + 1], source[tri * 3 + 2]);
                    var second = (source[other * 3], source[other * 3 + 1], source[other * 3 + 2]);
                    Assert.That(present.Contains(first), Is.EqualTo(present.Contains(second)), "a leaf card lost half of its triangles");
                }
                else if (!edges.ContainsKey(key)) edges.Add(key, tri);
            }
            Assert.That(pairCount * 2, Is.GreaterThan(paired.Length * .95f), "expected the scanned crown to consist of paired cards");
        }

        private static Vector3 Normal(Vector3[] vertices, int[] indices, int triangle)
        {
            var a = vertices[indices[triangle * 3]];
            return Vector3.Cross(vertices[indices[triangle * 3 + 1]] - a,
                vertices[indices[triangle * 3 + 2]] - a).normalized;
        }

        private static Bounds ReferencedBounds(Mesh mesh)
        {
            var vertices = mesh.vertices;
            var indices = mesh.GetTriangles(0);
            Assert.That(indices.Length, Is.GreaterThan(0));
            var bounds = new Bounds(vertices[indices[0]], Vector3.zero);
            foreach (var index in indices) bounds.Encapsulate(vertices[index]);
            return bounds;
        }

        [Test]
        public void ChunkIsRetainedAcrossMovementAndRegroundedAfterTerrainRevision()
        {
            vegetation.RefreshChunks(new Vector3(1f, 0f, 1f));
            var chunks = Field<IDictionary>(vegetation, "chunks");
            var key = new Vector2Int(0, 0);
            var before = chunks[key];
            Assert.That(before, Is.Not.Null);
            var instances = ChunkInstances(before);
            Assert.That(instances[0].Count, Is.GreaterThan(0));
            var oldPosition = ((Matrix4x4)instances[0][0]).GetColumn(3);

            vegetation.RefreshChunks(new Vector3(33f, 0f, 1f));
            Assert.That(chunks[key], Is.SameAs(before), "moving one chunk should retain overlapping batches");

            Array.Fill(heights, (byte)8);
            terrain.Update(heights, types, water, 1, 1);
            vegetation.RefreshChunks(new Vector3(33f, 0f, 1f));
            var after = chunks[key];
            Assert.That(after, Is.Not.SameAs(before));
            var updated = ChunkInstances(after);
            Assert.That(updated[0].Count, Is.EqualTo(instances[0].Count));
            var newPosition = ((Matrix4x4)updated[0][0]).GetColumn(3);
            Assert.That(newPosition.x, Is.EqualTo(oldPosition.x).Within(.0001f));
            Assert.That(newPosition.z, Is.EqualTo(oldPosition.z).Within(.0001f));
            Assert.That(newPosition.y, Is.EqualTo(oldPosition.y + 4f).Within(.0001f));
        }

        private static T Field<T>(object owner, string name) => (T)owner.GetType()
            .GetField(name, BindingFlags.Instance | BindingFlags.NonPublic).GetValue(owner);

        private static IList[] ChunkInstances(object chunk)
        {
            var lists = (Array)chunk.GetType().GetField("Instances", BindingFlags.Instance | BindingFlags.NonPublic)
                .GetValue(chunk);
            var result = new IList[lists.Length];
            for (var i = 0; i < lists.Length; i++) result[i] = (IList)lists.GetValue(i);
            return result;
        }
    }
}
