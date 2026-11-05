using System;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native
{
    // Presentation-only wind sea. Two disjoint frequency bands share stable Gaussian
    // phases while their amplitudes relax toward the current wind spectrum. No readback
    // or gameplay authority. The FFT is evaluated once, independently of render cameras.
    internal sealed class ModernWaterSpectrum : IDisposable
    {
        internal const int Resolution = 256;
        internal const float Period = 256f;
        internal const float DetailPeriod = 32f;
        internal const float MaximumDisplacement = 3f;
        internal const uint BroadSeed = 0x721ea92du;
        internal const uint DetailSeed = 0x58e217b3u;
        internal RenderTexture Displacement => broad.Displacement;
        internal RenderTexture NormalFoam => broad.NormalFoam;
        internal RenderTexture DetailDisplacement => detail.Displacement;
        internal RenderTexture DetailNormalFoam => detail.NormalFoam;

        private sealed class Band : IDisposable
        {
            internal readonly float Period;
            internal readonly int Detail;
            internal readonly Texture2D Gaussian;
            internal readonly RenderTexture Amplitude, Spectrum, Scratch, Spatial;
            internal readonly RenderTexture Displacement, NormalFoam;
            internal bool Initialized;

            internal Band(float period, bool detail, uint seed)
            {
                Period = period;
                Detail = detail ? 1 : 0;
                try
                {
                    Gaussian = new Texture2D(Resolution, Resolution, TextureFormat.RGFloat, false, true) {
                        name = "Water fixed Gaussian phases", filterMode = FilterMode.Point,
                        wrapMode = TextureWrapMode.Repeat
                    };
                    Gaussian.SetPixelData(GaussianLattice(seed), 0);
                    Gaussian.Apply(false, true);
                    Amplitude = Texture("Water relaxed amplitudes", RenderTextureFormat.RFloat);
                    Spectrum = Texture("Water spectrum", RenderTextureFormat.RGFloat, 3);
                    Scratch = Texture("Water FFT rows", RenderTextureFormat.RGFloat, 3);
                    Spatial = Texture("Water FFT columns", RenderTextureFormat.RGFloat, 3);
                    Displacement = Texture("Water displacement", RenderTextureFormat.ARGBHalf, 1, true);
                    NormalFoam = Texture("Water slopes and breaking", RenderTextureFormat.ARGBHalf, 1, true);
                }
                catch { Dispose(); throw; }
            }

            public void Dispose()
            {
                Destroy(Gaussian);
                foreach (var texture in new[] { Amplitude, Spectrum, Scratch, Spatial, Displacement, NormalFoam })
                {
                    if (texture != null) texture.Release();
                    Destroy(texture);
                }
            }
        }

        private readonly ComputeShader shader;
        private readonly Band broad, detail;
        private readonly int amplitudes, evolve, rows, columns, assemble;
        private bool disposed;

        internal ModernWaterSpectrum()
        {
            if (!SystemInfo.supportsComputeShaders || !SystemInfo.supports2DArrayTextures)
                throw new NotSupportedException("Modern spectral water requires compute and texture arrays");
            var source = Resources.Load<ComputeShader>("ModernWaterSpectrum");
            if (source == null) throw new InvalidOperationException("ModernWaterSpectrum compute shader is missing");
            try
            {
                shader = UnityEngine.Object.Instantiate(source);
                amplitudes = shader.FindKernel("UpdateAmplitudes");
                evolve = shader.FindKernel("EvolveSpectrum");
                rows = shader.FindKernel("InverseRows");
                columns = shader.FindKernel("InverseColumns");
                assemble = shader.FindKernel("Assemble");
                broad = new Band(Period, false, BroadSeed);
                detail = new Band(DetailPeriod, true, DetailSeed);
            }
            catch { Dispose(); throw; }
        }

        internal void Update(float time, Vector2 windVelocity, float dt)
        {
            if (disposed) throw new ObjectDisposedException(nameof(ModernWaterSpectrum));
            if (!Finite(time) || !Finite(dt) || !Finite(windVelocity.x) || !Finite(windVelocity.y))
                throw new ArgumentException("Water spectrum time and wind must be finite");
            shader.SetFloat("_TimeSeconds", time);
            shader.SetFloat("_DeltaSeconds", Mathf.Clamp(dt, 0f, .25f));
            shader.SetVector("_Wind", new Vector4(windVelocity.x, windVelocity.y, 0f, 0f));
            shader.SetFloat("_MaximumDisplacement", MaximumDisplacement);
            UpdateBand(broad);
            UpdateBand(detail);
        }

        private void UpdateBand(Band band)
        {
            shader.SetFloat("_Period", band.Period);
            shader.SetInt("_Detail", band.Detail);
            shader.SetInt("_Initialize", band.Initialized ? 0 : 1);
            shader.SetTexture(amplitudes, "_Amplitude", band.Amplitude);
            shader.Dispatch(amplitudes, Resolution / 8, Resolution / 8, 1);
            shader.SetTexture(evolve, "_Gaussian", band.Gaussian);
            shader.SetTexture(evolve, "_Amplitude", band.Amplitude);
            shader.SetTexture(evolve, "_Spectrum", band.Spectrum);
            shader.Dispatch(evolve, Resolution / 8, Resolution / 8, 1);
            shader.SetTexture(rows, "_TransformInput", band.Spectrum);
            shader.SetTexture(rows, "_TransformOutput", band.Scratch);
            shader.Dispatch(rows, Resolution, 1, 3);
            shader.SetTexture(columns, "_TransformInput", band.Scratch);
            shader.SetTexture(columns, "_TransformOutput", band.Spatial);
            shader.Dispatch(columns, Resolution, 1, 3);
            shader.SetTexture(assemble, "_TransformInput", band.Spatial);
            shader.SetTexture(assemble, "_Displacement", band.Displacement);
            shader.SetTexture(assemble, "_NormalFoam", band.NormalFoam);
            shader.Dispatch(assemble, Resolution / 8, Resolution / 8, 1);
            band.Displacement.GenerateMips();
            band.NormalFoam.GenerateMips();
            band.Initialized = true;
        }

        private static RenderTexture Texture(string name, RenderTextureFormat format, int layers = 1, bool mips = false)
        {
            var texture = new RenderTexture(Resolution, Resolution, 0, format, RenderTextureReadWrite.Linear) {
                name = name, enableRandomWrite = true, useMipMap = mips, autoGenerateMips = false,
                wrapMode = TextureWrapMode.Repeat, filterMode = mips ? FilterMode.Trilinear : FilterMode.Point,
                dimension = layers > 1 ? TextureDimension.Tex2DArray : TextureDimension.Tex2D,
                volumeDepth = layers
            };
            if (!texture.Create()) { Destroy(texture); throw new InvalidOperationException("Cannot allocate " + name); }
            return texture;
        }

        // Box-Muller phases are independent of Unity's global RNG and never regenerated
        // on a wind change. Spectrum amplitudes use the same reference equation as GPU.
        internal static Vector2[] GaussianLattice(uint seed)
        {
            var values = new Vector2[Resolution * Resolution];
            for (var i = 0; i < values.Length; i++)
            {
                var u = (Hash(seed + (uint)i * 2u) + .5) / 4294967296.0;
                var v = (Hash(seed + (uint)i * 2u + 1u) + .5) / 4294967296.0;
                var radius = Math.Sqrt(-2.0 * Math.Log(u));
                values[i] = new Vector2((float)(radius * Math.Cos(2.0 * Math.PI * v)),
                    (float)(radius * Math.Sin(2.0 * Math.PI * v)));
            }
            return values;
        }

        private static uint Hash(uint value)
        {
            unchecked { value ^= value >> 16; value *= 0x7feb352du; value ^= value >> 15;
                value *= 0x846ca68bu; return value ^ (value >> 16); }
        }

        internal static float SpectralAmplitude(int x, int z, float period, Vector2 wind, bool detail)
        {
            if (x == Resolution / 2 || z == Resolution / 2) return 0f;
            var k = new Vector2(x < Resolution / 2 ? x : x - Resolution,
                z < Resolution / 2 ? z : z - Resolution) * (2f * Mathf.PI / period);
            var magnitude = k.magnitude;
            if (magnitude < .0001f) return 0f;
            var speed = Mathf.Min(wind.magnitude, 18f);
            var direction = wind.magnitude > .01f ? wind.normalized : Vector2.right;
            var omega = Mathf.Sqrt(9.81f * magnitude);
            var peak = 9.81f * 1.05f / Mathf.Max(speed, 2f);
            var sigma = omega <= peak ? .07f : .09f;
            var peakShape = Mathf.Exp(-.5f * Mathf.Pow((omega - peak) / (sigma * peak), 2));
            var density = .0081f * 9.81f * 9.81f / Mathf.Pow(omega, 5)
                * Mathf.Exp(-1.25f * Mathf.Pow(peak / omega, 4)) * Mathf.Pow(3.3f, peakShape);
            var directional = Mathf.Pow(Mathf.Max(0f, Vector2.Dot(k / magnitude, direction)), 4) * (8f / (3f * Mathf.PI));
            var wavelength = 2f * Mathf.PI / magnitude;
            var broadWeight = Mathf.SmoothStep(0f, 1f, Mathf.InverseLerp(16f, 24f, wavelength));
            var bandWeight = detail ? 1f - broadWeight : broadWeight;
            // Suppress below 4 texels/wavelength in the detail band. Broad λ>=16m.
            var resolutionWeight = Mathf.SmoothStep(0f, 1f, Mathf.InverseLerp(2f, 4f, wavelength / (period / Resolution)));
            var dk = 2f * Mathf.PI / period;
            var variance = density * directional * (.5f * 9.81f / omega) / magnitude * dk * dk;
            return .5f * Mathf.Sqrt(Mathf.Max(0f, variance * bandWeight * resolutionWeight)) * Mathf.Clamp01(speed / 2f);
        }

        private static bool Finite(float value) => !float.IsNaN(value) && !float.IsInfinity(value);
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
            broad?.Dispose(); detail?.Dispose(); Destroy(shader);
        }
    }
}
