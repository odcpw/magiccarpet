using System;
using UnityEngine;

namespace MagicCarpet.Native
{
    // Cosmetic linear wave field; never participates in authoritative collision or RNG.
    // A finite camera-near patch is not the toroidal world: its outer edges absorb waves.
    internal sealed class ModernWaterInteractions : IDisposable
    {
        internal const int Resolution = 256;
        internal const float Size = 128f;
        internal const float CellSize = Size / Resolution;
        internal const float FixedStep = 1f / 60f;
        internal const float WaveSpeed = 5f;
        internal const int MaximumSteps = 4;
        internal const int MaximumImpulses = 32;
        private const float WorldPeriod = 1024f;
        private readonly ComputeShader compute;
        private readonly int clearKernel, scrollKernel, stepKernel;
        private readonly Vector4[] impulses = new Vector4[MaximumImpulses];
        private RenderTexture current, previous, scratch;
        private Vector2 origin;
        private float accumulated;
        private int impulseCount;
        private bool initialized, needsClear = true, disposed;

        // R height in world units, G vertical speed, B persistent foam coverage.
        internal RenderTexture State => current;
        internal RenderTexture PreviousState => previous;
        internal float Interpolation => Mathf.Clamp01(accumulated / FixedStep);
        internal Vector4 WorldRect => new Vector4(origin.x, origin.y, Size, 1f / Size);

        internal ModernWaterInteractions()
        {
            if (!SystemInfo.supportsComputeShaders || !SystemInfo.SupportsRenderTextureFormat(RenderTextureFormat.ARGBFloat))
                throw new NotSupportedException("Modern water interactions require compute shaders and floating-point textures");
            var resource = Resources.Load<ComputeShader>("ModernWaterInteractions");
            if (resource == null) throw new InvalidOperationException("Missing ModernWaterInteractions compute shader");
            compute = UnityEngine.Object.Instantiate(resource);
            try
            {
                clearKernel = compute.FindKernel("Clear");
                scrollKernel = compute.FindKernel("Scroll");
                stepKernel = compute.FindKernel("Step");
                current = Create("Water interactions current");
                previous = Create("Water interactions previous");
                scratch = Create("Water interactions scratch");
                compute.SetInt("_Resolution", Resolution);
                compute.SetFloat("_CellSize", CellSize);
                compute.SetFloat("_Step", FixedStep);
                compute.SetFloat("_WaveSpeedSquared", WaveSpeed * WaveSpeed);
            }
            catch { Dispose(); throw; }
        }

        private static RenderTexture Create(string name)
        {
            var texture = new RenderTexture(Resolution, Resolution, 0, RenderTextureFormat.ARGBFloat,
                RenderTextureReadWrite.Linear) {
                name = name, enableRandomWrite = true, filterMode = FilterMode.Bilinear,
                wrapMode = TextureWrapMode.Clamp, useMipMap = false, autoGenerateMips = false
            };
            if (texture.Create()) return texture;
            Destroy(texture);
            throw new InvalidOperationException("Cannot allocate local water interaction field");
        }

        // Call after a scene generation or wet-mask revision. Old waves/foam must not survive
        // a changed water body, nor reappear when new land becomes water again.
        internal void Reset()
        {
            ThrowIfDisposed();
            needsClear = true;
            accumulated = 0f;
            impulseCount = 0;
        }

        internal void AddImpulse(Vector3 position, float radius, float strength)
        {
            ThrowIfDisposed();
            if (!Finite(position.x) || !Finite(position.z) || !Finite(radius) || !Finite(strength))
                throw new ArgumentException("Water impulse requires finite position, radius and strength");
            if (radius <= 0f) throw new ArgumentOutOfRangeException(nameof(radius));
            if (strength == 0f) return;
            var impulse = new Vector4(Mathf.Repeat(position.x, WorldPeriod), Mathf.Repeat(position.z, WorldPeriod),
                Mathf.Clamp(radius, CellSize * 3f, 12f), Mathf.Clamp(strength, -8f, 8f));
            impulseCount = AppendImpulse(impulses, impulseCount, impulse);
        }

        // Strength is the total impulse budget for this observation, normally scaled by dt.
        // A teleport is not a wake. Shortest wrapped segments preserve trails across map edges.
        internal void AddWake(Vector3 from, Vector3 to, float radius, float strength)
        {
            if (!Finite(from.x) || !Finite(from.z) || !Finite(to.x) || !Finite(to.z)
                || !Finite(radius) || radius <= 0f || !Finite(strength))
                throw new ArgumentException("Water wake requires finite endpoints, positive radius and finite strength");
            var delta = new Vector2(WrappedDelta(to.x - from.x), WrappedDelta(to.z - from.z));
            if (delta.sqrMagnitude > Size * Size * .25f) return;
            var count = Mathf.Clamp(Mathf.CeilToInt(delta.magnitude / Mathf.Max(CellSize, radius * .5f)), 1, MaximumImpulses);
            for (var i = 0; i < count; i++)
            {
                var fraction = (i + .5f) / count;
                AddImpulse(new Vector3(from.x + delta.x * fraction, 0f, from.z + delta.y * fraction), radius, strength / count);
            }
        }

        internal void Update(float dt, Vector3 cameraPosition, Vector2 windVelocity, Texture coastTexture,
            Texture breakingTexture = null, float waterTime = -1f)
        {
            ThrowIfDisposed();
            if (!Finite(dt) || dt < 0f || !Finite(cameraPosition.x) || !Finite(cameraPosition.z)
                || !Finite(windVelocity.x) || !Finite(windVelocity.y) || !Finite(waterTime))
                throw new ArgumentException("Water update requires finite nonnegative time and finite positions/wind");
            if (coastTexture == null) throw new ArgumentNullException(nameof(coastTexture));
            var nextOrigin = SnappedOrigin(cameraPosition);
            var shift = initialized ? HistoryShift(origin, nextOrigin) : Vector2Int.zero;
            // Discard excess cosmetic history after a stall instead of unbounded catch-up.
            if (!initialized || Mathf.Abs(shift.x) >= Resolution || Mathf.Abs(shift.y) >= Resolution || dt > .25f)
                needsClear = true;
            if (needsClear)
            {
                Clear(current); Clear(previous); Clear(scratch);
                accumulated = 0f;
                needsClear = false;
            }
            else if (shift != Vector2Int.zero)
            {
                compute.SetInts("_Scroll", shift.x, shift.y);
                Scroll(current, scratch);
                var oldCurrent = current; current = scratch; scratch = oldCurrent;
                Scroll(previous, scratch);
                var oldPrevious = previous; previous = scratch; scratch = oldPrevious;
            }
            initialized = true;
            origin = nextOrigin;
            compute.SetVector("_WorldRect", WorldRect);
            // This is an explicit wind-drift approximation, not material flow recovered
            // from a height + vertical-velocity solver and not wave phase velocity.
            var drift = Vector2.ClampMagnitude(windVelocity, 40f) * .035f;
            compute.SetVector("_FoamDrift", new Vector4(drift.x, drift.y, 0f, 0f));
            compute.SetVector("_Wind", new Vector4(windVelocity.x, windVelocity.y, 0f, 0f));
            compute.SetFloat("_WaterTime", waterTime >= 0f ? waterTime : Time.time);
            compute.SetTexture(stepKernel, "_Coast", coastTexture);
            compute.SetTexture(stepKernel, "_Breaking", breakingTexture != null ? breakingTexture : Texture2D.blackTexture);
            compute.SetInt("_HasBreaking", breakingTexture != null ? 1 : 0);
            var steps = ConsumeSteps(ref accumulated, dt);
            if (steps == 0) return; // Impulses remain queued until the next fixed step.
            compute.SetVectorArray("_Impulses", impulses);
            for (var i = 0; i < steps; i++)
            {
                compute.SetInt("_ImpulseCount", i == 0 ? impulseCount : 0);
                compute.SetTexture(stepKernel, "_Input", current);
                compute.SetTexture(stepKernel, "_Output", scratch);
                compute.Dispatch(stepKernel, Resolution / 8, Resolution / 8, 1);
                var oldPrevious = previous; previous = current; current = scratch; scratch = oldPrevious;
            }
            impulseCount = 0;
        }

        private void Clear(RenderTexture texture)
        {
            compute.SetTexture(clearKernel, "_Output", texture);
            compute.Dispatch(clearKernel, Resolution / 8, Resolution / 8, 1);
        }

        private void Scroll(RenderTexture input, RenderTexture output)
        {
            compute.SetTexture(scrollKernel, "_Input", input);
            compute.SetTexture(scrollKernel, "_Output", output);
            compute.Dispatch(scrollKernel, Resolution / 8, Resolution / 8, 1);
        }

        internal static Vector2 SnappedOrigin(Vector3 position) => new Vector2(
            Mathf.Floor(position.x / CellSize) * CellSize - Size * .5f,
            Mathf.Floor(position.z / CellSize) * CellSize - Size * .5f);

        // Shift maps a new destination cell to an old source cell. A world wrap is
        // continuous, but exceeding the finite patch resets instead of wrapping storage.
        internal static Vector2Int HistoryShift(Vector2 oldOrigin, Vector2 newOrigin) => new Vector2Int(
            Mathf.RoundToInt(WrappedDelta(newOrigin.x - oldOrigin.x) / CellSize),
            Mathf.RoundToInt(WrappedDelta(newOrigin.y - oldOrigin.y) / CellSize));

        internal static float WrappedDelta(float delta) => Mathf.Repeat(delta + WorldPeriod * .5f, WorldPeriod) - WorldPeriod * .5f;

        internal static int ConsumeSteps(ref float remainder, float dt)
        {
            var total = remainder + Mathf.Min(dt, FixedStep * MaximumSteps);
            var count = Mathf.Min(MaximumSteps, Mathf.FloorToInt(total / FixedStep));
            remainder = Mathf.Clamp(total - count * FixedStep, 0f, FixedStep);
            return count;
        }

        internal static int AppendImpulse(Vector4[] queue, int count, Vector4 impulse)
        {
            // Merge nearly coincident stamps, retaining a bounded velocity budget.
            for (var i = 0; i < count; i++)
            {
                var delta = new Vector2(WrappedDelta(queue[i].x - impulse.x), WrappedDelta(queue[i].y - impulse.y));
                if (delta.sqrMagnitude >= .0625f * Mathf.Min(queue[i].z, impulse.z) * Mathf.Min(queue[i].z, impulse.z)) continue;
                queue[i] = new Vector4(queue[i].x, queue[i].y, Mathf.Max(queue[i].z, impulse.z),
                    Mathf.Clamp(queue[i].w + impulse.w, -8f, 8f));
                return count;
            }
            if (count < queue.Length) { queue[count] = impulse; return count + 1; }
            var weakest = 0;
            for (var i = 1; i < count; i++) if (Mathf.Abs(queue[i].w) < Mathf.Abs(queue[weakest].w)) weakest = i;
            if (Mathf.Abs(impulse.w) > Mathf.Abs(queue[weakest].w)) queue[weakest] = impulse;
            return count;
        }

        // Compact, zero-area continuous radial pressure profile. Positive strength
        // depresses the middle and raises an annulus; it does not keep adding water.
        internal static float PressureProfile(float squaredRadiusFraction)
        {
            var u = Mathf.Clamp01(squaredRadiusFraction);
            return -(1f - u) * (1f - 3f * u);
        }

        internal static float DiscretePressure(Vector2 delta, float radius)
        {
            float Potential(Vector2 p)
            {
                var a = Mathf.Clamp01(1f - p.sqrMagnitude / (radius * radius));
                return a * a * a;
            }
            return (Potential(delta + new Vector2(CellSize, 0)) + Potential(delta - new Vector2(CellSize, 0))
                + Potential(delta + new Vector2(0, CellSize)) + Potential(delta - new Vector2(0, CellSize))
                - 4f * Potential(delta)) * radius * radius / (12f * CellSize * CellSize);
        }

        private static bool Finite(float value) => !float.IsNaN(value) && !float.IsInfinity(value);
        private void ThrowIfDisposed() { if (disposed) throw new ObjectDisposedException(nameof(ModernWaterInteractions)); }
        private static void Destroy(UnityEngine.Object value)
        {
            if (value == null) return;
            if (Application.isPlaying) UnityEngine.Object.Destroy(value);
            else UnityEngine.Object.DestroyImmediate(value);
        }
        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            foreach (var texture in new[] { current, previous, scratch })
                if (texture != null) { texture.Release(); Destroy(texture); }
            Destroy(compute);
        }
    }
}
