using System;
using System.IO;
using NUnit.Framework;
using UnityEngine;

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
                using var audio = OwnerAudioPresentation.LoadCampaignEnvironment(
                    cache, NativeSim.CampaignSelection.Create(32, 31, 1), root.transform);
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
                Assert.That(audio.HadSuppressedUnknown, Is.False);
            }
            finally { UnityEngine.Object.DestroyImmediate(root); }
        }

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
