using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class Level2EffectPresenterEditModeTests
    {
        private static string PrivateRoot =>
            Path.GetFullPath(Path.Combine(Application.dataPath, "../.."));

        private static string Level2Package =>
            Path.Combine(PrivateRoot, "generated/l2-debug3.fJ3PaM/level2");

        private static string Level2DayCache =>
            Path.Combine(PrivateRoot, "generated/level2-day-owner-cache");

        [Test]
        public void Level2SpellPresenterProjectsRootedFamiliesInRustEventOrder()
        {
            var events = new[]
            {
                EventRow(0, 17, 401, 2),
                EventRow(1, 19, 402, 3),
                EventRow(2, 21, 403, 4),
                EventRow(3, 11, 404, 5),
                EventRow(4, 23, 405, 6)
            };
            var step = Step(7, 12, events.Length, 3120, 4096, 384);
            var presenter = new Level2SpellPresenter();
            presenter.Reset(step.generation);

            Present(presenter, step, events);

            var effects = presenter.RootedEffects;
            Assert.That(effects, Has.Length.EqualTo(5));
            CollectionAssert.AreEqual(
                new byte[] { 17, 19, 21, 11, 23 },
                effects.Select(effect => effect.Family).ToArray());
            CollectionAssert.AreEqual(
                new byte[] { 17, 22, 13, 45, 76 },
                effects.Select(effect => effect.Model).ToArray());
            CollectionAssert.AreEqual(
                new byte[] { 17, 22, 19, 51, 83 },
                effects.Select(effect => effect.Action).ToArray());

            for (var index = 0; index < effects.Length; index++)
            {
                var effect = effects[index];
                Assert.That(effect.ClassId, Is.EqualTo(10));
                Assert.That(effect.Generation, Is.EqualTo(step.generation));
                Assert.That(effect.Tick, Is.EqualTo(step.revision));
                Assert.That(effect.SourceSlot, Is.EqualTo((ushort)(401 + index)));
                Assert.That(effect.EventOrdinal, Is.EqualTo((uint)index));
                Assert.That(effect.SourcePass, Is.EqualTo((byte)(index + 2)));
                Assert.That(effect.X, Is.EqualTo(step.x));
                Assert.That(effect.Y, Is.EqualTo(step.y));
                Assert.That(effect.Z, Is.EqualTo(step.z));
                Assert.That(
                    effect.UsesMsprd,
                    Is.EqualTo(effect.Family == 23),
                    $"MSPRD recipe binding for family {effect.Family}");
            }
        }

        [Test]
        public void Level2RootedEffectsExpireOnFollowingProjectionAndResetIdempotently()
        {
            var presenter = new Level2SpellPresenter();
            presenter.Reset(9);
            var firstEvents = new[] { EventRow(0, 17, 501, 1, generation: 9) };
            var first = Step(9, 30, firstEvents.Length, 100, 200, 256);
            Present(presenter, first, firstEvents);
            Assert.That(presenter.RootedEffects, Has.Length.EqualTo(1));

            var following = Step(9, 31, 0, 100, 200, 256);
            Present(presenter, following, Array.Empty<NativeSim.CampaignEvent>());
            Assert.That(presenter.RootedEffects, Is.Empty);
            Present(presenter, following, Array.Empty<NativeSim.CampaignEvent>());
            Assert.That(presenter.RootedEffects, Is.Empty);

            presenter.Reset(10);
            Assert.That(presenter.RootedEffects, Is.Empty);
            presenter.Dispose();
            Assert.That(presenter.RootedEffects, Is.Empty);
        }

        [Test]
        public void Level2RootedEffectsRejectMalformedStaleDuplicateAndUnrootedRows()
        {
            var valid = EventRow(0, 17, 601, 1, generation: 11);
            var validStep = Step(11, 40, 1, 700, 800, 256);

            var malformed = new Level2SpellPresenter();
            malformed.Reset(validStep.generation);
            var malformedRow = valid;
            malformedRow.structSize = 47;
            Present(malformed, validStep, new[] { malformedRow });
            Assert.That(malformed.RootedEffects, Is.Empty);

            var wrongKind = new Level2SpellPresenter();
            wrongKind.Reset(validStep.generation);
            var unrooted = valid;
            unrooted.kind = NativeSim.CampaignEventSpellAcquisition;
            Present(wrongKind, validStep, new[] { unrooted });
            Assert.That(wrongKind.RootedEffects, Is.Empty);

            var rejected = new Level2SpellPresenter();
            rejected.Reset(validStep.generation);
            var rejectedRow = valid;
            rejectedRow.flags = 0;
            Present(rejected, validStep, new[] { rejectedRow });
            Assert.That(rejected.RootedEffects, Is.Empty);

            var duplicate = new Level2SpellPresenter();
            duplicate.Reset(validStep.generation);
            var duplicateRow = EventRow(
                1,
                17,
                valid.sourceSlot,
                2,
                generation: 11);
            var duplicateStep = Step(11, 41, 2, 700, 800, 256);
            Present(duplicate, duplicateStep, new[] { valid, duplicateRow });
            Assert.That(duplicate.RootedEffects, Is.Empty);

            var unknown = new Level2SpellPresenter();
            unknown.Reset(validStep.generation);
            var unknownRow = EventRow(0, 99, 602, 1, generation: 11);
            Present(unknown, validStep, new[] { unknownRow });
            Assert.That(unknown.RootedEffects, Is.Empty);

            var stale = new Level2SpellPresenter();
            stale.Reset(validStep.generation);
            Present(stale, validStep, new[] { valid });
            var staleStep = Step(10, 41, 1, 700, 800, 256);
            var staleRow = EventRow(0, 17, 601, 1, generation: 10);
            Present(stale, staleStep, new[] { staleRow });
            Assert.That(stale.RootedEffects, Is.Empty);
        }

        [Test]
        public void Level2RootedEffectsConsumeProductionCampaignEventPage()
        {
            using var session = NativeCampaignSession.OpenLevel2(
                Level2Package,
                Level2DayCache);
            var presenter = new Level2SpellPresenter();
            presenter.Reset(session.Generation);

            var player = session.Players().Single(candidate => candidate.slot == 0);
            var playerEntity = session.Entities()
                .Single(candidate => candidate.entityIndex == player.entityIndex);
            var objective = session.ObjectiveStageState();
            var seen = new Dictionary<byte, Level2EffectPresentation>();
            var positions = session.Entities()
                .Where(candidate => candidate.active == 1)
                .Select(candidate => (candidate.x, candidate.y, candidate.z))
                .Append((playerEntity.x, playerEntity.y, playerEntity.z))
                .Append((
                    unchecked((ushort)objective.axisX),
                    unchecked((ushort)objective.axisY),
                    (short)256))
                .Distinct()
                .ToArray();
            foreach (var position in positions)
            {
                for (var pulse = 0; pulse < 4; pulse++)
                {
                    var stepped = session.Step(position.x, position.y, position.z);
                    var hud = session.DayHudState(stepped.snapshot.revision);
                    presenter.Present(
                        stepped.snapshot,
                        stepped.events,
                        hud,
                        session.ShieldState(),
                        session.InvisibleState(),
                        session.BeyondSightState());
                    foreach (var effect in presenter.RootedEffects)
                    {
                        seen.TryAdd(effect.Family, effect);
                    }

                    if (seen.Count == 5)
                    {
                        break;
                    }
                }
                if (seen.Count == 5)
                {
                    break;
                }
            }

            var expectedFamilies = new byte[] { 11, 17, 19, 21, 23 };
            Assert.That(
                seen.Keys,
                Is.EquivalentTo(expectedFamilies),
                $"production rooted families observed: " +
                string.Join(",", seen.Keys.OrderBy(family => family)));
            Assert.That(
                seen.Values.All(effect => effect.Generation == session.Generation),
                Is.True);
            Assert.That(
                seen.Values.All(effect => effect.ClassId == 10),
                Is.True);
        }

        private static void Present(
            Level2SpellPresenter presenter,
            NativeSim.CampaignStepSnapshot step,
            NativeSim.CampaignEvent[] events)
        {
            var hud = default(NativeSim.CampaignDayHudSnapshot);
            hud.abiVersion = NativeSim.AbiVersion;
            hud.structSize = 272;
            hud.generation = step.generation;
            hud.tick = step.revision;
            hud.localPlayerSlot = step.playerSlot;
            hud.localPlayerActive = 1;
            hud.localPlayerEntityIndex = step.playerEntityIndex;

            var shield = default(NativeSim.CampaignShieldSnapshot);
            shield.abiVersion = NativeSim.AbiVersion;
            shield.structSize = 72;
            shield.generation = step.generation;

            var invisible = default(NativeSim.CampaignInvisibleSnapshot);
            invisible.abiVersion = NativeSim.AbiVersion;
            invisible.structSize = 80;
            invisible.generation = step.generation;

            var beyondSight = default(NativeSim.CampaignBeyondSightSnapshot);
            beyondSight.abiVersion = NativeSim.AbiVersion;
            beyondSight.structSize = 80;
            beyondSight.generation = step.generation;

            presenter.Present(
                step,
                events,
                hud,
                shield,
                invisible,
                beyondSight);
        }

        private static NativeSim.CampaignStepSnapshot Step(
            ulong generation,
            ulong revision,
            int eventCount,
            ushort x,
            ushort y,
            short z)
        {
            return new NativeSim.CampaignStepSnapshot
            {
                abiVersion = NativeSim.AbiVersion,
                structSize = 112,
                generation = generation,
                revision = revision,
                eventCount = checked((uint)eventCount),
                playerSlot = 0,
                playerEntityIndex = 112,
                x = x,
                y = y,
                z = z,
                dispatchHash = new byte[32],
                reserved0 = new byte[3],
                reserved = new uint[4]
            };
        }

        private static NativeSim.CampaignEvent EventRow(
            uint ordinal,
            byte family,
            ushort sourceSlot,
            byte pass,
            byte flags = 1,
            byte kind = NativeSim.CampaignEventDynamicArena,
            ulong generation = 7)
        {
            return new NativeSim.CampaignEvent
            {
                abiVersion = NativeSim.AbiVersion,
                structSize = 48,
                generation = generation,
                ordinal = ordinal,
                pass = pass,
                kind = kind,
                family = family,
                flags = flags,
                sourceSlot = sourceSlot,
                reserved = new uint[4]
            };
        }
    }
}
