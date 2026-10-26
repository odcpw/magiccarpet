using System;
using System.Collections.Generic;
using UnityEngine;

namespace MagicCarpet.Native
{
    public sealed partial class MagicCarpetRuntime
    {
        private sealed class CampaignEntityPresenter
        {
            private readonly MagicCarpetRuntime owner;

            internal CampaignEntityPresenter(MagicCarpetRuntime owner)
            {
                this.owner = owner;
            }

            internal void Update()
            {
                if (owner.level2Mode)
                {
                    owner.RecordLevel2Presentation("entity");
                    owner.PresentLevel2ProductionEntityRows();
                    return;
                }

                owner.RecordLevel2Presentation("entity");
                var suppressionFlags = owner.CurrentNativeFinalPresentationOwns()
                    ? owner.terrainFrameInfo.flags
                    : 0u;
                if (owner.CurrentNativeFinalPresentationOwns())
                    owner.managedEntitySubmissionCount++;
                if (owner.entityRegistry.Count == 0) return;
                foreach (var marker in owner.entityRegistry)
                {
                    var entity = owner.entities[marker.Index];
                    var goatOrdinal =
                        owner.goatOrdinalByEntity != null &&
                        marker.Index < owner.goatOrdinalByEntity.Length
                            ? owner.goatOrdinalByEntity[marker.Index]
                            : -1;
                    var liveGoat = goatOrdinal >= 0;
                    var sourceX = entity.x;
                    var sourceY = entity.y;
                    var sourceZ = entity.z;
                    var sourceYaw = entity.yaw;
                    var animationFrame = marker.Group?.key.animation_frame ?? 0;
                    var active = entity.active != 0;
                    var drawingEnabled = true;
                    if (liveGoat)
                    {
                        var goat = owner.goats[goatOrdinal];
                        MagicCarpetRuntime.Require(
                            goat.entityIndex == marker.Index &&
                            goat.authority == NativeSim.GoatLive,
                            $"stable live Goat mapping {marker.Index}");
                        sourceX = goat.x;
                        sourceY = goat.y;
                        sourceZ = goat.z;
                        sourceYaw = goat.yaw;
                        animationFrame = goat.animationFrame;
                        active = goat.active != 0;
                        drawingEnabled = (goat.controlFlags & 0x00000400) == 0;
                    }
                    var nearestX = owner.unwrappedSourceX +
                        MagicCarpetRuntime.ModularDelta(
                            sourceX,
                            owner.presentationState.cameraX);
                    var nearestY = owner.unwrappedSourceY +
                        MagicCarpetRuntime.ModularDelta(
                            sourceY,
                            owner.presentationState.cameraY);
                    marker.Node.position = MagicCarpetRuntime.SourceToUnity(
                        nearestX,
                        nearestY,
                        sourceZ);
                    if (marker.Renderer != null)
                    {
                        var retiredOrReplacedImportedActor =
                            MagicCarpetRuntime.IsRetiredOrReplacedImportedActor(
                                entity,
                                marker.QualifiedFrozenActor);
                        var nativeFrozenActor =
                            !liveGoat && marker.QualifiedFrozenActor;
                        var distanceSquared = owner.presentationRig == null
                            ? float.PositiveInfinity
                            : UnityEngine.Vector3.SqrMagnitude(
                                marker.Node.position - owner.targetPosition);
                        marker.Renderer.enabled =
                            !retiredOrReplacedImportedActor &&
                            active && drawingEnabled &&
                            !(liveGoat &&
                              (suppressionFlags &
                               NativeSim.TerrainFrameGoatsComposited) != 0) &&
                            !(nativeFrozenActor &&
                              !MagicCarpetRuntime.ManagedFrozenActorVisible(
                                  1,
                                  marker.QualifiedFrozenActor,
                                  suppressionFlags)) &&
                            ((owner.debugEntities && !owner.qualifiedMode) ||
                             distanceSquared >= 1f);
                        if (marker.OwnerSprite && marker.Group != null &&
                            marker.SpriteVisual != null)
                        {
                            if (retiredOrReplacedImportedActor) continue;
                            MagicCarpetRuntime.Require(
                                liveGoat
                                    ? entity.authority == NativeSim.GoatLive &&
                                      marker.Group.group_id == "reset-050"
                                    : entity.authority == NativeSim.FrozenObserved ||
                                      entity.authority == NativeSim.VillagerLive,
                                $"owner sprite authority {marker.Index}");
                            if (owner.flightCamera != null)
                            {
                                marker.Node.rotation =
                                    owner.flightCamera.transform.rotation;
                                owner.UpdateOwnerEntitySprite(
                                    marker.Index,
                                    sourceYaw,
                                    animationFrame,
                                    marker.Group,
                                    marker.SpriteVisual,
                                    marker.Renderer);
                            }
                        }
                    }
                }
            }
        }

        private void UpdateEntityPresentation()
        {
            Level2Entities.Update();
        }
    }
}

namespace MagicCarpet.Native
{
    internal enum CampaignEntityFamily : byte
    {
        Goat = 1,
        Villager = 2,
        Bee = 3,
        Multipart = 4,
        Archer = 5,
        Model9 = 6,
        Trader = 7
    }

    /// <summary>
    /// One presentation row admitted from the current Level 2 native source
    /// frame. It is a view binding only: Rust owns the entity state and Unity
    /// owns the GameObject, indexed material, and transform copy.
    /// </summary>
    internal readonly struct CampaignEntityPresentation
    {
        internal CampaignEntityPresentation(
            CampaignEntityFamily family,
            ulong generation,
            ulong tick,
            ushort entityIndex,
            ushort renderParameterIndex,
            int baseTmap,
            int selector,
            short sourceYaw,
            Vector3 sourcePosition,
            Vector3 scale,
            Quaternion rotation,
            bool mirrored,
            GameObject gameObject)
        {
            Family = family;
            Generation = generation;
            Tick = tick;
            EntityIndex = entityIndex;
            RenderParameterIndex = renderParameterIndex;
            BaseTmap = baseTmap;
            Selector = selector;
            SourceYaw = sourceYaw;
            SourcePosition = sourcePosition;
            Scale = scale;
            Rotation = rotation;
            Mirrored = mirrored;
            GameObject = gameObject;
        }

        internal CampaignEntityFamily Family { get; }
        internal ulong Generation { get; }
        internal ulong Tick { get; }
        internal ushort EntityIndex { get; }
        internal ushort RenderParameterIndex { get; }
        internal int BaseTmap { get; }
        internal int Selector { get; }
        internal short SourceYaw { get; }
        internal Vector3 SourcePosition { get; }
        internal Vector3 Scale { get; }
        internal Quaternion Rotation { get; }
        internal bool Mirrored { get; }
        internal bool Visible => GameObject != null &&
                                  GameObject.GetComponentInChildren<Renderer>() != null &&
                                  GameObject.GetComponentInChildren<Renderer>().enabled;
        internal GameObject GameObject { get; }
    }

    /// <summary>
    /// Presents the typed Level 2 Goat, ordinary Villager, and admitted live
    /// class-5 source rows.
    /// Every frame is validated before the registry is mutated. A malformed,
    /// stale, duplicate, or unbound frame clears the registry instead of
    /// leaving a prior Unity view or creating a placeholder.
    /// </summary>
    internal sealed class CampaignEntityPresenter : IDisposable
    {
        private const int EntityCount = 1000;
        private const int GoatCount = 36;
        private const int EntityStructSize = 52;
        private const int GoatStructSize = 84;
        private const float SourceUnitsPerUnityUnit = 64f;
        private const ushort GoatFirstEntityIndex = 39;
        private const ushort GoatRenderParameterIndex = 238;
        private const ushort GoatBaseTmap = 152;
        private const int GoatLogicalHeight = 150;
        private const ushort VillagerParameterRelation = 100;
        private const byte Class5 = 5;
        private const byte GoatModel = 1;
        private const byte BeeModel = 2;
        private const byte MultipartModel = 3;
        private const byte ArcherModel = 4;
        private const byte Model9 = 9;
        private const byte TraderModel = 14;
        private const byte VillagerModel = 13;

        private static readonly int[] Mode20YawLookup =
            { 0, 0, 0, 1, 1, 1, 2, 2, 2, 2, 2, 1, 1, 1, 0, 0 };

        private static readonly ushort[] Level2VillagerEntityIndices =
        {
            77, 78, 79, 80, 81, 82, 83, 84, 85, 86,
            88, 89, 90, 91, 92, 93, 94, 95, 96, 97,
            98, 99, 100, 101, 102, 103, 104, 105, 106
        };

        private static readonly ushort[] Level2VillagerRenderParameters =
        {
            271, 241, 242, 242, 271, 241, 239, 242, 271, 241,
            271, 271, 241, 242, 242, 239, 239, 242, 271, 241,
            241, 242, 271, 241, 239, 242, 271, 241, 242
        };

        private static readonly Dictionary<ushort, ushort> VillagerRenderParameterByEntity =
            BuildVillagerRenderParameterMap();

        private readonly Transform root;
        private readonly bool ownsRoot;
        private readonly Dictionary<ushort, View> views = new();
        private readonly Dictionary<int, Material> materials = new();
        private readonly Dictionary<int, Texture2D> indexTextures = new();
        private readonly Dictionary<int, CampaignTextureDescriptor> textureRows = new();
        private Mesh quad;
        private Mesh mirroredQuad;
        private Texture2D paletteTexture;
        private OwnerAssetCache loadedCache;
        private PresenterCleanup cleanup;
        private CampaignEntityPresentation[] presentations =
            Array.Empty<CampaignEntityPresentation>();
        private ulong observedGeneration = ulong.MaxValue;
        private ulong observedTick;
        private bool disposed;

        internal CampaignEntityPresenter()
            : this(null)
        {
        }

        internal CampaignEntityPresenter(Transform parent)
        {
            root = new GameObject(
                "Level 2 Rust entity presentation").transform;
            ownsRoot = true;
            if (parent != null)
            {
                root.SetParent(parent, false);
            }
        }

        internal Transform Root => root;

        internal CampaignEntityPresentation[] Presentations => presentations;

        internal int LiveViewCount => views.Count;

        internal bool LastPresentSucceeded { get; private set; }

        internal void AttachTo(Transform parent)
        {
            if (disposed || parent == null || root == null)
            {
                return;
            }

            if (root.parent != parent)
            {
                root.SetParent(parent, true);
            }

            cleanup ??= root.gameObject.AddComponent<PresenterCleanup>();
            cleanup.Presenter = this;
        }

        internal bool Present(
            NativeCampaignSession session,
            OwnerAssetCache assetCache,
            short cameraYaw)
        {
            if (session == null || disposed)
            {
                return FailClosed(observedGeneration, observedTick);
            }

            try
            {
                return Present(
                    session.Generation,
                    session.Tick,
                    session.Entities(),
                    session.Goats(),
                    assetCache,
                    cameraYaw);
            }
            catch (Exception)
            {
                return FailClosed(session.Generation, session.Tick);
            }
        }

        internal bool Present(
            ulong generation,
            ulong tick,
            NativeSim.CampaignEntitySnapshot[] entities,
            NativeSim.CampaignGoatSnapshot[] goats,
            OwnerAssetCache assetCache,
            short cameraYaw)
        {
            if (disposed)
            {
                return FailClosed(generation, tick);
            }

            if (observedGeneration != ulong.MaxValue &&
                (generation < observedGeneration ||
                 generation == observedGeneration && tick < observedTick))
            {
                return FailClosedWithoutAdvancingFrame();
            }

            if (observedGeneration != ulong.MaxValue &&
                generation > observedGeneration)
            {
                ClearViews();
            }

            if (!TryBuildProjections(
                    generation,
                    tick,
                    entities,
                    goats,
                    cameraYaw,
                    out var projections) ||
                !TryLoadMaterialDescriptor(assetCache))
            {
                return FailClosed(generation, tick);
            }

            try
            {
                foreach (var projection in projections)
                {
                    if (EnsureMaterial(projection.Selector) == null)
                    {
                        return FailClosed(generation, tick);
                    }
                }

                var liveIds = new HashSet<ushort>();
                var nextPresentations = new List<CampaignEntityPresentation>(
                    projections.Count);
                foreach (var projection in projections)
                {
                    liveIds.Add(projection.EntityIndex);
                    var view = UpsertView(projection);
                    nextPresentations.Add(view.Presentation);
                }

                RemoveMissingViews(liveIds);
                presentations = nextPresentations.ToArray();
                observedGeneration = generation;
                observedTick = tick;
                LastPresentSucceeded = true;
                return true;
            }
            catch (Exception)
            {
                return FailClosed(generation, tick);
            }
        }

        internal void Reset(ulong generation)
        {
            if (disposed)
            {
                return;
            }

            ClearViews();
            observedGeneration = generation;
            observedTick = 0;
            LastPresentSucceeded = false;
        }

        public void Dispose()
        {
            if (disposed)
            {
                return;
            }

            ClearViews();
            DisposeAssets();
            DestroyObject(quad);
            DestroyObject(mirroredQuad);
            quad = null;
            mirroredQuad = null;
            if (cleanup != null)
            {
                cleanup.Presenter = null;
                cleanup = null;
            }
            if (ownsRoot && root != null)
            {
                DestroyObject(root.gameObject);
            }

            presentations = Array.Empty<CampaignEntityPresentation>();
            disposed = true;
            LastPresentSucceeded = false;
        }

        private static Dictionary<ushort, ushort> BuildVillagerRenderParameterMap()
        {
            var result = new Dictionary<ushort, ushort>();
            for (var index = 0; index < Level2VillagerEntityIndices.Length; index++)
            {
                result.Add(
                    Level2VillagerEntityIndices[index],
                    Level2VillagerRenderParameters[index]);
            }

            return result;
        }

        private void DisposeFromParentDestroy()
        {
            if (disposed)
            {
                return;
            }

            // The parent is already destroying the view hierarchy. Clearing
            // the registry without destroying child nodes again avoids
            // re-entering Unity's parent-destroy traversal.
            views.Clear();
            presentations = Array.Empty<CampaignEntityPresentation>();
            DisposeAssets();
            DestroyObject(quad);
            DestroyObject(mirroredQuad);
            quad = null;
            mirroredQuad = null;
            cleanup = null;
            disposed = true;
            LastPresentSucceeded = false;
        }

        private bool TryBuildProjections(
            ulong generation,
            ulong tick,
            NativeSim.CampaignEntitySnapshot[] entities,
            NativeSim.CampaignGoatSnapshot[] goats,
            short cameraYaw,
            out List<Projection> projections)
        {
            projections = new List<Projection>(GoatCount +
                                                Level2VillagerEntityIndices.Length + 16);
            if (entities == null || goats == null ||
                entities.Length != EntityCount || goats.Length != GoatCount)
            {
                return false;
            }

            var entitySeen = new bool[EntityCount];
            foreach (var entity in entities)
            {
                if (entity.entityIndex >= EntityCount ||
                    entitySeen[entity.entityIndex] ||
                    entity.abiVersion != NativeSim.AbiVersion ||
                    entity.structSize != EntityStructSize ||
                    !Zero(entity.reserved))
                {
                    return false;
                }

                entitySeen[entity.entityIndex] = true;
            }

            var goatSeen = new bool[EntityCount];
            foreach (var goat in goats)
            {
                if (goat.entityIndex >= EntityCount ||
                    goatSeen[goat.entityIndex] ||
                    goat.entityIndex < GoatFirstEntityIndex ||
                    goat.entityIndex >= GoatFirstEntityIndex + GoatCount ||
                    goat.abiVersion != NativeSim.AbiVersion ||
                    goat.structSize != GoatStructSize ||
                    goat.baseTmap != GoatBaseTmap ||
                    goat.renderParameterIndex != GoatRenderParameterIndex ||
                    !Zero(goat.reserved2) ||
                    goat.reserved0 != 0 ||
                    goat.reserved1 != 0 ||
                    !KnownGoatAction(goat.action))
                {
                    return false;
                }

                goatSeen[goat.entityIndex] = true;
                var entity = entities[goat.entityIndex];
                if (goat.authority != NativeSim.GoatLive || goat.active == 0)
                {
                    continue;
                }

                if (!MatchesLiveGoat(entity, goat))
                {
                    return false;
                }

                if ((goat.controlFlags & 0x00000021) != 0)
                {
                    continue;
                }

                Mode20Selection(
                    GoatBaseTmap,
                    goat.yaw,
                    cameraYaw,
                    out var selector,
                    out var mirrored);
                projections.Add(
                    new Projection(
                        CampaignEntityFamily.Goat,
                        generation,
                        tick,
                        goat.entityIndex,
                        GoatRenderParameterIndex,
                        GoatBaseTmap,
                        GoatLogicalHeight,
                        selector,
                        goat.yaw,
                        SourceToUnity(goat.x, goat.y, goat.z),
                        mirrored,
                        true));
            }

            for (var ordinal = 0; ordinal < GoatCount; ordinal++)
            {
                if (!goatSeen[GoatFirstEntityIndex + ordinal])
                {
                    return false;
                }
            }

            foreach (var entity in entities)
            {
                if (entity.active == 0 ||
                    entity.classId != Class5 ||
                    entity.model != GoatModel)
                {
                    continue;
                }

                if (entity.entityIndex < GoatFirstEntityIndex ||
                    entity.entityIndex >= GoatFirstEntityIndex + GoatCount ||
                    !goatSeen[entity.entityIndex])
                {
                    return false;
                }
            }

            for (var index = 0; index < Level2VillagerEntityIndices.Length; index++)
            {
                var entityIndex = Level2VillagerEntityIndices[index];
                var entity = entities[entityIndex];
                if (entity.active == 0)
                {
                    continue;
                }

                if (!VillagerRenderParameterByEntity.TryGetValue(
                        entityIndex,
                        out var renderParameter) ||
                    entity.authority != NativeSim.FrozenObserved ||
                    entity.sourceId != entityIndex ||
                    entity.classId != Class5 ||
                    entity.model != VillagerModel ||
                    entity.parameterRelation != VillagerParameterRelation ||
                    !KnownVillagerAction(entity.action) ||
                    !TryVillagerSprite(
                        renderParameter,
                        out var baseTmap,
                        out var logicalHeight))
                {
                    return false;
                }

                if (entity.action == 108 || entity.action == 109)
                {
                    continue;
                }

                Mode20Selection(
                    baseTmap,
                    entity.yaw,
                    cameraYaw,
                    out var selector,
                    out var mirrored);
                projections.Add(
                    new Projection(
                        CampaignEntityFamily.Villager,
                        generation,
                        tick,
                        entity.entityIndex,
                        renderParameter,
                        baseTmap,
                        logicalHeight,
                        selector,
                        entity.yaw,
                        SourceToUnity(entity.x, entity.y, entity.z),
                        mirrored,
                        true));
            }

            foreach (var entity in entities)
            {
                if (entity.active != 0 &&
                    entity.classId == Class5 &&
                    entity.model == VillagerModel &&
                    !VillagerRenderParameterByEntity.ContainsKey(entity.entityIndex))
                {
                    return false;
                }
            }

            foreach (var entity in entities)
            {
                if (entity.authority != NativeSim.CampaignLive)
                {
                    continue;
                }

                if (entity.active > 1 ||
                    entity.classId != Class5 ||
                    entity.sourceId != entity.entityIndex ||
                    entity.nextRelation != ushort.MaxValue ||
                    entity.parameterRelation != ushort.MaxValue ||
                    !TryClass5Sprite(
                        entity.model,
                        entity.action,
                        out var family,
                        out var renderParameter,
                        out var baseTmap,
                        out var logicalHeight,
                        out var renderMode) ||
                    !TryDirectionalSelection(
                        renderMode,
                        baseTmap,
                        entity.yaw,
                        cameraYaw,
                        out var selector,
                        out var mirrored))
                {
                    return false;
                }

                projections.Add(
                    new Projection(
                        family,
                        generation,
                        tick,
                        entity.entityIndex,
                        renderParameter,
                        baseTmap,
                        logicalHeight,
                        selector,
                        entity.yaw,
                        SourceToUnity(entity.x, entity.y, entity.z),
                        mirrored,
                        entity.active != 0));
            }

            return true;
        }

        private static bool MatchesLiveGoat(
            NativeSim.CampaignEntitySnapshot entity,
            NativeSim.CampaignGoatSnapshot goat)
        {
            // The campaign entity endpoint is the frozen source identity
            // frame. Live action, phase, pose, and yaw are authoritative in
            // the typed Goat sidecar and are allowed to advance independently.
            return entity.authority == NativeSim.FrozenObserved &&
                   entity.active != 0 &&
                   entity.sourceId == goat.entityIndex &&
                   entity.classId == goat.classId &&
                   entity.model == goat.model &&
                   entity.parameterRelation == 98;
        }

        private static bool KnownGoatAction(byte action)
        {
            return action >= 8 && action <= 15;
        }

        private static bool KnownVillagerAction(byte action)
        {
            return action >= 104 && action <= 111;
        }

        private static bool TryClass5Sprite(
            byte model,
            byte action,
            out CampaignEntityFamily family,
            out ushort renderParameter,
            out int baseTmap,
            out int logicalHeight,
            out byte renderMode)
        {
            switch (model)
            {
                case BeeModel when action >= 16 && action <= 23:
                    family = CampaignEntityFamily.Bee;
                    renderParameter = 3;
                    baseTmap = 14;
                    logicalHeight = 150;
                    renderMode = 17;
                    return true;
                case MultipartModel when action >= 25 && action <= 31:
                    family = CampaignEntityFamily.Multipart;
                    renderParameter = 88;
                    baseTmap = 30;
                    logicalHeight = 180;
                    renderMode = 17;
                    return true;
                case ArcherModel when action >= 33 && action <= 39:
                    family = CampaignEntityFamily.Archer;
                    renderParameter = 0;
                    baseTmap = 8;
                    logicalHeight = 250;
                    renderMode = 20;
                    return true;
                case Model9 when action == 72 || action == 73:
                    family = CampaignEntityFamily.Model9;
                    renderParameter = 220;
                    baseTmap = 158;
                    logicalHeight = 125;
                    renderMode = 0;
                    return true;
                case TraderModel when action >= 112 && action <= 119:
                    family = CampaignEntityFamily.Trader;
                    renderParameter = 219;
                    baseTmap = 138;
                    logicalHeight = 250;
                    renderMode = 20;
                    return true;
                default:
                    family = default;
                    renderParameter = 0;
                    baseTmap = 0;
                    logicalHeight = 0;
                    renderMode = 0;
                    return false;
            }
        }

        private static bool TryDirectionalSelection(
            byte renderMode,
            int baseTmap,
            short entityYaw,
            short cameraYaw,
            out int selector,
            out bool mirrored)
        {
            switch (renderMode)
            {
                case 0:
                    selector = baseTmap;
                    mirrored = false;
                    return true;
                case 17:
                    Mode17Selection(
                        baseTmap,
                        entityYaw,
                        cameraYaw,
                        out selector,
                        out mirrored);
                    return true;
                case 20:
                    Mode20Selection(
                        baseTmap,
                        entityYaw,
                        cameraYaw,
                        out selector,
                        out mirrored);
                    return true;
                default:
                    selector = 0;
                    mirrored = false;
                    return false;
            }
        }

        private static void Mode17Selection(
            int baseTmap,
            short entityYaw,
            short cameraYaw,
            out int selector,
            out bool mirrored)
        {
            var orientation = ((((int)entityYaw - (ushort)cameraYaw) >> 3) & 0xf0) >> 4;
            selector = baseTmap + (orientation < 8 ? orientation : 15 - orientation);
            mirrored = orientation >= 8;
        }

        private static bool TryVillagerSprite(
            ushort renderParameter,
            out int baseTmap,
            out int logicalHeight)
        {
            switch (renderParameter)
            {
                case 239:
                    baseTmap = 155;
                    logicalHeight = 200;
                    return true;
                case 241:
                    baseTmap = 180;
                    logicalHeight = 250;
                    return true;
                case 242:
                    baseTmap = 183;
                    logicalHeight = 250;
                    return true;
                case 271:
                    baseTmap = 199;
                    logicalHeight = 300;
                    return true;
                default:
                    baseTmap = 0;
                    logicalHeight = 0;
                    return false;
            }
        }

        private static void Mode20Selection(
            int baseTmap,
            short entityYaw,
            short cameraYaw,
            out int selector,
            out bool mirrored)
        {
            var orientation = ((((int)entityYaw - (ushort)cameraYaw) >> 3) & 0xf0) >> 4;
            selector = baseTmap + Mode20YawLookup[orientation];
            mirrored = orientation >= 8;
        }

        private static Vector3 SourceToUnity(ushort x, ushort y, short z)
        {
            return new Vector3(
                x / SourceUnitsPerUnityUnit,
                z / SourceUnitsPerUnityUnit,
                y / SourceUnitsPerUnityUnit);
        }

        private bool TryLoadMaterialDescriptor(OwnerAssetCache assetCache)
        {
            if (assetCache == null)
            {
                return false;
            }

            if (ReferenceEquals(loadedCache, assetCache) && textureRows.Count > 0)
            {
                return true;
            }

            if (loadedCache != null &&
                !string.Equals(
                    loadedCache.ContentHash,
                    assetCache.ContentHash,
                    StringComparison.Ordinal))
            {
                ClearViews();
                DisposeAssets();
            }

            try
            {
                var descriptor = JsonUtility.FromJson<CampaignMaterialDescriptor>(
                    assetCache.ReadOutputText(
                        "components/material/materials/descriptor.json"));
                if (descriptor == null ||
                    descriptor.schema != "mc2-level2-day-materials/v1" ||
                    descriptor.palette == null ||
                    descriptor.palette.entries != 256 ||
                    descriptor.palette.rgb8_path != "palette/pald.rgb8" ||
                    descriptor.tmaps0 == null ||
                    descriptor.tmaps0.bank != "TMAPS0-0" ||
                    descriptor.tmaps0.palette_asset_id !=
                    "owner_data:DATA/PALD-0.DAT" ||
                    descriptor.tmaps0.textures == null)
                {
                    return false;
                }

                textureRows.Clear();
                foreach (var row in descriptor.tmaps0.textures)
                {
                    if (row == null)
                    {
                        return false;
                    }

                    if (row.index < 0 ||
                        row.width <= 0 || row.height <= 0 ||
                        row.frame_count <= 0 || string.IsNullOrWhiteSpace(row.indices_path) ||
                        textureRows.ContainsKey(row.index))
                    {
                        return false;
                    }

                    textureRows.Add(row.index, row);
                }

                loadedCache = assetCache;
                return true;
            }
            catch (Exception)
            {
                textureRows.Clear();
                return false;
            }
        }

        private Material EnsureMaterial(int selector)
        {
            if (materials.TryGetValue(selector, out var existing))
            {
                return existing;
            }

            if (!textureRows.TryGetValue(selector, out var row) ||
                loadedCache == null)
            {
                return null;
            }

            var indices = loadedCache.ReadOutput(
                $"components/material/{row.indices_path}");
            if (indices.Length != row.width * row.height)
            {
                return null;
            }

            var palette = EnsurePalette();
            if (palette == null)
            {
                return null;
            }

            var indicesTexture = new Texture2D(
                row.width,
                row.height,
                TextureFormat.R8,
                false,
                true)
            {
                name = $"Level 2 indexed entity selector {selector}",
                filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Clamp
            };
            indicesTexture.LoadRawTextureData(indices);
            indicesTexture.Apply(false, true);

            var shader = Resources.Load<Shader>("EntitySprite");
            if (shader == null)
            {
                DestroyObject(indicesTexture);
                return null;
            }

            var material = new Material(shader)
            {
                name = $"Level 2 indexed entity selector {selector}"
            };
            material.SetTexture("_Indices", indicesTexture);
            material.SetTexture("_Palette", palette);
            materials.Add(selector, material);
            indexTextures.Add(selector, indicesTexture);
            return material;
        }

        private Texture2D EnsurePalette()
        {
            if (paletteTexture != null)
            {
                return paletteTexture;
            }

            if (loadedCache == null)
            {
                return null;
            }

            var bytes = loadedCache.ReadOutput(
                "components/material/palette/pald.rgb8");
            if (bytes.Length != 256 * 3)
            {
                return null;
            }

            paletteTexture = new Texture2D(
                256,
                1,
                TextureFormat.RGB24,
                false,
                false)
            {
                name = "Level 2 Day PALD RGB8 entity palette",
                filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Clamp
            };
            paletteTexture.LoadRawTextureData(bytes);
            paletteTexture.Apply(false, true);
            return paletteTexture;
        }

        private View UpsertView(Projection projection)
        {
            if (!views.TryGetValue(projection.EntityIndex, out var view))
            {
                var node = new GameObject(
                    $"Level 2 {projection.Family} {projection.EntityIndex:D3}");
                node.transform.SetParent(root, false);
                var visual = new GameObject(
                    $"Level 2 {projection.Family} {projection.EntityIndex:D3} sprite");
                visual.transform.SetParent(node.transform, false);
                var filter = visual.AddComponent<MeshFilter>();
                var renderer = visual.AddComponent<MeshRenderer>();
                view = new View(node, visual.transform, filter, renderer);
                views.Add(projection.EntityIndex, view);
            }

            if (view == null)
            {
                throw new InvalidOperationException(
                    "Level 2 entity view registry returned a null view");
            }

            var row = textureRows[projection.Selector];
            var height = projection.LogicalHeight / SourceUnitsPerUnityUnit;
            var width = height * row.width / row.height;
            var scale = new Vector3(width, height, 1f);
            var rotation = Quaternion.Euler(
                0f,
                180f - projection.SourceYaw * (360f / 2048f),
                0f);
            view.Node.transform.SetPositionAndRotation(
                projection.SourcePosition,
                rotation);
            view.Visual.localPosition = new Vector3(0f, height * 0.5f, 0f);
            view.Visual.localScale = scale;
            view.Filter.sharedMesh = GetQuad(projection.Mirrored);
            view.Renderer.sharedMaterial = materials[projection.Selector];
            view.Renderer.enabled = projection.Visible;
            view.Presentation = new CampaignEntityPresentation(
                projection.Family,
                projection.Generation,
                projection.Tick,
                projection.EntityIndex,
                projection.RenderParameterIndex,
                projection.BaseTmap,
                projection.Selector,
                projection.SourceYaw,
                projection.SourcePosition,
                scale,
                rotation,
                projection.Mirrored,
                view.Node);
            return view;
        }

        private Mesh GetQuad(bool mirrored)
        {
            if (mirrored && mirroredQuad != null)
            {
                return mirroredQuad;
            }

            if (!mirrored && quad != null)
            {
                return quad;
            }

            var mesh = new Mesh
            {
                name = mirrored
                    ? "Level 2 entity mirrored billboard quad"
                    : "Level 2 entity billboard quad"
            };
            mesh.vertices = new[]
            {
                new Vector3(-0.5f, -0.5f, 0f),
                new Vector3(0.5f, -0.5f, 0f),
                new Vector3(-0.5f, 0.5f, 0f),
                new Vector3(0.5f, 0.5f, 0f)
            };
            mesh.uv = mirrored
                ? new[]
                {
                    new Vector2(1f, 1f), new Vector2(0f, 1f),
                    new Vector2(1f, 0f), new Vector2(0f, 0f)
                }
                : new[]
                {
                    new Vector2(0f, 1f), new Vector2(1f, 1f),
                    new Vector2(0f, 0f), new Vector2(1f, 0f)
                };
            mesh.triangles = new[] { 0, 2, 1, 2, 3, 1 };
            mesh.RecalculateBounds();
            if (mirrored) mirroredQuad = mesh;
            else quad = mesh;
            return mesh;
        }

        private void RemoveMissingViews(HashSet<ushort> liveIds)
        {
            var removed = new List<ushort>();
            foreach (var pair in views)
            {
                if (liveIds.Contains(pair.Key))
                {
                    continue;
                }

                DestroyObject(pair.Value.Node);
                removed.Add(pair.Key);
            }

            foreach (var entityIndex in removed)
            {
                views.Remove(entityIndex);
            }
        }

        private void ClearViews()
        {
            foreach (var view in views.Values)
            {
                DestroyObject(view.Node);
            }

            views.Clear();
            presentations = Array.Empty<CampaignEntityPresentation>();
        }

        private void DisposeAssets()
        {
            foreach (var material in materials.Values)
            {
                DestroyObject(material);
            }

            foreach (var texture in indexTextures.Values)
            {
                DestroyObject(texture);
            }

            DestroyObject(paletteTexture);
            materials.Clear();
            indexTextures.Clear();
            textureRows.Clear();
            paletteTexture = null;
            loadedCache = null;
        }

        private bool FailClosed(ulong generation, ulong tick)
        {
            ClearViews();
            if (observedGeneration == ulong.MaxValue ||
                generation > observedGeneration ||
                generation == observedGeneration && tick >= observedTick)
            {
                observedGeneration = generation;
                observedTick = tick;
            }

            LastPresentSucceeded = false;
            return false;
        }

        private bool FailClosedWithoutAdvancingFrame()
        {
            ClearViews();
            LastPresentSucceeded = false;
            return false;
        }

        private static bool Zero(uint[] values)
        {
            if (values == null || values.Length == 0)
            {
                return false;
            }

            foreach (var value in values)
            {
                if (value != 0)
                {
                    return false;
                }
            }

            return true;
        }

        private static void DestroyObject(UnityEngine.Object value)
        {
            if (value == null)
            {
                return;
            }

            if (Application.isPlaying)
            {
                UnityEngine.Object.Destroy(value);
            }
            else
            {
                UnityEngine.Object.DestroyImmediate(value);
            }
        }

        [Serializable]
        private sealed class CampaignMaterialDescriptor
        {
            public string schema;
            public CampaignPaletteDescriptor palette;
            public CampaignTmapsDescriptor tmaps0;
        }

        [Serializable]
        private sealed class CampaignPaletteDescriptor
        {
            public int entries;
            public string rgb8_path;
        }

        [Serializable]
        private sealed class CampaignTmapsDescriptor
        {
            public string bank;
            public string palette_asset_id;
            public CampaignTextureDescriptor[] textures;
        }

        [Serializable]
        private sealed class CampaignTextureDescriptor
        {
            public int index;
            public int width;
            public int height;
            public int frame_count;
            public string indices_path;
        }

        private readonly struct Projection
        {
            internal Projection(
                CampaignEntityFamily family,
                ulong generation,
                ulong tick,
                ushort entityIndex,
                ushort renderParameterIndex,
                int baseTmap,
                int logicalHeight,
                int selector,
                short sourceYaw,
                Vector3 sourcePosition,
                bool mirrored,
                bool visible)
            {
                Family = family;
                Generation = generation;
                Tick = tick;
                EntityIndex = entityIndex;
                RenderParameterIndex = renderParameterIndex;
                BaseTmap = baseTmap;
                LogicalHeight = logicalHeight;
                Selector = selector;
                SourceYaw = sourceYaw;
                SourcePosition = sourcePosition;
                Mirrored = mirrored;
                Visible = visible;
            }

            internal CampaignEntityFamily Family { get; }
            internal ulong Generation { get; }
            internal ulong Tick { get; }
            internal ushort EntityIndex { get; }
            internal ushort RenderParameterIndex { get; }
            internal int BaseTmap { get; }
            internal int LogicalHeight { get; }
            internal int Selector { get; }
            internal short SourceYaw { get; }
            internal Vector3 SourcePosition { get; }
            internal bool Mirrored { get; }
            internal bool Visible { get; }
        }

        private sealed class View
        {
            internal View(
                GameObject node,
                Transform visual,
                MeshFilter filter,
                MeshRenderer renderer)
            {
                Node = node;
                Visual = visual;
                Filter = filter;
                Renderer = renderer;
            }

            internal readonly GameObject Node;
            internal readonly Transform Visual;
            internal readonly MeshFilter Filter;
            internal readonly MeshRenderer Renderer;
            internal CampaignEntityPresentation Presentation;
        }

        private sealed class PresenterCleanup : MonoBehaviour
        {
            internal CampaignEntityPresenter Presenter;

            private void OnDestroy()
            {
                Presenter?.DisposeFromParentDestroy();
            }
        }
    }

    public sealed partial class MagicCarpetRuntime
    {
        private global::MagicCarpet.Native.CampaignEntityPresenter
            level2ProductionEntityPresenter;
        private CampaignEntityPresenterDriver level2EntityPresenterDriver;

        private void OnEnable()
        {
            EnsureLevel2EntityPresenterDriver();
        }

        internal CampaignEntityPresentation[] QualificationLevel2EntityPresentations
        {
            get
            {
                if ((!level2Mode || campaignSession == null) &&
                    level2ProductionEntityPresenter != null)
                {
                    QualificationDisposeLevel2EntityPresenter();
                }

                return level2ProductionEntityPresenter?.Presentations ??
                       Array.Empty<CampaignEntityPresentation>();
            }
        }

        internal bool QualificationPresentLevel2EntityRows()
        {
            if (!level2Mode || campaignSession == null || ownerAssetCache == null)
            {
                return false;
            }

            UpdateEntityPresentation();
            return level2ProductionEntityPresenter != null &&
                   level2ProductionEntityPresenter.LastPresentSucceeded;
        }

        internal void QualificationDisposeLevel2EntityPresenter()
        {
            level2ProductionEntityPresenter?.Dispose();
            level2ProductionEntityPresenter = null;
        }

        private void PresentLevel2ProductionEntityRows()
        {
            if (!level2Mode || campaignSession == null || ownerAssetCache == null)
            {
                QualificationDisposeLevel2EntityPresenter();
                return;
            }

            EnsureLevel2EntityPresenterDriver();
            var presenter = level2ProductionEntityPresenter;
            if (presenter == null || presenter.Root == null)
            {
                presenter?.Dispose();
                presenter =
                    new global::MagicCarpet.Native.CampaignEntityPresenter(transform);
                level2ProductionEntityPresenter = presenter;
            }
            if (presenter == null)
            {
                return;
            }

            var parent = level2IndexedFrameObject == null
                ? transform
                : level2IndexedFrameObject.transform;
            presenter.AttachTo(parent);
            presenter.Present(
                campaignSession,
                ownerAssetCache,
                renderCameraState.yaw);
        }

        private void EnsureLevel2EntityPresenterDriver()
        {
            var driver = level2EntityPresenterDriver;
            if (driver == null)
            {
                driver = gameObject.GetComponent<CampaignEntityPresenterDriver>();
            }

            if (driver == null)
            {
                driver = gameObject.AddComponent<CampaignEntityPresenterDriver>();
            }

            if (driver == null)
            {
                return;
            }

            driver.Owner = this;
            level2EntityPresenterDriver = driver;
        }

        private sealed class CampaignEntityPresenterDriver : MonoBehaviour
        {
            internal MagicCarpetRuntime Owner;

            private void LateUpdate()
            {
                if (Owner == null)
                {
                    return;
                }

                Owner.PresentLevel2ProductionEntityRows();
            }
        }
    }
}
