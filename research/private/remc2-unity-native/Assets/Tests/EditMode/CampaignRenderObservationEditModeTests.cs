using System;
using System.IO;
using System.Linq;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class CampaignRenderObservationEditModeTests
    {
        [Serializable] private sealed class Scenario {
            public GameRuntimeTransport.InstalledPackageResource[] packages;
        }

        [Test]
        public void InstalledObservationKeepsStepRejectionPauseResetAndReplayBound()
        {
            var path = Environment.GetEnvironmentVariable("MC_RENDER_OBSERVATION_SCENARIO");
            if (string.IsNullOrEmpty(path)) Assert.Ignore("Requires explicit current installed package scenario");
            var packages = JsonUtility.FromJson<Scenario>(File.ReadAllText(path)).packages;
            using var session = NativeSession.OpenGameRuntime(packages);
            void Compare() {
                var observedIdentity = session.ObservedCampaignIdentity();
                var freshIdentity = session.GameRuntimeIdentity();
                Assert.That(JsonUtility.ToJson(observedIdentity), Is.EqualTo(JsonUtility.ToJson(freshIdentity)),
                    "metadata must already reflect the completed operation before the explicit fresh read");
                var full = session.GameRuntimeCampaignSnapshot();
                var render = session.GameRuntimeCampaignRenderObservation();
                Assert.That(render.identity.generation, Is.EqualTo(full.identity.generation));
                Assert.That(render.identity.tick, Is.EqualTo(full.identity.tick));
                Assert.That(JsonUtility.ToJson(render.value.player_pose), Is.EqualTo(JsonUtility.ToJson(full.value.player_pose)));
                Assert.That(JsonUtility.ToJson(render.value.source_camera_pose), Is.EqualTo(JsonUtility.ToJson(full.value.source_camera_pose)));
                Assert.That(JsonUtility.ToJson(render.value.player_vitals), Is.EqualTo(JsonUtility.ToJson(full.value.player_vitals)));
                Assert.That(render.value.effect_head, Is.EqualTo(full.value.effect_head));
                Assert.That(JsonUtility.ToJson(render.value.player_motion), Is.EqualTo(JsonUtility.ToJson(full.value.player_motion)));
                Assert.That(JsonUtility.ToJson(render.value.objective_progress), Is.EqualTo(JsonUtility.ToJson(full.value.objective_progress)));
                Assert.That(render.value.objective_secret_announcement_record,
                    Is.EqualTo(full.value.objective_secret_announcement_record));
                Assert.That(JsonUtility.ToJson(render.value.source_scoreboard), Is.EqualTo(JsonUtility.ToJson(full.value.source_scoreboard)));
                Assert.That(render.value.player_spells.Select(JsonUtility.ToJson),
                    Is.EqualTo(full.value.player_spells.Select(JsonUtility.ToJson)));
                Assert.That(render.value.presentation_entities.Select(JsonUtility.ToJson),
                    Is.EqualTo(full.value.presentation_entities.Select(JsonUtility.ToJson)));
                Assert.That(render.value.participant_notifications.Select(JsonUtility.ToJson),
                    Is.EqualTo(full.value.participant_notifications.Select(JsonUtility.ToJson)));
                Assert.That(session.GameRuntimeCampaignSnapshot().value.state_hash, Is.EqualTo(full.value.state_hash));
            }
            Compare();
            var before = session.GameRuntimeCampaignSnapshot();
            var locked = before.value.player_spells.First(spell => !spell.acquired).id;
            var rejected = Assert.Throws<EngineSessionTransport.CommandException>(() =>
                session.GameRuntimeCampaignRenderStep(0, 0, 0, 0, 0, selectSpell: locked));
            Assert.That(rejected.Status, Is.EqualTo(NativeSim.ValidationFailed));
            Assert.That(rejected.CommandSubmission, Is.True);
            Assert.That(session.GameRuntimeCampaignSnapshot().value.state_hash, Is.EqualTo(before.value.state_hash));
            session.GameRuntimeCampaignRenderStep(0, 0, 1, 0, 0, elapsedCentiseconds: 3);
            Compare();
            Assert.That(ulong.Parse(session.GameRuntimeCampaignRenderObservation().identity.tick),
                Is.EqualTo(ulong.Parse(before.identity.tick) + 1));
            session.GameRuntimePause();
            session.GameRuntimeCampaignPausedRenderFrame(4);
            Compare();
            var save = session.GameRuntimeSaveReplay();
            using (var restored = NativeSession.OpenGameRuntimeReplay(save, packages)) {
                Assert.That(restored.GameRuntimeCampaignRenderObservation().value.tick,
                    Is.EqualTo(session.GameRuntimeCampaignRenderObservation().value.tick));
                Assert.That(restored.GameRuntimeCampaignSnapshot().value.state_hash,
                    Is.EqualTo(session.GameRuntimeCampaignSnapshot().value.state_hash));
            }
            session.GameRuntimeResume();
            session.GameRuntimeCampaignReset();
            Compare();
            Assert.That(session.GameRuntimeCampaignRenderObservation().identity.generation,
                Is.Not.EqualTo(before.identity.generation));
        }

        [Test]
        public void InstalledBinaryPacketRejectsCorruptionAndKeepsLargeCountersExact()
        {
            var path = Environment.GetEnvironmentVariable("MC_RENDER_OBSERVATION_SCENARIO");
            if (string.IsNullOrEmpty(path)) Assert.Ignore("Requires explicit current installed package scenario");
            var packages = JsonUtility.FromJson<Scenario>(File.ReadAllText(path)).packages;
            using var session = NativeSession.OpenGameRuntime(packages);
            var identity = session.GameRuntimeIdentity();
            var bytes = new byte[1024 * 1024];
            Assert.That(NativeSim.mc_game_runtime_render_observation_copy_v1(session.Handle,
                ulong.Parse(identity.generation), ulong.Parse(identity.session.generation),
                ulong.Parse(identity.session.tick), bytes, (uint)bytes.Length, out var length), Is.EqualTo(NativeSim.Ok));
            var publication = CampaignRenderTransport.Decode(bytes, (int)length);
            Assert.That(publication.RuntimeId.ToString(), Is.EqualTo(identity.runtime_id));
            Assert.That(publication.Mode, Is.EqualTo(identity.mode));
            Assert.That(publication.Observation.identity.tick, Is.EqualTo(identity.session.tick));
            var originalHash = session.GameRuntimeCampaignSnapshot().value.state_hash;
            foreach (var truncated in new[] { 0, 28, 29, (int)length / 2, (int)length - 1 })
                Assert.Throws<InvalidDataException>(() => CampaignRenderTransport.Decode(bytes, truncated));
            var bad = (byte[])bytes.Clone();
            bad[4] = 1;
            Assert.Throws<InvalidDataException>(() => CampaignRenderTransport.Decode(bad, (int)length));
            bad[4] = 2; bad[28] = 2;
            Assert.Throws<InvalidDataException>(() => CampaignRenderTransport.Decode(bad, (int)length));
            bad[28] = 0;
            for (var i = 12; i < 20; i++) bad[i] = 255;
            Assert.That(CampaignRenderTransport.Decode(bad, (int)length).RuntimeId, Is.EqualTo(ulong.MaxValue));
            Assert.That(session.GameRuntimeCampaignSnapshot().value.state_hash, Is.EqualTo(originalHash),
                "reading and rejecting malformed copies cannot mutate gameplay");
            Assert.That(session.GameRuntimeFinishMissionIfReady(), Is.Null,
                "ordinary no-op completion poll must retain the exact identity");
            Assert.That(JsonUtility.ToJson(session.ObservedCampaignIdentity()),
                Is.EqualTo(JsonUtility.ToJson(session.GameRuntimeIdentity())));
        }

        private static EngineSessionTransport.CampaignRenderObservation Observation()
            => new EngineSessionTransport.CampaignRenderObservation {
                contract = "carpet-flight-campaign-render-observation/v1",
                terrain_mutation_sequence = "18446744073709551615",
                identity = new EngineSessionTransport.Identity {
                    engine_contract = "carpet-flight-game-session/v6",
                    package_graph_hash = "sha256:package", definition_hash = "sha256:definition",
                    definition_revision = "1", session_id = "8", generation = "7", tick = "42"
                },
                value = new EngineSessionTransport.CampaignSnapshotValue {
                    tick = "42", effect_head = "100", player_pose = new EngineSessionTransport.Pose { x = 10, y = 20 },
                    player_motion = new EngineSessionTransport.Motion { actual_speed = 80 },
                    player_vitals = new EngineSessionTransport.PlayerVitals { life = 80, max_life = 100, mana = 60, max_mana = 100 },
                    source_camera_pose = new EngineSessionTransport.CameraPose { x = 11, y = 21 }, source_camera_fov = 128,
                    player_spells = new[] { new EngineSessionTransport.SpellObservation {
                        id = "spell.fireball", display_name = "Fireball", acquired = true,
                        selected = true, hand = "left", availability = "ready"
                    } },
                    objective_progress = new EngineSessionTransport.ObjectiveProgress { total_count = 1 },
                    presentation_entities = Array.Empty<EngineSessionTransport.PresentationEntity>(),
                    state_hash = "0000000000000001", terrain_hash = "0000000000000002",
                    effect_journal_sha256 = new string('a', 64)
                }
            };

        [Test]
        public void FullSnapshotKeepsInheritedRenderFieldsAndDiagnosticHashesInUnityJson()
        {
            var original = Observation();
            var full = new EngineSessionTransport.CampaignSnapshotObservation {
                identity = original.identity, value = (EngineSessionTransport.CampaignSnapshotValue)original.value
            };
            var restored = JsonUtility.FromJson<EngineSessionTransport.CampaignSnapshotObservation>(JsonUtility.ToJson(full));
            EngineSessionTransport.ValidateCampaignSnapshot(restored.value, restored.identity);
            Assert.That(restored.value.player_pose.x, Is.EqualTo(10));
            Assert.That(restored.value.source_camera_pose.y, Is.EqualTo(21));
            Assert.That(restored.value.player_spells.Single().selected, Is.True);
            Assert.That(restored.value.state_hash, Is.EqualTo(full.value.state_hash));
            Assert.That(restored.value.effect_journal_sha256, Is.EqualTo(full.value.effect_journal_sha256));
        }

        [Test]
        public void CompactValueDeserializesRenderFieldsWithoutExposingDiagnosticHashes()
        {
            var original = Observation();
            // JsonUtility serializes the declared base type as the render view.
            var restored = JsonUtility.FromJson<EngineSessionTransport.CampaignRenderObservation>(JsonUtility.ToJson(original));
            EngineSessionTransport.ValidateCampaignRenderObservation(restored);
            Assert.That(restored.terrain_mutation_sequence, Is.EqualTo("18446744073709551615"));
            Assert.That(restored.value.player_vitals.mana, Is.EqualTo(60));
            Assert.That(restored.value.player_motion.actual_speed, Is.EqualTo(80));
            Assert.That(typeof(EngineSessionTransport.CampaignRenderValue).GetField("state_hash"), Is.Null);
        }

        [Test]
        public void CompactReadRejectsWrongContractTickAndMalformedRevision()
        {
            var observation = Observation();
            observation.contract = "campaign_snapshot/v5";
            Assert.Throws<InvalidOperationException>(() => EngineSessionTransport.ValidateCampaignRenderObservation(observation));
            observation.contract = "carpet-flight-campaign-render-observation/v1";
            observation.value.tick = "41";
            Assert.Throws<InvalidOperationException>(() => EngineSessionTransport.ValidateCampaignRenderObservation(observation));
            observation.value.tick = "42";
            observation.terrain_mutation_sequence = "18446744073709551616";
            Assert.Throws<InvalidOperationException>(() => EngineSessionTransport.ValidateCampaignRenderObservation(observation));
        }

        [Test]
        public void ObservationValidationFailureIsNotACommandSubmissionRejection()
        {
            int Read(IntPtr buffer, uint capacity, out uint length) {
                length = 0;
                return NativeSim.ValidationFailed;
            }
            var error = Assert.Throws<EngineSessionTransport.CommandException>(() =>
                EngineSessionTransport.ReadJson<EngineSessionTransport.CampaignRenderObservation>(Read, "observation failure"));
            Assert.That(error.Status, Is.EqualTo(NativeSim.ValidationFailed));
            Assert.That(error.CommandSubmission, Is.False, "a committed step must not be retried after a failed read");
        }
    }
}
