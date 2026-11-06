using System;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native
{
    // Presentation-only, sandbox-only. One asynchronous reduction samples the
    // same displaced vertices as water rendering; it never supplies gameplay physics.
    internal sealed class ModernWaterHeightProbe : IDisposable
    {
        internal const float MaximumAge = .15f;
        // The 16 m sampled half-width retains 3 m chop, one 2 m triangle and
        // a 1 m camera margin. Samples outside this central square are rejected.
        internal const float SafeHalfWidth = 10f;
        private readonly ComputeShader compute;
        private readonly ComputeBuffer maximum;
        private readonly int kernel;
        private bool pending, valid, disposed, failed, bufferReleased;
        private AsyncGPUReadbackRequest activeRequest;
        private Vector2 center;
        private float sampledAt, height;
        private ulong revision, epoch;
        internal bool Pending => pending;
        internal bool BufferReleased => bufferReleased;

        internal ModernWaterHeightProbe()
        {
            if (!SystemInfo.supportsComputeShaders || !SystemInfo.supportsAsyncGPUReadback)
                throw new NotSupportedException("Asynchronous compute readback is unavailable");
            var source = Resources.Load<ComputeShader>("ModernWaterHeightProbe");
            if (source == null) throw new InvalidOperationException("Missing water height probe shader");
            compute = UnityEngine.Object.Instantiate(source);
            try { kernel = compute.FindKernel("SampleMaximum"); maximum = new ComputeBuffer(1, sizeof(float)); }
            catch { Destroy(compute); throw; }
        }

        internal static bool IsUsable(Vector2 point, Vector2 sampledCenter, float age, ulong wantedRevision, ulong sampledRevision)
        {
            return float.IsFinite(point.x) && float.IsFinite(point.y) && float.IsFinite(age)
                && age >= 0 && age <= MaximumAge && wantedRevision == sampledRevision
                && Mathf.Abs(point.x-sampledCenter.x) <= SafeHalfWidth
                && Mathf.Abs(point.y-sampledCenter.y) <= SafeHalfWidth;
        }

        internal bool TryClearance(Vector2 point, float now, ulong wantedRevision, out float clearance)
        {
            clearance = 0;
            var age = now-sampledAt;
            if (disposed || !valid || !IsUsable(point, center, age, wantedRevision, revision)) return false;
            // Local velocity is clamped to 8 m/s. The extra .35 m allows broad
            // FFT motion while a readback is in flight; unlike the local bound it
            // is empirical, not a guaranteed wave-velocity bound.
            clearance = Mathf.Max(1.2f, height + .65f + .35f + 8f*age);
            return float.IsFinite(clearance);
        }

        internal void Submit(Vector2 position, float now, ulong currentRevision, Texture broad, Texture coast,
            Texture local, Vector4 localRect, Vector4 wind, float waterTime)
        {
            if (disposed || failed || pending || !float.IsFinite(position.x) || !float.IsFinite(position.y)) return;
            var sampleCenter = new Vector2(Mathf.Round(position.x/2)*2, Mathf.Round(position.y/2)*2);
            var requestEpoch = epoch;
            try
            {
                compute.SetVector("_Center", new Vector4(sampleCenter.x,sampleCenter.y,0,0));
                compute.SetVector("_LocalRect", localRect); compute.SetVector("_Wind", wind);
                compute.SetFloat("_WaterTime", waterTime);
                compute.SetTexture(kernel,"_Broad",broad); compute.SetTexture(kernel,"_Coast",coast);
                compute.SetTexture(kernel,"_Local",local); compute.SetBuffer(kernel,"_Maximum",maximum);
                compute.Dispatch(kernel,1,1,1);
                pending = true;
                activeRequest = AsyncGPUReadback.Request(maximum, request => {
                    pending = false;
                    if (disposed) { ReleaseBuffer(); return; }
                    if (request.hasError)
                    { valid = false; failed = true; Debug.LogWarning("MC_WATER_HEIGHT_PROBE fallback=True reason=readback_error"); return; }
                    if (requestEpoch != epoch) return;
                    var value = request.GetData<float>()[0];
                    valid = float.IsFinite(value);
                    if (valid) { height=value; center=sampleCenter; sampledAt=now; revision=currentRevision; }
                });
            }
            catch (Exception error)
            { pending = false; valid = false; failed = true; Debug.LogWarning($"MC_WATER_HEIGHT_PROBE fallback=True reason={error.Message}"); }
        }

        internal void Invalidate()
        {
            valid = false;
            unchecked { epoch++; }
        }

        public void Dispose()
        {
            if (disposed) return;
            disposed = true; valid = false;
            try
            {
                // Teardown only: Unity may stop pumping readback callbacks during
                // application quit. Drain this probe's one request before release;
                // normal frame updates never wait on the GPU.
                if (pending) { activeRequest.WaitForCompletion(); pending = false; }
            }
            finally { ReleaseBuffer(); Destroy(compute); }
        }

        private void ReleaseBuffer()
        {
            if (bufferReleased) return;
            bufferReleased = true;
            maximum.Release();
        }
        private static void Destroy(UnityEngine.Object value)
        {
            if (value == null) return;
            if (Application.isPlaying) UnityEngine.Object.Destroy(value);
            else UnityEngine.Object.DestroyImmediate(value);
        }
    }
}
