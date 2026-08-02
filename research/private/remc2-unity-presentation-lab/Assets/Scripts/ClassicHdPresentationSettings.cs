using System;
using System.Globalization;
using System.IO;
using UnityEngine;

namespace MagicCarpet.PresentationLab
{
    public enum ClassicHdProfileId
    {
        Compatibility320x200 = 0,
        Hd640x480 = 1,
        Hd960x720 = 2,
        Hd1280x960 = 3
    }

    public enum ClassicHdAssetTier
    {
        OriginalIndexed = 0,
        OptionalHighResolution = 1
    }

    public enum ClassicHdBlurMode
    {
        Off = 0,
        Spatial2x2 = 1,
        TableSn = 2
    }

    public enum ClassicHdOutputFilter
    {
        Point = 0,
        Bilinear = 1
    }

    /// <summary>
    /// Describes the asset tier that the owner-bound Classic HD route actually
    /// selected.  The requested setting and the effective renderer binding are
    /// kept separate so an unavailable optional pack cannot silently become a
    /// release claim.
    /// </summary>
    public sealed class ClassicHdAssetTierResolution
    {
        public ClassicHdAssetTierResolution(
            string requestedToken,
            string effectiveToken,
            bool fallbackApplied,
            bool optionalPackPresent,
            string reason)
        {
            RequestedToken = requestedToken ?? string.Empty;
            EffectiveToken = effectiveToken ?? string.Empty;
            FallbackApplied = fallbackApplied;
            OptionalPackPresent = optionalPackPresent;
            Reason = reason ?? string.Empty;
        }

        public string RequestedToken { get; private set; }
        public string EffectiveToken { get; private set; }
        public bool FallbackApplied { get; private set; }
        public bool OptionalPackPresent { get; private set; }
        public string Reason { get; private set; }
    }

    /// <summary>
    /// Resolves the owner-local Classic HD asset contract without opening any
    /// source DAT/TAB path.  The current owner renderer is deliberately bound
    /// to the qualified original-indexed cache.  A future optional pack gets a
    /// named boundary, but it is not treated as active until the native graph
    /// consumes and qualifies it.
    /// </summary>
    public static class ClassicHdAssetTierResolver
    {
        public const string OriginalIndexedToken = "original-indexed";
        public const string OptionalHighResolutionToken = "optional-high-resolution";
        public const string OptionalPackDirectory = "optional-high-resolution";
        public const string OptionalPackManifest = "manifest.json";

        public static ClassicHdAssetTierResolution Resolve(
            ClassicHdProfileInfo profile,
            string requestedToken,
            string assetCachePath)
        {
            string requested = CanonicalToken(requestedToken);
            if (string.Equals(requested, OriginalIndexedToken, StringComparison.Ordinal))
            {
                return new ClassicHdAssetTierResolution(
                    requested,
                    OriginalIndexedToken,
                    false,
                    false,
                    "qualified owner renderer uses the original indexed cache");
            }

            bool optionalPackPresent = HasOptionalPack(assetCachePath);
            if (!profile.OptionalHighResolutionAssets)
            {
                return Fallback(
                    requested,
                    optionalPackPresent,
                    "the selected Classic HD profile has no qualified optional high-resolution asset tier");
            }

            if (!optionalPackPresent)
            {
                return Fallback(
                    requested,
                    false,
                    "optional high-resolution asset pack is absent; using the original indexed cache");
            }

            return Fallback(
                requested,
                true,
                "optional high-resolution asset pack is present but the owner native graph is not yet qualified to consume it");
        }

        public static bool HasOptionalPack(string assetCachePath)
        {
            if (string.IsNullOrWhiteSpace(assetCachePath))
            {
                return false;
            }

            string directory = Path.Combine(assetCachePath, OptionalPackDirectory);
            return Directory.Exists(directory) &&
                   File.Exists(Path.Combine(directory, OptionalPackManifest));
        }

        private static ClassicHdAssetTierResolution Fallback(
            string requested,
            bool optionalPackPresent,
            string reason)
        {
            return new ClassicHdAssetTierResolution(
                requested,
                OriginalIndexedToken,
                true,
                optionalPackPresent,
                reason);
        }

        private static string CanonicalToken(string token)
        {
            if (string.Equals(token, OptionalHighResolutionToken, StringComparison.OrdinalIgnoreCase))
            {
                return OptionalHighResolutionToken;
            }

            return OriginalIndexedToken;
        }
    }

    [Serializable]
    public struct ClassicHdProfileInfo
    {
        public ClassicHdProfileId Id;
        public int Width;
        public int Height;
        public int Stride;
        public bool ProductionSupported;
        public bool OptionalHighResolutionAssets;

        public ClassicHdProfileInfo(
            ClassicHdProfileId id,
            int width,
            int height,
            bool productionSupported,
            bool optionalHighResolutionAssets)
        {
            Id = id;
            Width = width;
            Height = height;
            Stride = width;
            ProductionSupported = productionSupported;
            OptionalHighResolutionAssets = optionalHighResolutionAssets;
        }
    }

    /// <summary>
    /// Player-facing Classic HD settings.  The strings are intentionally stable
    /// JSON tokens so a launcher can round-trip them without referencing Unity.
    /// </summary>
    [Serializable]
    public sealed class ClassicHdPresentationSettings
    {
        public int schemaVersion = 1;
        public string profile = "hd-640x480";
        public string assetTier = "original-indexed";
        public float viewDistance = 1.0f;
        public bool reflections = true;
        public bool dynamicLighting = true;
        public string blur = "tablesn";
        public float uiScale = 1.0f;
        public bool fullscreen;
        public int windowWidth = 1280;
        public int windowHeight = 960;
        public string outputFilter = "point";

        public static ClassicHdPresentationSettings Default()
        {
            return new ClassicHdPresentationSettings();
        }

        public ClassicHdPresentationSettings Clone()
        {
            return new ClassicHdPresentationSettings
            {
                schemaVersion = schemaVersion,
                profile = profile,
                assetTier = assetTier,
                viewDistance = viewDistance,
                reflections = reflections,
                dynamicLighting = dynamicLighting,
                blur = blur,
                uiScale = uiScale,
                fullscreen = fullscreen,
                windowWidth = windowWidth,
                windowHeight = windowHeight,
                outputFilter = outputFilter
            };
        }

        public bool TryValidate(out string reason)
        {
            reason = string.Empty;
            if (schemaVersion != 1)
            {
                reason = "unsupported Classic HD settings schema version";
                return false;
            }

            ClassicHdProfileInfo profileInfo;
            if (!TryGetProfile(profile, out profileInfo))
            {
                reason = "unsupported Classic HD profile";
                return false;
            }

            if (profileInfo.Id == ClassicHdProfileId.Compatibility320x200)
            {
                reason = "the compatibility profile is reserved for the Exact compatibility fixture";
                return false;
            }

            if (!string.Equals(assetTier, "original-indexed", StringComparison.OrdinalIgnoreCase) &&
                !string.Equals(assetTier, "optional-high-resolution", StringComparison.OrdinalIgnoreCase))
            {
                reason = "unsupported Classic HD asset tier";
                return false;
            }

            if (viewDistance < 0.25f || viewDistance > 4.0f || float.IsNaN(viewDistance) || float.IsInfinity(viewDistance))
            {
                reason = "Classic HD view distance must be between 0.25 and 4.0";
                return false;
            }

            if (!string.Equals(blur, "off", StringComparison.OrdinalIgnoreCase) &&
                !string.Equals(blur, "spatial-2x2", StringComparison.OrdinalIgnoreCase) &&
                !string.Equals(blur, "tablesn", StringComparison.OrdinalIgnoreCase))
            {
                reason = "unsupported Classic HD blur mode";
                return false;
            }

            if (uiScale < 0.5f || uiScale > 4.0f || float.IsNaN(uiScale) || float.IsInfinity(uiScale))
            {
                reason = "Classic HD UI scale must be between 0.5 and 4.0";
                return false;
            }

            if (windowWidth < 320 || windowWidth > 16384 || windowHeight < 200 || windowHeight > 16384)
            {
                reason = "Classic HD output window is outside the supported bounds";
                return false;
            }

            if (!string.Equals(outputFilter, "point", StringComparison.OrdinalIgnoreCase) &&
                !string.Equals(outputFilter, "bilinear", StringComparison.OrdinalIgnoreCase))
            {
                reason = "unsupported Classic HD output filter";
                return false;
            }

            return true;
        }

        public static bool TryGetProfile(string token, out ClassicHdProfileInfo profileInfo)
        {
            if (string.Equals(token, "compatibility-320x200", StringComparison.OrdinalIgnoreCase) ||
                string.Equals(token, "compatibility-320", StringComparison.OrdinalIgnoreCase))
            {
                profileInfo = new ClassicHdProfileInfo(
                    ClassicHdProfileId.Compatibility320x200, 320, 200, false, false);
                return true;
            }

            if (string.Equals(token, "hd-640x480", StringComparison.OrdinalIgnoreCase) ||
                string.Equals(token, "640x480", StringComparison.OrdinalIgnoreCase))
            {
                profileInfo = new ClassicHdProfileInfo(
                    ClassicHdProfileId.Hd640x480, 640, 480, true, false);
                return true;
            }

            if (string.Equals(token, "hd-960x720", StringComparison.OrdinalIgnoreCase) ||
                string.Equals(token, "960x720", StringComparison.OrdinalIgnoreCase))
            {
                profileInfo = new ClassicHdProfileInfo(
                    ClassicHdProfileId.Hd960x720, 960, 720, true, true);
                return true;
            }

            if (string.Equals(token, "hd-1280x960", StringComparison.OrdinalIgnoreCase) ||
                string.Equals(token, "1280x960", StringComparison.OrdinalIgnoreCase))
            {
                profileInfo = new ClassicHdProfileInfo(
                    ClassicHdProfileId.Hd1280x960, 1280, 960, true, true);
                return true;
            }

            profileInfo = default(ClassicHdProfileInfo);
            return false;
        }

        public static string CanonicalProfile(ClassicHdProfileId profileId)
        {
            switch (profileId)
            {
                case ClassicHdProfileId.Compatibility320x200:
                    return "compatibility-320x200";
                case ClassicHdProfileId.Hd640x480:
                    return "hd-640x480";
                case ClassicHdProfileId.Hd960x720:
                    return "hd-960x720";
                case ClassicHdProfileId.Hd1280x960:
                    return "hd-1280x960";
                default:
                    return string.Empty;
            }
        }
    }

    public sealed class ClassicHdSettingsLoadResult
    {
        private ClassicHdSettingsLoadResult(
            bool isValid,
            bool usedDefaults,
            bool explicitFile,
            string settingsPath,
            ClassicHdPresentationSettings settings,
            string reason)
        {
            IsValid = isValid;
            UsedDefaults = usedDefaults;
            ExplicitFile = explicitFile;
            SettingsPath = settingsPath ?? string.Empty;
            Settings = settings ?? ClassicHdPresentationSettings.Default();
            Reason = reason ?? string.Empty;
        }

        public bool IsValid { get; private set; }
        public bool UsedDefaults { get; private set; }
        public bool ExplicitFile { get; private set; }
        public string SettingsPath { get; private set; }
        public ClassicHdPresentationSettings Settings { get; private set; }
        public string Reason { get; private set; }

        public static ClassicHdSettingsLoadResult Valid(
            ClassicHdPresentationSettings settings,
            bool usedDefaults,
            bool explicitFile,
            string settingsPath,
            string reason)
        {
            return new ClassicHdSettingsLoadResult(true, usedDefaults, explicitFile, settingsPath, settings, reason);
        }

        public static ClassicHdSettingsLoadResult Invalid(
            bool explicitFile,
            string settingsPath,
            string reason)
        {
            return new ClassicHdSettingsLoadResult(
                false,
                true,
                explicitFile,
                settingsPath,
                ClassicHdPresentationSettings.Default(),
                reason);
        }
    }

    public static class ClassicHdSettingsStore
    {
        public const string DefaultFileName = "classic-hd-settings.json";
        private const string SettingsArgument = "-mc-settings";
        private const string ProfileArgument = "-mc-classic-profile";
        private const string ResolutionArgument = "-mc-classic-resolution";
        private const string AssetTierArgument = "-mc-classic-asset-tier";
        private const string ViewDistanceArgument = "-mc-classic-view-distance";
        private const string ReflectionsArgument = "-mc-classic-reflections";
        private const string DynamicLightingArgument = "-mc-classic-dynamic-lighting";
        private const string BlurArgument = "-mc-classic-blur";
        private const string UiScaleArgument = "-mc-ui-scale";
        private const string WindowWidthArgument = "-mc-window-width";
        private const string WindowHeightArgument = "-mc-window-height";
        private const string FullscreenArgument = "-mc-fullscreen";
        private const string OutputFilterArgument = "-mc-output-filter";

        public static string DefaultPath()
        {
            return Path.Combine(Application.persistentDataPath, "magiccarpet", "presentation", DefaultFileName);
        }

        public static ClassicHdSettingsLoadResult LoadFromArguments(string[] arguments)
        {
            string settingsPath = CommandLineValue(arguments, SettingsArgument);
            bool explicitFile = !string.IsNullOrWhiteSpace(settingsPath);
            if (!explicitFile)
            {
                settingsPath = DefaultPath();
            }

            ClassicHdPresentationSettings settings = ClassicHdPresentationSettings.Default();
            bool usedDefaults = true;
            if (File.Exists(settingsPath))
            {
                string error;
                if (!TryRead(settingsPath, out settings, out error))
                {
                    return ClassicHdSettingsLoadResult.Invalid(explicitFile, settingsPath, error);
                }

                usedDefaults = false;
            }
            else if (explicitFile)
            {
                return ClassicHdSettingsLoadResult.Invalid(true, settingsPath, "Classic HD settings file does not exist");
            }

            string overrideError;
            if (!TryApplyCommandLineOverrides(arguments, settings, out overrideError))
            {
                return ClassicHdSettingsLoadResult.Invalid(explicitFile, settingsPath, overrideError);
            }

            string validationError;
            if (!settings.TryValidate(out validationError))
            {
                return ClassicHdSettingsLoadResult.Invalid(explicitFile, settingsPath, validationError);
            }

            return ClassicHdSettingsLoadResult.Valid(
                settings,
                usedDefaults,
                explicitFile,
                settingsPath,
                usedDefaults ? "Classic HD defaults selected" : "Classic HD settings loaded");
        }

        public static bool TryRead(string path, out ClassicHdPresentationSettings settings, out string reason)
        {
            settings = ClassicHdPresentationSettings.Default();
            reason = string.Empty;
            try
            {
                string json = File.ReadAllText(path);
                ClassicHdPresentationSettings parsed = JsonUtility.FromJson<ClassicHdPresentationSettings>(json);
                if (parsed == null)
                {
                    reason = "Classic HD settings JSON is empty";
                    return false;
                }

                string validationError;
                if (!parsed.TryValidate(out validationError))
                {
                    reason = validationError;
                    return false;
                }

                settings = parsed;
                return true;
            }
            catch (Exception exception)
            {
                reason = "Classic HD settings JSON could not be read: " + exception.Message;
                return false;
            }
        }

        public static bool TrySave(string path, ClassicHdPresentationSettings settings, out string reason)
        {
            reason = string.Empty;
            if (settings == null)
            {
                reason = "Classic HD settings are null";
                return false;
            }

            string validationError;
            if (!settings.TryValidate(out validationError))
            {
                reason = validationError;
                return false;
            }

            if (string.IsNullOrWhiteSpace(path) || !Path.IsPathRooted(path))
            {
                reason = "Classic HD settings path must be absolute";
                return false;
            }

            try
            {
                string directory = Path.GetDirectoryName(path);
                if (!string.IsNullOrEmpty(directory))
                {
                    Directory.CreateDirectory(directory);
                }

                File.WriteAllText(path, JsonUtility.ToJson(settings, true));
                return true;
            }
            catch (Exception exception)
            {
                reason = "Classic HD settings could not be saved: " + exception.Message;
                return false;
            }
        }

        public static string CommandLineValue(string[] arguments, string name)
        {
            if (arguments == null)
            {
                return string.Empty;
            }

            string prefix = name + "=";
            for (int index = 0; index < arguments.Length; index++)
            {
                string argument = arguments[index] ?? string.Empty;
                if (string.Equals(argument, name, StringComparison.OrdinalIgnoreCase))
                {
                    return index + 1 < arguments.Length ? arguments[index + 1] : string.Empty;
                }

                if (argument.StartsWith(prefix, StringComparison.OrdinalIgnoreCase))
                {
                    return argument.Substring(prefix.Length);
                }
            }

            return string.Empty;
        }

        private static bool TryApplyCommandLineOverrides(
            string[] arguments,
            ClassicHdPresentationSettings settings,
            out string reason)
        {
            reason = string.Empty;
            string value = CommandLineValue(arguments, ProfileArgument);
            if (!string.IsNullOrWhiteSpace(value))
            {
                ClassicHdProfileInfo profileInfo;
                if (!ClassicHdPresentationSettings.TryGetProfile(value, out profileInfo))
                {
                    reason = "unsupported Classic HD profile override";
                    return false;
                }

                settings.profile = ClassicHdPresentationSettings.CanonicalProfile(profileInfo.Id);
            }

            value = CommandLineValue(arguments, ResolutionArgument);
            if (!string.IsNullOrWhiteSpace(value))
            {
                ClassicHdProfileInfo profileInfo;
                if (!ClassicHdPresentationSettings.TryGetProfile(value, out profileInfo))
                {
                    reason = "unsupported Classic HD resolution override";
                    return false;
                }

                settings.profile = ClassicHdPresentationSettings.CanonicalProfile(profileInfo.Id);
            }

            value = CommandLineValue(arguments, AssetTierArgument);
            if (!string.IsNullOrWhiteSpace(value))
            {
                settings.assetTier = value;
            }

            value = CommandLineValue(arguments, ViewDistanceArgument);
            if (!string.IsNullOrWhiteSpace(value) && !TryParseFloat(value, out settings.viewDistance))
            {
                reason = "Classic HD view distance override is not numeric";
                return false;
            }

            value = CommandLineValue(arguments, ReflectionsArgument);
            if (!string.IsNullOrWhiteSpace(value) && !TryParseBool(value, out settings.reflections))
            {
                reason = "Classic HD reflections override is not boolean";
                return false;
            }

            value = CommandLineValue(arguments, DynamicLightingArgument);
            if (!string.IsNullOrWhiteSpace(value) && !TryParseBool(value, out settings.dynamicLighting))
            {
                reason = "Classic HD dynamic-lighting override is not boolean";
                return false;
            }

            value = CommandLineValue(arguments, BlurArgument);
            if (!string.IsNullOrWhiteSpace(value))
            {
                settings.blur = value;
            }

            value = CommandLineValue(arguments, UiScaleArgument);
            if (!string.IsNullOrWhiteSpace(value) && !TryParseFloat(value, out settings.uiScale))
            {
                reason = "Classic HD UI scale override is not numeric";
                return false;
            }

            value = CommandLineValue(arguments, WindowWidthArgument);
            if (!string.IsNullOrWhiteSpace(value) && !int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out settings.windowWidth))
            {
                reason = "Classic HD window width override is not an integer";
                return false;
            }

            value = CommandLineValue(arguments, WindowHeightArgument);
            if (!string.IsNullOrWhiteSpace(value) && !int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out settings.windowHeight))
            {
                reason = "Classic HD window height override is not an integer";
                return false;
            }

            value = CommandLineValue(arguments, FullscreenArgument);
            if (!string.IsNullOrWhiteSpace(value) && !TryParseBool(value, out settings.fullscreen))
            {
                reason = "Classic HD fullscreen override is not boolean";
                return false;
            }

            value = CommandLineValue(arguments, OutputFilterArgument);
            if (!string.IsNullOrWhiteSpace(value))
            {
                settings.outputFilter = value;
            }

            return true;
        }

        private static bool TryParseFloat(string value, out float result)
        {
            return float.TryParse(value, NumberStyles.Float, CultureInfo.InvariantCulture, out result);
        }

        private static bool TryParseBool(string value, out bool result)
        {
            if (bool.TryParse(value, out result))
            {
                return true;
            }

            if (value == "1" || string.Equals(value, "on", StringComparison.OrdinalIgnoreCase))
            {
                result = true;
                return true;
            }

            if (value == "0" || string.Equals(value, "off", StringComparison.OrdinalIgnoreCase))
            {
                result = false;
                return true;
            }

            result = false;
            return false;
        }
    }
}
