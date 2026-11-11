using System;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernCollectorTests
    {
        private static NativeSim.CampaignCastleChild Child() => new NativeSim.CampaignCastleChild {
            entityIndex = 372, sourceId = 152, slotGeneration = 11,
            renderParameter = 169, renderHeight = 800, ownerPlayerColor = 0,
            relationFlags = 3, drawing = 1
        };

        private static NativeSim.CampaignEntityPresentation Sprite() => new NativeSim.CampaignEntityPresentation {
            entityIndex = 372, sourceId = 152, renderParameterIndex = 169,
            logicalHeight = 800, selectedTmap = 88
        };

        [Test]
        public void NativeLossReuseAndResetDoNotLeaveAStaleBalloon()
        {
            var parent = new GameObject("collector lifecycle test");
            var presenter = new ModernCollector(parent.transform);
            try
            {
                var child = Child(); var sprite = Sprite();
                presenter.Present(1, 2304, new[] { child }, new[] { sprite });
                Assert.That(presenter.Replaces(372, 152), Is.True);
                var collection = parent.transform.GetChild(0);
                Assert.That(collection.childCount, Is.EqualTo(1));
                var first = collection.GetChild(0);
                Assert.That(first.gameObject.activeSelf, Is.True);

                child.drawing = 0;
                presenter.Present(1, 2305, new[] { child }, new[] { sprite });
                Assert.That(presenter.Replaces(372, 152), Is.False);
                Assert.That(first.gameObject.activeSelf, Is.False, "hidden source row must hide the model");
                presenter.Present(1, 2306, Array.Empty<NativeSim.CampaignCastleChild>(), Array.Empty<NativeSim.CampaignEntityPresentation>());
                Assert.That(collection.childCount, Is.Zero, "reclaimed source row must destroy the visual");

                child.drawing = 1; child.slotGeneration = 12;
                presenter.Present(1, 2307, new[] { child }, new[] { sprite });
                Assert.That(presenter.Replaces(372, 152), Is.True);
                var reused = collection.GetChild(0);
                child.slotGeneration = 13;
                presenter.Present(1, 2308, new[] { child }, new[] { sprite });
                Assert.That(collection.childCount, Is.EqualTo(1));
                Assert.That(collection.GetChild(0), Is.Not.SameAs(reused), "reused slot gets a fresh instance");
                var beforeReset = collection.GetChild(0);
                presenter.Present(2, 0, Array.Empty<NativeSim.CampaignCastleChild>(), Array.Empty<NativeSim.CampaignEntityPresentation>());
                Assert.That(presenter.Replaces(372, 152), Is.False);
                Assert.That(collection.childCount, Is.Zero, "reset clears the previous generation");
                presenter.Present(2, 2304, new[] { child }, new[] { sprite });
                Assert.That(presenter.Replaces(372, 152), Is.True);
                Assert.That(collection.GetChild(0), Is.Not.SameAs(beforeReset));
                sprite.selectedTmap = 89;
                presenter.Present(2, 2305, new[] { child }, new[] { sprite });
                Assert.That(presenter.Replaces(372, 152), Is.False, "mismatched sprite must not be replaced");
            }
            finally
            {
                presenter.Dispose();
                UnityEngine.Object.DestroyImmediate(parent);
            }
        }
    }
}
