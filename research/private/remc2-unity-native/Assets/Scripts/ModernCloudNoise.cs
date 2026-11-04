using System;
using UnityEngine;

namespace MagicCarpet.Native
{
    // The cloud field is sampled many times per pixel. Store its lattice once
    // and let the texture unit interpolate it instead of hashing eight corners
    // for every octave of every ray-march step. No simulation state is involved.
    internal static class ModernCloudNoise
    {
        internal const int Size = 128;

        internal static ushort[] Lattice()
        {
            var values = new ushort[Size * Size * Size];
            for (var z = 0; z < Size; z++)
            for (var y = 0; y < Size; y++)
            for (var x = 0; x < Size; x++)
                values[x + Size * (y + Size * z)] = (ushort)Math.Round(
                    Corner(x - Size / 2, y - Size / 2, z - Size / 2) * ushort.MaxValue);
            return values;
        }

        internal static double Corner(int x, int y, int z)
        {
            unchecked
            {
                var hash = (uint)x * 0x8da6b343u ^ (uint)y * 0xd8163841u ^
                    (uint)z * 0xcb1ab31fu ^ 0x9e3779b9u;
                hash ^= hash >> 16; hash *= 0x7feb352du;
                hash ^= hash >> 15; hash *= 0x846ca68bu; hash ^= hash >> 16;
                return (hash & 0x00ffffffu) / 16777216.0;
            }
        }

        internal static Texture3D Create()
        {
            var texture = new Texture3D(Size, Size, Size, TextureFormat.R16, false) {
                name = "NightSpire cloud lattice", wrapMode = TextureWrapMode.Repeat,
                filterMode = FilterMode.Bilinear
            };
            try
            {
                texture.SetPixelData(Lattice(), 0);
                texture.Apply(false, true);
                return texture;
            }
            catch
            {
                UnityEngine.Object.Destroy(texture);
                throw;
            }
        }
    }
}
