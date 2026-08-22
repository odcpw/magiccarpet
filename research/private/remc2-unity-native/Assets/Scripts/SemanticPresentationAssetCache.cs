using System;
using UnityEngine;

namespace MagicCarpet.Native
{
    /// <summary>
    /// Actual Unity texture boundary for neutral content-addressed indexed assets.
    /// Cache identity is the immutable definition plus semantic slot and content hash.
    /// </summary>
    internal sealed class SemanticPresentationAssetCache : IDisposable
    {
        private string identity;
        private Texture2D texture;

        internal int InvalidationCount { get; private set; }

        internal Texture2D Bind(EngineSessionTransport.PresentationAssetObservation observation)
        {
            if (observation?.identity == null || observation.value == null)
            {
                throw new ArgumentNullException(nameof(observation));
            }
            var asset = observation.value;
            var nextIdentity = observation.identity.definition_hash + "\0" + asset.slot_id +
                "\0" + asset.content_address;
            if (texture != null && identity == nextIdentity)
            {
                return texture;
            }
            var paletteColors = checked(asset.palette_rgba8.Length / 4);
            var pixelCount = checked(asset.width * asset.height * asset.frame_count);
            if (asset.palette_rgba8.Length == 0 || asset.palette_rgba8.Length % 4 != 0 ||
                asset.indices.Length != pixelCount || asset.frame_count != 1)
            {
                throw new InvalidOperationException("indexed asset shape is invalid for this texture boundary");
            }
            var rgba = new byte[checked(pixelCount * 4)];
            for (var index = 0; index < pixelCount; index++)
            {
                var paletteIndex = asset.indices[index];
                if (paletteIndex >= paletteColors)
                {
                    throw new InvalidOperationException("indexed asset references a missing palette color");
                }
                Buffer.BlockCopy(asset.palette_rgba8, paletteIndex * 4, rgba, index * 4, 4);
            }
            var replacement = new Texture2D(asset.width, asset.height, TextureFormat.RGBA32, false)
            {
                filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Clamp,
                name = asset.slot_id + "@" + asset.content_address,
            };
            replacement.LoadRawTextureData(rgba);
            replacement.Apply(false, true);
            var staleTexture = texture;
            if (staleTexture != null)
            {
                UnityEngine.Object.DestroyImmediate(staleTexture);
                InvalidationCount += 1;
            }
            texture = replacement;
            identity = nextIdentity;
            return texture;
        }

        public void Dispose()
        {
            if (texture != null)
            {
                UnityEngine.Object.DestroyImmediate(texture);
                texture = null;
            }
            identity = null;
        }
    }
}
