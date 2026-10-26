namespace MagicCarpet.Native
{
    /// <summary>
    /// PlayerInput MouseAndKeysEvents_17A00 menu states 0, 5, 6 and 8.
    /// This is a presentation input owner: no pause or gameplay step lives
    /// here. Bound physical keys are sampled by the device adapter.
    /// </summary>
    internal sealed class SourceCampaignMenus
    {
        internal enum Mode : byte { Flight = 0, Spells = 5, Map = 6, PlayerInformation = 7, MapSpells = 8 }
        internal enum AbandonMenuState : byte { None = 0, FlightConfirmation = 13, MapConfirmation = 14 }

        internal readonly struct Result
        {
            internal readonly Mode Before, After;
            internal readonly bool RequestAbandonConfirmation;
            internal readonly AbandonMenuState AbandonMenuState;
            internal readonly bool MapToggleRequested;
            internal readonly byte CycleHand;
            internal readonly int CycleDirection;
            private readonly bool bothButtonsHeld;
            private readonly bool cycleModifierHeld;
            internal bool Changed => Before != After;
            internal bool MapChanged => IsMap(Before) != IsMap(After);
            internal bool SpellsChanged => IsSpells(Before) != IsSpells(After);
            internal bool SuppressCasting => Changed || IsSpells(After) || bothButtonsHeld || cycleModifierHeld;
            internal Result(Mode before, Mode after, bool abandon, AbandonMenuState abandonMenuState,
                bool mapToggleRequested, bool both, bool cycleModifier, byte cycleHand, int cycleDirection)
            {
                Before = before;
                After = after;
                RequestAbandonConfirmation = abandon;
                AbandonMenuState = abandonMenuState;
                MapToggleRequested = mapToggleRequested;
                bothButtonsHeld = both;
                cycleModifierHeld = cycleModifier;
                CycleHand = cycleHand;
                CycleDirection = cycleDirection;
            }
        }

        private bool previousBoth, previousLeft, previousRight;
        private AbandonMenuState pendingAbandonMenuState;
        internal Mode Current { get; private set; }
        internal bool MapOpen => IsMap(Current);
        internal bool SpellOpen => IsSpells(Current);
        internal bool PlayerInformationOpen => Current == Mode.PlayerInformation;
        internal bool AdmitFlightInput => !SpellOpen;
        internal bool CaptureMouse => !SpellOpen;

        private static bool IsMap(Mode mode) => mode is Mode.Map or Mode.PlayerInformation or Mode.MapSpells;
        private static bool IsSpells(Mode mode) => mode is Mode.Spells or Mode.MapSpells;

        internal void Reset()
        {
            Current = Mode.Flight;
            previousBoth = previousLeft = previousRight = false;
            pendingAbandonMenuState = AbandonMenuState.None;
        }

        internal bool TryGetPendingBanish(out AbandonMenuState menuState)
        {
            menuState = pendingAbandonMenuState;
            return menuState != AbandonMenuState.None;
        }

        /// <summary>
        /// Source sub_18B30 treats key code 1 as action 29 while menu state is
        /// 13 or 14. Keep this as an intent only; the caller clears the pending
        /// state after Rust admits the generation-bound command.
        /// </summary>
        internal bool TryRequestBanishFromSourceKey1(bool key1Down) =>
            key1Down && pendingAbandonMenuState is
                AbandonMenuState.FlightConfirmation or AbandonMenuState.MapConfirmation;

        internal void ConfirmBanishCommandAdmitted() => pendingAbandonMenuState = AbandonMenuState.None;

        internal void CancelAbandonCommandAdmitted() => pendingAbandonMenuState = AbandonMenuState.None;

        internal bool RestoreRejectedMapToggle(Result transition)
        {
            if (!transition.MapToggleRequested || Current != transition.After ||
                IsMap(transition.Before) == IsMap(transition.After))
                return false;
            Current = transition.Before;
            return true;
        }

        internal void CloseSpells()
        {
            if (SpellOpen) Current = MapOpen ? Mode.Map : Mode.Flight;
        }

        internal Result Step(bool enterPressed, bool spellHeld, bool escapePressed,
            bool leftHeld, bool rightHeld, bool alive, bool informationHeld = false,
            bool nextMarkedHeld = false, bool previousMarkedHeld = false)
        {
            var before = Current;
            var both = leftHeld && rightHeld;
            var bothPressed = both && !previousBoth;
            var leftPressed = leftHeld && !previousLeft;
            var rightPressed = rightHeld && !previousRight;
            previousBoth = both;
            previousLeft = leftHeld;
            previousRight = rightHeld;
            var abandon = false;
            var mapToggleRequested = false;
            byte cycleHand = 0;
            var direction = 0;
            var reverse = previousMarkedHeld || MapOpen && informationHeld;
            var cycleModifier = reverse || nextMarkedHeld;
            void Cycle()
            {
                // sub_18DA0 consumes a button pulse, with left precedence.
                cycleHand = leftPressed ? (byte)1 : rightPressed ? (byte)2 : (byte)0;
                direction = reverse ? -1 : 1;
            }
            if (!alive)
            {
                Current = Mode.Flight;
                pendingAbandonMenuState = AbandonMenuState.None;
            }
            else if (SpellOpen)
            {
                // The source retains a drag after Ctrl is released until
                // both physical casting buttons are also released.
                if (!spellHeld && !leftHeld && !rightHeld) CloseSpells();
            }
            else if (MapOpen)
            {
                // The map branch handles close before its Ctrl submenu.
                if (enterPressed || bothPressed)
                {
                    Current = Mode.Flight;
                    mapToggleRequested = true;
                }
                else if (escapePressed)
                {
                    abandon = true;
                    pendingAbandonMenuState = AbandonMenuState.MapConfirmation;
                }
                else if (spellHeld) Current = Mode.MapSpells;
                else if (reverse)
                {
                    Cycle();
                    if (cycleHand == 0) Current = Mode.PlayerInformation;
                }
                else if (nextMarkedHeld) Cycle();
                else Current = Mode.Map;
            }
            else
            {
                // Ctrl has precedence over both-button/Enter map input in
                // the normal flight branch, as the source bit 0x10 gate does.
                if (spellHeld) Current = Mode.Spells;
                else if (cycleModifier) Cycle();
                else if (escapePressed)
                {
                    abandon = true;
                    pendingAbandonMenuState = AbandonMenuState.FlightConfirmation;
                }
                else if (enterPressed || bothPressed)
                {
                    Current = Mode.Map;
                    mapToggleRequested = true;
                }
            }
            // MouseAndKeysEvents consumes both casting pulses for the map
            // gesture. Keeping the physical buttons held cannot start spells
            // on the following frame after that consumed edge.
            return new Result(before, Current, abandon, pendingAbandonMenuState,
                mapToggleRequested, both, cycleModifier, cycleHand, direction);
        }
    }
}
