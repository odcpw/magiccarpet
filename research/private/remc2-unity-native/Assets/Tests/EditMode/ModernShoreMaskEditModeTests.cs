using System;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernShoreMaskEditModeTests
    {
        private static byte[] Atlas()
        {
            var atlas = new byte[256 * 608];
            Array.Fill(atlas, (byte)12);
            // An asymmetric full 8x8 water patch at source UV block (0,1).
            foreach (var type in new[] { 4, 44 })
                for (var v = 8; v < 16; v++) for (var u = 0; u < 8; u++)
                    atlas[(type / 8 * 32 + v) * 256 + type % 8 * 32 + u] = 11;
            return atlas;
        }

        [Test]
        public void SourceUvRowsRotateAndMirrorAsymmetricMaskOnBothCheckerboardCells()
        {
            using var mask = new ModernShoreMask(Atlas());
            var x = new[] { 0, 3, 0, 3, 1, 2, 1, 2 };
            var z = new[] { 1, 1, 2, 2, 0, 0, 3, 3 };
            for (var orientation = 0; orientation < 8; orientation++)
            {
                for (var iz = 0; iz < 4; iz++) for (var ix = 0; ix < 4; ix++)
                    Assert.That(mask.Sample(44, (byte)(orientation << 4), (ix + .5f) / 4, (iz + .5f) / 4),
                        Is.EqualTo(ix == x[orientation] && iz == z[orientation] ? 1f : 0f),
                        $"orientation {orientation}, block {ix},{iz}");
            }
            var heights = new byte[65536]; var types = new byte[65536]; var angles = new byte[65536];
            Array.Fill(types, (byte)1);
            types[0] = types[1] = 44; angles[0] = angles[1] = 0x50;
            mask.Update(heights, types, angles, 1);
            // Adjacent cells have opposite checkerboard diagonals; the affine
            // UV transform gives the same coverage on either triangulation.
            Assert.That(mask.SampleWorld(2.5f, .5f), Is.EqualTo(1));
            Assert.That(mask.SampleWorld(6.5f, .5f), Is.EqualTo(1));
            Assert.That(mask.Texture.GetPixel(2, 0).g, Is.EqualTo(1));
        }

        [Test]
        public void CoverageHonorsMaterialHeightRevisionAndPeriodicFiltering()
        {
            var atlas = Atlas();
            using var mask = new ModernShoreMask(atlas);
            Assert.That(mask.HasCoverage(44), Is.True);
            Assert.That(mask.HasCoverage(4), Is.False, "a low palette index in a nonreflective tile is land");
            Assert.That(mask.HasCoverage(0), Is.True);
            Assert.That(mask.HasCoverage(200), Is.False);
            atlas[(44 / 8 * 32 + 8) * 256 + 44 % 8 * 32] = 255;
            Assert.That(mask.Sample(44, 0, .125f, .375f), Is.EqualTo(1), "constructor retained derived data");
            var heights = new byte[65536]; var types = new byte[65536]; var angles = new byte[65536];
            Array.Fill(types, (byte)1);
            types[0] = 44; types[1] = 0; types[2] = 44; heights[2] = 1;
            angles[3] = 8;
            mask.Update(heights, types, angles, 7);
            Assert.That(mask.SampleWorld(.5f, 1.5f), Is.EqualTo(1));
            Assert.That(mask.SampleWorld(4.5f, .5f), Is.EqualTo(1));
            Assert.That(mask.SampleWorld(8.5f, 1.5f), Is.Zero, "elevated transition is land");
            Assert.That(mask.SampleWorld(12.5f, .5f), Is.EqualTo(1), "explicit source wave bit remains water");
            Assert.That(mask.Texture.GetPixel(0, 1).g, Is.EqualTo(1));
            Assert.That(mask.Texture.GetPixel(8, 1).g, Is.Zero);
            Assert.That(mask.SampleWorld(-.5f, .5f), Is.EqualTo(mask.SampleWorld(1023.5f, .5f)));
            Assert.That(mask.SampleWorld(4f, .5f), Is.EqualTo(mask.SampleWorld(1028f, -1023.5f)));
            types[1] = 1;
            mask.Update(heights, types, angles, 7);
            Assert.That(mask.SampleWorld(4.5f, .5f), Is.EqualTo(1), "same revision does not read caller mutation");
            mask.Update(heights, types, angles, 8);
            Assert.That(mask.SampleWorld(4.5f, .5f), Is.Zero);
            Assert.That(mask.Revision, Is.EqualTo(8));
            Assert.That(mask.Texture.wrapMode, Is.EqualTo(TextureWrapMode.Repeat));
        }

        [Test]
        public void RejectsWrongAtlasAndPlaneLengths()
        {
            Assert.Throws<ArgumentException>(() => new ModernShoreMask(new byte[256]));
            using var mask = new ModernShoreMask(Atlas());
            Assert.Throws<ArgumentException>(() => mask.Update(new byte[1], new byte[65536], new byte[65536], 1));
        }
    }
}
