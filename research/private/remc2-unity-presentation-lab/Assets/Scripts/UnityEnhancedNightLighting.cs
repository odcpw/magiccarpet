using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.PresentationLab
{
    public enum UnityEnhancedLightingQuality
    {
        Low = 0,
        Medium = 1,
        High = 2,
        WholeHog = 3
    }

    [Serializable]
    public struct UnityEnhancedNightLightingDescriptor
    {
        public Vector3 MoonDirection;
        public Color MoonColor;
        public float MoonIntensity;
        public Color FogColor;
        public float FogDensity;
        public Color AmbientColor;
        public float Exposure;
        public Color SkyColor;
        public Color HorizonColor;
        public Color GroundColor;
        public float AmbientIntensity;

        public static UnityEnhancedNightLightingDescriptor Level1Night()
        {
            return new UnityEnhancedNightLightingDescriptor
            {
                MoonDirection = new Vector3(-0.35f, -0.8f, -0.25f).normalized,
                MoonColor = new Color(0.42f, 0.56f, 1f, 1f),
                MoonIntensity = 0.80f,
                FogColor = new Color(0.018f, 0.028f, 0.09f, 1f),
                FogDensity = 0.006f,
                AmbientColor = new Color(0.10f, 0.14f, 0.30f, 1f),
                Exposure = 0.65f,
                SkyColor = new Color(0.035f, 0.075f, 0.24f, 1f),
                HorizonColor = new Color(0.02f, 0.045f, 0.14f, 1f),
                GroundColor = new Color(0.012f, 0.025f, 0.07f, 1f),
                AmbientIntensity = 1.40f
            };
        }
    }

    [Serializable]
    public struct UnityEnhancedLocalLightRequest
    {
        public ulong StableIdentity;
        public Vector3 Position;
        public Color Color;
        public float Intensity;
        public float Range;
        public int Priority;

        public UnityEnhancedLocalLightRequest(
            ulong stableIdentity,
            Vector3 position,
            Color color,
            float intensity,
            float range,
            int priority)
        {
            StableIdentity = stableIdentity;
            Position = position;
            Color = color;
            Intensity = intensity;
            Range = range;
            Priority = priority;
        }
    }

    [DisallowMultipleComponent]
    public sealed class UnityEnhancedNightLighting : MonoBehaviour
    {
        private const int ShaderWarmLightCap = 4;
        private const float BaseNightFogDensity = 0.006f;
        private const float NightAerialFogStart = 96f;
        private const float NightAerialFogEnd = 720f;
        private const string ShaderWarmLightCountName = "_EnhancedWarmLightCount";
        private const string ShaderWarmLightPositionsName = "_EnhancedWarmLightPositions";
        private const string ShaderWarmLightColorsName = "_EnhancedWarmLightColors";
        private const string ShaderWarmLightRangesName = "_EnhancedWarmLightRanges";
        private readonly List<UnityEnhancedLocalLightRequest> sortedRequests = new List<UnityEnhancedLocalLightRequest>();
        private readonly List<Light> pooledLights = new List<Light>();
        private readonly List<Light> activeLights = new List<Light>();
        private readonly Vector4[] shaderWarmLightPositions = new Vector4[ShaderWarmLightCap];
        private readonly Vector4[] shaderWarmLightColors = new Vector4[ShaderWarmLightCap];
        private readonly Vector4[] shaderWarmLightRanges = new Vector4[ShaderWarmLightCap];
        private Transform lightRoot;
        private Light moon;
        private UnityEnhancedLightingQuality quality = UnityEnhancedLightingQuality.High;
        private ulong generation;
        private bool hasDescriptor;
        private bool capturedRenderSettings;
        private bool priorFog;
        private Color priorFogColor;
        private float priorFogDensity;
        private Color priorAmbient;
        private AmbientMode priorAmbientMode;
        private float priorAmbientIntensity;
        private Color priorAmbientSky;
        private Color priorAmbientEquator;
        private Color priorAmbientGround;
        private FogMode priorFogMode;
        private float priorFogStartDistance;
        private float priorFogEndDistance;
        private Material priorSkybox;
        private Material generatedSkybox;
        private Light priorSun;
        private Camera presentationCamera;
        private CameraClearFlags priorCameraClearFlags;
        private Color priorCameraBackgroundColor;
        private bool capturedCameraSettings;

        public UnityEnhancedLightingQuality Quality { get { return quality; } }
        public int ActiveLocalLightCount { get { return activeLights.Count; } }
        public int PooledLocalLightCount { get { return pooledLights.Count; } }
        public int DroppedLocalLightCount { get; private set; }
        public ulong Generation { get { return generation; } }
        public bool HasDescriptor { get { return hasDescriptor; } }
        public float AppliedExposure { get; private set; }
        public float AppliedFogStartDistance { get; private set; }
        public float AppliedFogEndDistance { get; private set; }
        public int ShaderVisibleWarmLightCount { get; private set; }

        public int LocalLightCap
        {
            get
            {
                switch (quality)
                {
                    case UnityEnhancedLightingQuality.Low: return 2;
                    case UnityEnhancedLightingQuality.Medium: return 6;
                    case UnityEnhancedLightingQuality.High: return 12;
                    default: return 24;
                }
            }
        }

        public void SetQuality(UnityEnhancedLightingQuality nextQuality)
        {
            quality = nextQuality;
            if (activeLights.Count > LocalLightCap)
            {
                for (int index = activeLights.Count - 1; index >= LocalLightCap; index--)
                {
                    RecycleLight(activeLights[index]);
                    activeLights.RemoveAt(index);
                }
            }
        }

        public void ApplyDescriptor(
            ulong nextGeneration,
            UnityEnhancedNightLightingDescriptor descriptor,
            IList<UnityEnhancedLocalLightRequest> requests,
            bool reset)
        {
            ApplyDescriptor(nextGeneration, descriptor, requests, reset, null);
        }

        public void ApplyDescriptor(
            ulong nextGeneration,
            UnityEnhancedNightLightingDescriptor descriptor,
            IList<UnityEnhancedLocalLightRequest> requests,
            bool reset,
            Camera camera)
        {
            CaptureRenderSettings();
            CaptureCamera(camera);
            if (reset || !hasDescriptor || nextGeneration != generation)
            {
                ResetForGeneration(nextGeneration);
            }

            EnsureHierarchy();
            EnsureMoon();
            // MoonDirection is the direction travelled by the light rays. The
            // sky uses its inverse as the visible moon direction, while URP
            // derives the shader light direction from the inverse of this
            // transform. Keep the light above the terrain and the shadow map
            // aligned with the same night descriptor.
            moon.transform.rotation = Quaternion.LookRotation(
                descriptor.MoonDirection.normalized,
                Vector3.up);
            moon.color = descriptor.MoonColor;
            moon.intensity = Mathf.Max(0f, descriptor.MoonIntensity);
            RenderSettings.fog = descriptor.FogDensity > 0f;
            RenderSettings.fogColor = descriptor.FogColor;
            float densityScale = Mathf.Clamp(
                descriptor.FogDensity / BaseNightFogDensity,
                0.75f,
                1.5f);
            AppliedFogStartDistance = NightAerialFogStart / densityScale;
            AppliedFogEndDistance = NightAerialFogEnd / densityScale;
            // Keep close terrain readable and reserve the fog for the owner
            // world horizon. This is a bounded presentation effect; the Rust
            // camera and terrain planes remain authoritative.
            RenderSettings.fogMode = FogMode.Linear;
            RenderSettings.fogStartDistance = AppliedFogStartDistance;
            RenderSettings.fogEndDistance = AppliedFogEndDistance;
            RenderSettings.fogDensity = Mathf.Max(0f, descriptor.FogDensity);
            RenderSettings.ambientMode = AmbientMode.Trilight;
            RenderSettings.ambientSkyColor = descriptor.SkyColor;
            RenderSettings.ambientEquatorColor = descriptor.AmbientColor;
            RenderSettings.ambientGroundColor = descriptor.GroundColor;
            RenderSettings.ambientIntensity = Mathf.Clamp(descriptor.AmbientIntensity, 0f, 8f);
            RenderSettings.ambientLight = descriptor.AmbientColor;
            RenderSettings.sun = moon;
            AppliedExposure = Mathf.Clamp(descriptor.Exposure, -4f, 4f);
            if (presentationCamera != null)
            {
                if (ApplyProceduralSkybox(descriptor))
                {
                    presentationCamera.clearFlags = CameraClearFlags.Skybox;
                }
                else
                {
                    presentationCamera.clearFlags = CameraClearFlags.SolidColor;
                    presentationCamera.backgroundColor = descriptor.HorizonColor;
                }
            }
            sortedRequests.Clear();
            if (requests != null)
            {
                for (int index = 0; index < requests.Count; index++)
                {
                    sortedRequests.Add(requests[index]);
                }
            }
            sortedRequests.Sort(CompareRequests);
            DroppedLocalLightCount = Mathf.Max(0, sortedRequests.Count - LocalLightCap);
            while (activeLights.Count > 0)
            {
                RecycleLight(activeLights[activeLights.Count - 1]);
                activeLights.RemoveAt(activeLights.Count - 1);
            }
            int accepted = Mathf.Min(sortedRequests.Count, LocalLightCap);
            for (int index = 0; index < accepted; index++)
            {
                UnityEnhancedLocalLightRequest request = sortedRequests[index];
                Light light = AcquireLight(index);
                light.transform.SetPositionAndRotation(request.Position, Quaternion.identity);
                light.color = request.Color;
                light.intensity = Mathf.Max(0f, request.Intensity);
                light.range = Mathf.Max(0.05f, request.Range);
                activeLights.Add(light);
            }

            ApplyShaderWarmLightField(accepted);

            generation = nextGeneration;
            hasDescriptor = true;
        }

        public void ResetForGeneration(ulong nextGeneration)
        {
            generation = nextGeneration;
            hasDescriptor = false;
            DroppedLocalLightCount = 0;
            AppliedExposure = 0f;
            AppliedFogStartDistance = 0f;
            AppliedFogEndDistance = 0f;
            ClearShaderWarmLightField();
            while (activeLights.Count > 0)
            {
                RecycleLight(activeLights[activeLights.Count - 1]);
                activeLights.RemoveAt(activeLights.Count - 1);
            }
        }

        public void ResetForSession()
        {
            ResetForGeneration(0);
            RestoreRenderSettings();
        }

        private void ApplyShaderWarmLightField(int accepted)
        {
            ClearShaderWarmLightField();
            int count = 0;
            int boundedAccepted = Mathf.Min(accepted, sortedRequests.Count);
            for (int index = 0; index < boundedAccepted && count < ShaderWarmLightCap; index++)
            {
                UnityEnhancedLocalLightRequest request = sortedRequests[index];
                if (!IsWarmLight(request.Color))
                {
                    continue;
                }

                shaderWarmLightPositions[count] = new Vector4(
                    request.Position.x,
                    request.Position.y,
                    request.Position.z,
                    1f);
                shaderWarmLightColors[count] = new Vector4(
                    request.Color.r,
                    request.Color.g,
                    request.Color.b,
                    Mathf.Max(0f, request.Intensity));
                shaderWarmLightRanges[count] = new Vector4(
                    Mathf.Min(Mathf.Max(0.05f, request.Range) * 2f, 24f),
                    0f,
                    0f,
                    0f);
                count++;
            }

            ShaderVisibleWarmLightCount = count;
            Shader.SetGlobalInt(ShaderWarmLightCountName, count);
            Shader.SetGlobalVectorArray(ShaderWarmLightPositionsName, shaderWarmLightPositions);
            Shader.SetGlobalVectorArray(ShaderWarmLightColorsName, shaderWarmLightColors);
            Shader.SetGlobalVectorArray(ShaderWarmLightRangesName, shaderWarmLightRanges);
        }

        private void ClearShaderWarmLightField()
        {
            for (int index = 0; index < ShaderWarmLightCap; index++)
            {
                shaderWarmLightPositions[index] = Vector4.zero;
                shaderWarmLightColors[index] = Vector4.zero;
                shaderWarmLightRanges[index] = Vector4.zero;
            }

            ShaderVisibleWarmLightCount = 0;
            Shader.SetGlobalInt(ShaderWarmLightCountName, 0);
            Shader.SetGlobalVectorArray(ShaderWarmLightPositionsName, shaderWarmLightPositions);
            Shader.SetGlobalVectorArray(ShaderWarmLightColorsName, shaderWarmLightColors);
            Shader.SetGlobalVectorArray(ShaderWarmLightRangesName, shaderWarmLightRanges);
        }

        private static bool IsWarmLight(Color color)
        {
            return color.r > color.g * 0.85f && color.r > color.b * 1.2f;
        }

        private static int CompareRequests(UnityEnhancedLocalLightRequest left, UnityEnhancedLocalLightRequest right)
        {
            int priority = right.Priority.CompareTo(left.Priority);
            return priority != 0 ? priority : left.StableIdentity.CompareTo(right.StableIdentity);
        }

        private void EnsureHierarchy()
        {
            if (lightRoot != null)
            {
                return;
            }

            GameObject root = new GameObject("EnhancedNightLights");
            root.transform.SetParent(transform, false);
            root.hideFlags = HideFlags.DontSave;
            lightRoot = root.transform;
        }

        private void EnsureMoon()
        {
            if (moon != null)
            {
                return;
            }

            GameObject moonObject = new GameObject("EnhancedMoonLight");
            moonObject.transform.SetParent(transform, false);
            moonObject.hideFlags = HideFlags.DontSave;
            moon = moonObject.AddComponent<Light>();
            moon.type = LightType.Directional;
            moon.shadows = LightShadows.Soft;
        }

        private Light AcquireLight(int index)
        {
            Light light;
            if (pooledLights.Count > 0)
            {
                int last = pooledLights.Count - 1;
                light = pooledLights[last];
                pooledLights.RemoveAt(last);
                light.gameObject.SetActive(true);
                return light;
            }

            GameObject lightObject = new GameObject("EnhancedLocalLight_" + index);
            lightObject.transform.SetParent(lightRoot, false);
            lightObject.hideFlags = HideFlags.DontSave;
            light = lightObject.AddComponent<Light>();
            light.type = LightType.Point;
            light.shadows = LightShadows.None;
            return light;
        }

        private void RecycleLight(Light light)
        {
            if (light == null)
            {
                return;
            }

            light.gameObject.SetActive(false);
            pooledLights.Add(light);
        }

        private void CaptureRenderSettings()
        {
            if (capturedRenderSettings)
            {
                return;
            }

            priorFog = RenderSettings.fog;
            priorFogColor = RenderSettings.fogColor;
            priorFogDensity = RenderSettings.fogDensity;
            priorAmbient = RenderSettings.ambientLight;
            priorAmbientMode = RenderSettings.ambientMode;
            priorAmbientIntensity = RenderSettings.ambientIntensity;
            priorAmbientSky = RenderSettings.ambientSkyColor;
            priorAmbientEquator = RenderSettings.ambientEquatorColor;
            priorAmbientGround = RenderSettings.ambientGroundColor;
            priorFogMode = RenderSettings.fogMode;
            priorFogStartDistance = RenderSettings.fogStartDistance;
            priorFogEndDistance = RenderSettings.fogEndDistance;
            priorSkybox = RenderSettings.skybox;
            priorSun = RenderSettings.sun;
            capturedRenderSettings = true;
        }

        private void CaptureCamera(Camera camera)
        {
            if (camera == null)
            {
                return;
            }

            if (!capturedCameraSettings || presentationCamera != camera)
            {
                presentationCamera = camera;
                priorCameraClearFlags = camera.clearFlags;
                priorCameraBackgroundColor = camera.backgroundColor;
                capturedCameraSettings = true;
            }
        }

        private void RestoreRenderSettings()
        {
            if (capturedRenderSettings)
            {
                RenderSettings.fog = priorFog;
                RenderSettings.fogColor = priorFogColor;
                RenderSettings.fogDensity = priorFogDensity;
                RenderSettings.fogMode = priorFogMode;
                RenderSettings.fogStartDistance = priorFogStartDistance;
                RenderSettings.fogEndDistance = priorFogEndDistance;
                RenderSettings.ambientMode = priorAmbientMode;
                RenderSettings.ambientIntensity = priorAmbientIntensity;
                RenderSettings.ambientSkyColor = priorAmbientSky;
                RenderSettings.ambientEquatorColor = priorAmbientEquator;
                RenderSettings.ambientGroundColor = priorAmbientGround;
                RenderSettings.ambientLight = priorAmbient;
                RenderSettings.skybox = priorSkybox;
                RenderSettings.sun = priorSun;
                capturedRenderSettings = false;
            }

            if (capturedCameraSettings && presentationCamera != null)
            {
                presentationCamera.clearFlags = priorCameraClearFlags;
                presentationCamera.backgroundColor = priorCameraBackgroundColor;
                capturedCameraSettings = false;
            }

            presentationCamera = null;
        }

        private bool ApplyProceduralSkybox(UnityEnhancedNightLightingDescriptor descriptor)
        {
            Shader shader = Shader.Find("MagicCarpet/Enhanced/NightSky");
            bool customNightSky = shader != null;
            if (shader == null)
            {
                shader = Shader.Find("Skybox/Procedural");
            }
            if (shader == null)
            {
                return false;
            }

            if (generatedSkybox == null || generatedSkybox.shader != shader)
            {
                if (generatedSkybox != null)
                {
                    DestroyImmediate(generatedSkybox);
                }

                generatedSkybox = new Material(shader)
                {
                    name = "EnhancedLevel1NightProceduralSky",
                    hideFlags = HideFlags.DontSave
                };
            }

            if (customNightSky)
            {
                generatedSkybox.SetColor("_ZenithColor", new Color(0.012f, 0.035f, 0.16f, 1f));
                generatedSkybox.SetColor("_HorizonColor", new Color(0.045f, 0.12f, 0.32f, 1f));
                generatedSkybox.SetColor("_GroundColor", new Color(0.004f, 0.012f, 0.04f, 1f));
                generatedSkybox.SetColor("_StarColor", new Color(0.55f, 0.72f, 1f, 1f));
                generatedSkybox.SetFloat("_StarDensity", 0.025f);
                generatedSkybox.SetFloat("_StarIntensity", 0.8f);
                generatedSkybox.SetFloat("_StarTwinkleStrength", 0.18f);
                generatedSkybox.SetVector("_MoonDirection", -descriptor.MoonDirection.normalized);
                generatedSkybox.SetColor("_MoonColor", descriptor.MoonColor);
                generatedSkybox.SetFloat("_MoonIntensity", descriptor.MoonIntensity);
                generatedSkybox.SetFloat("_MoonSize", 0.035f);
                generatedSkybox.SetFloat("_MoonGlow", 0.45f);
                generatedSkybox.SetFloat("_HorizonGlowStrength", 0.18f);
                generatedSkybox.SetFloat("_Exposure", Mathf.Clamp(0.85f + descriptor.Exposure * 0.25f, 0f, 4f));
            }
            else if (generatedSkybox.HasProperty("_SkyTint"))
            {
                generatedSkybox.SetColor("_SkyTint", descriptor.SkyColor);
            }
            if (!customNightSky && generatedSkybox.HasProperty("_GroundColor"))
            {
                generatedSkybox.SetColor("_GroundColor", descriptor.GroundColor);
            }
            if (!customNightSky && generatedSkybox.HasProperty("_SunSize"))
            {
                generatedSkybox.SetFloat("_SunSize", 0.015f);
            }
            if (!customNightSky && generatedSkybox.HasProperty("_SunSizeConvergence"))
            {
                generatedSkybox.SetFloat("_SunSizeConvergence", 5f);
            }
            if (!customNightSky && generatedSkybox.HasProperty("_AtmosphereThickness"))
            {
                generatedSkybox.SetFloat("_AtmosphereThickness", 0.55f);
            }
            if (!customNightSky && generatedSkybox.HasProperty("_Exposure"))
            {
                generatedSkybox.SetFloat("_Exposure", Mathf.Clamp(0.45f + descriptor.Exposure * 0.45f, 0f, 8f));
            }

            RenderSettings.skybox = generatedSkybox;
            return true;
        }

        private void OnDestroy()
        {
            ClearShaderWarmLightField();
            RestoreRenderSettings();
            if (generatedSkybox != null)
            {
                DestroyImmediate(generatedSkybox);
                generatedSkybox = null;
            }
        }
    }
}
