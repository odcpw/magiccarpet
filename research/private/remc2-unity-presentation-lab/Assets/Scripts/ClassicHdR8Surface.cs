using System;
using System.IO;
using UnityEngine;

namespace MagicCarpet.PresentationLab
{
    /// <summary>
    /// Persistent indexed presentation surface.  The byte planes are caller
    /// owned and reused; Upload does not allocate a Texture2D or managed buffer.
    /// </summary>
    public sealed class ClassicHdR8Surface : IDisposable
    {
        private Texture2D indexTexture;
        private Texture2D paletteTexture;
        private Material material;
        private byte[] rowOrientedBytes;
        private byte[] paletteRgbaBytes;
        private string externalPalettePath;

        public int Width { get; private set; }
        public int Height { get; private set; }
        public int Stride { get; private set; }
        public int AllocationEpoch { get; private set; }
        public int UploadCount { get; private set; }
        public Texture2D IndexTexture { get { return indexTexture; } }
        public Texture2D PaletteTexture { get { return paletteTexture; } }
        public Material Material { get { return material; } }

        public bool Configure(ClassicHdPresentationSettings settings, out string reason)
        {
            reason = string.Empty;
            if (settings == null)
            {
                reason = "Classic HD surface settings are null";
                return false;
            }

            string validationError;
            if (!settings.TryValidate(out validationError))
            {
                reason = validationError;
                return false;
            }

            ClassicHdProfileInfo profileInfo;
            if (!ClassicHdPresentationSettings.TryGetProfile(settings.profile, out profileInfo))
            {
                reason = "Classic HD surface profile is unknown";
                return false;
            }

            if (indexTexture != null && Width == profileInfo.Width && Height == profileInfo.Height)
            {
                indexTexture.filterMode = settings.outputFilter.Equals("bilinear", StringComparison.OrdinalIgnoreCase)
                    ? FilterMode.Bilinear
                    : FilterMode.Point;
                return true;
            }

            ReleaseUnityObjects();
            Width = profileInfo.Width;
            Height = profileInfo.Height;
            Stride = profileInfo.Stride;
            rowOrientedBytes = new byte[Stride * Height];
            indexTexture = new Texture2D(Width, Height, TextureFormat.R8, false, true);
            indexTexture.name = "ClassicHd_R8_IndexSurface";
            indexTexture.filterMode = settings.outputFilter.Equals("bilinear", StringComparison.OrdinalIgnoreCase)
                ? FilterMode.Bilinear
                : FilterMode.Point;
            indexTexture.wrapMode = TextureWrapMode.Clamp;
            indexTexture.anisoLevel = 0;

            paletteTexture = new Texture2D(256, 1, TextureFormat.RGBA32, false, true);
            paletteTexture.name = "ClassicHd_OwnerPalette_Resolved";
            paletteTexture.filterMode = FilterMode.Point;
            paletteTexture.wrapMode = TextureWrapMode.Clamp;
            Color32[] palette = BuildNightPalette();
            paletteTexture.SetPixels32(palette);
            paletteTexture.Apply(false, false);
            paletteRgbaBytes = new byte[256 * 4];
            externalPalettePath = string.Empty;

            Shader shader = Resources.Load<Shader>("ClassicHdIndexedUnlit");
            if (shader == null)
            {
                shader = Shader.Find("MagicCarpet/ClassicHd/IndexedUnlit");
            }

            if (shader == null)
            {
                reason = "Classic HD indexed palette shader is unavailable";
                ReleaseUnityObjects();
                return false;
            }

            material = new Material(shader);
            material.name = "ClassicHd_IndexedPalette_Material";
            material.SetTexture("_IndexTex", indexTexture);
            material.SetTexture("_PaletteTex", paletteTexture);
            AllocationEpoch++;
            return true;
        }

        public bool UploadTopRowFirst(byte[] source, int sourceStride, out string reason)
        {
            reason = string.Empty;
            if (indexTexture == null || rowOrientedBytes == null)
            {
                reason = "Classic HD surface is not configured";
                return false;
            }

            if (source == null || sourceStride < Width || source.Length < sourceStride * Height)
            {
                reason = "Classic HD source plane is smaller than the configured profile";
                return false;
            }

            if (!TryApplyExternalOwnerPalette(out reason))
            {
                return false;
            }

            for (int sourceRow = 0; sourceRow < Height; sourceRow++)
            {
                int destinationRow = Height - sourceRow - 1;
                Buffer.BlockCopy(source, sourceRow * sourceStride, rowOrientedBytes, destinationRow * Stride, Width);
            }

            indexTexture.SetPixelData(rowOrientedBytes, 0);
            indexTexture.Apply(false, false);
            UploadCount++;
            return true;
        }

        private bool TryApplyExternalOwnerPalette(out string reason)
        {
            reason = string.Empty;
            string assetCachePath = Environment.GetEnvironmentVariable("MC_ASSET_CACHE_PATH");
            if (string.IsNullOrWhiteSpace(assetCachePath))
            {
                return true;
            }

            string palettePath = Path.Combine(assetCachePath, "palette", "paln.rgb8");
            if (string.Equals(palettePath, externalPalettePath, StringComparison.Ordinal))
            {
                return true;
            }

            if (!File.Exists(palettePath))
            {
                reason = "Classic HD owner palette is missing: " + palettePath;
                return false;
            }

            byte[] paletteRgb8;
            try
            {
                paletteRgb8 = File.ReadAllBytes(palettePath);
            }
            catch (Exception exception)
            {
                reason = "Classic HD owner palette could not be read: " + exception.Message;
                return false;
            }

            if (paletteRgb8.Length != 256 * 3)
            {
                reason = "Classic HD owner palette must contain exactly 768 RGB8 bytes";
                return false;
            }

            for (int index = 0; index < 256; index++)
            {
                int source = index * 3;
                int destination = index * 4;
                paletteRgbaBytes[destination] = paletteRgb8[source];
                paletteRgbaBytes[destination + 1] = paletteRgb8[source + 1];
                paletteRgbaBytes[destination + 2] = paletteRgb8[source + 2];
                paletteRgbaBytes[destination + 3] = 255;
            }

            paletteTexture.SetPixelData(paletteRgbaBytes, 0);
            paletteTexture.Apply(false, false);
            externalPalettePath = palettePath;
            return true;
        }

        public void Clear()
        {
            if (rowOrientedBytes == null || indexTexture == null)
            {
                return;
            }

            Array.Clear(rowOrientedBytes, 0, rowOrientedBytes.Length);
            indexTexture.SetPixelData(rowOrientedBytes, 0);
            indexTexture.Apply(false, false);
        }

        public void Dispose()
        {
            ReleaseUnityObjects();
            rowOrientedBytes = null;
            paletteRgbaBytes = null;
            externalPalettePath = string.Empty;
            Width = 0;
            Height = 0;
            Stride = 0;
        }

        private static Color32[] BuildNightPalette()
        {
            Color32[] palette = new Color32[256];
            for (int index = 0; index < palette.Length; index++)
            {
                byte value = (byte)index;
                byte red = (byte)Mathf.Min(255, 4 + index * 3 / 4);
                byte green = (byte)Mathf.Min(255, 8 + index * 2 / 3);
                byte blue = (byte)Mathf.Min(255, 18 + index);
                if (index > 220)
                {
                    red = (byte)Mathf.Min(255, 130 + (index - 220) * 5);
                    green = (byte)Mathf.Min(255, 92 + (index - 220) * 6);
                    blue = (byte)Mathf.Min(255, 44 + (index - 220) * 7);
                }

                palette[index] = new Color32(red, green, blue, value == 0 ? (byte)255 : (byte)255);
            }

            return palette;
        }

        private void ReleaseUnityObjects()
        {
            DestroyUnityObject(material);
            DestroyUnityObject(paletteTexture);
            DestroyUnityObject(indexTexture);
            material = null;
            paletteTexture = null;
            indexTexture = null;
        }

        private static void DestroyUnityObject(UnityEngine.Object unityObject)
        {
            if (unityObject == null)
            {
                return;
            }

            if (Application.isPlaying)
            {
                UnityEngine.Object.Destroy(unityObject);
            }
            else
            {
                UnityEngine.Object.DestroyImmediate(unityObject);
            }
        }
    }
}
