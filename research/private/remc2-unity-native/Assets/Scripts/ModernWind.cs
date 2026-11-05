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
        internal Vector2 Velocity { get; private set; }
        internal Vector2 Offset => new Vector2((float)offsetX, (float)offsetZ);
        internal Vector2 CloudOffset { get; private set; }
        internal float Time { get; private set; }

        internal ModernWind() : this(new Vector2(8f, 3.5f)) { }
        internal ModernWind(Vector2 initialVelocity)
        {
            SetTarget(initialVelocity);
            Velocity = target;
        }

        internal void SetTarget(Vector2 velocity)
        {
            if (!Finite(velocity.x) || !Finite(velocity.y)) throw new ArgumentException("Wind must be finite", nameof(velocity));
            target = Vector2.ClampMagnitude(velocity, 40f);
        }

        internal void Update(float dt, float time)
        {
            if (!Finite(dt) || dt < 0f) throw new ArgumentOutOfRangeException(nameof(dt));
            if (!Finite(time)) throw new ArgumentOutOfRangeException(nameof(time));
            Time = time;
            // Exact integration of exponential relaxation, independent of frame rate.
            // A long pause advances cosmetic weather safely without numerical substeps.
            var decay = Math.Exp(-dt / ResponseSeconds);
            var integratedDecay = ResponseSeconds * (1.0 - decay);
            CloudOffset += new Vector2((float)(target.x * (double)dt + (Velocity.x - target.x) * integratedDecay),
                (float)(target.y * (double)dt + (Velocity.y - target.y) * integratedDecay));
            offsetX = Wrap(offsetX + target.x * (double)dt + (Velocity.x - target.x) * integratedDecay);
            offsetZ = Wrap(offsetZ + target.y * (double)dt + (Velocity.y - target.y) * integratedDecay);
            Velocity = target + (Velocity - target) * (float)decay;
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
