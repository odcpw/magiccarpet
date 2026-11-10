using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernDolmensEditModeTests
    {
        [Test]
        public void AdmissionRequiresCurrentDolmenCarrierAndSourceIdentity()
        {
            var entity = new NativeSim.CampaignEntitySnapshot {
                entityIndex = 130, sourceId = 130, active = 1, classId = 2, model = 2, action = 6
            };
            var view = new NativeSim.CampaignEntityPresentation {
                entityIndex = 130, sourceId = 130, renderParameterIndex = 39,
                baseTmap = 79, selectedTmap = 79, logicalHeight = 400
            };
            Assert.That(ModernDolmens.Matches(entity, view), Is.True);
            var recycled = entity; recycled.sourceId++;
            Assert.That(ModernDolmens.Matches(recycled, view), Is.False);
            recycled = entity; recycled.model = 1;
            Assert.That(ModernDolmens.Matches(recycled, view), Is.False);
            recycled = entity; recycled.active = 0;
            Assert.That(ModernDolmens.Matches(recycled, view), Is.False);
            var other = view; other.selectedTmap = 78;
            Assert.That(ModernDolmens.Matches(entity, other), Is.False);
        }

        [TestCase("ModernScenery/Dolmen")]
        [TestCase("ModernScenery/DolmenVoxel")]
        public void ImportedModelHasGroundedWideSourceShapeAndBoundedGeometry(string resource)
        {
            var prefab = Resources.Load<GameObject>(resource);
            Assert.That(prefab, Is.Not.Null, resource);
            var instance = Object.Instantiate(prefab);
            try
            {
                var renderers = instance.GetComponentsInChildren<MeshRenderer>();
                Assert.That(renderers.Length, Is.GreaterThan(0));
                var bounds = renderers[0].bounds;
                foreach (var renderer in renderers) bounds.Encapsulate(renderer.bounds);
                Assert.That(bounds.min.y, Is.EqualTo(0).Within(.03));
                Assert.That(bounds.size.y, Is.InRange(.9f, 1.1f));
                Assert.That(bounds.size.x / bounds.size.y, Is.InRange(1.45f, 2.0f));
                Assert.That(bounds.size.z, Is.InRange(.45f, 1.0f));
                var triangles = 0;
                foreach (var filter in instance.GetComponentsInChildren<MeshFilter>())
                    triangles += filter.sharedMesh.triangles.Length / 3;
                Assert.That(triangles, Is.InRange(100, 15000));
            }
            finally { Object.DestroyImmediate(instance); }
        }
    }
}
