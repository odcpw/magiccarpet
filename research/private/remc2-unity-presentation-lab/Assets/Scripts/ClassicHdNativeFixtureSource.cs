using System;
using System.Runtime.InteropServices;

namespace MagicCarpet.PresentationLab
{
    /// <summary>
    /// Calls the additive Classic HD ABI with buffers owned by the presenter.
    /// The ABI currently exposes a deterministic fixture, so this class is
    /// deliberately named and reported as a fixture rather than an owner
    /// renderer. It is only enabled by the explicit native presentation path.
    /// </summary>
    internal sealed class ClassicHdNativeFixtureSource : IDisposable
    {
        private byte[] indexBytes;
        private byte[] provenanceBytes;
        private GCHandle indexHandle;
        private GCHandle provenanceHandle;
        private bool indexPinned;
        private bool provenancePinned;
        private uint profileId;
        private uint width;
        private uint height;

        public bool IsConfigured { get; private set; }
        public ulong LastIndexHash { get; private set; }
        public ulong LastProvenanceHash { get; private set; }
        public ulong LastReceiptHash { get; private set; }
        public uint LastFlags { get; private set; }

        public bool Configure(ClassicHdProfileInfo profile, out string reason)
        {
            reason = string.Empty;
            Dispose();

            if (!ClassicHdAbiContract.HasExpectedLayout(out reason))
            {
                return false;
            }

            try
            {
                if (ClassicHdAbiContract.PresentationAbiVersion() != ClassicHdAbiContract.AbiVersion)
                {
                    reason = "Classic HD native presentation ABI version mismatch";
                    return false;
                }

                if (ClassicHdAbiContract.ClassicHdProfileCount() != ClassicHdAbiContract.ProfileCount)
                {
                    reason = "Classic HD native profile count mismatch";
                    return false;
                }

                profileId = ProfileId(profile.Id);
                McClassicHdBufferInfoV1 nativeProfile = default(McClassicHdBufferInfoV1);
                int status = ClassicHdAbiContract.ClassicHdProfileInfo(
                    profileId,
                    out nativeProfile,
                    (uint)Marshal.SizeOf(typeof(McClassicHdBufferInfoV1)));
                if (status != 0)
                {
                    reason = "Classic HD native profile query failed with status " + status;
                    return false;
                }

                if (nativeProfile.Width != profile.Width || nativeProfile.Height != profile.Height ||
                    nativeProfile.Stride != profile.Stride ||
                    nativeProfile.IndexBytes != (ulong)(profile.Stride * profile.Height))
                {
                    reason = "Classic HD native profile dimensions do not match the managed profile";
                    return false;
                }

                int length = checked(profile.Stride * profile.Height);
                indexBytes = new byte[length];
                provenanceBytes = new byte[length];
                indexHandle = GCHandle.Alloc(indexBytes, GCHandleType.Pinned);
                indexPinned = true;
                provenanceHandle = GCHandle.Alloc(provenanceBytes, GCHandleType.Pinned);
                provenancePinned = true;
                width = (uint)profile.Width;
                height = (uint)profile.Height;
                IsConfigured = true;
                return true;
            }
            catch (DllNotFoundException exception)
            {
                reason = "Classic HD native plugin is unavailable: " + exception.Message;
                Dispose();
                return false;
            }
            catch (EntryPointNotFoundException exception)
            {
                reason = "Classic HD native plugin is missing the additive ABI: " + exception.Message;
                Dispose();
                return false;
            }
            catch (BadImageFormatException exception)
            {
                reason = "Classic HD native plugin has the wrong architecture: " + exception.Message;
                Dispose();
                return false;
            }
            catch (Exception exception)
            {
                reason = "Classic HD native fixture setup failed: " + exception.Message;
                Dispose();
                return false;
            }
        }

        public bool Fill(
            byte[] destination,
            ulong generation,
            ulong tick,
            ulong resetEpoch,
            ulong inputHash,
            out string reason)
        {
            reason = string.Empty;
            if (!IsConfigured || indexBytes == null || provenanceBytes == null)
            {
                reason = "Classic HD native fixture is not configured";
                return false;
            }

            if (destination == null || destination.Length < indexBytes.Length)
            {
                reason = "Classic HD native fixture destination is too small";
                return false;
            }

            McClassicHdFrameQueryV1 query = new McClassicHdFrameQueryV1
            {
                AbiVersion = ClassicHdAbiContract.AbiVersion,
                StructSize = (uint)Marshal.SizeOf(typeof(McClassicHdFrameQueryV1)),
                ProfileId = profileId,
                Generation = generation,
                Tick = tick,
                InputHash = inputHash,
                ResetEpoch = resetEpoch
            };
            McClassicHdFrameInfoV1 info = default(McClassicHdFrameInfoV1);
            int status;
            try
            {
                status = ClassicHdAbiContract.ClassicHdFrameCopyFixture(
                    ref query,
                    indexHandle.AddrOfPinnedObject(),
                    (ulong)indexBytes.Length,
                    provenanceHandle.AddrOfPinnedObject(),
                    (ulong)provenanceBytes.Length,
                    out info,
                    (uint)Marshal.SizeOf(typeof(McClassicHdFrameInfoV1)));
            }
            catch (Exception exception)
            {
                reason = "Classic HD native fixture call failed: " + exception.Message;
                return false;
            }

            if (status != 0)
            {
                reason = "Classic HD native fixture returned status " + status;
                return false;
            }

            if (info.Generation != generation || info.Tick != tick || info.ProfileId != profileId ||
                info.Width != width || info.Height != height || info.Stride != width ||
                info.IndexBytes != (ulong)indexBytes.Length ||
                info.ProvenanceBytes != (ulong)provenanceBytes.Length)
            {
                reason = "Classic HD native fixture returned an inconsistent frame receipt";
                return false;
            }

            Buffer.BlockCopy(indexBytes, 0, destination, 0, indexBytes.Length);
            LastIndexHash = info.IndexHash;
            LastProvenanceHash = info.ProvenanceHash;
            LastReceiptHash = info.ReceiptHash;
            LastFlags = info.Flags;
            return true;
        }

        public void Dispose()
        {
            IsConfigured = false;
            if (provenancePinned)
            {
                provenanceHandle.Free();
                provenancePinned = false;
            }

            if (indexPinned)
            {
                indexHandle.Free();
                indexPinned = false;
            }

            indexBytes = null;
            provenanceBytes = null;
            LastIndexHash = 0;
            LastProvenanceHash = 0;
            LastReceiptHash = 0;
            LastFlags = 0;
        }

        private static uint ProfileId(ClassicHdProfileId id)
        {
            switch (id)
            {
                case ClassicHdProfileId.Compatibility320x200:
                    return ClassicHdAbiContract.ProfileCompatibility320;
                case ClassicHdProfileId.Hd640x480:
                    return ClassicHdAbiContract.Profile640;
                case ClassicHdProfileId.Hd960x720:
                    return ClassicHdAbiContract.Profile960;
                case ClassicHdProfileId.Hd1280x960:
                    return ClassicHdAbiContract.Profile1280;
                default:
                    throw new ArgumentOutOfRangeException(nameof(id), id, "unknown Classic HD profile");
            }
        }
    }
}
