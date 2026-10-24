using System;
using System.IO;
using System.Security.Cryptography;
using UnityEngine;

namespace MagicCarpet.Native
{
    /// <summary>
    /// Presentation-only projection of the Rust-owned outer runtime mode.
    /// Destination-bound source art is visible only while Rust says the
    /// runtime is loading. This class neither advances the runtime nor
    /// chooses a destination or transition duration.
    /// </summary>
    internal sealed class CampaignTransitionPresenter : IDisposable
    {
        /// <summary>
        /// Presentation scheduling guard for one resolver-owned Loading
        /// identity. It can delay an ordinary open until a render boundary has
        /// observed the loading frame, but it cannot select a destination or
        /// mutate the Rust runtime.
        /// </summary>
        internal sealed class LoadingGate
        {
            private string runtimeId;
            private string loadingGeneration;
            private string destinationLevelId;
            private string definitionHash, packageHash, definitionRevision;
            private bool frameObserved;
            private bool openClaimed;

            internal bool IsPending => !string.IsNullOrEmpty(runtimeId);

            internal void Begin(
                GameRuntimeTransport.Identity loading,
                string destination)
            {
                if (IsPending ||
                    loading?.mode != GameRuntimeTransport.LoadingMode ||
                    string.IsNullOrWhiteSpace(loading.runtime_id) ||
                    string.IsNullOrWhiteSpace(loading.generation) ||
                    loading.pending_definition?.runtime_flow?.bound_level_id != destination ||
                    string.IsNullOrWhiteSpace(loading.pending_definition?.definition_hash) ||
                    string.IsNullOrWhiteSpace(loading.pending_definition.package_graph_hash) ||
                    string.IsNullOrWhiteSpace(loading.pending_definition.definition_revision) ||
                    !string.IsNullOrEmpty(loading.session?.session_id) ||
                    string.IsNullOrWhiteSpace(destination))
                {
                    throw new InvalidOperationException(
                        "loading presentation gate requires one resolver-bound Loading identity");
                }
                runtimeId = loading.runtime_id;
                loadingGeneration = loading.generation;
                destinationLevelId = destination;
                definitionHash = loading.pending_definition.definition_hash;
                packageHash = loading.pending_definition.package_graph_hash;
                definitionRevision = loading.pending_definition.definition_revision;
                frameObserved = false;
                openClaimed = false;
            }

            internal void Observe(
                GameRuntimeTransport.Identity loading,
                bool presentationVisible)
            {
                ValidateLoading(loading, presentationVisible);
                frameObserved = true;
            }

            internal bool TryClaimOpen(
                GameRuntimeTransport.Identity loading,
                bool presentationVisible)
            {
                ValidateLoading(loading, presentationVisible);
                if (!frameObserved || openClaimed)
                {
                    return false;
                }
                openClaimed = true;
                return true;
            }

            internal void Complete(GameRuntimeTransport.Identity playing)
            {
                if (!IsPending || !openClaimed ||
                    playing?.mode != GameRuntimeTransport.PlayingMode ||
                    playing.runtime_id != runtimeId ||
                    GameRuntimeTransport.ParseCounter(playing.generation, "generation") !=
                        checked(GameRuntimeTransport.ParseCounter(loadingGeneration, "generation") + 1) ||
                    playing.active_runtime_flow?.bound_level_id != destinationLevelId ||
                    playing.session?.definition_hash != definitionHash ||
                    playing.session.package_graph_hash != packageHash ||
                    playing.session.definition_revision != definitionRevision ||
                    playing.session.tick != "0" ||
                    string.IsNullOrWhiteSpace(playing.session?.session_id))
                {
                    throw new InvalidOperationException(
                        "loading presentation gate completion is not resolver-bound Playing");
                }
                Reset();
            }

            internal void Reset()
            {
                runtimeId = null;
                loadingGeneration = null;
                destinationLevelId = null;
                definitionHash = packageHash = definitionRevision = null;
                frameObserved = false;
                openClaimed = false;
            }

            private void ValidateLoading(
                GameRuntimeTransport.Identity loading,
                bool presentationVisible)
            {
                if (!IsPending || openClaimed || !presentationVisible ||
                    loading?.mode != GameRuntimeTransport.LoadingMode ||
                    loading.runtime_id != runtimeId ||
                    loading.generation != loadingGeneration ||
                    loading.pending_definition?.runtime_flow?.bound_level_id !=
                        destinationLevelId ||
                    loading.pending_definition.definition_hash != definitionHash ||
                    loading.pending_definition.package_graph_hash != packageHash ||
                    loading.pending_definition.definition_revision != definitionRevision ||
                    !string.IsNullOrEmpty(loading.session?.session_id))
                {
                    throw new InvalidOperationException(
                        "loading presentation gate lost its exact resolver identity");
                }
            }
        }

        internal const int FrameWidth = 320;
        internal const int FrameHeight = 200;
        internal const string IndicesPath =
            "frame/reset-loaded-main-320x200.indices.u8";
        internal const string PalettePath =
            "frame/reset-loaded-main.palette.rgb6";
        [Serializable] private sealed class CacheManifest
        { public string schema; public FrameBinding[] frames; public CacheBindings bindings; public Asset[] outputs; }
        [Serializable] private sealed class CacheBindings
        { public string campaign_package_sha256, level_export_package_sha256; }
        [Serializable] private sealed class PackageManifest
        { public string package_sha256; }
        [Serializable] private sealed class FrameBinding
        { public string profile; public int width, height; public Asset indices, palette_rgb6; }
        [Serializable] private sealed class Asset
        { public string path; public string sha256; public long bytes; }
        private sealed class FrameBytes
        { internal byte[] Indices, Palette; }

        private readonly Func<GameRuntimeTransport.LoadingPresentationBinding, FrameBytes> resolveFrame;
        private readonly Transform parent;
        private string boundRuntime;
        private string boundGeneration;
        private string boundDefinition;
        private bool disposed;
        internal enum RecoveryAction { None, Retry, ReturnMenu, Quit }
        private RecoveryAction recoveryAction;
        private Vector2 failureScroll;
        internal string Failure { get; private set; }
        internal bool HasFailure => Failure != null;

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
        internal GameRuntimeTransport.LoadingPresentationBinding Destination { get; private set; }
        internal GameRuntimeTransport.LoadingPresentationBinding FailedDestination { get; private set; }
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
            this.parent = parent;
            ValidateFrame(topDownIndices, paletteRgb6);
            resolveFrame = binding => binding.artwork_profile == "ordinary"
                ? new FrameBytes { Indices = topDownIndices, Palette = paletteRgb6 }
                : throw new InvalidDataException("this cache has no secret loading artwork");
            CreateSurface(parent);
            try { BindFrame(topDownIndices, paletteRgb6); }
            catch { Dispose(); throw; }
        }

        internal CampaignTransitionPresenter(Transform parent, string loadingAssetCache)
        {
            this.parent = parent;
            if (string.IsNullOrWhiteSpace(loadingAssetCache))
                throw new ArgumentException("loading asset cache path is required", nameof(loadingAssetCache));
            resolveFrame = binding => ReadFrame(loadingAssetCache, binding.artwork_profile);
        }

        private static void ValidateFrame(byte[] topDownIndices, byte[] paletteRgb6)
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

            for (var index = 0; index < paletteRgb6.Length; index++)
            {
                if (paletteRgb6[index] > 63)
                {
                    throw new InvalidDataException(
                        "campaign loading frame palette contains a non-RGB6 channel");
                }
            }
        }

        private void CreateSurface(Transform parent)
        {
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

                material = new Material(shader)
                { name = "MC2 destination loading material" };
                renderer.sharedMaterial = material;
                root.SetActive(false);
            }
            catch { Hide(); ReleaseSurface(); throw; }
        }

        private void BindFrame(byte[] topDownIndices, byte[] paletteRgb6)
        {
            ValidateFrame(topDownIndices, paletteRgb6);
            var paletteRgb8 = Array.ConvertAll(paletteRgb6, value => checked((byte)(value * 4)));
            Texture2D candidateIndices = null, candidatePalette = null;
            try
            {
                candidateIndices = new Texture2D(
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
                candidateIndices.LoadRawTextureData(topDownIndices);
                candidateIndices.Apply(false, true);

                candidatePalette = new Texture2D(
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
                candidatePalette.LoadRawTextureData(paletteRgb8);
                candidatePalette.Apply(false, true);
                material.SetTexture("_Indices", candidateIndices);
                material.SetTexture("_Coverage", Texture2D.whiteTexture);
                material.SetTexture("_Palette", candidatePalette);
                DestroyObject(indicesTexture);
                DestroyObject(paletteTexture);
                indicesTexture = candidateIndices;
                paletteTexture = candidatePalette;
                candidateIndices = candidatePalette = null;
                IndicesSha256 = Sha256(topDownIndices);
                PaletteRgb6Sha256 = Sha256(paletteRgb6);
                PaletteRgb8Sha256 = Sha256(paletteRgb8);
            }
            finally
            {
                DestroyObject(candidateIndices);
                DestroyObject(candidatePalette);
            }
        }

        internal void Present(GameRuntimeTransport.Identity identity)
        {
            if (disposed)
            {
                throw new ObjectDisposedException(nameof(CampaignTransitionPresenter));
            }
            GameRuntimeTransport.ValidateIdentityPresentation(identity);
            LastPresentationSequence = new[] { identity.presentation.cue_id };
            if (identity.mode != GameRuntimeTransport.LoadingMode)
            {
                Hide();
                LastPresentedCueId = identity.presentation.cue_id;
                return;
            }
            if (boundRuntime == identity.runtime_id && boundGeneration == identity.generation &&
                boundDefinition == identity.pending_definition?.definition_hash)
            {
                root.SetActive(true);
                return;
            }
            Hide();
            Present(identity, GameRuntimeTransport.ReadLoadingPresentation(identity));
        }

        internal void Present(GameRuntimeTransport.Identity identity,
            GameRuntimeTransport.LoadingPresentationBinding binding)
        {
            if (disposed) throw new ObjectDisposedException(nameof(CampaignTransitionPresenter));
            Hide();
            GameRuntimeTransport.ValidateLoadingPresentation(identity, binding);
            var frame = resolveFrame(binding);
            if (root == null) CreateSurface(parent);
            BindFrame(frame.Indices, frame.Palette);
            Destination = binding;
            boundRuntime = identity.runtime_id;
            boundGeneration = identity.generation;
            boundDefinition = binding.definition_hash;
            LastPresentedCueId = binding.cue_id;
            LastPresentationSequence = new[] { binding.cue_id };
            Failure = null;
            root.SetActive(true);
        }

        internal void Present(
            GameRuntimeTransport.CompletionTransitionReceipt receipt)
        {
            if (disposed)
            {
                throw new ObjectDisposedException(nameof(CampaignTransitionPresenter));
            }
            GameRuntimeTransport.ValidateCompletionPresentationSequence(receipt);
            LastPresentationSequence = Array.ConvertAll(
                receipt.presentation_sequence,
                cue => cue.cue_id);
            var sequence = LastPresentationSequence;
            Present(receipt.after);
            LastPresentationSequence = sequence;
        }

        internal void Hide()
        {
            if (root != null) root.SetActive(false);
            Destination = null;
            boundRuntime = boundGeneration = boundDefinition = null;
        }

        internal void Failed(Exception error)
        {
            FailedDestination = Destination ?? FailedDestination;
            Hide();
            Failure = error?.Message ?? "The destination could not be opened.";
            recoveryAction = RecoveryAction.None;
        }

        internal void ClearFailure() { Failure = null; FailedDestination = null; recoveryAction = RecoveryAction.None; }

        internal bool TryTakeRecoveryAction(out RecoveryAction action)
        {
            action = recoveryAction;
            recoveryAction = RecoveryAction.None;
            return action != RecoveryAction.None;
        }

        internal void DrawFailure()
        {
            if (!HasFailure) return;
            var width = Mathf.Min(620, Screen.width - 32);
            GUILayout.BeginArea(new Rect((Screen.width - width) * .5f,
                Mathf.Max(16, (Screen.height - 280) * .5f), width, 280), GUI.skin.box);
            GUILayout.Label("Unable to load the selected mission");
            failureScroll = GUILayout.BeginScrollView(failureScroll, GUILayout.Height(95));
            GUILayout.Label(Failure, new GUIStyle(GUI.skin.label) { wordWrap = true });
            GUILayout.EndScrollView();
            GUILayout.FlexibleSpace();
            if (GUILayout.Button("Retry selected mission", GUILayout.Height(38))) recoveryAction = RecoveryAction.Retry;
            if (GUILayout.Button("Return to menu", GUILayout.Height(38))) recoveryAction = RecoveryAction.ReturnMenu;
            if (GUILayout.Button("Quit", GUILayout.Height(38))) recoveryAction = RecoveryAction.Quit;
            if (Event.current.type == EventType.KeyUp && Event.current.keyCode == KeyCode.Escape)
            { recoveryAction = RecoveryAction.ReturnMenu; Event.current.Use(); }
            GUILayout.EndArea();
        }

        public void Dispose()
        {
            disposed = true;
            Hide();
            ReleaseSurface();
        }

        private void ReleaseSurface()
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

        private static FrameBytes ReadFrame(string directory, string profile)
        {
            var cache = Path.GetFullPath(directory);
            var manifestPath = ConfinedFile(cache, "manifest.json");
            var manifest = JsonUtility.FromJson<CacheManifest>(
                System.Text.Encoding.UTF8.GetString(ReadBounded(manifestPath, 64 * 1024)));
            if (manifest?.schema != "mc2-loading-asset-cache/v1" || manifest.frames == null)
                throw new InvalidDataException("loading asset cache schema or frames are unavailable");
            var matches = Array.FindAll(manifest.frames, row => row?.profile == profile);
            if (matches.Length != 1 || matches[0].width != FrameWidth || matches[0].height != FrameHeight)
                throw new InvalidDataException("loading asset cache has no unique source profile: " + profile);
            return new FrameBytes {
                Indices = ReadAsset(cache, matches[0].indices, FrameWidth * FrameHeight),
                Palette = ReadAsset(cache, matches[0].palette_rgb6, 256 * 3)
            };
        }

        internal static GameRuntimeTransport.InstalledPackageResource PreflightDestination(
            GameRuntimeTransport.LoadingPresentationBinding binding,
            GameRuntimeTransport.InstalledPackageResource[] packages)
        {
            GameRuntimeTransport.InstalledPackageResource match = null;
            foreach (var resource in packages ?? Array.Empty<GameRuntimeTransport.InstalledPackageResource>())
            {
                if (resource == null || string.IsNullOrWhiteSpace(resource.package_dir)) continue;
                var manifestPath = Path.Combine(resource.package_dir, "manifest.json");
                if (!File.Exists(manifestPath)) continue;
                var manifest = JsonUtility.FromJson<PackageManifest>(System.Text.Encoding.UTF8.GetString(
                    ReadBounded(ConfinedFile(resource.package_dir, "manifest.json"), 1024 * 1024)));
                if (manifest?.package_sha256 != binding.source_package_sha256) continue;
                if (match != null) throw new InvalidDataException("loading destination has duplicate installed package bindings");
                match = resource;
            }
            if (match == null) throw new FileNotFoundException("The selected mission package is missing or changed.");
            if (string.IsNullOrWhiteSpace(match.presentation_cache_path))
                throw new DirectoryNotFoundException("The selected mission has no presentation cache installed.");
            var rootPath = Path.GetFullPath(match.presentation_cache_path);
            var cache = JsonUtility.FromJson<CacheManifest>(System.Text.Encoding.UTF8.GetString(
                ReadBounded(ConfinedFile(rootPath, "manifest.json"), 16 * 1024 * 1024)));
            if (cache?.bindings?.level_export_package_sha256 != binding.owner_export_package_sha256 ||
                !string.IsNullOrEmpty(cache.bindings.campaign_package_sha256) &&
                cache.bindings.campaign_package_sha256 != binding.source_package_sha256 ||
                cache.outputs == null || cache.outputs.Length == 0 || cache.outputs.Length > 20000)
                throw new InvalidDataException("The presentation cache does not bind the selected mission package.");
            foreach (var output in cache.outputs)
            {
                if (output == null || output.bytes < 0 || output.sha256?.Length != 64)
                    throw new InvalidDataException("destination cache output metadata is invalid");
                using var stream = File.OpenRead(ConfinedFile(rootPath, output.path));
                using var sha = SHA256.Create();
                if (stream.Length != output.bytes ||
                    BitConverter.ToString(sha.ComputeHash(stream)).Replace("-", "").ToLowerInvariant() != output.sha256)
                    throw new InvalidDataException("A selected-mission presentation asset is missing or changed: " + output.path);
            }
            return match;
        }

        private static byte[] ReadAsset(string directory, Asset asset, int size)
        {
            if (asset == null || asset.bytes != size || asset.sha256?.Length != 64)
                throw new InvalidDataException("loading asset metadata has an invalid extent or digest");
            var bytes = ReadBounded(ConfinedFile(directory, asset.path), size);
            if (bytes.Length != size || Sha256(bytes) != asset.sha256)
                throw new InvalidDataException("loading asset bytes no longer match their cache binding");
            return bytes;
        }

        private static byte[] ReadBounded(string path, int maximum)
        {
            using var stream = File.OpenRead(path);
            if (stream.Length > maximum)
                throw new InvalidDataException("loading asset exceeds its bounded extent: " + path);
            var result = new byte[maximum + 1];
            var length = 0;
            while (length < result.Length)
            {
                var read = stream.Read(result, length, result.Length - length);
                if (read == 0) break;
                length += read;
            }
            if (length > maximum) throw new InvalidDataException("loading asset grew beyond its extent");
            Array.Resize(ref result, length);
            return result;
        }

        private static string ConfinedFile(string directory, string relative)
        {
            if (string.IsNullOrWhiteSpace(relative) || Path.IsPathRooted(relative) || relative.Contains('\\'))
                throw new InvalidDataException("loading asset path must be relative to its cache");
            var rootPath = Path.GetFullPath(directory);
            var path = rootPath;
            if ((File.GetAttributes(rootPath) & FileAttributes.ReparsePoint) != 0)
                throw new InvalidDataException("loading asset cache cannot be a symbolic link");
            var parts = relative.Split('/');
            foreach (var part in parts)
            {
                if (part.Length == 0 || part == "." || part == "..")
                    throw new InvalidDataException("loading asset path escapes its cache");
                path = Path.Combine(path, part);
                if ((File.GetAttributes(path) & FileAttributes.ReparsePoint) != 0)
                    throw new InvalidDataException("loading asset path cannot contain a symbolic link");
            }
            if (!File.Exists(path)) throw new FileNotFoundException("loading asset is missing", path);
            return path;
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
