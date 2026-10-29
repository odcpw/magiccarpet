using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class SourceMinimapLabelsTests
    {
        [Test]
        public void OriginalFontMasksPreserveCoverageAndLabelsBeyondMapEdge()
        {
            var root = Path.GetFullPath(Path.Combine(Application.dataPath,
                "../../generated/mc-level2-vault-successor-environment-20261025"));
            var descriptor = JsonUtility.FromJson<OwnerAssetCache.EnvironmentDescriptor>(
                File.ReadAllText(Path.Combine(root, "materials/descriptor.json")));
            var bank = descriptor.bitmap_banks.Single(row => row.bank == "FONT1");
            var font = new Dictionary<int, SourceMinimapLabels.Glyph>();
            foreach (var character in "Nyphur")
            {
                var row = bank.bitmaps.Single(glyph => glyph.selector == character + 1);
                font[character + 1] = new SourceMinimapLabels.Glyph {
                    Width = row.width, Height = row.height,
                    Mask = File.ReadAllBytes(Path.Combine(root, row.mask_path))
                };
            }
            var tables = File.ReadAllBytes(Path.Combine(root, descriptor.tables.path));
            var frame = Enumerable.Repeat((byte)3, 320 * 200).ToArray();
            var coverage = new bool[frame.Length];
            var snapshot = new SourceMinimapLabels.Snapshot {
                generation = 7, tick = 11,
                labels = new[] { new SourceMinimapLabels.Label {
                    source_order = 1001, subject_entity_index = 112,
                    x = 120, y = 64, palette_index = 92, text = "Nyphur"
                } }
            };
            var written = SourceMinimapLabels.Rasterize(frame, snapshot, tables, font, 7, 11, coverage);
            Assert.That(written, Is.EqualTo("Nyphur".Sum(character =>
                font[character + 1].Mask.Count(value => value != 0))));
            Assert.That(coverage.Where((value, index) => value && index % 320 >= 64).Any(), Is.True,
                "source name must survive outside the minimap rectangle");
            Assert.That(frame.Where((value, index) => coverage[index]).All(value => value == 92), Is.True);
            Assert.That(frame.Where((value, index) => !coverage[index]).All(value => value == 3), Is.True,
                "transparent FONT1 pixels leave the indexed frame unchanged");

            snapshot.labels[0].x = 0;
            snapshot.labels[0].y = 0;
            Assert.That(SourceMinimapLabels.Rasterize(frame, snapshot, tables, font, 7, 11), Is.Zero,
                "off-circle dot suppresses its entire label");
            Assert.Throws<InvalidOperationException>(() =>
                SourceMinimapLabels.Rasterize(frame, snapshot, tables, font, 7, 12));
        }

        [Test]
        public void SourcePickupCastAndExpiryReachEnvironmentMapLabels()
        {
            // This exercises the native source-session position input, not a
            // player navigation or borrowed GameRuntime acquisition route.
            var generated = Path.GetFullPath(Path.Combine(Application.dataPath, "../../generated"));
            var package = Path.Combine(generated, "mc-level2-vault-successor-closure-20261025");
            var cachePath = Path.Combine(generated, "mc-level2-vault-successor-environment-20261025");
            var cache = OwnerAssetCache.OpenAndValidateEnvironment(cachePath, package);
            using var session = NativeCampaignSession.OpenLevel2(package, cachePath);
            using var surface = new CampaignPresentationSurface();
            var labels = (SourceMinimapLabels)typeof(CampaignPresentationSurface)
                .GetField("sourceMinimapLabels", BindingFlags.Instance | BindingFlags.NonPublic)
                .GetValue(surface);
            var read = typeof(SourceMinimapLabels).GetMethod("Read", BindingFlags.Static | BindingFlags.NonPublic);
            SourceMinimapLabels.Snapshot Plan(bool fullMap) => (SourceMinimapLabels.Snapshot)read.Invoke(
                null, new object[] { session.Handle, session.Generation, session.Tick, fullMap });

            var source = session.Entities().Single(row => row.entityIndex == 38);
            Assert.That((source.active, source.classId, source.model, source.action),
                Is.EqualTo(((byte)1, (byte)15, (byte)12, (byte)37)));
            surface.PresentEnvironment(session, cache);
            Assert.That(Plan(false).labels, Is.Empty);
            Assert.That(labels.Coverage, Is.Null);

            var presenter = new CampaignSpellPresenter();
            presenter.Reset(session.Generation);
            var sourcePassBound = checked((int)session.WorldInfo().entityCount * 8);
            var castle = NativeCampaignSessionEditModeTests.DriveLevel2OrdinaryManaWindow(
                session, presenter, 1_000, sourcePassBound);
            var acquired = false;
            for (var pass = 0; pass < 4 && !acquired; pass++)
            {
                var step = NativeCampaignSessionEditModeTests.StepLevel2SpellPresentation(
                    session, presenter, source.x, source.y, source.z, flags: 0);
                acquired = step.events.Any(row => row.kind == NativeSim.CampaignEventSpellAcquisition &&
                    row.family == 12 && row.sourceSlot == source.entityIndex && row.flags == 1);
            }
            Assert.That(acquired, Is.True, "ordinary overlap must acquire the source holder");
            var cold = session.BeyondSightState();
            Assert.That(cold.holderEntityIndex, Is.EqualTo(source.entityIndex));
            Assert.That(cold.activeValue, Is.Zero);
            surface.PresentEnvironment(session, cache);
            Assert.That(Plan(false).labels, Is.Empty);
            Assert.That(labels.Coverage, Is.Null);

            NativeCampaignSessionEditModeTests.DriveLevel2FirstCastleDefenderReturn(
                session, presenter, (castle.x, castle.y, castle.z), cold.manaCost,
                sourcePassBound, castle.rootTick);
            session.Step(source.x, source.y, source.z, flags: 0x40, requestedSpell: 12);
            Assert.That(session.BeyondSightState().activeValue, Is.GreaterThan(0));
            var corner = Plan(false);
            Assert.That(corner.labels, Has.Length.EqualTo(1));
            Assert.That(corner.labels[0].text, Is.EqualTo("Nyphur"));
            Assert.That(corner.labels[0].subject_entity_index, Is.EqualTo(112));
            surface.PresentEnvironment(session, cache);
            // The actual opponent is outside the corner map at this source
            // pickup; the native plan exists but its dot and name must clip.
            Assert.That(labels.Coverage, Is.Null);

            var full = Plan(true);
            Assert.That(full.labels, Has.Length.EqualTo(1));
            Assert.That(full.labels[0].text, Is.EqualTo("Nyphur"));
            Assert.That(full.labels[0].subject_entity_index, Is.EqualTo(112));
            surface.PresentEnvironment(session, cache, 0, true);
            Assert.That(labels.Coverage, Is.Not.Null);
            Assert.That(labels.Coverage.Any(pixel => pixel), Is.True,
                "the visible native full-map label must use the owner's FONT1 masks");
            var overlay = surface.Root.GetComponent<CampaignMinimapOverlay>();
            var texture = (Texture2D)typeof(CampaignMinimapOverlay)
                .GetField("texture", BindingFlags.Instance | BindingFlags.NonPublic).GetValue(overlay);
            Assert.That((texture.width, texture.height), Is.EqualTo((320, 200)));
            var pixels = texture.GetPixels32();
            var palette = session.WorldMap(session.Tick).palette;
            var color = full.labels[0].palette_index * 3;
            Assert.That(Enumerable.Range(0, labels.Coverage.Length).Any(index =>
            {
                var pixel = pixels[(199 - index / 320) * 320 + index % 320];
                return labels.Coverage[index] && pixel.a == 255 &&
                    pixel.r == palette[color] && pixel.g == palette[color + 1] && pixel.b == palette[color + 2];
            }), Is.True, "FONT1 coverage reaches the actual indexed map overlay");

            // Move the already acquired and funded source session to the
            // current opponent through the existing position input. This is
            // controlled component evidence, not GameRuntime navigation.
            var opponent = session.Entities().Single(row => row.entityIndex == 112 && row.active == 1);
            session.Step(opponent.x, opponent.y, opponent.z, flags: 0);
            Assert.That(session.BeyondSightState().activeValue, Is.GreaterThan(0));
            var nearbyPlan = Plan(false);
            Assert.That(nearbyPlan.labels, Has.Length.EqualTo(1));
            Assert.That(nearbyPlan.labels[0].subject_entity_index, Is.EqualTo(opponent.entityIndex));
            Assert.That(nearbyPlan.labels[0].text, Is.EqualTo("Nyphur"));
            surface.PresentEnvironment(session, cache);
            Assert.That(labels.Coverage, Is.Not.Null);
            Assert.That(labels.Coverage.Any(pixel => pixel), Is.True,
                "an in-range native opponent must materialize original FONT1 on the corner map");
            texture = (Texture2D)typeof(CampaignMinimapOverlay)
                .GetField("texture", BindingFlags.Instance | BindingFlags.NonPublic).GetValue(overlay);
            Assert.That((texture.width, texture.height), Is.EqualTo((320, 200)));
            pixels = texture.GetPixels32();
            palette = session.DayIndexedFramePalette;
            color = nearbyPlan.labels[0].palette_index * 3;
            Assert.That(Enumerable.Range(0, labels.Coverage.Length).Any(index =>
            {
                var pixel = pixels[(199 - index / 320) * 320 + index % 320];
                return labels.Coverage[index] && pixel.a == 255 &&
                    pixel.r == palette[color] && pixel.g == palette[color + 1] && pixel.b == palette[color + 2];
            }), Is.True, "the corner-map overlay publishes the source-colored name glyphs");
            surface.PresentEnvironment(session, cache, 0, true);
            Assert.That(labels.Coverage, Is.Not.Null);
            Assert.That(labels.Coverage.Any(pixel => pixel), Is.True);

            for (var pass = 0; pass < sourcePassBound && session.BeyondSightState().activeValue > 0; pass++)
                session.Step(opponent.x, opponent.y, opponent.z, flags: 0);
            Assert.That(session.BeyondSightState().activeValue, Is.Zero,
                "ordinary source visits must expire the active effect");
            Assert.That(session.BeyondSightState().holderEntityIndex, Is.EqualTo(source.entityIndex));
            surface.PresentEnvironment(session, cache);
            Assert.That(Plan(false).labels, Is.Empty);
            Assert.That(labels.Coverage, Is.Null);
            surface.PresentEnvironment(session, cache, 0, true);
            Assert.That(Plan(true).labels, Is.Empty);
            Assert.That(labels.Coverage, Is.Null);

            var oldGeneration = session.Generation;
            var oldTick = session.Tick;
            session.Reset();
            Assert.That(session.Generation, Is.Not.EqualTo(oldGeneration));
            surface.PresentEnvironment(session, cache);
            Assert.That(labels.Coverage, Is.Null);
            Assert.That(Plan(false).labels, Is.Empty);
            var status = NativeSim.mc_native_campaign_minimap_labels_json(
                session.Handle, oldGeneration, oldTick, IntPtr.Zero, 0, out var required);
            Assert.That(status, Is.EqualTo(NativeSim.GenerationMismatch));
            Assert.That(required, Is.Zero);

        }

        [Test]
        public void OverlayRetainsOnlyNameCoverageOutsideMapAndRestoresSmallTexture()
        {
            var gameObject = new GameObject("Source minimap label regression");
            try
            {
                var overlay = gameObject.AddComponent<CampaignMinimapOverlay>();
                var frame = new byte[320 * 200];
                var palette = new byte[768];
                var tables = new byte[83456];
                var coverage = new bool[frame.Length];
                frame[32 * 320 + 90] = 7;
                palette[21] = 210;
                coverage[32 * 320 + 90] = true;
                overlay.Present(frame, palette, tables, coverage);
                var field = typeof(CampaignMinimapOverlay).GetField("texture",
                    BindingFlags.NonPublic | BindingFlags.Instance);
                var texture = (Texture2D)field.GetValue(overlay);
                Assert.That(texture.width, Is.EqualTo(320));
                Assert.That(texture.height, Is.EqualTo(200));
                var pixels = texture.GetPixels32();
                Assert.That(pixels[(199 - 32) * 320 + 90].a, Is.EqualTo(255));
                Assert.That(pixels[(199 - 32) * 320 + 90].r, Is.EqualTo(210));
                Assert.That(pixels[(199 - 32) * 320 + 91].a, Is.Zero);
                overlay.Present(frame, palette, tables);
                texture = (Texture2D)field.GetValue(overlay);
                Assert.That(texture.width, Is.EqualTo(64));
                Assert.That(texture.height, Is.EqualTo(64));
            }
            finally { UnityEngine.Object.DestroyImmediate(gameObject); }
        }
    }
}
