using System;
using System.Collections.Generic;

namespace MagicCarpet.Native
{
    /// <summary>
    /// One current Unity effect admission copied from the Level 2 Rust
    /// campaign stream. These fields are identifiers and timeline bindings,
    /// not a managed spell simulation.
    /// </summary>
    internal readonly struct Level2SpellPresentation
    {
        internal Level2SpellPresentation(
            byte family,
            ulong generation,
            ulong tick,
            ushort sourceSlot,
            uint eventOrdinal,
            ushort holderEntityIndex,
            ushort parentEntityIndex)
        {
            Family = family;
            Generation = generation;
            Tick = tick;
            SourceSlot = sourceSlot;
            EventOrdinal = eventOrdinal;
            HolderEntityIndex = holderEntityIndex;
            ParentEntityIndex = parentEntityIndex;
        }

        internal byte Family { get; }
        internal ulong Generation { get; }
        internal ulong Tick { get; }
        internal ushort SourceSlot { get; }
        internal uint EventOrdinal { get; }
        internal ushort HolderEntityIndex { get; }
        internal ushort ParentEntityIndex { get; }
    }

    /// <summary>
    /// A successful Rust spell-acquisition event retained to bind later
    /// active snapshots back to their source slot and event ordinal.
    /// </summary>
    internal readonly struct Level2SpellPresentationReceipt
    {
        internal Level2SpellPresentationReceipt(
            byte family,
            ulong generation,
            ulong tick,
            ushort sourceSlot,
            uint eventOrdinal)
        {
            Family = family;
            Generation = generation;
            Tick = tick;
            SourceSlot = sourceSlot;
            EventOrdinal = eventOrdinal;
        }

        internal byte Family { get; }
        internal ulong Generation { get; }
        internal ulong Tick { get; }
        internal ushort SourceSlot { get; }
        internal uint EventOrdinal { get; }
    }

    /// <summary>
    /// Presentation-only admission for the three non-rooted Level 2 spell
    /// holder families. Rust remains the sole authority for acquisition,
    /// activation, mana, visibility, expiry, and cleanup; this class merely
    /// rejects mismatched records before Unity draws a current effect label.
    /// </summary>
    internal sealed class Level2SpellPresenter
    {
        private const byte ShieldFamily = 6;
        private const byte InvisibleFamily = 11;
        private const byte BeyondSightFamily = 12;
        private const byte ShieldAction = 18;
        private const byte InvisibleAction = 33;
        private const byte BeyondSightAction = 36;

        private readonly Dictionary<byte, Level2SpellPresentationReceipt> receipts =
            new();
        private readonly Level2EffectPresenter rootedEffectPresenter =
            new();
        private Level2SpellPresentation[] activeEffects =
            Array.Empty<Level2SpellPresentation>();
        private ulong observedGeneration = ulong.MaxValue;
        private ulong observedTick;

        internal Level2SpellPresentation[] ActiveEffects => activeEffects;

        internal Level2EffectPresentation[] RootedEffects =>
            rootedEffectPresenter.ActiveEffects;

        internal Level2SpellPresentationReceipt[] Receipts
        {
            get
            {
                var ordered = new List<Level2SpellPresentationReceipt>(3);
                AppendReceipt(ordered, ShieldFamily);
                AppendReceipt(ordered, InvisibleFamily);
                AppendReceipt(ordered, BeyondSightFamily);
                return ordered.ToArray();
            }
        }

        internal void Present(
            NativeSim.CampaignStepSnapshot step,
            NativeSim.CampaignEvent[] events,
            NativeSim.CampaignDayHudSnapshot hud,
            NativeSim.CampaignShieldSnapshot shield,
            NativeSim.CampaignInvisibleSnapshot invisible,
            NativeSim.CampaignBeyondSightSnapshot beyondSight)
        {
            // Every call starts with no visible effect. A malformed or stale
            // record therefore cannot leave a prior frame's Unity effect live.
            activeEffects = Array.Empty<Level2SpellPresentation>();
            rootedEffectPresenter.Present(step, events);
            if (!FrameIsBound(step, events, hud, shield, invisible, beyondSight))
            {
                return;
            }

            if (observedGeneration != ulong.MaxValue)
            {
                if (step.generation < observedGeneration ||
                    (step.generation == observedGeneration &&
                     step.revision < observedTick))
                {
                    return;
                }
                if (step.generation > observedGeneration)
                {
                    receipts.Clear();
                }
            }

            if (!TryRecordAcquisitions(step, events))
            {
                return;
            }

            observedGeneration = step.generation;
            observedTick = step.revision;
            var admitted = new List<Level2SpellPresentation>(3);
            TryPresentShield(admitted, step, hud, shield);
            TryPresentInvisible(admitted, step, hud, invisible);
            TryPresentBeyondSight(admitted, step, hud, beyondSight);
            activeEffects = admitted.ToArray();
        }

        internal void Reset(ulong generation)
        {
            receipts.Clear();
            activeEffects = Array.Empty<Level2SpellPresentation>();
            rootedEffectPresenter.Reset(generation);
            observedGeneration = generation;
            observedTick = 0;
        }

        internal void Dispose()
        {
            receipts.Clear();
            activeEffects = Array.Empty<Level2SpellPresentation>();
            rootedEffectPresenter.Dispose();
            observedGeneration = ulong.MaxValue;
            observedTick = 0;
        }

        private void AppendReceipt(
            List<Level2SpellPresentationReceipt> ordered,
            byte family)
        {
            if (receipts.TryGetValue(family, out var receipt))
            {
                ordered.Add(receipt);
            }
        }

        private static bool FrameIsBound(
            NativeSim.CampaignStepSnapshot step,
            NativeSim.CampaignEvent[] events,
            NativeSim.CampaignDayHudSnapshot hud,
            NativeSim.CampaignShieldSnapshot shield,
            NativeSim.CampaignInvisibleSnapshot invisible,
            NativeSim.CampaignBeyondSightSnapshot beyondSight)
        {
            if (events == null)
            {
                return false;
            }

            if (step.abiVersion != NativeSim.AbiVersion ||
                step.structSize != 112 ||
                step.eventCount != (uint)events.Length ||
                hud.abiVersion != NativeSim.AbiVersion ||
                hud.structSize != 272 ||
                hud.generation != step.generation ||
                hud.tick != step.revision ||
                hud.localPlayerActive != 1 ||
                hud.localPlayerEntityIndex == 0 ||
                step.playerSlot != hud.localPlayerSlot ||
                step.playerEntityIndex != hud.localPlayerEntityIndex ||
                shield.abiVersion != NativeSim.AbiVersion ||
                shield.structSize != 72 ||
                shield.generation != step.generation ||
                invisible.abiVersion != NativeSim.AbiVersion ||
                invisible.structSize != 80 ||
                invisible.generation != step.generation ||
                beyondSight.abiVersion != NativeSim.AbiVersion ||
                beyondSight.structSize != 80 ||
                beyondSight.generation != step.generation)
            {
                return false;
            }

            var ordinals = new HashSet<uint>();
            for (var index = 0; index < events.Length; index++)
            {
                var eventRow = events[index];
                if (eventRow.abiVersion != NativeSim.AbiVersion ||
                    eventRow.structSize != 48 ||
                    eventRow.generation != step.generation ||
                    eventRow.ordinal != (uint)index ||
                    !ordinals.Add(eventRow.ordinal))
                {
                    return false;
                }
            }
            return true;
        }

        private bool TryRecordAcquisitions(
            NativeSim.CampaignStepSnapshot step,
            NativeSim.CampaignEvent[] events)
        {
            var pending = new Dictionary<byte, Level2SpellPresentationReceipt>();
            foreach (var eventRow in events)
            {
                if (eventRow.kind != NativeSim.CampaignEventSpellAcquisition ||
                    !IsKnownFamily(eventRow.family))
                {
                    continue;
                }
                if (eventRow.flags != 1 || eventRow.sourceSlot == 0)
                {
                    // A rejected or source-less event must never authorize a
                    // Unity effect. Existing valid receipts are retained for
                    // their own live holder, but no fallback is introduced.
                    continue;
                }
                if (!pending.TryAdd(
                        eventRow.family,
                        new Level2SpellPresentationReceipt(
                            eventRow.family,
                            step.generation,
                            step.revision,
                            eventRow.sourceSlot,
                            eventRow.ordinal)))
                {
                    // Two success receipts for one family in the same ordered
                    // batch are ambiguous, so keep this frame dark.
                    return false;
                }
            }

            foreach (var pair in pending)
            {
                receipts[pair.Key] = pair.Value;
            }
            return true;
        }

        private void TryPresentShield(
            List<Level2SpellPresentation> admitted,
            NativeSim.CampaignStepSnapshot step,
            NativeSim.CampaignDayHudSnapshot hud,
            NativeSim.CampaignShieldSnapshot state)
        {
            TryPresent(
                admitted,
                step,
                hud,
                ShieldFamily,
                ShieldAction,
                state.active,
                state.tier,
                state.action,
                state.holderEntityIndex,
                state.parentEntityIndex,
                state.activeValue,
                state.activationValue,
                cleanupRequested: false);
        }

        private void TryPresentInvisible(
            List<Level2SpellPresentation> admitted,
            NativeSim.CampaignStepSnapshot step,
            NativeSim.CampaignDayHudSnapshot hud,
            NativeSim.CampaignInvisibleSnapshot state)
        {
            TryPresent(
                admitted,
                step,
                hud,
                InvisibleFamily,
                InvisibleAction,
                state.active,
                state.tier,
                state.action,
                state.holderEntityIndex,
                state.parentEntityIndex,
                state.activeValue,
                state.activationValue,
                state.cleanupRequested != 0);
        }

        private void TryPresentBeyondSight(
            List<Level2SpellPresentation> admitted,
            NativeSim.CampaignStepSnapshot step,
            NativeSim.CampaignDayHudSnapshot hud,
            NativeSim.CampaignBeyondSightSnapshot state)
        {
            TryPresent(
                admitted,
                step,
                hud,
                BeyondSightFamily,
                BeyondSightAction,
                state.active,
                state.tier,
                state.action,
                state.holderEntityIndex,
                state.parentEntityIndex,
                state.activeValue,
                state.activationValue,
                state.cleanupRequested != 0);
        }

        private void TryPresent(
            List<Level2SpellPresentation> admitted,
            NativeSim.CampaignStepSnapshot step,
            NativeSim.CampaignDayHudSnapshot hud,
            byte family,
            byte expectedAction,
            byte holderActive,
            byte tier,
            byte action,
            ushort holderEntityIndex,
            ushort parentEntityIndex,
            short activeValue,
            short activationValue,
            bool cleanupRequested)
        {
            if (!receipts.TryGetValue(family, out var receipt))
            {
                return;
            }
            if (holderActive != 1)
            {
                receipts.Remove(family);
                return;
            }
            if (cleanupRequested ||
                action != expectedAction ||
                activeValue <= 0 ||
                activationValue <= 0 ||
                receipt.Generation != step.generation ||
                receipt.SourceSlot == 0 ||
                receipt.SourceSlot != holderEntityIndex ||
                holderEntityIndex == 0 ||
                parentEntityIndex == 0 ||
                parentEntityIndex != hud.localPlayerEntityIndex ||
                !TryFindAdmittedHudSlot(
                    hud,
                    family,
                    action,
                    tier,
                    activeValue,
                    activationValue,
                    holderEntityIndex,
                    parentEntityIndex))
            {
                return;
            }

            admitted.Add(new Level2SpellPresentation(
                family,
                step.generation,
                step.revision,
                receipt.SourceSlot,
                receipt.EventOrdinal,
                holderEntityIndex,
                parentEntityIndex));
        }

        private static bool TryFindAdmittedHudSlot(
            NativeSim.CampaignDayHudSnapshot hud,
            byte family,
            byte action,
            byte tier,
            short activeValue,
            short activationValue,
            ushort holderEntityIndex,
            ushort parentEntityIndex)
        {
            return HudSlotIsAdmitted(
                       hud.left,
                       family,
                       action,
                       tier,
                       activeValue,
                       activationValue,
                       holderEntityIndex,
                       parentEntityIndex) ||
                   HudSlotIsAdmitted(
                       hud.right,
                       family,
                       action,
                       tier,
                       activeValue,
                       activationValue,
                       holderEntityIndex,
                       parentEntityIndex);
        }

        private static bool HudSlotIsAdmitted(
            NativeSim.CampaignDayHudSpellSlot slot,
            byte family,
            byte action,
            byte tier,
            short activeValue,
            short activationValue,
            ushort holderEntityIndex,
            ushort parentEntityIndex)
        {
            return slot.rawSpellSelector == family &&
                   slot.model == family &&
                   slot.tier >= 0 &&
                   slot.tier == tier &&
                   slot.action == action &&
                   slot.activeValue == activeValue &&
                   slot.activationValue == activationValue &&
                   slot.holderEntityIndex == holderEntityIndex &&
                   slot.parentEntityIndex == parentEntityIndex &&
                   slot.drawAdmitted == 1;
        }

        private static bool IsKnownFamily(byte family)
        {
            return family is ShieldFamily or InvisibleFamily or BeyondSightFamily;
        }
    }
}
