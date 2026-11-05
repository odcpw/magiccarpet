using System;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernWaterCoastEditModeTests
    {
        [Test]
        public void StripeCoastHasWorldSpaceDistanceDepthAndCorrectSeamGradient()
        {
            var mask = new bool[256 * 256];
            for (var z = 0; z < 256; z++)
            for (var x = 0; x < 256; x++) mask[x + z * 256] = x < 64;
            using var coast = new ModernWaterCoast();
            coast.UpdateMask(mask, 3);
            Assert.That(coast.SampleDistance(2, 200), Is.EqualTo(2).Within(.001));
            Assert.That(coast.SampleDistance(6, 200), Is.EqualTo(6).Within(.001));
            Assert.That(coast.SampleDistance(254, 200), Is.EqualTo(2).Within(.001));
            Assert.That(coast.SampleDistance(258, 200), Is.EqualTo(-2).Within(.001));
            Assert.That(coast.SampleDistance(1022, 200), Is.EqualTo(-2).Within(.001));
            Assert.That(coast.SampleDistance(0, 200), Is.EqualTo(0).Within(.001));
            Assert.That(coast.SampleGradient(0, 200).x, Is.EqualTo(1).Within(.001));
            Assert.That(coast.SampleGradient(256, 200).x, Is.EqualTo(-1).Within(.001));
            Assert.That(coast.SampleDepth(6, 200), Is.EqualTo(2.7).Within(.001));
            Assert.That(coast.SampleDepth(258, 200), Is.Zero);
            Assert.That(coast.Texture.wrapMode, Is.EqualTo(TextureWrapMode.Repeat));
            Assert.That(coast.Texture.format, Is.EqualTo(TextureFormat.RGBAHalf));
            Assert.That(coast.Revision, Is.EqualTo(3));
        }

        [Test]
        public void NearestCellDistancesAreEuclideanAndWrapAcrossBothAxes()
        {
            var mask = new bool[256 * 256];
            Array.Fill(mask, true);
            mask[0] = false;
            using var coast = new ModernWaterCoast();
            coast.UpdateMask(mask, 1);
            Assert.That(coast.SampleDistance(14, 18), Is.EqualTo(18).Within(.001), "3-4-5 centre distance minus half a cell");
            Assert.That(coast.SampleDistance(1014, 1010), Is.EqualTo(18).Within(.001));
            for (var i = 0; i < 32; i++)
            {
                var x = i * 31.75f - 117f; var z = i * 17.25f;
                Assert.That(coast.SampleDistance(x, z), Is.EqualTo(coast.SampleDistance(x + 1024, z - 2048)).Within(.001));
                Assert.That(coast.SampleGradient(x, z), Is.EqualTo(coast.SampleGradient(x + 1024, z - 2048)));
            }
        }

        [Test]
        public void OneCellChannelsAndBarriersRetainSourceClassificationAfterMutation()
        {
            var mask = new bool[256 * 256];
            for (var z = 0; z < 256; z++) mask[256 * z] = true;
            using var coast = new ModernWaterCoast();
            coast.UpdateMask(mask, 1);
            Assert.That(coast.IsWater(2, 402), Is.True);
            Assert.That(coast.SampleDistance(2, 402), Is.EqualTo(2).Within(.001));
            Assert.That(coast.IsWater(6, 402), Is.False);
            Assert.That(coast.IsWater(1022, 402), Is.False);
            // Break the channel with a single-cell obstacle; no source-mask dilation.
            mask[256 * 100] = false;
            coast.UpdateMask(mask, 2);
            Assert.That(coast.IsWater(2, 402), Is.False);
            Assert.That(coast.SampleDistance(2, 402), Is.EqualTo(-2).Within(.001));
            Assert.That(coast.IsWater(2, 398), Is.True);
            Assert.That(coast.IsWater(2, 406), Is.True);
            // Authored caller mutation alone cannot silently mutate an uploaded revision.
            mask[256 * 100] = true;
            coast.UpdateMask(mask, 2);
            Assert.That(coast.IsWater(2, 402), Is.False);
            coast.UpdateMask(mask, 3);
            Assert.That(coast.IsWater(2, 402), Is.True);
        }

        [Test]
        public void UniformMasksHaveFiniteBoundedDistanceAndNoInventedCoast()
        {
            var mask = new bool[256 * 256];
            using var coast = new ModernWaterCoast();
            coast.UpdateMask(mask, 1);
            Assert.That(coast.SampleDistance(900, 12), Is.EqualTo(-ModernWaterCoast.MaximumDistance));
            Assert.That(coast.SampleDepth(900, 12), Is.Zero);
            Assert.That(coast.SampleGradient(900, 12), Is.EqualTo(Vector2.zero));
            Array.Fill(mask, true);
            coast.UpdateMask(mask, 2);
            Assert.That(coast.SampleDistance(900, 12), Is.EqualTo(ModernWaterCoast.MaximumDistance));
            Assert.That(coast.SampleDepth(900, 12), Is.EqualTo(24));
            Assert.That(coast.SampleGradient(900, 12), Is.EqualTo(Vector2.zero));
        }

        [Test]
        public void TerrainRevisionRefreshesTheActualMaskAndDoesNotTouchSourceArrays()
        {
            var heights = new byte[256 * 256];
            var types = new byte[heights.Length];
            var angles = new byte[heights.Length];
            angles[255 + 255 * 256] = 8;
            var root = new GameObject("Water coast source test");
            var material = new Material(Shader.Find("Hidden/InternalErrorShader"));
            using var coast = new ModernWaterCoast();
            using var terrain = new ModernTerrainMesh(root.transform, material, material);
            try
            {
                terrain.Update(heights, types, angles, 7, 1);
                coast.Update(terrain);
                Assert.That(coast.IsWater(-2, -2), Is.True);
                Assert.That(coast.IsWater(2, 2), Is.False);
                var firstRevision = coast.Revision;
                angles[0] = 8;
                terrain.Update(heights, types, angles, 7, 2);
                coast.Update(terrain);
                Assert.That(coast.Revision, Is.GreaterThan(firstRevision));
                Assert.That(coast.IsWater(2, 2), Is.True);
                Assert.That(coast.Revision, Is.EqualTo(terrain.Revision));
                Assert.That(angles[255 + 255 * 256], Is.EqualTo(8));
                Assert.That(heights[0], Is.Zero);
            }
            finally
            {
                terrain.Dispose();
                UnityEngine.Object.DestroyImmediate(root);
                UnityEngine.Object.DestroyImmediate(material);
            }
        }
    }
}
