using System;
using System.IO;
using UnityEngine;

namespace MagicCarpet.PresentationLab
{
    public enum PresentationModeId
    {
        Unknown = 0,
        ClassicHd = 1,
        UnityEnhanced = 2
    }

    /// <summary>
    /// Selects a presentation mode before opening a presentation session.
    /// This project is intentionally not the Exact project and does not own gameplay.
    /// </summary>
    public sealed class PresentationModeRouter : MonoBehaviour
    {
        private const string ModeArgument = "-mc-presentation-mode";
        private const string PackageArgument = "-mc-package";
        private const string ClosureArgument = "-mc-closure";
        private const string CacheArgument = "-mc-asset-cache";

        private PresentationSessionLease sessionLease;
        private UnityEnhancedWorldRoot enhancedRoot;
        private bool ownsEnhancedRoot;
        private OwnerInputValidation ownerValidation;
        private ClassicHdSettingsLoadResult classicHdSettingsResult;
        private ClassicHdPresentation classicHdPresentation;
        private ClassicHdWindowLifecycleRecorder windowLifecycleRecorder;
        private UnityEnhancedOwnerSessionSource enhancedOwnerSession;
        private ClassicHdAssetTierResolution classicHdAssetTierResolution;
        private ulong inputSequence;
        private string statusLine;
        private GUIStyle badgeStyle;
        private GUIStyle detailStyle;
        private bool applicationFocused = true;
        private bool applicationPaused;
        private int outputRestoreRequests;

        public PresentationModeId SelectedMode { get; private set; }
        public bool SessionIsOpen { get { return sessionLease != null && sessionLease.IsOpen; } }
        public string StatusLine { get { return statusLine ?? string.Empty; } }
        public OwnerInputValidation OwnerValidation { get { return ownerValidation; } }
        public ClassicHdPresentationSettings ClassicHdSettings
        {
            get { return classicHdSettingsResult == null ? null : classicHdSettingsResult.Settings; }
        }
        public ClassicHdPresentation ClassicHdRenderer { get { return classicHdPresentation; } }
        public string SettingsPath
        {
            get { return classicHdSettingsResult == null ? string.Empty : classicHdSettingsResult.SettingsPath; }
        }
        public UnityEnhancedWorldRoot EnhancedRoot { get { return enhancedRoot; } }
        public UnityEnhancedOwnerSessionSource EnhancedOwnerSession { get { return enhancedOwnerSession; } }
        public bool IsApplicationSuspended { get { return !applicationFocused || applicationPaused; } }
        public bool ApplicationFocused { get { return applicationFocused; } }
        public bool ApplicationPaused { get { return applicationPaused; } }
        public int OutputRestoreRequests { get { return outputRestoreRequests; } }
        public ClassicHdWindowLifecycleRecorder WindowLifecycleRecorder { get { return windowLifecycleRecorder; } }
        public ClassicHdAssetTierResolution ClassicHdAssetTierResolution
        {
            get { return classicHdAssetTierResolution; }
        }

        private void Awake()
        {
            SelectedMode = PresentationModeId.Unknown;
            ownerValidation = OwnerInputValidation.Invalid("NO OWNER INPUTS: no mode or owner-data paths were supplied");
            statusLine = "NO OWNER INPUTS: presentation lab diagnostic boot";

            string[] arguments = Environment.GetCommandLineArgs();
            string modeArgument = CommandLineValue(arguments, ModeArgument);
            if (string.IsNullOrWhiteSpace(modeArgument))
            {
                return;
            }

            PresentationModeId parsedMode;
            if (!TryParseMode(modeArgument, out parsedMode))
            {
                statusLine = "INVALID PRESENTATION MODE: " + modeArgument + ". Use classic-hd or unity-enhanced.";
                return;
            }

            SelectedMode = parsedMode;

            if (SelectedMode == PresentationModeId.ClassicHd)
            {
                classicHdSettingsResult = ClassicHdSettingsStore.LoadFromArguments(arguments);
                if (!classicHdSettingsResult.IsValid)
                {
                    statusLine = "INVALID CLASSIC HD SETTINGS: " + classicHdSettingsResult.Reason;
                    return;
                }
            }

            string projectRoot = Directory.GetParent(Application.dataPath).FullName;
            string packagePath = CommandLineValue(arguments, PackageArgument);
            string closurePath = CommandLineValue(arguments, ClosureArgument);
            string cachePath = CommandLineValue(arguments, CacheArgument);
            if (SelectedMode == PresentationModeId.ClassicHd)
            {
                ClassicHdProfileInfo profileInfo;
                if (!ClassicHdPresentationSettings.TryGetProfile(ClassicHdSettings.profile, out profileInfo))
                {
                    statusLine = "INVALID CLASSIC HD PROFILE: " + ClassicHdSettings.profile;
                    return;
                }

                classicHdAssetTierResolution = ClassicHdAssetTierResolver.Resolve(
                    profileInfo,
                    ClassicHdSettings.assetTier,
                    cachePath);
            }
            ownerValidation = OwnerInputValidator.Validate(SelectedMode, packagePath, closurePath, cachePath, projectRoot);
            if (!ownerValidation.IsValid)
            {
                statusLine = "NO OWNER INPUTS: " + ownerValidation.Reason;
                return;
            }

            sessionLease = PresentationSessionLease.Open(SelectedMode, ownerValidation);
            if (SelectedMode == PresentationModeId.ClassicHd)
            {
                ApplyClassicHdOutputSettings(ClassicHdSettings);
                classicHdPresentation = gameObject.AddComponent<ClassicHdPresentation>();
                string presentationError;
                if (!classicHdPresentation.ConfigureOwnerSession(
                        ClassicHdSettings,
                        packagePath,
                        closurePath,
                        cachePath,
                        out presentationError))
                {
                    classicHdPresentation = null;
                    sessionLease.Dispose();
                    sessionLease = null;
                    statusLine = "CLASSIC HD PRESENTATION FAILED CLOSED: " + presentationError;
                    return;
                }

                string lifecycleReceiptPath = CommandLineValue(arguments, "-mc-lifecycle-receipt");
                if (!string.IsNullOrWhiteSpace(lifecycleReceiptPath))
                {
                    windowLifecycleRecorder = gameObject.AddComponent<ClassicHdWindowLifecycleRecorder>();
                    string lifecycleReason;
                    if (!windowLifecycleRecorder.Configure(
                            this,
                            classicHdPresentation,
                            ClassicHdSettings,
                            lifecycleReceiptPath,
                            CommandLineValue(arguments, "-mc-desktop-session"),
                            CommandLineValue(arguments, "-mc-window-driver"),
                            CommandLineValue(arguments, "-mc-window-case"),
                            out lifecycleReason))
                    {
                        DestroyImmediate(windowLifecycleRecorder);
                        windowLifecycleRecorder = null;
                        statusLine = "CLASSIC HD OS WINDOW RECORDER FAILED CLOSED: " + lifecycleReason;
                        sessionLease.Dispose();
                        sessionLease = null;
                        return;
                    }
                }
            }

            if (SelectedMode == PresentationModeId.UnityEnhanced)
            {
                enhancedRoot = UnityEngine.Object.FindFirstObjectByType<UnityEnhancedWorldRoot>();
                if (enhancedRoot == null)
                {
                    GameObject rootObject = new GameObject("Unity Enhanced Presentation Root");
                    enhancedRoot = rootObject.AddComponent<UnityEnhancedWorldRoot>();
                    ownsEnhancedRoot = true;
                }

                enhancedRoot.Open(SelectedMode, sessionLease);
                enhancedOwnerSession = new UnityEnhancedOwnerSessionSource();
                string enhancedSourceError;
                if (!enhancedOwnerSession.Configure(
                        packagePath,
                        closurePath,
                        cachePath,
                        out enhancedSourceError))
                {
                    enhancedOwnerSession.Dispose();
                    enhancedOwnerSession = null;
                    enhancedRoot.Close();
                    sessionLease.Dispose();
                    sessionLease = null;
                    statusLine = "UNITY ENHANCED PRESENTATION FAILED CLOSED: " + enhancedSourceError;
                    return;
                }

                if (!enhancedRoot.ConfigureOwnerActorCache(cachePath, out enhancedSourceError))
                {
                    enhancedOwnerSession.Dispose();
                    enhancedOwnerSession = null;
                    enhancedRoot.Close();
                    sessionLease.Dispose();
                    sessionLease = null;
                    statusLine = "UNITY ENHANCED PRESENTATION FAILED CLOSED: " + enhancedSourceError;
                    return;
                }

                if (!ApplyPendingEnhancedReset(out enhancedSourceError))
                {
                    enhancedOwnerSession.Dispose();
                    enhancedOwnerSession = null;
                    enhancedRoot.Close();
                    sessionLease.Dispose();
                    sessionLease = null;
                    statusLine = "UNITY ENHANCED PRESENTATION FAILED CLOSED: " + enhancedSourceError;
                    return;
                }
            }

            statusLine = "PRESENTATION SESSION OPEN: " + ModeName(SelectedMode) +
                         " / " + PresentationDetail() +
                         " / gameplay authority remains Rust / visual history is presentation-only";
        }

        private void OnApplicationFocus(bool hasFocus)
        {
            applicationFocused = hasFocus;
            string reason;
            bool applied = ApplyApplicationLifecycleState(out reason);
            if (windowLifecycleRecorder != null)
            {
                windowLifecycleRecorder.RecordUnityLifecycleCallback("focus", applied, reason);
            }
        }

        private void OnApplicationPause(bool paused)
        {
            applicationPaused = paused;
            string reason;
            bool applied = ApplyApplicationLifecycleState(out reason);
            if (windowLifecycleRecorder != null)
            {
                windowLifecycleRecorder.RecordUnityLifecycleCallback("pause", applied, reason);
            }
        }

        /// <summary>
        /// Bounded qualification entry point for the same lifecycle state that
        /// Unity supplies through OnApplicationFocus/OnApplicationPause.
        /// </summary>
        public bool ApplyApplicationLifecycleForQualification(
            bool hasFocus,
            bool paused,
            out string reason)
        {
            applicationFocused = hasFocus;
            applicationPaused = paused;
            return ApplyApplicationLifecycleState(out reason);
        }

        private bool ApplyApplicationLifecycleState(out string reason)
        {
            reason = string.Empty;
            if (SelectedMode != PresentationModeId.ClassicHd || classicHdPresentation == null)
            {
                return true;
            }

            if (!classicHdPresentation.SetApplicationLifecycleState(
                    applicationFocused,
                    applicationPaused,
                    out reason))
            {
                statusLine = "CLASSIC HD LIFECYCLE FAILED: " + reason;
                return false;
            }

            if (applicationFocused && !applicationPaused)
            {
                outputRestoreRequests++;
                ApplyClassicHdOutputSettings(ClassicHdSettings);
            }

            return true;
        }

        private void OnDestroy()
        {
            if (windowLifecycleRecorder != null)
            {
                windowLifecycleRecorder.Flush();
            }

            if (enhancedOwnerSession != null)
            {
                enhancedOwnerSession.Dispose();
                enhancedOwnerSession = null;
            }

            if (enhancedRoot != null)
            {
                enhancedRoot.Close();
                if (ownsEnhancedRoot)
                {
                    Destroy(enhancedRoot.gameObject);
                }

                enhancedRoot = null;
                ownsEnhancedRoot = false;
            }

            if (sessionLease != null)
            {
                sessionLease.Dispose();
                sessionLease = null;
            }
        }

        private void Update()
        {
            if (windowLifecycleRecorder != null)
            {
                windowLifecycleRecorder.ObserveRenderedFrame();
            }

            if (SelectedMode != PresentationModeId.UnityEnhanced ||
                enhancedOwnerSession == null ||
                enhancedRoot == null ||
                enhancedRoot.State != UnityEnhancedLifecycleState.Open)
            {
                return;
            }

            inputSequence++;
            enhancedRoot.InputAdapter.Publish(new UnityEnhancedInputFrame(
                inputSequence,
                new Vector2(Input.GetAxisRaw("Horizontal"), Input.GetAxisRaw("Vertical")),
                new Vector2(Input.GetAxisRaw("Mouse X"), Input.GetAxisRaw("Mouse Y")),
                false,
                Input.GetMouseButton(0),
                SpellSelectionFromInput()));
        }

        private void FixedUpdate()
        {
            if (SelectedMode != PresentationModeId.UnityEnhanced ||
                enhancedOwnerSession == null ||
                enhancedRoot == null ||
                enhancedRoot.State != UnityEnhancedLifecycleState.Open)
            {
                return;
            }

            UnityEnhancedInputFrame input;
            if (!enhancedRoot.InputAdapter.TryConsumeForTick(out input))
            {
                return;
            }

            string reason;
            if (!CaptureEnhancedFrame(
                    input.Flight.x,
                    input.Flight.y,
                    input.Look.x,
                    input.Look.y,
                    input.Cast,
                    input.SpellSelection,
                    out reason))
            {
                statusLine = "UNITY ENHANCED CAPTURE STOPPED: " + reason;
            }
        }

        /// <summary>
        /// Advances the owner-backed Enhanced route once. Editor qualification
        /// commands use this method with a deterministic input recording; the
        /// normal player loop publishes in Update and consumes one frame from
        /// the sequence-bound adapter in FixedUpdate.
        /// </summary>
        public bool CaptureEnhancedFrame(
            float horizontalInput,
            float verticalInput,
            bool cast,
            int spellSelection,
            out string reason)
        {
            return CaptureEnhancedFrame(
                horizontalInput,
                verticalInput,
                0f,
                0f,
                cast,
                spellSelection,
                out reason);
        }

        public bool CaptureEnhancedFrame(
            float horizontalInput,
            float verticalInput,
            float lookYawInput,
            float lookPitchInput,
            bool cast,
            int spellSelection,
            out string reason)
        {
            reason = string.Empty;
            if (enhancedOwnerSession == null || enhancedRoot == null)
            {
                reason = "Unity Enhanced owner session is not configured";
                return false;
            }

            if (!enhancedOwnerSession.Advance(
                    horizontalInput,
                    verticalInput,
                    lookYawInput,
                    lookPitchInput,
                    cast,
                    spellSelection,
                    out UnityEnhancedWorldSnapshot snapshot,
                    out reason))
            {
                return false;
            }

            if (!enhancedRoot.ApplySnapshot(snapshot))
            {
                reason = "Unity Enhanced rejected the owner snapshot at generation " +
                         snapshot.Generation + " tick " + snapshot.Tick +
                         (string.IsNullOrEmpty(enhancedRoot.LastRejectReason)
                             ? string.Empty
                             : ": " + enhancedRoot.LastRejectReason);
                return false;
            }

            return true;
        }

        /// <summary>
        /// Applies one raw external owner command to the Enhanced presentation
        /// route. This is the parity path for the sealed PlaythroughRecorder
        /// journal; it does not synthesize completion or gameplay state.
        /// </summary>
        public bool CaptureEnhancedOwnerCommand(
            sbyte opcode,
            sbyte argument1,
            sbyte argument2,
            sbyte roll,
            sbyte pitch,
            sbyte flags,
            short requestedSpell,
            out string reason)
        {
            reason = string.Empty;
            if (enhancedOwnerSession == null || enhancedRoot == null)
            {
                reason = "Unity Enhanced owner session is not configured";
                return false;
            }

            UnityEnhancedWorldSnapshot snapshot;
            if (!enhancedOwnerSession.AdvanceOwnerCommand(
                    opcode,
                    argument1,
                    argument2,
                    roll,
                    pitch,
                    flags,
                    requestedSpell,
                    out snapshot,
                    out reason))
            {
                return false;
            }

            if (!enhancedRoot.ApplySnapshot(snapshot))
            {
                reason = "Unity Enhanced rejected the owner raw-command snapshot at generation " +
                         snapshot.Generation + " tick " + snapshot.Tick +
                         (string.IsNullOrEmpty(enhancedRoot.LastRejectReason)
                             ? string.Empty
                             : ": " + enhancedRoot.LastRejectReason);
                return false;
            }

            return true;
        }

        /// <summary>
        /// Replays the owner session from its Rust reset boundary without
        /// recreating the Unity hierarchy. The next accepted snapshot carries
        /// the generation reset and clears presentation-only pools.
        /// </summary>
        public bool ResetEnhancedOwnerSession(out string reason)
        {
            reason = string.Empty;
            if (enhancedOwnerSession == null || enhancedRoot == null)
            {
                reason = "Unity Enhanced owner session is not configured";
                return false;
            }

            if (!enhancedOwnerSession.Reset(out reason))
            {
                return false;
            }

            return ApplyPendingEnhancedReset(out reason);
        }

        private bool ApplyPendingEnhancedReset(out string reason)
        {
            reason = string.Empty;
            if (enhancedOwnerSession == null || enhancedRoot == null)
            {
                reason = "Unity Enhanced owner session is not configured";
                return false;
            }

            if (!enhancedOwnerSession.TryTakeResetSnapshot(out UnityEnhancedWorldSnapshot resetSnapshot))
            {
                reason = "Unity Enhanced owner reset snapshot is missing";
                return false;
            }

            if (!enhancedRoot.ApplySnapshot(resetSnapshot))
            {
                reason = "Unity Enhanced rejected the owner reset snapshot at generation " +
                         resetSnapshot.Generation + " tick " + resetSnapshot.Tick +
                         (string.IsNullOrEmpty(enhancedRoot.LastRejectReason)
                             ? string.Empty
                             : ": " + enhancedRoot.LastRejectReason);
                return false;
            }

            return true;
        }

        private void OnGUI()
        {
            if (badgeStyle == null)
            {
                badgeStyle = new GUIStyle(GUI.skin.box);
                badgeStyle.alignment = TextAnchor.UpperLeft;
                badgeStyle.fontSize = 18;
                badgeStyle.normal.textColor = Color.white;
                detailStyle = new GUIStyle(GUI.skin.label);
                detailStyle.fontSize = 13;
                detailStyle.wordWrap = true;
                detailStyle.normal.textColor = new Color(0.86f, 0.9f, 0.95f, 1f);
            }

            float width = Mathf.Max(360f, Mathf.Min(Screen.width - 40f, 980f));
            GUI.color = SessionIsOpen ? new Color(0.06f, 0.25f, 0.13f, 0.94f) : new Color(0.30f, 0.08f, 0.05f, 0.96f);
            GUI.Box(new Rect(20f, 20f, width, 122f), string.Empty, badgeStyle);
            GUI.color = Color.white;
            GUI.Label(new Rect(36f, 32f, width - 32f, 28f),
                SessionIsOpen ? "PRESENTATION LAB" : "PRESENTATION LAB / DIAGNOSTIC", badgeStyle);
            GUI.Label(new Rect(36f, 65f, width - 48f, 44f), StatusLine, detailStyle);
            GUI.Label(new Rect(36f, 112f, width - 48f, 24f),
                "No Exact claim. No mode switching after session open. No owner payload is embedded." +
                (ClassicHdSettings == null ? string.Empty : " " + ClassicHdSettings.profile + " / " + ClassicHdSettings.outputFilter),
                detailStyle);
        }

        private static string CommandLineValue(string[] arguments, string name)
        {
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

        private static bool TryParseMode(string value, out PresentationModeId mode)
        {
            if (string.Equals(value, "classic-hd", StringComparison.OrdinalIgnoreCase))
            {
                mode = PresentationModeId.ClassicHd;
                return true;
            }

            if (string.Equals(value, "unity-enhanced", StringComparison.OrdinalIgnoreCase))
            {
                mode = PresentationModeId.UnityEnhanced;
                return true;
            }

            mode = PresentationModeId.Unknown;
            return false;
        }

        private static string ModeName(PresentationModeId mode)
        {
            switch (mode)
            {
                case PresentationModeId.ClassicHd:
                    return "classic-hd";
                case PresentationModeId.UnityEnhanced:
                    return "unity-enhanced";
                default:
                    return "unknown";
            }
        }

        private string PresentationDetail()
        {
            if (SelectedMode == PresentationModeId.UnityEnhanced)
            {
                return enhancedOwnerSession == null
                    ? "Unity Enhanced diagnostic shell"
                    : "owner-bound Rust snapshots / URP terrain / generated presentation families";
            }

            if (SelectedMode != PresentationModeId.ClassicHd || ClassicHdSettings == null)
            {
                return "presentation lab diagnostic shell";
            }

            string assetTierDetail = ClassicHdSettings.assetTier;
            if (classicHdAssetTierResolution != null)
            {
                assetTierDetail += " / " +
                                   (classicHdAssetTierResolution.FallbackApplied
                                       ? "fallback-original"
                                       : "baseline");
                if (classicHdAssetTierResolution.FallbackApplied)
                {
                    assetTierDetail += " (" + classicHdAssetTierResolution.Reason + ")";
                }
            }

            return ClassicHdSettings.profile + " / " + assetTierDetail +
                   " / persistent R8 surface / owner-bound Rust profile-sized rerender / " +
                   (classicHdSettingsResult.UsedDefaults ? "defaults" : "settings file");
        }

        private static int SpellSelectionFromInput()
        {
            if (Input.GetKeyDown(KeyCode.Alpha1)) return 0;
            if (Input.GetKeyDown(KeyCode.Alpha2)) return 1;
            if (Input.GetKeyDown(KeyCode.Alpha3)) return 2;
            if (Input.GetKeyDown(KeyCode.Alpha4)) return 3;
            return -1;
        }

        private static void ApplyClassicHdOutputSettings(ClassicHdPresentationSettings settings)
        {
            if (settings == null || Application.isBatchMode)
            {
                return;
            }

            FullScreenMode mode = settings.fullscreen
                ? FullScreenMode.FullScreenWindow
                : FullScreenMode.Windowed;
            Screen.SetResolution(settings.windowWidth, settings.windowHeight, mode);
        }
    }
}
