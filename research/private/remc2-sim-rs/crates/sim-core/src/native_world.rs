//! Strict owner-package loader for an immutable native simulation world.
//!
//! The loader consumes an explicit level-export directory and an explicit
//! asset-closure path. It never searches for owner data, embeds regression
//! bytes, or treats observed entities as simulated entities.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    path::{Component, Path, PathBuf},
    sync::OnceLock,
};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    FlightContext, FlightEffectsContext, FlightEvent, FlightProfile, FlightState, FlightWorld,
    PlayerCommand, Position, SOURCE_SPELL_INDEX_MAP, SourceAudioCallTag, SpellFireContext,
    archer_tmap::{ArcherTmapBank, ArcherTmapClocks},
    combat_tmap::{CombatTmapBank, FireballTmapClock, PossessionTmapClock},
    conformance_hash,
    economy_tmap::{CastleTmapClock, EconomyTmapBank},
    entity_passes_for_speed_index,
    firefly_tmap::{FireflyTmapBank, FireflyTmapClocks},
    frozen_actor_tmap::{FrozenActorTmapBank, VillagerTmapClocks},
    goat::{
        GOAT_BASE_TMAP, GOAT_CLASS, GOAT_MODEL, GOAT_PATROL_ACTION, GOAT_RENDER_PARAMETER_INDEX,
        GOAT_STAGE_POINT, GoatBehaviorConfig, GoatPlayerTarget, GoatState, GoatStepEvent,
        LEVEL_ONE_GOAT_INDICES, LiveWorldState,
    },
    goat_tmap::{
        GOAT_TMAP_SELECTORS, GOAT_TMAP_STATE_COUNT, GoatTmapBank, GoatTmapClocks, GoatTmapFrame,
        GoatTmapSelectorBank,
    },
    house::{
        HOUSE_CLASS, HOUSE_MODEL, HouseFamily, HouseResidentEntry, HouseState, HouseStepEvent,
    },
    latch_command_with_context,
    legacy_terrain::{
        LEGACY_MAP_CELLS, LegacyTerrain, interpolate_cave, interpolate_ground, material_mask,
    },
    level1_mission::{
        ARCHER_BASE_TMAP, ARCHER_LOGICAL_HEIGHT, ARCHER_RENDER_INDEX, ARCHER_RENDER_MODE,
        CASTLE_SPELL_FAMILY, CASTLE_STAGE_ONE_COLLISION_HALF_WIDTH, CASTLE_STAGE_ONE_LIFE,
        CASTLE_STAGE_ONE_MANA_CAPACITY, CastleBuildFootprint, CastleTerrainUpdate,
        FIREBALL_BASE_TMAP, FIREBALL_IMPACT_BASE_TMAP, FIREBALL_IMPACT_LOGICAL_HEIGHT,
        FIREBALL_IMPACT_RENDER_INDEX, FIREBALL_IMPACT_RENDER_MODE, FIREBALL_LOGICAL_HEIGHT,
        FIREBALL_RENDER_INDEX, FIREBALL_RENDER_MODE, FIREFLY_BASE_TMAP, FIREFLY_LOGICAL_HEIGHT,
        FIREFLY_RENDER_INDEX, FIREFLY_RENDER_MODE, GROUND_FIRE_BASE_TMAP,
        GROUND_FIRE_FIRST_RENDER_INDEX, GROUND_FIRE_LOGICAL_HEIGHTS,
        GROUND_FIRE_PRESENTATION_BYTE_11, GROUND_FIRE_PRESENTATION_SPEEDS, GROUND_FIRE_RENDER_MODE,
        HouseResidentRequest, Level1AmbientEntity, Level1CastlePlacementBlocker,
        Level1CollisionTarget, Level1MissionState, Level1MissionTemplate,
        Level1ResetRawSourceState, Level1ResetSourceSlot, Level1SourceCollisionTarget,
        Level1SourceEffect, Level1SourceEntityAuthority, Level1SourceEntityView,
        MANA_CARRIER_ATTRACTION_RADIUS, MANA_CARRIER_BASE_TMAP, MANA_CARRIER_COLLISION_HALF_HEIGHT,
        MANA_CARRIER_COLLISION_HALF_WIDTH, MANA_CARRIER_COLLISION_VERTICAL_OFFSET,
        MANA_CARRIER_LOGICAL_HEIGHT, MANA_CARRIER_MAX_LIFE, MANA_CARRIER_MAX_MANA,
        MANA_CARRIER_RENDER_INDEX, MANA_CARRIER_RENDER_MODE, MANA_CARRIER_SPEED,
        MANA_SPHERE_COLLISION_DIMENSIONS, ManaCarrier, ManaSphere, POSSESSION_ATTRACTION_BASE_TMAP,
        POSSESSION_ATTRACTION_LOGICAL_HEIGHT, POSSESSION_ATTRACTION_RENDER_INDEX,
        POSSESSION_ATTRACTION_RENDER_MODE, POSSESSION_IMPACT_BASE_TMAP,
        POSSESSION_IMPACT_LOGICAL_HEIGHT, POSSESSION_IMPACT_RENDER_INDEX,
        POSSESSION_IMPACT_RENDER_MODE, SPEED_SPELL_ACTIVATION_TICKS, SPEED_SPELL_FAMILY,
        SPEED_SPELL_MANA_COST, SPELL_JAR_BASE_TMAP, SPELL_JAR_LOGICAL_HEIGHT,
        SPELL_JAR_RENDER_INDEX, SPELL_JAR_RENDER_MODE, mana_sphere_size_ordinal,
    },
    mark_source_audio_call,
    mission_content::{
        MissionContentProfile, MissionContentSession, MissionContentTemplate, REMC2_SPEC_COMMIT,
    },
    renderable_map_chain::{RenderableMapChains, RenderableMapSeed, raw_source_map_chain_entries},
    source_sprite_tmap::{
        ImportedResetSpriteTuple, SourceSpriteContractRoute, SourceSpriteContractRow,
        SourceSpriteDisposition, SourceSpriteLegacyFamily, SourceSpriteTemplateDisposition,
        SourceSpriteTmapBank, SourceSpriteTmapFrame, imported_reset_sprite_disposition,
        level2_bad_stone_sprite_disposition, source_sprite_dispositions,
        unknown_source_sprite_tuple,
    },
    spell_activation::{SpellActivationHolder, SpellCasterState, activate_spell},
    state_hash,
    static_world_tmap::StaticWorldTmapBank,
    step_entity_body_with_context, step_entity_pass_with_context,
    terrain_frame::{
        Level1CastleRootRender, Level1FrozenActorRender, Level1TerrainFrame,
        Level1TerrainFrameCamera, Level1TerrainFrameResources, Level1WorldFrameResources,
        TerrainFrameError, render_level1_source_world_frame,
    },
    villager::{
        LEVEL_ONE_VILLAGER_INDICES, LEVEL_TWO_VILLAGER_INDICES, VILLAGER_CLASS, VILLAGER_MODEL,
        VillagerBehaviorConfig, VillagerFamily, VillagerState, VillagerStepEffect,
    },
};

const MANIFEST_SCHEMA: &str = "mc2-owner-level-export/v2";
const CLOSURE_SCHEMA: &str = "mc2-level-asset-closure/v2";
const AUDIO_CACHE_SCHEMA: &str = "mc2-owner-unity-asset-cache/v1";
const AUDIO_CACHE_MANIFEST_SHA256: &str =
    "e3aaf1cf03e390850c5da44ed4b3880352b425bd063657e3cdbbcb8086a4d7ee";
const AUDIO_CACHE_CONTENT_SHA256: &str =
    "d299990e300117d065eb3a75008f6a69911b1ba356e4e06452c0a826450a01e5";
const CP1_CACHE_MANIFEST_SHA256: &str =
    "4c399e3f0c351893d94d619401769c01be7bcccc3dd16c5258ce722f2d98925e";
const CP1_CACHE_CONTENT_SHA256: &str =
    "d23f9a0972e910c78756ca4a59d6bc2f8b63d96adf02a695d1402ed9c1141d94";
const CP2_CACHE_MANIFEST_SHA256: &str =
    "328a1e2725b6d32d5f01fbc8117b10014fb8b593955c15382c23a9a5cfcabf17";
const CP2_CACHE_CONTENT_SHA256: &str =
    "99331dafb639a693b16ba9482012e9d197b256ae23619416f6bec78fedb72a2e";
const CP3_CACHE_MANIFEST_SHA256: &str =
    "03217301d152546a4d69e3e99c43d425a5a73030d6b0d8715115fbf45068a1cb";
const CP3_CACHE_CONTENT_SHA256: &str =
    "f3eb5bf87d0fc8f0ea12a986c30c74e97f9f5fa8f48f299ead2233348f2f2faa";
const CP4_CACHE_MANIFEST_SHA256: &str =
    "5b226745eb6d63bb098f41be1d22f6b52eef66c467291c8c09059613e24c55f2";
const CP4_CACHE_CONTENT_SHA256: &str =
    "a2d72aac353be23a2cc598a6db7ac66f059b1aed86eae4f0f3237cf897e8ca16";
const CP4_CORRECTION1_CACHE_MANIFEST_SHA256: &str =
    "1729afa00604b64960e73acbf18d93c00c89d67949033380d57534ffeb6e62e3";
const CP4_CORRECTION1_CACHE_CONTENT_SHA256: &str =
    "fc86d3ec774427441563a37c7f8d62f23c4585d66f910842fef516c326dfe47e";
const CP4_CORRECTION2_CACHE_MANIFEST_SHA256: &str =
    "3909be6b29c1f879208f0a15ec2bccee58d65311cefd6698f0870d16c9742a8d";
const CP4_CORRECTION2_CACHE_CONTENT_SHA256: &str =
    "ee063ce1d5f804c8efc412dff2bedb0800d6fd347b38a6d0376e2a2e4dccd9dc";
const CP4_CORRECTION6_CACHE_MANIFEST_SHA256: &str =
    "474d4b24c157896ce98f0a8baa1593f98762fc314f081600c44a94f97743fad8";
const CP4_CORRECTION6_CACHE_CONTENT_SHA256: &str =
    "a1dc7c3fca20da019807093530c601b872c3d81284a19d17d5e1aa1bccafd752";
const PLAN29_FINAL_CACHE_MANIFEST_SHA256: &str =
    "1050b4f1e3f0e63fa292fe7798d19f4b390c063aa5afd5a350fba9e300298082";
const PLAN29_FINAL_CACHE_CONTENT_SHA256: &str =
    "b664ad00c94e3f9393bc92ecac91a821673222cd365ed47617c4fab63db733bf";
pub const FINAL_LEVEL1_CACHE_DIRECTORY: &str = "level1-unity-asset-cache-plan29-final";
const RESET_SCHEMA: &str = "mc2-flight-reset-projection/v1";
const ENTITY_COUNT: usize = 1_000;
const ENTITY_COUNT_U32: u32 = 1_000;
const ENTITY_RECORD_BYTES: usize = 176;
const ENTITY_RECORD_BYTES_U16: u16 = 176;
const PLAYER_COUNT: usize = 8;
const PLAYER_COUNT_I16: i16 = 8;
const PLAYER_COUNT_U32: u32 = 8;
const PLAYER_RECORD_BYTES: usize = 573;
const PLAYER_RECORD_BYTES_U16: u16 = 573;
const AUTHORED_LEVEL_BYTES: usize = 0x6604;
const TERRAIN_BYTES: usize = LEGACY_MAP_CELLS * 7;
const RUNTIME_BYTES: usize = 2_754;
const PARTICLE_COUNT: usize = 347;
const PARTICLE_COUNT_U32: u32 = 347;
const PARTICLE_RECORD_BYTES: usize = 14;
const LEVEL_SPRITE_SELECTOR_COUNT: usize = 504;
const LEVEL_SPRITE_SELECTOR_COUNT_U32: u32 = 504;
const ENTITY_PARAMETER_COUNT: usize = 157;
const ENTITY_PARAMETER_COUNT_U32: u32 = 157;
const ENTITY_PARAMETER_BYTES: usize = 34;
const PLAYER_SPELL_COUNT: usize = 26;
const PLAYER_SPELL_COUNT_I16: i16 = 26;
const FRAME_ASSET_COUNT: usize = 76;
const LEVEL_ONE_CONTROL_MODE_SEAM_DEFAULT: u8 = 0;
const LEVEL2_UPSTREAM_COMMIT: &str = "6806c7204d17220ecc0430759f784aea8ee2ddb2";
const LEVEL2_BAD_STONE_SLOTS: [u16; 4] = [33, 34, 35, 36];
const LEVEL2_BAD_STONE_PARAMETER: u16 = 60;
const LEVEL2_BAD_STONE_PARTICLE: u16 = 270;
const LEVEL2_BAD_STONE_CLASS: u8 = 2;
const LEVEL2_BAD_STONE_MODEL: u8 = 3;
const LEVEL2_BAD_STONE_ACTION: u8 = 9;
const LEVEL2_BAD_STONE_RECORD_SHA256: [&str; 4] = [
    "981e405438d22a87b31ba98a0497661e32ada9b92cc39ac5a8ab8db1da75af64",
    "147ee088b9bc5a93ad82e4a9f6436f027091d818e2a2485edb4e244453fecc83",
    "172814a4c5b93c5fb8540f91b8b7f40ab7cb02fc91c21ef5b6e97e8e92035e5f",
    "21c50f0a7f1e9156c3ae57fd0e214e6320006ca7bb6eae34f481e344a9319061",
];

const LEVEL2_ORDINARY_HOUSE_RECORD_SHA256: [&str; 22] = [
    "c18a15cda8a0d674648dc7eb0df7f921f61d5d2e1c07ec4ea60f0cff69922b92",
    "0de741297215d8de3046d6cb63fcbba39ccf4e9da14d9d1a0e81dbaa644f457f",
    "8aac3cd14373a44912391d9f20f867dd4851849173d9530f7ed187718b3747a6",
    "f28b58ec7dae4ef1499dbb5f924d68b6d881a870bf674229f93b12aad6f73b49",
    "76bf6b491295f8815ab164e39af98cf75850f8bfac52d9bc2933e86da79c2a79",
    "91b1871e5e31d372a56328cfa2ecbf7114ba4f1e931535dd145194e03130f234",
    "2272f0ad71524772987a58544931dcf037c26e1e17cc7f1beb8834c1e4c632f1",
    "e526ba68df84cf0a27a11e8bdecf41f25c63169aad63b3cc36053c1a987e931f",
    "506567c3bfb1936694f0e37c8ad3a83e31c8647566fa11decfbdc082a9e7ab7c",
    "d633a7660c7340481cd9af843990a8a7e97714c9687301043023140baeee6721",
    "0c904b9383f3e409c0594470709d0373ab1606127582a70360fc175da752668a",
    "2218af53c8806f059a0fab1168ceb0ffe7f9360661110613e7ff7f40a91f62c2",
    "ca4b9ee15eb6a6f1554d35dc96bb838c03a6ea9e2d4e0dfa3d25b898c61c0776",
    "fd5e90df8902404479bdae6354216135678e863474b4cf73430ee5f8f967c5e2",
    "f4f0f614ad6178f487854c1d89fe3c29d55af40651d430161a5ab8dc8eea2594",
    "406a79107fc302225b693953e42dc853f49673034cb089a777dcba2718e81d11",
    "3c97eb8c19ed1b7b10dcd85cbf988db063a652cb3f3ab60459ad3c2b923d5157",
    "c5577525a9345ed2fc507e93d49ef2d5360d8df5909c159556a6015440581d20",
    "4a26529ae45598edc8aea39fed684c2f685d0217e0cf895c9cf31cf79e63e2c1",
    "6e04bf521a6ced114befb2073c8e62cfa4897e3bbf7d4106ab59331a4ba2efde",
    "e9a9a5db3696c71747531793a5549b8cb4d2a32dd7e85493de26687fdc988c87",
    "69664a826be13739182b48d6d5f2ca3f5439bf35d51646745523448edad6d898",
];

const LEVEL2_ORDINARY_VILLAGER_RECORD_SHA256: [&str; 29] = [
    "5eaf19bb26cc786fad37414c043f49e5baeb3c42c1de4422ce43e5847aaff366",
    "7b3a3af48d529ac7541f2961efc92d6adb93b5b50e116e0d2683d1f787ff1247",
    "abc36c18ca3e419dbf8ac380494ee1e093a951830b65a2008338adc408097625",
    "2962d9152c9ff3b5a93d0cc9cdb112ea6846e447a8cf953065a618efb4dd6818",
    "b1dda94eec7184f7484e976f2997d3825483e67f99e5549d25ed90d3a60ddbb6",
    "922570b0abf464c6d94a8fe3e8d2335893ba0df8dd5362f51a195781b28ac5ca",
    "6801a632eef06a308feee48394575ff974cd5094555d5a9356b2dc8a68b40234",
    "3db6ad7168f323bb5553af914e2501e0cf137c5a241e8ebbc76d2f4455cee0ce",
    "45da323dc731bae8b8588f0dbdaf28645d3ae2b3b5be75c1feb15bc04e4c9a80",
    "1f68249aa347bb743c45b6e2b53cb866a2d573e32e22955f982612e94ee26947",
    "0a6bbe8dab70230bbd6b3ae023f45a85cce5b9b7dee4b45eff66c25768ac11ba",
    "b6133f1ff436d642cec62e364d9aeb87f81a5794052917e0d9f4e84a563d64b2",
    "c6349b6aa8f337fc66a7715af0181f38e693e285bb2fb3925b6286ccfa0bdf7c",
    "4391697e3193dc69edd2208b254d8e33ee58f5314a28bd0420e6429f7d28b0e0",
    "58eb04566387afe262bd8e8867e0a96c9035366653d49fe54f0ef05189929451",
    "f1026ec007f6679695a9a363fa4b2c0cfb1dc72acd10e23210462f2626dafa4f",
    "ee22af7dcb28748ef63d232be16cd54d8bb319a981000300ce0cd5442680e0dc",
    "ba03bea4caac070c9c0a72d11dbb0a612ef16367f5273350bb17fd70ed50d3bf",
    "6fb2d85d5d2f0a714aa50570e7ead27e452c319e80f6a4437d6cbd59b081f512",
    "7bd726325fb6988cbbdd627f40e450f3ad8a4c2d4346eb17b56ddf3424109fe8",
    "5a51e57d93609a924a9a79a12f5603e760f5471b5532d704d8f25dc08e06d9b2",
    "3afef4044a9e00ecdef3af98587d1186e42546393d490a7226f2d028e5b037cb",
    "72be5a854c180229be7c6aae00da8de1a0e9326fc7e55b4a8619926037dac26f",
    "730cf4692a67708ece6d1259a6e27fe51e2eb1b742237bf456c3c083e594c975",
    "022802548c2956a12a42ea129acdc28835bd27cc55e61572efeeada84f82be18",
    "383cfb2204a5e06ff84678c68affe46623698dae9b6b05a68126ea3704f4fed7",
    "30f6e38715e5b31c5fa1e8cfb9f65077a6a4fa555409571e3890da085f88e864",
    "e6e2b04784e7849b7ac96285dea904d22b42e89507db49596e77719f53606727",
    "1f2a1b75e71cbee293924b8507555cbb16aa7efb8d9fed6d0277d3d03ff22a9a",
];

const LEVEL2_DOLMEN_RECORD_SHA256: [&str; 3] = [
    "bb1e5fdfa35988c38152a238e0e1b75eb42a1ac975900351049909bfea3aabc1",
    "ce0ebe5e166f69eb59a0f391709e6c55fabc9ffbb0212a392dafafffb9f9f2f3",
    "f15617fce90097aab627b8149ec1b5f71b1ab4ea3d03ce690ae74762ffbd4e98",
];

const PACKAGE_MEMBERS: [(&str, usize); 6] = [
    ("authored-level.bin", AUTHORED_LEVEL_BYTES),
    ("entities.bin", 12 + ENTITY_COUNT * ENTITY_RECORD_BYTES),
    ("players.bin", 12 + PLAYER_COUNT * PLAYER_RECORD_BYTES),
    ("presentation.bin", 10_720),
    ("runtime.bin", RUNTIME_BYTES),
    ("terrain.bin", TERRAIN_BYTES),
];

const LEVEL2_PACKAGE_MEMBERS: [(&str, usize); 13] = [
    ("authored-level.bin", AUTHORED_LEVEL_BYTES),
    (
        "entities-post-features.bin",
        12 + ENTITY_COUNT * ENTITY_RECORD_BYTES,
    ),
    ("entities.bin", 12 + ENTITY_COUNT * ENTITY_RECORD_BYTES),
    ("feature-allocator.bin", 4_032),
    ("feature-construction-inputs.bin", 34_164),
    ("feature-end.bin", 16),
    ("feature-start.bin", 16),
    ("players.bin", 12 + PLAYER_COUNT * PLAYER_RECORD_BYTES),
    ("presentation.bin", 10_720),
    ("runtime.bin", RUNTIME_BYTES),
    ("terrain-post-features.bin", TERRAIN_BYTES),
    ("terrain-pre-features.bin", TERRAIN_BYTES),
    ("terrain.bin", TERRAIN_BYTES),
];

/// A fail-closed package or closure validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeWorldError(String);

impl NativeWorldError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for NativeWorldError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for NativeWorldError {}

type Result<T> = std::result::Result<T, NativeWorldError>;

/// The owner-package profile selects both the schema member set and the
/// external/internal level identity. It is deliberately typed so a Level 2
/// package cannot flow through the sealed Level 1 loader by accident.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerPackageProfile {
    Level1,
    Level2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerPackageSchema {
    V2,
    V3,
}

/// Resolve the single Level 1 presentation-cache root used by Rust fixtures.
///
/// `private_root` is the repository's `research/private` directory.  The
/// environment override is intentionally common to sim-core and sim-capi;
/// without it, the sealed PLAN29 final cache is selected.
#[must_use]
pub fn resolve_level1_presentation_cache_root(private_root: impl AsRef<Path>) -> PathBuf {
    std::env::var_os("MC_OWNER_ASSET_CACHE").map_or_else(
        || {
            private_root
                .as_ref()
                .join("generated")
                .join(FINAL_LEVEL1_CACHE_DIRECTORY)
        },
        PathBuf::from,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MapType {
    Day,
    Night,
    Cave,
}

impl MapType {
    fn decode(id: u8, name: &str) -> Result<Self> {
        match (id, name) {
            (0, "Day") => Ok(Self::Day),
            (1, "Night") => Ok(Self::Night),
            (2, "Cave") => Ok(Self::Cave),
            _ => Err(NativeWorldError::new(format!(
                "unsupported or inconsistent map type {id}/{name}"
            ))),
        }
    }

    #[must_use]
    pub const fn id(self) -> u8 {
        match self {
            Self::Day => 0,
            Self::Night => 1,
            Self::Cave => 2,
        }
    }

    #[must_use]
    pub const fn profile(self) -> FlightProfile {
        match self {
            Self::Day | Self::Night => FlightProfile::OUTDOOR,
            Self::Cave => FlightProfile::CAVE,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LevelMetadata {
    pub external_level: u16,
    pub internal_level: u16,
    pub map_type: MapType,
    pub capture_site: String,
    pub capture_frame: u32,
    pub vga_resolution_selector: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityAuthority {
    Inactive,
    FrozenObserved,
    PlayerFlightLive,
    SpellActivationLive,
    GoatLive,
    VillagerLive,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntitySnapshot {
    pub index: u16,
    /// Original `id_0x1A_26`, used as the sample arbitration flag for
    /// positional sound IDs 37 and 39. This is distinct from the pool slot.
    pub source_id: u16,
    pub authority: EntityAuthority,
    pub active: bool,
    pub max_life: i32,
    pub life: i32,
    pub next_relation: Option<u16>,
    pub class: u8,
    pub model: u8,
    pub control_flags: u32,
    pub entity_rng: u16,
    pub action: u8,
    pub action_marker: i8,
    pub level_gate: i8,
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub fov: i16,
    pub phase: u8,
    pub active_value: i16,
    pub activation_value: u16,
    pub actual_speed: i16,
    pub mana_cost: i32,
    pub mana: i32,
    /// Signed source field at record offset 94. Active renderable rows use a
    /// non-negative index into the 347-row particle presentation table.
    pub particle_parameter_index: Option<u16>,
    pub parameter_relation: Option<u16>,
    pub special_relation: EntitySpecialRelation,
    record: Box<[u8]>,
}

impl EntitySnapshot {
    #[must_use]
    pub fn record_bytes(&self) -> &[u8] {
        &self.record
    }

    #[cfg(test)]
    pub(crate) fn with_record_byte(mut self, offset: usize, value: u8) -> Self {
        self.record[offset] = value;
        self
    }

    /// Raw zero-terminated `oldMapEntity_0x16_22` link used by the outdoor
    /// terrain-cell painter traversal.
    #[must_use]
    pub fn old_map_entity(&self) -> u16 {
        // The private package prepends the four-byte exported entity header
        // (slot, active, padding) before the remc2 record body.
        u16::from_le_bytes([self.record[26], self.record[27]])
    }

    /// Raw zero-terminated `nextEntity_0x18_24` link toward the terrain-cell
    /// head. Together with `oldMapEntity`, this validates exact map surgery.
    #[must_use]
    pub fn next_entity(&self) -> u16 {
        u16::from_le_bytes([self.record[28], self.record[29]])
    }

    /// Raw source `dword_0x10` constructor field. Scenery constructors use
    /// this as their deterministic slot modulo eleven value.
    #[must_use]
    pub fn source_word_10(&self) -> u16 {
        u16::from_le_bytes([self.record[20], self.record[21]])
    }

    /// Raw source `word_0x24_36` targeted by the action-51 first pass.
    #[must_use]
    pub fn word_24(&self) -> u16 {
        u16::from_le_bytes([self.record[40], self.record[41]])
    }

    /// Raw source `word_0x26_38` targeted by the action-51 first pass.
    #[must_use]
    pub fn word_26(&self) -> u16 {
        u16::from_le_bytes([self.record[42], self.record[43]])
    }

    /// Raw reset `animationFrame_0x60_96` used by fixed and base-plus-frame
    /// sprite families.
    #[must_use]
    pub fn animation_frame(&self) -> i8 {
        i8::from_le_bytes([self.record[96]])
    }

    #[must_use]
    fn castle_placement_blocker(&self) -> Level1CastlePlacementBlocker {
        Level1CastlePlacementBlocker {
            source_entity_index: self.index,
            position: self.position,
            // Exported axis_4d pitch/roll retain original record offsets
            // 0x58/0x5a. They are distinct from the orientation fields.
            half_x: i16::from_le_bytes([self.record[88], self.record[89]]),
            half_y: i16::from_le_bytes([self.record[90], self.record[91]]),
        }
    }

    #[must_use]
    pub fn collision_target(&self) -> Level1CollisionTarget {
        Level1CollisionTarget {
            entity_index: self.index,
            position: self.position,
            collision_vertical_offset: i16::from_le_bytes([self.record[86], self.record[87]]),
            collision_half_width: i16::from_le_bytes([self.record[88], self.record[89]]),
            collision_half_height: i16::from_le_bytes([self.record[92], self.record[93]]),
            active: self.active,
        }
    }

    fn ambient_entity(&self) -> Option<Level1AmbientEntity> {
        if !self.active
            || self.class != 10
            || !matches!(
                (self.model, self.action),
                (0, 0) | (1, 1) | (14, 14) | (60, 65)
            )
        {
            return None;
        }
        let read_u16 = |offset| u16::from_le_bytes([self.record[offset], self.record[offset + 1]]);
        let read_i16 = |offset| i16::from_le_bytes([self.record[offset], self.record[offset + 1]]);
        let read_u32 = |offset| {
            u32::from_le_bytes([
                self.record[offset],
                self.record[offset + 1],
                self.record[offset + 2],
                self.record[offset + 3],
            ])
        };
        Some(Level1AmbientEntity {
            source_entity_index: self.index,
            parent_entity_index: 0,
            source_id: self.source_id,
            position: self.position,
            max_life: self.max_life,
            life: self.life,
            control_flags: self.control_flags,
            search_radius: read_u32(20),
            entity_rng: read_u16(24),
            class: self.class,
            model: self.model,
            action: self.action,
            phase: self.phase,
            yaw: self.yaw,
            // Particle `array_0x52` axes, distinct from orientation yaw/pitch.
            pitch: read_i16(88),
            fov: read_i16(92),
            vertical_velocity: read_i16(48),
            actual_speed: self.actual_speed,
            max_speed: read_i16(138),
            render_parameter_index: self.particle_parameter_index.unwrap_or(0),
            animation_frame: self.animation_frame(),
            animation_limit: i8::from_le_bytes([self.record[97]]),
            active: true,
            map_linked: self.control_flags & 4 != 0,
            last_material_effect: None,
            collision_probe_ran: false,
        })
    }
}

/// One Level 2 class-2/model-3/action-9 bad stone.  The package already
/// contains the post-`GenerateEvents_49290` reset rows; this wrapper keeps the
/// source row lossless while giving the production Level 2 session the exact
/// action-9 terrain-snap seam from remc2 `sub_65110`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level2BadStone {
    baseline: EntitySnapshot,
    entity: EntitySnapshot,
    map_cell: (u8, u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2BadStoneStep {
    pub source_slot: u16,
    pub previous_position: Position,
    pub position: Position,
    pub previous_control_flags: u32,
    pub control_flags: u32,
    pub previous_phase: u8,
    pub phase: u8,
}

impl Level2BadStone {
    fn from_entity(entity: &EntitySnapshot, planes: &TerrainPlanes) -> Result<Self> {
        let Some(ordinal) = LEVEL2_BAD_STONE_SLOTS
            .iter()
            .position(|slot| *slot == entity.index)
        else {
            return Err(NativeWorldError::new(format!(
                "Level 2 bad stone source slot {} is not one of 33..=36",
                entity.index
            )));
        };
        if !entity.active
            || entity.class != LEVEL2_BAD_STONE_CLASS
            || entity.model != LEVEL2_BAD_STONE_MODEL
            || entity.action != LEVEL2_BAD_STONE_ACTION
        {
            return Err(NativeWorldError::new(format!(
                "Level 2 bad stone {} has class/model/action {}/{}/{}",
                entity.index, entity.class, entity.model, entity.action
            )));
        }
        if entity.max_life != 300
            || entity.life != 300
            || entity.actual_speed != 16
            || entity.source_word_10() != entity.index % 11
            || entity.control_flags & 4 == 0
            || entity.control_flags & 0x0002_0000 == 0
            || entity.particle_parameter_index != Some(LEVEL2_BAD_STONE_PARTICLE)
            || entity.parameter_relation != Some(LEVEL2_BAD_STONE_PARAMETER - 1)
            || entity.special_relation != EntitySpecialRelation::GlobalSpecialState
            || entity.old_map_entity() != 0
            || entity.next_entity() != 0
        {
            return Err(NativeWorldError::new(format!(
                "Level 2 bad stone {} constructor fields do not match pinned sub_4AE80",
                entity.index
            )));
        }
        let map_cell = (
            (entity.position.x >> 8) as u8,
            (entity.position.y >> 8) as u8,
        );
        let map_index = terrain_index(map_cell.0, map_cell.1);
        if planes.entity_indices[map_index] != entity.index as i16 {
            return Err(NativeWorldError::new(format!(
                "Level 2 bad stone {} is not the head of its authored terrain map cell",
                entity.index
            )));
        }
        let record_hash = sha256_hex(entity.record_bytes());
        if record_hash != LEVEL2_BAD_STONE_RECORD_SHA256[ordinal] {
            return Err(NativeWorldError::new(format!(
                "Level 2 bad stone {} record hash changed: {record_hash}",
                entity.index
            )));
        }
        Ok(Self {
            baseline: entity.clone(),
            entity: entity.clone(),
            map_cell,
        })
    }

    #[must_use]
    pub fn entity(&self) -> &EntitySnapshot {
        &self.entity
    }

    #[must_use]
    pub const fn map_cell(&self) -> (u8, u8) {
        self.map_cell
    }

    #[must_use]
    pub fn source_record_sha256(&self) -> String {
        sha256_hex(self.entity.record_bytes())
    }

    /// Apply the exact class-2 action-9 body: set source control byte 2 bit 1,
    /// snap Z to the shared terrain query, then advance the source action
    /// cadence byte. No movement, RNG draw, or allocation is performed.
    pub fn step_action_9(&mut self, terrain: &SessionTerrain) -> Level2BadStoneStep {
        let previous_position = self.entity.position;
        let previous_control_flags = self.entity.control_flags;
        let previous_phase = self.entity.phase;
        self.entity.record[18] |= 2;
        self.entity.control_flags = u32::from_le_bytes([
            self.entity.record[16],
            self.entity.record[17],
            self.entity.record[18],
            self.entity.record[19],
        ]);
        self.entity.position.z = FlightWorld::ground_height(terrain, self.entity.position);
        self.entity.record[84..86].copy_from_slice(&self.entity.position.z.to_le_bytes());
        self.entity.phase = self.entity.phase.wrapping_add(1);
        self.entity.record[66] = self.entity.phase;
        Level2BadStoneStep {
            source_slot: self.entity.index,
            previous_position,
            position: self.entity.position,
            previous_control_flags,
            control_flags: self.entity.control_flags,
            previous_phase,
            phase: self.entity.phase,
        }
    }

    pub fn reset(&mut self) {
        self.entity = self.baseline.clone();
    }
}

/// One Level 2 class-2/model-2/action-6 dolmen.  `sub_4ADF0` installs the
/// same map-linked constructor state as the bad-stone family, while
/// `AddDolmen02_02_65080` marks player proximity and snaps the entity to the
/// shared terrain altitude.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level2Dolmen {
    baseline: EntitySnapshot,
    entity: EntitySnapshot,
    map_cell: (u8, u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2DolmenStep {
    pub source_slot: u16,
    pub previous_position: Position,
    pub position: Position,
    pub previous_control_flags: u32,
    pub control_flags: u32,
    pub previous_phase: u8,
    pub phase: u8,
}

fn level2_source_intersects(left: &EntitySnapshot, right: &EntitySnapshot) -> bool {
    let i16_at = |entity: &EntitySnapshot, offset: usize| {
        i16::from_le_bytes([entity.record[offset], entity.record[offset + 1]])
    };
    let dx = i32::from(right.position.x.wrapping_sub(left.position.x).cast_signed());
    let dy = i32::from(right.position.y.wrapping_sub(left.position.y).cast_signed());
    let dz = i32::from(left.position.z) + i32::from(i16_at(left, 90))
        - i32::from(right.position.z)
        - i32::from(i16_at(right, 90));
    dx.abs() < i32::from(i16_at(left, 86)) + i32::from(i16_at(right, 86))
        && dy.abs() < i32::from(i16_at(left, 88)) + i32::from(i16_at(right, 88))
        && dz.abs() < i32::from(i16_at(left, 92)) + i32::from(i16_at(right, 92))
}

fn level2_wrapping_xy_distance(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    dx.wrapping_mul(dx)
        .wrapping_add(dy.wrapping_mul(dy))
        .cast_unsigned()
}

fn level2_mana_sphere_dimension(mana: i32) -> i16 {
    MANA_SPHERE_COLLISION_DIMENSIONS[usize::from(mana_sphere_size_ordinal(mana))]
}

fn level2_carrier_sphere_overlap(carrier: Position, sphere: &ManaSphere) -> bool {
    let extent = level2_mana_sphere_dimension(sphere.mana);
    let dx = i32::from(sphere.position.x.wrapping_sub(carrier.x).cast_signed());
    let dy = i32::from(sphere.position.y.wrapping_sub(carrier.y).cast_signed());
    let dz = i32::from(carrier.z) + i32::from(MANA_CARRIER_COLLISION_VERTICAL_OFFSET)
        - i32::from(sphere.position.z);
    dx.abs() < i32::from(MANA_CARRIER_COLLISION_HALF_WIDTH + extent)
        && dy.abs() < i32::from(MANA_CARRIER_COLLISION_HALF_WIDTH + extent)
        && dz.abs() < i32::from(MANA_CARRIER_COLLISION_HALF_HEIGHT + extent)
}

fn level2_castle_sphere_overlap(castle: Position, sphere: &ManaSphere) -> bool {
    let extent = level2_mana_sphere_dimension(sphere.mana);
    let dx = i32::from(sphere.position.x.wrapping_sub(castle.x).cast_signed());
    let dy = i32::from(sphere.position.y.wrapping_sub(castle.y).cast_signed());
    dx.abs() < i32::from(CASTLE_STAGE_ONE_COLLISION_HALF_WIDTH + extent)
        && dy.abs() < i32::from(CASTLE_STAGE_ONE_COLLISION_HALF_WIDTH + extent)
}

/// `sub_60810`'s stage table with the imported player spell coefficient
/// normalised to 256 (the Level 2 economy projection has no live spell-tier
/// mutator yet). The returned pair is `(max_life, mana_capacity)`.
fn level2_castle_stage_stats(level: u8) -> (i32, i32) {
    match level {
        0 => (0, 5_000),
        1 => (20_000, 8_500),
        2 => (40_000, 18_000),
        3 => (40_000, 38_800),
        4 => (60_000, 78_600),
        5 => (60_000, 158_200),
        6 => (80_000, 317_400),
        _ => (80_000, 300_000_000),
    }
}

/// `sub_60400`'s stage-controller counts and the corresponding model-79
/// placement points from `x_BYTE_DB038` (pinned `EventsFunctions.cpp`).  The
/// source stores this as one compact count/offset table; spelling it out here
/// keeps source order stable and makes allocation failure observable after any
/// committed prefix.
fn level2_castle_controller_points(level: u8) -> &'static [(i8, i8)] {
    match level {
        1 => &[(4, 4)],
        2 | 3 => &[(3, 3), (17, 3), (3, 17), (17, 17)],
        4 | 5 => &[(3, 3), (31, 3), (3, 31), (31, 31)],
        6 | 7 => &[
            (3, 3),
            (24, 3),
            (45, 3),
            (3, 24),
            (45, 24),
            (3, 45),
            (24, 45),
            (45, 45),
        ],
        _ => &[],
    }
}

/// BUILD00DATTAB dimensions implied by the source controller point table.
/// The validated Level 2 footprint already supplies the effective dimensions
/// used by the fixed-resolution source path; controller offsets are applied
/// relative to those dimensions without another VGA-halving pass.
fn level2_castle_controller_dimensions(level: u8) -> (u8, u8) {
    match level {
        1 => (8, 8),
        2 | 3 => (20, 20),
        4 | 5 => (34, 34),
        6 | 7 => (48, 48),
        _ => (0, 0),
    }
}

/// Port of `sub_4AA40`'s Castle root/`axis_0x9A_154x` placement split. The
/// caller supplies the validated owner BUILD footprint; Level 2's legacy
/// compatibility `build_castle` path still uses the projectile position as
/// both values when no footprint is available.
#[must_use]
pub fn level2_castle_root_positions<W: FlightWorld>(
    world: &W,
    footprint: &CastleBuildFootprint,
    projectile_position: Position,
) -> (Position, Position) {
    // The qualified Level 2 profile pins `x_WORD_180660_VGA_type_resolution`
    // to 1 (the fixed 320x200 path), so the production helper uses the
    // source's halved BUILD dimensions by default.
    level2_castle_root_positions_with_resolution(world, footprint, projectile_position, true)
}

/// Derive the Castle root and `axis_0x9A_154x` anchor exactly as
/// `sub_4AA40` does, including the source's VGA-size split.  The pinned
/// 320x200 path passes `low_resolution = true`, which halves the BUILD table
/// dimensions before the perimeter minimum (`sub_48E60`) is sampled.  Keeping
/// the selector explicit makes the alternate-resolution exclusion visible at
/// this seam instead of silently inferring it from host display state.
#[must_use]
pub fn level2_castle_root_positions_with_resolution<W: FlightWorld>(
    world: &W,
    footprint: &CastleBuildFootprint,
    projectile_position: Position,
    low_resolution: bool,
) -> (Position, Position) {
    let mut root_position = Position {
        x: projectile_position.x & 0xff00,
        y: projectile_position.y & 0xff00,
        z: world.ground_height(projectile_position),
    };
    if ((root_position.x >> 8) + (root_position.y >> 8)) & 1 != 0 {
        root_position.x = root_position.x.wrapping_add(256);
    }
    let center_x = (root_position.x >> 8) as u8;
    let center_y = (root_position.y >> 8) as u8;
    let width = if low_resolution {
        footprint.width >> 1
    } else {
        footprint.width
    };
    let height = if low_resolution {
        footprint.height >> 1
    } else {
        footprint.height
    };
    let start_x = center_x.wrapping_sub(width / 2);
    let start_y = center_y.wrapping_sub(height / 2);
    let mut minimum = 250_u8;
    let mut sample = |x: u8, y: u8| {
        let height = world.ground_height(Position {
            x: u16::from(x) << 8,
            y: u16::from(y) << 8,
            z: 0,
        });
        // `sub_48E60` scans the byte height plane, not the interpolated
        // world-height result.  At an exact tile corner the interpolation
        // is exactly `32 * mapHeightmap`; retain the source byte boundary
        // while making the conversion explicit for generic FlightWorlds.
        let cell_height = (i32::from(height) >> 5).clamp(0, i32::from(u8::MAX)) as u8;
        minimum = minimum.min(cell_height);
    };
    for offset in 0..width {
        let x = start_x.wrapping_add(offset);
        sample(x, start_y);
        sample(x, start_y.wrapping_add(height));
    }
    for offset in 0..height {
        let y = start_y.wrapping_add(offset);
        sample(start_x.wrapping_add(width), y);
        sample(start_x, y);
    }
    let construction_anchor = Position {
        z: i16::from(minimum) * 32,
        ..root_position
    };
    (root_position, construction_anchor)
}

impl Level2Dolmen {
    fn from_entity(entity: &EntitySnapshot, planes: &TerrainPlanes) -> Result<Self> {
        let Some(ordinal) = [108_u16, 109, 110]
            .iter()
            .position(|slot| *slot == entity.index)
        else {
            return Err(NativeWorldError::new(format!(
                "Level 2 dolmen source slot {} is not one of 108..=110",
                entity.index
            )));
        };
        if !entity.active
            || entity.class != 2
            || entity.model != 2
            || entity.action != 6
            || entity.max_life != 300
            || entity.life != 300
            || entity.actual_speed != 16
            || entity.source_word_10() != entity.index % 11
            || entity.control_flags & 4 == 0
            || entity.particle_parameter_index != Some(39)
            || entity.parameter_relation != Some(59)
            || entity.special_relation != EntitySpecialRelation::GlobalSpecialState
            || entity.old_map_entity() != 0
            || entity.next_entity() != 0
        {
            return Err(NativeWorldError::new(format!(
                "Level 2 dolmen {} constructor fields do not match pinned sub_4ADF0",
                entity.index
            )));
        }
        let map_cell = (
            (entity.position.x >> 8) as u8,
            (entity.position.y >> 8) as u8,
        );
        if planes.entity_indices[terrain_index(map_cell.0, map_cell.1)] != entity.index as i16 {
            return Err(NativeWorldError::new(format!(
                "Level 2 dolmen {} is not the head of its authored terrain map cell",
                entity.index
            )));
        }
        let record_hash = sha256_hex(entity.record_bytes());
        if record_hash != LEVEL2_DOLMEN_RECORD_SHA256[ordinal] {
            return Err(NativeWorldError::new(format!(
                "Level 2 dolmen {} record hash changed: {record_hash}",
                entity.index
            )));
        }
        Ok(Self {
            baseline: entity.clone(),
            entity: entity.clone(),
            map_cell,
        })
    }

    #[must_use]
    pub fn entity(&self) -> &EntitySnapshot {
        &self.entity
    }

    #[must_use]
    pub const fn map_cell(&self) -> (u8, u8) {
        self.map_cell
    }

    /// Apply `AddDolmen02_02_65080`: the source mutates a nearby player
    /// entity's byte-1 interaction flag, then only snaps the dolmen Z to the
    /// shared terrain altitude.  It does not advance the dolmen phase or set
    /// any dolmen control bit.
    pub fn step_action_6(&mut self, terrain: &SessionTerrain) -> Level2DolmenStep {
        let previous_position = self.entity.position;
        let previous_control_flags = self.entity.control_flags;
        let previous_phase = self.entity.phase;
        self.entity.position.z = FlightWorld::ground_height(terrain, self.entity.position);
        self.entity.record[84..86].copy_from_slice(&self.entity.position.z.to_le_bytes());
        Level2DolmenStep {
            source_slot: self.entity.index,
            previous_position,
            position: self.entity.position,
            previous_control_flags,
            control_flags: self.entity.control_flags,
            previous_phase,
            phase: self.entity.phase,
        }
    }

    pub fn reset(&mut self) {
        self.entity = self.baseline.clone();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntitySpecialRelation {
    None,
    PlayerState(u8),
    GlobalSpecialState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerSnapshot {
    pub slot: u8,
    pub is_ai: bool,
    pub entity_index: u16,
    pub active_player_index: i16,
    pub turn: i32,
    pub left_spell_slot: u8,
    pub right_spell_slot: u8,
    /// Raw signed selectors. Level 2 uses -1/-1 for an empty local hand;
    /// Level 1's normalized selectors remain available through the legacy
    /// `left_spell_slot`/`right_spell_slot` fields.
    pub left_spell_slot_raw: i16,
    pub right_spell_slot_raw: i16,
    pub left_subspell_index: i8,
    pub right_subspell_index: i8,
    pub selected_spell_ui_index: u8,
    pub selected_subspell_index: i8,
    pub enabled_spell_holders: [u16; PLAYER_SPELL_COUNT],
    pub spell_hand_eligibility: [u8; PLAYER_SPELL_COUNT],
    pub family_selected_tiers: [i8; PLAYER_SPELL_COUNT],
    record: Box<[u8]>,
}

impl PlayerSnapshot {
    #[must_use]
    pub fn record_bytes(&self) -> &[u8] {
        &self.record
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerrainPlanes {
    pub terrain_types: Box<[u8]>,
    pub ground_heights: Box<[u8]>,
    pub shading: Box<[u8]>,
    pub angles: Box<[u8]>,
    pub secondary_heights: Box<[u8]>,
    pub entity_indices: Box<[i16]>,
}

/// Mutable terrain owned by exactly one play session.
///
/// `NativeWorld` remains immutable and shareable. Castle construction changes
/// these planes immediately after the action-44 source slot so subsequent
/// source entities, flight, collision, and rendering all read the same state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionTerrain {
    map_type: MapType,
    planes: TerrainPlanes,
    terrain_rng: u16,
    mutation_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HouseRemovalResident {
    pub position: Position,
    pub model: u8,
    pub action: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HouseRemovalOutcome {
    pub final_rng: u16,
    pub final_occupants: i32,
    pub residents: Vec<HouseRemovalResident>,
}

impl SessionTerrain {
    #[must_use]
    pub fn from_inspection(inspection: &NativeWorldInspection) -> Self {
        Self {
            map_type: inspection.metadata.map_type,
            planes: inspection.terrain_planes.clone(),
            // Level 2 preserves the post-feature terrain RNG boundary.
            terrain_rng: inspection.terrain_rng,
            mutation_sequence: 0,
        }
    }

    #[must_use]
    pub fn from_world(world: &NativeWorld) -> Self {
        Self {
            map_type: world.metadata.map_type,
            planes: world.terrain_planes.clone(),
            // Exact post-generation `rand2_17B4E0` observed at the qualified
            // Package-M export seam. Castle topology variants continue this
            // independent 16-bit terrain stream.
            terrain_rng: 39_663,
            mutation_sequence: 0,
        }
    }

    #[must_use]
    pub const fn planes(&self) -> &TerrainPlanes {
        &self.planes
    }

    #[must_use]
    pub fn ground_heights(&self) -> &[u8] {
        &self.planes.ground_heights
    }

    /// Borrow the immutable ground plane and mutable shading plane together
    /// for the source outer-turn dynamic-light subtraction/service boundary.
    pub fn dynamic_lighting_planes(&mut self) -> (&[u8], &mut [u8]) {
        (
            self.planes.ground_heights.as_ref(),
            self.planes.shading.as_mut(),
        )
    }

    #[must_use]
    pub const fn terrain_rng(&self) -> u16 {
        self.terrain_rng
    }

    #[must_use]
    pub const fn mutation_sequence(&self) -> u64 {
        self.mutation_sequence
    }

    /// Canonical content hash for the six mutable source terrain planes.
    ///
    /// FNV-1a is applied to the schema domain, little-endian dimensions, five
    /// byte planes, then every signed entity index in little-endian form.
    /// The mutation sequence is intentionally separate from content identity.
    #[must_use]
    pub fn deterministic_hash(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut append = |bytes: &[u8]| {
            for byte in bytes {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        append(b"mc2-session-terrain-v1\0");
        append(&256_u16.to_le_bytes());
        append(&256_u16.to_le_bytes());
        append(&self.planes.terrain_types);
        append(&self.planes.ground_heights);
        append(&self.planes.shading);
        append(&self.planes.angles);
        append(&self.planes.secondary_heights);
        for entity_index in &self.planes.entity_indices {
            append(&entity_index.to_le_bytes());
        }
        hash
    }

    /// Apply one source-ordered BUILD[1] terrain update.
    ///
    /// # Errors
    ///
    /// Returns an error for a malformed timer. Mutation is transactional: an
    /// error leaves all six terrain planes, the terrain RNG, and the mutation
    /// sequence unchanged.
    pub fn apply_castle_update(
        &mut self,
        update: CastleTerrainUpdate,
        footprint: &CastleBuildFootprint,
    ) -> std::result::Result<(), String> {
        if update.is_ambient() {
            return self.apply_ambient_update(update);
        }
        let mut staged = self.clone();
        staged.apply_castle_update_inner(update, footprint)?;
        staged.mutation_sequence = staged.mutation_sequence.wrapping_add(1);
        *self = staged;
        Ok(())
    }

    pub fn apply_ambient_update(
        &mut self,
        update: CastleTerrainUpdate,
    ) -> std::result::Result<(), String> {
        let mut staged = self.clone();
        match update {
            CastleTerrainUpdate::AmbientMaterial {
                position,
                in_type,
                target_type,
            } => {
                let x = position.x.wrapping_add(128).to_be_bytes()[0];
                let y = position.y.wrapping_add(128).to_be_bytes()[0];
                apply_ambient_material(
                    &mut staged.planes,
                    x,
                    y,
                    in_type,
                    target_type,
                    staged.map_type,
                )?;
            }
            CastleTerrainUpdate::AmbientCollision {
                position,
                height_delta,
            } => apply_ambient_collision(
                &mut staged.planes,
                position,
                height_delta,
                staged.map_type,
                &mut staged.terrain_rng,
            ),
            _ => {
                return Err("non-ambient update requires a Castle BUILD footprint".to_owned());
            }
        }
        staged.mutation_sequence = staged.mutation_sequence.wrapping_add(1);
        *self = staged;
        Ok(())
    }

    /// Remove one source action-53 house BUILD footprint.
    ///
    /// Resident selection and height restoration share the house RNG and must
    /// therefore run in the same row-major loop. Terrain topology retains its
    /// independent session RNG. The returned resident requests are already in
    /// exact allocation order.
    ///
    /// # Errors
    ///
    /// Returns an error for a malformed footprint. Mutation is transactional.
    pub fn apply_house_removal(
        &mut self,
        position: Position,
        mut house_rng: u16,
        mut occupants: i32,
        produces_residents: bool,
        footprint: &CastleBuildFootprint,
    ) -> std::result::Result<HouseRemovalOutcome, String> {
        let expected = usize::from(footprint.width) * usize::from(footprint.height);
        if footprint.cells().len() != expected {
            return Err(format!(
                "house BUILD {} has {} cells, expected {expected}",
                footprint.index,
                footprint.cells().len()
            ));
        }
        let mut staged = self.clone();
        let center_x = position.x.wrapping_add(128).to_be_bytes()[0];
        let center_y = position.y.wrapping_add(128).to_be_bytes()[0];
        let start_x = center_x.wrapping_sub(footprint.width / 2);
        let start_y = center_y.wrapping_sub(footprint.height / 2);
        let corner_height =
            |x: u8, y: u8| i16::from(staged.planes.ground_heights[terrain_index(x, y)]);
        let z_coefficient = (corner_height(start_x, start_y)
            + corner_height(start_x.wrapping_add(footprint.width), start_y)
            + corner_height(
                start_x.wrapping_add(footprint.width),
                start_y.wrapping_add(footprint.height),
            )
            + corner_height(start_x, start_y.wrapping_add(footprint.height)))
            >> 2;
        let mut residents = Vec::new();
        let mut nonempty_ordinal = 0_u32;
        for row in 0..footprint.height {
            for column in 0..footprint.width {
                let cell = footprint.cells()
                    [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
                if cell == [0xff, 0xff] {
                    continue;
                }
                let x = start_x.wrapping_add(column);
                let y = start_y.wrapping_add(row);
                nonempty_ordinal = nonempty_ordinal.wrapping_add(1);
                let resident_z = if nonempty_ordinal & 7 == 0 {
                    z_coefficient.wrapping_sub(10).wrapping_mul(32)
                } else {
                    z_coefficient.wrapping_mul(32)
                };
                if occupants > 0 {
                    occupants -= 1;
                    if produces_residents {
                        let (model, action) = if occupants == 0 {
                            (12, 97)
                        } else if occupants < 4 {
                            (4, 33)
                        } else {
                            house_rng = next_house_rng(house_rng);
                            match house_rng % 12 {
                                0 | 1 => (4, 33),
                                2 | 3 => (14, 113),
                                4..=8 => (13, 105),
                                9..=11 => (12, 97),
                                _ => unreachable!("modulo twelve"),
                            }
                        };
                        residents.push(HouseRemovalResident {
                            position: Position {
                                x: u16::from(x) << 8,
                                y: u16::from(y) << 8,
                                z: resident_z,
                            },
                            model,
                            action,
                        });
                    }
                }
                let index = terrain_index(x, y);
                staged.planes.angles[index] = (staged.planes.angles[index] & 0x70) | 1;
                add_building_to_terrain(
                    &mut staged.planes,
                    x,
                    y,
                    staged.map_type,
                    &mut staged.terrain_rng,
                );
                if cell[1] == 0xff {
                    continue;
                }
                let height = staged.planes.ground_heights[index];
                staged.planes.ground_heights[index] =
                    reduce_house_height(height, cell[1], &mut house_rng);
            }
        }
        for row in 0..footprint.height {
            for column in 0..footprint.width {
                smooth_building_height(
                    &mut staged.planes,
                    start_x.wrapping_add(column),
                    start_y.wrapping_add(row),
                    staged.map_type,
                );
            }
        }
        staged.mutation_sequence = staged.mutation_sequence.wrapping_add(1);
        *self = staged;
        Ok(HouseRemovalOutcome {
            final_rng: house_rng,
            final_occupants: occupants,
            residents,
        })
    }

    /// Apply one source action-51 BUILD terrain event transactionally.
    pub fn apply_house_construction(
        &mut self,
        position: Position,
        remaining: Option<i32>,
        footprint: &CastleBuildFootprint,
    ) -> std::result::Result<(), String> {
        let expected = usize::from(footprint.width) * usize::from(footprint.height);
        if footprint.cells().len() != expected {
            return Err(format!(
                "house BUILD {} has {} cells, expected {expected}",
                footprint.index,
                footprint.cells().len()
            ));
        }
        let mut staged = self.clone();
        match remaining {
            Some(remaining) => {
                let remaining = u8::try_from(remaining).map_err(|_| {
                    format!("house action-51 remaining count {remaining} does not fit u8")
                })?;
                if remaining == 0 {
                    return Err(
                        "house action-51 construction pass requires remaining > 0".to_owned()
                    );
                }
                apply_level1_load_house_sweep(
                    &mut staged.planes,
                    &mut staged.terrain_rng,
                    position,
                    footprint,
                    remaining,
                );
            }
            None => {
                apply_level1_load_house_sweep(
                    &mut staged.planes,
                    &mut staged.terrain_rng,
                    position,
                    footprint,
                    0,
                );
                apply_level1_load_house_edge_smoothing(&mut staged.planes, position, footprint);
            }
        }
        staged.mutation_sequence = staged.mutation_sequence.wrapping_add(1);
        *self = staged;
        Ok(())
    }

    fn apply_castle_update_inner(
        &mut self,
        update: CastleTerrainUpdate,
        footprint: &CastleBuildFootprint,
    ) -> std::result::Result<(), String> {
        if let CastleTerrainUpdate::AmbientMaterial {
            position,
            in_type,
            target_type,
        } = update
        {
            let x = position.x.wrapping_add(128).to_be_bytes()[0];
            let y = position.y.wrapping_add(128).to_be_bytes()[0];
            apply_ambient_material(&mut self.planes, x, y, in_type, target_type, self.map_type)?;
            return Ok(());
        }
        if let CastleTerrainUpdate::AmbientCollision {
            position,
            height_delta,
        } = update
        {
            apply_ambient_collision(
                &mut self.planes,
                position,
                height_delta,
                self.map_type,
                &mut self.terrain_rng,
            );
            return Ok(());
        }
        let position = match update {
            CastleTerrainUpdate::ConstructionPass { position, .. }
            | CastleTerrainUpdate::ConstructionCompleted { position }
            | CastleTerrainUpdate::StageDestroyed { position, .. } => position,
            CastleTerrainUpdate::AmbientMaterial { .. }
            | CastleTerrainUpdate::AmbientCollision { .. } => {
                unreachable!("ambient update returned above")
            }
        };
        let center_x = position.x.wrapping_add(128).to_be_bytes()[0];
        let center_y = position.y.wrapping_add(128).to_be_bytes()[0];
        let start_x = center_x.wrapping_sub(footprint.width / 2);
        let start_y = center_y.wrapping_sub(footprint.height / 2);

        match update {
            CastleTerrainUpdate::ConstructionPass { remaining, .. } => {
                if !(1..=18).contains(&remaining) {
                    return Err(format!(
                        "Castle construction remaining count {remaining} is outside 1..=18"
                    ));
                }
                let construction_base = i16::from(position.z >> 5);
                for row in 0..footprint.height {
                    for column in 0..footprint.width {
                        let cell = footprint.cells()
                            [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
                        let x = start_x.wrapping_add(column);
                        let y = start_y.wrapping_add(row);
                        let index = terrain_index(x, y);
                        if (remaining % 7 == 0 || remaining == 1) && cell[0] != 0xff {
                            apply_castle_material(&mut self.planes, x, y, cell[0], self.map_type);
                        }
                        if cell[1] == 0xff {
                            continue;
                        }
                        let current = i16::from(self.planes.ground_heights[index]);
                        let target = construction_base + i16::from(cell[1]);
                        let delta = target - current;
                        if delta == 0 {
                            continue;
                        }
                        if current == 0
                            || building_topology_rebuild_required(self.planes.terrain_types[index])
                        {
                            self.planes.angles[index] = (self.planes.angles[index] & 0xf8) | 1;
                            add_building_to_terrain(
                                &mut self.planes,
                                x,
                                y,
                                self.map_type,
                                &mut self.terrain_rng,
                            );
                        }
                        let step = delta / remaining;
                        self.planes.ground_heights[index] = current.wrapping_add(step) as u8;
                        if remaining == 1 && self.planes.angles[index].cast_signed() < 0 {
                            self.planes.angles[index] &= 0x7f;
                            if self.map_type != MapType::Cave {
                                self.planes.angles[index] |= 0x08;
                            }
                        }
                        if self.map_type == MapType::Cave {
                            let desired =
                                target.max(i16::from(self.planes.ground_heights[index])) + 100;
                            let desired = desired.min(255);
                            let secondary = i16::from(self.planes.secondary_heights[index]);
                            if desired > secondary {
                                self.planes.secondary_heights[index] =
                                    secondary.wrapping_add((desired - secondary) / remaining) as u8;
                            }
                            normalize_cave_clearance(&mut self.planes, index);
                        } else if remaining == 2 {
                            self.planes.angles[index] &= 0xf7;
                        }
                    }
                }
            }
            CastleTerrainUpdate::ConstructionCompleted { .. } => {
                if self.map_type != MapType::Cave {
                    for row in 0..footprint.height {
                        for column in 0..footprint.width {
                            let index = terrain_index(
                                start_x.wrapping_add(column),
                                start_y.wrapping_add(row),
                            );
                            if self.planes.angles[index] & 0x08 != 0 {
                                self.planes.angles[index] |= 0x80;
                                self.planes.angles[index] &= 0xf7;
                            }
                        }
                    }
                }
            }
            CastleTerrainUpdate::StageDestroyed { entity_rng, .. } => {
                remove_castle_stage(
                    &mut self.planes,
                    start_x,
                    start_y,
                    footprint,
                    self.map_type,
                    &mut self.terrain_rng,
                    entity_rng,
                );
            }
            CastleTerrainUpdate::AmbientMaterial { .. }
            | CastleTerrainUpdate::AmbientCollision { .. } => {
                unreachable!("ambient update returned above")
            }
        }
        Ok(())
    }
}

pub(crate) fn reduce_house_height(height: u8, reduction: u8, house_rng: &mut u16) -> u8 {
    if reduction >= height {
        return 0;
    }
    *house_rng = next_house_rng(*house_rng);
    if *house_rng % 0x32 <= 20 {
        height.wrapping_sub(reduction)
    } else {
        *house_rng = next_house_rng(*house_rng);
        height
            .wrapping_sub(reduction)
            .wrapping_add((*house_rng % 0x14) as u8)
    }
}

impl FlightWorld for SessionTerrain {
    fn ground_height(&self, position: Position) -> i16 {
        interpolate_ground(&self.planes.ground_heights, position.x, position.y) as i16
    }

    fn ceiling_height(&self, position: Position) -> Option<i16> {
        (self.map_type == MapType::Cave).then(|| {
            interpolate_cave(&self.planes.secondary_heights, position.x, position.y) as i16
        })
    }

    fn material_mask(&self, position: Position) -> u32 {
        let x = position.x.to_be_bytes()[0];
        let y = position.y.to_be_bytes()[0];
        material_mask(self.planes.terrain_types[terrain_index(x, y)])
    }

    fn terrain_type(&self, position: Position) -> u8 {
        let x = position.x.to_be_bytes()[0];
        let y = position.y.to_be_bytes()[0];
        self.planes.terrain_types[terrain_index(x, y)]
    }

    fn angle_flags(&self, tile: crate::Tile) -> u8 {
        self.planes.angles[terrain_index(tile.x, tile.y)]
    }
}

const TERRAIN_GENERATION_TABLE_PREFIX: [u8; 0x70] = [
    0x1b, 0x00, 0x1b, 0x50, 0x1b, 0x30, 0x1b, 0x60, 0x1a, 0x00, 0x1a, 0x50, 0x1a, 0x30, 0x1a, 0x60,
    0x0a, 0x00, 0x0a, 0x50, 0x0a, 0x30, 0x0a, 0x60, 0x0a, 0x00, 0x0a, 0x50, 0x0a, 0x30, 0x0a, 0x60,
    0x0b, 0x00, 0x0b, 0x50, 0x0b, 0x30, 0x0b, 0x60, 0x0b, 0x00, 0x0b, 0x50, 0x0b, 0x30, 0x0b, 0x60,
    0x0c, 0x00, 0x0c, 0x50, 0x0c, 0x30, 0x0c, 0x60, 0x0c, 0x00, 0x0c, 0x50, 0x0c, 0x30, 0x0c, 0x60,
    0x15, 0x00, 0x15, 0x50, 0x15, 0x30, 0x15, 0x60, 0x16, 0x00, 0x16, 0x50, 0x16, 0x30, 0x16, 0x60,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x00, 0x18, 0x50, 0x18, 0x30, 0x18, 0x60,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x17, 0x00, 0x17, 0x50, 0x17, 0x30, 0x17, 0x60,
];

fn terrain_index(x: u8, y: u8) -> usize {
    usize::from(x) + usize::from(y) * 256
}

fn building_topology_rebuild_required(terrain_type: u8) -> bool {
    matches!(
        terrain_type,
        0 | 0x25 | 0x26 | 0x2c..=0x2f | 0x51 | 0x53 | 0x68 | 0x69 | 0x6d | 0x72 | 0x74
    )
}

fn add_building_to_terrain(
    planes: &mut TerrainPlanes,
    x: u8,
    y: u8,
    map_type: MapType,
    terrain_rng: &mut u16,
) {
    // `AddBuildingToTerrain_46570` unconditionally marks the selected cell
    // and its west, north-west, and north neighbors before rebuilding their
    // topology. Unlike `sub_462A0`, bit 0x80 never suppresses these writes.
    for (mark_x, mark_y) in [
        (x, y),
        (x.wrapping_sub(1), y),
        (x.wrapping_sub(1), y.wrapping_sub(1)),
        (x, y.wrapping_sub(1)),
    ] {
        planes.terrain_types[terrain_index(mark_x, mark_y)] = 1;
    }
    rebuild_marked_building_topology(planes, x, y, 1, 1, map_type, terrain_rng);
}

fn rebuild_building_topology(
    planes: &mut TerrainPlanes,
    start_x: u8,
    start_y: u8,
    end_x: u8,
    end_y: u8,
    map_type: MapType,
    terrain_rng: &mut u16,
) {
    let width = end_x.wrapping_sub(start_x).wrapping_add(1);
    let height = end_y.wrapping_sub(start_y).wrapping_add(1);

    // `sub_462A0` marks every selected cell plus its west, north-west, and
    // north neighbors only when the signed angle byte is non-negative.
    // Material BUILD cells retain bit 0x80 and must not be rewritten here.
    for row in 0..height {
        for column in 0..width {
            let x = start_x.wrapping_add(column);
            let y = start_y.wrapping_add(row);
            for (mark_x, mark_y) in [
                (x, y),
                (x.wrapping_sub(1), y),
                (x.wrapping_sub(1), y.wrapping_sub(1)),
                (x, y.wrapping_sub(1)),
            ] {
                let index = terrain_index(mark_x, mark_y);
                if planes.angles[index].cast_signed() >= 0 {
                    planes.terrain_types[index] = 1;
                }
            }
        }
    }

    rebuild_marked_building_topology(
        planes,
        start_x,
        start_y,
        width,
        height,
        map_type,
        terrain_rng,
    );
}

fn rebuild_marked_building_topology(
    planes: &mut TerrainPlanes,
    start_x: u8,
    start_y: u8,
    width: u8,
    height: u8,
    map_type: MapType,
    terrain_rng: &mut u16,
) {
    let topology_width = width.wrapping_add(1);
    let topology_height = height.wrapping_add(1);
    let topology_start_x = start_x.wrapping_sub(1);
    let topology_start_y = start_y.wrapping_sub(1);

    for row in 0..topology_height {
        for column in 0..topology_width {
            let tile_x = topology_start_x.wrapping_add(column);
            let tile_y = topology_start_y.wrapping_add(row);
            let index = terrain_index(tile_x, tile_y);
            if planes.terrain_types[index] != 1 {
                continue;
            }
            let topology_index = 343 * usize::from(planes.angles[index] & 7)
                + 49 * usize::from(
                    planes.angles[terrain_index(tile_x.wrapping_add(1), tile_y)] & 7,
                )
                + 7 * usize::from(
                    planes.angles[terrain_index(tile_x.wrapping_add(1), tile_y.wrapping_add(1))]
                        & 7,
                )
                + usize::from(planes.angles[terrain_index(tile_x, tile_y.wrapping_add(1))] & 7);
            let [terrain_type, rotation] = building_topology_entry(topology_index);
            planes.terrain_types[index] = terrain_type;
            if terrain_type >= 8 {
                // The decompiled source writes `(angle & 0x87) + rotation`.
                planes.angles[index] = (planes.angles[index] & 0x87).wrapping_add(rotation);
            } else {
                *terrain_rng = terrain_rng.wrapping_mul(9_377).wrapping_add(9_439);
                planes.angles[index] =
                    (planes.angles[index] & 0x87).wrapping_add(16 * (*terrain_rng % 7) as u8);
            }
        }
    }

    // Finally recalculate the one-cell-expanded shading neighborhood in the
    // literal source order. The signed eight-bit temporary deliberately wraps.
    for row in 0..topology_height.wrapping_add(1) {
        for column in 0..topology_width.wrapping_add(1) {
            let tile_x = topology_start_x.wrapping_add(column);
            let tile_y = topology_start_y.wrapping_add(row);
            let down_right = planes.ground_heights
                [terrain_index(tile_x.wrapping_add(1), tile_y.wrapping_add(1))];
            let up_left = planes.ground_heights
                [terrain_index(tile_x.wrapping_sub(1), tile_y.wrapping_sub(1))];
            let mut shade = (32_i16 - i16::from(down_right)) as i8;
            shade = shade.wrapping_add(up_left as i8);
            shade = if shade >= 28 {
                if shade > 40 {
                    (shade & 7).wrapping_add(40)
                } else {
                    shade
                }
            } else {
                (shade & 3).wrapping_add(28)
            };
            let index = terrain_index(tile_x, tile_y);
            planes.shading[index] = if map_type == MapType::Day {
                shade as u8
            } else {
                64_i8.wrapping_sub(shade) as u8
            };
            if map_type == MapType::Cave {
                normalize_cave_clearance(planes, index);
            } else {
                planes.angles[index] &= 0xf7;
            }
        }
    }
}

pub(crate) fn apply_level1_load_path_point(
    planes: &mut TerrainPlanes,
    x: u8,
    y: u8,
    terrain_rng: &mut u16,
) {
    let index = terrain_index(x, y);
    planes.angles[index] = (planes.angles[index] & 0xf0) | 1;
    rebuild_building_topology(planes, x, y, x, y, MapType::Night, terrain_rng);
}

pub(crate) fn level1_load_house_base(
    planes: &TerrainPlanes,
    tile_x: u8,
    tile_y: u8,
    footprint: &CastleBuildFootprint,
) -> (u8, u8, i16) {
    let mut center_x = tile_x;
    let center_y = tile_y;
    let mut start_x = center_x.wrapping_sub(footprint.width / 2);
    let start_y = center_y.wrapping_sub(footprint.height / 2);
    if start_x.wrapping_add(start_y) & 1 != 0 {
        center_x = center_x.wrapping_add(1);
        start_x = start_x.wrapping_add(1);
    }
    let corner_sum = u16::from(planes.ground_heights[terrain_index(start_x, start_y)])
        + u16::from(
            planes.ground_heights[terrain_index(start_x.wrapping_add(footprint.width), start_y)],
        )
        + u16::from(
            planes.ground_heights[terrain_index(
                start_x.wrapping_add(footprint.width),
                start_y.wrapping_add(footprint.height),
            )],
        )
        + u16::from(
            planes.ground_heights[terrain_index(start_x, start_y.wrapping_add(footprint.height))],
        );
    (
        center_x,
        center_y,
        i16::try_from((corner_sum >> 2) * 32).expect("average u8 corner height fits i16"),
    )
}

pub(crate) fn apply_level1_load_house_sweep(
    planes: &mut TerrainPlanes,
    terrain_rng: &mut u16,
    position: Position,
    footprint: &CastleBuildFootprint,
    remaining: u8,
) {
    let center_x = position.x.wrapping_add(128).to_be_bytes()[0];
    let center_y = position.y.wrapping_add(128).to_be_bytes()[0];
    let start_x = center_x.wrapping_sub(footprint.width / 2);
    let start_y = center_y.wrapping_sub(footprint.height / 2);
    let construction_base = position.z >> 5;

    if remaining == 0 {
        for row in 0..footprint.height {
            for column in 0..footprint.width {
                let cell = footprint.cells()
                    [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
                if cell[0] != 0xff {
                    rebuild_building_topology(
                        planes,
                        start_x.wrapping_add(column),
                        start_y.wrapping_add(row),
                        start_x.wrapping_add(column),
                        start_y.wrapping_add(row),
                        MapType::Night,
                        terrain_rng,
                    );
                }
            }
        }
        return;
    }

    for row in 0..footprint.height {
        for column in 0..footprint.width {
            let cell = footprint.cells()
                [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
            if cell[1] == 0xff {
                continue;
            }
            let x = start_x.wrapping_add(column);
            let y = start_y.wrapping_add(row);
            let index = terrain_index(x, y);
            let current = i16::from(planes.ground_heights[index]);
            let target = construction_base + i16::from(cell[1]);
            let next = current.wrapping_add((target - current) / i16::from(remaining));
            planes.ground_heights[index] = next.to_le_bytes()[0];
            if planes.angles[index].trailing_zeros() >= 3 {
                planes.angles[index] = (planes.angles[index] & 0xf0) | 1;
                rebuild_building_topology(planes, x, y, x, y, MapType::Night, terrain_rng);
            }
        }
    }

    if remaining.is_multiple_of(5) || remaining == 1 {
        for row in 0..footprint.height {
            for column in 0..footprint.width {
                let cell = footprint.cells()
                    [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
                if cell[0] == 0xff {
                    continue;
                }
                let x = start_x.wrapping_add(column);
                let y = start_y.wrapping_add(row);
                let index = terrain_index(x, y);
                planes.angles[index] = (planes.angles[index] & 0xf0) | 1;
                planes.terrain_types[index] = 1;
            }
        }
        for row in 0..footprint.height {
            for column in 0..footprint.width {
                let cell = footprint.cells()
                    [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
                if cell[0] != 0xff {
                    apply_level1_house_material(
                        planes,
                        start_x.wrapping_add(column),
                        start_y.wrapping_add(row),
                        column,
                        cell[0],
                        terrain_rng,
                    );
                }
            }
        }
    }
}

pub(crate) fn apply_level1_load_house_edge_smoothing(
    planes: &mut TerrainPlanes,
    position: Position,
    footprint: &CastleBuildFootprint,
) {
    let center_x = position.x.wrapping_add(128).to_be_bytes()[0];
    let center_y = position.y.wrapping_add(128).to_be_bytes()[0];
    let footprint_start_x = center_x.wrapping_sub(footprint.width / 2);
    let footprint_start_y = center_y.wrapping_sub(footprint.height / 2);
    let half_width = footprint.width / 2;
    let half_height = footprint.height / 2;
    for radius in [2_u8, 5] {
        smooth_level1_house_edges(
            planes,
            footprint_start_x,
            footprint_start_y,
            half_width,
            half_height,
            radius,
        );
    }
}

fn smooth_level1_house_edges(
    planes: &mut TerrainPlanes,
    start_x: u8,
    start_y: u8,
    half_width: u8,
    half_height: u8,
    radius: u8,
) {
    let boundary_x = start_x.wrapping_sub(half_width);
    let boundary_y = start_y.wrapping_sub(half_height);
    for row in 0..half_height.wrapping_mul(2) {
        for inset in 0..=radius {
            smooth_level1_house_height(
                planes,
                boundary_x.wrapping_sub(radius).wrapping_add(inset),
                boundary_y.wrapping_add(row),
            );
            smooth_level1_house_height(
                planes,
                boundary_x
                    .wrapping_add(half_width.wrapping_mul(2))
                    .wrapping_add(inset),
                boundary_y.wrapping_add(row),
            );
        }
    }
    for column in 0..half_width
        .wrapping_mul(2)
        .wrapping_add(radius.wrapping_mul(2))
    {
        for inset in 0..=radius {
            smooth_level1_house_height(
                planes,
                boundary_x.wrapping_sub(radius).wrapping_add(column),
                boundary_y.wrapping_sub(radius).wrapping_add(inset),
            );
            smooth_level1_house_height(
                planes,
                boundary_x.wrapping_sub(radius).wrapping_add(column),
                boundary_y
                    .wrapping_add(half_height.wrapping_mul(2))
                    .wrapping_add(inset),
            );
        }
    }
}

fn smooth_level1_house_height(planes: &mut TerrainPlanes, tile_x: u8, tile_y: u8) {
    let index = terrain_index(tile_x, tile_y);
    if planes.angles[index].trailing_zeros() >= 3
        || planes.ground_heights[index] == 0
        || !level1_house_smoothing_type(
            planes.terrain_types[terrain_index(tile_x.wrapping_sub(1), tile_y.wrapping_sub(1))],
        )
        || !level1_house_smoothing_type(
            planes.terrain_types[terrain_index(tile_x, tile_y.wrapping_sub(1))],
        )
        || !level1_house_smoothing_type(
            planes.terrain_types[terrain_index(tile_x.wrapping_sub(1), tile_y)],
        )
        || !level1_house_smoothing_type(planes.terrain_types[index])
    {
        return;
    }
    let mut height_sum = 0_u32;
    let mut height_count = 0_u32;
    for row in 0..3_u8 {
        for column in 0..3_u8 {
            let neighbor = terrain_index(
                tile_x.wrapping_sub(1).wrapping_add(column),
                tile_y.wrapping_sub(1).wrapping_add(row),
            );
            if level1_house_smoothing_type(planes.terrain_types[neighbor]) {
                height_sum += u32::from(planes.ground_heights[neighbor]);
                height_count += 1;
            }
        }
    }
    if height_count != 0 {
        planes.ground_heights[index] =
            u8::try_from(height_sum / height_count).expect("average of u8 heights fits u8");
    }
}

const fn level1_house_smoothing_type(terrain_type: u8) -> bool {
    terrain_type <= 5 || terrain_type > 0x22
}

fn apply_level1_house_material(
    planes: &mut TerrainPlanes,
    x: u8,
    y: u8,
    column: u8,
    material: u8,
    terrain_rng: &mut u16,
) {
    let index = terrain_index(x, y);
    if material < 8 {
        planes.angles[index] = (planes.angles[index] & 0xf0) | material;
        rebuild_building_topology(planes, x, y, x, y, MapType::Night, terrain_rng);
        return;
    }
    match material {
        8 => planes.terrain_types[index] = 8,
        9 => planes.terrain_types[index] = 9,
        16..=19 => {
            let corner = classify_castle_corner(planes, x, y, column);
            let table_offset = match material {
                16 => 2 * corner,
                17 => 0x40 + 2 * corner,
                18 => 0x50 + 2 * (8 * usize::from(x.wrapping_add(y) & 1) + corner),
                19 => 0x60 + 2 * (8 * usize::from(x.wrapping_add(y) & 1) + corner),
                _ => unreachable!(),
            };
            let preserve = material == 16 && matches!(planes.terrain_types[index], 10..=12);
            if !preserve {
                let table = terrain_generation_table();
                planes.terrain_types[index] = table[table_offset];
                planes.angles[index] = (planes.angles[index] & 0x8f) | table[table_offset + 1];
            }
        }
        _ => return,
    }
    planes.angles[index] |= 0x80;
    for (corner_x, corner_y) in [
        (x, y),
        (x.wrapping_add(1), y),
        (x.wrapping_add(1), y.wrapping_add(1)),
        (x, y.wrapping_add(1)),
    ] {
        planes.angles[terrain_index(corner_x, corner_y)] &= 0xf7;
    }
}

pub(crate) fn level1_feature_ground_height(planes: &TerrainPlanes, x: u16, y: u16) -> i16 {
    let bytes = interpolate_ground(&planes.ground_heights, x, y).to_le_bytes();
    i16::from_le_bytes([bytes[0], bytes[1]])
}

fn terrain_generation_table() -> &'static [u8; 0x120] {
    static TABLE: OnceLock<[u8; 0x120]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let source =
            include_str!("../../../../../../upstream/magic-carpet-2-hd/remc2/engine/Unk_D4A30.cpp");
        let initializer = source
            .split_once("{//terrain gen")
            .expect("remc2 terrain-generation initializer")
            .1
            .split_once("};")
            .expect("remc2 terrain-generation terminator")
            .0;
        let initializer = initializer
            .lines()
            .map(|line| line.split_once("//").map_or(line, |(values, _)| values))
            .collect::<String>();
        let bytes = initializer.as_bytes();
        let mut values = Vec::with_capacity(0x120);
        let mut cursor = 0;
        while cursor + 3 < bytes.len() {
            if bytes[cursor] == b'0'
                && bytes[cursor + 1] == b'x'
                && bytes[cursor + 2].is_ascii_hexdigit()
                && bytes[cursor + 3].is_ascii_hexdigit()
            {
                let hex = std::str::from_utf8(&bytes[cursor + 2..cursor + 4])
                    .expect("ASCII hexadecimal terrain-generation byte");
                values.push(
                    u8::from_str_radix(hex, 16).expect("terrain-generation hexadecimal byte"),
                );
                cursor += 4;
            } else {
                cursor += 1;
            }
        }
        assert_eq!(values.len(), 0x120);
        values
            .try_into()
            .expect("terrain-generation table has fixed extent")
    })
}

fn building_topology_entry(index: usize) -> [u8; 2] {
    debug_assert!(index < 7_usize.pow(4));
    let catalog = terrain_corner_catalog();
    for (terrain_type, corners) in catalog.iter().enumerate() {
        if corners.iter().any(|corner| *corner < 0) {
            continue;
        }
        let c = corners.map(|corner| corner as usize);
        for (candidate, rotation) in [
            (343 * c[0] + 49 * c[1] + 7 * c[2] + c[3], 0),
            (343 * c[1] + 49 * c[0] + 7 * c[3] + c[2], 16),
            (343 * c[2] + 49 * c[3] + 7 * c[0] + c[1], 48),
            (343 * c[3] + 49 * c[2] + 7 * c[1] + c[0], 32),
            (343 * c[1] + 49 * c[2] + 7 * c[3] + c[0], 96),
            (343 * c[2] + 49 * c[1] + 7 * c[0] + c[3], 112),
            (343 * c[3] + 49 * c[0] + 7 * c[1] + c[2], 80),
            (343 * c[0] + 49 * c[3] + 7 * c[2] + c[1], 64),
        ] {
            if candidate == index {
                return [terrain_type as u8, rotation];
            }
        }
    }
    [1, 0]
}

fn terrain_corner_catalog() -> &'static [[i8; 4]; 148] {
    static CATALOG: OnceLock<[[i8; 4]; 148]> = OnceLock::new();
    CATALOG.get_or_init(|| {
        // remc2 keeps this functional terrain-topology catalog in a dedicated
        // source table. Parsing its hexadecimal initializer here avoids
        // duplicating a second hand-maintained 592-byte private table.
        let source =
            include_str!("../../../../../../upstream/magic-carpet-2-hd/remc2/engine/Unk_D47E0.cpp");
        let initializer = source
            .split_once("{//terrain gen")
            .expect("remc2 terrain-corner initializer")
            .1
            .split_once("};")
            .expect("remc2 terrain-corner terminator")
            .0;
        let bytes = initializer.as_bytes();
        let mut values = Vec::with_capacity(148 * 4);
        let mut cursor = 0;
        while cursor + 3 < bytes.len() {
            if bytes[cursor] == b'0'
                && bytes[cursor + 1] == b'x'
                && bytes[cursor + 2].is_ascii_hexdigit()
                && bytes[cursor + 3].is_ascii_hexdigit()
            {
                let hex = std::str::from_utf8(&bytes[cursor + 2..cursor + 4])
                    .expect("ASCII hexadecimal terrain corner");
                values.push(u8::from_str_radix(hex, 16).expect("terrain corner byte") as i8);
                cursor += 4;
            } else {
                cursor += 1;
            }
        }
        assert_eq!(values.len(), 148 * 4);
        let mut result = [[0_i8; 4]; 148];
        for (row, values) in result.iter_mut().zip(values.chunks_exact(4)) {
            row.copy_from_slice(values);
        }
        result
    })
}

fn classify_castle_corner(planes: &TerrainPlanes, x: u8, y: u8, fallback: u8) -> usize {
    let heights = [
        planes.ground_heights[terrain_index(x, y)],
        planes.ground_heights[terrain_index(x.wrapping_add(1), y)],
        planes.ground_heights[terrain_index(x.wrapping_add(1), y.wrapping_add(1))],
        planes.ground_heights[terrain_index(x, y.wrapping_add(1))],
    ];
    let mut maximum = 0_u8;
    let mut maximum_corner = 0_usize;
    let mut minimum = u8::MAX;
    for (corner, height) in heights.into_iter().enumerate() {
        if height > maximum {
            maximum = height;
            maximum_corner = corner;
        }
        minimum = minimum.min(height);
    }
    let mut second_maximum = 0_u8;
    let mut second_corner = usize::from(fallback);
    for (corner, height) in heights.into_iter().enumerate() {
        if corner != maximum_corner && height > second_maximum {
            second_maximum = height;
            second_corner = corner;
        }
    }
    if maximum.wrapping_sub(minimum) >= 8 && maximum.wrapping_sub(second_maximum) >= 8 {
        return maximum_corner;
    }
    match maximum_corner {
        0 => {
            if second_corner == 1 {
                4
            } else {
                7
            }
        }
        1 => {
            if second_corner == 2 {
                5
            } else {
                4
            }
        }
        2 => {
            if second_corner == 3 {
                6
            } else {
                5
            }
        }
        3 => {
            if second_corner != 0 {
                6
            } else {
                7
            }
        }
        _ => 0,
    }
}

fn apply_castle_material(
    planes: &mut TerrainPlanes,
    x: u8,
    y: u8,
    material: u8,
    map_type: MapType,
) {
    let index = terrain_index(x, y);
    let preserve_existing_material =
        material == 0x10 && matches!(planes.terrain_types[index], 10 | 11 | 12);
    if !preserve_existing_material {
        let corner = classify_castle_corner(planes, x, y, 7);
        let table_offset = match material {
            0x10 => 2 * corner,
            0x12 => 0x50 + 2 * (8 * usize::from(x.wrapping_add(y) & 1) + corner),
            _ => {
                // BUILD[1] is qualified to contain only 0x10 and 0x12 controls.
                return;
            }
        };
        planes.terrain_types[index] = TERRAIN_GENERATION_TABLE_PREFIX[table_offset];
        planes.angles[index] =
            (planes.angles[index] & 0x8f) | TERRAIN_GENERATION_TABLE_PREFIX[table_offset + 1];
    }
    planes.angles[index] |= 0x80;
    if map_type == MapType::Cave {
        for (corner_x, corner_y) in [
            (x, y),
            (x.wrapping_add(1), y),
            (x.wrapping_add(1), y.wrapping_add(1)),
            (x, y.wrapping_add(1)),
        ] {
            normalize_cave_clearance(planes, terrain_index(corner_x, corner_y));
        }
    } else {
        for (corner_x, corner_y) in [
            (x, y),
            (x.wrapping_add(1), y),
            (x.wrapping_add(1), y.wrapping_add(1)),
            (x, y.wrapping_add(1)),
        ] {
            planes.angles[terrain_index(corner_x, corner_y)] &= 0xf7;
        }
    }
}

fn apply_ambient_material(
    planes: &mut TerrainPlanes,
    x: u8,
    y: u8,
    in_type: u8,
    target_type: u8,
    map_type: MapType,
) -> std::result::Result<(), String> {
    let base_offset = match target_type {
        20 => 0x10,
        21 => 0x20,
        22 => 0x30,
        _ => {
            return Err(format!(
                "ambient terrain target {target_type} is outside 20..=22"
            ));
        }
    };
    let corner = classify_castle_corner(planes, x, y, in_type);
    let table_offset = base_offset + 2 * corner;
    let index = terrain_index(x, y);
    planes.terrain_types[index] = TERRAIN_GENERATION_TABLE_PREFIX[table_offset];
    planes.angles[index] =
        (planes.angles[index] & 0x8f) | TERRAIN_GENERATION_TABLE_PREFIX[table_offset + 1];
    planes.angles[index] |= 0x80;
    for (corner_x, corner_y) in [
        (x, y),
        (x.wrapping_add(1), y),
        (x.wrapping_add(1), y.wrapping_add(1)),
        (x, y.wrapping_add(1)),
    ] {
        let index = terrain_index(corner_x, corner_y);
        if map_type == MapType::Cave {
            normalize_cave_clearance(planes, index);
        } else {
            planes.angles[index] &= 0xf7;
        }
    }
    Ok(())
}

fn apply_ambient_collision(
    planes: &mut TerrainPlanes,
    position: Position,
    height_delta: i8,
    map_type: MapType,
    terrain_rng: &mut u16,
) {
    let base_x = (position.x as i16).wrapping_add(128) >> 8;
    let base_y = (position.y as i16).wrapping_add(128) >> 8;
    for (offset_x, offset_y) in [(0_i16, 0_i16), (1, 0), (0, 1), (1, 1)] {
        let signed_x = base_x.wrapping_add(offset_x);
        let signed_y = base_y.wrapping_add(offset_y);
        let x = signed_x as u8;
        let y = signed_y as u8;
        let index = terrain_index(x, y);
        if planes.angles[index] & 0x80 != 0 {
            break;
        }
        let unclamped = i16::from(planes.ground_heights[index]) + i16::from(height_delta);
        let height = unclamped.clamp(0, 200) as u8;
        planes.ground_heights[index] = height;
        if map_type == MapType::Cave {
            let secondary = i16::from(planes.secondary_heights[index]) - i16::from(height_delta);
            planes.secondary_heights[index] = if secondary >= 255 {
                u8::MAX
            } else {
                secondary as u8
            };
        }
        if height != 0 {
            planes.angles[index] = (planes.angles[index] & 0xf8) | 1;
        } else {
            let allowed = [
                (x.wrapping_sub(1), y.wrapping_sub(1)),
                (x, y.wrapping_sub(1)),
                (x.wrapping_add(1), y.wrapping_sub(1)),
                (x.wrapping_add(1), y),
                (x.wrapping_sub(1), y),
                (x.wrapping_sub(1), y.wrapping_add(1)),
                (x, y.wrapping_add(1)),
                (x.wrapping_add(1), y.wrapping_add(1)),
            ]
            .into_iter()
            .all(|(x, y)| {
                let angle = planes.angles[terrain_index(x, y)] & 7;
                !matches!(angle, 2 | 3 | 5)
            });
            if allowed {
                planes.angles[index] &= 0xf0;
            }
        }
        add_building_to_terrain(planes, x, y, map_type, terrain_rng);
        if !(0..=200).contains(&unclamped) && signed_x == 0 && signed_y == 0 {
            break;
        }
    }
}

fn normalize_cave_clearance(planes: &mut TerrainPlanes, index: usize) {
    if planes.secondary_heights[index] > planes.ground_heights[index] {
        planes.angles[index] &= 0xf7;
    } else {
        planes.secondary_heights[index] = planes.ground_heights[index].wrapping_sub(1);
        planes.angles[index] |= 0x08;
    }
}

fn remove_castle_stage(
    planes: &mut TerrainPlanes,
    start_x: u8,
    start_y: u8,
    footprint: &CastleBuildFootprint,
    map_type: MapType,
    terrain_rng: &mut u16,
    mut entity_rng: u16,
) {
    for row in 0..footprint.height {
        for column in 0..footprint.width {
            let cell = footprint.cells()
                [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
            if cell[0] == 0xff && cell[1] == 0xff {
                continue;
            }
            let x = start_x.wrapping_add(column);
            let y = start_y.wrapping_add(row);
            let index = terrain_index(x, y);
            planes.angles[index] = (planes.angles[index] & 0x70) | 1;
            add_building_to_terrain(planes, x, y, map_type, terrain_rng);
            if cell[1] == 0xff {
                continue;
            }
            let offset = cell[1];
            let height = planes.ground_heights[index];
            if offset >= height {
                planes.ground_heights[index] = 0;
                continue;
            }
            entity_rng = entity_rng.wrapping_mul(9_377).wrapping_add(9_439);
            if entity_rng % 50 <= 20 {
                planes.ground_heights[index] = height.wrapping_sub(offset);
            } else {
                entity_rng = entity_rng.wrapping_mul(9_377).wrapping_add(9_439);
                planes.ground_heights[index] = height
                    .wrapping_sub(offset)
                    .wrapping_add((entity_rng % 20) as u8);
            }
        }
    }
    for row in 0..footprint.height {
        for column in 0..footprint.width {
            smooth_building_height(
                planes,
                start_x.wrapping_add(column),
                start_y.wrapping_add(row),
                map_type,
            );
        }
    }
}

const fn next_house_rng(seed: u16) -> u16 {
    seed.wrapping_mul(9_377).wrapping_add(9_439)
}

fn smooth_building_height(planes: &mut TerrainPlanes, x: u8, y: u8, map_type: MapType) {
    let index = terrain_index(x, y);
    let ordinary = |terrain_type: u8| terrain_type <= 5 || terrain_type > 0x22;
    if planes.angles[index] & 7 == 0
        || planes.ground_heights[index] == 0
        || !ordinary(planes.terrain_types[terrain_index(x.wrapping_sub(1), y.wrapping_sub(1))])
        || !ordinary(planes.terrain_types[terrain_index(x, y.wrapping_sub(1))])
        || !ordinary(planes.terrain_types[terrain_index(x.wrapping_sub(1), y)])
        || !ordinary(planes.terrain_types[index])
    {
        return;
    }
    let mut height_sum = 0_u32;
    let mut height_count = 0_u32;
    for row_offset in [u8::MAX, 0, 1] {
        for column_offset in [u8::MAX, 0, 1] {
            let neighbor = terrain_index(x.wrapping_add(column_offset), y.wrapping_add(row_offset));
            if ordinary(planes.terrain_types[neighbor]) {
                height_sum = height_sum.wrapping_add(u32::from(planes.ground_heights[neighbor]));
                height_count += 1;
            }
        }
    }
    if height_count != 0 {
        planes.ground_heights[index] = (height_sum / height_count) as u8;
        if map_type == MapType::Cave {
            normalize_cave_clearance(planes, index);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeRelation {
    Scalar(u32),
    AuthoredFeature(u16),
    LiveEntity(u16),
    LiveEntityIndex(u16),
    AxisScalar(u32),
    Null,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStage {
    pub kind: i8,
    pub flags: i8,
    pub axis_x: i16,
    pub axis_y: i16,
    pub relation: RuntimeRelation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeStageVariable {
    pub kind: i8,
    pub flags: i8,
    pub word_2: u16,
    pub relation: RuntimeRelation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSnapshot {
    pub global_turn: u32,
    pub random_state: u32,
    pub local_player_slot: u8,
    pub player_count: u8,
    pub maptype_music: i32,
    pub stage_index: u8,
    /// `byte_0x36E02`, consumed by remc2's objective-presentation service.
    pub objective_presentation_phase: u8,
    /// Remaining objective-textbox presentation ticks. The atlas identifies
    /// source byte `byte_counter_current_objective_box_0x36E04` as a countdown,
    /// not an objective row selector.
    pub objective_textbox_ticks: u8,
    pub stages: Box<[RuntimeStage]>,
    pub stage_variables: Box<[RuntimeStageVariable]>,
    pub objectives: Box<[RuntimeObjective]>,
    pub player_inputs: [PlayerCommand; PLAYER_COUNT],
    bytes: Box<[u8]>,
}

impl RuntimeSnapshot {
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeObjective {
    pub is_level_end: u8,
    pub text_selector: u8,
    pub done: u8,
    pub stage_selectors: [u8; 8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParticleParameter {
    pub base_tmap: u16,
    pub word_2: u16,
    pub word_4: u16,
    pub speed: u16,
    pub rotation_speed: u16,
    pub byte_10: u8,
    pub byte_11: i8,
    pub render_mode: u8,
    pub byte_13: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationSelectors {
    pub particles: Box<[ParticleParameter]>,
    pub level_sprite_selectors: Box<[u8]>,
    pub entity_parameters: Box<[[u8; ENTITY_PARAMETER_BYTES]]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameObservation {
    pub ordinal: u16,
    pub category: String,
    pub hook: String,
    pub stage: Option<String>,
    pub selector: Value,
    pub event_sha256: String,
    pub event: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetRef {
    pub asset_id: String,
    pub content_address: String,
    pub owner_asset_ids: Box<[String]>,
    pub observation: FrameObservation,
}

/// An immutable, validated owner-data world.
#[derive(Clone, Debug)]
pub struct NativeWorld {
    metadata: LevelMetadata,
    package_sha256: String,
    closure_sha256: String,
    profile: FlightProfile,
    terrain_planes: TerrainPlanes,
    terrain: LegacyTerrain,
    entities: Box<[EntitySnapshot]>,
    players: Box<[PlayerSnapshot]>,
    runtime: RuntimeSnapshot,
    presentation: PresentationSelectors,
    asset_refs: Box<[AssetRef]>,
    sound_asset_refs: BTreeMap<u16, u32>,
    music_asset_ref: Option<u32>,
    goat_tmap_bank: Option<GoatTmapBank>,
    source_sprite_tmap_bank: Option<SourceSpriteTmapBank>,
    reset_source_map_chains: RenderableMapChains,
    raw_source_entity_indices: Box<[u16]>,
    house_build_footprints: BTreeMap<u8, CastleBuildFootprint>,
    reset_state: FlightState,
    reset_context: FlightContext,
    goat_stage_point: Position,
    goat_config: GoatBehaviorConfig,
    reset_live_world: LiveWorldState,
    level1_mission: Level1MissionTemplate,
}

/// Common owner data decoded for a profile before mission-specific behavior is
/// constructed. Level 2 uses this inspection seam while its mission dispatch
/// remains intentionally unsupported; it shares the same typed component
/// arenas and parsers as Level 1.
#[derive(Clone, Debug)]
pub struct NativeWorldInspection {
    profile: OwnerPackageProfile,
    schema: OwnerPackageSchema,
    metadata: LevelMetadata,
    package_sha256: String,
    terrain_planes: TerrainPlanes,
    terrain: LegacyTerrain,
    entities: Box<[EntitySnapshot]>,
    players: Box<[PlayerSnapshot]>,
    runtime: RuntimeSnapshot,
    presentation: PresentationSelectors,
    terrain_rng: u16,
    reset_state: Option<FlightState>,
    authored_level: Box<[u8]>,
}

impl NativeWorldInspection {
    #[must_use]
    pub const fn schema(&self) -> OwnerPackageSchema {
        self.schema
    }

    #[must_use]
    pub const fn profile(&self) -> OwnerPackageProfile {
        self.profile
    }

    #[must_use]
    pub const fn metadata(&self) -> &LevelMetadata {
        &self.metadata
    }

    #[must_use]
    pub const fn package_sha256(&self) -> &str {
        self.package_sha256.as_str()
    }

    #[must_use]
    pub const fn terrain_planes(&self) -> &TerrainPlanes {
        &self.terrain_planes
    }

    #[must_use]
    pub const fn terrain(&self) -> &LegacyTerrain {
        &self.terrain
    }

    #[must_use]
    pub fn entities(&self) -> &[EntitySnapshot] {
        &self.entities
    }

    #[must_use]
    pub fn players(&self) -> &[PlayerSnapshot] {
        &self.players
    }

    #[must_use]
    pub const fn runtime(&self) -> &RuntimeSnapshot {
        &self.runtime
    }

    #[must_use]
    pub const fn presentation(&self) -> &PresentationSelectors {
        &self.presentation
    }

    #[must_use]
    pub const fn terrain_rng(&self) -> u16 {
        self.terrain_rng
    }

    #[must_use]
    pub const fn reset_state(&self) -> Option<&FlightState> {
        self.reset_state.as_ref()
    }

    /// Select the typed mission profile from metadata already checked by the
    /// owner-package loader. Unknown metadata cannot fall back to Level 1.
    pub fn mission_content_profile(&self) -> Result<MissionContentProfile> {
        MissionContentProfile::from_metadata(
            self.metadata.external_level,
            self.metadata.internal_level,
            self.metadata.map_type.id(),
        )
        .map_err(|error| NativeWorldError::new(error.to_string()))
    }

    /// Build the generic authored/reset template from the qualified package.
    /// This retains all 1,200 authored records and all 1,000 reset slots before
    /// any Level 2 behavior dispatcher is selected.
    pub fn mission_content_template(&self) -> Result<MissionContentTemplate> {
        let profile = self.mission_content_profile()?;
        let slots = self
            .entities
            .iter()
            .map(|entity| Level1ResetSourceSlot {
                index: entity.index,
                active: entity.active,
                class: entity.class,
                model: entity.model,
                action: entity.action,
                phase: entity.phase,
                entity_rng: entity.entity_rng,
                control_flags: entity.control_flags,
            })
            .collect::<Vec<_>>();
        MissionContentTemplate::decode(
            profile,
            &self.authored_level,
            &slots,
            self.runtime.random_state,
        )
        .map_err(|error| NativeWorldError::new(error.to_string()))
    }

    /// Construct an isolated deterministic session from the generic profile.
    pub fn mission_content_session(&self) -> Result<MissionContentSession> {
        Ok(self.mission_content_template()?.session())
    }

    /// Build the profile-specific Level 2 frame-zero session. All mutable
    /// state is cloned from this validated inspection; no implementation
    /// fixture or stale Level 1 reset projection is consulted.
    pub fn level2_reset_session(&self) -> Result<Level2ResetSession> {
        if self.profile != OwnerPackageProfile::Level2 {
            return Err(NativeWorldError::new(
                "Level 2 reset session requested for a non-Level 2 package",
            ));
        }
        let state = self
            .reset_state
            .ok_or_else(|| NativeWorldError::new("validated Level 2 package has no reset state"))?;
        let mission = self.mission_content_template()?.session();
        let bad_stones = level2_bad_stones(&self.entities, &self.terrain_planes)?;
        let shared_spells = Level2SharedSpellResetSession::from_parts(
            SessionTerrain::from_inspection(self),
            &self.entities,
            &self.players,
            &self.presentation,
        )?;
        let ordinary = Level2OrdinaryResetSession::from_parts(
            SessionTerrain::from_inspection(self),
            self.entities.clone(),
            self.players.clone(),
            self.presentation.clone(),
        )?;
        let economy = Level2EconomyState::from_parts(
            SessionTerrain::from_inspection(self),
            &self.entities,
            self.entities[usize::from(self.players[0].entity_index)].mana,
            self.entities[usize::from(self.players[0].entity_index)].model,
            0,
            self.players[0].enabled_spell_holders[2],
        )?;
        Ok(Level2ResetSession::new(
            self.metadata.map_type.profile(),
            state,
            SessionTerrain::from_inspection(self),
            self.entities.clone(),
            self.players.clone(),
            self.runtime.clone(),
            self.presentation.clone(),
            bad_stones,
            shared_spells,
            economy,
            ordinary,
            mission,
        ))
    }

    /// Mission construction is intentionally fail-closed until the Level 2
    /// content profile is implemented by the later mission beads.
    pub fn mission_unimplemented(&self) -> Result<()> {
        Err(NativeWorldError::new(format!(
            "mission behavior is unsupported for {:?} owner profile",
            self.profile
        )))
    }
}

/// Production Level 2 reset projection for the ordinary resident/static
/// families.  It owns the shared `HouseFamily`/`VillagerFamily` behavior and
/// the three typed dolmens; no Level 1 slot assumptions are consulted.
const LEVEL2_SHARED_SPELL_PICKUP_SLOTS: [u16; 2] = [38, 75];
const LEVEL2_SHARED_SPELL_HOLDER_SLOTS: [u16; 2] = [113, 114];
const LEVEL2_SHARED_SPELL_ENTITY_SHA256: [&str; 4] = [
    "21f016732b7a687ac53cd9fc452a148dce204e4944b30755abc2f312c5460182",
    "899b6f9f0c614cdb77c1c7c3e7922ac6eadd03391fdd252f347b29d93fee3e81",
    "9312c195108896b32d553bdce73a43484bf80e010fcf762fc7c01d7f7ecca954",
    "fedeb2b2fbe40c6ec72614bc4a971e2e826d52a8348156040e872c14a7c0a80e",
];
const LEVEL2_SHARED_SPELL_PLAYER_ENTITY_SHA256: &str =
    "b5234d405349902b3ef653ed52ac175c929b6024a19869a503fbe65f190d3834";

/// Level 2's shared spell/economy reset projection. The two active authored
/// pickups and the AI's Fireball/Possession holders are retained as typed,
/// source-ordered records; Shield, Invisible, and Beyond Sight are deliberately
/// not admitted here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level2SharedSpellResetSession {
    baseline_terrain: SessionTerrain,
    terrain: SessionTerrain,
    baseline_pickups: Box<[EntitySnapshot]>,
    pickups: Box<[EntitySnapshot]>,
    baseline_holders: Box<[EntitySnapshot]>,
    holders: Box<[EntitySnapshot]>,
    baseline_player_mana: i32,
    player_mana: i32,
    baseline_player_entity: EntitySnapshot,
    player_entity: EntitySnapshot,
    baseline_local_player: PlayerSnapshot,
    local_player: PlayerSnapshot,
    baseline_speed_pickup_state: Level2SpeedPickupState,
    speed_pickup_state: Level2SpeedPickupState,
    baseline_caster: SpellCasterState,
    caster: SpellCasterState,
    baseline_fireball: SpellActivationHolder,
    fireball: SpellActivationHolder,
    baseline_possession: SpellActivationHolder,
    possession: SpellActivationHolder,
    baseline_speed: SpellActivationHolder,
    speed: SpellActivationHolder,
    player_entity_sha256: String,
    events: Vec<FlightEvent>,
    pickup_events: Vec<Level2SharedSpellEvent>,
}

/// Mutable ownership projection for the authored Speed pickup. These fields
/// are the source's owner/collected/visible/state-timer writes that do not fit
/// the lossless reset entity record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2SpeedPickupState {
    pub owner_entity_index: u16,
    pub collected: bool,
    pub visible: bool,
    pub state_timer: u8,
}

impl Default for Level2SpeedPickupState {
    fn default() -> Self {
        Self {
            owner_entity_index: 0,
            collected: false,
            visible: true,
            state_timer: 0,
        }
    }
}

/// Typed mission event emitted before the pickup's sound preparation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level2SharedSpellEvent {
    SpellCollected {
        source_slot: u16,
        player_entity_index: u16,
        family: u8,
    },
}

/// One source-turn result for the Level 2 authored Speed pickup (slot 75).
///
/// The reset row is a world pickup (`class 15/model 3/action 10`), so the
/// source converts the same entity into the player's holder (`action 9`).
/// There is no respawning variant in the Level 2 reset census; replacement
/// allocation therefore remains deliberately absent from this result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2SpeedPickupStep {
    pub source_slot: u16,
    pub player_entity_index: u16,
    pub previous_action: u8,
    pub action: u8,
    pub previous_phase: u8,
    pub phase: u8,
    pub collected: bool,
}

/// Mutable Level 2 Castle root fields shared by the mana/carrier seam. Castle
/// construction animation and defense dispatch remain separate Level 2 work;
/// this type owns the source life/mana/damage mailbox and kill credit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2CastleState {
    pub source_entity_index: u16,
    /// Source `id_0x1A_26` copied into Castle child records. This is kept
    /// separate from the player/owner relation used by economy arbitration.
    pub source_id_entity_index: u16,
    pub source_class: u8,
    pub source_model: u8,
    pub source_byte_38: u8,
    pub source_particle_parameter: u16,
    pub owner_entity_index: u16,
    pub position: Position,
    /// Source `axis_0x9A_154x`: the construction/child anchor is distinct
    /// from the root's terrain-synchronised position.
    pub anchor_position: Position,
    pub life: i32,
    pub max_life: i32,
    pub mana: i32,
    pub mana_capacity: i32,
    pub pending_damage: i32,
    pub pending_attacker_entity_index: u16,
    pub killer_entity_index: u16,
    pub control_flags: u32,
    pub timer: u8,
    pub phase: u8,
    /// Root action (`4` stable, `5` construction, `6` destruction) and the
    /// source construction substate used by `BeginOfCastleCreation_5FA70`.
    pub action: u8,
    pub build_state: u8,
    pub level: u8,
    pub construction_slot: u16,
    pub defense_slot: u16,
    pub construction_model: u8,
    pub construction_action: u8,
    pub construction_control_flags: u32,
    pub construction_parent_slot: u16,
    pub construction_owner_entity_index: u16,
    pub construction_id: u16,
    pub construction_position: Position,
    pub defense_model: u8,
    pub defense_action: u8,
    pub defense_control_flags: u32,
    pub defense_parent_slot: u16,
    pub defense_owner_entity_index: u16,
    pub defense_id: u16,
    pub defense_position: Position,
    pub child_chain_slots: [u16; 2],
    pub child_chain_models: [u8; 2],
    pub child_chain_active: [bool; 2],
    /// Source `word_0x34_52` head/tail and ordered model-79 chain created by
    /// `sub_613D0`.  The fixed array covers the largest pinned stage (eight
    /// controllers) without introducing an allocation into this Copy state.
    pub controller_chain_slots: [u16; 8],
    pub controller_chain_count: u8,
    pub controller_chain_head: u16,
    pub controller_chain_tail: u16,
    pub construction_child_active: bool,
    pub defense_child_active: bool,
    pub active: bool,
}

/// One source-linked construction/defense child record. The root keeps the
/// current slots for quick dispatch, while this ordered chain mirrors the
/// source `next_0` walk used by state 4's model-42 scan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2CastleChild {
    pub source_entity_index: u16,
    pub id_entity_index: u16,
    pub parent_entity_index: u16,
    pub owner_entity_index: u16,
    pub position: Position,
    pub model: u8,
    pub action: u8,
    pub control_flags: u32,
    /// Source `word_0x32_50` / `word_0x34_52` links.  `chain_previous_slot`
    /// is the root or prior model-79 child; `chain_next_slot` is updated on
    /// that predecessor when the next child is committed.
    pub chain_previous_slot: u16,
    pub chain_next_slot: u16,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level2EconomyEvent {
    ManaSphereSpawned {
        sphere_id: u32,
        source_entity_index: u16,
        mana: i32,
    },
    ManaSphereClaimed {
        sphere_id: u32,
        owner_entity_index: u16,
    },
    ManaSphereCollected {
        sphere_id: u32,
        carrier_id: u32,
        mana: i32,
    },
    ManaSpheresMerged {
        target_sphere_id: u32,
        source_sphere_id: u32,
        owner_entity_index: u16,
        mana: i32,
    },
    ManaCarrierSpawned {
        source_entity_index: u16,
        owner_entity_index: u16,
    },
    ManaCarrierArrived {
        carrier_id: u32,
        castle_id: u16,
        mana: i32,
    },
    CastleBuilt {
        source_entity_index: u16,
        owner_entity_index: u16,
    },
    CastleDestroyed {
        source_entity_index: u16,
        killer_entity_index: u16,
    },
    CastleConstructionSound {
        source_entity_index: u16,
        sound_id: u16,
    },
    CastleDestructionSound {
        source_entity_index: u16,
        sound_id: u16,
    },
    CastleStageDestroyed {
        source_entity_index: u16,
        previous_level: u8,
        next_level: u8,
    },
    DamageApplied {
        target_entity_index: u16,
        attacker_entity_index: u16,
        damage: i32,
        lethal: bool,
    },
}

/// Production Level 2 economy projection for the shared mana sphere, carrier,
/// Castle, damage, and bounded source allocation paths. The source arena free
/// stack is reconstructed from the validated reset rows; failed allocations
/// leave arena/object fields unchanged after the source's ordered dying-row
/// RNG and owner writes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level2EconomyState {
    baseline_terrain: SessionTerrain,
    terrain: SessionTerrain,
    baseline_free_slots: Box<[u16]>,
    free_slots: Vec<u16>,
    baseline_slot_rng: BTreeMap<u16, u16>,
    slot_rng: BTreeMap<u16, u16>,
    baseline_source_rng: BTreeMap<u16, u16>,
    source_rng: BTreeMap<u16, u16>,
    baseline_source_owner: BTreeMap<u16, u16>,
    source_owner: BTreeMap<u16, u16>,
    baseline_owner_class: BTreeMap<u16, u8>,
    owner_class: BTreeMap<u16, u8>,
    baseline_owner_max_mana: BTreeMap<u16, i32>,
    owner_max_mana: BTreeMap<u16, i32>,
    baseline_spheres: Vec<ManaSphere>,
    spheres: Vec<ManaSphere>,
    baseline_carrier: Option<ManaCarrier>,
    carrier: Option<ManaCarrier>,
    baseline_castle: Option<Level2CastleState>,
    castle: Option<Level2CastleState>,
    baseline_castle_children: Vec<Level2CastleChild>,
    castle_children: Vec<Level2CastleChild>,
    baseline_map_chains: RenderableMapChains,
    map_chains: RenderableMapChains,
    baseline_player_mana: i32,
    player_mana: i32,
    baseline_player_castle_entity_index: u16,
    player_castle_entity_index: u16,
    baseline_player_castle_stage: u8,
    player_castle_stage: u8,
    baseline_player_castle_aux_flags: u8,
    player_castle_aux_flags: u8,
    baseline_player_model: u8,
    player_model: u8,
    baseline_player_terrain_flags: u8,
    player_terrain_flags: u8,
    baseline_player_spell_slot_2: u16,
    player_spell_slot_2: u16,
    /// Imported `sub_60810` life coefficient per Castle stage. The owner
    /// export does not expose the nested player-state array, so fixed-profile
    /// sessions default to the source identity coefficient (256); callers
    /// preserving those bytes can bind each stage explicitly.
    baseline_castle_stage_coefficients: [u16; 8],
    castle_stage_coefficients: [u16; 8],
    events: Vec<Level2EconomyEvent>,
}

impl Level2EconomyState {
    fn from_parts(
        terrain: SessionTerrain,
        entities: &[EntitySnapshot],
        player_mana: i32,
        player_model: u8,
        player_terrain_flags: u8,
        player_spell_slot_2: u16,
    ) -> Result<Self> {
        let mut free_slots = entities
            .iter()
            .filter(|entity| !entity.active && entity.index != 0)
            .map(|entity| entity.index)
            .collect::<Vec<_>>();
        free_slots.sort_unstable_by(|left, right| right.cmp(left));
        let slot_rng = entities
            .iter()
            .filter(|entity| !entity.active && entity.index != 0)
            .map(|entity| (entity.index, entity.entity_rng))
            .collect::<BTreeMap<_, _>>();
        let owner_class = entities
            .iter()
            .filter(|entity| entity.active)
            .map(|entity| (entity.index, entity.class))
            .collect::<BTreeMap<_, _>>();
        let source_owner = entities
            .iter()
            .map(|entity| {
                (
                    entity.index,
                    u16::from_le_bytes([entity.record[44], entity.record[45]]),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let owner_max_mana = entities
            .iter()
            .filter(|entity| entity.active)
            .map(|entity| (entity.index, entity.mana))
            .collect::<BTreeMap<_, _>>();
        let raw_map_entries =
            raw_source_map_chain_entries(&terrain.planes().entity_indices, |source_entity_index| {
                entities
                    .get(usize::from(source_entity_index))
                    .map(EntitySnapshot::old_map_entity)
            })
            .map_err(NativeWorldError::new)?;
        let map_seeds = raw_map_entries
            .iter()
            .map(|entry| {
                let entity = &entities[usize::from(entry.entity_index)];
                RenderableMapSeed {
                    entity_index: entry.entity_index,
                    active: entity.active,
                    x: entity.position.x,
                    y: entity.position.y,
                }
            })
            .collect::<Vec<_>>();
        let map_chains = RenderableMapChains::from_raw_reset(
            &map_seeds,
            &terrain.planes().entity_indices,
            |source_entity_index| {
                entities
                    .get(usize::from(source_entity_index))
                    .map(EntitySnapshot::old_map_entity)
            },
            |source_entity_index| {
                entities
                    .get(usize::from(source_entity_index))
                    .map(EntitySnapshot::next_entity)
            },
        )
        .map_err(NativeWorldError::new)?;
        Ok(Self {
            baseline_terrain: terrain.clone(),
            terrain,
            baseline_free_slots: free_slots.clone().into_boxed_slice(),
            free_slots,
            baseline_slot_rng: slot_rng.clone(),
            slot_rng,
            baseline_source_rng: BTreeMap::new(),
            source_rng: BTreeMap::new(),
            baseline_source_owner: source_owner.clone(),
            source_owner,
            baseline_owner_class: owner_class.clone(),
            owner_class,
            baseline_owner_max_mana: owner_max_mana.clone(),
            owner_max_mana,
            baseline_spheres: Vec::new(),
            spheres: Vec::new(),
            baseline_carrier: None,
            carrier: None,
            baseline_castle: None,
            castle: None,
            baseline_castle_children: Vec::new(),
            castle_children: Vec::new(),
            baseline_map_chains: map_chains.clone(),
            map_chains,
            baseline_player_mana: player_mana,
            player_mana,
            baseline_player_castle_entity_index: 0,
            player_castle_entity_index: 0,
            baseline_player_castle_stage: 0,
            player_castle_stage: 0,
            baseline_player_castle_aux_flags: 0,
            player_castle_aux_flags: 0,
            baseline_castle_stage_coefficients: [256; 8],
            castle_stage_coefficients: [256; 8],
            events: Vec::new(),
        })
    }

    fn allocate_slot(&mut self) -> Option<u16> {
        self.free_slots.pop()
    }

    fn release_slot(&mut self, slot: u16, entity_rng: Option<u16>) {
        if slot == 0 || self.free_slots.contains(&slot) {
            return;
        }
        if let Some(entity_rng) = entity_rng {
            self.slot_rng.insert(slot, entity_rng);
        }
        self.free_slots.push(slot);
        self.free_slots
            .sort_unstable_by(|left, right| right.cmp(left));
    }

    #[must_use]
    pub fn spheres(&self) -> &[ManaSphere] {
        &self.spheres
    }

    #[must_use]
    pub fn carrier(&self) -> Option<&ManaCarrier> {
        self.carrier.as_ref().filter(|carrier| carrier.active)
    }

    #[must_use]
    pub const fn castle(&self) -> Option<Level2CastleState> {
        self.castle
    }

    #[must_use]
    pub fn castle_children(&self) -> &[Level2CastleChild] {
        &self.castle_children
    }

    #[must_use]
    pub const fn map_chains(&self) -> &RenderableMapChains {
        &self.map_chains
    }

    #[must_use]
    pub const fn player_mana(&self) -> i32 {
        self.player_mana
    }

    #[must_use]
    pub const fn player_castle_entity_index(&self) -> u16 {
        self.player_castle_entity_index
    }

    #[must_use]
    pub const fn player_castle_stage(&self) -> u8 {
        self.player_castle_stage
    }

    #[must_use]
    pub const fn player_castle_aux_flags(&self) -> u8 {
        self.player_castle_aux_flags
    }

    #[must_use]
    pub const fn free_slot_count(&self) -> usize {
        self.free_slots.len()
    }

    #[must_use]
    pub fn events(&self) -> &[Level2EconomyEvent] {
        &self.events
    }

    /// Start a new source turn's economy event stream. Mutations append to
    /// this stream so one turn can be audited in source order.
    pub fn begin_turn(&mut self) {
        self.events.clear();
    }

    /// Override the imported owner capacity used by the source's
    /// `sub_36D50` merge arbitration when two non-neutral owners collide.
    pub fn set_owner_max_mana(&mut self, owner_entity_index: u16, max_mana: i32) {
        self.owner_max_mana.insert(owner_entity_index, max_mana);
    }

    /// Bind one imported `sub_60810` life coefficient. `256` is the source
    /// identity coefficient; callers with the preserved nested player-state
    /// bytes can provide the exact value for each stage.
    pub fn set_castle_stage_coefficient(&mut self, level: u8, coefficient: u16) -> bool {
        let Some(value) = self.castle_stage_coefficients.get_mut(usize::from(level)) else {
            return false;
        };
        *value = coefficient;
        true
    }

    #[must_use]
    pub fn source_owner(&self, source_entity_index: u16) -> u16 {
        self.source_owner
            .get(&source_entity_index)
            .copied()
            .unwrap_or_default()
    }

    /// Source `TransformEntityToManaSphere_36BA0` with `use_mana_fraction`
    /// false. Allocation failure occurs after the dying source RNG write and
    /// does not consume a free slot or create a partial sphere.
    pub fn spawn_mana_sphere_from_death(
        &mut self,
        source_entity_index: u16,
        position: Position,
        source_yaw: i16,
        source_rng_after_death: u16,
        mana: i32,
        owner_entity_index: u16,
    ) -> Option<u32> {
        self.spawn_mana_spheres_from_death(
            source_entity_index,
            position,
            source_yaw,
            source_rng_after_death,
            mana,
            owner_entity_index,
            false,
        )
        .into_iter()
        .next()
    }

    /// Source `TransformEntityToManaSphere_36BA0`. With `use_mana_fraction`
    /// enabled, mana is split into `clamp(mana / 1000, 1, 16)` spheres and the
    /// final sphere receives the exact remainder. The source writes the dying
    /// entity RNG before attempting allocations, so an exhausted arena may
    /// return a partial vector while preserving the already-created spheres.
    pub fn spawn_mana_spheres_from_death(
        &mut self,
        source_entity_index: u16,
        position: Position,
        source_yaw: i16,
        source_rng_after_death: u16,
        mana: i32,
        owner_entity_index: u16,
        use_mana_fraction: bool,
    ) -> Vec<u32> {
        if mana <= 0 {
            return Vec::new();
        }
        self.source_rng
            .insert(source_entity_index, source_rng_after_death);
        // TransformEntityToManaSphere_36BA0 clears the dying source's owner
        // relation after the sphere loop, even when the arena is exhausted.
        self.source_owner.insert(source_entity_index, 0);
        let fraction = if use_mana_fraction {
            (mana / 1_000).clamp(1, 16)
        } else {
            1
        };
        let each_mana = mana / fraction;
        let mut sphere_ids = Vec::with_capacity(usize::try_from(fraction).unwrap_or(1));
        for sphere_index in 0..fraction {
            let Some(sphere_slot) = self.allocate_slot() else {
                break;
            };
            let sphere_seed = self.slot_rng.get(&sphere_slot).copied().unwrap_or(0);
            let sphere_rng = level2_next_rng16(sphere_seed);
            let yaw =
                i16::try_from((i32::from(sphere_rng % 0x71) + i32::from(source_yaw) - 56) & 0x07ff)
                    .expect("Level 2 mana sphere yaw fits 11 bits");
            let speed_rng = level2_next_rng16(sphere_rng);
            let speed = i16::try_from(speed_rng % 0x30 + 16).expect("mana sphere speed fits i16");
            let mut velocity = Position::default();
            crate::move_entity(&mut velocity, yaw, 0, speed);
            let ground = self.terrain.ground_height(position);
            let height_above_ground = i32::from(position.z) - i32::from(ground);
            let vertical_velocity = (1_024 - height_above_ground).wrapping_div(8) as i16;
            let sphere_mana = if sphere_index + 1 == fraction {
                mana - each_mana * (fraction - 1)
            } else {
                each_mana
            };
            let sphere = ManaSphere {
                id: u32::from(sphere_slot),
                source_entity_index: sphere_slot,
                entity_rng: speed_rng,
                position,
                velocity_x: velocity.x.cast_signed(),
                velocity_y: velocity.y.cast_signed(),
                vertical_velocity,
                yaw,
                mana: sphere_mana,
                owner_entity_index,
                incoming_owner_entity_index: 0,
                incoming_possession_payload: 0,
                assigned_carrier_id: 0,
                phase: sphere_slot as u8,
                active: true,
                pending_delete: false,
            };
            sphere_ids.push(sphere.id);
            self.spheres.push(sphere);
            self.events.push(Level2EconomyEvent::ManaSphereSpawned {
                sphere_id: sphere.id,
                source_entity_index,
                mana: sphere_mana,
            });
        }
        sphere_ids
    }

    pub fn claim_mana_sphere(&mut self, sphere_slot: u16, owner_entity_index: u16) -> bool {
        let Some(sphere) = self
            .spheres
            .iter_mut()
            .find(|sphere| sphere.active && sphere.source_entity_index == sphere_slot)
        else {
            return false;
        };
        if sphere.owner_entity_index == owner_entity_index {
            return false;
        }
        sphere.owner_entity_index = owner_entity_index;
        self.events.push(Level2EconomyEvent::ManaSphereClaimed {
            sphere_id: sphere.id,
            owner_entity_index,
        });
        true
    }

    /// Source `sub_36D50`: merge one active mana sphere into another and
    /// preserve owner arbitration. Neutral ownership adopts the other owner;
    /// class-10 owners yield to a non-class-10 owner; otherwise the source
    /// compares imported owner max-mana values (ties select the source side).
    pub fn merge_mana_spheres(&mut self, target_slot: u16, source_slot: u16) -> bool {
        if target_slot == source_slot {
            return false;
        }
        let Some(target_index) = self
            .spheres
            .iter()
            .position(|sphere| sphere.active && sphere.source_entity_index == target_slot)
        else {
            return false;
        };
        let Some(source_index) = self
            .spheres
            .iter()
            .position(|sphere| sphere.active && sphere.source_entity_index == source_slot)
        else {
            return false;
        };
        let target_owner = self.spheres[target_index].owner_entity_index;
        let source_owner = self.spheres[source_index].owner_entity_index;
        let owner = if target_owner == 0 {
            source_owner
        } else if source_owner == 0 {
            target_owner
        } else if self.owner_class.get(&target_owner) == Some(&10)
            && self.owner_class.get(&source_owner) != Some(&10)
        {
            source_owner
        } else if self.owner_class.get(&source_owner) == Some(&10)
            && self.owner_class.get(&target_owner) != Some(&10)
        {
            target_owner
        } else if self
            .owner_max_mana
            .get(&target_owner)
            .copied()
            .unwrap_or_default()
            > self
                .owner_max_mana
                .get(&source_owner)
                .copied()
                .unwrap_or_default()
        {
            target_owner
        } else {
            source_owner
        };
        let source_mana = self.spheres[source_index].mana;
        let target_id = self.spheres[target_index].id;
        let source_id = self.spheres[source_index].id;
        self.spheres[target_index].mana = self.spheres[target_index].mana.wrapping_add(source_mana);
        self.spheres[target_index].owner_entity_index = owner;
        self.spheres[source_index].assigned_carrier_id = 0;
        self.spheres[source_index].pending_delete = true;
        self.spheres[source_index].active = false;
        self.events.push(Level2EconomyEvent::ManaSpheresMerged {
            target_sphere_id: target_id,
            source_sphere_id: source_id,
            owner_entity_index: owner,
            mana: source_mana,
        });
        true
    }

    /// Source carrier action 9's sphere absorption tail. The carrier receives
    /// the entire sphere payload only when the target is owned by the same
    /// player, within the Day attraction radius, and inside the strict source
    /// AABB (`AddBallon_60AB0`/`sub_106C0`). The sphere is disabled for the
    /// next reclaim pass; no allocation is attempted on this path.
    pub fn absorb_sphere_into_carrier(&mut self, sphere_slot: u16, carrier_slot: u16) -> bool {
        let Some(carrier) = self
            .carrier
            .as_ref()
            .copied()
            .filter(|carrier| carrier.active && carrier.source_entity_index == carrier_slot)
        else {
            return false;
        };
        let Some(sphere) = self
            .spheres
            .iter()
            .copied()
            .find(|sphere| sphere.active && sphere.source_entity_index == sphere_slot)
        else {
            return false;
        };
        if sphere.owner_entity_index != carrier.owner_entity_index
            || level2_wrapping_xy_distance(carrier.position, sphere.position)
                > u32::from(MANA_CARRIER_ATTRACTION_RADIUS)
                    * u32::from(MANA_CARRIER_ATTRACTION_RADIUS)
            || !level2_carrier_sphere_overlap(carrier.position, &sphere)
        {
            return false;
        }
        let mana = sphere.mana;
        let carrier_state = self.carrier.as_mut().expect("carrier remains installed");
        carrier_state.carried_mana = carrier_state.carried_mana.wrapping_add(mana);
        carrier_state.target_sphere_id = 0;
        carrier_state.owner_entity_index = sphere.owner_entity_index;
        carrier_state.life = carrier_state.max_life;
        let sphere_state = self
            .spheres
            .iter_mut()
            .find(|candidate| candidate.active && candidate.source_entity_index == sphere_slot)
            .expect("sphere remains installed");
        sphere_state.assigned_carrier_id = 0;
        sphere_state.pending_delete = true;
        sphere_state.active = false;
        self.events.push(Level2EconomyEvent::ManaSphereCollected {
            sphere_id: sphere.id,
            carrier_id: carrier.id,
            mana,
        });
        true
    }

    pub fn spawn_mana_carrier(
        &mut self,
        position: Position,
        owner_entity_index: u16,
        target_sphere_id: u32,
    ) -> Option<u16> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self
            .slot_rng
            .get(&source_entity_index)
            .copied()
            .unwrap_or(0);
        let carrier = ManaCarrier {
            id: u32::from(source_entity_index),
            source_entity_index,
            entity_rng,
            owner_entity_index,
            position,
            yaw: 0,
            pitch: 0,
            speed: MANA_CARRIER_SPEED,
            life: MANA_CARRIER_MAX_LIFE,
            max_life: MANA_CARRIER_MAX_LIFE,
            max_mana: MANA_CARRIER_MAX_MANA,
            target_sphere_id,
            carried_mana: 0,
            returning_to_castle: false,
            pending_damage: 0,
            pending_attacker_entity_index: 0,
            active: true,
        };
        self.carrier = Some(carrier);
        self.events.push(Level2EconomyEvent::ManaCarrierSpawned {
            source_entity_index,
            owner_entity_index,
        });
        Some(source_entity_index)
    }

    pub fn build_castle(&mut self, position: Position, owner_entity_index: u16) -> Option<u16> {
        let mut root_position = position;
        root_position.z = self.terrain.ground_height(root_position);
        self.build_castle_at(root_position, root_position, owner_entity_index, false)
    }

    /// Build a Castle from the source projectile placement path, retaining
    /// the distinct snapped root position and BUILD-derived construction
    /// anchor. This is the production entry point when the owner BUILD
    /// footprint is available from the validated cache.
    pub fn build_castle_with_footprint(
        &mut self,
        projectile_position: Position,
        owner_entity_index: u16,
        footprint: &CastleBuildFootprint,
    ) -> Option<u16> {
        if footprint.index != 1 {
            return None;
        }
        let (root_position, construction_anchor) =
            level2_castle_root_positions(&self.terrain, footprint, projectile_position);
        self.build_castle_at(root_position, construction_anchor, owner_entity_index, true)
    }

    fn build_castle_at(
        &mut self,
        position: Position,
        anchor_position: Position,
        owner_entity_index: u16,
        source_constructor: bool,
    ) -> Option<u16> {
        if self.castle.is_some_and(|castle| castle.active) {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        self.castle = Some(Level2CastleState {
            source_entity_index,
            source_id_entity_index: source_entity_index,
            source_class: 3,
            source_model: 2,
            source_byte_38: 33,
            source_particle_parameter: 177,
            owner_entity_index,
            position,
            anchor_position,
            life: if source_constructor {
                40_000
            } else {
                CASTLE_STAGE_ONE_LIFE
            },
            max_life: if source_constructor {
                40_000
            } else {
                CASTLE_STAGE_ONE_LIFE
            },
            mana: 0,
            mana_capacity: if source_constructor {
                0
            } else {
                CASTLE_STAGE_ONE_MANA_CAPACITY
            },
            pending_damage: 0,
            pending_attacker_entity_index: 0,
            killer_entity_index: 0,
            control_flags: 0,
            timer: 0,
            phase: source_entity_index as u8,
            action: if source_constructor { 5 } else { 4 },
            build_state: if source_constructor { 0 } else { 4 },
            level: if source_constructor { 0 } else { 1 },
            construction_slot: 0,
            defense_slot: 0,
            construction_model: 0,
            construction_action: 0,
            construction_control_flags: 0,
            construction_parent_slot: 0,
            construction_owner_entity_index: 0,
            construction_id: 0,
            construction_position: Position::default(),
            defense_model: 0,
            defense_action: 0,
            defense_control_flags: 0,
            defense_parent_slot: 0,
            defense_owner_entity_index: 0,
            defense_id: 0,
            defense_position: Position::default(),
            child_chain_slots: [0; 2],
            child_chain_models: [0; 2],
            child_chain_active: [false; 2],
            controller_chain_slots: [0; 8],
            controller_chain_count: 0,
            controller_chain_head: 0,
            controller_chain_tail: 0,
            construction_child_active: false,
            defense_child_active: false,
            active: true,
        });
        self.player_castle_entity_index = if source_constructor {
            0
        } else {
            source_entity_index
        };
        self.player_castle_stage = if source_constructor { 0 } else { 1 };
        self.player_castle_aux_flags = 0;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .expect("Castle root source slot fits the map chain");
        self.events.push(Level2EconomyEvent::CastleBuilt {
            source_entity_index,
            owner_entity_index,
        });
        Some(source_entity_index)
    }

    /// Set the source Castle anchor (`axis_0x9A_154x`) used by child creation
    /// and the action-4 model-39 chain scan. The root position remains the
    /// terrain-synchronised entity position.
    pub fn set_castle_anchor(&mut self, castle_slot: u16, anchor: Position) -> bool {
        let Some(castle) = self
            .castle
            .as_mut()
            .filter(|castle| castle.active && castle.source_entity_index == castle_slot)
        else {
            return false;
        };
        castle.anchor_position = anchor;
        true
    }

    /// Re-enter the source Castle construction action for a reset root. This
    /// is the bounded `BeginOfCastleCreation_5FA70` entry; child allocation is
    /// committed only by the following `step_castle_action` call.
    pub fn start_castle_construction(&mut self, castle_slot: u16) -> bool {
        let Some(castle) = self
            .castle
            .as_mut()
            .filter(|castle| castle.active && castle.source_entity_index == castle_slot)
        else {
            return false;
        };
        if castle.action != 4 {
            return false;
        }
        castle.action = 5;
        castle.build_state = 0;
        castle.level = 0;
        castle.construction_slot = 0;
        castle.defense_slot = 0;
        castle.timer = 0;
        castle.construction_model = 0;
        castle.construction_action = 0;
        castle.construction_control_flags = 0;
        castle.construction_parent_slot = 0;
        castle.construction_owner_entity_index = 0;
        castle.construction_id = 0;
        castle.construction_position = Position::default();
        castle.defense_model = 0;
        castle.defense_action = 0;
        castle.defense_control_flags = 0;
        castle.defense_parent_slot = 0;
        castle.defense_owner_entity_index = 0;
        castle.defense_id = 0;
        castle.defense_position = Position::default();
        castle.child_chain_slots = [0; 2];
        castle.child_chain_models = [0; 2];
        castle.child_chain_active = [false; 2];
        castle.controller_chain_slots = [0; 8];
        castle.controller_chain_count = 0;
        castle.controller_chain_head = 0;
        castle.controller_chain_tail = 0;
        castle.construction_child_active = false;
        castle.defense_child_active = false;
        castle.life = 0;
        castle.max_life = 0;
        castle.mana_capacity = 5_000;
        self.player_castle_stage = 0;
        self.player_castle_aux_flags = 0;
        true
    }

    /// Commit the construction child completion callback (`state 2`) after
    /// the model-42 child has finished its own action stream.
    pub fn complete_castle_construction(&mut self, castle_slot: u16) -> bool {
        let construction_slot = {
            let Some(castle) = self
                .castle
                .as_mut()
                .filter(|castle| castle.active && castle.source_entity_index == castle_slot)
            else {
                return false;
            };
            if castle.action != 5 || castle.build_state != 4 || !castle.construction_child_active {
                return false;
            }
            let construction_slot = castle.construction_slot;
            castle.construction_child_active = false;
            castle.child_chain_active[0] = false;
            castle.build_state = 2;
            construction_slot
        };
        if let Some(child) = self
            .castle_children
            .iter_mut()
            .find(|child| child.source_entity_index == construction_slot)
        {
            child.active = false;
            let inactive = *child;
            self.sync_castle_child_map(inactive);
        }
        true
    }

    /// Enter the source model-41 defense-child state (`state 5`).
    pub fn start_castle_defense_child(&mut self, castle_slot: u16) -> bool {
        let Some(castle) = self
            .castle
            .as_mut()
            .filter(|castle| castle.active && castle.source_entity_index == castle_slot)
        else {
            return false;
        };
        if castle.action != 5 || castle.build_state != 4 {
            return false;
        }
        castle.build_state = 5;
        true
    }

    fn advance_castle_phase(&mut self, castle_slot: u16) {
        if let Some(castle) = self
            .castle
            .as_mut()
            .filter(|castle| castle.source_entity_index == castle_slot)
        {
            castle.phase = castle.phase.wrapping_add(1);
        }
    }

    fn sync_castle_child_map(&mut self, child: Level2CastleChild) {
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: child.source_entity_index,
                active: child.active,
                x: child.position.x,
                y: child.position.y,
            })
            .expect("Castle child source slot fits the map chain");
    }

    /// Bounded projection of `sub_5FD00`: release only the mana that exceeds
    /// the owner's stage capacity (or all mana for a zero-stage Castle),
    /// preserving source order and the exact final remainder. The source's
    /// subsequent `sub_5FF50` controller walk is represented by the existing
    /// ordered sphere/child vectors; no synthetic child is allocated here.
    fn release_castle_excess_mana(&mut self, castle_slot: u16) {
        let Some(castle) = self
            .castle
            .filter(|castle| castle.active && castle.source_entity_index == castle_slot)
        else {
            return;
        };
        let release = if castle.level == 0 {
            castle.mana
        } else {
            self.player_mana
                .saturating_add(castle.mana)
                .saturating_sub(castle.mana_capacity)
        };
        if release <= 0 {
            return;
        }
        let fraction = (release / 1_000).clamp(1, 32);
        let sphere_count = fraction.min(i32::try_from(self.free_slots.len()).unwrap_or(i32::MAX));
        if sphere_count == 0 {
            return;
        }
        // The source recomputes the per-sphere share from the number of
        // allocations that can actually be committed, then gives the final
        // source-ordered sphere the exact remainder.
        let each_mana = release / sphere_count;
        let mut remaining = release;
        let mut source_rng = self
            .source_rng
            .get(&castle_slot)
            .copied()
            .or_else(|| self.slot_rng.get(&castle_slot).copied())
            .unwrap_or_default();
        let mut released: i32 = 0;
        for sphere_index in 0..sphere_count {
            let Some(sphere_slot) = self.allocate_slot() else {
                break;
            };
            let sphere_seed = self.slot_rng.get(&sphere_slot).copied().unwrap_or_default();
            let sphere_rng = level2_next_rng16(sphere_seed);
            let speed_rng = level2_next_rng16(sphere_rng);
            source_rng = level2_next_rng16(source_rng);
            let radius =
                i16::try_from(source_rng % 0x1400 + 3_840).expect("Castle sphere radius fits i16");
            source_rng = level2_next_rng16(source_rng);
            let yaw = i16::try_from(source_rng & 0x07ff).expect("Castle sphere yaw fits i16");
            let mut sphere_position = castle.position;
            crate::move_entity(&mut sphere_position, yaw, 0, radius);
            let ground = self.terrain.ground_height(castle.position);
            let height_above_ground = i32::from(castle.position.z) - i32::from(ground);
            let vertical_velocity = (1_024 - height_above_ground).wrapping_div(8) as i16;
            let sphere_mana = if sphere_index + 1 == sphere_count {
                remaining
            } else {
                each_mana
            };
            remaining = remaining.saturating_sub(sphere_mana);
            released = released.saturating_add(sphere_mana);
            let sphere = ManaSphere {
                id: u32::from(sphere_slot),
                source_entity_index: sphere_slot,
                entity_rng: speed_rng,
                position: sphere_position,
                velocity_x: 0,
                velocity_y: 0,
                vertical_velocity,
                yaw: 0,
                mana: sphere_mana,
                owner_entity_index: castle.owner_entity_index,
                incoming_owner_entity_index: 0,
                incoming_possession_payload: 0,
                assigned_carrier_id: 0,
                phase: sphere_slot as u8,
                active: true,
                pending_delete: false,
            };
            self.spheres.push(sphere);
            self.events.push(Level2EconomyEvent::ManaSphereSpawned {
                sphere_id: sphere.id,
                source_entity_index: castle_slot,
                mana: sphere_mana,
            });
        }
        self.source_rng.insert(castle_slot, source_rng);
        if released != 0 {
            if let Some(castle) = self.castle.as_mut() {
                castle.mana = castle.mana.wrapping_sub(released);
            }
        }
    }

    /// Advance the bounded Castle root action. This follows the source state
    /// table in `BeginOfCastleCreation_5FA70`: states 0/3 allocate a class-10
    /// model-42 child, state 4 scans for that child, and state 5 allocates the
    /// model-41 child. The model-42 allocation is atomic; the bounded
    /// model-79 stage-controller loop may commit a partial chain when the
    /// source arena exhausts.
    pub fn step_castle_action(&mut self, castle_slot: u16) -> bool {
        let Some(snapshot) = self
            .castle
            .filter(|castle| castle.active && castle.source_entity_index == castle_slot)
        else {
            return false;
        };
        match snapshot.action {
            5 if snapshot.build_state == 0 || snapshot.build_state == 3 => {
                let Some(child_slot) = self.allocate_slot() else {
                    self.advance_castle_phase(castle_slot);
                    return false;
                };
                let next_level = snapshot.level.saturating_add(1).max(1);
                let (base_life, stage_mana_capacity) = level2_castle_stage_stats(next_level);
                let stage_coefficient = self
                    .castle_stage_coefficients
                    .get(usize::from(next_level))
                    .copied()
                    .unwrap_or(256);
                let stage_life =
                    i32::try_from((i64::from(base_life) * i64::from(stage_coefficient)) >> 8)
                        .unwrap_or(if base_life >= 0 { i32::MAX } else { i32::MIN });
                let stale_stage_children = self
                    .castle_children
                    .iter()
                    .filter(|child| {
                        child.active
                            && child.parent_entity_index == snapshot.source_entity_index
                            && child.model == 79
                    })
                    .copied()
                    .collect::<Vec<_>>();
                for stale_child in stale_stage_children {
                    if let Some(child) = self
                        .castle_children
                        .iter_mut()
                        .find(|child| child.source_entity_index == stale_child.source_entity_index)
                    {
                        child.active = false;
                    }
                    self.sync_castle_child_map(Level2CastleChild {
                        active: false,
                        ..stale_child
                    });
                    // `sub_57F20` returns the disabled controller to the
                    // source free arena after unlinking it from the map.
                    self.release_slot(stale_child.source_entity_index, None);
                }
                let construction_child = Level2CastleChild {
                    source_entity_index: child_slot,
                    id_entity_index: snapshot.source_id_entity_index,
                    parent_entity_index: snapshot.source_entity_index,
                    owner_entity_index: snapshot.owner_entity_index,
                    position: snapshot.anchor_position,
                    model: 42,
                    action: 44,
                    control_flags: 1,
                    chain_previous_slot: 0,
                    chain_next_slot: 0,
                    active: true,
                };
                self.sync_castle_child_map(construction_child);
                self.castle_children.push(construction_child);
                // `sub_613D0` removes the prior `word_0x34_52` chain, then
                // allocates the source-ordered model-79 controller points
                // from x_BYTE_DB038.  NewEvent failure is deliberately
                // non-atomic: model-42 and any committed controller prefix
                // remain installed, exactly as in the source loop.
                let (controller_width, controller_height) =
                    level2_castle_controller_dimensions(next_level);
                let center_x = snapshot.position.x.wrapping_add(128).to_be_bytes()[0];
                let center_y = snapshot.position.y.wrapping_add(128).to_be_bytes()[0];
                let start_x = center_x.wrapping_sub(controller_width / 2);
                let start_y = center_y.wrapping_sub(controller_height / 2);
                let mut controller_slots = [0_u16; 8];
                let mut controller_count = 0_u8;
                let mut previous_child_index: Option<usize> = None;
                let mut previous_slot = snapshot.source_entity_index;
                for &(offset_x, offset_y) in level2_castle_controller_points(next_level) {
                    let Some(stage_slot) = self.allocate_slot() else {
                        break;
                    };
                    let stage_position = Position {
                        x: u16::from(start_x.wrapping_add_signed(offset_x)) << 8,
                        y: u16::from(start_y.wrapping_add_signed(offset_y)) << 8,
                        z: 0,
                    };
                    let ground = self.terrain.ground_height(stage_position);
                    let stage_child = Level2CastleChild {
                        source_entity_index: stage_slot,
                        id_entity_index: snapshot.source_id_entity_index,
                        // Retain the root relation as the typed owner of the
                        // chain; exact source word32/word34 links are exposed
                        // separately below.
                        parent_entity_index: snapshot.source_entity_index,
                        owner_entity_index: snapshot.owner_entity_index,
                        position: Position {
                            z: ground.saturating_add(if next_level <= 1 { 384 } else { 224 }),
                            ..stage_position
                        },
                        model: 79,
                        action: 0,
                        control_flags: 0,
                        chain_previous_slot: previous_slot,
                        chain_next_slot: 0,
                        active: true,
                    };
                    self.sync_castle_child_map(stage_child);
                    self.castle_children.push(stage_child);
                    let child_index = self.castle_children.len() - 1;
                    if let Some(previous_index) = previous_child_index {
                        self.castle_children[previous_index].chain_next_slot = stage_slot;
                    }
                    previous_child_index = Some(child_index);
                    previous_slot = stage_slot;
                    controller_slots[usize::from(controller_count)] = stage_slot;
                    controller_count = controller_count.saturating_add(1);
                }
                let castle = self.castle.as_mut().expect("Castle remains installed");
                castle.construction_slot = child_slot;
                castle.construction_model = 42;
                castle.construction_action = 44;
                castle.construction_control_flags = 1;
                castle.construction_parent_slot = castle.source_entity_index;
                castle.construction_owner_entity_index = castle.owner_entity_index;
                castle.construction_id = castle.source_id_entity_index;
                castle.construction_position = castle.anchor_position;
                castle.child_chain_slots[0] = child_slot;
                castle.child_chain_models[0] = 42;
                castle.child_chain_active[0] = true;
                castle.controller_chain_slots = controller_slots;
                castle.controller_chain_count = controller_count;
                castle.controller_chain_head = controller_slots[0];
                castle.controller_chain_tail = if controller_count == 0 {
                    0
                } else {
                    controller_slots[usize::from(controller_count - 1)]
                };
                castle.construction_child_active = true;
                castle.build_state = 4;
                castle.level = next_level;
                castle.life = stage_life;
                castle.max_life = stage_life;
                castle.mana_capacity = stage_mana_capacity;
                castle.control_flags &= !0x40;
                self.player_castle_entity_index = castle.source_entity_index;
                self.player_castle_stage = castle.level;
                self.player_castle_aux_flags = 0;
                self.events
                    .push(Level2EconomyEvent::CastleConstructionSound {
                        source_entity_index: castle.source_entity_index,
                        sound_id: 10,
                    });
                self.advance_castle_phase(castle_slot);
                true
            }
            5 if snapshot.build_state == 2 => {
                let castle = self.castle.as_mut().expect("Castle remains installed");
                castle.action = 4;
                castle.build_state = 0;
                self.events.push(Level2EconomyEvent::CastleBuilt {
                    source_entity_index: castle.source_entity_index,
                    owner_entity_index: castle.owner_entity_index,
                });
                self.advance_castle_phase(castle_slot);
                true
            }
            5 if snapshot.build_state == 1 => {
                let castle = self.castle.as_mut().expect("Castle remains installed");
                castle.position.z = self.terrain.ground_height(castle.position);
                self.advance_castle_phase(castle_slot);
                true
            }
            5 if snapshot.build_state == 4 => {
                if self
                    .castle_children
                    .iter()
                    .any(|child| child.active && child.model == 42)
                {
                    self.advance_castle_phase(castle_slot);
                    return true;
                }
                self.castle
                    .as_mut()
                    .expect("Castle remains installed")
                    .build_state = 3;
                self.advance_castle_phase(castle_slot);
                true
            }
            5 if snapshot.build_state == 5 => {
                let Some(child_slot) = self.allocate_slot() else {
                    self.advance_castle_phase(castle_slot);
                    return false;
                };
                let defense_child = Level2CastleChild {
                    source_entity_index: child_slot,
                    id_entity_index: snapshot.source_id_entity_index,
                    parent_entity_index: snapshot.source_entity_index,
                    owner_entity_index: snapshot.owner_entity_index,
                    position: snapshot.anchor_position,
                    model: 41,
                    action: 43,
                    control_flags: 0,
                    chain_previous_slot: 0,
                    chain_next_slot: 0,
                    active: true,
                };
                self.sync_castle_child_map(defense_child);
                self.castle_children.push(defense_child);
                let castle = self.castle.as_mut().expect("Castle remains installed");
                castle.defense_slot = child_slot;
                castle.defense_model = 41;
                castle.defense_action = 43;
                castle.defense_control_flags = 0;
                castle.defense_parent_slot = castle.source_entity_index;
                castle.defense_owner_entity_index = castle.owner_entity_index;
                castle.defense_id = castle.source_id_entity_index;
                castle.defense_position = castle.anchor_position;
                castle.child_chain_slots[1] = child_slot;
                castle.child_chain_models[1] = 41;
                castle.child_chain_active[1] = true;
                castle.defense_child_active = true;
                castle.build_state = 6;
                self.advance_castle_phase(castle_slot);
                true
            }
            5 if snapshot.build_state == 6 => {
                self.castle
                    .as_mut()
                    .expect("Castle remains installed")
                    .position
                    .z = self.terrain.ground_height(snapshot.position);
                self.advance_castle_phase(castle_slot);
                true
            }
            4 if snapshot.timer > 0 => {
                let castle = self.castle.as_mut().expect("Castle remains installed");
                if castle.timer == 1 {
                    castle.timer = 0;
                    if castle.control_flags & 0x10 == 0 {
                        castle.action = 5;
                        castle.build_state = 3;
                    }
                } else {
                    castle.timer = castle.timer.wrapping_sub(1);
                }
                castle.position.z = self.terrain.ground_height(castle.position);
                self.advance_castle_phase(castle_slot);
                true
            }
            4 if snapshot.life < 0 => {
                self.castle
                    .as_mut()
                    .expect("Castle remains installed")
                    .action = 6;
                self.advance_castle_phase(castle_slot);
                true
            }
            4 if snapshot.control_flags & 0x40 != 0 => {
                let castle = self.castle.as_mut().expect("Castle remains installed");
                castle.control_flags &= !0x40;
                castle.build_state = 3;
                castle.action = 5;
                self.advance_castle_phase(castle_slot);
                true
            }
            4 => {
                let source_phase = snapshot.phase;
                if snapshot.pending_attacker_entity_index != 0 {
                    let _ = self.resolve_damage(castle_slot);
                    if self.castle.is_some_and(|castle| castle.life < 0) {
                        self.castle
                            .as_mut()
                            .expect("Castle remains installed")
                            .action = 6;
                        self.advance_castle_phase(castle_slot);
                        return true;
                    }
                    if self
                        .castle
                        .is_some_and(|castle| castle.control_flags & 0x40 != 0)
                    {
                        let castle = self.castle.as_mut().expect("Castle remains installed");
                        castle.action = 5;
                        castle.build_state = 0;
                        castle.position.z = self.terrain.ground_height(castle.position);
                        self.advance_castle_phase(castle_slot);
                        return true;
                    }
                }
                if source_phase & 1 != 0 {
                    self.advance_castle_phase(castle_slot);
                    return true;
                }
                if let Some(castle) = self.castle.as_mut() {
                    castle.position.z = self.terrain.ground_height(castle.position);
                }
                self.release_castle_excess_mana(castle_slot);
                let Some(castle) = self.castle else {
                    self.advance_castle_phase(castle_slot);
                    return false;
                };
                if castle.mana >= castle.mana_capacity {
                    self.advance_castle_phase(castle_slot);
                    return true;
                }
                let Some(sphere_index) = self.spheres.iter().position(|sphere| {
                    sphere.active
                        && sphere.owner_entity_index == castle.owner_entity_index
                        && level2_castle_sphere_overlap(castle.position, sphere)
                }) else {
                    self.advance_castle_phase(castle_slot);
                    return true;
                };
                let sphere = self.spheres[sphere_index];
                let castle = self.castle.as_mut().expect("Castle remains installed");
                castle.mana = castle.mana.wrapping_add(sphere.mana);
                self.spheres[sphere_index].pending_delete = true;
                self.spheres[sphere_index].active = false;
                self.events.push(Level2EconomyEvent::ManaSphereCollected {
                    sphere_id: sphere.id,
                    carrier_id: u32::from(castle.source_entity_index),
                    mana: sphere.mana,
                });
                self.advance_castle_phase(castle_slot);
                true
            }
            6 => {
                if self.free_slots.is_empty() {
                    self.advance_castle_phase(castle_slot);
                    return false;
                }
                let construction_slot = snapshot.construction_slot;
                let defense_slot = snapshot.defense_slot;
                if construction_slot != 0 {
                    self.release_slot(construction_slot, None);
                }
                if defense_slot != 0 {
                    self.release_slot(defense_slot, None);
                }
                let child_slots = self
                    .castle_children
                    .iter()
                    .filter(|child| {
                        child.active && child.parent_entity_index == snapshot.source_entity_index
                    })
                    .map(|child| child.source_entity_index)
                    .collect::<Vec<_>>();
                for child_slot in child_slots {
                    self.release_slot(child_slot, None);
                }
                let teardown_children = self
                    .castle_children
                    .iter()
                    .filter(|child| child.parent_entity_index == snapshot.source_entity_index)
                    .copied()
                    .collect::<Vec<_>>();
                for child in teardown_children {
                    if let Some(installed) = self.castle_children.iter_mut().find(|installed| {
                        installed.source_entity_index == child.source_entity_index
                    }) {
                        installed.active = false;
                    }
                    self.sync_castle_child_map(Level2CastleChild {
                        active: false,
                        ..child
                    });
                }
                let castle = self.castle.as_mut().expect("Castle remains installed");
                castle.action = 4;
                castle.build_state = 0;
                castle.level = snapshot.level.saturating_sub(1);
                castle.life = 0;
                castle.max_life = 0;
                castle.mana = 0;
                castle.mana_capacity = 5_000;
                castle.construction_slot = 0;
                castle.defense_slot = 0;
                castle.construction_model = 0;
                castle.construction_action = 0;
                castle.construction_control_flags = 0;
                castle.construction_parent_slot = 0;
                castle.construction_owner_entity_index = 0;
                castle.construction_id = 0;
                castle.construction_position = Position::default();
                castle.defense_model = 0;
                castle.defense_action = 0;
                castle.defense_control_flags = 0;
                castle.defense_parent_slot = 0;
                castle.defense_owner_entity_index = 0;
                castle.defense_id = 0;
                castle.defense_position = Position::default();
                castle.child_chain_slots = [0; 2];
                castle.child_chain_models = [0; 2];
                castle.child_chain_active = [false; 2];
                castle.controller_chain_slots = [0; 8];
                castle.controller_chain_count = 0;
                castle.controller_chain_head = 0;
                castle.controller_chain_tail = 0;
                castle.construction_child_active = false;
                castle.defense_child_active = false;
                castle.timer = 5;
                if castle.level == 0 {
                    castle.active = false;
                    self.player_castle_entity_index = 0;
                    self.player_castle_stage = 0;
                    self.player_castle_aux_flags = 0;
                    self.events.push(Level2EconomyEvent::CastleDestroyed {
                        source_entity_index: castle.source_entity_index,
                        killer_entity_index: castle.killer_entity_index,
                    });
                } else {
                    self.player_castle_stage = castle.level;
                    self.player_castle_aux_flags = 0;
                }
                if let Some(castle) = self.castle.filter(|castle| !castle.active) {
                    self.map_chains
                        .sync_entity(RenderableMapSeed {
                            entity_index: castle.source_entity_index,
                            active: false,
                            x: castle.position.x,
                            y: castle.position.y,
                        })
                        .expect("Castle root source slot fits the map chain");
                }
                self.advance_castle_phase(castle_slot);
                true
            }
            _ => {
                // The outer source entity dispatcher advances byte 0x3E
                // after every action call, including unhandled actions.
                self.advance_castle_phase(castle_slot);
                false
            }
        }
    }

    pub fn return_carrier_to_castle(&mut self, carrier_slot: u16) -> bool {
        let Some(castle) = self.castle.as_mut().filter(|castle| castle.active) else {
            return false;
        };
        let Some(carrier) = self
            .carrier
            .as_mut()
            .filter(|carrier| carrier.active && carrier.source_entity_index == carrier_slot)
        else {
            return false;
        };
        if carrier.owner_entity_index != castle.owner_entity_index {
            return false;
        }
        carrier.target_sphere_id = u32::from(castle.source_entity_index);
        carrier.returning_to_castle = true;
        true
    }

    /// Advance a returning carrier to a supplied source position and commit
    /// `AddBallon_60AB0`'s Castle arrival only once the source X/Y distance,
    /// stage, and terrain upper-bound gates pass. The carrier remains active
    /// with its life restored; its mana payload is transferred and cleared at
    /// arrival, not when the return is scheduled.
    pub fn step_carrier_to_castle(&mut self, carrier_slot: u16, position: Position) -> bool {
        let Some(castle) = self.castle.filter(|castle| castle.active) else {
            return false;
        };
        let raw_position = position;
        let ground = self.terrain.ground_height(raw_position);
        let Some(carrier) = self
            .carrier
            .as_mut()
            .filter(|carrier| carrier.active && carrier.source_entity_index == carrier_slot)
        else {
            return false;
        };
        if !carrier.returning_to_castle
            || carrier.owner_entity_index != castle.owner_entity_index
            || castle.level == 0
        {
            return false;
        }
        let radius = u32::from(carrier.speed.unsigned_abs()) * u32::from(castle.level);
        let terrain_upper_bound = ground.saturating_add(MANA_CARRIER_COLLISION_VERTICAL_OFFSET);
        if level2_wrapping_xy_distance(raw_position, castle.position) > radius.wrapping_mul(radius)
            || raw_position.z > terrain_upper_bound
        {
            return false;
        }
        let mut position = raw_position;
        if position.z < ground {
            position.z = ground;
        }
        carrier.position = position;
        let mana = carrier.carried_mana;
        let castle_state = self.castle.as_mut().expect("Castle remains installed");
        castle_state.mana = castle_state.mana.wrapping_add(mana);
        carrier.carried_mana = 0;
        carrier.target_sphere_id = 0;
        carrier.returning_to_castle = false;
        carrier.life = carrier.max_life;
        self.events.push(Level2EconomyEvent::ManaCarrierArrived {
            carrier_id: carrier.id,
            castle_id: castle.source_entity_index,
            mana,
        });
        true
    }

    pub fn submit_damage(&mut self, target_slot: u16, damage: i32, attacker: u16) -> bool {
        if damage <= 0 || attacker == 0 {
            return false;
        }
        if let Some(carrier) = self.carrier.as_mut().filter(|carrier| {
            carrier.active && carrier.life >= 0 && carrier.source_entity_index == target_slot
        }) {
            carrier.pending_damage = if carrier.pending_attacker_entity_index == 0 {
                damage
            } else {
                carrier.pending_damage.wrapping_add(damage)
            };
            carrier.pending_attacker_entity_index = attacker;
            return true;
        }
        if let Some(castle) = self.castle.as_mut().filter(|castle| {
            castle.active && castle.life >= 0 && castle.source_entity_index == target_slot
        }) {
            castle.pending_damage = if castle.pending_attacker_entity_index == 0 {
                damage
            } else {
                castle.pending_damage.wrapping_add(damage)
            };
            castle.pending_attacker_entity_index = attacker;
            return true;
        }
        false
    }

    pub fn resolve_damage(&mut self, target_slot: u16) -> Option<bool> {
        if let Some(carrier) = self
            .carrier
            .as_mut()
            .filter(|carrier| carrier.active && carrier.source_entity_index == target_slot)
        {
            let damage = carrier.pending_damage;
            let attacker = carrier.pending_attacker_entity_index;
            if attacker == 0 {
                return None;
            }
            carrier.life = carrier.life.wrapping_sub(damage);
            carrier.pending_damage = 0;
            carrier.pending_attacker_entity_index = 0;
            let lethal = carrier.life < 0;
            if lethal {
                carrier.active = false;
            }
            self.events.push(Level2EconomyEvent::DamageApplied {
                target_entity_index: target_slot,
                attacker_entity_index: attacker,
                damage,
                lethal,
            });
            return Some(lethal);
        }
        let castle = self.castle.as_mut()?;
        if !castle.active || castle.source_entity_index != target_slot {
            return None;
        }
        let damage = castle.pending_damage;
        let attacker = castle.pending_attacker_entity_index;
        if attacker == 0 {
            return None;
        }
        castle.life = castle.life.wrapping_sub(damage);
        castle.pending_damage = 0;
        castle.pending_attacker_entity_index = 0;
        let lethal = castle.life < 0;
        if lethal {
            castle.killer_entity_index = attacker;
        } else {
            // sub_609E0 clears the damage mailbox, marks the owner's linked
            // player record, and requests state-0 reconstruction when the
            // attacker mailbox points back at this Castle root.
            self.player_castle_aux_flags = 4;
            if attacker == castle.source_id_entity_index && castle.level < 7 {
                castle.control_flags |= 0x40;
            }
        }
        self.events.push(Level2EconomyEvent::DamageApplied {
            target_entity_index: target_slot,
            attacker_entity_index: attacker,
            damage,
            lethal,
        });
        Some(lethal)
    }

    pub fn reclaim_inactive_slots(&mut self) {
        if let Some(carrier) = self.carrier
            && !carrier.active
        {
            self.release_slot(carrier.source_entity_index, Some(carrier.entity_rng));
            self.carrier = None;
        }
        let reclaimed = self
            .spheres
            .iter()
            .filter(|sphere| !sphere.active)
            .map(|sphere| (sphere.source_entity_index, sphere.entity_rng))
            .collect::<Vec<_>>();
        self.spheres.retain(|sphere| sphere.active);
        for (slot, entity_rng) in reclaimed {
            self.release_slot(slot, Some(entity_rng));
        }
        if let Some(castle) = self.castle
            && !castle.active
        {
            self.release_slot(castle.source_entity_index, None);
            self.castle = None;
        }
    }

    #[must_use]
    pub fn reset_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(REMC2_SPEC_COMMIT.as_bytes());
        hasher.update(self.terrain.deterministic_hash().to_le_bytes());
        hasher.update(self.player_mana.to_le_bytes());
        hasher.update(self.player_castle_entity_index.to_le_bytes());
        hasher.update([self.player_castle_stage, self.player_castle_aux_flags]);
        for coefficient in self.castle_stage_coefficients {
            hasher.update(coefficient.to_le_bytes());
        }
        for slot in &self.free_slots {
            hasher.update(slot.to_le_bytes());
        }
        for (slot, rng) in &self.slot_rng {
            hasher.update(slot.to_le_bytes());
            hasher.update(rng.to_le_bytes());
        }
        for (slot, rng) in &self.source_rng {
            hasher.update(slot.to_le_bytes());
            hasher.update(rng.to_le_bytes());
        }
        for (slot, owner) in &self.source_owner {
            hasher.update(slot.to_le_bytes());
            hasher.update(owner.to_le_bytes());
        }
        for (owner, class) in &self.owner_class {
            hasher.update(owner.to_le_bytes());
            hasher.update([*class]);
        }
        for (owner, max_mana) in &self.owner_max_mana {
            hasher.update(owner.to_le_bytes());
            hasher.update(max_mana.to_le_bytes());
        }
        for sphere in &self.spheres {
            hasher.update(sphere.id.to_le_bytes());
            hasher.update(sphere.source_entity_index.to_le_bytes());
            hasher.update(sphere.entity_rng.to_le_bytes());
            hasher.update(sphere.position.x.to_le_bytes());
            hasher.update(sphere.position.y.to_le_bytes());
            hasher.update(sphere.position.z.to_le_bytes());
            hasher.update(sphere.velocity_x.to_le_bytes());
            hasher.update(sphere.velocity_y.to_le_bytes());
            hasher.update(sphere.vertical_velocity.to_le_bytes());
            hasher.update(sphere.yaw.to_le_bytes());
            hasher.update(sphere.mana.to_le_bytes());
            hasher.update(sphere.owner_entity_index.to_le_bytes());
            hasher.update(sphere.incoming_owner_entity_index.to_le_bytes());
            hasher.update(sphere.incoming_possession_payload.to_le_bytes());
            hasher.update(sphere.assigned_carrier_id.to_le_bytes());
            hasher.update([sphere.phase]);
            hasher.update([u8::from(sphere.active), u8::from(sphere.pending_delete)]);
        }
        if let Some(carrier) = self.carrier {
            hasher.update([1]);
            hasher.update(carrier.id.to_le_bytes());
            hasher.update(carrier.source_entity_index.to_le_bytes());
            hasher.update(carrier.entity_rng.to_le_bytes());
            hasher.update(carrier.owner_entity_index.to_le_bytes());
            hasher.update(carrier.position.x.to_le_bytes());
            hasher.update(carrier.position.y.to_le_bytes());
            hasher.update(carrier.position.z.to_le_bytes());
            hasher.update(carrier.yaw.to_le_bytes());
            hasher.update(carrier.pitch.to_le_bytes());
            hasher.update(carrier.speed.to_le_bytes());
            hasher.update(carrier.carried_mana.to_le_bytes());
            hasher.update(carrier.life.to_le_bytes());
            hasher.update(carrier.max_life.to_le_bytes());
            hasher.update(carrier.max_mana.to_le_bytes());
            hasher.update(carrier.target_sphere_id.to_le_bytes());
            hasher.update(carrier.pending_damage.to_le_bytes());
            hasher.update(carrier.pending_attacker_entity_index.to_le_bytes());
            hasher.update([
                u8::from(carrier.returning_to_castle),
                u8::from(carrier.active),
            ]);
        } else {
            hasher.update([0]);
        }
        if let Some(castle) = self.castle {
            hasher.update([1]);
            hasher.update(castle.source_entity_index.to_le_bytes());
            hasher.update(castle.source_id_entity_index.to_le_bytes());
            hasher.update([
                castle.source_class,
                castle.source_model,
                castle.source_byte_38,
            ]);
            hasher.update(castle.source_particle_parameter.to_le_bytes());
            hasher.update(castle.owner_entity_index.to_le_bytes());
            hasher.update(castle.position.x.to_le_bytes());
            hasher.update(castle.position.y.to_le_bytes());
            hasher.update(castle.position.z.to_le_bytes());
            hasher.update(castle.anchor_position.x.to_le_bytes());
            hasher.update(castle.anchor_position.y.to_le_bytes());
            hasher.update(castle.anchor_position.z.to_le_bytes());
            hasher.update(castle.mana.to_le_bytes());
            hasher.update(castle.life.to_le_bytes());
            hasher.update(castle.max_life.to_le_bytes());
            hasher.update(castle.mana_capacity.to_le_bytes());
            hasher.update(castle.pending_damage.to_le_bytes());
            hasher.update(castle.pending_attacker_entity_index.to_le_bytes());
            hasher.update(castle.killer_entity_index.to_le_bytes());
            hasher.update(castle.control_flags.to_le_bytes());
            hasher.update([castle.timer]);
            hasher.update([castle.phase]);
            hasher.update(castle.construction_slot.to_le_bytes());
            hasher.update(castle.defense_slot.to_le_bytes());
            hasher.update([
                castle.action,
                castle.build_state,
                castle.level,
                castle.construction_model,
                castle.construction_action,
                castle.defense_model,
                castle.defense_action,
                u8::from(castle.construction_child_active),
                u8::from(castle.defense_child_active),
                u8::from(castle.active),
            ]);
            hasher.update(castle.construction_control_flags.to_le_bytes());
            hasher.update(castle.defense_control_flags.to_le_bytes());
            hasher.update(castle.construction_parent_slot.to_le_bytes());
            hasher.update(castle.construction_owner_entity_index.to_le_bytes());
            hasher.update(castle.construction_id.to_le_bytes());
            hasher.update(castle.construction_position.x.to_le_bytes());
            hasher.update(castle.construction_position.y.to_le_bytes());
            hasher.update(castle.construction_position.z.to_le_bytes());
            hasher.update(castle.defense_parent_slot.to_le_bytes());
            hasher.update(castle.defense_owner_entity_index.to_le_bytes());
            hasher.update(castle.defense_id.to_le_bytes());
            hasher.update(castle.defense_position.x.to_le_bytes());
            hasher.update(castle.defense_position.y.to_le_bytes());
            hasher.update(castle.defense_position.z.to_le_bytes());
            for slot in castle.child_chain_slots {
                hasher.update(slot.to_le_bytes());
            }
            hasher.update(castle.child_chain_models);
            hasher.update([
                u8::from(castle.child_chain_active[0]),
                u8::from(castle.child_chain_active[1]),
            ]);
            for slot in castle.controller_chain_slots {
                hasher.update(slot.to_le_bytes());
            }
            hasher.update(castle.controller_chain_count.to_le_bytes());
            hasher.update(castle.controller_chain_head.to_le_bytes());
            hasher.update(castle.controller_chain_tail.to_le_bytes());
        } else {
            hasher.update([0]);
        }
        for child in &self.castle_children {
            hasher.update(child.source_entity_index.to_le_bytes());
            hasher.update(child.id_entity_index.to_le_bytes());
            hasher.update(child.parent_entity_index.to_le_bytes());
            hasher.update(child.owner_entity_index.to_le_bytes());
            hasher.update(child.position.x.to_le_bytes());
            hasher.update(child.position.y.to_le_bytes());
            hasher.update(child.position.z.to_le_bytes());
            hasher.update([child.model, child.action, u8::from(child.active)]);
            hasher.update(child.control_flags.to_le_bytes());
            hasher.update(child.chain_previous_slot.to_le_bytes());
            hasher.update(child.chain_next_slot.to_le_bytes());
        }
        for source_entity_index in self.map_chains.linked_entity_indices() {
            hasher.update(source_entity_index.to_le_bytes());
            hasher.update(self.map_chains.tile_of(source_entity_index).to_le_bytes());
            hasher.update(
                self.map_chains
                    .old_map_entity(source_entity_index)
                    .to_le_bytes(),
            );
            hasher.update(
                self.map_chains
                    .next_entity(source_entity_index)
                    .to_le_bytes(),
            );
        }
        format!("{:x}", hasher.finalize())
    }

    pub fn reset(&mut self) {
        self.terrain = self.baseline_terrain.clone();
        self.free_slots = self.baseline_free_slots.to_vec();
        self.slot_rng = self.baseline_slot_rng.clone();
        self.source_rng = self.baseline_source_rng.clone();
        self.source_owner = self.baseline_source_owner.clone();
        self.owner_class = self.baseline_owner_class.clone();
        self.owner_max_mana = self.baseline_owner_max_mana.clone();
        self.spheres = self.baseline_spheres.clone();
        self.carrier = self.baseline_carrier;
        self.castle = self.baseline_castle;
        self.castle_children = self.baseline_castle_children.clone();
        self.map_chains = self.baseline_map_chains.clone();
        self.player_mana = self.baseline_player_mana;
        self.player_castle_entity_index = self.baseline_player_castle_entity_index;
        self.player_castle_stage = self.baseline_player_castle_stage;
        self.player_castle_aux_flags = self.baseline_player_castle_aux_flags;
        self.castle_stage_coefficients = self.baseline_castle_stage_coefficients;
        self.events.clear();
    }
}

fn level2_next_rng16(seed: u16) -> u16 {
    seed.wrapping_mul(9_377).wrapping_add(9_439)
}

impl Level2SharedSpellResetSession {
    fn from_parts(
        terrain: SessionTerrain,
        entities: &[EntitySnapshot],
        players: &[PlayerSnapshot],
        presentation: &PresentationSelectors,
    ) -> Result<Self> {
        const PARAMETER_60: [u8; ENTITY_PARAMETER_BYTES] = [
            1, 0, 22, 0, 5, 0, 22, 0, 5, 0, 0, 7, 0, 0, 252, 255, 0, 1, 0, 2, 255, 255, 255, 255,
            0, 9, 40, 0, 0, 16, 0, 2, 0, 0,
        ];
        const PARTICLE_77: ParticleParameter = ParticleParameter {
            base_tmap: 59,
            word_2: 8492,
            word_4: 42,
            speed: 184,
            rotation_speed: 150,
            byte_10: 0,
            byte_11: -1,
            render_mode: 1,
            byte_13: 0,
        };
        if presentation.entity_parameters.get(60) != Some(&PARAMETER_60)
            || presentation.particles.get(77) != Some(&PARTICLE_77)
            || presentation.level_sprite_selectors.get(59) != Some(&255)
        {
            return Err(NativeWorldError::new(
                "Level 2 shared spell parameter or presentation row changed",
            ));
        }
        let mut rows = Vec::with_capacity(4);
        for (ordinal, slot) in LEVEL2_SHARED_SPELL_PICKUP_SLOTS
            .into_iter()
            .chain(LEVEL2_SHARED_SPELL_HOLDER_SLOTS)
            .enumerate()
        {
            let entity = entities.get(usize::from(slot)).ok_or_else(|| {
                NativeWorldError::new(format!("missing Level 2 shared spell slot {slot}"))
            })?;
            let expected = if ordinal < 2 {
                (
                    15,
                    if slot == 38 { 12 } else { 3 },
                    if slot == 38 { 37 } else { 10 },
                    0x4,
                    (0, 0),
                )
            } else if slot == 113 {
                (15, 0, 0, 0x5, (112, 114))
            } else {
                (15, 1, 3, 0x5, (113, 0))
            };
            if !entity.active
                || (
                    entity.class,
                    entity.model,
                    entity.action,
                    entity.control_flags,
                    (entity.old_map_entity(), entity.next_entity()),
                ) != expected
                || entity.particle_parameter_index != Some(77)
                || entity.parameter_relation != Some(59)
                || sha256_hex(entity.record_bytes()) != LEVEL2_SHARED_SPELL_ENTITY_SHA256[ordinal]
            {
                return Err(NativeWorldError::new(format!(
                    "Level 2 shared spell census row {slot} changed"
                )));
            }
            rows.push(entity.clone());
        }
        let local_player = players
            .first()
            .ok_or_else(|| NativeWorldError::new("Level 2 shared spell local player is absent"))?;
        let baseline_local_player = local_player.clone();
        let player_entity = entities
            .get(usize::from(local_player.entity_index))
            .ok_or_else(|| NativeWorldError::new("Level 2 shared spell player entity is absent"))?;
        if !player_entity.active
            || player_entity.class != 3
            || player_entity.model != 0
            || sha256_hex(player_entity.record_bytes()) != LEVEL2_SHARED_SPELL_PLAYER_ENTITY_SHA256
        {
            return Err(NativeWorldError::new(
                "Level 2 shared spell player entity changed",
            ));
        }
        let pickups = rows[..2].to_vec().into_boxed_slice();
        let holders = rows[2..].to_vec().into_boxed_slice();
        let caster = SpellCasterState {
            entity_index: local_player.entity_index,
            model: player_entity.model,
            mana: player_entity.mana,
            control_flags: player_entity.control_flags,
            player_color_index: 0,
            control_mode: 0,
        };
        let spell_holder = |entity: &EntitySnapshot| SpellActivationHolder {
            entity_index: entity.index,
            model: entity.model,
            level_gate: entity.level_gate,
            active_value: entity.active_value,
            activation_value: entity.activation_value,
            mana_cost: entity.mana_cost,
            action_marker: entity.action_marker,
        };
        let fireball = spell_holder(&holders[0]);
        let possession = spell_holder(&holders[1]);
        let speed = SpellActivationHolder {
            entity_index: 75,
            model: SPEED_SPELL_FAMILY,
            level_gate: 0,
            active_value: 0,
            activation_value: SPEED_SPELL_ACTIVATION_TICKS,
            mana_cost: SPEED_SPELL_MANA_COST,
            action_marker: 0,
        };
        Ok(Self {
            baseline_terrain: terrain.clone(),
            terrain,
            baseline_pickups: pickups.clone(),
            pickups,
            baseline_holders: holders.clone(),
            holders,
            baseline_player_mana: player_entity.mana,
            player_mana: player_entity.mana,
            baseline_player_entity: player_entity.clone(),
            player_entity: player_entity.clone(),
            baseline_local_player: baseline_local_player.clone(),
            local_player: baseline_local_player,
            baseline_speed_pickup_state: Level2SpeedPickupState::default(),
            speed_pickup_state: Level2SpeedPickupState::default(),
            baseline_caster: caster,
            caster,
            baseline_fireball: fireball,
            fireball,
            baseline_possession: possession,
            possession,
            baseline_speed: speed,
            speed,
            player_entity_sha256: LEVEL2_SHARED_SPELL_PLAYER_ENTITY_SHA256.to_owned(),
            events: Vec::new(),
            pickup_events: Vec::new(),
        })
    }

    #[must_use]
    pub fn pickups(&self) -> &[EntitySnapshot] {
        &self.pickups
    }

    #[must_use]
    pub fn holders(&self) -> &[EntitySnapshot] {
        &self.holders
    }

    #[must_use]
    pub const fn player_mana(&self) -> i32 {
        self.player_mana
    }

    #[must_use]
    pub const fn baseline_player_mana(&self) -> i32 {
        self.baseline_player_mana
    }

    /// Execute remc2's shared `sub_5F660` activation primitive for the
    /// qualified Level 2 Fireball holder (model 0) or Possession holder
    /// (model 1). Projectile factories remain owned by their existing Level 1
    /// mission implementation; this seam proves the exact mana/gate,
    /// direction, timer, pointer, and audio-event transition on the Level 2
    /// reset data.
    pub fn activate_fireball(&mut self, direction_mask: u16, help_enabled: bool) -> bool {
        self.events.clear();
        let activated = activate_spell(
            &mut self.caster,
            Some(&mut self.fireball),
            direction_mask,
            help_enabled,
            &mut self.events,
        );
        self.player_mana = self.caster.mana;
        activated
    }

    pub fn activate_possession(&mut self, direction_mask: u16, help_enabled: bool) -> bool {
        self.events.clear();
        let activated = activate_spell(
            &mut self.caster,
            Some(&mut self.possession),
            direction_mask,
            help_enabled,
            &mut self.events,
        );
        self.player_mana = self.caster.mana;
        activated
    }

    pub fn activate_speed(&mut self, direction_mask: u16, help_enabled: bool) -> bool {
        self.events.clear();
        let activated = activate_spell(
            &mut self.caster,
            Some(&mut self.speed),
            direction_mask,
            help_enabled,
            &mut self.events,
        );
        self.player_mana = self.caster.mana;
        activated
    }

    #[must_use]
    pub fn fireball_holder(&self) -> SpellActivationHolder {
        self.fireball
    }

    #[must_use]
    pub fn possession_holder(&self) -> SpellActivationHolder {
        self.possession
    }

    #[must_use]
    pub fn speed_holder(&self) -> SpellActivationHolder {
        self.speed
    }

    #[must_use]
    pub fn activation_events(&self) -> &[FlightEvent] {
        &self.events
    }

    #[must_use]
    pub const fn speed_pickup_state(&self) -> Level2SpeedPickupState {
        self.speed_pickup_state
    }

    #[must_use]
    pub fn pickup_events(&self) -> &[Level2SharedSpellEvent] {
        &self.pickup_events
    }

    /// Dispatch the authored Level 2 Speed pickup at source slot 75.
    ///
    /// This follows the source order of `GetScroll_69DB0`: advance the pickup
    /// cadence byte, reject non-overlap or an already-owned family, then
    /// convert the world pickup into its holder and prepare sound 18. The
    /// model-12 row at slot 38 is intentionally not routed through this
    /// Level 1 family primitive; its later Level 2 family owns that behavior.
    pub fn step_speed_pickup(
        &mut self,
        player_entity_index: u16,
        player_position: Position,
    ) -> Result<Level2SpeedPickupStep> {
        if player_entity_index != self.player_entity.index {
            return Err(NativeWorldError::new(format!(
                "Level 2 Speed pickup received player slot {player_entity_index}, expected {}",
                self.player_entity.index
            )));
        }
        self.events.clear();
        self.pickup_events.clear();
        let pickup_index = self
            .pickups
            .iter()
            .position(|entity| entity.index == LEVEL2_SHARED_SPELL_PICKUP_SLOTS[1])
            .ok_or_else(|| NativeWorldError::new("Level 2 Speed pickup slot 75 is absent"))?;
        let pickup = &mut self.pickups[pickup_index];
        let previous_action = pickup.action;
        let previous_phase = pickup.phase;
        // `sub_68FF0` begins by relaxing the pickup body against the shared
        // terrain query. The surrounding Level2 session owns map-chain
        // surgery; this adapter deliberately claims only the row-local Z
        // write and records that boundary in the typed state below.
        let ground = self.terrain.ground_height(pickup.position);
        if pickup.position.z > ground {
            pickup.position.z = pickup.position.z.wrapping_sub(128);
        }
        if pickup.position.z <= ground {
            pickup.position.z = ground;
        }
        pickup.record[84..86].copy_from_slice(&pickup.position.z.to_le_bytes());
        pickup.phase = pickup.phase.wrapping_add(1);
        pickup.record[66] = pickup.phase;
        self.player_entity.position = player_position;
        self.player_entity.record[80..82].copy_from_slice(&player_position.x.to_le_bytes());
        self.player_entity.record[82..84].copy_from_slice(&player_position.y.to_le_bytes());
        self.player_entity.record[84..86].copy_from_slice(&player_position.z.to_le_bytes());

        let overlaps = level2_source_intersects(pickup, &self.player_entity);
        let already_owned = self.speed.active_value != 0 || pickup.action == 9;
        let in_cadence = previous_phase & 3 == 0;
        if pickup.action != 10 || !overlaps || already_owned || !in_cadence {
            return Ok(Level2SpeedPickupStep {
                source_slot: pickup.index,
                player_entity_index,
                previous_action,
                action: pickup.action,
                previous_phase,
                phase: pickup.phase,
                collected: false,
            });
        }

        // `sub_68FF0` preserves the class-15 source row as the holder. The
        // fixed Speed action/cost are the Level 1 family constants already
        // bound by this Level 2 census.
        pickup.record[16] |= 1;
        pickup.record[19] &= !2;
        pickup.control_flags |= 1;
        pickup.control_flags &= !0x0200_0000;
        pickup.record[44..46].copy_from_slice(&player_entity_index.to_le_bytes());
        pickup.action = 9;
        pickup.record[73] = pickup.action;
        pickup.active_value = 0;
        pickup.record[50..52].copy_from_slice(&0_i16.to_le_bytes());
        pickup.record[58..60].copy_from_slice(&64_i16.to_le_bytes());
        // The player state stores the holder relation and subspell selection
        // separately from the entity row. Level 2's source uses family 3 and
        // the first Speed subspell for this authored pickup.
        let family = usize::from(SPEED_SPELL_FAMILY);
        self.local_player.enabled_spell_holders[family] = pickup.index;
        self.local_player.record[268 + family * 2..270 + family * 2]
            .copy_from_slice(&pickup.index.to_le_bytes());
        self.local_player.record[424 + family] = 1;
        self.local_player.family_selected_tiers[family] = 0;
        self.local_player.record[528 + family] = 0;
        if self.local_player.left_spell_slot_raw < 0 || self.local_player.right_spell_slot_raw != -1
        {
            self.local_player.left_spell_slot_raw = i16::from(SPEED_SPELL_FAMILY);
            self.local_player.left_spell_slot = SPEED_SPELL_FAMILY;
            self.local_player.left_subspell_index = 0;
            self.local_player.record[554..556]
                .copy_from_slice(&i16::from(SPEED_SPELL_FAMILY).to_le_bytes());
            self.local_player.record[558] = 0;
        } else {
            self.local_player.right_spell_slot_raw = i16::from(SPEED_SPELL_FAMILY);
            self.local_player.right_spell_slot = SPEED_SPELL_FAMILY;
            self.local_player.right_subspell_index = 0;
            self.local_player.record[556..558]
                .copy_from_slice(&i16::from(SPEED_SPELL_FAMILY).to_le_bytes());
            self.local_player.record[559] = 0;
        }
        self.speed_pickup_state = Level2SpeedPickupState {
            owner_entity_index: player_entity_index,
            collected: true,
            visible: false,
            state_timer: 64,
        };
        self.speed.entity_index = pickup.index;
        self.speed.model = SPEED_SPELL_FAMILY;
        self.speed.activation_value = SPEED_SPELL_ACTIVATION_TICKS;
        self.speed.mana_cost = SPEED_SPELL_MANA_COST;
        self.pickup_events
            .push(Level2SharedSpellEvent::SpellCollected {
                source_slot: pickup.index,
                player_entity_index,
                family: SPEED_SPELL_FAMILY,
            });
        mark_source_audio_call(SourceAudioCallTag::SpeedPickup);
        self.events.push(FlightEvent::PrepareSound {
            #[cfg(test)]
            audio_site: crate::MissionAudioSite::SpeedPickup,
            entity_index: player_entity_index,
            player_index: 0,
            sound_id: 18,
        });
        Ok(Level2SpeedPickupStep {
            source_slot: pickup.index,
            player_entity_index,
            previous_action,
            action: pickup.action,
            previous_phase,
            phase: pickup.phase,
            collected: true,
        })
    }

    /// Dispatch one of the two reset-census pickup rows in source order.
    ///
    /// Slot 38 is returned as a mechanical exclusion (`None`) because its
    /// model-12 family is owned by a later Level 2 bead. Unknown slots fail
    /// closed instead of being treated as a nearby Level 1 family.
    pub fn step_shared_pickup(
        &mut self,
        source_slot: u16,
        player_entity_index: u16,
        player_position: Position,
    ) -> Result<Option<Level2SpeedPickupStep>> {
        match source_slot {
            38 if player_entity_index == self.player_entity.index => Ok(None),
            38 => Err(NativeWorldError::new(format!(
                "Level 2 shared pickup received player slot {player_entity_index}, expected {}",
                self.player_entity.index
            ))),
            75 => self
                .step_speed_pickup(player_entity_index, player_position)
                .map(Some),
            _ => Err(NativeWorldError::new(format!(
                "unknown Level 2 shared pickup source slot {source_slot}"
            ))),
        }
    }

    #[must_use]
    pub fn reset_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(REMC2_SPEC_COMMIT.as_bytes());
        for entity in self.pickups.iter().chain(self.holders.iter()) {
            hasher.update(entity.record_bytes());
        }
        hasher.update(self.player_entity.record_bytes());
        hasher.update(self.local_player.record_bytes());
        hasher.update(self.player_entity_sha256.as_bytes());
        hasher.update(self.terrain.deterministic_hash().to_le_bytes());
        hasher.update(self.player_mana.to_le_bytes());
        hasher.update(self.speed.entity_index.to_le_bytes());
        hasher.update(self.speed.active_value.to_le_bytes());
        hasher.update(self.speed.activation_value.to_le_bytes());
        hasher.update(self.speed.mana_cost.to_le_bytes());
        hasher.update(self.speed_pickup_state.owner_entity_index.to_le_bytes());
        hasher.update([u8::from(self.speed_pickup_state.collected)]);
        hasher.update([u8::from(self.speed_pickup_state.visible)]);
        hasher.update([self.speed_pickup_state.state_timer]);
        format!("{:x}", hasher.finalize())
    }

    pub fn reset(&mut self) {
        self.pickups.clone_from(&self.baseline_pickups);
        self.terrain = self.baseline_terrain.clone();
        self.holders.clone_from(&self.baseline_holders);
        self.player_mana = self.baseline_player_mana;
        self.player_entity = self.baseline_player_entity.clone();
        self.local_player = self.baseline_local_player.clone();
        self.speed_pickup_state = self.baseline_speed_pickup_state;
        self.caster = self.baseline_caster;
        self.fireball = self.baseline_fireball;
        self.possession = self.baseline_possession;
        self.speed = self.baseline_speed;
        self.events.clear();
        self.pickup_events.clear();
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level2OrdinaryResetSession {
    baseline_terrain: SessionTerrain,
    pub terrain: SessionTerrain,
    baseline_houses: HouseFamily,
    houses: HouseFamily,
    baseline_villagers: VillagerFamily,
    villagers: VillagerFamily,
    baseline_dolmens: Box<[Level2Dolmen]>,
    dolmens: Box<[Level2Dolmen]>,
    baseline_players: Box<[EntitySnapshot]>,
    players: Box<[EntitySnapshot]>,
}

impl Level2OrdinaryResetSession {
    fn from_parts(
        terrain: SessionTerrain,
        entities: Box<[EntitySnapshot]>,
        players: Box<[PlayerSnapshot]>,
        presentation: PresentationSelectors,
    ) -> Result<Self> {
        let houses =
            HouseFamily::from_level_two_entities(&entities).map_err(NativeWorldError::new)?;
        let villager_config = VillagerBehaviorConfig::from_parameter_record(
            presentation.entity_parameters.get(100).ok_or_else(|| {
                NativeWorldError::new("missing Level 2 villager parameter row 101")
            })?,
        )
        .map_err(NativeWorldError::new)?;
        if villager_config != VillagerBehaviorConfig::LEVEL_TWO {
            return Err(NativeWorldError::new(
                "Level 2 villager parameter row 101 does not match the shared movement config",
            ));
        }
        let villagers = VillagerFamily::from_level_two_entities(&entities, villager_config)
            .map_err(NativeWorldError::new)?;
        let dolmens = [108_u16, 109, 110]
            .iter()
            .map(|slot| {
                Level2Dolmen::from_entity(
                    entities.get(usize::from(*slot)).ok_or_else(|| {
                        NativeWorldError::new(format!("missing Level 2 dolmen {slot}"))
                    })?,
                    terrain.planes(),
                )
            })
            .collect::<Result<Vec<_>>>()?
            .into_boxed_slice();
        validate_level2_ordinary_census(&entities, &presentation)?;
        let player_entities = players
            .iter()
            .filter_map(|player| entities.get(usize::from(player.entity_index)))
            .filter(|entity| entity.active)
            .cloned()
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(Self {
            baseline_terrain: terrain.clone(),
            terrain,
            baseline_houses: houses.clone(),
            houses,
            baseline_villagers: villagers.clone(),
            villagers,
            baseline_dolmens: dolmens.clone(),
            dolmens,
            baseline_players: player_entities.clone(),
            players: player_entities,
        })
    }

    #[must_use]
    pub fn houses(&self) -> &[HouseState] {
        self.houses.houses()
    }

    #[must_use]
    pub fn villagers(&self) -> &[VillagerState] {
        self.villagers.villagers()
    }

    #[must_use]
    pub fn dolmens(&self) -> &[Level2Dolmen] {
        &self.dolmens
    }

    #[must_use]
    pub fn player_interaction_flags(&self) -> Vec<(u16, u32)> {
        self.players
            .iter()
            .map(|entity| (entity.index, entity.control_flags))
            .collect()
    }

    #[must_use]
    pub fn reset_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(REMC2_SPEC_COMMIT.as_bytes());
        hasher
            .update(format!("{:?}{:?}{:?}", self.houses, self.villagers, self.dolmens).as_bytes());
        for entity in &self.players {
            hasher.update(entity.record_bytes());
        }
        format!("{:x}", hasher.finalize())
    }

    pub fn step_house_action_52(&mut self, source_slot: u16) -> Vec<HouseStepEvent> {
        self.houses.step_source_slot(source_slot, &self.terrain)
    }

    pub fn step_villager_action_105(&mut self, source_slot: u16) -> Vec<VillagerStepEffect> {
        let mut source_views = self
            .houses
            .houses()
            .iter()
            .map(house_source_entity_view)
            .chain(
                self.villagers
                    .villagers()
                    .iter()
                    .map(villager_source_entity_view),
            )
            .collect::<Vec<_>>();
        source_views.sort_unstable_by_key(|view| view.source_entity_index);
        self.villagers
            .step_source_slot(source_slot, &self.terrain, &source_views, &mut self.houses)
    }

    pub fn inject_house_damage(
        &mut self,
        source_slot: u16,
        damage: i32,
        attacker: i16,
    ) -> Result<()> {
        self.houses
            .inject_damage(source_slot, damage, attacker)
            .map_err(NativeWorldError::new)
    }

    pub fn inject_villager_damage(
        &mut self,
        source_slot: u16,
        damage: i32,
        attacker: i16,
    ) -> Result<()> {
        self.villagers
            .inject_damage(source_slot, damage, attacker)
            .map_err(NativeWorldError::new)
    }

    pub fn step_dolmen_action_6(&mut self, source_slot: u16) -> Result<Level2DolmenStep> {
        let index = self
            .dolmens
            .iter()
            .position(|dolmen| dolmen.entity.index == source_slot)
            .ok_or_else(|| {
                NativeWorldError::new(format!("unknown Level 2 dolmen {source_slot}"))
            })?;
        let dolmen_entity = self.dolmens[index].entity.clone();
        for player in &mut self.players {
            if player.active && player.life >= 0 && level2_source_intersects(&dolmen_entity, player)
            {
                player.record[17] |= 0x10;
                player.control_flags = u32::from_le_bytes([
                    player.record[16],
                    player.record[17],
                    player.record[18],
                    player.record[19],
                ]);
            }
        }
        Ok(self.dolmens[index].step_action_6(&self.terrain))
    }

    pub fn reset(&mut self) {
        self.terrain = self.baseline_terrain.clone();
        self.houses = self.baseline_houses.clone();
        self.villagers = self.baseline_villagers.clone();
        self.dolmens = self.baseline_dolmens.clone();
        self.players = self.baseline_players.clone();
    }
}

fn validate_level2_ordinary_census(
    entities: &[EntitySnapshot],
    presentation: &PresentationSelectors,
) -> Result<()> {
    const PARAMETER_59: [u8; ENTITY_PARAMETER_BYTES] = [
        0, 0, 56, 0, 5, 0, 22, 0, 5, 0, 0, 7, 0, 0, 252, 255, 0, 1, 0, 2, 255, 255, 255, 255, 0,
        12, 50, 0, 0, 16, 0, 2, 0, 0,
    ];
    const PARAMETER_100: [u8; ENTITY_PARAMETER_BYTES] = [
        41, 0, 22, 0, 5, 0, 22, 0, 5, 0, 0, 1, 0, 0, 0, 255, 15, 0, 0, 2, 254, 254, 255, 255, 0, 9,
        40, 0, 0, 14, 0, 2, 9, 0,
    ];
    if presentation.entity_parameters.get(59) != Some(&PARAMETER_59)
        || presentation.entity_parameters.get(100) != Some(&PARAMETER_100)
    {
        return Err(NativeWorldError::new(
            "Level 2 ordinary resident/static parameter rows changed",
        ));
    }
    let particle = |index: u16| presentation.particles.get(usize::from(index)).copied();
    if particle(177)
        != Some(ParticleParameter {
            base_tmap: 96,
            word_2: 8492,
            word_4: 42,
            speed: 389,
            rotation_speed: 400,
            byte_10: 0,
            byte_11: -1,
            render_mode: 0,
            byte_13: 0,
        })
    {
        return Err(NativeWorldError::new(
            "Level 2 ordinary house particle row 177 changed",
        ));
    }
    if presentation.level_sprite_selectors.get(96) != Some(&255) {
        return Err(NativeWorldError::new(
            "Level 2 ordinary house particle selector 96 changed",
        ));
    }
    let expected_particles = [
        (
            39_u16,
            ParticleParameter {
                base_tmap: 79,
                word_2: 8492,
                word_4: 42,
                speed: 666,
                rotation_speed: 400,
                byte_10: 0,
                byte_11: 1,
                render_mode: 0,
                byte_13: 0,
            },
            0_u8,
        ),
        (
            239,
            ParticleParameter {
                base_tmap: 155,
                word_2: 8492,
                word_4: 42,
                speed: 90,
                rotation_speed: 200,
                byte_10: 0,
                byte_11: -1,
                render_mode: 20,
                byte_13: 0,
            },
            255,
        ),
        (
            241,
            ParticleParameter {
                base_tmap: 180,
                word_2: 8492,
                word_4: 42,
                speed: 116,
                rotation_speed: 250,
                byte_10: 0,
                byte_11: -1,
                render_mode: 20,
                byte_13: 0,
            },
            255,
        ),
        (
            242,
            ParticleParameter {
                base_tmap: 183,
                word_2: 8492,
                word_4: 42,
                speed: 116,
                rotation_speed: 250,
                byte_10: 0,
                byte_11: -1,
                render_mode: 20,
                byte_13: 0,
            },
            255,
        ),
        (
            271,
            ParticleParameter {
                base_tmap: 199,
                word_2: 8492,
                word_4: 42,
                speed: 96,
                rotation_speed: 300,
                byte_10: 0,
                byte_11: -1,
                render_mode: 20,
                byte_13: 0,
            },
            255,
        ),
    ];
    for (index, expected, selector) in expected_particles {
        if particle(index) != Some(expected)
            || presentation
                .level_sprite_selectors
                .get(usize::from(expected.base_tmap))
                != Some(&selector)
        {
            return Err(NativeWorldError::new(format!(
                "Level 2 ordinary presentation particle {index} changed"
            )));
        }
    }
    for (ordinal, index) in (1_u16..=22).enumerate() {
        let entity = entities
            .get(usize::from(index))
            .ok_or_else(|| NativeWorldError::new(format!("missing Level 2 house {index}")))?;
        if !entity.active
            || entity.class != 10
            || entity.model != 45
            || entity.action != 52
            || entity.source_word_10() != 2
            || entity.control_flags != 0x0000_000d
            || entity.particle_parameter_index != Some(177)
            || entity.parameter_relation != Some(59)
            || entity.special_relation != EntitySpecialRelation::GlobalSpecialState
            || entity.old_map_entity() != 0
            || entity.next_entity() != 0
            || sha256_hex(entity.record_bytes()) != LEVEL2_ORDINARY_HOUSE_RECORD_SHA256[ordinal]
        {
            return Err(NativeWorldError::new(format!(
                "Level 2 ordinary house census row {index} changed"
            )));
        }
    }
    for (ordinal, index) in LEVEL_TWO_VILLAGER_INDICES.iter().copied().enumerate() {
        let entity = entities
            .get(usize::from(index))
            .ok_or_else(|| NativeWorldError::new(format!("missing Level 2 villager {index}")))?;
        let expected_links = match index {
            77 => (0, 105),
            105 => (77, 0),
            97 => (0, 106),
            106 => (97, 0),
            _ => (0, 0),
        };
        let census_mismatch = !entity.active
            || entity.class != 5
            || entity.model != 13
            || entity.action != 105
            || entity.source_word_10() != 2
            || entity.control_flags != 0x0000_000c
            || entity.max_life != 1_000
            || entity.life != 1_000
            || entity.actual_speed != if index == 77 { 30 } else { 18 }
            || entity.parameter_relation != Some(100)
            || entity.special_relation != EntitySpecialRelation::GlobalSpecialState
            || entity.old_map_entity() != expected_links.0
            || entity.next_entity() != expected_links.1
            || !matches!(entity.particle_parameter_index, Some(239 | 241 | 242 | 271))
            || sha256_hex(entity.record_bytes()) != LEVEL2_ORDINARY_VILLAGER_RECORD_SHA256[ordinal];
        if census_mismatch {
            return Err(NativeWorldError::new(format!(
                "Level 2 ordinary villager census row {index} changed"
            )));
        }
    }
    Ok(())
}

/// Mutable Level 2 frame-zero state used by the generic flight machinery.
/// The owner package remains immutable; each call returns an isolated clone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level2ResetSession {
    profile: FlightProfile,
    baseline_state: FlightState,
    pub state: FlightState,
    baseline_terrain: SessionTerrain,
    pub terrain: SessionTerrain,
    baseline_context: FlightContext,
    pub context: FlightContext,
    entities: Box<[EntitySnapshot]>,
    players: Box<[PlayerSnapshot]>,
    runtime: RuntimeSnapshot,
    presentation: PresentationSelectors,
    baseline_bad_stones: Box<[Level2BadStone]>,
    bad_stones: Box<[Level2BadStone]>,
    baseline_shared_spells: Level2SharedSpellResetSession,
    shared_spells: Level2SharedSpellResetSession,
    economy: Level2EconomyState,
    ordinary: Level2OrdinaryResetSession,
    mission: MissionContentSession,
}

impl Level2ResetSession {
    fn new(
        profile: FlightProfile,
        state: FlightState,
        terrain: SessionTerrain,
        entities: Box<[EntitySnapshot]>,
        players: Box<[PlayerSnapshot]>,
        runtime: RuntimeSnapshot,
        presentation: PresentationSelectors,
        bad_stones: Box<[Level2BadStone]>,
        shared_spells: Level2SharedSpellResetSession,
        economy: Level2EconomyState,
        ordinary: Level2OrdinaryResetSession,
        mission: MissionContentSession,
    ) -> Self {
        // Pinned `sub_56D60` reset order (6806c7204d17):
        // ClearSettings -> LevelDecompress -> GenerateLevelMap /
        // GenerateFeatures -> InitStages -> InitStageVars -> Init0x3664C /
        // sub_4A1E0(0,1) -> sub_53160 -> sub_60F00. The package export has
        // already captured the resulting six terrain planes, arenas, runtime
        // stages, and player rows; this constructor only projects that
        // qualified frame zero into the generic flight session.
        let context = FlightContext {
            effects: Some(FlightEffectsContext {
                global_rng: runtime.random_state,
                local_player_index: runtime.local_player_slot,
                ..FlightEffectsContext::default()
            }),
            ..FlightContext::default()
        };
        Self {
            profile,
            baseline_state: state,
            state,
            baseline_terrain: terrain.clone(),
            terrain,
            baseline_context: context.clone(),
            context,
            entities,
            players,
            runtime,
            presentation,
            baseline_bad_stones: bad_stones.clone(),
            bad_stones,
            baseline_shared_spells: shared_spells.clone(),
            shared_spells,
            economy,
            ordinary,
            mission,
        }
    }

    #[must_use]
    pub const fn profile(&self) -> FlightProfile {
        self.profile
    }

    #[must_use]
    pub fn entities(&self) -> &[EntitySnapshot] {
        &self.entities
    }

    #[must_use]
    pub fn players(&self) -> &[PlayerSnapshot] {
        &self.players
    }

    #[must_use]
    pub const fn runtime(&self) -> &RuntimeSnapshot {
        &self.runtime
    }

    #[must_use]
    pub fn bad_stones(&self) -> &[Level2BadStone] {
        &self.bad_stones
    }

    #[must_use]
    pub fn shared_spells(&self) -> &Level2SharedSpellResetSession {
        &self.shared_spells
    }

    #[must_use]
    pub fn shared_spells_mut(&mut self) -> &mut Level2SharedSpellResetSession {
        &mut self.shared_spells
    }

    #[must_use]
    pub fn economy(&self) -> &Level2EconomyState {
        &self.economy
    }

    #[must_use]
    pub fn economy_mut(&mut self) -> &mut Level2EconomyState {
        &mut self.economy
    }

    #[must_use]
    pub fn ordinary(&self) -> &Level2OrdinaryResetSession {
        &self.ordinary
    }

    #[must_use]
    pub fn ordinary_mut(&mut self) -> &mut Level2OrdinaryResetSession {
        &mut self.ordinary
    }

    #[must_use]
    pub fn bad_stone_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(REMC2_SPEC_COMMIT.as_bytes());
        for stone in &self.bad_stones {
            hasher.update(stone.entity.record_bytes());
        }
        format!("{:x}", hasher.finalize())
    }

    #[must_use]
    pub fn bad_stone_presentation(
        &self,
        source_slot: u16,
    ) -> Option<&[u8; ENTITY_PARAMETER_BYTES]> {
        self.bad_stones
            .iter()
            .find(|stone| stone.entity.index == source_slot)
            .and_then(|stone| stone.entity.parameter_relation)
            .and_then(|index| self.presentation.entity_parameters.get(usize::from(index)))
    }

    #[must_use]
    pub fn bad_stone_particle_presentation(&self, source_slot: u16) -> Option<&ParticleParameter> {
        let index = self
            .bad_stones
            .iter()
            .find(|stone| stone.entity.index == source_slot)
            .and_then(|stone| stone.entity.particle_parameter_index)?;
        self.presentation.particles.get(usize::from(index))
    }

    #[must_use]
    pub fn bad_stone_sprite_selector(&self, source_slot: u16) -> Option<u8> {
        let particle = self.bad_stone_particle_presentation(source_slot)?;
        self.presentation
            .level_sprite_selectors
            .get(usize::from(particle.base_tmap))
            .copied()
    }

    pub fn bad_stone_sprite_disposition(
        &self,
        source_slot: u16,
    ) -> std::result::Result<SourceSpriteTemplateDisposition, String> {
        let stone = self
            .bad_stones
            .iter()
            .find(|stone| stone.entity.index == source_slot)
            .ok_or_else(|| format!("unknown Level 2 bad stone source slot {source_slot}"))?;
        level2_bad_stone_sprite_disposition(ImportedResetSpriteTuple {
            class: stone.entity.class,
            model: stone.entity.model,
            action: stone.entity.action,
            entity_parameter_index: stone.entity.parameter_relation.unwrap_or_default(),
            particle_parameter_index: stone.entity.particle_parameter_index.unwrap_or_default(),
            animation_frame: stone.entity.animation_frame(),
            control_flags: stone.entity.control_flags,
        })
    }

    pub fn step_bad_stone_action_9(&mut self, source_slot: u16) -> Result<Level2BadStoneStep> {
        let Some(stone) = self
            .bad_stones
            .iter_mut()
            .find(|stone| stone.entity.index == source_slot)
        else {
            return Err(NativeWorldError::new(format!(
                "Level 2 bad stone source slot {source_slot} is not active"
            )));
        };
        Ok(stone.step_action_9(&self.terrain))
    }

    #[must_use]
    pub fn mission(&self) -> &MissionContentSession {
        &self.mission
    }

    #[must_use]
    pub fn state_hash(&self) -> u64 {
        state_hash(&self.state)
    }

    #[must_use]
    pub fn terrain_hash(&self) -> u64 {
        self.terrain.deterministic_hash()
    }

    /// Execute one source outer turn against the Day terrain and reset state.
    pub fn step_outer_turn(
        &mut self,
        command: PlayerCommand,
        speed_index: u8,
    ) -> Vec<crate::StepEvidence> {
        crate::step_outer_turn_with_context(
            &mut self.state,
            command,
            speed_index,
            self.profile,
            &self.terrain,
            &mut self.context,
        )
    }

    /// Restore every mutable session projection to the decoded frame zero.
    pub fn reset(&mut self) {
        self.state = self.baseline_state;
        self.terrain = self.baseline_terrain.clone();
        self.context = self.baseline_context.clone();
        self.bad_stones.clone_from(&self.baseline_bad_stones);
        self.shared_spells = self.baseline_shared_spells.clone();
        self.economy.reset();
        self.ordinary.reset();
        self.mission.reset();
    }
}

fn level2_bad_stones(
    entities: &[EntitySnapshot],
    planes: &TerrainPlanes,
) -> Result<Box<[Level2BadStone]>> {
    LEVEL2_BAD_STONE_SLOTS
        .iter()
        .map(|slot| {
            let entity = entities.get(usize::from(*slot)).ok_or_else(|| {
                NativeWorldError::new(format!("missing Level 2 bad stone slot {slot}"))
            })?;
            Level2BadStone::from_entity(entity, planes)
        })
        .collect::<Result<Vec<_>>>()
        .map(Vec::into_boxed_slice)
}

struct CommonPackage {
    package: ValidatedPackage,
    metadata: LevelMetadata,
    terrain_planes: TerrainPlanes,
    terrain: LegacyTerrain,
    entities: Vec<EntitySnapshot>,
    players: Vec<PlayerSnapshot>,
    runtime: RuntimeSnapshot,
    presentation: PresentationSelectors,
    terrain_rng: u16,
    reset_state: Option<FlightState>,
    authored: Vec<u8>,
}

impl NativeWorld {
    /// Load and validate an export package and its separately supplied closure.
    ///
    /// # Errors
    ///
    /// Returns an error on any schema, identity, size, version, relation,
    /// reset, closure-binding, or frame-resolution mismatch.
    pub fn load(package_dir: impl AsRef<Path>, closure_path: impl AsRef<Path>) -> Result<Self> {
        let owner_cache = std::env::var_os("MC_ASSET_CACHE_PATH").map(PathBuf::from);
        Self::load_internal(
            package_dir.as_ref(),
            closure_path.as_ref(),
            owner_cache.as_deref(),
        )
    }

    /// Load without a private owner-asset cache, independent of the process
    /// environment.
    ///
    /// This keeps cache-free conformance fixtures deterministic when a parent
    /// qualification runner supplies `MC_ASSET_CACHE_PATH` for production-path
    /// tests in the same process.
    ///
    /// # Errors
    ///
    /// Returns an error on any package, closure, identity, size, version,
    /// relation, reset, or frame-resolution mismatch.
    pub fn load_without_owner_cache(
        package_dir: impl AsRef<Path>,
        closure_path: impl AsRef<Path>,
    ) -> Result<Self> {
        Self::load_internal(package_dir.as_ref(), closure_path.as_ref(), None)
    }

    /// Load with an explicit qualified private owner-asset cache.
    ///
    /// This is the deterministic equivalent of setting `MC_ASSET_CACHE_PATH`
    /// and avoids process-global environment mutation in tools and tests.
    ///
    /// # Errors
    ///
    /// Returns an error on any package, closure, cache identity, binding,
    /// output hash, or qualified descriptor mismatch.
    pub fn load_with_owner_cache(
        package_dir: impl AsRef<Path>,
        closure_path: impl AsRef<Path>,
        owner_cache: impl AsRef<Path>,
    ) -> Result<Self> {
        Self::load_internal(
            package_dir.as_ref(),
            closure_path.as_ref(),
            Some(owner_cache.as_ref()),
        )
    }

    /// Decode the Level 2 common owner package without constructing mission
    /// behavior. This is the production-path seam for the Level 2 profile:
    /// all package members, typed relations, reset state, and Day metadata
    /// are validated before the explicit unsupported-mission boundary.
    pub fn inspect_level2(package_dir: impl AsRef<Path>) -> Result<NativeWorldInspection> {
        let package_dir = package_dir.as_ref();
        let manifest_path = package_dir.join("manifest.json");
        let manifest_bytes = read_file(&manifest_path)?;
        let manifest: Value = serde_json::from_slice(&manifest_bytes)
            .map_err(|error| NativeWorldError::new(format!("invalid manifest JSON: {error}")))?;
        let object = as_object(&manifest, "manifest")?;
        let common = parse_common_package(package_dir, object, OwnerPackageProfile::Level2)?;
        Ok(NativeWorldInspection {
            profile: OwnerPackageProfile::Level2,
            schema: common.package.schema,
            metadata: common.metadata,
            package_sha256: common.package.package_sha256,
            terrain_planes: common.terrain_planes,
            terrain: common.terrain,
            entities: common.entities.into_boxed_slice(),
            players: common.players.into_boxed_slice(),
            runtime: common.runtime,
            presentation: common.presentation,
            terrain_rng: common.terrain_rng,
            reset_state: common.reset_state,
            authored_level: common.authored.into_boxed_slice(),
        })
    }

    fn load_internal(
        package_dir: &Path,
        closure_path: &Path,
        owner_cache: Option<&Path>,
    ) -> Result<Self> {
        let manifest_path = package_dir.join("manifest.json");
        let manifest_bytes = read_file(&manifest_path)?;
        let manifest: Value = serde_json::from_slice(&manifest_bytes)
            .map_err(|error| NativeWorldError::new(format!("invalid manifest JSON: {error}")))?;
        let object = as_object(&manifest, "manifest")?;
        let common = parse_common_package(package_dir, object, OwnerPackageProfile::Level1)?;
        let CommonPackage {
            package,
            metadata,
            terrain_planes,
            terrain,
            mut entities,
            players,
            runtime,
            presentation,
            terrain_rng: _,
            reset_state,
            authored: _,
        } = common;
        let reset_state = reset_state.ok_or_else(|| {
            NativeWorldError::new("Level 1 common package is missing reset_projection")
        })?;
        let members = package.members;
        let package_sha256 = package.package_sha256;

        let authored = member(&members, "authored-level.bin")?;
        let reset_context = build_level_one_context(&metadata, &reset_state, &players, &entities)?;
        validate_reset_contract(
            require_object(object, "reset_projection", "manifest")?,
            &reset_state,
            &reset_context,
        )?;
        apply_authority(&mut entities, &reset_state, &reset_context)?;
        let goat_stage_point = decode_level_one_goat_stage_point(&runtime)?;
        let goat_config = GoatBehaviorConfig::from_parameter_record(
            presentation
                .entity_parameters
                .get(98)
                .ok_or_else(|| NativeWorldError::new("Level 1 Goat parameter row 98 is absent"))?,
        )
        .map_err(NativeWorldError::new)?;
        VillagerBehaviorConfig::from_parameter_record(
            presentation.entity_parameters.get(100).ok_or_else(|| {
                NativeWorldError::new("Level 1 Villager parameter row 100 is absent")
            })?,
        )
        .map_err(NativeWorldError::new)?;
        let goat_render = presentation
            .particles
            .get(usize::from(GOAT_RENDER_PARAMETER_INDEX))
            .ok_or_else(|| NativeWorldError::new("Level 1 Goat render parameter 238 is absent"))?;
        let exact_goat_render = ParticleParameter {
            base_tmap: GOAT_BASE_TMAP,
            word_2: 8_492,
            word_4: 42,
            speed: 150,
            rotation_speed: 150,
            byte_10: 0,
            byte_11: 1,
            render_mode: 20,
            byte_13: 0,
        };
        if *goat_render != exact_goat_render {
            return Err(NativeWorldError::new(format!(
                "Level 1 Goat render parameter 238 mismatch: got {goat_render:?}, expected {exact_goat_render:?}"
            )));
        }
        let reset_live_world = LiveWorldState::from_entities_with_map(
            &entities,
            &terrain_planes.entity_indices,
            goat_stage_point,
            goat_config,
        )
        .map_err(NativeWorldError::new)?;
        let global_mana_denominator = level_one_global_mana_denominator(&entities)?;
        let initial_source_slots = entities
            .iter()
            .map(|entity| Level1ResetSourceSlot {
                index: entity.index,
                active: entity.active,
                class: entity.class,
                model: entity.model,
                action: entity.action,
                phase: entity.phase,
                entity_rng: entity.entity_rng,
                control_flags: entity.control_flags,
            })
            .collect::<Vec<_>>();
        let mut initial_ambient_entities = entities
            .iter()
            .filter_map(EntitySnapshot::ambient_entity)
            .collect::<Vec<_>>();
        initial_ambient_entities.sort_unstable_by_key(|entity| entity.source_entity_index);
        let emitter_slots = initial_ambient_entities
            .iter()
            .filter(|entity| (entity.model, entity.action) == (60, 65))
            .map(|entity| entity.source_entity_index)
            .collect::<Vec<_>>();
        let mut smoke_ordinal = 0_usize;
        for entity in &mut initial_ambient_entities {
            match (entity.model, entity.action) {
                (14, 14) => {
                    entity.parent_entity_index = emitter_slots[smoke_ordinal];
                    smoke_ordinal += 1;
                }
                (0, 0) => entity.parent_entity_index = entity.source_id,
                _ => {}
            }
        }
        let initial_castle_blockers = entities
            .iter()
            .filter(|entity| {
                entity.active && entity.life >= 0 && entity.class == 3 && entity.model == 2
            })
            .map(EntitySnapshot::castle_placement_blocker)
            .collect::<Vec<_>>();
        let initial_building_blockers = entities
            .iter()
            .filter(|entity| entity.active && entity.class == 10 && entity.model == 45)
            .map(EntitySnapshot::castle_placement_blocker)
            .collect::<Vec<_>>();
        let initial_possession_targets = entities
            .iter()
            .filter(|entity| {
                entity.active
                    && entity.life >= 0
                    && entity.class == 10
                    && entity.model == 45
                    // remc2 `sub_108B0` rejects model-45 buildings whose
                    // BLDGPRM byte_2 has bit 8 set. In the qualified Level 1
                    // package those are rows 17 and 51..=53.
                    && !matches!(entity.level_gate, 17 | 51 | 52 | 53)
            })
            .map(EntitySnapshot::collision_target)
            .collect::<Vec<_>>();
        let initial_source_collision_targets = entities
            .iter()
            .filter(|entity| entity.active && entity.control_flags & 8 != 0)
            .map(|entity| Level1SourceCollisionTarget {
                target: entity.collision_target(),
                source_id: entity.source_id,
            })
            .collect::<Vec<_>>();
        let (closure_sha256, mut asset_refs) =
            parse_closure(closure_path, &package_sha256, &manifest_bytes, &members)?;
        let cache_assets = match owner_cache {
            Some(cache_path) => append_level_one_cache_assets(
                cache_path,
                &package_sha256,
                &closure_sha256,
                &mut asset_refs,
            )?,
            None => LevelOneCacheAssets::default(),
        };
        let mut level1_mission =
            Level1MissionTemplate::decode_with_source_slots_and_placement_blockers(
                authored,
                &terrain,
                global_mana_denominator,
                runtime.random_state,
                &initial_source_slots,
                &initial_ambient_entities,
                &initial_castle_blockers,
                &initial_building_blockers,
                &initial_possession_targets,
                &initial_source_collision_targets,
                runtime.objective_presentation_phase,
                runtime.objective_textbox_ticks,
                cache_assets.castle_stage_one_footprint,
            )
            .map_err(NativeWorldError::new)?;
        let raw_source_states = entities
            .iter()
            .map(|entity| Level1ResetRawSourceState {
                index: entity.index,
                source_id: entity.source_id,
                life: entity.life,
                building_index: i16::from_le_bytes([
                    entity.record_bytes()[20],
                    entity.record_bytes()[21],
                ]),
                axis: Position {
                    x: u16::from_le_bytes([entity.record_bytes()[158], entity.record_bytes()[159]]),
                    y: u16::from_le_bytes([entity.record_bytes()[160], entity.record_bytes()[161]]),
                    z: i16::from_le_bytes([entity.record_bytes()[162], entity.record_bytes()[163]]),
                },
                word_24: entity.word_24(),
                word_26: entity.word_26(),
                word_2e: i16::from_le_bytes([entity.record_bytes()[50], entity.record_bytes()[51]]),
            })
            .collect::<Vec<_>>();
        level1_mission
            .bind_initial_raw_source_states(&raw_source_states)
            .map_err(NativeWorldError::new)?;
        let raw_source_entries =
            raw_source_map_chain_entries(&terrain_planes.entity_indices, |source_entity_index| {
                entities
                    .get(usize::from(source_entity_index))
                    .map(EntitySnapshot::old_map_entity)
            })
            .map_err(NativeWorldError::new)?;
        let raw_source_entity_indices = raw_source_entries
            .iter()
            .map(|entry| entry.entity_index)
            .collect::<Vec<_>>();
        let reset_source_map_seeds = raw_source_entries
            .iter()
            .map(|entry| {
                let entity = &entities[usize::from(entry.entity_index)];
                RenderableMapSeed {
                    entity_index: entry.entity_index,
                    active: entity.active,
                    x: entity.position.x,
                    y: entity.position.y,
                }
            })
            .collect::<Vec<_>>();
        let reset_source_map_chains = RenderableMapChains::from_raw_reset(
            &reset_source_map_seeds,
            &terrain_planes.entity_indices,
            |source_entity_index| {
                entities
                    .get(usize::from(source_entity_index))
                    .map(EntitySnapshot::old_map_entity)
            },
            |source_entity_index| {
                entities
                    .get(usize::from(source_entity_index))
                    .map(EntitySnapshot::next_entity)
            },
        )
        .map_err(NativeWorldError::new)?;

        let profile = metadata.map_type.profile();
        Ok(Self {
            metadata,
            package_sha256,
            closure_sha256,
            profile,
            terrain_planes,
            terrain,
            entities: entities.into_boxed_slice(),
            players: players.into_boxed_slice(),
            runtime,
            presentation,
            asset_refs: asset_refs.into_boxed_slice(),
            sound_asset_refs: cache_assets.sound_asset_refs,
            music_asset_ref: cache_assets.music_asset_ref,
            goat_tmap_bank: cache_assets.goat_tmap_bank,
            source_sprite_tmap_bank: cache_assets.source_sprite_tmap_bank,
            reset_source_map_chains,
            raw_source_entity_indices: raw_source_entity_indices.into_boxed_slice(),
            house_build_footprints: cache_assets.house_build_footprints,
            reset_state,
            reset_context,
            goat_stage_point,
            goat_config,
            reset_live_world,
            level1_mission,
        })
    }

    #[must_use]
    pub const fn metadata(&self) -> &LevelMetadata {
        &self.metadata
    }

    #[must_use]
    pub const fn package_sha256(&self) -> &str {
        self.package_sha256.as_str()
    }

    #[must_use]
    pub const fn closure_sha256(&self) -> &str {
        self.closure_sha256.as_str()
    }

    #[must_use]
    pub const fn profile(&self) -> FlightProfile {
        self.profile
    }

    #[must_use]
    pub const fn terrain_planes(&self) -> &TerrainPlanes {
        &self.terrain_planes
    }

    #[must_use]
    pub const fn terrain(&self) -> &LegacyTerrain {
        &self.terrain
    }

    #[must_use]
    pub fn entities(&self) -> &[EntitySnapshot] {
        &self.entities
    }

    #[must_use]
    pub fn house_build_footprint(&self, building_index: u8) -> Option<&CastleBuildFootprint> {
        self.house_build_footprints.get(&building_index)
    }

    #[must_use]
    pub fn players(&self) -> &[PlayerSnapshot] {
        &self.players
    }

    #[must_use]
    pub const fn runtime(&self) -> &RuntimeSnapshot {
        &self.runtime
    }

    #[must_use]
    pub const fn presentation(&self) -> &PresentationSelectors {
        &self.presentation
    }

    #[must_use]
    pub fn asset_refs(&self) -> &[AssetRef] {
        &self.asset_refs
    }

    #[must_use]
    pub fn sound_asset_ref(&self, sound_id: u16) -> Option<u32> {
        self.sound_asset_refs.get(&sound_id).copied()
    }

    #[must_use]
    pub const fn music_asset_ref(&self) -> Option<u32> {
        self.music_asset_ref
    }

    #[must_use]
    pub const fn goat_tmap_bank(&self) -> Option<&GoatTmapBank> {
        self.goat_tmap_bank.as_ref()
    }

    #[must_use]
    pub const fn source_sprite_tmap_bank(&self) -> Option<&SourceSpriteTmapBank> {
        self.source_sprite_tmap_bank.as_ref()
    }

    #[must_use]
    pub fn raw_source_entity_indices(&self) -> &[u16] {
        &self.raw_source_entity_indices
    }

    #[must_use]
    pub const fn reset_source_map_chains(&self) -> &RenderableMapChains {
        &self.reset_source_map_chains
    }

    #[must_use]
    pub const fn reset_state(&self) -> &FlightState {
        &self.reset_state
    }

    #[must_use]
    pub const fn reset_context(&self) -> &FlightContext {
        &self.reset_context
    }

    #[must_use]
    pub const fn goat_stage_point(&self) -> Position {
        self.goat_stage_point
    }

    #[must_use]
    pub const fn goat_config(&self) -> GoatBehaviorConfig {
        self.goat_config
    }

    #[must_use]
    pub fn reset_live_world(&self) -> &LiveWorldState {
        &self.reset_live_world
    }

    #[must_use]
    pub const fn level1_mission(&self) -> &Level1MissionTemplate {
        &self.level1_mission
    }

    /// Build the mutable per-session context.
    ///
    /// The package's reset projection predates typed presentation events, so
    /// `reset_context` must remain byte-for-byte compatible with its preserved
    /// conformance hash. Level 1 effects are an additive session surface.
    #[must_use]
    pub fn session_context(&self) -> FlightContext {
        let mut context = self.reset_context.clone();
        context.effects = Some(FlightEffectsContext {
            local_player_index: self.runtime.local_player_slot,
            periodic_spell_effect: None,
            ..FlightEffectsContext::default()
        });
        // The preserved reset conformance projection intentionally contains
        // only the flight-critical left holder. A live Level 1 session needs
        // the complete owned hand state: Fireball in the left hand and
        // Possession in the right, plus the enabled-family lookup used by
        // selected casting.
        let player = &self.players[usize::from(self.runtime.local_player_slot)];
        if let Some(fire) = context.spell_fire.as_mut() {
            let left_slot = usize::from(player.left_spell_slot);
            let right_slot = usize::from(player.right_spell_slot);
            let left_index = player.enabled_spell_holders[left_slot];
            let right_index = player.enabled_spell_holders[right_slot];
            fire.left_holder_index = left_index;
            fire.right_holder_index = right_index;
            fire.left_subspell_index = player.left_subspell_index;
            fire.right_subspell_index = player.right_subspell_index;
            fire.selected_spell_ui_index = player.selected_spell_ui_index;
            fire.selected_subspell_index = player.selected_subspell_index;
            fire.spell_hand_eligibility = player.spell_hand_eligibility;
            fire.family_selected_tiers = player.family_selected_tiers;
            let selected_family =
                SOURCE_SPELL_INDEX_MAP[usize::from(player.selected_spell_ui_index)];
            fire.selected_holder_index = player.enabled_spell_holders[usize::from(selected_family)];
            fire.enabled_holder_indices[left_slot] = left_index;
            fire.enabled_holder_indices[right_slot] = right_index;
            if right_index != 0
                && right_index != left_index
                && let Some(holder) = self.entities.get(usize::from(right_index))
            {
                fire.holders.push(SpellActivationHolder {
                    entity_index: holder.index,
                    model: holder.model,
                    level_gate: holder.level_gate,
                    active_value: holder.active_value,
                    activation_value: holder.activation_value,
                    mana_cost: holder.mana_cost,
                    action_marker: holder.action_marker,
                });
            }
        }
        context
    }

    #[must_use]
    pub fn session(&self) -> NativeSession<'_> {
        let mut mission = self.level1_mission.session(
            self.entities[usize::from(self.reset_state.entity_index)].life,
            1_000,
        );
        mission.set_source_turn_counter(
            self.players[usize::from(self.runtime.local_player_slot)]
                .turn
                .cast_unsigned(),
        );
        mission.set_reset_source_map_chains(self.reset_source_map_chains.clone());
        NativeSession {
            world: self,
            terrain: SessionTerrain::from_world(self),
            state: self.reset_state,
            context: self.session_context(),
            live_world: self.reset_live_world.clone(),
            goat_tmap_clocks: self.goat_tmap_bank.as_ref().map(GoatTmapClocks::from_bank),
            camera_fov: 128,
            pending_house_effects: Vec::new(),
            mission,
        }
    }
}

impl FlightWorld for NativeWorld {
    fn ground_height(&self, position: Position) -> i16 {
        self.terrain.ground_height(position)
    }

    fn ceiling_height(&self, position: Position) -> Option<i16> {
        self.terrain.ceiling_height(position)
    }

    fn material_mask(&self, position: Position) -> u32 {
        self.terrain.material_mask(position)
    }

    fn terrain_type(&self, position: Position) -> u8 {
        self.terrain.terrain_type(position)
    }

    fn angle_flags(&self, tile: crate::Tile) -> u8 {
        self.terrain.angle_flags(tile)
    }
}

/// Per-play-session mutable state, independent from the immutable world.
pub struct NativeSourceFrameResources<'a> {
    pub frozen_actor_tmap_bank: Option<&'a FrozenActorTmapBank>,
    pub villager_tmap_clocks: Option<&'a VillagerTmapClocks>,
    pub archer_tmap_bank: &'a ArcherTmapBank,
    pub archer_tmap_clocks: &'a ArcherTmapClocks,
    pub firefly_tmap_bank: &'a FireflyTmapBank,
    pub firefly_tmap_clocks: &'a FireflyTmapClocks,
    pub combat_tmap_bank: &'a CombatTmapBank,
    pub static_world_tmap_bank: &'a StaticWorldTmapBank,
    pub economy_tmap_bank: &'a EconomyTmapBank,
    pub fireball_tmap_clock: &'a FireballTmapClock,
    pub possession_tmap_clock: &'a PossessionTmapClock,
    pub castle_tmap_clock: &'a CastleTmapClock,
}

/// Per-play-session mutable state, independent from the immutable world.
#[derive(Debug)]
pub struct NativeSession<'world> {
    world: &'world NativeWorld,
    pub terrain: SessionTerrain,
    pub state: FlightState,
    pub context: FlightContext,
    pub live_world: LiveWorldState,
    pub goat_tmap_clocks: Option<GoatTmapClocks>,
    pub camera_fov: i16,
    pub pending_house_effects: Vec<HouseStepEvent>,
    pub mission: Level1MissionState,
}

fn goat_source_entity_view(goat: &GoatState) -> Level1SourceEntityView {
    Level1SourceEntityView {
        source_entity_index: goat.entity_index,
        source_id: goat.source_id,
        authority: Level1SourceEntityAuthority::LiveGoat,
        active: goat.active,
        visible: goat.active,
        class: GOAT_CLASS,
        model: GOAT_MODEL,
        action: goat.action,
        phase: goat.phase,
        entity_rng: goat.rng,
        control_flags: goat.control_flags,
        position: goat.position,
        max_life: goat.max_life,
        life: goat.life,
        pending_damage: goat.pending_damage,
        pending_attacker: goat.pending_attacker.cast_unsigned(),
        owner_entity_index: goat.current_attacker.cast_unsigned(),
        target_entity_index: goat.target_entity_index.unwrap_or(0),
        linked_entity_index: 0,
        actual_speed: goat.actual_speed,
        maximum_speed: goat.maximum_speed,
        collision_half_width: goat.collision_half_width,
        collision_half_depth: goat.collision_half_width,
        collision_vertical_offset: goat.collision_vertical_offset,
        collision_half_height: goat.collision_half_height,
        villager_entry_eligible: false,
        builder_placement_latch: 0,
    }
}

fn house_source_entity_view(house: &HouseState) -> Level1SourceEntityView {
    Level1SourceEntityView {
        source_entity_index: house.entity_index,
        source_id: house.source_id,
        authority: Level1SourceEntityAuthority::LiveHouse,
        active: house.active,
        visible: house.active,
        class: HOUSE_CLASS,
        model: HOUSE_MODEL,
        action: house.action,
        phase: house.phase,
        entity_rng: house.rng,
        control_flags: house.control_flags,
        position: house.position,
        max_life: house.max_life,
        life: house.life,
        pending_damage: house.pending_damage,
        pending_attacker: house.pending_attacker.cast_unsigned(),
        owner_entity_index: house.owner_entity_index,
        target_entity_index: 0,
        linked_entity_index: 0,
        actual_speed: 0,
        maximum_speed: 0,
        collision_half_width: house.collision_half_width,
        collision_half_depth: house.collision_half_depth,
        collision_vertical_offset: house.collision_vertical_offset,
        collision_half_height: house.collision_half_height,
        villager_entry_eligible: house.parameter.produces_villagers(),
        builder_placement_latch: 0,
    }
}

fn villager_source_entity_view(villager: &VillagerState) -> Level1SourceEntityView {
    Level1SourceEntityView {
        source_entity_index: villager.entity_index,
        source_id: villager.source_id,
        authority: Level1SourceEntityAuthority::LiveVillager,
        active: villager.active,
        visible: villager.active,
        class: VILLAGER_CLASS,
        model: VILLAGER_MODEL,
        action: villager.action,
        phase: villager.phase,
        entity_rng: villager.rng,
        control_flags: villager.control_flags,
        position: villager.position,
        max_life: villager.max_life,
        life: villager.life,
        pending_damage: villager.pending_damage,
        pending_attacker: villager.pending_attacker.cast_unsigned(),
        owner_entity_index: villager.current_attacker.cast_unsigned(),
        target_entity_index: villager.target_entity_index.unwrap_or(0),
        linked_entity_index: 0,
        actual_speed: villager.actual_speed,
        maximum_speed: villager.maximum_speed,
        collision_half_width: villager.collision_half_width,
        collision_half_depth: villager.collision_half_width,
        collision_vertical_offset: villager.collision_vertical_offset,
        collision_half_height: villager.collision_half_height,
        villager_entry_eligible: false,
        builder_placement_latch: 0,
    }
}

fn current_live_world_source_entity_view(
    live_world: &LiveWorldState,
    source_entity_index: u16,
) -> Option<Level1SourceEntityView> {
    live_world
        .goats()
        .iter()
        .find(|goat| goat.entity_index == source_entity_index)
        .map(goat_source_entity_view)
        .or_else(|| {
            live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == source_entity_index)
                .map(house_source_entity_view)
        })
        .or_else(|| {
            live_world
                .villagers()
                .iter()
                .find(|villager| villager.entity_index == source_entity_index)
                .map(villager_source_entity_view)
        })
}

/// Update only the current host-owned row after that source action. The
/// turn-start vector remains allocated and ordered once; this does not admit
/// the row into any frozen remc2 seeker list.
pub fn refresh_level1_current_source_view(
    source_views: &mut Vec<Level1SourceEntityView>,
    live_world: &LiveWorldState,
    source_entity_index: u16,
) {
    let Some(current) = current_live_world_source_entity_view(live_world, source_entity_index)
    else {
        return;
    };
    match source_views.binary_search_by_key(&source_entity_index, |view| view.source_entity_index) {
        Ok(index) => source_views[index] = current,
        Err(index) => source_views.insert(index, current),
    }
}

/// Build the production turn-start source snapshot from the existing typed
/// authorities. The returned values live only on the caller's stack/heap for
/// the duration of this source turn.
#[must_use]
pub fn level1_turn_start_source_views(
    mission: &Level1MissionState,
    player: &FlightState,
    live_world: &LiveWorldState,
) -> Vec<Level1SourceEntityView> {
    let mut imported = Vec::with_capacity(
        live_world.goats().len() + live_world.houses().len() + live_world.villagers().len(),
    );
    imported.extend(live_world.goats().iter().map(goat_source_entity_view));
    imported.extend(live_world.houses().iter().map(house_source_entity_view));
    imported.extend(
        live_world
            .villagers()
            .iter()
            .map(villager_source_entity_view),
    );
    mission.source_entity_views(player, &imported)
}

/// Apply one resident dispatch's effects after its mission borrow has ended.
///
/// House entry stages both authorities and commits them together. Rejected
/// full/dead entries leave both hashes unchanged. Returned seeds let the C API
/// update its presentation chain from the same applied effect; the direct
/// session already receives the mission-owned chain update.
pub fn apply_level1_source_effects(
    mission: &mut Level1MissionState,
    live_world: &mut LiveWorldState,
    effects: &[Level1SourceEffect],
) -> std::result::Result<Vec<RenderableMapSeed>, String> {
    if effects.is_empty() {
        return Ok(Vec::new());
    }

    let mut staged_mission = mission.clone();
    let mut staged_world = live_world.clone();
    let mut render_updates = Vec::new();
    let applied = (|| {
        for effect in effects.iter().copied() {
            match effect {
                Level1SourceEffect::CreateBuilderHouse { house } => {
                    staged_world.insert_builder_house(house)?;
                    staged_mission.update_external_collision_target(house_collision_target(&house));
                    if let Some(seed) = staged_mission.apply_local_source_effect(effect)? {
                        render_updates.push(seed);
                    }
                }
                Level1SourceEffect::InitializeBuilderHouse { house } => {
                    staged_world.initialize_builder_house(house)?;
                    staged_mission.update_external_collision_target(house_collision_target(&house));
                    staged_mission.apply_local_source_effect(effect)?;
                }
                Level1SourceEffect::SetBuilderHouseAction {
                    source_entity_index,
                    action,
                } => {
                    staged_world.set_builder_house_action(source_entity_index, action)?;
                    staged_mission.apply_local_source_effect(effect)?;
                }
                Level1SourceEffect::TryEnterResident {
                    resident_source_entity_index,
                    house_source_entity_index,
                } => {
                    if staged_mission.owns_authored_house(house_source_entity_index) {
                        if let Some(seed) = staged_mission.apply_local_source_effect(effect)? {
                            render_updates.push(seed);
                        }
                        continue;
                    }
                    if staged_world.try_enter_resident(house_source_entity_index)
                        == HouseResidentEntry::Entered
                    {
                        if let Some(seed) = staged_mission.apply_local_source_effect(
                            Level1SourceEffect::DisableSourceSlot {
                                source_entity_index: resident_source_entity_index,
                            },
                        )? {
                            render_updates.push(seed);
                        }
                    }
                }
                Level1SourceEffect::TryEnterVillager {
                    resident_source_entity_index,
                    house_source_entity_index,
                } => {
                    if staged_mission.owns_authored_house(house_source_entity_index) {
                        staged_mission.apply_local_source_effect(effect)?;
                        continue;
                    }
                    if staged_world.try_enter_resident(house_source_entity_index)
                        != HouseResidentEntry::Entered
                    {
                        staged_mission
                            .reject_dynamic_villager_entry(resident_source_entity_index)?;
                    }
                }
                _ => {
                    if let Some(seed) = staged_mission.apply_local_source_effect(effect)? {
                        render_updates.push(seed);
                    }
                }
            }
        }
        Ok::<(), String>(())
    })();
    if let Err(error) = applied {
        if let Some(source_entity_index) = effects.iter().find_map(|effect| match effect {
            Level1SourceEffect::CreateBuilderHouse { house } => Some(house.entity_index),
            _ => None,
        }) {
            let _ = mission.rollback_builder_house_allocation(source_entity_index);
        }
        return Err(error);
    }
    *mission = staged_mission;
    *live_world = staged_world;
    Ok(render_updates)
}

fn goat_collision_target(goat: &GoatState) -> Level1CollisionTarget {
    Level1CollisionTarget {
        entity_index: goat.entity_index,
        position: goat.position,
        collision_half_width: goat.collision_half_width,
        collision_vertical_offset: goat.collision_vertical_offset,
        collision_half_height: goat.collision_half_height,
        active: goat.active,
    }
}

fn villager_collision_target(villager: &VillagerState) -> Level1CollisionTarget {
    Level1CollisionTarget {
        entity_index: villager.entity_index,
        position: villager.position,
        collision_half_width: villager.collision_half_width,
        collision_vertical_offset: villager.collision_vertical_offset,
        collision_half_height: villager.collision_half_height,
        active: villager.active,
    }
}

fn source_sprite_disposition_index(disposition: &SourceSpriteDisposition) -> u16 {
    match disposition {
        SourceSpriteDisposition::Render(submission) => submission.source_entity_index,
        SourceSpriteDisposition::Legacy {
            source_entity_index,
            ..
        }
        | SourceSpriteDisposition::Excluded {
            source_entity_index,
            ..
        } => *source_entity_index,
    }
}

fn house_collision_target(house: &HouseState) -> Level1CollisionTarget {
    Level1CollisionTarget {
        entity_index: house.entity_index,
        position: house.position,
        collision_half_width: house.collision_half_width,
        collision_vertical_offset: house.collision_vertical_offset,
        collision_half_height: house.collision_half_height,
        active: house.active,
    }
}

impl NativeSession<'_> {
    /// Compose the legacy typed live-world hash with the authoritative
    /// mission-owned global RNG under the explicit v2 domain.
    #[must_use]
    pub fn live_world_hash_v2(&self) -> u64 {
        self.live_world.hash_v2(self.mission.global_rng())
    }

    #[cfg(test)]
    pub(crate) fn first_goat_source_view_for_test(&self) -> Level1SourceEntityView {
        goat_source_entity_view(&self.live_world.goats()[0])
    }

    #[must_use]
    pub fn source_sprite_dispositions(
        &self,
    ) -> std::result::Result<Vec<crate::source_sprite_tmap::SourceSpriteDisposition>, String> {
        let mut dispositions = self.mission.source_sprite_dispositions(&self.state)?;
        let mut qualified = dispositions
            .iter()
            .map(source_sprite_disposition_index)
            .collect::<BTreeSet<_>>();
        let legacy = self.mission.source_sprite_legacy_dispositions(&qualified)?;
        qualified.extend(legacy.iter().map(source_sprite_disposition_index));
        dispositions.extend(legacy);

        let mut imported_rows = Vec::new();
        for source_entity_index in self.mission.source_map_chains().linked_entity_indices() {
            if qualified.contains(&source_entity_index) {
                continue;
            }
            let entity = self
                .world
                .entities
                .get(usize::from(source_entity_index))
                .ok_or_else(|| unknown_source_sprite_tuple(source_entity_index, 0, 0, 0, 0))?;
            let source = self
                .mission
                .source_slots()
                .get(usize::from(source_entity_index))
                .ok_or_else(|| unknown_source_sprite_tuple(source_entity_index, 0, 0, 0, 0))?;
            let route = if entity.authority == EntityAuthority::GoatLive {
                if (source.class, source.model) != (GOAT_CLASS, GOAT_MODEL)
                    || !matches!(source.action, 9 | 12 | 13 | 14 | GOAT_PATROL_ACTION)
                {
                    return Err(format!(
                        "live Goat source row mismatch: slot={source_entity_index} source={}/{}/{} phase={} rng={} flags={:#x}",
                        source.class,
                        source.model,
                        source.action,
                        source.phase,
                        source.entity_rng,
                        source.control_flags,
                    ));
                }
                SourceSpriteContractRoute::Legacy(SourceSpriteLegacyFamily::Goat)
            } else if entity.authority == EntityAuthority::VillagerLive {
                if (source.class, source.model) != (VILLAGER_CLASS, VILLAGER_MODEL)
                    || !matches!(source.action, 104..=111)
                {
                    return Err(format!(
                        "live Villager source row mismatch: slot={source_entity_index} source={}/{}/{} phase={} rng={} flags={:#x}",
                        source.class,
                        source.model,
                        source.action,
                        source.phase,
                        source.entity_rng,
                        source.control_flags,
                    ));
                }
                SourceSpriteContractRoute::Legacy(SourceSpriteLegacyFamily::Resident)
            } else {
                let tuple = ImportedResetSpriteTuple {
                    class: source.class,
                    model: source.model,
                    action: source.action,
                    entity_parameter_index: entity.parameter_relation.unwrap_or(0),
                    particle_parameter_index: entity.particle_parameter_index.unwrap_or(0),
                    animation_frame: entity.animation_frame(),
                    control_flags: entity.control_flags,
                };
                match imported_reset_sprite_disposition(tuple).map_err(|_| {
                    unknown_source_sprite_tuple(
                        source_entity_index,
                        source.class,
                        source.model,
                        source.action,
                        entity.particle_parameter_index.unwrap_or(0),
                    )
                })? {
                    SourceSpriteTemplateDisposition::Render(_) => {
                        SourceSpriteContractRoute::Legacy(SourceSpriteLegacyFamily::ImportedReset)
                    }
                    SourceSpriteTemplateDisposition::Excluded(reason) => {
                        SourceSpriteContractRoute::Excluded(reason)
                    }
                }
            };
            imported_rows.push(SourceSpriteContractRow {
                source_entity_index,
                position: entity.position,
                yaw: entity.yaw,
                pitch: entity.pitch,
                roll: 0,
                control_flags: source.control_flags,
                class: source.class,
                model: source.model,
                action: source.action,
                xclass: 0,
                xmodel: 0,
                route,
            });
        }
        let imported = source_sprite_dispositions(imported_rows)?;
        qualified.extend(imported.iter().map(source_sprite_disposition_index));
        dispositions.extend(imported);
        dispositions.sort_unstable_by_key(source_sprite_disposition_index);

        for source_entity_index in self.mission.source_map_chains().linked_entity_indices() {
            if !qualified.contains(&source_entity_index) {
                let Some(source) = self
                    .mission
                    .source_slots()
                    .get(usize::from(source_entity_index))
                else {
                    return Err(unknown_source_sprite_tuple(source_entity_index, 0, 0, 0, 0));
                };
                return Err(unknown_source_sprite_tuple(
                    source_entity_index,
                    source.class,
                    source.model,
                    source.action,
                    0,
                ));
            }
        }
        Ok(dispositions)
    }

    #[cfg(test)]
    pub(crate) fn clone_for_source_frame_test(&self) -> Self {
        Self {
            world: self.world,
            terrain: self.terrain.clone(),
            state: self.state,
            context: self.context.clone(),
            live_world: self.live_world.clone(),
            goat_tmap_clocks: self.goat_tmap_clocks.clone(),
            camera_fov: self.camera_fov,
            pending_house_effects: self.pending_house_effects.clone(),
            mission: self.mission.clone(),
        }
    }

    /// Render the complete source-ordered frame from this session's actual
    /// production state and the cache-backed CP1 source TMAP bank.
    ///
    /// Callers supply only the established immutable legacy selector banks;
    /// source rows, map chains, live families, particle parameters, and the
    /// local player are always projected from this session.
    pub fn render_source_world_frame(
        &self,
        camera: Level1TerrainFrameCamera,
        terrain: Level1TerrainFrameResources<'_>,
        banks: NativeSourceFrameResources<'_>,
    ) -> std::result::Result<Level1TerrainFrame, TerrainFrameError> {
        self.render_source_world_frame_inner(camera, terrain, banks, None)
    }

    #[cfg(test)]
    pub(crate) fn render_source_world_frame_with_typed_no_draw_for_test(
        &self,
        camera: Level1TerrainFrameCamera,
        terrain: Level1TerrainFrameResources<'_>,
        banks: NativeSourceFrameResources<'_>,
        source_entity_index: u16,
    ) -> std::result::Result<Level1TerrainFrame, TerrainFrameError> {
        self.render_source_world_frame_inner(camera, terrain, banks, Some(source_entity_index))
    }

    fn render_source_world_frame_inner(
        &self,
        camera: Level1TerrainFrameCamera,
        terrain: Level1TerrainFrameResources<'_>,
        banks: NativeSourceFrameResources<'_>,
        typed_no_draw_source: Option<u16>,
    ) -> std::result::Result<Level1TerrainFrame, TerrainFrameError> {
        let source_bank = self.world.source_sprite_tmap_bank.as_ref().ok_or_else(|| {
            TerrainFrameError("production source frame has no qualified owner TMAP bank".to_owned())
        })?;
        let goat_tmap_clocks = self.goat_tmap_clocks.as_ref().ok_or_else(|| {
            TerrainFrameError("production source frame has no live Goat TMAP clocks".to_owned())
        })?;
        let mut dispositions = self
            .source_sprite_dispositions()
            .map_err(TerrainFrameError)?;
        if let Some(source_entity_index) = typed_no_draw_source {
            let disposition = dispositions
                .iter_mut()
                .find(|disposition| {
                    source_sprite_disposition_index(disposition) == source_entity_index
                })
                .ok_or_else(|| {
                    TerrainFrameError(format!(
                        "typed no-draw source slot {source_entity_index} has no disposition"
                    ))
                })?;
            if !matches!(disposition, SourceSpriteDisposition::Render(_)) {
                return Err(TerrainFrameError(format!(
                    "typed no-draw source slot {source_entity_index} is not renderable"
                )));
            }
            *disposition = SourceSpriteDisposition::Excluded {
                source_entity_index,
                reason: crate::source_sprite_tmap::SourceSpriteExclusionReason::SourceNoDrawFlags,
            };
        }
        let mut source_map_chains = self.mission.source_map_chains().clone();
        for disposition in &dispositions {
            let SourceSpriteDisposition::Render(submission) = disposition else {
                continue;
            };
            if submission.source_entity_index != self.state.entity_index {
                continue;
            }
            source_map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: submission.source_entity_index,
                    active: true,
                    x: submission.position.x,
                    y: submission.position.y,
                })
                .map_err(TerrainFrameError)?;
        }
        let frozen_actors = self
            .world
            .entities
            .iter()
            .filter(|entity| {
                entity.active
                    && ((entity.class == 2 && entity.model == 2)
                        || (entity.class == 5 && entity.model == 13))
            })
            .map(|entity| Level1FrozenActorRender {
                source_entity_index: entity.index,
                position: entity.position,
                yaw: entity.yaw,
                render_parameter_index: entity.particle_parameter_index.unwrap_or(0),
            })
            .collect::<Vec<_>>();
        let ambient_entities = self.mission.ambient_entities().copied().collect::<Vec<_>>();
        let dynamic_residents = self
            .mission
            .dynamic_residents()
            .copied()
            .collect::<Vec<_>>();
        let castle_root = self
            .mission
            .castle_exists
            .then_some(Level1CastleRootRender {
                source_entity_index: self.mission.castle_source_entity_index,
                position: self.mission.castle_position,
                active: true,
            });
        render_level1_source_world_frame(
            camera,
            terrain,
            Level1WorldFrameResources {
                local_player_entity_index: self.state.entity_index,
                goats: self.live_world.goats(),
                frozen_actors: &frozen_actors,
                ambient_entities: &ambient_entities,
                frozen_actor_tmap_bank: banks.frozen_actor_tmap_bank,
                villager_tmap_clocks: banks.villager_tmap_clocks,
                mission_actors: self.mission.actors(),
                dynamic_residents: &dynamic_residents,
                mission_segments: self.mission.multipart_segments(),
                castle_root,
                mana_spheres: self.mission.mana_spheres(),
                mana_carrier: self.mission.mana_carrier(),
                map_chains: &source_map_chains,
                goat_tmap_clocks,
                archer_tmap_bank: banks.archer_tmap_bank,
                archer_tmap_clocks: banks.archer_tmap_clocks,
                firefly_tmap_bank: banks.firefly_tmap_bank,
                firefly_tmap_clocks: banks.firefly_tmap_clocks,
                projectiles: self.mission.projectiles(),
                impacts: self.mission.impacts(),
                possessed_buildings: self.mission.possessed_buildings(),
                combat_tmap_bank: banks.combat_tmap_bank,
                static_world_tmap_bank: banks.static_world_tmap_bank,
                economy_tmap_bank: banks.economy_tmap_bank,
                fireball_tmap_clock: banks.fireball_tmap_clock,
                possession_tmap_clock: banks.possession_tmap_clock,
                castle_tmap_clock: banks.castle_tmap_clock,
                particle_parameters: &self.world.presentation.particles,
                source_slots: self.mission.source_slots(),
                night: self.world.metadata.map_type != MapType::Day,
            },
            crate::source_sprite_tmap::SourceSpriteWorldResources {
                dispositions: &dispositions,
                tmap_bank: source_bank,
            },
        )
    }

    pub fn reset(&mut self) {
        self.terrain = SessionTerrain::from_world(self.world);
        self.state = self.world.reset_state;
        self.context = self.world.session_context();
        self.live_world = self.world.reset_live_world.clone();
        self.goat_tmap_clocks = self
            .world
            .goat_tmap_bank
            .as_ref()
            .map(GoatTmapClocks::from_bank);
        self.camera_fov = 128;
        self.pending_house_effects.clear();
        self.mission = self.world.level1_mission.session(
            self.world.entities[usize::from(self.world.reset_state.entity_index)].life,
            1_000,
        );
        self.mission.set_source_turn_counter(
            self.world.players[usize::from(self.world.runtime.local_player_slot)]
                .turn
                .cast_unsigned(),
        );
        self.mission
            .set_reset_source_map_chains(self.world.reset_source_map_chains.clone());
    }

    pub fn step(&mut self, command: PlayerCommand) -> Vec<crate::StepEvidence> {
        self.step_with_speed_index(command, 0)
    }

    /// Execute one source outer turn with the exact 1/4/8 complete
    /// `UpdateEntities` sweep selected by remc2's speed index.
    pub fn step_with_speed_index(
        &mut self,
        command: PlayerCommand,
        speed_index: u8,
    ) -> Vec<crate::StepEvidence> {
        self.mission.finish_hud_draw(self.state.tick);
        if let (Some(clocks), Some(bank)) = (
            self.goat_tmap_clocks.as_mut(),
            self.world.goat_tmap_bank.as_ref(),
        ) {
            clocks.service_unpaused_outer_turn(bank);
        }
        self.mission.advance_exit_palette_frame();
        self.mission.sync_collected_spell_holders(&mut self.context);
        self.mission
            .sync_speed_holder_projection(&mut self.state, &mut self.context);
        let mut holders_before = self
            .context
            .spell_fire
            .as_ref()
            .map_or_else(Vec::new, |fire| fire.holders.clone());
        self.context.events.clear();
        let latched_command =
            latch_command_with_context(&mut self.state, command, &mut self.context);
        self.mission.set_external_collision_targets(
            self.live_world
                .goats()
                .iter()
                .map(goat_collision_target)
                .chain(
                    self.live_world
                        .villagers()
                        .iter()
                        .map(villager_collision_target),
                ),
        );
        {
            let (_, shading) = self.terrain.dynamic_lighting_planes();
            self.mission
                .subtract_dynamic_lighting_before_updates(shading);
        }
        let mut source_views =
            level1_turn_start_source_views(&self.mission, &self.state, &self.live_world);
        let mut evidence = Vec::new();
        for _ in 0..entity_passes_for_speed_index(speed_index) {
            self.mission.begin_source_turn();
            self.reclaim_imported_source_slots();
            if self.mission.take_group_7_villager_release() {
                let released = self.live_world.release_villager_stage_group_7();
                debug_assert_eq!(released, 28);
            }
            let current_player_target = [GoatPlayerTarget {
                entity_index: self.state.entity_index,
                position: self.state.position,
                control_flags: 0,
            }];
            let begin_goat_events = self.live_world.begin_source_turn(&current_player_target);
            self.process_goat_events(&begin_goat_events);
            source_views =
                level1_turn_start_source_views(&self.mission, &self.state, &self.live_world);
            let frozen_house_source_views = source_views.clone();
            for source_slot in 1..ENTITY_COUNT {
                let source_slot = u16::try_from(source_slot).expect("source cursor fits u16");
                if let Some((owner_entity_index, payload)) =
                    self.mission.take_building_possession_mailbox(source_slot)
                {
                    self.live_world
                        .submit_house_possession(source_slot, owner_entity_index, payload)
                        .expect("possession mailbox targets a live eligible house");
                }
                let house_state_before_dispatch = self
                    .live_world
                    .houses()
                    .iter()
                    .find(|house| house.entity_index == source_slot && house.active)
                    .is_some_and(|house| {
                        house.action == crate::house::HOUSE_REMOVE_ACTION
                            || (house.action == crate::house::HOUSE_LIVE_ACTION
                                && house.parameter.produces_villagers()
                                && ((house.pending_attacker != 0 && house.occupants > 2)
                                    || (house.phase & 0x1f == 0
                                        && house.capacity > 5
                                        && i32::from(house.capacity) == house.occupants)))
                    })
                    .then(|| self.live_world.clone());
                let house_events = self.live_world.step_house_source_slot_with_action_51_block(
                    source_slot,
                    &self.terrain,
                    self.mission
                        .current_house_action_51_blocked(&frozen_house_source_views, source_slot),
                );
                if !self.process_house_events(&house_events, &frozen_house_source_views) {
                    self.live_world =
                        house_state_before_dispatch.expect("failed house dispatch was staged");
                }
                let player_target = [GoatPlayerTarget {
                    entity_index: self.state.entity_index,
                    position: self.state.position,
                    control_flags: 0,
                }];
                let goat_events =
                    self.live_world
                        .step_source_slot(source_slot, &self.terrain, &player_target);
                let villager_effects = self.live_world.step_villager_source_slot(
                    source_slot,
                    &self.terrain,
                    &source_views,
                );
                self.mission
                    .apply_imported_villager_effects(&villager_effects);
                self.process_goat_events(&goat_events);
                if let Some(house) = self
                    .live_world
                    .houses()
                    .iter()
                    .find(|house| house.entity_index == source_slot)
                {
                    self.mission
                        .update_external_collision_target(house_collision_target(house));
                    self.mission.sync_imported_source_slot(
                        house.entity_index,
                        house.active,
                        house.action,
                        house.phase,
                        house.rng,
                        house.control_flags,
                    );
                } else if let Some(goat) = self
                    .live_world
                    .goats()
                    .iter()
                    .find(|goat| goat.entity_index == source_slot)
                {
                    self.mission
                        .update_external_collision_target(goat_collision_target(goat));
                    self.mission.sync_imported_source_slot(
                        goat.entity_index,
                        goat.active,
                        goat.action,
                        goat.phase,
                        goat.rng,
                        goat.control_flags,
                    );
                } else if let Some(villager) = self
                    .live_world
                    .villagers()
                    .iter()
                    .find(|villager| villager.entity_index == source_slot)
                {
                    self.mission
                        .update_external_collision_target(villager_collision_target(villager));
                    self.mission.sync_imported_source_slot(
                        villager.entity_index,
                        villager.active,
                        villager.action,
                        villager.phase,
                        villager.rng,
                        villager.control_flags,
                    );
                }
                refresh_level1_current_source_view(
                    &mut source_views,
                    &self.live_world,
                    source_slot,
                );
                if let Some(dolmen) =
                    self.world
                        .entities
                        .get(usize::from(source_slot))
                        .filter(|entity| {
                            entity.active
                                && entity.class == 2
                                && entity.model == 2
                                && entity.action == 6
                        })
                {
                    self.mission.step_dolmen_source_slot(
                        &self.state,
                        source_slot,
                        dolmen.collision_target(),
                    );
                }

                let (fireball_activated, possession_activated, castle_cast_yaw) = if source_slot
                    == self.state.entity_index
                {
                    self.mission
                        .prepare_player_action_before_flight(&mut self.state);
                    let pass_evidence = if self.mission.exit_sequence_owns_player_action() {
                        self.state.tick = self.state.tick.wrapping_add(1);
                        Vec::new()
                    } else {
                        match self.mission.player_action {
                            crate::level1_mission::PLAYER_ACTION_ACTIVE => {
                                vec![step_entity_pass_with_context(
                                    &mut self.state,
                                    latched_command,
                                    self.world.profile,
                                    &self.terrain,
                                    &mut self.context,
                                )]
                            }
                            crate::level1_mission::PLAYER_ACTION_DYING => {
                                vec![step_entity_body_with_context(
                                    &mut self.state,
                                    self.world.profile,
                                    &self.terrain,
                                    &mut self.context,
                                )]
                            }
                            _ => Vec::new(),
                        }
                    };
                    if !self.mission.exit_sequence_owns_player_action() {
                        let flight_proposed = pass_evidence
                            .last()
                            .map_or(self.state.position, |pass| pass.proposed);
                        self.mission
                            .advance_player_lifecycle_after_flight_with_proposed(
                                &self.terrain,
                                &mut self.state,
                                &mut self.context,
                                flight_proposed,
                            );
                    }
                    evidence.extend(pass_evidence);
                    let activated_model = |model| {
                        self.context.spell_fire.as_ref().is_some_and(|fire| {
                            fire.holders.iter().any(|holder| {
                                holder.model == model
                                    && holder.activation_value != 0
                                    && holder.active_value == holder.activation_value.cast_signed()
                                    && holders_before
                                        .iter()
                                        .find(|before| before.entity_index == holder.entity_index)
                                        .is_none_or(|before| {
                                            before.active_value != holder.active_value
                                        })
                            })
                        })
                    };
                    let fireball = activated_model(0);
                    let possession = activated_model(1);
                    let castle = activated_model(CASTLE_SPELL_FAMILY);
                    self.mission
                        .capture_spell_holder_active_values(&self.context);
                    holders_before = self
                        .context
                        .spell_fire
                        .as_ref()
                        .map_or_else(Vec::new, |fire| fire.holders.clone());
                    (fireball, possession, castle.then_some(self.state.yaw))
                } else {
                    (false, false, None)
                };
                let mission_event_start = self.mission.events().len();
                if !self.mission.exit_sequence_owns_player_action()
                    && self.mission.player_action == crate::level1_mission::PLAYER_ACTION_ACTIVE
                {
                    self.mission.dispatch_speed_holder_slot(
                        source_slot,
                        &mut self.state,
                        &mut self.context,
                    );
                }
                let source_effects = self.mission.dispatch_source_slot_with_views_and_look(
                    &self.terrain,
                    &self.state,
                    &source_views,
                    source_slot,
                    fireball_activated,
                    possession_activated,
                    castle_cast_yaw,
                    0,
                    0,
                );
                self.mission
                    .apply_pending_player_transients(&mut self.state);
                apply_level1_source_effects(
                    &mut self.mission,
                    &mut self.live_world,
                    &source_effects,
                )
                .expect("resident source effects apply after slot dispatch");
                if source_slot == self.state.entity_index && self.mission.exit_sequence_active {
                    self.mission.advance_exit_sequence(
                        &self.terrain,
                        &mut self.state,
                        &mut self.context,
                        &mut self.camera_fov,
                    );
                }
                self.apply_castle_terrain_updates();
                self.inject_new_fireball_impacts(mission_event_start);
                self.reclaim_imported_source_slots();
            }
            self.live_world.finish_source_turn();
        }
        {
            let (ground_heights, shading) = self.terrain.dynamic_lighting_planes();
            self.mission.service_dynamic_lighting_after_updates(
                &self.state,
                &source_views,
                ground_heights,
                shading,
            );
        }
        self.mission.finish_source_turn(&self.state);
        if let Some(fire) = self.context.spell_fire.as_mut() {
            // The class-15 handlers consume one active tick after their
            // same-turn projectile factories: model 0 in `sub_693F0` and
            // model 1 in `sub_69640`. Keep the direct Rust session in
            // lockstep with the C ABI production session so both spells can
            // return to their inactive, retriggerable state.
            for holder in &mut fire.holders {
                if matches!(holder.model, 0 | 1) && holder.active_value > 0 {
                    holder.active_value -= 1;
                }
            }
        }
        self.mission.sync_collected_spell_holders(&mut self.context);
        self.mission
            .sync_speed_holder_projection(&mut self.state, &mut self.context);
        evidence
    }

    fn apply_castle_terrain_updates(&mut self) {
        let updates = self
            .mission
            .drain_castle_terrain_updates()
            .collect::<Vec<_>>();
        if updates.is_empty() {
            return;
        }
        for update in updates {
            if update.is_ambient() {
                self.terrain
                    .apply_ambient_update(update)
                    .expect("qualified Level 1 ambient terrain update");
            } else {
                let footprint = self
                    .mission
                    .castle_stage_one_footprint()
                    .expect("Castle terrain updates require the validated BUILD[1] footprint");
                self.terrain
                    .apply_castle_update(update, footprint)
                    .expect("qualified Level 1 Castle terrain update");
            }
        }
    }

    fn process_goat_events(&mut self, events: &[GoatStepEvent]) {
        for event in events {
            match *event {
                GoatStepEvent::ManaConverted { entity_index, mana } => {
                    if let Some((position, yaw, rng)) = self
                        .live_world
                        .goats()
                        .iter()
                        .find(|goat| goat.entity_index == entity_index)
                        .map(|goat| (goat.position, goat.yaw, goat.rng))
                    {
                        self.mission.spawn_mana_sphere_from_death(
                            &self.terrain,
                            entity_index,
                            position,
                            yaw,
                            rng,
                            mana,
                            0,
                        );
                        self.mission.disable_imported_source_slot(entity_index);
                    }
                }
                GoatStepEvent::ExtraManaSphereRequested { entity_index } => {
                    if let Some(position) = self
                        .live_world
                        .goats()
                        .iter()
                        .find(|goat| goat.entity_index == entity_index)
                        .map(|goat| goat.position)
                    {
                        self.mission
                            .spawn_death_burst_from_death(entity_index, position);
                    }
                }
                GoatStepEvent::Reclaimed { .. } => {}
                _ => {}
            }
        }
    }

    fn process_house_events(
        &mut self,
        events: &[HouseStepEvent],
        frozen_source_views: &[Level1SourceEntityView],
    ) -> bool {
        let allocation_transaction = events.iter().any(|event| {
            matches!(
                event,
                HouseStepEvent::ResidentRequested { .. } | HouseStepEvent::RemoveFootprint { .. }
            )
        });
        let terrain_before_dispatch = allocation_transaction.then(|| self.terrain.clone());
        let mission_before_dispatch = allocation_transaction.then(|| self.mission.clone());
        let pending_effect_count = self.pending_house_effects.len();
        for event in events {
            match *event {
                HouseStepEvent::ConstructionPass {
                    entity_index,
                    source_id,
                    position,
                    building_index,
                    remaining,
                    first_pass,
                } => {
                    let footprint = self
                        .world
                        .house_build_footprint(building_index)
                        .expect("Level 1 action-51 house BUILD footprint");
                    if first_pass {
                        let effect_start = self.mission.house_construction_effects().len();
                        self.mission.apply_house_construction_first_pass(
                            frozen_source_views,
                            entity_index,
                            source_id,
                            position,
                            footprint,
                        );
                        let effects =
                            self.mission.house_construction_effects()[effect_start..].to_vec();
                        self.live_world
                            .apply_house_construction_source_effects(&effects);
                    }
                    self.terrain
                        .apply_house_construction(position, Some(remaining), footprint)
                        .expect("qualified Level 1 action-51 construction pass");
                    self.pending_house_effects.push(*event);
                }
                HouseStepEvent::ConstructionCompleted {
                    entity_index,
                    position,
                    building_index,
                    collision_half_width,
                    collision_half_depth,
                    ..
                } => {
                    let footprint = self
                        .world
                        .house_build_footprint(building_index)
                        .expect("Level 1 action-51 house BUILD footprint");
                    self.terrain
                        .apply_house_construction(position, None, footprint)
                        .expect("qualified Level 1 action-51 construction completion");
                    self.mission
                        .apply_house_construction_completion(
                            frozen_source_views,
                            entity_index,
                            position,
                            collision_half_width,
                            collision_half_depth,
                            |index| {
                                self.world
                                    .house_build_footprint(index)
                                    .map(|value| (value.width, value.height))
                            },
                        )
                        .expect("qualified Level 1 action-51 completion links");
                    self.pending_house_effects.push(*event);
                }
                HouseStepEvent::Possessed {
                    entity_index,
                    owner_entity_index,
                    payload,
                } => self.mission.apply_building_possession(
                    &self.state,
                    entity_index,
                    owner_entity_index,
                    payload,
                ),
                HouseStepEvent::RemoveFootprint {
                    entity_index,
                    attacker_entity_index,
                    position,
                    building_index,
                    occupants,
                    rng,
                    xtype,
                    ..
                } => {
                    let produces_residents = self
                        .live_world
                        .houses()
                        .iter()
                        .find(|house| house.entity_index == entity_index)
                        .is_some_and(|house| house.parameter.produces_villagers());
                    let footprint = self
                        .world
                        .house_build_footprint(building_index)
                        .cloned()
                        .expect("Level 1 action-53 house BUILD footprint");
                    let outcome = self
                        .terrain
                        .apply_house_removal(
                            position,
                            rng,
                            occupants,
                            produces_residents,
                            &footprint,
                        )
                        .expect("qualified Level 1 house terrain removal");
                    self.live_world
                        .commit_house_removal_state(
                            entity_index,
                            outcome.final_rng,
                            outcome.final_occupants,
                        )
                        .expect("commit action-53 house state");
                    let requests = outcome
                        .residents
                        .iter()
                        .map(|resident| HouseResidentRequest {
                            position: resident.position,
                            model: resident.model,
                            action: resident.action,
                            attacker_entity_index,
                        })
                        .collect::<Vec<_>>();
                    let Some(_order) = self
                        .mission
                        .commit_imported_house_demolition(entity_index, xtype, &requests)
                        .expect("qualified action-53 resident/xtype transaction")
                    else {
                        self.terrain = terrain_before_dispatch
                            .expect("action-53 terrain transaction was staged");
                        self.mission = mission_before_dispatch
                            .expect("action-53 mission transaction was staged");
                        self.pending_house_effects.truncate(pending_effect_count);
                        return false;
                    };
                    for resident in outcome.residents {
                        self.pending_house_effects
                            .push(HouseStepEvent::ResidentRequested {
                                entity_index,
                                attacker_entity_index,
                                position: resident.position,
                                model: resident.model,
                                action: resident.action,
                            });
                    }
                    self.pending_house_effects.push(*event);
                }
                HouseStepEvent::ResidentRequested {
                    attacker_entity_index,
                    position,
                    model,
                    action,
                    ..
                } => {
                    let request = HouseResidentRequest {
                        position,
                        model,
                        action,
                        attacker_entity_index,
                    };
                    let Some(_) = self
                        .mission
                        .spawn_house_residents_atomic(&[request])
                        .expect("qualified action-52 resident transaction")
                    else {
                        self.terrain = terrain_before_dispatch
                            .expect("action-52 terrain transaction was staged");
                        self.mission = mission_before_dispatch
                            .expect("action-52 mission transaction was staged");
                        self.pending_house_effects.truncate(pending_effect_count);
                        return false;
                    };
                    self.pending_house_effects.push(*event);
                }
                HouseStepEvent::Damaged { .. } | HouseStepEvent::DestructionStarted { .. } => {
                    self.pending_house_effects.push(*event);
                }
            }
        }
        true
    }

    fn reclaim_imported_source_slots(&mut self) {
        for source_slot in self.mission.take_reclaimed_imported_source_slots() {
            let _ = self.live_world.reclaim_source_slot(source_slot);
        }
    }

    fn inject_new_fireball_impacts(&mut self, event_start: usize) {
        let impacts = self.mission.events()[event_start..]
            .iter()
            .filter_map(|event| {
                let crate::level1_mission::Level1MissionEvent::FireballImpacted {
                    impact_id,
                    position,
                    ..
                } = event
                else {
                    return None;
                };
                let payload = self
                    .mission
                    .impacts()
                    .iter()
                    .find(|impact| impact.id == *impact_id)
                    .map(|impact| {
                        (
                            impact.payload,
                            impact.owner_entity_index,
                            impact.external_target_entity_index,
                        )
                    })?;
                Some((*position, payload.0, payload.1, payload.2))
            })
            .collect::<Vec<_>>();
        for (position, payload, owner_entity_index, direct_target_entity_index) in impacts {
            let targets = self
                .live_world
                .goats()
                .iter()
                .filter(|goat| {
                    goat.active
                        && goat.entity_index != owner_entity_index
                        && (goat.entity_index == direct_target_entity_index
                            || crate::level1_mission::source_entities_overlap(
                                position,
                                crate::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                                crate::level1_mission::FIREBALL_IMPACT_COLLISION_VERTICAL_OFFSET,
                                crate::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_HEIGHT,
                                goat.position,
                                goat.collision_half_width,
                                goat.collision_vertical_offset,
                                goat.collision_half_height,
                            ))
                })
                .map(|goat| goat.entity_index)
                .chain(
                    self.live_world
                        .villagers()
                        .iter()
                        .filter(|villager| {
                            villager.active
                                && villager.entity_index != owner_entity_index
                                && (villager.entity_index == direct_target_entity_index
                                    || crate::level1_mission::source_entities_overlap(
                                        position,
                                        crate::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                                        crate::level1_mission::FIREBALL_IMPACT_COLLISION_VERTICAL_OFFSET,
                                        crate::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_HEIGHT,
                                        villager.position,
                                        villager.collision_half_width,
                                        villager.collision_vertical_offset,
                                        villager.collision_half_height,
                                    ))
                        })
                        .map(|villager| villager.entity_index),
                )
                .chain(
                    self.live_world
                        .houses()
                        .iter()
                        .filter(|house| {
                            house.active
                                && house.entity_index != owner_entity_index
                                && (house.entity_index == direct_target_entity_index
                                    || crate::level1_mission::source_entities_overlap_xy(
                                        position,
                                        crate::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                                        crate::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                                        house.position,
                                        house.collision_half_width,
                                        house.collision_half_depth,
                                    ))
                        })
                        .map(|house| house.entity_index),
                )
                .collect::<Vec<_>>();
            for target in targets {
                let _ = self.live_world.inject_damage(
                    target,
                    payload,
                    owner_entity_index.cast_signed(),
                );
            }
        }
    }
}

fn validate_level_one_mission_stages(runtime: &RuntimeSnapshot) -> Result<()> {
    let expected = [
        (5, 29_440_u16, 54_272_u16, 0_u32),
        (5, 49_664_u16, 54_528_u16, 0_u32),
        (7, 0, 0, 4),
        (0, 0, 0, 15),
        (7, 0, 0, 19),
    ];
    if runtime.stage_index != 5 {
        return Err(NativeWorldError::new(format!(
            "Level 1 must expose five mission stages, got {}",
            runtime.stage_index
        )));
    }
    for (index, (kind, x, y, scalar)) in expected.into_iter().enumerate() {
        let stage = &runtime.stages[index];
        if stage.kind != kind
            || stage.axis_x.cast_unsigned() != x
            || stage.axis_y.cast_unsigned() != y
            || stage.relation != RuntimeRelation::Scalar(scalar)
        {
            return Err(NativeWorldError::new(format!(
                "Level 1 mission stage {index} mismatch: {stage:?}"
            )));
        }
    }
    Ok(())
}

fn validate_level_one_mission_presentation(presentation: &PresentationSelectors) -> Result<()> {
    let expected = [
        (
            ARCHER_RENDER_INDEX,
            ARCHER_BASE_TMAP,
            ARCHER_RENDER_MODE,
            ARCHER_LOGICAL_HEIGHT,
            133_u16,
            1_i8,
        ),
        (
            FIREFLY_RENDER_INDEX,
            FIREFLY_BASE_TMAP,
            FIREFLY_RENDER_MODE,
            FIREFLY_LOGICAL_HEIGHT,
            210,
            1,
        ),
        (
            FIREBALL_RENDER_INDEX,
            FIREBALL_BASE_TMAP,
            FIREBALL_RENDER_MODE,
            FIREBALL_LOGICAL_HEIGHT,
            97,
            1,
        ),
        (
            FIREBALL_IMPACT_RENDER_INDEX,
            FIREBALL_IMPACT_BASE_TMAP,
            FIREBALL_IMPACT_RENDER_MODE,
            FIREBALL_IMPACT_LOGICAL_HEIGHT,
            256,
            -1,
        ),
        (
            POSSESSION_IMPACT_RENDER_INDEX,
            POSSESSION_IMPACT_BASE_TMAP,
            POSSESSION_IMPACT_RENDER_MODE,
            POSSESSION_IMPACT_LOGICAL_HEIGHT,
            264,
            -1,
        ),
        (
            POSSESSION_ATTRACTION_RENDER_INDEX,
            POSSESSION_ATTRACTION_BASE_TMAP,
            POSSESSION_ATTRACTION_RENDER_MODE,
            POSSESSION_ATTRACTION_LOGICAL_HEIGHT,
            133,
            1,
        ),
        (
            SPELL_JAR_RENDER_INDEX,
            SPELL_JAR_BASE_TMAP,
            SPELL_JAR_RENDER_MODE,
            SPELL_JAR_LOGICAL_HEIGHT,
            184,
            -1,
        ),
        (
            MANA_CARRIER_RENDER_INDEX,
            MANA_CARRIER_BASE_TMAP,
            MANA_CARRIER_RENDER_MODE,
            MANA_CARRIER_LOGICAL_HEIGHT,
            545,
            -16,
        ),
    ];
    for (index, base_tmap, render_mode, logical_height, speed, byte_11) in expected {
        let parameter = presentation
            .particles
            .get(usize::from(index))
            .ok_or_else(|| {
                NativeWorldError::new(format!("Level 1 render parameter {index} is absent"))
            })?;
        if parameter.base_tmap != base_tmap
            || parameter.word_2 != 8_492
            || parameter.word_4 != 42
            || parameter.speed != speed
            || parameter.rotation_speed != logical_height
            || parameter.byte_10 != 0
            || parameter.byte_11 != byte_11
            || parameter.render_mode != render_mode
            || parameter.byte_13 != 0
        {
            return Err(NativeWorldError::new(format!(
                "Level 1 render parameter {index} mismatch: {parameter:?}"
            )));
        }
    }
    for (phase, ((logical_height, speed), byte_11)) in GROUND_FIRE_LOGICAL_HEIGHTS
        .into_iter()
        .zip(GROUND_FIRE_PRESENTATION_SPEEDS)
        .zip(GROUND_FIRE_PRESENTATION_BYTE_11)
        .enumerate()
    {
        let index = GROUND_FIRE_FIRST_RENDER_INDEX
            + u16::try_from(phase).expect("eight ground-fire phases fit u16");
        let parameter = presentation
            .particles
            .get(usize::from(index))
            .ok_or_else(|| {
                NativeWorldError::new(format!("Level 1 render parameter {index} is absent"))
            })?;
        if parameter.base_tmap != GROUND_FIRE_BASE_TMAP
            || parameter.word_2 != 8_492
            || parameter.word_4 != 42
            || parameter.speed != speed
            || parameter.rotation_speed != logical_height
            || parameter.byte_10 != 0
            || parameter.byte_11 != byte_11
            || parameter.render_mode != GROUND_FIRE_RENDER_MODE
            || parameter.byte_13 != 0
        {
            return Err(NativeWorldError::new(format!(
                "Level 1 render parameter {index} mismatch: {parameter:?}"
            )));
        }
    }
    Ok(())
}

fn level_one_global_mana_denominator(entities: &[EntitySnapshot]) -> Result<i32> {
    let mut denominator = 1_i32;
    for entity in entities.iter().skip(1).filter(|entity| entity.active) {
        let contributes = entity.class == 5
            || entity.class == 10 && matches!(entity.model, 39 | 45 | 58)
            || entity.class == 3 && matches!(entity.model, 2 | 3);
        if contributes {
            denominator = denominator
                .checked_add(entity.mana)
                .ok_or_else(|| NativeWorldError::new("Level 1 global mana denominator overflow"))?;
        }
    }
    if denominator <= 0 {
        return Err(NativeWorldError::new(
            "Level 1 global mana denominator is not positive",
        ));
    }
    Ok(denominator)
}

fn decode_level_one_goat_stage_point(runtime: &RuntimeSnapshot) -> Result<Position> {
    let stage = runtime
        .stage_variables
        .get(3)
        .ok_or_else(|| NativeWorldError::new("Level 1 Goat stage slot 3 is absent"))?;
    let RuntimeRelation::AxisScalar(axis) = stage.relation else {
        return Err(NativeWorldError::new(
            "Level 1 Goat stage slot 3 is not an axis payload",
        ));
    };
    let point = Position {
        x: axis as u16,
        y: (axis >> 16) as u16,
        z: 0,
    };
    if stage.kind != 2 || stage.flags & 1 == 0 || point != GOAT_STAGE_POINT {
        return Err(NativeWorldError::new(format!(
            "Level 1 Goat stage slot 3 mismatch: kind {}, flags {}, point ({}, {})",
            stage.kind, stage.flags, point.x, point.y
        )));
    }
    Ok(point)
}

fn parse_metadata(manifest: &serde_json::Map<String, Value>) -> Result<LevelMetadata> {
    let level = require_object(manifest, "level", "manifest")?;
    let map_type = require_object(level, "map_type", "manifest.level")?;
    let capture = require_object(manifest, "capture", "manifest")?;
    let dimensions = require_object(manifest, "dimensions", "manifest")?;
    let terrain_dimensions = require_array(dimensions, "terrain", "manifest.dimensions")?;
    if terrain_dimensions.as_slice() != [json!(256), json!(256)] {
        return Err(NativeWorldError::new(
            "manifest terrain dimensions must be 256x256",
        ));
    }
    Ok(LevelMetadata {
        external_level: as_u16(
            require_value(level, "external", "manifest.level")?,
            "manifest.level.external",
        )?,
        internal_level: as_u16(
            require_value(level, "internal", "manifest.level")?,
            "manifest.level.internal",
        )?,
        map_type: MapType::decode(
            as_u8(
                require_value(map_type, "id", "manifest.level.map_type")?,
                "manifest.level.map_type.id",
            )?,
            require_string(map_type, "name", "manifest.level.map_type")?,
        )?,
        capture_site: require_string(capture, "site", "manifest.capture")?.to_owned(),
        capture_frame: as_u32(
            require_value(capture, "frame", "manifest.capture")?,
            "manifest.capture.frame",
        )?,
        vga_resolution_selector: as_u8(
            require_value(
                require_object(manifest, "presentation_dependencies", "manifest")?,
                "vga_resolution_selector",
                "manifest.presentation_dependencies",
            )?,
            "manifest.presentation_dependencies.vga_resolution_selector",
        )?,
    })
}

fn validate_manifest_counts(
    counts: &serde_json::Map<String, Value>,
    profile: OwnerPackageProfile,
) -> Result<()> {
    let expected_active = match profile {
        OwnerPackageProfile::Level1 => 177,
        OwnerPackageProfile::Level2 => 114,
    };
    for (name, expected) in [
        ("authored_features", 1_200_u64),
        ("entity_slots", 1_000),
        ("active_entities", expected_active),
        ("players", 8),
        ("particle_parameters", 347),
        ("level_sprite_selectors", 504),
        ("entity_parameter_records", 157),
    ] {
        let actual = require_value(counts, name, "manifest.counts")?
            .as_u64()
            .ok_or_else(|| NativeWorldError::new(format!("manifest count {name} is not u64")))?;
        if actual != expected {
            return Err(NativeWorldError::new(format!(
                "manifest count {name} must be {expected}, got {actual}"
            )));
        }
    }
    Ok(())
}

fn validate_profile(metadata: &LevelMetadata, profile: OwnerPackageProfile) -> Result<()> {
    let expected = match profile {
        OwnerPackageProfile::Level1 => (1, 0, MapType::Night),
        OwnerPackageProfile::Level2 => (2, 1, MapType::Day),
    };
    if (
        metadata.external_level,
        metadata.internal_level,
        metadata.map_type,
    ) != (expected.0, expected.1, expected.2)
    {
        return Err(NativeWorldError::new(format!(
            "owner package metadata does not match {:?} profile: external {}, internal {}, map {:?}",
            profile, metadata.external_level, metadata.internal_level, metadata.map_type
        )));
    }
    Ok(())
}

fn validate_level_two_profile(
    manifest: &serde_json::Map<String, Value>,
    runtime: &RuntimeSnapshot,
    players: &[PlayerSnapshot],
    entities: &[EntitySnapshot],
) -> Result<()> {
    if runtime.local_player_slot != 0 || runtime.player_count != 2 || runtime.maptype_music != 2 {
        return Err(NativeWorldError::new(
            "Level 2 runtime must select local player 0, two offline players, and Day music",
        ));
    }
    if players.get(0).is_none_or(|player| player.is_ai)
        || players.get(1).is_none_or(|player| !player.is_ai)
    {
        return Err(NativeWorldError::new(
            "Level 2 player ordering must be local player 0 followed by AI player 1",
        ));
    }
    for (slot, player) in players.iter().take(2).enumerate() {
        if entities
            .get(usize::from(player.entity_index))
            .is_none_or(|entity| !entity.active)
        {
            return Err(NativeWorldError::new(format!(
                "Level 2 player {slot} must reference an active entity"
            )));
        }
    }
    let dependencies = require_object(manifest, "presentation_dependencies", "manifest")?;
    if require_string(dependencies, "map_type", "presentation_dependencies")? != "Day"
        || as_u8(
            require_value(
                dependencies,
                "vga_resolution_selector",
                "presentation_dependencies",
            )?,
            "presentation_dependencies.vga_resolution_selector",
        )? != 1
        || as_u8(
            require_value(dependencies, "texture_size", "presentation_dependencies")?,
            "presentation_dependencies.texture_size",
        )? != 32
        || as_u8(
            require_value(
                dependencies,
                "terrain_graphics_flags",
                "presentation_dependencies",
            )?,
            "presentation_dependencies.terrain_graphics_flags",
        )? != 0
        || require_value(
            dependencies,
            "terrain_graphics_flag_2",
            "presentation_dependencies",
        )? != &Value::Bool(false)
        || require_string(dependencies, "terrain_bank", "presentation_dependencies")? != "TMAPS0-0"
        || require_string(dependencies, "sprite_bank", "presentation_dependencies")? != "MSPRD0-0"
        || require_string(dependencies, "web_bank", "presentation_dependencies")? != "MWEBD0-0"
        || require_string(dependencies, "block_bank", "presentation_dependencies")? != "BL32N0-0"
        || require_string(dependencies, "palette", "presentation_dependencies")? != "PALD-0"
        || require_string(dependencies, "sky_bank", "presentation_dependencies")? != "SKYD0-0"
        || as_u8(
            require_value(dependencies, "sound_bank", "presentation_dependencies")?,
            "presentation_dependencies.sound_bank",
        )? != 1
        || as_u8(
            require_value(dependencies, "cursor", "presentation_dependencies")?,
            "presentation_dependencies.cursor",
        )? != 9
    {
        return Err(NativeWorldError::new(
            "Level 2 Day presentation dependencies are not the qualified owner set",
        ));
    }
    let resolved = require_object(dependencies, "resolved_files", "presentation_dependencies")?;
    for (name, expected) in [
        ("level_records", "owner:GAME/NETHERW/CLEVELS/LEVELS.DAT"),
        ("terrain_bank", "owner:GAME/NETHERW/CDATA/TMAPS0-0.DAT"),
        ("sprite_bank", "owner:CD_Files/DATA/MSPRD0-0.DAT"),
        ("web_bank", "owner:CD_Files/DATA/MWEBD0-0.DAT"),
        ("block_bank", "owner:CD_Files/DATA/BL32N0-0.DAT"),
        ("palette", "owner:CD_Files/DATA/PALD-0.DAT"),
        ("sky_bank", "owner:CD_Files/DATA/SKYD0-0.DAT"),
        ("building_bank", "owner:CD_Files/DATA/BUILD0-0.DAT"),
        ("spell_table", "owner:CD_Files/DATA/SPELLS.DAT"),
        ("font_bank", "owner:CD_Files/DATA/FONT1.DAT"),
        ("language", "owner:CD_Files/LANGUAGE/L1.TXT"),
    ] {
        if require_string(resolved, name, "presentation_dependencies.resolved_files")? != expected {
            return Err(NativeWorldError::new(format!(
                "Level 2 presentation dependency {name} changed"
            )));
        }
    }
    let source = require_object(manifest, "source", "manifest")?;
    if require_string(source, "upstream_commit", "manifest.source")? != LEVEL2_UPSTREAM_COMMIT {
        return Err(NativeWorldError::new(
            "Level 2 source pin does not match the remc2 specification",
        ));
    }
    let normalization = require_object(manifest, "normalization", "manifest")?;
    if require_string(normalization, "integer_encoding", "manifest.normalization")?
        != "explicit fixed-width little-endian semantic fields"
        || require_string(normalization, "native_pointers", "manifest.normalization")?
            != "stable tagged relations; live/authored pools are one-based and zero is null"
    {
        return Err(NativeWorldError::new(
            "Level 2 normalization policy is not fixed-width little-endian",
        ));
    }
    let volatile = require_array(
        normalization,
        "volatile_host_state",
        "manifest.normalization",
    )?;
    if volatile.as_slice()
        != [json!(
            "player time_393 host-clock sample zeroed; secondary terrain plane preserved"
        )]
    {
        return Err(NativeWorldError::new(
            "Level 2 normalization volatile-host policy changed",
        ));
    }
    Ok(())
}

fn validate_level_two_provenance_members(members: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    for (name, magic) in [
        ("feature-start.bin", *b"MFS3"),
        ("feature-end.bin", *b"MFE3"),
        ("feature-allocator.bin", *b"MCA3"),
        ("feature-construction-inputs.bin", *b"MCI3"),
    ] {
        let bytes = member(members, name)?;
        if bytes.get(..4) != Some(magic.as_slice()) || read_u16(bytes, 4, name)? != 1 {
            return Err(NativeWorldError::new(format!(
                "Level 2 {name} header/version mismatch"
            )));
        }
    }
    validate_header(
        member(members, "entities-post-features.bin")?,
        *b"MCE3",
        1,
        ENTITY_RECORD_BYTES_U16,
        ENTITY_COUNT_U32,
        "entities-post-features.bin",
    )?;
    if read_u16(
        member(members, "feature-start.bin")?,
        6,
        "feature-start record size",
    )? != 16
        || read_u16(
            member(members, "feature-end.bin")?,
            6,
            "feature-end record size",
        )? != 16
    {
        return Err(NativeWorldError::new(
            "Level 2 feature boundary record size changed",
        ));
    }
    let allocator = member(members, "feature-allocator.bin")?;
    if read_u16(allocator, 6, "feature allocator header bytes")? != 32 || allocator.len() < 32 {
        return Err(NativeWorldError::new(
            "Level 2 feature allocator layout changed",
        ));
    }
    let inputs = member(members, "feature-construction-inputs.bin")?;
    if read_u16(inputs, 6, "feature construction record size")? != 16
        || read_u16(inputs, 8, "feature construction record count")? != 9
    {
        return Err(NativeWorldError::new(
            "Level 2 feature construction input layout changed",
        ));
    }
    Ok(())
}

fn validate_manifest_layouts(manifest: &serde_json::Map<String, Value>) -> Result<()> {
    let sizes = require_object(manifest, "record_sizes", "manifest")?;
    require_exact_u64(sizes, "entity", ENTITY_RECORD_BYTES as u64, "record_sizes")?;
    require_exact_u64(sizes, "player", PLAYER_RECORD_BYTES as u64, "record_sizes")?;
    let layouts = require_object(manifest, "record_layouts", "manifest")?;
    let entity = require_object(layouts, "entity", "record_layouts")?;
    for (name, expected) in [
        ("index_offset", 0),
        ("active_offset", 2),
        ("next_relation_offset", 4),
        ("parameter_relation_offset", 164),
        ("special_domain_offset", 168),
        ("special_relation_offset", 172),
    ] {
        require_exact_u64(entity, name, expected, "record_layouts.entity")?;
    }
    let player = require_object(layouts, "player", "record_layouts")?;
    for (name, expected) in [
        ("index_offset", 0),
        ("normalized_host_time_offset", 50),
        ("legacy_v1_bytes", 565),
        ("ambient_loop_5_distance_offset", 565),
        ("ambient_loop_31_distance_offset", 569),
        ("record_bytes", 573),
    ] {
        require_exact_u64(player, name, expected, "record_layouts.player")?;
    }
    let runtime = require_object(layouts, "runtime", "record_layouts")?;
    require_exact_u64(
        runtime,
        "scalar_prefix_bytes",
        192,
        "record_layouts.runtime",
    )?;
    require_exact_u64(
        runtime,
        "bytes",
        RUNTIME_BYTES as u64,
        "record_layouts.runtime",
    )?;
    for (name, offset, count, bytes) in [
        ("stages", 192, 8, 14),
        ("objectives", 304, 8, 11),
        ("stage_variables", 392, 11, 12),
        ("events", 524, 50, 43),
        ("player_inputs", 2_674, 8, 10),
    ] {
        let section = require_object(runtime, name, "record_layouts.runtime")?;
        require_exact_u64(section, "offset", offset, name)?;
        require_exact_u64(section, "count", count, name)?;
        require_exact_u64(section, "record_bytes", bytes, name)?;
    }
    Ok(())
}

fn parse_common_package(
    package_dir: &Path,
    manifest: &serde_json::Map<String, Value>,
    profile: OwnerPackageProfile,
) -> Result<CommonPackage> {
    let schema = require_string(manifest, "schema", "manifest")?;
    let expected_schema = match profile {
        OwnerPackageProfile::Level1 => MANIFEST_SCHEMA,
        OwnerPackageProfile::Level2 => "mc2-owner-level-export/v3",
    };
    if schema != expected_schema {
        return Err(NativeWorldError::new(format!(
            "manifest schema must be {expected_schema} for {profile:?}"
        )));
    }
    if require_string(manifest, "byte_order", "manifest")? != "little-endian" {
        return Err(NativeWorldError::new(
            "manifest byte_order must be little-endian",
        ));
    }

    let package = validate_package(package_dir, manifest)?;
    let authored = member(&package.members, "authored-level.bin")?.to_vec();
    if profile == OwnerPackageProfile::Level2 {
        validate_level_two_provenance_members(&package.members)?;
    }
    let metadata = parse_metadata(manifest)?;
    validate_profile(&metadata, profile)?;
    let counts = require_object(manifest, "counts", "manifest")?;
    validate_manifest_counts(counts, profile)?;
    validate_manifest_layouts(manifest)?;

    if authored.get(6).copied() != Some(metadata.map_type.id()) {
        return Err(NativeWorldError::new(
            "authored-level.bin map type does not match manifest",
        ));
    }

    let terrain_planes = parse_terrain(member(&package.members, "terrain.bin")?)?;
    let terrain_rng = match profile {
        OwnerPackageProfile::Level1 => 39_663,
        OwnerPackageProfile::Level2 => read_u16(
            member(&package.members, "feature-end.bin")?,
            8,
            "Level 2 feature-end terrain RNG",
        )?,
    };
    let terrain = LegacyTerrain::new(
        terrain_planes.terrain_types.to_vec(),
        terrain_planes.ground_heights.to_vec(),
        (metadata.map_type == MapType::Cave).then(|| terrain_planes.secondary_heights.to_vec()),
        terrain_planes.angles.to_vec(),
    )
    .map_err(NativeWorldError::new)?;
    let entities = parse_entities(member(&package.members, "entities.bin")?)?;
    let players = parse_players(member(&package.members, "players.bin")?, profile)?;
    let runtime = parse_runtime(member(&package.members, "runtime.bin")?)?;
    let presentation = parse_presentation(member(&package.members, "presentation.bin")?)?;
    if profile == OwnerPackageProfile::Level1 {
        validate_level_one_mission_stages(&runtime)?;
        validate_level_one_mission_presentation(&presentation)?;
    } else {
        validate_level_two_profile(manifest, &runtime, &players, &entities)?;
    }
    validate_manifest_relations(counts, &entities, &players, &runtime, &presentation)?;
    let reset_state = match (profile, manifest.get("reset_projection")) {
        // Level 2 captures may carry a copied Level 1 projection for package
        // provenance. Validate its shape when present, but always construct
        // the actual Level 2 reset from the profile's own player/entity rows.
        (OwnerPackageProfile::Level2, Some(reset)) => {
            let reset = as_object(reset, "reset_projection")?;
            let _ = parse_reset_state(
                reset,
                member(&package.members, "entities.bin")?,
                member(&package.members, "players.bin")?,
                member(&package.members, "runtime.bin")?,
            )?;
            Some(parse_level_two_reset_state(
                member(&package.members, "entities.bin")?,
                member(&package.members, "players.bin")?,
                member(&package.members, "runtime.bin")?,
            )?)
        }
        (OwnerPackageProfile::Level1, Some(reset)) => {
            let reset = as_object(reset, "reset_projection")?;
            let state = parse_reset_state(
                reset,
                member(&package.members, "entities.bin")?,
                member(&package.members, "players.bin")?,
                member(&package.members, "runtime.bin")?,
            )?;
            Some(state)
        }
        (OwnerPackageProfile::Level2, None) => Some(parse_level_two_reset_state(
            member(&package.members, "entities.bin")?,
            member(&package.members, "players.bin")?,
            member(&package.members, "runtime.bin")?,
        )?),
        (OwnerPackageProfile::Level1, None) => {
            return Err(NativeWorldError::new(
                "manifest is missing reset_projection",
            ));
        }
    };
    Ok(CommonPackage {
        package,
        metadata,
        terrain_planes,
        terrain,
        entities,
        players,
        runtime,
        presentation,
        terrain_rng,
        reset_state,
        authored,
    })
}

struct ValidatedPackage {
    schema: OwnerPackageSchema,
    package_sha256: String,
    members: BTreeMap<String, Vec<u8>>,
}

fn validate_package(
    package_dir: &Path,
    manifest: &serde_json::Map<String, Value>,
) -> Result<ValidatedPackage> {
    let schema = match require_string(manifest, "schema", "manifest")? {
        MANIFEST_SCHEMA => OwnerPackageSchema::V2,
        "mc2-owner-level-export/v3" => OwnerPackageSchema::V3,
        other => {
            return Err(NativeWorldError::new(format!(
                "unsupported owner package schema {other}"
            )));
        }
    };
    let files = require_object(manifest, "files", "manifest")?;
    let expected_members: &[(&str, usize)] = match schema {
        OwnerPackageSchema::V2 => &PACKAGE_MEMBERS,
        OwnerPackageSchema::V3 => &LEVEL2_PACKAGE_MEMBERS,
    };
    let mut allowed_paths = expected_members
        .iter()
        .map(|(name, _)| *name)
        .collect::<BTreeSet<_>>();
    allowed_paths.insert("manifest.json");
    if files.len() != expected_members.len() {
        return Err(NativeWorldError::new(
            "manifest files map does not contain the exact typed package member set",
        ));
    }
    for entry in fs::read_dir(package_dir).map_err(|error| {
        NativeWorldError::new(format!(
            "cannot enumerate package {}: {error}",
            package_dir.display()
        ))
    })? {
        let entry = entry.map_err(|error| {
            NativeWorldError::new(format!("cannot inspect package member: {error}"))
        })?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| NativeWorldError::new("package contains a non-UTF-8 member name"))?;
        if !allowed_paths.contains(name)
            || !entry
                .file_type()
                .map_err(|error| {
                    NativeWorldError::new(format!("cannot inspect package member {name}: {error}"))
                })?
                .is_file()
        {
            return Err(NativeWorldError::new(format!(
                "package contains an unlisted or non-file member {name}"
            )));
        }
    }
    let mut members = BTreeMap::new();
    let mut identity = Vec::new();
    for (name, expected_size) in expected_members {
        let entry = require_object(files, name, "manifest.files")?;
        require_exact_u64(entry, "bytes", *expected_size as u64, name)?;
        let expected_hash = require_hash_string(entry, "sha256", name)?;
        let bytes = read_file(&package_dir.join(name))?;
        if bytes.len() != *expected_size {
            return Err(NativeWorldError::new(format!(
                "{name} has {} bytes, expected {expected_size}",
                bytes.len()
            )));
        }
        let actual_hash = sha256_hex(&bytes);
        if actual_hash != expected_hash {
            return Err(NativeWorldError::new(format!("{name} hash mismatch")));
        }
        identity.extend_from_slice(name.as_bytes());
        identity.push(0);
        identity.extend_from_slice(actual_hash.as_bytes());
        identity.push(0);
        identity.extend_from_slice(expected_size.to_string().as_bytes());
        identity.push(b'\n');
        members.insert((*name).to_owned(), bytes);
    }
    let actual_package_hash = sha256_hex(&identity);
    let expected_package_hash = require_hash_string(manifest, "package_sha256", "manifest")?;
    if actual_package_hash != expected_package_hash {
        return Err(NativeWorldError::new("package_sha256 mismatch"));
    }
    Ok(ValidatedPackage {
        schema,
        package_sha256: expected_package_hash.to_owned(),
        members,
    })
}

fn parse_terrain(bytes: &[u8]) -> Result<TerrainPlanes> {
    if bytes.len() != TERRAIN_BYTES {
        return Err(NativeWorldError::new("terrain.bin has an invalid extent"));
    }
    let plane = |index: usize| {
        bytes[index * LEGACY_MAP_CELLS..(index + 1) * LEGACY_MAP_CELLS]
            .to_vec()
            .into_boxed_slice()
    };
    let entity_bytes = &bytes[5 * LEGACY_MAP_CELLS..];
    let mut entity_indices = Vec::with_capacity(LEGACY_MAP_CELLS);
    for pair in entity_bytes.chunks_exact(2) {
        entity_indices.push(i16::from_le_bytes([pair[0], pair[1]]));
    }
    Ok(TerrainPlanes {
        terrain_types: plane(0),
        ground_heights: plane(1),
        shading: plane(2),
        angles: plane(3),
        secondary_heights: plane(4),
        entity_indices: entity_indices.into_boxed_slice(),
    })
}

fn parse_entities(bytes: &[u8]) -> Result<Vec<EntitySnapshot>> {
    validate_header(
        bytes,
        *b"MCE1",
        1,
        ENTITY_RECORD_BYTES_U16,
        ENTITY_COUNT_U32,
        "entities.bin",
    )?;
    let mut entities = Vec::with_capacity(ENTITY_COUNT);
    for index in 0..ENTITY_COUNT {
        let record = record(bytes, 12, ENTITY_RECORD_BYTES, index, "entities.bin")?;
        let stored_index = read_u16(record, 0, "entity index")?;
        if usize::from(stored_index) != index {
            return Err(NativeWorldError::new(format!(
                "entity slot/index mismatch at {index}: {stored_index}"
            )));
        }
        let active_byte = read_u8(record, 2, "entity active")?;
        if active_byte > 1 {
            return Err(NativeWorldError::new(format!(
                "entity {index} has invalid active byte {active_byte}"
            )));
        }
        let next = relation_one_based(
            read_u32(record, 4, "entity next relation")?,
            ENTITY_COUNT,
            "entity next relation",
        )?;
        let parameter_relation = relation_one_based(
            read_u32(record, 164, "entity parameter relation")?,
            ENTITY_PARAMETER_COUNT,
            "entity parameter relation",
        )?;
        let particle_parameter_raw = read_i16(record, 94, "entity particle parameter index")?;
        let particle_parameter_index =
            (particle_parameter_raw >= 0).then(|| particle_parameter_raw.cast_unsigned());
        let domain = read_u8(record, 168, "entity special domain")?;
        if record[169..172] != [0, 0, 0] {
            return Err(NativeWorldError::new(format!(
                "entity {index} has non-zero special relation padding"
            )));
        }
        let special_value = read_u32(record, 172, "entity special relation")?;
        let special_relation = match (domain, special_value) {
            (0, 0) => EntitySpecialRelation::None,
            (1, 1..=8) => EntitySpecialRelation::PlayerState(
                u8::try_from(special_value - 1).expect("bounded player slot"),
            ),
            (2, 1) => EntitySpecialRelation::GlobalSpecialState,
            _ => {
                return Err(NativeWorldError::new(format!(
                    "entity {index} has invalid special relation {domain}/{special_value}"
                )));
            }
        };
        let active = active_byte == 1;
        entities.push(EntitySnapshot {
            index: stored_index,
            source_id: read_u16(record, 30, "entity source id")?,
            authority: if active {
                EntityAuthority::FrozenObserved
            } else {
                EntityAuthority::Inactive
            },
            active,
            max_life: read_i32(record, 8, "entity max life")?,
            life: read_i32(record, 12, "entity life")?,
            next_relation: next,
            class: read_u8(record, 67, "entity class")?,
            model: read_u8(record, 68, "entity model")?,
            control_flags: read_u32(record, 16, "entity control flags")?,
            entity_rng: read_u16(record, 24, "entity RNG")?,
            action: read_u8(record, 73, "entity action")?,
            action_marker: read_i8(record, 64, "entity action marker")?,
            level_gate: read_i8(record, 74, "entity level gate")?,
            position: Position {
                x: read_u16(record, 80, "entity x")?,
                y: read_u16(record, 82, "entity y")?,
                z: read_i16(record, 84, "entity z")?,
            },
            yaw: read_i16(record, 32, "entity yaw")?,
            pitch: read_i16(record, 34, "entity pitch")?,
            fov: read_i16(record, 38, "entity fov")?,
            phase: read_u8(record, 66, "entity phase")?,
            active_value: read_i16(record, 50, "entity active value")?,
            activation_value: read_u16(record, 52, "entity activation value")?,
            actual_speed: read_i16(record, 134, "entity actual speed")?,
            mana_cost: read_i32(record, 144, "entity mana cost")?,
            mana: read_i32(record, 148, "entity mana")?,
            particle_parameter_index,
            parameter_relation,
            special_relation,
            record: record.to_vec().into_boxed_slice(),
        });
    }
    Ok(entities)
}

fn parse_players(bytes: &[u8], profile: OwnerPackageProfile) -> Result<Vec<PlayerSnapshot>> {
    validate_header(
        bytes,
        *b"MCP1",
        2,
        PLAYER_RECORD_BYTES_U16,
        PLAYER_COUNT_U32,
        "players.bin",
    )?;
    let mut players = Vec::with_capacity(PLAYER_COUNT);
    for slot in 0..PLAYER_COUNT {
        let record = record(bytes, 12, PLAYER_RECORD_BYTES, slot, "players.bin")?;
        if usize::from(read_u16(record, 0, "player index")?) != slot {
            return Err(NativeWorldError::new(format!(
                "player slot/index mismatch at {slot}"
            )));
        }
        let ai = read_u8(record, 2, "player ai flag")?;
        if ai > 1 || record[3] != 0 {
            return Err(NativeWorldError::new(format!(
                "player {slot} has invalid flags"
            )));
        }
        let entity_index = read_u16(record, 4, "player entity index")?;
        if usize::from(entity_index) >= ENTITY_COUNT {
            return Err(NativeWorldError::new(format!(
                "player {slot} entity relation is out of bounds"
            )));
        }
        let left = read_i16(record, 554, "left spell slot")?;
        let right = read_i16(record, 556, "right spell slot")?;
        let level2_empty_hand = profile == OwnerPackageProfile::Level2 && left == -1 && right == -1;
        if (!level2_empty_hand && !(0..PLAYER_SPELL_COUNT_I16).contains(&left))
            || (!level2_empty_hand && !(0..PLAYER_SPELL_COUNT_I16).contains(&right))
        {
            return Err(NativeWorldError::new(format!(
                "player {slot} spell selection is out of bounds"
            )));
        }
        let left_subspell_index = read_u8(record, 558, "left subspell index")?.cast_signed();
        let right_subspell_index = read_u8(record, 559, "right subspell index")?.cast_signed();
        let selected_spell_ui_index = read_u8(record, 561, "selected spell UI index")?;
        let selected_subspell_index =
            read_u8(record, 562, "selected subspell index")?.cast_signed();
        if !(0..=2).contains(&left_subspell_index)
            || !(0..=2).contains(&right_subspell_index)
            || usize::from(selected_spell_ui_index) >= SOURCE_SPELL_INDEX_MAP.len()
            || !(0..=2).contains(&selected_subspell_index)
        {
            return Err(NativeWorldError::new(format!(
                "player {slot} subspell/menu selection is out of bounds"
            )));
        }
        let mut holders = [0_u16; PLAYER_SPELL_COUNT];
        for (index, holder) in holders.iter_mut().enumerate() {
            *holder = read_u16(record, 268 + index * 2, "enabled spell holder")?;
            if usize::from(*holder) >= ENTITY_COUNT && *holder != 0 {
                return Err(NativeWorldError::new(format!(
                    "player {slot} spell holder relation is out of bounds"
                )));
            }
        }
        let mut spell_hand_eligibility = [0_u8; PLAYER_SPELL_COUNT];
        spell_hand_eligibility.copy_from_slice(&record[398..424]);
        let mut family_selected_tiers = [0_i8; PLAYER_SPELL_COUNT];
        for (tier, value) in family_selected_tiers.iter_mut().zip(&record[528..554]) {
            *tier = value.cast_signed();
        }
        players.push(PlayerSnapshot {
            slot: u8::try_from(slot).expect("eight player slots"),
            is_ai: ai == 1,
            entity_index,
            active_player_index: read_i16(record, 6, "active player index")?,
            turn: read_i32(record, 8, "player turn")?,
            left_spell_slot: u8::try_from(left.max(0)).expect("validated spell slot"),
            right_spell_slot: u8::try_from(right.max(0)).expect("validated spell slot"),
            left_spell_slot_raw: left,
            right_spell_slot_raw: right,
            left_subspell_index,
            right_subspell_index,
            selected_spell_ui_index,
            selected_subspell_index,
            enabled_spell_holders: holders,
            spell_hand_eligibility,
            family_selected_tiers,
            record: record.to_vec().into_boxed_slice(),
        });
    }
    Ok(players)
}

#[allow(clippy::too_many_lines)]
fn parse_runtime(bytes: &[u8]) -> Result<RuntimeSnapshot> {
    if bytes.len() != RUNTIME_BYTES
        || bytes.get(..4) != Some(b"MCR1")
        || read_u16(bytes, 4, "runtime version")? != 1
        || read_u16(bytes, 6, "runtime reserved")? != 0
    {
        return Err(NativeWorldError::new(
            "runtime.bin header/version/extent mismatch",
        ));
    }
    let local_player = read_i16(bytes, 16, "runtime local player slot")?;
    let player_count = read_i16(bytes, 18, "runtime player count")?;
    if !(0..PLAYER_COUNT_I16).contains(&local_player)
        || !(1..=PLAYER_COUNT_I16).contains(&player_count)
    {
        return Err(NativeWorldError::new(
            "runtime player slot/count is out of bounds",
        ));
    }
    let mut stages = Vec::with_capacity(8);
    for index in 0..8 {
        let stage = record(bytes, 192, 14, index, "runtime stages")?;
        let domain = read_u8(stage, 6, "runtime stage relation domain")?;
        if stage[7..10] != [0, 0, 0] {
            return Err(NativeWorldError::new(
                "runtime stage relation padding is non-zero",
            ));
        }
        let value = read_u32(stage, 10, "runtime stage relation")?;
        let relation = match domain {
            0 => RuntimeRelation::Scalar(value),
            1 => RuntimeRelation::AuthoredFeature(one_based(
                value,
                1_200,
                "runtime authored-feature relation",
            )?),
            2 => RuntimeRelation::LiveEntity(one_based(
                value,
                ENTITY_COUNT,
                "runtime live-entity relation",
            )?),
            3 if value < ENTITY_COUNT_U32 => {
                RuntimeRelation::LiveEntityIndex(u16::try_from(value).expect("entity index bound"))
            }
            _ => {
                return Err(NativeWorldError::new(format!(
                    "runtime stage {index} relation {domain}/{value} is invalid"
                )));
            }
        };
        stages.push(RuntimeStage {
            kind: read_i8(stage, 0, "runtime stage type")?,
            flags: read_i8(stage, 1, "runtime stage flags")?,
            axis_x: read_i16(stage, 2, "runtime stage x")?,
            axis_y: read_i16(stage, 4, "runtime stage y")?,
            relation,
        });
    }
    let mut objectives = Vec::with_capacity(8);
    for index in 0..8 {
        let objective = record(bytes, 304, 11, index, "runtime objectives")?;
        objectives.push(RuntimeObjective {
            is_level_end: objective[0],
            text_selector: objective[1],
            done: objective[2],
            stage_selectors: objective[3..11]
                .try_into()
                .expect("validated objective record width"),
        });
    }
    let mut stage_variables = Vec::with_capacity(11);
    for index in 0..11 {
        let stage_var = record(bytes, 392, 12, index, "runtime stage variables")?;
        let domain = read_u8(stage_var, 4, "runtime stage-variable domain")?;
        if stage_var[5..8] != [0, 0, 0] {
            return Err(NativeWorldError::new(
                "runtime stage-variable relation padding is non-zero",
            ));
        }
        let value = read_u32(stage_var, 8, "runtime stage-variable relation")?;
        let relation = match domain {
            0 => RuntimeRelation::AxisScalar(value),
            1 => RuntimeRelation::LiveEntity(one_based(
                value,
                ENTITY_COUNT,
                "runtime stage-variable entity relation",
            )?),
            _ => {
                return Err(NativeWorldError::new(format!(
                    "runtime stage-variable {index} domain is invalid"
                )));
            }
        };
        stage_variables.push(RuntimeStageVariable {
            kind: read_i8(stage_var, 0, "runtime stage-variable type")?,
            flags: read_i8(stage_var, 1, "runtime stage-variable flags")?,
            word_2: read_u16(stage_var, 2, "runtime stage-variable word 2")?,
            relation,
        });
    }
    for index in 0..50 {
        let event = record(bytes, 524, 43, index, "runtime events")?;
        let domain = read_u8(event, 10, "runtime event domain")?;
        if event[11..14] != [0, 0, 0] {
            return Err(NativeWorldError::new(
                "runtime event relation padding is non-zero",
            ));
        }
        let value = read_u32(event, 14, "runtime event relation")?;
        match (domain, value) {
            (0, 0) => {}
            (1, _) => {
                one_based(value, ENTITY_COUNT, "runtime event entity relation")?;
            }
            _ => {
                return Err(NativeWorldError::new(format!(
                    "runtime event {index} relation is invalid"
                )));
            }
        }
    }
    let mut player_inputs = [PlayerCommand::default(); PLAYER_COUNT];
    for (index, command) in player_inputs.iter_mut().enumerate() {
        let input = record(bytes, 2_674, 10, index, "runtime player inputs")?;
        *command = PlayerCommand::from_bytes(
            input
                .try_into()
                .map_err(|_| NativeWorldError::new("runtime input extent mismatch"))?,
        );
    }
    Ok(RuntimeSnapshot {
        global_turn: read_u32(bytes, 8, "runtime global turn")?,
        random_state: read_u32(bytes, 12, "runtime random state")?,
        local_player_slot: u8::try_from(local_player).expect("validated player slot"),
        player_count: u8::try_from(player_count).expect("validated player count"),
        maptype_music: read_i32(bytes, 32, "runtime maptype music")?,
        stage_index: read_u8(bytes, 65, "runtime stage index")?,
        objective_presentation_phase: read_u8(bytes, 67, "runtime objective presentation phase")?,
        objective_textbox_ticks: read_u8(bytes, 69, "runtime objective textbox ticks")?,
        stages: stages.into_boxed_slice(),
        stage_variables: stage_variables.into_boxed_slice(),
        objectives: objectives.into_boxed_slice(),
        player_inputs,
        bytes: bytes.to_vec().into_boxed_slice(),
    })
}

fn parse_presentation(bytes: &[u8]) -> Result<PresentationSelectors> {
    if bytes.len() != 10_720
        || bytes.get(..4) != Some(b"MCV1")
        || read_u16(bytes, 4, "presentation version")? != 1
        || read_u16(bytes, 6, "presentation reserved")? != 0
        || read_u32(bytes, 8, "presentation particle count")? != PARTICLE_COUNT_U32
    {
        return Err(NativeWorldError::new(
            "presentation.bin header/version/count/extent mismatch",
        ));
    }
    let mut particles = Vec::with_capacity(PARTICLE_COUNT);
    for index in 0..PARTICLE_COUNT {
        let particle = record(
            bytes,
            12,
            PARTICLE_RECORD_BYTES,
            index,
            "presentation particles",
        )?;
        let render_mode = read_u8(particle, 12, "particle render mode")?;
        if render_mode > 36 {
            return Err(NativeWorldError::new(format!(
                "particle {index} render mode {render_mode} is invalid"
            )));
        }
        particles.push(ParticleParameter {
            base_tmap: read_u16(particle, 0, "particle base tmap")?,
            word_2: read_u16(particle, 2, "particle word 2")?,
            word_4: read_u16(particle, 4, "particle word 4")?,
            speed: read_u16(particle, 6, "particle speed")?,
            rotation_speed: read_u16(particle, 8, "particle rotation speed")?,
            byte_10: read_u8(particle, 10, "particle byte 10")?,
            byte_11: read_i8(particle, 11, "particle byte 11")?,
            render_mode,
            byte_13: read_u8(particle, 13, "particle byte 13")?,
        });
    }
    let selector_count_offset = 12 + PARTICLE_COUNT * PARTICLE_RECORD_BYTES;
    if read_u32(bytes, selector_count_offset, "presentation selector count")?
        != LEVEL_SPRITE_SELECTOR_COUNT_U32
    {
        return Err(NativeWorldError::new(
            "presentation level-sprite selector count mismatch",
        ));
    }
    let selector_start = selector_count_offset + 4;
    let selector_end = selector_start + LEVEL_SPRITE_SELECTOR_COUNT;
    let parameter_count_offset = selector_end;
    if read_u32(
        bytes,
        parameter_count_offset,
        "presentation parameter count",
    )? != ENTITY_PARAMETER_COUNT_U32
    {
        return Err(NativeWorldError::new(
            "presentation entity-parameter count mismatch",
        ));
    }
    let parameter_start = parameter_count_offset + 4;
    let mut entity_parameters = Vec::with_capacity(ENTITY_PARAMETER_COUNT);
    for index in 0..ENTITY_PARAMETER_COUNT {
        entity_parameters.push(
            record(
                bytes,
                parameter_start,
                ENTITY_PARAMETER_BYTES,
                index,
                "presentation entity parameters",
            )?
            .try_into()
            .map_err(|_| NativeWorldError::new("entity-parameter extent mismatch"))?,
        );
    }
    Ok(PresentationSelectors {
        particles: particles.into_boxed_slice(),
        level_sprite_selectors: bytes[selector_start..selector_end]
            .to_vec()
            .into_boxed_slice(),
        entity_parameters: entity_parameters.into_boxed_slice(),
    })
}

fn validate_manifest_relations(
    counts: &serde_json::Map<String, Value>,
    entities: &[EntitySnapshot],
    players: &[PlayerSnapshot],
    runtime: &RuntimeSnapshot,
    presentation: &PresentationSelectors,
) -> Result<()> {
    let active_count = entities.iter().filter(|entity| entity.active).count();
    let expected_active = require_value(counts, "active_entities", "manifest.counts")?
        .as_u64()
        .ok_or_else(|| NativeWorldError::new("active_entities is not u64"))?;
    if active_count as u64 != expected_active {
        return Err(NativeWorldError::new(format!(
            "active entity count mismatch: {active_count}/{expected_active}"
        )));
    }
    if players.len() != PLAYER_COUNT
        || runtime.stages.len() != 8
        || runtime.stage_variables.len() != 11
        || runtime.objectives.len() != 8
        || presentation.particles.len() != PARTICLE_COUNT
        || presentation.level_sprite_selectors.len() != LEVEL_SPRITE_SELECTOR_COUNT
        || presentation.entity_parameters.len() != ENTITY_PARAMETER_COUNT
    {
        return Err(NativeWorldError::new(
            "decoded component count does not match manifest",
        ));
    }
    Ok(())
}

/// Decode the Level 2 frame-zero flight projection directly from the typed
/// owner rows. The package's optional `reset_projection` is a copied Level 1
/// seal, so it cannot identify the Level 2 player (slot 111).
fn parse_level_two_reset_state(
    entities: &[u8],
    players: &[u8],
    runtime: &[u8],
) -> Result<FlightState> {
    let slot = read_i16(runtime, 16, "Level 2 local player slot")?;
    if slot != 0 {
        return Err(NativeWorldError::new(format!(
            "Level 2 reset requires local player slot 0, got {slot}"
        )));
    }
    let player = record(players, 12, PLAYER_RECORD_BYTES, 0, "players.bin")?;
    let entity_index = read_u16(player, 4, "Level 2 local player entity relation")?;
    if entity_index != 111 {
        return Err(NativeWorldError::new(format!(
            "Level 2 reset requires local wizard entity 111, got {entity_index}"
        )));
    }
    let entity = record(
        entities,
        12,
        ENTITY_RECORD_BYTES,
        usize::from(entity_index),
        "entities.bin",
    )?;
    let active = read_u8(entity, 2, "Level 2 local wizard active flag")?;
    if active != 1 {
        return Err(NativeWorldError::new(
            "Level 2 local wizard reset entity is inactive",
        ));
    }
    let state = FlightState {
        entity_index,
        position: Position {
            x: read_u16(entity, 80, "Level 2 position.x")?,
            y: read_u16(entity, 82, "Level 2 position.y")?,
            z: read_i16(entity, 84, "Level 2 position.z")?,
        },
        yaw: read_i16(entity, 32, "Level 2 yaw")?,
        display_pitch: read_i16(entity, 34, "Level 2 display pitch")?,
        effective_pitch: read_i16(player, 38, "Level 2 effective pitch")?,
        roll_accumulator: read_i16(player, 46, "Level 2 roll accumulator")?,
        pitch_accumulator: read_i16(player, 48, "Level 2 pitch accumulator")?,
        target_speed: read_i16(player, 20, "Level 2 target speed")?,
        speed_changed_flag: 0,
        actual_speed: read_i16(entity, 134, "Level 2 actual speed")?,
        strafe_speed: read_i16(player, 22, "Level 2 strafe speed")?,
        roll_delta: read_i16(player, 16, "Level 2 roll delta")?,
        pitch_delta: read_i16(player, 18, "Level 2 pitch delta")?,
        recoil_speed: read_i16(player, 32, "Level 2 recoil speed")?,
        recoil_yaw: read_i16(player, 34, "Level 2 recoil yaw")?,
        impulse_x: read_i16(player, 54, "Level 2 impulse x")?,
        impulse_y: read_i16(player, 56, "Level 2 impulse y")?,
        impulse_z: read_i16(player, 58, "Level 2 impulse z")?,
        entity_fov: read_i16(entity, 38, "Level 2 entity fov")?,
        water_counter: 0,
        speed_modifier: read_u8(player, 44, "Level 2 speed modifier")?,
        speed_modifier_counter: read_i8(player, 45, "Level 2 speed modifier counter")?,
        immobilize_counter: 0,
        immobilize_subcounter: 0,
        tether_prior_distance: 0,
        tether_target_index: 0,
        tether_holder_index: 0,
        tether_tier: 0,
        speed_spell_holder_index: 0,
        entity_phase: read_u8(entity, 66, "Level 2 entity phase")?,
        ambient_loop_5_distance: read_i32(player, 565, "Level 2 ambient loop 5 distance")?,
        ambient_loop_31_distance: read_i32(player, 569, "Level 2 ambient loop 31 distance")?,
        player_color_index: read_u8(player, 40, "Level 2 player color")?,
        music_countdown: 0,
        recovery_latch: false,
        skip_flight_once: read_u8(entity, 17, "Level 2 entity control flags")? & 8 != 0,
        tick: 0,
    };
    Ok(state)
}

#[allow(clippy::too_many_lines)]
fn parse_reset_state(
    reset: &serde_json::Map<String, Value>,
    entities: &[u8],
    players: &[u8],
    runtime: &[u8],
) -> Result<FlightState> {
    if require_string(reset, "schema", "reset_projection")? != RESET_SCHEMA {
        return Err(NativeWorldError::new(
            "reset projection schema must be mc2-flight-reset-projection/v1",
        ));
    }
    let source = require_object(reset, "source", "reset_projection")?;
    require_exact_u64(source, "capture_frame", 0, "reset_projection.source")?;
    let local_mapping = require_object(source, "local_player_slot", "reset_projection.source")?;
    require_mapping_scalar(
        local_mapping,
        "runtime.bin",
        16,
        "i16_le",
        "local_player_slot",
    )?;
    require_exact_i64(local_mapping, "minimum", 0, "local_player_slot")?;
    require_exact_i64(local_mapping, "maximum", 7, "local_player_slot")?;
    let slot = read_i16(runtime, 16, "local player slot")?;
    if !(0..PLAYER_COUNT_I16).contains(&slot) {
        return Err(NativeWorldError::new(
            "reset local-player slot is out of bounds",
        ));
    }
    let player = record(
        players,
        12,
        PLAYER_RECORD_BYTES,
        usize::try_from(slot).expect("validated player slot"),
        "players.bin",
    )?;
    let entity_index = read_u16(player, 4, "reset player entity relation")?;
    let entity = record(
        entities,
        12,
        ENTITY_RECORD_BYTES,
        usize::from(entity_index),
        "entities.bin",
    )?;
    let mappings = require_object(reset, "field_mappings", "reset_projection")?;

    let exported_i16 = |name: &str, offset: usize| -> Result<i16> {
        require_mapping(
            mappings,
            name,
            "exported_field",
            "players.bin",
            Some("local_player_slot"),
            offset,
            "i16_le",
        )?;
        read_i16(player, offset, name)
    };
    let signed_entity_field = |name: &str, offset: usize| -> Result<i16> {
        require_mapping(
            mappings,
            name,
            "entity_relation_field",
            "entities.bin",
            None,
            offset,
            "i16_le",
        )?;
        read_i16(entity, offset, name)
    };
    let unsigned_entity_field = |name: &str, offset: usize| -> Result<u16> {
        require_mapping(
            mappings,
            name,
            "entity_relation_field",
            "entities.bin",
            None,
            offset,
            "u16_le",
        )?;
        read_u16(entity, offset, name)
    };
    require_mapping(
        mappings,
        "entity_index",
        "exported_field",
        "players.bin",
        Some("local_player_slot"),
        4,
        "u16_le",
    )?;
    let state = FlightState {
        entity_index,
        position: Position {
            x: unsigned_entity_field("position.x", 80)?,
            y: unsigned_entity_field("position.y", 82)?,
            z: signed_entity_field("position.z", 84)?,
        },
        yaw: signed_entity_field("yaw", 32)?,
        display_pitch: signed_entity_field("display_pitch", 34)?,
        effective_pitch: exported_i16("effective_pitch", 38)?,
        roll_accumulator: exported_i16("roll_accumulator", 46)?,
        pitch_accumulator: exported_i16("pitch_accumulator", 48)?,
        target_speed: exported_i16("target_speed", 20)?,
        speed_changed_flag: zero_i16(mappings, "speed_changed_flag")?,
        actual_speed: signed_entity_field("actual_speed", 134)?,
        strafe_speed: exported_i16("strafe_speed", 22)?,
        roll_delta: exported_i16("roll_delta", 16)?,
        pitch_delta: exported_i16("pitch_delta", 18)?,
        recoil_speed: exported_i16("recoil_speed", 32)?,
        recoil_yaw: exported_i16("recoil_yaw", 34)?,
        impulse_x: exported_i16("impulse_x", 54)?,
        impulse_y: exported_i16("impulse_y", 56)?,
        impulse_z: exported_i16("impulse_z", 58)?,
        entity_fov: signed_entity_field("entity_fov", 38)?,
        water_counter: zero_i8(mappings, "water_counter")?,
        speed_modifier: {
            require_mapping(
                mappings,
                "speed_modifier",
                "exported_field",
                "players.bin",
                Some("local_player_slot"),
                44,
                "u8",
            )?;
            read_u8(player, 44, "speed modifier")?
        },
        speed_modifier_counter: {
            require_mapping(
                mappings,
                "speed_modifier_counter",
                "exported_field",
                "players.bin",
                Some("local_player_slot"),
                45,
                "i8",
            )?;
            read_i8(player, 45, "speed modifier counter")?
        },
        immobilize_counter: zero_i8(mappings, "immobilize_counter")?,
        immobilize_subcounter: zero_i8(mappings, "immobilize_subcounter")?,
        tether_prior_distance: zero_i32(mappings, "tether_prior_distance")?,
        tether_target_index: zero_u16(mappings, "tether_target_index")?,
        tether_holder_index: zero_u16(mappings, "tether_holder_index")?,
        tether_tier: zero_u16(mappings, "tether_tier")?,
        speed_spell_holder_index: zero_u16(mappings, "speed_spell_holder_index")?,
        entity_phase: {
            require_mapping(
                mappings,
                "entity_phase",
                "entity_relation_field",
                "entities.bin",
                None,
                66,
                "u8",
            )?;
            read_u8(entity, 66, "entity phase")?
        },
        ambient_loop_5_distance: mapped_player_i32(
            mappings,
            player,
            "ambient_loop_5_distance",
            565,
        )?,
        ambient_loop_31_distance: mapped_player_i32(
            mappings,
            player,
            "ambient_loop_31_distance",
            569,
        )?,
        player_color_index: {
            require_mapping(
                mappings,
                "player_color_index",
                "exported_field",
                "players.bin",
                Some("local_player_slot"),
                40,
                "u8_low_of_i16_le",
            )?;
            read_u8(player, 40, "player color index")?
        },
        music_countdown: zero_i16(mappings, "music_countdown")?,
        recovery_latch: zero_bool(mappings, "recovery_latch")?,
        skip_flight_once: {
            let mapping = require_object(mappings, "skip_flight_once", "field_mappings")?;
            require_string(mapping, "kind", "skip_flight_once")?
                .eq("entity_relation_bit")
                .then_some(())
                .ok_or_else(|| NativeWorldError::new("skip_flight_once kind mismatch"))?;
            require_string(mapping, "file", "skip_flight_once")?
                .eq("entities.bin")
                .then_some(())
                .ok_or_else(|| NativeWorldError::new("skip_flight_once file mismatch"))?;
            require_exact_u64(mapping, "offset", 17, "skip_flight_once")?;
            require_exact_u64(mapping, "bit", 3, "skip_flight_once")?;
            read_u8(entity, 17, "skip flight entity flags")? & 8 != 0
        },
        tick: u64::try_from(zero_i64(mappings, "tick")?)
            .map_err(|_| NativeWorldError::new("tick seam default is negative"))?,
    };
    if mappings.len() != 39 {
        return Err(NativeWorldError::new(format!(
            "reset projection must map exactly 39 fields, got {}",
            mappings.len()
        )));
    }
    Ok(state)
}

fn build_level_one_context(
    metadata: &LevelMetadata,
    state: &FlightState,
    players: &[PlayerSnapshot],
    entities: &[EntitySnapshot],
) -> Result<FlightContext> {
    if metadata.external_level != 1 || metadata.internal_level != 0 {
        return Err(NativeWorldError::new(
            "context reconstruction currently supports only export Level 1",
        ));
    }
    let player_slot = players
        .iter()
        .position(|player| player.entity_index == state.entity_index)
        .ok_or_else(|| NativeWorldError::new("reset entity has no player relation"))?;
    let player = &players[player_slot];
    let left_slot = usize::from(player.left_spell_slot);
    let holder_index = player.enabled_spell_holders[left_slot];
    if holder_index == 0 || usize::from(holder_index) >= entities.len() {
        return Err(NativeWorldError::new(
            "Level 1 left-spell activation holder is absent or invalid",
        ));
    }
    let caster = &entities[usize::from(state.entity_index)];
    let holder = &entities[usize::from(holder_index)];
    if !caster.active || !holder.active {
        return Err(NativeWorldError::new(
            "Level 1 flight caster or left-spell holder is inactive",
        ));
    }
    Ok(FlightContext {
        effects: None,
        spell_fire: Some(SpellFireContext {
            left_holder_index: holder_index,
            left_subspell_index: player.left_subspell_index,
            right_subspell_index: player.right_subspell_index,
            selected_spell_ui_index: player.selected_spell_ui_index,
            selected_subspell_index: player.selected_subspell_index,
            spell_hand_eligibility: player.spell_hand_eligibility,
            family_selected_tiers: player.family_selected_tiers,
            caster: SpellCasterState {
                entity_index: caster.index,
                model: caster.model,
                mana: caster.mana,
                control_flags: caster.control_flags,
                player_color_index: state.player_color_index,
                // This value is proven for Level 1 by the preserved regression
                // state but is not serialized by package v2.
                control_mode: LEVEL_ONE_CONTROL_MODE_SEAM_DEFAULT,
            },
            holders: vec![SpellActivationHolder {
                entity_index: holder.index,
                model: holder.model,
                level_gate: holder.level_gate,
                active_value: holder.active_value,
                activation_value: holder.activation_value,
                mana_cost: holder.mana_cost,
                action_marker: holder.action_marker,
            }],
            ..SpellFireContext::default()
        }),
        ..FlightContext::default()
    })
}

fn validate_reset_state_hash_contract(
    reset: &serde_json::Map<String, Value>,
    state: &FlightState,
) -> Result<()> {
    if require_value(reset, "expected_state", "reset_projection")? != &flight_state_json(state) {
        return Err(NativeWorldError::new(
            "decoded reset state does not match manifest expected_state",
        ));
    }
    let hash = require_object(reset, "state_hash", "reset_projection")?;
    for (name, expected) in [
        ("api", "mc_sim_reset"),
        ("schema", "mc2-flight-state-hash/fnv1a64-v1"),
        ("algorithm", "FNV-1a 64-bit"),
        ("byte_order", "little-endian"),
    ] {
        if require_string(hash, name, "reset state_hash")? != expected {
            return Err(NativeWorldError::new(format!(
                "reset state_hash {name} contract mismatch"
            )));
        }
    }
    require_exact_u64(hash, "canonical_bytes", 81, "reset state_hash")?;
    let ordered_fields = require_array(hash, "ordered_fields", "reset state_hash")?;
    let expected_fields = [
        "entity_index:u16",
        "position.x:u16",
        "position.y:u16",
        "position.z:i16",
        "yaw:i16",
        "display_pitch:i16",
        "effective_pitch:i16",
        "roll_accumulator:i16",
        "pitch_accumulator:i16",
        "target_speed:i16",
        "speed_changed_flag:i16",
        "actual_speed:i16",
        "strafe_speed:i16",
        "roll_delta:i16",
        "pitch_delta:i16",
        "recoil_speed:i16",
        "recoil_yaw:i16",
        "impulse_x:i16",
        "impulse_y:i16",
        "impulse_z:i16",
        "entity_fov:i16",
        "water_counter:i8",
        "speed_modifier:u8",
        "speed_modifier_counter:i8",
        "immobilize_counter:i8",
        "immobilize_subcounter:i8",
        "tether_prior_distance:i32",
        "tether_target_index:u16",
        "tether_holder_index:u16",
        "tether_tier:u16",
        "speed_spell_holder_index:u16",
        "entity_phase:u8",
        "ambient_loop_5_distance:i32",
        "ambient_loop_31_distance:i32",
        "player_color_index:u8",
        "music_countdown:i16",
        "recovery_latch:u8",
        "skip_flight_once:u8",
        "tick:u64",
    ];
    if ordered_fields.len() != expected_fields.len()
        || ordered_fields
            .iter()
            .zip(expected_fields)
            .any(|(actual, expected)| actual.as_str() != Some(expected))
    {
        return Err(NativeWorldError::new(
            "reset state_hash ordered_fields contract mismatch",
        ));
    }
    let expected_hash = parse_u64_hex(require_string(hash, "hex", "reset state_hash")?)?;
    if state_hash(state) != expected_hash {
        return Err(NativeWorldError::new("reset state hash mismatch"));
    }
    Ok(())
}

fn validate_reset_contract(
    reset: &serde_json::Map<String, Value>,
    state: &FlightState,
    context: &FlightContext,
) -> Result<()> {
    validate_reset_state_hash_contract(reset, state)?;
    let conformance = require_object(reset, "conformance_hash", "reset_projection")?;
    if require_string(conformance, "schema", "reset conformance_hash")?
        != "mc2-flight-conformance-hash/fnv1a64-v1"
        || require_string(conformance, "context", "reset conformance_hash")?
            != "NativeSim::owner_level_one"
    {
        return Err(NativeWorldError::new(
            "reset conformance hash contract mismatch",
        ));
    }
    let expected_conformance = parse_u64_hex(require_string(
        conformance,
        "hex",
        "reset conformance_hash",
    )?)?;
    let actual_conformance = conformance_hash(state, context);
    if actual_conformance != expected_conformance {
        return Err(NativeWorldError::new(format!(
            "reset conformance hash mismatch: expected {expected_conformance:016x}, got {actual_conformance:016x}"
        )));
    }
    Ok(())
}

fn apply_authority(
    entities: &mut [EntitySnapshot],
    state: &FlightState,
    context: &FlightContext,
) -> Result<()> {
    let player = entities
        .get_mut(usize::from(state.entity_index))
        .ok_or_else(|| NativeWorldError::new("reset player entity is out of bounds"))?;
    if !player.active {
        return Err(NativeWorldError::new("reset player entity is inactive"));
    }
    player.authority = EntityAuthority::PlayerFlightLive;
    let fire = context
        .spell_fire
        .as_ref()
        .ok_or_else(|| NativeWorldError::new("Level 1 spell context is absent"))?;
    if fire.holders.len() != 1 || fire.holders[0].entity_index != fire.left_holder_index {
        return Err(NativeWorldError::new(
            "Level 1 spell activation authority is not singular",
        ));
    }
    let holder = entities
        .get_mut(usize::from(fire.left_holder_index))
        .ok_or_else(|| NativeWorldError::new("spell holder entity is out of bounds"))?;
    if !holder.active {
        return Err(NativeWorldError::new("spell holder entity is inactive"));
    }
    holder.authority = EntityAuthority::SpellActivationLive;
    for index in LEVEL_ONE_GOAT_INDICES {
        let goat = entities.get_mut(usize::from(index)).ok_or_else(|| {
            NativeWorldError::new(format!("Goat entity {index} is out of bounds"))
        })?;
        if !goat.active || goat.class != GOAT_CLASS || goat.model != GOAT_MODEL {
            return Err(NativeWorldError::new(format!(
                "Goat authority row {index} is not active class {GOAT_CLASS}/model {GOAT_MODEL}"
            )));
        }
        goat.authority = EntityAuthority::GoatLive;
    }
    for index in LEVEL_ONE_VILLAGER_INDICES {
        let villager = entities.get_mut(usize::from(index)).ok_or_else(|| {
            NativeWorldError::new(format!("Villager entity {index} is out of bounds"))
        })?;
        if !villager.active || villager.class != VILLAGER_CLASS || villager.model != VILLAGER_MODEL
        {
            return Err(NativeWorldError::new(format!(
                "Villager authority row {index} is not active class {VILLAGER_CLASS}/model {VILLAGER_MODEL}"
            )));
        }
        villager.authority = EntityAuthority::VillagerLive;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn parse_closure(
    path: &Path,
    package_sha256: &str,
    manifest_bytes: &[u8],
    members: &BTreeMap<String, Vec<u8>>,
) -> Result<(String, Vec<AssetRef>)> {
    let bytes = read_file(path)?;
    let mut closure: Value = serde_json::from_slice(&bytes)
        .map_err(|error| NativeWorldError::new(format!("invalid closure JSON: {error}")))?;
    let closure_object = as_object(&closure, "closure")?;
    if require_string(closure_object, "schema", "closure")? != CLOSURE_SCHEMA {
        return Err(NativeWorldError::new(
            "closure schema must be mc2-level-asset-closure/v2",
        ));
    }
    if require_string(closure_object, "scope", "closure")? != "PRIVATE_TRACK_P_OWNER_DATA" {
        return Err(NativeWorldError::new("closure scope mismatch"));
    }
    let expected_closure_hash =
        require_hash_string(closure_object, "closure_sha256", "closure")?.to_owned();
    as_object_mut(&mut closure, "closure")?.remove("closure_sha256");
    let canonical = canonical_json(&closure)?;
    if sha256_hex(canonical.as_bytes()) != expected_closure_hash {
        return Err(NativeWorldError::new("closure_sha256 mismatch"));
    }

    let object = as_object(
        &serde_json::from_slice::<Value>(&bytes)
            .map_err(|error| NativeWorldError::new(format!("invalid closure JSON: {error}")))?,
        "closure",
    )?
    .clone();
    let source = require_object(&object, "source", "closure")?;
    let level_export = require_object(source, "level_export", "closure.source")?;
    if require_hash_string(level_export, "package_sha256", "closure level_export")?
        != package_sha256
    {
        return Err(NativeWorldError::new(
            "closure level-export package binding mismatch",
        ));
    }
    let trace = require_object(source, "frame_trace", "closure.source")?;
    let bindings = require_object(trace, "bindings", "closure frame_trace")?;
    if require_hash_string(
        bindings,
        "level_export_package_sha256",
        "closure frame bindings",
    )? != package_sha256
    {
        return Err(NativeWorldError::new(
            "closure frame-trace package binding mismatch",
        ));
    }
    require_exact_u64(trace, "canonical_event_count", 76, "closure frame_trace")?;
    if require_value(trace, "byte_identical", "closure frame_trace")? != &Value::Bool(true)
        || require_value(trace, "independent_pass_runs", "closure frame_trace")?
            != &Value::Bool(true)
    {
        return Err(NativeWorldError::new(
            "closure frame trace is not independently byte-identical",
        ));
    }
    let closure_files = require_object(level_export, "files", "closure level_export")?;
    for (name, entry) in closure_files {
        let member_bytes = if name == "manifest.json" {
            manifest_bytes
        } else {
            member(members, name)?
        };
        let expected_size = require_value(
            as_object(entry, "closure file entry")?,
            "size",
            "closure file entry",
        )?
        .as_u64()
        .ok_or_else(|| NativeWorldError::new("closure file size is not u64"))?;
        let expected_hash =
            require_hash_string(as_object(entry, "closure file entry")?, "sha256", name)?;
        if member_bytes.len() as u64 != expected_size || sha256_hex(member_bytes) != expected_hash {
            return Err(NativeWorldError::new(format!(
                "closure file binding mismatch for {name}"
            )));
        }
    }
    for required in [
        "manifest.json",
        "entities.bin",
        "presentation.bin",
        "runtime.bin",
    ] {
        if !closure_files.contains_key(required) {
            return Err(NativeWorldError::new(format!(
                "closure level-export binding omits {required}"
            )));
        }
    }

    let qualification = require_object(&object, "qualification", "closure")?;
    for (name, expected) in [
        ("canonical_frame_events_promoted", 76),
        ("unresolved_frame_used", 0),
        ("ambiguous_frame_used", 0),
    ] {
        require_exact_u64(qualification, name, expected, "closure qualification")?;
    }
    if require_value(
        qualification,
        "zero_unresolved_frame_used_claimed",
        "closure qualification",
    )? != &Value::Bool(true)
        || require_string(qualification, "frame_used", "closure qualification")?
            != "PASS_FINAL_ENTRY_TRACE"
    {
        return Err(NativeWorldError::new(
            "closure qualification does not prove zero unresolved frame use",
        ));
    }
    let sets = require_object(&object, "sets", "closure")?;
    let frame_used = require_object(sets, "frame_used", "closure sets")?;
    if require_string(frame_used, "status", "closure frame_used")? != "qualified_final_entry_trace"
        || !require_array(frame_used, "unresolved", "closure frame_used")?.is_empty()
    {
        return Err(NativeWorldError::new(
            "closure frame-used set is not fully resolved",
        ));
    }
    let assets = require_array(frame_used, "assets", "closure frame_used")?;
    if assets.len() != FRAME_ASSET_COUNT {
        return Err(NativeWorldError::new(format!(
            "closure must contain {FRAME_ASSET_COUNT} frame assets, got {}",
            assets.len()
        )));
    }
    let mut refs = Vec::with_capacity(FRAME_ASSET_COUNT);
    for (index, asset) in assets.iter().enumerate() {
        let asset = as_object(asset, "frame asset")?;
        let ordinal = as_u16(
            require_value(asset, "trace_event_ordinal", "frame asset")?,
            "trace_event_ordinal",
        )?;
        let expected_ordinal = u16::try_from(index + 1).expect("76 assets");
        if ordinal != expected_ordinal {
            return Err(NativeWorldError::new(format!(
                "frame asset ordinal mismatch: {ordinal}/{expected_ordinal}"
            )));
        }
        let event = require_value(asset, "trace_event", "frame asset")?.clone();
        let event_hash = sha256_hex(canonical_json(&event)?.as_bytes());
        let declared_event_hash =
            require_hash_string(asset, "trace_event_sha256", "frame asset")?.to_owned();
        if event_hash != declared_event_hash {
            return Err(NativeWorldError::new(format!(
                "frame asset {ordinal} event hash mismatch"
            )));
        }
        let content_address = require_string(asset, "content_address", "frame asset")?.to_owned();
        validate_content_address(&content_address)?;
        if content_address != format!("sha256:{declared_event_hash}") {
            return Err(NativeWorldError::new(format!(
                "frame asset {ordinal} content address does not bind its event"
            )));
        }
        if require_string(asset, "resolution", "frame asset")? != "unique" {
            return Err(NativeWorldError::new(format!(
                "frame asset {ordinal} is not uniquely resolved"
            )));
        }
        let owner_ids = require_array(asset, "owner_asset_ids", "frame asset")?;
        if owner_ids.is_empty() {
            return Err(NativeWorldError::new(format!(
                "frame asset {ordinal} has no owner asset relation"
            )));
        }
        let event_object = as_object(&event, "trace event")?;
        refs.push(AssetRef {
            asset_id: require_string(asset, "asset_id", "frame asset")?.to_owned(),
            content_address,
            owner_asset_ids: owner_ids
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| NativeWorldError::new("owner asset id is not a string"))
                })
                .collect::<Result<Vec<_>>>()?
                .into_boxed_slice(),
            observation: FrameObservation {
                ordinal,
                category: require_string(event_object, "category", "trace event")?.to_owned(),
                hook: require_string(event_object, "hook", "trace event")?.to_owned(),
                stage: event_object
                    .get("stage")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                selector: event_object.get("selector").cloned().unwrap_or(Value::Null),
                event_sha256: declared_event_hash,
                event,
            },
        });
    }
    Ok((expected_closure_hash, refs))
}

fn read_owner_cache_output(root: &Path, relative: &str) -> Result<Vec<u8>> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(NativeWorldError::new(
            "owner cache output path is not a confined relative path",
        ));
    }
    let root = root
        .canonicalize()
        .map_err(|error| NativeWorldError::new(format!("cannot resolve owner cache: {error}")))?;
    let candidate = root.join(relative_path);
    if fs::symlink_metadata(&candidate)
        .map_err(|error| NativeWorldError::new(format!("cannot inspect owner output: {error}")))?
        .file_type()
        .is_symlink()
    {
        return Err(NativeWorldError::new("owner cache output is a symlink"));
    }
    let resolved = candidate
        .canonicalize()
        .map_err(|error| NativeWorldError::new(format!("cannot resolve owner output: {error}")))?;
    if !resolved.starts_with(&root) {
        return Err(NativeWorldError::new("owner cache output escapes its root"));
    }
    read_file(&resolved)
}

/// One independently identity-qualified view of a private owner cache.
///
/// Consumers must obtain every parsed output through `read`: the manifest is
/// checked once, duplicate paths fail closed, and each read is bound to both
/// its declared byte count and SHA-256.
pub(crate) struct QualifiedOwnerCacheReader {
    root: PathBuf,
    manifest: Value,
    outputs: BTreeMap<String, (String, u64)>,
}

impl QualifiedOwnerCacheReader {
    pub(crate) fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        let manifest_bytes = read_owner_cache_output(root, "manifest.json")?;
        let manifest_sha256 = sha256_hex(&manifest_bytes);
        let manifest: Value = serde_json::from_slice(&manifest_bytes).map_err(|error| {
            NativeWorldError::new(format!("invalid owner cache manifest JSON: {error}"))
        })?;
        let object = as_object(&manifest, "owner cache manifest")?;
        let content_sha256 =
            require_hash_string(object, "cache_content_sha256", "owner cache manifest")?;
        let qualified_identity = matches!(
            (manifest_sha256.as_str(), content_sha256),
            (AUDIO_CACHE_MANIFEST_SHA256, AUDIO_CACHE_CONTENT_SHA256)
                | (CP1_CACHE_MANIFEST_SHA256, CP1_CACHE_CONTENT_SHA256)
                | (CP2_CACHE_MANIFEST_SHA256, CP2_CACHE_CONTENT_SHA256)
                | (CP3_CACHE_MANIFEST_SHA256, CP3_CACHE_CONTENT_SHA256)
                | (CP4_CACHE_MANIFEST_SHA256, CP4_CACHE_CONTENT_SHA256)
                | (
                    CP4_CORRECTION1_CACHE_MANIFEST_SHA256,
                    CP4_CORRECTION1_CACHE_CONTENT_SHA256
                )
                | (
                    CP4_CORRECTION2_CACHE_MANIFEST_SHA256,
                    CP4_CORRECTION2_CACHE_CONTENT_SHA256
                )
                | (
                    CP4_CORRECTION6_CACHE_MANIFEST_SHA256,
                    CP4_CORRECTION6_CACHE_CONTENT_SHA256
                )
                | (
                    PLAN29_FINAL_CACHE_MANIFEST_SHA256,
                    PLAN29_FINAL_CACHE_CONTENT_SHA256
                )
        );
        if require_string(object, "schema", "owner cache manifest")? != AUDIO_CACHE_SCHEMA
            || require_string(object, "scope", "owner cache manifest")?
                != "PRIVATE_TRACK_P_OWNER_DATA"
            || !qualified_identity
        {
            return Err(NativeWorldError::new(
                "owner cache identity or private scope mismatch",
            ));
        }
        let mut outputs = BTreeMap::new();
        for output in require_array(object, "outputs", "owner cache manifest")? {
            let output = as_object(output, "owner cache output")?;
            let path = require_string(output, "path", "owner cache output")?.to_owned();
            let hash = require_hash_string(output, "sha256", "owner cache output")?.to_owned();
            let bytes = require_value(output, "bytes", "owner cache output")?
                .as_u64()
                .ok_or_else(|| NativeWorldError::new("owner cache output size is not u64"))?;
            if outputs.insert(path, (hash, bytes)).is_some() {
                return Err(NativeWorldError::new(
                    "owner cache manifest has duplicate output paths",
                ));
            }
        }
        Ok(Self {
            root: root.to_path_buf(),
            manifest,
            outputs,
        })
    }

    fn manifest_object(&self) -> Result<&serde_json::Map<String, Value>> {
        as_object(&self.manifest, "owner cache manifest")
    }

    pub(crate) fn read(&self, relative: &str) -> Result<Vec<u8>> {
        let (expected_hash, expected_bytes) = self.outputs.get(relative).ok_or_else(|| {
            NativeWorldError::new(format!("owner cache output is undeclared: {relative}"))
        })?;
        let bytes = read_owner_cache_output(&self.root, relative)?;
        if bytes.len() as u64 != *expected_bytes || sha256_hex(&bytes) != *expected_hash {
            return Err(NativeWorldError::new(format!(
                "owner cache output hash/size mismatch: {relative}"
            )));
        }
        Ok(bytes)
    }
}

#[derive(Debug, Default)]
struct LevelOneCacheAssets {
    sound_asset_refs: BTreeMap<u16, u32>,
    music_asset_ref: Option<u32>,
    goat_tmap_bank: Option<GoatTmapBank>,
    source_sprite_tmap_bank: Option<SourceSpriteTmapBank>,
    castle_stage_one_footprint: Option<CastleBuildFootprint>,
    house_build_footprints: BTreeMap<u8, CastleBuildFootprint>,
}

fn append_level_one_cache_assets(
    root: &Path,
    package_sha256: &str,
    closure_sha256: &str,
    refs: &mut Vec<AssetRef>,
) -> Result<LevelOneCacheAssets> {
    let cache = QualifiedOwnerCacheReader::open(root)?;
    let manifest = cache.manifest_object()?;
    let bindings = require_object(manifest, "bindings", "audio cache manifest")?;
    if require_hash_string(
        bindings,
        "level_export_package_sha256",
        "audio cache bindings",
    )? != package_sha256
        || require_hash_string(
            bindings,
            "level_asset_closure_sha256",
            "audio cache bindings",
        )? != closure_sha256
    {
        return Err(NativeWorldError::new(
            "owner audio cache package/closure binding mismatch",
        ));
    }
    let assets = require_object(manifest, "assets", "audio cache manifest")?;
    let castle_asset = require_object(assets, "level1_castle_stage", "owner cache assets")?;
    require_exact_u64(castle_asset, "build_index", 1, "Castle cache asset")?;
    require_exact_u64(castle_asset, "effective_width", 8, "Castle cache asset")?;
    require_exact_u64(castle_asset, "effective_height", 8, "Castle cache asset")?;
    let castle_descriptor_path = require_string(castle_asset, "path", "Castle cache asset")?;
    let castle_cells_path = require_string(castle_asset, "cells_path", "Castle cache asset")?;
    let house_assets = require_object(assets, "level1_house_footprints", "owner cache assets")?;
    let house_descriptor_path = require_string(house_assets, "path", "house BUILD cache asset")?;
    let house_cells_path = require_string(house_assets, "cells_path", "house BUILD cache asset")?;
    let audio = require_object(assets, "level1_audio", "audio cache assets")?;
    let sound_path = require_string(audio, "sound_descriptor_path", "audio cache Level 1 assets")?;
    let music_path = require_string(audio, "music_descriptor_path", "audio cache Level 1 assets")?;
    let load_output = |path: &str| cache.read(path);
    let source_sprite_tmap_bank = if assets.contains_key("source_frame_census") {
        let terrain_assets = require_object(assets, "terrain_materials", "owner cache assets")?;
        let terrain_path = require_string(terrain_assets, "path", "owner terrain cache asset")?;
        let terrain_bytes = load_output(terrain_path)?;
        let terrain: Value = serde_json::from_slice(&terrain_bytes).map_err(|error| {
            NativeWorldError::new(format!("invalid owner terrain descriptor: {error}"))
        })?;
        let terrain = as_object(&terrain, "owner terrain descriptor")?;
        let tmaps = require_object(terrain, "tmaps1", "owner terrain descriptor")?;
        let mut frames = Vec::new();
        for texture in require_array(tmaps, "textures", "owner TMAP descriptor")? {
            let texture = as_object(texture, "owner TMAP row")?;
            if require_string(texture, "status", "owner TMAP row")? != "decoded_indexed_texture" {
                continue;
            }
            let selector = as_u16(
                require_value(texture, "index", "owner TMAP row")?,
                "owner TMAP selector",
            )?;
            let width = as_u16(
                require_value(texture, "width", "owner TMAP row")?,
                "owner TMAP width",
            )?;
            let height = as_u16(
                require_value(texture, "height", "owner TMAP row")?,
                "owner TMAP height",
            )?;
            let path = require_string(texture, "indices_path", "owner TMAP row")?;
            frames.push(SourceSpriteTmapFrame {
                selector,
                width,
                height,
                pixels: load_output(path)?.into_boxed_slice(),
            });
        }
        let bank = SourceSpriteTmapBank::new(frames).map_err(NativeWorldError::new)?;
        for selector in [54_u16, 64, 80, 81, 138, 141, 151, 463] {
            if bank.frame(selector).is_none() {
                return Err(NativeWorldError::new(format!(
                    "owner source TMAP bank is missing selector {selector}"
                )));
            }
        }
        Some(bank)
    } else {
        None
    };

    let castle_descriptor_bytes = load_output(castle_descriptor_path)?;
    let castle_descriptor: Value =
        serde_json::from_slice(&castle_descriptor_bytes).map_err(|error| {
            NativeWorldError::new(format!("invalid Castle BUILD descriptor: {error}"))
        })?;
    let castle_descriptor = as_object(&castle_descriptor, "Castle BUILD descriptor")?;
    if require_string(castle_descriptor, "schema", "Castle BUILD descriptor")?
        != "mc2-level1-castle-build-footprint/v1"
    {
        return Err(NativeWorldError::new(
            "Castle BUILD descriptor schema mismatch",
        ));
    }
    require_exact_u64(
        castle_descriptor,
        "build_index",
        1,
        "Castle BUILD descriptor",
    )?;
    require_exact_u64(
        castle_descriptor,
        "vga_resolution_selector",
        1,
        "Castle BUILD descriptor",
    )?;
    let dimensions = require_object(castle_descriptor, "dimensions", "Castle BUILD descriptor")?;
    let raw = require_object(dimensions, "raw_tab", "Castle BUILD dimensions")?;
    let runtime = require_object(dimensions, "loader_runtime", "Castle BUILD dimensions")?;
    let effective = require_object(
        dimensions,
        "castle_consumer_effective",
        "Castle BUILD dimensions",
    )?;
    for (row, width, height, label) in [
        (raw, 8, 8, "raw Castle BUILD dimensions"),
        (runtime, 16, 16, "runtime Castle BUILD dimensions"),
        (effective, 8, 8, "effective Castle BUILD dimensions"),
    ] {
        require_exact_u64(row, "width", width, label)?;
        require_exact_u64(row, "height", height, label)?;
    }
    if require_hash_string(raw, "tab_record_sha256", "Castle BUILD raw dimensions")?
        != "27a9da49ded1a0aa1a4e01dbaab8634404f5bf9d6789a5785884e94e35e2f804"
    {
        return Err(NativeWorldError::new(
            "Castle BUILD TAB record identity changed",
        ));
    }
    let cells = require_object(castle_descriptor, "cells", "Castle BUILD descriptor")?;
    if require_string(cells, "path", "Castle BUILD cells")? != castle_cells_path
        || require_hash_string(cells, "sha256", "Castle BUILD cells")?
            != "f9fb9d8a5c6d2841849bba03479e6503966fa4ec14020508c343296602aa8ae3"
    {
        return Err(NativeWorldError::new(
            "Castle BUILD descriptor cell binding changed",
        ));
    }
    require_exact_u64(cells, "bytes", 128, "Castle BUILD cells")?;
    require_exact_u64(cells, "stride_bytes", 2, "Castle BUILD cells")?;
    let castle_cells = load_output(castle_cells_path)?;
    if sha256_hex(&castle_cells)
        != "f9fb9d8a5c6d2841849bba03479e6503966fa4ec14020508c343296602aa8ae3"
    {
        return Err(NativeWorldError::new(
            "Castle BUILD materialized cells changed",
        ));
    }
    let parameters = require_object(
        castle_descriptor,
        "building_parameters",
        "Castle BUILD descriptor",
    )?;
    require_exact_u64(parameters, "record_index", 1, "Castle BUILD parameters")?;
    require_exact_u64(parameters, "word_0", 2, "Castle BUILD parameters")?;
    require_exact_u64(parameters, "flags", 3, "Castle BUILD parameters")?;
    require_exact_u64(parameters, "byte_3", 0, "Castle BUILD parameters")?;
    if require_hash_string(parameters, "record_sha256", "Castle BUILD parameters")?
        != "7d6e4762a22170651b49bef86a5bb9d5de987f5d5f3e546901291fd24c411d39"
    {
        return Err(NativeWorldError::new(
            "Castle BLDGPRM record identity changed",
        ));
    }
    let castle_stage_one_footprint = CastleBuildFootprint::from_owner_cells(1, 8, 8, &castle_cells)
        .map_err(NativeWorldError::new)?;

    let house_descriptor_bytes = load_output(house_descriptor_path)?;
    let house_descriptor: Value =
        serde_json::from_slice(&house_descriptor_bytes).map_err(|error| {
            NativeWorldError::new(format!("invalid house BUILD descriptor: {error}"))
        })?;
    let house_descriptor = as_object(&house_descriptor, "house BUILD descriptor")?;
    if require_string(house_descriptor, "schema", "house BUILD descriptor")?
        != "mc2-level1-house-build-footprints/v1"
    {
        return Err(NativeWorldError::new(
            "house BUILD descriptor schema mismatch",
        ));
    }
    require_exact_u64(
        house_descriptor,
        "vga_resolution_selector",
        1,
        "house BUILD descriptor",
    )?;
    let house_cells_descriptor =
        require_object(house_descriptor, "cells", "house BUILD descriptor")?;
    if require_string(house_cells_descriptor, "path", "house BUILD cells")? != house_cells_path {
        return Err(NativeWorldError::new(
            "house BUILD descriptor cell path mismatch",
        ));
    }
    require_exact_u64(
        house_cells_descriptor,
        "stride_bytes",
        2,
        "house BUILD cells",
    )?;
    let house_cells = load_output(house_cells_path)?;
    if require_hash_string(house_cells_descriptor, "sha256", "house BUILD cells")?
        != sha256_hex(&house_cells)
        || require_value(house_cells_descriptor, "bytes", "house BUILD cells")?.as_u64()
            != Some(house_cells.len() as u64)
    {
        return Err(NativeWorldError::new(
            "house BUILD descriptor cell binding changed",
        ));
    }
    let expected_house_indices = [17_u8, 26, 29, 30, 31, 34, 42, 47, 50, 51, 52, 53];
    let house_rows = require_array(house_descriptor, "footprints", "house BUILD descriptor")?;
    if house_rows.len() != expected_house_indices.len() {
        return Err(NativeWorldError::new(
            "house BUILD descriptor row count changed",
        ));
    }
    let mut house_build_footprints = BTreeMap::new();
    for (row, expected_index) in house_rows.iter().zip(expected_house_indices) {
        let row = as_object(row, "house BUILD row")?;
        let build_index = as_u8(
            require_value(row, "build_index", "house BUILD row")?,
            "house BUILD index",
        )?;
        if build_index != expected_index {
            return Err(NativeWorldError::new(
                "house BUILD descriptor ordering changed",
            ));
        }
        let width = as_u8(
            require_value(row, "width", "house BUILD row")?,
            "house BUILD width",
        )?;
        let height = as_u8(
            require_value(row, "height", "house BUILD row")?,
            "house BUILD height",
        )?;
        let offset = usize::try_from(
            require_value(row, "blob_offset", "house BUILD row")?
                .as_u64()
                .ok_or_else(|| NativeWorldError::new("house BUILD offset is not u64"))?,
        )
        .map_err(|_| NativeWorldError::new("house BUILD offset exceeds usize"))?;
        let length = usize::try_from(
            require_value(row, "bytes", "house BUILD row")?
                .as_u64()
                .ok_or_else(|| NativeWorldError::new("house BUILD length is not u64"))?,
        )
        .map_err(|_| NativeWorldError::new("house BUILD length exceeds usize"))?;
        let end = offset
            .checked_add(length)
            .filter(|end| *end <= house_cells.len())
            .ok_or_else(|| NativeWorldError::new("house BUILD row escapes cell blob"))?;
        let payload = &house_cells[offset..end];
        if length != usize::from(width) * usize::from(height) * 2
            || require_hash_string(row, "sha256", "house BUILD row")? != sha256_hex(payload)
        {
            return Err(NativeWorldError::new(format!(
                "house BUILD row {build_index} binding changed"
            )));
        }
        let footprint = CastleBuildFootprint::from_owner_cells(build_index, width, height, payload)
            .map_err(NativeWorldError::new)?;
        house_build_footprints.insert(build_index, footprint);
    }

    let sound_bytes = load_output(sound_path)?;
    let sound: Value = serde_json::from_slice(&sound_bytes)
        .map_err(|error| NativeWorldError::new(format!("invalid sound descriptor: {error}")))?;
    let sound = as_object(&sound, "sound descriptor")?;
    if require_string(sound, "schema", "sound descriptor")? != "mc2-level1-sound-bank/v1" {
        return Err(NativeWorldError::new("sound descriptor schema mismatch"));
    }
    require_exact_u64(sound, "bank", 1, "sound descriptor")?;
    require_exact_u64(sound, "quality", 822, "sound descriptor")?;
    require_exact_u64(sound, "quality_index", 3, "sound descriptor")?;
    let sound_rows = require_array(sound, "level1_assets", "sound descriptor")?;
    let expected_sound_ids = [
        1_u16, 2, 3, 4, 5, 8, 9, 10, 12, 14, 15, 16, 18, 19, 27, 29, 30, 31, 33, 34, 37, 39, 40,
        41, 43, 44, 46, 54, 55, 56, 57, 63,
    ];
    if sound_rows.len() != expected_sound_ids.len() {
        return Err(NativeWorldError::new(
            "sound descriptor Level 1 mapping count changed",
        ));
    }
    let mut sound_refs = BTreeMap::new();
    for (row, expected_sound_id) in sound_rows.iter().zip(expected_sound_ids) {
        let row = as_object(row, "sound mapping")?;
        let sound_id = as_u16(require_value(row, "sound_id", "sound mapping")?, "sound_id")?;
        if sound_id != expected_sound_id {
            return Err(NativeWorldError::new("sound descriptor ordering changed"));
        }
        let (asset_id, content_hash, mapping_status) = match row.get("asset_path") {
            Some(Value::String(path)) => {
                let payload = load_output(path)?;
                let declared = require_hash_string(row, "asset_sha256", "sound mapping")?;
                if sha256_hex(&payload) != declared {
                    return Err(NativeWorldError::new(
                        "sound mapping does not bind its materialized payload",
                    ));
                }
                (
                    format!("owner_cache:{path}"),
                    declared.to_owned(),
                    "proven_Level1",
                )
            }
            Some(Value::Null) if sound_id == 12 => (
                "owner_cache:audio/sound-bank-1-822.json#sound_id=12".to_owned(),
                require_hash_string(row, "record_sha256", "sound mapping")?.to_owned(),
                "explicit_null",
            ),
            _ => {
                return Err(NativeWorldError::new(
                    "sound mapping has invalid materialization status",
                ));
            }
        };
        if require_string(row, "mapping_status", "sound mapping")? != mapping_status {
            return Err(NativeWorldError::new("sound mapping status changed"));
        }
        let event = json!({
            "category": "audio",
            "hook": "Level1SoundBank",
            "selector": {
                "bank": 1,
                "quality": 822,
                "sound_id": sound_id,
                "mapping_status": mapping_status,
            },
        });
        let event_sha256 = sha256_hex(canonical_json(&event)?.as_bytes());
        let index = u32::try_from(refs.len())
            .map_err(|_| NativeWorldError::new("audio asset-ref index exceeds u32"))?;
        refs.push(AssetRef {
            asset_id,
            content_address: format!("sha256:{content_hash}"),
            owner_asset_ids: vec![format!(
                "owner_data:SOUND/SOUND.DAT#bank=1;quality=822;sound_id={sound_id}"
            )]
            .into_boxed_slice(),
            observation: FrameObservation {
                ordinal: u16::try_from(index + 1)
                    .map_err(|_| NativeWorldError::new("audio ordinal exceeds u16"))?,
                category: "audio".to_owned(),
                hook: "Level1SoundBank".to_owned(),
                stage: Some("session_event".to_owned()),
                selector: event["selector"].clone(),
                event_sha256,
                event,
            },
        });
        sound_refs.insert(sound_id, index);
    }

    let music_bytes = load_output(music_path)?;
    let music: Value = serde_json::from_slice(&music_bytes)
        .map_err(|error| NativeWorldError::new(format!("invalid music descriptor: {error}")))?;
    let music = as_object(&music, "music descriptor")?;
    if require_string(music, "schema", "music descriptor")? != "mc2-level1-music-channel/v1" {
        return Err(NativeWorldError::new("music descriptor schema mismatch"));
    }
    require_exact_u64(music, "channel", 0, "music descriptor")?;
    require_exact_u64(music, "driver_index", 2, "music descriptor")?;
    require_exact_u64(music, "logical_track", 1, "music descriptor")?;
    require_exact_u64(music, "playback_slot", 0, "music descriptor")?;
    require_exact_u64(music, "loop_scan_pointer_slot", 1, "music descriptor")?;
    let wave_path = require_string(music, "unity_wave_path", "music descriptor")?;
    let wave = load_output(wave_path)?;
    let wave_hash = require_hash_string(music, "unity_wave_sha256", "music descriptor")?;
    if sha256_hex(&wave) != wave_hash {
        return Err(NativeWorldError::new(
            "music descriptor does not bind its Unity WAVE",
        ));
    }
    let event = json!({
        "category": "audio",
        "hook": "Level1MusicChannel",
        "selector": {
            "channel": 0,
            "driver_type": "f",
            "logical_track": 1,
            "playback_slot": 0,
            "loop_scan_pointer_slot": 1,
        },
    });
    let event_sha256 = sha256_hex(canonical_json(&event)?.as_bytes());
    let music_index = u32::try_from(refs.len())
        .map_err(|_| NativeWorldError::new("music asset-ref index exceeds u32"))?;
    refs.push(AssetRef {
        asset_id: format!("owner_cache:{wave_path}"),
        content_address: format!("sha256:{wave_hash}"),
        owner_asset_ids: vec![
            "owner_data:SOUND/MUSIC.DAT#channel=0;driver=f;logical_track=1".to_owned(),
        ]
        .into_boxed_slice(),
        observation: FrameObservation {
            ordinal: u16::try_from(music_index + 1)
                .map_err(|_| NativeWorldError::new("music ordinal exceeds u16"))?,
            category: "audio".to_owned(),
            hook: "Level1MusicChannel".to_owned(),
            stage: Some("session_event".to_owned()),
            selector: event["selector"].clone(),
            event_sha256,
            event,
        },
    });
    let terrain_assets = require_object(assets, "terrain_materials", "owner cache assets")?;
    let materials_path = require_string(
        terrain_assets,
        "path",
        "owner cache terrain-material assets",
    )?;
    let materials_bytes = load_output(materials_path)?;
    let materials: Value = serde_json::from_slice(&materials_bytes)
        .map_err(|error| NativeWorldError::new(format!("invalid terrain materials: {error}")))?;
    let materials = as_object(&materials, "terrain materials")?;
    let tmaps1 = require_object(materials, "tmaps1", "terrain materials")?;
    let golden = require_object(tmaps1, "goat_animation_golden", "TMAPS1 materials")?;
    if require_hash_string(golden, "file_sha256", "Goat TMAP golden")?
        != "a5c3d430ba22c3503453124d76119ed7b93ff6ea8445e23cd616faa1015b5f64"
        || require_hash_string(golden, "source_oracle_manifest_sha256", "Goat TMAP golden")?
            != "cad16d830ece1f6422ec4f3f6a0a5ab4954fe6c20344e2e03c6ce1f0bc41e73b"
        || require_string(golden, "selector_state_scope", "Goat TMAP golden")?
            != "selector_global_shared_TMAP_state_not_per_entity"
    {
        return Err(NativeWorldError::new(
            "Goat TMAP golden identity or selector scope changed",
        ));
    }
    let textures = require_array(tmaps1, "textures", "TMAPS1 materials")?;
    let expected_cursors = [
        [490_u32, 818, 1146, 1472, 1812],
        [710_u32, 1196, 1700, 2202, 2724],
        [490_u32, 822, 1152, 1482, 1816],
    ];
    let expected_hashes = [
        [
            "cd5e773ab7c4e4e88536c5a690a8a1794a1b67361b402ab8365e9df3e59a1e9d",
            "502c7e18c327f09578c7ebcd31c351cd2c537bae9482c3f7630a7035be130190",
            "b22359a1aebc1f7afea7e5aa7a4e8c5bdfc5b238ca9a7f162e110470b3350103",
            "2011b3fb136f072ca7dcdae7199b2e8b9dbbdb71bdaa812b1bd659b695c51d12",
            "cd5e773ab7c4e4e88536c5a690a8a1794a1b67361b402ab8365e9df3e59a1e9d",
        ],
        [
            "acea2351981a1fd9aefaf7a2713bc00c4c2b2461dd087b9a2621e7fa3fb819a3",
            "4068c077eb4f66e39ae3bd13202462b477a418aa7781822d09f0feb3337765c7",
            "d5d9ccc838119171cb7ac06315e6e6994e72c058cf40b3b9a9b6fb6709e8931f",
            "f0f656d86c51628b5f0aac90932b79b448f64ef1ef38b41fcffc9d275df71fb1",
            "acea2351981a1fd9aefaf7a2713bc00c4c2b2461dd087b9a2621e7fa3fb819a3",
        ],
        [
            "7d081998c98ba0aa2bb57d4956926f3b8c80b31ae6ee481f8c9174551d6db722",
            "897331a6a567a6f64be563755ba33a5ab1e75e0a1d77d5a1fc9a67ca8657d44a",
            "22d9b6b2bad1a7fef49afcb05284c513f14200852c5cbaad097f684a0197f940",
            "752525ceb8c9a097f0676def3bdeb58258bd093ce7efacf3fe170dc7ec4cbc84",
            "7d081998c98ba0aa2bb57d4956926f3b8c80b31ae6ee481f8c9174551d6db722",
        ],
    ];
    let expected_payload_bytes = [1818_u32, 2730, 1822];
    let expected_state_names = [
        "reset-base",
        "frame-01",
        "frame-02",
        "frame-03",
        "frame-04",
        "closure-frame-01",
    ];
    let mut selector_rows = Vec::with_capacity(GOAT_TMAP_SELECTORS.len());
    for selector_ordinal in 0..GOAT_TMAP_SELECTORS.len() {
        let selector_row = (|| -> Result<_> {
            let expected_selector = GOAT_TMAP_SELECTORS[selector_ordinal];
            let texture = textures
                .iter()
                .find(|texture| {
                    texture
                        .as_object()
                        .and_then(|object| object.get("index"))
                        .and_then(Value::as_u64)
                        == Some(u64::from(expected_selector))
                })
                .ok_or_else(|| {
                    NativeWorldError::new(format!(
                        "Goat TMAP selector {expected_selector} is absent"
                    ))
                })?;
            let texture = as_object(texture, "Goat TMAP texture")?;
            if require_value(texture, "animated", "Goat TMAP texture")?.as_bool() != Some(true)
                || require_exact_u64(texture, "frame_count", 4, "Goat TMAP texture").is_err()
                || require_exact_u64(texture, "header_word", 5123, "Goat TMAP texture").is_err()
                || require_exact_u64(
                    texture,
                    "animation_or_allocation_group",
                    152,
                    "Goat TMAP texture",
                )
                .is_err()
                || require_string(texture, "animation_materialization", "Goat TMAP texture")?
                    != "exact_qualified_Goat_SS2_four_transition_states; selector_global_shared_TMAP_state_not_per_entity"
            {
                return Err(NativeWorldError::new(format!(
                    "Goat TMAP selector {expected_selector} descriptor changed"
                )));
            }
            let width = as_u16(
                require_value(texture, "width", "Goat TMAP texture")?,
                "Goat TMAP width",
            )?;
            let height = as_u16(
                require_value(texture, "height", "Goat TMAP texture")?,
                "Goat TMAP height",
            )?;
            let expected_width = if expected_selector == 153 { 32 } else { 22 };
            if width != expected_width || height != 22 {
                return Err(NativeWorldError::new(format!(
                    "Goat TMAP selector {expected_selector} dimensions changed"
                )));
            }
            let decoded_path =
                require_string(texture, "decoded_payload_path", "Goat TMAP texture")?;
            let decoded_payload = load_output(decoded_path)?;
            if decoded_payload.len() != expected_payload_bytes[selector_ordinal] as usize {
                return Err(NativeWorldError::new(format!(
                    "Goat TMAP selector {expected_selector} payload extent changed"
                )));
            }
            let rows = require_array(texture, "animation_states", "Goat TMAP texture")?;
            if rows.len() != expected_state_names.len() {
                return Err(NativeWorldError::new(format!(
                    "Goat TMAP selector {expected_selector} state count changed"
                )));
            }
            let mut runtime_states = Vec::with_capacity(GOAT_TMAP_STATE_COUNT);
            for (state_ordinal, row) in rows.iter().take(GOAT_TMAP_STATE_COUNT).enumerate() {
                let row = as_object(row, "Goat TMAP animation state")?;
                if require_string(row, "state", "Goat TMAP animation state")?
                    != expected_state_names[state_ordinal]
                    || as_u32(
                        require_value(row, "stream_cursor", "Goat TMAP animation state")?,
                        "Goat TMAP stream cursor",
                    )? != expected_cursors[selector_ordinal][state_ordinal]
                    || as_u8(
                        require_value(row, "frame_index", "Goat TMAP animation state")?,
                        "Goat TMAP frame index",
                    )? != state_ordinal as u8 + 1
                    || as_u16(
                        require_value(row, "width", "Goat TMAP animation state")?,
                        "Goat TMAP state width",
                    )? != width
                    || as_u16(
                        require_value(row, "height", "Goat TMAP animation state")?,
                        "Goat TMAP state height",
                    )? != height
                    || require_hash_string(row, "sha256", "Goat TMAP animation state")?
                        != expected_hashes[selector_ordinal][state_ordinal]
                {
                    return Err(NativeWorldError::new(format!(
                        "Goat TMAP selector {expected_selector} state {state_ordinal} changed"
                    )));
                }
                let path = require_string(row, "indices_path", "Goat TMAP animation state")?;
                let pixels = load_output(path)?;
                let declared_bytes = require_value(row, "bytes", "Goat TMAP animation state")?
                    .as_u64()
                    .ok_or_else(|| {
                        NativeWorldError::new("Goat TMAP state byte count is not u64")
                    })?;
                if pixels.len() as u64 != declared_bytes
                    || sha256_hex(&pixels) != expected_hashes[selector_ordinal][state_ordinal]
                {
                    return Err(NativeWorldError::new(format!(
                        "Goat TMAP selector {expected_selector} state payload changed"
                    )));
                }
                runtime_states.push(GoatTmapFrame {
                    state_ordinal: state_ordinal as u8,
                    frame_index: state_ordinal as u8 + 1,
                    stream_cursor: expected_cursors[selector_ordinal][state_ordinal],
                    width,
                    height,
                    sha256: expected_hashes[selector_ordinal][state_ordinal].to_owned(),
                    pixels: pixels.into_boxed_slice(),
                });
            }
            let closure = as_object(&rows[5], "Goat TMAP closure state")?;
            if require_string(closure, "state", "Goat TMAP closure state")?
                != expected_state_names[5]
                || as_u32(
                    require_value(closure, "stream_cursor", "Goat TMAP closure state")?,
                    "Goat TMAP closure cursor",
                )? != expected_cursors[selector_ordinal][1]
                || as_u8(
                    require_value(closure, "frame_index", "Goat TMAP closure state")?,
                    "Goat TMAP closure frame index",
                )? != 2
                || require_hash_string(closure, "sha256", "Goat TMAP closure state")?
                    != expected_hashes[selector_ordinal][1]
                || require_string(closure, "indices_path", "Goat TMAP closure state")?
                    != require_string(
                        as_object(&rows[1], "Goat TMAP frame-01 state")?,
                        "indices_path",
                        "Goat TMAP frame-01 state",
                    )?
            {
                return Err(NativeWorldError::new(format!(
                    "Goat TMAP selector {expected_selector} closure changed"
                )));
            }
            if runtime_states[4].pixels != runtime_states[0].pixels {
                return Err(NativeWorldError::new(format!(
                    "Goat TMAP selector {expected_selector} frame 4 does not restore reset pixels"
                )));
            }
            let states: [GoatTmapFrame; GOAT_TMAP_STATE_COUNT] = runtime_states
                .try_into()
                .map_err(|_| NativeWorldError::new("Goat TMAP runtime state count changed"))?;
            Ok(GoatTmapSelectorBank {
                selector: expected_selector,
                stream_start: expected_cursors[selector_ordinal][0],
                payload_bytes: expected_payload_bytes[selector_ordinal],
                states,
            })
        })()?;
        selector_rows.push(selector_row);
    }
    let selectors: [GoatTmapSelectorBank; GOAT_TMAP_SELECTORS.len()] = selector_rows
        .try_into()
        .map_err(|_| NativeWorldError::new("Goat TMAP selector count changed"))?;
    let bank = GoatTmapBank::new(selectors).map_err(NativeWorldError::new)?;
    Ok(LevelOneCacheAssets {
        sound_asset_refs: sound_refs,
        music_asset_ref: Some(music_index),
        goat_tmap_bank: Some(bank),
        source_sprite_tmap_bank,
        castle_stage_one_footprint: Some(castle_stage_one_footprint),
        house_build_footprints,
    })
}

fn flight_state_json(state: &FlightState) -> Value {
    json!({
        "entity_index": state.entity_index,
        "position": {
            "x": state.position.x,
            "y": state.position.y,
            "z": state.position.z,
        },
        "yaw": state.yaw,
        "display_pitch": state.display_pitch,
        "effective_pitch": state.effective_pitch,
        "roll_accumulator": state.roll_accumulator,
        "pitch_accumulator": state.pitch_accumulator,
        "target_speed": state.target_speed,
        "speed_changed_flag": state.speed_changed_flag,
        "actual_speed": state.actual_speed,
        "strafe_speed": state.strafe_speed,
        "roll_delta": state.roll_delta,
        "pitch_delta": state.pitch_delta,
        "recoil_speed": state.recoil_speed,
        "recoil_yaw": state.recoil_yaw,
        "impulse_x": state.impulse_x,
        "impulse_y": state.impulse_y,
        "impulse_z": state.impulse_z,
        "entity_fov": state.entity_fov,
        "water_counter": state.water_counter,
        "speed_modifier": state.speed_modifier,
        "speed_modifier_counter": state.speed_modifier_counter,
        "immobilize_counter": state.immobilize_counter,
        "immobilize_subcounter": state.immobilize_subcounter,
        "tether_prior_distance": state.tether_prior_distance,
        "tether_target_index": state.tether_target_index,
        "tether_holder_index": state.tether_holder_index,
        "tether_tier": state.tether_tier,
        "speed_spell_holder_index": state.speed_spell_holder_index,
        "entity_phase": state.entity_phase,
        "ambient_loop_5_distance": state.ambient_loop_5_distance,
        "ambient_loop_31_distance": state.ambient_loop_31_distance,
        "player_color_index": state.player_color_index,
        "music_countdown": state.music_countdown,
        "recovery_latch": state.recovery_latch,
        "skip_flight_once": state.skip_flight_once,
        "tick": state.tick,
    })
}

fn require_mapping(
    mappings: &serde_json::Map<String, Value>,
    name: &str,
    kind: &str,
    file: &str,
    record_name: Option<&str>,
    offset: usize,
    encoding: &str,
) -> Result<()> {
    let mapping = require_object(mappings, name, "field_mappings")?;
    if require_string(mapping, "kind", name)? != kind
        || require_string(mapping, "file", name)? != file
        || require_string(mapping, "encoding", name)? != encoding
    {
        return Err(NativeWorldError::new(format!(
            "reset mapping {name} kind/file/encoding mismatch"
        )));
    }
    require_exact_u64(mapping, "offset", offset as u64, name)?;
    match record_name {
        Some(expected) if require_string(mapping, "record", name)? != expected => {
            return Err(NativeWorldError::new(format!(
                "reset mapping {name} record selector mismatch"
            )));
        }
        None if mapping.contains_key("record") => {
            return Err(NativeWorldError::new(format!(
                "reset mapping {name} has an unexpected record selector"
            )));
        }
        _ => {}
    }
    Ok(())
}

fn require_mapping_scalar(
    mapping: &serde_json::Map<String, Value>,
    file: &str,
    offset: usize,
    encoding: &str,
    name: &str,
) -> Result<()> {
    if require_string(mapping, "file", name)? != file
        || require_string(mapping, "encoding", name)? != encoding
    {
        return Err(NativeWorldError::new(format!(
            "{name} source mapping mismatch"
        )));
    }
    require_exact_u64(mapping, "offset", offset as u64, name)
}

fn mapped_player_i32(
    mappings: &serde_json::Map<String, Value>,
    player: &[u8],
    name: &str,
    offset: usize,
) -> Result<i32> {
    require_mapping(
        mappings,
        name,
        "exported_field",
        "players.bin",
        Some("local_player_slot"),
        offset,
        "i32_le",
    )?;
    read_i32(player, offset, name)
}

fn zero_i64(mappings: &serde_json::Map<String, Value>, name: &str) -> Result<i64> {
    let mapping = require_object(mappings, name, "field_mappings")?;
    if require_string(mapping, "kind", name)? != "zero_at_seam" {
        return Err(NativeWorldError::new(format!(
            "reset mapping {name} must be zero_at_seam"
        )));
    }
    let value = require_value(mapping, "value", name)?
        .as_i64()
        .ok_or_else(|| NativeWorldError::new(format!("{name} seam value is not an integer")))?;
    if value != 0 {
        return Err(NativeWorldError::new(format!(
            "reset mapping {name} seam value is not zero"
        )));
    }
    Ok(value)
}

fn zero_i8(mappings: &serde_json::Map<String, Value>, name: &str) -> Result<i8> {
    i8::try_from(zero_i64(mappings, name)?)
        .map_err(|_| NativeWorldError::new(format!("{name} does not fit i8")))
}

fn zero_i16(mappings: &serde_json::Map<String, Value>, name: &str) -> Result<i16> {
    i16::try_from(zero_i64(mappings, name)?)
        .map_err(|_| NativeWorldError::new(format!("{name} does not fit i16")))
}

fn zero_i32(mappings: &serde_json::Map<String, Value>, name: &str) -> Result<i32> {
    i32::try_from(zero_i64(mappings, name)?)
        .map_err(|_| NativeWorldError::new(format!("{name} does not fit i32")))
}

fn zero_u16(mappings: &serde_json::Map<String, Value>, name: &str) -> Result<u16> {
    u16::try_from(zero_i64(mappings, name)?)
        .map_err(|_| NativeWorldError::new(format!("{name} does not fit u16")))
}

fn zero_bool(mappings: &serde_json::Map<String, Value>, name: &str) -> Result<bool> {
    let mapping = require_object(mappings, name, "field_mappings")?;
    if require_string(mapping, "kind", name)? != "zero_at_seam"
        || require_value(mapping, "value", name)? != &Value::Bool(false)
    {
        return Err(NativeWorldError::new(format!(
            "reset mapping {name} must be false zero_at_seam"
        )));
    }
    Ok(false)
}

fn validate_header(
    bytes: &[u8],
    magic: [u8; 4],
    version: u16,
    record_size: u16,
    count: u32,
    name: &str,
) -> Result<()> {
    if bytes.len() != 12 + count as usize * usize::from(record_size)
        || bytes.get(..4) != Some(magic.as_slice())
        || read_u16(bytes, 4, "component version")? != version
        || read_u16(bytes, 6, "component record size")? != record_size
        || read_u32(bytes, 8, "component count")? != count
    {
        return Err(NativeWorldError::new(format!(
            "{name} header/version/record-size/count/extent mismatch"
        )));
    }
    Ok(())
}

fn relation_one_based(value: u32, maximum: usize, name: &str) -> Result<Option<u16>> {
    if value == 0 {
        Ok(None)
    } else {
        one_based(value, maximum, name).map(Some)
    }
}

fn one_based(value: u32, maximum: usize, name: &str) -> Result<u16> {
    let maximum_u32 = u32::try_from(maximum)
        .map_err(|_| NativeWorldError::new(format!("{name} maximum does not fit u32")))?;
    if value == 0 || value > maximum_u32 {
        Err(NativeWorldError::new(format!(
            "{name} value {value} is outside 1..={maximum}"
        )))
    } else {
        u16::try_from(value - 1)
            .map_err(|_| NativeWorldError::new(format!("{name} does not fit u16")))
    }
}

fn record<'a>(
    bytes: &'a [u8],
    start: usize,
    size: usize,
    index: usize,
    name: &str,
) -> Result<&'a [u8]> {
    let offset = start
        .checked_add(
            index
                .checked_mul(size)
                .ok_or_else(|| NativeWorldError::new(format!("{name} record offset overflow")))?,
        )
        .ok_or_else(|| NativeWorldError::new(format!("{name} record offset overflow")))?;
    bytes
        .get(offset..offset + size)
        .ok_or_else(|| NativeWorldError::new(format!("{name} record {index} is truncated")))
}

fn member<'a>(members: &'a BTreeMap<String, Vec<u8>>, name: &str) -> Result<&'a [u8]> {
    members
        .get(name)
        .map(Vec::as_slice)
        .ok_or_else(|| NativeWorldError::new(format!("package member {name} is unavailable")))
}

fn read_file(path: &Path) -> Result<Vec<u8>> {
    fs::read(path)
        .map_err(|error| NativeWorldError::new(format!("cannot read {}: {error}", path.display())))
}

fn read_u8(bytes: &[u8], offset: usize, name: &str) -> Result<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or_else(|| NativeWorldError::new(format!("{name} is out of bounds")))
}

fn read_i8(bytes: &[u8], offset: usize, name: &str) -> Result<i8> {
    read_u8(bytes, offset, name).map(u8::cast_signed)
}

fn read_u16(bytes: &[u8], offset: usize, name: &str) -> Result<u16> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| NativeWorldError::new(format!("{name} is out of bounds")))?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn read_i16(bytes: &[u8], offset: usize, name: &str) -> Result<i16> {
    read_u16(bytes, offset, name).map(u16::cast_signed)
}

fn read_u32(bytes: &[u8], offset: usize, name: &str) -> Result<u32> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| NativeWorldError::new(format!("{name} is out of bounds")))?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn read_i32(bytes: &[u8], offset: usize, name: &str) -> Result<i32> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| NativeWorldError::new(format!("{name} is out of bounds")))?;
    Ok(i32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn require_value<'a>(
    object: &'a serde_json::Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<&'a Value> {
    object
        .get(name)
        .ok_or_else(|| NativeWorldError::new(format!("{context} is missing {name}")))
}

fn require_object<'a>(
    object: &'a serde_json::Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<&'a serde_json::Map<String, Value>> {
    as_object(require_value(object, name, context)?, name)
}

fn require_array<'a>(
    object: &'a serde_json::Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<&'a Vec<Value>> {
    require_value(object, name, context)?
        .as_array()
        .ok_or_else(|| NativeWorldError::new(format!("{context}.{name} is not an array")))
}

fn require_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<&'a str> {
    require_value(object, name, context)?
        .as_str()
        .ok_or_else(|| NativeWorldError::new(format!("{context}.{name} is not a string")))
}

fn require_hash_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<&'a str> {
    let value = require_string(object, name, context)?;
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(NativeWorldError::new(format!(
            "{context}.{name} is not a SHA-256 hex digest"
        )));
    }
    Ok(value)
}

fn require_exact_u64(
    object: &serde_json::Map<String, Value>,
    name: &str,
    expected: u64,
    context: &str,
) -> Result<()> {
    let actual = require_value(object, name, context)?
        .as_u64()
        .ok_or_else(|| NativeWorldError::new(format!("{context}.{name} is not u64")))?;
    if actual == expected {
        Ok(())
    } else {
        Err(NativeWorldError::new(format!(
            "{context}.{name} must be {expected}, got {actual}"
        )))
    }
}

fn require_exact_i64(
    object: &serde_json::Map<String, Value>,
    name: &str,
    expected: i64,
    context: &str,
) -> Result<()> {
    let actual = require_value(object, name, context)?
        .as_i64()
        .ok_or_else(|| NativeWorldError::new(format!("{context}.{name} is not i64")))?;
    if actual == expected {
        Ok(())
    } else {
        Err(NativeWorldError::new(format!(
            "{context}.{name} must be {expected}, got {actual}"
        )))
    }
}

fn as_object<'a>(value: &'a Value, context: &str) -> Result<&'a serde_json::Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| NativeWorldError::new(format!("{context} is not an object")))
}

fn as_object_mut<'a>(
    value: &'a mut Value,
    context: &str,
) -> Result<&'a mut serde_json::Map<String, Value>> {
    value
        .as_object_mut()
        .ok_or_else(|| NativeWorldError::new(format!("{context} is not an object")))
}

fn as_u8(value: &Value, context: &str) -> Result<u8> {
    value
        .as_u64()
        .and_then(|value| u8::try_from(value).ok())
        .ok_or_else(|| NativeWorldError::new(format!("{context} is not u8")))
}

fn as_u16(value: &Value, context: &str) -> Result<u16> {
    value
        .as_u64()
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| NativeWorldError::new(format!("{context} is not u16")))
}

fn as_u32(value: &Value, context: &str) -> Result<u32> {
    value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| NativeWorldError::new(format!("{context} is not u32")))
}

fn parse_u64_hex(value: &str) -> Result<u64> {
    u64::from_str_radix(value, 16)
        .map_err(|error| NativeWorldError::new(format!("invalid u64 hex hash: {error}")))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn canonical_json(value: &Value) -> Result<String> {
    match value {
        Value::Null => Ok("null".to_owned()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Number(value) => Ok(value.to_string()),
        Value::String(value) => serde_json::to_string(value)
            .map_err(|error| NativeWorldError::new(format!("cannot encode JSON string: {error}"))),
        Value::Array(values) => {
            let encoded = values
                .iter()
                .map(canonical_json)
                .collect::<Result<Vec<_>>>()?;
            Ok(format!("[{}]", encoded.join(",")))
        }
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            let mut fields = Vec::with_capacity(keys.len());
            for key in keys {
                let encoded_key = serde_json::to_string(key).map_err(|error| {
                    NativeWorldError::new(format!("cannot encode JSON key: {error}"))
                })?;
                fields.push(format!("{encoded_key}:{}", canonical_json(&object[key])?));
            }
            Ok(format!("{{{}}}", fields.join(",")))
        }
    }
}

fn validate_content_address(value: &str) -> Result<()> {
    let Some(hash) = value.strip_prefix("sha256:") else {
        return Err(NativeWorldError::new("asset content address is not sha256"));
    };
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(NativeWorldError::new(
            "asset content address has an invalid digest",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_camera::TerrainSlopeCamera;
    use crate::{regression_state::RegressionStateSequence, source_projection_hash};
    use std::path::PathBuf;

    const OWNED_LEVEL_ONE_STATE_SEQUENCE: &[u8] = include_bytes!(
        "../../../../../../upstream/magic-carpet-2-hd/remc2-regression-test/memimages/regressions/level1/sequence-002285FF-00356038.bin"
    );

    fn fixture_paths() -> (PathBuf, PathBuf) {
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        (
            private.join("generated/level1-export-m"),
            private.join("generated/level1-asset-closure.json"),
        )
    }

    fn level_two_fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated/l2-debug3.fJ3PaM/level2")
    }

    #[test]
    fn level_two_common_inspection_uses_typed_profile_and_exact_component_order() {
        let package = level_two_fixture_path();
        let inspection = NativeWorld::inspect_level2(&package).expect("qualified Level 2 package");
        assert_eq!(inspection.profile(), OwnerPackageProfile::Level2);
        assert_eq!(inspection.schema(), OwnerPackageSchema::V3);
        assert_eq!(inspection.metadata().external_level, 2);
        assert_eq!(inspection.metadata().internal_level, 1);
        assert_eq!(inspection.metadata().map_type, MapType::Day);
        assert_eq!(inspection.entities().len(), ENTITY_COUNT);
        assert_eq!(
            inspection
                .entities()
                .iter()
                .filter(|entity| entity.active)
                .count(),
            114
        );
        assert_eq!(inspection.players().len(), PLAYER_COUNT);
        assert_eq!(inspection.runtime().stages.len(), 8);
        assert_eq!(inspection.runtime().stage_variables.len(), 11);
        assert_eq!(inspection.runtime().objectives.len(), 8);
        assert_eq!(inspection.presentation().particles.len(), PARTICLE_COUNT);
        assert_eq!(
            inspection.presentation().level_sprite_selectors.len(),
            LEVEL_SPRITE_SELECTOR_COUNT
        );
        assert_eq!(
            inspection.presentation().entity_parameters.len(),
            ENTITY_PARAMETER_COUNT
        );
        assert_eq!(inspection.entities()[0].index, 0);
        assert_eq!(inspection.entities()[999].index, 999);
        assert_eq!(inspection.players()[0].slot, 0);
        assert_eq!(inspection.players()[7].slot, 7);
        assert_eq!(inspection.players()[0].left_spell_slot_raw, -1);
        assert_eq!(inspection.players()[0].right_spell_slot_raw, -1);
        let reset = inspection.reset_state().expect("Level 2 frame-zero reset");
        assert_eq!(reset.entity_index, 111);
        assert_eq!(
            reset.position,
            Position {
                x: 384,
                y: 62_080,
                z: 256
            }
        );
        assert_eq!(inspection.terrain_rng(), 9_858);
        assert!(inspection.mission_unimplemented().is_err());
    }

    #[test]
    fn level_two_fresh_common_package_without_reset_is_explicitly_inspectable() {
        let source = level_two_fixture_path();
        let root = std::env::temp_dir().join(format!(
            "mc2-level2-fresh-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        fs::create_dir_all(&root).unwrap();
        for (name, _) in LEVEL2_PACKAGE_MEMBERS {
            fs::copy(source.join(name), root.join(name)).unwrap();
        }
        let manifest_path = root.join("manifest.json");
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(source.join("manifest.json")).unwrap()).unwrap();
        manifest.as_object_mut().unwrap().remove("reset_projection");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        let inspection = NativeWorld::inspect_level2(&root).expect("fresh Level 2 package");
        assert_eq!(inspection.metadata().external_level, 2);
        assert_eq!(
            inspection
                .reset_state()
                .expect("Level 2 frame-zero reset")
                .entity_index,
            111
        );
        let error = inspection.mission_unimplemented().unwrap_err();
        assert!(
            error
                .to_string()
                .contains("mission behavior is unsupported")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn level_two_reset_session_matches_authoritative_order_and_isolation() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let template = inspection
            .mission_content_template()
            .expect("Level 2 content template");
        let wizard = template.wizard_map_settings();
        assert_eq!(wizard.len(), 8);
        assert_eq!(
            (
                wizard[0].aggression,
                wizard[0].reflexes,
                wizard[0].perception,
                wizard[0].life
            ),
            (203, 15, 28, 4)
        );
        assert_eq!(
            (
                wizard[1].aggression,
                wizard[1].reflexes,
                wizard[1].perception,
                wizard[1].life
            ),
            (243, 39, 90, 79)
        );
        assert_eq!(&wizard[0].starting_spells[..4], &[1, 1, 1, 1]);
        assert_eq!(&wizard[1].starting_spells[..4], &[1, 1, 0, 1]);
        assert_eq!(&wizard[1].blocked_spells[..5], &[0, 0, 1, 1, 1]);
        assert_eq!(wizard[0].record_bytes().len(), 110);
        let expected_wizard_hashes = [
            "922ab4bf6c34f644190a441cee4a187c37b8732cc5ba89997c8b241d950e7a84",
            "569d4f28bd524b619dfc9eb665f20b4817895cf53d14fe4efe8b4caf7abd250c",
            "a4921745a0b53cc469ea7040da8a3fbad53a04d5493c24f2ebd4dabf9071bc1b",
            "e1fb0004db6b25ec87db76ccbb5771c41864dfbcfd08f802832043d6676172e1",
            "e1fb0004db6b25ec87db76ccbb5771c41864dfbcfd08f802832043d6676172e1",
            "e1fb0004db6b25ec87db76ccbb5771c41864dfbcfd08f802832043d6676172e1",
            "e1fb0004db6b25ec87db76ccbb5771c41864dfbcfd08f802832043d6676172e1",
            "a7a8b0d420d86ff48e6b99a869b7eb0690947089f79e6e9944aecb1bb2a0ba6a",
        ];
        for (settings, expected_hash) in wizard.iter().zip(expected_wizard_hashes) {
            assert_eq!(
                format!("{:x}", Sha256::digest(settings.record_bytes())),
                expected_hash
            );
        }

        let active: Vec<u16> = inspection
            .entities()
            .iter()
            .filter(|entity| entity.active)
            .map(|entity| entity.index)
            .collect();
        assert_eq!(active, (1_u16..=114).collect::<Vec<_>>());
        assert_eq!(
            (
                inspection.entities()[111].class,
                inspection.entities()[111].model
            ),
            (3, 0)
        );
        assert_eq!(
            (
                inspection.entities()[112].class,
                inspection.entities()[112].model
            ),
            (3, 1)
        );
        assert_eq!(
            (
                inspection.entities()[113].class,
                inspection.entities()[113].model,
                inspection.entities()[113].action
            ),
            (15, 0, 0)
        );
        assert_eq!(
            (
                inspection.entities()[114].class,
                inspection.entities()[114].model,
                inspection.entities()[114].action
            ),
            (15, 1, 3)
        );
        assert_eq!(inspection.players()[0].entity_index, 111);
        assert_eq!(inspection.players()[1].entity_index, 112);
        assert_eq!(
            &inspection.players()[1].enabled_spell_holders[..2],
            &[113, 114]
        );

        let mut first = inspection
            .level2_reset_session()
            .expect("Level 2 reset session");
        let mut second = inspection
            .level2_reset_session()
            .expect("independent reset session");
        let state_hash = first.state_hash();
        let terrain_hash = first.terrain_hash();
        let mission_hash = first.mission().state_hash();
        assert_eq!(state_hash, 0x57c3_7985_e90c_696c);
        assert_eq!(terrain_hash, 0xe32f_0658_6f8f_2f8b);
        for _ in 0..19 {
            first.step_outer_turn(PlayerCommand::default(), 0);
            second.step_outer_turn(PlayerCommand::default(), 0);
        }
        assert_eq!(first.state_hash(), second.state_hash());
        assert_eq!(first.terrain_hash(), second.terrain_hash());
        let stepped_state_hash = second.state_hash();
        let stepped_terrain_hash = second.terrain_hash();
        let stepped_mission_hash = second.mission().state_hash();
        first.state.position.x = first.state.position.x.wrapping_add(1);
        first
            .mission
            .set_source_slot_active(1, false)
            .expect("source slot mutation");
        assert_eq!(second.state_hash(), stepped_state_hash);
        assert_eq!(second.terrain_hash(), stepped_terrain_hash);
        assert_eq!(second.mission().state_hash(), stepped_mission_hash);
        first.reset();
        assert_eq!(first.state_hash(), state_hash);
        assert_eq!(first.terrain_hash(), terrain_hash);
        assert_eq!(first.mission().state_hash(), mission_hash);
    }

    #[test]
    fn level_two_economy_projection_tracks_source_order_failure_rng_and_reset() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut first = inspection
            .level2_reset_session()
            .expect("Level 2 reset session");
        let second = inspection
            .level2_reset_session()
            .expect("independent Level 2 reset session");
        let baseline_hash = first.economy().reset_hash();
        let baseline_free_slots = first.economy().free_slot_count();
        assert_eq!(baseline_free_slots, second.economy().free_slot_count());
        assert_eq!(baseline_hash, second.economy().reset_hash());
        let raw_map_entries = raw_source_map_chain_entries(
            &inspection.terrain_planes().entity_indices,
            |source_entity_index| {
                inspection
                    .entities()
                    .get(usize::from(source_entity_index))
                    .map(EntitySnapshot::old_map_entity)
            },
        )
        .expect("validated Level 2 raw map chains");
        let mut expected_linked = raw_map_entries
            .iter()
            .map(|entry| entry.entity_index)
            .collect::<Vec<_>>();
        expected_linked.sort_unstable();
        assert_eq!(
            first.economy().map_chains().linked_entity_indices(),
            expected_linked
        );
        let mut expected_by_tile = BTreeMap::<u16, Vec<u16>>::new();
        for entry in raw_map_entries {
            expected_by_tile
                .entry(entry.tile)
                .or_default()
                .push(entry.entity_index);
        }
        for (tile, expected_chain) in expected_by_tile {
            assert_eq!(
                first
                    .economy()
                    .map_chains()
                    .entity_chain((tile & 0xff) as u8, (tile >> 8) as u8),
                expected_chain
            );
        }

        let sphere_slot = *first.economy().free_slots.last().expect("free sphere slot");
        let seed = first.economy().slot_rng[&sphere_slot];
        let sphere_rng = level2_next_rng16(seed);
        let expected_yaw =
            i16::try_from((i32::from(sphere_rng % 0x71) + i32::from(0x120_i16) - 56) & 0x07ff)
                .expect("sphere yaw");
        let sphere_id = first
            .economy_mut()
            .spawn_mana_sphere_from_death(
                42,
                Position {
                    x: 384,
                    y: 62_080,
                    z: 1_024,
                },
                0x120,
                0x4321,
                321,
                0,
            )
            .expect("sphere allocation");
        assert_eq!(sphere_id, u32::from(sphere_slot));
        let sphere = &first.economy().spheres()[0];
        assert_eq!(sphere.source_entity_index, sphere_slot);
        assert_eq!(sphere.entity_rng, level2_next_rng16(sphere_rng));
        assert_eq!(sphere.yaw, expected_yaw);
        assert_eq!(sphere.mana, 321);
        assert_eq!(sphere.owner_entity_index, 0);
        assert_eq!(first.economy().source_owner(42), 0);
        assert_eq!(
            first.economy().events()[0],
            Level2EconomyEvent::ManaSphereSpawned {
                sphere_id,
                source_entity_index: 42,
                mana: 321,
            }
        );
        assert_eq!(first.economy().free_slot_count(), baseline_free_slots - 1);

        assert!(first.economy_mut().claim_mana_sphere(sphere_slot, 111));
        assert_eq!(
            first.economy().events()[1],
            Level2EconomyEvent::ManaSphereClaimed {
                sphere_id,
                owner_entity_index: 111,
            }
        );
        assert!(!first.economy_mut().claim_mana_sphere(sphere_slot, 111));

        let carrier_slot = first
            .economy_mut()
            .spawn_mana_carrier(
                Position {
                    x: 384,
                    y: 62_080,
                    z: 1_024,
                },
                111,
                sphere_id,
            )
            .expect("carrier allocation");
        let carrier = first.economy().carrier().expect("active carrier");
        assert_eq!(carrier.source_entity_index, carrier_slot);
        assert_eq!(carrier.speed, MANA_CARRIER_SPEED);
        assert_eq!(carrier.life, MANA_CARRIER_MAX_LIFE);
        assert_eq!(carrier.max_mana, MANA_CARRIER_MAX_MANA);
        assert_eq!(
            first.economy().events()[2],
            Level2EconomyEvent::ManaCarrierSpawned {
                source_entity_index: carrier_slot,
                owner_entity_index: 111,
            }
        );
        assert!(
            first
                .economy_mut()
                .absorb_sphere_into_carrier(sphere_slot, carrier_slot)
        );
        assert_eq!(first.economy().spheres()[0].active, false);
        assert_eq!(first.economy().spheres()[0].assigned_carrier_id, 0);
        assert_eq!(
            first.economy().carrier().expect("carrier").carried_mana,
            321
        );
        assert_eq!(
            first.economy().events()[3],
            Level2EconomyEvent::ManaSphereCollected {
                sphere_id,
                carrier_id: u32::from(carrier_slot),
                mana: 321,
            }
        );

        let castle_slot = first
            .economy_mut()
            .build_castle(
                Position {
                    x: 1_024,
                    y: 1_024,
                    z: 0,
                },
                111,
            )
            .expect("castle allocation");
        assert_eq!(
            first.economy().castle().expect("castle").life,
            CASTLE_STAGE_ONE_LIFE
        );
        assert_eq!(
            first.economy().events()[4],
            Level2EconomyEvent::CastleBuilt {
                source_entity_index: castle_slot,
                owner_entity_index: 111,
            }
        );
        assert!(first.economy_mut().return_carrier_to_castle(carrier_slot));
        assert_eq!(first.economy().castle().expect("castle").mana, 0);
        assert!(first.economy().carrier().expect("returning carrier").active);
        let castle_position = first.economy().castle().expect("castle").position;
        let below_ground_far = Position {
            x: castle_position.x.wrapping_add(4_096),
            y: castle_position.y,
            z: i16::MIN,
        };
        let carrier_z_before_failed_move = first.economy().carrier().expect("carrier").position.z;
        assert!(
            !first
                .economy_mut()
                .step_carrier_to_castle(carrier_slot, below_ground_far)
        );
        assert_eq!(
            first
                .economy()
                .carrier()
                .expect("unchanged carrier")
                .position
                .z,
            carrier_z_before_failed_move
        );
        assert!(
            first
                .economy_mut()
                .step_carrier_to_castle(carrier_slot, castle_position)
        );
        assert_eq!(first.economy().castle().expect("castle").mana, 321);
        assert_eq!(
            first
                .economy()
                .carrier()
                .expect("arrived carrier")
                .carried_mana,
            0
        );
        assert_eq!(
            first.economy().events()[5],
            Level2EconomyEvent::ManaCarrierArrived {
                carrier_id: u32::from(carrier_slot),
                castle_id: castle_slot,
                mana: 321,
            }
        );

        assert!(first.economy_mut().submit_damage(castle_slot, 100, 77));
        assert!(first.economy_mut().submit_damage(castle_slot, 200, 77));
        assert_eq!(
            first.economy().castle().expect("castle").pending_damage,
            300
        );
        assert_eq!(first.economy_mut().resolve_damage(castle_slot), Some(false));
        assert_eq!(
            first.economy().castle().expect("castle").life,
            CASTLE_STAGE_ONE_LIFE - 300
        );
        assert_eq!(
            first.economy().events()[6],
            Level2EconomyEvent::DamageApplied {
                target_entity_index: castle_slot,
                attacker_entity_index: 77,
                damage: 300,
                lethal: false,
            }
        );
        let remaining_life = first.economy().castle().expect("castle").life;
        assert!(
            first
                .economy_mut()
                .submit_damage(castle_slot, remaining_life + 1, 88)
        );
        assert_eq!(first.economy_mut().resolve_damage(castle_slot), Some(true));
        let dead_castle = first.economy().castle().expect("castle");
        assert!(dead_castle.active);
        assert_eq!(dead_castle.killer_entity_index, 88);
        assert_eq!(
            first.economy().events()[7],
            Level2EconomyEvent::DamageApplied {
                target_entity_index: castle_slot,
                attacker_entity_index: 88,
                damage: remaining_life + 1,
                lethal: true,
            }
        );
        assert!(first.economy_mut().step_castle_action(castle_slot));
        assert_eq!(first.economy().castle().expect("castle").action, 6);
        assert!(first.economy_mut().step_castle_action(castle_slot));
        let reset_castle = first.economy().castle().expect("reset Castle");
        assert!(!reset_castle.active);
        assert_eq!((reset_castle.action, reset_castle.level), (4, 0));

        first.economy_mut().reclaim_inactive_slots();
        assert_eq!(first.economy().free_slot_count(), baseline_free_slots - 1);
        assert!(first.economy().spheres().is_empty());
        assert!(first.economy().carrier().is_some());
        assert!(first.economy().castle().is_none());

        let failed_hash_before = first.economy().reset_hash();
        first.economy_mut().free_slots.clear();
        assert!(
            first
                .economy_mut()
                .spawn_mana_sphere_from_death(43, Position::default(), 0, 0x9876, 12, 0)
                .is_none()
        );
        assert!(first.economy().spheres().is_empty());
        assert_eq!(first.economy().free_slot_count(), 0);
        assert_ne!(first.economy().reset_hash(), failed_hash_before);
        assert_eq!(first.economy().source_rng.get(&43), Some(&0x9876));

        assert_eq!(second.economy().reset_hash(), baseline_hash);
        first.reset();
        assert_eq!(first.economy().reset_hash(), baseline_hash);
        assert_eq!(first.economy().free_slot_count(), baseline_free_slots);
        assert!(first.economy().spheres().is_empty());
        assert!(first.economy().carrier().is_none());
        assert!(first.economy().castle().is_none());

        let fractional_ids = first.economy_mut().spawn_mana_spheres_from_death(
            44,
            Position::default(),
            0,
            0x2468,
            3_500,
            111,
            true,
        );
        assert_eq!(fractional_ids.len(), 3);
        let fractional_mana = fractional_ids
            .iter()
            .map(|id| {
                first
                    .economy()
                    .spheres()
                    .iter()
                    .find(|sphere| sphere.id == *id)
                    .expect("fractional sphere")
                    .mana
            })
            .collect::<Vec<_>>();
        assert_eq!(fractional_mana, vec![1_166, 1_166, 1_168]);
        assert_eq!(fractional_mana.iter().sum::<i32>(), 3_500);
        assert_eq!(first.economy().events().len(), 3);
        first.reset();
        assert_eq!(first.economy().reset_hash(), baseline_hash);

        let mut merge_session = inspection
            .level2_reset_session()
            .expect("merge projection session");
        let neutral_id = merge_session
            .economy_mut()
            .spawn_mana_sphere_from_death(45, Position::default(), 0, 0x1111, 100, 0)
            .expect("neutral sphere");
        let owned_id = merge_session
            .economy_mut()
            .spawn_mana_sphere_from_death(46, Position::default(), 0, 0x2222, 50, 111)
            .expect("owned sphere");
        merge_session.economy_mut().begin_turn();
        assert!(merge_session.economy_mut().merge_mana_spheres(
            u16::try_from(neutral_id).expect("sphere slot"),
            u16::try_from(owned_id).expect("sphere slot")
        ));
        let merged = merge_session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.id == neutral_id)
            .expect("merged target");
        assert_eq!(merged.owner_entity_index, 111);
        assert_eq!(merged.mana, 150);
        assert!(
            !merge_session
                .economy()
                .spheres()
                .iter()
                .find(|sphere| sphere.id == owned_id)
                .expect("merged source")
                .active
        );
        assert_eq!(
            merge_session.economy().events(),
            &[Level2EconomyEvent::ManaSpheresMerged {
                target_sphere_id: neutral_id,
                source_sphere_id: owned_id,
                owner_entity_index: 111,
                mana: 50,
            }]
        );

        let mut carrier_gates = inspection
            .level2_reset_session()
            .expect("carrier gate session");
        let carrier_position = Position::default();
        let carrier_slot = carrier_gates
            .economy_mut()
            .spawn_mana_carrier(carrier_position, 111, 0)
            .expect("carrier allocation");
        let wrong_owner = carrier_gates
            .economy_mut()
            .spawn_mana_sphere_from_death(47, carrier_position, 0, 0x3333, 1, 112)
            .expect("wrong-owner sphere");
        assert!(!carrier_gates.economy_mut().absorb_sphere_into_carrier(
            u16::try_from(wrong_owner).expect("sphere slot"),
            carrier_slot
        ));
        let far_sphere = carrier_gates
            .economy_mut()
            .spawn_mana_sphere_from_death(
                48,
                Position {
                    x: 2_048,
                    ..carrier_position
                },
                0,
                0x4444,
                1,
                111,
            )
            .expect("far sphere");
        assert!(!carrier_gates.economy_mut().absorb_sphere_into_carrier(
            u16::try_from(far_sphere).expect("sphere slot"),
            carrier_slot
        ));
        let near_non_overlap = carrier_gates
            .economy_mut()
            .spawn_mana_sphere_from_death(
                49,
                Position {
                    x: 400,
                    ..carrier_position
                },
                0,
                0x5555,
                1,
                111,
            )
            .expect("near non-overlap sphere");
        assert!(!carrier_gates.economy_mut().absorb_sphere_into_carrier(
            u16::try_from(near_non_overlap).expect("sphere slot"),
            carrier_slot
        ));
        let near = carrier_gates
            .economy_mut()
            .spawn_mana_sphere_from_death(50, carrier_position, 0, 0x6666, 1, 111)
            .expect("near sphere");
        assert!(
            carrier_gates.economy_mut().absorb_sphere_into_carrier(
                u16::try_from(near).expect("sphere slot"),
                carrier_slot
            )
        );
        assert_eq!(
            carrier_gates
                .economy()
                .carrier()
                .expect("carrier")
                .target_sphere_id,
            0
        );

        let mut castle_lifecycle = inspection
            .level2_reset_session()
            .expect("Castle lifecycle session");
        let lifecycle_slot = castle_lifecycle
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("lifecycle Castle");
        let construction_anchor = Position {
            x: 3_072,
            y: 4_096,
            z: 512,
        };
        assert!(
            castle_lifecycle
                .economy_mut()
                .set_castle_anchor(lifecycle_slot, construction_anchor)
        );
        assert!(
            castle_lifecycle
                .economy_mut()
                .start_castle_construction(lifecycle_slot)
        );
        castle_lifecycle.economy_mut().free_slots.clear();
        assert!(
            !castle_lifecycle
                .economy_mut()
                .step_castle_action(lifecycle_slot)
        );
        let blocked = castle_lifecycle.economy().castle().expect("blocked Castle");
        assert_eq!(
            (blocked.action, blocked.build_state, blocked.level),
            (5, 0, 0)
        );
        castle_lifecycle.reset();
        assert_eq!(
            castle_lifecycle
                .economy()
                .map_chains()
                .linked_entity_indices(),
            expected_linked
        );
        let lifecycle_slot = castle_lifecycle
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("lifecycle Castle retry");
        assert!(
            castle_lifecycle
                .economy_mut()
                .set_castle_anchor(lifecycle_slot, construction_anchor)
        );
        assert!(
            castle_lifecycle
                .economy_mut()
                .start_castle_construction(lifecycle_slot)
        );
        assert!(
            castle_lifecycle
                .economy_mut()
                .step_castle_action(lifecycle_slot)
        );
        let construction = castle_lifecycle
            .economy()
            .castle()
            .expect("construction Castle");
        assert_eq!(
            (
                construction.construction_model,
                construction.construction_action,
                construction.construction_control_flags
            ),
            (42, 44, 1)
        );
        assert_eq!(castle_lifecycle.economy().castle_children().len(), 2);
        assert_eq!(
            castle_lifecycle.economy().castle_children()[0],
            Level2CastleChild {
                source_entity_index: construction.construction_slot,
                id_entity_index: lifecycle_slot,
                parent_entity_index: lifecycle_slot,
                owner_entity_index: 111,
                position: construction_anchor,
                model: 42,
                action: 44,
                control_flags: 1,
                chain_previous_slot: 0,
                chain_next_slot: 0,
                active: true,
            }
        );
        assert!(
            castle_lifecycle
                .economy()
                .map_chains()
                .tracks(construction.construction_slot)
        );
        assert!(
            castle_lifecycle
                .economy()
                .map_chains()
                .is_linked(construction.construction_slot)
        );
        let stage_child = castle_lifecycle
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.active && child.model == 79)
            .expect("source sub_613D0 stage controller");
        assert_eq!(stage_child.parent_entity_index, lifecycle_slot);
        assert_eq!(stage_child.id_entity_index, lifecycle_slot);
        assert_eq!(
            stage_child.position.z,
            castle_lifecycle
                .economy()
                .terrain
                .ground_height(construction.position)
                .saturating_add(384)
        );
        let lifecycle_castle = castle_lifecycle.economy().castle().expect("Castle");
        assert_eq!(lifecycle_castle.controller_chain_count, 1);
        assert_eq!(
            lifecycle_castle.controller_chain_head,
            lifecycle_castle.controller_chain_slots[0]
        );
        assert_eq!(
            lifecycle_castle.controller_chain_tail,
            lifecycle_castle.controller_chain_slots[0]
        );
        assert_eq!(stage_child.chain_previous_slot, lifecycle_slot);
        assert_eq!(stage_child.chain_next_slot, 0);
        assert_eq!(lifecycle_castle.source_class, 3);
        assert_eq!(lifecycle_castle.source_model, 2);
        assert_eq!(lifecycle_castle.source_byte_38, 33);
        assert_eq!(lifecycle_castle.source_particle_parameter, 177);
        assert_eq!(
            castle_lifecycle.economy().player_castle_entity_index(),
            lifecycle_slot
        );
        assert_eq!(castle_lifecycle.economy().player_castle_stage(), 1);
        assert!(
            castle_lifecycle
                .economy_mut()
                .complete_castle_construction(lifecycle_slot)
        );
        assert!(
            castle_lifecycle
                .economy_mut()
                .step_castle_action(lifecycle_slot)
        );
        let built = castle_lifecycle.economy().castle().expect("built Castle");
        assert_eq!((built.action, built.build_state, built.level), (4, 0, 1));

        // Source sub_60810/sub_613D0 stage-two vector: the table updates
        // life/capacity and the controller anchor drops from +384 to +224.
        let mut stage_two = inspection
            .level2_reset_session()
            .expect("stage-two Castle session");
        let stage_two_slot = stage_two
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("stage-two Castle");
        assert!(
            stage_two
                .economy_mut()
                .start_castle_construction(stage_two_slot)
        );
        assert!(stage_two.economy_mut().step_castle_action(stage_two_slot));
        {
            let economy = stage_two.economy_mut();
            let castle = economy.castle.as_mut().expect("stage-two Castle");
            castle.action = 5;
            castle.build_state = 3;
            castle.level = 1;
            for child in &mut economy.castle_children {
                if child.parent_entity_index == stage_two_slot && child.model == 42 {
                    child.active = false;
                }
            }
        }
        assert!(stage_two.economy_mut().step_castle_action(stage_two_slot));
        let stage_two_castle = stage_two.economy().castle().expect("stage-two Castle");
        assert_eq!(
            (
                stage_two_castle.level,
                stage_two_castle.life,
                stage_two_castle.max_life,
                stage_two_castle.mana_capacity
            ),
            (2, 40_000, 40_000, 18_000)
        );
        let stage_two_child = stage_two
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.active && child.model == 79)
            .expect("stage-two controller");
        assert_eq!(stage_two_child.position.z, 224);
        let stage_two_castle = stage_two.economy().castle().expect("stage-two Castle");
        assert_eq!(stage_two_castle.controller_chain_count, 4);
        assert_eq!(
            stage_two_castle.controller_chain_head,
            stage_two_castle.controller_chain_slots[0]
        );
        assert_eq!(
            stage_two_castle.controller_chain_tail,
            stage_two_castle.controller_chain_slots[3]
        );
        let stage_two_chain = stage_two
            .economy()
            .castle_children()
            .iter()
            .filter(|child| child.active && child.model == 79)
            .collect::<Vec<_>>();
        assert_eq!(stage_two_chain.len(), 4);
        for (index, child) in stage_two_chain.iter().enumerate() {
            let expected_previous = if index == 0 {
                stage_two_slot
            } else {
                stage_two_chain[index - 1].source_entity_index
            };
            let expected_next = stage_two_chain
                .get(index + 1)
                .map_or(0, |next| next.source_entity_index);
            assert_eq!(child.chain_previous_slot, expected_previous);
            assert_eq!(child.chain_next_slot, expected_next);
        }

        let mut defense_lifecycle = inspection
            .level2_reset_session()
            .expect("Castle defense lifecycle session");
        let defense_root = defense_lifecycle
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("defense Castle");
        assert!(
            defense_lifecycle
                .economy_mut()
                .set_castle_stage_coefficient(1, 128)
        );
        assert!(
            defense_lifecycle
                .economy_mut()
                .start_castle_construction(defense_root)
        );
        assert!(
            defense_lifecycle
                .economy_mut()
                .step_castle_action(defense_root)
        );
        assert!(
            defense_lifecycle
                .economy_mut()
                .start_castle_defense_child(defense_root)
        );
        assert!(
            defense_lifecycle
                .economy_mut()
                .step_castle_action(defense_root)
        );
        let defense = defense_lifecycle.economy().castle().expect("defense child");
        assert_eq!(
            (
                defense.defense_model,
                defense.defense_action,
                defense.defense_control_flags
            ),
            (41, 43, 0)
        );
        assert_eq!((defense.life, defense.max_life), (10_000, 10_000));
        assert_eq!(defense_lifecycle.economy().castle_children().len(), 3);
        assert_eq!(
            defense_lifecycle
                .economy()
                .castle_children()
                .iter()
                .filter(|child| child.active && child.model == 41)
                .count(),
            1
        );
        assert_eq!(
            defense_lifecycle
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.active && child.model == 41)
                .expect("defense child")
                .position,
            defense_lifecycle
                .economy()
                .castle()
                .expect("defense Castle")
                .anchor_position
        );

        let mut action4_session = inspection
            .level2_reset_session()
            .expect("Castle action-4 session");
        let action4_slot = action4_session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("action-4 Castle");
        let action4_position = action4_session.economy().castle().expect("Castle").position;
        let action4_sphere = action4_session
            .economy_mut()
            .spawn_mana_sphere_from_death(51, action4_position, 0, 0x7777, 100, 111)
            .expect("Castle absorption sphere");
        action4_session.economy_mut().begin_turn();
        assert!(
            action4_session
                .economy_mut()
                .step_castle_action(action4_slot)
        );
        assert_eq!(action4_session.economy().castle().expect("Castle").mana, 0);
        assert_eq!(
            action4_session.economy().castle().expect("Castle").phase,
            (action4_slot as u8).wrapping_add(1)
        );
        assert!(
            action4_session
                .economy_mut()
                .step_castle_action(action4_slot)
        );
        assert_eq!(
            action4_session.economy().castle().expect("Castle").mana,
            100
        );
        assert!(
            !action4_session
                .economy()
                .spheres()
                .iter()
                .find(|sphere| sphere.id == action4_sphere)
                .expect("absorbed sphere")
                .active
        );
        // Source EndOfCastleProjectile calls sub_609E0 before the stage
        // maintenance chain. A non-lethal hit from the root's source ID sets
        // the player byte-195 marker and requests state-0 construction.
        let mut damage_session = inspection
            .level2_reset_session()
            .expect("Castle damage session");
        let damage_slot = damage_session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("damage Castle");
        assert!(
            damage_session
                .economy_mut()
                .submit_damage(damage_slot, 1, damage_slot)
        );
        assert!(damage_session.economy_mut().step_castle_action(damage_slot));
        let damage_transition = damage_session.economy().castle().expect("Castle");
        assert_eq!(
            (damage_transition.action, damage_transition.build_state),
            (5, 0)
        );
        assert_eq!(damage_session.economy().player_castle_aux_flags(), 4);
        assert_ne!(damage_transition.control_flags & 0x40, 0);

        // Source sub_5FD00 releases only overflow above stage capacity. The
        // anchor is deliberately separated from the root to prove the later
        // source-order model-39 scan does not immediately reclaim that sphere.
        let mut release_session = inspection
            .level2_reset_session()
            .expect("Castle release session");
        let release_slot = release_session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("release Castle");
        let release_anchor = Position {
            x: 8_192,
            y: 8_192,
            z: 0,
        };
        assert!(
            release_session
                .economy_mut()
                .set_castle_anchor(release_slot, release_anchor)
        );
        {
            let economy = release_session.economy_mut();
            economy.player_mana = 0;
            let castle = economy.castle.as_mut().expect("release Castle");
            castle.mana = castle.mana_capacity + 1_000;
            // Force the source's even-phase maintenance branch for this
            // vector while retaining the slot-seeded default in production.
            castle.phase &= !1;
        }
        assert!(
            release_session
                .economy_mut()
                .step_castle_action(release_slot)
        );
        let release_castle = release_session.economy().castle().expect("release Castle");
        assert_eq!(release_castle.mana, release_castle.mana_capacity);
        assert!(
            release_session
                .economy()
                .spheres()
                .iter()
                .any(|sphere| sphere.active
                    && sphere.owner_entity_index == 111
                    && sphere.mana == 1_000)
        );

        // When the requested source fraction is larger than the remaining
        // arena, sub_5FD00 recomputes the share from the committed count; it
        // does not silently lose the unreleased remainder.
        let mut partial_release = inspection
            .level2_reset_session()
            .expect("partial Castle release session");
        let partial_slot = partial_release
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("partial release Castle");
        {
            let economy = partial_release.economy_mut();
            economy.player_mana = 0;
            economy.free_slots.truncate(2);
            let castle = economy.castle.as_mut().expect("partial release Castle");
            castle.mana = castle.mana_capacity + 3_500;
            castle.phase &= !1;
        }
        assert!(
            partial_release
                .economy_mut()
                .step_castle_action(partial_slot)
        );
        let partial = partial_release
            .economy()
            .spheres()
            .iter()
            .filter(|sphere| sphere.active && sphere.owner_entity_index == 111)
            .map(|sphere| sphere.mana)
            .collect::<Vec<_>>();
        assert_eq!(partial, vec![1_750, 1_750]);
        assert_eq!(partial.iter().sum::<i32>(), 3_500);
        action4_session
            .economy_mut()
            .castle
            .as_mut()
            .expect("Castle")
            .timer = 2;
        assert!(
            action4_session
                .economy_mut()
                .step_castle_action(action4_slot)
        );
        assert_eq!(action4_session.economy().castle().expect("Castle").timer, 1);
        assert!(
            action4_session
                .economy_mut()
                .step_castle_action(action4_slot)
        );
        let timer_transition = action4_session.economy().castle().expect("Castle");
        assert_eq!(
            (
                timer_transition.action,
                timer_transition.build_state,
                timer_transition.timer
            ),
            (5, 3, 0)
        );
    }

    #[test]
    fn level_two_castle_root_positions_follow_source_grid_and_anchor_vector() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let terrain = SessionTerrain::from_inspection(&inspection);
        let footprint =
            CastleBuildFootprint::from_owner_cells(1, 2, 2, &[0xff, 0xff, 1, 1, 9, 2, 0xff, 0xff])
                .expect("source BUILD vector");
        let projectile = Position {
            x: 0x13_45,
            y: 0x56_78,
            z: 1_234,
        };
        let (root, anchor) = level2_castle_root_positions(&terrain, &footprint, projectile);
        assert_eq!((root.x, root.y), (0x14_00, 0x56_00));
        assert_eq!((anchor.x, anchor.y), (root.x, root.y));
        let center_x = (root.x >> 8) as u8;
        let center_y = (root.y >> 8) as u8;
        let width = footprint.width >> 1;
        let height = footprint.height >> 1;
        let start_x = center_x.wrapping_sub(width / 2);
        let start_y = center_y.wrapping_sub(height / 2);
        let mut minimum = 250_u8;
        for offset in 0..width {
            for (x, y) in [
                (start_x.wrapping_add(offset), start_y),
                (start_x.wrapping_add(offset), start_y.wrapping_add(height)),
            ] {
                minimum = minimum.min(
                    (terrain.ground_height(Position {
                        x: u16::from(x) << 8,
                        y: u16::from(y) << 8,
                        z: 0,
                    }) >> 5) as u8,
                );
            }
        }
        for offset in 0..height {
            for (x, y) in [
                (start_x.wrapping_add(width), start_y.wrapping_add(offset)),
                (start_x, start_y.wrapping_add(offset)),
            ] {
                minimum = minimum.min(
                    (terrain.ground_height(Position {
                        x: u16::from(x) << 8,
                        y: u16::from(y) << 8,
                        z: 0,
                    }) >> 5) as u8,
                );
            }
        }
        assert_eq!(anchor.z, i16::from(minimum) * 32);

        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 placement session");
        let castle_slot = session
            .economy_mut()
            .build_castle_with_footprint(projectile, 111, &footprint)
            .expect("source-derived Castle placement");
        let castle = session.economy().castle().expect("Castle");
        assert_eq!(castle.source_entity_index, castle_slot);
        assert_eq!(castle.position, root);
        assert_eq!(castle.anchor_position, anchor);
        assert_eq!(
            (
                castle.action,
                castle.build_state,
                castle.level,
                castle.life,
                castle.max_life,
                castle.mana_capacity
            ),
            (5, 0, 0, 40_000, 40_000, 0)
        );
        assert_eq!(session.economy().player_castle_entity_index(), 0);
        assert!(session.economy().map_chains().is_linked(castle_slot));
    }

    #[test]
    fn level_two_reset_state_matches_preserved_frame_zero_projection() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../../../../upstream/magic-carpet-2-hd/remc2-regression-test/memimages/regressions",
        );
        let bytes = fs::read(root.join("level2/sequence-002285FF-00356038.bin"))
            .expect("preserved Level 2 regression state");
        let sequence = RegressionStateSequence::new(&bytes).expect("Level 2 regression sequence");
        let frame = sequence.frame(0).expect("Level 2 frame zero");
        let expected = frame.project_player_flight(frame.local_player_slot());
        assert_eq!(inspection.reset_state(), Some(&expected));
    }

    #[test]
    fn level_two_reset_session_matches_all_nineteen_preserved_idle_frames() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../../../../upstream/magic-carpet-2-hd/remc2-regression-test/memimages/regressions",
        );
        let bytes = fs::read(root.join("level2/sequence-002285FF-00356038.bin"))
            .expect("preserved Level 2 regression state");
        let sequence = RegressionStateSequence::new(&bytes).expect("Level 2 regression sequence");
        let player_slot = sequence
            .frame(0)
            .expect("Level 2 frame zero")
            .local_player_slot();
        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 reset session");
        for target_frame in 1..sequence.frame_count() {
            let after = sequence
                .frame(target_frame)
                .expect("preserved Level 2 frame");
            let decoded = after.decoded_flight_input(player_slot);
            decoded.apply(&mut session.state);
            let profile = session.profile();
            crate::step_entity_pass_with_context(
                &mut session.state,
                decoded.command,
                profile,
                &session.terrain,
                &mut session.context,
            );
            let mut expected = after.project_player_flight(player_slot);
            expected.tick = session.state.tick;
            assert_eq!(session.state, expected, "Level 2 frame {target_frame}");
        }
    }

    #[test]
    fn level_two_day_terrain_outdoor_flight_and_camera_use_shared_production_planes() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 reset session");
        let regression_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../../../../upstream/magic-carpet-2-hd/remc2-regression-test/memimages/regressions",
        );
        let terrain_sequence =
            fs::read(regression_root.join("level2/sequence-002285FF-002DC4E0.bin"))
                .expect("preserved Level 2 terrain sequence");
        let frame_bytes = 0x70000;
        assert!(terrain_sequence.len() >= frame_bytes);
        let oracle_frame = &terrain_sequence[..frame_bytes];
        let oracle = LegacyTerrain::from_regression_sequence(&terrain_sequence, 0, false)
            .expect("Level 2 Day terrain oracle");
        let plane =
            |index: usize| &oracle_frame[index * LEGACY_MAP_CELLS..(index + 1) * LEGACY_MAP_CELLS];
        assert_eq!(session.terrain.planes().terrain_types.as_ref(), plane(0));
        assert_eq!(session.terrain.planes().ground_heights.as_ref(), plane(1));
        assert_eq!(session.terrain.planes().shading.as_ref(), plane(2));
        assert_eq!(session.terrain.planes().angles.as_ref(), plane(3));
        // Day does not query the cave ceiling. The authored six-plane reset
        // preserves its secondary-height plane, but that plane is not part of
        // the Day frame-0 comparison (the preserved sequence's plane 4 is not
        // the authored package's secondary-height encoding).
        let expected_entity_indices: Vec<i16> = oracle_frame[5 * LEGACY_MAP_CELLS..]
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        assert_eq!(
            session.terrain.planes().entity_indices.as_ref(),
            expected_entity_indices.as_slice()
        );
        assert_eq!(inspection.metadata().map_type, MapType::Day);
        assert_eq!(session.profile(), FlightProfile::OUTDOOR);
        assert_eq!(session.terrain.planes().terrain_types.len(), 256 * 256);
        assert_eq!(session.terrain.planes().ground_heights.len(), 256 * 256);
        assert_eq!(session.terrain.planes().shading.len(), 256 * 256);
        assert_eq!(session.terrain.planes().angles.len(), 256 * 256);
        assert_eq!(session.terrain.planes().secondary_heights.len(), 256 * 256);
        assert_eq!(session.terrain.planes().entity_indices.len(), 256 * 256);
        assert_eq!(session.terrain.ceiling_height(session.state.position), None);

        // Day uses the same shared terrain queries as Level 1; compare the
        // immutable package terrain against the mutable session projection at
        // non-trivial wrapped map points.
        for position in [
            Position {
                x: 128,
                y: 32_896,
                z: 0,
            },
            Position {
                x: 38_912,
                y: 26_880,
                z: 0,
            },
            Position {
                x: 65_408,
                y: 26_880,
                z: 0,
            },
        ] {
            assert_eq!(
                session.terrain.ground_height(position),
                oracle.ground_height(position)
            );
            assert_eq!(
                session.terrain.terrain_type(position),
                oracle.terrain_type(position)
            );
            assert_eq!(
                session.terrain.material_mask(position),
                oracle.material_mask(position)
            );
        }
        assert_eq!(
            session.terrain.terrain_type(Position {
                x: 128,
                y: 32_896,
                z: 0
            }),
            69
        );
        assert_eq!(session.terrain.planes().ground_heights[0 + 128 * 256], 2);

        let mut camera = TerrainSlopeCamera::default();
        let mut oracle_camera = TerrainSlopeCamera::default();
        assert_eq!(
            camera.update(session.terrain.ground_heights(), 0x0080, 0x8080),
            oracle_camera.update(oracle.ground_heights(), 0x0080, 0x8080)
        );
        assert_eq!(
            camera.update(session.terrain.ground_heights(), 0x0080, 0x8080),
            oracle_camera.update(oracle.ground_heights(), 0x0080, 0x8080)
        );

        // The Level 2 local wizard uses the same outdoor input/flight pass;
        // non-zero forward input is observable in the authoritative state.
        let before = session.state;
        let evidence = session.step_outer_turn(
            PlayerCommand {
                flags: 1,
                ..PlayerCommand::default()
            },
            0,
        );
        assert_eq!(evidence.len(), 1);
        assert_eq!(session.state.target_speed, 16);
        assert_eq!(session.state.speed_changed_flag, 1);
        assert_eq!(session.state.tick, before.tick + 1);
        session.reset();
        assert_eq!(session.state, before);
    }

    #[test]
    fn level_two_bad_stones_reconstruct_action9_map_chain_and_presentation() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 reset session with bad stones");
        let expected_cells = [(2, 203), (2, 103), (51, 154), (208, 154)];
        let expected_hashes = LEVEL2_BAD_STONE_RECORD_SHA256;
        let baseline_hash = session.bad_stone_hash();
        assert_eq!(session.bad_stones().len(), 4);
        assert_eq!(
            session
                .bad_stone_presentation(33)
                .map(|bytes| sha256_hex(bytes)),
            Some("2c3dfa0bc2870d8d1f2c9aa203f9ca6f19363f5411f515627bf43437397f0f78".to_owned())
        );
        assert_eq!(
            session
                .bad_stone_presentation(33)
                .expect("bad-stone presentation"),
            &[
                0, 0, 56, 0, 5, 0, 22, 0, 5, 0, 0, 7, 0, 0, 252, 255, 0, 1, 0, 2, 255, 255, 255,
                255, 0, 12, 50, 0, 0, 16, 0, 2, 0, 0,
            ]
        );
        assert_eq!(
            session.bad_stone_particle_presentation(33),
            Some(&ParticleParameter {
                base_tmap: 198,
                word_2: 8_492,
                word_4: 42,
                speed: 138,
                rotation_speed: 400,
                byte_10: 0,
                byte_11: 1,
                render_mode: 0,
                byte_13: 0,
            })
        );
        assert_eq!(session.bad_stone_sprite_selector(33), Some(0));
        assert_eq!(
            session.bad_stone_sprite_disposition(33).unwrap(),
            SourceSpriteTemplateDisposition::Render(
                crate::source_sprite_tmap::SourceSpriteTemplate {
                    render_parameter_index: 270,
                    base_tmap: 198,
                    render_mode: 0,
                    logical_height: 42,
                    animation_owner:
                        crate::source_sprite_tmap::SourceSpriteAnimationOwner::SourceEntity,
                    palette_owner: crate::source_sprite_tmap::SourceSpritePaletteOwner::NightLevel,
                    shadow_eligible: true,
                    reflection_eligible: true,
                }
            )
        );
        for (ordinal, stone) in session.bad_stones().iter().enumerate() {
            let entity = stone.entity();
            assert_eq!(entity.index, LEVEL2_BAD_STONE_SLOTS[ordinal]);
            assert_eq!((entity.class, entity.model, entity.action), (2, 3, 9));
            assert_eq!(entity.max_life, 300);
            assert_eq!(entity.life, 300);
            assert_eq!(entity.source_word_10(), entity.index % 11);
            assert_eq!(entity.particle_parameter_index, Some(270));
            assert_eq!(entity.parameter_relation, Some(59));
            assert_eq!(
                entity.special_relation,
                EntitySpecialRelation::GlobalSpecialState
            );
            assert_eq!(entity.old_map_entity(), 0);
            assert_eq!(entity.next_entity(), 0);
            assert_eq!(stone.source_record_sha256(), expected_hashes[ordinal]);
            assert_eq!(stone.map_cell(), expected_cells[ordinal]);
            let map_index = terrain_index(stone.map_cell().0, stone.map_cell().1);
            assert_eq!(
                session.terrain.planes().entity_indices[map_index],
                entity.index as i16
            );
            assert_eq!(
                entity.position.z,
                FlightWorld::ground_height(&session.terrain, entity.position)
            );
        }
        let terrain_hash = session.terrain_hash();
        let mission_hash = session.mission().state_hash();
        let baseline_phase = session.bad_stones()[0].entity.phase;
        let expected_z =
            FlightWorld::ground_height(&session.terrain, session.bad_stones()[0].entity.position);
        {
            let stone = &mut session.bad_stones[0];
            stone.entity.position.z = expected_z.wrapping_add(1);
            stone.entity.record[84..86].copy_from_slice(&stone.entity.position.z.to_le_bytes());
            stone.entity.control_flags &= !0x0002_0000;
            stone.entity.record[18] &= !2;
        }
        let step = session
            .step_bad_stone_action_9(33)
            .expect("action-9 source dispatch");
        assert_eq!(step.source_slot, 33);
        assert_eq!(step.position.z, expected_z);
        assert_ne!(step.position.z, step.previous_position.z);
        assert_eq!(step.previous_control_flags & 0x0002_0000, 0);
        assert_ne!(step.control_flags & 0x0002_0000, 0);
        assert_eq!(step.phase, step.previous_phase.wrapping_add(1));
        assert_ne!(session.bad_stone_hash(), baseline_hash);
        assert_eq!(session.terrain_hash(), terrain_hash);
        assert_eq!(session.mission().state_hash(), mission_hash);
        assert!(session.step_bad_stone_action_9(32).is_err());
        session.reset();
        assert_eq!(session.bad_stone_hash(), baseline_hash);
        assert_eq!(session.bad_stones()[0].entity().phase, baseline_phase);
    }

    #[test]
    fn level_two_bad_stone_constructor_rejects_wrong_model_action_parameter_and_cross_level_rows() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let stone = inspection.entities()[33].clone();
        let planes = inspection.terrain_planes();
        let mut wrong_model = stone.clone();
        wrong_model.model = 2;
        assert!(Level2BadStone::from_entity(&wrong_model, planes).is_err());
        let mut wrong_action = stone.clone();
        wrong_action.action = 6;
        assert!(Level2BadStone::from_entity(&wrong_action, planes).is_err());
        let mut wrong_parameter = stone.clone();
        wrong_parameter.parameter_relation = Some(60);
        assert!(Level2BadStone::from_entity(&wrong_parameter, planes).is_err());
        let dolmen = inspection.entities()[108].clone();
        assert!(Level2BadStone::from_entity(&dolmen, planes).is_err());
    }

    #[test]
    fn level_two_ordinary_reset_binds_census_shared_lifecycle_and_isolation() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut first = inspection
            .level2_reset_session()
            .expect("Level 2 ordinary reset session");
        let second = inspection
            .level2_reset_session()
            .expect("independent Level 2 ordinary reset session");
        assert_eq!(first.ordinary().houses().len(), 22);
        assert_eq!(first.ordinary().villagers().len(), 29);
        assert_eq!(first.ordinary().dolmens().len(), 3);
        assert_eq!(
            first
                .ordinary()
                .villagers()
                .iter()
                .map(|villager| villager.entity_index)
                .collect::<Vec<_>>(),
            LEVEL_TWO_VILLAGER_INDICES
        );
        let baseline_hash = first.ordinary().reset_hash();
        assert_eq!(baseline_hash, second.ordinary().reset_hash());

        let house_phase = first.ordinary().houses()[0].phase;
        assert!(first.ordinary_mut().step_house_action_52(1).is_empty());
        assert_eq!(
            first.ordinary().houses()[0].phase,
            house_phase.wrapping_add(1)
        );
        first
            .ordinary_mut()
            .inject_house_damage(1, 1, 111)
            .expect("house damage mailbox");
        assert!(
            first
                .ordinary_mut()
                .step_house_action_52(1)
                .iter()
                .any(|event| matches!(
                    event,
                    HouseStepEvent::Damaged {
                        entity_index: 1,
                        ..
                    }
                ))
        );

        let villager_phase = first.ordinary().villagers()[0].phase;
        assert!(first.ordinary_mut().step_villager_action_105(77).is_empty());
        assert_eq!(
            first.ordinary().villagers()[0].phase,
            villager_phase.wrapping_add(1)
        );
        first
            .ordinary_mut()
            .inject_villager_damage(77, 2_000, 111)
            .expect("villager damage mailbox");
        first.ordinary_mut().step_villager_action_105(77);
        assert_eq!(first.ordinary().villagers()[0].action, 108);
        first.ordinary_mut().step_villager_action_105(77);
        assert!(!first.ordinary().villagers()[0].active);
        assert_ne!(
            first.ordinary().villagers()[0].control_flags & 0x0000_0400,
            0
        );

        let dolmen_before = first.ordinary().dolmens()[0].entity().clone();
        let baseline_flags = first.ordinary().player_interaction_flags();
        first.ordinary_mut().players[0].position = dolmen_before.position;
        let step = first
            .ordinary_mut()
            .step_dolmen_action_6(108)
            .expect("dolmen action 6");
        assert_eq!(step.previous_position, dolmen_before.position);
        assert_eq!(step.previous_control_flags, step.control_flags);
        assert_eq!(step.previous_phase, step.phase);
        assert_eq!(
            first.ordinary().dolmens()[0].entity().control_flags,
            dolmen_before.control_flags
        );
        assert_ne!(first.ordinary().player_interaction_flags(), baseline_flags);
        first.ordinary_mut().reset();
        assert_eq!(first.ordinary().reset_hash(), baseline_hash);
        assert_eq!(first.ordinary().player_interaction_flags(), baseline_flags);
        let dolmen_position = first.ordinary().dolmens()[0].entity().position;
        first.ordinary_mut().players[0].position = Position {
            x: dolmen_position.x.wrapping_add(0x4000),
            y: dolmen_position.y,
            z: dolmen_position.z,
        };
        let far_flags = first.ordinary().player_interaction_flags();
        first
            .ordinary_mut()
            .step_dolmen_action_6(108)
            .expect("far dolmen action 6");
        assert_eq!(first.ordinary().player_interaction_flags(), far_flags);
        first.ordinary_mut().reset();
        first.ordinary_mut().players[0].position = dolmen_position;
        first.ordinary_mut().players[0].life = -1;
        let dead_player_flags = first.ordinary().player_interaction_flags();
        first
            .ordinary_mut()
            .step_dolmen_action_6(108)
            .expect("dead-player dolmen action 6");
        assert_eq!(
            first.ordinary().player_interaction_flags(),
            dead_player_flags
        );
        assert_eq!(second.ordinary().reset_hash(), baseline_hash);
    }

    #[test]
    fn level_two_shared_spells_bind_activation_mana_events_and_reset() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut first = inspection
            .level2_reset_session()
            .expect("Level 2 reset session");
        let second = inspection
            .level2_reset_session()
            .expect("isolated Level 2 reset session");
        assert_eq!(
            first
                .shared_spells()
                .pickups()
                .iter()
                .map(|entity| entity.index)
                .collect::<Vec<_>>(),
            vec![38, 75]
        );
        assert_eq!(
            first
                .shared_spells()
                .holders()
                .iter()
                .map(|entity| entity.index)
                .collect::<Vec<_>>(),
            vec![113, 114]
        );
        assert_eq!(first.shared_spells().player_mana(), 1_000);
        assert!(first.shared_spells_mut().activate_fireball(256, true));
        assert_eq!(first.shared_spells().fireball_holder().active_value, 5);
        assert_eq!(first.shared_spells().activation_events(), &[]);
        assert!(first.shared_spells_mut().activate_possession(512, true));
        assert_eq!(first.shared_spells().possession_holder().active_value, 3);
        first.shared_spells_mut().reset();
        assert!(first.shared_spells_mut().activate_speed(256, true));
        assert_eq!(
            first.shared_spells().speed_holder().active_value,
            i16::try_from(SPEED_SPELL_ACTIVATION_TICKS).expect("speed timer fits i16")
        );
        first.shared_spells_mut().reset();
        assert_eq!(second.shared_spells().fireball_holder().active_value, 0);
        first.shared_spells_mut().caster.mana = 0;
        first.shared_spells_mut().fireball.active_value = 0;
        assert!(!first.shared_spells_mut().activate_fireball(256, true));
        assert!(!first.shared_spells().activation_events().is_empty());
        first.shared_spells_mut().reset();
        assert_eq!(first.shared_spells().fireball_holder().active_value, 0);
        assert_eq!(first.shared_spells().player_mana(), 1_000);
        assert_eq!(first.shared_spells(), second.shared_spells());
    }

    #[test]
    fn level_two_speed_pickup_converts_in_source_order_and_rejects_excluded_family() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut first = inspection
            .level2_reset_session()
            .expect("Level 2 reset session");
        let second = inspection
            .level2_reset_session()
            .expect("isolated Level 2 reset session");
        let pickup_position = first.shared_spells().pickups()[1].position;
        let baseline_hash = first.shared_spells().reset_hash();

        let step = first
            .shared_spells_mut()
            .step_speed_pickup(111, pickup_position)
            .expect("Speed pickup source dispatch");
        assert!(step.collected);
        assert_eq!(step.source_slot, 75);
        assert_eq!(step.previous_action, 10);
        assert_eq!(step.action, 9);
        assert_eq!(step.phase, step.previous_phase.wrapping_add(1));
        assert_eq!(first.shared_spells().pickups()[1].action, 9);
        let collected_pickup = &first.shared_spells().pickups()[1];
        assert_eq!(
            u16::from_le_bytes([
                collected_pickup.record_bytes()[44],
                collected_pickup.record_bytes()[45]
            ]),
            111
        );
        assert_ne!(collected_pickup.control_flags & 1, 0);
        assert_eq!(collected_pickup.record_bytes()[19] & 2, 0);
        assert_eq!(
            i16::from_le_bytes([
                collected_pickup.record_bytes()[58],
                collected_pickup.record_bytes()[59]
            ]),
            64
        );
        assert_eq!(first.shared_spells().speed_holder().entity_index, 75);
        assert_eq!(
            first.shared_spells().speed_pickup_state(),
            Level2SpeedPickupState {
                owner_entity_index: 111,
                collected: true,
                visible: false,
                state_timer: 64,
            }
        );
        assert_ne!(first.shared_spells().reset_hash(), baseline_hash);
        assert_eq!(
            first.shared_spells().pickup_events(),
            &[Level2SharedSpellEvent::SpellCollected {
                source_slot: 75,
                player_entity_index: 111,
                family: SPEED_SPELL_FAMILY,
            }]
        );
        assert!(matches!(
            first.shared_spells().activation_events(),
            [FlightEvent::PrepareSound {
                sound_id: 18,
                entity_index: 111,
                player_index: 0,
                ..
            }]
        ));

        // A second source turn sees the converted holder, so no duplicate
        // acquisition or second sound is emitted.
        let second_step = first
            .shared_spells_mut()
            .step_speed_pickup(111, pickup_position)
            .expect("converted holder source dispatch");
        assert!(!second_step.collected);
        assert!(first.shared_spells().activation_events().is_empty());

        first.shared_spells_mut().reset();
        let far_position = Position {
            x: pickup_position.x.wrapping_add(0x2000),
            y: pickup_position.y,
            z: pickup_position.z,
        };
        let non_overlap = first
            .shared_spells_mut()
            .step_speed_pickup(111, far_position)
            .expect("non-overlapping source dispatch");
        assert!(!non_overlap.collected);
        assert_eq!(first.shared_spells().pickups()[1].action, 10);
        first.shared_spells_mut().reset();
        first.shared_spells_mut().pickups[1].phase = 77;
        first.shared_spells_mut().pickups[1].record[66] = 77;
        let direct_update = first
            .shared_spells_mut()
            .step_speed_pickup(111, pickup_position)
            .expect("non-cadence source dispatch");
        assert!(!direct_update.collected);
        assert_eq!(first.shared_spells().pickups()[1].action, 10);

        // The model-12 row is a later Level 2 family, not a Level 1 Speed
        // pickup; this bounded adapter never silently coerces it.
        let mut excluded = inspection
            .level2_reset_session()
            .expect("Level 2 reset session");
        let slot38 = excluded.shared_spells().pickups()[0].position;
        let excluded_step = excluded
            .shared_spells_mut()
            .step_shared_pickup(38, 111, slot38)
            .expect("Speed source dispatch remains well formed");
        assert!(excluded_step.is_none());
        assert_eq!(excluded.shared_spells().pickups()[0].action, 37);

        first.shared_spells_mut().reset();
        assert_eq!(first.shared_spells(), second.shared_spells());
        assert!(
            first
                .shared_spells_mut()
                .step_shared_pickup(38, 112, pickup_position)
                .is_err()
        );
        assert!(
            first
                .shared_spells_mut()
                .step_speed_pickup(112, pickup_position)
                .is_err()
        );
    }

    #[test]
    fn level_two_shared_spell_census_rejects_tampered_rows_and_presentation() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let players = inspection.players();
        let mut entities = inspection.entities().to_vec();
        entities[38].record[68] = 3;
        assert!(
            Level2SharedSpellResetSession::from_parts(
                SessionTerrain::from_inspection(&inspection),
                &entities,
                players,
                inspection.presentation(),
            )
            .is_err()
        );
        let mut entities = inspection.entities().to_vec();
        entities[113].record[28] ^= 1;
        assert!(
            Level2SharedSpellResetSession::from_parts(
                SessionTerrain::from_inspection(&inspection),
                &entities,
                players,
                inspection.presentation(),
            )
            .is_err()
        );
        let mut entities = inspection.entities().to_vec();
        entities[75].record[73] = 9;
        assert!(
            Level2SharedSpellResetSession::from_parts(
                SessionTerrain::from_inspection(&inspection),
                &entities,
                players,
                inspection.presentation(),
            )
            .is_err()
        );
        let mut presentation = inspection.presentation().clone();
        presentation.level_sprite_selectors[59] = 0;
        assert!(
            Level2SharedSpellResetSession::from_parts(
                SessionTerrain::from_inspection(&inspection),
                inspection.entities(),
                players,
                &presentation,
            )
            .is_err()
        );
    }

    #[test]
    fn level_two_ordinary_census_rejects_wrong_rows_hashes_and_presentation() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut entities = inspection.entities().to_vec();
        let presentation = inspection.presentation().clone();
        entities[77].action = 106;
        assert!(validate_level2_ordinary_census(&entities, &presentation).is_err());
        entities = inspection.entities().to_vec();
        entities[1].record[74] ^= 1;
        assert!(validate_level2_ordinary_census(&entities, &presentation).is_err());
        entities = inspection.entities().to_vec();
        entities[108].parameter_relation = Some(60);
        assert!(Level2Dolmen::from_entity(&entities[108], inspection.terrain_planes()).is_err());
        entities = inspection.entities().to_vec();
        entities[1].model = 44;
        assert!(HouseState::from_level_two_snapshot(&entities[1]).is_err());
        entities = inspection.entities().to_vec();
        entities[88] = entities[77].clone();
        assert!(
            VillagerFamily::from_level_two_entities(&entities, VillagerBehaviorConfig::LEVEL_TWO)
                .is_err()
        );
        let mut wrong_presentation = presentation;
        wrong_presentation.level_sprite_selectors[79] = 1;
        assert!(
            validate_level2_ordinary_census(&inspection.entities, &wrong_presentation).is_err()
        );
        let mut wrong_house_presentation = inspection.presentation().clone();
        wrong_house_presentation.level_sprite_selectors[96] = 0;
        assert!(
            validate_level2_ordinary_census(&inspection.entities, &wrong_house_presentation)
                .is_err()
        );
        let mut left = inspection.entities()[108].clone();
        let mut right = inspection.entities()[111].clone();
        left.position.x = u16::MAX - 5;
        right.position.y = left.position.y;
        right.position.z = left.position.z;
        right.position.x = 2;
        for entity in [&mut left, &mut right] {
            for (offset, value) in [(86, 16_i16), (88, 16), (90, 0), (92, 16)] {
                entity.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
            }
        }
        assert!(level2_source_intersects(&left, &right));
        right.position.x = 32;
        assert!(!level2_source_intersects(&left, &right));
    }

    #[test]
    fn level_two_profile_rejects_cross_level_relabeling_and_level_one_loader() {
        let package = level_two_fixture_path();
        let root = std::env::temp_dir().join(format!(
            "mc2-level2-profile-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        fs::create_dir_all(&root).unwrap();
        let manifest_path = root.join("manifest.json");
        fs::copy(package.join("manifest.json"), &manifest_path).unwrap();
        for (name, _) in LEVEL2_PACKAGE_MEMBERS {
            fs::copy(package.join(name), root.join(name)).unwrap();
        }
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        manifest["level"]["external"] = json!(1);
        manifest["level"]["internal"] = json!(0);
        manifest["level"]["map_type"] = json!({"id": 1, "name": "Night"});
        let mut identity = Vec::new();
        for (name, expected_size) in LEVEL2_PACKAGE_MEMBERS {
            let bytes = fs::read(root.join(name)).unwrap();
            assert_eq!(bytes.len(), expected_size);
            let hash = sha256_hex(&bytes);
            identity.extend_from_slice(name.as_bytes());
            identity.push(0);
            identity.extend_from_slice(hash.as_bytes());
            identity.push(0);
            identity.extend_from_slice(expected_size.to_string().as_bytes());
            identity.push(b'\n');
        }
        manifest["package_sha256"] = json!(sha256_hex(&identity));
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        assert!(NativeWorld::inspect_level2(&root).is_err());
        let (_, closure) = fixture_paths();
        assert!(NativeWorld::load(&root, closure).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn level_one_v2_rejects_level_two_extra_members() {
        let (package, closure) = fixture_paths();
        let temp = TempPackage::copy_from(&package, &closure);
        let l2 = level_two_fixture_path();
        fs::copy(
            l2.join("feature-start.bin"),
            temp.package.join("feature-start.bin"),
        )
        .unwrap();
        let manifest_path = temp.package.join("manifest.json");
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        let l2_manifest: Value =
            serde_json::from_slice(&fs::read(l2.join("manifest.json")).unwrap()).unwrap();
        manifest["files"]["feature-start.bin"] = l2_manifest["files"]["feature-start.bin"].clone();
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        let error = NativeWorld::load(&temp.package, &temp.closure).unwrap_err();
        assert!(error.to_string().contains("exact typed package member set"));
    }

    #[test]
    fn production_speed_sweeps_preserve_the_complete_quiet_reset_rng_schedule() {
        use crate::level1_mission::GlobalRngSite;

        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("validated package M");
        for (speed_index, expected_passes) in [(0_u8, 1_u16), (1, 4), (2, 8)] {
            let mut session = world.session();
            session.mission.clear_global_rng_trace();
            let evidence = session.step_with_speed_index(PlayerCommand::default(), speed_index);
            let trace = session.mission.global_rng_trace();
            assert_eq!(evidence.len(), usize::from(expected_passes));
            assert_eq!(trace.len(), usize::from(expected_passes) * 10);
            for pass_index in 0..expected_passes {
                let pass = &trace[usize::from(pass_index) * 10..usize::from(pass_index + 1) * 10];
                assert_eq!(
                    pass.iter()
                        .map(|entry| entry.pass_index)
                        .collect::<Vec<_>>(),
                    vec![pass_index; 10],
                    "no draw may escape or enter scheduled pass {pass_index}"
                );
                assert_eq!(
                    pass.iter()
                        .map(|entry| entry.source_slot)
                        .collect::<Vec<_>>(),
                    [0_u16, 113, 114, 115, 116, 117, 118, 119, 120, 121]
                );
                assert_eq!(pass[0].site, GlobalRngSite::UpdateEntities);
                assert!(
                    pass[1..]
                        .iter()
                        .all(|entry| entry.site == GlobalRngSite::Model60Smoke)
                );
            }
            assert_eq!(trace[0].before, 0xad6e_67d9);
            assert_eq!(
                trace[..10]
                    .iter()
                    .map(|entry| entry.after)
                    .collect::<Vec<_>>(),
                [
                    0x9909_f858,
                    0xa631_b437,
                    0x829a_3776,
                    0xd2c9_9e15,
                    0xeb07_8414,
                    0xde4d_0573,
                    0xa334_be32,
                    0x0eea_ca51,
                    0x661c_c5d0,
                    0x43e9_ccaf,
                ]
            );
            for (index, entry) in trace.iter().enumerate() {
                assert_eq!(
                    entry.after,
                    entry.before.wrapping_mul(9_377).wrapping_add(9_439)
                );
                if let Some(next) = trace.get(index + 1) {
                    assert_eq!(next.before, entry.after);
                }
            }
            assert_eq!(
                (
                    session.mission.objective_presentation_palette_mod,
                    session.mission.objective_presentation_phase,
                    session.mission.objective_textbox_ticks,
                ),
                (2, 1, 0),
                "speed index {speed_index} advances presentation once, not once per entity pass"
            );
            session.step_with_speed_index(PlayerCommand::default(), speed_index);
            assert_eq!(
                (
                    session.mission.objective_presentation_palette_mod,
                    session.mission.objective_presentation_phase,
                    session.mission.objective_textbox_ticks,
                ),
                (3, 0, 200),
                "speed index {speed_index} services initial objective once on the second outer frame"
            );
        }
    }

    #[test]
    fn package_m_loads_the_exact_native_world() {
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load_without_owner_cache(package, closure)
            .expect("validated cache-free package M");
        assert_eq!(world.metadata.map_type, MapType::Night);
        assert_eq!(world.profile, FlightProfile::OUTDOOR);
        assert_eq!(
            world.terrain.ceiling_height(world.reset_state.position),
            None
        );
        assert_eq!(world.entities.len(), 1_000);
        assert_eq!(
            world.entities.iter().filter(|entity| entity.active).count(),
            177
        );
        assert_eq!(
            world
                .entities
                .iter()
                .filter(|entity| {
                    entity.active
                        && entity.control_flags & crate::level1_mission::SOURCE_REPLACEABLE_FLAG
                            != 0
                })
                .count(),
            31
        );
        assert_eq!(world.players.len(), 8);
        assert_eq!(world.asset_refs.len(), 76);
        assert_eq!(world.runtime.objective_presentation_phase, 1);
        assert_eq!(world.runtime.objective_textbox_ticks, 0);
        assert_eq!(world.runtime.objectives.len(), 8);
        assert_eq!(
            world.runtime.objectives[0],
            RuntimeObjective {
                is_level_end: 0,
                text_selector: 0,
                done: 0,
                stage_selectors: [1, 1, 1, 1, 1, 0, 0, 0],
            }
        );
        assert_eq!(
            world.package_sha256(),
            "730d2fa084265df839f3f2b55ed766ac167a8b152ca4631b5ded9d0bb9104ac8"
        );
        assert_eq!(
            world.closure_sha256(),
            "878123908f6de05f684f4cd1d73cff8d2ff08701cb6fdd12b7165bfcacc75719"
        );
        assert_eq!(state_hash(world.reset_state()), 0x9da4_bc91_af58_f3ce);
        assert_eq!(
            conformance_hash(world.reset_state(), world.reset_context()),
            0x77b9_6b82_492d_6f4b
        );
        assert_eq!(
            world.reset_state.position,
            Position {
                x: 19_840,
                y: 56_960,
                z: 5_024,
            }
        );
        assert_eq!(world.reset_state.entity_phase, 153);
        assert_eq!(world.reset_state.ambient_loop_5_distance, 2_048);
        assert_eq!(world.reset_state.ambient_loop_31_distance, 1_746);
        assert_eq!(
            world.presentation.particles[usize::from(MANA_CARRIER_RENDER_INDEX)],
            ParticleParameter {
                base_tmap: MANA_CARRIER_BASE_TMAP,
                word_2: 8_492,
                word_4: 42,
                speed: 545,
                rotation_speed: MANA_CARRIER_LOGICAL_HEIGHT,
                byte_10: 0,
                byte_11: -16,
                render_mode: MANA_CARRIER_RENDER_MODE,
                byte_13: 0,
            }
        );
        let (castle_blockers, building_blockers) = world.level1_mission.castle_placement_blockers();
        assert!(castle_blockers.is_empty());
        assert_eq!(building_blockers.len(), 47);
        let expected_buildings = world
            .entities
            .iter()
            .filter(|entity| entity.active && entity.class == 10 && entity.model == 45);
        for (blocker, entity) in building_blockers.iter().zip(expected_buildings) {
            assert_eq!(*blocker, entity.castle_placement_blocker());
        }
        let possession_targets = world.level1_mission.possession_targets();
        assert_eq!(possession_targets.len(), 41);
        assert!(possession_targets.iter().all(|target| {
            let entity = &world.entities[usize::from(target.entity_index)];
            !matches!(entity.level_gate, 17 | 51 | 52 | 53) && *target == entity.collision_target()
        }));

        let mut first_session = world.session();
        let second_session = world.session();
        let reset_lighting_hash = first_session.mission.dynamic_lighting_hash_v1();
        assert_eq!(
            (
                first_session.mission.dynamic_light_active_count(),
                second_session.mission.dynamic_light_active_count(),
                reset_lighting_hash,
            ),
            (0, 0, second_session.mission.dynamic_lighting_hash_v1()),
            "each production session starts from the same complete 50-slot lighting domain"
        );
        first_session
            .mission
            .spawn_death_burst_from_death(
                first_session.state.entity_index,
                first_session.state.position,
            )
            .expect("production death-burst light admission");
        assert_eq!(first_session.mission.dynamic_light_active_count(), 1);
        assert_eq!(
            (
                first_session.mission.dynamic_lights()[0].intensity,
                first_session.mission.dynamic_lights()[0].jitter_range,
                first_session.mission.dynamic_lights()[0].life_scaled,
            ),
            (
                crate::level1_mission::PLAYER_DEATH_BURST_DYNAMIC_LIGHT_RADIUS,
                crate::level1_mission::PLAYER_DEATH_BURST_DYNAMIC_LIGHT_KIND,
                true,
            )
        );
        assert_ne!(
            first_session.mission.dynamic_lighting_hash_v1(),
            reset_lighting_hash
        );
        assert_eq!(
            second_session.mission.dynamic_lighting_hash_v1(),
            reset_lighting_hash,
            "one production session cannot mutate another session's lighting RNG/history"
        );
        first_session.step(PlayerCommand::default());
        assert_eq!(
            first_session.mission.dynamic_light_active_count(),
            0,
            "production dispatch expires the one-tick burst before the post-update light service"
        );
        assert_eq!(
            first_session.mission.dynamic_lights()[0].history,
            [0; crate::dynamic_lighting::DYNAMIC_LIGHT_HISTORY_CELLS],
            "the production subtract/update/service schedule retains no stale one-tick burst footprint"
        );
        first_session.reset();
        assert_eq!(
            (
                first_session.mission.dynamic_light_active_count(),
                first_session.mission.dynamic_lighting_hash_v1(),
            ),
            (0, reset_lighting_hash),
            "production reset restores RNG, count, inactive payloads, and retained histories"
        );

        let scheduled = [0_u8, 1, 2].map(|speed_index| {
            let mut session = world.session();
            assert_eq!(
                session.mission.admit_linked_dynamic_light_for_test(
                    session.state.entity_index,
                    session.state.position,
                ),
                Some(0)
            );
            let evidence = session.step_with_speed_index(
                PlayerCommand {
                    flags: 1,
                    ..PlayerCommand::default()
                },
                speed_index,
            );
            assert_eq!(
                session.mission.dynamic_lighting_rng_for_test(),
                0x84ca_7cd2,
                "one live light consumes exactly one RNG draw after the final source pass"
            );
            assert_eq!(
                session.mission.dynamic_lights()[0].position,
                session.state.position,
                "light service must observe the final player position after every full pass"
            );
            (
                evidence.len(),
                session.state.position,
                session.state.target_speed,
                session.state.actual_speed,
                session.state.tick,
                session.mission.source_turn_counter(),
            )
        });
        assert_eq!(
            scheduled,
            [
                (
                    1,
                    Position {
                        x: 19_840,
                        y: 56_944,
                        z: 5_024,
                    },
                    16,
                    16,
                    1,
                    2,
                ),
                (
                    4,
                    Position {
                        x: 19_840,
                        y: 56_800,
                        z: 5_028,
                    },
                    64,
                    64,
                    4,
                    5,
                ),
                (
                    8,
                    Position {
                        x: 19_840,
                        y: 56_480,
                        z: 5_040,
                    },
                    80,
                    80,
                    8,
                    9,
                ),
            ],
            "production 1/4/8 full source sweeps must preserve exact final state and position"
        );

        let mut threshold_world = world.clone();
        let threshold_goat_index = usize::from(LEVEL_ONE_GOAT_INDICES[0]);
        let initial_player_position = threshold_world.reset_state.position;
        let threshold_goat_position = Position {
            x: initial_player_position.x,
            y: initial_player_position.y.wrapping_sub(6_144),
            z: initial_player_position.z,
        };
        let mut threshold_goat = threshold_world.entities[threshold_goat_index]
            .clone()
            .with_record_byte(0x003d, 0)
            .with_record_byte(0x003e, 0);
        threshold_goat.position = threshold_goat_position;
        threshold_goat.action = 0;
        threshold_goat.control_flags &= !1;
        threshold_world.entities[threshold_goat_index] = threshold_goat;
        threshold_world.reset_live_world = LiveWorldState::from_entities(
            &threshold_world.entities,
            threshold_world.goat_stage_point,
            threshold_world.goat_config,
        )
        .expect("threshold-crossing production Goat family");
        assert_eq!(
            i32::from(initial_player_position.y)
                .wrapping_sub(i32::from(threshold_goat_position.y))
                .pow(2),
            0x0240_0000,
            "the outer-turn starting player position is exactly outside the strict Goat threshold"
        );
        assert!(
            i32::from(56_944_u16)
                .wrapping_sub(i32::from(threshold_goat_position.y))
                .pow(2)
                < 0x0240_0000,
            "pass-one movement crosses into the strict Goat threshold"
        );
        let threshold_crossings = [1_u8, 2].map(|speed_index| {
            let mut threshold_session = threshold_world.session();
            threshold_session.step_with_speed_index(
                PlayerCommand {
                    flags: 1,
                    ..PlayerCommand::default()
                },
                speed_index,
            );
            let threshold_goat = threshold_session
                .live_world
                .goats()
                .iter()
                .find(|goat| goat.entity_index == LEVEL_ONE_GOAT_INDICES[0])
                .expect("threshold Goat remains live");
            (
                threshold_session.state.position,
                threshold_goat.position,
                threshold_goat.proximity_countdown,
                threshold_goat.proximity_subcounter,
            )
        });
        assert_eq!(
            threshold_crossings,
            [
                (
                    Position {
                        x: 19_840,
                        y: 56_800,
                        z: 5_028,
                    },
                    threshold_goat_position,
                    14,
                    0,
                ),
                (
                    Position {
                        x: 19_840,
                        y: 56_480,
                        z: 5_040,
                    },
                    threshold_goat_position,
                    10,
                    0,
                ),
            ],
            "speed-index 1/2 later passes must observe pass-one player movement at Goat begin-turn proximity"
        );
    }

    #[test]
    fn owner_level_one_has_a_qualified_castle_site_near_the_stage_three_route() {
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("validated package M");
        let (castles, buildings) = world.level1_mission.castle_placement_blockers();
        let former_site = Position {
            x: 50_048,
            y: 51_584,
            z: 1_200,
        };
        let qualified_site = Position {
            x: 49_920,
            y: 43_776,
            z: 1_200,
        };
        assert!(!crate::level1_mission::castle_placement_valid(
            &world,
            former_site,
            castles,
            buildings
        ));
        assert!(crate::level1_mission::castle_placement_valid(
            &world,
            qualified_site,
            castles,
            buildings
        ));
    }

    #[test]
    fn qualified_level_one_castle_build_reaches_every_owner_height_transactionally() {
        let (package, closure) = fixture_paths();
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load_with_owner_cache(
            package,
            closure,
            private.join("generated/level1-unity-asset-cache"),
        )
        .expect("validated package M and owner cache");
        let guard_x = 20_u8;
        let guard_y = 21_u8;
        let mut unconditional = world.terrain_planes().clone();
        let mut signed = unconditional.clone();
        for (x, y, angle) in [
            (guard_x, guard_y, 0x80),
            (guard_x.wrapping_sub(1), guard_y, 0x81),
            (guard_x.wrapping_sub(1), guard_y.wrapping_sub(1), 0x82),
            (guard_x, guard_y.wrapping_sub(1), 0x83),
        ] {
            let index = terrain_index(x, y);
            unconditional.terrain_types[index] = 42;
            unconditional.angles[index] = angle;
            signed.terrain_types[index] = 42;
            signed.angles[index] = angle;
        }
        let mut unconditional_rng = 9_775;
        let mut signed_rng = 9_775;
        add_building_to_terrain(
            &mut unconditional,
            guard_x,
            guard_y,
            MapType::Night,
            &mut unconditional_rng,
        );
        apply_level1_load_path_point(&mut signed, guard_x, guard_y, &mut signed_rng);
        for (x, y) in [
            (guard_x, guard_y),
            (guard_x.wrapping_sub(1), guard_y),
            (guard_x.wrapping_sub(1), guard_y.wrapping_sub(1)),
            (guard_x, guard_y.wrapping_sub(1)),
        ] {
            let index = terrain_index(x, y);
            assert_ne!(
                unconditional.terrain_types[index], 42,
                "AddBuildingToTerrain must mark bit-0x80 cell ({x},{y})"
            );
            assert_eq!(
                signed.terrain_types[index], 42,
                "sub_462A0 must preserve bit-0x80 cell ({x},{y})"
            );
        }
        let footprint = world
            .level1_mission()
            .castle_stage_one_footprint()
            .expect("qualified BUILD[1]");
        let candidate = Position {
            x: 49_920,
            y: 43_776,
            z: 1_200,
        };
        let mut root = Position {
            x: candidate.x & 0xff00,
            y: candidate.y & 0xff00,
            z: world.ground_height(candidate),
        };
        if ((root.x >> 8) + (root.y >> 8)) & 1 != 0 {
            root.x = root.x.wrapping_add(256);
        }
        let start_x = (root.x >> 8) as u8 - footprint.width / 2;
        let start_y = (root.y >> 8) as u8 - footprint.height / 2;
        let mut base = u8::MAX;
        for offset in 0..footprint.width {
            for (x, y) in [
                (start_x.wrapping_add(offset), start_y),
                (
                    start_x.wrapping_add(offset),
                    start_y.wrapping_add(footprint.height),
                ),
            ] {
                base = base.min(world.terrain_planes().ground_heights[terrain_index(x, y)]);
            }
        }
        for offset in 0..footprint.height {
            for (x, y) in [
                (
                    start_x.wrapping_add(footprint.width),
                    start_y.wrapping_add(offset),
                ),
                (start_x, start_y.wrapping_add(offset)),
            ] {
                base = base.min(world.terrain_planes().ground_heights[terrain_index(x, y)]);
            }
        }
        let anchor = Position {
            z: i16::from(base) * 32,
            ..root
        };
        let mut terrain = SessionTerrain::from_world(&world);
        let unchanged = terrain.clone();
        let error = terrain
            .apply_castle_update(
                CastleTerrainUpdate::ConstructionPass {
                    position: anchor,
                    remaining: 0,
                },
                footprint,
            )
            .expect_err("a malformed construction timer must fail closed");
        assert!(error.contains("outside 1..=18"));
        assert_eq!(
            terrain, unchanged,
            "a rejected update must not mutate planes, RNG, or sequence"
        );

        let mut first_session = world.session();
        let mut second_session = world.session();
        first_session
            .terrain
            .apply_castle_update(
                CastleTerrainUpdate::ConstructionPass {
                    position: anchor,
                    remaining: 18,
                },
                footprint,
            )
            .expect("qualified first construction pass");
        assert_ne!(first_session.terrain, second_session.terrain);
        assert_eq!(second_session.terrain.mutation_sequence(), 0);
        first_session.reset();
        second_session.reset();
        assert_eq!(first_session.terrain, second_session.terrain);
        assert_eq!(first_session.terrain, SessionTerrain::from_world(&world));

        for remaining in (1..=18).rev() {
            terrain
                .apply_castle_update(
                    CastleTerrainUpdate::ConstructionPass {
                        position: anchor,
                        remaining,
                    },
                    footprint,
                )
                .expect("qualified Level 1 footprint does not require an unsupported topology");
        }
        terrain
            .apply_castle_update(
                CastleTerrainUpdate::ConstructionCompleted { position: anchor },
                footprint,
            )
            .expect("qualified completion");
        for row in 0..footprint.height {
            for column in 0..footprint.width {
                let cell = footprint.cells()
                    [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
                assert_ne!(cell[1], 0xff);
                assert_eq!(
                    terrain.planes().ground_heights
                        [terrain_index(start_x.wrapping_add(column), start_y.wrapping_add(row))],
                    base.wrapping_add(cell[1])
                );
            }
        }
        assert_eq!(terrain.terrain_rng(), 37_256);
        assert_eq!(terrain.mutation_sequence(), 19);
        assert_eq!(
            sha256_hex(&terrain.planes().terrain_types),
            "6cf4397c008d4ea16e37340bbe819f6d58235e61f8d669260ee9f0ac41e94e16"
        );
        assert_eq!(
            sha256_hex(&terrain.planes().ground_heights),
            "7eff800ac2a60ccf3314bd917408c65a11c1a5cbe59b52d9670b4ed76b3c4533"
        );
        assert_eq!(
            sha256_hex(&terrain.planes().shading),
            "ad91ad8c250c0cd4e3bd9248ae9fa7bf95cb72d056921e7f0dbff03c7fdf7ec7"
        );
        assert_eq!(
            sha256_hex(&terrain.planes().angles),
            "79962c82610ea6abf727a11f82f5351b69355915f11aba9738e4eaca8d18484e"
        );
        terrain
            .apply_castle_update(
                CastleTerrainUpdate::StageDestroyed {
                    position: anchor,
                    entity_rng: 100,
                },
                footprint,
            )
            .expect("qualified Level 1 teardown");
        assert_eq!(terrain.terrain_rng(), 33_028);
        assert_eq!(terrain.mutation_sequence(), 20);
        assert_eq!(
            sha256_hex(&terrain.planes().terrain_types),
            "5793a3375673e55b74dd3b62159ff6b239241035a3100d599987350b6faa24ae"
        );
        assert_eq!(
            sha256_hex(&terrain.planes().ground_heights),
            "50f3a45214f435ac3b8423254a69b7cc87b80a8039085c25a0fe2033eed1ecba"
        );
        assert_eq!(
            sha256_hex(&terrain.planes().shading),
            "f1a63a2c818a1e16f8e781dd8ef0777833b5194ac8f69a604723408c23e486a8"
        );
        assert_eq!(
            sha256_hex(&terrain.planes().angles),
            "4c72d5ae2beee363ffa2b51ef0b181bcd8c8d5f05ac4a801735ec2722aad3891"
        );
        assert_eq!(
            sha256_hex(&terrain.planes().secondary_heights),
            "de2f256064a0af797747c2b97505dc0b9f3df0de4f489eac731c23ae9ca9cc31"
        );
        assert_eq!(
            terrain.planes().entity_indices,
            world.terrain_planes().entity_indices
        );

        let mut ambient = SessionTerrain::from_world(&world);
        let material_x = 200_u8;
        let material_y = 201_u8;
        for (x, y) in [
            (material_x, material_y),
            (material_x.wrapping_add(1), material_y),
            (material_x.wrapping_add(1), material_y.wrapping_add(1)),
            (material_x, material_y.wrapping_add(1)),
        ] {
            ambient.planes.ground_heights[terrain_index(x, y)] = 50;
            ambient.planes.angles[terrain_index(x, y)] = 0;
        }
        ambient
            .apply_castle_update(
                CastleTerrainUpdate::AmbientMaterial {
                    position: Position {
                        x: u16::from(material_x) << 8,
                        y: u16::from(material_y) << 8,
                        z: 1_600,
                    },
                    in_type: 2,
                    target_type: 20,
                },
                footprint,
            )
            .expect("source action-0 terrain material update");
        assert_eq!(
            ambient.planes.terrain_types[terrain_index(material_x, material_y)],
            TERRAIN_GENERATION_TABLE_PREFIX[0x18]
        );
        assert_eq!(
            ambient.planes.angles[terrain_index(material_x, material_y)],
            (TERRAIN_GENERATION_TABLE_PREFIX[0x19] | 0x80) & 0xf7
        );

        let collision_x = 210_u8;
        let collision_y = 211_u8;
        for (x, y) in [
            (collision_x, collision_y),
            (collision_x.wrapping_add(1), collision_y),
            (collision_x.wrapping_add(1), collision_y.wrapping_add(1)),
            (collision_x, collision_y.wrapping_add(1)),
        ] {
            ambient.planes.ground_heights[terrain_index(x, y)] = 100;
            ambient.planes.angles[terrain_index(x, y)] = 0;
        }
        ambient
            .apply_castle_update(
                CastleTerrainUpdate::AmbientCollision {
                    position: Position {
                        x: u16::from(collision_x) << 8,
                        y: u16::from(collision_y) << 8,
                        z: 3_200,
                    },
                    height_delta: -3,
                },
                footprint,
            )
            .expect("source action-0 SEARCH terrain deformation");
        for (x, y) in [
            (collision_x, collision_y),
            (collision_x.wrapping_add(1), collision_y),
            (collision_x, collision_y.wrapping_add(1)),
            (collision_x.wrapping_add(1), collision_y.wrapping_add(1)),
        ] {
            assert_eq!(ambient.planes.ground_heights[terrain_index(x, y)], 97);
        }
        assert_eq!(ambient.mutation_sequence(), 2);
    }

    #[test]
    fn qualified_level_one_house_footprints_drive_transactional_action_53_terrain() {
        let (package, closure) = fixture_paths();
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load_with_owner_cache(
            package,
            closure,
            private.join("generated/level1-unity-asset-cache"),
        )
        .expect("validated package M and owner cache");
        let expected = [
            (17, 48, 48),
            (26, 9, 11),
            (29, 13, 11),
            (30, 13, 15),
            (31, 23, 11),
            (34, 22, 22),
            (42, 11, 11),
            (47, 11, 11),
            (50, 7, 7),
            (51, 4, 12),
            (52, 12, 4),
            (53, 8, 8),
        ];
        for (index, width, height) in expected {
            let footprint = world
                .house_build_footprint(index)
                .expect("qualified Level 1 house BUILD row");
            assert_eq!(
                (footprint.index, footprint.width, footprint.height),
                (index, width, height)
            );
            assert_eq!(
                footprint.cells().len(),
                usize::from(width) * usize::from(height)
            );
        }

        let house = &world.entities()[11];
        assert_eq!(house.level_gate, 29);
        let footprint = world
            .house_build_footprint(house.level_gate.cast_unsigned())
            .expect("house 11 BUILD[29]");
        let mut terrain = SessionTerrain::from_world(&world);
        let before = terrain.deterministic_hash();
        let outcome = terrain
            .apply_house_removal(house.position, house.entity_rng, 2, true, footprint)
            .expect("action-53 terrain mutation");
        assert_eq!(outcome.final_occupants, 0);
        assert_eq!(outcome.final_rng, house.entity_rng);
        assert_eq!(
            outcome
                .residents
                .iter()
                .map(|resident| (resident.model, resident.action))
                .collect::<Vec<_>>(),
            [(4, 33), (12, 97)]
        );
        assert_eq!(terrain.mutation_sequence(), 1);
        assert_ne!(terrain.deterministic_hash(), before);

        let mut session = world.session();
        let empty_mission_before = session.mission.clone();
        let empty_live_world_before = session.live_world.clone();
        let empty_global_rng = session.mission.global_rng();
        let empty_events = session.mission.events().to_vec();
        let empty_arena_hash = session.mission.source_arena_hash_v1();
        let empty_live_hash = session.live_world_hash_v2();
        let actor_storage = session.mission.actors().as_ptr();
        let goat_storage = session.live_world.goats().as_ptr();
        assert!(
            apply_level1_source_effects(&mut session.mission, &mut session.live_world, &[],)
                .expect("empty effect batch is an exact no-op")
                .is_empty()
        );
        assert_eq!(session.mission, empty_mission_before);
        assert_eq!(session.live_world, empty_live_world_before);
        assert_eq!(session.mission.global_rng(), empty_global_rng);
        assert_eq!(session.mission.events(), empty_events);
        assert_eq!(session.mission.source_arena_hash_v1(), empty_arena_hash);
        assert_eq!(session.live_world_hash_v2(), empty_live_hash);
        assert_eq!(
            session.mission.actors().as_ptr(),
            actor_storage,
            "empty production dispatch must not clone mission storage"
        );
        assert_eq!(
            session.live_world.goats().as_ptr(),
            goat_storage,
            "empty production dispatch must not clone live-world storage"
        );

        let resident_position = session.live_world.houses()[0].position;
        let resident_slot = session
            .mission
            .spawn_house_resident(resident_position, 4, 33, 0)
            .expect("resident constructor")
            .expect("qualified source arena has a resident slot");
        let occupants_before = session.live_world.houses()[0].occupants;
        let live_hash_before = session.live_world_hash_v2();
        let arena_hash_before = session.mission.source_arena_hash_v1();
        let updates = apply_level1_source_effects(
            &mut session.mission,
            &mut session.live_world,
            &[Level1SourceEffect::TryEnterResident {
                resident_source_entity_index: resident_slot,
                house_source_entity_index: 1,
            }],
        )
        .expect("atomic resident entry effect");
        assert_eq!(updates.len(), 1);
        assert_eq!(
            session.live_world.houses()[0].occupants,
            occupants_before + 1,
            "successful entry increments the sole house authority exactly once"
        );
        assert!(!session.mission.dynamic_residents().next().unwrap().active);
        assert_ne!(session.live_world_hash_v2(), live_hash_before);
        assert_ne!(session.mission.source_arena_hash_v1(), arena_hash_before);

        while session.live_world.houses()[0].occupants
            < i32::from(session.live_world.houses()[0].capacity)
        {
            assert_eq!(
                session.live_world.try_enter_resident(1),
                HouseResidentEntry::Entered
            );
        }
        let full_resident = session
            .mission
            .spawn_house_resident(resident_position, 4, 33, 0)
            .expect("resident constructor")
            .expect("second resident slot");
        let full_live_hash = session.live_world_hash_v2();
        let full_arena_hash = session.mission.source_arena_hash_v1();
        assert!(
            apply_level1_source_effects(
                &mut session.mission,
                &mut session.live_world,
                &[Level1SourceEffect::TryEnterResident {
                    resident_source_entity_index: full_resident,
                    house_source_entity_index: 1,
                }],
            )
            .expect("full resident entry rejection")
            .is_empty()
        );
        assert_eq!(session.live_world_hash_v2(), full_live_hash);
        assert_eq!(session.mission.source_arena_hash_v1(), full_arena_hash);
        assert!(
            session
                .mission
                .dynamic_residents()
                .find(|resident| resident.source_entity_index == full_resident)
                .is_some_and(|resident| resident.active)
        );

        let mut dead_session = world.session();
        let lethal = dead_session.live_world.houses()[0].life + 1;
        dead_session
            .live_world
            .inject_damage(1, lethal, 152)
            .expect("lethal house damage");
        let _ = dead_session
            .live_world
            .step_house_source_slot(1, &dead_session.terrain);
        let _ = dead_session
            .live_world
            .step_house_source_slot(1, &dead_session.terrain);
        assert!(!dead_session.live_world.houses()[0].active);
        let dead_resident = dead_session
            .mission
            .spawn_house_resident(resident_position, 4, 33, 0)
            .expect("resident constructor")
            .expect("dead-house resident slot");
        let dead_live_hash = dead_session.live_world_hash_v2();
        let dead_arena_hash = dead_session.mission.source_arena_hash_v1();
        assert!(
            apply_level1_source_effects(
                &mut dead_session.mission,
                &mut dead_session.live_world,
                &[Level1SourceEffect::TryEnterResident {
                    resident_source_entity_index: dead_resident,
                    house_source_entity_index: 1,
                }],
            )
            .expect("dead resident entry rejection")
            .is_empty()
        );
        assert_eq!(dead_session.live_world_hash_v2(), dead_live_hash);
        assert_eq!(dead_session.mission.source_arena_hash_v1(), dead_arena_hash);
    }

    #[test]
    fn direct_session_action_51_applies_source_ordered_synthetic_build_planes() {
        let (package, closure) = fixture_paths();
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let mut world = NativeWorld::load_with_owner_cache(
            package,
            closure,
            private.join("generated/level1-unity-asset-cache"),
        )
        .expect("validated package M and owner cache");
        let footprint =
            CastleBuildFootprint::from_owner_cells(29, 2, 2, &[8, 4, 0xff, 0xff, 9, 2, 1, 6])
                .expect("synthetic action-51 BUILD");
        world.house_build_footprints.insert(29, footprint);
        let mut session = world.session();
        let position = Position {
            x: 100 << 8,
            y: 100 << 8,
            z: 20 * 32,
        };
        let start_x = 99_u8;
        let start_y = 99_u8;
        for row in 0..4_u8 {
            for column in 0..4_u8 {
                let index = terrain_index(
                    start_x.wrapping_sub(1).wrapping_add(column),
                    start_y.wrapping_sub(1).wrapping_add(row),
                );
                session.terrain.planes.ground_heights[index] = 10;
                session.terrain.planes.terrain_types[index] = 42;
                session.terrain.planes.angles[index] = 0;
                session.terrain.planes.shading[index] = 0;
            }
        }
        let mut house = session
            .live_world
            .houses()
            .iter()
            .find(|house| house.entity_index == 11)
            .copied()
            .expect("house 11");
        house.position = position;
        house.building_index = 29;
        house.action = crate::house::HOUSE_TERRAIN_TO_LIVE_ACTION;
        house.max_life = 6;
        house.life = 6;
        house.owner_entity_index = session.state.entity_index;
        house.control_flags |= 1;
        session.live_world.initialize_builder_house(house).unwrap();
        let frozen_source_views =
            level1_turn_start_source_views(&session.mission, &session.state, &session.live_world);

        let sample = |terrain: &SessionTerrain| {
            [(0_u8, 0_u8), (0, 1), (1, 1)].map(|(column, row)| {
                let index = terrain_index(start_x.wrapping_add(column), start_y.wrapping_add(row));
                (
                    terrain.planes.ground_heights[index],
                    terrain.planes.terrain_types[index],
                    terrain.planes.angles[index],
                    terrain.planes.shading[index],
                )
            })
        };
        assert_eq!(sample(&session.terrain), [(10, 42, 0, 0); 3]);

        let first = session
            .live_world
            .step_house_source_slot(house.entity_index, &session.terrain);
        assert!(matches!(
            first.as_slice(),
            [HouseStepEvent::ConstructionPass { remaining: 5, .. }]
        ));
        assert!(session.process_house_events(&first, &frozen_source_views));
        let mid = sample(&session.terrain);
        assert_eq!(mid, [(12, 8, 145, 35), (12, 9, 129, 32), (13, 37, 1, 30)]);

        for expected_remaining in (1..=4).rev() {
            let events = session
                .live_world
                .step_house_source_slot(house.entity_index, &session.terrain);
            assert!(matches!(
                events.as_slice(),
                [HouseStepEvent::ConstructionPass { remaining, .. }]
                    if *remaining == expected_remaining
            ));
            assert!(session.process_house_events(&events, &frozen_source_views));
        }
        let before_completion = sample(&session.terrain);
        assert_eq!(before_completion.map(|cell| cell.0), [24, 22, 26]);

        let completed = session
            .live_world
            .step_house_source_slot(house.entity_index, &session.terrain);
        assert!(matches!(
            completed.as_slice(),
            [HouseStepEvent::ConstructionCompleted { .. }]
        ));
        assert!(session.process_house_events(&completed, &frozen_source_views));
        let final_planes = sample(&session.terrain);
        assert_eq!(
            final_planes,
            [(24, 8, 145, 36), (22, 9, 129, 32), (26, 37, 1, 18)]
        );
        let live = session
            .live_world
            .houses()
            .iter()
            .find(|house| house.entity_index == 11)
            .copied()
            .expect("completed house 11");
        assert_eq!(live.action, crate::house::HOUSE_LIVE_ACTION);
        assert_eq!(live.owner_entity_index, session.state.entity_index);
        assert_eq!(live.control_flags & 1, 0);
        assert_eq!(session.terrain.mutation_sequence(), 6);
        assert!(matches!(
            session.pending_house_effects.as_slice(),
            [
                HouseStepEvent::ConstructionPass { remaining: 5, .. },
                HouseStepEvent::ConstructionPass { remaining: 4, .. },
                HouseStepEvent::ConstructionPass { remaining: 3, .. },
                HouseStepEvent::ConstructionPass { remaining: 2, .. },
                HouseStepEvent::ConstructionPass { remaining: 1, .. },
                HouseStepEvent::ConstructionCompleted { .. },
            ]
        ));
    }

    #[test]
    fn house_action_53_interleaves_resident_and_height_rng_in_one_cell() {
        let mut boundary_20_rng = 3;
        assert_eq!(reduce_house_height(100, 10, &mut boundary_20_rng), 90);
        assert_eq!(
            boundary_20_rng, 37_570,
            "`rand % 0x32 == 20` consumes one draw"
        );
        let mut boundary_21_rng = 20;
        assert_eq!(reduce_house_height(100, 10, &mut boundary_21_rng), 108);
        assert_eq!(
            boundary_21_rng, 14_898,
            "`rand % 0x32 == 21` consumes the second draw"
        );
        let mut wrapping_rng = 29;
        assert_eq!(
            reduce_house_height(250, 1, &mut wrapping_rng),
            12,
            "the second branch truncates to the source byte"
        );
        assert_eq!(wrapping_rng, 20_859);
        let mut no_draw_rng = 0xabcd;
        assert_eq!(reduce_house_height(10, 10, &mut no_draw_rng), 0);
        assert_eq!(no_draw_rng, 0xabcd);

        let footprint =
            CastleBuildFootprint::from_owner_cells(99, 1, 1, &[0xff, 10]).expect("one-cell BUILD");
        let mut terrain = SessionTerrain {
            map_type: MapType::Night,
            planes: TerrainPlanes {
                terrain_types: vec![0; 65_536].into_boxed_slice(),
                ground_heights: vec![100; 65_536].into_boxed_slice(),
                shading: vec![0; 65_536].into_boxed_slice(),
                angles: vec![0; 65_536].into_boxed_slice(),
                secondary_heights: vec![0; 65_536].into_boxed_slice(),
                entity_indices: vec![0; 65_536].into_boxed_slice(),
            },
            terrain_rng: 39_663,
            mutation_sequence: 0,
        };
        let outcome = terrain
            .apply_house_removal(Position { x: 0, y: 0, z: 0 }, 2, 5, true, &footprint)
            .expect("synthetic action-53 pass");
        assert_eq!(outcome.final_occupants, 4);
        assert_eq!(outcome.final_rng, 62_591);
        assert_eq!(
            outcome.residents,
            [HouseRemovalResident {
                position: Position {
                    x: 0,
                    y: 0,
                    z: 3_200
                },
                model: 13,
                action: 105,
            }]
        );
        assert_eq!(terrain.ground_heights()[0], 100);
        assert_eq!(terrain.mutation_sequence(), 1);
    }

    #[test]
    fn direct_session_commits_house_action_53_trace_before_reclaim() {
        let (package, closure) = fixture_paths();
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load_with_owner_cache(
            package,
            closure,
            private.join("generated/level1-unity-asset-cache"),
        )
        .expect("validated package M and owner cache");
        let mut session = world.session();
        let initial_terrain_hash = session.terrain.deterministic_hash();
        session
            .live_world
            .inject_damage(11, 28_001, session.state.entity_index.cast_signed())
            .expect("fatal house damage");

        session.step(PlayerCommand::default());
        let dying = session
            .live_world
            .houses()
            .iter()
            .find(|house| house.entity_index == 11)
            .expect("house 11");
        assert!(dying.active);
        assert_eq!(dying.action, crate::house::HOUSE_REMOVE_ACTION);
        assert_eq!(
            session.terrain.mutation_sequence(),
            2,
            "the first source turn applies two action-0 village terrain effects"
        );

        session.step(PlayerCommand::default());
        let removed = session
            .live_world
            .houses()
            .iter()
            .find(|house| house.entity_index == 11)
            .expect("house 11 retained for deterministic history");
        assert!(!removed.active);
        assert_eq!(removed.occupants, 0);
        assert_eq!(removed.rng, world.entities()[11].entity_rng);
        assert_eq!(
            session.terrain.mutation_sequence(),
            3,
            "house removal follows the two source-ordered ambient mutations"
        );
        assert_ne!(session.terrain.deterministic_hash(), initial_terrain_hash);
        assert_eq!(
            session
                .pending_house_effects
                .iter()
                .filter_map(|event| match event {
                    HouseStepEvent::ResidentRequested { model, action, .. } =>
                        Some((*model, *action)),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            [(4, 33), (12, 97)]
        );

        let mut replay = world.session();
        replay
            .live_world
            .inject_damage(11, 28_001, replay.state.entity_index.cast_signed())
            .expect("fatal replay damage");
        replay.step(PlayerCommand::default());
        replay.step(PlayerCommand::default());
        assert_eq!(replay.live_world, session.live_world);
        assert_eq!(replay.terrain, session.terrain);
        assert_eq!(replay.mission, session.mission);
        assert_eq!(replay.pending_house_effects, session.pending_house_effects);

        let mut exhausted = world.session();
        exhausted
            .live_world
            .inject_damage(11, 28_001, exhausted.state.entity_index.cast_signed())
            .expect("fatal house damage");
        let destruction_started = exhausted
            .live_world
            .step_house_source_slot(11, &exhausted.terrain);
        assert!(matches!(
            destruction_started.as_slice(),
            [HouseStepEvent::DestructionStarted { .. }]
        ));
        while exhausted
            .mission
            .spawn_house_resident(Position::default(), 4, 33, 152)
            .unwrap()
            .is_some()
        {}
        let live_before = exhausted.live_world.clone();
        let terrain_before = exhausted.terrain.clone();
        let mission_before = exhausted.mission.clone();
        let effects_before = exhausted.pending_house_effects.clone();
        let removal = exhausted
            .live_world
            .step_house_source_slot(11, &exhausted.terrain);
        let frozen_source_views = level1_turn_start_source_views(
            &exhausted.mission,
            &exhausted.state,
            &exhausted.live_world,
        );
        assert!(!exhausted.process_house_events(&removal, &frozen_source_views));
        exhausted.live_world = live_before.clone();
        assert_eq!(exhausted.live_world, live_before);
        assert_eq!(exhausted.terrain, terrain_before);
        assert_eq!(exhausted.mission, mission_before);
        assert_eq!(exhausted.pending_house_effects, effects_before);
    }

    #[test]
    fn qualified_owner_cache_adds_descriptor_backed_audio_and_goat_tmap_states() {
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let mut refs = Vec::new();
        let cache = append_level_one_cache_assets(
            &private.join("generated/level1-unity-asset-cache"),
            "730d2fa084265df839f3f2b55ed766ac167a8b152ca4631b5ded9d0bb9104ac8",
            "878123908f6de05f684f4cd1d73cff8d2ff08701cb6fdd12b7165bfcacc75719",
            &mut refs,
        )
        .expect("qualified owner audio cache");
        let sound = cache.sound_asset_refs;
        let music = cache.music_asset_ref;
        let goat_tmaps = cache.goat_tmap_bank;
        let castle = cache
            .castle_stage_one_footprint
            .expect("qualified Castle BUILD footprint");
        assert_eq!((castle.index, castle.width, castle.height), (1, 8, 8));
        assert_eq!(castle.cells().len(), 64);
        assert_eq!(refs.len(), 33);
        assert_eq!(
            sound.keys().copied().collect::<Vec<_>>(),
            [
                1, 2, 3, 4, 5, 8, 9, 10, 12, 14, 15, 16, 18, 19, 27, 29, 30, 31, 33, 34, 37, 39,
                40, 41, 43, 44, 46, 54, 55, 56, 57, 63
            ]
        );
        assert_eq!(sound[&12], 8);
        assert_eq!(
            refs[8].asset_id,
            "owner_cache:audio/sound-bank-1-822.json#sound_id=12"
        );
        assert_eq!(music, Some(32));
        assert_eq!(
            refs[32].content_address,
            "sha256:03b3f951048504d8a6d30611d6c44a62ec1c463726b30c33c16e3cb69824605d"
        );
        let goat_tmaps = goat_tmaps.expect("qualified Goat TMAP bank");
        assert_eq!(
            std::array::from_fn(|ordinal| goat_tmaps.selectors()[ordinal].selector),
            GOAT_TMAP_SELECTORS
        );
        assert_eq!(
            std::array::from_fn(|ordinal| {
                goat_tmaps.selectors()[0].states[ordinal].stream_cursor
            }),
            [490, 818, 1146, 1472, 1812]
        );
        assert_eq!(goat_tmaps.selectors()[1].states[0].pixels.len(), 32 * 22);
    }

    #[test]
    fn qualified_goat_tmap_clocks_materialize_every_exact_pixel_state_and_replay() {
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let cache = append_level_one_cache_assets(
            &private.join("generated/level1-unity-asset-cache"),
            "730d2fa084265df839f3f2b55ed766ac167a8b152ca4631b5ded9d0bb9104ac8",
            "878123908f6de05f684f4cd1d73cff8d2ff08701cb6fdd12b7165bfcacc75719",
            &mut Vec::new(),
        )
        .expect("qualified owner cache");
        let bank = cache.goat_tmap_bank.expect("qualified Goat TMAP bank");
        let expected_hashes = [
            [
                "cd5e773ab7c4e4e88536c5a690a8a1794a1b67361b402ab8365e9df3e59a1e9d",
                "502c7e18c327f09578c7ebcd31c351cd2c537bae9482c3f7630a7035be130190",
                "b22359a1aebc1f7afea7e5aa7a4e8c5bdfc5b238ca9a7f162e110470b3350103",
                "2011b3fb136f072ca7dcdae7199b2e8b9dbbdb71bdaa812b1bd659b695c51d12",
                "cd5e773ab7c4e4e88536c5a690a8a1794a1b67361b402ab8365e9df3e59a1e9d",
            ],
            [
                "acea2351981a1fd9aefaf7a2713bc00c4c2b2461dd087b9a2621e7fa3fb819a3",
                "4068c077eb4f66e39ae3bd13202462b477a418aa7781822d09f0feb3337765c7",
                "d5d9ccc838119171cb7ac06315e6e6994e72c058cf40b3b9a9b6fb6709e8931f",
                "f0f656d86c51628b5f0aac90932b79b448f64ef1ef38b41fcffc9d275df71fb1",
                "acea2351981a1fd9aefaf7a2713bc00c4c2b2461dd087b9a2621e7fa3fb819a3",
            ],
            [
                "7d081998c98ba0aa2bb57d4956926f3b8c80b31ae6ee481f8c9174551d6db722",
                "897331a6a567a6f64be563755ba33a5ab1e75e0a1d77d5a1fc9a67ca8657d44a",
                "22d9b6b2bad1a7fef49afcb05284c513f14200852c5cbaad097f684a0197f940",
                "752525ceb8c9a097f0676def3bdeb58258bd093ce7efacf3fe170dc7ec4cbc84",
                "7d081998c98ba0aa2bb57d4956926f3b8c80b31ae6ee481f8c9174551d6db722",
            ],
        ];
        let expected_ordinals = [0_u8, 1, 2, 3, 4, 1];
        let mut clocks = GoatTmapClocks::from_bank(&bank);
        let reset_pixels: [Box<[u8]>; GOAT_TMAP_SELECTORS.len()] =
            std::array::from_fn(|ordinal| clocks.states()[ordinal].pixels().into());

        for (step, expected_ordinal) in expected_ordinals.into_iter().enumerate() {
            for (selector_ordinal, clock) in clocks.states().iter().enumerate() {
                assert_eq!(clock.state_ordinal(), expected_ordinal);
                assert_eq!(
                    sha256_hex(clock.pixels()),
                    expected_hashes[selector_ordinal][usize::from(expected_ordinal)]
                );
            }
            if expected_ordinal == 4 {
                for (selector_ordinal, clock) in clocks.states().iter().enumerate() {
                    assert_eq!(clock.pixels(), reset_pixels[selector_ordinal].as_ref());
                }
            }
            if step + 1 < expected_ordinals.len() {
                clocks
                    .latch_draw_attempts(0b111)
                    .expect("three qualified selectors");
                clocks.service_unpaused_outer_turn(&bank);
            }
        }

        let expected_replay = clocks.clone();
        clocks.reset(&bank);
        for _ in 0..5 {
            clocks
                .latch_draw_attempts(0b111)
                .expect("three qualified selectors");
            clocks.service_unpaused_outer_turn(&bank);
        }
        assert_eq!(clocks, expected_replay);
        assert_eq!(clocks.hash_v1(), expected_replay.hash_v1());
    }

    #[test]
    fn authority_is_narrow_and_explicit() {
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("validated package M");
        assert_eq!(
            world.entities[152].authority,
            EntityAuthority::PlayerFlightLive
        );
        assert_eq!(
            world.entities[153].authority,
            EntityAuthority::SpellActivationLive
        );
        assert_eq!(
            world
                .entities
                .iter()
                .filter(|entity| entity.authority == EntityAuthority::PlayerFlightLive)
                .count(),
            1
        );
        assert_eq!(
            world
                .entities
                .iter()
                .filter(|entity| entity.authority == EntityAuthority::SpellActivationLive)
                .count(),
            1
        );
        assert_eq!(
            world
                .entities
                .iter()
                .filter(|entity| entity.authority == EntityAuthority::FrozenObserved)
                .count(),
            114
        );
        assert_eq!(
            world
                .entities
                .iter()
                .filter(|entity| entity.authority == EntityAuthority::GoatLive)
                .count(),
            LEVEL_ONE_GOAT_INDICES.len()
        );
        assert_eq!(
            world
                .entities
                .iter()
                .filter(|entity| entity.authority == EntityAuthority::VillagerLive)
                .count(),
            LEVEL_ONE_VILLAGER_INDICES.len()
        );
        assert_eq!(
            world
                .entities
                .iter()
                .filter(|entity| entity.authority == EntityAuthority::Inactive)
                .count(),
            823
        );
        let session = world.session();
        let dispositions = session
            .source_sprite_dispositions()
            .expect("startup-valid live Villagers have typed source dispositions");
        for source_entity_index in LEVEL_ONE_VILLAGER_INDICES {
            assert!(
                dispositions.iter().any(|disposition| matches!(
                    disposition,
                    SourceSpriteDisposition::Legacy {
                        source_entity_index: index,
                        family: SourceSpriteLegacyFamily::Resident,
                    } if *index == source_entity_index
                )),
                "live Villager slot {source_entity_index} must use its authority-aware resident renderer"
            );
        }
        for source_entity_index in LEVEL_ONE_GOAT_INDICES {
            assert!(
                dispositions.iter().any(|disposition| matches!(
                    disposition,
                    SourceSpriteDisposition::Legacy {
                        source_entity_index: index,
                        family: SourceSpriteLegacyFamily::Goat,
                    } if *index == source_entity_index
                )),
                "live Goat slot {source_entity_index} must use its authority-aware Goat renderer"
            );
        }
    }

    #[test]
    fn native_world_delegates_exact_legacy_terrain_queries() {
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("validated package M");
        for position in [
            Position { x: 0, y: 0, z: 0 },
            Position {
                x: 19_840,
                y: 56_960,
                z: 5_024,
            },
            Position {
                x: 0xffff,
                y: 0xffff,
                z: -1,
            },
            Position {
                x: 0x7f55,
                y: 0x80aa,
                z: 1_234,
            },
        ] {
            assert_eq!(
                world.ground_height(position),
                world.terrain.ground_height(position)
            );
            assert_eq!(
                world.material_mask(position),
                world.terrain.material_mask(position)
            );
        }
    }

    #[test]
    fn sessions_are_independent_and_reset_to_package_projection() {
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("validated package M");
        let mut first = world.session();
        let second = world.session();
        assert_eq!(
            conformance_hash(world.reset_state(), world.reset_context()),
            0x77b9_6b82_492d_6f4b,
            "the preserved package reset contract must not absorb presentation state"
        );
        assert_eq!(
            conformance_hash(&first.state, &first.context),
            0xe7ec_6bc1_8b5f_e775,
            "the additive complete Level 1 hand/audio session contract is independently pinned"
        );
        assert_eq!(
            first.context.effects,
            Some(FlightEffectsContext {
                local_player_index: 0,
                periodic_spell_effect: None,
                ..FlightEffectsContext::default()
            })
        );
        first.step(PlayerCommand {
            flags: 1,
            ..PlayerCommand::default()
        });
        assert_ne!(first.state, second.state);
        first.reset();
        assert_eq!(first.state, *world.reset_state());
        assert_eq!(first.context, world.session_context());
    }

    #[test]
    fn level_one_package_excludes_the_family_four_periodic_sound_branch() {
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("validated package M");
        let player = &world.players[usize::from(world.runtime.local_player_slot)];
        assert_eq!(player.enabled_spell_holders[4], 0);
        assert_eq!(
            world
                .entities
                .iter()
                .filter(|entity| entity.active && entity.class == 15)
                .map(|entity| entity.model)
                .collect::<Vec<_>>(),
            [0, 1],
            "the canonical Level 1 package exposes only Fireball and Possession holders"
        );

        let mut session = world.session();
        assert_eq!(
            session
                .context
                .spell_fire
                .as_ref()
                .unwrap()
                .enabled_holder_indices[4],
            0
        );
        assert_eq!(
            session
                .context
                .effects
                .as_ref()
                .unwrap()
                .periodic_spell_effect,
            None
        );
        session.state.entity_phase = 0;
        session.step(PlayerCommand::default());
        assert!(session.context.events.iter().all(|event| !matches!(
            event,
            crate::FlightEvent::PrepareSound {
                audio_site: crate::MissionAudioSite::PeriodicSpellExcluded,
                ..
            }
        )));
    }

    #[test]
    fn native_session_dispatches_an_ordinary_castle_defender_from_action_zero_to_one() {
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("validated package M");
        let mut session = world.session();
        let controller_slot = session
            .mission
            .spawn_castle_defense_controller(
                Position {
                    x: 100 << 8,
                    y: 100 << 8,
                    z: 384,
                },
                session.state.entity_index,
            )
            .expect("ordinary Level 1 source pool has a controller slot");
        assert_eq!(
            session.mission.castle_defense_controllers()[0].action_state,
            0
        );

        session.step(PlayerCommand::default());

        let controller = session
            .mission
            .castle_defense_controllers()
            .iter()
            .find(|controller| controller.source_entity_index == controller_slot)
            .unwrap();
        assert_eq!(controller.action_state, 1);
    }

    #[test]
    fn player_fireball_crosses_the_shared_arena_into_goat_death_and_one_mana_sphere() {
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("validated package M");
        let mut session = world.session();
        let goat = session
            .live_world
            .goats()
            .iter()
            .find(|goat| goat.active)
            .expect("Level 1 has an active Goat")
            .clone();

        session.state.position = Position {
            x: goat.position.x,
            y: goat.position.y.wrapping_add(384),
            z: goat.position.z.wrapping_sub(100),
        };
        session.state.yaw = 0;
        session.state.effective_pitch = 0;
        session.mission.has_fireball = true;
        session.mission.player_mana = 1_000;
        for _ in 0..3 {
            assert!(session.mission.spawn_fireball(&session.state));
        }

        for _ in 0..24 {
            session.step(PlayerCommand::default());
            if session
                .live_world
                .goats()
                .iter()
                .find(|candidate| candidate.entity_index == goat.entity_index)
                .is_some_and(|candidate| !candidate.active)
            {
                break;
            }
        }
        let retired = session
            .live_world
            .goats()
            .iter()
            .find(|candidate| candidate.entity_index == goat.entity_index)
            .expect("typed Goat row remains addressable after retirement");
        assert!(!retired.active);
        let sphere_count = session.mission.mana_spheres().len();
        assert_eq!(sphere_count, 1);
        let sphere = session.mission.mana_spheres()[0];
        assert_eq!(sphere.mana, goat.mana);
        assert_eq!(sphere.id, u32::from(sphere.source_entity_index));
        assert_eq!(
            session.mission.source_slots()[usize::from(sphere.source_entity_index)].entity_rng,
            sphere.entity_rng
        );

        for _ in 0..8 {
            session.step(PlayerCommand::default());
        }
        assert_eq!(
            session.mission.mana_spheres().len(),
            sphere_count,
            "retiring the imported Goat must not emit duplicate mana"
        );
        assert!(
            !session
                .mission
                .imported_source_slot_is_live(goat.entity_index)
        );
    }

    #[test]
    fn dolmen_contact_selects_high_recharge_on_the_following_player_action() {
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("validated package M");
        let dolmen = world
            .entities()
            .iter()
            .find(|entity| {
                entity.active && entity.class == 2 && entity.model == 2 && entity.action == 6
            })
            .expect("Level 1 dolmen");
        let mut session = world.session();
        session.state.position = dolmen.position;
        session.state.actual_speed = 0;
        session.state.target_speed = 0;
        session.mission.player_mana = 0;
        session.mission.player_max_mana = 10_000;

        session.step(PlayerCommand::default());
        assert_eq!(
            session.mission.player_mana,
            crate::level1_mission::PLAYER_OUTDOOR_MANA_REGEN,
            "the dolmen status bit changes the next regen value after this tick's regen"
        );

        session.state.position.x = session.state.position.x.wrapping_add(4_096);
        session.step(PlayerCommand::default());
        assert_eq!(
            session.mission.player_mana,
            crate::level1_mission::PLAYER_OUTDOOR_MANA_REGEN
                + crate::level1_mission::PLAYER_RECHARGE_MINIMUM_MANA_REGEN,
            "the latched high recharge value is consumed once after leaving"
        );

        session.step(PlayerCommand::default());
        assert_eq!(
            session.mission.player_mana,
            crate::level1_mission::PLAYER_OUTDOOR_MANA_REGEN * 2
                + crate::level1_mission::PLAYER_RECHARGE_MINIMUM_MANA_REGEN,
            "the following out-of-range tick returns to outdoor recharge"
        );
    }

    #[test]
    fn neutral_session_replay_matches_original_idle_source_projection() {
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("validated package M");
        let sequence =
            RegressionStateSequence::new(OWNED_LEVEL_ONE_STATE_SEQUENCE).expect("state sequence");
        let mut session = world.session();
        for frame_index in 0..sequence.frame_count() {
            if frame_index > 0 {
                session.step(PlayerCommand::default());
            }
            if matches!(frame_index, 0 | 8 | 19) {
                let frame = sequence.frame(frame_index).expect("captured frame");
                let original = frame.project_player_flight(frame.local_player_slot());
                assert_eq!(
                    source_projection_hash(&session.state),
                    state_hash(&original),
                    "source projection at idle frame {frame_index}"
                );
            }
        }
    }

    #[test]
    fn corrupt_package_hash_version_relation_and_bounds_fail_closed() {
        let (package, closure) = fixture_paths();
        for mutation in [
            Mutation::ManifestSchema,
            Mutation::EntityMagic,
            Mutation::ResetMapping,
        ] {
            let temp = TempPackage::copy_from(&package, &closure);
            temp.mutate(mutation);
            assert!(
                NativeWorld::load(&temp.package, &temp.closure).is_err(),
                "mutation {mutation:?} must fail"
            );
        }
    }

    #[test]
    fn corrupt_component_version_relation_and_bounds_reach_their_parser_gates() {
        let (package, _) = fixture_paths();

        let mut players = fs::read(package.join("players.bin")).unwrap();
        players[4] ^= 0xff;
        assert!(parse_players(&players, OwnerPackageProfile::Level1).is_err());

        let mut entities = fs::read(package.join("entities.bin")).unwrap();
        let relation_offset = 12 + 152 * ENTITY_RECORD_BYTES + 164;
        entities[relation_offset..relation_offset + 4].copy_from_slice(&999_u32.to_le_bytes());
        assert!(parse_entities(&entities).is_err());

        let mut players = fs::read(package.join("players.bin")).unwrap();
        players[16..18].copy_from_slice(&1_000_u16.to_le_bytes());
        assert!(parse_players(&players, OwnerPackageProfile::Level1).is_err());

        let mut runtime = fs::read(package.join("runtime.bin")).unwrap();
        let stage_relation_offset = 192 + 6;
        runtime[stage_relation_offset] = 9;
        assert!(parse_runtime(&runtime).is_err());

        let mut presentation = fs::read(package.join("presentation.bin")).unwrap();
        presentation[4] ^= 0xff;
        assert!(parse_presentation(&presentation).is_err());
    }

    #[test]
    fn carrier_presentation_extent_mutations_fail_closed() {
        let (package, _) = fixture_paths();
        let original = fs::read(package.join("presentation.bin")).unwrap();
        let row = 12 + usize::from(MANA_CARRIER_RENDER_INDEX) * PARTICLE_RECORD_BYTES;
        for (offset, bytes) in [
            (6_usize, 544_u16.to_le_bytes()),
            (8_usize, 799_u16.to_le_bytes()),
        ] {
            let mut presentation = original.clone();
            presentation[row + offset..row + offset + 2].copy_from_slice(&bytes);
            let parsed = parse_presentation(&presentation).expect("well-formed mutation");
            let error = validate_level_one_mission_presentation(&parsed).unwrap_err();
            assert!(error.to_string().contains("169 mismatch"));
        }
        let mut presentation = original.clone();
        presentation[row + 11] = (-15_i8).cast_unsigned();
        let parsed = parse_presentation(&presentation).expect("well-formed mutation");
        let error = validate_level_one_mission_presentation(&parsed).unwrap_err();
        assert!(error.to_string().contains("169 mismatch"));

        for index in [
            POSSESSION_ATTRACTION_RENDER_INDEX,
            POSSESSION_IMPACT_RENDER_INDEX,
            GROUND_FIRE_FIRST_RENDER_INDEX,
            GROUND_FIRE_FIRST_RENDER_INDEX + 7,
        ] {
            let mut presentation = original.clone();
            let row = 12 + usize::from(index) * PARTICLE_RECORD_BYTES;
            presentation[row] ^= 1;
            let parsed = parse_presentation(&presentation).expect("well-formed mutation");
            let error = validate_level_one_mission_presentation(&parsed).unwrap_err();
            assert!(
                error.to_string().contains(&format!("{index} mismatch")),
                "row {index} mutation reached the exact Level 1 presentation gate: {error}"
            );
        }
    }

    #[test]
    fn corrupt_closure_hash_binding_and_unresolved_frame_use_fail_closed() {
        let (package, closure) = fixture_paths();
        for mutation in [
            ClosureMutation::Hash,
            ClosureMutation::PackageBinding,
            ClosureMutation::Unresolved,
            ClosureMutation::AssetOrdinal,
        ] {
            let temp = TempPackage::copy_from(&package, &closure);
            temp.mutate_closure(mutation);
            assert!(
                NativeWorld::load(&temp.package, &temp.closure).is_err(),
                "closure mutation {mutation:?} must fail"
            );
        }
    }

    #[test]
    fn v1_or_incomplete_reset_is_rejected_instead_of_synthesizing_ambient() {
        let (package, closure) = fixture_paths();
        let temp = TempPackage::copy_from(&package, &closure);
        let manifest_path = temp.package.join("manifest.json");
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        manifest["schema"] = json!("mc2-owner-level-export/v1");
        manifest["reset_projection"]["field_mappings"]
            .as_object_mut()
            .unwrap()
            .remove("ambient_loop_31_distance");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        assert!(NativeWorld::load(&temp.package, &temp.closure).is_err());
    }

    #[derive(Clone, Copy, Debug)]
    enum Mutation {
        ManifestSchema,
        EntityMagic,
        ResetMapping,
    }

    #[derive(Clone, Copy, Debug)]
    enum ClosureMutation {
        Hash,
        PackageBinding,
        Unresolved,
        AssetOrdinal,
    }

    struct TempPackage {
        root: PathBuf,
        package: PathBuf,
        closure: PathBuf,
    }

    impl TempPackage {
        fn copy_from(package: &Path, closure: &Path) -> Self {
            let root = std::env::temp_dir().join(format!(
                "mc2-native-world-{}-{}",
                std::process::id(),
                unique_suffix()
            ));
            let output = root.join("package");
            fs::create_dir_all(&output).unwrap();
            fs::copy(package.join("manifest.json"), output.join("manifest.json")).unwrap();
            for (name, _) in PACKAGE_MEMBERS {
                fs::copy(package.join(name), output.join(name)).unwrap();
            }
            let closure_output = root.join("closure.json");
            fs::copy(closure, &closure_output).unwrap();
            Self {
                root,
                package: output,
                closure: closure_output,
            }
        }

        fn mutate(&self, mutation: Mutation) {
            match mutation {
                Mutation::ManifestSchema | Mutation::ResetMapping => {
                    let path = self.package.join("manifest.json");
                    let mut manifest: Value =
                        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                    if matches!(mutation, Mutation::ManifestSchema) {
                        manifest["schema"] = json!("mc2-owner-level-export/v999");
                    } else {
                        manifest["reset_projection"]["field_mappings"]["ambient_loop_31_distance"]
                            ["offset"] = json!(560);
                    }
                    fs::write(path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
                }
                Mutation::EntityMagic => self.flip_byte("entities.bin", 0),
            }
        }

        fn mutate_closure(&self, mutation: ClosureMutation) {
            let mut closure: Value =
                serde_json::from_slice(&fs::read(&self.closure).unwrap()).unwrap();
            match mutation {
                ClosureMutation::Hash => closure["closure_sha256"] = json!("0".repeat(64)),
                ClosureMutation::PackageBinding => {
                    closure["source"]["level_export"]["package_sha256"] = json!("0".repeat(64));
                }
                ClosureMutation::Unresolved => {
                    closure["sets"]["frame_used"]["unresolved"] = json!(["not-resolved"]);
                }
                ClosureMutation::AssetOrdinal => {
                    closure["sets"]["frame_used"]["assets"][0]["trace_event_ordinal"] = json!(2);
                }
            }
            if !matches!(mutation, ClosureMutation::Hash) {
                closure.as_object_mut().unwrap().remove("closure_sha256");
                closure["closure_sha256"] =
                    json!(sha256_hex(canonical_json(&closure).unwrap().as_bytes()));
            }
            fs::write(&self.closure, serde_json::to_vec_pretty(&closure).unwrap()).unwrap();
        }

        fn flip_byte(&self, name: &str, offset: usize) {
            let path = self.package.join(name);
            let mut bytes = fs::read(&path).unwrap();
            bytes[offset] ^= 0xff;
            fs::write(path, bytes).unwrap();
        }
    }

    impl Drop for TempPackage {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn unique_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time after epoch")
            .as_nanos()
    }
}
