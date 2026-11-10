using System;
using System.Collections.Generic;
using System.Linq;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernExperienceScrollTests
    {
        private static NativeSim.CampaignEntityPresentation View() => new NativeSim.CampaignEntityPresentation {
            generation = 4, entityIndex = 31, sourceId = 77, renderParameterIndex = 280,
            baseTmap = 259, selectedTmap = 259, logicalHeight = 75, renderMode = 1,
            x = 640, y = 1280, z = 192
        };
        private static NativeSim.CampaignEntitySnapshot Entity() => new NativeSim.CampaignEntitySnapshot {
            active = 1, entityIndex = 31, sourceId = 77, classId = 14, model = 5, action = 10
        };

        [TestCase(false)]
        [TestCase(true)]
        public void SourceOccupantControlsAdmissionRemovalAndSlotReuse(bool voxel)
        {
            var parent = new GameObject("scroll test");
            var camera = new GameObject("scroll camera").AddComponent<Camera>();
            try
            {
                using var presenter = new ModernExperienceScroll(parent.transform, voxel);
                var view = View(); var entity = Entity();
                var entities = new Dictionary<ushort, NativeSim.CampaignEntitySnapshot> { [31] = entity };
                Assert.That(ModernExperienceScroll.Matches(entity, view), Is.True);
                var other = entity; other.model = 6;
                Assert.That(ModernExperienceScroll.Matches(other, view), Is.False);
                other = entity; other.active = 0;
                Assert.That(ModernExperienceScroll.Matches(other, view), Is.False);
                var wrong = view; wrong.selectedTmap++;
                Assert.That(ModernExperienceScroll.Matches(entity, wrong), Is.False);
                wrong = view; wrong.logicalHeight++;
                Assert.That(ModernExperienceScroll.Matches(entity, wrong), Is.False);
                presenter.Present(entities, new[] { view });
                Assert.That(presenter.Replaces(31), Is.True);
                Assert.That(parent.transform.childCount, Is.EqualTo(1));
                var first = parent.transform.GetChild(0);
                presenter.Render(camera, true);
                Assert.That(first.position, Is.EqualTo(new Vector3(10, 3, 20)));
                Assert.That(first.localScale.x, Is.EqualTo(75f / 64f).Within(.0001f));
                var renderers = first.GetComponentsInChildren<MeshRenderer>();
                Assert.That(renderers.Length, Is.GreaterThan(0));
                var materials = renderers.SelectMany(r => r.sharedMaterials).Select(m => m.name).Distinct().ToArray();
                Assert.That(materials, Does.Contain("Scroll parchment"));
                Assert.That(materials, Does.Contain("Scroll violet binding"));
                var bounds = renderers[0].bounds;
                foreach (var renderer in renderers.Skip(1)) bounds.Encapsulate(renderer.bounds);
                Assert.That(bounds.size.x / bounds.size.y, Is.InRange(2.3f, 3.2f));
                Assert.That(bounds.size.y, Is.InRange(.8f, 1.4f));
                // The next native presentation omits the collected occupant.
                presenter.Present(entities, Array.Empty<NativeSim.CampaignEntityPresentation>());
                Assert.That(presenter.Replaces(31), Is.False);
                Assert.That(parent.transform.childCount, Is.Zero);
                presenter.Present(entities, new[] { view });
                first = parent.transform.GetChild(0);
                view.sourceId++; entity.sourceId++; entities[31] = entity;
                presenter.Present(entities, new[] { view });
                Assert.That(parent.transform.childCount, Is.EqualTo(1));
                Assert.That(parent.transform.GetChild(0), Is.Not.SameAs(first));
                entities[31] = other;
                presenter.Present(entities, new[] { view });
                Assert.That(presenter.Replaces(31), Is.False);
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
