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
            Assert.That(economyEvents, Is.Not.Empty);
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
    }
}
