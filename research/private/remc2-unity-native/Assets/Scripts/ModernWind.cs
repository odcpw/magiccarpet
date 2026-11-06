using System;
using UnityEngine;

namespace MagicCarpet.Native
{
    // Shared cosmetic weather only. Source Wind spell and flight physics remain Rust-owned.
    // Consumers use integrated Offset for advection, never Velocity * absolute time:
    // changing wind should not teleport cloud, foam or vegetation phases.
    internal sealed class ModernWind
    {
        internal const float ResponseSeconds = 4f;
        internal const float Period = 1024f;
        private Vector2 target;
        private double offsetX, offsetZ;
        private double cloudX, cloudZ, weatherTime;
        private double velocityX, velocityZ, weatherCorrectionX, weatherCorrectionZ;
        internal Vector2 Velocity { get; private set; }
        internal Vector2 Offset => new Vector2((float)offsetX, (float)offsetZ);
        internal Vector2 CloudOffset => new Vector2((float)cloudX, (float)cloudZ);
        internal float Time { get; private set; }
        internal bool WeatherEnabled { get; private set; }

        // A slow direction change, a broad gust and a smaller flutter. Vector sums
        // avoid angle-wrap discontinuities. This is authored cosmetic weather, not
        // a meteorological simulation. Its double clock is advanced only by dt.
        private static readonly WeatherMode[] WeatherModes = {
            new WeatherMode(3.0, .6, .13, .4),
            new WeatherMode(-1.0, 3.3, .08, 2.1),
            new WeatherMode(.7, .6, .40, 1.7)
        };

        private readonly struct WeatherMode
        {
            internal readonly double X, Z, Frequency, Phase;
            internal WeatherMode(double x, double z, double frequency, double phase)
            { X = x; Z = z; Frequency = frequency; Phase = phase; }
        }

        internal ModernWind()
        {
            WeatherEnabled = true;
            WeatherVelocity(0, out velocityX, out velocityZ);
            Velocity = new Vector2((float)velocityX, (float)velocityZ);
        }

        internal ModernWind(Vector2 initialVelocity)
        {
            SetTarget(initialVelocity);
            Velocity = target;
            velocityX = target.x; velocityZ = target.y;
        }

        internal void SetTarget(Vector2 velocity)
        {
            if (!Finite(velocity.x) || !Finite(velocity.y)) throw new ArgumentException("Wind must be finite", nameof(velocity));
            target = Vector2.ClampMagnitude(velocity, 40f);
            WeatherEnabled = false;
        }

        // A review/art override can yield back to weather without changing the
        // current velocity or accumulated advection at the instant of transition.
        internal void ResumeWeather()
        {
            if (WeatherEnabled) return;
            WeatherVelocity(weatherTime, out var x, out var z);
            weatherCorrectionX = velocityX - x;
            weatherCorrectionZ = velocityZ - z;
            WeatherEnabled = true;
        }

        internal void Update(float dt, float time)
        {
            if (!Finite(dt) || dt < 0f) throw new ArgumentOutOfRangeException(nameof(dt));
            if (!Finite(time)) throw new ArgumentOutOfRangeException(nameof(time));
            Time = time;
            var decay = Math.Exp(-(double)dt / ResponseSeconds);
            var integratedDecay = ResponseSeconds * (1.0 - decay);
            double dx, dz;
            if (WeatherEnabled)
            {
                // Integrate the varying field analytically, including the transient
                // after ResumeWeather. Sampling one target per frame would make wind
                // advection depend on frame rate and stutter across long frames.
                WeatherIntegral(weatherTime, dt, out dx, out dz);
                dx += weatherCorrectionX * integratedDecay;
                dz += weatherCorrectionZ * integratedDecay;
                weatherCorrectionX *= decay; weatherCorrectionZ *= decay;
                WeatherVelocity(weatherTime + dt, out velocityX, out velocityZ);
                velocityX += weatherCorrectionX; velocityZ += weatherCorrectionZ;
            }
            else
            {
                dx = target.x * (double)dt + (velocityX - target.x) * integratedDecay;
                dz = target.y * (double)dt + (velocityZ - target.y) * integratedDecay;
                velocityX = target.x + (velocityX - target.x) * decay;
                velocityZ = target.y + (velocityZ - target.y) * decay;
            }
            weatherTime += dt;
            cloudX += dx; cloudZ += dz;
            offsetX = Wrap(offsetX + dx); offsetZ = Wrap(offsetZ + dz);
            Velocity = new Vector2((float)velocityX, (float)velocityZ);
        }

        private static void WeatherVelocity(double time, out double x, out double z)
        {
            x = 6.5; z = 2.5;
            foreach (var mode in WeatherModes)
            {
                var value = Math.Sin(time * mode.Frequency + mode.Phase);
                x += mode.X * value; z += mode.Z * value;
            }
        }

        private static void WeatherIntegral(double start, double duration, out double x, out double z)
        {
            x = 6.5 * duration; z = 2.5 * duration;
            foreach (var mode in WeatherModes)
            {
                // Cos(a)-cos(b) in this form avoids cancellation for tiny frame dt.
                var value = 2 * Math.Sin((start + duration * .5) * mode.Frequency + mode.Phase)
                    * Math.Sin(duration * .5 * mode.Frequency) / mode.Frequency;
                x += mode.X * value; z += mode.Z * value;
            }
        }

        internal Vector2 SampleVelocity(Vector2 position, float time) =>
            Velocity + SampleGust(position, time) * Mathf.Min(2f, Velocity.magnitude * .16f);

        // Two curl modes form a smooth, deterministic, divergence-free cosmetic gust.
        // Integer spatial modes make values and derivatives periodic on the source torus.
        internal static Vector2 SampleGust(Vector2 position, float time)
        {
            var p = new Vector2(Mathf.Repeat(position.x, Period), Mathf.Repeat(position.y, Period)) * (2f * Mathf.PI / Period);
            var a = Mathf.Cos(3f * p.x + 2f * p.y - time * .31f + .7f);
            var b = Mathf.Cos(-p.x + 5f * p.y + time * .19f + 2.3f);
            return new Vector2(2f * a + 2.5f * b, -3f * a + .5f * b) / 6f;
        }

        private static double Wrap(double value) => value - Math.Floor(value / Period) * Period;
        private static bool Finite(float value) => !float.IsNaN(value) && !float.IsInfinity(value);
    }
}
