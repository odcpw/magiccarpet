using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
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

        internal ulong EffectCursor => effectCursor;

        internal CampaignPackageIdentity PackageIdentity => packageIdentity;

        internal NativeSim.CampaignSelection Selection => packageIdentity.Selection;

        internal NativeSim.CampaignWorldInfo InitialWorldInfo => initialWorldInfo;

        internal NativeSim.PresentationFrameInfo CampaignFrameInfo => campaignFrameInfo;

        internal byte[] CampaignFrameIndices => campaignFrameIndices;

        internal byte[] DayIndexedFrameIndices => dayIndexedFrameIndices;

        internal byte[] DayIndexedFramePalette => dayIndexedFramePalette;

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
            if (!IsPresentationSelection(selection))
            {
                throw new InvalidDataException(
                    "campaign presentation borrow rejects an unknown profile");
            }
            var packageIdentity = ReadPackageIdentity(packagePath, selection);
            var status = NativeSim.mc_native_campaign_world_info(
                handle, out var worldInfo, 64);
            NativeSession.Check(status, "read borrowed campaign world identity");
            ValidateWorldInfo(worldInfo, selection);
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
            if (envelope?.identity == null ||
                envelope.identity.external_level != selection.externalLevel ||
                envelope.identity.internal_level != selection.internalLevel ||
                envelope.identity.map_type_id != selection.mapType)
            {
                throw new InvalidDataException(
                    "campaign package definition does not match the requested selection");
            }
            return new CampaignPackageIdentity(
                package,
                Sha256(manifest),
                Sha256(definition),
                selection);
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

        private static bool IsPresentationSelection(
            NativeSim.CampaignSelection selection)
        {
            return selection.mapType == 0 &&
                   (selection.externalLevel == 2 && selection.internalLevel == 1 ||
                    selection.externalLevel == 3 && selection.internalLevel == 2);
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
                 effect.kind != NativeSim.CampaignEffectSceneChange) ||
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
            ValidateCampaignFrameInfo(info, generation, expectedTick);
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
            ulong expectedTick)
        {
            var expectedFlags =
                NativeSim.TerrainFrameIsolatedSkyTerrain |
                NativeSim.TerrainFrameSkySeeded |
                NativeSim.TerrainFrameNonTerrainOmitted;
            if (info.abiVersion != NativeSim.AbiVersion ||
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
            if (externalLevel < 1 || externalLevel > 3)
                throw new InvalidOperationException("campaign Day HUD profile is unknown");
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
            if (slot.drawAdmitted > 1 || !AllZero(slot.reserved))
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
                    slot.drawAdmitted != 0)
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
                    row.visible != 1 ||
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
            foreach (var row in rows)
            {
                if (
                    row.abiVersion != NativeSim.AbiVersion ||
                    row.structSize != 48 ||
                    row.generation != expectedGeneration ||
                    row.tick != expectedTick ||
                    row.sourceOrder <= previousOrder ||
                    (row.subjectEntityIndex == 0 && row.kind != 2) ||
                    row.visible > 1 ||
                    row.kind == 0 ||
                    !AllZero(row.reserved))
                {
                    throw new InvalidOperationException(
                        "native Level 2 minimap mark rows are not source-ordered " +
                        "qualified rows");
                }
                previousOrder = row.sourceOrder;
            }
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

        internal NativeSim.CampaignEntityPresentation[] EntityPresentations(short cameraYaw)
        {
            var rows = new NativeSim.CampaignEntityPresentation[1000];
            var status = NativeSim.mc_native_campaign_entity_presentations_copy(
                Handle,
                generation,
                tick,
                cameraYaw,
                rows,
                checked((uint)rows.Length),
                64,
                out var written);
            NativeSession.Check(status, "read campaign entity presentation");
            Array.Resize(ref rows, checked((int)written));
            var seen = new HashSet<ushort>();
            foreach (var row in rows)
            {
                if (row.abiVersion != NativeSim.AbiVersion || row.structSize != 64 ||
                    row.generation != generation || row.tick != tick ||
                    !seen.Add(row.entityIndex) || row.selectedTmap < row.baseTmap ||
                    row.orientationSector > 15 || row.mirrored > 1 ||
                    row.reserved0 != 0 || !AllZero(row.reserved))
                    throw new InvalidOperationException(
                        "campaign entity presentation identity is invalid");
            }
            return rows;
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

        private static void ValidateDayMusicState(
            NativeSim.CampaignDayMusicSnapshot state,
            ulong expectedGeneration,
            ulong expectedTick)
        {
            if (state.abiVersion != NativeSim.AbiVersion ||
                state.structSize != 80 ||
                state.generation != expectedGeneration ||
                state.tick != expectedTick ||
                state.assetSlot != 0 ||
                state.track != 2 ||
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
