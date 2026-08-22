using System;
using System.Linq;
using NUnit.Framework;

namespace MagicCarpet.Native.Tests
{
    public sealed class SemanticFlightEditModeTests
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
        public void PublishedFlightLimitChangesTheOrdinaryRustObservationOnly()
        {
            var workspace = Environment.GetEnvironmentVariable("MC_SEMANTIC_VARIANT_WORKSPACE");
            Assert.That(workspace, Is.Not.Null.And.Not.Empty,
                "F6 qualification must provide the published definition workspace");
            using var baseline = NativeSession.OpenEngine(
                PackagePath, ClosurePath, AssetCachePath);
            using var authored = NativeSession.OpenEngine(
                PackagePath, ClosurePath, AssetCachePath, workspace);

            for (var tick = 0; tick < 2; tick++)
            {
                baseline.Step(0, 0, 1);
                authored.Step(0, 0, 1);
            }

            var baselineSnapshot = baseline.EngineSnapshot();
            var authoredSnapshot = authored.EngineSnapshot();
            Assert.That(baselineSnapshot.value.player_motion.target_speed, Is.EqualTo(32));
            Assert.That(authoredSnapshot.value.player_motion.target_speed, Is.EqualTo(16));
            Assert.That(
                authoredSnapshot.value.player_motion.actual_speed,
                Is.Not.EqualTo(baselineSnapshot.value.player_motion.actual_speed));
            Assert.That(
                new[]
                {
                    authoredSnapshot.value.player_pose.x,
                    authoredSnapshot.value.player_pose.y,
                    unchecked((ushort)authoredSnapshot.value.player_pose.z),
                },
                Is.Not.EqualTo(new[]
                {
                    baselineSnapshot.value.player_pose.x,
                    baselineSnapshot.value.player_pose.y,
                    unchecked((ushort)baselineSnapshot.value.player_pose.z),
                }));
            Assert.That(authoredSnapshot.value.tick, Is.EqualTo(baselineSnapshot.value.tick));
            Assert.That(authoredSnapshot.value.caster_mana,
                Is.EqualTo(baselineSnapshot.value.caster_mana));
            Assert.That(authoredSnapshot.value.objective_count,
                Is.EqualTo(baselineSnapshot.value.objective_count));
            Assert.That(authoredSnapshot.value.action_sequence,
                Is.EqualTo(baselineSnapshot.value.action_sequence));
            Assert.That(
                authored.EngineEvents("0").value.events.Select(value => value.event_id),
                Is.EqualTo(baseline.EngineEvents("0").value.events.Select(value => value.event_id)));

            var firstValue = authoredSnapshot.value;
            authored.Reset();
            for (var tick = 0; tick < 2; tick++)
                authored.Step(0, 0, 1);
            var replay = authored.EngineSnapshot().value;
            Assert.That(replay.player_motion.target_speed,
                Is.EqualTo(firstValue.player_motion.target_speed));
            Assert.That(replay.player_motion.actual_speed,
                Is.EqualTo(firstValue.player_motion.actual_speed));
            Assert.That(replay.player_pose.x, Is.EqualTo(firstValue.player_pose.x));
            Assert.That(replay.player_pose.y, Is.EqualTo(firstValue.player_pose.y));
            Assert.That(replay.player_pose.z, Is.EqualTo(firstValue.player_pose.z));
            Assert.That(replay.state_hash, Is.EqualTo(firstValue.state_hash));
            Assert.That(replay.conformance_hash, Is.EqualTo(firstValue.conformance_hash));
        }
    }
}
