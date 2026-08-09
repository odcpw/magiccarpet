namespace MagicCarpet.PresentationLab
{
    /// <summary>
    /// One ordinary player command from the owner-local PlaythroughRecorder
    /// journal.  Its requested-spell value is the recorder's additive field;
    /// this is intentionally distinct from treating the ten journal bytes as
    /// a raw generic Rust PlayerCommand memory image.
    /// The journal is external to this presentation lab; this value carries
    /// only the native input record across the presentation boundary.
    /// </summary>
    public readonly struct ClassicHdOwnerCommand
    {
        public readonly sbyte Opcode;
        public readonly sbyte Argument1;
        public readonly sbyte Argument2;
        public readonly sbyte Roll;
        public readonly sbyte Pitch;
        public readonly sbyte Flags;
        public readonly short RequestedSpell;

        public ClassicHdOwnerCommand(
            sbyte opcode,
            sbyte argument1,
            sbyte argument2,
            sbyte roll,
            sbyte pitch,
            sbyte flags,
            short requestedSpell)
        {
            Opcode = opcode;
            Argument1 = argument1;
            Argument2 = argument2;
            Roll = roll;
            Pitch = pitch;
            Flags = flags;
            RequestedSpell = requestedSpell;
        }
    }
}
