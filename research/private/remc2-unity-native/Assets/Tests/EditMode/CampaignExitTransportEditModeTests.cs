using System;
using System.IO;
using System.Runtime.Serialization.Json;
using NUnit.Framework;

namespace MagicCarpet.Native.Tests
{
    public sealed class CampaignExitTransportEditModeTests
    {
        [Test]
        public void EarnedNightSpireExitReachesResultsOnItsRetainedGameplayTick()
        {
            var replayPath = Environment.GetEnvironmentVariable("MC_NIGHT_EXIT_BOUNDARY_SAVE");
            var resourcesPath = Environment.GetEnvironmentVariable("MC_CAMPAIGN_SAVE_RESOURCES");
            if (string.IsNullOrEmpty(replayPath) || string.IsNullOrEmpty(resourcesPath))
                Assert.Ignore("Requires the earned NightSpire exit checkpoint and installed campaign resources.");

            using var input = File.OpenRead(resourcesPath);
            var resources = (GameRuntimeTransport.InstalledPackageResource[])new DataContractJsonSerializer(
                typeof(GameRuntimeTransport.InstalledPackageResource[])).ReadObject(input);
            Assert.That(resources, Has.Length.EqualTo(30));
            using var runtime = NativeSession.OpenGameRuntimeReplay(File.ReadAllBytes(replayPath), resources);
            var before = runtime.GameRuntimeIdentity();
            var previous = runtime.GameRuntimeCampaignSnapshot().value;
            Assert.That(before.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
            Assert.That(before.active_runtime_flow.bound_level_id, Is.EqualTo("level.magic_carpet_2.night_spire"));
            Assert.That(before.session.tick, Is.EqualTo("10695"));
            Assert.That(previous.exit_sequence_active && previous.objective_progress.all_objectives_satisfied, Is.True);

            for (var frame = 0; frame < 187; frame++)
            {
                Assert.That(runtime.GameRuntimeFinishMissionIfReady(), Is.Null);
                var stepped = runtime.GameRuntimeCampaignStep(0, 0, 0, 0, 0);
                Assert.That(stepped.identity.tick, Is.EqualTo(before.session.tick));
                Assert.That(stepped.identity.generation, Is.EqualTo(before.session.generation));
                Assert.That(stepped.value.state_hash, Is.Not.EqualTo(previous.state_hash));
                Assert.That(stepped.value.exit_sequence_active, Is.EqualTo(frame != 186));
                previous = stepped.value;
            }

            var receipt = runtime.GameRuntimeFinishMissionIfReady();
            Assert.That(receipt, Is.Not.Null);
            Assert.That(receipt.kind, Is.EqualTo(SecretTransitionTransport.OrdinaryCompletionBoundaryKind));
            var completion = receipt.ordinary_completion;
            Assert.That(completion.source_session.session_id, Is.EqualTo(before.session.session_id));
            Assert.That(completion.source_session.generation, Is.EqualTo(before.session.generation));
            Assert.That(completion.source_session.tick, Is.EqualTo(before.session.tick));
            Assert.That(completion.after.mode, Is.EqualTo(GameRuntimeTransport.CampaignMode));
            Assert.That(completion.after.campaign_menu.phase, Is.EqualTo("results"));
            Assert.That(completion.after.campaign_menu.completed_levels,
                Is.EqualTo(new[] { "level.magic_carpet_2.night_spire" }));
            Assert.That(completion.after.campaign_menu.available_levels,
                Does.Contain("level.magic_carpet_2.payahandra_ruins"));
        }

        [Test]
        public void SourceSpeedStillRejectsAnOrdinaryStallOrAnUnboundedExitStep()
        {
            Assert.Throws<InvalidDataException>(() => GameRuntimeTransport.ValidateSourceSpeedStepTick(10, 10, 11, false, false));
            Assert.Throws<InvalidDataException>(() => GameRuntimeTransport.ValidateSourceSpeedStepTick(10, 11, 14, false, false));
            Assert.Throws<InvalidDataException>(() => GameRuntimeTransport.ValidateSourceSpeedStepTick(10, 9, 11, true, true));
            Assert.Throws<InvalidDataException>(() => GameRuntimeTransport.ValidateSourceSpeedStepTick(10, 12, 11, true, false));
            Assert.DoesNotThrow(() => GameRuntimeTransport.ValidateSourceSpeedStepTick(10, 14, 14, false, false));
            Assert.DoesNotThrow(() => GameRuntimeTransport.ValidateSourceSpeedStepTick(10, 10, 11, false, true));
            Assert.DoesNotThrow(() => GameRuntimeTransport.ValidateSourceSpeedStepTick(10, 10, 11, true, false));
        }
    }
}
