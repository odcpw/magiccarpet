using System;
using System.Globalization;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using UnityEngine;

namespace MagicCarpet.Native
{
    /// <summary>
    /// Managed transport for the neutral Rust GameSession contract. This type
    /// parses observations but owns no gameplay state; every command and read
    /// crosses the explicitly versioned native ABI.
    /// </summary>
    internal sealed class EngineSessionTransport
    {
        internal const string EngineContract = "carpet-flight-game-session/v1";
        internal const string TransportContract = "carpet-flight-native-json/v1";
        private static readonly string[] RequiredCapabilities =
            { "snapshot", "step", "reset", "events", "audio", "presentation_assets" };

        private delegate int JsonRead(IntPtr buffer, uint capacity, out uint length);

        [Serializable]
        internal sealed class Contract
        {
            public string engine_contract;
            public string transport_contract;
            public int transport_version;
            public string encoding;
            public string u64_counter_encoding;
            public string u64_hash_encoding;
            public string[] capabilities;
            public Limits limits;
            public string unknown_command_fields;
        }

        [Serializable]
        internal sealed class Limits
        {
            public uint maximum_asset_id_bytes;
            public uint maximum_command_json_bytes;
            public uint maximum_event_page_size;
            public uint maximum_output_json_bytes;
            public string json_length_type;
        }

        [Serializable]
        internal sealed class Identity
        {
            public string engine_contract;
            public string package_graph_hash;
            public string definition_hash;
            public string definition_revision;
            public string session_id;
            public string generation;
            public string tick;
        }

        [Serializable]
        internal sealed class Pose
        {
            public ushort x;
            public ushort y;
            public short z;
            public short yaw;
            public short display_pitch;
            public short effective_pitch;
            public short roll;
        }

        [Serializable]
        internal sealed class Motion
        {
            public short target_speed;
            public short actual_speed;
            public short strafe_speed;
        }

        [Serializable]
        internal sealed class SnapshotValue
        {
            public string tick;
            public string event_head;
            public string state_hash;
            public string conformance_hash;
            public Pose player_pose;
            public Motion player_motion;
            public int caster_mana;
            public uint objective_count;
            public string action_sequence;
        }

        [Serializable]
        internal sealed class SnapshotObservation
        {
            public Identity identity;
            public SnapshotValue value;
        }

        [Serializable]
        internal sealed class AssetReference
        {
            public string id;
            public string content_address;
        }

        [Serializable]
        internal sealed class GameEvent
        {
            public string sequence;
            public string state_hash;
            public string event_id;
            public string actor;
            public string participant;
            public AssetReference asset;
        }

        [Serializable]
        internal sealed class CompatibilityReceipt
        {
            public string contract;
            public uint event_count;
            public string event_content_sha256;
            public string event_lifecycle_sha256;
            public uint audio_count;
            public string audio_content_sha256;
            public string audio_lifecycle_sha256;
        }

        [Serializable]
        internal sealed class EventPage
        {
            public string requested_cursor;
            public string next_cursor;
            public string event_head;
            public bool has_more;
            public GameEvent[] events;
            public CompatibilityReceipt compatibility;
        }

        [Serializable]
        internal sealed class EventObservation
        {
            public Identity identity;
            public EventPage value;
        }

        [Serializable]
        internal sealed class AudioCommand
        {
            public string event_sequence;
            public string state_hash;
            public AssetReference asset;
            public int[] relative_position;
            public uint playback_rate_hz;
            public string actor;
            public string participant;
            public short sound_flags;
            public ushort volume_q15;
            public ushort pan_q16;
            public short rate_delta_percent;
            public ushort voice_index;
            public byte play_type;
            public sbyte loop_count;
            public string action;
            public byte pooled_flush_rank;
        }

        [Serializable]
        internal sealed class AudioObservation
        {
            public Identity identity;
            public AudioCommand value;
        }

        [Serializable]
        internal sealed class PresentationAssetBudgets
        {
            public uint max_bytes;
            public ushort max_frames;
            public ushort max_palette_colors;
            public uint max_pixels;
        }

        [Serializable]
        internal sealed class PresentationAssetValue
        {
            public string slot_id;
            public string content_address;
            public string schema;
            public string kind;
            public string runtime_format;
            public ushort width;
            public ushort height;
            public ushort frame_count;
            public byte[] palette_rgba8;
            public string color_contract;
            public uint frame_duration_ms;
            public string origin;
            public string coordinates;
            public string[] dependencies;
            public PresentationAssetBudgets budgets;
            public byte[] indices;
        }

        [Serializable]
        internal sealed class PresentationAssetObservation
        {
            public Identity identity;
            public PresentationAssetValue value;
        }

        [Serializable]
        private sealed class Command
        {
            public string longitudinal;
            public string lateral;
            public int roll_axis;
            public int pitch_axis;
            public bool cast_left;
            public bool cast_right;
            public bool cast_selected;
            public int look_yaw;
            public int look_pitch;
        }

        internal ulong Handle { get; }

        internal Contract NegotiatedContract { get; }

        internal SnapshotObservation Current { get; private set; }

        internal EngineSessionTransport(
            string packagePath,
            string closurePath,
            string assetCachePath,
            string workspacePath = null)
        {
            NegotiatedContract = ReadContract();
            ValidateContract(NegotiatedContract);
            if (string.IsNullOrWhiteSpace(packagePath) ||
                string.IsNullOrWhiteSpace(closurePath) ||
                string.IsNullOrWhiteSpace(assetCachePath))
            {
                throw new ArgumentException(
                    "neutral engine session requires explicit package, closure, and asset-cache paths");
            }
            var package = Encoding.UTF8.GetBytes(packagePath);
            var closure = Encoding.UTF8.GetBytes(closurePath);
            var cache = Encoding.UTF8.GetBytes(assetCachePath);
            var workspace = string.IsNullOrWhiteSpace(workspacePath)
                ? null
                : Encoding.UTF8.GetBytes(workspacePath);
            ulong handle;
            var status = workspace == null
                ? NativeSim.mc_game_session_open_v1(
                    package,
                    checked((uint)package.Length),
                    closure,
                    checked((uint)closure.Length),
                    cache,
                    checked((uint)cache.Length),
                    out handle)
                : NativeSim.mc_game_session_open_workspace_v1(
                    package,
                    checked((uint)package.Length),
                    closure,
                    checked((uint)closure.Length),
                    cache,
                    checked((uint)cache.Length),
                    workspace,
                    checked((uint)workspace.Length),
                    out handle);
            Check(status, "open neutral engine session");
            if (handle == 0)
            {
                throw new InvalidOperationException(
                    "neutral engine session returned handle zero");
            }
            Handle = handle;
            try
            {
                Current = Snapshot();
                if (ParseCounter(Current.identity.session_id, "session_id") != Handle)
                {
                    throw new InvalidOperationException(
                        "neutral session identity does not match its native handle");
                }
            }
            catch
            {
                NativeSim.mc_game_session_destroy_v1(Handle);
                throw;
            }
        }

        internal SnapshotObservation Snapshot()
        {
            var result = ReadJson<SnapshotObservation>(
                (IntPtr buffer, uint capacity, out uint length) =>
                    NativeSim.mc_game_session_snapshot_json_v1(
                        Handle, buffer, capacity, out length),
                "read neutral snapshot");
            ValidateIdentity(result.identity);
            ValidateSnapshot(result.value, result.identity);
            Current = result;
            return result;
        }

        internal SnapshotObservation Step(
            sbyte roll,
            sbyte pitch,
            sbyte flags,
            ushort lookYaw,
            ushort lookPitch)
        {
            var bits = unchecked((byte)flags);
            if ((bits & 0x80) != 0 || (bits & 0x03) == 0x03 || (bits & 0x0c) == 0x0c)
            {
                throw new ArgumentException("movement flags are outside the neutral command contract");
            }
            var command = new Command
            {
                longitudinal = (bits & 0x01) != 0
                    ? "accelerate"
                    : (bits & 0x02) != 0 ? "decelerate" : "coast",
                lateral = (bits & 0x08) != 0
                    ? "positive"
                    : (bits & 0x04) != 0 ? "negative" : "neutral",
                roll_axis = roll,
                pitch_axis = pitch,
                cast_left = (bits & 0x10) != 0,
                cast_right = (bits & 0x20) != 0,
                cast_selected = (bits & 0x40) != 0,
                look_yaw = unchecked((short)lookYaw),
                look_pitch = unchecked((short)lookPitch),
            };
            var bytes = Encoding.UTF8.GetBytes(JsonUtility.ToJson(command));
            if (bytes.Length > NegotiatedContract.limits.maximum_command_json_bytes)
            {
                throw new InvalidOperationException("managed neutral command exceeds negotiated bound");
            }
            Check(
                NativeSim.mc_game_session_step_json_v1(
                    Handle,
                    ParseCounter(Current.identity.generation, "generation"),
                    ParseCounter(Current.identity.tick, "tick"),
                    bytes,
                    checked((uint)bytes.Length)),
                "step neutral engine session");
            return Snapshot();
        }

        internal SnapshotObservation Reset()
        {
            Check(
                NativeSim.mc_game_session_reset_v1(
                    Handle,
                    ParseCounter(Current.identity.generation, "generation"),
                    ParseCounter(Current.identity.tick, "tick")),
                "reset neutral engine session");
            return Snapshot();
        }

        internal EventObservation Events(string cursor, uint limit = 1024)
        {
            if (limit > NegotiatedContract.limits.maximum_event_page_size)
            {
                throw new ArgumentOutOfRangeException(nameof(limit));
            }
            var generation = ParseCounter(Current.identity.generation, "generation");
            var tick = ParseCounter(Current.identity.tick, "tick");
            var parsedCursor = ParseCounter(cursor, "cursor");
            var result = ReadJson<EventObservation>(
                (IntPtr buffer, uint capacity, out uint length) =>
                    NativeSim.mc_game_session_events_json_v1(
                        Handle,
                        generation,
                        tick,
                        parsedCursor,
                        limit,
                        buffer,
                        capacity,
                        out length),
                "read neutral events");
            ValidateIdentity(result.identity);
            if (result.value == null || result.value.events == null ||
                result.value.compatibility == null ||
                result.value.requested_cursor != cursor)
            {
                throw new InvalidOperationException("neutral event page is incomplete");
            }
            return result;
        }

        internal AudioObservation Audio(string eventSequence)
        {
            var generation = ParseCounter(Current.identity.generation, "generation");
            var tick = ParseCounter(Current.identity.tick, "tick");
            var sequence = ParseCounter(eventSequence, "event_sequence");
            var result = ReadJson<AudioObservation>(
                (IntPtr buffer, uint capacity, out uint length) =>
                    NativeSim.mc_game_session_audio_json_v1(
                        Handle,
                        generation,
                        tick,
                        sequence,
                        buffer,
                        capacity,
                        out length),
                "read neutral audio");
            ValidateIdentity(result.identity);
            if (result.value == null || result.value.event_sequence != eventSequence ||
                result.value.asset == null || result.value.relative_position?.Length != 3)
            {
                throw new InvalidOperationException("neutral audio observation is incomplete");
            }
            return result;
        }

        internal PresentationAssetObservation PresentationAsset(string semanticId)
        {
            if (string.IsNullOrWhiteSpace(semanticId))
            {
                throw new ArgumentException("semantic asset ID is required", nameof(semanticId));
            }
            var bytes = Encoding.UTF8.GetBytes(semanticId);
            if (bytes.Length > NegotiatedContract.limits.maximum_asset_id_bytes)
            {
                throw new ArgumentOutOfRangeException(nameof(semanticId));
            }
            var result = ReadJson<PresentationAssetObservation>(
                (IntPtr buffer, uint capacity, out uint length) =>
                    NativeSim.mc_game_session_presentation_asset_json_v1(
                        Handle,
                        ParseCounter(Current.identity.generation, "generation"),
                        ParseCounter(Current.identity.tick, "tick"),
                        bytes,
                        checked((uint)bytes.Length),
                        buffer,
                        capacity,
                        out length),
                "read neutral presentation asset");
            ValidateIdentity(result.identity);
            var value = result.value;
            if (value == null || value.slot_id != semanticId || value.budgets == null ||
                value.palette_rgba8 == null || value.indices == null ||
                value.schema != "carpet-flight-indexed-sprite/v1" ||
                value.kind != "indexed_sprite" || value.runtime_format != "indexed8-rgba8/v1")
            {
                throw new InvalidOperationException("neutral presentation asset is incomplete");
            }
            return result;
        }

        internal NativeSim.SessionSnapshot CompatibilitySnapshot()
        {
            var neutral = Snapshot();
            Check(
                NativeSim.mc_native_session_snapshot(Handle, out var legacy, 120),
                "read same-state compatibility snapshot");
            var value = neutral.value;
            if (legacy.generation != ParseCounter(neutral.identity.generation, "generation") ||
                legacy.tick != ParseCounter(neutral.identity.tick, "tick") ||
                legacy.eventHead != ParseCounter(value.event_head, "event_head") ||
                legacy.stateHash != ParseHash(value.state_hash, "state_hash") ||
                legacy.conformanceHash != ParseHash(value.conformance_hash, "conformance_hash") ||
                legacy.x != value.player_pose.x || legacy.y != value.player_pose.y ||
                legacy.z != value.player_pose.z || legacy.yaw != value.player_pose.yaw ||
                legacy.displayPitch != value.player_pose.display_pitch ||
                legacy.effectivePitch != value.player_pose.effective_pitch ||
                legacy.rollAccumulator != value.player_pose.roll ||
                legacy.targetSpeed != value.player_motion.target_speed ||
                legacy.actualSpeed != value.player_motion.actual_speed ||
                legacy.strafeSpeed != value.player_motion.strafe_speed ||
                legacy.casterMana != value.caster_mana ||
                legacy.objectiveCount != value.objective_count ||
                legacy.fireSequence != ParseCounter(value.action_sequence, "action_sequence"))
            {
                throw new InvalidOperationException(
                    "neutral and same-state compatibility snapshots disagree");
            }
            return legacy;
        }

        internal void Destroy()
        {
            Check(
                NativeSim.mc_game_session_destroy_v1(Handle),
                "destroy neutral engine session");
        }

        private static Contract ReadContract()
        {
            return ReadJson<Contract>(NativeSim.mc_game_session_contract_json_v1,
                "read neutral engine contract");
        }

        internal static void ValidateContract(Contract contract)
        {
            if (contract == null ||
                contract.engine_contract != EngineContract ||
                contract.transport_contract != TransportContract ||
                contract.transport_version != 1 ||
                contract.encoding != "utf-8-json" ||
                contract.u64_counter_encoding != "decimal-string" ||
                contract.u64_hash_encoding != "lowercase-hex-16" ||
                contract.unknown_command_fields != "reject" ||
                contract.limits == null ||
                contract.limits.maximum_asset_id_bytes != 256 ||
                contract.limits.maximum_command_json_bytes != 4096 ||
                contract.limits.maximum_event_page_size != 1024 ||
                contract.limits.maximum_output_json_bytes != 16 * 1024 * 1024 ||
                contract.limits.json_length_type != "u32" ||
                contract.capabilities == null ||
                !RequiredCapabilities.SequenceEqual(contract.capabilities))
            {
                throw new InvalidOperationException(
                    "native engine contract or required capabilities are unsupported");
            }
        }

        private static void ValidateIdentity(Identity identity)
        {
            if (identity == null || identity.engine_contract != EngineContract ||
                string.IsNullOrWhiteSpace(identity.package_graph_hash) ||
                string.IsNullOrWhiteSpace(identity.definition_hash))
            {
                throw new InvalidOperationException("neutral identity is incomplete");
            }
            ParseCounter(identity.definition_revision, "definition_revision");
            ParseCounter(identity.session_id, "session_id");
            ParseCounter(identity.generation, "generation");
            ParseCounter(identity.tick, "tick");
        }

        private static void ValidateSnapshot(SnapshotValue value, Identity identity)
        {
            if (value == null || value.player_pose == null || value.player_motion == null ||
                value.tick != identity.tick)
            {
                throw new InvalidOperationException("neutral snapshot is incomplete");
            }
            ParseCounter(value.event_head, "event_head");
            ParseCounter(value.action_sequence, "action_sequence");
            ParseHash(value.state_hash, "state_hash");
            ParseHash(value.conformance_hash, "conformance_hash");
        }

        private static T ReadJson<T>(JsonRead read, string operation) where T : class
        {
            Check(read(IntPtr.Zero, 0, out var required), operation + " size", NativeSim.Bounds);
            if (required == 0 || required > 16 * 1024 * 1024)
            {
                throw new InvalidOperationException(operation + " returned an invalid byte length");
            }
            var buffer = Marshal.AllocHGlobal(checked((int)required));
            try
            {
                Check(read(buffer, required, out var written), operation);
                if (written != required)
                {
                    throw new InvalidOperationException(operation + " changed length during copy");
                }
                var bytes = new byte[checked((int)written)];
                Marshal.Copy(buffer, bytes, 0, checked((int)written));
                var value = JsonUtility.FromJson<T>(Encoding.UTF8.GetString(bytes));
                return value ?? throw new InvalidOperationException(operation + " returned null JSON");
            }
            finally
            {
                Marshal.FreeHGlobal(buffer);
            }
        }

        private static ulong ParseCounter(string value, string field)
        {
            if (string.IsNullOrEmpty(value) ||
                (value.Length > 1 && value[0] == '0') ||
                !ulong.TryParse(value, NumberStyles.None, CultureInfo.InvariantCulture, out var parsed))
            {
                throw new InvalidOperationException(field + " is not a canonical decimal u64 string");
            }
            return parsed;
        }

        private static ulong ParseHash(string value, string field)
        {
            if (value == null || value.Length != 16 ||
                value.Any(character =>
                    (character < '0' || character > '9') &&
                    (character < 'a' || character > 'f')) ||
                !ulong.TryParse(
                    value,
                    NumberStyles.AllowHexSpecifier,
                    CultureInfo.InvariantCulture,
                    out var parsed))
            {
                throw new InvalidOperationException(field + " is not a lowercase 16-digit u64 hash");
            }
            return parsed;
        }

        private static void Check(int status, string operation, int expected = NativeSim.Ok)
        {
            if (status != expected)
            {
                throw new InvalidOperationException(
                    $"{operation} failed: status={status}, expected={expected}");
            }
        }
    }
}
