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
        public void RuntimeReplaySaveRestoresRealLevelOneIntoFreshManagedOwner()
        {
            using (var source = new GameRuntimeTransport())
            {
                source.BootstrapPackageSet(ResolverPackages());
                source.OpenLevel();
                source.CampaignStep(0, 0, 0, 0, 0);
                source.CampaignStep(0, 3, 1, 0, 0);
                source.CampaignStep(0, 0, 0, 64, unchecked((ushort)-32));
                var sourceIdentity = source.IdentitySnapshot();
                var sourceSnapshot = source.CampaignSnapshot();
                Assert.That(sourceIdentity.session.tick, Is.EqualTo("3"));

                var save = source.SaveReplay();
                Assert.That(save, Is.Not.Empty);
                Assert.That(save.Length, Is.LessThanOrEqualTo(
                    source.NegotiatedContract.limits.maximum_replay_save_json_bytes));
                Assert.That(source.IdentitySnapshot().session.tick, Is.EqualTo("3"));
                AssertSameCampaignState(sourceSnapshot, source.CampaignSnapshot());

                using (var restored = new GameRuntimeTransport())
                {
                    var playing = restored.LoadReplay(save, ResolverPackages());
                    Assert.That(playing.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
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
                }
            }

            // No-Claim: this proves opaque replay bytes pass through the
            // managed transport into one fresh real-L1 Rust owner. It does not
            // prove filesystem/UI persistence, paused or cross-level saves,
            // migration, cloud saves, or production menu wiring.
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
        public void RustGameRuntimePauseResumeFreezesTheSameLevelOneSession()
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

            // No-Claim: this is a quiescent pause boundary only. There is no
            // paused_frame transport or MC2 pre-gate RNG/objective/presentation
            // service, replay-visible paused time, or production pause UI.
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
