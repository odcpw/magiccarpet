using System;
using System.Runtime.InteropServices;

namespace MagicCarpet.Native
{
    internal static class NativeSim
    {
        internal const uint AbiVersion = 2;
        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_world_entity_presentations_copy(
            ulong handle, ulong generation, ulong tick, short cameraYaw,
            [Out] CampaignEntityPresentation[] buffer, uint capacity, uint elementSize, out uint written);
        [StructLayout(LayoutKind.Sequential)]
        internal struct VoxelState {
            internal uint abiVersion,structSize;
            internal ulong generation,tick,revision,contentHash;
            internal uint enabled,removedCount;
            internal ulong impactHead,worldGeneration;
        }
        [StructLayout(LayoutKind.Sequential)]
        internal struct VoxelImpactChild
        {
            internal uint abiVersion,structSize;
            internal ulong generation,sequence;
            internal uint slotGeneration;
            internal ushort slot,reserved;
        }
        [DllImport(Library,CallingConvention=CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_voxel_impact_children_copy(ulong handle,ulong generation,ulong tick,
            [Out] VoxelImpactChild[] buffer,uint capacity,uint elementSize,out uint written);
        [DllImport(Library,CallingConvention=CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_voxel_enable_json_v1(ulong handle,ulong generation,ulong tick,byte[] request,uint length);
        [DllImport(Library,CallingConvention=CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_voxel_state_v1(ulong handle,ulong generation,ulong tick,out VoxelState state,uint size);
        [DllImport(Library,CallingConvention=CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_voxel_snapshot_json_v1(ulong handle,ulong generation,ulong tick,uint includeBase,ulong afterImpact,IntPtr buffer,uint capacity,out uint length);
        internal const int Ok = 0;
        internal const int InvalidArgument = 3;
        internal const int ValidationFailed = 8;
        internal const int UnknownHandle = 9;
        internal const int Bounds = 10;
        internal const int BufferTooSmall = 11;
        internal const int GenerationMismatch = 12;
        internal const int CursorExpired = 13;
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
        internal const byte CampaignEconomyCastleDefenderCollectedSphere = 16;
        internal const byte CampaignEconomyManaSphereSpawned = 1;
        internal const byte CampaignEconomyDeathBurstSpawned = 8;
        internal const byte CampaignGoatBleat = 1;
        internal const byte CampaignGoatExtraManaSphereRequest = 3;
        internal const byte CampaignGoatRemovalQueued = 4;
        internal const byte CampaignGoatReclaimed = 5;
        internal const uint EventLevel1ExitSequenceStarted = 35;
        internal const uint EventLevel1ExitSequenceCompleted = 36;
        internal const uint EventLevel1WaterSplashed = 37;
        internal const uint EventLevel1AbilityImproved = 38;
        internal const byte CampaignEventSharedPickup = 1;
        internal const byte CampaignEventSpellAcquisition = 2;
        internal const byte CampaignEventBadStone = 3;
        internal const byte CampaignEventHouse = 4;
        internal const byte CampaignEventVillager = 5;
        internal const byte CampaignEventDolmen = 6;
        internal const byte CampaignEventCastle = 7;
        internal const byte CampaignEventDynamicArena = 8;
        internal const byte CampaignEventGoat = 9;
        internal const byte CampaignEventEconomy = 10;
        internal const byte CampaignEventAuthoredGroup = 11;
        internal const byte CampaignEventError = 255;
        internal const uint CampaignEffectDispatch = 0x100;
        internal const uint CampaignEffectSharedSpell = 0x101;
        internal const uint CampaignEffectFlight = 0x102;
        internal const uint CampaignEffectSceneChange = 0x103;
        internal const uint CampaignEffectObjectivePresentation = 0x104;
        internal const byte AudioActionPreparedPending = 0;
        internal const byte AudioActionStart = 1;
        internal const byte AudioActionRestart = 2;
        internal const byte AudioActionSuppressExisting = 3;
        internal const byte AudioActionSuppressNoVoice = 4;
        internal const byte AudioActionSuppressDistanceOrVolume = 5;
        internal const byte AudioActionSuppressPendingReplaced = 6;
        internal const byte AudioActionUpdateExisting = 7;
        internal const byte AudioActionEndLoop = 8;
        internal const byte AudioActionSuppressNoExisting = 9;
        internal const byte AudioActionSuppressNullSample = 10;
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
        internal const byte VillagerLive = 8;
        internal const byte CampaignLive = 9;
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
        internal const uint PresentationFrameCompositorComplete = 0x2000;
        internal const uint PresentationFrameHasProvenance = 0x4000;
        internal const uint PresentationFrameOverlaysComposited = 0x8000;
        internal const uint PresentationFramePostprocessComplete = 0x10000;
        internal const uint PresentationFrameLightingComplete = 0x20000;
        internal const uint PresentationInputPause = 0x01;
        internal const uint PresentationInputChat = 0x02;
        internal const uint PresentationInputOptions = 0x04;
        internal const uint PresentationInputVolume = 0x08;
        internal const uint PresentationInputOkCancel = 0x10;
        internal const uint PresentationInputPointer = 0x20;
        internal const uint PresentationInputNotificationName = 0x40;
        internal const uint PresentationInputDebugMetrics = 0x80;
        internal const uint PresentationInputAll = 0xff;
        internal const int TerrainFrameWidth = 320;
        internal const int TerrainFrameHeight = 200;
        internal const int TerrainFramePixels = TerrainFrameWidth * TerrainFrameHeight;
        internal const int DayIndexedFrameWidth = 320;
        internal const int DayIndexedFrameHeight = 200;
        internal const int DayIndexedFrameIndexBytes =
            DayIndexedFrameWidth * DayIndexedFrameHeight;
        internal const int DayIndexedFramePaletteBytes = 256 * 3;
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
        internal struct CampaignSelection
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ushort externalLevel;
            internal ushort internalLevel;
            internal byte mapType;
            internal byte reserved0;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;

            internal static CampaignSelection Create(
                ushort externalLevel,
                ushort internalLevel,
                byte mapType)
            {
                return new CampaignSelection
                {
                    abiVersion = AbiVersion,
                    structSize = 32,
                    externalLevel = externalLevel,
                    internalLevel = internalLevel,
                    mapType = mapType,
                    reserved = new uint[4]
                };
            }

            internal static CampaignSelection Level2Day()
            {
                return Create(2, 1, 0);
            }
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignStepInput
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal Command command;
            internal byte playerSlot;
            internal byte speedIndex;
            internal ushort reserved0;
            internal ushort playerX;
            internal ushort playerY;
            internal short playerZ;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;

            internal static CampaignStepInput Create(
                ulong generation,
                Command command,
                byte playerSlot,
                byte speedIndex,
                ushort playerX,
                ushort playerY,
                short playerZ)
            {
                return new CampaignStepInput
                {
                    abiVersion = AbiVersion,
                    structSize = 80,
                    generation = generation,
                    command = command,
                    playerSlot = playerSlot,
                    speedIndex = speedIndex,
                    playerX = playerX,
                    playerY = playerY,
                    playerZ = playerZ,
                    reserved = new uint[4]
                };
            }
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignEvent
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal uint ordinal;
            internal byte pass;
            internal byte kind;
            internal byte family;
            internal byte flags;
            internal ushort sourceSlot;
            internal ushort reserved0;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignAuthoredGroupEvent
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal uint ordinal;
            internal byte pass;
            internal byte flags;
            internal ushort group;
            internal ushort sourceSlot;
            internal ushort eventIndex;
            internal ushort authoredIndex;
            internal ushort reserved0;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignEconomyEvent
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal uint ordinal;
            internal byte pass;
            internal byte family;
            internal byte code;
            internal byte flags;
            internal ushort sourceSlot;
            internal ushort reserved0;
            internal uint primaryId;
            internal uint secondaryId;
            internal ushort entity0;
            internal ushort entity1;
            internal int value0;
            internal int value1;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignGoatEvent
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal uint ordinal;
            internal byte pass;
            internal byte family;
            internal byte code;
            internal byte flags;
            internal ushort sourceSlot;
            internal ushort eventIndex;
            internal ushort entity0;
            internal ushort entity1;
            internal int value0;
            internal int value1;
            internal ushort soundId;
            internal byte action;
            internal byte reserved0;
            internal ushort x;
            internal ushort y;
            internal short z;
            internal uint assetRefIndex;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 2)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignStepSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong revision;
            internal ulong stateHash;
            internal ulong terrainHash;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 32)]
            internal byte[] dispatchHash;
            internal uint eventCount;
            internal uint eventKindMask;
            internal byte playerSlot;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 3)]
            internal byte[] reserved0;
            internal ushort playerEntityIndex;
            internal ushort reserved1;
            internal ushort x;
            internal ushort y;
            internal short z;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignWorldInfo
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
            internal uint objectiveCount;
            internal ushort terrainWidth;
            internal ushort terrainHeight;
            internal ulong resetStateHash;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignTerrainState
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
        internal struct CampaignDayIndexedFrameInfo
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal ushort width;
            internal ushort height;
            internal ushort stride;
            internal ushort reserved0;
            internal uint indexBytes;
            internal uint paletteBytes;
            internal ulong indexHash;
            internal ulong paletteHash;
            internal ulong terrainInputHash;
            internal ulong terrainCommandHash;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 32)]
            internal byte[] packageSha256;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 32)]
            internal byte[] cacheContentSha256;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignObjectiveStageSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal byte objectiveSlot;
            internal byte stageSlot;
            internal byte isLevelEnd;
            internal byte done;
            internal byte textSelector;
            internal byte stageState;
            internal sbyte stageKind;
            internal sbyte stageFlags;
            internal short axisX;
            internal short axisY;
            internal byte relationDomain;
            internal byte reserved0;
            internal ushort relationValue;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignPlayerSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal byte slot;
            internal byte isAi;
            internal byte active;
            internal byte reserved0;
            internal ushort entityIndex;
            internal short activePlayerIndex;
            internal int turn;
            internal short leftSpellSlotRaw;
            internal short rightSpellSlotRaw;
            internal sbyte leftSubspellIndex;
            internal sbyte rightSubspellIndex;
            internal byte selectedSpellUiIndex;
            internal sbyte selectedSubspellIndex;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 2)]
            internal byte[] reserved1;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignEntitySnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ushort entityIndex;
            internal ushort sourceId;
            internal ushort nextRelation;
            internal ushort parameterRelation;
            internal byte authority;
            internal byte active;
            internal byte classId;
            internal byte model;
            internal byte action;
            internal byte phase;
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
        internal struct CampaignVoxelDeath
        {
            internal uint abiVersion, structSize;
            internal ulong generation, sequence;
            internal uint slotGeneration;
            internal ushort entityIndex, sourceId;
        }

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_voxel_deaths_copy(
            ulong handle, ulong generation, ulong tick, ulong afterSequence,
            [Out] CampaignVoxelDeath[] buffer, uint capacity, uint elementSize, out uint written);

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignCharacterState
        {
            internal uint abiVersion, structSize;
            internal ulong generation, tick;
            internal uint slotGeneration, parentGeneration;
            internal ushort entityIndex, sourceId, parentIndex, parentId;
            internal byte classId, model, action, phase, active, hasTarget, hasParent, stance;
            internal ushort x, y;
            internal short z, yaw, pitch, speed;
            internal int life;
            internal ushort targetIndex, targetX, targetY;
            internal short targetZ;
            internal uint controlFlags;
            internal ushort ageTicks, reserved1;
        }

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_character_states_copy(
            ulong handle, ulong generation, ulong tick,
            [Out] CampaignCharacterState[] buffer, uint capacity, uint elementSize, out uint written);

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignEntityPresentation
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal ushort entityIndex;
            internal ushort sourceId;
            internal ushort renderParameterIndex;
            internal ushort baseTmap;
            internal ushort selectedTmap;
            internal ushort logicalHeight;
            internal byte renderMode;
            internal byte orientationSector;
            internal byte mirrored;
            internal byte reserved0;
            internal ushort x;
            internal ushort y;
            internal short z;
            internal short yaw;
            internal byte pixelMode;
            internal byte ownerColor;
            internal ushort brightness;
            internal uint drawOrder;
            internal uint controlFlags;
            internal short primaryDepthAdjustment;
            internal ushort reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignShieldSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal byte active;
            internal byte tier;
            internal byte action;
            internal byte reserved0;
            internal ushort holderEntityIndex;
            internal ushort parentEntityIndex;
            internal short activeValue;
            internal short activationValue;
            internal short countdown;
            internal short reserved1;
            internal int manaCost;
            internal int parentMana;
            internal int parentLife;
            internal int parentManaRegen;
            internal uint parentStatusFlags;
            internal int experienceAwarded;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignInvisibleSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal byte active;
            internal byte tier;
            internal byte action;
            internal byte parentVisibilityMode;
            internal ushort holderEntityIndex;
            internal ushort parentEntityIndex;
            internal short activeValue;
            internal short activationValue;
            internal short countdown;
            internal short reserved0;
            internal int manaCost;
            internal int manaRegenRequirement;
            internal int parentMana;
            internal int parentLife;
            internal int parentManaRegen;
            internal uint parentStatusFlags;
            internal uint spellStatusFlags;
            internal int experienceAwarded;
            internal byte cleanupRequested;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 3)]
            internal byte[] reserved1;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 3)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignBeyondSightSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal byte active;
            internal byte tier;
            internal byte action;
            internal byte reserved0;
            internal ushort holderEntityIndex;
            internal ushort parentEntityIndex;
            internal short activeValue;
            internal short activationValue;
            internal short countdown;
            internal short reserved1;
            internal int manaCost;
            internal int manaRegenRequirement;
            internal int parentMana;
            internal int parentLife;
            internal int parentManaRegen;
            internal int experienceAwarded;
            internal byte cleanupRequested;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 3)]
            internal byte[] reserved2;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 5)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignOpponentSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal byte active;
            internal byte playerSlot;
            internal byte isAi;
            internal byte colorIndex;
            internal ushort entityIndex;
            internal byte entityClass;
            internal byte entityModel;
            internal byte entityAction;
            internal byte reserved0;
            internal ushort entitySourceId;
            internal ushort entityNextRelation;
            internal ushort entityNextEntity;
            internal ushort reserved1;
            internal uint entityControlFlags;
            internal int entityLife;
            internal int entityMaxLife;
            internal int entityMana;
            internal int entityMaxMana;
            internal ushort fireballHolderIndex;
            internal byte fireballAction;
            internal byte reserved2;
            internal ushort fireballParentEntityIndex;
            internal ushort fireballNextEntity;
            internal ushort possessionHolderIndex;
            internal byte possessionAction;
            internal byte reserved3;
            internal ushort possessionParentEntityIndex;
            internal ushort possessionNextEntity;
            internal byte objectiveRelationDomain;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 3)]
            internal byte[] reserved4;
            internal ushort objectiveRelationValue;
            internal ushort reserved5;
            internal uint visibilityFlags;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignDayHudSpellSlot
        {
            internal ushort holderEntityIndex;
            internal ushort parentEntityIndex;
            internal short rawSpellSelector;
            internal byte colorIndex;
            internal byte model;
            internal sbyte tier;
            internal byte action;
            internal sbyte actionMarker;
            internal short activeValue;
            internal ushort activationValue;
            internal uint controlFlags;
            internal int maxMana;
            internal int manaRegen;
            internal int casterMana;
            internal ushort frameSelector;
            internal ushort iconSelector;
            internal byte levelTextSelector;
            internal byte drawAdmitted;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 2)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignDayHudSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal byte localPlayerSlot;
            internal byte localPlayerActive;
            internal byte localPlayerColorIndex;
            internal byte hitFrameTimePreDraw;
            internal ushort localPlayerEntityIndex;
            internal byte colorPhase1;
            internal byte colorPhase2;
            internal uint localPlayerControlFlags;
            internal int localPlayerLife;
            internal int localPlayerMaxLife;
            internal int localPlayerMana;
            internal int localPlayerMaxMana;
            internal int globalManaDenominator;
            internal ushort minimapPanelSelector;
            internal ushort castlePanelSelector;
            internal ushort balloonPanelSelector;
            internal ushort playerPanelSelector;
            internal ushort playerIconSelector;
            internal ushort lifeManaIconSelector;
            internal byte lifePaletteIndex;
            internal byte manaPaletteIndex;
            internal byte manaMaxPaletteIndex;
            internal byte reserved0;
            internal CampaignOpponentSnapshot nyphur;
            internal CampaignDayHudSpellSlot left;
            internal CampaignDayHudSpellSlot right;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignDayMusicSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal uint assetSlot;
            internal uint startCount;
            internal uint updateCount;
            internal uint fadeCallbackCount;
            internal uint stopCount;
            internal uint endCount;
            internal byte initialized;
            internal byte loaded;
            internal byte active;
            internal byte playing;
            internal byte track;
            internal byte volume;
            internal byte updatesEnabled;
            internal byte timerActive;
            internal byte targetMode;
            internal sbyte direction;
            internal byte currentUnits;
            internal byte timerMultiplier;
            internal ushort timerHz;
            internal ushort reserved0;
            internal ulong stateHash;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 2)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignMinimapState
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal ushort localPlayerEntityIndex;
            internal ushort reserved0;
            internal uint entityCount;
            internal uint markCount;
            internal ulong entityHash;
            internal ulong markHash;
            internal uint visibilityFlags;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 3)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignMinimapEntity
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal ushort sourceOrder;
            internal ushort subjectEntityIndex;
            internal short x;
            internal short y;
            internal ushort selector;
            internal byte paletteIndex;
            internal byte kind;
            internal byte size;
            internal byte anchor;
            internal byte helpSelector;
            internal byte visible;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 3)]
            internal uint[] reserved;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignMinimapMark
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal ushort sourceOrder;
            internal ushort subjectEntityIndex;
            internal short x;
            internal short y;
            internal byte paletteIndex;
            internal byte kind;
            internal byte helpSelector;
            internal byte visible;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 3)]
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
        internal struct CampaignGoatSnapshot
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
            internal ushort partnerEntityIndex;
            internal ushort reserved1;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 3)]
            internal uint[] reserved2;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct CampaignSpellPickupSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ushort sourceEntityIndex;
            internal ushort authoredIndex;
            internal byte family;
            internal byte action;
            internal byte phase;
            internal byte active;
            internal ushort x;
            internal ushort y;
            internal short z;
            internal ushort entityRng;
            internal ushort reserved0;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 9)]
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
            internal int possessionExperience;
            internal ushort lastPossessedBuildingEntity;
            internal ushort lastPossessedBuildingOwner;
            internal ushort possessedBuildingCount;
            internal byte possessionTier;
            internal byte speedTrailCount;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct Level1PlayerLifecycleSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal ulong lifecycleHash;
            internal ulong sourceArenaHash;
            internal byte authority;
            internal byte playerAction;
            internal byte playerDead;
            internal byte inputSuppressed;
            internal byte playerActive;
            internal byte playerVisible;
            internal byte playerTargetable;
            internal byte menuCloseRequested;
            internal byte notificationActive;
            internal byte notificationState;
            internal byte banishedCameraActive;
            internal byte markerPresent;
            internal short fallVelocity;
            internal short notificationTimer;
            internal int actionTimer;
            internal int replaceableCursor;
            internal ushort killerEntityIndex;
            internal ushort killerScoreAwards;
            internal ushort notificationId;
            internal ushort playerEntityRng;
            internal uint playerControlFlags;
            internal short playerEntityRoll;
            internal short playerYaw;
            internal short controllerPitch;
            internal short controllerRoll;
            internal short recoilSpeed;
            internal ushort markerSourceEntityIndex;
            internal ushort markerX;
            internal ushort markerY;
            internal short markerZ;
            internal ushort markerTriggerEntityIndex;
            internal ushort markerParticleParameterIndex;
            internal int markerLife;
            internal int markerMaxLife;
            internal uint sourcePoolRebuildCount;
            internal ulong lifecycleTraceHash;
            internal ushort lifecycleTraceCount;
            internal byte markerDispatchPhase;
            internal byte markerClass;
            internal byte markerModel;
            internal byte markerAction;
            internal byte markerByte38;
            internal byte sourceColorPhase6;
            internal uint sourceTurnCounter;
            internal byte menuState;
            internal byte killerScoreVictimColor;
            internal short effectivePitch;
            internal uint markerConstructorPhase;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal byte[] reserved;
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
            internal ushort selectedHolderEntityIndex;
            internal ushort selectedIconSelector;
            internal byte selectedModel;
            internal sbyte selectedTier;
            internal byte selectedEnabledFlags;
            internal byte selectedFrameSelector;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct Level1SpellMenuSnapshot
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal byte authority;
            internal byte selectedUiIndex;
            internal sbyte selectedTier;
            internal byte leftFamily;
            internal sbyte leftTier;
            internal byte rightFamily;
            internal sbyte rightTier;
            internal byte reserved0;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal byte[] familyEnabled;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal sbyte[] familyUnlockedTier;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal sbyte[] familySelectedTier;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal byte[] familyHandEligibility;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal ushort[] holderEntityIndex;
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
            internal int stateValue;
            internal uint reserved;
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
        internal struct PresentationFrameQuery
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal ulong presentationRevision;
            internal ushort viewportWidth;
            internal ushort viewportHeight;
            internal ushort screenStride;
            internal ushort reserved0;
            internal byte configuredBlurMode;
            internal byte blurCapability;
            internal byte highSpeedSystem;
            internal byte scratchPresent;
            internal byte vgaResolution;
            internal byte spatialSmoothing;
            internal byte minimapEnabled;
            internal byte topBarEnabled;
            internal byte menuOverlay;
            internal byte gamePaused;
            internal byte levelEnd;
            internal byte objectiveSelector;
            internal byte objectiveTextbox;
            internal byte tutorialOrBriefing;
            internal byte helpPopup;
            internal byte debugText;
            internal uint reserved1;
            internal uint reserved2;

            internal static PresentationFrameQuery Create(
                ulong generation,
                ulong tick)
            {
                return new PresentationFrameQuery
                {
                    abiVersion = AbiVersion,
                    structSize = 64,
                    generation = generation,
                    tick = tick,
                    presentationRevision = 1,
                    viewportWidth = TerrainFrameWidth,
                    viewportHeight = TerrainFrameHeight,
                    screenStride = TerrainFrameWidth,
                    highSpeedSystem = 1,
                    scratchPresent = 1,
                    vgaResolution = 1,
                    minimapEnabled = 1,
                    topBarEnabled = 1,
                    objectiveSelector = byte.MaxValue
                };
            }

            internal static PresentationFrameQuery CampaignSubset(
                ulong generation,
                ulong tick)
            {
                return new PresentationFrameQuery
                {
                    abiVersion = AbiVersion,
                    structSize = 64,
                    generation = generation,
                    tick = tick,
                    presentationRevision = tick,
                    viewportWidth = TerrainFrameWidth,
                    viewportHeight = TerrainFrameHeight,
                    screenStride = TerrainFrameWidth,
                    vgaResolution = 1
                };
            }
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct Level1PresentationInput
        {
            internal uint abiVersion;
            internal uint structSize;
            internal uint authorityMask;
            internal byte selectedMenuItem;
            internal byte pauseMenuSuppressed;
            internal byte pauseTextSuppressed;
            internal byte levelEndTextSuppressed;
            internal byte recursivePauseWriter;
            internal byte loadAvailable;
            internal byte soundAvailable;
            internal byte musicAvailable;
            internal uint optionsEnabledMask;
            internal uint optionsSelectedMask;
            internal byte soundVolume;
            internal byte musicVolume;
            internal byte volumeTarget;
            internal byte chatRecipient;
            internal byte chatMode;
            internal byte chatPlayerMask;
            internal byte localPlayerIndex;
            internal byte frameTimingIndex;
            internal byte cursorVisible;
            internal byte reserved0;
            internal short mouseX;
            internal short mouseY;
            internal short joystick;
            internal byte textColourPhase;
            internal byte reserved1;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 8 * 64)]
            internal byte[] chatLines;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 8 * 16)]
            internal byte[] chatNames;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 16)]
            internal byte[] notificationPlayerName;
            internal uint debugClockBucket;
            internal short soundNumber;
            internal ushort reserved2;
            internal uint debugHeapSize;
            internal uint debugMemoryUsed;
            internal uint debugMemoryFree;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)]
            internal uint[] reserved;

            internal static Level1PresentationInput Create()
            {
                return new Level1PresentationInput
                {
                    abiVersion = AbiVersion,
                    structSize = 740,
                    chatLines = new byte[8 * 64],
                    chatNames = new byte[8 * 16],
                    notificationPlayerName = new byte[16],
                    reserved = new uint[4]
                };
            }
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct PresentationFrameInfo
        {
            internal uint abiVersion;
            internal uint structSize;
            internal ulong generation;
            internal ulong tick;
            internal ulong presentationRevision;
            internal ulong immutableInputHash;
            internal ushort width;
            internal ushort height;
            internal ushort stride;
            internal ushort viewportWidth;
            internal uint indexBytes;
            internal uint provenanceBytes;
            internal uint flags;
            internal byte effectivePostprocessMode;
            internal byte historyState;
            internal byte newlyCommitted;
            internal byte reserved0;
            internal ulong indexHash;
            internal ulong provenanceHash;
            internal ulong lightingHash;
            internal ulong sourceArenaHash;
            internal uint overlayPixelsWritten;
            internal ushort overlayStageCount;
            internal ushort reserved1;
            internal uint reserved2;
            internal uint reserved3;
            internal uint reserved4;
            internal uint reserved5;
            internal uint reserved6;
            internal uint reserved7;
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
            internal byte pooledFlushRank;
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
        internal static extern int mc_native_campaign_session_open(
            byte[] packageDir,
            uint packageDirLength,
            byte[] closurePath,
            uint closurePathLength,
            ref CampaignSelection selection,
            out ulong handle);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_session_open_with_asset_cache(
            byte[] packageDir,
            uint packageDirLength,
            byte[] closurePath,
            uint closurePathLength,
            byte[] assetCachePath,
            uint assetCachePathLength,
            ref CampaignSelection selection,
            out ulong handle);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_session_destroy(ulong handle);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_session_reset(ulong handle);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_shield_state(
            ulong handle,
            ulong generation,
            out CampaignShieldSnapshot state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_shield_activate(
            ulong handle,
            ulong generation,
            ushort directionMask,
            byte helpEnabled,
            out CampaignShieldSnapshot state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_shield_damage_submit(
            ulong handle,
            ulong generation,
            ushort targetEntityIndex,
            int damage,
            ushort attackerEntityIndex);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_invisible_state(
            ulong handle,
            ulong generation,
            out CampaignInvisibleSnapshot state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_invisible_activate(
            ulong handle,
            ulong generation,
            ushort directionMask,
            byte helpEnabled,
            out CampaignInvisibleSnapshot state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_beyond_sight_state(
            ulong handle,
            ulong generation,
            out CampaignBeyondSightSnapshot state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_beyond_sight_activate(
            ulong handle,
            ulong generation,
            ushort directionMask,
            byte helpEnabled,
            out CampaignBeyondSightSnapshot state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_opponent_state(
            ulong handle,
            ulong generation,
            out CampaignOpponentSnapshot state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_day_hud_state(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            ref CampaignDayHudSnapshot state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_day_music_state(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            ref CampaignDayMusicSnapshot state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_day_music_advance(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            uint callbackCount,
            ref CampaignDayMusicSnapshot state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_day_music_stop(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            ref CampaignDayMusicSnapshot state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_day_music_end(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            ref CampaignDayMusicSnapshot state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_minimap_state(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            ref CampaignMinimapState state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_minimap_labels_json(
            ulong handle, ulong generation, ulong expectedTick,
            IntPtr buffer, uint capacity, out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_minimap_entities_copy(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            uint startOrdinal,
            [Out] CampaignMinimapEntity[] buffer,
            uint capacity,
            uint elementSize,
            out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_minimap_marks_copy(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            uint startOrdinal,
            [Out] CampaignMinimapMark[] buffer,
            uint capacity,
            uint elementSize,
            out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_session_step(
            ulong handle,
            ref CampaignStepInput input,
            out CampaignStepSnapshot snapshot,
            uint snapshotSize,
            [Out] CampaignEvent[] events,
            uint eventCapacity,
            uint eventElementSize,
            out uint eventCount);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_events_copy_since(
            ulong handle,
            ref EventQuery query,
            [Out] NativeEvent[] buffer,
            uint capacity,
            uint elementSize,
            out EventBatch batch,
            uint batchSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_presentation_frame_copy(
            ulong handle,
            ref PresentationFrameQuery query,
            [Out] byte[] indexBuffer,
            uint indexCapacity,
            IntPtr provenanceBuffer,
            uint provenanceCapacity,
            out PresentationFrameInfo info,
            uint infoSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_economy_events(
            ulong handle,
            [Out] CampaignEconomyEvent[] events,
            uint eventCapacity,
            uint eventElementSize,
            out uint eventCount);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_authored_group_events(
            ulong handle,
            [Out] CampaignAuthoredGroupEvent[] events,
            uint eventCapacity,
            uint eventElementSize,
            out uint eventCount);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_goat_events(
            ulong handle,
            [Out] CampaignGoatEvent[] events,
            uint eventCapacity,
            uint eventElementSize,
            out uint eventCount);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_world_info(
            ulong handle,
            out CampaignWorldInfo info,
            uint infoSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_terrain_state(
            ulong handle,
            ulong generation,
            out CampaignTerrainState state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_day_indexed_frame_render(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            ref CampaignDayIndexedFrameInfo info,
            uint infoSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_day_indexed_frame_copy(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            [Out] byte[] indexBuffer,
            uint indexCapacity,
            [Out] byte[] paletteBuffer,
            uint paletteCapacity,
            out uint indexWritten,
            out uint paletteWritten);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_scene_frame_copy(
            ulong handle, ulong generation, ulong expectedTick,
            [Out] byte[] indices, uint capacity, out uint written,
            out ulong revision, out byte blurMode, out byte useExitTables,
            out byte sourceSpritesComposited);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_world_map_copy(
            ulong handle, ulong generation, ulong expectedTick,
            [Out] byte[] indices, uint indexCapacity,
            [Out] byte[] palette, uint paletteCapacity,
            [Out] CampaignMinimapEntity[] entities, uint entityCapacity,
            [Out] CampaignMinimapMark[] marks, uint markCapacity,
            ref CampaignMinimapState state, uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_world_map_labels_json(
            ulong handle, ulong generation, ulong expectedTick,
            IntPtr buffer, uint capacity, out uint length);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_cave_occlusion_copy(
            ulong handle, ulong generation, ulong expectedTick,
            [Out] ushort[] terrainOrders, uint terrainCapacity,
            [Out] ushort[] cellOrders, uint cellCapacity,
            out uint terrainWritten, out uint cellWritten);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_terrain_plane_length(
            ulong handle,
            ulong generation,
            uint kind,
            out uint count);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_terrain_plane_copy_u8(
            ulong handle,
            ulong generation,
            uint kind,
            [Out] byte[] buffer,
            uint capacity,
            out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_terrain_entity_indices_copy(
            ulong handle,
            ulong generation,
            [Out] short[] buffer,
            uint capacity,
            out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_objective_stage_state(
            ulong handle,
            ulong generation,
            byte objectiveSlot,
            byte stageSlot,
            out CampaignObjectiveStageSnapshot state,
            uint stateSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_player_count(
            ulong handle,
            out uint count);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_entity_count(
            ulong handle,
            out uint count);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_players_copy(
            ulong handle,
            uint startSlot,
            [Out] CampaignPlayerSnapshot[] buffer,
            uint capacity,
            uint elementSize,
            out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_entities_copy(
            ulong handle,
            uint startIndex,
            [Out] CampaignEntitySnapshot[] buffer,
            uint capacity,
            uint elementSize,
            out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_entity_presentations_copy(
            ulong handle,
            ulong generation,
            ulong tick,
            short cameraYaw,
            [Out] CampaignEntityPresentation[] buffer,
            uint capacity,
            uint elementSize,
            out uint written);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_goats_copy(
            ulong handle,
            ulong generation,
            uint startOrdinal,
            [Out] CampaignGoatSnapshot[] buffer,
            uint capacity,
            uint elementSize,
            out GoatBatch batch,
            uint batchSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_goat_damage_submit(
            ulong handle,
            ulong generation,
            ushort targetEntityIndex,
            int damage,
            ushort attackerEntityIndex);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_spell_pickups_copy(
            ulong handle,
            ulong generation,
            uint startOrdinal,
            [Out] CampaignSpellPickupSnapshot[] buffer,
            uint capacity,
            uint elementSize,
            out GoatBatch batch,
            uint batchSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_mana_spheres_copy(
            ulong handle,
            ulong generation,
            uint startOrdinal,
            [Out] ManaSphereSnapshot[] buffer,
            uint capacity,
            uint elementSize,
            out GoatBatch batch,
            uint batchSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_campaign_mana_carriers_copy(
            ulong handle,
            ulong generation,
            uint startOrdinal,
            [Out] ManaCarrierSnapshot[] buffer,
            uint capacity,
            uint elementSize,
            out GoatBatch batch,
            uint batchSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_session_destroy(ulong handle);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_contract_json_v1(
            IntPtr buffer,
            uint capacity,
            out uint length);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_create_v1(out ulong handle);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_identity_json_v1(
            ulong handle,
            IntPtr buffer,
            uint capacity,
            out uint length);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_bootstrap_package_set_json_v1(
            ulong handle,
            ulong generation,
            byte[] requestJson,
            uint requestJsonLength);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_save_replay_json_v1(
            ulong handle,
            ulong runtimeGeneration,
            ulong sessionGeneration,
            ulong expectedTick,
            IntPtr buffer,
            uint capacity,
            out uint length);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_load_replay_json_v1(
            ulong handle,
            ulong generation,
            byte[] packageSetJson,
            uint packageSetJsonLength,
            byte[] saveJson,
            uint saveJsonLength);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_load_replay_from_runtime_json_v1(
            ulong handle, ulong generation,
            ulong sourceHandle, ulong sourceGeneration,
            byte[] packageSetJson, uint packageSetJsonLength,
            byte[] saveJson, uint saveJsonLength);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_open_level_v1(
            ulong handle,
            ulong generation);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_restart_new_game_v1(
            ulong handle,
            ulong generation);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_pause_v1(
            ulong handle,
            ulong generation);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_paused_frame_v1(
            ulong handle,
            ulong runtimeGeneration,
            ulong sessionGeneration,
            ulong expectedTick);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_paused_frame_timed_v1(
            ulong handle, ulong runtimeGeneration, ulong sessionGeneration,
            ulong expectedTick, uint elapsedCentiseconds, byte showObjective);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_presentation_frame_v1(
            ulong handle, ulong runtimeGeneration, ulong sessionGeneration,
            ulong expectedTick, byte notificationsVisible);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_participant_notifications_json_v1(
            ulong handle, IntPtr buffer, uint capacity, out uint length);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_resume_v1(
            ulong handle,
            ulong generation);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_accept_transition_v1(
            ulong handle,
            ulong runtimeGeneration,
            ulong sessionGeneration,
            ulong expectedTick);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_campaign_menu_command_json_v1(
            ulong handle, ulong generation, byte[] request, uint requestLength);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_finish_mission_if_ready_v1(
            ulong handle, ulong runtimeGeneration, ulong sessionGeneration, ulong expectedTick);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_completion_transition_receipt_json_v1(
            ulong handle,
            IntPtr buffer,
            uint capacity,
            out uint length);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_boundary_transition_receipt_json_v1(
            ulong handle,
            IntPtr buffer,
            uint capacity,
            out uint length);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_active_entry_presentation_json_v1(
            ulong handle,
            ulong generation,
            IntPtr buffer,
            uint capacity,
            out uint length);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_close_v1(
            ulong handle,
            ulong runtimeGeneration,
            ulong sessionGeneration,
            ulong expectedTick);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_destroy_v1(ulong handle);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_session_contract_json_v1(
            IntPtr buffer,
            uint capacity,
            out uint length);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_session_open_v1(
            byte[] packageDir,
            uint packageDirLength,
            byte[] closurePath,
            uint closurePathLength,
            byte[] assetCachePath,
            uint assetCachePathLength,
            out ulong handle);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_session_open_workspace_v1(
            byte[] packageDir,
            uint packageDirLength,
            byte[] closurePath,
            uint closurePathLength,
            byte[] assetCachePath,
            uint assetCachePathLength,
            byte[] workspacePath,
            uint workspacePathLength,
            out ulong handle);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_session_destroy_v1(ulong handle);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_session_snapshot_json_v1(
            ulong handle,
            IntPtr buffer,
            uint capacity,
            out uint length);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_session_step_json_v1(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            byte[] commandJson,
            uint commandJsonLength);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_session_reset_v1(
            ulong handle,
            ulong generation,
            ulong expectedTick);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_session_events_json_v1(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            ulong cursor,
            uint limit,
            IntPtr buffer,
            uint capacity,
            out uint length);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_session_audio_json_v1(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            ulong eventSequence,
            IntPtr buffer,
            uint capacity,
            out uint length);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_session_presentation_asset_json_v1(
            ulong handle,
            ulong generation,
            ulong expectedTick,
            byte[] semanticId,
            uint semanticIdLength,
            IntPtr buffer,
            uint capacity,
            out uint length);

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
        internal static extern int mc_native_session_presentation_frame_copy(
            ulong handle,
            ref PresentationFrameQuery query,
            [Out] byte[] indexBuffer,
            uint indexCapacity,
            [Out] byte[] provenanceBuffer,
            uint provenanceCapacity,
            out PresentationFrameInfo info,
            uint infoSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_level1_presentation_input(
            ulong handle,
            ref Level1PresentationInput input);

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
        internal static extern int mc_native_level1_player_lifecycle_snapshot(
            ulong handle,
            out Level1PlayerLifecycleSnapshot snapshot,
            uint snapshotSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_level1_player_menu_state(
            ulong handle,
            byte menuState);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_level1_game_paused(
            ulong handle,
            byte paused);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_level1_player_inject_damage_for_qualification(
            ulong handle,
            int damage,
            ushort attackerEntityIndex);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_level1_hud_snapshot(
            ulong handle,
            out Level1HudSnapshot snapshot,
            uint snapshotSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_level1_spell_menu_snapshot(
            ulong handle,
            out Level1SpellMenuSnapshot snapshot,
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
        internal static extern int mc_game_runtime_show_new_game_map_v1(ulong handle, ulong generation);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_audio_command_for_event_v1(
            ulong handle, ulong generation, ulong eventSequence,
            out AudioCommand command, uint commandSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_audio_command_release_v1(
            ulong handle, ulong generation, ulong eventSequence);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_audio_command_discard_v1(
            ulong handle, ulong generation, ulong eventSequence);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_game_runtime_audio_voice_complete_v1(
            ulong handle, ulong generation, ushort voiceIndex, ulong eventSequence);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_audio_command_for_event(
            ulong handle,
            ulong generation,
            ulong eventSequence,
            out AudioCommand command,
            uint commandSize);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_audio_command_release(
            ulong handle,
            ulong generation,
            ulong eventSequence);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
        internal static extern int mc_native_audio_command_discard(
            ulong handle,
            ulong generation,
            ulong eventSequence);

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
            RequireSize<CampaignSelection>(32);
            RequireSize<CampaignStepInput>(80);
            RequireSize<CampaignEvent>(48);
            RequireSize<CampaignAuthoredGroupEvent>(48);
            RequireSize<CampaignEconomyEvent>(64);
            RequireSize<CampaignGoatEvent>(64);
            RequireSize<CampaignStepSnapshot>(112);
            RequireSize<CampaignWorldInfo>(64);
            RequireSize<CampaignTerrainState>(64);
            RequireSize<CampaignDayIndexedFrameInfo>(152);
            RequireSize<CampaignObjectiveStageSnapshot>(48);
            RequireSize<CampaignPlayerSnapshot>(48);
            RequireSize<CampaignEntitySnapshot>(52);
            RequireSize<CampaignEntityPresentation>(64);
            RequireSize<CampaignCharacterState>(80);
            RequireSize<VoxelState>(64);
            RequireOffset<VoxelState>(nameof(VoxelState.worldGeneration), 56);
            RequireSize<VoxelImpactChild>(32);
            RequireOffset<VoxelImpactChild>(nameof(VoxelImpactChild.sequence), 16);
            RequireOffset<VoxelImpactChild>(nameof(VoxelImpactChild.slotGeneration), 24);
            RequireOffset<VoxelImpactChild>(nameof(VoxelImpactChild.slot), 28);
            RequireSize<CampaignVoxelDeath>(32);
            RequireOffset<CampaignVoxelDeath>(nameof(CampaignVoxelDeath.sequence), 16);
            RequireOffset<CampaignCharacterState>(nameof(CampaignCharacterState.x), 48);
            RequireOffset<CampaignCharacterState>(nameof(CampaignCharacterState.life), 60);
            RequireSize<CampaignShieldSnapshot>(72);
            RequireSize<CampaignInvisibleSnapshot>(80);
            RequireSize<CampaignBeyondSightSnapshot>(80);
            RequireSize<CampaignOpponentSnapshot>(104);
            RequireSize<CampaignDayHudSpellSlot>(48);
            RequireSize<CampaignDayHudSnapshot>(272);
            RequireSize<CampaignDayMusicSnapshot>(80);
            RequireSize<CampaignMinimapState>(72);
            RequireSize<CampaignMinimapEntity>(56);
            RequireSize<CampaignMinimapMark>(48);
            RequireSize<Command>(32);
            RequireSize<CameraInput>(28);
            RequireSize<SessionSnapshot>(120);
            RequireSize<ObjectiveSnapshot>(40);
            RequireSize<EntitySnapshot>(52);
            RequireSize<GoatSnapshot>(84);
            RequireSize<CampaignGoatSnapshot>(84);
            RequireSize<CampaignSpellPickupSnapshot>(64);
            RequireSize<GoatBatch>(64);
            RequireSize<Level1MissionSnapshot>(96);
            RequireSize<Level1PlayerLifecycleSnapshot>(144);
            RequireSize<Level1HudSpellSlot>(36);
            RequireSize<Level1HudSnapshot>(128);
            RequireSize<Level1SpellMenuSnapshot>(64);
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
            RequireSize<PresentationFrameQuery>(64);
            RequireSize<Level1PresentationInput>(740);
            RequireSize<PresentationFrameInfo>(128);
            RequireSize<AssetInfo>(32);
            RequireSize<EventQuery>(40);
            RequireSize<EventBatch>(56);
            RequireSize<NativeEvent>(72);
            RequireSize<AudioCommand>(88);
            RequireOffset<WorldInfo>(nameof(WorldInfo.resetStateHash), 40);
            RequireOffset<CampaignSelection>(nameof(CampaignSelection.reserved), 16);
            RequireOffset<CampaignStepInput>(nameof(CampaignStepInput.command), 16);
            RequireOffset<CampaignEvent>(nameof(CampaignEvent.sourceSlot), 24);
            RequireOffset<CampaignAuthoredGroupEvent>(nameof(CampaignAuthoredGroupEvent.authoredIndex), 28);
            RequireOffset<CampaignEconomyEvent>(nameof(CampaignEconomyEvent.primaryId), 28);
            RequireOffset<CampaignEconomyEvent>(nameof(CampaignEconomyEvent.value0), 40);
            RequireOffset<CampaignGoatEvent>(nameof(CampaignGoatEvent.entity0), 28);
            RequireOffset<CampaignGoatEvent>(nameof(CampaignGoatEvent.value0), 32);
            RequireOffset<CampaignGoatEvent>(nameof(CampaignGoatEvent.soundId), 40);
            RequireOffset<CampaignGoatEvent>(nameof(CampaignGoatEvent.assetRefIndex), 52);
            RequireOffset<CampaignStepSnapshot>(nameof(CampaignStepSnapshot.dispatchHash), 40);
            RequireOffset<CampaignWorldInfo>(nameof(CampaignWorldInfo.resetStateHash), 40);
            RequireOffset<CampaignTerrainState>(nameof(CampaignTerrainState.mutationSequence), 24);
            RequireOffset<CampaignTerrainState>(nameof(CampaignTerrainState.width), 40);
            RequireOffset<CampaignTerrainState>(nameof(CampaignTerrainState.reserved), 48);
            RequireOffset<CampaignDayIndexedFrameInfo>(
                nameof(CampaignDayIndexedFrameInfo.indexBytes), 32);
            RequireOffset<CampaignDayIndexedFrameInfo>(
                nameof(CampaignDayIndexedFrameInfo.indexHash), 40);
            RequireOffset<CampaignDayIndexedFrameInfo>(
                nameof(CampaignDayIndexedFrameInfo.packageSha256), 72);
            RequireOffset<CampaignDayIndexedFrameInfo>(
                nameof(CampaignDayIndexedFrameInfo.cacheContentSha256), 104);
            RequireOffset<CampaignDayIndexedFrameInfo>(
                nameof(CampaignDayIndexedFrameInfo.reserved), 136);
            RequireOffset<CampaignObjectiveStageSnapshot>(
                nameof(CampaignObjectiveStageSnapshot.axisX), 24);
            RequireOffset<CampaignObjectiveStageSnapshot>(
                nameof(CampaignObjectiveStageSnapshot.relationValue), 30);
            RequireOffset<CampaignObjectiveStageSnapshot>(
                nameof(CampaignObjectiveStageSnapshot.reserved), 32);
            RequireOffset<CampaignPlayerSnapshot>(nameof(CampaignPlayerSnapshot.turn), 16);
            RequireOffset<CampaignEntitySnapshot>(nameof(CampaignEntitySnapshot.reserved), 36);
            RequireOffset<CampaignEntityPresentation>(nameof(CampaignEntityPresentation.entityIndex), 24);
            RequireOffset<CampaignEntityPresentation>(nameof(CampaignEntityPresentation.x), 40);
            RequireOffset<CampaignEntityPresentation>(nameof(CampaignEntityPresentation.pixelMode), 48);
            RequireOffset<CampaignEntityPresentation>(nameof(CampaignEntityPresentation.brightness), 50);
            RequireOffset<CampaignEntityPresentation>(nameof(CampaignEntityPresentation.drawOrder), 52);
            RequireOffset<CampaignEntityPresentation>(nameof(CampaignEntityPresentation.controlFlags), 56);
            RequireOffset<CampaignEntityPresentation>(nameof(CampaignEntityPresentation.primaryDepthAdjustment), 60);
            RequireOffset<CampaignEntityPresentation>(nameof(CampaignEntityPresentation.reserved), 62);
            RequireOffset<CampaignShieldSnapshot>(nameof(CampaignShieldSnapshot.parentStatusFlags), 48);
            RequireOffset<CampaignInvisibleSnapshot>(nameof(CampaignInvisibleSnapshot.parentStatusFlags), 52);
            RequireOffset<CampaignBeyondSightSnapshot>(nameof(CampaignBeyondSightSnapshot.experienceAwarded), 52);
            RequireOffset<CampaignOpponentSnapshot>(nameof(CampaignOpponentSnapshot.entityControlFlags), 36);
            RequireOffset<CampaignOpponentSnapshot>(nameof(CampaignOpponentSnapshot.objectiveRelationValue), 76);
            RequireOffset<CampaignOpponentSnapshot>(nameof(CampaignOpponentSnapshot.visibilityFlags), 80);
            RequireOffset<CampaignDayHudSpellSlot>(nameof(CampaignDayHudSpellSlot.maxMana), 20);
            RequireOffset<CampaignDayHudSpellSlot>(nameof(CampaignDayHudSpellSlot.drawAdmitted), 37);
            RequireOffset<CampaignDayHudSnapshot>(nameof(CampaignDayHudSnapshot.globalManaDenominator), 52);
            RequireOffset<CampaignDayHudSnapshot>(nameof(CampaignDayHudSnapshot.playerPanelSelector), 62);
            RequireOffset<CampaignDayHudSnapshot>(nameof(CampaignDayHudSnapshot.lifePaletteIndex), 68);
            RequireOffset<CampaignDayHudSnapshot>(nameof(CampaignDayHudSnapshot.hitFrameTimePreDraw), 27);
            RequireOffset<CampaignDayHudSnapshot>(nameof(CampaignDayHudSnapshot.colorPhase1), 30);
            RequireOffset<CampaignDayHudSnapshot>(nameof(CampaignDayHudSnapshot.colorPhase2), 31);
            RequireOffset<CampaignDayHudSnapshot>(nameof(CampaignDayHudSnapshot.nyphur), 72);
            RequireOffset<CampaignDayHudSnapshot>(nameof(CampaignDayHudSnapshot.left), 176);
            RequireOffset<CampaignDayHudSnapshot>(nameof(CampaignDayHudSnapshot.right), 224);
            RequireOffset<CampaignDayMusicSnapshot>(
                nameof(CampaignDayMusicSnapshot.initialized), 48);
            RequireOffset<CampaignDayMusicSnapshot>(
                nameof(CampaignDayMusicSnapshot.timerHz), 60);
            RequireOffset<CampaignDayMusicSnapshot>(
                nameof(CampaignDayMusicSnapshot.stateHash), 64);
            RequireOffset<CampaignMinimapState>(nameof(CampaignMinimapState.entityCount), 28);
            RequireOffset<CampaignMinimapState>(nameof(CampaignMinimapState.entityHash), 40);
            RequireOffset<CampaignMinimapEntity>(nameof(CampaignMinimapEntity.sourceOrder), 24);
            RequireOffset<CampaignMinimapEntity>(nameof(CampaignMinimapEntity.x), 28);
            RequireOffset<CampaignMinimapEntity>(nameof(CampaignMinimapEntity.selector), 32);
            RequireOffset<CampaignMinimapEntity>(nameof(CampaignMinimapEntity.helpSelector), 38);
            RequireOffset<CampaignMinimapMark>(nameof(CampaignMinimapMark.sourceOrder), 24);
            RequireOffset<CampaignMinimapMark>(nameof(CampaignMinimapMark.x), 28);
            RequireOffset<CampaignMinimapMark>(nameof(CampaignMinimapMark.paletteIndex), 32);
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
            RequireOffset<CampaignGoatSnapshot>(nameof(CampaignGoatSnapshot.partnerEntityIndex), 68);
            RequireOffset<CampaignSpellPickupSnapshot>(
                nameof(CampaignSpellPickupSnapshot.sourceEntityIndex), 8);
            RequireOffset<CampaignSpellPickupSnapshot>(nameof(CampaignSpellPickupSnapshot.x), 16);
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
                nameof(Level1MissionSnapshot.possessionExperience), 84);
            RequireOffset<Level1MissionSnapshot>(
                nameof(Level1MissionSnapshot.lastPossessedBuildingEntity), 88);
            RequireOffset<Level1MissionSnapshot>(
                nameof(Level1MissionSnapshot.possessedBuildingCount), 92);
            RequireOffset<Level1MissionSnapshot>(
                nameof(Level1MissionSnapshot.speedTrailCount), 95);
            RequireOffset<Level1PlayerLifecycleSnapshot>(
                nameof(Level1PlayerLifecycleSnapshot.playerAction), 41);
            RequireOffset<Level1PlayerLifecycleSnapshot>(
                nameof(Level1PlayerLifecycleSnapshot.actionTimer), 56);
            RequireOffset<Level1PlayerLifecycleSnapshot>(
                nameof(Level1PlayerLifecycleSnapshot.playerControlFlags), 72);
            RequireOffset<Level1PlayerLifecycleSnapshot>(
                nameof(Level1PlayerLifecycleSnapshot.sourcePoolRebuildCount), 108);
            RequireOffset<Level1PlayerLifecycleSnapshot>(
                nameof(Level1PlayerLifecycleSnapshot.markerDispatchPhase), 122);
            RequireOffset<Level1PlayerLifecycleSnapshot>(
                nameof(Level1PlayerLifecycleSnapshot.sourceTurnCounter), 128);
            RequireOffset<Level1PlayerLifecycleSnapshot>(
                nameof(Level1PlayerLifecycleSnapshot.effectivePitch), 134);
            RequireOffset<Level1PlayerLifecycleSnapshot>(
                nameof(Level1PlayerLifecycleSnapshot.markerConstructorPhase), 136);
            RequireOffset<Level1HudSpellSlot>(
                nameof(Level1HudSpellSlot.maxMana), 12);
            RequireOffset<Level1HudSnapshot>(
                nameof(Level1HudSnapshot.globalManaDenominator), 32);
            RequireOffset<Level1HudSnapshot>(
                nameof(Level1HudSnapshot.left), 48);
            RequireOffset<Level1HudSnapshot>(
                nameof(Level1HudSnapshot.right), 84);
            RequireOffset<Level1HudSnapshot>(
                nameof(Level1HudSnapshot.selectedHolderEntityIndex), 120);
            RequireOffset<Level1SpellMenuSnapshot>(
                nameof(Level1SpellMenuSnapshot.familyEnabled), 32);
            RequireOffset<Level1SpellMenuSnapshot>(
                nameof(Level1SpellMenuSnapshot.holderEntityIndex), 48);
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
                nameof(Level1ActorSnapshot.stateValue), 48);
            RequireOffset<Level1ActorSnapshot>(
                nameof(Level1ActorSnapshot.reserved), 52);
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
            RequireOffset<PresentationFrameQuery>(
                nameof(PresentationFrameQuery.configuredBlurMode), 40);
            RequireOffset<Level1PresentationInput>(
                nameof(Level1PresentationInput.mouseX), 38);
            RequireOffset<Level1PresentationInput>(
                nameof(Level1PresentationInput.debugClockBucket), 704);
            RequireOffset<PresentationFrameInfo>(
                nameof(PresentationFrameInfo.immutableInputHash), 32);
            RequireOffset<PresentationFrameInfo>(
                nameof(PresentationFrameInfo.indexHash), 64);
            RequireOffset<PresentationFrameInfo>(
                nameof(PresentationFrameInfo.reserved2), 104);
            RequireOffset<NativeEvent>(nameof(NativeEvent.stateHash), 24);
            RequireOffset<NativeEvent>(nameof(NativeEvent.reserved), 56);
            RequireOffset<AudioCommand>(nameof(AudioCommand.playbackRateHz), 48);
            RequireOffset<AudioCommand>(nameof(AudioCommand.soundId), 52);
            RequireOffset<AudioCommand>(nameof(AudioCommand.volumeQ15), 60);
            RequireOffset<AudioCommand>(nameof(AudioCommand.playType), 68);
            RequireOffset<AudioCommand>(nameof(AudioCommand.pooledFlushRank), 71);
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
