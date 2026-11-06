using System;
using System.Collections;
using System.Collections.Generic;
using System.Reflection;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.Rendering;

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
            Array.Fill(types,(byte)1); // Explicit dry material; type 0 is calm water.
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
        public void DistantRepresentationsHaveValidIndicesAndPreserveCoverage()
        {
            var original = Field<Mesh[]>(vegetation, "meshes");
            var intermediate = Field<Mesh[]>(vegetation, "intermediateMeshes");
            var distant = Field<Mesh[]>(vegetation, "distantMeshes");
            var nearMaterials = Field<Material[]>(vegetation, "materials");
            var farMaterials = Field<Material[]>(vegetation, "distantMaterials");
            long originalTriangles = 0;
            long distantTriangles = 0;
            for (var kind = 0; kind < original.Length; kind++)
            {
                Assert.That(nearMaterials[kind].enableInstancing, Is.True);
                Assert.That(farMaterials[kind].enableInstancing, Is.True);
                Assert.That(nearMaterials[kind].GetFloat("_LodThreshold"), Is.EqualTo(-1f));
                Assert.That(farMaterials[kind].GetFloat("_LodThreshold"), Is.Zero);
                Assert.That(distant[kind].uv.Length, Is.EqualTo(distant[kind].vertexCount));
                Assert.That(intermediate[kind].GetIndexCount(0), Is.GreaterThan(0).And.LessThanOrEqualTo(original[kind].GetIndexCount(0)));
                for (var sub = 0; sub < distant[kind].subMeshCount; sub++)
                {
                    if (sub < original[kind].subMeshCount) originalTriangles += original[kind].GetTriangles(sub).Length / 3;
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
            foreach (var kind in new[] { 0, 1, 2, 6 })
            {
                Assert.That(farMaterials[kind].shader.name, Is.EqualTo("MagicCarpet/ModernVegetationImpostor"));
                Assert.That(farMaterials[kind].GetTexture("_BaseMap"), Is.Not.Null);
                Assert.That(farMaterials[kind].GetTexture("_ImpostorData"), Is.Not.Null);
                Assert.That(distant[kind].GetIndexCount(0), Is.EqualTo(6), "one camera-facing quad");
                Assert.That(distant[kind].bounds.Contains(original[kind].bounds.min), Is.True);
                Assert.That(distant[kind].bounds.Contains(original[kind].bounds.max), Is.True);
            }
            Assert.That(distant[4].GetIndexCount(0), Is.LessThan(original[4].GetIndexCount(0) / 2), "rock");
            Assert.That(distant[5].GetIndexCount(0), Is.LessThan(original[5].GetIndexCount(0) / 2), "tree branches");
            AssertWholeLeafCards(original[6], intermediate[6]);
            var originalLeafBounds = ReferencedBounds(original[6]);
            var intermediateLeafBounds = ReferencedBounds(intermediate[6]);
            Assert.That(Vector3.Distance(originalLeafBounds.min, intermediateLeafBounds.min), Is.LessThan(.0001f));
            Assert.That(Vector3.Distance(originalLeafBounds.max, intermediateLeafBounds.max), Is.LessThan(.0001f));
        }

        [Test]
        public void AuthoredIntermediateMeshesMatchTheQualifiedPilotFactory()
        {
            var authored = Field<Mesh[]>(vegetation, "intermediateMeshes");
            for (var kind = 0; kind < authored.Length; kind++)
            {
                var reference = ModernVegetation.CreateIntermediateForAuthoring(kind);
                try
                {
                    var actual = authored[kind];
                    Assert.That(actual.vertexCount, Is.EqualTo(reference.vertexCount), $"kind {kind}");
                    Assert.That(actual.subMeshCount, Is.EqualTo(reference.subMeshCount), $"kind {kind}");
                    CollectionAssert.AreEqual(reference.vertices, actual.vertices, $"vertices kind {kind}");
                    CollectionAssert.AreEqual(reference.normals, actual.normals, $"normals kind {kind}");
                    CollectionAssert.AreEqual(reference.tangents, actual.tangents, $"tangents kind {kind}");
                    CollectionAssert.AreEqual(reference.colors, actual.colors, $"colors kind {kind}");
                    CollectionAssert.AreEqual(reference.uv, actual.uv, $"UV kind {kind}");
                    Assert.That(actual.bounds, Is.EqualTo(reference.bounds), $"bounds kind {kind}");
                    for (var sub = 0; sub < reference.subMeshCount; sub++)
                    {
                        Assert.That(actual.GetTopology(sub), Is.EqualTo(reference.GetTopology(sub)), $"topology kind {kind}");
                        CollectionAssert.AreEqual(reference.GetIndices(sub), actual.GetIndices(sub), $"indices kind {kind}");
                    }
                }
                finally { UnityEngine.Object.DestroyImmediate(reference); }
            }
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
            var oldLodCenter = LodCenter(before, 0, 0);

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
            var newLodCenter = LodCenter(after, 0, 0);
            Assert.That(newLodCenter.x, Is.EqualTo(oldLodCenter.x).Within(.0001f));
            Assert.That(newLodCenter.z, Is.EqualTo(oldLodCenter.z).Within(.0001f));
            Assert.That(newLodCenter.y, Is.EqualTo(oldLodCenter.y + 4f).Within(.0001f));
        }

        [Test]
        public void ProjectedSizeRespondsToDistanceFieldOfViewAndInstanceScale()
        {
            var cameraObject = new GameObject("LOD camera");
            try
            {
                var camera = cameraObject.AddComponent<Camera>();
                camera.pixelRect = new Rect(0, 0, 1280, 720);
                camera.fieldOfView = 60f;
                var method = typeof(ModernVegetation).GetMethod("ProjectedPixels", BindingFlags.Static | BindingFlags.NonPublic);
                var bounds = new Bounds(Vector3.zero, Vector3.one * 2f);
                float Project(Vector3 position, float scale) => (float)method.Invoke(null, new object[] {
                    camera.transform.position, camera.transform.forward,
                    Mathf.Abs(camera.projectionMatrix.m11) * camera.pixelHeight * .5f, camera.orthographic,
                    Matrix4x4.TRS(position, Quaternion.identity, Vector3.one * scale), bounds });
                var near = Project(new Vector3(0, 0, 10), 1);
                var far = Project(new Vector3(0, 0, 20), 1);
                Assert.That(near, Is.EqualTo(far * 2).Within(.01f));
                Assert.That(Project(new Vector3(0, 0, 10), 2), Is.EqualTo(near * 2).Within(.01f));
                Assert.That(Project(new Vector3(8, 0, 10), 1), Is.EqualTo(near).Within(.01f),
                    "objects at equal view depth have the same projected scale across the screen");
                camera.fieldOfView = 90f;
                Assert.That(Project(new Vector3(0, 0, 10), 1), Is.LessThan(near));
            }
            finally { UnityEngine.Object.DestroyImmediate(cameraObject); }
        }

        [Test]
        public void ImpostorQuadAndBoundingSphereCoverOriginalMesh()
        {
            var originals = Field<Mesh[]>(vegetation, "meshes");
            var cards = Field<Mesh[]>(vegetation, "distantMeshes");
            var materials = Field<Material[]>(vegetation, "distantMaterials");
            foreach (var kind in new[] { 0, 1, 2, 6 })
            {
                CollectionAssert.AreEqual(new[] { Vector2.zero, Vector2.right, Vector2.one, Vector2.up }, cards[kind].uv);
                var center = (Vector3)materials[kind].GetVector("_ImpostorCenter");
                var radius = materials[kind].GetFloat("_ImpostorDiameter") * .5f;
                Assert.That(Vector3.Distance(center, originals[kind].bounds.center), Is.LessThan(.0001f));
                foreach (var vertex in originals[kind].vertices)
                    Assert.That(Vector3.Distance(vertex, center), Is.LessThanOrEqualTo(radius));
            }
        }

        [Test]
        public void GroupClassificationIsConservativeAcrossTheTransitionBand()
        {
            var method = typeof(ModernVegetation).GetMethod("GroupLodMode", BindingFlags.Static | BindingFlags.NonPublic);
            int Classify(float nearDepth, float farDepth) => (int)method.Invoke(null, new object[] {
                new Bounds(new Vector3(0, 0, (nearDepth + farDepth) * .5f), new Vector3(0, 0, farDepth - nearDepth)),
                2f, 2f, Vector3.zero, Vector3.forward, 360f, false });
            Assert.That(Classify(10, 12), Is.EqualTo(1), "both objects exceed the detail threshold");
            Assert.That(Classify(25, 30), Is.EqualTo(2), "both objects belong to the intermediate mesh band");
            Assert.That(Classify(80, 100), Is.EqualTo(3), "both objects are below the distant threshold");
            Assert.That(Classify(10, 80), Is.EqualTo(4), "a group spanning thresholds must partition");
        }

        [Test]
        public void ViewCacheInvalidatesForMovementAndProjectionChanges()
        {
            var cameraObject = new GameObject("LOD cache camera");
            try
            {
                var camera = cameraObject.AddComponent<Camera>();
                camera.pixelRect = new Rect(0, 0, 1280, 720);
                var view = Field<object>(vegetation, "mainView");
                var update = view.GetType().GetMethod("Update", BindingFlags.Instance | BindingFlags.NonPublic);
                int Version() => (int)update.Invoke(view, new object[] { camera });
                var initial = Version();
                Assert.That(Version(), Is.EqualTo(initial));
                cameraObject.transform.position = Vector3.right;
                var moved = Version();
                Assert.That(moved, Is.GreaterThan(initial));
                camera.fieldOfView = 80f;
                Assert.That(Version(), Is.GreaterThan(moved));
            }
            finally { UnityEngine.Object.DestroyImmediate(cameraObject); }
        }

        [Test]
        public void VisibleBatchingPreservesMatricesFadesKeysAndCapacity()
        {
            var expected = new Dictionary<(int camera, int kind, int level, ShadowCastingMode shadows, int binX, int binZ), List<(Matrix4x4 matrix, float fade)>>();
            var actual = new Dictionary<(int camera, int kind, int level, ShadowCastingMode shadows, int binX, int binZ), List<(Matrix4x4 matrix, float fade)>>();
            var submissions = new List<int>();
            var fullShadowBin = false;
            void Capture(int camera, ModernVegetation.BatchBuffer buffer)
            {
                Assert.That(buffer.Instances.Count, Is.InRange(1, 256));
                submissions.Add(buffer.Instances.Count);
                if (buffer.Shadows == ShadowCastingMode.On && buffer.Instances.Count == 256) fullShadowBin = true;
                var key = (camera, buffer.Kind, buffer.Level, buffer.Shadows,
                    buffer.Shadows == ShadowCastingMode.On ? buffer.BinX : 0,
                    buffer.Shadows == ShadowCastingMode.On ? buffer.BinZ : 0);
                if (!actual.TryGetValue(key, out var entries)) actual[key] = entries = new List<(Matrix4x4, float)>();
                var properties = buffer.Properties();
                var uploaded = properties == null ? null : properties.GetFloatArray("_LodThreshold");
                for (var i = 0; i < buffer.Instances.Count; i++)
                {
                    if (buffer.Shadows == ShadowCastingMode.On)
                    {
                        var position = buffer.Instances[i].GetColumn(3);
                        Assert.That(Mathf.FloorToInt(position.x / 64f), Is.EqualTo(buffer.BinX));
                        Assert.That(Mathf.FloorToInt(position.z / 64f), Is.EqualTo(buffer.BinZ));
                    }
                    var fade = buffer.Fades[i];
                    if (uploaded == null) Assert.That(fade, Is.EqualTo(buffer.Level == 2 ? 0f : -1f));
                    else Assert.That(uploaded[i], Is.EqualTo(fade));
                    entries.Add((buffer.Instances[i], fade));
                }
            }
            var main = new ModernVegetation.BatchAccumulator(buffer => Capture(0, buffer));
            var reflection = new ModernVegetation.BatchAccumulator(buffer => Capture(1, buffer));
            var serial = 0;
            void Add(ModernVegetation.BatchAccumulator accumulator, int camera, int kind, int level,
                ShadowCastingMode shadows, int count, bool withFades)
            {
                var matrices = new List<Matrix4x4>(count);
                var fades = withFades ? new float[count] : null;
                for (var i = 0; i < count; i++)
                {
                    // On-mode includes negative bins, a boundary, and a 256 m
                    // direct-map collision. Off-mode remains globally grouped.
                    var x = shadows == ShadowCastingMode.On ?
                        (kind == 5 ? .1f : i % 4 == 0 ? 0f : i % 4 == 1 ? 256f : i % 4 == 2 ? -.1f : 64f) : serial;
                    var z = shadows == ShadowCastingMode.On ? (kind == 5 || i % 4 < 2 ? 0f : -.1f) : level;
                    var matrix = Matrix4x4.Translate(new Vector3(x, serial++, z));
                    var fade = withFades ? ((i % 19) - 9) / 10f : level == 2 ? 0f : -1f;
                    var key = (camera, kind, level, shadows,
                        shadows == ShadowCastingMode.On ? Mathf.FloorToInt(x / 64f) : 0,
                        shadows == ShadowCastingMode.On ? Mathf.FloorToInt(z / 64f) : 0);
                    if (!expected.TryGetValue(key, out var entries)) expected[key] = entries = new List<(Matrix4x4, float)>();
                    matrices.Add(matrix);
                    if (fades != null) fades[i] = fade;
                    entries.Add((matrix, fade));
                }
                accumulator.Add(kind, level, shadows, matrices, fades);
            }

            Add(main, 0, 0, 0, ShadowCastingMode.Off, 170, false);
            Add(main, 0, 4, 2, ShadowCastingMode.ShadowsOnly, 37, false);
            Add(main, 0, 0, 0, ShadowCastingMode.Off, 130, true);
            Add(reflection, 1, 0, 0, ShadowCastingMode.Off, 23, true);
            Add(main, 0, 0, 0, ShadowCastingMode.Off, 300, false);
            Add(main, 0, 4, 2, ShadowCastingMode.On, 265, true);
            Add(main, 0, 4, 2, ShadowCastingMode.On, 70, false);
            Add(main, 0, 5, 1, ShadowCastingMode.On, 300, true);
            Add(reflection, 1, 4, 2, ShadowCastingMode.On, 7, false);
            main.Flush(); reflection.Flush();
            // Buffers may be reused after a completed camera render.
            Add(main, 0, 0, 0, ShadowCastingMode.Off, 3, false);
            main.Flush();

            Assert.That(actual.Count, Is.EqualTo(expected.Count));
            foreach (var entry in expected)
            {
                Assert.That(actual.ContainsKey(entry.Key), Is.True, $"missing key {entry.Key}");
                CollectionAssert.AreEqual(entry.Value, actual[entry.Key], $"matrix/fade stream {entry.Key}");
            }
            Assert.That(submissions, Does.Contain(256), "large streams must flush at the proven cap");
            Assert.That(fullShadowBin, Is.True, "a single shadow bin must also split at 256");
            Assert.Throws<ArgumentOutOfRangeException>(() => main.Add(0, 0, ShadowCastingMode.TwoSided, new List<Matrix4x4>(), null));
        }

        private static T Field<T>(object owner, string name) => (T)owner.GetType()
            .GetField(name, BindingFlags.Instance | BindingFlags.NonPublic).GetValue(owner);

        private static Vector3 LodCenter(object chunk, int kind, int index)
        {
            var lists = (Array)chunk.GetType().GetField("LodInstances", BindingFlags.Instance | BindingFlags.NonPublic)
                .GetValue(chunk);
            var entry = ((IList)lists.GetValue(kind))[index];
            return (Vector3)entry.GetType().GetField("Center", BindingFlags.Instance | BindingFlags.NonPublic).GetValue(entry);
        }

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
