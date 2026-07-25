using System;
using System.Collections.Generic;
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
            sbyte argument2 = 0)
        {
            var command = NativeSim.Command.Create(
                roll, pitch, flags, opcode, argument1, argument2);
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
