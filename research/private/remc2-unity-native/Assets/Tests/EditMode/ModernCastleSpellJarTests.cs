using System;
using System.Collections.Generic;
using System.Linq;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernCastleSpellJarTests
    {
        private static NativeSim.CampaignEntityPresentation View() => new NativeSim.CampaignEntityPresentation {
            generation = 4, entityIndex = 31, sourceId = 77, renderParameterIndex = 77,
            baseTmap = 59, selectedTmap = 59, logicalHeight = 150, renderMode = 1,
            x = 640, y = 1280, z = 192
        };
        private static NativeSim.CampaignEntitySnapshot Entity() => new NativeSim.CampaignEntitySnapshot {
            active = 1, entityIndex = 31, sourceId = 77, classId = 15, model = 2, action = 7
        };

        [TestCase(false)]
        [TestCase(true)]
        public void VisibleNativeOccupantControlsAdmissionCollectionAndSlotReuse(bool voxel)
        {
            var parent = new GameObject("castle jar test");
            var camera = new GameObject("castle jar camera").AddComponent<Camera>();
            try
            {
                using var presenter = new ModernCastleSpellJar(parent.transform, voxel);
                var view = View(); var entity = Entity();
                var entities = new Dictionary<ushort, NativeSim.CampaignEntitySnapshot> { [31] = entity };
                Assert.That(ModernCastleSpellJar.Matches(entity, view), Is.True);
                var other = entity; other.model = 3;
                Assert.That(ModernCastleSpellJar.Matches(other, view), Is.False, "Castle action on Speed model");
                other = entity; other.action = 6;
                Assert.That(ModernCastleSpellJar.Matches(other, view), Is.False, "holder is not a world pickup");
                other = entity; other.action = 8;
                Assert.That(ModernCastleSpellJar.Matches(other, view), Is.True, "source respawn form");
                var wrong = view; wrong.controlFlags = 1;
                Assert.That(ModernCastleSpellJar.Matches(entity, wrong), Is.False, "hidden source flag");
                wrong = view; wrong.selectedTmap++;
                Assert.That(ModernCastleSpellJar.Matches(entity, wrong), Is.False);
                presenter.Present(entities, new[] { view });
                Assert.That(presenter.Replaces(31), Is.True);
                Assert.That(parent.transform.childCount, Is.EqualTo(1));
                var first = parent.transform.GetChild(0);
                presenter.Render(camera, true);
                Assert.That(first.position, Is.EqualTo(new Vector3(10, 3, 20)));
                Assert.That(first.localScale.x, Is.EqualTo(150f / 64f).Within(.0001f));
                var renderers = first.GetComponentsInChildren<MeshRenderer>();
                Assert.That(renderers.Length, Is.GreaterThan(0));
                var materials = renderers.SelectMany(r => r.sharedMaterials).Select(m => m.name).Distinct().ToArray();
                Assert.That(materials, Does.Contain("Jar silver"));
                Assert.That(materials, Does.Contain("Jar red lid"));
                Assert.That(materials, Does.Contain("Jar dark rim"));
                var bounds = renderers[0].bounds;
                foreach (var renderer in renderers.Skip(1)) bounds.Encapsulate(renderer.bounds);
                Assert.That(bounds.size.x / bounds.size.y, Is.InRange(1.10f, 1.40f));
                Assert.That(bounds.size.y, Is.InRange(2.25f, 2.45f));
                // The collected holder is omitted from the native visible rows.
                presenter.Present(entities, Array.Empty<NativeSim.CampaignEntityPresentation>());
                Assert.That(presenter.Replaces(31), Is.False);
                Assert.That(parent.transform.childCount, Is.Zero);
                presenter.Present(entities, new[] { view });
                first = parent.transform.GetChild(0);
                view.generation++;
                presenter.Present(entities, new[] { view });
                Assert.That(parent.transform.childCount, Is.EqualTo(1));
                Assert.That(parent.transform.GetChild(0), Is.Not.SameAs(first));
                other = entity; other.active = 0; entities[31] = other;
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
