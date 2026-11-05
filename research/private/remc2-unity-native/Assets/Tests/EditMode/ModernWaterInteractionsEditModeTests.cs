using NUnit.Framework;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernWaterInteractionsEditModeTests
    {
        [Test]
        public void FixedStepIsStableAndCatchUpIsBounded()
        {
            var courant = ModernWaterInteractions.WaveSpeed * ModernWaterInteractions.FixedStep / ModernWaterInteractions.CellSize;
            Assert.That(courant * courant * 2f, Is.LessThan(1f));
            var remainder = 0f;
            var steps = 0;
            for (var i = 0; i < 120; i++) steps += ModernWaterInteractions.ConsumeSteps(ref remainder, 1f / 120f);
            Assert.That(steps, Is.EqualTo(60));
            Assert.That(remainder, Is.LessThan(ModernWaterInteractions.FixedStep));
            Assert.That(ModernWaterInteractions.ConsumeSteps(ref remainder, 5f), Is.EqualTo(ModernWaterInteractions.MaximumSteps));
            Assert.That(remainder, Is.LessThan(ModernWaterInteractions.FixedStep));
        }

        [Test]
        public void ScrollingPreservesWorldCellsIncludingNegativeAndWrappedPositions()
        {
            var origin = ModernWaterInteractions.SnappedOrigin(new Vector3(-.1f, 0, 1023.9f));
            Assert.That(origin, Is.EqualTo(new Vector2(-64.5f, 959.5f)));
            var moved = ModernWaterInteractions.SnappedOrigin(new Vector3(.1f, 0, .1f));
            Assert.That(ModernWaterInteractions.HistoryShift(origin, moved), Is.EqualTo(new Vector2Int(1, 1)));
            Assert.That(ModernWaterInteractions.HistoryShift(moved, moved + new Vector2(1024f, -1024f)), Is.EqualTo(Vector2Int.zero));
            // World-equivalent coordinates are periodic; an ordinary 150-unit movement
            // exceeds this finite field and must be cleared rather than circularly read.
            Assert.That(ModernWaterInteractions.HistoryShift(moved, moved + new Vector2(150f, 0)).x,
                Is.GreaterThanOrEqualTo(ModernWaterInteractions.Resolution));
        }

        [Test]
        public void ImpulsePressureHasDepressionAnnulusAndZeroIntegratedArea()
        {
            Assert.That(ModernWaterInteractions.PressureProfile(0), Is.LessThan(0));
            Assert.That(ModernWaterInteractions.PressureProfile(.6f), Is.GreaterThan(0));
            Assert.That(ModernWaterInteractions.PressureProfile(1), Is.Zero);
            double integral = 0;
            const int samples = 10000;
            // The radial area measure is constant in u = (r/R)^2.
            for (var i = 0; i < samples; i++) integral += ModernWaterInteractions.PressureProfile((i + .5f) / samples) / samples;
            Assert.That(integral, Is.EqualTo(0).Within(.000001));
            // GPU stamps use the discrete Laplacian of a compact potential so a
            // subcell offset cannot introduce the continuous kernel's sampling bias.
            foreach (var radius in new[] { 1.5f, 3f })
            foreach (var offset in new[] { 0f, .13f, .25f })
            {
                double discrete = 0;
                for (var y = -10; y <= 10; y++)
                for (var x = -10; x <= 10; x++)
                    discrete += ModernWaterInteractions.DiscretePressure(new Vector2(x * .5f + offset, y * .5f + offset), radius);
                Assert.That(discrete, Is.EqualTo(0).Within(.00001));
            }
        }

        [Test]
        public void SourceQueueMergesAcrossWrapAndKeepsStrongestWhenFull()
        {
            var queue = new Vector4[ModernWaterInteractions.MaximumImpulses];
            var count = ModernWaterInteractions.AppendImpulse(queue, 0, new Vector4(1023.9f, 100f, 2f, 1f));
            count = ModernWaterInteractions.AppendImpulse(queue, count, new Vector4(.1f, 100f, 2f, 2f));
            Assert.That(count, Is.EqualTo(1));
            Assert.That(queue[0].w, Is.EqualTo(3));
            for (var i = 1; i < queue.Length; i++)
                count = ModernWaterInteractions.AppendImpulse(queue, count, new Vector4(i * 5, 0, 1.5f, .1f));
            Assert.That(count, Is.EqualTo(queue.Length));
            count = ModernWaterInteractions.AppendImpulse(queue, count, new Vector4(500, 0, 2, 4));
            Assert.That(count, Is.EqualTo(queue.Length));
            Assert.That(System.Array.Exists(queue, item => item.x == 500 && item.w == 4), Is.True);
        }

        [Test]
        public void ComputeImpulseScrollWrapAndResetPreserveTheExpectedField()
        {
            RequireCompute();
            var coast = new Texture2D(1, 1, TextureFormat.RGBAFloat, false, true);
            coast.SetPixel(0, 0, new Color(100f, 20f, 0f, 0f)); coast.Apply();
            ModernWaterInteractions water = null;
            try
            {
                water = new ModernWaterInteractions();
                water.Update(0f, new Vector3(64, 0, 64), Vector2.zero, coast);
                Assert.That(Read(water.State, 128, 128), Is.EqualTo(Color.clear));
                water.AddImpulse(new Vector3(64.25f, 0, 64.25f), 3f, 2f);
                water.Update(ModernWaterInteractions.FixedStep, new Vector3(64, 0, 64), Vector2.zero, coast);
                var initial = Read(water.State, 128, 128);
                Assert.That(initial.r, Is.LessThan(-.01f));
                Assert.That(initial.b, Is.GreaterThan(.1f));
                Assert.That(Read(water.State, 132, 128).r, Is.GreaterThan(0));
                Assert.That(Read(water.State, 0, 128).r, Is.Zero);
                water.Update(0f, new Vector3(64.5f, 0, 64), Vector2.zero, coast);
                AssertColor(Read(water.State, 127, 128), initial);
                water.Update(0f, new Vector3(1088.5f, 0, 64), Vector2.zero, coast);
                AssertColor(Read(water.State, 127, 128), initial);
                water.Reset();
                water.Update(0f, new Vector3(1088.5f, 0, 64), Vector2.zero, coast);
                Assert.That(Read(water.State, 127, 128), Is.EqualTo(Color.clear));
            }
            finally { water?.Dispose(); Object.DestroyImmediate(coast); }
        }

        [Test]
        public void ComputeLandBarrierBlocksWavePropagationAndMutationClearsHistory()
        {
            RequireCompute();
            var coast = new Texture2D(256, 256, TextureFormat.RGBAFloat, false, true) { wrapMode = TextureWrapMode.Repeat };
            var pixels = new Color[256 * 256];
            for (var i = 0; i < pixels.Length; i++) pixels[i] = new Color((i % 256) == 16 ? -2 : 100, 20, 0, 0);
            coast.SetPixels(pixels); coast.Apply();
            ModernWaterInteractions water = null;
            try
            {
                water = new ModernWaterInteractions();
                var camera = new Vector3(64, 0, 64);
                water.AddImpulse(new Vector3(62.25f, 0, 64.25f), 1.5f, 2);
                for (var i = 0; i < 90; i++) water.Update(ModernWaterInteractions.FixedStep, camera, Vector2.zero, coast);
                Assert.That(Mathf.Abs(Read(water.State, 122, 128).r), Is.GreaterThan(.00001f));
                Assert.That(Read(water.State, 140, 128).r, Is.EqualTo(0).Within(.000001));
                Assert.That(Read(water.State, 130, 128).r, Is.Zero);
                water.Reset();
                water.Update(0, camera, Vector2.zero, coast);
                Assert.That(Read(water.State, 122, 128), Is.EqualTo(Color.clear));
            }
            finally { water?.Dispose(); Object.DestroyImmediate(coast); }
        }

        private static void RequireCompute()
        {
            if (!SystemInfo.supportsComputeShaders || SystemInfo.graphicsDeviceType == GraphicsDeviceType.Null)
                Assert.Ignore("Requires the real compute graphics backend; null-device runs do not qualify GPU behavior");
        }

        private static Color Read(RenderTexture source, int x, int y)
        {
            var previous = RenderTexture.active;
            var read = new Texture2D(1, 1, TextureFormat.RGBAFloat, false, true);
            try
            {
                RenderTexture.active = source;
                read.ReadPixels(new Rect(x, y, 1, 1), 0, 0, false);
                read.Apply();
                return read.GetPixel(0, 0);
            }
            finally { RenderTexture.active = previous; Object.DestroyImmediate(read); }
        }

        private static void AssertColor(Color actual, Color expected)
        {
            Assert.That(actual.r, Is.EqualTo(expected.r).Within(.000001));
            Assert.That(actual.g, Is.EqualTo(expected.g).Within(.000001));
            Assert.That(actual.b, Is.EqualTo(expected.b).Within(.000001));
        }
    }
}
