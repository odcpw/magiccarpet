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
