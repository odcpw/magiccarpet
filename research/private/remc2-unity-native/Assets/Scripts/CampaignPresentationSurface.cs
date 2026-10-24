using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native
{
    internal sealed class Level3EntitySurfaceMarker : MonoBehaviour
    {
        internal ushort EntityIndex { get; private set; }
        internal ushort SourceId { get; private set; }
        internal byte ClassId { get; private set; }
        internal byte Model { get; private set; }
        internal byte Action { get; private set; }
        internal ulong Generation { get; private set; }
        internal ulong Tick { get; private set; }
        internal ushort AssetSelector { get; private set; }
        internal bool SpriteReady { get; private set; }
        internal ushort SourceX { get; private set; }
        internal ushort SourceY { get; private set; }
        internal short SourceZ { get; private set; }

        internal void Bind(
            NativeSim.CampaignEntitySnapshot row,
            ulong generation,
            ulong tick)
        {
            // A source slot can remain live while its presentation disappears,
            // or be reused for another family. Only this frame's Rust draw
            // admission may enable its sprite; a previous frame is not authority.
            SpriteReady = false;
            AssetSelector = 0;
            var visual = transform.Find("package sprite");
            if (visual != null)
                visual.GetComponent<MeshRenderer>().enabled = false;
            EntityIndex = row.entityIndex;
            SourceId = row.sourceId;
            ClassId = row.classId;
            Model = row.model;
            Action = row.action;
            Generation = generation;
            Tick = tick;
            SourceX = row.x;
            SourceY = row.y;
            SourceZ = row.z;
            transform.localPosition = new Vector3(
                row.x / 64f,
                row.z / 64f,
                row.y / 64f);
        }

        internal void BindSprite(
            NativeSim.CampaignEntityPresentation row,
            OwnerAssetCache.Level3Texture texture,
            Mesh quad,
            Material material)
        {
            var visual = transform.Find("package sprite");
            if (visual == null)
            {
                visual = new GameObject("package sprite").transform;
                visual.SetParent(transform, false);
                visual.gameObject.AddComponent<MeshFilter>();
                visual.gameObject.AddComponent<MeshRenderer>();
            }
            var height = row.logicalHeight / 64f;
            var width = height * texture.width / texture.height;
            visual.localPosition = new Vector3(0f, height * 0.5f, 0f);
            visual.localScale = new Vector3(row.mirrored == 0 ? width : -width, height, 1f);
            visual.GetComponent<MeshFilter>().sharedMesh = quad;
            var renderer = visual.GetComponent<MeshRenderer>();
            renderer.sharedMaterial = material;
            renderer.enabled = true;
            AssetSelector = row.selectedTmap;
            SpriteReady = true;
        }
    }

    /// <summary>
    /// A shared campaign Unity consumer. Rust supplies the live frame,
    /// entity and spell-inventory state; the sealed owner cache supplies only
    /// validated HUD/spell/minimap pixels. Rust supplies sprite selectors and
    /// draw admission; the client only projects the returned pose and pixels.
    /// </summary>
    internal sealed class CampaignPresentationSurface : IDisposable
    {
        private readonly Transform root;
        private readonly Dictionary<ushort, Level3EntitySurfaceMarker> entities = new();
        private readonly List<Texture2D> hudTextures = new();
        private readonly Dictionary<int, OwnerAssetCache.Level3Texture> materialRows = new();
        private readonly Dictionary<int, Texture2D> entityTextures = new();
        private readonly Dictionary<int, Material> entityMaterials = new();
        private GameObject worldObject;
        private Mesh worldMesh;
        private Camera frameCamera;
        private Camera entityCamera;
        private const int FrameLayer = 31;
        private const int EntityLayer = 30;
        private Material worldMaterial;
        private Texture2D worldIndices;
        private Texture2D worldPalette;
        private Texture2D hudPalette;
        private byte[] minimapTables;
        private CampaignMinimapOverlay minimapOverlay;
        private readonly byte[] presentedIndices = new byte[320 * 200];
        private readonly Dictionary<int, MinimapSymbolPixels> minimapSymbols = new();
        internal sealed class MinimapSymbolPixels
        {
            internal int Width;
            internal int Height;
            internal byte[] Indices;
            internal byte[] Mask;
        }
        private bool disposed;
        private bool night;
        private Func<byte[], byte[]> paletteTransform;
        private byte[] worldPaletteSource;
        private byte[] entityPaletteSource;

        /// Preferences transform retained source palette bytes; source caches
        /// and native indexed frames remain immutable presentation inputs.
        internal void SetPaletteTransform(Func<byte[], byte[]> transform)
        {
            paletteTransform = transform;
            RefreshPreferences();
        }

        internal void RefreshPreferences()
        {
            if (worldPalette != null && worldPaletteSource != null)
            {
                worldPalette.LoadRawTextureData(TransformPalette(worldPaletteSource));
                worldPalette.Apply(false, false);
            }
            if (hudPalette != null && entityPaletteSource != null)
            {
                hudPalette.LoadRawTextureData(TransformPalette(entityPaletteSource));
                hudPalette.Apply(false, false);
            }
        }

        private byte[] TransformPalette(byte[] source) => paletteTransform?.Invoke(source) ?? source;

        [Serializable]
        private sealed class NightMaterials
        {
            public OwnerAssetCache.Level3TextureBank tmaps1;
        }

        [Serializable]
        private sealed class NightHud
        {
            public NightHudBitmap[] bitmap_assets;
        }

        [Serializable]
        private sealed class NightHudBitmap
        {
            public int selector;
            public int stored_width;
            public int stored_height;
            public string indices_path;
            public string mask_path;
        }

        internal CampaignPresentationSurface(Transform parent = null)
        {
            root = new GameObject("Campaign Rust presentation surface").transform;
            if (parent != null) root.SetParent(parent, false);
        }

        internal Transform Root => root;
        internal ulong Generation { get; private set; }
        internal ulong Tick { get; private set; }
        internal ulong FrameHash { get; private set; }
        internal ulong TerrainHash { get; private set; }
        internal bool WorldReady => worldObject != null &&
                                    worldObject.GetComponent<MeshRenderer>()?.enabled == true &&
                                    frameCamera != null;
        internal Camera FrameCamera => frameCamera;
        internal bool HudReady { get; private set; }
        internal bool MinimapPanelReady { get; private set; }
        internal bool LiveMinimapReady { get; private set; }
        internal int EntitySurfaceCount => entities.Count;
        internal int RenderedEntitySurfaceCount => entities.Values.Count(row => row.SpriteReady);
        internal int HudBitmapSurfaceCount { get; private set; }
        internal int SpellTierSurfaceCount { get; private set; }
        internal int LiveSpellSurfaceCount { get; private set; }
        internal string CacheContentHash { get; private set; }

        internal void PresentNight(NativeCampaignSession session, OwnerAssetCache cache, short cameraYaw)
        {
            if (disposed) throw new ObjectDisposedException(nameof(CampaignPresentationSurface));
            if (session == null || cache == null || cache.IsDayAggregate ||
                session.Selection.externalLevel != 1 || session.Selection.internalLevel != 0 ||
                session.Selection.mapType != 1)
                throw new InvalidOperationException("Night presentation requires the qualified Night campaign and cache");
            night = true;
            var info = session.CopyPresentationFrame(session.Tick);
            if (materialRows.Count == 0)
            {
                var descriptor = JsonUtility.FromJson<NightMaterials>(cache.ReadOutputText("terrain/materials.json"));
                if (descriptor?.tmaps1?.textures == null)
                    throw new InvalidOperationException("Night sprite descriptor is missing");
                foreach (var row in descriptor.tmaps1.textures)
                    if (!string.IsNullOrEmpty(row.indices_path)) materialRows.Add(row.index, row);
            }
            var generation = session.Generation;
            var tick = session.Tick;
            var minimap = session.MinimapState(tick);
            var minimapEntities = session.MinimapEntities(tick, minimap);
            var minimapMarks = session.MinimapMarks(tick, minimap);
            minimapTables ??= cache.ReadOutput("palette/tablesn.u8");
            if (minimapSymbols.Count == 0)
            {
                var hud = JsonUtility.FromJson<NightHud>(cache.ReadOutputText("hud/level1.json"));
                if (hud?.bitmap_assets == null)
                    throw new InvalidOperationException("Night minimap symbol descriptor is missing");
                foreach (var row in hud.bitmap_assets)
                {
                    if (row.selector != 58 && row.selector != 66) continue;
                    var indices = cache.ReadOutput(row.indices_path);
                    var mask = cache.ReadOutput(row.mask_path);
                    if (row.stored_width <= 0 || row.stored_height <= 0 ||
                        indices.Length != checked(row.stored_width * row.stored_height) ||
                        mask.Length != indices.Length)
                        throw new InvalidOperationException("Night minimap symbol extent is invalid");
                    minimapSymbols.Add(row.selector, new MinimapSymbolPixels {
                        Width = row.stored_width, Height = row.stored_height,
                        Indices = indices, Mask = mask
                    });
                }
            }
            Buffer.BlockCopy(session.CampaignFrameIndices, 0, presentedIndices, 0, presentedIndices.Length);
            MaterializeMinimapPixels(presentedIndices, minimapEntities, minimapTables, generation, tick, minimapSymbols);
            MaterializeMinimapMarks(presentedIndices, minimapMarks, minimapTables, generation, tick);
            var palette = cache.ReadOutput("palette/paln.rgb8");
            EnsureWorld(presentedIndices, palette);
            minimapOverlay ??= root.gameObject.AddComponent<CampaignMinimapOverlay>();
            minimapOverlay.Present(presentedIndices, palette, minimapTables);
            LiveMinimapReady = minimap.entityCount == (uint)minimapEntities.Length &&
                               minimap.markCount == (uint)minimapMarks.Length;
            MaterializeEntities(session.Entities(), session.EntityPresentations(cameraYaw), cache,
                session.Generation, session.Tick);
            Generation = info.generation;
            Tick = info.tick;
            FrameHash = info.indexHash;
            CacheContentHash = cache.ContentHash;
        }

        internal void Present(NativeCampaignSession session, OwnerAssetCache cache, short cameraYaw = 0)
        {
            if (disposed) throw new ObjectDisposedException(nameof(CampaignPresentationSurface));
            if (session == null) throw new ArgumentNullException(nameof(session));
            if (cache == null || !cache.IsDayAggregate)
                throw new InvalidOperationException("Day presentation requires a qualified Day aggregate");
            var selection = session.Selection;
            if (selection.externalLevel != cache.Level3Materials.profile.external_level ||
                selection.internalLevel != cache.Level3Materials.profile.internal_level ||
                selection.mapType != 0)
                throw new InvalidOperationException("Day presentation/session selection mismatch");

            var generation = session.Generation;
            var tick = session.Tick;
            var frame = session.CopyDayIndexedFrame(tick);
            var terrain = session.TerrainState();
            var entityRows = session.Entities();
            var hud = session.DayHudState(tick);
            var minimap = session.MinimapState(tick);
            var minimapEntities = session.MinimapEntities(tick, minimap);
            var minimapMarks = session.MinimapMarks(tick, minimap);
            var entityPresentations = session.EntityPresentations(cameraYaw);
            if (frame.generation != generation || frame.tick != tick ||
                terrain.generation != generation ||
                terrain.tick != tick)
                throw new InvalidOperationException("Level 3 presentation generation/tick mismatch");

            minimapTables ??= cache.ReadLevel3MaterialOutput("palette/tablesd.u8");
            if (minimapSymbols.Count == 0 && cache.Level3Materials.minimap_symbols != null)
            {
                foreach (var symbol in cache.Level3Materials.minimap_symbols.symbols)
                    minimapSymbols.Add(symbol.selector, new MinimapSymbolPixels {
                        Width = symbol.width, Height = symbol.height,
                        Indices = cache.ReadLevel3MaterialOutput(symbol.indices_path),
                        Mask = cache.ReadLevel3MaterialOutput(symbol.mask_path)
                    });
            }
            Buffer.BlockCopy(session.DayIndexedFrameIndices, 0, presentedIndices, 0, presentedIndices.Length);
            MaterializeMinimapPixels(presentedIndices, minimapEntities, minimapTables, generation, tick, minimapSymbols);
            MaterializeMinimapMarks(presentedIndices, minimapMarks, minimapTables, generation, tick);
            EnsureWorld(presentedIndices, session.DayIndexedFramePalette);
            if (cache.Level3Materials.minimap_symbols != null)
            {
                minimapOverlay ??= root.gameObject.AddComponent<CampaignMinimapOverlay>();
                minimapOverlay.Present(presentedIndices, session.DayIndexedFramePalette, minimapTables);
            }
            MaterializeHud(cache, hud);
            LiveMinimapReady = minimap.entityCount == (uint)minimapEntities.Length &&
                               minimap.markCount == (uint)minimapMarks.Length;
            MaterializeEntities(entityRows, entityPresentations, cache, generation, tick);

            Generation = generation;
            Tick = tick;
            FrameHash = frame.indexHash;
            TerrainHash = terrain.terrainHash;
            CacheContentHash = cache.ContentHash;
        }

        // Rust supplies visibility, source order, positions, colors and command
        // kinds. This is only the indexed-pixel consumer, matching
        // presentation_bridge::push_minimap_entities; no Unity classification.
        internal static int MaterializeMinimapPixels(
            byte[] frame, NativeSim.CampaignMinimapEntity[] commands,
            byte[] tables, ulong generation, ulong tick,
            IReadOnlyDictionary<int, MinimapSymbolPixels> symbols = null)
        {
            if (frame.Length != 320 * 200 || tables.Length != 83456)
                throw new InvalidOperationException("Campaign minimap pixel inputs have invalid extents");
            var written = 0;
            foreach (var command in commands)
            {
                if (command.generation != generation || command.tick != tick)
                    throw new InvalidOperationException("Campaign minimap command is from another revision");
                if (command.visible == 0) continue;
                MinimapSymbolPixels symbol = null;
                if (command.kind == 2 && (symbols == null || !symbols.TryGetValue(command.selector, out symbol))) continue;
                if (command.kind != 1 && command.kind != 2 && command.kind != 3)
                    throw new InvalidOperationException("Unknown campaign minimap pixel command");
                var left = command.x / 2;
                var top = command.y / 2;
                var size = command.kind == 3 ? 1 : (Math.Max(1, (int)command.size) + 1) / 2;
                var width = symbol?.Width ?? size;
                var height = symbol?.Height ?? size;
                if (symbol != null)
                {
                    if (width <= 0 || height <= 0 || symbol.Indices.Length != width * height || symbol.Mask.Length != width * height)
                        throw new InvalidOperationException("Campaign minimap symbol extent mismatch");
                    if (command.anchor == 3) left -= width / 2;
                    if (command.anchor == 2 || command.anchor == 3) top -= height;
                }
                for (var dy = 0; dy < height; dy++)
                for (var dx = 0; dx < width; dx++)
                {
                    var source = dy * width + dx;
                    if (symbol != null && symbol.Mask[source] == 0) continue;
                    var x = left + dx;
                    var y = top + dy;
                    if (x < 0 || x >= 64 || y < 0 || y >= 64) continue;
                    var sourceY = y * 2;
                    var phase = sourceY < 64 ? 63 - sourceY : sourceY - 64;
                    var halfSpan = (64 * tables[0x14300 + phase * 4]) >> 8;
                    if (x * 2 < 64 - halfSpan || x * 2 >= 64 + halfSpan) continue;
                    var offset = y * 320 + x;
                    frame[offset] = command.kind == 3
                        ? tables[0x4000 + (frame[offset] << 8) + command.paletteIndex]
                        : symbol != null ? symbol.Indices[source] : command.paletteIndex;
                    written++;
                }
            }
            return written;
        }

        internal static int MaterializeMinimapMarks(byte[] frame,
            NativeSim.CampaignMinimapMark[] marks, byte[] tables, ulong generation, ulong tick)
        {
            if (frame.Length != 320 * 200 || tables.Length != 83456)
                throw new InvalidOperationException("Campaign minimap mark inputs have invalid extents");
            var written = 0;
            for (var ordinal = 0; ordinal < marks.Length; ordinal++)
            {
                var mark = marks[ordinal];
                if (mark.generation != generation || mark.tick != tick)
                    throw new InvalidOperationException("Campaign minimap mark is from another revision");
                if (mark.kind == 3)
                {
                    if (ordinal + 7 > marks.Length)
                        throw new InvalidOperationException("Campaign minimap arrow is incomplete");
                    for (var vertex = 0; vertex < 7; vertex++)
                    {
                        var row = marks[ordinal + vertex];
                        if (row.kind != 3 || row.sourceOrder != mark.sourceOrder + vertex || row.generation != generation ||
                            row.tick != tick || row.visible != mark.visible || row.paletteIndex != mark.paletteIndex)
                            throw new InvalidOperationException("Campaign minimap arrow vertices disagree");
                    }
                    if (mark.visible != 0)
                    {
                        var pixels = new byte[64 * 64];
                        var mask = new bool[64 * 64];
                        for (var vertex = 0; vertex < 7; vertex++)
                        {
                            var from = marks[ordinal + vertex];
                            var to = marks[ordinal + (vertex + 1) % 7];
                            MagicCarpetRuntime.DrawOwnerMinimapLine(pixels, mask, 64,
                                from.x / 2, from.y / 2, to.x / 2, to.y / 2, mark.paletteIndex);
                        }
                        for (var pixel = 0; pixel < pixels.Length; pixel++)
                        {
                            if (!mask[pixel]) continue;
                            frame[(pixel / 64) * 320 + pixel % 64] = pixels[pixel];
                            written++;
                        }
                    }
                    ordinal += 6;
                    continue;
                }
                if (mark.visible == 0) continue; // Rust owns the original pulse cadence.
                if (mark.kind != 2)
                    throw new InvalidOperationException("Unknown campaign minimap mark command");
                var x = mark.x / 2;
                var y = mark.y / 2;
                // DrawObjectiveRectangle_64CE0: three-pixel interior margin,
                // then a 3x3 outline (the center remains terrain/entity pixels).
                if (y < 3 || y >= 61 || x < 3 || x >= 61) continue;
                var sourceY = y * 2;
                var phase = sourceY < 64 ? 63 - sourceY : sourceY - 64;
                var halfSpan = (32 * tables[0x14300 + phase * 4]) >> 8;
                if (x < 32 - halfSpan + 3 || x >= 32 + halfSpan - 3) continue;
                for (var dy = -1; dy <= 1; dy++)
                for (var dx = -1; dx <= 1; dx++)
                {
                    if (dx == 0 && dy == 0) continue;
                    frame[(y + dy) * 320 + x + dx] = mark.paletteIndex;
                    written++;
                }
            }
            return written;
        }

        private void EnsureWorld(byte[] indices, byte[] palette)
        {
            if (indices == null || indices.Length != NativeSim.DayIndexedFrameIndexBytes ||
                palette == null || palette.Length != NativeSim.DayIndexedFramePaletteBytes)
                throw new InvalidOperationException("Level 3 indexed world buffers are incomplete");
            if (worldObject == null)
            {
                var shader = Resources.Load<Shader>("TerrainIndexedFrame");
                if (shader == null) throw new InvalidOperationException("indexed world shader is missing");
                worldObject = new GameObject("Level 3 live indexed world");
                worldObject.layer = FrameLayer;
                worldObject.transform.SetParent(root, false);
                worldObject.transform.localScale = new Vector3(3.2f, 2f, 1f);
                var filter = worldObject.AddComponent<MeshFilter>();
                worldMesh = CreateQuad();
                // Both native frames and decoded owner TMAPs are top-down.
                // Unity's bottom-up UV convention must not invert either.
                worldMesh.uv = new[]
                {
                    new Vector2(0f, 1f), new Vector2(1f, 1f),
                    new Vector2(0f, 0f), new Vector2(1f, 0f)
                };
                filter.sharedMesh = worldMesh;
                var renderer = worldObject.AddComponent<MeshRenderer>();
                renderer.shadowCastingMode = ShadowCastingMode.Off;
                renderer.receiveShadows = false;
                worldIndices = new Texture2D(
                    NativeSim.DayIndexedFrameWidth,
                    NativeSim.DayIndexedFrameHeight,
                    TextureFormat.R8,
                    false,
                    true)
                {
                    filterMode = FilterMode.Point,
                    wrapMode = TextureWrapMode.Clamp
                };
                worldPalette = new Texture2D(256, 1, TextureFormat.RGB24, false, false)
                {
                    filterMode = FilterMode.Point,
                    wrapMode = TextureWrapMode.Clamp
                };
                worldMaterial = new Material(shader);
                worldMaterial.SetTexture("_Indices", worldIndices);
                worldMaterial.SetTexture("_Coverage", Texture2D.whiteTexture);
                worldMaterial.SetTexture("_Palette", worldPalette);
                renderer.sharedMaterial = worldMaterial;
                // The indexed frame keeps MC2's display aspect. Clear the
                // whole target first so its letterbox cannot retain Title,
                // Loading, or earlier world pixels outside that viewport.
                var backgroundCamera = new GameObject("Campaign background camera").AddComponent<Camera>();
                backgroundCamera.transform.SetParent(root, false);
                backgroundCamera.clearFlags = CameraClearFlags.SolidColor;
                backgroundCamera.backgroundColor = Color.black;
                backgroundCamera.cullingMask = 0;
                backgroundCamera.depth = -101f;
                backgroundCamera.enabled = !Application.isBatchMode;
                frameCamera = new GameObject("Campaign indexed frame camera").AddComponent<Camera>();
                frameCamera.transform.SetParent(root, false);
                frameCamera.transform.localPosition = new Vector3(0f, 0f, -1f);
                frameCamera.orthographic = true;
                frameCamera.orthographicSize = 1f;
                frameCamera.aspect = 1.6f;
                frameCamera.nearClipPlane = 0.01f;
                frameCamera.farClipPlane = 10f;
                frameCamera.clearFlags = CameraClearFlags.SolidColor;
                frameCamera.backgroundColor = Color.black;
                frameCamera.cullingMask = 1 << FrameLayer;
                frameCamera.depth = -100f;
                frameCamera.enabled = !Application.isBatchMode;
                entityCamera = new GameObject("Campaign sprite camera").AddComponent<Camera>();
                entityCamera.transform.SetParent(root, false);
                entityCamera.nearClipPlane = 0.035f;
                entityCamera.farClipPlane = 1800f;
                entityCamera.fieldOfView = Level1ExitPresentationMath.UnityVerticalFov(128);
                entityCamera.clearFlags = CameraClearFlags.Depth;
                entityCamera.cullingMask = 1 << EntityLayer;
                entityCamera.depth = -99f;
                entityCamera.enabled = false;
            }
            worldIndices.LoadRawTextureData(indices);
            worldIndices.Apply(false, false);
            worldPaletteSource = palette;
            worldPalette.LoadRawTextureData(TransformPalette(palette));
            worldPalette.Apply(false, false);
            worldObject.GetComponent<MeshRenderer>().enabled = true;
        }

        internal void PresentView(EngineSessionTransport.Pose pose)
        {
            if (pose == null || entityCamera == null)
                throw new InvalidOperationException("campaign view requires a current Rust player pose");
            var rect = Level1ExitPresentationMath.SourceDisplayRect(Screen.width, Screen.height);
            frameCamera.rect = rect;
            entityCamera.rect = rect;
            // Source yaw zero looks toward decreasing map Y, with increasing
            // map X to the right (classic_hd_terrain::terrain_tile_for).
            // Pitch is a screen offset, not a rotation of the world camera.
            var rotation = Quaternion.Euler(0f, pose.yaw * (360f / 2048f), 0f);
            entityCamera.transform.localPosition = new Vector3(0f, pose.z / 64f, 0f);
            entityCamera.transform.localRotation = rotation;
            entityCamera.projectionMatrix = SourceSpriteProjection(
                pose.display_pitch, pose.roll, entityCamera.nearClipPlane, entityCamera.farClipPlane);
            foreach (var marker in entities.Values)
            {
                // Always project immutable copied coordinates, so repeated
                // presentation cannot accumulate a camera-relative offset.
                var x = ((marker.SourceX - pose.x + 32768) & 65535) - 32768;
                var y = ((marker.SourceY - pose.y + 32768) & 65535) - 32768;
                marker.transform.localPosition = new Vector3(x / 64f, marker.SourceZ / 64f, -y / 64f);
                marker.transform.localRotation = rotation;
            }
            entityCamera.enabled = !Application.isBatchMode;
        }

        internal static Matrix4x4 SourceSpriteProjection(short pitch, short roll, float near, float far)
        {
            // GameRenderHD::DrawTerrainAndParticles_3C080 and Rust
            // ClassicHdProjection: project into 320x200 first; the complete
            // image is then stretched to the common 4:3 display rectangle.
            const float width = 320f, height = 200f;
            const int scale = 7 * 377 * 128 >> 11;
            var offset = (pitch * 320) >> 8;
            var angle = (roll & 2047) * (2f * Mathf.PI / 2048f);
            var sin = Mathf.Sin(angle);
            var cos = Mathf.Cos(angle);
            var matrix = new Matrix4x4();
            matrix.m00 = 2f * scale * cos / width;
            matrix.m01 = -2f * scale * sin / width;
            matrix.m02 = 2f * offset * sin / width;
            matrix.m10 = 2f * scale * sin / height;
            matrix.m11 = 2f * scale * cos / height;
            matrix.m12 = -2f * offset * cos / height;
            matrix.m22 = -(far + near) / (far - near);
            matrix.m23 = -2f * far * near / (far - near);
            matrix.m32 = -1f;
            return matrix;
        }

        private void MaterializeHud(
            OwnerAssetCache cache,
            NativeSim.CampaignDayHudSnapshot hud)
        {
            if (!ReferenceEquals(cache.DayHud, null) && HudBitmapSurfaceCount == 0)
            {
                var descriptor = cache.DayHud;
                var paletteBytes = cache.ReadLevel3GameplayOutput(descriptor.palette.rgb8_path);
                if (paletteBytes.Length != 256 * 3)
                    throw new InvalidOperationException("Level 3 HUD palette is incomplete");
                hudPalette = new Texture2D(256, 1, TextureFormat.RGB24, false, false)
                {
                    filterMode = FilterMode.Point,
                    wrapMode = TextureWrapMode.Clamp
                };
                entityPaletteSource = paletteBytes;
                hudPalette.LoadRawTextureData(TransformPalette(paletteBytes));
                hudPalette.Apply(false, false);
                foreach (var row in descriptor.bitmap_assets)
                {
                    var indices = cache.ReadLevel3GameplayOutput(row.indices_path);
                    var mask = cache.ReadLevel3GameplayOutput(row.mask_path);
                    if (indices.Length != row.stored_width * row.stored_height ||
                        mask.Length != indices.Length)
                        throw new InvalidOperationException($"Level 3 HUD selector {row.selector} is incomplete");
                    var texture = new Texture2D(
                        row.stored_width,
                        row.stored_height,
                        TextureFormat.R8,
                        false,
                        true)
                    {
                        name = $"Level 3 HUD selector {row.selector}",
                        filterMode = FilterMode.Point,
                        wrapMode = TextureWrapMode.Clamp
                    };
                    texture.LoadRawTextureData(indices);
                    texture.Apply(false, true);
                    hudTextures.Add(texture);
                }
                foreach (var tier in descriptor.spell_tier_text.tiers)
                {
                    var rgba = cache.ReadLevel3GameplayOutput(tier.raster_path);
                    if (rgba.Length != tier.stored_width * tier.stored_height * 4)
                        throw new InvalidOperationException($"Level 3 spell tier {tier.tier} is incomplete");
                    var texture = new Texture2D(
                        tier.stored_width,
                        tier.stored_height,
                        TextureFormat.RGBA32,
                        false,
                        true)
                    {
                        name = $"Level 3 spell tier {tier.tier}",
                        filterMode = FilterMode.Point,
                        wrapMode = TextureWrapMode.Clamp
                    };
                    texture.LoadRawTextureData(rgba);
                    texture.Apply(false, true);
                    hudTextures.Add(texture);
                }
                HudBitmapSurfaceCount = descriptor.bitmap_assets.Length;
                SpellTierSurfaceCount = descriptor.spell_tier_text.tiers.Length;
            }
            if (cache.DayHud == null || HudBitmapSurfaceCount != 34 || SpellTierSurfaceCount != 5)
                throw new InvalidOperationException("Level 3 HUD cache surface census is incomplete");
            LiveSpellSurfaceCount =
                (hud.left.rawSpellSelector >= 0 ? 1 : 0) +
                (hud.right.rawSpellSelector >= 0 ? 1 : 0);
            MinimapPanelReady = cache.DayHud.bitmap_selectors.Contains(1);
            HudReady = true;
        }

        private void MaterializeEntities(
            NativeSim.CampaignEntitySnapshot[] rows,
            NativeSim.CampaignEntityPresentation[] presentations,
            OwnerAssetCache cache,
            ulong generation,
            ulong tick)
        {
            if (rows == null || rows.Length != 1000)
                throw new InvalidOperationException("Level 3 entity frame is incomplete");
            var active = new HashSet<ushort>();
            foreach (var row in rows)
            {
                if (row.abiVersion != NativeSim.AbiVersion || row.structSize != 52 ||
                    row.entityIndex >= rows.Length || !active.Add(row.entityIndex))
                    throw new InvalidOperationException("Level 3 entity frame identity is invalid");
                if (row.active == 0) continue;
                if (!entities.TryGetValue(row.entityIndex, out var marker))
                {
                    var node = new GameObject($"Level 3 entity {row.entityIndex:D3}");
                    node.layer = EntityLayer;
                    node.transform.SetParent(root, false);
                    marker = node.AddComponent<Level3EntitySurfaceMarker>();
                    entities.Add(row.entityIndex, marker);
                }
                marker.Bind(row, generation, tick);
            }
            if (materialRows.Count == 0)
            {
                foreach (var texture in cache.Level3Materials.tmaps0.textures)
                    materialRows.Add(texture.index, texture);
            }
            foreach (var presentation in presentations)
            {
                if (!entities.TryGetValue(presentation.entityIndex, out var marker) ||
                    !materialRows.TryGetValue(presentation.selectedTmap, out var texture))
                    throw new InvalidOperationException(
                        $"Level 3 entity {presentation.entityIndex} selector {presentation.selectedTmap} is absent from the qualified cache");
                marker.BindSprite(
                    presentation,
                    texture,
                    worldMesh,
                    EnsureEntityMaterial(cache, texture));
                marker.transform.Find("package sprite").gameObject.layer = EntityLayer;
            }
            var live = new HashSet<ushort>(rows.Where(row => row.active != 0)
                .Select(row => row.entityIndex));
            foreach (var stale in entities.Keys.Where(key => !live.Contains(key)).ToArray())
            {
                DestroyObject(entities[stale].gameObject);
                entities.Remove(stale);
            }
        }

        private Material EnsureEntityMaterial(
            OwnerAssetCache cache,
            OwnerAssetCache.Level3Texture row)
        {
            if (entityMaterials.TryGetValue(row.index, out var material)) return material;
            var bytes = night ? cache.ReadOutput(row.indices_path) : cache.ReadLevel3MaterialOutput(row.indices_path);
            if (bytes.Length != row.width * row.height)
                throw new InvalidOperationException($"Level 3 TMAP {row.index} is incomplete");
            var texture = new Texture2D(row.width, row.height, TextureFormat.R8, false, true)
            {
                filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Clamp
            };
            texture.LoadRawTextureData(bytes);
            texture.Apply(false, true);
            var shader = Resources.Load<Shader>("EntitySprite");
            if (shader == null) throw new InvalidOperationException("entity sprite shader is missing");
            material = new Material(shader);
            material.SetTexture("_Indices", texture);
            material.SetTexture("_Palette", EnsureEntityPalette(cache));
            entityTextures.Add(row.index, texture);
            entityMaterials.Add(row.index, material);
            return material;
        }

        private Texture2D EnsureEntityPalette(OwnerAssetCache cache)
        {
            if (hudPalette != null) return hudPalette;
            var bytes = night ? cache.ReadOutput("palette/paln.rgb8") :
                cache.ReadLevel3MaterialOutput(cache.Level3Materials.palette.rgb8_path);
            if (bytes.Length != 256 * 3)
                throw new InvalidOperationException("Level 3 material palette is incomplete");
            hudPalette = new Texture2D(256, 1, TextureFormat.RGB24, false, false)
            {
                filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Clamp
            };
            entityPaletteSource = bytes;
            hudPalette.LoadRawTextureData(TransformPalette(bytes));
            hudPalette.Apply(false, false);
            return hudPalette;
        }

        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            DestroyObject(worldMaterial);
            DestroyObject(worldIndices);
            DestroyObject(worldPalette);
            DestroyObject(hudPalette);
            foreach (var texture in hudTextures) DestroyObject(texture);
            foreach (var material in entityMaterials.Values) DestroyObject(material);
            foreach (var texture in entityTextures.Values) DestroyObject(texture);
            hudTextures.Clear();
            entityMaterials.Clear();
            entityTextures.Clear();
            materialRows.Clear();
            DestroyObject(worldMesh);
            if (root != null) DestroyObject(root.gameObject);
            entities.Clear();
            HudReady = false;
            MinimapPanelReady = false;
            LiveMinimapReady = false;
        }

        private static Mesh CreateQuad()
        {
            var mesh = new Mesh { name = "Level 3 indexed world quad" };
            mesh.vertices = new[]
            {
                new Vector3(-0.5f, -0.5f, 0f),
                new Vector3(0.5f, -0.5f, 0f),
                new Vector3(-0.5f, 0.5f, 0f),
                new Vector3(0.5f, 0.5f, 0f)
            };
            mesh.uv = new[]
            {
                new Vector2(0f, 0f), new Vector2(1f, 0f),
                new Vector2(0f, 1f), new Vector2(1f, 1f)
            };
            mesh.triangles = new[] { 0, 2, 1, 2, 3, 1 };
            mesh.RecalculateBounds();
            return mesh;
        }

        private static void DestroyObject(UnityEngine.Object value)
        {
            if (value == null) return;
            if (Application.isPlaying) UnityEngine.Object.Destroy(value);
            else UnityEngine.Object.DestroyImmediate(value);
        }
    }
}
