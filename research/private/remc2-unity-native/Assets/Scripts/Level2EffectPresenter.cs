using System;
using System.Collections.Generic;

namespace MagicCarpet.Native
{
    /// <summary>
    /// A current rooted Level 2 effect admitted from the ordered native event
    /// page. The generic campaign ABI binds an effect to its source slot and
    /// event ordinal; the current frame position is the only transform carried
    /// by that page, so no managed position or lifetime is invented here.
    /// </summary>
    internal readonly struct Level2EffectPresentation
    {
        internal Level2EffectPresentation(
            byte family,
            byte classId,
            byte model,
            byte action,
            ulong generation,
            ulong tick,
            ushort sourceSlot,
            uint eventOrdinal,
            byte sourcePass,
            ushort x,
            ushort y,
            short z,
            Level2EffectRecipe recipe)
        {
            Family = family;
            ClassId = classId;
            Model = model;
            Action = action;
            Generation = generation;
            Tick = tick;
            SourceSlot = sourceSlot;
            EventOrdinal = eventOrdinal;
            SourcePass = sourcePass;
            X = x;
            Y = y;
            Z = z;
            Recipe = recipe;
        }

        internal byte Family { get; }
        internal byte ClassId { get; }
        internal byte Model { get; }
        internal byte Action { get; }
        internal ulong Generation { get; }
        internal ulong Tick { get; }
        internal ushort SourceSlot { get; }
        internal uint EventOrdinal { get; }
        internal byte SourcePass { get; }
        internal ushort X { get; }
        internal ushort Y { get; }
        internal short Z { get; }
        internal Level2EffectRecipe Recipe { get; }
        internal bool UsesMsprd => Recipe == Level2EffectRecipe.Msprd;
    }

    /// <summary>
    /// Asset recipe selected by the pinned source family. The presenter does
    /// not load or synthesize assets; the owner cache remains the authority
    /// consumed by the eventual Unity visual delegate.
    /// </summary>
    internal enum Level2EffectRecipe : byte
    {
        None = 0,
        Msprd = 1
    }

    /// <summary>
    /// Presentation-only projection for rooted, nonterminal Level 2 effects.
    /// Rust owns allocation, scheduling, terrain mutation, lifetime, and
    /// reclaim. This class admits only the known dynamic-event roots and
    /// drops the current view when the following projection no longer carries
    /// the source event.
    /// </summary>
    internal sealed class Level2EffectPresenter
    {
        private const byte TerrainModificationFamily = 11;
        private const byte MeteorFamily = 17;
        private const byte WindFamily = 19;
        private const byte Model13Family = 21;
        private const byte FireSpheresFamily = 23;
        private const byte RootClass = 10;

        private Level2EffectPresentation[] activeEffects =
            Array.Empty<Level2EffectPresentation>();
        private ulong observedGeneration = ulong.MaxValue;
        private ulong observedTick;

        internal Level2EffectPresentation[] ActiveEffects => activeEffects;

        internal void Present(
            NativeSim.CampaignStepSnapshot step,
            NativeSim.CampaignEvent[] events)
        {
            // A projection is replace-only. Expiry, malformed pages, stale
            // cursors, reset, and disposal therefore cannot leave an older
            // effect view live.
            activeEffects = Array.Empty<Level2EffectPresentation>();
            if (!FrameIsBound(step, events))
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
            }

            var seenSourceSlots = new HashSet<ushort>();
            var admitted = new List<Level2EffectPresentation>();
            foreach (var eventRow in events)
            {
                if (eventRow.kind != NativeSim.CampaignEventDynamicArena ||
                    !TryDescribe(eventRow.family, out var model, out var action, out var recipe))
                {
                    // Other dynamic families, authored-group rows, and spell
                    // acquisitions belong to their own presenters.
                    continue;
                }

                if (eventRow.flags != 1 ||
                    eventRow.sourceSlot == 0 ||
                    !seenSourceSlots.Add(eventRow.sourceSlot))
                {
                    // A known root with no successful source slot, or two
                    // views claiming one source slot in one ordered page, is
                    // ambiguous. Fail closed instead of duplicating a view.
                    activeEffects = Array.Empty<Level2EffectPresentation>();
                    return;
                }

                admitted.Add(new Level2EffectPresentation(
                    eventRow.family,
                    RootClass,
                    model,
                    action,
                    step.generation,
                    step.revision,
                    eventRow.sourceSlot,
                    eventRow.ordinal,
                    eventRow.pass,
                    step.x,
                    step.y,
                    step.z,
                    recipe));
            }

            observedGeneration = step.generation;
            observedTick = step.revision;
            activeEffects = admitted.ToArray();
        }

        internal void Reset(ulong generation)
        {
            activeEffects = Array.Empty<Level2EffectPresentation>();
            observedGeneration = generation;
            observedTick = 0;
        }

        internal void Dispose()
        {
            activeEffects = Array.Empty<Level2EffectPresentation>();
            observedGeneration = ulong.MaxValue;
            observedTick = 0;
        }

        private static bool FrameIsBound(
            NativeSim.CampaignStepSnapshot step,
            NativeSim.CampaignEvent[] events)
        {
            if (events == null)
            {
                return false;
            }

            if (step.abiVersion != NativeSim.AbiVersion ||
                step.structSize != 112 ||
                step.eventCount != (uint)events.Length)
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

        private static bool TryDescribe(
            byte family,
            out byte model,
            out byte action,
            out Level2EffectRecipe recipe)
        {
            switch (family)
            {
                case TerrainModificationFamily:
                    model = 45;
                    action = 51;
                    recipe = Level2EffectRecipe.None;
                    return true;
                case MeteorFamily:
                    model = 17;
                    action = 17;
                    recipe = Level2EffectRecipe.None;
                    return true;
                case WindFamily:
                    model = 22;
                    action = 22;
                    recipe = Level2EffectRecipe.None;
                    return true;
                case Model13Family:
                    model = 13;
                    action = 19;
                    recipe = Level2EffectRecipe.None;
                    return true;
                case FireSpheresFamily:
                    model = 76;
                    action = 83;
                    recipe = Level2EffectRecipe.Msprd;
                    return true;
                default:
                    model = 0;
                    action = 0;
                    recipe = Level2EffectRecipe.None;
                    return false;
            }
        }
    }
}
