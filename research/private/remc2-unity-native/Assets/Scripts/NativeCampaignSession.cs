using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Threading;
using UnityEngine;

namespace MagicCarpet.Native
{
    /// One managed consumer for descriptor-backed campaign sessions. Retained
    /// Level 2 methods are presentation adapters over this handle; Rust remains
    /// the sole owner of gameplay state, ordering, effects, and reset identity.
    internal sealed class NativeCampaignSession : IDisposable
    {
        [Serializable]
        private sealed class CampaignDefinitionEnvelope
        {
            public CampaignDefinitionIdentity identity;
        }

        [Serializable]
        private sealed class CampaignDefinitionIdentity
        {
            public ushort external_level;
            public ushort internal_level;
            public byte map_type_id;
        }

        internal readonly struct CampaignPackageIdentity
        {
            internal CampaignPackageIdentity(
                string packagePath,
                string manifestSha256,
                string definitionSha256,
                NativeSim.CampaignSelection selection)
            {
                PackagePath = packagePath;
                ManifestSha256 = manifestSha256;
                DefinitionSha256 = definitionSha256;
                Selection = selection;
            }

            internal string PackagePath { get; }
            internal string ManifestSha256 { get; }
            internal string DefinitionSha256 { get; }
            internal NativeSim.CampaignSelection Selection { get; }
        }

        private static int activeSessionCount;
        private long handleBits;
        private ulong generation;
        private ulong tick;
        private ulong effectCursor;
        private ulong voxelDeathCursor, voxelDeathGeneration = ulong.MaxValue;
        private readonly NativeSim.CampaignVoxelDeath[] voxelDeathBuffer = new NativeSim.CampaignVoxelDeath[1024];
        private readonly NativeSim.VoxelImpactChild[] voxelImpactChildBuffer = new NativeSim.VoxelImpactChild[1024];
        private readonly NativeSim.CampaignEntityPresentation[] entityPresentationBuffer = new NativeSim.CampaignEntityPresentation[1000];
        private readonly HashSet<ushort> entityPresentationSlots = new HashSet<ushort>();
        private readonly List<NativeSim.VoxelImpactChild> voxelImpactChildren = new List<NativeSim.VoxelImpactChild>();
        private readonly NativeSim.RuntimeBuilding[] buildingBuffer = new NativeSim.RuntimeBuilding[1000];
        private ModernBuildings.LiveBuilding[] buildingObservations = Array.Empty<ModernBuildings.LiveBuilding>();
        private NativeSim.CampaignCharacterState[] cachedCharacterStates;
        private ulong characterStateGeneration, characterStateTick;
        private readonly NativeSim.CampaignCastleChild[] castleChildBuffer = new NativeSim.CampaignCastleChild[128];
        private readonly HashSet<ushort> castleChildSeen = new HashSet<ushort>();
        private int castleChildCount;
        private ulong castleChildGeneration = ulong.MaxValue, castleChildTick;
        private readonly bool ownsHandle;
        private readonly bool dayMusicAvailable;
        private readonly CampaignPackageIdentity packageIdentity;
        private readonly NativeSim.CampaignWorldInfo initialWorldInfo;
        private NativeSim.PresentationFrameInfo campaignFrameInfo;
        private readonly byte[] campaignFrameIndices =
            new byte[NativeSim.TerrainFramePixels];
        private readonly byte[] campaignFrameScratchIndices =
            new byte[NativeSim.TerrainFramePixels];
        private readonly byte[] dayCompatibilityFrameIndices =
            new byte[NativeSim.DayIndexedFrameIndexBytes];
        private readonly byte[] dayIndexedFrameIndices =
            new byte[NativeSim.DayIndexedFrameIndexBytes];
        private readonly byte[] dayIndexedFramePalette =
            new byte[NativeSim.DayIndexedFramePaletteBytes];
        internal byte[] SceneFrameIndices { get; } = new byte[NativeSim.DayIndexedFrameIndexBytes];
        internal ulong SceneFrameRevision { get; private set; }
        internal byte SceneBlurMode { get; private set; }
        internal bool SceneUseExitTables { get; private set; }
        internal bool SceneSpritesComposited { get; private set; }

        private NativeCampaignSession(
            ulong handle,
            bool dayMusicAvailable,
            CampaignPackageIdentity packageIdentity,
            NativeSim.CampaignWorldInfo initialWorldInfo,
            bool ownsHandle = true)
        {
            handleBits = unchecked((long)handle);
            this.ownsHandle = ownsHandle;
            this.dayMusicAvailable = dayMusicAvailable;
            this.packageIdentity = packageIdentity;
            this.initialWorldInfo = initialWorldInfo;
            if (ownsHandle)
            {
                Interlocked.Increment(ref activeSessionCount);
            }
        }

        internal static int ActiveSessionCount =>
            Volatile.Read(ref activeSessionCount);

        internal ulong Handle
        {
            get
            {
                var value = Volatile.Read(ref handleBits);
                if (value == 0)
                {
                    throw new ObjectDisposedException(nameof(NativeCampaignSession));
                }
                return unchecked((ulong)value);
            }
        }

        internal ulong Generation => generation;

        internal ulong Tick => tick;

        internal IReadOnlyList<ModernBuildings.LiveBuilding> BuildingPresentations(out bool changed)
        {
            if (ownsHandle) throw new InvalidOperationException("Live BUILD observation requires the runtime-owned campaign");
            NativeSession.Check(NativeSim.mc_game_runtime_buildings_copy_v1(Handle, generation, tick,
                buildingBuffer, (uint)buildingBuffer.Length, 48, out var written), "read live BUILD owners");
            if (written > buildingBuffer.Length) throw new InvalidOperationException("BUILD count exceeds bounded buffer");
            changed = buildingObservations.Length != written;
            for (var i = 0; i < written; i++)
            {
                var row = buildingBuffer[i];
                if (row.abiVersion != NativeSim.AbiVersion || row.structSize != 48 ||
                    row.generation != generation || row.tick != tick || row.active > 1 || row.drawing > 1 ||
                    row.reserved0 != 0 || row.reserved1 != 0 || row.reserved2 != 0)
                    throw new InvalidOperationException("BUILD observation identity or layout mismatch");
                if (changed) continue;
                var previous = buildingObservations[i];
                changed = previous.Slot != row.sourceSlot || previous.SlotGeneration != row.slotGeneration ||
                    previous.BuildingIndex != row.buildingIndex || previous.Action != row.action ||
                    previous.Drawing != (row.active != 0 && row.drawing != 0) ||
                    previous.X != row.x || previous.Y != row.y || previous.Z != row.z;
            }
            if (changed)
            {
                var next = new ModernBuildings.LiveBuilding[written];
                for (var i = 0; i < written; i++) {
                    var row = buildingBuffer[i];
                    next[i] = new ModernBuildings.LiveBuilding { Slot = row.sourceSlot,
                        SlotGeneration = row.slotGeneration, BuildingIndex = row.buildingIndex,
                        Action = row.action, Drawing = row.active != 0 && row.drawing != 0,
                        X = row.x, Y = row.y, Z = row.z };
                }
                buildingObservations = next;
            }
            return buildingObservations;
        }

        // Stable for a voxel world's lifetime; the outer session generation
        // may change during a pause/preferences rebind without a world reset.
        internal ulong VoxelWorldEpoch { get; private set; }

        [Serializable] private sealed class VoxelEnableRequest {public ushort selected_slot;public uint[] modern_heights;}
        [Serializable] internal sealed class VoxelVolumeProjection {
            public int kind,slot,origin_x,origin_y,origin_z,cell_size;
            public int[] dimensions,terrain_columns,building_bytes;
        }
        [Serializable] internal sealed class VoxelRemovedCell {public int kind,slot,x,y,z;}
        [Serializable] internal sealed class VoxelImpact {
            public string sequence,revision;
            public int projectile_slot,slot_generation,model,from_x,from_y,from_z,x,y,z,target_slot;
            public int velocity_x,velocity_y,velocity_z,normal_x,normal_y,normal_z;
            public bool has_target,geometry_changed;
        }
        [Serializable] internal sealed class VoxelProjection {
            public string generation,tick,revision,content_hash,base_hash,impact_head;
            public VoxelVolumeProjection[] volumes;
            public VoxelRemovedCell[] removed;
            public VoxelImpact[] impacts;
        }

        internal void EnableVoxelWorld(ushort selectedSlot,float[] modernHeights) {
            if(modernHeights==null||modernHeights.Length!=17*21+256*320)throw new ArgumentException("Invalid voxel seam samples");
            var bits=new uint[modernHeights.Length];
            for(int i=0;i<bits.Length;i++){
                if(!float.IsFinite(modernHeights[i]))throw new ArgumentException("Nonfinite voxel seam sample");
                bits[i]=unchecked((uint)BitConverter.SingleToInt32Bits(modernHeights[i]));
            }
            var bytes=Encoding.UTF8.GetBytes(JsonUtility.ToJson(new VoxelEnableRequest{selected_slot=selectedSlot,modern_heights=bits}));
            NativeSession.Check(NativeSim.mc_game_runtime_voxel_enable_json_v1(Handle,generation,tick,bytes,(uint)bytes.Length),"enable native voxel geometry");
        }

        internal NativeSim.VoxelState VoxelWorldState() {
            NativeSession.Check(NativeSim.mc_game_runtime_voxel_state_v1(Handle,generation,tick,out var state,(uint)Marshal.SizeOf<NativeSim.VoxelState>()),"read native voxel state");
            if(state.abiVersion!=NativeSim.AbiVersion||state.structSize!=64||state.generation!=generation||state.tick!=tick||state.enabled>1)
                throw new InvalidOperationException("Native voxel state identity/layout mismatch");
            VoxelWorldEpoch=state.worldGeneration;
            return state;
        }

        // Native checks each impact child's current active lifetime and model
        // for this exact tick; consumers must not retain these slots across ticks.
        internal IReadOnlyList<NativeSim.VoxelImpactChild> VoxelImpactChildren()
        {
            NativeSession.Check(NativeSim.mc_game_runtime_voxel_impact_children_copy(Handle,generation,tick,voxelImpactChildBuffer,
                (uint)voxelImpactChildBuffer.Length,32,out var written),"read live voxel impact children");
            if(written>voxelImpactChildBuffer.Length)throw new InvalidOperationException("Native voxel impact child count exceeds buffer");
            voxelImpactChildren.Clear();
            for(var i=0;i<written;i++)
            {
                var row=voxelImpactChildBuffer[i];
                if(row.abiVersion!=NativeSim.AbiVersion||row.structSize!=32||row.generation!=generation||row.sequence==0||row.reserved!=0)
                    throw new InvalidOperationException("Native voxel impact child identity/layout mismatch");
                voxelImpactChildren.Add(row);
            }
            return voxelImpactChildren;
        }

        internal VoxelProjection VoxelWorldProjection(bool includeBase,ulong afterImpact=0) {
            int status=NativeSim.mc_game_runtime_voxel_snapshot_json_v1(Handle,generation,tick,includeBase?1u:0u,afterImpact,IntPtr.Zero,0,out var length);
            if(status!=NativeSim.BufferTooSmall&&!(status==NativeSim.Bounds&&length>0))NativeSession.Check(status,"size native voxel snapshot");
            if(length==0||length>16*1024*1024)throw new InvalidOperationException("Native voxel snapshot exceeds bound");
            var memory=Marshal.AllocHGlobal(checked((int)length));
            try{
                NativeSession.Check(NativeSim.mc_game_runtime_voxel_snapshot_json_v1(Handle,generation,tick,includeBase?1u:0u,afterImpact,memory,length,out var written),"read native voxel snapshot");
                if(written>length)throw new InvalidOperationException("Native voxel snapshot grew without an authoritative step");
                var bytes=new byte[written];Marshal.Copy(memory,bytes,0,bytes.Length);
                var value=JsonUtility.FromJson<VoxelProjection>(Encoding.UTF8.GetString(bytes));
                if(value==null||value.generation!=generation.ToString()||value.tick!=tick.ToString()||!ulong.TryParse(value.base_hash,out _)||value.removed==null||value.volumes==null||value.impacts==null)
                    throw new InvalidOperationException("Native voxel snapshot identity/data mismatch");
                return value;
            }finally{Marshal.FreeHGlobal(memory);}
        }

        internal ulong EffectCursor => effectCursor;

        internal CampaignPackageIdentity PackageIdentity => packageIdentity;

        internal NativeSim.CampaignSelection Selection => packageIdentity.Selection;

        internal NativeSim.CampaignWorldInfo InitialWorldInfo => initialWorldInfo;

        internal NativeSim.PresentationFrameInfo CampaignFrameInfo => campaignFrameInfo;

        internal byte[] CampaignFrameIndices => campaignFrameIndices;

        internal byte[] DayIndexedFrameIndices => dayIndexedFrameIndices;

        internal byte[] DayIndexedFramePalette => dayIndexedFramePalette;
        internal ushort[] CaveTerrainOrders { get; private set; }
        internal ushort[] CaveCellOrders { get; private set; }

        internal static NativeCampaignSession OpenLevel2(string packagePath)
        {
            return Open(packagePath, NativeSim.CampaignSelection.Level2Day(), null);
        }

        internal static NativeCampaignSession OpenLevel2(
            string packagePath,
            string assetCachePath)
        {
            return Open(
                packagePath,
                NativeSim.CampaignSelection.Level2Day(),
                assetCachePath);
        }

        internal static NativeCampaignSession Open(
            string packagePath,
            NativeSim.CampaignSelection selection,
            string assetCachePath = null)
        {
            if (string.IsNullOrWhiteSpace(packagePath))
            {
                throw new InvalidDataException(
                    "campaign package path must be non-empty");
            }
            packagePath = Path.GetFullPath(packagePath);
            ValidateSelection(selection);
            var identity = ReadPackageIdentity(packagePath, selection);
            NativeSim.AssertLayouts();
            if (NativeSim.mc_native_abi_version() != NativeSim.AbiVersion)
            {
                throw new InvalidOperationException(
                    "sim_capi native ABI is not version 2");
            }

            var package = Encoding.UTF8.GetBytes(packagePath);
            var closure = Array.Empty<byte>();
            var overrideCachePath = !string.IsNullOrWhiteSpace(assetCachePath);
            int status;
            ulong handle;
            var dayMusicAvailable = false;
            if (overrideCachePath)
            {
                var fullCachePath = Path.GetFullPath(assetCachePath);
                dayMusicAvailable = HasDayMusicAssets(fullCachePath);
                var cachePath = Encoding.UTF8.GetBytes(fullCachePath);
                status = NativeSim.mc_native_campaign_session_open_with_asset_cache(
                    package, checked((uint)package.Length), closure, 0,
                    cachePath, checked((uint)cachePath.Length), ref selection,
                    out handle);
            }
            else
            {
                status = NativeSim.mc_native_campaign_session_open(
                    package, checked((uint)package.Length), closure, 0,
                    ref selection, out handle);
            }
            NativeSession.Check(status, "open campaign session");
            if (handle == 0)
            {
                throw new InvalidOperationException(
                    "campaign session returned handle zero");
            }
            try
            {
                var worldStatus = NativeSim.mc_native_campaign_world_info(
                    handle,
                    out var worldInfo,
                    64);
                NativeSession.Check(worldStatus, "read opened campaign world identity");
                ValidateWorldInfo(worldInfo, selection);
                var afterOpen = ReadPackageIdentity(packagePath, selection);
                if (!SameIdentity(identity, afterOpen))
                {
                    throw new InvalidDataException(
                        "campaign package or definition changed while opening");
                }
                return new NativeCampaignSession(
                    handle,
                    dayMusicAvailable,
                    identity,
                    worldInfo);
            }
            catch (Exception error)
            {
                var destroyStatus = NativeSim.mc_native_campaign_session_destroy(handle);
                if (destroyStatus != NativeSim.Ok)
                {
                    throw new InvalidOperationException(
                        "campaign open validation failed and handle cleanup returned " +
                        $"status {destroyStatus}: {NativeSession.ReadLastError()}",
                        error);
                }
                throw;
            }
        }

        /// Read-only retained Level 2 projections over the campaign nested in
        /// a game-runtime handle.  Disposal never destroys the borrowed
        /// runtime, and gameplay mutation remains on GameRuntimeTransport.
        internal static NativeCampaignSession BorrowGameRuntime(
            ulong handle,
            GameRuntimeTransport.Identity identity,
            string packagePath)
        {
            return BorrowGameRuntime(
                handle,
                identity,
                packagePath,
                NativeSim.CampaignSelection.Level2Day());
        }

        internal static NativeCampaignSession BorrowGameRuntime(
            ulong handle,
            GameRuntimeTransport.Identity identity,
            string packagePath,
            NativeSim.CampaignSelection selection)
        {
            if (handle == 0 || identity?.session == null ||
                identity.session.session_id != handle.ToString() ||
                (identity.mode != GameRuntimeTransport.PlayingMode &&
                 identity.mode != GameRuntimeTransport.PausedMode))
            {
                throw new InvalidOperationException(
                    "campaign presentation borrow requires the bound runtime handle");
            }
            ValidateSelection(selection);
            var packageIdentity = ReadPackageIdentity(packagePath, selection);
            var status = NativeSim.mc_native_campaign_world_info(
                handle, out var worldInfo, 64);
            NativeSession.Check(status, "read borrowed campaign world identity");
            ValidateWorldInfo(worldInfo, packageIdentity.Selection);
            var afterBorrow = ReadPackageIdentity(
                packagePath,
                packageIdentity.Selection);
            if (!SameIdentity(packageIdentity, afterBorrow))
            {
                throw new InvalidDataException(
                    "campaign package or definition changed while borrowing");
            }
            var borrowed = new NativeCampaignSession(
                handle,
                false,
                packageIdentity,
                worldInfo,
                false);
            borrowed.SynchronizeGameRuntimeIdentity(identity);
            return borrowed;
        }

        internal void SynchronizeGameRuntimeIdentity(
            GameRuntimeTransport.Identity identity)
        {
            if (ownsHandle || identity?.session == null ||
                identity.session.session_id != Handle.ToString() ||
                (identity.mode != GameRuntimeTransport.PlayingMode &&
                 identity.mode != GameRuntimeTransport.PausedMode) ||
                !ulong.TryParse(identity.session.generation, out generation) ||
                !ulong.TryParse(identity.session.tick, out tick))
            {
                throw new InvalidOperationException(
                    "borrowed campaign projection lost its runtime identity binding");
            }
        }

        internal static CampaignPackageIdentity ReadPackageIdentity(
            string packagePath,
            NativeSim.CampaignSelection selection)
        {
            ValidateSelection(selection);
            if (string.IsNullOrWhiteSpace(packagePath))
            {
                throw new InvalidDataException(
                    "campaign package path must be non-empty");
            }

            var package = Path.GetFullPath(packagePath);
            if (!Directory.Exists(package))
            {
                throw new InvalidDataException(
                    $"campaign package is missing: {package}");
            }
            if ((File.GetAttributes(package) & FileAttributes.ReparsePoint) != 0)
            {
                throw new InvalidDataException(
                    $"campaign package cannot be a symbolic link: {package}");
            }

            var manifest = RequireRegularFile(package, "manifest.json");
            var definition = RequireRegularFile(package, "campaign-definition.json");
            var envelope = JsonUtility.FromJson<CampaignDefinitionEnvelope>(
                File.ReadAllText(definition));
            if (envelope?.identity == null)
            {
                throw new InvalidDataException(
                    "campaign package definition has no source selection identity");
            }
            var packageSelection = NativeSim.CampaignSelection.Create(
                envelope.identity.external_level,
                envelope.identity.internal_level,
                envelope.identity.map_type_id);
            ValidateSelection(packageSelection);
            if (!SameSelection(packageSelection, selection))
            {
                throw new InvalidDataException(
                    "campaign package definition does not match the requested selection");
            }
            return new CampaignPackageIdentity(
                package,
                Sha256(manifest),
                Sha256(definition),
                packageSelection);
        }

        private static string RequireRegularFile(string packagePath, string name)
        {
            var file = Path.Combine(packagePath, name);
            if (!File.Exists(file))
            {
                throw new InvalidDataException(
                    $"campaign package is missing {name}");
            }
            var attributes = File.GetAttributes(file);
            if ((attributes & FileAttributes.ReparsePoint) != 0 ||
                (attributes & FileAttributes.Directory) != 0)
            {
                throw new InvalidDataException(
                    $"campaign package {name} must be a regular file");
            }
            return file;
        }

        private static string Sha256(string file)
        {
            using var sha256 = SHA256.Create();
            using var input = File.OpenRead(file);
            return BitConverter.ToString(sha256.ComputeHash(input))
                .Replace("-", string.Empty)
                .ToLowerInvariant();
        }

        private static bool HasDayMusicAssets(string assetCachePath)
        {
            return File.Exists(Path.Combine(
                       assetCachePath,
                       "components/audio/manifest.json")) ||
                   File.Exists(Path.Combine(
                       assetCachePath,
                       "audio/descriptor.json"));
        }

        private static bool SameIdentity(
            CampaignPackageIdentity left,
            CampaignPackageIdentity right)
        {
            return left.PackagePath == right.PackagePath &&
                   left.ManifestSha256 == right.ManifestSha256 &&
                   left.DefinitionSha256 == right.DefinitionSha256 &&
                   SameSelection(left.Selection, right.Selection);
        }

        private static bool SameSelection(
            NativeSim.CampaignSelection left,
            NativeSim.CampaignSelection right)
        {
            return left.externalLevel == right.externalLevel &&
                   left.internalLevel == right.internalLevel &&
                   left.mapType == right.mapType;
        }

        private static void ValidateSelection(NativeSim.CampaignSelection selection)
        {
            if (selection.abiVersion != NativeSim.AbiVersion ||
                selection.structSize != 32 ||
                selection.reserved0 != 0 ||
                !AllZero(selection.reserved))
            {
                throw new InvalidDataException(
                    "campaign selection has an invalid ABI or reserved state");
            }
        }

        private static void ValidateWorldInfo(
            NativeSim.CampaignWorldInfo info,
            NativeSim.CampaignSelection selection)
        {
            if (info.abiVersion != NativeSim.AbiVersion ||
                info.structSize != 64 ||
                info.externalLevel != selection.externalLevel ||
                info.internalLevel != selection.internalLevel ||
                info.mapType != selection.mapType ||
                info.terrainWidth == 0 ||
                info.terrainHeight == 0 ||
                info.resetStateHash == 0 ||
                info.reserved0 != 0 ||
                !AllZero(info.reserved))
            {
                throw new InvalidDataException(
                    "native campaign world identity does not match the requested selection");
            }
        }

        internal NativeSim.CampaignWorldInfo WorldInfo()
        {
            var status = NativeSim.mc_native_campaign_world_info(
                Handle,
                out var info,
                64);
            NativeSession.Check(status, "read campaign world info");
            ValidateWorldInfo(info, packageIdentity.Selection);
            return info;
        }

        internal int CopyEffects(
            ulong cursor,
            NativeSim.NativeEvent[] buffer,
            out NativeSim.EventBatch batch)
        {
            if (buffer == null)
            {
                throw new ArgumentNullException(nameof(buffer));
            }
            var query = NativeSim.EventQuery.Create(generation, cursor);
            return NativeSim.mc_native_campaign_events_copy_since(
                Handle,
                ref query,
                buffer,
                checked((uint)buffer.Length),
                72,
                out batch,
                56);
        }

        internal void BeginRuntimeEffectConsumption(
            EngineSessionTransport.CampaignRenderObservation snapshot)
        {
            if (ownsHandle || snapshot?.identity == null || snapshot.value == null ||
                snapshot.identity.session_id != Handle.ToString() ||
                snapshot.identity.generation != generation.ToString() ||
                snapshot.identity.tick != tick.ToString() ||
                !ulong.TryParse(snapshot.value.effect_head, out var head))
                throw new InvalidOperationException("effect cursor requires the bound runtime snapshot");
            // Restore/reset begins at the authoritative head, not historical audio.
            effectCursor = head;
        }

        internal NativeSim.NativeEvent[] DrainEffects()
        {
            const int PageCapacity = 1_024;
            const int RetainedEffectBound = 16_384;
            var cursor = effectCursor;
            var effects = new List<NativeSim.NativeEvent>();
            while (true)
            {
                var page = new NativeSim.NativeEvent[PageCapacity];
                var status = CopyEffects(cursor, page, out var batch);
                NativeSession.Check(status, "copy campaign effects");
                ValidateEffectBatch(batch, generation, cursor, page.Length);
                for (var index = 0; index < checked((int)batch.written); index++)
                {
                    var effect = page[index];
                    ValidateEffect(
                        effect,
                        generation,
                        checked(cursor + (ulong)index + 1));
                    effects.Add(effect);
                }
                if (effects.Count > RetainedEffectBound)
                {
                    throw new InvalidOperationException(
                        "campaign effect journal exceeds the retained safety bound");
                }
                cursor = batch.nextCursor;
                if (batch.hasMore == 0)
                {
                    break;
                }
                if (batch.written == 0)
                {
                    throw new InvalidOperationException(
                        "campaign effect journal page made no progress");
                }
            }
            effectCursor = cursor;
            return effects.ToArray();
        }

        private static void ValidateEffectBatch(
            NativeSim.EventBatch batch,
            ulong expectedGeneration,
            ulong requestedCursor,
            int capacity)
        {
            if (batch.abiVersion != NativeSim.AbiVersion ||
                batch.structSize != 56 ||
                batch.generation != expectedGeneration ||
                batch.written > checked((uint)capacity) ||
                batch.hasMore > 1 ||
                batch.nextCursor != requestedCursor + batch.written ||
                batch.eventHead < batch.nextCursor ||
                (batch.hasMore == 0 && batch.nextCursor != batch.eventHead) ||
                (batch.hasMore != 0 && batch.nextCursor >= batch.eventHead) ||
                !AllZero(batch.reserved0) ||
                !AllZero(batch.reserved))
            {
                throw new InvalidOperationException(
                    "native campaign effect page violates the generic journal contract");
            }
        }

        private static void ValidateEffect(
            NativeSim.NativeEvent effect,
            ulong expectedGeneration,
            ulong expectedSequence)
        {
            if (effect.abiVersion != NativeSim.AbiVersion ||
                effect.structSize != 72 ||
                effect.generation != expectedGeneration ||
                effect.sequence != expectedSequence ||
                effect.stateHash == 0 ||
                (effect.kind != NativeSim.CampaignEffectDispatch &&
                 effect.kind != NativeSim.CampaignEffectSharedSpell &&
                 effect.kind != NativeSim.CampaignEffectFlight &&
                 effect.kind != NativeSim.CampaignEffectSceneChange &&
                 effect.kind != NativeSim.CampaignEffectObjectivePresentation) ||
                !AllZero(effect.reserved))
            {
                throw new InvalidOperationException(
                    "native campaign effect violates the generic journal contract");
            }
        }

        internal NativeSim.PresentationFrameInfo CopyPresentationFrame(
            ulong expectedTick)
        {
            var query = NativeSim.PresentationFrameQuery.CampaignSubset(
                generation,
                expectedTick);
            var status = NativeSim.mc_native_campaign_presentation_frame_copy(
                Handle,
                ref query,
                campaignFrameScratchIndices,
                checked((uint)campaignFrameScratchIndices.Length),
                IntPtr.Zero,
                0,
                out var info,
                128);
            NativeSession.Check(status, "copy generic campaign presentation frame");
            ValidateCampaignFrameInfo(info, generation, expectedTick, Selection.mapType);
            Buffer.BlockCopy(
                campaignFrameScratchIndices,
                0,
                campaignFrameIndices,
                0,
                campaignFrameIndices.Length);
            campaignFrameInfo = info;
            return info;
        }

        internal static void ValidateCampaignFrameInfo(
            NativeSim.PresentationFrameInfo info,
            ulong expectedGeneration,
            ulong expectedTick,
            byte mapType = 0)
        {
            var expectedFlags =
                NativeSim.TerrainFrameIsolatedSkyTerrain |
                NativeSim.TerrainFrameNonTerrainOmitted;
            if (mapType != 2) expectedFlags |= NativeSim.TerrainFrameSkySeeded;
            if (info.abiVersion != NativeSim.AbiVersion ||
                mapType > 2 ||
                info.structSize != 128 ||
                info.generation != expectedGeneration ||
                info.tick != expectedTick ||
                info.presentationRevision != expectedTick ||
                info.immutableInputHash == 0 ||
                info.width != NativeSim.TerrainFrameWidth ||
                info.height != NativeSim.TerrainFrameHeight ||
                info.stride != NativeSim.TerrainFrameWidth ||
                info.viewportWidth != NativeSim.TerrainFrameWidth ||
                info.indexBytes != checked((uint)NativeSim.TerrainFramePixels) ||
                info.provenanceBytes != 0 ||
                info.flags != expectedFlags ||
                info.effectivePostprocessMode != 0 ||
                info.historyState != 1 ||
                info.newlyCommitted > 1 ||
                info.reserved0 != 0 ||
                info.indexHash == 0 ||
                info.provenanceHash != 0 ||
                info.lightingHash != 0 ||
                info.sourceArenaHash != 0 ||
                info.overlayPixelsWritten != 0 ||
                info.overlayStageCount != 0 ||
                info.reserved1 != 0 ||
                info.reserved2 != 0 ||
                info.reserved3 != 0 ||
                info.reserved4 != 0 ||
                info.reserved5 != 0 ||
                info.reserved6 != 0 ||
                info.reserved7 != 0)
            {
                throw new InvalidOperationException(
                    "native campaign frame violates the generic presentation subset");
            }
        }

        internal NativeSim.CampaignTerrainState TerrainState()
        {
            var status = NativeSim.mc_native_campaign_terrain_state(
                Handle,
                generation,
                out var state,
                64);
            NativeSession.Check(status, "read Level 2 campaign terrain state");
            ValidateTerrainState(state, generation);
            return state;
        }

        internal NativeSim.CampaignDayIndexedFrameInfo RenderDayIndexedFrame(
            ulong expectedTick)
        {
            var info = default(NativeSim.CampaignDayIndexedFrameInfo);
            var status = NativeSim.mc_native_campaign_day_indexed_frame_render(
                Handle,
                generation,
                expectedTick,
                ref info,
                152);
            NativeSession.Check(status, "render Level 2 Day indexed frame");
            ValidateDayIndexedFrameInfo(info, generation, expectedTick);
            return info;
        }

        internal NativeSim.CampaignDayIndexedFrameInfo CopyDayIndexedFrame(
            ulong expectedTick)
        {
            var generic = CopyPresentationFrame(expectedTick);
            var info = RenderDayIndexedFrame(expectedTick);
            var status = NativeSim.mc_native_campaign_day_indexed_frame_copy(
                Handle,
                generation,
                expectedTick,
                dayCompatibilityFrameIndices,
                checked((uint)dayCompatibilityFrameIndices.Length),
                dayIndexedFramePalette,
                checked((uint)dayIndexedFramePalette.Length),
                out var indexWritten,
                out var paletteWritten);
            NativeSession.Check(status, "copy Level 2 Day indexed frame");
            if (
                indexWritten != (uint)dayIndexedFrameIndices.Length ||
                paletteWritten != (uint)dayIndexedFramePalette.Length)
            {
                throw new InvalidOperationException(
                    "Level 2 Day indexed frame copy returned incomplete buffers");
            }
            if (generic.indexHash != info.indexHash ||
                !campaignFrameIndices.SequenceEqual(dayCompatibilityFrameIndices))
            {
                throw new InvalidOperationException(
                    "generic campaign frame disagrees with the retained Level 2 Day adapter");
            }
            Buffer.BlockCopy(
                campaignFrameIndices,
                0,
                dayIndexedFrameIndices,
                0,
                dayIndexedFrameIndices.Length);
            NativeSession.Check(NativeSim.mc_native_campaign_scene_frame_copy(
                Handle, generation, expectedTick, SceneFrameIndices, (uint)SceneFrameIndices.Length,
                out var sceneWritten, out var sceneRevision, out var blurMode, out var useExitTables,
                out var sourceSpritesComposited),
                "copy composed-world source inputs");
            if (sceneWritten != SceneFrameIndices.Length || blurMode > 2 || useExitTables > 1 ||
                sourceSpritesComposited > 1)
                throw new InvalidOperationException("Scene frame returned incomplete bytes or invalid source state");
            SceneFrameRevision = sceneRevision;
            SceneBlurMode = blurMode;
            SceneUseExitTables = useExitTables != 0;
            SceneSpritesComposited = sourceSpritesComposited != 0;
            {
                // The retained ABI name is historical; outdoor and Cave
                // frames both supply source painter-order coverage.
                CaveTerrainOrders ??= new ushort[320 * 200];
                CaveCellOrders ??= new ushort[256 * 256];
                NativeSession.Check(NativeSim.mc_native_campaign_cave_occlusion_copy(
                    Handle, generation, expectedTick, CaveTerrainOrders, (uint)CaveTerrainOrders.Length,
                    CaveCellOrders, (uint)CaveCellOrders.Length, out var terrainWritten, out var cellWritten),
                    "copy source terrain occlusion order");
                if (terrainWritten != CaveTerrainOrders.Length || cellWritten != CaveCellOrders.Length)
                    throw new InvalidOperationException("Terrain occlusion copy returned incomplete planes");
            }
            return info;
        }

        internal (NativeSim.CampaignDayIndexedFrameInfo info,
            byte[] indices,
            byte[] palette) DayIndexedFrame(ulong expectedTick)
        {
            var info = CopyDayIndexedFrame(expectedTick);
            return (info, dayIndexedFrameIndices, dayIndexedFramePalette);
        }

        internal static void ValidateDayIndexedFrameInfo(
            NativeSim.CampaignDayIndexedFrameInfo info,
            ulong expectedGeneration,
            ulong expectedTick)
        {
            if (
                info.abiVersion != NativeSim.AbiVersion ||
                info.structSize != 152 ||
                info.generation != expectedGeneration ||
                info.tick != expectedTick ||
                info.width != NativeSim.DayIndexedFrameWidth ||
                info.height != NativeSim.DayIndexedFrameHeight ||
                info.stride != NativeSim.DayIndexedFrameWidth ||
                info.reserved0 != 0 ||
                info.indexBytes != NativeSim.DayIndexedFrameIndexBytes ||
                info.paletteBytes != NativeSim.DayIndexedFramePaletteBytes ||
                info.indexHash == 0 ||
                info.paletteHash == 0 ||
                info.terrainInputHash == 0 ||
                info.terrainCommandHash == 0 ||
                AllZero(info.packageSha256) ||
                AllZero(info.cacheContentSha256) ||
                !AllZero(info.reserved))
            {
                throw new InvalidOperationException(
                    "native Level 2 Day indexed frame returned invalid ABI, " +
                    "generation/tick, dimensions, hashes, or reserved state");
            }
        }

        internal static void ValidateTerrainState(
            NativeSim.CampaignTerrainState state,
            ulong expectedGeneration)
        {
            if (
                state.abiVersion != NativeSim.AbiVersion ||
                state.structSize != 64 ||
                state.generation != expectedGeneration ||
                state.width != 256 ||
                state.height != 256 ||
                state.planeCount != 6 ||
                state.hashSchema != 1 ||
                state.authority != NativeSim.SessionTerrainLive ||
                state.reserved0 != 0 ||
                !AllZero(state.reserved))
            {
                throw new InvalidOperationException(
                "native Level 2 terrain state returned invalid generation, " +
                "layout, dimensions, hash schema, authority, or reserved state");
            }
        }

        internal NativeSim.CampaignDayHudSnapshot DayHudState(ulong expectedTick)
        {
            var state = default(NativeSim.CampaignDayHudSnapshot);
            var status = NativeSim.mc_native_campaign_day_hud_state(
                Handle,
                generation,
                expectedTick,
                ref state,
                272);
            NativeSession.Check(status, "read campaign Day HUD state");
            ValidateDayHudState(
                state,
                generation,
                expectedTick,
                packageIdentity.Selection.externalLevel);
            return state;
        }

        internal byte CastleHudHitFrames(ulong expectedTick)
        {
            var status = NativeSim.mc_native_campaign_castle_hud_hit_frames(
                Handle, generation, expectedTick, out var frames);
            NativeSession.Check(status, "read campaign Castle HUD hit frames");
            if (frames > 4)
                throw new InvalidOperationException("native Castle HUD hit counter is outside the source range");
            return frames;
        }

        internal static void ValidateDayHudState(
            NativeSim.CampaignDayHudSnapshot state,
            ulong expectedGeneration,
            ulong expectedTick)
        {
            ValidateDayHudState(state, expectedGeneration, expectedTick, 2);
        }

        private static void ValidateDayHudState(
            NativeSim.CampaignDayHudSnapshot state,
            ulong expectedGeneration,
            ulong expectedTick,
            int externalLevel)
        {
            if (
                state.abiVersion != NativeSim.AbiVersion ||
                state.structSize != 272 ||
                state.generation != expectedGeneration ||
                state.tick != expectedTick ||
                state.localPlayerActive > 1 ||
                state.localPlayerMaxLife <= 0 ||
                state.localPlayerMaxMana <= 0 ||
                state.globalManaDenominator <= 0 ||
                !OwnerAssetCache.IsQualifiedDayHudSpriteSelector(state.minimapPanelSelector) ||
                !OwnerAssetCache.IsQualifiedDayHudSpriteSelector(state.castlePanelSelector) ||
                !OwnerAssetCache.IsQualifiedDayHudSpriteSelector(state.balloonPanelSelector) ||
                !OwnerAssetCache.IsQualifiedDayHudSpriteSelector(state.playerPanelSelector) ||
                !OwnerAssetCache.IsQualifiedDayHudSpriteSelector(state.playerIconSelector) ||
                !OwnerAssetCache.IsQualifiedDayHudSpriteSelector(state.lifeManaIconSelector) ||
                state.lifePaletteIndex == 0 ||
                state.manaPaletteIndex == 0 ||
                state.manaMaxPaletteIndex == 0 ||
                state.colorPhase1 > 1 ||
                state.colorPhase2 > 1 ||
                state.reserved0 != 0)
            {
                throw new InvalidOperationException(
                    "native Level 2 Day HUD state returned invalid ABI, " +
                    "generation/tick, status selectors, values, or source phases");
            }

            ValidateDayHudSpellSlot(state.left, state.localPlayerEntityIndex);
            ValidateDayHudSpellSlot(state.right, state.localPlayerEntityIndex);
            // The successful native HUD call has already validated the full
            // registered selection against this bound live campaign owner.
            var nyphur = state.nyphur;
            if (
                nyphur.abiVersion != NativeSim.AbiVersion ||
                nyphur.structSize != 104 ||
                nyphur.generation != expectedGeneration ||
                nyphur.active > 1 ||
                (nyphur.visibilityFlags & ~0x1Fu) != 0 ||
                !AllZero(nyphur.reserved4) ||
                !AllZero(nyphur.reserved))
            {
                throw new InvalidOperationException(
                    "native Level 2 Day HUD state returned an unqualified Nyphur row");
            }
            if (externalLevel == 2)
            {
                if (nyphur.active != 1 || nyphur.playerSlot != 1 ||
                    nyphur.isAi != 1 || nyphur.colorIndex != 1 ||
                    nyphur.entityIndex != 112 || nyphur.entityClass != 3 ||
                    nyphur.entityModel != 1 || nyphur.entityAction != 1 ||
                    nyphur.fireballHolderIndex != 113 ||
                    nyphur.possessionHolderIndex != 114)
                    throw new InvalidOperationException(
                        "native Level 2 Day HUD state returned an unqualified Nyphur row");
            }
            else if (nyphur.active == 0)
            {
                if (nyphur.playerSlot != byte.MaxValue ||
                    nyphur.entityIndex != ushort.MaxValue ||
                    nyphur.fireballHolderIndex != 0 ||
                    nyphur.possessionHolderIndex != 0)
                    throw new InvalidOperationException(
                        "campaign Day HUD absent opponent sentinel is invalid");
            }
            else if (nyphur.playerSlot != 1 || nyphur.isAi != 1 ||
                     nyphur.entityClass != 3 || nyphur.entityModel != 1)
            {
                throw new InvalidOperationException(
                    "campaign Day HUD package opponent identity is invalid");
            }
        }

        private static void ValidateDayHudSpellSlot(
            NativeSim.CampaignDayHudSpellSlot slot,
            ushort localPlayerEntityIndex)
        {
            if (slot.drawAdmitted > 1 || slot.helpUnavailable > 1 ||
                slot.reservedPadding != 0 || !AllZero(slot.reserved))
            {
                throw new InvalidOperationException(
                    "native Level 2 Day HUD spell row has invalid admission or reserved state");
            }
            if (slot.rawSpellSelector < 0)
            {
                if (
                    slot.holderEntityIndex != 0 ||
                    slot.parentEntityIndex != 0 ||
                    slot.frameSelector != 0 ||
                    slot.iconSelector != 0 ||
                    slot.levelTextSelector != 0 ||
                    slot.drawAdmitted != 0 ||
                    slot.helpUnavailable != 0)
                {
                    throw new InvalidOperationException(
                        "native Level 2 Day HUD empty spell row is not empty");
                }
                return;
            }
            if (
                slot.holderEntityIndex == 0 ||
                slot.parentEntityIndex != localPlayerEntityIndex ||
                slot.tier < 0 ||
                !OwnerAssetCache.IsQualifiedDayHudSpriteSelector(slot.frameSelector) ||
                !OwnerAssetCache.IsQualifiedDayHudSpriteSelector(slot.iconSelector) ||
                !OwnerAssetCache.IsQualifiedDayHudLevelTextSelector(slot.levelTextSelector))
            {
                throw new InvalidOperationException(
                    "native Level 2 Day HUD spell row is outside qualified owner roots");
            }
        }

        private readonly byte[] worldMapIndices = new byte[320 * 200];
        private readonly byte[] worldMapPalette = new byte[768];
        private readonly NativeSim.CampaignMinimapEntity[] worldMapEntities = new NativeSim.CampaignMinimapEntity[1000];
        private readonly NativeSim.CampaignMinimapMark[] worldMapMarks = new NativeSim.CampaignMinimapMark[64];

        internal (byte[] indices, byte[] palette, NativeSim.CampaignMinimapState state,
            NativeSim.CampaignMinimapEntity[] entities, NativeSim.CampaignMinimapMark[] marks)
            WorldMap(ulong expectedTick)
        {
            var state = default(NativeSim.CampaignMinimapState);
            NativeSession.Check(NativeSim.mc_native_campaign_world_map_copy(
                Handle, generation, expectedTick, worldMapIndices, (uint)worldMapIndices.Length,
                worldMapPalette, (uint)worldMapPalette.Length,
                worldMapEntities, (uint)worldMapEntities.Length,
                worldMapMarks, (uint)worldMapMarks.Length, ref state, 72), "copy source split world map");
            ValidateMinimapState(state, generation, expectedTick);
            var entities = worldMapEntities.Take(checked((int)state.entityCount)).ToArray();
            var marks = worldMapMarks.Take(checked((int)state.markCount)).ToArray();
            ValidateMinimapEntities(entities, generation, expectedTick);
            ValidateMinimapMarks(marks, generation, expectedTick);
            return (worldMapIndices, worldMapPalette, state, entities, marks);
        }

        internal NativeSim.CampaignMinimapState MinimapState(ulong expectedTick)
        {
            var state = default(NativeSim.CampaignMinimapState);
            var status = NativeSim.mc_native_campaign_minimap_state(
                Handle,
                generation,
                expectedTick,
                ref state,
                72);
            NativeSession.Check(status, "read Level 2 minimap state");
            ValidateMinimapState(state, generation, expectedTick);
            return state;
        }

        private readonly byte[] minimapTerrainCoverage = new byte[64 * 64];

        internal byte[] MinimapTerrainCoverage(ulong expectedTick)
        {
            NativeSession.Check(NativeSim.mc_native_campaign_minimap_terrain_coverage_copy(
                Handle, generation, expectedTick, minimapTerrainCoverage,
                (uint)minimapTerrainCoverage.Length, out var length), "copy minimap terrain coverage");
            if (length != minimapTerrainCoverage.Length || minimapTerrainCoverage.Any(value => value > 1))
                throw new InvalidOperationException("minimap terrain coverage returned invalid bytes");
            return minimapTerrainCoverage;
        }

        internal NativeSim.CampaignMinimapEntity[] MinimapEntities(
            ulong expectedTick,
            NativeSim.CampaignMinimapState state)
        {
            ValidateMinimapState(state, generation, expectedTick);
            var count = checked((int)state.entityCount);
            var rows = new NativeSim.CampaignMinimapEntity[count];
            var status = NativeSim.mc_native_campaign_minimap_entities_copy(
                Handle,
                generation,
                expectedTick,
                0,
                rows,
                state.entityCount,
                56,
                out var written);
            NativeSession.Check(status, "read Level 2 minimap entities");
            if (written != state.entityCount)
            {
                throw new InvalidOperationException(
                    $"Level 2 minimap entity copy wrote {written}, expected {state.entityCount}");
            }
            ValidateMinimapEntities(rows, generation, expectedTick);
            return rows;
        }

        internal NativeSim.CampaignMinimapMark[] MinimapMarks(
            ulong expectedTick,
            NativeSim.CampaignMinimapState state)
        {
            ValidateMinimapState(state, generation, expectedTick);
            var count = checked((int)state.markCount);
            var rows = new NativeSim.CampaignMinimapMark[count];
            var status = NativeSim.mc_native_campaign_minimap_marks_copy(
                Handle,
                generation,
                expectedTick,
                0,
                rows,
                state.markCount,
                48,
                out var written);
            NativeSession.Check(status, "read Level 2 minimap marks");
            if (written != state.markCount)
            {
                throw new InvalidOperationException(
                    $"Level 2 minimap mark copy wrote {written}, expected {state.markCount}");
            }
            ValidateMinimapMarks(rows, generation, expectedTick);
            return rows;
        }

        internal static void ValidateMinimapState(
            NativeSim.CampaignMinimapState state,
            ulong expectedGeneration,
            ulong expectedTick)
        {
            if (
                state.abiVersion != NativeSim.AbiVersion ||
                state.structSize != 72 ||
                state.generation != expectedGeneration ||
                state.tick != expectedTick ||
                state.entityCount > 1000 ||
                state.markCount > 64 ||
                (state.visibilityFlags & ~0x1Fu) != 0 ||
                state.reserved0 != 0 ||
                !AllZero(state.reserved))
            {
                throw new InvalidOperationException(
                    "native Level 2 minimap state returned invalid ABI, generation/tick, " +
                    "counts, visibility, or reserved state");
            }
        }

        private static void ValidateMinimapEntities(
            NativeSim.CampaignMinimapEntity[] rows,
            ulong expectedGeneration,
            ulong expectedTick)
        {
            var previousOrder = -1;
            foreach (var row in rows)
            {
                if (
                    row.abiVersion != NativeSim.AbiVersion ||
                    row.structSize != 56 ||
                    row.generation != expectedGeneration ||
                    row.tick != expectedTick ||
                    row.sourceOrder <= previousOrder ||
                    row.subjectEntityIndex == 0 ||
                    (row.visible != 1 && !(row.visible == 0 && row.helpSelector == 27 && row.kind == 1)) ||
                    row.kind == 0 ||
                    row.size == 0 ||
                    !AllZero(row.reserved))
                {
                    throw new InvalidOperationException(
                        "native Level 2 minimap entity rows are not source-ordered " +
                        "qualified rows");
                }
                previousOrder = row.sourceOrder;
            }
        }

        internal static void ValidateMinimapMarks(
            NativeSim.CampaignMinimapMark[] rows,
            ulong expectedGeneration,
            ulong expectedTick)
        {
            var previousOrder = -1;
            var arrowRemaining = 0;
            byte arrowVisible = 0, arrowPalette = 0;
            foreach (var row in rows)
            {
                if (
                    row.abiVersion != NativeSim.AbiVersion ||
                    row.structSize != 48 ||
                    row.generation != expectedGeneration ||
                    row.tick != expectedTick ||
                    row.sourceOrder <= previousOrder ||
                    (row.subjectEntityIndex == 0 && row.kind != 2 && row.kind != 3) ||
                    row.visible > 1 ||
                    row.kind == 0 ||
                    !AllZero(row.reserved))
                {
                    throw new InvalidOperationException(
                        "native Level 2 minimap mark rows are not source-ordered " +
                        "qualified rows");
                }
                if (row.kind == 3)
                {
                    if (row.subjectEntityIndex != 0 || row.helpSelector != 0)
                        throw new InvalidOperationException("native map arrow must not impersonate an entity");
                    if (arrowRemaining == 0)
                    {
                        arrowRemaining = 7;
                        arrowVisible = row.visible;
                        arrowPalette = row.paletteIndex;
                    }
                    else if (row.sourceOrder != previousOrder + 1)
                        throw new InvalidOperationException("native map arrow vertex order is discontinuous");
                    if (row.visible != arrowVisible || row.paletteIndex != arrowPalette)
                        throw new InvalidOperationException("native map arrow vertices disagree");
                    arrowRemaining--;
                }
                else if (arrowRemaining != 0)
                    throw new InvalidOperationException("native map arrow is incomplete");
                previousOrder = row.sourceOrder;
            }
            if (arrowRemaining != 0)
                throw new InvalidOperationException("native map arrow is incomplete");
        }

        internal NativeSim.CampaignObjectiveStageSnapshot ObjectiveStageState(
            byte objectiveSlot = 0,
            byte stageSlot = 3)
        {
            var status = NativeSim.mc_native_campaign_objective_stage_state(
                Handle,
                generation,
                objectiveSlot,
                stageSlot,
                out var state,
                48);
            NativeSession.Check(status, "read Level 2 campaign objective stage state");
            ValidateObjectiveStageState(state, generation, objectiveSlot, stageSlot);
            return state;
        }

        internal static void ValidateObjectiveStageState(
            NativeSim.CampaignObjectiveStageSnapshot state,
            ulong expectedGeneration,
            byte expectedObjectiveSlot,
            byte expectedStageSlot)
        {
            if (
                state.abiVersion != NativeSim.AbiVersion ||
                state.structSize != 48 ||
                state.generation != expectedGeneration ||
                state.objectiveSlot != expectedObjectiveSlot ||
                state.stageSlot != expectedStageSlot ||
                state.relationDomain > 3 ||
                state.reserved0 != 0 ||
                !AllZero(state.reserved))
            {
                throw new InvalidOperationException(
                    "native Level 2 objective stage state returned invalid ABI, " +
                    "generation, slot, relation domain, or reserved state");
            }
        }

        internal byte[] TerrainPlane(uint kind)
        {
            if (kind == NativeSim.TerrainEntityIndex)
            {
                throw new ArgumentException(
                    "signed entity-index terrain plane requires TerrainEntityIndices()",
                    nameof(kind));
            }
            var status = NativeSim.mc_native_campaign_terrain_plane_length(
                Handle,
                generation,
                kind,
                out var count);
            NativeSession.Check(status, "read Level 2 campaign terrain plane length");
            var values = new byte[checked((int)count)];
            status = NativeSim.mc_native_campaign_terrain_plane_copy_u8(
                Handle,
                generation,
                kind,
                values,
                count,
                out var written);
            NativeSession.Check(status, "read Level 2 campaign terrain plane");
            if (written != count)
            {
                throw new InvalidOperationException(
                    $"Level 2 terrain plane copy wrote {written}, expected {count}");
            }
            return values;
        }

        internal short[] TerrainEntityIndices()
        {
            var status = NativeSim.mc_native_campaign_terrain_plane_length(
                Handle,
                generation,
                NativeSim.TerrainEntityIndex,
                out var count);
            NativeSession.Check(status, "read Level 2 entity-index terrain length");
            var values = new short[checked((int)count)];
            status = NativeSim.mc_native_campaign_terrain_entity_indices_copy(
                Handle,
                generation,
                values,
                count,
                out var written);
            NativeSession.Check(status, "read Level 2 entity-index terrain plane");
            if (written != count)
            {
                throw new InvalidOperationException(
                    $"Level 2 entity-index terrain copy wrote {written}, expected {count}");
            }
            return values;
        }

        internal NativeSim.CampaignShieldSnapshot ShieldState()
        {
            var status = NativeSim.mc_native_campaign_shield_state(
                Handle,
                generation,
                out var state,
                72);
            NativeSession.Check(status, "read Level 2 Shield state");
            return state;
        }

        internal NativeSim.CampaignShieldSnapshot ActivateShield(
            ushort directionMask = 256,
            bool helpEnabled = false)
        {
            var status = NativeSim.mc_native_campaign_shield_activate(
                Handle,
                generation,
                directionMask,
                helpEnabled ? (byte)1 : (byte)0,
                out var state,
                72);
            NativeSession.Check(status, "activate Level 2 Shield");
            return state;
        }

        internal void SubmitShieldDamage(
            ushort targetEntityIndex,
            int damage,
            ushort attackerEntityIndex)
        {
            var status = NativeSim.mc_native_campaign_shield_damage_submit(
                Handle,
                generation,
                targetEntityIndex,
                damage,
                attackerEntityIndex);
            NativeSession.Check(status, "submit Level 2 Shield damage");
        }

        internal NativeSim.CampaignInvisibleSnapshot InvisibleState()
        {
            var status = NativeSim.mc_native_campaign_invisible_state(
                Handle,
                generation,
                out var state,
                80);
            NativeSession.Check(status, "read Level 2 Invisible state");
            return state;
        }

        internal NativeSim.CampaignInvisibleSnapshot ActivateInvisible(
            ushort directionMask = 256,
            bool helpEnabled = false)
        {
            var status = NativeSim.mc_native_campaign_invisible_activate(
                Handle,
                generation,
                directionMask,
                helpEnabled ? (byte)1 : (byte)0,
                out var state,
                80);
            NativeSession.Check(status, "activate Level 2 Invisible");
            return state;
        }

        internal NativeSim.CampaignBeyondSightSnapshot BeyondSightState()
        {
            var status = NativeSim.mc_native_campaign_beyond_sight_state(
                Handle,
                generation,
                out var state,
                80);
            NativeSession.Check(status, "read Level 2 Beyond Sight state");
            return state;
        }

        internal NativeSim.CampaignBeyondSightSnapshot ActivateBeyondSight(
            ushort directionMask = 256,
            bool helpEnabled = false)
        {
            var status = NativeSim.mc_native_campaign_beyond_sight_activate(
                Handle,
                generation,
                directionMask,
                helpEnabled ? (byte)1 : (byte)0,
                out var state,
                80);
            NativeSession.Check(status, "activate Level 2 Beyond Sight");
            return state;
        }

        internal NativeSim.CampaignOpponentSnapshot OpponentState()
        {
            var status = NativeSim.mc_native_campaign_opponent_state(
                Handle,
                generation,
                out var state,
                104);
            NativeSession.Check(status, "read Level 2 opponent state");
            return state;
        }

        internal NativeSim.CampaignPlayerSnapshot[] Players()
        {
            var status = NativeSim.mc_native_campaign_player_count(
                Handle,
                out var count);
            NativeSession.Check(status, "read campaign player count");
            var players = new NativeSim.CampaignPlayerSnapshot[checked((int)count)];
            status = NativeSim.mc_native_campaign_players_copy(
                Handle,
                0,
                players,
                count,
                48,
                out var written);
            NativeSession.Check(status, "read campaign players");
            if (written != count)
            {
                throw new InvalidOperationException(
                    $"campaign player copy wrote {written}, expected {count}");
            }
            return players;
        }

        internal NativeSim.CampaignEntitySnapshot[] Entities()
        {
            var status = NativeSim.mc_native_campaign_entity_count(
                Handle,
                out var count);
            NativeSession.Check(status, "read campaign entity count");
            var entities = new NativeSim.CampaignEntitySnapshot[checked((int)count)];
            status = NativeSim.mc_native_campaign_entities_copy(
                Handle,
                0,
                entities,
                count,
                52,
                out var written);
            NativeSession.Check(status, "read campaign entities");
            if (written != count)
            {
                throw new InvalidOperationException(
                    $"campaign entity copy wrote {written}, expected {count}");
            }
            return entities;
        }

        internal NativeSim.CampaignEntityPresentation[] EntityPresentations(short cameraYaw, bool worldView = false)
        {
            uint written;
            var status = worldView
                ? NativeSim.mc_native_campaign_world_entity_presentations_copy(Handle,generation,tick,cameraYaw,
                    entityPresentationBuffer,checked((uint)entityPresentationBuffer.Length),64,out written)
                : NativeSim.mc_native_campaign_entity_presentations_copy(Handle,generation,tick,cameraYaw,
                    entityPresentationBuffer,checked((uint)entityPresentationBuffer.Length),64,out written);
            NativeSession.Check(status, "read campaign entity presentation");
            if (written > entityPresentationBuffer.Length)
                throw new InvalidOperationException("Campaign entity presentation batch exceeds capacity");
            var rows = new NativeSim.CampaignEntityPresentation[checked((int)written)];
            Array.Copy(entityPresentationBuffer, rows, rows.Length);
            entityPresentationSlots.Clear();
            foreach (var row in rows)
            {
                if (row.abiVersion != NativeSim.AbiVersion || row.structSize != 64 ||
                    row.generation != generation || row.tick != tick ||
                    !entityPresentationSlots.Add(row.entityIndex) || row.selectedTmap < row.baseTmap ||
                    row.orientationSector > 15 || row.mirrored > 1 ||
                    row.reserved0 != 0 || row.reserved != 0 ||
                    row.pixelMode > 7 || row.brightness > 0x2000)
                    throw new InvalidOperationException(
                        "campaign entity presentation identity is invalid");
            }
            return rows;
        }

        // The caller applies deaths before CharacterStates removes/reuses a slot.
        // Advance each cursor only after its callback returns successfully.
        internal void DrainVoxelDeaths(Action<NativeSim.CampaignVoxelDeath> apply)
        {
            if (apply == null) throw new ArgumentNullException(nameof(apply));
            if (voxelDeathGeneration != VoxelWorldEpoch)
            {
                voxelDeathGeneration = VoxelWorldEpoch;
                voxelDeathCursor = 0;
            }
            var status = NativeSim.mc_native_campaign_voxel_deaths_copy(Handle, generation, tick,
                voxelDeathCursor, voxelDeathBuffer, (uint)voxelDeathBuffer.Length, 32, out var written);
            NativeSession.Check(status, "read voxel death journal");
            if (written > voxelDeathBuffer.Length) throw new InvalidOperationException("Voxel death count exceeds buffer");
            for (var i = 0; i < written; i++)
            {
                var row = voxelDeathBuffer[i];
                if (row.abiVersion != NativeSim.AbiVersion || row.structSize != 32 || row.generation != VoxelWorldEpoch ||
                    row.sequence <= voxelDeathCursor || (voxelDeathCursor != 0 && row.sequence != voxelDeathCursor + 1))
                    throw new InvalidOperationException("Invalid or noncontiguous voxel death row");
                apply(row);
                voxelDeathCursor = row.sequence;
            }
        }

        internal NativeSim.CampaignCharacterState[] CharacterStates()
        {
            if(cachedCharacterStates!=null && characterStateGeneration==generation && characterStateTick==tick)return cachedCharacterStates;
            var rows = new NativeSim.CampaignCharacterState[1000];
            var status = NativeSim.mc_native_campaign_character_states_copy_v2(Handle, generation, tick,
                rows, (uint)rows.Length, 96, out var written);
            NativeSession.Check(status, "read live character animation state");
            if (written > rows.Length) throw new InvalidOperationException("Character batch exceeds capacity");
            Array.Resize(ref rows, checked((int)written));
            var seen = new HashSet<ushort>();
            foreach (var row in rows)
                if (row.abiVersion != NativeSim.AbiVersion || row.structSize != 96 ||
                    row.generation != generation || row.tick != tick || !seen.Add(row.entityIndex) ||
                    row.stance > 1 || row.active > 1 || row.hasTarget > 1 || row.hasParent > 1 ||
                    !HasSupportedCharacterIdentity(row))
                    throw new InvalidOperationException("Invalid source-backed character state");
            cachedCharacterStates=rows;characterStateGeneration=generation;characterStateTick=tick;
            return rows;
        }

        internal ArraySegment<NativeSim.CampaignCastleChild> CastleChildren()
        {
            if (castleChildGeneration == generation && castleChildTick == tick)
                return new ArraySegment<NativeSim.CampaignCastleChild>(castleChildBuffer, 0, castleChildCount);
            var status = NativeSim.mc_native_campaign_castle_children_copy_v1(
                Handle, generation, tick, castleChildBuffer, (uint)castleChildBuffer.Length, 96, out var written);
            NativeSession.Check(status, "read live Castle balloon state");
            if (written > castleChildBuffer.Length) throw new InvalidOperationException("Castle balloon batch exceeds capacity");
            castleChildSeen.Clear();
            for (var i = 0; i < written; i++)
            {
                var row = castleChildBuffer[i];
                if (row.abiVersion != NativeSim.AbiVersion || row.structSize != 96 ||
                    row.generation != generation || row.tick != tick || !castleChildSeen.Add(row.entityIndex) ||
                    row.classId != 3 || row.model != 3 || row.action != 9 || row.active != 1 ||
                    row.drawing > 1 || row.relationFlags > 7 ||
                    (row.relationFlags & 1) != 0 && row.ownerPlayerColor > 7 ||
                    (row.relationFlags & 2) != 0 && row.castleIndex == 0 ||
                    (row.relationFlags & 4) != 0 && row.targetIndex == 0 ||
                    row.renderParameter == 0 || row.renderHeight == 0 || row.reserved0 != 0)
                    throw new InvalidOperationException("Invalid source-backed Castle balloon state");
            }
            castleChildCount = checked((int)written);
            castleChildGeneration = generation; castleChildTick = tick;
            return new ArraySegment<NativeSim.CampaignCastleChild>(castleChildBuffer, 0, castleChildCount);
        }

        internal static bool HasSupportedCharacterIdentity(NativeSim.CampaignCharacterState row)
        {
            if (row.lifecycleFlags > 1 || row.lifecycleFlags == 1 &&
                (row.classId != 5 || row.model != 13 || row.action != 108 || row.life < 0)) return false;
            if (row.classId == 5 && row.model == 19)
                return row.renderParameter == 287 && row.sourceHalfWidth == 0 && row.renderHeight == 200 &&
                    row.hasParent == 0 && row.partIndex == 0;
            if (row.classId == 5 && row.model == 3)
                return row.sourceHalfWidth > 0 && row.renderHeight > 0 &&
                    (row.renderParameter == 88 && row.hasParent == 0 && row.partIndex == 0 ||
                     row.renderParameter >= 89 && row.renderParameter <= 104 &&
                     row.partIndex == row.renderParameter - 89 && (row.hasParent == 0 || row.parentIndex != 0));
            if (row.classId == 5 && (row.model == 12 || row.model == 13 || row.model == 14))
                return ModernVillagerAppearances.TryGet(row.renderParameter, out var binding) && binding.Model == row.model;
            return row.renderParameter == 0 &&
                (row.classId == 5 && (row.model == 1 || row.model == 4) ||
                 row.classId == 9 && (row.model == 0 || row.model == 13 || row.model == 28));
        }

        internal NativeSim.CampaignGoatSnapshot[] Goats()
        {
            var goats = new NativeSim.CampaignGoatSnapshot[36];
            var status = NativeSim.mc_native_campaign_goats_copy(
                Handle,
                generation,
                0,
                goats,
                checked((uint)goats.Length),
                84,
                out var batch,
                64);
            NativeSession.Check(status, "read Level 2 Goats");
            if (
                batch.abiVersion != NativeSim.AbiVersion ||
                batch.structSize != 64 ||
                batch.generation != generation ||
                batch.startOrdinal != 0 ||
                batch.written != goats.Length ||
                batch.totalCount != goats.Length ||
                batch.hasMore != 0)
            {
                throw new InvalidOperationException("Level 2 Goat batch metadata is inconsistent");
            }
            return goats;
        }

        internal void SubmitGoatDamage(
            ushort targetEntityIndex,
            int damage,
            ushort attackerEntityIndex)
        {
            var status = NativeSim.mc_native_campaign_goat_damage_submit(
                Handle,
                generation,
                targetEntityIndex,
                damage,
                attackerEntityIndex);
            NativeSession.Check(status, "submit Level 2 Goat damage");
        }

        internal NativeSim.CampaignSpellPickupSnapshot[] SpellPickups()
        {
            var rows = new List<NativeSim.CampaignSpellPickupSnapshot>();
            uint start = 0;
            uint? total = null;
            ulong? tick = null;
            ulong? liveWorldHash = null;
            while (true)
            {
                var page = new NativeSim.CampaignSpellPickupSnapshot[64];
                var status = NativeSim.mc_native_campaign_spell_pickups_copy(
                    Handle,
                    generation,
                    start,
                    page,
                    checked((uint)page.Length),
                    64,
                    out var batch,
                    64);
                NativeSession.Check(status, "read Level 2 spell pickups");
                if (
                    batch.abiVersion != NativeSim.AbiVersion ||
                    batch.structSize != 64 ||
                    batch.generation != generation ||
                    batch.startOrdinal != start ||
                    batch.written > (uint)page.Length ||
                    (total.HasValue && batch.totalCount != total.Value) ||
                    (tick.HasValue && batch.tick != tick.Value) ||
                    (liveWorldHash.HasValue && batch.liveWorldHash != liveWorldHash.Value))
                {
                    throw new InvalidOperationException(
                        "Level 2 spell-pickup batch metadata is inconsistent");
                }
                total ??= batch.totalCount;
                tick ??= batch.tick;
                liveWorldHash ??= batch.liveWorldHash;
                rows.AddRange(page.Take(checked((int)batch.written)));
                if (batch.hasMore == 0)
                {
                    if ((uint)rows.Count != batch.totalCount)
                    {
                        throw new InvalidOperationException(
                            "Level 2 spell-pickup batch ended before total count");
                    }
                    return rows.ToArray();
                }
                if (batch.written == 0)
                {
                    throw new InvalidOperationException(
                        "Level 2 spell-pickup batch made no progress");
                }
                start = checked(start + batch.written);
            }
        }

        internal NativeSim.ManaSphereSnapshot[] ManaSpheres()
        {
            var rows = new List<NativeSim.ManaSphereSnapshot>();
            uint start = 0;
            uint? total = null;
            ulong? tick = null;
            ulong? liveWorldHash = null;
            while (true)
            {
                var page = new NativeSim.ManaSphereSnapshot[64];
                var status = NativeSim.mc_native_campaign_mana_spheres_copy(
                    Handle,
                    generation,
                    start,
                    page,
                    checked((uint)page.Length),
                    64,
                    out var batch,
                    64);
                NativeSession.Check(status, "read Level 2 mana spheres");
                if (
                    batch.abiVersion != NativeSim.AbiVersion ||
                    batch.structSize != 64 ||
                    batch.generation != generation ||
                    batch.startOrdinal != start ||
                    batch.written > (uint)page.Length ||
                    (total.HasValue && batch.totalCount != total.Value) ||
                    (tick.HasValue && batch.tick != tick.Value) ||
                    (liveWorldHash.HasValue && batch.liveWorldHash != liveWorldHash.Value))
                {
                    throw new InvalidOperationException(
                        "Level 2 mana-sphere batch metadata is inconsistent");
                }
                total ??= batch.totalCount;
                tick ??= batch.tick;
                liveWorldHash ??= batch.liveWorldHash;
                rows.AddRange(page.Take(checked((int)batch.written)));
                if (batch.hasMore == 0)
                {
                    if ((uint)rows.Count != batch.totalCount)
                    {
                        throw new InvalidOperationException(
                            "Level 2 mana-sphere batch ended before total count");
                    }
                    return rows.ToArray();
                }
                if (batch.written == 0)
                {
                    throw new InvalidOperationException(
                        "Level 2 mana-sphere batch made no progress");
                }
                start = checked(start + batch.written);
            }
        }

        internal NativeSim.ManaCarrierSnapshot[] ManaCarriers()
        {
            var rows = new List<NativeSim.ManaCarrierSnapshot>();
            uint start = 0;
            uint? total = null;
            ulong? tick = null;
            ulong? liveWorldHash = null;
            while (true)
            {
                var page = new NativeSim.ManaCarrierSnapshot[64];
                var status = NativeSim.mc_native_campaign_mana_carriers_copy(
                    Handle,
                    generation,
                    start,
                    page,
                    checked((uint)page.Length),
                    80,
                    out var batch,
                    64);
                NativeSession.Check(status, "read Level 2 mana carriers");
                if (
                    batch.abiVersion != NativeSim.AbiVersion ||
                    batch.structSize != 64 ||
                    batch.generation != generation ||
                    batch.startOrdinal != start ||
                    batch.written > (uint)page.Length ||
                    (total.HasValue && batch.totalCount != total.Value) ||
                    (tick.HasValue && batch.tick != tick.Value) ||
                    (liveWorldHash.HasValue && batch.liveWorldHash != liveWorldHash.Value))
                {
                    throw new InvalidOperationException(
                        "Level 2 mana-carrier batch metadata is inconsistent");
                }
                total ??= batch.totalCount;
                tick ??= batch.tick;
                liveWorldHash ??= batch.liveWorldHash;
                rows.AddRange(page.Take(checked((int)batch.written)));
                if (batch.hasMore == 0)
                {
                    if ((uint)rows.Count != batch.totalCount)
                    {
                        throw new InvalidOperationException(
                            "Level 2 mana-carrier batch ended before total count");
                    }
                    return rows.ToArray();
                }
                if (batch.written == 0)
                {
                    throw new InvalidOperationException(
                        "Level 2 mana-carrier batch made no progress");
                }
                start = checked(start + batch.written);
            }
        }

        internal (NativeSim.CampaignStepSnapshot snapshot, NativeSim.CampaignEvent[] events)
            Step(
                ushort playerX,
                ushort playerY,
                short playerZ,
                byte playerSlot = 0,
                byte speedIndex = 0,
                sbyte flags = 1,
                short requestedSpell = NativeSim.NoRequestedSpell,
                sbyte roll = 0,
                sbyte pitch = 0,
                sbyte opcode = 0,
                sbyte argument1 = 0,
                sbyte argument2 = 0)
        {
            var command = NativeSim.Command.Create(
                roll,
                pitch,
                flags,
                opcode: opcode,
                argument1: argument1,
                argument2: argument2,
                requestedSpell: requestedSpell);
            var input = NativeSim.CampaignStepInput.Create(
                generation,
                command,
                playerSlot,
                speedIndex,
                playerX,
                playerY,
                playerZ);
            var events = new NativeSim.CampaignEvent[1024];
            var status = NativeSim.mc_native_campaign_session_step(
                Handle,
                ref input,
                out var snapshot,
                112,
                events,
                checked((uint)events.Length),
                48,
                out var eventCount);
            NativeSession.Check(status, "step campaign session");
            if (eventCount > (uint)events.Length)
            {
                throw new InvalidOperationException(
                    $"campaign event count {eventCount} exceeds buffer {events.Length}");
            }
            if (snapshot.abiVersion != NativeSim.AbiVersion ||
                snapshot.structSize != 112 ||
                snapshot.generation != generation ||
                snapshot.revision != checked(tick + 1) ||
                snapshot.stateHash == 0 ||
                snapshot.terrainHash == 0 ||
                snapshot.eventCount != eventCount ||
                snapshot.playerSlot != playerSlot ||
                snapshot.reserved1 != 0 ||
                !AllZero(snapshot.reserved0) ||
                !AllZero(snapshot.reserved) ||
                AllZero(snapshot.dispatchHash))
            {
                throw new InvalidOperationException(
                    "native campaign step returned an invalid generic snapshot");
            }
            generation = snapshot.generation;
            tick = snapshot.revision;
            return (snapshot, events.Take(checked((int)eventCount)).ToArray());
        }

        internal NativeSim.CampaignDayMusicSnapshot DayMusicState()
        {
            var state = default(NativeSim.CampaignDayMusicSnapshot);
            var status = NativeSim.mc_native_campaign_day_music_state(
                Handle,
                generation,
                tick,
                ref state,
                80);
            NativeSession.Check(status, "read Level 2 Day music state");
            ValidateDayMusicState(state, generation, tick);
            return state;
        }

        internal NativeSim.CampaignDayMusicSnapshot AdvanceDayMusic(uint callbackCount)
        {
            var state = default(NativeSim.CampaignDayMusicSnapshot);
            var status = NativeSim.mc_native_campaign_day_music_advance(
                Handle,
                generation,
                tick,
                callbackCount,
                ref state,
                80);
            NativeSession.Check(status, "advance Level 2 Day music fade");
            ValidateDayMusicState(state, generation, tick);
            return state;
        }

        internal NativeSim.CampaignDayMusicSnapshot StopDayMusic()
        {
            var state = default(NativeSim.CampaignDayMusicSnapshot);
            var status = NativeSim.mc_native_campaign_day_music_stop(
                Handle,
                generation,
                tick,
                ref state,
                80);
            NativeSession.Check(status, "stop Level 2 Day music");
            ValidateDayMusicState(state, generation, tick);
            return state;
        }

        internal NativeSim.CampaignDayMusicSnapshot EndDayMusic()
        {
            var state = default(NativeSim.CampaignDayMusicSnapshot);
            var status = NativeSim.mc_native_campaign_day_music_end(
                Handle,
                generation,
                tick,
                ref state,
                80);
            NativeSession.Check(status, "end Level 2 Day music");
            ValidateDayMusicState(state, generation, tick);
            return state;
        }

        internal NativeSim.CampaignEconomyEvent[] EconomyEvents()
        {
            var events = new NativeSim.CampaignEconomyEvent[1024];
            var status = NativeSim.mc_native_campaign_economy_events(
                Handle,
                events,
                checked((uint)events.Length),
                64,
                out var eventCount);
            NativeSession.Check(status, "read Level 2 economy events");
            if (eventCount > (uint)events.Length)
            {
                throw new InvalidOperationException(
                    $"campaign economy event count {eventCount} exceeds buffer {events.Length}");
            }
            return events.Take(checked((int)eventCount)).ToArray();
        }

        internal NativeSim.CampaignAuthoredGroupEvent[] AuthoredGroupEvents()
        {
            var events = new NativeSim.CampaignAuthoredGroupEvent[4096];
            var status = NativeSim.mc_native_campaign_authored_group_events(
                Handle,
                events,
                checked((uint)events.Length),
                48,
                out var eventCount);
            NativeSession.Check(status, "read Level 2 authored-group events");
            if (eventCount > (uint)events.Length)
            {
                throw new InvalidOperationException(
                    $"campaign authored-group event count {eventCount} exceeds buffer {events.Length}");
            }
            return events.Take(checked((int)eventCount)).ToArray();
        }

        internal NativeSim.CampaignGoatEvent[] GoatEvents()
        {
            var events = new NativeSim.CampaignGoatEvent[1024];
            var status = NativeSim.mc_native_campaign_goat_events(
                Handle,
                events,
                checked((uint)events.Length),
                64,
                out var eventCount);
            NativeSession.Check(status, "read Level 2 Goat events");
            if (eventCount > (uint)events.Length)
            {
                throw new InvalidOperationException(
                    $"campaign Goat event count {eventCount} exceeds buffer {events.Length}");
            }
            return events.Take(checked((int)eventCount)).ToArray();
        }

        internal void Reset()
        {
            var status = NativeSim.mc_native_campaign_session_reset(Handle);
            NativeSession.Check(status, "reset campaign session");
            generation++;
            tick = 0;
            effectCursor = 0;
            campaignFrameInfo = default;
            Array.Clear(campaignFrameIndices, 0, campaignFrameIndices.Length);
            Array.Clear(
                campaignFrameScratchIndices,
                0,
                campaignFrameScratchIndices.Length);
            Array.Clear(
                dayCompatibilityFrameIndices,
                0,
                dayCompatibilityFrameIndices.Length);
            Array.Clear(dayIndexedFrameIndices, 0, dayIndexedFrameIndices.Length);
            Array.Clear(dayIndexedFramePalette, 0, dayIndexedFramePalette.Length);
        }

        public void Dispose()
        {
            var value = Interlocked.Exchange(ref handleBits, 0);
            if (value == 0)
            {
                return;
            }
            if (!ownsHandle)
            {
                return;
            }
            if (dayMusicAvailable)
            {
                var endState = default(NativeSim.CampaignDayMusicSnapshot);
                var endStatus = NativeSim.mc_native_campaign_day_music_end(
                    unchecked((ulong)value),
                    generation,
                    tick,
                    ref endState,
                    80);
                if (endStatus != NativeSim.Ok)
                {
                    UnityEngine.Debug.LogError(
                        $"end campaign Day music failed: {endStatus}: " +
                        NativeSession.ReadLastError());
                }
            }
            var status = NativeSim.mc_native_campaign_session_destroy(
                unchecked((ulong)value));
            if (status != NativeSim.Ok)
            {
                Interlocked.CompareExchange(ref handleBits, value, 0);
                UnityEngine.Debug.LogError(
                    $"destroy campaign session failed: {status}: " +
                    NativeSession.ReadLastError());
                return;
            }
            Interlocked.Decrement(ref activeSessionCount);
        }

        private void ValidateDayMusicState(
            NativeSim.CampaignDayMusicSnapshot state,
            ulong expectedGeneration,
            ulong expectedTick)
        {
            var expectedTrack = Selection.mapType switch
            {
                0 => 2, // Day
                1 => 1, // Night
                2 => 3, // Cave
                _ => throw new InvalidDataException("Unknown campaign music map type"),
            };
            if (state.abiVersion != NativeSim.AbiVersion ||
                state.structSize != 80 ||
                state.generation != expectedGeneration ||
                state.tick != expectedTick ||
                state.assetSlot != 0 ||
                state.track != expectedTrack ||
                state.volume != 127 ||
                state.initialized > 1 ||
                state.loaded > 1 ||
                state.active > 1 ||
                state.playing > 1 ||
                state.updatesEnabled > 1 ||
                state.timerActive > 1 ||
                (state.targetMode != 1 && state.targetMode != 2) ||
                (state.direction != -1 && state.direction != 1) ||
                state.currentUnits > 127 ||
                state.timerHz !=
                    ((state.timerMultiplier >= 1 && state.timerMultiplier <= 4)
                        ? 30 * state.timerMultiplier
                        : 30) ||
                state.stateHash == 0 ||
                !AllZero(state.reserved) ||
                state.reserved0 != 0)
            {
                throw new InvalidDataException(
                    "Level 2 Day music state violates the native lifecycle contract");
            }
        }

        private static bool AllZero(uint[] values)
        {
            return values != null && values.All(value => value == 0);
        }

        private static bool AllZero(byte[] values)
        {
            return values != null && values.All(value => value == 0);
        }
    }
}
