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
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_LEVEL1_PACKAGE") ??
            Path.Combine(PrivateRoot, "generated/mc-cf15-level1-package-v3-current");

        private static string LevelTwoPackage =>
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_PACKAGE") ??
            Path.Combine(
                PrivateRoot,
                "generated/mc-cf02-campaign-package-v2-b/qualified-level2");

        private static string LevelThreePackage =>
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_PACKAGE_LEVEL3_V1") ??
            Path.Combine(
                PrivateRoot,
                "generated/mc-cf14-campaign-package-v1-a/qualified-level3");

        private static string LevelTwoOwnerCache =>
            Path.Combine(PrivateRoot, "generated/level2-day-owner-cache");

        private static string LevelOneOwnerCache =>
            Path.Combine(
                PrivateRoot,
                "generated/level1-unity-asset-cache-plan29-final");

        private static string OpenedLevelTwoReplay =>
            Path.Combine(PrivateRoot, "generated/opened-l2-runtime-replay-v5.json");

        private static GameRuntimeTransport.InstalledPackageResource Package(string path)
        {
            return new GameRuntimeTransport.InstalledPackageResource
            {
                package_dir = path,
                owner_cache_path = null
            };
        }

        private static GameRuntimeTransport.InstalledPackageResource[] ResolverPackages()
        {
            return new[]
            {
                Package(LevelThreePackage),
                Package(LevelOnePackage),
                Package(LevelTwoPackage)
            };
        }

        private static string[] RuntimeArguments(params string[] packages)
        {
            var arguments = new string[1 + packages.Length * 2];
            arguments[0] = "-mc-game-runtime";
            for (var index = 0; index < packages.Length; index++)
            {
                arguments[1 + index * 2] = "-mc-runtime-package";
                arguments[2 + index * 2] = packages[index];
            }
            return arguments;
        }

        private static string[] RuntimeReplayArguments(
            string replayPath,
            params string[] packages)
        {
            var arguments = RuntimeArguments(packages);
            Array.Resize(ref arguments, arguments.Length + 2);
            arguments[arguments.Length - 2] = "-mc-runtime-replay-load";
            arguments[arguments.Length - 1] = replayPath;
            return arguments;
        }

        private static string[] OpenedLevelTwoReplayArguments()
        {
            return new[]
            {
                "-mc-game-runtime",
                "-mc-runtime-package", LevelThreePackage,
                "-mc-runtime-package", LevelOnePackage,
                "-mc-runtime-package", LevelTwoPackage,
                "-mc-runtime-presentation-cache", LevelTwoOwnerCache,
                "-mc-runtime-replay-load", OpenedLevelTwoReplay
            };
        }

        private static string[] RuntimeReplaySaveArguments(
            string replayPath,
            params string[] packages)
        {
            var arguments = RuntimeArguments(packages);
            Array.Resize(ref arguments, arguments.Length + 2);
            arguments[arguments.Length - 2] = "-mc-runtime-replay-save";
            arguments[arguments.Length - 1] = replayPath;
            return arguments;
        }

        private static byte[] PackageSetJson(
            params GameRuntimeTransport.InstalledPackageResource[] packages)
        {
            return Encoding.UTF8.GetBytes(JsonUtility.ToJson(
                new GameRuntimeTransport.InstalledPackageSetRequest
                {
                    request_contract = GameRuntimeTransport.InstalledPackageSetContract,
                    packages = packages
                }));
        }

        private static void AssertSameCampaignState(
            EngineSessionTransport.CampaignSnapshotObservation expected,
            EngineSessionTransport.CampaignSnapshotObservation actual)
        {
            Assert.That(actual.value.effect_head, Is.EqualTo(expected.value.effect_head));
            Assert.That(actual.value.state_hash, Is.EqualTo(expected.value.state_hash));
            Assert.That(actual.value.terrain_hash, Is.EqualTo(expected.value.terrain_hash));
            Assert.That(
                actual.value.effect_journal_sha256,
                Is.EqualTo(expected.value.effect_journal_sha256));
            Assert.That(actual.value.player_pose.x, Is.EqualTo(expected.value.player_pose.x));
            Assert.That(actual.value.player_pose.y, Is.EqualTo(expected.value.player_pose.y));
            Assert.That(actual.value.player_pose.z, Is.EqualTo(expected.value.player_pose.z));
            Assert.That(actual.value.player_pose.yaw, Is.EqualTo(expected.value.player_pose.yaw));
            Assert.That(
                actual.value.player_pose.display_pitch,
                Is.EqualTo(expected.value.player_pose.display_pitch));
            Assert.That(
                actual.value.player_pose.effective_pitch,
                Is.EqualTo(expected.value.player_pose.effective_pitch));
            Assert.That(actual.value.player_pose.roll, Is.EqualTo(expected.value.player_pose.roll));
            Assert.That(
                actual.value.player_motion.target_speed,
                Is.EqualTo(expected.value.player_motion.target_speed));
            Assert.That(
                actual.value.player_motion.actual_speed,
                Is.EqualTo(expected.value.player_motion.actual_speed));
            Assert.That(
                actual.value.player_motion.strafe_speed,
                Is.EqualTo(expected.value.player_motion.strafe_speed));
            Assert.That(
                actual.value.player_vitals.life,
                Is.EqualTo(expected.value.player_vitals.life));
            Assert.That(
                actual.value.player_vitals.max_life,
                Is.EqualTo(expected.value.player_vitals.max_life));
            Assert.That(
                actual.value.player_vitals.mana,
                Is.EqualTo(expected.value.player_vitals.mana));
            Assert.That(
                actual.value.player_vitals.max_mana,
                Is.EqualTo(expected.value.player_vitals.max_mana));
            Assert.That(
                actual.value.objective_progress.current_objective,
                Is.EqualTo(expected.value.objective_progress.current_objective));
            Assert.That(
                actual.value.objective_progress.completed_count,
                Is.EqualTo(expected.value.objective_progress.completed_count));
            Assert.That(
                actual.value.objective_progress.total_count,
                Is.EqualTo(expected.value.objective_progress.total_count));
            Assert.That(
                actual.value.objective_progress.all_objectives_satisfied,
                Is.EqualTo(expected.value.objective_progress.all_objectives_satisfied));
        }

        private static int SignedAngle11(short value)
        {
            var wrapped = value & 0x07ff;
            return wrapped >= 1024 ? wrapped - 2048 : wrapped;
        }

        private static Quaternion ExpectedRuntimeCameraRotation(
            EngineSessionTransport.Pose pose)
        {
            return Quaternion.Euler(
                -SignedAngle11(pose.display_pitch) * (360f / 2048f),
                180f - pose.yaw * (360f / 2048f),
                -SignedAngle11(pose.roll) * (360f / 2048f));
        }

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
                Assert.That(
                    runtime.NegotiatedContract.installed_package_set.request_contract,
                    Is.EqualTo(GameRuntimeTransport.InstalledPackageSetContract));
                var title = runtime.IdentitySnapshot();
                Assert.That(title.mode, Is.EqualTo(GameRuntimeTransport.TitleMode));
                Assert.That(title.generation, Is.EqualTo("1"));
                Assert.That(title.pending_definition.definition_hash, Is.Null.Or.Empty);
                Assert.That(
                    title.pending_definition.runtime_flow?.game_id,
                    Is.Null.Or.Empty);
                Assert.That(title.active_runtime_flow?.game_id, Is.Null.Or.Empty);
                Assert.That(title.session.session_id, Is.Null.Or.Empty);

                title.pending_definition.runtime_flow =
                    new GameRuntimeTransport.RuntimeFlowDefinition
                    {
                        bound_level_id = "level.partial.binding"
                    };
                Assert.Throws<InvalidOperationException>(() =>
                    GameRuntimeTransport.ValidateIdentity(title, runtime.Handle));
                title = runtime.IdentitySnapshot();

                title.mode = GameRuntimeTransport.PlayingMode;
                Assert.That(
                    runtime.IdentitySnapshot().mode,
                    Is.EqualTo(GameRuntimeTransport.TitleMode));

                var packageSet = PackageSetJson(ResolverPackages());
                Assert.That(
                    NativeSim.mc_game_runtime_bootstrap_package_set_json_v1(
                        runtime.Handle,
                        2,
                        packageSet,
                        checked((uint)packageSet.Length)),
                    Is.EqualTo(NativeSim.GenerationMismatch));
                Assert.That(
                    runtime.IdentitySnapshot().mode,
                    Is.EqualTo(GameRuntimeTransport.TitleMode));

                using (var laterRuntime = new GameRuntimeTransport())
                {
                    var later = Assert.Throws<GameRuntimeTransport.TransitionException>(
                        () => laterRuntime.BootstrapPackageSet(
                            Package(LevelTwoPackage),
                            Package(LevelOnePackage)));
                    Assert.That(later.Status, Is.EqualTo(NativeSim.ValidationFailed));
                    Assert.That(
                        laterRuntime.IdentitySnapshot().mode,
                        Is.EqualTo(GameRuntimeTransport.TitleMode));
                }

                var loading = runtime.BootstrapPackageSet(ResolverPackages());
                Assert.That(loading.mode, Is.EqualTo(GameRuntimeTransport.LoadingMode));
                Assert.That(loading.generation, Is.EqualTo("2"));
                Assert.That(loading.pending_definition.definition_hash, Is.Not.Empty);
                Assert.That(
                    loading.pending_definition.runtime_flow.game_id,
                    Is.EqualTo("game.magic_carpet_2"));
                Assert.That(
                    loading.pending_definition.runtime_flow.bound_level_id,
                    Is.EqualTo("level.magic_carpet_2.night_spire"));
                Assert.That(
                    loading.pending_definition.runtime_flow.entries.Length,
                    Is.EqualTo(3));
                Assert.That(loading.active_runtime_flow?.game_id, Is.Null.Or.Empty);
                Assert.That(loading.session.session_id, Is.Null.Or.Empty);

                loading.active_runtime_flow = new GameRuntimeTransport.RuntimeFlowDefinition
                {
                    bound_level_id = "level.partial.binding"
                };
                Assert.Throws<InvalidOperationException>(() =>
                    GameRuntimeTransport.ValidateIdentity(loading, runtime.Handle));
                loading = runtime.IdentitySnapshot();

                var repeatedNewGame = Assert.Throws<GameRuntimeTransport.TransitionException>(
                    () => runtime.BootstrapPackageSet(ResolverPackages()));
                Assert.That(repeatedNewGame.Status, Is.EqualTo(NativeSim.InvalidArgument));
                Assert.That(runtime.IdentitySnapshot().mode, Is.EqualTo(loading.mode));
                Assert.That(
                    runtime.Current.pending_definition.definition_hash,
                    Is.EqualTo(loading.pending_definition.definition_hash));

                var playing = runtime.OpenLevel();
                Assert.That(playing.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(playing.generation, Is.EqualTo("3"));
                Assert.That(playing.pending_definition.definition_hash, Is.Null.Or.Empty);
                Assert.That(
                    playing.pending_definition.runtime_flow?.game_id,
                    Is.Null.Or.Empty);
                Assert.That(
                    playing.active_runtime_flow.game_id,
                    Is.EqualTo("game.magic_carpet_2"));
                Assert.That(
                    playing.active_runtime_flow.bound_level_id,
                    Is.EqualTo("level.magic_carpet_2.night_spire"));
                Assert.That(playing.session.session_id, Is.EqualTo(runtime.Handle.ToString()));
                Assert.That(playing.session.generation, Is.EqualTo("1"));
                Assert.That(playing.session.tick, Is.EqualTo("0"));

                playing.pending_definition.runtime_flow =
                    new GameRuntimeTransport.RuntimeFlowDefinition
                    {
                        bound_level_id = "level.partial.binding"
                    };
                Assert.Throws<InvalidOperationException>(() =>
                    GameRuntimeTransport.ValidateIdentity(playing, runtime.Handle));
                playing = runtime.IdentitySnapshot();

                playing.active_runtime_flow.bound_level_id =
                    "level.magic_carpet_2.payahandra_ruins";
                Assert.That(
                    runtime.IdentitySnapshot().active_runtime_flow.bound_level_id,
                    Is.EqualTo("level.magic_carpet_2.night_spire"));

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

                // No-Claim: this proves complete-set admission and a
                // resolver-derived entry open only. It does not expose
                // completion acceptance, materialize later L2/L3 sessions,
                // discover/install packages, migrate production presentation,
                // or prove replay persistence, results, or shell wiring.
            }
            finally
            {
                runtime.Dispose();
            }
        }

        [Test]
        [Timeout(180000)]
        public void RuntimeCompletionTransitionRejectsInProgressAndStaleGuardsAtomically()
        {
            using (var runtime = new GameRuntimeTransport())
            {
                Assert.That(
                    runtime.NegotiatedContract.completion_transition.receipt_contract,
                    Is.EqualTo(GameRuntimeTransport.CompletionTransitionReceiptContract));
                runtime.BootstrapPackageSet(ResolverPackages());
                var before = runtime.OpenLevel();
                var rejected = Assert.Throws<GameRuntimeTransport.TransitionException>(
                    () => runtime.AcceptCompletionTransition());
                Assert.That(rejected.Status, Is.EqualTo(NativeSim.InvalidArgument));
                var after = runtime.IdentitySnapshot();
                Assert.That(after.mode, Is.EqualTo(before.mode));
                Assert.That(after.generation, Is.EqualTo(before.generation));
                Assert.That(after.session.generation, Is.EqualTo(before.session.generation));
                Assert.That(after.session.tick, Is.EqualTo(before.session.tick));

                Assert.That(
                    NativeSim.mc_game_runtime_accept_transition_v1(
                        runtime.Handle,
                        ulong.Parse(before.generation) - 1,
                        ulong.Parse(before.session.generation),
                        ulong.Parse(before.session.tick)),
                    Is.EqualTo(NativeSim.GenerationMismatch));
                Assert.That(
                    NativeSim.mc_game_runtime_accept_transition_v1(
                        runtime.Handle,
                        ulong.Parse(before.generation),
                        ulong.Parse(before.session.generation) + 1,
                        ulong.Parse(before.session.tick)),
                    Is.EqualTo(NativeSim.GenerationMismatch));
                Assert.That(
                    NativeSim.mc_game_runtime_accept_transition_v1(
                        runtime.Handle,
                        ulong.Parse(before.generation),
                        ulong.Parse(before.session.generation),
                        ulong.Parse(before.session.tick) + 1),
                    Is.EqualTo(NativeSim.InvalidArgument));
                Assert.That(
                    NativeSim.mc_game_runtime_completion_transition_receipt_json_v1(
                        runtime.Handle,
                        IntPtr.Zero,
                        0,
                        out _),
                    Is.EqualTo(NativeSim.InvalidArgument));
                var final = runtime.IdentitySnapshot();
                Assert.That(final.mode, Is.EqualTo(before.mode));
                Assert.That(final.generation, Is.EqualTo(before.generation));
                Assert.That(final.session.generation, Is.EqualTo(before.session.generation));
                Assert.That(final.session.tick, Is.EqualTo(before.session.tick));
            }
        }

        [Test]
        [Timeout(180000)]
        public void OpenedLevelTwoReplayKeepsLivePresentationOnOneRuntimeHandle()
        {
            Assert.That(File.Exists(OpenedLevelTwoReplay), Is.True,
                "focused owner-local opened-Level-2 replay fixture");
            var root = new GameObject("Game Runtime Level 2 Composition Test");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    OpenedLevelTwoReplayArguments());
                var opened = runtime.GameRuntimeProductionIdentity;
                Assert.That(opened.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(
                    opened.active_runtime_flow.bound_level_id,
                    Is.EqualTo("level.magic_carpet_2.payahandra_ruins"));
                var handle = ulong.Parse(opened.runtime_id);
                Assert.That(opened.session.session_id, Is.EqualTo(opened.runtime_id));
                Assert.That(ulong.Parse(opened.session.generation), Is.GreaterThan(0));
                Assert.That(runtime.GameRuntimeLevel2PresentationReady, Is.True);
                Assert.That(runtime.Level2ProductionWorldInfo.externalLevel, Is.EqualTo(2));
                Assert.That(runtime.Level2ProductionDayHudPresenterReady, Is.True);
                Assert.That(runtime.Level2ProductionMinimapProjectionReady, Is.True);

                runtime.QualificationApplyGameRuntimeProductionMovement(
                    accelerate: true,
                    decelerate: false,
                    strafeLeft: false,
                    strafeRight: false);
                var tickBefore = ulong.Parse(opened.session.tick);
                runtime.QualificationDriveGameRuntimeFrames(1);
                var stepped = runtime.GameRuntimeProductionIdentity;
                Assert.That(ulong.Parse(stepped.session.tick), Is.EqualTo(tickBefore + 1));
                Assert.That(runtime.GameRuntimeLevel2PresentationReady, Is.True);
                Assert.That(runtime.Level2ProductionDayHud.tick, Is.EqualTo(tickBefore + 1));

                Assert.That(
                    NativeSim.mc_game_runtime_accept_transition_v1(
                        handle,
                        ulong.Parse(opened.generation) - 1,
                        ulong.Parse(opened.session.generation),
                        ulong.Parse(opened.session.tick)),
                    Is.EqualTo(NativeSim.GenerationMismatch));
                Assert.That(
                    runtime.GameRuntimeProductionIdentity.active_runtime_flow.bound_level_id,
                    Is.EqualTo("level.magic_carpet_2.payahandra_ruins"));
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void ProductionLoadingGateWaitsForVisibleFrameBeforeRealRustOpen()
        {
            var root = new GameObject("Game Runtime Loading Gate Test");
            try
            {
                var cache = OwnerAssetCache.OpenAndValidate(
                    LevelOneOwnerCache,
                    MagicCarpetRuntime.ExpectedPackageHash,
                    MagicCarpetRuntime.ExpectedOwnerAssetClosureHash);
                using var presenter = new CampaignTransitionPresenter(
                    root.transform,
                    cache);
                using var transport = new GameRuntimeTransport();
                var gate = new MagicCarpetRuntime.GameRuntimeLoadingPresentationGate();

                var title = transport.IdentitySnapshot();
                presenter.Present(title);
                Assert.That(presenter.IsVisible, Is.False);

                var loading = transport.BootstrapPackageSet(ResolverPackages());
                GameRuntimeTransport.ValidateIdentity(loading, transport.Handle);
                Assert.That(loading.mode, Is.EqualTo(GameRuntimeTransport.LoadingMode));
                Assert.That(loading.session.session_id, Is.Null.Or.Empty);
                presenter.Present(loading);
                gate.Begin(
                    loading,
                    loading.pending_definition.runtime_flow.bound_level_id);
                Assert.That(presenter.IsVisible, Is.True);
                Assert.That(
                    presenter.IndicesSha256,
                    Is.EqualTo(
                        "fc27e10c722725e5d4e99ecc01a5bf1f0ff4269e9ff766fdbbad7f424860cc59"));
                Assert.That(
                    presenter.PaletteRgb6Sha256,
                    Is.EqualTo(
                        "7ced7d7bca9414b2890dc086c90efabf424de5694f140262f058ee175f0a1797"));
                Assert.That(
                    gate.TryClaimOpen(loading, presenter.IsVisible),
                    Is.False,
                    "fixed updates cannot skip the presentation-frame boundary");
                Assert.That(
                    gate.TryClaimOpen(transport.IdentitySnapshot(), presenter.IsVisible),
                    Is.False,
                    "repeated Loading polling neither opens nor duplicates state");

                gate.Observe(transport.IdentitySnapshot(), presenter.IsVisible);
                Assert.That(
                    gate.TryClaimOpen(transport.IdentitySnapshot(), presenter.IsVisible),
                    Is.True);
                var opened = transport.OpenLevel();
                GameRuntimeTransport.ValidateIdentity(opened, transport.Handle);
                presenter.Present(opened);
                gate.Complete(opened);
                Assert.That(opened.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(opened.runtime_id, Is.EqualTo(loading.runtime_id));
                Assert.That(opened.session.session_id,
                    Is.EqualTo(transport.Handle.ToString()));
                Assert.That(opened.session.tick, Is.EqualTo("0"));
                Assert.That(opened.active_runtime_flow.bound_level_id,
                    Is.EqualTo(loading.pending_definition.runtime_flow.bound_level_id));
                Assert.That(presenter.IsVisible, Is.False);
                Assert.That(gate.IsPending, Is.False);
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void RuntimeReplaySaveRestoresRealLevelOneIntoFreshManagedOwner()
        {
            using (var source = new GameRuntimeTransport())
            {
                source.BootstrapPackageSet(ResolverPackages());
                source.OpenLevel();
                source.CampaignStep(0, 0, 0, 0, 0);
                source.CampaignStep(0, 3, 1, 0, 0);
                source.CampaignStep(0, 0, 0, 64, unchecked((ushort)-32));
                var sourceIdentity = source.Pause();
                var sourceSnapshot = source.CampaignSnapshot();
                Assert.That(sourceIdentity.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                Assert.That(sourceIdentity.session.tick, Is.EqualTo("3"));

                var save = source.SaveReplay();
                Assert.That(save, Is.Not.Empty);
                Assert.That(save.Length, Is.LessThanOrEqualTo(
                    source.NegotiatedContract.limits.maximum_replay_save_json_bytes));
                var saveJson = Encoding.UTF8.GetString(save);
                Assert.That(saveJson, Does.Contain(GameRuntimeTransport.ReplaySaveContract));
                Assert.That(source.IdentitySnapshot().session.tick, Is.EqualTo("3"));
                AssertSameCampaignState(sourceSnapshot, source.CampaignSnapshot());

                using (var staleTarget = new GameRuntimeTransport())
                {
                    var staleSave = Encoding.UTF8.GetBytes(saveJson.Replace(
                        GameRuntimeTransport.ReplaySaveContract,
                        "carpet-flight-mc2-campaign-replay-save/v4"));
                    Assert.That(staleSave, Is.Not.EqualTo(save));
                    var staleTitle = staleTarget.IdentitySnapshot();
                    var stale = Assert.Throws<GameRuntimeTransport.TransitionException>(() =>
                        staleTarget.LoadReplay(staleSave, ResolverPackages()));
                    Assert.That(stale.Status, Is.EqualTo(NativeSim.ValidationFailed));
                    var afterStale = staleTarget.IdentitySnapshot();
                    Assert.That(afterStale.mode, Is.EqualTo(GameRuntimeTransport.TitleMode));
                    Assert.That(afterStale.generation, Is.EqualTo(staleTitle.generation));
                }

                using (var restored = new GameRuntimeTransport())
                {
                    var playing = restored.LoadReplay(save, ResolverPackages());
                    Assert.That(playing.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                    Assert.That(playing.generation, Is.EqualTo("2"));
                    Assert.That(playing.session.session_id, Is.EqualTo(restored.Handle.ToString()));
                    Assert.That(playing.session.generation, Is.EqualTo("1"));
                    Assert.That(playing.session.tick, Is.EqualTo("3"));
                    Assert.That(
                        playing.session.package_graph_hash,
                        Is.EqualTo(sourceIdentity.session.package_graph_hash));
                    Assert.That(
                        playing.session.definition_hash,
                        Is.EqualTo(sourceIdentity.session.definition_hash));
                    Assert.That(
                        playing.session.definition_revision,
                        Is.EqualTo(sourceIdentity.session.definition_revision));
                    Assert.That(
                        playing.active_runtime_flow.bound_level_id,
                        Is.EqualTo(sourceIdentity.active_runtime_flow.bound_level_id));
                    AssertSameCampaignState(sourceSnapshot, restored.CampaignSnapshot());
                    Assert.Throws<InvalidOperationException>(() =>
                        restored.CampaignStep(0, 0, 0, 0, 0));
                    Assert.That(
                        restored.IdentitySnapshot().mode,
                        Is.EqualTo(GameRuntimeTransport.PausedMode));
                    var resumed = restored.Resume();
                    Assert.That(resumed.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                    Assert.That(resumed.session.generation, Is.EqualTo("2"));
                    Assert.That(
                        restored.CampaignStep(0, 0, 0, 0, 0).identity.tick,
                        Is.EqualTo("4"));
                }
            }

            // This proves opaque v5 quiescent-Paused replay bytes pass through
            // the managed transport, remain mutation-frozen, and resume on one
            // fresh real-L1 Rust owner while stale v4 fails atomically. It does
            // not prove paused-frame service, cross-level saves, migration, or
            // menu wiring.
        }

        [Test]
        [Timeout(180000)]
        public void ProductionMagicCarpetRuntimeRestoresItsReplayIntoOneFreshOwner()
        {
            var nativeSessions = NativeSession.ActiveSessionCount;
            var campaignSessions = NativeCampaignSession.ActiveSessionCount;
            Assert.That(campaignSessions, Is.Zero);
            var replayPath = Path.Combine(
                Path.GetTempPath(),
                $"magiccarpet-runtime-{Guid.NewGuid():N}.replay.json");
            GameRuntimeTransport.Identity sourceIdentity = null;
            EngineSessionTransport.CampaignSnapshotObservation sourceSnapshot = null;

            var sourceRoot = new GameObject("replay source production owner");
            sourceRoot.SetActive(false);
            var source = sourceRoot.AddComponent<MagicCarpetRuntime>();
            try
            {
                source.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeReplaySaveArguments(
                        replayPath,
                        LevelThreePackage,
                        LevelOnePackage,
                        LevelTwoPackage));
                source.QualificationDriveGameRuntimeFrames(3);
                sourceIdentity = source.QualificationPauseGameRuntimeProduction();
                sourceSnapshot = source.GameRuntimeProductionCampaignSnapshot;
                Assert.That(sourceIdentity.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                Assert.That(sourceIdentity.session.tick, Is.EqualTo("3"));
                source.QualificationRequestGameRuntimeProductionReplaySave();
                source.QualificationDriveGameRuntimeFrames(1);
                Assert.That(File.Exists(replayPath), Is.True);
                Assert.That(new FileInfo(replayPath).Length, Is.GreaterThan(0));
                Assert.That(source.GameRuntimeProductionIdentity.session.tick, Is.EqualTo("3"));
                AssertSameCampaignState(
                    sourceSnapshot,
                    source.GameRuntimeProductionCampaignSnapshot);
            }
            finally
            {
                source.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(sourceRoot);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions));

            var restoredRoot = new GameObject("replay restored production owner");
            restoredRoot.SetActive(false);
            var restored = restoredRoot.AddComponent<MagicCarpetRuntime>();
            try
            {
                restored.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeReplayArguments(
                        replayPath,
                        LevelThreePackage,
                        LevelOnePackage,
                        LevelTwoPackage));
                Assert.That(restored.GameRuntimeProductionStartupReady, Is.True);
                var restoredIdentity = restored.GameRuntimeProductionIdentity;
                Assert.That(restoredIdentity.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                Assert.That(restoredIdentity.generation, Is.EqualTo("2"));
                Assert.That(restoredIdentity.session.generation, Is.EqualTo("1"));
                Assert.That(restoredIdentity.session.tick, Is.EqualTo("3"));
                Assert.That(
                    restoredIdentity.session.session_id,
                    Is.Not.EqualTo(sourceIdentity.session.session_id));
                Assert.That(
                    restoredIdentity.session.package_graph_hash,
                    Is.EqualTo(sourceIdentity.session.package_graph_hash));
                Assert.That(
                    restoredIdentity.session.definition_hash,
                    Is.EqualTo(sourceIdentity.session.definition_hash));
                Assert.That(
                    restoredIdentity.active_runtime_flow.bound_level_id,
                    Is.EqualTo(sourceIdentity.active_runtime_flow.bound_level_id));
                AssertSameCampaignState(
                    sourceSnapshot,
                    restored.GameRuntimeProductionCampaignSnapshot);

                restored.QualificationDriveGameRuntimeFrames(1);
                Assert.That(
                    restored.GameRuntimeProductionIdentity.session.tick,
                    Is.EqualTo("3"));
                Assert.That(
                    restored.QualificationResumeGameRuntimeProduction().mode,
                    Is.EqualTo(GameRuntimeTransport.PlayingMode));
                restored.QualificationDriveGameRuntimeFrames(1);
                Assert.That(
                    restored.GameRuntimeProductionIdentity.session.tick,
                    Is.EqualTo("4"));
                Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions + 1));
                Assert.That(
                    NativeCampaignSession.ActiveSessionCount,
                    Is.EqualTo(campaignSessions));
            }
            finally
            {
                restored.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(restoredRoot);
                if (File.Exists(replayPath)) File.Delete(replayPath);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions));
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.EqualTo(campaignSessions));

            // No-Claim: this proves the production runtime owner atomically
            // writes and restores one owner-local quiescent-Paused real-L1
            // replay file, remains frozen, and resumes in a fresh facade. It
            // does not provide paused-frame service, menu UI, cross-level
            // saves, migration, cloud sync, or completion handling.
        }

        [Test]
        [Timeout(180000)]
        public void ProductionNativeSessionFacadeOwnsOneResolverRuntimeHandle()
        {
            var nativeSessions = NativeSession.ActiveSessionCount;
            var campaignSessions = NativeCampaignSession.ActiveSessionCount;
            Assert.That(campaignSessions, Is.Zero);

            var incomplete = Assert.Throws<GameRuntimeTransport.TransitionException>(() =>
                MagicCarpetRuntime.OpenGameRuntimeSessionForPackages(
                    Package(LevelTwoPackage),
                    Package(LevelOnePackage)));
            Assert.That(incomplete.Status, Is.EqualTo(NativeSim.ValidationFailed));
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions));
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.EqualTo(campaignSessions));

            NativeSession session = null;
            ulong handle = 0;
            try
            {
                session = MagicCarpetRuntime.OpenGameRuntimeSessionForPackages(
                    ResolverPackages());
                handle = session.Handle;
                Assert.That(session.UsesGameRuntimeContract, Is.True);
                Assert.That(session.UsesEngineContract, Is.False);
                Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions + 1));
                Assert.That(NativeCampaignSession.ActiveSessionCount, Is.EqualTo(campaignSessions));

                var identity = session.GameRuntimeIdentity();
                Assert.That(identity.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(
                    identity.active_runtime_flow.bound_level_id,
                    Is.EqualTo("level.magic_carpet_2.night_spire"));
                Assert.That(identity.session.session_id, Is.EqualTo(handle.ToString()));
                Assert.That(identity.session.generation, Is.EqualTo("1"));
                Assert.That(identity.session.tick, Is.EqualTo("0"));

                var before = session.GameRuntimeCampaignSnapshot();
                Assert.That(before.identity.session_id, Is.EqualTo(handle.ToString()));
                var stepped = session.GameRuntimeCampaignStep(0, 0, 1, 0, 0);
                Assert.That(stepped.identity.tick, Is.EqualTo("1"));
                Assert.That(
                    session.GameRuntimeCampaignEvents("0").identity.session_id,
                    Is.EqualTo(handle.ToString()));
                var reset = session.GameRuntimeCampaignReset();
                Assert.That(reset.identity.generation, Is.EqualTo("2"));
                Assert.That(reset.identity.tick, Is.EqualTo("0"));
                Assert.That(reset.value.state_hash, Is.EqualTo(before.value.state_hash));

                Assert.That(
                    NativeSim.mc_game_session_destroy_v1(handle),
                    Is.EqualTo(NativeSim.InvalidArgument));
                Assert.That(
                    session.GameRuntimeIdentity().session.session_id,
                    Is.EqualTo(handle.ToString()));
            }
            finally
            {
                session?.Dispose();
            }

            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions));
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.EqualTo(campaignSessions));
            if (handle != 0)
            {
                Assert.That(
                    NativeSim.mc_game_runtime_identity_json_v1(
                        handle, IntPtr.Zero, 0, out _),
                    Is.EqualTo(NativeSim.UnknownHandle));
            }

            // No-Claim: this proves the production NativeSession facade owns
            // one resolver-derived runtime handle and its generic gameplay
            // facet. It does not migrate MagicCarpetRuntime presentation,
            // completion transitions, later-level rendering, or shell input.
        }

        [Test]
        [Timeout(180000)]
        public void ProductionMagicCarpetRuntimeOwnsAndStepsOneResolverRuntimeHandle()
        {
            var nativeSessions = NativeSession.ActiveSessionCount;
            var campaignSessions = NativeCampaignSession.ActiveSessionCount;
            Assert.That(campaignSessions, Is.Zero);
            var root = new GameObject("resolver runtime production owner");
            root.SetActive(false);
            MagicCarpetRuntime runtime = null;
            ulong handle = 0;
            try
            {
                runtime = root.AddComponent<MagicCarpetRuntime>();
                var incomplete = Assert.Throws<GameRuntimeTransport.TransitionException>(() =>
                    runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                        RuntimeArguments(LevelTwoPackage, LevelOnePackage)));
                Assert.That(incomplete.Status, Is.EqualTo(NativeSim.ValidationFailed));
                Assert.That(runtime.GameRuntimeProductionStartupReady, Is.False);
                Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions));
                Assert.That(
                    NativeCampaignSession.ActiveSessionCount,
                    Is.EqualTo(campaignSessions));

                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                Assert.That(runtime.GameRuntimeProductionStartupReady, Is.True);
                Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions + 1));
                Assert.That(NativeCampaignSession.ActiveSessionCount, Is.EqualTo(campaignSessions));

                var identity = runtime.GameRuntimeProductionIdentity;
                handle = ulong.Parse(identity.session.session_id);
                Assert.That(identity.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(
                    identity.active_runtime_flow.bound_level_id,
                    Is.EqualTo("level.magic_carpet_2.night_spire"));
                Assert.That(identity.session.session_id, Is.EqualTo(handle.ToString()));
                Assert.That(identity.session.generation, Is.EqualTo("1"));
                Assert.That(identity.session.tick, Is.EqualTo("0"));

                var before = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(before.identity.session_id, Is.EqualTo(handle.ToString()));
                runtime.QualificationDriveGameRuntimeFrames(1);
                var stepped = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(stepped.identity.tick, Is.EqualTo("1"));
                runtime.QualificationResetGameRuntimeProduction();
                var reset = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(reset.identity.generation, Is.EqualTo("2"));
                Assert.That(reset.identity.tick, Is.EqualTo("0"));
                Assert.That(reset.value.state_hash, Is.EqualTo(before.value.state_hash));

                Assert.That(
                    NativeSim.mc_game_session_destroy_v1(handle),
                    Is.EqualTo(NativeSim.InvalidArgument));
                Assert.That(
                    runtime.GameRuntimeProductionIdentity.session.session_id,
                    Is.EqualTo(handle.ToString()));
            }
            finally
            {
                runtime?.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }

            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions));
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.EqualTo(campaignSessions));
            if (handle != 0)
            {
                Assert.That(
                    NativeSim.mc_game_runtime_identity_json_v1(
                        handle, IntPtr.Zero, 0, out _),
                    Is.EqualTo(NativeSim.UnknownHandle));
            }

            // No-Claim: this proves MagicCarpetRuntime owns one resolver-derived
            // runtime handle and drives its generic fixed-step/reset facet. It
            // does not migrate terrain/HUD/audio/entity presentation, expose
            // completion transitions, discover packages, or render later levels.
        }

        [Test]
        [Timeout(180000)]
        public void ProductionGameRuntimePausedUpdateServicesRustAndDropsPausedFireInput()
        {
            var root = new GameObject("resolver runtime pause input");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                runtime.QualificationDriveGameRuntimeFrames(1);
                var playing = runtime.GameRuntimeProductionIdentity;
                var beforePause = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(playing.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(runtime.GameRuntimeProductionPauseOverlayText, Is.Null);
                Assert.That(beforePause.identity.tick, Is.EqualTo("1"));
                Assert.That(beforePause.value.presentation_entities, Is.Empty);

                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                var paused = runtime.GameRuntimeProductionIdentity;
                Assert.That(paused.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
                Assert.That(paused.session.session_id, Is.EqualTo(playing.session.session_id));
                Assert.That(
                    runtime.GameRuntimeProductionPauseOverlayText,
                    Is.EqualTo("PAUSED\nP resume \u00b7 Escape close"));
                runtime.QualificationDriveGameRuntimeFrames(1);
                var afterFirstPausedUpdate = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(
                    afterFirstPausedUpdate.identity.tick,
                    Is.EqualTo(beforePause.identity.tick));
                Assert.That(
                    afterFirstPausedUpdate.value.state_hash,
                    Is.Not.EqualTo(beforePause.value.state_hash));
                Assert.That(
                    afterFirstPausedUpdate.value.terrain_hash,
                    Is.EqualTo(beforePause.value.terrain_hash));
                Assert.That(
                    afterFirstPausedUpdate.value.effect_journal_sha256,
                    Is.EqualTo(beforePause.value.effect_journal_sha256));
                Assert.That(afterFirstPausedUpdate.value.presentation_entities, Is.Empty);

                runtime.QualificationDriveGameRuntimeFrames(1);
                var afterSecondPausedUpdate = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(
                    afterSecondPausedUpdate.identity.tick,
                    Is.EqualTo(beforePause.identity.tick));
                Assert.That(
                    afterSecondPausedUpdate.value.state_hash,
                    Is.Not.EqualTo(afterFirstPausedUpdate.value.state_hash));
                Assert.That(
                    afterSecondPausedUpdate.value.player_pose.x,
                    Is.EqualTo(beforePause.value.player_pose.x));
                Assert.That(
                    afterSecondPausedUpdate.value.player_pose.y,
                    Is.EqualTo(beforePause.value.player_pose.y));
                Assert.That(
                    afterSecondPausedUpdate.value.player_pose.z,
                    Is.EqualTo(beforePause.value.player_pose.z));

                runtime.QualificationApplyGameRuntimeProductionInput(false, true);
                runtime.QualificationApplyGameRuntimeProductionInput(true, false);
                Assert.That(
                    runtime.GameRuntimeProductionIdentity.mode,
                    Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(runtime.GameRuntimeProductionPauseOverlayText, Is.Null);
                runtime.QualificationDriveGameRuntimeFrames(1);
                var resumed = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(resumed.identity.tick, Is.EqualTo("2"));
                Assert.That(resumed.value.presentation_entities, Is.Empty);

                // This proves each production paused FixedUpdate services one
                // Rust-owned replay-visible pre-gate frame, freezes gameplay
                // tick/pose, presents Paused visibly, and drops a Fireball
                // click received while paused. It does not claim later-level
                // pause policy, pause-menu commands/audio, or completion.
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void ProductionCloseThenEnterRestartsResolverEntryOnSameHandleWithoutSessionAba()
        {
            var root = new GameObject("resolver runtime close input");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                runtime.QualificationDriveGameRuntimeFrames(1);
                var playing = runtime.GameRuntimeProductionIdentity;
                var handle = ulong.Parse(playing.runtime_id);
                var runtimeGeneration = ulong.Parse(playing.generation);
                var sessionGeneration = ulong.Parse(playing.session.generation);
                var tick = ulong.Parse(playing.session.tick);
                var activeSessionCount = NativeSession.ActiveSessionCount;
                Assert.That(playing.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(runtime.GameRuntimeProductionFireballTexture, Is.Not.Null);

                runtime.QualificationCloseGameRuntimeProduction();
                var title = runtime.GameRuntimeProductionIdentity;
                Assert.That(title.runtime_id, Is.EqualTo(playing.runtime_id));
                Assert.That(ulong.Parse(title.generation), Is.EqualTo(runtimeGeneration + 1));
                Assert.That(title.mode, Is.EqualTo(GameRuntimeTransport.TitleMode));
                Assert.That(title.session.session_id, Is.Null.Or.Empty);
                Assert.That(title.pending_definition.definition_hash, Is.Null.Or.Empty);
                Assert.That(title.active_runtime_flow?.game_id, Is.Null.Or.Empty);
                Assert.That(runtime.GameRuntimeProductionStartupReady, Is.False);
                Assert.That(runtime.GameRuntimeProductionFireballTexture, Is.Null);
                Assert.That(runtime.GameRuntimeProductionFireballMarkerCount, Is.Zero);
                StringAssert.Contains("TITLE", runtime.GameRuntimeProductionFlightHudText);

                Assert.That(
                    NativeSim.mc_game_runtime_close_v1(
                        handle,
                        runtimeGeneration,
                        sessionGeneration,
                        tick),
                    Is.EqualTo(NativeSim.GenerationMismatch));
                Assert.That(
                    NativeSim.mc_game_session_snapshot_json_v1(
                        handle,
                        IntPtr.Zero,
                        0,
                        out _),
                    Is.EqualTo(NativeSim.InvalidArgument));
                var command = Encoding.UTF8.GetBytes("{}");
                Assert.That(
                    NativeSim.mc_game_session_step_json_v1(
                        handle,
                        sessionGeneration,
                        tick,
                        command,
                        checked((uint)command.Length)),
                    Is.EqualTo(NativeSim.InvalidArgument));
                Assert.That(
                    runtime.GameRuntimeProductionIdentity.runtime_id,
                    Is.EqualTo(playing.runtime_id));

                runtime.QualificationRestartGameRuntimeProduction();
                var restarted = runtime.GameRuntimeProductionIdentity;
                Assert.That(restarted.runtime_id, Is.EqualTo(playing.runtime_id));
                Assert.That(ulong.Parse(restarted.generation), Is.EqualTo(runtimeGeneration + 3));
                Assert.That(restarted.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                Assert.That(
                    restarted.active_runtime_flow.bound_level_id,
                    Is.EqualTo(restarted.active_runtime_flow.entry_level_id));
                Assert.That(
                    restarted.active_runtime_flow.bound_level_id,
                    Is.EqualTo("level.magic_carpet_2.night_spire"));
                Assert.That(restarted.session.session_id, Is.EqualTo(playing.runtime_id));
                Assert.That(restarted.session.generation, Is.EqualTo("2"));
                Assert.That(restarted.session.tick, Is.EqualTo("0"));
                Assert.That(runtime.GameRuntimeProductionStartupReady, Is.True);
                Assert.That(runtime.GameRuntimeProductionFireballTexture, Is.Not.Null);
                Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeSessionCount));
                Assert.That(
                    NativeSim.mc_game_session_step_json_v1(
                        handle,
                        sessionGeneration,
                        tick,
                        command,
                        checked((uint)command.Length)),
                    Is.EqualTo(NativeSim.GenerationMismatch));
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(
                    runtime.GameRuntimeProductionCampaignSnapshot.identity.tick,
                    Is.EqualTo("1"));

                // No-Claim: Escape and Enter close and restart the retained
                // resolver entry on one Rust runtime without session-guard ABA.
                // They do not choose packages, accept completion, quick-load
                // after close, persist the closed state, or present a menu.
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void ProductionGameRuntimeSteeringChangesRustPoseAndSemanticCameraOnSameHandle()
        {
            var root = new GameObject("resolver runtime steering input");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                var beforeIdentity = runtime.GameRuntimeProductionIdentity;
                var before = runtime.GameRuntimeProductionCampaignSnapshot;
                var cameraBefore = runtime.GameRuntimeProductionCameraRotation;
                Assert.That(before.identity.tick, Is.EqualTo("0"));
                Assert.That(
                    Quaternion.Angle(
                        cameraBefore,
                        ExpectedRuntimeCameraRotation(before.value.player_pose)),
                    Is.LessThan(0.001f));

                runtime.QualificationApplyGameRuntimeProductionSteering(24f, -8f);
                runtime.QualificationApplyGameRuntimeProductionMovement(
                    true,
                    false,
                    false,
                    false);
                Assert.That(
                    Quaternion.Angle(
                        runtime.GameRuntimeProductionCameraRotation,
                        cameraBefore),
                    Is.LessThan(0.001f),
                    "Unity input alone must not move the semantic camera");
                runtime.QualificationDriveGameRuntimeFrames(3);

                var afterIdentity = runtime.GameRuntimeProductionIdentity;
                var after = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(afterIdentity.runtime_id, Is.EqualTo(beforeIdentity.runtime_id));
                Assert.That(
                    afterIdentity.session.session_id,
                    Is.EqualTo(beforeIdentity.session.session_id));
                Assert.That(after.identity.tick, Is.EqualTo("3"));
                Assert.That(
                    after.value.player_pose.roll != before.value.player_pose.roll ||
                    after.value.player_pose.yaw != before.value.player_pose.yaw ||
                    after.value.player_pose.display_pitch !=
                        before.value.player_pose.display_pitch,
                    Is.True,
                    "the steering command must change the Rust-owned player pose");
                Assert.That(
                    Quaternion.Angle(
                        runtime.GameRuntimeProductionCameraRotation,
                        ExpectedRuntimeCameraRotation(after.value.player_pose)),
                    Is.LessThan(0.001f));
                Assert.That(
                    Quaternion.Angle(
                        runtime.GameRuntimeProductionCameraRotation,
                        cameraBefore),
                    Is.GreaterThan(0.01f));

                // No-Claim: this proves production mouse/JLIK deflection reaches
                // the same Rust runtime handle and the existing semantic camera
                // follows the returned pose. It does not prove terrain/HUD/audio,
                // spell-menu input, controller rebinding, or later-level cameras.
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void ProductionGameRuntimeFlightHudReadsReturnedRustMotionOnSameHandle()
        {
            var root = new GameObject("resolver runtime flight HUD");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                var beforeIdentity = runtime.GameRuntimeProductionIdentity;
                var before = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(
                    runtime.GameRuntimeProductionFlightHudText,
                    Is.EqualTo(
                        "CARPET FLIGHT — RUST GAME RUNTIME · PLAYING\n" +
                        $"speed {before.value.player_motion.actual_speed,4} / " +
                        $"{before.value.player_motion.target_speed,4}  " +
                        $"strafe {before.value.player_motion.strafe_speed,4}  " +
                        $"altitude {before.value.player_pose.z,6}  " +
                        $"tick {before.identity.tick}\n" +
                        $"life {before.value.player_vitals.life,6} / " +
                        $"{before.value.player_vitals.max_life,6}  " +
                        $"mana {before.value.player_vitals.mana,6} / " +
                        $"{before.value.player_vitals.max_mana,6}\n" +
                        $"objective {before.value.objective_progress.current_objective} · " +
                        $"{before.value.objective_progress.completed_count}/" +
                        $"{before.value.objective_progress.total_count} complete\n" +
                        "event none\n" +
                        "W/S speed · A/D strafe · persistent mouse or I/J/K/L steer · " +
                        "C centre · Space fire · P pause · Escape close"));

                runtime.QualificationApplyGameRuntimeProductionMovement(
                    true,
                    false,
                    true,
                    false);
                runtime.QualificationDriveGameRuntimeFrames(3);

                var afterIdentity = runtime.GameRuntimeProductionIdentity;
                var after = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(afterIdentity.runtime_id, Is.EqualTo(beforeIdentity.runtime_id));
                Assert.That(
                    afterIdentity.session.session_id,
                    Is.EqualTo(beforeIdentity.session.session_id));
                Assert.That(after.identity.tick, Is.EqualTo("3"));
                Assert.That(
                    after.value.player_motion.actual_speed,
                    Is.Not.EqualTo(before.value.player_motion.actual_speed));
                var recentEvents = runtime.GameRuntimeProductionRecentSemanticEvents;
                var latestEvent = recentEvents.Length == 0
                    ? "none"
                    : $"{recentEvents[^1].sequence} {recentEvents[^1].event_id} " +
                      $"actor={(string.IsNullOrEmpty(recentEvents[^1].actor) ? "-" : recentEvents[^1].actor)}";
                Assert.That(
                    runtime.GameRuntimeProductionFlightHudText,
                    Is.EqualTo(
                        "CARPET FLIGHT — RUST GAME RUNTIME · PLAYING\n" +
                        $"speed {after.value.player_motion.actual_speed,4} / " +
                        $"{after.value.player_motion.target_speed,4}  " +
                        $"strafe {after.value.player_motion.strafe_speed,4}  " +
                        $"altitude {after.value.player_pose.z,6}  " +
                        $"tick {after.identity.tick}\n" +
                        $"life {after.value.player_vitals.life,6} / " +
                        $"{after.value.player_vitals.max_life,6}  " +
                        $"mana {after.value.player_vitals.mana,6} / " +
                        $"{after.value.player_vitals.max_mana,6}\n" +
                        $"objective {after.value.objective_progress.current_objective} · " +
                        $"{after.value.objective_progress.completed_count}/" +
                        $"{after.value.objective_progress.total_count} complete\n" +
                        $"event {latestEvent}\n" +
                        "W/S speed · A/D strafe · persistent mouse or I/J/K/L steer · " +
                        "C centre · Space fire · P pause · Escape close"));

                // No-Claim: this proves the production HUD text consumes the
                // same-handle Rust motion, altitude, mode, and tick returned by
                // the campaign snapshot. It does not prove source-art HUD assets,
                // source-art life/mana gauges, spells/objectives, minimap/terrain/audio,
                // or later levels.
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void ProductionGameRuntimeObjectiveHudConsumesRustSemanticProgressOnSameHandle()
        {
            var root = new GameObject("resolver runtime objective HUD");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                var beforeIdentity = runtime.GameRuntimeProductionIdentity;
                var before = runtime.GameRuntimeProductionCampaignSnapshot;
                var progress = before.value.objective_progress;
                Assert.That(progress, Is.Not.Null);
                Assert.That(
                    progress.current_objective,
                    Is.EqualTo("objective.reach.my_beacon"));
                Assert.That(progress.completed_count, Is.Zero);
                Assert.That(progress.total_count, Is.EqualTo(5));
                Assert.That(progress.all_objectives_satisfied, Is.False);
                Assert.That(runtime.GameRuntimeProductionObjectiveBarWidth, Is.Zero);
                Assert.That(
                    runtime.GameRuntimeProductionFlightHudText,
                    Does.Contain(
                        "objective objective.reach.my_beacon · 0/5 complete"));

                runtime.QualificationApplyGameRuntimeProductionMovement(
                    true,
                    false,
                    false,
                    false);
                var commandedRoll = 0f;
                var commandedPitch = 0f;
                for (var index = 0; index < 512; index++)
                {
                    var pose = runtime.GameRuntimeProductionCampaignSnapshot.value.player_pose;
                    var deltaX = unchecked((short)(39424 - pose.x));
                    var deltaY = unchecked((short)(58624 - pose.y));
                    var desired = Mathf.RoundToInt(
                        512f + Mathf.Atan2(deltaY, deltaX) * (1024f / Mathf.PI));
                    desired = (desired % 2048 + 2048) % 2048;
                    var difference = desired - (pose.yaw & 0x07ff);
                    if (difference > 1024) difference -= 2048;
                    if (difference < -1024) difference += 2048;
                    var nextRoll = Mathf.Clamp(difference, -24, 24);
                    const float nextPitch = -127f;
                    runtime.QualificationApplyGameRuntimeProductionSteering(
                        (nextRoll - commandedRoll) / 4.5f,
                        (commandedPitch - nextPitch) / 4.5f);
                    commandedRoll = nextRoll;
                    commandedPitch = nextPitch;
                    runtime.QualificationDriveGameRuntimeFrames(1);
                    if (runtime.GameRuntimeProductionCampaignSnapshot.value
                        .objective_progress.completed_count == 1)
                    {
                        break;
                    }
                }
                var steppedIdentity = runtime.GameRuntimeProductionIdentity;
                var stepped = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(
                    steppedIdentity.runtime_id,
                    Is.EqualTo(beforeIdentity.runtime_id));
                Assert.That(
                    steppedIdentity.session.session_id,
                    Is.EqualTo(beforeIdentity.session.session_id));
                Assert.That(
                    stepped.identity.session_id,
                    Is.EqualTo(before.identity.session_id));
                Assert.That(
                    stepped.value.objective_progress.current_objective,
                    Is.EqualTo("objective.reach.foot_of_spire"));
                Assert.That(stepped.value.objective_progress.completed_count, Is.EqualTo(1));
                Assert.That(stepped.value.objective_progress.total_count, Is.EqualTo(5));
                Assert.That(runtime.GameRuntimeProductionObjectiveBarWidth, Is.EqualTo(48));
                Assert.That(
                    runtime.GameRuntimeProductionFlightHudText,
                    Does.Contain(
                        "objective objective.reach.foot_of_spire · 1/5 complete"));

                runtime.QualificationResetGameRuntimeProduction();
                var reset = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(
                    reset.identity.session_id,
                    Is.EqualTo(before.identity.session_id));
                Assert.That(reset.identity.generation, Is.EqualTo("2"));
                Assert.That(
                    reset.value.objective_progress.current_objective,
                    Is.EqualTo("objective.reach.my_beacon"));
                Assert.That(reset.value.objective_progress.completed_count, Is.Zero);
                Assert.That(reset.value.objective_progress.total_count, Is.EqualTo(5));
                Assert.That(runtime.GameRuntimeProductionObjectiveBarWidth, Is.Zero);
                Assert.That(
                    runtime.GameRuntimeProductionFlightHudText,
                    Does.Contain(
                        "objective objective.reach.my_beacon · 0/5 complete"));

                // No-Claim: this proves the real Level 1 Rust snapshot's
                // semantic objective ID, counts, and directly derived generic
                // bar width are presented by Unity on one runtime/session
                // handle through ordinary commanded advancement and reset. It
                // does not localize objective text, use source-art gauges, or
                // prove later-level HUD presentation.
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void ProductionGameRuntimeSelectsAndCastsAnAcquiredSemanticSpell()
        {
            var root = new GameObject("resolver runtime semantic spell input");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                var initial = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(initial.identity.tick, Is.EqualTo("0"));
                Assert.That(initial.value.presentation_entities, Is.Empty);

                Assert.That(
                    runtime.QualificationSelectGameRuntimeProductionSpell("spell.fireball"),
                    Is.True);
                Assert.That(
                    runtime.QualificationSelectGameRuntimeProductionSpell("spell.castle"),
                    Is.False,
                    "Unity must not overwrite a pending semantic selection before Rust consumes it");
                runtime.QualificationDriveGameRuntimeFrames(1);
                var selected = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(selected.identity.tick, Is.EqualTo("1"));
                Assert.That(selected.value.presentation_entities, Is.Empty);

                runtime.QualificationRequestGameRuntimeSelectedSpellCast();
                runtime.QualificationDriveGameRuntimeFrames(1);
                var cast = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(cast.identity.tick, Is.EqualTo("2"));
                Assert.That(
                    Array.Exists(
                        cast.value.presentation_entities,
                        entity => entity.kind == "spell.fireball.projectile"),
                    Is.True);
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void ProductionGameRuntimeVitalsHudConsumesRustFireballManaAndResetOnSameHandle()
        {
            var root = new GameObject("resolver runtime vitals HUD");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                var beforeIdentity = runtime.GameRuntimeProductionIdentity;
                var before = runtime.GameRuntimeProductionCampaignSnapshot;
                var beforeVitals = before.value.player_vitals;
                Assert.That(beforeVitals, Is.Not.Null);
                Assert.That(beforeVitals.life, Is.Positive);
                Assert.That(beforeVitals.max_life, Is.GreaterThanOrEqualTo(beforeVitals.life));
                Assert.That(beforeVitals.mana, Is.Positive);
                Assert.That(beforeVitals.max_mana, Is.GreaterThanOrEqualTo(beforeVitals.mana));
                Assert.That(
                    runtime.GameRuntimeProductionVitalsBarWidths,
                    Is.EqualTo(new Vector2Int(240, 240)));
                Assert.That(
                    runtime.GameRuntimeProductionFlightHudText,
                    Does.Contain(
                        $"life {beforeVitals.life,6} / {beforeVitals.max_life,6}  " +
                        $"mana {beforeVitals.mana,6} / {beforeVitals.max_mana,6}"));

                runtime.QualificationRequestGameRuntimeFireball();
                runtime.QualificationDriveGameRuntimeFrames(2);

                var afterIdentity = runtime.GameRuntimeProductionIdentity;
                var after = runtime.GameRuntimeProductionCampaignSnapshot;
                var afterVitals = after.value.player_vitals;
                Assert.That(afterIdentity.runtime_id, Is.EqualTo(beforeIdentity.runtime_id));
                Assert.That(
                    afterIdentity.session.session_id,
                    Is.EqualTo(beforeIdentity.session.session_id));
                Assert.That(after.identity.tick, Is.EqualTo("2"));
                Assert.That(afterVitals.life, Is.EqualTo(beforeVitals.life));
                Assert.That(afterVitals.max_life, Is.EqualTo(beforeVitals.max_life));
                Assert.That(afterVitals.max_mana, Is.EqualTo(beforeVitals.max_mana));
                Assert.That(afterVitals.mana, Is.EqualTo(beforeVitals.mana - 100));
                Assert.That(
                    runtime.GameRuntimeProductionVitalsBarWidths,
                    Is.EqualTo(new Vector2Int(
                        240,
                        (int)((long)afterVitals.mana * 240 / afterVitals.max_mana))));
                Assert.That(
                    runtime.GameRuntimeProductionFlightHudText,
                    Does.Contain(
                        $"life {afterVitals.life,6} / {afterVitals.max_life,6}  " +
                        $"mana {afterVitals.mana,6} / {afterVitals.max_mana,6}"));

                runtime.QualificationResetGameRuntimeProduction();
                var resetIdentity = runtime.GameRuntimeProductionIdentity;
                var reset = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(resetIdentity.runtime_id, Is.EqualTo(beforeIdentity.runtime_id));
                Assert.That(
                    resetIdentity.session.session_id,
                    Is.EqualTo(beforeIdentity.session.session_id));
                Assert.That(reset.identity.generation, Is.EqualTo("2"));
                Assert.That(reset.identity.tick, Is.EqualTo("0"));
                Assert.That(reset.value.player_vitals.life, Is.EqualTo(beforeVitals.life));
                Assert.That(reset.value.player_vitals.max_life, Is.EqualTo(beforeVitals.max_life));
                Assert.That(reset.value.player_vitals.mana, Is.EqualTo(beforeVitals.mana));
                Assert.That(reset.value.player_vitals.max_mana, Is.EqualTo(beforeVitals.max_mana));
                Assert.That(
                    runtime.GameRuntimeProductionVitalsBarWidths,
                    Is.EqualTo(new Vector2Int(240, 240)));
                Assert.That(
                    runtime.GameRuntimeProductionFlightHudText,
                    Does.Contain(
                        $"life {beforeVitals.life,6} / {beforeVitals.max_life,6}  " +
                        $"mana {beforeVitals.mana,6} / {beforeVitals.max_mana,6}"));

                // No-Claim: this proves generic numeric local-player life/mana
                // projection, generic bars, one real Fireball charge, reset,
                // and same-handle Unity consumption. It does not prove source-art
                // HUD, damage or death presentation, spell inventory, objectives,
                // L2/L3, audio, or completion transitions.
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void ProductionGameRuntimeHudConsumesOrderedSemanticFireballEventsOnceOnSameHandle()
        {
            var root = new GameObject("resolver runtime semantic event HUD");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                var beforeIdentity = runtime.GameRuntimeProductionIdentity;
                var before = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(
                    runtime.GameRuntimeProductionSemanticEventCursor,
                    Is.EqualTo(before.value.effect_head));
                Assert.That(runtime.GameRuntimeProductionRecentSemanticEvents, Is.Empty);

                runtime.QualificationApplyGameRuntimeProductionInput(false, true);
                runtime.QualificationDriveGameRuntimeFrames(1);

                var afterIdentity = runtime.GameRuntimeProductionIdentity;
                var after = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(afterIdentity.runtime_id, Is.EqualTo(beforeIdentity.runtime_id));
                Assert.That(
                    afterIdentity.session.session_id,
                    Is.EqualTo(beforeIdentity.session.session_id));
                Assert.That(after.identity.tick, Is.EqualTo("1"));
                Assert.That(
                    runtime.GameRuntimeProductionSemanticEventCursor,
                    Is.EqualTo(after.value.effect_head));

                var events = runtime.GameRuntimeProductionRecentSemanticEvents;
                Assert.That(events, Is.Not.Empty);
                var expectedSequence = ulong.Parse(before.value.effect_head);
                EngineSessionTransport.GameEvent activation = null;
                foreach (var gameEvent in events)
                {
                    expectedSequence++;
                    Assert.That(gameEvent.sequence, Is.EqualTo(expectedSequence.ToString()));
                    if (gameEvent.event_id == "spell.activation_requested")
                    {
                        activation = gameEvent;
                    }
                }
                Assert.That(expectedSequence.ToString(), Is.EqualTo(after.value.effect_head));
                Assert.That(activation, Is.Not.Null);
                Assert.That(ulong.TryParse(activation.actor, out var actor), Is.True);
                Assert.That(actor, Is.Not.Zero);
                var latest = events[^1];
                Assert.That(
                    runtime.GameRuntimeProductionFlightHudText,
                    Does.Contain(
                        $"event {latest.sequence} {latest.event_id} " +
                        $"actor={(string.IsNullOrEmpty(latest.actor) ? "-" : latest.actor)}"));

                var cursor = runtime.GameRuntimeProductionSemanticEventCursor;
                runtime.QualificationRefreshGameRuntimeProductionEvents();
                var repeated = runtime.GameRuntimeProductionRecentSemanticEvents;
                Assert.That(runtime.GameRuntimeProductionSemanticEventCursor, Is.EqualTo(cursor));
                Assert.That(repeated.Length, Is.EqualTo(events.Length));
                for (var index = 0; index < events.Length; index++)
                {
                    Assert.That(repeated[index].sequence, Is.EqualTo(events[index].sequence));
                    Assert.That(repeated[index].event_id, Is.EqualTo(events[index].event_id));
                    Assert.That(repeated[index].actor, Is.EqualTo(events[index].actor));
                }

                // No-Claim: this proves the production HUD drains one ordered,
                // same-session semantic Fireball event stream exactly once. It
                // does not play audio, localize hints, place impact VFX, retain
                // UI history across reset, or prove later-level presentation.
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        [Timeout(180000)]
        public void ProductionMagicCarpetRuntimeRendersTheSemanticFireballAssetWithoutOwnerCache()
        {
            const string content =
                "sha256:331df08c5fa8483d5dd75dc10abd9dd76cc26918d6447403b3f3199bc31f521d";
            var nativeSessions = NativeSession.ActiveSessionCount;
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);
            var root = new GameObject("semantic Fireball runtime owner");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(
                    RuntimeArguments(LevelThreePackage, LevelOnePackage, LevelTwoPackage));
                var texture = runtime.GameRuntimeProductionFireballTexture;
                Assert.That(texture, Is.Not.Null);
                Assert.That(texture.width, Is.EqualTo(34));
                Assert.That(texture.height, Is.EqualTo(35));
                Assert.That(
                    runtime.GameRuntimeProductionFireballContentAddress,
                    Is.EqualTo(content));
                Assert.That(runtime.GameRuntimeProductionFireballMarkerCount, Is.Zero);
                Assert.That(
                    runtime.GameRuntimeProductionCampaignSnapshot.value.presentation_entities,
                    Is.Empty);

                runtime.QualificationRequestGameRuntimeFireball();
                runtime.QualificationDriveGameRuntimeFrames(1);
                var snapshot = runtime.GameRuntimeProductionCampaignSnapshot;
                Assert.That(snapshot.identity.tick, Is.EqualTo("1"));
                Assert.That(snapshot.value.presentation_entities, Has.Length.EqualTo(1));
                var fireball = snapshot.value.presentation_entities[0];
                Assert.That(fireball.kind, Is.EqualTo("spell.fireball.projectile"));
                Assert.That(fireball.logical_height, Is.EqualTo(100));
                Assert.That(
                    fireball.asset.id,
                    Is.EqualTo("presentation.spell.fireball.projectile_asset"));
                Assert.That(fireball.asset.content_address, Is.EqualTo(content));
                Assert.That(ulong.Parse(fireball.actor), Is.Not.Zero);
                Assert.That(runtime.GameRuntimeProductionFireballMarkerCount, Is.EqualTo(1));
                Assert.That(
                    runtime.GameRuntimeProductionRenderedFireballTexture,
                    Is.SameAs(texture));
                Assert.That(runtime.GameRuntimeProductionFireballTexture, Is.SameAs(texture));

                runtime.QualificationResetGameRuntimeProduction();
                Assert.That(
                    runtime.GameRuntimeProductionCampaignSnapshot.identity.generation,
                    Is.EqualTo("2"));
                Assert.That(
                    runtime.GameRuntimeProductionCampaignSnapshot.value.presentation_entities,
                    Is.Empty);
                Assert.That(runtime.GameRuntimeProductionFireballMarkerCount, Is.Zero);
                Assert.That(runtime.GameRuntimeProductionFireballTexture, Is.SameAs(texture));
                Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions + 1));
                Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);
            }
            finally
            {
                runtime.QualificationDisposeNativeOwners();
                UnityEngine.Object.DestroyImmediate(root);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(nativeSessions));
            Assert.That(NativeCampaignSession.ActiveSessionCount, Is.Zero);

            // No-Claim: this proves one real L1 one-frame Fireball asset is
            // queried and rendered from the resolver-owned runtime snapshot
            // without an owner-cache/TMAP selector path. It does not prove
            // terrain, other entities, impacts, animation, L2/L3 assets,
            // completion transitions, or a complete playable scene.
        }

        [Test]
        [Timeout(180000)]
        public void RustGameRuntimePausedFrameServicesTheSameLevelOneSession()
        {
            using var runtime = new GameRuntimeTransport();
            runtime.BootstrapPackageSet(ResolverPackages());
            var playing = runtime.OpenLevel();
            var before = runtime.CampaignSnapshot();

            var paused = runtime.Pause();
            Assert.That(paused.mode, Is.EqualTo(GameRuntimeTransport.PausedMode));
            Assert.That(paused.generation, Is.EqualTo("4"));
            Assert.That(paused.session.session_id, Is.EqualTo(playing.session.session_id));
            Assert.That(paused.session.generation, Is.EqualTo("2"));
            Assert.That(paused.session.tick, Is.EqualTo("0"));
            Assert.That(
                paused.active_runtime_flow.bound_level_id,
                Is.EqualTo(playing.active_runtime_flow.bound_level_id));
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
            var afterFirstPausedFrame = runtime.CampaignPausedFrame();
            Assert.That(afterFirstPausedFrame.identity.tick, Is.EqualTo(before.identity.tick));
            Assert.That(
                afterFirstPausedFrame.value.state_hash,
                Is.Not.EqualTo(before.value.state_hash));
            Assert.That(
                afterFirstPausedFrame.value.terrain_hash,
                Is.EqualTo(before.value.terrain_hash));
            Assert.That(
                afterFirstPausedFrame.value.effect_journal_sha256,
                Is.EqualTo(before.value.effect_journal_sha256));
            Assert.That(
                afterFirstPausedFrame.value.player_pose.x,
                Is.EqualTo(before.value.player_pose.x));
            Assert.That(
                afterFirstPausedFrame.value.player_pose.y,
                Is.EqualTo(before.value.player_pose.y));
            Assert.That(
                afterFirstPausedFrame.value.player_pose.z,
                Is.EqualTo(before.value.player_pose.z));
            var afterSecondPausedFrame = runtime.CampaignPausedFrame();
            Assert.That(afterSecondPausedFrame.identity.tick, Is.EqualTo(before.identity.tick));
            Assert.That(
                afterSecondPausedFrame.value.state_hash,
                Is.Not.EqualTo(afterFirstPausedFrame.value.state_hash));

            var resumed = runtime.Resume();
            Assert.That(resumed.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
            Assert.That(resumed.generation, Is.EqualTo("5"));
            Assert.That(resumed.session.generation, Is.EqualTo("3"));
            Assert.That(resumed.session.tick, Is.EqualTo("0"));
            Assert.That(
                resumed.active_runtime_flow.bound_level_id,
                Is.EqualTo(playing.active_runtime_flow.bound_level_id));
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

            // This proves the managed transport services two real-L1 native
            // paused frames while gameplay identity/tick/pose remain frozen,
            // then resumes ordinary stepping. Production consumption is owned
            // by the dedicated MagicCarpetRuntime paused-Update test.
        }

        [Test]
        [Timeout(180000)]
        public void RustGameRuntimeOwnsTheBorrowedCampaignStepEventAndResetFacet()
        {
            var runtime = new GameRuntimeTransport();
            var handle = runtime.Handle;
            try
            {
                runtime.BootstrapPackageSet(ResolverPackages());
                var playing = runtime.OpenLevel();
                Assert.That(playing.session.session_id, Is.EqualTo(handle.ToString()));

                var initial = runtime.CampaignSnapshot();
                using (var other = new GameRuntimeTransport())
                {
                    other.BootstrapPackageSet(ResolverPackages());
                    var otherPlaying = other.OpenLevel();
                    var otherInitial = other.CampaignSnapshot();
                    Assert.That(otherPlaying.session.generation, Is.EqualTo("1"));
                    Assert.That(otherPlaying.session.tick, Is.EqualTo("0"));
                    Assert.That(other.Handle, Is.Not.EqualTo(handle));
                    Assert.Throws<InvalidOperationException>(() =>
                        EngineSessionTransport.StepHandle(
                            handle,
                            otherPlaying.session,
                            0,
                            0,
                            1,
                            0,
                            0,
                            runtime.NegotiatedSessionContract.limits.maximum_command_json_bytes));
                    var unchanged = runtime.CampaignSnapshot();
                    var otherUnchanged = other.CampaignSnapshot();
                    Assert.That(unchanged.identity.generation, Is.EqualTo("1"));
                    Assert.That(unchanged.identity.tick, Is.EqualTo("0"));
                    Assert.That(otherUnchanged.identity.generation, Is.EqualTo("1"));
                    Assert.That(otherUnchanged.identity.tick, Is.EqualTo("0"));
                    AssertSameCampaignState(initial, unchanged);
                    AssertSameCampaignState(otherInitial, otherUnchanged);
                }

                var stepped = runtime.CampaignStep(0, 0, 1, 0, 0);
                Assert.That(stepped.identity.session_id, Is.EqualTo(handle.ToString()));
                Assert.That(stepped.identity.generation, Is.EqualTo("1"));
                Assert.That(stepped.identity.tick, Is.EqualTo("1"));
                Assert.That(runtime.Current.generation, Is.EqualTo("3"));
                Assert.That(runtime.Current.session.tick, Is.EqualTo("1"));

                var head = ulong.Parse(stepped.value.effect_head);
                Assert.That(head, Is.GreaterThan(0UL));
                var cursor = "0";
                var drained = 0UL;
                while (true)
                {
                    var events = runtime.CampaignEvents(cursor);
                    Assert.That(events.identity.session_id, Is.EqualTo(handle.ToString()));
                    Assert.That(events.identity.tick, Is.EqualTo("1"));
                    Assert.That(events.value.requested_cursor, Is.EqualTo(cursor));
                    Assert.That(events.value.event_head, Is.EqualTo(stepped.value.effect_head));
                    foreach (var gameEvent in events.value.events)
                    {
                        drained++;
                        Assert.That(gameEvent.sequence, Is.EqualTo(drained.ToString()));
                    }
                    if (!events.value.has_more)
                    {
                        break;
                    }
                    Assert.That(events.value.next_cursor, Is.Not.EqualTo(cursor));
                    cursor = events.value.next_cursor;
                }
                Assert.That(drained, Is.EqualTo(head));

                Assert.That(
                    NativeSim.mc_game_session_destroy_v1(handle),
                    Is.EqualTo(NativeSim.InvalidArgument));
                Assert.That(runtime.IdentitySnapshot().session.tick, Is.EqualTo("1"));

                var reset = runtime.CampaignReset();
                Assert.That(reset.identity.session_id, Is.EqualTo(handle.ToString()));
                Assert.That(reset.identity.generation, Is.EqualTo("2"));
                Assert.That(reset.identity.tick, Is.EqualTo("0"));
                Assert.That(runtime.Current.generation, Is.EqualTo("3"));
                AssertSameCampaignState(initial, reset);

                var command = Encoding.UTF8.GetBytes("{}");
                Assert.That(
                    NativeSim.mc_game_session_step_json_v1(
                        handle,
                        1,
                        1,
                        command,
                        checked((uint)command.Length)),
                    Is.EqualTo(NativeSim.GenerationMismatch));

                runtime.Dispose();
                runtime.Dispose();
                Assert.That(
                    NativeSim.mc_game_session_snapshot_json_v1(
                        handle,
                        IntPtr.Zero,
                        0,
                        out _),
                    Is.EqualTo(NativeSim.UnknownHandle));

                // No-Claim: this is a borrow-only managed gameplay facet on
                // the Rust runtime handle. It does not replace the production
                // MagicCarpetRuntime bootstrap or prove campaign presentation,
                // HUD/minimap/audio, later-level policy, or paused-frame work.
            }
            finally
            {
                runtime.Dispose();
            }
        }
    }
}
