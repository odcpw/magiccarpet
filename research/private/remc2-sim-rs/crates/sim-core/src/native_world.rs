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
        GoatStepEvent, LEVEL_ONE_GOAT_INDICES, LEVEL_TWO_GOAT_INDICES, LiveWorldState,
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
        POSSESSION_IMPACT_LOGICAL_HEIGHT, POSSESSION_IMPACT_MODEL, POSSESSION_IMPACT_RENDER_INDEX,
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
    V4,
    V5,
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
    /// Source `subSpellIndex_0x2A_42` (the low word exported at bytes
    /// 46..48 after the four-byte entity header). `SetSpell_6D5E0` carries
    /// this into allocated spell holders.
    pub subspell_index: u16,
    /// Source `byte_0x3B_59`, exported at byte 63 after the four-byte entity
    /// header and written by `SetSpell_6D5E0`.
    pub byte_3b: u8,
    /// Source `byte_0x3C_60`, reset by `SetSpell_6D5E0`.
    pub byte_3c: u8,
    /// Source `fontTypeIndex_0x3D_61`, reset by `SetSpell_6D5E0`.
    pub font_type_index: u8,
    pub active_value: i16,
    pub activation_value: u16,
    /// Source `manaRegen_0x88_136`, exported at bytes 140..144 after the
    /// four-byte entity header.
    pub mana_regen: i32,
    pub actual_speed: i16,
    /// Source `maxMana_0x8C_140`, exported at bytes 144..148. `mana_cost`
    /// remains as the historical activation-cost name for callers that use
    /// the lossless entity projection.
    pub max_mana: i32,
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

    /// Raw source `playerEntityIndex_0x94_148`, exported four bytes after the
    /// original source offset because the package record carries a four-byte
    /// header.  This relation is distinct from `parentId_0x28` and from the
    /// presentation `source_id`.
    #[must_use]
    pub fn source_player_entity_index(&self) -> u16 {
        u16::from_le_bytes([self.record[152], self.record[153]])
    }

    /// Return the source draw-disable state written by
    /// `DisableEntityDrawing04_57F10`. This is byte 17 bit 2 (the 0x400 bit
    /// of the control dword), not byte 16 bit 0 used by pickup ownership.
    #[must_use]
    pub const fn drawing_disabled(&self) -> bool {
        self.control_flags & ENTITY_DRAW_DISABLED_BIT != 0
    }

    /// Apply the source draw-disable write to both the typed control dword
    /// and its lossless exported record bytes.
    fn disable_drawing(&mut self) {
        self.control_flags |= ENTITY_DRAW_DISABLED_BIT;
        self.record[16..20].copy_from_slice(&self.control_flags.to_le_bytes());
    }

    /// Update the source position while keeping the lossless record projection
    /// in lockstep.  Level 2 switch tests and the future objective bridge use
    /// this seam instead of mutating a typed cache and leaving stale bytes.
    fn set_position(&mut self, position: Position) {
        self.position = position;
        self.record[80..82].copy_from_slice(&position.x.to_le_bytes());
        self.record[82..84].copy_from_slice(&position.y.to_le_bytes());
        self.record[84..86].copy_from_slice(&position.z.to_le_bytes());
    }

    /// Keep the source orientation field in lockstep with a callback write.
    fn set_yaw(&mut self, yaw: i16) {
        self.yaw = yaw;
        self.record[32..34].copy_from_slice(&yaw.to_le_bytes());
    }

    /// Keep the source `roll_0x20_32` field in lockstep with the Nyphur
    /// movement helper's target-heading write.
    fn set_source_roll(&mut self, roll: i16) {
        self.record[36..38].copy_from_slice(&roll.to_le_bytes());
    }

    /// Raw source `roll_0x20_32`, kept separate from the exported fov word.
    fn source_roll(&self) -> i16 {
        i16::from_le_bytes([self.record[36], self.record[37]])
    }

    /// Keep the source callback speed field in lockstep with its typed cache.
    fn set_actual_speed(&mut self, actual_speed: i16) {
        self.actual_speed = actual_speed;
        self.record[134..136].copy_from_slice(&actual_speed.to_le_bytes());
    }

    /// Raw source `minSpeed_0x84_132`, consumed by `sub_13890` before its
    /// common movement callback.
    fn minimum_speed(&self) -> i16 {
        i16::from_le_bytes([self.record[132], self.record[133]])
    }

    /// Keep the source life field in lockstep with a common AI upkeep write.
    fn set_life(&mut self, life: i32) {
        self.life = life;
        self.record[12..16].copy_from_slice(&life.to_le_bytes());
    }

    /// Keep the source mana field in lockstep with a common AI upkeep write.
    fn set_mana(&mut self, mana: i32) {
        self.mana = mana;
        self.record[148..152].copy_from_slice(&mana.to_le_bytes());
    }

    /// Keep the source mana-regeneration field in lockstep with a common AI
    /// upkeep write.
    fn set_mana_regen(&mut self, mana_regen: i32) {
        self.mana_regen = mana_regen;
        self.record[140..144].copy_from_slice(&mana_regen.to_le_bytes());
    }

    /// Borrow the source `str_0x5E_94` payload cleared by `sub_12A70` while
    /// the AI cooldown carrier is active.
    fn source_upkeep_mailbox(&self) -> &[u8] {
        &self.record[98..134]
    }

    fn source_upkeep_word_7a(&self) -> u16 {
        u16::from_le_bytes([self.record[126], self.record[127]])
    }

    fn source_upkeep_word_62(&self) -> u16 {
        u16::from_le_bytes([self.record[102], self.record[103]])
    }

    fn source_upkeep_payload(&self) -> i32 {
        i32::from_le_bytes([
            self.record[98],
            self.record[99],
            self.record[100],
            self.record[101],
        ])
    }

    fn set_source_upkeep_payload(&mut self, value: i32) {
        self.record[98..102].copy_from_slice(&value.to_le_bytes());
    }

    fn clear_source_upkeep_owner(&mut self) {
        self.record[102..104].fill(0);
    }

    /// Apply the source `memset(str_0x5E_94, 0, 36)` operation.
    fn clear_source_upkeep_mailbox(&mut self) {
        self.record[98..134].fill(0);
    }

    /// Apply the source `sub_5EF70` post-contact write.  The qualified
    /// Nyphur row is model 1, so this is deliberately a typed source guard,
    /// not a generic timer write on every AI entity.
    fn apply_source_sub_5ef70(&mut self) {
        if self.index != 0 && self.class == 3 && self.model == 0 {
            self.record[58..60].copy_from_slice(&100_i16.to_le_bytes());
        }
    }

    /// Apply the already-qualified `SetSpell_6D5E0` profile to an AI holder.
    /// The active-holder branch only queues `tier + 1` in word 0x2C; the
    /// cold branch carries the complete authored subspell/mana payload.
    fn apply_nyphur_spell_profile(&mut self, profile: Level2SetSpellProfile, tier: u8) {
        let active_word_2e = u16::from_le_bytes([self.record[50], self.record[51]]);
        if active_word_2e != 0 {
            self.record[48..50].copy_from_slice(&u16::from(tier + 1).to_le_bytes());
            return;
        }
        self.subspell_index = profile.subspell_index;
        self.record[46..48].copy_from_slice(&profile.subspell_index.to_le_bytes());
        self.activation_value = profile.word_0x30;
        self.record[52..54].copy_from_slice(&profile.word_0x30.to_le_bytes());
        self.byte_3b = profile.byte_0x3b;
        self.record[63] = profile.byte_0x3b;
        self.byte_3c = 0;
        self.record[64] = 0;
        self.action_marker = 0;
        self.record[65] = 0;
        self.font_type_index = 0;
        self.mana_regen = profile.mana_regen;
        self.record[140..144].copy_from_slice(&profile.mana_regen.to_le_bytes());
        self.max_mana = profile.max_mana;
        self.mana_cost = profile.max_mana;
        self.record[144..148].copy_from_slice(&profile.max_mana.to_le_bytes());
        self.mana = profile.mana;
        self.record[148..152].copy_from_slice(&profile.mana.to_le_bytes());
        self.level_gate = tier.cast_signed();
        self.record[74] = tier;
    }

    /// Keep the source entity RNG in lockstep with a callback draw.
    fn set_entity_rng(&mut self, entity_rng: u16) {
        self.entity_rng = entity_rng;
        self.record[24..26].copy_from_slice(&entity_rng.to_le_bytes());
    }

    /// Keep the source control dword in lockstep with a callback flag write.
    fn set_control_flags(&mut self, control_flags: u32) {
        self.control_flags = control_flags;
        self.record[16..20].copy_from_slice(&control_flags.to_le_bytes());
    }

    /// Raw source `word_0x30_48` writer used by the wind target callback.
    fn set_source_word_30(&mut self, value: u16) {
        self.record[48..50].copy_from_slice(&value.to_le_bytes());
    }

    /// Keep the lossless source cadence byte in sync with a typed callback
    /// projection that reuses an imported frame-zero entity slot.
    fn set_phase(&mut self, phase: u8) {
        self.phase = phase;
        self.record[66] = phase;
    }

    /// Keep the low source word of `dword_0x10` in sync with a callback
    /// cooldown projected onto an imported class-11 root.
    fn set_source_word_10(&mut self, value: u16) {
        self.record[20..22].copy_from_slice(&value.to_le_bytes());
    }

    /// Raw source `dword_0x10_16`, used by the Trader when it reserves a
    /// class-10/model-45 building target.
    fn source_dword_10(&self) -> i32 {
        i32::from_le_bytes([
            self.record[20],
            self.record[21],
            self.record[22],
            self.record[23],
        ])
    }

    /// Keep all four bytes of source `dword_0x10_16` in lockstep with a
    /// Trader target-reservation write.
    fn set_source_dword_10(&mut self, value: i32) {
        self.record[20..24].copy_from_slice(&value.to_le_bytes());
    }

    /// Keep the local-player damage/killer words in the lossless source
    /// record when `sub_5EFA0` applies a contact mailbox.
    fn set_damage_result_words(&mut self, killer: u16, attacker: u16) {
        self.record[40..42].copy_from_slice(&killer.to_le_bytes());
        self.record[42..44].copy_from_slice(&attacker.to_le_bytes());
    }

    /// Apply the source death action and clear word-0x2C when the local
    /// player crosses below zero life in `AddPlayer03_00_5E010`.
    fn set_player_death_state(&mut self) {
        self.action = 2;
        self.record[48..50].fill(0);
        self.record[73] = 2;
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

    /// Raw source `word_0x96_150`, the model-4 special-spawn relation used
    /// by `sub_68E50`.
    #[must_use]
    pub fn source_word_96(&self) -> u16 {
        u16::from_le_bytes([self.record[154], self.record[155]])
    }

    /// Raw source `word_0x98_152`, the identity checksum paired with
    /// `word_0x96_150` by the Nyphur target selectors.
    fn source_word_98(&self) -> i16 {
        i16::from_le_bytes([self.record[156], self.record[157]])
    }

    /// Install the two-word source relation written by the pinned target
    /// selectors. The relation stays in the lossless entity record; the
    /// common carrier does not invent a second target identity.
    fn set_source_relation(&mut self, target: &EntitySnapshot) {
        self.record[154..156].copy_from_slice(&target.index.to_le_bytes());
        self.record[156..158].copy_from_slice(&level2_nyphur_source_identity(target).to_le_bytes());
    }

    /// Apply the source relation clear used by `sub_5EFA0` when a previous
    /// target is no longer a live contact.
    fn clear_source_word_96(&mut self) {
        self.record[154..156].fill(0);
    }

    /// Source `array_0x52_82.yaw`, distinct from the entity orientation yaw.
    #[must_use]
    fn array_yaw(&self) -> i16 {
        i16::from_le_bytes([self.record[86], self.record[87]])
    }

    /// Raw source `array_0x52_82.pitch` consumed by the model-76 target
    /// follower when it retunes the root oscillation envelope from a live
    /// target row.
    #[must_use]
    fn array_pitch(&self) -> i16 {
        i16::from_le_bytes([self.record[88], self.record[89]])
    }

    /// Raw source `array_0x52_82.roll`, the Y half-width used by
    /// `CompareAxisWithShift_10750`.
    #[must_use]
    fn array_roll(&self) -> i16 {
        i16::from_le_bytes([self.record[90], self.record[91]])
    }

    /// Raw source `array_0x52_82.fov`, used by the cave ceiling clamp in
    /// `sub_33340` after the target movement branch.
    #[must_use]
    fn array_fov(&self) -> i16 {
        i16::from_le_bytes([self.record[92], self.record[93]])
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

    /// Raw source `word_0x32_50`, the peer-link field consumed by the
    /// class-5 Archer action-35 callback.
    #[must_use]
    pub fn word_32(&self) -> u16 {
        u16::from_le_bytes([self.record[54], self.record[55]])
    }

    /// Raw source `word_0x34_52`, the head/next link used by class-5 child
    /// chains. This is distinct from the class-list `next_relation` field.
    #[must_use]
    pub fn word_34(&self) -> u16 {
        u16::from_le_bytes([self.record[56], self.record[57]])
    }

    /// Raw source `word_0x2E_46` used by `sub_5F890` for the spell-holder
    /// action/cooldown counter.  This is deliberately kept separate from the
    /// public entity action byte at record offset 73.
    #[must_use]
    pub fn source_word_2e(&self) -> u16 {
        u16::from_le_bytes([self.record[46], self.record[47]])
    }

    /// Raw source `word_0x30_48`, the spell-holder counter copied by
    /// `sub_5F890(a1, 1)` before a Castle stage is reinstalled.
    #[must_use]
    pub fn source_word_30(&self) -> u16 {
        u16::from_le_bytes([self.record[48], self.record[49]])
    }

    /// Exported `word_0x30_48` from the spell-holder source struct. The
    /// entities export prepends a four-byte slot/active header, so this
    /// source field is at record bytes 52..54. Keep this helper separate from
    /// the legacy `source_word_30` projection used by older economy rows.
    #[must_use]
    pub fn source_spell_word_30(&self) -> u16 {
        u16::from_le_bytes([self.record[52], self.record[53]])
    }

    /// Source SetSpell parent relation written at the holder record's
    /// `word_0x28_40` slot. This is distinct from the terrain oldMapEntity
    /// link, which remains zero for the acquired source pickup.
    #[must_use]
    pub fn source_spell_parent_entity_index(&self) -> u16 {
        u16::from_le_bytes([self.record[44], self.record[45]])
    }

    /// Raw source `dword_0x5E_94`, the spell payload mailbox consumed by
    /// `sub_11900` and the wind parent pulse helper.
    #[must_use]
    fn source_wind_spell_payload(&self) -> i32 {
        i32::from_le_bytes(
            self.record[98..102]
                .try_into()
                .expect("entity record payload"),
        )
    }

    /// Raw source `word_0x62_98`, the last source entity that wrote the spell
    /// payload mailbox.
    #[must_use]
    fn source_wind_spell_owner(&self) -> u16 {
        u16::from_le_bytes([self.record[102], self.record[103]])
    }

    /// Apply `sub_11900(a1, this, 0, payload)` to a class-10/model-45 row.
    fn apply_wind_model45_mailbox(&mut self, parent_entity_id: u16, payload: u16) {
        let next_payload = if self.source_wind_spell_owner() != 0 {
            i32::from(payload)
        } else {
            self.source_wind_spell_payload()
                .wrapping_add(i32::from(payload))
        };
        self.record[98..102].copy_from_slice(&next_payload.to_le_bytes());
        self.record[102..104].copy_from_slice(&parent_entity_id.to_le_bytes());
    }

    /// Apply the model-2 player branch of the wind pulse helper. This writes
    /// the thirty-tick mailbox, source parent relation, and additive payload
    /// exactly where `sub_33710` stores them in the imported entity record.
    fn apply_wind_model2_mailbox(
        &mut self,
        parent_source_slot: u16,
        parent_entity_id: u16,
        payload: u16,
    ) {
        self.activation_value = 30;
        self.record[52..54].copy_from_slice(&30_u16.to_le_bytes());
        self.record[42..44].copy_from_slice(&parent_source_slot.to_le_bytes());
        let next_payload = self
            .source_wind_spell_payload()
            .wrapping_add(i32::from(payload));
        self.record[98..102].copy_from_slice(&next_payload.to_le_bytes());
        self.record[102..104].copy_from_slice(&parent_entity_id.to_le_bytes());
    }

    /// Apply one `sub_10C80(a2 = 0)` Fire-Spheres mailbox write. The model-2,
    /// model-45, and generic map scans share this exact write after their
    /// distinct admission predicates: replace an ownerless payload, otherwise
    /// add, then record the colliding child identity.
    fn apply_model76_mailbox(&mut self, owner_entity_id: u16, payload: u16) {
        let next_payload = if self.source_wind_spell_owner() != 0 {
            self.source_wind_spell_payload()
                .wrapping_add(i32::from(payload))
        } else {
            i32::from(payload)
        };
        self.record[98..102].copy_from_slice(&next_payload.to_le_bytes());
        self.record[102..104].copy_from_slice(&owner_entity_id.to_le_bytes());
    }

    /// Raw source `byte_0x38_56`, exported four bytes later in the lossless
    /// entity record. Bit zero admits the `sub_10C80(a2 = 0)` generic scan.
    fn source_byte_38(&self) -> u8 {
        self.record[60]
    }

    /// Raw source `word_0x4A_74`, the return-side hand override consumed by
    /// `sub_68FF0` before it installs a holder.
    #[must_use]
    pub fn source_word_4a(&self) -> i16 {
        i16::from_le_bytes([self.record[78], self.record[79]])
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

fn sync_level2_spell_candidate_projection(target: &mut EntitySnapshot, source: &EntitySnapshot) {
    target.active = source.active;
    target.model = source.model;
    target.life = source.life;
    target.position = source.position;
    target.record[2] = source.record[2];
    target.record[12..16].copy_from_slice(&source.record[12..16]);
    target.record[67..69].copy_from_slice(&source.record[67..69]);
    target.record[80..86].copy_from_slice(&source.record[80..86]);
}

fn level2_experience_scroll_overlap(scroll: Position, player: Position) -> bool {
    let dx = i32::from(player.x.wrapping_sub(scroll.x).cast_signed()).abs();
    let dy = i32::from(player.y.wrapping_sub(scroll.y).cast_signed()).abs();
    let scroll_z = scroll
        .z
        .wrapping_add(LEVEL2_SCROLL_COLLISION_VERTICAL_OFFSET);
    let player_z = player
        .z
        .wrapping_add(LEVEL2_PLAYER_COLLISION_VERTICAL_OFFSET);
    let dz = i32::from(player_z.wrapping_sub(scroll_z)).abs();
    dx < i32::from(LEVEL2_SCROLL_COLLISION_HALF_WIDTH + LEVEL2_PLAYER_COLLISION_HALF_WIDTH)
        && dy < i32::from(LEVEL2_SCROLL_COLLISION_HALF_WIDTH + LEVEL2_PLAYER_COLLISION_HALF_WIDTH)
        && dz < i32::from(LEVEL2_SCROLL_COLLISION_HALF_HEIGHT + LEVEL2_PLAYER_COLLISION_HALF_HEIGHT)
}

fn level2_wrapping_xy_distance(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    dx.wrapping_mul(dx)
        .wrapping_add(dy.wrapping_mul(dy))
        .cast_unsigned()
}

/// Explicit signed-16 subtraction used by remc2's `sub_58440` and
/// `sub_10630`. Position Z is already stored as `i16`, but spelling out the
/// source truncation keeps the high-bit boundary contract visible.
fn level2_signed16_difference(left: i16, right: i16) -> i32 {
    i32::from(i16::from_le_bytes(right.wrapping_sub(left).to_le_bytes()))
}

/// Pinned `sub_58440`: Castle defender sphere arbitration compares the
/// signed-16 X/Y/Z deltas as a squared 3-D distance, not the XY-only arrival
/// metric used by `AddBallon_60AB0`.
fn level2_wrapping_source_distance_3d(left: Position, right: Position) -> u32 {
    let dx = i64::from(i32::from(right.x.wrapping_sub(left.x).cast_signed()));
    let dy = i64::from(i32::from(right.y.wrapping_sub(left.y).cast_signed()));
    let dz = i64::from(level2_signed16_difference(left.z, right.z));
    u32::try_from(dx * dx + dy * dy + dz * dz).unwrap_or(u32::MAX)
}

/// `sub_5F810` tests byte 1 bit `0x20` of the source control dword. The
/// little-endian entity export places that bit at dword mask `0x2000`.
const LEVEL2_MANA_SPHERE_TARGET_SUPPRESSION_BIT: u32 = 0x0000_2000;

/// Pinned signed-16 XY distance used by `Maths::EuclideanDistXYZ_58490` in
/// `AddBallon_60AB0` before its class-3 target gate.  The helper's recovered
/// cutoff path ignores Z; the holder's Z participates only in the later
/// terrain gate.
fn level2_wrapping_target_distance(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    crate::legacy_sqrt(
        dx.wrapping_mul(dx)
            .wrapping_add(dy.wrapping_mul(dy))
            .cast_unsigned(),
    )
}

fn level2_mana_sphere_dimension(mana: i32) -> i16 {
    MANA_SPHERE_COLLISION_DIMENSIONS[usize::from(mana_sphere_size_ordinal(mana))]
}

/// `sub_4ABA0` installs the defender's source collision axes through
/// `SetEntityIndexAndRot_49CD0(169)`.  The qualified particle row 169 is
/// speed 545 / rotation 800, so the source stores 272 for X/Y and 400 for
/// the vertical offset/extent.  Keep these separate from the holder's
/// movement speed: `sub_106C0` consumes the collision axes, not `actSpeed`.
const LEVEL2_CASTLE_DEFENDER_COLLISION_HALF_WIDTH: i16 = 272;
const LEVEL2_CASTLE_DEFENDER_COLLISION_VERTICAL_OFFSET: i16 = 400;

/// Pinned `sub_106C0(sphere, defender)` strict AABB.  All source axis fields
/// are signed 16-bit values, while the additions are performed in the
/// promoted integer domain before the strict `<` comparison.
fn level2_castle_defender_sphere_overlap(defender_position: Position, sphere: &ManaSphere) -> bool {
    let sphere_extent = i32::from(level2_mana_sphere_dimension(sphere.mana));
    let defender_width = i32::from(LEVEL2_CASTLE_DEFENDER_COLLISION_HALF_WIDTH);
    let dx = i32::from(
        sphere
            .position
            .x
            .wrapping_sub(defender_position.x)
            .cast_signed(),
    );
    let dy = i32::from(
        sphere
            .position
            .y
            .wrapping_sub(defender_position.y)
            .cast_signed(),
    );
    let dz = i32::from(sphere.position.z) + sphere_extent
        - i32::from(defender_position.z)
        - i32::from(LEVEL2_CASTLE_DEFENDER_COLLISION_VERTICAL_OFFSET);
    dx.abs() < sphere_extent + defender_width
        && dy.abs() < sphere_extent + defender_width
        && dz.abs() < sphere_extent + i32::from(LEVEL2_CASTLE_DEFENDER_COLLISION_VERTICAL_OFFSET)
}

/// Pinned remc2 `sub_10A50` SEARCH.DAT walk for model-39 mana spheres. The
/// four class-0 cells precede the twelve class-1 cells; candidates are read
/// from the live map chain rather than the allocation vector.
const LEVEL2_MANA_SPHERE_SEARCH_OFFSETS: [(i8, i8); 16] = [
    (0, 0),
    (1, 0),
    (0, 1),
    (1, 1),
    (-1, -1),
    (0, -1),
    (1, -1),
    (2, -1),
    (-1, 0),
    (2, 0),
    (-1, 1),
    (2, 1),
    (-1, 2),
    (0, 2),
    (1, 2),
    (2, 2),
];

/// Pinned `sub_108B0` uses `AddE7EE0x_10080(0, ...)` for the tier-0
/// Possession flyer.  That class-0 query is only the first four SEARCH.DAT
/// cells; the remaining twelve cells belong to the wider class-1 sphere walk.
const LEVEL2_POSSESSION_PROJECTILE_SEARCH_OFFSETS: [(i8, i8); 4] = [(0, 0), (1, 0), (0, 1), (1, 1)];

fn level2_first_colliding_mana_sphere_slot(
    map_chains: &RenderableMapChains,
    spheres: &[ManaSphere],
    source_slot: u16,
    sphere: ManaSphere,
) -> Option<u16> {
    let tile_x = ((u32::from(sphere.position.x) + 128) >> 8) as u8;
    let tile_y = ((u32::from(sphere.position.y) + 128) >> 8) as u8;
    for (delta_x, delta_y) in LEVEL2_MANA_SPHERE_SEARCH_OFFSETS {
        let search_x = tile_x.wrapping_add_signed(delta_x);
        let search_y = tile_y.wrapping_add_signed(delta_y);
        for candidate_slot in map_chains.entity_chain(search_x, search_y) {
            if candidate_slot == source_slot {
                continue;
            }
            let Some(candidate) = spheres.iter().find(|candidate| {
                candidate.active && candidate.source_entity_index == candidate_slot
            }) else {
                continue;
            };
            let left = level2_mana_sphere_dimension(sphere.mana);
            let right = level2_mana_sphere_dimension(candidate.mana);
            if crate::level1_mission::source_entities_overlap(
                sphere.position,
                left,
                left,
                left,
                candidate.position,
                right,
                right,
                right,
            ) {
                return Some(candidate_slot);
            }
        }
    }
    None
}

/// Pinned `sub_108B0` map-chain walk for the ordinary tier-0 Possession
/// flyer. The source returns the first eligible strict-overlap model-39 row;
/// allocation order never substitutes for the chain order.
fn level2_first_possession_target_sphere_slot(
    map_chains: &RenderableMapChains,
    spheres: &[ManaSphere],
    projectile: Level2PossessionProjectile,
) -> Option<u16> {
    for candidate_slot in source_map_search_slots(
        map_chains,
        projectile.position,
        &LEVEL2_POSSESSION_PROJECTILE_SEARCH_OFFSETS,
    ) {
        let Some(candidate) = spheres.iter().find(|candidate| {
            candidate.active
                && !candidate.pending_delete
                && candidate.source_entity_index == candidate_slot
                && u16::try_from(candidate.id).ok() != Some(projectile.source_id_entity_index)
                && candidate.player_entity_index != projectile.source_id_entity_index
        }) else {
            continue;
        };
        let dimension = level2_mana_sphere_dimension(candidate.mana);
        if crate::level1_mission::source_entities_overlap(
            projectile.position,
            POSSESSION_PROJECTILE_COLLISION_HALF_WIDTH,
            POSSESSION_PROJECTILE_COLLISION_VERTICAL_OFFSET,
            POSSESSION_PROJECTILE_COLLISION_HALF_HEIGHT,
            candidate.position,
            dimension,
            dimension,
            dimension,
        ) {
            return Some(candidate_slot);
        }
    }
    None
}

fn level2_possession_guidance_target_top(sphere: &ManaSphere) -> Position {
    Position {
        z: sphere
            .position
            .z
            .wrapping_add(level2_mana_sphere_dimension(sphere.mana)),
        ..sphere.position
    }
}

/// Qualified Level-2 subset of `sub_67CB0`'s model-1/0x11 acquisition list.
/// The represented arena currently carries model-39 rows; source rebuilds
/// their special list in ascending arena-slot order and strict-lower score
/// replacement preserves the earlier row on ties.
fn level2_possession_guidance_target(
    spheres: &[ManaSphere],
    source_slots: &[u16],
    projectile: Level2PossessionProjectile,
) -> Option<(u16, Position)> {
    let mut selected = None;
    let mut selected_score = u32::MAX;
    for source_slot in source_slots {
        let Some(sphere) = spheres.iter().find(|sphere| {
            sphere.source_entity_index == *source_slot
                && (sphere.active || sphere.pending_delete)
                && sphere.player_entity_index != projectile.source_id_entity_index
        }) else {
            continue;
        };
        let target_top = level2_possession_guidance_target_top(sphere);
        let Some(score) = projectile_target_score(
            projectile.position,
            projectile.yaw,
            projectile.pitch,
            target_top,
        ) else {
            continue;
        };
        if score < selected_score {
            selected = Some((sphere.source_entity_index, target_top));
            selected_score = score;
        }
    }
    selected
}

/// Resolve an already-installed source slot without a generation guard.
/// If a model-39 row reuses that slot, source follows the reused row; if the
/// slot is absent, the one-shot acquisition bit prevents retargeting.
fn level2_possession_guidance_relation(
    spheres: &[ManaSphere],
    possession_projectiles: &[Level2PossessionProjectile],
    stale_source_records: &BTreeMap<u16, Position>,
    target_source_entity_index: u16,
) -> Option<Position> {
    spheres
        .iter()
        .find(|sphere| sphere.source_entity_index == target_source_entity_index)
        .map(level2_possession_guidance_target_top)
        .or_else(|| {
            possession_projectiles
                .iter()
                .find(|projectile| projectile.source_entity_index == target_source_entity_index)
                // A freshly constructed model-1 row has source array-yaw 0,
                // so `sub_65580` uses its stored position without a Z offset.
                .map(|projectile| projectile.position)
        })
        .or_else(|| {
            stale_source_records
                .get(&target_source_entity_index)
                .copied()
        })
}

fn level2_carrier_sphere_overlap(carrier: Position, sphere: &ManaSphere) -> bool {
    let extent = level2_mana_sphere_dimension(sphere.mana);
    let dx = i32::from(sphere.position.x.wrapping_sub(carrier.x).cast_signed());
    let dy = i32::from(sphere.position.y.wrapping_sub(carrier.y).cast_signed());
    // `sub_106C0(sphere, carrier)` compares the source sphere axis (including
    // its mana-sized yaw/fov extent) against the carrier's 272/400/400
    // collision axes.  Keep the strict signed source expression rather than
    // treating the sphere as a point at its centre.
    let dz = i32::from(sphere.position.z) + i32::from(extent)
        - i32::from(carrier.z)
        - i32::from(MANA_CARRIER_COLLISION_VERTICAL_OFFSET);
    dx.abs() < i32::from(MANA_CARRIER_COLLISION_HALF_WIDTH + extent)
        && dy.abs() < i32::from(MANA_CARRIER_COLLISION_HALF_WIDTH + extent)
        && dz.abs() < i32::from(MANA_CARRIER_COLLISION_HALF_HEIGHT + extent)
}

/// `AddBallon_60AB0` compares a class-10 target's raw
/// `playerEntityIndex_0x94` with the carrier's raw `id_0x1A` relation. The
/// Level 2 projection stores that raw carrier relation in
/// `owner_entity_index`; `ManaCarrier::id` remains the presentation/source
/// slot identity and must not be used for this gate.
fn level2_carrier_sphere_relation_matches(carrier: &ManaCarrier, sphere: &ManaSphere) -> bool {
    sphere.owner_entity_index == carrier.owner_entity_index
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

/// Source `my_sign32` vertical launch calculation shared by
/// `TransformEntityToManaSphere_36BA0` and `sub_5FD00`:
/// `(1024 - height - sign(1024 - height) * 7) >> 3`.
#[must_use]
fn level2_mana_sphere_vertical_velocity(height_above_ground: i32) -> i16 {
    let delta = 1_024_i32.saturating_sub(height_above_ground);
    let sign = match delta.cmp(&0) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    };
    i16::try_from((delta - sign * 7) >> 3).expect("mana sphere vertical velocity fits i16")
}

/// `sub_60400`'s model-3 action-9 defender-holder count consumed by
/// `sub_5FF50`. The source's stage-0/default path has no children to maintain.
const fn level2_castle_defender_count(level: u8) -> u8 {
    match level {
        1 | 2 | 3 => 1,
        4 | 5 => 2,
        6 | 7 => 3,
        _ => 0,
    }
}

/// `sub_60400`'s `array_0x5C_92` model-15 tail count consumed by
/// `sub_5FF50`.  The source has no tail entries for stages 1/2, then grows
/// the authored rows as 4/6/14/18/34 through stage 7.
const fn level2_castle_auxiliary_count(level: u8) -> u8 {
    match level {
        3 => 4,
        4 => 6,
        5 => 14,
        6 => 18,
        7 => 34,
        _ => 0,
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

/// BUILD00DATTAB dimensions consumed by `sub_613D0` on the fixed 320x200
/// path.  The source TAB bytes are `[8, 21, 21, 35, 35, 48, 48]`; the
/// runtime loader doubles them and the Castle controller halves them again
/// under VGA selector 1, yielding the same effective values.  Keep the
/// asymmetrical 21/35 entries even though integer centring shares coordinates
/// with 20/34.
fn level2_castle_controller_dimensions(level: u8) -> (u8, u8) {
    match level {
        1 => (8, 8),
        2 | 3 => (21, 21),
        4 | 5 => (35, 35),
        6 | 7 => (48, 48),
        _ => (0, 0),
    }
}

/// Effective BUILD dimensions after the source VGA selector-1 half-size
/// conversion used by `RemoveCastleStage_385C0`.  The pinned runtime records
/// are 16/42/42/70/70/96/96, yielding the same effective values as the
/// controller-offset table after the single source half-size conversion.
const fn level2_castle_removal_dimensions(level: u8) -> (u8, u8) {
    match level {
        1 => (8, 8),
        2 | 3 => (21, 21),
        4 | 5 => (35, 35),
        6 | 7 => (48, 48),
        _ => (0, 0),
    }
}

/// Source `sub_613D0` backs down from the current Castle level while the
/// player's `array_0x24E[9 + level]` coefficient is zero. The selected level,
/// rather than the requested level, controls both DB038 geometry and model-79
/// word `0x4A`; an all-zero prefix emits no controllers.
fn level2_castle_controller_selection(level: u8, coefficients: &[u8; 8]) -> Option<(u8, u8)> {
    let mut selected = level.min(7);
    while selected > 0 {
        let coefficient = coefficients[usize::from(selected)];
        if coefficient != 0 {
            return Some((selected, coefficient));
        }
        selected -= 1;
    }
    None
}

/// `SetShiftByCastle_49EC0`'s fixed 320x200/VGA selector-1 result. The
/// controller dimensions are already the effective post-VGA values, so the
/// source half-size must not be applied a second time here.
fn level2_castle_shift_axes(level: u8) -> (i16, i16) {
    let (width, height) = level2_castle_controller_dimensions(level);
    let width = i32::from(width);
    let height = i32::from(height);
    (
        i16::try_from(((width << 8) + 1_280) >> 1).expect("Castle shift pitch fits i16"),
        i16::try_from(((height << 8) + 1_280) >> 1).expect("Castle shift roll fits i16"),
    )
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
    // `CastleBuildFootprint` is already the consumer-effective 8x8 BUILD
    // footprint (the raw/runtime descriptor widths are normalized at load),
    // so do not apply the source VGA half-size a second time here.
    level2_castle_root_positions_with_resolution(world, footprint, projectile_position, false)
}

/// Derive the Castle root and `axis_0x9A_154x` anchor exactly as
/// `sub_4AA40` does, including the source's VGA-size split. The explicit
/// selector is retained for callers that still provide raw BUILD dimensions;
/// production Level 2 calls pass `false` because the validated footprint is
/// already normalized to consumer-effective dimensions.
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

/// The exact fixed-width Type_str_164 fields needed by the open Level 2
/// Nyphur/common-upkeep and terminal seams.  This is a carrier only: policy
/// remains source-backed work for a later slice, and no defaults are generated
/// when the owner package omits these bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeStr164Auxiliary {
    pub word_0x26_38: [i16; 8],
    pub byte_0x154_340: u8,
    pub dword_0x16D_365: i32,
    pub word_0x1A2_418: i16,
    pub array_0x1FC_508: [u16; 32],
    pub word_0x242_578: i16,
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
    /// Source `array_0x403` learned/acquired flags (record 424..450), kept
    /// distinct from the hand-side bytes at record 398..424.
    pub learned_spell_flags: [u8; PLAYER_SPELL_COUNT],
    /// Source `array_0x3E9`/`SpellsEnabled` projection (record 450..476).
    /// This is distinct from the hand-side eligibility bytes at record
    /// 398..424 and the holder relations above.
    pub spell_enabled_flags: [u8; PLAYER_SPELL_COUNT],
    pub family_selected_tiers: [i8; PLAYER_SPELL_COUNT],
    /// Source `SpellExperience_0x263_611x`, stored after the 60-byte player
    /// prefix. This persistent row is distinct from the runtime family-12
    /// award buffer below.
    pub persistent_spell_experience: [i32; PLAYER_SPELL_COUNT],
    /// Source `spellsExperience_0x2CB_715x`, the runtime experience buffer.
    pub runtime_spell_experience: [i32; PLAYER_SPELL_COUNT],
    /// Source `SpellLevels_0x41D_1053z`, recomputed from persistent plus
    /// runtime experience at the family threshold table.
    pub spell_unlocked_levels: [i8; PLAYER_SPELL_COUNT],
    /// Source-exact Level 2 Type_str_164 carrier. Level 1 deliberately has no
    /// corresponding field because its sealed player record remains v2.
    pub type_str_164_auxiliary: Option<TypeStr164Auxiliary>,
    /// Source `array_0x2BDE[playerColorIndex].byte_0x006_2BE4_11236`.
    ///
    /// This mutable Level 2 carrier is intentionally separate from the
    /// compact ActPlayerIndex record at bytes 6..8.  Objective kind 3 reads
    /// this authoritative value; v3/v4 inspection rows retain `None` and
    /// cannot construct a qualified mutable Level 2 session.
    pub banishment_state_0x006: Option<u8>,
    record: Box<[u8]>,
}

impl PlayerSnapshot {
    #[must_use]
    pub fn record_bytes(&self) -> &[u8] {
        &self.record
    }

    #[must_use]
    fn legacy_record_bytes(&self) -> &[u8] {
        &self.record[..PLAYER_LEGACY_RECORD_BYTES]
    }

    #[must_use]
    pub const fn type_str_164_auxiliary(&self) -> Option<&TypeStr164Auxiliary> {
        self.type_str_164_auxiliary.as_ref()
    }

    /// Source `playerColorIndex_0x38_56` from the linked player-state record.
    /// `sub_68FF0` reads this dword_0xA4 field while walking model-0 entities;
    /// it is not the entity's unrelated byte at record offset 60.
    #[must_use]
    pub fn player_color_index(&self) -> u8 {
        self.record[40]
    }

    /// Source `array_0x2BDE[color].byte_0x006`, consumed by objective kind 3
    /// (the banished-player predicate). This is distinct from both the linked
    /// entity's life/drawing state and ActPlayerIndex record[6..8].
    #[must_use]
    fn objective_player_state(&self) -> Option<u8> {
        self.banishment_state_0x006
    }

    /// Source `roll_0x155_341`, exported at the compact player-state offset
    /// used by the Level 2 reset projection.
    fn roll_0x155(&self) -> i16 {
        i16::from_le_bytes([self.record[46], self.record[47]])
    }

    /// Keep the linked player-state roll feedback in lockstep with source
    /// `sub_33340` writes.
    fn set_roll_0x155(&mut self, value: i16) {
        self.record[46..48].copy_from_slice(&value.to_le_bytes());
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
            0x11 => 0x40 + 2 * corner,
            0x12 => 0x50 + 2 * (8 * usize::from(x.wrapping_add(y) & 1) + corner),
            _ => {
                // Level 2's qualified BUILD stages use only the admitted
                // 0x10, 0x11, and 0x12 controls here.
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
) -> u16 {
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
    entity_rng
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

    /// `sub_12330` advances only the high byte of the selected runtime
    /// StageVar's word before applying its thinning rule. Keep the decoded
    /// record and the lossless runtime byte carrier synchronized so reset and
    /// campaign hashing observe the same construction cadence.
    fn advance_level2_stage_variable_counter(&mut self, stage_var_index: u8) -> bool {
        let Some(stage_var) = self.stage_variables.get_mut(usize::from(stage_var_index)) else {
            return false;
        };
        let counter = (stage_var.word_2 >> 8) as u8;
        stage_var.word_2 = (stage_var.word_2 & 0x00ff) | (u16::from(counter.wrapping_add(1)) << 8);
        let offset = 392 + usize::from(stage_var_index) * 12 + 2;
        let Some(bytes) = self.bytes.get_mut(offset..offset + 2) else {
            return false;
        };
        bytes.copy_from_slice(&stage_var.word_2.to_le_bytes());
        true
    }

    fn set_level2_objective_stage_selector_for(
        &mut self,
        objective_index: usize,
        stage_index: usize,
        state: u8,
    ) {
        if let Some(objective) = self.objectives.get_mut(objective_index) {
            if let Some(selector) = objective.stage_selectors.get_mut(stage_index) {
                *selector = state;
                self.bytes[304 + objective_index * 11 + stage_index + 3] = state;
            }
        }
    }

    fn set_level2_objective_stage_selector(&mut self, stage_index: usize, state: u8) {
        self.set_level2_objective_stage_selector_for(0, stage_index, state);
    }

    #[cfg(test)]
    fn set_level2_objective_text_selector(&mut self, stage_index: usize) {
        self.set_level2_objective_text_selector_for(0, stage_index);
    }

    fn set_level2_objective_text_selector_for(
        &mut self,
        objective_index: usize,
        stage_index: usize,
    ) {
        if let Some(objective) = self.objectives.get_mut(objective_index) {
            if let Ok(selector) = u8::try_from(stage_index) {
                objective.text_selector = selector;
                self.bytes[304 + objective_index * 11 + 1] = selector;
            }
        }
    }

    fn set_level2_objective_stage_state(&mut self, state: u8) {
        self.set_level2_objective_stage_selector(LEVEL2_SELECTED_OBJECTIVE_STAGE_INDEX, state);
    }

    fn set_level2_objective_level_end_for(&mut self, objective_index: usize, state: u8) {
        if let Some(objective) = self.objectives.get_mut(objective_index) {
            objective.is_level_end = state;
            self.bytes[304 + objective_index * 11] = state;
        }
    }

    /// Bind an authored event into the stage table as remc2 `sub_58DA0` does
    /// for stage kinds 1/2/4/6.  The exported runtime uses an authored-feature
    /// relation for the pre-construction pointer; after construction kinds
    /// 1/2/4 retain a one-based live-entity relation while kind 6 stores the
    /// zero-based entity index.  The source also sets the stage's bound flag.
    fn bind_authored_stage_entity(&mut self, authored_index: u16, source_slot: u16) -> usize {
        let mut bound = 0;
        for (stage_index, stage) in self.stages.iter_mut().enumerate() {
            if !matches!(stage.kind, 1 | 2 | 4 | 6)
                || stage.relation != RuntimeRelation::AuthoredFeature(authored_index)
            {
                continue;
            }
            stage.relation = if stage.kind == 6 {
                RuntimeRelation::LiveEntityIndex(source_slot)
            } else {
                RuntimeRelation::LiveEntity(source_slot)
            };
            stage.flags |= 1;
            let offset = 192 + stage_index * 14;
            self.bytes[offset + 1] = stage.flags as u8;
            if stage.kind == 6 {
                self.bytes[offset + 6] = 3;
                self.bytes[offset + 10..offset + 14]
                    .copy_from_slice(&u32::from(source_slot).to_le_bytes());
            } else {
                self.bytes[offset + 6] = 2;
                self.bytes[offset + 10..offset + 14]
                    .copy_from_slice(&u32::from(source_slot).wrapping_add(1).to_le_bytes());
            }
            bound += 1;
        }
        bound
    }

    fn mark_level2_objective_done_for(&mut self, player_color_index: u8) {
        let index = usize::from(player_color_index);
        if let Some(objective) = self.objectives.get_mut(index) {
            objective.done = 1;
            self.bytes[304 + index * 11 + 2] = 1;
        }
    }

    #[cfg(test)]
    fn set_level2_is_level_end(&mut self, state: u8) {
        if let Some(objective) = self.objectives.get_mut(0) {
            objective.is_level_end = state;
            self.bytes[304] = state;
        }
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
    collision_masks: Option<CollisionMaskBank>,
    level2_castle_stage_footprints: BTreeMap<u8, CastleBuildFootprint>,
    level2_house_build_footprints: BTreeMap<u8, CastleBuildFootprint>,
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

    /// Borrow the immutable source-exact Level 2 collision bank. Historical
    /// v3 packages intentionally return `None`; they are not collision
    /// authority and cannot be upgraded by inference.
    #[must_use]
    pub const fn collision_mask_bank(&self) -> Option<&CollisionMaskBank> {
        self.collision_masks.as_ref()
    }

    /// Query remc2 model-45 occupancy through the validated Level 2 carrier.
    pub fn model45_occupancy(&self, selector: u16, x: i32, y: i32) -> Result<bool> {
        let bank = self.collision_mask_bank().ok_or_else(|| {
            NativeWorldError::new("Level 2 collision occupancy requires a qualified v5 package")
        })?;
        bank.model45_occupancy(selector, x, y)
            .map_err(|error: CollisionMaskLookupError| {
                NativeWorldError::new(format!("model-45 occupancy lookup failed: {error:?}"))
            })
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
        // Inspection deliberately accepts historical v3/v4 package rows, but
        // they have no source-backed type-3 banishment carrier.  Reject before
        // constructing any mutable mission/session state rather than creating
        // a `None` session that merely suppresses objective completion.
        let stage_count = usize::from(self.runtime.stage_index).min(self.runtime.stages.len());
        if self
            .runtime
            .stages
            .iter()
            .take(stage_count)
            .any(|stage| stage.kind == 3)
            && self
                .players
                .iter()
                .any(|player| player.banishment_state_0x006.is_none())
        {
            return Err(NativeWorldError::new(
                "Level 2 type-3 objective requires a source banishment-state carrier before session creation",
            ));
        }
        let state = self
            .reset_state
            .ok_or_else(|| NativeWorldError::new("validated Level 2 package has no reset state"))?;
        let mission = self.mission_content_template()?.session();
        let nyphur_dispatcher = Level2NyphurDispatcher::from_qualified_reset(
            &self.players,
            &self.entities,
            &self.runtime,
            &mission,
        )?;
        let bad_stones = level2_bad_stones(&self.entities, &self.terrain_planes)?;
        let mut economy = Level2EconomyState::from_parts_with_castle_stage_footprints(
            SessionTerrain::from_inspection(self),
            &self.entities,
            self.entities[usize::from(self.players[0].entity_index)].mana,
            self.entities[usize::from(self.players[0].entity_index)].model,
            0,
            self.players[0].enabled_spell_holders[2],
            self.runtime.random_state,
            self.level2_castle_stage_footprints.clone(),
        )?;
        let wizard = mission
            .wizard_map_settings()
            .first()
            .ok_or_else(|| NativeWorldError::new("Level 2 Wizard 0 settings are absent"))?;
        let shared_spells = Level2SharedSpellResetSession::from_parts_with_starting_spells(
            SessionTerrain::from_inspection(self),
            &self.entities,
            &self.players,
            &self.presentation,
            Some((&mut economy, wizard)),
        )?;
        let castle_holder = shared_spells
            .dynamic_pickups
            .iter()
            .find(|holder| holder.model == CASTLE_SPELL_FAMILY)
            .ok_or_else(|| NativeWorldError::new("Level 2 family-2 starting holder is absent"))?;
        economy.initialize_player_spell_cleanup_context(
            self.entities[usize::from(self.players[0].entity_index)].model,
            0,
            castle_holder.index,
            castle_holder.active_value.cast_unsigned(),
            castle_holder.activation_value,
        );
        let ordinary = Level2OrdinaryResetSession::from_parts(
            SessionTerrain::from_inspection(self),
            self.entities.clone(),
            self.players.clone(),
            self.presentation.clone(),
            self.level2_house_build_footprints.clone(),
        )?;
        let goat_config = GoatBehaviorConfig::from_level_two_parameter_record(
            self.presentation
                .entity_parameters
                .get(98)
                .ok_or_else(|| NativeWorldError::new("Level 2 Goat parameter row 98 is absent"))?,
        )
        .map_err(NativeWorldError::new)?;
        let goats =
            GoatFamily::from_level_two_entities(&self.entities, GOAT_STAGE_POINT, goat_config)
                .map_err(NativeWorldError::new)?;
        Ok(Level2ResetSession::new(
            self.metadata.map_type.profile(),
            self.package_sha256.clone(),
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
            goats,
            mission,
            nyphur_dispatcher,
            self.collision_masks.clone(),
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
/// Authored Level 2 spell pickups which are materialised by the source event
/// constructors rather than occupying a frame-zero reset slot.  The indices
/// are the pinned authored-level rows (155/156); row 38 is the active reset
/// pickup for family 12.  Keeping these source rows explicit lets the generic
/// `sub_68FF0` acquisition seam be reached from the production reset session
/// without pretending that the authored rows are already live entities.
const LEVEL2_GENERIC_SPELL_PICKUPS: [(u8, u16, u8); 3] =
    [(6, 155, 19), (11, 156, 34), (12, 38, 37)];
const LEVEL2_SHARED_SPELL_ENTITY_SHA256: [&str; 4] = [
    "21f016732b7a687ac53cd9fc452a148dce204e4944b30755abc2f312c5460182",
    "899b6f9f0c614cdb77c1c7c3e7922ac6eadd03391fdd252f347b29d93fee3e81",
    "9312c195108896b32d553bdce73a43484bf80e010fcf762fc7c01d7f7ecca954",
    "fedeb2b2fbe40c6ec72614bc4a971e2e826d52a8348156040e872c14a7c0a80e",
];
const LEVEL2_SHARED_SPELL_PLAYER_ENTITY_SHA256: &str =
    "b5234d405349902b3ef653ed52ac175c929b6024a19869a503fbe65f190d3834";
const LEVEL2_NYPHUR_ENTITY_INDEX: u16 = 112;
const LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX: u16 = 113;
const LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX: u16 = 114;
/// The qualified Level 2 objective ledger selects stage index 3 for the
/// first type-5 point objective.  Model-32's switch consumes this exact
/// stage byte after the objective evaluator writes the source completion
/// sentinel.
const LEVEL2_SELECTED_OBJECTIVE_STAGE_INDEX: usize = 3;
#[cfg(test)]
const LEVEL2_BEE_OBJECTIVE_STAGE_INDEX: usize = 2;
const LEVEL2_BEE_OBJECTIVE_AUTHORED_INDEX: u16 = 229;
/// `str_D7BD6[73]` values consumed by `sub_1BF90`'s class-3 target scan.
const LEVEL2_BEE_AWARENESS_RADIUS: i64 = 0x1400;
const LEVEL2_BEE_AWARENESS_HALF_ANGLE: i32 = 0x02AA;
/// `sub_1CE80` accepts a strict 3-D contact only below 1024 source units.
const LEVEL2_BEE_CONTACT_RADIUS: u32 = 1024;
/// Row-229's constructor subspell (`subSpellIndex_0x2A_42`) is 200 in the
/// qualified Day package; the live source field remains authoritative below.
#[cfg(test)]
const LEVEL2_BEE_CONTACT_SUBSPELL: i32 = 200;
/// Descriptor-73 movement values consumed by `sub_1B8C0` before the action-17
/// cadence scan. These are source fields, not tuning constants: the Bee
/// settles to 51 units above terrain, relaxes by -32 while airborne, probes
/// a 128-unit collision footprint, and rejects a tile slope of 20 or more.
const LEVEL2_BEE_GROUND_CLEARANCE: i16 = 0x0033;
const LEVEL2_BEE_GROUND_RELAXATION: i16 = -32;
const LEVEL2_BEE_COLLISION_HALF_EXTENT: i16 = 128;
/// `sub_106C0` Fireball source dimensions.  These are kept local to the Day
/// producer because the Level 1 projectile helper owns the same values for a
/// different arena, while this route must not create a second shared
/// projectile framework.
const LEVEL2_FIREBALL_COLLISION_HALF_WIDTH: i16 = 48;
const LEVEL2_FIREBALL_COLLISION_VERTICAL_OFFSET: i16 = 50;
const LEVEL2_FIREBALL_COLLISION_HALF_HEIGHT: i16 = 50;
const LEVEL2_BEE_TERRAIN_THRESHOLD: i32 = 20;
const LEVEL2_BEE_TURN_LIMIT: i16 = 0x0071;
const LEVEL2_BEE_MOVEMENT_SKIP_BIT: u32 = 0x0000_0800;
const LEVEL2_BEE_OBSTRUCTION_BIT: u32 = 0x0004_0000;
const LEVEL2_OBJECTIVE_POINT_RADIUS: i32 = 768;
const LEVEL2_NYPHUR_ENTITY_SHA256: &str =
    "a240c8711868a249b2e3ebb43c59613dc88d979a07de279ece869f14ffcf888e";
/// Qualified frame-zero `byte_0x3E_62` for the Nyphur entity. The source
/// dispatcher increments this byte after its callback, so the census hash
/// below normalizes only this known mutable field.
const LEVEL2_NYPHUR_INITIAL_PHASE: u8 = 113;
const LEVEL2_NYPHUR_PLAYER_SHA256: &str =
    "3859adcd13d8f32f27220c29e678581f4563fd051275ecc8b48390352dda5159";
const LEVEL2_NYPHUR_FIREBALL_SHA256: &str =
    "9312c195108896b32d553bdce73a43484bf80e010fcf762fc7c01d7f7ecca954";
const LEVEL2_NYPHUR_POSSESSION_SHA256: &str =
    "fedeb2b2fbe40c6ec72614bc4a971e2e826d52a8348156040e872c14a7c0a80e";

#[path = "campaign/level2.rs"]
mod campaign;
mod level2_nyphur;
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
    baseline_terrain: SessionTerrain,
    terrain: SessionTerrain,
    baseline_pickups: Box<[EntitySnapshot]>,
    pickups: Box<[EntitySnapshot]>,
    baseline_dynamic_pickups: Vec<EntitySnapshot>,
    dynamic_pickups: Vec<EntitySnapshot>,
    baseline_authored_pickup_slots: BTreeMap<u16, u16>,
    authored_pickup_slots: BTreeMap<u16, u16>,
    baseline_holders: Box<[EntitySnapshot]>,
    holders: Box<[EntitySnapshot]>,
    baseline_player_mana: i32,
    player_mana: i32,
    baseline_player_life_regen: i32,
    player_life_regen: i32,
    baseline_player_entity: EntitySnapshot,
    player_entity: EntitySnapshot,
    /// All source player records/entities are retained for the generic
    /// `sub_68FF0` linked-list scan.  The previous seam only kept player 0,
    /// which made a pickup silently local-only.
    baseline_players: Box<[PlayerSnapshot]>,
    players: Box<[PlayerSnapshot]>,
    baseline_player_entities: Box<[EntitySnapshot]>,
    player_entities: Box<[EntitySnapshot]>,
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
    /// Reachable family-6 active lifecycle. Other new active families remain
    /// explicitly outside this bounded slice.
    baseline_shield: Option<ShieldState>,
    shield: Option<ShieldState>,
    /// Reachable family-11 active lifecycle. AI visibility consumption remains
    /// explicitly outside this bounded slice.
    baseline_invisible: Option<InvisibleState>,
    invisible: Option<InvisibleState>,
    /// Reachable family-12 active lifecycle. The first-pulse effect allocation
    /// is intentionally not part of class-15/action-36; holder timing and
    /// cleanup are live.
    baseline_beyond_sight: Option<BeyondSightState>,
    beyond_sight: Option<BeyondSightState>,
    baseline_shield_damage_mailbox: Option<Level2ShieldDamageMailbox>,
    shield_damage_mailbox: Option<Level2ShieldDamageMailbox>,
    /// Transient player/event state consumed by the Beyond Sight first-pulse
    /// constructor. This is deliberately separate from the serialized entity
    /// record, matching the source `dword_0xA4_164x` allocation.
    baseline_beyond_sight_auxiliary: Level2PlayerEventAuxiliary,
    beyond_sight_auxiliary: Level2PlayerEventAuxiliary,
    /// Generic source acquisition states for authored Level 2 families 6,
    /// 11, and 12.  Family-specific active lifecycles remain separate.
    baseline_acquisitions: Box<[SpellHolderAcquisitionState]>,
    acquisitions: Box<[SpellHolderAcquisitionState]>,
    /// Source `OptionsSettingFlag_24 & 0x20`. `PlayerEvents_51BB0` toggles
    /// this session-global bit before the entity passes; cold
    /// `SetSpell_6D5E0` projections then force mana regeneration to zero and
    /// the holder's current mana to one. The option is runtime input, not an
    /// owner-package field.
    baseline_free_spell_usage: bool,
    free_spell_usage: bool,
    baseline_persistent_spell_experience: [i32; PLAYER_SPELL_COUNT],
    persistent_spell_experience: [i32; PLAYER_SPELL_COUNT],
    baseline_spell_experience: [i32; PLAYER_SPELL_COUNT],
    spell_experience: [i32; PLAYER_SPELL_COUNT],
    baseline_spell_levels: [i8; PLAYER_SPELL_COUNT],
    spell_levels: [i8; PLAYER_SPELL_COUNT],
    /// Persistent replacement rows can coexist with the authored ordinary
    /// source for the same family. Keep their source-local acquisition state
    /// separate instead of letting one family slot overwrite the other.
    baseline_persistent_acquisitions: BTreeMap<u16, SpellHolderAcquisitionState>,
    persistent_acquisitions: BTreeMap<u16, SpellHolderAcquisitionState>,
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
    pub speed: i16,
    pub target_sphere_slot: u16,
    /// Source `byte_0x3E_62`, used by `sub_5FF50`'s modulo cadence gate.
    /// This is explicitly injectable because it is not the Castle phase.
    pub source_byte_3e: u8,
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
    pub active: bool,
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
    /// Source `byte_0x3E_62`, incremented after every ApplyEvents callback.
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
    baseline_terrain: SessionTerrain,
    terrain: SessionTerrain,
    baseline_free_slots: Box<[u16]>,
    free_slots: Vec<u16>,
    baseline_slot_generations: BTreeMap<u16, u32>,
    slot_generations: BTreeMap<u16, u32>,
    baseline_slot_rng: BTreeMap<u16, u16>,
    slot_rng: BTreeMap<u16, u16>,
    baseline_global_rng: u32,
    global_rng: u32,
    /// Source entity[0] is the transient `sub_605E0`/`RemoveCastleStage`
    /// sentinel. Its rand_0x14 stream is independent of the Castle root.
    baseline_sentinel_rng: u16,
    sentinel_rng: u16,
    baseline_source_rng: BTreeMap<u16, u16>,
    source_rng: BTreeMap<u16, u16>,
    baseline_source_owner: BTreeMap<u16, u16>,
    source_owner: BTreeMap<u16, u16>,
    baseline_owner_class: BTreeMap<u16, u8>,
    owner_class: BTreeMap<u16, u8>,
    baseline_owner_max_mana: BTreeMap<u16, i32>,
    owner_max_mana: BTreeMap<u16, i32>,
    baseline_owner_mana_rows: Vec<Level2OwnerManaRow>,
    owner_mana_rows: Vec<Level2OwnerManaRow>,
    baseline_owner_mana_aggregate: BTreeMap<u16, i32>,
    owner_mana_aggregate: BTreeMap<u16, i32>,
    baseline_owner_mana_global: i32,
    owner_mana_global: i32,
    baseline_spheres: Vec<ManaSphere>,
    spheres: Vec<ManaSphere>,
    baseline_death_bursts: Vec<Level2DeathBurst>,
    death_bursts: Vec<Level2DeathBurst>,
    baseline_spell_replacements: Vec<Level2SpellReplacement>,
    spell_replacements: Vec<Level2SpellReplacement>,
    baseline_spell_pickups: Vec<Level2SpellPickupSource>,
    spell_pickups: Vec<Level2SpellPickupSource>,
    baseline_authored_model45: Vec<Level2AuthoredModel45Source>,
    authored_model45: Vec<Level2AuthoredModel45Source>,
    baseline_authored_model1: Vec<Level2AuthoredModel1Source>,
    authored_model1: Vec<Level2AuthoredModel1Source>,
    baseline_authored_model13: Vec<Level2AuthoredModel13Source>,
    authored_model13: Vec<Level2AuthoredModel13Source>,
    baseline_authored_bees: Vec<Level2AuthoredBeeSource>,
    authored_bees: Vec<Level2AuthoredBeeSource>,
    baseline_bee_model2_constructor_phase: u8,
    bee_model2_constructor_phase: u8,
    baseline_authored_switch_producers: Vec<Level2AuthoredSwitchProducerSource>,
    authored_switch_producers: Vec<Level2AuthoredSwitchProducerSource>,
    baseline_authored_trees: Vec<Level2AuthoredTreeSource>,
    authored_trees: Vec<Level2AuthoredTreeSource>,
    baseline_authored_model32: Vec<Level2AuthoredModel32Source>,
    authored_model32: Vec<Level2AuthoredModel32Source>,
    baseline_authored_model59: Vec<Level2AuthoredModel59Source>,
    authored_model59: Vec<Level2AuthoredModel59Source>,
    baseline_model59_smoke_children: Vec<Level2Model59SmokeChild>,
    model59_smoke_children: Vec<Level2Model59SmokeChild>,
    baseline_authored_model53: Vec<Level2AuthoredModel53Source>,
    authored_model53: Vec<Level2AuthoredModel53Source>,
    baseline_authored_model5_water_splashes: Vec<Level2AuthoredModel5WaterSplashSource>,
    authored_model5_water_splashes: Vec<Level2AuthoredModel5WaterSplashSource>,
    baseline_model13_spawned_children: Vec<Level2Model13SpawnedChild>,
    model13_spawned_children: Vec<Level2Model13SpawnedChild>,
    baseline_authored_model6: Vec<Level2AuthoredModel6Source>,
    authored_model6: Vec<Level2AuthoredModel6Source>,
    baseline_model6_spawned_children: Vec<Level2Model6SpawnedChild>,
    model6_spawned_children: Vec<Level2Model6SpawnedChild>,
    baseline_authored_model17: Vec<Level2AuthoredModel17Source>,
    authored_model17: Vec<Level2AuthoredModel17Source>,
    baseline_model17_spawned_children: Vec<Level2Model17SpawnedChild>,
    model17_spawned_children: Vec<Level2Model17SpawnedChild>,
    baseline_authored_model22: Vec<Level2AuthoredModel22Source>,
    authored_model22: Vec<Level2AuthoredModel22Source>,
    baseline_model22_spawned_children: Vec<Level2Model22SpawnedChild>,
    model22_spawned_children: Vec<Level2Model22SpawnedChild>,
    baseline_authored_model76: Vec<Level2AuthoredModel76Source>,
    authored_model76: Vec<Level2AuthoredModel76Source>,
    baseline_model76_children: Vec<Level2Model76Child>,
    model76_children: Vec<Level2Model76Child>,
    baseline_model76_tail_spawns: Vec<Level2Model76TailSpawn>,
    model76_tail_spawns: Vec<Level2Model76TailSpawn>,
    baseline_authored_class14: Vec<Level2AuthoredClass14Source>,
    authored_class14: Vec<Level2AuthoredClass14Source>,
    baseline_authored_class5: Vec<Level2AuthoredClass5Source>,
    authored_class5: Vec<Level2AuthoredClass5Source>,
    baseline_authored_class5_runtime: Vec<Level2AuthoredClass5RuntimeState>,
    authored_class5_runtime: Vec<Level2AuthoredClass5RuntimeState>,
    baseline_authored_class5_model3: Vec<Level2AuthoredClass5Model3State>,
    authored_class5_model3: Vec<Level2AuthoredClass5Model3State>,
    baseline_authored_class5_model3_children: Vec<Level2AuthoredClass5Model3Child>,
    authored_class5_model3_children: Vec<Level2AuthoredClass5Model3Child>,
    baseline_authored_class5_archer: Vec<Level2AuthoredClass5ArcherState>,
    authored_class5_archer: Vec<Level2AuthoredClass5ArcherState>,
    baseline_authored_class5_archer_children: Vec<Level2AuthoredClass5ArcherChild>,
    authored_class5_archer_children: Vec<Level2AuthoredClass5ArcherChild>,
    baseline_house_residents: Vec<Level2HouseResidentSource>,
    house_residents: Vec<Level2HouseResidentSource>,
    baseline_class5_model3_constructor_phase: u8,
    class5_model3_constructor_phase: u8,
    baseline_class5_model9_constructor_phase: u8,
    class5_model9_constructor_phase: u8,
    baseline_class5_model14_constructor_phase: u8,
    class5_model14_constructor_phase: u8,
    baseline_class5_model4_constructor_phase: u8,
    class5_model4_constructor_phase: u8,
    /// Source `word_0x36DFE`, written by `sub_51570` for the latest model-3
    /// event and left unchanged by model 5.
    baseline_class14_model3_handle: u16,
    class14_model3_handle: u16,
    baseline_beyond_sight_effects: Vec<Level2BeyondSightPulseEffect>,
    beyond_sight_effects: Vec<Level2BeyondSightPulseEffect>,
    baseline_mana_attraction_fields: Vec<Level2ManaAttractionField>,
    mana_attraction_fields: Vec<Level2ManaAttractionField>,
    baseline_possession_projectiles: Vec<Level2PossessionProjectile>,
    possession_projectiles: Vec<Level2PossessionProjectile>,
    baseline_possession_guidance_source_slots: Vec<u16>,
    possession_guidance_source_slots: Vec<u16>,
    baseline_possession_guidance_stale_records: BTreeMap<u16, Position>,
    possession_guidance_stale_records: BTreeMap<u16, Position>,
    baseline_possession_effects: Vec<Level2PossessionEffect>,
    possession_effects: Vec<Level2PossessionEffect>,
    /// Transient public Fireball products.  These are born from the live
    /// family-0 holder and therefore intentionally have no baseline twin.
    fireball_projectiles: Vec<Level2FireballProjectile>,
    baseline_carriers: Vec<ManaCarrier>,
    carriers: Vec<ManaCarrier>,
    baseline_castle_projectiles: Vec<Level2CastleProjectile>,
    castle_projectiles: Vec<Level2CastleProjectile>,
    baseline_castle: Option<Level2CastleState>,
    castle: Option<Level2CastleState>,
    baseline_castle_children: Vec<Level2CastleChild>,
    castle_children: Vec<Level2CastleChild>,
    baseline_castle_footprint: Option<CastleBuildFootprint>,
    castle_footprint: Option<CastleBuildFootprint>,
    /// Source `RemoveCastleStage_385C0` indexes the old Castle level through
    /// DATA_BUILD00DATTAB's `byte_0x46`, rather than always using record 1.
    /// The qualified package currently binds stage 1; additional preserved
    /// TAB records can be installed through `set_castle_stage_footprint`.
    baseline_castle_stage_footprints: BTreeMap<u8, CastleBuildFootprint>,
    castle_stage_footprints: BTreeMap<u8, CastleBuildFootprint>,
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
    baseline_player_spell_holder_action: u16,
    player_spell_holder_action: u16,
    baseline_player_spell_holder_max_action: u16,
    player_spell_holder_max_action: u16,
    /// Imported `sub_60810` life coefficient per Castle stage. The owner
    /// export does not expose the nested player-state array, so fixed-profile
    /// sessions default to the source identity coefficient (256); callers
    /// preserving those bytes can bind each stage explicitly.
    baseline_castle_stage_coefficients: [u16; 8],
    castle_stage_coefficients: [u16; 8],
    /// Imported `sub_613D0` controller coefficient from the player's
    /// `array_0x24E[9 + level]`. The owner export does not expose this
    /// nested byte array, so fixed-profile sessions bind the explicit source
    /// identity value `1` unless a caller provides the preserved byte.
    baseline_castle_controller_coefficients: [u8; 8],
    castle_controller_coefficients: [u8; 8],
    events: Vec<Level2EconomyEvent>,
}

mod level2_class5;
mod level2_dynamic_economy;

pub(crate) const MODEL3_TARGET_RANGE: u32 = 0x1400;
pub(crate) const MODEL3_TARGET_HALF_ANGLE: i16 = 0x00aa;
pub(crate) const MODEL3_RAW_SOURCE_DISABLED_BIT: u32 = 0x20;

impl Level2EconomyState {
    #[cfg(test)]
    fn from_parts(
        terrain: SessionTerrain,
        entities: &[EntitySnapshot],
        player_mana: i32,
        player_model: u8,
        player_terrain_flags: u8,
        player_spell_slot_2: u16,
        global_rng: u32,
    ) -> Result<Self> {
        Self::from_parts_with_castle_stage_footprints(
            terrain,
            entities,
            player_mana,
            player_model,
            player_terrain_flags,
            player_spell_slot_2,
            global_rng,
            BTreeMap::new(),
        )
    }

    fn from_parts_with_castle_stage_footprints(
        terrain: SessionTerrain,
        entities: &[EntitySnapshot],
        player_mana: i32,
        player_model: u8,
        player_terrain_flags: u8,
        player_spell_slot_2: u16,
        global_rng: u32,
        castle_stage_footprints: BTreeMap<u8, CastleBuildFootprint>,
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
            .map(|entity| (entity.index, entity.source_player_entity_index()))
            .collect::<BTreeMap<_, _>>();
        let baseline_owner_max_mana = entities
            .iter()
            .filter(|entity| entity.active)
            .map(|entity| (entity.index, entity.max_mana))
            .collect::<BTreeMap<_, _>>();
        let static_spheres = level2_static_mana_spheres_from_entities(entities, &source_owner);
        let owner_mana_rows = level2_owner_mana_rows_from_entities(entities);
        let owner_max_mana =
            level2_owner_max_mana_projection(&baseline_owner_max_mana, &owner_mana_rows);
        let (owner_mana_aggregate, owner_mana_global) =
            level2_owner_mana_projection(&owner_mana_rows);
        let sentinel_rng = entities
            .iter()
            .find(|entity| entity.index == 0)
            .map_or(0, |entity| entity.entity_rng);
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
        let (player_spell_holder_action, player_spell_holder_max_action) =
            if let Some(holder_index) = (player_spell_slot_2 != 0).then_some(player_spell_slot_2) {
                entities
                    .get(usize::from(holder_index))
                    .map_or((0, 0), |holder| {
                        (holder.source_word_2e(), holder.source_word_30())
                    })
            } else {
                (0, 0)
            };
        let mut state = Self {
            baseline_terrain: terrain.clone(),
            terrain,
            baseline_free_slots: free_slots.clone().into_boxed_slice(),
            free_slots,
            baseline_slot_generations: BTreeMap::new(),
            slot_generations: BTreeMap::new(),
            baseline_slot_rng: slot_rng.clone(),
            slot_rng,
            baseline_global_rng: global_rng,
            global_rng,
            baseline_sentinel_rng: sentinel_rng,
            sentinel_rng,
            baseline_source_rng: BTreeMap::new(),
            source_rng: BTreeMap::new(),
            baseline_source_owner: source_owner.clone(),
            source_owner,
            baseline_owner_class: owner_class.clone(),
            owner_class,
            baseline_owner_max_mana,
            owner_max_mana,
            baseline_owner_mana_rows: owner_mana_rows.clone(),
            owner_mana_rows,
            baseline_owner_mana_aggregate: owner_mana_aggregate.clone(),
            owner_mana_aggregate,
            baseline_owner_mana_global: owner_mana_global,
            owner_mana_global,
            baseline_spheres: static_spheres.clone(),
            spheres: static_spheres,
            baseline_death_bursts: Vec::new(),
            death_bursts: Vec::new(),
            baseline_spell_replacements: Vec::new(),
            spell_replacements: Vec::new(),
            baseline_spell_pickups: Vec::new(),
            spell_pickups: Vec::new(),
            baseline_authored_model45: Vec::new(),
            authored_model45: Vec::new(),
            baseline_authored_model1: Vec::new(),
            authored_model1: Vec::new(),
            baseline_authored_model13: Vec::new(),
            authored_model13: Vec::new(),
            baseline_authored_bees: Vec::new(),
            authored_bees: Vec::new(),
            baseline_bee_model2_constructor_phase: 0,
            bee_model2_constructor_phase: 0,
            baseline_authored_switch_producers: Vec::new(),
            authored_switch_producers: Vec::new(),
            baseline_authored_trees: Vec::new(),
            authored_trees: Vec::new(),
            baseline_authored_model32: Vec::new(),
            authored_model32: Vec::new(),
            baseline_authored_model59: Vec::new(),
            authored_model59: Vec::new(),
            baseline_model59_smoke_children: Vec::new(),
            model59_smoke_children: Vec::new(),
            baseline_authored_model53: Vec::new(),
            authored_model53: Vec::new(),
            baseline_authored_model5_water_splashes: Vec::new(),
            authored_model5_water_splashes: Vec::new(),
            baseline_model13_spawned_children: Vec::new(),
            model13_spawned_children: Vec::new(),
            baseline_authored_model6: Vec::new(),
            authored_model6: Vec::new(),
            baseline_model6_spawned_children: Vec::new(),
            model6_spawned_children: Vec::new(),
            baseline_authored_model17: Vec::new(),
            authored_model17: Vec::new(),
            baseline_model17_spawned_children: Vec::new(),
            model17_spawned_children: Vec::new(),
            baseline_authored_model22: Vec::new(),
            authored_model22: Vec::new(),
            baseline_model22_spawned_children: Vec::new(),
            model22_spawned_children: Vec::new(),
            baseline_authored_model76: Vec::new(),
            authored_model76: Vec::new(),
            baseline_model76_children: Vec::new(),
            model76_children: Vec::new(),
            baseline_model76_tail_spawns: Vec::new(),
            model76_tail_spawns: Vec::new(),
            baseline_authored_class14: Vec::new(),
            authored_class14: Vec::new(),
            baseline_authored_class5: Vec::new(),
            authored_class5: Vec::new(),
            baseline_authored_class5_runtime: Vec::new(),
            authored_class5_runtime: Vec::new(),
            baseline_authored_class5_model3: Vec::new(),
            authored_class5_model3: Vec::new(),
            baseline_authored_class5_model3_children: Vec::new(),
            authored_class5_model3_children: Vec::new(),
            baseline_authored_class5_archer: Vec::new(),
            authored_class5_archer: Vec::new(),
            baseline_authored_class5_archer_children: Vec::new(),
            authored_class5_archer_children: Vec::new(),
            baseline_house_residents: Vec::new(),
            house_residents: Vec::new(),
            baseline_class5_model3_constructor_phase: 0,
            class5_model3_constructor_phase: 0,
            baseline_class5_model9_constructor_phase: 0,
            class5_model9_constructor_phase: 0,
            baseline_class5_model14_constructor_phase: 0,
            class5_model14_constructor_phase: 0,
            baseline_class5_model4_constructor_phase: 0,
            class5_model4_constructor_phase: 0,
            baseline_class14_model3_handle: 0,
            class14_model3_handle: 0,
            baseline_beyond_sight_effects: Vec::new(),
            beyond_sight_effects: Vec::new(),
            baseline_mana_attraction_fields: Vec::new(),
            mana_attraction_fields: Vec::new(),
            baseline_possession_projectiles: Vec::new(),
            possession_projectiles: Vec::new(),
            baseline_possession_guidance_source_slots: Vec::new(),
            possession_guidance_source_slots: Vec::new(),
            baseline_possession_guidance_stale_records: BTreeMap::new(),
            possession_guidance_stale_records: BTreeMap::new(),
            baseline_possession_effects: Vec::new(),
            possession_effects: Vec::new(),
            fireball_projectiles: Vec::new(),
            baseline_carriers: Vec::new(),
            carriers: Vec::new(),
            baseline_castle_projectiles: Vec::new(),
            castle_projectiles: Vec::new(),
            baseline_castle: None,
            castle: None,
            baseline_castle_children: Vec::new(),
            castle_children: Vec::new(),
            baseline_castle_footprint: None,
            castle_footprint: None,
            baseline_castle_stage_footprints: castle_stage_footprints.clone(),
            castle_stage_footprints,
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
            baseline_player_model: player_model,
            player_model,
            baseline_player_terrain_flags: player_terrain_flags,
            player_terrain_flags,
            baseline_player_spell_slot_2: player_spell_slot_2,
            player_spell_slot_2,
            baseline_player_spell_holder_action: player_spell_holder_action,
            player_spell_holder_action,
            baseline_player_spell_holder_max_action: player_spell_holder_max_action,
            player_spell_holder_max_action,
            baseline_castle_stage_coefficients: [256; 8],
            castle_stage_coefficients: [256; 8],
            baseline_castle_controller_coefficients: [1; 8],
            castle_controller_coefficients: [1; 8],
            events: Vec::new(),
        };
        // The imported owner rows and typed static sphere projection use the
        // same source slot. Rebuild once after construction so the
        // install/replacement semantics remain explicit if a future profile
        // includes a source-linked model-39 row outside the baseline filter.
        state.refresh_owner_mana_rows();
        state.publish_owner_mana_projection();
        Ok(state)
    }

    fn for_each_live_replaceable_arena_row(
        &self,
        mut visit: impl FnMut(u16, Level2DynamicArenaKind),
    ) {
        for child in self.castle_children.iter().filter(|child| {
            child.active
                && child.source_class == 5
                && child.model == 15
                && child.control_flags & SOURCE_REPLACEABLE_FLAG != 0
        }) {
            visit(
                child.source_entity_index,
                Level2DynamicArenaKind::CastleAuxiliary,
            );
        }

        macro_rules! visit_flagged_rows {
            ($rows:expr, $kind:ident) => {
                for row in $rows
                    .iter()
                    .filter(|row| row.active && row.control_flags & SOURCE_REPLACEABLE_FLAG != 0)
                {
                    visit(row.source_entity_index, Level2DynamicArenaKind::$kind);
                }
            };
        }

        // These are the explicitly modeled economy-arena carriers whose
        // stored control dword carries byte[2] & 2. This owner-local scan does
        // not claim authority over Level 2 rows outside this economy arena.
        visit_flagged_rows!(self.spheres, ManaSphere);
        visit_flagged_rows!(self.death_bursts, DeathBurst);
        visit_flagged_rows!(self.authored_model1, AuthoredModel1);
        visit_flagged_rows!(self.model59_smoke_children, Model59SmokeChild);
        visit_flagged_rows!(
            self.authored_model5_water_splashes,
            AuthoredModel5WaterSplash
        );
        visit_flagged_rows!(self.authored_model13, AuthoredModel13);
        visit_flagged_rows!(self.authored_model6, AuthoredModel6);
        visit_flagged_rows!(self.model13_spawned_children, Model13SpawnedChild);
        visit_flagged_rows!(self.model17_spawned_children, Model17SpawnedChild);
        visit_flagged_rows!(self.model76_tail_spawns, Model76TailSpawn);
    }

    fn allocate_slot(&mut self) -> Option<u16> {
        if let Some(slot) = self.free_slots.pop() {
            // `NewEvent` overwrites the fixed source record at this slot.
            self.possession_guidance_stale_records.remove(&slot);
            let generation = self.slot_generations.entry(slot).or_default();
            *generation = generation.wrapping_add(1);
            return Some(slot);
        }

        // `NewEvent_4A050` falls through to the lowest live replaceable row
        // when `sub_49F90` has no ordinary free slot. Evict the typed
        // projection directly; `release_slot` would incorrectly add this row
        // to the ordinary free stack and would lose the source relation that
        // `sub_5FF50` cleans for Castle auxiliaries.
        let mut candidate = None;
        self.for_each_live_replaceable_arena_row(|slot, kind| {
            if candidate.map_or(true, |(current, _)| slot < current) {
                candidate = Some((slot, kind));
            }
        });
        let (slot, kind) = candidate?;
        self.possession_guidance_stale_records.remove(&slot);

        macro_rules! remove_flagged_row {
            ($rows:expr) => {{
                let Some(index) = $rows.iter().position(|row| {
                    row.active
                        && row.source_entity_index == slot
                        && row.control_flags & SOURCE_REPLACEABLE_FLAG != 0
                }) else {
                    return None;
                };
                $rows.remove(index);
            }};
        }

        match kind {
            Level2DynamicArenaKind::CastleAuxiliary => {
                let Some(index) = self.castle_children.iter().position(|child| {
                    child.active
                        && child.source_entity_index == slot
                        && child.source_class == 5
                        && child.model == 15
                        && child.control_flags & SOURCE_REPLACEABLE_FLAG != 0
                }) else {
                    return None;
                };
                self.castle_children.remove(index);
            }
            Level2DynamicArenaKind::ManaSphere => remove_flagged_row!(self.spheres),
            Level2DynamicArenaKind::DeathBurst => remove_flagged_row!(self.death_bursts),
            Level2DynamicArenaKind::AuthoredModel1 => remove_flagged_row!(self.authored_model1),
            Level2DynamicArenaKind::Model59SmokeChild => {
                remove_flagged_row!(self.model59_smoke_children);
            }
            Level2DynamicArenaKind::AuthoredModel5WaterSplash => {
                remove_flagged_row!(self.authored_model5_water_splashes);
            }
            Level2DynamicArenaKind::AuthoredModel13 => {
                remove_flagged_row!(self.authored_model13);
            }
            Level2DynamicArenaKind::AuthoredModel6 => remove_flagged_row!(self.authored_model6),
            Level2DynamicArenaKind::Model13SpawnedChild => {
                remove_flagged_row!(self.model13_spawned_children);
            }
            Level2DynamicArenaKind::Model17SpawnedChild => {
                remove_flagged_row!(self.model17_spawned_children);
            }
            Level2DynamicArenaKind::Model76TailSpawn => {
                remove_flagged_row!(self.model76_tail_spawns);
            }
            _ => return None,
        }
        let _ = self.map_chains.sync_entity(RenderableMapSeed {
            entity_index: slot,
            active: false,
            x: 0,
            y: 0,
        });
        self.slot_rng.remove(&slot);
        self.source_rng.remove(&slot);
        let generation = self.slot_generations.entry(slot).or_default();
        *generation = generation.wrapping_add(1);
        Some(slot)
    }

    #[must_use]
    fn new_event_rng(&self, source_slot: u16) -> u16 {
        // Pinned remc2 NewEvent_4A050: rand_0x14 is the source slot plus the
        // global D41A0.rand_0x8 stream, not the inactive row's stale RNG.
        source_slot.wrapping_add(self.global_rng as u16)
    }

    /// Advance the source-global `D41A0.rand_0x8` stream once for each
    /// `ApplyEvents_498A0` invocation. The source keeps this as a full
    /// 32-bit value; NewEvent consumes its low word only after adding the
    /// allocated source slot. Keeping the full stream is required so the
    /// next source pass does not silently repeat after a high-word carry.
    fn advance_global_rng(&mut self) {
        self.global_rng = level2_next_rng32(self.global_rng);
    }

    fn release_slot(&mut self, slot: u16, entity_rng: Option<u16>) {
        if slot == 0 {
            return;
        }
        let _ = self.map_chains.sync_entity(RenderableMapSeed {
            entity_index: slot,
            active: false,
            x: 0,
            y: 0,
        });
        if self.free_slots.contains(&slot) {
            return;
        }
        if let Some(entity_rng) = entity_rng {
            self.slot_rng.insert(slot, entity_rng);
        }
        self.free_slots.push(slot);
        self.free_slots
            .sort_unstable_by(|left, right| right.cmp(left));
    }

    /// Rebuild the source free-arena sidecars using the validated reset pool
    /// and the currently live typed rows.  This is the bounded equivalent of
    /// `sub_49F90`: reclaim rows already marked inactive, then retry the
    /// allocator from descending source-slot order without inventing a new
    /// pool or changing any live relation.
    fn rebuild_free_arena(&mut self) {
        self.reclaim_inactive_slots();
        let occupied = self
            .live_arena_cursor_rows()
            .into_iter()
            .map(|row| row.source_slot)
            // Possession callbacks disable their row in the landing pass but
            // source `UpdateEntities_57730` does not free it until the next
            // pass prologue. It is inactive (and therefore absent from the
            // live cursor) yet must remain unavailable to `NewEvent`.
            .chain(
                self.possession_projectiles
                    .iter()
                    .filter(|projectile| !projectile.active)
                    .map(|projectile| projectile.source_entity_index),
            )
            .chain(
                self.possession_effects
                    .iter()
                    .filter(|effect| !effect.active)
                    .map(|effect| effect.source_entity_index),
            )
            .chain(
                self.fireball_projectiles
                    .iter()
                    .filter(|projectile| !projectile.active && projectile.drawing_disabled)
                    .map(|projectile| projectile.source_entity_index),
            )
            .collect::<BTreeSet<_>>();
        self.free_slots = self
            .baseline_free_slots
            .iter()
            .copied()
            .filter(|slot| !occupied.contains(slot))
            .collect();
        self.free_slots
            .sort_unstable_by(|left, right| right.cmp(left));
    }

    #[must_use]
    pub fn spheres(&self) -> &[ManaSphere] {
        &self.spheres
    }

    #[must_use]
    pub fn death_bursts(&self) -> &[Level2DeathBurst] {
        &self.death_bursts
    }

    #[must_use]
    pub fn spell_replacements(&self) -> &[Level2SpellReplacement] {
        &self.spell_replacements
    }

    #[must_use]
    pub fn spell_pickups(&self) -> &[Level2SpellPickupSource] {
        &self.spell_pickups
    }

    #[must_use]
    pub fn authored_model45(&self) -> &[Level2AuthoredModel45Source] {
        &self.authored_model45
    }

    #[must_use]
    pub fn authored_model1(&self) -> &[Level2AuthoredModel1Source] {
        &self.authored_model1
    }

    #[must_use]
    pub fn authored_model13(&self) -> &[Level2AuthoredModel13Source] {
        &self.authored_model13
    }

    #[must_use]
    pub fn authored_bees(&self) -> &[Level2AuthoredBeeSource] {
        &self.authored_bees
    }

    #[must_use]
    pub fn authored_switch_producers(&self) -> &[Level2AuthoredSwitchProducerSource] {
        &self.authored_switch_producers
    }

    #[must_use]
    pub fn authored_trees(&self) -> &[Level2AuthoredTreeSource] {
        &self.authored_trees
    }

    #[must_use]
    pub fn authored_model32(&self) -> &[Level2AuthoredModel32Source] {
        &self.authored_model32
    }

    #[must_use]
    pub fn authored_model59(&self) -> &[Level2AuthoredModel59Source] {
        &self.authored_model59
    }

    #[must_use]
    pub fn model59_smoke_children(&self) -> &[Level2Model59SmokeChild] {
        &self.model59_smoke_children
    }

    #[must_use]
    pub fn authored_model53(&self) -> &[Level2AuthoredModel53Source] {
        &self.authored_model53
    }

    #[must_use]
    pub fn authored_model5_water_splashes(&self) -> &[Level2AuthoredModel5WaterSplashSource] {
        &self.authored_model5_water_splashes
    }

    #[must_use]
    pub fn model13_spawned_children(&self) -> &[Level2Model13SpawnedChild] {
        &self.model13_spawned_children
    }

    #[must_use]
    pub fn authored_model6(&self) -> &[Level2AuthoredModel6Source] {
        &self.authored_model6
    }

    #[must_use]
    pub fn model6_spawned_children(&self) -> &[Level2Model6SpawnedChild] {
        &self.model6_spawned_children
    }

    #[must_use]
    pub fn authored_model17(&self) -> &[Level2AuthoredModel17Source] {
        &self.authored_model17
    }

    #[must_use]
    pub fn model17_spawned_children(&self) -> &[Level2Model17SpawnedChild] {
        &self.model17_spawned_children
    }

    #[must_use]
    pub fn authored_model22(&self) -> &[Level2AuthoredModel22Source] {
        &self.authored_model22
    }

    #[must_use]
    pub fn model22_spawned_children(&self) -> &[Level2Model22SpawnedChild] {
        &self.model22_spawned_children
    }

    #[must_use]
    pub fn authored_model76(&self) -> &[Level2AuthoredModel76Source] {
        &self.authored_model76
    }

    #[must_use]
    pub fn model76_children(&self) -> &[Level2Model76Child] {
        &self.model76_children
    }

    /// Bind the exact `array_0x52_82` collision selectors copied by the
    /// source `SetEntityIndexAndRot_49CD0` constructor. The orbit words are
    /// intentionally not overwritten: they are a separate callback state.
    pub fn bind_model76_collision_axes(
        &mut self,
        parent_entity_index: u16,
        collision_yaw: i16,
        collision_pitch: i16,
        collision_roll: i16,
        collision_fov: i16,
    ) -> bool {
        let mut bound = false;
        for child in self
            .model76_children
            .iter_mut()
            .filter(|child| child.parent_entity_index == parent_entity_index)
        {
            child.collision_yaw = collision_yaw;
            child.collision_pitch = collision_pitch;
            child.collision_roll = collision_roll;
            child.collision_fov = collision_fov;
            bound = true;
        }
        bound
    }

    #[must_use]
    pub fn model76_tail_spawns(&self) -> &[Level2Model76TailSpawn] {
        &self.model76_tail_spawns
    }

    #[must_use]
    pub fn authored_class14(&self) -> &[Level2AuthoredClass14Source] {
        &self.authored_class14
    }

    #[must_use]
    pub fn authored_class5(&self) -> &[Level2AuthoredClass5Source] {
        &self.authored_class5
    }

    #[must_use]
    pub fn authored_class5_runtime(&self) -> &[Level2AuthoredClass5RuntimeState] {
        &self.authored_class5_runtime
    }

    #[must_use]
    pub fn authored_class5_model3(&self) -> &[Level2AuthoredClass5Model3State] {
        &self.authored_class5_model3
    }

    #[must_use]
    pub fn authored_class5_model3_children(&self) -> &[Level2AuthoredClass5Model3Child] {
        &self.authored_class5_model3_children
    }

    #[must_use]
    pub fn authored_class5_archer(&self) -> &[Level2AuthoredClass5ArcherState] {
        &self.authored_class5_archer
    }

    #[must_use]
    pub fn authored_class5_archer_children(&self) -> &[Level2AuthoredClass5ArcherChild] {
        &self.authored_class5_archer_children
    }

    fn authored_class5_runtime_mut(
        &mut self,
        source_entity_index: u16,
    ) -> Option<&mut Level2AuthoredClass5RuntimeState> {
        self.authored_class5_runtime
            .iter_mut()
            .find(|state| state.source_entity_index == source_entity_index)
    }

    #[must_use]
    pub const fn class14_model3_handle(&self) -> u16 {
        self.class14_model3_handle
    }

    #[must_use]
    pub fn beyond_sight_effects(&self) -> &[Level2BeyondSightPulseEffect] {
        &self.beyond_sight_effects
    }

    #[must_use]
    pub fn mana_attraction_fields(&self) -> &[Level2ManaAttractionField] {
        &self.mana_attraction_fields
    }

    /// Allocate the source class-10/model-54 temporary field from an authored
    /// feature. `sub_4A310` binds `dword_0x10` to `(stageTag << 8)^2` and
    /// `maxLife` to `max(8 * stageTag + 16, 128)` before `sub_38D80` runs.
    /// Action 76 is the pinned duplicate dispatch row and is accepted without
    /// changing the source constructor payload.
    pub fn allocate_mana_attraction_field(
        &mut self,
        authored_index: u16,
        position: Position,
        stage_tag: i16,
        action: u8,
    ) -> Option<u16> {
        debug_assert_eq!((MANA_ATTRACTION_CLASS, MANA_ATTRACTION_MODEL), (10, 54));
        if !matches!(
            action,
            MANA_ATTRACTION_ACTION | MANA_ATTRACTION_ALIAS_ACTION
        ) || stage_tag < 0
        {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let stage = u32::try_from(stage_tag).ok()?;
        let radius = stage.checked_mul(256)?;
        let max_life = i16::try_from((8 * stage + 16).max(128)).ok()?;
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        self.mana_attraction_fields.push(Level2ManaAttractionField {
            source_entity_index,
            entity_rng,
            authored_index,
            position,
            radius_squared: radius.saturating_mul(radius),
            max_life,
            life: max_life,
            action,
            active: true,
        });
        self.source_rng.insert(source_entity_index, entity_rng);
        Some(source_entity_index)
    }

    /// Allocate an authored class-15 source row through the shared
    /// `NewEvent_4A050` arena.  The caller supplies the already decoded
    /// `sub_4A310` family/action and terrain-centred position.
    pub fn allocate_spell_pickup(
        &mut self,
        authored_index: u16,
        family: u8,
        action: u8,
        position: Position,
    ) -> Option<Level2SpellPickupSource> {
        if physical_spell_action(family, PhysicalSpellActionRole::OrdinaryPickup) != Some(action) {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2SpellPickupSource {
            source_entity_index,
            entity_rng,
            authored_index,
            family,
            action,
            phase: source_entity_index as u8,
            position,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.spell_pickups.push(source);
        Some(source)
    }

    /// Allocate the class-15 holder made by `AddSpellXX_XX_51120` during the
    /// isolated `sub_5CF40` pass. These rows use the source action `3*family`
    /// rather than the ordinary pickup action and have no authored pickup id.
    fn allocate_starting_spell_holder(
        &mut self,
        family: u8,
        position: Position,
    ) -> Option<Level2StartingSpellHolderSource> {
        if family > 3 || self.free_slot_count() == 0 {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2StartingSpellHolderSource {
            source_entity_index,
            entity_rng,
            family,
            action: family.saturating_mul(3),
            phase: source_entity_index as u8,
            position,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        Some(source)
    }

    /// Allocate the authored class-10/model-1/action-1 effect from the
    /// pinned `NewAdd0A01_4E3B0` constructor. The immediate `AddEvent2`
    /// presentation child is intentionally deferred; the root's source
    /// arena identity and Quickfair callback are source-backed here.
    pub fn allocate_authored_model1(
        &mut self,
        authored_index: u16,
        position: Position,
    ) -> Option<Level2AuthoredModel1Source> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let mut position = position;
        position.z = self.terrain.ground_height(position);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2AuthoredModel1Source {
            source_entity_index,
            entity_rng,
            authored_index,
            model: 1,
            action: 1,
            subspell_index: 400,
            max_life: 1,
            life: 1,
            // NewAdd0A01 clears bits 3/17, then sets byte 2 bit 1.
            control_flags: 0x0002_0000,
            particle_index: 41,
            last_sound_id: 0,
            position,
            // NewEvent seeds byte 0x3E from the allocated source slot; the
            // Quickfair constructor does not overwrite that cursor.
            phase: source_entity_index as u8,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model1.push(source);
        Some(source)
    }

    /// Allocate the class-10/model-45/action-51 producer row from authored
    /// group 8.  The qualified rows use BLDGPRM/BUILD entry 21; keeping that
    /// source-data binding explicit prevents a generic model-45 shim from
    /// silently accepting a different authored parameter.
    pub fn allocate_authored_model45(
        &mut self,
        authored_index: u16,
        target_group: u16,
        parameter_1: u16,
        position: Position,
    ) -> Option<Level2AuthoredModel45Source> {
        if parameter_1 != 21 {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        // BUILD0-0.TAB[21] is raw 15x15 in the qualified owner package.
        // CreateIndexes_6EB90 doubles it to runtime 30x30 under VGA selector
        // 1, then sub_49A30 halves the runtime dimensions back to an
        // effective 15x15 view/terrain footprint.
        let raw_width = 15_u16;
        let raw_height = 15_u16;
        let runtime_width = raw_width * 2;
        let runtime_height = raw_height * 2;
        let width = runtime_width >> 1;
        let height = runtime_height >> 1;
        let mut source_position = position;
        // sub_49A30 first copies the position snapped to 256-unit tile
        // boundaries, then applies its parity adjustment below.
        source_position.x &= 0xff00;
        source_position.y &= 0xff00;
        let tile_x = source_position.x >> 8;
        let tile_y = source_position.y >> 8;
        let mut corner_x = tile_x.wrapping_sub(width / 2);
        let corner_y = tile_y.wrapping_sub(height / 2);
        if (u16::from(corner_x) + u16::from(corner_y)) % 2 != 0 {
            source_position.x = source_position.x.wrapping_add(256);
            corner_x = corner_x.wrapping_add(1);
        }
        let corner_height = |x: u16, y: u16| {
            i16::from(
                self.terrain.planes.ground_heights
                    [terrain_index(x.to_le_bytes()[0], y.to_le_bytes()[0])],
            )
        };
        let average_height = (corner_height(corner_x, corner_y)
            + corner_height(corner_x.wrapping_add(width), corner_y)
            + corner_height(corner_x.wrapping_add(width), corner_y.wrapping_add(height))
            + corner_height(corner_x, corner_y.wrapping_add(height)))
            >> 2;
        source_position.z = average_height.saturating_mul(32);
        let minimum_speed = i16::try_from((runtime_width * runtime_height) >> 4)
            .expect("qualified BUILD dimensions fit minimum speed");
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: source_position.x,
                y: source_position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let subspell_index = 6_u16;
        // NewEvent zeroes mana.  BLDGPRM[21].byte_2 has bit 3 set, so
        // sub_49A30 skips the branch that derives mana from subSpellIndex.
        let mana = 0;
        let source = Level2AuthoredModel45Source {
            source_entity_index,
            entity_rng,
            authored_index,
            target_group,
            parameter_1,
            source_class: 10,
            model: 45,
            action: 51,
            max_life: 30,
            life: 30,
            actual_speed: 16,
            minimum_speed,
            subspell_index,
            dword_0x10: 2,
            yaw: 0,
            font_type_index: 54,
            // AddTerrainModification starts byte 0x38 at 33.  BLDGPRM[21]
            // has bit 3 set, so sub_49A30 skips the branch that would OR 2.
            source_byte_38: 33,
            // The control byte initialized to 9 is not modified by
            // sub_49A30; it is distinct from source byte 0x38.
            control_flags: 9,
            mana,
            max_mana: 0,
            particle_index: 177,
            particle_byte_0x5d: 1,
            view_pitch: 2560,
            view_yaw: 0,
            view_fov: 256,
            view_roll: 2560,
            axis_x: 0,
            axis_z: 0,
            byte_0x46: 21,
            parent_entity_index: 0,
            source_byte_3e: source_entity_index as u8,
            position: source_position,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model45.push(source);
        Some(source)
    }

    /// Allocate an authored Day Bee from the class-5/model-2 constructor
    /// `sub_4B590`.  The caller has already proved the authored kind/subtype
    /// and producer group; this method owns the real source arena slot, RNG,
    /// map link, and constructor defaults.
    fn allocate_authored_bee(
        &mut self,
        authored_index: u16,
        position: Position,
        stage_route: Option<Level2BeeStageRoute>,
    ) -> Option<Level2AuthoredBeeSource> {
        let source_entity_index = self.allocate_slot()?;
        let initial_rng = self.new_event_rng(source_entity_index);
        let mut position = position;
        position.z = self.terrain.ground_height(position);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        // Pinned `sub_4B590`: NewEvent supplies the initial rand word, the
        // constructor advances it once, and the same low 11 bits seed yaw,
        // roll, and pitch.  The descriptor is `str_D7BD6[73]`.
        let entity_rng = level2_next_rng16(initial_rng);
        let angle = (entity_rng & 0x07ff) as i16 - 1;
        let source_byte_3e = self.bee_model2_constructor_phase;
        self.bee_model2_constructor_phase = source_byte_3e.wrapping_add(1);
        let action_window = 30;
        let source_byte_39 = action_window
            - i16::from(source_byte_3e % u8::try_from(action_window).expect("positive window"))
            + 4;
        let (action, stage_var1, stage_var2, stage_target) =
            stage_route.map_or((17, 0, 0, None), |route| {
                (
                    8 * 2 + 7,
                    route.stage_var1,
                    route.stage_var2,
                    Some(route.stage_target),
                )
            });
        let source = Level2AuthoredBeeSource {
            source_entity_index,
            entity_id: source_entity_index,
            entity_rng,
            authored_index,
            source_class: 5,
            model: 2,
            action,
            minimum_speed: 64,
            maximum_speed: 30,
            actual_speed: 32,
            // NewEvent_4A050 initializes the control dword to 8. The
            // movement helper only consumes the later byte-1 bit 8 (0x800),
            // so the constructor's low-byte marker remains intact here.
            control_flags: 8,
            mana: 1500,
            max_life: 3000,
            life: 3000,
            pending_damage: 0,
            pending_attacker_entity_index: 0,
            damage_attacker_entity_index: 0,
            subspell_index: 200,
            dword_0x10: i32::from(source_entity_index % 100),
            target_entity_index: None,
            yaw: angle,
            pitch: angle,
            roll: angle,
            source_byte_3e,
            source_byte_39,
            vertical_step: -32,
            action_window,
            stage_var1,
            stage_var2,
            stage_target,
            last_sound_id: 0,
            position,
            particle_index: 3,
            shift_rot_x: 128,
            shift_rot_y: 128,
            phase: 0,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_bees.push(source);
        Some(source)
    }

    /// Bind the source `word_0x96` target mailbox for the bounded Bee action
    /// tests and the future objective/AI owner.  The target is intentionally
    /// explicit; no player or objective is inferred here.
    pub fn set_authored_bee_target(&mut self, source_slot: u16, target: Option<u16>) -> bool {
        let Some(source) = self
            .authored_bees
            .iter_mut()
            .find(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        source.target_entity_index = target;
        true
    }

    #[cfg(test)]
    fn replace_authored_bee_for_test(&mut self, updated: Level2AuthoredBeeSource) -> bool {
        let Some(source) = self
            .authored_bees
            .iter_mut()
            .find(|source| source.source_entity_index == updated.source_entity_index)
        else {
            return false;
        };
        *source = updated;
        true
    }

    /// Run the source `sub_1BF90` class-3 target scan for an action-17 Bee.
    ///
    /// The caller supplies the live `dword_38519` class-3 list in source
    /// order. This method owns the action-17 `sub_1B8C0` movement prelude,
    /// model-2 descriptor cadence, and two wander RNG draws; the normal
    /// action-17 callback remains responsible for the follow-up sound draw
    /// and action transition.
    pub fn acquire_authored_bee_target(
        &mut self,
        source_slot: u16,
        candidates: &[(u16, Position, u32)],
    ) -> Option<u16> {
        let Some(source_index) = self
            .authored_bees
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return None;
        };
        let mut source = self.authored_bees[source_index];
        if source.action != 17 {
            return None;
        }

        // `sub_1BF90(a1, 16)` always runs `sub_1B8C0` first. That movement
        // callback is intentionally before the cadence guard: even a
        // non-cadence action-17 turn changes position/yaw and only then
        // decides whether to consume the two wander draws and scan targets.
        move_level2_bee_source(&mut source, &self.terrain);
        if source.action_window <= 0 {
            self.source_rng.insert(source_slot, source.entity_rng);
            self.authored_bees[source_index] = source;
            return None;
        }
        if source.source_byte_3e
            % u8::try_from(source.action_window).expect("positive Bee action window")
            != 0
        {
            self.source_rng.insert(source_slot, source.entity_rng);
            self.authored_bees[source_index] = source;
            return None;
        }

        // `sub_1BF90` wanders before scanning `dword_38519`: both LCG draws
        // are observable even when no target survives the source filters.
        let first = level2_next_rng16(source.entity_rng);
        let second = level2_next_rng16(first);
        let direction = 2 * (u32::from(first % 0x9d) / 79);
        let roll_delta = i32::from((second & 0xff) as u8) + 85;
        let roll_sign = i32::try_from(direction).expect("Bee roll direction fits") - 1;
        source.roll = (i32::from(source.roll) + roll_delta * roll_sign) as i16 & 0x07ff;
        source.entity_rng = second;

        // The source performs the two wander draws before consulting
        // byte-0x39; a zero window suppresses only the class-3 scan.
        if source.source_byte_39 == 0 {
            self.source_rng.insert(source_slot, source.entity_rng);
            self.authored_bees[source_index] = source;
            return None;
        }

        let selected = select_level2_bee_class3_target(&source, candidates);
        // `sub_1BF90` writes word_0x96 only when a new class-3 target wins.
        // On a gated or empty scan the stale relation remains in the source
        // row; action 17 itself does not consume that relation as a target.
        if let Some(target_index) = selected {
            source.target_entity_index = Some(target_index);
        }
        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_bees[source_index] = source;
        selected
    }

    /// Allocate one authored class-11 model-0/model-1/model-12 switch producer using
    /// the shared `NewEvent_4A050` source arena.  `stage_tag` is the source
    /// entity id consumed by the next `sub_4A1E0` group scan.
    pub fn allocate_authored_switch_producer(
        &mut self,
        authored_index: u16,
        model: u8,
        stage_tag: u16,
        word_10: i16,
        position: Position,
    ) -> Option<Level2AuthoredSwitchProducerSource> {
        if !matches!(model, 0 | 1 | 12) || stage_tag == 0 {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let mut position = position;
        position.z = self.terrain.ground_height(position);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2AuthoredSwitchProducerSource {
            source_entity_index,
            entity_id: stage_tag,
            entity_rng,
            authored_index,
            source_class: 11,
            model,
            action: model,
            stage_tag,
            word_10,
            dword_0x10: 0,
            position,
            // NewEvent_4A050 initializes byte_0x3E from the allocated source
            // entity slot. AddSwitchXX_50A90 and sub_4A310 preserve it for
            // class-11 producers, so the typed phase carries that runtime
            // seed rather than an authored-row or zero-based substitute.
            phase: source_entity_index as u8,
            cooldown: 0,
            fired: false,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_switch_producers.push(source);
        Some(source)
    }

    /// Allocate one authored class-2/model-0/action-0 tree using the exact
    /// `AddTree_4AC40` constructor sequence.  The authored position is
    /// already terrain-grounded by `sub_4A310`; only the constructor's two
    /// signed 0..63-minus-32 offsets alter x/y before the map link is made.
    pub fn allocate_authored_tree(
        &mut self,
        authored_index: u16,
        position: Position,
    ) -> Option<Level2AuthoredTreeSource> {
        let source_entity_index = self.allocate_slot()?;
        let initial_rng = self.new_event_rng(source_entity_index);
        let life_rng = level2_next_rng16(initial_rng);
        let x_rng = level2_next_rng16(life_rng);
        let y_rng = level2_next_rng16(x_rng);
        let particle_rng = level2_next_rng16(y_rng);
        let x_offset = i16::try_from(x_rng & 0x3f)
            .expect("tree x offset draw fits i16")
            .wrapping_sub(32);
        let y_offset = i16::try_from(y_rng & 0x3f)
            .expect("tree y offset draw fits i16")
            .wrapping_sub(32);
        let particle_index = if particle_rng & 1 != 0 { 84 } else { 83 };
        let position = Position {
            x: position.x.wrapping_add(x_offset as u16),
            y: position.y.wrapping_add(y_offset as u16),
            z: position.z,
        };
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2AuthoredTreeSource {
            source_entity_index,
            entity_id: source_entity_index,
            entity_rng: particle_rng,
            authored_index,
            source_class: 2,
            model: 0,
            action: 0,
            dword_0x10: i32::from(source_entity_index % 11),
            source_byte_38: 1,
            // AddTree writes the random 2,500..7,499 life first, then
            // CopyMaxLifeToLife restores NewEvent's default max/life of 300.
            max_life: 300,
            life: 300,
            // NewEvent starts at 8 and AddEventToMap sets the map bit 4.
            // The callback's byte-2 bit is represented by the same source
            // dword and is added only when the row is dispatched.
            control_flags: 0x0000_000c,
            owner_entity_index: 0,
            pending_damage: 0,
            particle_index,
            // Particle rows 83 and 84 both have rotSpeed 800 and speed 0;
            // SetHalfSpeedEntity therefore writes yaw/fov 400 and pitch/roll
            // zero after the fourth constructor draw.
            yaw: 400,
            pitch: 0,
            roll: 0,
            fov: 400,
            position,
            phase: source_entity_index as u8,
            last_sphere_source_entity_index: 0,
            last_sphere_word_0x2c: 0,
            last_sphere_position_z: 0,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, particle_rng);
        self.authored_trees.push(source);
        Some(source)
    }

    /// Allocate an authored class-11/model-32 objective switch through the
    /// pinned `AddSwitch32_50B80` constructor and `sub_4A310` field writes.
    pub fn allocate_authored_model32(
        &mut self,
        authored_index: u16,
        stage_tag: u16,
        objective_stage_index: u8,
        position: Position,
    ) -> Option<Level2AuthoredModel32Source> {
        if stage_tag == 0 || objective_stage_index >= 8 {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let mut position = position;
        position.z = self.terrain.ground_height(position);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2AuthoredModel32Source {
            source_entity_index,
            entity_id: stage_tag,
            entity_rng,
            authored_index,
            source_class: 11,
            model: 32,
            action: 32,
            stage_tag,
            objective_stage_index,
            // The model-32 branch skips SetEntityShiftRot, so NewEvent's
            // zeroed word_0x10 remains untouched by sub_4A310.
            word_10: 0,
            // NewEvent_4A050 defaults maxLife/life to 300. AddSwitch32 does
            // not alter those words; sub_4A310 copies maxLife to life.
            max_life: 300,
            life: 300,
            // AddSwitchXX clears NewEvent's byte-zero 8 and sets bit 0,
            // yielding the exact little-endian control dword 1.
            control_flags: 1,
            source_byte_3e: source_entity_index as u8,
            position,
            phase: 0,
            fired: false,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model32.push(source);
        Some(source)
    }

    /// Consume one model-32 source's objective callback. The caller owns the
    /// objective predicate and downstream group scan; this method only
    /// applies the source's one-shot drawing/fired transition.
    fn complete_authored_model32(
        &mut self,
        source_slot: u16,
    ) -> Option<Level2AuthoredModel32Source> {
        let source = self.authored_model32.iter_mut().find(|source| {
            source.active && source.source_entity_index == source_slot && !source.fired
        })?;
        source.fired = true;
        source.drawing_enabled = false;
        Some(*source)
    }

    /// Allocate the authored class-10/model-59 checkpoint row from the
    /// generic `sub_4A310` path. The pinned `ArriveCheckpoint_4EB50`
    /// constructor performs two entity-local RNG draws: max-life in
    /// `800..899`, then actual speed in `0..16`; it writes action 64/model 59
    /// and preserves the direct authored position.
    pub fn allocate_authored_model59(
        &mut self,
        authored_index: u16,
        position: Position,
    ) -> Option<Level2AuthoredModel59Source> {
        // `ArriveCheckpoint_4EB50` checks the shared NewEvent free count
        // before it consumes either constructor RNG draw.
        if self.free_slots.len() < 32 {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let initial_rng = self.new_event_rng(source_entity_index);
        let life_rng = level2_next_rng16(initial_rng);
        let max_life = i16::try_from(life_rng % 0x64).expect("checkpoint life fits i16") + 800;
        let speed_rng = level2_next_rng16(life_rng);
        let actual_speed = i16::try_from(speed_rng % 0x11).expect("checkpoint speed fits i16");
        let source = Level2AuthoredModel59Source {
            source_entity_index,
            entity_id: source_entity_index,
            entity_rng: speed_rng,
            authored_index,
            source_class: 10,
            model: 59,
            action: 64,
            max_life,
            life: max_life,
            actual_speed,
            // NewEvent starts with dword 8; ArriveCheckpoint applies
            // `(byte0 & 0xf6) | 1`, yielding the little-endian control dword 1.
            control_flags: 1,
            source_byte_3e: source_entity_index as u8,
            position,
            phase: source_entity_index as u8,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, speed_rng);
        self.authored_model59.push(source);
        Some(source)
    }

    fn allocate_model59_smoke_child(&mut self, position: Position) -> Option<u16> {
        // `SetParticleSmoke3B_4E9E0` advances the process-global stream before
        // `SetSmoke4_4EAA0` attempts NewEvent. The draw survives failure.
        self.global_rng = level2_next_rng32(self.global_rng);
        let source_entity_index = self.allocate_slot()?;
        let initial_rng = self.new_event_rng(source_entity_index);
        let entity_rng = level2_next_rng16(initial_rng);
        let actual_speed =
            i16::try_from(entity_rng % 0x35 + 51).expect("model-59 smoke child speed fits");
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, Some(entity_rng));
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.model59_smoke_children.push(Level2Model59SmokeChild {
            source_entity_index,
            entity_rng,
            source_class: 10,
            model: 13,
            action: 13,
            max_life: 32,
            life: 32,
            max_speed: 30,
            actual_speed,
            dword_0x10: 0,
            particle_index: 67,
            yaw: 0,
            control_flags: 0x0002_0000,
            position,
            phase: source_entity_index as u8,
            drawing_enabled: true,
            active: true,
        });
        Some(source_entity_index)
    }

    /// Advance the authored action-64 checkpoint callback. The parent emits
    /// one model-13/action-13 smoke child per successful callback allocation;
    /// the child then follows the source `sub_32160` motion/expiry path.
    pub fn step_authored_model59_slot(&mut self, source_slot: u16) -> bool {
        let Some(source_index) = self
            .authored_model59
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        let life_before = self.authored_model59[source_index].life;
        let mut source = self.authored_model59[source_index];
        source.life = source.life.wrapping_sub(1);
        if life_before < 0 {
            source.drawing_enabled = false;
            source.active = true;
            source.source_byte_3e = source.source_byte_3e.wrapping_add(1);
            source.phase = source.phase.wrapping_add(1);
            self.source_rng.insert(source_slot, source.entity_rng);
            self.authored_model59[source_index] = source;
            return true;
        }

        let mut smoke_position = source.position;
        source.entity_rng = level2_next_rng16(source.entity_rng);
        smoke_position.x = smoke_position.x.wrapping_add(source.entity_rng % 0xA0);
        source.entity_rng = level2_next_rng16(source.entity_rng);
        smoke_position.z = smoke_position
            .z
            .wrapping_add(i16::try_from(source.entity_rng % 0xA0).expect("smoke z offset"));
        if let Some(child_slot) = self.allocate_model59_smoke_child(smoke_position) {
            // `AddParticleSmoke0A_3D` consumes the parent's third draw only
            // after SetSmoke4 returns a live child.
            source.entity_rng = level2_next_rng16(source.entity_rng);
            if let Some(child) = self
                .model59_smoke_children
                .iter_mut()
                .find(|child| child.source_entity_index == child_slot)
            {
                child.actual_speed = child
                    .actual_speed
                    .wrapping_add(source.actual_speed)
                    .wrapping_add(
                        i16::try_from(source.entity_rng % 0x4D)
                            .expect("model-59 smoke parent speed draw fits"),
                    );
            }
        }
        source.source_byte_3e = source.source_byte_3e.wrapping_add(1);
        source.phase = source.phase.wrapping_add(1);
        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_model59[source_index] = source;
        true
    }

    /// Advance one model-59 smoke child through the pinned `sub_32160`
    /// callback. Its particle ramp differs from the rooted model-13 family:
    /// it rises toward selector 74 and falls back toward selector 67.
    pub fn step_model59_smoke_child_slot(&mut self, source_slot: u16) -> bool {
        let Some(child_index) = self
            .model59_smoke_children
            .iter()
            .position(|child| child.active && child.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut child = self.model59_smoke_children[child_index];
        let life_before = child.life;
        child.life = child.life.wrapping_sub(1);
        if life_before < 0 {
            child.drawing_enabled = false;
            child.active = true;
            child.phase = child.phase.wrapping_add(1);
            self.model59_smoke_children[child_index] = child;
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: true,
                    x: child.position.x,
                    y: child.position.y,
                })
                .expect("Level 2 model-59 smoke child fits the map chain");
            return true;
        }
        child.actual_speed = (child.actual_speed - 4).clamp(64, 128);
        let mut predicted = child.position;
        predicted.z = predicted
            .z
            .wrapping_add(child.actual_speed)
            .max(self.terrain.ground_height(predicted));
        child.dword_0x10 += 1;
        if child.dword_0x10 < 16 {
            crate::move_entity(&mut predicted, child.yaw, 0, child.max_speed);
            child.max_speed = (child.max_speed - 52).clamp(30, 1024);
            if child.dword_0x10 & 1 == 0 && child.particle_index < 74 {
                child.particle_index += 1;
            }
        }
        if child.life < 6 && child.particle_index > 67 {
            child.particle_index -= 1;
        }
        child.position = predicted;
        child.phase = child.phase.wrapping_add(1);
        self.model59_smoke_children[child_index] = child;
        self.source_rng.insert(source_slot, child.entity_rng);
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: child.position.x,
                y: child.position.y,
            })
            .is_ok()
    }

    /// Allocate the authored class-10/model-53/action-58 row from remc2
    /// `sub_504B0`.
    /// The constructor intentionally does not add the row to the render map;
    /// it writes its authored position directly, sets action 58/model 53,
    /// life 6, subspell 3000, and clears NewEvent's default control bit 0x08.
    /// The remaining NewEvent defaults are retained because `sub_38B90`
    /// consumes the source RNG immediately on its live callback path.
    pub fn allocate_authored_model53(
        &mut self,
        authored_index: u16,
        position: Position,
    ) -> Option<Level2AuthoredModel53Source> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let source = Level2AuthoredModel53Source {
            source_entity_index,
            entity_id: source_entity_index,
            entity_rng,
            authored_index,
            source_class: 10,
            model: 53,
            action: 58,
            max_life: 6,
            life: 6,
            subspell_index: 3000,
            actual_speed: 16,
            dword_0x10: 0,
            control_flags: 0,
            shift_rot_x: 0,
            shift_rot_y: 0,
            source_byte_39: -6,
            byte_0x43: 10,
            byte_0x44: 0,
            byte_0x46: 0,
            word_0x2c: 0,
            sprite_index: 0,
            last_sound_id: 0,
            water_spawn_requested: false,
            position,
            phase: source_entity_index as u8,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model53.push(source);
        Some(source)
    }

    /// Run remc2 class-10/action-58 `sub_38B90` for one live source turn.
    ///
    /// `sub_504B0` is the only pinned constructor for this action and creates
    /// model 53. The callback decrements life before its negative terminal
    /// return, latches sound 30 and shifts, then consumes RNG-L2-0263 before
    /// the three accepted SEARCH.DAT shape-0 cells (0264..0267). Each accepted
    /// child is the existing `NewAdd0A00_4E320` model-0 arena row with the
    /// source's copied identity and action-58 control/shift overrides.
    pub fn step_authored_model53_slot(&mut self, source_slot: u16) -> bool {
        let Some(source_index) = self
            .authored_model53
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };

        let mut source = self.authored_model53[source_index];
        let life_before = source.life;
        source.life = source.life.wrapping_sub(1);
        if life_before >= 0 {
            if source.control_flags & 0x02 == 0 {
                source.control_flags |= 0x0001_0002;
                source.last_sound_id = 30;
            }
            let signed_term = 768_i32 * source.dword_0x10;
            let sign = signed_term.signum();
            source.shift_rot_x = i16::try_from((signed_term - sign * 5) >> 2)
                .expect("Level 2 action-58 shift rotation fits i16");
            source.shift_rot_y = 512;

            // `sub_10C80(a2 = 0)` is a no-RNG target-mailbox scan. Its
            // cross-entity record writes remain outside this economy-owned
            // carrier; the source transition and RNG-producing child path are
            // kept exact here.
            source.entity_rng = level2_next_rng16(source.entity_rng);
            for (offset_x, offset_y) in [(0_i32, 0_i32), (1, 0), (0, 1)] {
                source.entity_rng = level2_next_rng16(source.entity_rng);
                let child_x = i32::from(source.position.x) - 96
                    + 160 * offset_x
                    + i32::from(source.entity_rng % 0x81)
                    - 64;
                source.entity_rng = level2_next_rng16(source.entity_rng);
                let child_y = i32::from(source.position.y) - 96
                    + 160 * offset_y
                    + i32::from(source.entity_rng % 0x81)
                    - 64;
                let child_position = Position {
                    x: child_x as u16,
                    y: child_y as u16,
                    z: source.position.z,
                };
                let _ = self.allocate_model0_child(
                    source_slot,
                    source.entity_id,
                    0,
                    child_position,
                    0x0003_0080,
                    512,
                    512,
                );
            }
            source.dword_0x10 = (source.dword_0x10 + 2).rem_euclid(7);
        }
        // ApplyEvents advances byte 0x3E even when sub_38B90 returns its
        // terminal positive result, so the typed phase follows every visit.
        source.phase = source.phase.wrapping_add(1);
        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_model53[source_index] = source;
        true
    }

    /// Materialize remc2's `NewAdd0A05_4E570` consumer for the model-53 water
    /// branch. The constructor is class 10/model 5/action 5, max/life 8,
    /// subspell 0, control dword 0x20000, grounded, mapped, and sprite 244.
    pub fn allocate_authored_model5_water_splash(
        &mut self,
        position: Position,
    ) -> Option<Level2AuthoredModel5WaterSplashSource> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let mut grounded_position = position;
        grounded_position.z = self.terrain.ground_height(position);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: grounded_position.x,
                y: grounded_position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2AuthoredModel5WaterSplashSource {
            source_entity_index,
            entity_id: source_entity_index,
            entity_rng,
            source_class: 10,
            model: 5,
            action: 5,
            max_life: 8,
            life: 8,
            subspell_index: 0,
            dword_0x10: 0,
            control_flags: 0x20_000,
            sprite_index: 244,
            last_sound_id: 0,
            position: grounded_position,
            phase: source_entity_index as u8,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model5_water_splashes.push(source);
        Some(source)
    }

    /// Run the bounded action-5 lifecycle from `AddAsh0A_05_318B0`: consume
    /// the eight-life splash window, emit source sound 27 once through the
    /// typed sidecar, then disable drawing and retire the row on the following
    /// callback. The `sub_585A0` animation increment is a no-op for this
    /// constructor's zero animation limit and is therefore represented by the
    /// unchanged sprite. `reclaim_inactive_slots` mirrors the source's same-
    /// pass `ApplyEvents` cleanup after the draw-disable bit is set.
    pub fn step_authored_model5_water_splash_slot(&mut self, source_slot: u16) -> bool {
        let Some(source) = self
            .authored_model5_water_splashes
            .iter_mut()
            .find(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        if source.life >= 0 {
            source.life -= 1;
            if source.control_flags & 0x2 == 0 {
                source.control_flags |= 0x2;
                source.last_sound_id = 27;
            }
        } else {
            source.drawing_enabled = false;
            source.active = false;
        }
        source.phase = source.phase.wrapping_add(1);
        self.source_rng.insert(source_slot, source.entity_rng);
        true
    }

    /// Allocate the authored class-10/subtype-39 consumer from the generic
    /// `sub_4A310` path.  The pinned subtype table selects
    /// `CreateManaSphere512_50080`, whose constructor writes a model-39,
    /// action-41 sphere with a fixed 512 mana payload.  These rows are
    /// dormant authored records, so their source owner/player links remain
    /// zero until a later source-owned claim/possession callback changes
    /// them; no relation is inferred from the activating switch.
    pub fn allocate_authored_model39(
        &mut self,
        _authored_index: u16,
        trigger_source_entity_index: u16,
        position: Position,
    ) -> Option<ManaSphere> {
        const AUTHORED_MANA: i32 = 512;
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let sphere = ManaSphere {
            id: u32::from(source_entity_index),
            source_entity_index,
            entity_rng,
            position,
            velocity_x: 0,
            velocity_y: 0,
            // CreateManaSphere_500C0 seeds word_0x2C with 128.  The first
            // ordinary TransformArcherToMana callback therefore rises before
            // gravity can reach the ground; preserving that seed prevents a
            // same-pass source cursor from merging freshly materialized
            // authored spheres before their later source rows are visited.
            vertical_velocity: 128,
            yaw: 0,
            mana: AUTHORED_MANA,
            owner_entity_index: 0,
            player_entity_index: 0,
            incoming_owner_entity_index: 0,
            incoming_possession_payload: 0,
            attraction_source_entity_index: 0,
            attraction_step: 0,
            attraction_distance_high: 0,
            assigned_carrier_id: 0,
            assigned_defender_slot: 0,
            control_flags: 0,
            // NewEvent seeds byte 0x3E from the allocated arena row.  The
            // typed phase is the corresponding source-visible byte.
            phase: source_entity_index as u8,
            active: true,
            pending_delete: false,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.spheres.push(sphere);
        self.refresh_owner_mana_rows();
        self.events.push(Level2EconomyEvent::ManaSphereSpawned {
            sphere_id: sphere.id,
            source_entity_index: trigger_source_entity_index,
            mana: AUTHORED_MANA,
        });
        Some(sphere)
    }

    /// Advance one live authored switch callback.  The caller supplies the
    /// source `CompareAxisWithShift` result; this method owns the source
    /// phase/cooldown writes and reports a fired target group.
    pub fn step_authored_switch_producer_slot(
        &mut self,
        source_slot: u16,
        overlap: bool,
    ) -> Option<u16> {
        let source = self
            .authored_switch_producers
            .iter_mut()
            .find(|source| source.active && source.source_entity_index == source_slot)?;
        let target_group = if source.cooldown == 0 && source.phase & 7 == 0 && overlap {
            source.cooldown = 10;
            source.fired = true;
            // `AddSwitch0B_00_6F030`/`CheckpointArrived_6F070` hide
            // model-0 sources after the stage-tag group scan succeeds.
            // Model-1's `sub_6F0B0` reloads its cooldown but leaves the
            // source visible. Keep the arena row active for source-order /
            // replay identity in both cases.
            if source.model == 0 {
                source.drawing_enabled = false;
            }
            Some(source.stage_tag)
        } else {
            None
        };
        if source.cooldown != 0 && target_group.is_none() && !overlap {
            source.cooldown = source.cooldown.saturating_sub(1);
        }
        source.phase = source.phase.wrapping_add(1);
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .ok();
        target_group
    }

    /// Bounded projection of the generic `sub_1BD90`/`sub_1BF90` awareness
    /// search used by actions 16 and 17.  The pinned descriptor permits an
    /// XY search radius of `word_0x1C_28 == 0x1400`; a mailbox pointing at an
    /// active entity outside that band is not an actionable target for these
    /// transitions.  This keeps the explicit target mailbox from becoming a
    /// synthetic "any target means attack" shortcut.
    fn bee_target_in_awareness_band(source: &Level2AuthoredBeeSource, target: Position) -> bool {
        let dx = i32::from(target.x) - i32::from(source.position.x);
        let dy = i32::from(target.y) - i32::from(source.position.y);
        let distance_squared = i64::from(dx) * i64::from(dx) + i64::from(dy) * i64::from(dy);
        distance_squared <= 5_120_i64 * 5_120_i64
    }

    /// Run the `byte_0x3E & 3 == 0` steering tail in `sub_1C310`. The source
    /// first points its roll at the live target, then lets the first nearby
    /// same-model peer override that roll. Peer bounds use the entity's
    /// source pitch field exactly as the decompilation does.
    fn steer_authored_bee_source(
        source: &mut Level2AuthoredBeeSource,
        target: Position,
        peers: &[(u16, Position)],
    ) {
        if source.source_byte_3e & 3 != 0 {
            return;
        }
        source.roll = i16::try_from(heading_for_positions(source.position, target))
            .expect("Bee target heading fits i16");
        let peer_bound = i32::from(source.pitch);
        if peer_bound <= 0 {
            return;
        }
        for &(peer_id, peer_position) in peers {
            if peer_id == source.entity_id {
                continue;
            }
            let dx = (i32::from(source.position.x) - i32::from(peer_position.x)).abs();
            let dy = (i32::from(source.position.y) - i32::from(peer_position.y)).abs();
            if dx < peer_bound && dy < peer_bound {
                source.roll = i16::try_from(heading_for_positions(peer_position, source.position))
                    .expect("Bee peer heading fits i16");
                break;
            }
        }
    }

    /// Advance one authored Bee through the live action 16..19 band.  The
    /// generic creature helpers are represented only for the fields this
    /// family actually consumes: the action-18 latch, target-Z tracking,
    /// strict target contact/recoil, and source RNG/phase ordering. The live
    /// outer dispatcher runs action-17's movement/acquisition prelude through
    /// `acquire_authored_bee_target` before calling this state transition.
    pub fn step_authored_bee_slot(
        &mut self,
        source_slot: u16,
        target_position: Option<Position>,
    ) -> bool {
        self.step_authored_bee_slot_with_target(
            source_slot,
            target_position.map(|position| (position, 0, true)),
        )
    }

    fn step_authored_bee_slot_with_target(
        &mut self,
        source_slot: u16,
        target: Option<(Position, i16, bool)>,
    ) -> bool {
        self.step_authored_bee_slot_with_target_and_peers(source_slot, target, &[], &[])
    }

    fn step_authored_bee_slot_with_target_and_peers(
        &mut self,
        source_slot: u16,
        target: Option<(Position, i16, bool)>,
        peers: &[(u16, Position)],
        class3_candidates: &[(u16, Position, u32)],
    ) -> bool {
        let Some(source_index) = self
            .authored_bees
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut source = self.authored_bees[source_index];
        // `sub_1BF90` consumes the class-5/model-2 action-17 damage mailbox
        // before its ordinary target/steering branch.  The attacker word is
        // copied exactly once and the mailbox is cleared in the same callback;
        // later Fireball callbacks therefore cannot duplicate the hit.
        if source.action == 17 && source.pending_attacker_entity_index != 0 {
            source.life = source.life.wrapping_sub(source.pending_damage as i16);
            source.damage_attacker_entity_index = source.pending_attacker_entity_index;
            source.pending_damage = 0;
            source.pending_attacker_entity_index = 0;
        }
        match source.action {
            16 => {
                if target
                    .map(|(position, _, valid)| {
                        valid && Self::bee_target_in_awareness_band(&source, position)
                    })
                    .unwrap_or(false)
                {
                    source.action = 18;
                    source.dword_0x10 = 1;
                }
            }
            17 => {
                if target
                    .map(|(position, _, valid)| {
                        valid && Self::bee_target_in_awareness_band(&source, position)
                    })
                    .unwrap_or(false)
                {
                    source.action = 18;
                    source.entity_rng = level2_next_rng16(source.entity_rng);
                    source.last_sound_id = (source.entity_rng & 1) + 12;
                    source.dword_0x10 = 1;
                }
            }
            18 => {
                let countdown_before = source.dword_0x10;
                if countdown_before != 0 {
                    source.dword_0x10 -= 1;
                    if countdown_before == 1 {
                        source.actual_speed = 5 * source.minimum_speed / 2;
                    }
                }
                if let Some((target, target_height_offset, target_valid)) = target {
                    let desired_z = target_height_offset.wrapping_add(target.z);
                    let delta = source.position.z.wrapping_sub(desired_z);
                    let direction: i16 = if delta == 0 {
                        0
                    } else if delta < 0 {
                        -1
                    } else {
                        1
                    };
                    source.position.z = source
                        .position
                        .z
                        .wrapping_add(direction.wrapping_mul(source.vertical_step));
                    // `sub_1F6D0` updates target-Z before entering
                    // `sub_1C310`; that helper performs movement even when
                    // its subsequent target validity check rejects the
                    // relation, but skips steering/cadence in that case.
                    move_level2_bee_source(&mut source, &self.terrain);
                    if !target_valid {
                        source.action = 17;
                    } else {
                        Self::steer_authored_bee_source(&mut source, target, peers);
                        let on_action_window = source.action_window > 0
                            && source.source_byte_3e
                                % u8::try_from(source.action_window)
                                    .expect("positive Bee action window")
                                == 0;
                        let distance_squared =
                            level2_wrapping_source_distance_3d(source.position, target);
                        let out_of_range = source.action_window > 0
                            && on_action_window
                            && crate::legacy_sqrt(distance_squared)
                                >= u32::try_from(LEVEL2_BEE_AWARENESS_RADIUS)
                                    .expect("Bee awareness radius fits");
                        if out_of_range {
                            // `sub_1C310` drops a live relation only at the
                            // source cadence; the stale word_0x96 remains for
                            // the next action-17 scan to overwrite.
                            source.action = 17;
                        } else if source.action == 18
                            && on_action_window
                            && distance_squared
                                < u32::from(LEVEL2_BEE_CONTACT_RADIUS)
                                    .saturating_mul(u32::from(LEVEL2_BEE_CONTACT_RADIUS))
                        {
                            source.entity_rng = level2_next_rng16(source.entity_rng);
                            source.last_sound_id = (source.entity_rng & 1) + 12;
                            source.actual_speed = -source.maximum_speed;
                            source.dword_0x10 = i32::from(3 * source.action_window);
                        }
                    }
                } else {
                    source.action = 17;
                }
                if source.action != 18 {
                    source.actual_speed = source.minimum_speed;
                }
            }
            19 => {
                // `sub_1C560` only re-evaluates its linked-target relation on
                // the descriptor's 30-tick byte-3E window.  A valid mailbox
                // outside that window remains in action 19 until the next
                // helper visit rather than jumping directly to attack.
                if source.action_window > 0
                    && source.source_byte_3e
                        % u8::try_from(source.action_window).expect("positive action window")
                        == 0
                    && target
                        .map(|(position, _, valid)| {
                            valid && Self::bee_target_in_awareness_band(&source, position)
                        })
                        .unwrap_or(false)
                {
                    source.action = 18;
                    source.dword_0x10 = 1;
                }
            }
            20 => {
                // `sub_1F830` -> `PreKillEntity_1C890(a1, 16)` changes the
                // root and any linked children to action 21. Authored Day
                // Bees have no child chain in this bounded arena.
                source.action = 21;
            }
            21 => {
                // `sub_1F850` -> `KillEntity_1C930`: convert constructor
                // mana into one source-owned sphere, then disable drawing
                // and let the normal arena reclaim the source slot.
                let _ = self.spawn_mana_sphere_from_death(
                    source.source_entity_index,
                    source.position,
                    source.yaw,
                    source.entity_rng,
                    source.mana,
                    source.entity_id,
                );
                // The Bee has no suppression bit in its constructor state,
                // so `KillEntity_1C930` also allocates the ordered
                // class-10/model-1/action-1 death burst after the sphere.
                let _ = self.spawn_death_burst_from_death_with_id(
                    source.source_entity_index,
                    source.entity_id,
                    source.position,
                );
                source.action = 21;
                source.drawing_enabled = false;
                source.active = false;
                source.life = -1;
                source.target_entity_index = None;
            }
            23 => {
                // `sub_1F8A0 -> sub_1D5D0(16)` dispatches the bounded
                // authored L2-06 selector-2 callback before its own roll
                // tail. Other selectors remain deliberately fail-closed.
                if source.stage_var2 == 2 {
                    if source.life < 0 {
                        // Both selector-2 helpers select their terminal
                        // branch before `sub_1B8C0`. A helper which itself
                        // exhausts movement retries remains action 23 until
                        // the following callback reaches here.
                        source.action = 20;
                    } else if let Some(stage_target) = source.stage_target {
                        let outside_point_band = (i32::from(stage_target.x as i16)
                            - i32::from(source.position.x as i16))
                        .abs()
                            > 3_072
                            || (i32::from(stage_target.y as i16)
                                - i32::from(source.position.y as i16))
                            .abs()
                                > 3_072;
                        move_level2_bee_source(&mut source, &self.terrain);
                        if outside_point_band {
                            if source.source_byte_3e & 7 == 0 {
                                if source.control_flags & LEVEL2_BEE_OBSTRUCTION_BIT == 0 {
                                    source.roll = i16::try_from(heading_for_positions(
                                        source.position,
                                        stage_target,
                                    ))
                                    .expect("Bee StageVar heading fits i16");
                                    if source.source_byte_3e & 0x3f == 0 {
                                        let first = level2_next_rng16(source.entity_rng);
                                        let second = level2_next_rng16(first);
                                        let direction = 2 * (u32::from(first % 0x9d) / 79);
                                        let delta = (i32::from((second & 0xff) as u8) + 85)
                                            * (i32::try_from(direction)
                                                .expect("Bee roll direction fits")
                                                - 1);
                                        source.roll =
                                            ((i32::from(source.roll) + delta) as i16) & 0x07ff;
                                        source.entity_rng = second;
                                    }
                                }
                                let peer_bound = i32::from(source.pitch);
                                if peer_bound > 0 {
                                    for &(peer_id, peer_position) in peers {
                                        if peer_id == source.entity_id {
                                            continue;
                                        }
                                        let dx = (i32::from(source.position.x)
                                            - i32::from(peer_position.x))
                                        .abs();
                                        let dy = (i32::from(source.position.y)
                                            - i32::from(peer_position.y))
                                        .abs();
                                        if dx < peer_bound && dy < peer_bound {
                                            source.roll = i16::try_from(heading_for_positions(
                                                peer_position,
                                                source.position,
                                            ))
                                            .expect("Bee peer heading fits i16");
                                            break;
                                        }
                                    }
                                }
                            }
                        } else if source.source_byte_3e & 0x0f == 0
                            && source.control_flags & LEVEL2_BEE_OBSTRUCTION_BIT == 0
                        {
                            source.entity_rng = level2_next_rng16(source.entity_rng);
                            source.roll = ((i32::from(source.roll)
                                + i32::from(source.entity_rng % 0x71)
                                + 142) as i16)
                                & 0x07ff;
                        }

                        if source.action == 23
                            && source.stage_var2 == 2
                            && source.source_byte_39 != 0
                            && source.action_window > 0
                            && source.source_byte_3e
                                % u8::try_from(source.action_window)
                                    .expect("positive Bee action window")
                                == 0
                            && let Some(target_index) =
                                select_level2_bee_class3_target(&source, class3_candidates)
                        {
                            source.target_entity_index = Some(target_index);
                            source.stage_var2 = 10;
                            // Model 2's descriptor lacks the bit-8 route, so
                            // `sub_1E040(..., 16)` selects action 18.
                            source.action = 18;
                        }
                    }
                } else if source.stage_var2 == 10 {
                    // Existing selector-10 preservation route.
                    source.action = 18;
                }
                if source.source_byte_3e & 7 == 0 && (1..=9).contains(&source.stage_var2) {
                    source.entity_rng = level2_next_rng16(source.entity_rng);
                    let direction = 2 * (u32::from(source.entity_rng % 0x9d) / 79);
                    source.entity_rng = level2_next_rng16(source.entity_rng);
                    let delta = i32::from(source.entity_rng % 0x55)
                        * (i32::try_from(direction).expect("Bee roll direction fits") - 1);
                    source.roll = ((i32::from(source.roll) + delta) as i16) & 0x07ff;
                }
                if source.action == 18 {
                    // The source applies the action-18 latch after the
                    // selector callback and action-23 roll tail.
                    source.dword_0x10 = 1;
                }
            }
            22 => {
                // Deliberate missing callback slot: the source body is empty,
                // but the outer ApplyEvents bookkeeping below still advances
                // the per-entity cursor/phase once for the visited row.
            }
            _ => {}
        }
        // `ApplyEvents_498A0` advances byte 0x3E after every live callback;
        // action 19's generic helper uses that advancing cursor for its
        // descriptor-windowed reevaluation.
        source.source_byte_3e = source.source_byte_3e.wrapping_add(1);
        source.phase = source.phase.wrapping_add(1);
        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_bees[source_index] = source;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    /// Allocate the authored class-10/model-13 emitter initialized by the
    /// pinned `sub_4EF90` constructor.  The authored trigger is owned by the
    /// Level 2 activation route; this method owns source-pool allocation,
    /// map linkage, and the exact constructor payload.
    pub fn allocate_authored_model13(
        &mut self,
        authored_index: u16,
        position: Position,
    ) -> Option<Level2AuthoredModel13Source> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let mut position = position;
        position.z = self.terrain.ground_height(position);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        // sub_4EF90 clears bits 3 and 17, sets byte 2 bit 1, then sets the
        // drawing bit in byte 0 before copying max-life to life.
        let source = Level2AuthoredModel13Source {
            source_entity_index,
            entity_id: source_entity_index,
            entity_rng,
            authored_index,
            source_class: 10,
            model: 19,
            action: 19,
            subspell_index: 200,
            max_life: 240,
            life: 240,
            dword_0x10: 0,
            particle_index: 228,
            control_flags: 0x0002_0001,
            shift_rot_x: 512,
            shift_rot_y: 512,
            position,
            phase: source_entity_index as u8,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model13.push(source);
        Some(source)
    }

    /// Allocate the authored class-10/model-6 event initialized by
    /// `NewAdd0A06_4E5F0`. The constructor's fixed payload is source-backed:
    /// action/model 6, subspell 50, max/life 240, particle 228, clear low
    /// control byte, set byte-2 bit 1, shifts 272/1536, and height offset 0.
    pub fn allocate_authored_model6(
        &mut self,
        authored_index: u16,
        position: Position,
    ) -> Option<Level2AuthoredModel6Source> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let mut position = position;
        position.z = self.terrain.ground_height(position);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2AuthoredModel6Source {
            source_entity_index,
            entity_id: source_entity_index,
            entity_rng,
            authored_index,
            model: 6,
            action: 6,
            subspell_index: 50,
            max_life: 240,
            life: 240,
            height_offset: 0,
            dword_0x10: 0,
            particle_index: 228,
            control_flags: 0x0002_0000,
            shift_rot_x: 272,
            shift_rot_y: 1536,
            position,
            phase: source_entity_index as u8,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model6.push(source);
        Some(source)
    }

    /// Allocate the generic class-14/model-3 or model-5 authored event
    /// produced by `sub_4A310`.  The pinned constructor uses action 8 and
    /// particle 338 for model 3 (`sub_51570`), and action 10 and particle 280
    /// for model 5 (`sub_51610`).  No mission trigger is inferred here: the
    /// caller explicitly selects an authored row that has already fired.
    pub fn allocate_authored_class14(
        &mut self,
        authored_index: u16,
        model: u8,
        activation_group: i16,
        stage_tag: i16,
        position: Position,
    ) -> Option<Level2AuthoredClass14Source> {
        let (action, particle, control_flags, shift_rot_x, shift_rot_y) = match model {
            3 => (8, 338, 1, 0, 0),
            5 => (10, 280, 0, 768, 1280),
            _ => return None,
        };
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2AuthoredClass14Source {
            source_entity_index,
            entity_rng,
            authored_index,
            model,
            action,
            particle,
            control_flags,
            shift_rot_x,
            shift_rot_y,
            activation_group,
            stage_tag,
            position,
            phase: source_entity_index as u8,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_class14.push(source);
        if model == 3 {
            self.class14_model3_handle = source_entity_index;
        }
        Some(source)
    }

    fn allocate_house_resident(
        &mut self,
        parent_house_entity_index: u16,
        owner_entity_index: u16,
        request: HouseResidentRequest,
    ) -> Option<u16> {
        if !matches!(
            (request.model, request.action),
            (4, 33) | (12, 97) | (13, 105) | (14, 113)
        ) {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: request.position.x,
                y: request.position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2HouseResidentSource {
            source_entity_index,
            entity_rng,
            parent_house_entity_index,
            owner_entity_index,
            attacker_entity_index: request.attacker_entity_index,
            position: request.position,
            model: request.model,
            action: request.action,
            phase: source_entity_index as u8,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.source_owner
            .insert(source_entity_index, owner_entity_index);
        self.house_residents.push(source);
        Some(source_entity_index)
    }

    fn allocate_house_residents_atomic(
        &mut self,
        requests: &[(u16, u16, HouseResidentRequest)],
    ) -> Option<Vec<u16>> {
        let mut staged = self.clone();
        let mut source_slots = Vec::with_capacity(requests.len());
        for &(parent_house_entity_index, owner_entity_index, request) in requests {
            let source_entity_index = staged.allocate_house_resident(
                parent_house_entity_index,
                owner_entity_index,
                request,
            )?;
            source_slots.push(source_entity_index);
        }
        *self = staged;
        Some(source_slots)
    }

    fn step_house_resident_slot(&mut self, source_slot: u16) -> bool {
        let Some(source) = self
            .house_residents
            .iter_mut()
            .find(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        source.phase = source.phase.wrapping_add(1);
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    #[must_use]
    pub fn house_residents(&self) -> &[Level2HouseResidentSource] {
        &self.house_residents
    }

    fn set_house_owner_relation(&mut self, source_slot: u16, owner_entity_index: u16) {
        self.source_owner.insert(source_slot, owner_entity_index);
    }

    fn disable_house_source_slot(&mut self, source_slot: u16) -> Result<()> {
        self.source_owner.remove(&source_slot);
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: false,
                x: 0,
                y: 0,
            })
            .map_err(NativeWorldError::new)
    }

    /// Advance one model-1 root through `AddQuickfair0A_01_30F60`.
    ///
    /// The callback's transition order is life guard, first sound latch,
    /// SEARCH.DAT shape-0 walk, then the independent ApplyEvents phase
    /// increment. A negative pre-decrement return hides the root and consumes
    /// no Quickfair draw. The three accepted shape cells use the existing
    /// model-0 child arena, which is the pinned `NewAdd0A00_4E320` producer
    /// used by `IfSubtypeCallCreatingManaSphere_4A190(10, 0)`.
    pub fn step_authored_model1_slot(&mut self, source_slot: u16) -> bool {
        let Some(source_index) = self
            .authored_model1
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut source = self.authored_model1[source_index];
        let life_before = source.life;
        source.life = source.life.wrapping_sub(1);
        if life_before < 0 {
            source.drawing_enabled = false;
        } else {
            if source.control_flags & 0x02 == 0 {
                source.control_flags |= 0x02;
                source.last_sound_id = 3;
                self.prepare_sound(source_slot, source_slot, -1, 3);
            }
            for (offset_x, offset_y) in [(0_i32, 0_i32), (1, 0), (0, 1)] {
                source.entity_rng = level2_next_rng16(source.entity_rng);
                if source.entity_rng % 0x9d < 79 {
                    continue;
                }
                source.entity_rng = level2_next_rng16(source.entity_rng);
                let child_x = i32::from(source.position.x) - 96
                    + 192 * offset_x
                    + i32::from(source.entity_rng % 0x81)
                    - 64;
                source.entity_rng = level2_next_rng16(source.entity_rng);
                let child_y = i32::from(source.position.y) - 96
                    + 192 * offset_y
                    + i32::from(source.entity_rng % 0x81)
                    - 64;
                let child_position = Position {
                    x: child_x as u16,
                    y: child_y as u16,
                    z: source.position.z,
                };
                let _ = self.allocate_model0_child(
                    source_slot,
                    source_slot,
                    0,
                    child_position,
                    // NewAdd0A00 starts at 0x0002_0000; Quickfair's
                    // producer then applies the source's 0x80 child bit.
                    0x0002_0080,
                    128,
                    128,
                );
            }
        }
        source.phase = source.phase.wrapping_add(1);
        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_model1[source_index] = source;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    pub fn step_spell_pickup_slot(&mut self, source_slot: u16) -> bool {
        let Some(source) = self
            .spell_pickups
            .iter_mut()
            .find(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        source.phase = source.phase.wrapping_add(1);
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    pub fn step_authored_model45_slot(&mut self, source_slot: u16) -> bool {
        let Some(source_index) = self
            .authored_model45
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut source = self.authored_model45[source_index];

        // `sub_3A650` is class-10/model-45/action-51's callback.  The
        // constructor leaves `word_0x96_150` at zero for these authored rows,
        // so its source scan has no parent target; it only sets the hidden
        // drawing bit.  ApplyEvents immediately follows that callback with
        // sub_57F20, unlinking and returning the row to NewEvent's free pool.
        if source.action == 51 {
            source.drawing_enabled = false;
            source.active = false;
            source.source_byte_3e = source.source_byte_3e.wrapping_add(1);
            let map_ok = self
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: false,
                    x: source.position.x,
                    y: source.position.y,
                })
                .is_ok();
            self.authored_model45.remove(source_index);
            self.release_slot(source_slot, Some(source.entity_rng));
            return map_ok;
        }
        false
    }

    /// Advance one authored model-13 emitter through the source action-19
    /// callback `sub_32F40`.  The qualified Level 2 row uses SEARCH.DAT
    /// class zero (the four centre cells), preserves the source LCG draw
    /// order, and allocates subtype-14 children only on the source's odd-life
    /// branch.  Cross-entity presentation (`sub_10C80`) is intentionally
    /// outside this typed state seam.
    pub fn step_authored_model13_slot(&mut self, source_slot: u16) -> bool {
        let Some(source_index) = self
            .authored_model13
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        let source_before = self.authored_model13[source_index];
        let life_before = source_before.life;
        let mut source = source_before;
        source.life = source.life.wrapping_sub(1);
        if life_before < 0 {
            source.drawing_enabled = false;
            // `DisableEntityDrawing04_57F10` only sets the hidden-drawing
            // bit. The source row remains in the entity arena and continues
            // through the cursor; reclaim is a separate reset/pool concern.
            source.active = true;
            self.authored_model13[source_index] = source;
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: true,
                    x: source.position.x,
                    y: source.position.y,
                })
                .expect("Level 2 model-13 source slot fits the map chain");
            return true;
        }

        source.dword_0x10 = 0;
        // SEARCH.DAT class zero is the four-pixel centre block in the exact
        // row-major order produced by sub_10130 after sub_101C0 loading.
        for (offset_x, offset_y) in [(0_i32, 0_i32), (1, 0), (0, 1), (1, 1)] {
            source.entity_rng = level2_next_rng16(source.entity_rng);
            if source.entity_rng % 0x9d < 79 {
                continue;
            }
            source.entity_rng = level2_next_rng16(source.entity_rng);
            let child_x = i32::from(source.position.x) - 96
                + 192 * offset_x
                + i32::from(source.entity_rng % 0x81)
                - 64;
            source.entity_rng = level2_next_rng16(source.entity_rng);
            let child_y = i32::from(source.position.y) - 96
                + 192 * offset_y
                + i32::from(source.entity_rng % 0x81)
                - 64;
            if source.life & 1 != 0 {
                let mut yaw = ((source.life / 2) & 1) << 8;
                while yaw < 0x800 {
                    let position = Position {
                        x: child_x as u16,
                        y: child_y as u16,
                        z: source.position.z,
                    };
                    let _ = self.allocate_model13_spawned_child(
                        source_slot,
                        source.entity_id,
                        position,
                        yaw,
                    );
                    yaw = yaw.wrapping_add(0x200);
                }
            }
        }
        source.position.z = self.terrain.ground_height(source.position);
        self.source_rng.insert(source_slot, source.entity_rng);
        source.phase = source.phase.wrapping_add(1);
        self.authored_model13[source_index] = source;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    fn allocate_model13_spawned_child(
        &mut self,
        parent_entity_index: u16,
        parent_entity_id: u16,
        position: Position,
        yaw: i16,
    ) -> Option<u16> {
        let source_entity_index = self.allocate_slot()?;
        // SetParticleSmoke3C first advances the process-global RNG for the
        // child's life, then SetSmoke4 advances the newly allocated row's
        // own RNG for its actual speed.
        self.global_rng = level2_next_rng32(self.global_rng);
        let max_life =
            i16::try_from(self.global_rng as u16 % 0x21 + 28).expect("model-13 child life fits");
        let entity_rng = level2_next_rng16(self.new_event_rng(source_entity_index));
        let actual_speed =
            i16::try_from(entity_rng % 0x35 + 51).expect("model-13 child speed fits");
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, Some(entity_rng));
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.model13_spawned_children
            .push(Level2Model13SpawnedChild {
                source_entity_index,
                entity_rng,
                parent_entity_index,
                parent_entity_id,
                source_class: 10,
                model: 14,
                action: 14,
                max_life,
                life: max_life,
                max_speed: 30,
                actual_speed,
                dword_0x10: 0,
                particle_index: 9,
                entity_index: 9,
                subtype: 14,
                yaw,
                control_flags: 0x0002_0000,
                position,
                phase: source_entity_index as u8,
                drawing_enabled: true,
                active: true,
            });
        Some(source_entity_index)
    }

    /// Advance one model-14 child through the bounded source `sub_322A0`
    /// callback: signed life sentinel, upward movement, terrain clamp, and
    /// sprite index cadence. The callback has no cross-entity relations.
    pub fn step_model13_spawned_child_slot(&mut self, source_slot: u16) -> bool {
        let Some(child_index) = self
            .model13_spawned_children
            .iter()
            .position(|child| child.active && child.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut child = self.model13_spawned_children[child_index];
        let life_before = child.life;
        child.life = child.life.wrapping_sub(1);
        if life_before < 0 {
            child.drawing_enabled = false;
            // The pinned disable helper hides the row but does not unlink or
            // reclaim it from the source arena.
            child.active = true;
            self.model13_spawned_children[child_index] = child;
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: true,
                    x: child.position.x,
                    y: child.position.y,
                })
                .expect("Level 2 model-13 child slot fits the map chain");
            return true;
        }
        child.actual_speed = (child.actual_speed - 4).clamp(64, 128);
        let mut predicted = child.position;
        predicted.z = predicted
            .z
            .saturating_add(child.actual_speed)
            .max(self.terrain.ground_height(predicted));
        child.dword_0x10 += 1;
        if child.dword_0x10 < 16 {
            crate::move_entity(&mut predicted, child.yaw, 0, child.max_speed);
            child.max_speed = (child.max_speed - 52).clamp(30, 1024);
            if child.dword_0x10 & 1 == 0 && child.particle_index < 16 {
                child.particle_index += 1;
            }
        }
        if child.life < 6 && child.particle_index > 9 {
            child.particle_index -= 1;
        }
        child.position = predicted;
        child.phase = child.phase.wrapping_add(1);
        self.model13_spawned_children[child_index] = child;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: child.position.x,
                y: child.position.y,
            })
            .is_ok()
    }

    /// Advance one model-6 row through the pinned `sub_31760` callback.
    /// This preserves the signed life sentinel, the late-life countdown,
    /// source RNG cadence, class-9/model-14 child allocation, terrain-height
    /// projection, water disable, and map-link state. The source's
    /// `sub_5C870` player-distance mailbox and `sub_11400` attraction scan are
    /// intentionally separate cross-entity follow-up surfaces.
    pub fn step_authored_model6_slot(&mut self, source_slot: u16) -> bool {
        let Some(source_index) = self
            .authored_model6
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        let source_before = self.authored_model6[source_index];
        let life_before = source_before.life;
        let mut source = source_before;
        source.life = source.life.wrapping_sub(1);
        if life_before < 0 {
            source.active = false;
            self.authored_model6[source_index] = source;
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: false,
                    x: source.position.x,
                    y: source.position.y,
                })
                .expect("Level 2 model-6 source slot fits the map chain");
            return true;
        }

        // sub_5C870 updates the local player's minimum distance sidecar. The
        // typed economy does not own that nested player record, so preserve
        // the callback's state mutation here and leave the sidecar bridge to
        // the cross-entity Level 2 integration bead.
        if source.life < 12 {
            if source.dword_0x10 > 0 {
                source.dword_0x10 -= 1;
                source.particle_index = source.particle_index.wrapping_sub(1);
                let low_control = i8::from_le_bytes([source.control_flags as u8]);
                if low_control >= 0 {
                    source.entity_rng = level2_next_rng16(source.entity_rng);
                    if source.entity_rng % 7 == 0 {
                        let child = self.allocate_model6_spawned_child(
                            source_slot,
                            source.entity_id,
                            source.position,
                        );
                        // NewEvent failure consumes no parent state beyond the
                        // source RNG write, matching the pinned branch.
                        let _ = child;
                    }
                }
            }
        } else if source.dword_0x10 <= 6 {
            source.dword_0x10 += 1;
            source.particle_index = source.particle_index.wrapping_add(1);
        }
        source.position.z = source
            .height_offset
            .wrapping_add(self.terrain.ground_height(source.position));
        if self.terrain.material_mask(source.position) == crate::WATER_AMBIENCE_MASK {
            source.active = false;
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: false,
                    x: source.position.x,
                    y: source.position.y,
                })
                .expect("Level 2 model-6 water disable fits the map chain");
        }
        self.source_rng.insert(source_slot, source.entity_rng);
        source.phase = source.phase.wrapping_add(1);
        self.authored_model6[source_index] = source;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    fn allocate_model6_spawned_child(
        &mut self,
        parent_entity_index: u16,
        parent_entity_id: u16,
        position: Position,
    ) -> Option<u16> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.model6_spawned_children.push(Level2Model6SpawnedChild {
            source_entity_index,
            entity_rng,
            parent_entity_index,
            parent_entity_id,
            source_class: 9,
            model: 14,
            action: 15,
            max_life: 32,
            life: 15,
            actual_speed: 128,
            min_speed: 128,
            max_speed: 0,
            yaw: 0,
            height_offset: 0,
            dword_0x10: 100,
            // subtype-14's constructor sets particle 196; the parent then
            // copies that newly-created child's word-0x5A and adds two.
            particle_index: 198,
            control_flags: 0,
            position,
            active: true,
        });
        Some(source_entity_index)
    }

    /// Allocate the authored class-10/model-17 Meteor initialized by
    /// `AddMeteor_4ED70`.  The constructor is explicit in the pinned source:
    /// action/model 17, life/max-life 10, subspell 3000, and a map-linked
    /// position.  The authored parameter selectors are activation metadata,
    /// not constructor fields for this source row.
    pub fn allocate_authored_model17(
        &mut self,
        authored_index: u16,
        position: Position,
    ) -> Option<Level2AuthoredModel17Source> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let mut position = position;
        position.z = self.terrain.ground_height(position);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = Level2AuthoredModel17Source {
            source_entity_index,
            entity_id: source_entity_index,
            entity_rng,
            authored_index,
            source_class: 10,
            model: 17,
            action: 17,
            subspell_index: 3_000,
            max_life: 10,
            life: 10,
            dword_0x10: 0,
            control_flags: 0,
            shift_rot_x: 0,
            shift_rot_y: 0,
            yaw: 0,
            position,
            phase: source_entity_index as u8,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model17.push(source);
        Some(source)
    }

    /// Advance one authored Meteor through pinned `sub_32880`.  SEARCH.DAT
    /// shape 0 is the four-cell 2x2 kernel centered at (15,15); the source's
    /// iterator executes the first three cells because its return value is
    /// tested for `== 1` before each child allocation.  The source RNG is
    /// advanced once before the walk, then once for each child coordinate.
    pub fn step_authored_model17_slot(&mut self, source_slot: u16) -> bool {
        let Some(source_index) = self
            .authored_model17
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        let source_before = self.authored_model17[source_index];
        let mut source = source_before;
        source.life = source.life.wrapping_sub(1);
        if source.life < 0 {
            source.active = false;
            self.authored_model17[source_index] = source;
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: false,
                    x: source.position.x,
                    y: source.position.y,
                })
                .expect("Level 2 model-17 source slot fits the map chain");
            return true;
        }

        if source.control_flags & 0x02 == 0 {
            // `sub_32880` sets byte 0 bit 1 and byte 2 bit 0, then prepares
            // sound 30.  The audio ledger owns presentation dispatch; the
            // raw source write belongs in this state tuple.
            source.control_flags |= 0x0001_0002;
        }
        let signed_term = 768_i32 * source.dword_0x10;
        let sign = signed_term.signum();
        source.shift_rot_x = i16::try_from((signed_term - sign * 5) >> 2)
            .expect("Level 2 Meteor shift rotation fits i16");
        source.shift_rot_y = 512;

        // Source AddE7EE0x( dword, dword ) walks the first three cells of
        // each SEARCH.DAT shape.  For shape zero these are (0,0), (1,0), and
        // (0,1), with a 160-unit cell stride and signed 96/64 offsets.
        source.entity_rng = level2_next_rng16(source.entity_rng);
        for (offset_x, offset_y) in [(0_i32, 0_i32), (1, 0), (0, 1)] {
            source.entity_rng = level2_next_rng16(source.entity_rng);
            let x_jitter = i32::from(source.entity_rng % 0x81);
            let child_x = i32::from(source.position.x) - 96 + 160 * offset_x + x_jitter - 64;
            source.entity_rng = level2_next_rng16(source.entity_rng);
            let y_jitter = i32::from(source.entity_rng % 0x81);
            let child_y = i32::from(source.position.y) - 96 + 160 * offset_y + y_jitter - 64;
            let child_position = Position {
                x: child_x as u16,
                y: child_y as u16,
                z: source.position.z,
            };
            let _ = self.allocate_model0_child(
                source_slot,
                source.entity_id,
                source.yaw,
                child_position,
                0x0003_0080,
                512,
                512,
            );
        }
        source.dword_0x10 = (source.dword_0x10 + 2).rem_euclid(11);
        self.source_rng.insert(source_slot, source.entity_rng);
        source.phase = source.phase.wrapping_add(1);
        self.authored_model17[source_index] = source;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    fn allocate_model0_child(
        &mut self,
        parent_entity_index: u16,
        parent_entity_id: u16,
        parent_yaw: i16,
        position: Position,
        control_flags: u32,
        shift_rot_x: i16,
        shift_rot_y: i16,
    ) -> Option<u16> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.model17_spawned_children
            .push(Level2Model17SpawnedChild {
                source_entity_index,
                entity_rng,
                parent_entity_index,
                parent_entity_id,
                source_class: 10,
                model: 0,
                action: 0,
                subspell_index: 400,
                max_life: 8,
                life: 8,
                dword_0x10: 0,
                control_flags,
                shift_rot_x,
                shift_rot_y,
                yaw: parent_yaw,
                position,
                phase: source_entity_index as u8,
                active: true,
            });
        Some(source_entity_index)
    }

    /// The bounded child seam preserves the post-spawn `NewAdd0A00` life
    /// sentinel and map/reclaim order. Its terrain mutation/particle callback
    /// (`sub_30D50`) remains a distinct follow-up, so this method does not
    /// invent terrain edits while still making the generated row observable
    /// and finite.
    pub fn step_model17_spawned_child_slot(&mut self, source_slot: u16) -> bool {
        let Some(child_index) = self
            .model17_spawned_children
            .iter()
            .position(|child| child.active && child.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut child = self.model17_spawned_children[child_index];
        child.life = child.life.wrapping_sub(1);
        if child.life < -1 {
            child.active = false;
            self.model17_spawned_children[child_index] = child;
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: false,
                    x: child.position.x,
                    y: child.position.y,
                })
                .expect("Level 2 model-17 child slot fits the map chain");
            return true;
        }
        child.phase = child.phase.wrapping_add(1);
        self.model17_spawned_children[child_index] = child;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: child.position.x,
                y: child.position.y,
            })
            .is_ok()
    }

    /// Allocate the authored class-10/model-22 wind parent and its eleven
    /// cloned model-75/action-82 chain rows from `AddWind_4F040`. The source
    /// requires twelve available arena rows before it starts; no synthetic
    /// success is reported for an exhausted constructor.
    pub fn allocate_authored_model22(
        &mut self,
        authored_index: u16,
        axis_position: Position,
        position: Position,
    ) -> Option<Level2AuthoredModel22Source> {
        // `AddWind_4F040` refuses to start unless the source free-list has
        // room for the parent plus all eleven cloned chain rows.
        if self.free_slots.len() < 12 {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = level2_next_rng16(self.new_event_rng(source_entity_index));
        let mut position = position;
        position.z = self.terrain.ground_height(position);
        let yaw = i16::try_from((i32::from(entity_rng & 0x07ff) - 1) & 0x07ff)
            .expect("Level 2 wind yaw fits");
        let parent = Level2AuthoredModel22Source {
            source_entity_index,
            entity_id: source_entity_index,
            entity_rng,
            authored_index,
            source_class: 10,
            model: 22,
            action: 22,
            word_0x2c: 0,
            word_0x2e: 1,
            byte_0x38: 1,
            actual_speed: 50,
            minimum_speed: 20,
            maximum_speed: 10,
            max_life: 500,
            life: 500,
            subspell_index: 1_000,
            control_flags: 0,
            word_0x30: 0,
            yaw,
            pitch: yaw,
            roll: yaw,
            particle_index: LEVEL2_MODEL22_PARTICLE_INDEXES[0],
            shift_rot_x: LEVEL2_MODEL22_SHIFT_X[0],
            shift_rot_y: LEVEL2_MODEL22_SHIFT_Y[0],
            axis_position,
            position,
            // NewEvent seeds byte 0x3e from the source slot; ApplyEvents
            // increments it after the action callback returns.
            phase: source_entity_index as u8,
            active: true,
        };
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model22.push(parent);

        let mut z_offset = 2 * LEVEL2_MODEL22_SHIFT_Y[0];
        for segment_index in 1..=11_u8 {
            let Some(child_slot) = self.allocate_slot() else {
                break;
            };
            let child_z = i32::from(position.z)
                .saturating_add(i32::from(z_offset))
                .clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
            let child_position = Position {
                z: child_z,
                ..position
            };
            let child = Level2Model22SpawnedChild {
                source_entity_index: child_slot,
                entity_id: source_entity_index,
                entity_rng,
                max_life: 500,
                life: 500,
                parent_entity_index: source_entity_index,
                parent_entity_id: source_entity_index,
                source_class: 10,
                model: 75,
                action: 82,
                segment_index,
                word_0x2c: i16::from(segment_index),
                word_0x36: z_offset,
                yaw,
                pitch: yaw,
                roll: yaw,
                particle_index: LEVEL2_MODEL22_PARTICLE_INDEXES[usize::from(segment_index)],
                shift_rot_x: LEVEL2_MODEL22_SHIFT_X[usize::from(segment_index)],
                shift_rot_y: LEVEL2_MODEL22_SHIFT_Y[usize::from(segment_index)],
                position: child_position,
                phase: segment_index - 1,
                drawing_enabled: true,
                active: true,
            };
            if self
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: child_slot,
                    active: true,
                    x: child_position.x,
                    y: child_position.y,
                })
                .is_err()
            {
                self.release_slot(child_slot, None);
                break;
            }
            self.source_rng.insert(child_slot, entity_rng);
            self.model22_spawned_children.push(child);
            z_offset = z_offset.saturating_add(2 * child.shift_rot_y);
        }
        if let Some(parent) = self
            .authored_model22
            .iter_mut()
            .find(|parent| parent.source_entity_index == source_entity_index)
        {
            parent.entity_rng = entity_rng;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model22
            .iter()
            .find(|parent| parent.source_entity_index == source_entity_index)
            .copied()
    }

    /// Advance one authored wind parent through the bounded kinematic portion
    /// of `sub_33110`: source life, parent heading/translation, and linked
    /// chain spacing. The source's cross-entity pulse helpers (`sub_33340`
    /// and `sub_33710`) remain a separate relation-heavy follow-up.
    pub fn step_authored_model22_slot(&mut self, source_slot: u16) -> bool {
        let Some(parent_index) = self
            .authored_model22
            .iter()
            .position(|parent| parent.active && parent.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut parent = self.authored_model22[parent_index];
        parent.life = parent.life.wrapping_sub(1);
        if parent.life < 0 {
            parent.active = false;
            self.authored_model22[parent_index] = parent;
            let child_slots = self
                .model22_spawned_children
                .iter()
                .filter(|child| child.active && child.parent_entity_index == source_slot)
                .map(|child| child.source_entity_index)
                .collect::<Vec<_>>();
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: false,
                    x: parent.position.x,
                    y: parent.position.y,
                })
                .expect("Level 2 wind parent slot fits the map chain");
            let mut child_disable_rows = Vec::new();
            for child in &mut self.model22_spawned_children {
                if child_slots.contains(&child.source_entity_index) {
                    child.active = false;
                    child_disable_rows.push((
                        child.source_entity_index,
                        child.position.x,
                        child.position.y,
                    ));
                }
            }
            for (child_slot, x, y) in child_disable_rows {
                self.map_chains
                    .sync_entity(RenderableMapSeed {
                        entity_index: child_slot,
                        active: false,
                        x,
                        y,
                    })
                    .expect("Level 2 wind child slot fits the map chain");
            }
            return true;
        }
        parent.word_0x30 = parent.position.z as i16;
        if parent.phase & 0x0f == 0 {
            parent.entity_rng = level2_next_rng16(parent.entity_rng);
            if parent.entity_rng & 1 == 0 {
                parent.word_0x2e = -parent.word_0x2e;
            }
        }
        parent.roll = (parent.roll + 11 * parent.word_0x2e) & 0x07ff;
        let mut predicted = parent.position;
        crate::move_entity(&mut predicted, parent.roll, 0, 32);
        parent.axis_position = predicted;
        parent.yaw = (parent.yaw + 341) & 0x07ff;
        crate::move_entity(&mut predicted, parent.yaw, 0, 120);
        predicted.z = self.terrain.ground_height(predicted);
        parent.position = predicted;

        let mut previous = parent.position;
        let mut child_sync_rows = Vec::new();
        for child in self
            .model22_spawned_children
            .iter_mut()
            .filter(|child| child.active && child.parent_entity_index == source_slot)
        {
            child.yaw = i16::try_from(heading_for_positions(child.position, previous))
                .expect("Level 2 wind child heading fits");
            let distance = level2_wrapping_source_distance_3d(previous, child.position);
            let minimum_gap = 72 - 4 * (12 - child.word_0x2c);
            let mut child_position = child.position;
            if distance > u32::try_from(minimum_gap).unwrap_or(0) {
                crate::move_entity(
                    &mut child_position,
                    child.yaw,
                    0,
                    i16::try_from(distance - u32::try_from(minimum_gap).unwrap_or(0))
                        .unwrap_or(i16::MAX),
                );
            }
            child_position.z = i32::from(parent.position.z)
                .saturating_add(i32::from(child.word_0x36))
                .clamp(i32::from(i16::MIN), i32::from(i16::MAX))
                as i16;
            child.position = child_position;
            previous = child.position;
            child_sync_rows.push((
                child.source_entity_index,
                child.position.x,
                child.position.y,
            ));
        }
        for (child_slot, x, y) in child_sync_rows {
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: child_slot,
                    active: true,
                    x,
                    y,
                })
                .expect("Level 2 wind child slot fits the map chain");
        }
        parent.phase = parent.phase.wrapping_add(1);
        self.source_rng.insert(source_slot, parent.entity_rng);
        self.authored_model22[parent_index] = parent;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: parent.position.x,
                y: parent.position.y,
            })
            .is_ok()
    }

    /// Advance one class-10/model-75/action-82 child through `ApplyEvents`.
    /// Model 75 is rejected by the class-10 model-range gate before action
    /// dispatch, so the row is hidden and byte 0x3e advances. Class 10's
    /// action-82 slot is disabled; the similarly numbered class-5 slot is
    /// unrelated. No callback scans players, transfers a spell, draws RNG,
    /// emits audio, changes action/life, or settles terrain.
    pub fn step_model22_spawned_child_slot(&mut self, source_slot: u16) -> bool {
        let Some(child) = self
            .model22_spawned_children
            .iter_mut()
            .find(|child| child.active && child.source_entity_index == source_slot)
        else {
            return false;
        };
        child.drawing_enabled = false;
        child.phase = child.phase.wrapping_add(1);
        self.source_rng.insert(source_slot, child.entity_rng);
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: child.active,
                x: child.position.x,
                y: child.position.y,
            })
            .expect("Level 2 wind child slot fits the map chain");
        true
    }

    /// Allocate the authored class-10/model-76 Fire Spheres root plus its
    /// exact 25 linked model-77 children. The pinned constructor is atomic:
    /// if any row cannot be reserved or linked, the whole family is rolled
    /// back with no partial live cluster.
    pub fn allocate_authored_model76(
        &mut self,
        authored_index: u16,
        position: Position,
    ) -> Option<Level2AuthoredModel76Source> {
        if self.free_slots.len() < 1 + LEVEL2_MODEL76_CHILD_COUNT {
            return None;
        }
        let state_before = self.clone();
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let mut root_position = position;
        root_position.z = self.terrain.ground_height(root_position);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: root_position.x,
                y: root_position.y,
            })
            .is_err()
        {
            *self = state_before;
            return None;
        }
        let root = Level2AuthoredModel76Source {
            source_entity_index,
            entity_id: source_entity_index,
            entity_rng,
            authored_index,
            source_class: 10,
            model: 76,
            action: 83,
            max_life: 80,
            life: 80,
            subspell_index: 70,
            actual_speed: 40,
            minimum_speed: 480,
            maximum_speed: 192,
            word_0x2c: 192,
            font_type_index: 18,
            state: 0,
            target_entity_index: 0,
            target_position: root_position,
            target_pitch: 0,
            target_height_offset: 0,
            target_control_flags: ENTITY_DRAW_DISABLED_BIT,
            target_life: -1,
            shift_rot_x: 640,
            shift_rot_y: 640,
            yaw: 0,
            pitch: 0,
            position: root_position,
            phase: source_entity_index as u8,
            control_flags: 1,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_model76.push(root);
        for index in 0..LEVEL2_MODEL76_CHILD_COUNT {
            let child_slot = self.allocate_slot().unwrap_or_default();
            if child_slot == 0 {
                *self = state_before;
                return None;
            }
            let row_index = u8::try_from(index / 5).expect("model-76 child row fits");
            let column_index = u8::try_from(index % 5).expect("model-76 child column fits");
            let child_rng = level2_next_rng16(entity_rng);
            let orbit = i16::try_from((child_rng & 0x3f) + 84).expect("model-76 orbit fits");
            let (yaw, pitch, roll, fov, control_flags) = match row_index {
                0 => (
                    wrap_source_angle(512 - 96 * i16::from(column_index)),
                    0,
                    orbit,
                    0,
                    if column_index == 0 {
                        0x0000_0008
                    } else {
                        0x0080_0000
                    },
                ),
                1 => (
                    512,
                    wrap_source_angle(512 - 96 * i16::from(column_index)),
                    0,
                    orbit,
                    0x0080_0000,
                ),
                2 => (
                    0,
                    wrap_source_angle(-96 * i16::from(column_index)),
                    0,
                    orbit,
                    if column_index == 0 {
                        0x0000_0008
                    } else {
                        0x0080_0000
                    },
                ),
                3 => (
                    256,
                    wrap_source_angle(256 - 96 * i16::from(column_index)),
                    0,
                    orbit,
                    0x0080_0000,
                ),
                _ => (
                    768,
                    wrap_source_angle(768 - 96 * i16::from(column_index)),
                    0,
                    orbit,
                    0x0080_0000,
                ),
            };
            let mut child_position = root_position;
            crate::move_entity(&mut child_position, yaw, pitch, root.word_0x2c);
            let child = Level2Model76Child {
                source_entity_index: child_slot,
                entity_id: root.entity_id,
                entity_rng: child_rng,
                parent_entity_index: source_entity_index,
                parent_entity_id: root.entity_id,
                source_class: 10,
                model: 77,
                action: 84,
                max_life: root.max_life,
                life: root.life,
                subspell_index: root.subspell_index,
                actual_speed: root.actual_speed,
                minimum_speed: root.minimum_speed,
                maximum_speed: root.maximum_speed,
                word_0x2c: root.word_0x2c,
                row_index,
                column_index,
                chain_index: u8::try_from(index).expect("model-76 child index fits"),
                yaw,
                pitch,
                roll,
                fov,
                collision_yaw: 0,
                collision_pitch: 0,
                collision_roll: 0,
                collision_fov: 0,
                shift_rot_x: root.shift_rot_x,
                shift_rot_y: root.shift_rot_y,
                position: child_position,
                phase: root.phase,
                control_flags,
                drawing_enabled: true,
                active: true,
            };
            if self
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: child_slot,
                    active: true,
                    x: child_position.x,
                    y: child_position.y,
                })
                .is_err()
            {
                *self = state_before;
                return None;
            }
            self.source_rng.insert(child_slot, child_rng);
            self.model76_children.push(child);
        }
        self.authored_model76
            .iter()
            .find(|source| source.source_entity_index == source_entity_index)
            .copied()
    }

    fn allocate_model76_tail_spawn(
        &mut self,
        parent_entity_index: u16,
        parent_entity_id: u16,
        position: Position,
    ) -> Option<u16> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.model76_tail_spawns.push(Level2Model76TailSpawn {
            source_entity_index,
            entity_rng,
            parent_entity_index,
            parent_entity_id,
            source_class: 10,
            model: 0,
            action: 0,
            subspell_index: 400,
            max_life: 8,
            life: 8,
            shift_rot_x: 128,
            shift_rot_y: 128,
            particle_index: 7,
            position,
            phase: source_entity_index as u8,
            control_flags: 0x0002_0000,
            active: true,
        });
        Some(source_entity_index)
    }

    fn disable_model76_cluster(&mut self, parent_entity_index: u16) {
        if let Some(parent) = self
            .authored_model76
            .iter_mut()
            .find(|source| source.source_entity_index == parent_entity_index)
        {
            parent.drawing_enabled = false;
            parent.active = false;
        }
        for child in self
            .model76_children
            .iter_mut()
            .filter(|child| child.parent_entity_index == parent_entity_index)
        {
            child.drawing_enabled = false;
            child.active = false;
        }
    }

    /// Advance one Fire Spheres root through the pinned `sub_339B0` root
    /// callback: target-follow/clamp, expanding orbit, child movement order,
    /// terminal `word_0x2c` countdown, model-0 tail spawn, and whole-cluster
    /// disable semantics.
    pub fn step_authored_model76_slot(&mut self, source_slot: u16) -> bool {
        let Some(source_index) = self
            .authored_model76
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut source = self.authored_model76[source_index];
        let target_live = source.target_entity_index != 0
            && source.target_life >= 0
            && source.target_control_flags & ENTITY_DRAW_DISABLED_BIT == 0;
        if source.state == 0 {
            if source.target_entity_index != 0 {
                if !target_live {
                    source.state = 2;
                } else {
                    source.maximum_speed = (source.target_pitch >> 1).max(128);
                    source.minimum_speed = ((6 * source.target_pitch) >> 2).min(640);
                    source.position = source.target_position;
                    source.position.z = source.position.z.wrapping_add(source.target_height_offset);
                    source.state = 1;
                }
            } else {
                source.state = 1;
            }
        }
        if source.state != 2 && source.target_entity_index != 0 {
            if !target_live {
                source.state = 2;
            } else {
                source.position = source.target_position;
                source.position.z = source.position.z.wrapping_add(source.target_height_offset);
            }
        }
        if source.state == 2 {
            if source.font_type_index < 0 {
                source.font_type_index = source.font_type_index.wrapping_neg();
            }
            source.word_0x2c = source
                .word_0x2c
                .wrapping_sub(i16::from(source.font_type_index));
            if source.word_0x2c < 0 {
                let _ = self.allocate_model76_tail_spawn(
                    source_slot,
                    source.entity_id,
                    source.position,
                );
                self.disable_model76_cluster(source_slot);
                self.map_chains
                    .sync_entity(RenderableMapSeed {
                        entity_index: source_slot,
                        active: false,
                        x: source.position.x,
                        y: source.position.y,
                    })
                    .expect("Level 2 model-76 source slot fits the map chain");
                for child in self
                    .model76_children
                    .iter()
                    .filter(|child| child.parent_entity_index == source_slot)
                {
                    self.map_chains
                        .sync_entity(RenderableMapSeed {
                            entity_index: child.source_entity_index,
                            active: false,
                            x: child.position.x,
                            y: child.position.y,
                        })
                        .expect("Level 2 model-76 child slot fits the map chain");
                }
                return true;
            }
            source.phase = source.phase.wrapping_add(1);
            self.authored_model76[source_index] = source;
            self.source_rng.insert(source_slot, source.entity_rng);
            return true;
        }

        let ground = self.terrain.ground_height(source.position);
        let minimum_z = i32::from(ground) + i32::from(source.word_0x2c);
        if i32::from(source.position.z) < minimum_z {
            source.position.z = minimum_z.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
        }
        if let Some(ceiling) = self.terrain.ceiling_height(source.position) {
            let maximum_z = i32::from(ceiling) - i32::from(source.word_0x2c);
            if i32::from(source.position.z) > maximum_z {
                source.position.z =
                    maximum_z.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
            }
        }
        let next_word_0x2c = source
            .word_0x2c
            .wrapping_add(i16::from(source.font_type_index));
        if next_word_0x2c > source.minimum_speed {
            source.word_0x2c = source.minimum_speed;
            source.font_type_index = source.font_type_index.wrapping_neg();
        } else if next_word_0x2c < source.maximum_speed {
            source.word_0x2c = source.maximum_speed;
            source.font_type_index = source.font_type_index.wrapping_neg();
        } else {
            source.word_0x2c = next_word_0x2c;
        }

        source.yaw = wrap_source_angle(source.yaw.wrapping_add(22));
        source.pitch = wrap_source_angle(source.pitch.wrapping_add(16));
        let mut child_sync_rows = Vec::new();
        for child in self
            .model76_children
            .iter_mut()
            .filter(|child| child.active && child.parent_entity_index == source_slot)
        {
            child.yaw = wrap_source_angle(child.yaw.wrapping_add(child.roll));
            child.pitch = wrap_source_angle(child.pitch.wrapping_add(child.fov));
            let mut child_position = source.position;
            crate::move_entity(
                &mut child_position,
                wrap_source_angle(child.yaw.wrapping_add(source.yaw)),
                wrap_source_angle(child.pitch.wrapping_add(source.pitch)),
                source.word_0x2c,
            );
            child.position = child_position;
            child.phase = child.phase.wrapping_add(1);
            child_sync_rows.push((
                child.source_entity_index,
                child.position.x,
                child.position.y,
            ));
        }
        for (child_slot, x, y) in child_sync_rows {
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: child_slot,
                    active: true,
                    x,
                    y,
                })
                .expect("Level 2 model-76 child slot fits the map chain");
        }
        // Pinned `sub_33C00` probes each column-0 child through the shared
        // `sub_10C80` collision/effect owner.  This Level 2 slice has no
        // generic runtime collision owner for authored model-76 rows yet, so
        // it deliberately records only the source movement/map state here;
        // no synthetic hit or damage event is emitted.
        source.life = source.life.wrapping_sub(1);
        if source.life < 1 {
            source.state = 2;
        }
        source.phase = source.phase.wrapping_add(1);
        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_model76[source_index] = source;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    /// The linked Fire Spheres children are positioned by the root callback.
    /// Their individual cursor visits are still observable in the live arena,
    /// but have no additional bounded behavior beyond phase retention.
    pub fn step_model76_child_slot(&mut self, source_slot: u16) -> bool {
        let Some(child) = self
            .model76_children
            .iter_mut()
            .find(|child| child.active && child.source_entity_index == source_slot)
        else {
            return false;
        };
        child.phase = child.phase.wrapping_add(1);
        true
    }

    /// Advance the Fire Spheres terminal model-0 tail through the same bounded
    /// life/reclaim seam used for other model-0 rows in this Rust campaign
    /// path. Terrain-side effects remain a separate follow-up.
    pub fn step_model76_tail_spawn_slot(&mut self, source_slot: u16) -> bool {
        let Some(tail) = self
            .model76_tail_spawns
            .iter_mut()
            .find(|tail| tail.active && tail.source_entity_index == source_slot)
        else {
            return false;
        };
        tail.life = tail.life.wrapping_sub(1);
        if tail.life < -1 {
            tail.active = false;
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: false,
                    x: tail.position.x,
                    y: tail.position.y,
                })
                .expect("Level 2 model-76 tail slot fits the map chain");
            return true;
        }
        tail.phase = tail.phase.wrapping_add(1);
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: tail.position.x,
                y: tail.position.y,
            })
            .is_ok()
    }

    /// Advance the generated class-9/model-14 child through `sub_322A0`.
    pub fn step_model6_spawned_child_slot(&mut self, source_slot: u16) -> bool {
        let Some(child_index) = self
            .model6_spawned_children
            .iter()
            .position(|child| child.active && child.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut child = self.model6_spawned_children[child_index];
        let life_before = child.life;
        child.life = child.life.wrapping_sub(1);
        if life_before < 0 {
            child.active = false;
            self.model6_spawned_children[child_index] = child;
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: false,
                    x: child.position.x,
                    y: child.position.y,
                })
                .expect("Level 2 model-6 child slot fits the map chain");
            return true;
        }
        child.actual_speed = (child.actual_speed - 4).clamp(64, 128);
        child.position.z = child.position.z.wrapping_add(child.actual_speed);
        let ground = self.terrain.ground_height(child.position);
        if child.position.z < ground {
            child.position.z = ground;
        }
        child.dword_0x10 += 1;
        if child.dword_0x10 < 16 {
            crate::move_entity(&mut child.position, child.yaw, 0, child.max_speed);
            child.max_speed = (child.max_speed - 52).clamp(30, 1024);
            if child.dword_0x10 & 1 == 0 && child.particle_index < 16 {
                child.particle_index += 1;
            }
        }
        if child.life < 6 && child.particle_index > 9 {
            child.particle_index -= 1;
        }
        self.model6_spawned_children[child_index] = child;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: child.position.x,
                y: child.position.y,
            })
            .is_ok()
    }

    /// Dispatch the source callbacks selected by the class-14 constructor.
    /// `sub_59C40_getTerrtoZ` (model 3/action 8) and the non-collecting branch
    /// of `UpdateScroll_59C80` (model 5/action 10) both snap the row to the
    /// current terrain height before their mission-specific work.  The
    /// Level-2 economy owns that shared source write; the enclosing reset
    /// session owns the model-5 player scan and experience presentation after
    /// this callback returns.
    pub fn step_authored_class14_slot(&mut self, source_slot: u16, ground_height: i16) -> bool {
        let Some(source_index) = self
            .authored_class14
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut source = self.authored_class14[source_index];
        source.position.z = ground_height;
        self.authored_class14[source_index] = source;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    /// Apply `DisableEntityDrawing04_57F10` to an authored class-14 row. The
    /// source callback leaves the row for the arena reclaim pass; unlinking
    /// here preserves that ordering instead of deleting it from the handler.
    pub fn deactivate_authored_class14_slot(&mut self, source_slot: u16) -> bool {
        let Some(source) = self
            .authored_class14
            .iter_mut()
            .find(|source| source.source_entity_index == source_slot && source.active)
        else {
            return false;
        };
        source.active = false;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: false,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    /// Allocate a separately dispatched class-10/model-74/action-72 effect.
    /// The caller supplies the already source-derived payload; this method
    /// owns the shared NewEvent slot/RNG, map link, and reclaim identity just
    /// like the other dynamic Level 2 rows. Class-15/action-36 never calls it.
    pub fn allocate_beyond_sight_pulse_effect(
        &mut self,
        holder_entity_index: u16,
        parent_entity_index: u16,
        parent_entity_id: u16,
        subspell_index: u16,
        life: i16,
        effect_parameter: u8,
        actual_speed: i16,
        position: Position,
        axis_position: Position,
        yaw: i16,
        pitch: i16,
    ) -> Option<Level2BeyondSightPulseEffect> {
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let effect = Level2BeyondSightPulseEffect {
            source_entity_index,
            entity_rng,
            holder_entity_index,
            parent_entity_index,
            parent_entity_id,
            source_class: 10,
            model: 74,
            origin_subtype: 25,
            subspell_index,
            life,
            effect_parameter,
            actual_speed,
            position,
            axis_position,
            yaw,
            pitch,
            sound_id: 9,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.beyond_sight_effects.push(effect);
        Some(effect)
    }

    /// The separate action-72/model-74 effect has no generic movement callback
    /// in this bounded slice; keep its payload live and map-linked for a
    /// future admitted dispatcher without mutating constructor fields.
    pub fn step_beyond_sight_pulse_effect_slot(&mut self, source_slot: u16) -> bool {
        let Some(effect) = self
            .beyond_sight_effects
            .iter()
            .find(|effect| effect.active && effect.source_entity_index == source_slot)
            .copied()
        else {
            return false;
        };
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: effect.position.x,
                y: effect.position.y,
            })
            .is_ok()
    }

    /// Advance one class-10/model-54/69 temporary field through
    /// `sub_38D80`. Eligible mana spheres receive one source-linked attraction
    /// mailbox in source order; an already-targeted sphere is left untouched.
    pub fn step_mana_attraction_slot(&mut self, source_slot: u16) -> bool {
        self.step_mana_attraction_slot_with_membership(source_slot, None)
    }

    /// Advance one attraction field using the model-39 membership rebuilt at
    /// the beginning of the ordinary `UpdateEntities_57730` pass. New spheres
    /// allocated during that pass remain eligible for their own higher-slot
    /// callbacks, but cannot become field candidates until the next pass
    /// rebuilds the source-linked membership list.
    pub fn step_mana_attraction_slot_with_sphere_membership(
        &mut self,
        source_slot: u16,
        sphere_membership: &BTreeSet<u16>,
    ) -> bool {
        self.step_mana_attraction_slot_with_membership(source_slot, Some(sphere_membership))
    }

    fn step_mana_attraction_slot_with_membership(
        &mut self,
        source_slot: u16,
        sphere_membership: Option<&BTreeSet<u16>>,
    ) -> bool {
        let Some(field_index) = self
            .mana_attraction_fields
            .iter()
            .position(|field| field.active && field.source_entity_index == source_slot)
        else {
            return false;
        };
        let (position, radius_squared, action, life_before) = {
            let field = self.mana_attraction_fields[field_index];
            (
                field.position,
                field.radius_squared,
                field.action,
                field.life,
            )
        };
        debug_assert!(matches!(
            action,
            MANA_ATTRACTION_ACTION | MANA_ATTRACTION_ALIAS_ACTION
        ));

        // `sub_38D80` decrements before testing the signed life sentinel. A
        // zero-life row therefore gets one final producer pass; the next
        // cursor visit disables it and returns the slot to the arena stack.
        if let Some(field) = self.mana_attraction_fields.get_mut(field_index) {
            field.life = field.life.saturating_sub(1);
            if life_before < 0 {
                field.active = false;
                self.map_chains
                    .sync_entity(RenderableMapSeed {
                        entity_index: source_slot,
                        active: false,
                        x: position.x,
                        y: position.y,
                    })
                    .expect("Level 2 mana-attraction source slot fits the map chain");
                return true;
            }
        }

        for sphere in self.spheres.iter_mut().filter(|sphere| {
            sphere.active
                && sphere.attraction_source_entity_index == 0
                && sphere_membership.map_or(true, |membership| {
                    membership.contains(&sphere.source_entity_index)
                })
        }) {
            let distance_squared = level2_wrapping_xy_distance(position, sphere.position);
            if distance_squared < radius_squared {
                let distance = crate::legacy_sqrt(distance_squared);
                sphere.attraction_step =
                    u16::try_from(distance.min(42)).expect("Level 2 attraction step fits u16");
                sphere.attraction_distance_high = u16::try_from(distance >> 16)
                    .expect("Level 2 attraction distance high word fits u16");
                sphere.attraction_source_entity_index = source_slot;
            }
        }
        true
    }

    /// Allocate the source class-15 row created by `sub_69250`. This uses the
    /// same free stack, constructor RNG, map chain, and failure transaction as
    /// every other Level 2 `NewEvent_4A050` allocation.
    pub fn allocate_spell_replacement(
        &mut self,
        family: u8,
        parent_entity_index: u16,
        position: Position,
    ) -> Option<Level2SpellReplacement> {
        if family >= 26 {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let replacement = Level2SpellReplacement {
            source_entity_index,
            entity_rng,
            parent_entity_index,
            position,
            family,
            action: family.saturating_mul(3).saturating_add(2),
            phase: source_entity_index as u8,
            control_flags: 0,
            active: true,
        };
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.spell_replacements.push(replacement);
        Some(replacement)
    }

    /// Convert one persistent pickup source into its holder action after the
    /// shared `sub_68FF0` transition. The source row remains in the live
    /// arena (the new replacement is the row that carries the persistent
    /// action), so the cursor can still observe its phase/map writes.
    pub fn mark_spell_replacement_acquired(
        &mut self,
        source_slot: u16,
        holder_action: u8,
    ) -> Option<Position> {
        let replacement = self.spell_replacements.iter_mut().find(|replacement| {
            replacement.active && replacement.source_entity_index == source_slot
        })?;
        replacement.action = holder_action;
        Some(replacement.position)
    }

    /// The family callback remains outside this bounded allocator slice. The
    /// source cursor still visits the live row and preserves source phase/map
    /// state; no invented spell effect is emitted here.
    pub fn step_spell_replacement_slot(&mut self, source_slot: u16) -> bool {
        let Some(replacement) = self.spell_replacements.iter_mut().find(|replacement| {
            replacement.active && replacement.source_entity_index == source_slot
        }) else {
            return false;
        };
        replacement.phase = replacement.phase.wrapping_add(1);
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: replacement.position.x,
                y: replacement.position.y,
            })
            .is_ok()
    }

    #[must_use]
    pub fn carrier(&self) -> Option<&ManaCarrier> {
        self.carriers.iter().find(|carrier| carrier.active)
    }

    /// Return source-carrier rows in allocation order, including inactive
    /// rows that remain until the source reclaim pass. The legacy `carrier()`
    /// accessor remains as a single-row active compatibility view for callers
    /// that only render the first live carrier.
    #[must_use]
    pub fn carriers(&self) -> &[ManaCarrier] {
        &self.carriers
    }

    #[must_use]
    pub const fn castle(&self) -> Option<Level2CastleState> {
        self.castle
    }

    #[must_use]
    pub fn castle_projectiles(&self) -> &[Level2CastleProjectile] {
        &self.castle_projectiles
    }

    #[must_use]
    pub fn possession_projectiles(&self) -> &[Level2PossessionProjectile] {
        &self.possession_projectiles
    }

    #[must_use]
    pub fn fireball_projectiles(&self) -> &[Level2FireballProjectile] {
        &self.fireball_projectiles
    }

    #[must_use]
    pub fn possession_effects(&self) -> &[Level2PossessionEffect] {
        &self.possession_effects
    }

    #[must_use]
    pub fn player_castle_shift_aabb_overlap(&self, player: &EntitySnapshot) -> bool {
        let Some(castle) = self.castle.filter(|castle| {
            castle.active && self.player_castle_entity_index == castle.source_entity_index
        }) else {
            return false;
        };
        level2_model76_sub106c0_matches(
            castle.position,
            castle.shift_yaw,
            castle.shift_pitch,
            castle.shift_roll,
            castle.shift_fov,
            player.position,
            player.array_yaw(),
            player.array_pitch(),
            player.array_roll(),
            player.array_fov(),
        )
    }

    #[must_use]
    pub fn castle_children(&self) -> &[Level2CastleChild] {
        &self.castle_children
    }

    #[must_use]
    fn slot_generation(&self, slot: u16) -> u32 {
        self.slot_generations
            .get(&slot)
            .copied()
            .unwrap_or_default()
    }

    fn for_each_live_arena_slot(&self, mut visit: impl FnMut(u16, Level2DynamicArenaKind)) {
        for projectile in self
            .castle_projectiles
            .iter()
            .filter(|projectile| projectile.active)
        {
            visit(
                projectile.source_entity_index,
                Level2DynamicArenaKind::CastleProjectile,
            );
        }
        for projectile in self
            .fireball_projectiles
            .iter()
            .filter(|projectile| projectile.active)
        {
            visit(
                projectile.source_entity_index,
                Level2DynamicArenaKind::FireballProjectile,
            );
        }
        for projectile in self
            .possession_projectiles
            .iter()
            .filter(|projectile| projectile.active)
        {
            visit(
                projectile.source_entity_index,
                Level2DynamicArenaKind::PossessionProjectile,
            );
        }
        for effect in self
            .possession_effects
            .iter()
            .filter(|effect| effect.active)
        {
            visit(
                effect.source_entity_index,
                Level2DynamicArenaKind::PossessionEffect,
            );
        }
        if let Some(castle) = self.castle.filter(|castle| castle.active) {
            visit(
                castle.source_entity_index,
                Level2DynamicArenaKind::CastleRoot,
            );
        }
        for child in self.castle_children.iter().filter(|child| child.active) {
            visit(
                child.source_entity_index,
                match child.model {
                    15 => Level2DynamicArenaKind::CastleAuxiliary,
                    3 => Level2DynamicArenaKind::CastleDefender,
                    79 => Level2DynamicArenaKind::CastleController,
                    42 | 41 => Level2DynamicArenaKind::CastleChild,
                    _ => Level2DynamicArenaKind::CastleChild,
                },
            );
        }
        for sphere in self.spheres.iter().filter(|sphere| sphere.active) {
            visit(
                sphere.source_entity_index,
                Level2DynamicArenaKind::ManaSphere,
            );
        }
        for burst in self.death_bursts.iter().filter(|burst| burst.active) {
            visit(
                burst.source_entity_index,
                Level2DynamicArenaKind::DeathBurst,
            );
        }
        for carrier in self.carriers.iter().filter(|carrier| carrier.active) {
            visit(
                carrier.source_entity_index,
                Level2DynamicArenaKind::ManaCarrier,
            );
        }
        for replacement in self
            .spell_replacements
            .iter()
            .filter(|replacement| replacement.active)
        {
            visit(
                replacement.source_entity_index,
                Level2DynamicArenaKind::SpellReplacement,
            );
        }
        for source in self.spell_pickups.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::SpellPickup,
            );
        }
        for source in self.authored_model1.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel1,
            );
        }
        for source in self.authored_model45.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel45,
            );
        }
        for source in self.authored_model13.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel13,
            );
        }
        for source in self.authored_bees.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredBee,
            );
        }
        for source in self
            .authored_switch_producers
            .iter()
            .filter(|source| source.active)
        {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredSwitchProducer,
            );
        }
        for source in self.authored_trees.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredTree,
            );
        }
        for source in self.authored_model32.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel32,
            );
        }
        for source in self.authored_model59.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel59,
            );
        }
        for child in self
            .model59_smoke_children
            .iter()
            .filter(|child| child.active)
        {
            visit(
                child.source_entity_index,
                Level2DynamicArenaKind::Model59SmokeChild,
            );
        }
        for source in self.authored_model53.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel53,
            );
        }
        for source in self
            .authored_model5_water_splashes
            .iter()
            .filter(|source| source.active)
        {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel5WaterSplash,
            );
        }
        for child in self
            .model13_spawned_children
            .iter()
            .filter(|child| child.active)
        {
            visit(
                child.source_entity_index,
                Level2DynamicArenaKind::Model13SpawnedChild,
            );
        }
        for source in self.authored_model6.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel6,
            );
        }
        for child in self
            .model6_spawned_children
            .iter()
            .filter(|child| child.active)
        {
            visit(
                child.source_entity_index,
                Level2DynamicArenaKind::Model6SpawnedChild,
            );
        }
        for source in self.authored_model17.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel17,
            );
        }
        for child in self
            .model17_spawned_children
            .iter()
            .filter(|child| child.active)
        {
            visit(
                child.source_entity_index,
                Level2DynamicArenaKind::Model17SpawnedChild,
            );
        }
        for source in self.authored_model22.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel22,
            );
        }
        for child in self
            .model22_spawned_children
            .iter()
            .filter(|child| child.active)
        {
            visit(
                child.source_entity_index,
                Level2DynamicArenaKind::Model22SpawnedChild,
            );
        }
        for source in self.authored_model76.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredModel76,
            );
        }
        for child in self.model76_children.iter().filter(|child| child.active) {
            visit(
                child.source_entity_index,
                Level2DynamicArenaKind::Model76Child,
            );
        }
        for tail in self.model76_tail_spawns.iter().filter(|tail| tail.active) {
            visit(
                tail.source_entity_index,
                Level2DynamicArenaKind::Model76TailSpawn,
            );
        }
        for source in self.authored_class14.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredClass14,
            );
        }
        for source in self.authored_class5.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::AuthoredClass5,
            );
        }
        for child in self
            .authored_class5_archer_children
            .iter()
            .filter(|child| child.active)
        {
            visit(
                child.source_entity_index,
                Level2DynamicArenaKind::AuthoredClass5ArcherChild,
            );
        }
        for child in self
            .authored_class5_model3_children
            .iter()
            .filter(|child| child.active)
        {
            visit(
                child.source_entity_index,
                Level2DynamicArenaKind::AuthoredClass5Model3Child,
            );
        }
        for source in self.house_residents.iter().filter(|source| source.active) {
            visit(
                source.source_entity_index,
                Level2DynamicArenaKind::HouseResident,
            );
        }
        for effect in self
            .beyond_sight_effects
            .iter()
            .filter(|effect| effect.active)
        {
            visit(
                effect.source_entity_index,
                Level2DynamicArenaKind::BeyondSightPulseEffect,
            );
        }
        for field in self
            .mana_attraction_fields
            .iter()
            .filter(|field| field.active)
        {
            visit(
                field.source_entity_index,
                Level2DynamicArenaKind::ManaAttraction,
            );
        }
    }

    /// Return the currently live dynamic source rows in ascending source-slot
    /// order. This is the bounded arena-cursor seam: allocations and releases
    /// are observed from live projections on every pass rather than captured
    /// once at reset. Per-object movement remains owned by the economy bead.
    #[must_use]
    pub fn live_arena_slots(&self) -> Vec<(u16, Level2DynamicArenaKind)> {
        let mut slots = BTreeMap::new();
        self.for_each_live_arena_slot(|slot, kind| {
            slots.insert(slot, kind);
        });
        slots.into_iter().collect()
    }

    #[must_use]
    fn live_arena_cursor_rows(&self) -> Vec<Level2DynamicArenaCursorRow> {
        let mut rows = BTreeMap::new();
        self.for_each_live_arena_slot(|slot, kind| {
            rows.insert(
                slot,
                Level2DynamicArenaCursorRow {
                    source_slot: slot,
                    generation: self.slot_generation(slot),
                    kind,
                },
            );
        });
        rows.into_values().collect()
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
    pub const fn player_spell_slot_2(&self) -> u16 {
        self.player_spell_slot_2
    }

    #[must_use]
    pub const fn player_spell_holder_action(&self) -> u16 {
        self.player_spell_holder_action
    }

    /// Bind the source conditions used by `sub_605E0` when the final Castle
    /// stage is removed. The reset package normally leaves this disabled; the
    /// setter lets an imported player/terrain projection opt into the exact
    /// model-1, terrain-bit-4 holder cleanup without conflating it with the
    /// economy owner relation.
    pub fn set_player_spell_cleanup_context(
        &mut self,
        player_model: u8,
        terrain_flags: u8,
        holder_slot: u16,
    ) {
        self.player_model = player_model;
        self.player_terrain_flags = terrain_flags;
        self.player_spell_slot_2 = holder_slot;
        self.player_spell_holder_action = 0;
        self.player_spell_holder_max_action = 1;
    }

    /// Bind the preserved holder `word_0x2E`/`word_0x30` pair when a caller
    /// supplies a synthetic player cleanup context.  The ordinary setter
    /// above remains a compact compatibility helper for callers that only
    /// know the holder relation.
    pub fn set_player_spell_holder_counters(&mut self, action: u16, max_action: u16) {
        self.player_spell_holder_action = action;
        self.player_spell_holder_max_action = max_action;
    }

    fn initialize_player_spell_cleanup_context(
        &mut self,
        player_model: u8,
        terrain_flags: u8,
        holder_slot: u16,
        action: u16,
        max_action: u16,
    ) {
        self.player_model = player_model;
        self.player_terrain_flags = terrain_flags;
        self.player_spell_slot_2 = holder_slot;
        self.player_spell_holder_action = action;
        self.player_spell_holder_max_action = max_action;
        self.baseline_player_model = player_model;
        self.baseline_player_terrain_flags = terrain_flags;
        self.baseline_player_spell_slot_2 = holder_slot;
        self.baseline_player_spell_holder_action = action;
        self.baseline_player_spell_holder_max_action = max_action;
    }

    fn seal_direct_starting_spell_baseline(&mut self) {
        self.baseline_free_slots = self.free_slots.clone().into_boxed_slice();
        self.baseline_slot_generations = self.slot_generations.clone();
        self.baseline_source_rng = self.source_rng.clone();
        self.baseline_map_chains = self.map_chains.clone();
    }

    #[must_use]
    pub const fn free_slot_count(&self) -> usize {
        self.free_slots.len()
    }

    #[must_use]
    pub fn events(&self) -> &[Level2EconomyEvent] {
        &self.events
    }

    /// Record one source-shaped `PrepareEventSound_6E450` call. The handler
    /// slot and the exact entity argument are separate because house and
    /// switch callbacks prepare sound on an owner/player row rather than on
    /// their triggering source row.
    pub(super) fn prepare_sound(
        &mut self,
        source_entity_index: u16,
        entity_index: u16,
        player_index: i16,
        sound_id: u16,
    ) {
        self.events.push(Level2EconomyEvent::PreparedSound {
            source_entity_index,
            entity_index,
            player_index,
            sound_id,
        });
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

    /// Refresh the current class/model-filtered source rows after a typed
    /// economy mutation. Baseline rows retain the last source-boundary
    /// relation; the owning pass below is the only place that adopts newly
    /// written `source_owner` relations. The published aggregate and global
    /// counter remain frozen until that boundary as they do in `sub_60F00`.
    fn refresh_owner_mana_rows(&mut self) {
        let baseline_source_slots = self
            .baseline_owner_mana_rows
            .iter()
            .map(|row| row.source_entity_index)
            .collect::<BTreeSet<_>>();
        let mut active_dynamic_source_slots = BTreeSet::new();
        if let Some(castle) = self.castle.filter(|castle| castle.active) {
            active_dynamic_source_slots.insert(castle.source_entity_index);
        }
        active_dynamic_source_slots.extend(
            self.castle_children
                .iter()
                .filter(|child| child.active)
                .map(|child| child.source_entity_index),
        );
        active_dynamic_source_slots.extend(
            self.spheres
                .iter()
                .filter(|sphere| sphere.active)
                .map(|sphere| sphere.source_entity_index),
        );
        active_dynamic_source_slots.extend(
            self.carriers
                .iter()
                .filter(|carrier| carrier.active)
                .map(|carrier| carrier.source_entity_index),
        );
        self.owner_mana_rows.retain(|row| {
            baseline_source_slots.contains(&row.source_entity_index)
                || active_dynamic_source_slots.contains(&row.source_entity_index)
        });
        if let Some(castle) = self.castle.filter(|castle| castle.active) {
            self.install_owner_mana_row(
                castle.source_entity_index,
                castle.source_class,
                castle.source_model,
                castle.source_id_entity_index,
                castle.mana,
            );
        }
        let children = self
            .castle_children
            .iter()
            .filter(|child| child.active)
            .copied()
            .collect::<Vec<_>>();
        for child in children {
            self.install_owner_mana_row(
                child.source_entity_index,
                child.source_class,
                child.model,
                child.player_entity_index,
                child.mana,
            );
        }
        let spheres = self
            .spheres
            .iter()
            .filter(|sphere| sphere.active)
            .copied()
            .collect::<Vec<_>>();
        for sphere in spheres {
            self.install_owner_mana_row(
                sphere.source_entity_index,
                10,
                39,
                sphere.owner_entity_index,
                sphere.mana,
            );
        }
        let carriers = self
            .carriers
            .iter()
            .filter(|carrier| carrier.active)
            .copied()
            .collect::<Vec<_>>();
        for carrier in carriers {
            self.install_owner_mana_row(
                carrier.source_entity_index,
                3,
                3,
                carrier.owner_entity_index,
                carrier.carried_mana,
            );
        }
        self.owner_mana_rows
            .sort_unstable_by_key(|row| row.source_entity_index);
    }

    fn publish_owner_mana_projection(&mut self) {
        let (aggregate, global) = level2_owner_mana_projection(&self.owner_mana_rows);
        self.owner_mana_aggregate = aggregate;
        self.owner_mana_global = global;
    }

    /// Run the source `sub_60F00` owner-row reset at the owning-pass
    /// boundary. House callbacks update `source_owner` during the cursor;
    /// those writes become visible only at this next boundary, while typed
    /// sphere/carrier callbacks continue to refresh their own rows without
    /// retroactively changing the frozen baseline relation for the pass.
    fn rebuild_owner_mana_projection_at_source_boundary(&mut self) {
        self.owner_mana_rows = self
            .baseline_owner_mana_rows
            .iter()
            .filter_map(|row| {
                self.source_owner
                    .get(&row.source_entity_index)
                    .map(|&owner| Level2OwnerManaRow {
                        player_entity_index: owner,
                        ..*row
                    })
            })
            .collect();
        self.refresh_owner_mana_rows();
        self.publish_owner_mana_projection();
    }

    /// Run the source `sub_60F00` max-mana reset/sink at its owning-pass
    /// boundary. Generic sphere/carrier mutations refresh the underlying rows
    /// but must not publish aggregate/global/capacity earlier than this turn.
    fn rebuild_owner_max_mana_projection(&mut self) {
        self.owner_max_mana =
            level2_owner_max_mana_projection(&self.baseline_owner_max_mana, &self.owner_mana_rows);
    }

    #[must_use]
    pub fn owner_mana_aggregate(&self, owner_entity_index: u16) -> i32 {
        self.owner_mana_aggregate
            .get(&owner_entity_index)
            .copied()
            .unwrap_or_default()
    }

    #[must_use]
    pub const fn owner_mana_global(&self) -> i32 {
        self.owner_mana_global
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

    /// Bind the raw player `array_0x24E[9 + level]` byte copied to each
    /// model-79 controller by source `sub_613D0`.
    pub fn set_castle_controller_coefficient(&mut self, level: u8, coefficient: u8) -> bool {
        let Some(value) = self
            .castle_controller_coefficients
            .get_mut(usize::from(level))
        else {
            return false;
        };
        *value = coefficient;
        true
    }

    /// Bind a defender's source `byte_0x3E_62` cadence phase. The source
    /// stores this byte on the child entity, independently of Castle phase;
    /// callers without an imported byte use the Castle phase at allocation.
    pub fn set_castle_defender_source_phase(&mut self, slot: u16, phase: u8) -> bool {
        let Some(child) = self.castle_children.iter_mut().find(|child| {
            child.active
                && child.source_entity_index == slot
                && child.model == 3
                && child.action == 9
        }) else {
            return false;
        };
        child.source_byte_3e = phase;
        true
    }

    /// Rebuild `UpdateEntities_57730`'s model-39 guidance membership once at
    /// the source-pass boundary. Same-pass births are deliberately absent;
    /// callbacks resolve these fixed slots against the current record.
    fn rebuild_possession_guidance_source_slots(&mut self) {
        self.possession_guidance_source_slots = self
            .spheres
            .iter()
            .filter(|sphere| sphere.active || sphere.pending_delete)
            .map(|sphere| sphere.source_entity_index)
            .collect();
        self.possession_guidance_source_slots.sort_unstable();
        self.possession_guidance_source_slots.dedup();
    }

    pub fn reclaim_inactive_slots(&mut self) {
        // Castle projectiles are disabled by their source callback and
        // reclaimed only by reclaim_hidden_castle_rows_at_source_boundary at
        // the following UpdateEntities prologue. Same-pass reclamation would
        // erase a blocked flyer before the source cursor can observe the
        // hidden row's reserved slot.
        let reclaimed_carriers = self
            .carriers
            .iter()
            .filter(|carrier| !carrier.active)
            .map(|carrier| (carrier.source_entity_index, carrier.entity_rng))
            .collect::<Vec<_>>();
        self.carriers.retain(|carrier| carrier.active);
        for (slot, entity_rng) in reclaimed_carriers {
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed = self
            .spheres
            .iter()
            .filter(|sphere| !sphere.active)
            .map(|sphere| {
                (
                    sphere.source_entity_index,
                    sphere.entity_rng,
                    level2_possession_guidance_target_top(sphere),
                )
            })
            .collect::<Vec<_>>();
        self.spheres.retain(|sphere| sphere.active);
        for (slot, entity_rng, target_top) in reclaimed {
            // `sub_57F20` frees the slot but leaves its fixed entity record in
            // place until `NewEvent` overwrites it. An already-guided flyer
            // therefore continues `sub_65610` against this stale transform.
            self.possession_guidance_stale_records
                .insert(slot, target_top);
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_bursts = self
            .death_bursts
            .iter()
            .filter(|burst| !burst.active)
            .map(|burst| (burst.source_entity_index, burst.entity_rng))
            .collect::<Vec<_>>();
        self.death_bursts.retain(|burst| burst.active);
        for (slot, entity_rng) in reclaimed_bursts {
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_spell_replacements = self
            .spell_replacements
            .iter()
            .filter(|replacement| !replacement.active)
            .map(|replacement| (replacement.source_entity_index, replacement.entity_rng))
            .collect::<Vec<_>>();
        self.spell_replacements
            .retain(|replacement| replacement.active);
        for (slot, entity_rng) in reclaimed_spell_replacements {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_spell_pickups = self
            .spell_pickups
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.spell_pickups.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_spell_pickups {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model1 = self
            .authored_model1
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model1.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_model1 {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model45 = self
            .authored_model45
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model45.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_model45 {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model13 = self
            .authored_model13
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model13.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_model13 {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_bees = self
            .authored_bees
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_bees.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_bees {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_switch_producers = self
            .authored_switch_producers
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_switch_producers
            .retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_switch_producers {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_trees = self
            .authored_trees
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_trees.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_trees {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model32 = self
            .authored_model32
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model32.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_model32 {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model59 = self
            .authored_model59
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model59.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_model59 {
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model59_smoke_children = self
            .model59_smoke_children
            .iter()
            .filter(|child| !child.active)
            .map(|child| (child.source_entity_index, child.entity_rng))
            .collect::<Vec<_>>();
        self.model59_smoke_children.retain(|child| child.active);
        for (slot, entity_rng) in reclaimed_model59_smoke_children {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model53 = self
            .authored_model53
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model53.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_model53 {
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model5_water_splashes = self
            .authored_model5_water_splashes
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model5_water_splashes
            .retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_model5_water_splashes {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model13_children = self
            .model13_spawned_children
            .iter()
            .filter(|child| !child.active)
            .map(|child| (child.source_entity_index, child.entity_rng))
            .collect::<Vec<_>>();
        self.model13_spawned_children.retain(|child| child.active);
        for (slot, entity_rng) in reclaimed_model13_children {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model6 = self
            .authored_model6
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model6.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_model6 {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model6_children = self
            .model6_spawned_children
            .iter()
            .filter(|child| !child.active)
            .map(|child| (child.source_entity_index, child.entity_rng))
            .collect::<Vec<_>>();
        self.model6_spawned_children.retain(|child| child.active);
        for (slot, entity_rng) in reclaimed_model6_children {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model17 = self
            .authored_model17
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model17.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_model17 {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model17_children = self
            .model17_spawned_children
            .iter()
            .filter(|child| !child.active)
            .map(|child| (child.source_entity_index, child.entity_rng))
            .collect::<Vec<_>>();
        self.model17_spawned_children.retain(|child| child.active);
        for (slot, entity_rng) in reclaimed_model17_children {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model22 = self
            .authored_model22
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model22.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_model22 {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model22_children = self
            .model22_spawned_children
            .iter()
            .filter(|child| !child.active)
            .map(|child| (child.source_entity_index, child.entity_rng))
            .collect::<Vec<_>>();
        self.model22_spawned_children.retain(|child| child.active);
        for (slot, entity_rng) in reclaimed_model22_children {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model76 = self
            .authored_model76
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_model76.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_model76 {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model76_children = self
            .model76_children
            .iter()
            .filter(|child| !child.active)
            .map(|child| (child.source_entity_index, child.entity_rng))
            .collect::<Vec<_>>();
        self.model76_children.retain(|child| child.active);
        for (slot, entity_rng) in reclaimed_model76_children {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_model76_tails = self
            .model76_tail_spawns
            .iter()
            .filter(|tail| !tail.active)
            .map(|tail| (tail.source_entity_index, tail.entity_rng))
            .collect::<Vec<_>>();
        self.model76_tail_spawns.retain(|tail| tail.active);
        for (slot, entity_rng) in reclaimed_model76_tails {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_class14 = self
            .authored_class14
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_class14.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_class14 {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let inactive_class5_roots = self
            .authored_class5
            .iter()
            .filter(|source| !source.active)
            .map(|source| source.source_entity_index)
            .collect::<BTreeSet<_>>();
        for child in &mut self.authored_class5_model3_children {
            if inactive_class5_roots.contains(&child.parent_entity_index) {
                child.active = false;
            }
        }
        for child in &mut self.authored_class5_archer_children {
            if inactive_class5_roots.contains(&child.parent_entity_index) {
                child.active = false;
            }
        }
        let reclaimed_class5_archer_children = self
            .authored_class5_archer_children
            .iter()
            .filter(|child| !child.active)
            .map(|child| (child.source_entity_index, child.entity_rng))
            .collect::<Vec<_>>();
        self.authored_class5_archer_children
            .retain(|child| child.active);
        for (slot, entity_rng) in reclaimed_class5_archer_children {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_class5_model3_children = self
            .authored_class5_model3_children
            .iter()
            .filter(|child| !child.active)
            .map(|child| (child.source_entity_index, child.entity_rng))
            .collect::<Vec<_>>();
        self.authored_class5_model3_children
            .retain(|child| child.active);
        for (slot, entity_rng) in reclaimed_class5_model3_children {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_class5 = self
            .authored_class5
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.authored_class5.retain(|source| source.active);
        let live_class5_slots = self
            .authored_class5
            .iter()
            .map(|source| source.source_entity_index)
            .collect::<BTreeSet<_>>();
        self.authored_class5_runtime
            .retain(|state| live_class5_slots.contains(&state.source_entity_index));
        self.authored_class5_model3
            .retain(|state| live_class5_slots.contains(&state.source_entity_index));
        self.authored_class5_archer
            .retain(|state| live_class5_slots.contains(&state.source_entity_index));
        for (slot, entity_rng) in reclaimed_class5 {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_house_residents = self
            .house_residents
            .iter()
            .filter(|source| !source.active)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        self.house_residents.retain(|source| source.active);
        for (slot, entity_rng) in reclaimed_house_residents {
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_beyond_sight_effects = self
            .beyond_sight_effects
            .iter()
            .filter(|effect| !effect.active)
            .map(|effect| (effect.source_entity_index, effect.entity_rng))
            .collect::<Vec<_>>();
        self.beyond_sight_effects.retain(|effect| effect.active);
        for (slot, entity_rng) in reclaimed_beyond_sight_effects {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_mana_attraction = self
            .mana_attraction_fields
            .iter()
            .filter(|field| !field.active)
            .map(|field| (field.source_entity_index, field.entity_rng))
            .collect::<Vec<_>>();
        self.mana_attraction_fields.retain(|field| field.active);
        for (slot, entity_rng) in reclaimed_mana_attraction {
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(entity_rng));
        }
        let reclaimed_children = self
            .castle_children
            .iter()
            .filter(|child| !child.active)
            .map(|child| (child.source_entity_index, child.entity_rng))
            .collect::<Vec<_>>();
        self.castle_children.retain(|child| child.active);
        for (slot, entity_rng) in reclaimed_children {
            self.release_slot(slot, Some(entity_rng));
        }
        if let Some(castle) = self.castle
            && !castle.active
        {
            self.source_rng.remove(&castle.source_entity_index);
            self.release_slot(castle.source_entity_index, None);
            self.castle = None;
        }
        self.refresh_owner_mana_rows();
    }

    #[must_use]
    pub fn reset_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(REMC2_SPEC_COMMIT.as_bytes());
        hasher.update(self.terrain.deterministic_hash().to_le_bytes());
        hasher.update(self.global_rng.to_le_bytes());
        hasher.update(self.sentinel_rng.to_le_bytes());
        hasher.update(self.player_mana.to_le_bytes());
        hasher.update(self.player_castle_entity_index.to_le_bytes());
        hasher.update([self.player_castle_stage, self.player_castle_aux_flags]);
        hasher.update([self.player_model, self.player_terrain_flags]);
        hasher.update(self.player_spell_slot_2.to_le_bytes());
        hasher.update(self.player_spell_holder_action.to_le_bytes());
        hasher.update(self.player_spell_holder_max_action.to_le_bytes());
        for coefficient in self.castle_stage_coefficients {
            hasher.update(coefficient.to_le_bytes());
        }
        hasher.update(self.castle_controller_coefficients);
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
        for row in &self.owner_mana_rows {
            hasher.update(row.source_entity_index.to_le_bytes());
            hasher.update([row.source_class, row.source_model]);
            hasher.update(row.player_entity_index.to_le_bytes());
            hasher.update(row.mana.to_le_bytes());
            hasher.update([u8::from(row.active)]);
        }
        for (owner, mana) in &self.owner_mana_aggregate {
            hasher.update(owner.to_le_bytes());
            hasher.update(mana.to_le_bytes());
        }
        hasher.update(self.owner_mana_global.to_le_bytes());
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
            hasher.update(sphere.player_entity_index.to_le_bytes());
            hasher.update(sphere.incoming_owner_entity_index.to_le_bytes());
            hasher.update(sphere.incoming_possession_payload.to_le_bytes());
            hasher.update(sphere.attraction_source_entity_index.to_le_bytes());
            hasher.update(sphere.attraction_step.to_le_bytes());
            hasher.update(sphere.attraction_distance_high.to_le_bytes());
            hasher.update(sphere.assigned_carrier_id.to_le_bytes());
            hasher.update(sphere.assigned_defender_slot.to_le_bytes());
            hasher.update(sphere.control_flags.to_le_bytes());
            hasher.update([sphere.phase]);
            hasher.update([u8::from(sphere.active), u8::from(sphere.pending_delete)]);
        }
        for burst in &self.death_bursts {
            hasher.update(burst.source_entity_index.to_le_bytes());
            hasher.update(burst.entity_rng.to_le_bytes());
            hasher.update(burst.parent_entity_index.to_le_bytes());
            hasher.update(burst.id_entity_index.to_le_bytes());
            hasher.update(burst.position.x.to_le_bytes());
            hasher.update(burst.position.y.to_le_bytes());
            hasher.update(burst.position.z.to_le_bytes());
            hasher.update(burst.max_life.to_le_bytes());
            hasher.update(burst.life_ticks.to_le_bytes());
            hasher.update(burst.subspell_index.to_le_bytes());
            hasher.update(burst.control_flags.to_le_bytes());
            hasher.update([u8::from(burst.active)]);
        }
        for replacement in &self.spell_replacements {
            hasher.update(replacement.source_entity_index.to_le_bytes());
            hasher.update(replacement.entity_rng.to_le_bytes());
            hasher.update(replacement.parent_entity_index.to_le_bytes());
            hasher.update(replacement.position.x.to_le_bytes());
            hasher.update(replacement.position.y.to_le_bytes());
            hasher.update(replacement.position.z.to_le_bytes());
            hasher.update([replacement.family, replacement.action, replacement.phase]);
            hasher.update(replacement.control_flags.to_le_bytes());
            hasher.update([u8::from(replacement.active)]);
        }
        for source in &self.spell_pickups {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([source.family, source.action, source.phase]);
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update([u8::from(source.active)]);
        }
        hasher.update(b"authored-model1-v1");
        for source in &self.authored_model1 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([source.model, source.action, source.phase]);
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.particle_index.to_le_bytes());
            hasher.update(source.last_sound_id.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update([u8::from(source.drawing_enabled), u8::from(source.active)]);
        }
        for source in &self.authored_model45 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update(source.target_group.to_le_bytes());
            hasher.update(source.parameter_1.to_le_bytes());
            hasher.update([source.source_class, source.model, source.action]);
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.actual_speed.to_le_bytes());
            hasher.update(source.minimum_speed.to_le_bytes());
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.dword_0x10.to_le_bytes());
            hasher.update(source.yaw.to_le_bytes());
            hasher.update([source.font_type_index, source.source_byte_38]);
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.mana.to_le_bytes());
            hasher.update(source.max_mana.to_le_bytes());
            hasher.update(source.particle_index.to_le_bytes());
            hasher.update([source.particle_byte_0x5d]);
            hasher.update(source.view_pitch.to_le_bytes());
            hasher.update(source.view_yaw.to_le_bytes());
            hasher.update(source.view_fov.to_le_bytes());
            hasher.update(source.view_roll.to_le_bytes());
            hasher.update(source.axis_x.to_le_bytes());
            hasher.update(source.axis_z.to_le_bytes());
            hasher.update(source.byte_0x46.to_le_bytes());
            hasher.update(source.parent_entity_index.to_le_bytes());
            hasher.update([source.source_byte_3e]);
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update([u8::from(source.drawing_enabled), u8::from(source.active)]);
        }
        for source in &self.authored_model13 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([source.source_class, source.model, source.action]);
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.dword_0x10.to_le_bytes());
            hasher.update(source.particle_index.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.shift_rot_x.to_le_bytes());
            hasher.update(source.shift_rot_y.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update([
                source.phase,
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
        }
        for source in &self.authored_bees {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([
                source.source_class,
                source.model,
                source.action,
                source.phase,
                source.source_byte_3e,
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
            hasher.update(source.minimum_speed.to_le_bytes());
            hasher.update(source.maximum_speed.to_le_bytes());
            hasher.update(source.actual_speed.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.mana.to_le_bytes());
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.pending_damage.to_le_bytes());
            hasher.update(source.pending_attacker_entity_index.to_le_bytes());
            hasher.update(source.damage_attacker_entity_index.to_le_bytes());
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.dword_0x10.to_le_bytes());
            hasher.update(source.target_entity_index.unwrap_or(u16::MAX).to_le_bytes());
            hasher.update(source.yaw.to_le_bytes());
            hasher.update(source.pitch.to_le_bytes());
            hasher.update(source.roll.to_le_bytes());
            hasher.update(source.source_byte_39.to_le_bytes());
            hasher.update([source.stage_var1, source.stage_var2]);
            match source.stage_target {
                Some(target) => {
                    hasher.update([1]);
                    hasher.update(target.x.to_le_bytes());
                    hasher.update(target.y.to_le_bytes());
                    hasher.update(target.z.to_le_bytes());
                }
                None => hasher.update([0]),
            }
            hasher.update(source.vertical_step.to_le_bytes());
            hasher.update(source.action_window.to_le_bytes());
            hasher.update(source.last_sound_id.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update(source.particle_index.to_le_bytes());
            hasher.update(source.shift_rot_x.to_le_bytes());
            hasher.update(source.shift_rot_y.to_le_bytes());
        }
        hasher.update(b"level2-bee-model2-constructor-phase-v1");
        hasher.update([self.bee_model2_constructor_phase]);
        hasher.update(b"level2-authored-switch-producer-v1");
        for source in &self.authored_switch_producers {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([
                source.source_class,
                source.model,
                source.action,
                source.phase,
                source.cooldown,
                u8::from(source.fired),
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
            hasher.update(source.stage_tag.to_le_bytes());
            hasher.update(source.word_10.to_le_bytes());
            hasher.update(source.dword_0x10.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
        }
        hasher.update(b"level2-authored-tree-v1");
        for source in &self.authored_trees {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([
                source.source_class,
                source.model,
                source.action,
                source.source_byte_38,
                source.phase,
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
            hasher.update(source.dword_0x10.to_le_bytes());
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.owner_entity_index.to_le_bytes());
            hasher.update(source.pending_damage.to_le_bytes());
            hasher.update(source.particle_index.to_le_bytes());
            hasher.update(source.yaw.to_le_bytes());
            hasher.update(source.pitch.to_le_bytes());
            hasher.update(source.roll.to_le_bytes());
            hasher.update(source.fov.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update(source.last_sphere_source_entity_index.to_le_bytes());
            hasher.update(source.last_sphere_word_0x2c.to_le_bytes());
            hasher.update(source.last_sphere_position_z.to_le_bytes());
        }
        hasher.update(b"level2-authored-model32-v1");
        for source in &self.authored_model32 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([
                source.source_class,
                source.model,
                source.action,
                source.objective_stage_index,
                source.phase,
                source.source_byte_3e,
                u8::from(source.fired),
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
            hasher.update(source.stage_tag.to_le_bytes());
            hasher.update(source.word_10.to_le_bytes());
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
        }
        hasher.update(b"level2-authored-model59-v1");
        for source in &self.authored_model59 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([
                source.source_class,
                source.model,
                source.action,
                source.source_byte_3e,
                source.phase,
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.actual_speed.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
        }
        if !self.model59_smoke_children.is_empty() {
            hasher.update(b"level2-model59-smoke-child-v1");
            for child in &self.model59_smoke_children {
                hasher.update(child.source_entity_index.to_le_bytes());
                hasher.update(child.entity_rng.to_le_bytes());
                hasher.update([
                    child.source_class,
                    child.model,
                    child.action,
                    child.phase,
                    u8::from(child.drawing_enabled),
                    u8::from(child.active),
                ]);
                hasher.update(child.max_life.to_le_bytes());
                hasher.update(child.life.to_le_bytes());
                hasher.update(child.max_speed.to_le_bytes());
                hasher.update(child.actual_speed.to_le_bytes());
                hasher.update(child.dword_0x10.to_le_bytes());
                hasher.update(child.particle_index.to_le_bytes());
                hasher.update(child.yaw.to_le_bytes());
                hasher.update(child.control_flags.to_le_bytes());
                hasher.update(child.position.x.to_le_bytes());
                hasher.update(child.position.y.to_le_bytes());
                hasher.update(child.position.z.to_le_bytes());
            }
        }
        hasher.update(b"level2-authored-model53-v1");
        for source in &self.authored_model53 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([
                source.source_class,
                source.model,
                source.action,
                source.phase,
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.actual_speed.to_le_bytes());
            hasher.update(source.dword_0x10.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.shift_rot_x.to_le_bytes());
            hasher.update(source.shift_rot_y.to_le_bytes());
            hasher.update([source.source_byte_39 as u8]);
            hasher.update([
                source.byte_0x43,
                source.byte_0x44,
                source.byte_0x46,
                u8::from(source.water_spawn_requested),
            ]);
            hasher.update(source.word_0x2c.to_le_bytes());
            hasher.update(source.sprite_index.to_le_bytes());
            hasher.update(source.last_sound_id.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
        }
        hasher.update(b"level2-authored-model5-water-splash-v1");
        for source in &self.authored_model5_water_splashes {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update([
                source.source_class,
                source.model,
                source.action,
                source.phase,
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.dword_0x10.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.sprite_index.to_le_bytes());
            hasher.update(source.last_sound_id.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
        }
        for child in &self.model13_spawned_children {
            hasher.update(child.source_entity_index.to_le_bytes());
            hasher.update(child.entity_rng.to_le_bytes());
            hasher.update(child.parent_entity_index.to_le_bytes());
            hasher.update(child.parent_entity_id.to_le_bytes());
            hasher.update([child.source_class, child.model, child.action]);
            hasher.update(child.max_life.to_le_bytes());
            hasher.update(child.life.to_le_bytes());
            hasher.update(child.max_speed.to_le_bytes());
            hasher.update(child.actual_speed.to_le_bytes());
            hasher.update(child.dword_0x10.to_le_bytes());
            hasher.update(child.particle_index.to_le_bytes());
            hasher.update(child.entity_index.to_le_bytes());
            hasher.update([child.subtype]);
            hasher.update(child.yaw.to_le_bytes());
            hasher.update(child.control_flags.to_le_bytes());
            hasher.update(child.position.x.to_le_bytes());
            hasher.update(child.position.y.to_le_bytes());
            hasher.update(child.position.z.to_le_bytes());
            hasher.update([
                child.phase,
                u8::from(child.drawing_enabled),
                u8::from(child.active),
            ]);
        }
        for source in &self.authored_model6 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([source.model, source.action]);
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.height_offset.to_le_bytes());
            hasher.update(source.dword_0x10.to_le_bytes());
            hasher.update(source.particle_index.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.shift_rot_x.to_le_bytes());
            hasher.update(source.shift_rot_y.to_le_bytes());
            hasher.update(source.phase.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update([u8::from(source.active)]);
        }
        for child in &self.model6_spawned_children {
            hasher.update(child.source_entity_index.to_le_bytes());
            hasher.update(child.entity_rng.to_le_bytes());
            hasher.update(child.parent_entity_index.to_le_bytes());
            hasher.update(child.parent_entity_id.to_le_bytes());
            hasher.update([child.source_class, child.model, child.action]);
            hasher.update(child.max_life.to_le_bytes());
            hasher.update(child.life.to_le_bytes());
            hasher.update(child.actual_speed.to_le_bytes());
            hasher.update(child.min_speed.to_le_bytes());
            hasher.update(child.max_speed.to_le_bytes());
            hasher.update(child.yaw.to_le_bytes());
            hasher.update(child.height_offset.to_le_bytes());
            hasher.update(child.dword_0x10.to_le_bytes());
            hasher.update(child.particle_index.to_le_bytes());
            hasher.update(child.control_flags.to_le_bytes());
            hasher.update(child.position.x.to_le_bytes());
            hasher.update(child.position.y.to_le_bytes());
            hasher.update(child.position.z.to_le_bytes());
            hasher.update([u8::from(child.active)]);
        }
        for source in &self.authored_model17 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([source.source_class, source.model, source.action]);
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.dword_0x10.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.shift_rot_x.to_le_bytes());
            hasher.update(source.shift_rot_y.to_le_bytes());
            hasher.update(source.yaw.to_le_bytes());
            hasher.update(source.phase.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update([u8::from(source.active)]);
        }
        for child in &self.model17_spawned_children {
            hasher.update(child.source_entity_index.to_le_bytes());
            hasher.update(child.entity_rng.to_le_bytes());
            hasher.update(child.parent_entity_index.to_le_bytes());
            hasher.update(child.parent_entity_id.to_le_bytes());
            hasher.update([child.source_class, child.model, child.action]);
            hasher.update(child.subspell_index.to_le_bytes());
            hasher.update(child.max_life.to_le_bytes());
            hasher.update(child.life.to_le_bytes());
            hasher.update(child.dword_0x10.to_le_bytes());
            hasher.update(child.control_flags.to_le_bytes());
            hasher.update(child.shift_rot_x.to_le_bytes());
            hasher.update(child.shift_rot_y.to_le_bytes());
            hasher.update(child.yaw.to_le_bytes());
            hasher.update(child.phase.to_le_bytes());
            hasher.update(child.position.x.to_le_bytes());
            hasher.update(child.position.y.to_le_bytes());
            hasher.update(child.position.z.to_le_bytes());
            hasher.update([u8::from(child.active)]);
        }
        for source in &self.authored_model22 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([source.source_class, source.model, source.action]);
            hasher.update(source.word_0x2c.to_le_bytes());
            hasher.update(source.word_0x2e.to_le_bytes());
            hasher.update([source.byte_0x38]);
            hasher.update(source.actual_speed.to_le_bytes());
            hasher.update(source.minimum_speed.to_le_bytes());
            hasher.update(source.maximum_speed.to_le_bytes());
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.control_flags.to_le_bytes());
            hasher.update(source.word_0x30.to_le_bytes());
            hasher.update(source.yaw.to_le_bytes());
            hasher.update(source.pitch.to_le_bytes());
            hasher.update(source.roll.to_le_bytes());
            hasher.update(source.particle_index.to_le_bytes());
            hasher.update(source.shift_rot_x.to_le_bytes());
            hasher.update(source.shift_rot_y.to_le_bytes());
            hasher.update(source.axis_position.x.to_le_bytes());
            hasher.update(source.axis_position.y.to_le_bytes());
            hasher.update(source.axis_position.z.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update([source.phase, u8::from(source.active)]);
        }
        for child in &self.model22_spawned_children {
            hasher.update(child.source_entity_index.to_le_bytes());
            hasher.update(child.entity_id.to_le_bytes());
            hasher.update(child.entity_rng.to_le_bytes());
            hasher.update(child.max_life.to_le_bytes());
            hasher.update(child.life.to_le_bytes());
            hasher.update(child.parent_entity_index.to_le_bytes());
            hasher.update(child.parent_entity_id.to_le_bytes());
            hasher.update([child.source_class, child.model, child.action]);
            hasher.update([child.segment_index]);
            hasher.update(child.word_0x2c.to_le_bytes());
            hasher.update(child.word_0x36.to_le_bytes());
            hasher.update(child.yaw.to_le_bytes());
            hasher.update(child.pitch.to_le_bytes());
            hasher.update(child.roll.to_le_bytes());
            hasher.update(child.particle_index.to_le_bytes());
            hasher.update(child.shift_rot_x.to_le_bytes());
            hasher.update(child.shift_rot_y.to_le_bytes());
            hasher.update(child.position.x.to_le_bytes());
            hasher.update(child.position.y.to_le_bytes());
            hasher.update(child.position.z.to_le_bytes());
            hasher.update([
                child.phase,
                u8::from(child.drawing_enabled),
                u8::from(child.active),
            ]);
        }
        for source in &self.authored_model76 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_id.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([source.source_class, source.model, source.action]);
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.actual_speed.to_le_bytes());
            hasher.update(source.minimum_speed.to_le_bytes());
            hasher.update(source.maximum_speed.to_le_bytes());
            hasher.update(source.word_0x2c.to_le_bytes());
            hasher.update(source.font_type_index.to_le_bytes());
            hasher.update([source.state]);
            hasher.update(source.target_entity_index.to_le_bytes());
            hasher.update(source.target_position.x.to_le_bytes());
            hasher.update(source.target_position.y.to_le_bytes());
            hasher.update(source.target_position.z.to_le_bytes());
            hasher.update(source.target_pitch.to_le_bytes());
            hasher.update(source.target_height_offset.to_le_bytes());
            hasher.update(source.target_control_flags.to_le_bytes());
            hasher.update(source.target_life.to_le_bytes());
            hasher.update(source.shift_rot_x.to_le_bytes());
            hasher.update(source.shift_rot_y.to_le_bytes());
            hasher.update(source.yaw.to_le_bytes());
            hasher.update(source.pitch.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update([
                source.phase,
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
        }
        for child in &self.model76_children {
            hasher.update(child.source_entity_index.to_le_bytes());
            hasher.update(child.entity_id.to_le_bytes());
            hasher.update(child.entity_rng.to_le_bytes());
            hasher.update(child.parent_entity_index.to_le_bytes());
            hasher.update(child.parent_entity_id.to_le_bytes());
            hasher.update([child.source_class, child.model, child.action]);
            hasher.update(child.max_life.to_le_bytes());
            hasher.update(child.life.to_le_bytes());
            hasher.update(child.subspell_index.to_le_bytes());
            hasher.update(child.actual_speed.to_le_bytes());
            hasher.update(child.minimum_speed.to_le_bytes());
            hasher.update(child.maximum_speed.to_le_bytes());
            hasher.update(child.word_0x2c.to_le_bytes());
            hasher.update([child.row_index, child.column_index, child.chain_index]);
            hasher.update(child.yaw.to_le_bytes());
            hasher.update(child.pitch.to_le_bytes());
            hasher.update(child.roll.to_le_bytes());
            hasher.update(child.fov.to_le_bytes());
            hasher.update(child.collision_yaw.to_le_bytes());
            hasher.update(child.collision_pitch.to_le_bytes());
            hasher.update(child.collision_roll.to_le_bytes());
            hasher.update(child.collision_fov.to_le_bytes());
            hasher.update(child.shift_rot_x.to_le_bytes());
            hasher.update(child.shift_rot_y.to_le_bytes());
            hasher.update(child.position.x.to_le_bytes());
            hasher.update(child.position.y.to_le_bytes());
            hasher.update(child.position.z.to_le_bytes());
            hasher.update([
                child.phase,
                u8::from(child.drawing_enabled),
                u8::from(child.active),
            ]);
        }
        for tail in &self.model76_tail_spawns {
            hasher.update(tail.source_entity_index.to_le_bytes());
            hasher.update(tail.entity_rng.to_le_bytes());
            hasher.update(tail.parent_entity_index.to_le_bytes());
            hasher.update(tail.parent_entity_id.to_le_bytes());
            hasher.update([tail.source_class, tail.model, tail.action]);
            hasher.update(tail.subspell_index.to_le_bytes());
            hasher.update(tail.max_life.to_le_bytes());
            hasher.update(tail.life.to_le_bytes());
            hasher.update(tail.shift_rot_x.to_le_bytes());
            hasher.update(tail.shift_rot_y.to_le_bytes());
            hasher.update(tail.particle_index.to_le_bytes());
            hasher.update(tail.position.x.to_le_bytes());
            hasher.update(tail.position.y.to_le_bytes());
            hasher.update(tail.position.z.to_le_bytes());
            hasher.update([tail.phase]);
            hasher.update(tail.control_flags.to_le_bytes());
            hasher.update([u8::from(tail.active)]);
        }
        hasher.update(self.class14_model3_handle.to_le_bytes());
        for source in &self.authored_class14 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([source.model, source.action]);
            hasher.update(source.particle.to_le_bytes());
            hasher.update([source.control_flags]);
            hasher.update(source.shift_rot_x.to_le_bytes());
            hasher.update(source.shift_rot_y.to_le_bytes());
            hasher.update(source.activation_group.to_le_bytes());
            hasher.update(source.stage_tag.to_le_bytes());
            hasher.update(source.phase.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
            hasher.update([u8::from(source.active)]);
        }
        hasher.update(b"authored-class5-v4");
        hasher.update([
            self.class5_model3_constructor_phase,
            self.class5_model4_constructor_phase,
            self.class5_model9_constructor_phase,
            self.class5_model14_constructor_phase,
        ]);
        for source in &self.authored_class5 {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.authored_index.to_le_bytes());
            hasher.update([
                source.model,
                source.action,
                source.source_byte_3e,
                source.phase,
                u8::from(source.drawing_enabled),
                u8::from(source.active),
            ]);
            hasher.update(source.minimum_speed.to_le_bytes());
            hasher.update(source.maximum_speed.to_le_bytes());
            hasher.update(source.actual_speed.to_le_bytes());
            hasher.update(source.max_life.to_le_bytes());
            hasher.update(source.life.to_le_bytes());
            hasher.update(source.mana.to_le_bytes());
            hasher.update(source.max_mana.to_le_bytes());
            hasher.update(source.subspell_index.to_le_bytes());
            hasher.update(source.dword_0x10.to_le_bytes());
            hasher.update(source.pending_word_0x62.to_le_bytes());
            hasher.update(source.word_0x26.to_le_bytes());
            hasher.update(source.word_0x24.to_le_bytes());
            hasher.update([source.byte_0x3b]);
            hasher.update(source.word_0x96_150.to_le_bytes());
            hasher.update([source.byte_0x46_70]);
            hasher.update(source.particle_index.to_le_bytes());
            hasher.update(source.source_byte_39.to_le_bytes());
            hasher.update(source.yaw.to_le_bytes());
            hasher.update(source.pitch.to_le_bytes());
            hasher.update(source.roll.to_le_bytes());
            hasher.update(source.fov.to_le_bytes());
            hasher.update(source.shift_rot_x.to_le_bytes());
            hasher.update(source.shift_rot_y.to_le_bytes());
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
        }
        hasher.update(b"authored-class5-model3-v1");
        for state in &self.authored_class5_model3 {
            hasher.update(state.source_entity_index.to_le_bytes());
            hasher.update(state.damage_dword_0x5e_94.to_le_bytes());
            hasher.update(state.word_0x32.to_le_bytes());
            hasher.update(state.word_0x34.to_le_bytes());
            hasher.update(state.word_0x36.to_le_bytes());
            hasher.update(state.control_flags.to_le_bytes());
            hasher.update([state.byte_0x43, state.byte_0x44]);
            hasher.update(state.word_0x2c.to_le_bytes());
            hasher.update(state.descriptor_index.to_le_bytes());
            hasher.update([state.last_child_root_phase]);
        }
        for child in &self.authored_class5_model3_children {
            hasher.update(child.source_entity_index.to_le_bytes());
            hasher.update(child.entity_rng.to_le_bytes());
            hasher.update(child.parent_entity_index.to_le_bytes());
            hasher.update(child.word_0x32.to_le_bytes());
            hasher.update(child.word_0x34.to_le_bytes());
            hasher.update(child.word_0x36.to_le_bytes());
            hasher.update(child.word_0x24.to_le_bytes());
            hasher.update(child.pending_word_0x62.to_le_bytes());
            hasher.update(child.word_0x26.to_le_bytes());
            hasher.update(child.word_0x96.to_le_bytes());
            hasher.update(child.damage_dword_0x5e_94.to_le_bytes());
            hasher.update(child.dword_0x10.to_le_bytes());
            hasher.update(child.minimum_speed.to_le_bytes());
            hasher.update(child.maximum_speed.to_le_bytes());
            hasher.update(child.actual_speed.to_le_bytes());
            hasher.update(child.max_life.to_le_bytes());
            hasher.update(child.life.to_le_bytes());
            hasher.update(child.mana.to_le_bytes());
            hasher.update(child.control_flags.to_le_bytes());
            hasher.update([
                child.model,
                child.action,
                child.source_byte_3e,
                child.byte_0x3b,
                child.byte_0x43,
                child.byte_0x44,
                child.byte_0x46,
            ]);
            hasher.update(child.word_0x2c.to_le_bytes());
            hasher.update(child.particle_index.to_le_bytes());
            hasher.update(child.yaw.to_le_bytes());
            hasher.update(child.pitch.to_le_bytes());
            hasher.update(child.roll.to_le_bytes());
            hasher.update(child.fov.to_le_bytes());
            hasher.update(child.shift_rot_x.to_le_bytes());
            hasher.update(child.shift_rot_y.to_le_bytes());
            hasher.update(child.position.x.to_le_bytes());
            hasher.update(child.position.y.to_le_bytes());
            hasher.update(child.position.z.to_le_bytes());
            hasher.update([
                child.phase,
                u8::from(child.drawing_enabled),
                u8::from(child.active),
            ]);
        }
        hasher.update(b"authored-class5-archer-v1");
        for state in &self.authored_class5_archer {
            hasher.update(state.source_entity_index.to_le_bytes());
            hasher.update(state.slot_generation.to_le_bytes());
            hasher.update(state.word_0x32.to_le_bytes());
            hasher.update(state.word_0x34.to_le_bytes());
            hasher.update(state.descriptor_index.to_le_bytes());
            hasher.update([state.byte_0x38, state.xtype, state.last_child_root_phase]);
            hasher.update(state.pending_damage.to_le_bytes());
        }
        for child in &self.authored_class5_archer_children {
            hasher.update(child.source_entity_index.to_le_bytes());
            hasher.update(child.entity_id.to_le_bytes());
            hasher.update(child.entity_rng.to_le_bytes());
            hasher.update(child.parent_entity_index.to_le_bytes());
            hasher.update(child.parent_entity_id.to_le_bytes());
            hasher.update(child.slot_generation.to_le_bytes());
            hasher.update(child.word_0x32.to_le_bytes());
            hasher.update(child.word_0x34.to_le_bytes());
            hasher.update(child.word_0x24.to_le_bytes());
            hasher.update(child.pending_word_0x62.to_le_bytes());
            hasher.update(child.word_0x26.to_le_bytes());
            hasher.update(child.word_0x96.to_le_bytes());
            hasher.update(child.dword_0x10.to_le_bytes());
            hasher.update([
                child.source_class,
                child.model,
                child.action,
                child.xtype,
                child.xsubtype,
                child.source_byte_3e,
                child.phase,
                u8::from(child.drawing_enabled),
                u8::from(child.active),
            ]);
            hasher.update(child.minimum_speed.to_le_bytes());
            hasher.update(child.maximum_speed.to_le_bytes());
            hasher.update(child.actual_speed.to_le_bytes());
            hasher.update(child.max_life.to_le_bytes());
            hasher.update(child.life.to_le_bytes());
            hasher.update(child.subspell_index.to_le_bytes());
            hasher.update(child.particle_index.to_le_bytes());
            hasher.update(child.control_flags.to_le_bytes());
            hasher.update(child.yaw.to_le_bytes());
            hasher.update(child.pitch.to_le_bytes());
            hasher.update(child.roll.to_le_bytes());
            hasher.update(child.fov.to_le_bytes());
            hasher.update(child.shift_rot_x.to_le_bytes());
            hasher.update(child.shift_rot_y.to_le_bytes());
            hasher.update(child.position.x.to_le_bytes());
            hasher.update(child.position.y.to_le_bytes());
            hasher.update(child.position.z.to_le_bytes());
        }
        for state in &self.authored_class5_runtime {
            hasher.update(state.source_entity_index.to_le_bytes());
            hasher.update([state.byte_0x38, state.xtype]);
            hasher.update(state.descriptor_index.to_le_bytes());
            hasher.update(state.pending_damage.to_le_bytes());
            hasher.update(state.linked_head.to_le_bytes());
        }
        hasher.update(b"level2-house-resident-v1");
        for source in &self.house_residents {
            hasher.update(source.source_entity_index.to_le_bytes());
            hasher.update(source.entity_rng.to_le_bytes());
            hasher.update(source.parent_house_entity_index.to_le_bytes());
            hasher.update(source.owner_entity_index.to_le_bytes());
            hasher.update(source.attacker_entity_index.to_le_bytes());
            hasher.update([
                source.model,
                source.action,
                source.phase,
                u8::from(source.active),
            ]);
            hasher.update(source.position.x.to_le_bytes());
            hasher.update(source.position.y.to_le_bytes());
            hasher.update(source.position.z.to_le_bytes());
        }
        for effect in &self.beyond_sight_effects {
            hasher.update(effect.source_entity_index.to_le_bytes());
            hasher.update(effect.entity_rng.to_le_bytes());
            hasher.update(effect.holder_entity_index.to_le_bytes());
            hasher.update(effect.parent_entity_index.to_le_bytes());
            hasher.update(effect.parent_entity_id.to_le_bytes());
            hasher.update([effect.source_class, effect.model, effect.origin_subtype]);
            hasher.update(effect.subspell_index.to_le_bytes());
            hasher.update(effect.life.to_le_bytes());
            hasher.update(effect.effect_parameter.to_le_bytes());
            hasher.update(effect.actual_speed.to_le_bytes());
            hasher.update(effect.position.x.to_le_bytes());
            hasher.update(effect.position.y.to_le_bytes());
            hasher.update(effect.position.z.to_le_bytes());
            hasher.update(effect.axis_position.x.to_le_bytes());
            hasher.update(effect.axis_position.y.to_le_bytes());
            hasher.update(effect.axis_position.z.to_le_bytes());
            hasher.update(effect.yaw.to_le_bytes());
            hasher.update(effect.pitch.to_le_bytes());
            hasher.update([effect.sound_id, u8::from(effect.active)]);
        }
        for field in &self.mana_attraction_fields {
            hasher.update(field.source_entity_index.to_le_bytes());
            hasher.update(field.entity_rng.to_le_bytes());
            hasher.update(field.authored_index.to_le_bytes());
            hasher.update(field.position.x.to_le_bytes());
            hasher.update(field.position.y.to_le_bytes());
            hasher.update(field.position.z.to_le_bytes());
            hasher.update(field.radius_squared.to_le_bytes());
            hasher.update(field.max_life.to_le_bytes());
            hasher.update(field.life.to_le_bytes());
            hasher.update([field.action, u8::from(field.active)]);
        }
        for projectile in &self.fireball_projectiles {
            hasher.update(projectile.source_entity_index.to_le_bytes());
            hasher.update(projectile.entity_rng.to_le_bytes());
            hasher.update(projectile.owner_entity_index.to_le_bytes());
            hasher.update(projectile.holder_entity_index.to_le_bytes());
            hasher.update(projectile.source_subspell_index.to_le_bytes());
            hasher.update(projectile.source_mana.to_le_bytes());
            hasher.update([
                projectile.source_class,
                projectile.source_model,
                projectile.action,
            ]);
            hasher.update(projectile.payload.to_le_bytes());
            hasher.update(projectile.position.x.to_le_bytes());
            hasher.update(projectile.position.y.to_le_bytes());
            hasher.update(projectile.position.z.to_le_bytes());
            hasher.update(projectile.axis_position.x.to_le_bytes());
            hasher.update(projectile.axis_position.y.to_le_bytes());
            hasher.update(projectile.axis_position.z.to_le_bytes());
            hasher.update(projectile.yaw.to_le_bytes());
            hasher.update(projectile.pitch.to_le_bytes());
            hasher.update(projectile.actual_speed.to_le_bytes());
            hasher.update(projectile.minimum_speed.to_le_bytes());
            hasher.update(projectile.life.to_le_bytes());
            hasher.update(projectile.max_life.to_le_bytes());
            hasher.update(projectile.dword_0x10.to_le_bytes());
            hasher.update(projectile.control_flags.to_le_bytes());
            hasher.update(projectile.source_word_2c.to_le_bytes());
            hasher.update([
                projectile.phase,
                u8::from(projectile.active),
                u8::from(projectile.drawing_disabled),
            ]);
        }
        hasher.update(b"level2-possession-projectile-v2");
        for projectile in &self.possession_projectiles {
            hasher.update(projectile.source_entity_index.to_le_bytes());
            hasher.update(projectile.entity_rng.to_le_bytes());
            hasher.update(projectile.source_id_entity_index.to_le_bytes());
            hasher.update(projectile.holder_entity_index.to_le_bytes());
            hasher.update(projectile.target_source_entity_index.to_le_bytes());
            hasher.update([
                projectile.source_class,
                projectile.source_model,
                projectile.action,
                projectile.output_class,
                projectile.output_model,
                projectile.phase,
                u8::from(projectile.acquisition_attempted),
                u8::from(projectile.active),
            ]);
            hasher.update(projectile.position.x.to_le_bytes());
            hasher.update(projectile.position.y.to_le_bytes());
            hasher.update(projectile.position.z.to_le_bytes());
            hasher.update(projectile.axis_position.x.to_le_bytes());
            hasher.update(projectile.axis_position.y.to_le_bytes());
            hasher.update(projectile.axis_position.z.to_le_bytes());
            hasher.update(projectile.yaw.to_le_bytes());
            hasher.update(projectile.pitch.to_le_bytes());
            hasher.update(projectile.roll.to_le_bytes());
            hasher.update(projectile.fov.to_le_bytes());
            hasher.update(projectile.actual_speed.to_le_bytes());
            hasher.update(projectile.minimum_speed.to_le_bytes());
            hasher.update(projectile.life.to_le_bytes());
            hasher.update(projectile.max_life.to_le_bytes());
        }
        hasher.update(b"level2-possession-guidance-source-slots-v1");
        for source_slot in &self.possession_guidance_source_slots {
            hasher.update(source_slot.to_le_bytes());
        }
        hasher.update(b"level2-possession-guidance-stale-records-v1");
        for (source_slot, position) in &self.possession_guidance_stale_records {
            hasher.update(source_slot.to_le_bytes());
            hasher.update(position.x.to_le_bytes());
            hasher.update(position.y.to_le_bytes());
            hasher.update(position.z.to_le_bytes());
        }
        hasher.update(b"level2-possession-effect-v1");
        for effect in &self.possession_effects {
            hasher.update(effect.source_entity_index.to_le_bytes());
            hasher.update(effect.entity_rng.to_le_bytes());
            hasher.update(effect.source_id_entity_index.to_le_bytes());
            hasher.update(effect.projectile_entity_index.to_le_bytes());
            hasher.update([
                effect.source_class,
                effect.source_model,
                effect.action,
                effect.phase,
                u8::from(effect.active),
            ]);
            hasher.update(effect.position.x.to_le_bytes());
            hasher.update(effect.position.y.to_le_bytes());
            hasher.update(effect.position.z.to_le_bytes());
            hasher.update(effect.yaw.to_le_bytes());
            hasher.update(effect.pitch.to_le_bytes());
            hasher.update(effect.life.to_le_bytes());
            hasher.update(effect.max_life.to_le_bytes());
        }
        if self.carriers.is_empty() {
            hasher.update([0]);
        }
        for carrier in &self.carriers {
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
        }
        for projectile in &self.castle_projectiles {
            hasher.update(projectile.source_entity_index.to_le_bytes());
            hasher.update(projectile.entity_rng.to_le_bytes());
            hasher.update(projectile.owner_entity_index.to_le_bytes());
            hasher.update(projectile.holder_entity_index.to_le_bytes());
            hasher.update(projectile.source_subspell_index.to_le_bytes());
            hasher.update(projectile.source_mana.to_le_bytes());
            hasher.update([projectile.source_a4_byte_154]);
            hasher.update(projectile.position.x.to_le_bytes());
            hasher.update(projectile.position.y.to_le_bytes());
            hasher.update(projectile.position.z.to_le_bytes());
            hasher.update(projectile.axis_position.x.to_le_bytes());
            hasher.update(projectile.axis_position.y.to_le_bytes());
            hasher.update(projectile.axis_position.z.to_le_bytes());
            hasher.update(projectile.target_position.x.to_le_bytes());
            hasher.update(projectile.target_position.y.to_le_bytes());
            hasher.update(projectile.target_position.z.to_le_bytes());
            hasher.update(projectile.target_anchor.x.to_le_bytes());
            hasher.update(projectile.target_anchor.y.to_le_bytes());
            hasher.update(projectile.target_anchor.z.to_le_bytes());
            hasher.update([
                projectile.source_class,
                projectile.source_model,
                projectile.source_byte_43,
                projectile.source_byte_44,
                u8::from(projectile.site_rejected),
                u8::from(projectile.active),
                u8::from(projectile.drawing_disabled),
            ]);
            hasher.update(projectile.source_word_96.to_le_bytes());
            hasher.update(projectile.target_castle_slot.to_le_bytes());
            hasher.update(projectile.yaw.to_le_bytes());
            hasher.update(projectile.pitch.to_le_bytes());
            hasher.update(projectile.actual_speed.to_le_bytes());
            hasher.update(projectile.minimum_speed.to_le_bytes());
            hasher.update(projectile.life.to_le_bytes());
            hasher.update(projectile.max_life.to_le_bytes());
            hasher.update(projectile.control_flags.to_le_bytes());
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
            hasher.update(castle.shift_pitch.to_le_bytes());
            hasher.update(castle.shift_roll.to_le_bytes());
            hasher.update(castle.shift_yaw.to_le_bytes());
            hasher.update(castle.shift_fov.to_le_bytes());
            hasher.update(castle.source_word_2e.to_le_bytes());
            hasher.update(castle.source_word_30.to_le_bytes());
            hasher.update(castle.source_word_7c.to_le_bytes());
            hasher.update(castle.source_word_80.to_le_bytes());
            hasher.update([u8::from(castle.upgrade_holder_clear_pending)]);
            for slot in castle.defender_slots {
                hasher.update(slot.to_le_bytes());
            }
            hasher.update([castle.defender_count]);
            hasher.update(castle.defender_total_mana.to_le_bytes());
            hasher.update(castle.defender_total_max_mana.to_le_bytes());
            for slot in castle.defender_target_spheres {
                hasher.update(slot.to_le_bytes());
            }
            for slot in castle.auxiliary_slots {
                hasher.update(slot.to_le_bytes());
            }
            hasher.update([castle.auxiliary_count, castle.auxiliary_cooldown]);
            for position in castle.auxiliary_positions {
                hasher.update(position.x.to_le_bytes());
                hasher.update(position.y.to_le_bytes());
                hasher.update(position.z.to_le_bytes());
            }
            for yaw in castle.auxiliary_yaws {
                hasher.update(yaw.to_le_bytes());
            }
            for roll in castle.auxiliary_rolls {
                hasher.update(roll.to_le_bytes());
            }
            for player in castle.auxiliary_player_entity_indices {
                hasher.update(player.to_le_bytes());
            }
        } else {
            hasher.update([0]);
        }
        for child in &self.castle_children {
            hasher.update(child.source_entity_index.to_le_bytes());
            hasher.update(child.entity_rng.to_le_bytes());
            hasher.update(child.id_entity_index.to_le_bytes());
            hasher.update([child.source_class]);
            hasher.update(child.parent_entity_index.to_le_bytes());
            hasher.update(child.owner_entity_index.to_le_bytes());
            hasher.update(child.player_entity_index.to_le_bytes());
            hasher.update(child.position.x.to_le_bytes());
            hasher.update(child.position.y.to_le_bytes());
            hasher.update(child.position.z.to_le_bytes());
            hasher.update(child.axis_position.x.to_le_bytes());
            hasher.update(child.axis_position.y.to_le_bytes());
            hasher.update(child.axis_position.z.to_le_bytes());
            hasher.update([child.model, child.action, u8::from(child.active)]);
            hasher.update(child.mana.to_le_bytes());
            hasher.update(child.max_mana.to_le_bytes());
            hasher.update(child.life.to_le_bytes());
            hasher.update(child.max_life.to_le_bytes());
            hasher.update(child.speed.to_le_bytes());
            hasher.update(child.target_sphere_slot.to_le_bytes());
            hasher.update([
                child.source_byte_3e,
                child.source_byte_3b,
                child.cave_collision_cooldown,
            ]);
            hasher.update(child.source_dword_10.to_le_bytes());
            hasher.update(child.source_word_2e.to_le_bytes());
            hasher.update(child.construction_target_height.to_le_bytes());
            hasher.update(child.source_subspell_index.to_le_bytes());
            hasher.update(child.yaw.to_le_bytes());
            hasher.update(child.roll.to_le_bytes());
            hasher.update(child.control_flags.to_le_bytes());
            hasher.update(child.chain_previous_slot.to_le_bytes());
            hasher.update(child.chain_next_slot.to_le_bytes());
            hasher.update([child.source_byte_43]);
            hasher.update(child.source_word_4a.to_le_bytes());
        }
        if let Some(footprint) = &self.castle_footprint {
            hasher.update([1, footprint.index, footprint.width, footprint.height]);
            for cell in footprint.cells() {
                hasher.update(cell);
            }
        } else {
            hasher.update([0]);
        }
        for (stage, footprint) in &self.castle_stage_footprints {
            hasher.update([*stage, footprint.index, footprint.width, footprint.height]);
            for cell in footprint.cells() {
                hasher.update(cell);
            }
        }
        hasher.update([0xff]);
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
        self.slot_generations = self.baseline_slot_generations.clone();
        self.slot_rng = self.baseline_slot_rng.clone();
        self.global_rng = self.baseline_global_rng;
        self.sentinel_rng = self.baseline_sentinel_rng;
        self.source_rng = self.baseline_source_rng.clone();
        self.source_owner = self.baseline_source_owner.clone();
        self.owner_class = self.baseline_owner_class.clone();
        self.owner_max_mana = self.baseline_owner_max_mana.clone();
        self.owner_mana_rows = self.baseline_owner_mana_rows.clone();
        self.owner_mana_aggregate = self.baseline_owner_mana_aggregate.clone();
        self.owner_mana_global = self.baseline_owner_mana_global;
        self.spheres = self.baseline_spheres.clone();
        self.death_bursts = self.baseline_death_bursts.clone();
        self.spell_replacements = self.baseline_spell_replacements.clone();
        self.spell_pickups = self.baseline_spell_pickups.clone();
        self.authored_model1 = self.baseline_authored_model1.clone();
        self.authored_model45 = self.baseline_authored_model45.clone();
        self.authored_model13 = self.baseline_authored_model13.clone();
        self.authored_bees = self.baseline_authored_bees.clone();
        self.bee_model2_constructor_phase = self.baseline_bee_model2_constructor_phase;
        self.authored_switch_producers = self.baseline_authored_switch_producers.clone();
        self.authored_trees = self.baseline_authored_trees.clone();
        self.authored_model32 = self.baseline_authored_model32.clone();
        self.authored_model59 = self.baseline_authored_model59.clone();
        self.model59_smoke_children = self.baseline_model59_smoke_children.clone();
        self.authored_model53 = self.baseline_authored_model53.clone();
        self.authored_model5_water_splashes = self.baseline_authored_model5_water_splashes.clone();
        self.model13_spawned_children = self.baseline_model13_spawned_children.clone();
        self.authored_model6 = self.baseline_authored_model6.clone();
        self.model6_spawned_children = self.baseline_model6_spawned_children.clone();
        self.authored_model17 = self.baseline_authored_model17.clone();
        self.model17_spawned_children = self.baseline_model17_spawned_children.clone();
        self.authored_model22 = self.baseline_authored_model22.clone();
        self.model22_spawned_children = self.baseline_model22_spawned_children.clone();
        self.authored_model76 = self.baseline_authored_model76.clone();
        self.model76_children = self.baseline_model76_children.clone();
        self.model76_tail_spawns = self.baseline_model76_tail_spawns.clone();
        self.authored_class14 = self.baseline_authored_class14.clone();
        self.class14_model3_handle = self.baseline_class14_model3_handle;
        self.authored_class5 = self.baseline_authored_class5.clone();
        self.authored_class5_runtime = self.baseline_authored_class5_runtime.clone();
        self.authored_class5_model3 = self.baseline_authored_class5_model3.clone();
        self.authored_class5_model3_children =
            self.baseline_authored_class5_model3_children.clone();
        self.authored_class5_archer = self.baseline_authored_class5_archer.clone();
        self.authored_class5_archer_children =
            self.baseline_authored_class5_archer_children.clone();
        self.class5_model3_constructor_phase = self.baseline_class5_model3_constructor_phase;
        self.class5_model4_constructor_phase = self.baseline_class5_model4_constructor_phase;
        self.class5_model9_constructor_phase = self.baseline_class5_model9_constructor_phase;
        self.class5_model14_constructor_phase = self.baseline_class5_model14_constructor_phase;
        self.house_residents = self.baseline_house_residents.clone();
        self.beyond_sight_effects = self.baseline_beyond_sight_effects.clone();
        self.mana_attraction_fields = self.baseline_mana_attraction_fields.clone();
        self.possession_projectiles = self.baseline_possession_projectiles.clone();
        self.possession_guidance_source_slots =
            self.baseline_possession_guidance_source_slots.clone();
        self.possession_guidance_stale_records =
            self.baseline_possession_guidance_stale_records.clone();
        self.possession_effects = self.baseline_possession_effects.clone();
        self.fireball_projectiles.clear();
        self.carriers = self.baseline_carriers.clone();
        self.castle_projectiles = self.baseline_castle_projectiles.clone();
        self.castle = self.baseline_castle;
        self.castle_children = self.baseline_castle_children.clone();
        self.castle_footprint = self.baseline_castle_footprint.clone();
        self.castle_stage_footprints = self.baseline_castle_stage_footprints.clone();
        self.map_chains = self.baseline_map_chains.clone();
        self.player_mana = self.baseline_player_mana;
        self.player_castle_entity_index = self.baseline_player_castle_entity_index;
        self.player_castle_stage = self.baseline_player_castle_stage;
        self.player_castle_aux_flags = self.baseline_player_castle_aux_flags;
        self.player_model = self.baseline_player_model;
        self.player_terrain_flags = self.baseline_player_terrain_flags;
        self.player_spell_slot_2 = self.baseline_player_spell_slot_2;
        self.player_spell_holder_action = self.baseline_player_spell_holder_action;
        self.player_spell_holder_max_action = self.baseline_player_spell_holder_max_action;
        self.castle_stage_coefficients = self.baseline_castle_stage_coefficients;
        self.castle_controller_coefficients = self.baseline_castle_controller_coefficients;
        self.refresh_owner_mana_rows();
        self.publish_owner_mana_projection();
        self.rebuild_owner_max_mana_projection();
        self.events.clear();
    }
}

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

impl Level2SharedSpellResetSession {
    fn from_parts_with_starting_spells(
        terrain: SessionTerrain,
        entities: &[EntitySnapshot],
        players: &[PlayerSnapshot],
        presentation: &PresentationSelectors,
        starting_spells: Option<(&mut Level2EconomyState, &MissionWizardMapSettings)>,
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
        let player_entities = players
            .iter()
            .map(|player| {
                entities
                    .get(usize::from(player.entity_index))
                    .cloned()
                    .ok_or_else(|| {
                        NativeWorldError::new(format!(
                            "Level 2 spell player entity {} is absent",
                            player.entity_index
                        ))
                    })
            })
            .collect::<Result<Vec<_>>>()?
            .into_boxed_slice();
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
        let acquisitions = LEVEL2_GENERIC_SPELL_PICKUPS
            .into_iter()
            .map(|(family, source_slot, source_action)| {
                let family_index = usize::from(family);
                let mut state = SpellHolderAcquisitionState::new(
                    family,
                    source_slot,
                    player_entity.index,
                    source_action,
                    local_player.family_selected_tiers[family_index],
                );
                state.holder_relation = local_player.enabled_spell_holders[family_index];
                state.eligibility = local_player.spell_hand_eligibility[family_index];
                state.learned = local_player.learned_spell_flags[family_index];
                state.left_spell_slot = local_player.left_spell_slot_raw;
                state.right_spell_slot = local_player.right_spell_slot_raw;
                state.left_subspell_index = local_player.left_subspell_index;
                state.right_subspell_index = local_player.right_subspell_index;
                if let Some(source) = entities.get(usize::from(source_slot)) {
                    state.subspell_index = source.subspell_index;
                    state.word_0x30 = source.source_spell_word_30();
                    state.byte_0x3b = source.byte_3b;
                    state.byte_0x3c = source.byte_3c;
                    state.font_type_index = source.font_type_index;
                    state.mana_regen = source.mana_regen;
                    state.max_mana = source.max_mana;
                    state.mana = source.mana;
                    state.source_life = source.life;
                    state.source_draw_enabled =
                        source.control_flags & ENTITY_DRAW_DISABLED_BIT == 0;
                    state.return_side = source.source_word_4a();
                }
                state
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let mut session = Self {
            baseline_terrain: terrain.clone(),
            terrain,
            baseline_pickups: pickups.clone(),
            pickups,
            baseline_dynamic_pickups: Vec::new(),
            dynamic_pickups: Vec::new(),
            baseline_authored_pickup_slots: BTreeMap::new(),
            authored_pickup_slots: BTreeMap::new(),
            baseline_holders: holders.clone(),
            holders,
            baseline_player_mana: player_entity.mana,
            player_mana: player_entity.mana,
            baseline_player_life_regen: 0,
            player_life_regen: 0,
            baseline_player_entity: player_entity.clone(),
            player_entity: player_entity.clone(),
            baseline_players: players.to_vec().into_boxed_slice(),
            players: players.to_vec().into_boxed_slice(),
            baseline_player_entities: player_entities.clone(),
            player_entities,
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
            baseline_shield: None,
            shield: None,
            baseline_invisible: None,
            invisible: None,
            baseline_beyond_sight: None,
            beyond_sight: None,
            baseline_shield_damage_mailbox: None,
            shield_damage_mailbox: None,
            baseline_beyond_sight_auxiliary: Level2PlayerEventAuxiliary {
                entity_index: player_entity.index,
                ..Level2PlayerEventAuxiliary::default()
            },
            beyond_sight_auxiliary: Level2PlayerEventAuxiliary {
                entity_index: player_entity.index,
                ..Level2PlayerEventAuxiliary::default()
            },
            baseline_acquisitions: acquisitions.clone(),
            acquisitions,
            baseline_free_spell_usage: false,
            free_spell_usage: false,
            baseline_persistent_spell_experience: local_player.persistent_spell_experience,
            persistent_spell_experience: local_player.persistent_spell_experience,
            baseline_spell_experience: local_player.runtime_spell_experience,
            spell_experience: local_player.runtime_spell_experience,
            baseline_spell_levels: local_player.spell_unlocked_levels,
            spell_levels: local_player.spell_unlocked_levels,
            baseline_persistent_acquisitions: BTreeMap::new(),
            persistent_acquisitions: BTreeMap::new(),
            player_entity_sha256: LEVEL2_SHARED_SPELL_PLAYER_ENTITY_SHA256.to_owned(),
            events: Vec::new(),
            pickup_events: Vec::new(),
        };
        if let Some((economy, wizard)) = starting_spells {
            session.materialize_direct_starting_spells(economy, wizard)?;
            economy.seal_direct_starting_spell_baseline();
        }
        Ok(session)
    }

    fn materialize_direct_starting_spells(
        &mut self,
        economy: &mut Level2EconomyState,
        wizard: &MissionWizardMapSettings,
    ) -> Result<()> {
        let economy_before = economy.clone();
        let template = self.holders.first().cloned().ok_or_else(|| {
            NativeWorldError::new("Level 2 shared spell holder template is absent")
        })?;
        let player_entity_index = self.player_entity.index;
        let local_player_index = self
            .players
            .iter()
            .position(|player| player.entity_index == player_entity_index)
            .ok_or_else(|| NativeWorldError::new("Level 2 local player row is absent"))?;
        let mut rows = Vec::new();
        for family in 0_u8..=25 {
            let index = usize::from(family);
            if wizard.starting_spells[index] == 0 || wizard.blocked_spells[index] != 0 {
                continue;
            }
            let tier = wizard.spell_levels[index].min(2);
            let Some(profile) = level2_direct_starting_spell_profile(family, tier) else {
                *economy = economy_before;
                return Err(NativeWorldError::new(format!(
                    "Level 2 direct starting family {family} has no bounded source profile"
                )));
            };
            let source = economy
                .allocate_starting_spell_holder(family, self.player_entity.position)
                .ok_or_else(|| {
                    *economy = economy_before.clone();
                    NativeWorldError::new(format!(
                        "Level 2 direct starting family {family} could not allocate a holder"
                    ))
                })?;
            let mut holder = template.clone();
            holder.index = source.source_entity_index;
            holder.source_id = source.source_entity_index;
            holder.authority = EntityAuthority::SpellActivationLive;
            holder.active = source.active;
            holder.max_life = 0;
            holder.life = 0;
            holder.class = 15;
            holder.model = source.family;
            holder.control_flags = 5;
            holder.entity_rng = source.entity_rng;
            holder.action = source.action;
            holder.action_marker = 0;
            holder.level_gate = tier.cast_signed();
            holder.position = source.position;
            holder.yaw = 75;
            holder.pitch = 768;
            holder.fov = 1280;
            holder.phase = source.phase;
            holder.subspell_index = profile.subspell_index;
            holder.byte_3b = profile.byte_0x3b;
            holder.byte_3c = 0;
            holder.font_type_index = 0;
            holder.active_value = 0;
            holder.activation_value = profile.word_0x30;
            holder.mana_regen = profile.mana_regen;
            holder.max_mana = profile.max_mana;
            holder.mana_cost = profile.max_mana;
            holder.mana = profile.mana;
            holder.particle_parameter_index = Some(77);
            holder.parameter_relation = Some(59);
            holder.next_relation = None;
            holder.special_relation = EntitySpecialRelation::None;
            holder.record[0..2].copy_from_slice(&source.source_entity_index.to_le_bytes());
            holder.record[2] = u8::from(source.active);
            holder.record[4..8].fill(0);
            holder.record[8..16].fill(0);
            holder.record[16..20].copy_from_slice(&holder.control_flags.to_le_bytes());
            holder.record[24..26].copy_from_slice(&source.entity_rng.to_le_bytes());
            holder.record[26..30].fill(0);
            holder.record[30..32].copy_from_slice(&source.source_entity_index.to_le_bytes());
            holder.record[32..34].copy_from_slice(&75_i16.to_le_bytes());
            holder.record[34..36].copy_from_slice(&768_i16.to_le_bytes());
            holder.record[36..38].copy_from_slice(&768_i16.to_le_bytes());
            holder.record[38..40].copy_from_slice(&1280_i16.to_le_bytes());
            holder.record[44..46].copy_from_slice(&player_entity_index.to_le_bytes());
            holder.record[46..48].copy_from_slice(&profile.subspell_index.to_le_bytes());
            holder.record[48..50].fill(0);
            holder.record[50..52].fill(0);
            holder.record[52..54].copy_from_slice(&profile.word_0x30.to_le_bytes());
            holder.record[58..60].fill(0);
            holder.record[164..168].copy_from_slice(&60_u32.to_le_bytes());
            holder.record[63] = profile.byte_0x3b;
            holder.record[64] = 0;
            holder.record[65] = 0;
            holder.record[66] = source.phase;
            holder.record[67] = 15;
            holder.record[68] = family;
            holder.record[73] = source.action;
            holder.record[74] = tier;
            holder.record[78..80].fill(0);
            holder.record[86..88].copy_from_slice(&75_i16.to_le_bytes());
            holder.record[88..90].copy_from_slice(&768_i16.to_le_bytes());
            holder.record[90..92].copy_from_slice(&768_i16.to_le_bytes());
            holder.record[92..94].copy_from_slice(&1280_i16.to_le_bytes());
            holder.record[94..96].copy_from_slice(&77_i16.to_le_bytes());
            holder.record[80..82].copy_from_slice(&source.position.x.to_le_bytes());
            holder.record[82..84].copy_from_slice(&source.position.y.to_le_bytes());
            holder.record[84..86].copy_from_slice(&source.position.z.to_le_bytes());
            holder.record[140..144].copy_from_slice(&profile.mana_regen.to_le_bytes());
            holder.record[144..148].copy_from_slice(&profile.max_mana.to_le_bytes());
            holder.record[148..152].copy_from_slice(&profile.mana.to_le_bytes());
            rows.push((family, tier, holder));
        }

        for (_, _, holder) in &mut rows {
            let source_entity_index = holder.index;
            let old_map_entity = economy.map_chains.old_map_entity(source_entity_index);
            let next_entity = economy.map_chains.next_entity(source_entity_index);
            holder.record[26..28].copy_from_slice(&old_map_entity.to_le_bytes());
            holder.record[28..30].copy_from_slice(&next_entity.to_le_bytes());
        }

        let mut player = self.players[local_player_index].clone();
        let mut left = player.left_spell_slot_raw;
        let mut right = player.right_spell_slot_raw;
        for (family, tier, holder) in &rows {
            let index = usize::from(*family);
            player.enabled_spell_holders[index] = holder.index;
            player.learned_spell_flags[index] = 1;
            player.family_selected_tiers[index] = tier.cast_signed();
            player.spell_unlocked_levels[index] =
                player.spell_unlocked_levels[index].max(tier.cast_signed());
            player.record[268 + index * 2..270 + index * 2]
                .copy_from_slice(&holder.index.to_le_bytes());
            player.record[424 + index] = 1;
            player.record[528 + index] = *tier;
            player.record[502 + index] = player.spell_unlocked_levels[index].cast_unsigned();
            if left == -1 {
                left = i16::from(*family);
                player.left_subspell_index = 0;
            } else if right == -1 {
                right = i16::from(*family);
                player.right_subspell_index = 0;
            }
        }
        player.left_spell_slot_raw = left;
        player.right_spell_slot_raw = right;
        player.left_spell_slot = u8::try_from(left.max(0)).expect("starting left spell fits");
        player.right_spell_slot = u8::try_from(right.max(0)).expect("starting right spell fits");
        player.record[554..556].copy_from_slice(&left.to_le_bytes());
        player.record[556..558].copy_from_slice(&right.to_le_bytes());
        player.record[558] = player.left_subspell_index.cast_unsigned();
        player.record[559] = player.right_subspell_index.cast_unsigned();

        self.dynamic_pickups
            .extend(rows.into_iter().map(|(_, _, holder)| holder));
        self.players[local_player_index] = player.clone();
        self.local_player = player;
        for family in 0..PLAYER_SPELL_COUNT {
            self.spell_levels[family] =
                self.spell_levels[family].max(self.local_player.family_selected_tiers[family]);
        }
        self.baseline_dynamic_pickups = self.dynamic_pickups.clone();
        self.baseline_players = self.players.clone();
        self.baseline_local_player = self.local_player.clone();
        self.baseline_spell_levels = self.spell_levels;
        Ok(())
    }

    #[must_use]
    pub fn pickups(&self) -> &[EntitySnapshot] {
        &self.pickups
    }

    #[must_use]
    pub fn dynamic_pickups(&self) -> &[EntitySnapshot] {
        &self.dynamic_pickups
    }

    #[must_use]
    pub fn authored_pickup_slots(&self) -> &BTreeMap<u16, u16> {
        &self.authored_pickup_slots
    }

    /// Borrow the source player rows for read-only campaign verification.
    /// Gameplay mutation remains owned by the shared spell route.
    #[must_use]
    pub fn players(&self) -> &[PlayerSnapshot] {
        &self.players
    }

    /// Borrow the source local-player entity used by spell activation and
    /// Castle resource upkeep. This is an inspection-only projection.
    #[must_use]
    pub const fn player_entity(&self) -> &EntitySnapshot {
        &self.player_entity
    }

    fn pickup(&self, source_slot: u16) -> Option<&EntitySnapshot> {
        self.pickups
            .iter()
            .chain(self.dynamic_pickups.iter())
            .find(|entity| entity.index == source_slot)
    }

    fn pickup_mut(&mut self, source_slot: u16) -> Option<&mut EntitySnapshot> {
        if let Some(entity) = self
            .pickups
            .iter_mut()
            .find(|entity| entity.index == source_slot)
        {
            return Some(entity);
        }
        self.dynamic_pickups
            .iter_mut()
            .find(|entity| entity.index == source_slot)
    }

    #[must_use]
    pub fn shield_state(&self) -> Option<ShieldState> {
        self.shield
    }

    fn sync_shield_projection(&mut self) {
        let Some(shield) = self.shield else {
            return;
        };

        if let Some(holder) = self.pickup_mut(shield.holder_entity_index) {
            holder.action = shield.action;
            holder.active_value = shield.active_value;
            holder.activation_value = shield.activation_value.cast_unsigned();
            holder.record[44..46].copy_from_slice(&shield.parent_entity_index.to_le_bytes());
            holder.record[50..52].copy_from_slice(&shield.active_value.to_le_bytes());
            holder.record[52..54].copy_from_slice(&shield.activation_value.to_le_bytes());
            holder.record[58..60].copy_from_slice(&shield.countdown.to_le_bytes());
            holder.record[73] = shield.action;
        }

        let sync_parent = |entity: &mut EntitySnapshot| {
            if entity.index != shield.parent_entity_index {
                return;
            }
            entity.life = shield.parent_life;
            entity.mana = shield.parent_mana;
            entity.mana_regen = shield.parent_mana_regen;
            entity.control_flags = shield.parent_status_flags;
            entity.record[12..16].copy_from_slice(&shield.parent_life.to_le_bytes());
            entity.record[16..20].copy_from_slice(&shield.parent_status_flags.to_le_bytes());
            entity.record[140..144].copy_from_slice(&shield.parent_mana_regen.to_le_bytes());
            entity.record[148..152].copy_from_slice(&shield.parent_mana.to_le_bytes());
        };
        sync_parent(&mut self.player_entity);
        for entity in &mut self.player_entities {
            sync_parent(entity);
        }
        if shield.parent_entity_index == self.player_entity.index {
            self.player_mana = shield.parent_mana;
        }
    }

    fn apply_shield_cleanup(&mut self) {
        let Some(shield) = self.shield else {
            return;
        };
        let queued_spell = self
            .pickup(shield.holder_entity_index)
            .map(|holder| {
                u16::from_le_bytes(
                    holder.record[48..50]
                        .try_into()
                        .expect("queued Shield SetSpell word extent"),
                )
            })
            .unwrap_or_default();
        if queued_spell != 0 {
            let tier = queued_spell.saturating_sub(1).min(2) as u8;
            if let Some(profile) = level2_set_spell_profile(6, tier) {
                if let Some(holder) = self.pickup_mut(shield.holder_entity_index) {
                    holder.level_gate = tier.cast_signed();
                    holder.subspell_index = profile.subspell_index;
                    holder.activation_value = profile.word_0x30;
                    holder.byte_3b = profile.byte_0x3b;
                    holder.mana_regen = profile.mana_regen;
                    holder.max_mana = profile.max_mana;
                    holder.mana_cost = profile.max_mana;
                    holder.mana = profile.mana;
                    holder.record[46..48].copy_from_slice(&profile.subspell_index.to_le_bytes());
                    holder.record[48..50].fill(0);
                    holder.record[52..54].copy_from_slice(&profile.word_0x30.to_le_bytes());
                    holder.record[63] = profile.byte_0x3b;
                    holder.record[74] = tier.cast_signed().cast_unsigned();
                    holder.record[140..144].copy_from_slice(&profile.mana_regen.to_le_bytes());
                    holder.record[144..148].copy_from_slice(&profile.max_mana.to_le_bytes());
                    holder.record[148..152].copy_from_slice(&profile.mana.to_le_bytes());
                }
                if let Some(shield) = self.shield.as_mut() {
                    shield.tier = tier;
                    shield.activation_value = profile.word_0x30.cast_signed();
                    shield.mana_cost = profile.max_mana;
                }
            } else if let Some(holder) = self.pickup_mut(shield.holder_entity_index) {
                holder.record[48..50].fill(0);
            }
        }
        if let Some(shield) = self.shield.as_mut() {
            shield.cleanup_requested = false;
        }
    }

    pub fn activate_shield(&mut self, direction_mask: u16, help_enabled: bool) -> bool {
        self.events.clear();
        let Some(shield) = self.shield.as_mut() else {
            return false;
        };
        let activated = shield.activate(direction_mask, help_enabled, &mut self.events);
        self.sync_shield_projection();
        activated
    }

    /// Submit one source damage mailbox entry for the active family-6 holder.
    /// The outer source pass consumes it at the holder's action-18 callback;
    /// attacker identity is retained for the typed event projection.
    pub fn submit_shield_damage(
        &mut self,
        target_entity_index: u16,
        damage: i32,
        attacker_entity_index: u16,
    ) -> bool {
        if damage <= 0 || attacker_entity_index == 0 {
            return false;
        }
        let Some(shield) = self.shield else {
            return false;
        };
        if shield.parent_entity_index != target_entity_index {
            return false;
        }
        if let Some(pending) = self
            .shield_damage_mailbox
            .as_mut()
            .filter(|pending| pending.target_entity_index == target_entity_index)
        {
            pending.damage = pending.damage.wrapping_add(damage);
            pending.attacker_entity_index = attacker_entity_index;
        } else {
            self.shield_damage_mailbox = Some(Level2ShieldDamageMailbox {
                target_entity_index,
                damage,
                attacker_entity_index,
            });
        }
        true
    }

    /// Consume one source hit through the active family-6 Shield status
    /// branches (`sub_6A480`/`sub_68DE0` consumer projection).  The typed
    /// Shield state remains authoritative, then the holder and parent entity
    /// records are synchronized through the same production projection used
    /// by outer-turn expiry.  A missing Shield is a bounded no-op rather than
    /// a fallback to a neighboring spell family.
    pub fn absorb_shield_damage(&mut self, raw_damage: i32) -> Option<ShieldDamage> {
        let shield = self.shield.as_mut()?;
        let damage = shield.absorb_damage(raw_damage);
        self.sync_shield_projection();
        Some(damage)
    }

    fn absorb_player_shield_damage(
        &mut self,
        target_entity_index: u16,
        raw_damage: i32,
        attacker_entity_index: u16,
    ) -> Option<ShieldDamage> {
        let shield = self.shield?;
        if shield.parent_entity_index != target_entity_index
            || raw_damage <= 0
            || shield.parent_status_flags
                & (crate::spell_activation::SHIELD_MANA_STATUS_BIT
                    | crate::spell_activation::SHIELD_INVULNERABLE_STATUS_BIT)
                == 0
        {
            return None;
        }
        let damage = self.shield.as_mut()?.absorb_damage(raw_damage);
        self.sync_shield_projection();
        self.pickup_events
            .push(Level2SharedSpellEvent::ShieldDamageApplied {
                target_entity_index,
                attacker_entity_index,
                damage,
            });
        Some(damage)
    }

    fn consume_shield_damage_mailbox(&mut self) {
        let Some(shield) = self.shield else {
            return;
        };
        let Some(mailbox) = self
            .shield_damage_mailbox
            .filter(|mailbox| mailbox.target_entity_index == shield.parent_entity_index)
        else {
            return;
        };
        self.shield_damage_mailbox = None;
        if let Some(damage) = self.absorb_shield_damage(mailbox.damage) {
            self.pickup_events
                .push(Level2SharedSpellEvent::ShieldDamageApplied {
                    target_entity_index: mailbox.target_entity_index,
                    attacker_entity_index: mailbox.attacker_entity_index,
                    damage,
                });
        }
    }

    fn step_shield_source_slot(&mut self, source_slot: u16) {
        if self
            .shield
            .is_none_or(|shield| shield.holder_entity_index != source_slot)
        {
            return;
        }
        // The public ABI adapter retains its accepted holder-side consumer.
        // Automatic collision does not use this side mailbox; it writes the
        // player entity mailbox and is evaluated at the player source visit.
        self.consume_shield_damage_mailbox();
        if let Some(shield) = self.shield.as_mut() {
            let _step: ShieldStep = shield.step();
        }
        self.sync_shield_projection();
        if self.shield.is_some_and(|shield| shield.cleanup_requested) {
            self.apply_shield_cleanup();
            self.sync_shield_projection();
        }
    }

    #[must_use]
    pub fn invisible_state(&self) -> Option<InvisibleState> {
        self.invisible
    }

    pub fn activate_invisible(&mut self, direction_mask: u16, help_enabled: bool) -> bool {
        self.events.clear();
        let Some(invisible) = self.invisible.as_mut() else {
            return false;
        };
        let activated = invisible.activate(direction_mask, help_enabled, &mut self.events);
        self.sync_invisible_projection();
        activated
    }

    fn sync_invisible_projection(&mut self) {
        let Some(invisible) = self.invisible else {
            return;
        };
        let owner_word_clear_pending = invisible.owner_word_clear_pending;

        if let Some(holder) = self.pickup_mut(invisible.holder_entity_index) {
            holder.action = invisible.action;
            holder.active_value = invisible.active_value;
            holder.activation_value = invisible.activation_value.cast_unsigned();
            holder.control_flags = (holder.control_flags & !INVISIBLE_STATUS_BIT)
                | (invisible.spell_status_flags & INVISIBLE_STATUS_BIT);
            holder.record[50..52].copy_from_slice(&invisible.active_value.to_le_bytes());
            holder.record[52..54].copy_from_slice(&invisible.activation_value.to_le_bytes());
            holder.record[58..60].copy_from_slice(&invisible.countdown.to_le_bytes());
            holder.record[16..20].copy_from_slice(&holder.control_flags.to_le_bytes());
            holder.record[73] = invisible.action;
        }

        let sync_parent = |entity: &mut EntitySnapshot| {
            if entity.index != invisible.parent_entity_index {
                return;
            }
            entity.life = invisible.parent_life;
            entity.mana = invisible.parent_mana;
            entity.mana_regen = invisible.parent_mana_regen;
            entity.control_flags = invisible.parent_status_flags;
            entity.record[12..16].copy_from_slice(&invisible.parent_life.to_le_bytes());
            entity.record[16..20].copy_from_slice(&invisible.parent_status_flags.to_le_bytes());
            entity.record[140..144].copy_from_slice(&invisible.parent_mana_regen.to_le_bytes());
            entity.record[148..152].copy_from_slice(&invisible.parent_mana.to_le_bytes());
        };
        sync_parent(&mut self.player_entity);
        for entity in &mut self.player_entities {
            sync_parent(entity);
        }
        let mut local_player_projection = None;
        for player in &mut self.players {
            if player.entity_index == invisible.parent_entity_index {
                // `sub_6B1C0` clears the owner state word at offset 345 on
                // the first sustained pulse before publishing the tier's
                // visibility byte at offset 447. Keep the raw player record
                // write separate from the status-bit asymmetry above.
                if owner_word_clear_pending {
                    player.record[345..347].fill(0);
                }
                player.record[447] = invisible.parent_visibility_mode;
                if player.slot == 0 {
                    local_player_projection = Some(player.clone());
                }
            }
        }
        if let Some(player) = local_player_projection {
            self.local_player = player;
        }
        if owner_word_clear_pending {
            if let Some(invisible) = self.invisible.as_mut() {
                invisible.owner_word_clear_pending = false;
            }
        }
        if invisible.parent_entity_index == self.player_entity.index {
            self.player_mana = invisible.parent_mana;
        }
    }

    /// Find a generic command mutation that aliases one already-modeled typed
    /// lifecycle. The C ABI command context is local-only, so both holder and
    /// caster identities must agree before it can hand state back to a source
    /// lifecycle.
    fn matching_typed_spell_fire_holder(
        caster: SpellCasterState,
        holders: &[SpellActivationHolder],
        parent_entity_index: u16,
        holder_entity_index: u16,
        family: u8,
    ) -> Option<SpellActivationHolder> {
        if caster.entity_index != parent_entity_index {
            return None;
        }
        holders
            .iter()
            .copied()
            .find(|holder| holder.entity_index == holder_entity_index && holder.model == family)
    }

    /// `sub_5F660` mutates the shared command holder after the typed source
    /// callback. Reconcile the matching family-6 alias so the next ordinary
    /// `sub_6A480` visit sees the same active state; remote holders never
    /// enter this local command mirror.
    fn sync_shield_from_spell_fire(
        &mut self,
        caster: SpellCasterState,
        holders: &[SpellActivationHolder],
    ) {
        let Some(shield) = self.shield else {
            return;
        };
        let Some(holder) = Self::matching_typed_spell_fire_holder(
            caster,
            holders,
            shield.parent_entity_index,
            shield.holder_entity_index,
            SHIELD_FAMILY,
        ) else {
            return;
        };
        let shield = self.shield.as_mut().expect("Shield state remains present");
        shield.active_value = holder.active_value;
        shield.activation_value = holder.activation_value.cast_signed();
        shield.parent_mana = caster.mana;
        shield.parent_status_flags = caster.control_flags & !0x3;
        if holder.active_value != 0 {
            shield.action = SHIELD_ACTIVE_ACTION;
            shield.cleanup_requested = false;
        }
        self.sync_shield_projection();
    }

    /// `sub_5F660` mutates the shared command holder before the next
    /// family-11 source visit. Keep the existing typed lifecycle as the
    /// authority for that following `sub_6B1C0` pulse by reconciling only the
    /// matching local holder; remote player holders never enter this command
    /// mirror.
    fn sync_invisible_from_spell_fire(
        &mut self,
        caster: SpellCasterState,
        holders: &[SpellActivationHolder],
    ) {
        let Some(invisible) = self.invisible.as_mut() else {
            return;
        };
        let Some(holder) = Self::matching_typed_spell_fire_holder(
            caster,
            holders,
            invisible.parent_entity_index,
            invisible.holder_entity_index,
            INVISIBLE_FAMILY,
        ) else {
            return;
        };

        invisible.active_value = holder.active_value;
        invisible.activation_value = holder.activation_value.cast_signed();
        invisible.parent_mana = caster.mana;
        invisible.parent_status_flags = caster.control_flags & !0x3;
        if holder.active_value != 0 {
            invisible.action = INVISIBLE_ACTIVE_ACTION;
            invisible.cleanup_requested = false;
        }
        self.sync_invisible_projection();
    }

    fn apply_invisible_cleanup(&mut self) {
        let Some(invisible) = self.invisible else {
            return;
        };
        let queued_spell = self
            .pickup(invisible.holder_entity_index)
            .map(|holder| {
                u16::from_le_bytes(
                    holder.record[48..50]
                        .try_into()
                        .expect("queued Invisible SetSpell word extent"),
                )
            })
            .unwrap_or_default();
        if queued_spell != 0 {
            let tier = queued_spell.saturating_sub(1).min(2) as u8;
            if let Some(profile) = level2_set_spell_profile(11, tier) {
                if let Some(holder) = self.pickup_mut(invisible.holder_entity_index) {
                    holder.level_gate = tier.cast_signed();
                    holder.subspell_index = profile.subspell_index;
                    holder.activation_value = profile.word_0x30;
                    holder.byte_3b = profile.byte_0x3b;
                    holder.mana_regen = profile.mana_regen;
                    holder.max_mana = profile.max_mana;
                    holder.mana_cost = profile.max_mana;
                    holder.mana = profile.mana;
                    holder.record[46..48].copy_from_slice(&profile.subspell_index.to_le_bytes());
                    holder.record[48..50].fill(0);
                    holder.record[52..54].copy_from_slice(&profile.word_0x30.to_le_bytes());
                    holder.record[63] = profile.byte_0x3b;
                    holder.record[74] = tier.cast_signed().cast_unsigned();
                    holder.record[140..144].copy_from_slice(&profile.mana_regen.to_le_bytes());
                    holder.record[144..148].copy_from_slice(&profile.max_mana.to_le_bytes());
                    holder.record[148..152].copy_from_slice(&profile.mana.to_le_bytes());
                }
                if let Some(invisible) = self.invisible.as_mut() {
                    invisible.tier = tier;
                    invisible.activation_value = profile.word_0x30.cast_signed();
                    invisible.mana_cost = profile.max_mana;
                    invisible.mana_regen_requirement = profile.mana_regen;
                }
            } else if let Some(holder) = self.pickup_mut(invisible.holder_entity_index) {
                holder.record[48..50].fill(0);
            }
        }
        if let Some(invisible) = self.invisible.as_mut() {
            invisible.cleanup_requested = false;
        }
    }

    fn step_invisible_source_slot(&mut self, source_slot: u16, parent_castle_mana: Option<i32>) {
        if self
            .invisible
            .is_none_or(|invisible| invisible.holder_entity_index != source_slot)
        {
            return;
        }
        if let Some(invisible) = self.invisible.as_mut() {
            let _step: InvisibleStep = invisible.step(parent_castle_mana);
        }
        self.sync_invisible_projection();
        if self
            .invisible
            .is_some_and(|invisible| invisible.cleanup_requested)
        {
            self.apply_invisible_cleanup();
            self.sync_invisible_projection();
        }
    }

    #[must_use]
    pub fn beyond_sight_state(&self) -> Option<BeyondSightState> {
        self.beyond_sight
    }

    #[must_use]
    pub const fn beyond_sight_auxiliary(&self) -> Level2PlayerEventAuxiliary {
        self.beyond_sight_auxiliary
    }

    /// Bind the latched player-input words to the same transient fields that
    /// `PlayerEvents_51BB0` writes before the source entity pass.
    pub fn set_beyond_sight_player_input(
        &mut self,
        entity_index: u16,
        next_entity: u16,
        entity_index2: u16,
    ) {
        self.beyond_sight_auxiliary.entity_index = entity_index;
        self.beyond_sight_auxiliary.next_entity = next_entity.cast_signed();
        self.beyond_sight_auxiliary.entity_index2 = entity_index2.cast_signed();
    }

    pub fn activate_beyond_sight(&mut self, direction_mask: u16, help_enabled: bool) -> bool {
        self.events.clear();
        let Some(beyond_sight) = self.beyond_sight.as_mut() else {
            return false;
        };
        let activated = beyond_sight.activate(direction_mask, help_enabled, &mut self.events);
        self.sync_beyond_sight_projection();
        activated
    }

    fn sync_beyond_sight_projection(&mut self) {
        let Some(beyond_sight) = self.beyond_sight else {
            return;
        };

        if let Some(holder) = self.pickup_mut(beyond_sight.holder_entity_index) {
            holder.action = beyond_sight.action;
            holder.active_value = beyond_sight.active_value;
            holder.activation_value = beyond_sight.activation_value.cast_unsigned();
            holder.record[50..52].copy_from_slice(&beyond_sight.active_value.to_le_bytes());
            holder.record[52..54].copy_from_slice(&beyond_sight.activation_value.to_le_bytes());
            holder.record[58..60].copy_from_slice(&beyond_sight.countdown.to_le_bytes());
            holder.record[73] = beyond_sight.action;
        }

        let sync_parent = |entity: &mut EntitySnapshot| {
            if entity.index != beyond_sight.parent_entity_index {
                return;
            }
            entity.life = beyond_sight.parent_life;
            entity.mana = beyond_sight.parent_mana;
            entity.mana_regen = beyond_sight.parent_mana_regen;
            entity.record[12..16].copy_from_slice(&beyond_sight.parent_life.to_le_bytes());
            entity.record[140..144].copy_from_slice(&beyond_sight.parent_mana_regen.to_le_bytes());
            entity.record[148..152].copy_from_slice(&beyond_sight.parent_mana.to_le_bytes());
        };
        sync_parent(&mut self.player_entity);
        for entity in &mut self.player_entities {
            sync_parent(entity);
        }
        if beyond_sight.parent_entity_index == self.player_entity.index {
            self.player_mana = beyond_sight.parent_mana;
        }
    }

    /// Copy the post-upkeep entity-111 values into local typed parent caches
    /// without invoking their projection writers. The next source callback
    /// may legitimately consume the fresh mana/regen, while a projection
    /// here would replay the previous callback's stale cache over the player.
    fn sync_typed_spell_parent_caches_from_player_entity(&mut self) {
        let parent_entity_index = self.player_entity.index;
        let parent_life = self.player_entity.life;
        let parent_mana = self.player_entity.mana;
        let parent_mana_regen = self.player_entity.mana_regen;
        if let Some(shield) = self
            .shield
            .as_mut()
            .filter(|shield| shield.parent_entity_index == parent_entity_index)
        {
            shield.parent_life = parent_life;
            shield.parent_mana = parent_mana;
            shield.parent_mana_regen = parent_mana_regen;
        }
        if let Some(invisible) = self
            .invisible
            .as_mut()
            .filter(|invisible| invisible.parent_entity_index == parent_entity_index)
        {
            invisible.parent_life = parent_life;
            invisible.parent_mana = parent_mana;
            invisible.parent_mana_regen = parent_mana_regen;
        }
        if let Some(beyond_sight) = self
            .beyond_sight
            .as_mut()
            .filter(|beyond_sight| beyond_sight.parent_entity_index == parent_entity_index)
        {
            beyond_sight.parent_life = parent_life;
            beyond_sight.parent_mana = parent_mana;
            beyond_sight.parent_mana_regen = parent_mana_regen;
        }
    }

    /// Reconcile the generic `sub_5F660` holder mutation with the modeled
    /// family-12 lifecycle before its following ordinary source pulse. The
    /// typed state remains the authority for the later pulse/effect path.
    fn sync_beyond_sight_from_spell_fire(
        &mut self,
        caster: SpellCasterState,
        holders: &[SpellActivationHolder],
    ) {
        let Some(beyond_sight) = self.beyond_sight else {
            return;
        };
        let Some(holder) = Self::matching_typed_spell_fire_holder(
            caster,
            holders,
            beyond_sight.parent_entity_index,
            beyond_sight.holder_entity_index,
            BEYOND_SIGHT_FAMILY,
        ) else {
            return;
        };
        let beyond_sight = self
            .beyond_sight
            .as_mut()
            .expect("Beyond Sight state remains present");
        beyond_sight.active_value = holder.active_value;
        beyond_sight.activation_value = holder.activation_value.cast_signed();
        beyond_sight.parent_mana = caster.mana;
        if holder.active_value != 0 {
            beyond_sight.action = BEYOND_SIGHT_ACTIVE_ACTION;
            beyond_sight.cleanup_requested = false;
        }
        self.sync_beyond_sight_projection();
    }

    fn apply_beyond_sight_cleanup(&mut self) {
        let Some(beyond_sight) = self.beyond_sight else {
            return;
        };
        let queued_spell = self
            .pickup(beyond_sight.holder_entity_index)
            .map(|holder| {
                u16::from_le_bytes(
                    holder.record[48..50]
                        .try_into()
                        .expect("queued Beyond Sight SetSpell word extent"),
                )
            })
            .unwrap_or_default();
        if queued_spell != 0 {
            let tier = queued_spell.saturating_sub(1).min(2) as u8;
            if let Some(profile) = level2_set_spell_profile(BEYOND_SIGHT_FAMILY, tier) {
                if let Some(holder) = self.pickup_mut(beyond_sight.holder_entity_index) {
                    holder.level_gate = tier.cast_signed();
                    holder.subspell_index = profile.subspell_index;
                    holder.activation_value = profile.word_0x30;
                    holder.byte_3b = profile.byte_0x3b;
                    holder.mana_regen = profile.mana_regen;
                    holder.max_mana = profile.max_mana;
                    holder.mana_cost = profile.max_mana;
                    holder.mana = profile.mana;
                    holder.record[46..48].copy_from_slice(&profile.subspell_index.to_le_bytes());
                    holder.record[48..50].fill(0);
                    holder.record[52..54].copy_from_slice(&profile.word_0x30.to_le_bytes());
                    holder.record[63] = profile.byte_0x3b;
                    holder.record[74] = tier.cast_signed().cast_unsigned();
                    holder.record[140..144].copy_from_slice(&profile.mana_regen.to_le_bytes());
                    holder.record[144..148].copy_from_slice(&profile.max_mana.to_le_bytes());
                    holder.record[148..152].copy_from_slice(&profile.mana.to_le_bytes());
                }
                if let Some(beyond_sight) = self.beyond_sight.as_mut() {
                    beyond_sight.tier = tier;
                    beyond_sight.activation_value = profile.word_0x30.cast_signed();
                    beyond_sight.mana_cost = profile.max_mana;
                    beyond_sight.mana_regen_requirement = profile.mana_regen;
                }
            } else if let Some(holder) = self.pickup_mut(beyond_sight.holder_entity_index) {
                holder.record[48..50].fill(0);
            }
        }
        if let Some(beyond_sight) = self.beyond_sight.as_mut() {
            beyond_sight.cleanup_requested = false;
        }
    }

    fn step_beyond_sight_source_slot(&mut self, source_slot: u16, parent_castle_mana: Option<i32>) {
        if self
            .beyond_sight
            .is_none_or(|beyond_sight| beyond_sight.holder_entity_index != source_slot)
        {
            return;
        }
        let Some(mut beyond_sight) = self.beyond_sight.take() else {
            return;
        };
        if beyond_sight.holder_entity_index != source_slot {
            self.beyond_sight = Some(beyond_sight);
            return;
        }
        let parent_entity_index = beyond_sight.parent_entity_index;
        let experience_awarded = beyond_sight.award_experience_if_fresh_pulse();
        if experience_awarded != 0 {
            // `sub_6B310` orders sub_6D8B0 before sub_68D50. Publish the
            // owner row and threshold presentation before the predicate
            // can collapse a failed fresh pulse.
            self.award_beyond_sight_experience(parent_entity_index, experience_awarded);
        }
        let _step =
            beyond_sight.step_after_experience_award(parent_castle_mana, experience_awarded);
        self.beyond_sight = Some(beyond_sight);
        // `sub_6B310` ends after its own predicate/charge/countdown and
        // never dispatches the separate class-10/model-74/action-72
        // machinery.
        self.sync_beyond_sight_projection();
        if self
            .beyond_sight
            .is_some_and(|beyond_sight| beyond_sight.cleanup_requested)
        {
            self.apply_beyond_sight_cleanup();
            self.sync_beyond_sight_projection();
        }
    }

    /// Materialise one authored class-15 row from `sub_4A1E0`'s group pass.
    /// The source constructor is deliberately explicit: the row is absent at
    /// reset, receives a shared-arena slot/RNG, and only then enters the
    /// generic `sub_68FF0` acquisition scan.
    pub fn materialize_authored_spell_pickup(
        &mut self,
        economy: &mut Level2EconomyState,
        authored: AuthoredFeature,
        terrain: &SessionTerrain,
    ) -> Result<u16> {
        if let Some(source_slot) = self.authored_pickup_slots.get(&authored.index) {
            return Ok(*source_slot);
        }
        let family = match authored.subtype {
            6 => 6,
            11 => 11,
            _ => {
                return Err(NativeWorldError::new(format!(
                    "unsupported authored Level 2 spell subtype {} at row {}",
                    authored.subtype, authored.index
                )));
            }
        };
        if authored.kind != 15 || authored.stage_tag != 1 {
            return Err(NativeWorldError::new(format!(
                "authored Level 2 spell row {} is not a stage-1 class-15 source",
                authored.index
            )));
        }
        let action = physical_spell_action(family, PhysicalSpellActionRole::OrdinaryPickup)
            .expect("validated authored Level 2 family has an ordinary action");
        let position = authored.world_position(terrain.ground_height(authored.world_position(0)));
        let source = economy
            .allocate_spell_pickup(authored.index, family, action, position)
            .ok_or_else(|| {
                NativeWorldError::new(format!(
                    "Level 2 authored spell row {} could not allocate a source slot",
                    authored.index
                ))
            })?;
        let template = self
            .pickups
            .first()
            .ok_or_else(|| NativeWorldError::new("Level 2 shared spell template is absent"))?;
        let mut entity = template.clone();
        entity.index = source.source_entity_index;
        entity.source_id = 0;
        entity.authority = EntityAuthority::SpellActivationLive;
        entity.active = true;
        entity.max_life = 0;
        entity.life = 0;
        entity.class = 15;
        entity.model = source.family;
        entity.control_flags = 4;
        entity.entity_rng = source.entity_rng;
        entity.action = source.action;
        entity.action_marker = 0;
        entity.level_gate = 0;
        entity.position = source.position;
        entity.phase = source.phase;
        entity.subspell_index = 0;
        entity.byte_3b = 0;
        entity.byte_3c = 0;
        entity.font_type_index = 0;
        entity.active_value = 0;
        entity.activation_value = 0;
        entity.mana_regen = 0;
        entity.max_mana = 0;
        entity.mana_cost = 0;
        entity.mana = 0;
        entity.next_relation = None;
        entity.special_relation = EntitySpecialRelation::None;
        entity.record[0..2].copy_from_slice(&source.source_entity_index.to_le_bytes());
        entity.record[2] = 1;
        entity.record[4..8].fill(0);
        entity.record[8..16].fill(0);
        entity.record[16..20].copy_from_slice(&4_u32.to_le_bytes());
        entity.record[24..26].copy_from_slice(&source.entity_rng.to_le_bytes());
        entity.record[26..30].fill(0);
        entity.record[30..32].fill(0);
        entity.record[46..54].fill(0);
        entity.record[58..60].fill(0);
        entity.record[63..66].fill(0);
        entity.record[66] = source.phase;
        entity.record[67] = 15;
        entity.record[68] = source.family;
        entity.record[73] = source.action;
        entity.record[74] = 0;
        entity.record[78..80].fill(0);
        entity.record[80..82].copy_from_slice(&source.position.x.to_le_bytes());
        entity.record[82..84].copy_from_slice(&source.position.y.to_le_bytes());
        entity.record[84..86].copy_from_slice(&source.position.z.to_le_bytes());
        entity.record[140..152].fill(0);
        self.dynamic_pickups.push(entity.clone());
        self.authored_pickup_slots
            .insert(authored.index, source.source_entity_index);
        let index = self
            .acquisitions
            .iter()
            .position(|state| state.family == family)
            .expect("authored Level 2 family has an acquisition state");
        let state = &mut self.acquisitions[index];
        state.source_slot = source.source_entity_index;
        state.authored_source_index = authored.index;
        state.source_action = source.action;
        state.source_life = entity.life;
        state.source_draw_enabled = true;
        state.subspell_index = entity.subspell_index;
        state.word_0x30 = entity.source_spell_word_30();
        state.byte_0x3b = entity.byte_3b;
        state.byte_0x3c = entity.byte_3c;
        state.font_type_index = entity.font_type_index;
        state.mana_regen = entity.mana_regen;
        state.max_mana = entity.max_mana;
        state.mana = entity.mana;
        state.return_side = entity.source_word_4a();
        Ok(source.source_entity_index)
    }

    /// Bind an economy-owned `sub_69250` replacement to the shared spell
    /// source projection. In remc2 the newly allocated class-15 row is a live
    /// persistent pickup in the same entity arena; keeping an entity record
    /// here lets the normal collision/hand transition inspect its action and
    /// position on the next cursor pass.
    fn materialize_spell_replacement_pickup(
        &mut self,
        replacement: Level2SpellReplacement,
    ) -> Result<()> {
        let family_index = self
            .acquisitions
            .iter()
            .position(|state| state.family == replacement.family)
            .ok_or_else(|| {
                NativeWorldError::new(format!(
                    "unsupported Level 2 persistent spell family {}",
                    replacement.family
                ))
            })?;
        if self
            .dynamic_pickups
            .iter()
            .any(|entity| entity.index == replacement.source_entity_index)
        {
            return Ok(());
        }
        let template = self
            .pickups
            .first()
            .ok_or_else(|| NativeWorldError::new("Level 2 shared spell template is absent"))?;
        let mut entity = template.clone();
        entity.index = replacement.source_entity_index;
        entity.source_id = 0;
        entity.authority = EntityAuthority::SpellActivationLive;
        entity.active = replacement.active;
        entity.max_life = 0;
        entity.life = 0;
        entity.class = 15;
        entity.model = replacement.family;
        entity.control_flags = replacement.control_flags | 4;
        entity.entity_rng = replacement.entity_rng;
        entity.action = replacement.action;
        entity.action_marker = 0;
        entity.level_gate = 0;
        entity.position = replacement.position;
        entity.phase = replacement.phase;
        entity.subspell_index = 0;
        entity.byte_3b = 0;
        entity.byte_3c = 0;
        entity.font_type_index = 0;
        entity.active_value = 0;
        entity.activation_value = 0;
        entity.mana_regen = 0;
        entity.max_mana = 0;
        entity.mana_cost = 0;
        entity.mana = 0;
        entity.next_relation = None;
        entity.special_relation = EntitySpecialRelation::None;
        entity.record[0..2].copy_from_slice(&replacement.source_entity_index.to_le_bytes());
        entity.record[2] = 1;
        entity.record[4..8].fill(0);
        entity.record[8..16].fill(0);
        entity.record[16..20].copy_from_slice(&entity.control_flags.to_le_bytes());
        entity.record[24..26].copy_from_slice(&replacement.entity_rng.to_le_bytes());
        entity.record[26..30].fill(0);
        entity.record[30..32].fill(0);
        entity.record[46..54].fill(0);
        entity.record[58..60].fill(0);
        entity.record[63..66].fill(0);
        entity.record[66] = replacement.phase;
        entity.record[67] = 15;
        entity.record[68] = replacement.family;
        entity.record[73] = replacement.action;
        entity.record[74] = 0;
        entity.record[78..80].fill(0);
        entity.record[80..82].copy_from_slice(&replacement.position.x.to_le_bytes());
        entity.record[82..84].copy_from_slice(&replacement.position.y.to_le_bytes());
        entity.record[84..86].copy_from_slice(&replacement.position.z.to_le_bytes());
        entity.record[140..152].fill(0);
        self.dynamic_pickups.push(entity.clone());

        let mut state = self.acquisitions[family_index];
        state.source_slot = replacement.source_entity_index;
        state.authored_source_index = 0;
        state.source_action = replacement.action;
        state.source_life = entity.life;
        state.source_draw_enabled = true;
        state.subspell_index = entity.subspell_index;
        state.word_0x30 = entity.source_spell_word_30();
        state.byte_0x3b = entity.byte_3b;
        state.byte_0x3c = entity.byte_3c;
        state.font_type_index = entity.font_type_index;
        state.mana_regen = entity.mana_regen;
        state.max_mana = entity.max_mana;
        state.mana = entity.mana;
        state.return_side = entity.source_word_4a();
        self.persistent_acquisitions
            .insert(replacement.source_entity_index, state);
        Ok(())
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

    /// Keep the shared spell projection's local player entity in lockstep
    /// with the Level 2 source damage mailbox.  The raw entity record is
    /// updated because reset/campaign hashes bind the lossless source bytes.
    fn set_player_entity_life(&mut self, life: i32) {
        self.player_entity.life = life;
        self.player_entity.record[12..16].copy_from_slice(&life.to_le_bytes());
        for entity in &mut self.player_entities {
            if entity.index == self.player_entity.index {
                entity.life = life;
                entity.record[12..16].copy_from_slice(&life.to_le_bytes());
            }
        }
    }

    /// Execute the source PlayerEvents phase update once for the complete
    /// outer frame.  The source increments Turn and rewrites its colorIndex
    /// array before any speed-index entity pass; the bounded projection keeps
    /// the lossless turn carrier in both local/player views and returns the
    /// two HUD phases consumed by this slice.
    fn step_day_hud_player_events(&mut self) -> (u8, u8) {
        let local_player_slot = self.local_player.slot;
        let turn = self.local_player.turn.wrapping_add(1);
        self.local_player.turn = turn;
        self.local_player.record[8..12].copy_from_slice(&turn.to_le_bytes());
        if let Some(player) = self
            .players
            .iter_mut()
            .find(|player| player.slot == local_player_slot)
        {
            player.turn = turn;
            player.record[8..12].copy_from_slice(&turn.to_le_bytes());
        }
        level2_day_color_phases(turn)
    }

    /// Carry the source PlayerHitFrameTime_406 write through every typed
    /// player view.  The value is decremented only by the post-update HUD
    /// phase, never by a snapshot read.
    fn set_player_hit_frame_time(&mut self, value: u8) {
        let local_player_slot = self.local_player.slot;
        self.local_player.record[406] = value;
        if let Some(player) = self
            .players
            .iter_mut()
            .find(|player| player.slot == local_player_slot)
        {
            player.record[406] = value;
        }
    }

    /// Keep the shared spell projection's source damage mailbox in lockstep
    /// with the Level 2 player route.  The exported entity record carries a
    /// four-byte header, so the source payload lives at bytes 98..102 and its
    /// owner at bytes 102..104; bytes 104..108 remain untouched.
    fn set_player_entity_damage_mailbox(&mut self, damage: i32, attacker: u16) {
        self.player_entity.record[98..102].copy_from_slice(&damage.to_le_bytes());
        self.player_entity.record[102..104].copy_from_slice(&attacker.to_le_bytes());
        for entity in &mut self.player_entities {
            if entity.index == self.player_entity.index {
                entity.record[98..102].copy_from_slice(&damage.to_le_bytes());
                entity.record[102..104].copy_from_slice(&attacker.to_le_bytes());
            }
        }
    }

    /// Mirror `sub_5EFA0`'s attacker/killer words and the following player
    /// death action into the shared source projection.
    fn set_player_entity_damage_result(&mut self, killer: u16, attacker: u16, dead: bool) {
        self.player_entity.set_damage_result_words(killer, attacker);
        if dead {
            self.player_entity.set_player_death_state();
        }
        for entity in &mut self.player_entities {
            if entity.index == self.player_entity.index {
                entity.set_damage_result_words(killer, attacker);
                if dead {
                    entity.set_player_death_state();
                }
            }
        }
    }

    /// Execute the qualified local-player portion of remc2's
    /// `AddPlayer03_00_5E010` upkeep. The source adds the previous
    /// `manaRegen` value, clamps mana to the signed `[0,maxMana]` interval,
    /// then selects the Castle/boost or outdoor recharge rate from the
    /// shift-AABB result. The relation alone never selects the Castle rate.
    fn step_local_player_unboosted_upkeep(
        &mut self,
        castle_entity_index: u16,
        castle_overlap: bool,
        projected_max_mana: Option<i32>,
    ) -> bool {
        const RECHARGE_BOOST_FLAG: u32 = 0x0000_1000;
        let player_entity_index = self.player_entity.index;
        if self.player_entity.life < 0 {
            return false;
        }

        if let Some(max_mana) = projected_max_mana {
            self.player_entity.max_mana = max_mana;
            self.player_entity.record[144..148].copy_from_slice(&max_mana.to_le_bytes());
            if let Some(entity) = self
                .player_entities
                .iter_mut()
                .find(|entity| entity.index == player_entity_index)
            {
                entity.max_mana = max_mana;
                entity.record[144..148].copy_from_slice(&max_mana.to_le_bytes());
            }
        }

        // The source entity is authoritative for this callback; the caster
        // projection is only the activation gate and is synchronized after
        // the upkeep write.
        let mut life = self.player_entity.life.wrapping_add(self.player_life_regen);
        life = life.clamp(-1, self.player_entity.max_life);
        let mut mana = self
            .player_entity
            .mana
            .wrapping_add(self.player_entity.mana_regen);
        if mana < 0 {
            mana = 0;
        }
        if mana > self.player_entity.max_mana {
            mana = self.player_entity.max_mana;
        }
        let castle_overlap_active = castle_entity_index != 0 && castle_overlap;
        let recharge_boost_active = self.player_entity.control_flags & RECHARGE_BOOST_FLAG != 0;
        let castle_or_boost = castle_overlap_active || recharge_boost_active;
        let next_mana_regen = if castle_or_boost {
            (self.player_entity.max_mana / 200).max(1_000)
        } else {
            (self.player_entity.max_mana / 2_000).max(100)
        };
        let next_life_regen = if castle_or_boost {
            self.player_entity.max_life / 250
        } else {
            self.player_entity.max_life / 2_000
        };

        self.player_entity.life = life;
        self.player_entity.record[12..16].copy_from_slice(&life.to_le_bytes());
        self.player_entity.mana = mana;
        self.player_entity.mana_regen = next_mana_regen;
        self.player_life_regen = next_life_regen;
        self.player_entity.record[140..144].copy_from_slice(&next_mana_regen.to_le_bytes());
        self.player_entity.record[148..152].copy_from_slice(&mana.to_le_bytes());
        if let Some(entity) = self
            .player_entities
            .iter_mut()
            .find(|entity| entity.index == player_entity_index)
        {
            entity.life = life;
            entity.record[12..16].copy_from_slice(&life.to_le_bytes());
            entity.mana = mana;
            entity.mana_regen = next_mana_regen;
            entity.record[140..144].copy_from_slice(&next_mana_regen.to_le_bytes());
            entity.record[148..152].copy_from_slice(&mana.to_le_bytes());
        }
        if recharge_boost_active {
            self.player_entity.control_flags &= !RECHARGE_BOOST_FLAG;
            self.player_entity.record[16..20]
                .copy_from_slice(&self.player_entity.control_flags.to_le_bytes());
            self.caster.control_flags = self.player_entity.control_flags;
            for entity in &mut self.player_entities {
                if entity.index == player_entity_index {
                    entity.control_flags = self.player_entity.control_flags;
                    entity.record[16..20].copy_from_slice(&entity.control_flags.to_le_bytes());
                }
            }
        }
        for player in &mut self.players {
            if player.entity_index == player_entity_index {
                let mut word_159 = u16::from_le_bytes(
                    player.record[345..347]
                        .try_into()
                        .expect("qualified player word159 field"),
                );
                if castle_overlap_active {
                    word_159 = 2;
                }
                // `AddPlayer03_00_5E010` refreshes this latch only from the
                // Castle overlap predicate, then performs the ordinary
                // same-tick countdown. The independent recharge boost never
                // writes it.
                word_159 = word_159.saturating_sub(1);
                player.record[345..347].copy_from_slice(&word_159.to_le_bytes());
                if player.slot == self.local_player.slot {
                    self.local_player = player.clone();
                }
            }
        }
        self.caster.mana = mana;
        self.player_mana = mana;
        true
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

    #[must_use]
    pub fn spell_experience(&self) -> &[i32; PLAYER_SPELL_COUNT] {
        &self.spell_experience
    }

    #[must_use]
    pub fn spell_levels(&self) -> &[i8; PLAYER_SPELL_COUNT] {
        &self.spell_levels
    }

    /// Apply the source `sub_6D8B0(parent, 12, 1)` family-12 award and
    /// `sub_6D9C0` projection to the exact owning player row. The persistent
    /// and runtime words are summed with source i32 wrapping; a newly
    /// computed unlocked level can clamp a selected tier downward, but never
    /// raises it automatically. The convenience arrays are local-player
    /// projections only.
    fn award_beyond_sight_experience(&mut self, parent_entity_index: u16, amount: i32) {
        let family = usize::from(BEYOND_SIGHT_FAMILY);
        let owner_index = self
            .players
            .iter()
            .position(|player| player.entity_index == parent_entity_index)
            .expect("Beyond Sight parent remains in the source player rows");
        let mut owner = self.players[owner_index].clone();
        let previous_unlocked = owner.spell_unlocked_levels[family];
        let runtime = owner.runtime_spell_experience[family].wrapping_add(amount);
        let total = owner.persistent_spell_experience[family].wrapping_add(runtime);
        let unlocked = beyond_sight_unlocked_tier(total);
        owner.runtime_spell_experience[family] = runtime;
        owner.spell_unlocked_levels[family] = unlocked;
        owner.record[164 + family * std::mem::size_of::<i32>()
            ..164 + (family + 1) * std::mem::size_of::<i32>()]
            .copy_from_slice(&runtime.to_le_bytes());
        owner.record[502 + family] = unlocked.cast_unsigned();
        if owner.family_selected_tiers[family] > unlocked {
            owner.family_selected_tiers[family] = unlocked;
            owner.record[528 + family] = unlocked.cast_unsigned();
        }
        let source_family_enabled =
            owner.spell_enabled_flags[family] != 0 || owner.enabled_spell_holders[family] != 0;
        let owner_is_local = owner.entity_index == self.local_player.entity_index;
        self.players[owner_index] = owner.clone();
        if owner_is_local {
            self.spell_experience[family] = runtime;
            self.spell_levels[family] = unlocked;
            self.local_player = owner;
        }
        if unlocked != previous_unlocked && source_family_enabled {
            mark_source_audio_call(SourceAudioCallTag::AbilityImprovedTier);
            self.pickup_events
                .push(Level2SharedSpellEvent::AbilityImproved {
                    source_family: BEYOND_SIGHT_FAMILY,
                    tier: unlocked,
                });
            self.events.push(FlightEvent::PrepareSound {
                #[cfg(test)]
                audio_site: crate::MissionAudioSite::AbilityImproved,
                entity_index: 0,
                player_index: -1,
                sound_id: 61,
            });
        }
    }

    /// Run the source `UpdateScroll_59C80` callback for a live model-5 row.
    /// The source scans every live model-0 player in linked-list order, but
    /// only the local LevelIndex player receives XP and tier updates. A
    /// remote collision still consumes the scroll, so the scan deliberately
    /// does not return early after the first overlap.
    pub fn collect_level2_experience_scroll(
        &mut self,
        source_slot: u16,
        scroll_position: Position,
        local_player_slot: u8,
        local_player_entity_index: u16,
        local_player_position: Position,
        local_player_life: i32,
        entities: &[EntitySnapshot],
    ) -> bool {
        let mut first_collision_entity = None;
        let mut local_collision = false;
        let mut local_player_index = None;
        for (index, player) in self.players.iter().enumerate() {
            let is_local = player.player_color_index() == local_player_slot
                && player.entity_index == local_player_entity_index;
            if is_local {
                local_player_index = Some(index);
            }
            let (active, class, model, life, position) = if is_local {
                (true, 3, 0, local_player_life, local_player_position)
            } else if let Some(entity) = entities
                .iter()
                .find(|entity| entity.index == player.entity_index)
            {
                (
                    entity.active,
                    entity.class,
                    entity.model,
                    entity.life,
                    entity.position,
                )
            } else {
                continue;
            };
            if !active
                || class != 3
                || model != 0
                || life < 0
                || !level2_experience_scroll_overlap(scroll_position, position)
            {
                continue;
            }
            first_collision_entity.get_or_insert(player.entity_index);
            local_collision |= is_local;
        }
        let Some(first_collision_entity) = first_collision_entity else {
            return false;
        };
        self.pickup_events
            .push(Level2SharedSpellEvent::ExperienceScrollCollected {
                source_slot,
                player_entity_index: first_collision_entity,
            });
        if local_collision {
            let owner_index =
                local_player_index.expect("local player is present for a local collision");
            let enabled_holders = self.players[owner_index].enabled_spell_holders;
            let mut improved = Vec::new();
            let mut owner = self.players[owner_index].clone();
            for family in 0..PLAYER_SPELL_COUNT {
                if enabled_holders[family] == 0 {
                    continue;
                }
                let mut experience = owner.runtime_spell_experience[family]
                    .wrapping_add(LEVEL2_EXPERIENCE_SCROLL_AWARD);
                if family == usize::from(CASTLE_SPELL_FAMILY) {
                    experience = experience.min(7);
                }
                owner.runtime_spell_experience[family] = experience;
                owner.record[164 + family * std::mem::size_of::<i32>()
                    ..164 + (family + 1) * std::mem::size_of::<i32>()]
                    .copy_from_slice(&experience.to_le_bytes());
                self.spell_experience[family] = experience;
                let previous_level = owner.spell_unlocked_levels[family];
                let total = owner.persistent_spell_experience[family].wrapping_add(experience);
                let level = LEVEL2_SPELL_TIER_THRESHOLDS[family]
                    .iter()
                    .rposition(|threshold| total >= *threshold)
                    .unwrap_or(0) as i8;
                owner.spell_unlocked_levels[family] = level;
                owner.record[502 + family] = level.cast_unsigned();
                self.spell_levels[family] = level;
                if owner.family_selected_tiers[family] > level {
                    owner.family_selected_tiers[family] = level;
                    owner.record[528 + family] = level.cast_unsigned();
                }
                if level != previous_level {
                    improved.push((family as u8, level));
                }
            }
            self.players[owner_index] = owner.clone();
            mark_source_audio_call(SourceAudioCallTag::ExperienceScrollPickup);
            self.events.push(FlightEvent::PrepareSound {
                #[cfg(test)]
                audio_site: crate::MissionAudioSite::ExperienceScroll,
                entity_index: local_player_entity_index,
                player_index: i16::from(local_player_slot),
                sound_id: 63,
            });
            for (family, tier) in improved {
                mark_source_audio_call(SourceAudioCallTag::AbilityImprovedTier);
                self.pickup_events
                    .push(Level2SharedSpellEvent::AbilityImproved {
                        source_family: family,
                        tier,
                    });
                self.events.push(FlightEvent::PrepareSound {
                    #[cfg(test)]
                    audio_site: crate::MissionAudioSite::AbilityImproved,
                    entity_index: local_player_entity_index,
                    player_index: i16::from(local_player_slot),
                    sound_id: 61,
                });
            }
            self.local_player = owner;
        }
        true
    }

    #[must_use]
    pub fn acquisition_states(&self) -> &[SpellHolderAcquisitionState] {
        &self.acquisitions
    }

    #[must_use]
    pub fn persistent_acquisition_states(&self) -> &BTreeMap<u16, SpellHolderAcquisitionState> {
        &self.persistent_acquisitions
    }

    /// Reach the pinned generic `sub_68FF0` acquisition helper from the
    /// production Level 2 reset projection.  The caller owns collision
    /// detection (the source `sub_106C0` query); this method applies the
    /// source relation, hand, timer, action, player-record, and presentation
    /// writes once that predicate is true.
    pub fn acquire_level2_spell_holder(
        &mut self,
        family: u8,
        role: PhysicalSpellActionRole,
        overlaps: bool,
    ) -> Result<SpellHolderAcquisitionOutcome> {
        let owner_index = self
            .players
            .iter()
            .position(|player| player.entity_index == self.player_entity.index)
            .unwrap_or(0);
        self.acquire_level2_spell_holder_for_player(family, role, overlaps, owner_index)
    }

    fn acquire_level2_spell_holder_for_player(
        &mut self,
        family: u8,
        role: PhysicalSpellActionRole,
        overlaps: bool,
        owner_index: usize,
    ) -> Result<SpellHolderAcquisitionOutcome> {
        let index = self
            .acquisitions
            .iter()
            .position(|state| state.family == family)
            .ok_or_else(|| {
                NativeWorldError::new(format!("unsupported Level 2 spell family {family}"))
            })?;
        let owner = self.players[owner_index].clone();
        let family_index = usize::from(family);
        let left_spell_slot = owner.left_spell_slot_raw;
        let right_spell_slot = owner.right_spell_slot_raw;
        let left_subspell_index = owner.left_subspell_index;
        let right_subspell_index = owner.right_subspell_index;
        self.acquisitions[index].owner_entity_index = owner.entity_index;
        self.acquisitions[index].holder_relation = owner.enabled_spell_holders[usize::from(family)];
        // SetSpell reads the owner's current array_0x437 entry on every
        // acquisition; do not keep the reset-time tier when gameplay changes
        // the selected tier between source pulses.
        self.acquisitions[index].selected_tier = owner.family_selected_tiers[family_index];
        self.acquisitions[index].left_spell_slot = left_spell_slot;
        self.acquisitions[index].right_spell_slot = right_spell_slot;
        self.acquisitions[index].left_subspell_index = left_subspell_index;
        self.acquisitions[index].right_subspell_index = right_subspell_index;
        if let Some(pickup) = self.pickup(self.acquisitions[index].source_slot) {
            self.acquisitions[index].return_side = pickup.source_word_4a();
        }
        // `SetSpell_6D5E0` has a distinct active-holder branch: it only
        // rewrites word_0x2C and must leave the cold payload/hash projection
        // untouched.  Preserve those fields across the generic owner/hand
        // transition so the typed state follows the same branch.
        let active_setspell_projection = self
            .pickup(self.acquisitions[index].source_slot)
            .and_then(|pickup| {
                (u16::from_le_bytes(
                    pickup.record[50..52]
                        .try_into()
                        .expect("entity word 0x2e record extent"),
                ) != 0)
                    .then_some(self.acquisitions[index])
            });
        let return_side = self.acquisitions[index].return_side;
        let result = acquire_level2_spell_holder_with_return_side(
            &mut self.acquisitions[index],
            role,
            overlaps,
            return_side,
        );
        let outcome = match result {
            SpellHolderAcquisitionResult::Acquired(outcome) => outcome,
            SpellHolderAcquisitionResult::Rejected(failure) => {
                return Err(NativeWorldError::new(format!(
                    "Level 2 spell family {family} acquisition rejected: {failure:?}"
                )));
            }
        };
        if let Some(previous) = active_setspell_projection {
            let state = &mut self.acquisitions[index];
            state.subspell_index = previous.subspell_index;
            state.word_0x30 = previous.word_0x30;
            state.byte_0x3b = previous.byte_0x3b;
            state.byte_0x3c = previous.byte_0x3c;
            state.font_type_index = previous.font_type_index;
            state.mana_regen = previous.mana_regen;
            state.max_mana = previous.max_mana;
            state.mana = previous.mana;
        } else if self.free_spell_usage {
            // Cold `SetSpell_6D5E0` applies the global free-spell option only
            // after calculating the ordinary profile. `max_mana` remains the
            // source mana-cost result; only these two fields are overridden.
            let state = &mut self.acquisitions[index];
            state.mana_regen = 0;
            state.mana = 1;
        }
        let state = self.acquisitions[index];
        let owner_entity_index = self.players[owner_index].entity_index;
        {
            let owner = &mut self.players[owner_index];
            owner.enabled_spell_holders[family_index] = outcome.source_slot;
            owner.spell_hand_eligibility[family_index] = state.eligibility;
            owner.learned_spell_flags[family_index] = state.learned;
            owner.family_selected_tiers[family_index] = state.selected_tier;
            owner.record[268 + family_index * 2..270 + family_index * 2]
                .copy_from_slice(&outcome.source_slot.to_le_bytes());
            owner.record[398 + family_index] = state.eligibility;
            owner.record[424 + family_index] = state.learned;
            owner.record[528 + family_index] = state.selected_tier.to_le_bytes()[0];
            match outcome.hand {
                crate::spell_activation::SpellHand::Left => {
                    owner.left_spell_slot_raw = state.left_spell_slot;
                    owner.left_spell_slot = family;
                    owner.left_subspell_index = state.left_subspell_index;
                    owner.record[554..556].copy_from_slice(&state.left_spell_slot.to_le_bytes());
                    owner.record[558] = state.left_subspell_index.to_le_bytes()[0];
                }
                crate::spell_activation::SpellHand::Right => {
                    owner.right_spell_slot_raw = state.right_spell_slot;
                    owner.right_spell_slot = family;
                    owner.right_subspell_index = state.right_subspell_index;
                    owner.record[556..558].copy_from_slice(&state.right_spell_slot.to_le_bytes());
                    owner.record[559] = state.right_subspell_index.to_le_bytes()[0];
                }
            }
            if owner_index == 0 {
                self.local_player = owner.clone();
            }
        }
        if let Some(pickup) = self.pickup_mut(outcome.source_slot) {
            pickup.control_flags |= 1;
            pickup.record[16] |= 1;
            pickup.record[19] &= !2;
            pickup.control_flags = u32::from_le_bytes(
                pickup.record[16..20]
                    .try_into()
                    .expect("entity control flag record extent"),
            );
            pickup.record[44..46].copy_from_slice(&state.parent_entity_index.to_le_bytes());
            pickup.action = outcome.holder_action;
            pickup.record[73] = outcome.holder_action;
            pickup.record[58..60].copy_from_slice(&state.presentation_timer.to_le_bytes());
            // SetSpell_6D5E0's source-visible writes. An already-active
            // holder updates only word_0x2C; a cold holder receives the full
            // subspell/font/mana payload.
            let active_word_2e = u16::from_le_bytes(
                pickup.record[50..52]
                    .try_into()
                    .expect("entity word 0x2e record extent"),
            );
            if active_word_2e != 0 {
                let loc_spell_id = u16::from(
                    level2_applied_tier(state.selected_tier)
                        .expect("successful SetSpell keeps a non-negative tier"),
                ) + 1;
                pickup.record[48..50].copy_from_slice(&loc_spell_id.to_le_bytes());
            } else {
                pickup.subspell_index = state.subspell_index;
                pickup.record[46..48].copy_from_slice(&state.subspell_index.to_le_bytes());
                pickup.record[52..54].copy_from_slice(&state.word_0x30.to_le_bytes());
                pickup.activation_value = state.word_0x30;
                pickup.byte_3b = state.byte_0x3b;
                pickup.record[63] = state.byte_0x3b;
                pickup.byte_3c = 0;
                pickup.record[64] = 0;
                pickup.action_marker = 0;
                pickup.font_type_index = 0;
                pickup.record[65] = 0;
                pickup.mana_regen = state.mana_regen;
                pickup.record[140..144].copy_from_slice(&state.mana_regen.to_le_bytes());
                pickup.max_mana = state.max_mana;
                pickup.mana_cost = state.max_mana;
                pickup.record[144..148].copy_from_slice(&state.max_mana.to_le_bytes());
                pickup.mana = state.mana;
                pickup.record[148..152].copy_from_slice(&state.mana.to_le_bytes());
                let applied_tier = level2_applied_tier(state.selected_tier)
                    .expect("successful SetSpell keeps a non-negative tier");
                pickup.record[74] = applied_tier;
                pickup.level_gate = applied_tier.cast_signed();
            }
            // Successful `sub_68FF0` sets the pickup ownership bit; it does
            // not call DisableEntityDrawing. The latter is only applied by
            // the positive-life prefix when life decrements from one to zero.
            pickup.record[78..80].copy_from_slice(&0_i16.to_le_bytes());
        }
        // The active SetSpell branch only queues the newly selected tier in
        // word_0x2E; the running holder keeps its current word_0x30/mana
        // payload until cleanup. Read that post-branch projection before
        // seeding a typed active lifecycle so it cannot jump tiers early.
        let active_holder_projection = self.pickup(outcome.source_slot).cloned();
        if family == 6 {
            let parent = self
                .player_entities
                .get(owner_index)
                .cloned()
                .unwrap_or_else(|| self.player_entity.clone());
            let mut shield = ShieldState::new(
                level2_applied_tier(state.selected_tier)
                    .expect("successful Shield SetSpell keeps a non-negative tier"),
                outcome.source_slot,
                outcome.parent_entity_index,
                parent.mana,
                parent.life,
            )
            .expect("validated Level 2 Shield tier");
            shield.parent_mana_regen = parent.mana_regen;
            shield.parent_status_flags = parent.control_flags;
            self.shield = Some(shield);
            self.sync_shield_projection();
        } else if family == 11 {
            let parent = self
                .player_entities
                .get(owner_index)
                .cloned()
                .unwrap_or_else(|| self.player_entity.clone());
            let mut invisible = InvisibleState::new(
                level2_applied_tier(state.selected_tier)
                    .expect("successful Invisible SetSpell keeps a non-negative tier"),
                outcome.source_slot,
                outcome.parent_entity_index,
                parent.mana,
                parent.life,
            )
            .expect("validated Level 2 Invisible tier");
            invisible.parent_mana_regen = parent.mana_regen;
            invisible.parent_status_flags = parent.control_flags;
            self.invisible = Some(invisible);
            self.sync_invisible_projection();
        } else if family == BEYOND_SIGHT_FAMILY {
            let parent = self
                .player_entities
                .get(owner_index)
                .cloned()
                .unwrap_or_else(|| self.player_entity.clone());
            let mut beyond_sight = BeyondSightState::new(
                level2_applied_tier(state.selected_tier)
                    .expect("successful Beyond Sight SetSpell keeps a non-negative tier"),
                outcome.source_slot,
                outcome.parent_entity_index,
                parent.mana,
                parent.life,
            )
            .expect("validated Level 2 Beyond Sight tier");
            beyond_sight.parent_mana_regen = parent.mana_regen;
            if let Some(holder) = active_holder_projection.as_ref() {
                let active_word_2e = u16::from_le_bytes(
                    holder.record[50..52]
                        .try_into()
                        .expect("Beyond Sight active word extent"),
                );
                if active_word_2e != 0 {
                    beyond_sight.active_value = active_word_2e.cast_signed();
                    beyond_sight.activation_value = u16::from_le_bytes(
                        holder.record[52..54]
                            .try_into()
                            .expect("Beyond Sight activation word extent"),
                    )
                    .cast_signed();
                    beyond_sight.mana_cost = holder.max_mana;
                    beyond_sight.mana_regen_requirement = holder.mana_regen;
                    for tier in 0..=2 {
                        let Some(profile) = level2_set_spell_profile(BEYOND_SIGHT_FAMILY, tier)
                        else {
                            continue;
                        };
                        if beyond_sight.activation_value == profile.word_0x30.cast_signed()
                            && holder.max_mana == profile.max_mana
                            && holder.mana_regen == profile.mana_regen
                        {
                            beyond_sight.tier = tier;
                            break;
                        }
                    }
                }
            }
            self.beyond_sight = Some(beyond_sight);
            self.sync_beyond_sight_projection();
        }
        self.pickup_events
            .push(Level2SharedSpellEvent::SpellAcquired {
                source_slot: outcome.source_slot,
                player_entity_index: owner_entity_index,
                family,
                holder_action: outcome.holder_action,
            });
        mark_source_audio_call(SourceAudioCallTag::SpeedPickup);
        self.events.push(FlightEvent::PrepareSound {
            #[cfg(test)]
            audio_site: crate::MissionAudioSite::SpeedPickup,
            entity_index: owner_entity_index,
            player_index: -1,
            sound_id: 18,
        });
        Ok(outcome)
    }

    /// Source-pass adapter for the authored Level 2 pickup rows. This follows
    /// the `sub_68FF0` prefix before calling the owner transition: life is
    /// decremented first, the pickup is terrain-relaxed, and the source map
    /// scan considers only live model-0 players with an empty SpellEnabled
    /// relation. Rows 155/156 are admitted only after their authored event
    /// constructor materialises a live map entity.
    pub fn step_level2_spell_acquisition(
        &mut self,
        family: u8,
        player_entity_index: u16,
        player_position: Position,
    ) -> Result<Option<SpellHolderAcquisitionOutcome>> {
        let source_player_order = (0..self.players.len()).collect::<Vec<_>>();
        self.step_level2_spell_acquisition_with_castle_mana(
            family,
            player_entity_index,
            player_position,
            None,
            &source_player_order,
        )
    }

    fn step_level2_spell_acquisition_with_castle_mana(
        &mut self,
        family: u8,
        player_entity_index: u16,
        player_position: Position,
        parent_castle_mana: Option<i32>,
        source_player_order: &[usize],
    ) -> Result<Option<SpellHolderAcquisitionOutcome>> {
        if player_entity_index != self.player_entity.index {
            return Err(NativeWorldError::new(format!(
                "Level 2 spell acquisition received player slot {player_entity_index}, expected {}",
                self.player_entity.index
            )));
        }
        self.player_entity.position = player_position;
        self.player_entity.record[80..82].copy_from_slice(&player_position.x.to_le_bytes());
        self.player_entity.record[82..84].copy_from_slice(&player_position.y.to_le_bytes());
        self.player_entity.record[84..86].copy_from_slice(&player_position.z.to_le_bytes());
        if let Some(player_entity) = self
            .player_entities
            .iter_mut()
            .find(|entity| entity.index == player_entity_index)
        {
            player_entity.position = player_position;
            player_entity.record[80..82].copy_from_slice(&player_position.x.to_le_bytes());
            player_entity.record[82..84].copy_from_slice(&player_position.y.to_le_bytes());
            player_entity.record[84..86].copy_from_slice(&player_position.z.to_le_bytes());
        }
        let Some(index) = self
            .acquisitions
            .iter()
            .position(|state| state.family == family)
        else {
            return Err(NativeWorldError::new(format!(
                "unsupported Level 2 spell family {family}"
            )));
        };
        let source_slot = self.acquisitions[index].source_slot;
        let source_action = self
            .pickup(source_slot)
            .map(|pickup| pickup.action)
            .unwrap_or(self.acquisitions[index].source_action);
        let role = if physical_spell_action(family, PhysicalSpellActionRole::PersistentPickup)
            == Some(source_action)
        {
            PhysicalSpellActionRole::PersistentPickup
        } else if physical_spell_action(family, PhysicalSpellActionRole::OrdinaryPickup)
            == Some(source_action)
        {
            PhysicalSpellActionRole::OrdinaryPickup
        } else if physical_spell_action(family, PhysicalSpellActionRole::Active)
            == Some(source_action)
        {
            if family == 6 && source_action == SHIELD_ACTIVE_ACTION {
                self.step_shield_source_slot(source_slot);
            } else if family == 11 && source_action == INVISIBLE_ACTIVE_ACTION {
                self.step_invisible_source_slot(source_slot, parent_castle_mana);
            } else if family == BEYOND_SIGHT_FAMILY && source_action == BEYOND_SIGHT_ACTIVE_ACTION {
                self.step_beyond_sight_source_slot(source_slot, parent_castle_mana);
            }
            // The source cursor still reaches the live holder row, but its
            // pickup transition has already completed. The active family
            // lifecycles advance above; other active families remain
            // deliberately bounded to their acquisition seam.
            return Ok(None);
        } else {
            return Err(NativeWorldError::new(format!(
                "Level 2 spell family {family} has unsupported source action {source_action}"
            )));
        };
        if !self.acquisitions[index].step_source_life() {
            // `sub_68FF0` disables drawing exactly when the positive source
            // life reaches zero. Keep the raw entity byte17 bit2/dword 0x400
            // in lock-step with the typed cadence state.
            let source_life = self.acquisitions[index].source_life;
            if self.acquisitions[index].source_life == 0 {
                let source_slot = self.acquisitions[index].source_slot;
                if let Some(pickup) = self.pickup_mut(source_slot) {
                    pickup.life = source_life;
                    pickup.record[12..16].copy_from_slice(&source_life.to_le_bytes());
                    pickup.disable_drawing();
                }
                self.acquisitions[index].source_draw_enabled = false;
            } else {
                let source_slot = self.acquisitions[index].source_slot;
                if let Some(pickup) = self.pickup_mut(source_slot) {
                    pickup.life = source_life;
                    pickup.record[12..16].copy_from_slice(&source_life.to_le_bytes());
                }
            }
            return Ok(None);
        }
        if self.pickup(source_slot).is_none() {
            return Ok(None);
        }
        let mut pickup = self
            .pickup(source_slot)
            .cloned()
            .expect("validated Level 2 spell source row remains live");
        let ground = self.terrain.ground_height(pickup.position);
        if pickup.position.z > ground {
            pickup.position.z = pickup.position.z.wrapping_sub(128);
        }
        if pickup.position.z <= ground {
            pickup.position.z = ground;
        }
        pickup.record[84..86].copy_from_slice(&pickup.position.z.to_le_bytes());
        // Source `byte_0x3E_62` is the action cadence byte at record 66,
        // distinct from the byte0 control flags at record 16.
        if pickup.phase & 3 != 0 {
            if let Some(live) = self.pickup_mut(source_slot) {
                live.clone_from(&pickup);
            }
            return Ok(None);
        }
        // First source-list pass: the active level/player-colour owner marks
        // the pickup's byte0 flag before the collision owner pass. Level 2's
        // local source colour is zero (the same `LevelIndex_0xc` selector
        // used by the pinned routine); this branch is intentionally kept
        // separate from the SpellEnabled collision gate below.
        if pickup.control_flags & 1 == 0
            && source_player_order.iter().copied().any(|player_index| {
                let Some(player) = self.players.get(player_index) else {
                    return false;
                };
                player.enabled_spell_holders[usize::from(family)] != 0
                    && player.player_color_index() == 0
                    && self
                        .player_entities
                        .get(player_index)
                        .is_some_and(|entity| {
                            entity.active && entity.model == 0 && entity.life >= 0
                        })
            })
        {
            pickup.control_flags |= 1;
            pickup.record[16] |= 1;
        }
        let mut owner_index = None;
        for &player_index in source_player_order {
            let Some(player) = self.players.get(player_index) else {
                continue;
            };
            if player.enabled_spell_holders[usize::from(family)] != 0 {
                continue;
            }
            let Some(entity) = self.player_entities.get(player_index) else {
                continue;
            };
            // The source linked-list filter is model==0 and life>=0. The
            // collision query itself is exactly `sub_106C0`.
            if entity.active
                && entity.model == 0
                && entity.life >= 0
                && level2_source_intersects(&pickup, entity)
            {
                owner_index = Some(player_index);
                break;
            }
        }
        let Some(owner_index) = owner_index else {
            if let Some(live) = self.pickup_mut(source_slot) {
                live.clone_from(&pickup);
            }
            return Ok(None);
        };
        if let Some(live) = self.pickup_mut(source_slot) {
            live.clone_from(&pickup);
        }
        self.acquire_level2_spell_holder_for_player(family, role, true, owner_index)
            .map(Some)
    }

    /// Production source-dispatch wrapper for `sub_69250`: when the live
    /// source carries a persistent action, the successful holder transition
    /// creates the next replacement in the shared economy arena and binds it
    /// back into this same entity projection for the run-again cursor.
    pub fn step_level2_spell_acquisition_with_economy(
        &mut self,
        economy: &mut Level2EconomyState,
        family: u8,
        player_entity_index: u16,
        player_position: Position,
    ) -> Result<Option<SpellHolderAcquisitionOutcome>> {
        let source_player_order = (0..self.players.len()).collect::<Vec<_>>();
        self.step_level2_spell_acquisition_with_economy_in_source_order(
            economy,
            family,
            player_entity_index,
            player_position,
            &source_player_order,
        )
    }

    fn step_level2_spell_acquisition_with_economy_in_source_order(
        &mut self,
        economy: &mut Level2EconomyState,
        family: u8,
        player_entity_index: u16,
        player_position: Position,
        source_player_order: &[usize],
    ) -> Result<Option<SpellHolderAcquisitionOutcome>> {
        let active_parent = self
            .invisible
            .and_then(|invisible| {
                (invisible.holder_entity_index
                    == self
                        .acquisitions
                        .iter()
                        .find(|state| state.family == family)
                        .map_or(0, |state| state.source_slot))
                .then_some(invisible.parent_entity_index)
            })
            .or_else(|| {
                self.beyond_sight.and_then(|beyond_sight| {
                    (beyond_sight.holder_entity_index
                        == self
                            .acquisitions
                            .iter()
                            .find(|state| state.family == family)
                            .map_or(0, |state| state.source_slot))
                    .then_some(beyond_sight.parent_entity_index)
                })
            });
        let parent_castle_mana = economy
            .castle()
            .filter(|castle| {
                castle.active
                    && matches!(family, 11 | BEYOND_SIGHT_FAMILY)
                    && active_parent == Some(castle.owner_entity_index)
            })
            .map(|castle| castle.mana);
        let result = self.step_level2_spell_acquisition_with_castle_mana(
            family,
            player_entity_index,
            player_position,
            parent_castle_mana,
            source_player_order,
        )?;
        let Some(outcome) = result else {
            return Ok(None);
        };
        if outcome.replacement_action.is_none() {
            return Ok(Some(outcome));
        }
        let economy_before = economy.clone();
        let position = economy
            .mark_spell_replacement_acquired(outcome.source_slot, outcome.holder_action)
            .ok_or_else(|| {
                NativeWorldError::new(format!(
                    "Level 2 persistent source {} is absent from the economy arena",
                    outcome.source_slot
                ))
            })?;
        let Some(replacement) =
            economy.allocate_spell_replacement(family, outcome.parent_entity_index, position)
        else {
            // `sub_69250` keeps the successful holder transition even when
            // NewEvent has no free row; the next replacement is simply
            // absent until a later source creates one.
            return Ok(Some(outcome));
        };
        if let Err(error) = self.materialize_spell_replacement_pickup(replacement) {
            *economy = economy_before;
            return Err(error);
        }
        Ok(Some(outcome))
    }

    /// Dispatch one economy-owned persistent source without aliasing the
    /// authored ordinary source of the same family. The source game can keep
    /// both rows live at once, so the temporary family slot is restored after
    /// this source-local transition and the replacement state is keyed by its
    /// own arena slot.
    pub fn step_level2_spell_replacement_acquisition_with_economy(
        &mut self,
        economy: &mut Level2EconomyState,
        source_slot: u16,
        family: u8,
        player_entity_index: u16,
        player_position: Position,
    ) -> Result<Option<SpellHolderAcquisitionOutcome>> {
        let source_player_order = (0..self.players.len()).collect::<Vec<_>>();
        self.step_level2_spell_replacement_acquisition_with_economy_in_source_order(
            economy,
            source_slot,
            family,
            player_entity_index,
            player_position,
            &source_player_order,
        )
    }

    fn step_level2_spell_replacement_acquisition_with_economy_in_source_order(
        &mut self,
        economy: &mut Level2EconomyState,
        source_slot: u16,
        family: u8,
        player_entity_index: u16,
        player_position: Position,
        source_player_order: &[usize],
    ) -> Result<Option<SpellHolderAcquisitionOutcome>> {
        let family_index = self
            .acquisitions
            .iter()
            .position(|state| state.family == family)
            .ok_or_else(|| {
                NativeWorldError::new(format!("unsupported Level 2 spell family {family}"))
            })?;
        let pickup = self.pickup(source_slot).cloned().ok_or_else(|| {
            NativeWorldError::new(format!(
                "Level 2 persistent source row {source_slot} is absent"
            ))
        })?;
        let base_state = self.acquisitions[family_index];
        let mut source_state = self
            .persistent_acquisitions
            .remove(&source_slot)
            .unwrap_or(base_state);
        source_state.source_slot = source_slot;
        source_state.authored_source_index = 0;
        source_state.source_action = pickup.action;
        source_state.source_life = pickup.life;
        source_state.source_draw_enabled = pickup.control_flags & ENTITY_DRAW_DISABLED_BIT == 0;
        self.acquisitions[family_index] = source_state;
        let result = self.step_level2_spell_acquisition_with_castle_mana(
            family,
            player_entity_index,
            player_position,
            None,
            source_player_order,
        );
        let result_state = self.acquisitions[family_index];
        self.acquisitions[family_index] = base_state;
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(error) => {
                self.persistent_acquisitions
                    .insert(source_slot, result_state);
                return Err(error);
            }
        };
        let Some(outcome) = outcome else {
            self.persistent_acquisitions
                .insert(source_slot, result_state);
            return Ok(None);
        };
        if outcome.replacement_action.is_none() {
            return Ok(Some(outcome));
        }

        let economy_before = economy.clone();
        let position = economy
            .mark_spell_replacement_acquired(source_slot, outcome.holder_action)
            .ok_or_else(|| {
                NativeWorldError::new(format!(
                    "Level 2 persistent source {source_slot} is absent from the economy arena"
                ))
            })?;
        let Some(replacement) =
            economy.allocate_spell_replacement(family, outcome.parent_entity_index, position)
        else {
            // The holder transition has succeeded; a full source arena simply
            // means there is no next persistent row to materialise.
            return Ok(Some(outcome));
        };
        if let Err(error) = self.materialize_spell_replacement_pickup(replacement) {
            *economy = economy_before;
            self.persistent_acquisitions
                .insert(source_slot, result_state);
            return Err(error);
        }
        let mut next_state = result_state;
        next_state.source_slot = replacement.source_entity_index;
        next_state.authored_source_index = 0;
        next_state.source_action = replacement.action;
        next_state.source_life = 0;
        next_state.source_draw_enabled = true;
        self.persistent_acquisitions
            .insert(replacement.source_entity_index, next_state);
        Ok(Some(outcome))
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
        self.step_speed_pickup_inner(player_entity_index, player_position, true)
    }

    /// Source `UpdateEntities` keeps one presentation/event stream across
    /// ascending entity slots. The outer dispatcher therefore uses this
    /// preserving variant; isolated direct calls retain the public method's
    /// per-call event reset.
    fn step_speed_pickup_in_source_order(
        &mut self,
        player_entity_index: u16,
        player_position: Position,
    ) -> Result<Level2SpeedPickupStep> {
        self.step_speed_pickup_inner(player_entity_index, player_position, false)
    }

    fn step_speed_pickup_inner(
        &mut self,
        player_entity_index: u16,
        player_position: Position,
        clear_events: bool,
    ) -> Result<Level2SpeedPickupStep> {
        if player_entity_index != self.player_entity.index {
            return Err(NativeWorldError::new(format!(
                "Level 2 Speed pickup received player slot {player_entity_index}, expected {}",
                self.player_entity.index
            )));
        }
        if clear_events {
            self.events.clear();
        }
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
                .step_speed_pickup_in_source_order(player_entity_index, player_position)
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
        for entity in &self.dynamic_pickups {
            hasher.update(entity.record_bytes());
        }
        hasher.update(self.player_entity.record_bytes());
        for player in &self.players {
            hasher.update(player.record_bytes());
        }
        for entity in &self.player_entities {
            hasher.update(entity.record_bytes());
        }
        hasher.update(self.local_player.record_bytes());
        hasher.update(self.player_entity_sha256.as_bytes());
        hasher.update(self.terrain.deterministic_hash().to_le_bytes());
        hasher.update(self.player_mana.to_le_bytes());
        hasher.update(self.player_life_regen.to_le_bytes());
        for experience in self.persistent_spell_experience {
            hasher.update(experience.to_le_bytes());
        }
        for experience in self.spell_experience {
            hasher.update(experience.to_le_bytes());
        }
        for level in self.spell_levels {
            hasher.update(level.to_le_bytes());
        }
        hasher.update(self.speed.entity_index.to_le_bytes());
        hasher.update(self.speed.active_value.to_le_bytes());
        hasher.update(self.speed.activation_value.to_le_bytes());
        hasher.update(self.speed.mana_cost.to_le_bytes());
        if let Some(shield) = self.shield {
            hasher.update([1]);
            hasher.update(shield.holder_entity_index.to_le_bytes());
            hasher.update(shield.parent_entity_index.to_le_bytes());
            hasher.update([shield.tier, shield.action]);
            hasher.update(shield.active_value.to_le_bytes());
            hasher.update(shield.activation_value.to_le_bytes());
            hasher.update(shield.mana_cost.to_le_bytes());
            hasher.update(shield.parent_mana.to_le_bytes());
            hasher.update(shield.parent_life.to_le_bytes());
            hasher.update(shield.parent_mana_regen.to_le_bytes());
            hasher.update(shield.parent_status_flags.to_le_bytes());
            hasher.update(shield.countdown.to_le_bytes());
            hasher.update(shield.experience_awarded.to_le_bytes());
            hasher.update([u8::from(shield.cleanup_requested)]);
        } else {
            hasher.update([0]);
        }
        if let Some(mailbox) = self.shield_damage_mailbox {
            hasher.update(b"shield-damage-mailbox-v1");
            hasher.update(mailbox.target_entity_index.to_le_bytes());
            hasher.update(mailbox.damage.to_le_bytes());
            hasher.update(mailbox.attacker_entity_index.to_le_bytes());
        }
        if let Some(invisible) = self.invisible {
            hasher.update([1]);
            hasher.update(invisible.holder_entity_index.to_le_bytes());
            hasher.update(invisible.parent_entity_index.to_le_bytes());
            hasher.update([invisible.tier, invisible.action]);
            hasher.update(invisible.active_value.to_le_bytes());
            hasher.update(invisible.activation_value.to_le_bytes());
            hasher.update(invisible.mana_cost.to_le_bytes());
            hasher.update(invisible.mana_regen_requirement.to_le_bytes());
            hasher.update(invisible.parent_mana.to_le_bytes());
            hasher.update(invisible.parent_life.to_le_bytes());
            hasher.update(invisible.parent_mana_regen.to_le_bytes());
            hasher.update(invisible.parent_status_flags.to_le_bytes());
            hasher.update(invisible.spell_status_flags.to_le_bytes());
            hasher.update([invisible.parent_visibility_mode]);
            hasher.update([u8::from(invisible.owner_word_clear_pending)]);
            hasher.update(invisible.countdown.to_le_bytes());
            hasher.update(invisible.experience_awarded.to_le_bytes());
            hasher.update([u8::from(invisible.cleanup_requested)]);
        } else {
            hasher.update([0]);
        }
        if let Some(beyond_sight) = self.beyond_sight {
            hasher.update([1]);
            hasher.update(beyond_sight.holder_entity_index.to_le_bytes());
            hasher.update(beyond_sight.parent_entity_index.to_le_bytes());
            hasher.update([beyond_sight.tier, beyond_sight.action]);
            hasher.update(beyond_sight.active_value.to_le_bytes());
            hasher.update(beyond_sight.activation_value.to_le_bytes());
            hasher.update(beyond_sight.mana_cost.to_le_bytes());
            hasher.update(beyond_sight.mana_regen_requirement.to_le_bytes());
            hasher.update(beyond_sight.parent_mana.to_le_bytes());
            hasher.update(beyond_sight.parent_life.to_le_bytes());
            hasher.update(beyond_sight.parent_mana_regen.to_le_bytes());
            hasher.update(beyond_sight.countdown.to_le_bytes());
            hasher.update(beyond_sight.experience_awarded.to_le_bytes());
            hasher.update([u8::from(beyond_sight.cleanup_requested)]);
        } else {
            hasher.update([0]);
        }
        hasher.update(self.beyond_sight_auxiliary.entity_index.to_le_bytes());
        hasher.update(self.beyond_sight_auxiliary.next_entity.to_le_bytes());
        hasher.update(self.beyond_sight_auxiliary.entity_index2.to_le_bytes());
        hasher.update([self.beyond_sight_auxiliary.effect_parameter]);
        // Preserve the established false-state hash while still binding the
        // newly reachable true state. No prior valid session could carry the
        // source option, so the tagged true branch is collision-free with the
        // old state space without needlessly invalidating existing baselines.
        if self.free_spell_usage {
            hasher.update(b"level2-free-spell-usage-v1");
            hasher.update([1]);
        }
        for state in &self.acquisitions {
            hasher.update([state.family]);
            hasher.update(state.source_slot.to_le_bytes());
            hasher.update(state.authored_source_index.to_le_bytes());
            hasher.update(state.owner_entity_index.to_le_bytes());
            hasher.update([state.source_action, state.holder_action]);
            hasher.update(state.replacement_action.unwrap_or(u8::MAX).to_le_bytes());
            hasher.update(state.parent_entity_index.to_le_bytes());
            hasher.update(state.presentation_timer.to_le_bytes());
            hasher.update(state.holder_relation.to_le_bytes());
            hasher.update([state.eligibility]);
            hasher.update([state.learned]);
            hasher.update(state.left_spell_slot.to_le_bytes());
            hasher.update(state.right_spell_slot.to_le_bytes());
            hasher.update([
                state.left_subspell_index.to_le_bytes()[0],
                state.right_subspell_index.to_le_bytes()[0],
                state.selected_tier.to_le_bytes()[0],
                u8::from(state.source_draw_enabled),
            ]);
            hasher.update(state.subspell_index.to_le_bytes());
            hasher.update(state.word_0x30.to_le_bytes());
            hasher.update([state.byte_0x3b]);
            hasher.update([state.byte_0x3c, state.font_type_index]);
            hasher.update(state.mana_regen.to_le_bytes());
            hasher.update(state.max_mana.to_le_bytes());
            hasher.update(state.mana.to_le_bytes());
            hasher.update(state.source_life.to_le_bytes());
            hasher.update(state.return_side.to_le_bytes());
        }
        for (source_slot, state) in &self.persistent_acquisitions {
            hasher.update([0xfe]);
            hasher.update(source_slot.to_le_bytes());
            hasher.update([state.family, state.source_action, state.holder_action]);
            hasher.update(state.source_slot.to_le_bytes());
            hasher.update(state.holder_relation.to_le_bytes());
            hasher.update([state.eligibility, state.learned]);
            hasher.update(state.source_life.to_le_bytes());
            hasher.update([u8::from(state.source_draw_enabled)]);
        }
        hasher.update(self.speed_pickup_state.owner_entity_index.to_le_bytes());
        hasher.update([u8::from(self.speed_pickup_state.collected)]);
        hasher.update([u8::from(self.speed_pickup_state.visible)]);
        hasher.update([self.speed_pickup_state.state_timer]);
        format!("{:x}", hasher.finalize())
    }

    pub fn reset(&mut self) {
        self.pickups.clone_from(&self.baseline_pickups);
        self.dynamic_pickups
            .clone_from(&self.baseline_dynamic_pickups);
        self.authored_pickup_slots
            .clone_from(&self.baseline_authored_pickup_slots);
        self.terrain = self.baseline_terrain.clone();
        self.holders.clone_from(&self.baseline_holders);
        self.player_mana = self.baseline_player_mana;
        self.player_life_regen = self.baseline_player_life_regen;
        self.player_entity = self.baseline_player_entity.clone();
        self.players.clone_from(&self.baseline_players);
        self.player_entities
            .clone_from(&self.baseline_player_entities);
        self.local_player = self.baseline_local_player.clone();
        self.speed_pickup_state = self.baseline_speed_pickup_state;
        self.caster = self.baseline_caster;
        self.fireball = self.baseline_fireball;
        self.possession = self.baseline_possession;
        self.speed = self.baseline_speed;
        self.shield = self.baseline_shield;
        self.shield_damage_mailbox = self.baseline_shield_damage_mailbox;
        self.invisible = self.baseline_invisible;
        self.beyond_sight = self.baseline_beyond_sight;
        self.beyond_sight_auxiliary = self.baseline_beyond_sight_auxiliary;
        self.acquisitions.clone_from(&self.baseline_acquisitions);
        self.free_spell_usage = self.baseline_free_spell_usage;
        self.persistent_acquisitions
            .clone_from(&self.baseline_persistent_acquisitions);
        self.persistent_spell_experience = self.baseline_persistent_spell_experience;
        self.spell_experience = self.baseline_spell_experience;
        self.spell_levels = self.baseline_spell_levels;
        self.events.clear();
        self.pickup_events.clear();
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level2OrdinaryResetSession {
    baseline_terrain: SessionTerrain,
    pub terrain: SessionTerrain,
    house_build_footprints: BTreeMap<u8, CastleBuildFootprint>,
    baseline_houses: HouseFamily,
    houses: HouseFamily,
    baseline_villagers: VillagerFamily,
    villagers: VillagerFamily,
    baseline_dolmens: Box<[Level2Dolmen]>,
    dolmens: Box<[Level2Dolmen]>,
    baseline_players: Box<[EntitySnapshot]>,
    players: Box<[EntitySnapshot]>,
    baseline_population_census: u32,
    population_census: u32,
}

impl Level2OrdinaryResetSession {
    fn from_parts(
        terrain: SessionTerrain,
        entities: Box<[EntitySnapshot]>,
        players: Box<[PlayerSnapshot]>,
        presentation: PresentationSelectors,
        house_build_footprints: BTreeMap<u8, CastleBuildFootprint>,
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
        let population_census = u32::try_from(
            villagers
                .villagers()
                .iter()
                .filter(|villager| villager.active)
                .count(),
        )
        .expect("Level 2 ordinary population census fits u32");
        Ok(Self {
            baseline_terrain: terrain.clone(),
            terrain,
            house_build_footprints,
            baseline_houses: houses.clone(),
            houses,
            baseline_villagers: villagers.clone(),
            villagers,
            baseline_dolmens: dolmens.clone(),
            dolmens,
            baseline_players: player_entities.clone(),
            players: player_entities,
            baseline_population_census: population_census,
            population_census,
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
        hasher.update(self.population_census.to_le_bytes());
        hasher
            .update(format!("{:?}{:?}{:?}", self.houses, self.villagers, self.dolmens).as_bytes());
        for entity in &self.players {
            hasher.update(entity.record_bytes());
        }
        format!("{:x}", hasher.finalize())
    }

    /// Source `sub_1B8C0` decrements the class-5 population census when a
    /// movement attempt exhausts all headings and kills the resident. Keep
    /// this counter in the reset-scoped ordinary package; the Level 2 mission
    /// allocator is a separate, later sink and must not be inferred here.
    #[must_use]
    pub const fn population_census(&self) -> u32 {
        self.population_census
    }

    fn apply_villager_effects(&mut self, effects: &[VillagerStepEffect]) {
        for effect in effects {
            if matches!(effect, VillagerStepEffect::PopulationCensusDecrement { .. }) {
                self.population_census = self.population_census.saturating_sub(1);
            }
        }
    }

    pub fn step_house_action_52(&mut self, source_slot: u16) -> Vec<HouseStepEvent> {
        self.houses.step_source_slot(source_slot, &self.terrain)
    }

    fn apply_house_terrain_events(
        &mut self,
        economy: &mut Level2EconomyState,
        events: &[HouseStepEvent],
    ) -> Result<()> {
        for event in events {
            match *event {
                HouseStepEvent::ConstructionPass {
                    position,
                    building_index,
                    remaining,
                    ..
                } => {
                    let footprint = self
                        .house_build_footprints
                        .get(&building_index)
                        .ok_or_else(|| {
                            NativeWorldError::new(format!(
                                "Level 2 House BUILD footprint {building_index} is absent"
                            ))
                        })?;
                    self.terrain
                        .apply_house_construction(position, Some(remaining), footprint)
                        .map_err(NativeWorldError::new)?;
                }
                HouseStepEvent::ConstructionCompleted {
                    position,
                    building_index,
                    ..
                } => {
                    let footprint = self
                        .house_build_footprints
                        .get(&building_index)
                        .ok_or_else(|| {
                            NativeWorldError::new(format!(
                                "Level 2 House BUILD footprint {building_index} is absent"
                            ))
                        })?;
                    self.terrain
                        .apply_house_construction(position, None, footprint)
                        .map_err(NativeWorldError::new)?;
                }
                HouseStepEvent::RemoveFootprint {
                    entity_index,
                    attacker_entity_index,
                    position,
                    building_index,
                    occupants,
                    rng,
                    ..
                } => {
                    let produces_residents = self
                        .houses
                        .houses()
                        .iter()
                        .find(|house| house.entity_index == entity_index)
                        .is_some_and(|house| house.parameter.produces_villagers());
                    let footprint = self
                        .house_build_footprints
                        .get(&building_index)
                        .ok_or_else(|| {
                            NativeWorldError::new(format!(
                                "Level 2 House BUILD footprint {building_index} is absent"
                            ))
                        })?;
                    let outcome = self
                        .terrain
                        .apply_house_removal(
                            position,
                            rng,
                            occupants,
                            produces_residents,
                            footprint,
                        )
                        .map_err(NativeWorldError::new)?;
                    let owner_entity_index = self
                        .houses
                        .houses()
                        .iter()
                        .find(|house| house.entity_index == entity_index)
                        .map_or(0, |house| house.owner_entity_index);
                    let requests = outcome
                        .residents
                        .iter()
                        .map(|resident| {
                            (
                                entity_index,
                                owner_entity_index,
                                HouseResidentRequest {
                                    position: resident.position,
                                    model: resident.model,
                                    action: resident.action,
                                    attacker_entity_index,
                                },
                            )
                        })
                        .collect::<Vec<_>>();
                    if !requests.is_empty()
                        && economy.allocate_house_residents_atomic(&requests).is_none()
                    {
                        return Err(NativeWorldError::new(format!(
                            "Level 2 House {entity_index} resident allocation failed"
                        )));
                    }
                    self.houses
                        .commit_removal_state(
                            entity_index,
                            outcome.final_rng,
                            outcome.final_occupants,
                        )
                        .map_err(NativeWorldError::new)?;
                    economy.disable_house_source_slot(entity_index)?;
                }
                HouseStepEvent::ResidentRequested {
                    entity_index,
                    attacker_entity_index,
                    position,
                    model,
                    action,
                } => {
                    let owner_entity_index = self
                        .houses
                        .houses()
                        .iter()
                        .find(|house| house.entity_index == entity_index)
                        .map_or(0, |house| house.owner_entity_index);
                    let request = HouseResidentRequest {
                        position,
                        model,
                        action,
                        attacker_entity_index,
                    };
                    if economy
                        .allocate_house_residents_atomic(&[(
                            entity_index,
                            owner_entity_index,
                            request,
                        )])
                        .is_none()
                    {
                        return Err(NativeWorldError::new(format!(
                            "Level 2 House {entity_index} resident allocation failed"
                        )));
                    }
                }
                HouseStepEvent::Possessed {
                    entity_index,
                    owner_entity_index,
                    payload: _,
                } => {
                    economy.prepare_sound(entity_index, owner_entity_index, -1, 4);
                    economy.set_house_owner_relation(entity_index, owner_entity_index);
                }
                HouseStepEvent::Damaged { .. } | HouseStepEvent::DestructionStarted { .. } => {}
            }
        }
        Ok(())
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

    pub fn submit_house_possession(
        &mut self,
        source_slot: u16,
        owner_entity_index: u16,
        payload: u32,
    ) -> Result<()> {
        self.houses
            .submit_possession(source_slot, owner_entity_index, payload)
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
        self.population_census = self.baseline_population_census;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Level2Group10Switch25State {
    source_slot: u16,
    authored_index: u16,
    target_group: u16,
    position: Position,
    phase: u8,
}

/// Callback state for a group-zero class-11 model-0/model-1 producer that is
/// already present in the imported reset entity array.  The entity remains
/// the visible/source authority; this wrapper only carries the mutable
/// callback words needed to execute the generic stage-tag chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Level2ImportedSwitchRootState {
    source_slot: u16,
    authored_index: u16,
    model: u8,
    target_group: u16,
    word_10: i16,
    position: Position,
    phase: u8,
    cooldown: u8,
    drawing_enabled: bool,
    active: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level2ResetSession {
    profile: FlightProfile,
    package_sha256: String,
    collision_masks: Option<CollisionMaskBank>,
    baseline_state: FlightState,
    pub state: FlightState,
    baseline_terrain: SessionTerrain,
    pub terrain: SessionTerrain,
    baseline_context: FlightContext,
    pub context: FlightContext,
    baseline_entities: Box<[EntitySnapshot]>,
    entities: Box<[EntitySnapshot]>,
    baseline_players: Box<[PlayerSnapshot]>,
    players: Box<[PlayerSnapshot]>,
    baseline_runtime: RuntimeSnapshot,
    runtime: RuntimeSnapshot,
    presentation: PresentationSelectors,
    baseline_bad_stones: Box<[Level2BadStone]>,
    bad_stones: Box<[Level2BadStone]>,
    baseline_shared_spells: Level2SharedSpellResetSession,
    shared_spells: Level2SharedSpellResetSession,
    economy: Level2EconomyState,
    ordinary: Level2OrdinaryResetSession,
    baseline_goats: GoatFamily,
    goats: GoatFamily,
    mission: MissionContentSession,
    baseline_nyphur_dispatcher: Level2NyphurDispatcher,
    nyphur_dispatcher: Level2NyphurDispatcher,
    baseline_nyphur_dispatch_traces: Vec<Level2NyphurDispatchTrace>,
    nyphur_dispatch_traces: Vec<Level2NyphurDispatchTrace>,
    /// Source `sub_11900` player damage mailbox.  The Bee contact writes this
    /// during its source-slot callback; the local-player route consumes it on
    /// the next source-ordered visit.
    baseline_player_pending_damage: i32,
    player_pending_damage: i32,
    baseline_player_pending_attacker: u16,
    player_pending_attacker: u16,
    baseline_player_life: i32,
    player_life: i32,
    baseline_level2_day_hud_color_phase_1: u8,
    level2_day_hud_color_phase_1: u8,
    baseline_level2_day_hud_color_phase_2: u8,
    level2_day_hud_color_phase_2: u8,
    baseline_level2_day_hud_hit_frame_time_pre_draw: u8,
    level2_day_hud_hit_frame_time_pre_draw: u8,
    baseline_level2_day_hud_player_panel_selector: u16,
    level2_day_hud_player_panel_selector: u16,
    dispatch_events: Vec<Level2DispatchEvent>,
    economy_event_cursor: usize,
    baseline_level2_group8_activated: bool,
    level2_group8_activated: bool,
    /// Mutable selected-objective stage state consumed by class-11/model-32
    /// switches.  The qualified reset starts at one; objective progression
    /// owns the later transition to exactly two.
    baseline_level2_selected_objective_stage_state: u8,
    level2_selected_objective_stage_state: u8,
    baseline_level2_objective_pause: u8,
    level2_objective_pause: u8,
    baseline_level2_objective_done: bool,
    level2_objective_done: bool,
    /// Authored class-10/model-54 rows already consumed by a rooted class-11
    /// stage-tag producer.  The source can run the same group again with
    /// `a2 == 0`; retaining the authored identity prevents a normal outer
    /// turn from repeatedly constructing the same temporary field.
    baseline_level2_mana_attraction_activated: BTreeSet<u16>,
    level2_mana_attraction_activated: BTreeSet<u16>,
    baseline_level2_model13_activated: BTreeSet<u16>,
    level2_model13_activated: BTreeSet<u16>,
    baseline_level2_bees_activated: BTreeSet<u16>,
    level2_bees_activated: BTreeSet<u16>,
    baseline_level2_switch_producers_activated: BTreeSet<u16>,
    level2_switch_producers_activated: BTreeSet<u16>,
    baseline_level2_model32_activated: BTreeSet<u16>,
    level2_model32_activated: BTreeSet<u16>,
    baseline_level2_model76_activated: BTreeSet<u16>,
    level2_model76_activated: BTreeSet<u16>,
    /// Authored class-10/model-22 wind rows consumed by one-shot switches.
    /// The source disables the authored feature after dispatch, so a later
    /// parent expiry must not rematerialize the same row.
    baseline_level2_model22_activated: BTreeSet<u16>,
    level2_model22_activated: BTreeSet<u16>,
    /// Authored class-10/subtype-39 rows consumed by a live stage-tag
    /// producer. The source group scan can run again, so retain authored
    /// identity even though the typed consumers live in the sphere arena.
    baseline_level2_model39_activated: BTreeSet<u16>,
    level2_model39_activated: BTreeSet<u16>,
    /// Authored class-10/model-6 rows consumed by the rooted group-24
    /// producer. The group scan is repeatable, so keep authored identity
    /// separate from the live model-6 arena rows.
    baseline_level2_model6_activated: BTreeSet<u16>,
    level2_model6_activated: BTreeSet<u16>,
    baseline_level2_model59_activated: BTreeSet<u16>,
    level2_model59_activated: BTreeSet<u16>,
    baseline_level2_model53_activated: BTreeSet<u16>,
    level2_model53_activated: BTreeSet<u16>,
    baseline_level2_model1_activated: BTreeSet<u16>,
    level2_model1_activated: BTreeSet<u16>,
    baseline_level2_model17_activated: BTreeSet<u16>,
    level2_model17_activated: BTreeSet<u16>,
    baseline_level2_imported_switch_roots: Box<[Level2ImportedSwitchRootState]>,
    level2_imported_switch_roots: Box<[Level2ImportedSwitchRootState]>,
    baseline_level2_group10_switch25: Option<Level2Group10Switch25State>,
    level2_group10_switch25: Option<Level2Group10Switch25State>,
    /// Authored class-14 subtype-3/5 rows already consumed by an active
    /// class-11 stage-tag producer.  The source group scan is repeatable, so
    /// retain authored identity to prevent duplicate arena rows on later
    /// source passes.
    baseline_level2_class14_activated: BTreeSet<u16>,
    level2_class14_activated: BTreeSet<u16>,
    /// Authored class-2/model-0 tree rows already consumed by the group-31
    /// LevelEnd producer.  Keep this separate from class-14 activation so a
    /// malformed partial group cannot be mistaken for a complete transition.
    baseline_level2_tree_activated: BTreeSet<u16>,
    level2_tree_activated: BTreeSet<u16>,
    /// Authored class-5/model-3/9 rows consumed by the model-45 group-22
    /// callback. Retain authored identity across repeatable source scans.
    baseline_level2_class5_activated: BTreeSet<u16>,
    level2_class5_activated: BTreeSet<u16>,
    /// Authored class-10/model-45 rows consumed by the bounded groups 3 and
    /// 8 scans. The source group scan is repeatable, but each authored row is
    /// consumed only once when `a2 == 1`.
    baseline_level2_model45_activated: BTreeSet<u16>,
    level2_model45_activated: BTreeSet<u16>,
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
        || entities
            .get(usize::from(entity.source_word_96()))
            .is_some_and(|target| entity.source_word_98() == level2_nyphur_source_identity(target));
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
