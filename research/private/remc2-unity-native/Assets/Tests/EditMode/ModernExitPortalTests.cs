using System;
using System.Linq;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernExitPortalTests
    {
        private static NativeSim.CampaignEntityPresentation Row(ushort slot = 31) =>
            new NativeSim.CampaignEntityPresentation {
                generation = 4, entityIndex = slot, sourceId = 77,
                renderParameterIndex = 338, baseTmap = 461, selectedTmap = 461,
                logicalHeight = 700, x = 640, y = 1280, z = 192
            };

        [TestCase(false)]
        [TestCase(true)]
        public void SourceRowsControlAdmissionRemovalAndReuse(bool voxel)
        {
            var parent = new GameObject("portal test");
            var camera = new GameObject("portal camera").AddComponent<Camera>();
            try
            {
                using var portal = new ModernExitPortal(parent.transform, voxel);
                var row = Row();
                Assert.That(ModernExitPortal.Matches(row), Is.True);
                var other = row; other.renderParameterIndex++;
                Assert.That(ModernExitPortal.Matches(other), Is.False);
                portal.Present(new[] { other });
                Assert.That(portal.Replaces(row.entityIndex), Is.False);
                Assert.That(parent.transform.childCount, Is.Zero);
                portal.Present(new[] { row });
                Assert.That(portal.Replaces(row.entityIndex), Is.True);
                Assert.That(parent.transform.childCount, Is.EqualTo(1));
                var first = parent.transform.GetChild(0);
                portal.Render(camera, true);
                Assert.That(first.position, Is.EqualTo(new Vector3(10, 3, 20)));
                Assert.That(first.localScale.x, Is.EqualTo(700f / 64f).Within(.0001f));
                Assert.That(first.GetComponentsInChildren<MeshRenderer>().Length, Is.GreaterThan(0));
                Assert.That(first.GetComponentsInChildren<MeshFilter>().Sum(x => x.sharedMesh.triangles.Length / 3),
                    Is.LessThan(voxel ? 100000 : 15000));
                row.sourceId++;
                portal.Present(new[] { row });
                Assert.That(parent.transform.childCount, Is.EqualTo(1));
                Assert.That(parent.transform.GetChild(0), Is.Not.SameAs(first));
                row.generation++;
                portal.Present(new[] { row });
                Assert.That(parent.transform.childCount, Is.EqualTo(1));
                portal.Present(Array.Empty<NativeSim.CampaignEntityPresentation>());
                Assert.That(portal.Replaces(row.entityIndex), Is.False);
                Assert.That(parent.transform.childCount, Is.Zero);
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(camera.gameObject);
                UnityEngine.Object.DestroyImmediate(parent);
            }
        }
    }
}
