using System;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native
{
    // Four condensate channels share one spatial field and one ray march. No velocity
    // grid, pressure solver, gameplay force, or camera-dependent density reset.
    internal sealed class ModernSteamVolume : IDisposable
    {
        internal const float Size = 12f;
        internal const int Capacity = 4;
        private const int WakeCapacity = 16;
        private const float Step = 1f / 30f;
        private struct Parcel { internal Vector3 Position; internal float Age, Water, Liquid; internal bool Active; }
        private struct Wake { internal Vector3 From, To; internal float Age, Speed; }
        private readonly Parcel[] parcels = new Parcel[Capacity];
        private readonly Wake[] wakes = new Wake[WakeCapacity];
        private readonly Vector4[] sources = new Vector4[Capacity], wakeFrom = new Vector4[WakeCapacity], wakeTo = new Vector4[WakeCapacity];
        private readonly ComputeShader compute;
        private readonly Material material;
        private readonly GameObject node;
        private readonly MeshRenderer renderer;
        private readonly int clearKernel, predictKernel, advanceKernel, limitKernel, normalizeKernel, resolution;
        private readonly ComputeBuffer massParts, massScale;
        private readonly bool enabled = Array.IndexOf(Environment.GetCommandLineArgs(), "-mc-steam-off") < 0;
        private RenderTexture current, scratch, predicted;
        private Vector3 origin;
        private Vector4 resetChannels;
        private int nextParcel, nextWake;
        private float remainder, clock;
        private bool initialized, disposed;
        internal RenderTexture State => current;
        internal Vector3 Origin => origin;
        internal Vector3 RenderOrigin => node.transform.position - Vector3.one*(Size*.5f);
        internal Vector3? LatestCenter => parcels[(nextParcel + Capacity - 1) % Capacity].Active
            ? parcels[(nextParcel + Capacity - 1) % Capacity].Position : (Vector3?)null;

        internal ModernSteamVolume(Transform parent, int gridResolution = 48)
        {
            if (gridResolution != 32 && gridResolution != 48 && gridResolution != 64)
                throw new ArgumentOutOfRangeException(nameof(gridResolution));
            if (!SystemInfo.supportsComputeShaders || !SystemInfo.supports3DTextures
                || !SystemInfo.SupportsRenderTextureFormat(RenderTextureFormat.ARGBFloat))
                throw new NotSupportedException("Steam requires 3D floating-point compute textures");
            resolution = gridResolution;
            var source = Resources.Load<ComputeShader>("ModernSteamVolume");
            var shader = Resources.Load<Shader>("ModernSteamVolume");
            if (source == null || shader == null) throw new InvalidOperationException("Missing steam volume resources");
            compute = UnityEngine.Object.Instantiate(source);
            material = new Material(shader);
            try
            {
                clearKernel = compute.FindKernel("Clear"); predictKernel = compute.FindKernel("Predict"); advanceKernel = compute.FindKernel("Advance");
                limitKernel = compute.FindKernel("LimitMass"); normalizeKernel = compute.FindKernel("Normalize");
                var groups = resolution / 4;
                massParts = new ComputeBuffer(groups*groups*groups*2, 16);
                massScale = new ComputeBuffer(1,16);
                compute.SetInt("_GroupCount",groups*groups*groups);
                compute.SetBuffer(advanceKernel,"_MassParts",massParts);
                compute.SetBuffer(limitKernel,"_MassParts",massParts);
                compute.SetBuffer(limitKernel,"_MassScale",massScale);
                compute.SetBuffer(normalizeKernel,"_MassScale",massScale);
                current = Create("Steam condensate current"); scratch = Create("Steam condensate next"); predicted = Create("Steam transport predictor");
                compute.SetInt("_Resolution", resolution);
                node = GameObject.CreatePrimitive(PrimitiveType.Cube);
                node.name = "Persistent steam volume";
                node.transform.SetParent(parent, false);
                node.transform.localScale = Vector3.one * Size;
                var collider = node.GetComponent<Collider>(); collider.enabled = false; Destroy(collider);
                renderer = node.GetComponent<MeshRenderer>(); renderer.sharedMaterial = material;
                renderer.shadowCastingMode = ShadowCastingMode.Off; renderer.receiveShadows = false;
                renderer.lightProbeUsage = LightProbeUsage.Off;
                Reset();
            }
            catch { Dispose(); throw; }
        }

        private RenderTexture Create(string label)
        {
            var texture = new RenderTexture(resolution, resolution, 0, RenderTextureFormat.ARGBFloat,
                RenderTextureReadWrite.Linear) { name = label, dimension = TextureDimension.Tex3D,
                volumeDepth = resolution, enableRandomWrite = true, filterMode = FilterMode.Bilinear,
                wrapMode = TextureWrapMode.Clamp, useMipMap = false, autoGenerateMips = false };
            if (texture.Create()) return texture;
            Destroy(texture); throw new InvalidOperationException("Cannot allocate steam density texture");
        }

        internal void Quench(Vector3 position, ModernFireballEnergy energy)
        {
            if (!Finite(position) || energy.VaporizedKilograms <= 0) return;
            // Keep sources in one unwrapped coordinate frame, including a world seam.
            if (initialized) position = Nearest(position, origin + Vector3.one * (Size * .5f));
            parcels[nextParcel] = new Parcel { Position = position + Vector3.up * .45f,
                Water = (float)energy.VaporizedKilograms, Active = true };
            resetChannels[nextParcel] = 1;
            nextParcel = (nextParcel + 1) % Capacity;
        }

        internal void Disturb(Vector3 from, Vector3 to, float dt)
        {
            if (!initialized || !Finite(from) || !Finite(to) || !float.IsFinite(dt) || dt <= 0 || dt > 1) return;
            from = Nearest(from, origin + Vector3.one * (Size * .5f)); to = Nearest(to, from);
            var distance = Vector3.Distance(from, to);
            if (distance < .015f || distance > 64 || distance / dt > 120) return;
            // Ignore sweeps wholly outside the finite domain.
            var closest = Vector3.Min(Vector3.Max((from + to) * .5f, origin), origin + Vector3.one * Size);
            if (Vector3.Distance(closest, (from + to) * .5f) > distance * .5f + 2) return;
            // Merge adjacent observations rather than making wake strength frame-rate dependent.
            var previous = (nextWake + WakeCapacity - 1) % WakeCapacity;
            ref var old = ref wakes[previous];
            if (old.Speed > 0 && old.Age < .2f && Vector3.Distance(old.To, from) < .5f
                && Vector3.Distance(old.From, to) < 2.5f)
            { old.To = to; old.Speed = Mathf.Max(old.Speed, Mathf.Min(12, distance / dt)); return; }
            wakes[nextWake] = new Wake { From = from, To = to, Speed = Mathf.Min(12, distance / dt) };
            nextWake = (nextWake + 1) % WakeCapacity;
        }

        internal void Update(float dt, Vector2 wind)
        {
            if (disposed) throw new ObjectDisposedException(nameof(ModernSteamVolume));
            if (!float.IsFinite(dt) || dt < 0 || !float.IsFinite(wind.x) || !float.IsFinite(wind.y))
                throw new ArgumentException("Steam update needs finite nonnegative time and wind");
            remainder += Mathf.Min(dt, Step * 3);
            while (remainder >= Step) { Advance(wind); remainder -= Step; }
        }

        private void Advance(Vector2 wind)
        {
            clock += Step;
            var injection = Vector4.zero; var retention = Vector4.one - resetChannels;
            resetChannels = Vector4.zero;
            var active = false; var focus = Vector3.zero; var weight = 0f;
            for (var i = 0; i < Capacity; i++)
            {
                ref var parcel = ref parcels[i];
                if (!parcel.Active) { retention[i] = 0; sources[i] = Vector4.zero; continue; }
                parcel.Age += Step;
                var state = ModernSteamPlume.Evaluate(parcel.Water, parcel.Age, 12, .75);
                var liquid = (float)state.CondensedKg;
                // Finite review lifetime; evaporation removes density, never restores escaped mass.
                liquid *= 1 - Mathf.SmoothStep(0, 1, Mathf.InverseLerp(30, 40, parcel.Age));
                // Source support follows the same authored carrier field as density.
                // A separate parcel-rise estimate previously made the review camera
                // track below the actual transported cloud.
                var carrier = Carrier(parcel.Position, wind, clock);
                parcel.Position += Carrier(parcel.Position + carrier*(Step*.5f), wind, clock) * Step;
                parcel.Position.y = Mathf.Max(.45f, parcel.Position.y);
                if (liquid > parcel.Liquid) injection[i] = liquid - parcel.Liquid;
                else if (parcel.Liquid > 0) retention[i] *= liquid / parcel.Liquid;
                parcel.Liquid = liquid;
                if (parcel.Age >= 40) { parcel.Active = false; retention[i] = 0; continue; }
                var sigma = Mathf.Lerp(.32f, .75f, Mathf.Clamp01(parcel.Age / 5));
                sources[i] = new Vector4(parcel.Position.x, parcel.Position.y, parcel.Position.z, sigma);
                var w = Mathf.Max(.01f, liquid); focus += parcel.Position * w; weight += w; active = true;
            }
            renderer.enabled = active && enabled;
            if (!active) return;
            focus /= weight;
            var cell = Size / resolution;
            var nextOrigin = new Vector3(Mathf.Floor((focus.x - Size * .5f) / cell) * cell,
                Mathf.Floor((focus.y - Size * .5f) / cell) * cell, Mathf.Floor((focus.z - Size * .5f) / cell) * cell);
            if (!initialized) { origin = nextOrigin; initialized = true; }
            // A/B control retains identical parcel centers and review-camera motion.
            if (!enabled) { origin = nextOrigin; return; }
            var wakeCount = 0;
            for (var i = 0; i < WakeCapacity; i++)
            {
                ref var wake = ref wakes[i]; wake.Age += Step;
                if (wake.Speed <= 0 || wake.Age > 3) continue;
                wake.From += new Vector3(wind.x, .25f, wind.y) * Step;
                wake.To += new Vector3(wind.x, .25f, wind.y) * Step;
                wakeFrom[wakeCount] = new Vector4(wake.From.x, wake.From.y, wake.From.z, wake.Speed * Mathf.Exp(-wake.Age * 1.2f));
                wakeTo[wakeCount++] = new Vector4(wake.To.x, wake.To.y, wake.To.z, 1.5f);
            }
            compute.SetVector("_OldOrigin", origin); compute.SetVector("_Origin", nextOrigin);
            compute.SetVector("_Wind", new Vector4(wind.x, .65f, wind.y, clock));
            compute.SetVector("_Injection", injection); compute.SetVector("_Retention", retention);
            compute.SetVectorArray("_Sources", sources);
            compute.SetVectorArray("_WakeFrom", wakeFrom); compute.SetVectorArray("_WakeTo", wakeTo);
            compute.SetInt("_WakeCount", wakeCount); compute.SetFloat("_Step", Step); compute.SetFloat("_Size", Size);
            compute.SetTexture(predictKernel, "_Read", current); compute.SetTexture(predictKernel, "_Write", predicted);
            compute.Dispatch(predictKernel, resolution / 4, resolution / 4, resolution / 4);
            compute.SetTexture(advanceKernel, "_Read", current); compute.SetTexture(advanceKernel, "_Predicted", predicted);
            compute.SetTexture(advanceKernel, "_Write", scratch);
            compute.Dispatch(advanceKernel, resolution / 4, resolution / 4, resolution / 4);
            compute.Dispatch(limitKernel,1,1,1);
            compute.SetTexture(normalizeKernel,"_Write",scratch);
            compute.Dispatch(normalizeKernel,resolution/4,resolution/4,resolution/4);
            (current, scratch) = (scratch, current); origin = nextOrigin;
            PlaceForView(origin + Vector3.one*(Size*.5f));
            material.SetTexture("_Density", current);
            material.SetFloat("_Clock", clock);
        }

        internal void PlaceForView(Vector3 viewer)
        {
            if (!initialized || !Finite(viewer)) return;
            var center=Nearest(origin+Vector3.one*(Size*.5f),viewer);
            node.transform.position=center;
            var displayOrigin=center-Vector3.one*(Size*.5f);
            material.SetVector("_Volume",new Vector4(displayOrigin.x,displayOrigin.y,displayOrigin.z,Size));
        }

        internal void Reset()
        {
            Array.Clear(parcels, 0, parcels.Length); Array.Clear(wakes, 0, wakes.Length);
            remainder = clock = 0; nextParcel = nextWake = 0; resetChannels = Vector4.zero; initialized = false;
            foreach (var texture in new[] { current, scratch, predicted })
            { compute.SetTexture(clearKernel, "_Write", texture); compute.Dispatch(clearKernel, resolution / 4, resolution / 4, resolution / 4); }
            if (renderer != null) renderer.enabled = false;
        }
        private static Vector3 Nearest(Vector3 p, Vector3 reference) => new Vector3(
            reference.x + ModernWaterInteractions.WrappedDelta(p.x - reference.x), p.y,
            reference.z + ModernWaterInteractions.WrappedDelta(p.z - reference.z));
        private static Vector3 Carrier(Vector3 p, Vector2 wind, float t) => new Vector3(wind.x,.65f,wind.y)
            + .55f*new Vector3(Mathf.Sin(p.y*1.7f+t*.7f)+Mathf.Cos(p.z*1.3f-t*.4f),
                Mathf.Sin(p.z*1.6f+t*.5f)+Mathf.Cos(p.x*1.4f+t*.3f),Mathf.Sin(p.x*1.8f-t*.6f)+Mathf.Cos(p.y*1.5f+t*.4f))
            + .18f*new Vector3(Mathf.Sin(p.y*3.7f-t*.8f),Mathf.Cos(p.z*3.3f+t*.7f),Mathf.Sin(p.x*3.6f+t*.6f));
        private static bool Finite(Vector3 p) => float.IsFinite(p.x) && float.IsFinite(p.y) && float.IsFinite(p.z);
        private static void Destroy(UnityEngine.Object o)
        { if (o == null) return; if (Application.isPlaying) UnityEngine.Object.Destroy(o); else UnityEngine.Object.DestroyImmediate(o); }
        public void Dispose()
        {
            if (disposed) return; disposed = true;
            foreach (var texture in new[] { current, scratch, predicted }) if (texture != null) { texture.Release(); Destroy(texture); }
            massParts?.Dispose(); massScale?.Dispose();
            Destroy(node); Destroy(material); Destroy(compute);
        }
    }
}
