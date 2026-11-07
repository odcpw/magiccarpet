using System.Collections.Generic;
using System.Linq;
using MagicCarpet.Native.Voxels;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class VoxelGoatEditModeTests
    {
        private static ModernCharacterAsset Character() => Resources.Load<ModernCharacterAsset>("ModernCharacters/Goat/Character");
        private static VoxelGoatAsset Asset(int resolution) => Resources.Load<VoxelGoatAsset>("ModernCharacters/Goat/Voxels/Goat" + resolution);

        [Test]
        public void ImportedResolutionsUseApprovedSkinAndSampledColors()
        {
            var character = Character();
            Assert.That(character, Is.Not.Null, "Prepare the approved character first");
            var source = character.model.GetComponentsInChildren<SkinnedMeshRenderer>(true).Single().sharedMesh;
            var counts = new int[3];
            var resolutions = new[] { 24, 40, 64 };
            for (var i = 0; i < resolutions.Length; i++)
            {
                var asset = Asset(resolutions[i]);
                Assert.That(asset, Is.Not.Null, "Run VoxelGoatImport.Prepare first");
                Assert.That(asset.sourceMesh, Is.SameAs(source));
                Assert.That(asset.mesh.vertexCount, Is.EqualTo(asset.cellColors.Length * 24));
                Assert.That(asset.mesh.boneWeights.Length, Is.EqualTo(asset.mesh.vertexCount));
                Assert.That(asset.mesh.bindposes.Length, Is.EqualTo(source.bindposes.Length));
                Assert.That(asset.cellColors.Select(c => ((int)c.r << 16) | (c.g << 8) | c.b).Distinct().Count(), Is.GreaterThan(8));
                Assert.That(asset.resolution, Is.EqualTo(resolutions[i]));
                Assert.That(asset.cellSize, Is.GreaterThan(0));
                counts[i] = asset.cellColors.Length;
            }
            Assert.That(counts[1], Is.GreaterThan(counts[0]));
            Assert.That(counts[2], Is.GreaterThan(counts[1]));
        }

        [Test]
        public void CurrentPoseBecomesWorldSpaceFragmentsAndGaitUsesTravel()
        {
            var source = Character(); var asset = Asset(24);
            Assert.That(source, Is.Not.Null); Assert.That(asset, Is.Not.Null);
            using var goat = new VoxelGoat(source, asset, null, 2.34f);
            goat.Root.transform.position = new Vector3(4, 2, -3);
            goat.Root.transform.rotation = Quaternion.Euler(0, 37, 0);
            goat.Render(0, 0);
            var idle = goat.SnapshotFragments();
            goat.Render(.2f, .2f);
            Assert.That(goat.CurrentClip, Is.EqualTo("Walk"));
            var phase = goat.ClipTime;
            var moving = goat.SnapshotFragments();
            Assert.That(moving.Count, Is.GreaterThan(0).And.LessThanOrEqualTo(asset.cellColors.Length));
            Assert.That(moving[0].Size, Is.EqualTo(2.34f / asset.resolution).Within(.0001f));
            var idleKeys = new HashSet<Vector3Int>(idle.Select(f => GridKey(f.Position, goat.Root.transform.position, f.Size)));
            var movingKeys = new HashSet<Vector3Int>(moving.Select(f => GridKey(f.Position, goat.Root.transform.position, f.Size)));
            Assert.That(movingKeys.Except(idleKeys).Count(), Is.GreaterThan(10),
                "Animated fragments must change with the gait, not use a rest pose");
            AssertPoseGrid(goat, asset, moving);
            Assert.That(VoxelRubble.Partition(moving, 16).Count, Is.EqualTo(16),
                "A rotated animated goat must split into distinct rubble clusters");

            var beforeMove = moving[0].Position;
            goat.Root.transform.position += Vector3.right * 3;
            var translated = goat.SnapshotFragments();
            Assert.That(Vector3.Distance(translated[0].Position - beforeMove, Vector3.right * 3), Is.LessThan(.0001f));
            AssertPoseGrid(goat, asset, translated);
            goat.Render(.2f, 0);
            Assert.That(goat.CurrentClip, Is.EqualTo("Idle"));
            goat.Render(.2f, .2f);
            Assert.That(goat.CurrentClip, Is.EqualTo("Walk"));
            Assert.That(phase, Is.GreaterThan(0));
        }

        [Test]
        public void SnapshotBoundsFollowVisibleGoatAtSandboxScale()
        {
            var source = Character(); var asset = Asset(40);
            Assert.That(source, Is.Not.Null); Assert.That(asset, Is.Not.Null);
            using var goat = new VoxelGoat(source, asset, null, 1.2f);
            goat.Root.transform.SetPositionAndRotation(new Vector3(15, 4, -3), Quaternion.Euler(0, 37, 0));
            goat.Render(.2f, .15f);
            var renderer = goat.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true).Single(r => r.sharedMesh == asset.mesh);
            var fragments = goat.SnapshotFragments();
            Assert.That(fragments, Is.Not.Empty);
            var actual = new Bounds(fragments[0].Position, Vector3.zero);
            foreach (var fragment in fragments) actual.Encapsulate(fragment.Position);
            actual.Expand(fragments[0].Size);
            var visible = renderer.bounds;
            Assert.That(fragments[0].Size, Is.EqualTo(1.2f / 40).Within(.001f));
            Assert.That(actual.size.y, Is.InRange(.6f, 1.8f), "Fragments must be goat-sized, not hundreds of times larger");
            Assert.That(Vector3.Distance(actual.center, visible.center), Is.LessThan(.6f),
                "Fragment cloud must occupy the visible renderer's world position");
            Assert.That(actual.size.x, Is.LessThan(visible.size.x + .3f));
            Assert.That(actual.size.y, Is.LessThan(visible.size.y + .3f));
            Assert.That(actual.size.z, Is.LessThan(visible.size.z + .3f));
            Assert.That(Vector3.Distance(actual.center, goat.Root.transform.position), Is.LessThan(2f));
        }

        private static Vector3Int GridKey(Vector3 position, Vector3 origin, float size)
        {
            var p = (position - origin) / size;
            return new Vector3Int(Mathf.RoundToInt(p.x), Mathf.RoundToInt(p.y), Mathf.RoundToInt(p.z));
        }

        // Independently bake the visible skin. Every original centre maps to a
        // unique returned grid cell within half a cell per axis, and collisions
        // carry the arithmetic mean of all contributing original colors.
        private static void AssertPoseGrid(VoxelGoat goat, VoxelGoatAsset asset, List<VoxelFragment> fragments)
        {
            var renderer = goat.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true).Single(r => r.sharedMesh == asset.mesh);
            var mesh = new Mesh();
            try
            {
                renderer.BakeMesh(mesh, true);
                var vertices = mesh.vertices;
                var origin = goat.Root.transform.position;
                var size = fragments[0].Size;
                var result = new Dictionary<Vector3Int, VoxelFragment>();
                foreach (var fragment in fragments)
                {
                    Assert.That(fragment.Size, Is.EqualTo(size));
                    var key = GridKey(fragment.Position, origin, size);
                    Assert.That(result.ContainsKey(key), Is.False, "Grid cells must not overlap or repeat");
                    result.Add(key, fragment);
                    Assert.That(Vector3.Distance(fragment.Position, origin + (Vector3)key * size), Is.LessThan(size * .0001f));
                }
                var expected = new Dictionary<Vector3Int, List<Color32>>();
                for (var i = 0; i < asset.cellColors.Length; i++)
                {
                    var center = Vector3.zero;
                    for (var j = 0; j < 24; j++) center += vertices[i * 24 + j];
                    var world = renderer.transform.TransformPoint(center / 24);
                    var key = GridKey(world, origin, size);
                    Assert.That(result.TryGetValue(key, out var fragment), Is.True, "Every baked centre has a fragment");
                    var delta = world - fragment.Position;
                    Assert.That(Mathf.Abs(delta.x), Is.LessThanOrEqualTo(size * .5001f));
                    Assert.That(Mathf.Abs(delta.y), Is.LessThanOrEqualTo(size * .5001f));
                    Assert.That(Mathf.Abs(delta.z), Is.LessThanOrEqualTo(size * .5001f));
                    if (!expected.TryGetValue(key, out var colors)) expected.Add(key, colors = new List<Color32>());
                    colors.Add(asset.cellColors[i]);
                }
                Assert.That(result.Count, Is.EqualTo(expected.Count));
                foreach (var pair in expected)
                {
                    var colors = pair.Value;
                    var expectedColor = new Color32((byte)(colors.Sum(c => (long)c.r) / colors.Count),
                        (byte)(colors.Sum(c => (long)c.g) / colors.Count), (byte)(colors.Sum(c => (long)c.b) / colors.Count),
                        (byte)(colors.Sum(c => (long)c.a) / colors.Count));
                    Assert.That(result[pair.Key].Color, Is.EqualTo(expectedColor));
                }
            }
            finally { Object.DestroyImmediate(mesh); }
        }
    }
}
