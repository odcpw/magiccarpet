using System;
using System.IO;
using System.Text;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class GameRuntimeEditModeTests
    {
        private static string PrivateRoot =>
            Path.GetFullPath(Path.Combine(Application.dataPath, "../.."));

        private static string LevelOnePackage =>
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_LEVEL1_PACKAGE_V1") ??
            Path.Combine(PrivateRoot, "generated/mc-cf15-level1-package-current");

        private static string LevelTwoPackage =>
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_PACKAGE_V1") ??
            Path.Combine(
                PrivateRoot,
                "generated/mc-cf02-campaign-package-v1-a/qualified-level2");

        [Test]
        [Timeout(180000)]
        public void RustGameRuntimeOwnsTitleLoadingAndRealLevelOnePlaying()
        {
            var runtime = new GameRuntimeTransport();
            try
            {
                Assert.That(
                    runtime.NegotiatedContract.runtime_contract,
                    Is.EqualTo(GameRuntimeTransport.RuntimeContract));
                var title = runtime.IdentitySnapshot();
                Assert.That(title.mode, Is.EqualTo(GameRuntimeTransport.TitleMode));
                Assert.That(title.generation, Is.EqualTo("1"));
                Assert.That(title.pending_definition.definition_hash, Is.Null.Or.Empty);
                Assert.That(title.session.session_id, Is.Null.Or.Empty);

                title.mode = GameRuntimeTransport.PlayingMode;
                Assert.That(
                    runtime.IdentitySnapshot().mode,
                    Is.EqualTo(GameRuntimeTransport.TitleMode));

                var levelOne = Encoding.UTF8.GetBytes(LevelOnePackage);
                Assert.That(
                    NativeSim.mc_game_runtime_new_game_v1(
                        runtime.Handle,
                        2,
                        levelOne,
                        checked((uint)levelOne.Length),
                        Array.Empty<byte>(),
                        0),
                    Is.EqualTo(NativeSim.GenerationMismatch));
                Assert.That(
                    runtime.IdentitySnapshot().mode,
                    Is.EqualTo(GameRuntimeTransport.TitleMode));

                var loading = runtime.NewGame(LevelOnePackage);
                Assert.That(loading.mode, Is.EqualTo(GameRuntimeTransport.LoadingMode));
                Assert.That(loading.generation, Is.EqualTo("2"));
                Assert.That(loading.pending_definition.definition_hash, Is.Not.Empty);
                Assert.That(loading.session.session_id, Is.Null.Or.Empty);

                var repeatedNewGame = Assert.Throws<GameRuntimeTransport.TransitionException>(
                    () => runtime.NewGame(LevelTwoPackage));
                Assert.That(repeatedNewGame.Status, Is.EqualTo(NativeSim.InvalidArgument));
                Assert.That(runtime.IdentitySnapshot().mode, Is.EqualTo(loading.mode));
                Assert.That(
                    runtime.Current.pending_definition.definition_hash,
                    Is.EqualTo(loading.pending_definition.definition_hash));

                var playing = runtime.OpenLevel();
                Assert.That(playing.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(playing.generation, Is.EqualTo("3"));
                Assert.That(playing.pending_definition.definition_hash, Is.Null.Or.Empty);
                Assert.That(playing.session.session_id, Is.EqualTo(runtime.Handle.ToString()));
                Assert.That(playing.session.generation, Is.EqualTo("1"));
                Assert.That(playing.session.tick, Is.EqualTo("0"));

                var snapshot = runtime.CampaignSnapshot();
                Assert.That(snapshot.identity.session_id, Is.EqualTo(playing.session.session_id));
                Assert.That(snapshot.identity.tick, Is.EqualTo("0"));
                for (var poll = 0; poll < 5; poll++)
                {
                    var observed = runtime.IdentitySnapshot();
                    Assert.That(observed.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                    Assert.That(observed.generation, Is.EqualTo("3"));
                    Assert.That(observed.session.tick, Is.EqualTo("0"));
                }

                runtime.Dispose();
                runtime.Dispose();
                Assert.Throws<ObjectDisposedException>(() => runtime.IdentitySnapshot());
            }
            finally
            {
                runtime.Dispose();
            }
        }

        [Test]
        [Timeout(180000)]
        public void RustGameRuntimePauseResumeFreezesTheSameLevelOneSession()
        {
            using var runtime = new GameRuntimeTransport();
            runtime.NewGame(LevelOnePackage);
            var playing = runtime.OpenLevel();
            var before = runtime.CampaignSnapshot();

            var paused = runtime.Pause();
            Assert.That(paused.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
            Assert.That(paused.generation, Is.EqualTo("4"));
            Assert.That(paused.session.session_id, Is.EqualTo(playing.session.session_id));
            Assert.That(paused.session.generation, Is.EqualTo("2"));
            Assert.That(paused.session.tick, Is.EqualTo("0"));
            Assert.That(
                paused.session.definition_hash,
                Is.EqualTo(playing.session.definition_hash));

            paused.mode = GameRuntimeTransport.PlayingMode;
            Assert.That(
                runtime.IdentitySnapshot().mode,
                Is.EqualTo(GameRuntimeTransport.PausedMode));

            var command = Encoding.UTF8.GetBytes("{}");
            Assert.That(
                NativeSim.mc_game_session_step_json_v1(
                    runtime.Handle,
                    2,
                    0,
                    command,
                    checked((uint)command.Length)),
                Is.EqualTo(NativeSim.InvalidArgument));
            Assert.That(
                NativeSim.mc_game_session_reset_v1(runtime.Handle, 2, 0),
                Is.EqualTo(NativeSim.InvalidArgument));
            var frozen = runtime.CampaignSnapshot();
            Assert.That(frozen.identity.tick, Is.EqualTo(before.identity.tick));
            Assert.That(frozen.value.state_hash, Is.EqualTo(before.value.state_hash));
            Assert.That(frozen.value.terrain_hash, Is.EqualTo(before.value.terrain_hash));
            Assert.That(
                frozen.value.effect_journal_sha256,
                Is.EqualTo(before.value.effect_journal_sha256));
            Assert.That(frozen.value.player_pose.x, Is.EqualTo(before.value.player_pose.x));
            Assert.That(frozen.value.player_pose.y, Is.EqualTo(before.value.player_pose.y));
            Assert.That(frozen.value.player_pose.z, Is.EqualTo(before.value.player_pose.z));

            var resumed = runtime.Resume();
            Assert.That(resumed.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
            Assert.That(resumed.generation, Is.EqualTo("5"));
            Assert.That(resumed.session.generation, Is.EqualTo("3"));
            Assert.That(resumed.session.tick, Is.EqualTo("0"));
            Assert.That(
                NativeSim.mc_game_session_step_json_v1(
                    runtime.Handle,
                    2,
                    0,
                    command,
                    checked((uint)command.Length)),
                Is.EqualTo(NativeSim.GenerationMismatch));
            Assert.That(
                NativeSim.mc_game_session_step_json_v1(
                    runtime.Handle,
                    3,
                    0,
                    command,
                    checked((uint)command.Length)),
                Is.EqualTo(NativeSim.Ok));
            Assert.That(runtime.IdentitySnapshot().session.tick, Is.EqualTo("1"));

            // No-Claim: this is a quiescent pause boundary only. There is no
            // paused_frame transport or MC2 pre-gate RNG/objective/presentation
            // service, replay-visible paused time, or production pause UI.
        }
    }
}
