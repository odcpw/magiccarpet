namespace MagicCarpet.Native
{
    /// <summary>
    /// PlayerInput MouseAndKeysEvents_17A00 menu states 0, 5, 6 and 8.
    /// This is a presentation input owner: no pause or gameplay step lives
    /// here. Bound physical keys are sampled by the device adapter.
    /// </summary>
    internal sealed class SourceCampaignMenus
    {
        internal enum Mode : byte { Flight = 0, Spells = 5, Map = 6, MapSpells = 8 }

        internal readonly struct Result
        {
            internal readonly Mode Before, After;
            internal readonly bool RequestAbandonConfirmation;
            private readonly bool bothButtonsHeld;
            internal bool Changed => Before != After;
            internal bool MapChanged => IsMap(Before) != IsMap(After);
            internal bool SpellsChanged => IsSpells(Before) != IsSpells(After);
            internal bool SuppressCasting => Changed || IsSpells(After) || bothButtonsHeld;
            internal Result(Mode before, Mode after, bool abandon, bool both)
            {
                Before = before;
                After = after;
                RequestAbandonConfirmation = abandon;
                bothButtonsHeld = both;
            }
        }

        private bool previousBoth;
        internal Mode Current { get; private set; }
        internal bool MapOpen => IsMap(Current);
        internal bool SpellOpen => IsSpells(Current);
        internal bool AdmitFlightInput => !SpellOpen;
        internal bool CaptureMouse => !SpellOpen;

        private static bool IsMap(Mode mode) => mode is Mode.Map or Mode.MapSpells;
        private static bool IsSpells(Mode mode) => mode is Mode.Spells or Mode.MapSpells;

        internal void Reset() { Current = Mode.Flight; previousBoth = false; }

        internal void CloseSpells()
        {
            if (SpellOpen) Current = MapOpen ? Mode.Map : Mode.Flight;
        }

        internal Result Step(bool enterPressed, bool spellHeld, bool escapePressed,
            bool leftHeld, bool rightHeld, bool alive)
        {
            var before = Current;
            var both = leftHeld && rightHeld;
            var bothPressed = both && !previousBoth;
            previousBoth = both;
            var abandon = false;
            if (!alive)
                Current = Mode.Flight;
            else if (SpellOpen)
            {
                // The source retains a drag after Ctrl is released until
                // both physical casting buttons are also released.
                if (!spellHeld && !leftHeld && !rightHeld) CloseSpells();
            }
            else if (MapOpen)
            {
                // The map branch handles close before its Ctrl submenu.
                if (enterPressed || bothPressed) Current = Mode.Flight;
                else if (escapePressed) abandon = true;
                else if (spellHeld) Current = Mode.MapSpells;
            }
            else
            {
                // Ctrl has precedence over both-button/Enter map input in
                // the normal flight branch, as the source bit 0x10 gate does.
                if (spellHeld) Current = Mode.Spells;
                else if (escapePressed) abandon = true;
                else if (enterPressed || bothPressed) Current = Mode.Map;
            }
            // MouseAndKeysEvents consumes both casting pulses for the map
            // gesture. Keeping the physical buttons held cannot start spells
            // on the following frame after that consumed edge.
            return new Result(before, Current, abandon, both);
        }
    }
}
