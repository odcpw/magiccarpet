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
        private string statusLine;
        private GUIStyle badgeStyle;
        private GUIStyle detailStyle;

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
                if (!classicHdPresentation.Configure(ClassicHdSettings, out presentationError))
                {
                    classicHdPresentation = null;
                    sessionLease.Dispose();
                    sessionLease = null;
                    statusLine = "CLASSIC HD PRESENTATION FAILED CLOSED: " + presentationError;
                    return;
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
            }

            statusLine = "PRESENTATION SESSION OPEN: " + ModeName(SelectedMode) +
                         " / " + PresentationDetail() +
                         " / gameplay authority remains Rust / visual history is presentation-only";
        }

        private void OnDestroy()
        {
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
            if (SelectedMode != PresentationModeId.ClassicHd || ClassicHdSettings == null)
            {
                return "Unity Enhanced lab placeholder";
            }

            return ClassicHdSettings.profile + " / " + ClassicHdSettings.assetTier +
                   " / persistent R8 surface / " + (classicHdSettingsResult.UsedDefaults ? "defaults" : "settings file");
        }

        private static void ApplyClassicHdOutputSettings(ClassicHdPresentationSettings settings)
        {
            if (settings == null || Application.isBatchMode)
            {
                return;
            }

            Screen.fullScreen = settings.fullscreen;
            Screen.SetResolution(settings.windowWidth, settings.windowHeight, settings.fullscreen);
        }
    }
}
