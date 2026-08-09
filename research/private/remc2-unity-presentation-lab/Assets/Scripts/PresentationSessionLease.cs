using System;

namespace MagicCarpet.PresentationLab
{
    /// <summary>
    /// A lifecycle seam for the future Rust snapshot bridge.
    /// Opening this lease does not load owner data or mutate gameplay state.
    /// </summary>
    public sealed class PresentationSessionLease : IDisposable
    {
        private PresentationSessionLease(PresentationModeId mode)
        {
            Mode = mode;
            IsOpen = true;
        }

        public PresentationModeId Mode { get; private set; }
        public bool IsOpen { get; private set; }

        public static PresentationSessionLease Open(PresentationModeId mode, OwnerInputValidation validation)
        {
            if (mode != PresentationModeId.ClassicHd && mode != PresentationModeId.UnityEnhanced)
            {
                throw new InvalidOperationException("A presentation session requires a supported presentation mode");
            }

            if (validation == null || !validation.IsValid)
            {
                throw new InvalidOperationException("A presentation session requires validated external owner inputs");
            }

            return new PresentationSessionLease(mode);
        }

        public void Dispose()
        {
            Close();
        }

        public void Close()
        {
            IsOpen = false;
        }
    }
}
