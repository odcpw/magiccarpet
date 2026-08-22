using System;
using System.Linq;
using NUnit.Framework;

namespace MagicCarpet.Native.Tests
{
    public sealed class SemanticFireballEditModeTests
    {
        private static string PrivateRoot =>
            System.IO.Path.GetFullPath(System.IO.Path.Combine(UnityEngine.Application.dataPath, "../.."));

        private static string PackagePath =>
            System.IO.Path.Combine(PrivateRoot, "generated/level1-export-m");

        private static string ClosurePath =>
            System.IO.Path.Combine(PrivateRoot, "generated/level1-asset-closure.json");

        private static string AssetCachePath =>
            System.IO.Path.Combine(PrivateRoot, "generated/level1-unity-asset-cache-plan29-final");

        [Test]
        [Timeout(120000)]
        public void PublishedFireballCostChangesTheOrdinaryRustManaDebitOnly()
        {
            var workspace = Environment.GetEnvironmentVariable("MC_SEMANTIC_VARIANT_WORKSPACE");
            Assert.That(workspace, Is.Not.Null.And.Not.Empty,
                "semantic qualification must provide the published definition workspace");
            using var baseline = NativeSession.OpenEngine(
                PackagePath, ClosurePath, AssetCachePath);
            using var authored = NativeSession.OpenEngine(
                PackagePath, ClosurePath, AssetCachePath, workspace);

            baseline.Step(0, 0, 0x10);
            authored.Step(0, 0, 0x10);
            var baselineSnapshot = baseline.EngineSnapshot();
            var authoredSnapshot = authored.EngineSnapshot();
            Assert.That(baselineSnapshot.value.caster_mana, Is.EqualTo(900));
            Assert.That(authoredSnapshot.value.caster_mana, Is.EqualTo(800));
            Assert.That(authoredSnapshot.value.tick, Is.EqualTo(baselineSnapshot.value.tick));
            Assert.That(authoredSnapshot.value.player_pose.x,
                Is.EqualTo(baselineSnapshot.value.player_pose.x));
            Assert.That(authoredSnapshot.value.player_pose.y,
                Is.EqualTo(baselineSnapshot.value.player_pose.y));
            Assert.That(authoredSnapshot.value.player_pose.z,
                Is.EqualTo(baselineSnapshot.value.player_pose.z));
            Assert.That(authoredSnapshot.value.player_motion.target_speed,
                Is.EqualTo(baselineSnapshot.value.player_motion.target_speed));
            Assert.That(authoredSnapshot.value.player_motion.actual_speed,
                Is.EqualTo(baselineSnapshot.value.player_motion.actual_speed));
            Assert.That(authoredSnapshot.value.objective_count,
                Is.EqualTo(baselineSnapshot.value.objective_count));
            Assert.That(authoredSnapshot.value.action_sequence,
                Is.EqualTo(baselineSnapshot.value.action_sequence));
            var baselineEvents = baseline.EngineEvents("0").value.events
                .Select(value => value.event_id).ToArray();
            var authoredEvents = authored.EngineEvents("0").value.events
                .Select(value => value.event_id).ToArray();
            Assert.That(authoredEvents, Is.EqualTo(baselineEvents));
            Assert.That(authoredEvents, Does.Contain("combat.fireball_spawned"));

            var firstValue = authoredSnapshot.value;
            authored.Reset();
            authored.Step(0, 0, 0x10);
            var replay = authored.EngineSnapshot().value;
            Assert.That(replay.caster_mana, Is.EqualTo(firstValue.caster_mana));
            Assert.That(replay.state_hash, Is.EqualTo(firstValue.state_hash));
            Assert.That(replay.conformance_hash, Is.EqualTo(firstValue.conformance_hash));
        }
    }
}
