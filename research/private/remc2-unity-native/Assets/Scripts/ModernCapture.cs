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
        internal static bool HideHud => Environment.GetCommandLineArgs().Contains("-mc-modern-hide-hud");
        internal static bool Measure => Environment.GetCommandLineArgs().Contains("-mc-modern-metrics");
        internal static bool VisualDebug => Environment.GetCommandLineArgs().Contains("-mc-modern-visual-debug");
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

        internal static long BeginMeasurement() => Measure ? System.Diagnostics.Stopwatch.GetTimestamp() : 0;

        internal static void EndMeasurement(string scope, long start)
        {
            if (start == 0) return;
            if (!scopeTimes.TryGetValue(scope, out var samples))
                scopeTimes.Add(scope, samples = new List<double>(120));
            if (samples.Count >= 120) return;
            samples.Add((System.Diagnostics.Stopwatch.GetTimestamp() - start) * 1000.0 / System.Diagnostics.Stopwatch.Frequency);
            if (samples.Count != 120) return;
            samples.Sort();
            Debug.Log($"MC_MODERN_SCOPE scope={scope} samples=120 mean_ms={samples.Average():F2} median_ms={samples[60]:F2} p95_ms={samples[113]:F2}");
        }

        private void Awake()
        {
            started = Time.realtimeSinceStartup;
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
            if (!Measure || reported || Time.realtimeSinceStartup - started < 5f) return;
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
