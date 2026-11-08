using System;
using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using UnityEngine;
using UnityEngine.Rendering.Universal;
using Unity.Profiling;

namespace MagicCarpet.Native
{
    internal sealed class ModernCapture : MonoBehaviour
    {
        // Process arguments never change. Reading them inside every actor draw
        // repeatedly marshals and allocates the complete argument array in Mono.
        private static readonly string[] Arguments = Environment.GetCommandLineArgs();
        internal static readonly bool HideHud = Arguments.Contains("-mc-modern-hide-hud");
        internal static readonly bool Measure = Arguments.Contains("-mc-modern-metrics");
        internal static readonly bool VisualDebug = Arguments.Contains("-mc-modern-visual-debug");
        internal static readonly bool ProceduralClouds = Arguments.Contains("-mc-modern-procedural-clouds");
        internal static readonly bool FullTerrainMaterial = Arguments.Contains("-mc-modern-full-terrain-material");
        internal static int EntityVisibility { get; private set; }
        internal static bool IsolateSmoke { get; private set; }
        internal static int SmokeDebug { get; private set; }
        private int vegetationDebug;
        internal static Vector3? ReviewPosition { get; private set; }
        internal static Quaternion ReviewRotation { get; private set; }
        private readonly List<float> frameTimes = new();
        private float started;
        private bool reported;
        private static readonly Dictionary<string, List<double>> scopeTimes = new();
        private static readonly Dictionary<string, long> identityReadCounts = new();
        private static float measurementReadyAt = float.PositiveInfinity;
        private readonly FrameTiming[] timings = new FrameTiming[1];
        private ulong lastTimingTimestamp;
        private Vector3? orbitCenter;
        private float orbitRadius, orbitHeight, orbitSpeed;
        private readonly List<ProfilerRecorder> counters = new();
        private static readonly string[] CounterNames = { "Draw Calls Count", "Batches Count", "SetPass Calls Count", "Triangles Count" };
        [Serializable] private sealed class CounterSample { public string name; public bool available; public long value; }
        [Serializable] private sealed class ScopeSample { public string name; public double[] milliseconds; }
        [Serializable] private sealed class IdentityReads { public string caller; public long count; }
        [Serializable] private sealed class ProfileFrame { public int frame, bodies; public ulong tick, gpuFrameTimestamp; public float wallSeconds, gameSeconds; public double milliseconds, gpuMilliseconds; public bool gpuAvailable; public CounterSample[] counters; }
        [Serializable] private sealed class Profile { public string mode = "unthrottled; counters refer to last completed frame"; public ProfileFrame[] frames; public ScopeSample[] scopes; public IdentityReads[] identityReads; }
        private static readonly List<ProfileFrame> profileFrames = new();
        private static int measurementEpoch;

        // Campaign startup can take longer than the old Awake+5 second window.
        // Start the voxel's common scope window only after its world is ready.
        internal static void BeginWorldMeasurement()
        {
            if (!Measure) return;
            measurementEpoch++;
            scopeTimes.Clear(); profileFrames.Clear();
            lock (identityReadCounts) identityReadCounts.Clear();
            measurementReadyAt = Time.realtimeSinceStartup + 1f;
            Debug.Log("MC_MODERN_MEASUREMENT_WORLD_READY epoch=" + measurementEpoch);
        }

        internal static void WriteWorldProfile(string directory)
        {
            if (!Measure) return;
            IdentityReads[] identities;
            lock (identityReadCounts) identities = identityReadCounts.Select(pair => new IdentityReads {
                caller = pair.Key, count = pair.Value
            }).ToArray();
            File.WriteAllText(Path.Combine(directory, "performance.json"), JsonUtility.ToJson(new Profile {
                frames = profileFrames.ToArray(), scopes = scopeTimes.Select(pair => new ScopeSample { name = pair.Key, milliseconds = pair.Value.ToArray() }).ToArray(),
                identityReads = identities
            }, true));
        }

        internal static void CountIdentityRead(string caller)
        {
            if (!Measure || Time.realtimeSinceStartup < measurementReadyAt) return;
            caller ??= "unknown";
            lock (identityReadCounts) {
                identityReadCounts.TryGetValue(caller, out var count);
                identityReadCounts[caller] = count + 1;
            }
        }

        internal void SampleWorldFrame(ulong tick, int bodies, double milliseconds, float gameSeconds, float wallSeconds)
        {
            if (!Measure || profileFrames.Count >= 12000) return;
            var latest = FrameTimingManager.GetLatestTimings(1, timings) > 0 ? timings[0] : default;
            profileFrames.Add(new ProfileFrame { frame = Time.frameCount, tick = tick, bodies = bodies,
                milliseconds = milliseconds, gameSeconds = gameSeconds, wallSeconds = wallSeconds,
                gpuMilliseconds = latest.gpuFrameTime, gpuFrameTimestamp = latest.frameStartTimestamp, gpuAvailable = latest.gpuFrameTime > 0,
                counters = counters.Select((recorder, index) => new CounterSample { name = CounterNames[index], available = recorder.Valid, value = recorder.Valid ? recorder.LastValue : -1 }).ToArray() });
        }

        internal static long BeginMeasurement() => Measure && Time.realtimeSinceStartup >= measurementReadyAt
            ? System.Diagnostics.Stopwatch.GetTimestamp() : 0;

        internal static void EndMeasurement(string scope, long start)
        {
            if (start == 0) return;
            RecordTiming(scope, (System.Diagnostics.Stopwatch.GetTimestamp() - start) * 1000.0 / System.Diagnostics.Stopwatch.Frequency);
        }

        private static void RecordTiming(string scope, double milliseconds)
        {
            if (!scopeTimes.TryGetValue(scope, out var samples))
                scopeTimes.Add(scope, samples = new List<double>(120));
            if (samples.Count >= (Voxels.VoxelModernWorld.Requested ? 12000 : 120)) return;
            samples.Add(milliseconds);
            if (samples.Count != 120) return;
            var sorted = samples.OrderBy(value => value).ToArray();
            Debug.Log($"MC_MODERN_SCOPE scope={scope} samples=120 mean_ms={samples.Average():F2} median_ms={sorted[60]:F2} p95_ms={sorted[113]:F2}");
        }

        private void Awake()
        {
            // Explicit profiling control; normal play keeps the user's quality settings.
            if (Arguments.Contains("-mc-modern-no-vsync")) QualitySettings.vSyncCount = 0;
            if (Measure)
                Debug.Log($"MC_MODERN_DEVICE quality={QualitySettings.names[QualitySettings.GetQualityLevel()]} vsync={QualitySettings.vSyncCount} target_fps={Application.targetFrameRate} graphics={SystemInfo.graphicsDeviceType} gpu={SystemInfo.graphicsDeviceName} frame_timings={FrameTimingManager.IsFeatureEnabled()}");
            started = Time.realtimeSinceStartup;
            measurementReadyAt = started + 5f;
            var warmupOption = Array.IndexOf(Arguments, "-mc-modern-warmup");
            if (warmupOption >= 0 && warmupOption + 1 < Arguments.Length)
            {
                var warmup = float.Parse(Arguments[warmupOption + 1], CultureInfo.InvariantCulture);
                if (float.IsNaN(warmup) || float.IsInfinity(warmup) || warmup < 5f || warmup > 120f)
                    throw new ArgumentException("Modern measurement warmup must be between 5 and 120 seconds");
                measurementReadyAt = started + warmup;
            }
            EntityVisibility = 0;
            IsolateSmoke = false;
            SmokeDebug = 0;
            Shader.SetGlobalFloat("_ModernSmokeDebug", 0);
            Shader.SetGlobalFloat("_ModernVegetationDebug", 0);
            scopeTimes.Clear();
            profileFrames.Clear();
            if (Measure) foreach (var name in CounterNames) counters.Add(ProfilerRecorder.StartNew(ProfilerCategory.Render, name, 1));
            ReviewPosition = null;
            var args = Environment.GetCommandLineArgs();
            var review = Array.IndexOf(args, "-mc-modern-review-camera");
            if (review >= 0)
            {
                if (review + 1 >= args.Length) throw new ArgumentException("Review camera requires x,y,z,yaw,pitch");
                var values = args[review + 1].Split(',').Select(value => float.Parse(value, CultureInfo.InvariantCulture)).ToArray();
                if (values.Length != 5 || values.Any(value => float.IsNaN(value) || float.IsInfinity(value)))
                    throw new ArgumentException("Review camera requires five finite coordinates/angles");
                ReviewPosition = new Vector3(values[0], values[1], values[2]);
                ReviewRotation = Quaternion.Euler(values[4], values[3], 0);
                Debug.Log($"MC_MODERN_REVIEW_CAMERA presentation_only=True position={ReviewPosition} rotation={ReviewRotation.eulerAngles}");
            }
            var orbit = Array.IndexOf(args, "-mc-modern-review-orbit");
            if (orbit >= 0)
            {
                if (orbit + 1 >= args.Length) throw new ArgumentException("Review orbit requires center-x,y,z,radius,height,degrees-per-second");
                var values = args[orbit + 1].Split(',').Select(value => float.Parse(value, CultureInfo.InvariantCulture)).ToArray();
                if (values.Length != 6 || values.Any(value => float.IsNaN(value) || float.IsInfinity(value)) || values[3] <= 0)
                    throw new ArgumentException("Review orbit requires six finite values and positive radius");
                orbitCenter = new Vector3(values[0], values[1], values[2]);
                orbitRadius = values[3]; orbitHeight = values[4]; orbitSpeed = values[5];
                Debug.Log("MC_MODERN_REVIEW_ORBIT presentation_only=True readback_captures_are_not_performance_measurements=True");
            }
        }

        private void OnDestroy()
        {
            foreach (var counter in counters) counter.Dispose();
        }

        private void Update()
        {
            if (orbitCenter.HasValue)
            {
                var angle = (Time.realtimeSinceStartup - started) * orbitSpeed * Mathf.Deg2Rad;
                ReviewPosition = orbitCenter.Value + new Vector3(Mathf.Cos(angle) * orbitRadius, orbitHeight, Mathf.Sin(angle) * orbitRadius);
                ReviewRotation = Quaternion.LookRotation(orbitCenter.Value - ReviewPosition.Value);
            }
            if (Measure && (!reported || Voxels.VoxelModernWorld.Requested))
            {
                FrameTimingManager.CaptureFrameTimings();
                if (Time.realtimeSinceStartup >= measurementReadyAt &&
                    FrameTimingManager.GetLatestTimings(1, timings) > 0 &&
                    timings[0].frameStartTimestamp != lastTimingTimestamp)
                {
                    var timing = timings[0];
                    lastTimingTimestamp = timing.frameStartTimestamp;
                    RecordTiming("cpu_frame", timing.cpuFrameTime);
                    RecordTiming("cpu_main", timing.cpuMainThreadFrameTime);
                    RecordTiming("cpu_render", timing.cpuRenderThreadFrameTime);
                    RecordTiming("cpu_present_wait", timing.cpuMainThreadPresentWaitTime);
                    if (timing.gpuFrameTime > 0) RecordTiming("gpu_frame", timing.gpuFrameTime);
                }
            }
            if (VisualDebug)
            {
                var capture = false;
                if (Input.GetKeyDown(KeyCode.F4))
                { vegetationDebug = (vegetationDebug + 1) % 4; Shader.SetGlobalFloat("_ModernVegetationDebug", vegetationDebug); capture = true; }
                if (Input.GetKeyDown(KeyCode.F6)) { EntityVisibility = (EntityVisibility + 1) % 3; capture = true; }
                if (Input.GetKeyDown(KeyCode.F7) && RenderSettings.sun != null)
                { RenderSettings.sun.shadows = RenderSettings.sun.shadows == LightShadows.None ? LightShadows.Soft : LightShadows.None; capture = true; }
                if (Input.GetKeyDown(KeyCode.F8))
                { var data = GetComponent<Camera>().GetUniversalAdditionalCameraData(); data.renderPostProcessing = !data.renderPostProcessing; capture = true; }
                if (Input.GetKeyDown(KeyCode.F12))
                { SmokeDebug = (SmokeDebug + 1) % 5; Shader.SetGlobalFloat("_ModernSmokeDebug", SmokeDebug); capture = true; }
                if (Input.GetKeyDown(KeyCode.Insert)) { IsolateSmoke = !IsolateSmoke; capture = true; }
                if (capture || Input.GetKeyDown(KeyCode.F9)) StartCoroutine(CaptureDiagnostic());
            }
            if (!Measure || reported || Time.realtimeSinceStartup < measurementReadyAt) return;
            frameTimes.Add(Time.unscaledDeltaTime * 1000f);
            if (frameTimes.Count < 120) return;
            frameTimes.Sort();
            Debug.Log($"MC_MODERN_FRAME_TIMES samples={frameTimes.Count} mean_ms={frameTimes.Average():F2} median_ms={frameTimes[60]:F2} p95_ms={frameTimes[113]:F2} width={Screen.width} height={Screen.height}");
            reported = true;
        }

        private IEnumerator CaptureDiagnostic()
        {
            yield return new WaitForEndOfFrame();
            var args = Environment.GetCommandLineArgs();
            var option = Array.IndexOf(args, "-mc-modern-capture");
            if (option < 0 || option + 1 >= args.Length) yield break;
            var original = Path.GetFullPath(args[option + 1]);
            var path = Path.Combine(Path.GetDirectoryName(original), Path.GetFileNameWithoutExtension(original) + $"-debug-{Time.frameCount}.png");
            ScreenCapture.CaptureScreenshot(path);
            Debug.Log($"MC_MODERN_DIAGNOSTIC path={path} entities={EntityVisibility} smoke_debug={SmokeDebug} vegetation_debug={vegetationDebug} isolate_smoke={IsolateSmoke} shadows={RenderSettings.sun?.shadows} post={GetComponent<Camera>().GetUniversalAdditionalCameraData().renderPostProcessing} position={transform.position} rotation={transform.eulerAngles}");
        }

        private IEnumerator Start()
        {
            var args = Environment.GetCommandLineArgs();
            var option = Array.IndexOf(args, "-mc-modern-capture");
            if (option < 0 || option + 1 >= args.Length) yield break;
            var path = Path.GetFullPath(args[option + 1]);
            var delayOption = Array.IndexOf(args, "-mc-modern-capture-delay");
            var delay = delayOption >= 0 && delayOption + 1 < args.Length
                ? float.Parse(args[delayOption + 1], CultureInfo.InvariantCulture) : 10f;
            var countOption = Array.IndexOf(args, "-mc-modern-capture-count");
            var count = countOption >= 0 && countOption + 1 < args.Length
                ? int.Parse(args[countOption + 1], CultureInfo.InvariantCulture) : 1;
            if (count < 1 || count > 120) throw new ArgumentException("Capture count must be between 1 and 120");
            var intervalOption = Array.IndexOf(args, "-mc-modern-capture-interval");
            var interval = intervalOption >= 0 && intervalOption + 1 < args.Length
                ? float.Parse(args[intervalOption + 1], CultureInfo.InvariantCulture) : .25f;
            if (float.IsNaN(interval) || float.IsInfinity(interval) || interval < .05f || interval > 10f)
                throw new ArgumentException("Capture interval must be between .05 and 10 seconds");
            yield return new WaitForSecondsRealtime(Mathf.Max(1, delay));
            Directory.CreateDirectory(Path.GetDirectoryName(path));
            for (var frame = 0; frame < count; frame++)
            {
                yield return new WaitForEndOfFrame();
                var output = count == 1 ? path : Path.Combine(Path.GetDirectoryName(path),
                    Path.GetFileNameWithoutExtension(path) + $"-{frame:D3}" + Path.GetExtension(path));
                ScreenCapture.CaptureScreenshot(output);
                Debug.Log($"MC_MODERN_CAPTURE path={output} camera={transform.position} rotation={transform.eulerAngles} frame_ms={Time.unscaledDeltaTime * 1000:F2}");
                if (frame + 1 < count) yield return new WaitForSecondsRealtime(interval);
            }
            yield return new WaitForSecondsRealtime(2);
            if (args.Contains("-mc-modern-exit-after-capture")) Application.Quit();
        }
    }
}
