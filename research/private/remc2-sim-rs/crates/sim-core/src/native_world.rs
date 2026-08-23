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

#[cfg(test)]
use crate::goat::LEVEL_TWO_GOAT_INDICES;
use crate::{
    FlightContext, FlightEffectsContext, FlightEvent, FlightProfile, FlightState, FlightWorld,
    PlayerCommand, Position, SOURCE_SPELL_INDEX_MAP, SourceAudioCallTag, SpellFireContext,
    angle_distance,
    archer_tmap::{ArcherTmapBank, ArcherTmapClocks},
    collision_mask::{
        CollisionMaskBank, CollisionMaskLookupError, LEVEL2_COLLISION_MASK_BANK_BYTES,
    },
    combat_tmap::{CombatTmapBank, FireballTmapClock, PossessionTmapClock},
    conformance_hash,
    economy_tmap::{CastleTmapClock, EconomyTmapBank},
    entity_passes_for_speed_index,
    firefly_tmap::{FireflyTmapBank, FireflyTmapClocks},
    frozen_actor_tmap::{FrozenActorTmapBank, VillagerTmapClocks},
    goat::{
        GOAT_BASE_TMAP, GOAT_CLASS, GOAT_MODEL, GOAT_PATROL_ACTION, GOAT_RENDER_PARAMETER_INDEX,
        GOAT_STAGE_POINT, GoatBehaviorConfig, GoatFamily, GoatPlayerTarget, GoatState,
        GoatStepEvent, LEVEL_ONE_GOAT_INDICES, LiveWorldState,
    },
    goat_tmap::{
        GOAT_TMAP_SELECTORS, GOAT_TMAP_STATE_COUNT, GoatTmapBank, GoatTmapClocks, GoatTmapFrame,
        GoatTmapSelectorBank,
    },
    heading_for_positions,
    house::{
        HOUSE_CLASS, HOUSE_MODEL, HouseFamily, HouseParameter, HouseResidentEntry, HouseState,
        HouseStepEvent,
    },
    latch_command_with_context,
    legacy_terrain::{
        LEGACY_MAP_CELLS, LegacyTerrain, interpolate_cave, interpolate_ground, material_mask,
    },
    level1_mission::{
        ARCHER_BASE_TMAP, ARCHER_LOGICAL_HEIGHT, ARCHER_RENDER_INDEX, ARCHER_RENDER_MODE,
        AuthoredFeature, CASTLE_SPELL_FAMILY, CASTLE_STAGE_ONE_COLLISION_HALF_WIDTH,
        CASTLE_STAGE_ONE_LIFE, CASTLE_STAGE_ONE_MANA_CAPACITY, CastleBuildFootprint,
        CastleTerrainUpdate, DEATH_BURST_ACTION, DEATH_BURST_CLASS, DEATH_BURST_MODEL,
        FIREBALL_BASE_TMAP, FIREBALL_IMPACT_BASE_TMAP, FIREBALL_IMPACT_LOGICAL_HEIGHT,
        FIREBALL_IMPACT_RENDER_INDEX, FIREBALL_IMPACT_RENDER_MODE, FIREBALL_LIFETIME_TICKS,
        FIREBALL_LOGICAL_HEIGHT, FIREBALL_PROJECTILE_CLASS, FIREBALL_PROJECTILE_MODEL,
        FIREBALL_RENDER_INDEX, FIREBALL_RENDER_MODE, FIREBALL_SPEED, FIREBALL_SPELL_FAMILY,
        FIREFLY_BASE_TMAP, FIREFLY_LOGICAL_HEIGHT, FIREFLY_RENDER_INDEX, FIREFLY_RENDER_MODE,
        GROUND_FIRE_BASE_TMAP, GROUND_FIRE_FIRST_RENDER_INDEX, GROUND_FIRE_LOGICAL_HEIGHTS,
        GROUND_FIRE_PRESENTATION_BYTE_11, GROUND_FIRE_PRESENTATION_SPEEDS, GROUND_FIRE_RENDER_MODE,
        HouseResidentRequest, Level1AmbientEntity, Level1CastlePlacementBlocker,
        Level1CollisionTarget, Level1MissionState, Level1MissionTemplate,
        Level1ResetRawSourceState, Level1ResetSourceSlot, Level1SourceCollisionTarget,
        Level1SourceEffect, Level1SourceEntityAuthority, Level1SourceEntityView,
        MANA_CARRIER_ATTRACTION_RADIUS, MANA_CARRIER_BASE_TMAP, MANA_CARRIER_COLLISION_HALF_HEIGHT,
        MANA_CARRIER_COLLISION_HALF_WIDTH, MANA_CARRIER_COLLISION_VERTICAL_OFFSET,
        MANA_CARRIER_DESCENT, MANA_CARRIER_GROUND_CLEARANCE, MANA_CARRIER_LOGICAL_HEIGHT,
        MANA_CARRIER_MAX_LIFE, MANA_CARRIER_MAX_MANA, MANA_CARRIER_RENDER_INDEX,
        MANA_CARRIER_RENDER_MODE, MANA_CARRIER_SPEED, MANA_SPHERE_COLLISION_DIMENSIONS,
        MANA_SPHERE_GRAVITY, MANA_SPHERE_MAX_XY_VELOCITY, MANA_SPHERE_TERMINAL_VELOCITY,
        ManaCarrier, ManaSphere, PLAYER_DEATH_BURST_MAX_LIFE, PLAYER_DEATH_BURST_SUBSPELL_INDEX,
        POSSESSION_ATTRACTION_BASE_TMAP, POSSESSION_ATTRACTION_LOGICAL_HEIGHT,
        POSSESSION_ATTRACTION_RENDER_INDEX, POSSESSION_ATTRACTION_RENDER_MODE,
        POSSESSION_IMPACT_ACTION, POSSESSION_IMPACT_BASE_TMAP, POSSESSION_IMPACT_CLASS,
        POSSESSION_IMPACT_COLLISION_HALF_HEIGHT, POSSESSION_IMPACT_COLLISION_HALF_WIDTH,
        POSSESSION_IMPACT_COLLISION_VERTICAL_OFFSET, POSSESSION_IMPACT_LIFE,
        POSSESSION_IMPACT_LOGICAL_HEIGHT, POSSESSION_IMPACT_RENDER_INDEX,
        POSSESSION_IMPACT_RENDER_MODE, POSSESSION_PROJECTILE_ACTION, POSSESSION_PROJECTILE_CLASS,
        POSSESSION_PROJECTILE_COLLISION_HALF_HEIGHT, POSSESSION_PROJECTILE_COLLISION_HALF_WIDTH,
        POSSESSION_PROJECTILE_COLLISION_VERTICAL_OFFSET, POSSESSION_PROJECTILE_LIFETIME_TICKS,
        POSSESSION_PROJECTILE_MODEL, POSSESSION_PROJECTILE_SPEED, POSSESSION_SPELL_FAMILY,
        PROJECTILE_ACQUISITION_ANGLE, SOURCE_REPLACEABLE_FLAG, SPEED_SPELL_ACTIVATION_TICKS,
        SPEED_SPELL_FAMILY, SPEED_SPELL_MANA_COST, SPELL_JAR_BASE_TMAP, SPELL_JAR_LOGICAL_HEIGHT,
        SPELL_JAR_RENDER_INDEX, SPELL_JAR_RENDER_MODE, mana_sphere_size_ordinal,
        projectile_guidance_angles, projectile_target_score, source_map_search_slots,
        source_search_offsets,
    },
    mark_source_audio_call,
    mission_content::{
        MissionContentProfile, MissionContentSession, MissionContentTemplate,
        MissionWizardMapSettings, REMC2_SPEC_COMMIT,
    },
    renderable_map_chain::{RenderableMapChains, RenderableMapSeed, raw_source_map_chain_entries},
    source_sprite_tmap::{
        ImportedResetSpriteTuple, SourceSpriteContractRoute, SourceSpriteContractRow,
        SourceSpriteDisposition, SourceSpriteLegacyFamily, SourceSpriteTemplateDisposition,
        SourceSpriteTmapBank, SourceSpriteTmapFrame, imported_reset_sprite_disposition,
        level2_bad_stone_sprite_disposition, source_sprite_dispositions,
        unknown_source_sprite_tuple,
    },
    spell_activation::{
        BEYOND_SIGHT_ACTIVE_ACTION, BEYOND_SIGHT_FAMILY, BeyondSightState, INVISIBLE_ACTIVE_ACTION,
        INVISIBLE_FAMILY, INVISIBLE_STATUS_BIT, InvisibleState, InvisibleStep,
        Level2SetSpellProfile, PhysicalSpellActionRole, SHIELD_ACTIVE_ACTION, SHIELD_FAMILY,
        ShieldDamage, ShieldState, ShieldStep, SpellActivationHolder, SpellCasterState,
        SpellHolderAcquisitionOutcome, SpellHolderAcquisitionResult, SpellHolderAcquisitionState,
        acquire_level2_spell_holder_with_return_side, activate_spell, beyond_sight_unlocked_tier,
        level2_applied_tier, level2_set_spell_profile, physical_spell_action,
    },
    state_hash,
    static_world_tmap::StaticWorldTmapBank,
    step_entity_body_with_context, step_entity_pass_with_context,
    terrain_frame::{
        Level1CastleRootRender, Level1FrozenActorRender, Level1TerrainFrame,
        Level1TerrainFrameCamera, Level1TerrainFrameResources, Level1WorldFrameResources,
        TerrainFrameError, render_level1_source_world_frame,
    },
    turn_toward,
    villager::{
        LEVEL_ONE_VILLAGER_INDICES, LEVEL_TWO_VILLAGER_INDICES, VILLAGER_CLASS, VILLAGER_MODEL,
        VillagerBehaviorConfig, VillagerFamily, VillagerState, VillagerStepEffect,
    },
};

#[cfg(test)]
use crate::level1_mission::POSSESSION_IMPACT_MODEL;
#[cfg(test)]
use crate::mission_content::checked_in_level2_activation_plan;

const MANIFEST_SCHEMA: &str = "mc2-owner-level-export/v2";
const LEVEL2_MANIFEST_SCHEMA_V3: &str = "mc2-owner-level-export/v3";
const LEVEL2_MANIFEST_SCHEMA_V4: &str = "mc2-owner-level-export/v4";
const LEVEL2_MANIFEST_SCHEMA_V5: &str = "mc2-owner-level-export/v5";
const CLOSURE_SCHEMA: &str = "mc2-level-asset-closure/v2";
const AUDIO_CACHE_SCHEMA: &str = "mc2-owner-unity-asset-cache/v1";
const LEVEL2_GAMEPLAY_CACHE_SCHEMA: &str = "mc2-level2-day-gameplay-cache/v1";
const LEVEL2_NYPHUR_SPELL_COOLDOWNS: [i16; PLAYER_SPELL_COUNT] = [
    2, 10, 40, 32, 300, 1, 1, 1, 1, 4, 1, 1, 0, 0, 0, 0, 0, 0, 400, 600, 600, 400, 400, 0, 0, 0,
];
/// Pinned `Spells.cpp` initializes byte_0 of every `SPELLS_BEGIN_BUFFER_str`
/// family row to 0x03.  `sub_15F20` compares the caller's starting tier with
/// that source-table bound; it does not use the AI's SpellIndex as a limit.
const LEVEL2_NYPHUR_SOURCE_SPELL_TIER_COUNT: i8 = 3;
/// `NewEvent_4A050` defaults used by the Castle model-3/action-9 holder.
/// `AddBallon_60AB0` multiplies this holder speed by the target Castle's
/// `dword_0x10_16` stage scalar when deciding whether to transfer mana.
const LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE: i32 = 300;
const LEVEL2_CASTLE_CHILD_DEFAULT_SPEED: i16 = 16;
/// Pinned `sub_4C1E0` / descriptor-83 Castle auxiliary parameters.
const LEVEL2_CASTLE_AUXILIARY_MAX_LIFE: i32 = 1_000;
const LEVEL2_CASTLE_AUXILIARY_MINIMUM_SPEED: i16 = 30;
const LEVEL2_CASTLE_AUXILIARY_CADENCE: u8 = 15;
const LEVEL2_CASTLE_AUXILIARY_TARGET_RANGE: u32 = 0x0f00;
const LEVEL2_CASTLE_AUXILIARY_TARGET_HALF_ANGLE: i16 = 0x0200;
const LEVEL2_CASTLE_AUXILIARY_ALLOWED_MATERIALS: u32 = 0x0002_0000;
/// `sub_4ABA0` writes maxMana=10000 for a Castle class-3/model-3 defender.
const LEVEL2_CASTLE_DEFENDER_MAX_MANA: i32 = 10_000;
/// The same constructor writes maxLife=10000 and actSpeed=48; these are not
/// the 300/16 defaults used by model-15/41/42/79 auxiliary children.
const LEVEL2_CASTLE_DEFENDER_MAX_LIFE: i32 = 10_000;
const LEVEL2_CASTLE_DEFENDER_SPEED: i16 = 48;
/// Source class-10 temporary mana-attraction effect tuple from
/// `sub_4A310`/`sub_38D80` (EventsFunctions.cpp:28349-28385).
const MANA_ATTRACTION_CLASS: u8 = 10;
const MANA_ATTRACTION_MODEL: u8 = 54;
const MANA_ATTRACTION_ACTION: u8 = 59;
const MANA_ATTRACTION_ALIAS_ACTION: u8 = 76;
/// `sub_60D50` receives the model-3 holder profile installed by the source
/// class-3/subtype-3 constructor: particle row 169 has rotSpeed `0x320`, so
/// `SetEntityIndexAndRot_49CD0` binds fov to 400.
const LEVEL2_CASTLE_DEFENDER_CAVE_FOV: i16 = 400;
// `str_D7BD6[68].word_0xc` is the third argument to `sub_580E0`; the
// fourth (`word_0xa`, 1536) is unused by that helper.  The resulting floor
// clearance is therefore 512 source units.
const LEVEL2_CASTLE_DEFENDER_CAVE_FLOOR_CLEARANCE: i16 = 512;
const LEVEL2_CASTLE_DEFENDER_CAVE_STEP: i16 = -16;
const LEVEL2_CASTLE_DEFENDER_CAVE_BLOCKED_SPEED: i16 = 48;
const LEVEL2_CASTLE_DEFENDER_CAVE_OPEN_SPEED: i16 = 96;
const LEVEL2_CASTLE_DEFENDER_CAVE_SOUND: u16 = 22;
const LEVEL2_CASTLE_DEFENDER_CAVE_SOUND_COOLDOWN: u8 = 32;
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
/// `DisableEntityDrawing04_57F10` sets byte 17 bit 2 of the exported entity
/// control dword (the source dword value is 0x400).
const ENTITY_DRAW_DISABLED_BIT: u32 = 0x0000_0400;
const PLAYER_COUNT: usize = 8;
const PLAYER_COUNT_I16: i16 = 8;
const PLAYER_COUNT_U32: u32 = 8;
const PLAYER_LEGACY_RECORD_BYTES: usize = 573;
const PLAYER_LEGACY_RECORD_BYTES_U16: u16 = 573;
const TYPE_STR_164_AUXILIARY_BYTES: usize = 89;
/// Version-3 Level 2 records end after the Type_str_164 auxiliary carrier.
/// They remain decode-only inspection input; qualified sessions require v5.
const PLAYER_LEVEL2_V4_RECORD_BYTES: usize =
    PLAYER_LEGACY_RECORD_BYTES + TYPE_STR_164_AUXILIARY_BYTES;
const PLAYER_LEVEL2_V4_RECORD_BYTES_U16: u16 = 662;
/// `type_str_0x2BDE.byte_0x006_2BE4_11236`, appended after the existing
/// Type_str_164 carrier rather than repurposing compact ActPlayerIndex bytes.
const PLAYER_LEVEL2_BANISHMENT_STATE_OFFSET: usize = PLAYER_LEVEL2_V4_RECORD_BYTES;
const PLAYER_LEVEL2_BANISHMENT_STATE_BYTES: usize = 1;
const PLAYER_LEVEL2_RECORD_BYTES: usize =
    PLAYER_LEVEL2_V4_RECORD_BYTES + PLAYER_LEVEL2_BANISHMENT_STATE_BYTES;
const PLAYER_LEVEL2_RECORD_BYTES_U16: u16 = 663;
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
const LEVEL2_EXPERIENCE_SCROLL_AWARD: i32 = 4;
const LEVEL2_SCROLL_COLLISION_HALF_WIDTH: i16 = 768;
const LEVEL2_SCROLL_COLLISION_VERTICAL_OFFSET: i16 = 37;
const LEVEL2_SCROLL_COLLISION_HALF_HEIGHT: i16 = 1_280;
const LEVEL2_PLAYER_COLLISION_HALF_WIDTH: i16 = 121;
const LEVEL2_PLAYER_COLLISION_VERTICAL_OFFSET: i16 = 100;
const LEVEL2_PLAYER_COLLISION_HALF_HEIGHT: i16 = 100;

// `sub_6DB50` walks the owned SPELLS.DAT rows in family order.  These are the
// two progression thresholds used by the Level 2 scroll callback; the first
// row is the reset level and the following rows are the tier-1/tier-2
// `xpos1_E` values from SPELLS.DAT.
const LEVEL2_SPELL_TIER_THRESHOLDS: [[i32; 3]; PLAYER_SPELL_COUNT] = [
    [0, 400, 12_000],
    [0, 600, 4_800],
    [0, 40, 80],
    [0, 400, 1_200],
    [0, 350, 1_050],
    [0, 400, 1_200],
    [0, 400, 1_200],
    [0, 1_000, 2_000],
    [0, 400, 1_200],
    [0, 2_000, 10_000],
    [0, 400, 2_000],
    [0, 330, 990],
    [0, 360, 1_080],
    [0, 400, 1_200],
    [0, 400, 1_200],
    [0, 800, 1_600],
    [0, 1_200, 2_400],
    [0, 1_800, 3_600],
    [0, 2_000, 8_000],
    [0, 450, 1_200],
    [0, 1_500, 3_000],
    [0, 400, 1_200],
    [0, 360, 720],
    [0, 400, 1_200],
    [0, 800, 1_600],
    [0, 400, 1_600],
];
const FRAME_ASSET_COUNT: usize = 76;
const LEVEL_ONE_CONTROL_MODE_SEAM_DEFAULT: u8 = 0;
const LEVEL2_UPSTREAM_COMMIT: &str = "6806c7204d17220ecc0430759f784aea8ee2ddb2";
const LEVEL2_QUALIFIED_CLOSURE_SHA256: &str =
    "bee3b7a4fd585d6631523b87217978cbfa58ed08cd494fa7921486053de1dccc";
const LEVEL2_QUALIFIED_OWNER_MANIFEST_SCHEMA: &str = "mc2-owner-level-export/v5";
const LEVEL2_QUALIFIED_PLAYERS_COMPONENT_MAGIC: &str = "MCP1";
const LEVEL2_QUALIFIED_PLAYERS_COMPONENT_VERSION: u64 = 4;
const LEVEL2_QUALIFIED_PLAYERS_COMPONENT_RECORD_BYTES: u64 = 663;
const LEVEL2_QUALIFIED_PLAYERS_COMPONENT_COUNT: u64 = 8;
const LEVEL2_QUALIFIED_CATALOG_FILE_SHA256: &str =
    "a25488be0933e5b4499665d37440a9860c8614649d9f695080da511c11d75b44";
const LEVEL2_QUALIFIED_CATALOG_SHA256: &str =
    "ea9a735c598f2cbed81541366e1fdf8a5fcbc8cef7aecd086bb8a6a9e7c2429e";
const LEVEL2_QUALIFIED_BUILD_DAT_SHA256: &str =
    "3db5d9a763dedafbe3077eeeef47431da0b5d397e311f6ee46719a0fc51e8e5d";
const LEVEL2_QUALIFIED_BUILD_DAT_BYTES: u64 = 80_064;
const LEVEL2_QUALIFIED_BUILD_TAB_SHA256: &str =
    "e064a1520e96f65c13fde9a05918d240ee4f708ab6aefc793ec8b391f220c05e";
const LEVEL2_QUALIFIED_BUILD_TAB_BYTES: u64 = 462;
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
    (
        "players.bin",
        12 + PLAYER_COUNT * PLAYER_LEGACY_RECORD_BYTES,
    ),
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
    (
        "players.bin",
        12 + PLAYER_COUNT * PLAYER_LEGACY_RECORD_BYTES,
    ),
    ("presentation.bin", 10_720),
    ("runtime.bin", RUNTIME_BYTES),
    ("terrain-post-features.bin", TERRAIN_BYTES),
    ("terrain-pre-features.bin", TERRAIN_BYTES),
    ("terrain.bin", TERRAIN_BYTES),
];

const LEVEL2_V4_PACKAGE_MEMBERS: [(&str, usize); 14] = [
    ("authored-level.bin", AUTHORED_LEVEL_BYTES),
    ("collision-masks.bin", LEVEL2_COLLISION_MASK_BANK_BYTES),
    (
        "entities-post-features.bin",
        12 + ENTITY_COUNT * ENTITY_RECORD_BYTES,
    ),
    ("entities.bin", 12 + ENTITY_COUNT * ENTITY_RECORD_BYTES),
    ("feature-allocator.bin", 4_032),
    ("feature-construction-inputs.bin", 34_164),
    ("feature-end.bin", 16),
    ("feature-start.bin", 16),
    (
        "players.bin",
        12 + PLAYER_COUNT * PLAYER_LEVEL2_V4_RECORD_BYTES,
    ),
    ("presentation.bin", 10_720),
    ("runtime.bin", RUNTIME_BYTES),
    ("terrain-post-features.bin", TERRAIN_BYTES),
    ("terrain-pre-features.bin", TERRAIN_BYTES),
    ("terrain.bin", TERRAIN_BYTES),
];

/// The qualified v5 owner package differs from v4 only in the explicitly
/// versioned one-byte Type_str_0x2BDE carrier appended to each player row.
const LEVEL2_V5_PACKAGE_MEMBERS: [(&str, usize); 14] = [
    ("authored-level.bin", AUTHORED_LEVEL_BYTES),
    ("collision-masks.bin", LEVEL2_COLLISION_MASK_BANK_BYTES),
    (
        "entities-post-features.bin",
        12 + ENTITY_COUNT * ENTITY_RECORD_BYTES,
    ),
    ("entities.bin", 12 + ENTITY_COUNT * ENTITY_RECORD_BYTES),
    ("feature-allocator.bin", 4_032),
    ("feature-construction-inputs.bin", 34_164),
    ("feature-end.bin", 16),
    ("feature-start.bin", 16),
    (
        "players.bin",
        12 + PLAYER_COUNT * PLAYER_LEVEL2_RECORD_BYTES,
    ),
    ("presentation.bin", 10_720),
    ("runtime.bin", RUNTIME_BYTES),
    ("terrain-post-features.bin", TERRAIN_BYTES),
    ("terrain-pre-features.bin", TERRAIN_BYTES),
    ("terrain.bin", TERRAIN_BYTES),
];

mod world_types;
use world_types::Result;
pub use world_types::*;

#[path = "campaign/level2.rs"]
mod campaign;
mod level2_nyphur;
pub(crate) use level2_nyphur::level2_nyphur_source_identity_parts;
pub use level2_nyphur::{
    Level2NyphurCommonUpkeepSnapshot, Level2NyphurDispatchCall, Level2NyphurDispatchHelper,
    Level2NyphurDispatchTrace, Level2NyphurParticipant, Level2NyphurVisibility,
};
use level2_nyphur::{Level2NyphurDispatcher, level2_nyphur_source_identity};

/// One source-owned Day spell-hand row consumed by the HUD presenter.
///
/// The row deliberately carries the holder's live source fields together
/// with the direct MSPRD0-0/text selectors selected by the pinned draw routine.
/// Unity may validate and draw these values, but it must not reconstruct the
/// holder chain or choose a neighboring selector when a row is absent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2DayHudSpellSlot {
    pub holder_entity_index: u16,
    pub parent_entity_index: u16,
    pub raw_spell_selector: i16,
    pub color_index: u8,
    pub model: u8,
    pub tier: i8,
    pub action: u8,
    pub action_marker: i8,
    pub active_value: i16,
    pub activation_value: u16,
    pub control_flags: u32,
    pub max_mana: i32,
    pub mana_regen: i32,
    pub caster_mana: i32,
    pub frame_selector: u16,
    pub icon_selector: u16,
    pub level_text_selector: u8,
    /// Final source admission for `DrawSpellIcon_2E260`. Unity presents the
    /// carried decision and never reconstructs the enabled/active/phase
    /// suppression predicate.
    pub draw_admitted: bool,
}

impl Level2DayHudSpellSlot {
    #[must_use]
    const fn empty(raw_spell_selector: i16, color_index: u8) -> Self {
        Self {
            holder_entity_index: 0,
            parent_entity_index: 0,
            raw_spell_selector,
            color_index,
            model: 0,
            tier: 0,
            action: 0,
            action_marker: 0,
            active_value: 0,
            activation_value: 0,
            control_flags: 0,
            max_mana: 0,
            mana_regen: 0,
            caster_mana: 0,
            frame_selector: 0,
            icon_selector: 0,
            level_text_selector: 0,
            draw_admitted: false,
        }
    }
}

/// `SetDefaultSpells_5C0A0` sets `isEnabled_1 & 4` for these source spell
/// models. Keeping this source table beside the projection lets Rust carry
/// the final `DrawSpellIcon_2E260` admission without asking Unity to rebuild
/// source spell metadata.
const LEVEL2_DAY_SPELL_MODELS_WITH_DRAW_SUPPRESSION: [u8; 7] = [3, 4, 6, 8, 11, 12, 14];

fn level2_day_spell_draw_admitted(model: u8, active_value: i16, color_phase_1: u8) -> bool {
    !(LEVEL2_DAY_SPELL_MODELS_WITH_DRAW_SUPPRESSION.contains(&model)
        && active_value > 0
        && active_value < 32
        && color_phase_1 != 0)
}

fn level2_day_color_phases(turn: i32) -> (u8, u8) {
    // EventsFunctions.cpp writes colorIndex[i] = Turn / i & 1.
    let color_phase_1 = u8::try_from((turn / 1) & 1).expect("source color phase is one bit");
    let color_phase_2 = u8::try_from((turn / 2) & 1).expect("source color phase is one bit");
    (color_phase_1, color_phase_2)
}

fn level2_day_player_panel_selector(hit_frame_time: u8, color_phase_1: u8) -> u16 {
    if hit_frame_time > 0 && color_phase_1 != 0 {
        55
    } else {
        41
    }
}

const LEVEL2_DAY_HUD_PALETTE_TABLE: [[u8; 3]; 8] = [
    [0x60, 0x64, 0x7B],
    [0x7B, 0x77, 0x7B],
    [0x1C, 0x18, 0x7B],
    [0x5B, 0x57, 0x7B],
    [0x9A, 0x97, 0x7B],
    [0xDB, 0xD8, 0x7B],
    [0x76, 0xA0, 0x7B],
    [0x3D, 0x3A, 0x7B],
];

fn level2_day_hud_palette_indices(color_index: u8) -> Result<[u8; 3]> {
    LEVEL2_DAY_HUD_PALETTE_TABLE
        .get(usize::from(color_index))
        .copied()
        .ok_or_else(|| {
            NativeWorldError::new(format!(
                "Level 2 Day HUD player color {color_index} has no source palette row"
            ))
        })
}

/// Minimal authoritative projection for the Day top bar and two spell hands.
///
/// `tick` is the simulation carrier; the C ABI adds the campaign generation
/// at the handle boundary. Nyphur and local-visibility data are copied from
/// their typed reset-owned projections so the Unity route cannot silently
/// replace them with a generic participant or a Level 1 cache row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2DayHudSnapshot {
    pub tick: u64,
    pub local_player_slot: u8,
    pub local_player_entity_index: u16,
    pub local_player_color_index: u8,
    pub local_player_active: bool,
    pub hit_frame_time_pre_draw: u8,
    pub color_phase_1: u8,
    pub color_phase_2: u8,
    pub local_player_control_flags: u32,
    pub local_player_life: i32,
    pub local_player_max_life: i32,
    pub local_player_mana: i32,
    pub local_player_max_mana: i32,
    pub global_mana_denominator: i32,
    pub minimap_panel_selector: u16,
    pub castle_panel_selector: u16,
    pub balloon_panel_selector: u16,
    pub player_panel_selector: u16,
    pub player_icon_selector: u16,
    pub life_mana_icon_selector: u16,
    pub life_palette_index: u8,
    pub mana_palette_index: u8,
    pub mana_max_palette_index: u8,
    pub left: Level2DayHudSpellSlot,
    pub right: Level2DayHudSpellSlot,
    pub nyphur: Level2NyphurParticipant,
    pub nyphur_visibility: Level2NyphurVisibility,
}

/// Level 2's shared spell/economy reset projection. The two active authored
/// pickups and the AI's Fireball/Possession holders are retained as typed,
/// source-ordered records; generic acquisition states additionally root the
/// source family-6/11/12 pickup transition without claiming active lifecycles.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level2SharedSpellResetSession {
    pickups: Box<[EntitySnapshot]>,

    dynamic_pickups: Vec<EntitySnapshot>,

    authored_pickup_slots: BTreeMap<u16, u16>,

    holders: Box<[EntitySnapshot]>,

    player_mana: i32,

    player_life_regen: i32,

    player_entity: EntitySnapshot,
    /// All source player records/entities are retained for the generic
    /// `sub_68FF0` linked-list scan.  The previous seam only kept player 0,
    /// which made a pickup silently local-only.
    players: Box<[PlayerSnapshot]>,

    player_entities: Box<[EntitySnapshot]>,

    local_player: PlayerSnapshot,

    speed_pickup_state: Level2SpeedPickupState,

    caster: SpellCasterState,

    fireball: SpellActivationHolder,

    possession: SpellActivationHolder,

    speed: SpellActivationHolder,
    /// Reachable family-6 active lifecycle. Other new active families remain
    /// explicitly outside this bounded slice.
    shield: Option<ShieldState>,
    /// Reachable family-11 active lifecycle. AI visibility consumption remains
    /// explicitly outside this bounded slice.
    invisible: Option<InvisibleState>,
    /// Reachable family-12 active lifecycle. The first-pulse effect allocation
    /// is intentionally not part of class-15/action-36; holder timing and
    /// cleanup are live.
    beyond_sight: Option<BeyondSightState>,

    shield_damage_mailbox: Option<Level2ShieldDamageMailbox>,
    /// Transient player/event state consumed by the Beyond Sight first-pulse
    /// constructor. This is deliberately separate from the serialized entity
    /// record, matching the source `dword_0xA4_164x` allocation.
    beyond_sight_auxiliary: Level2PlayerEventAuxiliary,
    /// Generic source acquisition states for authored Level 2 families 6,
    /// 11, and 12.  Family-specific active lifecycles remain separate.
    acquisitions: Box<[SpellHolderAcquisitionState]>,
    /// Source `OptionsSettingFlag_24 & 0x20`. `PlayerEvents_51BB0` toggles
    /// this session-global bit before the entity passes; cold
    /// `SetSpell_6D5E0` projections then force mana regeneration to zero and
    /// the holder's current mana to one. The option is runtime input, not an
    /// owner-package field.
    free_spell_usage: bool,

    persistent_spell_experience: [i32; PLAYER_SPELL_COUNT],

    spell_experience: [i32; PLAYER_SPELL_COUNT],

    spell_levels: [i8; PLAYER_SPELL_COUNT],
    /// Persistent replacement rows can coexist with the authored ordinary
    /// source for the same family. Keep their source-local acquisition state
    /// separate instead of letting one family slot overwrite the other.
    persistent_acquisitions: BTreeMap<u16, SpellHolderAcquisitionState>,
    player_entity_sha256: String,
    events: Vec<FlightEvent>,
    pickup_events: Vec<Level2SharedSpellEvent>,
    /// Ordered producer outbox drained into the canonical campaign journal at
    /// the source-handler boundary. Unlike the two compatibility vectors
    /// above, this preserves cross-domain chronology and is never exposed as
    /// replay authority.
    pending_effects: Vec<crate::campaign::journal::CampaignEffect>,
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
    SpellAcquired {
        source_slot: u16,
        player_entity_index: u16,
        family: u8,
        holder_action: u8,
    },
    ShieldDamageApplied {
        target_entity_index: u16,
        attacker_entity_index: u16,
        damage: ShieldDamage,
    },
    ExperienceScrollCollected {
        source_slot: u16,
        player_entity_index: u16,
    },
    AbilityImproved {
        source_family: u8,
        tier: i8,
    },
}

/// One source damage mailbox entry consumed by the active family-6 holder.
/// The attacker is retained for the typed event projection even though the
/// Shield status branches only consume the damage amount.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2ShieldDamageMailbox {
    pub target_entity_index: u16,
    pub damage: i32,
    pub attacker_entity_index: u16,
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
    /// Source `SetShiftByCastle_49EC0` particle axes.  The destruction tail
    /// then overrides yaw/fov with `-8192/0x4000` before `sub_60810` and
    /// `sub_613D0` run.
    pub shift_pitch: i16,
    pub shift_roll: i16,
    pub shift_yaw: i16,
    pub shift_fov: i16,
    /// Source root words touched by `sub_5FCA0_destroy_castle_level` after
    /// the post-downgrade economy/defender walk.
    pub source_word_2e: u16,
    pub source_word_30: u16,
    /// Source Castle upgrade mailbox (`word_0x7C`/`word_0x80`).  The
    /// class-10/model-43/action-45 product writes the player id here; the
    /// action-4 root consumes it on its next source visit.
    pub source_word_7c: u16,
    pub source_word_80: u16,
    pub upgrade_holder_clear_pending: bool,
    /// Source `sub_5FF50` model-3 holder slots. The pinned stage table
    /// requires at most three defenders; inactive/dead slots are replaced in
    /// source order after a stage transition.
    pub defender_slots: [u16; 3],
    pub defender_count: u8,
    /// Source owner aggregate counters (`dword_0x12E_302` and
    /// `dword_0x13C_316`) rebuilt by `sub_5FF50`.
    pub defender_total_mana: i32,
    pub defender_total_max_mana: i32,
    /// Per-holder source `word_0x96_150` target relation: Castle root slot
    /// or the selected nearest mana-sphere slot.
    pub defender_target_spheres: [u16; 3],
    /// Source `sub_5FF50` model-15 tail slots (`array_0x5C_92`).
    pub auxiliary_slots: [u16; 34],
    pub auxiliary_count: u8,
    pub auxiliary_cooldown: u8,
    pub auxiliary_positions: [Position; 34],
    pub auxiliary_yaws: [u16; 34],
    pub auxiliary_rolls: [u16; 34],
    pub auxiliary_player_entity_indices: [u16; 34],
    pub construction_child_active: bool,
    pub defense_child_active: bool,
    pub active: bool,
}

/// The ordinary tier-0 Possession flyer created by `sub_69900`.
///
/// This deliberately carries its source class/model/action and output tuple
/// instead of sharing Castle or Fireball state. `CastPosses_65F60` only
/// retires the flyer after its model-12 allocation succeeds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2PossessionProjectile {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    /// Source `id_0x1A_26`, copied to the model-12 effect.  This is the
    /// mailbox writer identity; it is not a shortcut ownership relation.
    pub source_id_entity_index: u16,
    pub holder_entity_index: u16,
    pub source_class: u8,
    pub source_model: u8,
    pub action: u8,
    pub output_class: u8,
    pub output_model: u8,
    /// Source `word_0x96`: the arena slot selected by the one-shot
    /// `sub_67CB0` acquisition. Zero is the sentinel; keeping only the slot
    /// deliberately preserves source reuse semantics.
    pub target_source_entity_index: u16,
    /// Source control-byte bit 0x02. A failed initial acquisition is not
    /// retried on later callbacks.
    pub acquisition_attempted: bool,
    pub position: Position,
    /// Source `axis_0x9A`, projected 10,240 units from the raw parent
    /// position using the wrapping A4 yaw/pitch sums. CastPosses does not
    /// consume it, but it is part of the persistent model-1 record.
    pub axis_position: Position,
    pub yaw: i16,
    pub pitch: i16,
    /// Source roll/fov carry the desired guidance yaw/pitch separately from
    /// the currently steered yaw/pitch.
    pub roll: i16,
    pub fov: i16,
    pub actual_speed: i16,
    pub minimum_speed: i16,
    pub life: i16,
    pub max_life: i16,
    pub phase: u8,
    pub active: bool,
}

/// The class-10/model-12 ownership mailbox pulse created by `NewAdd0A0C`.
/// `PossesHitMana_320E0` re-scans the live map on every callback; it does not
/// retain the collision candidate that caused the flyer to land.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2PossessionEffect {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    /// Source `id_0x1A_26` copied from the flyer and written into every
    /// qualifying model-39 mailbox by this callback.
    pub source_id_entity_index: u16,
    pub projectile_entity_index: u16,
    pub source_class: u8,
    pub source_model: u8,
    pub action: u8,
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub life: i16,
    pub max_life: i16,
    pub phase: u8,
    pub active: bool,
}

/// The ordinary human Castle spell flyer created by `sub_69AB0`.
///
/// This class-9/model-10 row is deliberately separate from the later
/// class-3/model-2 Castle seed.  A failed root allocation leaves this flyer
/// live so the source collision callback can retry it on a later pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2CastleProjectile {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub owner_entity_index: u16,
    pub holder_entity_index: u16,
    /// Source holder `subSpellIndex_0x2E` copied by `sub_69AB0` after the
    /// successful class-9/model-10 allocation.
    pub source_subspell_index: u16,
    /// Source holder `mana_0x94` copied into the new flyer payload.
    pub source_mana: i32,
    /// Source parent `Type_str_164.byte_0x154`, transferred to the flyer and
    /// cleared on the parent only after allocation succeeds.
    pub source_a4_byte_154: u8,
    pub position: Position,
    /// Source `axis_0x9A_154x` target used after the gate-only first tick.
    pub axis_position: Position,
    pub target_position: Position,
    pub target_anchor: Position,
    pub source_class: u8,
    pub source_model: u8,
    pub source_byte_43: u8,
    pub source_byte_44: u8,
    pub source_word_96: u16,
    pub target_castle_slot: u16,
    /// Source flyer orientation used by 66D00's landing bounce.
    pub yaw: i16,
    pub pitch: i16,
    pub actual_speed: i16,
    pub minimum_speed: i16,
    pub life: i32,
    pub max_life: i32,
    pub control_flags: u32,
    pub site_rejected: bool,
    pub active: bool,
    pub drawing_disabled: bool,
}

/// The ordinary public family-0 Fireball flyer created by `sub_693F0` and
/// `sub_6DCA0`.  It remains a distinct source row because its callback owns
/// the one-shot collision gate, RNG/audio tail, and following-pass reclaim;
/// the target Bee owns only the destination mailbox transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2FireballProjectile {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub owner_entity_index: u16,
    pub holder_entity_index: u16,
    pub source_subspell_index: u16,
    pub source_mana: i32,
    pub source_class: u8,
    pub source_model: u8,
    pub action: u8,
    pub payload: i32,
    pub position: Position,
    pub axis_position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub actual_speed: i16,
    pub minimum_speed: i16,
    pub life: i16,
    pub max_life: i16,
    /// Source `dword_0x10`, copied from the parent A4 byte and consumed by
    /// the delay prefix before movement/collision/RNG.
    pub dword_0x10: i32,
    /// Source byte-0 bit 1: after the first callback the collision scan is
    /// never repeated, including when no Bee was eligible.
    pub control_flags: u32,
    /// Source word-0x2c random displacement after the collision scan.
    pub source_word_2c: i16,
    pub phase: u8,
    pub active: bool,
    pub drawing_disabled: bool,
}

/// One source-linked construction/defense child record. The root keeps the
/// current slots for quick dispatch, while this ordered chain mirrors the
/// source `next_0` walk used by state 4's model-42 scan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2CastleChild {
    pub source_entity_index: u16,
    /// Source `NewEvent_4A050` rand_0x14 seed (slot + global D41A0 rand).
    pub entity_rng: u16,
    pub id_entity_index: u16,
    /// Source allocation class.  The model-15 auxiliary tail is class 5;
    /// construction, controller, and defense children use source class 10.
    pub source_class: u8,
    pub parent_entity_index: u16,
    pub owner_entity_index: u16,
    /// Source playerEntityIndex relation.  For model-15 auxiliary rows this
    /// is copied from the Castle root id.
    pub player_entity_index: u16,
    pub position: Position,
    /// Source `axis_0x9A_154x`. The model-79 defense callback captures its
    /// pre-hover position here before applying the signed six-unit height
    /// adjustment; it is distinct from the live entity position.
    pub axis_position: Position,
    pub model: u8,
    pub action: u8,
    pub mana: i32,
    pub max_mana: i32,
    /// Source `NewEvent_4A050` life/speed defaults retained for the typed
    /// model-3/action-9 `AddBallon_60AB0` holder path.
    pub life: i32,
    pub max_life: i32,
    /// Source `actSpeed_0x82_130`. Model-15 action 121 starts at its fixed
    /// descriptor minimum of 30 and action 122 sets it to zero while aiming.
    pub speed: i16,
    /// Source `word_0x96_150`. Castle defenders use it for a ManaSphere;
    /// model-15 auxiliaries use the same raw relation for their player target.
    pub target_sphere_slot: u16,
    /// Source damage mailbox `dword_0x5E_94` / `word_0x62_98` and the two
    /// attacker relations written by `sub_23C40` / `sub_23E60`.
    pub pending_damage: i32,
    pub pending_attacker_entity_index: u16,
    pub damage_attacker_entity_index: u16,
    pub killer_entity_index: u16,
    /// Source `byte_0x3E_62`, used by `sub_5FF50`'s modulo cadence gate.
    /// This is explicitly injectable because it is not the Castle phase.
    pub source_byte_3e: u8,
    /// Source constructor cadence byte `byte_0x39_57`. A non-zero value gates
    /// the action-121 class-3 target scan.
    pub source_byte_39: u8,
    /// Source `byte_0x3B_59`, kept distinct from the source map-chain byte.
    pub source_byte_3b: u8,
    /// Source `byte_0x46_70`: the MSPRD stage for class-10/model-41, or the
    /// DATA_BUILD stage selector for class-10/model-42; it is also reused as
    /// the Cave defender's sound debounce timer in `sub_60D50`.
    pub cave_collision_cooldown: u8,
    /// Source construction callback clock (`dword_0x10_16`).
    pub source_dword_10: i32,
    /// Source construction callback height word (`word_0x2E_46`).
    pub source_word_2e: i16,
    /// Source construction callback target (`subSpellIndex_0x2A_42`).
    pub construction_target_height: i16,
    /// Source `subSpellIndex_0x2A_42` for non-construction child products.
    pub source_subspell_index: u16,
    pub yaw: u16,
    pub roll: u16,
    /// Source particle/visual selector written by `SetEntityIndexAndRot`.
    pub visual_index: u16,
    pub control_flags: u32,
    /// Source `word_0x32_50` / `word_0x34_52` links.  `chain_previous_slot`
    /// is the root or prior model-79 child; `chain_next_slot` is updated on
    /// that predecessor when the next child is committed.
    pub chain_previous_slot: u16,
    pub chain_next_slot: u16,
    /// Raw model-79 fields written by `sub_613D0`: byte `0x43` carries the
    /// selected player stage coefficient and word `0x4A` carries the BUILD
    /// stage index.  The fixed profile binds byte `0x43` to the imported
    /// player `array_0x24E[9 + level]` coefficient; it defaults to the
    /// explicit identity value `1` when that nested player export is absent.
    pub source_byte_43: u8,
    pub source_word_4a: u16,
    pub active: bool,
}

/// One row consumed by remc2's `sub_60F00` owner-mana rebuild.  The source
/// scans the active entity arena and applies an explicit class/model filter;
/// retaining that projection keeps the aggregate independent from the
/// Castle's local defender vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Level2OwnerManaRow {
    source_entity_index: u16,
    source_class: u8,
    source_model: u8,
    player_entity_index: u16,
    mana: i32,
    active: bool,
}

fn level2_owner_mana_row_qualifies(source_class: u8, source_model: u8) -> bool {
    (source_class == 3 && matches!(source_model, 2 | 3))
        || source_class == 5
        || (source_class == 10 && matches!(source_model, 39 | 45 | 58))
}

fn level2_owner_mana_rows_from_entities(entities: &[EntitySnapshot]) -> Vec<Level2OwnerManaRow> {
    entities
        .iter()
        .filter(|entity| {
            entity.active && level2_owner_mana_row_qualifies(entity.class, entity.model)
        })
        .map(|entity| Level2OwnerManaRow {
            source_entity_index: entity.index,
            source_class: entity.class,
            source_model: entity.model,
            player_entity_index: entity.source_player_entity_index(),
            mana: entity.mana,
            active: true,
        })
        .collect()
}

/// Project the active authored class-10/model-39 rows into the economy's
/// typed sphere arena.  These rows already exist in the source entity arena
/// at reset; they are not allocations and must retain their source slot,
/// entity RNG, transform, mana, control dword, and raw player relation.
///
/// `source_owner` is the lossless `playerEntityIndex_0x94` relation map built
/// by `Level2EconomyState::from_parts`.  Reusing it for both owner and player
/// relations keeps the carrier gate and `sub_5F810` selector on source-backed
/// links instead of inventing a relation from the arena slot.
fn level2_static_mana_spheres_from_entities(
    entities: &[EntitySnapshot],
    source_owner: &BTreeMap<u16, u16>,
) -> Vec<ManaSphere> {
    entities
        .iter()
        .filter_map(|entity| {
            if !entity.active || entity.class != 10 || entity.model != 39 {
                return None;
            }
            let player_entity_index = *source_owner.get(&entity.index)?;
            Some(ManaSphere {
                id: u32::from(entity.index),
                source_entity_index: entity.index,
                entity_rng: entity.entity_rng,
                position: entity.position,
                velocity_x: 0,
                velocity_y: 0,
                vertical_velocity: 0,
                yaw: entity.yaw,
                mana: entity.mana,
                owner_entity_index: player_entity_index,
                player_entity_index,
                incoming_owner_entity_index: 0,
                incoming_possession_payload: 0,
                attraction_source_entity_index: 0,
                attraction_step: 0,
                attraction_distance_high: 0,
                assigned_carrier_id: 0,
                assigned_defender_slot: 0,
                control_flags: entity.control_flags,
                phase: entity.phase,
                active: true,
                pending_delete: false,
            })
        })
        .collect()
}

fn level2_owner_mana_projection(rows: &[Level2OwnerManaRow]) -> (BTreeMap<u16, i32>, i32) {
    // `sub_60F00` seeds the global accumulator with one before the scan.
    let mut global = 1_i32;
    let mut aggregate = BTreeMap::new();
    for row in rows.iter().filter(|row| {
        row.active && level2_owner_mana_row_qualifies(row.source_class, row.source_model)
    }) {
        global = global.saturating_add(row.mana);
        // Only class-10/model-45 rows feed the owner's dword_0x13C.  The
        // other accepted rows feed the source max-mana/global counters via
        // `sub_61000`, but must not be substituted into this gate.
        if row.source_class == 10 && row.source_model == 45 && row.player_entity_index != 0 {
            let entry = aggregate.entry(row.player_entity_index).or_insert(0_i32);
            *entry = (*entry).saturating_add(row.mana);
        }
    }
    (aggregate, global)
}

/// Project the source `maxMana` sink implemented by `sub_61000`.
/// `sub_60F00` restores each imported player's baseline capacity before
/// scanning the active arena; qualifying rows then add their mana to the
/// player named by `playerEntityIndex_0x94`. Rows without an owner relation
/// still contribute to the source global counter, but cannot update a
/// player's capacity.
fn level2_owner_max_mana_projection(
    baseline: &BTreeMap<u16, i32>,
    rows: &[Level2OwnerManaRow],
) -> BTreeMap<u16, i32> {
    let mut projected = baseline.clone();
    for row in rows.iter().filter(|row| {
        row.active
            && row.player_entity_index != 0
            && level2_owner_mana_row_qualifies(row.source_class, row.source_model)
    }) {
        let entry = projected.entry(row.player_entity_index).or_insert(0_i32);
        *entry = entry.saturating_add(row.mana);
    }
    projected
}

/// The short-lived class-10/model-1/action-1 row created by the source
/// `KillEntity_1C930` tail after a creature's mana conversion.  It is kept in
/// the same dynamic arena as spheres and carriers so allocation order,
/// source-cursor reachability, and reclaim are observable in Level 2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2DeathBurst {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub parent_entity_index: u16,
    pub id_entity_index: u16,
    pub position: Position,
    pub max_life: i32,
    pub life_ticks: i16,
    pub subspell_index: u16,
    pub control_flags: u32,
    /// Source `byte_0x3E_62`, including the common UpdateEntities epilogue
    /// when a callback replaces its own fixed slot with this row.
    pub source_byte_3e: u8,
    pub active: bool,
}

/// The class-10/model-40/action-42 row allocated by
/// `sub_5E310_multiplayer_test_die` when a dying player reaches terrain.
/// Allocation success is the source transaction boundary: only then does the
/// player advance to action 3 and do live model-39 spheres rebind to this row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2PlayerDeathMarker {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub trigger_player_entity_index: u16,
    pub position: Position,
    pub dispatch_phase: u8,
    pub constructor_phase: u32,
    pub byte_38: u8,
    pub max_life: i32,
    pub life: i32,
    pub particle_parameter_index: u16,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2PlayerDeathNotification {
    pub id: u16,
    pub state: u8,
    pub timer: i16,
}

/// The class-15 row created by remc2 `sub_69250` through the family
/// `arsub_2a881e`/`AddSpellXX_XX_51120` constructor.  It deliberately lives
/// in the economy arena so it consumes and returns the same source slots as
/// Castle, sphere, carrier, and death-burst allocations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2SpellReplacement {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub parent_entity_index: u16,
    pub position: Position,
    pub family: u8,
    pub action: u8,
    /// Source `byte_0x3E_62`, initialized from the allocated source slot.
    pub phase: u8,
    pub control_flags: u32,
    pub active: bool,
}

/// A class-15 source row created by `sub_4A1E0`/`sub_4A310`, rather than a
/// reset-time entity.  Authored spell pickups consume the same `NewEvent`
/// free stack and RNG stream as every other dynamic Level 2 row, but retain
/// their authored index so the dormant-to-live transition is auditable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2SpellPickupSource {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub authored_index: u16,
    pub family: u8,
    pub action: u8,
    pub phase: u8,
    pub position: Position,
    pub active: bool,
}

/// The class-15 holder row created by the isolated direct-starting-spell
/// `sub_5CF40` path.  Holders consume source slots and map-chain links, but
/// they are inventory rows in the shared-spell arena, not world spell
/// pickups owned by the economy dispatcher.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Level2StartingSpellHolderSource {
    source_entity_index: u16,
    entity_rng: u16,
    family: u8,
    action: u8,
    phase: u8,
    position: Position,
    active: bool,
}

/// The runtime class-10/model-45/action-51 row created by an authored
/// class-10/subtype-45 group-8 event through `AddTerrainModification_50250`.
/// `sub_4A310` then applies `sub_49A30(par1)` before the row enters the
/// action-51 callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredModel45Source {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub authored_index: u16,
    pub target_group: u16,
    pub parameter_1: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub max_life: i32,
    pub life: i32,
    pub actual_speed: i16,
    pub minimum_speed: i16,
    pub subspell_index: u16,
    pub dword_0x10: i32,
    pub yaw: u16,
    pub font_type_index: u8,
    pub source_byte_38: u8,
    pub control_flags: u32,
    pub mana: i32,
    pub max_mana: i32,
    /// Source `word_0x5A_90`, bound by `SetEntityIndexAndRot_49CD0(177)`.
    pub particle_index: u16,
    /// Source `byte_0x5D_93` loaded from particle row 177 before the
    /// `sub_49A30` view-rectangle override.
    pub particle_byte_0x5d: u8,
    pub view_pitch: i16,
    pub view_yaw: i16,
    pub view_fov: i16,
    pub view_roll: i16,
    /// Source `axis_0x9A_154x.x`, unused by the action-51 producer callback.
    pub axis_x: i16,
    /// Source `axis_0x9A_154x.z`, unused by the action-51 producer callback.
    pub axis_z: i16,
    /// Source `byte_0x46_70`, the authored BUILD parameter index.
    pub byte_0x46: u8,
    /// Source `word_0x96_150`, zero for `AddTerrainModification_50250`.
    pub parent_entity_index: u16,
    /// Source `byte_0x3E_62`, incremented by the active source scheduler after
    /// every admitted callback.
    pub source_byte_3e: u8,
    pub position: Position,
    pub drawing_enabled: bool,
    pub active: bool,
}

/// The authored class-10/model-13 effect created by remc2 `sub_4EF90`.
/// Its action-19 callback is a short-lived SEARCH.DAT particle emitter; the
/// typed row keeps the source fields that affect the callback and replay
/// identity instead of reducing it to a generic timer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredModel13Source {
    pub source_entity_index: u16,
    pub entity_id: u16,
    pub entity_rng: u16,
    pub authored_index: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub subspell_index: u16,
    pub max_life: i16,
    pub life: i16,
    pub dword_0x10: i32,
    pub particle_index: u16,
    pub control_flags: u32,
    pub shift_rot_x: i16,
    pub shift_rot_y: i16,
    pub position: Position,
    pub phase: u8,
    pub drawing_enabled: bool,
    pub active: bool,
}

/// A class-10/model-14 child emitted by model-13 action 19's subtype-14
/// constructor. The source callback copies the parent identity and a yaw
/// phase into each child while the constructor owns its own RNG/lifetime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2Model13SpawnedChild {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub parent_entity_index: u16,
    pub parent_entity_id: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub max_life: i16,
    pub life: i16,
    pub max_speed: i16,
    pub actual_speed: i16,
    pub dword_0x10: i32,
    pub particle_index: u16,
    pub entity_index: i16,
    pub subtype: u8,
    pub yaw: i16,
    pub control_flags: u32,
    pub position: Position,
    pub phase: u8,
    pub drawing_enabled: bool,
    pub active: bool,
}

/// Authored Level 2 Day Bee constructed by remc2 `sub_4B590` (class 5,
/// model 2).  Bee rows are dormant authored features, not reset-pool rows;
/// the source pass allocates them when their class-11 producer group fires.
/// The state keeps the words consumed by actions 16..19 so target loss,
/// vertical tracking, recoil, RNG, and the action-18 latch remain observable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredBeeSource {
    pub source_entity_index: u16,
    pub entity_id: u16,
    pub entity_rng: u16,
    pub authored_index: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub actual_speed: i16,
    /// NewEvent's source control dword. Action-17 movement consumes bit
    /// 0x800 as the one-tick movement skip and stores the `sub_1B8C0`
    /// obstruction marker in bit 0x40000.
    pub control_flags: u32,
    /// `SetEvent144_49C70` initializes the Bee's death-conversion mana from
    /// half of its max life (the pinned constructor therefore starts at
    /// 1500, not zero).
    pub mana: i32,
    pub max_life: i16,
    pub life: i16,
    /// Source `sub_1BF90` damage mailbox. A Fireball writes these fields;
    /// action 17 consumes and clears them at the next Bee callback.
    pub pending_damage: i32,
    pub pending_attacker_entity_index: u16,
    /// Source word-0x26 after mailbox consumption.
    pub damage_attacker_entity_index: u16,
    /// Source word-0x24 copied from word-0x26 by `PreKillEntity_1C890`.
    /// This is the identity carried into the qualifying creature census.
    pub kill_credit_entity_index: u16,
    pub subspell_index: u16,
    pub dword_0x10: i32,
    pub target_entity_index: Option<u16>,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    /// Source `byte_0x3E_62`, the per-model constructor counter.
    pub source_byte_3e: u8,
    /// Source `byte_0x39_57`, derived from descriptor row 73's cycle window.
    pub source_byte_39: i16,
    /// Descriptor row 73 `word_0xE_14` used by action 18 vertical tracking.
    pub vertical_step: i16,
    /// Descriptor row 73 `word_0x1A_26` used by the action-18 recoil latch.
    pub action_window: i16,
    /// Source `StageVar1_0x48_72`, the one-based runtime StageVar index set
    /// by `sub_12330` after a successful constructor allocation.
    pub stage_var1: u8,
    /// Source `StageVar2_0x49_73`, used by scripted action 23's movement
    /// callback.
    pub stage_var2: u8,
    /// Runtime kind-2 point copied from the selected StageVar. The authored
    /// selector remains in `MissionContentSession`; this is the mutable event
    /// callback's point carrier.
    pub stage_target: Option<Position>,
    /// Most recent `PrepareEventSound_6E450` sound selected by the Bee helper.
    /// Zero means that this source has not emitted one yet.
    pub last_sound_id: u16,
    pub position: Position,
    pub particle_index: u16,
    pub shift_rot_x: u16,
    pub shift_rot_y: u16,
    pub phase: u8,
    pub drawing_enabled: bool,
    pub active: bool,
}

/// Validated constructor output from the typed authored-StageVar relation.
/// This stays private to the Level 2 reset producer so the economy allocator
/// cannot infer behavior from a reduced runtime row or bypass construction
/// success when advancing the StageVar counter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Level2BeeStageRoute {
    stage_var1: u8,
    stage_var2: u8,
    stage_target: Position,
}

fn level2_bee_source_tile(position: Position) -> (u8, u8) {
    (position.x.to_be_bytes()[0], position.y.to_be_bytes()[0])
}

fn level2_bee_slope_measure(terrain: &SessionTerrain, position: Position) -> i32 {
    let (tile_x, tile_y) = level2_bee_source_tile(position);
    let height = |x: u8, y: u8| {
        i32::from(terrain.ground_height(Position {
            x: u16::from(x) << 8,
            y: u16::from(y) << 8,
            z: 0,
        })) >> 5
    };
    let point_1 = height(tile_x, tile_y);
    let point_2 = height(tile_x.wrapping_add(1), tile_y);
    let point_3 = height(tile_x.wrapping_add(1), tile_y.wrapping_add(1));
    let point_4 = height(tile_x, tile_y.wrapping_add(1));
    (point_1 + point_4 - point_2 - point_3)
        .abs()
        .max((point_1 + point_2 - point_4 - point_3).abs())
}

fn level2_bee_probe_blocked(terrain: &SessionTerrain, candidate: Position, heading: i16) -> bool {
    // Descriptor 73's dword_0x14 is 0xffff_ffff, so the source's
    // `~dword_0x14 & material_mask` test rejects no Day material. Its
    // collision probe still has a 128-unit footprint and a Cave-only angle /
    // ceiling branch; the authored Bee constructor is Day-only, but retaining
    // that branch keeps this helper source-shaped if a preserved Cave row is
    // ever supplied.
    let mut probe = candidate;
    let mut distance = 0_i32;
    while distance <= i32::from(LEVEL2_BEE_COLLISION_HALF_EXTENT) {
        if terrain.map_type == MapType::Cave {
            let tile = crate::Tile {
                x: probe.x.to_be_bytes()[0],
                y: probe.y.to_be_bytes()[0],
            };
            if terrain.angle_flags(tile) & 8 != 0 {
                return true;
            }
            if let Some(ceiling) = terrain.ceiling_height(probe) {
                let ground = terrain.ground_height(probe);
                if i32::from(128) + i32::from(ground) + i32::from(LEVEL2_BEE_GROUND_CLEARANCE)
                    > i32::from(ceiling)
                {
                    return true;
                }
            }
        }
        distance += 256;
        crate::move_entity(&mut probe, heading, 0, 256);
    }
    false
}

fn level2_bee_retry_headings(yaw: i16) -> [i16; 4] {
    // This is the source's byte-preserving retry sequence from sub_1B8C0;
    // the third and fourth headings are not ordinary +/-341 turns.
    let yaw = i32::from(yaw);
    let first = yaw & 0x07ff;
    let second = (yaw + 341) & 0x07ff;
    let third = ((yaw.wrapping_sub(85) as u16 & 0x00ff)
        | ((((yaw.wrapping_sub(341) as u16) >> 8) & 7) << 8))
        & 0x07ff;
    let fourth = ((((yaw + 0x400) as u16) & 0x0700) | (yaw as u8 as u16)) & 0x07ff;
    [
        i16::try_from(first).expect("Bee source retry heading fits"),
        i16::try_from(second).expect("Bee source retry heading fits"),
        i16::try_from(third).expect("Bee source retry heading fits"),
        i16::try_from(fourth).expect("Bee source retry heading fits"),
    ]
}

fn level2_bee_turn_delta(from: i16, to: i16) -> i16 {
    let from = i32::from(from) & 0x07ff;
    let to = i32::from(to) & 0x07ff;
    let mut delta = to - from;
    if delta.abs() > 1024 {
        delta += if delta >= 0 { -2048 } else { 2048 };
    }
    if delta == 0 {
        return 0;
    }
    let magnitude = delta.abs().min(i32::from(LEVEL2_BEE_TURN_LIMIT));
    i16::try_from(magnitude * delta.signum()).expect("Bee source turn delta fits")
}

fn move_level2_bee_source(source: &mut Level2AuthoredBeeSource, terrain: &SessionTerrain) {
    if source.control_flags & LEVEL2_BEE_MOVEMENT_SKIP_BIT != 0 {
        source.control_flags &= !LEVEL2_BEE_MOVEMENT_SKIP_BIT;
        return;
    }

    let original = source.position;
    let original_yaw = source.yaw;
    for (attempt, heading) in level2_bee_retry_headings(original_yaw)
        .into_iter()
        .enumerate()
    {
        let mut candidate = original;
        let ground = terrain.ground_height(candidate);
        if candidate.z > ground {
            candidate.z = candidate.z.wrapping_add(LEVEL2_BEE_GROUND_RELAXATION);
        }
        if candidate.z <= ground.wrapping_add(LEVEL2_BEE_GROUND_CLEARANCE) {
            candidate.z = ground.wrapping_add(LEVEL2_BEE_GROUND_CLEARANCE);
        }
        crate::move_entity(&mut candidate, heading, 0, source.actual_speed);
        let crossed_tile = level2_bee_source_tile(candidate) != level2_bee_source_tile(original);
        let blocked = crossed_tile
            && (level2_bee_probe_blocked(terrain, candidate, heading)
                || level2_bee_slope_measure(terrain, candidate) >= LEVEL2_BEE_TERRAIN_THRESHOLD);
        if blocked {
            source.control_flags |= LEVEL2_BEE_OBSTRUCTION_BIT;
            // The source commits each failed heading before the next retry;
            // a later successful retry therefore retains the marker.
            source.yaw = heading;
            continue;
        }

        source.position = candidate;
        source.yaw = heading.wrapping_add(level2_bee_turn_delta(heading, source.roll)) & 0x07ff;
        if attempt == 0 {
            source.control_flags &= !LEVEL2_BEE_OBSTRUCTION_BIT;
        }
        return;
    }

    // Descriptor 73 has byte_0x20 bit 0 set, so the source kills a Bee when
    // every retry is blocked. Population-census decrementation is owned by
    // the broader class-5 census projection and remains outside this typed
    // economy seam.
    source.life = -1;
}

fn select_level2_bee_class3_target(
    source: &Level2AuthoredBeeSource,
    candidates: &[(u16, Position, u32)],
) -> Option<u16> {
    let radius_squared = LEVEL2_BEE_AWARENESS_RADIUS * LEVEL2_BEE_AWARENESS_RADIUS;
    let mut selected: Option<(u16, i64)> = None;
    for &(target_index, target_position, control_flags) in candidates {
        if control_flags & 0x20 != 0 {
            continue;
        }
        let dx = i32::from(
            target_position
                .x
                .wrapping_sub(source.position.x)
                .cast_signed(),
        );
        let dy = i32::from(
            target_position
                .y
                .wrapping_sub(source.position.y)
                .cast_signed(),
        );
        let distance_squared = i64::from(dx) * i64::from(dx) + i64::from(dy) * i64::from(dy);
        if distance_squared > radius_squared {
            continue;
        }
        let heading = heading_for_positions(source.position, target_position);
        if i32::from(angle_distance(i32::from(source.yaw), heading))
            >= LEVEL2_BEE_AWARENESS_HALF_ANGLE
        {
            continue;
        }
        // The source uses strict `<`, preserving the first linked-list row
        // when two candidates are equally distant.
        if selected
            .map(|(_, best_distance)| distance_squared < best_distance)
            .unwrap_or(true)
        {
            selected = Some((target_index, distance_squared));
        }
    }
    selected.map(|(target_index, _)| target_index)
}

/// Authored class-11 model-0/model-1/model-12 switch producer materialized by
/// the source `sub_4A1E0` group scan.  The authored `stage_tag` is copied into
/// the live entity id and is therefore the next activation group, while the
/// source phase/cooldown words remain mutable across outer turns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredSwitchProducerSource {
    pub source_entity_index: u16,
    pub entity_id: u16,
    pub entity_rng: u16,
    pub authored_index: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub stage_tag: u16,
    pub word_10: i16,
    /// `AddSwitchXX_50A90` initializes the source dword to zero before the
    /// class-11 stage-tag constructor copies its group id.
    pub dword_0x10: i32,
    pub position: Position,
    pub phase: u8,
    pub cooldown: u8,
    pub fired: bool,
    pub drawing_enabled: bool,
    pub active: bool,
}

/// Authored class-2/model-0/action-0 trees created by the group-31
/// `sub_4A310` producer.  The constructor is the pinned `AddTree_4AC40`
/// path: four entity-local draws seed the lifetime, two signed position
/// offsets, and the half-speed particle choice.  The owner/damage mailbox is
/// retained because `AddTree02_00_64E20` consumes it at the later callback
/// boundary where RNG-L2-0464/0465 become reachable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredTreeSource {
    pub source_entity_index: u16,
    pub entity_id: u16,
    pub entity_rng: u16,
    pub authored_index: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub dword_0x10: i32,
    pub source_byte_38: u8,
    pub max_life: i32,
    pub life: i32,
    pub control_flags: u32,
    pub owner_entity_index: u16,
    pub pending_damage: i32,
    pub particle_index: u16,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub fov: i16,
    pub position: Position,
    pub phase: u8,
    pub last_sphere_source_entity_index: u16,
    pub last_sphere_word_0x2c: i16,
    pub last_sphere_position_z: i16,
    pub drawing_enabled: bool,
    pub active: bool,
}

/// Authored class-10/model-1/action-1 effect rows created by remc2's
/// `NewAdd0A01_4E3B0` constructor.  These are short-lived presentation
/// events, but they still consume the shared source arena and map chain. The
/// typed row keeps the Quickfair callback's first sound and source RNG
/// observable without inventing a separate child arena.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredModel1Source {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub authored_index: u16,
    pub model: u8,
    pub action: u8,
    pub subspell_index: u16,
    pub max_life: i16,
    pub life: i16,
    pub control_flags: u32,
    pub particle_index: u16,
    pub last_sound_id: u16,
    pub position: Position,
    pub phase: u8,
    pub drawing_enabled: bool,
    pub active: bool,
}

/// Authored class-11/model-32 switch constructed by remc2's
/// `AddSwitch32_50B80`/`sub_4A310` path.  Unlike model-0/model-1 overlap
/// switches, action 32 consumes an objective selector (`byte_0x46`) and
/// fires its stage-tag group when any participating owner reaches state 2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredModel32Source {
    pub source_entity_index: u16,
    pub entity_id: u16,
    pub entity_rng: u16,
    pub authored_index: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub stage_tag: u16,
    pub objective_stage_index: u8,
    pub word_10: i16,
    pub max_life: i32,
    pub life: i32,
    pub control_flags: u32,
    pub source_byte_3e: u8,
    pub position: Position,
    pub phase: u8,
    pub fired: bool,
    pub drawing_enabled: bool,
    pub active: bool,
}

/// Authored class-10/model-59 checkpoint row constructed by remc2
/// `ArriveCheckpoint_4EB50`. The constructor is source-backed here: action
/// 64/model 59, two entity-local RNG draws for the 800..899 lifetime and
/// 0..16 speed, the NewEvent byte-0x3E seed, and the direct authored position
/// write. Its live action-64 callback emits mapped model-13/action-13 smoke
/// children, while the checkpoint parent intentionally remains unlinked from
/// the render map; source ordering, reset, and reclaim are real.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredModel59Source {
    pub source_entity_index: u16,
    pub entity_id: u16,
    pub entity_rng: u16,
    pub authored_index: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub max_life: i16,
    pub life: i16,
    pub actual_speed: i16,
    pub control_flags: u32,
    pub source_byte_3e: u8,
    pub position: Position,
    pub phase: u8,
    pub drawing_enabled: bool,
    pub active: bool,
}

/// A class-10/model-13/action-13 smoke child emitted by the authored
/// model-59 checkpoint callback.  This is deliberately a separate type from
/// the model-13/action-14 children emitted by the rooted model-13 family:
/// remc2 uses the same `sub_32160` motion callback but a different particle
/// threshold (74/67) and constructor path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2Model59SmokeChild {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub max_life: i16,
    pub life: i16,
    pub max_speed: i16,
    pub actual_speed: i16,
    pub dword_0x10: i32,
    pub particle_index: u16,
    pub yaw: i16,
    pub control_flags: u32,
    pub position: Position,
    pub phase: u8,
    pub drawing_enabled: bool,
    pub active: bool,
}

/// Authored class-10/model-53/action-58 row constructed by remc2 `sub_504B0`.
/// The fields include the NewEvent defaults consumed by `sub_38B90`, its
/// dword/shift transition state, and the source sound latch. The action-58
/// callback emits class-10/model-0 children through the shared child arena;
/// the older model-53 water state remains represented for reset compatibility
/// but is not touched by this source-correct callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredModel53Source {
    pub source_entity_index: u16,
    pub entity_id: u16,
    pub entity_rng: u16,
    pub authored_index: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub max_life: i16,
    pub life: i16,
    pub subspell_index: u16,
    pub actual_speed: i16,
    pub dword_0x10: i32,
    pub control_flags: u32,
    pub shift_rot_x: i16,
    pub shift_rot_y: i16,
    pub source_byte_39: i8,
    pub byte_0x43: u8,
    pub byte_0x44: u8,
    pub byte_0x46: u8,
    pub word_0x2c: i16,
    pub sprite_index: u16,
    pub last_sound_id: u16,
    pub water_spawn_requested: bool,
    pub position: Position,
    pub phase: u8,
    pub drawing_enabled: bool,
    pub active: bool,
}

/// The class-10/model-5/action-5 water splash created by remc2's
/// `NewAdd0A05_4E570` for the model-53 water branch. It is a live mapped
/// source row; its own action callback remains a later presentation slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredModel5WaterSplashSource {
    pub source_entity_index: u16,
    pub entity_id: u16,
    pub entity_rng: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub max_life: i16,
    pub life: i16,
    pub subspell_index: u16,
    pub dword_0x10: i32,
    pub control_flags: u32,
    pub sprite_index: u16,
    pub last_sound_id: u16,
    pub position: Position,
    pub phase: u8,
    pub drawing_enabled: bool,
    pub active: bool,
}

/// The authored class-10/model-6 event constructed by remc2's
/// `NewAdd0A06_4E5F0` and dispatched through `sub_31760`.  The fields mirror
/// the callback's mutable words rather than collapsing them into a generic
/// timer: the final-life countdown, height offset, particle index, source RNG,
/// and control bytes all affect the callback's branch and child allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredModel6Source {
    pub source_entity_index: u16,
    /// NewEvent_4A050's `id_0x1A` identity. It is explicit even when the
    /// qualified allocator gives it the same numeric value as the pool slot;
    /// the model-6 child copies this field, not the arena cursor implicitly.
    pub entity_id: u16,
    pub entity_rng: u16,
    pub authored_index: u16,
    pub model: u8,
    pub action: u8,
    pub subspell_index: u16,
    pub max_life: i16,
    pub life: i16,
    pub height_offset: i16,
    pub dword_0x10: i32,
    pub particle_index: u16,
    pub control_flags: u32,
    pub shift_rot_x: u16,
    pub shift_rot_y: u16,
    pub position: Position,
    pub phase: u8,
    pub active: bool,
}

/// The class-9/model-14 child emitted by `sub_31760`'s RNG `% 7` branch.
/// remc2's constructor sets the child to action 15/model 14, then the parent
/// copies its id, lifetime, height word, and particle index adjustment.  The
/// child callback is kept typed so allocation order and the short vertical
/// lifetime remain observable in the shared source arena.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2Model6SpawnedChild {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub parent_entity_index: u16,
    pub parent_entity_id: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub max_life: i16,
    pub life: i16,
    pub actual_speed: i16,
    pub min_speed: i16,
    pub max_speed: i16,
    pub yaw: i16,
    pub height_offset: i16,
    pub dword_0x10: i32,
    pub particle_index: u16,
    pub control_flags: u32,
    pub position: Position,
    pub active: bool,
}

/// The authored class-10/model-17 Meteor constructed by `AddMeteor_4ED70`.
/// The callback is the pinned `sub_32880` path: it advances the signed life
/// sentinel, emits the source sound/control write, walks SEARCH.DAT shape 0,
/// allocates class-10/model-0 children, and wraps its dword-0x10 phase at 11.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredModel17Source {
    pub source_entity_index: u16,
    pub entity_id: u16,
    pub entity_rng: u16,
    pub authored_index: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub subspell_index: u16,
    pub max_life: i16,
    pub life: i16,
    pub dword_0x10: i32,
    pub control_flags: u32,
    pub shift_rot_x: i16,
    pub shift_rot_y: i16,
    pub yaw: i16,
    pub position: Position,
    pub phase: u8,
    pub active: bool,
}

/// A class-10/model-0 child emitted by Quickfair, action 58, or Meteor through
/// the pinned `NewAdd0A00_4E320` constructor. The stored row is the
/// post-spawn state after the producer applies its source-specific
/// control/shift overrides; the action-0 terrain callback remains a separate
/// follow-up. Reusing this existing child arena preserves source order,
/// parent identity, map link, life sentinel, and reclaim behavior without a
/// new Level 2 twin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2Model17SpawnedChild {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub parent_entity_index: u16,
    pub parent_entity_id: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub subspell_index: u16,
    pub max_life: i16,
    pub life: i16,
    pub dword_0x10: i32,
    pub control_flags: u32,
    pub shift_rot_x: i16,
    pub shift_rot_y: i16,
    pub yaw: i16,
    pub position: Position,
    pub phase: u8,
    pub active: bool,
}

/// The authored class-10/model-22 wind parent created by the pinned
/// `AddWind_4F040` constructor through `sub_4A310`. The source callback moves
/// this parent and its chained model-75/action-82 rows; cross-entity pulse
/// effects remain a separate follow-up.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredModel22Source {
    pub source_entity_index: u16,
    pub entity_id: u16,
    pub entity_rng: u16,
    pub authored_index: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub word_0x2c: i16,
    pub word_0x2e: i16,
    pub byte_0x38: u8,
    pub actual_speed: i16,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub max_life: i16,
    pub life: i16,
    pub subspell_index: u16,
    pub control_flags: u32,
    pub word_0x30: i16,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub particle_index: u16,
    /// Source `array_0x52_82.pitch` from particle row 293, used as the
    /// parent's X half-width by `CompareAxisWithShift_10750`.
    pub shift_rot_x: i16,
    /// Source `array_0x52_82.roll` from particle row 293, used as the
    /// parent's Y half-width by `CompareAxisWithShift_10750`.
    pub shift_rot_y: i16,
    pub axis_position: Position,
    pub position: Position,
    pub phase: u8,
    pub active: bool,
}

/// One class-10/model-75/action-82 row cloned by `AddWind_4F040`. `ApplyEvents`
/// excludes model 75 before consulting class 10's disabled action-82 slot; it
/// hides the row while the parent action-22 callback continues to own chain
/// movement and linked expiry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2Model22SpawnedChild {
    pub source_entity_index: u16,
    pub entity_id: u16,
    pub entity_rng: u16,
    pub max_life: i16,
    pub life: i16,
    pub parent_entity_index: u16,
    pub parent_entity_id: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub segment_index: u8,
    pub word_0x2c: i16,
    pub word_0x36: i16,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub particle_index: u16,
    pub shift_rot_x: i16,
    pub shift_rot_y: i16,
    pub position: Position,
    pub phase: u8,
    pub drawing_enabled: bool,
    pub active: bool,
}

/// The authored class-10/model-76 Fire Spheres root constructed by the pinned
/// `AddFireSpheres_4F2A0` path. The linked model-77 children are stored as
/// separate rows, but the root keeps the mutable orbit/countdown words that
/// own the whole-cluster callback and expiry semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredModel76Source {
    pub source_entity_index: u16,
    pub entity_id: u16,
    pub entity_rng: u16,
    pub authored_index: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub max_life: i16,
    pub life: i16,
    pub subspell_index: u16,
    pub actual_speed: i16,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub word_0x2c: i16,
    pub font_type_index: i8,
    pub state: u8,
    pub target_entity_index: u16,
    pub target_position: Position,
    pub target_pitch: i16,
    pub target_height_offset: i16,
    pub target_control_flags: u32,
    pub target_life: i32,
    pub shift_rot_x: i16,
    pub shift_rot_y: i16,
    pub yaw: i16,
    pub pitch: i16,
    pub position: Position,
    pub phase: u8,
    pub control_flags: u32,
    pub drawing_enabled: bool,
    pub active: bool,
}

/// One linked class-10/model-77 child cloned by `AddFireSpheres_4F2A0`. The
/// root callback owns the shared orbit/update; the child row keeps the exact
/// topology, row/column indexes, and orientation words the source mutates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2Model76Child {
    pub source_entity_index: u16,
    pub entity_id: u16,
    pub entity_rng: u16,
    pub parent_entity_index: u16,
    pub parent_entity_id: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub max_life: i16,
    pub life: i16,
    pub subspell_index: u16,
    pub actual_speed: i16,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub word_0x2c: i16,
    pub row_index: u8,
    pub column_index: u8,
    pub chain_index: u8,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub fov: i16,
    /// `array_0x52_82` copied by `SetEntityIndexAndRot_49CD0` from particle
    /// row 340. These are kept separate from the orbit words above: the
    /// callback mutates yaw/pitch/roll/fov while the collision owner consumes
    /// the copied selector axes.
    pub collision_yaw: i16,
    pub collision_pitch: i16,
    pub collision_roll: i16,
    pub collision_fov: i16,
    pub shift_rot_x: i16,
    pub shift_rot_y: i16,
    pub position: Position,
    pub phase: u8,
    pub control_flags: u32,
    pub drawing_enabled: bool,
    pub active: bool,
}

/// The model-0 tail spawned when the Fire Spheres root finishes its terminal
/// countdown. This uses the exact `NewAdd0A00_4E320` constructor payload
/// instead of aliasing a different model-0 source that rewrites its fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2Model76TailSpawn {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub parent_entity_index: u16,
    pub parent_entity_id: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub subspell_index: u16,
    pub max_life: i16,
    pub life: i16,
    pub shift_rot_x: i16,
    pub shift_rot_y: i16,
    pub particle_index: u16,
    pub position: Position,
    pub phase: u8,
    pub control_flags: u32,
    pub active: bool,
}

// Presentation rows 293..=304 in the qualified Level 2 package have
// rotation speeds 70,80,90,100,110,120,130,150,180,220,280,360. The pinned
// `sub_4F1C0` derives these fixed shifts as 550/1000 and 450/1000.
const LEVEL2_MODEL22_PARTICLE_INDEXES: [u16; 12] =
    [293, 294, 295, 296, 297, 298, 299, 300, 301, 302, 303, 304];
const LEVEL2_MODEL22_SHIFT_X: [i16; 12] = [38, 44, 49, 55, 60, 66, 71, 82, 99, 121, 154, 198];
const LEVEL2_MODEL22_SHIFT_Y: [i16; 12] = [31, 36, 40, 45, 49, 54, 58, 67, 81, 99, 126, 162];
const LEVEL2_MODEL76_CHILD_COUNT: usize = 25;

const fn wrap_source_angle(angle: i16) -> i16 {
    angle & 0x07ff
}

/// A dormant authored class-14/model-3 or model-5 event materialized by the
/// generic `sub_4A310` constructor.  The mission callbacks which decide when
/// groups 5/28/31 fire remain a separate slice; once a row is selected, the
/// source constructor allocates this inert event, links it to the map, and
/// leaves the payload unchanged on each cursor visit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredClass14Source {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub authored_index: u16,
    pub model: u8,
    pub action: u8,
    pub particle: u16,
    /// `sub_51570` sets bit 0 unconditionally; `sub_51610` sets it only when
    /// the source special-settings flag 4 is enabled.  The qualified Level 2
    /// package has that flag clear for the generic model-5 rows.
    pub control_flags: u8,
    /// Model 5 applies `SetEntityShiftRot(768, 1280)`; model 3 leaves the
    /// constructor's zero shift untouched.
    pub shift_rot_x: u16,
    pub shift_rot_y: u16,
    pub activation_group: i16,
    pub stage_tag: i16,
    pub position: Position,
    pub phase: u8,
    /// Source byte-1 bit 4 (`DisableEntityDrawing04_57F10`).  This is
    /// distinct from allocation/liveness until the next UpdateEntities
    /// reclaim prologue clears the fixed row.
    pub drawing_enabled: bool,
    pub active: bool,
}

/// Authored class-5/model-3, model-4, or model-9 source rows reached by a
/// Level 2 producer.  Model 4 is the `AddArchers_4BA10` group-20 constructor;
/// model 3 and model 9 remain the model-45 group-22 creature roots.
/// group-22 callback.  The pinned constructors are live arena entities (not
/// narrative markers): model 3 is the 9,000-life creature root and model 9
/// is the 1,000-life creature root.  Their detailed action callbacks remain
/// a later creature slice, but allocation, source ordering, map linkage,
/// reset, and cursor visits are source-backed here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredClass5Source {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub authored_index: u16,
    pub model: u8,
    pub action: u8,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub actual_speed: i16,
    pub max_life: i32,
    pub life: i32,
    pub mana: i32,
    pub max_mana: i32,
    pub subspell_index: u16,
    pub dword_0x10: i32,
    /// Pending `str_0x5E_94.word_0x62_98` consumed by `sub_2A6B0`.
    pub pending_word_0x62: u16,
    /// Current target `word_0x26_38` written by `sub_2A6B0`.
    pub word_0x26: u16,
    /// Fallback/child link `word_0x24_36` written on the byte-0x3B-zero
    /// branch of `sub_2A6B0`.
    pub word_0x24: u16,
    /// Source `byte_0x3B_59`, retained for authored source mailbox branches.
    pub byte_0x3b: u8,
    /// Target relation `word_0x96_150` inspected by the action-25 wrapper.
    pub word_0x96_150: u16,
    /// Source selector `byte_0x46_70` used by the action-25 wrapper.
    pub byte_0x46_70: u8,
    pub particle_index: u16,
    pub source_byte_3e: u8,
    /// AddTrader_4C0B0's fixed byte-0x39 cadence value; other authored
    /// class-5 constructors leave this field at the NewEvent zero.
    pub source_byte_39: u8,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub fov: i16,
    pub shift_rot_x: i16,
    pub shift_rot_y: i16,
    pub position: Position,
    pub phase: u8,
    pub drawing_enabled: bool,
    pub active: bool,
}

/// Source fields required by class-5 families that are not shared by every
/// class-5 constructor. Keeping this keyed sidecar separate avoids forcing
/// unrelated model-3/model-9 rows to manufacture Trader-only state while the
/// source slot remains the single identity joining both records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredClass5RuntimeState {
    pub source_entity_index: u16,
    pub byte_0x38: u8,
    pub xtype: u8,
    pub descriptor_index: u16,
    pub pending_damage: i32,
    pub linked_head: u16,
}

/// The model-3-only words which are present in the source class-5 carrier but
/// are not part of the shared model-4/model-9/trader projection.  The source
/// slot remains the sole identity joining this sidecar to its root; no second
/// class-5 root is created here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredClass5Model3State {
    pub source_entity_index: u16,
    pub damage_dword_0x5e_94: i32,
    pub word_0x32: u16,
    pub word_0x34: u16,
    pub word_0x36: i16,
    pub control_flags: u32,
    pub byte_0x43: u8,
    pub byte_0x44: u8,
    pub word_0x2c: i16,
    pub descriptor_index: u16,
    pub last_child_root_phase: u8,
}

/// A linked child allocated by `sub_4B6F0` for a class-5/model-3 root.  The
/// raw chain words and action-local mailbox are retained because the child is
/// itself visited by the source cursor and participates in deterministic
/// damage, peer selection, movement, reset, and hashing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredClass5Model3Child {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub parent_entity_index: u16,
    pub word_0x32: u16,
    pub word_0x34: u16,
    pub word_0x36: i16,
    pub word_0x24: u16,
    pub pending_word_0x62: u16,
    pub word_0x26: u16,
    pub word_0x96: u16,
    pub damage_dword_0x5e_94: i32,
    pub dword_0x10: i32,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub actual_speed: i16,
    pub max_life: i32,
    pub life: i32,
    pub mana: i32,
    pub control_flags: u32,
    pub model: u8,
    pub action: u8,
    pub source_byte_3e: u8,
    pub byte_0x3b: u8,
    pub byte_0x43: u8,
    pub byte_0x44: u8,
    pub byte_0x46: u8,
    pub word_0x2c: i16,
    pub particle_index: u16,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub fov: i16,
    pub shift_rot_x: i16,
    pub shift_rot_y: i16,
    pub position: Position,
    pub phase: u8,
    pub drawing_enabled: bool,
    pub active: bool,
}

/// The model-4-only words owned by `AddArchers_4BA10`.  The root remains in
/// `authored_class5`; this typed sidecar supplies the linked-peer/head words,
/// constructor generation, and child-cursor guard without manufacturing a
/// second root arena.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredClass5ArcherState {
    pub source_entity_index: u16,
    pub slot_generation: u32,
    pub word_0x32: u16,
    pub word_0x34: u16,
    pub descriptor_index: u16,
    pub byte_0x38: u8,
    pub xtype: u8,
    pub pending_damage: i32,
    pub last_child_root_phase: u8,
}

/// Retained class-9/model-13 child emitted by Archer `sub_1CCE0`.  This is a
/// real shared source-arena row: its parent, source RNG, slot generation,
/// chain words, signed rotations, map position, cursor phase, and lifecycle
/// are all reset/hash/reclaim inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredClass5ArcherChild {
    pub source_entity_index: u16,
    pub entity_id: u16,
    pub entity_rng: u16,
    pub parent_entity_index: u16,
    pub parent_entity_id: u16,
    pub slot_generation: u32,
    pub word_0x32: u16,
    pub word_0x34: u16,
    pub word_0x24: u16,
    pub pending_word_0x62: u16,
    pub word_0x26: u16,
    pub word_0x96: u16,
    pub dword_0x10: i32,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub actual_speed: i16,
    pub max_life: i16,
    pub life: i16,
    pub subspell_index: u16,
    pub particle_index: u16,
    pub control_flags: u32,
    pub xtype: u8,
    pub xsubtype: u8,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub fov: i16,
    pub shift_rot_x: i16,
    pub shift_rot_y: i16,
    pub position: Position,
    pub source_byte_3e: u8,
    pub phase: u8,
    pub drawing_enabled: bool,
    pub active: bool,
}

/// A resident constructed by an ordinary Level 2 House source callback.
/// The row is owned by the shared Level 2 arena, preserving source-slot,
/// owner, parent, map, and reset identity with the other dynamic families.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2HouseResidentSource {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub parent_house_entity_index: u16,
    pub owner_entity_index: u16,
    pub attacker_entity_index: u16,
    pub position: Position,
    pub model: u8,
    pub action: u8,
    pub phase: u8,
    pub active: bool,
}

/// The separately named class-10/model-74/action-72 effect payload. The
/// class-15/action-36 `sub_6B310` callback never constructs or dispatches this
/// row; a future/other dispatcher may use this typed arena seam when its
/// pinned source route is admitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2BeyondSightPulseEffect {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub holder_entity_index: u16,
    pub parent_entity_index: u16,
    pub parent_entity_id: u16,
    pub source_class: u8,
    pub model: u8,
    pub origin_subtype: u8,
    pub subspell_index: u16,
    pub life: i16,
    pub effect_parameter: u8,
    pub actual_speed: i16,
    pub position: Position,
    pub axis_position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub sound_id: u8,
    pub active: bool,
}

/// The transient `Type_str_164` projection reserved for the separate
/// action-72/model-74 path. It is not consumed by class-15/action-36.
/// These values are not part of the serialized entity record: the outer
/// player-input latch writes the two angle words, while the entity update
/// pass advances and then consumes the byte-0x154 effect parameter.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Level2PlayerEventAuxiliary {
    pub entity_index: u16,
    pub next_entity: i16,
    pub entity_index2: i16,
    pub effect_parameter: u8,
}

/// The temporary class-10/model-54 field created by `sub_4A310` for authored
/// Level 2 rows (alias action 76 shares the same `sub_38D80` callback as
/// action 59). It writes one pending attraction source to each eligible
/// mana-sphere row, then expires through the normal source-arena reclaim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2ManaAttractionField {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub authored_index: u16,
    pub position: Position,
    pub radius_squared: u32,
    pub max_life: i16,
    pub life: i16,
    pub action: u8,
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
    /// Source `AddBallon_60AB0` class-3 transfer into the typed Castle root.
    CastleDefenderArrived {
        defender_slot: u16,
        castle_id: u16,
        mana: i32,
    },
    /// Source `AddBallon_60AB0` class-10 transfer into a defender holder.
    /// This is distinct from `CastleDefenderArrived`, whose class-3 branch
    /// deposits holder mana into the Castle root.
    CastleDefenderCollectedSphere {
        defender_slot: u16,
        sphere_id: u32,
        mana: i32,
    },
    DeathBurstSpawned {
        source_entity_index: u16,
        parent_entity_index: u16,
        id_entity_index: u16,
    },
    DeathBurstExpired {
        source_entity_index: u16,
        parent_entity_index: u16,
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
    /// Source Cave defender transition sound 22 from `sub_60D50`.
    /// The C ABI presents this through the existing Castle sound code until a
    /// versioned economy-event code is allocated.
    CastleDefenderCaveSound { defender_slot: u16, sound_id: u16 },
    FireSphereCollisionSound {
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
    /// Source `PrepareEventSound(root_slot, -1, sound)` emitted by the
    /// reachable class-5/model-3 action callbacks.
    AuthoredClass5Model3Sound {
        source_entity_index: u16,
        sound_id: u16,
    },
    /// Source `PrepareEventSound_6E450(source, player, sound)` emitted by a
    /// Level 2 economy/building/switch callback. `source_entity_index` keeps
    /// the handler's scheduler slot distinct from the exact entity argument
    /// passed to the sound preparation helper.
    PreparedSound {
        source_entity_index: u16,
        entity_index: u16,
        player_index: i16,
        sound_id: u16,
    },
}

/// Production Level 2 economy projection for the shared mana sphere, carrier,
/// Castle, damage, and bounded source allocation paths. The source arena free
/// stack is reconstructed from the validated reset rows; failed allocations
/// leave arena/object fields unchanged after the source's ordered dying-row
/// RNG and owner writes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level2EconomyState {
    free_slots: Vec<u16>,

    /// Slots skipped by an explicit source replacement-cursor decrement.
    /// `sub_5E310` performs one such decrement after its marker attempt;
    /// `sub_49F90` rebuilds and clears the exclusion on the next rebuild.
    replacement_cursor_exclusions: BTreeSet<u16>,

    /// Monotonic identity for remc2's rebuilt class/model list heads.
    /// `NewEvent_4A050` clears every head when it takes the replacement path;
    /// ordinary free-slot allocation leaves them intact.
    replacement_list_epoch: u32,

    slot_generations: BTreeMap<u16, u32>,

    slot_rng: BTreeMap<u16, u16>,

    pub(crate) global_rng: u32,
    /// Source entity[0] is the transient `sub_605E0`/`RemoveCastleStage`
    /// sentinel. Its rand_0x14 stream is independent of the Castle root.
    sentinel_rng: u16,

    source_rng: BTreeMap<u16, u16>,

    /// Raw common-header byte 0x3E for Level 2 carriers whose reused typed
    /// domain payload does not otherwise expose that source coordinate.
    /// NewEvent initializes it from the numeric slot and UpdateEntities
    /// advances it after the callback, including on a retained tombstone.
    source_phase_sidecars: BTreeMap<u16, u8>,

    source_owner: BTreeMap<u16, u16>,

    owner_class: BTreeMap<u16, u8>,

    pub(crate) owner_max_mana: BTreeMap<u16, i32>,

    owner_mana_rows: Vec<Level2OwnerManaRow>,

    owner_mana_aggregate: BTreeMap<u16, i32>,

    owner_mana_global: i32,

    spheres: Vec<ManaSphere>,

    death_bursts: Vec<Level2DeathBurst>,

    player_death_markers: Vec<Level2PlayerDeathMarker>,

    spell_replacements: Vec<Level2SpellReplacement>,

    spell_pickups: Vec<Level2SpellPickupSource>,

    authored_model45: Vec<Level2AuthoredModel45Source>,

    authored_model1: Vec<Level2AuthoredModel1Source>,

    authored_model13: Vec<Level2AuthoredModel13Source>,

    authored_bees: Vec<Level2AuthoredBeeSource>,

    bee_model2_constructor_phase: u8,

    authored_bee_kill_census: BTreeMap<u16, u32>,
    deferred_authored_bee_reclaims: BTreeSet<u16>,

    authored_switch_producers: Vec<Level2AuthoredSwitchProducerSource>,

    authored_trees: Vec<Level2AuthoredTreeSource>,

    authored_model32: Vec<Level2AuthoredModel32Source>,

    authored_model59: Vec<Level2AuthoredModel59Source>,

    model59_smoke_children: Vec<Level2Model59SmokeChild>,

    authored_model53: Vec<Level2AuthoredModel53Source>,

    authored_model5_water_splashes: Vec<Level2AuthoredModel5WaterSplashSource>,

    model13_spawned_children: Vec<Level2Model13SpawnedChild>,

    authored_model6: Vec<Level2AuthoredModel6Source>,

    model6_spawned_children: Vec<Level2Model6SpawnedChild>,

    authored_model17: Vec<Level2AuthoredModel17Source>,

    model17_spawned_children: Vec<Level2Model17SpawnedChild>,

    authored_model22: Vec<Level2AuthoredModel22Source>,

    model22_spawned_children: Vec<Level2Model22SpawnedChild>,

    authored_model76: Vec<Level2AuthoredModel76Source>,

    model76_children: Vec<Level2Model76Child>,

    model76_tail_spawns: Vec<Level2Model76TailSpawn>,

    authored_class14: Vec<Level2AuthoredClass14Source>,

    authored_class5: Vec<Level2AuthoredClass5Source>,

    authored_class5_runtime: Vec<Level2AuthoredClass5RuntimeState>,

    authored_class5_model3: Vec<Level2AuthoredClass5Model3State>,

    authored_class5_model3_children: Vec<Level2AuthoredClass5Model3Child>,

    authored_class5_archer: Vec<Level2AuthoredClass5ArcherState>,

    authored_class5_archer_children: Vec<Level2AuthoredClass5ArcherChild>,

    house_residents: Vec<Level2HouseResidentSource>,

    class5_model3_constructor_phase: u8,

    class5_model9_constructor_phase: u8,

    class5_model14_constructor_phase: u8,

    class5_model4_constructor_phase: u8,
    /// Source `array_0x10[15]` constructor counter for Castle auxiliaries.
    castle_auxiliary_constructor_phase: u8,
    /// Source `word_0x36DFE`, written by `sub_51570` for the latest model-3
    /// event and left unchanged by model 5.
    class14_model3_handle: u16,

    beyond_sight_effects: Vec<Level2BeyondSightPulseEffect>,

    mana_attraction_fields: Vec<Level2ManaAttractionField>,

    possession_projectiles: Vec<Level2PossessionProjectile>,

    possession_guidance_source_slots: Vec<u16>,

    possession_guidance_stale_records: BTreeMap<u16, Position>,

    possession_effects: Vec<Level2PossessionEffect>,
    /// Transient public Fireball products.  These are born from the live
    /// family-0 holder and therefore intentionally have no baseline twin.
    fireball_projectiles: Vec<Level2FireballProjectile>,

    carriers: Vec<ManaCarrier>,

    castle_projectiles: Vec<Level2CastleProjectile>,

    castle: Option<Level2CastleState>,

    castle_children: Vec<Level2CastleChild>,

    castle_footprint: Option<CastleBuildFootprint>,
    /// Source `RemoveCastleStage_385C0` indexes the old Castle level through
    /// DATA_BUILD00DATTAB's `byte_0x46`, rather than always using record 1.
    /// The qualified package currently binds stage 1; additional preserved
    /// TAB records can be installed through `set_castle_stage_footprint`.
    castle_stage_footprints: BTreeMap<u8, CastleBuildFootprint>,

    map_chains: RenderableMapChains,

    player_mana: i32,

    player_castle_entity_index: u16,

    player_castle_stage: u8,

    player_castle_aux_flags: u8,

    player_model: u8,

    player_terrain_flags: u8,

    player_spell_slot_2: u16,

    player_spell_holder_action: u16,

    player_spell_holder_max_action: u16,
    /// Imported `sub_60810` life coefficient per Castle stage. The owner
    /// export does not expose the nested player-state array, so fixed-profile
    /// sessions default to the source identity coefficient (256); callers
    /// preserving those bytes can bind each stage explicitly.
    castle_stage_coefficients: [u16; 8],
    /// Imported `sub_613D0` controller coefficient from the player's
    /// `array_0x24E[9 + level]`. The owner export does not expose this
    /// nested byte array, so fixed-profile sessions bind the explicit source
    /// identity value `1` unless a caller provides the preserved byte.
    castle_controller_coefficients: [u8; 8],
    events: Vec<Level2EconomyEvent>,
}

mod level2_authored_slots;
mod level2_class5;
mod level2_dynamic_economy;
mod level2_sessions;
#[cfg(test)]
use level2_sessions::validate_level2_ordinary_census;

pub(crate) const MODEL3_TARGET_RANGE: u32 = 0x1400;
pub(crate) const MODEL3_TARGET_HALF_ANGLE: i16 = 0x00aa;
pub(crate) const MODEL3_RAW_SOURCE_DISABLED_BIT: u32 = 0x20;

fn level2_next_rng16(seed: u16) -> u16 {
    seed.wrapping_mul(9_377).wrapping_add(9_439)
}

fn level2_next_rng32(seed: u32) -> u32 {
    seed.wrapping_mul(9_377).wrapping_add(9_439)
}

/// The bounded direct-start profile used by Wizard 0's authored families.
/// Families outside 0..3 remain intentionally unsupported in this slice;
/// family 2 is the pinned Castle row consumed by `SetSpell_6D5E0`.
#[must_use]
const fn level2_direct_starting_spell_profile(
    family: u8,
    tier: u8,
) -> Option<Level2SetSpellProfile> {
    let profile = match family {
        0 => match tier {
            0 => Level2SetSpellProfile {
                subspell_index: 250,
                word_0x30: 5,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 100,
                mana: 20,
            },
            1 => Level2SetSpellProfile {
                subspell_index: 160,
                word_0x30: 11,
                byte_0x3b: 0,
                mana_regen: 0,
                max_mana: 250,
                mana: 22,
            },
            2 => Level2SetSpellProfile {
                subspell_index: 80,
                word_0x30: 15,
                byte_0x3b: 1,
                mana_regen: 2_500,
                max_mana: 1_500,
                mana: 100,
            },
            _ => return None,
        },
        1 => match tier {
            0 => Level2SetSpellProfile {
                subspell_index: 10,
                word_0x30: 3,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 100,
                mana: 33,
            },
            1 => Level2SetSpellProfile {
                subspell_index: 15,
                word_0x30: 41,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 250,
                mana: 6,
            },
            2 => Level2SetSpellProfile {
                subspell_index: 20,
                word_0x30: 51,
                byte_0x3b: 1,
                mana_regen: 1_000,
                max_mana: 1_000,
                mana: 19,
            },
            _ => return None,
        },
        2 => match tier {
            0 => Level2SetSpellProfile {
                subspell_index: 0,
                word_0x30: 101,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 1_000,
                mana: 9,
            },
            1 => Level2SetSpellProfile {
                subspell_index: 0,
                word_0x30: 101,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 1_250,
                mana: 12,
            },
            2 => Level2SetSpellProfile {
                subspell_index: 1,
                word_0x30: 101,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 2_000,
                mana: 19,
            },
            _ => return None,
        },
        3 => match tier {
            0 => Level2SetSpellProfile {
                subspell_index: 2,
                word_0x30: 301,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 1_000,
                mana: 3,
            },
            1 => Level2SetSpellProfile {
                subspell_index: 3,
                word_0x30: 451,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 2_500,
                mana: 5,
            },
            2 => Level2SetSpellProfile {
                subspell_index: 4,
                word_0x30: 501,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 5_000,
                mana: 9,
            },
            _ => return None,
        },
        _ => return None,
    };
    Some(profile)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level2OrdinaryResetSession {
    house_build_footprints: BTreeMap<u8, CastleBuildFootprint>,

    houses: HouseFamily,

    villagers: VillagerFamily,

    dolmens: Box<[Level2Dolmen]>,

    players: Box<[EntitySnapshot]>,

    population_census: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Level2Group10Switch25State {
    pub(crate) source_slot: u16,
    pub(crate) authored_index: u16,
    pub(crate) target_group: u16,
    pub(crate) position: Position,
    pub(crate) phase: u8,
}

/// Callback state for a group-zero class-11 model-0/model-1 producer that is
/// already present in the imported reset entity array.  The entity remains
/// the visible/source authority; this wrapper only carries the mutable
/// callback words needed to execute the generic stage-tag chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Level2ImportedSwitchRootState {
    pub(crate) source_slot: u16,
    pub(crate) authored_index: u16,
    pub(crate) model: u8,
    pub(crate) target_group: u16,
    pub(crate) word_10: i16,
    pub(crate) position: Position,
    pub(crate) phase: u8,
    pub(crate) cooldown: u8,
    pub(crate) drawing_enabled: bool,
    pub(crate) active: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level2LiveState {
    pub(crate) core: crate::campaign::state::CanonicalRuntimeState,
    pub(crate) terrain: SessionTerrain,
    pub(crate) economy: crate::campaign::source_arena::SourceArena,
    pub(crate) bad_stones: Box<[Level2BadStone]>,
    pub(crate) shared_spells: Level2SharedSpellResetSession,
    pub(crate) ordinary: Level2OrdinaryResetSession,
    pub(crate) goats: GoatFamily,
    pub(crate) mission: MissionContentSession,
    pub(crate) nyphur_dispatcher: Level2NyphurDispatcher,
    /// Source `sub_11900` player damage mailbox.  The Bee contact writes this
    /// during its source-slot callback; the local-player route consumes it on
    /// the next source-ordered visit.
    pub(crate) player_pending_damage: i32,
    pub(crate) player_pending_attacker: u16,
    pub(crate) player_life: i32,
    /// Source per-player `MenuState_0x3DF`, distinct from the spell-fire
    /// holder's `spell_menu_stage` byte.
    pub(crate) player_menu_state: u8,
    pub(crate) player_death_notification: Option<Level2PlayerDeathNotification>,
    pub(crate) level2_group8_activated: bool,
    /// Mutable selected-objective stage state consumed by class-11/model-32
    /// switches.  The qualified reset starts at one; objective progression
    /// owns the later transition to exactly two.
    pub(crate) level2_selected_objective_stage_state: u8,
    pub(crate) level2_objective_pause: u8,
    pub(crate) level2_objective_done: bool,
    /// Authored class-10/model-54 rows already consumed by a rooted class-11
    /// stage-tag producer.  The source can run the same group again with
    /// `a2 == 0`; retaining the authored identity prevents a normal outer
    /// turn from repeatedly constructing the same temporary field.
    pub(crate) level2_mana_attraction_activated: BTreeSet<u16>,
    pub(crate) level2_model13_activated: BTreeSet<u16>,
    pub(crate) level2_bees_activated: BTreeSet<u16>,
    pub(crate) level2_switch_producers_activated: BTreeSet<u16>,
    pub(crate) level2_model32_activated: BTreeSet<u16>,
    pub(crate) level2_model76_activated: BTreeSet<u16>,
    /// Authored class-10/model-22 wind rows consumed by one-shot switches.
    /// The source disables the authored feature after dispatch, so a later
    /// parent expiry must not rematerialize the same row.
    pub(crate) level2_model22_activated: BTreeSet<u16>,
    /// Authored class-10/subtype-39 rows consumed by a live stage-tag
    /// producer. The source group scan can run again, so retain authored
    /// identity even though the typed consumers live in the sphere arena.
    pub(crate) level2_model39_activated: BTreeSet<u16>,
    /// Authored class-10/model-6 rows consumed by the rooted group-24
    /// producer. The group scan is repeatable, so keep authored identity
    /// separate from the live model-6 arena rows.
    pub(crate) level2_model6_activated: BTreeSet<u16>,
    pub(crate) level2_model59_activated: BTreeSet<u16>,
    pub(crate) level2_model53_activated: BTreeSet<u16>,
    pub(crate) level2_model1_activated: BTreeSet<u16>,
    pub(crate) level2_model17_activated: BTreeSet<u16>,
    pub(crate) level2_imported_switch_roots: Box<[Level2ImportedSwitchRootState]>,
    pub(crate) level2_group10_switch25: Option<Level2Group10Switch25State>,
    /// Authored class-14 subtype-3/5 rows already consumed by an active
    /// class-11 stage-tag producer.  The source group scan is repeatable, so
    /// retain authored identity to prevent duplicate arena rows on later
    /// source passes.
    pub(crate) level2_class14_activated: BTreeSet<u16>,
    /// Authored class-2/model-0 tree rows already consumed by the group-31
    /// LevelEnd producer.  Keep this separate from class-14 activation so a
    /// malformed partial group cannot be mistaken for a complete transition.
    pub(crate) level2_tree_activated: BTreeSet<u16>,
    /// Authored class-5/model-3/9 rows consumed by the model-45 group-22
    /// callback. Retain authored identity across repeatable source scans.
    pub(crate) level2_class5_activated: BTreeSet<u16>,
    /// Authored class-10/model-45 rows consumed by the bounded groups 3 and
    /// 8 scans. The source group scan is repeatable, but each authored row is
    /// consumed only once when `a2 == 1`.
    pub(crate) level2_model45_activated: BTreeSet<u16>,
}

impl Level2LiveState {
    pub(crate) fn reset_scratch(
        &self,
        generation: u64,
        valid: bool,
    ) -> crate::campaign::state::ScratchState {
        let (color_phase_1, color_phase_2) =
            level2_day_color_phases(self.shared_spells.local_player.turn);
        let hit_frame_time_pre_draw = self.shared_spells.local_player.record_bytes()[406];
        crate::campaign::state::ScratchState {
            validity_epoch: generation,
            valid,
            dispatch_receipt: self.core.effect_journal.receipt(),
            dispatch_events: Vec::new(),
            economy_event_cursor: self.economy.events().len(),
            nyphur_dispatch_traces: Vec::new(),
            level2_day_hud_color_phase_1: color_phase_1,
            level2_day_hud_color_phase_2: color_phase_2,
            level2_day_hud_hit_frame_time_pre_draw: hit_frame_time_pre_draw,
            level2_day_hud_player_panel_selector: level2_day_player_panel_selector(
                hit_frame_time_pre_draw,
                color_phase_1,
            ),
        }
    }
}

impl std::ops::Deref for Level2LiveState {
    type Target = crate::campaign::state::CanonicalRuntimeState;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl std::ops::DerefMut for Level2LiveState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.core
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level2ResetSession {
    pub(crate) session: crate::campaign::state::CampaignSession,
}

impl std::ops::Deref for Level2ResetSession {
    type Target = Level2LiveState;

    fn deref(&self) -> &Self::Target {
        &self.session.state.live
    }
}

impl std::ops::DerefMut for Level2ResetSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.session.state.live
    }
}

/// One lossless, source-ordered result from the bounded Level 2 dispatcher.
///
/// The source dispatcher is deliberately observable: handlers may emit typed
/// events or fail closed, and neither result is silently discarded. This is
/// the A4.1 seam; dynamic arena scanning and shared terrain authority remain
/// separate follow-on slices.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Level2DispatchEvent {
    SharedPickup {
        pass: u8,
        source_slot: u16,
        result: Option<Level2SpeedPickupStep>,
    },
    SpellAcquisition {
        pass: u8,
        source_slot: u16,
        family: u8,
        result: Option<SpellHolderAcquisitionOutcome>,
    },
    AuthoredGroupActivated {
        pass: u8,
        source_slot: u16,
        group: u16,
        authored_indices: Box<[u16]>,
    },
    BadStone {
        pass: u8,
        step: Level2BadStoneStep,
    },
    House {
        pass: u8,
        source_slot: u16,
        events: Box<[HouseStepEvent]>,
    },
    Villager {
        pass: u8,
        source_slot: u16,
        effects: Box<[VillagerStepEffect]>,
    },
    Dolmen {
        pass: u8,
        step: Level2DolmenStep,
    },
    Castle {
        pass: u8,
        source_slot: u16,
        advanced: bool,
    },
    Error {
        pass: u8,
        source_slot: u16,
        message: String,
    },
    DynamicArena {
        pass: u8,
        source_slot: u16,
        kind: Level2DynamicArenaKind,
    },
    Economy {
        pass: u8,
        source_slot: u16,
        event: Level2EconomyEvent,
    },
    Goat {
        pass: u8,
        source_slot: u16,
        events: Box<[GoatStepEvent]>,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Level2DynamicArenaKind {
    CastleProjectile,
    FireballProjectile,
    PossessionProjectile,
    PossessionEffect,
    CastleRoot,
    CastleChild,
    CastleAuxiliary,
    CastleDefender,
    CastleController,
    ManaSphere,
    ManaCarrier,
    DeathBurst,
    PlayerDeathMarker,
    SpellReplacement,
    SpellPickup,
    AuthoredModel1,
    AuthoredModel45,
    AuthoredModel13,
    AuthoredBee,
    Model13SpawnedChild,
    AuthoredModel6,
    Model6SpawnedChild,
    AuthoredModel17,
    Model17SpawnedChild,
    AuthoredModel22,
    Model22SpawnedChild,
    AuthoredModel76,
    Model76Child,
    Model76TailSpawn,
    AuthoredClass14,
    AuthoredClass5,
    AuthoredClass5ArcherChild,
    AuthoredClass5Model3Child,
    HouseResident,
    AuthoredSwitchProducer,
    AuthoredTree,
    AuthoredModel32,
    AuthoredModel59,
    Model59SmokeChild,
    AuthoredModel53,
    AuthoredModel5WaterSplash,
    BeyondSightPulseEffect,
    ManaAttraction,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Level2DynamicArenaCursorRow {
    pub source_slot: u16,
    pub generation: u32,
    pub kind: Level2DynamicArenaKind,
}

impl Level2DispatchEvent {
    /// The lossless scalar projection used by the additive campaign C ABI.
    /// Keeping this mapping in sim-core makes the Rust digest and exported
    /// event rows derive from one source instead of hashing Rust `Debug`
    /// formatting that a client cannot reproduce.
    #[must_use]
    pub fn campaign_projection(&self) -> (u8, u8, u8, u8, u16) {
        match self {
            Self::SharedPickup {
                pass,
                source_slot,
                result,
            } => (*pass, 1, 0, u8::from(result.is_some()), *source_slot),
            Self::SpellAcquisition {
                pass,
                source_slot,
                family,
                result,
            } => (*pass, 2, *family, u8::from(result.is_some()), *source_slot),
            Self::AuthoredGroupActivated {
                pass,
                source_slot,
                group,
                authored_indices,
            } => (
                *pass,
                11,
                u8::try_from(*group).unwrap_or(u8::MAX),
                u8::try_from(authored_indices.len()).unwrap_or(u8::MAX),
                *source_slot,
            ),
            Self::BadStone { pass, step } => (
                *pass,
                3,
                0,
                u8::from(step.position != step.previous_position),
                step.source_slot,
            ),
            Self::House {
                pass,
                source_slot,
                events,
            } => (*pass, 4, 0, u8::from(!events.is_empty()), *source_slot),
            Self::Villager {
                pass,
                source_slot,
                effects,
            } => (*pass, 5, 0, u8::from(!effects.is_empty()), *source_slot),
            Self::Dolmen { pass, step } => (
                *pass,
                6,
                0,
                u8::from(step.position != step.previous_position),
                step.source_slot,
            ),
            Self::Castle {
                pass,
                source_slot,
                advanced,
            } => (*pass, 7, 0, u8::from(*advanced), *source_slot),
            Self::DynamicArena {
                pass,
                source_slot,
                kind,
            } => (
                *pass,
                8,
                level2_dynamic_arena_family(*kind),
                1,
                *source_slot,
            ),
            Self::Economy {
                pass,
                source_slot,
                event,
            } => (
                *pass,
                10,
                level2_economy_event_family(*event),
                1,
                *source_slot,
            ),
            Self::Goat {
                pass,
                source_slot,
                events,
            } => (*pass, 9, 0, u8::from(!events.is_empty()), *source_slot),
            Self::Error {
                pass, source_slot, ..
            } => (*pass, 255, 0, 1, *source_slot),
        }
    }
}

#[must_use]
pub const fn level2_dynamic_arena_family(kind: Level2DynamicArenaKind) -> u8 {
    match kind {
        Level2DynamicArenaKind::CastleProjectile => 38,
        Level2DynamicArenaKind::FireballProjectile => 41,
        // Appended production family codes. Existing consumer projections
        // retain their established values.
        Level2DynamicArenaKind::PossessionProjectile => 39,
        Level2DynamicArenaKind::PossessionEffect => 40,
        Level2DynamicArenaKind::CastleRoot => 1,
        Level2DynamicArenaKind::CastleChild => 2,
        Level2DynamicArenaKind::CastleAuxiliary => 3,
        Level2DynamicArenaKind::CastleDefender => 4,
        Level2DynamicArenaKind::CastleController => 5,
        Level2DynamicArenaKind::ManaSphere => 6,
        Level2DynamicArenaKind::ManaCarrier => 7,
        Level2DynamicArenaKind::DeathBurst => 8,
        Level2DynamicArenaKind::PlayerDeathMarker => 43,
        Level2DynamicArenaKind::SpellReplacement => 9,
        Level2DynamicArenaKind::SpellPickup => 10,
        Level2DynamicArenaKind::AuthoredModel1 => 31,
        Level2DynamicArenaKind::AuthoredModel45 => 11,
        Level2DynamicArenaKind::AuthoredModel13 => 21,
        Level2DynamicArenaKind::AuthoredBee => 26,
        Level2DynamicArenaKind::Model13SpawnedChild => 22,
        Level2DynamicArenaKind::AuthoredModel6 => 15,
        Level2DynamicArenaKind::Model6SpawnedChild => 16,
        Level2DynamicArenaKind::AuthoredModel17 => 17,
        Level2DynamicArenaKind::Model17SpawnedChild => 18,
        Level2DynamicArenaKind::AuthoredModel22 => 19,
        Level2DynamicArenaKind::Model22SpawnedChild => 20,
        Level2DynamicArenaKind::AuthoredModel76 => 23,
        Level2DynamicArenaKind::Model76Child => 24,
        Level2DynamicArenaKind::Model76TailSpawn => 25,
        Level2DynamicArenaKind::AuthoredClass14 => 13,
        Level2DynamicArenaKind::AuthoredClass5 => 30,
        Level2DynamicArenaKind::AuthoredClass5ArcherChild => 37,
        Level2DynamicArenaKind::AuthoredClass5Model3Child => 36,
        Level2DynamicArenaKind::HouseResident => 35,
        Level2DynamicArenaKind::AuthoredSwitchProducer => 27,
        Level2DynamicArenaKind::AuthoredTree => 42,
        Level2DynamicArenaKind::AuthoredModel32 => 29,
        Level2DynamicArenaKind::AuthoredModel59 => 28,
        Level2DynamicArenaKind::Model59SmokeChild => 34,
        Level2DynamicArenaKind::AuthoredModel53 => 32,
        Level2DynamicArenaKind::AuthoredModel5WaterSplash => 33,
        Level2DynamicArenaKind::BeyondSightPulseEffect => 14,
        Level2DynamicArenaKind::ManaAttraction => 12,
    }
}

#[must_use]
pub const fn level2_economy_event_family(event: Level2EconomyEvent) -> u8 {
    match event {
        Level2EconomyEvent::ManaSphereSpawned { .. }
        | Level2EconomyEvent::ManaSphereClaimed { .. }
        | Level2EconomyEvent::ManaSphereCollected { .. }
        | Level2EconomyEvent::ManaSpheresMerged { .. } => 1,
        Level2EconomyEvent::ManaCarrierSpawned { .. }
        | Level2EconomyEvent::ManaCarrierArrived { .. } => 2,
        Level2EconomyEvent::CastleDefenderArrived { .. }
        | Level2EconomyEvent::CastleDefenderCollectedSphere { .. }
        | Level2EconomyEvent::CastleBuilt { .. }
        | Level2EconomyEvent::CastleDestroyed { .. }
        | Level2EconomyEvent::CastleConstructionSound { .. }
        | Level2EconomyEvent::CastleDestructionSound { .. }
        | Level2EconomyEvent::CastleDefenderCaveSound { .. }
        | Level2EconomyEvent::CastleStageDestroyed { .. } => 3,
        Level2EconomyEvent::DeathBurstSpawned { .. }
        | Level2EconomyEvent::DeathBurstExpired { .. } => 4,
        Level2EconomyEvent::DamageApplied { .. } => 5,
        Level2EconomyEvent::FireSphereCollisionSound { .. } => 6,
        Level2EconomyEvent::AuthoredClass5Model3Sound { .. } => 7,
        Level2EconomyEvent::PreparedSound { .. } => 8,
    }
}

#[must_use]
fn level2_economy_event_source_slot(event: Level2EconomyEvent, fallback: u16) -> u16 {
    let id = match event {
        Level2EconomyEvent::ManaSphereSpawned {
            source_entity_index,
            ..
        }
        | Level2EconomyEvent::ManaCarrierSpawned {
            source_entity_index,
            ..
        }
        | Level2EconomyEvent::DeathBurstSpawned {
            source_entity_index,
            ..
        }
        | Level2EconomyEvent::DeathBurstExpired {
            source_entity_index,
            ..
        }
        | Level2EconomyEvent::CastleBuilt {
            source_entity_index,
            ..
        }
        | Level2EconomyEvent::CastleDestroyed {
            source_entity_index,
            ..
        }
        | Level2EconomyEvent::CastleConstructionSound {
            source_entity_index,
            ..
        }
        | Level2EconomyEvent::CastleDestructionSound {
            source_entity_index,
            ..
        }
        | Level2EconomyEvent::FireSphereCollisionSound {
            source_entity_index,
            ..
        }
        | Level2EconomyEvent::CastleStageDestroyed {
            source_entity_index,
            ..
        } => u32::from(source_entity_index),
        Level2EconomyEvent::CastleDefenderCaveSound { defender_slot, .. } => {
            u32::from(defender_slot)
        }
        Level2EconomyEvent::ManaSphereClaimed { sphere_id, .. }
        | Level2EconomyEvent::ManaSpheresMerged {
            target_sphere_id: sphere_id,
            ..
        }
        | Level2EconomyEvent::ManaCarrierArrived {
            carrier_id: sphere_id,
            ..
        } => sphere_id,
        // The collection callback is owned by the carrier source row even
        // though the payload also names the collected sphere. Keep the
        // dispatcher slot bound to that handler, matching ApplyEvents' live
        // cursor rather than the consumed object.
        Level2EconomyEvent::ManaSphereCollected { carrier_id, .. } => carrier_id,
        Level2EconomyEvent::CastleDefenderArrived { defender_slot, .. } => u32::from(defender_slot),
        Level2EconomyEvent::CastleDefenderCollectedSphere { defender_slot, .. } => {
            u32::from(defender_slot)
        }
        Level2EconomyEvent::DamageApplied {
            target_entity_index,
            ..
        } => u32::from(target_entity_index),
        Level2EconomyEvent::AuthoredClass5Model3Sound {
            source_entity_index,
            ..
        } => u32::from(source_entity_index),
        Level2EconomyEvent::PreparedSound {
            source_entity_index,
            ..
        } => u32::from(source_entity_index),
    };
    u16::try_from(id).unwrap_or(fallback)
}

/// Source `CompareAxisWithShift_10750`: signed 16-bit X/Y deltas must be
/// strictly inside the sum of both entities' axis half-widths.
fn level2_compare_axis_with_shift(
    event_position: Position,
    event_half_x: i16,
    event_half_y: i16,
    target_position: Position,
    target_half_x: i16,
    target_half_y: i16,
) -> bool {
    let dx = i16::from_ne_bytes(
        event_position
            .x
            .wrapping_sub(target_position.x)
            .to_ne_bytes(),
    )
    .unsigned_abs();
    let dy = i16::from_ne_bytes(
        event_position
            .y
            .wrapping_sub(target_position.y)
            .to_ne_bytes(),
    )
    .unsigned_abs();
    i32::from(dx) < i32::from(event_half_x) + i32::from(target_half_x)
        && i32::from(dy) < i32::from(event_half_y) + i32::from(target_half_y)
}

/// Exact signed-axis predicate used by remc2 `sub_10630`/`sub_106C0`.
/// X/Y narrow the subtraction to signed 16 bits before `abs`; Z keeps the
/// promoted sum of the four signed 16-bit operands.
fn level2_model76_sub106c0_matches(
    source_position: Position,
    source_yaw: i16,
    source_pitch: i16,
    source_roll: i16,
    source_fov: i16,
    target_position: Position,
    target_yaw: i16,
    target_pitch: i16,
    target_roll: i16,
    target_fov: i16,
) -> bool {
    let dx = i16::from_ne_bytes(
        source_position
            .x
            .wrapping_sub(target_position.x)
            .to_ne_bytes(),
    )
    .unsigned_abs();
    let dy = i16::from_ne_bytes(
        source_position
            .y
            .wrapping_sub(target_position.y)
            .to_ne_bytes(),
    )
    .unsigned_abs();
    let dz = (i32::from(source_position.z) + i32::from(source_yaw)
        - i32::from(target_position.z)
        - i32::from(target_yaw))
    .abs();
    let pitch_limit = i32::from(source_pitch) + i32::from(target_pitch);
    let roll_limit = i32::from(source_roll) + i32::from(target_roll);
    let fov_limit = i32::from(source_fov) + i32::from(target_fov);
    pitch_limit > 0
        && roll_limit > 0
        && fov_limit > 0
        && i32::from(dx) < pitch_limit
        && i32::from(dy) < roll_limit
        && dz < fov_limit
}

/// Source model-45 MSPRD probe inside `sub_10C80(a2 = 0)`. The qualified
/// bank retains the runtime TAB dimensions and signed pixels; VGA selector 1
/// halves the dimensions once more at this call site before the source uses
/// the resulting width as its row stride.
fn level2_model76_model45_occupancy(
    bank: &CollisionMaskBank,
    selector: u16,
    source_position: Position,
) -> bool {
    let Some(mask) = bank.mask(selector) else {
        return false;
    };
    let width = i32::from(mask.width() >> 1);
    let height = i32::from(mask.height() >> 1);
    let source_x = i32::from(source_position.x) >> 8;
    let source_y = i32::from(source_position.y) >> 8;
    let mut left = source_x - (width >> 1);
    let top = source_y - (height >> 1);
    if (top + left) % 2 != 0 {
        left += 1;
    }
    let pixel_x = source_x - left;
    let pixel_y = source_y - top;
    let Some(pixel_index) = pixel_y
        .checked_mul(width)
        .and_then(|row| row.checked_add(pixel_x))
        .and_then(|index| usize::try_from(index).ok())
    else {
        return false;
    };
    mask.pixels()
        .get(pixel_index)
        .is_some_and(|pixel| *pixel != -1)
}

fn level2_model76_map_search_slots(
    map_chains: &RenderableMapChains,
    child: Level2Model76Child,
) -> Vec<u16> {
    let center_x = ((u32::from(child.position.x) + 128) >> 8) as u8;
    let center_y = ((u32::from(child.position.y) + 128) >> 8) as u8;
    let pitch = i32::from(child.collision_pitch);
    let sign_adjustment = if pitch + 255 < 0 { -255 } else { 0 };
    let radius = (pitch + 255 - sign_adjustment) >> 8;
    let mut slots = Vec::new();
    for delta_y in -radius..=radius {
        for delta_x in -radius..=radius {
            slots.extend(map_chains.entity_chain(
                i32::from(center_x).wrapping_add(delta_x) as u8,
                i32::from(center_y).wrapping_add(delta_y) as u8,
            ));
        }
    }
    slots
}

/// Exact source target filter for remc2 `sub_33810(a1x, a2x)`.
///
/// The caller supplies the wind parent's source `id_0x1A_26`; the source
/// rejects a same-id class-3 target and excludes model 2 before the movement
/// branches run.  This helper intentionally contains no movement policy so
/// later target-family slices can share the same allowlist.
fn level2_wind_sub33810_accepts(parent_entity_id: u16, target: &EntitySnapshot) -> bool {
    let class_offset = target.class.wrapping_sub(2);
    if class_offset > 8 {
        return false;
    }
    match class_offset {
        0 => matches!(target.model, 7 | 8),
        1 => target.source_id != parent_entity_id && target.model != 2,
        2 | 4 | 5 | 6 | 7 => false,
        3 => {
            if matches!(target.action, 180 | 232) {
                return false;
            }
            !matches!(target.model, 10 | 15 | 18 | 27 | 28)
        }
        8 => matches!(target.model, 13 | 14 | 39 | 57),
        _ => false,
    }
}

fn level2_nyphur_state_hash(participant: &Level2NyphurParticipant) -> String {
    let mut hasher = Sha256::new();
    hasher.update(REMC2_SPEC_COMMIT.as_bytes());
    hasher.update(b"level2-nyphur-participant-v1");
    hasher.update([participant.player_slot, u8::from(participant.is_ai)]);
    hasher.update(participant.color_index.to_le_bytes());
    hasher.update(participant.entity_index.to_le_bytes());
    hasher.update([
        participant.entity_class,
        participant.entity_model,
        participant.entity_action,
    ]);
    hasher.update(participant.entity_source_id.to_le_bytes());
    hasher.update(
        participant
            .entity_next_relation
            .unwrap_or(u16::MAX)
            .to_le_bytes(),
    );
    hasher.update(participant.entity_next_entity.to_le_bytes());
    hasher.update(participant.entity_control_flags.to_le_bytes());
    hasher.update(participant.entity_life.to_le_bytes());
    hasher.update(participant.entity_max_life.to_le_bytes());
    hasher.update(participant.entity_mana.to_le_bytes());
    hasher.update(participant.entity_max_mana.to_le_bytes());
    hasher.update(participant.fireball_holder_index.to_le_bytes());
    hasher.update([participant.fireball_action]);
    hasher.update(participant.fireball_parent_entity_index.to_le_bytes());
    hasher.update(participant.fireball_next_entity.to_le_bytes());
    hasher.update(participant.possession_holder_index.to_le_bytes());
    hasher.update([participant.possession_action]);
    hasher.update(participant.possession_parent_entity_index.to_le_bytes());
    hasher.update(participant.possession_next_entity.to_le_bytes());
    if let RuntimeRelation::LiveEntity(index) = participant.objective_stage_relation {
        hasher.update([1]);
        hasher.update(index.to_le_bytes());
    } else {
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

fn level2_nyphur_participant(
    players: &[PlayerSnapshot],
    entities: &[EntitySnapshot],
    runtime: &RuntimeSnapshot,
    baseline_entity: &EntitySnapshot,
    target_identity: impl Fn(u16) -> Option<i16>,
) -> Result<Level2NyphurParticipant> {
    let player = players
        .get(1)
        .ok_or_else(|| NativeWorldError::new("Level 2 Nyphur AI player row 1 is absent"))?;
    if player.slot != 1
        || !player.is_ai
        || player.entity_index != LEVEL2_NYPHUR_ENTITY_INDEX
        || player.player_color_index() != 1
        || player.left_spell_slot_raw != 0
        || player.right_spell_slot_raw != 1
        || player.enabled_spell_holders[0] != LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX
        || player.enabled_spell_holders[1] != LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX
        || player.learned_spell_flags
            != [
                0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
            ]
        || sha256_hex(player.legacy_record_bytes()) != LEVEL2_NYPHUR_PLAYER_SHA256
    {
        return Err(NativeWorldError::new(
            "Level 2 Nyphur player ownership/hand census row changed",
        ));
    }

    let entity = entities
        .get(usize::from(LEVEL2_NYPHUR_ENTITY_INDEX))
        .ok_or_else(|| NativeWorldError::new("Level 2 Nyphur entity row is absent"))?;
    let source_relation_is_valid = entity.source_word_96() == 0
        || target_identity(entity.source_word_96())
            .is_some_and(|identity| entity.source_word_98() == identity);
    if !entity.active
        || entity.class != 3
        || entity.model != 1
        || entity.action != 1
        || entity.source_id != LEVEL2_NYPHUR_ENTITY_INDEX
        || entity.next_relation != Some(0)
        || entity.next_entity() != LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX
        || entity.parameter_relation != Some(67)
        || entity.special_relation != EntitySpecialRelation::PlayerState(1)
        || entity.control_flags != 12
        || entity.life != 3_085
        || entity.max_life != 3_085
        || entity.mana != 1_000
        || entity.max_mana != 1_000
        || !source_relation_is_valid
        || !level2_nyphur_entity_record_matches_census(entity, baseline_entity)
    {
        return Err(NativeWorldError::new(
            "Level 2 Nyphur constructor/ownership census row changed",
        ));
    }

    let fireball = entities
        .get(usize::from(LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX))
        .ok_or_else(|| NativeWorldError::new("Level 2 Nyphur Fireball holder is absent"))?;
    if !fireball.active
        || fireball.class != 15
        || fireball.model != 0
        || fireball.action != 0
        || fireball.old_map_entity() != LEVEL2_NYPHUR_ENTITY_INDEX
        || fireball.next_entity() != LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX
        || sha256_hex(fireball.record_bytes()) != LEVEL2_NYPHUR_FIREBALL_SHA256
    {
        return Err(NativeWorldError::new(
            "Level 2 Nyphur Fireball holder relation changed",
        ));
    }

    let possession = entities
        .get(usize::from(LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX))
        .ok_or_else(|| NativeWorldError::new("Level 2 Nyphur Possession holder is absent"))?;
    if !possession.active
        || possession.class != 15
        || possession.model != 1
        || possession.action != 3
        || possession.old_map_entity() != LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX
        || possession.next_entity() != 0
        || sha256_hex(possession.record_bytes()) != LEVEL2_NYPHUR_POSSESSION_SHA256
    {
        return Err(NativeWorldError::new(
            "Level 2 Nyphur Possession holder relation changed",
        ));
    }

    let objective_stage_relation = runtime
        .stages
        .get(1)
        .ok_or_else(|| NativeWorldError::new("Level 2 Nyphur objective stage is absent"))?
        .relation;
    if objective_stage_relation != RuntimeRelation::LiveEntity(LEVEL2_NYPHUR_ENTITY_INDEX) {
        return Err(NativeWorldError::new(
            "Level 2 Nyphur objective stage relation changed",
        ));
    }

    Ok(Level2NyphurParticipant {
        player_slot: player.slot,
        entity_index: entity.index,
        is_ai: player.is_ai,
        color_index: player.player_color_index(),
        entity_class: entity.class,
        entity_model: entity.model,
        entity_action: entity.action,
        entity_source_id: entity.source_id,
        entity_next_relation: entity.next_relation,
        entity_next_entity: entity.next_entity(),
        entity_control_flags: entity.control_flags,
        entity_life: entity.life,
        entity_max_life: entity.max_life,
        entity_mana: entity.mana,
        entity_max_mana: entity.max_mana,
        fireball_holder_index: fireball.index,
        fireball_action: fireball.action,
        fireball_parent_entity_index: fireball.old_map_entity(),
        fireball_next_entity: fireball.next_entity(),
        possession_holder_index: possession.index,
        possession_action: possession.action,
        possession_parent_entity_index: possession.old_map_entity(),
        possession_next_entity: possession.next_entity(),
        objective_stage_relation,
    })
}

fn level2_nyphur_entity_record_matches_census(
    entity: &EntitySnapshot,
    baseline_entity: &EntitySnapshot,
) -> bool {
    let record = entity.record_bytes();
    if record.get(66) != Some(&entity.phase) {
        return false;
    }
    let baseline_record = baseline_entity.record_bytes();
    if baseline_record.len() != record.len() {
        return false;
    }
    let mut normalized = record.to_vec();
    // The ordinary Nyphur pass owns its entity RNG, yaw, target-heading roll,
    // position, phase, and word-0x96/0x98 target relation. Keep reset bytes for
    // the constructor hash while the caller separately validates any live
    // relation and retains every unrelated census byte.
    normalized[24..26].copy_from_slice(&baseline_record[24..26]);
    normalized[32..34].copy_from_slice(&baseline_record[32..34]);
    normalized[36..38].copy_from_slice(&baseline_record[36..38]);
    normalized[80..86].copy_from_slice(&baseline_record[80..86]);
    normalized[154..158].copy_from_slice(&baseline_record[154..158]);
    normalized[66] = LEVEL2_NYPHUR_INITIAL_PHASE;
    let mut hasher = Sha256::new();
    hasher.update(normalized);
    format!("{:x}", hasher.finalize()) == LEVEL2_NYPHUR_ENTITY_SHA256
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
    collision_masks: Option<CollisionMaskBank>,
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
        Self::inspect_level2_internal(package_dir.as_ref(), None)
    }

    /// Decode the Level 2 package and bind the explicit qualified gameplay
    /// component cache used by the production campaign C API. The component
    /// manifest carries the closure identity because the Level 2 C API does
    /// not consume a closure path.
    pub fn inspect_level2_with_owner_cache(
        package_dir: impl AsRef<Path>,
        owner_cache: impl AsRef<Path>,
    ) -> Result<NativeWorldInspection> {
        Self::inspect_level2_internal(package_dir.as_ref(), Some(owner_cache.as_ref()))
    }

    fn inspect_level2_internal(
        package_dir: &Path,
        owner_cache: Option<&Path>,
    ) -> Result<NativeWorldInspection> {
        let manifest_path = package_dir.join("manifest.json");
        let manifest_bytes = read_file(&manifest_path)?;
        let manifest: Value = serde_json::from_slice(&manifest_bytes)
            .map_err(|error| NativeWorldError::new(format!("invalid manifest JSON: {error}")))?;
        let object = as_object(&manifest, "manifest")?;
        let common = parse_common_package(package_dir, object, OwnerPackageProfile::Level2)?;
        let (level2_castle_stage_footprints, level2_house_build_footprints) = owner_cache
            .map(|owner_cache| {
                load_level2_castle_stage_footprints(owner_cache, &common.package.package_sha256)
            })
            .transpose()?
            .unwrap_or_default();
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
            collision_masks: common.collision_masks,
            level2_castle_stage_footprints,
            level2_house_build_footprints,
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
            collision_masks: _,
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
        collision_half_depth: goat.collision_half_depth,
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
    schema: OwnerPackageSchema,
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
    let expected_volatile = match schema {
        OwnerPackageSchema::V3 => {
            "player time_393 host-clock sample zeroed; secondary terrain plane preserved"
        }
        OwnerPackageSchema::V4 | OwnerPackageSchema::V5 => {
            "inactive non-Cave secondary terrain plane zeroed; player time_393 host-clock sample zeroed"
        }
        OwnerPackageSchema::V2 => {
            return Err(NativeWorldError::new(
                "Level 2 player profile requires schema v3, v4, or v5",
            ));
        }
    };
    if volatile.as_slice() != [json!(expected_volatile)] {
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

fn validate_v4_non_cave_terrain_normalization(
    members: &BTreeMap<String, Vec<u8>>,
    schema: OwnerPackageSchema,
    map_type: MapType,
) -> Result<()> {
    if !matches!(schema, OwnerPackageSchema::V4 | OwnerPackageSchema::V5)
        || map_type == MapType::Cave
    {
        return Ok(());
    }
    for name in [
        "terrain.bin",
        "terrain-pre-features.bin",
        "terrain-post-features.bin",
    ] {
        let terrain = member(members, name)?;
        if terrain.len() != TERRAIN_BYTES
            || terrain[0x40000..0x50000].iter().any(|value| *value != 0)
        {
            return Err(NativeWorldError::new(format!(
                "v4/v5 non-Cave {name} secondary terrain plane is not zero-normalized"
            )));
        }
    }
    Ok(())
}

fn validate_type_str_164_auxiliary_layout(
    player_layout: &serde_json::Map<String, Value>,
) -> Result<()> {
    let expected = json!({
        "schema": "mc2-type-str-164-auxiliary/v1",
        "record_offset": PLAYER_LEGACY_RECORD_BYTES,
        "record_bytes": TYPE_STR_164_AUXILIARY_BYTES,
        "source_struct": "Type_str_164",
        "source_struct_bytes": 1136,
        "descriptor_bounds": {
            "source_min_inclusive": 38,
            "source_max_exclusive": 580,
            "record_min_inclusive": 0,
            "record_max_exclusive": TYPE_STR_164_AUXILIARY_BYTES,
        },
        "fields": [
            {
                "name": "word_0x26_38",
                "source_offset": 38,
                "record_offset": 0,
                "count": 8,
                "element_bytes": 2,
                "bytes": 16,
                "encoding": "i16_le",
            },
            {
                "name": "byte_0x154_340",
                "source_offset": 340,
                "record_offset": 16,
                "count": 1,
                "element_bytes": 1,
                "bytes": 1,
                "encoding": "u8",
            },
            {
                "name": "dword_0x16D_365",
                "source_offset": 365,
                "record_offset": 17,
                "count": 1,
                "element_bytes": 4,
                "bytes": 4,
                "encoding": "i32_le",
            },
            {
                "name": "word_0x1A2_418",
                "source_offset": 418,
                "record_offset": 21,
                "count": 1,
                "element_bytes": 2,
                "bytes": 2,
                "encoding": "i16_le",
            },
            {
                "name": "array_0x1FC_508",
                "source_offset": 508,
                "record_offset": 23,
                "count": 32,
                "element_bytes": 2,
                "bytes": 64,
                "encoding": "u16_le",
            },
            {
                "name": "word_0x242_578",
                "source_offset": 578,
                "record_offset": 87,
                "count": 1,
                "element_bytes": 2,
                "bytes": 2,
                "encoding": "i16_le",
            },
        ],
    });
    if require_value(
        player_layout,
        "type_str_164_auxiliary",
        "record_layouts.player",
    )? != &expected
    {
        return Err(NativeWorldError::new(
            "Level 2 player Type_str_164 descriptor layout is missing, duplicated, or out of bounds",
        ));
    }
    Ok(())
}

fn validate_banishment_state_layout(
    player_layout: &serde_json::Map<String, Value>,
    reference: &serde_json::Map<String, Value>,
) -> Result<()> {
    let expected_layout = json!({
        "schema": "mc2-player-banishment-state/v1",
        "record_offset": PLAYER_LEVEL2_BANISHMENT_STATE_OFFSET,
        "record_bytes": PLAYER_LEVEL2_BANISHMENT_STATE_BYTES,
        "source_struct": "type_str_0x2BDE",
        "source_struct_bytes": 2124,
        "source_offset": 6,
        "source_field": "byte_0x006_2BE4_11236",
        "encoding": "u8",
    });
    if require_value(
        player_layout,
        "banishment_state_0x006",
        "record_layouts.player",
    )? != &expected_layout
    {
        return Err(NativeWorldError::new(
            "Level 2 player banishment-state descriptor layout is missing or changed",
        ));
    }
    let expected_reference = json!({
        "schema": "mc2-player-banishment-state/v1",
        "source_struct": "type_str_0x2BDE",
        "source_field": "byte_0x006_2BE4_11236",
        "source_offset": 6,
        "descriptor_table": "engine_support.cpp:601-613",
        "serialization": "engine_support_converts.cpp:239-249",
        "consumer": "EventsFunctions.cpp:40781-40785",
    });
    if require_value(reference, "banishment_state_0x006", "manifest.reference")?
        != &expected_reference
    {
        return Err(NativeWorldError::new(
            "Level 2 player banishment-state source reference is missing or changed",
        ));
    }
    Ok(())
}

fn validate_manifest_layouts(
    manifest: &serde_json::Map<String, Value>,
    profile: OwnerPackageProfile,
    schema: OwnerPackageSchema,
) -> Result<()> {
    let sizes = require_object(manifest, "record_sizes", "manifest")?;
    require_exact_u64(sizes, "entity", ENTITY_RECORD_BYTES as u64, "record_sizes")?;
    let (expected_player_bytes, has_type_str_164_auxiliary, has_banishment_state) =
        match (profile, schema) {
            (OwnerPackageProfile::Level1, OwnerPackageSchema::V2)
            | (OwnerPackageProfile::Level2, OwnerPackageSchema::V3) => {
                (PLAYER_LEGACY_RECORD_BYTES, false, false)
            }
            (OwnerPackageProfile::Level2, OwnerPackageSchema::V4) => {
                (PLAYER_LEVEL2_V4_RECORD_BYTES, true, false)
            }
            (OwnerPackageProfile::Level2, OwnerPackageSchema::V5) => {
                (PLAYER_LEVEL2_RECORD_BYTES, true, true)
            }
            _ => {
                return Err(NativeWorldError::new(
                    "owner package profile/schema pair is not a supported player layout",
                ));
            }
        };
    require_exact_u64(
        sizes,
        "player",
        expected_player_bytes as u64,
        "record_sizes",
    )?;
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
        ("record_bytes", expected_player_bytes),
    ] {
        require_exact_u64(player, name, expected as u64, "record_layouts.player")?;
    }
    let reference = require_object(manifest, "reference", "manifest")?;
    if has_type_str_164_auxiliary {
        validate_type_str_164_auxiliary_layout(player)?;
    } else if player.contains_key("type_str_164_auxiliary") {
        return Err(NativeWorldError::new(
            "legacy player layout must not contain the Level 2 Type_str_164 carrier",
        ));
    }
    if has_banishment_state {
        validate_banishment_state_layout(player, reference)?;
    } else if player.contains_key("banishment_state_0x006")
        || reference.contains_key("banishment_state_0x006")
    {
        return Err(NativeWorldError::new(
            "pre-v5 player layout must not contain the Level 2 banishment-state carrier",
        ));
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
    match profile {
        OwnerPackageProfile::Level1 if schema != MANIFEST_SCHEMA => {
            return Err(NativeWorldError::new(format!(
                "manifest schema must be {MANIFEST_SCHEMA} for {profile:?}"
            )));
        }
        OwnerPackageProfile::Level2
            if !matches!(
                schema,
                LEVEL2_MANIFEST_SCHEMA_V3 | LEVEL2_MANIFEST_SCHEMA_V4 | LEVEL2_MANIFEST_SCHEMA_V5
            ) =>
        {
            return Err(NativeWorldError::new(format!(
                "manifest schema must be {LEVEL2_MANIFEST_SCHEMA_V3}, {LEVEL2_MANIFEST_SCHEMA_V4}, or {LEVEL2_MANIFEST_SCHEMA_V5} for {profile:?}"
            )));
        }
        _ => {}
    }
    if require_string(manifest, "byte_order", "manifest")? != "little-endian" {
        return Err(NativeWorldError::new(
            "manifest byte_order must be little-endian",
        ));
    }

    let package = validate_package(package_dir, manifest)?;
    let authored = member(&package.members, "authored-level.bin")?.to_vec();
    let collision_masks = match (profile, package.schema) {
        (OwnerPackageProfile::Level2, OwnerPackageSchema::V4 | OwnerPackageSchema::V5) => Some(
            CollisionMaskBank::parse_source_exact(member(&package.members, "collision-masks.bin")?)
                .map_err(|error| {
                    NativeWorldError::new(format!(
                        "qualified Level 2 collision-mask bank rejected: {error:?}"
                    ))
                })?,
        ),
        _ => None,
    };
    if profile == OwnerPackageProfile::Level2 {
        validate_level_two_provenance_members(&package.members)?;
    }
    let metadata = parse_metadata(manifest)?;
    validate_profile(&metadata, profile)?;
    let counts = require_object(manifest, "counts", "manifest")?;
    validate_manifest_counts(counts, profile)?;
    validate_manifest_layouts(manifest, profile, package.schema)?;
    validate_v4_non_cave_terrain_normalization(
        &package.members,
        package.schema,
        metadata.map_type,
    )?;

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
    let players = parse_players(
        member(&package.members, "players.bin")?,
        profile,
        package.schema,
    )?;
    let runtime = parse_runtime(member(&package.members, "runtime.bin")?)?;
    let presentation = parse_presentation(member(&package.members, "presentation.bin")?)?;
    if profile == OwnerPackageProfile::Level1 {
        validate_level_one_mission_stages(&runtime)?;
        validate_level_one_mission_presentation(&presentation)?;
    } else {
        validate_level_two_profile(manifest, &runtime, &players, &entities, package.schema)?;
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
        collision_masks,
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
        LEVEL2_MANIFEST_SCHEMA_V3 => OwnerPackageSchema::V3,
        LEVEL2_MANIFEST_SCHEMA_V4 => OwnerPackageSchema::V4,
        LEVEL2_MANIFEST_SCHEMA_V5 => OwnerPackageSchema::V5,
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
        OwnerPackageSchema::V4 => &LEVEL2_V4_PACKAGE_MEMBERS,
        OwnerPackageSchema::V5 => &LEVEL2_V5_PACKAGE_MEMBERS,
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
            subspell_index: read_u16(record, 46, "entity subspell index")?,
            byte_3b: read_u8(record, 63, "entity byte 0x3b")?,
            byte_3c: read_u8(record, 64, "entity byte 0x3c")?,
            font_type_index: read_u8(record, 65, "entity font type index")?,
            active_value: read_i16(record, 50, "entity active value")?,
            activation_value: read_u16(record, 52, "entity activation value")?,
            mana_regen: read_i32(record, 140, "entity mana regen")?,
            actual_speed: read_i16(record, 134, "entity actual speed")?,
            max_mana: read_i32(record, 144, "entity max mana")?,
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

fn parse_type_str_164_auxiliary(record: &[u8]) -> Result<TypeStr164Auxiliary> {
    let carrier = record
        .get(PLAYER_LEGACY_RECORD_BYTES..PLAYER_LEVEL2_V4_RECORD_BYTES)
        .ok_or_else(|| NativeWorldError::new("Level 2 player Type_str_164 carrier is truncated"))?;
    let mut word_0x26_38 = [0_i16; 8];
    for (index, value) in word_0x26_38.iter_mut().enumerate() {
        *value = read_i16(carrier, index * 2, "Type_str_164 word_0x26_38")?;
    }
    let mut array_0x1FC_508 = [0_u16; 32];
    for (index, value) in array_0x1FC_508.iter_mut().enumerate() {
        *value = read_u16(carrier, 23 + index * 2, "Type_str_164 array_0x1FC_508")?;
    }
    Ok(TypeStr164Auxiliary {
        word_0x26_38,
        byte_0x154_340: read_u8(carrier, 16, "Type_str_164 byte_0x154_340")?,
        dword_0x16D_365: read_i32(carrier, 17, "Type_str_164 dword_0x16D_365")?,
        word_0x1A2_418: read_i16(carrier, 21, "Type_str_164 word_0x1A2_418")?,
        array_0x1FC_508,
        word_0x242_578: read_i16(carrier, 87, "Type_str_164 word_0x242_578")?,
    })
}

fn parse_players(
    bytes: &[u8],
    profile: OwnerPackageProfile,
    schema: OwnerPackageSchema,
) -> Result<Vec<PlayerSnapshot>> {
    let (version, record_size, record_size_u16, has_type_str_164_auxiliary, has_banishment_state) =
        match (profile, schema) {
            (OwnerPackageProfile::Level1, OwnerPackageSchema::V2)
            | (OwnerPackageProfile::Level2, OwnerPackageSchema::V3) => (
                2,
                PLAYER_LEGACY_RECORD_BYTES,
                PLAYER_LEGACY_RECORD_BYTES_U16,
                false,
                false,
            ),
            (OwnerPackageProfile::Level2, OwnerPackageSchema::V4) => (
                3,
                PLAYER_LEVEL2_V4_RECORD_BYTES,
                PLAYER_LEVEL2_V4_RECORD_BYTES_U16,
                true,
                false,
            ),
            (OwnerPackageProfile::Level2, OwnerPackageSchema::V5) => (
                4,
                PLAYER_LEVEL2_RECORD_BYTES,
                PLAYER_LEVEL2_RECORD_BYTES_U16,
                true,
                true,
            ),
            _ => {
                return Err(NativeWorldError::new(
                    "owner package profile/schema pair is not a supported player layout",
                ));
            }
        };
    validate_header(
        bytes,
        *b"MCP1",
        version,
        record_size_u16,
        PLAYER_COUNT_U32,
        "players.bin",
    )?;
    let mut players = Vec::with_capacity(PLAYER_COUNT);
    for slot in 0..PLAYER_COUNT {
        let record = record(bytes, 12, record_size, slot, "players.bin")?;
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
        let mut learned_spell_flags = [0_u8; PLAYER_SPELL_COUNT];
        learned_spell_flags.copy_from_slice(&record[424..450]);
        let mut spell_enabled_flags = [0_u8; PLAYER_SPELL_COUNT];
        spell_enabled_flags.copy_from_slice(&record[450..476]);
        let mut family_selected_tiers = [0_i8; PLAYER_SPELL_COUNT];
        for (tier, value) in family_selected_tiers.iter_mut().zip(&record[528..554]) {
            *tier = value.cast_signed();
        }
        let mut persistent_spell_experience = [0_i32; PLAYER_SPELL_COUNT];
        for (index, experience) in persistent_spell_experience.iter_mut().enumerate() {
            *experience = read_i32(
                record,
                60 + index * std::mem::size_of::<i32>(),
                "persistent spell experience",
            )?;
        }
        let mut runtime_spell_experience = [0_i32; PLAYER_SPELL_COUNT];
        for (index, experience) in runtime_spell_experience.iter_mut().enumerate() {
            *experience = read_i32(
                record,
                164 + index * std::mem::size_of::<i32>(),
                "runtime spell experience",
            )?;
        }
        let mut spell_unlocked_levels = [0_i8; PLAYER_SPELL_COUNT];
        for (index, level) in spell_unlocked_levels.iter_mut().enumerate() {
            let raw = read_u8(record, 502 + index, "unlocked spell level")?;
            if raw > 2 {
                return Err(NativeWorldError::new(format!(
                    "player {slot} unlocked spell family {index} has invalid level {raw}"
                )));
            }
            *level = raw.cast_signed();
        }
        let type_str_164_auxiliary = has_type_str_164_auxiliary
            .then(|| parse_type_str_164_auxiliary(record))
            .transpose()?;
        let banishment_state_0x006 = has_banishment_state
            .then(|| {
                read_u8(
                    record,
                    PLAYER_LEVEL2_BANISHMENT_STATE_OFFSET,
                    "Level 2 banishment state",
                )
            })
            .transpose()?;
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
            learned_spell_flags,
            spell_enabled_flags,
            family_selected_tiers,
            persistent_spell_experience,
            runtime_spell_experience,
            spell_unlocked_levels,
            type_str_164_auxiliary,
            banishment_state_0x006,
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
        objective_state: crate::campaign::objectives::ObjectiveState::new(
            read_u8(bytes, 65, "runtime stage index")?,
            read_u8(bytes, 67, "runtime objective presentation phase")?,
            read_u8(bytes, 69, "runtime objective textbox ticks")?,
            objectives.into_boxed_slice(),
        ),
        stages: stages.into_boxed_slice(),
        stage_variables: stage_variables.into_boxed_slice(),
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
    let player = record(players, 12, PLAYER_LEGACY_RECORD_BYTES, 0, "players.bin")?;
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
        PLAYER_LEGACY_RECORD_BYTES,
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

/// Load the seven Castle stage BUILD rows from one explicitly selected
/// Level 2 gameplay component.  The C API resolves an aggregate owner cache
/// to this component before calling the core, so this reader never interprets
/// an aggregate manifest.
fn validate_level2_v5_owner_cache_identity(
    profile: &serde_json::Map<String, Value>,
    bindings: &serde_json::Map<String, Value>,
) -> Result<()> {
    let owner_package = require_object(profile, "owner_package", "Level 2 gameplay cache profile")?;
    if require_string(
        owner_package,
        "manifest_schema",
        "Level 2 gameplay owner package",
    )? != LEVEL2_QUALIFIED_OWNER_MANIFEST_SCHEMA
    {
        return Err(NativeWorldError::new(
            "Level 2 gameplay cache owner package schema mismatch",
        ));
    }
    let players_component = require_object(
        owner_package,
        "players_component",
        "Level 2 gameplay owner package",
    )?;
    if require_string(
        players_component,
        "magic",
        "Level 2 gameplay owner players component",
    )? != LEVEL2_QUALIFIED_PLAYERS_COMPONENT_MAGIC
    {
        return Err(NativeWorldError::new(
            "Level 2 gameplay cache owner players magic mismatch",
        ));
    }
    for (name, expected) in [
        ("version", LEVEL2_QUALIFIED_PLAYERS_COMPONENT_VERSION),
        (
            "record_bytes",
            LEVEL2_QUALIFIED_PLAYERS_COMPONENT_RECORD_BYTES,
        ),
        ("count", LEVEL2_QUALIFIED_PLAYERS_COMPONENT_COUNT),
    ] {
        require_exact_u64(
            players_component,
            name,
            expected,
            "Level 2 gameplay owner players component",
        )?;
    }
    if require_string(
        bindings,
        "owner_manifest_schema",
        "Level 2 gameplay cache bindings",
    )? != LEVEL2_QUALIFIED_OWNER_MANIFEST_SCHEMA
        || require_string(
            bindings,
            "players_component_magic",
            "Level 2 gameplay cache bindings",
        )? != LEVEL2_QUALIFIED_PLAYERS_COMPONENT_MAGIC
    {
        return Err(NativeWorldError::new(
            "Level 2 gameplay cache owner binding schema mismatch",
        ));
    }
    for (name, expected) in [
        (
            "players_component_version",
            LEVEL2_QUALIFIED_PLAYERS_COMPONENT_VERSION,
        ),
        (
            "players_component_record_bytes",
            LEVEL2_QUALIFIED_PLAYERS_COMPONENT_RECORD_BYTES,
        ),
        (
            "players_component_count",
            LEVEL2_QUALIFIED_PLAYERS_COMPONENT_COUNT,
        ),
    ] {
        require_exact_u64(bindings, name, expected, "Level 2 gameplay cache bindings")?;
    }
    Ok(())
}

fn load_level2_castle_stage_footprints(
    root: &Path,
    package_sha256: &str,
) -> Result<(
    BTreeMap<u8, CastleBuildFootprint>,
    BTreeMap<u8, CastleBuildFootprint>,
)> {
    let manifest_bytes = read_owner_cache_output(root, "manifest.json")?;
    let manifest_value: Value = serde_json::from_slice(&manifest_bytes).map_err(|error| {
        NativeWorldError::new(format!(
            "invalid Level 2 gameplay cache manifest JSON: {error}"
        ))
    })?;
    let claimed_manifest_sha256 = require_hash_string(
        as_object(&manifest_value, "Level 2 gameplay cache manifest")?,
        "manifest_payload_sha256",
        "Level 2 gameplay cache manifest",
    )?;
    let mut unsigned_manifest = manifest_value.clone();
    as_object_mut(&mut unsigned_manifest, "Level 2 gameplay cache manifest")?
        .remove("manifest_payload_sha256");
    let canonical_manifest = serde_json::to_string_pretty(&unsigned_manifest).map_err(|error| {
        NativeWorldError::new(format!(
            "cannot encode Level 2 gameplay cache manifest canonically: {error}"
        ))
    })? + "\n";
    if sha256_hex(canonical_manifest.as_bytes()) != claimed_manifest_sha256 {
        return Err(NativeWorldError::new(
            "Level 2 gameplay cache manifest metadata hash mismatch",
        ));
    }
    let manifest = as_object(&manifest_value, "Level 2 gameplay cache manifest")?;
    if require_string(manifest, "schema", "Level 2 gameplay cache manifest")?
        != LEVEL2_GAMEPLAY_CACHE_SCHEMA
    {
        return Err(NativeWorldError::new(
            "Level 2 owner cache is not a gameplay component",
        ));
    }
    let content_sha256 = require_hash_string(
        manifest,
        "cache_content_sha256",
        "Level 2 gameplay cache manifest",
    )?;
    let bindings = require_object(manifest, "bindings", "Level 2 gameplay cache manifest")?;
    if require_hash_string(
        bindings,
        "level_export_package_sha256",
        "Level 2 gameplay cache bindings",
    )? != package_sha256
        || require_string(bindings, "remc2_commit", "Level 2 gameplay cache bindings")?
            != LEVEL2_UPSTREAM_COMMIT
        || require_hash_string(
            bindings,
            "level_asset_closure_sha256",
            "Level 2 gameplay cache bindings",
        )? != LEVEL2_QUALIFIED_CLOSURE_SHA256
        || require_hash_string(
            bindings,
            "owner_catalog_file_sha256",
            "Level 2 gameplay cache bindings",
        )? != LEVEL2_QUALIFIED_CATALOG_FILE_SHA256
        || require_hash_string(
            bindings,
            "owner_catalog_sha256",
            "Level 2 gameplay cache bindings",
        )? != LEVEL2_QUALIFIED_CATALOG_SHA256
    {
        return Err(NativeWorldError::new(
            "Level 2 gameplay cache package or remc2 binding mismatch",
        ));
    }
    let closure_sha256 = require_hash_string(
        bindings,
        "level_asset_closure_sha256",
        "Level 2 gameplay cache bindings",
    )?;
    let profile = require_object(manifest, "profile", "Level 2 gameplay cache manifest")?;
    if require_string(profile, "name", "Level 2 gameplay cache profile")? != "level2-day"
        || require_exact_u64(
            profile,
            "external_level",
            2,
            "Level 2 gameplay cache profile",
        )
        .is_err()
        || require_exact_u64(
            profile,
            "internal_level",
            1,
            "Level 2 gameplay cache profile",
        )
        .is_err()
        || require_hash_string(profile, "package_sha256", "Level 2 gameplay cache profile")?
            != package_sha256
        || require_hash_string(profile, "closure_sha256", "Level 2 gameplay cache profile")?
            != closure_sha256
        || require_hash_string(profile, "closure_sha256", "Level 2 gameplay cache profile")?
            != LEVEL2_QUALIFIED_CLOSURE_SHA256
        || require_hash_string(
            profile,
            "catalog_file_sha256",
            "Level 2 gameplay cache profile",
        )? != LEVEL2_QUALIFIED_CATALOG_FILE_SHA256
        || require_hash_string(profile, "catalog_sha256", "Level 2 gameplay cache profile")?
            != LEVEL2_QUALIFIED_CATALOG_SHA256
        || require_string(profile, "spec_commit", "Level 2 gameplay cache profile")?
            != LEVEL2_UPSTREAM_COMMIT
    {
        return Err(NativeWorldError::new(
            "Level 2 gameplay cache profile binding mismatch",
        ));
    }
    let map_type = require_object(profile, "map_type", "Level 2 gameplay cache profile")?;
    if require_exact_u64(map_type, "id", 0, "Level 2 gameplay cache map type").is_err()
        || require_string(map_type, "name", "Level 2 gameplay cache map type")? != "Day"
    {
        return Err(NativeWorldError::new(
            "Level 2 gameplay cache map profile mismatch",
        ));
    }
    validate_level2_v5_owner_cache_identity(profile, bindings)?;

    let mut outputs = BTreeMap::<String, (String, u64)>::new();
    let mut content_identity = String::new();
    for output in require_array(manifest, "outputs", "Level 2 gameplay cache manifest")? {
        let output = as_object(output, "Level 2 gameplay cache output")?;
        let path = require_string(output, "path", "Level 2 gameplay cache output")?.to_owned();
        let hash =
            require_hash_string(output, "sha256", "Level 2 gameplay cache output")?.to_owned();
        let bytes = require_value(output, "bytes", "Level 2 gameplay cache output")?
            .as_u64()
            .ok_or_else(|| {
                NativeWorldError::new("Level 2 gameplay cache output size is not u64")
            })?;
        use std::fmt::Write as _;
        writeln!(&mut content_identity, "{path}\0{hash}\0{bytes}")
            .expect("writing to a String cannot fail");
        if outputs.insert(path, (hash, bytes)).is_some() {
            return Err(NativeWorldError::new(
                "Level 2 gameplay cache manifest has duplicate output paths",
            ));
        }
    }
    if sha256_hex(content_identity.as_bytes()) != content_sha256 {
        return Err(NativeWorldError::new(
            "Level 2 gameplay cache output identity mismatch",
        ));
    }
    let read_output = |path: &str| -> Result<Vec<u8>> {
        let (expected_hash, expected_bytes) = outputs.get(path).ok_or_else(|| {
            NativeWorldError::new(format!(
                "Level 2 gameplay cache output is undeclared: {path}"
            ))
        })?;
        let bytes = read_owner_cache_output(root, path)?;
        if bytes.len() as u64 != *expected_bytes || sha256_hex(&bytes) != *expected_hash {
            return Err(NativeWorldError::new(format!(
                "Level 2 gameplay cache output hash/size mismatch: {path}"
            )));
        }
        Ok(bytes)
    };
    let read_qualified_build_output =
        |path: &str, qualified_hash: &str, qualified_bytes: u64| -> Result<Vec<u8>> {
            let (declared_hash, declared_bytes) = outputs.get(path).ok_or_else(|| {
                NativeWorldError::new(format!(
                    "Level 2 qualified BUILD output is undeclared: {path}"
                ))
            })?;
            if declared_hash != qualified_hash || *declared_bytes != qualified_bytes {
                return Err(NativeWorldError::new(format!(
                    "Level 2 qualified BUILD output declaration mismatch: {path}"
                )));
            }
            let bytes = read_output(path)?;
            if bytes.len() as u64 != qualified_bytes || sha256_hex(&bytes) != qualified_hash {
                return Err(NativeWorldError::new(format!(
                    "Level 2 qualified BUILD output identity mismatch: {path}"
                )));
            }
            Ok(bytes)
        };
    let tab = read_qualified_build_output(
        "gameplay/DATA_BUILD0-0.TAB.bin",
        LEVEL2_QUALIFIED_BUILD_TAB_SHA256,
        LEVEL2_QUALIFIED_BUILD_TAB_BYTES,
    )?;
    let dat = read_qualified_build_output(
        "gameplay/DATA_BUILD0-0.DAT.bin",
        LEVEL2_QUALIFIED_BUILD_DAT_SHA256,
        LEVEL2_QUALIFIED_BUILD_DAT_BYTES,
    )?;
    let mut footprints = BTreeMap::new();
    let mut previous_end = None;
    for stage in 1_u8..=7 {
        let record_offset = usize::from(stage) * 6;
        let data_offset = usize::try_from(read_u32(
            &tab,
            record_offset,
            "Level 2 Castle BUILD TAB offset",
        )?)
        .map_err(|_| NativeWorldError::new("Level 2 Castle BUILD TAB offset exceeds usize"))?;
        let width = read_u8(&tab, record_offset + 4, "Level 2 Castle BUILD TAB width")?;
        let height = read_u8(&tab, record_offset + 5, "Level 2 Castle BUILD TAB height")?;
        let expected_dimensions = level2_castle_removal_dimensions(stage);
        if (width, height) != expected_dimensions {
            return Err(NativeWorldError::new(format!(
                "Level 2 Castle BUILD stage {stage} dimensions changed: {width}x{height}"
            )));
        }
        if previous_end.is_some_and(|end| data_offset != end) {
            return Err(NativeWorldError::new(format!(
                "Level 2 Castle BUILD stage {stage} is not contiguous in DATA_BUILD0-0.DAT"
            )));
        }
        let payload_bytes = usize::from(width)
            .checked_mul(usize::from(height))
            .and_then(|cells| cells.checked_mul(2))
            .ok_or_else(|| NativeWorldError::new("Level 2 Castle BUILD payload size overflow"))?;
        let data_end = data_offset
            .checked_add(payload_bytes)
            .filter(|end| *end <= dat.len())
            .ok_or_else(|| {
                NativeWorldError::new(format!(
                    "Level 2 Castle BUILD stage {stage} escapes DATA_BUILD0-0.DAT"
                ))
            })?;
        let footprint = CastleBuildFootprint::from_owner_cells(
            stage,
            width,
            height,
            &dat[data_offset..data_end],
        )
        .map_err(NativeWorldError::new)?;
        footprints.insert(stage, footprint);
        previous_end = Some(data_end);
    }
    let mut house_footprints = BTreeMap::new();
    for building_index in [26_u8, 29, 30, 44, 50, 55, 59, 60, 64] {
        if HouseParameter::level_two(building_index).is_none() {
            return Err(NativeWorldError::new(format!(
                "Level 2 House BUILD selector {building_index} has no qualified BLDGPRM row"
            )));
        }
        let record_offset = usize::from(building_index) * 6;
        let data_offset = usize::try_from(read_u32(
            &tab,
            record_offset,
            "Level 2 House BUILD TAB offset",
        )?)
        .map_err(|_| NativeWorldError::new("Level 2 House BUILD TAB offset exceeds usize"))?;
        let width = read_u8(&tab, record_offset + 4, "Level 2 House BUILD TAB width")?;
        let height = read_u8(&tab, record_offset + 5, "Level 2 House BUILD TAB height")?;
        let payload_bytes = usize::from(width)
            .checked_mul(usize::from(height))
            .and_then(|cells| cells.checked_mul(2))
            .ok_or_else(|| NativeWorldError::new("Level 2 House BUILD payload size overflow"))?;
        let data_end = data_offset
            .checked_add(payload_bytes)
            .filter(|end| *end <= dat.len())
            .ok_or_else(|| {
                NativeWorldError::new(format!(
                    "Level 2 House BUILD selector {building_index} escapes DATA_BUILD0-0.DAT"
                ))
            })?;
        let footprint = CastleBuildFootprint::from_owner_cells(
            building_index,
            width,
            height,
            &dat[data_offset..data_end],
        )
        .map_err(NativeWorldError::new)?;
        house_footprints.insert(building_index, footprint);
    }
    Ok((footprints, house_footprints))
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
#[path = "native_world/tests_split/mod.rs"]
mod tests;
