using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using System.Threading;

namespace MagicCarpet.Native
{
    /// Managed reachability for the additive Level 2 campaign C ABI.
    /// This is intentionally separate from NativeSession: the legacy handle
    /// remains Level 1-only while the campaign handle owns its own generation,
    /// event buffer, and reset lifecycle.
    internal sealed class NativeCampaignSession : IDisposable
    {
        internal const string ExpectedLevel2ManifestSha256 =
            "dcaa5103908ce2db22c71a4b55e2a1a1c9bc7a182b3fee8e3e81b9be40539cc3";
        private static readonly IReadOnlyDictionary<string, string>
            ExpectedLevel2PackageFiles =
            new Dictionary<string, string>
            {
                ["authored-level.bin"] =
                    "9041c66e605d573ec9aa2cbe9d8d8daabd8db3602fcfce4898f64e21fc37405e",
                ["entities-post-features.bin"] =
                    "9c6acead68279d3737046e9dcfd035d261c92886eb4e80961ed70e2119a84a23",
                ["entities.bin"] =
                    "155055a020d642641baff6674253605c309a7891342e05a7878f784e752dab7a",
                ["feature-allocator.bin"] =
                    "7d20f7d0446512b72ba7b699ecb9fcb4fa3eddcd6d782aabdd823af77a840ea1",
                ["feature-construction-inputs.bin"] =
                    "3054ad4408a00a4b154ec4406ab28eb5c4906bc346dcb5bc4c1748f5b10f0205",
                ["feature-end.bin"] =
                    "646b7b6664587a3c1c3b76ed6d37a8367f9f183a7758f3beef2a3e20064bc4bc",
                ["feature-start.bin"] =
                    "363264d46bf0e2dc74e39ab75ab77b65216500472b2ae3912f4eb2b901bacee3",
                ["players.bin"] =
                    "845265777788b9191a038543b1ba408ed32e37133f9d110ca540cab8396982cb",
                ["presentation.bin"] =
                    "2944b802258550f24f0537accc4d5764d81dd28a080a935cc9c0f0d1fc1f67f2",
                ["runtime.bin"] =
                    "6f1734d967a5e739d493faafdb41a822669966db46f9ffbb30cf1cc8bad30133",
                ["terrain-post-features.bin"] =
                    "451db813d79572b043638226101f2eeb313242216adda2740e4be644c070dedf",
                ["terrain-pre-features.bin"] =
                    "47c4818d811357642ae85b2d646463ad8834e259d3dffd608b1a2b84c95fc82b",
                ["terrain.bin"] =
                    "69f21d0840e7f2945b0f0123a954641a1b34e3d5ad8d9bf8a56c25224f57a203",
                ["manifest.json"] = ExpectedLevel2ManifestSha256
            };

        private static int activeSessionCount;
        private long handleBits;
        private ulong generation;

        private NativeCampaignSession(ulong handle)
        {
            handleBits = unchecked((long)handle);
            Interlocked.Increment(ref activeSessionCount);
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

        internal static NativeCampaignSession OpenLevel2(string packagePath)
        {
            if (string.IsNullOrWhiteSpace(packagePath))
            {
                throw new InvalidDataException(
                    "Level 2 campaign package path must be non-empty");
            }
            packagePath = Path.GetFullPath(packagePath);
            ValidateLevel2PackageIdentity(packagePath);
            NativeSim.AssertLayouts();
            if (NativeSim.mc_native_abi_version() != NativeSim.AbiVersion)
            {
                throw new InvalidOperationException(
                    "sim_capi native ABI is not version 2");
            }

            var package = Encoding.UTF8.GetBytes(packagePath);
            var closure = Array.Empty<byte>();
            var selection = NativeSim.CampaignSelection.Level2Day();
            var status = NativeSim.mc_native_campaign_session_open(
                package,
                checked((uint)package.Length),
                closure,
                0,
                ref selection,
                out var handle);
            NativeSession.Check(status, "open Level 2 campaign session");
            if (handle == 0)
            {
                throw new InvalidOperationException(
                    "campaign session returned handle zero");
            }
            return new NativeCampaignSession(handle);
        }

        private static void ValidateLevel2PackageIdentity(string packagePath)
        {
            if (string.IsNullOrWhiteSpace(packagePath))
            {
                throw new InvalidDataException(
                    "Level 2 campaign package path must be non-empty");
            }

            var package = Path.GetFullPath(packagePath);
            if (!Directory.Exists(package))
            {
                throw new InvalidDataException(
                    $"Level 2 campaign package is missing: {package}");
            }

            using var sha256 = SHA256.Create();
            foreach (var expected in ExpectedLevel2PackageFiles)
            {
                var file = Path.Combine(package, expected.Key);
                if (!File.Exists(file))
                {
                    throw new InvalidDataException(
                        $"Level 2 campaign package is missing {expected.Key}");
                }
                var digest = BitConverter.ToString(
                        sha256.ComputeHash(File.ReadAllBytes(file)))
                    .Replace("-", string.Empty)
                    .ToLowerInvariant();
                if (digest != expected.Value)
                {
                    throw new InvalidDataException(
                        $"Level 2 campaign package identity mismatch for " +
                        $"{expected.Key}: {digest}");
                }
            }
        }

        internal NativeSim.CampaignWorldInfo WorldInfo()
        {
            var status = NativeSim.mc_native_campaign_world_info(
                Handle,
                out var info,
                64);
            NativeSession.Check(status, "read campaign world info");
            return info;
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
                sbyte flags = 1)
        {
            var command = NativeSim.Command.Create(0, 0, flags);
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
            NativeSession.Check(status, "step Level 2 campaign session");
            if (eventCount > (uint)events.Length)
            {
                throw new InvalidOperationException(
                    $"campaign event count {eventCount} exceeds buffer {events.Length}");
            }
            generation = snapshot.generation;
            return (snapshot, events.Take(checked((int)eventCount)).ToArray());
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
            NativeSession.Check(status, "reset Level 2 campaign session");
            generation++;
        }

        public void Dispose()
        {
            var value = Interlocked.Exchange(ref handleBits, 0);
            if (value == 0)
            {
                return;
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

        private static bool AllZero(uint[] values)
        {
            return values != null && values.All(value => value == 0);
        }
    }
}
