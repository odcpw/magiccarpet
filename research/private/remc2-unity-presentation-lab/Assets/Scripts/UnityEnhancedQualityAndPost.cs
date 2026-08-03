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
                    settings.RenderScale = 1f;
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

            if (RenderScale < 0.5f || RenderScale > 1f || float.IsNaN(RenderScale) || float.IsInfinity(RenderScale))
            {
                reason = "Enhanced render scale must be between 0.5 and 1.0";
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
    /// default; spatial/bilinear URP upscaling is the safe baseline.
    /// </summary>
    [DisallowMultipleComponent]
    public sealed class UnityEnhancedQualityAndPost : MonoBehaviour
    {
        private Volume volume;
        private VolumeProfile volumeProfile;
        private Bloom bloom;
        private Tonemapping toneMapping;
        private ColorAdjustments colorGrading;
        private Camera targetCamera;
        private UniversalAdditionalCameraData cameraData;
        private float priorRenderScale = 1f;
        private UpscalingFilterSelection priorUpscalingFilter = UpscalingFilterSelection.Auto;
        private bool capturedPipelineSettings;
        private bool priorRequiresColorTexture;
        private bool capturedColorTextureSetting;

        public UnityEnhancedQualitySettings Settings { get; private set; }
        public float AppliedRenderScale { get; private set; }
        public bool HdrEnabled { get; private set; }
        public bool BloomEnabled { get; private set; }
        public bool ToneMappingEnabled { get; private set; }
        public bool ColorGradingEnabled { get; private set; }
        public bool AmbientOcclusionRequested { get; private set; }
        public bool TemporalUpscalingEnabled { get; private set; }
        public bool UsesSpatialUpscaling { get; private set; }
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
            cameraData.resetHistory = true;
            ApplyPipelineScale(settings.RenderScale);
            EnsureVolume();
            bloom.active = settings.Bloom;
            bloom.intensity.value = settings.Bloom ? 0.65f : 0f;
            toneMapping.active = settings.ToneMapping;
            toneMapping.mode.value = TonemappingMode.ACES;
            colorGrading.active = settings.ColorGrading;
            AppliedPostExposure = Mathf.Clamp(
                sceneExposure + (settings.Tier == UnityEnhancedQualityTier.Low ? -0.15f : 0f),
                -4f,
                4f);
            colorGrading.postExposure.value = AppliedPostExposure;
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
            AppliedRenderScale = 0f;
            AppliedPostExposure = 0f;
            TemporalUpscalingEnabled = false;
            UsesSpatialUpscaling = false;
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
            UniversalRenderPipeline.asset.upscalingFilter = scale < 0.999f
                ? UpscalingFilterSelection.Auto
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
            RestoreColorTextureSetting();
            RestorePipelineScale();
            if (volumeProfile != null)
            {
                DestroyImmediate(volumeProfile);
            }
        }
    }
}
