using System;
using System.Collections.Generic;
using System.Linq;
using MagicCarpet.Native.Voxels;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class VoxelVolumeEditModeTests
    {
        private GameObject parent;
        private Material material;
        private VoxelVolume volume;
        private static readonly Color32 Stone = new Color32(120, 90, 70, 255);

        private static (Vector3, Vector3, Color32)[] RenderedVertices(GameObject owner) => owner.GetComponentsInChildren<MeshRenderer>()
            .Where(renderer => renderer.enabled && renderer.gameObject.activeInHierarchy)
            .SelectMany(renderer => {
                var mesh = renderer.GetComponent<MeshFilter>().sharedMesh;
                var positions = mesh.vertices; var normals = mesh.normals; var colors = mesh.colors32;
                return positions.Select((position, i) => (position, normals[i], colors[i]));
            }).ToArray();

        [TestCase(2)]
        [TestCase(4)]
        public void GroupedRenderingRetainsExactFacesAndSmallCollidersDuringChunkEdits(int groupSize)
        {
            var other = new GameObject("Grouped volume");
            var dimensions = new Vector3Int(66, 3, 18);
            var origin = new Vector3(8, 3, -5);
            Create(dimensions, .125f, origin);
            using var grouped = new VoxelVolume(other.transform, origin, dimensions, .125f, material, groupSize);
            try
            {
                for (int z = 0; z < dimensions.z; z++)
                for (int x = 0; x < dimensions.x; x++)
                {
                    var color = new Color32((byte)x, (byte)z, 90, 255);
                    volume.SetCell(x, 0, z, color); grouped.SetCell(x, 0, z, color);
                }
                volume.Build(); grouped.Build();
                void AssertSame()
                {
                    var expected = RenderedVertices(parent).GroupBy(vertex => vertex).ToDictionary(group => group.Key, group => group.Count());
                    var actual = RenderedVertices(other).GroupBy(vertex => vertex).ToDictionary(group => group.Key, group => group.Count());
                    Assert.That(actual.Count == expected.Count && expected.All(pair => actual.TryGetValue(pair.Key, out var count) && count == pair.Value), Is.True,
                        "Rendered positions, normals, colors and face multiplicities must remain exact");
                    Assert.That(other.GetComponentsInChildren<MeshCollider>().Length,
                        Is.EqualTo(parent.GetComponentsInChildren<MeshCollider>().Length));
                    Assert.That(other.GetComponentsInChildren<MeshCollider>().Sum(c => c.sharedMesh.triangles.Length),
                        Is.EqualTo(parent.GetComponentsInChildren<MeshCollider>().Sum(c => c.sharedMesh.triangles.Length)));
                }
                AssertSame();
                Assert.That(other.GetComponentsInChildren<MeshRenderer>().Count(r => r.enabled),
                    Is.LessThan(parent.GetComponentsInChildren<MeshRenderer>().Count(r => r.enabled)));
                var cells = Enumerable.Range(14, 38).Select(x => new Vector3Int(x, 0, 8)).ToArray();
                var legacyEdit = volume.BeginRemoveCells(cells); var groupedEdit = grouped.BeginRemoveCells(cells);
                while (!legacyEdit.Complete || !groupedEdit.Complete)
                {
                    legacyEdit.Step(1); groupedEdit.Step(1); AssertSame();
                }
                CollectionAssert.AreEqual(legacyEdit.Removed.Select(c => c.Position), groupedEdit.Removed.Select(c => c.Position));
            }
            finally { grouped.Dispose(); UnityEngine.Object.DestroyImmediate(other); }
        }

        [Test]
        public void EmptyRenderGroupDisappearsWhenLastPhysicalCellIsRemoved()
        {
            using var grouped = new VoxelVolume(parent.transform, Vector3.zero, new Vector3Int(17, 1, 1), 1, material, 4);
            grouped.SetCell(16, 0, 0, Stone); grouped.Build();
            Assert.That(RenderedVertices(parent).Length, Is.EqualTo(24));
            grouped.RemoveSphere(new Vector3(16.5f, .5f, .5f), .1f);
            Assert.That(RenderedVertices(parent), Is.Empty);
            Assert.That(parent.GetComponentsInChildren<MeshCollider>(), Is.Empty);
        }

        [SetUp]
        public void SetUp()
        {
            parent = new GameObject("Voxel test parent");
            var shader = Shader.Find("Universal Render Pipeline/Lit") ?? Shader.Find("Standard") ?? Shader.Find("Hidden/InternalErrorShader");
            Assert.That(shader, Is.Not.Null);
            material = new Material(shader);
        }

        [TearDown]
        public void TearDown()
        {
            volume?.Dispose();
            UnityEngine.Object.DestroyImmediate(parent);
            UnityEngine.Object.DestroyImmediate(material);
        }

        private void Create(Vector3Int size, float cellSize = 1f, Vector3? origin = null)
        {
            volume = new VoxelVolume(parent.transform, origin ?? Vector3.zero, size, cellSize, material);
        }

        [Test]
        public void NewlyExposedBuriedChunkRetainsTerrainCollisionLayer()
        {
            parent.layer = 8;
            Create(new Vector3Int(48, 48, 48));
            for (int z = 0; z < 48; z++)
            for (int y = 0; y < 48; y++)
            for (int x = 0; x < 48; x++) volume.SetCell(x, y, z, Stone);
            volume.Build();
            Assert.That(parent.GetComponentsInChildren<MeshCollider>().Any(c => c.name == "Voxel Chunk 1,1,1"), Is.False);
            volume.RemoveSphere(new Vector3(24.5f, 24.5f, 24.5f), 1f);
            var exposed = parent.GetComponentsInChildren<MeshCollider>().Single(c => c.name == "Voxel Chunk 1,1,1");
            Assert.That(exposed.gameObject.layer, Is.EqualTo(8));
            Physics.SyncTransforms();
            Assert.That(Physics.Raycast(new Vector3(24.5f, 24.5f, 24.5f), Vector3.down, out _, 3f, 1 << 8), Is.True);
        }

        [Test]
        public void SphereRemovesCentersOnBoundaryAndReturnsWorldSpaceColoredCells()
        {
            var origin = new Vector3(10, 20, 30);
            Create(new Vector3Int(5, 3, 3), 2f, origin);
            for (int z = 0; z < 3; z++)
            for (int y = 0; y < 3; y++)
            for (int x = 0; x < 5; x++) volume.SetCell(x, y, z, Stone);
            volume.Build();
            var center = origin + new Vector3(5, 3, 3);
            var removed = volume.RemoveSphere(center, 2f);
            Assert.That(removed.Count, Is.EqualTo(7));
            Assert.That(removed.All(cell => cell.Size == 2f && cell.Color.Equals(Stone)), Is.True);
            Assert.That(removed.Select(cell => cell.Position), Does.Contain(center));
            Assert.That(removed.Select(cell => cell.Position), Does.Contain(center + Vector3.right * 2));
            Assert.That(volume.RemoveSphere(center, 2f), Is.Empty);
            Assert.Throws<ArgumentOutOfRangeException>(() => volume.RemoveSphere(center, -1f));
        }

        [Test]
        public void ColliderIsRecookedAndRayPassesThroughRemovedCell()
        {
            Create(new Vector3Int(3, 1, 3));
            for (int z = 0; z < 3; z++)
            for (int x = 0; x < 3; x++) volume.SetCell(x, 0, z, Stone);
            volume.Build();
            var collider = parent.GetComponentInChildren<MeshCollider>();
            var meshBefore = collider.sharedMesh;
            var ray = new Ray(new Vector3(1.5f, 3f, 1.5f), Vector3.down);
            Physics.SyncTransforms();
            Assert.That(collider.Raycast(ray, out _, 5f), Is.True);
            Assert.That(volume.RemoveSphere(new Vector3(1.5f, .5f, 1.5f), 0f).Count, Is.EqualTo(1));
            Physics.SyncTransforms();
            Assert.That(collider.sharedMesh, Is.Not.SameAs(meshBefore));
            Assert.That(collider.Raycast(ray, out _, 5f), Is.False);
            Assert.That(collider.Raycast(new Ray(new Vector3(.5f, 3f, .5f), Vector3.down), out _, 5f), Is.True);
        }

        [Test]
        public void PillarLossDetachesEntireRoofButPreservesOtherSupportedCells()
        {
            Create(new Vector3Int(5, 4, 1));
            for (int y = 0; y < 3; y++) volume.SetCell(1, y, 0, Stone);
            for (int x = 0; x < 4; x++) volume.SetCell(x, 3, 0, Stone);
            volume.SetCell(4, 0, 0, Stone);
            volume.Build();
            volume.RemoveSphere(new Vector3(1.5f, 1.5f, .5f), 0f);
            var groups = volume.DetachUnsupported();
            Assert.That(groups.Count, Is.EqualTo(1));
            Assert.That(groups[0].Count, Is.EqualTo(5)); // Upper pillar and all four roof cells.
            Assert.That(groups[0].Count(c => c.Position.y == 3.5f), Is.EqualTo(4));
            Assert.That(volume.DetachUnsupported(), Is.Empty);
            Assert.That(volume.RemoveSphere(new Vector3(1.5f, .5f, .5f), 0f).Count, Is.EqualTo(1));
            Assert.That(volume.RemoveSphere(new Vector3(4.5f, .5f, .5f), 0f).Count, Is.EqualTo(1));
        }

        [Test]
        public void DiagonalContactDoesNotJoinUnsupportedIslands()
        {
            Create(new Vector3Int(3, 3, 1));
            volume.SetCell(0, 1, 0, Stone);
            volume.SetCell(1, 2, 0, Stone);
            volume.Build();
            var groups = volume.DetachUnsupported();
            Assert.That(groups.Count, Is.EqualTo(2));
            Assert.That(groups.All(group => group.Count == 1), Is.True);
            Assert.That(parent.GetComponentsInChildren<MeshCollider>(), Is.Empty);
        }

        [TestCase(0, 1, 3)]
        [TestCase(1, 2, 4)]
        [TestCase(2, 0, 0)]
        public void AffectedCollapseMatchesFullFloodAfterRemoval(int scenario, int expectedGroups, int expectedCells)
        {
            var size = new Vector3Int(9, 4, 9);
            var cells = new List<Vector3Int>();
            Vector3 center;
            float radius;
            if (scenario == 0)
            {
                // One bridge is attached through the left-hand pillar.
                for (int x = 0; x < 7; x++) cells.Add(new Vector3Int(x, 2, 1));
                cells.Add(new Vector3Int(0, 1, 1)); cells.Add(new Vector3Int(0, 0, 1));
                center = new Vector3(3.5f, 2.5f, 1.5f); radius = 0;
            }
            else if (scenario == 1)
            {
                // Two arms lose their shared central support in one removal.
                cells.Add(new Vector3Int(3, 0, 1)); cells.Add(new Vector3Int(3, 1, 1));
                cells.Add(new Vector3Int(4, 1, 1)); cells.Add(new Vector3Int(5, 1, 1));
                cells.Add(new Vector3Int(3, 1, 2)); cells.Add(new Vector3Int(3, 1, 3));
                center = new Vector3(3.5f, 1.5f, 1.5f); radius = 0;
            }
            else
            {
                // A crater inside solid anchored terrain must not detach its shell.
                for (int z = 0; z < size.z; z++)
                for (int y = 0; y < size.y; y++)
                for (int x = 0; x < size.x; x++) cells.Add(new Vector3Int(x, y, z));
                center = new Vector3(4.5f, 2.5f, 4.5f); radius = 1.01f;
            }
            using (var local = new VoxelVolume(parent.transform, Vector3.zero, size, 1, material))
            using (var full = new VoxelVolume(parent.transform, Vector3.zero, size, 1, material))
            using (var incremental = new VoxelVolume(parent.transform, Vector3.zero, size, 1, material))
            {
                foreach (Vector3Int p in cells)
                {
                    local.SetCell(p.x, p.y, p.z, Stone);
                    full.SetCell(p.x, p.y, p.z, Stone);
                    incremental.SetCell(p.x, p.y, p.z, Stone);
                }
                local.Build(); full.Build(); incremental.Build();
                Assert.That(local.DetachUnsupported(), Is.Empty, "Scenario starts supported.");
                Assert.That(full.DetachUnsupported(), Is.Empty, "Scenario starts supported.");
                Assert.That(incremental.DetachUnsupported(), Is.Empty, "Scenario starts supported.");
                Assert.That(local.RemoveSphere(center, radius).Count, Is.EqualTo(full.RemoveSphere(center, radius).Count));
                incremental.RemoveSphere(center, radius);
                var actual = local.DetachAffectedUnsupported();
                var expected = full.DetachUnsupported();
                var support = incremental.BeginDetachAffectedUnsupported();
                Assert.Throws<InvalidOperationException>(() => incremental.RemoveSphere(center, radius));
                int steps = 0;
                while (!support.Complete && steps++ < 10000) support.Step(1);
                Assert.That(support.Complete, Is.True, "Budgeted support search must finish.");
                Assert.That(actual.Count, Is.EqualTo(expectedGroups));
                Assert.That(actual.Sum(group => group.Count), Is.EqualTo(expectedCells));
                // Compare components as sets: flood and hash iteration order is irrelevant.
                string[] Canonical(List<List<VoxelFragment>> groups) => groups
                    .Select(group => string.Join(";", group.Select(c => c.Position.ToString("R")).OrderBy(s => s)))
                    .OrderBy(s => s).ToArray();
                CollectionAssert.AreEqual(Canonical(expected), Canonical(actual));
                CollectionAssert.AreEqual(Canonical(expected), Canonical(support.Groups));
                Assert.That(local.DetachUnsupported(), Is.Empty, "No affected island remains.");
                Assert.That(local.DetachAffectedUnsupported(), Is.Empty, "The removal frontier is consumed.");
                Assert.That(incremental.DetachUnsupported(), Is.Empty, "Budgeted search leaves no affected island.");
            }
        }

        [Test]
        public void ChunkSeamHidesSharedFacesAndExposesThemAfterRemoval()
        {
            Create(new Vector3Int(17, 1, 1));
            volume.SetCell(15, 0, 0, Stone);
            volume.SetCell(16, 0, 0, Stone);
            volume.Build();
            var filters = parent.GetComponentsInChildren<MeshFilter>();
            Assert.That(filters.Length, Is.EqualTo(2));
            Assert.That(filters.Sum(f => f.sharedMesh.triangles.Length), Is.EqualTo(60));
            foreach (var filter in filters)
            {
                var mesh = filter.sharedMesh;
                var vertices = mesh.vertices;
                var normals = mesh.normals;
                var indices = mesh.triangles;
                for (int i = 0; i < indices.Length; i += 3)
                    Assert.That(Vector3.Dot(Vector3.Cross(vertices[indices[i + 1]] - vertices[indices[i]], vertices[indices[i + 2]] - vertices[indices[i]]), normals[indices[i]]), Is.GreaterThan(0));
            }
            volume.RemoveSphere(new Vector3(16.5f, .5f, .5f), 0);
            filters = parent.GetComponentsInChildren<MeshFilter>();
            Assert.That(filters.Length, Is.EqualTo(1));
            Assert.That(filters[0].sharedMesh.triangles.Length, Is.EqualTo(36));
        }

        [TestCase(.125f)]
        [TestCase(1f / 6f)]
        public void IncrementalSphereMatchesSynchronousResultAndCollidersEachStep(float cell)
        {
            parent.layer = 8;
            var size = new Vector3Int(48, 1, 16);
            var other = new GameObject("Synchronous comparison");
            other.layer = 9;
            var offset = new Vector3(100, 0, 0);
            try
            {
                using (var stepped = new VoxelVolume(parent.transform, Vector3.zero, size, cell, material))
                using (var synchronous = new VoxelVolume(other.transform, offset, size, cell, material))
                {
                    for (int z = 0; z < size.z; z++) for (int x = 0; x < size.x; x++)
                    {
                        stepped.SetCell(x, 0, z, Stone);
                        synchronous.SetCell(x, 0, z, Stone);
                    }
                    stepped.Build(); synchronous.Build();
                    Vector3 center = new Vector3(16 * cell, .5f * cell, 8 * cell);
                    float radius = 3.1f * cell;
                    var expected = synchronous.RemoveSphere(center + offset, radius);
                    var edit = stepped.BeginRemoveSphere(center, radius);
                    Assert.Throws<InvalidOperationException>(() => stepped.RemoveSphere(center, radius));
                    Assert.Throws<InvalidOperationException>(() => stepped.DetachAffectedUnsupported());
                    int steps = 0;
                    while (!edit.Complete)
                    {
                        edit.Step(1); steps++;
                        Physics.SyncTransforms();
                        foreach (var fragment in edit.Removed)
                        {
                            var ray = new Ray(fragment.Position + Vector3.up * 2, Vector3.down);
                            Assert.That(Physics.Raycast(ray, 4, 1 << 8), Is.False, "Every committed hole must already be reflected in collision.");
                        }
                        Assert.That(Physics.Raycast(new Ray(new Vector3(.5f * cell, 2, .5f * cell), Vector3.down), 4, 1 << 8), Is.True);
                    }
                    Assert.That(steps, Is.GreaterThan(1));
                    Assert.That(edit.ScannedChunks, Is.EqualTo(steps));
                    Assert.That(edit.RebuiltChunks, Is.GreaterThan(0));
                    Assert.That(edit.MaximumStepMilliseconds, Is.GreaterThanOrEqualTo(0));
                    var expectedPositions = expected.Select(c => c.Position - offset).ToArray();
                    Assert.That(edit.Removed.Count, Is.EqualTo(expectedPositions.Length));
                    foreach (var p in expectedPositions)
                        Assert.That(edit.Removed.Any(c => (c.Position - p).sqrMagnitude < 1e-8f), Is.True);
                    Assert.That(stepped.DetachAffectedUnsupported(), Is.Empty);
                }
            }
            finally { UnityEngine.Object.DestroyImmediate(other); }
        }
    }
}
