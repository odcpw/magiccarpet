using System;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
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
        internal const string RuntimeContract = "carpet-flight-game-runtime/v6";
        internal const string TransportContract =
            "carpet-flight-native-game-runtime-json/v4";
        internal const string InstalledPackageSetContract =
            "carpet-flight-installed-package-set-bootstrap/v1";
        internal const string TitleMode = "title";
        internal const string LoadingMode = "loading";
        internal const string PlayingMode = "playing";
        internal const string PausedMode = "paused";

        private delegate int JsonRead(IntPtr buffer, uint capacity, out uint length);

        [Serializable]
        internal sealed class Contract
        {
            public string runtime_contract;
            public string transport_contract;
            public int transport_version;
            public string encoding;
            public string u64_counter_encoding;
            public string[] modes;
            public string[] transitions;
            public string playing_session_transport;
            public string package_profile;
            public InstalledPackageSetTransport installed_package_set;
            public RuntimeFlowContract runtime_flow;
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
        internal sealed class InstalledPackageResource
        {
            public string package_dir;
            public string owner_cache_path;
        }

        [Serializable]
        internal sealed class InstalledPackageSetRequest
        {
            public string request_contract;
            public InstalledPackageResource[] packages;
        }

        [Serializable]
        internal sealed class RuntimeFlowContract
        {
            public string title;
            public string loading;
            public string playing_and_paused;
        }

        [Serializable]
        internal sealed class Limits
        {
            public uint maximum_output_json_bytes;
            public uint maximum_installed_package_set_json_bytes;
            public uint maximum_installed_package_count;
            public string json_length_type;
        }

        [Serializable]
        internal sealed class DefinitionIdentity
        {
            public string package_graph_hash;
            public string definition_hash;
            public string definition_revision;
            public RuntimeFlowDefinition runtime_flow;
        }

        [Serializable]
        internal sealed class RuntimeFlowDestination
        {
            public string kind;
            public string level_id;
            public string node_id;
        }

        [Serializable]
        internal sealed class RuntimeFlowEntry
        {
            public string level_id;
            public RuntimeFlowDestination completion_destination;
        }

        [Serializable]
        internal sealed class RuntimeFlowDefinition
        {
            public string game_id;
            public string bound_level_id;
            public string entry_level_id;
            public RuntimeFlowEntry[] entries;
        }

        [Serializable]
        internal sealed class Identity
        {
            public string runtime_contract;
            public string runtime_id;
            public string generation;
            public string mode;
            public DefinitionIdentity pending_definition;
            public RuntimeFlowDefinition active_runtime_flow;
            public EngineSessionTransport.Identity session;
        }

        internal sealed class TransitionException : InvalidOperationException
        {
            internal int Status { get; }

            internal TransitionException(string operation, int status)
                : base($"{operation} failed with native status {status}")
            {
                Status = status;
            }
        }

        private long handleBits;

        internal ulong Handle => unchecked((ulong)Interlocked.Read(ref handleBits));

        internal Contract NegotiatedContract { get; }

        internal EngineSessionTransport.Contract NegotiatedSessionContract { get; }

        internal Identity Current { get; private set; }

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

        internal Identity IdentitySnapshot()
        {
            var handle = RequireHandle();
            var identity = ReadJson<Identity>(
                (IntPtr buffer, uint capacity, out uint length) =>
                    NativeSim.mc_game_runtime_identity_json_v1(
                        handle, buffer, capacity, out length),
                "read game runtime identity");
            ValidateIdentity(identity, handle);
            Current = identity;
            return identity;
        }

        internal Identity BootstrapPackageSet(params InstalledPackageResource[] packages)
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
                     string.IsNullOrWhiteSpace(resource.owner_cache_path)))
                {
                    throw new ArgumentException(
                        "each installed package resource needs a package path and optional nonblank cache path",
                        nameof(packages));
                }
            }
            var request = Encoding.UTF8.GetBytes(JsonUtility.ToJson(
                new InstalledPackageSetRequest
                {
                    request_contract = InstalledPackageSetContract,
                    packages = packages
                }));
            if ((uint)request.Length >
                NegotiatedContract.limits.maximum_installed_package_set_json_bytes)
            {
                throw new ArgumentException(
                    "installed package-set request exceeds the negotiated bound",
                    nameof(packages));
            }
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

        internal EngineSessionTransport.CampaignSnapshotObservation CampaignSnapshot()
        {
            RequireSessionMode(allowPaused: true, "campaign snapshot");
            return ValidateCampaignObservation(
                EngineSessionTransport.CampaignSnapshotForHandle(RequireHandle()));
        }

        internal EngineSessionTransport.CampaignSnapshotObservation CampaignStep(
            sbyte roll,
            sbyte pitch,
            sbyte flags,
            ushort lookYaw,
            ushort lookPitch)
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
                NegotiatedSessionContract.limits.maximum_command_json_bytes);
            var identity = IdentitySnapshot();
            if (identity.mode != PlayingMode ||
                ParseCounter(identity.generation, "generation") != beforeRuntimeGeneration ||
                !PreservesSessionBinding(before.session, identity.session) ||
                !SameRuntimeFlow(before.active_runtime_flow, identity.active_runtime_flow) ||
                ParseCounter(identity.session.generation, "session_generation") !=
                    beforeSessionGeneration ||
                ParseCounter(identity.session.tick, "tick") != checked(beforeTick + 1))
            {
                throw new InvalidOperationException(
                    "campaign step did not advance only the bound gameplay tick");
            }
            return CampaignSnapshot();
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
                contract.transport_version != 4 ||
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
                contract.runtime_flow == null ||
                contract.runtime_flow.title != "absent" ||
                contract.runtime_flow.loading != "pending_definition.runtime_flow" ||
                contract.runtime_flow.playing_and_paused != "active_runtime_flow" ||
                contract.limits == null ||
                contract.limits.maximum_output_json_bytes != 16 * 1024 * 1024 ||
                contract.limits.maximum_installed_package_set_json_bytes != 64 * 1024 ||
                contract.limits.maximum_installed_package_count != 16 ||
                contract.limits.json_length_type != "u32" ||
                contract.modes == null ||
                !new[] { TitleMode, LoadingMode, PlayingMode, PausedMode }
                    .SequenceEqual(contract.modes) ||
                contract.transitions == null ||
                !new[] { "bootstrap_package_set", "open_level", "pause", "resume" }
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

        internal static void ValidateIdentity(Identity identity, ulong handle)
        {
            if (identity == null || identity.runtime_contract != RuntimeContract ||
                ParseCounter(identity.runtime_id, "runtime_id") != handle ||
                !new[] { TitleMode, LoadingMode, PlayingMode, PausedMode }
                    .Contains(identity.mode))
            {
                throw new InvalidOperationException("game runtime identity is incomplete");
            }
            ParseCounter(identity.generation, "generation");
            switch (identity.mode)
            {
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
                ParseCounter(identity.session.generation, "session_generation") !=
                    checked(beforeSessionGeneration + 1) ||
                ParseCounter(identity.session.tick, "tick") != beforeTick)
            {
                throw new InvalidOperationException(
                    operation + " did not preserve the bound gameplay session");
            }
            return identity;
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
                    flow.entries != null);
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
                flow.entry_level_id != levelIds[0] ||
                !levelIds.Contains(flow.bound_level_id))
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
            if (left == null || right == null ||
                left.game_id != right.game_id ||
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
            return true;
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
