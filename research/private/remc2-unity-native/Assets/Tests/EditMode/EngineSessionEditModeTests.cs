using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class EngineSessionEditModeTests
    {
        [Serializable]
        private sealed class Corpus
        {
            public Suite[] suites;
        }

        [Serializable]
        private sealed class Suite
        {
            public string id;
            public Checkpoint[] checkpoints;
        }

        [Serializable]
        private sealed class Checkpoint
        {
            public int unity_checkpoint_tick;
            public StateIdentity state_identity;
            public DigestIdentity event_identity;
            public DigestIdentity audio_identity;
        }

        [Serializable]
        private sealed class StateIdentity
        {
            public int tick;
            public int event_head;
            public string state_hash_fnv1a64;
            public string conformance_hash_fnv1a64;
            public int[] position;
            public int yaw;
            public int display_pitch;
            public int effective_pitch;
            public int roll;
            public int target_speed;
            public int actual_speed;
            public int strafe_speed;
            public int fire_sequence;
        }

        [Serializable]
        private sealed class DigestIdentity
        {
            public uint count;
            public string content_sha256;
            public string lifecycle_sha256;
        }

        private static string PrivateRoot =>
            Path.GetFullPath(Path.Combine(Application.dataPath, "../.."));

        private static string PackagePath =>
            Path.Combine(PrivateRoot, "generated/level1-export-m");

        private static string ClosurePath =>
            Path.Combine(PrivateRoot, "generated/level1-asset-closure.json");

        private static string AssetCachePath =>
            Path.Combine(PrivateRoot, "generated/level1-unity-asset-cache-plan29-final");

        private static string CorpusPath =>
            Path.Combine(
                PrivateRoot,
                "remc2-unity-native/qualification/level1-reference-corpus.json");

        private static string CampaignPackagePath =>
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_PACKAGE_V1") ??
            Path.Combine(
                PrivateRoot,
                "generated/mc-cf02-campaign-package-v1-a/qualified-level2");

        private static IReadOnlyDictionary<int, Checkpoint> Checkpoints()
        {
            var corpus = JsonUtility.FromJson<Corpus>(File.ReadAllText(CorpusPath));
            var suite = corpus.suites.Single(value =>
                value.id == "unity-synthetic-presentation-route");
            Assert.That(suite.checkpoints, Has.Length.EqualTo(5));
            return suite.checkpoints.ToDictionary(
                value => value.unity_checkpoint_tick);
        }

        private static void Command(
            int index,
            out sbyte roll,
            out sbyte pitch,
            out sbyte flags)
        {
            var rawFlags = index < 360 ? 1 : 0;
            roll = 0;
            pitch = 0;
            if (index < 145)
                roll = 38;
            else if (index >= 180 && index < 250)
            {
                roll = -32;
                pitch = -22;
            }
            else if (index >= 250 && index < 325)
                pitch = 24;
            if (index >= 325 && index < 365)
                rawFlags |= 0x08;
            if (index is 160 or 300)
                rawFlags |= 0x10;
            flags = checked((sbyte)rawFlags);
        }

        private static string VerifyRoute(
            NativeSession session,
            IReadOnlyDictionary<int, Checkpoint> checkpoints,
            int readCount,
            bool verifyLifecycle)
        {
            var cursor = "0";
            var rows = new List<string>();
            var joinedAudio = false;
            for (var index = 0; index < 440; index++)
            {
                for (var poll = 0; poll < readCount; poll++)
                    session.EngineSnapshot();
                Command(index, out var roll, out var pitch, out var flags);
                var compatibility = session.Step(roll, pitch, flags);
                var tick = index + 1;
                if (!checkpoints.TryGetValue(tick, out var checkpoint))
                    continue;
                var observation = session.EngineSnapshot();
                var value = observation.value;
                var expected = checkpoint.state_identity;
                Assert.That(observation.identity.tick, Is.EqualTo(expected.tick.ToString()));
                Assert.That(value.tick, Is.EqualTo(expected.tick.ToString()));
                Assert.That(value.event_head, Is.EqualTo(expected.event_head.ToString()));
                Assert.That(value.state_hash, Is.EqualTo(expected.state_hash_fnv1a64));
                Assert.That(value.conformance_hash, Is.EqualTo(expected.conformance_hash_fnv1a64));
                Assert.That(
                    new[] { (int)value.player_pose.x, value.player_pose.y, value.player_pose.z },
                    Is.EqualTo(expected.position));
                Assert.That(value.player_pose.yaw, Is.EqualTo(expected.yaw));
                Assert.That(value.player_pose.display_pitch, Is.EqualTo(expected.display_pitch));
                Assert.That(value.player_pose.effective_pitch, Is.EqualTo(expected.effective_pitch));
                Assert.That(value.player_pose.roll, Is.EqualTo(expected.roll));
                Assert.That(value.player_motion.target_speed, Is.EqualTo(expected.target_speed));
                Assert.That(value.player_motion.actual_speed, Is.EqualTo(expected.actual_speed));
                Assert.That(value.player_motion.strafe_speed, Is.EqualTo(expected.strafe_speed));
                Assert.That(value.action_sequence, Is.EqualTo(expected.fire_sequence.ToString()));
                Assert.That(compatibility.tick, Is.EqualTo((ulong)expected.tick));

                var page = session.EngineEvents(cursor);
                cursor = page.value.next_cursor;
                var receipt = page.value.compatibility;
                Assert.That(receipt.contract, Is.EqualTo("mc2-owned-event-audio-identity/v1"));
                Assert.That(receipt.event_count, Is.EqualTo(checkpoint.event_identity.count));
                Assert.That(
                    receipt.event_content_sha256,
                    Is.EqualTo(checkpoint.event_identity.content_sha256));
                Assert.That(receipt.audio_count, Is.EqualTo(checkpoint.audio_identity.count));
                Assert.That(
                    receipt.audio_content_sha256,
                    Is.EqualTo(checkpoint.audio_identity.content_sha256));
                if (verifyLifecycle)
                {
                    Assert.That(
                        receipt.event_lifecycle_sha256,
                        Is.EqualTo(checkpoint.event_identity.lifecycle_sha256));
                    Assert.That(
                        receipt.audio_lifecycle_sha256,
                        Is.EqualTo(checkpoint.audio_identity.lifecycle_sha256));
                }
                if (!joinedAudio)
                {
                    foreach (var gameEvent in page.value.events)
                    {
                        try
                        {
                            var audio = session.EngineAudio(gameEvent.sequence);
                            Assert.That(audio.value.event_sequence, Is.EqualTo(gameEvent.sequence));
                            Assert.That(audio.value.state_hash, Is.EqualTo(gameEvent.state_hash));
                            Assert.That(audio.value.asset.id, Is.EqualTo(gameEvent.asset.id));
                            joinedAudio = true;
                            break;
                        }
                        catch (InvalidOperationException error)
                            when (error.Message.Contains("status=10"))
                        {
                            // This semantic event has no audio sidecar.
                        }
                    }
                }
                rows.Add(string.Join("|", new[]
                {
                    observation.identity.tick,
                    value.state_hash,
                    value.conformance_hash,
                    value.event_head,
                    receipt.event_content_sha256,
                    receipt.audio_content_sha256,
                    cursor,
                }));
            }
            Assert.That(rows, Has.Count.EqualTo(checkpoints.Count));
            Assert.That(joinedAudio, Is.True);
            return string.Join("\n", rows);
        }

        [Test]
        public void NeutralManagedContractRejectsHiddenMutationAndUnboundedOrSourceInputs()
        {
            using var session = NativeSession.OpenEngine(
                PackagePath, ClosurePath, AssetCachePath);
            Assert.That(session.UsesEngineContract, Is.True);
            var contract = session.EngineContract;
            Assert.That(contract.engine_contract, Is.EqualTo(EngineSessionTransport.EngineContract));
            Assert.That(contract.transport_contract, Is.EqualTo(EngineSessionTransport.TransportContract));
            Assert.That(contract.capabilities, Is.EqualTo(new[]
                { "snapshot", "step", "reset", "events", "audio", "presentation_assets" }));

            var unsupported = new EngineSessionTransport.Contract
            {
                engine_contract = contract.engine_contract,
                transport_contract = contract.transport_contract,
                transport_version = 999,
                encoding = contract.encoding,
                u64_counter_encoding = contract.u64_counter_encoding,
                u64_hash_encoding = contract.u64_hash_encoding,
                capabilities = contract.capabilities,
                limits = contract.limits,
                unknown_command_fields = contract.unknown_command_fields,
            };
            Assert.Throws<InvalidOperationException>(
                () => EngineSessionTransport.ValidateContract(unsupported));

            var initial = session.EngineSnapshot();
            initial.value.player_pose.x = 0;
            Assert.That(session.EngineSnapshot().value.player_pose.x, Is.EqualTo(19840));
            Assert.Throws<InvalidOperationException>(
                () => session.Step(0, 0, 0, opcode: 40, argument1: 1));
            Assert.That(session.EngineSnapshot().identity.tick, Is.EqualTo("0"));

            var unknown = Encoding.UTF8.GetBytes("{\"source_opcode\":1}");
            Assert.That(
                NativeSim.mc_game_session_step_json_v1(
                    session.Handle, 1, 0, unknown, checked((uint)unknown.Length)),
                Is.EqualTo(3));
            Assert.That(session.EngineSnapshot().identity.tick, Is.EqualTo("0"));

            Assert.That(
                NativeSim.mc_game_session_snapshot_json_v1(
                    session.Handle, IntPtr.Zero, 0, out var required),
                Is.EqualTo(NativeSim.Bounds));
            Assert.That(required, Is.GreaterThan(0));
            var shortBuffer = Marshal.AllocHGlobal(checked((int)required - 1));
            try
            {
                Assert.That(
                    NativeSim.mc_game_session_snapshot_json_v1(
                        session.Handle, shortBuffer, required - 1, out var repeated),
                    Is.EqualTo(NativeSim.Bounds));
                Assert.That(repeated, Is.EqualTo(required));
            }
            finally
            {
                Marshal.FreeHGlobal(shortBuffer);
            }
            Assert.That(session.EngineSnapshot().identity.tick, Is.EqualTo("0"));

            Command(0, out var roll, out var pitch, out var flags);
            session.Step(roll, pitch, flags);
            Assert.That(
                NativeSim.mc_game_session_step_json_v1(
                    session.Handle, 1, 0, Encoding.UTF8.GetBytes("{}"), 2),
                Is.EqualTo(3));
            Assert.Throws<ArgumentOutOfRangeException>(() => session.EngineEvents("0", 1025));
            Assert.That(session.EngineSnapshot().identity.tick, Is.EqualTo("1"));
        }

        [Test]
        [Timeout(180000)]
        public void NeutralManagedRouteMatchesCorpusReplayAndZeroOneManyReads()
        {
            var checkpoints = Checkpoints();
            var summaries = new List<string>();
            foreach (var reads in new[] { 0, 1, 7 })
            {
                using var session = NativeSession.OpenEngine(
                    PackagePath, ClosurePath, AssetCachePath);
                var first = VerifyRoute(session, checkpoints, reads, true);
                if (reads == 0)
                {
                    var before = session.EngineSnapshot();
                    session.Reset();
                    var reset = session.EngineSnapshot();
                    Assert.That(reset.identity.generation, Is.EqualTo("2"));
                    Assert.That(reset.identity.tick, Is.EqualTo("0"));
                    Assert.That(
                        NativeSim.mc_game_session_step_json_v1(
                            session.Handle,
                            1,
                            0,
                            Encoding.UTF8.GetBytes("{}"),
                            2),
                        Is.EqualTo(NativeSim.GenerationMismatch));
                    var replay = VerifyRoute(session, checkpoints, reads, false);
                    Assert.That(replay, Is.EqualTo(first));
                    Assert.That(before.identity.tick, Is.EqualTo("440"));
                }
                summaries.Add(first);
            }
            Assert.That(summaries[1], Is.EqualTo(summaries[0]));
            Assert.That(summaries[2], Is.EqualTo(summaries[0]));
        }

        [Test]
        public void NeutralManagedPresentationRejectsMutatedOwnerCacheManifest()
        {
            var root = Path.Combine(
                Path.GetTempPath(),
                "magiccarpet-f5-cache-negative-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(root);
            try
            {
                File.WriteAllText(
                    Path.Combine(root, "manifest.json"),
                    File.ReadAllText(Path.Combine(AssetCachePath, "manifest.json")) + "\n");
                var error = Assert.Throws<InvalidDataException>(() =>
                    OwnerAssetCache.OpenAndValidate(
                        root,
                        MagicCarpetRuntime.ExpectedPackageHash,
                        MagicCarpetRuntime.ExpectedClosureHash));
                StringAssert.Contains("manifest metadata mismatch", error.Message);
            }
            finally
            {
                Directory.Delete(root, true);
            }
        }

        [Test]
        public void NeutralManagedCampaignProfileOpensStepsPagesResetsAndFailsClosed()
        {
            var transport = MagicCarpetRuntime.OpenEngineSessionForPackage(
                CampaignPackagePath, null, null);
            try
            {
                Assert.That(
                    transport.Profile,
                    Is.EqualTo(EngineSessionTransport.CampaignProfile));
                var campaignProfile = transport.NegotiatedContract.session_profiles
                    .Single(profile =>
                        profile.profile == EngineSessionTransport.CampaignProfile);
                Assert.That(campaignProfile.snapshot_value, Is.EqualTo("campaign_snapshot/v1"));
                Assert.That(
                    campaignProfile.capabilities,
                    Is.EqualTo(new[] { "snapshot", "step", "reset", "events" }));

                var initial = transport.CampaignSnapshot();
                Assert.That(initial.identity.generation, Is.EqualTo("1"));
                Assert.That(initial.identity.tick, Is.EqualTo("0"));
                Assert.That(initial.value.tick, Is.EqualTo("0"));
                Assert.That(initial.value.effect_head, Is.EqualTo("0"));
                Assert.That(initial.value.state_hash, Has.Length.EqualTo(16));
                Assert.That(initial.value.terrain_hash, Has.Length.EqualTo(16));
                Assert.That(initial.value.effect_journal_sha256, Has.Length.EqualTo(64));

                for (var poll = 0; poll < 3; poll++)
                {
                    var read = transport.CampaignSnapshot();
                    Assert.That(read.value.state_hash, Is.EqualTo(initial.value.state_hash));
                    Assert.That(
                        read.value.effect_journal_sha256,
                        Is.EqualTo(initial.value.effect_journal_sha256));
                }

                var stepped = transport.CampaignStep(0, 0, 0x01, 0, 0);
                Assert.That(stepped.identity.tick, Is.EqualTo("1"));
                var head = ulong.Parse(stepped.value.effect_head);
                Assert.That(head, Is.GreaterThan(0UL));

                var cursor = "0";
                var drained = 0UL;
                while (true)
                {
                    var page = transport.Events(cursor);
                    foreach (var gameEvent in page.value.events)
                    {
                        drained++;
                        Assert.That(gameEvent.sequence, Is.EqualTo(drained.ToString()));
                        Assert.That(gameEvent.event_id, Does.Not.Contain("level"));
                        Assert.That(gameEvent.event_id, Does.Not.Contain("source"));
                        Assert.That(gameEvent.event_id, Does.Not.Contain("slot"));
                    }
                    if (!page.value.has_more) break;
                    cursor = page.value.next_cursor;
                }
                Assert.That(drained, Is.EqualTo(head));

                Assert.Throws<InvalidOperationException>(() => transport.Snapshot());
                Assert.Throws<InvalidOperationException>(() => transport.Step(0, 0, 0, 0, 0));
                Assert.Throws<InvalidOperationException>(() => transport.Audio("1"));
                Assert.Throws<InvalidOperationException>(() =>
                    transport.PresentationAsset("presentation.spell.fireball.projectile_asset"));
                Assert.Throws<InvalidOperationException>(() => transport.CompatibilitySnapshot());

                var reset = transport.CampaignReset();
                Assert.That(reset.identity.generation, Is.EqualTo("2"));
                Assert.That(reset.identity.tick, Is.EqualTo("0"));
                Assert.That(reset.value.state_hash, Is.EqualTo(initial.value.state_hash));
                Assert.That(
                    reset.value.effect_journal_sha256,
                    Is.EqualTo(initial.value.effect_journal_sha256));

                var stale = Encoding.UTF8.GetBytes("{}");
                Assert.That(
                    NativeSim.mc_game_session_step_json_v1(
                        transport.Handle, 1, 0, stale, checked((uint)stale.Length)),
                    Is.EqualTo(NativeSim.GenerationMismatch));

                var replay = transport.CampaignStep(0, 0, 0x01, 0, 0);
                Assert.That(replay.identity.generation, Is.EqualTo("2"));
                Assert.That(replay.value.state_hash, Is.EqualTo(stepped.value.state_hash));
                Assert.That(
                    replay.value.effect_journal_sha256,
                    Is.EqualTo(stepped.value.effect_journal_sha256));
            }
            finally
            {
                transport.Destroy();
            }
        }

        [Test]
        public void NeutralManagedRegistryServesOwnedAndCampaignSessionsTogether()
        {
            using var owned = NativeSession.OpenEngine(
                PackagePath, ClosurePath, AssetCachePath);
            var campaign = MagicCarpetRuntime.OpenEngineSessionForPackage(
                CampaignPackagePath, null, null);
            try
            {
                Assert.That(campaign.Handle, Is.Not.EqualTo(owned.Handle));
                var ownedBefore = owned.EngineSnapshot();
                var stepped = campaign.CampaignStep(0, 0, 0x01, 0, 0);
                Assert.That(stepped.identity.tick, Is.EqualTo("1"));
                var ownedAfter = owned.EngineSnapshot();
                Assert.That(ownedAfter.identity.tick, Is.EqualTo(ownedBefore.identity.tick));
                Assert.That(
                    ownedAfter.value.state_hash,
                    Is.EqualTo(ownedBefore.value.state_hash));
            }
            finally
            {
                campaign.Destroy();
            }
            Assert.That(owned.EngineSnapshot().identity.generation, Is.EqualTo("1"));
        }

        [Test]
        public void NeutralManagedCampaignSelectionIsByPackageAndFailsClosedOnWrongShapes()
        {
            Assert.Throws<ArgumentException>(() =>
                MagicCarpetRuntime.OpenEngineSessionForPackage(
                    CampaignPackagePath, ClosurePath, null));
            Assert.Throws<ArgumentException>(() =>
                MagicCarpetRuntime.OpenEngineSessionForPackage(
                    PackagePath, null, AssetCachePath));
            Assert.Throws<InvalidOperationException>(() =>
                NativeSession.OpenEngine(CampaignPackagePath, null, null));
        }

        [Test]
        public void NeutralManagedSessionOwnsTheRealLevelOnePresentationRoute()
        {
            var baseline = NativeSession.ActiveSessionCount;
            var root = new GameObject("neutral engine presentation route");
            root.SetActive(false);
            MagicCarpetRuntime runtime = null;
            try
            {
                runtime = root.AddComponent<MagicCarpetRuntime>();
                runtime.QualificationInitializeEnginePresentation(
                    PackagePath, ClosurePath, AssetCachePath);
                Assert.That(
                    runtime.QualificationDriveEnginePresentation(8),
                    Is.EqualTo(Enumerable.Repeat(1, 6).ToArray()));
                Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(baseline + 1));
            }
            finally
            {
                runtime?.QualificationDisposeEnginePresentation();
                UnityEngine.Object.DestroyImmediate(root);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(baseline));
        }
    }
}
