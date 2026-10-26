using System;

namespace MagicCarpet.Native
{
    // Presentation-only state for one source secret portal. Rust remains the
    // authority for discovery and completion; this object owns only the
    // source animation cursor and the one-shot opening-sound edge.
    internal sealed class SourceSecretPortalAnimation
    {
        internal readonly struct Presentation
        {
            internal bool Visible { get; }
            internal int Selector { get; }
            internal bool PlayOpeningSound { get; }

            private Presentation(bool visible, int selector, bool playOpeningSound)
            {
                Visible = visible;
                Selector = selector;
                PlayOpeningSound = playOpeningSound;
            }

            internal static Presentation Hidden => new Presentation(false, -1, false);
            internal static Presentation OpeningSound => new Presentation(false, -1, true);
            internal static Presentation Frame(int selector) => new Presentation(true, selector, false);
        }

        private enum Phase : byte
        {
            Hidden,
            OpeningSoundPending,
            Opening,
            Looping,
            Completed
        }

        private const uint SourceFrameStepTicks = 8;
        private const int OpeningFirst = 70;
        private const int OpeningLast = 83;
        private const int LoopFirst = 270;
        private const int LoopLast = 272;
        private const int CompletedFirst = 305;
        private const int CompletedLast = 311;

        private Phase phase;
        private bool initialized;
        private int selector;
        private uint lastTime;

        // now is the source 100 Hz presentation clock. Advance only once for
        // each source map pass: DrawFrameAnim advances at most one frame even
        // when the clock has been delayed by multiple frame intervals.
        internal Presentation Step(bool isSecret, bool discovered, bool completed,
            uint now, bool sourcePassAdvanced, bool sourceSelection)
        {
            if (!isSecret)
            {
                Reset();
                return Presentation.Hidden;
            }

            if (completed)
            {
                if (!initialized || phase != Phase.Completed)
                {
                    initialized = true;
                    phase = Phase.Completed;
                    selector = CompletedFirst;
                    lastTime = 0;
                }
                if (sourcePassAdvanced)
                    Advance(now, CompletedFirst, CompletedLast, out _);
                return Presentation.Frame(selector);
            }

            if (!discovered)
            {
                Reset();
                return Presentation.Hidden;
            }

            if (!initialized || phase == Phase.Hidden || phase == Phase.Completed)
            {
                initialized = true;
                phase = Phase.OpeningSoundPending;
                selector = OpeningFirst;
                lastTime = 0;
            }

            // The original only draws unfinished secret portals in source map
            // selection state (a4 == 3). It retains their cursor across other
            // map presentation phases without advancing or drawing them.
            if (!sourceSelection)
                return Presentation.Hidden;

            if (phase == Phase.OpeningSoundPending)
            {
                if (!sourcePassAdvanced)
                    return Presentation.Hidden;

                phase = Phase.Opening;
                return Presentation.OpeningSound;
            }

            if (phase == Phase.Opening)
            {
                var drawn = selector;
                var wrapped = sourcePassAdvanced &&
                    Advance(now, OpeningFirst, OpeningLast, out drawn);
                if (wrapped)
                {
                    // The source draws openingFirst on this wrap pass, then
                    // stores loopFirst for the next pass.
                    phase = Phase.Looping;
                    selector = LoopFirst;
                }
                return Presentation.Frame(drawn);
            }

            if (phase == Phase.Looping && sourcePassAdvanced)
                Advance(now, LoopFirst, LoopLast, out _);

            return phase == Phase.Looping
                ? Presentation.Frame(selector)
                : Presentation.Hidden;
        }

        internal void Reset()
        {
            phase = Phase.Hidden;
            initialized = false;
            selector = 0;
            lastTime = 0;
        }

        // DrawFrameAnim increments while selector <= end; selector end+1 is
        // therefore drawn for one full source interval before wrapping.
        private bool Advance(uint now, int first, int last, out int drawnSelector)
        {
            drawnSelector = selector;
            if (unchecked(now - lastTime) < SourceFrameStepTicks)
                return false;

            var wrapped = selector > last;
            selector = wrapped ? first : selector + 1;
            lastTime = now;
            drawnSelector = selector;
            return wrapped;
        }
    }
}
