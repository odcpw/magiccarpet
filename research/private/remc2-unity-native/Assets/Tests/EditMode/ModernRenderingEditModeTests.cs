using NUnit.Framework;
using UnityEngine;
using UnityEngine.Rendering.Universal;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernRenderingEditModeTests
    {
        [Test]
        public void CloudLatticeRetainsSignedShaderCornersWithinSixteenBitQuantization()
        {
            var lattice = ModernCloudNoise.Lattice();
            const int side = ModernCloudNoise.Size;
            Assert.That(lattice.Length, Is.EqualTo(side * side * side));
            // The shader samples (integer + smooth fraction + 64.5) / 128.
            // Verify all lattice corners including negative coordinates and
            // the x-fastest texture upload order, not just a constant fixture.
            double maximumError = 0;
            for (var z = 0; z < side; z++)
            for (var y = 0; y < side; y++)
            for (var x = 0; x < side; x++)
            {
                var stored = lattice[x + side * (y + side * z)] / 65535.0;
                var expected = ModernCloudNoise.Corner(x - side / 2, y - side / 2, z - side / 2);
                maximumError = System.Math.Max(maximumError, System.Math.Abs(stored - expected));
            }
            Assert.That(maximumError, Is.LessThanOrEqualTo(.5 / 65535.0 + 1e-10));
            Assert.That(lattice[0], Is.Not.EqualTo(lattice[side * side / 2]));
        }

        [Test]
        public void ReflectionCameraRetainsSmokeDepthButOmitsUnusedColorAndShadowPass()
        {
            var root = new GameObject("Reflection budget test");
            var water = new Material(Shader.Find("MagicCarpet/ModernWater"));
            var reflection = new ModernWaterReflection(root.transform, water);
            try
            {
                var camera = root.GetComponentInChildren<Camera>();
                var data = camera.GetUniversalAdditionalCameraData();
                Assert.That(camera.enabled, Is.False);
                Assert.That(data.requiresDepthTexture, Is.True, "reflected soft smoke consumes scene depth");
                Assert.That(data.requiresColorTexture, Is.False);
                Assert.That(data.renderShadows, Is.False);
                Assert.That(data.renderPostProcessing, Is.False);
            }
            finally
            {
                reflection.Dispose();
                Object.DestroyImmediate(root);
                Object.DestroyImmediate(water);
            }
        }
    }
}
