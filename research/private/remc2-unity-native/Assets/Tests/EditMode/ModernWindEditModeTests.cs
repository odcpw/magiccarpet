using System;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernWindEditModeTests
    {
        [Test]
        public void WindChangesContinuouslyAndIntegratedOffsetIsFrameRateIndependent()
        {
            var single = new ModernWind(new Vector2(8, 3));
            var divided = new ModernWind(new Vector2(8, 3));
            single.SetTarget(new Vector2(-5, 11));
            divided.SetTarget(new Vector2(-5, 11));
            Assert.That(single.Velocity, Is.EqualTo(new Vector2(8, 3)), "target changes do not rotate existing motion instantly");
            single.Update(2, 2);
            for (var i = 0; i < 120; i++) divided.Update(1f / 60, (i + 1) / 60f);
            Assert.That(Vector2.Distance(single.Velocity, divided.Velocity), Is.LessThan(.0002f));
            Assert.That(Vector2.Distance(single.Offset, divided.Offset), Is.LessThan(.0002f));
            Assert.That(single.Velocity.x, Is.InRange(-5f, 8f));
            Assert.That(single.Velocity.y, Is.InRange(3f, 11f));
        }

        [Test]
        public void AdvectionWrapsWithoutPhaseJumpAndPauseRemainsFinite()
        {
            var wind = new ModernWind(new Vector2(8, -4));
            wind.Update(129, 129);
            Assert.That(wind.Offset.x, Is.EqualTo(8).Within(.0001));
            Assert.That(wind.Offset.y, Is.EqualTo(508).Within(.0001));
            wind.SetTarget(Vector2.zero);
            wind.Update(10000, 10129);
            Assert.That(wind.Velocity, Is.EqualTo(Vector2.zero));
            Assert.That(wind.Offset.x, Is.InRange(0f, 1024f));
            Assert.That(wind.Offset.y, Is.InRange(0f, 1024f));
        }

        [Test]
        public void GustIsDeterministicPeriodicBoundedAndChangesWithTime()
        {
            var changed = false;
            for (var i = 0; i < 64; i++)
            {
                var point = new Vector2(i * 13.25f, i * 7.75f - 100);
                var gust = ModernWind.SampleGust(point, 37.5f);
                var repeated = ModernWind.SampleGust(point + new Vector2(1024, -2048), 37.5f);
                Assert.That(Vector2.Distance(gust, repeated), Is.LessThan(.0001f));
                Assert.That(gust, Is.EqualTo(ModernWind.SampleGust(point, 37.5f)));
                Assert.That(gust.magnitude, Is.LessThanOrEqualTo(1f));
                changed |= Vector2.Distance(gust, ModernWind.SampleGust(point, 40f)) > .01f;
            }
            Assert.That(changed, Is.True);
        }

        [Test]
        public void InvalidInputsRejectAndExtremeAuthoredWindIsBounded()
        {
            var wind = new ModernWind();
            Assert.Throws<ArgumentException>(() => wind.SetTarget(new Vector2(float.NaN, 1)));
            Assert.Throws<ArgumentOutOfRangeException>(() => wind.Update(-1, 0));
            Assert.Throws<ArgumentOutOfRangeException>(() => wind.Update(1, float.PositiveInfinity));
            wind.SetTarget(new Vector2(1000, 0));
            wind.Update(100, 100);
            Assert.That(wind.Velocity.magnitude, Is.LessThanOrEqualTo(40.001f));
        }
    }
}
