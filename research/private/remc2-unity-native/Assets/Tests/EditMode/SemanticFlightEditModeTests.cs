using System;
using System.IO;
using System.Linq;
using NUnit.Framework;
using UnityEngine;

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
        public void PreferenceKeyCaptureConsumesTextInputAndPersistsOnlyConfirmedBindings()
        {
            var profile = Path.Combine(Path.GetTempPath(), $"mc-key-capture-{Guid.NewGuid():N}");
            try
            {
                using (var owner = new GameRuntimeTransport())
                {
                    var preferences = new PlayerPreferencesPresenter(json =>
                        GameRuntimeTransport.PreferencesCommand(owner.Handle, owner.IdentitySnapshot(), json), profile);
                    preferences.Open();
                    preferences.BeginKeyCapture(0);
                    var key = Event.KeyboardEvent("w");
                    Assert.That(preferences.CaptureKey(key), Is.True);
                    Assert.That(key.type, Is.EqualTo(EventType.Used),
                        "the name field must not receive a captured character");
                    Assert.That(preferences.Current.preferences.keys[0], Is.EqualTo("UpArrow"),
                        "capture edits the pending preferences, not live controls");
                    Assert.That(preferences.Apply(), Is.True);
                    Assert.That(preferences.Current.preferences.keys[0], Is.EqualTo("W"));
                    Assert.That(preferences.Current.preferences.player_name, Is.EqualTo("PLAYER"));
                    var confirmed = File.ReadAllBytes(Path.Combine(profile, "player-preferences.json"));

                    preferences.Open();
                    preferences.BeginKeyCapture(0);
                    Assert.That(preferences.CaptureKey(Event.KeyboardEvent("s")), Is.True);
                    preferences.Cancel();
                    Assert.That(preferences.Current.preferences.keys[0], Is.EqualTo("W"));
                    Assert.That(File.ReadAllBytes(Path.Combine(profile, "player-preferences.json")),
                        Is.EqualTo(confirmed));

                    preferences.Open();
                    preferences.BeginKeyCapture(0);
                    var recoveryKey = Event.KeyboardEvent("f10");
                    Assert.That(recoveryKey.keyCode, Is.EqualTo(KeyCode.F10));
                    Assert.That(preferences.CaptureKey(recoveryKey), Is.True);
                    Assert.That(preferences.Apply(), Is.False, "reserved recovery keys remain unavailable");
                    Assert.That(preferences.IsOpen, Is.True, "an invalid edit can still be cancelled");
                    preferences.Cancel();
                    Assert.That(preferences.Current.preferences.keys[0], Is.EqualTo("W"));
                }
                using var coldOwner = new GameRuntimeTransport();
                var coldPreferences = new PlayerPreferencesPresenter(json =>
                    GameRuntimeTransport.PreferencesCommand(coldOwner.Handle, coldOwner.IdentitySnapshot(), json), profile);
                Assert.That(coldPreferences.Current.load_status, Is.EqualTo("loaded"));
                Assert.That(coldPreferences.Current.preferences.keys[0], Is.EqualTo("W"));
                Assert.That(coldPreferences.Current.preferences.player_name, Is.EqualTo("PLAYER"));
            }
            finally
            {
                if (Directory.Exists(profile)) Directory.Delete(profile, true);
            }
        }

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
