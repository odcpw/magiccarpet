using System;
using System.Collections.Generic;
using UnityEngine;

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

        public static UnityEnhancedNightLightingDescriptor Level1Night()
        {
            return new UnityEnhancedNightLightingDescriptor
            {
                MoonDirection = new Vector3(-0.35f, -0.8f, -0.25f).normalized,
                MoonColor = new Color(0.42f, 0.56f, 1f, 1f),
                MoonIntensity = 0.65f,
                FogColor = new Color(0.018f, 0.028f, 0.09f, 1f),
                FogDensity = 0.012f,
                AmbientColor = new Color(0.035f, 0.055f, 0.12f, 1f),
                Exposure = -0.2f
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
        private readonly List<UnityEnhancedLocalLightRequest> sortedRequests = new List<UnityEnhancedLocalLightRequest>();
        private readonly List<Light> pooledLights = new List<Light>();
        private readonly List<Light> activeLights = new List<Light>();
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

        public UnityEnhancedLightingQuality Quality { get { return quality; } }
        public int ActiveLocalLightCount { get { return activeLights.Count; } }
        public int PooledLocalLightCount { get { return pooledLights.Count; } }
        public int DroppedLocalLightCount { get; private set; }
        public ulong Generation { get { return generation; } }
        public bool HasDescriptor { get { return hasDescriptor; } }

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
            CaptureRenderSettings();
            if (reset || !hasDescriptor || nextGeneration != generation)
            {
                ResetForGeneration(nextGeneration);
            }

            EnsureHierarchy();
            EnsureMoon();
            moon.transform.rotation = Quaternion.LookRotation(-descriptor.MoonDirection.normalized, Vector3.up);
            moon.color = descriptor.MoonColor;
            moon.intensity = Mathf.Max(0f, descriptor.MoonIntensity);
            RenderSettings.fog = descriptor.FogDensity > 0f;
            RenderSettings.fogColor = descriptor.FogColor;
            RenderSettings.fogDensity = Mathf.Max(0f, descriptor.FogDensity);
            RenderSettings.ambientLight = descriptor.AmbientColor;
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

            generation = nextGeneration;
            hasDescriptor = true;
        }

        public void ResetForGeneration(ulong nextGeneration)
        {
            generation = nextGeneration;
            hasDescriptor = false;
            DroppedLocalLightCount = 0;
            while (activeLights.Count > 0)
            {
                RecycleLight(activeLights[activeLights.Count - 1]);
                activeLights.RemoveAt(activeLights.Count - 1);
            }
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
            capturedRenderSettings = true;
        }

        private void OnDestroy()
        {
            if (capturedRenderSettings)
            {
                RenderSettings.fog = priorFog;
                RenderSettings.fogColor = priorFogColor;
                RenderSettings.fogDensity = priorFogDensity;
                RenderSettings.ambientLight = priorAmbient;
            }
        }
    }
}
