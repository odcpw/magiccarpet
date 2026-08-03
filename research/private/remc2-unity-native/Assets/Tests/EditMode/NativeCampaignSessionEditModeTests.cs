using System.IO;
using System.Linq;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class NativeCampaignSessionEditModeTests
    {
        private static string PrivateRoot =>
            Path.GetFullPath(Path.Combine(Application.dataPath, "../.."));

        private static string Level2Package =>
            Path.Combine(PrivateRoot, "generated/l2-debug3.fJ3PaM/level2");

        [Test]
        public void Level2CampaignCabiIsReachableFromManagedSession()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);
            var initial = session.WorldInfo();
            Assert.That(initial.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
            Assert.That(initial.structSize, Is.EqualTo(64));
            Assert.That(initial.externalLevel, Is.EqualTo(2));
            Assert.That(initial.internalLevel, Is.EqualTo(1));
            Assert.That(initial.entityCount, Is.EqualTo(1000));
            Assert.That(initial.activeEntityCount, Is.EqualTo(114));
            Assert.That(initial.playerCount, Is.EqualTo(8));
            Assert.That(initial.terrainWidth, Is.EqualTo(256));
            Assert.That(initial.terrainHeight, Is.EqualTo(256));

            var players = session.Players();
            Assert.That(players, Has.Length.EqualTo(8));
            Assert.That(players.Count(player => player.active != 0), Is.EqualTo(2));

            var entities = session.Entities();
            Assert.That(entities, Has.Length.EqualTo(1000));
            Assert.That(entities.Count(entity => entity.active != 0), Is.EqualTo(114));

            var stepped = session.Step(123, 456, 7);
            Assert.That(stepped.snapshot.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
            Assert.That(stepped.snapshot.structSize, Is.EqualTo(112));
            Assert.That(stepped.snapshot.generation, Is.EqualTo(0));
            Assert.That(stepped.snapshot.revision, Is.EqualTo(1));
            Assert.That(stepped.snapshot.eventCount, Is.GreaterThan(0));
            Assert.That(stepped.events, Has.Length.EqualTo((int)stepped.snapshot.eventCount));
            Assert.That(
                stepped.events.Any(eventRow => eventRow.kind != NativeSim.CampaignEventError),
                Is.True);
            var economyEvents = session.EconomyEvents();
            // The idle Level 2 fixture may legitimately produce no economy
            // sidecars on its first turn. The accessor still has to return a
            // valid managed array, while the native C ABI test exercises a
            // positive carrier/economy route.
            Assert.That(economyEvents, Is.Not.Null);
            Assert.That(
                economyEvents.All(eventRow =>
                    eventRow.abiVersion == NativeSim.AbiVersion &&
                    eventRow.generation == stepped.snapshot.generation),
                Is.True);

            session.Reset();
            Assert.That(session.Generation, Is.EqualTo(1));
            var reset = session.WorldInfo();
            Assert.That(reset.resetStateHash, Is.EqualTo(initial.resetStateHash));
        }

        [Test]
        public void Level2CampaignGoatDispatchIsReachableFromManagedSession()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);

            var stepped = session.Step(123, 456, 7);
            var goatEvents = stepped.events
                .Where(eventRow => eventRow.kind == NativeSim.CampaignEventGoat)
                .ToArray();

            Assert.That(goatEvents, Is.Not.Empty);
            Assert.That(goatEvents.Select(eventRow => eventRow.sourceSlot), Does.Contain(39));
            Assert.That(goatEvents.Select(eventRow => eventRow.sourceSlot), Does.Contain(71));
            Assert.That(
                goatEvents.All(eventRow =>
                    eventRow.abiVersion == NativeSim.AbiVersion &&
                    eventRow.generation == stepped.snapshot.generation),
                Is.True);
        }

        [Test]
        public void Level2CampaignGoatSnapshotsAreReachableFromManagedSession()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);

            var goats = session.Goats();

            Assert.That(goats, Has.Length.EqualTo(36));
            Assert.That(goats.Select(goat => goat.entityIndex), Does.Contain(39));
            Assert.That(goats.Select(goat => goat.entityIndex), Does.Contain(71));
            Assert.That(
                goats.Where(goat => goat.entityIndex == 39 || goat.entityIndex == 71)
                    .All(goat =>
                        goat.abiVersion == NativeSim.AbiVersion &&
                        goat.structSize == 84 &&
                        goat.classId == 5 &&
                        goat.model == 1 &&
                        goat.action == 11),
                Is.True);
        }

        [Test]
        public void Level2CampaignManaArenaSnapshotsAreReachableFromManagedSession()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);

            var spheres = session.ManaSpheres();
            Assert.That(spheres, Is.Not.Null);
            Assert.That(
                spheres.All(sphere =>
                    sphere.abiVersion == NativeSim.AbiVersion &&
                    sphere.structSize == 64 &&
                    sphere.classId == 10 &&
                    sphere.model == 39 &&
                    sphere.action == 41),
                Is.True);

            // The reset fixture has no carrier rows, but the accessor must
            // preserve the shared-arena contract and return a valid empty
            // array; the native C ABI test covers the positive multi-carrier
            // route and retained-row semantics.
            var carriers = session.ManaCarriers();
            Assert.That(carriers, Is.Not.Null);
            Assert.That(
                carriers.All(carrier =>
                    carrier.abiVersion == NativeSim.AbiVersion &&
                    carrier.structSize == 80 &&
                    carrier.generation == session.Generation),
                Is.True);

            session.Reset();
            Assert.That(session.ManaSpheres(), Has.Length.EqualTo(spheres.Length));
            Assert.That(session.ManaCarriers(), Is.Empty);
        }

        [Test]
        public void Level2CampaignShieldSnapshotBridgeIsReachableAndResetBound()
        {
            using var session = NativeCampaignSession.OpenLevel2(Level2Package);
            var state = session.ShieldState();
            Assert.That(state.abiVersion, Is.EqualTo(NativeSim.AbiVersion));
            Assert.That(state.structSize, Is.EqualTo(72));
            Assert.That(state.generation, Is.EqualTo(session.Generation));
            Assert.That(state.active, Is.EqualTo(0));
            Assert.Throws<System.InvalidOperationException>(() => session.ActivateShield());

            session.Reset();
            var reset = session.ShieldState();
            Assert.That(reset.generation, Is.EqualTo(session.Generation));
            Assert.That(reset.active, Is.EqualTo(0));
        }
    }
}
