using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Security.Cryptography;
using System.Text;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native
{
    public sealed class MagicCarpetRuntime : MonoBehaviour
    {
        internal readonly struct SpellMenuCommand
        {
            internal SpellMenuCommand(sbyte opcode, sbyte argument1, sbyte argument2)
            {
                Opcode = opcode;
                Argument1 = argument1;
                Argument2 = argument2;
            }

            internal sbyte Opcode { get; }
            internal sbyte Argument1 { get; }
            internal sbyte Argument2 { get; }
        }

        internal sealed class SpellMenuCommandBuffer
        {
            private readonly Queue<SpellMenuCommand> commands = new();
            private SpellMenuCommand pendingHeld;
            private bool hasPendingHeld;

            internal int Count => commands.Count + (hasPendingHeld ? 1 : 0);

            internal void Enqueue(SpellMenuCommand command)
            {
                if (command.Opcode == 41)
                {
                    pendingHeld = command;
                    hasPendingHeld = true;
                    return;
                }
                if (command.Opcode is 31 or 32)
                {
                    FlushHeld();
                }
                commands.Enqueue(command);
            }

            internal bool TryDequeue(out SpellMenuCommand command)
            {
                if (commands.Count != 0)
                {
                    command = commands.Dequeue();
                    return true;
                }
                if (hasPendingHeld)
                {
                    command = pendingHeld;
                    hasPendingHeld = false;
                    return true;
                }
                command = default;
                return false;
            }

            internal void Clear()
            {
                commands.Clear();
                pendingHeld = default;
                hasPendingHeld = false;
            }

            private void FlushHeld()
            {
                if (!hasPendingHeld)
                {
                    return;
                }
                commands.Enqueue(pendingHeld);
                hasPendingHeld = false;
            }
        }

        internal readonly struct SpellMenuGeometry
        {
            internal SpellMenuGeometry(
                int rows,
                int columns,
                int referenceY,
                int edgeWidth,
                int panelWidth,
                int panelHeight,
                int tierCount,
                int tierPanelWidth)
            {
                Rows = rows;
                Columns = columns;
                ReferenceY = referenceY;
                EdgeWidth = edgeWidth;
                PanelWidth = panelWidth;
                PanelHeight = panelHeight;
                TierCount = tierCount;
                TierPanelWidth = tierPanelWidth;
            }

            internal int Rows { get; }
            internal int Columns { get; }
            internal int ReferenceY { get; }
            internal int EdgeWidth { get; }
            internal int PanelWidth { get; }
            internal int PanelHeight { get; }
            internal int TierCount { get; }
            internal int TierPanelWidth { get; }
        }

        internal sealed class SpellMenuController
        {
            internal int MenuState { get; private set; }
            internal int HoverUiIndex { get; private set; } = -1;
            internal int SelectedUiIndex { get; private set; } = -1;
            internal int HoverTier { get; private set; } = -1;
            internal int Stage { get; private set; }

            private int pressedMouseButton = -1;
            private float virtualLogicalX;
            private float priorPhysicalLogicalX;

            internal void Open(
                float logicalX,
                float logicalY,
                NativeSim.Level1SpellMenuSnapshot snapshot,
                SpellMenuGeometry geometry)
            {
                MenuState = 5;
                ClearChoice();
                UpdateFamilyHover(logicalX, logicalY, snapshot, geometry);
            }

            internal void Close()
            {
                MenuState = 0;
                HoverUiIndex = -1;
                ClearChoice();
            }

            internal void UpdateFamilyHover(
                float logicalX,
                float logicalY,
                NativeSim.Level1SpellMenuSnapshot snapshot,
                SpellMenuGeometry geometry)
            {
                if (MenuState != 5 || SelectedUiIndex >= 0)
                {
                    return;
                }
                var uiIndex = FamilyFromLogical(logicalX, logicalY, geometry);
                HoverUiIndex = FamilyEnabled(snapshot, uiIndex) ? uiIndex : -1;
                HoverTier = -1;
            }

            internal bool Press(
                int mouseButton,
                float logicalX,
                float logicalY,
                NativeSim.Level1SpellMenuSnapshot snapshot,
                SpellMenuGeometry geometry,
                out SpellMenuCommand command)
            {
                command = default;
                UpdateFamilyHover(logicalX, logicalY, snapshot, geometry);
                if (
                    MenuState != 5 ||
                    SelectedUiIndex >= 0 ||
                    HoverUiIndex < 0 ||
                    mouseButton is < 0 or > 1)
                {
                    return false;
                }

                SelectedUiIndex = HoverUiIndex;
                Stage = mouseButton + 1;
                pressedMouseButton = mouseButton;
                HoverTier = snapshot.familySelectedTier[SelectedUiIndex];
                var startX = SpellTierMenuStartX(
                    SelectedUiIndex,
                    geometry.Columns,
                    geometry.EdgeWidth,
                    geometry.PanelWidth,
                    geometry.TierCount,
                    geometry.TierPanelWidth);
                virtualLogicalX =
                    startX +
                    HoverTier * geometry.TierPanelWidth +
                    geometry.TierPanelWidth / 2f;
                priorPhysicalLogicalX = logicalX;
                command = new SpellMenuCommand(
                    40,
                    checked((sbyte)SelectedUiIndex),
                    checked((sbyte)Stage));
                return true;
            }

            internal bool Hold(
                int mouseButton,
                float logicalX,
                NativeSim.Level1SpellMenuSnapshot snapshot,
                SpellMenuGeometry geometry,
                out SpellMenuCommand command)
            {
                command = default;
                if (
                    MenuState != 5 ||
                    SelectedUiIndex < 0 ||
                    mouseButton != pressedMouseButton)
                {
                    return false;
                }
                virtualLogicalX += logicalX - priorPhysicalLogicalX;
                priorPhysicalLogicalX = logicalX;
                var startX = SpellTierMenuStartX(
                    SelectedUiIndex,
                    geometry.Columns,
                    geometry.EdgeWidth,
                    geometry.PanelWidth,
                    geometry.TierCount,
                    geometry.TierPanelWidth);
                HoverTier = SpellTierFromLogicalX(
                    virtualLogicalX,
                    startX,
                    geometry.TierPanelWidth,
                    UnlockedTier(snapshot, SelectedUiIndex, geometry));
                command = new SpellMenuCommand(
                    41,
                    checked((sbyte)HoverTier),
                    0);
                return true;
            }

            internal bool ConfigureEligibility(
                int mouseButton,
                float logicalX,
                float logicalY,
                NativeSim.Level1SpellMenuSnapshot snapshot,
                SpellMenuGeometry geometry,
                out SpellMenuCommand command)
            {
                command = default;
                UpdateFamilyHover(logicalX, logicalY, snapshot, geometry);
                if (
                    MenuState != 5 ||
                    SelectedUiIndex >= 0 ||
                    HoverUiIndex < 0 ||
                    mouseButton is < 0 or > 1)
                {
                    return false;
                }
                var eligibility = mouseButton + 1;
                var nextEligibility =
                    snapshot.familyHandEligibility[HoverUiIndex] == eligibility
                        ? 0
                        : eligibility;
                command = new SpellMenuCommand(
                    38,
                    checked((sbyte)HoverUiIndex),
                    checked((sbyte)nextEligibility));
                return true;
            }

            internal static bool Cycle(
                bool rightHand,
                bool backward,
                NativeSim.Level1SpellMenuSnapshot snapshot,
                out SpellMenuCommand command)
            {
                command = default;
                var candidate = (int)(rightHand
                    ? snapshot.rightFamily
                    : snapshot.leftFamily);
                var direction = backward ? -1 : 1;
                var requiredEligibility = rightHand ? 2 : 1;
                for (var misses = 0; misses < 26; misses++)
                {
                    candidate = (candidate + direction + 26) % 26;
                    if (
                        snapshot.familyEnabled != null &&
                        snapshot.familyHandEligibility != null &&
                        snapshot.familySelectedTier != null &&
                        candidate < snapshot.familyEnabled.Length &&
                        candidate < snapshot.familyHandEligibility.Length &&
                        candidate < snapshot.familySelectedTier.Length &&
                        snapshot.familyEnabled[candidate] != 0 &&
                        snapshot.familyHandEligibility[candidate] ==
                            requiredEligibility)
                    {
                        command = new SpellMenuCommand(
                            checked((sbyte)(rightHand ? 32 : 31)),
                            checked((sbyte)candidate),
                            snapshot.familySelectedTier[candidate]);
                        return true;
                    }
                }
                return false;
            }

            internal bool Release(int mouseButton, out SpellMenuCommand command)
            {
                command = default;
                if (
                    MenuState != 5 ||
                    SelectedUiIndex < 0 ||
                    mouseButton != pressedMouseButton)
                {
                    return false;
                }
                command = new SpellMenuCommand(
                    checked((sbyte)(mouseButton == 0 ? 31 : 32)),
                    checked((sbyte)SelectedUiIndex),
                    checked((sbyte)HoverTier));
                ClearChoice();
                return true;
            }

            internal static int FamilyFromLogical(
                float logicalX,
                float logicalY,
                SpellMenuGeometry geometry)
            {
                var column = Mathf.Clamp(
                    Mathf.FloorToInt(
                        (logicalX - geometry.EdgeWidth) / geometry.PanelWidth),
                    0,
                    geometry.Columns - 1);
                var row = Mathf.Clamp(
                    Mathf.FloorToInt(
                        (logicalY - geometry.ReferenceY) / geometry.PanelHeight),
                    0,
                    geometry.Rows - 1);
                return row * geometry.Columns + column;
            }

            private static bool FamilyEnabled(
                NativeSim.Level1SpellMenuSnapshot snapshot,
                int uiIndex)
            {
                return uiIndex >= 0 &&
                       snapshot.familyEnabled != null &&
                       uiIndex < snapshot.familyEnabled.Length &&
                       snapshot.familyEnabled[uiIndex] != 0;
            }

            private static int UnlockedTier(
                NativeSim.Level1SpellMenuSnapshot snapshot,
                int uiIndex,
                SpellMenuGeometry geometry)
            {
                return Mathf.Clamp(
                    snapshot.familyUnlockedTier[uiIndex],
                    0,
                    geometry.TierCount - 1);
            }

            private void ClearChoice()
            {
                SelectedUiIndex = -1;
                HoverTier = -1;
                Stage = 0;
                pressedMouseButton = -1;
            }
        }

        internal const int AuthoritativeSimulationHz = 30;
        private const float SourceUnitsPerUnityUnit = 64f;
        private const int TerrainCells = 256;
        private const int TerrainSourceStep = 256;
        private const float MapUnitySize = 1024f;
        private const float MouseDeflectionGain = 4.5f;
        private const float KeyboardDeflectionRate = 110f;
        private const int CarpetOverlayLayer = 30;
        private const byte Level1ExitSwitchActorKind = 6;
        private const byte Level1MultipartHeadActorKind = 7;
        private const byte Level1ExperienceScrollActorKind = 8;
        private const byte Level1TreeActorKind = 9;
        private const byte Level1StoneActorKind = 10;
        private const byte Level1CheckpointSmokeEmitterActorKind = 11;
        private const int AutoplayFinalTick = 440;
        private static readonly int[] AutoplayCaptureTicks = { 145, 163, 220, 303, 400 };
        private static readonly ulong[] AutoplayTerrainIndexHashes =
        {
            0x0a49c01e798aa855,
            0xae4de9caf41d5473,
            0xba9873861a554bcf,
            0xb09bfe27ac05352f,
            0x053d7e07096e5077
        };
        private static readonly ulong[] AutoplayTerrainCoverageHashes =
        {
            0x4805132e71142646,
            0x1606b3e784f10124,
            0xd1d7279797f670f3,
            0x8e818be5a1fecac3,
            0x35be7aa3bef7daef
        };
        private static readonly ulong[] AutoplayTerrainReadbackBottomUpHashes =
        {
            0x6116176f23d78261,
            0x513fbc1f0ce27501,
            0x9d5154f952c93735,
            0xbf6fa51c101089b5,
            0x42a6120608fd3c75
        };
        private static readonly ulong[] AutoplayTerrainLogicalTopDownHashes =
        {
            0x6d2584eb3f5e1d21,
            0x24a3ce628a9b18e1,
            0x42e55c4ea9e59855,
            0x5dde721e6b801ec5,
            0x501e861d317ae825
        };
        private static readonly uint[] AutoplayTerrainReflected =
            { 0, 0, 0, 0, 924 };
        private static readonly uint[] AutoplayTerrainMode5 =
            { 586, 606, 588, 362, 214 };
        private static readonly uint[] AutoplayTerrainMode26 =
            { 0, 0, 6, 234, 36 };
        private static readonly ulong[] AutoplayTerrainWrites =
            { 57887, 61935, 29921, 50474, 6895 };
        private static readonly ulong[] AutoplayStateHashes =
        {
            0x4c3ead1f07462136,
            0x14b0c5a8341ab28b,
            0x8d15e8c188324cb0,
            0x031307cd7b0a3e21,
            0x6ea3b184dbd5eeb1
        };
        private static readonly ulong[] AutoplayConformanceHashes =
        {
            0xa6e0c8caeacf73f8,
            0x4bc2312004a14bd6,
            0x7f05e39bbd5b680d,
            0x902cdb8f4d06504c,
            0x263fb33a7683a4da
        };
        private static readonly ulong[] AutoplayEventHeads =
            { 953, 1073, 1434, 1969, 2600 };
        private static readonly int[] YawLookupMode20 =
            { 0, 0, 0, 1, 1, 1, 2, 2, 2, 2, 2, 1, 1, 1, 0, 0 };
        private static readonly int[] YawLookupMode19 =
            { 0, 1, 1, 2, 2, 3, 3, 4, 4, 3, 3, 2, 2, 1, 1, 0 };
        private static readonly string[] AutoplayCommandPrefixHashes =
        {
            "a2eec00e0cdd9a65392775c8ccc2310b4004ae176639e4e5fc698818b0f62e40",
            "29e1f705610953107f09e61ed03309addcb5c3866a5dc647ead3e837311917a5",
            "fd5aa43307fa298aceb3d7f6074bfa78968684a46680da2daf20508cb5ec80d5",
            "414173b2b45c6b7cdd421919eb9ec5fa3035c1fa628d24e9a12fc1e6bb23e43c",
            "e393b346b503dc0c058214f70eed3dfd6ab6a4103b8ec2cfe74d3d4324fe42eb"
        };
        private static readonly string[] AutoplayEventLifecycleHashes =
        {
            "6291b1222c881fe09b508737ba2648d6da5d7914ccadb1eb3efec880f731da57",
            "a896059932ffa14df7da7d168f03fabbeb0297f63add55419a2fe0cc962bf536",
            "8500955f340b67a010cca7c51744c9b083e11e6cf2daaa8a9dffc604531740c6",
            "b7ad589a207fe6957b07ac996a53800f15eabec1b4bb041bf7fbe3ef255486d6",
            "ea4be024470e14902c78b5b66d7e7373b592c39aafc24d8004a9cec2e24dd68f"
        };
        private static readonly string[] AutoplayEventContentHashes =
        {
            "247058da12a1dee22b45b16e0fbe8cdf59a886be703e384c177815e6d622c8f0",
            "a5681b2062bbe0c4652ba643d339a99156dac6aeb6b45088bf7aa13e5916852e",
            "6422adc6869e1c151100cc18c63374bec8ac361aae0a149a4ceb85eb62b178d0",
            "02645bd42837207218f7b066dcf146719758a1e31375118ec2549c5b39c46b96",
            "7ec85e7dc7e4ccbf83d147bb8f650ff1666d68b775402aeb4f3cb9b0947a6940"
        };
        private const ulong ExpectedResetHash = 0x9da4bc91af58f3ce;
        private const ulong ExpectedResetConformanceHash = 0x77b96b82492d6f4b;
        private const ulong ExpectedSessionConformanceHash = 0x36bd7f1ddf0537b5;
        private static readonly byte[] ExpectedLevel1TerrainSelectors =
        {
            0, 1, 2, 4, 5, 9, 22, 23, 24, 25, 26, 27, 36, 37, 38, 39, 44, 45,
            46, 47, 52, 53, 54, 55, 56, 57, 58, 59, 64, 65, 66, 67, 69, 70, 71,
            72, 73, 74, 75, 76, 84, 85, 86, 87, 88, 89, 97, 99, 101, 114, 115,
            116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 128, 131, 136, 139
        };
        private static readonly ushort[] ExpectedLevel1GoatEntities =
        {
            50, 79, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94,
            95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108,
            109, 110, 111
        };
        internal const string ExpectedPackageHash =
            "730d2fa084265df839f3f2b55ed766ac167a8b152ca4631b5ded9d0bb9104ac8";
        internal const string ExpectedClosureHash =
            "878123908f6de05f684f4cd1d73cff8d2ff08701cb6fdd12b7165bfcacc75719";
        internal const string ExpectedOwnerAssetClosureHash =
            "878123908f6de05f684f4cd1d73cff8d2ff08701cb6fdd12b7165bfcacc75719";
        internal const string ExpectedNativePluginHash =
            "536c31fb1f97b7362327ee4007c7cd9feb5013c79a485a5890f906b754bd332a";

        private NativeSession session;
        private PlaythroughRecorder playthroughRecorder;
        private OwnerAssetCache ownerAssetCache;
        private OwnerAudioPresentation ownerAudio;
        private AudioQualificationCapture audioCapture;
        private LocalAudioQualification localAudioQualification;
        private PositionalAudioQualification positionalAudioQualification;
        private NativeSim.WorldInfo worldInfo;
        private NativeSim.SessionSnapshot snapshot;
        private NativeSim.TerrainState terrainState;
        private NativeSim.PresentationState presentationState;
        private NativeSim.RenderCameraState renderCameraState;
        private NativeSim.ObjectiveSnapshot[] objectives;
        private NativeSim.EntitySnapshot[] entities;
        private NativeSim.GoatSnapshot[] goats;
        private NativeSim.Level1MissionSnapshot mission;
        private NativeSim.Level1HudSnapshot level1Hud;
        private NativeSim.Level1SpellMenuSnapshot level1SpellMenu;
        private NativeSim.Level1MinimapMarks level1MinimapMarks;
        private NativeSim.Level1MinimapEntityCommand[] level1MinimapEntities =
            Array.Empty<NativeSim.Level1MinimapEntityCommand>();
        private NativeSim.Level1MinimapEntitiesInfo level1MinimapEntitiesInfo;
        private NativeSim.Level1PaletteInfo level1Palette;
        private NativeSim.Level1ExitPresentation exitPresentation;
        private NativeSim.Level1ActorSnapshot[] missionActors =
            Array.Empty<NativeSim.Level1ActorSnapshot>();
        private NativeSim.Level1MultipartSegmentSnapshot[] multipartSegments =
            Array.Empty<NativeSim.Level1MultipartSegmentSnapshot>();
        private NativeSim.Level1CastleSnapshot level1Castle;
        private NativeSim.Level1CastleLifecycleSnapshot castleLifecycle;
        private NativeSim.ManaCarrierSnapshot manaCarrier;
        private NativeSim.ManaSphereSnapshot[] manaSpheres =
            Array.Empty<NativeSim.ManaSphereSnapshot>();
        private NativeSim.FireballProjectileSnapshot[] fireballProjectiles =
            Array.Empty<NativeSim.FireballProjectileSnapshot>();
        private NativeSim.FireballImpactSnapshot[] fireballImpacts =
            Array.Empty<NativeSim.FireballImpactSnapshot>();
        private int[] goatOrdinalByEntity;
        private byte[] terrainTypes;
        private byte[] groundHeights;
        private byte[] terrainShade;
        private byte[] terrainAngles;
        private byte[] secondaryHeights;
        private short[] terrainEntityIndices;
        private Texture2D typeTexture;
        private Texture2D heightTexture;
        private Texture2D shadeTexture;
        private Texture2D angleTexture;
        private Texture2D secondaryTexture;
        private Texture2D minimapTexture;
        private Mesh terrainMesh;
        private readonly Dictionary<int, Texture2D> ownerObjectiveTextures = new();
        private readonly Dictionary<int, HudObjectiveRowDescriptor> ownerObjectiveRows = new();
        private Texture2D ownerAuthorityBadgeTexture;
        private Texture2D ownerPaletteTexture;
        private byte[] ownerOriginalPaletteRgb8;
        private readonly byte[] level1EffectivePaletteRgb8 = new byte[256 * 3];
        private Texture2D ownerShadeLutTexture;
        private Texture2DArray ownerTerrainTiles;
        private Material ownerTerrainMaterial;
        private Texture2D terrainPnt5RowsTexture;
        private Texture2D terrainSurfaceDisplacementTexture;
        private Texture2D terrainGridCellValidTexture;
        private readonly byte[] terrainFrameIndices =
            new byte[NativeSim.TerrainFramePixels];
        private readonly byte[] terrainFrameCoverage =
            new byte[NativeSim.TerrainFramePixels];
        private readonly byte[] terrainFrameUploadIndices =
            new byte[NativeSim.TerrainFramePixels];
        private readonly byte[] terrainFrameUploadCoverage =
            new byte[NativeSim.TerrainFramePixels];
        private Texture2D terrainFrameIndexTexture;
        private Texture2D terrainFrameCoverageTexture;
        private Material terrainFrameCompositeMaterial;
        private Material terrainDepthOnlyMaterial;
        private CommandBuffer terrainFrameCommandBuffer;
        private NativeSim.TerrainFrameInfo terrainFrameInfo;
        private bool terrainFrameCompositeLive;
        private bool terrainFrameLiveLogged;
        private bool terrainFrameGpuValidated;
        private bool terrainFrameRouteGpuValidated;
        private int ownerObjectiveVisibleSelector = -1;
        private bool ownerObjectiveBlendLogged;
        private readonly float[] terrainPnt5Rows =
            new float[TerrainCells * TerrainCells];
        private readonly float[] terrainSurfaceDisplacement =
            new float[TerrainCells * TerrainCells];
        private readonly byte[] terrainGridVertexValid =
            new byte[TerrainCells * TerrainCells];
        private readonly byte[] terrainGridCellValid =
            new byte[TerrainCells * TerrainCells];
        private bool terrainGridLiveLogged;
        private byte[] ownerHudPaletteRgb8;
        private readonly Dictionary<int, Color32[]> ownerHudBasePixels = new();
        private readonly Dictionary<int, byte[]> ownerObjectiveRasters = new();
        private readonly Dictionary<int, byte[]> ownerObjectiveOverlayIndices = new();
        private readonly Dictionary<int, byte[]> ownerObjectiveOverlayMasks = new();
        private readonly Dictionary<int, Color32[]> ownerObjectiveUploadPixels = new();
        private Color32[] ownerAuthorityBadgeBasePixels;
        private byte appliedExitPaletteEffect = byte.MaxValue;
        private byte appliedExitPaletteWeight = byte.MaxValue;
        private byte appliedExitPaletteDenominator = byte.MaxValue;
        private short appliedPaletteCount = short.MinValue;
        private byte[] ownerHudTables;
        private byte[] ownerHudClrn;
        private byte[] ownerHudMinimapAperture;
        private int[] ownerHudSine;
        private TerrainMaterialDescriptor terrainMaterialDescriptor;
        private EntityPresentationDescriptor entityPresentationDescriptor;
        private HudDescriptor hudDescriptor;
        private readonly Dictionary<int, TmapDescriptor> tmapDescriptors = new();
        private readonly Dictionary<int, HudBitmapDescriptor> hudBitmapDescriptors = new();
        private readonly Dictionary<int, Texture2D> hudBitmapTextures = new();
        private readonly Dictionary<int, byte[]> hudBitmapIndices = new();
        private readonly Dictionary<int, byte[]> hudBitmapMasks = new();
        private readonly Dictionary<int, Texture2D> ownerSpellTierTextures = new();
        private readonly Dictionary<int, Material> entitySpriteMaterials = new();
        private readonly Dictionary<int, Texture2D> entitySpriteTextures = new();
        private readonly Dictionary<int, string> entitySpriteIndicesSha256 = new();
        private Mesh entityQuadMesh;
        private Mesh entityQuadMirroredMesh;
        private readonly List<EntityMarker> entityRegistry = new();
        private readonly Dictionary<ushort, MissionMarker> missionActorRegistry = new();
        private readonly Dictionary<ushort, MissionMarker> multipartSegmentRegistry = new();
        private readonly Dictionary<uint, MissionMarker> missionProjectileRegistry = new();
        private readonly Dictionary<uint, MissionMarker> missionImpactRegistry = new();
        private readonly Dictionary<uint, MissionMarker> manaSphereRegistry = new();
        private MissionMarker castleMarker;
        private MissionMarker manaCarrierMarker;
        private Transform missionPresentationRoot;
        private readonly List<Transform> terrainTiles = new();
        private readonly Queue<string> recentEventDiagnostics = new();
        private Camera flightCamera;
        private Camera carpetOverlayCamera;
        private Transform presentationRig;
        private Transform carpetVisual;
        private Material fireballMaterial;
        private Vector3 targetPosition;
        private Quaternion targetRotation;
        private string packagePath;
        private string closurePath;
        private string fatalError;
        private string movementResolution = "accepted";
        private GUIStyle hudStyle;
        private GUIStyle titleStyle;
        private float steeringRoll;
        private float steeringPitch;
        private sbyte appliedRoll;
        private sbyte appliedPitch;
        private bool fireRequested;
        private bool rightSpellRequested;
        private bool selectedSpellRequested;
        private short requestedSpell = NativeSim.NoRequestedSpell;
        private bool spellMenuOpen;
        private int spellMenuHoverUiIndex = -1;
        private int spellMenuSelectedUiIndex = -1;
        private int spellMenuHoverTier = -1;
        private readonly SpellMenuController spellMenuController = new();
        private readonly SpellMenuCommandBuffer spellMenuCommands = new();
        private bool autoplay;
        private bool autoplayQuit;
        private bool level1Autoplay;
        private bool level1AutoplayQuit;
        private bool level1AutoplayFast;
        private bool level1AutoplayPostgame;
        private Level1Autopilot level1AutopilotDriver;
        private string level1AutoplayPhase = string.Empty;
        private bool localAudioQualificationMode;
        private bool localAudioQualificationExitRequested;
        private bool positionalAudioQualificationMode;
        private bool positionalAudioQualificationExitRequested;
        private bool qualifiedMode;
        private bool debugMode;
        private bool debugEntities;
        private bool entityGallery;
        private bool entityGalleryCaptured;
        private bool hudCaptureRequested;
        private bool hudCaptured;
        private bool hudComparisonCaptured;
        private bool level1StageFourCaptured;
        private bool runtimeExitRequested;
        private bool disposed;
        private bool autoplayCapturePending;
        private int simTick;
        private int autoplayCaptureTick = -1;
        private int entityGalleryFrames;
        private int level1StageFourCaptureFrames;
        private int autoplayFinalWaitFrames;
        private string autoplayCaptureFailure;
        private float minimumCarpetTerrainClearanceSource = float.PositiveInfinity;
        private ulong eventCursor;
        private ulong fireEventsObserved;
        private ulong fireInputLatchedCount;
        private ulong fireFlagsSubmittedCount;
        private bool AutomatedInput => autoplay || level1Autoplay;
        private ulong fireVisualSpawnedCount;
        private ulong goatLiveWorldHash;
        private ulong goatBatchGeneration;
        private ulong goatBatchTick;
        private readonly List<NativeSim.NativeEvent> qualificationGoatEvents = new();

        internal readonly struct GoatPresentationProbe
        {
            internal GoatPresentationProbe(
                ulong generation,
                ulong snapshotTick,
                ulong goatBatchGeneration,
                ulong goatBatchTick,
                ulong presentationGeneration,
                ulong presentationTick,
                ulong renderGeneration,
                ulong renderTick,
                ulong eventCursor,
                ulong snapshotEventHead,
                ulong goatLiveWorldHash,
                NativeSim.GoatSnapshot goat,
                ushort cameraX,
                ushort cameraY,
                short cameraZ,
                long unwrappedCameraX,
                long unwrappedCameraY,
                short renderYaw,
                short renderOffsetX,
                short renderOffsetY,
                Vector3 cameraTargetPosition,
                Vector3 nodePosition,
                bool rendererEnabled,
                int materialSelector,
                string indicesSha256,
                bool indicesTextureBound,
                bool mirroredMesh)
            {
                Generation = generation;
                SnapshotTick = snapshotTick;
                GoatBatchGeneration = goatBatchGeneration;
                GoatBatchTick = goatBatchTick;
                PresentationGeneration = presentationGeneration;
                PresentationTick = presentationTick;
                RenderGeneration = renderGeneration;
                RenderTick = renderTick;
                EventCursor = eventCursor;
                SnapshotEventHead = snapshotEventHead;
                GoatLiveWorldHash = goatLiveWorldHash;
                Goat = goat;
                CameraX = cameraX;
                CameraY = cameraY;
                CameraZ = cameraZ;
                UnwrappedCameraX = unwrappedCameraX;
                UnwrappedCameraY = unwrappedCameraY;
                RenderYaw = renderYaw;
                RenderOffsetX = renderOffsetX;
                RenderOffsetY = renderOffsetY;
                CameraTargetPosition = cameraTargetPosition;
                NodePosition = nodePosition;
                RendererEnabled = rendererEnabled;
                MaterialSelector = materialSelector;
                IndicesSha256 = indicesSha256;
                IndicesTextureBound = indicesTextureBound;
                MirroredMesh = mirroredMesh;
            }

            internal ulong Generation { get; }
            internal ulong SnapshotTick { get; }
            internal ulong GoatBatchGeneration { get; }
            internal ulong GoatBatchTick { get; }
            internal ulong PresentationGeneration { get; }
            internal ulong PresentationTick { get; }
            internal ulong RenderGeneration { get; }
            internal ulong RenderTick { get; }
            internal ulong EventCursor { get; }
            internal ulong SnapshotEventHead { get; }
            internal ulong GoatLiveWorldHash { get; }
            internal NativeSim.GoatSnapshot Goat { get; }
            internal ushort CameraX { get; }
            internal ushort CameraY { get; }
            internal short CameraZ { get; }
            internal long UnwrappedCameraX { get; }
            internal long UnwrappedCameraY { get; }
            internal short RenderYaw { get; }
            internal short RenderOffsetX { get; }
            internal short RenderOffsetY { get; }
            internal Vector3 CameraTargetPosition { get; }
            internal Vector3 NodePosition { get; }
            internal bool RendererEnabled { get; }
            internal int MaterialSelector { get; }
            internal string IndicesSha256 { get; }
            internal bool IndicesTextureBound { get; }
            internal bool MirroredMesh { get; }
        }

        internal bool QualificationStartupReady =>
            qualifiedMode &&
            string.IsNullOrEmpty(fatalError) &&
            session != null &&
            ownerAssetCache != null &&
            hudDescriptor != null &&
            ownerAudio != null &&
            presentationRig != null &&
            flightCamera != null &&
            terrainFrameCompositeLive;

        internal bool QualificationStartupPresentationClean =>
            carpetVisual == null &&
            carpetOverlayCamera == null &&
            fireballMaterial == null &&
            FindObjectsByType<Rigidbody>(FindObjectsSortMode.None).Length == 0 &&
            FindObjectsByType<Collider>(FindObjectsSortMode.None).Length == 0 &&
            FindObjectsByType<Terrain>(FindObjectsSortMode.None).Length == 0 &&
            FindObjectsByType<Light>(FindObjectsSortMode.None).Length == 0;

        internal string QualificationFatalError => fatalError;

        internal ulong QualificationNativeTick => snapshot.tick;
        internal int QualificationSpellMenuState => spellMenuController.MenuState;
        internal int QualificationSpellMenuStage => spellMenuController.Stage;
        internal int QualificationSpellMenuHoverTier => spellMenuController.HoverTier;
        internal int QualificationPendingSpellMenuCommands => spellMenuCommands.Count;
        internal NativeSim.Level1SpellMenuSnapshot QualificationSpellMenuSnapshot =>
            level1SpellMenu;
        internal byte QualificationSelectedSpellModel => level1Hud.selectedModel;
        internal uint QualificationSpellDirectionMask =>
            level1Hud.playerStatusFlags & 0x0300;
        internal uint QualificationProjectileCount => mission.projectileCount;
        internal ulong QualificationGoatLiveWorldHash => goatLiveWorldHash;
        internal int QualificationGoatLiveCount => goats?.Length ?? 0;
        internal int QualificationGoatLiveSpriteCount
        {
            get
            {
                var count = 0;
                foreach (var marker in entityRegistry)
                {
                    if (
                        marker.OwnerSprite &&
                        goatOrdinalByEntity != null &&
                        marker.Index < goatOrdinalByEntity.Length &&
                        goatOrdinalByEntity[marker.Index] >= 0)
                    {
                        count++;
                    }
                }
                return count;
            }
        }

        internal GoatPresentationProbe QualificationGoatPresentation(ushort entityIndex)
        {
            Require(
                qualifiedMode && session != null && goats != null,
                "qualified live Goat presentation probe");
            var marker = entityRegistry.Find(value => value.Index == entityIndex);
            Require(
                marker != null &&
                marker.OwnerSprite &&
                marker.Node != null &&
                marker.Renderer != null &&
                marker.SpriteVisual != null,
                $"qualified Goat marker {entityIndex}");
            var ordinal =
                goatOrdinalByEntity != null &&
                entityIndex < goatOrdinalByEntity.Length
                    ? goatOrdinalByEntity[entityIndex]
                    : -1;
            Require(ordinal >= 0, $"qualified Goat ordinal {entityIndex}");
            var goat = goats[ordinal];
            Require(
                goat.entityIndex == entityIndex &&
                goat.authority == NativeSim.GoatLive,
                $"qualified Goat row {entityIndex}");
            var materialSelector = -1;
            foreach (var pair in entitySpriteMaterials)
            {
                if (ReferenceEquals(pair.Value, marker.Renderer.sharedMaterial))
                {
                    materialSelector = pair.Key;
                    break;
                }
            }
            Require(materialSelector >= 0, $"qualified Goat material selector {entityIndex}");
            var mesh = marker.SpriteVisual.GetComponent<MeshFilter>()?.sharedMesh;
            Require(mesh != null, $"qualified Goat mesh {entityIndex}");
            var mirroredMesh = ReferenceEquals(mesh, entityQuadMirroredMesh);
            Require(
                mirroredMesh || ReferenceEquals(mesh, entityQuadMesh),
                $"qualified Goat mesh identity {entityIndex}");
            Require(
                entitySpriteIndicesSha256.TryGetValue(
                    materialSelector,
                    out var indicesSha256),
                $"qualified Goat indices identity {entityIndex}");
            Require(
                entitySpriteTextures.TryGetValue(
                    materialSelector,
                    out var expectedIndicesTexture),
                $"qualified Goat indices texture {entityIndex}");
            var boundIndicesTexture =
                marker.Renderer.sharedMaterial.GetTexture("_Indices");
            Require(presentationRig != null, "qualified Goat presentation rig");
            return new GoatPresentationProbe(
                snapshot.generation,
                snapshot.tick,
                goatBatchGeneration,
                goatBatchTick,
                presentationState.generation,
                presentationState.tick,
                renderCameraState.generation,
                renderCameraState.tick,
                eventCursor,
                snapshot.eventHead,
                goatLiveWorldHash,
                goat,
                presentationState.cameraX,
                presentationState.cameraY,
                presentationState.cameraZ,
                unwrappedSourceX,
                unwrappedSourceY,
                renderCameraState.yaw,
                renderCameraState.renderOffsetX,
                renderCameraState.renderOffsetY,
                targetPosition,
                marker.Node.position,
                marker.Renderer.enabled,
                materialSelector,
                indicesSha256,
                ReferenceEquals(boundIndicesTexture, expectedIndicesTexture),
                mirroredMesh);
        }

        internal GoatPresentationProbe QualificationBindGoatMode20Sector(
            ushort entityIndex,
            int orientation)
        {
            Require(
                qualifiedMode && orientation >= 0 && orientation < 16,
                "qualified Goat mode20 sector");
            var marker = entityRegistry.Find(value => value.Index == entityIndex);
            Require(
                marker != null &&
                marker.Group != null &&
                marker.Group.particle.render_mode == 20 &&
                marker.Group.selector.mirror_when_orientation_gte_8,
                $"qualified Goat mode20 group {entityIndex}");
            var syntheticYaw = unchecked((short)(
                renderCameraState.yaw + (orientation << 7)));
            UpdateOwnerEntitySprite(
                entityIndex,
                syntheticYaw,
                marker.Group.key.animation_frame,
                marker.Group,
                marker.SpriteVisual,
                marker.Renderer);
            var probe = QualificationGoatPresentation(entityIndex);
            UpdateEntityPresentation();
            return probe;
        }

        internal void QualificationInjectGoatDamage(ushort entityIndex, int damage)
        {
            Require(
                qualifiedMode && session != null && damage > 0,
                "qualified Goat damage request");
            var batch = session.InjectGoatDamageForQualification(
                snapshot.generation,
                entityIndex,
                damage);
            Require(
                batch.tick == snapshot.tick &&
                batch.liveWorldHash != goatLiveWorldHash,
                "qualification Goat damage mutates only native pending state");
            RefreshLiveGoats();
            Require(
                goatBatchTick == snapshot.tick &&
                goatLiveWorldHash == batch.liveWorldHash,
                "qualification Goat damage refreshes the same native tick");
            UpdateEntityPresentation();
        }

        internal void QualificationInjectGoatDamageAtGeneration(
            ulong generation,
            ushort entityIndex,
            int damage)
        {
            Require(
                qualifiedMode && session != null && damage > 0,
                "qualified stale-generation Goat damage request");
            session.InjectGoatDamageForQualification(
                generation,
                entityIndex,
                damage);
        }

        internal NativeSim.NativeEvent[] QualificationGoatEvents(ushort entityIndex)
        {
            var matches = new List<NativeSim.NativeEvent>();
            foreach (var nativeEvent in qualificationGoatEvents)
            {
                if (nativeEvent.entityIndex == entityIndex)
                    matches.Add(nativeEvent);
            }
            return matches.ToArray();
        }

        internal void QualificationResetSession()
        {
            Require(qualifiedMode && session != null, "qualified Goat reset request");
            ResetSession();
        }

        internal void QualificationDriveSpellMenuInput(
            bool controlHeld,
            float logicalX,
            float logicalY,
            bool leftDown = false,
            bool leftHeld = false,
            bool leftUp = false,
            bool rightDown = false,
            bool rightHeld = false,
            bool rightUp = false,
            bool configureEligibility = false)
        {
            Require(
                qualifiedMode && session != null,
                "qualified spell-menu input request");
            if (controlHeld && !spellMenuOpen)
            {
                OpenSpellMenu(logicalX, logicalY);
            }
            if (spellMenuOpen)
            {
                ProcessSpellMenuInput(
                    logicalX,
                    logicalY,
                    leftDown,
                    leftHeld,
                    leftUp,
                    rightDown,
                    rightHeld,
                    rightUp,
                    configureEligibility);
            }
            if (!controlHeld && !leftHeld && !rightHeld && spellMenuOpen)
            {
                CloseSpellMenu(false);
            }
        }

        internal void QualificationRequestSpellCast(bool rightHand)
        {
            Require(
                qualifiedMode && session != null && !spellMenuOpen,
                "qualified spell-cast input request");
            LatchSpellCastInput(rightHand);
        }

        internal void QualificationRequestSelectedSpellCast()
        {
            Require(
                qualifiedMode && session != null && !spellMenuOpen,
                "qualified selected-spell input request");
            selectedSpellRequested = true;
            fireInputLatchedCount++;
        }

        internal void QualificationQueueRawSpellAssignment(
            bool rightHand,
            sbyte family,
            sbyte tier)
        {
            Require(
                qualifiedMode &&
                session != null &&
                !spellMenuOpen &&
                family >= 0 &&
                level1SpellMenu.familyEnabled != null &&
                family < level1SpellMenu.familyEnabled.Length &&
                level1SpellMenu.familyEnabled[family] != 0 &&
                tier >= 0,
                "qualified raw spell assignment request");
            spellMenuCommands.Enqueue(
                new SpellMenuCommand(
                    (sbyte)(rightHand ? 32 : 31),
                    family,
                    tier));
        }

        internal bool QualificationCycleSpell(bool rightHand, bool backward)
        {
            Require(
                qualifiedMode && session != null && !spellMenuOpen,
                "qualified spell-cycle input request");
            return QueueSpellCycle(rightHand, backward);
        }
        private ushort priorRawX;
        private ushort priorRawY;
        private long unwrappedSourceX;
        private long unwrappedSourceY;
        private readonly HashSet<int> capturedAutoplayTicks = new();
        private readonly HashSet<int> magentaCleanAutoplayTicks = new();
        private readonly HashSet<int> qualifiedTerrainRouteTicks = new();
        private readonly HashSet<int> qualifiedTerrainGpuRouteTicks = new();
        private readonly MemoryStream autoplayCommandBytes = new();
        private readonly MemoryStream autoplayEventContentBytes = new();
        private readonly MemoryStream autoplayEventLifecycleBytes = new();
        private readonly List<EntityGalleryItem> entityGalleryItems = new();
        private ulong autoplayEventRangeStart = 1;
        private short autoplayInitialYaw;
        private short autoplayInitialAltitude;
        private ulong autoplayInitialFireEvents;
        private bool autoplayTurnObserved;
        private bool autoplayAltitudeChangeObserved;
        private static readonly Vector2[][] TerrainOrientationUvs =
        {
            new[] { new Vector2(0, 1), new Vector2(1, 1), new Vector2(0, 0), new Vector2(1, 0) },
            new[] { new Vector2(1, 1), new Vector2(0, 1), new Vector2(1, 0), new Vector2(0, 0) },
            new[] { new Vector2(0, 0), new Vector2(1, 0), new Vector2(0, 1), new Vector2(1, 1) },
            new[] { new Vector2(1, 0), new Vector2(0, 0), new Vector2(1, 1), new Vector2(0, 1) },
            new[] { new Vector2(0, 1), new Vector2(0, 0), new Vector2(1, 1), new Vector2(1, 0) },
            new[] { new Vector2(0, 0), new Vector2(0, 1), new Vector2(1, 0), new Vector2(1, 1) },
            new[] { new Vector2(1, 1), new Vector2(1, 0), new Vector2(0, 1), new Vector2(0, 0) },
            new[] { new Vector2(1, 0), new Vector2(1, 1), new Vector2(0, 0), new Vector2(0, 1) },
        };

        [Serializable]
        private sealed class FullFrameCaptureManifest
        {
            public string schema;
            public string presentation_mode;
            public string evidence_authority;
            public string oracle_status;
            public int checkpoint_tick;
            public int last_command_index;
            public ulong native_render_tick;
            public bool exact_checkpoint;
            public string command_encoding;
            public string command_prefix_sha256;
            public SnapshotEvidence snapshot;
            public EventRangeEvidence events;
            public PresentationEvidence presentation;
            public RenderCameraEvidence render_camera;
            public TerrainFrameEvidence terrain;
            public string terrain_gpu_bottom_up_fnv1a64;
            public string terrain_logical_top_down_fnv1a64;
            public int width;
            public int height;
            public string package_sha256;
            public string closure_sha256;
            public string owner_cache_content_sha256;
            public string native_plugin_sha256;
            public string unity_version;
            public string unity_build_guid;
            public string graphics_api;
            public string graphics_device;
            public string png_sha256;
            public string rgba_sha256;
            public int magenta_pixels;
            public bool magenta_gate_pass;
        }

        [Serializable]
        private sealed class SnapshotEvidence
        {
            public ulong generation;
            public ulong tick;
            public ulong event_head;
            public string state_hash_fnv1a64;
            public string conformance_hash_fnv1a64;
            public int entity_index;
            public int x;
            public int y;
            public int z;
            public int yaw;
            public int display_pitch;
            public int effective_pitch;
            public int roll_accumulator;
            public int target_speed;
            public int actual_speed;
            public int strafe_speed;
            public int phase;
            public int last_resolution;
            public int profile;
            public int source_turn;
            public uint fire_sequence;
        }

        [Serializable]
        private sealed class EventRangeEvidence
        {
            public string content_schema;
            public string lifecycle_schema;
            public ulong first_sequence;
            public ulong last_sequence;
            public ulong count;
            public string content_sha256;
            public string lifecycle_sha256;
            public bool cursor_equals_head;
        }

        [Serializable]
        private sealed class PresentationEvidence
        {
            public ulong generation;
            public ulong tick;
            public int source_turn;
            public int camera_x;
            public int camera_y;
            public int camera_z;
            public int camera_yaw;
            public int camera_pitch;
            public int camera_roll;
            public int camera_fov;
            public int authority;
            public int flags;
        }

        [Serializable]
        private sealed class RenderCameraEvidence
        {
            public ulong generation;
            public ulong tick;
            public int source_turn;
            public int history_x;
            public int history_y;
            public int eye_z;
            public int yaw;
            public int pitch;
            public int roll;
            public int fov;
            public int slope_target_x;
            public int slope_target_y;
            public int render_offset_x;
            public int render_offset_y;
            public int authority;
            public int flags;
        }

        [Serializable]
        private sealed class TerrainFrameEvidence
        {
            public ulong generation;
            public ulong tick;
            public int width;
            public int height;
            public int stride;
            public int viewport_x;
            public int viewport_y;
            public int viewport_width;
            public int viewport_height;
            public string index_hash_fnv1a64;
            public string coverage_hash_fnv1a64;
            public uint reflected_triangles;
            public uint primary_mode5_triangles;
            public uint primary_mode26_triangles;
            public ulong terrain_pixel_writes;
        }

        [Serializable]
        private sealed class RouteCheckpointRecord
        {
            public int checkpoint_tick;
            public int last_command_index;
            public string command_prefix_sha256;
            public SnapshotEvidence snapshot;
            public EventRangeEvidence events;
            public PresentationEvidence presentation;
            public RenderCameraEvidence render_camera;
            public TerrainFrameEvidence terrain;
        }

        [Serializable]
        private sealed class TerrainMaterialDescriptor
        {
            public string schema;
            public PaletteDescriptor palette;
            public Bl32Descriptor bl32n;
            public TerrainUvDescriptor terrain_uv;
            public RenderingScopeDescriptor rendering_scope;
            public TmapsDescriptor tmaps1;
        }

        [Serializable]
        private sealed class PaletteDescriptor
        {
            public string path;
            public string rgb6_to_rgb8;
            public string rgb8_path;
            public string shade_lut_path;
            public string tablesn_path;
            public string clrn_path;
        }

        [Serializable]
        private sealed class Bl32Descriptor
        {
            public string indices_path;
            public int tile_width;
            public int tile_height;
            public int tile_count;
            public string terrain_selector;
            public int[] level1_used_selectors;
            public int level1_used_selector_count;
            public int level1_used_selector_max;
        }

        [Serializable]
        private sealed class TerrainUvDescriptor
        {
            public int source_row_count;
            public string persistent_group_expression;
            public UvGroupRecord[] persistent_unity_group_records;
        }

        [Serializable]
        private sealed class UvGroupRecord
        {
            public int group;
            public int[] points_flat;
        }

        [Serializable]
        private sealed class RenderingScopeDescriptor
        {
            public string qualified;
            public string not_yet_qualified;
            public string claim;
        }

        [Serializable]
        private sealed class TmapsDescriptor
        {
            public int[] frame_used_selectors;
            public int[] scene_reachable_selectors;
            public TmapDescriptor[] textures;
        }

        [Serializable]
        private sealed class TmapDescriptor
        {
            public int index;
            public string status;
            public int width;
            public int height;
            public int animation_or_allocation_group;
            public bool animated;
            public int frame_count;
            public string indices_path;
            public string decoded_payload_path;
        }

        [Serializable]
        private sealed class EntityPresentationDescriptor
        {
            public string schema;
            public int qualified_actor_count;
            public int mapped_reset_effect_count;
            public int reset_chain_drawable_count;
            public int qualified_view_count;
            public int suppressed_terrain_head_count;
            public int off_chain_active_count;
            public int active_reset_count;
            public EntityPresentationGroup[] groups;
        }

        [Serializable]
        private sealed class EntityPresentationGroup
        {
            public string group_id;
            public int[] entity_indices;
            public int entity_count;
            public int[] reset_chain_drawable_entity_indices;
            public int reset_chain_drawable_count;
            public int[] qualified_view_entity_indices;
            public int qualified_view_count;
            public int[] suppressed_terrain_head_entity_indices;
            public int[] off_chain_entity_indices;
            public string authority;
            public EntityPresentationKey key;
            public string semantic_family;
            public string qualified_status;
            public string qualified_hidden_reason;
            public EntityParticleDescriptor particle;
            public EntitySelectorDescriptor selector;
            public EntityBillboardDescriptor billboard;
        }

        [Serializable]
        private sealed class EntityPresentationKey
        {
            public int class_id;
            public int model;
            public int action;
            public int parameter_record_index;
            public int particle_parameter_index;
            public int animation_frame;
            public long control_flags_u32;
        }

        [Serializable]
        private sealed class EntityParticleDescriptor
        {
            public int base_tmap_selector;
            public int logical_height_source_units;
            public int render_mode;
        }

        [Serializable]
        private sealed class EntitySelectorDescriptor
        {
            public string bank;
            public int[] qualified_selectors;
            public string expression;
            public int[] yaw_lookup_0_15;
            public bool mirror_when_orientation_gte_8;
            public string yaw_sector_expression;
        }

        [Serializable]
        private sealed class EntityBillboardDescriptor
        {
            public string facing;
            public string world_height;
            public string world_width;
            public string anchor;
            public string source_anchor_status;
            public string hotspot;
            public string depth;
            public string compositing;
            public EntityTextureAnimationDescriptor texture_animation;
        }

        [Serializable]
        private sealed class EntityTextureAnimationDescriptor
        {
            public bool animated;
            public int frame_count;
            public string scope;
            public string cache_payload;
            public string qualified_view;
        }

        [Serializable]
        private sealed class HudDescriptor
        {
            public string schema;
            public HudReferenceLayout reference_layout;
            public HudBitmapDescriptor[] bitmap_assets;
            public HudTopBarDescriptor top_bar;
            public HudSpellsDescriptor spells;
            public HudObjectiveDescriptor objective;
            public HudAuthorityBadgeDescriptor authority_badge;
            public HudMinimapDescriptor minimap;
            public string[] deferred;
        }

        [Serializable]
        private sealed class HudReferenceLayout
        {
            public int width;
            public int height;
            public string uniform_scale_expression;
            public string filter;
            public string resolution_bank;
            public int source_scale;
            public string bank_status;
            public string high_resolution_bank_status;
        }

        [Serializable]
        private sealed class HudBitmapDescriptor
        {
            public int selector;
            public string bank;
            public string source_asset_id;
            public string indices_path;
            public string mask_path;
            public string indices_sha256;
            public string mask_sha256;
            public int stored_width;
            public int stored_height;
            public int runtime_width;
            public int runtime_height;
            public int source_scale;
            public string palette;
            public string palette_effects;
            public string filter;
            public string authority;
        }

        [Serializable]
        private sealed class HudTopBarDescriptor
        {
            public string source_function;
            public HudPanelDescriptor[] panels;
            public HudDeferredField health_mana_bars;
        }

        [Serializable]
        private sealed class HudPanelDescriptor
        {
            public string semantic_role;
            public int selector;
            public int x;
            public int y;
            public string authority;
        }

        [Serializable]
        private sealed class HudDeferredField
        {
            public string authority;
            public string reason;
        }

        [Serializable]
        private sealed class HudSpellsDescriptor
        {
            public string source_function;
            public int frame_selector;
            public HudSpellSlotDescriptor[] slots;
            public HudSpellMenuDescriptor menu;
            public HudSpellTierRowDescriptor[] tier_rows;
            public HudDeferredField glow_tier_charge;
        }

        [Serializable]
        private sealed class HudSpellMenuDescriptor
        {
            public string source_function;
            public int rows;
            public int columns;
            public int reference_y;
            public int edge_selector;
            public int panel_selector;
            public int selection_frame_selector;
            public int disabled_panel_selector;
            public int small_icon_base_selector;
            public int left_assignment_corner_selector;
            public int right_assignment_corner_selector;
            public int tier_panel_selector;
            public int tier_dark_panel_selector;
            public int tier_locked_panel_selector;
            public int tier_selection_frame_selector;
            public int tier_number_base_selector;
            public int tier_icon_base_selector;
            public int tier_count;
            public int panel_width;
            public int panel_height;
            public int edge_width;
            public string authority;
        }

        [Serializable]
        private sealed class HudSpellTierRowDescriptor
        {
            public int tier;
            public string text;
            public string render_path;
            public int render_width;
            public int render_height;
            public int[] glyph_indices;
            public int palette_index;
        }

        [Serializable]
        private sealed class HudSpellSlotDescriptor
        {
            public string semantic_role;
            public int x;
            public int y;
            public int holder_entity;
            public int model;
            public int icon_selector;
            public string authority;
            public string activation_authority;
        }

        [Serializable]
        private sealed class HudObjectiveDescriptor
        {
            public string source_function;
            public string selector_expression;
            public int selector;
            public string text_bytes_path;
            public string text_sha256;
            public string text_encoding;
            public string font_bank;
            public string glyph_expression;
            public int[] glyph_indices;
            public int[] frame_selectors;
            public string raster_path;
            public int raster_plane_bytes;
            public int render_width;
            public int render_height;
            public int reference_x;
            public int reference_y;
            public string authority;
            public int objective_textbox_ticks;
            public HudTintDescriptor tint;
            public HudObjectiveBackgroundBlendDescriptor background_blend;
            public string[] draw_order;
            public HudObjectiveRowDescriptor[] rows;
        }

        [Serializable]
        private sealed class HudObjectiveBackgroundBlendDescriptor
        {
            public string source_table_path;
            public int table_offset;
            public int clrn_selector;
            public int blend_selector;
            public string expression;
            public string underlay_scope;
            public string display_adaptation;
        }

        [Serializable]
        private sealed class HudObjectiveRowDescriptor
        {
            public int selector;
            public string text_bytes_path;
            public string text_sha256;
            public string[] lines_ascii;
            public int[] text_advances;
            public string raster_path;
            public int raster_plane_bytes;
            public string foreground_indices_sha256;
            public string foreground_mask_sha256;
            public int foreground_opaque_pixels;
            public int render_width;
            public int render_height;
            public int reference_x;
            public int reference_y;
            public int text_x;
            public int text_y;
            public int blend_x;
            public int blend_y;
            public int blend_width;
            public int blend_height;
            public string authority;
        }

        [Serializable]
        private sealed class HudTintDescriptor
        {
            public string active_map_palette_lut;
            public int lut_selector;
            public int palette_index;
            public int[] paln_rgb8;
            public string historical_pointer_name;
        }

        [Serializable]
        private sealed class HudAuthorityBadgeDescriptor
        {
            public string text;
            public string render_path;
            public int render_width;
            public int render_height;
            public int reference_x;
            public int reference_y;
        }

        [Serializable]
        private sealed class HudMinimapDescriptor
        {
            public int x;
            public int y;
            public int width;
            public int height;
            public int logical_width;
            public int logical_height;
            public int presentation_scale;
            public string map_mode;
            public string resolution_branch;
            public HudMinimapApertureDescriptor aperture;
            public HudMinimapRotationDescriptor rotation;
            public string terrain_authority;
            public string entity_positions_authority;
            public string entity_overlay_mode;
            public string player_pose_authority;
            public string entity_colour_filter_status;
            public string objective_marks_status;
        }

        [Serializable]
        private sealed class HudMinimapApertureDescriptor
        {
            public string path;
            public string sha256;
            public int opaque_pixels;
            public int table_offset;
            public int phase_step;
            public HudMinimapScanlineDescriptor[] scanlines;
        }

        [Serializable]
        private sealed class HudMinimapScanlineDescriptor
        {
            public int y;
            public int start_x_inclusive;
            public int end_x_exclusive;
        }

        [Serializable]
        private sealed class HudMinimapRotationDescriptor
        {
            public string sine_table_path;
            public string sine_table_sha256;
        }

        [RuntimeInitializeOnLoadMethod(RuntimeInitializeLoadType.AfterSceneLoad)]
        private static void EnsureRuntime()
        {
            if (FindFirstObjectByType<MagicCarpetRuntime>() == null)
            {
                new GameObject("Magic Carpet Runtime").AddComponent<MagicCarpetRuntime>();
            }
        }

        private void Awake()
        {
            try
            {
                var arguments = Environment.GetCommandLineArgs();
                autoplay = HasArgument(arguments, "-mc-autoplay");
                autoplayQuit = HasArgument(arguments, "-mc-autoplay-quit");
                level1Autoplay = HasArgument(arguments, "-mc-level1-autoplay");
                level1AutoplayQuit =
                    HasArgument(arguments, "-mc-level1-autoplay-quit");
                level1AutoplayFast =
                    HasArgument(arguments, "-mc-level1-autoplay-fast");
                level1AutoplayPostgame =
                    HasArgument(arguments, "-mc-level1-autoplay-postgame");
                if (autoplay && level1Autoplay)
                {
                    throw new InvalidDataException(
                        "synthetic autoplay and Level 1 autoplay are mutually exclusive");
                }
                if (level1AutoplayFast && !level1Autoplay)
                {
                    throw new InvalidDataException(
                        "-mc-level1-autoplay-fast requires -mc-level1-autoplay");
                }
                if (level1AutoplayPostgame && !level1Autoplay)
                {
                    throw new InvalidDataException(
                        "-mc-level1-autoplay-postgame requires -mc-level1-autoplay");
                }
                if (level1AutoplayFast)
                {
                    Debug.Log(
                        "MC_LEVEL1_AUTOPLAY fast fixed-tick pump enabled");
                }
                localAudioQualificationMode =
                    HasArgument(arguments, "-mc-audio-qualify-local");
                positionalAudioQualificationMode =
                    HasArgument(arguments, "-mc-audio-qualify-positional");
                qualifiedMode = HasArgument(arguments, "-mc-qualified");
                debugMode = HasArgument(arguments, "-mc-debug");
                debugEntities = HasArgument(arguments, "-mc-debug-entities");
                entityGallery = HasArgument(arguments, "-mc-entity-gallery");
                if (qualifiedMode == debugMode)
                {
                    throw new InvalidDataException(
                        "select exactly one presentation mode: -mc-qualified or -mc-debug");
                }
                if (qualifiedMode && (debugEntities || entityGallery))
                {
                    throw new InvalidDataException(
                        "qualified mode rejects debug entities and entity gallery");
                }
                if (!debugMode && debugEntities)
                {
                    throw new InvalidDataException(
                        "-mc-debug-entities requires explicit -mc-debug");
                }
                var requestedAudioCapture =
                    Environment.GetEnvironmentVariable("MC_AUDIO_CAPTURE_PATH");
                if ((localAudioQualificationMode ||
                     positionalAudioQualificationMode) &&
                    string.IsNullOrWhiteSpace(requestedAudioCapture))
                {
                    throw new InvalidDataException(
                        "audio qualification requires MC_AUDIO_CAPTURE_PATH");
                }
                if (!string.IsNullOrWhiteSpace(requestedAudioCapture))
                {
                    AudioQualificationCapture.ResolveQualifiedCapturePath(
                        requestedAudioCapture);
                }
                ResolveOwnerPaths(arguments, out packagePath, out closurePath);
                var assetCachePath = OwnerAssetCache.ResolveOptionalPath(arguments);
                if (qualifiedMode && assetCachePath == null)
                {
                    throw new InvalidDataException(
                        "qualified mode requires the exact owner asset cache");
                }
                if (assetCachePath != null)
                {
                    ownerAssetCache = OwnerAssetCache.OpenAndValidate(
                        assetCachePath,
                        ExpectedPackageHash,
                        ExpectedOwnerAssetClosureHash);
                    Debug.Log(
                        $"MC_ASSET_CACHE VERIFIED outputs={ownerAssetCache.OutputCount} " +
                        $"content={ownerAssetCache.ContentHash}");
                }
                session = OpenSessionWithAssetCache(
                    packagePath, closurePath, assetCachePath);
                LoadAndValidateExactWorld();
                if (ownerAssetCache != null)
                {
                    LoadAndValidateOwnerHudDescriptor();
                    ownerAudio = OwnerAudioPresentation.Load(
                        ownerAssetCache,
                        "audio/sound-bank-1-822.json",
                        "audio/music-channel-0-f.json",
                        transform,
                        session.CompleteAudioVoice);
                    Debug.Log(
                        "MC_AUDIO VERIFIED bank=1 quality=822 refs=31 " +
                        "music=C2GAME1-adlmidi-bank59.wav");
                }
                Debug.Log(
                    $"MC_PRESENTATION_MODE mode={(qualifiedMode ? "qualified" : "debug")} " +
                    $"ownerCache={(ownerAssetCache != null)}");

                var playthroughDisabled =
                    Environment.GetEnvironmentVariable("MC_PLAYTHROUGH_DISABLE") == "1";
                var playthroughRequested =
                    HasArgument(arguments, "-mc-record-playthrough");
                var normalHumanPlay =
                    !Application.isBatchMode &&
                    !AutomatedInput &&
                    !localAudioQualificationMode &&
                    !positionalAudioQualificationMode &&
                    !entityGallery;
                if (!playthroughDisabled && (playthroughRequested || normalHumanPlay))
                {
                    playthroughRecorder = PlaythroughRecorder.Start(
                        packagePath,
                        closurePath,
                        snapshot,
                        mission);
                    Debug.Log(
                        $"MC_PLAYTHROUGH_RECORD path={playthroughRecorder.DirectoryPath}");
                }

                if (HasArgument(arguments, "-mc-self-test"))
                {
                    RunHeadlessSelfTest();
                    return;
                }

                Application.targetFrameRate = 120;
                // remc2's default outer game-frame cadence. Every simulation
                // turn, creature update, spell lifetime and movement step is
                // measured against this clock.
                Time.fixedDeltaTime = 1f / AuthoritativeSimulationHz;
                hudCaptureRequested = !string.IsNullOrWhiteSpace(
                    Environment.GetEnvironmentVariable("MC_HUD_CAPTURE_PATH"));
                ConfigureWorld();
                ConfigureCameraAndCarpet();
                InitializeUnwrappedPose();
                ConfigureExactTerrainFrameComposite();
                ReadAuthoritativePose(true);
                if (localAudioQualificationMode)
                {
                    if (ownerAudio == null || audioCapture == null)
                    {
                        throw new InvalidDataException(
                            "local audio qualification requires the exact owner cache " +
                            "and Unity listener capture");
                    }
                    localAudioQualification = new LocalAudioQualification(
                        ownerAudio,
                        audioCapture,
                        session,
                        snapshot.generation,
                        ResetNativeGenerationForAudioQualification);
                    ReleaseMouse();
                }
                else if (positionalAudioQualificationMode)
                {
                    if (ownerAudio == null || audioCapture == null)
                    {
                        throw new InvalidDataException(
                            "positional audio qualification requires the exact owner " +
                            "cache and Unity listener capture");
                    }
                    positionalAudioQualification = new PositionalAudioQualification(
                        ownerAudio,
                        audioCapture,
                        session);
                    ReleaseMouse();
                }
                if (entityGallery) ConfigureEntityGallery();
                if (autoplay)
                {
                    autoplayInitialYaw = snapshot.yaw;
                    autoplayInitialAltitude = snapshot.z;
                    autoplayInitialFireEvents = fireEventsObserved;
                    ReleaseMouse();
                    Debug.Log(
                        $"MC_AUTOPLAY BEGIN mode={(qualifiedMode ? "qualified" : "debug")} " +
                        $"position={snapshot.x}," +
                        $"{snapshot.y},{snapshot.z} yaw={snapshot.yaw} " +
                        $"registry={entityRegistry.Count} debugEntities={debugEntities}");
                }
                else if (level1Autoplay)
                {
                    level1AutopilotDriver =
                        new Level1Autopilot(level1AutoplayPostgame);
                    ReleaseMouse();
                    Debug.Log(
                        $"MC_LEVEL1_AUTOPLAY BEGIN position={snapshot.x}," +
                        $"{snapshot.y},{snapshot.z} yaw={snapshot.yaw} " +
                        $"stage={mission.currentStage} " +
                        $"postgame={level1AutoplayPostgame}");
                }
                else
                {
                    CaptureMouse();
                }
            }
            catch (Exception exception)
            {
                fatalError = exception.ToString();
                playthroughRecorder?.RecordFatal(exception);
                enabled = false;
                Debug.LogError("MC_FATAL " + fatalError);
                // A player that cannot establish the explicit owner-data and
                // presentation contract must never remain as a frozen,
                // apparently runnable window. This is deliberately uniform
                // across batch and interactive player launches.
                DisposeSession();
                Application.Quit(2);
            }
        }

        private void FixedUpdate()
        {
            if (level1AutoplayFast) return;
            if (runtimeExitRequested || autoplayCapturePending) return;
            try
            {
                FixedUpdateCore(true);
            }
            catch (Exception exception)
            {
                runtimeExitRequested = true;
                fatalError = exception.ToString();
                playthroughRecorder?.RecordFatal(exception);
                Debug.LogError("MC_RUNTIME_FATAL " + fatalError);
                if (AutomatedInput || Application.isBatchMode)
                {
                    DisposeSession();
                    Application.Quit(3);
                }
                else
                {
                    enabled = false;
                }
            }
        }

        private void FixedUpdateCore(bool updatePresentation)
        {
            if (localAudioQualificationMode || positionalAudioQualificationMode) return;
            if (entityGallery) return;
            int flags;
            short selectedSpell;
            sbyte opcode = 0;
            sbyte argument1 = 0;
            sbyte argument2 = 0;
            if (level1Autoplay)
            {
                var command = level1AutopilotDriver.Next(
                    Level1Autopilot.Observe(
                        snapshot,
                        mission,
                        missionActors,
                        goats,
                        manaSpheres,
                        level1Castle));
                appliedRoll = command.Roll;
                appliedPitch = command.Pitch;
                flags = unchecked((byte)command.Flags);
                selectedSpell = command.RequestedSpell;
                opcode = command.Opcode;
                argument1 = command.Argument1;
                argument2 = command.Argument2;
                if (command.Phase != level1AutoplayPhase)
                {
                    level1AutoplayPhase = command.Phase;
                    Debug.Log(
                        $"MC_LEVEL1_AUTOPLAY PHASE tick={simTick} " +
                        $"stage={mission.currentStage} phase={command.Phase} " +
                        $"position={snapshot.x},{snapshot.y},{snapshot.z} " +
                        $"life={mission.playerLife} mana={mission.playerMana}");
                }
                else if (
                    command.Phase == "possess-mana" &&
                    simTick % 1_024 == 0)
                {
                    Debug.Log(
                        $"MC_LEVEL1_AUTOPLAY AIM tick={simTick} " +
                        $"distance={level1AutopilotDriver.LastAimDistance:F0} " +
                        $"yawError={level1AutopilotDriver.LastAimYawError} " +
                        $"pitchError={level1AutopilotDriver.LastAimPitchError} " +
                        $"input={appliedRoll},{appliedPitch},{flags} " +
                        $"position={snapshot.x},{snapshot.y},{snapshot.z}");
                }
            }
            else if (autoplay)
            {
                BuildAutoplayCommand(simTick, out appliedRoll, out appliedPitch, out flags);
                selectedSpell = requestedSpell;
                requestedSpell = NativeSim.NoRequestedSpell;
                if ((flags & 0x10) != 0)
                {
                    fireInputLatchedCount++;
                    Debug.Log($"MC_FIRE input_latched source=autoplay count={fireInputLatchedCount}");
                }
            }
            else
            {
                flags = spellMenuOpen ? 0 : ReadLiveMovementFlags();
                appliedRoll = Cursor.lockState == CursorLockMode.Locked
                    ? ClampToSByte(steeringRoll)
                    : (sbyte)0;
                appliedPitch = Cursor.lockState == CursorLockMode.Locked
                    ? ClampToSByte(steeringPitch)
                    : (sbyte)0;
                selectedSpell = requestedSpell;
                requestedSpell = NativeSim.NoRequestedSpell;
            }
            if (!level1Autoplay && !autoplay &&
                spellMenuCommands.TryDequeue(out var spellMenuCommand))
            {
                opcode = spellMenuCommand.Opcode;
                argument1 = spellMenuCommand.Argument1;
                argument2 = spellMenuCommand.Argument2;
            }

            if ((flags & 0x10) != 0)
            {
                fireFlagsSubmittedCount++;
                Debug.Log(
                    $"MC_FIRE flags_submitted tick={simTick} flags={flags} " +
                    $"count={fireFlagsSubmittedCount}");
            }
            if (autoplay)
            {
                AppendAutoplayCommand(appliedRoll, appliedPitch, unchecked((sbyte)flags));
            }
            snapshot = session.Step(
                appliedRoll,
                appliedPitch,
                unchecked((sbyte)flags),
                opcode: opcode,
                argument1: argument1,
                argument2: argument2,
                requestedSpell: selectedSpell);
            RefreshLiveTerrain();
            RefreshLiveGoats();
            RefreshLiveVillagers();
            RefreshLiveMission();
            playthroughRecorder?.RecordTick(
                simTick,
                opcode,
                argument1,
                argument2,
                appliedRoll,
                appliedPitch,
                unchecked((sbyte)flags),
                selectedSpell,
                snapshot,
                mission,
                level1SpellMenu,
                level1Palette,
                castleLifecycle,
                spellMenuOpen,
                CurrentSpeedSpellTicks(),
                goats?.Length ?? 0,
                manaSpheres?.Length ?? 0);
            if (updatePresentation)
            {
                RefreshFastForwardPresentation();
            }
            simTick++;
            if (level1Autoplay)
            {
                FinishLevel1AutoplayIfTerminal();
            }
            var terrainRouteIndex = Array.IndexOf(AutoplayCaptureTicks, simTick);
            if (autoplay && terrainFrameCompositeLive && terrainRouteIndex >= 0)
            {
                Debug.Log(
                    $"MC_TERRAIN_ROUTE_OBSERVED tick={simTick} " +
                    $"indexHash=0x{terrainFrameInfo.indexHash:x16} " +
                    $"coverageHash=0x{terrainFrameInfo.coverageHash:x16} " +
                    $"triangles={terrainFrameInfo.reflectedTriangles}+" +
                    $"{terrainFrameInfo.primaryMode5Triangles}+" +
                    $"{terrainFrameInfo.primaryMode26Triangles} " +
                    $"writes={terrainFrameInfo.terrainPixelWrites}");
                ValidateTerrainFrameGpuGolden(
                    $"route-{simTick}",
                    AutoplayTerrainReadbackBottomUpHashes[terrainRouteIndex],
                    AutoplayTerrainLogicalTopDownHashes[terrainRouteIndex],
                    false);
                qualifiedTerrainGpuRouteTicks.Add(simTick);
                terrainFrameRouteGpuValidated =
                    qualifiedTerrainGpuRouteTicks.Count ==
                    AutoplayCaptureTicks.Length;
                Require(
                    terrainFrameInfo.tick == (ulong)(simTick - 1) &&
                    terrainFrameInfo.indexHash ==
                        AutoplayTerrainIndexHashes[terrainRouteIndex] &&
                    terrainFrameInfo.coverageHash ==
                        AutoplayTerrainCoverageHashes[terrainRouteIndex] &&
                    terrainFrameInfo.reflectedTriangles ==
                        AutoplayTerrainReflected[terrainRouteIndex] &&
                    terrainFrameInfo.primaryMode5Triangles ==
                        AutoplayTerrainMode5[terrainRouteIndex] &&
                    terrainFrameInfo.primaryMode26Triangles ==
                        AutoplayTerrainMode26[terrainRouteIndex] &&
                    terrainFrameInfo.terrainPixelWrites ==
                        AutoplayTerrainWrites[terrainRouteIndex],
                    $"deterministic terrain route vector {simTick}");
                Require(!RenderSettings.fog, "Unity fog disabled on exact terrain frame");
                qualifiedTerrainRouteTicks.Add(simTick);
                Debug.Log(
                    $"MC_TERRAIN_FRAME_ROUTE tick={simTick} " +
                    $"renderTick={terrainFrameInfo.tick} " +
                    $"indexHash=0x{terrainFrameInfo.indexHash:x16} " +
                    $"coverageHash=0x{terrainFrameInfo.coverageHash:x16} " +
                    $"triangles={terrainFrameInfo.reflectedTriangles}+" +
                    $"{terrainFrameInfo.primaryMode5Triangles}+" +
                    $"{terrainFrameInfo.primaryMode26Triangles} " +
                    $"writes={terrainFrameInfo.terrainPixelWrites}");
            }
            if (autoplay && terrainRouteIndex >= 0)
            {
                Require(
                    !autoplayCapturePending &&
                    !capturedAutoplayTicks.Contains(simTick),
                    $"autoplay capture checkpoint {simTick} is unique");
                autoplayCapturePending = true;
                autoplayCaptureTick = simTick;
                StartCoroutine(CaptureAutoplayFrameAtEndOfFrame(simTick));
            }
            if (simTick == 40 || simTick == 200) ownerAudio?.LogProbe(simTick);

            if (autoplay)
            {
                autoplayTurnObserved |= snapshot.yaw != autoplayInitialYaw;
                autoplayAltitudeChangeObserved |= snapshot.z != autoplayInitialAltitude;
                if (simTick % 40 == 0)
                {
                    Debug.Log(
                        $"MC_AUTOPLAY tick={simTick} input={appliedRoll},{appliedPitch}," +
                        $"{flags} position={snapshot.x},{snapshot.y},{snapshot.z} " +
                        $"yaw={snapshot.yaw} speed={snapshot.actualSpeed} " +
                        $"events={snapshot.eventHead} resolution={movementResolution}");
                }
            }
        }

        private void Update()
        {
            if (localAudioQualificationMode || positionalAudioQualificationMode) return;
            if (level1AutoplayFast && !runtimeExitRequested)
            {
                try
                {
                    const int ticksPerRenderedFrame = 256;
                    for (var tick = 0;
                         tick < ticksPerRenderedFrame && !runtimeExitRequested;
                         tick++)
                    {
                        FixedUpdateCore(false);
                    }
                    if (!runtimeExitRequested)
                    {
                        RefreshFastForwardPresentation();
                    }
                }
                catch (Exception exception)
                {
                    runtimeExitRequested = true;
                    fatalError = exception.ToString();
                    playthroughRecorder?.RecordFatal(exception);
                    Debug.LogError("MC_RUNTIME_FATAL " + fatalError);
                    DisposeSession();
                    Application.Quit(3);
                }
                return;
            }
            var controlHeld =
                Input.GetKey(KeyCode.LeftControl) ||
                Input.GetKey(KeyCode.RightControl);
            var spellLogical = ScreenToSpellLogical(Input.mousePosition);
            if (!AutomatedInput && !spellMenuOpen &&
                (Input.GetKeyDown(KeyCode.LeftControl) ||
                 Input.GetKeyDown(KeyCode.RightControl)))
            {
                OpenSpellMenu(spellLogical.x, spellLogical.y);
            }
            if (!AutomatedInput && spellMenuOpen)
            {
                ProcessSpellMenuInput(
                    spellLogical.x,
                    spellLogical.y,
                    Input.GetMouseButtonDown(0),
                    Input.GetMouseButton(0),
                    Input.GetMouseButtonUp(0),
                    Input.GetMouseButtonDown(1),
                    Input.GetMouseButton(1),
                    Input.GetMouseButtonUp(1),
                    Input.GetKey(KeyCode.LeftShift) ||
                    Input.GetKey(KeyCode.RightShift));
                if (!controlHeld &&
                    !Input.GetMouseButton(0) &&
                    !Input.GetMouseButton(1))
                {
                    CloseSpellMenu(true);
                }
            }
            else if (!AutomatedInput && Cursor.lockState == CursorLockMode.Locked)
            {
                steeringRoll = Mathf.Clamp(
                    steeringRoll + Input.GetAxisRaw("Mouse X") * MouseDeflectionGain,
                    sbyte.MinValue,
                    sbyte.MaxValue);
                steeringPitch = Mathf.Clamp(
                    steeringPitch - Input.GetAxisRaw("Mouse Y") * MouseDeflectionGain,
                    sbyte.MinValue,
                    sbyte.MaxValue);
                var keyboardStep = KeyboardDeflectionRate * Time.unscaledDeltaTime;
                if (Input.GetKey(KeyCode.J)) steeringRoll -= keyboardStep;
                if (Input.GetKey(KeyCode.L)) steeringRoll += keyboardStep;
                if (Input.GetKey(KeyCode.I)) steeringPitch -= keyboardStep;
                if (Input.GetKey(KeyCode.K)) steeringPitch += keyboardStep;
                steeringRoll = Mathf.Clamp(steeringRoll, sbyte.MinValue, sbyte.MaxValue);
                steeringPitch = Mathf.Clamp(steeringPitch, sbyte.MinValue, sbyte.MaxValue);
                var cycleBackward =
                    Input.GetKey(KeyCode.LeftAlt) ||
                    Input.GetKey(KeyCode.RightAlt);
                var cycleForward =
                    Input.GetKey(KeyCode.LeftShift) ||
                    Input.GetKey(KeyCode.RightShift);
                if (Input.GetMouseButtonDown(0))
                {
                    if (cycleBackward || cycleForward)
                    {
                        QueueSpellCycle(false, cycleBackward);
                    }
                    else
                    {
                        LatchSpellCastInput(false);
                    }
                }
                if (Input.GetMouseButtonDown(1))
                {
                    if (cycleBackward || cycleForward)
                    {
                        QueueSpellCycle(true, cycleBackward);
                    }
                    else
                    {
                        LatchSpellCastInput(true);
                    }
                }
                if (Input.GetKeyDown(KeyCode.C) || Input.GetMouseButtonDown(2))
                {
                    CenterSteering();
                }
            }
            if (!AutomatedInput && !spellMenuOpen && Input.GetKeyDown(KeyCode.Space))
            {
                // Keyboard fire remains available while the mouse is released;
                // cursor capture is a presentation concern, not simulation input.
                selectedSpellRequested = true;
                fireInputLatchedCount++;
                Debug.Log(
                    $"MC_FIRE input_latched source=selected-space " +
                    $"count={fireInputLatchedCount}");
            }
            if (!AutomatedInput && Input.GetKeyDown(KeyCode.Escape))
            {
                if (spellMenuOpen)
                {
                    CloseSpellMenu(false);
                }
                else
                {
                    ReleaseMouse();
                }
            }
            if (!AutomatedInput && Input.GetMouseButtonDown(0) &&
                !spellMenuOpen &&
                Cursor.lockState != CursorLockMode.Locked) CaptureMouse();
            if (Input.GetKeyDown(KeyCode.F11)) Screen.fullScreen = !Screen.fullScreen;
            if (Input.GetKeyDown(KeyCode.F10)) Application.Quit();
            if (Input.GetKeyDown(KeyCode.R)) ResetSession();
        }

        private void RefreshFastForwardPresentation()
        {
            session.EvaluateGoatDrawAdmission(snapshot.generation, snapshot.tick);
            movementResolution = ResolutionName(snapshot.lastResolution);
            RefreshLivePlayerRow();
            DrainEvents();
            ReadAuthoritativePose(false);
            UpdateExactTerrainGrid();
        }

        private void LateUpdate()
        {
            if (localAudioQualificationMode)
            {
                if (localAudioQualificationExitRequested) return;
                try
                {
                    localAudioQualification.Tick(Time.unscaledDeltaTime);
                    if (localAudioQualification.Completed)
                    {
                        localAudioQualificationExitRequested = true;
                        var passed = localAudioQualification.Passed;
                        if (!passed)
                        {
                            Debug.LogError(
                                "MC_AUDIO_LOCAL_QUAL FAILURE_SUMMARY " +
                                localAudioQualification.FailureSummary);
                        }
                        DisposeSession();
                        Application.Quit(passed ? 0 : 5);
                    }
                }
                catch (Exception exception)
                {
                    localAudioQualificationExitRequested = true;
                    Debug.LogError("MC_AUDIO_LOCAL_QUAL FATAL " + exception);
                    DisposeSession();
                    Application.Quit(5);
                }
                return;
            }
            if (positionalAudioQualificationMode)
            {
                if (positionalAudioQualificationExitRequested) return;
                try
                {
                    positionalAudioQualification.Tick(Time.unscaledDeltaTime);
                    if (positionalAudioQualification.Completed)
                    {
                        positionalAudioQualificationExitRequested = true;
                        var passed = positionalAudioQualification.Passed;
                        if (!passed)
                        {
                            Debug.LogError(
                                "MC_AUDIO_POSITIONAL_QUAL FAILURE_SUMMARY " +
                                positionalAudioQualification.FailureSummary);
                        }
                        DisposeSession();
                        Application.Quit(passed ? 0 : 6);
                    }
                }
                catch (Exception exception)
                {
                    positionalAudioQualificationExitRequested = true;
                    Debug.LogError("MC_AUDIO_POSITIONAL_QUAL FATAL " + exception);
                    DisposeSession();
                    Application.Quit(6);
                }
                return;
            }
            ownerAudio?.Tick(Time.unscaledDeltaTime);
            if (presentationRig == null) return;
            presentationRig.SetPositionAndRotation(targetPosition, targetRotation);
            if (autoplay) MeasureCarpetTerrainClearance();

            if (entityGallery)
            {
                entityGalleryFrames++;
                var capturePath =
                    Environment.GetEnvironmentVariable("MC_ENTITY_GALLERY_CAPTURE_PATH");
                if (!entityGalleryCaptured && entityGalleryFrames >= 3 &&
                    !string.IsNullOrWhiteSpace(capturePath))
                {
                    var absolute = Path.GetFullPath(capturePath);
                    Directory.CreateDirectory(Path.GetDirectoryName(absolute));
                    ScreenCapture.CaptureScreenshot(absolute);
                    entityGalleryCaptured = true;
                    Debug.Log($"MC_ENTITY_GALLERY CAPTURE path={absolute}");
                }
                if (autoplayQuit && entityGalleryFrames >= 10)
                {
                    var physicsClean =
                        FindObjectsByType<Rigidbody>(FindObjectsSortMode.None).Length == 0 &&
                        FindObjectsByType<Collider>(FindObjectsSortMode.None).Length == 0;
                    var passed = entityGalleryItems.Count == 8 &&
                                 entitySpriteMaterials.Count >= 8 &&
                                 physicsClean &&
                                 (string.IsNullOrWhiteSpace(capturePath) ||
                                  entityGalleryCaptured);
                    Debug.Log(
                        $"MC_ENTITY_GALLERY {(passed ? "PASS" : "FAIL")} " +
                        $"items={entityGalleryItems.Count} " +
                        $"selectorMaterials={entitySpriteMaterials.Count} " +
                        $"captured={entityGalleryCaptured} physicsClean={physicsClean}");
                    DisposeSession();
                    Application.Quit(passed ? 0 : 4);
                }
                return;
            }

            if (hudCaptureRequested && !hudCaptured && simTick >= 10)
            {
                var capturePath = Path.GetFullPath(
                    Environment.GetEnvironmentVariable("MC_HUD_CAPTURE_PATH"));
                Directory.CreateDirectory(Path.GetDirectoryName(capturePath));
                ScreenCapture.CaptureScreenshot(capturePath);
                hudCaptured = true;
                Debug.Log($"MC_OWNER_HUD CAPTURE path={capturePath} tick={simTick}");
            }
            var hudComparisonPath =
                Environment.GetEnvironmentVariable("MC_HUD_COMPARISON_CAPTURE_PATH");
            if (
                !hudComparisonCaptured &&
                !string.IsNullOrWhiteSpace(hudComparisonPath) &&
                simTick >= 145)
            {
                var absolute = Path.GetFullPath(hudComparisonPath);
                Directory.CreateDirectory(Path.GetDirectoryName(absolute));
                ScreenCapture.CaptureScreenshot(absolute);
                hudComparisonCaptured = true;
                Debug.Log(
                    $"MC_OWNER_HUD COMPARISON_CAPTURE path={absolute} tick={simTick}");
            }
            var stageFourCapturePath =
                Environment.GetEnvironmentVariable("MC_LEVEL1_STAGE4_CAPTURE_PATH");
            var activeFireflies = 0;
            var nearestFireflyDistanceSquared = long.MaxValue;
            if (mission.currentStage == 4)
            {
                foreach (var actor in missionActors)
                {
                    if (actor.kind != 2 || actor.active == 0)
                        continue;
                    activeFireflies++;
                    var deltaX = ModularDelta(actor.x, snapshot.x);
                    var deltaY = ModularDelta(actor.y, snapshot.y);
                    var deltaZ = actor.z - snapshot.z;
                    var distanceSquared =
                        (long)deltaX * deltaX +
                        (long)deltaY * deltaY +
                        (long)deltaZ * deltaZ;
                    nearestFireflyDistanceSquared =
                        Math.Min(nearestFireflyDistanceSquared, distanceSquared);
                }
            }
            if (
                level1Autoplay &&
                !level1StageFourCaptured &&
                !string.IsNullOrWhiteSpace(stageFourCapturePath) &&
                mission.currentStage == 4 &&
                nearestFireflyDistanceSquared <= 2_500L * 2_500L)
            {
                level1StageFourCaptureFrames++;
                if (level1StageFourCaptureFrames >= 2)
                {
                    var absolute = Path.GetFullPath(stageFourCapturePath);
                    Directory.CreateDirectory(Path.GetDirectoryName(absolute));
                    ScreenCapture.CaptureScreenshot(absolute);
                    level1StageFourCaptured = true;
                    Debug.Log(
                        $"MC_LEVEL1_STAGE4_CAPTURE path={absolute} tick={simTick} " +
                        $"activeFireflies={activeFireflies} " +
                        $"nearestDistanceSquared={nearestFireflyDistanceSquared} " +
                        $"nativeFireflies={((terrainFrameInfo.flags & NativeSim.TerrainFrameFirefliesComposited) != 0).ToString().ToLowerInvariant()}");
                }
            }

            if (!autoplay) return;
            if (autoplayQuit && simTick >= AutoplayFinalTick)
            {
                autoplayFinalWaitFrames++;
                var captureDirectory =
                    Environment.GetEnvironmentVariable("MC_AUTOPLAY_CAPTURE_DIR") ?? "/tmp";
                var capturesReady =
                    capturedAutoplayTicks.Count == AutoplayCaptureTicks.Length &&
                    magentaCleanAutoplayTicks.Count == AutoplayCaptureTicks.Length &&
                    string.IsNullOrEmpty(autoplayCaptureFailure) &&
                    Array.TrueForAll(
                        AutoplayCaptureTicks,
                        tick =>
                            File.Exists(Path.Combine(
                                captureDirectory,
                                $"magiccarpet-autoplay-{tick:D3}.png")) &&
                            File.Exists(Path.Combine(
                                captureDirectory,
                                $"magiccarpet-autoplay-{tick:D3}.json")));
                if (!capturesReady && autoplayFinalWaitFrames < 120)
                    return;
                var physicsClean =
                    FindObjectsByType<Rigidbody>(FindObjectsSortMode.None).Length == 0 &&
                    FindObjectsByType<Collider>(FindObjectsSortMode.None).Length == 0;
                var ownerSpriteCount =
                    entityRegistry.FindAll(marker => marker.OwnerSprite).Count;
                var presentationClean =
                    qualifiedMode
                        ? ownerSpriteCount == 64
                        : ownerAssetCache == null ||
                          ownerSpriteCount == (debugEntities ? 83 : 64);
                var terrainFrameClean =
                    !qualifiedMode ||
                    (
                        terrainFrameCompositeLive &&
                        terrainFrameGpuValidated &&
                        terrainFrameRouteGpuValidated &&
                        qualifiedTerrainRouteTicks.Count ==
                            AutoplayCaptureTicks.Length &&
                        qualifiedTerrainGpuRouteTicks.Count ==
                            AutoplayCaptureTicks.Length &&
                        terrainFrameInfo.generation == renderCameraState.generation &&
                        terrainFrameInfo.tick == renderCameraState.tick &&
                        terrainFrameInfo.indexBytes == NativeSim.TerrainFramePixels &&
                        terrainFrameInfo.coverageBytes == NativeSim.TerrainFramePixels &&
                        (terrainFrameInfo.flags &
                         (
                             NativeSim.TerrainFrameGoatsComposited |
                             NativeSim.TerrainFrameArchersComposited |
                             NativeSim.TerrainFrameFirefliesComposited |
                             NativeSim.TerrainFrameCombatComposited |
                             NativeSim.TerrainFrameObjectivePropsComposited |
                             NativeSim.TerrainFrameEconomyComposited |
                             NativeSim.TerrainFrameReflectedSpritesComposited |
                             NativeSim.TerrainFrameSpriteShadowsComposited |
                             NativeSim.TerrainFrameFrozenActorsComposited
                         )) ==
                        (
                            NativeSim.TerrainFrameGoatsComposited |
                            NativeSim.TerrainFrameArchersComposited |
                            NativeSim.TerrainFrameFirefliesComposited |
                            NativeSim.TerrainFrameCombatComposited |
                            NativeSim.TerrainFrameObjectivePropsComposited |
                            NativeSim.TerrainFrameEconomyComposited |
                            NativeSim.TerrainFrameReflectedSpritesComposited |
                            NativeSim.TerrainFrameSpriteShadowsComposited |
                            NativeSim.TerrainFrameFrozenActorsComposited
                        )
                    );
                var carpetOverlayClean =
                    qualifiedMode
                        ? carpetVisual == null && carpetOverlayCamera == null
                        : carpetOverlayCamera != null &&
                          (flightCamera.cullingMask & (1 << CarpetOverlayLayer)) == 0 &&
                          carpetOverlayCamera.cullingMask == (1 << CarpetOverlayLayer) &&
                          carpetOverlayCamera.depth > flightCamera.depth;
                var firePresentationClean =
                    !qualifiedMode || fireVisualSpawnedCount == 0;
                var diagnosticObjectsClean =
                    !qualifiedMode ||
                    (
                        terrainDepthOnlyMaterial == null &&
                        fireballMaterial == null &&
                        FindObjectsByType<Light>(FindObjectsSortMode.None).Length == 0 &&
                        terrainTiles.TrueForAll(tile => {
                            var renderer = tile.GetComponent<MeshRenderer>();
                            return renderer != null && !renderer.enabled;
                        })
                    );
                var passed = autoplayTurnObserved && autoplayAltitudeChangeObserved &&
                             fireEventsObserved > autoplayInitialFireEvents &&
                             entityRegistry.Count == 177 && physicsClean &&
                             presentationClean && terrainFrameClean &&
                             carpetOverlayClean && firePresentationClean &&
                             diagnosticObjectsClean && capturesReady;
                var autoplayMarker = qualifiedMode
                    ? "MC_QUALIFIED_AUTOPLAY"
                    : "MC_DEBUG_AUTOPLAY";
                Debug.Log(
                    $"{autoplayMarker} {(passed ? "PASS" : "FAIL")} tick={simTick} " +
                    $"turn={autoplayTurnObserved} altitude={autoplayAltitudeChangeObserved} " +
                    $"fire={fireEventsObserved - autoplayInitialFireEvents} " +
                    $"registry={entityRegistry.Count} ownerSprites={ownerSpriteCount} " +
                    $"selectorMaterials={entitySpriteMaterials.Count} " +
                    $"presentationClean={presentationClean} physicsClean={physicsClean} " +
                    $"terrainFrameClean={terrainFrameClean} " +
                    $"carpetOverlayClean={carpetOverlayClean} " +
                    $"firePresentationClean={firePresentationClean} " +
                    $"diagnosticObjectsClean={diagnosticObjectsClean} " +
                    $"carpetWorldClearanceDiagnostic=" +
                    $"{minimumCarpetTerrainClearanceSource:F3} " +
                    $"captures={capturedAutoplayTicks.Count}/{AutoplayCaptureTicks.Length} " +
                    $"magentaClean={magentaCleanAutoplayTicks.Count}/" +
                    $"{AutoplayCaptureTicks.Length} " +
                    $"captureFilesReady={capturesReady}");
                DisposeSession();
                Application.Quit(passed ? 0 : 3);
            }
        }

        private IEnumerator CaptureAutoplayFrameAtEndOfFrame(int checkpointTick)
        {
            yield return new WaitForEndOfFrame();
            Texture2D frame = null;
            try
            {
                Require(
                    autoplayCapturePending &&
                    autoplayCaptureTick == checkpointTick &&
                    simTick == checkpointTick,
                    $"autoplay capture remained frozen at checkpoint {checkpointTick}");
                if (qualifiedMode)
                {
                    Require(
                        terrainFrameInfo.tick == (ulong)(checkpointTick - 1),
                        $"autoplay terrain frame matches checkpoint {checkpointTick}");
                }
                frame = ScreenCapture.CaptureScreenshotAsTexture();
                Require(frame != null, "full-frame screenshot texture");
                Require(
                    frame.width == 1600 && frame.height == 900,
                    $"qualified full-frame capture is exactly 1600x900, got " +
                    $"{frame.width}x{frame.height}");

                var directory = ResolveAutoplayCaptureDirectory();
                var stem = Path.Combine(
                    directory, $"magiccarpet-autoplay-{checkpointTick:D3}");
                var pngPath =
                    AudioQualificationCapture.ResolveQualifiedCapturePath(stem + ".png");
                var manifestPath =
                    AudioQualificationCapture.ResolveQualifiedCapturePath(stem + ".json");
                var png = frame.EncodeToPNG();
                var rgba = frame.GetRawTextureData<byte>();
                var magentaPixels = 0;
                for (var offset = 0; offset < rgba.Length; offset += 4)
                {
                    if (
                        rgba[offset] == 255 &&
                        rgba[offset + 1] == 0 &&
                        rgba[offset + 2] == 255)
                        magentaPixels++;
                }
                Require(
                    magentaPixels == 0,
                    $"full-frame checkpoint {checkpointTick} contains no magenta pixels");
                Require(
                    eventCursor == snapshot.eventHead,
                    $"event cursor reaches checkpoint head {checkpointTick}");
                var eventCount = snapshot.eventHead >= autoplayEventRangeStart
                    ? snapshot.eventHead - autoplayEventRangeStart + 1
                    : 0;
                var routeIndex = Array.IndexOf(AutoplayCaptureTicks, checkpointTick);
                Require(routeIndex >= 0, "registered graphical route checkpoint");

                var manifest = new FullFrameCaptureManifest {
                    schema = "mc2-private-qualified-full-frame-v2",
                    presentation_mode = qualifiedMode ? "qualified" : "debug",
                    evidence_authority = "synthetic-native-unity-regression",
                    oracle_status = "no-dynamic-remc2-route-capture",
                    checkpoint_tick = checkpointTick,
                    last_command_index = checkpointTick - 1,
                    native_render_tick = terrainFrameInfo.tick,
                    exact_checkpoint = simTick == checkpointTick,
                    command_encoding = "mc2-player-command-10-byte-le/v1",
                    command_prefix_sha256 =
                        Sha256Hex(autoplayCommandBytes.ToArray()),
                    snapshot = SnapshotEvidenceFrom(
                        snapshot,
                        presentationState.sourceTurn),
                    events = new EventRangeEvidence {
                        content_schema =
                            "mc2-native-event-logical-content-le/sha256-v1",
                        lifecycle_schema =
                            "mc2-native-event-logical-lifecycle-le/sha256-v1",
                        first_sequence = autoplayEventRangeStart,
                        last_sequence = snapshot.eventHead,
                        count = eventCount,
                        content_sha256 =
                            Sha256Hex(autoplayEventContentBytes.ToArray()),
                        lifecycle_sha256 =
                            Sha256Hex(autoplayEventLifecycleBytes.ToArray()),
                        cursor_equals_head = eventCursor == snapshot.eventHead
                    },
                    presentation = PresentationEvidenceFrom(presentationState),
                    render_camera = RenderCameraEvidenceFrom(renderCameraState),
                    terrain = TerrainFrameEvidenceFrom(terrainFrameInfo),
                    terrain_gpu_bottom_up_fnv1a64 =
                        $"{AutoplayTerrainReadbackBottomUpHashes[routeIndex]:x16}",
                    terrain_logical_top_down_fnv1a64 =
                        $"{AutoplayTerrainLogicalTopDownHashes[routeIndex]:x16}",
                    width = frame.width,
                    height = frame.height,
                    package_sha256 = ExpectedPackageHash,
                    closure_sha256 = ExpectedClosureHash,
                    owner_cache_content_sha256 = ownerAssetCache?.ContentHash ?? "none",
                    native_plugin_sha256 = ExpectedNativePluginHash,
                    unity_version = Application.unityVersion,
                    unity_build_guid = Application.buildGUID,
                    graphics_api = SystemInfo.graphicsDeviceType.ToString(),
                    graphics_device = SystemInfo.graphicsDeviceName,
                    png_sha256 = Sha256Hex(png),
                    rgba_sha256 = Sha256Hex(rgba),
                    magenta_pixels = magentaPixels,
                    magenta_gate_pass = magentaPixels == 0
                };
                File.WriteAllBytes(pngPath, png);
                File.WriteAllText(
                    manifestPath,
                    JsonUtility.ToJson(manifest, true) + "\n");
                capturedAutoplayTicks.Add(checkpointTick);
                magentaCleanAutoplayTicks.Add(checkpointTick);
                autoplayEventRangeStart = snapshot.eventHead + 1;
                autoplayEventContentBytes.SetLength(0);
                autoplayEventLifecycleBytes.SetLength(0);
                Debug.Log(
                    $"MC_AUTOPLAY FULL_FRAME PASS checkpointTick={checkpointTick} " +
                    $"nativeRenderTick={terrainFrameInfo.tick} " +
                    $"size={frame.width}x{frame.height} magenta={magentaPixels} " +
                    $"pngSha256={manifest.png_sha256} path={pngPath}");
            }
            catch (Exception exception)
            {
                autoplayCaptureFailure = exception.ToString();
                runtimeExitRequested = true;
                Debug.LogError(
                    $"MC_AUTOPLAY FULL_FRAME FAIL checkpointTick={checkpointTick} " +
                    autoplayCaptureFailure);
                DisposeSession();
                Application.Quit(3);
            }
            finally
            {
                if (frame != null) Destroy(frame);
                autoplayCaptureTick = -1;
                autoplayCapturePending = false;
            }
        }

        private static string ResolveAutoplayCaptureDirectory()
        {
            var requested =
                Environment.GetEnvironmentVariable("MC_AUTOPLAY_CAPTURE_DIR") ?? "/tmp";
            var directory = Path.GetFullPath(requested);
            var temporaryRoot = Path.GetFullPath(Path.GetTempPath())
                .TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
            Require(
                (directory + Path.DirectorySeparatorChar).StartsWith(
                    temporaryRoot + Path.DirectorySeparatorChar,
                    StringComparison.Ordinal),
                "private full-frame captures stay in system temporary storage");
            Directory.CreateDirectory(directory);
            AudioQualificationCapture.ResolveQualifiedCapturePath(
                Path.Combine(directory, ".mc-full-frame-path-probe"));
            return directory;
        }

        private void AppendAutoplayCommand(sbyte roll, sbyte pitch, sbyte flags)
        {
            AppendAutoplayCommand(autoplayCommandBytes, roll, pitch, flags);
        }

        private static void AppendAutoplayCommand(
            MemoryStream target,
            sbyte roll,
            sbyte pitch,
            sbyte flags)
        {
            target.WriteByte(0);
            target.WriteByte(0);
            target.WriteByte(0);
            target.WriteByte(unchecked((byte)roll));
            target.WriteByte(unchecked((byte)pitch));
            target.WriteByte(unchecked((byte)flags));
            target.WriteByte(0);
            target.WriteByte(0);
            target.WriteByte(0);
            target.WriteByte(0);
        }

        private static void AppendCanonicalEvent(
            MemoryStream content,
            MemoryStream lifecycle,
            NativeSim.NativeEvent nativeEvent)
        {
            using (var writer = new BinaryWriter(
                content, Encoding.UTF8, leaveOpen: true))
            {
                WriteCanonicalEventFields(writer, nativeEvent, includeGeneration: false);
            }
            using (var writer = new BinaryWriter(
                lifecycle, Encoding.UTF8, leaveOpen: true))
            {
                WriteCanonicalEventFields(writer, nativeEvent, includeGeneration: true);
            }
        }

        private static void WriteCanonicalEventFields(
            BinaryWriter writer,
            NativeSim.NativeEvent nativeEvent,
            bool includeGeneration)
        {
            if (includeGeneration) writer.Write(nativeEvent.generation);
            writer.Write(nativeEvent.sequence);
            writer.Write(nativeEvent.stateHash);
            writer.Write(nativeEvent.kind);
            writer.Write(nativeEvent.code);
            writer.Write(nativeEvent.entityIndex);
            writer.Write(nativeEvent.playerIndex);
            writer.Write(nativeEvent.value0);
            writer.Write(nativeEvent.value1);
            writer.Write(nativeEvent.assetRefIndex);
        }

        private static SnapshotEvidence SnapshotEvidenceFrom(
            NativeSim.SessionSnapshot value,
            int sourceTurn)
        {
            return new SnapshotEvidence {
                generation = value.generation,
                tick = value.tick,
                event_head = value.eventHead,
                state_hash_fnv1a64 = $"{value.stateHash:x16}",
                conformance_hash_fnv1a64 = $"{value.conformanceHash:x16}",
                entity_index = value.entityIndex,
                x = value.x,
                y = value.y,
                z = value.z,
                yaw = value.yaw,
                display_pitch = value.displayPitch,
                effective_pitch = value.effectivePitch,
                roll_accumulator = value.rollAccumulator,
                target_speed = value.targetSpeed,
                actual_speed = value.actualSpeed,
                strafe_speed = value.strafeSpeed,
                phase = value.phase,
                last_resolution = value.lastResolution,
                profile = value.profile,
                source_turn = sourceTurn,
                fire_sequence = value.fireSequence
            };
        }

        private static PresentationEvidence PresentationEvidenceFrom(
            NativeSim.PresentationState value)
        {
            return new PresentationEvidence {
                generation = value.generation,
                tick = value.tick,
                source_turn = value.sourceTurn,
                camera_x = value.cameraX,
                camera_y = value.cameraY,
                camera_z = value.cameraZ,
                camera_yaw = value.cameraYaw,
                camera_pitch = value.cameraPitch,
                camera_roll = value.cameraRoll,
                camera_fov = value.cameraFov,
                authority = value.cameraAuthority,
                flags = value.cameraFlags
            };
        }

        private static RenderCameraEvidence RenderCameraEvidenceFrom(
            NativeSim.RenderCameraState value)
        {
            return new RenderCameraEvidence {
                generation = value.generation,
                tick = value.tick,
                source_turn = value.sourceTurn,
                history_x = value.historyX,
                history_y = value.historyY,
                eye_z = value.eyeZ,
                yaw = value.yaw,
                pitch = value.pitch,
                roll = value.roll,
                fov = value.fov,
                slope_target_x = value.slopeTargetX,
                slope_target_y = value.slopeTargetY,
                render_offset_x = value.renderOffsetX,
                render_offset_y = value.renderOffsetY,
                authority = value.authority,
                flags = value.flags
            };
        }

        private static TerrainFrameEvidence TerrainFrameEvidenceFrom(
            NativeSim.TerrainFrameInfo value)
        {
            return new TerrainFrameEvidence {
                generation = value.generation,
                tick = value.tick,
                width = value.width,
                height = value.height,
                stride = value.stride,
                viewport_x = value.viewportX,
                viewport_y = value.viewportY,
                viewport_width = value.viewportWidth,
                viewport_height = value.viewportHeight,
                index_hash_fnv1a64 = $"{value.indexHash:x16}",
                coverage_hash_fnv1a64 = $"{value.coverageHash:x16}",
                reflected_triangles = value.reflectedTriangles,
                primary_mode5_triangles = value.primaryMode5Triangles,
                primary_mode26_triangles = value.primaryMode26Triangles,
                terrain_pixel_writes = value.terrainPixelWrites
            };
        }

        private static string Sha256Hex(byte[] bytes)
        {
            using var sha256 = SHA256.Create();
            return BitConverter.ToString(sha256.ComputeHash(bytes))
                .Replace("-", string.Empty)
                .ToLowerInvariant();
        }

        private static string Sha256Hex(Unity.Collections.NativeArray<byte> bytes)
        {
            return Sha256Hex(bytes.ToArray());
        }

        private void LoadAndValidateExactWorld()
        {
            worldInfo = session.WorldInfo();
            snapshot = session.Snapshot();
            var packageHash = session.WorldString(NativeSim.WorldPackageHash);
            var closureHash = session.WorldString(NativeSim.WorldClosureHash);
            Require(worldInfo.externalLevel == 1 && worldInfo.internalLevel == 0, "world 1/0");
            Require(worldInfo.mapType == 1 && worldInfo.profile == 0, "Night/outdoor profile");
            Require(worldInfo.entityCount == 1000, "1000 entities");
            Require(worldInfo.activeEntityCount == 177, "177 active entities");
            Require(worldInfo.playerCount == 8, "8 players");
            Require(
                worldInfo.assetRefCount == (ownerAssetCache == null ? 76u : 109u),
                "76 frame refs plus 32 sound and one music owner-cache refs");
            Require(worldInfo.terrainWidth == 256 && worldInfo.terrainHeight == 256, "256x256 terrain");
            Require(worldInfo.resetStateHash == ExpectedResetHash, "reset state hash");
            Require(
                worldInfo.resetConformanceHash == ExpectedResetConformanceHash,
                "package reset conformance hash");
            Require(packageHash == ExpectedPackageHash, "package hash");
            Require(closureHash == ExpectedClosureHash, "closure hash");
            Require(snapshot.stateHash == ExpectedResetHash, "snapshot reset hash");
            Require(
                snapshot.conformanceHash == ExpectedSessionConformanceHash,
                "Level 1 session-audio conformance hash");

            terrainState = session.TerrainState();
            Require(
                terrainState.generation == snapshot.generation &&
                terrainState.tick == snapshot.tick,
                "initial live terrain matches atomic flight snapshot");
            terrainTypes = session.BytePlane(NativeSim.TerrainTypes);
            groundHeights = session.BytePlane(NativeSim.TerrainHeight);
            terrainShade = session.BytePlane(NativeSim.TerrainShade);
            terrainAngles = session.BytePlane(NativeSim.TerrainAngle);
            secondaryHeights = session.BytePlane(NativeSim.TerrainSecondary);
            terrainEntityIndices = session.EntityIndexPlane();
            Require(
                terrainTypes.Length == 65536 && groundHeights.Length == 65536 &&
                terrainShade.Length == 65536 && terrainAngles.Length == 65536 &&
                secondaryHeights.Length == 65536 && terrainEntityIndices.Length == 65536,
                "all six 65536-cell terrain planes");
            Require(
                TerrainHash(
                    terrainTypes,
                    groundHeights,
                    terrainShade,
                    terrainAngles,
                    secondaryHeights,
                    terrainEntityIndices) == terrainState.terrainHash,
                "initial live terrain canonical hash");
            var terrainSelectorCoverage = new HashSet<byte>(terrainTypes);
            byte maximumTerrainSelector = 0;
            foreach (var selector in terrainSelectorCoverage)
            {
                if (selector > maximumTerrainSelector) maximumTerrainSelector = selector;
            }
            Require(
                terrainSelectorCoverage.SetEquals(ExpectedLevel1TerrainSelectors) &&
                terrainSelectorCoverage.Count == 65 &&
                maximumTerrainSelector == 139,
                "exact oracle-matched Level 1 BL32N selector set");
            Debug.Log(
                $"MC_TERRAIN_SELECTORS VERIFIED unique={terrainSelectorCoverage.Count} " +
                $"max={maximumTerrainSelector} bank=152");

            objectives = session.Objectives();
            Require(objectives.Length == 8 && snapshot.objectiveCount == 8, "8 objective rows");
            foreach (var objective in objectives)
            {
                Require(objective.authority == NativeSim.FrozenObserved, "frozen objective row");
            }
            entities = session.Entities();
            var active = 0;
            foreach (var entity in entities)
            {
                if (entity.active != 0) active++;
            }
            Require(active == 177, "177 active entity rows");
            Require(entities[152].authority == NativeSim.PlayerFlightLive, "entity 152 live player");
            Require(
                entities[153].authority == NativeSim.SpellActivationLive,
                "entity 153 spell activation authority");
            goatOrdinalByEntity = new int[entities.Length];
            Array.Fill(goatOrdinalByEntity, -1);
            RefreshLiveGoats();
            RefreshLiveMission();
            for (var ordinal = 0; ordinal < goats.Length; ordinal++)
            {
                var goat = goats[ordinal];
                Require(
                    goat.entityIndex == ExpectedLevel1GoatEntities[ordinal] &&
                    goat.entityIndex < entities.Length &&
                    goatOrdinalByEntity[goat.entityIndex] == -1,
                    $"exact Level 1 Goat ordinal {ordinal}");
                goatOrdinalByEntity[goat.entityIndex] = ordinal;
                Require(
                    goat.classId == 5 &&
                    goat.model == 1 &&
                    goat.renderParameterIndex == 238 &&
                    goat.baseTmap == 152 &&
                    goat.animationFrame == 0,
                    $"qualified Goat source/render row {goat.entityIndex}");
                Require(
                    entities[goat.entityIndex].authority == NativeSim.GoatLive,
                    $"legacy entity overlay marks GoatLive {goat.entityIndex}");
            }
            Debug.Log(
                $"MC_LEVEL1 VERIFIED package={packageHash} closure={closureHash} " +
                $"entities={entities.Length}/{active} objectives={objectives.Length} " +
                $"goats={goats.Length} reset={snapshot.stateHash:x16} " +
                $"conformance={snapshot.conformanceHash:x16}");
        }

        private void LoadAndValidateOwnerHudDescriptor()
        {
            hudDescriptor = JsonUtility.FromJson<HudDescriptor>(
                ownerAssetCache.ReadOutputText("hud/level1.json"));
            Require(
                hudDescriptor != null &&
                hudDescriptor.schema == "mc2-level1-owner-hud/v1" &&
                hudDescriptor.reference_layout != null &&
                hudDescriptor.reference_layout.width == 640 &&
                hudDescriptor.reference_layout.height == 480 &&
                hudDescriptor.reference_layout.resolution_bank == "HSPRN0-0" &&
                hudDescriptor.reference_layout.source_scale == 1 &&
                hudDescriptor.reference_layout.filter == "nearest" &&
                hudDescriptor.reference_layout.bank_status ==
                    "source_mapped_Night_high_resolution_bank_native" &&
                hudDescriptor.reference_layout.high_resolution_bank_status.StartsWith(
                    "Qualified", StringComparison.Ordinal),
                "qualified Level 1 HSPRN owner HUD reference layout");

            var expectedSelectors = new[]
            {
                1, 2, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50,
                51, 52, 53, 54, 55, 58, 66, 83, 84, 88, 89, 90, 91,
                97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107,
                108, 109, 110, 111, 112, 113, 114, 115, 116, 117,
                118, 119, 120, 121, 122, 123, 124, 125, 126, 149, 150,
                161, 162, 163, 164, 165, 166, 167, 171, 172, 173,
                179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189,
                190,
            };
            Require(
                hudDescriptor.bitmap_assets != null &&
                hudDescriptor.bitmap_assets.Length == expectedSelectors.Length,
                "complete Level 1 HUD bitmap descriptor set");
            hudBitmapDescriptors.Clear();
            for (var index = 0; index < expectedSelectors.Length; index++)
            {
                var row = hudDescriptor.bitmap_assets[index];
                Require(
                    row != null &&
                    row.selector == expectedSelectors[index] &&
                    row.bank == "HSPRN0-0" &&
                    row.source_asset_id ==
                        $"owner_data:DATA/HSPRN0-0.DAT#{row.selector}" &&
                    row.source_scale == 1 &&
                    row.runtime_width == row.stored_width &&
                    row.runtime_height == row.stored_height &&
                    row.palette == "PALN-0" &&
                    row.filter == "nearest" &&
                    row.authority == "FrozenObserved" &&
                    !string.IsNullOrEmpty(row.indices_path) &&
                    !string.IsNullOrEmpty(row.mask_path) &&
                    row.indices_sha256.Length == 64 &&
                    row.mask_sha256.Length == 64 &&
                    !hudBitmapDescriptors.ContainsKey(row.selector),
                    $"qualified HUD bitmap selector {expectedSelectors[index]}");
                Require(
                    ownerAssetCache.ReadOutput(row.indices_path).Length ==
                        row.stored_width * row.stored_height &&
                    ownerAssetCache.ReadOutput(row.mask_path).Length ==
                        row.stored_width * row.stored_height,
                    $"qualified HUD bitmap payload {row.selector}");
                hudBitmapDescriptors.Add(row.selector, row);
            }
            Require(
                hudDescriptor.top_bar != null &&
                hudDescriptor.top_bar.panels != null &&
                hudDescriptor.top_bar.panels.Length == 6 &&
                hudDescriptor.top_bar.health_mana_bars != null &&
                hudDescriptor.top_bar.health_mana_bars.authority ==
                    "MissionStateLiveSourceMapped",
                "source-mapped owner top bar and live integer health/mana");
            var expectedPanels = new[]
            {
                ("minimap_panel", 40, 2, 2),
                ("castle_empty_panel", 54, 126, 2),
                ("balloon_empty_panel", 54, 254, 2),
                ("player_health_mana_panel", 41, 382, 2),
                ("player_icon", 43, 384, 2),
                ("player_health_mana_icons", 42, 420, 2),
            };
            for (var index = 0; index < expectedPanels.Length; index++)
            {
                var row = hudDescriptor.top_bar.panels[index];
                var expected = expectedPanels[index];
                Require(
                    row.semantic_role == expected.Item1 &&
                    row.selector == expected.Item2 &&
                    row.x == expected.Item3 &&
                    row.y == expected.Item4 &&
                    row.authority == "SourceMappedLive",
                    $"qualified HUD panel {expected.Item1}");
            }
            Require(
                hudDescriptor.spells != null &&
                hudDescriptor.spells.frame_selector == 1 &&
                hudDescriptor.spells.menu != null &&
                hudDescriptor.spells.menu.source_function ==
                    "DrawBottomSpellsMenu_2ECC0@0x20fcc0" &&
                hudDescriptor.spells.menu.rows == 2 &&
                hudDescriptor.spells.menu.columns == 13 &&
                hudDescriptor.spells.menu.reference_y == 408 &&
                hudDescriptor.spells.menu.edge_selector == 88 &&
                hudDescriptor.spells.menu.panel_selector == 89 &&
                hudDescriptor.spells.menu.selection_frame_selector == 90 &&
                hudDescriptor.spells.menu.disabled_panel_selector == 91 &&
                hudDescriptor.spells.menu.small_icon_base_selector == 97 &&
                hudDescriptor.spells.menu.left_assignment_corner_selector == 149 &&
                hudDescriptor.spells.menu.right_assignment_corner_selector == 150 &&
                hudDescriptor.spells.menu.tier_panel_selector == 161 &&
                hudDescriptor.spells.menu.tier_dark_panel_selector == 162 &&
                hudDescriptor.spells.menu.tier_locked_panel_selector == 163 &&
                hudDescriptor.spells.menu.tier_selection_frame_selector == 164 &&
                hudDescriptor.spells.menu.tier_number_base_selector == 165 &&
                hudDescriptor.spells.menu.tier_icon_base_selector == 179 &&
                hudDescriptor.spells.menu.tier_count == 3 &&
                hudDescriptor.spells.menu.panel_width == 48 &&
                hudDescriptor.spells.menu.panel_height == 36 &&
                hudDescriptor.spells.menu.edge_width == 8 &&
                hudDescriptor.spells.menu.authority == "SourceMappedLive" &&
                hudDescriptor.spells.slots != null &&
                hudDescriptor.spells.slots.Length == 2 &&
                hudDescriptor.spells.slots[0].semantic_role == "left_fireball" &&
                hudDescriptor.spells.slots[0].x == 510 &&
                hudDescriptor.spells.slots[0].y == 2 &&
                hudDescriptor.spells.slots[0].holder_entity == 153 &&
                hudDescriptor.spells.slots[0].model == 0 &&
                hudDescriptor.spells.slots[0].icon_selector == 123 &&
                hudDescriptor.spells.slots[0].authority == "MissionStateLive" &&
                hudDescriptor.spells.slots[0].activation_authority ==
                    "SpellActivationLive" &&
                hudDescriptor.spells.slots[1].semantic_role == "right_possess" &&
                hudDescriptor.spells.slots[1].x == 574 &&
                hudDescriptor.spells.slots[1].y == 2 &&
                hudDescriptor.spells.slots[1].holder_entity == 154 &&
                hudDescriptor.spells.slots[1].model == 1 &&
                hudDescriptor.spells.slots[1].icon_selector == 124 &&
                hudDescriptor.spells.slots[1].authority == "MissionStateLive" &&
                hudDescriptor.spells.slots[1].activation_authority ==
                    "SpellActivationLive" &&
                hudDescriptor.spells.tier_rows != null &&
                hudDescriptor.spells.tier_rows.Length == 5 &&
                hudDescriptor.spells.glow_tier_charge.authority ==
                    "MissionStateLiveSourceMapped",
                "qualified live spell selectors, activation, tier and charge authority");
            for (var tier = 0; tier < hudDescriptor.spells.tier_rows.Length; tier++)
            {
                var row = hudDescriptor.spells.tier_rows[tier];
                Require(
                    row.tier == tier &&
                    row.text == new[] { "I", "II", "III", "IV", "V" }[tier] &&
                    row.render_width == 8 * row.text.Length &&
                    row.render_height == 14 &&
                    row.palette_index == 61 &&
                    !string.IsNullOrEmpty(row.render_path),
                    $"qualified HSPRN spell tier {tier}");
            }
            Require(
                hudDescriptor.objective != null &&
                hudDescriptor.objective.selector == 48 &&
                hudDescriptor.objective.authority ==
                    "SourceMappedLiveIndexedFramebuffer" &&
                hudDescriptor.objective.objective_textbox_ticks == 33 &&
                hudDescriptor.objective.render_width == 256 &&
                hudDescriptor.objective.render_height == 38 &&
                hudDescriptor.objective.reference_x == 192 &&
                hudDescriptor.objective.reference_y == 234 &&
                hudDescriptor.objective.raster_plane_bytes == 256 * 38 &&
                !string.IsNullOrEmpty(hudDescriptor.objective.raster_path) &&
                hudDescriptor.objective.tint != null &&
                hudDescriptor.objective.tint.active_map_palette_lut == "CLRN-0" &&
                hudDescriptor.objective.tint.lut_selector == 3840 &&
                hudDescriptor.objective.tint.palette_index == 119 &&
                hudDescriptor.objective.tint.paln_rgb8 != null &&
                hudDescriptor.objective.tint.paln_rgb8.Length == 3 &&
                hudDescriptor.objective.tint.paln_rgb8[0] == 252 &&
                hudDescriptor.objective.tint.paln_rgb8[1] == 36 &&
                hudDescriptor.objective.tint.paln_rgb8[2] == 36 &&
                hudDescriptor.objective.background_blend != null &&
                hudDescriptor.objective.background_blend.source_table_path ==
                    "palette/tablesn.u8" &&
                hudDescriptor.objective.background_blend.table_offset == 0x4000 &&
                hudDescriptor.objective.background_blend.clrn_selector == 0 &&
                hudDescriptor.objective.background_blend.blend_selector == 61 &&
                hudDescriptor.objective.draw_order != null &&
                hudDescriptor.objective.draw_order.Length == 3 &&
                hudDescriptor.objective.draw_order[0] ==
                    "TABLESN_background_blend" &&
                hudDescriptor.objective.draw_order[1] ==
                    "HFONT3_text_palette_119" &&
                hudDescriptor.objective.draw_order[2] ==
                    "HSPRN_frame_171_172_173" &&
                hudDescriptor.objective.rows != null &&
                hudDescriptor.objective.rows.Length == 6 &&
                hudDescriptor.objective.rows[0].selector == 48 &&
                hudDescriptor.objective.rows[5].selector == 53 &&
                snapshot.objectiveTextboxTicks == 33 &&
                objectives[0].textSelector == 0 &&
                ownerAssetCache.ReadOutputText(
                    hudDescriptor.objective.text_bytes_path) ==
                    "\"Fly towards my beacon.\"",
                "qualified source-mapped live Level 1 objectives");
            var expectedObjectiveGeometry = new[,]
            {
                { 48, 192, 234, 256, 38, 224, 246, 208, 244, 224, 18 },
                { 49, 128, 220, 384, 52, 160, 232, 144, 230, 352, 32 },
                { 50, 144, 220, 352, 52, 164, 232, 160, 230, 320, 32 },
                { 51, 144, 220, 352, 52, 164, 232, 160, 230, 320, 32 },
                { 52, 176, 220, 288, 52, 208, 232, 192, 230, 256, 32 },
                { 53, 128, 220, 384, 52, 160, 232, 144, 230, 352, 32 },
            };
            for (var index = 0; index < expectedObjectiveGeometry.GetLength(0); index++)
            {
                var row = hudDescriptor.objective.rows[index];
                Require(
                    row.selector == expectedObjectiveGeometry[index, 0] &&
                    row.reference_x == expectedObjectiveGeometry[index, 1] &&
                    row.reference_y == expectedObjectiveGeometry[index, 2] &&
                    row.render_width == expectedObjectiveGeometry[index, 3] &&
                    row.render_height == expectedObjectiveGeometry[index, 4] &&
                    row.text_x == expectedObjectiveGeometry[index, 5] &&
                    row.text_y == expectedObjectiveGeometry[index, 6] &&
                    row.blend_x == expectedObjectiveGeometry[index, 7] &&
                    row.blend_y == expectedObjectiveGeometry[index, 8] &&
                    row.blend_width == expectedObjectiveGeometry[index, 9] &&
                    row.blend_height == expectedObjectiveGeometry[index, 10] &&
                    row.raster_plane_bytes ==
                        row.render_width * row.render_height &&
                    row.foreground_indices_sha256 != null &&
                    row.foreground_indices_sha256.Length == 64 &&
                    row.foreground_mask_sha256 != null &&
                    row.foreground_mask_sha256.Length == 64 &&
                    row.foreground_opaque_pixels > 0 &&
                    row.authority ==
                        "SourceMappedLiveIndexedFramebuffer" &&
                    !string.IsNullOrEmpty(row.raster_path) &&
                    row.reference_y >= 128,
                    $"qualified objective indexed geometry {row.selector}");
            }
            Require(
                hudDescriptor.authority_badge != null &&
                hudDescriptor.authority_badge.text == "FROZEN OBSERVED" &&
                hudDescriptor.authority_badge.render_width == 120 &&
                hudDescriptor.authority_badge.render_height == 14 &&
                hudDescriptor.minimap != null &&
                hudDescriptor.minimap.width == 128 &&
                hudDescriptor.minimap.height == 128 &&
                hudDescriptor.minimap.logical_width == 128 &&
                hudDescriptor.minimap.logical_height == 128 &&
                hudDescriptor.minimap.presentation_scale == 1 &&
                hudDescriptor.minimap.map_mode ==
                    "player_centered_yaw_rotated_TABLESN_scanline_aperture" &&
                hudDescriptor.minimap.resolution_branch ==
                    "HSPRN_native_128x128_scaling256" &&
                hudDescriptor.minimap.aperture != null &&
                hudDescriptor.minimap.aperture.opaque_pixels == 12848 &&
                hudDescriptor.minimap.aperture.table_offset == 0x14300 &&
                hudDescriptor.minimap.aperture.phase_step == 0x40000 &&
                hudDescriptor.minimap.aperture.scanlines != null &&
                hudDescriptor.minimap.aperture.scanlines.Length == 128 &&
                hudDescriptor.minimap.rotation != null &&
                !string.IsNullOrEmpty(
                    hudDescriptor.minimap.rotation.sine_table_path) &&
                hudDescriptor.minimap.terrain_authority ==
                    "OwnerLevelStaticLiveSampling" &&
                hudDescriptor.minimap.entity_positions_authority ==
                    "SourceArenaLive" &&
                hudDescriptor.minimap.entity_overlay_mode ==
                    "Rust_source_slot_ordered_DrawMinimapEntities_commands" &&
                hudDescriptor.minimap.player_pose_authority == "PlayerFlightLive" &&
                hudDescriptor.minimap.entity_colour_filter_status.StartsWith(
                    "SourceMappedLive", StringComparison.Ordinal) &&
                hudDescriptor.minimap.objective_marks_status.StartsWith(
                    "SourceMappedLive", StringComparison.Ordinal),
                "qualified live mixed-authority minimap descriptor");
            Debug.Log(
                "MC_HUD_DESCRIPTOR VERIFIED bitmaps=80 bank=HSPRN-native " +
                "objectives=48-53/indexed-live-framebuffer " +
                "blend=TABLESN:0x4000+61x256+dst tint=CLRN3840:119:252,36,36 " +
                "spells=123-126/MissionStateLive+activationLive " +
                "minimap=128-native/TABLESN-aperture/exact-fixed-rotation " +
                "entityPass=Rust-source-slot-ordered objectiveMarks=SourceMappedLive");
        }

        private void ConfigureWorld()
        {
            RenderSettings.fog = true;
            RenderSettings.fogColor = new Color(0.10f, 0.13f, 0.22f);
            RenderSettings.fogMode = FogMode.ExponentialSquared;
            RenderSettings.fogDensity = 0.0015f;
            RenderSettings.ambientLight = new Color(0.20f, 0.23f, 0.36f);

            if (debugMode)
            {
                var moon = new GameObject("Diagnostic Moon Light").AddComponent<Light>();
                moon.type = LightType.Directional;
                moon.color = new Color(0.62f, 0.72f, 1f);
                moon.intensity = 1.15f;
                moon.transform.rotation = Quaternion.Euler(38f, -24f, 0f);
                moon.shadows = LightShadows.Soft;
            }

            typeTexture = CreatePlaneTexture("Terrain Types", terrainTypes);
            heightTexture = CreatePlaneTexture("Ground Heights", groundHeights);
            shadeTexture = CreatePlaneTexture("Terrain Shade", terrainShade);
            angleTexture = CreatePlaneTexture("Terrain Angles", terrainAngles);
            secondaryTexture = CreatePlaneTexture("Secondary Heights", secondaryHeights);
            CreateTerrainCopies();
            CreateEntityRegistry();
            CreateMissionPresentationRegistry();
            if (ownerAssetCache != null) CreateOwnerHud();
            else CreateMinimap();
        }

        private void ConfigureCameraAndCarpet()
        {
            presentationRig = new GameObject("Presentation Rig").transform;
            flightCamera = new GameObject("Flight Camera").AddComponent<Camera>();
            flightCamera.gameObject.AddComponent<AudioListener>();
            audioCapture = flightCamera.gameObject.AddComponent<AudioQualificationCapture>();
            flightCamera.transform.SetParent(presentationRig, false);
            flightCamera.transform.localPosition = Vector3.zero;
            // Unity needs a positive geometric near plane. MC2 did not have
            // one equivalent value: terrain used a 128 projection-divisor
            // floor plus a -256 rejection boundary, while objects required
            // forward depth >64. Those rules are locked in sim-core; this is
            // an explicitly presentation-only raster safety adaptation.
            flightCamera.nearClipPlane = 0.035f;
            flightCamera.farClipPlane = 1800f;
            flightCamera.fieldOfView =
                Level1ExitPresentationMath.UnityVerticalFov(128);
            flightCamera.rect =
                Level1ExitPresentationMath.SourceDisplayRect(
                    Screen.width, Screen.height);
            flightCamera.clearFlags = CameraClearFlags.SolidColor;
            flightCamera.backgroundColor = RenderSettings.fogColor;
            Debug.Log(
                "MC_RENDER_ADAPTATION unityNearClip=0.035 " +
                "sourceUnits=2.24 sourceTerrainDepth=-256/128 " +
                "sourceObjectDepth=>64");

            if (debugMode)
            {
                var carpet = DiagnosticPrimitive(
                    PrimitiveType.Cube,
                    "Diagnostic Carpet",
                    new Color(0.38f, 0.04f, 0.12f),
                    presentationRig);
                carpetVisual = carpet.transform;
                carpet.transform.localPosition = new Vector3(0f, -1.95f, 1.34f);
                carpet.transform.localScale = new Vector3(0.92f, 0.045f, 1.38f);
                var trim = DiagnosticPrimitive(
                    PrimitiveType.Cube,
                    "Diagnostic Carpet Trim",
                    new Color(0.85f, 0.55f, 0.12f),
                    carpet.transform);
                trim.transform.localPosition = new Vector3(0f, 0.58f, 0.02f);
                trim.transform.localScale = new Vector3(1.05f, 0.35f, 1.02f);
                SetLayerRecursively(carpet, CarpetOverlayLayer);
                flightCamera.cullingMask &= ~(1 << CarpetOverlayLayer);
                carpetOverlayCamera =
                    new GameObject("Diagnostic Carpet Overlay Camera").AddComponent<Camera>();
                carpetOverlayCamera.transform.SetParent(presentationRig, false);
                carpetOverlayCamera.transform.localPosition = Vector3.zero;
                carpetOverlayCamera.transform.localRotation = Quaternion.identity;
                carpetOverlayCamera.CopyFrom(flightCamera);
                carpetOverlayCamera.clearFlags = CameraClearFlags.Depth;
                carpetOverlayCamera.cullingMask = 1 << CarpetOverlayLayer;
                carpetOverlayCamera.depth = flightCamera.depth + 1f;
                carpetOverlayCamera.nearClipPlane = 0.01f;
                carpetOverlayCamera.farClipPlane = 10f;
                fireballMaterial =
                    CreateMaterial(new Color(1f, 0.15f, 0.015f), 0.2f, 0f);
                fireballMaterial.EnableKeyword("_EMISSION");
                fireballMaterial.SetColor(
                    "_EmissionColor", new Color(5f, 0.45f, 0.02f));
            }
        }

        private void ConfigureExactTerrainFrameComposite()
        {
            if (ownerAssetCache == null || entityGallery)
            {
                return;
            }
            if (localAudioQualificationMode || positionalAudioQualificationMode)
            {
                if (qualifiedMode) DisableTerrainMeshRenderers();
                return;
            }
            Require(flightCamera != null, "terrain frame flight camera");
            Require(ownerPaletteTexture != null, "terrain frame PALN texture");
            Require(
                SystemInfo.SupportsTextureFormat(TextureFormat.R8),
                "GPU support for indexed R8 terrain frame");

            terrainFrameIndexTexture = CreateTerrainFrameTexture(
                "Exact isolated MC2 sky and terrain indices");
            terrainFrameCoverageTexture = CreateTerrainFrameTexture(
                "Exact isolated MC2 terrain coverage");

            var compositeShader = Resources.Load<Shader>("TerrainIndexedFrame");
            Require(compositeShader != null, "indexed terrain frame composite shader");
            terrainFrameCompositeMaterial = new Material(compositeShader)
            {
                name = "Exact isolated 320x200 sky and terrain PALN composite"
            };
            terrainFrameCompositeMaterial.SetTexture(
                "_Indices", terrainFrameIndexTexture);
            terrainFrameCompositeMaterial.SetTexture(
                "_Coverage", terrainFrameCoverageTexture);
            terrainFrameCompositeMaterial.SetTexture("_Palette", ownerPaletteTexture);

            terrainFrameCommandBuffer = new CommandBuffer
            {
                name = "Exact isolated MC2 sky and terrain before owner overlays"
            };
            terrainFrameCommandBuffer.Blit(
                null,
                BuiltinRenderTextureType.CameraTarget,
                terrainFrameCompositeMaterial,
                0);
            flightCamera.AddCommandBuffer(
                CameraEvent.BeforeForwardOpaque,
                terrainFrameCommandBuffer);

            if (qualifiedMode)
            {
                DisableTerrainMeshRenderers();
                return;
            }

            var depthShader = Resources.Load<Shader>("TerrainDepthOnly");
            Require(depthShader != null, "terrain diagnostic depth-only shader");
            terrainDepthOnlyMaterial = new Material(depthShader)
            {
                name = "Diagnostic Unity terrain depth only over exact indexed frame"
            };
            terrainDepthOnlyMaterial.SetTexture(
                "_SurfaceDisplacement", terrainSurfaceDisplacementTexture);
            terrainDepthOnlyMaterial.SetTexture(
                "_GridCellValid", terrainGridCellValidTexture);
            terrainDepthOnlyMaterial.SetFloat(
                "_SourceUnitsPerUnityUnit", SourceUnitsPerUnityUnit);
            foreach (var tile in terrainTiles)
            {
                var renderer = tile.GetComponent<MeshRenderer>();
                Require(renderer != null, $"terrain depth renderer {tile.name}");
                renderer.sharedMaterial = terrainDepthOnlyMaterial;
            }
        }

        private void DisableTerrainMeshRenderers()
        {
            foreach (var tile in terrainTiles)
            {
                var renderer = tile.GetComponent<MeshRenderer>();
                Require(renderer != null, $"terrain renderer {tile.name}");
                renderer.enabled = false;
            }
        }

        private static Texture2D CreateTerrainFrameTexture(string name)
        {
            return new Texture2D(
                NativeSim.TerrainFrameWidth,
                NativeSim.TerrainFrameHeight,
                TextureFormat.R8,
                false,
                true)
            {
                name = name,
                filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Clamp
            };
        }

        private void UpdateExactTerrainFrame()
        {
            if (terrainFrameCompositeMaterial == null) return;
            terrainFrameInfo = session.WorldFrame(
                renderCameraState,
                terrainFrameIndices,
                terrainFrameCoverage);

            // Native rows are source top-to-bottom. Unity raw texture row zero
            // is the bottom row. Reverse row order exactly once before upload;
            // the source buffers and their native hashes remain unchanged.
            FlipTerrainFrameRows(terrainFrameIndices, terrainFrameUploadIndices);
            FlipTerrainFrameRows(terrainFrameCoverage, terrainFrameUploadCoverage);
            terrainFrameIndexTexture.LoadRawTextureData(terrainFrameUploadIndices);
            terrainFrameCoverageTexture.LoadRawTextureData(terrainFrameUploadCoverage);
            terrainFrameIndexTexture.Apply(false, false);
            terrainFrameCoverageTexture.Apply(false, false);
            if (!terrainFrameGpuValidated)
            {
                var first = ValidateTerrainFrameGpuGolden(
                    "reset-world-supported-families",
                    0xad0386b8aca8afbd,
                    0x77fdd7ddf04bb56d,
                    true);
                var repeated = ValidateTerrainFrameGpuGolden(
                    "reset-world-supported-families-repeat",
                    0xad0386b8aca8afbd,
                    0x77fdd7ddf04bb56d,
                    false);
                Require(first == repeated, "same-tick terrain GPU readback stability");
                terrainFrameGpuValidated = true;
            }
            terrainFrameCompositeLive = true;
            UpdateOwnerObjectiveTextbox();

            if (!terrainFrameLiveLogged)
            {
                terrainFrameLiveLogged = true;
                Debug.Log(
                    "MC_TERRAIN_FRAME_LIVE PASS " +
                    $"generation={terrainFrameInfo.generation} " +
                    $"tick={terrainFrameInfo.tick} " +
                    $"extent={terrainFrameInfo.width}x{terrainFrameInfo.height} " +
                    $"indexHash=0x{terrainFrameInfo.indexHash:x16} " +
                    $"coverageHash=0x{terrainFrameInfo.coverageHash:x16} " +
                    $"triangles={terrainFrameInfo.reflectedTriangles}+" +
                    $"{terrainFrameInfo.primaryMode5Triangles}+" +
                    $"{terrainFrameInfo.primaryMode26Triangles} " +
                    $"writes={terrainFrameInfo.terrainPixelWrites} " +
                    "layer=source-ordered-terrain-actors-projectiles-economy " +
                    "frozenActors=villagers-dolmens-native " +
                    "remainingNonTerrain=omitted " +
                    "upload=R8-point-top-down-row-flip-once");
            }
        }

        private void UpdateOwnerObjectiveTextbox()
        {
            if (
                hudDescriptor?.objective?.background_blend == null ||
                ownerObjectiveRows.Count == 0)
                return;
            var visibleSelector = mission.objectiveTextboxTicks > 0
                ? mission.textSelector
                : -1;
            if (
                ownerObjectiveVisibleSelector >= 48 &&
                ownerObjectiveVisibleSelector <= 53 &&
                ownerObjectiveVisibleSelector != visibleSelector &&
                ownerObjectiveRows.TryGetValue(
                    ownerObjectiveVisibleSelector,
                    out var previousRow))
            {
                UpdateOwnerObjectiveTexture(previousRow, 0);
            }
            if (
                visibleSelector >= 48 &&
                visibleSelector <= 53 &&
                ownerObjectiveRows.TryGetValue(visibleSelector, out var row))
            {
                UpdateOwnerObjectiveTexture(
                    row,
                    mission.objectiveTextboxTicks);
            }
            ownerObjectiveVisibleSelector = visibleSelector;
        }

        private void UpdateOwnerObjectiveTexture(
            HudObjectiveRowDescriptor row,
            int visibleTicks)
        {
            byte[] raster = null;
            byte[] overlayIndices = null;
            byte[] overlayMask = null;
            Color32[] uploadPixels = null;
            Texture2D texture = null;
            Require(
                ownerObjectiveRasters.TryGetValue(row.selector, out raster) &&
                ownerObjectiveOverlayIndices.TryGetValue(
                    row.selector, out overlayIndices) &&
                ownerObjectiveOverlayMasks.TryGetValue(
                    row.selector, out overlayMask) &&
                ownerObjectiveUploadPixels.TryGetValue(
                    row.selector, out uploadPixels) &&
                ownerObjectiveTextures.TryGetValue(
                    row.selector, out texture),
                $"indexed objective runtime resources {row.selector}");
            var blend = hudDescriptor.objective.background_blend;
            Require(
                blend.table_offset == 0x4000 &&
                blend.clrn_selector == 0 &&
                blend.blend_selector == ownerHudClrn[0] &&
                blend.blend_selector == 61 &&
                row.reference_y >= 128,
                $"indexed objective source blend contract {row.selector}");
            ObjectiveTextboxCompositor.ComposeOverlayIndices(
                visibleTicks,
                terrainFrameIndices,
                NativeSim.TerrainFrameWidth,
                NativeSim.TerrainFrameHeight,
                hudDescriptor.reference_layout.width,
                hudDescriptor.reference_layout.height,
                row.reference_x,
                row.reference_y,
                row.render_width,
                row.render_height,
                row.blend_x,
                row.blend_y,
                row.blend_width,
                row.blend_height,
                ownerHudTables,
                blend.table_offset,
                checked((byte)blend.blend_selector),
                raster,
                overlayIndices,
                overlayMask);
            ObjectiveTextboxCompositor.MaterializeBottomUpRgba(
                overlayIndices,
                overlayMask,
                row.render_width,
                row.render_height,
                ownerHudPaletteRgb8,
                uploadPixels);
            texture.SetPixels32(uploadPixels);
            texture.Apply(false, false);
            if (visibleTicks > 0 && !ownerObjectiveBlendLogged)
            {
                ownerObjectiveBlendLogged = true;
                Debug.Log(
                    "MC_OBJECTIVE_INDEXED_BLEND LIVE " +
                    $"selector={row.selector} ticks={visibleTicks} " +
                    $"outer={row.reference_x},{row.reference_y}," +
                    $"{row.render_width},{row.render_height} " +
                    $"blend={row.blend_x},{row.blend_y}," +
                    $"{row.blend_width},{row.blend_height} " +
                    "order=TABLESN61->HFONT3-119->HSPRN171/172/173 " +
                    "underlay=qualified-320x200-indexed-world/" +
                    "pixel-centre-nearest-to-640x480");
            }
        }

        private static void FlipTerrainFrameRows(byte[] source, byte[] destination)
        {
            Require(
                source != null && destination != null &&
                source.Length == NativeSim.TerrainFramePixels &&
                destination.Length == NativeSim.TerrainFramePixels,
                "terrain frame row-flip extents");
            for (var sourceY = 0; sourceY < NativeSim.TerrainFrameHeight; sourceY++)
            {
                var destinationY = NativeSim.TerrainFrameHeight - 1 - sourceY;
                Buffer.BlockCopy(
                    source,
                    sourceY * NativeSim.TerrainFrameWidth,
                    destination,
                    destinationY * NativeSim.TerrainFrameWidth,
                    NativeSim.TerrainFrameWidth);
            }
            Require(
                destination[0] ==
                    source[(NativeSim.TerrainFrameHeight - 1) *
                           NativeSim.TerrainFrameWidth] &&
                destination[NativeSim.TerrainFramePixels - 1] ==
                    source[NativeSim.TerrainFrameWidth - 1],
                "terrain frame row flip maps source top and bottom exactly once");
        }

        private ulong ValidateTerrainFrameGpuGolden(
            string label,
            ulong expectedReadbackBottomUpHash,
            ulong expectedLogicalTopDownHash,
            bool validateRawUploads)
        {
            Require(
                terrainFrameIndexTexture != null &&
                terrainFrameCoverageTexture != null &&
                terrainFrameCompositeMaterial != null &&
                ownerHudPaletteRgb8 != null &&
                ownerHudPaletteRgb8.Length == 256 * 3,
                "terrain frame GPU golden resources");
            Require(
                QualitySettings.activeColorSpace == ColorSpace.Gamma,
                "qualified terrain frame GPU golden uses project Gamma color space");
            Require(
                terrainFrameIndexTexture.format == TextureFormat.R8 &&
                terrainFrameIndexTexture.width == NativeSim.TerrainFrameWidth &&
                terrainFrameIndexTexture.height == NativeSim.TerrainFrameHeight &&
                terrainFrameIndexTexture.mipmapCount == 1 &&
                terrainFrameIndexTexture.filterMode == FilterMode.Point &&
                terrainFrameIndexTexture.wrapMode == TextureWrapMode.Clamp,
                "terrain frame GPU texture is point R8 without mipmaps");
            Require(
                terrainFrameCoverageTexture.format == TextureFormat.R8 &&
                terrainFrameCoverageTexture.width == NativeSim.TerrainFrameWidth &&
                terrainFrameCoverageTexture.height == NativeSim.TerrainFrameHeight &&
                terrainFrameCoverageTexture.mipmapCount == 1 &&
                terrainFrameCoverageTexture.filterMode == FilterMode.Point &&
                terrainFrameCoverageTexture.wrapMode == TextureWrapMode.Clamp &&
                ownerPaletteTexture.format == TextureFormat.RGB24 &&
                ownerPaletteTexture.mipmapCount == 1 &&
                ownerPaletteTexture.filterMode == FilterMode.Point &&
                ownerPaletteTexture.wrapMode == TextureWrapMode.Clamp,
                "terrain coverage R8 and PALN GPU texture contracts");

            var target = RenderTexture.GetTemporary(
                NativeSim.TerrainFrameWidth,
                NativeSim.TerrainFrameHeight,
                0,
                RenderTextureFormat.ARGB32,
                RenderTextureReadWrite.Linear);
            target.filterMode = FilterMode.Point;
            target.wrapMode = TextureWrapMode.Clamp;
            Require(
                target.antiAliasing == 1,
                "terrain frame GPU golden render target has no multisampling");
            var prior = RenderTexture.active;
            var readback = new Texture2D(
                NativeSim.TerrainFrameWidth,
                NativeSim.TerrainFrameHeight,
                TextureFormat.RGBA32,
                false,
                true);
            try
            {
                Graphics.Blit(
                    Texture2D.blackTexture,
                    target,
                    terrainFrameCompositeMaterial,
                    0);
                RenderTexture.active = target;
                readback.ReadPixels(
                    new Rect(
                        0,
                        0,
                        NativeSim.TerrainFrameWidth,
                        NativeSim.TerrainFrameHeight),
                    0,
                    0,
                    false);
                readback.Apply(false, false);
                var observed = readback.GetRawTextureData<byte>();
                Require(
                    observed.Length == NativeSim.TerrainFramePixels * 4,
                    "terrain frame GPU RGBA readback extent");
                for (var pixel = 0; pixel < NativeSim.TerrainFramePixels; pixel++)
                {
                    var uploadY = pixel / NativeSim.TerrainFrameWidth;
                    var x = pixel % NativeSim.TerrainFrameWidth;
                    var sourceY = NativeSim.TerrainFrameHeight - 1 - uploadY;
                    var sourcePixel =
                        sourceY * NativeSim.TerrainFrameWidth + x;
                    Require(
                        terrainFrameUploadIndices[pixel] ==
                            terrainFrameIndices[sourcePixel],
                        $"terrain frame logical source-to-readback orientation {pixel}");
                    var palette = terrainFrameIndices[sourcePixel] * 3;
                    var rgba = pixel * 4;
                    Require(
                        observed[rgba] == ownerHudPaletteRgb8[palette] &&
                        observed[rgba + 1] == ownerHudPaletteRgb8[palette + 1] &&
                        observed[rgba + 2] == ownerHudPaletteRgb8[palette + 2] &&
                        observed[rgba + 3] == 255,
                        $"terrain frame GPU palette resolve pixel {pixel} " +
                        $"index={terrainFrameIndices[sourcePixel]}");
                }
                var readbackBottomUp = observed.ToArray();
                var logicalTopDown = new byte[readbackBottomUp.Length];
                var rgbaStride = NativeSim.TerrainFrameWidth * 4;
                for (var sourceY = 0;
                     sourceY < NativeSim.TerrainFrameHeight;
                     sourceY++)
                {
                    var readbackY = NativeSim.TerrainFrameHeight - 1 - sourceY;
                    Buffer.BlockCopy(
                        readbackBottomUp,
                        readbackY * rgbaStride,
                        logicalTopDown,
                        sourceY * rgbaStride,
                        rgbaStride);
                }
                var readbackBottomUpHash = ByteHash(readbackBottomUp);
                var logicalTopDownHash = ByteHash(logicalTopDown);
                Debug.Log(
                    "MC_TERRAIN_FRAME_GPU_OBSERVED " +
                    $"label={label} " +
                    $"rgbaReadbackBottomUpHash=0x{readbackBottomUpHash:x16} " +
                    $"rgbaLogicalTopDownHash=0x{logicalTopDownHash:x16}");
                Require(
                    expectedReadbackBottomUpHash == 0 ||
                    readbackBottomUpHash == expectedReadbackBottomUpHash,
                    $"terrain frame GPU {label} bottom-up readback golden hash " +
                    $"expected=0x{expectedReadbackBottomUpHash:x16} " +
                    $"observed=0x{readbackBottomUpHash:x16}");
                Require(
                    expectedLogicalTopDownHash == 0 ||
                    logicalTopDownHash == expectedLogicalTopDownHash,
                    $"terrain frame GPU {label} logical top-down golden hash " +
                    $"expected=0x{expectedLogicalTopDownHash:x16} " +
                    $"observed=0x{logicalTopDownHash:x16}");
                if (validateRawUploads)
                {
                    ValidateRawTerrainFrameGpuPlane(
                        terrainFrameIndexTexture,
                        terrainFrameIndices,
                        terrainFrameUploadIndices,
                        "indices");
                    ValidateRawTerrainFrameGpuPlane(
                        terrainFrameCoverageTexture,
                        terrainFrameCoverage,
                        terrainFrameUploadCoverage,
                        "coverage");
                    ValidateTerrainPaletteRampGpuGolden();
                }
                Debug.Log(
                    "MC_TERRAIN_FRAME_GPU_GOLDEN PASS " +
                    $"label={label} " +
                    $"pixels={NativeSim.TerrainFramePixels} " +
                    $"rgbaReadbackBottomUpHash=0x{readbackBottomUpHash:x16} " +
                    $"rgbaLogicalTopDownHash=0x{logicalTopDownHash:x16} " +
                    "format=R8-to-RGBA32 point=true mipmaps=1 " +
                    "orientation=source-top-down-to-unity-bottom-up-once " +
                    $"palette=PALN exactAllPixels=true " +
                    $"graphicsApi={SystemInfo.graphicsDeviceType} " +
                    $"graphicsDevice={SystemInfo.graphicsDeviceName}");
                WriteTerrainFrameQualificationArtifacts(
                    label,
                    readbackBottomUpHash,
                    logicalTopDownHash,
                    readback);
                return readbackBottomUpHash;
            }
            finally
            {
                RenderTexture.active = prior;
                RenderTexture.ReleaseTemporary(target);
                Destroy(readback);
            }
        }

        private static void ValidateRawTerrainFrameGpuPlane(
            Texture2D texture,
            byte[] sourceTopDown,
            byte[] uploadBottomUp,
            string label)
        {
            var shader = Resources.Load<Shader>("TerrainFrameRawGolden");
            Require(shader != null, "raw terrain-frame GPU golden shader");
            var material = new Material(shader)
            {
                name = $"Raw terrain-frame {label} qualification material"
            };
            material.SetTexture("_Indices", texture);
            var target = RenderTexture.GetTemporary(
                NativeSim.TerrainFrameWidth,
                NativeSim.TerrainFrameHeight,
                0,
                RenderTextureFormat.ARGB32,
                RenderTextureReadWrite.Linear);
            target.filterMode = FilterMode.Point;
            target.wrapMode = TextureWrapMode.Clamp;
            var prior = RenderTexture.active;
            var readback = new Texture2D(
                NativeSim.TerrainFrameWidth,
                NativeSim.TerrainFrameHeight,
                TextureFormat.RGBA32,
                false,
                true);
            try
            {
                Graphics.Blit(Texture2D.blackTexture, target, material, 0);
                RenderTexture.active = target;
                readback.ReadPixels(
                    new Rect(
                        0,
                        0,
                        NativeSim.TerrainFrameWidth,
                        NativeSim.TerrainFrameHeight),
                    0,
                    0,
                    false);
                readback.Apply(false, false);
                var observed = readback.GetRawTextureData<byte>();
                for (var pixel = 0; pixel < NativeSim.TerrainFramePixels; pixel++)
                {
                    var uploadY = pixel / NativeSim.TerrainFrameWidth;
                    var x = pixel % NativeSim.TerrainFrameWidth;
                    var sourceY = NativeSim.TerrainFrameHeight - 1 - uploadY;
                    var sourcePixel =
                        sourceY * NativeSim.TerrainFrameWidth + x;
                    var expected = sourceTopDown[sourcePixel];
                    var rgba = pixel * 4;
                    Require(
                        uploadBottomUp[pixel] == expected &&
                        observed[rgba] == expected &&
                        observed[rgba + 1] == expected &&
                        observed[rgba + 2] == expected &&
                        observed[rgba + 3] == 255,
                        $"raw terrain-frame {label} GPU pixel {pixel}");
                }
                var rawHash = ByteHash(observed);
                var expectedHash = label == "indices"
                    ? 0xa572934d2db1f794UL
                    : 0x0982b037e66fa6d4UL;
                Require(
                    rawHash == expectedHash,
                    $"raw terrain-frame {label} reset GPU hash");
                Debug.Log(
                    $"MC_TERRAIN_FRAME_RAW_GPU_GOLDEN PASS plane={label} " +
                    $"pixels={NativeSim.TerrainFramePixels} " +
                    $"rgbaHash=0x{rawHash:x16}");
            }
            finally
            {
                RenderTexture.active = prior;
                RenderTexture.ReleaseTemporary(target);
                Destroy(readback);
                Destroy(material);
            }
        }

        private void ValidateTerrainPaletteRampGpuGolden()
        {
            var ramp = new byte[256];
            for (var index = 0; index < ramp.Length; index++)
                ramp[index] = checked((byte)index);
            var rampTexture = new Texture2D(
                256, 1, TextureFormat.R8, false, true)
            {
                name = "Terrain PALN qualification index ramp",
                filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Clamp
            };
            rampTexture.LoadRawTextureData(ramp);
            rampTexture.Apply(false, false);
            var shader = Resources.Load<Shader>("TerrainIndexedFrame");
            Require(shader != null, "terrain PALN ramp production shader");
            var material = new Material(shader)
            {
                name = "Terrain PALN 256-entry qualification material"
            };
            material.SetTexture("_Indices", rampTexture);
            material.SetTexture("_Coverage", Texture2D.whiteTexture);
            material.SetTexture("_Palette", ownerPaletteTexture);
            var target = RenderTexture.GetTemporary(
                256, 1, 0, RenderTextureFormat.ARGB32, RenderTextureReadWrite.Linear);
            target.filterMode = FilterMode.Point;
            var prior = RenderTexture.active;
            var readback = new Texture2D(
                256, 1, TextureFormat.RGBA32, false, true);
            try
            {
                Graphics.Blit(Texture2D.blackTexture, target, material, 0);
                RenderTexture.active = target;
                readback.ReadPixels(new Rect(0, 0, 256, 1), 0, 0, false);
                readback.Apply(false, false);
                var observed = readback.GetRawTextureData<byte>();
                for (var index = 0; index < 256; index++)
                {
                    var palette = index * 3;
                    var rgba = index * 4;
                    Require(
                        observed[rgba] == ownerHudPaletteRgb8[palette] &&
                        observed[rgba + 1] == ownerHudPaletteRgb8[palette + 1] &&
                        observed[rgba + 2] == ownerHudPaletteRgb8[palette + 2] &&
                        observed[rgba + 3] == 255,
                        $"terrain PALN GPU ramp entry {index}");
                }
                var rgbaHash = ByteHash(observed);
                Require(
                    rgbaHash == 0xc9f06b92b2133799UL,
                    "terrain PALN complete GPU ramp hash");
                Debug.Log(
                    "MC_TERRAIN_FRAME_PALN_GPU_GOLDEN PASS entries=256 " +
                    $"rgbaHash=0x{rgbaHash:x16}");
            }
            finally
            {
                RenderTexture.active = prior;
                RenderTexture.ReleaseTemporary(target);
                Destroy(readback);
                Destroy(material);
                Destroy(rampTexture);
            }
        }

        private void WriteTerrainFrameQualificationArtifacts(
            string label,
            ulong rgbaReadbackBottomUpHash,
            ulong rgbaLogicalTopDownHash,
            Texture2D rgbaReadback)
        {
            if (!label.StartsWith("route-", StringComparison.Ordinal)) return;
            var requested =
                Environment.GetEnvironmentVariable("MC_AUTOPLAY_CAPTURE_DIR");
            if (string.IsNullOrWhiteSpace(requested)) return;
            var directory = Path.GetFullPath(requested);
            var temporaryRoot = Path.GetFullPath(Path.GetTempPath())
                .TrimEnd(Path.DirectorySeparatorChar) +
                Path.DirectorySeparatorChar;
            Require(
                (directory + Path.DirectorySeparatorChar).StartsWith(
                    temporaryRoot,
                    StringComparison.Ordinal),
                "private terrain-frame qualification artifacts stay in system temporary storage");
            Directory.CreateDirectory(directory);
            var stem = Path.Combine(directory, $"terrain-frame-{label}");
            var indexPath =
                AudioQualificationCapture.ResolveQualifiedCapturePath(
                    stem + ".indices.r8");
            var coveragePath =
                AudioQualificationCapture.ResolveQualifiedCapturePath(
                    stem + ".coverage.r8");
            var rgbaPath =
                AudioQualificationCapture.ResolveQualifiedCapturePath(
                    stem + ".rgba.png");
            var manifestPath =
                AudioQualificationCapture.ResolveQualifiedCapturePath(
                    stem + ".json");
            File.WriteAllBytes(indexPath, terrainFrameIndices);
            File.WriteAllBytes(coveragePath, terrainFrameCoverage);
            File.WriteAllBytes(rgbaPath, rgbaReadback.EncodeToPNG());
            File.WriteAllText(
                manifestPath,
                "{\n" +
                "  \"schema\": \"mc2-private-isolated-terrain-frame-v2\",\n" +
                $"  \"label\": \"{label}\",\n" +
                $"  \"generation\": {terrainFrameInfo.generation},\n" +
                $"  \"render_tick\": {terrainFrameInfo.tick},\n" +
                $"  \"width\": {terrainFrameInfo.width},\n" +
                $"  \"height\": {terrainFrameInfo.height},\n" +
                $"  \"index_hash_fnv1a64\": \"{terrainFrameInfo.indexHash:x16}\",\n" +
                $"  \"coverage_hash_fnv1a64\": \"{terrainFrameInfo.coverageHash:x16}\",\n" +
                $"  \"rgba_readback_bottom_up_hash_fnv1a64\": \"{rgbaReadbackBottomUpHash:x16}\",\n" +
                $"  \"rgba_logical_top_down_hash_fnv1a64\": \"{rgbaLogicalTopDownHash:x16}\",\n" +
                $"  \"reflected_triangles\": {terrainFrameInfo.reflectedTriangles},\n" +
                $"  \"primary_mode5_triangles\": {terrainFrameInfo.primaryMode5Triangles},\n" +
                $"  \"primary_mode26_triangles\": {terrainFrameInfo.primaryMode26Triangles},\n" +
                $"  \"terrain_pixel_writes\": {terrainFrameInfo.terrainPixelWrites},\n" +
                "  \"r8_row_order\": \"source-top-down\",\n" +
                "  \"gpu_readback_storage_order\": \"unity-bottom-up\",\n" +
                "  \"rgba_png_row_order\": \"logical-top-down\",\n" +
                "  \"scope\": \"source-ordered-world-frame; terrain, qualified " +
                "actors, projectiles, and economy composited; remaining " +
                "non-terrain omitted\"\n" +
                "}\n");
        }

        private static ulong ByteHash(byte[] bytes)
        {
            const ulong Offset = 0xcbf29ce484222325UL;
            const ulong Prime = 0x100000001b3UL;
            var hash = Offset;
            unchecked
            {
                foreach (var value in bytes)
                {
                    hash ^= value;
                    hash *= Prime;
                }
            }
            return hash;
        }

        private static ulong ByteHash(Unity.Collections.NativeArray<byte> bytes)
        {
            const ulong Offset = 0xcbf29ce484222325UL;
            const ulong Prime = 0x100000001b3UL;
            var hash = Offset;
            unchecked
            {
                foreach (var value in bytes)
                {
                    hash ^= value;
                    hash *= Prime;
                }
            }
            return hash;
        }

        internal static ulong TerrainHash(
            byte[] types,
            byte[] heights,
            byte[] shading,
            byte[] angles,
            byte[] secondary,
            short[] entityIndices)
        {
            const ulong Offset = 0xcbf29ce484222325UL;
            const ulong Prime = 0x100000001b3UL;
            var hash = Offset;
            void Append(byte value)
            {
                unchecked
                {
                    hash ^= value;
                    hash *= Prime;
                }
            }
            void AppendBytes(byte[] values)
            {
                foreach (var value in values) Append(value);
            }

            AppendBytes(Encoding.ASCII.GetBytes("mc2-session-terrain-v1\0"));
            Append(0);
            Append(1);
            Append(0);
            Append(1);
            AppendBytes(types);
            AppendBytes(heights);
            AppendBytes(shading);
            AppendBytes(angles);
            AppendBytes(secondary);
            foreach (var entityIndex in entityIndices)
            {
                var value = unchecked((ushort)entityIndex);
                Append((byte)value);
                Append((byte)(value >> 8));
            }
            return hash;
        }

        private void RefreshLiveTerrain()
        {
            var next = session.TerrainState();
            Require(
                next.generation == snapshot.generation &&
                next.tick == snapshot.tick,
                "live terrain matches atomic flight snapshot");
            var changed =
                next.mutationSequence != terrainState.mutationSequence ||
                next.terrainHash != terrainState.terrainHash;
            if (!changed)
            {
                terrainState = next;
                return;
            }

            var nextTypes = session.BytePlane(NativeSim.TerrainTypes);
            var nextHeights = session.BytePlane(NativeSim.TerrainHeight);
            var nextShading = session.BytePlane(NativeSim.TerrainShade);
            var nextAngles = session.BytePlane(NativeSim.TerrainAngle);
            var nextSecondary = session.BytePlane(NativeSim.TerrainSecondary);
            var nextEntityIndices = session.EntityIndexPlane();
            Require(
                TerrainHash(
                    nextTypes,
                    nextHeights,
                    nextShading,
                    nextAngles,
                    nextSecondary,
                    nextEntityIndices) == next.terrainHash,
                "mutated live terrain canonical hash");

            terrainTypes = nextTypes;
            groundHeights = nextHeights;
            terrainShade = nextShading;
            terrainAngles = nextAngles;
            secondaryHeights = nextSecondary;
            terrainEntityIndices = nextEntityIndices;
            terrainState = next;
            ApplyLiveTerrainPresentation();
            Debug.Log(
                $"MC_TERRAIN_MUTATION_APPLIED sequence={terrainState.mutationSequence} " +
                $"hash=0x{terrainState.terrainHash:x16}");
        }

        private void ApplyLiveTerrainPresentation()
        {
            UploadPlaneTexture(typeTexture, terrainTypes);
            UploadPlaneTexture(heightTexture, groundHeights);
            UploadPlaneTexture(shadeTexture, terrainShade);
            UploadPlaneTexture(angleTexture, terrainAngles);
            UploadPlaneTexture(secondaryTexture, secondaryHeights);
            UpdatePersistentTerrainMesh();
            UpdateDiagnosticMinimap();
            UpdateExactTerrainGrid();
        }

        private static void UploadPlaneTexture(Texture2D texture, byte[] values)
        {
            if (texture == null) return;
            texture.LoadRawTextureData(values);
            texture.Apply(false, false);
        }

        private void UpdatePersistentTerrainMesh()
        {
            if (terrainMesh == null) return;
            var vertexCount = TerrainCells * TerrainCells * 4;
            var vertices = new Vector3[vertexCount];
            var colors = new Color[vertexCount];
            var uvs = new Vector2[vertexCount];
            var selectors = new Vector2[vertexCount];
            var shades = new Vector2[vertexCount];
            for (var y = 0; y < TerrainCells; y++)
            {
                for (var x = 0; x < TerrainCells; x++)
                {
                    var sourceIndex = x + y * TerrainCells;
                    var lowerLeft = sourceIndex * 4;
                    var lowerRight = lowerLeft + 1;
                    var upperLeft = lowerLeft + 2;
                    var upperRight = lowerLeft + 3;
                    vertices[lowerLeft] = TerrainVertex(x, y);
                    vertices[lowerRight] = TerrainVertex(x + 1, y);
                    vertices[upperLeft] = TerrainVertex(x, y + 1);
                    vertices[upperRight] = TerrainVertex(x + 1, y + 1);
                    var diagnostic = DiagnosticTerrainColor(
                        terrainTypes[sourceIndex],
                        terrainShade[sourceIndex],
                        groundHeights[sourceIndex]);
                    colors[lowerLeft] = colors[lowerRight] =
                        colors[upperLeft] = colors[upperRight] = diagnostic;
                    var orientation = (terrainAngles[sourceIndex] >> 4) & 7;
                    var cellUvs = TerrainOrientationUvs[orientation];
                    uvs[lowerLeft] = cellUvs[0];
                    uvs[lowerRight] = cellUvs[1];
                    uvs[upperLeft] = cellUvs[2];
                    uvs[upperRight] = cellUvs[3];
                    var selector = new Vector2(terrainTypes[sourceIndex], 0f);
                    selectors[lowerLeft] = selectors[lowerRight] =
                        selectors[upperLeft] = selectors[upperRight] = selector;
                    shades[lowerLeft] =
                        new Vector2(TerrainPlaneValue(terrainShade, x, y), 0);
                    shades[lowerRight] =
                        new Vector2(TerrainPlaneValue(terrainShade, x + 1, y), 0);
                    shades[upperLeft] =
                        new Vector2(TerrainPlaneValue(terrainShade, x, y + 1), 0);
                    shades[upperRight] =
                        new Vector2(TerrainPlaneValue(terrainShade, x + 1, y + 1), 0);
                }
            }
            terrainMesh.vertices = vertices;
            terrainMesh.colors = colors;
            terrainMesh.uv = uvs;
            terrainMesh.uv2 = selectors;
            terrainMesh.uv3 = shades;
            terrainMesh.RecalculateNormals();
            terrainMesh.RecalculateBounds();
        }

        private void UpdateDiagnosticMinimap()
        {
            if (
                minimapTexture == null ||
                ownerAssetCache != null ||
                minimapTexture.width != TerrainCells ||
                minimapTexture.height != TerrainCells)
                return;
            var pixels = new Color32[TerrainCells * TerrainCells];
            for (var index = 0; index < pixels.Length; index++)
            {
                pixels[index] = DiagnosticTerrainColor(
                    terrainTypes[index],
                    terrainShade[index],
                    groundHeights[index]);
            }
            minimapTexture.SetPixels32(pixels);
            minimapTexture.Apply(false, false);
        }

        private void CreateTerrainCopies()
        {
            var vertexCount = TerrainCells * TerrainCells * 4;
            var vertices = new Vector3[vertexCount];
            var colors = new Color[vertices.Length];
            var uvs = new Vector2[vertices.Length];
            var selectors = new Vector2[vertices.Length];
            var shades = new Vector2[vertices.Length];
            var mapVertices = new Vector2[vertices.Length];
            var mapCells = new Vector2[vertices.Length];
            var triangles = new int[TerrainCells * TerrainCells * 6];
            var triangle = 0;
            for (var y = 0; y < TerrainCells; y++)
            {
                for (var x = 0; x < TerrainCells; x++)
                {
                    var sourceIndex = x + y * TerrainCells;
                    var lowerLeft = sourceIndex * 4;
                    var lowerRight = lowerLeft + 1;
                    var upperLeft = lowerLeft + 2;
                    var upperRight = lowerLeft + 3;
                    vertices[lowerLeft] = TerrainVertex(x, y);
                    vertices[lowerRight] = TerrainVertex(x + 1, y);
                    vertices[upperLeft] = TerrainVertex(x, y + 1);
                    vertices[upperRight] = TerrainVertex(x + 1, y + 1);
                    var diagnostic = DiagnosticTerrainColor(
                        terrainTypes[sourceIndex],
                        terrainShade[sourceIndex],
                        groundHeights[sourceIndex]);
                    colors[lowerLeft] = colors[lowerRight] =
                        colors[upperLeft] = colors[upperRight] = diagnostic;
                    // These are the eight base D4 transforms in remc2's
                    // UVTable_D4350. The low two camera-relative table bits are
                    // unnecessary in a persistent world-space Unity mesh.
                    var orientation = (terrainAngles[sourceIndex] >> 4) & 7;
                    var cellUvs = TerrainOrientationUvs[orientation];
                    uvs[lowerLeft] = cellUvs[0];
                    uvs[lowerRight] = cellUvs[1];
                    uvs[upperLeft] = cellUvs[2];
                    uvs[upperRight] = cellUvs[3];
                    var selector = new Vector2(terrainTypes[sourceIndex], 0f);
                    selectors[lowerLeft] = selectors[lowerRight] =
                        selectors[upperLeft] = selectors[upperRight] = selector;
                    shades[lowerLeft] = new Vector2(TerrainPlaneValue(terrainShade, x, y), 0);
                    shades[lowerRight] =
                        new Vector2(TerrainPlaneValue(terrainShade, x + 1, y), 0);
                    shades[upperLeft] =
                        new Vector2(TerrainPlaneValue(terrainShade, x, y + 1), 0);
                    shades[upperRight] =
                        new Vector2(TerrainPlaneValue(terrainShade, x + 1, y + 1), 0);
                    mapVertices[lowerLeft] = TerrainMapUv(x, y);
                    mapVertices[lowerRight] = TerrainMapUv(x + 1, y);
                    mapVertices[upperLeft] = TerrainMapUv(x, y + 1);
                    mapVertices[upperRight] = TerrainMapUv(x + 1, y + 1);
                    var mapCell = TerrainMapUv(x, y);
                    mapCells[lowerLeft] = mapCells[lowerRight] =
                        mapCells[upperLeft] = mapCells[upperRight] = mapCell;
                    if (((x + y) & 1) == 0)
                    {
                        triangles[triangle++] = lowerLeft;
                        triangles[triangle++] = upperLeft;
                        triangles[triangle++] = upperRight;
                        triangles[triangle++] = lowerLeft;
                        triangles[triangle++] = upperRight;
                        triangles[triangle++] = lowerRight;
                    }
                    else
                    {
                        triangles[triangle++] = lowerLeft;
                        triangles[triangle++] = upperLeft;
                        triangles[triangle++] = lowerRight;
                        triangles[triangle++] = upperLeft;
                        triangles[triangle++] = upperRight;
                        triangles[triangle++] = lowerRight;
                    }
                }
            }
            terrainMesh = new Mesh
            {
                name = "Exact 257x257 Wrapped Level One Mesh",
                indexFormat = IndexFormat.UInt32,
                vertices = vertices,
                colors = colors,
                uv = uvs,
                uv2 = selectors,
                uv3 = shades,
                uv4 = mapVertices,
                uv5 = mapCells,
                triangles = triangles
            };
            terrainMesh.RecalculateNormals();
            terrainMesh.RecalculateBounds();
            var material = ownerAssetCache == null
                ? CreateMaterial(Color.white, 0.12f, 0f)
                : CreateExactOwnerTerrainMaterial();
            for (var tileY = -1; tileY <= 1; tileY++)
            {
                for (var tileX = -1; tileX <= 1; tileX++)
                {
                    var tile = new GameObject($"Exact Terrain Copy {tileX},{tileY}");
                    tile.AddComponent<MeshFilter>().sharedMesh = terrainMesh;
                    tile.AddComponent<MeshRenderer>().sharedMaterial = material;
                    tile.transform.position = new Vector3(
                        tileX * MapUnitySize,
                        0f,
                        tileY * MapUnitySize);
                    terrainTiles.Add(tile.transform);
                }
            }
        }

        private Vector3 TerrainVertex(int x, int y)
        {
            var sourceIndex = (x & 255) + (y & 255) * TerrainCells;
            return SourceToUnity(
                x * TerrainSourceStep,
                y * TerrainSourceStep,
                groundHeights[sourceIndex] * 32);
        }

        private static byte TerrainPlaneValue(byte[] plane, int x, int y)
        {
            return plane[(x & 255) + (y & 255) * TerrainCells];
        }

        private static Vector2 TerrainMapUv(int x, int y)
        {
            return new Vector2(
                ((x & 255) + 0.5f) / TerrainCells,
                ((y & 255) + 0.5f) / TerrainCells);
        }

        private void MeasureCarpetTerrainClearance()
        {
            if (carpetVisual == null || groundHeights == null) return;
            for (var z = -1; z <= 1; z += 2)
            {
                for (var x = -1; x <= 1; x += 2)
                {
                    var corner = carpetVisual.TransformPoint(
                        new Vector3(0.5f * x, -0.5f, 0.5f * z));
                    var sourceX = corner.x * SourceUnitsPerUnityUnit;
                    var sourceY = corner.z * SourceUnitsPerUnityUnit;
                    var terrain = DisplayedTerrainHeightSource(sourceX, sourceY);
                    var clearance =
                        corner.y * SourceUnitsPerUnityUnit - terrain;
                    minimumCarpetTerrainClearanceSource = Mathf.Min(
                        minimumCarpetTerrainClearanceSource,
                        clearance);
                }
            }
        }

        /// <summary>
        /// Height of the actual alternating-diagonal Unity terrain mesh, in
        /// source units. This deliberately uses 256 for the odd upper triangle;
        /// the collision sampler's recovered 255 rule is qualified separately.
        /// </summary>
        private float DisplayedTerrainHeightSource(float sourceX, float sourceY)
        {
            var wrappedX = Mathf.Repeat(sourceX, 65536f);
            var wrappedY = Mathf.Repeat(sourceY, 65536f);
            var tileX = Mathf.FloorToInt(wrappedX / TerrainSourceStep) & 255;
            var tileY = Mathf.FloorToInt(wrappedY / TerrainSourceStep) & 255;
            var fractionX = wrappedX - tileX * TerrainSourceStep;
            var fractionY = wrappedY - tileY * TerrainSourceStep;
            var h00 = 32f * TerrainPlaneValue(groundHeights, tileX, tileY);
            var h10 = 32f * TerrainPlaneValue(groundHeights, tileX + 1, tileY);
            var h01 = 32f * TerrainPlaneValue(groundHeights, tileX, tileY + 1);
            var h11 = 32f * TerrainPlaneValue(groundHeights, tileX + 1, tileY + 1);
            var x = fractionX / TerrainSourceStep;
            var y = fractionY / TerrainSourceStep;
            if (((tileX + tileY) & 1) == 0)
            {
                return fractionX <= fractionY
                    ? h00 + y * (h01 - h00) + x * (h11 - h01)
                    : h00 + y * (h11 - h10) + x * (h10 - h00);
            }
            return fractionX + fractionY >= TerrainSourceStep
                ? h01 + (1f - y) * (h10 - h11) + x * (h11 - h01)
                : h00 + y * (h01 - h00) + x * (h10 - h00);
        }

        private Material CreateExactOwnerTerrainMaterial()
        {
            var descriptor = JsonUtility.FromJson<TerrainMaterialDescriptor>(
                ownerAssetCache.ReadOutputText("terrain/materials.json"));
            Require(
                descriptor != null &&
                descriptor.schema == "mc2-level1-night-materials/v1" &&
                descriptor.bl32n != null &&
                descriptor.bl32n.indices_path == "terrain/bl32n.indices.u8" &&
                descriptor.bl32n.tile_width == 32 &&
                descriptor.bl32n.tile_height == 32 &&
                descriptor.bl32n.tile_count == 152 &&
                descriptor.bl32n.terrain_selector ==
                    "terrain_type_plane_u8_direct_tile_index" &&
                descriptor.bl32n.level1_used_selector_count == 65 &&
                descriptor.bl32n.level1_used_selector_max == 139 &&
                ExactSelectorList(descriptor.bl32n.level1_used_selectors) &&
                descriptor.palette != null &&
                descriptor.palette.rgb6_to_rgb8 ==
                    "channel_u8 = channel_rgb6 * 4" &&
                descriptor.palette.rgb8_path == "palette/paln.rgb8" &&
                descriptor.rendering_scope != null &&
                descriptor.rendering_scope.claim ==
                    "indexed_night_material_and_static_shade_bridge",
                "qualified Night BL32N descriptor");
            Require(
                ExactUvDescriptor(descriptor.terrain_uv),
                "source-locked terrain UV correspondence");
            Require(
                descriptor.tmaps1 != null &&
                descriptor.tmaps1.frame_used_selectors != null &&
                descriptor.tmaps1.frame_used_selectors.Length == 4 &&
                descriptor.tmaps1.frame_used_selectors[0] == 40 &&
                descriptor.tmaps1.frame_used_selectors[1] == 41 &&
                descriptor.tmaps1.frame_used_selectors[2] == 54 &&
                descriptor.tmaps1.frame_used_selectors[3] == 63 &&
                descriptor.tmaps1.scene_reachable_selectors != null &&
                descriptor.tmaps1.scene_reachable_selectors.Length == 172 &&
                descriptor.tmaps1.textures != null &&
                descriptor.tmaps1.textures.Length == 172,
                "qualified Level 1 TMAP selector closure");
            terrainMaterialDescriptor = descriptor;
            tmapDescriptors.Clear();
            foreach (var row in descriptor.tmaps1.textures)
            {
                Require(!tmapDescriptors.ContainsKey(row.index),
                    $"unique TMAP descriptor {row.index}");
                tmapDescriptors.Add(row.index, row);
            }
            foreach (var selector in descriptor.tmaps1.frame_used_selectors)
            {
                var row = Array.Find(
                    descriptor.tmaps1.textures,
                    value => value.index == selector);
                Require(
                    row != null &&
                    row.status == "decoded_indexed_texture" &&
                    !row.animated &&
                    row.frame_count == 1 &&
                    !string.IsNullOrEmpty(row.indices_path) &&
                    !string.IsNullOrEmpty(row.decoded_payload_path),
                    $"qualified frame-used TMAP selector {selector}");
                // Touch both validated representations. The indexed first frame
                // serves rendering; the complete decoded payload preserves exact
                // animation/delta-stream semantics for later entity presentation.
                Require(
                    ownerAssetCache.ReadOutput(row.indices_path).Length ==
                        row.width * row.height &&
                    ownerAssetCache.ReadOutput(row.decoded_payload_path).Length >=
                        6 + row.width * row.height,
                    $"qualified frame-used TMAP bytes {selector}");
            }

            var paletteRgb8 = ownerAssetCache.ReadOutput(descriptor.palette.rgb8_path);
            Require(
                paletteRgb8.Length == 256 * 3 && Array.TrueForAll(
                    paletteRgb8, value => value <= 252),
                "qualified PALN exact RGB6-times-four palette bytes");
            ownerOriginalPaletteRgb8 = (byte[])paletteRgb8.Clone();
            ownerPaletteTexture = new Texture2D(
                256, 1, TextureFormat.RGB24, false, false)
            {
                name = "Exact owner PALN-0 ordered palette",
                filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Clamp
            };
            ownerPaletteTexture.LoadRawTextureData(paletteRgb8);
            ownerPaletteTexture.Apply(false, false);

            var shadeLut = ownerAssetCache.ReadOutput(descriptor.palette.shade_lut_path);
            Require(shadeLut.Length == 256 * 256, "qualified TABLESN shade LUT bytes");
            Require(
                ownerAssetCache.ReadOutput(descriptor.palette.tablesn_path).Length == 83456 &&
                ownerAssetCache.ReadOutput(descriptor.palette.clrn_path).Length == 4096,
                "qualified complete TABLESN and CLRN bytes");
            ownerShadeLutTexture = new Texture2D(
                256, 256, TextureFormat.R8, false, true)
            {
                name = "Exact owner TABLESN static terrain shade LUT",
                filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Clamp
            };
            ownerShadeLutTexture.LoadRawTextureData(shadeLut);
            ownerShadeLutTexture.Apply(false, true);

            var bank = ownerAssetCache.ReadOutput(descriptor.bl32n.indices_path);
            Require(bank.Length == 152 * 32 * 32, "qualified BL32N tile bank bytes");
            Require(
                SystemInfo.supports2DArrayTextures,
                "GPU support for exact indexed BL32N texture array");
            ownerTerrainTiles = new Texture2DArray(
                32, 32, 152, TextureFormat.R8, false, true)
            {
                name = "Exact owner BL32N-0 indexed tiles",
                filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Repeat
            };
            for (var slice = 0; slice < 152; slice++)
            {
                ownerTerrainTiles.SetPixelData(
                    bank, 0, slice, slice * 32 * 32);
            }
            ownerTerrainTiles.Apply(false, true);
            Require(
                ownerTerrainTiles.format == TextureFormat.R8 &&
                ownerTerrainTiles.mipmapCount == 1 &&
                ownerTerrainTiles.filterMode == FilterMode.Point,
                "BL32N GPU texture is point-sampled R8 with no mipmaps");

            var shader = Resources.Load<Shader>("Terrain");
            Require(shader != null, "indexed owner terrain shader");
            var material = new Material(shader)
            {
                name = "Level 1 indexed Night material and static shade bridge"
            };
            material.SetTexture("_TileArray", ownerTerrainTiles);
            material.SetTexture("_ShadeLut", ownerShadeLutTexture);
            material.SetTexture("_Palette", ownerPaletteTexture);
            terrainPnt5RowsTexture = CreateDynamicTerrainTexture(
                "Exact pnt5 rows", TextureFormat.RFloat);
            terrainSurfaceDisplacementTexture = CreateDynamicTerrainTexture(
                "Exact water surface displacement", TextureFormat.RFloat);
            terrainGridCellValidTexture = CreateDynamicTerrainTexture(
                "Exact submitted terrain cells", TextureFormat.R8);
            material.SetTexture("_Pnt5Rows", terrainPnt5RowsTexture);
            material.SetTexture(
                "_SurfaceDisplacement", terrainSurfaceDisplacementTexture);
            material.SetTexture("_GridCellValid", terrainGridCellValidTexture);
            material.SetFloat("_SourceUnitsPerUnityUnit", SourceUnitsPerUnityUnit);
            ownerTerrainMaterial = material;
            UpdateExactTerrainGrid();
            RenderSettings.fog = false;
            ValidateIndexedGpuGolden(
                ownerTerrainTiles, ownerShadeLutTexture, bank, shadeLut, paletteRgb8);
            ValidatePnt5GpuGolden(ownerShadeLutTexture, shadeLut);
            Debug.Log(
                "MC_MATERIAL_BRIDGE VERIFIED bl32=152 selectors_used=65 " +
                "palette=PALN shade=TABLESN-live-pnt5 uv=UVTable-D4 " +
                "tmaps_frame=40,41,54,63 sceneTmaps=172 decoded=161 placeholders=11 " +
                "grid=40x21 water=source-order fog=source-distance " +
                "deferred=screen-linear-mode5-raster");
            return material;
        }

        private static Texture2D CreateDynamicTerrainTexture(
            string name,
            TextureFormat format)
        {
            return new Texture2D(256, 256, format, false, true)
            {
                name = name,
                filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Repeat
            };
        }

        private void UpdateExactTerrainGrid()
        {
            if (ownerTerrainMaterial == null) return;
            var grid = session.TerrainGrid();
            Require(grid.Length == 840, "exact outdoor terrain grid count");
            Array.Clear(terrainPnt5Rows, 0, terrainPnt5Rows.Length);
            Array.Clear(
                terrainSurfaceDisplacement,
                0,
                terrainSurfaceDisplacement.Length);
            Array.Clear(terrainGridVertexValid, 0, terrainGridVertexValid.Length);
            Array.Clear(terrainGridCellValid, 0, terrainGridCellValid.Length);

            var uniqueTiles = new HashSet<int>();
            for (var index = 0; index < grid.Length; index++)
            {
                var vertex = grid[index];
                Require(
                    vertex.abiVersion == NativeSim.AbiVersion &&
                    vertex.structSize == 64 &&
                    vertex.index == index &&
                    vertex.row == index / 40 &&
                    vertex.column == index % 40 &&
                    vertex.pnt5Written == (
                        (vertex.featureFlags & 0x0002) == 0 ? 1 : 0) &&
                    vertex.reserved0 == 0 &&
                    vertex.reserved1 == 0 &&
                    vertex.reserved2 == 0 &&
                    vertex.reserved3 == 0,
                    $"complete terrain grid vertex {index}");
                var mapIndex = vertex.tileX + vertex.tileY * TerrainCells;
                Require(uniqueTiles.Add(mapIndex), $"unique terrain grid tile {index}");
                terrainSurfaceDisplacement[mapIndex] =
                    vertex.surfaceDisplacement;
                if (vertex.pnt5Written == 0) continue;
                Require(
                    vertex.tablesnRow == ((vertex.pnt5 >> 16) & 0xff),
                    $"terrain grid TABLESN row {index}");
                terrainPnt5Rows[mapIndex] = vertex.pnt5 / 65536f;
                terrainGridVertexValid[mapIndex] = 255;
            }

            var submittedCells = 0;
            for (var y = 0; y < TerrainCells; y++)
            {
                var nextY = (y + 1) & 255;
                for (var x = 0; x < TerrainCells; x++)
                {
                    var nextX = (x + 1) & 255;
                    var i00 = x + y * TerrainCells;
                    var i10 = nextX + y * TerrainCells;
                    var i01 = x + nextY * TerrainCells;
                    var i11 = nextX + nextY * TerrainCells;
                    if (
                        terrainGridVertexValid[i00] != 0 &&
                        terrainGridVertexValid[i10] != 0 &&
                        terrainGridVertexValid[i01] != 0 &&
                        terrainGridVertexValid[i11] != 0)
                    {
                        terrainGridCellValid[i00] = 255;
                        submittedCells++;
                    }
                }
            }
            terrainPnt5RowsTexture.SetPixelData(terrainPnt5Rows, 0);
            terrainPnt5RowsTexture.Apply(false, false);
            terrainSurfaceDisplacementTexture.SetPixelData(
                terrainSurfaceDisplacement, 0);
            terrainSurfaceDisplacementTexture.Apply(false, false);
            terrainGridCellValidTexture.SetPixelData(terrainGridCellValid, 0);
            terrainGridCellValidTexture.Apply(false, false);
            if (!terrainGridLiveLogged)
            {
                terrainGridLiveLogged = true;
                Debug.Log(
                    "MC_TERRAIN_GRID_LIVE PASS vertices=840 " +
                    $"uniqueTiles={uniqueTiles.Count} submittedCells={submittedCells} " +
                    "formation=source-fixed16.16 pnt5=exact " +
                    "raster=Unity-perspective/deferred");
            }
        }

        private void ValidatePnt5GpuGolden(
            Texture2D shadeLutTexture,
            byte[] shadeLut)
        {
            Require(session.TerrainPnt5VectorCount() == 13,
                "native pnt5 qualification vector count");
            var shader = Resources.Load<Shader>("TerrainPnt5Golden");
            Require(shader != null, "pnt5 TABLESN GPU golden shader");
            var material = new Material(shader)
            {
                name = "Source pnt5 TABLESN qualification material"
            };
            material.SetTexture("_ShadeLut", shadeLutTexture);
            var target = RenderTexture.GetTemporary(
                1, 1, 0, RenderTextureFormat.ARGB32, RenderTextureReadWrite.Linear);
            var prior = RenderTexture.active;
            var readback = new Texture2D(1, 1, TextureFormat.RGBA32, false, true);
            var observedRows = new byte[13];
            try
            {
                for (uint index = 0; index < 13; index++)
                {
                    var vector = session.TerrainPnt5VectorAt(index);
                    Require(
                        vector.abiVersion == NativeSim.AbiVersion &&
                        vector.structSize == 56 &&
                        (vector.visible == 0 || vector.visible == 1) &&
                        vector.visible == (
                            vector.viewY > -256 &&
                            vector.distanceSquared < 26214400u ? 1 : 0) &&
                        vector.tablesnRow == ((vector.pnt5 >> 16) & 0xff) &&
                        vector.landDisplacement == (
                            (vector.angle & 8) != 0 ? -(vector.wave >> 10) : 0) &&
                        vector.lightingWave == (
                            (vector.angle & 8) != 0 && vector.shade < 56
                                ? vector.wave
                                : 0) &&
                        vector.reserved != null &&
                        Array.TrueForAll(vector.reserved, value => value == 0),
                        $"complete native pnt5 vector {index}");
                    var sourceIndex = (byte)((index * 17 + 5) & 0xff);
                    var expected =
                        shadeLut[vector.tablesnRow * 256 + sourceIndex];
                    material.SetFloat("_GoldenSourceIndex", sourceIndex);
                    material.SetFloat("_GoldenPnt5Rows", vector.pnt5 / 65536f);
                    Graphics.Blit(Texture2D.blackTexture, target, material, 0);
                    RenderTexture.active = target;
                    readback.ReadPixels(new Rect(0, 0, 1, 1), 0, 0, false);
                    readback.Apply(false, false);
                    var observed = readback.GetRawTextureData<byte>()[0];
                    Require(
                        observed == expected,
                        $"pnt5 GPU golden {index} row={vector.tablesnRow} " +
                        $"source={sourceIndex} expected={expected} observed={observed}");
                    observedRows[index] = vector.tablesnRow;
                }
                Require(
                    new HashSet<byte>(observedRows).Count >= 5,
                    "pnt5 vectors span distinct TABLESN rows");
                Debug.Log(
                    "MC_PNT5_GPU_GOLDEN PASS vectors=13 " +
                    $"rows={string.Join(",", observedRows)} " +
                    "contract=source-pnt5-bits16..23/TABLESN");
            }
            finally
            {
                RenderTexture.active = prior;
                RenderTexture.ReleaseTemporary(target);
                Destroy(readback);
                Destroy(material);
            }
        }

        private static void ValidateIndexedGpuGolden(
            Texture2DArray tileArray,
            Texture2D shadeLutTexture,
            byte[] bank,
            byte[] shadeLut,
            byte[] paletteRgb8)
        {
            const int brightness = 40;
            var selectorFiveSources = new HashSet<byte>();
            var selectorFiveShaded = new HashSet<byte>();
            for (var texel = 0; texel < 32 * 32; texel++)
            {
                var source = bank[5 * 32 * 32 + texel];
                selectorFiveSources.Add(source);
                selectorFiveShaded.Add(shadeLut[brightness * 256 + source]);
            }
            Require(
                selectorFiveSources.SetEquals(new byte[] { 133, 134, 146, 191, 209, 223 }) &&
                selectorFiveShaded.SetEquals(new byte[] { 91, 139, 140, 141, 143, 144 }),
                "CPU selector-5 source and TABLESN shade-40 diversity");
            Debug.Log(
                "MC_CPU_MATERIAL_DIVERSITY PASS selector=5 texels=1024 " +
                "source_unique=6 source=133,134,146,191,209,223 " +
                "shade40_unique=6 shaded=91,139,140,141,143,144");

            var goldenShader = Resources.Load<Shader>("TerrainGolden");
            Require(goldenShader != null, "isolated indexed terrain GPU golden shader");
            var goldenMaterial = new Material(goldenShader)
            {
                name = "One-pixel indexed terrain qualification material"
            };
            goldenMaterial.SetTexture("_TileArray", tileArray);
            goldenMaterial.SetTexture("_ShadeLut", shadeLutTexture);
            goldenMaterial.SetFloat("_GoldenBrightness", brightness);

            var selectors = new[] { 5, 5, 22, 139 };
            var xs = new[] { 7, 0, 7, 7 };
            var ys = new[] { 11, 0, 11, 11 };
            var observedResults = new byte[selectors.Length];
            var sourceResults = new byte[selectors.Length];
            var target = RenderTexture.GetTemporary(
                1, 1, 0, RenderTextureFormat.ARGB32, RenderTextureReadWrite.Linear);
            var prior = RenderTexture.active;
            var readback = new Texture2D(1, 1, TextureFormat.RGBA32, false, true);
            try
            {
                for (var sample = 0; sample < selectors.Length; sample++)
                {
                    var selector = selectors[sample];
                    var x = xs[sample];
                    var y = ys[sample];
                    var sourceIndex = bank[selector * 32 * 32 + y * 32 + x];
                    var expected = shadeLut[brightness * 256 + sourceIndex];
                    goldenMaterial.SetFloat("_GoldenSelector", selector);
                    goldenMaterial.SetFloat("_GoldenU", (x + 0.5f) / 32f);
                    goldenMaterial.SetFloat("_GoldenV", (y + 0.5f) / 32f);
                    Graphics.Blit(Texture2D.blackTexture, target, goldenMaterial, 0);
                    RenderTexture.active = target;
                    readback.ReadPixels(new Rect(0, 0, 1, 1), 0, 0, false);
                    readback.Apply(false, false);
                    var observed = readback.GetRawTextureData<byte>()[0];
                    Require(
                        observed == expected,
                        $"GPU indexed shade golden sample {sample} expected {expected} " +
                        $"observed {observed}");
                    sourceResults[sample] = sourceIndex;
                    observedResults[sample] = observed;
                }
                Require(
                    new HashSet<byte>(observedResults).Count == selectors.Length,
                    "GPU indexed shade samples retain distinct tile detail");
                var first = observedResults[0] * 3;
                Debug.Log(
                    "MC_GPU_MATERIAL_GOLDEN PASS samples=4 brightness=40 " +
                    $"source_indices={sourceResults[0]},{sourceResults[1]}," +
                    $"{sourceResults[2]},{sourceResults[3]} " +
                    $"results={observedResults[0]},{observedResults[1]}," +
                    $"{observedResults[2]},{observedResults[3]} unique=4 " +
                    $"first_rgb={paletteRgb8[first]},{paletteRgb8[first + 1]}," +
                    $"{paletteRgb8[first + 2]}");
            }
            finally
            {
                RenderTexture.active = prior;
                RenderTexture.ReleaseTemporary(target);
                Destroy(readback);
                Destroy(goldenMaterial);
            }
        }

        private static bool ExactSelectorList(int[] values)
        {
            if (values == null || values.Length != ExpectedLevel1TerrainSelectors.Length)
                return false;
            for (var index = 0; index < values.Length; index++)
            {
                if (values[index] != ExpectedLevel1TerrainSelectors[index]) return false;
            }
            return true;
        }

        private static bool ExactUvDescriptor(TerrainUvDescriptor descriptor)
        {
            if (descriptor == null ||
                descriptor.source_row_count != 32 ||
                descriptor.persistent_group_expression != "(terrain_angle_u8 >> 4) & 7" ||
                descriptor.persistent_unity_group_records == null ||
                descriptor.persistent_unity_group_records.Length != 8)
                return false;
            for (var group = 0; group < 8; group++)
            {
                var record = descriptor.persistent_unity_group_records[group];
                if (record == null || record.group != group ||
                    record.points_flat == null || record.points_flat.Length != 8)
                    return false;
                for (var point = 0; point < 4; point++)
                {
                    if (record.points_flat[point * 2] !=
                            (int)TerrainOrientationUvs[group][point].x ||
                        record.points_flat[point * 2 + 1] !=
                            (int)TerrainOrientationUvs[group][point].y)
                        return false;
                }
            }
            return true;
        }

        private static Color DiagnosticTerrainColor(byte type, byte shade, byte height)
        {
            var brightness = Mathf.Lerp(0.48f, 1.15f, shade / 255f);
            Color baseColor;
            if (type == 1) baseColor = new Color(0.025f, 0.19f, 0.38f);
            else if (type >= 10 && type <= 12) baseColor = new Color(0.60f, 0.10f, 0.02f);
            else baseColor = Color.Lerp(
                new Color(0.08f, 0.24f, 0.10f),
                new Color(0.52f, 0.43f, 0.27f),
                height / 255f);
            return baseColor * brightness;
        }

        private void CreateEntityRegistry()
        {
            var groupByEntity = new Dictionary<int, EntityPresentationGroup>();
            if (ownerAssetCache != null)
            {
                entityPresentationDescriptor =
                    JsonUtility.FromJson<EntityPresentationDescriptor>(
                        ownerAssetCache.ReadOutputText("entities/presentation.json"));
                Require(
                    entityPresentationDescriptor != null &&
                    entityPresentationDescriptor.schema ==
                        "mc2-level1-entity-presentations/v1" &&
                    entityPresentationDescriptor.active_reset_count == 177 &&
                    entityPresentationDescriptor.qualified_actor_count == 64 &&
                    entityPresentationDescriptor.mapped_reset_effect_count == 19 &&
                    entityPresentationDescriptor.reset_chain_drawable_count == 83 &&
                    entityPresentationDescriptor.qualified_view_count == 64 &&
                    entityPresentationDescriptor.suppressed_terrain_head_count == 47 &&
                    entityPresentationDescriptor.off_chain_active_count == 47 &&
                    entityPresentationDescriptor.groups != null &&
                    entityPresentationDescriptor.groups.Length == 17,
                    "qualified Level 1 entity presentation descriptor");
                foreach (var group in entityPresentationDescriptor.groups)
                {
                    Require(
                        group != null && group.entity_indices != null &&
                        group.entity_indices.Length == group.entity_count &&
                        group.reset_chain_drawable_entity_indices != null &&
                        group.reset_chain_drawable_entity_indices.Length ==
                            group.reset_chain_drawable_count &&
                        group.qualified_view_entity_indices != null &&
                        group.qualified_view_entity_indices.Length ==
                            group.qualified_view_count,
                        $"complete entity presentation group {group?.group_id}");
                    foreach (var index in group.entity_indices)
                    {
                        Require(!groupByEntity.ContainsKey(index),
                            $"unique entity presentation row {index}");
                        groupByEntity.Add(index, group);
                    }
                }
                Require(groupByEntity.Count == 177,
                    "177 exact entity presentation assignments");
                Require(
                    groupByEntity.TryGetValue(50, out var goatGroup) &&
                    goatGroup.group_id == "reset-050" &&
                    goatGroup.authority == "FrozenObserved" &&
                    goatGroup.entity_indices.Length == ExpectedLevel1GoatEntities.Length,
                    "exact Goat owner-evidence presentation group");
                for (var ordinal = 0; ordinal < ExpectedLevel1GoatEntities.Length; ordinal++)
                {
                    Require(
                        goatGroup.entity_indices[ordinal] ==
                            ExpectedLevel1GoatEntities[ordinal],
                        $"exact Goat owner-evidence member {ordinal}");
                }
            }

            var qualifiedSpriteCount = 0;
            var liveGoatSpriteCount = 0;
            var liveVillagerSpriteCount = 0;
            var qualifiedFrozenActorCount = 0;
            var resetEffectDebugCount = 0;
            var unresolvedDebugCount = 0;
            foreach (var entity in entities)
            {
                if (entity.active == 0) continue;
                groupByEntity.TryGetValue(entity.entityIndex, out var group);
                if (group != null)
                {
                    Require(
                        group.key.class_id == entity.classId &&
                        group.key.model == entity.model &&
                        group.key.action == entity.action &&
                        group.key.parameter_record_index ==
                            (entity.hasParameterRecord != 0
                                ? entity.parameterRecordIndex
                                : -1),
                        $"entity presentation key {entity.entityIndex}");
                }
                var semantic = group?.semantic_family ?? SemanticFamily(entity);
                // The presentation rig already carries the visible carpet.
                // A second marker at the camera origin would clip the view.
                var qualifiedSprite =
                    ownerAssetCache != null && group != null &&
                    Array.IndexOf(
                        group.qualified_view_entity_indices,
                        (int)entity.entityIndex) >= 0;
                var resetEffectDebug =
                    ownerAssetCache != null && debugEntities && group != null &&
                    Array.IndexOf(
                        group.reset_chain_drawable_entity_indices,
                        (int)entity.entityIndex) >= 0 &&
                    !qualifiedSprite;
                var visible = qualifiedSprite || resetEffectDebug || debugEntities ||
                              (ownerAssetCache == null && semantic != null &&
                               entity.authority != NativeSim.PlayerFlightLive);
                var authority = AuthorityLabel(entity);
                var node = new GameObject(
                    $"Entity {entity.entityIndex:D3} {semantic ?? "unresolved"} [{authority}]");
                Renderer renderer = null;
                Transform spriteVisual = null;
                var ownerSprite = qualifiedSprite || resetEffectDebug;
                var qualifiedFrozenActor =
                    qualifiedSprite &&
                    (entity.authority == NativeSim.FrozenObserved ||
                     entity.authority == NativeSim.VillagerLive);
                if (qualifiedFrozenActor)
                    qualifiedFrozenActorCount++;
                if (visible)
                {
                    if (ownerSprite)
                    {
                        spriteVisual = CreateOwnerEntitySprite(
                            entity, group, node.transform, out renderer);
                        if (qualifiedSprite)
                        {
                            qualifiedSpriteCount++;
                            if (entity.authority == NativeSim.GoatLive)
                                liveGoatSpriteCount++;
                            else if (entity.authority == NativeSim.VillagerLive)
                                liveVillagerSpriteCount++;
                        }
                        else resetEffectDebugCount++;
                    }
                    else
                    {
                        var marker = DiagnosticPrimitive(
                            MarkerPrimitive(semantic),
                            $"Unresolved diagnostic marker {entity.entityIndex:D3}",
                            MarkerColor(semantic, entity.authority),
                            node.transform);
                        marker.transform.localScale = MarkerScale(semantic);
                        renderer = marker.GetComponent<Renderer>();
                        unresolvedDebugCount++;
                    }
                }
                entityRegistry.Add(new EntityMarker
                {
                    Index = entity.entityIndex,
                    Node = node.transform,
                    Renderer = renderer,
                    Label = $"{entity.entityIndex:D3} {semantic ?? "unresolved"} {authority}",
                    Family = semantic,
                    Visible = visible,
                    Group = group,
                    SpriteVisual = spriteVisual,
                    OwnerSprite = ownerSprite,
                    QualifiedFrozenActor = qualifiedFrozenActor
                });
            }
            Require(entityRegistry.Count == 177, "177 entity registry nodes");
            if (!debugEntities)
            {
                Require(
                    entityRegistry.Find(marker => marker.Index == 152)?.Renderer == null,
                    "normal viewer hides duplicate player-origin marker");
                if (ownerAssetCache != null)
                {
                    Require(qualifiedSpriteCount == 64,
                        "qualified viewer has exactly 64 stable actor sprites");
                    Require(
                        liveGoatSpriteCount == 33 &&
                        liveVillagerSpriteCount == 28 &&
                        qualifiedSpriteCount - liveGoatSpriteCount -
                            liveVillagerSpriteCount == 3 &&
                        qualifiedFrozenActorCount == 31,
                        "qualified owner sprites split 33 GoatLive, 28 VillagerLive, and 3 FrozenObserved");
                    Require(resetEffectDebugCount == 0 && unresolvedDebugCount == 0,
                        "qualified viewer has no transient/control diagnostic proxies");
                }
            }
            Debug.Log(
                $"MC_ENTITY_PRESENTATION VERIFIED registry=177 qualifiedSprites=" +
                $"{qualifiedSpriteCount} goatLive={liveGoatSpriteCount} " +
                $"villagerLive={liveVillagerSpriteCount} " +
                $"frozenOwnerSprites={qualifiedFrozenActorCount} " +
                $"resetEffectDebug={resetEffectDebugCount} " +
                $"unresolvedDebug={unresolvedDebugCount} " +
                $"mode={(qualifiedMode ? "qualified" : "debug")} " +
                "qualifiedSpriteAuthority=33-GoatLive+28-VillagerLive+3-FrozenObserved " +
                "managedFallbacks=native-capability-gated " +
                "nativeAnimatedFamilies=Goat,Archer,Villager nativeStaticFamilies=Dolmen " +
                "pivot=source-indexed-compositor noPhysics=true");
            UpdateEntityPresentation();
        }

        private void CreateMissionPresentationRegistry()
        {
            missionPresentationRoot =
                new GameObject("Live Level 1 mission presentation").transform;
            missionPresentationRoot.SetParent(transform, false);
            if (ownerAssetCache == null)
            {
                Debug.Log(
                    "MC_LEVEL1_PRESENTATION absent reason=owner-cache-not-supplied");
                return;
            }
            Require(missionActors.Length == 45, "45 live Level 1 mission actors");
            foreach (var actor in missionActors)
            {
                Require(
                    !missionActorRegistry.ContainsKey(actor.authoredIndex),
                    $"unique live mission actor {actor.authoredIndex}");
                missionActorRegistry.Add(
                    actor.authoredIndex,
                    CreateMissionMarker(
                        $"Mission actor {actor.authoredIndex:D3} kind={actor.kind}",
                        actor.authoredIndex));
            }
            Require(
                missionActorRegistry.Count == 45,
                "45 separate live mission presentation nodes");
            Debug.Log(
                "MC_LEVEL1_PRESENTATION CREATED actors=45 multipart=dynamic " +
                "effects=snapshot-owned ownerTmaps=true noPhysics=true");
        }

        private MissionMarker CreateMissionMarker(string name, ulong identity)
        {
            var node = new GameObject(name).transform;
            node.SetParent(missionPresentationRoot, false);
            var visual = new GameObject("Exact owner TMAP sprite").transform;
            visual.SetParent(node, false);
            visual.gameObject.AddComponent<MeshFilter>().sharedMesh = EntityQuad(false);
            var renderer = visual.gameObject.AddComponent<MeshRenderer>();
            renderer.enabled = false;
            return new MissionMarker
            {
                Identity = identity,
                Node = node,
                Visual = visual,
                Renderer = renderer,
                Selector = -1
            };
        }

        private void UpdateMissionPresentation()
        {
            if (ownerAssetCache == null || missionPresentationRoot == null) return;

            foreach (var actor in missionActors)
            {
                Require(
                    missionActorRegistry.TryGetValue(
                        actor.authoredIndex, out var marker),
                    $"live mission actor presentation {actor.authoredIndex}");
                if (
                    actor.kind == Level1ExitSwitchActorKind ||
                    actor.kind == Level1CheckpointSmokeEmitterActorKind)
                {
                    // Exit switches and checkpoint-smoke parents are authored
                    // source entities with no sprite of their own.
                    marker.Renderer.enabled = false;
                    continue;
                }
                MissionActorSelector(actor, out var selector, out var mirrored);
                UpdateMissionMarker(
                    marker,
                    actor.x,
                    actor.y,
                    actor.z,
                    selector,
                    actor.logicalHeight,
                    mirrored,
                    ManagedMissionActorVisible(
                        actor.kind,
                        actor.active,
                        actor.visible,
                        terrainFrameInfo.flags));
            }

            var liveSegmentIds = new HashSet<ushort>();
            foreach (var segment in multipartSegments)
            {
                Require(
                    liveSegmentIds.Add(segment.sourceEntityIndex),
                    $"unique multipart segment source slot {segment.sourceEntityIndex}");
                if (!multipartSegmentRegistry.TryGetValue(
                        segment.sourceEntityIndex, out var marker))
                {
                    marker = CreateMissionMarker(
                        $"Multipart segment {segment.sourceEntityIndex:D3} " +
                        $"head={segment.headAuthoredIndex:D3} ordinal={segment.ordinal:D2}",
                        segment.sourceEntityIndex);
                    multipartSegmentRegistry.Add(segment.sourceEntityIndex, marker);
                }
                UpdateMissionMarker(
                    marker,
                    segment.x,
                    segment.y,
                    segment.z,
                    segment.baseTmap,
                    segment.logicalHeight,
                    false,
                    segment.active != 0 &&
                    segment.visible != 0 &&
                    (terrainFrameInfo.flags &
                     NativeSim.TerrainFrameFirefliesComposited) == 0);
            }
            RemoveMissingMultipartMarkers(liveSegmentIds);

            castleMarker ??= CreateMissionMarker("Level 1 stage-one castle", ulong.MaxValue - 1);
            UpdateMissionMarker(
                castleMarker,
                castleLifecycle.rootX,
                castleLifecycle.rootY,
                castleLifecycle.rootZ,
                level1Castle.baseTmap,
                level1Castle.logicalHeight,
                false,
                ManagedEconomyVisible(
                    castleLifecycle.rootActive,
                    terrainFrameInfo.flags));

            manaCarrierMarker ??=
                CreateMissionMarker("Level 1 mana carrier", ulong.MaxValue - 2);
            UpdateMissionMarker(
                manaCarrierMarker,
                manaCarrier.x,
                manaCarrier.y,
                manaCarrier.z,
                manaCarrier.baseTmap,
                manaCarrier.logicalHeight,
                false,
                ManagedEconomyVisible(manaCarrier.active, terrainFrameInfo.flags));

            var liveSpheres = new HashSet<uint>();
            foreach (var sphere in manaSpheres)
            {
                Require(
                    liveSpheres.Add(sphere.sphereId),
                    $"unique mana sphere {sphere.sphereId}");
                if (!manaSphereRegistry.TryGetValue(sphere.sphereId, out var marker))
                {
                    marker = CreateMissionMarker(
                        $"Mana sphere {sphere.sphereId} owner={sphere.ownerEntityIndex}",
                        sphere.sphereId);
                    manaSphereRegistry.Add(sphere.sphereId, marker);
                }
                UpdateMissionMarker(
                    marker,
                    sphere.x,
                    sphere.y,
                    sphere.z,
                    sphere.baseTmap,
                    sphere.logicalHeight,
                    false,
                    ManagedEconomyVisible(sphere.active, terrainFrameInfo.flags));
            }
            RemoveMissingMissionMarkers(manaSphereRegistry, liveSpheres);

            var liveProjectiles = new HashSet<uint>();
            foreach (var projectile in fireballProjectiles)
            {
                Require(
                    liveProjectiles.Add(projectile.projectileId),
                    $"unique Fireball projectile {projectile.projectileId}");
                if (!missionProjectileRegistry.TryGetValue(
                    projectile.projectileId, out var marker))
                {
                    marker = CreateMissionMarker(
                        $"Level 1 projectile {projectile.projectileId} " +
                        $"kind={projectile.projectileKind}",
                        projectile.projectileId);
                    missionProjectileRegistry.Add(projectile.projectileId, marker);
                }
                var selector = (int)projectile.baseTmap;
                var mirrored = false;
                if (projectile.projectileKind == 1 || projectile.projectileKind == 3)
                {
                    Require(
                        projectile.renderMode == 21 &&
                        projectile.baseTmap == 463 &&
                        projectile.animationFrame == 0,
                        $"Fireball projectile presentation {projectile.projectileId}");
                }
                else if (projectile.projectileKind == 2)
                {
                    Require(
                        projectile.renderMode == 19 &&
                        projectile.baseTmap == 105 &&
                        projectile.animationFrame == 0,
                        $"Archer arrow presentation {projectile.projectileId}");
                    var orientation =
                        (((projectile.yaw - renderCameraState.yaw) >> 3) & 0xF0) >> 4;
                    selector += YawLookupMode19[orientation];
                    mirrored = orientation >= 8;
                }
                else if (projectile.projectileKind == 4)
                {
                    Require(
                        projectile.renderMode == 21 &&
                        projectile.baseTmap == 61 &&
                        projectile.animationFrame == 0,
                        $"Castle projectile presentation {projectile.projectileId}");
                }
                else if (projectile.projectileKind == 5)
                {
                    Require(
                        projectile.renderMode == 21 &&
                        projectile.baseTmap == 144 &&
                        projectile.animationFrame == 0,
                        $"Possession projectile presentation {projectile.projectileId}");
                }
                else
                {
                    throw new InvalidOperationException(
                        $"unsupported Level 1 projectile kind {projectile.projectileKind}");
                }
                UpdateMissionMarker(
                    marker,
                    projectile.x,
                    projectile.y,
                    projectile.z,
                    selector,
                    projectile.logicalHeight,
                    mirrored,
                    ManagedMissionProjectileVisible(
                        projectile.projectileKind,
                        projectile.active,
                        terrainFrameInfo.flags));
            }
            RemoveMissingMissionMarkers(
                missionProjectileRegistry, liveProjectiles);

            var liveImpacts = new HashSet<uint>();
            foreach (var impact in fireballImpacts)
            {
                Require(
                    liveImpacts.Add(impact.impactId),
                    $"unique Fireball impact {impact.impactId}");
                if (!missionImpactRegistry.TryGetValue(
                    impact.impactId, out var marker))
                {
                    marker = CreateMissionMarker(
                        $"Fireball impact {impact.impactId}",
                        impact.impactId);
                    missionImpactRegistry.Add(impact.impactId, marker);
                }
                var supportedImpact =
                    impact.model == 0 &&
                    impact.renderMode == 36 &&
                    impact.baseTmap == 38 &&
                    impact.logicalHeight == 250 ||
                    impact.model == 5 &&
                    impact.renderMode == 11 &&
                    impact.baseTmap == 186 &&
                    impact.logicalHeight == 300 ||
                    impact.model == 12 &&
                    impact.renderMode == 21 &&
                    impact.baseTmap == 54 &&
                    impact.renderParameterIndex == 41 &&
                    impact.logicalHeight == 250 ||
                    impact.model == 13 &&
                    impact.renderMode == 21 &&
                    impact.baseTmap == 57 &&
                    impact.renderParameterIndex >= 67 &&
                    impact.renderParameterIndex <= 74 &&
                    impact.logicalHeight ==
                        (impact.renderParameterIndex - 66) * 50;
                Require(
                    supportedImpact &&
                    impact.animationFrame >= 0 &&
                    (impact.model == 13
                        ? impact.animationFrame >= 67 && impact.animationFrame <= 74
                        : impact.animationFrame <= 9),
                    $"class-10 effect presentation {impact.impactId}");
                UpdateMissionMarker(
                    marker,
                    impact.x,
                    impact.y,
                    impact.z,
                    impact.model == 12 || impact.model == 13
                        ? impact.baseTmap
                        : impact.baseTmap + impact.animationFrame,
                    impact.logicalHeight,
                    false,
                    ManagedMissionImpactVisible(
                        impact.active,
                        terrainFrameInfo.flags));
            }
            RemoveMissingMissionMarkers(missionImpactRegistry, liveImpacts);
        }

        private void MissionActorSelector(
            NativeSim.Level1ActorSnapshot actor,
            out int selector,
            out bool mirrored)
        {
            selector = actor.baseTmap;
            mirrored = false;
            if (actor.renderMode == 0 || actor.renderMode == 1)
            {
                Require(
                    actor.kind == 3 || actor.kind == 4 || actor.kind == 5 ||
                    actor.kind == Level1ExperienceScrollActorKind ||
                    actor.kind == Level1TreeActorKind ||
                    actor.kind == Level1StoneActorKind,
                    $"fixed-selector mission actor {actor.authoredIndex}");
                return;
            }
            var orientation =
                (((actor.yaw - renderCameraState.yaw) >> 3) & 0xF0) >> 4;
            if (actor.renderMode == 20)
            {
                Require(
                    actor.kind == 1,
                    $"mode20 Archer actor {actor.authoredIndex}");
                selector += YawLookupMode20[orientation];
                mirrored = orientation >= 8;
                return;
            }
            if (actor.renderMode == 17)
            {
                Require(
                    actor.kind == 2 || actor.kind == Level1MultipartHeadActorKind,
                    $"mode17 Firefly/multipart actor {actor.authoredIndex}");
                selector += orientation < 8 ? orientation : 15 - orientation;
                mirrored = orientation >= 8;
                return;
            }
            throw new InvalidOperationException(
                $"unsupported live mission actor render mode {actor.renderMode} " +
                $"for {actor.authoredIndex}");
        }

        internal static bool ManagedMissionActorVisible(
            byte actorKind,
            byte active,
            byte visible,
            uint worldFrameFlags)
        {
            if (active == 0 || visible == 0) return false;
            return actorKind switch
            {
                1 => (worldFrameFlags &
                      NativeSim.TerrainFrameArchersComposited) == 0,
                2 => (worldFrameFlags &
                      NativeSim.TerrainFrameFirefliesComposited) == 0,
                3 or 4 or 5 => (worldFrameFlags &
                                NativeSim.TerrainFrameObjectivePropsComposited) == 0,
                Level1MultipartHeadActorKind => (worldFrameFlags &
                    NativeSim.TerrainFrameFirefliesComposited) == 0,
                Level1ExperienceScrollActorKind => (worldFrameFlags &
                    NativeSim.TerrainFrameObjectivePropsComposited) == 0,
                Level1TreeActorKind or Level1StoneActorKind => (worldFrameFlags &
                    NativeSim.TerrainFrameObjectivePropsComposited) == 0,
                _ => true
            };
        }

        internal static bool ManagedMissionProjectileVisible(
            uint projectileKind,
            byte active,
            uint worldFrameFlags)
        {
            if (active == 0) return false;
            return (worldFrameFlags & NativeSim.TerrainFrameCombatComposited) == 0;
        }

        internal static bool ManagedMissionImpactVisible(
            byte active,
            uint worldFrameFlags)
        {
            return active != 0 &&
                   (worldFrameFlags & NativeSim.TerrainFrameCombatComposited) == 0;
        }

        internal static bool ManagedEconomyVisible(byte active, uint worldFrameFlags)
        {
            return active != 0 &&
                   (worldFrameFlags & NativeSim.TerrainFrameEconomyComposited) == 0;
        }

        internal static bool ManagedFrozenActorVisible(
            byte active,
            bool qualifiedFrozenActor,
            uint worldFrameFlags)
        {
            return active != 0 &&
                   (!qualifiedFrozenActor ||
                    (worldFrameFlags &
                     NativeSim.TerrainFrameFrozenActorsComposited) == 0);
        }

        private void UpdateMissionMarker(
            MissionMarker marker,
            ushort sourceX,
            ushort sourceY,
            short sourceZ,
            int selector,
            ushort logicalHeight,
            bool mirrored,
            bool active)
        {
            var nearestX = unwrappedSourceX +
                ModularDelta(sourceX, presentationState.cameraX);
            var nearestY = unwrappedSourceY +
                ModularDelta(sourceY, presentationState.cameraY);
            marker.Node.position = SourceToUnity(nearestX, nearestY, sourceZ);
            if (flightCamera != null)
                marker.Node.rotation = flightCamera.transform.rotation;
            if (!active)
            {
                marker.Renderer.enabled = false;
                return;
            }
            if (!tmapDescriptors.TryGetValue(selector, out var textureRow))
                throw new InvalidOperationException(
                    $"required invariant failed: live mission TMAP selector {selector}");
            Require(
                textureRow.status == "decoded_indexed_texture" &&
                textureRow.width > 0 &&
                textureRow.height > 0 &&
                logicalHeight > 0,
                $"decoded live mission selector {selector}");
            marker.Renderer.sharedMaterial =
                EntitySpriteMaterial(selector, textureRow);
            marker.Visual.GetComponent<MeshFilter>().sharedMesh =
                EntityQuad(mirrored);
            var height = logicalHeight / SourceUnitsPerUnityUnit;
            var width = height * textureRow.width / textureRow.height;
            marker.Visual.localPosition = new Vector3(0f, height * 0.5f, 0f);
            marker.Visual.localScale = new Vector3(width, height, 1f);
            marker.Selector = selector;
            marker.Renderer.enabled =
                presentationRig == null ||
                Vector3.SqrMagnitude(marker.Node.position - targetPosition) >= 1f;
        }

        private static void RemoveMissingMissionMarkers(
            Dictionary<uint, MissionMarker> registry,
            HashSet<uint> liveIds)
        {
            var removed = new List<uint>();
            foreach (var pair in registry)
            {
                if (liveIds.Contains(pair.Key)) continue;
                if (pair.Value.Node != null)
                    Destroy(pair.Value.Node.gameObject);
                removed.Add(pair.Key);
            }
            foreach (var id in removed) registry.Remove(id);
        }

        private void RemoveMissingMultipartMarkers(HashSet<ushort> liveIds)
        {
            var removed = new List<ushort>();
            foreach (var pair in multipartSegmentRegistry)
            {
                if (liveIds.Contains(pair.Key)) continue;
                if (pair.Value.Node != null)
                    Destroy(pair.Value.Node.gameObject);
                removed.Add(pair.Key);
            }
            foreach (var id in removed) multipartSegmentRegistry.Remove(id);
        }

        private Transform CreateOwnerEntitySprite(
            NativeSim.EntitySnapshot entity,
            EntityPresentationGroup group,
            Transform parent,
            out Renderer renderer)
        {
            Require(
                group.authority == "FrozenObserved" &&
                group.selector != null &&
                group.selector.bank == "TMAPS1-0" &&
                group.selector.qualified_selectors != null &&
                group.selector.qualified_selectors.Length > 0 &&
                group.particle != null &&
                group.particle.logical_height_source_units > 0 &&
                group.billboard != null &&
                group.billboard.facing == "full_camera_facing_screen_plane" &&
                group.billboard.anchor ==
                    "viewer_bottom_center_at_entity_xyz_approximation" &&
                group.billboard.source_anchor_status != null &&
                group.billboard.source_anchor_status.StartsWith(
                    "deferred: original DrawSprite_41BD3",
                    StringComparison.Ordinal),
                $"qualified owner sprite descriptor {entity.entityIndex}");
            var visual = new GameObject(
                $"Owner TMAP sprite {entity.entityIndex:D3}").transform;
            visual.SetParent(parent, false);
            var filter = visual.gameObject.AddComponent<MeshFilter>();
            filter.sharedMesh = EntityQuad(false);
            renderer = visual.gameObject.AddComponent<MeshRenderer>();
            UpdateOwnerEntitySprite(entity, group, visual, renderer);
            return visual;
        }

        private Mesh EntityQuad(bool mirrored)
        {
            if (mirrored && entityQuadMirroredMesh != null)
                return entityQuadMirroredMesh;
            if (!mirrored && entityQuadMesh != null) return entityQuadMesh;
            var mesh = CreateEntityQuad(mirrored);
            if (mirrored) entityQuadMirroredMesh = mesh;
            else entityQuadMesh = mesh;
            return mesh;
        }

        private static Mesh CreateEntityQuad(bool mirrored)
        {
            var mesh = new Mesh
            {
                name = mirrored
                    ? "Source-centered owner entity billboard quad UV-mirrored"
                    : "Source-centered owner entity billboard quad"
            };
            mesh.vertices = new[]
            {
                new Vector3(-0.5f, -0.5f, 0f),
                new Vector3(0.5f, -0.5f, 0f),
                new Vector3(-0.5f, 0.5f, 0f),
                new Vector3(0.5f, 0.5f, 0f)
            };
            // Source TMAP rows are top-to-bottom; raw Unity R8 row zero is
            // sampled at v=0, so top vertices deliberately use v=0.
            mesh.uv = mirrored
                ? new[]
                {
                    new Vector2(1f, 1f), new Vector2(0f, 1f),
                    new Vector2(1f, 0f), new Vector2(0f, 0f)
                }
                : new[]
                {
                    new Vector2(0f, 1f), new Vector2(1f, 1f),
                    new Vector2(0f, 0f), new Vector2(1f, 0f)
                };
            mesh.triangles = new[] { 0, 2, 1, 2, 3, 1 };
            mesh.RecalculateBounds();
            return mesh;
        }

        private void UpdateOwnerEntitySprite(
            NativeSim.EntitySnapshot entity,
            EntityPresentationGroup group,
            Transform visual,
            Renderer renderer)
        {
            UpdateOwnerEntitySprite(
                entity.entityIndex,
                entity.yaw,
                group.key.animation_frame,
                group,
                visual,
                renderer);
        }

        private void UpdateOwnerEntitySprite(
            ushort entityIndex,
            short entityYaw,
            int animationFrame,
            EntityPresentationGroup group,
            Transform visual,
            Renderer renderer)
        {
            var orientation = 0;
            var mirror = false;
            var selector = group.particle.base_tmap_selector;
            if (group.particle.render_mode == 20)
            {
                Require(
                    group.selector.yaw_lookup_0_15 != null &&
                    group.selector.yaw_lookup_0_15.Length == 16 &&
                    group.selector.yaw_sector_expression ==
                        "(((entity_yaw-camera_yaw)>>3)&0xF0)>>4",
                    $"mode20 yaw mapping {entityIndex}");
                QualificationMode20Selection(
                    entityYaw,
                    renderCameraState.yaw,
                    out orientation,
                    out var selectorOffset,
                    out var rearHalf);
                selector += group.selector.yaw_lookup_0_15[orientation];
                Require(
                    selectorOffset == group.selector.yaw_lookup_0_15[orientation],
                    $"mode20 source lookup {entityIndex}");
                mirror = rearHalf &&
                         group.selector.mirror_when_orientation_gte_8;
            }
            else if (group.particle.render_mode >= 2 &&
                     group.particle.render_mode <= 16 ||
                     group.particle.render_mode >= 22 &&
                     group.particle.render_mode <= 36)
            {
                selector += animationFrame;
            }
            Require(Array.IndexOf(group.selector.qualified_selectors, selector) >= 0,
                $"qualified owner entity selector {selector}");
            if (!tmapDescriptors.TryGetValue(selector, out var textureRow))
                throw new InvalidOperationException(
                    $"required invariant failed: materialized owner entity selector {selector}");
            Require(
                textureRow.status == "decoded_indexed_texture" &&
                textureRow.width > 0 && textureRow.height > 0,
                $"decoded owner entity selector {selector}");
            renderer.sharedMaterial = EntitySpriteMaterial(selector, textureRow);
            var height = group.particle.logical_height_source_units /
                         SourceUnitsPerUnityUnit;
            var width = height * textureRow.width / textureRow.height;
            visual.GetComponent<MeshFilter>().sharedMesh = EntityQuad(mirror);
            visual.localPosition = new Vector3(0f, height * 0.5f, 0f);
            visual.localScale = new Vector3(width, height, 1f);
        }

        internal static void QualificationMode20Selection(
            short entityYaw,
            short cameraYaw,
            out int orientation,
            out int selectorOffset,
            out bool rearHalf)
        {
            orientation = (((entityYaw - cameraYaw) >> 3) & 0xF0) >> 4;
            selectorOffset = YawLookupMode20[orientation];
            rearHalf = orientation >= 8;
        }

        private Material EntitySpriteMaterial(int selector, TmapDescriptor row)
        {
            if (entitySpriteMaterials.TryGetValue(selector, out var material))
                return material;
            Require(ownerPaletteTexture != null, "owner sprite PALN texture");
            var bytes = ownerAssetCache.ReadOutput(row.indices_path);
            Require(bytes.Length == row.width * row.height,
                $"owner TMAP selector {selector} extent");
            var texture = new Texture2D(
                row.width, row.height, TextureFormat.R8, false, true)
            {
                name = $"Exact owner TMAPS1 selector {selector} base frame",
                filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Clamp
            };
            texture.LoadRawTextureData(bytes);
            texture.Apply(false, true);
            var shader = Resources.Load<Shader>("EntitySprite");
            Require(shader != null, "indexed owner entity sprite shader");
            material = new Material(shader)
            {
                name = $"Indexed Night owner entity selector {selector}"
            };
            material.SetTexture("_Indices", texture);
            material.SetTexture("_Palette", ownerPaletteTexture);
            entitySpriteMaterials.Add(selector, material);
            entitySpriteTextures.Add(selector, texture);
            entitySpriteIndicesSha256.Add(selector, Sha256Hex(bytes));
            return material;
        }

        private void ConfigureEntityGallery()
        {
            Require(
                ownerAssetCache != null && entityPresentationDescriptor != null &&
                flightCamera != null,
                "entity gallery requires qualified owner cache and camera");
            RenderSettings.fog = false;
            flightCamera.backgroundColor = new Color(0.055f, 0.065f, 0.09f);
            if (terrainFrameCommandBuffer != null)
            {
                flightCamera.RemoveCommandBuffer(
                    CameraEvent.BeforeForwardOpaque,
                    terrainFrameCommandBuffer);
            }
            foreach (var terrain in terrainTiles) terrain.gameObject.SetActive(false);
            foreach (var marker in entityRegistry)
            {
                if (marker.Renderer != null) marker.Renderer.enabled = false;
            }
            var carpet = GameObject.Find("Diagnostic Carpet");
            if (carpet != null) carpet.SetActive(false);

            var dolmen = Array.Find(
                entityPresentationDescriptor.groups,
                group => group.semantic_family == "dolmen");
            var goat = Array.Find(
                entityPresentationDescriptor.groups,
                group => group.semantic_family == "goat");
            var villagers = new[]
            {
                "villager-a", "villager-b", "villager-c", "villager-d"
            };
            Require(dolmen != null && goat != null, "gallery dolmen and goat groups");
            AddGallerySprite("dolmen / 79", dolmen, 79, false,
                -7f, 0.6f, 18f);
            AddGallerySprite("goat front / 152", goat, 152, false,
                -0.3f, 0.6f, 18f);
            AddGallerySprite("goat side / 154", goat, 154, false,
                4.2f, 0.6f, 18f);
            AddGallerySprite("goat mirror / 154", goat, 154, true,
                8.7f, 0.6f, 18f);
            for (var index = 0; index < villagers.Length; index++)
            {
                var group = Array.Find(
                    entityPresentationDescriptor.groups,
                    value => value.semantic_family == villagers[index]);
                Require(group != null, $"gallery {villagers[index]} group");
                AddGallerySprite(
                    $"{villagers[index]} / {group.selector.qualified_selectors[0]}",
                    group,
                    group.selector.qualified_selectors[0],
                    false,
                    -7f + index * (14f / 3f),
                    -5.0f,
                    18f);
            }
            Require(entityGalleryItems.Count == 8,
                "entity gallery has eight controlled owner sprites");
            Debug.Log(
                "MC_ENTITY_GALLERY VERIFIED items=8 dolmen=79 " +
                "goat=152,154,154-mirror villagers=155,180,183,199 " +
                "neutralBackground=true simulationMutation=false noPhysics=true");
            ReleaseMouse();
        }

        private void AddGallerySprite(
            string label,
            EntityPresentationGroup group,
            int selector,
            bool mirror,
            float localX,
            float baselineY,
            float localZ)
        {
            Require(Array.IndexOf(group.selector.qualified_selectors, selector) >= 0,
                $"qualified gallery selector {selector}");
            if (!tmapDescriptors.TryGetValue(selector, out var row))
                throw new InvalidOperationException(
                    $"required invariant failed: gallery selector {selector}");
            var visual = new GameObject("Gallery " + label).transform;
            visual.SetParent(flightCamera.transform, false);
            visual.localRotation = Quaternion.identity;
            visual.gameObject.AddComponent<MeshFilter>().sharedMesh = EntityQuad(mirror);
            var renderer = visual.gameObject.AddComponent<MeshRenderer>();
            renderer.sharedMaterial = EntitySpriteMaterial(selector, row);
            var height = group.particle.logical_height_source_units /
                         SourceUnitsPerUnityUnit;
            var width = height * row.width / row.height;
            visual.localPosition =
                new Vector3(localX, baselineY + height * 0.5f, localZ);
            visual.localScale = new Vector3(width, height, 1f);
            entityGalleryItems.Add(new EntityGalleryItem
            {
                Label = label,
                Visual = visual,
                Selector = selector,
                Mirrored = mirror
            });
        }

        private void ApplyExitPalettePresentation()
        {
            if (
                ownerOriginalPaletteRgb8 == null ||
                ownerHudPaletteRgb8 == null ||
                ownerPaletteTexture == null ||
                ownerAuthorityBadgeTexture == null)
                return;
            if (
                appliedExitPaletteEffect == level1Palette.effect &&
                appliedExitPaletteWeight == level1Palette.weight &&
                appliedExitPaletteDenominator == level1Palette.denominator &&
                appliedPaletteCount == level1Palette.paletteCount)
                return;

            // Rust materializes PaletteChanges from the exact PALN RGB6 bytes.
            // Every indexed Unity consumer receives the same atomic RGB8
            // palette; Unity does not recreate or time the effect.
            Buffer.BlockCopy(
                level1EffectivePaletteRgb8,
                0,
                ownerHudPaletteRgb8,
                0,
                ownerHudPaletteRgb8.Length);
            ownerPaletteTexture.LoadRawTextureData(ownerHudPaletteRgb8);
            ownerPaletteTexture.Apply(false, false);

            foreach (var pair in ownerHudBasePixels)
            {
                ApplyNativePaletteToTexture(
                    hudBitmapTextures[pair.Key],
                    pair.Value);
            }
            ApplyNativePaletteToTexture(
                ownerAuthorityBadgeTexture,
                ownerAuthorityBadgeBasePixels);

            appliedExitPaletteEffect = level1Palette.effect;
            appliedExitPaletteWeight = level1Palette.weight;
            appliedExitPaletteDenominator = level1Palette.denominator;
            appliedPaletteCount = level1Palette.paletteCount;
        }

        private void ApplyNativePaletteToTexture(
            Texture2D texture,
            Color32[] originalPixels)
        {
            if (texture == null || originalPixels == null) return;
            var paletteMap = new Dictionary<int, int>();
            for (var paletteIndex = 0; paletteIndex < 256; paletteIndex++)
            {
                var offset = paletteIndex * 3;
                var key =
                    ownerOriginalPaletteRgb8[offset] |
                    ownerOriginalPaletteRgb8[offset + 1] << 8 |
                    ownerOriginalPaletteRgb8[offset + 2] << 16;
                if (!paletteMap.ContainsKey(key))
                    paletteMap.Add(key, paletteIndex);
            }
            var effective = new Color32[originalPixels.Length];
            for (var index = 0; index < originalPixels.Length; index++)
            {
                var source = originalPixels[index];
                if (source.a == 0)
                {
                    effective[index] = source;
                    continue;
                }
                var key = source.r | source.g << 8 | source.b << 16;
                Require(
                    paletteMap.TryGetValue(key, out var paletteIndex),
                    "owner HUD pixel resolves to PALN index");
                var offset = paletteIndex * 3;
                effective[index] = new Color32(
                    ownerHudPaletteRgb8[offset],
                    ownerHudPaletteRgb8[offset + 1],
                    ownerHudPaletteRgb8[offset + 2],
                    source.a);
            }
            texture.SetPixels32(effective);
            texture.Apply(false, false);
        }

        private void CreateOwnerHud()
        {
            var hudPalette = ownerAssetCache.ReadOutput("palette/paln.rgb8");
            Require(
                ownerOriginalPaletteRgb8 != null &&
                hudPalette.Length == ownerOriginalPaletteRgb8.Length,
                "shared owner PALN is initialized before HUD");
            for (var index = 0; index < hudPalette.Length; index++)
                Require(
                    hudPalette[index] == ownerOriginalPaletteRgb8[index],
                    $"shared owner PALN byte {index}");
            ownerHudPaletteRgb8 = (byte[])ownerOriginalPaletteRgb8.Clone();
            ownerHudTables = ownerAssetCache.ReadOutput("palette/tablesn.u8");
            ownerHudClrn = ownerAssetCache.ReadOutput("palette/clrn.indices.u8");
            ownerHudMinimapAperture = ownerAssetCache.ReadOutput(
                hudDescriptor.minimap.aperture.path);
            var sinePayload = ownerAssetCache.ReadOutput(
                hudDescriptor.minimap.rotation.sine_table_path);
            Require(
                ownerHudPaletteRgb8.Length == 256 * 3 &&
                ownerHudTables.Length == 83456 &&
                ownerHudClrn.Length == 4096 &&
                ownerHudClrn[4095] == 176 &&
                ownerHudMinimapAperture.Length == 128 * 128 &&
                sinePayload.Length == 2560 * 4,
                "owner HUD PALN, CLRN, TABLESN, aperture and sine payloads");
            var opaquePixels = 0;
            foreach (var value in ownerHudMinimapAperture)
            {
                Require(value == 0 || value == 1, "binary minimap aperture");
                opaquePixels += value;
            }
            Require(opaquePixels == 12848, "exact 128x128 TABLESN minimap aperture");
            for (var y = 0; y < 128; y++)
            {
                var row = hudDescriptor.minimap.aperture.scanlines[y];
                Require(
                    row.y == y &&
                    row.start_x_inclusive >= 0 &&
                    row.end_x_exclusive <= 128 &&
                    row.start_x_inclusive < row.end_x_exclusive,
                    $"valid minimap aperture scanline {y}");
                for (var x = 0; x < 128; x++)
                {
                    var expected = (
                        x >= row.start_x_inclusive &&
                        x < row.end_x_exclusive) ? 1 : 0;
                    Require(
                        ownerHudMinimapAperture[y * 128 + x] == expected,
                        $"minimap aperture scanline contract {y}");
                }
            }
            ownerHudSine = new int[2560];
            for (var index = 0; index < ownerHudSine.Length; index++)
                ownerHudSine[index] = BitConverter.ToInt32(sinePayload, index * 4);
            Require(
                ownerHudSine[0] == 0 &&
                ownerHudSine[512] == 65536 &&
                ownerHudSine[1024] == 0 &&
                ownerHudSine[1536] == -65536 &&
                ownerHudSine[2048] == 0,
                "exact remc2 minimap sine cardinal values");
            var directionYaws = new[]
            {
                0, 0, 0, 0,
                512, 512, 512, 512,
                1024, 1024, 1024, 1024,
                1536, 1536, 1536, 1536
            };
            var directionXs = new[]
            {
                64, 96, 64, 32, 64, 96, 64, 32,
                64, 96, 64, 32, 64, 96, 64, 32
            };
            var directionYs = new[]
            {
                32, 64, 96, 64, 32, 64, 96, 64,
                32, 64, 96, 64, 32, 64, 96, 64
            };
            var expectedTileXs = new[]
            {
                77, 109, 77, 45, 109, 77, 45, 77,
                77, 45, 77, 109, 45, 77, 109, 77
            };
            var expectedTileYs = new[]
            {
                190, 222, 254, 222, 222, 254, 222, 190,
                254, 222, 190, 222, 222, 190, 222, 254
            };
            for (var index = 0; index < directionYaws.Length; index++)
            {
                OwnerMinimapSourceFixed(
                    snapshot.x,
                    snapshot.y,
                    directionYaws[index],
                    directionXs[index],
                    directionYs[index],
                    out var sourceX,
                    out var sourceY);
                Require(
                    ((sourceX >> 8) & 255) == expectedTileXs[index] &&
                    ((sourceY >> 8) & 255) == expectedTileYs[index],
                    $"exact minimap cardinal direction sample {index}");
            }
            for (var yaw = 0; yaw < 2048; yaw += 256)
            {
                OwnerMinimapSourceFixed(
                    snapshot.x, snapshot.y, yaw, 64, 64,
                    out var centreX, out var centreY);
                Require(
                    centreX == snapshot.x && centreY == snapshot.y,
                    $"exact minimap centre at yaw {yaw}");
            }
            hudBitmapTextures.Clear();
            hudBitmapIndices.Clear();
            hudBitmapMasks.Clear();
            ownerHudBasePixels.Clear();
            foreach (var row in hudDescriptor.bitmap_assets)
            {
                var indices = ownerAssetCache.ReadOutput(row.indices_path);
                var mask = ownerAssetCache.ReadOutput(row.mask_path);
                var rgba = new Color32[row.stored_width * row.stored_height];
                for (var sourceY = 0; sourceY < row.stored_height; sourceY++)
                {
                    var unityY = row.stored_height - 1 - sourceY;
                    for (var x = 0; x < row.stored_width; x++)
                    {
                        var source = sourceY * row.stored_width + x;
                        if (mask[source] == 0)
                        {
                            rgba[unityY * row.stored_width + x] = new Color32(0, 0, 0, 0);
                            continue;
                        }
                        var paletteOffset = indices[source] * 3;
                        rgba[unityY * row.stored_width + x] = new Color32(
                            ownerHudPaletteRgb8[paletteOffset],
                            ownerHudPaletteRgb8[paletteOffset + 1],
                            ownerHudPaletteRgb8[paletteOffset + 2],
                            255);
                    }
                }
                var texture = new Texture2D(
                    row.stored_width,
                    row.stored_height,
                    TextureFormat.RGBA32,
                    false,
                    true)
                {
                    name = $"Owner HSPRN selector {row.selector} FrozenObserved",
                    filterMode = FilterMode.Point,
                    wrapMode = TextureWrapMode.Clamp
                };
                texture.SetPixels32(rgba);
                texture.Apply(false, false);
                hudBitmapTextures.Add(row.selector, texture);
                hudBitmapIndices.Add(row.selector, indices);
                hudBitmapMasks.Add(row.selector, mask);
                ownerHudBasePixels.Add(
                    row.selector,
                    (Color32[])rgba.Clone());
            }
            ownerSpellTierTextures.Clear();
            foreach (var row in hudDescriptor.spells.tier_rows)
            {
                ownerSpellTierTextures.Add(
                    row.tier,
                    CreateTopDownRgbaTexture(
                        $"Owner HFONT3 spell tier {row.text}",
                        ownerAssetCache.ReadOutput(row.render_path),
                        row.render_width,
                        row.render_height));
            }
            ownerObjectiveTextures.Clear();
            ownerObjectiveRows.Clear();
            ownerObjectiveRasters.Clear();
            ownerObjectiveOverlayIndices.Clear();
            ownerObjectiveOverlayMasks.Clear();
            ownerObjectiveUploadPixels.Clear();
            foreach (var row in hudDescriptor.objective.rows)
            {
                Require(
                    row.selector >= 48 &&
                    row.selector <= 53 &&
                    row.authority ==
                        "SourceMappedLiveIndexedFramebuffer" &&
                    !ownerObjectiveRows.ContainsKey(row.selector),
                    $"source-mapped objective row {row.selector}");
                var raster = ownerAssetCache.ReadOutput(row.raster_path);
                var planeBytes = row.render_width * row.render_height;
                Require(
                    row.raster_plane_bytes == planeBytes &&
                    raster.Length == planeBytes * 2,
                    $"indexed objective raster extent {row.selector}");
                var foregroundIndices = new byte[planeBytes];
                var foregroundMask = new byte[planeBytes];
                Buffer.BlockCopy(raster, 0, foregroundIndices, 0, planeBytes);
                Buffer.BlockCopy(
                    raster,
                    planeBytes,
                    foregroundMask,
                    0,
                    planeBytes);
                var foregroundOpaquePixels = 0;
                var foregroundMaskBinary = true;
                foreach (var value in foregroundMask)
                {
                    if (value == 1) foregroundOpaquePixels++;
                    else if (value != 0) foregroundMaskBinary = false;
                }
                Require(
                    Sha256Hex(foregroundIndices) ==
                        row.foreground_indices_sha256 &&
                    Sha256Hex(foregroundMask) ==
                        row.foreground_mask_sha256 &&
                    foregroundOpaquePixels ==
                        row.foreground_opaque_pixels &&
                    foregroundMaskBinary,
                    $"indexed objective planes {row.selector}");
                ownerObjectiveRows.Add(row.selector, row);
                ownerObjectiveRasters.Add(row.selector, raster);
                ownerObjectiveOverlayIndices.Add(
                    row.selector,
                    new byte[planeBytes]);
                ownerObjectiveOverlayMasks.Add(
                    row.selector,
                    new byte[planeBytes]);
                ownerObjectiveUploadPixels.Add(
                    row.selector,
                    new Color32[planeBytes]);
                var objectiveTexture = new Texture2D(
                    row.render_width,
                    row.render_height,
                    TextureFormat.RGBA32,
                    false,
                    true)
                {
                    name =
                        $"Owner Level 1 objective {row.selector} indexed live blend",
                    filterMode = FilterMode.Point,
                    wrapMode = TextureWrapMode.Clamp
                };
                ownerObjectiveTextures.Add(row.selector, objectiveTexture);
            }
            ownerAuthorityBadgeTexture = CreateTopDownRgbaTexture(
                "Owner HFONT3 FrozenObserved badge",
                ownerAssetCache.ReadOutput(hudDescriptor.authority_badge.render_path),
                hudDescriptor.authority_badge.render_width,
                hudDescriptor.authority_badge.render_height);
            ownerAuthorityBadgeBasePixels =
                ownerAuthorityBadgeTexture.GetPixels32();
            minimapTexture = new Texture2D(
                hudDescriptor.minimap.logical_width,
                hudDescriptor.minimap.logical_height,
                TextureFormat.RGBA32,
                false,
                true)
            {
                name = "Owner Level 1 mixed-authority player-centred minimap",
                filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Clamp
            };
            UpdateOwnerMinimap();
            appliedExitPaletteEffect = byte.MaxValue;
            appliedExitPaletteWeight = byte.MaxValue;
            appliedExitPaletteDenominator = byte.MaxValue;
            ApplyExitPalettePresentation();
            Debug.Log(
                "MC_OWNER_HUD READY textures=80 " +
                "objectives=selectors48-53/indexed-live-TABLESN-blend " +
                "minimap=128-native/TABLESN-aperture/fixed-rotation " +
                "terrain/OwnerLevelStatic,entities/MissionStateLive," +
                "playerPose/PlayerFlightLive objectiveMarks=SourceMappedLive " +
                "bars=MissionStateLiveSourceMapped highres=HSPRN-native");
        }

        private static Texture2D CreateTopDownRgbaTexture(
            string name,
            byte[] topDownRgba,
            int width,
            int height)
        {
            Require(topDownRgba.Length == width * height * 4, name + " RGBA extent");
            var flipped = new byte[topDownRgba.Length];
            var rowBytes = width * 4;
            for (var sourceY = 0; sourceY < height; sourceY++)
            {
                Buffer.BlockCopy(
                    topDownRgba,
                    sourceY * rowBytes,
                    flipped,
                    (height - 1 - sourceY) * rowBytes,
                    rowBytes);
            }
            var texture = new Texture2D(
                width, height, TextureFormat.RGBA32, false, true)
            {
                name = name,
                filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Clamp
            };
            texture.LoadRawTextureData(flipped);
            texture.Apply(false, false);
            return texture;
        }

        private void UpdateOwnerMinimap()
        {
            if (
                minimapTexture == null ||
                ownerHudTables == null ||
                ownerHudClrn == null ||
                ownerHudMinimapAperture == null ||
                ownerHudSine == null)
                return;
            const int side = 128;
            var indices = new byte[side * side];
            var objectiveOverlay = new bool[side * side];
            var pixels = new Color32[side * side];

            for (var screenY = 0; screenY < side; screenY++)
            {
                for (var screenX = 0; screenX < side; screenX++)
                {
                    if (ownerHudMinimapAperture[screenY * side + screenX] == 0)
                        continue;
                    OwnerMinimapSourceFixed(
                        snapshot.x,
                        snapshot.y,
                        snapshot.yaw,
                        screenX,
                        screenY,
                        out var sourceX,
                        out var sourceY);
                    var tileX = (sourceX >> 8) & 255;
                    var tileY = (sourceY >> 8) & 255;
                    var tile = tileX + tileY * 256;
                    var baseIndex = ownerHudTables[0x14000 + terrainTypes[tile]];
                    var terrainIndex =
                        ownerHudTables[(terrainShade[tile] << 8) | baseIndex];
                    var framebufferX =
                        screenX * NativeSim.TerrainFrameWidth / 640;
                    var framebufferY =
                        screenY * NativeSim.TerrainFrameHeight / 480;
                    var framebufferIndex = terrainFrameIndices[
                        framebufferY * NativeSim.TerrainFrameWidth +
                        framebufferX];
                    indices[screenY * side + screenX] = ownerHudTables[
                        0x4000 + (framebufferIndex << 8) + terrainIndex];
                }
            }

            // Rust owns the complete source-slot walk and emits commands in
            // castle-rope then ascending 1..999 order. Unity only performs the
            // source raster operations against the owner aperture/bitmaps.
            foreach (var command in level1MinimapEntities)
            {
                if (
                    command.x < 0 || command.x >= side ||
                    command.y < 0 || command.y >= side ||
                    ownerHudMinimapAperture[command.y * side + command.x] == 0)
                    continue;
                if (command.kind == NativeSim.Level1MinimapCommandBlendPixel)
                {
                    BlendOwnerMinimapIndexUnclipped(
                        indices, side, command.x, command.y, command.paletteIndex);
                }
                else if (command.kind == NativeSim.Level1MinimapCommandDot)
                {
                    for (var drawY = 0; drawY < command.size; drawY++)
                    {
                        for (var drawX = 0; drawX < command.size; drawX++)
                            SetOwnerMinimapIndexUnclipped(
                                indices,
                                side,
                                command.x + drawX,
                                command.y + drawY,
                                command.paletteIndex);
                    }
                }
                else
                {
                    BlitOwnerMinimapBitmap(
                        indices,
                        side,
                        command.selector,
                        command.x,
                        command.y,
                        command.anchor);
                }
            }

            // The source centre is (63,64). Blend each of the 41 unique cross
            // pixels exactly once; the previous four-arm loop transformed the
            // centre four times.
            var crossX = level1MinimapEntitiesInfo.crossX;
            var crossY = level1MinimapEntitiesInfo.crossY;
            var crossArm = level1MinimapEntitiesInfo.crossArmLength;
            var crossColour = level1MinimapEntitiesInfo.crossColour;
            BlendOwnerMinimapIndexUnclipped(
                indices, side, crossX, crossY, crossColour);
            for (var arm = 1; arm <= crossArm; arm++)
            {
                BlendOwnerMinimapIndexUnclipped(
                    indices, side, crossX - arm, crossY, crossColour);
                BlendOwnerMinimapIndexUnclipped(
                    indices, side, crossX + arm, crossY, crossColour);
                BlendOwnerMinimapIndexUnclipped(
                    indices, side, crossX, crossY - arm, crossColour);
                BlendOwnerMinimapIndexUnclipped(
                    indices, side, crossX, crossY + arm, crossColour);
            }

            DrawOwnerMinimapMarks(indices, objectiveOverlay, side);

            for (var y = 0; y < side; y++)
            {
                for (var x = 0; x < side; x++)
                {
                    if (
                        ownerHudMinimapAperture[y * side + x] == 0 &&
                        !objectiveOverlay[y * side + x])
                        continue;
                    SetTopDownPixel(
                        pixels,
                        side,
                        x,
                        y,
                        PaletteColour(ownerHudPaletteRgb8, indices[y * side + x]));
                }
            }
            minimapTexture.SetPixels32(pixels);
            minimapTexture.Apply(false, false);
        }

        private void DrawOwnerMinimapMarks(
            byte[] indices,
            bool[] overlay,
            int side)
        {
            var count = Math.Min(level1MinimapMarks.markCount, (byte)16);
            for (var ordinal = 0; ordinal < count; ordinal++)
            {
                var x = level1MinimapMarks.markX[ordinal];
                var y = level1MinimapMarks.markY[ordinal];
                if (!OwnerMinimapMarkInsideAperture(x, y, side)) continue;
                if (level1MinimapMarks.markPulse == 0) continue;
                var colour = level1MinimapMarks.markColour[ordinal];
                for (var offsetY = -1; offsetY <= 1; offsetY++)
                {
                    for (var offsetX = -1; offsetX <= 1; offsetX++)
                    {
                        if (offsetX == 0 && offsetY == 0) continue;
                        SetOwnerMinimapOverlayIndex(
                            indices,
                            overlay,
                            side,
                            x + offsetX,
                            y + offsetY,
                            colour);
                    }
                }
            }

            if (level1MinimapMarks.arrowVisible == 0) return;
            var arrow = level1MinimapMarks.arrowXY;
            const byte arrowColour = 119;
            int[,] segments =
            {
                { 0, 1 },
                { 1, 3 },
                { 3, 4 },
                { 4, 5 },
                { 5, 6 },
                { 6, 2 },
                { 2, 0 }
            };
            for (var segment = 0; segment < segments.GetLength(0); segment++)
            {
                var from = segments[segment, 0];
                var to = segments[segment, 1];
                DrawOwnerMinimapLine(
                    indices,
                    overlay,
                    side,
                    arrow[from * 2],
                    arrow[from * 2 + 1],
                    arrow[to * 2],
                    arrow[to * 2 + 1],
                    arrowColour);
            }
        }

        private bool OwnerMinimapMarkInsideAperture(int x, int y, int side)
        {
            if (x < 0 || x >= side || y < 3 || y >= side - 3) return false;
            var left = 0;
            while (
                left < side &&
                ownerHudMinimapAperture[y * side + left] == 0)
                left++;
            var rightExclusive = side;
            while (
                rightExclusive > left &&
                ownerHudMinimapAperture[
                    y * side + rightExclusive - 1] == 0)
                rightExclusive--;
            return x >= left + 3 && x < rightExclusive - 3;
        }

        private static void DrawOwnerMinimapLine(
            byte[] indices,
            bool[] overlay,
            int side,
            int x1,
            int y1,
            int x2,
            int y2,
            byte colour)
        {
            // `DrawLine_BD542` sorts by Y before its integer viewport clip.
            if (y1 > y2)
            {
                (x1, x2) = (x2, x1);
                (y1, y2) = (y2, y1);
            }
            if (y2 < 0 || y1 >= side) return;
            if (y1 < 0)
            {
                x1 += -y1 * (x2 - x1) / (y2 - y1);
                y1 = 0;
            }
            if (y2 >= side)
            {
                x2 = x1 + (side - y1) * (x2 - x1) / (y2 - y1);
                y2 = side - 1;
            }

            if (x1 >= 0 && x1 < side)
            {
                if (x2 >= side)
                {
                    y2 = y1 + (side - x1) * (y2 - y1) / (x2 - x1);
                    x2 = side - 1;
                }
                else if (x2 < 0)
                {
                    y2 = y1 + x1 * (y2 - y1) / (x1 - x2);
                    x2 = 0;
                }
            }
            else if (x1 >= side)
            {
                if (x2 >= side) return;
                y1 += (x1 - side) * (y2 - y1) / (x1 - x2);
                x1 = side - 1;
                if (x2 < 0)
                {
                    y2 -= -x2 * (y2 - y1) / (x1 - x2);
                    x2 = 0;
                }
            }
            else
            {
                if (x2 < 0) return;
                y1 += -x1 * (y2 - y1) / (x2 - x1);
                x1 = 0;
                if (x2 >= side)
                {
                    y2 = y1 + side * (y2 - y1) / x2;
                    x2 = side - 1;
                }
            }

            SetOwnerMinimapOverlayIndex(
                indices, overlay, side, x1, y1, colour);
            var stepX = x2 >= x1 ? 1 : -1;
            var deltaX = Math.Abs(x2 - x1);
            var deltaY = y2 - y1;
            if (deltaX == 0)
            {
                for (var y = y1 + 1; y <= y2; y++)
                    SetOwnerMinimapOverlayIndex(
                        indices, overlay, side, x1, y, colour);
                return;
            }
            if (deltaY == 0)
            {
                for (var x = x1 + stepX; x != x2 + stepX; x += stepX)
                    SetOwnerMinimapOverlayIndex(
                        indices, overlay, side, x, y1, colour);
                return;
            }

            var major = Math.Max(deltaX, deltaY);
            var minor = Math.Min(deltaX, deltaY);
            var error = 2 * minor - major;
            var majorIsX = deltaX >= deltaY;
            var xCurrent = x1;
            var yCurrent = y1;
            for (var remaining = major; remaining > 0; remaining--)
            {
                if (majorIsX) xCurrent += stepX;
                else yCurrent++;
                if (error >= 0)
                {
                    if (majorIsX) yCurrent++;
                    else xCurrent += stepX;
                    error += 2 * (minor - major);
                }
                else
                {
                    error += 2 * minor;
                }
                SetOwnerMinimapOverlayIndex(
                    indices, overlay, side, xCurrent, yCurrent, colour);
            }
        }

        private static void SetOwnerMinimapOverlayIndex(
            byte[] indices,
            bool[] overlay,
            int side,
            int x,
            int y,
            byte paletteIndex)
        {
            if (x < 0 || y < 0 || x >= side || y >= side) return;
            var offset = y * side + x;
            indices[offset] = paletteIndex;
            overlay[offset] = true;
        }

        private bool OwnerMinimapBlip(
            byte classId,
            byte model,
            byte action,
            out byte paletteIndex,
            out int selector,
            out int blipSize)
        {
            paletteIndex = 0;
            selector = 0;
            blipSize = 1;
            switch (classId)
            {
                case 2:
                    if (model == 3 || (model == 0 && action == 2) || model == 1)
                        return false;
                    paletteIndex = 0x84;
                    return true;
                case 5:
                    if (
                        action == 0xb4 ||
                        (action >= 0xe8 && action <= 0xea))
                        return false;
                    paletteIndex = model is >= 12 and <= 14
                        ? ownerHudClrn[15]
                        : ownerHudClrn[4095];
                    return true;
                case 9:
                    paletteIndex = ownerHudClrn[3855];
                    return true;
                case 10:
                    if (model == 18 || model == 78) return false;
                    paletteIndex = model is 39 or 57
                        ? (byte)0xe8
                        : ownerHudClrn[3855];
                    blipSize = model == 34 ? 2 : 1;
                    return true;
                case 11:
                    if (model != 12) return false;
                    selector = 83;
                    return true;
                case 12:
                case 15:
                    paletteIndex = ownerHudClrn[3840];
                    return true;
                case 14:
                    if (model != 5) return false;
                    paletteIndex = ownerHudClrn[4095];
                    return true;
                default:
                    return false;
            }
        }

        private void PlotOwnerMinimapBlip(
            byte[] indices,
            ushort sourceX,
            ushort sourceY,
            byte paletteIndex,
            int selector,
            int blipSize,
            int sineRaw,
            int cosineRaw)
        {
            const int side = 128;
            const int centre = side / 2;
            const int scaling = 256;
            var deltaX = ModularDelta(sourceX, snapshot.x);
            var deltaY = ModularDelta(sourceY, snapshot.y);
            var inverseScale = 0x10000 / scaling;
            var localDownAxis =
                (int)(-((long)inverseScale * sineRaw) >> 16);
            var localRightAxis =
                (int)(((long)inverseScale * cosineRaw) >> 16);
            var x = centre + (int)(
                ((long)deltaX * localRightAxis -
                 (long)deltaY * localDownAxis) >> 16);
            var y = centre + (int)(
                ((long)localRightAxis * deltaY +
                 (long)localDownAxis * deltaX) >> 16);
            if (
                x < 0 || x >= side || y < 0 || y >= side ||
                ownerHudMinimapAperture[y * side + x] == 0)
                return;
            if (selector != 0)
            {
                BlitOwnerMinimapBitmap(indices, side, selector, x, y);
                return;
            }
            for (var drawY = 0; drawY < blipSize; drawY++)
            {
                for (var drawX = 0; drawX < blipSize; drawX++)
                    SetOwnerMinimapIndex(
                        indices, side, x + drawX, y + drawY, paletteIndex);
            }
        }

        private void BlitOwnerMinimapBitmap(
            byte[] destination,
            int destinationSide,
            int selector,
            int anchorX,
            int anchorY,
            byte anchor = NativeSim.Level1MinimapAnchorBottomCenter)
        {
            HudBitmapDescriptor descriptor = null;
            byte[] source = null;
            byte[] mask = null;
            Require(
                hudBitmapDescriptors.TryGetValue(selector, out descriptor) &&
                hudBitmapIndices.TryGetValue(selector, out source) &&
                hudBitmapMasks.TryGetValue(selector, out mask),
                $"materialized minimap HSPRN selector {selector}");
            var left = anchor == NativeSim.Level1MinimapAnchorBottomLeft
                ? anchorX
                : anchorX - descriptor.stored_width / 2;
            var top = anchorY - descriptor.stored_height;
            for (var sourceY = 0; sourceY < descriptor.stored_height; sourceY++)
            {
                for (var sourceX = 0; sourceX < descriptor.stored_width; sourceX++)
                {
                    var sourceOffset = sourceY * descriptor.stored_width + sourceX;
                    if (mask[sourceOffset] == 0) continue;
                    SetOwnerMinimapIndexUnclipped(
                        destination,
                        destinationSide,
                        left + sourceX,
                        top + sourceY,
                        source[sourceOffset]);
                }
            }
        }

        private void BlendOwnerMinimapIndex(
            byte[] indices,
            int side,
            int x,
            int y,
            byte blendIndex)
        {
            if (
                x < 0 || y < 0 || x >= side || y >= side ||
                ownerHudMinimapAperture[y * side + x] == 0)
                return;
            var offset = y * side + x;
            indices[offset] =
                ownerHudTables[0x4000 + (blendIndex << 8) + indices[offset]];
        }

        private void BlendOwnerMinimapIndexUnclipped(
            byte[] indices,
            int side,
            int x,
            int y,
            byte blendIndex)
        {
            if (x < 0 || y < 0 || x >= side || y >= side) return;
            var offset = y * side + x;
            indices[offset] =
                ownerHudTables[0x4000 + (blendIndex << 8) + indices[offset]];
        }

        private void SetOwnerMinimapIndex(
            byte[] indices,
            int side,
            int x,
            int y,
            byte paletteIndex)
        {
            if (
                x < 0 || y < 0 || x >= side || y >= side ||
                ownerHudMinimapAperture[y * side + x] == 0)
                return;
            indices[y * side + x] = paletteIndex;
        }

        private static void SetOwnerMinimapIndexUnclipped(
            byte[] indices,
            int side,
            int x,
            int y,
            byte paletteIndex)
        {
            if (x < 0 || y < 0 || x >= side || y >= side) return;
            indices[y * side + x] = paletteIndex;
        }

        private void OwnerMinimapSourceFixed(
            int playerX,
            int playerY,
            int yaw,
            int screenX,
            int screenY,
            out int sourceX,
            out int sourceY)
        {
            const int side = 128;
            const int scaling = 256;
            var angle = yaw & 0x7ff;
            var sineScaled = (int)(((long)scaling * ownerHudSine[angle]) >> 16);
            var cosineScaled = (int)(
                ((long)scaling * ownerHudSine[angle + 0x200]) >> 16);
            var columnX = side * cosineScaled / side;
            var columnY = side * sineScaled / side;
            var originX = playerX -
                (side * columnX - side * sineScaled) / 2;
            var originY = playerY -
                (side * cosineScaled + side * columnY) / 2;
            sourceX = originX - screenY * sineScaled + screenX * columnX;
            sourceY = originY + screenY * cosineScaled + screenX * columnY;
        }

        private static void SetTopDownPixel(
            Color32[] pixels,
            int width,
            int x,
            int y,
            Color32 colour)
        {
            if (x < 0 || y < 0 || x >= width || y >= width) return;
            pixels[(width - 1 - y) * width + x] = colour;
        }

        private static Color32 PaletteColour(byte[] palette, int index)
        {
            var offset = index * 3;
            return new Color32(
                palette[offset], palette[offset + 1], palette[offset + 2], 255);
        }

        private void CreateMinimap()
        {
            minimapTexture = new Texture2D(256, 256, TextureFormat.RGBA32, false, true)
            {
                name = "Exact-data diagnostic minimap",
                filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Repeat
            };
            var pixels = new Color32[65536];
            if (ownerAssetCache != null)
            {
                var palette = ownerAssetCache.ReadOutput("palette/paln.rgb8");
                var bank = ownerAssetCache.ReadOutput("terrain/bl32n.indices.u8");
                for (var index = 0; index < pixels.Length; index++)
                {
                    var paletteIndex =
                        bank[terrainTypes[index] * 32 * 32 + 16 * 32 + 16];
                    var offset = paletteIndex * 3;
                    pixels[index] = new Color32(
                        palette[offset],
                        palette[offset + 1],
                        palette[offset + 2],
                        255);
                }
            }
            else
            {
                for (var index = 0; index < pixels.Length; index++)
                {
                    pixels[index] = DiagnosticTerrainColor(
                        terrainTypes[index],
                        terrainShade[index],
                        groundHeights[index]);
                }
            }
            minimapTexture.SetPixels32(pixels);
            minimapTexture.Apply(false, false);
        }

        private static Texture2D CreatePlaneTexture(string name, byte[] values)
        {
            var texture = new Texture2D(256, 256, TextureFormat.R8, false, true)
            {
                name = name,
                filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Repeat
            };
            texture.LoadRawTextureData(values);
            texture.Apply(false, false);
            return texture;
        }

        private void InitializeUnwrappedPose()
        {
            presentationState = session.PresentationState();
            ValidatePresentationState(presentationState);
            renderCameraState = session.RenderCameraState();
            ValidateRenderCameraState(renderCameraState, presentationState);
            priorRawX = presentationState.cameraX;
            priorRawY = presentationState.cameraY;
            unwrappedSourceX = presentationState.cameraX;
            unwrappedSourceY = presentationState.cameraY;
            Require(ModularDelta(0, 65535) == 1, "wrap delta 65535->0");
            Require(ModularDelta(65535, 0) == -1, "wrap delta 0->65535");
            Debug.Log(
                "MC_WRAP_SELF_TEST PASS forward=1 reverse=-1 " +
                $"cameraEyeOffset=128 sourceUnits scale={SourceUnitsPerUnityUnit}:1");
        }

        private void ReadAuthoritativePose(bool snap)
        {
            presentationState = session.PresentationState();
            ValidatePresentationState(presentationState);
            renderCameraState = session.RenderCameraState();
            ValidateRenderCameraState(renderCameraState, presentationState);
            UpdateExactTerrainFrame();
            if (!snap)
            {
                unwrappedSourceX += ModularDelta(presentationState.cameraX, priorRawX);
                unwrappedSourceY += ModularDelta(presentationState.cameraY, priorRawY);
            }
            priorRawX = presentationState.cameraX;
            priorRawY = presentationState.cameraY;
            targetPosition = SourceToUnity(
                unwrappedSourceX + renderCameraState.renderOffsetX,
                unwrappedSourceY + renderCameraState.renderOffsetY,
                presentationState.cameraZ);
            var yawDegrees = 180f - renderCameraState.yaw * (360f / 2048f);
            var signedPitch = SignedAngle11(presentationState.cameraPitch);
            var pitchDegrees = -signedPitch * (360f / 2048f);
            var signedRoll = SignedAngle11(presentationState.cameraRoll);
            var bankDegrees = -signedRoll * (360f / 2048f);
            targetRotation = Quaternion.Euler(pitchDegrees, yawDegrees, bankDegrees);
            if (flightCamera != null)
            {
                var unityFov =
                    Level1ExitPresentationMath.UnityVerticalFov(
                        presentationState.cameraFov);
                flightCamera.fieldOfView = unityFov;
                flightCamera.rect =
                    Level1ExitPresentationMath.SourceDisplayRect(
                        Screen.width, Screen.height);
                if (carpetOverlayCamera != null)
                {
                    carpetOverlayCamera.fieldOfView = unityFov;
                    carpetOverlayCamera.rect = flightCamera.rect;
                }
            }
            // Billboards read the camera's world rotation below. Apply the
            // authoritative pose first so they cannot inherit a render-frame-old
            // rotation when several fixed simulation ticks precede one frame.
            if (presentationRig != null)
            {
                presentationRig.SetPositionAndRotation(targetPosition, targetRotation);
            }
            RepositionTerrainCopies();
            UpdateEntityPresentation();
            UpdateMissionPresentation();
            if (hudDescriptor != null) UpdateOwnerMinimap();
        }

        private static void ValidatePresentationState(NativeSim.PresentationState state)
        {
            Require(state.abiVersion == NativeSim.AbiVersion, "presentation ABI version");
            Require(state.structSize == 64, "presentation struct size");
            Require(
                state.cameraAuthority == NativeSim.PlayerFlightLive,
                "presentation camera player-flight authority");
            Require(
                (state.cameraFlags & NativeSim.PresentationCameraExactInputs) != 0 &&
                (state.cameraFlags & NativeSim.PresentationCameraEyeZPlus128) != 0,
                "presentation camera exact source inputs and z+128 eye");
        }

        private static void ValidateRenderCameraState(
            NativeSim.RenderCameraState render,
            NativeSim.PresentationState history)
        {
            Require(
                render.abiVersion == NativeSim.AbiVersion &&
                render.structSize == 72,
                "render camera ABI layout");
            Require(
                render.generation == history.generation &&
                render.tick == history.tick &&
                render.sourceTurn == history.sourceTurn &&
                render.historyX == history.cameraX &&
                render.historyY == history.cameraY &&
                render.eyeZ == history.cameraZ &&
                render.yaw == (history.cameraYaw & 0x07ff) &&
                render.pitch == history.cameraPitch &&
                render.roll == history.cameraRoll &&
                render.fov == history.cameraFov,
                "render camera preserves the complete stored history row");
            Require(
                render.authority == NativeSim.PlayerFlightLive &&
                (render.flags & NativeSim.RenderCameraTerrainSlopeXY) != 0,
                "render camera exact terrain-slope X/Y authority");
        }

        private static void ValidateTerrainGrid(NativeSim.TerrainGridVertex[] grid)
        {
            Require(grid != null && grid.Length == 840, "terrain grid has 40x21 vertices");
            var unique = new HashSet<int>();
            for (var index = 0; index < grid.Length; index++)
            {
                var vertex = grid[index];
                Require(
                    vertex.abiVersion == NativeSim.AbiVersion &&
                    vertex.structSize == 64 &&
                    vertex.index == index &&
                    vertex.row == index / 40 &&
                    vertex.column == index % 40 &&
                    vertex.pnt5Written == (
                        (vertex.featureFlags & 0x0002) == 0 ? 1 : 0) &&
                    vertex.reserved0 == 0 &&
                    vertex.reserved1 == 0 &&
                    vertex.reserved2 == 0 &&
                    vertex.reserved3 == 0,
                    $"terrain grid ABI vertex {index}");
                Require(
                    unique.Add(vertex.tileX + vertex.tileY * TerrainCells),
                    $"terrain grid unique map vertex {index}");
                if (vertex.pnt5Written != 0)
                {
                    Require(
                        vertex.tablesnRow == ((vertex.pnt5 >> 16) & 0xff),
                        $"terrain grid pnt5 row {index}");
                }
            }
        }

        private void RepositionTerrainCopies()
        {
            var centerX = Mathf.Floor((float)unwrappedSourceX / 65536f) * MapUnitySize;
            var centerY = Mathf.Floor((float)unwrappedSourceY / 65536f) * MapUnitySize;
            var index = 0;
            for (var y = -1; y <= 1; y++)
            {
                for (var x = -1; x <= 1; x++)
                {
                    terrainTiles[index++].position = new Vector3(
                        centerX + x * MapUnitySize,
                        0f,
                        centerY + y * MapUnitySize);
                }
            }
        }

        private void UpdateEntityPresentation()
        {
            if (entityRegistry.Count == 0) return;
            foreach (var marker in entityRegistry)
            {
                var entity = entities[marker.Index];
                var goatOrdinal =
                    goatOrdinalByEntity != null &&
                    marker.Index < goatOrdinalByEntity.Length
                        ? goatOrdinalByEntity[marker.Index]
                        : -1;
                var liveGoat = goatOrdinal >= 0;
                var sourceX = entity.x;
                var sourceY = entity.y;
                var sourceZ = entity.z;
                var sourceYaw = entity.yaw;
                var animationFrame = marker.Group?.key.animation_frame ?? 0;
                var active = entity.active != 0;
                var drawingEnabled = true;
                if (liveGoat)
                {
                    var goat = goats[goatOrdinal];
                    Require(
                        goat.entityIndex == marker.Index &&
                        goat.authority == NativeSim.GoatLive,
                        $"stable live Goat mapping {marker.Index}");
                    sourceX = goat.x;
                    sourceY = goat.y;
                    sourceZ = goat.z;
                    sourceYaw = goat.yaw;
                    animationFrame = goat.animationFrame;
                    active = goat.active != 0;
                    drawingEnabled = (goat.controlFlags & 0x00000400) == 0;
                }
                var nearestX = unwrappedSourceX +
                    ModularDelta(sourceX, presentationState.cameraX);
                var nearestY = unwrappedSourceY +
                    ModularDelta(sourceY, presentationState.cameraY);
                marker.Node.position = SourceToUnity(nearestX, nearestY, sourceZ);
                if (marker.Renderer != null)
                {
                    var nativeFrozenActor =
                        !liveGoat &&
                        marker.QualifiedFrozenActor;
                    var distanceSquared = presentationRig == null
                        ? float.PositiveInfinity
                        : Vector3.SqrMagnitude(
                            marker.Node.position - targetPosition);
                    marker.Renderer.enabled =
                        active && drawingEnabled &&
                        !(liveGoat &&
                          (terrainFrameInfo.flags &
                           NativeSim.TerrainFrameGoatsComposited) != 0) &&
                        !(nativeFrozenActor &&
                          !ManagedFrozenActorVisible(
                              1,
                              marker.QualifiedFrozenActor,
                              terrainFrameInfo.flags)) &&
                        (debugEntities || distanceSquared >= 1f);
                    if (marker.OwnerSprite && marker.Group != null &&
                        marker.SpriteVisual != null)
                    {
                        Require(
                            liveGoat
                                ? entity.authority == NativeSim.GoatLive &&
                                  marker.Group.group_id == "reset-050"
                                : entity.authority == NativeSim.FrozenObserved ||
                                  entity.authority == NativeSim.VillagerLive,
                            $"owner sprite authority {marker.Index}");
                        if (flightCamera != null)
                        {
                            marker.Node.rotation = flightCamera.transform.rotation;
                            UpdateOwnerEntitySprite(
                                marker.Index,
                                sourceYaw,
                                animationFrame,
                                marker.Group,
                                marker.SpriteVisual,
                                marker.Renderer);
                        }
                    }
                }
            }
        }

        private void RefreshLivePlayerRow()
        {
            var live = session.EntityAt(snapshot.entityIndex);
            Require(
                live.x == snapshot.x && live.y == snapshot.y && live.z == snapshot.z &&
                live.yaw == snapshot.yaw && live.pitch == snapshot.displayPitch &&
                live.actualSpeed == snapshot.actualSpeed && live.phase == snapshot.phase,
                "live player entity row agrees with atomic snapshot");
            entities[live.entityIndex] = live;
        }

        private void RefreshLiveGoats()
        {
            goats = session.Goats(
                snapshot.generation,
                snapshot.tick,
                out var batch);
            Require(
                goats.Length == ExpectedLevel1GoatEntities.Length &&
                batch.liveWorldHash != 0,
                "complete hash-bound live Goat family");
            goatLiveWorldHash = batch.liveWorldHash;
            goatBatchGeneration = batch.generation;
            goatBatchTick = batch.tick;
            for (var ordinal = 0; ordinal < goats.Length; ordinal++)
            {
                var goat = goats[ordinal];
                Require(
                    goat.entityIndex == ExpectedLevel1GoatEntities[ordinal] &&
                    goat.classId == 5 &&
                    goat.model == 1 &&
                    goat.renderParameterIndex == 238 &&
                    goat.baseTmap == 152 &&
                    goat.animationFrame == 0,
                    $"exact live Goat row {ordinal}");
                var entity = entities[goat.entityIndex];
                entity.authority = NativeSim.GoatLive;
                entity.active = goat.active;
                entity.classId = goat.classId;
                entity.model = goat.model;
                entity.action = goat.action;
                entity.phase = goat.phase;
                entity.x = goat.x;
                entity.y = goat.y;
                entity.z = goat.z;
                entity.yaw = goat.yaw;
                entity.pitch = goat.pitch;
                entity.actualSpeed = goat.actualSpeed;
                entities[goat.entityIndex] = entity;
            }
        }

        private void RefreshLiveVillagers()
        {
            if (entities == null || entities.Length <= 78) return;
            for (ushort entityIndex = 51; entityIndex <= 78; entityIndex++)
            {
                var villager = session.EntityAt(entityIndex);
                Require(
                    villager.entityIndex == entityIndex &&
                    villager.classId == 5 &&
                    villager.model == 13 &&
                    villager.authority == NativeSim.VillagerLive,
                    $"exact live Villager row {entityIndex}");
                entities[entityIndex] = villager;
            }
        }

        private void RefreshLiveMission()
        {
            var nextMission = session.Level1Mission();
            var nextHud = session.Level1Hud(nextMission);
            var nextSpellMenu = session.Level1SpellMenu(nextMission);
            var nextMinimapMarks = session.Level1MinimapMarks(nextMission);
            var nextMinimapEntities = session.Level1MinimapEntities(
                nextMission, out var nextMinimapEntitiesInfo);
            var nextPalette = ownerAssetCache == null
                ? default
                : session.Level1Palette(
                    nextMission, level1EffectivePaletteRgb8);
            var nextExitPresentation =
                session.Level1ExitPresentation(nextMission);
            Require(
                nextMission.generation == snapshot.generation &&
                nextMission.tick == snapshot.tick &&
                nextHud.generation == snapshot.generation &&
                nextHud.tick == snapshot.tick &&
                nextSpellMenu.generation == snapshot.generation &&
                nextSpellMenu.tick == snapshot.tick &&
                nextMinimapMarks.generation == snapshot.generation &&
                nextMinimapMarks.tick == snapshot.tick &&
                nextMinimapEntitiesInfo.generation == snapshot.generation &&
                nextMinimapEntitiesInfo.tick == snapshot.tick &&
                (ownerAssetCache == null ||
                    (nextPalette.generation == snapshot.generation &&
                     nextPalette.tick == snapshot.tick)) &&
                nextExitPresentation.generation == snapshot.generation &&
                nextExitPresentation.tick == snapshot.tick,
                "live Level 1 mission and exit presentation match atomic flight snapshot");
            if (simTick == 0 && nextMission.fidelityGapFlags != 0)
            {
                Debug.LogWarning(
                    $"MC_LEVEL1_FIDELITY_GAPS flags=0x{nextMission.fidelityGapFlags:x2} " +
                    $"unifiedSourcePool={((nextMission.fidelityGapFlags & 0x01) == 0).ToString().ToLowerInvariant()} " +
                    "archerTerrainAvoidance=true " +
                    "archerLinkedAction35=true");
            }
            var nextActors = session.Level1Actors(nextMission);
            var nextMultipartSegments = session.Level1MultipartSegments(nextMission);
            var nextCastle = session.Level1Castle(nextMission);
            var nextCastleLifecycle = session.Level1CastleLifecycle(nextMission);
            var nextCarrier = session.Level1ManaCarrier(nextMission);
            var nextManaSpheres = session.Level1ManaSpheres(nextMission);
            var nextProjectiles = session.FireballProjectiles(nextMission);
            var nextImpacts = session.FireballImpacts(nextMission);
            Require(
                nextActors.Length == nextMission.actorCount &&
                nextActors.Length == 45 &&
                nextMultipartSegments.Length <= 48 &&
                nextProjectiles.Length == nextMission.projectileCount &&
                nextImpacts.Length == nextMission.impactCount &&
                nextCastleLifecycle.rootActive == nextMission.castleExists &&
                nextCastleLifecycle.terrainMutationSequence ==
                    terrainState.mutationSequence,
                "complete live Level 1 mission snapshot family");
            var actorIds = new HashSet<ushort>();
            foreach (var actor in nextActors)
            {
                Require(
                    actorIds.Add(actor.authoredIndex),
                    $"unique live Level 1 actor {actor.authoredIndex}");
            }
            mission = nextMission;
            level1Hud = nextHud;
            level1SpellMenu = nextSpellMenu;
            level1MinimapMarks = nextMinimapMarks;
            level1MinimapEntities = nextMinimapEntities;
            level1MinimapEntitiesInfo = nextMinimapEntitiesInfo;
            level1Palette = nextPalette;
            exitPresentation = nextExitPresentation;
            missionActors = nextActors;
            multipartSegments = nextMultipartSegments;
            level1Castle = nextCastle;
            castleLifecycle = nextCastleLifecycle;
            manaCarrier = nextCarrier;
            manaSpheres = nextManaSpheres;
            fireballProjectiles = nextProjectiles;
            fireballImpacts = nextImpacts;
            ApplyExitPalettePresentation();
        }

        private void DrainEvents()
        {
            var buffer = new NativeSim.NativeEvent[64];
            while (true)
            {
                var status = session.CopyEvents(
                    snapshot.generation,
                    eventCursor,
                    buffer,
                    out var batch,
                    out var error);
                if (status != NativeSim.Ok)
                {
                    throw new InvalidOperationException(
                        $"drain events status {status}: {error}");
                }
                for (var index = 0; index < batch.written; index++)
                {
                    var nativeEvent = buffer[index];
                    playthroughRecorder?.RecordEvent(nativeEvent);
                    if (autoplay)
                    {
                        AppendCanonicalEvent(
                            autoplayEventContentBytes,
                            autoplayEventLifecycleBytes,
                            nativeEvent);
                    }
                    AddEventDiagnostic(
                        $"{nativeEvent.sequence}:kind={nativeEvent.kind} " +
                        $"code={nativeEvent.code} asset={nativeEvent.assetRefIndex}");
                    var goatEvent =
                        nativeEvent.kind >= NativeSim.EventGoatSoundRequest &&
                        nativeEvent.kind <= NativeSim.EventGoatEngagementBoundary;
                    if (goatEvent)
                    {
                        HandleGoatEvent(nativeEvent);
                        if (nativeEvent.kind == NativeSim.EventGoatSoundRequest &&
                            ownerAudio != null)
                        {
                            NativeSim.AudioCommand? audioCommand = null;
                            if (session.TryAudioCommand(
                                nativeEvent, out var command, out _))
                            {
                                audioCommand = command;
                            }
                            ownerAudio.Consume(nativeEvent, audioCommand);
                        }
                    }
                    else if (ownerAudio != null &&
                        EventUsesAudioSidecar(nativeEvent.kind) &&
                        OwnerAudioPresentation.EventSoundId(
                            nativeEvent.kind, nativeEvent.code) != 12)
                    {
                        NativeSim.AudioCommand? audioCommand = null;
                        if (session.TryAudioCommand(
                            nativeEvent, out var command, out _))
                        {
                            audioCommand = command;
                        }
                        ownerAudio.Consume(nativeEvent, audioCommand);
                    }
                    else
                    {
                        ownerAudio?.Consume(nativeEvent);
                    }
                    if (nativeEvent.kind == NativeSim.EventSpellActivation)
                    {
                        fireEventsObserved++;
                        Debug.Log(
                            $"MC_FIRE kind4_event sequence={nativeEvent.sequence} " +
                            $"count={fireEventsObserved}");
                        if (debugMode && ownerAssetCache == null)
                        {
                            SpawnDiagnosticFireVisual(nativeEvent);
                        }
                        else if (ownerAssetCache == null)
                        {
                            Debug.Log(
                                $"MC_FIRE visual_suppressed sequence={nativeEvent.sequence} " +
                                "reason=projectile-presentation-not-qualified");
                        }
                    }
                }
                eventCursor = batch.nextCursor;
                if (batch.hasMore == 0) break;
            }
        }

        internal static bool EventUsesAudioSidecar(uint kind)
        {
            return kind == NativeSim.EventPrepareSound ||
                kind == NativeSim.EventEndLoop ||
                kind == NativeSim.EventGoatSoundRequest ||
                kind == NativeSim.EventLevel1ArcherArrowSound ||
                kind == NativeSim.EventLevel1FireflyChargeSound ||
                kind == NativeSim.EventLevel1FireballImpacted ||
                kind == NativeSim.EventLevel1CastleProjectileSpawned ||
                kind == NativeSim.EventLevel1CastleConstructionSound ||
                kind == NativeSim.EventLevel1ManaSphereClaimed ||
                kind == NativeSim.EventLevel1WaterSplashed;
        }

        private void HandleGoatEvent(NativeSim.NativeEvent nativeEvent)
        {
            Require(
                nativeEvent.kind >= NativeSim.EventGoatSoundRequest &&
                nativeEvent.kind <= NativeSim.EventGoatEngagementBoundary,
                "typed Goat event range");
            qualificationGoatEvents.Add(nativeEvent);
            if (nativeEvent.kind == NativeSim.EventGoatSoundRequest)
            {
                Require(
                    nativeEvent.code == 46,
                    "Goat sound request identifies exact owner sound 46");
                Debug.Log(
                    $"MC_GOAT_SOUND_REQUEST sequence={nativeEvent.sequence} " +
                    $"entity={nativeEvent.entityIndex} sound=46 " +
                    "playback=authoritative-audio-sidecar");
                return;
            }
            Debug.Log(
                $"MC_GOAT_EVENT sequence={nativeEvent.sequence} " +
                $"kind={nativeEvent.kind} entity={nativeEvent.entityIndex} " +
                $"value0={nativeEvent.value0} value1={nativeEvent.value1} " +
                "state=bulk-snapshot-authoritative");
        }

        private void SpawnDiagnosticFireVisual(NativeSim.NativeEvent nativeEvent)
        {
            if (flightCamera == null) return;
            var fireball = GameObject.CreatePrimitive(PrimitiveType.Sphere);
            fireball.name = $"DIAGNOSTIC spell event {nativeEvent.sequence} — not projectile";
            Destroy(fireball.GetComponent<Collider>());
            fireball.transform.position =
                flightCamera.transform.position + flightCamera.transform.forward * 5f;
            fireball.transform.localScale = Vector3.one * 0.24f;
            fireball.GetComponent<Renderer>().material = fireballMaterial;
            var visual = fireball.AddComponent<DiagnosticFireVisual>();
            visual.Velocity = flightCamera.transform.forward * 22f;
            var trail = fireball.AddComponent<TrailRenderer>();
            trail.time = 0.9f;
            trail.startWidth = 0.20f;
            trail.endWidth = 0f;
            trail.material = fireballMaterial;
            fireVisualSpawnedCount++;
            Debug.Log(
                $"MC_FIRE visual_spawned sequence={nativeEvent.sequence} " +
                $"count={fireVisualSpawnedCount}");
        }

        private void AddEventDiagnostic(string value)
        {
            recentEventDiagnostics.Enqueue(value);
            while (recentEventDiagnostics.Count > 4) recentEventDiagnostics.Dequeue();
        }

        private void ResetSession()
        {
            snapshot = session.Reset();
            playthroughRecorder?.RecordReset(snapshot);
            ownerAudio?.Reset();
            eventCursor = 0;
            qualificationGoatEvents.Clear();
            simTick = 0;
            CenterSteering();
            fireRequested = false;
            rightSpellRequested = false;
            selectedSpellRequested = false;
            requestedSpell = NativeSim.NoRequestedSpell;
            spellMenuOpen = false;
            spellMenuHoverUiIndex = -1;
            spellMenuSelectedUiIndex = -1;
            spellMenuHoverTier = -1;
            spellMenuController.Close();
            spellMenuCommands.Clear();
            terrainFrameGpuValidated = false;
            terrainFrameRouteGpuValidated = false;
            qualifiedTerrainRouteTicks.Clear();
            qualifiedTerrainGpuRouteTicks.Clear();
            autoplayCommandBytes.SetLength(0);
            autoplayEventContentBytes.SetLength(0);
            autoplayEventLifecycleBytes.SetLength(0);
            autoplayEventRangeStart = 1;
            InitializeUnwrappedPose();
            RefreshLiveTerrain();
            RefreshLivePlayerRow();
            RefreshLiveGoats();
            RefreshLiveVillagers();
            RefreshLiveMission();
            ReadAuthoritativePose(true);
            UpdateExactTerrainGrid();
        }

        private ulong ResetNativeGenerationForAudioQualification()
        {
            snapshot = session.Reset();
            eventCursor = 0;
            simTick = 0;
            Debug.Log(
                $"MC_AUDIO_LOCAL_QUAL NATIVE_RESET generation={snapshot.generation}");
            return snapshot.generation;
        }

        private static List<RouteCheckpointRecord> CollectAutoplayRouteEvidence(
            NativeSession routeSession)
        {
            var records = new List<RouteCheckpointRecord>();
            using var commandBytes = new MemoryStream();
            using var eventContentBytes = new MemoryStream();
            using var eventLifecycleBytes = new MemoryStream();
            var eventBuffer = new NativeSim.NativeEvent[64];
            var terrainIndices = new byte[NativeSim.TerrainFramePixels];
            var terrainCoverage = new byte[NativeSim.TerrainFramePixels];
            ulong cursor = 0;
            ulong rangeStart = 1;

            // DrawWorld services draw-gated selector clocks on every outer
            // turn. Prime the reset frame and render every stepped frame so
            // this headless evidence route has the same presentation history
            // as the graphical player; checkpoint-only sampling would create
            // a different animation timeline.
            var routePresentation = routeSession.PresentationState();
            var routeCamera = routeSession.RenderCameraState();
            _ = routeSession.WorldFrame(
                routeCamera,
                terrainIndices,
                terrainCoverage);

            for (var commandIndex = 0; commandIndex < AutoplayFinalTick; commandIndex++)
            {
                BuildAutoplayCommand(
                    commandIndex,
                    out var roll,
                    out var pitch,
                    out var flags);
                AppendAutoplayCommand(
                    commandBytes,
                    roll,
                    pitch,
                    unchecked((sbyte)flags));
                var routeSnapshot = routeSession.Step(
                    roll,
                    pitch,
                    unchecked((sbyte)flags));
                while (true)
                {
                    var status = routeSession.CopyEvents(
                        routeSnapshot.generation,
                        cursor,
                        eventBuffer,
                        out var batch,
                        out var error);
                    Require(status == NativeSim.Ok, "route event copy " + error);
                    for (var index = 0; index < batch.written; index++)
                    {
                        var nativeEvent = eventBuffer[index];
                        Require(
                            nativeEvent.generation == routeSnapshot.generation &&
                            nativeEvent.sequence == cursor + (ulong)index + 1,
                            "route event generation and sequence continuity");
                        AppendCanonicalEvent(
                            eventContentBytes,
                            eventLifecycleBytes,
                            nativeEvent);
                    }
                    cursor = batch.nextCursor;
                    if (batch.hasMore == 0) break;
                }
                Require(cursor == routeSnapshot.eventHead, "route cursor reaches event head");

                routePresentation = routeSession.PresentationState();
                routeCamera = routeSession.RenderCameraState();
                var routeTerrain = routeSession.WorldFrame(
                    routeCamera,
                    terrainIndices,
                    terrainCoverage);
                var checkpointTick = commandIndex + 1;
                var checkpointIndex = Array.IndexOf(AutoplayCaptureTicks, checkpointTick);
                if (checkpointIndex < 0) continue;
                var count = routeSnapshot.eventHead >= rangeStart
                    ? routeSnapshot.eventHead - rangeStart + 1
                    : 0;
                var record = new RouteCheckpointRecord {
                    checkpoint_tick = checkpointTick,
                    last_command_index = commandIndex,
                    command_prefix_sha256 = Sha256Hex(commandBytes.ToArray()),
                    snapshot = SnapshotEvidenceFrom(
                        routeSnapshot,
                        routePresentation.sourceTurn),
                    events = new EventRangeEvidence {
                        content_schema =
                            "mc2-native-event-logical-content-le/sha256-v1",
                        lifecycle_schema =
                            "mc2-native-event-logical-lifecycle-le/sha256-v1",
                        first_sequence = rangeStart,
                        last_sequence = routeSnapshot.eventHead,
                        count = count,
                        content_sha256 = Sha256Hex(eventContentBytes.ToArray()),
                        lifecycle_sha256 = Sha256Hex(eventLifecycleBytes.ToArray()),
                        cursor_equals_head = cursor == routeSnapshot.eventHead
                    },
                    presentation = PresentationEvidenceFrom(routePresentation),
                    render_camera = RenderCameraEvidenceFrom(routeCamera),
                    terrain = TerrainFrameEvidenceFrom(routeTerrain)
                };
                records.Add(record);
                Debug.Log(
                    $"MC_ROUTE_OBSERVED tick={record.checkpoint_tick} " +
                    $"recordJson={JsonUtility.ToJson(record)}");
                rangeStart = routeSnapshot.eventHead + 1;
                eventContentBytes.SetLength(0);
                eventLifecycleBytes.SetLength(0);
            }
            Require(
                records.Count == AutoplayCaptureTicks.Length,
                "all route checkpoints collected");
            for (var index = 0; index < records.Count; index++)
            {
                ValidateAutoplayRouteEvidence(records[index], index);
            }
            return records;
        }

        private static void ValidateAutoplayRouteEvidence(
            RouteCheckpointRecord record,
            int index)
        {
            Require(
                record.checkpoint_tick == AutoplayCaptureTicks[index] &&
                record.last_command_index == AutoplayCaptureTicks[index] - 1 &&
                record.command_prefix_sha256 == AutoplayCommandPrefixHashes[index],
                $"route command identity {record.checkpoint_tick}");
            Require(
                record.snapshot.tick == (ulong)record.checkpoint_tick &&
                record.snapshot.event_head == AutoplayEventHeads[index] &&
                record.snapshot.state_hash_fnv1a64 ==
                    $"{AutoplayStateHashes[index]:x16}" &&
                record.snapshot.conformance_hash_fnv1a64 ==
                    $"{AutoplayConformanceHashes[index]:x16}",
                $"route snapshot identity {record.checkpoint_tick}");
            Require(
                record.events.cursor_equals_head &&
                record.events.content_sha256 == AutoplayEventContentHashes[index],
                $"route event identity {record.checkpoint_tick}");
            if (record.snapshot.generation == 1)
            {
                Require(
                    record.events.lifecycle_sha256 ==
                        AutoplayEventLifecycleHashes[index],
                    $"route lifecycle event identity {record.checkpoint_tick}");
            }
            Debug.Log(
                $"MC_ROUTE_CHECKPOINT tick={record.checkpoint_tick} " +
                $"generation={record.snapshot.generation} " +
                $"eventContent={record.events.content_sha256} " +
                $"eventLifecycle={record.events.lifecycle_sha256} " +
                $"recordJson={JsonUtility.ToJson(record)}");
            Require(
                record.presentation.tick == (ulong)(record.checkpoint_tick - 1) &&
                record.render_camera.tick == record.presentation.tick &&
                record.render_camera.source_turn == record.presentation.source_turn &&
                record.terrain.tick == record.presentation.tick,
                $"route camera clock identity {record.checkpoint_tick}");
            Require(
                record.terrain.index_hash_fnv1a64 ==
                    $"{AutoplayTerrainIndexHashes[index]:x16}" &&
                record.terrain.coverage_hash_fnv1a64 ==
                    $"{AutoplayTerrainCoverageHashes[index]:x16}" &&
                record.terrain.reflected_triangles == AutoplayTerrainReflected[index] &&
                record.terrain.primary_mode5_triangles == AutoplayTerrainMode5[index] &&
                record.terrain.primary_mode26_triangles == AutoplayTerrainMode26[index] &&
                record.terrain.terrain_pixel_writes == AutoplayTerrainWrites[index],
                $"route terrain identity {record.checkpoint_tick}");
        }

        private static string NormalizedRouteEvidence(
            List<RouteCheckpointRecord> records)
        {
            var normalized = new List<RouteCheckpointRecord>(records.Count);
            foreach (var record in records)
            {
                var copy = JsonUtility.FromJson<RouteCheckpointRecord>(
                    JsonUtility.ToJson(record));
                copy.snapshot.generation = 0;
                copy.presentation.generation = 0;
                copy.render_camera.generation = 0;
                copy.terrain.generation = 0;
                copy.events.lifecycle_sha256 = "generation-normalized";
                normalized.Add(copy);
            }
            return JsonUtility.ToJson(
                new RouteCheckpointRecordList { records = normalized });
        }

        [Serializable]
        private sealed class RouteCheckpointRecordList
        {
            public List<RouteCheckpointRecord> records;
        }

        private void RunHeadlessSelfTest()
        {
            enabled = false;
            Require(packagePath != null && closurePath != null, "explicit package paths");
            var safeCaptureProbe = Path.Combine(
                Path.GetTempPath(), "magiccarpet-audio-path-selftest.wav");
            Require(
                AudioQualificationCapture.ResolveQualifiedCapturePath(safeCaptureProbe) ==
                    Path.GetFullPath(safeCaptureProbe),
                "audio capture accepts a direct system-temporary path");
            Require(
                RejectsAudioCapturePath(Application.dataPath),
                "audio capture rejects Unity build/data paths");
            Require(
                RejectsAudioCapturePath(
                    Path.Combine(Path.GetTempPath(), "..", "magiccarpet-escape.wav")),
                "audio capture rejects parent escape from system temporary root");
            if (ownerAssetCache != null)
            {
                Require(
                    hudDescriptor != null &&
                    hudDescriptor.objective.selector == 48 &&
                    hudDescriptor.objective.authority ==
                        "SourceMappedLiveIndexedFramebuffer" &&
                    hudDescriptor.objective.background_blend.blend_selector == 61 &&
                    hudDescriptor.minimap.entity_positions_authority ==
                        "SourceArenaLive" &&
                    hudDescriptor.minimap.player_pose_authority == "PlayerFlightLive" &&
                    hudDescriptor.top_bar.health_mana_bars.authority ==
                        "MissionStateLiveSourceMapped" &&
                    hudDescriptor.spells.slots[0].activation_authority ==
                        "SpellActivationLive",
                    "headless owner HUD descriptor authority");
                Require(ownerAudio != null && worldInfo.assetRefCount == 109,
                    "headless descriptor-backed owner audio");
                var cleanupBefore =
                    OwnerAudioPresentation.QualificationTransactionalCleanupCount;
                var forcedLoadFailureObserved = false;
                try
                {
                    OwnerAudioPresentation.Load(
                        ownerAssetCache,
                        "audio/sound-bank-1-822.json",
                        "audio/music-channel-0-f.json",
                        transform,
                        qualificationFailAfterSoundRows: 2);
                }
                catch (InvalidDataException exception)
                {
                    forcedLoadFailureObserved =
                        exception.Message ==
                            "qualification-forced owner-audio load failure";
                }
                Require(
                    forcedLoadFailureObserved &&
                    OwnerAudioPresentation.QualificationTransactionalCleanupCount ==
                        cleanupBefore + 1,
                    "partial owner-audio construction is transactionally disposed");
            }
            Require(ModularDelta(0, 65535) == 1, "forward wrap delta");
            Require(ModularDelta(65535, 0) == -1, "reverse wrap delta");

            using (var idleRoute = OpenSessionWithAssetCache(
                       packagePath, closurePath, ownerAssetCache?.RootPath))
            {
                Require(
                    idleRoute.Snapshot().stateHash == ExpectedResetHash,
                    "original idle frame 0 state hash");
                ulong idleFrame8NativeHash = 0;
                ulong idleFrame19NativeHash = 0;
                ulong idleFrame8SourceHash = 0;
                ulong idleFrame19SourceHash = 0;
                for (var idleStep = 1; idleStep <= 19; idleStep++)
                {
                    var idleSnapshot = idleRoute.Step(0, 0, 0);
                    if (idleStep == 8)
                    {
                        idleFrame8NativeHash = idleSnapshot.stateHash;
                        idleFrame8SourceHash = idleRoute.SourceProjectionHash();
                    }
                    else if (idleStep == 19)
                    {
                        idleFrame19NativeHash = idleSnapshot.stateHash;
                        idleFrame19SourceHash = idleRoute.SourceProjectionHash();
                    }
                }
                Require(
                    idleFrame8SourceHash == 0x0e4547b42a98c976 &&
                    idleFrame19SourceHash == 0x13f7f355fcf6738f,
                    "neutral native replay matches original idle source projections");
                Debug.Log(
                    "MC_IDLE_QUAL PASS exactReset=true sourceProjectionExact=true " +
                    $"nativeHashes=9da4bc91af58f3ce,{idleFrame8NativeHash:x16}," +
                    $"{idleFrame19NativeHash:x16} " +
                    $"sourceHashes=9da4bc91af58f3ce,{idleFrame8SourceHash:x16}," +
                    $"{idleFrame19SourceHash:x16}");
            }

            using (var routeFirst = OpenSessionWithAssetCache(
                       packagePath, closurePath, ownerAssetCache?.RootPath))
            using (var routeSecond = OpenSessionWithAssetCache(
                       packagePath, closurePath, ownerAssetCache?.RootPath))
            {
                var routeReset = routeFirst.Snapshot();
                var routeResetPresentation = routeFirst.PresentationState();
                Require(
                    routeReset.tick == 0 &&
                    routeReset.eventHead == 0 &&
                    routeReset.stateHash == ExpectedResetHash &&
                    routeFirst.SourceProjectionHash() == ExpectedResetHash &&
                    routeReset.conformanceHash == ExpectedSessionConformanceHash &&
                    routeResetPresentation.generation == routeReset.generation &&
                    routeResetPresentation.tick == 0 &&
                    routeResetPresentation.sourceTurn == 1,
                    "route reset registry identity");
                Debug.Log(
                    "MC_ROUTE_RESET PASS tick=0 eventHead=0 " +
                    $"state={routeReset.stateHash:x16} " +
                    $"sourceProjection={routeFirst.SourceProjectionHash():x16} " +
                    $"packageConformance={ExpectedResetConformanceHash:x16} " +
                    $"sessionConformance={routeReset.conformanceHash:x16} " +
                    $"sourceTurn={routeResetPresentation.sourceTurn} " +
                    $"position={routeReset.x},{routeReset.y},{routeReset.z}");
                var firstRoute = CollectAutoplayRouteEvidence(routeFirst);
                var secondRoute = CollectAutoplayRouteEvidence(routeSecond);
                Require(
                    JsonUtility.ToJson(
                        new RouteCheckpointRecordList { records = firstRoute }) ==
                    JsonUtility.ToJson(
                        new RouteCheckpointRecordList { records = secondRoute }),
                    "independent sessions reproduce the complete autoplay route");
                var routeGenerationBeforeReset = firstRoute[0].snapshot.generation;
                var resetSnapshot = routeFirst.Reset();
                Require(
                    resetSnapshot.generation != routeGenerationBeforeReset &&
                    resetSnapshot.tick == 0 &&
                    resetSnapshot.eventHead == 0 &&
                    resetSnapshot.stateHash == ExpectedResetHash,
                    "route reset advances generation and restores the exact baseline");
                var staleEvents = new NativeSim.NativeEvent[1];
                var staleStatus = routeFirst.CopyEvents(
                    routeGenerationBeforeReset,
                    0,
                    staleEvents,
                    out _,
                    out _);
                Require(
                    staleStatus != NativeSim.Ok,
                    "route reset invalidates the old generation");
                var replayRoute = CollectAutoplayRouteEvidence(routeFirst);
                Require(
                    replayRoute.Count == firstRoute.Count,
                    "route reset replay checkpoint count");
                for (var index = 0; index < firstRoute.Count; index++)
                {
                    var beforeReset = firstRoute[index];
                    var afterReset = replayRoute[index];
                    Require(
                        afterReset.snapshot.generation == resetSnapshot.generation &&
                        afterReset.events.content_sha256 ==
                            beforeReset.events.content_sha256 &&
                        afterReset.events.lifecycle_sha256 !=
                            beforeReset.events.lifecycle_sha256 &&
                        afterReset.events.first_sequence ==
                            beforeReset.events.first_sequence &&
                        afterReset.events.last_sequence ==
                            beforeReset.events.last_sequence &&
                        afterReset.events.count == beforeReset.events.count,
                        $"route reset lifecycle identity {afterReset.checkpoint_tick}");
                }
                Require(
                    NormalizedRouteEvidence(firstRoute) ==
                    NormalizedRouteEvidence(secondRoute) &&
                    NormalizedRouteEvidence(firstRoute) ==
                    NormalizedRouteEvidence(replayRoute),
                    "route reset replay preserves normalized checkpoint evidence");
                Debug.Log(
                    "MC_ROUTE_QUAL PASS checkpoints=5 steps=440 " +
                    "sessions=2 resetReplay=true generationInvalidation=true " +
                    "state=true camera=true terrain=true events=true");
            }

            using var first = OpenSessionWithAssetCache(
                packagePath, closurePath, ownerAssetCache?.RootPath);
            using var second = OpenSessionWithAssetCache(
                packagePath, closurePath, ownerAssetCache?.RootPath);
            var firstSnapshot = first.Snapshot();
            var secondSnapshot = second.Snapshot();
            Require(firstSnapshot.stateHash == secondSnapshot.stateHash, "independent reset states");
            var expectedCameraSnapshot = firstSnapshot;
            for (var tick = 0; tick < 8; tick++)
            {
                expectedCameraSnapshot = firstSnapshot;
                firstSnapshot = first.Step(0, 0, 1);
            }
            Require(firstSnapshot.stateHash == 0x956246bd7f270164, "8-forward state hash");
            Require(
                firstSnapshot.x == 19840 && firstSnapshot.y == 56480 &&
                firstSnapshot.z == 5040 && firstSnapshot.phase == 161,
                "8-forward route");
            Require(second.Snapshot().stateHash == ExpectedResetHash, "second session independence");
            var player = first.EntityAt(firstSnapshot.entityIndex);
            Require(
                player.x == firstSnapshot.x && player.y == firstSnapshot.y &&
                player.z == firstSnapshot.z && player.phase == firstSnapshot.phase,
                "headless live player row");
            var firstPresentation = first.PresentationState();
            ValidatePresentationState(firstPresentation);
            var firstRenderCamera = first.RenderCameraState();
            ValidateRenderCameraState(firstRenderCamera, firstPresentation);
            ValidateTerrainGrid(first.TerrainGrid());
            Require(
                firstPresentation.generation == firstSnapshot.generation &&
                firstPresentation.tick == expectedCameraSnapshot.tick &&
                firstPresentation.cameraX == expectedCameraSnapshot.x &&
                firstPresentation.cameraY == expectedCameraSnapshot.y &&
                firstPresentation.cameraZ == expectedCameraSnapshot.z + 128,
                "headless presentation camera is the stored pre-entity-update row");
            Require(
                Mathf.Abs(firstRenderCamera.slopeTargetX) <= 100 &&
                Mathf.Abs(firstRenderCamera.slopeTargetY) <= 100 &&
                Mathf.Abs(firstRenderCamera.renderOffsetX) <= 100 &&
                Mathf.Abs(firstRenderCamera.renderOffsetY) <= 100,
                "headless DrawWorld terrain-slope targets and filters are bounded");
            var lookSource = firstSnapshot;
            firstSnapshot = first.Step(
                0,
                0,
                0,
                64,
                unchecked((ushort)(short)-32),
                8,
                4);
            firstPresentation = first.PresentationState();
            ValidatePresentationState(firstPresentation);
            firstRenderCamera = first.RenderCameraState();
            ValidateRenderCameraState(firstRenderCamera, firstPresentation);
            Require(
                firstPresentation.tick == lookSource.tick &&
                firstPresentation.cameraX == lookSource.x &&
                firstPresentation.cameraY == lookSource.y &&
                firstPresentation.cameraZ == lookSource.z + 128 &&
                firstPresentation.cameraYaw ==
                    unchecked((short)(lookSource.yaw + 64)) &&
                firstRenderCamera.yaw ==
                    ((lookSource.yaw + 64) & 0x07ff) &&
                firstPresentation.cameraPitch ==
                    SignedAngle11(lookSource.displayPitch) / 2 - 32 &&
                firstPresentation.cameraFov == 132,
                "headless additive camera input tail preserves look yaw/pitch and FOV");

            firstSnapshot = first.Step(0, 0, 0x10);
            var eventBuffer = new NativeSim.NativeEvent[64];
            var sawFire = false;
            var sawAudio = false;
            var sawMusicStart = false;
            ulong previous = 0;
            ulong qualificationCursor = 0;
            NativeSim.EventBatch batch;
            int status;
            string eventError;
            while (true)
            {
                status = first.CopyEvents(
                    firstSnapshot.generation,
                    qualificationCursor,
                    eventBuffer,
                    out batch,
                    out eventError);
                Require(status == NativeSim.Ok, "fire event copy " + eventError);
                for (var index = 0; index < batch.written; index++)
                {
                    Require(
                        eventBuffer[index].sequence > previous,
                        "ordered event sequence");
                    previous = eventBuffer[index].sequence;
                    sawFire |=
                        eventBuffer[index].kind == NativeSim.EventSpellActivation;
                    sawMusicStart |=
                        eventBuffer[index].kind == NativeSim.EventStartMusic;
                    if (
                        (eventBuffer[index].kind >= NativeSim.EventPrepareSound &&
                         eventBuffer[index].kind <= NativeSim.EventUpdateMusic) ||
                        eventBuffer[index].kind == NativeSim.EventStartMusic)
                    {
                        sawAudio = true;
                        var isSourceNoSoundSentinel =
                            eventBuffer[index].kind ==
                                NativeSim.EventPrepareSound &&
                            eventBuffer[index].code == ushort.MaxValue;
                        Require(
                            ownerAssetCache != null
                                ? isSourceNoSoundSentinel
                                    ? eventBuffer[index].assetRefIndex ==
                                        uint.MaxValue
                                    : eventBuffer[index].assetRefIndex >= 76 &&
                                      eventBuffer[index].assetRefIndex <= 108
                                : eventBuffer[index].assetRefIndex == uint.MaxValue,
                            ownerAssetCache != null
                                ? "typed audio event descriptor-backed asset reference " +
                                  "or exact remc2 -1 no-sound sentinel"
                                : "debug audio event has no absent-cache asset reference");
                        if (isSourceNoSoundSentinel)
                        {
                            Require(
                                !first.TryAudioCommand(
                                    eventBuffer[index], out _, out _),
                                "exact remc2 -1 no-sound sentinel has no audio sidecar");
                        }
                        else if (ownerAudio != null &&
                            (eventBuffer[index].kind ==
                                NativeSim.EventPrepareSound ||
                             eventBuffer[index].kind ==
                                NativeSim.EventEndLoop) &&
                            eventBuffer[index].code != 12)
                        {
                            Require(
                                first.TryAudioCommand(
                                    eventBuffer[index], out _, out _),
                                "playable effect event has a shared-voice sidecar");
                        }
                        else if (
                            eventBuffer[index].kind ==
                                NativeSim.EventUpdateMusic ||
                            eventBuffer[index].kind ==
                                NativeSim.EventStartMusic ||
                            eventBuffer[index].code == 12)
                        {
                            ownerAudio?.Consume(eventBuffer[index]);
                        }
                    }
                }
                qualificationCursor = batch.nextCursor;
                if (batch.hasMore == 0) break;
            }
            Require(sawFire, "typed fire event");
            Require(ownerAssetCache == null || sawAudio, "typed owner audio event");
            Require(
                ownerAssetCache == null || sawMusicStart,
                "typed music start arrives from the source-turn lifecycle");
            Require(ownerAudio == null || !ownerAudio.HadSuppressedUnknown,
                "no unknown audio mapping in qualification event vector");
            Require(
                ownerAudio == null ||
                !ownerAudio.HadSuppressedUnsupportedPositional,
                "qualified Level 1 vector emits no missing positional sidecar");
            if (ownerAudio != null)
            {
                session.Reset();
                ownerAudio.Reset();
                Require(
                    !ownerAudio.QualificationMusicIsPlaying &&
                    !ownerAudio.QualificationMusicUpdatesEnabled &&
                    !ownerAudio.QualificationMusicTimerActive &&
                    ownerAudio.QualificationMusicStartCount == 0 &&
                    ownerAudio.QualificationMusicTargetMode == 1 &&
                    ownerAudio.QualificationMusicCurrentUnits == 0 &&
                    ownerAudio.QualificationMusicDirection == -1,
                    "music reset restores the exact source control state");
                var musicStartEvent = new NativeSim.NativeEvent {
                    kind = NativeSim.EventStartMusic,
                    code = 1,
                    value0 = 0,
                    value1 = 1,
                    assetRefIndex = 108
                };
                ownerAudio.Consume(musicStartEvent);
                Require(
                    ownerAudio.QualificationMusicIsPlaying &&
                    ownerAudio.QualificationMusicUpdatesEnabled &&
                    ownerAudio.QualificationMusicStartCount == 1 &&
                    !ownerAudio.QualificationMusicTimerActive,
                    "source-turn event starts the qualified Level 1 base track once");
                var combatMusicEvent = new NativeSim.NativeEvent {
                    kind = NativeSim.EventUpdateMusic,
                    code = 2,
                    value0 = 3,
                    assetRefIndex = 108
                };
                ownerAudio.Consume(combatMusicEvent);
                Require(
                    ownerAudio.QualificationMusicTimerActive &&
                    ownerAudio.QualificationMusicTargetMode == 2 &&
                    ownerAudio.QualificationMusicDirection == 1 &&
                    ownerAudio.QualificationMusicTimerMultiplier == 3,
                    "mode 2 arms the exact 90 Hz upward envelope");
                ownerAudio.QualificationAdvanceMusicCallbacks(127);
                Require(
                    ownerAudio.QualificationMusicCurrentUnits == 127 &&
                    ownerAudio.QualificationMusicTimerActive,
                    "127 callbacks reach the exact upper endpoint");
                ownerAudio.QualificationAdvanceMusicCallbacks(1);
                Require(!ownerAudio.QualificationMusicTimerActive,
                    "the following callback retires the source timer");
                combatMusicEvent.code = 1;
                ownerAudio.Consume(combatMusicEvent);
                ownerAudio.QualificationAdvanceMusicCallbacks(19);
                Require(
                    ownerAudio.QualificationMusicCurrentUnits == 108 &&
                    ownerAudio.QualificationMusicDirection == -1,
                    "mid-lifecycle mode 1 reverses from the current integer value");
                ownerAudio.Reset();
                session.SubmitAudioQualification(
                    NativeSim.EventPrepareSound, 29, 0, 0x7fff, 0x7fff, 3,
                    out var oneShotEvent, out var oneShotCommand);
                ownerAudio.Consume(oneShotEvent, oneShotCommand, false);
                Require(ownerAudio.QualificationOneShotPlayCount == 1,
                    "proven local feedback one-shot is playable");
                oneShotEvent.sequence++;
                oneShotEvent.code = 37;
                oneShotEvent.assetRefIndex = 96;
                ownerAudio.Consume(oneShotEvent);
                Require(
                    ownerAudio.HadSuppressedUnsupportedPositional &&
                    ownerAudio.QualificationOneShotPlayCount == 1,
                    "positional event missing its sidecar is structured and silent");
                ownerAudio.Reset();
                session.SubmitAudioQualification(
                    NativeSim.EventPrepareSound, 2, 0, 0, 0x7fff,
                    NativeSim.AudioPlayIfNotPlaying,
                    out var envelopeEvent, out var envelopeCommand);
                ownerAudio.Consume(envelopeEvent, envelopeCommand, false);
                ownerAudio.Tick(10.01f / 60f);
                Require(ownerAudio.QualificationLoopUnits(2) == 10,
                    "loop starts at zero and ramps one unit per 60 Hz callback");
                session.SubmitAudioQualification(
                    NativeSim.EventPrepareSound, 2, 0, 0, 0x7fff,
                    NativeSim.AudioPlayIfNotPlaying,
                    out envelopeEvent, out envelopeCommand);
                ownerAudio.Consume(envelopeEvent, envelopeCommand, false);
                ownerAudio.Tick(5.01f / 60f);
                Require(ownerAudio.QualificationLoopUnits(2) == 15,
                    "repeated PrepareSound does not restart its loop envelope");
                session.SubmitAudioQualification(
                    NativeSim.EventEndLoop, 2, 0, 0, 0x7fff,
                    NativeSim.AudioPlayIfNotPlaying,
                    out envelopeEvent, out envelopeCommand);
                ownerAudio.Consume(envelopeEvent, envelopeCommand, false);
                ownerAudio.Tick(15.01f / 60f);
                Require(ownerAudio.QualificationLoopUnits(2) == 0,
                    "EndLoop fades to zero one unit per 60 Hz callback");
                Require(ownerAudio.QualificationLoopIsPlaying(2),
                    "zero-volume loop remains resident in the shared voice pool");
            }
            status = first.CopyEvents(
                firstSnapshot.generation,
                qualificationCursor,
                eventBuffer,
                out var emptyBatch,
                out eventError);
            Require(status == NativeSim.Ok && emptyBatch.written == 0, "event cursor exhaustion");
            var oldGeneration = firstSnapshot.generation;
            var reset = first.Reset();
            Require(reset.generation != oldGeneration, "reset increments generation");
            var resetPresentation = first.PresentationState();
            ValidatePresentationState(resetPresentation);
            var resetRenderCamera = first.RenderCameraState();
            ValidateRenderCameraState(resetRenderCamera, resetPresentation);
            var independentResetRenderCamera = second.RenderCameraState();
            var independentResetPresentation = second.PresentationState();
            ValidateRenderCameraState(
                independentResetRenderCamera,
                independentResetPresentation);
            ValidateTerrainGrid(first.TerrainGrid());
            ValidateTerrainGrid(second.TerrainGrid());
            NativeSim.TerrainFrameInfo resetTerrainFrame = default;
            if (ownerAssetCache != null)
            {
                var resetIndices = new byte[NativeSim.TerrainFramePixels];
                var resetCoverage = new byte[NativeSim.TerrainFramePixels];
                var repeatedIndices = new byte[NativeSim.TerrainFramePixels];
                var repeatedCoverage = new byte[NativeSim.TerrainFramePixels];
                var independentIndices = new byte[NativeSim.TerrainFramePixels];
                var independentCoverage = new byte[NativeSim.TerrainFramePixels];
                var uploadIndices = new byte[NativeSim.TerrainFramePixels];
                var restoredIndices = new byte[NativeSim.TerrainFramePixels];
                resetTerrainFrame = first.TerrainFrame(
                    resetRenderCamera, resetIndices, resetCoverage);
                var repeatedTerrainFrame = first.TerrainFrame(
                    resetRenderCamera, repeatedIndices, repeatedCoverage);
                var independentTerrainFrame = second.TerrainFrame(
                    independentResetRenderCamera,
                    independentIndices,
                    independentCoverage);
                Require(
                    resetTerrainFrame.indexHash == 0x2f38beef8e4a62fb &&
                    resetTerrainFrame.coverageHash == 0x65fa674ae4afefec &&
                    resetTerrainFrame.reflectedTriangles == 0 &&
                    resetTerrainFrame.primaryMode5Triangles == 578 &&
                    resetTerrainFrame.primaryMode26Triangles == 0 &&
                    resetTerrainFrame.terrainPixelWrites == 37174,
                    "managed reset terrain-frame vector");
                Require(
                    resetTerrainFrame.indexHash == repeatedTerrainFrame.indexHash &&
                    resetTerrainFrame.coverageHash ==
                        repeatedTerrainFrame.coverageHash &&
                    resetTerrainFrame.indexHash ==
                        independentTerrainFrame.indexHash &&
                    resetTerrainFrame.coverageHash ==
                        independentTerrainFrame.coverageHash &&
                    ArrayEqual(resetIndices, repeatedIndices) &&
                    ArrayEqual(resetCoverage, repeatedCoverage) &&
                    ArrayEqual(resetIndices, independentIndices) &&
                    ArrayEqual(resetCoverage, independentCoverage),
                    "same-tick and independent-session terrain frames");
                FlipTerrainFrameRows(resetIndices, uploadIndices);
                FlipTerrainFrameRows(uploadIndices, restoredIndices);
                Require(
                    ArrayEqual(resetIndices, restoredIndices),
                    "terrain frame row flip is an exact involution");
            }
            Require(
                resetPresentation.generation == reset.generation &&
                resetPresentation.tick == reset.tick &&
                resetPresentation.cameraX == reset.x &&
                resetPresentation.cameraY == reset.y &&
                resetPresentation.cameraZ == reset.z + 128,
                "reset restores the initial stored camera row");
            Require(
                resetRenderCamera.slopeTargetX ==
                    independentResetRenderCamera.slopeTargetX &&
                resetRenderCamera.slopeTargetY ==
                    independentResetRenderCamera.slopeTargetY &&
                resetRenderCamera.renderOffsetX ==
                    independentResetRenderCamera.renderOffsetX &&
                resetRenderCamera.renderOffsetY ==
                    independentResetRenderCamera.renderOffsetY,
                "reset restarts DrawWorld terrain-slope accumulators");
            status = first.CopyEvents(
                oldGeneration,
                0,
                eventBuffer,
                out _,
                out _);
            Require(status == NativeSim.GenerationMismatch, "old generation invalidated");
            var replay = reset;
            for (var tick = 0; tick < 8; tick++)
                replay = first.Step(0, 0, 1);
            Require(
                replay.stateHash == 0x956246bd7f270164 &&
                replay.x == 19840 &&
                replay.y == 56480 &&
                replay.z == 5040 &&
                replay.phase == 161,
                "reset and replay return to the eight-forward golden");

            Debug.Log(
                "MC_SELF_TEST PASS abi=2 layouts=72/32/28/120/40/52/64/72/56/64/40/96/32/40/56/72/88 " +
                "world=1/0 planes=6x65536 objectives=8 entities=1000/177 " +
                $"hash={0x956246bd7f270164:x16} position=19840,56480,5040 " +
                "presentation=pre-update/z+128/look-tail/slope-xy/reset/terrain-grid840 " +
                $"terrainFrame={(ownerAssetCache != null ? $"{resetTerrainFrame.indexHash:x16}/{resetTerrainFrame.coverageHash:x16}/rowFlip" : "none")} " +
                "independent=true sameTickFrame=true resetReplay=true " +
                "generationInvalidation=true " +
                "fireEvent=true wrap=true " +
                $"ownerHud={(ownerAssetCache != null ? "48/mixed-authority" : "none")} " +
                $"ownerAudio={(ownerAssetCache != null ? "Night822/C2GAME1/refs32/shared10voices/positionalStereo/sourceTurnStart/music90Hz" : "none")}");
            DisposeSession();
            Application.Quit(0);
        }

        private static bool ArrayEqual(byte[] left, byte[] right)
        {
            if (left == null || right == null || left.Length != right.Length) return false;
            for (var index = 0; index < left.Length; index++)
            {
                if (left[index] != right[index]) return false;
            }
            return true;
        }

        private int ReadLiveMovementFlags()
        {
            var flags = 0;
            // remc2 samples held arrow state once per outer game turn. This
            // method is called exactly once by the authoritative 30 Hz step.
            if (Input.GetKey(KeyCode.W) || Input.GetKey(KeyCode.UpArrow)) flags |= 0x01;
            if (Input.GetKey(KeyCode.S) || Input.GetKey(KeyCode.DownArrow)) flags |= 0x02;
            if (Input.GetKey(KeyCode.A)) flags |= 0x04;
            if (Input.GetKey(KeyCode.D)) flags |= 0x08;
            if (fireRequested)
            {
                flags |= 0x10;
                fireRequested = false;
            }
            if (rightSpellRequested)
            {
                flags |= 0x20;
                rightSpellRequested = false;
            }
            if (selectedSpellRequested)
            {
                flags |= 0x40;
                selectedSpellRequested = false;
            }
            return flags;
        }

        private int CurrentSpeedSpellTicks()
        {
            foreach (var actor in missionActors)
            {
                if (actor.kind == 4 && actor.collected != 0)
                {
                    return actor.stateValue;
                }
            }
            return 0;
        }

        private static bool RejectsAudioCapturePath(string path)
        {
            try
            {
                AudioQualificationCapture.ResolveQualifiedCapturePath(path);
                return false;
            }
            catch (InvalidDataException)
            {
                return true;
            }
        }

        private void DrawOwnerHud()
        {
            Require(
                hudDescriptor != null &&
                minimapTexture != null &&
                ownerObjectiveTextures.Count == 6 &&
                ownerAuthorityBadgeTexture != null,
                "owner HUD is fully materialized before draw");
            var scale = Mathf.Min(
                Screen.width / 640f,
                Screen.height / 480f);
            var offsetX = (Screen.width - 640f * scale) * 0.5f;
            var offsetY = (Screen.height - 480f * scale) * 0.5f;
            var priorMatrix = GUI.matrix;
            var priorColour = GUI.color;
            GUI.matrix = Matrix4x4.TRS(
                new Vector3(offsetX, offsetY, 0f),
                Quaternion.identity,
                new Vector3(scale, scale, 1f));
            GUI.color = Color.white;
            try
            {
                GUI.DrawTexture(
                    new Rect(
                        hudDescriptor.minimap.x,
                        hudDescriptor.minimap.y,
                        hudDescriptor.minimap.width,
                        hudDescriptor.minimap.height),
                    minimapTexture,
                    ScaleMode.StretchToFill,
                    true);
                // DrawGameFrame_2BE30 draws both hands before the top bar.
                DrawOwnerSpellSlot(
                    hudDescriptor.spells.slots[0].x,
                    hudDescriptor.spells.slots[0].y,
                    level1Hud.left);
                DrawOwnerSpellSlot(
                    hudDescriptor.spells.slots[1].x,
                    hudDescriptor.spells.slots[1].y,
                    level1Hud.right);
                DrawOwnerTopBar();
                if (
                    mission.objectiveTextboxTicks > 0 &&
                    ownerObjectiveRows.TryGetValue(mission.textSelector, out var objectiveRow) &&
                    ownerObjectiveTextures.TryGetValue(
                        mission.textSelector, out var objectiveTexture))
                {
                    GUI.DrawTexture(
                        new Rect(
                            objectiveRow.reference_x,
                            objectiveRow.reference_y,
                            objectiveRow.render_width,
                            objectiveRow.render_height),
                        objectiveTexture,
                        ScaleMode.StretchToFill,
                        true);
                }
                if (spellMenuOpen)
                {
                    DrawOwnerSpellMenu();
                }
            }
            finally
            {
                GUI.matrix = priorMatrix;
                GUI.color = priorColour;
            }
        }

        private void DrawOwnerSpellMenu()
        {
            var menu = hudDescriptor.spells.menu;
            DrawOwnerHudBitmap(menu.edge_selector, 0, menu.reference_y);
            DrawOwnerHudBitmap(
                menu.edge_selector,
                640 - menu.edge_width,
                menu.reference_y);
            for (var uiIndex = 0; uiIndex < menu.rows * menu.columns; uiIndex++)
            {
                var x =
                    menu.edge_width +
                    (uiIndex % menu.columns) * menu.panel_width;
                var y =
                    menu.reference_y +
                    (uiIndex / menu.columns) * menu.panel_height;
                var enabled = SpellEnabledForUiIndex(uiIndex);
                DrawOwnerHudBitmap(
                    enabled
                        ? menu.panel_selector
                        : menu.disabled_panel_selector,
                    x,
                    y);
                if (enabled)
                {
                    DrawOwnerHudBitmap(
                        menu.small_icon_base_selector + uiIndex,
                        x,
                        y);
                    if (level1SpellMenu.leftFamily == uiIndex)
                    {
                        DrawOwnerHudBitmap(
                            menu.left_assignment_corner_selector,
                            x,
                            y);
                    }
                    if (level1SpellMenu.rightFamily == uiIndex)
                    {
                        DrawOwnerHudBitmap(
                            menu.right_assignment_corner_selector,
                            x + menu.panel_width - 12,
                            y);
                    }
                }
                if (spellMenuHoverUiIndex == uiIndex)
                {
                    DrawOwnerHudBitmap(menu.selection_frame_selector, x, y);
                }
            }
            if (spellMenuSelectedUiIndex >= 0)
            {
                DrawOwnerSpellTierMenu();
            }
        }

        private void DrawOwnerSpellTierMenu()
        {
            var menu = hudDescriptor.spells.menu;
            var tierPanel = hudBitmapDescriptors[menu.tier_locked_panel_selector];
            var startX = SpellTierMenuStartX(
                spellMenuSelectedUiIndex,
                menu.columns,
                menu.edge_width,
                menu.panel_width,
                menu.tier_count,
                tierPanel.runtime_width);
            var y = menu.reference_y - tierPanel.runtime_height;
            var unlockedTier = SpellUnlockedTier(spellMenuSelectedUiIndex);
            for (var tier = 0; tier < menu.tier_count; tier++)
            {
                var x = startX + tier * tierPanel.runtime_width;
                if (tier > unlockedTier)
                {
                    DrawOwnerHudBitmap(menu.tier_locked_panel_selector, x, y);
                    continue;
                }
                DrawOwnerHudBitmap(menu.tier_panel_selector, x, y);
                DrawOwnerHudBitmap(menu.tier_number_base_selector + tier, x + 6, y + 10);
                DrawOwnerHudBitmap(
                    menu.tier_icon_base_selector +
                    3 * spellMenuSelectedUiIndex +
                    tier,
                    x + 18,
                    y + 6);
                if (spellMenuHoverTier == tier)
                {
                    DrawOwnerHudBitmap(menu.tier_selection_frame_selector, x, y);
                }
            }
        }

        private void DrawOwnerTopBar()
        {
            DrawOwnerHudBitmap(40, 2, 2);

            const int castlePanelX = 126;
            if (level1Castle.active != 0 && level1Castle.level > 0)
            {
                DrawOwnerHudBitmap(
                    level1Hud.castleHitFrameTicks > 0 &&
                    level1Hud.colorPhase1 != 0
                        ? 55
                        : 41,
                    castlePanelX,
                    2);
                var castleIcon = 43 + level1Castle.level;
                Require(
                    castleIcon >= 44 && castleIcon <= 50,
                    $"source castle HUD icon selector {castleIcon}");
                DrawOwnerHudBitmap(castleIcon, castlePanelX + 2, 2);
                DrawOwnerHudBitmap(42, castlePanelX + 38, 2);
                DrawOwnerHudBar(
                    castlePanelX + 58,
                    10,
                    SourceHudWidth(level1Castle.life, level1Castle.maxLife),
                    10,
                    level1Hud.lifeColorIndex);
                DrawOwnerHudBar(
                    castlePanelX + 58,
                    28,
                    SourceHudWidth(
                        level1Castle.manaCapacity,
                        level1Hud.globalManaDenominator),
                    10,
                    level1Hud.manaMaxColorIndex);
                DrawOwnerHudBar(
                    castlePanelX + 58,
                    28,
                    SourceHudWidth(
                        level1Castle.mana + level1Hud.securedManaComponent,
                        level1Hud.globalManaDenominator),
                    10,
                    level1Hud.manaColorIndex);
                var goalX = castlePanelX + 58 +
                    Math.Min(64, Math.Max(0, level1Hud.goalPercent * 64 / 100));
                var goalColour = level1Hud.colorPhase2 == 0
                    ? level1Hud.manaColorIndex
                    : level1Hud.manaMaxColorIndex;
                DrawOwnerHudBar(goalX, 26, 2, 2, goalColour);
                DrawOwnerHudBar(goalX, 38, 2, 2, goalColour);
            }
            else
            {
                DrawOwnerHudBitmap(54, castlePanelX, 2);
            }

            // Level 1 has no reachable player balloon ownership row.
            DrawOwnerHudBitmap(54, 254, 2);

            const int playerPanelX = 382;
            DrawOwnerHudBitmap(
                level1Hud.playerHitFrameTicks > 0 &&
                level1Hud.colorPhase1 != 0
                    ? 55
                    : 41,
                playerPanelX,
                2);
            DrawOwnerHudBitmap(43, playerPanelX + 2, 2);
            DrawOwnerHudBitmap(42, playerPanelX + 38, 2);
            DrawOwnerHudBar(
                playerPanelX + 58,
                10,
                SourceHudWidth(mission.playerLife, mission.playerMaxLife),
                10,
                level1Hud.lifeColorIndex);
            DrawOwnerHudBar(
                playerPanelX + 58,
                28,
                SourceHudWidth(
                    mission.playerMaxMana,
                    level1Hud.globalManaDenominator),
                10,
                level1Hud.manaMaxColorIndex);
            DrawOwnerHudBar(
                playerPanelX + 58,
                28,
                SourceHudWidth(
                    mission.playerMana,
                    level1Hud.globalManaDenominator),
                10,
                level1Hud.manaColorIndex);
        }

        private void DrawOwnerSpellSlot(
            int x,
            int y,
            NativeSim.Level1HudSpellSlot slot)
        {
            if (slot.holderEntityIndex == 0) return;
            if (
                (slot.enabledFlags & 4) != 0 &&
                slot.activeValue > 0 &&
                slot.activeValue < 32 &&
                level1Hud.colorPhase1 != 0)
                return;
            Require(
                slot.frameSelector == (slot.activeValue == 0 ? 1 : 2) &&
                slot.iconSelector == 123 + slot.model &&
                slot.tier >= 0 &&
                slot.tier < 5,
                $"source spell HUD row {slot.holderEntityIndex}");
            DrawOwnerHudBitmap(slot.frameSelector, x, y);
            DrawOwnerHudBitmap(slot.iconSelector, x, y);
            var tierRow = hudDescriptor.spells.tier_rows[slot.tier];
            Require(
                ownerSpellTierTextures.TryGetValue(
                    slot.tier, out var tierTexture),
                $"materialized spell tier {slot.tier}");
            GUI.DrawTexture(
                new Rect(
                    x + 64 - tierRow.render_width - 2,
                    y,
                    tierRow.render_width,
                    tierRow.render_height),
                tierTexture,
                ScaleMode.StretchToFill,
                true);
            if (slot.maxMana <= 0) return;

            var remainder = slot.casterMana % slot.maxMana;
            DrawOwnerHudBar(
                x + 4,
                y + 36,
                (int)(56L * remainder / slot.maxMana),
                4,
                level1Hud.manaMaxColorIndex);
            var wholeCharges = slot.casterMana / slot.maxMana;
            for (var column = 0; column < 27 && wholeCharges > 0; column++)
            {
                for (var row = 0; row < 2 && wholeCharges > 0; row++)
                {
                    DrawOwnerHudBar(
                        x + 2 * (column + 2),
                        y + 2 * (row + 18),
                        2,
                        2,
                        level1Hud.manaColorIndex);
                    wholeCharges--;
                }
            }
        }

        private void DrawOwnerHudBitmap(int selector, int x, int y)
        {
            HudBitmapDescriptor bitmap = null;
            Texture2D texture = null;
            Require(
                hudBitmapDescriptors.TryGetValue(selector, out bitmap) &&
                hudBitmapTextures.TryGetValue(selector, out texture),
                $"materialized HSPRN selector {selector}");
            GUI.DrawTexture(
                new Rect(x, y, bitmap.runtime_width, bitmap.runtime_height),
                texture,
                ScaleMode.StretchToFill,
                true);
        }

        private static int SourceHudWidth(int value, int denominator)
        {
            if (denominator <= 0) return 0;
            return Math.Min(64, (int)((long)value * 64 / denominator));
        }

        private void DrawOwnerHudBar(
            int x,
            int y,
            int width,
            int height,
            byte paletteIndex)
        {
            width = Math.Min(width, 64);
            if (width < 2) return;
            var prior = GUI.color;
            GUI.color = PaletteColour(ownerHudPaletteRgb8, paletteIndex);
            GUI.DrawTexture(
                new Rect(x, y, width, height),
                Texture2D.whiteTexture,
                ScaleMode.StretchToFill,
                true);
            GUI.color = prior;
        }

        private void OnGUI()
        {
            hudStyle ??= new GUIStyle(GUI.skin.label)
            {
                fontSize = Mathf.Max(14, Screen.height / 58),
                normal = { textColor = new Color(0.95f, 0.88f, 0.68f) }
            };
            titleStyle ??= new GUIStyle(hudStyle)
            {
                fontSize = Mathf.Max(21, Screen.height / 38),
                fontStyle = FontStyle.Bold
            };
            if (fatalError != null)
            {
                GUI.Label(new Rect(20, 20, Screen.width - 40, Screen.height - 40),
                    "MC LEVEL 1 VIEWER FAILED\n" + fatalError, titleStyle);
                return;
            }
            if (session == null) return;
            if (entityGallery)
            {
                GUI.Label(new Rect(24, 18, Screen.width - 48, 40),
                    "LEVEL 1 OWNER-SPRITE QUALIFICATION GALLERY · PALN / TMAP BASE FRAMES",
                    titleStyle);
                foreach (var item in entityGalleryItems)
                {
                    var screen = flightCamera.WorldToScreenPoint(item.Visual.position);
                    if (screen.z <= 0f) continue;
                    GUI.Label(
                        new Rect(
                            screen.x - 80f,
                            Screen.height - screen.y +
                                Mathf.Abs(item.Visual.lossyScale.y) * 18f + 8f,
                            160f,
                            24f),
                        item.Label,
                        hudStyle);
                }
                GUI.Label(
                    new Rect(24, Screen.height - 38, Screen.width - 48, 28),
                    "TEST-ONLY VIEW · 1 source unit = 1/64 Unity unit · " +
                    "camera-facing · no simulation mutation · no Unity physics",
                    hudStyle);
                return;
            }

            if (qualifiedMode)
            {
                DrawOwnerHud();
                return;
            }

            GUI.Label(new Rect(24, 18, 840, 40),
                ownerAssetCache == null
                    ? "MC2 LEVEL 1 — EXACT DATA / DIAGNOSTIC PRESENTATION"
                    : "MC2 LEVEL 1 — INDEXED NIGHT MATERIAL BRIDGE", titleStyle);
            GUI.Label(
                new Rect(26, 60, 760, 250),
                $"speed {snapshot.actualSpeed,4} / {snapshot.targetSpeed,4}  " +
                $"altitude {snapshot.z,6}  yaw {snapshot.yaw,4}\n" +
                $"raw {snapshot.x},{snapshot.y}  unwrapped {unwrappedSourceX},{unwrappedSourceY}  " +
                $"render offset {renderCameraState.renderOffsetX}," +
                $"{renderCameraState.renderOffsetY}  " +
                $"tick {snapshot.tick} phase {snapshot.phase}\n" +
                $"state {snapshot.stateHash:x16}  conformance {snapshot.conformanceHash:x16}\n" +
                $"events head/cursor {snapshot.eventHead}/{eventCursor}  " +
                $"fire chain latched/submitted/kind4/visual " +
                $"{fireInputLatchedCount}/{fireFlagsSubmittedCount}/" +
                $"{fireEventsObserved}/{fireVisualSpawnedCount}  movement {movementResolution}\n" +
                $"objective textbox ticks {snapshot.objectiveTextboxTicks} " +
                $"(FrozenObserved) · catalog {objectives.Length} frozen rows\n" +
                $"entities 1000 / active registry {entityRegistry.Count} · " +
                $"entity153 SpellActivationLive (holder dynamics unavailable)\n" +
                (ownerAssetCache == null
                    ? "DIAGNOSTIC TERRAIN / MARKERS / FIRE VISUAL — NOT QUALIFIED AS MC2 ART\n"
                    : "BL32N TERRAIN + 64 TMAP ACTORS + PALN · ACTOR TMAP ANIMATION BASE-FRAME ONLY\n") +
                "W/S speed · A/D strafe · persistent mouse or I/J/K/L steer · C centre · Space fire",
                hudStyle);

            if (minimapTexture != null)
            {
                var size = Mathf.Min(256f, Screen.height * 0.29f);
                var rect = new Rect(Screen.width - size - 22f, 22f, size, size);
                GUI.DrawTexture(rect, minimapTexture, ScaleMode.StretchToFill, false);
                GUI.Label(new Rect(rect.x, rect.yMax + 2, size, 26),
                    ownerAssetCache == null
                        ? "EXACT-DATA DIAGNOSTIC MINIMAP"
                        : "INDEXED NIGHT MATERIAL OVERVIEW", hudStyle);
                var playerX = rect.x + snapshot.x / 65536f * rect.width;
                var playerY = rect.yMax - snapshot.y / 65536f * rect.height;
                GUI.color = Color.yellow;
                GUI.DrawTexture(new Rect(playerX - 4, playerY - 4, 8, 8), Texture2D.whiteTexture);
                GUI.color = Color.white;
            }

            var eventY = Screen.height - 100f;
            foreach (var value in recentEventDiagnostics)
            {
                GUI.Label(new Rect(24, eventY, 700, 22), "event " + value, hudStyle);
                eventY += 20f;
            }
            DrawEntityLabels();
        }

        private void DrawEntityLabels()
        {
            if (flightCamera == null) return;
            if (!debugEntities)
            {
                var nearest = new Dictionary<string, EntityMarker>();
                foreach (var marker in entityRegistry)
                {
                    if (!marker.Visible || marker.Renderer == null ||
                        !marker.Renderer.enabled || marker.Family == null) continue;
                    if (!nearest.TryGetValue(marker.Family, out var prior) ||
                        Vector3.SqrMagnitude(marker.Node.position - flightCamera.transform.position) <
                        Vector3.SqrMagnitude(prior.Node.position - flightCamera.transform.position))
                    {
                        nearest[marker.Family] = marker;
                    }
                }
                foreach (var marker in nearest.Values) DrawEntityLabel(marker);
                return;
            }
            foreach (var marker in entityRegistry)
            {
                if (marker.Visible && marker.Renderer != null && marker.Renderer.enabled)
                    DrawEntityLabel(marker);
            }
        }

        private void DrawEntityLabel(EntityMarker marker)
        {
            var screen = flightCamera.WorldToScreenPoint(marker.Node.position);
            if (screen.z <= 0f || screen.z > 280f) return;
            GUI.Label(
                new Rect(screen.x + 4, Screen.height - screen.y, 360, 22),
                marker.Label,
                hudStyle);
        }

        private static string SemanticFamily(NativeSim.EntitySnapshot entity)
        {
            if (entity.classId == 3 && entity.model == 0) return "player/carpet";
            if (entity.classId == 2 && entity.model == 2) return "dolmen";
            if (entity.classId == 5 && entity.model == 1) return "goat";
            if (entity.classId == 5 && entity.model == 13) return "villager";
            return null;
        }

        private static string AuthorityLabel(NativeSim.EntitySnapshot entity)
        {
            if (entity.authority == NativeSim.PlayerFlightLive) return "PlayerFlightLive";
            if (entity.authority == NativeSim.SpellActivationLive)
                return "SpellActivationLive; holder fields unavailable";
            if (entity.authority == NativeSim.GoatLive) return "GoatLive";
            if (entity.authority == NativeSim.VillagerLive) return "VillagerLive";
            return "FrozenObserved";
        }

        private static PrimitiveType MarkerPrimitive(string family)
        {
            if (family == "goat" || family == "villager") return PrimitiveType.Capsule;
            if (family == "player/carpet") return PrimitiveType.Sphere;
            return PrimitiveType.Cube;
        }

        private static Vector3 MarkerScale(string family)
        {
            if (family == "dolmen") return new Vector3(1.2f, 4f, 1.2f);
            if (family == "goat") return new Vector3(0.8f, 0.7f, 1.2f);
            if (family == "villager") return new Vector3(0.8f, 1.4f, 0.8f);
            return Vector3.one * 0.65f;
        }

        private static Color MarkerColor(string family, byte authority)
        {
            if (authority == NativeSim.PlayerFlightLive) return Color.yellow;
            if (authority == NativeSim.SpellActivationLive) return new Color(1f, 0.2f, 0.8f);
            if (family == "dolmen") return new Color(0.55f, 0.60f, 0.68f);
            if (family == "goat") return new Color(0.92f, 0.90f, 0.72f);
            if (family == "villager") return new Color(0.25f, 0.85f, 0.45f);
            return new Color(0.95f, 0.25f, 0.15f);
        }

        private static GameObject DiagnosticPrimitive(
            PrimitiveType primitive,
            string name,
            Color color,
            Transform parent)
        {
            var value = GameObject.CreatePrimitive(primitive);
            value.name = name;
            if (parent != null) value.transform.SetParent(parent, false);
            var collider = value.GetComponent<Collider>();
            if (collider != null) Destroy(collider);
            value.GetComponent<Renderer>().material = CreateMaterial(color, 0.25f, 0.05f);
            return value;
        }

        private static void SetLayerRecursively(GameObject value, int layer)
        {
            value.layer = layer;
            foreach (Transform child in value.transform)
                SetLayerRecursively(child.gameObject, layer);
        }

        private static Material CreateMaterial(Color color, float smoothness, float metallic)
        {
            var material = new Material(Resources.Load<Shader>("Surface")) { color = color };
            material.SetFloat("_Glossiness", smoothness);
            material.SetFloat("_Metallic", metallic);
            return material;
        }

        private static Vector3 SourceToUnity(long x, long y, int z)
        {
            return new Vector3(
                x / SourceUnitsPerUnityUnit,
                z / SourceUnitsPerUnityUnit,
                y / SourceUnitsPerUnityUnit);
        }

        private static int ModularDelta(ushort current, ushort previous)
        {
            return ((current - previous + 32768) & 0xffff) - 32768;
        }

        private void FinishLevel1AutoplayIfTerminal()
        {
            const int maximumTicks = 120_000;
            var complete = mission.exitSequenceComplete != 0 &&
                           mission.exitResultCode == 10;
            var failed = mission.playerDead != 0 || simTick >= maximumTicks;
            if (!complete && !failed) return;

            runtimeExitRequested = true;
            var passed = complete && !failed;
            Debug.Log(
                $"MC_LEVEL1_AUTOPLAY {(passed ? "PASS" : "FAIL")} " +
                $"tick={simTick} stage={mission.currentStage} " +
                $"exitComplete={mission.exitSequenceComplete} " +
                $"result={mission.exitResultCode} playerDead={mission.playerDead} " +
                $"life={mission.playerLife} mana={mission.playerMana} " +
                $"phase={level1AutoplayPhase} postgame={level1AutoplayPostgame}");
            if (level1AutoplayQuit || Application.isBatchMode)
            {
                DisposeSession();
                Application.Quit(passed ? 0 : 4);
            }
            else
            {
                level1Autoplay = false;
                ReleaseMouse();
            }
        }

        private static int SignedAngle11(short value)
        {
            var wrapped = value & 0x07ff;
            return wrapped >= 1024 ? wrapped - 2048 : wrapped;
        }

        private static void BuildAutoplayCommand(
            int tick,
            out sbyte roll,
            out sbyte pitch,
            out int flags)
        {
            flags = tick < 360 ? 0x01 : 0;
            roll = 0;
            pitch = 0;
            if (tick < 145) roll = 38;
            else if (tick is >= 180 and < 250) { roll = -32; pitch = -22; }
            else if (tick is >= 250 and < 325) pitch = 24;
            if (tick is >= 325 and < 365) flags |= 0x08;
            if (tick == 160 || tick == 300) flags |= 0x10;
        }

        private static sbyte ClampToSByte(float value)
        {
            return (sbyte)Mathf.Clamp(Mathf.RoundToInt(value), sbyte.MinValue, sbyte.MaxValue);
        }

        private static string ResolutionName(byte value)
        {
            return value switch
            {
                0 => "accepted",
                1 => "redirected",
                2 => "rejected",
                3 => "rejected-and-stopped",
                4 => "recovered",
                5 => "skipped",
                _ => "unknown"
            };
        }

        private static bool HasArgument(string[] arguments, string name)
        {
            return Array.Exists(arguments, argument => argument == name);
        }

        private static string ArgumentValue(string[] arguments, string name)
        {
            for (var index = 0; index + 1 < arguments.Length; index++)
            {
                if (arguments[index] == name) return arguments[index + 1];
            }
            return null;
        }

        private static NativeSession OpenSessionWithAssetCache(
            string package,
            string closure,
            string assetCache)
        {
            var prior =
                Environment.GetEnvironmentVariable("MC_ASSET_CACHE_PATH");
            try
            {
                Environment.SetEnvironmentVariable(
                    "MC_ASSET_CACHE_PATH", assetCache);
                return NativeSession.Open(package, closure);
            }
            finally
            {
                Environment.SetEnvironmentVariable(
                    "MC_ASSET_CACHE_PATH", prior);
            }
        }

        private static void ResolveOwnerPaths(
            string[] arguments,
            out string package,
            out string closure)
        {
            package = ArgumentValue(arguments, "-mc-package") ??
                      Environment.GetEnvironmentVariable("MC_PACKAGE_PATH");
            closure = ArgumentValue(arguments, "-mc-closure") ??
                      Environment.GetEnvironmentVariable("MC_CLOSURE_PATH");
#if UNITY_EDITOR
            package ??= Path.GetFullPath(Path.Combine(
                Application.dataPath,
                "../../generated/level1-export-m"));
            closure ??= Path.GetFullPath(Path.Combine(
                Application.dataPath,
                "../../generated/level1-asset-closure.json"));
#endif
            if (string.IsNullOrWhiteSpace(package) || string.IsNullOrWhiteSpace(closure))
            {
                throw new InvalidOperationException(
                    "Pass -mc-package and -mc-closure, or MC_PACKAGE_PATH/MC_CLOSURE_PATH. " +
                    "Owner data is never bundled into the Unity player.");
            }
            package = Path.GetFullPath(package);
            closure = Path.GetFullPath(closure);
        }

        private static void Require(bool condition, string description)
        {
            if (!condition)
            {
                throw new InvalidOperationException("required invariant failed: " + description);
            }
        }

        private void CenterSteering()
        {
            steeringRoll = 0f;
            steeringPitch = 0f;
            appliedRoll = 0;
            appliedPitch = 0;
        }

        private void LatchSpellCastInput(bool rightHand)
        {
            // Latched until exactly one FixedUpdate consumes it.
            if (rightHand)
            {
                rightSpellRequested = true;
            }
            else
            {
                fireRequested = true;
            }
            fireInputLatchedCount++;
            Debug.Log(
                $"MC_FIRE input_latched source={(rightHand ? "right-mouse" : "mouse")} " +
                $"count={fireInputLatchedCount}");
        }

        private bool QueueSpellCycle(bool rightHand, bool backward)
        {
            if (!SpellMenuController.Cycle(
                    rightHand,
                    backward,
                    level1SpellMenu,
                    out var command))
            {
                return false;
            }
            spellMenuCommands.Enqueue(command);
            Debug.Log(
                $"MC_SPELL_CYCLE hand={(rightHand ? "right" : "left")} " +
                $"direction={(backward ? "backward" : "forward")} " +
                $"family={command.Argument1} tier={command.Argument2}");
            return true;
        }

        private SpellMenuGeometry CurrentSpellMenuGeometry()
        {
            var menu = hudDescriptor.spells.menu;
            var tierPanel = hudBitmapDescriptors[menu.tier_locked_panel_selector];
            return new SpellMenuGeometry(
                menu.rows,
                menu.columns,
                menu.reference_y,
                menu.edge_width,
                menu.panel_width,
                menu.panel_height,
                menu.tier_count,
                tierPanel.runtime_width);
        }

        private static Vector2 ScreenToSpellLogical(Vector3 mousePosition)
        {
            var scale = Mathf.Min(Screen.width / 640f, Screen.height / 480f);
            var offsetX = (Screen.width - 640f * scale) * 0.5f;
            var offsetY = (Screen.height - 480f * scale) * 0.5f;
            return new Vector2(
                (mousePosition.x - offsetX) / scale,
                (Screen.height - mousePosition.y - offsetY) / scale);
        }

        private void OpenSpellMenu(float logicalX, float logicalY)
        {
            spellMenuOpen = true;
            spellMenuController.Open(
                logicalX,
                logicalY,
                level1SpellMenu,
                CurrentSpellMenuGeometry());
            SyncSpellMenuControllerState();
            ReleaseMouse();
        }

        private void CloseSpellMenu(bool captureMouse)
        {
            spellMenuOpen = false;
            spellMenuController.Close();
            SyncSpellMenuControllerState();
            if (captureMouse)
            {
                CaptureMouse();
            }
        }

        private void ProcessSpellMenuInput(
            float logicalX,
            float logicalY,
            bool leftDown,
            bool leftHeld,
            bool leftUp,
            bool rightDown,
            bool rightHeld,
            bool rightUp,
            bool configureEligibility = false)
        {
            if (!spellMenuOpen)
            {
                return;
            }
            var geometry = CurrentSpellMenuGeometry();
            SpellMenuCommand command;
            var emitted = false;
            if (configureEligibility && leftDown)
            {
                emitted = spellMenuController.ConfigureEligibility(
                    0,
                    logicalX,
                    logicalY,
                    level1SpellMenu,
                    geometry,
                    out command);
            }
            else if (configureEligibility && rightDown)
            {
                emitted = spellMenuController.ConfigureEligibility(
                    1,
                    logicalX,
                    logicalY,
                    level1SpellMenu,
                    geometry,
                    out command);
            }
            else if (leftDown)
            {
                emitted = spellMenuController.Press(
                    0,
                    logicalX,
                    logicalY,
                    level1SpellMenu,
                    geometry,
                    out command);
            }
            else if (rightDown)
            {
                emitted = spellMenuController.Press(
                    1,
                    logicalX,
                    logicalY,
                    level1SpellMenu,
                    geometry,
                    out command);
            }
            else if (leftUp)
            {
                emitted = spellMenuController.Release(0, out command);
            }
            else if (rightUp)
            {
                emitted = spellMenuController.Release(1, out command);
            }
            else if (leftHeld)
            {
                emitted = spellMenuController.Hold(
                    0,
                    logicalX,
                    level1SpellMenu,
                    geometry,
                    out command);
            }
            else if (rightHeld)
            {
                emitted = spellMenuController.Hold(
                    1,
                    logicalX,
                    level1SpellMenu,
                    geometry,
                    out command);
            }
            else
            {
                spellMenuController.UpdateFamilyHover(
                    logicalX,
                    logicalY,
                    level1SpellMenu,
                    geometry);
                command = default;
            }
            if (emitted)
            {
                spellMenuCommands.Enqueue(command);
                Debug.Log(
                    $"MC_SPELL_MENU opcode={command.Opcode} " +
                    $"argument1={command.Argument1} argument2={command.Argument2}");
            }
            SyncSpellMenuControllerState();
        }

        private void SyncSpellMenuControllerState()
        {
            spellMenuHoverUiIndex = spellMenuController.HoverUiIndex;
            spellMenuSelectedUiIndex = spellMenuController.SelectedUiIndex;
            spellMenuHoverTier = spellMenuController.HoverTier;
        }

        private bool SpellEnabledForUiIndex(int uiIndex)
        {
            return uiIndex >= 0 &&
                   level1SpellMenu.familyEnabled != null &&
                   uiIndex < level1SpellMenu.familyEnabled.Length &&
                   level1SpellMenu.familyEnabled[uiIndex] != 0;
        }

        private int SpellUnlockedTier(int uiIndex)
        {
            if (!SpellEnabledForUiIndex(uiIndex))
            {
                return 0;
            }
            return Mathf.Clamp(
                level1SpellMenu.familyUnlockedTier[uiIndex],
                0,
                hudDescriptor.spells.menu.tier_count - 1);
        }

        internal static int SpellTierMenuStartX(
            int uiIndex,
            int columns,
            int edgeWidth,
            int familyPanelWidth,
            int tierCount,
            int tierPanelWidth)
        {
            var totalWidth = tierCount * tierPanelWidth;
            var familyCentre =
                edgeWidth +
                (uiIndex % columns) * familyPanelWidth +
                familyPanelWidth / 2;
            return Mathf.Clamp(
                familyCentre - totalWidth / 2,
                0,
                640 - totalWidth);
        }

        internal static int SpellTierFromLogicalX(
            float logicalX,
            int startX,
            int tierPanelWidth,
            int unlockedTier)
        {
            var tier = Mathf.FloorToInt((logicalX - startX) / tierPanelWidth);
            return Mathf.Clamp(tier, 0, unlockedTier);
        }

        private static void CaptureMouse()
        {
            Cursor.lockState = CursorLockMode.Locked;
            Cursor.visible = false;
        }

        private static void ReleaseMouse()
        {
            Cursor.lockState = CursorLockMode.None;
            Cursor.visible = true;
        }

        private void OnDestroy()
        {
            DisposeSession();
        }

        private void OnApplicationQuit()
        {
            DisposeSession();
        }

        private void DisposeSession()
        {
            if (disposed) return;
            disposed = true;
            if (flightCamera != null && terrainFrameCommandBuffer != null)
            {
                flightCamera.RemoveCommandBuffer(
                    CameraEvent.BeforeForwardOpaque,
                    terrainFrameCommandBuffer);
            }
            terrainFrameCommandBuffer?.Release();
            terrainFrameCommandBuffer = null;
            terrainFrameCompositeLive = false;
            if (terrainFrameCompositeMaterial != null)
                Destroy(terrainFrameCompositeMaterial);
            if (terrainDepthOnlyMaterial != null)
                Destroy(terrainDepthOnlyMaterial);
            if (terrainFrameIndexTexture != null)
                Destroy(terrainFrameIndexTexture);
            if (terrainFrameCoverageTexture != null)
                Destroy(terrainFrameCoverageTexture);
            terrainFrameCompositeMaterial = null;
            terrainDepthOnlyMaterial = null;
            terrainFrameIndexTexture = null;
            terrainFrameCoverageTexture = null;
            audioCapture?.Finish();
            playthroughRecorder?.Finish(fatalError);
            playthroughRecorder?.Dispose();
            playthroughRecorder = null;
            ownerAudio?.Dispose();
            ownerAudio = null;
            session?.Dispose();
            session = null;
        }

        private sealed class EntityMarker
        {
            internal ushort Index;
            internal Transform Node;
            internal Renderer Renderer;
            internal string Label;
            internal string Family;
            internal bool Visible;
            internal EntityPresentationGroup Group;
            internal Transform SpriteVisual;
            internal bool OwnerSprite;
            internal bool QualifiedFrozenActor;
        }

        private sealed class MissionMarker
        {
            internal ulong Identity;
            internal Transform Node;
            internal Transform Visual;
            internal Renderer Renderer;
            internal int Selector;
        }

        private sealed class EntityGalleryItem
        {
            internal string Label;
            internal Transform Visual;
            internal int Selector;
            internal bool Mirrored;
        }
    }

    public sealed class DiagnosticFireVisual : MonoBehaviour
    {
        public Vector3 Velocity { get; set; }
        private float lifetime = 4f;

        private void Update()
        {
            transform.position += Velocity * Time.deltaTime;
            transform.localScale *= Mathf.Exp(0.18f * Time.deltaTime);
            lifetime -= Time.deltaTime;
            if (lifetime <= 0f) Destroy(gameObject);
        }
    }
}
