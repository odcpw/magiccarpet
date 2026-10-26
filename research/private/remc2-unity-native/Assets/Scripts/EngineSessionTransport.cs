using System;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using UnityEngine;

namespace MagicCarpet.Native
{
    /// <summary>
    /// Managed transport for the neutral Rust GameSession contract. This type
    /// parses observations but owns no gameplay state; every command and read
    /// crosses the explicitly versioned native ABI. The session profile is
    /// selected by the package itself: a campaign descriptor envelope opens
    /// the campaign adapter behind the same handles and negotiated contract.
    /// </summary>
    internal sealed class EngineSessionTransport
    {
        internal const string EngineContract = "carpet-flight-game-session/v6";
        internal const string TransportContract = "carpet-flight-native-json/v6";
        internal const string OwnedProfile = "mc2-owned";
        internal const string CampaignProfile = "mc2-campaign";
        private const string CampaignDescriptorEnvelope = "campaign-definition.json";
        private static readonly string[] RequiredCapabilities =
            { "snapshot", "step", "reset", "events", "audio", "presentation_assets",
              "presentation_entities", "semantic_spell_selection", "spell_observations" };
        private static readonly string[] RequiredOwnedCapabilities =
            { "snapshot", "step", "reset", "events", "audio", "presentation_assets" };
        private static readonly string[] RequiredCampaignCapabilities =
            { "snapshot", "step", "reset", "events", "presentation_assets",
              "presentation_entities", "semantic_spell_selection", "spell_observations" };

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
            public SessionProfile[] session_profiles;
            public Limits limits;
            public string unknown_command_fields;
        }

        [Serializable]
        internal sealed class SessionProfile
        {
            public string profile;
            public string snapshot_value;
            public string[] capabilities;
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
        internal sealed class CameraPose
        {
            public ushort x;
            public ushort y;
            public short z;
            public short yaw;
            public short pitch;
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
        internal sealed class PlayerVitals
        {
            public int life;
            public int max_life;
            public int mana;
            public int max_mana;
        }

        [Serializable]
        internal sealed class ObjectiveProgress
        {
            public string current_objective;
            public uint completed_count;
            public uint total_count;
            public bool all_objectives_satisfied;
        }

        [Serializable]
        internal sealed class SpellObservation
        {
            public string id;
            public string display_name;
            public bool acquired;
            public bool selected;
            public sbyte tier;
            public byte unlocked_tier;
            public byte menu_slot;
            public int mana_cost;
            public string hand;
            public string availability;
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
        internal sealed class CampaignSnapshotValue
        {
            public string tick;
            public string effect_head;
            public string state_hash;
            public string terrain_hash;
            public string effect_journal_sha256;
            public Pose player_pose;
            public Motion player_motion;
            public PlayerVitals player_vitals;
            public SpellObservation[] player_spells;
            public ushort player_immobilized_ticks;
            public CameraPose source_camera_pose;
            public short source_camera_fov;
            public ObjectiveProgress objective_progress;
            public byte objective_textbox_ticks;
            public bool exit_sequence_active;
            public PresentationEntity[] presentation_entities;
        }

        [Serializable]
        internal sealed class CampaignSnapshotObservation
        {
            public Identity identity;
            public CampaignSnapshotValue value;
        }

        [Serializable]
        internal sealed class AssetReference
        {
            public string id;
            public string content_address;
        }

        [Serializable]
        internal sealed class PresentationPose
        {
            public ushort x;
            public ushort y;
            public short z;
            public short yaw;
            public short pitch;
        }

        [Serializable]
        internal sealed class PresentationEntity
        {
            public string actor;
            public string kind;
            public PresentationPose pose;
            public ushort logical_height;
            public AssetReference asset;
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
        private class Command
        {
            public string longitudinal;
            public string lateral;
            public int roll_axis;
            public int pitch_axis;
            public bool cast_left;
            public bool cast_right;
            public bool cast_selected;
            public bool recenter;
            public bool full_stop;
            public bool recover_player;
            public bool remove_castle_stage;
            public uint elapsed_centiseconds;
            public bool show_objective;
            public int look_yaw;
            public int look_pitch;
        }

        [Serializable]
        private sealed class SpellSelectionCommand : Command
        {
            public string select_spell;
        }

        [Serializable]
        internal sealed class SpellAssignment
        {
            public string spell_id;
            public byte tier;
            public string hand;
        }

        [Serializable]
        private sealed class SpellAssignmentCommand : Command
        {
            public SpellAssignment assign_spell;
        }

        internal ulong Handle { get; }

        internal Contract NegotiatedContract { get; }

        internal string Profile { get; }

        internal SnapshotObservation Current { get; private set; }

        internal CampaignSnapshotObservation CurrentCampaign { get; private set; }

        private Identity CurrentIdentity =>
            Profile == CampaignProfile ? CurrentCampaign.identity : Current.identity;

        internal static bool IsCampaignPackage(string packagePath)
        {
            return !string.IsNullOrWhiteSpace(packagePath) &&
                File.Exists(Path.Combine(packagePath, CampaignDescriptorEnvelope));
        }

        internal EngineSessionTransport(
            string packagePath,
            string closurePath,
            string assetCachePath,
            string workspacePath = null)
        {
            NegotiatedContract = ReadValidatedContract();
            Profile = IsCampaignPackage(packagePath) ? CampaignProfile : OwnedProfile;
            byte[] closure;
            byte[] cache;
            if (Profile == CampaignProfile)
            {
                if (!string.IsNullOrWhiteSpace(closurePath))
                {
                    throw new ArgumentException(
                        "campaign descriptor packages do not accept a closure path");
                }
                if (!string.IsNullOrWhiteSpace(workspacePath))
                {
                    throw new ArgumentException(
                        "campaign descriptor packages have no workspace-authored variant route yet");
                }
                closure = Array.Empty<byte>();
                cache = string.IsNullOrWhiteSpace(assetCachePath)
                    ? Array.Empty<byte>()
                    : Encoding.UTF8.GetBytes(assetCachePath);
            }
            else
            {
                if (string.IsNullOrWhiteSpace(packagePath) ||
                    string.IsNullOrWhiteSpace(closurePath) ||
                    string.IsNullOrWhiteSpace(assetCachePath))
                {
                    throw new ArgumentException(
                        "neutral engine session requires explicit package, closure, and asset-cache paths");
                }
                closure = Encoding.UTF8.GetBytes(closurePath);
                cache = Encoding.UTF8.GetBytes(assetCachePath);
            }
            var package = Encoding.UTF8.GetBytes(packagePath);
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
                var sessionId = Profile == CampaignProfile
                    ? CampaignSnapshot().identity.session_id
                    : Snapshot().identity.session_id;
                if (ParseCounter(sessionId, "session_id") != Handle)
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
            RequireProfile(OwnedProfile, "the game snapshot observation");
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

        internal CampaignSnapshotObservation CampaignSnapshot()
        {
            RequireProfile(CampaignProfile, "the campaign snapshot observation");
            var result = CampaignSnapshotForHandle(Handle);
            CurrentCampaign = result;
            return result;
        }

        internal SnapshotObservation Step(
            sbyte roll,
            sbyte pitch,
            sbyte flags,
            ushort lookYaw,
            ushort lookPitch)
        {
            RequireProfile(OwnedProfile, "the game step observation");
            StepNative(roll, pitch, flags, lookYaw, lookPitch);
            return Snapshot();
        }

        internal CampaignSnapshotObservation CampaignStep(
            sbyte roll,
            sbyte pitch,
            sbyte flags,
            ushort lookYaw,
            ushort lookPitch,
            string selectSpell = null,
            bool recenter = false,
            bool fullStop = false)
        {
            RequireProfile(CampaignProfile, "the campaign step observation");
            StepNative(roll, pitch, flags, lookYaw, lookPitch, selectSpell, recenter, fullStop);
            return CampaignSnapshot();
        }

        private void StepNative(
            sbyte roll,
            sbyte pitch,
            sbyte flags,
            ushort lookYaw,
            ushort lookPitch,
            string selectSpell = null,
            bool recenter = false,
            bool fullStop = false)
        {
            StepHandle(
                Handle,
                CurrentIdentity,
                roll,
                pitch,
                flags,
                lookYaw,
                lookPitch,
                NegotiatedContract.limits.maximum_command_json_bytes,
                selectSpell,
                recenter,
                fullStop);
        }

        internal static void StepHandle(
            ulong handle,
            Identity identity,
            sbyte roll,
            sbyte pitch,
            sbyte flags,
            ushort lookYaw,
            ushort lookPitch,
            uint maximumCommandJsonBytes,
            string selectSpell = null,
            bool recenter = false,
            bool fullStop = false,
            bool recoverPlayer = false,
            uint elapsedCentiseconds = 0,
            SpellAssignment assignment = null,
            bool showObjective = false,
            bool removeCastleStage = false)
        {
            ValidateIdentityForHandle(handle, identity);
            var bits = unchecked((byte)flags);
            if ((bits & 0x80) != 0 || (bits & 0x03) == 0x03 || (bits & 0x0c) == 0x0c)
            {
                throw new ArgumentException("movement flags are outside the neutral command contract");
            }
            if (assignment != null && selectSpell != null)
                throw new ArgumentException("spell selection and assignment are separate commands");
            Command command = assignment != null ? new SpellAssignmentCommand { assign_spell = assignment } : selectSpell == null
                ? new Command()
                : new SpellSelectionCommand { select_spell = selectSpell };
            command.longitudinal = (bits & 0x01) != 0
                ? "accelerate"
                : (bits & 0x02) != 0 ? "decelerate" : "coast";
            command.lateral = (bits & 0x08) != 0
                ? "positive"
                : (bits & 0x04) != 0 ? "negative" : "neutral";
            command.roll_axis = roll;
            command.pitch_axis = pitch;
            command.cast_left = (bits & 0x10) != 0;
            command.cast_right = (bits & 0x20) != 0;
            command.cast_selected = (bits & 0x40) != 0;
            command.recenter = recenter;
            command.full_stop = fullStop;
            command.recover_player = recoverPlayer;
            command.remove_castle_stage = removeCastleStage;
            command.elapsed_centiseconds = elapsedCentiseconds;
            command.show_objective = showObjective;
            command.look_yaw = unchecked((short)lookYaw);
            command.look_pitch = unchecked((short)lookPitch);
            var bytes = Encoding.UTF8.GetBytes(JsonUtility.ToJson(command));
            if (bytes.Length > maximumCommandJsonBytes)
            {
                throw new InvalidOperationException("managed neutral command exceeds negotiated bound");
            }
            Check(
                NativeSim.mc_game_session_step_json_v1(
                    handle,
                    ParseCounter(identity.generation, "generation"),
                    ParseCounter(identity.tick, "tick"),
                    bytes,
                    checked((uint)bytes.Length)),
                "step neutral engine session");
        }

        internal SnapshotObservation Reset()
        {
            RequireProfile(OwnedProfile, "the game reset observation");
            ResetNative();
            return Snapshot();
        }

        internal CampaignSnapshotObservation CampaignReset()
        {
            RequireProfile(CampaignProfile, "the campaign reset observation");
            ResetNative();
            return CampaignSnapshot();
        }

        private void ResetNative()
        {
            ResetHandle(Handle, CurrentIdentity);
        }

        internal static void ResetHandle(ulong handle, Identity identity)
        {
            ValidateIdentityForHandle(handle, identity);
            Check(
                NativeSim.mc_game_session_reset_v1(
                    handle,
                    ParseCounter(identity.generation, "generation"),
                    ParseCounter(identity.tick, "tick")),
                "reset neutral engine session");
        }

        internal EventObservation Events(string cursor, uint limit = 1024)
        {
            return EventsForHandle(
                Handle,
                CurrentIdentity,
                cursor,
                limit,
                NegotiatedContract.limits.maximum_event_page_size,
                Profile);
        }

        internal static EventObservation EventsForHandle(
            ulong handle,
            Identity identity,
            string cursor,
            uint limit,
            uint maximumEventPageSize,
            string profile)
        {
            ValidateIdentityForHandle(handle, identity);
            if (profile != OwnedProfile && profile != CampaignProfile)
            {
                throw new ArgumentException(
                    "event compatibility profile is not negotiated",
                    nameof(profile));
            }
            if (limit > maximumEventPageSize)
            {
                throw new ArgumentOutOfRangeException(nameof(limit));
            }
            var generation = ParseCounter(identity.generation, "generation");
            var tick = ParseCounter(identity.tick, "tick");
            var parsedCursor = ParseCounter(cursor, "cursor");
            var result = ReadJson<EventObservation>(
                (IntPtr buffer, uint capacity, out uint length) =>
                    NativeSim.mc_game_session_events_json_v1(
                        handle,
                        generation,
                        tick,
                        parsedCursor,
                        limit,
                        buffer,
                        capacity,
                        out length),
                "read neutral events");
            ValidateIdentityForHandle(handle, result.identity);
            if (result.value == null || result.value.events == null ||
                result.value.requested_cursor != cursor)
            {
                throw new InvalidOperationException("neutral event page is incomplete");
            }
            // The MC2-owned adapter proves retained event/audio identities via
            // its receipt; the campaign adapter retains none, and a fabricated
            // receipt must never be accepted.
            if (profile == OwnedProfile && result.value.compatibility == null)
            {
                throw new InvalidOperationException("neutral event page is incomplete");
            }
            if (profile == CampaignProfile &&
                !string.IsNullOrEmpty(result.value.compatibility?.contract))
            {
                throw new InvalidOperationException(
                    "campaign event page carries an unexpected compatibility receipt");
            }
            return result;
        }

        internal AudioObservation Audio(string eventSequence)
        {
            RequireProfile(OwnedProfile, "the positional audio capability");
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
            if (Profile != OwnedProfile && Profile != CampaignProfile)
            {
                throw new InvalidOperationException(
                    "the authored presentation-asset capability is not negotiated");
            }
            return PresentationAssetForHandle(
                Handle,
                CurrentIdentity,
                semanticId,
                NegotiatedContract.limits.maximum_asset_id_bytes);
        }

        internal static PresentationAssetObservation PresentationAssetForHandle(
            ulong handle,
            Identity identity,
            string semanticId,
            uint maximumAssetIdBytes)
        {
            ValidateIdentityForHandle(handle, identity);
            if (string.IsNullOrWhiteSpace(semanticId))
            {
                throw new ArgumentException("semantic asset ID is required", nameof(semanticId));
            }
            var bytes = Encoding.UTF8.GetBytes(semanticId);
            if (bytes.Length > maximumAssetIdBytes)
            {
                throw new ArgumentOutOfRangeException(nameof(semanticId));
            }
            var result = ReadJson<PresentationAssetObservation>(
                (IntPtr buffer, uint capacity, out uint length) =>
                    NativeSim.mc_game_session_presentation_asset_json_v1(
                        handle,
                        ParseCounter(identity.generation, "generation"),
                        ParseCounter(identity.tick, "tick"),
                        bytes,
                        checked((uint)bytes.Length),
                        buffer,
                        capacity,
                        out length),
                "read neutral presentation asset");
            ValidateIdentityForHandle(handle, result.identity);
            if (result.identity.generation != identity.generation ||
                result.identity.tick != identity.tick ||
                result.identity.package_graph_hash != identity.package_graph_hash ||
                result.identity.definition_hash != identity.definition_hash ||
                result.identity.definition_revision != identity.definition_revision)
            {
                throw new InvalidOperationException(
                    "neutral presentation asset is not bound to the current session identity");
            }
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
            RequireProfile(OwnedProfile, "the same-state compatibility projection");
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

        internal static Contract ReadValidatedContract()
        {
            var contract = ReadJson<Contract>(NativeSim.mc_game_session_contract_json_v1,
                "read neutral engine contract");
            ValidateContract(contract);
            return contract;
        }

        internal static CampaignSnapshotObservation CampaignSnapshotForHandle(ulong handle)
        {
            var result = ReadJson<CampaignSnapshotObservation>(
                (IntPtr buffer, uint capacity, out uint length) =>
                    NativeSim.mc_game_session_snapshot_json_v1(
                        handle, buffer, capacity, out length),
                "read neutral campaign snapshot");
            ValidateIdentityForHandle(handle, result.identity);
            ValidateCampaignSnapshot(result.value, result.identity);
            return result;
        }

        internal static void ValidateContract(Contract contract)
        {
            if (contract == null ||
                contract.engine_contract != EngineContract ||
                contract.transport_contract != TransportContract ||
                contract.transport_version != 6 ||
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
            ValidateProfile(
                contract, OwnedProfile, "game_snapshot/v1", RequiredOwnedCapabilities);
            ValidateProfile(
                contract,
                CampaignProfile,
                "campaign_snapshot/v5",
                RequiredCampaignCapabilities);
        }

        private static void ValidateProfile(
            Contract contract,
            string profile,
            string snapshotValue,
            string[] capabilities)
        {
            var declared = contract.session_profiles?.SingleOrDefault(
                value => value?.profile == profile);
            if (declared == null ||
                declared.snapshot_value != snapshotValue ||
                declared.capabilities == null ||
                !capabilities.SequenceEqual(declared.capabilities))
            {
                throw new InvalidOperationException(
                    $"native engine contract does not negotiate the {profile} session profile");
            }
        }

        private void RequireProfile(string required, string what)
        {
            if (Profile != required)
            {
                throw new InvalidOperationException(
                    $"{what} is not negotiated for the {Profile} session profile");
            }
        }

        internal static void ValidateIdentity(Identity identity)
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

        private static void ValidateIdentityForHandle(ulong handle, Identity identity)
        {
            ValidateIdentity(identity);
            if (handle == 0 || ParseCounter(identity.session_id, "session_id") != handle)
            {
                throw new InvalidOperationException(
                    "neutral identity does not belong to the supplied session handle");
            }
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

        internal static void ValidateCampaignSnapshot(CampaignSnapshotValue value, Identity identity)
        {
            if (value == null || value.player_pose == null || value.player_motion == null ||
                value.source_camera_pose == null ||
                value.player_vitals == null ||
                value.player_spells == null ||
                value.objective_progress == null ||
                value.presentation_entities == null ||
                value.tick != identity.tick)
            {
                throw new InvalidOperationException("neutral campaign snapshot is incomplete");
            }
            if (value.player_vitals.max_life <= 0 || value.player_vitals.max_mana <= 0 ||
                value.player_vitals.life > value.player_vitals.max_life ||
                value.player_vitals.mana > value.player_vitals.max_mana)
            {
                throw new InvalidOperationException(
                    "neutral campaign player vitals are invalid");
            }
            var objectiveProgress = value.objective_progress;
            if (objectiveProgress.completed_count > objectiveProgress.total_count ||
                (objectiveProgress.all_objectives_satisfied &&
                 !string.IsNullOrEmpty(objectiveProgress.current_objective)) ||
                (!string.IsNullOrEmpty(objectiveProgress.current_objective) &&
                 string.IsNullOrWhiteSpace(objectiveProgress.current_objective)))
            {
                throw new InvalidOperationException(
                    "neutral campaign objective progress is invalid");
            }
            ParseCounter(value.effect_head, "effect_head");
            ParseHash(value.state_hash, "state_hash");
            ParseHash(value.terrain_hash, "terrain_hash");
            var spellIds = new System.Collections.Generic.HashSet<string>();
            var selectedSpells = 0;
            foreach (var spell in value.player_spells)
            {
                if (spell == null || string.IsNullOrWhiteSpace(spell.id) ||
                    string.IsNullOrWhiteSpace(spell.display_name) ||
                    !spellIds.Add(spell.id) ||
                    (spell.hand != "none" && spell.hand != "left" &&
                     spell.hand != "right" && spell.hand != "both") ||
                    (spell.availability != "ready" && spell.availability != "not_acquired" &&
                     spell.availability != "invalid_holder" && spell.availability != "active" &&
                     spell.availability != "insufficient_mana") ||
                    (!spell.acquired && (spell.selected || spell.hand != "none" ||
                     (spell.availability != "not_acquired" && spell.availability != "invalid_holder"))) ||
                    (spell.acquired && (spell.availability == "not_acquired" ||
                     spell.availability == "invalid_holder")))
                {
                    throw new InvalidOperationException("neutral spell observation is invalid");
                }
                if (spell.selected && ++selectedSpells > 1)
                    throw new InvalidOperationException("neutral spell selection is ambiguous");
            }
            foreach (var entity in value.presentation_entities)
            {
                if (entity == null || entity.pose == null || entity.asset == null ||
                    string.IsNullOrWhiteSpace(entity.kind) ||
                    string.IsNullOrWhiteSpace(entity.asset.id) ||
                    string.IsNullOrWhiteSpace(entity.asset.content_address) ||
                    entity.logical_height == 0)
                {
                    throw new InvalidOperationException(
                        "neutral presentation entity is incomplete");
                }
                ParseCounter(entity.actor, "presentation_actor");
            }
            if (value.effect_journal_sha256 == null ||
                value.effect_journal_sha256.Length != 64 ||
                value.effect_journal_sha256.Any(character =>
                    (character < '0' || character > '9') &&
                    (character < 'a' || character > 'f')))
            {
                throw new InvalidOperationException(
                    "effect_journal_sha256 is not a lowercase SHA-256 hex receipt");
            }
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

        internal sealed class CommandException : InvalidOperationException
        {
            internal int Status { get; }

            internal CommandException(string operation, int status, int expected, string detail)
                : base($"{operation} failed: status={status}, expected={expected}: {detail}")
            {
                Status = status;
            }
        }

        private static void Check(int status, string operation, int expected = NativeSim.Ok)
        {
            if (status != expected)
            {
                var detail = NativeSession.ReadLastError();
                throw new CommandException(operation, status, expected, detail);
            }
        }
    }
}
