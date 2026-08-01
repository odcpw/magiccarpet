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
    /// Buffers host/XR input as presentation commands. The Rust bridge can
    /// translate a consumed frame into its existing command ABI later; this
    /// adapter has no simulation or owner-data dependency.
    /// </summary>
    [DisallowMultipleComponent]
    public sealed class UnityEnhancedInputAdapter : MonoBehaviour
    {
        private UnityEnhancedInputFrame pending;
        private bool hasPending;
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

        public void Clear()
        {
            hasPending = false;
        }

        public void ResetForSession()
        {
            pending = default(UnityEnhancedInputFrame);
            hasPending = false;
            lastPublishedSequence = 0;
        }
    }
}
