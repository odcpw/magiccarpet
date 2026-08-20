using System;
using System.Collections.Generic;
using System.IO;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native
{
    internal enum Level2WorldObjectKind : byte
    {
        Scenery = 1,
        Building = 2,
        Castle = 3
    }

    internal enum Level2WorldObjectLifecycle : byte
    {
        Active = 1,
        Built = 2,
        StageDestroyed = 3,
        Destroyed = 4
    }

    internal readonly struct Level2WorldObjectDescription
    {
        internal Level2WorldObjectDescription(
            Level2WorldObjectKind kind,
            Level2WorldObjectLifecycle lifecycle,
            ushort sourceSlot,
            ushort sourceId,
            byte classId,
            byte model,
            byte action,
            byte phase,
            ushort x,
            ushort y,
            short z,
            ushort assetSelector)
        {
            Kind = kind;
            Lifecycle = lifecycle;
            SourceSlot = sourceSlot;
            SourceId = sourceId;
            ClassId = classId;
            Model = model;
            Action = action;
            Phase = phase;
            X = x;
            Y = y;
            Z = z;
            AssetSelector = assetSelector;
        }

        internal Level2WorldObjectKind Kind { get; }
        internal Level2WorldObjectLifecycle Lifecycle { get; }
        internal ushort SourceSlot { get; }
        internal ushort SourceId { get; }
        internal byte ClassId { get; }
        internal byte Model { get; }
        internal byte Action { get; }
        internal byte Phase { get; }
        internal ushort X { get; }
        internal ushort Y { get; }
        internal short Z { get; }
        internal ushort AssetSelector { get; }

        internal static Level2WorldObjectDescription FromEntity(
            NativeSim.CampaignEntitySnapshot row,
            Level2WorldObjectKind kind,
            ushort assetSelector)
        {
            return new Level2WorldObjectDescription(
                kind,
                Level2WorldObjectLifecycle.Active,
                row.entityIndex,
                row.sourceId,
                row.classId,
                row.model,
                row.action,
                row.phase,
                row.x,
                row.y,
                row.z,
                assetSelector);
        }
    }

    internal struct Level2CastleLifecycleState
    {
        internal ushort SourceSlot;
        internal ushort OwnerEntityIndex;
        internal byte Action;
        internal byte Stage;
        internal bool Active;
        internal ulong Generation;
        internal ulong Tick;
        internal Level2WorldObjectLifecycle Lifecycle;
    }

    internal static class Level2WorldObjectProjection
    {
        private const byte SceneryClass = 2;
        private const byte BuildingClass = 10;
        private const byte CastleClass = 3;
        private const byte CastleEconomyFamily = 3;
        private const byte CastleBuiltCode = 10;
        private const byte CastleDestroyedCode = 11;
        private const byte CastleConstructionSoundCode = 12;
        private const byte CastleDestructionSoundCode = 13;
        private const byte CastleStageDestroyedCode = 14;
        private const byte CastleDefenderArrivedCode = 7;
        private const byte CastleDefenderCollectedSphereCode = 16;
        private const byte CastleDefenderCaveSoundCode = 17;

        internal static bool TryDescribe(
            NativeSim.CampaignEntitySnapshot row,
            out Level2WorldObjectDescription description)
        {
            description = default;
            if (row.abiVersion != NativeSim.AbiVersion ||
                row.structSize != 52 ||
                row.active == 0 ||
                row.entityIndex == 0 ||
                row.sourceId == 0)
            {
                return false;
            }

            // Class 2/model 1 is the ordinary stone producer. Its source
            // render profile resolves to the owner TMAP 078 entry.
            if (row.classId == SceneryClass &&
                row.model == 1 &&
                row.action == 3)
            {
                description = Level2WorldObjectDescription.FromEntity(
                    row,
                    Level2WorldObjectKind.Scenery,
                    78);
                return true;
            }

            if (row.classId == SceneryClass &&
                row.model == 2 &&
                row.action == 6)
            {
                description = Level2WorldObjectDescription.FromEntity(
                    row,
                    Level2WorldObjectKind.Scenery,
                    79);
                return true;
            }

            if (row.classId == SceneryClass &&
                row.model == 3 &&
                row.action == 9)
            {
                description = Level2WorldObjectDescription.FromEntity(
                    row,
                    Level2WorldObjectKind.Scenery,
                    198);
                return true;
            }

            // Class 11 rows are authored switch/objective-switch callbacks,
            // not drawable scenery. Class 2/model 0 tree rows also have no
            // qualified Level 2 owner-cache TMAP, so both families fail
            // closed instead of creating a guessed placeholder.

            if (row.classId == BuildingClass &&
                row.model == 45 &&
                row.action == 52)
            {
                description = Level2WorldObjectDescription.FromEntity(
                    row,
                    Level2WorldObjectKind.Building,
                    96);
                return true;
            }

            // The live Castle root is a distinct class-3/model-2 row. It is
            // admitted here only when the source lifecycle/event path has
            // already supplied that exact state; reset class-3 players and
            // other class-3 rows belong to their own authority.
            if (row.classId == CastleClass &&
                row.model == 2 &&
                (row.action == 4 || row.action == 5 || row.action == 6))
            {
                description = Level2WorldObjectDescription.FromEntity(
                    row,
                    Level2WorldObjectKind.Castle,
                    96);
                return true;
            }

            return false;
        }

        internal static bool TryProjectRows(
            NativeSim.CampaignEntitySnapshot[] rows,
            out Level2WorldObjectDescription[] descriptions)
        {
            descriptions = Array.Empty<Level2WorldObjectDescription>();
            if (rows == null)
            {
                return false;
            }

            var sourceSlots = new HashSet<ushort>();
            var projected = new List<Level2WorldObjectDescription>();
            foreach (var row in rows)
            {
                if (row.abiVersion != NativeSim.AbiVersion ||
                    row.structSize != 52)
                {
                    return false;
                }

                if (!TryDescribe(row, out var description))
                {
                    continue;
                }

                if (!sourceSlots.Add(description.SourceSlot))
                {
                    // One source slot cannot own two simultaneous projected
                    // views. This is the managed equivalent of the source
                    // arena's unique row identity.
                    return false;
                }
                projected.Add(description);
            }

            descriptions = projected.ToArray();
            return true;
        }

        internal static bool TryApplyCastleEvent(
            NativeSim.CampaignEconomyEvent row,
            ulong generation,
            ulong tick,
            bool dispatchBound,
            ref Level2CastleLifecycleState state)
        {
            if (row.abiVersion != NativeSim.AbiVersion ||
                row.structSize != 64 ||
                row.generation != generation ||
                row.family != CastleEconomyFamily ||
                row.sourceSlot == 0 ||
                row.primaryId != row.sourceSlot ||
                !dispatchBound)
            {
                return false;
            }

            switch (row.code)
            {
                case CastleDefenderArrivedCode:
                case CastleDefenderCollectedSphereCode:
                case CastleDefenderCaveSoundCode:
                case CastleConstructionSoundCode:
                case CastleDestructionSoundCode:
                    // These are typed Castle-family sidecars, but they do
                    // not change the root view's lifecycle.
                    return true;

                case CastleBuiltCode:
                    state = new Level2CastleLifecycleState
                    {
                        SourceSlot = row.sourceSlot,
                        OwnerEntityIndex = row.entity0,
                        Action = 5,
                        Stage = 0,
                        Active = true,
                        Generation = generation,
                        Tick = tick,
                        Lifecycle = Level2WorldObjectLifecycle.Built
                    };
                    return true;

                case CastleStageDestroyedCode:
                    if (state.SourceSlot != row.sourceSlot ||
                        !state.Active ||
                        row.value0 < byte.MinValue ||
                        row.value0 > byte.MaxValue ||
                        row.value1 < byte.MinValue ||
                        row.value1 > byte.MaxValue)
                    {
                        return false;
                    }
                    state.Action = 6;
                    state.Stage = (byte)row.value1;
                    state.Active = state.Stage != 0;
                    state.Generation = generation;
                    state.Tick = tick;
                    state.Lifecycle = Level2WorldObjectLifecycle.StageDestroyed;
                    return true;

                case CastleDestroyedCode:
                    if (state.SourceSlot != row.sourceSlot ||
                        state.Lifecycle == Level2WorldObjectLifecycle.Destroyed ||
                        (!state.Active &&
                         state.Lifecycle != Level2WorldObjectLifecycle.StageDestroyed))
                    {
                        return false;
                    }
                    state.Action = 6;
                    state.Active = false;
                    state.Generation = generation;
                    state.Tick = tick;
                    state.Lifecycle = Level2WorldObjectLifecycle.Destroyed;
                    return true;

                default:
                    // Do not silently turn a future/unknown Castle payload
                    // into a placeholder view.
                    return false;
            }
        }
    }

    internal sealed class Level2WorldObjectMarker : MonoBehaviour
    {
        internal Level2WorldObjectKind Kind { get; private set; }
        internal Level2WorldObjectLifecycle Lifecycle { get; private set; }
        internal ushort SourceSlot { get; private set; }
        internal ushort SourceId { get; private set; }
        internal byte ClassId { get; private set; }
        internal byte Model { get; private set; }
        internal byte Action { get; private set; }
        internal byte Phase { get; private set; }
        internal ushort AssetSelector { get; private set; }
        internal ulong Generation { get; private set; }
        internal ulong Tick { get; private set; }
        internal ushort SourceX { get; private set; }
        internal ushort SourceY { get; private set; }
        internal short SourceZ { get; private set; }
        internal string AssetPath { get; private set; }
        internal bool Live { get; private set; }

        internal void Bind(
            Level2WorldObjectDescription description,
            ulong generation,
            ulong tick,
            string assetPath)
        {
            Kind = description.Kind;
            Lifecycle = description.Lifecycle;
            SourceSlot = description.SourceSlot;
            SourceId = description.SourceId;
            ClassId = description.ClassId;
            Model = description.Model;
            Action = description.Action;
            Phase = description.Phase;
            AssetSelector = description.AssetSelector;
            Generation = generation;
            Tick = tick;
            SourceX = description.X;
            SourceY = description.Y;
            SourceZ = description.Z;
            AssetPath = assetPath;
            Live = true;
        }

        internal void Hide()
        {
            Live = false;
        }
    }

    public sealed partial class MagicCarpetRuntime
    {
        private sealed class Level2IndexedWorldPresenter
        {
            private const string MaterialDescriptorPath =
                "components/material/materials/descriptor.json";
            private const string MaterialSchema =
                "mc2-level2-day-materials/v1";

            private readonly MagicCarpetRuntime owner;
            private readonly Dictionary<ushort, Level2WorldObjectView> worldObjectViews =
                new();
            private readonly Dictionary<ushort, Level2WorldObjectAsset> worldObjectAssets =
                new();
            private readonly Dictionary<ushort, Level2CastleLifecycleState> castleStates =
                new();
            private Transform worldObjectsRoot;
            private Mesh worldObjectMesh;
            private Shader worldObjectShader;
            private Level2MaterialDescriptor materialDescriptor;
            private ulong observedGeneration = ulong.MaxValue;
            private ulong observedTick;

            [Serializable]
            private sealed class Level2MaterialDescriptor
            {
                public string schema;
                public Level2MaterialTmaps0 tmaps0;
            }

            [Serializable]
            private sealed class Level2MaterialTmaps0
            {
                public int[] selectors;
                public Level2MaterialTexture[] textures;
            }

            [Serializable]
            private sealed class Level2MaterialTexture
            {
                public int index;
                public int width;
                public int height;
                public int frame_count;
                public string indices_path;
            }

            private sealed class Level2WorldObjectAsset
            {
                internal ushort Selector;
                internal int Width;
                internal int Height;
                internal int FrameCount;
                internal string RelativePath;
                internal Texture2D Indices;
                internal Material Material;
            }

            private sealed class Level2WorldObjectView
            {
                internal GameObject Object;
                internal MeshRenderer Renderer;
                internal Level2WorldObjectMarker Marker;
                internal ushort AssetSelector;
                internal int MissingProjections;
            }

            internal Level2IndexedWorldPresenter(MagicCarpetRuntime owner)
            {
                this.owner = owner;
            }

            internal void Ensure()
            {
                MagicCarpetRuntime.Require(
                    owner.level2Mode && owner.campaignSession != null,
                    "Level 2 indexed frame presentation has a campaign owner");
                if (owner.level2IndexedFrameRenderer != null)
                {
                    EnsureWorldObjectPresentation();
                    return;
                }

                MagicCarpetRuntime.Require(
                    SystemInfo.SupportsTextureFormat(TextureFormat.R8),
                    "Level 2 indexed frame R8 texture support");
                MagicCarpetRuntime.Require(
                    SystemInfo.SupportsTextureFormat(TextureFormat.RGB24),
                    "Level 2 indexed frame RGB8 palette texture support");
                var shader = Resources.Load<Shader>("TerrainIndexedFrame");
                MagicCarpetRuntime.Require(
                    shader != null,
                    "Level 2 indexed frame palette shader");

                owner.level2IndexedFrameObject = new GameObject(
                    "Level 2 Day Rust indexed frame presentation");
                owner.level2IndexedFrameObject.transform.SetParent(
                    owner.transform,
                    false);
                owner.level2IndexedFrameObject.transform.localPosition =
                    Vector3.zero;
                owner.level2IndexedFrameObject.transform.localScale =
                    new Vector3(3.2f, 2f, 1f);
                var filter = owner.level2IndexedFrameObject.AddComponent<MeshFilter>();
                owner.level2IndexedFrameMesh = CreateQuad();
                filter.sharedMesh = owner.level2IndexedFrameMesh;
                owner.level2IndexedFrameRenderer =
                    owner.level2IndexedFrameObject.AddComponent<MeshRenderer>();
                owner.level2IndexedFrameRenderer.shadowCastingMode =
                    UnityEngine.Rendering.ShadowCastingMode.Off;
                owner.level2IndexedFrameRenderer.receiveShadows = false;

                owner.level2IndexedFrameIndexTexture = new Texture2D(
                    NativeSim.DayIndexedFrameWidth,
                    NativeSim.DayIndexedFrameHeight,
                    TextureFormat.R8,
                    false,
                    true)
                {
                    name = "Level 2 Day Rust indexed frame indices",
                    filterMode = FilterMode.Point,
                    wrapMode = TextureWrapMode.Clamp
                };
                owner.level2IndexedFramePaletteTexture = new Texture2D(
                    256,
                    1,
                    TextureFormat.RGB24,
                    false,
                    false)
                {
                    name = "Level 2 Day Rust indexed frame RGB8 palette",
                    filterMode = FilterMode.Point,
                    wrapMode = TextureWrapMode.Clamp
                };
                owner.level2IndexedFrameMaterial = new Material(shader)
                {
                    name = "Level 2 Day Rust indexed frame palette material"
                };
                owner.level2IndexedFrameMaterial.SetTexture(
                    "_Indices",
                    owner.level2IndexedFrameIndexTexture);
                owner.level2IndexedFrameMaterial.SetTexture(
                    "_Coverage",
                    Texture2D.whiteTexture);
                owner.level2IndexedFrameMaterial.SetTexture(
                    "_Palette",
                    owner.level2IndexedFramePaletteTexture);
                owner.level2IndexedFrameRenderer.sharedMaterial =
                    owner.level2IndexedFrameMaterial;
                owner.level2IndexedFrameRenderer.enabled = false;

                owner.level2IndexedFrameCamera = new GameObject(
                        "Level 2 Day indexed frame presentation camera")
                    .AddComponent<Camera>();
                owner.level2IndexedFrameCamera.transform.SetParent(
                    owner.transform,
                    false);
                owner.level2IndexedFrameCamera.transform.localPosition =
                    new Vector3(0f, 0f, -1f);
                owner.level2IndexedFrameCamera.transform.localRotation =
                    Quaternion.identity;
                owner.level2IndexedFrameCamera.clearFlags =
                    CameraClearFlags.SolidColor;
                owner.level2IndexedFrameCamera.backgroundColor = Color.black;
                owner.level2IndexedFrameCamera.orthographic = true;
                owner.level2IndexedFrameCamera.orthographicSize = 1f;
                owner.level2IndexedFrameCamera.aspect = 1.6f;
                owner.level2IndexedFrameCamera.nearClipPlane = 0.01f;
                owner.level2IndexedFrameCamera.farClipPlane = 10f;
                owner.level2IndexedFrameCamera.cullingMask =
                    1 << owner.level2IndexedFrameObject.layer;
                owner.level2IndexedFrameCamera.depth = -100f;
                EnsureWorldObjectPresentation();
            }

            internal void Update(ulong expectedTick)
            {
                owner.RecordLevel2Presentation("indexed-world");
                Ensure();
                owner.level2IndexedFrameRenderer.enabled = false;
                owner.level2IndexedFrameLive = false;
                owner.level2IndexedFrameInfo = default;

                // The copy is generation/tick-bound in the native carrier. Do
                // not touch either Unity texture until the complete copy
                // succeeds; stale requests therefore cannot re-present prior
                // bytes.
                var info = owner.campaignSession.CopyDayIndexedFrame(expectedTick);
                var indices = owner.campaignSession.DayIndexedFrameIndices;
                var palette = owner.campaignSession.DayIndexedFramePalette;
                MagicCarpetRuntime.Require(
                    info.generation == owner.campaignSession.Generation &&
                    info.tick == expectedTick &&
                    info.indexBytes == (uint)indices.Length &&
                    info.paletteBytes == (uint)palette.Length,
                    "Level 2 indexed frame generation/tick and byte binding");

                owner.level2IndexedFrameIndexTexture.LoadRawTextureData(indices);
                owner.level2IndexedFrameIndexTexture.Apply(false, false);
                owner.level2IndexedFramePaletteTexture.LoadRawTextureData(palette);
                owner.level2IndexedFramePaletteTexture.Apply(false, false);
                UpdateWorldObjects(info.generation, info.tick);
                owner.level2IndexedFrameInfo = info;
                owner.level2IndexedFrameRenderer.enabled = true;
                owner.level2IndexedFrameLive = true;
                owner.level2IndexedFrameUploadCount++;
            }

            internal void Dispose()
            {
                if (owner.level2IndexedFrameRenderer != null)
                {
                    owner.level2IndexedFrameRenderer.enabled = false;
                }

                DestroyWorldObjectViews();
                DestroyObject(owner.level2IndexedFrameObject);
                DestroyObject(owner.level2IndexedFrameCamera?.gameObject);
                DestroyObject(owner.level2IndexedFrameMaterial);
                DestroyObject(owner.level2IndexedFrameIndexTexture);
                DestroyObject(owner.level2IndexedFramePaletteTexture);
                DestroyObject(owner.level2IndexedFrameMesh);
                foreach (var asset in worldObjectAssets.Values)
                {
                    DestroyObject(asset.Material);
                    DestroyObject(asset.Indices);
                }
                worldObjectAssets.Clear();
                castleStates.Clear();
                materialDescriptor = null;
                worldObjectsRoot = null;
                DestroyObject(worldObjectMesh);
                worldObjectMesh = null;
                worldObjectShader = null;
                observedGeneration = ulong.MaxValue;
                observedTick = 0;

                owner.level2IndexedFrameObject = null;
                owner.level2IndexedFrameCamera = null;
                owner.level2IndexedFrameMesh = null;
                owner.level2IndexedFrameRenderer = null;
                owner.level2IndexedFrameIndexTexture = null;
                owner.level2IndexedFramePaletteTexture = null;
                owner.level2IndexedFrameMaterial = null;
                owner.level2IndexedFrameInfo = default;
                owner.level2IndexedFrameLive = false;
            }

            private void EnsureWorldObjectPresentation()
            {
                if (worldObjectsRoot != null)
                {
                    return;
                }

                worldObjectShader = Resources.Load<Shader>("EntitySprite");
                MagicCarpetRuntime.Require(
                    worldObjectShader != null,
                    "Level 2 world-object owner sprite shader");
                worldObjectMesh = CreateSpriteQuad();
                var root = new GameObject(
                    "Level 2 Day scenery and building projections");
                root.layer = owner.level2IndexedFrameObject.layer;
                root.transform.SetParent(
                    owner.level2IndexedFrameObject.transform,
                    false);
                root.transform.localPosition = Vector3.zero;
                root.transform.localRotation = Quaternion.identity;
                root.transform.localScale = Vector3.one;
                worldObjectsRoot = root.transform;
            }

            private void UpdateWorldObjects(ulong generation, ulong tick)
            {
                if (observedGeneration != ulong.MaxValue &&
                    (generation < observedGeneration ||
                     generation == observedGeneration && tick < observedTick))
                {
                    AdvanceMissingWorldObjects();
                    return;
                }
                if (generation > observedGeneration)
                {
                    castleStates.Clear();
                }
                observedGeneration = generation;
                observedTick = tick;
                AdvanceMissingWorldObjects();

                var rows = owner.campaignSession.Entities();
                if (!Level2WorldObjectProjection.TryProjectRows(
                        rows,
                        out var descriptions))
                {
                    ReclaimMissingWorldObjects();
                    return;
                }

                var economyEvents = owner.campaignSession.EconomyEvents();
                if (!ApplyCastleEvents(economyEvents, generation, tick))
                {
                    ReclaimMissingWorldObjects();
                    return;
                }

                var desired = new Dictionary<ushort, Level2WorldObjectDescription>();
                foreach (var description in descriptions)
                {
                    if (!desired.TryAdd(description.SourceSlot, description))
                    {
                        ReclaimMissingWorldObjects();
                        return;
                    }
                }

                foreach (var state in castleStates.Values)
                {
                    if (!state.Active ||
                        !TryDescribeCastleState(state, rows, out var description))
                    {
                        continue;
                    }
                    if (!desired.TryAdd(description.SourceSlot, description))
                    {
                        ReclaimMissingWorldObjects();
                        return;
                    }
                }

                foreach (var description in desired.Values)
                {
                    PresentWorldObject(description, generation, tick);
                }
                ReclaimMissingWorldObjects();
            }

            private bool ApplyCastleEvents(
                NativeSim.CampaignEconomyEvent[] economyEvents,
                ulong generation,
                ulong tick)
            {
                if (economyEvents == null)
                {
                    return false;
                }

                var seenOrdinals = new HashSet<uint>();
                foreach (var row in economyEvents)
                {
                    if (row.abiVersion != NativeSim.AbiVersion ||
                        row.structSize != 64 ||
                        row.generation != generation ||
                        !seenOrdinals.Add(row.ordinal))
                    {
                        return false;
                    }
                    if (row.family != 3)
                    {
                        continue;
                    }

                    if (!HasBoundCastleDispatch(row, generation))
                    {
                        return false;
                    }

                    var state = castleStates.TryGetValue(
                        row.sourceSlot,
                        out var prior)
                        ? prior
                        : default;
                    if (!Level2WorldObjectProjection.TryApplyCastleEvent(
                            row,
                            generation,
                            tick,
                            true,
                            ref state))
                    {
                        return false;
                    }
                    if (state.SourceSlot != 0)
                    {
                        castleStates[row.sourceSlot] = state;
                    }
                }
                return true;
            }

            private bool HasBoundCastleDispatch(
                NativeSim.CampaignEconomyEvent row,
                ulong generation)
            {
                var dispatch = owner.level2StepEvents;
                if (dispatch == null)
                {
                    return false;
                }
                foreach (var eventRow in dispatch)
                {
                    if (eventRow.abiVersion == NativeSim.AbiVersion &&
                        eventRow.structSize == 48 &&
                        eventRow.generation == generation &&
                        eventRow.ordinal == row.ordinal &&
                        eventRow.kind == NativeSim.CampaignEventEconomy &&
                        eventRow.family == 3 &&
                        eventRow.sourceSlot == row.sourceSlot)
                    {
                        return true;
                    }
                }
                return false;
            }

            private bool TryDescribeCastleState(
                Level2CastleLifecycleState state,
                NativeSim.CampaignEntitySnapshot[] rows,
                out Level2WorldObjectDescription description)
            {
                description = default;
                foreach (var row in rows)
                {
                    if (row.entityIndex == state.SourceSlot &&
                        row.active != 0 &&
                        row.abiVersion == NativeSim.AbiVersion &&
                        row.structSize == 52 &&
                        row.classId == 3 &&
                        (row.model == 0 || row.model == 1) &&
                        row.sourceId != 0)
                    {
                        description = new Level2WorldObjectDescription(
                            Level2WorldObjectKind.Castle,
                            state.Lifecycle,
                            state.SourceSlot,
                            state.OwnerEntityIndex,
                            3,
                            2,
                            state.Action,
                            row.phase,
                            row.x,
                            row.y,
                            row.z,
                            96);
                        return true;
                    }
                }

                // A previously admitted Castle keeps its last authoritative
                // transform until the source row's following projection
                // reclaims it. Never invent a position for a new root.
                if (worldObjectViews.TryGetValue(
                        state.SourceSlot,
                        out var view) &&
                    view.Marker != null &&
                    view.Marker.Kind == Level2WorldObjectKind.Castle)
                {
                    description = new Level2WorldObjectDescription(
                        Level2WorldObjectKind.Castle,
                        state.Lifecycle,
                        state.SourceSlot,
                        state.OwnerEntityIndex,
                        3,
                        2,
                        state.Action,
                        view.Marker.Phase,
                        view.Marker.SourceX,
                        view.Marker.SourceY,
                        view.Marker.SourceZ,
                        96);
                    return true;
                }
                return false;
            }

            private void PresentWorldObject(
                Level2WorldObjectDescription description,
                ulong generation,
                ulong tick)
            {
                var asset = GetWorldObjectAsset(description.AssetSelector);
                if (!worldObjectViews.TryGetValue(
                        description.SourceSlot,
                        out var view) ||
                    view.AssetSelector != description.AssetSelector ||
                    view.Marker.Kind != description.Kind)
                {
                    if (view != null)
                    {
                        DestroyWorldObjectView(description.SourceSlot, view);
                    }
                    view = CreateWorldObjectView(description.SourceSlot, asset);
                    worldObjectViews[description.SourceSlot] = view;
                }

                view.MissingProjections = 0;
                view.Marker.Bind(
                    description,
                    generation,
                    tick,
                    asset.RelativePath);
                view.Object.transform.localPosition = ProjectPosition(
                    description.X,
                    description.Y,
                    description.SourceSlot);
                view.Object.transform.localScale = new Vector3(
                    asset.Width / 320f,
                    asset.Height / 200f,
                    1f);
                view.Renderer.sharedMaterial = asset.Material;
                view.Renderer.enabled = true;
            }

            private Level2WorldObjectView CreateWorldObjectView(
                ushort sourceSlot,
                Level2WorldObjectAsset asset)
            {
                var viewObject = new GameObject(
                    $"Level 2 world object slot {sourceSlot:D3}");
                viewObject.layer = owner.level2IndexedFrameObject.layer;
                viewObject.transform.SetParent(worldObjectsRoot, false);
                var filter = viewObject.AddComponent<MeshFilter>();
                filter.sharedMesh = worldObjectMesh;
                var renderer = viewObject.AddComponent<MeshRenderer>();
                renderer.shadowCastingMode = ShadowCastingMode.Off;
                renderer.receiveShadows = false;
                renderer.sharedMaterial = asset.Material;
                renderer.enabled = false;
                var marker = viewObject.AddComponent<Level2WorldObjectMarker>();
                return new Level2WorldObjectView
                {
                    Object = viewObject,
                    Renderer = renderer,
                    Marker = marker,
                    AssetSelector = asset.Selector,
                    MissingProjections = 0
                };
            }

            private Level2WorldObjectAsset GetWorldObjectAsset(ushort selector)
            {
                if (worldObjectAssets.TryGetValue(selector, out var existing))
                {
                    return existing;
                }
                MagicCarpetRuntime.Require(
                    owner.ownerAssetCache != null &&
                    owner.level2IndexedFramePaletteTexture != null,
                    "Level 2 world-object owner cache and palette");
                LoadMaterialDescriptor();
                var textureDescriptor = FindMaterialTexture(selector);
                if (textureDescriptor == null)
                {
                    throw new InvalidDataException(
                        $"Level 2 world-object TMAP selector {selector} is malformed");
                }
                if (textureDescriptor.width <= 0 ||
                    textureDescriptor.height <= 0 ||
                    textureDescriptor.frame_count <= 0 ||
                    string.IsNullOrWhiteSpace(textureDescriptor.indices_path) ||
                    !textureDescriptor.indices_path.StartsWith(
                        "terrain/tmaps0/",
                        StringComparison.Ordinal) ||
                    textureDescriptor.indices_path.Contains("..", StringComparison.Ordinal))
                {
                    throw new InvalidDataException(
                        $"Level 2 world-object TMAP selector {selector} is malformed");
                }

                var relativePath =
                    $"components/material/{textureDescriptor.indices_path}";
                var bytes = owner.ownerAssetCache.ReadOutput(relativePath);
                var expectedBytes = checked(
                    textureDescriptor.width * textureDescriptor.height);
                MagicCarpetRuntime.Require(
                    bytes.Length == expectedBytes,
                    $"Level 2 world-object TMAP {selector} byte length");
                var indices = new Texture2D(
                    textureDescriptor.width,
                    textureDescriptor.height,
                    TextureFormat.R8,
                    false,
                    true)
                {
                    name = $"Level 2 Day TMAP {selector:D3} indices",
                    filterMode = FilterMode.Point,
                    wrapMode = TextureWrapMode.Clamp
                };
                indices.LoadRawTextureData(bytes);
                indices.Apply(false, false);
                var material = new Material(worldObjectShader)
                {
                    name = $"Level 2 Day TMAP {selector:D3} owner sprite material"
                };
                material.SetTexture("_Indices", indices);
                material.SetTexture(
                    "_Palette",
                    owner.level2IndexedFramePaletteTexture);
                var asset = new Level2WorldObjectAsset
                {
                    Selector = selector,
                    Width = textureDescriptor.width,
                    Height = textureDescriptor.height,
                    FrameCount = textureDescriptor.frame_count,
                    RelativePath = relativePath,
                    Indices = indices,
                    Material = material
                };
                worldObjectAssets.Add(selector, asset);
                return asset;
            }

            private void LoadMaterialDescriptor()
            {
                if (materialDescriptor != null)
                {
                    return;
                }
                var text = owner.ownerAssetCache.ReadOutputText(MaterialDescriptorPath);
                materialDescriptor = JsonUtility.FromJson<Level2MaterialDescriptor>(text);
                MagicCarpetRuntime.Require(
                    materialDescriptor != null &&
                    materialDescriptor.schema == MaterialSchema &&
                    materialDescriptor.tmaps0 != null &&
                    materialDescriptor.tmaps0.selectors != null &&
                    materialDescriptor.tmaps0.textures != null,
                    "Level 2 material descriptor for world objects");
            }

            private Level2MaterialTexture FindMaterialTexture(ushort selector)
            {
                foreach (var candidate in materialDescriptor.tmaps0.textures)
                {
                    if (candidate != null && candidate.index == selector)
                    {
                        return candidate;
                    }
                }
                return null;
            }

            private void AdvanceMissingWorldObjects()
            {
                foreach (var view in worldObjectViews.Values)
                {
                    view.MissingProjections++;
                    view.Renderer.enabled = false;
                    view.Marker.Hide();
                }
            }

            private void ReclaimMissingWorldObjects()
            {
                var reclaimed = new List<ushort>();
                foreach (var pair in worldObjectViews)
                {
                    if (pair.Value.MissingProjections >= 2)
                    {
                        reclaimed.Add(pair.Key);
                    }
                }
                foreach (var sourceSlot in reclaimed)
                {
                    var view = worldObjectViews[sourceSlot];
                    DestroyWorldObjectView(sourceSlot, view);
                }
            }

            private void DestroyWorldObjectViews()
            {
                var views = new List<Level2WorldObjectView>(
                    worldObjectViews.Values);
                worldObjectViews.Clear();
                foreach (var view in views)
                {
                    DestroyObject(view.Object);
                }
            }

            private void DestroyWorldObjectView(
                ushort sourceSlot,
                Level2WorldObjectView view)
            {
                worldObjectViews.Remove(sourceSlot);
                DestroyObject(view.Object);
            }

            private static Vector3 ProjectPosition(
                ushort x,
                ushort y,
                ushort sourceSlot)
            {
                return new Vector3(
                    x / 65535f - 0.5f,
                    0.5f - y / 65535f,
                    -0.01f - sourceSlot / 1000000f);
            }

            private static void DestroyObject(UnityEngine.Object value)
            {
                if (value == null) return;
                if (Application.isPlaying)
                    UnityEngine.Object.Destroy(value);
                else
                    UnityEngine.Object.DestroyImmediate(value);
            }

            private static Mesh CreateQuad()
            {
                var mesh = new Mesh
                {
                    name = "Level 2 Day Rust indexed frame quad"
                };
                mesh.vertices = new[]
                {
                    new Vector3(-0.5f, -0.5f, 0f),
                    new Vector3(0.5f, -0.5f, 0f),
                    new Vector3(-0.5f, 0.5f, 0f),
                    new Vector3(0.5f, 0.5f, 0f)
                };
                // The native frame is top-to-bottom, while Unity samples raw
                // texture row zero at v=0. Keep the native bytes intact and
                // map the top vertices to v=0 so source order is preserved.
                mesh.uv = new[]
                {
                    new Vector2(0f, 1f),
                    new Vector2(1f, 1f),
                    new Vector2(0f, 0f),
                    new Vector2(1f, 0f)
                };
                mesh.triangles = new[] { 0, 2, 1, 2, 3, 1 };
                mesh.RecalculateBounds();
                return mesh;
            }

            private static Mesh CreateSpriteQuad()
            {
                var mesh = new Mesh
                {
                    name = "Level 2 Day world-object sprite quad"
                };
                mesh.vertices = new[]
                {
                    new Vector3(-0.5f, -0.5f, 0f),
                    new Vector3(0.5f, -0.5f, 0f),
                    new Vector3(-0.5f, 0.5f, 0f),
                    new Vector3(0.5f, 0.5f, 0f)
                };
                mesh.uv = new[]
                {
                    new Vector2(0f, 1f),
                    new Vector2(1f, 1f),
                    new Vector2(0f, 0f),
                    new Vector2(1f, 0f)
                };
                mesh.triangles = new[] { 0, 2, 1, 2, 3, 1 };
                mesh.RecalculateBounds();
                return mesh;
            }
        }

        private void EnsureLevel2IndexedFramePresentation()
        {
            Level2IndexedWorld.Ensure();
        }

        private void UpdateLevel2IndexedFrame(ulong expectedTick)
        {
            Level2IndexedWorld.Update(expectedTick);
        }

        private void DestroyLevel2IndexedFramePresentation()
        {
            level2IndexedWorldPresenter?.Dispose();
        }
    }
}
