using System;
using UnityEngine;

namespace MagicCarpet.PresentationLab
{
    [Serializable]
    public struct UnityEnhancedPresentationEvent
    {
        public ulong Sequence;
        public int Kind;
        public Vector3 Position;

        public UnityEnhancedPresentationEvent(ulong sequence, int kind, Vector3 position)
        {
            Sequence = sequence;
            Kind = kind;
            Position = position;
        }
    }

    /// <summary>
    /// HUD and audio sinks are deliberately passive. They accept presentation
    /// state/events and never become gameplay authorities.
    /// </summary>
    [DisallowMultipleComponent]
    public sealed class UnityEnhancedPresentationAdapters : MonoBehaviour
    {
        private ulong lastEventSequence;
        private ulong lastTick;
        private ulong currentGeneration;

        public ulong LastEventSequence { get { return lastEventSequence; } }
        public ulong LastTick { get { return lastTick; } }
        public ulong CurrentGeneration { get { return currentGeneration; } }

        public void ApplyTick(ulong generation, ulong tick)
        {
            if (generation != currentGeneration)
            {
                Reset(generation);
            }

            if (tick >= lastTick)
            {
                lastTick = tick;
            }
        }

        public bool AcceptEvent(UnityEnhancedPresentationEvent presentationEvent)
        {
            if (presentationEvent.Sequence <= lastEventSequence)
            {
                return false;
            }

            lastEventSequence = presentationEvent.Sequence;
            return true;
        }

        public void Reset(ulong generation)
        {
            currentGeneration = generation;
            lastTick = 0;
            lastEventSequence = 0;
        }
    }
}
