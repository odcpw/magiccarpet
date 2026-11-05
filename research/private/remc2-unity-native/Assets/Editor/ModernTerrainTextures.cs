using System;
using System.IO;
using System.Security.Cryptography;
using UnityEditor;
using UnityEngine;

namespace MagicCarpet.Native.Editor
{
    // Offline channel packing. Color RGB keeps sRGB encoding; alpha is linear
    // roughness. The original source maps remain available for comparisons.
    public static class ModernTerrainTextures
    {
        private const string Folder = "Assets/Resources/ModernTextures/";

        public static void Prepare()
        {
            foreach (var layer in new[] { "Rock", "Ground" })
            {
                var colorPath = Folder + layer + "Albedo.jpg";
                var roughPath = Folder + layer + "Roughness.jpg";
                var packedPath = Folder + layer + "AlbedoRoughness.png";
                var stampPath = packedPath + ".sources.txt";
                var stamp = "RGBA-v1: RGB=unmodified-sRGB-albedo, A=linear-roughness-R\n" +
                    Hash(colorPath) + "\n" + Hash(roughPath) + "\n";
                if (!File.Exists(packedPath))
                {
                    var color = ReadImage(colorPath);
                    var rough = ReadImage(roughPath);
                    var packed = new Texture2D(color.width, color.height, TextureFormat.RGBA32, false, true);
                    try
                    {
                        if (color.width != rough.width || color.height != rough.height)
                            throw new InvalidDataException("Terrain packing requires matching source dimensions");
                        var pixels = color.GetPixels32();
                        var values = rough.GetPixels32();
                        for (var i = 0; i < pixels.Length; i++) pixels[i].a = values[i].r;
                        packed.SetPixels32(pixels);
                        packed.Apply(false, false);
                        File.WriteAllBytes(packedPath, packed.EncodeToPNG());
                        File.WriteAllText(stampPath, stamp);
                    }
                    finally
                    {
                        UnityEngine.Object.DestroyImmediate(color);
                        UnityEngine.Object.DestroyImmediate(rough);
                        UnityEngine.Object.DestroyImmediate(packed);
                    }
                    AssetDatabase.ImportAsset(packedPath, ImportAssetOptions.ForceSynchronousImport);
                    AssetDatabase.ImportAsset(stampPath, ImportAssetOptions.ForceSynchronousImport);
                }
                if (!File.Exists(stampPath) || File.ReadAllText(stampPath) != stamp)
                    throw new InvalidDataException("Terrain packed source changed; preserve the previous output before regenerating: " + packedPath);
                var importer = AssetImporter.GetAtPath(packedPath) as TextureImporter;
                if (importer == null) throw new InvalidDataException("Missing terrain packed importer: " + packedPath);
                var platform = importer.GetPlatformTextureSettings("Standalone");
                var stale = importer.textureType != TextureImporterType.Default || importer.textureShape != TextureImporterShape.Texture2D || !importer.sRGBTexture ||
                    !importer.mipmapEnabled || importer.alphaSource != TextureImporterAlphaSource.FromInput ||
                    importer.alphaIsTransparency || importer.anisoLevel != 8 || importer.maxTextureSize != 2048 ||
                    importer.mipMapsPreserveCoverage || importer.wrapMode != TextureWrapMode.Repeat || importer.filterMode != FilterMode.Bilinear ||
                    importer.textureCompression != TextureImporterCompression.CompressedHQ ||
                    !platform.overridden || platform.maxTextureSize != 2048 || platform.format != TextureImporterFormat.BC7 || platform.compressionQuality != 100;
                if (stale)
                {
                    importer.textureType = TextureImporterType.Default;
                    importer.textureShape = TextureImporterShape.Texture2D;
                    importer.sRGBTexture = true;
                    importer.mipmapEnabled = true;
                    importer.alphaSource = TextureImporterAlphaSource.FromInput;
                    importer.alphaIsTransparency = false;
                    importer.mipMapsPreserveCoverage = false;
                    importer.anisoLevel = 8;
                    importer.maxTextureSize = 2048;
                    importer.wrapMode = TextureWrapMode.Repeat;
                    importer.filterMode = FilterMode.Bilinear;
                    importer.textureCompression = TextureImporterCompression.CompressedHQ;
                    platform.name = "Standalone";
                    platform.overridden = true;
                    platform.maxTextureSize = 2048;
                    platform.format = TextureImporterFormat.BC7;
                    platform.compressionQuality = 100;
                    importer.SetPlatformTextureSettings(platform);
                    importer.SaveAndReimport();
                }
                var resource = Resources.Load<Texture2D>("ModernTextures/" + layer + "AlbedoRoughness");
                if (resource == null || resource.width != 2048 || resource.height != 2048 || resource.mipmapCount != 12)
                    throw new InvalidDataException("Packed terrain resource lacks original dimensions/mips: " + packedPath);
                Debug.Log($"MC_MODERN_TERRAIN_PACK layer={layer} size={resource.width} format={resource.format} mips={resource.mipmapCount}");
            }
            AssetDatabase.SaveAssets();
        }

        private static Texture2D ReadImage(string path)
        {
            var texture = new Texture2D(2, 2, TextureFormat.RGBA32, false, true);
            if (ImageConversion.LoadImage(texture, File.ReadAllBytes(path), false)) return texture;
            UnityEngine.Object.DestroyImmediate(texture);
            throw new InvalidDataException("Cannot decode terrain source " + path);
        }

        // Invoke on the actual graphics backend: PNG byte equality alone
        // cannot prove imported sRGB/alpha sampling or compressed mip behavior.
        public static void ValidateGpu()
        {
            Prepare();
            if (SystemInfo.graphicsDeviceType == UnityEngine.Rendering.GraphicsDeviceType.Null)
                throw new InvalidOperationException("Packed terrain GPU validation requires a graphics device");
            foreach (var layer in new[] { "Rock", "Ground" })
            foreach (var size in new[] { 2048, 256, 32 })
            {
                var prefix = "ModernTextures/" + layer;
                var packed = ReadGpu(Resources.Load<Texture2D>(prefix + "AlbedoRoughness"), size);
                var color = ReadGpu(Resources.Load<Texture2D>(prefix + "Albedo"), size);
                var rough = ReadGpu(Resources.Load<Texture2D>(prefix + "Roughness"), size);
                double rgbError = 0, roughError = 0;
                for (var i = 0; i < packed.Length; i++)
                {
                    rgbError += Math.Abs(packed[i].r - color[i].r) + Math.Abs(packed[i].g - color[i].g) + Math.Abs(packed[i].b - color[i].b);
                    roughError += Math.Abs(packed[i].a - rough[i].r);
                }
                rgbError /= packed.Length * 3;
                roughError /= packed.Length;
                Debug.Log($"MC_MODERN_TERRAIN_GPU layer={layer} size={size} mean_linear_rgb_error={rgbError:F6} mean_linear_rough_error={roughError:F6} gpu={SystemInfo.graphicsDeviceName}");
                // A 1.5% mean normalized-channel budget admits block-compression
                // differences while rejecting a mistaken sRGB roughness decode.
                if (rgbError > .015 || roughError > .015)
                    throw new InvalidDataException("Packed terrain exceeds its shader-input error budget");
            }
        }

        private static Color[] ReadGpu(Texture texture, int size)
        {
            if (texture == null) throw new InvalidDataException("Missing GPU validation texture");
            var previous = RenderTexture.active;
            var target = new RenderTexture(size, size, 0, RenderTextureFormat.ARGBFloat, RenderTextureReadWrite.Linear);
            var readback = new Texture2D(size, size, TextureFormat.RGBAFloat, false, true);
            try
            {
                if (!target.Create()) throw new InvalidOperationException("Cannot allocate terrain validation target");
                Graphics.Blit(texture, target);
                RenderTexture.active = target;
                readback.ReadPixels(new Rect(0, 0, size, size), 0, 0, false);
                readback.Apply(false, false);
                return readback.GetPixels();
            }
            finally
            {
                RenderTexture.active = previous;
                target.Release();
                UnityEngine.Object.DestroyImmediate(target);
                UnityEngine.Object.DestroyImmediate(readback);
            }
        }

        private static string Hash(string path)
        {
            using var sha = SHA256.Create();
            return BitConverter.ToString(sha.ComputeHash(File.ReadAllBytes(path))).Replace("-", "").ToLowerInvariant();
        }

        // Read-only diagnosis for native image decoding/conversion: never
        // regenerates packed content or relaxes its exact channel checks.
        public static void DiagnoseDecode()
        {
            foreach (var layer in new[] { "Rock", "Ground" })
            for (var repeat = 0; repeat < 8; repeat++)
            {
                var color = ReadImage(Folder + layer + "Albedo.jpg");
                var rough = ReadImage(Folder + layer + "Roughness.jpg");
                var packed = ReadImage(Folder + layer + "AlbedoRoughness.png");
                var fresh = new Texture2D(color.width, color.height, TextureFormat.RGBA32, false, true);
                var roundtrip = new Texture2D(2, 2, TextureFormat.RGBA32, false, true);
                try
                {
                    var c = color.GetPixels32(); var r = rough.GetPixels32(); var p = packed.GetPixels32();
                    var c2 = color.GetPixels32(); var r2 = rough.GetPixels32(); var p2 = packed.GetPixels32();
                    var expected = (Color32[])c.Clone();
                    long readsChanged = 0, colorChanged = 0, roughChanged = 0, roundtripChanged = 0;
                    var examples = "";
                    for (var i = 0; i < p.Length; i++)
                    {
                        if (!c[i].Equals(c2[i]) || !r[i].Equals(r2[i]) || !p[i].Equals(p2[i])) readsChanged++;
                        if (p[i].r != c[i].r || p[i].g != c[i].g || p[i].b != c[i].b)
                        { colorChanged++; if (examples.Length < 600) examples += $" RGB[{i}]={c[i]}->{p[i]}"; }
                        if (p[i].a != r[i].r)
                        { roughChanged++; if (examples.Length < 600) examples += $" A[{i}]={r[i].r}->{p[i].a}"; }
                        expected[i].a = r[i].r;
                    }
                    fresh.SetPixels32(expected); fresh.Apply(false, false);
                    ImageConversion.LoadImage(roundtrip, fresh.EncodeToPNG(), false);
                    var rt = roundtrip.GetPixels32();
                    for (var i = 0; i < rt.Length; i++) if (!rt[i].Equals(expected[i])) roundtripChanged++;
                    string Digest(byte[] bytes)
                    {
                        using var sha = SHA256.Create();
                        return BitConverter.ToString(sha.ComputeHash(bytes)).Replace("-", "").ToLowerInvariant();
                    }
                    Debug.Log($"MC_TERRAIN_DECODE layer={layer} repeat={repeat} formats={color.format},{rough.format},{packed.format} source_raw={Digest(color.GetRawTextureData())} rough_raw={Digest(rough.GetRawTextureData())} packed_raw={Digest(packed.GetRawTextureData())} repeated_reads_changed={readsChanged} color_pixels_changed={colorChanged} rough_pixels_changed={roughChanged} png_roundtrip_changed={roundtripChanged} examples={examples}");
                }
                finally
                {
                    UnityEngine.Object.DestroyImmediate(color); UnityEngine.Object.DestroyImmediate(rough);
                    UnityEngine.Object.DestroyImmediate(packed); UnityEngine.Object.DestroyImmediate(fresh); UnityEngine.Object.DestroyImmediate(roundtrip);
                }
            }
        }
    }
}
