using System.Collections.Generic;
using MagicCarpet.Native.Voxels;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class VoxelPhysicsEditModeTests
    {
        [Test] public void PartitionPreservesEveryCellWithinTheBodyBudget()
        {
            var cells = new List<VoxelFragment>();
            for (int i = 0; i < 1003; i++) cells.Add(new VoxelFragment(new Vector3(i, 0, 0), .25f, new Color32(1, 2, 3, 255)));
            var groups = VoxelRubble.Partition(cells, VoxelRubble.MaxClustersPerEmission);
            Assert.That(groups.Count, Is.EqualTo(VoxelRubble.MaxClustersPerEmission));
            var seen = new HashSet<float>();
            foreach (var group in groups) foreach (var cell in group) Assert.That(seen.Add(cell.Position.x), Is.True);
            Assert.That(seen.Count, Is.EqualTo(cells.Count));
            Assert.That(VoxelRubble.MaxClustersPerEmission, Is.LessThanOrEqualTo(VoxelRubble.MaxBodies));
        }

        [Test] public void SphereMassAndEnergyAgreeWithItsFragmentVolume()
        {
            var cells = VoxelFireball.SphereCells(.8f, .1f);
            float mass = VoxelRubble.Mass(cells), split = 0f;
            foreach (var group in VoxelRubble.Partition(cells, 32)) split += VoxelRubble.Mass(group);
            Assert.That(split, Is.EqualTo(mass).Within(mass * .0001f));
            var velocity = new Vector3(3, 4, 0);
            Assert.That(VoxelFireball.ImpactEnergy(mass, velocity), Is.EqualTo(12.5f * mass).Within(.0001f));
            Assert.That(cells.Count, Is.GreaterThan(100));
            foreach (var cell in cells) Assert.That(cell.Position.magnitude, Is.LessThanOrEqualTo(.80001f));
        }

        [Test] public void FineSphereIsBoundedAndMeshRetainsAllColorsAndCells()
        {
            var cells = VoxelFireball.SphereCells(1f, .0001f);
            Assert.That(cells.Count, Is.LessThan(5000));
            Assert.That(cells[0].Size, Is.EqualTo(.125f).Within(.00001f));
            var mesh = VoxelRubble.BuildMesh(cells, Vector3.zero);
            try
            {
                Assert.That(mesh.vertexCount, Is.EqualTo(cells.Count * 24));
                Assert.That(mesh.triangles.Length, Is.EqualTo(cells.Count * 36));
                Assert.That(mesh.colors32[0], Is.EqualTo(cells[0].Color));
            }
            finally { Object.DestroyImmediate(mesh); }
        }

        [Test] public void RubbleRemainsVisibleBeforeSleepAndReportsHardLifetimeFallback()
        {
            Assert.That(VoxelRubble.ShouldBeginFade(2.99f, true), Is.False);
            Assert.That(VoxelRubble.ShouldBeginFade(3f, true), Is.True);
            Assert.That(VoxelRubble.ShouldBeginFade(44.99f, false), Is.False);
            Assert.That(VoxelRubble.ShouldBeginFade(45f, false), Is.True);
            Assert.That(VoxelRubble.FadeSeconds, Is.EqualTo(2f));
        }

        [Test] public void ClusterBoxesDoNotOverlapAndClearTheImpactPlane()
        {
            var cells = VoxelFireball.SphereCells(.8f, .1f);
            var groups = VoxelRubble.Partition(cells, 32);
            Assert.That(groups.Count, Is.GreaterThan(1));
            var bounds = new List<Bounds>();
            foreach (var group in groups)
            {
                var b = new Bounds(group[0].Position, Vector3.one * group[0].Size);
                foreach (var c in group) b.Encapsulate(new Bounds(c.Position, Vector3.one * c.Size));
                bounds.Add(b);
            }
            for (int i = 0; i < bounds.Count; i++) for (int j = i + 1; j < bounds.Count; j++)
            {
                Vector3 overlap = Vector3.Min(bounds[i].max, bounds[j].max) - Vector3.Max(bounds[i].min, bounds[j].min);
                Assert.That(Mathf.Min(overlap.x, Mathf.Min(overlap.y, overlap.z)), Is.LessThan(.0001f));
            }
            Vector3 normal = new Vector3(1, 1, 0).normalized;
            Vector3 point = new Vector3(.5f, 0, 0);
            Vector3 shift = VoxelRubble.ImpactClearance(groups, point, normal);
            foreach (var b in bounds)
            {
                float clearance = Vector3.Dot(b.center + shift - point, normal) -
                    (Mathf.Abs(normal.x) * b.extents.x + Mathf.Abs(normal.y) * b.extents.y + Mathf.Abs(normal.z) * b.extents.z);
                Assert.That(clearance, Is.GreaterThanOrEqualTo(.0049f));
            }
        }

        [Test] public void CachedPartitionsMatchCompactReferenceSelectionAndOrdering()
        {
            var cells = VoxelFireball.SphereCells(.8f, .1f);
            for (int count = 1; count <= 32; count *= 2)
            {
                var expected = ReferencePartition(cells, count);
                var actual = VoxelRubble.Partition(cells, count);
                Assert.That(actual.Count, Is.EqualTo(expected.Count));
                for (int i = 0; i < actual.Count; i++)
                {
                    Assert.That(actual[i].Count, Is.EqualTo(expected[i].Count));
                    for (int j = 0; j < actual[i].Count; j++)
                        Assert.That(actual[i][j].Position, Is.EqualTo(expected[i][j].Position));
                }
            }
        }

        [Test] public void GridFastPathMatchesCompactReferenceAndIrregularCellsFallBack()
        {
            var random = new System.Random(12345);
            var cells = new List<VoxelFragment>();
            for (int i = 0; i < 1600; i++)
            {
                var position = new Vector3(70.13f, -12.3f, 91.22f) +
                    new Vector3(random.Next(-24, 25), random.Next(-12, 13), random.Next(-15, 16)) * .125f;
                cells.Add(new VoxelFragment(position, .125f, new Color32(1, 2, 3, 255)));
            }
            Assert.That(VoxelRubble.TryFindGridSplit(cells, out _, out _), Is.True);
            AssertPartitionsEqual(cells, 32);

            var irregular = new List<VoxelFragment>(cells);
            var first = irregular[0];
            irregular[0] = new VoxelFragment(first.Position + new Vector3(.043f, 0, 0), first.Size, first.Color);
            Assert.That(VoxelRubble.TryFindGridSplit(irregular, out _, out _), Is.False);
            AssertPartitionsEqual(irregular, 16);

            irregular[0] = new VoxelFragment(first.Position, first.Size * 1.1f, first.Color);
            Assert.That(VoxelRubble.TryFindGridSplit(irregular, out _, out _), Is.False);
            AssertPartitionsEqual(irregular, 16);
        }

        private static void AssertPartitionsEqual(List<VoxelFragment> cells, int count)
        {
            var expected = ReferencePartition(cells, count);
            var actual = VoxelRubble.Partition(cells, count);
            Assert.That(actual.Count, Is.EqualTo(expected.Count));
            for (int i = 0; i < actual.Count; i++)
            {
                Assert.That(actual[i].Count, Is.EqualTo(expected[i].Count));
                for (int j = 0; j < actual[i].Count; j++)
                    Assert.That(actual[i][j].Position, Is.EqualTo(expected[i][j].Position));
            }
        }

        [Test] public void SolidCubePartitionsIntoCompactNonoverlappingBlocks()
        {
            var cells = new List<VoxelFragment>();
            for (int x = 0; x < 8; x++) for (int y = 0; y < 8; y++) for (int z = 0; z < 8; z++)
                cells.Add(new VoxelFragment(new Vector3(x, y, z), 1f, new Color32(1, 2, 3, 255)));
            var groups = VoxelRubble.Partition(cells, 32);
            Assert.That(groups.Count, Is.EqualTo(32));
            var seen = new HashSet<Vector3>();
            var bounds = new List<Bounds>();
            foreach (var group in groups)
            {
                var b = new Bounds(group[0].Position, Vector3.one);
                foreach (var cell in group) { Assert.That(seen.Add(cell.Position), Is.True); b.Encapsulate(new Bounds(cell.Position, Vector3.one)); }
                float longest = Mathf.Max(b.size.x, Mathf.Max(b.size.y, b.size.z));
                float shortest = Mathf.Min(b.size.x, Mathf.Min(b.size.y, b.size.z));
                Assert.That(longest / shortest, Is.LessThanOrEqualTo(2f));
                bounds.Add(b);
            }
            Assert.That(seen.Count, Is.EqualTo(512));
            for (int i = 0; i < bounds.Count; i++) for (int j = i + 1; j < bounds.Count; j++)
            {
                Vector3 overlap = Vector3.Min(bounds[i].max, bounds[j].max) - Vector3.Max(bounds[i].min, bounds[j].min);
                Assert.That(Mathf.Min(overlap.x, Mathf.Min(overlap.y, overlap.z)), Is.LessThan(.0001f));
            }
        }

        [Test] public void BlockedLongestAxisFallsBackToNextLongestSplittableAxis()
        {
            var cells = new List<VoxelFragment>();
            for (int x = 0; x <= 6; x++) for (int y = 0; y < 2; y++)
                cells.Add(new VoxelFragment(new Vector3(x * .4f, y * 2f, 0), 1f, new Color32(1, 2, 3, 255)));
            Assert.That(VoxelRubble.TryFindGridSplit(cells, out _, out _), Is.False);
            var groups = VoxelRubble.Partition(cells, 2);
            Assert.That(groups.Count, Is.EqualTo(2));
            Assert.That(groups[0].Count, Is.EqualTo(7));
            Assert.That(groups[1].Count, Is.EqualTo(7));
            foreach (var c in groups[0]) Assert.That(c.Position.y, Is.EqualTo(0));
            foreach (var c in groups[1]) Assert.That(c.Position.y, Is.EqualTo(2));
            AssertPartitionsEqual(cells, 2);
        }

        // Independent sort-based oracle for the longest-splittable-axis objective.
        private static List<List<VoxelFragment>> ReferencePartition(List<VoxelFragment> cells, int count)
        {
            var result = new List<List<VoxelFragment>> { new List<VoxelFragment>(cells) };
            while (result.Count < count)
            {
                int selected = -1, chosenAxis = -1, chosenMid = 0, selectedSize = 0;
                for (int g = 0; g < result.Count; g++)
                {
                    var group = result[g];
                    if (group.Count <= selectedSize || group.Count < 2) continue;
                    int bestAxis = -1, bestMid = 0;
                    float bestSpan = float.NegativeInfinity;
                    Vector3 minCorner = Vector3.one * float.PositiveInfinity, maxCorner = Vector3.one * float.NegativeInfinity;
                    foreach (var c in group)
                    {
                        minCorner = Vector3.Min(minCorner, c.Position - Vector3.one * (c.Size * .5f));
                        maxCorner = Vector3.Max(maxCorner, c.Position + Vector3.one * (c.Size * .5f));
                    }
                    Vector3 shapeSize = maxCorner - minCorner;
                    var sorted = new List<VoxelFragment>(group);
                    var rightMin = new float[group.Count];
                    for (int axis = 0; axis < 3; axis++)
                    {
                        int axisMid = 0, axisBalance = int.MaxValue;
                        int a = axis;
                        sorted.Sort((x, y) => x.Position[a].CompareTo(y.Position[a]));
                        float min = float.PositiveInfinity;
                        for (int i = sorted.Count - 1; i >= 0; i--)
                        { min = Mathf.Min(min, sorted[i].Position[a] - sorted[i].Size * .5f); rightMin[i] = min; }
                        float max = float.NegativeInfinity;
                        for (int i = 1; i < sorted.Count; i++)
                        {
                            var previous = sorted[i - 1];
                            max = Mathf.Max(max, previous.Position[a] + previous.Size * .5f);
                            float tolerance = Mathf.Min(previous.Size, sorted[i].Size) * .0001f;
                            if (max > rightMin[i] + tolerance) continue;
                            int balance = Mathf.Abs(sorted.Count - 2 * i);
                            if (balance < axisBalance) { axisBalance = balance; axisMid = i; }
                        }
                        if (axisMid > 0 && shapeSize[axis] > bestSpan)
                        { bestSpan = shapeSize[axis]; bestAxis = axis; bestMid = axisMid; }
                    }
                    if (bestAxis >= 0)
                    { selected = g; chosenAxis = bestAxis; chosenMid = bestMid; selectedSize = group.Count; }
                }
                if (selected < 0) break;
                var part = result[selected];
                part.Sort((a, b) => a.Position[chosenAxis].CompareTo(b.Position[chosenAxis]));
                var second = part.GetRange(chosenMid, part.Count - chosenMid);
                part.RemoveRange(chosenMid, part.Count - chosenMid);
                result.Add(second);
            }
            return result;
        }
    }
}
