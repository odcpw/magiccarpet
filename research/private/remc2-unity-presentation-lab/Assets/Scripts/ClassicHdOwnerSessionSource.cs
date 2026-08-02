using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;

namespace MagicCarpet.PresentationLab
{
    /// <summary>
    /// Owner-bound Classic HD source adapter.
    ///
    /// Rust remains authoritative: this class opens the external owner
    /// package/closure/cache, advances the native Rust session, and consumes a
    /// profile-sized frame rerendered by the additive Classic HD Rust graph.
    /// It is intentionally not an asset loader or a replacement gameplay
    /// simulation.
    /// </summary>
    public sealed class ClassicHdOwnerSessionSource : IDisposable
    {
        private const string Library = "sim_capi";
        private const uint AbiVersion = 2;
        private const uint CommandSize = 32;
        private const uint SnapshotSize = 120;
        private const uint RenderCameraStateSize = 72;
        private const uint MissionSnapshotSize = 96;
        private const uint QuerySize = 64;
        private const uint InfoSize = 128;
        private const string AssetCacheEnvironment = "MC_ASSET_CACHE_PATH";
        private const int SelectedSpellFlag = 0x40;
        private const sbyte SpellSelectionOpcode = 40;
        private const uint RequiredClassicHdFlags =
            0x4000u | // provenance present
            0x10000u | // postprocess complete
            0x20000u | // lighting complete
            0x40000u; // profile-sized Rust rerender

        private byte[] indexBytes;
        private byte[] provenanceBytes;
        private byte[] snapshotBytes;
        private byte[] renderCameraBytes;
        private byte[] packageBytes;
        private byte[] closureBytes;
        private GCHandle sourceIndexHandle;
        private GCHandle sourceProvenanceHandle;
        private GCHandle snapshotHandle;
        private GCHandle renderCameraHandle;
        private bool sourceIndexPinned;
        private bool sourceProvenancePinned;
        private bool snapshotPinned;
        private bool renderCameraPinned;
        private ulong handle;
        private string priorAssetCacheEnvironment;
        private bool ownsAssetCacheEnvironment;
        private int outputWidth;
        private int outputHeight;
        private uint profileId;
        private ulong generation;
        private ulong tick;
        private ulong presentationRevision;
        private McPresentationFrameQueryV2 frameQuery;
        private McCommandV2 command;
        private McCameraInputV2 cameraInput;
        private McSessionSnapshotV2 snapshot;
        private McLevel1MissionSnapshotV2 mission;

        public bool IsConfigured { get; private set; }
        public ulong Generation { get { return generation; } }
        public ulong Tick { get { return tick; } }
        public ulong SourceGeneration { get { return generation; } }
        public ulong SourceTick { get { return tick; } }
        public ulong LastIndexHash { get; private set; }
        public ulong LastProvenanceHash { get; private set; }
        public ulong LastImmutableInputHash { get; private set; }
        public ulong LastReceiptHash { get; private set; }
        public uint LastFlags { get; private set; }
        public byte LastHistoryState { get; private set; }
        public byte EffectivePostprocessMode { get; private set; }
        public int PlayerLife { get { return mission.PlayerLife; } }
        public int PlayerMaxLife { get { return mission.PlayerMaxLife; } }
        public bool PlayerDead { get { return mission.PlayerDead != 0; } }
        public bool HasSpeedSpell { get { return mission.HasSpeedSpell != 0; } }
        public int MissionProjectileCount { get { return checked((int)mission.ProjectileCount); } }
        public int MissionImpactCount { get { return checked((int)mission.ImpactCount); } }
        public short TargetSpeed { get { return snapshot.TargetSpeed; } }
        public short ActualSpeed { get { return snapshot.ActualSpeed; } }
        public ushort CameraX { get { return ReadRenderCameraUInt16(28); } }
        public ushort CameraY { get { return ReadRenderCameraUInt16(30); } }

        public bool Configure(
            ClassicHdProfileInfo profile,
            ClassicHdPresentationSettings requestedSettings,
            string packagePath,
            string closurePath,
            string assetCachePath,
            out string reason)
        {
            reason = string.Empty;
            Dispose();

            if (profile.Width <= 0 || profile.Height <= 0 || profile.Stride < profile.Width)
            {
                reason = "Classic HD owner source received an invalid output profile";
                return false;
            }

            if (requestedSettings == null || !requestedSettings.reflections || !requestedSettings.dynamicLighting)
            {
                reason = "owner Classic HD currently requires reflections and dynamic lighting to remain enabled";
                return false;
            }

            if (string.IsNullOrWhiteSpace(packagePath) || !Directory.Exists(packagePath))
            {
                reason = "Classic HD owner package directory does not exist";
                return false;
            }

            if (string.IsNullOrWhiteSpace(closurePath) || !File.Exists(closurePath))
            {
                reason = "Classic HD owner closure file does not exist";
                return false;
            }

            if (string.IsNullOrWhiteSpace(assetCachePath) || !Directory.Exists(assetCachePath))
            {
                reason = "Classic HD owner asset-cache directory does not exist";
                return false;
            }

            try
            {
                outputWidth = profile.Width;
                outputHeight = profile.Height;
                profileId = (uint)profile.Id;
                indexBytes = new byte[checked(profile.Stride * profile.Height)];
                provenanceBytes = new byte[checked(profile.Stride * profile.Height)];
                snapshotBytes = new byte[SnapshotSize];
                renderCameraBytes = new byte[RenderCameraStateSize];
                packageBytes = Encoding.UTF8.GetBytes(packagePath);
                closureBytes = Encoding.UTF8.GetBytes(closurePath);
                sourceIndexHandle = GCHandle.Alloc(indexBytes, GCHandleType.Pinned);
                sourceIndexPinned = true;
                sourceProvenanceHandle = GCHandle.Alloc(provenanceBytes, GCHandleType.Pinned);
                sourceProvenancePinned = true;
                snapshotHandle = GCHandle.Alloc(snapshotBytes, GCHandleType.Pinned);
                snapshotPinned = true;
                renderCameraHandle = GCHandle.Alloc(renderCameraBytes, GCHandleType.Pinned);
                renderCameraPinned = true;

                priorAssetCacheEnvironment = Environment.GetEnvironmentVariable(AssetCacheEnvironment);
                Environment.SetEnvironmentVariable(AssetCacheEnvironment, assetCachePath);
                ownsAssetCacheEnvironment = true;

                int status = mc_native_session_open(
                    packageBytes,
                    checked((uint)packageBytes.Length),
                    closureBytes,
                    checked((uint)closureBytes.Length),
                    out handle);
                if (status != 0)
                {
                    reason = StatusReason("owner session open", status);
                    Dispose();
                    return false;
                }

                status = mc_native_session_reset(
                    handle,
                    snapshotHandle.AddrOfPinnedObject(),
                    SnapshotSize);
                if (status != 0)
                {
                    reason = StatusReason("owner session reset", status);
                    Dispose();
                    return false;
                }

                if (!ReadSessionSnapshot(out reason) ||
                    !ReadMissionSnapshot(out reason) ||
                    !ReadRenderCameraClock(out reason))
                {
                    Dispose();
                    return false;
                }
                presentationRevision = 1;
                command = new McCommandV2
                {
                    AbiVersion = AbiVersion,
                    StructSize = CommandSize,
                    Flags = 1,
                    RequestedSpell = -1
                };
                cameraInput = new McCameraInputV2
                {
                    AbiVersion = AbiVersion,
                    StructSize = 28
                };
                frameQuery = new McPresentationFrameQueryV2
                {
                    AbiVersion = AbiVersion,
                    StructSize = QuerySize,
                    PresentationRevision = presentationRevision,
                    ViewportWidth = checked((ushort)profile.Width),
                    ViewportHeight = checked((ushort)profile.Height),
                    ScreenStride = checked((ushort)profile.Stride),
                    ConfiguredBlurMode = BlurMode(requestedSettings.blur),
                    BlurCapability = 1,
                    HighSpeedSystem = 1,
                    ScratchPresent = 1,
                    VgaResolution = 1,
                    SpatialSmoothing = (byte)(string.Equals(requestedSettings.blur, "spatial-2x2", StringComparison.OrdinalIgnoreCase) ? 1 : 0),
                    MinimapEnabled = 1,
                    TopBarEnabled = 1,
                    ObjectiveSelector = byte.MaxValue
                };
                IsConfigured = true;
                return true;
            }
            catch (DllNotFoundException exception)
            {
                reason = "Classic HD owner native plugin is unavailable: " + exception.Message;
            }
            catch (EntryPointNotFoundException exception)
            {
                reason = "Classic HD owner native ABI is missing: " + exception.Message;
            }
            catch (BadImageFormatException exception)
            {
                reason = "Classic HD owner native plugin has the wrong architecture: " + exception.Message;
            }
            catch (Exception exception)
            {
                reason = "Classic HD owner source setup failed: " + exception.Message;
            }

            Dispose();
            return false;
        }

        public bool Reset(out string reason)
        {
            reason = string.Empty;
            if (!IsConfigured || handle == 0 || snapshotHandle.IsAllocated == false)
            {
                reason = "Classic HD owner source is not configured";
                return false;
            }

            int status = mc_native_session_reset(
                handle,
                snapshotHandle.AddrOfPinnedObject(),
                SnapshotSize);
            if (status != 0)
            {
                reason = StatusReason("owner session reset", status);
                return false;
            }

            return ReadSessionSnapshot(out reason) &&
                   ReadMissionSnapshot(out reason) &&
                   ReadRenderCameraClock(out reason);
        }

        public bool Fill(
            byte[] destination,
            float horizontalInput,
            float verticalInput,
            out string reason)
        {
            return Fill(destination, horizontalInput, verticalInput, false, -1, out reason);
        }

        public bool Fill(
            byte[] destination,
            float horizontalInput,
            float verticalInput,
            bool cast,
            int spellSelection,
            out string reason)
        {
            reason = string.Empty;
            if (!IsConfigured || indexBytes == null || provenanceBytes == null)
            {
                reason = "Classic HD owner source is not configured";
                return false;
            }

            if (destination == null || destination.Length < outputWidth * outputHeight)
            {
                reason = "Classic HD owner destination is too small";
                return false;
            }

            command.Opcode = 0;
            command.Argument1 = 0;
            command.Roll = ClampInt8(horizontalInput * 127f);
            command.Pitch = ClampInt8(verticalInput * 127f);
            command.Flags = 1;
            command.RequestedSpell = -1;
            if (spellSelection >= 0)
            {
                command.Opcode = SpellSelectionOpcode;
                command.Argument1 = ClampInt8(spellSelection);
            }
            if (cast)
            {
                command.Flags = unchecked((sbyte)(command.Flags | SelectedSpellFlag));
            }
            int status = mc_native_session_step_with_camera(
                handle,
                ref command,
                ref cameraInput,
                snapshotHandle.AddrOfPinnedObject(),
                SnapshotSize);
            if (status != 0)
            {
                reason = StatusReason("owner session step", status);
                return false;
            }

            if (!ReadSessionSnapshot(out reason) ||
                !ReadMissionSnapshot(out reason) ||
                !ReadRenderCameraClock(out reason))
            {
                return false;
            }
            frameQuery.Generation = generation;
            frameQuery.Tick = tick;
            status = mc_native_session_classic_hd_frame_copy(
                handle,
                profileId,
                ref frameQuery,
                sourceIndexHandle.AddrOfPinnedObject(),
                checked((uint)indexBytes.Length),
                sourceProvenanceHandle.AddrOfPinnedObject(),
                checked((uint)provenanceBytes.Length),
                out McPresentationFrameInfoV2 info,
                InfoSize);
            if (status != 0)
            {
                reason = StatusReason("owner Classic HD profile rerender copy", status);
                return false;
            }

            if (info.AbiVersion != AbiVersion ||
                info.StructSize != InfoSize ||
                info.Generation != generation ||
                info.Tick != tick ||
                info.PresentationRevision != presentationRevision ||
                info.Width != outputWidth ||
                info.Height != outputHeight ||
                info.Stride != outputWidth ||
                info.ViewportWidth != outputWidth ||
                info.IndexBytes != indexBytes.Length ||
                info.ProvenanceBytes != provenanceBytes.Length ||
                (info.Flags & RequiredClassicHdFlags) != RequiredClassicHdFlags)
            {
                reason = "owner Classic HD profile rerender returned an inconsistent receipt";
                return false;
            }

            Buffer.BlockCopy(indexBytes, 0, destination, 0, indexBytes.Length);
            LastIndexHash = info.IndexHash;
            LastProvenanceHash = info.ProvenanceHash;
            LastImmutableInputHash = info.ImmutableInputHash;
            LastFlags = info.Flags;
            LastHistoryState = info.HistoryState;
            LastReceiptHash = ReceiptHash(info);
            EffectivePostprocessMode = info.EffectivePostprocessMode;
            return true;
        }

        /// <summary>
        /// Injects the existing private owner qualification damage boundary.
        /// Rust consumes the damage on the next step; Classic HD only observes
        /// the resulting mission state and never decides whether the player died.
        /// </summary>
        public bool InjectPlayerDamageForQualification(
            int damage,
            ushort attackerEntityIndex,
            out string reason)
        {
            reason = string.Empty;
            if (!IsConfigured || handle == 0)
            {
                reason = "Classic HD owner source is not configured";
                return false;
            }

            if (damage <= 0 || attackerEntityIndex == 0)
            {
                reason = "Classic HD qualification damage requires a positive amount and nonzero attacker";
                return false;
            }

            try
            {
                int status = mc_native_level1_player_inject_damage_for_qualification(
                    handle,
                    damage,
                    attackerEntityIndex);
                if (status != 0)
                {
                    reason = StatusReason("Classic HD player damage qualification", status);
                    return false;
                }

                return true;
            }
            catch (Exception exception)
            {
                reason = "Classic HD player damage qualification failed: " + exception.Message;
                return false;
            }
        }

        public void Dispose()
        {
            IsConfigured = false;
            if (handle != 0)
            {
                try
                {
                    mc_native_session_destroy(handle);
                }
                catch (Exception)
                {
                    // A failed teardown must not prevent managed buffers and
                    // the process-local cache binding from being released.
                }

                handle = 0;
            }

            if (snapshotPinned)
            {
                snapshotHandle.Free();
                snapshotPinned = false;
            }

            if (renderCameraPinned)
            {
                renderCameraHandle.Free();
                renderCameraPinned = false;
            }

            if (sourceProvenancePinned)
            {
                sourceProvenanceHandle.Free();
                sourceProvenancePinned = false;
            }

            if (sourceIndexPinned)
            {
                sourceIndexHandle.Free();
                sourceIndexPinned = false;
            }

            if (ownsAssetCacheEnvironment)
            {
                Environment.SetEnvironmentVariable(AssetCacheEnvironment, priorAssetCacheEnvironment);
                ownsAssetCacheEnvironment = false;
                priorAssetCacheEnvironment = null;
            }

            indexBytes = null;
            provenanceBytes = null;
            snapshotBytes = null;
            renderCameraBytes = null;
            packageBytes = null;
            closureBytes = null;
            generation = 0;
            tick = 0;
            profileId = 0;
            presentationRevision = 0;
            LastIndexHash = 0;
            LastProvenanceHash = 0;
            LastImmutableInputHash = 0;
            LastReceiptHash = 0;
            LastFlags = 0;
            LastHistoryState = 0;
            EffectivePostprocessMode = 0;
        }

        private bool ReadSessionSnapshot(out string reason)
        {
            reason = string.Empty;
            if (!snapshotHandle.IsAllocated)
            {
                reason = "Classic HD owner session snapshot buffer is not pinned";
                return false;
            }

            snapshot = Marshal.PtrToStructure<McSessionSnapshotV2>(snapshotHandle.AddrOfPinnedObject());
            if (snapshot.AbiVersion != AbiVersion || snapshot.StructSize != SnapshotSize)
            {
                reason = "Classic HD owner session snapshot failed its ABI/layout contract";
                return false;
            }

            return true;
        }

        private bool ReadMissionSnapshot(out string reason)
        {
            reason = string.Empty;
            int status = mc_native_level1_mission_snapshot(
                handle,
                out mission,
                MissionSnapshotSize);
            if (status != 0)
            {
                reason = StatusReason("Classic HD owner Level 1 mission snapshot", status);
                return false;
            }

            if (mission.AbiVersion != AbiVersion ||
                mission.StructSize != MissionSnapshotSize ||
                mission.Generation != snapshot.Generation ||
                mission.Tick != snapshot.Tick)
            {
                reason = "Classic HD owner mission snapshot failed its generation/layout contract";
                return false;
            }

            return true;
        }

        private bool ReadRenderCameraClock(out string reason)
        {
            reason = string.Empty;
            int status = mc_native_render_camera_state(
                handle,
                renderCameraHandle.AddrOfPinnedObject(),
                RenderCameraStateSize);
            if (status != 0)
            {
                reason = StatusReason("owner render-camera state", status);
                return false;
            }

            IntPtr address = renderCameraHandle.AddrOfPinnedObject();
            generation = unchecked((ulong)Marshal.ReadInt64(address, 8));
            tick = unchecked((ulong)Marshal.ReadInt64(address, 16));
            return true;
        }

        private ushort ReadRenderCameraUInt16(int offset)
        {
            return unchecked((ushort)Marshal.ReadInt16(renderCameraHandle.AddrOfPinnedObject(), offset));
        }

        private static byte BlurMode(string value)
        {
            if (string.Equals(value, "spatial-2x2", StringComparison.OrdinalIgnoreCase))
            {
                return 1;
            }

            if (string.Equals(value, "tablesn", StringComparison.OrdinalIgnoreCase))
            {
                return 2;
            }

            return 0;
        }

        private static sbyte ClampInt8(float value)
        {
            int rounded = (int)Math.Round(value);
            if (rounded < sbyte.MinValue)
            {
                return sbyte.MinValue;
            }

            if (rounded > sbyte.MaxValue)
            {
                return sbyte.MaxValue;
            }

            return (sbyte)rounded;
        }

        private static ulong ReceiptHash(McPresentationFrameInfoV2 info)
        {
            unchecked
            {
                ulong hash = 0xcbf29ce484222325UL;
                hash = Mix(hash, info.Generation);
                hash = Mix(hash, info.Tick);
                hash = Mix(hash, info.PresentationRevision);
                hash = Mix(hash, info.ImmutableInputHash);
                hash = Mix(hash, info.IndexHash);
                hash = Mix(hash, info.ProvenanceHash);
                hash = Mix(hash, info.Flags);
                return hash;
            }
        }

        private static ulong Mix(ulong hash, ulong value)
        {
            unchecked
            {
                hash ^= value;
                return hash * 0x100000001b3UL;
            }
        }

        private static string StatusReason(string operation, int status)
        {
            string nativeMessage = string.Empty;
            try
            {
                uint length;
                if (mc_native_last_error_length(out length) == 0 && length > 0)
                {
                    byte[] buffer = new byte[checked((int)length + 1)];
                    uint written;
                    if (mc_native_last_error_copy(buffer, (uint)buffer.Length, out written) == 0)
                    {
                        nativeMessage = Encoding.UTF8.GetString(buffer, 0, checked((int)written));
                    }
                }
            }
            catch (Exception)
            {
                nativeMessage = string.Empty;
            }

            return string.IsNullOrEmpty(nativeMessage)
                ? operation + " returned status " + status
                : operation + " returned status " + status + ": " + nativeMessage;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct McCommandV2
        {
            public uint AbiVersion;
            public uint StructSize;
            public sbyte Opcode;
            public sbyte Argument1;
            public sbyte Argument2;
            public sbyte Roll;
            public sbyte Pitch;
            public sbyte Flags;
            public short RequestedSpell;
            public uint Reserved0;
            public uint Reserved1;
            public uint Reserved2;
            public uint Reserved3;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct McCameraInputV2
        {
            public uint AbiVersion;
            public uint StructSize;
            public ushort LookYaw;
            public ushort LookPitch;
            public uint Reserved0;
            public uint Reserved1;
            public uint Reserved2;
            public uint Reserved3;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct McSessionSnapshotV2
        {
            public uint AbiVersion;
            public uint StructSize;
            public ulong Generation;
            public ulong Tick;
            public ulong EventHead;
            public ulong StateHash;
            public ulong ConformanceHash;
            public ushort EntityIndex;
            public ushort X;
            public ushort Y;
            public short Z;
            public short Yaw;
            public short DisplayPitch;
            public short EffectivePitch;
            public short RollAccumulator;
            public short TargetSpeed;
            public short ActualSpeed;
            public short StrafeSpeed;
            public byte Phase;
            public byte LastResolution;
            public byte Profile;
            public byte LocalPlayerSlot;
            public byte LeftSpellSlot;
            public byte RightSpellSlot;
            public byte SelectedSpellUiIndex;
            public byte ObjectiveTextboxTicks;
            public byte ObjectiveCount;
            public byte ObjectivesAuthority;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 2)]
            public byte[] Reserved1;
            public ushort LeftSpellHolder;
            public ushort RightSpellHolder;
            public ushort SelectedSpellHolder;
            public ushort Reserved0;
            public int CasterMana;
            public int MaptypeMusic;
            public uint FireSequence;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            public uint[] Reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct McLevel1MissionSnapshotV2
        {
            public uint AbiVersion;
            public uint StructSize;
            public ulong Generation;
            public ulong Tick;
            public byte Authority;
            public byte CurrentStage;
            public byte TextSelector;
            public byte ObjectiveTextboxTicks;
            public byte TasksCompleted;
            public byte PlayerDead;
            public byte HasFireball;
            public byte HasPossession;
            public byte HasCastleSpell;
            public byte HasSpeedSpell;
            public byte CastleExists;
            public byte CastleLevel;
            public byte FidelityGapFlags;
            public byte ExitSequenceActive;
            public byte ExitSequenceComplete;
            public byte ExitResultCode;
            public int PlayerLife;
            public int PlayerMaxLife;
            public int PlayerMana;
            public int PlayerMaxMana;
            public int CastleMana;
            public int SecuredManaPercent;
            public uint ActorCount;
            public uint ProjectileCount;
            public uint ImpactCount;
            public uint ExitSequencePhase;
            public int ExitSequenceTimer;
            public int PossessionExperience;
            public ushort LastPossessedBuildingEntity;
            public ushort LastPossessedBuildingOwner;
            public ushort PossessedBuildingCount;
            public byte PossessionTier;
            public byte SpeedTrailCount;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct McPresentationFrameQueryV2
        {
            public uint AbiVersion;
            public uint StructSize;
            public ulong Generation;
            public ulong Tick;
            public ulong PresentationRevision;
            public ushort ViewportWidth;
            public ushort ViewportHeight;
            public ushort ScreenStride;
            public ushort Reserved0;
            public byte ConfiguredBlurMode;
            public byte BlurCapability;
            public byte HighSpeedSystem;
            public byte ScratchPresent;
            public byte VgaResolution;
            public byte SpatialSmoothing;
            public byte MinimapEnabled;
            public byte TopBarEnabled;
            public byte MenuOverlay;
            public byte GamePaused;
            public byte LevelEnd;
            public byte ObjectiveSelector;
            public byte ObjectiveTextbox;
            public byte TutorialOrBriefing;
            public byte HelpPopup;
            public byte DebugText;
            public uint Reserved1;
            public uint Reserved2;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct McPresentationFrameInfoV2
        {
            public uint AbiVersion;
            public uint StructSize;
            public ulong Generation;
            public ulong Tick;
            public ulong PresentationRevision;
            public ulong ImmutableInputHash;
            public ushort Width;
            public ushort Height;
            public ushort Stride;
            public ushort ViewportWidth;
            public uint IndexBytes;
            public uint ProvenanceBytes;
            public uint Flags;
            public byte EffectivePostprocessMode;
            public byte HistoryState;
            public byte NewlyCommitted;
            public byte Reserved0;
            public ulong IndexHash;
            public ulong ProvenanceHash;
            public ulong LightingHash;
            public ulong SourceArenaHash;
            public uint OverlayPixelsWritten;
            public ushort OverlayStageCount;
            public ushort Reserved1;
            public uint Reserved2;
            public uint Reserved3;
            public uint Reserved4;
            public uint Reserved5;
            public uint Reserved6;
            public uint Reserved7;
        }

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_session_open(
            byte[] packageDir,
            uint packageDirLength,
            byte[] closurePath,
            uint closurePathLength,
            out ulong handle);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_session_destroy(ulong handle);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_session_reset(
            ulong handle,
            IntPtr snapshot,
            uint snapshotSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_render_camera_state(
            ulong handle,
            IntPtr state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_session_step_with_camera(
            ulong handle,
            ref McCommandV2 command,
            ref McCameraInputV2 cameraInput,
            IntPtr snapshot,
            uint snapshotSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_session_classic_hd_frame_copy(
            ulong handle,
            uint profileId,
            ref McPresentationFrameQueryV2 query,
            IntPtr indexBuffer,
            uint indexCapacity,
            IntPtr provenanceBuffer,
            uint provenanceCapacity,
            out McPresentationFrameInfoV2 info,
            uint infoSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_level1_mission_snapshot(
            ulong handle,
            out McLevel1MissionSnapshotV2 snapshot,
            uint snapshotSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_level1_player_inject_damage_for_qualification(
            ulong handle,
            int damage,
            ushort attackerEntityIndex);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_last_error_length(out uint length);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        private static extern int mc_native_last_error_copy(
            byte[] buffer,
            uint capacity,
            out uint length);
    }
}
