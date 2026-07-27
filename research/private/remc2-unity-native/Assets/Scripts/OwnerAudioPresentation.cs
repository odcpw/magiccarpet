using System;
using System.Collections.Generic;
using System.IO;
using System.Threading;
using UnityEngine;

namespace MagicCarpet.Native
{
    /// <summary>
    /// Owner-local Level 1 audio presentation. It consumes only validated
    /// cache outputs and typed simulation events; no owner bytes are embedded
    /// in the Unity project or build.
    /// </summary>
    internal sealed class OwnerAudioPresentation : IDisposable
    {
        private const uint FirstAudioAssetRef = 76;
        private const uint MusicAssetRef = 107;
        private const int MusicIdleMode = 1;
        private const int MusicCombatMode = 2;
        private static readonly int[] ExactSoundOrder =
        {
            1, 2, 3, 4, 5, 8, 9, 10, 12, 15, 16, 18, 19, 27, 29, 30, 31,
            33, 34, 37, 39, 40, 41, 43, 44, 46, 54, 55, 56, 57, 63
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
            public int channel;
            public string driver_type;
            public int driver_index;
            public int logical_track;
            public int playback_slot;
            public int loop_scan_pointer_slot;
            public string unity_wave_path;
            public string unity_wave_sha256;
            public string initial_start_status;
            public string optional_random_OGG_path;
            public string exact_DOS_hardware_timbre;
            public MusicDynamicControl dynamic_control;
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
        private readonly Dictionary<int, LoopState> loops = new();
        private readonly PositionalVoice[] positionalVoices;
        private readonly AudioSource musicSource;
        private readonly AudioClip musicClip;
        private readonly StreamingPcm16 musicStream;
        private readonly Action<ulong, ushort, ulong> voiceCompleted;
        private float fadeAccumulator;
        private float musicFadeAccumulator;
        private int oneShotPlayCount;
        private int musicStartCount;
        private bool musicUpdatesEnabled;
        private bool musicTimerActive;
        private int musicTargetMode = MusicIdleMode;
        private int musicCurrentUnits;
        private int musicDirection = -1;
        private int musicTimerMultiplier = 3;
        private bool disposed;
        private static int qualificationTransactionalCleanupCount;
        internal bool HadSuppressedUnknown { get; private set; }
        internal bool HadSuppressedUnsupportedPositional { get; private set; }

        private OwnerAudioPresentation(
            GameObject root,
            AudioSource musicSource,
            AudioClip musicClip,
            StreamingPcm16 musicStream,
            Action<ulong, ushort, ulong> voiceCompleted)
        {
            this.root = root;
            this.musicSource = musicSource;
            this.musicClip = musicClip;
            this.musicStream = musicStream;
            this.voiceCompleted = voiceCompleted;
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
            int qualificationFailAfterSoundRows = -1)
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
                    root, musicSource, musicClip, stream, voiceCompleted);
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
                    if (musicSource != null)
                    {
                        musicSource.Stop();
                        musicSource.clip = null;
                    }
                    if (musicClip != null) UnityEngine.Object.Destroy(musicClip);
                    if (root != null) UnityEngine.Object.Destroy(root);
                }
                Interlocked.Increment(ref qualificationTransactionalCleanupCount);
                throw;
            }
        }

        internal void LogProbe(int tick)
        {
            Debug.Log(
                $"MC_AUDIO_PROBE tick={tick} listener={AudioListener.volume:F3}/" +
                $"{AudioListener.pause} sourcePlaying={musicSource.isPlaying} " +
                $"timeSamples={musicSource.timeSamples} streamReads={musicStream.ReadCalls} " +
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
                    state.Source.volume = state.CurrentUnits / 127f;
                }
            }
            if (musicTimerActive)
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
                    if (nativeEvent.assetRefIndex != MusicAssetRef ||
                        (nativeEvent.code != 1 && nativeEvent.code != 2) ||
                        nativeEvent.value0 != 3)
                    {
                        Suppress(nativeEvent, "unknown_music_update");
                        break;
                    }
                    UpdateMusicEnvelope((int)nativeEvent.code, nativeEvent.value0);
                    break;
                case NativeSim.EventStartMusic:
                    if (nativeEvent.assetRefIndex != MusicAssetRef ||
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
                    UnityEngine.Object.Destroy(state.RenderedClip);
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
                if (clip != null) UnityEngine.Object.Destroy(clip);
            clips.Clear();
            if (musicClip != null) UnityEngine.Object.Destroy(musicClip);
            if (root != null) UnityEngine.Object.Destroy(root);
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
            Require(nativeEvent.assetRefIndex == ExpectedSoundAssetRef(soundId),
                $"sound ID {soundId} descriptor-backed asset reference");
            if (soundId == 12) return;
            if (!audioCommand.HasValue)
            {
                Suppress(
                    nativeEvent,
                    "missing_audio_command",
                    row.playback_scope == "original_stereo_positional");
                return;
            }
            var command = audioCommand.Value;
            ValidateAudioCommand(nativeEvent, command, row.play_class, soundId);
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
                    UnityEngine.Object.Destroy(state.RenderedClip);
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
            state.Source.volume = 1f;
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
                command.reserved0 == 0 &&
                command.reserved != null &&
                Array.TrueForAll(command.reserved, value => value == 0),
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
                    UnityEngine.Object.Destroy(state.RenderedClip);
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
            Require(nativeEvent.assetRefIndex == ExpectedSoundAssetRef(soundId),
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

        private static void ValidateMusicDynamicControl(MusicDynamicControl control)
        {
            Require(
                control != null &&
                control.audible_channel_resolution ==
                    "Unresolved_playback_slot0_has_no_CC119_but_loop_scan_slot1_tags_channels7_8" &&
                control.envelope_min == 0 &&
                control.envelope_max == 127 &&
                control.envelope_step_per_callback == 1 &&
                control.timer_hz == 90 &&
                control.timer_hz_formula == "30 * timer_multiplier" &&
                control.timer_multiplier == 3 &&
                control.update_mode_field == "target_mode",
                "qualified source dynamic-music control constants");

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
