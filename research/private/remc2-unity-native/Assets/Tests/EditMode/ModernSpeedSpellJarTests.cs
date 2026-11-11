using System;
using System.Collections.Generic;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernSpeedSpellJarTests
    {
        private static NativeSim.CampaignEntityPresentation View(ushort slot) => new NativeSim.CampaignEntityPresentation {
            generation = 4, entityIndex = slot, sourceId = slot, renderParameterIndex = 77,
            baseTmap = 59, selectedTmap = 59, logicalHeight = 150, renderMode = 1,
            x = 640, y = 1280, z = 192, controlFlags = 4
        };
        private static NativeSim.CampaignEntitySnapshot Entity(ushort slot, byte model, byte action) =>
            new NativeSim.CampaignEntitySnapshot {
                active = 1, entityIndex = slot, sourceId = slot, classId = 15, model = model, action = action
            };

        [TestCase(false)]
        [TestCase(true)]
        public void SpeedAndCastleCoexistAndNativePickupControlsEach(bool voxel)
        {
            var parent = new GameObject("spell jars");
            var camera = new GameObject("spell jar camera").AddComponent<Camera>();
            try
            {
                using var presenter = new ModernCastleSpellJar(parent.transform, voxel);
                var speed = Entity(49, 3, 10); var castle = Entity(31, 2, 7);
                var speedView = View(49); var castleView = View(31);
                var entities = new Dictionary<ushort, NativeSim.CampaignEntitySnapshot> { [49] = speed, [31] = castle };
                Assert.That(ModernCastleSpellJar.Matches(speed, speedView), Is.True);
                Assert.That(ModernCastleSpellJar.Matches(castle, castleView), Is.True);
                var other = speed; other.action = 11;
                Assert.That(ModernCastleSpellJar.Matches(other, speedView), Is.True, "respawning pickup");
                other.action = 9;
                Assert.That(ModernCastleSpellJar.Matches(other, speedView), Is.False, "collected holder");
                other.action = 7;
                Assert.That(ModernCastleSpellJar.Matches(other, speedView), Is.False, "Castle action on Speed model");
                var wrong = speedView; wrong.controlFlags = 5;
                Assert.That(ModernCastleSpellJar.Matches(speed, wrong), Is.False, "source hidden flag");
                presenter.Present(entities, new[] { speedView, castleView });
                presenter.Render(camera, true);
                Assert.That(presenter.Replaces(49), Is.True);
                Assert.That(presenter.Replaces(31), Is.True);
                Assert.That(parent.transform.childCount, Is.EqualTo(2));
                Assert.That(parent.transform.GetChild(0).GetComponentsInChildren<MeshRenderer>().Length, Is.GreaterThan(0));
                // Pickup suppresses only the Speed jar, leaving Castle visible.
                speed.action = 9; entities[49] = speed;
                presenter.Present(entities, new[] { castleView });
                Assert.That(presenter.Replaces(49), Is.False);
                Assert.That(presenter.Replaces(31), Is.True);
                Assert.That(parent.transform.childCount, Is.EqualTo(1));
                // A new generation may materialize the Speed jar again.
                speed.action = 10; entities[49] = speed; speedView.generation++;
                presenter.Present(entities, new[] { speedView });
                Assert.That(presenter.Replaces(49), Is.True);
                Assert.That(presenter.Replaces(31), Is.False);
                Assert.That(parent.transform.childCount, Is.EqualTo(1));
                presenter.Present(entities, Array.Empty<NativeSim.CampaignEntityPresentation>());
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
