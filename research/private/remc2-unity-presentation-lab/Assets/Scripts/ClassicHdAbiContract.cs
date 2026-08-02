using System;
using System.Runtime.InteropServices;

namespace MagicCarpet.PresentationLab
{
    /// <summary>
    /// Additive binding surface for sim-capi's size-queried Classic HD ABI.
    /// The lab does not ship the native library; the calls are optional and are
    /// never made until a private owner-local native plugin is supplied.
    /// </summary>
    public static class ClassicHdAbiContract
    {
        public const string NativeLibrary = "sim_capi";
        public const uint AbiVersion = 1;
        public const uint ProfileCompatibility320 = 0;
        public const uint Profile640 = 1;
        public const uint Profile960 = 2;
        public const uint Profile1280 = 3;
        public const uint ProfileCount = 4;
        public const uint FlagSyntheticFixture = 1u << 3;

        public static bool HasExpectedLayout(out string reason)
        {
            int bufferSize = Marshal.SizeOf(typeof(McClassicHdBufferInfoV1));
            int querySize = Marshal.SizeOf(typeof(McClassicHdFrameQueryV1));
            int frameSize = Marshal.SizeOf(typeof(McClassicHdFrameInfoV1));
            if (bufferSize != 104 || querySize != 64 || frameSize != 120)
            {
                reason = "Classic HD ABI layout mismatch: buffer=" + bufferSize +
                         ", query=" + querySize + ", frame=" + frameSize;
                return false;
            }

            reason = "Classic HD ABI v1 layout matches sim-capi";
            return true;
        }

        [DllImport(NativeLibrary, EntryPoint = "mc_presentation_abi_version", CallingConvention = CallingConvention.Cdecl)]
        public static extern uint PresentationAbiVersion();

        [DllImport(NativeLibrary, EntryPoint = "mc_classic_hd_profile_count", CallingConvention = CallingConvention.Cdecl)]
        public static extern uint ClassicHdProfileCount();

        [DllImport(NativeLibrary, EntryPoint = "mc_classic_hd_profile_info", CallingConvention = CallingConvention.Cdecl)]
        public static extern int ClassicHdProfileInfo(
            uint profileId,
            out McClassicHdBufferInfoV1 output,
            uint outputSize);

        [DllImport(NativeLibrary, EntryPoint = "mc_classic_hd_frame_required_bytes", CallingConvention = CallingConvention.Cdecl)]
        public static extern int ClassicHdFrameRequiredBytes(
            uint profileId,
            out ulong indexBytes,
            out ulong provenanceBytes,
            out ulong historyBytes);

        [DllImport(NativeLibrary, EntryPoint = "mc_classic_hd_frame_copy_fixture", CallingConvention = CallingConvention.Cdecl)]
        public static extern int ClassicHdFrameCopyFixture(
            ref McClassicHdFrameQueryV1 query,
            IntPtr indexBuffer,
            ulong indexCapacity,
            IntPtr provenanceBuffer,
            ulong provenanceCapacity,
            out McClassicHdFrameInfoV1 output,
            uint outputSize);
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct McClassicHdBufferInfoV1
    {
        public uint AbiVersion;
        public uint StructSize;
        public uint ProfileId;
        public uint Flags;
        public uint Width;
        public uint Height;
        public uint Stride;
        public uint ViewportX;
        public uint ViewportY;
        public uint ViewportWidth;
        public uint ViewportHeight;
        public ulong IndexBytes;
        public ulong ProvenanceBytes;
        public ulong HistoryBytes;
        public ulong ProfileRevision;
        public ulong MemoryBytes;
        public uint Reserved0;
        public uint Reserved1;
        public uint Reserved2;
        public uint Reserved3;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct McClassicHdFrameQueryV1
    {
        public uint AbiVersion;
        public uint StructSize;
        public uint ProfileId;
        public uint Reserved0;
        public ulong Generation;
        public ulong Tick;
        public ulong InputHash;
        public ulong ResetEpoch;
        public uint Reserved1;
        public uint Reserved2;
        public uint Reserved3;
        public uint Reserved4;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct McClassicHdFrameInfoV1
    {
        public uint AbiVersion;
        public uint StructSize;
        public uint ProfileId;
        public uint Flags;
        public ulong Generation;
        public ulong Tick;
        public ulong InputHash;
        public ulong ResetEpoch;
        public uint Width;
        public uint Height;
        public uint Stride;
        public ulong IndexBytes;
        public ulong ProvenanceBytes;
        public ulong IndexHash;
        public ulong ProvenanceHash;
        public ulong ReceiptHash;
        public uint Reserved0;
        public uint Reserved1;
        public uint Reserved2;
        public uint Reserved3;
    }
}
