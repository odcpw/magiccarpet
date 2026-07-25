using System;
using System.Collections.Generic;
using System.IO;
using UnityEngine;

namespace MagicCarpet.Native
{
    /// <summary>
    /// Explicitly enabled qualification tap for the post-listener Unity mix.
    /// It is dormant unless MC_AUDIO_CAPTURE_PATH is set and never packages
    /// owner audio into Assets or Builds.
    /// </summary>
    internal sealed class AudioQualificationCapture : MonoBehaviour
    {
        internal enum WindowExpectation
        {
            Audible,
            Silent
        }

        internal sealed class WindowResult
        {
            internal string Name;
            internal WindowExpectation Expectation;
            internal long StartSample;
            internal long MeasurementStartSample;
            internal long EndSample;
            internal long NonZeroSamples;
            internal float Peak;
            internal double Rms;
            internal double LeftRms;
            internal double RightRms;
            internal long LeftZeroCrossings;
            internal double MinimumBlockRms;
            internal bool ExpectedPass;
        }

        internal sealed class CaptureSummary
        {
            internal string OutputPath;
            internal int Channels;
            internal int SampleRate;
            internal long Samples;
            internal long NonZeroSamples;
            internal float Peak;
            internal bool AllWindowsPass;
            internal readonly List<WindowResult> Windows = new();

            internal WindowResult FindWindow(string name)
            {
                return Windows.Find(window => window.Name == name);
            }
        }

        private sealed class WindowMarker
        {
            internal string Name;
            internal WindowExpectation Expectation;
            internal long StartSample;
        }

        private readonly object gate = new();
        private readonly List<WindowMarker> windowMarkers = new();
        private MemoryStream pcm;
        private string outputPath;
        private int channels;
        private int sampleRate;
        private long nonZeroSamples;
        private float peak;
        private bool finished;
        private CaptureSummary finishedSummary;

        private void Awake()
        {
            var requested = Environment.GetEnvironmentVariable("MC_AUDIO_CAPTURE_PATH");
            if (string.IsNullOrWhiteSpace(requested))
            {
                enabled = false;
                return;
            }
            outputPath = ResolveQualifiedCapturePath(requested);
            sampleRate = AudioSettings.outputSampleRate;
            pcm = new MemoryStream(sampleRate * 2 * 2 * 15);
            Debug.Log($"MC_AUDIO_CAPTURE ARMED path={outputPath} rate={sampleRate}");
        }

        private void OnAudioFilterRead(float[] data, int callbackChannels)
        {
            if (!enabled || pcm == null || data == null) return;
            lock (gate)
            {
                if (finished) return;
                if (channels == 0) channels = callbackChannels;
                if (channels != callbackChannels) return;
                foreach (var sample in data)
                {
                    var clamped = Mathf.Clamp(sample, -1f, 1f);
                    if (clamped != 0f) nonZeroSamples++;
                    peak = Mathf.Max(peak, Mathf.Abs(clamped));
                    var value = (short)Mathf.RoundToInt(clamped * short.MaxValue);
                    pcm.WriteByte((byte)value);
                    pcm.WriteByte((byte)(value >> 8));
                }
            }
        }

        internal bool IsArmed => enabled && pcm != null && !finished;
        internal int CaptureChannels
        {
            get
            {
                lock (gate) return channels;
            }
        }

        internal int CaptureSampleRate => sampleRate;

        internal long CapturedSamples
        {
            get
            {
                lock (gate) return pcm?.Length / 2 ?? 0;
            }
        }

        internal void BeginWindow(string name, WindowExpectation expectation)
        {
            if (string.IsNullOrWhiteSpace(name))
                throw new InvalidDataException("audio capture window name is empty");
            lock (gate)
            {
                if (!IsArmed)
                    throw new InvalidDataException(
                        "audio capture window requested while capture is not armed");
                if (windowMarkers.Exists(marker => marker.Name == name))
                    throw new InvalidDataException(
                        $"duplicate audio capture window name: {name}");
                var startSample = pcm.Length / 2;
                windowMarkers.Add(new WindowMarker {
                    Name = name,
                    Expectation = expectation,
                    StartSample = startSample
                });
                Debug.Log(
                    $"MC_AUDIO_CAPTURE WINDOW_BEGIN name={name} " +
                    $"expected={expectation.ToString().ToLowerInvariant()} " +
                    $"sample={startSample}");
            }
        }

        internal CaptureSummary Finish()
        {
            lock (gate)
            {
                if (finished) return finishedSummary;
                if (pcm == null) return null;
                finished = true;
                var revalidated = ResolveQualifiedCapturePath(outputPath);
                if (revalidated != outputPath)
                    throw new InvalidDataException("audio capture path changed after validation");
                var payload = pcm.ToArray();
                using var output = File.Create(outputPath);
                using var writer = new BinaryWriter(output);
                writer.Write(System.Text.Encoding.ASCII.GetBytes("RIFF"));
                writer.Write(36 + payload.Length);
                writer.Write(System.Text.Encoding.ASCII.GetBytes("WAVEfmt "));
                writer.Write(16);
                writer.Write((short)1);
                writer.Write((short)channels);
                writer.Write(sampleRate);
                writer.Write(sampleRate * channels * 2);
                writer.Write((short)(channels * 2));
                writer.Write((short)16);
                writer.Write(System.Text.Encoding.ASCII.GetBytes("data"));
                writer.Write(payload.Length);
                writer.Write(payload);
                finishedSummary = BuildSummary(payload);
                Debug.Log(
                    $"MC_AUDIO_CAPTURE COMPLETE path={outputPath} channels={channels} " +
                    $"rate={sampleRate} samples={payload.Length / 2} " +
                    $"nonZero={nonZeroSamples} peak={peak:F6} " +
                    $"windows={finishedSummary.Windows.Count} " +
                    $"windowPass={finishedSummary.AllWindowsPass}");
                return finishedSummary;
            }
        }

        private CaptureSummary BuildSummary(byte[] payload)
        {
            var summary = new CaptureSummary {
                OutputPath = outputPath,
                Channels = channels,
                SampleRate = sampleRate,
                Samples = payload.Length / 2,
                NonZeroSamples = nonZeroSamples,
                Peak = peak,
                AllWindowsPass = windowMarkers.Count > 0 && channels > 0
            };
            for (var index = 0; index < windowMarkers.Count; index++)
            {
                var marker = windowMarkers[index];
                var endSample = index + 1 < windowMarkers.Count
                    ? windowMarkers[index + 1].StartSample
                    : summary.Samples;
                var result = MeasureWindow(
                    payload, marker.Name, marker.Expectation,
                    marker.StartSample, endSample);
                summary.Windows.Add(result);
                summary.AllWindowsPass &= result.ExpectedPass;
                Debug.Log(
                    $"MC_AUDIO_WINDOW name={result.Name} " +
                    $"expected={result.Expectation.ToString().ToLowerInvariant()} " +
                    $"start={result.StartSample} " +
                    $"measurementStart={result.MeasurementStartSample} " +
                    $"end={result.EndSample} " +
                    $"samples={result.EndSample - result.MeasurementStartSample} " +
                    $"nonZero={result.NonZeroSamples} peak={result.Peak:F6} " +
                    $"rms={result.Rms:F8} leftRms={result.LeftRms:F8} " +
                    $"rightRms={result.RightRms:F8} " +
                    $"leftZeroCrossings={result.LeftZeroCrossings} " +
                    $"minBlockRms={result.MinimumBlockRms:F8} " +
                    $"pass={result.ExpectedPass}");
            }
            return summary;
        }

        private WindowResult MeasureWindow(
            byte[] payload,
            string name,
            WindowExpectation expectation,
            long requestedStart,
            long requestedEnd)
        {
            var totalSamples = payload.Length / 2;
            var start = Math.Max(0, Math.Min(totalSamples, requestedStart));
            var end = Math.Max(start, Math.Min(totalSamples, requestedEnd));
            var blockSamples = Math.Max(
                Math.Max(1, channels),
                sampleRate * Math.Max(1, channels) / 50);
            // Window markers are written by the main thread while samples are
            // appended by Unity's audio thread. A callback already in flight
            // can therefore straddle a transition marker. Preserve the raw
            // marker for auditability, but qualify silent steady state after
            // one fixed 20 ms audio block. Audible ramp windows intentionally
            // retain their first samples.
            var measurementStart = expectation == WindowExpectation.Silent
                ? Math.Min(end, start + blockSamples)
                : start;
            long nonZero = 0;
            double sumSquares = 0;
            double leftSquares = 0;
            double rightSquares = 0;
            long leftSamples = 0;
            long rightSamples = 0;
            double blockSquares = 0;
            var blockCount = 0;
            var minimumBlockRms = double.MaxValue;
            var localPeak = 0f;
            short priorLeft = 0;
            var havePriorLeft = false;
            long leftZeroCrossings = 0;
            for (var sampleIndex = measurementStart;
                 sampleIndex < end;
                 sampleIndex++)
            {
                var byteOffset = checked((int)sampleIndex * 2);
                var value = (short)(
                    payload[byteOffset] | (payload[byteOffset + 1] << 8));
                var normalized = value / 32768.0;
                if (value != 0) nonZero++;
                localPeak = Mathf.Max(localPeak, (float)Math.Abs(normalized));
                var square = normalized * normalized;
                sumSquares += square;
                var channel = channels > 0 ? sampleIndex % channels : 0;
                if (channel == 0)
                {
                    leftSquares += square;
                    leftSamples++;
                    if (value != 0)
                    {
                        if (havePriorLeft &&
                            ((priorLeft < 0 && value > 0) ||
                             (priorLeft > 0 && value < 0)))
                            leftZeroCrossings++;
                        priorLeft = value;
                        havePriorLeft = true;
                    }
                }
                else if (channel == 1)
                {
                    rightSquares += square;
                    rightSamples++;
                }
                blockSquares += square;
                blockCount++;
                if (blockCount == blockSamples)
                {
                    minimumBlockRms = Math.Min(
                        minimumBlockRms,
                        Math.Sqrt(blockSquares / blockCount));
                    blockSquares = 0;
                    blockCount = 0;
                }
            }
            if (blockCount > 0)
            {
                minimumBlockRms = Math.Min(
                    minimumBlockRms,
                    Math.Sqrt(blockSquares / blockCount));
            }
            if (minimumBlockRms == double.MaxValue) minimumBlockRms = 0;
            var sampleCount = end - measurementStart;
            var rms = sampleCount > 0 ? Math.Sqrt(sumSquares / sampleCount) : 0;
            var minimumEvidenceSamples = sampleRate * Math.Max(1, channels) / 10;
            var pass = expectation == WindowExpectation.Audible
                ? sampleCount >= minimumEvidenceSamples &&
                  localPeak >= 0.002f && rms >= 0.0005
                : sampleCount >= minimumEvidenceSamples &&
                  localPeak <= 0.0005f && rms <= 0.00005;
            return new WindowResult {
                Name = name,
                Expectation = expectation,
                StartSample = start,
                MeasurementStartSample = measurementStart,
                EndSample = end,
                NonZeroSamples = nonZero,
                Peak = localPeak,
                Rms = rms,
                LeftRms = leftSamples > 0
                    ? Math.Sqrt(leftSquares / leftSamples)
                    : 0,
                RightRms = rightSamples > 0
                    ? Math.Sqrt(rightSquares / rightSamples)
                    : 0,
                LeftZeroCrossings = leftZeroCrossings,
                MinimumBlockRms = minimumBlockRms,
                ExpectedPass = pass
            };
        }

        internal static string ResolveQualifiedCapturePath(string requested)
        {
            if (string.IsNullOrWhiteSpace(requested))
                throw new InvalidDataException("audio capture path is empty");
            var temporaryRoot = Path.GetFullPath(Path.GetTempPath())
                .TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
            var rootPrefix = temporaryRoot + Path.DirectorySeparatorChar;
            var resolved = Path.GetFullPath(requested);
            if (!resolved.StartsWith(rootPrefix, StringComparison.Ordinal))
                throw new InvalidDataException(
                    "audio capture path must remain under the system temporary directory");
            var parent = Path.GetDirectoryName(resolved);
            if (string.IsNullOrEmpty(parent) || !Directory.Exists(parent))
                throw new InvalidDataException(
                    "audio capture parent must already exist");
            RejectReparsePoint(temporaryRoot);
            var relativeParent = parent.Substring(temporaryRoot.Length)
                .TrimStart(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
            var current = temporaryRoot;
            foreach (var segment in relativeParent.Split(
                         new[] {
                             Path.DirectorySeparatorChar,
                             Path.AltDirectorySeparatorChar
                         },
                         StringSplitOptions.RemoveEmptyEntries))
            {
                current = Path.Combine(current, segment);
                RejectReparsePoint(current);
            }
            if (Directory.Exists(resolved))
                throw new InvalidDataException("audio capture target is a directory");
            if (File.Exists(resolved)) RejectReparsePoint(resolved);
            return resolved;
        }

        private static void RejectReparsePoint(string path)
        {
            if ((File.GetAttributes(path) & FileAttributes.ReparsePoint) != 0)
                throw new InvalidDataException(
                    $"audio capture path contains a symlink/reparse point: {path}");
        }

        private void OnDestroy()
        {
            Finish();
        }
    }
}
