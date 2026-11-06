using System;
using UnityEngine;

namespace MagicCarpet.Native
{
    // One-metre, periodic presentation coverage derived from source mode 26.
    // R is water coverage; G marks zero-height mixed reflective tiles. The
    // source atlas is reduced by area averaging, so this is not 32px fidelity.
    internal sealed class ModernShoreMask : IDisposable
    {
        private const int Map = 256, Scale = 4, Size = Map * Scale, Tiles = 152;
        private readonly byte[] variants = new byte[Tiles * 8 * Scale * Scale];
        private readonly bool[] hasCoverage = new bool[Tiles];
        private readonly Color32[] pixels = new Color32[Size * Size];
        private bool initialized, disposed;
        internal Texture2D Texture { get; }
        internal ulong Revision { get; private set; }

        internal ModernShoreMask(byte[] atlas)
        {
            if (atlas == null || atlas.Length != 256 * 608)
                throw new ArgumentException("Shore atlas must be the 256 by 608 indexed owner atlas", nameof(atlas));
            for (var type = 0; type < Tiles; type++)
            {
                if (!ModernTerrainMesh.IsReflectiveMaterial((byte)type)) continue;
                for (var orientation = 0; orientation < 8; orientation++)
                for (var z = 0; z < Scale; z++) for (var x = 0; x < Scale; x++)
                {
                    var water = 0;
                    for (var dz = 0; dz < 8; dz++) for (var dx = 0; dx < 8; dx++)
                    {
                        var px = x * 8 + dx; var pz = z * 8 + dz;
                        // Canonical quadrant zero: source UV rows 0,4,...,28.
                        // All eight rows are affine, hence identical on either
                        // diagonal of the source checkerboard triangulation.
                        int u, v;
                        switch (orientation)
                        {
                            case 0: u=px; v=pz; break;
                            case 1: u=31-px; v=pz; break;
                            case 2: u=px; v=31-pz; break;
                            case 3: u=31-px; v=31-pz; break;
                            case 4: u=pz; v=px; break;
                            case 5: u=pz; v=31-px; break;
                            case 6: u=31-pz; v=px; break;
                            default: u=31-pz; v=31-px; break;
                        }
                        var atlasIndex = (type / 8 * 32 + v) * 256 + type % 8 * 32 + u;
                        if (atlas[atlasIndex] < 12) water++;
                    }
                    var value = (byte)((water * 255 + 32) / 64);
                    variants[Variant(type, orientation, x, z)] = value;
                    if (value != 0) hasCoverage[type] = true;
                }
            }
            hasCoverage[0] = true;
            Texture = new Texture2D(Size, Size, TextureFormat.RGBA32, true, true) {
                name = "Periodic source shoreline coverage",
                filterMode = FilterMode.Bilinear, wrapMode = TextureWrapMode.Repeat
            };
        }

        // Local coordinates are normalized within a four-metre source cell.
        internal float Sample(byte type, byte angle, float localX, float localZ)
        {
            ThrowIfDisposed();
            if (type == 0) return 1f;
            if (type >= Tiles) return 0f;
            var x = Mathf.Clamp(Mathf.FloorToInt(localX * Scale), 0, Scale - 1);
            var z = Mathf.Clamp(Mathf.FloorToInt(localZ * Scale), 0, Scale - 1);
            return variants[Variant(type, (angle >> 4) & 7, x, z)] / 255f;
        }

        internal bool HasCoverage(byte type)
        {
            ThrowIfDisposed();
            return type < Tiles && hasCoverage[type];
        }

        internal void Update(byte[] heights, byte[] types, byte[] angles, ulong revision)
        {
            ThrowIfDisposed();
            if (heights == null || types == null || angles == null ||
                heights.Length != Map * Map || types.Length != Map * Map || angles.Length != Map * Map)
                throw new ArgumentException("Shore source planes must each contain 256 by 256 bytes");
            if (initialized && Revision == revision) return;
            for (var z = 0; z < Map; z++) for (var x = 0; x < Map; x++)
            {
                var i = x + z * Map; var type = types[i];
                var wave = (angles[i] & 8) != 0;
                var partial = heights[i] == 0 && type != 0 && HasCoverage(type);
                for (var dz = 0; dz < Scale; dz++) for (var dx = 0; dx < Scale; dx++)
                {
                    byte value = 0;
                    if (wave || (heights[i] == 0 && type == 0)) value = 255;
                    else if (partial) value = variants[Variant(type, (angles[i] >> 4) & 7, dx, dz)];
                    pixels[x * Scale + dx + (z * Scale + dz) * Size] = new Color32(value, partial ? (byte)255 : (byte)0, 0, 255);
                }
            }
            Texture.SetPixels32(pixels); Texture.Apply(true, false);
            Revision = revision; initialized = true;
        }

        // Match bilinear base-level texture sampling, including the world seam.
        internal float SampleWorld(float x, float z)
        {
            ThrowIfDisposed();
            if (!initialized) return 0f;
            var fx = Mathf.Repeat(x, Size) - .5f; var fz = Mathf.Repeat(z, Size) - .5f;
            var ix = Mathf.FloorToInt(fx); var iz = Mathf.FloorToInt(fz);
            float At(int a, int b) => pixels[(a & (Size - 1)) + (b & (Size - 1)) * Size].r / 255f;
            return Mathf.Lerp(Mathf.Lerp(At(ix,iz),At(ix+1,iz),fx-ix),
                Mathf.Lerp(At(ix,iz+1),At(ix+1,iz+1),fx-ix),fz-iz);
        }

        private static int Variant(int type, int orientation, int x, int z) => ((type * 8 + orientation) * Scale + z) * Scale + x;
        private void ThrowIfDisposed() { if (disposed) throw new ObjectDisposedException(nameof(ModernShoreMask)); }
        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            if (Application.isPlaying) UnityEngine.Object.Destroy(Texture);
            else UnityEngine.Object.DestroyImmediate(Texture);
        }
    }
}
