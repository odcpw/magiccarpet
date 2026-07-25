using System;
using System.Collections.Generic;
using UnityEngine;

namespace MagicCarpet.Native
{
    /// <summary>
    /// Explicit owner-local qualification script for the already proven Level 1
    /// non-spatial sound surfaces. It is never enabled during normal play.
    /// Commands remain typed and capture evidence comes from the real Unity
    /// listener mix rather than directly from decoded owner samples.
    /// </summary>
    internal sealed class LocalAudioQualification
    {
        private sealed class Stage
        {
            internal string Name;
            internal float Duration;
            internal AudioQualificationCapture.WindowExpectation Expectation;
            internal Action Enter;
            internal Action Exit;
        }

        private readonly OwnerAudioPresentation audio;
        private readonly AudioQualificationCapture capture;
        private readonly NativeSession session;
        private readonly Func<ulong> resetNativeGeneration;
        private readonly List<Stage> stages = new();
        private readonly List<string> failures = new();
        private int stageIndex = -1;
        private long stageStartSample;
        private long priorCapturedSample;
        private float wallElapsed;
        private ulong sequence;
        private int marketTimeBeforeRepeat;
        private int marketStartsBeforeRepeat;
        private int oneShotsBeforeNull;
        private long silentLoopProgressBefore;
        private int silentLoopVoice;
        private int silentLoopStarts;
        private byte lastCommandAction;
        private ushort lastCommandVoice;
        private bool sawExpectedUnsupported;
        private bool sawExpectedUnknown;
        private ulong currentGeneration;

        internal LocalAudioQualification(
            OwnerAudioPresentation audio,
            AudioQualificationCapture capture,
            NativeSession session,
            ulong initialGeneration,
            Func<ulong> resetNativeGeneration)
        {
            this.audio = audio ??
                throw new ArgumentNullException(nameof(audio));
            this.capture = capture ??
                throw new ArgumentNullException(nameof(capture));
            this.session = session ??
                throw new ArgumentNullException(nameof(session));
            this.resetNativeGeneration = resetNativeGeneration ??
                throw new ArgumentNullException(nameof(resetNativeGeneration));
            currentGeneration = initialGeneration;
            if (!capture.IsArmed)
                throw new InvalidOperationException(
                    "local audio qualification requires an armed capture tap");

            BuildStages();
            audio.Reset(false);
            Check(!audio.QualificationMusicIsPlaying,
                "base music is disabled for isolated local-audio evidence");
            EnterNextStage();
            priorCapturedSample = capture.CapturedSamples;
            Debug.Log(
                $"MC_AUDIO_LOCAL_QUAL BEGIN stages={stages.Count} " +
                "music=isolated bank=1 quality=822");
        }

        internal bool Completed { get; private set; }
        internal bool Passed { get; private set; }
        internal string FailureSummary =>
            failures.Count == 0 ? "none" : string.Join(" | ", failures);

        internal void Tick(float deltaTime)
        {
            if (Completed) return;
            wallElapsed += Mathf.Max(0f, deltaTime);
            if (wallElapsed > 60f)
            {
                Check(false, "listener capture timeline completes within 60 seconds");
                Complete();
                return;
            }
            var captured = capture.CapturedSamples;
            var channels = capture.CaptureChannels;
            if (captured < priorCapturedSample)
            {
                Check(false, "captured sample clock is monotonic");
                Complete();
                return;
            }
            if (channels > 0 && captured > priorCapturedSample)
            {
                var capturedSeconds =
                    (captured - priorCapturedSample) /
                    (double)(capture.CaptureSampleRate * channels);
                audio.Tick((float)capturedSeconds);
            }
            priorCapturedSample = captured;
            if (channels <= 0) return;
            var requiredSamples = (long)Math.Ceiling(
                stages[stageIndex].Duration *
                capture.CaptureSampleRate * channels);
            if (captured - stageStartSample >= requiredSamples)
            {
                stages[stageIndex].Exit?.Invoke();
                EnterNextStage();
            }
        }

        private void BuildStages()
        {
            Add("silence_pre", 0.35f, false);
            Add("ocean_ramp_early", 0.35f, true,
                () => Prepare(1));
            Add("ocean_ramp_late", 0.85f, true);
            Add("ocean_steady", 0.40f, true, null, () => {
                Check(audio.QualificationLoopUnits(1) == 70,
                    "ocean reaches exact target volume 70");
                Check(audio.QualificationLoopPlayStarts(1) == 1,
                    "ocean starts exactly once");
            });
            Add("ocean_to_crickets", 1.25f, true,
                () => {
                    Prepare(2);
                    End(1);
                },
                () => {
                    Check(audio.QualificationLoopUnits(1) == 0,
                        "ocean reaches zero during water-to-land transition");
                    Check(audio.QualificationLoopUnits(2) == 70,
                        "crickets reach exact target volume 70");
                    Check(
                        audio.QualificationLoopIsPlaying(1) &&
                        audio.QualificationLoopIsPlaying(2) &&
                        audio.QualificationActiveLoopCount == 2,
                        "zero-volume ocean remains resident beside audible crickets");
                });
            Add("crickets_steady", 0.35f, true);
            Add("crickets_fade", 1.35f, true,
                () => End(2));
            Add("ambient_stop_silence", 0.35f, false,
                () => {
                    silentLoopProgressBefore =
                        audio.QualificationLoopProgressSamples(2);
                    silentLoopVoice = audio.QualificationLoopVoiceIndex(2);
                    silentLoopStarts = audio.QualificationLoopPlayStarts(2);
                },
                () => {
                    Check(
                        audio.QualificationLoopIsPlaying(2) &&
                        audio.QualificationLoopUnits(2) == 0 &&
                        audio.QualificationLoopProgressSamples(2) >
                            silentLoopProgressBefore,
                        "silent crickets remain resident and sample phase advances");
                    Prepare(2);
                    Check(
                        lastCommandAction == NativeSim.AudioActionUpdateExisting &&
                        lastCommandVoice == silentLoopVoice &&
                        audio.QualificationLoopVoiceIndex(2) == silentLoopVoice &&
                        audio.QualificationLoopPlayStarts(2) == silentLoopStarts,
                        "reactivation uses UPDATE_EXISTING on the same voice " +
                        "without restarting phase");
                    End(2);
                });

            Add("fire_distance_near_ramp", 2.05f, true,
                () => Prepare(5));
            Add("fire_distance_near_steady", 0.35f, true, null, () => {
                Check(audio.QualificationLoopUnits(5) == 120,
                    "near-fire loop reaches exact target volume 120");
            });
            Add("fire_distance_far_fade", 2.05f, true,
                () => End(5));
            Add("fire_distance_far_silence", 0.35f, false, null, () => {
                Check(
                    audio.QualificationLoopIsPlaying(5) &&
                    audio.QualificationLoopUnits(5) == 0,
                    "far-fire command leaves a silent resident loop");
            });

            Add("market_distance_near_ramp", 1.50f, true,
                () => Prepare(31));
            Add("market_distance_near_steady", 0.35f, true, null, () => {
                Check(audio.QualificationLoopUnits(31) == 85,
                    "near-market loop reaches exact target volume 85");
                marketTimeBeforeRepeat =
                    audio.QualificationLoopTimeSamples(31);
                marketStartsBeforeRepeat =
                    audio.QualificationLoopPlayStarts(31);
            });
            Add("market_repeat_continuity", 0.35f, true,
                () => Prepare(31),
                () => {
                    var after = audio.QualificationLoopTimeSamples(31);
                    Check(
                        marketTimeBeforeRepeat >= 0 &&
                        after > marketTimeBeforeRepeat,
                        "repeated PrepareSound advances rather than rewinds market");
                    Check(
                        audio.QualificationLoopPlayStarts(31) ==
                        marketStartsBeforeRepeat,
                        "repeated PrepareSound does not restart market");
                });
            Add("market_distance_far_fade", 1.50f, true,
                () => End(31));
            Add("market_distance_far_silence", 0.35f, false, null, () => {
                Check(
                    audio.QualificationLoopIsPlaying(31) &&
                    audio.QualificationLoopUnits(31) == 0,
                    "far-market command leaves a silent resident loop");
            });

            Add("cantuse_local_feedback", 0.45f, true,
                () => Prepare(29),
                () => {
                    Check(audio.QualificationOneShotPlayCount == 1,
                        "CANTUSE local feedback plays exactly once");
                });
            Add("feedback_tail_silence", 0.35f, false);
            Add("explicit_null_silence", 0.30f, false,
                () => {
                    oneShotsBeforeNull =
                        audio.QualificationOneShotPlayCount;
                    Prepare(12);
                },
                () => {
                    Check(
                        audio.QualificationOneShotPlayCount ==
                        oneShotsBeforeNull &&
                        audio.QualificationActiveLoopCount == 4,
                        "explicit NULL creates no source playback");
                });
            Add("missing_positional_sidecar_silence", 0.30f, false,
                () => Prepare(37),
                () => {
                    sawExpectedUnsupported =
                        audio.HadSuppressedUnsupportedPositional;
                    Check(sawExpectedUnsupported,
                        "positional event missing its sidecar is explicitly suppressed");
                });
            Add("unknown_mapping_silence", 0.30f, false,
                () => Prepare(999),
                () => {
                    sawExpectedUnknown = audio.HadSuppressedUnknown;
                    Check(sawExpectedUnknown,
                        "unknown event is explicitly suppressed");
                });
            Add("reset_generation_prepare", 0.30f, true,
                () => Prepare(1));
            Add("reset_generation_cleanup", 0.40f, false,
                () => {
                    var oldGeneration = currentGeneration;
                    currentGeneration = resetNativeGeneration();
                    Check(currentGeneration != oldGeneration,
                        "native reset advances the authoritative generation");
                    audio.Reset(false);
                },
                () => {
                    Check(
                        audio.QualificationActiveLoopCount == 0 &&
                        !audio.QualificationOneShotIsPlaying &&
                        !audio.QualificationMusicIsPlaying,
                        "reset clears loops, one-shots and music without stale generation audio");
                    Check(sawExpectedUnsupported && sawExpectedUnknown,
                        "negative-probe evidence survives reset verdict accounting");
                });
        }

        private void Add(
            string name,
            float duration,
            bool audible,
            Action enter = null,
            Action exit = null)
        {
            stages.Add(new Stage {
                Name = name,
                Duration = duration,
                Expectation = audible
                    ? AudioQualificationCapture.WindowExpectation.Audible
                    : AudioQualificationCapture.WindowExpectation.Silent,
                Enter = enter,
                Exit = exit
            });
        }

        private void EnterNextStage()
        {
            stageIndex++;
            if (stageIndex >= stages.Count)
            {
                Complete();
                return;
            }
            var stage = stages[stageIndex];
            stage.Enter?.Invoke();
            capture.BeginWindow(stage.Name, stage.Expectation);
            stageStartSample = capture.CapturedSamples;
            Debug.Log(
                $"MC_AUDIO_LOCAL_QUAL STAGE index={stageIndex} " +
                $"name={stage.Name} duration={stage.Duration:F2} " +
                $"expected={stage.Expectation.ToString().ToLowerInvariant()}");
        }

        private void Complete()
        {
            var summary = capture.Finish();
            Check(summary != null, "capture produces a summary");
            if (summary != null)
            {
                Check(summary.AllWindowsPass,
                    "every audible/silent listener window meets its threshold");
                var early = summary.FindWindow("ocean_ramp_early");
                var late = summary.FindWindow("ocean_ramp_late");
                Check(
                    early != null && late != null &&
                    early.Rms < late.Rms,
                    "start-at-zero ocean ramp gains measured listener energy");
                CheckContinuous(summary, "ocean_to_crickets");
                CheckContinuous(summary, "market_repeat_continuity");
            }
            Completed = true;
            Passed = failures.Count == 0;
            Debug.Log(
                $"MC_AUDIO_LOCAL_QUAL {(Passed ? "PASS" : "FAIL")} " +
                $"stages={stages.Count} windows={summary?.Windows.Count ?? 0} " +
                $"captureSamples={summary?.Samples ?? 0} " +
                $"nonZero={summary?.NonZeroSamples ?? 0} " +
                $"failures={FailureSummary}");
        }

        private void CheckContinuous(
            AudioQualificationCapture.CaptureSummary summary,
            string name)
        {
            var window = summary.FindWindow(name);
            Check(
                window != null &&
                window.MinimumBlockRms >= 0.00001,
                $"{name} has no silent 20 ms listener block");
        }

        private void Prepare(int soundId)
        {
            Consume(NativeSim.EventPrepareSound, soundId);
        }

        private void End(int soundId)
        {
            Consume(NativeSim.EventEndLoop, soundId);
        }

        private void Consume(uint kind, int soundId)
        {
            var nativeEvent = new NativeSim.NativeEvent {
                abiVersion = NativeSim.AbiVersion,
                structSize = 72,
                generation = currentGeneration,
                sequence = ++sequence,
                kind = kind,
                code = unchecked((uint)soundId),
                entityIndex = 152,
                playerIndex = 0,
                assetRefIndex = SoundAssetRef(soundId),
                reserved = new uint[4]
            };
            Debug.Log(
                $"MC_AUDIO_LOCAL_QUAL COMMAND generation={currentGeneration} " +
                $"sequence={sequence} kind={kind} id={soundId} " +
                $"asset={nativeEvent.assetRefIndex}");
            if ((kind == NativeSim.EventPrepareSound ||
                 kind == NativeSim.EventEndLoop) &&
                (soundId == 1 || soundId == 2 || soundId == 5 ||
                 soundId == 29 || soundId == 31))
            {
                session.SubmitAudioQualification(
                    kind,
                    checked((ushort)soundId),
                    0,
                    soundId == 29 ? (ushort)0x7fff : (ushort)0,
                    0x7fff,
                    soundId == 29 ? (byte)3 : NativeSim.AudioPlayIfNotPlaying,
                    out nativeEvent,
                    out var command);
                currentGeneration = nativeEvent.generation;
                sequence = nativeEvent.sequence;
                lastCommandAction = command.action;
                lastCommandVoice = command.voiceIndex;
                audio.Consume(nativeEvent, command);
            }
            else
            {
                audio.Consume(nativeEvent);
            }
        }

        private static uint SoundAssetRef(int soundId)
        {
            return soundId switch {
                1 => 76,
                2 => 77,
                5 => 78,
                12 => 79,
                29 => 80,
                31 => 81,
                37 => 82,
                _ => uint.MaxValue
            };
        }

        private void Check(bool condition, string contract)
        {
            if (condition) return;
            failures.Add(contract);
            Debug.LogError($"MC_AUDIO_LOCAL_QUAL ASSERT_FAIL contract={contract}");
        }
    }
}
