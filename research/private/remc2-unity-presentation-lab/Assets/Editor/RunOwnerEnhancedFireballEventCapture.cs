#if UNITY_EDITOR
using System;
using System.IO;
using System.Security.Cryptography;
using System.Reflection;
using UnityEditor;
using UnityEngine;

namespace MagicCarpet.PresentationLab.Editor
{
    /// <summary>
    /// Captures the first live Unity Enhanced frame containing the owner route's
    /// Fireball event. This exists only because the broad seven-route capture
    /// intentionally saves after transient effects have expired; it does not
    /// inject gameplay state or embed owner payloads.
    /// </summary>
    public static class RunOwnerEnhancedFireballEventCapture
    {
        private const int CaptureWidth = 1280;
        private const int CaptureHeight = 720;
        private const int RouteFrameLimit = 12;
        private const int ImpactRouteFrameLimit = 32;

        [MenuItem("Magic Carpet/Presentation Lab/Run Owner Enhanced Fireball Event Capture")]
        public static void RunOwnerUnityEnhancedFireballEventCapture()
        {
            string packagePath = CommandLineValue("-mc-package");
            string closurePath = CommandLineValue("-mc-closure");
            string assetCachePath = CommandLineValue("-mc-asset-cache");
            string captureDirectory = CommandLineValue("-mc-capture-dir");
            if (string.IsNullOrWhiteSpace(packagePath) ||
                string.IsNullOrWhiteSpace(closurePath) ||
                string.IsNullOrWhiteSpace(assetCachePath) ||
                string.IsNullOrWhiteSpace(captureDirectory))
            {
                throw new InvalidOperationException(
                    "owner Unity Enhanced Fireball event capture requires -mc-package, -mc-closure, -mc-asset-cache, and -mc-capture-dir");
            }

            captureDirectory = Path.GetFullPath(captureDirectory);
            Directory.CreateDirectory(captureDirectory);
            bool captureImpact = HasCommandLineFlag("-mc-await-impact");
            string captureName = captureImpact
                ? "l1-night-fireball-impact"
                : "l1-night-fireball-event";
            string outputPath = Path.Combine(captureDirectory, captureName + ".png");
            string receiptPath = Path.Combine(
                captureDirectory,
                "unity-enhanced-owner-fireball-" +
                (captureImpact ? "impact" : "event") +
                "-capture-receipt.json");
            if (File.Exists(outputPath) || File.Exists(receiptPath))
            {
                throw new InvalidOperationException(
                    "refusing to overwrite an existing owner Fireball event capture in " + captureDirectory);
            }

            GameObject root = new GameObject("Owner Unity Enhanced Fireball Event Capture Router");
            PresentationModeRouter router = root.AddComponent<PresentationModeRouter>();
            MethodInfo awake = typeof(PresentationModeRouter).GetMethod(
                "Awake",
                BindingFlags.Instance | BindingFlags.NonPublic);
            if (awake == null)
            {
                UnityEngine.Object.DestroyImmediate(root);
                throw new InvalidOperationException("presentation router Awake method is missing");
            }

            try
            {
                awake.Invoke(router, null);
                if (!router.SessionIsOpen ||
                    router.EnhancedRoot == null ||
                    router.EnhancedOwnerSession == null)
                {
                    FailAndExit(
                        "owner route did not open" +
                        " status=" + router.StatusLine +
                        " owner_validation=" +
                        (router.OwnerValidation == null ? "null" : router.OwnerValidation.Reason));
                    return;
                }

                UnityEnhancedWorldRoot enhanced = router.EnhancedRoot;
                UnityEnhancedOwnerSessionSource source = router.EnhancedOwnerSession;
                string reason = string.Empty;
                bool castSent = false;
                bool captured = false;
                int captureFrame = -1;
                byte[] pngBytes = null;
                ulong resolvedHash = 0;
                UnityEnhancedFireballViewportEvidence fireballEvidence =
                    default(UnityEnhancedFireballViewportEvidence);

                int routeFrameLimit = captureImpact ? ImpactRouteFrameLimit : RouteFrameLimit;
                for (int frame = 0; frame < routeFrameLimit; frame++)
                {
                    // This is the same owner-facing command path as normal
                    // gameplay: select the spell one tick before the cast.
                    bool selectSpell = frame == 3;
                    bool cast = frame == 4;
                    int spellSelection = selectSpell ? 0 : -1;
                    if (!router.CaptureEnhancedFrame(0f, 0f, cast, spellSelection, out reason))
                    {
                        FailAndExit("owner Fireball route frame " + frame + ": " + reason);
                        return;
                    }

                    castSent |= cast;
                    if (captureImpact)
                    {
                        // Wait for the same owner session to produce the
                        // authoritative collision row and its mapped Impact
                        // event. No synthetic event or position is injected.
                        if (!castSent || source.ActiveImpactCount <= 0 ||
                            source.LastVfxEventCount <= 0 ||
                            enhanced.VfxPresentation.ActiveEffectCount <= 0)
                        {
                            continue;
                        }
                    }
                    else
                    {
                        if (!castSent || source.ActiveProjectileCount <= 0)
                        {
                            continue;
                        }

                        // VFX events are only a spawn signal. Capture only after
                        // the authoritative Fire-family projectile renderer itself
                        // is active and projects into the capture viewport.
                        UnityEnhancedFireballViewportEvidence candidate;
                        if (!enhanced.TryGetFireballViewportEvidence(
                                CaptureWidth,
                                CaptureHeight,
                                out candidate))
                        {
                            continue;
                        }

                        fireballEvidence = candidate;
                        if (!candidate.CaptureReady)
                        {
                            continue;
                        }
                    }

                    if (!enhanced.CapturePresentationFramePng(
                            CaptureWidth,
                            CaptureHeight,
                            out pngBytes,
                            out resolvedHash,
                            out reason))
                    {
                        FailAndExit(
                            "resolved Fireball " +
                            (captureImpact ? "impact" : "event") +
                            " capture at frame " + frame + ": " + reason);
                        return;
                    }

                    captureFrame = frame;
                    captured = true;
                    break;
                }

                if (!captured || pngBytes == null || pngBytes.Length == 0)
                {
                    FailAndExit(
                        "owner Fireball route produced no live " +
                        (captureImpact ? "impact" : "event") +
                        " frame" +
                        " cast_sent=" + castSent +
                        " last_event_count=" + source.LastEventCount +
                        " last_vfx_event_count=" + source.LastVfxEventCount +
                        " active_projectiles=" + source.ActiveProjectileCount +
                        " active_impacts=" + source.ActiveImpactCount +
                        " active_effects=" + enhanced.VfxPresentation.ActiveEffectCount +
                        " fireball_renderer_found=" + fireballEvidence.RendererFound +
                        " fireball_renderer_active=" + fireballEvidence.RendererActive +
                        " fireball_center_in_front=" + fireballEvidence.CenterInFront +
                        " fireball_center_in_viewport=" + fireballEvidence.CenterInViewport +
                        " fireball_viewport_center=" + fireballEvidence.ViewportCenter);
                    return;
                }

                string fileSha256 = Sha256Hex(pngBytes);
                File.WriteAllBytes(outputPath, pngBytes);
                var receipt = new UnityEnhancedFireballEventCaptureReceipt
                {
                    schema_version = 1,
                    mode = "unity-enhanced",
                    profile = "level1-night-unity-enhanced",
                    route_id = captureImpact ? "fireball-impact" : "fireball-fight",
                    capture_name = captureName,
                    capture_status = "captured-for-human-review",
                    visual_review_status = "not-reviewed",
                    owner_payload_embedded = false,
                    owner_inputs_external = true,
                    unity_editor = Application.unityVersion,
                    graphics_api = SystemInfo.graphicsDeviceType.ToString(),
                    graphics_device = SystemInfo.graphicsDeviceName,
                    graphics_driver = SystemInfo.graphicsDeviceVersion,
                    frame_count = captureFrame + 1,
                    capture_frame = captureFrame,
                    cast_observed = castSent,
                    owner_event_count = source.LastEventCount,
                    owner_vfx_event_count = source.LastVfxEventCount,
                    active_projectile_count = source.ActiveProjectileCount,
                    active_impact_count = source.ActiveImpactCount,
                    active_effect_count = enhanced.VfxPresentation.ActiveEffectCount,
                    active_entity_count = source.ActiveEntityCount,
                    active_visual_count = enhanced.ActorPresentation.ActiveVisualCount,
                    fireball_renderer_found = fireballEvidence.RendererFound,
                    fireball_renderer_active = fireballEvidence.RendererActive,
                    fireball_capture_ready = fireballEvidence.CaptureReady,
                    fireball_center_in_front = fireballEvidence.CenterInFront,
                    fireball_center_in_viewport = fireballEvidence.CenterInViewport,
                    fireball_world_center = fireballEvidence.WorldCenter,
                    fireball_viewport_center = fireballEvidence.ViewportCenter,
                    fireball_screen_center = fireballEvidence.ScreenCenter,
                    source_generation = source.Generation.ToString(),
                    source_tick = source.Tick.ToString(),
                    png_file = Path.GetFileName(outputPath),
                    png_sha256 = fileSha256,
                    resolved_png_hash = resolvedHash.ToString("x16")
                };
                File.WriteAllText(receiptPath, JsonUtility.ToJson(receipt, true) + "\n");

                Debug.Log(
                    "OWNER UNITY ENHANCED FIREBALL EVENT CAPTURE PASS: frame=" + captureFrame +
                        " tick=" + source.Tick +
                        " events=" + source.LastEventCount +
                        " vfx_events=" + source.LastVfxEventCount +
                        " active_effects=" + enhanced.VfxPresentation.ActiveEffectCount +
                        " fireball_renderer_active=" + fireballEvidence.RendererActive +
                        " fireball_capture_ready=" + fireballEvidence.CaptureReady +
                        " fireball_viewport_center=" + fireballEvidence.ViewportCenter +
                        " fireball_screen_center=" + fireballEvidence.ScreenCenter +
                        " output_directory=" + captureDirectory +
                    " visual_review_status=not-reviewed" +
                    " owner_inputs_external=true" +
                    " png_sha256=" + fileSha256);

                source.Dispose();
                enhanced.Close();
                if (enhanced.gameObject != root)
                {
                    UnityEngine.Object.DestroyImmediate(enhanced.gameObject);
                }

                UnityEngine.Object.DestroyImmediate(root);
                root = null;
                EditorApplication.Exit(0);
            }
            finally
            {
                if (root != null)
                {
                    UnityEngine.Object.DestroyImmediate(root);
                }
            }
        }

        private static void FailAndExit(string message)
        {
            Debug.LogError("OWNER UNITY ENHANCED FIREBALL EVENT CAPTURE FAIL: " + message);
            EditorApplication.Exit(1);
        }

        private static string Sha256Hex(byte[] bytes)
        {
            using (SHA256 sha256 = SHA256.Create())
            {
                byte[] digest = sha256.ComputeHash(bytes ?? Array.Empty<byte>());
                return BitConverter.ToString(digest).Replace("-", string.Empty).ToLowerInvariant();
            }
        }

        private static string CommandLineValue(string name)
        {
            string[] arguments = Environment.GetCommandLineArgs();
            for (int index = 0; index < arguments.Length; index++)
            {
                if (string.Equals(arguments[index], name, StringComparison.OrdinalIgnoreCase))
                {
                    return index + 1 < arguments.Length ? arguments[index + 1] : string.Empty;
                }

                string prefix = name + "=";
                if (arguments[index].StartsWith(prefix, StringComparison.OrdinalIgnoreCase))
                {
                    return arguments[index].Substring(prefix.Length);
                }
            }

            return string.Empty;
        }

        private static bool HasCommandLineFlag(string name)
        {
            string[] arguments = Environment.GetCommandLineArgs();
            for (int index = 0; index < arguments.Length; index++)
            {
                if (string.Equals(arguments[index], name, StringComparison.OrdinalIgnoreCase))
                {
                    return true;
                }
            }

            return false;
        }

        [Serializable]
        private sealed class UnityEnhancedFireballEventCaptureReceipt
        {
            public int schema_version;
            public string mode;
            public string profile;
            public string route_id;
            public string capture_name;
            public string capture_status;
            public string visual_review_status;
            public bool owner_payload_embedded;
            public bool owner_inputs_external;
            public string unity_editor;
            public string graphics_api;
            public string graphics_device;
            public string graphics_driver;
            public int frame_count;
            public int capture_frame;
            public bool cast_observed;
            public int owner_event_count;
            public int owner_vfx_event_count;
            public int active_projectile_count;
            public int active_impact_count;
            public int active_effect_count;
            public int active_entity_count;
            public int active_visual_count;
            public bool fireball_renderer_found;
            public bool fireball_renderer_active;
            public bool fireball_capture_ready;
            public bool fireball_center_in_front;
            public bool fireball_center_in_viewport;
            public Vector3 fireball_world_center;
            public Vector3 fireball_viewport_center;
            public Vector3 fireball_screen_center;
            public string source_generation;
            public string source_tick;
            public string png_file;
            public string png_sha256;
            public string resolved_png_hash;
        }
    }
}
#endif
