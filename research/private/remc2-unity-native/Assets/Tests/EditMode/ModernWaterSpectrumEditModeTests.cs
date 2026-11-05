using System;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernWaterSpectrumEditModeTests
    {
        [Test]
        public void SpectrumPartitionsEnergyAndKeepsMapPeriod()
        {
            Assert.That(1024f % ModernWaterSpectrum.Period, Is.Zero);
            Assert.That(1024f % ModernWaterSpectrum.DetailPeriod, Is.Zero);
            var wind = new Vector2(8f, 3f);
            double broadVariance = 0, shortVariance = 0;
            for (var z = 0; z < ModernWaterSpectrum.Resolution; z++)
            for (var x = 0; x < ModernWaterSpectrum.Resolution; x++)
            {
                var broad = ModernWaterSpectrum.SpectralAmplitude(x, z, ModernWaterSpectrum.Period, wind, false);
                var detail = ModernWaterSpectrum.SpectralAmplitude(x, z, ModernWaterSpectrum.DetailPeriod, wind, true);
                Assert.That(float.IsNaN(broad) || float.IsInfinity(broad) || broad < 0f, Is.False);
                Assert.That(float.IsNaN(detail) || float.IsInfinity(detail) || detail < 0f, Is.False);
                broadVariance += 4 * broad * broad;
                shortVariance += 4 * detail * detail;
                if (x == 128 || z == 128) Assert.That(broad + detail, Is.Zero, "Nyquist displacement must preserve Hermitian symmetry");
            }
            Assert.That(broadVariance, Is.InRange(.005, 2.0), "physical Fourier amplitudes must not accidentally acquire an N² scale");
            Assert.That(shortVariance, Is.InRange(.00001, .5));
            Assert.That(ModernWaterSpectrum.SpectralAmplitude(0, 0, 256f, wind, false), Is.Zero);
            Assert.That(ModernWaterSpectrum.SpectralAmplitude(1, 0, 256f, Vector2.zero, false), Is.Zero);
            // Same physical k on both grids: amplitude²/dk² adds to the same
            // unsplit directional density rather than duplicating full energy.
            var wide = ModernWaterSpectrum.SpectralAmplitude(8, 8, 256f, wind, false);
            var narrow = ModernWaterSpectrum.SpectralAmplitude(1, 1, 32f, wind, true);
            Assert.That(wide, Is.GreaterThan(0));
            Assert.That(narrow, Is.GreaterThan(0));
            Assert.That(ModernWaterSpectrum.SpectralAmplitude(32, 0, 256f, wind, false), Is.Zero,
                "8m waves must not displace the coarse terrain water mesh");
        }

        [Test]
        public void GaussianPhasesAreDeterministicAndDoNotTouchUnityRandom()
        {
            var state = UnityEngine.Random.state;
            var first = ModernWaterSpectrum.GaussianLattice(ModernWaterSpectrum.BroadSeed);
            var second = ModernWaterSpectrum.GaussianLattice(ModernWaterSpectrum.BroadSeed);
            Assert.That(UnityEngine.Random.state, Is.EqualTo(state));
            CollectionAssert.AreEqual(first, second);
            double mean = 0, energy = 0;
            foreach (var value in first) { mean += value.x + value.y; energy += value.sqrMagnitude; }
            Assert.That(mean / (first.Length * 2), Is.EqualTo(0).Within(.015));
            Assert.That(energy / (first.Length * 2), Is.EqualTo(1).Within(.025));
        }

        [Test]
        public void GpuInverseTransformMatchesIndependentDirectFourierSum()
        {
            Assert.That(SystemInfo.supportsComputeShaders, Is.True, "This integration check needs the actual compute backend");
            using var spectrum = new ModernWaterSpectrum();
            var wind = new Vector2(7f, 3f);
            const float time = 1.25f;
            spectrum.Update(time, wind, 0f);
            var broad = Read(spectrum.Displacement);
            var detail = Read(spectrum.DetailDisplacement);
            foreach (var pixel in new[] { new Vector2Int(0, 0), new Vector2Int(11, 27), new Vector2Int(93, 151) })
            {
                CheckDirectSum(broad, pixel, ModernWaterSpectrum.Period, false, ModernWaterSpectrum.BroadSeed, wind, time);
                CheckDirectSum(detail, pixel, ModernWaterSpectrum.DetailPeriod, true, ModernWaterSpectrum.DetailSeed, wind, time);
            }
            spectrum.Update(time, new Vector2(-9f, 1f), 0f);
            var unchanged = Read(spectrum.Displacement);
            for (var i = 0; i < broad.Length; i += 137)
                Assert.That(unchanged[i], Is.EqualTo(broad[i]), "a zero-dt wind change cannot reseed phases or jump amplitudes");
            spectrum.Update(time + .5f, wind, .016f);
            var moved = Read(spectrum.Displacement);
            double motion = 0;
            for (var i = 0; i < moved.Length; i += 137) motion += Math.Abs(moved[i].g - broad[i].g);
            Assert.That(motion, Is.GreaterThan(.1), "spectral waves must actually travel");
            foreach (var color in Read(spectrum.NormalFoam))
            {
                Assert.That(float.IsNaN(color.r) || float.IsNaN(color.g) || float.IsInfinity(color.r) || float.IsInfinity(color.g), Is.False);
                Assert.That(color.b, Is.InRange(0f, 1f));
            }
        }

        private static void CheckDirectSum(Color[] actual, Vector2Int pixel, float period, bool detail, uint seed, Vector2 wind, float time)
        {
            var gaussian = ModernWaterSpectrum.GaussianLattice(seed);
            double height = 0, dx = 0, dz = 0;
            const int n = ModernWaterSpectrum.Resolution;
            for (var z = 0; z < n; z++)
            for (var x = 0; x < n; x++)
            {
                var amplitude = ModernWaterSpectrum.SpectralAmplitude(x, z, period, wind, detail);
                if (amplitude == 0) continue;
                var kx = (x < n / 2 ? x : x - n) * (2 * Math.PI / period);
                var kz = (z < n / 2 ? z : z - n) * (2 * Math.PI / period);
                var k = Math.Sqrt(kx * kx + kz * kz);
                var phase = 2 * Math.PI * ((x < n / 2 ? x : x - n) * pixel.x +
                    (z < n / 2 ? z : z - n) * pixel.y) / n - Math.Sqrt(9.81 * k) * time;
                var g = gaussian[x + z * n];
                height += 2 * amplitude * (g.x * Math.Cos(phase) - g.y * Math.Sin(phase));
                var horizontal = -2 * .82 * amplitude * (g.x * Math.Sin(phase) + g.y * Math.Cos(phase));
                dx += horizontal * kx / k;
                dz += horizontal * kz / k;
            }
            const float bound = ModernWaterSpectrum.MaximumDisplacement;
            var value = actual[pixel.x + pixel.y * n];
            Assert.That(value.r, Is.EqualTo(bound * Math.Tanh(dx / bound)).Within(.003), "FFT X displacement");
            Assert.That(value.g, Is.EqualTo(bound * Math.Tanh(height / bound)).Within(.003), "FFT height / normalization");
            Assert.That(value.b, Is.EqualTo(bound * Math.Tanh(dz / bound)).Within(.003), "FFT Z displacement");
        }

        private static Color[] Read(RenderTexture texture)
        {
            var previous = RenderTexture.active;
            var copy = new Texture2D(texture.width, texture.height, TextureFormat.RGBAFloat, false, true);
            try
            {
                RenderTexture.active = texture;
                copy.ReadPixels(new Rect(0, 0, texture.width, texture.height), 0, 0);
                copy.Apply(false);
                return copy.GetPixels();
            }
            finally { RenderTexture.active = previous; UnityEngine.Object.DestroyImmediate(copy); }
        }
    }
}
