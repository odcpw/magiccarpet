using System;
using System.Collections.Generic;
using System.IO;
using System.Security.Cryptography;
using UnityEngine;

namespace MagicCarpet.Native
{
    /// <summary>
    /// Presentation-only projection of the Rust-owned outer runtime mode.
    /// The qualified owner-cache reset frame is visible only while Rust says
    /// the runtime is loading. This class neither advances the runtime nor
    /// chooses a destination or transition duration.
    /// </summary>
    internal sealed class CampaignTransitionPresenter : IDisposable
    {
        internal const int FrameWidth = 320;
        internal const int FrameHeight = 200;
        internal const string IndicesPath =
            "frame/reset-loaded-main-320x200.indices.u8";
        internal const string PalettePath =
            "frame/reset-loaded-main.palette.rgb6";
        private static readonly HashSet<string> KnownCueIds = new(
            StringComparer.Ordinal)
        {
            GameRuntimeTransport.TitlePresentationCue,
            "presentation.campaign.map",
            "presentation.magic_carpet_2.night_spire.loading",
            "presentation.magic_carpet_2.night_spire.entry",
            "presentation.magic_carpet_2.night_spire.completion",
            "presentation.magic_carpet_2.payahandra_ruins.loading",
            "presentation.magic_carpet_2.payahandra_ruins.entry",
            "presentation.magic_carpet_2.payahandra_ruins.completion",
            "presentation.magic_carpet_2.revealed_way.loading",
            "presentation.magic_carpet_2.revealed_way.entry",
            "presentation.magic_carpet_2.revealed_way.completion"
        };

        private GameObject root;
        private Mesh mesh;
        private Texture2D indicesTexture;
        private Texture2D paletteTexture;
        private Material material;

        internal GameObject Root => root;
        internal Texture2D IndicesTexture => indicesTexture;
        internal Texture2D PaletteTexture => paletteTexture;
        internal Material Material => material;
        internal bool IsVisible => root != null && root.activeSelf;
        internal string IndicesSha256 { get; private set; }
        internal string PaletteRgb6Sha256 { get; private set; }
        internal string PaletteRgb8Sha256 { get; private set; }
        internal string LastPresentedCueId { get; private set; }
        internal string[] LastPresentationSequence { get; private set; } =
            Array.Empty<string>();

        internal CampaignTransitionPresenter(
            Transform parent,
            OwnerAssetCache ownerCache)
            : this(
                parent,
                ReadRequired(ownerCache, IndicesPath),
                ReadRequired(ownerCache, PalettePath))
        {
        }

        internal CampaignTransitionPresenter(
            Transform parent,
            byte[] topDownIndices,
            byte[] paletteRgb6)
        {
            if (topDownIndices == null ||
                topDownIndices.Length != FrameWidth * FrameHeight)
            {
                throw new InvalidDataException(
                    "campaign loading frame must contain exactly 320x200 indexed pixels");
            }
            if (paletteRgb6 == null || paletteRgb6.Length != 256 * 3)
            {
                throw new InvalidDataException(
                    "campaign loading frame palette must contain exactly 256 RGB6 rows");
            }

            var paletteRgb8 = new byte[paletteRgb6.Length];
            for (var index = 0; index < paletteRgb6.Length; index++)
            {
                if (paletteRgb6[index] > 63)
                {
                    throw new InvalidDataException(
                        "campaign loading frame palette contains a non-RGB6 channel");
                }
                paletteRgb8[index] = checked((byte)(paletteRgb6[index] * 4));
            }

            IndicesSha256 = Sha256(topDownIndices);
            PaletteRgb6Sha256 = Sha256(paletteRgb6);
            PaletteRgb8Sha256 = Sha256(paletteRgb8);

            var shader = Resources.Load<Shader>("TerrainIndexedFrame");
            if (shader == null)
            {
                throw new InvalidOperationException(
                    "campaign loading frame indexed palette shader is unavailable");
            }

            try
            {
                root = new GameObject("MC2 owner loading frame presentation");
                root.transform.SetParent(parent, false);
                root.transform.localPosition = Vector3.zero;
                root.transform.localScale = new Vector3(3.2f, 2f, 1f);

                mesh = CreateQuad();
                root.AddComponent<MeshFilter>().sharedMesh = mesh;
                var renderer = root.AddComponent<MeshRenderer>();
                renderer.shadowCastingMode =
                    UnityEngine.Rendering.ShadowCastingMode.Off;
                renderer.receiveShadows = false;

                indicesTexture = new Texture2D(
                    FrameWidth,
                    FrameHeight,
                    TextureFormat.R8,
                    false,
                    true)
                {
                    name = "MC2 SMATITLE loading indices",
                    filterMode = FilterMode.Point,
                    wrapMode = TextureWrapMode.Clamp
                };
                indicesTexture.LoadRawTextureData(topDownIndices);
                indicesTexture.Apply(false, true);

                paletteTexture = new Texture2D(
                    256,
                    1,
                    TextureFormat.RGB24,
                    false,
                    false)
                {
                    name = "MC2 SMATITLE loading RGB8 palette",
                    filterMode = FilterMode.Point,
                    wrapMode = TextureWrapMode.Clamp
                };
                paletteTexture.LoadRawTextureData(paletteRgb8);
                paletteTexture.Apply(false, true);

                material = new Material(shader)
                {
                    name = "MC2 SMATITLE indexed loading material"
                };
                material.SetTexture("_Indices", indicesTexture);
                material.SetTexture("_Coverage", Texture2D.whiteTexture);
                material.SetTexture("_Palette", paletteTexture);
                renderer.sharedMaterial = material;

                // Rust starts in title mode. Remain hidden until the first
                // observed loading identity says otherwise.
                root.SetActive(false);
            }
            catch
            {
                Dispose();
                throw;
            }
        }

        internal void Present(GameRuntimeTransport.Identity identity)
        {
            if (root == null)
            {
                throw new ObjectDisposedException(nameof(CampaignTransitionPresenter));
            }
            GameRuntimeTransport.ValidateIdentityPresentation(identity);
            LastPresentationSequence = new[] { identity.presentation.cue_id };
            ApplyCue(identity.presentation);
        }

        internal void Present(
            GameRuntimeTransport.CompletionTransitionReceipt receipt)
        {
            if (root == null)
            {
                throw new ObjectDisposedException(nameof(CampaignTransitionPresenter));
            }
            GameRuntimeTransport.ValidateCompletionPresentationSequence(receipt);
            LastPresentationSequence = Array.ConvertAll(
                receipt.presentation_sequence,
                cue => cue.cue_id);
            foreach (var cue in receipt.presentation_sequence)
            {
                ApplyCue(cue);
            }
        }

        private void ApplyCue(GameRuntimeTransport.RuntimePresentationCue cue)
        {
            GameRuntimeTransport.ValidatePresentationCue(cue);
            if (!KnownCueIds.Contains(cue.cue_id))
            {
                throw new InvalidDataException(
                    "campaign transition presenter has no binding for Rust cue " +
                    cue.cue_id);
            }
            LastPresentedCueId = cue.cue_id;
            root.SetActive(
                cue.phase == GameRuntimeTransport.LoadingPresentationPhase);
        }

        public void Dispose()
        {
            DestroyObject(material);
            DestroyObject(indicesTexture);
            DestroyObject(paletteTexture);
            DestroyObject(mesh);
            DestroyObject(root);
            material = null;
            indicesTexture = null;
            paletteTexture = null;
            mesh = null;
            root = null;
        }

        private static byte[] ReadRequired(
            OwnerAssetCache ownerCache,
            string relativePath)
        {
            if (ownerCache == null)
            {
                throw new ArgumentNullException(nameof(ownerCache));
            }
            return ownerCache.ReadOutput(relativePath);
        }

        private static Mesh CreateQuad()
        {
            var value = new Mesh
            {
                name = "MC2 owner loading frame quad",
                vertices = new[]
                {
                    new Vector3(-0.5f, -0.5f, 0f),
                    new Vector3(0.5f, -0.5f, 0f),
                    new Vector3(-0.5f, 0.5f, 0f),
                    new Vector3(0.5f, 0.5f, 0f)
                },
                // Source bytes are top-down. Preserve them byte-for-byte and
                // flip only the sampling coordinates at the presentation edge.
                uv = new[]
                {
                    new Vector2(0f, 1f),
                    new Vector2(1f, 1f),
                    new Vector2(0f, 0f),
                    new Vector2(1f, 0f)
                },
                triangles = new[] { 0, 2, 1, 2, 3, 1 }
            };
            value.RecalculateBounds();
            return value;
        }

        private static string Sha256(byte[] bytes)
        {
            using var sha256 = SHA256.Create();
            var digest = sha256.ComputeHash(bytes);
            return BitConverter.ToString(digest).Replace("-", "").ToLowerInvariant();
        }

        private static void DestroyObject(UnityEngine.Object value)
        {
            if (value == null) return;
            if (Application.isPlaying)
            {
                UnityEngine.Object.Destroy(value);
            }
            else
            {
                UnityEngine.Object.DestroyImmediate(value);
            }
        }
    }
}
