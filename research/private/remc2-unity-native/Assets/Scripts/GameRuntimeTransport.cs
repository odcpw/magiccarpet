using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Runtime.Serialization;
using System.Runtime.Serialization.Json;
using System.Text;
using System.Threading;
using UnityEngine;

namespace MagicCarpet.Native
{
    /// <summary>
    /// Observational transport for the Rust-owned outer game runtime. It owns
    /// no title, loading, or gameplay state; successful transitions and every
    /// identity read cross the versioned native ABI.
    /// </summary>
    internal sealed class GameRuntimeTransport : IDisposable
    {
        private static readonly bool LegacyRuntimeJson =
            Environment.GetCommandLineArgs().Contains("-mc-runtime-legacy-json");
        private static readonly RuntimeIdentityJsonDecoder IdentityJson = new(typeof(Identity));
        internal const string RuntimeContract = "carpet-flight-game-runtime/v12";
        internal const string TransportContract =
            "carpet-flight-native-game-runtime-json/v12";
        internal const string DefinitionContract =
            "carpet-flight-game-definition/v11";
        internal const string SemanticContractVersion = "36.0";
        internal const string CompletionTransitionReceiptContract =
            "carpet-flight-completion-transition-receipt/v3";
        internal const string ResultsCompletionDisposition = "results";
        internal const string ReplayMapCompletionDisposition = "replay_map";
        internal const string InstalledPackageSetContract =
            "carpet-flight-installed-package-set-bootstrap/v1";
        internal const string ReplaySaveContract =
            "carpet-flight-mc2-campaign-replay-save/v9";
        internal const int MaximumReplaySaveJsonBytes = 16 * 1024 * 1024;
        internal const string TitleMode = "title";
        internal const string CampaignMode = "campaign";
        internal const string CampaignMapPresentationPhase = "campaign_map";
        internal const string LoadingMode = "loading";
        internal const string PlayingMode = "playing";
        internal const string PausedMode = "paused";
        internal const string TitlePresentationPhase = "title";
        internal const string LoadingPresentationPhase = "loading";
        internal const string LevelEntryPresentationPhase = "level_entry";
        internal const string LevelCompletionPresentationPhase = "level_completion";
        internal const string TitlePresentationCue = "presentation.campaign.title";

        private delegate int JsonRead(IntPtr buffer, uint capacity, out uint length);

        [Serializable]
        internal sealed class Contract
        {
            public string runtime_contract;
            public string transport_contract;
            public int transport_version;
            public string definition_contract;
            public string semantic_contract_version;
            public string encoding;
            public string u64_counter_encoding;
            public string[] modes;
            public string[] transitions;
            public string playing_session_transport;
            public string package_profile;
            public InstalledPackageSetTransport installed_package_set;
            public ReplaySaveTransport replay_save;
            public PausedFrameTransport paused_frame;
            public PresentationFrameTransport presentation_frame;
            public RuntimeFlowContract runtime_flow;
            public CompletionTransitionTransport completion_transition;
            public Limits limits;
        }

        [Serializable]
        internal sealed class InstalledPackageSetTransport
        {
            public string request_contract;
            public string transport;
            public string selection_authority;
            public string input_order;
            public string lifetime;
        }

        [Serializable]
        internal sealed class ReplaySaveTransport
        {
            public string save_contract;
            public string save_transport;
            public string load_transport;
            public string warm_load_transport;
            public string load_package_authority;
            public string client_semantics;
        }

        [Serializable]
        internal sealed class PausedFrameTransport
        {
            public string transport;
            public string gameplay_tick;
            public string session_identity;
            public string deterministic_work;
            public string replay;
        }

        [Serializable]
        internal sealed class PresentationFrameTransport
        {
            public string transport, observation_transport, gameplay_tick,
                session_identity, deterministic_work, replay;
        }

        [Serializable]
        internal sealed class CompletionTransitionTransport
        {
            public string receipt_contract;
            public string accept_transport;
            public string receipt_transport;
            public string destination_authority;
            public string package_authority;
            public string presentation_sequence;
            public string[] guards;
        }

        [Serializable, DataContract]
        internal sealed class InstalledPackageResource
        {
            [DataMember] public string package_dir;
            [DataMember(EmitDefaultValue = false)] public string owner_cache_path;
            [DataMember(EmitDefaultValue = false)] public string presentation_cache_path;
            [DataMember(EmitDefaultValue = false)] public string authored_workspace_path;
        }

        [Serializable, DataContract]
        internal sealed class InstalledPackageSetRequest
        {
            [DataMember] public string request_contract;
            [DataMember] public InstalledPackageResource[] packages;
        }

        [Serializable]
        internal sealed class RuntimeFlowContract
        {
            public string title;
            public string loading;
            public string playing_and_paused;
            public string presentation_cue;
        }

        [Serializable]
        internal sealed class Limits
        {
            public uint maximum_output_json_bytes;
            public uint maximum_installed_package_set_json_bytes;
            public uint maximum_installed_package_count;
            public uint maximum_replay_save_json_bytes;
            public string json_length_type;
        }

        [Serializable]
        internal sealed class DefinitionIdentity
        {
            public string package_graph_hash;
            public string definition_hash;
            public string definition_revision;
            [OptionalField] public RuntimeFlowDefinition runtime_flow;
        }

        [Serializable]
        internal sealed class LoadingSourceSelection
        {
            public ushort external_level;
            public ushort internal_level;
            public byte map_type_id;
        }

        [Serializable]
        internal sealed class LoadingPresentationBinding
        {
            public string schema;
            public string level_id;
            public string cue_id;
            public string package_graph_hash;
            public string definition_hash;
            public string definition_revision;
            public string source_package_sha256;
            public string owner_export_package_sha256;
            public string runtime_capture_sha256;
            public LoadingSourceSelection source_selection;
            public string environment;
            public string artwork_profile;
        }

        [Serializable]
        private sealed class LoadingPresentationObservation
        {
            public Identity identity;
            public LoadingPresentationBinding value;
        }

        [Serializable]
        private sealed class ActiveEntryPresentationObservation
        {
            public Identity identity;
            public LoadingPresentationBinding value;
            public SourceAudioCapabilities source_audio_capabilities;
        }

        [DllImport("sim_capi", CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_game_runtime_loading_presentation_json_v1(
            ulong handle, ulong generation, IntPtr buffer, uint capacity, out uint length);

        [DllImport("sim_capi", CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_game_runtime_cancel_loading_v1(ulong handle, ulong generation);

        internal static LoadingPresentationBinding ReadLoadingPresentation(Identity loading)
        {
            ValidateIdentityPresentation(loading);
            if (loading.mode != LoadingMode || loading.pending_definition == null)
                throw new InvalidOperationException("loading assets require the pending Rust definition");
            var handle = ParseCounter(loading.runtime_id, "runtime_id");
            var generation = ParseCounter(loading.generation, "generation");
            var observation = ReadJson<LoadingPresentationObservation>(
                (IntPtr buffer, uint capacity, out uint length) =>
                    mc_game_runtime_loading_presentation_json_v1(handle, generation, buffer, capacity, out length),
                "read destination loading presentation");
            if (observation?.identity?.runtime_id != loading.runtime_id ||
                observation.identity.generation != loading.generation)
                throw new InvalidDataException("loading asset observation changed its runtime identity");
            ValidateLoadingPresentation(loading, observation.value);
            return observation.value;
        }

        internal static LoadingPresentationBinding ReadActiveEntryPresentation(Identity playing)
        {
            return ReadActiveEntryObservation(playing).value;
        }

        private static ActiveEntryPresentationObservation ReadActiveEntryObservation(Identity playing)
        {
            ValidateIdentityPresentation(playing);
            if (playing.mode is not (PlayingMode or PausedMode) || playing.active_runtime_flow == null ||
                !HasSessionIdentity(playing) || HasDefinitionIdentityData(playing.pending_definition))
            {
                throw new InvalidOperationException(
                    "active-entry assets require the exact Playing or Paused runtime identity");
            }
            var handle = ParseCounter(playing.runtime_id, "runtime_id");
            var generation = ParseCounter(playing.generation, "generation");
            var observation = ReadJson<ActiveEntryPresentationObservation>(
                (IntPtr buffer, uint capacity, out uint length) =>
                    NativeSim.mc_game_runtime_active_entry_presentation_json_v1(
                        handle, generation, buffer, capacity, out length),
                "read active-entry presentation");
            if (!SameIdentity(playing, observation?.identity))
            {
                throw new InvalidDataException(
                    "active-entry asset observation changed its runtime identity");
            }
            ValidateActiveEntryPresentation(playing, observation.value);
            return observation;
        }

        internal LoadingPresentationBinding BindActiveEntryPresentation(Identity playing)
        {
            var observation = ReadActiveEntryObservation(playing);
            var after = ReadIdentity(RequireHandle());
            var qualified = observation.source_audio_capabilities;
            var current = after.source_audio_capabilities;
            if (!SameIdentity(playing, after) || qualified == null || current == null ||
                qualified.sound_available != current.sound_available ||
                qualified.music_available != current.music_available)
                throw new InvalidDataException("active-entry audio capabilities do not bind the current runtime identity");
            Current = after;
            return observation.value;
        }

        internal static void ValidateActiveEntryPresentation(
            Identity playing,
            LoadingPresentationBinding binding)
        {
            ValidateIdentityPresentation(playing);
            var flow = playing?.active_runtime_flow;
            var session = playing?.session;
            if (playing?.mode is not (PlayingMode or PausedMode) || flow == null || session == null ||
                binding?.schema != "mc2-loading-presentation/v1" ||
                binding.level_id != flow.bound_level_id ||
                binding.cue_id != playing.presentation.cue_id ||
                binding.package_graph_hash != session.package_graph_hash ||
                binding.definition_hash != session.definition_hash ||
                binding.definition_revision != session.definition_revision ||
                !IsLoadingHash(binding.source_package_sha256) ||
                !IsLoadingHash(binding.owner_export_package_sha256) ||
                !IsLoadingHash(binding.runtime_capture_sha256) ||
                !IsLoadingDefinitionHash(binding.definition_hash) ||
                !IsLoadingDefinitionHash(binding.package_graph_hash) ||
                !IsLoadingPresentationSourceBinding(binding))
            {
                throw new InvalidDataException(
                    "active-entry assets do not bind the exact Playing or Paused session definition");
            }
        }

        internal static void ValidateLoadingPresentation(Identity loading, LoadingPresentationBinding binding)
        {
            ValidateIdentityPresentation(loading);
            var definition = loading.pending_definition;
            if (loading.mode != LoadingMode || definition?.runtime_flow == null ||
                binding?.schema != "mc2-loading-presentation/v1" ||
                binding.level_id != definition.runtime_flow.bound_level_id ||
                binding.cue_id != loading.presentation.cue_id ||
                binding.package_graph_hash != definition.package_graph_hash ||
                binding.definition_hash != definition.definition_hash ||
                binding.definition_revision != definition.definition_revision ||
                !IsLoadingHash(binding.source_package_sha256) ||
                !IsLoadingHash(binding.owner_export_package_sha256) ||
                !IsLoadingHash(binding.runtime_capture_sha256) ||
                !IsLoadingDefinitionHash(binding.definition_hash) || !IsLoadingDefinitionHash(binding.package_graph_hash) ||
                !IsLoadingPresentationSourceBinding(binding))
                throw new InvalidDataException("loading assets do not bind the exact pending destination definition");
        }

        private static bool IsLoadingPresentationSourceBinding(LoadingPresentationBinding binding)
        {
            var selection = binding?.source_selection;
            if (selection == null)
                return false;

            var expectedEnvironment = selection.map_type_id switch
            {
                0 => "day",
                1 => "night",
                2 => "cave",
                _ => null,
            };
            var expectedArtworkProfile = selection.internal_level > 24 && selection.internal_level < 50
                ? "secret"
                : "ordinary";
            return expectedEnvironment != null && binding.environment == expectedEnvironment &&
                binding.artwork_profile == expectedArtworkProfile;
        }

        private static bool IsLoadingHash(string value) => value?.Length == 64 &&
            value.All(c => c is >= '0' and <= '9' or >= 'a' and <= 'f');

        private static bool IsLoadingDefinitionHash(string value) =>
            value?.StartsWith("sha256:", StringComparison.Ordinal) == true && IsLoadingHash(value.Substring(7));

        internal static Identity CancelLoading(Identity loading)
        {
            var handle = ParseCounter(loading.runtime_id, "runtime_id");
            var generation = ParseCounter(loading.generation, "generation");
            Check(mc_game_runtime_cancel_loading_v1(handle, generation), "cancel loading");
            var after = ReadIdentity(handle);
            if (after.runtime_id != loading.runtime_id ||
                ParseCounter(after.generation, "generation") != checked(generation + 1) ||
                after.mode is not (TitleMode or CampaignMode) || HasDefinitionIdentityData(after.pending_definition) ||
                !string.IsNullOrEmpty(after.session?.session_id) ||
                !SameSecretCampaign(loading.secret_campaign, after.secret_campaign))
                throw new InvalidOperationException("loading cancellation did not return an unbound frontend owner");
            return after;
        }

        [Serializable]
        internal sealed class RuntimeFlowDestination
        {
            public string kind;
            [OptionalField] public string level_id;
            [OptionalField] public string node_id;
        }

        [Serializable]
        internal sealed class RuntimeFlowEntry
        {
            public string level_id;
            public RuntimeFlowDestination completion_destination;
        }

        [Serializable]
        internal sealed class RuntimeFlowSecretEntry
        {
            public string origin_level_id;
            public string secret_level_id;
        }

        [Serializable]
        internal sealed class RuntimeFlowDefinition
        {
            public string game_id;
            public string bound_level_id;
            public string entry_level_id;
            public RuntimeFlowEntry[] entries;
            [OptionalField] public RuntimeFlowSecretEntry[] secret_entries;
        }

        [Serializable]
        internal sealed class SecretCampaignSnapshot
        {
            public string[] discovered_levels;
            public string[] completed_levels;
            [OptionalField] public CompletedLevelStatistics[] level_statistics;
            [OptionalField] public string return_origin;
        }

        [Serializable]
        internal sealed class DisplaySettings
        {
            public bool minimap, top_bar, panels_opaque, flat_shading;
            public byte viewport_size;
        }

        [Serializable]
        internal sealed class SourceAudioCapabilities
        {
            public bool sound_available, music_available;
        }

        internal enum SourceViewportAction : byte
        {
            MenuDecrease = 0, MenuIncrease = 1, KeyboardEnlarge = 2, KeyboardShrink = 3
        }

        [Serializable]
        internal sealed class Identity
        {
            public string runtime_contract;
            public string runtime_id;
            public string generation;
            public string mode;
            public byte source_speed_index;
            public bool source_player_names_visible;
            [OptionalField] public SourceAudioCapabilities source_audio_capabilities;
            [OptionalField] public MediaSnapshot frontend_media;
            public PlayerPreferencesSnapshot player_preferences;
            public DisplaySettings display_settings;
            public RuntimePresentationCue presentation;
            [OptionalField] public CampaignMenuSnapshot campaign_menu;
            [OptionalField] public DefinitionIdentity pending_definition;
            [OptionalField] public RuntimeFlowDefinition active_runtime_flow;
            [OptionalField] public EngineSessionTransport.Identity session;
            [OptionalField] public SecretCampaignSnapshot secret_campaign;
        }

        [Serializable]
        internal sealed class MediaSnapshot
        {
            public string contract;
            public uint epoch;
            public bool active;
            public string clip_id;
            // A negative frame index means the Rust owner has no visible frame.
            public int frame_index;
            public int fade_in_level;
            public int fade_out_level;
            public string frames_path;
            public int width;
            public int height;
            public string subtitle;
            public int subtitle_viewport_offset;
            public MediaAction[] actions;
            public string error;
            public bool clear_input;
            public bool retain_previous_frame;
            [OptionalField] public CampaignReturnMediaSnapshot campaign_return;
        }

        [Serializable]
        internal sealed class CampaignReturnMediaSnapshot
        {
            public string phase;
            public bool blocking;
            public int? cutscene_id;
            [OptionalField] public CreditsSnapshot credits;
        }

        [Serializable]
        internal sealed class CreditsSnapshot
        {
            public string[] lines;
            public int cursor;
        }

        [Serializable]
        internal sealed class MediaAction
        {
            public string op;
            public int value;
            public string asset_path;
            public int? volume_units;
        }

        [DllImport("sim_capi", CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_game_runtime_preferences_command_json_v1(
            ulong handle, ulong generation, byte[] request, uint length);

        private const int SourceActionBlocked = 16;
        internal enum SourceBrightnessAction { Cycle, Increment, Decrement }
        internal enum SourceAudioAction { Sound, Music, Speech }

        [Serializable]
        private sealed class SourceBrightnessGuard
        {
            public string session_id, generation, expected_tick;
        }

        [Serializable]
        private sealed class SourceBrightnessCommand
        {
            public string operation = "source_brightness";
            public SourceBrightnessGuard session_guard;
            public string action;
        }

        [Serializable]
        private sealed class SourceAudioCommand
        {
            public string operation = "source_audio";
            public SourceBrightnessGuard session_guard;
            public string action;
        }

        [Serializable]
        private sealed class SourceSpeechCommand
        {
            public string operation = "source_audio";
            public SourceBrightnessGuard session_guard;
            public string action = "speech";
            public bool speech_available;
        }

        [Serializable]
        private sealed class SourceFlightAssistanceCommand
        {
            public string operation = "source_flight_assistance";
            public SourceBrightnessGuard session_guard;
        }

        internal static PlayerPreferencesSnapshot PreferencesCommand(
            ulong handle, Identity identity, string json)
        {
            var bytes = Encoding.UTF8.GetBytes(json);
            Check(mc_game_runtime_preferences_command_json_v1(handle,
                ParseCounter(identity.generation, "generation"), bytes,
                checked((uint)bytes.Length)), "apply player preferences");
            return ReadIdentity(handle).player_preferences;
        }

        [DllImport("sim_capi", CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_game_runtime_media_command_json_v1(
            ulong handle, ulong generation, byte[] request, uint length);

        internal static void MediaCommand(ulong handle, Identity identity, string json)
        {
            var bytes = Encoding.UTF8.GetBytes(json);
            Check(mc_game_runtime_media_command_json_v1(handle,
                ParseCounter(identity.generation, "generation"), bytes,
                checked((uint)bytes.Length)), "advance frontend media");
        }

        [Serializable]
        internal sealed class CampaignSpeechSegment
        {
            public int track, segment;
            public uint start_ms, length_ms;
        }

        [Serializable]
        internal sealed class CompletionStatistics
        {
            public int spells_found_percent, hit_accuracy_percent, creatures_killed_percent, mana_collected_percent;
            public uint time_taken_seconds;
        }

        [Serializable]
        internal sealed class CompletedLevelStatistics
        {
            public string level_id;
            public CompletionStatistics statistics;
        }

        [Serializable]
        internal sealed class CampaignMenuSnapshot
        {
            public string phase;
            [OptionalField] public string completed_level_id;
            public string[] completed_levels;
            public string[] available_levels;
            [OptionalField] public string selected_level_id;
            [OptionalField] public string briefing_level_id;
            [OptionalField] public CampaignSpeechSegment briefing_speech;
            [OptionalField] public CompletionStatistics completion_statistics;
            [OptionalField] public CompletedLevelStatistics[] level_statistics;
        }

        [Serializable]
        internal sealed class CampaignMenuCommand
        {
            public string action;
            public string level_id;
        }

        [Serializable]
        private sealed class CampaignActionCommand
        {
            public string action;
        }

        [Serializable]
        internal sealed class RuntimePresentationCue
        {
            public string cue_id;
            public string phase;
            [OptionalField] public string level_id;
            [OptionalField] public RuntimeFlowDestination completion_destination;
        }

        [Serializable]
        internal sealed class GameCompletion
        {
            public string state_hash;
        }

        [Serializable]
        internal sealed class CompletionTransitionReceipt
        {
            public string receipt_contract;
            public Identity before;
            public Identity after;
            public EngineSessionTransport.Identity source_session;
            public string completed_level_id;
            public RuntimeFlowDestination completion_destination;
            public GameCompletion completion;
            public string disposition;
            public RuntimePresentationCue[] presentation_sequence;
            public string receipt_sha256;
        }

        internal sealed class TransitionException : InvalidOperationException
        {
            internal int Status { get; }

            internal TransitionException(string operation, int status)
                : base($"{operation} failed with native status {status}: {NativeSession.ReadLastError()}")
            {
                Status = status;
            }
        }

        private long handleBits;

        internal ulong Handle => unchecked((ulong)Interlocked.Read(ref handleBits));

        internal Contract NegotiatedContract { get; }

        internal EngineSessionTransport.Contract NegotiatedSessionContract { get; }

        internal Identity Current { get; private set; }
        private readonly CampaignRenderTransport renderTransport = new();
        private static readonly bool JsonRenderObservations =
            Array.IndexOf(Environment.GetCommandLineArgs(), "-mc-render-json") >= 0;

        internal GameRuntimeTransport()
        {
            NegotiatedContract = ReadContract();
            ValidateContract(NegotiatedContract);
            NegotiatedSessionContract = EngineSessionTransport.ReadValidatedContract();
            Check(NativeSim.mc_game_runtime_create_v1(out var handle), "create game runtime");
            if (handle == 0 || handle > long.MaxValue)
            {
                if (handle != 0)
                {
                    NativeSim.mc_game_runtime_destroy_v1(handle);
                }
                throw new InvalidOperationException("game runtime returned an invalid handle");
            }
            Interlocked.Exchange(ref handleBits, checked((long)handle));
            try
            {
                Current = IdentitySnapshot();
                if (Current.mode != TitleMode || ParseCounter(Current.generation, "generation") != 1)
                {
                    throw new InvalidOperationException("new game runtime did not begin at Title");
                }
            }
            catch
            {
                try
                {
                    Dispose();
                }
                catch
                {
                    // Preserve the constructor failure; a native teardown
                    // diagnostic must not replace the binding error.
                }
                throw;
            }
        }

        internal Identity IdentitySnapshot([System.Runtime.CompilerServices.CallerMemberName] string caller = null)
        {
            var handle = RequireHandle();
            Current = ReadIdentity(handle, caller);
            return Current;
        }

        internal static Identity ReadIdentity(ulong handle,
            [System.Runtime.CompilerServices.CallerMemberName] string caller = null)
        {
            ModernCapture.CountIdentityRead(caller);
            var nativeStart = ModernCapture.BeginMeasurement();
            var bytes = ReadBytes(
                (IntPtr buffer, uint capacity, out uint length) =>
                    NativeSim.mc_game_runtime_identity_json_v1(
                        handle, buffer, capacity, out length),
                16 * 1024 * 1024,
                "read game runtime identity");
            ModernCapture.EndMeasurement("runtime_native_Identity", nativeStart);
            // Unity JsonUtility materializes empty nested DTOs for JSON null.
            // Identity nulls are authoritative: no session on a map, no stats
            // before completion, and no speech on a results screen. Preserve
            // those tokens rather than treating fabricated zero stats as real.
            var decodeStart = ModernCapture.BeginMeasurement();
            var identity = DecodeRuntimeJson<Identity>(bytes, LegacyRuntimeJson);
            ModernCapture.EndMeasurement("runtime_decode_Identity", decodeStart);
            ValidateIdentity(identity, handle);
            return identity;
        }

        internal Identity BootstrapPackageSet(params InstalledPackageResource[] packages)
        {
            var request = EncodePackageSet(packages);
            Check(
                NativeSim.mc_game_runtime_bootstrap_package_set_json_v1(
                    RequireHandle(),
                    ParseCounter(Current.generation, "generation"),
                    request,
                    checked((uint)request.Length)),
                "bootstrap installed package set");
            var identity = IdentitySnapshot();
            if (identity.mode != LoadingMode || !HasDefinitionIdentity(identity) ||
                HasSessionIdentityData(identity.session) ||
                HasRuntimeFlowData(identity.active_runtime_flow))
            {
                throw new InvalidOperationException("new game did not enter semantic Loading");
            }
            return identity;
        }

        internal byte[] SaveReplay()
        {
            var frontend = Current?.mode is CampaignMode or TitleMode;
            if (!frontend) RequireSessionMode(allowPaused: true, "save replay");
            var before = Current;
            var bytes = ReadBytes(
                (IntPtr buffer, uint capacity, out uint length) =>
                    NativeSim.mc_game_runtime_save_replay_json_v1(
                        RequireHandle(),
                        ParseCounter(before.generation, "generation"),
                        frontend ? 0 : ParseCounter(before.session.generation, "session_generation"),
                        frontend ? 0 : ParseCounter(before.session.tick, "tick"),
                        buffer,
                        capacity,
                        out length),
                NegotiatedContract.limits.maximum_replay_save_json_bytes,
                "save game replay");
            var after = IdentitySnapshot();
            if (after.mode != before.mode || after.generation != before.generation ||
                (frontend ? HasSessionIdentityData(after.session) ||
                    before.mode == CampaignMode && JsonUtility.ToJson(after.campaign_menu) != JsonUtility.ToJson(before.campaign_menu) :
                    after.session.generation != before.session.generation ||
                    after.session.tick != before.session.tick ||
                    !PreservesSessionBinding(before.session, after.session)) ||
                !SameSecretCampaign(before.secret_campaign, after.secret_campaign) ||
                !SameRuntimeFlow(before.active_runtime_flow, after.active_runtime_flow))
            {
                throw new InvalidOperationException("saving replay mutated the game runtime");
            }
            return bytes;
        }

        internal Identity LoadReplay(
            byte[] saveJson,
            params InstalledPackageResource[] packages)
        {
            return LoadReplayFrom(null, saveJson, packages);
        }

        internal Identity LoadReplayFrom(
            Identity source,
            byte[] saveJson,
            params InstalledPackageResource[] packages)
        {
            if (saveJson == null || saveJson.Length == 0 ||
                (uint)saveJson.Length >
                    NegotiatedContract.limits.maximum_replay_save_json_bytes)
            {
                throw new ArgumentException(
                    "replay save must be nonempty and within the negotiated bound",
                    nameof(saveJson));
            }
            if (Current?.mode != TitleMode)
            {
                throw new InvalidOperationException("replay load requires Title mode");
            }
            var packageSet = EncodePackageSet(packages);
            var beforeGeneration = ParseCounter(Current.generation, "generation");
            var status = source == null
                ? NativeSim.mc_game_runtime_load_replay_json_v1(
                    RequireHandle(),
                    beforeGeneration,
                    packageSet,
                    checked((uint)packageSet.Length),
                    saveJson,
                    checked((uint)saveJson.Length))
                : NativeSim.mc_game_runtime_load_replay_from_runtime_json_v1(
                    RequireHandle(), beforeGeneration,
                    ParseCounter(source.runtime_id, "source runtime_id"),
                    ParseCounter(source.generation, "source generation"),
                    packageSet, checked((uint)packageSet.Length), saveJson, checked((uint)saveJson.Length));
            Check(status, "load game replay");
            var identity = IdentitySnapshot();
            var loadedCampaign = identity.mode == CampaignMode;
            if ((!loadedCampaign && identity.mode != PlayingMode && identity.mode != PausedMode) ||
                ParseCounter(identity.generation, "generation") !=
                    checked(beforeGeneration + 1) ||
                HasDefinitionIdentityData(identity.pending_definition) ||
                (loadedCampaign ? HasSessionIdentityData(identity.session) || identity.campaign_menu == null :
                    !HasSessionIdentity(identity) || ParseCounter(identity.session.session_id, "session_id") != Handle) ||
                !HasRuntimeFlowData(identity.active_runtime_flow))
            {
                throw new InvalidOperationException(
                    "replay load did not enter a bound gameplay session or retained campaign map");
            }
            return identity;
        }

        internal Identity OpenLevel()
        {
            var pendingFlow = Current?.pending_definition?.runtime_flow;
            Check(
                NativeSim.mc_game_runtime_open_level_v1(
                    RequireHandle(),
                    ParseCounter(Current.generation, "generation")),
                "open pending level");
            var identity = IdentitySnapshot();
            if (identity.mode != PlayingMode ||
                HasDefinitionIdentityData(identity.pending_definition) ||
                !HasSessionIdentity(identity) ||
                ParseCounter(identity.session.session_id, "session_id") != Handle ||
                !SameRuntimeFlow(pendingFlow, identity.active_runtime_flow))
            {
                throw new InvalidOperationException("open level did not enter bound Playing");
            }
            return identity;
        }

        internal Identity CampaignMenuAction(string action, string levelId = null)
        {
            if (Current?.mode != CampaignMode)
                throw new InvalidOperationException("campaign action requires a Rust campaign owner");
            var generation = ParseCounter(Current.generation, "generation");
            // Omit level_id on commands that do not select a mission: Rust
            // rejects extra fields instead of treating them as destinations.
            var request = levelId == null
                ? Encoding.UTF8.GetBytes(JsonUtility.ToJson(new CampaignActionCommand { action = action }))
                : Encoding.UTF8.GetBytes(JsonUtility.ToJson(new CampaignMenuCommand { action = action, level_id = levelId }));
            Check(NativeSim.mc_game_runtime_campaign_menu_command_json_v1(RequireHandle(), generation, request, checked((uint)request.Length)), "campaign menu action");
            var identity = IdentitySnapshot();
            if (ParseCounter(identity.generation, "generation") != checked(generation + 1) || HasSessionIdentityData(identity.session))
                throw new InvalidOperationException("campaign action did not advance its guarded owner");
            return identity;
        }

        internal SecretTransitionTransport.AcceptedBoundaryReceipt FinishMissionIfReady()
        {
            RequireSessionMode(allowPaused: false, "finish mission boundary");
            var before = Current;
            var generation = ParseCounter(before.generation, "generation");
            Check(NativeSim.mc_game_runtime_finish_mission_if_ready_result_v1(RequireHandle(), generation,
                ParseCounter(before.session.generation, "session_generation"), ParseCounter(before.session.tick, "tick"),
                out var changed), "finish mission boundary");
            if (changed == 0) return null;
            if (changed != 1) throw new InvalidDataException("Invalid mission boundary transition flag");
            var after = IdentitySnapshot();
            if (SameIdentity(before, after))
                throw new InvalidDataException("Mission boundary reported a transition without changing identity");
            var maximumLength = NegotiatedContract.limits.maximum_output_json_bytes;
            var bytes = ReadBytes(
                (IntPtr buffer, uint capacity, out uint length) =>
                    NativeSim.mc_game_runtime_boundary_transition_receipt_json_v1(
                        RequireHandle(), buffer, capacity, out length),
                maximumLength,
                "read accepted runtime boundary receipt");
            var receipt = SecretTransitionTransport.Decode(bytes, maximumLength);
            if (receipt.kind == SecretTransitionTransport.OrdinaryCompletionBoundaryKind)
            {
                ValidateCompletionTransitionReceipt(
                    receipt.ordinary_completion, before, after, generation);
            }
            else
            {
                SecretTransitionTransport.ValidateSecretTransition(receipt, before, after);
            }
            return receipt;
        }

        internal Identity RestartNewGame()
        {
            var loading = BeginRestartNewGame();
            var playing = OpenLevel();
            if (ParseCounter(playing.generation, "generation") !=
                    checked(ParseCounter(loading.generation, "generation") + 1) ||
                !SameRuntimeFlow(loading.pending_definition.runtime_flow, playing.active_runtime_flow))
                throw new InvalidOperationException("restart did not open the resolver-owned entry session");
            return playing;
        }

        internal Identity BeginRestartNewGame()
        {
            if (Current?.mode != TitleMode ||
                HasDefinitionIdentityData(Current.pending_definition) ||
                HasSessionIdentityData(Current.session) ||
                HasRuntimeFlowData(Current.active_runtime_flow))
            {
                throw new InvalidOperationException(
                    "restart new game requires an unbound Rust Title");
            }
            var beforeRuntimeGeneration = ParseCounter(Current.generation, "generation");
            Check(
                NativeSim.mc_game_runtime_restart_new_game_v1(
                    RequireHandle(), beforeRuntimeGeneration),
                "restart resolver-owned new game");
            var loading = IdentitySnapshot();
            if (loading.mode != LoadingMode ||
                ParseCounter(loading.generation, "generation") !=
                    checked(beforeRuntimeGeneration + 1) ||
                !HasDefinitionIdentity(loading) ||
                HasSessionIdentityData(loading.session) ||
                HasRuntimeFlowData(loading.active_runtime_flow) ||
                loading.pending_definition.runtime_flow.bound_level_id !=
                    loading.pending_definition.runtime_flow.entry_level_id)
            {
                throw new InvalidOperationException(
                    "restart did not enter the resolver-owned entry Loading state");
            }
            return loading;
        }

        internal Identity ShowNewGameMap()
        {
            if (Current?.mode != LoadingMode)
                throw new InvalidOperationException("new-game map requires the resolver entry loading state");
            Check(NativeSim.mc_game_runtime_show_new_game_map_v1(RequireHandle(),
                ParseCounter(Current.generation, "generation")), "show source new-game map");
            var map = IdentitySnapshot();
            if (map.mode != CampaignMode || map.campaign_menu.phase != "mission_select" ||
                !string.IsNullOrEmpty(map.campaign_menu.completed_level_id) ||
                map.campaign_menu.completed_levels.Length != 0 ||
                map.campaign_menu.selected_level_id != map.active_runtime_flow.entry_level_id)
                throw new InvalidOperationException("new-game map must precede all earned completions");
            return map;
        }

        internal Identity Pause()
        {
            return SetPaused(
                PlayingMode,
                PausedMode,
                NativeSim.mc_game_runtime_pause_v1,
                "pause game runtime");
        }

        internal Identity Resume()
        {
            return SetPaused(
                PausedMode,
                PlayingMode,
                NativeSim.mc_game_runtime_resume_v1,
                "resume game runtime");
        }

        internal CompletionTransitionReceipt AcceptCompletionTransition()
        {
            RequireSessionMode(allowPaused: false, "accept completion transition");
            var before = Current;
            var beforeRuntimeGeneration = ParseCounter(before.generation, "generation");
            Check(
                NativeSim.mc_game_runtime_accept_transition_v1(
                    RequireHandle(),
                    beforeRuntimeGeneration,
                    ParseCounter(before.session.generation, "session_generation"),
                    ParseCounter(before.session.tick, "tick")),
                "accept completion transition");
            var receipt = ReadJson<CompletionTransitionReceipt>(
                (IntPtr buffer, uint capacity, out uint length) =>
                    NativeSim.mc_game_runtime_completion_transition_receipt_json_v1(
                        RequireHandle(), buffer, capacity, out length),
                "read completion transition receipt");
            var after = IdentitySnapshot();
            ValidateCompletionTransitionReceipt(
                receipt,
                before,
                after,
                beforeRuntimeGeneration);
            return receipt;
        }

        internal EngineSessionTransport.CampaignSnapshotObservation CampaignPausedFrame(uint elapsedCentiseconds = 0, bool showObjective = false)
        {
            CampaignPausedRenderFrame(elapsedCentiseconds, showObjective);
            return CampaignSnapshot();
        }

        internal EngineSessionTransport.CampaignRenderObservation CampaignPausedRenderFrame(uint elapsedCentiseconds = 0, bool showObjective = false)
        {
            RequireSessionMode(allowPaused: true, "campaign paused frame");
            if (Current.mode != PausedMode)
            {
                throw new InvalidOperationException(
                    "campaign paused frame requires Paused mode");
            }
            var before = Current;
            Check(
                NativeSim.mc_game_runtime_paused_frame_timed_v1(
                    RequireHandle(),
                    ParseCounter(before.generation, "generation"),
                    ParseCounter(before.session.generation, "session_generation"),
                    ParseCounter(before.session.tick, "tick"), elapsedCentiseconds, showObjective ? (byte)1 : (byte)0),
                "service game runtime paused frame");
            if (!JsonRenderObservations)
                return ReadRenderPublication(before, ParseCounter(before.session.tick, "tick")).Observation;
            var identity = IdentitySnapshot();
            if (identity.mode != PausedMode ||
                identity.generation != before.generation ||
                !PreservesSessionBinding(before.session, identity.session) ||
                identity.session.generation != before.session.generation ||
                identity.session.tick != before.session.tick ||
                !SameRuntimeFlow(before.active_runtime_flow, identity.active_runtime_flow) ||
                !SameSecretCampaign(before.secret_campaign, identity.secret_campaign))
            {
                throw new InvalidOperationException(
                    "paused frame mutated the runtime or gameplay session identity");
            }
            return JsonRenderObservation();
        }

        internal Identity Close() => Close(out _);

        internal Identity Close(out SecretTransitionTransport.Receipt secretReturn)
        {
            secretReturn = null;
            if ((Current?.mode != PlayingMode && Current?.mode != PausedMode) ||
                !HasSessionIdentity(Current))
            {
                throw new InvalidOperationException(
                    "close game runtime requires a bound Playing or Paused session");
            }
            var before = Current;
            var beforeRuntimeGeneration = ParseCounter(before.generation, "generation");
            Check(
                NativeSim.mc_game_runtime_close_v1(
                    RequireHandle(),
                    beforeRuntimeGeneration,
                    ParseCounter(before.session.generation, "session_generation"),
                    ParseCounter(before.session.tick, "tick")),
                "close game runtime");
            var identity = IdentitySnapshot();
            if (identity.mode == CampaignMode)
            {
                var maximumLength = NegotiatedContract.limits.maximum_output_json_bytes;
                var bytes = ReadBytes(
                    (IntPtr buffer, uint capacity, out uint length) =>
                        NativeSim.mc_game_runtime_boundary_transition_receipt_json_v1(
                            RequireHandle(), buffer, capacity, out length),
                    maximumLength, "read abandoned secret return receipt");
                var envelope = SecretTransitionTransport.Decode(bytes, maximumLength);
                var receipt = SecretTransitionTransport.ValidateSecretTransition(envelope, before, identity);
                if (receipt.kind != SecretTransitionTransport.AbandonedReturnKind)
                    throw new InvalidDataException("close did not return an abandoned-secret receipt");
                secretReturn = receipt;
                return identity;
            }
            if (identity.mode != TitleMode ||
                ParseCounter(identity.generation, "generation") !=
                    checked(beforeRuntimeGeneration + 1) ||
                HasDefinitionIdentityData(identity.pending_definition) ||
                HasSessionIdentityData(identity.session) ||
                HasRuntimeFlowData(identity.active_runtime_flow))
            {
                throw new InvalidOperationException(
                    "close game runtime did not enter an unbound Rust Title");
            }
            return identity;
        }

        internal EngineSessionTransport.CampaignSnapshotObservation CampaignSnapshot()
        {
            RequireSessionMode(allowPaused: true, "campaign snapshot");
            return ValidateCampaignObservation(
                EngineSessionTransport.CampaignSnapshotForHandle(RequireHandle()));
        }

        internal EngineSessionTransport.CampaignRenderObservation RenderObservation()
        {
            RequireSessionMode(allowPaused: true, "campaign render observation");
            if (!JsonRenderObservations)
                return ReadRenderPublication(Current, ParseCounter(Current.session.tick, "tick")).Observation;
            return JsonRenderObservation();
        }

        private EngineSessionTransport.CampaignRenderObservation JsonRenderObservation()
        {
            var observation = EngineSessionTransport.CampaignRenderObservationForHandle(RequireHandle());
            if (!PreservesSessionBinding(Current.session, observation.identity) ||
                observation.identity.generation != Current.session.generation ||
                observation.identity.tick != Current.session.tick)
                throw new InvalidOperationException("campaign render observation is not identity-bound");
            return observation;
        }

        private CampaignRenderTransport.Publication ReadRenderPublication(Identity before, ulong tick)
        {
            var publication = renderTransport.Read(RequireHandle(),
                ParseCounter(before.generation, "generation"),
                ParseCounter(before.session.generation, "session_generation"), tick);
            var observed = publication.Observation.identity;
            if (publication.RuntimeId != ParseCounter(before.runtime_id, "runtime_id") ||
                publication.RuntimeGeneration != ParseCounter(before.generation, "generation") ||
                publication.Mode != before.mode ||
                !PreservesSessionBinding(before.session, observed) ||
                observed.generation != before.session.generation ||
                ParseCounter(observed.tick, "tick") != tick)
                throw new InvalidDataException("Render publication changed its bound runtime or session");
            return publication;
        }

        private void PublishCampaignIdentity(Identity before, EngineSessionTransport.Identity sessionIdentity)
        {
            // Ordinary steps change the committed session tick. Other metadata
            // is changed only by explicit guarded control operations, which
            // retain their fresh full identity reads.
            Current = new Identity {
                runtime_contract = before.runtime_contract, runtime_id = before.runtime_id,
                generation = before.generation, mode = before.mode,
                source_speed_index = before.source_speed_index,
                source_player_names_visible = before.source_player_names_visible,
                source_audio_capabilities = before.source_audio_capabilities,
                frontend_media = before.frontend_media, player_preferences = before.player_preferences,
                display_settings = before.display_settings,
                presentation = before.presentation, campaign_menu = before.campaign_menu,
                pending_definition = before.pending_definition, active_runtime_flow = before.active_runtime_flow,
                session = sessionIdentity, secret_campaign = before.secret_campaign
            };
        }

        internal EngineSessionTransport.ParticipantNotificationsObservation ParticipantNotifications()
        {
            RequireSessionMode(allowPaused: true, "participant notifications");
            var observation = ReadJson<EngineSessionTransport.ParticipantNotificationsObservation>(
                (IntPtr buffer, uint capacity, out uint length) =>
                    NativeSim.mc_game_runtime_participant_notifications_json_v1(
                        RequireHandle(), buffer, capacity, out length),
                "read participant notifications");
            if (observation?.identity == null || observation.value == null ||
                observation.identity.session_id != Current.session.session_id ||
                observation.identity.generation != Current.session.generation ||
                observation.identity.tick != Current.session.tick)
                throw new InvalidDataException("participant notifications are not identity-bound");
            return observation;
        }

        internal EngineSessionTransport.ParticipantNotificationsObservation PresentationFrame(bool notificationsVisible)
        {
            RequireSessionMode(allowPaused: true, "presentation frame");
            var before = Current;
            // This status-only call mutates once. The bounded JSON getter below
            // is read-only, including its capacity probe.
            Check(NativeSim.mc_game_runtime_presentation_frame_v1(
                RequireHandle(), ParseCounter(before.generation, "generation"),
                ParseCounter(before.session.generation, "session_generation"),
                ParseCounter(before.session.tick, "tick"), notificationsVisible ? (byte)1 : (byte)0),
                "service presentation frame");
            return ParticipantNotifications();
        }

        internal Identity ToggleIconsAndMap()
        {
            RequireSessionMode(allowPaused: true, "toggle icons and map");
            var before = Current;
            Check(NativeSim.mc_game_runtime_toggle_icons_map_v1(RequireHandle(),
                ParseCounter(before.generation, "generation"),
                ParseCounter(before.session.generation, "session_generation"),
                ParseCounter(before.session.tick, "tick")), "toggle icons and map");
            var after = IdentitySnapshot();
            if (after.generation != before.generation || after.mode != before.mode ||
                !PreservesSessionBinding(before.session, after.session) ||
                after.session.generation != before.session.generation || after.session.tick != before.session.tick ||
                after.source_speed_index != before.source_speed_index ||
                after.display_settings.minimap == before.display_settings.minimap ||
                after.display_settings.top_bar == before.display_settings.top_bar ||
                after.display_settings.panels_opaque != before.display_settings.panels_opaque ||
                after.display_settings.flat_shading != before.display_settings.flat_shading ||
                after.display_settings.viewport_size != before.display_settings.viewport_size ||
                after.source_player_names_visible != before.source_player_names_visible)
                throw new InvalidDataException("icons and map toggle changed the wrong source state");
            return after;
        }

        internal Identity TogglePanelTransparency()
        {
            RequireSessionMode(allowPaused: true, "toggle panel transparency");
            var before = Current;
            Check(NativeSim.mc_game_runtime_toggle_panel_transparency_v1(RequireHandle(),
                ParseCounter(before.generation, "generation"),
                ParseCounter(before.session.generation, "session_generation"),
                ParseCounter(before.session.tick, "tick")), "toggle panel transparency");
            var after = IdentitySnapshot();
            if (after.generation != before.generation || after.mode != before.mode ||
                !PreservesSessionBinding(before.session, after.session) ||
                after.session.generation != before.session.generation || after.session.tick != before.session.tick ||
                after.source_speed_index != before.source_speed_index ||
                after.display_settings.minimap != before.display_settings.minimap ||
                after.display_settings.top_bar != before.display_settings.top_bar ||
                after.display_settings.panels_opaque == before.display_settings.panels_opaque ||
                after.display_settings.flat_shading != before.display_settings.flat_shading ||
                after.display_settings.viewport_size != before.display_settings.viewport_size ||
                after.source_player_names_visible != before.source_player_names_visible)
                throw new InvalidDataException("panel transparency toggle changed the wrong source state");
            return after;
        }

        internal Identity ToggleFlatShading()
        {
            RequireSessionMode(allowPaused: true, "toggle flat shading");
            var before = Current;
            Check(NativeSim.mc_game_runtime_toggle_flat_shading_v1(RequireHandle(),
                ParseCounter(before.generation, "generation"),
                ParseCounter(before.session.generation, "session_generation"),
                ParseCounter(before.session.tick, "tick")), "toggle flat shading");
            var after = IdentitySnapshot();
            if (after.generation != before.generation || after.mode != before.mode ||
                !PreservesSessionBinding(before.session, after.session) ||
                after.session.generation != before.session.generation || after.session.tick != before.session.tick ||
                after.source_speed_index != before.source_speed_index ||
                after.display_settings.minimap != before.display_settings.minimap ||
                after.display_settings.top_bar != before.display_settings.top_bar ||
                after.display_settings.panels_opaque != before.display_settings.panels_opaque ||
                after.display_settings.flat_shading == before.display_settings.flat_shading ||
                after.display_settings.viewport_size != before.display_settings.viewport_size ||
                after.source_player_names_visible != before.source_player_names_visible)
                throw new InvalidDataException("flat shading toggle changed the wrong source state");
            return after;
        }

        internal Identity TogglePlayerNames()
        {
            RequireSessionMode(allowPaused: true, "toggle player names");
            var before = Current;
            Check(NativeSim.mc_game_runtime_toggle_player_names_v1(RequireHandle(),
                ParseCounter(before.generation, "generation"),
                ParseCounter(before.session.generation, "session_generation"),
                ParseCounter(before.session.tick, "tick")), "toggle player names");
            var after = IdentitySnapshot();
            if (after.generation != before.generation || after.mode != before.mode ||
                !PreservesSessionBinding(before.session, after.session) ||
                after.session.generation != before.session.generation || after.session.tick != before.session.tick ||
                after.source_speed_index != before.source_speed_index ||
                after.source_player_names_visible == before.source_player_names_visible ||
                after.display_settings.minimap != before.display_settings.minimap ||
                after.display_settings.top_bar != before.display_settings.top_bar ||
                after.display_settings.panels_opaque != before.display_settings.panels_opaque ||
                after.display_settings.flat_shading != before.display_settings.flat_shading ||
                after.display_settings.viewport_size != before.display_settings.viewport_size)
                throw new InvalidDataException("player names toggle changed the wrong runtime state");
            return after;
        }

        internal bool SourceViewport(SourceViewportAction action, out Identity identity)
        {
            RequireSessionMode(allowPaused: true, "change source viewport");
            var before = Current;
            var size = before.display_settings.viewport_size;
            var expected = action switch {
                SourceViewportAction.MenuDecrease => Math.Max(16, size - 1),
                SourceViewportAction.MenuIncrease => Math.Min(40, size + 1),
                SourceViewportAction.KeyboardEnlarge => Math.Min(40, size + 1),
                SourceViewportAction.KeyboardShrink => size > 17 ? size - 1 : size,
                _ => throw new ArgumentOutOfRangeException(nameof(action))
            };
            var status = NativeSim.mc_game_runtime_source_viewport_v1(
                RequireHandle(), ParseCounter(before.generation, "generation"),
                ParseCounter(before.session.generation, "session_generation"),
                ParseCounter(before.session.tick, "tick"), (byte)action);
            if (status == SourceActionBlocked) {
                identity = null;
                return false;
            }
            Check(status, "change source viewport");
            var after = IdentitySnapshot();
            if (after.generation != before.generation || after.mode != before.mode ||
                !PreservesSessionBinding(before.session, after.session) ||
                after.session.generation != before.session.generation || after.session.tick != before.session.tick ||
                after.source_speed_index != before.source_speed_index ||
                after.display_settings.minimap != before.display_settings.minimap ||
                after.display_settings.top_bar != before.display_settings.top_bar ||
                after.display_settings.panels_opaque != before.display_settings.panels_opaque ||
                after.display_settings.flat_shading != before.display_settings.flat_shading ||
                after.display_settings.viewport_size != expected ||
                after.source_player_names_visible != before.source_player_names_visible)
                throw new InvalidDataException("source viewport changed the wrong runtime state");
            identity = after;
            return true;
        }

        internal bool SourceSpeed(bool secondary, out Identity identity)
        {
            RequireSessionMode(allowPaused: true, "change source speed");
            var before = Current;
            var status = NativeSim.mc_game_runtime_source_speed_v1(
                RequireHandle(), ParseCounter(before.generation, "generation"),
                ParseCounter(before.session.generation, "session_generation"),
                ParseCounter(before.session.tick, "tick"), secondary ? (byte)1 : (byte)0);
            if (status == SourceActionBlocked) {
                identity = null;
                return false;
            }
            Check(status, "change source speed");
            var after = IdentitySnapshot();
            var expectedSpeed = before.source_speed_index == 0 ? (secondary ? 2 : 1) : 0;
            if (after.generation != before.generation || after.mode != before.mode ||
                !PreservesSessionBinding(before.session, after.session) ||
                after.session.generation != before.session.generation || after.session.tick != before.session.tick ||
                after.display_settings.minimap != before.display_settings.minimap ||
                after.display_settings.top_bar != before.display_settings.top_bar ||
                after.display_settings.panels_opaque != before.display_settings.panels_opaque ||
                after.display_settings.flat_shading != before.display_settings.flat_shading ||
                after.display_settings.viewport_size != before.display_settings.viewport_size ||
                after.source_speed_index != expectedSpeed ||
                after.source_player_names_visible != before.source_player_names_visible)
                throw new InvalidDataException("source speed changed the wrong runtime state");
            identity = after;
            return true;
        }

        private static ulong SourceSpeedStepTick(Identity before, Identity after, bool recoverPlayer)
        {
            // A successful source revival resets speed before entity passes.
            if (after.source_speed_index != before.source_speed_index &&
                !(recoverPlayer && before.source_speed_index != 0 && after.source_speed_index == 0))
                throw new InvalidDataException("campaign step changed source speed without a revival");
            var passes = after.source_speed_index switch {
                0 => 1UL,
                1 => 4UL,
                2 => 8UL,
                _ => throw new InvalidDataException("source speed is outside its range")
            };
            var expectedTick = checked(ParseCounter(before.session.tick, "tick") + passes);
            if (ParseCounter(after.session.tick, "tick") != expectedTick)
                throw new InvalidDataException("campaign step did not advance its source speed passes");
            return expectedTick;
        }

        internal bool SourceFlightAssistance(out Identity identity)
        {
            RequireSessionMode(allowPaused: true, "change source flight assistance");
            var before = Current;
            var request = new SourceFlightAssistanceCommand {
                session_guard = new SourceBrightnessGuard {
                    session_id = before.session.session_id, generation = before.session.generation,
                    expected_tick = before.session.tick
                }
            };
            var bytes = Encoding.UTF8.GetBytes(JsonUtility.ToJson(request));
            var status = mc_game_runtime_preferences_command_json_v1(RequireHandle(),
                ParseCounter(before.generation, "generation"), bytes, checked((uint)bytes.Length));
            if (status == SourceActionBlocked) { identity = null; return false; }
            Check(status, "change source flight assistance");
            var after = IdentitySnapshot();
            var expected = JsonUtility.FromJson<PlayerPreferences>(
                JsonUtility.ToJson(before.player_preferences.preferences));
            expected.flight_assistance = !expected.flight_assistance;
            if (after.generation != before.generation || after.mode != before.mode ||
                !PreservesSessionBinding(before.session, after.session) ||
                after.session.generation != before.session.generation || after.session.tick != before.session.tick ||
                after.source_speed_index != before.source_speed_index ||
                after.source_player_names_visible != before.source_player_names_visible ||
                after.display_settings.minimap != before.display_settings.minimap ||
                after.display_settings.top_bar != before.display_settings.top_bar ||
                after.display_settings.panels_opaque != before.display_settings.panels_opaque ||
                after.display_settings.flat_shading != before.display_settings.flat_shading ||
                after.display_settings.viewport_size != before.display_settings.viewport_size ||
                JsonUtility.ToJson(after.player_preferences.preferences) != JsonUtility.ToJson(expected))
                throw new InvalidDataException("source flight assistance changed the wrong runtime state");
            identity = after;
            return true;
        }

        internal bool SourceAudio(SourceAudioAction action, bool speechAvailable, out Identity identity)
        {
            RequireSessionMode(allowPaused: true, "change source audio");
            var before = Current;
            var guard = new SourceBrightnessGuard {
                session_id = before.session.session_id, generation = before.session.generation,
                expected_tick = before.session.tick
            };
            object request = action switch {
                SourceAudioAction.Sound => new SourceAudioCommand { action = "sound", session_guard = guard },
                SourceAudioAction.Music => new SourceAudioCommand { action = "music", session_guard = guard },
                SourceAudioAction.Speech => new SourceSpeechCommand { session_guard = guard, speech_available = speechAvailable },
                _ => throw new ArgumentOutOfRangeException(nameof(action))
            };
            var bytes = Encoding.UTF8.GetBytes(JsonUtility.ToJson(request));
            var status = mc_game_runtime_preferences_command_json_v1(RequireHandle(),
                ParseCounter(before.generation, "generation"), bytes, checked((uint)bytes.Length));
            if (status == SourceActionBlocked) { identity = null; return false; }
            Check(status, "change source audio");
            var after = IdentitySnapshot();
            var oldPrefs = before.player_preferences.preferences;
            var newPrefs = after.player_preferences.preferences;
            if (after.generation != before.generation || after.mode != before.mode ||
                !PreservesSessionBinding(before.session, after.session) ||
                after.session.generation != before.session.generation || after.session.tick != before.session.tick ||
                after.source_speed_index != before.source_speed_index ||
                after.source_player_names_visible != before.source_player_names_visible ||
                after.display_settings.minimap != before.display_settings.minimap ||
                after.display_settings.top_bar != before.display_settings.top_bar ||
                after.display_settings.panels_opaque != before.display_settings.panels_opaque ||
                after.display_settings.flat_shading != before.display_settings.flat_shading ||
                after.display_settings.viewport_size != before.display_settings.viewport_size ||
                (before.source_audio_capabilities != null &&
                 (after.source_audio_capabilities == null ||
                  after.source_audio_capabilities.sound_available != before.source_audio_capabilities.sound_available ||
                  after.source_audio_capabilities.music_available != before.source_audio_capabilities.music_available)) ||
                newPrefs.sound_enabled != (action == SourceAudioAction.Sound ? !oldPrefs.sound_enabled : oldPrefs.sound_enabled) ||
                newPrefs.music_enabled != (action == SourceAudioAction.Music ? !oldPrefs.music_enabled : oldPrefs.music_enabled) ||
                newPrefs.speech_enabled != (action == SourceAudioAction.Speech ? !oldPrefs.speech_enabled : oldPrefs.speech_enabled))
                throw new InvalidDataException("source audio changed the wrong runtime state");
            identity = after;
            return true;
        }

        internal bool SourceBrightness(SourceBrightnessAction action, out Identity identity)
        {
            RequireSessionMode(allowPaused: true, "change source brightness");
            var before = Current;
            var actionName = action switch {
                SourceBrightnessAction.Cycle => "cycle",
                SourceBrightnessAction.Increment => "increment",
                SourceBrightnessAction.Decrement => "decrement",
                _ => throw new ArgumentOutOfRangeException(nameof(action))
            };
            var command = new SourceBrightnessCommand {
                action = actionName,
                session_guard = new SourceBrightnessGuard {
                    session_id = before.session.session_id,
                    generation = before.session.generation,
                    expected_tick = before.session.tick
                }
            };
            var bytes = Encoding.UTF8.GetBytes(JsonUtility.ToJson(command));
            var status = mc_game_runtime_preferences_command_json_v1(
                RequireHandle(), ParseCounter(before.generation, "generation"),
                bytes, checked((uint)bytes.Length));
            if (status == SourceActionBlocked) {
                identity = null;
                return false;
            }
            Check(status, "change source brightness");
            var after = IdentitySnapshot();
            if (after.generation != before.generation || after.mode != before.mode ||
                !PreservesSessionBinding(before.session, after.session) ||
                after.session.generation != before.session.generation || after.session.tick != before.session.tick ||
                after.source_speed_index != before.source_speed_index ||
                after.display_settings.minimap != before.display_settings.minimap ||
                after.display_settings.top_bar != before.display_settings.top_bar ||
                after.display_settings.panels_opaque != before.display_settings.panels_opaque ||
                after.display_settings.flat_shading != before.display_settings.flat_shading ||
                after.display_settings.viewport_size != before.display_settings.viewport_size ||
                after.player_preferences?.preferences?.brightness == null ||
                after.source_player_names_visible != before.source_player_names_visible)
                throw new InvalidDataException("source brightness changed the wrong runtime state");
            identity = after;
            return true;
        }

        internal EngineSessionTransport.CampaignSnapshotObservation CampaignStep(
            sbyte roll,
            sbyte pitch,
            sbyte flags,
            ushort lookYaw,
            ushort lookPitch,
            string selectSpell = null,
            bool recenter = false,
            bool fullStop = false,
            bool recoverPlayer = false,
            uint elapsedCentiseconds = 0,
            EngineSessionTransport.SpellAssignment assignment = null,
            bool showObjective = false,
            bool removeCastleStage = false,
            EngineSessionTransport.SpellMark mark = null,
            EngineSessionTransport.MarkedSpellCycle cycle = null,
            string abandonAction = null, Vector2? steeringObservation = null)
        {
            CampaignAdvance(roll, pitch, flags, lookYaw, lookPitch, selectSpell,
                recenter, fullStop, recoverPlayer, elapsedCentiseconds, assignment,
                showObjective, removeCastleStage, mark, cycle, abandonAction, steeringObservation);
            return CampaignSnapshot();
        }

        internal EngineSessionTransport.CampaignRenderObservation CampaignAdvance(
            sbyte roll, sbyte pitch, sbyte flags, ushort lookYaw, ushort lookPitch,
            string selectSpell = null, bool recenter = false, bool fullStop = false,
            bool recoverPlayer = false, uint elapsedCentiseconds = 0,
            EngineSessionTransport.SpellAssignment assignment = null,
            bool showObjective = false, bool removeCastleStage = false,
            EngineSessionTransport.SpellMark mark = null,
            EngineSessionTransport.MarkedSpellCycle cycle = null,
            string abandonAction = null, Vector2? steeringObservation = null)
        {
            RequireSessionMode(allowPaused: false, "campaign step");
            var before = Current;
            var beforeRuntimeGeneration = ParseCounter(before.generation, "generation");
            var beforeSessionGeneration =
                ParseCounter(before.session.generation, "session_generation");
            var beforeTick = ParseCounter(before.session.tick, "tick");
            EngineSessionTransport.StepHandle(
                RequireHandle(),
                before.session,
                roll,
                pitch,
                flags,
                lookYaw,
                lookPitch,
                NegotiatedSessionContract.limits.maximum_command_json_bytes,
                selectSpell, recenter, fullStop, recoverPlayer, elapsedCentiseconds, assignment, showObjective, removeCastleStage, mark, cycle, abandonAction, steeringObservation);
            if (!JsonRenderObservations)
            {
                // Revival may clear speed, and enabled flight assistance can
                // advance its recenter sequence without a runtime generation change.
                Identity updated = null;
                var steppedTick = checked(beforeTick + 1);
                if (before.source_speed_index != 0 || before.player_preferences.preferences.flight_assistance)
                {
                    updated = IdentitySnapshot();
                    if (before.source_speed_index != 0)
                        steppedTick = SourceSpeedStepTick(before, updated, recoverPlayer);
                }
                var publication = ReadRenderPublication(before, steppedTick);
                if (updated != null)
                {
                    var observed = publication.Observation.identity;
                    if (updated.mode != PlayingMode || updated.generation != before.generation ||
                        !PreservesSessionBinding(observed, updated.session) ||
                        updated.session.generation != observed.generation || updated.session.tick != observed.tick)
                        throw new InvalidDataException("source step identity does not match the stepped session");
                }
                else
                    PublishCampaignIdentity(before, publication.Observation.identity);
                return publication.Observation;
            }
            var identity = IdentitySnapshot();
            if (identity.mode != PlayingMode ||
                ParseCounter(identity.generation, "generation") != beforeRuntimeGeneration ||
                !PreservesSessionBinding(before.session, identity.session) ||
                !SameRuntimeFlow(before.active_runtime_flow, identity.active_runtime_flow) ||
                ParseCounter(identity.session.generation, "session_generation") !=
                    beforeSessionGeneration)
            {
                throw new InvalidOperationException(
                    "campaign step did not advance only the bound gameplay tick");
            }
            SourceSpeedStepTick(before, identity, recoverPlayer);
            return JsonRenderObservation();
        }

        internal EngineSessionTransport.EventObservation CampaignEvents(
            string cursor,
            uint limit = 1024)
        {
            RequireSessionMode(allowPaused: true, "campaign events");
            var result = EngineSessionTransport.EventsForHandle(
                RequireHandle(),
                Current.session,
                cursor,
                limit,
                NegotiatedSessionContract.limits.maximum_event_page_size,
                EngineSessionTransport.CampaignProfile);
            if (result.identity.session_id != Current.session.session_id ||
                result.identity.generation != Current.session.generation ||
                result.identity.tick != Current.session.tick)
            {
                throw new InvalidOperationException(
                    "runtime campaign events are not identity-bound");
            }
            return result;
        }

        internal EngineSessionTransport.PresentationAssetObservation PresentationAsset(
            string semanticId)
        {
            RequireSessionMode(allowPaused: true, "presentation asset");
            return EngineSessionTransport.PresentationAssetForHandle(
                RequireHandle(),
                Current.session,
                semanticId,
                NegotiatedSessionContract.limits.maximum_asset_id_bytes);
        }

        internal EngineSessionTransport.CampaignSnapshotObservation CampaignReset()
        {
            RequireSessionMode(allowPaused: false, "campaign reset");
            var before = Current;
            var beforeRuntimeGeneration = ParseCounter(before.generation, "generation");
            var beforeSessionGeneration =
                ParseCounter(before.session.generation, "session_generation");
            EngineSessionTransport.ResetHandle(RequireHandle(), before.session);
            var identity = IdentitySnapshot();
            if (identity.mode != PlayingMode ||
                ParseCounter(identity.generation, "generation") != beforeRuntimeGeneration ||
                !PreservesSessionBinding(before.session, identity.session) ||
                !SameRuntimeFlow(before.active_runtime_flow, identity.active_runtime_flow) ||
                ParseCounter(identity.session.generation, "session_generation") !=
                    checked(beforeSessionGeneration + 1) ||
                ParseCounter(identity.session.tick, "tick") != 0)
            {
                throw new InvalidOperationException(
                    "campaign reset did not replace only the bound gameplay generation");
            }
            return CampaignSnapshot();
        }

        public void Dispose()
        {
            var raw = Interlocked.Exchange(ref handleBits, 0);
            if (raw == 0)
            {
                return;
            }
            var status = NativeSim.mc_game_runtime_destroy_v1(unchecked((ulong)raw));
            if (status != NativeSim.Ok)
            {
                Interlocked.CompareExchange(ref handleBits, raw, 0);
                Check(status, "destroy game runtime");
            }
            Current = null;
            GC.SuppressFinalize(this);
        }

        internal static void ValidateContract(Contract contract)
        {
            if (contract == null ||
                contract.runtime_contract != RuntimeContract ||
                contract.transport_contract != TransportContract ||
                contract.transport_version != 12 ||
                contract.definition_contract != DefinitionContract ||
                contract.semantic_contract_version != SemanticContractVersion ||
                contract.encoding != "utf-8-json" ||
                contract.u64_counter_encoding != "decimal-string" ||
                contract.playing_session_transport !=
                    "mc_game_session_*_v1 on the same handle" ||
                contract.package_profile != EngineSessionTransport.CampaignProfile ||
                contract.installed_package_set == null ||
                contract.installed_package_set.request_contract !=
                    InstalledPackageSetContract ||
                contract.installed_package_set.transport !=
                    "mc_game_runtime_bootstrap_package_set_json_v1" ||
                contract.installed_package_set.selection_authority !=
                    "rust_owned_semantic_flow" ||
                contract.installed_package_set.input_order !=
                    "unordered_resource_locations" ||
                contract.installed_package_set.lifetime !=
                    "immutable_until_destroy" ||
                contract.replay_save == null ||
                contract.replay_save.save_contract != ReplaySaveContract ||
                contract.replay_save.save_transport !=
                    "mc_game_runtime_save_replay_json_v1" ||
                contract.replay_save.load_transport !=
                    "mc_game_runtime_load_replay_json_v1" ||
                contract.replay_save.warm_load_transport !=
                    "mc_game_runtime_load_replay_from_runtime_json_v1" ||
                contract.replay_save.load_package_authority !=
                    "complete_unordered_resource_set" ||
                contract.replay_save.client_semantics != "opaque_utf8_json" ||
                contract.paused_frame == null ||
                contract.paused_frame.transport != "mc_game_runtime_paused_frame_v1" ||
                contract.paused_frame.gameplay_tick != "frozen" ||
                contract.paused_frame.session_identity != "unchanged" ||
                contract.paused_frame.deterministic_work != "package_declared_pre_gate" ||
                contract.paused_frame.replay != "ordered_operation" ||
                contract.presentation_frame == null ||
                contract.presentation_frame.transport != "mc_game_runtime_presentation_frame_v1" ||
                contract.presentation_frame.observation_transport != "mc_game_runtime_participant_notifications_json_v1" ||
                contract.presentation_frame.gameplay_tick != "frozen" ||
                contract.presentation_frame.session_identity != "unchanged" ||
                contract.presentation_frame.deterministic_work != "package_presentation_only" ||
                contract.presentation_frame.replay != "ordered_operation" ||
                contract.runtime_flow == null ||
                contract.runtime_flow.title != "absent" ||
                contract.runtime_flow.loading != "pending_definition.runtime_flow" ||
                contract.runtime_flow.playing_and_paused != "active_runtime_flow" ||
                contract.runtime_flow.presentation_cue != "identity.presentation" ||
                contract.completion_transition == null ||
                contract.completion_transition.receipt_contract !=
                    CompletionTransitionReceiptContract ||
                contract.completion_transition.accept_transport !=
                    "mc_game_runtime_accept_transition_v1" ||
                contract.completion_transition.receipt_transport !=
                    "mc_game_runtime_completion_transition_receipt_json_v1" ||
                contract.completion_transition.destination_authority !=
                    "rust_owned_semantic_flow" ||
                contract.completion_transition.package_authority !=
                    "installed_package_resolver" ||
                contract.completion_transition.presentation_sequence !=
                    "ordered_rust_owned_semantic_cues" ||
                contract.completion_transition.guards == null ||
                !new[] { "runtime_generation", "session_generation", "expected_tick" }
                    .SequenceEqual(contract.completion_transition.guards) ||
                contract.limits == null ||
                contract.limits.maximum_output_json_bytes != 16 * 1024 * 1024 ||
                contract.limits.maximum_installed_package_set_json_bytes != 64 * 1024 ||
                contract.limits.maximum_installed_package_count < 16 ||
                contract.limits.maximum_replay_save_json_bytes != MaximumReplaySaveJsonBytes ||
                contract.limits.json_length_type != "u32" ||
                contract.modes == null ||
                !new[] { TitleMode, CampaignMode, LoadingMode, PlayingMode, PausedMode }
                    .SequenceEqual(contract.modes) ||
                contract.transitions == null ||
                !new[] {
                    "bootstrap_package_set", "restart_new_game", "show_new_game_map", "open_level", "cancel_loading",
                    "load_replay_save", "pause", "paused_frame", "resume",
                    "accept_completion_transition", "finish_mission_if_ready", "campaign_menu_command", "close"
                }
                    .SequenceEqual(contract.transitions))
            {
                throw new InvalidOperationException("native game runtime contract is unsupported");
            }
        }

        private static Contract ReadContract()
        {
            return ReadJson<Contract>(
                NativeSim.mc_game_runtime_contract_json_v1,
                "read game runtime contract");
        }

        private static void ValidateCompletionStatistics(CompletionStatistics statistics)
        {
            if (statistics == null || statistics.spells_found_percent < 0 || statistics.spells_found_percent > 100 ||
                statistics.hit_accuracy_percent < 0 || statistics.hit_accuracy_percent > 100 ||
                statistics.creatures_killed_percent < 0 || statistics.creatures_killed_percent > 100 ||
                statistics.mana_collected_percent < 0 || statistics.mana_collected_percent > 100)
                throw new InvalidOperationException("Campaign results are outside source percentage bounds");
        }

        internal static void ValidateIdentity(Identity identity, ulong handle)
        {
            if (identity == null || identity.display_settings == null || identity.runtime_contract != RuntimeContract ||
                identity.display_settings.viewport_size < 16 || identity.display_settings.viewport_size > 40 ||
                identity.source_speed_index > 2 ||
                ParseCounter(identity.runtime_id, "runtime_id") != handle ||
                !new[] { TitleMode, CampaignMode, LoadingMode, PlayingMode, PausedMode }
                    .Contains(identity.mode))
            {
                throw new InvalidOperationException("game runtime identity is incomplete");
            }
            ParseCounter(identity.generation, "generation");
            ValidateCampaignReturnMedia(identity);
            if (identity.mode != CampaignMode && HasCampaignMenuData(identity.campaign_menu))
                throw new InvalidOperationException("non-campaign identity carries a campaign menu");
            ValidateIdentityPresentation(identity);
            switch (identity.mode)
            {
                case CampaignMode:
                    if (HasDefinitionIdentityData(identity.pending_definition) || HasSessionIdentityData(identity.session) ||
                        !HasRuntimeFlowData(identity.active_runtime_flow) || identity.campaign_menu == null ||
                        !new[] { "results", "mission_select" }.Contains(identity.campaign_menu.phase) ||
                        identity.campaign_menu.completed_levels == null || identity.campaign_menu.available_levels == null ||
                        (!string.IsNullOrEmpty(identity.campaign_menu.completed_level_id) &&
                         !identity.campaign_menu.completed_levels.Contains(identity.campaign_menu.completed_level_id)) ||
                        (!string.IsNullOrEmpty(identity.campaign_menu.selected_level_id) &&
                         !identity.campaign_menu.available_levels.Contains(identity.campaign_menu.selected_level_id)))
                        throw new InvalidOperationException("Campaign runtime identity is incomplete");
                    ValidateRuntimeFlow(identity.active_runtime_flow);
                    var campaignLevels = identity.active_runtime_flow.entries.Select(entry => entry.level_id).ToArray();
                    var selectableLevels = new HashSet<string>(campaignLevels, StringComparer.Ordinal);
                    foreach (var secret in identity.secret_campaign?.discovered_levels ?? Array.Empty<string>())
                        selectableLevels.Add(secret);
                    var menu = identity.campaign_menu;
                    if (menu.briefing_speech != null &&
                        (menu.phase != "mission_select" || string.IsNullOrEmpty(menu.briefing_level_id) ||
                         menu.briefing_speech.track < 1 || menu.briefing_speech.track > 27 ||
                         menu.briefing_speech.segment != 0 || menu.briefing_speech.start_ms != 0 ||
                         menu.briefing_speech.length_ms == 0))
                        throw new InvalidOperationException("campaign briefing speech is not a source-bound map segment");
                    var firstMap = string.IsNullOrEmpty(menu.completed_level_id);
                    var pristineMap = firstMap && menu.completed_levels.Length == 0;
                    var progressedMap = firstMap && !pristineMap;
                    if (progressedMap)
                    {
                        // A restored secret return can retain an earned frontier
                        // without an ordinary Results owner. Match the source
                        // map projection rather than treating it as New Game.
                        var unfinished = campaignLevels.FirstOrDefault(level => !menu.completed_levels.Contains(level));
                        var frontier = unfinished == null ? campaignLevels.Length : Array.IndexOf(campaignLevels, unfinished) + 1;
                        var expectedAvailable = campaignLevels.Take(frontier).ToList();
                        var discovered = identity.secret_campaign?.discovered_levels ?? Array.Empty<string>();
                        foreach (var secret in identity.active_runtime_flow.secret_entries ?? Array.Empty<RuntimeFlowSecretEntry>())
                            if (expectedAvailable.Contains(secret.origin_level_id) && discovered.Contains(secret.secret_level_id))
                                expectedAvailable.Add(secret.secret_level_id);
                        var selectedSecret = (identity.active_runtime_flow.secret_entries ?? Array.Empty<RuntimeFlowSecretEntry>())
                            .Any(secret => secret.secret_level_id == menu.selected_level_id && expectedAvailable.Contains(secret.secret_level_id));
                        var expectedBriefing = selectedSecret ? menu.selected_level_id : unfinished;
                        if (menu.phase != "mission_select" || !menu.available_levels.SequenceEqual(expectedAvailable) ||
                            menu.briefing_level_id != expectedBriefing)
                            throw new InvalidOperationException("Campaign map does not match its retained frontier");
                    }
                    if ((pristineMap && (menu.phase != "mission_select" ||
                         identity.active_runtime_flow.bound_level_id != identity.active_runtime_flow.entry_level_id ||
                         menu.available_levels.Length != 1 ||
                         menu.available_levels[0] != identity.active_runtime_flow.entry_level_id ||
                         menu.selected_level_id != identity.active_runtime_flow.entry_level_id ||
                         menu.briefing_level_id != identity.active_runtime_flow.entry_level_id)) ||
                        (!firstMap && menu.completed_level_id != identity.active_runtime_flow.bound_level_id) ||
                        menu.completed_levels.Distinct().Count() != menu.completed_levels.Length ||
                        menu.available_levels.Distinct().Count() != menu.available_levels.Length ||
                        menu.completed_levels.Any(level => !campaignLevels.Contains(level)) ||
                        menu.available_levels.Any(level => !selectableLevels.Contains(level)) ||
                        (menu.phase == "results" && !string.IsNullOrEmpty(menu.selected_level_id)) ||
                        (!string.IsNullOrEmpty(menu.briefing_level_id) && !menu.available_levels.Contains(menu.briefing_level_id)))
                        throw new InvalidOperationException("Campaign menu is not bound to its retained flow and phase");
                    if (menu.level_statistics != null)
                    {
                        var seenStatistics = new HashSet<string>(StringComparer.Ordinal);
                        foreach (var row in menu.level_statistics)
                        {
                            if (row == null || !menu.completed_levels.Contains(row.level_id) ||
                                !seenStatistics.Add(row.level_id))
                                throw new InvalidOperationException("Campaign statistics have an unbound or duplicate mission");
                            ValidateCompletionStatistics(row.statistics);
                        }
                    }
                    if (menu.completion_statistics != null)
                    {
                        ValidateCompletionStatistics(menu.completion_statistics);
                        var completedStatistics = menu.level_statistics?.SingleOrDefault(row => row.level_id == menu.completed_level_id);
                        if (completedStatistics == null || JsonUtility.ToJson(completedStatistics.statistics) != JsonUtility.ToJson(menu.completion_statistics))
                            throw new InvalidOperationException("Current results differ from retained campaign statistics");
                    }
                    break;
                case TitleMode:
                    if (HasDefinitionIdentityData(identity.pending_definition) ||
                        HasSessionIdentityData(identity.session) ||
                        HasRuntimeFlowData(identity.active_runtime_flow))
                    {
                        throw new InvalidOperationException(
                            "Title runtime identity carries an unexpected binding");
                    }
                    break;
                case LoadingMode:
                    if (!HasDefinitionIdentity(identity) ||
                        HasSessionIdentityData(identity.session) ||
                        HasRuntimeFlowData(identity.active_runtime_flow) ||
                        string.IsNullOrWhiteSpace(
                            identity.pending_definition.package_graph_hash))
                    {
                        throw new InvalidOperationException(
                            "Loading runtime identity is incomplete");
                    }
                    ParseCounter(
                        identity.pending_definition.definition_revision,
                        "definition_revision");
                    ValidateRuntimeFlow(identity.pending_definition.runtime_flow);
                    break;
                case PlayingMode:
                case PausedMode:
                    if (HasDefinitionIdentityData(identity.pending_definition) ||
                        !HasSessionIdentity(identity) ||
                        !HasRuntimeFlowData(identity.active_runtime_flow))
                    {
                        throw new InvalidOperationException(
                            "Playing runtime identity is incomplete");
                    }
                    EngineSessionTransport.ValidateIdentity(identity.session);
                    ValidateRuntimeFlow(identity.active_runtime_flow);
                    break;
            }
            ValidateSecretCampaign(identity);
        }

        internal static bool CampaignReturnMediaBlocksInput(Identity identity) =>
            identity?.frontend_media?.campaign_return?.blocking == true;

        internal static void ValidateCampaignReturnMedia(Identity identity)
        {
            var pending = identity?.frontend_media?.campaign_return;
            if (pending == null) return;
            if (identity.mode != CampaignMode || !pending.blocking ||
                !new[] { "cutscene", "credits", "input_release" }.Contains(pending.phase) ||
                (pending.cutscene_id.HasValue && (pending.cutscene_id < 1 || pending.cutscene_id > 6)) ||
                (pending.phase == "cutscene" && !pending.cutscene_id.HasValue) ||
                (pending.phase == "credits") != (pending.credits != null))
                throw new InvalidOperationException("campaign return media has an invalid source phase");
            var credits = pending.credits;
            if (credits != null && (credits.cursor < 0 || credits.cursor > 117 ||
                credits.lines == null || credits.lines.Length > 8 ||
                credits.lines.Any(line => line == null || line.Length > 33 ||
                    line.Any(character => character > 127) || line == "!")))
                throw new InvalidOperationException("campaign credits exceed the source row extent");
        }

        private static void ValidateSecretCampaign(Identity identity)
        {
            var snapshot = identity?.secret_campaign;
            var flow = identity.active_runtime_flow ?? identity.pending_definition?.runtime_flow;
            var secretEntries = flow?.secret_entries ?? Array.Empty<RuntimeFlowSecretEntry>();
            var boundSecret = secretEntries.FirstOrDefault(entry =>
                entry != null && entry.secret_level_id == flow?.bound_level_id);
            if (snapshot == null)
            {
                if (boundSecret != null)
                    throw new InvalidOperationException(
                        "a secret-bound runtime is missing its discovered return frame");
                return;
            }

            if (snapshot.discovered_levels == null ||
                snapshot.discovered_levels.Length == 0 || snapshot.completed_levels == null ||
                snapshot.discovered_levels.Any(level => !IsSemanticId(level, "level")) ||
                snapshot.discovered_levels.Distinct(StringComparer.Ordinal).Count() !=
                    snapshot.discovered_levels.Length ||
                snapshot.completed_levels.Any(level => !snapshot.discovered_levels.Contains(level)) ||
                snapshot.completed_levels.Distinct(StringComparer.Ordinal).Count() !=
                    snapshot.completed_levels.Length ||
                (flow != null && snapshot.discovered_levels.Any(level =>
                    !secretEntries.Any(entry => entry != null && entry.secret_level_id == level))))
            {
                throw new InvalidOperationException(
                    "secret campaign progress is not bound to its runtime-flow catalog");
            }

            if (snapshot.level_statistics != null)
            {
                var seen = new HashSet<string>(StringComparer.Ordinal);
                foreach (var row in snapshot.level_statistics)
                {
                    if (row == null || !snapshot.completed_levels.Contains(row.level_id) ||
                        !seen.Add(row.level_id) ||
                        (flow != null && !secretEntries.Any(entry => entry != null && entry.secret_level_id == row.level_id)))
                        throw new InvalidOperationException("Secret statistics have an unbound or duplicate mission");
                    ValidateCompletionStatistics(row.statistics);
                }
            }

            if (boundSecret != null &&
                (!snapshot.discovered_levels.Contains(boundSecret.secret_level_id) ||
                 identity.mode is not (LoadingMode or PlayingMode or PausedMode) ||
                 ((identity.mode is PlayingMode or PausedMode) &&
                  snapshot.return_origin != boundSecret.origin_level_id)))
            {
                throw new InvalidOperationException(
                    "a secret-bound runtime has no exact discovered parent return frame");
            }

            if (snapshot.return_origin != null &&
                (identity.mode is not (PlayingMode or PausedMode or LoadingMode) || flow == null ||
                 !IsSemanticId(snapshot.return_origin, "level") ||
                 !secretEntries.Any(entry => entry != null &&
                     entry.origin_level_id == snapshot.return_origin &&
                     entry.secret_level_id == flow.bound_level_id &&
                     snapshot.discovered_levels.Contains(entry.secret_level_id))))
            {
                throw new InvalidOperationException(
                    "secret campaign return origin is not bound to the active secret level");
            }
        }

        internal static void ValidateIdentityPresentation(Identity identity)
        {
            ValidatePresentationCue(identity?.presentation);
            var expectedPhase = identity?.mode switch
            {
                CampaignMode => identity.campaign_menu?.phase == "results" ? LevelCompletionPresentationPhase : CampaignMapPresentationPhase,
                TitleMode => TitlePresentationPhase,
                LoadingMode => LoadingPresentationPhase,
                PlayingMode => LevelEntryPresentationPhase,
                PausedMode => LevelEntryPresentationPhase,
                _ => null
            };
            var expectedLevel = identity?.mode switch
            {
                CampaignMode => identity.campaign_menu?.phase == "results" ? identity.campaign_menu.completed_level_id : identity.campaign_menu?.selected_level_id,
                LoadingMode => identity.pending_definition?.runtime_flow?.bound_level_id,
                PlayingMode => identity.active_runtime_flow?.bound_level_id,
                PausedMode => identity.active_runtime_flow?.bound_level_id,
                _ => null
            };
            if (identity?.presentation?.phase != expectedPhase ||
                (expectedPhase == TitlePresentationPhase &&
                 identity.presentation.cue_id != TitlePresentationCue) ||
                (expectedPhase != TitlePresentationPhase &&
                 identity.presentation.level_id != expectedLevel))
            {
                throw new InvalidOperationException(
                    "runtime presentation cue is not identity-bound");
            }
        }

        private Identity SetPaused(
            string expectedMode,
            string nextMode,
            Func<ulong, ulong, int> transition,
            string operation)
        {
            if (Current?.mode != expectedMode || !HasSessionIdentity(Current))
            {
                throw new InvalidOperationException(
                    operation + " requires " + expectedMode + " mode");
            }
            var before = Current;
            var beforeRuntimeGeneration = ParseCounter(before.generation, "generation");
            var beforeSessionGeneration =
                ParseCounter(before.session.generation, "session_generation");
            var beforeTick = ParseCounter(before.session.tick, "tick");
            Check(
                transition(RequireHandle(), beforeRuntimeGeneration),
                operation);
            var identity = IdentitySnapshot();
            if (identity.mode != nextMode ||
                ParseCounter(identity.generation, "generation") !=
                    checked(beforeRuntimeGeneration + 1) ||
                identity.session.session_id != before.session.session_id ||
                identity.session.package_graph_hash != before.session.package_graph_hash ||
                identity.session.definition_hash != before.session.definition_hash ||
                identity.session.definition_revision != before.session.definition_revision ||
                !SameRuntimeFlow(before.active_runtime_flow, identity.active_runtime_flow) ||
                !SameSecretCampaign(before.secret_campaign, identity.secret_campaign) ||
                ParseCounter(identity.session.generation, "session_generation") !=
                    checked(beforeSessionGeneration + 1) ||
                ParseCounter(identity.session.tick, "tick") != beforeTick)
            {
                throw new InvalidOperationException(
                    operation + " did not preserve the bound gameplay session");
            }
            return identity;
        }

        private static void ValidateCompletionTransitionReceipt(
            CompletionTransitionReceipt receipt,
            Identity before,
            Identity after,
            ulong beforeRuntimeGeneration)
        {
            var sourceEntry = before?.active_runtime_flow?.entries?.FirstOrDefault(
                entry => entry?.level_id == before.active_runtime_flow.bound_level_id);
            if (receipt == null ||
                receipt.receipt_contract != CompletionTransitionReceiptContract ||
                !SameIdentity(receipt.before, before) ||
                !SameIdentity(receipt.after, after) ||
                !SameSessionIdentity(receipt.source_session, before?.session) ||
                receipt.completed_level_id != before?.active_runtime_flow?.bound_level_id ||
                !SameDestination(
                    receipt.completion_destination,
                    sourceEntry?.completion_destination) ||
                receipt.completion == null ||
                !IsLowerHex(receipt.completion.state_hash, 16) ||
                !IsLowerHex(receipt.receipt_sha256, 64) ||
                ParseCounter(after.generation, "generation") !=
                    checked(beforeRuntimeGeneration + 1))
            {
                throw new InvalidOperationException(
                    "completion transition receipt is not bound to the accepted Rust transition");
            }
            ValidateCompletionPresentationSequence(receipt);
            if (HasSessionIdentityData(after.session) || HasDefinitionIdentityData(after.pending_definition) ||
                !SameRuntimeFlow(before.active_runtime_flow, after.active_runtime_flow))
                throw new InvalidOperationException("completion did not return to its campaign owner");
        }

        internal static void ValidatePresentationCue(RuntimePresentationCue cue)
        {
            if (cue == null || !IsSemanticId(cue.cue_id, "presentation") ||
                !new[] {
                    TitlePresentationPhase,
                    CampaignMapPresentationPhase,
                    LoadingPresentationPhase,
                    LevelEntryPresentationPhase,
                    LevelCompletionPresentationPhase
                }.Contains(cue.phase))
            {
                throw new InvalidOperationException(
                    "runtime presentation cue is incomplete");
            }
            if (cue.phase == TitlePresentationPhase)
            {
                if (cue.cue_id != TitlePresentationCue ||
                    !string.IsNullOrEmpty(cue.level_id) ||
                    HasDestinationData(cue.completion_destination))
                {
                    throw new InvalidOperationException(
                        "Title presentation cue carries invalid context");
                }
                return;
            }
            if (cue.phase == CampaignMapPresentationPhase)
            {
                if (cue.cue_id != "presentation.campaign.map" ||
                    (!string.IsNullOrEmpty(cue.level_id) && !IsSemanticId(cue.level_id, "level")) ||
                    HasDestinationData(cue.completion_destination))
                    throw new InvalidOperationException("campaign map cue carries invalid context");
                return;
            }
            if (!IsSemanticId(cue.level_id, "level"))
            {
                throw new InvalidOperationException(
                    "level presentation cue has no semantic level context");
            }
            if (cue.phase == LevelCompletionPresentationPhase)
            {
                ValidateDestination(cue.completion_destination);
            }
            else if (HasDestinationData(cue.completion_destination))
            {
                throw new InvalidOperationException(
                    "non-completion presentation cue carries a destination");
            }
        }

        internal static void ValidateCompletionPresentationSequence(
            CompletionTransitionReceipt receipt)
        {
            var replayMap = receipt?.disposition == ReplayMapCompletionDisposition;
            if (receipt == null ||
                receipt.receipt_contract != CompletionTransitionReceiptContract ||
                !IsSemanticId(receipt.completed_level_id, "level") ||
                (!replayMap && receipt.disposition != ResultsCompletionDisposition) ||
                receipt.after?.mode != CampaignMode ||
                receipt.after.campaign_menu?.phase != (replayMap ? "mission_select" : "results") ||
                receipt.after.campaign_menu.completed_level_id != receipt.completed_level_id ||
                (replayMap && receipt.after.frontend_media?.campaign_return != null) ||
                receipt.presentation_sequence == null ||
                receipt.presentation_sequence.Length != (replayMap ? 1 : 2))
            {
                throw new InvalidOperationException(
                    "completion transition disposition does not match its campaign presentation");
            }
            ValidateIdentityPresentation(receipt.after);
            foreach (var cue in receipt.presentation_sequence)
            {
                ValidatePresentationCue(cue);
            }
            var destination = receipt.presentation_sequence[replayMap ? 0 : 1];
            if (!SamePresentationCue(destination, receipt.after.presentation))
                throw new InvalidOperationException(
                    "completion transition destination presentation is not receipt-bound");
            if (replayMap) return;
            var completion = receipt.presentation_sequence[0];
            if (completion.phase != LevelCompletionPresentationPhase ||
                completion.level_id != receipt.completed_level_id ||
                !SameDestination(
                    completion.completion_destination,
                    receipt.completion_destination))
            {
                throw new InvalidOperationException(
                    "completion transition presentation sequence is not receipt-bound");
            }
        }

        private void RequireSessionMode(bool allowPaused, string operation)
        {
            if (!HasSessionIdentity(Current) ||
                (Current.mode != PlayingMode && (!allowPaused || Current.mode != PausedMode)))
            {
                throw new InvalidOperationException(
                    operation + " requires a bound " +
                    (allowPaused ? "Playing or Paused" : "Playing") + " session");
            }
        }

        private EngineSessionTransport.CampaignSnapshotObservation ValidateCampaignObservation(
            EngineSessionTransport.CampaignSnapshotObservation snapshot)
        {
            if (snapshot?.identity == null || snapshot.value == null ||
                snapshot.identity.session_id != Current.session.session_id ||
                snapshot.identity.generation != Current.session.generation ||
                snapshot.identity.tick != Current.session.tick)
            {
                throw new InvalidOperationException(
                    "runtime campaign snapshot is not identity-bound");
            }
            return snapshot;
        }

        private bool PreservesSessionBinding(
            EngineSessionTransport.Identity before,
            EngineSessionTransport.Identity after)
        {
            return before != null && after != null &&
                ParseCounter(after.session_id, "session_id") == RequireHandle() &&
                after.session_id == before.session_id &&
                after.package_graph_hash == before.package_graph_hash &&
                after.definition_hash == before.definition_hash &&
                after.definition_revision == before.definition_revision;
        }

        private static bool HasDefinitionIdentity(Identity identity)
        {
            return !string.IsNullOrEmpty(identity?.pending_definition?.definition_hash);
        }

        private static bool HasDefinitionIdentityData(DefinitionIdentity identity)
        {
            return identity != null &&
                (!string.IsNullOrEmpty(identity.package_graph_hash) ||
                    !string.IsNullOrEmpty(identity.definition_hash) ||
                    !string.IsNullOrEmpty(identity.definition_revision) ||
                    HasRuntimeFlowData(identity.runtime_flow));
        }

        private static bool HasSessionIdentity(Identity identity)
        {
            return !string.IsNullOrEmpty(identity?.session?.session_id);
        }

        private static bool HasSessionIdentityData(EngineSessionTransport.Identity identity)
        {
            return identity != null &&
                (!string.IsNullOrEmpty(identity.engine_contract) ||
                    !string.IsNullOrEmpty(identity.package_graph_hash) ||
                    !string.IsNullOrEmpty(identity.definition_hash) ||
                    !string.IsNullOrEmpty(identity.definition_revision) ||
                    !string.IsNullOrEmpty(identity.session_id) ||
                    !string.IsNullOrEmpty(identity.generation) ||
                    !string.IsNullOrEmpty(identity.tick));
        }

        private static bool HasRuntimeFlowData(RuntimeFlowDefinition flow)
        {
            return flow != null &&
                (!string.IsNullOrEmpty(flow.game_id) ||
                    !string.IsNullOrEmpty(flow.bound_level_id) ||
                    !string.IsNullOrEmpty(flow.entry_level_id) ||
                    flow.entries != null ||
                    flow.secret_entries != null);
        }

        private static void ValidateRuntimeFlow(RuntimeFlowDefinition flow)
        {
            if (flow == null || !IsSemanticId(flow.game_id, "game") ||
                !IsSemanticId(flow.bound_level_id, "level") ||
                !IsSemanticId(flow.entry_level_id, "level") ||
                flow.entries == null || flow.entries.Length == 0)
            {
                throw new InvalidOperationException("runtime flow identity is incomplete");
            }
            var levelIds = flow.entries.Select(entry => entry?.level_id).ToArray();
            if (levelIds.Any(levelId => !IsSemanticId(levelId, "level")) ||
                levelIds.Distinct(StringComparer.Ordinal).Count() != levelIds.Length ||
                flow.entry_level_id != levelIds[0])
            {
                throw new InvalidOperationException("runtime flow level identity is invalid");
            }
            for (var index = 0; index < levelIds.Length; index++)
            {
                var entry = flow.entries[index];
                if (entry?.completion_destination == null)
                {
                    throw new InvalidOperationException("runtime flow entry is incomplete");
                }
                if (index + 1 < levelIds.Length)
                {
                    if (entry.completion_destination.kind != "level" ||
                        entry.completion_destination.level_id != levelIds[index + 1] ||
                        !string.IsNullOrEmpty(entry.completion_destination.node_id))
                    {
                        throw new InvalidOperationException(
                            "runtime flow level destination is invalid");
                    }
                }
                else if (entry.completion_destination.kind != "title" ||
                    !IsSemanticId(entry.completion_destination.node_id, "flow") ||
                    !string.IsNullOrEmpty(entry.completion_destination.level_id))
                {
                    throw new InvalidOperationException(
                        "runtime flow Title destination is invalid");
                }
            }
            var ordinaryLevels = new HashSet<string>(levelIds, StringComparer.Ordinal);
            var catalogLevels = new HashSet<string>(ordinaryLevels, StringComparer.Ordinal);
            foreach (var secret in flow.secret_entries ?? Array.Empty<RuntimeFlowSecretEntry>())
            {
                if (secret == null ||
                    !IsSemanticId(secret.origin_level_id, "level") ||
                    !IsSemanticId(secret.secret_level_id, "level") ||
                    !ordinaryLevels.Contains(secret.origin_level_id) ||
                    !catalogLevels.Add(secret.secret_level_id))
                {
                    throw new InvalidOperationException(
                        "runtime flow secret association is invalid or duplicated");
                }
            }
            if (!catalogLevels.Contains(flow.bound_level_id))
                throw new InvalidOperationException("runtime flow bound level is outside its validated catalog");
        }

        private static bool IsSemanticId(string value, string expectedNamespace)
        {
            if (string.IsNullOrEmpty(value))
            {
                return false;
            }
            var segments = value.Split('.');
            if (segments.Length < 2 || segments[0] != expectedNamespace ||
                value.Contains("level1") || value.Contains("level2") ||
                value.Contains("source_slot"))
            {
                return false;
            }
            foreach (var segment in segments)
            {
                if (segment.Length == 0 || segment[0] < 'a' || segment[0] > 'z' ||
                    segment.Any(character =>
                        (character < 'a' || character > 'z') &&
                        (character < '0' || character > '9') && character != '_') ||
                    IsPrivateSourceIdentitySegment(segment))
                {
                    return false;
                }
            }
            return true;
        }

        private static bool IsPrivateSourceIdentitySegment(string segment)
        {
            var components = segment.Split('_');
            for (var index = 0; index < components.Length; index++)
            {
                var component = components[index];
                if (component == "source" || component == "raw" ||
                    HasNumericSuffix(component, "source") ||
                    HasNumericSuffix(component, "raw") ||
                    HasNumericSuffix(component, "level") ||
                    new[] { "class", "model", "action", "selector", "family", "slot" }
                        .Any(prefix => component == prefix ||
                            HasNumericSuffix(component, prefix)) ||
                    (component == "level" && index + 1 < components.Length &&
                        components[index + 1].All(char.IsDigit)))
                {
                    return true;
                }
            }
            return false;
        }

        private static bool HasNumericSuffix(string value, string prefix)
        {
            return value.StartsWith(prefix, StringComparison.Ordinal) &&
                value.Length > prefix.Length &&
                value.Substring(prefix.Length).All(char.IsDigit);
        }

        private static bool SameRuntimeFlow(
            RuntimeFlowDefinition left,
            RuntimeFlowDefinition right)
        {
            if (left == null || right == null)
            {
                return !HasRuntimeFlowData(left) && !HasRuntimeFlowData(right);
            }
            if (left.game_id != right.game_id ||
                left.bound_level_id != right.bound_level_id ||
                left.entry_level_id != right.entry_level_id ||
                left.entries == null || right.entries == null ||
                left.entries.Length != right.entries.Length)
            {
                return false;
            }
            for (var index = 0; index < left.entries.Length; index++)
            {
                var leftEntry = left.entries[index];
                var rightEntry = right.entries[index];
                if (leftEntry?.completion_destination == null ||
                    rightEntry?.completion_destination == null ||
                    leftEntry.level_id != rightEntry.level_id ||
                    leftEntry.completion_destination.kind !=
                        rightEntry.completion_destination.kind ||
                    leftEntry.completion_destination.level_id !=
                        rightEntry.completion_destination.level_id ||
                    leftEntry.completion_destination.node_id !=
                        rightEntry.completion_destination.node_id)
                {
                    return false;
                }
            }
            var leftSecrets = left.secret_entries ?? Array.Empty<RuntimeFlowSecretEntry>();
            var rightSecrets = right.secret_entries ?? Array.Empty<RuntimeFlowSecretEntry>();
            if (leftSecrets.Length != rightSecrets.Length) return false;
            for (var index = 0; index < leftSecrets.Length; index++)
            {
                if (leftSecrets[index]?.origin_level_id != rightSecrets[index]?.origin_level_id ||
                    leftSecrets[index]?.secret_level_id != rightSecrets[index]?.secret_level_id)
                {
                    return false;
                }
            }
            return true;
        }

        private static bool SameIdentity(Identity left, Identity right)
        {
            return left != null && right != null &&
                left.runtime_contract == right.runtime_contract &&
                left.runtime_id == right.runtime_id &&
                left.generation == right.generation &&
                left.mode == right.mode &&
                SameCampaignReturnMedia(left.frontend_media, right.frontend_media) &&
                SamePresentationCue(left.presentation, right.presentation) &&
                SameDefinitionIdentity(left.pending_definition, right.pending_definition) &&
                SameRuntimeFlow(left.active_runtime_flow, right.active_runtime_flow) &&
                SameCampaignMenu(left.campaign_menu, right.campaign_menu) &&
                SameSessionIdentity(left.session, right.session) &&
                SameSecretCampaign(left.secret_campaign, right.secret_campaign);
        }

        private static bool SameCampaignReturnMedia(MediaSnapshot left, MediaSnapshot right)
        {
            var first = left?.campaign_return;
            var second = right?.campaign_return;
            if (first == null || second == null) return first == second;
            if (first.phase != second.phase || first.blocking != second.blocking ||
                first.cutscene_id != second.cutscene_id) return false;
            if (first.credits == null || second.credits == null) return first.credits == second.credits;
            return first.credits.cursor == second.credits.cursor &&
                first.credits.lines != null && second.credits.lines != null &&
                first.credits.lines.SequenceEqual(second.credits.lines);
        }

        private static bool HasCampaignMenuData(CampaignMenuSnapshot menu) => menu != null &&
            (!string.IsNullOrEmpty(menu.phase) || !string.IsNullOrEmpty(menu.completed_level_id) ||
             menu.completed_levels?.Length > 0 || menu.available_levels?.Length > 0 ||
             !string.IsNullOrEmpty(menu.selected_level_id) || !string.IsNullOrEmpty(menu.briefing_level_id) ||
             menu.briefing_speech != null);

        private static bool SameCampaignMenu(CampaignMenuSnapshot left, CampaignMenuSnapshot right)
        {
            if (!HasCampaignMenuData(left) || !HasCampaignMenuData(right))
                return !HasCampaignMenuData(left) && !HasCampaignMenuData(right);
            return left.phase == right.phase && left.completed_level_id == right.completed_level_id &&
                left.selected_level_id == right.selected_level_id && left.briefing_level_id == right.briefing_level_id &&
                SameBriefingSpeech(left.briefing_speech, right.briefing_speech) &&
                left.completed_levels.SequenceEqual(right.completed_levels) && left.available_levels.SequenceEqual(right.available_levels);
        }

        internal static bool SameSecretCampaign(
            SecretCampaignSnapshot left,
            SecretCampaignSnapshot right)
        {
            if (left == null || right == null) return left == right;
            if (left.discovered_levels == null || right.discovered_levels == null ||
                left.completed_levels == null || right.completed_levels == null)
            {
                return false;
            }
            var leftStatistics = left.level_statistics ?? Array.Empty<CompletedLevelStatistics>();
            var rightStatistics = right.level_statistics ?? Array.Empty<CompletedLevelStatistics>();
            if (leftStatistics.Length != rightStatistics.Length) return false;
            for (var i = 0; i < leftStatistics.Length; i++)
            {
                var a = leftStatistics[i];
                var b = rightStatistics[i];
                if (a == null || b == null || a.level_id != b.level_id ||
                    !SameCompletionStatistics(a.statistics, b.statistics))
                    return false;
            }
            return left.return_origin == right.return_origin &&
                left.discovered_levels.SequenceEqual(right.discovered_levels) &&
                left.completed_levels.SequenceEqual(right.completed_levels);
        }

        internal static bool SameCompletionStatistics(CompletionStatistics left, CompletionStatistics right) =>
            left != null && right != null &&
            left.spells_found_percent == right.spells_found_percent &&
            left.hit_accuracy_percent == right.hit_accuracy_percent &&
            left.creatures_killed_percent == right.creatures_killed_percent &&
            left.mana_collected_percent == right.mana_collected_percent &&
            left.time_taken_seconds == right.time_taken_seconds;

        private static bool SameBriefingSpeech(CampaignSpeechSegment left, CampaignSpeechSegment right)
        {
            if (left == null || right == null) return left == right;
            return left.track == right.track && left.segment == right.segment &&
                left.start_ms == right.start_ms && left.length_ms == right.length_ms;
        }

        private static bool SameDefinitionIdentity(
            DefinitionIdentity left,
            DefinitionIdentity right)
        {
            if (left == null || right == null)
            {
                return !HasDefinitionIdentityData(left) && !HasDefinitionIdentityData(right);
            }
            return left.package_graph_hash == right.package_graph_hash &&
                left.definition_hash == right.definition_hash &&
                left.definition_revision == right.definition_revision &&
                SameRuntimeFlow(left.runtime_flow, right.runtime_flow);
        }

        private static bool SameSessionIdentity(
            EngineSessionTransport.Identity left,
            EngineSessionTransport.Identity right)
        {
            if (left == null || right == null)
            {
                return !HasSessionIdentityData(left) && !HasSessionIdentityData(right);
            }
            return left.engine_contract == right.engine_contract &&
                left.package_graph_hash == right.package_graph_hash &&
                left.definition_hash == right.definition_hash &&
                left.definition_revision == right.definition_revision &&
                left.session_id == right.session_id &&
                left.generation == right.generation &&
                left.tick == right.tick;
        }

        private static bool SameDestination(
            RuntimeFlowDestination left,
            RuntimeFlowDestination right)
        {
            return left != null && right != null &&
                left.kind == right.kind &&
                left.level_id == right.level_id &&
                left.node_id == right.node_id;
        }

        private static bool SamePresentationCue(
            RuntimePresentationCue left,
            RuntimePresentationCue right)
        {
            return left != null && right != null &&
                left.cue_id == right.cue_id &&
                left.phase == right.phase &&
                left.level_id == right.level_id &&
                ((!HasDestinationData(left.completion_destination) &&
                  !HasDestinationData(right.completion_destination)) ||
                 SameDestination(
                     left.completion_destination,
                     right.completion_destination));
        }

        private static bool HasDestinationData(RuntimeFlowDestination destination)
        {
            return destination != null &&
                (!string.IsNullOrEmpty(destination.kind) ||
                 !string.IsNullOrEmpty(destination.level_id) ||
                 !string.IsNullOrEmpty(destination.node_id));
        }

        private static void ValidateDestination(RuntimeFlowDestination destination)
        {
            if (destination?.kind == "level")
            {
                if (!IsSemanticId(destination.level_id, "level") ||
                    !string.IsNullOrEmpty(destination.node_id))
                {
                    throw new InvalidOperationException(
                        "presentation cue level destination is invalid");
                }
                return;
            }
            if (destination?.kind == "title" &&
                IsSemanticId(destination.node_id, "flow") &&
                string.IsNullOrEmpty(destination.level_id))
            {
                return;
            }
            throw new InvalidOperationException(
                "presentation cue destination is invalid");
        }

        private static bool IsLowerHex(string value, int width)
        {
            return value != null && value.Length == width &&
                value.All(character =>
                    (character >= '0' && character <= '9') ||
                    (character >= 'a' && character <= 'f'));
        }

        private byte[] EncodePackageSet(InstalledPackageResource[] packages)
        {
            if (packages == null || packages.Length == 0 ||
                (uint)packages.Length >
                    NegotiatedContract.limits.maximum_installed_package_count)
            {
                throw new ArgumentException(
                    "installed package resources must be nonempty and bounded",
                    nameof(packages));
            }
            foreach (var resource in packages)
            {
                if (resource == null || string.IsNullOrWhiteSpace(resource.package_dir) ||
                    (resource.owner_cache_path != null &&
                     string.IsNullOrWhiteSpace(resource.owner_cache_path)) ||
                    (resource.presentation_cache_path != null &&
                     string.IsNullOrWhiteSpace(resource.presentation_cache_path)) ||
                    (resource.authored_workspace_path != null &&
                     string.IsNullOrWhiteSpace(resource.authored_workspace_path)))
                {
                    throw new ArgumentException(
                        "each installed package resource needs a package path and optional nonblank resource paths",
                        nameof(packages));
                }
            }
            using var stream = new MemoryStream();
            new DataContractJsonSerializer(typeof(InstalledPackageSetRequest)).WriteObject(stream,
                new InstalledPackageSetRequest {
                    request_contract = InstalledPackageSetContract,
                    packages = packages
                });
            var request = stream.ToArray();
            if ((uint)request.Length >
                NegotiatedContract.limits.maximum_installed_package_set_json_bytes)
            {
                throw new ArgumentException(
                    "installed package-set request exceeds the negotiated bound",
                    nameof(packages));
            }
            return request;
        }

        private ulong RequireHandle()
        {
            var handle = Handle;
            if (handle == 0)
            {
                throw new ObjectDisposedException(nameof(GameRuntimeTransport));
            }
            return handle;
        }

        private static T ReadJson<T>(JsonRead read, string operation) where T : class
        {
            var nativeStart = ModernCapture.BeginMeasurement();
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
                if (nativeStart != 0) ModernCapture.EndMeasurement("runtime_native_" + typeof(T).Name, nativeStart);
                var decodeStart = ModernCapture.BeginMeasurement();
                // Loading observations and completion receipts also embed an
                // identity; preserve absent session/statistics there as well.
                var value = typeof(T) == typeof(Contract)
                    ? JsonUtility.FromJson<T>(Encoding.UTF8.GetString(bytes))
                    : DecodeRuntimeJson<T>(bytes, LegacyRuntimeJson);
                if (decodeStart != 0) ModernCapture.EndMeasurement("runtime_decode_" + typeof(T).Name, decodeStart);
                return value ?? throw new InvalidOperationException(operation + " returned null JSON");
            }
            finally
            {
                Marshal.FreeHGlobal(buffer);
            }
        }

        internal static T DecodeRuntimeJson<T>(byte[] bytes, bool legacy) where T : class
        {
            if (!legacy && typeof(T) == typeof(Identity)) return IdentityJson.Decode(bytes) as T;
            using var stream = new MemoryStream(bytes, false);
            return new DataContractJsonSerializer(typeof(T)).ReadObject(stream) as T;
        }

        private static byte[] ReadBytes(
            JsonRead read,
            uint maximumLength,
            string operation)
        {
            Check(read(IntPtr.Zero, 0, out var required), operation + " size", NativeSim.Bounds);
            if (required == 0 || required > maximumLength)
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
                return bytes;
            }
            finally
            {
                Marshal.FreeHGlobal(buffer);
            }
        }

        internal static ulong ParseCounter(string value, string field)
        {
            if (string.IsNullOrEmpty(value) ||
                (value.Length > 1 && value[0] == '0') ||
                !ulong.TryParse(
                    value,
                    NumberStyles.None,
                    CultureInfo.InvariantCulture,
                    out var parsed))
            {
                throw new InvalidOperationException(
                    field + " is not a canonical decimal u64 string");
            }
            return parsed;
        }

        private static void Check(int status, string operation, int expected = NativeSim.Ok)
        {
            if (status != expected)
            {
                throw new TransitionException(operation, status);
            }
        }
    }
}
