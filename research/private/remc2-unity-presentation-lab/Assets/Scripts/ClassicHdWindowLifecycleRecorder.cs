using System;
using System.Collections.Generic;
using System.IO;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.PresentationLab
{
    [Serializable]
    public sealed class ClassicHdWindowLifecycleEvent
    {
        public int sequence;
        public string observation;
        public bool actual_os_callback;
        public bool synthetic_qualification;
        public bool focused;
        public bool paused;
        public bool suspended;
        public bool callback_succeeded;
        public string callback_reason;
        public string full_screen_mode;
        public int screen_width;
        public int screen_height;
        public int internal_width;
        public int internal_height;
        public int viewport_x;
        public int viewport_y;
        public int viewport_width;
        public int viewport_height;
        public string generation;
        public string tick;
        public int upload_count;
        public int allocation_epoch;
        public int output_restore_requests;
        public int stable_rendered_frames;
        public float realtime_seconds;
    }

    [Serializable]
    public sealed class ClassicHdWindowLifecycleReceipt
    {
        public int schema_version = 1;
        public string kind = "classic-hd-owner-os-window-lifecycle/v1";
        public string mode = "classic-hd";
        public string profile;
        public string qualification_status = "open-os-window-evidence-required";
        public bool owner_payload_embedded;
        public bool owner_inputs_external = true;
        public string unity_version;
        public string platform;
        public string graphics_api;
        public string graphics_device;
        public string graphics_driver;
        public string desktop_session;
        public string window_driver;
        public string window_case;
        public int requested_window_width;
        public int requested_window_height;
        public string requested_full_screen_mode;
        public int required_consecutive_stable_frames = 2;
        public bool initial_output_stable;
        public bool post_regain_output_stable;
        public bool external_focus_transition_observed;
        public bool external_focus_return_observed;
        public int actual_focus_callback_count;
        public int actual_pause_callback_count;
        public int rendered_observation_count;
        public string failure_reason;
        public ClassicHdWindowLifecycleEvent[] events = Array.Empty<ClassicHdWindowLifecycleEvent>();
    }

    /// <summary>
    /// Qualification-only recorder for real player focus/pause callbacks.
    /// Synthetic editor lifecycle calls intentionally never enter this trace.
    /// The recorder observes presentation state but never advances Rust.
    /// </summary>
    [DisallowMultipleComponent]
    public sealed class ClassicHdWindowLifecycleRecorder : MonoBehaviour
    {
        private const int RequiredStableFrames = 2;
        private const int MaximumEvents = 256;

        private readonly List<ClassicHdWindowLifecycleEvent> events =
            new List<ClassicHdWindowLifecycleEvent>();
        private PresentationModeRouter router;
        private ClassicHdPresentation presentation;
        private ClassicHdPresentationSettings settings;
        private string receiptPath;
        private string desktopSession;
        private string windowDriver;
        private string windowCase;
        private bool configured;
        private bool flushed;
        private bool sawFocusLoss;
        private bool sawFocusReturn;
        private bool initialOutputStable;
        private bool postRegainOutputStable;
        private bool awaitingInitialStability = true;
        private bool awaitingPostRegainStability;
        private bool hasLastRenderedState;
        private int stableRenderedFrames;
        private int renderedObservationCount;
        private int actualFocusCallbackCount;
        private int actualPauseCallbackCount;
        private int lastScreenWidth;
        private int lastScreenHeight;
        private int lastInternalWidth;
        private int lastInternalHeight;
        private Rect lastViewport;
        private string lastFullScreenMode = string.Empty;
        private int lastAllocationEpoch;

        public int EventCount { get { return events.Count; } }
        public bool ExternalFocusTransitionObserved { get { return sawFocusLoss; } }
        public bool ExternalFocusReturnObserved { get { return sawFocusReturn; } }
        public bool InitialOutputStable { get { return initialOutputStable; } }
        public bool PostRegainOutputStable { get { return postRegainOutputStable; } }
        public int StableRenderedFrames { get { return stableRenderedFrames; } }
        public string ReceiptPath { get { return receiptPath ?? string.Empty; } }

        public bool Configure(
            PresentationModeRouter owner,
            ClassicHdPresentation renderer,
            ClassicHdPresentationSettings requestedSettings,
            string outputPath,
            string requestedDesktopSession,
            string requestedWindowDriver,
            string requestedWindowCase,
            out string reason)
        {
            reason = string.Empty;
            if (owner == null || renderer == null || requestedSettings == null)
            {
                reason = "Classic HD OS window recorder requires an owner router, renderer, and settings";
                return false;
            }

            if (string.IsNullOrWhiteSpace(outputPath))
            {
                reason = "Classic HD OS window recorder requires an output receipt path";
                return false;
            }

            try
            {
                receiptPath = Path.GetFullPath(outputPath);
            }
            catch (Exception exception)
            {
                reason = "Classic HD OS window recorder output path is invalid: " + exception.Message;
                return false;
            }

            router = owner;
            presentation = renderer;
            settings = requestedSettings.Clone();
            desktopSession = requestedDesktopSession ?? string.Empty;
            windowDriver = requestedWindowDriver ?? string.Empty;
            windowCase = string.IsNullOrWhiteSpace(requestedWindowCase)
                ? RequestedFullScreenMode(settings)
                : requestedWindowCase;
            events.Clear();
            flushed = false;
            configured = true;
            RecordObservation("session-open", false, false, true, string.Empty);
            return true;
        }

        /// <summary>
        /// Called only by PresentationModeRouter's real Unity callbacks.
        /// ApplyApplicationLifecycleForQualification deliberately does not call
        /// this method, keeping synthetic evidence distinguishable.
        /// </summary>
        public void RecordUnityLifecycleCallback(
            string callbackName,
            bool callbackSucceeded,
            string callbackReason)
        {
            if (!configured)
            {
                return;
            }

            if (string.Equals(callbackName, "focus", StringComparison.Ordinal))
            {
                actualFocusCallbackCount++;
                if (!router.ApplicationFocused)
                {
                    sawFocusLoss = true;
                }
            }
            else if (string.Equals(callbackName, "pause", StringComparison.Ordinal))
            {
                actualPauseCallbackCount++;
            }

            // Unity may deliver OnApplicationFocus(true) before
            // OnApplicationPause(false). Treat the pair as one real return
            // once both state flags agree, regardless of callback order.
            if (sawFocusLoss && !sawFocusReturn &&
                router.ApplicationFocused && !router.IsApplicationSuspended)
            {
                sawFocusReturn = true;
                awaitingPostRegainStability = true;
                stableRenderedFrames = 0;
                hasLastRenderedState = false;
            }

            RecordObservation(
                "unity-on-application-" + callbackName,
                true,
                false,
                callbackSucceeded,
                callbackReason);
        }

        /// <summary>
        /// Samples the real player's rendered output. Two consecutive stable
        /// frames are required because Screen.SetResolution is asynchronous.
        /// </summary>
        public void ObserveRenderedFrame()
        {
            if (!configured || presentation == null || router == null || router.IsApplicationSuspended)
            {
                return;
            }

            renderedObservationCount++;
            int screenWidth = Screen.width;
            int screenHeight = Screen.height;
            Rect viewport = presentation.Viewport;
            string fullScreenMode = Screen.fullScreenMode.ToString();
            bool sameAsPrevious = hasLastRenderedState &&
                                  screenWidth == lastScreenWidth &&
                                  screenHeight == lastScreenHeight &&
                                  presentation.InternalWidth == lastInternalWidth &&
                                  presentation.InternalHeight == lastInternalHeight &&
                                  presentation.AllocationEpoch == lastAllocationEpoch &&
                                  viewport == lastViewport &&
                                  string.Equals(fullScreenMode, lastFullScreenMode, StringComparison.Ordinal);
            stableRenderedFrames = sameAsPrevious ? stableRenderedFrames + 1 : 1;
            hasLastRenderedState = true;
            lastScreenWidth = screenWidth;
            lastScreenHeight = screenHeight;
            lastInternalWidth = presentation.InternalWidth;
            lastInternalHeight = presentation.InternalHeight;
            lastAllocationEpoch = presentation.AllocationEpoch;
            lastViewport = viewport;
            lastFullScreenMode = fullScreenMode;

            if (stableRenderedFrames < RequiredStableFrames)
            {
                return;
            }

            if (awaitingInitialStability)
            {
                awaitingInitialStability = false;
                initialOutputStable = true;
                RecordObservation("initial-output-stable", false, false, true, string.Empty);
            }

            if (awaitingPostRegainStability)
            {
                awaitingPostRegainStability = false;
                postRegainOutputStable = true;
                RecordObservation("post-regain-output-stable", false, false, true, string.Empty);
            }
        }

        public void Flush()
        {
            if (!configured || flushed || string.IsNullOrWhiteSpace(receiptPath))
            {
                return;
            }

            ClassicHdWindowLifecycleReceipt receipt = new ClassicHdWindowLifecycleReceipt
            {
                profile = settings == null ? string.Empty : settings.profile,
                unity_version = Application.unityVersion,
                platform = Application.platform.ToString(),
                graphics_api = SystemInfo.graphicsDeviceType.ToString(),
                graphics_device = SystemInfo.graphicsDeviceName ?? string.Empty,
                graphics_driver = SystemInfo.graphicsDeviceVersion ?? string.Empty,
                desktop_session = desktopSession,
                window_driver = windowDriver,
                window_case = windowCase,
                requested_window_width = settings == null ? 0 : settings.windowWidth,
                requested_window_height = settings == null ? 0 : settings.windowHeight,
                requested_full_screen_mode = settings == null
                    ? string.Empty
                    : RequestedFullScreenMode(settings),
                initial_output_stable = initialOutputStable,
                post_regain_output_stable = postRegainOutputStable,
                external_focus_transition_observed = sawFocusLoss,
                external_focus_return_observed = sawFocusReturn,
                actual_focus_callback_count = actualFocusCallbackCount,
                actual_pause_callback_count = actualPauseCallbackCount,
                rendered_observation_count = renderedObservationCount,
                events = events.ToArray()
            };

            try
            {
                string directory = Path.GetDirectoryName(receiptPath);
                if (!string.IsNullOrEmpty(directory))
                {
                    Directory.CreateDirectory(directory);
                }

                File.WriteAllText(receiptPath, JsonUtility.ToJson(receipt, true));
                flushed = true;
                Debug.Log(
                    "CLASSIC HD OS WINDOW LIFECYCLE RECEIPT: path=" + receiptPath +
                    " focus_loss=" + (sawFocusLoss ? 1 : 0) +
                    " focus_return=" + (sawFocusReturn ? 1 : 0) +
                    " initial_stable=" + (initialOutputStable ? 1 : 0) +
                    " post_regain_stable=" + (postRegainOutputStable ? 1 : 0) +
                    " events=" + events.Count);
            }
            catch (Exception exception)
            {
                Debug.LogError("CLASSIC HD OS WINDOW LIFECYCLE RECEIPT WRITE FAILED: " + exception.Message);
            }
        }

        private void RecordObservation(
            string observation,
            bool actualOsCallback,
            bool syntheticQualification,
            bool callbackSucceeded,
            string callbackReason)
        {
            if (!configured || events.Count >= MaximumEvents || router == null || presentation == null)
            {
                return;
            }

            Rect viewport = presentation.Viewport;
            events.Add(new ClassicHdWindowLifecycleEvent
            {
                sequence = events.Count,
                observation = observation ?? string.Empty,
                actual_os_callback = actualOsCallback,
                synthetic_qualification = syntheticQualification,
                focused = router.ApplicationFocused,
                paused = router.ApplicationPaused,
                suspended = router.IsApplicationSuspended,
                callback_succeeded = callbackSucceeded,
                callback_reason = callbackReason ?? string.Empty,
                full_screen_mode = Screen.fullScreenMode.ToString(),
                screen_width = Screen.width,
                screen_height = Screen.height,
                internal_width = presentation.InternalWidth,
                internal_height = presentation.InternalHeight,
                viewport_x = Mathf.RoundToInt(viewport.x),
                viewport_y = Mathf.RoundToInt(viewport.y),
                viewport_width = Mathf.RoundToInt(viewport.width),
                viewport_height = Mathf.RoundToInt(viewport.height),
                generation = presentation.AuthoritativeGeneration.ToString(),
                tick = presentation.AuthoritativeTick.ToString(),
                upload_count = presentation.UploadCount,
                allocation_epoch = presentation.AllocationEpoch,
                output_restore_requests = router.OutputRestoreRequests,
                stable_rendered_frames = stableRenderedFrames,
                realtime_seconds = Time.realtimeSinceStartup
            });
        }

        private static string RequestedFullScreenMode(ClassicHdPresentationSettings requestedSettings)
        {
            return requestedSettings != null && requestedSettings.fullscreen
                ? FullScreenMode.FullScreenWindow.ToString()
                : FullScreenMode.Windowed.ToString();
        }

        private void OnApplicationQuit()
        {
            Flush();
        }

        private void OnDestroy()
        {
            Flush();
        }
    }
}
