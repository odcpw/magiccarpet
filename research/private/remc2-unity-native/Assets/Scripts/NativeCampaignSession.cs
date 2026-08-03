using System;
using System.Collections.Generic;
using System.Linq;
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
        private long handleBits;
        private ulong generation;

        private NativeCampaignSession(ulong handle)
        {
            handleBits = unchecked((long)handle);
        }

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

        internal NativeSim.CampaignWorldInfo WorldInfo()
        {
            var status = NativeSim.mc_native_campaign_world_info(
                Handle,
                out var info,
                64);
            NativeSession.Check(status, "read campaign world info");
            return info;
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
                UnityEngine.Debug.LogError(
                    $"destroy campaign session failed: {status}: " +
                    NativeSession.ReadLastError());
            }
        }
    }
}
