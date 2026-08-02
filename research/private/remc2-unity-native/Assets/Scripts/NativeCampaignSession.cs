using System;
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
