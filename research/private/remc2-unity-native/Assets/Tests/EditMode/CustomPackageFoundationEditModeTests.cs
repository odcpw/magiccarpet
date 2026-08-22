using System;
using System.Linq;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class CustomPackageFoundationEditModeTests
    {
        private const string AssetSlot = "presentation.spell.fireball.projectile_asset";
        private const string Engagement = "ai.engagement_boundary";

        [Serializable]
        private sealed class PoseProjection
        {
            public ushort x;
            public ushort y;
            public short z;
            public short yaw;
            public short display_pitch;
            public short effective_pitch;
            public short roll;
        }

        [Serializable]
        private sealed class MotionProjection
        {
            public short target_speed;
            public short actual_speed;
            public short strafe_speed;
        }

        [Serializable]
        private sealed class Projection
        {
            public string definition_hash;
            public string package_graph_hash;
            public string tick;
            public string event_head;
            public string state_hash;
            public string conformance_hash;
            public PoseProjection player_pose;
            public MotionProjection player_motion;
            public int caster_mana;
            public uint objective_count;
            public string action_sequence;
            public string[] event_ids;
            public string[] engagement_pairs;
            public string asset_content_address;
            public string asset_indices_base64;
        }

        private sealed class Run
        {
            internal EngineSessionTransport.SnapshotObservation Snapshot;
            internal EngineSessionTransport.GameEvent[] Events;
        }

        private static string PrivateRoot =>
            System.IO.Path.GetFullPath(System.IO.Path.Combine(Application.dataPath, "../.."));

        private static string PackagePath =>
            System.IO.Path.Combine(PrivateRoot, "generated/level1-export-m");

        private static string ClosurePath =>
            System.IO.Path.Combine(PrivateRoot, "generated/level1-asset-closure.json");

        private static string AssetCachePath =>
            System.IO.Path.Combine(PrivateRoot, "generated/level1-unity-asset-cache-plan29-final");

        private static string Workspace(string name)
        {
            var value = Environment.GetEnvironmentVariable(name);
            Assert.That(value, Is.Not.Null.And.Not.Empty, $"F10 requires {name}");
            return value;
        }

        private static Run Drive(NativeSession session)
        {
            session.Step(0, 0, 0x01);
            session.Step(0, 0, 0x11);
            return new Run
            {
                Snapshot = session.EngineSnapshot(),
                Events = session.EngineEvents("0").value.events,
            };
        }

        private static string[] EventIds(Run run) =>
            run.Events.Select(value => value.event_id).ToArray();

        private static string[] NonEngagementIds(Run run) =>
            run.Events.Where(value => value.event_id != Engagement)
                .Select(value => value.event_id).ToArray();

        private static EngineSessionTransport.GameEvent[] Engagements(Run run) =>
            run.Events.Where(value => value.event_id == Engagement).ToArray();

        private static Projection Project(
            Run run,
            EngineSessionTransport.PresentationAssetObservation asset)
        {
            var identity = run.Snapshot.identity;
            var value = run.Snapshot.value;
            return new Projection
            {
                definition_hash = identity.definition_hash,
                package_graph_hash = identity.package_graph_hash,
                tick = value.tick,
                event_head = value.event_head,
                state_hash = value.state_hash,
                conformance_hash = value.conformance_hash,
                player_pose = new PoseProjection
                {
                    x = value.player_pose.x,
                    y = value.player_pose.y,
                    z = value.player_pose.z,
                    yaw = value.player_pose.yaw,
                    display_pitch = value.player_pose.display_pitch,
                    effective_pitch = value.player_pose.effective_pitch,
                    roll = value.player_pose.roll,
                },
                player_motion = new MotionProjection
                {
                    target_speed = value.player_motion.target_speed,
                    actual_speed = value.player_motion.actual_speed,
                    strafe_speed = value.player_motion.strafe_speed,
                },
                caster_mana = value.caster_mana,
                objective_count = value.objective_count,
                action_sequence = value.action_sequence,
                event_ids = EventIds(run),
                engagement_pairs = Engagements(run)
                    .Select(row => $"{row.actor}>{row.participant}").ToArray(),
                asset_content_address = asset.value.content_address,
                asset_indices_base64 = Convert.ToBase64String(asset.value.indices),
            };
        }

        [Test]
        [Timeout(180000)]
        public void OneExecutableComposesAllFourPackageAuthoredEffects()
        {
            using var mc2 = NativeSession.OpenEngine(PackagePath, ClosurePath, AssetCachePath);
            using var customBaseline = NativeSession.OpenEngine(
                PackagePath, ClosurePath, AssetCachePath,
                Workspace("MC_F10_CUSTOM_BASELINE_WORKSPACE"));
            using var flight = NativeSession.OpenEngine(
                PackagePath, ClosurePath, AssetCachePath, Workspace("MC_F10_FLIGHT_WORKSPACE"));
            using var fireball = NativeSession.OpenEngine(
                PackagePath, ClosurePath, AssetCachePath, Workspace("MC_F10_FIREBALL_WORKSPACE"));
            using var enemyPolicy = NativeSession.OpenEngine(
                PackagePath, ClosurePath, AssetCachePath, Workspace("MC_F10_AI_WORKSPACE"));
            using var asset = NativeSession.OpenEngine(
                PackagePath, ClosurePath, AssetCachePath, Workspace("MC_F10_ASSET_WORKSPACE"));
            using var combined = NativeSession.OpenEngine(
                PackagePath, ClosurePath, AssetCachePath, Workspace("MC_F10_COMBINED_WORKSPACE"));

            var mc2Run = Drive(mc2);
            var baselineRun = Drive(customBaseline);
            var flightRun = Drive(flight);
            var fireballRun = Drive(fireball);
            var enemyRun = Drive(enemyPolicy);
            var assetRun = Drive(asset);
            var combinedRun = Drive(combined);
            var baselineValue = baselineRun.Snapshot.value;

            Assert.That(JsonUtility.ToJson(mc2Run.Snapshot.value),
                Is.EqualTo(JsonUtility.ToJson(baselineValue)));
            Assert.That(EventIds(mc2Run), Is.EqualTo(EventIds(baselineRun)));

            Assert.That(baselineValue.player_motion.target_speed, Is.EqualTo(32));
            Assert.That(flightRun.Snapshot.value.player_motion.target_speed, Is.EqualTo(16));
            Assert.That(JsonUtility.ToJson(flightRun.Snapshot.value.player_pose),
                Is.Not.EqualTo(JsonUtility.ToJson(baselineValue.player_pose)));
            Assert.That(flightRun.Snapshot.value.caster_mana, Is.EqualTo(baselineValue.caster_mana));
            Assert.That(EventIds(flightRun), Is.EqualTo(EventIds(baselineRun)));

            Assert.That(baselineValue.caster_mana, Is.EqualTo(900));
            Assert.That(fireballRun.Snapshot.value.caster_mana, Is.EqualTo(800));
            Assert.That(JsonUtility.ToJson(fireballRun.Snapshot.value.player_pose),
                Is.EqualTo(JsonUtility.ToJson(baselineValue.player_pose)));
            Assert.That(JsonUtility.ToJson(fireballRun.Snapshot.value.player_motion),
                Is.EqualTo(JsonUtility.ToJson(baselineValue.player_motion)));
            Assert.That(EventIds(fireballRun), Is.EqualTo(EventIds(baselineRun)));
            Assert.That(EventIds(fireballRun), Does.Contain("combat.fireball_spawned"));

            Assert.That(Engagements(baselineRun), Is.Empty);
            Assert.That(Engagements(enemyRun), Is.Not.Empty);
            Assert.That(Engagements(enemyRun).All(row =>
                !string.IsNullOrEmpty(row.actor) && !string.IsNullOrEmpty(row.participant)), Is.True);
            Assert.That(enemyRun.Snapshot.value.caster_mana, Is.EqualTo(baselineValue.caster_mana));
            Assert.That(JsonUtility.ToJson(enemyRun.Snapshot.value.player_pose),
                Is.EqualTo(JsonUtility.ToJson(baselineValue.player_pose)));
            Assert.That(JsonUtility.ToJson(enemyRun.Snapshot.value.player_motion),
                Is.EqualTo(JsonUtility.ToJson(baselineValue.player_motion)));
            Assert.That(NonEngagementIds(enemyRun), Is.EqualTo(EventIds(baselineRun)));

            var baselineAsset = customBaseline.EnginePresentationAsset(AssetSlot);
            var assetOnly = asset.EnginePresentationAsset(AssetSlot);
            var combinedAsset = combined.EnginePresentationAsset(AssetSlot);
            Assert.That(assetOnly.value.content_address,
                Is.Not.EqualTo(baselineAsset.value.content_address));
            Assert.That(JsonUtility.ToJson(assetRun.Snapshot.value),
                Is.EqualTo(JsonUtility.ToJson(baselineValue)));
            Assert.That(EventIds(assetRun), Is.EqualTo(EventIds(baselineRun)));
            using (var cache = new SemanticPresentationAssetCache())
            {
                var first = cache.Bind(baselineAsset);
                Assert.That(cache.Bind(baselineAsset), Is.SameAs(first));
                var second = cache.Bind(assetOnly);
                Assert.That(second, Is.Not.SameAs(first));
                Assert.That(cache.InvalidationCount, Is.EqualTo(1));
                Assert.That(cache.Bind(assetOnly), Is.SameAs(second));
            }

            Assert.That(combinedRun.Snapshot.value.player_motion.target_speed,
                Is.EqualTo(flightRun.Snapshot.value.player_motion.target_speed));
            Assert.That(JsonUtility.ToJson(combinedRun.Snapshot.value.player_pose),
                Is.EqualTo(JsonUtility.ToJson(flightRun.Snapshot.value.player_pose)));
            Assert.That(combinedRun.Snapshot.value.caster_mana,
                Is.EqualTo(fireballRun.Snapshot.value.caster_mana));
            Assert.That(Engagements(combinedRun).Select(row => new[] { row.actor, row.participant }),
                Is.EqualTo(Engagements(enemyRun).Select(row => new[] { row.actor, row.participant })));
            Assert.That(NonEngagementIds(combinedRun), Is.EqualTo(EventIds(baselineRun)));
            Assert.That(combinedAsset.value.content_address,
                Is.EqualTo(assetOnly.value.content_address));

            TestContext.Out.WriteLine(
                "MC_F10_MATRIX " + JsonUtility.ToJson(Project(combinedRun, combinedAsset)));
        }
    }
}
