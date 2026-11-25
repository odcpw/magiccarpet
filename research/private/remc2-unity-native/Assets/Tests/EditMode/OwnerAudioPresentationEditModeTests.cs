using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.TestTools;

namespace MagicCarpet.Native.Tests
{
    public sealed class OwnerAudioPresentationEditModeTests
    {
        private static string PrivateRoot =>
            Path.GetFullPath(Path.Combine(Application.dataPath, "../.."));

        private static string Level2OwnerCache =>
            Path.Combine(PrivateRoot, "generated/level2-day-owner-cache");

        private static string Level1OwnerCache =>
            Path.Combine(PrivateRoot, "generated/level1-unity-asset-cache-plan29-final");

        private static string Level3OwnerCache =>
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_OWNER_CACHE_LEVEL3_V1") ??
            Path.Combine(PrivateRoot, "generated/level3-day-spell-owner-cache-audio-v4");

        private static string SecretSource31NightCache =>
            Path.Combine(PrivateRoot, "generated/mc-paj2-environment-source31-audio-20261026");

        private static string SecretSource31Package =>
            Path.Combine(PrivateRoot, "generated/mc-paj2-source-secret-31-campaign-package");

        private static string SecretSource34DayCache =>
            Path.Combine(PrivateRoot, "generated/mc-paj2-environment-source34-audio-20261026");

        private static string SecretSource34Package =>
            Path.Combine(PrivateRoot, "generated/mc-paj2-source-secret-34-campaign-package");

        [Test]
        public void DayShoreFlightDrivesTheLiveAmbientVoices()
        {
            var generated = Path.Combine(PrivateRoot, "generated");
            var input = Environment.GetEnvironmentVariable("MC_DAY_AUDIO_REPLAY") ??
                Path.Combine(generated, "gameplay-audio-20261117/current-day-entry.mcs");
            var temporary = Path.Combine(Path.GetTempPath(), "mc-day-shore-audio-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(temporary);
            var root = new GameObject("Day shore real runtime audio");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            var activeBefore = NativeSession.ActiveSessionCount;
            try
            {
                var arguments = new List<string> {
                    "-mc-game-runtime", "-mc-runtime-replay-load", input,
                    "-mc-runtime-loading-cache", Path.Combine(generated, "mc2-level1-night-cache-v2"),
                    "-mc-runtime-menu-cache", Environment.GetEnvironmentVariable("MC_CAMPAIGN_MENU_CACHE") ??
                        Path.Combine(generated, "mc-shared-source-map-motion-20261026"),
                    "-mc-runtime-speech-cache", Path.Combine(generated, "mc2-campaign-speech-current"),
                    "-mc-runtime-media-cache", Path.Combine(generated, "mc2-campaign-media-cache-decoded"),
                    "-mc-preferences-profile", Path.Combine(temporary, "profile")
                };
                foreach (var pair in new[] {
                    ("settlement-20261105-integration/owner-closed-v6/level1", "settlement-20261105-integration/owner-closed-v6/environment-complete/cache-level1"),
                    ("mc-level2-vault-successor-closure-20261025", "mc-b9s2-cursor-level2-vault-20261029"),
                    ("settlement-20261105-integration/owner-closed-v6/level3", "settlement-20261105-integration/owner-closed-v6/environment-complete/cache-level3")
                })
                    arguments.AddRange(new[] { "-mc-runtime-package", Path.Combine(generated, pair.Item1),
                        "-mc-runtime-owner-cache", Path.Combine(generated, pair.Item2),
                        "-mc-runtime-presentation-cache", Path.Combine(generated, pair.Item2) });
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(arguments.ToArray());
                const System.Reflection.BindingFlags flags = System.Reflection.BindingFlags.Instance |
                    System.Reflection.BindingFlags.NonPublic;
                var campaign = (NativeCampaignSession)typeof(MagicCarpetRuntime).GetField("campaignSession", flags).GetValue(runtime);
                var audio = (OwnerAudioPresentation)typeof(MagicCarpetRuntime).GetField("ownerAudio", flags).GetValue(runtime);
                Assert.That((campaign.Selection.externalLevel, campaign.Selection.mapType), Is.EqualTo(((ushort)3, (byte)0)));
                var terrain = campaign.TerrainPlane(NativeSim.TerrainTypes);
                byte CurrentTerrain() {
                    var pose = runtime.GameRuntimeProductionCampaignSnapshot.value.player_pose;
                    return terrain[(pose.y >> 8) * 256 + (pose.x >> 8)];
                }
                Assert.That(CurrentTerrain(), Is.Not.Zero, "the real Day entry begins over land");
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(audio.QualificationLoopTargetUnits(2), Is.EqualTo(70));
                var landVoice = audio.QualificationLoopVoiceIndex(2);
                Assert.That(landVoice, Is.GreaterThanOrEqualTo(0));
                Assert.That(audio.QualificationLoopPlayStarts(2), Is.EqualTo(1));
                audio.Tick(0.5f);
                Assert.That(audio.QualificationLoopUnits(2), Is.EqualTo(30), "source ambience advances at 60 Hz");
                audio.Tick(1f);
                Assert.That(audio.QualificationLoopUnits(2), Is.EqualTo(70));
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(audio.QualificationLoopVoiceIndex(2), Is.EqualTo(landVoice));
                Assert.That(audio.QualificationLoopPlayStarts(2), Is.EqualTo(1), "repeated source prepares reuse the live loop");
                var steer = typeof(MagicCarpetRuntime).GetMethod("ApplyGameRuntimeSteeringInput", flags);
                var fullStop = typeof(MagicCarpetRuntime).GetField("preferenceFullStopRequested", flags);
                void Turn(int frames)
                {
                    // Same accumulated keyboard input consumed by the real
                    // fixed-update route; no pose or audio state is injected.
                    steer.Invoke(runtime, new object[] { 0f, 0f, false, true, false, false, false, 127f });
                    runtime.QualificationDriveGameRuntimeFrames(frames);
                    steer.Invoke(runtime, new object[] { 0f, 0f, false, false, false, false, true, 0f });
                    runtime.QualificationDriveGameRuntimeFrames(16);
                    fullStop.SetValue(runtime, true);
                    runtime.QualificationDriveGameRuntimeFrames(1);
                }
                void Accelerate(int frames)
                {
                    runtime.QualificationApplyGameRuntimeProductionMovement(true, false, false, false);
                    runtime.QualificationDriveGameRuntimeFrames(frames);
                    runtime.QualificationApplyGameRuntimeProductionMovement(false, false, false, false);
                    fullStop.SetValue(runtime, true);
                    runtime.QualificationDriveGameRuntimeFrames(1);
                }
                Turn(27);
                Turn(6);
                Accelerate(30);
                Assert.That(CurrentTerrain(), Is.Zero, "ordinary flight reaches real Day water");
                Assert.That(audio.QualificationLoopTargetUnits(2), Is.Zero, "water fades the land loop");
                Assert.That(audio.QualificationLoopTargetUnits(1), Is.EqualTo(70), "water prepares the ocean loop");
                var waterVoice = audio.QualificationLoopVoiceIndex(1);
                Assert.That(waterVoice, Is.GreaterThanOrEqualTo(0).And.Not.EqualTo(landVoice));
                Assert.That(audio.QualificationLoopPlayStarts(1), Is.EqualTo(1));
                audio.Tick(0.5f);
                Assert.That((audio.QualificationLoopUnits(2), audio.QualificationLoopUnits(1)), Is.EqualTo((40, 30)));
                audio.Tick(1f);
                Assert.That((audio.QualificationLoopUnits(2), audio.QualificationLoopUnits(1)), Is.EqualTo((0, 70)));
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That((audio.QualificationLoopVoiceIndex(1), audio.QualificationLoopPlayStarts(1)), Is.EqualTo((waterVoice, 1)));
                Turn(33);
                Accelerate(30);
                Assert.That(CurrentTerrain(), Is.Not.Zero, "ordinary return flight reaches real Day land");
                Assert.That((audio.QualificationLoopTargetUnits(1), audio.QualificationLoopTargetUnits(2)), Is.EqualTo((0, 70)));
                Assert.That((audio.QualificationLoopVoiceIndex(1), audio.QualificationLoopVoiceIndex(2)), Is.EqualTo((waterVoice, landVoice)));
                Assert.That((audio.QualificationLoopPlayStarts(1), audio.QualificationLoopPlayStarts(2)), Is.EqualTo((1, 1)));
                audio.Tick(0.5f);
                Assert.That((audio.QualificationLoopUnits(1), audio.QualificationLoopUnits(2)), Is.EqualTo((40, 30)));
                audio.Tick(1f);
                Assert.That((audio.QualificationLoopUnits(1), audio.QualificationLoopUnits(2)), Is.EqualTo((0, 70)));
                runtime.QualificationResetGameRuntimeProduction();
                Assert.That(audio.QualificationActiveLoopCount, Is.Zero, "reset releases the old generation's device voices");
            }
            finally
            {
                runtime.QualificationDisposeGameRuntimeProduction();
                UnityEngine.Object.DestroyImmediate(root);
                Directory.Delete(temporary, true);
            }
            Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore));
        }

        [Test]
        public void MusicWaveHashFailureReportsOriginalAndFreshBytesWithoutAcceptingReread()
        {
            var original = System.Text.Encoding.ASCII.GetBytes("abc");
            var expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
            var reads = 0;
            OwnerAudioPresentation.RequireMusicWaveHash(original, expected, "/cache/music.wav",
                () => { reads++; return original; });
            Assert.That(reads, Is.Zero, "valid payload never triggers diagnostic reread");

            var error = Assert.Throws<InvalidDataException>(() =>
                OwnerAudioPresentation.RequireMusicWaveHash(
                    System.Text.Encoding.ASCII.GetBytes("bad"), expected, "/cache/music.wav",
                    () => { reads++; return original; }));
            Assert.That(reads, Is.EqualTo(1));
            Assert.That(error.Message, Does.Contain("path=/cache/music.wav"));
            Assert.That(error.Message, Does.Contain("expected_sha256=" + expected));
            Assert.That(error.Message, Does.Contain("actual_sha256=2f05d4b689d270cafb02285f35f44866f7dc8a2d368a3f9d1124373eeab31fb1"));
            Assert.That(error.Message, Does.Contain("buffer_length=3"));
            Assert.That(error.Message, Does.Contain("same_buffer_sha256=2f05d4b689d270cafb02285f35f44866f7dc8a2d368a3f9d1124373eeab31fb1"));
            Assert.That(error.Message, Does.Contain("fresh_length=3 fresh_sha256=" + expected));
            Assert.That(error.Message, Does.Contain("hash_implementation=System.Security.Cryptography."));
        }

        [Test]
        public void MusicWaveHashFailureRemainsFailureWhenDiagnosticReadFails()
        {
            var error = Assert.Throws<InvalidDataException>(() =>
                OwnerAudioPresentation.RequireMusicWaveHash(new byte[] { 1 }, "unexpected",
                    "/cache/music.wav", () => throw new IOException("read failed")));
            Assert.That(error.Message, Does.Contain("owner audio contract failed: destination music WAVE content hash"));
            Assert.That(error.Message, Does.Contain("fresh_read_error=System.IO.IOException"));
        }

        [Test]
        public void AudioReplacementKeepsPreviousPresenterWhenDestinationFails()
        {
            var cache = OwnerAssetCache.OpenAndValidateEnvironment(
                Path.Combine(PrivateRoot, "generated/mc-oi68-source31-fullbank-v2-20261029"),
                SecretSource31Package);
            var selection = NativeSim.CampaignSelection.Create(32, 31, 1);
            var root = new GameObject("transactional campaign audio");
            OwnerAudioPresentation current = null;
            try
            {
                current = OwnerAudioPresentation.LoadCampaignEnvironment(cache, selection, root.transform);
                var previous = current;
                var previousRoot = root.transform.GetChild(0).gameObject;
                Assert.Throws<InvalidDataException>(() => OwnerAudioPresentation.Replace(ref current,
                    () => OwnerAudioPresentation.LoadCampaignEnvironment(cache, selection, root.transform,
                        qualificationFailAfterSoundRows: 2), _ => { }));
                Assert.That(current, Is.SameAs(previous));
                Assert.That(current.QualificationSoundCount, Is.EqualTo(64));
                Assert.That(root.transform.childCount, Is.EqualTo(1));

                Assert.Throws<InvalidDataException>(() => OwnerAudioPresentation.Replace(ref current,
                    () => OwnerAudioPresentation.LoadCampaignEnvironment(cache, selection, root.transform),
                    _ => throw new InvalidDataException("invalid replacement state")));
                Assert.That(current, Is.SameAs(previous));
                Assert.That(root.transform.childCount, Is.EqualTo(1));

                OwnerAudioPresentation.Replace(ref current,
                    () => OwnerAudioPresentation.LoadCampaignEnvironment(cache, selection, root.transform),
                    _ => { });
                Assert.That(current, Is.Not.SameAs(previous));
                Assert.That(current.QualificationSoundCount, Is.EqualTo(64));
                Assert.That(previousRoot == null, Is.True, "replaced audio root is destroyed");
                Assert.That(root.transform.childCount, Is.EqualTo(1));
            }
            finally
            {
                current?.Dispose();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        public void SecretNightFullBankPlaysSpiderAndRejectsMismatchedSidecar()
        {
            var cache = OwnerAssetCache.OpenAndValidateEnvironment(
                Path.Combine(PrivateRoot, "generated/mc-oi68-source31-fullbank-v2-20261029"),
                SecretSource31Package);
            var root = new GameObject("Secret Night full sound bank");
            try
            {
                var completions = 0;
                using var audio = OwnerAudioPresentation.LoadCampaignEnvironment(
                    cache, NativeSim.CampaignSelection.Create(32, 31, 1), root.transform,
                    (_, _, _) => completions++);
                Assert.That(audio.QualificationSoundCount, Is.EqualTo(64));
                Assert.That(audio.QualificationHasSound(32), Is.True);
                var sound = new NativeSim.NativeEvent {
                    abiVersion = NativeSim.AbiVersion, structSize = 72,
                    generation = 1, sequence = 1, stateHash = 1,
                    kind = NativeSim.EventPrepareSound, code = 32,
                    entityIndex = 17, playerIndex = 0, assetRefIndex = 32,
                    reserved = new uint[4]
                };
                var command = new NativeSim.AudioCommand {
                    abiVersion = NativeSim.AbiVersion, structSize = 88,
                    generation = 1, eventSequence = 1, stateHash = 1,
                    assetRefIndex = 32, playbackRateHz = 22050, soundId = 32,
                    entityIndex = 17, playerIndex = 0, volumeQ15 = 0x7fff,
                    panQ16 = 0x8000, voiceIndex = 0,
                    playType = NativeSim.AudioEntitySoundIfNotPlaying,
                    loopCount = 0, action = NativeSim.AudioActionStart,
                    reserved = new uint[4]
                };
                var mismatched = command;
                mismatched.soundId = 38;
                Assert.Throws<InvalidDataException>(() => audio.Consume(sound, mismatched, false));
                Assert.That(audio.QualificationPositionalPlayStarts(32), Is.EqualTo(0));
                audio.Consume(sound, command, false);
                Assert.That(audio.QualificationPositionalPlayStarts(32), Is.EqualTo(1));
                var falseNull = command;
                falseNull.action = NativeSim.AudioActionSuppressNullSample;
                falseNull.voiceIndex = NativeSim.AudioVoiceNone;
                Assert.Throws<InvalidDataException>(() => audio.Consume(sound, falseNull));

                // Bank1 record7 is a real NULL.WAV row, distinct from the
                // historical special-case record12. Its sidecar stays bound
                // to the event, but it owns no voice or completion callback.
                var nullSound = sound;
                nullSound.sequence = 2;
                nullSound.code = 7;
                nullSound.assetRefIndex = 7;
                var nullCommand = command;
                nullCommand.eventSequence = 2;
                nullCommand.soundId = 7;
                nullCommand.assetRefIndex = 7;
                nullCommand.action = NativeSim.AudioActionSuppressNullSample;
                nullCommand.voiceIndex = NativeSim.AudioVoiceNone;
                var wrongVoice = nullCommand;
                wrongVoice.voiceIndex = 0;
                Assert.Throws<InvalidDataException>(() => audio.Consume(nullSound, wrongVoice));
                var wrongAction = nullCommand;
                wrongAction.action = NativeSim.AudioActionStart;
                Assert.Throws<InvalidDataException>(() => audio.Consume(nullSound, wrongAction));
                audio.Consume(nullSound, nullCommand);
                audio.Tick(0);
                Assert.That(audio.QualificationPositionalPlayStarts(7), Is.EqualTo(0));
                Assert.That(completions, Is.EqualTo(0));
                Assert.That(audio.HadSuppressedUnknown, Is.False);
            }
            finally { UnityEngine.Object.DestroyImmediate(root); }
        }

        [Test]
        public void FiniteSourceSamplesKeepEndRequestsAndExactVolumeStopsSeparateFromAmbientFades()
        {
            var cache = OwnerAssetCache.OpenAndValidateEnvironment(
                Path.Combine(PrivateRoot, "generated/mc-oi68-source31-fullbank-v2-20261029"),
                SecretSource31Package);
            var root = new GameObject("source finite sample EndLoop consumer");
            var completions = 0;
            try
            {
                using var audio = OwnerAudioPresentation.LoadCampaignEnvironment(cache,
                    NativeSim.CampaignSelection.Create(32, 31, 1), root.transform,
                    (_, _, _) => completions++);
                var ambient = SourceAudioEvent(1, 1);
                var ambientCommand = SourceAudioCommand(ambient, 1, 1);
                ambientCommand.playType = NativeSim.AudioPlayIfNotPlaying;
                ambientCommand.loopCount = -1;
                audio.Consume(ambient, ambientCommand);
                audio.Tick(0.5f);
                Assert.That(audio.QualificationLoopUnits(1), Is.EqualTo(30), "ambient callback remains 60 Hz");
                ulong sequence = 2;
                foreach (var soundId in new ushort[] { 47, 49 })
                {
                    var start = SourceAudioEvent(soundId, sequence++);
                    var command = SourceAudioCommand(start, soundId, 0);
                    command.panQ16 = 0xffff;
                    audio.Consume(start, command);
                    var source = Array.Find(root.GetComponentsInChildren<AudioSource>(),
                        candidate => candidate.clip != null && candidate.clip.channels == 2 &&
                            candidate.clip.frequency == 22050 && candidate.clip.samples == (soundId == 47 ? 48827 : 50757));
                    Assert.That(source, Is.Not.Null);
                    Assert.That(source.loop, Is.False, "source loop_count0 is a finite sample");
                    Assert.That(audio.QualificationPositionalPan(soundId), Is.Zero,
                        "PlayEntitySounds case4 uses pan64 even for an off-center preparation");
                    var clip = source.clip;
                    var volume = source.volume;
                    var end = SourceAudioEvent(soundId, sequence++, NativeSim.EventEndLoop);
                    var endCommand = SourceAudioCommand(end, soundId, 0);
                    endCommand.action = NativeSim.AudioActionEndLoop;
                    audio.Consume(end, endCommand);
                    Assert.That(source.clip, Is.SameAs(clip), "EndLoop is not the volume512 EndSample edge");
                    Assert.That(source.volume, Is.EqualTo(volume));
                    Assert.That(audio.QualificationLoopVoiceIndex(soundId), Is.EqualTo(-1),
                        "a finite sample must never become an ambient envelope");
                    var replaced = SourceAudioEvent(soundId, sequence++);
                    var replacedCommand = SourceAudioCommand(replaced, soundId, NativeSim.AudioVoiceNone);
                    replacedCommand.volumeQ15 = 512;
                    replacedCommand.action = NativeSim.AudioActionSuppressPendingReplaced;
                    audio.Consume(replaced, replacedCommand);
                    Assert.That(source.clip, Is.SameAs(clip),
                        "a losing volume512 preparation must not become a source stop");
                    var stop = SourceAudioEvent(soundId, sequence++);
                    var stopCommand = SourceAudioCommand(stop, soundId, 0);
                    stopCommand.volumeQ15 = 512;
                    stopCommand.action = NativeSim.AudioActionEndLoop;
                    audio.Consume(stop, stopCommand);
                    Assert.That(source.clip, Is.Null);
                    Assert.That(completions, Is.Zero,
                        "the exact-volume source stop already retired its native voice");
                    stop.sequence = sequence++;
                    stopCommand.eventSequence = stop.sequence;
                    stopCommand.action = NativeSim.AudioActionSuppressNoExisting;
                    stopCommand.voiceIndex = NativeSim.AudioVoiceNone;
                    audio.Consume(stop, stopCommand);
                }
                audio.Tick(0.5f);
                Assert.That(audio.QualificationLoopUnits(1), Is.EqualTo(60),
                    "finite EndLoop requests do not speed up or retarget ambience");
                audio.Reset();
                audio.Tick(1f);
                Assert.That(audio.QualificationActiveLoopCount, Is.Zero);
                Assert.That(completions, Is.Zero, "reset releases device ownership without stale completion");
            }
            finally { UnityEngine.Object.DestroyImmediate(root); }
        }

        [UnityTest]
        public IEnumerator FiniteSourceSampleCompletesItsStartIdentityAfterEndRequestAndGenerationRebind()
        {
            var cache = OwnerAssetCache.OpenAndValidateEnvironment(
                Path.Combine(PrivateRoot, "generated/mc-oi68-source31-fullbank-v2-20261029"),
                SecretSource31Package);
            var root = new GameObject("source finite sample natural completion");
            var completed = new List<(ulong generation, ushort voice, ulong sequence)>();
            OwnerAudioPresentation audio = null;
            try
            {
                audio = OwnerAudioPresentation.LoadCampaignEnvironment(cache,
                    NativeSim.CampaignSelection.Create(32, 31, 1), root.transform,
                    (generation, voice, sequence) => completed.Add((generation, voice, sequence)));
                var start = SourceAudioEvent(49, 1);
                audio.Consume(start, SourceAudioCommand(start, 49, 0));
                var duplicate = SourceAudioEvent(49, 2);
                var duplicateCommand = SourceAudioCommand(duplicate, 49, 0);
                duplicateCommand.action = NativeSim.AudioActionSuppressExisting;
                audio.Consume(duplicate, duplicateCommand);
                Assert.That(audio.QualificationPositionalPlayStarts(49), Is.EqualTo(1));
                audio.RebindCampaignGeneration(8);
                var end = SourceAudioEvent(49, 3, NativeSim.EventEndLoop);
                end.generation = 8;
                var endCommand = SourceAudioCommand(end, 49, 0);
                endCommand.action = NativeSim.AudioActionEndLoop;
                audio.Consume(end, endCommand);
                var deadline = Time.realtimeSinceStartup + 5f;
                while (completed.Count == 0 && Time.realtimeSinceStartup < deadline)
                {
                    yield return null;
                    audio.Tick(0f);
                }
                CollectionAssert.AreEqual(new[] { (8UL, (ushort)0, 1UL) }, completed,
                    "natural finite completion owns the rebound generation and original start, not duplicate/end events");
                audio.Tick(0f);
                Assert.That(completed.Count, Is.EqualTo(1));
                audio.Reset();
                Assert.That(audio.QualificationPositionalIsPlaying(49), Is.False);
                audio.Tick(0f);
                Assert.That(completed.Count, Is.EqualTo(1));
            }
            finally
            {
                audio?.Dispose();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        private static NativeSim.NativeEvent SourceAudioEvent(ushort soundId, ulong sequence,
            uint kind = NativeSim.EventPrepareSound) => new NativeSim.NativeEvent {
            abiVersion = NativeSim.AbiVersion, structSize = 72, generation = 7,
            sequence = sequence, stateHash = 123, kind = kind, code = soundId,
            entityIndex = 321, playerIndex = -1, assetRefIndex = soundId, reserved = new uint[4]
        };

        private static NativeSim.AudioCommand SourceAudioCommand(NativeSim.NativeEvent sound,
            ushort soundId, ushort voiceIndex) => new NativeSim.AudioCommand {
            abiVersion = NativeSim.AbiVersion, structSize = 88, generation = sound.generation,
            eventSequence = sound.sequence, stateHash = sound.stateHash,
            soundId = soundId, entityIndex = sound.entityIndex, playerIndex = sound.playerIndex,
            assetRefIndex = sound.assetRefIndex, playbackRateHz = 22050,
            volumeQ15 = 0x7fff, panQ16 = 0x7fff, voiceIndex = voiceIndex,
            playType = NativeSim.AudioEntitySoundFiniteIfNotPlaying, loopCount = 0,
            action = NativeSim.AudioActionStart, reserved = new uint[4]
        };

        [Test]
        public void CaveLoadsSourceBankTwoAndLogicalTrackThreeFromItsOwnEnvironment()
        {
            var cache = OwnerAssetCache.OpenAndValidateEnvironment(
                Path.Combine(PrivateRoot, "generated/mc-oi62-environment-level4-audio-v3"),
                Path.Combine(PrivateRoot, "generated/mc-bhq6-ordinary-20261025-zclu5fjj/level4"));
            var root = new GameObject("Cave source audio consumer");
            try
            {
                using var audio = OwnerAudioPresentation.LoadCampaignEnvironment(cache,
                    NativeSim.CampaignSelection.Create(4, 3, 2), root.transform);
                Assert.That(audio.QualificationSoundCount, Is.EqualTo(69));
                Assert.That(audio.QualificationHasSound(32), Is.True, "authored Spider sound");
                var sources = root.GetComponentsInChildren<AudioSource>();
                var music = Array.Find(sources, source => source.clip != null && source.clip.channels == 2);
                Assert.That(music, Is.Not.Null);
                Assert.That(music.clip.name, Does.StartWith("C2GAME3"));
                Assert.That(music.clip.samples, Is.EqualTo(10584064));
                Assert.That(music.clip.frequency, Is.EqualTo(44100));
                var drip = new NativeSim.NativeEvent {
                    abiVersion = NativeSim.AbiVersion, structSize = 72,
                    generation = 1, sequence = 1, stateHash = 1,
                    kind = NativeSim.EventPrepareSound, code = 67,
                    entityIndex = 0, playerIndex = 0, assetRefIndex = 67,
                    reserved = new uint[4]
                };
                var command = new NativeSim.AudioCommand {
                    abiVersion = NativeSim.AbiVersion, structSize = 88,
                    generation = 1, eventSequence = 1, stateHash = 1,
                    assetRefIndex = 67, playbackRateHz = 22050, soundId = 67,
                    entityIndex = 0, playerIndex = 0, volumeQ15 = 0x7fff,
                    panQ16 = 0x7fff, voiceIndex = 0,
                    playType = NativeSim.AudioEntitySoundIfNotPlaying,
                    loopCount = 0, action = NativeSim.AudioActionStart,
                    reserved = new uint[4]
                };
                audio.Consume(drip, command, reportCompletion: false);
                Assert.That(audio.QualificationPositionalPlayStarts(67), Is.EqualTo(1));
                Assert.That(audio.HadSuppressedUnknown, Is.False);
            }
            finally { UnityEngine.Object.DestroyImmediate(root); }
        }

        [Test]
        public void SecretSource31NightAndSource34DayLoadTheirSelectedEnvironmentAudio()
        {
            var nightCache = OwnerAssetCache.OpenAndValidateEnvironment(
                SecretSource31NightCache, SecretSource31Package);
            var nightRoot = new GameObject("Secret source 31 Night audio consumer");
            try
            {
                using var audio = OwnerAudioPresentation.LoadCampaignEnvironment(
                    nightCache,
                    NativeSim.CampaignSelection.Create(32, 31, 1),
                    nightRoot.transform);
                Assert.That(audio.QualificationSoundCount, Is.EqualTo(32));
                Assert.That(audio.QualificationHasSound(12), Is.True,
                    "Night sound 12 remains an explicit null source row");
                Assert.That(audio.QualificationHasSound(13), Is.False);
                var music = Array.Find(nightRoot.GetComponentsInChildren<AudioSource>(),
                    source => source.clip != null && source.clip.channels == 2);
                Assert.That(music, Is.Not.Null);
                Assert.That(music.clip.name, Does.StartWith("C2GAME1"));
                Assert.That(music.clip.samples, Is.EqualTo(19472384));
                Assert.That(music.clip.frequency, Is.EqualTo(44100));
            }
            finally { UnityEngine.Object.DestroyImmediate(nightRoot); }

            var dayCache = OwnerAssetCache.OpenAndValidateEnvironment(
                SecretSource34DayCache, SecretSource34Package);
            var dayRoot = new GameObject("Secret source 34 Day audio consumer");
            try
            {
                using var audio = OwnerAudioPresentation.LoadCampaignEnvironment(
                    dayCache,
                    NativeSim.CampaignSelection.Create(35, 34, 0),
                    dayRoot.transform);
                Assert.That(audio.QualificationSoundCount, Is.EqualTo(29));
                Assert.That(audio.QualificationHasSound(61), Is.True);
                Assert.That(audio.QualificationHasSound(63), Is.False);
                var music = Array.Find(dayRoot.GetComponentsInChildren<AudioSource>(),
                    source => source.clip != null && source.clip.channels == 2);
                Assert.That(music, Is.Not.Null);
                Assert.That(music.clip.name, Does.StartWith("C2GAME2"));
                Assert.That(music.clip.samples, Is.EqualTo(16695296));
                Assert.That(music.clip.frequency, Is.EqualTo(44100));
            }
            finally { UnityEngine.Object.DestroyImmediate(dayRoot); }
        }

        [Test]
        public void SecretEnvironmentAudioRejectsWrongSelectionAndPackage()
        {
            var cache = OwnerAssetCache.OpenAndValidateEnvironment(
                SecretSource31NightCache, SecretSource31Package);
            var root = new GameObject("Secret source audio refusal test");
            try
            {
                var wrongSelection = Assert.Throws<InvalidDataException>(
                    () => OwnerAudioPresentation.LoadCampaignEnvironment(
                        cache,
                        NativeSim.CampaignSelection.Create(32, 31, 0),
                        root.transform));
                StringAssert.Contains("qualified destination-bound campaign",
                    wrongSelection.Message);

                var wrongPackage = Assert.Throws<InvalidDataException>(
                    () => OwnerAssetCache.OpenAndValidateEnvironment(
                        SecretSource31NightCache, SecretSource34Package));
                StringAssert.Contains("campaign package definition does not match",
                    wrongPackage.Message);
            }
            finally { UnityEngine.Object.DestroyImmediate(root); }
        }

        [Test]
        public void Level1LoaderRetainsItsExactNightDescriptorAndExplicitNullSound()
        {
            var cache = OwnerAssetCache.OpenAndValidate(
                Level1OwnerCache,
                "730d2fa084265df839f3f2b55ed766ac167a8b152ca4631b5ded9d0bb9104ac8",
                "878123908f6de05f684f4cd1d73cff8d2ff08701cb6fdd12b7165bfcacc75719");
            var root = new GameObject("Level 1 owner audio regression test");
            OwnerAudioPresentation audio = null;
            try
            {
                audio = OwnerAudioPresentation.Load(
                    cache,
                    "audio/sound-bank-1-822.json",
                    "audio/music-channel-0-f.json",
                    root.transform);
                Assert.That(audio.QualificationSoundCount, Is.EqualTo(32));
                Assert.That(audio.QualificationHasSound(12), Is.True);
                Assert.That(audio.QualificationHasSound(13), Is.False);

                audio.Consume(new NativeSim.NativeEvent {
                    abiVersion = NativeSim.AbiVersion,
                    structSize = 72,
                    generation = 1,
                    sequence = 1,
                    stateHash = 1,
                    kind = NativeSim.EventPrepareSound,
                    code = 12,
                    assetRefIndex = 84,
                    reserved = new uint[4]
                });
                Assert.That(audio.HadSuppressedUnknown, Is.False);
            }
            finally
            {
                audio?.Dispose();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        public void Level3DayCacheLoadsExactSoundsAndPlaysNativeSoundAndMusicEvents()
        {
            var cache = OwnerAssetCache.OpenAndValidateLevel3(Level3OwnerCache);
            var root = new GameObject("Level 3 owner audio test");
            OwnerAudioPresentation audio = null;
            try
            {
                audio = OwnerAudioPresentation.LoadCampaignEnvironment(cache, root.transform);
                Assert.That(audio.QualificationSoundCount, Is.EqualTo(29));
                var music = Array.Find(root.GetComponentsInChildren<AudioSource>(),
                    source => source.clip != null && source.clip.channels == 2);
                Assert.That(music.clip.name, Does.StartWith("C2GAME2"));
                Assert.That(music.clip.samples, Is.EqualTo(16695296));
                Assert.That(audio.QualificationHasSound(12), Is.True);
                Assert.That(audio.QualificationHasSound(13), Is.True);
                Assert.That(audio.QualificationHasSound(60), Is.True);
                Assert.That(audio.QualificationHasSound(63), Is.False);

                const ulong generation = 4;
                const ulong sequence = 19;
                const ulong stateHash = 0x1234_5678_9abc_def0;
                const uint assetRef = 501;
                var soundEvent = new NativeSim.NativeEvent {
                    abiVersion = NativeSim.AbiVersion,
                    structSize = 72,
                    generation = generation,
                    sequence = sequence,
                    stateHash = stateHash,
                    kind = NativeSim.EventPrepareSound,
                    code = 60,
                    entityIndex = 17,
                    playerIndex = 0,
                    assetRefIndex = assetRef,
                    reserved = new uint[4]
                };
                var command = new NativeSim.AudioCommand {
                    abiVersion = NativeSim.AbiVersion,
                    structSize = 88,
                    generation = generation,
                    eventSequence = sequence,
                    stateHash = stateHash,
                    assetRefIndex = assetRef,
                    playbackRateHz = 22050,
                    soundId = 60,
                    entityIndex = 17,
                    playerIndex = 0,
                    volumeQ15 = 0x7fff,
                    panQ16 = 0x8000,
                    voiceIndex = 0,
                    playType = NativeSim.AudioEntitySoundRestartOrStart,
                    loopCount = 0,
                    action = NativeSim.AudioActionStart,
                    pooledFlushRank = 0,
                    reserved = new uint[4]
                };
                audio.Consume(soundEvent, command, reportCompletion: false);
                Assert.That(audio.QualificationPositionalPlayStarts(60), Is.EqualTo(1));
                Assert.That(audio.HadSuppressedUnknown, Is.False);

                audio.Consume(new NativeSim.NativeEvent {
                    abiVersion = NativeSim.AbiVersion,
                    structSize = 72,
                    generation = generation,
                    sequence = sequence + 1,
                    stateHash = stateHash,
                    kind = NativeSim.EventStartMusic,
                    code = 1,
                    value0 = 0,
                    value1 = 1,
                    assetRefIndex = assetRef,
                    reserved = new uint[4]
                });
                Assert.That(audio.QualificationMusicStartCount, Is.EqualTo(1));
                Assert.That(audio.QualificationMusicUpdatesEnabled, Is.True);

                audio.EndCampaignSamples();
                var retained = Array.FindAll(root.GetComponentsInChildren<AudioSource>(),
                    source => source.clip != null);
                Assert.That(retained.Length, Is.EqualTo(1),
                    "EndSample retires every device sample while leaving music to its Rust projection");
                Assert.That(retained[0].clip.name, Does.StartWith("C2GAME2"));
                Assert.That(audio.QualificationMusicStartCount, Is.EqualTo(1),
                    "sample cleanup must not invent a music restart");
            }
            finally
            {
                audio?.Dispose();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        public void Level3DayLoaderRejectsWrongLevelAndPostValidationDescriptorTamper()
        {
            var wrongCache = OwnerAssetCache.OpenAndValidateLevel2(Level2OwnerCache);
            var root = new GameObject("Level 3 owner audio refusal test");
            try
            {
                var wrongLevel = Assert.Throws<InvalidDataException>(
                    () => OwnerAudioPresentation.LoadCampaignEnvironment(
                        wrongCache,
                        root.transform));
                StringAssert.Contains("Day/Cave", wrongLevel.Message);

                var tamperedRoot = Path.Combine(
                    Path.GetTempPath(),
                    $"magiccarpet-level3-audio-{Guid.NewGuid():N}");
                CopyTree(Level3OwnerCache, tamperedRoot);
                try
                {
                    var tamperedCache = OwnerAssetCache.OpenAndValidateLevel3(tamperedRoot);
                    var descriptorPath = Path.Combine(
                        tamperedRoot,
                        "components/audio/audio/descriptor.json");
                    var descriptor = File.ReadAllText(descriptorPath);
                    File.WriteAllText(
                        descriptorPath,
                        descriptor.Replace("mc2-day-audio/v1", "mc2-day-audio/x1"));
                    var tampered = Assert.Throws<InvalidDataException>(
                        () => OwnerAudioPresentation.LoadCampaignEnvironment(
                            tamperedCache,
                            root.transform));
                    StringAssert.Contains("Day/Cave audio descriptor", tampered.Message);
                }
                finally
                {
                    Directory.Delete(tamperedRoot, true);
                }
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        private static void CopyTree(string sourceRoot, string destinationRoot)
        {
            Directory.CreateDirectory(destinationRoot);
            foreach (var directory in Directory.GetDirectories(
                         sourceRoot,
                         "*",
                         SearchOption.AllDirectories))
            {
                var relative = directory.Substring(sourceRoot.Length)
                    .TrimStart(Path.DirectorySeparatorChar);
                Directory.CreateDirectory(Path.Combine(destinationRoot, relative));
            }
            foreach (var source in Directory.GetFiles(
                         sourceRoot,
                         "*",
                         SearchOption.AllDirectories))
            {
                var relative = source.Substring(sourceRoot.Length)
                    .TrimStart(Path.DirectorySeparatorChar);
                var destination = Path.Combine(destinationRoot, relative);
                Directory.CreateDirectory(Path.GetDirectoryName(destination));
                File.Copy(source, destination);
            }
        }
    }
}
