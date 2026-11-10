using NUnit.Framework;
using System.Collections.Generic;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernStoneStatueEditModeTests
    {
        [Test]
        public void OnlyNightBankUsesTheStatue()
        {
            Assert.That(ModernStoneStatues.SupportsBank("TMAPS1-0"), Is.True);
            Assert.That(ModernStoneStatues.SupportsBank("TMAPS0-0"), Is.False);
            Assert.That(ModernStoneStatues.SupportsBank(null), Is.False);
        }

        [Test]
        public void AdmissionRequiresLiveStoneAndMatchingSourceIdentity()
        {
            var entity = new NativeSim.CampaignEntitySnapshot {
                entityIndex = 130, sourceId = 130, active = 1, classId = 2, model = 1, action = 3
            };
            var view = new NativeSim.CampaignEntityPresentation {
                entityIndex = 130, sourceId = 130, renderParameterIndex = 79,
                baseTmap = 78, selectedTmap = 78, logicalHeight = 500
            };
            Assert.That(ModernStoneStatues.Matches(entity, view), Is.True);
            var other = entity; other.sourceId++;
            Assert.That(ModernStoneStatues.Matches(other, view), Is.False);
            other = entity; other.model = 2;
            Assert.That(ModernStoneStatues.Matches(other, view), Is.False);
            other = entity; other.active = 0;
            Assert.That(ModernStoneStatues.Matches(other, view), Is.False);
            var otherView = view; otherView.selectedTmap = 79;
            Assert.That(ModernStoneStatues.Matches(entity, otherView), Is.False);
            IReadOnlyDictionary<ushort, NativeSim.CampaignEntitySnapshot> current =
                new Dictionary<ushort, NativeSim.CampaignEntitySnapshot> { [entity.entityIndex] = entity };
            Assert.That(ModernStoneStatues.MatchesCurrent(current, view), Is.True);
            var stale = view; stale.sourceId++;
            Assert.That(ModernStoneStatues.MatchesCurrent(current, stale), Is.False);
            stale = view; stale.entityIndex++;
            Assert.That(ModernStoneStatues.MatchesCurrent(current, stale), Is.False);
        }

        [TestCase("ModernScenery/StoneStatue")]
        [TestCase("ModernScenery/StoneStatueVoxel")]
        public void ImportedStatueIsGroundedUprightAndBounded(string resource)
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
                Assert.That(bounds.size.y, Is.InRange(.95f, 1.05f));
                Assert.That(bounds.size.x / bounds.size.y, Is.InRange(.32f, .5f));
                Assert.That(bounds.size.z, Is.InRange(.20f, .35f));
                var triangles = 0;
                foreach (var filter in instance.GetComponentsInChildren<MeshFilter>())
                    triangles += filter.sharedMesh.triangles.Length / 3;
                Assert.That(triangles, Is.InRange(100, 30000));
            }
            finally { Object.DestroyImmediate(instance); }
        }
    }
}
