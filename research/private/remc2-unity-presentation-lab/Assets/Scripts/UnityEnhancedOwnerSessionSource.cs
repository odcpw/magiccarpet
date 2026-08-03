using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using UnityEngine;

namespace MagicCarpet.PresentationLab
{
    /// <summary>
    /// Owner-bound Rust source for the Unity Enhanced presentation route.
    ///
    /// This adapter owns no gameplay state. It opens the external owner
    /// package, submits the same command/camera records as the native client,
    /// and translates immutable native observations into an Enhanced snapshot
    /// consumed by UnityEnhancedWorldRoot. The generated meshes, lights, and
    /// effects remain presentation-only.
    /// </summary>
    public sealed class UnityEnhancedOwnerSessionSource : IDisposable
    {
        private const string Library = "sim_capi";
        private const string AssetCacheEnvironment = "MC_ASSET_CACHE_PATH";
        private const uint AbiVersion = 2;
        private const uint SessionSnapshotSize = 120;
        private const uint PresentationStateSize = 64;
        private const uint RenderCameraStateSize = 72;
        private const uint TerrainStateSize = 64;
        private const uint EntityElementSize = 52;
        private const uint MissionSnapshotSize = 96;
        private const uint ProjectileElementSize = 56;
        private const uint ImpactElementSize = 52;
        private const uint MinimapMarksSize = 168;
        private const uint MinimapEntityElementSize = 24;
        private const uint MinimapEntitiesInfoSize = 64;
        private const uint EventQuerySize = 40;
        private const uint EventBatchSize = 56;
        private const uint EventElementSize = 72;
        private const int OwnerHudWidth = 320;
        private const int OwnerHudHeight = 200;
        private const int OwnerHudStride = OwnerHudWidth;
        private const uint OwnerHudQuerySize = 64;
        private const uint OwnerHudInfoSize = 128;
        private const int OwnerPaletteBytes = 256 * 3;
        private const uint RequiredOwnerHudFlags =
            0x2000u | // compositor complete
            0x4000u | // provenance present
            0x8000u | // overlays composited
            0x10000u | // postprocess complete
            0x20000u; // lighting complete
        private const string OwnerPaletteRelativePath = "palette/paln.rgb8";
        private const byte Level1MissionLive = 6;
        private const int EventBatchCapacity = 128;
        private const int MinimapMaxMarks = 16;
        private const int MinimapMaxEntityCommands = 1152;
        private const int TerrainWidth = 256;
        private const int TerrainHeight = 256;
        private const int TerrainCellCount = TerrainWidth * TerrainHeight;
        private const float SourceUnitsPerUnityUnit = 64f;
        private const float TerrainSourceStep = 256f;
        private const float TerrainCellSize = TerrainSourceStep / SourceUnitsPerUnityUnit;
        // Owner-native rows without a verified Enhanced family are omitted;
        // Generic remains an explicit diagnostic-only presentation family.
        private const int UnsupportedPresentationFamily = -1;
        // Enhanced-only vertical presentation compression keeps owner terrain
        // relief readable while preserving authoritative source units and the
        // existing horizontal mapping. Exact and Classic HD do not use it.
        public const float EnhancedVerticalPresentationScale = 0.5f;
        // Enhanced keeps the Rust camera as its source, then gives the
        // modern presentation a readable cinematic eye height. This never
        // feeds back into gameplay or the Exact/Classic HD routes.
        private const float EnhancedCameraLift = 16f;
        private const float EnhancedCameraPitchBias = 24f;
        private const uint TerrainTypes = 0;
        private const uint TerrainHeightPlane = 1;
        private const uint TerrainShade = 2;
        private const uint TerrainAngle = 3;
        private const byte CameraAuthorityPlayerFlightLive = 2;
        private const byte PresentationCameraExactInputs = 0x01;
        private const byte PresentationCameraEyeZPlus128 = 0x02;
        private const byte RenderCameraTerrainSlopeXY = 0x01;
        private const byte SessionTerrainLive = 7;
        private const byte TerrainHashSchema = 1;
        private const int SelectedSpellFlag = 0x40;
        private const sbyte SpellSelectionOpcode = 40;
        private const uint EventRequestSpellActivation = 4;
        private const uint EventLevel1FireballSpawned = 15;
        private const uint EventLevel1FireballImpacted = 16;
        private const uint EventLevel1ActorKilled = 18;
        private const uint EventLevel1ArcherArrowSpawned = 24;
        private const uint EventLevel1FireflyFireballSpawned = 26;
        private const uint EventLevel1CastleProjectileSpawned = 28;
        private const uint EventLevel1PossessionSpawned = 31;
        private const uint EventLevel1ManaSphereSpawned = 32;
        private const uint EventLevel1WaterSplashed = 37;

        private byte[] packageBytes;
        private byte[] closureBytes;
        private ulong handle;
        private string priorAssetCacheEnvironment;
        private bool ownsAssetCacheEnvironment;
        private byte[] terrainTypes;
        private byte[] terrainHeights;
        private byte[] terrainShade;
        private byte[] terrainAngles;
        private byte[] ownerHudIndexBytes;
        private byte[] ownerHudProvenanceBytes;
        private byte[] ownerPaletteRgb8;
        private GCHandle ownerHudIndexHandle;
        private GCHandle ownerHudProvenanceHandle;
        private bool ownerHudIndexPinned;
        private bool ownerHudProvenancePinned;
        private McPresentationFrameQueryV2 ownerHudFrameQuery;
        private ulong ownerHudPresentationRevision;
        private McPresentationFrameInfoV2 ownerHudFrameInfo;
        private EntitySnapshot[] entities = Array.Empty<EntitySnapshot>();
        private ulong terrainMutationSequence;
        private bool terrainChangedSinceLastObservation;
        private MissionSnapshot mission;
        private ProjectileSnapshot[] projectiles = Array.Empty<ProjectileSnapshot>();
        private ImpactSnapshot[] impacts = Array.Empty<ImpactSnapshot>();
        private MinimapMarks minimapMarks;
        private MinimapEntitiesInfo minimapEntitiesInfo;
        private MinimapEntityCommand[] minimapCommands = Array.Empty<MinimapEntityCommand>();
        private EventSnapshot[] pendingEvents = Array.Empty<EventSnapshot>();
        private ulong eventCursor;
        private ulong eventHead;
        private int lastEventCount;
        private int lastVfxEventCount;
        private ushort previousCameraX;
        private ushort previousCameraY;
        private long unwrappedCameraX;
        private long unwrappedCameraY;
        private ulong cameraGeneration;
        private bool hasCameraHistory;
        private Vector3 lastPublishedTerrainOrigin;
        private bool hasPublishedTerrainOrigin;
        private UnityEnhancedWorldSnapshot pendingResetSnapshot;
        private ulong nextVfxSequence;
        private SessionSnapshot snapshot;
        private PresentationState presentation;
        private RenderCameraState renderCamera;

        public bool IsConfigured { get; private set; }
        public ulong Generation { get { return snapshot.generation; } }
        public ulong Tick { get { return snapshot.tick; } }
        public ulong LastStateHash { get { return snapshot.stateHash; } }
        public ulong LastConformanceHash { get { return snapshot.conformanceHash; } }
        public ulong PresentationTick { get { return presentation.tick; } }
        public ulong LastTerrainHash { get; private set; }
        public ulong LastWorldHash { get; private set; }
        public ulong TerrainMutationSequence { get { return terrainMutationSequence; } }
        public bool LastTerrainSnapshotPublished { get; private set; }
        public bool LastTerrainMutationPublished { get; private set; }
        public bool LastTerrainOriginChanged { get; private set; }
        public bool LastTerrainFullUpdate { get; private set; }
        public ulong EventCursor { get { return eventCursor; } }
        public ulong EventHead { get { return eventHead; } }
        public int LastEventCount { get { return lastEventCount; } }
        public int LastVfxEventCount { get { return lastVfxEventCount; } }
        public int MissionProjectileCount { get { return checked((int)mission.projectileCount); } }
        public int MissionImpactCount { get { return checked((int)mission.impactCount); } }
        public int MinimapCommandCount { get { return minimapCommands == null ? 0 : minimapCommands.Length; } }
        public ulong OwnerHudFrameGeneration { get { return ownerHudFrameInfo.generation; } }
        public ulong OwnerHudFrameTick { get { return ownerHudFrameInfo.tick; } }
        public int OwnerHudFrameWidth { get { return OwnerHudWidth; } }
        public int OwnerHudFrameHeight { get { return OwnerHudHeight; } }

        public static bool CanReuseOwnerHudFrame(
            uint receiptAbiVersion,
            uint receiptStructSize,
            ulong receiptGeneration,
            ulong receiptCameraTick,
            ulong expectedGeneration,
            ulong expectedCameraTick,
            ulong receiptRevision,
            ulong expectedRevision,
            uint receiptWidth,
            uint receiptHeight,
            uint receiptStride,
            uint receiptIndexBytes,
            uint receiptProvenanceBytes,
            uint receiptFlags,
            uint receiptOverlayPixelsWritten,
            ushort receiptOverlayStageCount)
        {
            return receiptAbiVersion == AbiVersion &&
                   receiptStructSize == OwnerHudInfoSize &&
                   receiptGeneration == expectedGeneration &&
                   receiptCameraTick == expectedCameraTick &&
                   receiptRevision == expectedRevision &&
                   receiptWidth == OwnerHudWidth &&
                   receiptHeight == OwnerHudHeight &&
                   receiptStride == OwnerHudStride &&
                   receiptIndexBytes == OwnerHudWidth * OwnerHudHeight &&
                   receiptProvenanceBytes == OwnerHudWidth * OwnerHudHeight &&
                   (receiptFlags & RequiredOwnerHudFlags) == RequiredOwnerHudFlags &&
                   receiptOverlayPixelsWritten > 0 &&
                   receiptOverlayStageCount > 0;
        }
        public byte MissionStage { get { return mission.currentStage; } }
        public byte TasksCompleted { get { return mission.tasksCompleted; } }
        public bool PlayerDead { get { return mission.playerDead != 0; } }
        public bool ExitSequenceActive { get { return mission.exitSequenceActive != 0; } }
        public bool ExitSequenceComplete { get { return mission.exitSequenceComplete != 0; } }
        public byte ExitResultCode { get { return mission.exitResultCode; } }
        public int PlayerLife { get { return mission.playerLife; } }
        public int PlayerMaxLife { get { return mission.playerMaxLife; } }
        public int PlayerMana { get { return mission.playerMana; } }
        public short TargetSpeed { get { return snapshot.targetSpeed; } }
        public short ActualSpeed { get { return snapshot.actualSpeed; } }
        public short StrafeSpeed { get { return snapshot.strafeSpeed; } }
        public Vector3 CameraPosition { get { return BuildCameraPose().Position; } }
        public int ActiveProjectileCount
        {
            get
            {
                int count = 0;
                for (int index = 0; index < projectiles.Length; index++)
                {
                    if (projectiles[index].active != 0) count++;
                }

                return count;
            }
        }
        public int ActiveImpactCount
        {
            get
            {
                int count = 0;
                for (int index = 0; index < impacts.Length; index++)
                {
                    if (impacts[index].active != 0) count++;
                }

                return count;
            }
        }
        public int EntityCount { get { return entities == null ? 0 : entities.Length; } }
        public bool HasPendingResetSnapshot { get { return pendingResetSnapshot != null; } }

        /// <summary>
        /// Converts the owner terrain height plane into Unity world units.
        /// The terrain plane stores one byte per 32 source-height units;
        /// camera and entity Z values are already in source-height units.
        /// </summary>
        public static float SourceHeightToUnity(byte sourceHeight)
        {
            return SourceVerticalToUnity(sourceHeight * 32f);
        }

        public static float SourceVerticalToUnity(float sourceZ)
        {
            return sourceZ / SourceUnitsPerUnityUnit * EnhancedVerticalPresentationScale;
        }

        public int ActiveEntityCount
        {
            get
            {
                int count = 0;
                for (int index = 0; index < entities.Length; index++)
                {
                    if (entities[index].active != 0) count++;
                }

                for (int index = 0; index < projectiles.Length; index++)
                {
                    if (projectiles[index].active != 0) count++;
                }

                for (int index = 0; index < impacts.Length; index++)
                {
                    if (impacts[index].active != 0) count++;
                }

                return count;
            }
        }

        public bool Configure(
            string packagePath,
            string closurePath,
            string assetCachePath,
            out string reason)
        {
            reason = string.Empty;
            Dispose();

            if (string.IsNullOrWhiteSpace(packagePath) || !Directory.Exists(packagePath))
            {
                reason = "Unity Enhanced owner package directory does not exist";
                return false;
            }

            if (string.IsNullOrWhiteSpace(closurePath) || !File.Exists(closurePath))
            {
                reason = "Unity Enhanced owner closure file does not exist";
                return false;
            }

            if (string.IsNullOrWhiteSpace(assetCachePath) || !Directory.Exists(assetCachePath))
            {
                reason = "Unity Enhanced owner asset-cache directory does not exist";
                return false;
            }

            string palettePath = Path.Combine(assetCachePath, OwnerPaletteRelativePath);
            if (!File.Exists(palettePath))
            {
                reason = "Unity Enhanced owner palette is missing: " + palettePath;
                return false;
            }

            try
            {
                ownerPaletteRgb8 = File.ReadAllBytes(palettePath);
                if (ownerPaletteRgb8.Length != OwnerPaletteBytes)
                {
                    reason = "Unity Enhanced owner palette must contain exactly 768 RGB8 bytes: " +
                             palettePath;
                    Dispose();
                    return false;
                }

                ownerHudIndexBytes = new byte[OwnerHudWidth * OwnerHudHeight];
                ownerHudProvenanceBytes = new byte[OwnerHudWidth * OwnerHudHeight];
                ownerHudIndexHandle = GCHandle.Alloc(ownerHudIndexBytes, GCHandleType.Pinned);
                ownerHudIndexPinned = true;
                ownerHudProvenanceHandle = GCHandle.Alloc(ownerHudProvenanceBytes, GCHandleType.Pinned);
                ownerHudProvenancePinned = true;
                ownerHudPresentationRevision = 1;
                packageBytes = Encoding.UTF8.GetBytes(packagePath);
                closureBytes = Encoding.UTF8.GetBytes(closurePath);
                priorAssetCacheEnvironment = Environment.GetEnvironmentVariable(AssetCacheEnvironment);
                Environment.SetEnvironmentVariable(AssetCacheEnvironment, assetCachePath);
                ownsAssetCacheEnvironment = true;

                int status = mc_native_session_open(
                    packageBytes,
                    checked((uint)packageBytes.Length),
                    closureBytes,
                    checked((uint)closureBytes.Length),
                    out handle);
                if (status != 0)
                {
                    reason = StatusReason("Unity Enhanced owner session open", status);
                    Dispose();
                    return false;
                }

                if (!Reset(out reason))
                {
                    Dispose();
                    return false;
                }

                IsConfigured = true;
                return true;
            }
            catch (DllNotFoundException exception)
            {
                reason = "Unity Enhanced owner native plugin is unavailable: " + exception.Message;
            }
            catch (EntryPointNotFoundException exception)
            {
                reason = "Unity Enhanced owner native ABI is missing: " + exception.Message;
            }
            catch (BadImageFormatException exception)
            {
                reason = "Unity Enhanced owner native plugin has the wrong architecture: " + exception.Message;
            }
            catch (Exception exception)
            {
                reason = "Unity Enhanced owner source setup failed: " + exception.Message;
            }

            Dispose();
            return false;
        }

        public bool Reset(out string reason)
        {
            reason = string.Empty;
            if (handle == 0)
            {
                reason = "Unity Enhanced owner source is not configured";
                return false;
            }

            int status = mc_native_session_reset(handle, out snapshot, SessionSnapshotSize);
            if (status != 0)
            {
                reason = StatusReason("Unity Enhanced owner session reset", status);
                return false;
            }

            if (!ValidateSessionSnapshot(out reason))
            {
                return false;
            }

            terrainTypes = null;
            terrainHeights = null;
            terrainShade = null;
            terrainAngles = null;
            entities = Array.Empty<EntitySnapshot>();
            terrainMutationSequence = ulong.MaxValue;
            terrainChangedSinceLastObservation = true;
            mission = default(MissionSnapshot);
            projectiles = Array.Empty<ProjectileSnapshot>();
            impacts = Array.Empty<ImpactSnapshot>();
            pendingEvents = Array.Empty<EventSnapshot>();
            eventCursor = snapshot.eventHead;
            eventHead = snapshot.eventHead;
            lastEventCount = 0;
            lastVfxEventCount = 0;
            cameraGeneration = 0;
            hasCameraHistory = false;
            lastPublishedTerrainOrigin = Vector3.zero;
            hasPublishedTerrainOrigin = false;
            pendingResetSnapshot = null;
            nextVfxSequence = 0;
            LastTerrainSnapshotPublished = false;
            LastTerrainMutationPublished = false;
            LastTerrainOriginChanged = false;
            LastTerrainFullUpdate = false;
            if (!ReadObservations(true, out reason))
            {
                return false;
            }

            pendingResetSnapshot = BuildSnapshot(true);
            return true;
        }

        /// <summary>
        /// Takes the generation-bound tick-0 reset snapshot. The caller must
        /// apply it before asking this source to advance the Rust session.
        /// </summary>
        public bool TryTakeResetSnapshot(out UnityEnhancedWorldSnapshot resetSnapshot)
        {
            resetSnapshot = pendingResetSnapshot;
            pendingResetSnapshot = null;
            return resetSnapshot != null;
        }

        /// <summary>
        /// Advances the Rust session once and returns one complete immutable
        /// Enhanced snapshot. The input values are presentation-client input;
        /// they are encoded into the existing native command ABI unchanged.
        /// </summary>
        public bool Advance(
            float horizontalInput,
            float verticalInput,
            bool cast,
            int spellSelection,
            out UnityEnhancedWorldSnapshot enhancedSnapshot,
            out string reason)
        {
            return Advance(
                horizontalInput,
                verticalInput,
                0f,
                0f,
                cast,
                spellSelection,
                out enhancedSnapshot,
                out reason);
        }

        public bool Advance(
            float horizontalInput,
            float verticalInput,
            float lookYawInput,
            float lookPitchInput,
            bool cast,
            int spellSelection,
            out UnityEnhancedWorldSnapshot enhancedSnapshot,
            out string reason)
        {
            enhancedSnapshot = null;
            reason = string.Empty;
            if (!IsConfigured || handle == 0)
            {
                reason = "Unity Enhanced owner source is not configured";
                return false;
            }

            if (pendingResetSnapshot != null)
            {
                reason = "Unity Enhanced reset snapshot must be applied before stepping the owner session";
                return false;
            }

            try
            {
                Command command = new Command
                {
                    abiVersion = AbiVersion,
                    structSize = 32,
                    roll = ClampInt8(horizontalInput * 127f),
                    pitch = ClampInt8(verticalInput * 127f),
                    flags = 1,
                    requestedSpell = -1,
                    reserved = new uint[4]
                };
                if (spellSelection >= 0)
                {
                    command.opcode = SpellSelectionOpcode;
                    command.argument1 = ClampInt8(spellSelection);
                }
                if (cast)
                {
                    command.flags = unchecked((sbyte)(command.flags | SelectedSpellFlag));
                }

                CameraInput camera = new CameraInput
                {
                    abiVersion = AbiVersion,
                    structSize = 28,
                    lookYaw = CameraInputValue(lookYawInput),
                    lookPitch = CameraInputValue(lookPitchInput),
                    reserved = new uint[4]
                };
                int status = mc_native_session_step_with_camera(
                    handle,
                    ref command,
                    ref camera,
                    out snapshot,
                    SessionSnapshotSize);
                if (status != 0)
                {
                    reason = StatusReason("Unity Enhanced owner session step", status);
                    return false;
                }

                if (!ValidateSessionSnapshot(out reason))
                {
                    return false;
                }

                if (!ReadObservations(false, out reason))
                {
                    return false;
                }

                enhancedSnapshot = BuildSnapshot(false);
                return true;
            }
            catch (Exception exception)
            {
                reason = "Unity Enhanced owner snapshot failed: " + exception.Message;
                return false;
            }
        }

        /// <summary>
        /// Advances the same owner-bound native session from one sealed
        /// external PlaythroughRecorder command. This preserves opcode,
        /// arguments, flags, and requested-spell fields for cross-mode replay;
        /// Unity still only receives the resulting immutable snapshot.
        /// </summary>
        public bool AdvanceOwnerCommand(
            sbyte opcode,
            sbyte argument1,
            sbyte argument2,
            sbyte roll,
            sbyte pitch,
            sbyte flags,
            short requestedSpell,
            out UnityEnhancedWorldSnapshot enhancedSnapshot,
            out string reason)
        {
            enhancedSnapshot = null;
            reason = string.Empty;
            if (!IsConfigured || handle == 0)
            {
                reason = "Unity Enhanced owner source is not configured";
                return false;
            }

            if (pendingResetSnapshot != null)
            {
                reason = "Unity Enhanced reset snapshot must be applied before stepping the owner session";
                return false;
            }

            try
            {
                Command command = new Command
                {
                    abiVersion = AbiVersion,
                    structSize = 32,
                    opcode = opcode,
                    argument1 = argument1,
                    argument2 = argument2,
                    roll = roll,
                    pitch = pitch,
                    flags = flags,
                    requestedSpell = requestedSpell,
                    reserved = new uint[4]
                };
                CameraInput camera = new CameraInput
                {
                    abiVersion = AbiVersion,
                    structSize = 28,
                    reserved = new uint[4]
                };
                int status = mc_native_session_step_with_camera(
                    handle,
                    ref command,
                    ref camera,
                    out snapshot,
                    SessionSnapshotSize);
                if (status != 0)
                {
                    reason = StatusReason("Unity Enhanced owner raw command step", status);
                    return false;
                }

                if (!ValidateSessionSnapshot(out reason) ||
                    !ReadObservations(false, out reason))
                {
                    return false;
                }

                enhancedSnapshot = BuildSnapshot(false);
                return true;
            }
            catch (Exception exception)
            {
                reason = "Unity Enhanced owner raw command snapshot failed: " + exception.Message;
                return false;
            }
        }

        public void Dispose()
        {
            IsConfigured = false;
            if (handle != 0)
            {
                try
                {
                    mc_native_session_destroy(handle);
                }
                catch (Exception)
                {
                    // Teardown must not prevent managed and environment cleanup.
                }

                handle = 0;
            }

            if (ownsAssetCacheEnvironment)
            {
                Environment.SetEnvironmentVariable(AssetCacheEnvironment, priorAssetCacheEnvironment);
                priorAssetCacheEnvironment = null;
                ownsAssetCacheEnvironment = false;
            }

            packageBytes = null;
            closureBytes = null;
            terrainTypes = null;
            terrainHeights = null;
            terrainShade = null;
            terrainAngles = null;
            if (ownerHudProvenancePinned)
            {
                ownerHudProvenanceHandle.Free();
                ownerHudProvenancePinned = false;
            }

            if (ownerHudIndexPinned)
            {
                ownerHudIndexHandle.Free();
                ownerHudIndexPinned = false;
            }

            ownerHudIndexBytes = null;
            ownerHudProvenanceBytes = null;
            ownerPaletteRgb8 = null;
            ownerHudFrameQuery = default(McPresentationFrameQueryV2);
            ownerHudFrameInfo = default(McPresentationFrameInfoV2);
            ownerHudPresentationRevision = 0;
            entities = Array.Empty<EntitySnapshot>();
            mission = default(MissionSnapshot);
            projectiles = Array.Empty<ProjectileSnapshot>();
            impacts = Array.Empty<ImpactSnapshot>();
            pendingEvents = Array.Empty<EventSnapshot>();
            snapshot = default(SessionSnapshot);
            presentation = default(PresentationState);
            renderCamera = default(RenderCameraState);
            terrainMutationSequence = 0;
            terrainChangedSinceLastObservation = false;
            eventCursor = 0;
            eventHead = 0;
            lastEventCount = 0;
            lastVfxEventCount = 0;
            previousCameraX = 0;
            previousCameraY = 0;
            unwrappedCameraX = 0;
            unwrappedCameraY = 0;
            cameraGeneration = 0;
            hasCameraHistory = false;
            lastPublishedTerrainOrigin = Vector3.zero;
            hasPublishedTerrainOrigin = false;
            pendingResetSnapshot = null;
            nextVfxSequence = 0;
            LastTerrainSnapshotPublished = false;
            LastTerrainMutationPublished = false;
            LastTerrainOriginChanged = false;
            LastTerrainFullUpdate = false;
        }

        /// <summary>
        /// Arms the owner session's existing action-51 production path with a
        /// pinned house/parent vector for terrain-mutation qualification. The
        /// next owner step performs the production update; this method only
        /// configures the external owner session and never changes Unity state.
        /// </summary>
        public bool PrepareTerrainMutationQualification(
            ushort houseEntityIndex,
            ushort parentEntityIndex,
            byte constructionSelector,
            out string reason)
        {
            reason = string.Empty;
            if (!IsConfigured || handle == 0)
            {
                reason = "Unity Enhanced owner source is not configured";
                return false;
            }

            try
            {
                int status = mc_native_level1_action_51_completion_setup_for_qualification(
                    handle,
                    houseEntityIndex,
                    parentEntityIndex,
                    constructionSelector);
                if (status != 0)
                {
                    reason = StatusReason("Unity Enhanced terrain mutation qualification setup", status);
                    return false;
                }

                return true;
            }
            catch (Exception exception)
            {
                reason = "Unity Enhanced terrain mutation qualification setup failed: " +
                         exception.Message;
                return false;
            }
        }

        /// <summary>
        /// Injects the existing private owner qualification damage boundary.
        /// The damage is consumed by Rust on the next owner step; Unity never
        /// writes mission state or decides whether the player is dead.
        /// </summary>
        public bool InjectPlayerDamageForQualification(
            int damage,
            ushort attackerEntityIndex,
            out string reason)
        {
            reason = string.Empty;
            if (!IsConfigured || handle == 0)
            {
                reason = "Unity Enhanced owner source is not configured";
                return false;
            }

            if (damage <= 0 || attackerEntityIndex == 0)
            {
                reason = "Unity Enhanced qualification damage requires a positive amount and nonzero attacker";
                return false;
            }

            try
            {
                int status = mc_native_level1_player_inject_damage_for_qualification(
                    handle,
                    damage,
                    attackerEntityIndex);
                if (status != 0)
                {
                    reason = StatusReason("Unity Enhanced player damage qualification", status);
                    return false;
                }

                return true;
            }
            catch (Exception exception)
            {
                reason = "Unity Enhanced player damage qualification failed: " +
                         exception.Message;
                return false;
            }
        }

        private bool ReadObservations(bool forceTerrain, out string reason)
        {
            reason = string.Empty;
            int status = mc_native_presentation_state(handle, out presentation, PresentationStateSize);
            if (status != 0)
            {
                reason = StatusReason("Unity Enhanced owner presentation state", status);
                return false;
            }

            if (presentation.abiVersion != AbiVersion ||
                presentation.structSize != PresentationStateSize ||
                presentation.generation != snapshot.generation ||
                (snapshot.tick == 0
                    ? presentation.tick != 0
                    : presentation.tick + 1 != snapshot.tick) ||
                presentation.cameraAuthority != CameraAuthorityPlayerFlightLive ||
                (presentation.cameraFlags & (PresentationCameraExactInputs | PresentationCameraEyeZPlus128)) !=
                    (PresentationCameraExactInputs | PresentationCameraEyeZPlus128) ||
                !AllZero(presentation.reserved))
            {
                reason = "Unity Enhanced owner presentation state failed its authority/layout contract";
                return false;
            }

            status = mc_native_render_camera_state(handle, out renderCamera, RenderCameraStateSize);
            if (status != 0)
            {
                reason = StatusReason("Unity Enhanced owner render camera", status);
                return false;
            }

            if (renderCamera.abiVersion != AbiVersion ||
                renderCamera.structSize != RenderCameraStateSize ||
                renderCamera.generation != presentation.generation ||
                renderCamera.tick != presentation.tick ||
                renderCamera.sourceTurn != presentation.sourceTurn ||
                renderCamera.historyX != presentation.cameraX ||
                renderCamera.historyY != presentation.cameraY ||
                renderCamera.eyeZ != presentation.cameraZ ||
                renderCamera.yaw != (presentation.cameraYaw & 0x07ff) ||
                renderCamera.pitch != presentation.cameraPitch ||
                renderCamera.roll != presentation.cameraRoll ||
                renderCamera.fov != presentation.cameraFov ||
                renderCamera.authority != CameraAuthorityPlayerFlightLive ||
                (renderCamera.flags & RenderCameraTerrainSlopeXY) == 0 ||
                !AllZero(renderCamera.reserved))
            {
                reason = "Unity Enhanced owner render camera failed its history/layout contract";
                return false;
            }

            status = mc_native_session_terrain_state(handle, out TerrainState terrainState, TerrainStateSize);
            if (status != 0)
            {
                reason = StatusReason("Unity Enhanced owner terrain state", status);
                return false;
            }

            if (terrainState.width != TerrainWidth || terrainState.height != TerrainHeight ||
                terrainState.planeCount != 6 || terrainState.hashSchema != TerrainHashSchema ||
                terrainState.authority != SessionTerrainLive ||
                terrainState.generation != snapshot.generation ||
                terrainState.tick != snapshot.tick || !AllZero(terrainState.reserved))
            {
                reason = "Unity Enhanced owner terrain state does not match the session snapshot";
                return false;
            }

            LastTerrainHash = terrainState.terrainHash;
            bool terrainChanged = forceTerrain || terrainTypes == null ||
                                  terrainState.mutationSequence != terrainMutationSequence;
            terrainChangedSinceLastObservation = terrainChanged;
            if (terrainChanged)
            {
                terrainTypes = ReadPlane(TerrainTypes, out reason);
                if (terrainTypes == null) return false;
                terrainHeights = ReadPlane(TerrainHeightPlane, out reason);
                if (terrainHeights == null) return false;
                terrainShade = ReadPlane(TerrainShade, out reason);
                if (terrainShade == null) return false;
                terrainAngles = ReadPlane(TerrainAngle, out reason);
                if (terrainAngles == null) return false;
                terrainMutationSequence = terrainState.mutationSequence;
            }

            status = mc_native_entity_count(handle, out uint entityCount);
            if (status != 0)
            {
                reason = StatusReason("Unity Enhanced owner entity count", status);
                return false;
            }

            EntitySnapshot[] nextEntities = new EntitySnapshot[checked((int)entityCount)];
            status = mc_native_entities_copy(
                handle,
                0,
                nextEntities,
                entityCount,
                EntityElementSize,
                out uint written);
            if (status != 0)
            {
                reason = StatusReason("Unity Enhanced owner entities", status);
                return false;
            }

            if (written != entityCount)
            {
                reason = "Unity Enhanced owner entity batch is incomplete";
                return false;
            }

            entities = nextEntities;
            for (int index = 0; index < entities.Length; index++)
            {
                EntitySnapshot entity = entities[index];
                if (entity.abiVersion != AbiVersion ||
                    entity.structSize != EntityElementSize ||
                    !AllZero(entity.reserved))
                {
                    reason = "Unity Enhanced owner entity batch failed its ABI/layout contract";
                    return false;
                }
            }

            status = mc_native_session_terrain_state(handle, out TerrainState finalTerrainState, TerrainStateSize);
            if (status != 0)
            {
                reason = StatusReason("Unity Enhanced owner final terrain state", status);
                return false;
            }

            if (finalTerrainState.generation != terrainState.generation ||
                finalTerrainState.tick != terrainState.tick ||
                finalTerrainState.mutationSequence != terrainState.mutationSequence ||
                finalTerrainState.terrainHash != terrainState.terrainHash ||
                finalTerrainState.width != TerrainWidth ||
                finalTerrainState.height != TerrainHeight ||
                finalTerrainState.planeCount != 6 ||
                finalTerrainState.hashSchema != TerrainHashSchema ||
                finalTerrainState.authority != SessionTerrainLive ||
                !AllZero(finalTerrainState.reserved))
            {
                reason = "Unity Enhanced owner terrain changed while observations were being copied";
                return false;
            }

            if (!ReadMissionObservations(out reason))
            {
                return false;
            }

            if (!ReadMinimapObservations(out reason))
            {
                return false;
            }

            if (!ReadEvents(out reason))
            {
                return false;
            }

            LastWorldHash = HashEntities(entities, projectiles, impacts);
            if (!ReadOwnerHudFrame(out reason))
            {
                return false;
            }

            return UpdateCameraHistory();
        }

        private bool ReadOwnerHudFrame(out string reason)
        {
            reason = string.Empty;
            if (!ownerHudIndexPinned || !ownerHudProvenancePinned ||
                ownerHudIndexBytes == null || ownerHudProvenanceBytes == null ||
                ownerPaletteRgb8 == null)
            {
                reason = "Unity Enhanced owner HUD buffers are not configured";
                return false;
            }

            if (CanReuseOwnerHudFrame(
                    ownerHudFrameInfo.abiVersion,
                    ownerHudFrameInfo.structSize,
                    ownerHudFrameInfo.generation,
                    ownerHudFrameInfo.tick,
                    snapshot.generation,
                    presentation.tick,
                    ownerHudFrameInfo.presentationRevision,
                    ownerHudPresentationRevision,
                    ownerHudFrameInfo.width,
                    ownerHudFrameInfo.height,
                    ownerHudFrameInfo.stride,
                    ownerHudFrameInfo.indexBytes,
                    ownerHudFrameInfo.provenanceBytes,
                    ownerHudFrameInfo.flags,
                    ownerHudFrameInfo.overlayPixelsWritten,
                    ownerHudFrameInfo.overlayStageCount))
            {
                return true;
            }

            ownerHudFrameQuery = new McPresentationFrameQueryV2
            {
                abiVersion = AbiVersion,
                structSize = OwnerHudQuerySize,
                generation = snapshot.generation,
                // The compositor-complete presentation graph is keyed to the
                // render-camera clock. Enhanced carries that source frame with
                // the same owner generation and camera history that produced
                // the camera pose.
                tick = presentation.tick,
                presentationRevision = ownerHudPresentationRevision,
                viewportWidth = OwnerHudWidth,
                viewportHeight = OwnerHudHeight,
                screenStride = OwnerHudStride,
                reserved0 = 0,
                configuredBlurMode = 0,
                blurCapability = 1,
                highSpeedSystem = 1,
                scratchPresent = 1,
                vgaResolution = 1,
                spatialSmoothing = 0,
                minimapEnabled = 1,
                topBarEnabled = 1,
                menuOverlay = 0,
                gamePaused = 0,
                levelEnd = (byte)(mission.exitSequenceActive != 0 ||
                                  mission.exitSequenceComplete != 0 ? 1 : 0),
                objectiveSelector = mission.textSelector,
                objectiveTextbox = (byte)(mission.objectiveTextboxTicks > 0 ? 1 : 0),
                tutorialOrBriefing = 0,
                helpPopup = 0,
                debugText = 0,
                reserved1 = 0,
                reserved2 = 0
            };

            int status = mc_native_session_presentation_frame_copy(
                handle,
                ref ownerHudFrameQuery,
                ownerHudIndexHandle.AddrOfPinnedObject(),
                checked((uint)ownerHudIndexBytes.Length),
                ownerHudProvenanceHandle.AddrOfPinnedObject(),
                checked((uint)ownerHudProvenanceBytes.Length),
                out ownerHudFrameInfo,
                OwnerHudInfoSize);
            if (status != 0)
            {
                reason = StatusReason("Unity Enhanced owner HUD frame copy", status);
                return false;
            }

            if (ownerHudFrameInfo.abiVersion != AbiVersion ||
                ownerHudFrameInfo.structSize != OwnerHudInfoSize ||
                ownerHudFrameInfo.generation != snapshot.generation ||
                ownerHudFrameInfo.tick != presentation.tick ||
                ownerHudFrameInfo.presentationRevision != ownerHudPresentationRevision ||
                ownerHudFrameInfo.width != OwnerHudWidth ||
                ownerHudFrameInfo.height != OwnerHudHeight ||
                ownerHudFrameInfo.stride != OwnerHudStride ||
                ownerHudFrameInfo.viewportWidth != OwnerHudWidth ||
                ownerHudFrameInfo.indexBytes != ownerHudIndexBytes.Length ||
                ownerHudFrameInfo.provenanceBytes != ownerHudProvenanceBytes.Length ||
                (ownerHudFrameInfo.flags & RequiredOwnerHudFlags) != RequiredOwnerHudFlags ||
                ownerHudFrameInfo.overlayPixelsWritten == 0 ||
                ownerHudFrameInfo.overlayStageCount == 0)
            {
                reason = "Unity Enhanced owner HUD frame failed its compositor-complete receipt contract";
                return false;
            }

            return true;
        }

        private bool ReadMissionObservations(out string reason)
        {
            reason = string.Empty;
            int status = mc_native_level1_mission_snapshot(
                handle,
                out mission,
                MissionSnapshotSize);
            if (status != 0)
            {
                reason = StatusReason("Unity Enhanced owner Level 1 mission snapshot", status);
                return false;
            }

            if (mission.abiVersion != AbiVersion ||
                mission.structSize != MissionSnapshotSize ||
                mission.generation != snapshot.generation ||
                mission.tick != snapshot.tick ||
                mission.authority != Level1MissionLive)
            {
                reason = "Unity Enhanced owner Level 1 mission snapshot failed its authority/layout contract";
                return false;
            }

            const uint MaxMissionRows = 4096;
            if (mission.projectileCount > MaxMissionRows || mission.impactCount > MaxMissionRows)
            {
                reason = "Unity Enhanced owner Level 1 mission snapshot exceeds the bounded effect-row budget";
                return false;
            }

            projectiles = ReadProjectiles(mission.projectileCount, out reason);
            if (projectiles == null)
            {
                return false;
            }

            impacts = ReadImpacts(mission.impactCount, out reason);
            return impacts != null;
        }

        private bool ReadMinimapObservations(out string reason)
        {
            reason = string.Empty;
            int status = mc_native_level1_minimap_marks(
                handle,
                out minimapMarks,
                MinimapMarksSize);
            if (status != 0)
            {
                reason = StatusReason("Unity Enhanced owner Level 1 minimap marks", status);
                return false;
            }

            if (minimapMarks.abiVersion != AbiVersion ||
                minimapMarks.structSize != MinimapMarksSize ||
                minimapMarks.generation != snapshot.generation ||
                minimapMarks.tick != snapshot.tick ||
                minimapMarks.markCount > MinimapMaxMarks ||
                minimapMarks.markX == null || minimapMarks.markX.Length != MinimapMaxMarks ||
                minimapMarks.markY == null || minimapMarks.markY.Length != MinimapMaxMarks ||
                minimapMarks.markColour == null || minimapMarks.markColour.Length != MinimapMaxMarks ||
                minimapMarks.markHelpSelector == null ||
                minimapMarks.markHelpSelector.Length != MinimapMaxMarks ||
                minimapMarks.arrowXY == null || minimapMarks.arrowXY.Length != 14 ||
                !AllZero(minimapMarks.reserved))
            {
                reason = "Unity Enhanced owner Level 1 minimap marks failed its ABI/layout contract";
                return false;
            }

            MinimapEntityCommand[] values = new MinimapEntityCommand[MinimapMaxEntityCommands];
            status = mc_native_level1_minimap_entities_copy(
                handle,
                snapshot.generation,
                snapshot.tick,
                values,
                (uint)values.Length,
                MinimapEntityElementSize,
                out minimapEntitiesInfo,
                MinimapEntitiesInfoSize);
            if (status != 0)
            {
                reason = StatusReason("Unity Enhanced owner Level 1 minimap entity commands", status);
                return false;
            }

            if (minimapEntitiesInfo.abiVersion != AbiVersion ||
                minimapEntitiesInfo.structSize != MinimapEntitiesInfoSize ||
                minimapEntitiesInfo.generation != snapshot.generation ||
                minimapEntitiesInfo.tick != snapshot.tick ||
                minimapEntitiesInfo.totalCount > (uint)MinimapMaxEntityCommands ||
                minimapEntitiesInfo.authority != Level1MissionLive ||
                !AllZero(minimapEntitiesInfo.reserved0) ||
                !AllZero(minimapEntitiesInfo.reserved))
            {
                reason = "Unity Enhanced owner Level 1 minimap entity header failed its ABI/layout contract";
                return false;
            }

            int commandCount = checked((int)minimapEntitiesInfo.totalCount);
            MinimapEntityCommand[] observed = new MinimapEntityCommand[commandCount];
            Array.Copy(values, observed, commandCount);
            for (int index = 0; index < observed.Length; index++)
            {
                MinimapEntityCommand command = observed[index];
                if (command.sourceSlot == 0 ||
                    command.kind < UnityEnhancedMinimapSnapshot.DotCommand ||
                    command.kind > UnityEnhancedMinimapSnapshot.BlendPixelCommand ||
                    command.order != index ||
                    !AllZero(command.reserved))
                {
                    reason = "Unity Enhanced owner Level 1 minimap entity command failed its source-order contract";
                    return false;
                }
            }

            minimapCommands = observed;
            return true;
        }

        private ProjectileSnapshot[] ReadProjectiles(uint count, out string reason)
        {
            reason = string.Empty;
            ProjectileSnapshot[] values = new ProjectileSnapshot[checked((int)count)];
            int status = mc_native_fireball_projectiles_copy(
                handle,
                mission.generation,
                mission.tick,
                0,
                values,
                count,
                ProjectileElementSize,
                out uint written);
            if (status != 0)
            {
                reason = StatusReason("Unity Enhanced owner Level 1 projectile snapshot", status);
                return null;
            }

            if (written != count)
            {
                reason = "Unity Enhanced owner Level 1 projectile snapshot is incomplete";
                return null;
            }

            for (int index = 0; index < values.Length; index++)
            {
                ProjectileSnapshot value = values[index];
                if (value.abiVersion != AbiVersion ||
                    value.structSize != ProjectileElementSize ||
                    value.authority != Level1MissionLive ||
                    value.active > 1 ||
                    value.reserved == null ||
                    value.reserved.Length != 2 ||
                    value.reserved[1] != 0)
                {
                    reason = "Unity Enhanced owner Level 1 projectile snapshot failed its ABI/layout contract";
                    return null;
                }
            }

            return values;
        }

        private ImpactSnapshot[] ReadImpacts(uint count, out string reason)
        {
            reason = string.Empty;
            ImpactSnapshot[] values = new ImpactSnapshot[checked((int)count)];
            int status = mc_native_fireball_impacts_copy(
                handle,
                mission.generation,
                mission.tick,
                0,
                values,
                count,
                ImpactElementSize,
                out uint written);
            if (status != 0)
            {
                reason = StatusReason("Unity Enhanced owner Level 1 impact snapshot", status);
                return null;
            }

            if (written != count)
            {
                reason = "Unity Enhanced owner Level 1 impact snapshot is incomplete";
                return null;
            }

            for (int index = 0; index < values.Length; index++)
            {
                ImpactSnapshot value = values[index];
                if (value.abiVersion != AbiVersion ||
                    value.structSize != ImpactElementSize ||
                    value.authority != Level1MissionLive ||
                    value.active > 1 ||
                    value.damageSubmitted > 1 ||
                    value.reserved0 != 0 ||
                    value.reserved1 != 0 ||
                    value.reserved == null ||
                    value.reserved.Length != 2 ||
                    value.reserved[1] != 0)
                {
                    reason = "Unity Enhanced owner Level 1 impact snapshot failed its ABI/layout contract";
                    return null;
                }
            }

            return values;
        }

        private bool ReadEvents(out string reason)
        {
            reason = string.Empty;
            var observed = new List<EventSnapshot>();
            ulong cursor = eventCursor;
            while (true)
            {
                EventQuery query = new EventQuery
                {
                    abiVersion = AbiVersion,
                    structSize = EventQuerySize,
                    generation = snapshot.generation,
                    cursor = cursor,
                    reserved = new uint[4]
                };
                EventSnapshot[] batchValues = new EventSnapshot[EventBatchCapacity];
                int status = mc_native_events_copy_since(
                    handle,
                    ref query,
                    batchValues,
                    EventBatchCapacity,
                    EventElementSize,
                    out EventBatch batch,
                    EventBatchSize);
                if (status != 0)
                {
                    reason = StatusReason("Unity Enhanced owner event journal", status);
                    return false;
                }

                if (batch.abiVersion != AbiVersion ||
                    batch.structSize != EventBatchSize ||
                    batch.generation != snapshot.generation ||
                    batch.written > (uint)EventBatchCapacity ||
                    batch.hasMore > 1 ||
                    batch.nextCursor < cursor ||
                    batch.eventHead < batch.nextCursor ||
                    !AllZero(batch.reserved0) ||
                    !AllZero(batch.reserved))
                {
                    reason = "Unity Enhanced owner event journal failed its ABI/layout contract";
                    return false;
                }

                ulong previousSequence = cursor;
                for (uint index = 0; index < batch.written; index++)
                {
                    EventSnapshot value = batchValues[checked((int)index)];
                    if (value.abiVersion != AbiVersion ||
                        value.structSize != EventElementSize ||
                        value.generation != snapshot.generation ||
                        value.sequence <= previousSequence ||
                        !AllZero(value.reserved))
                    {
                        reason = "Unity Enhanced owner event row failed its ABI/layout contract";
                        return false;
                    }

                    observed.Add(value);
                    previousSequence = value.sequence;
                }

                cursor = batch.nextCursor;
                eventHead = batch.eventHead;
                if (batch.hasMore == 0)
                {
                    break;
                }

                if (batch.written == 0)
                {
                    reason = "Unity Enhanced owner event journal made no progress while more rows were available";
                    return false;
                }
            }

            eventCursor = cursor;
            pendingEvents = observed.ToArray();
            lastEventCount = pendingEvents.Length;
            return true;
        }

        private byte[] ReadPlane(uint kind, out string reason)
        {
            reason = string.Empty;
            int status = mc_native_terrain_plane_length(handle, kind, out uint count);
            if (status != 0)
            {
                reason = StatusReason("Unity Enhanced owner terrain plane length", status);
                return null;
            }

            if (count != TerrainCellCount)
            {
                reason = "Unity Enhanced owner terrain plane has an unexpected extent";
                return null;
            }

            byte[] values = new byte[checked((int)count)];
            status = mc_native_terrain_plane_copy_u8(handle, kind, values, count, out uint written);
            if (status != 0)
            {
                reason = StatusReason("Unity Enhanced owner terrain plane copy", status);
                return null;
            }

            if (written != count)
            {
                reason = "Unity Enhanced owner terrain plane copy is incomplete";
                return null;
            }

            return values;
        }

        private bool UpdateCameraHistory()
        {
            if (!hasCameraHistory || cameraGeneration != snapshot.generation)
            {
                unwrappedCameraX = presentation.cameraX;
                unwrappedCameraY = presentation.cameraY;
                cameraGeneration = snapshot.generation;
                hasCameraHistory = true;
            }
            else
            {
                unwrappedCameraX += ModularDelta(presentation.cameraX, previousCameraX);
                unwrappedCameraY += ModularDelta(presentation.cameraY, previousCameraY);
            }

            previousCameraX = presentation.cameraX;
            previousCameraY = presentation.cameraY;
            return true;
        }

        private UnityEnhancedWorldSnapshot BuildSnapshot(bool reset)
        {
            Vector3 terrainOrigin = TerrainEpochOrigin();
            bool originChanged = !hasPublishedTerrainOrigin ||
                                 !Approximately(lastPublishedTerrainOrigin, terrainOrigin);
            bool publishTerrain = reset || terrainChangedSinceLastObservation || originChanged;
            LastTerrainSnapshotPublished = publishTerrain;
            LastTerrainMutationPublished = terrainChangedSinceLastObservation;
            LastTerrainOriginChanged = originChanged;
            LastTerrainFullUpdate = reset || terrainChangedSinceLastObservation || originChanged;
            var result = new UnityEnhancedWorldSnapshot
            {
                Generation = snapshot.generation,
                Tick = snapshot.tick,
                Reset = reset,
                CameraPose = BuildCameraPose(),
                Entities = BuildEntities(),
                Hud = BuildHudSnapshot(),
                Minimap = BuildMinimapSnapshot(),
                Terrain = publishTerrain
                    ? BuildTerrainSnapshot(reset, terrainOrigin, terrainChangedSinceLastObservation || originChanged)
                    : null,
                Water = publishTerrain ? BuildWaterSnapshot(reset, terrainOrigin) : null,
                NightLighting = UnityEnhancedNightLightingDescriptor.Level1Night(),
                LocalLights = BuildLocalLights(),
                VfxEvents = BuildVfxEvents(),
                Quality = UnityEnhancedQualitySettings.ForTier(UnityEnhancedQualityTier.WholeHog)
            };
            if (publishTerrain)
            {
                lastPublishedTerrainOrigin = terrainOrigin;
                hasPublishedTerrainOrigin = true;
            }
            terrainChangedSinceLastObservation = false;
            return result;
        }

        private UnityEnhancedHudSnapshot BuildHudSnapshot()
        {
            return new UnityEnhancedHudSnapshot
            {
                Generation = mission.generation,
                Tick = mission.tick,
                OwnerFrame = new UnityEnhancedOwnerHudFrame
                {
                    Generation = snapshot.generation,
                    Tick = snapshot.tick,
                    Width = OwnerHudWidth,
                    Height = OwnerHudHeight,
                    Stride = OwnerHudStride,
                    IndexedPixels = (byte[])ownerHudIndexBytes.Clone(),
                    Provenance = (byte[])ownerHudProvenanceBytes.Clone(),
                    PaletteRgb8 = (byte[])ownerPaletteRgb8.Clone()
                },
                PlayerLife = mission.playerLife,
                PlayerMaxLife = mission.playerMaxLife,
                PlayerMana = mission.playerMana,
                PlayerMaxMana = mission.playerMaxMana,
                CastleMana = mission.castleMana,
                SecuredManaPercent = mission.securedManaPercent,
                TargetSpeed = snapshot.targetSpeed,
                ActualSpeed = snapshot.actualSpeed,
                CurrentStage = mission.currentStage,
                TasksCompleted = mission.tasksCompleted,
                ObjectiveSelector = mission.textSelector,
                ObjectiveTextboxTicks = mission.objectiveTextboxTicks,
                ObjectiveCount = snapshot.objectiveCount,
                LeftSpellSlot = snapshot.leftSpellSlot,
                RightSpellSlot = snapshot.rightSpellSlot,
                SelectedSpellUiIndex = snapshot.selectedSpellUiIndex,
                SpeedTrailCount = mission.speedTrailCount,
                HasFireball = mission.hasFireball != 0,
                HasPossession = mission.hasPossession != 0,
                HasCastleSpell = mission.hasCastleSpell != 0,
                HasSpeedSpell = mission.hasSpeedSpell != 0,
                PlayerDead = mission.playerDead != 0,
                ExitSequenceActive = mission.exitSequenceActive != 0,
                ExitSequenceComplete = mission.exitSequenceComplete != 0,
                ExitResultCode = mission.exitResultCode
            };
        }

        private UnityEnhancedMinimapSnapshot BuildMinimapSnapshot()
        {
            var commands = new UnityEnhancedMinimapEntityCommand[minimapCommands.Length];
            for (int index = 0; index < minimapCommands.Length; index++)
            {
                MinimapEntityCommand source = minimapCommands[index];
                commands[index] = new UnityEnhancedMinimapEntityCommand
                {
                    SourceSlot = source.sourceSlot,
                    X = source.x,
                    Y = source.y,
                    Selector = source.selector,
                    Kind = source.kind,
                    PaletteIndex = source.paletteIndex,
                    Size = source.size,
                    Anchor = source.anchor,
                    HelpSelector = source.helpSelector,
                    Flags = source.flags,
                    Order = source.order
                };
            }

            return new UnityEnhancedMinimapSnapshot
            {
                Generation = minimapMarks.generation,
                Tick = minimapMarks.tick,
                MarkCount = minimapMarks.markCount,
                MarkPulse = minimapMarks.markPulse != 0,
                ArrowValid = minimapMarks.arrowValid != 0,
                ArrowVisible = minimapMarks.arrowVisible != 0,
                MarkX = (short[])minimapMarks.markX.Clone(),
                MarkY = (short[])minimapMarks.markY.Clone(),
                MarkColour = (byte[])minimapMarks.markColour.Clone(),
                MarkHelpSelector = (byte[])minimapMarks.markHelpSelector.Clone(),
                ArrowXY = (short[])minimapMarks.arrowXY.Clone(),
                TotalEntityCount = minimapEntitiesInfo.totalCount,
                CrossX = minimapEntitiesInfo.crossX,
                CrossY = minimapEntitiesInfo.crossY,
                CrossColour = minimapEntitiesInfo.crossColour,
                CrossArmLength = minimapEntitiesInfo.crossArmLength,
                RevealActive = minimapEntitiesInfo.revealActive != 0,
                Authority = minimapEntitiesInfo.authority,
                CommandHash = minimapEntitiesInfo.commandHash,
                EntityCommands = commands
            };
        }

        private Vector3 TerrainEpochOrigin()
        {
            float mapSize = TerrainWidth * TerrainCellSize;
            float cameraX = (float)unwrappedCameraX / SourceUnitsPerUnityUnit;
            float cameraZ = (float)unwrappedCameraY / SourceUnitsPerUnityUnit;
            return new Vector3(
                Mathf.Floor(cameraX / mapSize) * mapSize,
                0f,
                Mathf.Floor(cameraZ / mapSize) * mapSize);
        }

        private static bool Approximately(Vector3 left, Vector3 right)
        {
            return Mathf.Abs(left.x - right.x) < 0.001f &&
                   Mathf.Abs(left.y - right.y) < 0.001f &&
                   Mathf.Abs(left.z - right.z) < 0.001f;
        }

        private UnityEnhancedCameraPose BuildCameraPose()
        {
            float x = (float)(unwrappedCameraX + renderCamera.renderOffsetX) / SourceUnitsPerUnityUnit;
            float y = SourceVerticalToUnity(presentation.cameraZ) + EnhancedCameraLift;
            float z = (float)(unwrappedCameraY + renderCamera.renderOffsetY) / SourceUnitsPerUnityUnit;
            float yaw = 180f - renderCamera.yaw * (360f / 2048f);
            float pitch = -SignedAngle11(presentation.cameraPitch) * (360f / 2048f) + EnhancedCameraPitchBias;
            float roll = -SignedAngle11(presentation.cameraRoll) * (360f / 2048f);
            float fov = Mathf.Clamp(VerticalFov(presentation.cameraFov), 30f, 120f);
            return new UnityEnhancedCameraPose(
                new Vector3(x, y, z),
                Quaternion.Euler(pitch, yaw, roll),
                fov);
        }

        private UnityEnhancedTerrainSnapshot BuildTerrainSnapshot(
            bool reset,
            Vector3 origin,
            bool fullUpdate)
        {
            float[] heights = new float[TerrainCellCount];
            for (int index = 0; index < heights.Length; index++)
            {
                heights[index] = SourceHeightToUnity(terrainHeights[index]);
            }

            return new UnityEnhancedTerrainSnapshot
            {
                Generation = snapshot.generation,
                Tick = snapshot.tick,
                Reset = reset,
                Width = TerrainWidth,
                Height = TerrainHeight,
                CellSize = TerrainCellSize,
                Origin = origin,
                Heights = heights,
                TerrainTypes = (byte[])terrainTypes.Clone(),
                Shade = (byte[])terrainShade.Clone(),
                Angles = (byte[])terrainAngles.Clone(),
                DirtyRegions = reset || fullUpdate
                    ? new[] { new UnityEnhancedTerrainDirtyRegion(0, 0, TerrainWidth, TerrainHeight) }
                    : Array.Empty<UnityEnhancedTerrainDirtyRegion>(),
                WrapMode = UnityEnhancedTerrainWrapMode.Repeat
            };
        }

        private UnityEnhancedWaterSnapshot BuildWaterSnapshot(bool reset, Vector3 terrainOrigin)
        {
            byte[] shoreline = new byte[TerrainCellCount];
            bool hasWater = false;
            for (int index = 0; index < shoreline.Length; index++)
            {
                if (terrainTypes[index] == 0)
                {
                    hasWater = true;
                    int x = index % TerrainWidth;
                    int y = index / TerrainWidth;
                    if (IsShorelineCell(terrainTypes, TerrainWidth, TerrainHeight, x, y))
                    {
                        shoreline[index] = 1;
                    }
                }
            }

            return new UnityEnhancedWaterSnapshot
            {
                Generation = snapshot.generation,
                Tick = snapshot.tick,
                Reset = reset,
                Level = 0f,
                Center = terrainOrigin + new Vector3(TerrainWidth * TerrainCellSize * 0.5f, 0f, TerrainHeight * TerrainCellSize * 0.5f),
                Size = new Vector2(TerrainWidth * TerrainCellSize, TerrainHeight * TerrainCellSize),
                ReflectionEnabled = hasWater,
                ShorelineMask = shoreline,
                ShorelineWidth = TerrainWidth,
                ShorelineHeight = TerrainHeight,
                ShorelineCellSize = TerrainCellSize
            };
        }

        /// <summary>
        /// Selects only water cells touching a non-water cell for the
        /// presentation shoreline pass. The authoritative terrain remains a
        /// full Rust-owned plane; a shoreline is a boundary accent, not a
        /// second textured copy of every water cell.
        /// </summary>
        public static bool IsShorelineCell(
            byte[] terrainTypes,
            int width,
            int height,
            int x,
            int y)
        {
            if (terrainTypes == null || width <= 0 || height <= 0 ||
                terrainTypes.Length != width * height ||
                x < 0 || x >= width || y < 0 || y >= height)
            {
                return false;
            }

            int index = y * width + x;
            if (terrainTypes[index] != 0)
            {
                return false;
            }

            int left = x == 0 ? width - 1 : x - 1;
            int right = x == width - 1 ? 0 : x + 1;
            int below = y == 0 ? height - 1 : y - 1;
            int above = y == height - 1 ? 0 : y + 1;
            return terrainTypes[y * width + left] != 0 ||
                   terrainTypes[y * width + right] != 0 ||
                   terrainTypes[below * width + x] != 0 ||
                   terrainTypes[above * width + x] != 0;
        }

        private UnityEnhancedEntitySnapshot[] BuildEntities()
        {
            var result = new List<UnityEnhancedEntitySnapshot>(
                entities.Length + projectiles.Length + impacts.Length);
            for (int index = 0; index < entities.Length; index++)
            {
                EntitySnapshot source = entities[index];
                int family = PresentationFamily(source, snapshot.entityIndex);
                if (family == UnsupportedPresentationFamily)
                {
                    continue;
                }

                ushort sourceX = source.x;
                ushort sourceY = source.y;
                float x = (float)(unwrappedCameraX + ModularDelta(sourceX, presentation.cameraX)) / SourceUnitsPerUnityUnit;
                float z = (float)(unwrappedCameraY + ModularDelta(sourceY, presentation.cameraY)) / SourceUnitsPerUnityUnit;
                float y = SourceVerticalToUnity(source.z);
                ulong stableIdentity = ((ulong)snapshot.generation << 32) | (uint)(source.entityIndex + 1);
                result.Add(new UnityEnhancedEntitySnapshot
                {
                    StableIdentity = stableIdentity,
                    Generation = snapshot.generation,
                    Position = new Vector3(x, y, z),
                    Rotation = Quaternion.Euler(0f, 180f - source.yaw * (360f / 2048f), 0f),
                    Visible = source.active != 0,
                    PresentationFamily = family,
                    FacingDegrees = 180f - source.yaw * (360f / 2048f),
                    AnimationPhase = source.phase / 255f,
                    Scale = family == (int)UnityEnhancedPresentationFamily.Player ? 1.25f : 1f,
                    RenderLayer = source.entityIndex
                });
            }

            for (int index = 0; index < projectiles.Length; index++)
            {
                ProjectileSnapshot source = projectiles[index];
                if (source.projectileId == 0)
                {
                    continue;
                }

                // Native projectile kind 1 is the authoritative fireball. Keep
                // the projectile entity live and moving with the Rust snapshot;
                // the family only selects its Enhanced visual treatment.
                int family = source.projectileKind == 1
                    ? (int)UnityEnhancedPresentationFamily.Fire
                    : source.projectileKind == 5
                        ? (int)UnityEnhancedPresentationFamily.Possession
                        : (int)UnityEnhancedPresentationFamily.Projectile;
                Vector3 position = ProjectedPosition(source.x, source.y, source.z);
                result.Add(new UnityEnhancedEntitySnapshot
                {
                    StableIdentity = DynamicStableIdentity(0x20000000U, source.projectileId),
                    Generation = snapshot.generation,
                    Position = position,
                    Rotation = Quaternion.Euler(
                        -source.pitch * (360f / 2048f),
                        180f - source.yaw * (360f / 2048f),
                        0f),
                    Visible = source.active != 0,
                    PresentationFamily = family,
                    FacingDegrees = 180f - source.yaw * (360f / 2048f),
                    AnimationPhase = Mathf.Clamp01(Mathf.Max(0, source.animationFrame) / 255f),
                    Scale = source.projectileKind == 2 ? 0.8f : 1f,
                    RenderLayer = 2000 + (int)(source.projectileId % 1000U)
                });
            }

            for (int index = 0; index < impacts.Length; index++)
            {
                ImpactSnapshot source = impacts[index];
                if (source.impactId == 0)
                {
                    continue;
                }

                int family = source.model == 6
                    ? (int)UnityEnhancedPresentationFamily.Fire
                    : source.model == 13
                        ? (int)UnityEnhancedPresentationFamily.Smoke
                        : (int)UnityEnhancedPresentationFamily.Impact;
                Vector3 position = ProjectedPosition(source.x, source.y, source.z);
                result.Add(new UnityEnhancedEntitySnapshot
                {
                    StableIdentity = DynamicStableIdentity(0x40000000U, source.impactId),
                    Generation = snapshot.generation,
                    Position = position,
                    Rotation = Quaternion.identity,
                    Visible = source.active != 0,
                    PresentationFamily = family,
                    FacingDegrees = 0f,
                    AnimationPhase = Mathf.Clamp01(Mathf.Max(0, source.animationFrame) / 255f),
                    Scale = source.model == 13 ? 1.15f : 1f,
                    RenderLayer = 3000 + (int)(source.impactId % 1000U)
                });
            }

            return result.ToArray();
        }

        private UnityEnhancedLocalLightRequest[] BuildLocalLights()
        {
            var result = new List<UnityEnhancedLocalLightRequest>();
            for (int index = 0; index < entities.Length && result.Count < 24; index++)
            {
                EntitySnapshot source = entities[index];
                int family = PresentationFamily(source, snapshot.entityIndex);
                if (family == UnsupportedPresentationFamily ||
                    (family != (int)UnityEnhancedPresentationFamily.Firefly &&
                     family != (int)UnityEnhancedPresentationFamily.Mana &&
                     family != (int)UnityEnhancedPresentationFamily.Projectile &&
                     family != (int)UnityEnhancedPresentationFamily.Impact))
                {
                    continue;
                }

                Vector3 position = EntityPosition(source);
                Color color = family == (int)UnityEnhancedPresentationFamily.Firefly
                    ? new Color(0.48f, 0.92f, 0.28f, 1f)
                    : new Color(0.22f, 0.64f, 1f, 1f);
                result.Add(new UnityEnhancedLocalLightRequest(
                    ((ulong)snapshot.generation << 32) | (uint)(source.entityIndex + 1),
                    position,
                    color,
                    family == (int)UnityEnhancedPresentationFamily.Impact ? 2.5f : 1.5f,
                    family == (int)UnityEnhancedPresentationFamily.Impact ? 8f : 5f,
                    source.entityIndex));
            }

            for (int index = 0; index < projectiles.Length && result.Count < 24; index++)
            {
                ProjectileSnapshot source = projectiles[index];
                if (source.active == 0 || source.projectileId == 0)
                {
                    continue;
                }

                bool fireball = source.projectileKind == 1;
                result.Add(new UnityEnhancedLocalLightRequest(
                    DynamicStableIdentity(0x20000000U, source.projectileId),
                    ProjectedPosition(source.x, source.y, source.z),
                    fireball ? new Color(1f, 0.2f, 0.04f, 1f) : new Color(1f, 0.32f, 0.08f, 1f),
                    fireball ? 5.5f : 1.8f,
                    fireball ? 11f : 6f,
                    (ushort)(2000 + (source.projectileId % 1000U))));
            }

            for (int index = 0; index < impacts.Length && result.Count < 24; index++)
            {
                ImpactSnapshot source = impacts[index];
                if (source.active == 0 || source.impactId == 0)
                {
                    continue;
                }

                result.Add(new UnityEnhancedLocalLightRequest(
                    DynamicStableIdentity(0x40000000U, source.impactId),
                    ProjectedPosition(source.x, source.y, source.z),
                    source.model == 6
                        ? new Color(1f, 0.18f, 0.03f, 1f)
                        : new Color(1f, 0.72f, 0.18f, 1f),
                    source.model == 6 ? 2.6f : 1.8f,
                    source.model == 6 ? 8f : 6f,
                    (ushort)(3000 + (source.impactId % 1000U))));
            }

            return result.ToArray();
        }

        private UnityEnhancedVfxEvent[] BuildVfxEvents()
        {
            if (pendingEvents.Length == 0)
            {
                lastVfxEventCount = 0;
                return Array.Empty<UnityEnhancedVfxEvent>();
            }

            var result = new List<UnityEnhancedVfxEvent>(pendingEvents.Length);
            for (int index = 0; index < pendingEvents.Length; index++)
            {
                EventSnapshot source = pendingEvents[index];
                UnityEnhancedVfxKind kind;
                Vector3 position;
                float intensity;
                uint duration;
                ulong stableIdentity;
                if (source.kind == EventRequestSpellActivation)
                {
                    kind = UnityEnhancedVfxKind.SpellCast;
                    position = EntityPosition(FindEntity(snapshot.entityIndex));
                    intensity = 1.2f;
                    duration = 20;
                    stableIdentity = SourceStableIdentity(snapshot.entityIndex);
                }
                else if (IsProjectileSpawnEvent(source.kind))
                {
                    uint projectileId = source.kind == EventLevel1CastleProjectileSpawned
                        ? unchecked((uint)source.value0)
                        : source.code;
                    ProjectileSnapshot projectile = FindProjectile(projectileId);
                    if (projectile.projectileId == 0)
                    {
                        continue;
                    }

                    kind = source.kind == EventLevel1FireballSpawned
                        ? UnityEnhancedVfxKind.Fire
                        : UnityEnhancedVfxKind.SpellCast;
                    position = ProjectedPosition(projectile.x, projectile.y, projectile.z);
                    intensity = projectile.projectileKind == 2 ? 0.8f : 1f;
                    duration = 16;
                    stableIdentity = ((ulong)snapshot.generation << 32) |
                                      (0x20000000UL | projectile.projectileId);
                }
                else if (source.kind == EventLevel1FireballImpacted ||
                         source.kind == EventLevel1WaterSplashed)
                {
                    uint impactId = source.kind == EventLevel1WaterSplashed
                        ? source.entityIndex
                        : source.code;
                    ImpactSnapshot impact = FindImpact(impactId);
                    if (impact.impactId == 0)
                    {
                        continue;
                    }

                    kind = source.kind == EventLevel1WaterSplashed
                        ? UnityEnhancedVfxKind.Weather
                        : ImpactKind(impact.model);
                    position = ProjectedPosition(impact.x, impact.y, impact.z);
                    intensity = kind == UnityEnhancedVfxKind.Fire ? 1.3f : 1f;
                    duration = kind == UnityEnhancedVfxKind.Smoke ? 28u : 18u;
                    stableIdentity = ((ulong)snapshot.generation << 32) |
                                      (0x40000000UL | impact.impactId);
                }
                else if (source.kind == EventLevel1ActorKilled)
                {
                    kind = UnityEnhancedVfxKind.Destruction;
                    position = EntityPosition(FindEntity(source.entityIndex));
                    intensity = 1.1f;
                    duration = 14;
                    stableIdentity = SourceStableIdentity(source.entityIndex);
                }
                else if (source.kind == EventLevel1ManaSphereSpawned)
                {
                    kind = UnityEnhancedVfxKind.ManaPickup;
                    position = EntityPosition(FindEntity(source.entityIndex));
                    intensity = 0.9f;
                    duration = 18;
                    stableIdentity = SourceStableIdentity(source.entityIndex);
                }
                else
                {
                    continue;
                }

                nextVfxSequence++;
                result.Add(new UnityEnhancedVfxEvent(
                    nextVfxSequence,
                    snapshot.generation,
                    snapshot.tick,
                    stableIdentity,
                    kind,
                    position,
                    intensity,
                    duration));
            }

            lastVfxEventCount = result.Count;
            return result.ToArray();
        }

        private static bool IsProjectileSpawnEvent(uint kind)
        {
            return kind == EventLevel1FireballSpawned ||
                   kind == EventLevel1ArcherArrowSpawned ||
                   kind == EventLevel1FireflyFireballSpawned ||
                   kind == EventLevel1CastleProjectileSpawned ||
                   kind == EventLevel1PossessionSpawned;
        }

        private static UnityEnhancedVfxKind ImpactKind(byte model)
        {
            if (model == 6)
            {
                return UnityEnhancedVfxKind.Fire;
            }

            if (model == 13)
            {
                return UnityEnhancedVfxKind.Smoke;
            }

            return UnityEnhancedVfxKind.Impact;
        }

        private ulong SourceStableIdentity(ushort entityIndex)
        {
            return ((ulong)snapshot.generation << 32) | (uint)(entityIndex + 1);
        }

        private Vector3 ProjectedPosition(ushort x, ushort y, short sourceZ)
        {
            float projectedX = (float)(unwrappedCameraX + ModularDelta(x, presentation.cameraX)) /
                               SourceUnitsPerUnityUnit;
            float projectedY = SourceVerticalToUnity(sourceZ);
            float projectedZ = (float)(unwrappedCameraY + ModularDelta(y, presentation.cameraY)) /
                               SourceUnitsPerUnityUnit;
            return new Vector3(projectedX, projectedY, projectedZ);
        }

        private ProjectileSnapshot FindProjectile(uint projectileId)
        {
            for (int index = 0; index < projectiles.Length; index++)
            {
                if (projectiles[index].projectileId == projectileId)
                {
                    return projectiles[index];
                }
            }

            return default(ProjectileSnapshot);
        }

        private ImpactSnapshot FindImpact(uint impactId)
        {
            for (int index = 0; index < impacts.Length; index++)
            {
                if (impacts[index].impactId == impactId)
                {
                    return impacts[index];
                }
            }

            return default(ImpactSnapshot);
        }

        private Vector3 EntityPosition(EntitySnapshot source)
        {
            if (source.entityIndex == 0 && source.active == 0)
            {
                return BuildCameraPose().Position;
            }

            float x = (float)(unwrappedCameraX + ModularDelta(source.x, presentation.cameraX)) / SourceUnitsPerUnityUnit;
            float y = SourceVerticalToUnity(source.z);
            float z = (float)(unwrappedCameraY + ModularDelta(source.y, presentation.cameraY)) / SourceUnitsPerUnityUnit;
            return new Vector3(x, y, z);
        }

        private EntitySnapshot FindEntity(ushort entityIndex)
        {
            for (int index = 0; index < entities.Length; index++)
            {
                if (entities[index].entityIndex == entityIndex)
                {
                    return entities[index];
                }
            }

            return default(EntitySnapshot);
        }

        private static int PresentationFamily(EntitySnapshot source, ushort playerEntityIndex)
        {
            if (source.entityIndex == playerEntityIndex)
            {
                return (int)UnityEnhancedPresentationFamily.Player;
            }

            if (source.classId == 2)
            {
                return source.model == 2
                    ? (int)UnityEnhancedPresentationFamily.Dolmen
                    : (int)UnityEnhancedPresentationFamily.Tree;
            }

            if (source.classId == 5)
            {
                switch (source.model)
                {
                    case 1: return (int)UnityEnhancedPresentationFamily.Goat;
                    case 4: return (int)UnityEnhancedPresentationFamily.Archer;
                    case 13: return (int)UnityEnhancedPresentationFamily.Villager;
                    case 19: return (int)UnityEnhancedPresentationFamily.Firefly;
                    default: return UnsupportedPresentationFamily;
                }
            }

            if (source.classId == 3)
            {
                return (int)UnityEnhancedPresentationFamily.Castle;
            }

            if (source.classId == 10)
            {
                switch (source.model)
                {
                    // Level 1's forty-seven imported houses are the real
                    // town landmarks, not transient smoke rows.
                    case 45: return (int)UnityEnhancedPresentationFamily.Town;
                    case 42:
                    case 43:
                    case 79: return (int)UnityEnhancedPresentationFamily.Castle;
                    case 1:
                    case 3:
                    case 39: return (int)UnityEnhancedPresentationFamily.Mana;
                    case 6: return (int)UnityEnhancedPresentationFamily.Fire;
                    case 13:
                    case 59: return (int)UnityEnhancedPresentationFamily.Smoke;
                    case 40: return (int)UnityEnhancedPresentationFamily.Impact;
                    default: return UnsupportedPresentationFamily;
                }
            }

            if (source.classId == 15)
            {
                return (int)UnityEnhancedPresentationFamily.SpellJar;
            }

            return UnsupportedPresentationFamily;
        }

        private static int ModularDelta(ushort current, ushort previous)
        {
            return ((current - previous + 32768) & 0xffff) - 32768;
        }

        private static sbyte ClampInt8(float value)
        {
            int rounded = Mathf.RoundToInt(value);
            return (sbyte)Mathf.Clamp(rounded, sbyte.MinValue, sbyte.MaxValue);
        }

        private static ushort CameraInputValue(float value)
        {
            int units = Mathf.RoundToInt(value * 64f);
            units = Mathf.Clamp(units, short.MinValue, short.MaxValue);
            return unchecked((ushort)(short)units);
        }

        private static short SignedAngle11(short value)
        {
            int result = value & 0x07ff;
            return (short)(result >= 1024 ? result - 2048 : result);
        }

        private static float VerticalFov(short sourceFov)
        {
            if (sourceFov <= 0)
            {
                return 70f;
            }

            int sourceScale = (7 * 377 * sourceFov) >> 11;
            if (sourceScale <= 0)
            {
                return 70f;
            }

            return 2f * Mathf.Atan(120f / sourceScale) * Mathf.Rad2Deg;
        }

        private static ulong HashEntities(
            EntitySnapshot[] values,
            ProjectileSnapshot[] projectiles,
            ImpactSnapshot[] impacts)
        {
            unchecked
            {
                ulong hash = 0xcbf29ce484222325UL;
                for (int index = 0; index < values.Length; index++)
                {
                    hash ^= values[index].entityIndex;
                    hash *= 0x100000001b3UL;
                    hash ^= values[index].x;
                    hash *= 0x100000001b3UL;
                    hash ^= values[index].y;
                    hash *= 0x100000001b3UL;
                    hash ^= (uint)values[index].z;
                    hash *= 0x100000001b3UL;
                    hash ^= values[index].active;
                    hash *= 0x100000001b3UL;
                }

                for (int index = 0; index < projectiles.Length; index++)
                {
                    hash ^= 0x20000000U | projectiles[index].projectileId;
                    hash *= 0x100000001b3UL;
                    hash ^= projectiles[index].x;
                    hash *= 0x100000001b3UL;
                    hash ^= projectiles[index].y;
                    hash *= 0x100000001b3UL;
                    hash ^= (uint)projectiles[index].z;
                    hash *= 0x100000001b3UL;
                    hash ^= projectiles[index].active;
                    hash *= 0x100000001b3UL;
                    hash ^= projectiles[index].projectileKind;
                    hash *= 0x100000001b3UL;
                }

                for (int index = 0; index < impacts.Length; index++)
                {
                    hash ^= 0x40000000U | impacts[index].impactId;
                    hash *= 0x100000001b3UL;
                    hash ^= impacts[index].x;
                    hash *= 0x100000001b3UL;
                    hash ^= impacts[index].y;
                    hash *= 0x100000001b3UL;
                    hash ^= (uint)impacts[index].z;
                    hash *= 0x100000001b3UL;
                    hash ^= impacts[index].active;
                    hash *= 0x100000001b3UL;
                    hash ^= impacts[index].model;
                    hash *= 0x100000001b3UL;
                }

                return hash;
            }
        }

        private ulong DynamicStableIdentity(uint namespaceId, uint sourceId)
        {
            return ((ulong)snapshot.generation << 32) |
                   (namespaceId | (sourceId & 0x1fffffffU));
        }

        private static bool AllZero(uint[] values)
        {
            if (values == null)
            {
                return false;
            }

            for (int index = 0; index < values.Length; index++)
            {
                if (values[index] != 0) return false;
            }

            return true;
        }

        private static bool AllZero(byte[] values)
        {
            if (values == null)
            {
                return false;
            }

            for (int index = 0; index < values.Length; index++)
            {
                if (values[index] != 0) return false;
            }

            return true;
        }

        private bool ValidateSessionSnapshot(out string reason)
        {
            if (snapshot.abiVersion != AbiVersion ||
                snapshot.structSize != SessionSnapshotSize ||
                snapshot.generation == 0 ||
                snapshot.reserved0 != 0 ||
                !AllZero(snapshot.reserved1) ||
                !AllZero(snapshot.reserved))
            {
                reason = "Unity Enhanced owner session snapshot failed its ABI/layout contract";
                return false;
            }

            reason = string.Empty;
            return true;
        }

        private static string StatusReason(string operation, int status)
        {
            string nativeMessage = string.Empty;
            try
            {
                if (mc_native_last_error_length(out uint length) == 0 && length > 0)
                {
                    byte[] buffer = new byte[checked((int)length + 1)];
                    if (mc_native_last_error_copy(buffer, (uint)buffer.Length, out uint written) == 0)
                    {
                        nativeMessage = Encoding.UTF8.GetString(buffer, 0, checked((int)written));
                    }
                }
            }
            catch (Exception)
            {
                nativeMessage = string.Empty;
            }

            return string.IsNullOrEmpty(nativeMessage)
                ? operation + " returned status " + status
                : operation + " returned status " + status + ": " + nativeMessage;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct Command
        {
            public uint abiVersion;
            public uint structSize;
            public sbyte opcode;
            public sbyte argument1;
            public sbyte argument2;
            public sbyte roll;
            public sbyte pitch;
            public sbyte flags;
            public short requestedSpell;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            public uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct CameraInput
        {
            public uint abiVersion;
            public uint structSize;
            public ushort lookYaw;
            public ushort lookPitch;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            public uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct SessionSnapshot
        {
            public uint abiVersion;
            public uint structSize;
            public ulong generation;
            public ulong tick;
            public ulong eventHead;
            public ulong stateHash;
            public ulong conformanceHash;
            public ushort entityIndex;
            public ushort x;
            public ushort y;
            public short z;
            public short yaw;
            public short displayPitch;
            public short effectivePitch;
            public short rollAccumulator;
            public short targetSpeed;
            public short actualSpeed;
            public short strafeSpeed;
            public byte phase;
            public byte lastResolution;
            public byte profile;
            public byte localPlayerSlot;
            public byte leftSpellSlot;
            public byte rightSpellSlot;
            public byte selectedSpellUiIndex;
            public byte objectiveTextboxTicks;
            public byte objectiveCount;
            public byte objectivesAuthority;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 2)]
            public byte[] reserved1;
            public ushort leftSpellHolder;
            public ushort rightSpellHolder;
            public ushort selectedSpellHolder;
            public ushort reserved0;
            public int casterMana;
            public int maptypeMusic;
            public uint fireSequence;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            public uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct PresentationState
        {
            public uint abiVersion;
            public uint structSize;
            public ulong generation;
            public ulong tick;
            public int sourceTurn;
            public ushort cameraX;
            public ushort cameraY;
            public short cameraZ;
            public short cameraYaw;
            public short cameraPitch;
            public short cameraRoll;
            public short cameraFov;
            public byte cameraAuthority;
            public byte cameraFlags;
            public uint reserved0;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            public uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct RenderCameraState
        {
            public uint abiVersion;
            public uint structSize;
            public ulong generation;
            public ulong tick;
            public int sourceTurn;
            public ushort historyX;
            public ushort historyY;
            public short eyeZ;
            public short yaw;
            public short pitch;
            public short roll;
            public short fov;
            public short slopeTargetX;
            public short slopeTargetY;
            public short renderOffsetX;
            public short renderOffsetY;
            public byte authority;
            public byte flags;
            public ushort reserved0;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            public uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct McPresentationFrameQueryV2
        {
            public uint abiVersion;
            public uint structSize;
            public ulong generation;
            public ulong tick;
            public ulong presentationRevision;
            public ushort viewportWidth;
            public ushort viewportHeight;
            public ushort screenStride;
            public ushort reserved0;
            public byte configuredBlurMode;
            public byte blurCapability;
            public byte highSpeedSystem;
            public byte scratchPresent;
            public byte vgaResolution;
            public byte spatialSmoothing;
            public byte minimapEnabled;
            public byte topBarEnabled;
            public byte menuOverlay;
            public byte gamePaused;
            public byte levelEnd;
            public byte objectiveSelector;
            public byte objectiveTextbox;
            public byte tutorialOrBriefing;
            public byte helpPopup;
            public byte debugText;
            public uint reserved1;
            public uint reserved2;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct McPresentationFrameInfoV2
        {
            public uint abiVersion;
            public uint structSize;
            public ulong generation;
            public ulong tick;
            public ulong presentationRevision;
            public ulong immutableInputHash;
            public ushort width;
            public ushort height;
            public ushort stride;
            public ushort viewportWidth;
            public uint indexBytes;
            public uint provenanceBytes;
            public uint flags;
            public byte effectivePostprocessMode;
            public byte historyState;
            public byte newlyCommitted;
            public byte reserved0;
            public ulong indexHash;
            public ulong provenanceHash;
            public ulong lightingHash;
            public ulong sourceArenaHash;
            public uint overlayPixelsWritten;
            public ushort overlayStageCount;
            public ushort reserved1;
            public uint reserved2;
            public uint reserved3;
            public uint reserved4;
            public uint reserved5;
            public uint reserved6;
            public uint reserved7;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct TerrainState
        {
            public uint abiVersion;
            public uint structSize;
            public ulong generation;
            public ulong tick;
            public ulong mutationSequence;
            public ulong terrainHash;
            public ushort width;
            public ushort height;
            public byte planeCount;
            public byte hashSchema;
            public byte authority;
            public byte reserved0;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            public uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct EntitySnapshot
        {
            public uint abiVersion;
            public uint structSize;
            public ushort entityIndex;
            public ushort parameterRecordIndex;
            public byte authority;
            public byte active;
            public byte classId;
            public byte model;
            public byte action;
            public byte phase;
            public byte hasParameterRecord;
            public byte reserved0;
            public ushort x;
            public ushort y;
            public short z;
            public short yaw;
            public short pitch;
            public short fov;
            public short actualSpeed;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            public uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct MissionSnapshot
        {
            public uint abiVersion;
            public uint structSize;
            public ulong generation;
            public ulong tick;
            public byte authority;
            public byte currentStage;
            public byte textSelector;
            public byte objectiveTextboxTicks;
            public byte tasksCompleted;
            public byte playerDead;
            public byte hasFireball;
            public byte hasPossession;
            public byte hasCastleSpell;
            public byte hasSpeedSpell;
            public byte castleExists;
            public byte castleLevel;
            public byte fidelityGapFlags;
            public byte exitSequenceActive;
            public byte exitSequenceComplete;
            public byte exitResultCode;
            public int playerLife;
            public int playerMaxLife;
            public int playerMana;
            public int playerMaxMana;
            public int castleMana;
            public int securedManaPercent;
            public uint actorCount;
            public uint projectileCount;
            public uint impactCount;
            public uint exitSequencePhase;
            public int exitSequenceTimer;
            public int possessionExperience;
            public ushort lastPossessedBuildingEntity;
            public ushort lastPossessedBuildingOwner;
            public ushort possessedBuildingCount;
            public byte possessionTier;
            public byte speedTrailCount;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct ProjectileSnapshot
        {
            public uint abiVersion;
            public uint structSize;
            public uint projectileId;
            public ushort ownerEntityIndex;
            public ushort renderIndex;
            public byte authority;
            public byte active;
            public byte classId;
            public byte model;
            public ushort x;
            public ushort y;
            public short z;
            public short yaw;
            public short pitch;
            public short speed;
            public short lifeTicks;
            public ushort baseTmap;
            public int payload;
            public ushort logicalHeight;
            public byte renderMode;
            public sbyte animationFrame;
            public uint projectileKind;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 2)]
            public uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct ImpactSnapshot
        {
            public uint abiVersion;
            public uint structSize;
            public uint impactId;
            public uint sourceProjectileId;
            public byte authority;
            public byte active;
            public byte damageSubmitted;
            public byte classId;
            public byte model;
            public byte renderMode;
            public sbyte animationFrame;
            public byte reserved0;
            public ushort x;
            public ushort y;
            public short z;
            public short lifeTicks;
            public int payload;
            public ushort baseTmap;
            public ushort renderParameterIndex;
            public ushort logicalHeight;
            public ushort reserved1;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 2)]
            public uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct MinimapMarks
        {
            public uint abiVersion;
            public uint structSize;
            public ulong generation;
            public ulong tick;
            public byte markCount;
            public byte markPulse;
            public byte arrowValid;
            public byte arrowVisible;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 16)]
            public short[] markX;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 16)]
            public short[] markY;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 16)]
            public byte[] markColour;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 16)]
            public byte[] markHelpSelector;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 14)]
            public short[] arrowXY;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            public uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct MinimapEntityCommand
        {
            public ushort sourceSlot;
            public short x;
            public short y;
            public ushort selector;
            public byte kind;
            public byte paletteIndex;
            public byte size;
            public byte anchor;
            public byte helpSelector;
            public byte flags;
            public ushort order;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 2)]
            public uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct MinimapEntitiesInfo
        {
            public uint abiVersion;
            public uint structSize;
            public ulong generation;
            public ulong tick;
            public uint totalCount;
            public ushort crossX;
            public ushort crossY;
            public byte crossColour;
            public byte crossArmLength;
            public byte revealActive;
            public byte authority;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            public byte[] reserved0;
            public ulong commandHash;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            public uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct EventQuery
        {
            public uint abiVersion;
            public uint structSize;
            public ulong generation;
            public ulong cursor;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            public uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct EventBatch
        {
            public uint abiVersion;
            public uint structSize;
            public ulong generation;
            public ulong nextCursor;
            public ulong eventHead;
            public uint written;
            public byte hasMore;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 3)]
            public byte[] reserved0;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            public uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct EventSnapshot
        {
            public uint abiVersion;
            public uint structSize;
            public ulong generation;
            public ulong sequence;
            public ulong stateHash;
            public uint kind;
            public uint code;
            public ushort entityIndex;
            public short playerIndex;
            public int value0;
            public int value1;
            public uint assetRefIndex;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            public uint[] reserved;
        }

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_session_open(
            byte[] packageDir,
            uint packageDirLength,
            byte[] closurePath,
            uint closurePathLength,
            out ulong handle);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_session_destroy(ulong handle);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_session_reset(
            ulong handle,
            out SessionSnapshot snapshot,
            uint snapshotSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_session_step_with_camera(
            ulong handle,
            ref Command command,
            ref CameraInput cameraInput,
            out SessionSnapshot snapshot,
            uint snapshotSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_session_presentation_frame_copy(
            ulong handle,
            ref McPresentationFrameQueryV2 query,
            IntPtr indexBuffer,
            uint indexCapacity,
            IntPtr provenanceBuffer,
            uint provenanceCapacity,
            out McPresentationFrameInfoV2 info,
            uint infoSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_level1_action_51_completion_setup_for_qualification(
            ulong handle,
            ushort houseEntityIndex,
            ushort parentEntityIndex,
            byte constructionSelector);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_level1_player_inject_damage_for_qualification(
            ulong handle,
            int damage,
            ushort attackerEntityIndex);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_presentation_state(
            ulong handle,
            out PresentationState state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_render_camera_state(
            ulong handle,
            out RenderCameraState state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_session_terrain_state(
            ulong handle,
            out TerrainState state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_terrain_plane_length(
            ulong handle,
            uint kind,
            out uint count);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_terrain_plane_copy_u8(
            ulong handle,
            uint kind,
            [Out] byte[] buffer,
            uint capacity,
            out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_entity_count(
            ulong handle,
            out uint count);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_entities_copy(
            ulong handle,
            uint startIndex,
            [Out] EntitySnapshot[] buffer,
            uint capacity,
            uint elementSize,
            out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_level1_mission_snapshot(
            ulong handle,
            out MissionSnapshot snapshot,
            uint snapshotSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_level1_minimap_marks(
            ulong handle,
            out MinimapMarks state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_level1_minimap_entities_copy(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            [Out] MinimapEntityCommand[] buffer,
            uint capacity,
            uint elementSize,
            out MinimapEntitiesInfo info,
            uint infoSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_fireball_projectiles_copy(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            uint startOrdinal,
            [Out] ProjectileSnapshot[] buffer,
            uint capacity,
            uint elementSize,
            out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_fireball_impacts_copy(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            uint startOrdinal,
            [Out] ImpactSnapshot[] buffer,
            uint capacity,
            uint elementSize,
            out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_events_copy_since(
            ulong handle,
            ref EventQuery query,
            [Out] EventSnapshot[] buffer,
            uint capacity,
            uint elementSize,
            out EventBatch batch,
            uint batchSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_last_error_length(out uint length);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_last_error_copy(
            byte[] buffer,
            uint capacity,
            out uint written);
    }
}
