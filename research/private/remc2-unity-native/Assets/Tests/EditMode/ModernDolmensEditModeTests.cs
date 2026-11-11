using System.Collections.Generic;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernDolmensEditModeTests
    {
        [TestCase(false)]
        [TestCase(true)]
        public void RenderedDolmenPreservesImportedShapeAndSourceHeight(bool voxel)
        {
            var parent = new GameObject("dolmen test");
            var camera = new GameObject("dolmen camera").AddComponent<Camera>();
            try
            {
                using var presenter = new ModernDolmens(parent.transform, voxel, (_, _) => 2f);
                var entity = new NativeSim.CampaignEntitySnapshot {
                    entityIndex = 130, sourceId = 130, active = 1, classId = 2, model = 2, action = 6
                };
                var view = new NativeSim.CampaignEntityPresentation {
                    entityIndex = 130, sourceId = 130, renderParameterIndex = 39,
                    baseTmap = 79, selectedTmap = 79, logicalHeight = 400, x = 640, y = 1280
                };
                var entities = new Dictionary<ushort, NativeSim.CampaignEntitySnapshot> { [130] = entity };
                presenter.Observe(1, 0, entities, new[] { view });
                presenter.Render(camera, true);
                var collection = parent.transform.GetChild(0);
                var renderers = collection.GetComponentsInChildren<MeshRenderer>();
                Assert.That(renderers.Length, Is.GreaterThan(0));
                var bounds = renderers[0].bounds;
                foreach (var renderer in renderers) bounds.Encapsulate(renderer.bounds);
                Assert.That(bounds.min.y, Is.EqualTo(2f).Within(.2f));
                Assert.That(bounds.size.y, Is.EqualTo(400f / 64f).Within(.3f));
                Assert.That(bounds.size.x / bounds.size.y, Is.InRange(1.45f, 2.0f));
                Assert.That(bounds.center.x, Is.EqualTo(10f).Within(1f));
                Assert.That(bounds.center.z, Is.EqualTo(20f).Within(1f));
                Assert.That(presenter.Replaces(130), Is.True);
                presenter.Observe(1, 1, entities, System.Array.Empty<NativeSim.CampaignEntityPresentation>());
                Assert.That(presenter.Replaces(130), Is.False);
                Assert.That(collection.childCount, Is.Zero, "a removed native row must not leave a model");
                presenter.Observe(1, 2, entities, new[] { view });
                Assert.That(presenter.Replaces(130), Is.True);
                entity.model = 1; entities[130] = entity;
                presenter.Observe(1, 3, entities, new[] { view });
                Assert.That(presenter.Replaces(130), Is.False, "a recycled slot must not remain a Dolmen");
                Assert.That(collection.childCount, Is.Zero);
                entity.model = 2; entities[130] = entity;
                presenter.Observe(2, 0, entities, new[] { view });
                Assert.That(presenter.Replaces(130), Is.True, "reset generation can admit the new row");
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(camera.gameObject);
                UnityEngine.Object.DestroyImmediate(parent);
            }
        }

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
            other = view; other.controlFlags = 0x400;
            Assert.That(ModernDolmens.Matches(entity, other), Is.False, "source drawing disable bit");
            other = view; other.controlFlags = 0x20;
            Assert.That(ModernDolmens.Matches(entity, other), Is.False, "source no-draw bit");
        }

        [Test]
        public void VisualGroundingUsesTerrainAndBoundsSteepSlopePenetration()
        {
            Assert.That(ModernDolmens.VisualBaseHeight(10, 20, 4, (_, _) => 42), Is.EqualTo(42));
            var baseY = ModernDolmens.VisualBaseHeight(0, 0, 4, (x, _) => 10 + x);
            // Right support's highest footprint is x=.58*4. Grounding must not
            // bury it more than one quarter of the model's height.
            Assert.That(baseY, Is.EqualTo(11.32f).Within(.001f));
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
