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
        internal const string RuntimeContract = "carpet-flight-game-runtime/v1";
        internal const string TransportContract =
            "carpet-flight-native-game-runtime-json/v1";
        internal const string TitleMode = "title";
        internal const string LoadingMode = "loading";
        internal const string PlayingMode = "playing";

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
            public Limits limits;
        }

        [Serializable]
        internal sealed class Limits
        {
            public uint maximum_output_json_bytes;
            public string json_length_type;
        }

        [Serializable]
        internal sealed class DefinitionIdentity
        {
            public string package_graph_hash;
            public string definition_hash;
            public string definition_revision;
        }

        [Serializable]
        internal sealed class Identity
        {
            public string runtime_contract;
            public string runtime_id;
            public string generation;
            public string mode;
            public DefinitionIdentity pending_definition;
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

        internal Identity Current { get; private set; }

        internal GameRuntimeTransport()
        {
            NegotiatedContract = ReadContract();
            ValidateContract(NegotiatedContract);
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

        internal Identity NewGame(string packagePath, string ownerCachePath = null)
        {
            if (string.IsNullOrWhiteSpace(packagePath))
            {
                throw new ArgumentException("campaign package path is required", nameof(packagePath));
            }
            var package = Encoding.UTF8.GetBytes(packagePath);
            var cache = string.IsNullOrWhiteSpace(ownerCachePath)
                ? Array.Empty<byte>()
                : Encoding.UTF8.GetBytes(ownerCachePath);
            Check(
                NativeSim.mc_game_runtime_new_game_v1(
                    RequireHandle(),
                    ParseCounter(Current.generation, "generation"),
                    package,
                    checked((uint)package.Length),
                    cache,
                    checked((uint)cache.Length)),
                "begin new game");
            var identity = IdentitySnapshot();
            if (identity.mode != LoadingMode || !HasDefinitionIdentity(identity) ||
                HasSessionIdentity(identity))
            {
                throw new InvalidOperationException("new game did not enter semantic Loading");
            }
            return identity;
        }

        internal Identity OpenLevel()
        {
            Check(
                NativeSim.mc_game_runtime_open_level_v1(
                    RequireHandle(),
                    ParseCounter(Current.generation, "generation")),
                "open pending level");
            var identity = IdentitySnapshot();
            if (identity.mode != PlayingMode || HasDefinitionIdentity(identity) ||
                !HasSessionIdentity(identity) ||
                ParseCounter(identity.session.session_id, "session_id") != Handle)
            {
                throw new InvalidOperationException("open level did not enter bound Playing");
            }
            return identity;
        }

        internal EngineSessionTransport.CampaignSnapshotObservation CampaignSnapshot()
        {
            if (Current?.mode != PlayingMode)
            {
                throw new InvalidOperationException("campaign snapshot requires Playing mode");
            }
            var snapshot = ReadJson<EngineSessionTransport.CampaignSnapshotObservation>(
                (IntPtr buffer, uint capacity, out uint length) =>
                    NativeSim.mc_game_session_snapshot_json_v1(
                        RequireHandle(), buffer, capacity, out length),
                "read runtime campaign snapshot");
            if (snapshot?.identity == null || snapshot.value == null ||
                snapshot.identity.session_id != Current.session.session_id ||
                snapshot.identity.generation != Current.session.generation ||
                snapshot.identity.tick != Current.session.tick)
            {
                throw new InvalidOperationException("runtime campaign snapshot is not identity-bound");
            }
            EngineSessionTransport.ValidateIdentity(snapshot.identity);
            EngineSessionTransport.ValidateCampaignSnapshot(snapshot.value, snapshot.identity);
            return snapshot;
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
                contract.transport_version != 1 ||
                contract.encoding != "utf-8-json" ||
                contract.u64_counter_encoding != "decimal-string" ||
                contract.playing_session_transport !=
                    "mc_game_session_*_v1 on the same handle" ||
                contract.package_profile != EngineSessionTransport.CampaignProfile ||
                contract.limits == null ||
                contract.limits.maximum_output_json_bytes != 16 * 1024 * 1024 ||
                contract.limits.json_length_type != "u32" ||
                contract.modes == null ||
                !new[] { TitleMode, LoadingMode, PlayingMode }.SequenceEqual(contract.modes) ||
                contract.transitions == null ||
                !new[] { "new_game", "open_level" }.SequenceEqual(contract.transitions))
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

        private static void ValidateIdentity(Identity identity, ulong handle)
        {
            if (identity == null || identity.runtime_contract != RuntimeContract ||
                ParseCounter(identity.runtime_id, "runtime_id") != handle ||
                !new[] { TitleMode, LoadingMode, PlayingMode }.Contains(identity.mode))
            {
                throw new InvalidOperationException("game runtime identity is incomplete");
            }
            ParseCounter(identity.generation, "generation");
            switch (identity.mode)
            {
                case TitleMode:
                    if (HasDefinitionIdentity(identity) || HasSessionIdentity(identity))
                    {
                        throw new InvalidOperationException(
                            "Title runtime identity carries an unexpected binding");
                    }
                    break;
                case LoadingMode:
                    if (!HasDefinitionIdentity(identity) || HasSessionIdentity(identity) ||
                        string.IsNullOrWhiteSpace(
                            identity.pending_definition.package_graph_hash))
                    {
                        throw new InvalidOperationException(
                            "Loading runtime identity is incomplete");
                    }
                    ParseCounter(
                        identity.pending_definition.definition_revision,
                        "definition_revision");
                    break;
                case PlayingMode:
                    if (HasDefinitionIdentity(identity) || !HasSessionIdentity(identity))
                    {
                        throw new InvalidOperationException(
                            "Playing runtime identity is incomplete");
                    }
                    EngineSessionTransport.ValidateIdentity(identity.session);
                    break;
            }
        }

        private static bool HasDefinitionIdentity(Identity identity)
        {
            return !string.IsNullOrEmpty(identity?.pending_definition?.definition_hash);
        }

        private static bool HasSessionIdentity(Identity identity)
        {
            return !string.IsNullOrEmpty(identity?.session?.session_id);
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
