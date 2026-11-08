using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.Serialization;
using System.Runtime.Serialization.Json;
using System.Security.Cryptography;
using System.Threading;
using UnityEngine;

namespace MagicCarpet.Native
{
    /// <summary>
    /// Owner-local audio presentation. It consumes only level-bound validated
    /// cache outputs and typed simulation events; no owner bytes are embedded
    /// in the Unity project or build.
    /// </summary>
    internal sealed class OwnerAudioPresentation : IDisposable
    {
        private const uint FirstAudioAssetRef = 76;
        private const uint MusicAssetRef = 108;
        private const int MusicIdleMode = 1;
        private const int MusicCombatMode = 2;
        private static readonly int[] ExactSoundOrder =
        {
            1, 2, 3, 4, 5, 8, 9, 10, 12, 14, 15, 16, 18, 19, 27, 29, 30, 31,
            33, 34, 37, 39, 40, 41, 43, 44, 46, 54, 55, 56, 57, 63
        };
        private static readonly int[] ExactCampaignDaySoundOrder =
        {
            1, 2, 3, 5, 8, 9, 10, 12, 13, 14, 15, 16, 18, 19, 23, 29, 30, 31,
            40, 41, 43, 44, 47, 54, 55, 56, 57, 60, 61
        };
        private static readonly string[] ExactCampaignDaySoundFiles =
        {
            "OCEAN.WAV", "WIND.WAV", "EXPLOD3.WAV", "FIRE.WAV",
            "DRAGROAR.WAV", "FIREBAL1.WAV", "QUAKE4.WAV", "BEE1.WAV",
            "NULL.WAV", "SELECT.WAV", "FIREBAL2.WAV", "DIE.WAV",
            "GETSPELL.WAV", "SPEEDUP.WAV", "LITNING.WAV", "CANTUSE.WAV",
            "METEOREX.WAV", "MARKET.WAV", "MANASHOT.WAV", "SWITCH.WAV",
            "NULL.WAV", "NULL.WAV", "DOORC2.WAV", "HIT1.WAV",
            "HIT2.WAV", "HIT3.WAV", "HIT4.WAV", "MORPH.WAV", "SUCCESS2.WAV"
        };

        [Serializable]
        private sealed class SoundDescriptor
        {
            public string schema;
            public int bank;
            public int quality;
            public int quality_index;
            public SoundRow[] level1_assets;
        }

        [Serializable]
        private sealed class SoundRow
        {
            public int sound_id;
            public string semantic_name;
            public string play_class;
            public string playback_scope;
            public int loop_target_volume;
            public string asset_path;
            public string mapping_status;
        }

        [Serializable]
        private sealed class MusicDescriptor
        {
            public string schema;
            public int bank;
            public int channel;
            public string driver_type;
            public string driver;
            public int driver_index;
            public string map_type;
            public int maptype_music;
            public int logical_track;
            public int playback_slot;
            public int loop_scan_pointer_slot;
            public string unity_wave_path;
            public string unity_wave_sha256;
            public WaveDescriptor unity_wave;
            public string initial_start_status;
            public string optional_random_OGG_path;
            public string exact_DOS_hardware_timbre;
            public MusicDynamicControl dynamic_control;
            public MusicRecord[] records;
        }

        [Serializable]
        private sealed class MusicRecord
        {
            public int slot;
            public string filename;
            public string asset_path;
            public string asset_sha256;
        }

        [Serializable]
        private sealed class CampaignDayAudioDescriptor
        {
            public string schema;
            public CampaignDayProfile profile;
            public CampaignDaySoundDescriptor sound;
            public MusicDescriptor music;
        }

        [DataContract]
        private sealed class CampaignAudioNullableFieldProjection
        {
            [DataMember(Name = "profile", IsRequired = true)]
            public CampaignAudioProfileNullableFields profile;
            [DataMember(Name = "music", IsRequired = true)]
            public CampaignAudioMusicNullableFields music;
            [DataMember(Name = "sound", IsRequired = true)]
            public CampaignAudioSoundNullableFields sound;
        }

        [DataContract]
        private sealed class CampaignAudioProfileNullableFields
        {
            private string skyValue;
            public bool hasSky;

            [DataMember(Name = "sky")]
            public string sky
            {
                get => skyValue;
                set { skyValue = value; hasSky = true; }
            }
        }

        [DataContract]
        private sealed class CampaignAudioMusicNullableFields
        {
            [DataMember(Name = "records", IsRequired = true)]
            public CampaignAudioMusicRecordNullableFields[] records;
        }

        [DataContract]
        private sealed class CampaignAudioMusicRecordNullableFields
        {
            [DataMember(Name = "slot", IsRequired = true)]
            public int slot;
            [DataMember(Name = "asset_path", IsRequired = true)]
            public string asset_path;
            [DataMember(Name = "asset_sha256", IsRequired = true)]
            public string asset_sha256;
        }

        [DataContract]
        private sealed class CampaignAudioSoundNullableFields
        {
            [DataMember(Name = "records", IsRequired = true)]
            public CampaignAudioSoundRecordNullableFields[] records;
        }

        [DataContract]
        private sealed class CampaignAudioSoundRecordNullableFields
        {
            [DataMember(Name = "sound_id", IsRequired = true)]
            public int sound_id;
            [DataMember(Name = "asset_path", IsRequired = true)]
            public string asset_path;
            [DataMember(Name = "asset_sha256", IsRequired = true)]
            public string asset_sha256;
            [DataMember(Name = "wav", IsRequired = true)]
            public CampaignAudioWaveNullableFields wav;
        }

        [DataContract]
        private sealed class CampaignAudioWaveNullableFields
        {
            [DataMember(Name = "channels", IsRequired = true)]
            public int channels;
            [DataMember(Name = "sample_width_bytes", IsRequired = true)]
            public int sample_width_bytes;
            [DataMember(Name = "sample_rate", IsRequired = true)]
            public int sample_rate;
            [DataMember(Name = "frame_count", IsRequired = true)]
            public int frame_count;
        }

        [Serializable]
        private sealed class CampaignDayProfile
        {
            public int external_level, internal_level, vga_resolution_selector, texture_size;
            public int terrain_graphics_flags, key_color_1, key_color_2;
            public CampaignDayMapType map_type;
            public string palette, tables, colors, blocks, dormant_blocks16, sky, sprites, web, tmaps;
            public string campaign_package_sha256, owner_export_package_sha256, spec_commit;
        }

        [Serializable]
        private sealed class CampaignDayMapType
        {
            public int id;
            public string name;
        }

        [Serializable]
        private sealed class CampaignDaySoundDescriptor
        {
            public int bank;
            public int reachable_occurrence_count;
            public CampaignDaySoundRecord[] records;
        }

        [Serializable]
        private sealed class CampaignDaySoundRecord
        {
            public int sound_id;
            public int bank;
            public int quality;
            public int quality_index;
            public string filename;
            public string semantic_name;
            public string play_class;
            public string playback_scope;
            public int loop_target_volume;
            public string mapping_status;
            public string asset_path;
            public string asset_sha256;
            public WaveDescriptor wav;
        }

        [Serializable]
        private sealed class WaveDescriptor
        {
            public int channels;
            public int sample_width_bytes;
            public int sample_rate;
            public int frame_count;
        }

        [Serializable]
        private sealed class MusicDynamicControl
        {
            public string audible_channel_resolution;
            public int envelope_min;
            public int envelope_max;
            public int envelope_step_per_callback;
            public int timer_hz;
            public string timer_hz_formula;
            public int timer_multiplier;
            public string update_mode_field;
            public MusicControllerCensus playback_slot_census;
            public MusicControllerCensus loop_scan_pointer_census;
        }

        [Serializable]
        private sealed class MusicControllerCensus
        {
            public int division;
            public int midi_format;
            public int track_count;
            public int trigger_controller;
            public int[] tagged_channels;
            public MusicControllerEvent[] trigger_events;
            public MusicControllerEvent[] loop_controller_events;
        }

        [Serializable]
        private sealed class MusicControllerEvent
        {
            public int channel;
            public int controller;
            public int tick;
            public int value;
        }

        private sealed class LoopState
        {
            internal AudioSource Source;
            internal ushort VoiceIndex;
            internal int CurrentUnits;
            internal int TargetUnits;
            internal int PlayStarts;
            internal int LastTimeSamples;
            internal long ProgressSamples;
        }

        private sealed class PositionalVoice
        {
            internal AudioSource Source;
            internal AudioClip RenderedClip;
            internal int SoundId;
            internal int PlayStarts;
            internal int SuppressedExisting;
            internal float EffectiveVolume;
            internal float EffectivePan;
            internal float EffectivePitch;
            internal ulong Generation;
            internal ulong EventSequence;
            internal int StartFrame;
            internal bool WasPlaying;
            internal bool CompletionPending;
        }

        private sealed class StreamingPcm16
        {
            private readonly byte[] pcm;
            private readonly int channels;
            private readonly object positionLock = new();
            private int framePosition;
            private long readCalls;
            private long nonZeroSamples;

            internal StreamingPcm16(byte[] pcm, int channels)
            {
                this.pcm = pcm;
                this.channels = channels;
            }

            internal void Read(float[] output)
            {
                Interlocked.Increment(ref readCalls);
                long nonZero = 0;
                lock (positionLock)
                {
                    var samplePosition = framePosition * channels;
                    for (var index = 0; index < output.Length; index++)
                    {
                        var byteOffset = (samplePosition + index) * 2;
                        output[index] = byteOffset + 1 < pcm.Length
                            ? (short)(pcm[byteOffset] | (pcm[byteOffset + 1] << 8)) / 32768f
                            : 0f;
                        if (output[index] != 0f) nonZero++;
                    }
                    framePosition += output.Length / channels;
                }
                Interlocked.Add(ref nonZeroSamples, nonZero);
            }

            internal void Seek(int frame)
            {
                lock (positionLock)
                    framePosition = Mathf.Max(0, frame);
            }

            internal long ReadCalls => Interlocked.Read(ref readCalls);
            internal long NonZeroSamples => Interlocked.Read(ref nonZeroSamples);
        }

        private readonly GameObject root;
        private readonly Dictionary<int, AudioClip> clips = new();
        private readonly Dictionary<int, WaveData> waves = new();
        private readonly Dictionary<int, SoundRow> rows = new();
        private readonly HashSet<int> nullSamples = new();
        private readonly Dictionary<int, LoopState> loops = new();
        private readonly PositionalVoice[] positionalVoices;
        private readonly AudioSource musicSource;
        private readonly AudioClip musicClip;
        private readonly StreamingPcm16 musicStream;
        private readonly Action<ulong, ushort, ulong> voiceCompleted;
        private readonly bool legacyAssetReferences;
        private readonly byte campaignMusicTrack;
        private float fadeAccumulator;
        private float musicFadeAccumulator;
        private int oneShotPlayCount;
        private int musicStartCount;
        private bool musicUpdatesEnabled;
        private bool musicTimerActive;
        private bool campaignMusicDriven;
        private uint campaignMusicStartCount;
        private ulong campaignMusicGeneration;
        private int musicTargetMode = MusicIdleMode;
        private int musicCurrentUnits;
        private int musicDirection = -1;
        private int musicTimerMultiplier = 3;
        private bool disposed;
        private static int qualificationTransactionalCleanupCount;
        internal bool HadSuppressedUnknown { get; private set; }
        internal bool HadSuppressedUnsupportedPositional { get; private set; }
        private float preferenceSoundGain = 1f;
        private float preferenceMusicGain = 1f;
        private float speechSoundGain = 1f;
        private float speechMusicGain = 1f;
        private float campaignMusicGain = 1f;

        private float SoundGain => preferenceSoundGain * speechSoundGain;

        internal static void Replace(
            ref OwnerAudioPresentation current,
            Func<OwnerAudioPresentation> create,
            Action<OwnerAudioPresentation> configure)
        {
            OwnerAudioPresentation replacement = null;
            try
            {
                replacement = create();
                Require(replacement != null, "replacement audio presenter exists");
                configure(replacement);
            }
            catch
            {
                replacement?.Dispose();
                throw;
            }
            var previous = current;
            current = replacement;
            previous?.Dispose();
        }

        internal void ApplyPreferences(PlayerPreferences preferences)
        {
            preferenceSoundGain = preferences.sound_enabled ? preferences.sound_volume / 127f : 0;
            preferenceMusicGain = preferences.music_enabled ? preferences.music_volume / 127f : 0;
            ApplyMix();
        }

        internal void SetSpeechMix(float soundGain, float musicGain)
        {
            Require(!float.IsNaN(soundGain) && !float.IsInfinity(soundGain) &&
                !float.IsNaN(musicGain) && !float.IsInfinity(musicGain) &&
                soundGain >= 0f && soundGain <= 1f && musicGain >= 0f && musicGain <= 1f,
                "bounded source speech mix gains");
            speechSoundGain = soundGain;
            speechMusicGain = musicGain;
            ApplyMix();
        }

        private void ApplyMix()
        {
            musicSource.volume = 0.55f * preferenceMusicGain * speechMusicGain * campaignMusicGain;
            foreach (var voice in positionalVoices) voice.Source.volume = SoundGain;
            foreach (var state in loops.Values)
                state.Source.volume = state.CurrentUnits / 127f * SoundGain;
        }

        private OwnerAudioPresentation(
            GameObject root,
            AudioSource musicSource,
            AudioClip musicClip,
            StreamingPcm16 musicStream,
            Action<ulong, ushort, ulong> voiceCompleted,
            bool legacyAssetReferences = true,
            byte campaignMusicTrack = 1)
        {
            this.root = root;
            this.musicSource = musicSource;
            this.musicClip = musicClip;
            this.musicStream = musicStream;
            this.voiceCompleted = voiceCompleted;
            this.legacyAssetReferences = legacyAssetReferences;
            this.campaignMusicTrack = campaignMusicTrack;
            positionalVoices = new PositionalVoice[10];
            for (var index = 0; index < positionalVoices.Length; index++)
            {
                positionalVoices[index] = new PositionalVoice {
                    Source = AddSource(root, $"Original stereo voice {index}", false),
                    SoundId = -1
                };
            }
        }

        internal static OwnerAudioPresentation Load(
            OwnerAssetCache cache,
            string soundDescriptorPath,
            string musicDescriptorPath,
            Transform parent,
            Action<ulong, ushort, ulong> voiceCompleted = null,
            int qualificationFailAfterSoundRows = -1,
            bool legacyAssetReferences = true)
        {
            var sound = JsonUtility.FromJson<SoundDescriptor>(
                cache.ReadOutputText(soundDescriptorPath));
            var music = JsonUtility.FromJson<MusicDescriptor>(
                cache.ReadOutputText(musicDescriptorPath));
            Require(
                sound != null && sound.schema == "mc2-level1-sound-bank/v1" &&
                sound.bank == 1 && sound.quality == 822 && sound.quality_index == 3 &&
                sound.level1_assets != null &&
                sound.level1_assets.Length == ExactSoundOrder.Length,
                "qualified Night/822 sound descriptor");
            Require(
                music != null && music.schema == "mc2-level1-music-channel/v1" &&
                music.channel == 0 && music.driver_type == "f" &&
                music.driver_index == 2 && music.logical_track == 1 &&
                music.playback_slot == 0 && music.loop_scan_pointer_slot == 1 &&
                !string.IsNullOrWhiteSpace(music.unity_wave_path) &&
                music.unity_wave_sha256 ==
                    "03b3f951048504d8a6d30611d6c44a62ec1c463726b30c33c16e3cb69824605d" &&
                music.initial_start_status ==
                    "SourceOrderQualified_remc2_local_gameTurn1_after_second_DrawAndEvents" &&
                music.optional_random_OGG_path == "Excluded" &&
                music.exact_DOS_hardware_timbre == "Deferred",
                "qualified Level 1 music descriptor");
            ValidateMusicDynamicControl(music.dynamic_control);

            GameObject root = null;
            AudioSource musicSource = null;
            AudioClip musicClip = null;
            OwnerAudioPresentation presentation = null;
            try
            {
                root = new GameObject("Owner-local Level 1 audio");
                root.transform.SetParent(parent, false);
                musicSource = AddSource(
                    root, "C2GAME1 owner-local presentation adaptation", true);
                var musicWave = ParseWave(cache.ReadOutput(music.unity_wave_path));
                Require(
                    musicWave.Channels == 2 && musicWave.SampleWidth == 2 &&
                    musicWave.SampleRate == 44100 && musicWave.Frames == 19472384,
                    "qualified deterministic C2GAME1 PCM render");
                var stream = new StreamingPcm16(musicWave.Pcm, musicWave.Channels);
                musicClip = AudioClip.Create(
                    "C2GAME1 — deterministic ADLMIDI bank 59",
                    musicWave.Frames,
                    musicWave.Channels,
                    musicWave.SampleRate,
                    true,
                    stream.Read,
                    stream.Seek);
                musicSource.clip = musicClip;
                musicSource.volume = 0.55f;

                presentation = new OwnerAudioPresentation(
                    root, musicSource, musicClip, stream, voiceCompleted,
                    legacyAssetReferences: legacyAssetReferences);
                for (var index = 0; index < ExactSoundOrder.Length; index++)
                {
                    if (index == qualificationFailAfterSoundRows)
                        throw new InvalidDataException(
                            "qualification-forced owner-audio load failure");
                    var row = sound.level1_assets[index];
                    Require(row != null && row.sound_id == ExactSoundOrder[index],
                        $"sound descriptor row {index}");
                    Require(
                        row.playback_scope == ExpectedPlaybackScope(row.sound_id),
                        $"sound ID {row.sound_id} playback scope");
                    presentation.rows.Add(row.sound_id, row);
                    if (row.sound_id == 12)
                    {
                        Require(
                            row.mapping_status == "explicit_null" &&
                            string.IsNullOrEmpty(row.asset_path),
                            "sound ID 12 explicit null");
                        presentation.nullSamples.Add(row.sound_id);
                        continue;
                    }
                    Require(
                        row.mapping_status == "proven_Level1" &&
                        !string.IsNullOrWhiteSpace(row.asset_path),
                        $"sound ID {row.sound_id} materialization");
                    var wave = ParseWave(cache.ReadOutput(row.asset_path));
                    Require(
                        wave.Channels == 1 && wave.SampleWidth == 1 &&
                        wave.SampleRate == 22050,
                        $"sound ID {row.sound_id} exact PCM format");
                    presentation.waves.Add(row.sound_id, wave);
                    presentation.clips.Add(
                        row.sound_id,
                        CreatePcm8Clip(row.semantic_name, wave));
                }
                return presentation;
            }
            catch
            {
                if (presentation != null)
                {
                    presentation.Dispose();
                }
                else
                {
                    StopAndDetach(musicSource);
                    DestroyOwnedObject(musicClip);
                    DestroyOwnedObject(root);
                }
                Interlocked.Increment(ref qualificationTransactionalCleanupCount);
                throw;
            }
        }

        internal static OwnerAudioPresentation LoadCampaignEnvironment(
            OwnerAssetCache cache,
            Transform parent,
            Action<ulong, ushort, ulong> voiceCompleted = null,
            int qualificationFailAfterSoundRows = -1)
        {
            return LoadCampaignEnvironment(cache, NativeSim.CampaignSelection.Create(3, 2, 0),
                parent, voiceCompleted, qualificationFailAfterSoundRows);
        }

        internal static OwnerAudioPresentation LoadCampaignEnvironment(
            OwnerAssetCache cache,
            NativeSim.CampaignSelection expected,
            Transform parent,
            Action<ulong, ushort, ulong> voiceCompleted = null,
            int qualificationFailAfterSoundRows = -1)
        {
            var isEnvironment = cache != null && cache.IsEnvironment;
            var isDayAggregate = cache != null && cache.IsDayAggregate;
            var day = expected.mapType == 0;
            var night = expected.mapType == 1;
            var cave = expected.mapType == 2;
            Require(day || night || cave, "supported source campaign environment map type");
            var mapTypeName = day ? "Day" : night ? "Night" : "Cave";
            var audioSchema = day
                ? "mc2-day-audio/v1"
                : night ? "mc2-night-audio/v1" : "mc2-cave-audio/v1";
            var soundBank = expected.mapType;
            var track = day ? 2 : night ? 1 : 3;
            // Environment caches contain the whole selected source bank. Older
            // Day aggregates and environment caches retain their qualified
            // opening-mission subset and remain readable.
            var legacySoundCount = day ? ExactCampaignDaySoundOrder.Length : ExactSoundOrder.Length;
            var ordinaryDayAggregate = isDayAggregate && day &&
                (expected.externalLevel == 2 || expected.externalLevel == 3) &&
                expected.internalLevel == expected.externalLevel - 1;
            Require(
                expected.externalLevel > 0 && expected.internalLevel >= 0 &&
                cache != null && (isEnvironment || ordinaryDayAggregate),
                "qualified campaign Day/Cave/Night owner cache and destination");
            var component = ordinaryDayAggregate ? "components/audio/" : "";
            var descriptor = DecodeCampaignAudioDescriptor(
                cache.ReadOutput(component + "audio/descriptor.json"), isEnvironment);
            Require(
                descriptor != null &&
                (descriptor.schema == audioSchema ||
                    (isEnvironment && !cave && descriptor.schema == audioSchema.Replace("/v1", "/v2"))) &&
                descriptor.profile != null &&
                descriptor.profile.external_level == expected.externalLevel &&
                descriptor.profile.internal_level == expected.internalLevel &&
                descriptor.profile.map_type != null &&
                descriptor.profile.map_type.id == expected.mapType &&
                descriptor.profile.map_type.name == mapTypeName &&
                descriptor.sound != null && descriptor.sound.bank == soundBank &&
                (!day || descriptor.sound.reachable_occurrence_count == 34) &&
                descriptor.sound.records != null &&
                descriptor.sound.records.Length == (cave ? 69 :
                    descriptor.schema.EndsWith("/v2") ? 64 : legacySoundCount),
                "qualified destination-bound campaign Night/Day/Cave audio descriptor");
            if (isEnvironment)
                Require(EnvironmentAudioProfileMatches(
                    descriptor.profile, cache.EnvironmentMaterials?.profile),
                    "campaign audio descriptor matches validated Environment profile");
            ValidateCampaignMusic(descriptor.music, track);

            GameObject root = null;
            AudioSource musicSource = null;
            AudioClip musicClip = null;
            OwnerAudioPresentation presentation = null;
            try
            {
                root = new GameObject($"Owner-local Level {expected.externalLevel} audio");
                root.transform.SetParent(parent, false);
                musicSource = AddSource(
                    root, $"C2GAME{track} owner-local presentation adaptation", true);
                var musicWavePath = component + descriptor.music.unity_wave_path;
                var musicWaveBytes = cache.ReadOutput(musicWavePath);
                RequireMusicWaveHash(musicWaveBytes, descriptor.music.unity_wave_sha256,
                    Path.Combine(cache.RootPath, musicWavePath), () => cache.ReadOutput(musicWavePath));
                var musicWave = ParseWave(musicWaveBytes);
                Require(
                    musicWave.Channels == 2 && musicWave.SampleWidth == 2 &&
                    musicWave.SampleRate == 44100 &&
                    musicWave.Frames == (track == 1 ? 19472384 : track == 2 ? 16695296 : 10584064),
                    "qualified deterministic campaign selected PCM render");
                var musicRecord = descriptor.music.records[track - 1];
                var musicRecordBytes = cache.ReadOutput(component + musicRecord.asset_path);
                Require(Sha256Hex(musicRecordBytes) == musicRecord.asset_sha256,
                    "destination music XMI content hash");
                var stream = new StreamingPcm16(musicWave.Pcm, musicWave.Channels);
                musicClip = AudioClip.Create(
                    $"C2GAME{track} — deterministic ADLMIDI bank 59",
                    musicWave.Frames,
                    musicWave.Channels,
                    musicWave.SampleRate,
                    true,
                    stream.Read,
                    stream.Seek);
                musicSource.clip = musicClip;
                musicSource.volume = 0.55f;

                presentation = new OwnerAudioPresentation(
                    root,
                    musicSource,
                    musicClip,
                    stream,
                    voiceCompleted,
                    legacyAssetReferences: false,
                    campaignMusicTrack: (byte)track);
                for (var index = 0; index < descriptor.sound.records.Length; index++)
                {
                    if (index == qualificationFailAfterSoundRows)
                        throw new InvalidDataException(
                            "qualification-forced owner-audio load failure");
                    var record = descriptor.sound.records[index];
                    var fullBank = descriptor.sound.records.Length == (cave ? 69 : 64);
                    var soundId = fullBank ? index + 1 : day
                        ? ExactCampaignDaySoundOrder[index] : ExactSoundOrder[index];
                    var knownDayIndex = day ? Array.IndexOf(ExactCampaignDaySoundOrder, soundId) : -1;
                    var nightNull = night && soundId == 12;
                    var expectedPath = $"audio/sfx/{soundId:000}.wav";
                    Require(
                        record != null && record.sound_id == soundId &&
                        !string.IsNullOrEmpty(record.filename) &&
                        (knownDayIndex < 0 || record.filename == ExactCampaignDaySoundFiles[knownDayIndex]) &&
                        (!night ||
                            (record.bank == 1 && record.quality == 822 && record.quality_index == 3 &&
                             record.semantic_name == Path.GetFileNameWithoutExtension(record.filename)
                                 .ToLowerInvariant() &&
                             record.play_class == (nightNull ? "explicit_null" :
                                 CampaignDayPlayClass(soundId)) &&
                             record.playback_scope == (nightNull ? "explicit_null" :
                                 CampaignDayPlaybackScope(soundId)) &&
                             record.loop_target_volume == (nightNull ? 0 : CampaignDayLoopVolume(soundId)) &&
                             record.mapping_status == (nightNull ? "explicit_null" :
                                 "proven_campaign_Night"))) &&
                        (nightNull
                            ? record.asset_path == null && record.asset_sha256 == null &&
                              record.wav == null
                            : record.asset_path == expectedPath &&
                              IsSha256(record.asset_sha256) &&
                              CampaignDayWaveMatches(record.wav)),
                        $"campaign {mapTypeName} sound descriptor row {index}");
                    var row = new SoundRow {
                        sound_id = soundId,
                        semantic_name = night ? record.semantic_name :
                            Path.GetFileNameWithoutExtension(record.filename).ToLowerInvariant(),
                        play_class = night ? record.play_class : CampaignDayPlayClass(soundId),
                        playback_scope = night ? record.playback_scope : CampaignDayPlaybackScope(soundId),
                        loop_target_volume = night ? record.loop_target_volume : CampaignDayLoopVolume(soundId),
                        asset_path = record.asset_path,
                        mapping_status = night ? record.mapping_status :
                            cave ? "materialized_campaign_Cave" : "materialized_campaign_Day"
                    };
                    presentation.rows.Add(soundId, row);
                    if (record.filename == "NULL.WAV")
                        presentation.nullSamples.Add(soundId);
                    if (nightNull)
                        continue;
                    var soundWaveBytes = cache.ReadOutput(component + record.asset_path);
                    Require(Sha256Hex(soundWaveBytes) == record.asset_sha256,
                        $"campaign {mapTypeName} sound ID {soundId} content hash");
                    var wave = ParseWave(soundWaveBytes);
                    Require(
                        wave.Channels == 1 && wave.SampleWidth == 1 &&
                        wave.SampleRate == 22050 && wave.Frames == record.wav.frame_count,
                        $"campaign {mapTypeName} sound ID {soundId} exact PCM format");
                    presentation.waves.Add(soundId, wave);
                    presentation.clips.Add(
                        soundId,
                        CreatePcm8Clip(row.semantic_name, wave));
                }
                return presentation;
            }
            catch
            {
                if (presentation != null)
                {
                    presentation.Dispose();
                }
                else
                {
                    StopAndDetach(musicSource);
                    DestroyOwnedObject(musicClip);
                    DestroyOwnedObject(root);
                }
                Interlocked.Increment(ref qualificationTransactionalCleanupCount);
                throw;
            }
        }

        internal void LogProbe(int tick)
        {
            var source = musicSource;
            Require(source != null, "music probe owns its constructed source");
            Debug.Log(
                $"MC_AUDIO_PROBE tick={tick} listener={AudioListener.volume:F3}/" +
                $"{AudioListener.pause} sourcePlaying={source.isPlaying} " +
                $"timeSamples={source.timeSamples} streamReads={musicStream.ReadCalls} " +
                $"streamNonZero={musicStream.NonZeroSamples}");
        }

        internal void Tick(float deltaTime)
        {
            fadeAccumulator += deltaTime * 60f;
            var callbacks = Mathf.FloorToInt(fadeAccumulator);
            if (callbacks > 0)
            {
                fadeAccumulator -= callbacks;
                foreach (var pair in loops)
                {
                    var state = pair.Value;
                    var voice = positionalVoices[state.VoiceIndex];
                    if (voice.SoundId != pair.Key || !voice.WasPlaying)
                        continue;
                    var currentTimeSamples = state.Source.timeSamples;
                    if (state.Source.clip != null)
                    {
                        var delta = currentTimeSamples - state.LastTimeSamples;
                        if (delta < 0) delta += state.Source.clip.samples;
                        state.ProgressSamples += delta;
                    }
                    state.LastTimeSamples = currentTimeSamples;
                    if (state.CurrentUnits < state.TargetUnits)
                        state.CurrentUnits = Math.Min(
                            state.TargetUnits, state.CurrentUnits + callbacks);
                    else if (state.CurrentUnits > state.TargetUnits)
                        state.CurrentUnits = Math.Max(
                            state.TargetUnits, state.CurrentUnits - callbacks);
                    state.Source.volume = state.CurrentUnits / 127f * SoundGain;
                }
            }
            if (musicTimerActive && !campaignMusicDriven)
            {
                musicFadeAccumulator += deltaTime * MusicTimerHz(musicTimerMultiplier);
                var musicCallbacks = Mathf.FloorToInt(musicFadeAccumulator);
                if (musicCallbacks > 0)
                {
                    musicFadeAccumulator -= musicCallbacks;
                    AdvanceMusicCallbacks(musicCallbacks);
                }
            }
            PollPositionalCompletions();
        }

        internal void Consume(
            NativeSim.NativeEvent nativeEvent,
            NativeSim.AudioCommand? audioCommand = null,
            bool reportCompletion = true)
        {
            if (EventPreparesSound(nativeEvent.kind))
            {
                PrepareSound(nativeEvent, audioCommand, reportCompletion);
                return;
            }
            switch (nativeEvent.kind)
            {
                case NativeSim.EventEndLoop:
                    EndLoop(nativeEvent, audioCommand);
                    break;
                case NativeSim.EventUpdateMusic:
                    if (!MusicAssetReferenceMatches(nativeEvent.assetRefIndex) ||
                        (nativeEvent.code != 1 && nativeEvent.code != 2) ||
                        nativeEvent.value0 != 3)
                    {
                        Suppress(nativeEvent, "unknown_music_update");
                        break;
                    }
                    UpdateMusicEnvelope((int)nativeEvent.code, nativeEvent.value0);
                    break;
                case NativeSim.EventStartMusic:
                    if (!MusicAssetReferenceMatches(nativeEvent.assetRefIndex) ||
                        nativeEvent.code != 1 ||
                        nativeEvent.value0 != 0 ||
                        nativeEvent.value1 != 1)
                    {
                        Suppress(nativeEvent, "unknown_music_start");
                        break;
                    }
                    StartMusicFromSourceEvent();
                    break;
            }
        }

        internal void ConsumeObjectiveSound(
            NativeSim.NativeEvent nativeEvent,
            NativeSim.AudioCommand audioCommand)
        {
            Require(nativeEvent.kind == NativeSim.EventPrepareSound &&
                (nativeEvent.code == 41 || nativeEvent.code == 61),
                "source-authored objective sound event");
            PrepareSound(nativeEvent, audioCommand, true);
        }

        internal void LoadObjectiveSound61(OwnerAssetCache cache, string validatedPath)
        {
            Require(!legacyAssetReferences && cache != null && !rows.ContainsKey(61),
                "campaign-only missing objective sound binding");
            var bytes = cache.ReadOutput(validatedPath);
            using (var hash = SHA256.Create())
                Require(BitConverter.ToString(hash.ComputeHash(bytes)).Replace("-", "")
                    .ToLowerInvariant() ==
                    "04d167b6abc8e7624827213ed14cf3d3e60cfc18e8251ace32b7705774e60d9e",
                    "source Night/822 slot60 SUCCESS2.WAV payload");
            var wave = ParseWave(bytes);
            Require(wave.Channels == 1 && wave.SampleWidth == 1 &&
                wave.SampleRate == 22050 && wave.Frames == 33857,
                "source objective success sound PCM format");
            var clip = CreatePcm8Clip("success2", wave);
            rows.Add(61, new SoundRow {
                sound_id = 61,
                semantic_name = "success2",
                play_class = "one_shot",
                playback_scope = "original_stereo_positional",
                asset_path = validatedPath,
                mapping_status = "materialized_campaign_objective"
            });
            waves.Add(61, wave);
            clips.Add(61, clip);
        }

        internal void RebindCampaignGeneration(ulong generation)
        {
            Require(generation != 0, "campaign audio generation is live");
            foreach (var voice in positionalVoices)
                if (voice.Generation != 0) voice.Generation = generation;
            if (campaignMusicGeneration != 0) campaignMusicGeneration = generation;
        }

        /// Consume PauseUnpauseGame_18BB0's EndSample_8D8F0 edge. The Rust
        /// pool has already retired these voices, so device cleanup must not
        /// send ordinary finite-sample completion acknowledgements. Music is
        /// controlled by ApplyCampaignMusic; CD speech has its own source.
        internal void EndCampaignSamples()
        {
            loops.Clear();
            QualificationStopPositionals();
            fadeAccumulator = 0f;
        }

        internal void ApplyCampaignMusic(NativeSim.CampaignDayMusicSnapshot state)
        {
            Require(state.abiVersion == NativeSim.AbiVersion && state.structSize == 80 &&
                state.volume <= 127 && state.currentUnits <= 127 &&
                (state.playing == 0 || (state.initialized != 0 && state.loaded != 0 &&
                    state.active != 0 && state.track == campaignMusicTrack)),
                "authoritative campaign music state");
            campaignMusicDriven = true;
            musicUpdatesEnabled = state.updatesEnabled != 0;
            musicTimerActive = state.timerActive != 0;
            musicTargetMode = state.targetMode;
            musicCurrentUnits = state.currentUnits;
            musicDirection = state.direction;
            musicTimerMultiplier = state.timerMultiplier;
            campaignMusicGain = state.volume / 127f;
            var restarted = campaignMusicGeneration != state.generation ||
                campaignMusicStartCount != state.startCount;
            campaignMusicGeneration = state.generation;
            campaignMusicStartCount = state.startCount;
            if (state.playing == 0)
                musicSource.Stop();
            else if (restarted)
            {
                musicSource.Stop();
                musicSource.timeSamples = 0;
                musicStream.Seek(0);
                musicSource.Play();
                musicStartCount++;
                Debug.Log($"MC_MUSIC_START generation={state.generation} tick={state.tick} " +
                    $"logicalTrack={state.track} startCount={state.startCount}");
            }
            ApplyMix();
        }

        internal static bool EventPreparesSound(uint kind)
        {
            return kind == NativeSim.EventPrepareSound ||
                kind == NativeSim.EventGoatSoundRequest ||
                kind == NativeSim.EventLevel1ArcherArrowSound ||
                kind == NativeSim.EventLevel1FireflyChargeSound ||
                kind == NativeSim.EventLevel1FireballImpacted ||
                kind == NativeSim.EventLevel1CastleProjectileSpawned ||
                kind == NativeSim.EventLevel1CastleConstructionSound ||
                kind == NativeSim.EventLevel1ManaSphereClaimed ||
                kind == NativeSim.EventLevel1WaterSplashed;
        }

        internal static int EventSoundId(uint kind, uint code)
        {
            // FireballImpacted keeps the authored impact entity ID in `code`;
            // its attached remc2 PrepareSound sidecar always names sound 3.
            // All other PrepareSound-family events carry the sound ID directly.
            return kind == NativeSim.EventLevel1FireballImpacted
                ? 3
                : checked((int)code);
        }

        internal void Reset(bool restartMusic = false)
        {
            foreach (var state in loops.Values) state.Source.Stop();
            loops.Clear();
            QualificationStopPositionals();
            musicSource.Stop();
            musicStream.Seek(0);
            fadeAccumulator = 0f;
            ResetMusicEnvelope();
            campaignMusicDriven = false;
            campaignMusicGeneration = 0;
            campaignMusicStartCount = 0;
            campaignMusicGain = 1f;
            SetSpeechMix(1f, 1f);
            musicStartCount = 0;
            oneShotPlayCount = 0;
            HadSuppressedUnknown = false;
            HadSuppressedUnsupportedPositional = false;
            if (restartMusic)
                throw new InvalidOperationException(
                    "music restart must arrive through the native source-turn event");
        }

        internal int QualificationLoopUnits(int soundId)
        {
            return loops.TryGetValue(soundId, out var state)
                ? state.CurrentUnits
                : -1;
        }

        internal int QualificationLoopTargetUnits(int soundId)
        {
            return loops.TryGetValue(soundId, out var state)
                ? state.TargetUnits
                : -1;
        }

        internal int QualificationLoopTimeSamples(int soundId)
        {
            return loops.TryGetValue(soundId, out var state)
                ? state.Source.timeSamples
                : -1;
        }

        internal long QualificationLoopProgressSamples(int soundId)
        {
            return loops.TryGetValue(soundId, out var state)
                ? state.ProgressSamples
                : -1;
        }

        internal int QualificationLoopVoiceIndex(int soundId)
        {
            return loops.TryGetValue(soundId, out var state)
                ? state.VoiceIndex
                : -1;
        }

        internal int QualificationLoopPlayStarts(int soundId)
        {
            return loops.TryGetValue(soundId, out var state)
                ? state.PlayStarts
                : 0;
        }

        internal bool QualificationLoopIsPlaying(int soundId)
        {
            return loops.TryGetValue(soundId, out var state) &&
                   positionalVoices[state.VoiceIndex].SoundId == soundId &&
                   state.Source.isPlaying;
        }

        internal int QualificationActiveLoopCount
        {
            get
            {
                var count = 0;
                foreach (var pair in loops)
                {
                    if (positionalVoices[pair.Value.VoiceIndex].SoundId == pair.Key &&
                        pair.Value.Source.isPlaying)
                        count++;
                }
                return count;
            }
        }

        internal int QualificationOneShotPlayCount => oneShotPlayCount;
        internal int QualificationSoundCount => rows.Count;
        internal bool QualificationHasSound(int soundId) => rows.ContainsKey(soundId);
        internal bool QualificationOneShotIsPlaying =>
            QualificationPositionalIsPlaying(29);
        internal bool QualificationMusicIsPlaying => musicSource.isPlaying;
        internal int QualificationMusicStartCount => musicStartCount;
        internal bool QualificationMusicUpdatesEnabled => musicUpdatesEnabled;
        internal bool QualificationMusicTimerActive => musicTimerActive;
        internal int QualificationMusicTargetMode => musicTargetMode;
        internal int QualificationMusicCurrentUnits => musicCurrentUnits;
        internal int QualificationMusicDirection => musicDirection;
        internal int QualificationMusicTimerMultiplier => musicTimerMultiplier;
        internal static int QualificationTransactionalCleanupCount =>
            Interlocked.CompareExchange(
                ref qualificationTransactionalCleanupCount, 0, 0);

        internal void QualificationAdvanceMusicCallbacks(int callbacks)
        {
            AdvanceMusicCallbacks(callbacks);
        }

        internal int QualificationPositionalPlayStarts(int soundId)
        {
            var total = 0;
            foreach (var state in positionalVoices)
                if (state.SoundId == soundId) total += state.PlayStarts;
            return total;
        }

        internal int QualificationPositionalSuppressedExisting(int soundId)
        {
            var total = 0;
            foreach (var state in positionalVoices)
                if (state.SoundId == soundId) total += state.SuppressedExisting;
            return total;
        }

        internal bool QualificationPositionalIsPlaying(int soundId)
        {
            foreach (var state in positionalVoices)
                if (state.SoundId == soundId && state.Source.isPlaying) return true;
            return false;
        }

        internal float QualificationPositionalVolume(int soundId)
        {
            foreach (var state in positionalVoices)
                if (state.SoundId == soundId) return state.EffectiveVolume;
            return -1f;
        }

        internal float QualificationPositionalPan(int soundId)
        {
            foreach (var state in positionalVoices)
                if (state.SoundId == soundId) return state.EffectivePan;
            return float.NaN;
        }

        internal float QualificationPositionalPitch(int soundId)
        {
            foreach (var state in positionalVoices)
                if (state.SoundId == soundId) return state.EffectivePitch;
            return float.NaN;
        }

        internal void QualificationStopPositionals()
        {
            foreach (var state in positionalVoices)
            {
                state.Source.Stop();
                state.Source.clip = null;
                if (state.RenderedClip != null)
                    DestroyOwnedObject(state.RenderedClip);
                state.RenderedClip = null;
                state.SoundId = -1;
                state.Generation = 0;
                state.EventSequence = 0;
                state.WasPlaying = false;
                state.CompletionPending = false;
            }
        }

        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            QualificationStopPositionals();
            musicSource.Stop();
            musicSource.clip = null;
            foreach (var clip in clips.Values)
                DestroyOwnedObject(clip);
            clips.Clear();
            DestroyOwnedObject(musicClip);
            DestroyOwnedObject(root);
        }

        private void StartMusicFromSourceEvent()
        {
            ResetMusicEnvelope();
            musicUpdatesEnabled = true;
            if (!musicSource.isPlaying) musicSource.Play();
            musicStartCount++;
            Debug.Log(
                "MC_MUSIC_START logicalTrack=1 playbackSlot=0 " +
                "loopScanPointerSlot=1 sourceGameTurn=1");
        }

        private void UpdateMusicEnvelope(int mode, int timerMultiplier)
        {
            if (!musicUpdatesEnabled || musicTargetMode == mode) return;
            musicTargetMode = mode;
            musicDirection = -musicDirection;
            musicTimerMultiplier = timerMultiplier;
            musicTimerActive = true;
            musicFadeAccumulator = 0f;
        }

        private void AdvanceMusicCallbacks(int callbacks)
        {
            for (var index = 0; index < callbacks && musicTimerActive; index++)
            {
                var atTarget =
                    (musicCurrentUnits == 127 && musicTargetMode == MusicCombatMode) ||
                    (musicCurrentUnits == 0 && musicTargetMode == MusicIdleMode);
                if (atTarget)
                {
                    musicTimerActive = false;
                    musicFadeAccumulator = 0f;
                    break;
                }
                musicCurrentUnits += musicDirection;
                Require(
                    musicCurrentUnits >= 0 && musicCurrentUnits <= 127,
                    "music envelope stays within the original 0..127 range");
            }
        }

        private void ResetMusicEnvelope()
        {
            musicFadeAccumulator = 0f;
            musicUpdatesEnabled = false;
            musicTimerActive = false;
            musicTargetMode = MusicIdleMode;
            musicCurrentUnits = 0;
            musicDirection = -1;
            musicTimerMultiplier = 3;
        }

        private static int MusicTimerHz(int multiplier)
        {
            return multiplier >= 1 && multiplier <= 4
                ? 30 * multiplier
                : 30;
        }

        private void PrepareSound(
            NativeSim.NativeEvent nativeEvent,
            NativeSim.AudioCommand? audioCommand,
            bool reportCompletion)
        {
            var soundId = EventSoundId(nativeEvent.kind, nativeEvent.code);
            if (!rows.TryGetValue(soundId, out var row))
            {
                Suppress(nativeEvent, "unknown_prepare_sound");
                return;
            }
            Require(SoundAssetReferenceMatches(soundId, nativeEvent.assetRefIndex),
                $"sound ID {soundId} descriptor-backed asset reference");
            if (!audioCommand.HasValue)
            {
                if (nullSamples.Contains(soundId)) return;
                Suppress(
                    nativeEvent,
                    "missing_audio_command",
                    row.playback_scope == "original_stereo_positional");
                return;
            }
            var command = audioCommand.Value;
            ValidateAudioCommand(nativeEvent, command, row.play_class, soundId);
            if (nullSamples.Contains(soundId))
            {
                Require(command.action == NativeSim.AudioActionSuppressNullSample &&
                    command.voiceIndex == NativeSim.AudioVoiceNone,
                    $"sound ID {soundId} selected NULL sample owns no native voice");
                return;
            }
            Require(command.action != NativeSim.AudioActionSuppressNullSample,
                $"sound ID {soundId} playable selected sample cannot be suppressed as NULL");
            if (row.play_class == "loop")
            {
                PrepareLoop(nativeEvent, command, row);
                return;
            }
            PreparePositional(nativeEvent, command, reportCompletion);
            if (soundId == 29 &&
                (command.action == NativeSim.AudioActionStart ||
                 command.action == NativeSim.AudioActionRestart))
                oneShotPlayCount++;
        }

        private void PreparePositional(
            NativeSim.NativeEvent nativeEvent,
            NativeSim.AudioCommand command,
            bool reportCompletion)
        {
            var minimumRateDelta = command.soundId == 46 ? -10 :
                command.soundId >= 42 && command.soundId <= 44 ? -15 : 0;
            var maximumRateDelta = command.soundId == 46 ? 29 :
                command.soundId >= 42 && command.soundId <= 44 ? 14 : 0;
            Require(
                command.volumeQ15 <= 0x7fff &&
                command.rateDeltaPercent >= minimumRateDelta &&
                command.rateDeltaPercent <= maximumRateDelta,
                $"sound ID {command.soundId} bounded positional values");

            if (command.action == NativeSim.AudioActionSuppressExisting)
            {
                Require(command.voiceIndex < positionalVoices.Length,
                    $"sound ID {command.soundId} existing voice index");
                positionalVoices[command.voiceIndex].SuppressedExisting++;
                Debug.Log(
                    $"MC_AUDIO_POSITIONAL_SUPPRESS generation={nativeEvent.generation} " +
                    $"sequence={nativeEvent.sequence} id={command.soundId} " +
                    $"action={command.action} volumeQ15={command.volumeQ15} " +
                    $"panQ16={command.panQ16}");
                return;
            }
            if (command.action == NativeSim.AudioActionSuppressNoVoice ||
                command.action == NativeSim.AudioActionSuppressDistanceOrVolume ||
                command.action == NativeSim.AudioActionSuppressPendingReplaced)
            {
                Require(command.voiceIndex == NativeSim.AudioVoiceNone,
                    $"sound ID {command.soundId} suppressed command has no voice");
                Debug.Log(
                    $"MC_AUDIO_POSITIONAL_SUPPRESS generation={nativeEvent.generation} " +
                    $"sequence={nativeEvent.sequence} id={command.soundId} " +
                    $"action={command.action} volumeQ15={command.volumeQ15} " +
                    $"panQ16={command.panQ16}");
                return;
            }
            Require(
                command.action == NativeSim.AudioActionStart ||
                command.action == NativeSim.AudioActionRestart,
                $"sound ID {command.soundId} known positional action");
            Require(command.voiceIndex < positionalVoices.Length,
                $"sound ID {command.soundId} assigned voice index");
            var expectedRate = 22050 *
                (100 + command.rateDeltaPercent) / 100;
            Require(
                command.playbackRateHz == expectedRate &&
                command.playbackRateHz > 0,
                $"sound ID {command.soundId} exact playback rate");

            var state = positionalVoices[command.voiceIndex];
            if (command.action == NativeSim.AudioActionStart)
                Require(!state.Source.isPlaying && !state.WasPlaying,
                    $"sound ID {command.soundId} start owns an idle native voice");
            if (command.action == NativeSim.AudioActionRestart)
            {
                state.Source.Stop();
                state.Source.clip = null;
                if (state.RenderedClip != null)
                    DestroyOwnedObject(state.RenderedClip);
            }

            var volumeUnits = command.volumeQ15 >> 8;
            var panUnits = 127 * (uint)command.panQ16 / 0xffff;
            var volume = volumeUnits / 127f;
            var leftGain = volume * (127f - panUnits) / 127f;
            var rightGain = volume * panUnits / 127f;
            state.RenderedClip = CreateManualStereoClip(
                rows[command.soundId].semantic_name,
                waves[command.soundId],
                leftGain,
                rightGain);
            state.Source.clip = state.RenderedClip;
            state.Source.loop = false;
            state.Source.volume = SoundGain;
            state.Source.panStereo = 0f;
            state.Source.pitch = command.playbackRateHz / 22050f;
            state.SoundId = command.soundId;
            state.EffectiveVolume = volume;
            state.EffectivePan = PanUnitsToStereo(panUnits);
            state.EffectivePitch = state.Source.pitch;
            state.Generation = command.generation;
            state.EventSequence = command.eventSequence;
            state.StartFrame = Time.frameCount;
            state.WasPlaying = true;
            state.CompletionPending = reportCompletion && voiceCompleted != null;
            state.Source.Play();
            state.PlayStarts++;
            Debug.Log(
                $"MC_AUDIO_POSITIONAL_PLAY generation={nativeEvent.generation} " +
                $"sequence={nativeEvent.sequence} id={command.soundId} " +
                $"relative={command.relativeX},{command.relativeY},{command.relativeZ} " +
                $"volumeQ15={command.volumeQ15} volumeUnits={volumeUnits} " +
                $"panQ16={command.panQ16} panUnits={panUnits} " +
                $"leftGain={leftGain:F6} rightGain={rightGain:F6} " +
                $"rateDelta={command.rateDeltaPercent} rateHz={command.playbackRateHz} " +
                $"voice={command.voiceIndex} spatialBlend={state.Source.spatialBlend:F1} " +
                $"panStereo={state.Source.panStereo:F1} " +
                $"doppler={state.Source.dopplerLevel:F1}");
        }

        private void PrepareLoop(
            NativeSim.NativeEvent nativeEvent,
            NativeSim.AudioCommand command,
            SoundRow row)
        {
            if (command.action == NativeSim.AudioActionSuppressNoVoice ||
                command.action == NativeSim.AudioActionSuppressPendingReplaced)
                return;
            if (command.action == NativeSim.AudioActionUpdateExisting)
            {
                var found = loops.TryGetValue(command.soundId, out var existing);
                Require(
                    command.voiceIndex < positionalVoices.Length &&
                    found &&
                    existing.VoiceIndex == command.voiceIndex,
                    $"loop {command.soundId} update owns its native voice");
                existing.TargetUnits = Mathf.Clamp(row.loop_target_volume, 0, 127);
                return;
            }
            Require(
                command.action == NativeSim.AudioActionStart &&
                command.voiceIndex < positionalVoices.Length,
                $"loop {command.soundId} start action and voice");
            var voice = positionalVoices[command.voiceIndex];
            Require(!voice.Source.isPlaying && !voice.WasPlaying,
                $"loop {command.soundId} sequence={command.eventSequence} starts on " +
                $"idle shared voice {command.voiceIndex}; logicalSound={voice.SoundId} " +
                $"wasPlaying={voice.WasPlaying} sourcePlaying={voice.Source.isPlaying} " +
                $"clip={(voice.Source.clip == null ? "none" : voice.Source.clip.name)}");
            var panUnits = 64u;
            voice.RenderedClip = CreateManualStereoClip(
                row.semantic_name,
                waves[command.soundId],
                (127f - panUnits) / 127f,
                panUnits / 127f);
            voice.Source.clip = voice.RenderedClip;
            voice.Source.loop = true;
            voice.Source.volume = 0f;
            voice.Source.panStereo = 0f;
            voice.Source.pitch = 1f;
            voice.SoundId = command.soundId;
            voice.Generation = command.generation;
            voice.EventSequence = command.eventSequence;
            voice.StartFrame = Time.frameCount;
            voice.WasPlaying = true;
            // Original loop voices remain resident even after their envelope
            // reaches zero. They are never finite voice-completion candidates.
            voice.CompletionPending = false;
            voice.Source.Play();
            voice.PlayStarts++;
            loops[command.soundId] = new LoopState {
                Source = voice.Source,
                VoiceIndex = command.voiceIndex,
                CurrentUnits = 0,
                TargetUnits = Mathf.Clamp(row.loop_target_volume, 0, 127),
                PlayStarts = voice.PlayStarts,
                LastTimeSamples = voice.Source.timeSamples,
                ProgressSamples = 0
            };
        }

        private static void ValidateAudioCommand(
            NativeSim.NativeEvent nativeEvent,
            NativeSim.AudioCommand command,
            string playClass,
            int expectedSoundId)
        {
            var expectedLoopCount = playClass == "loop" ? (sbyte)-1 : (sbyte)0;
            Require(
                command.abiVersion == NativeSim.AbiVersion &&
                command.structSize == 88 &&
                command.generation == nativeEvent.generation &&
                command.eventSequence == nativeEvent.sequence &&
                command.stateHash == nativeEvent.stateHash &&
                command.assetRefIndex == nativeEvent.assetRefIndex &&
                command.soundId == expectedSoundId &&
                command.entityIndex == nativeEvent.entityIndex &&
                command.playerIndex == nativeEvent.playerIndex &&
                (command.playType == NativeSim.AudioEntitySoundRestartOrStart ||
                 command.playType == NativeSim.AudioPlayIfNotPlaying ||
                 command.playType == NativeSim.AudioEntitySoundIfNotPlaying) &&
                command.loopCount == expectedLoopCount &&
                command.pooledFlushRank <= 70 &&
                command.reserved != null &&
                command.reserved.Length == 4 &&
                (command.pooledFlushRank == 0
                    ? (command.reserved[0] == 0 && command.reserved[1] == 0)
                    : (command.reserved[0] != 0 || command.reserved[1] != 0)) &&
                command.reserved[2] == 0 &&
                command.reserved[3] == 0,
                $"sound ID {expectedSoundId} complete shared-voice sidecar identity");
        }

        private void PollPositionalCompletions()
        {
            for (ushort index = 0; index < positionalVoices.Length; index++)
            {
                var state = positionalVoices[index];
                if (state.Source.loop)
                    continue;
                if (!state.WasPlaying || Time.frameCount <= state.StartFrame ||
                    state.Source.isPlaying)
                    continue;
                var generation = state.Generation;
                var sequence = state.EventSequence;
                var notify = state.CompletionPending;
                state.Source.clip = null;
                if (state.RenderedClip != null)
                    DestroyOwnedObject(state.RenderedClip);
                state.RenderedClip = null;
                state.SoundId = -1;
                state.Generation = 0;
                state.EventSequence = 0;
                state.WasPlaying = false;
                state.CompletionPending = false;
                if (notify) voiceCompleted(generation, index, sequence);
            }
        }

        private static float PanUnitsToStereo(uint panUnits)
        {
            Require(panUnits <= 127, "original pan units fit 0..127");
            return panUnits <= 64
                ? ((int)panUnits - 64) / 64f
                : ((int)panUnits - 64) / 63f;
        }

        private void EndLoop(
            NativeSim.NativeEvent nativeEvent,
            NativeSim.AudioCommand? audioCommand)
        {
            var soundId = (int)nativeEvent.code;
            if (!rows.TryGetValue(soundId, out var row))
            {
                Suppress(nativeEvent, "unknown_end_loop");
                return;
            }
            Require(SoundAssetReferenceMatches(soundId, nativeEvent.assetRefIndex),
                $"EndLoop sound ID {soundId} descriptor-backed asset reference");
            if (!audioCommand.HasValue)
            {
                Suppress(nativeEvent, "missing_audio_command");
                return;
            }
            var command = audioCommand.Value;
            ValidateAudioCommand(nativeEvent, command, row.play_class, soundId);
            if (command.action == NativeSim.AudioActionSuppressNoExisting)
                return;
            var found = loops.TryGetValue(soundId, out var state);
            Require(
                command.action == NativeSim.AudioActionEndLoop &&
                command.voiceIndex < positionalVoices.Length &&
                found &&
                state.VoiceIndex == command.voiceIndex,
                $"EndLoop sound ID {soundId} owns its shared voice");
            state.TargetUnits = 0;
        }

        private void Suppress(
            NativeSim.NativeEvent nativeEvent,
            string reason,
            bool unsupportedPositional = false)
        {
            if (unsupportedPositional)
                HadSuppressedUnsupportedPositional = true;
            else
                HadSuppressedUnknown = true;
            Debug.LogWarning(
                (unsupportedPositional
                    ? "MC_AUDIO_UNSUPPORTED "
                    : "MC_AUDIO_UNKNOWN ") +
                $"generation={nativeEvent.generation} sequence={nativeEvent.sequence} " +
                $"kind={nativeEvent.kind} bank=1 quality=822 id={nativeEvent.code} " +
                $"entity={nativeEvent.entityIndex} player={nativeEvent.playerIndex} " +
                $"action=suppressed reason={reason}");
        }

        private static string ExpectedPlaybackScope(int soundId)
        {
            return soundId switch {
                1 => "local_ambient",
                2 => "local_ambient",
                3 => "original_stereo_positional",
                4 => "original_stereo_positional",
                5 => "local_ambient",
                8 => "original_stereo_positional",
                9 => "original_stereo_positional",
                10 => "original_stereo_positional",
                12 => "explicit_null",
                14 => "local_feedback",
                15 => "original_stereo_positional",
                16 => "original_stereo_positional",
                18 => "original_stereo_positional",
                19 => "original_stereo_positional",
                27 => "original_stereo_positional",
                29 => "local_feedback",
                30 => "original_stereo_positional",
                31 => "local_ambient",
                33 => "original_stereo_positional",
                34 => "original_stereo_positional",
                37 => "original_stereo_positional",
                39 => "original_stereo_positional",
                40 => "original_stereo_positional",
                41 => "original_stereo_positional",
                43 => "original_stereo_positional",
                44 => "original_stereo_positional",
                46 => "original_stereo_positional",
                54 => "original_stereo_positional",
                55 => "original_stereo_positional",
                56 => "original_stereo_positional",
                57 => "original_stereo_positional",
                63 => "original_stereo_positional",
                _ => null
            };
        }

        private bool SoundAssetReferenceMatches(int soundId, uint assetRefIndex)
        {
            return legacyAssetReferences
                ? assetRefIndex == ExpectedSoundAssetRef(soundId)
                : assetRefIndex != uint.MaxValue;
        }

        private bool MusicAssetReferenceMatches(uint assetRefIndex)
        {
            return legacyAssetReferences
                ? assetRefIndex == MusicAssetRef
                : assetRefIndex != uint.MaxValue;
        }

        private static void ValidateCampaignMusic(MusicDescriptor music, int track)
        {
            Require(track >= 1 && track <= 3, "Night, Day, or Cave music selection");
            var slot = track - 1;
            var label = $"C2GAME{track}";
            var mapType = track == 1 ? "Night" : track == 2 ? "Day" : "Cave";
            var xmiSha256 = track == 1
                ? "0f8081d8ef6ad609f406638862a573b9d1d014f940f0f3194e4d5687f724bd8d"
                : track == 2
                    ? "8c98b86e74a6187a3a08ac34ef81aa3bd97cf493c34bee88b6d6969e79a6c856"
                    : "566769008b9598bd03ad657575daa089f217385d7e7671dd948e6b2fea1e3251";
            var waveSha256 = track == 1
                ? "03b3f951048504d8a6d30611d6c44a62ec1c463726b30c33c16e3cb69824605d"
                : track == 2
                    ? "9030814e6160f3cd0b9cf5f3c5977715c10c012e9faaacfca3f745c69fa9f5e3"
                    : "4861f36b367de541f3438522cc66520fa8d9a5455aef35639b1bb218388e27a7";
            var frames = track == 1 ? 19472384 : track == 2 ? 16695296 : 10584064;
            Require(
                music != null && music.bank == 1 && music.channel == 0 &&
                music.driver == "f" && music.map_type == mapType &&
                music.maptype_music == track && music.logical_track == track &&
                music.playback_slot == slot && music.loop_scan_pointer_slot == track &&
                music.records != null && music.records.Length == 6 &&
                music.records[slot] != null && music.records[slot].slot == slot &&
                music.records[slot].filename == label + ".XMI" &&
                music.records[slot].asset_path == $"audio/music/{label}.xmi" &&
                music.records[slot].asset_sha256 == xmiSha256 &&
                music.unity_wave_path == $"audio/music/{label}-adlmidi-bank59.wav" &&
                music.unity_wave_sha256 == waveSha256 &&
                music.unity_wave != null && music.unity_wave.channels == 2 &&
                music.unity_wave.sample_width_bytes == 2 &&
                music.unity_wave.sample_rate == 44100 &&
                music.unity_wave.frame_count == frames &&
                music.initial_start_status ==
                    "SourceOrderQualified_remc2_local_gameTurn1_after_second_DrawAndEvents" &&
                music.optional_random_OGG_path == "Excluded" &&
                music.exact_DOS_hardware_timbre == "Deferred",
                "qualified campaign Night/Day/Cave selected music descriptor");
            for (var index = 0; index < music.records.Length; index++)
            {
                if (index == slot) continue;
                Require(music.records[index] != null && music.records[index].slot == index &&
                    music.records[index].asset_path == null && music.records[index].asset_sha256 == null,
                    "campaign music contains only its selected track payload");
            }
            ValidateMusicDynamicControl(music.dynamic_control, track);
        }

        private static bool CampaignDayWaveMatches(WaveDescriptor wave)
        {
            return wave != null && wave.channels == 1 &&
                wave.sample_width_bytes == 1 && wave.sample_rate == 22050 &&
                wave.frame_count > 0;
        }

        private static bool EnvironmentAudioProfileMatches(
            CampaignDayProfile audio,
            OwnerAssetCache.EnvironmentProfile environment)
        {
            return audio != null && environment != null &&
                audio.external_level == environment.external_level &&
                audio.internal_level == environment.internal_level &&
                audio.vga_resolution_selector == environment.vga_resolution_selector &&
                audio.texture_size == environment.texture_size &&
                audio.terrain_graphics_flags == environment.terrain_graphics_flags &&
                audio.key_color_1 == environment.key_color_1 &&
                audio.key_color_2 == environment.key_color_2 &&
                audio.map_type != null && environment.map_type != null &&
                audio.map_type.id == environment.map_type.id &&
                audio.map_type.name == environment.map_type.name &&
                audio.palette == environment.palette && audio.tables == environment.tables &&
                audio.colors == environment.colors && audio.blocks == environment.blocks &&
                audio.dormant_blocks16 == environment.dormant_blocks16 &&
                audio.sky == environment.sky && audio.sprites == environment.sprites &&
                audio.web == environment.web && audio.tmaps == environment.tmaps &&
                audio.campaign_package_sha256 == environment.campaign_package_sha256 &&
                audio.owner_export_package_sha256 == environment.owner_export_package_sha256 &&
                audio.spec_commit == environment.spec_commit;
        }

        private static CampaignDayAudioDescriptor DecodeCampaignAudioDescriptor(
            byte[] bytes, bool requireEnvironmentSky)
        {
            Require(bytes != null && bytes.Length > 0,
                "campaign audio descriptor bytes");
            var descriptor = JsonUtility.FromJson<CampaignDayAudioDescriptor>(
                System.Text.Encoding.UTF8.GetString(bytes));
            using var stream = new MemoryStream(bytes, false);
            var nullableFields = new DataContractJsonSerializer(
                typeof(CampaignAudioNullableFieldProjection)).ReadObject(stream)
                    as CampaignAudioNullableFieldProjection;
            Require(
                descriptor != null && descriptor.profile != null && descriptor.music != null &&
                descriptor.music.records != null && descriptor.sound != null &&
                descriptor.sound.records != null && nullableFields != null &&
                nullableFields.profile != null &&
                (!requireEnvironmentSky || nullableFields.profile.hasSky) &&
                nullableFields.music != null &&
                nullableFields.sound != null &&
                nullableFields.music.records != null &&
                nullableFields.sound.records != null &&
                descriptor.music.records.Length == nullableFields.music.records.Length &&
                descriptor.sound.records.Length == nullableFields.sound.records.Length,
                "complete campaign audio nullable-field projection");
            descriptor.profile.sky = nullableFields.profile.sky;
            for (var index = 0; index < descriptor.music.records.Length; index++)
            {
                var record = descriptor.music.records[index];
                var projected = nullableFields.music.records[index];
                Require(record != null && projected != null && record.slot == projected.slot,
                    "campaign audio nullable-field projection record slot");
                record.asset_path = projected.asset_path;
                record.asset_sha256 = projected.asset_sha256;
            }
            for (var index = 0; index < descriptor.sound.records.Length; index++)
            {
                var record = descriptor.sound.records[index];
                var projected = nullableFields.sound.records[index];
                Require(record != null && projected != null &&
                    record.sound_id == projected.sound_id,
                    "campaign audio nullable-field projection sound ID");
                record.asset_path = projected.asset_path;
                record.asset_sha256 = projected.asset_sha256;
                if (projected.wav == null)
                    record.wav = null;
                else
                    Require(record.wav != null,
                        "campaign audio wave DTO matches non-null source wave object");
            }
            return descriptor;
        }

        private static bool IsSha256(string value)
        {
            if (value == null || value.Length != 64) return false;
            foreach (var character in value)
            {
                var hex = (character >= '0' && character <= '9') ||
                    (character >= 'a' && character <= 'f');
                if (!hex) return false;
            }
            return true;
        }

        private static string Sha256Hex(byte[] payload)
        {
            return Sha256Hex(payload, out _);
        }

        private static string Sha256Hex(byte[] payload, out string implementation)
        {
            using (var hash = SHA256.Create())
            {
                implementation = hash.GetType().FullName;
                return BitConverter.ToString(hash.ComputeHash(payload)).Replace("-", "")
                    .ToLowerInvariant();
            }
        }

        internal static void RequireMusicWaveHash(
            byte[] payload, string expected, string path, Func<byte[]> readAgain)
        {
            var actual = Sha256Hex(payload, out var implementation);
            if (actual == expected) return;

            // Preserve the original failed buffer and fail even if either
            // subsequent observation matches. These reads are diagnostic only.
            string sameBuffer;
            try { sameBuffer = Sha256Hex(payload); }
            catch (Exception error) { sameBuffer = "error:" + error.GetType().FullName; }
            string fresh;
            try
            {
                var bytes = readAgain();
                fresh = $"fresh_length={bytes.Length} fresh_sha256={Sha256Hex(bytes)}";
            }
            catch (Exception error)
            {
                fresh = "fresh_read_error=" + error.GetType().FullName;
            }
            throw new InvalidDataException(
                $"owner audio contract failed: destination music WAVE content hash " +
                $"path={path} expected_sha256={expected} actual_sha256={actual} " +
                $"buffer_length={payload.Length} same_buffer_sha256={sameBuffer} " +
                $"{fresh} hash_implementation={implementation}");
        }

        private static string CampaignDayPlayClass(int soundId)
        {
            return soundId == 1 || soundId == 2 || soundId == 5 || soundId == 31
                ? "loop"
                : "one_shot";
        }

        private static string CampaignDayPlaybackScope(int soundId)
        {
            if (soundId == 1 || soundId == 2 || soundId == 5 || soundId == 31)
                return "local_ambient";
            return soundId == 14 || soundId == 29
                ? "local_feedback"
                : "original_stereo_positional";
        }

        private static int CampaignDayLoopVolume(int soundId)
        {
            return soundId switch {
                1 => 70,
                2 => 70,
                5 => 120,
                31 => 85,
                _ => 127
            };
        }

        private static void ValidateMusicDynamicControl(MusicDynamicControl control, int track = 1)
        {
            Require(
                control != null &&
                control.audible_channel_resolution == (track == 1
                    ? "Unresolved_playback_slot0_has_no_CC119_but_loop_scan_slot1_tags_channels7_8"
                    : "Source_playback_track_minus_one_loop_scan_track") &&
                control.envelope_min == 0 &&
                control.envelope_max == 127 &&
                control.envelope_step_per_callback == 1 &&
                control.timer_hz == 90 &&
                control.timer_hz_formula == "30 * timer_multiplier" &&
                control.timer_multiplier == 3 &&
                control.update_mode_field == "target_mode",
                "qualified source dynamic-music control constants");

            if (track != 1)
            {
                ValidateSelectedControllerCensus(control.playback_slot_census, track);
                ValidateSelectedControllerCensus(control.loop_scan_pointer_census, track + 1);
                return;
            }

            var playback = control.playback_slot_census;
            Require(
                CensusHeaderMatches(playback, 201) &&
                playback.tagged_channels.Length == 0 &&
                playback.trigger_events.Length == 0 &&
                playback.loop_controller_events.Length == 2 &&
                ControllerEventMatches(
                    playback.loop_controller_events[0], 1, 116, 0, 0) &&
                ControllerEventMatches(
                    playback.loop_controller_events[1], 1, 117, 158265, 127),
                "qualified C2GAME1 playback-slot controller census");

            var loopScan = control.loop_scan_pointer_census;
            Require(
                CensusHeaderMatches(loopScan, 167) &&
                loopScan.tagged_channels.Length == 2 &&
                loopScan.tagged_channels[0] == 7 &&
                loopScan.tagged_channels[1] == 8 &&
                loopScan.trigger_events.Length == 2 &&
                ControllerEventMatches(loopScan.trigger_events[0], 7, 119, 0, 0) &&
                ControllerEventMatches(loopScan.trigger_events[1], 8, 119, 0, 0) &&
                loopScan.loop_controller_events.Length == 1 &&
                ControllerEventMatches(
                    loopScan.loop_controller_events[0], 2, 116, 0, 0),
                "qualified C2GAME2 loop-scan controller census");
        }

        private static void ValidateSelectedControllerCensus(MusicControllerCensus census, int track)
        {
            var tagged = track == 2 || track == 3;
            Require(CensusHeaderMatches(census, track == 2 ? 167 : track == 3 ? 270 : 254) &&
                census.tagged_channels.Length == (tagged ? 2 : 0) &&
                census.trigger_events.Length == (tagged ? 2 : 0) &&
                (!tagged || (census.tagged_channels[0] == 7 && census.tagged_channels[1] == 8 &&
                    ControllerEventMatches(census.trigger_events[0], 7, 119, 0, 0) &&
                    ControllerEventMatches(census.trigger_events[1], 8, 119, 0, 0))) &&
                census.loop_controller_events.Length == (track == 2 ? 1 : 2) &&
                ControllerEventMatches(census.loop_controller_events[0], track == 3 ? 9 : 2,
                    116, track == 3 ? 3 : 0, 0) &&
                (track == 2 || ControllerEventMatches(census.loop_controller_events[1],
                    track == 3 ? 9 : 2, 117, track == 3 ? 86400 : 69117, 127)),
                "qualified selected source music controller census");
        }

        private static bool CensusHeaderMatches(
            MusicControllerCensus census,
            int division)
        {
            return census != null &&
                   census.division == division &&
                   census.midi_format == 0 &&
                   census.track_count == 1 &&
                   census.trigger_controller == 119 &&
                   census.tagged_channels != null &&
                   census.trigger_events != null &&
                   census.loop_controller_events != null;
        }

        private static bool ControllerEventMatches(
            MusicControllerEvent value,
            int channel,
            int controller,
            int tick,
            int eventValue)
        {
            return value != null &&
                   value.channel == channel &&
                   value.controller == controller &&
                   value.tick == tick &&
                   value.value == eventValue;
        }

        private static uint ExpectedSoundAssetRef(int soundId)
        {
            var index = Array.IndexOf(ExactSoundOrder, soundId);
            return index < 0 ? uint.MaxValue : FirstAudioAssetRef + (uint)index;
        }

        private static AudioSource AddSource(GameObject root, string name, bool loop)
        {
            var node = new GameObject(name);
            node.SetActive(false);
            node.transform.SetParent(root.transform, false);
            var source = node.AddComponent<AudioSource>();
            source.playOnAwake = false;
            source.loop = loop;
            source.spatialBlend = 0f;
            source.dopplerLevel = 0f;
            source.rolloffMode = AudioRolloffMode.Custom;
            node.SetActive(true);
            return source;
        }

        private static void DestroyOwnedObject(UnityEngine.Object value)
        {
            if (value == null) return;
            if (Application.isPlaying)
                UnityEngine.Object.Destroy(value);
            else
                UnityEngine.Object.DestroyImmediate(value);
        }

        private static void StopAndDetach(AudioSource source)
        {
            if (source == null) return;
            source.Stop();
            source.clip = null;
        }

        internal static AudioClip CreateMediaClip(string name, byte[] bytes)
        {
            var wave = ParseWave(bytes);
            if (wave.SampleWidth == 1 && wave.Channels == 1)
                return CreatePcm8Clip(name, wave);
            Require(wave.SampleWidth == 2, "media PCM16 or mono PCM8");
            var stream = new StreamingPcm16(wave.Pcm, wave.Channels);
            return AudioClip.Create(name, wave.Frames, wave.Channels,
                wave.SampleRate, true, stream.Read, stream.Seek);
        }

        private static AudioClip CreatePcm8Clip(string name, WaveData wave)
        {
            var samples = new float[wave.Frames];
            for (var index = 0; index < samples.Length; index++)
                samples[index] = (wave.Pcm[index] - 128) / 128f;
            var clip = AudioClip.Create(name, wave.Frames, 1, wave.SampleRate, false);
            Require(clip.SetData(samples, 0), $"upload PCM for {name}");
            return clip;
        }

        private static AudioClip CreateManualStereoClip(
            string name,
            WaveData wave,
            float leftGain,
            float rightGain)
        {
            Require(
                wave.Channels == 1 && wave.SampleWidth == 1 &&
                wave.SampleRate == 22050,
                $"manual original stereo source format for {name}");
            var samples = new float[checked(wave.Frames * 2)];
            for (var frame = 0; frame < wave.Frames; frame++)
            {
                var mono = (wave.Pcm[frame] - 128) / 128f;
                samples[frame * 2] = mono * leftGain;
                samples[frame * 2 + 1] = mono * rightGain;
            }
            var clip = AudioClip.Create(
                $"{name} — original integer stereo",
                wave.Frames,
                2,
                wave.SampleRate,
                false);
            Require(clip.SetData(samples, 0), $"upload original stereo PCM for {name}");
            return clip;
        }

        private readonly struct WaveData
        {
            internal readonly int Channels;
            internal readonly int SampleWidth;
            internal readonly int SampleRate;
            internal readonly int Frames;
            internal readonly byte[] Pcm;

            internal WaveData(int channels, int sampleWidth, int sampleRate, byte[] pcm)
            {
                Channels = channels;
                SampleWidth = sampleWidth;
                SampleRate = sampleRate;
                Pcm = pcm;
                Frames = pcm.Length / (channels * sampleWidth);
            }
        }

        private static WaveData ParseWave(byte[] bytes)
        {
            Require(
                bytes != null && bytes.Length >= 44 &&
                ReadAscii(bytes, 0, 4) == "RIFF" &&
                ReadAscii(bytes, 8, 4) == "WAVE",
                "RIFF/WAVE container");
            var riffBytes = checked((int)ReadU32(bytes, 4) + 8);
            Require(riffBytes <= bytes.Length, "RIFF declared extent");
            var cursor = 12;
            ushort channels = 0;
            ushort bits = 0;
            uint sampleRate = 0;
            byte[] pcm = null;
            while (cursor + 8 <= riffBytes)
            {
                var kind = ReadAscii(bytes, cursor, 4);
                var size = checked((int)ReadU32(bytes, cursor + 4));
                cursor += 8;
                Require(size >= 0 && cursor + size <= riffBytes, "WAVE chunk extent");
                if (kind == "fmt ")
                {
                    Require(size >= 16 && ReadU16(bytes, cursor) == 1, "PCM WAVE format");
                    channels = ReadU16(bytes, cursor + 2);
                    sampleRate = ReadU32(bytes, cursor + 4);
                    bits = ReadU16(bytes, cursor + 14);
                }
                else if (kind == "data")
                {
                    pcm = new byte[size];
                    Buffer.BlockCopy(bytes, cursor, pcm, 0, size);
                }
                cursor += size + (size & 1);
            }
            Require(
                channels > 0 && (bits == 8 || bits == 16) &&
                sampleRate > 0 && pcm != null &&
                pcm.Length % (channels * (bits / 8)) == 0,
                "complete supported PCM WAVE");
            return new WaveData(channels, bits / 8, checked((int)sampleRate), pcm);
        }

        private static ushort ReadU16(byte[] bytes, int offset)
        {
            return (ushort)(bytes[offset] | (bytes[offset + 1] << 8));
        }

        private static uint ReadU32(byte[] bytes, int offset)
        {
            return (uint)(bytes[offset] | (bytes[offset + 1] << 8) |
                (bytes[offset + 2] << 16) | (bytes[offset + 3] << 24));
        }

        private static string ReadAscii(byte[] bytes, int offset, int length)
        {
            return System.Text.Encoding.ASCII.GetString(bytes, offset, length);
        }

        private static void Require(bool condition, string contract)
        {
            if (!condition) throw new InvalidDataException($"owner audio contract failed: {contract}");
        }
    }
}
