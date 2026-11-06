using System;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernWindEditModeTests
    {
        [Test]
        public void DefaultWeatherVariesSpeedAndDirectionContinuouslyWithoutAnOverride()
        {
            var wind = new ModernWind();
            Assert.That(wind.WeatherEnabled, Is.True);
            var previous = wind.Velocity;
            var minimumSpeed = float.PositiveInfinity; var maximumSpeed = 0f;
            var minimumAngle = float.PositiveInfinity; var maximumAngle = float.NegativeInfinity;
            for (var frame = 1; frame <= 3600; frame++)
            {
                wind.Update(1f / 60, frame / 60f);
                Assert.That(Vector2.Distance(previous, wind.Velocity), Is.LessThan(.03f), "no frame-to-frame gust jump");
                var speed = wind.Velocity.magnitude;
                var angle = Mathf.Atan2(wind.Velocity.y, wind.Velocity.x) * Mathf.Rad2Deg;
                minimumSpeed = Mathf.Min(minimumSpeed, speed); maximumSpeed = Mathf.Max(maximumSpeed, speed);
                minimumAngle = Mathf.Min(minimumAngle, angle); maximumAngle = Mathf.Max(maximumAngle, angle);
                previous = wind.Velocity;
            }
            Assert.That(maximumSpeed - minimumSpeed, Is.GreaterThan(4f), "weather must visibly change speed");
            Assert.That(maximumAngle - minimumAngle, Is.GreaterThan(35f), "weather must change direction as well as amplitude");
            Assert.That(minimumSpeed, Is.GreaterThan(1f));
            Assert.That(maximumSpeed, Is.LessThan(15f));
        }

        [Test]
        public void AutonomousWeatherAndAdvectionAgreeAcrossFramePartitionsAndClockOrigins()
        {
            var coarse = new ModernWind();
            var fine = new ModernWind();
            coarse.Update(64, 64);
            // Binary-exact dt isolates integration from floating-point sums. The time
            // argument's origin is metadata; cosmetic elapsed dt owns weather phase.
            for (var frame = 1; frame <= 4096; frame++) fine.Update(1f / 64, 10000 + frame / 64f);
            Assert.That(Vector2.Distance(coarse.Velocity, fine.Velocity), Is.LessThan(.00001f));
            Assert.That(Vector2.Distance(coarse.Offset, fine.Offset), Is.LessThan(.0001f));
            Assert.That(Vector2.Distance(coarse.CloudOffset, fine.CloudOffset), Is.LessThan(.0001f));
        }

        [Test]
        public void ExplicitOverrideAndWeatherResumptionPreserveVelocityAndAdvectionContinuity()
        {
            var wind = new ModernWind();
            wind.Update(7, 7);
            var velocity = wind.Velocity; var offset = wind.CloudOffset;
            wind.SetTarget(new Vector2(-4, 7));
            Assert.That(wind.WeatherEnabled, Is.False);
            Assert.That(wind.Velocity, Is.EqualTo(velocity));
            Assert.That(wind.CloudOffset, Is.EqualTo(offset));
            wind.Update(80, 87);
            Assert.That(Vector2.Distance(wind.Velocity, new Vector2(-4, 7)), Is.LessThan(.0001f));
            velocity = wind.Velocity; offset = wind.CloudOffset;
            wind.ResumeWeather();
            Assert.That(wind.WeatherEnabled, Is.True);
            Assert.That(wind.Velocity, Is.EqualTo(velocity));
            Assert.That(wind.CloudOffset, Is.EqualTo(offset));
            wind.Update(.001f, 87.001f);
            Assert.That(Vector2.Distance(wind.Velocity, velocity), Is.LessThan(.01f));
            Assert.That(Vector2.Distance(wind.CloudOffset, offset), Is.LessThan(.01f));
        }

        [Test]
        public void ResumedWeatherIntegralIsIndependentOfFramePartition()
        {
            var coarse = new ModernWind(new Vector2(-9, 3));
            var fine = new ModernWind(new Vector2(-9, 3));
            Assert.That(coarse.WeatherEnabled, Is.False);
            coarse.Update(5, 5); fine.Update(5, 5);
            coarse.ResumeWeather(); fine.ResumeWeather();
            coarse.Update(16, 21);
            for (var frame = 1; frame <= 1024; frame++) fine.Update(1f / 64, 5 + frame / 64f);
            Assert.That(Vector2.Distance(coarse.Velocity, fine.Velocity), Is.LessThan(.00001f));
            Assert.That(Vector2.Distance(coarse.CloudOffset, fine.CloudOffset), Is.LessThan(.0001f));
        }

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
