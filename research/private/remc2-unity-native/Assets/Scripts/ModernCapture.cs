using System;
using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using UnityEngine;
using UnityEngine.Rendering.Universal;

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
        private static float measurementReadyAt = float.PositiveInfinity;
        private readonly FrameTiming[] timings = new FrameTiming[1];
        private ulong lastTimingTimestamp;

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
            if (samples.Count >= 120) return;
            samples.Add(milliseconds);
            if (samples.Count != 120) return;
            samples.Sort();
            Debug.Log($"MC_MODERN_SCOPE scope={scope} samples=120 mean_ms={samples.Average():F2} median_ms={samples[60]:F2} p95_ms={samples[113]:F2}");
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
        }

        private void Update()
        {
            if (Measure && !reported)
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
            yield return new WaitForSecondsRealtime(Mathf.Max(1, delay));
            yield return new WaitForEndOfFrame();
            Directory.CreateDirectory(Path.GetDirectoryName(path));
            ScreenCapture.CaptureScreenshot(path);
            Debug.Log($"MC_MODERN_CAPTURE path={path} camera={transform.position} rotation={transform.eulerAngles} frame_ms={Time.unscaledDeltaTime * 1000:F2}");
            yield return new WaitForSecondsRealtime(2);
            if (args.Contains("-mc-modern-exit-after-capture")) Application.Quit();
        }
    }
}
