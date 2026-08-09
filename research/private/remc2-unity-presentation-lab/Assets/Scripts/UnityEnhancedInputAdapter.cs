using System;
using UnityEngine;

namespace MagicCarpet.PresentationLab
{
    [Serializable]
    public struct UnityEnhancedInputFrame
    {
        public ulong Sequence;
        public Vector2 Flight;
        public Vector2 Look;
        public bool Recenter;
        public bool Cast;
        public int SpellSelection;

        public UnityEnhancedInputFrame(ulong sequence, Vector2 flight, Vector2 look, bool recenter, bool cast, int spellSelection)
        {
            Sequence = sequence;
            Flight = flight;
            Look = look;
            Recenter = recenter;
            Cast = cast;
            SpellSelection = spellSelection;
        }
    }

    /// <summary>
    /// Buffers host/XR input as presentation commands. The Rust bridge consumes
    /// one frame per fixed simulation tick; the latest flight/look values are
    /// held between render updates while one-shot cast/selection edges clear
    /// after their first fixed-tick use.
    /// </summary>
    [DisallowMultipleComponent]
    public sealed class UnityEnhancedInputAdapter : MonoBehaviour
    {
        private UnityEnhancedInputFrame pending;
        private UnityEnhancedInputFrame held;
        private bool hasPending;
        private bool hasHeld;
        private ulong lastPublishedSequence;

        public ulong LastPublishedSequence { get { return lastPublishedSequence; } }
        public bool HasPendingFrame { get { return hasPending; } }

        public bool Publish(UnityEnhancedInputFrame frame)
        {
            if (frame.Sequence <= lastPublishedSequence)
            {
                return false;
            }

            pending = frame;
            hasPending = true;
            lastPublishedSequence = frame.Sequence;
            return true;
        }

        public bool TryConsume(out UnityEnhancedInputFrame frame)
        {
            if (!hasPending)
            {
                frame = default(UnityEnhancedInputFrame);
                return false;
            }

            frame = pending;
            hasPending = false;
            return true;
        }

        public bool TryConsumeForTick(out UnityEnhancedInputFrame frame)
        {
            if (hasPending)
            {
                held = pending;
                hasPending = false;
                hasHeld = true;
            }

            if (!hasHeld)
            {
                frame = default(UnityEnhancedInputFrame);
                return false;
            }

            frame = held;
            held.Cast = false;
            held.SpellSelection = -1;
            held.Recenter = false;
            return true;
        }

        public void Clear()
        {
            hasPending = false;
            hasHeld = false;
        }

        public void ResetForSession()
        {
            pending = default(UnityEnhancedInputFrame);
            held = default(UnityEnhancedInputFrame);
            hasPending = false;
            hasHeld = false;
            lastPublishedSequence = 0;
        }
    }
}
