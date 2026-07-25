using System;
using System.Collections.Generic;
using UnityEngine;

namespace MagicCarpet.Native
{
    /// <summary>
    /// Explicit owner-local listener qualification for the original MC2
    /// integer stereo sidecar. The vectors originate in Rust and cross the
    /// native ABI before this script presents them.
    /// </summary>
    internal sealed class PositionalAudioQualification
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
        private readonly List<Stage> stages = new();
        private readonly List<string> failures = new();
        private int stageIndex = -1;
        private long stageStartSample;
        private long priorCapturedSample;
        private float wallElapsed;
        private int nearStarts;
        private int nearSuppressed;
        private ulong liveGeneration;

        internal PositionalAudioQualification(
            OwnerAudioPresentation audio,
            AudioQualificationCapture capture,
            NativeSession session)
        {
            this.audio = audio ?? throw new ArgumentNullException(nameof(audio));
            this.capture = capture ??
                throw new ArgumentNullException(nameof(capture));
            this.session = session ??
                throw new ArgumentNullException(nameof(session));
            if (!capture.IsArmed)
                throw new InvalidOperationException(
                    "positional audio qualification requires an armed capture tap");
            Check(
                NativeSim.mc_native_audio_qualification_vector_count() == 23,
                "Rust positional qualification vector count is 23");
            BuildStages();
            audio.Reset(false);
            EnterNextStage();
            priorCapturedSample = capture.CapturedSamples;
            Debug.Log(
                $"MC_AUDIO_POSITIONAL_QUAL BEGIN stages={stages.Count} " +
                "source=Rust_ABI_sidecar spatializer=manual_original_stereo");
        }

        internal bool Completed { get; private set; }
        internal bool Passed { get; private set; }
        internal string FailureSummary =>
            failures.Count == 0 ? "none" : string.Join(" | ", failures);

        internal void Tick(float deltaTime)
        {
            if (Completed) return;
            audio.Tick(deltaTime);
            wallElapsed += Mathf.Max(0f, deltaTime);
            if (wallElapsed > 45f)
            {
                Check(false, "positional listener timeline completes within 45 seconds");
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
            Add("positional_silence_pre", 0.35f, false);
            Add("positional_left", 0.45f, true,
                () => PlayVector(6),
                () => {
                    Check(
                        Mathf.Approximately(audio.QualificationPositionalPan(37), -1f),
                        "left vector maps original pan unit 0 to Unity -1");
                    Check(
                        Mathf.Approximately(
                            audio.QualificationPositionalVolume(37), 113f / 127f),
                        "left vector maps raw volume through original >>8 wrapper");
                });
            Add("positional_right", 0.45f, true,
                () => PlayVector(2),
                () => Check(
                    Mathf.Approximately(audio.QualificationPositionalPan(39), 1f),
                    "right vector maps original pan unit 127 to Unity +1"));
            Add("positional_near", 0.45f, true,
                () => PlayVector(0),
                () => {
                    Check(
                        Mathf.Approximately(
                            audio.QualificationPositionalPan(43), -1f / 64f),
                        "front center preserves original integer pan unit 63");
                    Check(
                        Mathf.Approximately(audio.QualificationPositionalPitch(43), 1f),
                        "neutral rate vector presents pitch 1");
                });
            Add("positional_far", 0.45f, true,
                () => PlayVector(21));
            Add("positional_rate_low", 0.40f, true,
                () => PlayVector(18),
                () => Check(
                    Mathf.Approximately(
                        audio.QualificationPositionalPitch(43), 18742f / 22050f),
                    "rate -15 vector presents exact integer playback-rate ratio"));
            Add("positional_rate_high", 0.40f, true,
                () => PlayVector(20),
                () => Check(
                    Mathf.Approximately(
                        audio.QualificationPositionalPitch(43), 25137f / 22050f),
                    "rate +14 vector presents exact integer playback-rate ratio"));
            Add("positional_if_not_playing", 0.45f, true,
                () => {
                    var command = Vector(0);
                    audio.QualificationStopPositionals();
                    Consume(command);
                    nearStarts = audio.QualificationPositionalPlayStarts(43);
                    nearSuppressed =
                        audio.QualificationPositionalSuppressedExisting(43);
                    Consume(Vector(22));
                },
                () => {
                    Check(
                        audio.QualificationPositionalPlayStarts(43) == nearStarts,
                        "IfNotPlaying duplicate does not restart a matching live voice");
                    Check(
                        audio.QualificationPositionalSuppressedExisting(43) ==
                            nearSuppressed + 1,
                        "IfNotPlaying duplicate records one existing-voice suppression");
                });
            Add("positional_below_volume_silence", 0.35f, false,
                () => {
                    audio.QualificationStopPositionals();
                    Consume(Vector(11));
                });
            Add("positional_missing_command_silence", 0.35f, false,
                () => {
                    audio.QualificationStopPositionals();
                    audio.Consume(EventWithoutCommand(37, 82));
                },
                () => Check(
                    audio.HadSuppressedUnsupportedPositional,
                    "positional ID without a complete sidecar fails closed"));
            Add("positional_unknown_silence", 0.35f, false,
                () => {
                    audio.QualificationStopPositionals();
                    audio.Consume(EventWithoutCommand(999, uint.MaxValue));
                },
                () => Check(
                    audio.HadSuppressedUnknown,
                    "unknown positional mapping remains silent"));
            Add("positional_silence_post", 0.35f, false,
                audio.QualificationStopPositionals);
            Add("live_completion_first", 0.55f, true,
                () => {
                    var pair = SubmitLive(29, 0, 0x7fff, 0x7fff, 3);
                    liveGeneration = pair.command.generation;
                    Check(
                        pair.command.action == NativeSim.AudioActionStart &&
                        pair.command.voiceIndex == 0,
                        "live allocator first completion probe owns voice zero");
                    audio.Consume(pair.nativeEvent, pair.command);
                });
            Add("live_completion_reuse", 0.55f, true,
                () => {
                    var pair = SubmitLive(29, 0, 0x7fff, 0x7fff, 3);
                    Check(
                        pair.command.action == NativeSim.AudioActionStart &&
                        pair.command.voiceIndex == 0,
                        "natural Unity completion callback releases voice zero for reuse");
                    audio.Consume(pair.nativeEvent, pair.command);
                });
            Add("live_pool_full", 0.40f, true,
                () => {
                    audio.QualificationStopPositionals();
                    for (short flags = 100; flags < 110; flags++)
                    {
                        var pair = SubmitLive(37, flags, 512, 0x7fff, 2);
                        Check(
                            pair.command.action == NativeSim.AudioActionStart &&
                            pair.command.voiceIndex == flags - 100,
                            $"shared pool assigns first-free voice {flags - 100}");
                        audio.Consume(pair.nativeEvent, pair.command, false);
                    }
                    var full = SubmitLive(37, 999, 512, 0x7fff, 2);
                    Check(
                        full.command.action == NativeSim.AudioActionSuppressNoVoice &&
                        full.command.voiceIndex == NativeSim.AudioVoiceNone,
                        "eleventh shared effect is dropped without stealing");
                    audio.Consume(full.nativeEvent, full.command, false);
                    var duplicate = SubmitLive(37, 100, 512, 0x7fff, 2);
                    Check(
                        duplicate.command.action ==
                            NativeSim.AudioActionSuppressExisting &&
                        duplicate.command.voiceIndex == 0,
                        "full pool still identifies and suppresses matching live effect");
                    audio.Consume(duplicate.nativeEvent, duplicate.command, false);
                });
            Add("live_reset_generation_cleanup", 0.35f, true,
                () => {
                    audio.Reset(false);
                    var reset = session.Reset();
                    Check(
                        reset.generation != liveGeneration,
                        "native reset increments allocator generation");
                    var pair = SubmitLive(29, 0, 0x7fff, 0x7fff, 3);
                    Check(
                        pair.command.generation == reset.generation &&
                        pair.command.action == NativeSim.AudioActionStart &&
                        pair.command.voiceIndex == 0,
                        "reset clears all ten voices and restarts first-free at zero");
                    audio.Consume(pair.nativeEvent, pair.command);
                });
            Add("live_silence_post", 0.35f, false,
                () => audio.Reset(false));
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
                $"MC_AUDIO_POSITIONAL_QUAL STAGE index={stageIndex} " +
                $"name={stage.Name} duration={stage.Duration:F2} " +
                $"expected={stage.Expectation.ToString().ToLowerInvariant()}");
        }

        private void Complete()
        {
            var summary = capture.Finish();
            Check(summary != null, "positional capture produces a summary");
            if (summary != null)
            {
                Check(summary.AllWindowsPass,
                    "every positional listener window meets its threshold");
                var left = summary.FindWindow("positional_left");
                var right = summary.FindWindow("positional_right");
                var near = summary.FindWindow("positional_near");
                var far = summary.FindWindow("positional_far");
                var rateLow = summary.FindWindow("positional_rate_low");
                var rateHigh = summary.FindWindow("positional_rate_high");
                Check(
                    left != null && left.LeftRms > left.RightRms * 4,
                    "left vector is left-channel dominant after the Unity listener");
                Check(
                    right != null && right.RightRms > right.LeftRms * 4,
                    "right vector is right-channel dominant after the Unity listener");
                Check(
                    near != null && far != null && near.Rms > far.Rms * 1.3,
                    "near vector has materially more listener energy than far vector");
                Check(
                    rateLow != null && rateHigh != null &&
                    rateHigh.LeftZeroCrossings >
                        rateLow.LeftZeroCrossings * 1.15,
                    "captured high-rate signal has materially more zero crossings " +
                    "than captured low-rate signal");
            }
            Completed = true;
            Passed = failures.Count == 0;
            Debug.Log(
                $"MC_AUDIO_POSITIONAL_QUAL {(Passed ? "PASS" : "FAIL")} " +
                $"stages={stages.Count} windows={summary?.Windows.Count ?? 0} " +
                $"captureSamples={summary?.Samples ?? 0} " +
                $"failures={FailureSummary}");
        }

        private void PlayVector(uint index)
        {
            audio.QualificationStopPositionals();
            Consume(Vector(index));
        }

        private static NativeSim.AudioCommand Vector(uint index)
        {
            var status = NativeSim.mc_native_audio_qualification_vector(
                index, out var command, 88);
            NativeSession.Check(status, $"read positional qualification vector {index}");
            return command;
        }

        private void Consume(NativeSim.AudioCommand command)
        {
            var nativeEvent = new NativeSim.NativeEvent {
                abiVersion = NativeSim.AbiVersion,
                structSize = 72,
                generation = command.generation,
                sequence = command.eventSequence,
                stateHash = command.stateHash,
                kind = NativeSim.EventPrepareSound,
                code = command.soundId,
                entityIndex = command.entityIndex,
                playerIndex = command.playerIndex,
                assetRefIndex = command.assetRefIndex,
                reserved = new uint[4]
            };
            audio.Consume(nativeEvent, command, false);
        }

        private (NativeSim.NativeEvent nativeEvent, NativeSim.AudioCommand command)
            SubmitLive(
                ushort soundId,
                short soundFlags,
                ushort volumeQ15,
                ushort panQ16,
                byte playType)
        {
            session.SubmitAudioQualification(
                NativeSim.EventPrepareSound,
                soundId,
                soundFlags,
                volumeQ15,
                panQ16,
                playType,
                out var nativeEvent,
                out var command);
            return (nativeEvent, command);
        }

        private static NativeSim.NativeEvent EventWithoutCommand(
            int soundId,
            uint assetRef)
        {
            return new NativeSim.NativeEvent {
                abiVersion = NativeSim.AbiVersion,
                structSize = 72,
                generation = 1,
                sequence = 10_000 + unchecked((uint)soundId),
                kind = NativeSim.EventPrepareSound,
                code = unchecked((uint)soundId),
                entityIndex = 152,
                playerIndex = -1,
                assetRefIndex = assetRef,
                reserved = new uint[4]
            };
        }

        private void Check(bool condition, string contract)
        {
            if (condition) return;
            failures.Add(contract);
            Debug.LogError(
                $"MC_AUDIO_POSITIONAL_QUAL ASSERT_FAIL contract={contract}");
        }
    }
}
