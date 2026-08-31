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

        internal void Bind(
            NativeSim.CampaignEntitySnapshot row,
            ulong generation,
            ulong tick)
        {
            EntityIndex = row.entityIndex;
            SourceId = row.sourceId;
            ClassId = row.classId;
            Model = row.model;
            Action = row.action;
            Generation = generation;
            Tick = tick;
            transform.localPosition = new Vector3(
                row.x / 64f,
                row.z / 64f,
                row.y / 64f);
        }
    }

    /// <summary>
    /// A package-neutral Level 3 Unity consumer. Rust supplies the live frame,
    /// entity and spell-inventory state; the sealed owner cache supplies only
    /// validated HUD/spell/minimap pixels. No Level 2 entity selector table is copied here.
    /// Entity nodes therefore remain typed position/lifecycle bindings until
    /// Rust exports their semantic presentation asset selector.
    /// </summary>
    internal sealed class Level3PresentationSurface : IDisposable
    {
        private readonly Transform root;
        private readonly Dictionary<ushort, Level3EntitySurfaceMarker> entities = new();
        private readonly List<Texture2D> hudTextures = new();
        private GameObject worldObject;
        private Mesh worldMesh;
        private Material worldMaterial;
        private Texture2D worldIndices;
        private Texture2D worldPalette;
        private Texture2D hudPalette;
        private bool disposed;

        internal Level3PresentationSurface(Transform parent = null)
        {
            root = new GameObject("Level 3 Rust presentation surface").transform;
            if (parent != null) root.SetParent(parent, false);
        }

        internal Transform Root => root;
        internal ulong Generation { get; private set; }
        internal ulong Tick { get; private set; }
        internal ulong FrameHash { get; private set; }
        internal ulong TerrainHash { get; private set; }
        internal bool WorldReady => worldObject != null &&
                                    worldObject.GetComponent<MeshRenderer>()?.enabled == true;
        internal bool HudReady { get; private set; }
        internal bool MinimapPanelReady { get; private set; }
        internal bool LiveMinimapReady => false;
        internal int EntitySurfaceCount => entities.Count;
        internal int HudBitmapSurfaceCount { get; private set; }
        internal int SpellTierSurfaceCount { get; private set; }
        internal int LiveSpellSurfaceCount { get; private set; }
        internal string CacheContentHash { get; private set; }

        internal void Present(NativeCampaignSession session, OwnerAssetCache cache)
        {
            if (disposed) throw new ObjectDisposedException(nameof(Level3PresentationSurface));
            if (session == null) throw new ArgumentNullException(nameof(session));
            if (cache == null || !cache.IsLevel3Aggregate)
                throw new InvalidOperationException("Level 3 presentation requires the qualified Level 3 aggregate");
            var selection = session.Selection;
            if (selection.externalLevel != 3 ||
                selection.internalLevel != 2 ||
                selection.mapType != 0)
                throw new InvalidOperationException("Level 3 presentation/session selection mismatch");

            var generation = session.Generation;
            var tick = session.Tick;
            var frame = session.CopyDayIndexedFrame(tick);
            var terrain = session.TerrainState();
            var entityRows = session.Entities();
            var localPlayer = session.Players()
                .Single(player => player.slot == 0 && player.active != 0);
            if (frame.generation != generation || frame.tick != tick ||
                terrain.generation != generation ||
                terrain.tick != tick)
                throw new InvalidOperationException("Level 3 presentation generation/tick mismatch");

            EnsureWorld(session.DayIndexedFrameIndices, session.DayIndexedFramePalette);
            MaterializeHud(cache, localPlayer);
            MaterializeEntities(entityRows, generation, tick);

            Generation = generation;
            Tick = tick;
            FrameHash = frame.indexHash;
            TerrainHash = terrain.terrainHash;
            CacheContentHash = cache.ContentHash;
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
                worldObject.transform.SetParent(root, false);
                var filter = worldObject.AddComponent<MeshFilter>();
                worldMesh = CreateQuad();
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
            }
            worldIndices.LoadRawTextureData(indices);
            worldIndices.Apply(false, false);
            worldPalette.LoadRawTextureData(palette);
            worldPalette.Apply(false, false);
            worldObject.GetComponent<MeshRenderer>().enabled = true;
        }

        private void MaterializeHud(
            OwnerAssetCache cache,
            NativeSim.CampaignPlayerSnapshot localPlayer)
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
                hudPalette.LoadRawTextureData(paletteBytes);
                hudPalette.Apply(false, true);
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
                (localPlayer.leftSpellSlotRaw >= 0 ? 1 : 0) +
                (localPlayer.rightSpellSlotRaw >= 0 ? 1 : 0);
            MinimapPanelReady = cache.DayHud.bitmap_selectors.Contains(1);
            HudReady = true;
        }

        private void MaterializeEntities(
            NativeSim.CampaignEntitySnapshot[] rows,
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
                    node.transform.SetParent(root, false);
                    marker = node.AddComponent<Level3EntitySurfaceMarker>();
                    entities.Add(row.entityIndex, marker);
                }
                marker.Bind(row, generation, tick);
            }
            var live = new HashSet<ushort>(rows.Where(row => row.active != 0)
                .Select(row => row.entityIndex));
            foreach (var stale in entities.Keys.Where(key => !live.Contains(key)).ToArray())
            {
                DestroyObject(entities[stale].gameObject);
                entities.Remove(stale);
            }
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
            hudTextures.Clear();
            DestroyObject(worldMesh);
            if (root != null) DestroyObject(root.gameObject);
            entities.Clear();
            HudReady = false;
            MinimapPanelReady = false;
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
