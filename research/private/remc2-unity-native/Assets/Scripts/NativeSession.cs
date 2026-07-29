using System;
using System.Collections.Generic;
using System.Security.Cryptography;
using System.Text;
using System.Threading;
#if UNITY_EDITOR
using UnityEditor;
#endif

namespace MagicCarpet.Native
{
    internal sealed class NativeSession : IDisposable
    {
        private static readonly object RegistryLock = new();
        private static readonly HashSet<NativeSession> Sessions = new();

        private long handleBits;
        private readonly NativeSim.TerrainGridVertex[] terrainGrid;
        private readonly NativeSim.GoatSnapshot[] goats;
        private readonly NativeSim.SelectorFrameState[] selectorFrames;
        private NativeSim.TerrainFrameQuery terrainFrameQuery;

        static NativeSession()
        {
#if UNITY_EDITOR
            AssemblyReloadEvents.beforeAssemblyReload += DisposeAll;
#endif
        }

        private NativeSession(ulong handle)
        {
            handleBits = unchecked((long)handle);
            terrainGrid = new NativeSim.TerrainGridVertex[
                checked((int)NativeSim.mc_native_terrain_grid_vertex_count())];
            goats = new NativeSim.GoatSnapshot[33];
            selectorFrames = new NativeSim.SelectorFrameState[3];
            terrainFrameQuery = NativeSim.TerrainFrameQuery.Create(0, 0);
            lock (RegistryLock)
            {
                Sessions.Add(this);
            }
        }

        internal ulong Handle
        {
            get
            {
                var value = Volatile.Read(ref handleBits);
                if (value == 0)
                {
                    throw new ObjectDisposedException(nameof(NativeSession));
                }
                return unchecked((ulong)value);
            }
        }

        internal static int ActiveSessionCount
        {
            get
            {
                lock (RegistryLock)
                {
                    return Sessions.Count;
                }
            }
        }

        internal static NativeSession Open(string packagePath, string closurePath)
        {
            NativeSim.AssertLayouts();
            if (NativeSim.mc_native_abi_version() != NativeSim.AbiVersion)
            {
                throw new InvalidOperationException("sim_capi native ABI is not version 2");
            }
            var package = Encoding.UTF8.GetBytes(packagePath);
            var closure = Encoding.UTF8.GetBytes(closurePath);
            var status = NativeSim.mc_native_session_open(
                package,
                checked((uint)package.Length),
                closure,
                checked((uint)closure.Length),
                out var handle);
            Check(status, "open native session");
            if (handle == 0)
            {
                throw new InvalidOperationException("native session returned handle zero");
            }
            return new NativeSession(handle);
        }

        internal NativeSim.WorldInfo WorldInfo()
        {
            var status = NativeSim.mc_native_world_info(Handle, out var info, 72);
            Check(status, "read world info");
            return info;
        }

        internal NativeSim.SessionSnapshot Snapshot()
        {
            var status = NativeSim.mc_native_session_snapshot(Handle, out var snapshot, 120);
            Check(status, "read session snapshot");
            return snapshot;
        }

        internal ulong SourceArenaHash()
        {
            var status = NativeSim.mc_native_session_source_arena_hash(
                Handle,
                out var hash);
            Check(status, "read native source-arena hash");
            return hash;
        }

        internal ulong SourceProjectionHash()
        {
            var status = NativeSim.mc_native_session_source_projection_hash(
                Handle,
                out var hash);
            Check(status, "read source projection hash");
            return hash;
        }

        internal NativeSim.PresentationState PresentationState()
        {
            var status = NativeSim.mc_native_presentation_state(Handle, out var state, 64);
            Check(status, "read presentation state");
            return state;
        }

        internal NativeSim.RenderCameraState RenderCameraState()
        {
            var status = NativeSim.mc_native_render_camera_state(Handle, out var state, 72);
            Check(status, "read render camera state");
            return state;
        }

        internal uint TerrainPnt5VectorCount()
        {
            return NativeSim.mc_native_terrain_pnt5_vector_count();
        }

        internal NativeSim.TerrainPnt5Vector TerrainPnt5VectorAt(uint index)
        {
            var status = NativeSim.mc_native_terrain_pnt5_vector(
                index, out var vector, 56);
            Check(status, $"read terrain pnt5 vector {index}");
            return vector;
        }

        internal NativeSim.TerrainGridVertex[] TerrainGrid()
        {
            var count = NativeSim.mc_native_terrain_grid_vertex_count();
            if (terrainGrid.Length != count)
            {
                throw new InvalidOperationException(
                    $"native terrain grid count changed to {count}, expected " +
                    terrainGrid.Length);
            }
            var status = NativeSim.mc_native_session_terrain_grid_copy(
                Handle, terrainGrid, count, 64, out var written);
            Check(status, "read terrain grid");
            if (written != count)
            {
                throw new InvalidOperationException(
                    $"native terrain grid wrote {written}, expected {count}");
            }
            return terrainGrid;
        }

        internal NativeSim.TerrainFrameInfo TerrainFrame(
            NativeSim.RenderCameraState camera,
            byte[] indices,
            byte[] coverage)
        {
            if (indices == null || indices.Length != NativeSim.TerrainFramePixels)
            {
                throw new ArgumentException(
                    $"terrain frame indices must contain exactly " +
                    $"{NativeSim.TerrainFramePixels} bytes",
                    nameof(indices));
            }
            if (coverage == null || coverage.Length != NativeSim.TerrainFramePixels)
            {
                throw new ArgumentException(
                    $"terrain frame coverage must contain exactly " +
                    $"{NativeSim.TerrainFramePixels} bytes",
                    nameof(coverage));
            }
            terrainFrameQuery.generation = camera.generation;
            terrainFrameQuery.tick = camera.tick;
            var status = NativeSim.mc_native_session_terrain_frame_copy(
                Handle,
                ref terrainFrameQuery,
                indices,
                checked((uint)indices.Length),
                coverage,
                checked((uint)coverage.Length),
                out var info,
                96);
            Check(status, "read isolated sky and terrain frame");
            var requiredFlags =
                NativeSim.TerrainFrameIsolatedSkyTerrain |
                NativeSim.TerrainFrameSkySeeded |
                NativeSim.TerrainFrameHasCoverage |
                NativeSim.TerrainFrameNonTerrainOmitted;
            if (
                info.abiVersion != NativeSim.AbiVersion ||
                info.structSize != 96 ||
                info.generation != terrainFrameQuery.generation ||
                info.tick != terrainFrameQuery.tick ||
                info.width != NativeSim.TerrainFrameWidth ||
                info.height != NativeSim.TerrainFrameHeight ||
                info.stride != NativeSim.TerrainFrameWidth ||
                info.viewportX != 0 ||
                info.viewportY != 0 ||
                info.viewportWidth != NativeSim.TerrainFrameWidth ||
                info.viewportHeight != NativeSim.TerrainFrameHeight ||
                info.reserved0 != 0 ||
                info.indexBytes != NativeSim.TerrainFramePixels ||
                info.coverageBytes != NativeSim.TerrainFramePixels ||
                (info.flags & requiredFlags) != requiredFlags ||
                info.reserved1 != 0 ||
                info.reserved2 != 0)
            {
                throw new InvalidOperationException(
                    "native terrain frame returned invalid layout, extent, " +
                    "generation, viewport, flags, or reserved state");
            }
            if (info.indexHash != ByteHash(indices) ||
                info.coverageHash != ByteHash(coverage))
            {
                throw new InvalidOperationException(
                    "native terrain frame hashes do not match copied bytes");
            }
            return info;
        }

        internal NativeSim.TerrainFrameInfo WorldFrame(
            NativeSim.RenderCameraState camera,
            byte[] indices,
            byte[] coverage)
        {
            if (indices == null || indices.Length != NativeSim.TerrainFramePixels)
            {
                throw new ArgumentException(
                    $"world frame indices must contain exactly " +
                    $"{NativeSim.TerrainFramePixels} bytes",
                    nameof(indices));
            }
            if (coverage == null || coverage.Length != NativeSim.TerrainFramePixels)
            {
                throw new ArgumentException(
                    $"world frame coverage must contain exactly " +
                    $"{NativeSim.TerrainFramePixels} bytes",
                    nameof(coverage));
            }
            terrainFrameQuery.generation = camera.generation;
            terrainFrameQuery.tick = camera.tick;
            var status = NativeSim.mc_native_session_world_frame_copy(
                Handle,
                ref terrainFrameQuery,
                indices,
                checked((uint)indices.Length),
                coverage,
                checked((uint)coverage.Length),
                out var info,
                96);
            Check(
                status,
                "read source-ordered terrain, sprite reflection/shadow, and entity world frame");
            var requiredFlags =
                NativeSim.TerrainFrameIsolatedSkyTerrain |
                NativeSim.TerrainFrameSkySeeded |
                NativeSim.TerrainFrameHasCoverage |
                NativeSim.TerrainFrameNonTerrainOmitted |
                NativeSim.TerrainFrameGoatsComposited |
                NativeSim.TerrainFrameArchersComposited |
                NativeSim.TerrainFrameFirefliesComposited |
                NativeSim.TerrainFrameCombatComposited |
                NativeSim.TerrainFrameObjectivePropsComposited |
                NativeSim.TerrainFrameEconomyComposited |
                NativeSim.TerrainFrameSpriteShadowsComposited |
                NativeSim.TerrainFrameFrozenActorsComposited;
            var reflectionExpected =
                camera.eyeZ < NativeSim.ReflectionEyeZLimit;
            if (
                info.abiVersion != NativeSim.AbiVersion ||
                info.structSize != 96 ||
                info.generation != terrainFrameQuery.generation ||
                info.tick != terrainFrameQuery.tick ||
                info.width != NativeSim.TerrainFrameWidth ||
                info.height != NativeSim.TerrainFrameHeight ||
                info.stride != NativeSim.TerrainFrameWidth ||
                info.viewportX != 0 ||
                info.viewportY != 0 ||
                info.viewportWidth != NativeSim.TerrainFrameWidth ||
                info.viewportHeight != NativeSim.TerrainFrameHeight ||
                info.reserved0 != 0 ||
                info.indexBytes != NativeSim.TerrainFramePixels ||
                info.coverageBytes != NativeSim.TerrainFramePixels ||
                (info.flags & requiredFlags) != requiredFlags ||
                ((info.flags & NativeSim.TerrainFrameReflectedSpritesComposited) != 0) !=
                    reflectionExpected ||
                info.reserved1 != 0 ||
                info.reserved2 != 0)
            {
                throw new InvalidOperationException(
                    "native world frame returned invalid layout, extent, " +
                    "generation, viewport, flags, or reserved state");
            }
            if (info.indexHash != ByteHash(indices) ||
                info.coverageHash != ByteHash(coverage))
            {
                throw new InvalidOperationException(
                    "native world frame hashes do not match copied bytes");
            }
            return info;
        }

        private static ulong ByteHash(byte[] bytes)
        {
            const ulong Offset = 0xcbf29ce484222325UL;
            const ulong Prime = 0x100000001b3UL;
            var hash = Offset;
            unchecked
            {
                foreach (var value in bytes)
                {
                    hash ^= value;
                    hash *= Prime;
                }
            }
            return hash;
        }

        internal NativeSim.SessionSnapshot Reset()
        {
            var status = NativeSim.mc_native_session_reset(Handle, out var snapshot, 120);
            Check(status, "reset session");
            return snapshot;
        }

        internal NativeSim.SessionSnapshot Step(
            sbyte roll,
            sbyte pitch,
            sbyte flags,
            ushort lookYaw = 0,
            ushort lookPitch = 0,
            sbyte opcode = 0,
            sbyte argument1 = 0,
            sbyte argument2 = 0,
            short requestedSpell = NativeSim.NoRequestedSpell)
        {
            var command = NativeSim.Command.Create(
                roll, pitch, flags, opcode, argument1, argument2, requestedSpell);
            var cameraInput = NativeSim.CameraInput.Create(lookYaw, lookPitch);
            var status = NativeSim.mc_native_session_step_with_camera(
                Handle,
                ref command,
                ref cameraInput,
                out var snapshot,
                120);
            Check(status, "step session");
            return snapshot;
        }

        internal string WorldString(uint kind)
        {
            var status = NativeSim.mc_native_world_string_length(Handle, kind, out var length);
            Check(status, "read world string length");
            var buffer = new byte[checked((int)length + 1)];
            status = NativeSim.mc_native_world_string_copy(
                Handle,
                kind,
                buffer,
                checked((uint)buffer.Length),
                out var written);
            Check(status, "read world string");
            return Encoding.UTF8.GetString(buffer, 0, checked((int)written));
        }

        internal NativeSim.TerrainState TerrainState()
        {
            var status = NativeSim.mc_native_session_terrain_state(
                Handle,
                out var state,
                64);
            Check(status, "read live session terrain state");
            if (
                state.abiVersion != NativeSim.AbiVersion ||
                state.structSize != 64 ||
                state.width != 256 ||
                state.height != 256 ||
                state.planeCount != 6 ||
                state.hashSchema != 1 ||
                state.authority != NativeSim.SessionTerrainLive ||
                state.reserved0 != 0 ||
                !AllZero(state.reserved))
            {
                throw new InvalidOperationException(
                    "native terrain state returned invalid layout, dimensions, " +
                    "hash schema, authority, or reserved state");
            }
            return state;
        }

        internal byte[] BytePlane(uint kind)
        {
            var status = NativeSim.mc_native_terrain_plane_length(Handle, kind, out var count);
            Check(status, "read terrain plane length");
            var values = new byte[checked((int)count)];
            status = NativeSim.mc_native_terrain_plane_copy_u8(
                Handle,
                kind,
                values,
                count,
                out var written);
            Check(status, "copy terrain plane");
            if (written != count)
            {
                throw new InvalidOperationException($"terrain plane wrote {written}/{count}");
            }
            return values;
        }

        internal short[] EntityIndexPlane()
        {
            var status = NativeSim.mc_native_terrain_plane_length(
                Handle,
                NativeSim.TerrainEntityIndex,
                out var count);
            Check(status, "read entity-index plane length");
            var values = new short[checked((int)count)];
            status = NativeSim.mc_native_terrain_entity_indices_copy(
                Handle,
                values,
                count,
                out var written);
            Check(status, "copy entity-index plane");
            if (written != count)
            {
                throw new InvalidOperationException($"entity-index plane wrote {written}/{count}");
            }
            return values;
        }

        internal NativeSim.EntitySnapshot[] Entities()
        {
            var status = NativeSim.mc_native_entity_count(Handle, out var count);
            Check(status, "read entity count");
            var values = new NativeSim.EntitySnapshot[checked((int)count)];
            status = NativeSim.mc_native_entities_copy(
                Handle,
                0,
                values,
                count,
                52,
                out var written);
            Check(status, "copy entities");
            if (written != count)
            {
                throw new InvalidOperationException($"entities wrote {written}/{count}");
            }
            return values;
        }

        internal NativeSim.EntitySnapshot EntityAt(uint index)
        {
            var value = new NativeSim.EntitySnapshot[1];
            var status = NativeSim.mc_native_entities_copy(
                Handle,
                index,
                value,
                1,
                52,
                out var written);
            Check(status, "copy live entity");
            if (written != 1)
            {
                throw new InvalidOperationException($"live entity wrote {written}/1");
            }
            return value[0];
        }

        internal NativeSim.GoatSnapshot[] Goats(
            ulong generation,
            ulong expectedTick,
            out NativeSim.GoatBatch batch)
        {
            var status = NativeSim.mc_native_goats_copy(
                Handle,
                generation,
                0,
                goats,
                checked((uint)goats.Length),
                84,
                out batch,
                64);
            Check(status, "copy live Goat family");
            if (
                batch.abiVersion != NativeSim.AbiVersion ||
                batch.structSize != 64 ||
                batch.generation != generation ||
                batch.tick != expectedTick ||
                batch.startOrdinal != 0 ||
                batch.written != goats.Length ||
                batch.totalCount != goats.Length ||
                batch.hasMore != 0 ||
                !AllZero(batch.reserved0) ||
                !AllZero(batch.reserved))
            {
                throw new InvalidOperationException(
                    "native Goat batch returned invalid layout, generation, tick, " +
                    "extent, continuation, or reserved state");
            }
            foreach (var goat in goats)
            {
                if (
                    goat.abiVersion != NativeSim.AbiVersion ||
                    goat.structSize != 84 ||
                    goat.authority != NativeSim.GoatLive ||
                    goat.reserved0 != 0 ||
                    !AllZero(goat.reserved))
                {
                    throw new InvalidOperationException(
                        $"native Goat {goat.entityIndex} returned invalid layout, " +
                        "authority, or reserved state");
                }
            }
            return goats;
        }

        internal NativeSim.Level1MissionSnapshot Level1Mission()
        {
            var status = NativeSim.mc_native_level1_mission_snapshot(
                Handle,
                out var mission,
                96);
            Check(status, "read live Level 1 mission");
            if (
                mission.abiVersion != NativeSim.AbiVersion ||
                mission.structSize != 96 ||
                mission.authority != NativeSim.Level1MissionLive ||
                mission.currentStage > 5 ||
                mission.textSelector < 48 ||
                mission.textSelector > 53 ||
                mission.actorCount != 45 ||
                mission.exitSequenceActive > 1 ||
                mission.exitSequenceComplete > 1 ||
                mission.exitSequencePhase > 12 ||
                mission.fidelityGapFlags != NativeSim.Level1CurrentFidelityGaps ||
                mission.possessionTier > 2)
            {
                throw new InvalidOperationException(
                    "native Level 1 mission returned invalid layout, authority, " +
                    "stage, authored actor extent, or possession tier");
            }
            return mission;
        }

        internal NativeSim.Level1HudSnapshot Level1Hud(
            NativeSim.Level1MissionSnapshot mission)
        {
            var status = NativeSim.mc_native_level1_hud_snapshot(
                Handle,
                out var hud,
                128);
            Check(status, "read live Level 1 HUD");
            if (
                hud.abiVersion != NativeSim.AbiVersion ||
                hud.structSize != 128 ||
                hud.authority != NativeSim.Level1MissionLive ||
                hud.generation != mission.generation ||
                hud.tick != mission.tick)
            {
                throw new InvalidOperationException(
                    "native Level 1 HUD returned invalid layout, authority, " +
                    "generation, or tick");
            }
            return hud;
        }

        internal NativeSim.Level1SpellMenuSnapshot Level1SpellMenu(
            NativeSim.Level1MissionSnapshot mission)
        {
            var status = NativeSim.mc_native_level1_spell_menu_snapshot(
                Handle,
                out var spellMenu,
                64);
            Check(status, "read live Level 1 spell menu");
            var validFamilies =
                spellMenu.familyEnabled != null &&
                spellMenu.familyEnabled.Length == 4 &&
                spellMenu.familyUnlockedTier != null &&
                spellMenu.familyUnlockedTier.Length == 4 &&
                spellMenu.familySelectedTier != null &&
                spellMenu.familySelectedTier.Length == 4 &&
                spellMenu.familyHandEligibility != null &&
                spellMenu.familyHandEligibility.Length == 4 &&
                spellMenu.holderEntityIndex != null &&
                spellMenu.holderEntityIndex.Length == 4;
            if (validFamilies)
            {
                for (var family = 0; family < 4; family++)
                {
                    var enabled = spellMenu.familyEnabled[family];
                    var unlocked = spellMenu.familyUnlockedTier[family];
                    var selected = spellMenu.familySelectedTier[family];
                    var eligibility = spellMenu.familyHandEligibility[family];
                    validFamilies &=
                        enabled <= 1 &&
                        unlocked is >= 0 and <= 2 &&
                        selected >= 0 &&
                        eligibility <= 2 &&
                        (enabled != 0) ==
                            (spellMenu.holderEntityIndex[family] != 0);
                }
            }
            if (
                spellMenu.abiVersion != NativeSim.AbiVersion ||
                spellMenu.structSize != 64 ||
                spellMenu.authority != NativeSim.Level1MissionLive ||
                spellMenu.generation != mission.generation ||
                spellMenu.tick != mission.tick ||
                spellMenu.selectedUiIndex >= 4 ||
                spellMenu.selectedTier is < 0 or > 2 ||
                spellMenu.leftFamily >= 4 ||
                spellMenu.leftTier < 0 ||
                spellMenu.rightFamily >= 4 ||
                spellMenu.rightTier < 0 ||
                spellMenu.reserved0 != 0 ||
                !validFamilies ||
                spellMenu.reserved == null ||
                Array.Exists(spellMenu.reserved, value => value != 0))
            {
                throw new InvalidOperationException(
                    "native Level 1 spell menu returned invalid layout, authority, " +
                    "generation, tick, family state, tier, or reserved bytes");
            }
            return spellMenu;
        }

        internal NativeSim.Level1MinimapMarks Level1MinimapMarks(
            NativeSim.Level1MissionSnapshot mission)
        {
            var status = NativeSim.mc_native_level1_minimap_marks(
                Handle,
                out var marks,
                168);
            Check(status, "read source-mapped Level 1 minimap marks");
            if (
                marks.abiVersion != NativeSim.AbiVersion ||
                marks.structSize != 168 ||
                marks.generation != mission.generation ||
                marks.tick != mission.tick ||
                marks.markCount > 16 ||
                marks.markPulse > 1 ||
                marks.arrowValid > 1 ||
                marks.arrowVisible > 1 ||
                (marks.arrowVisible != 0 && marks.arrowValid == 0) ||
                marks.markX == null || marks.markX.Length != 16 ||
                marks.markY == null || marks.markY.Length != 16 ||
                marks.markColour == null || marks.markColour.Length != 16 ||
                marks.markHelpSelector == null ||
                    marks.markHelpSelector.Length != 16 ||
                marks.arrowXY == null || marks.arrowXY.Length != 14 ||
                !AllZero(marks.reserved))
            {
                throw new InvalidOperationException(
                    "native Level 1 minimap marks returned invalid layout, " +
                    "clock, extent, pulse, or reserved state");
            }
            return marks;
        }

        internal NativeSim.Level1MinimapEntityCommand[] Level1MinimapEntities(
            NativeSim.Level1MissionSnapshot mission,
            out NativeSim.Level1MinimapEntitiesInfo info)
        {
            var commands = new NativeSim.Level1MinimapEntityCommand[
                NativeSim.Level1MinimapMaxEntityCommands];
            var status = NativeSim.mc_native_level1_minimap_entities_copy(
                Handle,
                mission.generation,
                mission.tick,
                commands,
                checked((uint)commands.Length),
                24,
                out info,
                64);
            Check(status, "copy source-ordered Level 1 minimap entity commands");
            if (
                info.abiVersion != NativeSim.AbiVersion ||
                info.structSize != 64 ||
                info.generation != mission.generation ||
                info.tick != mission.tick ||
                info.totalCount > NativeSim.Level1MinimapMaxEntityCommands ||
                info.crossX != 63 ||
                info.crossY != 64 ||
                info.crossColour != 176 ||
                info.crossArmLength != 10 ||
                info.revealActive > 1 ||
                info.authority != NativeSim.Level1MissionLive ||
                info.commandHash == 0 ||
                !AllZero(info.reserved0) ||
                !AllZero(info.reserved))
            {
                throw new InvalidOperationException(
                    "native Level 1 minimap entity stream returned invalid " +
                    "layout, clock, extent, cross, authority, hash, or reserved state");
            }
            Array.Resize(ref commands, checked((int)info.totalCount));
            ushort previousSourceSlot = 0;
            var seenEntityCommand = false;
            for (var index = 0; index < commands.Length; index++)
            {
                var command = commands[index];
                var validKind =
                    command.kind == NativeSim.Level1MinimapCommandDot &&
                    command.selector == 0 &&
                    command.size is 1 or 2 &&
                    command.anchor == NativeSim.Level1MinimapAnchorTopLeft ||
                    command.kind == NativeSim.Level1MinimapCommandSprite &&
                    command.selector is 58 or 66 or 83 or 84 &&
                    command.size == 1 &&
                    command.anchor is
                        NativeSim.Level1MinimapAnchorBottomLeft or
                        NativeSim.Level1MinimapAnchorBottomCenter ||
                    command.kind == NativeSim.Level1MinimapCommandBlendPixel &&
                    command.selector == 0 &&
                    command.size == 0 &&
                    command.anchor == NativeSim.Level1MinimapAnchorTopLeft;
                if (
                    command.sourceSlot == 0 ||
                    command.sourceSlot >= 1000 ||
                    !validKind ||
                    command.flags !=
                        (command.kind == NativeSim.Level1MinimapCommandBlendPixel
                            ? 0
                            : 0x07) ||
                    command.order != index ||
                    !AllZero(command.reserved))
                {
                    throw new InvalidOperationException(
                        $"native minimap command {index} returned invalid " +
                        "source slot, kind, flags, order, or reserved state");
                }
                if (command.kind != NativeSim.Level1MinimapCommandBlendPixel)
                {
                    if (seenEntityCommand && command.sourceSlot < previousSourceSlot)
                        throw new InvalidOperationException(
                            "native minimap entity command order regressed");
                    seenEntityCommand = true;
                    previousSourceSlot = command.sourceSlot;
                }
            }
            return commands;
        }

        internal NativeSim.Level1PaletteInfo Level1Palette(
            NativeSim.Level1MissionSnapshot mission,
            byte[] rgb8)
        {
            if (rgb8 == null || rgb8.Length != 768)
                throw new ArgumentException(
                    "Level 1 effective palette requires exactly 768 RGB8 bytes",
                    nameof(rgb8));
            var status = NativeSim.mc_native_level1_palette_copy(
                Handle,
                rgb8,
                checked((uint)rgb8.Length),
                out var info,
                80);
            Check(status, "read source-mapped Level 1 effective palette");
            byte[] computedHash;
            using (var sha256 = SHA256.Create())
                computedHash = sha256.ComputeHash(rgb8);
            if (
                info.abiVersion != NativeSim.AbiVersion ||
                info.structSize != 80 ||
                info.generation != mission.generation ||
                info.tick != mission.tick ||
                info.effect > NativeSim.Level1PaletteObjective ||
                info.weight > info.denominator ||
                info.flags != 1 ||
                info.rgb8Sha256 == null ||
                    info.rgb8Sha256.Length != computedHash.Length ||
                !EqualBytes(info.rgb8Sha256, computedHash) ||
                !AllZero(info.reserved))
            {
                throw new InvalidOperationException(
                    "native Level 1 palette returned invalid layout, clock, " +
                    "state, hash, or reserved fields");
            }
            return info;
        }

        internal NativeSim.Level1ExitPresentation Level1ExitPresentation(
            NativeSim.Level1MissionSnapshot mission)
        {
            var status = NativeSim.mc_native_level1_exit_presentation(
                Handle,
                out var state,
                64);
            Check(status, "read live Level 1 exit presentation");
            var expectedBaseFlags =
                NativeSim.Level1ExitFlagPaletteVisualCurrent |
                NativeSim.Level1ExitFlagFovPostActionNextHistory;
            var validPaletteDenominator =
                state.paletteDenominator == 0 ||
                state.paletteDenominator == 4 ||
                state.paletteDenominator == 16 ||
                state.paletteDenominator == 28;
            if (
                state.abiVersion != NativeSim.AbiVersion ||
                state.structSize != 64 ||
                state.generation != mission.generation ||
                state.tick != mission.tick ||
                state.authority != NativeSim.Level1MissionLive ||
                (state.playerAction != 0 && state.playerAction != 12) ||
                state.actionPhase > 12 ||
                state.paletteEffect > NativeSim.Level1PaletteObjective ||
                !validPaletteDenominator ||
                state.paletteWeight > state.paletteDenominator ||
                state.movementResolution > 3 ||
                state.targetValid > 1 ||
                state.inputSuppressed > 1 ||
                state.displayBlank > 1 ||
                state.active > 1 ||
                state.complete > 1 ||
                (state.active != 0 && state.complete != 0) ||
                state.active != mission.exitSequenceActive ||
                state.complete != mission.exitSequenceComplete ||
                state.actionPhase != mission.exitSequencePhase ||
                state.actionTimer != mission.exitSequenceTimer ||
                state.resultCode != mission.exitResultCode ||
                state.inputSuppressed !=
                    ((state.active != 0 || state.complete != 0) ? 1 : 0) ||
                (state.flags & NativeSim.Level1ExitKnownFlags) != state.flags ||
                (state.flags & expectedBaseFlags) != expectedBaseFlags ||
                state.reserved0 != 0 ||
                !AllZero(state.reserved))
            {
                throw new InvalidOperationException(
                    "native Level 1 exit presentation returned invalid layout, " +
                    "clock contract, action state, or reserved fields");
            }
            return state;
        }

        internal NativeSim.Level1ActorSnapshot[] Level1Actors(
            NativeSim.Level1MissionSnapshot mission)
        {
            var values = new NativeSim.Level1ActorSnapshot[
                checked((int)mission.actorCount)];
            var status = NativeSim.mc_native_level1_actors_copy(
                Handle,
                mission.generation,
                mission.tick,
                0,
                values,
                mission.actorCount,
                56,
                out var written);
            Check(status, "copy live Level 1 actors");
            if (written != mission.actorCount)
            {
                throw new InvalidOperationException(
                    $"Level 1 actors wrote {written}/{mission.actorCount}");
            }
            foreach (var actor in values)
            {
                var exactRenderMapping =
                    actor.kind == 1 &&
                    actor.renderMode == 20 &&
                    (
                        actor.renderParameterIndex == 0 &&
                        actor.baseTmap == 8 &&
                        actor.logicalHeight == 250 ||
                        actor.renderParameterIndex == 1 &&
                        actor.baseTmap == 11 &&
                        actor.logicalHeight == 250 ||
                        actor.renderParameterIndex == 206 &&
                        actor.baseTmap == 121 &&
                        actor.logicalHeight == 240
                    ) ||
                    actor.kind == 2 &&
                    actor.renderParameterIndex == 287 &&
                    actor.baseTmap == 279 &&
                    actor.renderMode == 17 &&
                    actor.logicalHeight == 200 ||
                    (actor.kind == 3 || actor.kind == 4) &&
                    actor.renderParameterIndex == 77 &&
                    actor.baseTmap == 59 &&
                    actor.renderMode == 1 &&
                    actor.logicalHeight == 150 ||
                    actor.kind == 5 &&
                    actor.renderParameterIndex == 338 &&
                    actor.baseTmap == 461 &&
                    actor.renderMode == 0 &&
                    actor.logicalHeight == 700 ||
                    actor.kind == 6 &&
                    actor.renderParameterIndex == ushort.MaxValue &&
                    actor.baseTmap == ushort.MaxValue &&
                    actor.renderMode == 0 &&
                    actor.logicalHeight == 0 ||
                    actor.kind == 7 &&
                    actor.renderParameterIndex == 88 &&
                    actor.baseTmap == 30 &&
                    actor.renderMode == 17 &&
                    actor.logicalHeight == 180 ||
                    actor.kind == 8 &&
                    actor.renderParameterIndex == 280 &&
                    actor.baseTmap == 259 &&
                    actor.renderMode == 1 &&
                    actor.logicalHeight == 75 ||
                    actor.kind == 9 &&
                    actor.renderMode == 0 &&
                    actor.logicalHeight == 400 &&
                    (
                        actor.renderParameterIndex == 83 &&
                        actor.baseTmap == 75 ||
                        actor.renderParameterIndex == 84 &&
                        actor.baseTmap == 87 ||
                        actor.renderParameterIndex == 226 &&
                        actor.baseTmap == 178 ||
                        actor.renderParameterIndex == 227 &&
                        actor.baseTmap == 179
                    ) ||
                    actor.kind == 10 &&
                    actor.renderParameterIndex == 79 &&
                    actor.baseTmap == 78 &&
                    actor.renderMode == 0 &&
                    actor.logicalHeight == 250 ||
                    actor.kind == 11 &&
                    actor.renderParameterIndex == ushort.MaxValue &&
                    actor.baseTmap == ushort.MaxValue &&
                    actor.renderMode == 0 &&
                    actor.logicalHeight == 0;
                if (
                    actor.abiVersion != NativeSim.AbiVersion ||
                    actor.structSize != 56 ||
                    actor.authority != NativeSim.Level1MissionLive ||
                    actor.kind < 1 ||
                    actor.kind > 11 ||
                    !exactRenderMapping ||
                    actor.animationFrame != 0 ||
                    actor.visible > 1 ||
                    actor.reserved != 0)
                {
                    throw new InvalidOperationException(
                        $"native Level 1 actor {actor.authoredIndex} returned " +
                        "invalid layout, authority, kind, render mapping, or reserved state");
                }
            }
            return values;
        }

        internal NativeSim.Level1MultipartSegmentSnapshot[] Level1MultipartSegments(
            NativeSim.Level1MissionSnapshot mission)
        {
            // Level 1 owns at most three simultaneously live 16-node chains.
            var values = new NativeSim.Level1MultipartSegmentSnapshot[64];
            var status = NativeSim.mc_native_level1_multipart_segments_copy(
                Handle,
                mission.generation,
                mission.tick,
                0,
                values,
                checked((uint)values.Length),
                64,
                out var written);
            Check(status, "copy live Level 1 multipart segments");
            Array.Resize(ref values, checked((int)written));
            foreach (var segment in values)
            {
                if (
                    segment.abiVersion != NativeSim.AbiVersion ||
                    segment.structSize != 64 ||
                    segment.authority != NativeSim.Level1MissionLive ||
                    segment.ordinal >= 16 ||
                    segment.classId != 5 ||
                    segment.model != 3 ||
                    (segment.action != 29 && segment.action != 232) ||
                    segment.baseTmap != 66 ||
                    segment.renderMode != 0 ||
                    segment.renderParameterIndex < 89 ||
                    segment.renderParameterIndex > 104 ||
                    segment.active > 1 ||
                    segment.visible > 1 ||
                    segment.reserved0 != 0 ||
                    segment.reserved != 0)
                {
                    throw new InvalidOperationException(
                        $"native multipart segment {segment.sourceEntityIndex} returned " +
                        "invalid layout, authority, chain identity, or render mapping");
                }
            }
            return values;
        }

        internal NativeSim.Level1CastleSnapshot Level1Castle(
            NativeSim.Level1MissionSnapshot mission)
        {
            var status = NativeSim.mc_native_level1_castle_snapshot(
                Handle,
                out var castle,
                80);
            Check(status, "read live Level 1 castle");
            var exactSourceIdentity = castle.active == 0
                ? AllZero(castle.reserved)
                : castle.reserved != null &&
                  castle.reserved.Length == 3 &&
                  castle.reserved[0] > 0 &&
                  castle.reserved[0] < 1000 &&
                  castle.reserved[1] > 0 &&
                  castle.reserved[2] == 0;
            if (
                castle.abiVersion != NativeSim.AbiVersion ||
                castle.structSize != 80 ||
                castle.generation != mission.generation ||
                castle.tick != mission.tick ||
                castle.authority != NativeSim.Level1MissionLive ||
                castle.classId != 3 ||
                castle.model != 2 ||
                castle.action != 5 ||
                castle.renderMode != 0 ||
                castle.renderParameterIndex != 177 ||
                castle.baseTmap != 96 ||
                castle.logicalHeight != 400 ||
                castle.reserved0 != 0 ||
                castle.reserved1 != 0 ||
                !exactSourceIdentity)
            {
                throw new InvalidOperationException(
                    "native Level 1 castle returned invalid layout, authority, " +
                    "identity, render mapping, or reserved state");
            }
            return castle;
        }

        internal NativeSim.Level1CastleLifecycleSnapshot Level1CastleLifecycle(
            NativeSim.Level1MissionSnapshot mission)
        {
            var status = NativeSim.mc_native_level1_castle_lifecycle_snapshot(
                Handle,
                out var lifecycle,
                80);
            Check(status, "read live Level 1 castle lifecycle");
            var absentConstructionIsZero =
                lifecycle.constructionPresent != 0 ||
                lifecycle.constructionId == 0 &&
                lifecycle.constructionSourceEntityIndex == 0 &&
                lifecycle.constructionParentEntityIndex == 0 &&
                lifecycle.constructionOwnerEntityIndex == 0 &&
                lifecycle.constructionX == 0 &&
                lifecycle.constructionY == 0 &&
                lifecycle.constructionZ == 0 &&
                lifecycle.constructionTimer == 0 &&
                lifecycle.constructionStage == 0 &&
                lifecycle.constructionActive == 0 &&
                lifecycle.constructionInitialized == 0;
            if (
                lifecycle.abiVersion != NativeSim.AbiVersion ||
                lifecycle.structSize != 80 ||
                lifecycle.generation != mission.generation ||
                lifecycle.tick != mission.tick ||
                lifecycle.authority != NativeSim.Level1MissionLive ||
                lifecycle.rootActive > 1 ||
                lifecycle.constructionPresent > 1 ||
                lifecycle.constructionActive > 1 ||
                lifecycle.constructionInitialized > 1 ||
                !absentConstructionIsZero ||
                !AllZero(lifecycle.reserved0))
            {
                throw new InvalidOperationException(
                    "native Level 1 castle lifecycle returned invalid layout, " +
                    "authority, generation, tick, flags, or absent state");
            }
            return lifecycle;
        }

        internal NativeSim.ManaCarrierSnapshot Level1ManaCarrier(
            NativeSim.Level1MissionSnapshot mission)
        {
            var status = NativeSim.mc_native_level1_mana_carrier_snapshot(
                Handle,
                out var carrier,
                80);
            Check(status, "read live Level 1 mana carrier");
            var exactSourceIdentity = carrier.carrierId == 0
                ? AllZero(carrier.reserved)
                : carrier.reserved != null &&
                  carrier.reserved.Length == 3 &&
                  carrier.carrierId < 1000 &&
                  carrier.reserved[0] == carrier.carrierId &&
                  carrier.reserved[1] == 0 &&
                  carrier.reserved[2] == 0;
            if (
                carrier.abiVersion != NativeSim.AbiVersion ||
                carrier.structSize != 80 ||
                carrier.generation != mission.generation ||
                carrier.tick != mission.tick ||
                carrier.authority != NativeSim.Level1MissionLive ||
                carrier.active > 1 ||
                carrier.classId != 3 ||
                carrier.model != 3 ||
                carrier.action != 9 ||
                carrier.returningToCastle > 1 ||
                carrier.targetSphereId >= 1000 ||
                carrier.renderMode != 0 ||
                carrier.renderParameterIndex != 169 ||
                carrier.baseTmap != 88 ||
                carrier.logicalHeight != 800 ||
                carrier.reserved0 != 0 ||
                carrier.reserved1 != 0 ||
                !exactSourceIdentity)
            {
                throw new InvalidOperationException(
                    "native Level 1 mana carrier returned invalid layout, authority, " +
                    "identity, render mapping, or reserved state");
            }
            return carrier;
        }

        internal NativeSim.ManaSphereSnapshot[] Level1ManaSpheres(
            NativeSim.Level1MissionSnapshot mission)
        {
            const uint pageCapacity = 128;
            const uint maximumSpheres = 4096;
            var collected = new List<NativeSim.ManaSphereSnapshot>();
            uint start = 0;
            while (true)
            {
                var page = new NativeSim.ManaSphereSnapshot[
                    checked((int)pageCapacity)];
                var status = NativeSim.mc_native_level1_mana_spheres_copy(
                    Handle,
                    mission.generation,
                    mission.tick,
                    start,
                    page,
                    pageCapacity,
                    64,
                    out var written);
                Check(status, "copy live Level 1 mana spheres");
                if (written > pageCapacity)
                    throw new InvalidOperationException(
                        $"mana sphere page wrote {written}/{pageCapacity}");
                for (var index = 0; index < written; index++)
                    collected.Add(page[index]);
                start = checked(start + written);
                if (start > maximumSpheres)
                    throw new InvalidOperationException(
                        "native Level 1 mana sphere family exceeds safety bound");
                if (written < pageCapacity) break;
            }
            foreach (var sphere in collected)
            {
                if (
                    sphere.abiVersion != NativeSim.AbiVersion ||
                    sphere.structSize != 64 ||
                    sphere.authority != NativeSim.Level1MissionLive ||
                    sphere.classId != 10 ||
                    sphere.model != 39 ||
                    sphere.action != 41 ||
                    sphere.renderMode != 21 ||
                    sphere.renderParameterIndex < 52 ||
                    sphere.logicalHeight < 50 ||
                    sphere.logicalHeight > 400 ||
                    sphere.reserved0 != 0 ||
                    !AllZero(sphere.reserved))
                {
                    throw new InvalidOperationException(
                        $"native mana sphere {sphere.sphereId} returned invalid layout, " +
                        "authority, identity, render mapping, or reserved state");
                }
            }
            return collected.ToArray();
        }

        internal NativeSim.FireballProjectileSnapshot[] FireballProjectiles(
            NativeSim.Level1MissionSnapshot mission)
        {
            if (mission.projectileCount == 0)
            {
                return Array.Empty<NativeSim.FireballProjectileSnapshot>();
            }
            var values = new NativeSim.FireballProjectileSnapshot[
                checked((int)mission.projectileCount)];
            var status = NativeSim.mc_native_fireball_projectiles_copy(
                Handle,
                mission.generation,
                mission.tick,
                0,
                values,
                mission.projectileCount,
                56,
                out var written);
            Check(status, "copy live Fireball projectiles");
            if (written != mission.projectileCount)
            {
                throw new InvalidOperationException(
                    $"Fireball projectiles wrote {written}/{mission.projectileCount}");
            }
            foreach (var projectile in values)
            {
                var exactRenderMapping =
                    projectile.projectileKind == 1 &&
                    projectile.renderIndex == 340 &&
                    projectile.baseTmap == 463 &&
                    projectile.logicalHeight == 100 &&
                    projectile.renderMode == 21 &&
                    projectile.classId == 9 &&
                    projectile.model == 0 ||
                    projectile.projectileKind == 2 &&
                    projectile.renderIndex == 195 &&
                    projectile.baseTmap == 105 &&
                    projectile.logicalHeight == 60 &&
                    projectile.renderMode == 19 &&
                    projectile.classId == 9 &&
                    projectile.model == 13;
                exactRenderMapping =
                    exactRenderMapping ||
                    (
                        projectile.projectileKind == 3 &&
                        projectile.renderIndex == 340 &&
                        projectile.baseTmap == 463 &&
                        projectile.logicalHeight == 100 &&
                        projectile.renderMode == 21 &&
                        projectile.classId == 9 &&
                        projectile.model == 0
                    ) ||
                    (
                        projectile.projectileKind == 4 &&
                        projectile.renderIndex == 18 &&
                        projectile.baseTmap == 61 &&
                        projectile.logicalHeight == 150 &&
                        projectile.renderMode == 21 &&
                        projectile.classId == 9 &&
                        projectile.model == 10
                    ) ||
                    (
                        projectile.projectileKind == 5 &&
                        projectile.renderIndex == 209 &&
                        projectile.baseTmap == 144 &&
                        projectile.logicalHeight == 150 &&
                        projectile.renderMode == 21 &&
                        projectile.classId == 9 &&
                        projectile.model == 1
                    );
                if (
                    projectile.abiVersion != NativeSim.AbiVersion ||
                    projectile.structSize != 56 ||
                    projectile.authority != NativeSim.Level1MissionLive ||
                    !exactRenderMapping ||
                    projectile.animationFrame != 0 ||
                    projectile.reserved == null ||
                    projectile.reserved.Length != 2 ||
                    projectile.reserved[0] == 0 ||
                    projectile.reserved[0] >= 1000 ||
                    projectile.reserved[1] != 0)
                {
                    throw new InvalidOperationException(
                        $"native Level 1 projectile {projectile.projectileId} " +
                        "returned invalid layout, authority, render mapping, or reserved state");
                }
            }
            return values;
        }

        internal NativeSim.FireballImpactSnapshot[] FireballImpacts(
            NativeSim.Level1MissionSnapshot mission)
        {
            if (mission.impactCount == 0)
            {
                return Array.Empty<NativeSim.FireballImpactSnapshot>();
            }
            var values = new NativeSim.FireballImpactSnapshot[
                checked((int)mission.impactCount)];
            var status = NativeSim.mc_native_fireball_impacts_copy(
                Handle,
                mission.generation,
                mission.tick,
                0,
                values,
                mission.impactCount,
                52,
                out var written);
            Check(status, "copy live Fireball impacts");
            if (written != mission.impactCount)
            {
                throw new InvalidOperationException(
                    $"Fireball impacts wrote {written}/{mission.impactCount}");
            }
            foreach (var impact in values)
            {
                var exactRenderMapping =
                    (
                        impact.model == 0 &&
                        impact.renderParameterIndex == 7 &&
                        impact.baseTmap == 38 &&
                        impact.logicalHeight == 250 &&
                        impact.renderMode == 36
                    ) ||
                    (
                        impact.model == 5 &&
                        impact.renderParameterIndex == 244 &&
                        impact.baseTmap == 186 &&
                        impact.logicalHeight == 300 &&
                        impact.renderMode == 11
                    ) ||
                    (
                        impact.model == 12 &&
                        impact.renderParameterIndex == 41 &&
                        impact.baseTmap == 54 &&
                        impact.logicalHeight == 250 &&
                        impact.renderMode == 21
                    ) ||
                    (
                        impact.model == 13 &&
                        impact.renderParameterIndex >= 67 &&
                        impact.renderParameterIndex <= 74 &&
                        impact.baseTmap == 57 &&
                        impact.logicalHeight ==
                            (impact.renderParameterIndex - 66) * 50 &&
                        impact.renderMode == 21
                    );
                if (
                    impact.abiVersion != NativeSim.AbiVersion ||
                    impact.structSize != 52 ||
                    impact.authority != NativeSim.Level1MissionLive ||
                    !exactRenderMapping ||
                    impact.animationFrame < 0 ||
                    (
                        impact.model == 13
                            ? impact.animationFrame < 67 || impact.animationFrame > 74
                            : impact.animationFrame > 9
                    ) ||
                    impact.reserved0 != 0 ||
                    impact.reserved1 != 0 ||
                    impact.reserved == null ||
                    impact.reserved.Length != 2 ||
                    impact.reserved[0] == 0 ||
                    impact.reserved[0] >= 1000 ||
                    impact.reserved[1] != 0)
                {
                    throw new InvalidOperationException(
                        $"native Fireball impact {impact.impactId} returned " +
                        "invalid layout, authority, or reserved state");
                }
            }
            return values;
        }

        internal NativeSim.GoatBatch InjectGoatDamageForQualification(
            ulong generation,
            ushort entityIndex,
            int damage)
        {
            var command = NativeSim.GoatDamageCommand.Create(
                generation,
                entityIndex,
                damage);
            var status = NativeSim.mc_native_session_goat_inject_damage(
                Handle,
                ref command,
                out var batch,
                64);
            Check(status, "inject qualification Goat damage");
            if (
                batch.abiVersion != NativeSim.AbiVersion ||
                batch.structSize != 64 ||
                batch.generation != generation ||
                batch.startOrdinal != 0 ||
                batch.written != 0 ||
                batch.totalCount != goats.Length ||
                batch.hasMore != 0 ||
                !AllZero(batch.reserved0) ||
                !AllZero(batch.reserved))
            {
                throw new InvalidOperationException(
                    "qualification Goat damage returned invalid batch");
            }
            return batch;
        }

        internal NativeSim.SelectorFrameState[] SelectorFrames(
            ulong generation,
            ulong expectedTick,
            out NativeSim.SelectorFrameBatch batch)
        {
            var status = NativeSim.mc_native_selector_frame_states_copy(
                Handle,
                generation,
                expectedTick,
                0,
                selectorFrames,
                checked((uint)selectorFrames.Length),
                64,
                out batch,
                88);
            Check(status, "copy selector-frame states");
            if (
                batch.abiVersion != NativeSim.AbiVersion ||
                batch.structSize != 88 ||
                batch.generation != generation ||
                batch.tick != expectedTick ||
                batch.selectorStateHash == 0 ||
                batch.validDrawMask != 0x07 ||
                batch.startOrdinal != 0 ||
                batch.written != selectorFrames.Length ||
                batch.totalCount != selectorFrames.Length ||
                batch.hasMore != 0 ||
                batch.reserved0 != 0 ||
                !AllZero(batch.reserved))
            {
                throw new InvalidOperationException(
                    "native selector-frame batch returned invalid identity, extent, or reserved state");
            }
            for (var ordinal = 0; ordinal < selectorFrames.Length; ordinal++)
            {
                var frame = selectorFrames[ordinal];
                var expectedSelector = 152 + ordinal;
                var knownFlags =
                    NativeSim.SelectorFrameAnimated |
                    NativeSim.SelectorFrameGroupedUpdate |
                    NativeSim.SelectorFrameUsedByPrecedingDraw |
                    NativeSim.SelectorFramePixelsMutable;
                if (
                    frame.abiVersion != NativeSim.AbiVersion ||
                    frame.structSize != 64 ||
                    frame.selector != expectedSelector ||
                    frame.maskBit != ordinal ||
                    frame.authority != NativeSim.SelectorFrameLive ||
                    frame.width == 0 ||
                    frame.height == 0 ||
                    frame.sourceFrameIndex < 1 ||
                    frame.sourceFrameIndex > frame.frameCount + 1 ||
                    frame.frameCount != 4 ||
                    frame.streamStart > frame.payloadBytes ||
                    frame.streamCursor > frame.payloadBytes ||
                    frame.pixelBytes != checked((uint)frame.width * frame.height) ||
                    (frame.flags & ~knownFlags) != 0 ||
                    (frame.flags & NativeSim.SelectorFrameAnimated) == 0 ||
                    (frame.flags & NativeSim.SelectorFramePixelsMutable) == 0 ||
                    (frame.flags & NativeSim.SelectorFrameGroupedUpdate) != 0 ||
                    frame.pixelHash == 0 ||
                    !AllZero(frame.reserved))
                {
                    throw new InvalidOperationException(
                        $"native selector-frame {expectedSelector} returned invalid source state");
                }
            }
            return selectorFrames;
        }

        internal NativeSim.SelectorFrameBatch EvaluateGoatDrawAdmission(
            ulong generation,
            ulong expectedTick)
        {
            var status = NativeSim.mc_native_session_goat_draw_admission_evaluate(
                Handle,
                generation,
                expectedTick,
                out var batch,
                88);
            Check(status, "evaluate source CPU Goat draw admission");
            if (
                batch.abiVersion != NativeSim.AbiVersion ||
                batch.structSize != 88 ||
                batch.generation != generation ||
                batch.tick != expectedTick ||
                batch.validDrawMask != 0x07 ||
                batch.startOrdinal != 0 ||
                batch.written != 0 ||
                batch.totalCount != selectorFrames.Length ||
                batch.hasMore != 0 ||
                batch.hasDrawFeedback != 1 ||
                batch.lastDrawTick != expectedTick ||
                (batch.lastDrawMask & ~0x07UL) != 0 ||
                batch.reserved0 != 0 ||
                !AllZero(batch.reserved))
            {
                throw new InvalidOperationException(
                    "native Goat CPU draw admission returned invalid receipt");
            }
            return batch;
        }

        internal NativeSim.SelectorFrameBatch SubmitSelectorDrawFeedbackForQualification(
            ulong generation,
            ulong drawTick,
            ulong selectorMask)
        {
            var command = NativeSim.SelectorDrawFeedback.Create(
                generation,
                drawTick,
                selectorMask);
            var status = NativeSim.mc_native_session_selector_draw_feedback_submit(
                Handle,
                ref command,
                out var batch,
                88);
            Check(status, "submit qualification selector draw feedback");
            if (
                batch.abiVersion != NativeSim.AbiVersion ||
                batch.structSize != 88 ||
                batch.generation != generation ||
                batch.tick != drawTick ||
                batch.validDrawMask != 0x07 ||
                batch.startOrdinal != 0 ||
                batch.written != 0 ||
                batch.totalCount != selectorFrames.Length ||
                batch.hasMore != 0 ||
                batch.hasDrawFeedback != 1 ||
                batch.lastDrawTick != drawTick ||
                (batch.lastDrawMask & selectorMask) != selectorMask ||
                batch.reserved0 != 0 ||
                !AllZero(batch.reserved))
            {
                throw new InvalidOperationException(
                    "qualification selector feedback returned invalid receipt");
            }
            return batch;
        }

        internal NativeSim.ObjectiveSnapshot[] Objectives()
        {
            var status = NativeSim.mc_native_objective_count(Handle, out var count);
            Check(status, "read objective count");
            var values = new NativeSim.ObjectiveSnapshot[checked((int)count)];
            status = NativeSim.mc_native_objectives_copy(
                Handle,
                0,
                values,
                count,
                40,
                out var written);
            Check(status, "copy objectives");
            if (written != count)
            {
                throw new InvalidOperationException($"objectives wrote {written}/{count}");
            }
            return values;
        }

        private static bool AllZero(byte[] values)
        {
            if (values == null) return false;
            foreach (var value in values)
            {
                if (value != 0) return false;
            }
            return true;
        }

        private static bool EqualBytes(byte[] left, byte[] right)
        {
            if (left == null || right == null || left.Length != right.Length)
                return false;
            for (var index = 0; index < left.Length; index++)
            {
                if (left[index] != right[index]) return false;
            }
            return true;
        }

        private static bool AllZero(uint[] values)
        {
            if (values == null) return false;
            foreach (var value in values)
            {
                if (value != 0) return false;
            }
            return true;
        }

        internal int CopyEvents(
            ulong generation,
            ulong cursor,
            NativeSim.NativeEvent[] events,
            out NativeSim.EventBatch batch,
            out string error)
        {
            var query = NativeSim.EventQuery.Create(generation, cursor);
            var status = NativeSim.mc_native_events_copy_since(
                Handle,
                ref query,
                events,
                checked((uint)events.Length),
                72,
                out batch,
                56);
            error = status == NativeSim.Ok ? null : ReadLastError();
            return status;
        }

        internal bool TryAudioCommand(
            NativeSim.NativeEvent nativeEvent,
            out NativeSim.AudioCommand command,
            out string error)
        {
            var status = NativeSim.mc_native_audio_command_for_event(
                Handle,
                nativeEvent.generation,
                nativeEvent.sequence,
                out command,
                88);
            if (status == NativeSim.Ok)
            {
                error = null;
                return true;
            }
            error = ReadLastError();
            if (status != NativeSim.Bounds)
            {
                throw new InvalidOperationException(
                    $"read positional audio sidecar: status {status}: {error}");
            }
            return false;
        }

        internal void CompleteAudioVoice(
            ulong generation,
            ushort voiceIndex,
            ulong eventSequence)
        {
            var status = NativeSim.mc_native_audio_voice_complete(
                Handle, generation, voiceIndex, eventSequence);
            Check(status, $"complete audio voice {voiceIndex}");
        }

        internal void SubmitAudioQualification(
            uint eventKind,
            ushort soundId,
            short soundFlags,
            ushort volumeQ15,
            ushort panQ16,
            byte playType,
            out NativeSim.NativeEvent nativeEvent,
            out NativeSim.AudioCommand command)
        {
            var status = NativeSim.mc_native_audio_qualification_submit(
                Handle,
                eventKind,
                soundId,
                soundFlags,
                volumeQ15,
                panQ16,
                playType,
                out nativeEvent,
                72,
                out command,
                88);
            Check(status, $"submit qualification audio {soundId}/{soundFlags}");
        }

        internal static void Check(int status, string operation)
        {
            if (status != NativeSim.Ok)
            {
                var detail = ReadLastError();
                throw new InvalidOperationException($"{operation}: status {status}: {detail}");
            }
        }

        internal static string ReadLastError()
        {
            var status = NativeSim.mc_native_last_error_length(out var length);
            if (status != NativeSim.Ok)
            {
                return $"last-error length status {status}";
            }
            var buffer = new byte[checked((int)length + 1)];
            status = NativeSim.mc_native_last_error_copy(
                buffer,
                checked((uint)buffer.Length),
                out var written);
            if (status != NativeSim.Ok)
            {
                return $"last-error copy status {status}";
            }
            return Encoding.UTF8.GetString(buffer, 0, checked((int)written));
        }

        public void Dispose()
        {
            Release();
        }

        private void Release()
        {
            var value = Interlocked.Exchange(ref handleBits, 0);
            if (value == 0)
            {
                return;
            }
            var status = NativeSim.mc_native_session_destroy(unchecked((ulong)value));
            if (status != NativeSim.Ok)
            {
                UnityEngine.Debug.LogError(
                    $"destroy native session failed: {status}: {ReadLastError()}");
            }
            lock (RegistryLock)
            {
                Sessions.Remove(this);
            }
        }

        private static void DisposeAll()
        {
            NativeSession[] sessions;
            lock (RegistryLock)
            {
                sessions = new NativeSession[Sessions.Count];
                Sessions.CopyTo(sessions);
            }
            foreach (var session in sessions)
            {
                session.Dispose();
            }
        }
    }
}
