using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Linq;
using Unity.Profiling;
using UnityEngine;
using Debug = UnityEngine.Debug;

namespace MagicCarpet.Native
{
    // Optional observer shared by Classic, Modern and voxel presentation. It
    // reads already completed DTOs, never makes a native call or advances play.
    internal sealed class RuntimeBenchmark : MonoBehaviour
    {
        [Serializable] private sealed class Sample
        {
            public int frame;
            public ulong tick, gpuTimestamp;
            public double wallSeconds, frameMs, gpuMs;
            public long draws, triangles;
        }
        [Serializable] private sealed class Report
        {
            public string schema = "mc-runtime-render-benchmark/v1";
            public string status, error, renderer, gpu, graphicsApi, resolution;
            public bool captureReadback, gpuTimingAvailable;
            public int targetFps, vsync, frames;
            public ulong firstTick, lastTick, sampleFirstTick, sampleLastTick;
            public double wallSeconds, measuredSeconds, renderFps, ticksPerSecond, p50Ms, p95Ms, p99Ms, maxMs, initialMaxMs;
            public Sample[] samples;
        }
        private static readonly string[] Arguments = Environment.GetCommandLineArgs();
        private readonly List<Sample> samples = new();
        private readonly FrameTiming[] timing = new FrameTiming[1];
        private ProfilerRecorder draws, triangles;
        private MagicCarpetRuntime runtime;
        private string output, picture;
        private ulong firstTick, lastTick;
        private int ticks = 360;
        private long started, previous;
        private bool ready, completed, pictured, ownsOutput;

        private static string Option(string key)
        {
            int i = Array.IndexOf(Arguments, key);
            if (i < 0) return null;
            if (i + 1 == Arguments.Length) throw new ArgumentException(key + " requires a value");
            return Arguments[i + 1];
        }
        [RuntimeInitializeOnLoadMethod(RuntimeInitializeLoadType.AfterSceneLoad)]
        private static void Bootstrap()
        {
            if (Array.IndexOf(Arguments, "-mc-benchmark") >= 0)
                new GameObject("Runtime performance observer").AddComponent<RuntimeBenchmark>();
        }
        private void Awake()
        {
            try
            {
                output = Option("-mc-benchmark");
                if (Directory.Exists(output) || File.Exists(output)) throw new IOException("Benchmark output must be new");
                Directory.CreateDirectory(output);
                ownsOutput = true;
                picture = Option("-mc-benchmark-picture");
                if (picture != null && File.Exists(picture)) throw new IOException("Benchmark picture must be new");
                var count = Option("-mc-benchmark-ticks");
                if (count != null) ticks = int.Parse(count, CultureInfo.InvariantCulture);
                if (ticks < 90 || ticks > 3600) throw new ArgumentOutOfRangeException(nameof(ticks));
                draws = ProfilerRecorder.StartNew(ProfilerCategory.Render, "Draw Calls Count", 1);
                triangles = ProfilerRecorder.StartNew(ProfilerCategory.Render, "Triangles Count", 1);
                started = previous = Stopwatch.GetTimestamp();
            }
            catch (Exception exception) { Fail(exception.Message); }
        }
        private void Update()
        {
            if (completed) return;
            long now = Stopwatch.GetTimestamp();
            double elapsed = (now - started) / (double)Stopwatch.Frequency;
            if (runtime == null) runtime = FindFirstObjectByType<MagicCarpetRuntime>();
            if (runtime != null && !string.IsNullOrEmpty(runtime.QualificationFatalError)) { Fail(runtime.QualificationFatalError); return; }
            var snapshot = runtime?.GameRuntimeProductionCampaignRenderObservation;
            if (snapshot?.identity == null || snapshot.value == null)
            {
                if (elapsed > 120) Fail("Runtime never produced a campaign observation");
                return;
            }
            if (!ulong.TryParse(snapshot.identity.tick, NumberStyles.None, CultureInfo.InvariantCulture, out var tick)) { Fail("Invalid completed tick"); return; }
            if (!ready)
            {
                ready = true; firstTick = lastTick = tick;
                started = previous = now;
                QualitySettings.vSyncCount = 0; Application.targetFrameRate = -1;
                ModernCapture.BeginWorldMeasurement();
                Debug.Log($"MC_RUNTIME_BENCHMARK_READY tick={tick} gpu={SystemInfo.graphicsDeviceName} resolution={Screen.width}x{Screen.height}");
                return;
            }
            if (tick < lastTick) { Fail("Campaign tick reset during benchmark"); return; }
            elapsed = (now - started) / (double)Stopwatch.Frequency;
            FrameTimingManager.CaptureFrameTimings();
            var latest = FrameTimingManager.GetLatestTimings(1, timing) > 0 ? timing[0] : default;
            if (samples.Count >= 120000) { Fail("Benchmark sample limit exceeded"); return; }
            samples.Add(new Sample { frame = Time.frameCount, tick = tick, wallSeconds = elapsed,
                frameMs = (now - previous) * 1000.0 / Stopwatch.Frequency,
                gpuTimestamp = latest.frameStartTimestamp, gpuMs = latest.gpuFrameTime,
                draws = draws.Valid ? draws.LastValue : -1, triangles = triangles.Valid ? triangles.LastValue : -1 });
            previous = now; lastTick = tick;
            if (picture != null && !pictured && tick >= firstTick + 60)
            {
                pictured = true; ScreenCapture.CaptureScreenshot(picture);
            }
            if (tick >= firstTick + (ulong)ticks) Finish("complete", null);
            else if (elapsed > 120) Fail("Campaign did not advance requested ticks within 120 seconds");
        }
        private void Fail(string error) { Debug.LogError("MC_RUNTIME_BENCHMARK_FAILED " + error); Finish("error", error); }
        private void Finish(string status, string error)
        {
            if (completed) return;
            completed = true;
            var warm = samples.Where(sample => sample.tick >= firstTick + 30).ToArray();
            var durations = warm.Select(sample => sample.frameMs).OrderBy(value => value).ToArray();
            double P(double p) => durations.Length == 0 ? 0 : durations[(int)((durations.Length - 1) * p)];
            double seconds = warm.Length > 1 ? warm[^1].wallSeconds - warm[0].wallSeconds : 0;
            var report = new Report { status = status, error = error,
                renderer = Arguments.Contains("-mc-voxel-modern") ? "Modern voxel" : Arguments.Contains("-mc-modern") ? "Modern" : "Rust original-style",
                gpu = SystemInfo.graphicsDeviceName, graphicsApi = SystemInfo.graphicsDeviceType.ToString(), resolution = $"{Screen.width}x{Screen.height}",
                targetFps = Application.targetFrameRate, vsync = QualitySettings.vSyncCount, captureReadback = picture != null,
                firstTick = firstTick, lastTick = lastTick, frames = warm.Length,
                sampleFirstTick = warm.Length == 0 ? 0 : warm[0].tick, sampleLastTick = warm.Length == 0 ? 0 : warm[^1].tick,
                wallSeconds = (Stopwatch.GetTimestamp() - started) / (double)Stopwatch.Frequency,
                measuredSeconds = seconds, renderFps = seconds > 0 ? (warm.Length - 1) / seconds : 0,
                ticksPerSecond = seconds > 0 ? (warm[^1].tick - warm[0].tick) / seconds : 0,
                p50Ms = P(.5), p95Ms = P(.95), p99Ms = P(.99), maxMs = P(1),
                initialMaxMs = samples.Count == 0 ? 0 : samples.Max(sample => sample.frameMs),
                gpuTimingAvailable = samples.Any(sample => sample.gpuTimestamp != 0 && sample.gpuMs > 0), samples = samples.ToArray() };
            if (ownsOutput)
            {
                File.WriteAllText(Path.Combine(output, "report.json"), JsonUtility.ToJson(report, true));
                ModernCapture.WriteWorldProfile(output);
            }
            Debug.Log($"MC_RUNTIME_BENCHMARK_END status={status} fps={report.renderFps:F2} ticks_per_second={report.ticksPerSecond:F2}");
            Application.Quit(status == "complete" ? 0 : 3);
        }
        private void OnDestroy() { draws.Dispose(); triangles.Dispose(); }
    }
}
