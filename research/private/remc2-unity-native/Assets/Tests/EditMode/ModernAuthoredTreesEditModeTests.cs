using System.Runtime.InteropServices;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernAuthoredTreesEditModeTests
    {
        private static ModernAuthoredTrees.TreeObservation Row(ushort id, byte action = 0, ushort parameter = 83) => new()
        {
            entityIndex = id, sourceId = id, classId = 2, model = 0, action = action,
            renderParameter = parameter, renderHeight = 400, drawing = 1,
            x = (ushort)(id * 64), y = 128, z = 64
        };

        [Test]
        public void ExactNativeFamilyAndLifecycleAreAdmitted()
        {
            Assert.That(Marshal.SizeOf<ModernAuthoredTrees.TreeObservation>(), Is.EqualTo(56));
            Assert.That(ModernAuthoredTrees.IsTree(Row(1, 0, 83)), Is.True);
            Assert.That(ModernAuthoredTrees.IsTree(Row(1, 1, 84)), Is.True);
            Assert.That(ModernAuthoredTrees.IsTree(Row(1, 2, 226)), Is.True);
            Assert.That(ModernAuthoredTrees.IsTree(Row(1, 2, 227)), Is.True);
            Assert.That(ModernAuthoredTrees.IsTree(Row(1, 2, 83)), Is.False);
            Assert.That(ModernAuthoredTrees.IsTree(Row(1, 0, 226)), Is.False);
            var wrong = Row(1); wrong.classId = 5;
            Assert.That(ModernAuthoredTrees.IsTree(wrong), Is.False);
        }

        [Test]
        public void BothStylesFollowLiveBurnedHiddenAndResetRows()
        {
            var root = new GameObject("Tree test root");
            var cameraObject = new GameObject("Tree test camera");
            var camera = cameraObject.AddComponent<Camera>();
            ModernAuthoredTrees trees = null;
            try
            {
                trees = new ModernAuthoredTrees(root.transform);
                trees.BeginFrame();
                for (ushort i = 1; i <= 18; i++)
                    Assert.That(trees.Observe(Row(i, 0, i % 2 == 0 ? (ushort)84 : (ushort)83), false), Is.True);
                trees.EndFrame(); trees.Render(camera, true);
                Assert.That(root.transform.childCount, Is.EqualTo(18));
                var first = root.transform.GetChild(0);
                Assert.That(first.position, Is.EqualTo(new Vector3(1, 1, 2)));
                var sourceTrunk = Resources.Load<Mesh>("ModernNature/tree_trunk_mid");
                var sourceLeaves = Resources.Load<Mesh>("ModernNature/tree_leaves_mid");
                var trunk = first.GetChild(0).GetComponent<MeshFilter>().sharedMesh;
                var nearLeaves = first.GetChild(2).GetComponent<MeshFilter>().sharedMesh;
                var farLeaves = first.GetChild(5).GetComponent<MeshFilter>().sharedMesh;
                var originalVertices = sourceTrunk.vertices;
                var trunkVertices = trunk.vertices;
                var originalRoot = Vector3.zero;
                var thickRoot = Vector3.zero;
                var rootCount = 0;
                for (int i = 0; i < originalVertices.Length; i++)
                {
                    if (originalVertices[i].y >= sourceTrunk.bounds.min.y + .2f) continue;
                    originalRoot += new Vector3(originalVertices[i].x, 0f, originalVertices[i].z);
                    thickRoot += new Vector3(trunkVertices[i].x, 0f, trunkVertices[i].z);
                    rootCount++;
                }
                Assert.That(rootCount, Is.GreaterThan(0));
                originalRoot /= rootCount;
                thickRoot /= rootCount;
                Assert.That(thickRoot.magnitude, Is.LessThan(.0001f), "the bole contacts the native anchor");
                for (int i = 0; i < originalVertices.Length; i++)
                {
                    if (originalVertices[i].y >= sourceTrunk.bounds.min.y + .2f) continue;
                    var relative = new Vector3(originalVertices[i].x, 0f, originalVertices[i].z) - originalRoot;
                    Assert.That(Vector3.Distance(new Vector3(trunkVertices[i].x, 0f, trunkVertices[i].z), relative * 3.2f),
                        Is.LessThan(.0001f));
                }
                Assert.That(nearLeaves.GetIndexCount(0), Is.EqualTo(sourceLeaves.GetIndexCount(0) * 10));
                Assert.That(farLeaves.GetIndexCount(0), Is.EqualTo(sourceLeaves.GetIndexCount(0) * 2));
                var sourceColor = sourceLeaves.colors[0];
                var leafColor = nearLeaves.colors[0];
                Assert.That(leafColor.r, Is.EqualTo(sourceColor.r * .5f).Within(.0001f));
                Assert.That(leafColor.g, Is.EqualTo(sourceColor.g * .95f).Within(.0001f));
                Assert.That(leafColor.b, Is.EqualTo(sourceColor.b * 1.4f).Within(.0001f));
                Assert.That(first.GetChild(2).GetComponent<MeshRenderer>().sharedMaterial.GetFloat("_Cutoff"), Is.EqualTo(.15f).Within(.0001f));
                Assert.That(root.transform.GetChild(1).GetChild(2).GetComponent<MeshFilter>().sharedMesh,
                    Is.SameAs(nearLeaves), "all authored trees share one canopy mesh");
                Assert.That(trees.Replaces(1, 1), Is.True);
                Assert.That(trees.Replaces(1, 2), Is.False);
                Assert.That(first.GetChild(2).GetComponent<MeshRenderer>().enabled, Is.True);
                trees.BeginFrame();
                Assert.That(trees.Observe(Row(1, 2, 226), true), Is.True);
                var hidden = Row(2); hidden.drawing = 0;
                Assert.That(trees.Observe(hidden, true), Is.True);
                trees.EndFrame();
                Assert.That(root.transform.childCount, Is.EqualTo(2));
                Assert.That(first.GetChild(2).GetComponent<MeshRenderer>().enabled, Is.False);
                Assert.That(first.GetChild(0).GetComponent<MeshFilter>().sharedMesh.name, Does.Contain("voxel"));
                Assert.That(first.GetChild(2).GetComponent<MeshFilter>().sharedMesh.GetIndexCount(0),
                    Is.GreaterThan(first.GetChild(5).GetComponent<MeshFilter>().sharedMesh.GetIndexCount(0)),
                    "the voxel canopy has a smaller distant mesh");
                var voxelNear = first.GetChild(2).GetComponent<MeshFilter>().sharedMesh;
                var voxelFar = first.GetChild(5).GetComponent<MeshFilter>().sharedMesh;
                if (SystemInfo.graphicsDeviceType == UnityEngine.Rendering.GraphicsDeviceType.Null)
                    Assert.That(voxelNear.colors32[0], Is.EqualTo(new Color32(32, 100, 76, 255)));
                Assert.That(voxelNear.GetIndexCount(0) / 3, Is.LessThanOrEqualTo(80_000),
                    "the shared near voxel canopy remains bounded");
                TestContext.WriteLine($"Tree canopy triangles: Modern near={nearLeaves.GetIndexCount(0) / 3}, far={farLeaves.GetIndexCount(0) / 3}; " +
                    $"Voxel near={voxelNear.GetIndexCount(0) / 3}, far={voxelFar.GetIndexCount(0) / 3} (null graphics device)");
                Assert.That(root.transform.GetChild(1).gameObject.activeSelf, Is.False);
                Assert.That(trees.Replaces(2, 2), Is.False);
                trees.BeginFrame();
                trees.Observe(Row(1), false);
                trees.EndFrame();
                Assert.That(root.transform.childCount, Is.EqualTo(1));
                Assert.That(first.GetChild(2).GetComponent<MeshRenderer>().enabled, Is.True);
                Assert.That(first.GetChild(0).GetComponent<MeshFilter>().sharedMesh.name, Does.Not.Contain("voxel"));
            }
            finally { trees?.Dispose(); Object.DestroyImmediate(root); Object.DestroyImmediate(cameraObject); }
        }
    }
}
