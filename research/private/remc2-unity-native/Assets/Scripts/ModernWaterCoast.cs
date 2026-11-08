using System;
using UnityEngine;

namespace MagicCarpet.Native
{
    // Presentation bathymetry, not source collision or recovered MC2 seabed data.
    // Texel centres coincide with source-cell centres. A source revision is uploaded
    // atomically so the wave obstacle and shoreline consumers see the same coast.
    internal sealed class ModernWaterCoast : IDisposable
    {
        internal const int Resolution = 256;
        internal const float Period = 1024f;
        internal const float CellSize = Period / Resolution;
        internal const float MaximumDistance = 128f;
        private const int Count = Resolution * Resolution;
        private const float FarSquared = 1e12f;
        private readonly bool[] wet = new bool[Count];
        private readonly float[] scratch = new float[Count];
        private readonly float[] distance = new float[Count];
        private readonly Color[] pixels = new Color[Count];
        private readonly float[] line = new float[Resolution * 3];
        private readonly float[] transformed = new float[Resolution * 3];
        private readonly int[] envelope = new int[Resolution * 3];
        private readonly double[] boundaries = new double[Resolution * 3 + 1];
        private bool initialized, disposed;

        internal Texture2D Texture { get; }
        internal ulong Revision { get; private set; }
        internal ulong ShapeRevision { get; private set; }

        internal ModernWaterCoast()
        {
            Texture = new Texture2D(Resolution, Resolution, TextureFormat.RGBAHalf, false, true) {
                name = "Periodic water coast and generated visual depth",
                filterMode = FilterMode.Bilinear,
                wrapMode = TextureWrapMode.Repeat
            };
        }

        internal void Update(ModernTerrainMesh terrain)
        {
            if (terrain == null) throw new ArgumentNullException(nameof(terrain));
            ThrowIfDisposed();
            if (initialized && Revision == terrain.Revision) return;
            bool changed = !initialized;
            for (var z = 0; z < Resolution; z++)
            for (var x = 0; x < Resolution; x++)
            {
                int i = Index(x, z);
                bool value = terrain.IsWater((x + .5f) * CellSize, (z + .5f) * CellSize);
                changed |= wet[i] != value;
                wet[i] = value;
            }
            if (changed) Rebuild(terrain.Revision);
            else Revision = terrain.Revision;
        }

        // Also supports authored water masks; copies the mask to retain ownership.
        internal void UpdateMask(bool[] source, ulong revision)
        {
            ThrowIfDisposed();
            if (source == null || source.Length != Count)
                throw new ArgumentException("Water mask must contain 256 by 256 cells", nameof(source));
            if (initialized && Revision == revision) return;
            bool changed = !initialized;
            for (int i = 0; i < Count; i++) { changed |= source[i] != wet[i]; wet[i] = source[i]; }
            if (changed) Rebuild(revision);
            else Revision = revision;
        }

        private void Rebuild(ulong revision)
        {
            // Each transform is linear in texel count. Triplicating a one-dimensional
            // line and keeping its middle period produces exact toroidal centre
            // distances, including narrow channels and coasts across the map seam.
            DistanceTo(false);
            for (var i = 0; i < Count; i++)
                if (wet[i]) distance[i] = Mathf.Min(MaximumDistance, Mathf.Sqrt(scratch[i]) * CellSize - CellSize * .5f);
            DistanceTo(true);
            for (var i = 0; i < Count; i++)
                if (!wet[i]) distance[i] = -Mathf.Min(MaximumDistance, Mathf.Sqrt(scratch[i]) * CellSize - CellSize * .5f);
            for (var z = 0; z < Resolution; z++)
            for (var x = 0; x < Resolution; x++)
            {
                var i = Index(x, z);
                var gradientX = (distance[Index(x + 1, z)] - distance[Index(x - 1, z)]) / (2f * CellSize);
                var gradientZ = (distance[Index(x, z + 1)] - distance[Index(x, z - 1)]) / (2f * CellSize);
                pixels[i] = new Color(distance[i], VisualDepth(distance[i]), gradientX, gradientZ);
            }
            Texture.SetPixels(pixels);
            Texture.Apply(false, false);
            Revision = revision;
            unchecked { ShapeRevision++; }
            initialized = true;
        }

        private void DistanceTo(bool targetWet)
        {
            for (var z = 0; z < Resolution; z++)
            {
                for (var x = 0; x < line.Length; x++) line[x] = wet[Index(x, z)] == targetWet ? 0f : FarSquared;
                TransformLine();
                for (var x = 0; x < Resolution; x++) scratch[Index(x, z)] = transformed[x + Resolution];
            }
            for (var x = 0; x < Resolution; x++)
            {
                for (var z = 0; z < line.Length; z++) line[z] = scratch[Index(x, z)];
                TransformLine();
                for (var z = 0; z < Resolution; z++) scratch[Index(x, z)] = transformed[z + Resolution];
            }
        }

        // Lower envelope of unit parabolas, evaluated with double intersections to
        // keep the finite no-site sentinel from swallowing small squared offsets.
        private void TransformLine()
        {
            var k = 0;
            envelope[0] = 0;
            boundaries[0] = double.NegativeInfinity;
            boundaries[1] = double.PositiveInfinity;
            for (var q = 1; q < line.Length; q++)
            {
                double crossing;
                while (true)
                {
                    var p = envelope[k];
                    crossing = ((double)line[q] + q * q - line[p] - p * p) / (2.0 * (q - p));
                    if (crossing > boundaries[k]) break;
                    k--;
                }
                envelope[++k] = q;
                boundaries[k] = crossing;
                boundaries[k + 1] = double.PositiveInfinity;
            }
            k = 0;
            for (var q = 0; q < line.Length; q++)
            {
                while (boundaries[k + 1] < q) k++;
                var delta = q - envelope[k];
                transformed[q] = delta * delta + line[envelope[k]];
            }
        }

        // Half-cell subtraction gives the correct interface for axis-aligned cells.
        // Diagonal corners are an explicitly smoothed visual distance approximation.
        internal float SampleDistance(float x, float z) => Sample(x, z).r;
        internal float SampleDepth(float x, float z) => Sample(x, z).g;
        internal Vector2 SampleGradient(float x, float z)
        {
            var value = Sample(x, z);
            return new Vector2(value.b, value.a);
        }

        internal bool IsWater(float x, float z)
        {
            ThrowIfDisposed();
            return initialized && wet[Index(Mathf.FloorToInt(Mathf.Repeat(x, Period) / CellSize),
                Mathf.FloorToInt(Mathf.Repeat(z, Period) / CellSize))];
        }

        private Color Sample(float x, float z)
        {
            ThrowIfDisposed();
            if (!initialized) return new Color(-MaximumDistance, 0, 0, 0);
            var cx = Mathf.Repeat(x, Period) / CellSize - .5f;
            var cz = Mathf.Repeat(z, Period) / CellSize - .5f;
            var ix = Mathf.FloorToInt(cx); var iz = Mathf.FloorToInt(cz);
            return Color.LerpUnclamped(Color.LerpUnclamped(pixels[Index(ix, iz)], pixels[Index(ix + 1, iz)], cx - ix),
                Color.LerpUnclamped(pixels[Index(ix, iz + 1)], pixels[Index(ix + 1, iz + 1)], cx - ix), cz - iz);
        }

        internal static float VisualDepth(float signedDistance) => Mathf.Min(24f, Mathf.Max(0f, signedDistance) * .45f);
        private static int Index(int x, int z) => (x & 255) + ((z & 255) << 8);
        private void ThrowIfDisposed()
        {
            if (disposed) throw new ObjectDisposedException(nameof(ModernWaterCoast));
        }

        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            if (Application.isPlaying) UnityEngine.Object.Destroy(Texture);
            else UnityEngine.Object.DestroyImmediate(Texture);
        }
    }
}
