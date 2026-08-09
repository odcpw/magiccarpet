using System;
using System.Globalization;
using UnityEngine;

namespace MagicCarpet.PresentationLab
{
    [Serializable]
    public struct UnityEnhancedPresentationMetrics
    {
        public ulong Generation;
        public ulong LastTick;
        public int AppliedSnapshots;
        public int RejectedSnapshots;
        public int ActiveEntities;
        public int PooledEntities;
        public int ActiveEffects;
        public int PooledEffects;
        public int DroppedEffects;
        public int DroppedLights;
        public float LastFrameMilliseconds;
        public long LastManagedBytes;
    }

    /// <summary>
    /// Owner-free instrumentation for support reports.  Reports contain only
    /// counts and timings, never machine paths or payload names.
    /// </summary>
    [DisallowMultipleComponent]
    public sealed class UnityEnhancedDiagnostics : MonoBehaviour
    {
        private float lastFrameStart;
        private UnityEnhancedPresentationMetrics metrics;

        public UnityEnhancedPresentationMetrics Metrics { get { return metrics; } }
        public bool ReducedMotion { get; private set; }
        public bool OverlayEnabled { get; private set; }

        public bool MeetsFrameBudget(float budgetMilliseconds)
        {
            return budgetMilliseconds > 0f && metrics.LastFrameMilliseconds <= budgetMilliseconds;
        }

        public void SetAccessibility(bool reducedMotion, bool overlayEnabled)
        {
            ReducedMotion = reducedMotion;
            OverlayEnabled = overlayEnabled;
        }

        public void RecordSnapshot(UnityEnhancedWorldSnapshot snapshot, UnityEnhancedWorldRoot root)
        {
            if (snapshot == null || root == null)
            {
                return;
            }

            metrics.Generation = snapshot.Generation;
            metrics.LastTick = snapshot.Tick;
            metrics.ActiveEntities = root.ActiveEntityCount;
            metrics.PooledEntities = root.PooledEntityCount;
            metrics.AppliedSnapshots++;
        }

        public void RecordRejectedSnapshot()
        {
            metrics.RejectedSnapshots++;
        }

        public void RecordEffects(UnityEnhancedVfxPresentation vfx, UnityEnhancedNightLighting lighting)
        {
            if (vfx != null)
            {
                metrics.ActiveEffects = vfx.ActiveEffectCount;
                metrics.PooledEffects = vfx.PooledEffectCount;
                metrics.DroppedEffects = vfx.DroppedEffectCount;
            }
            if (lighting != null)
            {
                metrics.DroppedLights = lighting.DroppedLocalLightCount;
            }
        }

        public void BeginFrame()
        {
            lastFrameStart = Time.realtimeSinceStartup;
        }

        public void EndFrame()
        {
            if (lastFrameStart > 0f)
            {
                metrics.LastFrameMilliseconds = Mathf.Max(0f, (Time.realtimeSinceStartup - lastFrameStart) * 1000f);
            }
        }

        public string BuildSupportReport()
        {
            return string.Join("\n", new[]
            {
                "enhanced.schema=1",
                "enhanced.generation=" + metrics.Generation.ToString(CultureInfo.InvariantCulture),
                "enhanced.tick=" + metrics.LastTick.ToString(CultureInfo.InvariantCulture),
                "enhanced.snapshots=" + metrics.AppliedSnapshots.ToString(CultureInfo.InvariantCulture),
                "enhanced.rejected_snapshots=" + metrics.RejectedSnapshots.ToString(CultureInfo.InvariantCulture),
                "enhanced.active_entities=" + metrics.ActiveEntities.ToString(CultureInfo.InvariantCulture),
                "enhanced.pooled_entities=" + metrics.PooledEntities.ToString(CultureInfo.InvariantCulture),
                "enhanced.active_effects=" + metrics.ActiveEffects.ToString(CultureInfo.InvariantCulture),
                "enhanced.pooled_effects=" + metrics.PooledEffects.ToString(CultureInfo.InvariantCulture),
                "enhanced.dropped_effects=" + metrics.DroppedEffects.ToString(CultureInfo.InvariantCulture),
                "enhanced.dropped_lights=" + metrics.DroppedLights.ToString(CultureInfo.InvariantCulture),
                "enhanced.frame_ms=" + metrics.LastFrameMilliseconds.ToString("F3", CultureInfo.InvariantCulture),
                "enhanced.reduced_motion=" + ReducedMotion.ToString().ToLowerInvariant(),
                "enhanced.overlay=" + OverlayEnabled.ToString().ToLowerInvariant()
            });
        }

        public void ResetForSession()
        {
            metrics = default(UnityEnhancedPresentationMetrics);
            lastFrameStart = 0f;
        }
    }
}
