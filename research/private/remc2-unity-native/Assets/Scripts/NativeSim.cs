using System;
using System.Runtime.InteropServices;

namespace MagicCarpet.Native
{
    internal static class NativeSim
    {
        internal const uint AbiVersion = 2;
        internal const int Ok = 0;
        internal const int Bounds = 10;
        internal const int GenerationMismatch = 12;
        internal const uint TerrainTypes = 0;
        internal const uint TerrainHeight = 1;
        internal const uint TerrainShade = 2;
        internal const uint TerrainAngle = 3;
        internal const uint TerrainSecondary = 4;
        internal const uint TerrainEntityIndex = 5;
        internal const uint WorldPackageHash = 1;
        internal const uint WorldClosureHash = 2;
        internal const uint EventPrepareSound = 1;
        internal const uint EventEndLoop = 2;
        internal const uint EventUpdateMusic = 3;
        internal const uint EventSpellActivation = 4;
        internal const uint EventStartMusic = 6;
        internal const uint EventGoatSoundRequest = 7;
        internal const uint EventGoatManaConverted = 8;
        internal const uint EventGoatExtraManaSphereRequest = 9;
        internal const uint EventGoatRemovalQueued = 10;
        internal const uint EventGoatReclaimed = 11;
        internal const uint EventGoatEngagementBoundary = 12;
        internal const uint EventLevel1ObjectiveAdvanced = 13;
        internal const uint EventLevel1SpellCollected = 14;
        internal const uint EventLevel1FireballSpawned = 15;
        internal const uint EventLevel1FireballImpacted = 16;
        internal const uint EventLevel1ActorDamaged = 17;
        internal const uint EventLevel1ActorKilled = 18;
        internal const uint EventLevel1CastleBuilt = 19;
        internal const uint EventLevel1CastleManaChanged = 20;
        internal const uint EventLevel1PlayerDamaged = 21;
        internal const uint EventLevel1PlayerDied = 22;
        internal const uint EventLevel1Completed = 23;
        internal const uint EventLevel1ArcherArrowSpawned = 24;
        internal const uint EventLevel1ArcherArrowSound = 25;
        internal const uint EventLevel1FireflyFireballSpawned = 26;
        internal const uint EventLevel1FireflyChargeSound = 27;
        internal const uint EventLevel1CastleProjectileSpawned = 28;
        internal const uint EventLevel1CastlePlacementRejected = 29;
        internal const uint EventLevel1CastleConstructionSound = 30;
        internal const uint EventLevel1PossessionSpawned = 31;
        internal const uint EventLevel1ManaSphereSpawned = 32;
        internal const uint EventLevel1ManaSphereClaimed = 33;
        internal const uint EventLevel1ManaSphereCollected = 34;
        internal const uint EventLevel1ExitSequenceStarted = 35;
        internal const uint EventLevel1ExitSequenceCompleted = 36;
        internal const uint EventLevel1WaterSplashed = 37;
        internal const byte AudioActionStart = 1;
        internal const byte AudioActionRestart = 2;
        internal const byte AudioActionSuppressExisting = 3;
        internal const byte AudioActionSuppressNoVoice = 4;
        internal const byte AudioActionSuppressDistanceOrVolume = 5;
        internal const byte AudioActionSuppressPendingReplaced = 6;
        internal const byte AudioActionUpdateExisting = 7;
        internal const byte AudioActionEndLoop = 8;
        internal const byte AudioActionSuppressNoExisting = 9;
        internal const ushort AudioVoiceNone = ushort.MaxValue;
        internal const byte AudioEntitySoundRestartOrStart = 1;
        internal const byte AudioPlayIfNotPlaying = 2;
        internal const byte AudioEntitySoundIfNotPlaying = 3;
        internal const byte FrozenObserved = 1;
        internal const byte PlayerFlightLive = 2;
        internal const byte SpellActivationLive = 3;
        internal const byte GoatLive = 4;
        internal const byte SelectorFrameLive = 5;
        internal const byte Level1MissionLive = 6;
        internal const byte SessionTerrainLive = 7;
        internal const byte Level1ExitPaletteNone = 0;
        internal const byte Level1ExitPaletteEntryFlash = 1;
        internal const byte Level1ExitPaletteBlack = 2;
        internal const byte Level1PaletteHit = 3;
        internal const byte Level1PaletteSpeed = 4;
        internal const byte Level1PaletteObjective = 5;
        internal const int Level1MinimapMaxEntityCommands = 1152;
        internal const byte Level1MinimapCommandDot = 1;
        internal const byte Level1MinimapCommandSprite = 2;
        internal const byte Level1MinimapCommandBlendPixel = 3;
        internal const byte Level1MinimapAnchorTopLeft = 1;
        internal const byte Level1MinimapAnchorBottomLeft = 2;
        internal const byte Level1MinimapAnchorBottomCenter = 3;
        internal const byte Level1ExitFlagPaletteVisualCurrent = 0x01;
        internal const byte Level1ExitFlagFovPostActionNextHistory = 0x02;
        internal const byte Level1ExitFlagEndSequence = 0x04;
        internal const byte Level1ExitKnownFlags = 0x07;
        internal const byte Level1CurrentFidelityGaps = 0;
        internal const uint SelectorFrameAnimated = 0x01;
        internal const uint SelectorFrameGroupedUpdate = 0x02;
        internal const uint SelectorFrameUsedByPrecedingDraw = 0x04;
        internal const uint SelectorFramePixelsMutable = 0x08;
        internal const byte PresentationCameraExactInputs = 0x01;
        internal const byte PresentationCameraEyeZPlus128 = 0x02;
        internal const byte RenderCameraTerrainSlopeXY = 0x01;
        internal const uint TerrainFrameIsolatedSkyTerrain = 0x01;
        internal const uint TerrainFrameSkySeeded = 0x02;
        internal const uint TerrainFrameHasCoverage = 0x04;
        internal const uint TerrainFrameNonTerrainOmitted = 0x08;
        internal const uint TerrainFrameGoatsComposited = 0x10;
        internal const uint TerrainFrameArchersComposited = 0x20;
        internal const uint TerrainFrameFirefliesComposited = 0x40;
        internal const uint TerrainFrameCombatComposited = 0x80;
        internal const uint TerrainFrameObjectivePropsComposited = 0x100;
        internal const uint TerrainFrameEconomyComposited = 0x200;
        internal const uint TerrainFrameReflectedSpritesComposited = 0x400;
        internal const uint TerrainFrameSpriteShadowsComposited = 0x800;
        internal const uint TerrainFrameFrozenActorsComposited = 0x1000;
        internal const int TerrainFrameWidth = 320;
        internal const int TerrainFrameHeight = 200;
        internal const int TerrainFramePixels = TerrainFrameWidth * TerrainFrameHeight;
        internal const int ReflectionEyeZLimit = 4_096;
        internal const int NoRequestedSpell = -1;

        private const string Library = "sim_capi";

        [StructLayout(LayoutKind.Sequential)]
        internal struct WorldInfo
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ushort externalLevel;
            internal ushort internalLevel;
            internal byte mapType;
            internal byte profile;
            internal ushort reserved0;
            internal uint entityCount;
            internal uint activeEntityCount;
            internal uint playerCount;
            internal uint assetRefCount;
            internal ushort terrainWidth;
            internal ushort terrainHeight;
            internal ulong resetStateHash;
            internal ulong resetConformanceHash;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct Command
        {
            internal uint abiVersion;
            internal uint structSize;
            internal sbyte opcode;
            internal sbyte argument1;
            internal sbyte argument2;
            internal sbyte roll;
            internal sbyte pitch;
            internal sbyte flags;
            internal short requestedSpell;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;

            internal static Command Create(
                sbyte roll,
                sbyte pitch,
                sbyte flags,
                sbyte opcode = 0,
                sbyte argument1 = 0,
                sbyte argument2 = 0,
                short requestedSpell = NoRequestedSpell)
            {
                return new Command
                {
                    abiVersion = AbiVersion,
                    structSize = 32,
                    opcode = opcode,
                    argument1 = argument1,
                    argument2 = argument2,
                    roll = roll,
                    pitch = pitch,
                    flags = flags,
                    requestedSpell = requestedSpell,
                    reserved = new uint[4]
                };
            }
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CameraInput
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ushort lookYaw;
            internal ushort lookPitch;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;

            internal static CameraInput Create(ushort lookYaw, ushort lookPitch)
            {
                return new CameraInput
                {
                    abiVersion = AbiVersion,
                    structSize = 28,
                    lookYaw = lookYaw,
                    lookPitch = lookPitch,
                    reserved = new uint[4]
                };
            }
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct SessionSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal ulong eventHead;
            internal ulong stateHash;
            internal ulong conformanceHash;
            internal ushort entityIndex;
            internal ushort x;
            internal ushort y;
            internal short z;
            internal short yaw;
            internal short displayPitch;
            internal short effectivePitch;
            internal short rollAccumulator;
            internal short targetSpeed;
            internal short actualSpeed;
            internal short strafeSpeed;
            internal byte phase;
            internal byte lastResolution;
            internal byte profile;
            internal byte localPlayerSlot;
            internal byte leftSpellSlot;
            internal byte rightSpellSlot;
            internal byte selectedSpellUiIndex;
            internal byte objectiveTextboxTicks;
            internal byte objectiveCount;
            internal byte objectivesAuthority;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 2)]
            internal byte[] reserved1;
            internal ushort leftSpellHolder;
            internal ushort rightSpellHolder;
            internal ushort selectedSpellHolder;
            internal ushort reserved0;
            internal int casterMana;
            internal int maptypeMusic;
            internal uint fireSequence;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct ObjectiveSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal byte slot;
            internal byte isLevelEnd;
            internal byte textSelector;
            internal byte done;
            internal byte authority;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 8)]
            internal byte[] stageSelectors;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 3)]
            internal byte[] reserved0;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct EntitySnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ushort entityIndex;
            internal ushort parameterRecordIndex;
            internal byte authority;
            internal byte active;
            internal byte classId;
            internal byte model;
            internal byte action;
            internal byte phase;
            internal byte hasParameterRecord;
            internal byte reserved0;
            internal ushort x;
            internal ushort y;
            internal short z;
            internal short yaw;
            internal short pitch;
            internal short fov;
            internal short actualSpeed;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct GoatSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ushort entityIndex;
            internal ushort targetEntityIndex;
            internal ushort renderParameterIndex;
            internal ushort baseTmap;
            internal byte authority;
            internal byte active;
            internal byte classId;
            internal byte model;
            internal byte action;
            internal byte phase;
            internal sbyte stageVar1;
            internal sbyte stageVar2;
            internal sbyte proximityCountdown;
            internal sbyte proximitySubcounter;
            internal sbyte animationFrame;
            internal byte reserved0;
            internal ushort x;
            internal ushort y;
            internal short z;
            internal short yaw;
            internal short pitch;
            internal short roll;
            internal short actualSpeed;
            internal short minimumSpeed;
            internal short maximumSpeed;
            internal ushort rng;
            internal int life;
            internal int maxLife;
            internal int mana;
            internal int pendingDamage;
            internal uint controlFlags;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct GoatBatch
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal ulong liveWorldHash;
            internal uint startOrdinal;
            internal uint written;
            internal uint totalCount;
            internal byte hasMore;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 3)]
            internal byte[] reserved0;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct Level1MissionSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal byte authority;
            internal byte currentStage;
            internal byte textSelector;
            internal byte objectiveTextboxTicks;
            internal byte tasksCompleted;
            internal byte playerDead;
            internal byte hasFireball;
            internal byte hasPossession;
            internal byte hasCastleSpell;
            internal byte hasSpeedSpell;
            internal byte castleExists;
            internal byte castleLevel;
            internal byte fidelityGapFlags;
            internal byte exitSequenceActive;
            internal byte exitSequenceComplete;
            internal byte exitResultCode;
            internal int playerLife;
            internal int playerMaxLife;
            internal int playerMana;
            internal int playerMaxMana;
            internal int castleMana;
            internal int securedManaPercent;
            internal uint actorCount;
            internal uint projectileCount;
            internal uint impactCount;
            internal uint exitSequencePhase;
            internal int exitSequenceTimer;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 2)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct Level1HudSpellSlot
        {
            internal ushort holderEntityIndex;
            internal ushort parentEntityIndex;
            internal byte model;
            internal sbyte tier;
            internal byte enabledFlags;
            internal sbyte actionMarker;
            internal short activeValue;
            internal ushort activationValue;
            internal int maxMana;
            internal int manaRegen;
            internal int casterMana;
            internal ushort frameSelector;
            internal ushort iconSelector;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 2)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct Level1HudSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal byte authority;
            internal byte manaColorIndex;
            internal byte manaMaxColorIndex;
            internal byte lifeColorIndex;
            internal byte colorPhase1;
            internal byte colorPhase2;
            internal byte playerHitFrameTicks;
            internal byte castleHitFrameTicks;
            internal int globalManaDenominator;
            internal int securedManaComponent;
            internal int goalPercent;
            internal uint playerStatusFlags;
            internal Level1HudSpellSlot left;
            internal Level1HudSpellSlot right;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 2)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct Level1MinimapMarks
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal byte markCount;
            internal byte markPulse;
            internal byte arrowValid;
            internal byte arrowVisible;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 16)]
            internal short[] markX;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 16)]
            internal short[] markY;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 16)]
            internal byte[] markColour;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 16)]
            internal byte[] markHelpSelector;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 14)]
            internal short[] arrowXY;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct Level1MinimapEntityCommand
        {
            internal ushort sourceSlot;
            internal short x;
            internal short y;
            internal ushort selector;
            internal byte kind;
            internal byte paletteIndex;
            internal byte size;
            internal byte anchor;
            internal byte helpSelector;
            internal byte flags;
            internal ushort order;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 2)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct Level1MinimapEntitiesInfo
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal uint totalCount;
            internal ushort crossX;
            internal ushort crossY;
            internal byte crossColour;
            internal byte crossArmLength;
            internal byte revealActive;
            internal byte authority;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal byte[] reserved0;
            internal ulong commandHash;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct Level1PaletteInfo
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal byte submode;
            internal byte effect;
            internal byte weight;
            internal byte denominator;
            internal short paletteCount;
            internal ushort flags;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 32)]
            internal byte[] rgb8Sha256;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct Level1ExitPresentation
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal byte authority;
            internal byte playerAction;
            internal byte actionPhase;
            internal byte paletteSubmode;
            internal byte paletteEffect;
            internal byte paletteWeight;
            internal byte paletteDenominator;
            internal byte movementResolution;
            internal ushort targetSourceEntityIndex;
            internal byte targetValid;
            internal byte inputSuppressed;
            internal short cameraFov;
            internal short actionTimer;
            internal byte playerStatusFlags;
            internal byte blurMode;
            internal byte displayBlank;
            internal byte resultCode;
            internal byte active;
            internal byte complete;
            internal byte flags;
            internal byte reserved0;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct Level1CastleSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal byte authority;
            internal byte active;
            internal byte level;
            internal byte classId;
            internal byte model;
            internal byte action;
            internal byte renderMode;
            internal byte reserved0;
            internal ushort ownerEntityIndex;
            internal ushort renderParameterIndex;
            internal ushort x;
            internal ushort y;
            internal short z;
            internal ushort baseTmap;
            internal ushort logicalHeight;
            internal ushort reserved1;
            internal int life;
            internal int maxLife;
            internal int mana;
            internal int manaCapacity;
            internal uint carrierId;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 3)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct TerrainState
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal ulong mutationSequence;
            internal ulong terrainHash;
            internal ushort width;
            internal ushort height;
            internal byte planeCount;
            internal byte hashSchema;
            internal byte authority;
            internal byte reserved0;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct Level1CastleLifecycleSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal uint rootPresentationId;
            internal ushort rootSourceEntityIndex;
            internal ushort rootOwnerEntityIndex;
            internal ushort rootX;
            internal ushort rootY;
            internal short rootZ;
            internal short rootBuildState;
            internal byte rootAction;
            internal byte rootLevel;
            internal byte rootActive;
            internal byte authority;
            internal int rootLife;
            internal uint constructionId;
            internal ushort constructionSourceEntityIndex;
            internal ushort constructionParentEntityIndex;
            internal ushort constructionOwnerEntityIndex;
            internal ushort constructionX;
            internal ushort constructionY;
            internal short constructionZ;
            internal short constructionTimer;
            internal byte constructionStage;
            internal byte constructionPresent;
            internal byte constructionActive;
            internal byte constructionInitialized;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 2)]
            internal byte[] reserved0;
            internal ulong terrainMutationSequence;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct ManaSphereSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal uint sphereId;
            internal ushort sourceEntityIndex;
            internal ushort ownerEntityIndex;
            internal byte authority;
            internal byte active;
            internal byte classId;
            internal byte model;
            internal byte action;
            internal byte phase;
            internal byte renderMode;
            internal byte reserved0;
            internal ushort x;
            internal ushort y;
            internal short z;
            internal short yaw;
            internal short velocityX;
            internal short velocityY;
            internal short verticalVelocity;
            internal ushort renderParameterIndex;
            internal ushort baseTmap;
            internal ushort logicalHeight;
            internal int mana;
            internal uint assignedCarrierId;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 3)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct ManaCarrierSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal uint carrierId;
            internal uint targetSphereId;
            internal ushort ownerEntityIndex;
            internal ushort renderParameterIndex;
            internal byte authority;
            internal byte active;
            internal byte classId;
            internal byte model;
            internal byte action;
            internal byte returningToCastle;
            internal byte renderMode;
            internal byte reserved0;
            internal ushort x;
            internal ushort y;
            internal short z;
            internal short yaw;
            internal short speed;
            internal ushort baseTmap;
            internal ushort logicalHeight;
            internal ushort reserved1;
            internal int carriedMana;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 3)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct Level1ActorSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ushort authoredIndex;
            internal short activationGroup;
            internal byte kind;
            internal byte authority;
            internal byte active;
            internal byte collected;
            internal byte classId;
            internal byte model;
            internal byte action;
            internal byte renderMode;
            internal ushort x;
            internal ushort y;
            internal short z;
            internal ushort renderParameterIndex;
            internal int maxLife;
            internal int life;
            internal int pendingDamage;
            internal ushort baseTmap;
            internal ushort logicalHeight;
            internal short yaw;
            internal sbyte animationFrame;
            internal byte visible;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 2)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct Level1MultipartSegmentSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ushort sourceEntityIndex;
            internal ushort headAuthoredIndex;
            internal ushort headSourceEntityIndex;
            internal ushort predecessorSourceEntityIndex;
            internal ushort successorSourceEntityIndex;
            internal byte ordinal;
            internal byte authority;
            internal byte active;
            internal byte visible;
            internal byte classId;
            internal byte model;
            internal byte action;
            internal byte renderMode;
            internal ushort x;
            internal ushort y;
            internal short z;
            internal short yaw;
            internal short pitch;
            internal byte phase;
            internal byte reserved0;
            internal short ownerId;
            internal int life;
            internal int pendingDamage;
            internal ushort renderParameterIndex;
            internal ushort baseTmap;
            internal ushort logicalHeight;
            internal short collisionHalfWidth;
            internal short collisionHalfHeight;
            internal short trailingDistance;
            internal uint reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct FireballProjectileSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal uint projectileId;
            internal ushort ownerEntityIndex;
            internal ushort renderIndex;
            internal byte authority;
            internal byte active;
            internal byte classId;
            internal byte model;
            internal ushort x;
            internal ushort y;
            internal short z;
            internal short yaw;
            internal short pitch;
            internal short speed;
            internal short lifeTicks;
            internal ushort baseTmap;
            internal int payload;
            internal ushort logicalHeight;
            internal byte renderMode;
            internal sbyte animationFrame;
            internal uint projectileKind;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 2)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct FireballImpactSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal uint impactId;
            internal uint sourceProjectileId;
            internal byte authority;
            internal byte active;
            internal byte damageSubmitted;
            internal byte classId;
            internal byte model;
            internal byte renderMode;
            internal sbyte animationFrame;
            internal byte reserved0;
            internal ushort x;
            internal ushort y;
            internal short z;
            internal short lifeTicks;
            internal int payload;
            internal ushort baseTmap;
            internal ushort renderParameterIndex;
            internal ushort logicalHeight;
            internal ushort reserved1;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 2)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct SelectorFrameState
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ushort selector;
            internal byte maskBit;
            internal byte authority;
            internal ushort width;
            internal ushort height;
            internal ushort sourceFrameIndex;
            internal ushort frameCount;
            internal uint streamStart;
            internal uint streamCursor;
            internal uint payloadBytes;
            internal uint pixelBytes;
            internal uint flags;
            internal ulong pixelHash;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct SelectorFrameBatch
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal ulong selectorStateHash;
            internal ulong lastDrawTick;
            internal ulong lastDrawMask;
            internal ulong validDrawMask;
            internal uint startOrdinal;
            internal uint written;
            internal uint totalCount;
            internal byte hasMore;
            internal byte hasDrawFeedback;
            internal ushort reserved0;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct SelectorDrawFeedback
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong drawTick;
            internal ulong selectorMask;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;

            internal static SelectorDrawFeedback Create(
                ulong generation,
                ulong drawTick,
                ulong selectorMask)
            {
                return new SelectorDrawFeedback
                {
                    abiVersion = AbiVersion,
                    structSize = 48,
                    generation = generation,
                    drawTick = drawTick,
                    selectorMask = selectorMask,
                    reserved = new uint[4]
                };
            }
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct GoatDamageCommand
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ushort entityIndex;
            internal ushort reserved0;
            internal int damage;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;

            internal static GoatDamageCommand Create(
                ulong generation,
                ushort entityIndex,
                int damage)
            {
                return new GoatDamageCommand
                {
                    abiVersion = AbiVersion,
                    structSize = 40,
                    generation = generation,
                    entityIndex = entityIndex,
                    damage = damage,
                    reserved = new uint[4]
                };
            }
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct PresentationState
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal int sourceTurn;
            internal ushort cameraX;
            internal ushort cameraY;
            internal short cameraZ;
            internal short cameraYaw;
            internal short cameraPitch;
            internal short cameraRoll;
            internal short cameraFov;
            internal byte cameraAuthority;
            internal byte cameraFlags;
            internal uint reserved0;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct RenderCameraState
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal int sourceTurn;
            internal ushort historyX;
            internal ushort historyY;
            internal short eyeZ;
            internal short yaw;
            internal short pitch;
            internal short roll;
            internal short fov;
            internal short slopeTargetX;
            internal short slopeTargetY;
            internal short renderOffsetX;
            internal short renderOffsetY;
            internal byte authority;
            internal byte flags;
            internal ushort reserved0;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct TerrainPnt5Vector
        {
            internal uint abiVersion;
            internal uint structSize;
            internal int turn;
            internal int tileX;
            internal int tileY;
            internal uint distanceSquared;
            internal byte shade;
            internal byte angle;
            internal byte visible;
            internal byte tablesnRow;
            internal int wave;
            internal int landDisplacement;
            internal int pnt5;
            internal int viewY;
            internal int lightingWave;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 2)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct TerrainGridVertex
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ushort index;
            internal byte row;
            internal byte column;
            internal byte tileX;
            internal byte tileY;
            internal byte shade;
            internal byte angle;
            internal int viewX;
            internal int viewDepth;
            internal uint distanceSquared;
            internal int pnt5;
            internal int rawWave;
            internal int surfaceDisplacement;
            internal int lightingWave;
            internal ushort featureFlags;
            internal byte tablesnRow;
            internal byte pnt5Written;
            internal uint reserved0;
            internal uint reserved1;
            internal uint reserved2;
            internal uint reserved3;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct TerrainFrameQuery
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal uint reserved0;
            internal uint reserved1;
            internal uint reserved2;
            internal uint reserved3;

            internal static TerrainFrameQuery Create(ulong generation, ulong tick)
            {
                return new TerrainFrameQuery
                {
                    abiVersion = AbiVersion,
                    structSize = 40,
                    generation = generation,
                    tick = tick
                };
            }
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct TerrainFrameInfo
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal ushort width;
            internal ushort height;
            internal ushort stride;
            internal ushort viewportX;
            internal ushort viewportY;
            internal ushort viewportWidth;
            internal ushort viewportHeight;
            internal ushort reserved0;
            internal uint indexBytes;
            internal uint coverageBytes;
            internal uint flags;
            internal uint reflectedTriangles;
            internal uint primaryMode5Triangles;
            internal uint primaryMode26Triangles;
            internal ulong indexHash;
            internal ulong coverageHash;
            internal ulong terrainPixelWrites;
            internal uint reserved1;
            internal uint reserved2;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct AssetInfo
        {
            internal uint abiVersion;
            internal uint structSize;
            internal uint assetIndex;
            internal ushort traceOrdinal;
            internal ushort ownerAssetCount;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct EventQuery
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong cursor;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;

            internal static EventQuery Create(ulong generation, ulong cursor)
            {
                return new EventQuery
                {
                    abiVersion = AbiVersion,
                    structSize = 40,
                    generation = generation,
                    cursor = cursor,
                    reserved = new uint[4]
                };
            }
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct EventBatch
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong nextCursor;
            internal ulong eventHead;
            internal uint written;
            internal byte hasMore;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 3)]
            internal byte[] reserved0;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct NativeEvent
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong sequence;
            internal ulong stateHash;
            internal uint kind;
            internal uint code;
            internal ushort entityIndex;
            internal short playerIndex;
            internal int value0;
            internal int value1;
            internal uint assetRefIndex;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct AudioCommand
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong eventSequence;
            internal ulong stateHash;
            internal uint assetRefIndex;
            internal int relativeX;
            internal int relativeY;
            internal int relativeZ;
            internal uint playbackRateHz;
            internal ushort soundId;
            internal ushort entityIndex;
            internal short playerIndex;
            internal short soundFlags;
            internal ushort volumeQ15;
            internal ushort panQ16;
            internal short rateDeltaPercent;
            internal ushort voiceIndex;
            internal byte playType;
            internal sbyte loopCount;
            internal byte action;
            internal byte reserved0;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern uint mc_native_abi_version();

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_session_open(
            byte[] packageDir,
            uint packageDirLength,
            byte[] closurePath,
            uint closurePathLength,
            out ulong handle);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_session_destroy(ulong handle);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_session_reset(
            ulong handle,
            out SessionSnapshot snapshot,
            uint snapshotSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_session_snapshot(
            ulong handle,
            out SessionSnapshot snapshot,
            uint snapshotSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_session_source_projection_hash(
            ulong handle,
            out ulong hash);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_session_source_arena_hash(
            ulong handle,
            out ulong hash);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_presentation_state(
            ulong handle,
            out PresentationState state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_render_camera_state(
            ulong handle,
            out RenderCameraState state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern uint mc_native_terrain_pnt5_vector_count();

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_terrain_pnt5_vector(
            uint index,
            out TerrainPnt5Vector vector,
            uint vectorSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern uint mc_native_terrain_grid_vertex_count();

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_session_terrain_grid_copy(
            ulong handle,
            [Out] TerrainGridVertex[] buffer,
            uint capacity,
            uint elementSize,
            out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_session_terrain_frame_copy(
            ulong handle,
            ref TerrainFrameQuery query,
            [Out] byte[] indexBuffer,
            uint indexCapacity,
            [Out] byte[] coverageBuffer,
            uint coverageCapacity,
            out TerrainFrameInfo info,
            uint infoSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_session_world_frame_copy(
            ulong handle,
            ref TerrainFrameQuery query,
            [Out] byte[] indexBuffer,
            uint indexCapacity,
            [Out] byte[] coverageBuffer,
            uint coverageCapacity,
            out TerrainFrameInfo info,
            uint infoSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_session_step(
            ulong handle,
            ref Command command,
            out SessionSnapshot snapshot,
            uint snapshotSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_session_step_with_camera(
            ulong handle,
            ref Command command,
            ref CameraInput cameraInput,
            out SessionSnapshot snapshot,
            uint snapshotSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_world_info(
            ulong handle,
            out WorldInfo info,
            uint infoSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_world_string_length(
            ulong handle,
            uint kind,
            out uint length);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_world_string_copy(
            ulong handle,
            uint kind,
            byte[] buffer,
            uint capacity,
            out uint length);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_terrain_plane_length(
            ulong handle,
            uint kind,
            out uint count);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_terrain_plane_copy_u8(
            ulong handle,
            uint kind,
            [Out] byte[] buffer,
            uint capacity,
            out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_terrain_entity_indices_copy(
            ulong handle,
            [Out] short[] buffer,
            uint capacity,
            out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_session_terrain_state(
            ulong handle,
            out TerrainState state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_entity_count(ulong handle, out uint count);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_entities_copy(
            ulong handle,
            uint startIndex,
            [Out] EntitySnapshot[] buffer,
            uint capacity,
            uint elementSize,
            out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_goats_copy(
            ulong handle,
            ulong generation,
            uint startOrdinal,
            [Out] GoatSnapshot[] buffer,
            uint capacity,
            uint elementSize,
            out GoatBatch batch,
            uint batchSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_level1_mission_snapshot(
            ulong handle,
            out Level1MissionSnapshot snapshot,
            uint snapshotSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_level1_hud_snapshot(
            ulong handle,
            out Level1HudSnapshot snapshot,
            uint snapshotSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_level1_minimap_marks(
            ulong handle,
            out Level1MinimapMarks state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_level1_minimap_entities_copy(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            [Out] Level1MinimapEntityCommand[] buffer,
            uint capacity,
            uint elementSize,
            out Level1MinimapEntitiesInfo info,
            uint infoSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_level1_palette_copy(
            ulong handle,
            [Out] byte[] rgb8Buffer,
            uint rgb8Capacity,
            out Level1PaletteInfo info,
            uint infoSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_level1_exit_presentation(
            ulong handle,
            out Level1ExitPresentation state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_level1_castle_snapshot(
            ulong handle,
            out Level1CastleSnapshot snapshot,
            uint snapshotSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_level1_castle_lifecycle_snapshot(
            ulong handle,
            out Level1CastleLifecycleSnapshot snapshot,
            uint snapshotSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_level1_mana_carrier_snapshot(
            ulong handle,
            out ManaCarrierSnapshot snapshot,
            uint snapshotSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_level1_mana_spheres_copy(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            uint startOrdinal,
            [Out] ManaSphereSnapshot[] buffer,
            uint capacity,
            uint elementSize,
            out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_level1_actors_copy(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            uint startOrdinal,
            [Out] Level1ActorSnapshot[] buffer,
            uint capacity,
            uint elementSize,
            out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_level1_multipart_segments_copy(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            uint startOrdinal,
            [Out] Level1MultipartSegmentSnapshot[] buffer,
            uint capacity,
            uint elementSize,
            out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_fireball_projectiles_copy(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            uint startOrdinal,
            [Out] FireballProjectileSnapshot[] buffer,
            uint capacity,
            uint elementSize,
            out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_fireball_impacts_copy(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            uint startOrdinal,
            [Out] FireballImpactSnapshot[] buffer,
            uint capacity,
            uint elementSize,
            out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_selector_frame_states_copy(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            uint startOrdinal,
            [Out] SelectorFrameState[] buffer,
            uint capacity,
            uint elementSize,
            out SelectorFrameBatch batch,
            uint batchSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_session_goat_draw_admission_evaluate(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            out SelectorFrameBatch batch,
            uint batchSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_session_selector_draw_feedback_submit(
            ulong handle,
            ref SelectorDrawFeedback command,
            out SelectorFrameBatch batch,
            uint batchSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_session_goat_inject_damage(
            ulong handle,
            ref GoatDamageCommand command,
            out GoatBatch batch,
            uint batchSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_objective_count(ulong handle, out uint count);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_objectives_copy(
            ulong handle,
            uint startSlot,
            [Out] ObjectiveSnapshot[] buffer,
            uint capacity,
            uint elementSize,
            out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_events_copy_since(
            ulong handle,
            ref EventQuery query,
            [Out] NativeEvent[] buffer,
            uint capacity,
            uint elementSize,
            out EventBatch batch,
            uint batchSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_audio_command_for_event(
            ulong handle,
            ulong generation,
            ulong eventSequence,
            out AudioCommand command,
            uint commandSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_audio_voice_complete(
            ulong handle,
            ulong generation,
            ushort voiceIndex,
            ulong eventSequence);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern uint mc_native_audio_qualification_vector_count();

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_audio_qualification_vector(
            uint index,
            out AudioCommand command,
            uint commandSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_audio_qualification_submit(
            ulong handle,
            uint eventKind,
            ushort soundId,
            short soundFlags,
            ushort volumeQ15,
            ushort panQ16,
            byte playType,
            out NativeEvent nativeEvent,
            uint eventSize,
            out AudioCommand command,
            uint commandSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_last_error_length(out uint length);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_last_error_copy(
            byte[] buffer,
            uint capacity,
            out uint length);

        internal static void AssertLayouts()
        {
            RequireSize<WorldInfo>(72);
            RequireSize<Command>(32);
            RequireSize<CameraInput>(28);
            RequireSize<SessionSnapshot>(120);
            RequireSize<ObjectiveSnapshot>(40);
            RequireSize<EntitySnapshot>(52);
            RequireSize<GoatSnapshot>(84);
            RequireSize<GoatBatch>(64);
            RequireSize<Level1MissionSnapshot>(96);
            RequireSize<Level1HudSpellSlot>(36);
            RequireSize<Level1HudSnapshot>(128);
            RequireSize<Level1MinimapMarks>(168);
            RequireSize<Level1MinimapEntityCommand>(24);
            RequireSize<Level1MinimapEntitiesInfo>(64);
            RequireSize<Level1PaletteInfo>(80);
            RequireSize<Level1ExitPresentation>(64);
            RequireSize<Level1CastleSnapshot>(80);
            RequireSize<TerrainState>(64);
            RequireSize<Level1CastleLifecycleSnapshot>(80);
            RequireSize<ManaSphereSnapshot>(64);
            RequireSize<ManaCarrierSnapshot>(80);
            RequireSize<Level1ActorSnapshot>(56);
            RequireSize<Level1MultipartSegmentSnapshot>(64);
            RequireSize<FireballProjectileSnapshot>(56);
            RequireSize<FireballImpactSnapshot>(52);
            RequireSize<SelectorFrameState>(64);
            RequireSize<SelectorFrameBatch>(88);
            RequireSize<SelectorDrawFeedback>(48);
            RequireSize<GoatDamageCommand>(40);
            RequireSize<PresentationState>(64);
            RequireSize<RenderCameraState>(72);
            RequireSize<TerrainPnt5Vector>(56);
            RequireSize<TerrainGridVertex>(64);
            RequireSize<TerrainFrameQuery>(40);
            RequireSize<TerrainFrameInfo>(96);
            RequireSize<AssetInfo>(32);
            RequireSize<EventQuery>(40);
            RequireSize<EventBatch>(56);
            RequireSize<NativeEvent>(72);
            RequireSize<AudioCommand>(88);
            RequireOffset<WorldInfo>(nameof(WorldInfo.resetStateHash), 40);
            RequireOffset<Command>(nameof(Command.requestedSpell), 14);
            RequireOffset<CameraInput>(nameof(CameraInput.lookYaw), 8);
            RequireOffset<CameraInput>(nameof(CameraInput.reserved), 12);
            RequireOffset<SessionSnapshot>(nameof(SessionSnapshot.entityIndex), 48);
            RequireOffset<SessionSnapshot>(nameof(SessionSnapshot.objectiveTextboxTicks), 77);
            RequireOffset<SessionSnapshot>(nameof(SessionSnapshot.objectivesAuthority), 79);
            RequireOffset<SessionSnapshot>(nameof(SessionSnapshot.leftSpellHolder), 82);
            RequireOffset<SessionSnapshot>(nameof(SessionSnapshot.reserved), 104);
            RequireOffset<ObjectiveSnapshot>(nameof(ObjectiveSnapshot.stageSelectors), 13);
            RequireOffset<ObjectiveSnapshot>(nameof(ObjectiveSnapshot.reserved), 24);
            RequireOffset<EntitySnapshot>(nameof(EntitySnapshot.reserved), 36);
            RequireOffset<GoatSnapshot>(nameof(GoatSnapshot.life), 48);
            RequireOffset<GoatSnapshot>(nameof(GoatSnapshot.reserved), 68);
            RequireOffset<GoatBatch>(nameof(GoatBatch.reserved), 48);
            RequireOffset<Level1MissionSnapshot>(
                nameof(Level1MissionSnapshot.playerLife), 40);
            RequireOffset<Level1MissionSnapshot>(
                nameof(Level1MissionSnapshot.actorCount), 64);
            RequireOffset<Level1MissionSnapshot>(
                nameof(Level1MissionSnapshot.exitSequencePhase), 76);
            RequireOffset<Level1MissionSnapshot>(
                nameof(Level1MissionSnapshot.exitSequenceTimer), 80);
            RequireOffset<Level1MissionSnapshot>(
                nameof(Level1MissionSnapshot.reserved), 84);
            RequireOffset<Level1HudSpellSlot>(
                nameof(Level1HudSpellSlot.maxMana), 12);
            RequireOffset<Level1HudSnapshot>(
                nameof(Level1HudSnapshot.globalManaDenominator), 32);
            RequireOffset<Level1HudSnapshot>(
                nameof(Level1HudSnapshot.left), 48);
            RequireOffset<Level1HudSnapshot>(
                nameof(Level1HudSnapshot.right), 84);
            RequireOffset<Level1HudSnapshot>(
                nameof(Level1HudSnapshot.reserved), 120);
            RequireOffset<Level1MinimapMarks>(
                nameof(Level1MinimapMarks.markX), 28);
            RequireOffset<Level1MinimapMarks>(
                nameof(Level1MinimapMarks.arrowXY), 124);
            RequireOffset<Level1MinimapMarks>(
                nameof(Level1MinimapMarks.reserved), 152);
            RequireOffset<Level1MinimapEntityCommand>(
                nameof(Level1MinimapEntityCommand.reserved), 16);
            RequireOffset<Level1MinimapEntitiesInfo>(
                nameof(Level1MinimapEntitiesInfo.commandHash), 40);
            RequireOffset<Level1MinimapEntitiesInfo>(
                nameof(Level1MinimapEntitiesInfo.reserved), 48);
            RequireOffset<Level1PaletteInfo>(
                nameof(Level1PaletteInfo.rgb8Sha256), 32);
            RequireOffset<Level1PaletteInfo>(
                nameof(Level1PaletteInfo.reserved), 64);
            RequireOffset<Level1ExitPresentation>(
                nameof(Level1ExitPresentation.playerAction), 25);
            RequireOffset<Level1ExitPresentation>(
                nameof(Level1ExitPresentation.cameraFov), 36);
            RequireOffset<Level1ExitPresentation>(
                nameof(Level1ExitPresentation.reserved), 48);
            RequireOffset<Level1CastleSnapshot>(
                nameof(Level1CastleSnapshot.life), 48);
            RequireOffset<Level1CastleSnapshot>(
                nameof(Level1CastleSnapshot.reserved), 68);
            RequireOffset<TerrainState>(
                nameof(TerrainState.mutationSequence), 24);
            RequireOffset<TerrainState>(
                nameof(TerrainState.width), 40);
            RequireOffset<TerrainState>(
                nameof(TerrainState.reserved), 48);
            RequireOffset<Level1CastleLifecycleSnapshot>(
                nameof(Level1CastleLifecycleSnapshot.rootLife), 44);
            RequireOffset<Level1CastleLifecycleSnapshot>(
                nameof(Level1CastleLifecycleSnapshot.constructionTimer), 64);
            RequireOffset<Level1CastleLifecycleSnapshot>(
                nameof(Level1CastleLifecycleSnapshot.terrainMutationSequence), 72);
            RequireOffset<ManaSphereSnapshot>(
                nameof(ManaSphereSnapshot.mana), 44);
            RequireOffset<ManaSphereSnapshot>(
                nameof(ManaSphereSnapshot.reserved), 52);
            RequireOffset<ManaCarrierSnapshot>(
                nameof(ManaCarrierSnapshot.carriedMana), 60);
            RequireOffset<ManaCarrierSnapshot>(
                nameof(ManaCarrierSnapshot.reserved), 64);
            RequireOffset<Level1ActorSnapshot>(
                nameof(Level1ActorSnapshot.maxLife), 28);
            RequireOffset<Level1ActorSnapshot>(
                nameof(Level1ActorSnapshot.baseTmap), 40);
            RequireOffset<Level1ActorSnapshot>(
                nameof(Level1ActorSnapshot.yaw), 44);
            RequireOffset<Level1ActorSnapshot>(
                nameof(Level1ActorSnapshot.reserved), 48);
            RequireOffset<Level1MultipartSegmentSnapshot>(
                nameof(Level1MultipartSegmentSnapshot.x), 26);
            RequireOffset<Level1MultipartSegmentSnapshot>(
                nameof(Level1MultipartSegmentSnapshot.life), 40);
            RequireOffset<Level1MultipartSegmentSnapshot>(
                nameof(Level1MultipartSegmentSnapshot.baseTmap), 50);
            RequireOffset<Level1MultipartSegmentSnapshot>(
                nameof(Level1MultipartSegmentSnapshot.reserved), 60);
            RequireOffset<FireballProjectileSnapshot>(
                nameof(FireballProjectileSnapshot.payload), 36);
            RequireOffset<FireballProjectileSnapshot>(
                nameof(FireballProjectileSnapshot.logicalHeight), 40);
            RequireOffset<FireballProjectileSnapshot>(
                nameof(FireballProjectileSnapshot.reserved), 48);
            RequireOffset<FireballImpactSnapshot>(
                nameof(FireballImpactSnapshot.payload), 32);
            RequireOffset<FireballImpactSnapshot>(
                nameof(FireballImpactSnapshot.baseTmap), 36);
            RequireOffset<FireballImpactSnapshot>(
                nameof(FireballImpactSnapshot.reserved), 44);
            RequireOffset<SelectorFrameState>(nameof(SelectorFrameState.pixelHash), 40);
            RequireOffset<SelectorFrameState>(nameof(SelectorFrameState.reserved), 48);
            RequireOffset<SelectorFrameBatch>(nameof(SelectorFrameBatch.startOrdinal), 56);
            RequireOffset<SelectorFrameBatch>(nameof(SelectorFrameBatch.reserved), 72);
            RequireOffset<SelectorDrawFeedback>(nameof(SelectorDrawFeedback.reserved), 32);
            RequireOffset<GoatDamageCommand>(nameof(GoatDamageCommand.reserved), 24);
            RequireOffset<PresentationState>(nameof(PresentationState.sourceTurn), 24);
            RequireOffset<PresentationState>(nameof(PresentationState.cameraX), 28);
            RequireOffset<PresentationState>(nameof(PresentationState.cameraAuthority), 42);
            RequireOffset<PresentationState>(nameof(PresentationState.reserved), 48);
            RequireOffset<RenderCameraState>(nameof(RenderCameraState.historyX), 28);
            RequireOffset<RenderCameraState>(nameof(RenderCameraState.slopeTargetX), 42);
            RequireOffset<RenderCameraState>(nameof(RenderCameraState.renderOffsetX), 46);
            RequireOffset<RenderCameraState>(nameof(RenderCameraState.reserved), 56);
            RequireOffset<TerrainPnt5Vector>(nameof(TerrainPnt5Vector.pnt5), 36);
            RequireOffset<TerrainGridVertex>(nameof(TerrainGridVertex.viewX), 16);
            RequireOffset<TerrainGridVertex>(nameof(TerrainGridVertex.featureFlags), 44);
            RequireOffset<TerrainGridVertex>(nameof(TerrainGridVertex.reserved0), 48);
            RequireOffset<TerrainFrameQuery>(nameof(TerrainFrameQuery.reserved0), 24);
            RequireOffset<TerrainFrameInfo>(nameof(TerrainFrameInfo.width), 24);
            RequireOffset<TerrainFrameInfo>(nameof(TerrainFrameInfo.indexBytes), 40);
            RequireOffset<TerrainFrameInfo>(nameof(TerrainFrameInfo.indexHash), 64);
            RequireOffset<TerrainFrameInfo>(nameof(TerrainFrameInfo.reserved1), 88);
            RequireOffset<NativeEvent>(nameof(NativeEvent.stateHash), 24);
            RequireOffset<NativeEvent>(nameof(NativeEvent.reserved), 56);
            RequireOffset<AudioCommand>(nameof(AudioCommand.playbackRateHz), 48);
            RequireOffset<AudioCommand>(nameof(AudioCommand.soundId), 52);
            RequireOffset<AudioCommand>(nameof(AudioCommand.volumeQ15), 60);
            RequireOffset<AudioCommand>(nameof(AudioCommand.playType), 68);
            RequireOffset<AudioCommand>(nameof(AudioCommand.reserved), 72);
        }

        private static void RequireSize<T>(int expected)
        {
            var actual = Marshal.SizeOf<T>();
            if (actual != expected)
            {
                throw new InvalidOperationException(
                    $"ABI layout {typeof(T).Name} size {actual}, expected {expected}");
            }
        }

        private static void RequireOffset<T>(string field, int expected)
        {
            var actual = Marshal.OffsetOf<T>(field).ToInt32();
            if (actual != expected)
            {
                throw new InvalidOperationException(
                    $"ABI layout {typeof(T).Name}.{field} offset {actual}, expected {expected}");
            }
        }
    }
}
