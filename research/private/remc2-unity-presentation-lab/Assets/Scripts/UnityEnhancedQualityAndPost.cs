using System;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace MagicCarpet.PresentationLab
{
    public enum UnityEnhancedQualityTier
    {
        Low = 0,
        Medium = 1,
        High = 2,
        WholeHog = 3
    }

    public enum UnityEnhancedTemporalMode
    {
        Off = 0,
        GatedExperimental = 1
    }

    [Serializable]
    public sealed class UnityEnhancedQualitySettings
    {
        public int SchemaVersion = 1;
        public UnityEnhancedQualityTier Tier = UnityEnhancedQualityTier.WholeHog;
        public bool Hdr = true;
        public bool Bloom = true;
        public bool ToneMapping = true;
        public bool ColorGrading = true;
        public bool AmbientOcclusion = true;
        public bool AntiAliasing = true;
        public bool ReducedMotion;
        public bool CameraShake = true;
        public bool SpeedStreaks = true;
        public bool MotionBlur = true;
        public float RenderScale = 1f;
        public UnityEnhancedTemporalMode TemporalMode = UnityEnhancedTemporalMode.Off;
        public bool ExperimentalTemporalGate;

        public static UnityEnhancedQualitySettings ForTier(UnityEnhancedQualityTier tier)
        {
            UnityEnhancedQualitySettings settings = new UnityEnhancedQualitySettings { Tier = tier };
            switch (tier)
            {
                case UnityEnhancedQualityTier.Low:
                    settings.Hdr = false;
                    settings.Bloom = false;
                    settings.ToneMapping = false;
                    settings.ColorGrading = false;
                    settings.AmbientOcclusion = false;
                    settings.AntiAliasing = false;
                    settings.CameraShake = false;
                    settings.SpeedStreaks = false;
                    settings.MotionBlur = false;
                    settings.RenderScale = 0.75f;
                    break;
                case UnityEnhancedQualityTier.Medium:
                    settings.Hdr = true;
                    settings.Bloom = false;
                    settings.ToneMapping = true;
                    settings.ColorGrading = true;
                    settings.AmbientOcclusion = false;
                    settings.AntiAliasing = true;
                    settings.CameraShake = true;
                    settings.SpeedStreaks = false;
                    settings.MotionBlur = false;
                    settings.RenderScale = 0.85f;
                    break;
                case UnityEnhancedQualityTier.High:
                    settings.RenderScale = 1f;
                    break;
                default:
                    // Whole Hog renders above the requested output size and
                    // lets URP resolve that image back to the profile. This
                    // is the Enhanced high-resolution path: the owner HUD
                    // remains a separate indexed overlay while the world and
                    // VFX receive a real supersampled camera render.
                    settings.RenderScale = 1.25f;
                    break;
            }
            return settings;
        }

        public bool TryValidate(out string reason)
        {
            reason = string.Empty;
            if (SchemaVersion != 1)
            {
                reason = "unsupported Enhanced quality schema";
                return false;
            }

            if (RenderScale < 0.5f || RenderScale > 1.5f || float.IsNaN(RenderScale) || float.IsInfinity(RenderScale))
            {
                reason = "Enhanced render scale must be between 0.5 and 1.5";
                return false;
            }

            if (TemporalMode == UnityEnhancedTemporalMode.GatedExperimental && !ExperimentalTemporalGate)
            {
                reason = "temporal upscaling requires the explicit experimental gate";
                return false;
            }

            if (ReducedMotion && (CameraShake || SpeedStreaks || MotionBlur))
            {
                reason = "reduced motion must disable shake, streaks, and motion blur";
                return false;
            }

            return true;
        }

        public bool TemporalUpscalingEnabled
        {
            get { return TemporalMode == UnityEnhancedTemporalMode.GatedExperimental && ExperimentalTemporalGate; }
        }
    }

    /// <summary>
    /// Applies Enhanced quality to URP's camera, render-scale, and volume
    /// boundaries. Temporal AA/upscaling remains explicitly gated off by
    /// default; lower tiers use spatial FSR reconstruction and Whole Hog uses
    /// a bounded supersampled world render.
    /// </summary>
    [DisallowMultipleComponent]
    public sealed class UnityEnhancedQualityAndPost : MonoBehaviour
    {
        private const float EnhancedBloomThreshold = 0.85f;
        private const float EnhancedBloomIntensity = 0.65f;
        private const float EnhancedBloomScatter = 0.78f;
        private const float EnhancedMotionBlurIntensity = 0.28f;
        private const float EnhancedMotionBlurClamp = 0.12f;
        private static readonly Color EnhancedBloomTint = new Color(1f, 0.38f, 0.12f, 1f);
        private const string EnhancedRendererName = "PresentationLabEnhancedRenderer";

        private Volume volume;
        private VolumeProfile volumeProfile;
        private Bloom bloom;
        private MotionBlur motionBlur;
        private Tonemapping toneMapping;
        private ColorAdjustments colorGrading;
        private Camera targetCamera;
        private UniversalAdditionalCameraData cameraData;
        private float priorRenderScale = 1f;
        private UpscalingFilterSelection priorUpscalingFilter = UpscalingFilterSelection.Auto;
        private bool capturedPipelineSettings;
        private bool priorRequiresColorTexture;
        private bool capturedColorTextureSetting;
        private bool hasAppliedSettings;
        private int priorRendererIndex = -1;
        private bool capturedRendererSelection;

        public UnityEnhancedQualitySettings Settings { get; private set; }
        public float AppliedRenderScale { get; private set; }
        public bool HdrEnabled { get; private set; }
        public bool BloomEnabled { get; private set; }
        public bool ToneMappingEnabled { get; private set; }
        public bool ColorGradingEnabled { get; private set; }
        public bool AmbientOcclusionRequested { get; private set; }
        public bool AmbientOcclusionEnabled { get; private set; }
        public bool TemporalUpscalingEnabled { get; private set; }
        public bool UsesSpatialUpscaling { get; private set; }
        public bool UsesSupersampling { get; private set; }
        public bool MotionBlurEnabled { get; private set; }
        public float AppliedMotionBlurIntensity { get; private set; }
        public int CameraHistoryResetRequestCount { get; private set; }
        public float AppliedPostExposure { get; private set; }
        public Volume PostVolume { get { return volume; } }
        public bool OpaqueTextureRequired { get; private set; }

        public bool Apply(UnityEnhancedQualitySettings settings, Camera camera)
        {
            return Apply(settings, camera, 0f);
        }

        public bool Apply(UnityEnhancedQualitySettings settings, Camera camera, float sceneExposure)
        {
            if (settings == null || camera == null)
            {
                return false;
            }

            string reason;
            if (!settings.TryValidate(out reason))
            {
                return false;
            }

            bool configurationChanged = !hasAppliedSettings ||
                                        targetCamera != camera ||
                                        !SettingsEquivalent(Settings, settings);
            Settings = settings;
            targetCamera = camera;
            HdrEnabled = settings.Hdr;
            BloomEnabled = settings.Bloom;
            ToneMappingEnabled = settings.ToneMapping;
            ColorGradingEnabled = settings.ColorGrading;
            AmbientOcclusionRequested = settings.AmbientOcclusion;
            TemporalUpscalingEnabled = settings.TemporalUpscalingEnabled;
            UsesSpatialUpscaling = settings.RenderScale < 0.999f;
            AppliedRenderScale = settings.RenderScale;
            UsesSupersampling = settings.RenderScale > 1.001f;
            camera.allowHDR = settings.Hdr;
            cameraData = camera.GetComponent<UniversalAdditionalCameraData>();
            if (cameraData == null)
            {
                cameraData = camera.gameObject.AddComponent<UniversalAdditionalCameraData>();
            }
            if (!capturedColorTextureSetting)
            {
                priorRequiresColorTexture = cameraData.requiresColorTexture;
                capturedColorTextureSetting = true;
            }
            cameraData.requiresColorTexture = true;
            OpaqueTextureRequired = true;
            cameraData.renderPostProcessing = true;
            cameraData.antialiasing = settings.AntiAliasing
                ? AntialiasingMode.FastApproximateAntialiasing
                : AntialiasingMode.None;
            cameraData.antialiasingQuality = AntialiasingQuality.High;
            if (configurationChanged)
            {
                // Quality is published on every owner tick. Resetting URP's
                // motion history on every tick would leave Motion Blur with
                // no previous camera frame, so only reset when the actual
                // camera/quality configuration changes.
                cameraData.resetHistory = true;
                CameraHistoryResetRequestCount++;
            }
            ApplyPipelineScale(settings.RenderScale);
            AmbientOcclusionEnabled = ApplyAmbientOcclusion(settings.AmbientOcclusion);
            EnsureVolume();
            bloom.active = settings.Bloom;
            bloom.threshold.value = EnhancedBloomThreshold;
            bloom.intensity.value = settings.Bloom ? EnhancedBloomIntensity : 0f;
            bloom.scatter.value = EnhancedBloomScatter;
            bloom.tint.value = EnhancedBloomTint;
            MotionBlurEnabled = settings.MotionBlur;
            AppliedMotionBlurIntensity = settings.MotionBlur ? EnhancedMotionBlurIntensity : 0f;
            motionBlur.active = settings.MotionBlur;
            motionBlur.mode.overrideState = true;
            motionBlur.mode.value = MotionBlurMode.CameraOnly;
            motionBlur.quality.overrideState = true;
            motionBlur.quality.value = settings.Tier == UnityEnhancedQualityTier.WholeHog
                ? MotionBlurQuality.High
                : MotionBlurQuality.Medium;
            motionBlur.intensity.overrideState = true;
            motionBlur.intensity.value = AppliedMotionBlurIntensity;
            motionBlur.clamp.overrideState = true;
            motionBlur.clamp.value = EnhancedMotionBlurClamp;
            toneMapping.active = settings.ToneMapping;
            toneMapping.mode.value = TonemappingMode.ACES;
            colorGrading.active = settings.ColorGrading;
            AppliedPostExposure = Mathf.Clamp(
                sceneExposure + (settings.Tier == UnityEnhancedQualityTier.Low ? -0.15f : 0f),
                -4f,
                4f);
            colorGrading.postExposure.value = AppliedPostExposure;
            hasAppliedSettings = true;
            // Owner capture can render immediately after Apply, before URP's
            // normal player-loop volume update. Refresh the real camera stack
            // here so Bloom/tone mapping are visible on that same frame.
            VolumeManager.instance.Update(camera.transform, camera.cullingMask);
            return true;
        }

        public void ResetForSession()
        {
            if (cameraData != null)
            {
                cameraData.resetHistory = true;
            }
            RestoreColorTextureSetting();
            RestorePipelineScale();
            Settings = null;
            hasAppliedSettings = false;
            AppliedRenderScale = 0f;
            AppliedPostExposure = 0f;
            TemporalUpscalingEnabled = false;
            UsesSpatialUpscaling = false;
            UsesSupersampling = false;
            RestoreAmbientOcclusion();
            AmbientOcclusionEnabled = false;
            MotionBlurEnabled = false;
            AppliedMotionBlurIntensity = 0f;
        }

        private static bool SettingsEquivalent(
            UnityEnhancedQualitySettings left,
            UnityEnhancedQualitySettings right)
        {
            if (left == null || right == null)
            {
                return left == right;
            }

            return left.SchemaVersion == right.SchemaVersion &&
                   left.Tier == right.Tier &&
                   left.Hdr == right.Hdr &&
                   left.Bloom == right.Bloom &&
                   left.ToneMapping == right.ToneMapping &&
                   left.ColorGrading == right.ColorGrading &&
                   left.AmbientOcclusion == right.AmbientOcclusion &&
                   left.AntiAliasing == right.AntiAliasing &&
                   left.ReducedMotion == right.ReducedMotion &&
                   left.CameraShake == right.CameraShake &&
                   left.SpeedStreaks == right.SpeedStreaks &&
                   left.MotionBlur == right.MotionBlur &&
                   left.RenderScale == right.RenderScale &&
                   left.TemporalMode == right.TemporalMode &&
                   left.ExperimentalTemporalGate == right.ExperimentalTemporalGate;
        }

        private void RestoreColorTextureSetting()
        {
            if (capturedColorTextureSetting && cameraData != null)
            {
                cameraData.requiresColorTexture = priorRequiresColorTexture;
            }
            capturedColorTextureSetting = false;
            OpaqueTextureRequired = false;
        }

        private bool ApplyAmbientOcclusion(bool enabled)
        {
            if (UniversalRenderPipeline.asset == null || cameraData == null)
            {
                return false;
            }

            CaptureRendererSelection();

            int enhancedRendererIndex = FindEnhancedRendererIndex();
            if (enhancedRendererIndex < 0 || !enabled || !HasActiveAmbientOcclusion(
                UniversalRenderPipeline.asset.rendererDataList[enhancedRendererIndex]))
            {
                cameraData.SetRenderer(priorRendererIndex);
                return false;
            }

            // The Enhanced renderer is serialized into the pipeline asset so
            // URP keeps SSAO shaders/resources in player builds. The camera
            // switches to it only for Enhanced quality; Exact and Classic HD
            // stay on the default renderer.
            cameraData.SetRenderer(enhancedRendererIndex);
            return true;
        }

        private void CaptureRendererSelection()
        {
            if (capturedRendererSelection)
            {
                return;
            }

            priorRendererIndex = -1;
            ScriptableRenderer currentRenderer = cameraData.scriptableRenderer;
            ReadOnlySpan<ScriptableRenderer> renderers = UniversalRenderPipeline.asset.renderers;
            for (int rendererIndex = 0; rendererIndex < renderers.Length; rendererIndex++)
            {
                if (renderers[rendererIndex] == currentRenderer)
                {
                    priorRendererIndex = rendererIndex;
                    break;
                }
            }

            capturedRendererSelection = true;
        }

        private int FindEnhancedRendererIndex()
        {
            ReadOnlySpan<ScriptableRendererData> rendererDataList =
                UniversalRenderPipeline.asset.rendererDataList;
            for (int rendererIndex = 0; rendererIndex < rendererDataList.Length; rendererIndex++)
            {
                ScriptableRendererData rendererData = rendererDataList[rendererIndex];
                if (rendererData != null && rendererData.name == EnhancedRendererName)
                {
                    return rendererIndex;
                }
            }

            return -1;
        }

        private static bool HasActiveAmbientOcclusion(ScriptableRendererData rendererData)
        {
            if (rendererData == null)
            {
                return false;
            }

            for (int featureIndex = 0; featureIndex < rendererData.rendererFeatures.Count; featureIndex++)
            {
                ScriptableRendererFeature rendererFeature = rendererData.rendererFeatures[featureIndex];
                if (rendererFeature is ScreenSpaceAmbientOcclusion ambientOcclusion &&
                    ambientOcclusion.isActive)
                {
                    return true;
                }
            }

            return false;
        }

        private void RestoreAmbientOcclusion()
        {
            if (cameraData != null && capturedRendererSelection)
            {
                // Presentation lab cameras use the pipeline default renderer
                // outside Unity Enhanced. This restores the exact mode
                // boundary when the Enhanced root closes.
                cameraData.SetRenderer(priorRendererIndex);
            }
            priorRendererIndex = -1;
            capturedRendererSelection = false;
        }

        private void EnsureVolume()
        {
            if (volume == null)
            {
                volume = GetComponent<Volume>();
                if (volume == null)
                {
                    volume = gameObject.AddComponent<Volume>();
                }
                volume.isGlobal = true;
                volume.priority = 100f;
                volume.hideFlags = HideFlags.DontSave;
            }

            if (volumeProfile == null)
            {
                volumeProfile = volume.profile;
                bloom = volumeProfile.Add<Bloom>(true);
                motionBlur = volumeProfile.Add<MotionBlur>(true);
                toneMapping = volumeProfile.Add<Tonemapping>(true);
                colorGrading = volumeProfile.Add<ColorAdjustments>(true);
            }
        }

        private void ApplyPipelineScale(float scale)
        {
            if (UniversalRenderPipeline.asset == null)
            {
                return;
            }

            if (!capturedPipelineSettings)
            {
                priorRenderScale = UniversalRenderPipeline.asset.renderScale;
                priorUpscalingFilter = UniversalRenderPipeline.asset.upscalingFilter;
                capturedPipelineSettings = true;
            }
            UniversalRenderPipeline.asset.renderScale = scale;
            // Use URP's built-in FidelityFX spatial reconstruction for the
            // scaled tiers. URP falls back to its supported filter when the
            // current graphics device cannot run FSR; Whole Hog remains a
            // native-resolution linear path. Temporal upscaling stays behind
            // the separate explicit experimental gate above.
            UniversalRenderPipeline.asset.upscalingFilter = scale < 0.999f
                ? UpscalingFilterSelection.FSR
                : UpscalingFilterSelection.Linear;
        }

        private void RestorePipelineScale()
        {
            if (!capturedPipelineSettings || UniversalRenderPipeline.asset == null)
            {
                return;
            }

            UniversalRenderPipeline.asset.renderScale = priorRenderScale;
            UniversalRenderPipeline.asset.upscalingFilter = priorUpscalingFilter;
            capturedPipelineSettings = false;
        }

        private void OnDestroy()
        {
            RestoreAmbientOcclusion();
            RestoreColorTextureSetting();
            RestorePipelineScale();
            if (volumeProfile != null)
            {
                DestroyImmediate(volumeProfile);
            }
        }
    }
}
