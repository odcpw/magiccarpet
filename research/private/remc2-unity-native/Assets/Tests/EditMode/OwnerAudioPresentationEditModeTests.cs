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
