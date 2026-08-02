//! Package-backed, handle-based ABI v2 for native presentation clients.
//!
//! All paths and data are supplied explicitly by the caller. This module does
//! not use the embedded owner regression bytes retained by the legacy v1 ABI.

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
    slice, str,
    sync::{Arc, Mutex, OnceLock},
};

use sha2::{Digest, Sha256};
#[cfg(test)]
use sim_core::native_world::resolve_level1_presentation_cache_root;
use sim_core::{
    FlightContext, FlightEvent, FlightState, FlightWorld, POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING,
    PlayerCommand, Position, PositionalSoundCommand, StepEvidence,
    archer_tmap::{ArcherTmapBank, ArcherTmapClocks},
    combat_tmap::{
        COMBAT_STATIC_EXTENTS, COMBAT_STATIC_SELECTORS, COMBAT_STATIC_SHA256, CombatTmapBank,
        FIREBALL_PAYLOAD_SHA256, FireballTmapClock, POSSESSION_PAYLOAD_SHA256, PossessionTmapClock,
    },
    conformance_hash, conformance_hash_v2,
    dynamic_lighting::DynamicLightingExecutionReceipt,
    economy_tmap::{
        CASTLE_ROOT_PAYLOAD_BYTES, CASTLE_ROOT_PAYLOAD_SHA256, CastleTmapClock, EconomyTmapBank,
        MANA_CARRIER_PAYLOAD_BYTES, MANA_CARRIER_PAYLOAD_SHA256, MANA_SPHERE_NEUTRAL_PAYLOAD_BYTES,
        MANA_SPHERE_NEUTRAL_PAYLOAD_SHA256, MANA_SPHERE_PLAYER_ONE_PAYLOAD_BYTES,
        MANA_SPHERE_PLAYER_ONE_PAYLOAD_SHA256,
    },
    firefly_tmap::{FireflyTmapBank, FireflyTmapClocks},
    frozen_actor_tmap::{
        DOLMEN_LOGICAL_HEIGHT, DOLMEN_PAYLOAD_BYTES, DOLMEN_PAYLOAD_SHA256, DOLMEN_RENDER_MODE,
        DOLMEN_RENDER_PARAMETER_INDEX, DOLMEN_SELECTOR, FrozenActorTmapBank,
        LEVEL1_FROZEN_ACTOR_ENTITY_INDICES, VILLAGER_ANIMATED_SELECTORS, VILLAGER_PAYLOAD_BYTES,
        VILLAGER_PAYLOAD_SHA256, VillagerTmapClocks,
    },
    goat::{
        GOAT_BASE_TMAP, GOAT_CLASS, GOAT_MODEL, GOAT_PATROL_ACTION, GoatPlayerTarget, GoatState,
        GoatStepEvent, LEVEL_ONE_GOAT_INDICES, LEVEL_TWO_GOAT_INDICES, LiveWorldState,
    },
    goat_draw_admission::{
        GoatDrawAdmission, GoatDrawCamera, evaluate_goat_draw_admission,
        outdoor_billboard_visit_map,
    },
    goat_tmap::{
        GOAT_TMAP_DRAW_MASK, GOAT_TMAP_SELECTORS, GoatTmapBank, GoatTmapClock, GoatTmapClocks,
    },
    house::{HouseState, HouseStepEvent},
    indexed_overlay::IndexedOverlayAssets,
    indexed_postprocess::{IndexedPostprocessBranch, IndexedPostprocessInput},
    level1_mission::{
        ARCHER_ARROW_BASE_TMAP, ARCHER_ARROW_LOGICAL_HEIGHT, ARCHER_ARROW_PROJECTILE_CLASS,
        ARCHER_ARROW_PROJECTILE_MODEL, ARCHER_ARROW_RENDER_INDEX, ARCHER_ARROW_RENDER_MODE,
        CASTLE_PROJECTILE_BASE_TMAP, CASTLE_PROJECTILE_CLASS, CASTLE_PROJECTILE_LOGICAL_HEIGHT,
        CASTLE_PROJECTILE_MODEL, CASTLE_PROJECTILE_RENDER_INDEX, CASTLE_PROJECTILE_RENDER_MODE,
        CASTLE_ROOT_ACTION, CASTLE_ROOT_BASE_TMAP, CASTLE_ROOT_CLASS, CASTLE_ROOT_LOGICAL_HEIGHT,
        CASTLE_ROOT_MODEL, CASTLE_ROOT_RENDER_INDEX, CASTLE_SPELL_FAMILY,
        CHECKPOINT_SMOKE_BASE_TMAP, CHECKPOINT_SMOKE_CLASS, CHECKPOINT_SMOKE_FIRST_RENDER_INDEX,
        CHECKPOINT_SMOKE_MODEL, CHECKPOINT_SMOKE_RENDER_MODE, DynamicResidentState,
        FIREBALL_BASE_TMAP, FIREBALL_IMPACT_BASE_TMAP, FIREBALL_IMPACT_CLASS,
        FIREBALL_IMPACT_LOGICAL_HEIGHT, FIREBALL_IMPACT_RENDER_INDEX, FIREBALL_IMPACT_RENDER_MODE,
        FIREBALL_LOGICAL_HEIGHT, FIREBALL_PROJECTILE_CLASS, FIREBALL_PROJECTILE_MODEL,
        FIREBALL_RENDER_INDEX, FIREBALL_RENDER_MODE, FireballImpact, GROUND_FIRE_BASE_TMAP,
        GROUND_FIRE_MODEL, GROUND_FIRE_RENDER_MODE, HouseConstructionSourceEffect,
        LEVEL1_MINIMAP_COMMAND_BLEND_PIXEL, LEVEL1_MINIMAP_MAX_ENTITY_COMMANDS,
        LEVEL1_MINIMAP_MAX_MARKS, Level1Actor, Level1ActorKind, Level1AmbientEntity,
        Level1CollisionTarget, Level1MinimapEntityCommand, Level1MinimapEntityInput,
        Level1MinimapMarks, Level1MissionEvent, Level1MissionState, Level1MultipartSegment,
        Level1PossessedBuilding, Level1Projectile, Level1ResetSourceSlot, Level1SourceEntityView,
        MANA_CARRIER_ACTION, MANA_CARRIER_BASE_TMAP, MANA_CARRIER_CLASS,
        MANA_CARRIER_LOGICAL_HEIGHT, MANA_CARRIER_MAX_LIFE, MANA_CARRIER_MAX_MANA,
        MANA_CARRIER_MODEL, MANA_CARRIER_RENDER_INDEX, MANA_CARRIER_SPEED, MANA_SPHERE_ACTION,
        MANA_SPHERE_CLASS, MANA_SPHERE_MODEL, MANA_SPHERE_NEUTRAL_BASE_TMAP,
        MANA_SPHERE_NEUTRAL_RENDER_BASE, MANA_SPHERE_PLAYER_ONE_BASE_TMAP,
        MANA_SPHERE_PLAYER_ONE_RENDER_BASE, ManaCarrier, ManaSphere,
        POSSESSION_ATTRACTION_BASE_TMAP, POSSESSION_ATTRACTION_LOGICAL_HEIGHT,
        POSSESSION_ATTRACTION_MODEL, POSSESSION_ATTRACTION_RENDER_INDEX,
        POSSESSION_ATTRACTION_RENDER_MODE, POSSESSION_IMPACT_BASE_TMAP, POSSESSION_IMPACT_CLASS,
        POSSESSION_IMPACT_LOGICAL_HEIGHT, POSSESSION_IMPACT_MODEL, POSSESSION_IMPACT_RENDER_INDEX,
        POSSESSION_IMPACT_RENDER_MODE, POSSESSION_PROJECTILE_BASE_TMAP,
        POSSESSION_PROJECTILE_CLASS, POSSESSION_PROJECTILE_LOGICAL_HEIGHT,
        POSSESSION_PROJECTILE_MODEL, POSSESSION_PROJECTILE_RENDER_INDEX,
        POSSESSION_PROJECTILE_RENDER_MODE, POSSESSION_STRONG_IMPACT_MODEL, WATER_SPLASH_BASE_TMAP,
        WATER_SPLASH_LOGICAL_HEIGHT, WATER_SPLASH_MODEL, WATER_SPLASH_RENDER_INDEX,
        WATER_SPLASH_RENDER_MODE, ground_fire_presentation, level1_map_castle_rope_pixels,
        level1_map_entity_command, level1_minimap_castle_rope_pixels,
        level1_minimap_entity_command, mana_sphere_size_ordinal,
    },
    native_world::{
        AssetRef, EntityAuthority, EntitySnapshot, Level2DispatchEvent, Level2EconomyEvent,
        Level2ResetSession, NativeWorld, NativeWorldInspection, ParticleParameter, SessionTerrain,
        TerrainPlanes, apply_level1_source_effects, level1_turn_start_source_views,
        refresh_level1_current_source_view,
    },
    positional_sound_should_replace, prepare_event_sound_command,
    presentation_frame::{
        MainHistory, PresentationFrameError, PresentationFrameInput, PresentationFrameState,
        SourceRenderedFrame,
    },
    render_camera::{TerrainSlopeCamera, TerrainSlopeCameraSample},
    renderable_map_chain::{RenderableMapChains, RenderableMapSeed},
    source_projection_hash,
    source_sprite_tmap::{
        ImportedResetSpriteTuple, SourceSpriteContractRoute, SourceSpriteContractRow,
        SourceSpriteDisposition, SourceSpriteLegacyFamily, SourceSpriteTemplateDisposition,
        SourceSpriteWorldResources, imported_reset_sprite_disposition, source_sprite_dispositions,
        unknown_source_sprite_tuple,
    },
    state_hash,
    static_world_tmap::{
        EXIT_TARGET_PAYLOAD_BYTES, EXIT_TARGET_PAYLOAD_SHA256, LEVEL1_AMBIENT_SELECTORS,
        LEVEL1_SCENERY_SELECTORS, SPELL_JAR_PAYLOAD_BYTES, SPELL_JAR_PAYLOAD_SHA256,
        StaticWorldTmapBank,
    },
    step_entity_body_with_context, step_outer_turn_with_context,
    terrain_frame::{
        LEVEL1_SKY_BYTES, LEVEL1_TERRAIN_ATLAS_BYTES, LEVEL1_VIEW_HEIGHT, LEVEL1_VIEW_PIXELS,
        LEVEL1_VIEW_WIDTH, Level1CastleRootRender, Level1FrozenActorRender, Level1TerrainFrame,
        Level1TerrainFrameCamera, Level1TerrainFrameResources, Level1WorldFrameResources,
        render_level1_source_world_frame, render_level1_source_world_map_frame_composed,
        render_level1_terrain_frame, render_level1_world_frame,
    },
    terrain_grid::{OUTDOOR_GRID_VERTICES, OutdoorGridInput, outdoor_grid},
    terrain_presentation::outdoor_terrain_pnt5,
    villager::{VILLAGER_CLASS, VILLAGER_MODEL, VillagerState},
};

use crate::presentation_bridge::{
    LIVE_MENU_AUTH_ALL, LIVE_MENU_AUTH_DEBUG_METRICS, Level1PresentationLive, LiveDebugEntity,
    LiveHud, LiveMenuState, LiveSpellMenu, LiveSpellSlot, PresentationBridgeQuery,
    build_level1_final_main,
};

#[cfg(test)]
use sim_core::level1_mission::FIREBALL_IMPACT_MODEL;

#[cfg(test)]
use sim_core::level1_mission::CASTLE_PROJECTILE_SOUND;

pub const MC_NATIVE_ABI_VERSION: u32 = 2;

// Pinned remc2 sub_83B50 fixes the owned game heap at 0x400000 plus
// 0x1000000. Its retained Level-1 source has no writer for the two allocator
// diagnostic globals, so their exact simulation-domain values remain zero.
const LEVEL1_SOURCE_HEAP_SIZE: u32 = 0x0140_0000;
const LEVEL1_SOURCE_MEMORY_USED: u32 = 0;
const LEVEL1_SOURCE_MEMORY_FREE: u32 = 0;

pub const MC_STATUS_OK: i32 = 0;
pub const MC_STATUS_NULL_POINTER: i32 = 1;
pub const MC_STATUS_INVALID_UTF8: i32 = 2;
pub const MC_STATUS_INVALID_ARGUMENT: i32 = 3;
pub const MC_STATUS_UNSUPPORTED_VERSION: i32 = 4;
pub const MC_STATUS_STRUCT_TOO_SMALL: i32 = 5;
pub const MC_STATUS_RESERVED_NONZERO: i32 = 6;
pub const MC_STATUS_IO_ERROR: i32 = 7;
pub const MC_STATUS_VALIDATION_FAILED: i32 = 8;
pub const MC_STATUS_UNKNOWN_HANDLE: i32 = 9;
pub const MC_STATUS_BOUNDS: i32 = 10;
pub const MC_STATUS_BUFFER_TOO_SMALL: i32 = 11;
pub const MC_STATUS_GENERATION_MISMATCH: i32 = 12;
pub const MC_STATUS_CURSOR_EXPIRED: i32 = 13;
pub const MC_STATUS_INTERNAL_PANIC: i32 = 14;
pub const MC_STATUS_LOCK_FAILED: i32 = 15;

pub const MC_PROFILE_OUTDOOR: u8 = 0;
pub const MC_PROFILE_CAVE: u8 = 1;

pub const MC_AUTHORITY_INACTIVE: u8 = 0;
pub const MC_AUTHORITY_FROZEN_OBSERVED: u8 = 1;
pub const MC_AUTHORITY_PLAYER_FLIGHT_LIVE: u8 = 2;
pub const MC_AUTHORITY_SPELL_ACTIVATION_LIVE: u8 = 3;
pub const MC_AUTHORITY_GOAT_LIVE: u8 = 4;
pub const MC_AUTHORITY_SELECTOR_FRAME_LIVE: u8 = 5;
pub const MC_AUTHORITY_LEVEL1_MISSION_LIVE: u8 = 6;
pub const MC_AUTHORITY_SESSION_TERRAIN_LIVE: u8 = 7;
pub const MC_AUTHORITY_VILLAGER_LIVE: u8 = 8;
pub const MC_LEVEL1_HOUSE_COMPLETION_ALLOCATED: u32 = 1;
pub const MC_LEVEL1_HOUSE_COMPLETION_ALLOCATION_FAILED: u32 = 2;
pub const MC_LEVEL1_EXIT_FLAG_PALETTE_VISUAL_CURRENT: u8 = 1 << 0;
pub const MC_LEVEL1_EXIT_FLAG_FOV_POST_ACTION_NEXT_HISTORY: u8 = 1 << 1;
pub const MC_LEVEL1_EXIT_FLAG_END_SEQUENCE: u8 = 1 << 2;
#[allow(dead_code)] // Public ABI bit retained after its implementation cleared.
pub const MC_LEVEL1_GAP_ALLOCATION_ONLY_ACTORS: u8 = 1 << 0;
#[allow(dead_code)] // Public ABI bit retained after its implementation cleared.
pub const MC_LEVEL1_GAP_CASTLE_BUILD_FOOTPRINT: u8 = 1 << 1;
#[allow(dead_code)] // Public ABI bit retained after its implementation cleared.
pub const MC_LEVEL1_GAP_CASTLE_CONSTRUCTION_TIMING: u8 = 1 << 2;
#[allow(dead_code)] // Public ABI bit retained after its implementation cleared.
pub const MC_LEVEL1_GAP_MANA_SPHERE_ALLOCATION_RNG: u8 = 1 << 3;
#[allow(dead_code)] // Public ABI bit retained after its implementation cleared.
pub const MC_LEVEL1_GAP_MANA_CARRIER_FLIGHT: u8 = 1 << 4;
#[allow(dead_code)] // Public ABI bit retained after its implementation cleared.
pub const MC_LEVEL1_GAP_MISSION_SOUND_POSITIONING: u8 = 1 << 5;
#[allow(dead_code)] // Public ABI bit retained after its implementation cleared.
pub const MC_LEVEL1_GAP_EXIT_COLLISION_AND_FADE: u8 = 1 << 6;
#[allow(dead_code)] // Public ABI bit retained after its implementation cleared.
pub const MC_LEVEL1_GAP_SECONDARY_GROUP_33_34_CREATURES: u8 = 1 << 7;
pub const MC_LEVEL1_CURRENT_FIDELITY_GAPS: u8 = 0;

pub const MC_SELECTOR_FRAME_ANIMATED: u32 = 0x0001;
pub const MC_SELECTOR_FRAME_GROUPED_UPDATE: u32 = 0x0002;
pub const MC_SELECTOR_FRAME_USED_BY_PRECEDING_DRAW: u32 = 0x0004;
pub const MC_SELECTOR_FRAME_PIXELS_MUTABLE: u32 = 0x0008;

pub const MC_TERRAIN_TYPES_U8: u32 = 0;
pub const MC_TERRAIN_HEIGHT_U8: u32 = 1;
pub const MC_TERRAIN_SHADE_U8: u32 = 2;
pub const MC_TERRAIN_ANGLE_U8: u32 = 3;
pub const MC_TERRAIN_SECONDARY_U8: u32 = 4;

pub const MC_TERRAIN_ENTITY_INDEX_I16: u32 = 5;

pub const MC_WORLD_STRING_SCHEMA: u32 = 0;
pub const MC_WORLD_STRING_PACKAGE_SHA256: u32 = 1;
pub const MC_WORLD_STRING_CLOSURE_SHA256: u32 = 2;
pub const MC_WORLD_STRING_STATE_HASH_SCHEMA: u32 = 3;
pub const MC_WORLD_STRING_CONFORMANCE_HASH_SCHEMA: u32 = 4;
pub const MC_WORLD_STRING_CONTEXT_SEAM: u32 = 5;

pub const MC_ASSET_STRING_ID: u32 = 0;
pub const MC_ASSET_STRING_CONTENT_ADDRESS: u32 = 1;
pub const MC_ASSET_STRING_CATEGORY: u32 = 2;
pub const MC_ASSET_STRING_HOOK: u32 = 3;
pub const MC_ASSET_STRING_STAGE: u32 = 4;
pub const MC_ASSET_STRING_OWNER_ID: u32 = 5;

pub const MC_EVENT_MOVEMENT_RESOLUTION: u32 = 0;
pub const MC_EVENT_PREPARE_SOUND: u32 = 1;
pub const MC_EVENT_END_LOOP: u32 = 2;
pub const MC_EVENT_UPDATE_MUSIC: u32 = 3;
pub const MC_EVENT_REQUEST_SPELL_ACTIVATION: u32 = 4;
pub const MC_EVENT_SHOW_HELP_HINT: u32 = 5;
pub const MC_EVENT_START_MUSIC: u32 = 6;
pub const MC_EVENT_GOAT_SOUND_REQUEST: u32 = 7;
pub const MC_EVENT_GOAT_MANA_CONVERTED: u32 = 8;
pub const MC_EVENT_GOAT_EXTRA_MANA_SPHERE_REQUEST: u32 = 9;
pub const MC_EVENT_GOAT_REMOVAL_QUEUED: u32 = 10;
pub const MC_EVENT_GOAT_RECLAIMED: u32 = 11;
pub const MC_EVENT_GOAT_ENGAGEMENT_BOUNDARY: u32 = 12;
pub const MC_EVENT_LEVEL1_OBJECTIVE_ADVANCED: u32 = 13;
pub const MC_EVENT_LEVEL1_SPELL_COLLECTED: u32 = 14;
pub const MC_EVENT_LEVEL1_FIREBALL_SPAWNED: u32 = 15;
pub const MC_EVENT_LEVEL1_FIREBALL_IMPACTED: u32 = 16;
pub const MC_EVENT_LEVEL1_ACTOR_DAMAGED: u32 = 17;
pub const MC_EVENT_LEVEL1_ACTOR_KILLED: u32 = 18;
pub const MC_EVENT_LEVEL1_CASTLE_BUILT: u32 = 19;
pub const MC_EVENT_LEVEL1_CASTLE_MANA_CHANGED: u32 = 20;
pub const MC_EVENT_LEVEL1_PLAYER_DAMAGED: u32 = 21;
pub const MC_EVENT_LEVEL1_PLAYER_DIED: u32 = 22;
pub const MC_EVENT_LEVEL1_COMPLETED: u32 = 23;
pub const MC_EVENT_LEVEL1_ARCHER_ARROW_SPAWNED: u32 = 24;
pub const MC_EVENT_LEVEL1_ARCHER_ARROW_SOUND: u32 = 25;
pub const MC_EVENT_LEVEL1_FIREFLY_FIREBALL_SPAWNED: u32 = 26;
pub const MC_EVENT_LEVEL1_FIREFLY_CHARGE_SOUND: u32 = 27;
pub const MC_EVENT_LEVEL1_CASTLE_PROJECTILE_SPAWNED: u32 = 28;
pub const MC_EVENT_LEVEL1_CASTLE_PLACEMENT_REJECTED: u32 = 29;
pub const MC_EVENT_LEVEL1_CASTLE_CONSTRUCTION_SOUND: u32 = 30;
pub const MC_EVENT_LEVEL1_POSSESSION_SPAWNED: u32 = 31;
pub const MC_EVENT_LEVEL1_MANA_SPHERE_SPAWNED: u32 = 32;
pub const MC_EVENT_LEVEL1_MANA_SPHERE_CLAIMED: u32 = 33;
pub const MC_EVENT_LEVEL1_MANA_SPHERE_COLLECTED: u32 = 34;
pub const MC_EVENT_LEVEL1_EXIT_SEQUENCE_STARTED: u32 = 35;
pub const MC_EVENT_LEVEL1_EXIT_SEQUENCE_COMPLETED: u32 = 36;
pub const MC_EVENT_LEVEL1_WATER_SPLASHED: u32 = 37;
pub const MC_EVENT_LEVEL1_ABILITY_IMPROVED: u32 = 38;

/// Additive Level 2 campaign event kinds (`mc_sim_level2.h`).
pub const MC_CAMPAIGN_EVENT_SPELL_ACQUISITION: u8 = 2;
pub const MC_CAMPAIGN_EVENT_GOAT: u8 = 9;
/// Additive Level 2 campaign economy events (`mc_sim_level2.h`, kind 10).
pub const MC_CAMPAIGN_EVENT_ECONOMY: u8 = 10;

pub const MC_CAMPAIGN_ECONOMY_MANA_SPHERE_SPAWNED: u8 = 1;
pub const MC_CAMPAIGN_ECONOMY_MANA_SPHERE_CLAIMED: u8 = 2;
pub const MC_CAMPAIGN_ECONOMY_MANA_SPHERE_COLLECTED: u8 = 3;
pub const MC_CAMPAIGN_ECONOMY_MANA_SPHERES_MERGED: u8 = 4;
pub const MC_CAMPAIGN_ECONOMY_MANA_CARRIER_SPAWNED: u8 = 5;
pub const MC_CAMPAIGN_ECONOMY_MANA_CARRIER_ARRIVED: u8 = 6;
pub const MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_ARRIVED: u8 = 7;
pub const MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_COLLECTED_SPHERE: u8 = 16;
pub const MC_CAMPAIGN_ECONOMY_DEATH_BURST_SPAWNED: u8 = 8;
pub const MC_CAMPAIGN_ECONOMY_DEATH_BURST_EXPIRED: u8 = 9;
pub const MC_CAMPAIGN_ECONOMY_CASTLE_BUILT: u8 = 10;
pub const MC_CAMPAIGN_ECONOMY_CASTLE_DESTROYED: u8 = 11;
pub const MC_CAMPAIGN_ECONOMY_CASTLE_CONSTRUCTION_SOUND: u8 = 12;
pub const MC_CAMPAIGN_ECONOMY_CASTLE_DESTRUCTION_SOUND: u8 = 13;
pub const MC_CAMPAIGN_ECONOMY_CASTLE_STAGE_DESTROYED: u8 = 14;
pub const MC_CAMPAIGN_ECONOMY_DAMAGE_APPLIED: u8 = 15;
pub const MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_CAVE_SOUND: u8 = 17;

pub const MC_AUDIO_ACTION_PREPARED_PENDING: u8 = 0;
pub const MC_AUDIO_ACTION_START: u8 = 1;
pub const MC_AUDIO_ACTION_RESTART: u8 = 2;
pub const MC_AUDIO_ACTION_SUPPRESS_EXISTING: u8 = 3;
pub const MC_AUDIO_ACTION_SUPPRESS_NO_VOICE: u8 = 4;
pub const MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME: u8 = 5;
pub const MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED: u8 = 6;
pub const MC_AUDIO_ACTION_UPDATE_EXISTING: u8 = 7;
pub const MC_AUDIO_ACTION_END_LOOP: u8 = 8;
pub const MC_AUDIO_ACTION_SUPPRESS_NO_EXISTING: u8 = 9;

pub const MC_ASSET_REF_NONE: u32 = u32::MAX;
pub const MC_PARAMETER_RECORD_NONE: u16 = u16::MAX;
pub const MC_REQUESTED_SPELL_NONE: i16 = -1;
pub const MC_ENTITY_INDEX_NONE: u16 = u16::MAX;
pub const MC_AUDIO_VOICE_NONE: u16 = u16::MAX;
pub const MC_PRESENTATION_CAMERA_EXACT_V2_INPUTS: u8 = 0x01;
pub const MC_PRESENTATION_CAMERA_EYE_Z_PLUS_128: u8 = 0x02;
pub const MC_RENDER_CAMERA_TERRAIN_SLOPE_XY: u8 = 0x01;
pub const MC_TERRAIN_FRAME_ISOLATED_SKY_TERRAIN: u32 = 0x0001;
pub const MC_TERRAIN_FRAME_SKY_SEEDED: u32 = 0x0002;
pub const MC_TERRAIN_FRAME_HAS_COVERAGE: u32 = 0x0004;
pub const MC_TERRAIN_FRAME_NON_TERRAIN_OMITTED: u32 = 0x0008;
pub const MC_TERRAIN_FRAME_GOATS_COMPOSITED: u32 = 0x0010;
pub const MC_TERRAIN_FRAME_ARCHERS_COMPOSITED: u32 = 0x0020;
pub const MC_TERRAIN_FRAME_FIREFLIES_COMPOSITED: u32 = 0x0040;
pub const MC_TERRAIN_FRAME_COMBAT_COMPOSITED: u32 = 0x0080;
pub const MC_TERRAIN_FRAME_OBJECTIVE_PROPS_COMPOSITED: u32 = 0x0100;
pub const MC_TERRAIN_FRAME_ECONOMY_COMPOSITED: u32 = 0x0200;
pub const MC_TERRAIN_FRAME_REFLECTED_SPRITES_COMPOSITED: u32 = 0x0400;
pub const MC_TERRAIN_FRAME_SPRITE_SHADOWS_COMPOSITED: u32 = 0x0800;
pub const MC_TERRAIN_FRAME_FROZEN_ACTORS_COMPOSITED: u32 = 0x1000;
pub const MC_PRESENTATION_FRAME_COMPOSITOR_COMPLETE: u32 = 0x2000;
pub const MC_PRESENTATION_FRAME_HAS_PROVENANCE: u32 = 0x4000;
pub const MC_PRESENTATION_FRAME_OVERLAYS_COMPOSITED: u32 = 0x8000;
pub const MC_PRESENTATION_FRAME_POSTPROCESS_COMPLETE: u32 = 0x1_0000;
pub const MC_PRESENTATION_FRAME_LIGHTING_COMPLETE: u32 = 0x2_0000;

fn world_frame_flags(reflected_sprites_composited: bool, sprite_shadows_composited: bool) -> u32 {
    let mut flags = MC_TERRAIN_FRAME_ISOLATED_SKY_TERRAIN
        | MC_TERRAIN_FRAME_SKY_SEEDED
        | MC_TERRAIN_FRAME_HAS_COVERAGE
        | MC_TERRAIN_FRAME_NON_TERRAIN_OMITTED
        | MC_TERRAIN_FRAME_GOATS_COMPOSITED
        | MC_TERRAIN_FRAME_ARCHERS_COMPOSITED
        | MC_TERRAIN_FRAME_FIREFLIES_COMPOSITED
        | MC_TERRAIN_FRAME_COMBAT_COMPOSITED
        | MC_TERRAIN_FRAME_OBJECTIVE_PROPS_COMPOSITED
        | MC_TERRAIN_FRAME_ECONOMY_COMPOSITED
        | MC_TERRAIN_FRAME_FROZEN_ACTORS_COMPOSITED;
    if reflected_sprites_composited {
        flags |= MC_TERRAIN_FRAME_REFLECTED_SPRITES_COMPOSITED;
    }
    if sprite_shadows_composited {
        flags |= MC_TERRAIN_FRAME_SPRITE_SHADOWS_COMPOSITED;
    }
    flags
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

const EVENT_CAPACITY: usize = 4_096;
const AUDIO_VOICE_COUNT: usize = 10;
const TERRAIN_CELLS: usize = 256 * 256;
const POSITIONAL_QUALIFICATION_VECTOR_COUNT: u32 = 23;
const TERRAIN_PNT5_QUALIFICATION_VECTOR_COUNT: u32 = 13;
// Pinned remc2 6806c720 `Events.cpp:561-605` assigns the NewEvent entity
// parameter row at lines 573 and 599, independently of its particle selector.
const LEVEL1_NEW_EVENT_ENTITY_PARAMETER_RECORD_INDEX: u16 = 59;
const TERRAIN_ATLAS_SHA256: &str =
    "d4d69babd648008aac40413e6089e27bbe32e6b2a5be4311eb10b1aaa33fc5de";
const TABLESN_SHA256: &str = "f88974baf68f728fb534a4dab35397b9cb3ca2f424019bbde36858eebbf97495";
const SKYN_SHA256: &str = "944112ef6eb54e042889c9cc841ed1d7bdb129c38c1c61ce4c8b8e81589f466b";
const PALN_RGB6_SHA256: &str = "6b55d0714b939e8bd25cef89466806299dbae933f2f2f96e0cf8402fc607d16f";

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McWorldInfoV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub external_level: u16,
    pub internal_level: u16,
    pub map_type: u8,
    pub profile: u8,
    pub reserved_0: u16,
    pub entity_count: u32,
    pub active_entity_count: u32,
    pub player_count: u32,
    pub asset_ref_count: u32,
    pub terrain_width: u16,
    pub terrain_height: u16,
    pub reset_state_hash: u64,
    pub reset_conformance_hash: u64,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCommandV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub opcode: i8,
    pub argument_1: i8,
    pub argument_2: i8,
    pub roll: i8,
    pub pitch: i8,
    pub flags: i8,
    pub requested_spell: i16,
    pub reserved: [u32; 4],
}

/// Additive tail for the two 16-bit view fields in the original ten-byte
/// player input record. `McCommandV2` remains byte-for-byte stable.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCameraInputV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub look_yaw: u16,
    pub look_pitch: u16,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McSessionSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub event_head: u64,
    pub state_hash: u64,
    pub conformance_hash: u64,
    pub entity_index: u16,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub yaw: i16,
    pub display_pitch: i16,
    pub effective_pitch: i16,
    pub roll_accumulator: i16,
    pub target_speed: i16,
    pub actual_speed: i16,
    pub strafe_speed: i16,
    pub phase: u8,
    pub last_resolution: u8,
    pub profile: u8,
    pub local_player_slot: u8,
    pub left_spell_slot: u8,
    pub right_spell_slot: u8,
    pub selected_spell_ui_index: u8,
    /// Validated reset observation. ABI v2 does not advance this presentation
    /// countdown; `objectives_authority` is therefore frozen-observed.
    pub objective_textbox_ticks: u8,
    pub objective_count: u8,
    pub objectives_authority: u8,
    pub reserved_1: [u8; 2],
    pub left_spell_holder: u16,
    pub right_spell_holder: u16,
    pub selected_spell_holder: u16,
    pub reserved_0: u16,
    pub caster_mana: i32,
    pub maptype_music: i32,
    pub fire_sequence: u32,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McObjectiveSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub slot: u8,
    pub is_level_end: u8,
    pub text_selector: u8,
    pub done: u8,
    pub authority: u8,
    pub stage_selectors: [u8; 8],
    pub reserved_0: [u8; 3],
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McEntitySnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub entity_index: u16,
    pub parameter_record_index: u16,
    /// The player-flight row merges current session pose/flight fields into
    /// the validated reset observation. Spell holder timer/action-marker
    /// mutations are authoritative in the core but are not fields in v2.
    pub authority: u8,
    pub active: u8,
    pub class_id: u8,
    pub model: u8,
    pub action: u8,
    pub phase: u8,
    pub has_parameter_record: u8,
    pub reserved_0: u8,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub yaw: i16,
    pub pitch: i16,
    pub fov: i16,
    pub actual_speed: i16,
    pub reserved: [u32; 4],
}

/// Generic campaign selection input. This is additive: the existing native
/// v2 open call and every Level 1 wrapper retain their original signatures.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignSelectionV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub external_level: u16,
    pub internal_level: u16,
    pub map_type: u8,
    pub reserved_0: u8,
    pub reserved: [u32; 4],
}

/// Additive Level 2 outer-turn input.  The embedded `McCommandV2` keeps the
/// source ten-byte command projection stable; the explicit player slot and
/// position let a Unity caller identify the active flight row without
/// mutating the legacy Level 1 handle ABI.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignStepInputV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub command: McCommandV2,
    pub player_slot: u8,
    pub speed_index: u8,
    pub reserved_0: u16,
    pub player_x: u16,
    pub player_y: u16,
    pub player_z: i16,
    pub reserved: [u32; 4],
}

/// One bounded typed event from a Level 2 source pass.  `kind` is one of the
/// `MC_CAMPAIGN_EVENT_*` constants in the public C header; `flags` is a
/// source-family-specific nonzero/result marker.  Detailed family state stays
/// on the typed campaign snapshot surfaces rather than leaking Rust enums.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignEventV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub ordinal: u32,
    pub pass: u8,
    pub kind: u8,
    pub family: u8,
    pub flags: u8,
    pub source_slot: u16,
    pub reserved_0: u16,
    pub reserved: [u32; 4],
}

/// Additive detailed payload for a kind-10 `McCampaignEventV2` row.  The
/// `ordinal` joins this sidecar to the generic event emitted by the same
/// campaign step.  `primary_id`, `secondary_id`, `entity_0`, `entity_1`, and
/// `value_0`/`value_1` are explicitly code-specific; every reserved field is
/// kept zero for forward-compatible extension.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignEconomyEventV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub ordinal: u32,
    pub pass: u8,
    pub family: u8,
    pub code: u8,
    pub flags: u8,
    pub source_slot: u16,
    pub reserved_0: u16,
    pub primary_id: u32,
    pub secondary_id: u32,
    pub entity_0: u16,
    pub entity_1: u16,
    pub value_0: i32,
    pub value_1: i32,
    pub reserved: [u32; 4],
}

/// Revisioned result of one Level 2 outer turn.  The digest is the exact
/// typed-dispatch observability digest for the returned event sequence.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignStepSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub revision: u64,
    pub state_hash: u64,
    pub terrain_hash: u64,
    pub dispatch_hash: [u8; 32],
    pub event_count: u32,
    pub event_kind_mask: u32,
    pub player_slot: u8,
    pub reserved_0: [u8; 3],
    pub player_entity_index: u16,
    pub reserved_1: u16,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub reserved: [u32; 4],
}

/// Common campaign metadata returned by the additive campaign session path.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignWorldInfoV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub external_level: u16,
    pub internal_level: u16,
    pub map_type: u8,
    pub profile: u8,
    pub reserved_0: u16,
    pub entity_count: u32,
    pub active_entity_count: u32,
    pub player_count: u32,
    pub objective_count: u32,
    pub terrain_width: u16,
    pub terrain_height: u16,
    pub reset_state_hash: u64,
    pub reserved: [u32; 4],
}

/// Bounded common player participation and player/entity relation row.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignPlayerSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub slot: u8,
    pub is_ai: u8,
    pub active: u8,
    pub reserved_0: u8,
    pub entity_index: u16,
    pub active_player_index: i16,
    pub turn: i32,
    pub left_spell_slot_raw: i16,
    pub right_spell_slot_raw: i16,
    pub left_subspell_index: i8,
    pub right_subspell_index: i8,
    pub selected_spell_ui_index: u8,
    pub selected_subspell_index: i8,
    pub reserved_1: [u8; 2],
    pub reserved: [u32; 4],
}

/// Bounded common entity relation row. Sentinel `u16::MAX` means no relation.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignEntitySnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub entity_index: u16,
    pub source_id: u16,
    pub next_relation: u16,
    pub parameter_relation: u16,
    pub authority: u8,
    pub active: u8,
    pub class_id: u8,
    pub model: u8,
    pub action: u8,
    pub phase: u8,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub yaw: i16,
    pub pitch: i16,
    pub fov: i16,
    pub actual_speed: i16,
    pub reserved: [u32; 4],
}

/// Additive Rust-owned Level 1 Goat row. The existing 52-byte entity snapshot
/// remains stable; presentation clients should fetch this family in one bulk
/// call when they need gameplay and render fields beyond pose.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McGoatSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub entity_index: u16,
    pub target_entity_index: u16,
    pub render_parameter_index: u16,
    pub base_tmap: u16,
    pub authority: u8,
    pub active: u8,
    pub class_id: u8,
    pub model: u8,
    pub action: u8,
    pub phase: u8,
    pub stage_var_1: i8,
    pub stage_var_2: i8,
    pub proximity_countdown: i8,
    pub proximity_subcounter: i8,
    /// Qualified per-entity reset observation only. The live animation state
    /// is selector-global and is exposed separately by
    /// `McSelectorFrameStateV2`; it must never be copied into this source byte.
    pub animation_frame: i8,
    pub reserved_0: u8,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub actual_speed: i16,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub rng: u16,
    pub life: i32,
    pub max_life: i32,
    pub mana: i32,
    pub pending_damage: i32,
    pub control_flags: u32,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McGoatBatchV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub live_world_hash: u64,
    pub start_ordinal: u32,
    pub written: u32,
    pub total_count: u32,
    pub has_more: u8,
    pub reserved_0: [u8; 3],
    pub reserved: [u32; 4],
}

/// Additive Level 2 campaign batch metadata for paged mana-sphere and carrier
/// copies.  The layout intentionally matches `McGoatBatchV2` so clients can
/// reuse the same generation/tick/hash coherence checks without changing the
/// sealed Level 1 snapshot records.
pub type McCampaignManaBatchV2 = McGoatBatchV2;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McLevel1MissionSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub authority: u8,
    pub current_stage: u8,
    pub text_selector: u8,
    pub objective_textbox_ticks: u8,
    pub tasks_completed: u8,
    pub player_dead: u8,
    pub has_fireball: u8,
    pub has_possession: u8,
    pub has_castle_spell: u8,
    pub has_speed_spell: u8,
    pub castle_exists: u8,
    pub castle_level: u8,
    /// Named source-mapped behavior that remains intentionally absent.
    pub fidelity_gap_flags: u8,
    pub exit_sequence_active: u8,
    pub exit_sequence_complete: u8,
    pub exit_result_code: u8,
    pub player_life: i32,
    pub player_max_life: i32,
    pub player_mana: i32,
    pub player_max_mana: i32,
    pub castle_mana: i32,
    pub secured_mana_percent: i32,
    pub actor_count: u32,
    pub projectile_count: u32,
    pub impact_count: u32,
    pub exit_sequence_phase: u32,
    pub exit_sequence_timer: i32,
    pub possession_experience: i32,
    pub last_possessed_building_entity: u16,
    pub last_possessed_building_owner: u16,
    pub possessed_building_count: u16,
    pub possession_tier: u8,
    pub speed_trail_count: u8,
}

/// ABI-additive source projection for the reachable Level 1 player actions
/// 0, 2, and human-only 3. The established 96-byte mission snapshot retains
/// its action!=0 `player_dead` compatibility byte.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McLevel1PlayerLifecycleSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub lifecycle_hash: u64,
    pub source_arena_hash: u64,
    pub authority: u8,
    pub player_action: u8,
    pub player_dead: u8,
    pub input_suppressed: u8,
    pub player_active: u8,
    pub player_visible: u8,
    pub player_targetable: u8,
    pub menu_close_requested: u8,
    pub notification_active: u8,
    pub notification_state: u8,
    pub banished_camera_active: u8,
    pub marker_present: u8,
    pub fall_velocity: i16,
    pub notification_timer: i16,
    pub action_timer: i32,
    pub replaceable_cursor: i32,
    pub killer_entity_index: u16,
    pub killer_score_awards: u16,
    pub notification_id: u16,
    pub player_entity_rng: u16,
    pub player_control_flags: u32,
    pub player_entity_roll: i16,
    pub player_yaw: i16,
    pub controller_pitch: i16,
    pub controller_roll: i16,
    pub recoil_speed: i16,
    pub marker_source_entity_index: u16,
    pub marker_x: u16,
    pub marker_y: u16,
    pub marker_z: i16,
    pub marker_trigger_entity_index: u16,
    pub marker_particle_parameter_index: u16,
    pub marker_life: i32,
    pub marker_max_life: i32,
    pub source_pool_rebuild_count: u32,
    pub lifecycle_trace_hash: u64,
    pub lifecycle_trace_count: u16,
    pub marker_dispatch_phase: u8,
    pub marker_class: u8,
    pub marker_model: u8,
    pub marker_action: u8,
    pub marker_byte_38: u8,
    pub source_color_phase_6: u8,
    pub source_turn_counter: u32,
    pub menu_state: u8,
    pub killer_score_victim_color: u8,
    pub effective_pitch: i16,
    pub marker_constructor_phase: u32,
    pub reserved: [u8; 4],
}

/// One selected spell-holder row consumed by `DrawSpellIcon_2E260`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McLevel1HudSpellSlotV2 {
    pub holder_entity_index: u16,
    pub parent_entity_index: u16,
    pub model: u8,
    pub tier: i8,
    pub enabled_flags: u8,
    pub action_marker: i8,
    pub active_value: i16,
    pub activation_value: u16,
    pub max_mana: i32,
    pub mana_regen: i32,
    pub caster_mana: i32,
    pub frame_selector: u16,
    pub icon_selector: u16,
    pub reserved: [u32; 2],
}

/// Additive source-mapped state for the ordinary Level 1 top bar and hands.
///
/// Rendering stays presentation-only, but every width, blink, selector and
/// color decision is driven by the same integer state used by remc2.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McLevel1HudSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub authority: u8,
    pub mana_color_index: u8,
    pub mana_max_color_index: u8,
    pub life_color_index: u8,
    pub color_phase_1: u8,
    pub color_phase_2: u8,
    pub player_hit_frame_ticks: u8,
    pub castle_hit_frame_ticks: u8,
    pub global_mana_denominator: i32,
    pub secured_mana_component: i32,
    pub goal_percent: i32,
    pub player_status_flags: u32,
    pub left: McLevel1HudSpellSlotV2,
    pub right: McLevel1HudSpellSlotV2,
    pub selected_holder_entity_index: u16,
    pub selected_icon_selector: u16,
    pub selected_model: u8,
    pub selected_tier: i8,
    pub selected_enabled_flags: u8,
    pub selected_frame_selector: u8,
}

/// Source spell-menu state for the four families reachable in Level 1.
///
/// This remains separate from the draw-oriented HUD row because
/// `SpellLevels` (unlocked ceiling), `array_0x437` (current family tier), and
/// the two hand subspell fields are distinct source state.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McLevel1SpellMenuSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub authority: u8,
    pub selected_ui_index: u8,
    pub selected_tier: i8,
    pub left_family: u8,
    pub left_tier: i8,
    pub right_family: u8,
    pub right_tier: i8,
    pub reserved_0: u8,
    pub family_enabled: [u8; 4],
    pub family_unlocked_tier: [i8; 4],
    pub family_selected_tier: [i8; 4],
    pub family_hand_eligibility: [u8; 4],
    pub holder_entity_index: [u16; 4],
    pub reserved: [u32; 2],
}

/// Atomic source-projected objective-mark and directional-arrow state for the
/// 128x128 Level 1 minimap.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McLevel1MinimapMarksV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub mark_count: u8,
    pub mark_pulse: u8,
    pub arrow_valid: u8,
    pub arrow_visible: u8,
    pub mark_x: [i16; LEVEL1_MINIMAP_MAX_MARKS],
    pub mark_y: [i16; LEVEL1_MINIMAP_MAX_MARKS],
    pub mark_colour: [u8; LEVEL1_MINIMAP_MAX_MARKS],
    pub mark_help_selector: [u8; LEVEL1_MINIMAP_MAX_MARKS],
    pub arrow_xy: [i16; 14],
    pub reserved: [u32; 4],
}

/// One source-ordered raster operation from `DrawMinimapEntities_B_61A00`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McLevel1MinimapEntityCommandV2 {
    pub source_slot: u16,
    pub x: i16,
    pub y: i16,
    pub selector: u16,
    pub kind: u8,
    pub palette_index: u8,
    pub size: u8,
    pub anchor: u8,
    pub help_selector: u8,
    pub flags: u8,
    pub order: u16,
    pub reserved: [u32; 2],
}

/// Atomic header for the Level 1 minimap entity-command stream.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McLevel1MinimapEntitiesInfoV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub total_count: u32,
    pub cross_x: u16,
    pub cross_y: u16,
    pub cross_colour: u8,
    pub cross_arm_length: u8,
    pub reveal_active: u8,
    pub authority: u8,
    pub reserved_0: [u8; 4],
    pub command_hash: u64,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McLevel1PaletteInfoV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub submode: u8,
    pub effect: u8,
    pub weight: u8,
    pub denominator: u8,
    pub palette_count: i16,
    pub flags: u16,
    pub rgb8_sha256: [u8; 32],
    pub reserved: [u32; 4],
}

/// Additive source-mapped presentation state for the Level 1 action-12 exit
/// sequence. The established 96-byte mission snapshot remains unchanged.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McLevel1ExitPresentationV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub authority: u8,
    pub player_action: u8,
    pub action_phase: u8,
    pub palette_submode: u8,
    pub palette_effect: u8,
    pub palette_weight: u8,
    pub palette_denominator: u8,
    pub movement_resolution: u8,
    pub target_source_entity_index: u16,
    pub target_valid: u8,
    pub input_suppressed: u8,
    pub camera_fov: i16,
    pub action_timer: i16,
    pub player_status_flags: u8,
    pub blur_mode: u8,
    pub display_blank: u8,
    pub result_code: u8,
    pub active: u8,
    pub complete: u8,
    pub flags: u8,
    pub reserved_0: u8,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McLevel1CastleSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub authority: u8,
    pub active: u8,
    pub level: u8,
    pub class_id: u8,
    pub model: u8,
    pub action: u8,
    pub render_mode: u8,
    pub reserved_0: u8,
    pub owner_entity_index: u16,
    pub render_parameter_index: u16,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub base_tmap: u16,
    pub logical_height: u16,
    pub reserved_1: u16,
    pub life: i32,
    pub max_life: i32,
    pub mana: i32,
    pub mana_capacity: i32,
    pub carrier_id: u32,
    pub reserved: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McTerrainStateV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub mutation_sequence: u64,
    pub terrain_hash: u64,
    pub width: u16,
    pub height: u16,
    pub plane_count: u8,
    pub hash_schema: u8,
    pub authority: u8,
    pub reserved_0: u8,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McLevel1CastleLifecycleSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub root_presentation_id: u32,
    pub root_source_entity_index: u16,
    pub root_owner_entity_index: u16,
    pub root_x: u16,
    pub root_y: u16,
    pub root_z: i16,
    pub root_build_state: i16,
    pub root_action: u8,
    pub root_level: u8,
    pub root_active: u8,
    pub authority: u8,
    pub root_life: i32,
    pub construction_id: u32,
    pub construction_source_entity_index: u16,
    pub construction_parent_entity_index: u16,
    pub construction_owner_entity_index: u16,
    pub construction_x: u16,
    pub construction_y: u16,
    pub construction_z: i16,
    pub construction_timer: i16,
    pub construction_stage: u8,
    pub construction_present: u8,
    pub construction_active: u8,
    pub construction_initialized: u8,
    pub reserved_0: [u8; 2],
    pub terrain_mutation_sequence: u64,
}

/// One source-ordered `sub_377A0` completion allocation attempt.
///
/// Allocated rows expose the full class-10/model-42/action-44 constructor
/// state. Failed rows retain the parent selector/state and leave child fields
/// zero, making the no-write branch observable without inspecting Rust state.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McLevel1HouseCompletionEffectV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub kind: u32,
    pub house_entity_index: u16,
    pub parent_entity_index: u16,
    pub source_entity_index: u16,
    pub source_id: u16,
    pub class_id: u8,
    pub model: u8,
    pub action: u8,
    pub construction_selector: u8,
    pub house_building_index: u8,
    pub constructor_latch: u8,
    pub active: u8,
    pub phase: u8,
    pub parent_word_2e: i16,
    pub parent_construction_selector: i16,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub collision_half_width: i16,
    pub collision_half_depth: i16,
    pub yaw: i16,
    pub fov: i16,
    pub effect_ordinal: u16,
    pub max_life: i32,
    pub life: i32,
    pub control_flags: u32,
    pub effect_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McManaSphereSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub sphere_id: u32,
    pub source_entity_index: u16,
    pub owner_entity_index: u16,
    pub authority: u8,
    pub active: u8,
    pub class_id: u8,
    pub model: u8,
    pub action: u8,
    pub phase: u8,
    pub render_mode: u8,
    pub reserved_0: u8,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub yaw: i16,
    pub velocity_x: i16,
    pub velocity_y: i16,
    pub vertical_velocity: i16,
    pub render_parameter_index: u16,
    pub base_tmap: u16,
    pub logical_height: u16,
    pub mana: i32,
    pub assigned_carrier_id: u32,
    pub reserved: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McManaCarrierSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub carrier_id: u32,
    pub target_sphere_id: u32,
    pub owner_entity_index: u16,
    pub render_parameter_index: u16,
    pub authority: u8,
    pub active: u8,
    pub class_id: u8,
    pub model: u8,
    pub action: u8,
    pub returning_to_castle: u8,
    pub render_mode: u8,
    pub reserved_0: u8,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub yaw: i16,
    pub speed: i16,
    pub base_tmap: u16,
    pub logical_height: u16,
    pub reserved_1: u16,
    pub carried_mana: i32,
    pub reserved: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McLevel1ActorSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub authored_index: u16,
    pub activation_group: i16,
    pub kind: u8,
    pub authority: u8,
    pub active: u8,
    pub collected: u8,
    pub class_id: u8,
    pub model: u8,
    pub action: u8,
    pub render_mode: u8,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub render_parameter_index: u16,
    pub max_life: i32,
    pub life: i32,
    pub pending_damage: i32,
    pub base_tmap: u16,
    pub logical_height: u16,
    pub yaw: i16,
    pub animation_frame: i8,
    pub visible: u8,
    pub state_value: i32,
    pub reserved: u32,
}

/// Source-slot keyed body node emitted by the model-3 multipart constructor.
/// These nodes have no authored level row, so they deliberately remain a
/// separate additive stream instead of pretending to be mission actors.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McLevel1MultipartSegmentSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub source_entity_index: u16,
    pub head_authored_index: u16,
    pub head_source_entity_index: u16,
    pub predecessor_source_entity_index: u16,
    pub successor_source_entity_index: u16,
    pub ordinal: u8,
    pub authority: u8,
    pub active: u8,
    pub visible: u8,
    pub class_id: u8,
    pub model: u8,
    pub action: u8,
    pub render_mode: u8,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub yaw: i16,
    pub pitch: i16,
    pub phase: u8,
    pub reserved_0: u8,
    pub owner_id: i16,
    pub life: i32,
    pub pending_damage: i32,
    pub render_parameter_index: u16,
    pub base_tmap: u16,
    pub logical_height: u16,
    pub collision_half_width: i16,
    pub collision_half_height: i16,
    pub trailing_distance: i16,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McFireballProjectileSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub projectile_id: u32,
    pub owner_entity_index: u16,
    pub render_index: u16,
    pub authority: u8,
    pub active: u8,
    pub class_id: u8,
    pub model: u8,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub yaw: i16,
    pub pitch: i16,
    pub speed: i16,
    pub life_ticks: i16,
    pub base_tmap: u16,
    pub payload: i32,
    pub logical_height: u16,
    pub render_mode: u8,
    pub animation_frame: i8,
    /// 1 = player Fireball; 2 = Archer arrow; 3 = Firefly fireball;
    /// 4 = Castle build projectile; 5 = Possession projectile.
    pub projectile_kind: u32,
    pub reserved: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McFireballImpactSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub impact_id: u32,
    pub source_projectile_id: u32,
    pub authority: u8,
    pub active: u8,
    pub damage_submitted: u8,
    pub class_id: u8,
    pub model: u8,
    pub render_mode: u8,
    pub animation_frame: i8,
    pub reserved_0: u8,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub life_ticks: i16,
    pub payload: i32,
    pub base_tmap: u16,
    pub render_parameter_index: u16,
    pub logical_height: u16,
    pub reserved_1: u16,
    pub reserved: [u32; 2],
}

/// Additive selector-global mutable TMAP descriptor state.
///
/// Stream offsets are relative to the validated decoded TMAP payload and are
/// never process pointers.  Pixel bytes remain Rust-owned; this row exposes a
/// stable hash and exact source descriptor metadata.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McSelectorFrameStateV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub selector: u16,
    pub mask_bit: u8,
    pub authority: u8,
    pub width: u16,
    pub height: u16,
    pub source_frame_index: u16,
    pub frame_count: u16,
    pub stream_start: u32,
    pub stream_cursor: u32,
    pub payload_bytes: u32,
    pub pixel_bytes: u32,
    pub flags: u32,
    pub pixel_hash: u64,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McSelectorFrameBatchV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub selector_state_hash: u64,
    pub last_draw_tick: u64,
    pub last_draw_mask: u64,
    pub valid_draw_mask: u64,
    pub start_ordinal: u32,
    pub written: u32,
    pub total_count: u32,
    pub has_more: u8,
    pub has_draw_feedback: u8,
    pub reserved_0: u16,
    pub reserved: [u32; 4],
}

/// Authoritative post-draw feedback for one already-presented simulation tick.
///
/// Bits 0, 1, and 2 map to selectors 152, 153, and 154 respectively.  The
/// `draw_tick` is the completed outer-turn/session snapshot tick for the
/// rendered Goat rows, not the intentionally pre-update camera-history tick.
/// Same-tick submissions are OR-accumulated before the next outer turn.
///
/// The actual CPU admission predicate is recovered in the following
/// milestone; current Unity presentation submits nothing and therefore stays
/// frozen.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McSelectorDrawFeedbackV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub draw_tick: u64,
    pub selector_mask: u64,
    pub reserved: [u32; 4],
}

/// Private qualification-only pending-damage injection. This writes the
/// source boundary consumed on the following native step; Unity never owns or
/// directly mutates Goat state.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McGoatDamageCommandV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub entity_index: u16,
    pub reserved_0: u16,
    pub damage: i32,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McAssetRefInfoV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub asset_index: u32,
    pub trace_ordinal: u16,
    pub owner_asset_count: u16,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McEventQueryV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub cursor: u64,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McLevel1ResidentObservationV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub required_mask: u8,
    pub observed_mask: u8,
    pub remaining_mask: u8,
    pub reserved_0: u8,
    pub digest: u64,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McEventBatchV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub next_cursor: u64,
    pub event_head: u64,
    pub written: u32,
    pub has_more: u8,
    pub reserved_0: [u8; 3],
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McEventV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub sequence: u64,
    pub state_hash: u64,
    pub kind: u32,
    pub code: u32,
    pub entity_index: u16,
    pub player_index: i16,
    pub value_0: i32,
    pub value_1: i32,
    pub asset_ref_index: u32,
    pub reserved: [u32; 4],
}

/// Additive original-stereo sidecar. `McEventV2` remains byte-for-byte stable;
/// clients request this record only for a positional `PrepareSound` sequence.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McAudioCommandV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub event_sequence: u64,
    pub state_hash: u64,
    pub asset_ref_index: u32,
    pub relative_x: i32,
    pub relative_y: i32,
    pub relative_z: i32,
    pub playback_rate_hz: u32,
    pub sound_id: u16,
    pub entity_index: u16,
    pub player_index: i16,
    pub sound_flags: i16,
    pub volume_q15: u16,
    pub pan_q16: u16,
    pub rate_delta_percent: i16,
    pub voice_index: u16,
    pub play_type: u8,
    pub loop_count: i8,
    pub action: u8,
    /// One-based order among audible ordinary pooled winners for this drain.
    /// Direct/immediate and suppressed commands keep zero.
    pub pooled_flush_rank: u8,
    pub reserved: [u32; 4],
}

/// Additive source-presentation state. Existing snapshots and hashes remain
/// byte-for-byte unchanged.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McPresentationStateV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub source_turn: i32,
    pub camera_x: u16,
    pub camera_y: u16,
    pub camera_z: i16,
    pub camera_yaw: i16,
    pub camera_pitch: i16,
    pub camera_roll: i16,
    pub camera_fov: i16,
    pub camera_authority: u8,
    pub camera_flags: u8,
    pub reserved_0: u32,
    pub reserved: [u32; 4],
}

/// Additive final outdoor-render camera transform. The raw stored history
/// remains available through `McPresentationStateV2`; this sidecar adds the
/// stateful X/Y slope target and easing used by `DrawWorld_411A0`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McRenderCameraStateV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub source_turn: i32,
    pub history_x: u16,
    pub history_y: u16,
    pub eye_z: i16,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub fov: i16,
    pub slope_target_x: i16,
    pub slope_target_y: i16,
    pub render_offset_x: i16,
    pub render_offset_y: i16,
    pub authority: u8,
    pub flags: u8,
    pub reserved_0: u16,
    pub reserved: [u32; 4],
}

/// One hard pnt5 qualification vector produced by the Rust source model.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McTerrainPnt5VectorV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub turn: i32,
    pub tile_x: i32,
    pub tile_y: i32,
    pub distance_squared: u32,
    pub shade: u8,
    pub angle: u8,
    pub visible: u8,
    pub tablesn_row: u8,
    pub wave: i32,
    pub land_displacement: i32,
    pub pnt5: i32,
    pub view_y: i32,
    pub lighting_wave: i32,
    pub reserved: [u32; 2],
}

/// One row-major vertex of the source 40 by 21 outdoor terrain lattice.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McTerrainGridVertexV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub index: u16,
    pub row: u8,
    pub column: u8,
    pub tile_x: u8,
    pub tile_y: u8,
    pub shade: u8,
    pub angle: u8,
    pub view_x: i32,
    pub view_depth: i32,
    pub distance_squared: u32,
    pub pnt5: i32,
    pub raw_wave: i32,
    pub surface_displacement: i32,
    pub lighting_wave: i32,
    pub feature_flags: u16,
    pub tablesn_row: u8,
    pub pnt5_written: u8,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McTerrainFrameQueryV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    /// Presentation/render-camera tick, not the post-simulation snapshot tick.
    pub tick: u64,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McTerrainFrameInfoV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub width: u16,
    pub height: u16,
    pub stride: u16,
    pub viewport_x: u16,
    pub viewport_y: u16,
    pub viewport_width: u16,
    pub viewport_height: u16,
    pub reserved_0: u16,
    pub index_bytes: u32,
    pub coverage_bytes: u32,
    pub flags: u32,
    pub reflected_triangles: u32,
    pub primary_mode5_triangles: u32,
    pub primary_mode26_triangles: u32,
    pub index_hash: u64,
    pub coverage_hash: u64,
    pub terrain_pixel_writes: u64,
    pub reserved: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McPresentationFrameQueryV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub presentation_revision: u64,
    pub viewport_width: u16,
    pub viewport_height: u16,
    pub screen_stride: u16,
    pub reserved_0: u16,
    pub configured_blur_mode: u8,
    pub blur_capability: u8,
    pub high_speed_system: u8,
    pub scratch_present: u8,
    pub vga_resolution: u8,
    pub spatial_smoothing: u8,
    pub minimap_enabled: u8,
    pub top_bar_enabled: u8,
    pub menu_overlay: u8,
    pub game_paused: u8,
    pub level_end: u8,
    pub objective_selector: u8,
    pub objective_textbox: u8,
    pub tutorial_or_briefing: u8,
    pub help_popup: u8,
    pub debug_text: u8,
    pub reserved: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct McLevel1PresentationInputV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub authority_mask: u32,
    pub selected_menu_item: u8,
    pub pause_menu_suppressed: u8,
    pub pause_text_suppressed: u8,
    pub level_end_text_suppressed: u8,
    pub recursive_pause_writer: u8,
    pub load_available: u8,
    pub sound_available: u8,
    pub music_available: u8,
    pub options_enabled_mask: u32,
    pub options_selected_mask: u32,
    pub sound_volume: u8,
    pub music_volume: u8,
    pub volume_target: u8,
    pub chat_recipient: u8,
    pub chat_mode: u8,
    pub chat_player_mask: u8,
    pub local_player_index: u8,
    pub frame_timing_index: u8,
    pub cursor_visible: u8,
    pub reserved_0: u8,
    pub mouse_x: i16,
    pub mouse_y: i16,
    pub joystick: i16,
    pub text_colour_phase: u8,
    pub reserved_1: u8,
    pub chat_lines: [u8; 8 * 64],
    pub chat_names: [u8; 8 * 16],
    pub notification_player_name: [u8; 16],
    pub debug_clock_bucket: u32,
    pub sound_number: i16,
    pub reserved_2: u16,
    pub debug_heap_size: u32,
    pub debug_memory_used: u32,
    pub debug_memory_free: u32,
    pub reserved: [u32; 4],
}

impl Default for McLevel1PresentationInputV2 {
    fn default() -> Self {
        Self {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<Self>(),
            authority_mask: 0,
            selected_menu_item: 0,
            pause_menu_suppressed: 0,
            pause_text_suppressed: 0,
            level_end_text_suppressed: 0,
            recursive_pause_writer: 0,
            load_available: 0,
            sound_available: 0,
            music_available: 0,
            options_enabled_mask: 0,
            options_selected_mask: 0,
            sound_volume: 0,
            music_volume: 0,
            volume_target: 0,
            chat_recipient: 0,
            chat_mode: 0,
            chat_player_mask: 0,
            local_player_index: 0,
            frame_timing_index: 0,
            cursor_visible: 0,
            reserved_0: 0,
            mouse_x: 0,
            mouse_y: 0,
            joystick: 0,
            text_colour_phase: 0,
            reserved_1: 0,
            chat_lines: [0; 8 * 64],
            chat_names: [0; 8 * 16],
            notification_player_name: [0; 16],
            debug_clock_bucket: 0,
            sound_number: 0,
            reserved_2: 0,
            debug_heap_size: 0,
            debug_memory_used: 0,
            debug_memory_free: 0,
            reserved: [0; 4],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McPresentationFrameInfoV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub presentation_revision: u64,
    pub immutable_input_hash: u64,
    pub width: u16,
    pub height: u16,
    pub stride: u16,
    pub viewport_width: u16,
    pub index_bytes: u32,
    pub provenance_bytes: u32,
    pub flags: u32,
    pub effective_postprocess_mode: u8,
    pub history_state: u8,
    pub newly_committed: u8,
    pub reserved_0: u8,
    pub index_hash: u64,
    pub provenance_hash: u64,
    pub lighting_hash: u64,
    pub source_arena_hash: u64,
    pub overlay_pixels_written: u32,
    pub overlay_stage_count: u16,
    pub reserved_1: u16,
    pub reserved: [u32; 6],
}

#[derive(Debug)]
struct AbiError {
    status: i32,
    message: String,
}

impl AbiError {
    fn new(status: i32, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

type AbiResult<T> = Result<T, AbiError>;

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CameraHistory {
    tick: u64,
    source_turn: i32,
    x: u16,
    y: u16,
    z: i16,
    yaw: i16,
    pitch: i16,
    roll: i16,
    fov: i16,
}

#[derive(Debug)]
struct TerrainFrameAssets {
    terrain_atlas: Box<[u8]>,
    tablesn: Box<[u8]>,
    sky: Box<[u8]>,
    palette_rgb6: Box<[u8]>,
    archer_tmap_bank: ArcherTmapBank,
    firefly_tmap_bank: FireflyTmapBank,
    frozen_actor_tmap_bank: FrozenActorTmapBank,
    combat_tmap_bank: CombatTmapBank,
    static_world_tmap_bank: StaticWorldTmapBank,
    economy_tmap_bank: EconomyTmapBank,
    indexed_overlays: IndexedOverlayAssets,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceRenderInputs {
    map_right_viewport: bool,
    camera: Level1TerrainFrameCamera,
    generation_asset_digest: u64,
    local_player_entity_index: u16,
    goats: Vec<GoatState>,
    frozen_actors: Vec<Level1FrozenActorRender>,
    ambient_entities: Vec<Level1AmbientEntity>,
    mission_actors: Vec<Level1Actor>,
    dynamic_residents: Vec<DynamicResidentState>,
    mission_segments: Vec<Level1MultipartSegment>,
    castle_root: Option<Level1CastleRootRender>,
    mana_spheres: Vec<ManaSphere>,
    mana_carrier: Option<ManaCarrier>,
    map_chains: RenderableMapChains,
    projectiles: Vec<Level1Projectile>,
    impacts: Vec<FireballImpact>,
    possessed_buildings: Vec<Level1PossessedBuilding>,
    source_slots: Vec<Level1ResetSourceSlot>,
    dispositions: Vec<SourceSpriteDisposition>,
    night: bool,
}

impl CameraHistory {
    fn capture(
        state: &FlightState,
        source_turn: i32,
        fov: i16,
        look_yaw: i16,
        look_pitch: i16,
    ) -> Self {
        let boost = i32::from(state.recoil_speed);
        let turn_phase = source_turn & 1;
        let pitch = i32::from(state.pitch_accumulator) / 2 + turn_phase * (boost / 16) - boost / 8
            + i32::from(look_pitch);
        Self {
            tick: state.tick,
            source_turn,
            x: state.position.x,
            y: state.position.y,
            z: state.position.z.wrapping_add(128),
            // PlayerEvents stores the raw wrapping i16 sum. DrawWorld masks
            // this value to 11 bits only when building the render transform.
            yaw: state.yaw.wrapping_add(look_yaw),
            pitch: pitch as i16,
            roll: state.roll_accumulator,
            fov,
        }
    }
}

#[derive(Debug)]
struct SessionRecord {
    world: Arc<NativeWorld>,
    terrain: SessionTerrain,
    state: FlightState,
    context: FlightContext,
    live_world: LiveWorldState,
    mission: Level1MissionState,
    frozen_actors: Box<[Level1FrozenActorRender]>,
    selector_frames: Option<GoatTmapClocks>,
    archer_selector_frames: Option<ArcherTmapClocks>,
    firefly_selector_frames: Option<FireflyTmapClocks>,
    villager_selector_frames: Option<VillagerTmapClocks>,
    fireball_selector_frame: Option<FireballTmapClock>,
    possession_selector_frame: Option<PossessionTmapClock>,
    castle_selector_frame: Option<CastleTmapClock>,
    renderable_map_chains: RenderableMapChains,
    selector_draw_feedback: Option<(u64, u8)>,
    notification_draw_feedback: Option<(u64, bool, u8, i16)>,
    goat_draw_admission: Option<GoatDrawAdmission>,
    generation: u64,
    event_head: u64,
    events: VecDeque<McEventV2>,
    audio_commands: BTreeMap<u64, McAudioCommandV2>,
    audio_voices: [Option<AudioVoice>; AUDIO_VOICE_COUNT],
    music_start_armed: bool,
    fire_sequence: u32,
    last_resolution: u8,
    source_turn: i32,
    camera_fov: i16,
    camera_look_yaw: i16,
    camera_look_pitch: i16,
    camera_history: CameraHistory,
    camera_slope: TerrainSlopeCamera,
    camera_slope_sample: TerrainSlopeCameraSample,
    terrain_frame_assets: Option<Arc<TerrainFrameAssets>>,
    source_render_asset_digest: Option<u64>,
    presentation_frame: PresentationFrameState,
    lighting_receipt: DynamicLightingExecutionReceipt,
    presentation_ui: LiveMenuState,
    debug_clock_bucket: Option<u32>,
    debug_previous_turn: i32,
    debug_fps: i32,
    sound_number: i16,
    game_timer_turn: i32,
    debug_heap_size: u32,
    debug_memory_used: u32,
    debug_memory_free: u32,
    game_paused: bool,
    pending_house_effects: Vec<HouseStepEvent>,
    #[cfg(test)]
    immediate_released_holder_handoff: Vec<(u8, u16, u8, u8)>,
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

fn level1_frozen_actor_renders(world: &NativeWorld) -> AbiResult<Box<[Level1FrozenActorRender]>> {
    let mut renders = Vec::with_capacity(LEVEL1_FROZEN_ACTOR_ENTITY_INDICES.len());
    for entity_index in LEVEL1_FROZEN_ACTOR_ENTITY_INDICES {
        let entity = world
            .entities()
            .get(usize::from(entity_index))
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    format!("frozen Level 1 actor row {entity_index} is absent"),
                )
            })?;
        let (expected_class, expected_model, expected_action, expected_relation, render_index) =
            match entity_index {
                51 | 55 | 58 | 59 | 63 | 66 | 70 | 71 | 74 | 78 => (5, 13, 111, 100, 271),
                52 | 56 | 60 | 67 | 68 | 75 => (5, 13, 111, 100, 241),
                53 | 64 | 72 => (5, 13, 111, 100, 239),
                54 | 57 | 61 | 62 | 65 | 69 | 73 | 76 | 77 => (5, 13, 111, 100, 242),
                130..=132 => (2, 2, 6, 59, DOLMEN_RENDER_PARAMETER_INDEX),
                _ => unreachable!("qualified frozen actor index set is exhaustive"),
            };
        let expected_flags = if entity_index >= 130 { 4 } else { 12 };
        if !entity.active
            || (entity_index < 130 && entity.authority != EntityAuthority::VillagerLive)
            || (entity_index >= 130 && entity.authority != EntityAuthority::FrozenObserved)
            || entity.class != expected_class
            || entity.model != expected_model
            || entity.action != expected_action
            || entity.parameter_relation != Some(expected_relation)
            || entity.particle_parameter_index != Some(render_index)
            || entity.animation_frame() != 0
            || entity.control_flags != expected_flags
        {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!(
                    "frozen Level 1 actor row {entity_index} changed: authority={:?} active={} class/model/action={}/{}/{} relation={:?} particle={:?} animation={} flags={:#010x}",
                    entity.authority,
                    entity.active,
                    entity.class,
                    entity.model,
                    entity.action,
                    entity.parameter_relation,
                    entity.particle_parameter_index,
                    entity.animation_frame(),
                    entity.control_flags
                ),
            ));
        }
        let parameter = world
            .presentation()
            .particles
            .get(usize::from(render_index))
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    format!("frozen Level 1 render parameter {render_index} is absent"),
                )
            })?;
        let expected_parameter = match render_index {
            239 => ParticleParameter {
                base_tmap: 155,
                word_2: 8_492,
                word_4: 42,
                speed: 90,
                rotation_speed: 200,
                byte_10: 0,
                byte_11: -1,
                render_mode: 20,
                byte_13: 0,
            },
            241 => ParticleParameter {
                base_tmap: 180,
                word_2: 8_492,
                word_4: 42,
                speed: 116,
                rotation_speed: 250,
                byte_10: 0,
                byte_11: -1,
                render_mode: 20,
                byte_13: 0,
            },
            242 => ParticleParameter {
                base_tmap: 183,
                word_2: 8_492,
                word_4: 42,
                speed: 116,
                rotation_speed: 250,
                byte_10: 0,
                byte_11: -1,
                render_mode: 20,
                byte_13: 0,
            },
            271 => ParticleParameter {
                base_tmap: 199,
                word_2: 8_492,
                word_4: 42,
                speed: 96,
                rotation_speed: 300,
                byte_10: 0,
                byte_11: -1,
                render_mode: 20,
                byte_13: 0,
            },
            DOLMEN_RENDER_PARAMETER_INDEX => ParticleParameter {
                base_tmap: DOLMEN_SELECTOR,
                word_2: 8_492,
                word_4: 42,
                speed: 666,
                rotation_speed: DOLMEN_LOGICAL_HEIGHT,
                byte_10: 0,
                byte_11: 1,
                render_mode: DOLMEN_RENDER_MODE,
                byte_13: 0,
            },
            _ => unreachable!("qualified frozen actor render parameters are exhaustive"),
        };
        if *parameter != expected_parameter {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!("frozen Level 1 render parameter {render_index} changed: {parameter:?}"),
            ));
        }
        renders.push(Level1FrozenActorRender {
            source_entity_index: entity_index,
            position: entity.position,
            yaw: entity.yaw,
            render_parameter_index: render_index,
        });
    }
    Ok(renders.into_boxed_slice())
}

fn reset_renderable_map_chains(
    world: &NativeWorld,
    frozen_actors: &[Level1FrozenActorRender],
) -> AbiResult<RenderableMapChains> {
    // The same source map is used for painter ordering and gameplay
    // collision. Preserve every reset row whose collision bit is set; the
    // compositor can still skip unsupported draw families without removing
    // the intervening source-order links.
    let mut seeds = world
        .entities()
        .iter()
        .filter(|entity| entity.active && entity.control_flags & 8 != 0)
        .map(|entity| RenderableMapSeed {
            entity_index: entity.index,
            // Some live reset rows are deliberately absent from map chains
            // behind source no-map/no-draw gates. Track them for later
            // insertion without requiring reset membership.
            active: false,
            x: entity.position.x,
            y: entity.position.y,
        })
        .collect::<Vec<_>>();
    seeds.extend(
        world
            .reset_live_world()
            .goats()
            .iter()
            .map(|goat| RenderableMapSeed {
                entity_index: goat.entity_index,
                active: false,
                x: goat.position.x,
                y: goat.position.y,
            }),
    );
    seeds.extend(frozen_actors.iter().map(|actor| RenderableMapSeed {
        entity_index: actor.source_entity_index,
        active: false,
        x: actor.position.x,
        y: actor.position.y,
    }));
    seeds.extend(
        world
            .level1_mission()
            .ambient_entities()
            .filter(|ambient| ambient.map_linked)
            .map(|ambient| RenderableMapSeed {
                entity_index: ambient.source_entity_index,
                active: false,
                x: ambient.position.x,
                y: ambient.position.y,
            }),
    );
    seeds.sort_unstable_by_key(|seed| seed.entity_index);
    seeds.dedup_by_key(|seed| seed.entity_index);
    RenderableMapChains::from_reset(
        &seeds,
        &world.terrain_planes().entity_indices,
        |entity_index| {
            world
                .entities()
                .get(usize::from(entity_index))
                .map(EntitySnapshot::old_map_entity)
        },
    )
    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))
}

fn native_composited_actor_kind(kind: Level1ActorKind) -> bool {
    matches!(
        kind,
        Level1ActorKind::Archer
            | Level1ActorKind::Firefly
            | Level1ActorKind::CastleSpellJar
            | Level1ActorKind::SpeedSpellJar
            | Level1ActorKind::ExitTarget
            | Level1ActorKind::MultipartHead
            | Level1ActorKind::ExperienceScroll
            | Level1ActorKind::Tree
            | Level1ActorKind::Stone
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AudioVoice {
    sound_id: u16,
    sound_flags: i16,
    event_sequence: u64,
}

fn append_source_render_records<T: std::fmt::Debug>(
    bytes: &mut Vec<u8>,
    family: &[u8],
    records: &[T],
) {
    bytes.extend_from_slice(
        &u16::try_from(family.len())
            .expect("source-render family tag fits u16")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(family);
    bytes.extend_from_slice(
        &u32::try_from(records.len())
            .expect("source-render record extent fits u32")
            .to_le_bytes(),
    );
    for record in records {
        // These source projections derive Debug with declaration-order field
        // names. Length-prefixing that lossless representation avoids struct
        // padding/host-endian dependence while covering every mutable field.
        let encoded = format!("{record:?}");
        bytes.extend_from_slice(
            &u32::try_from(encoded.len())
                .expect("source-render record fits u32")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(encoded.as_bytes());
    }
}

fn source_render_asset_digest(world: &NativeWorld, assets: &TerrainFrameAssets) -> AbiResult<u64> {
    let source_bank = world.source_sprite_tmap_bank().ok_or_else(|| {
        AbiError::new(
            MC_STATUS_GENERATION_MISMATCH,
            "source-render generation has no CP1 source TMAP bank",
        )
    })?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"mc2-source-render-assets-v1\0");
    for payload in [&assets.terrain_atlas, &assets.tablesn, &assets.sky] {
        bytes.extend_from_slice(
            &u32::try_from(payload.len())
                .expect("renderer asset payload fits u32")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(payload);
    }
    append_source_render_records(
        &mut bytes,
        b"archer-bank",
        std::slice::from_ref(&assets.archer_tmap_bank),
    );
    append_source_render_records(
        &mut bytes,
        b"firefly-bank",
        std::slice::from_ref(&assets.firefly_tmap_bank),
    );
    append_source_render_records(
        &mut bytes,
        b"frozen-bank",
        std::slice::from_ref(&assets.frozen_actor_tmap_bank),
    );
    append_source_render_records(
        &mut bytes,
        b"combat-bank",
        std::slice::from_ref(&assets.combat_tmap_bank),
    );
    append_source_render_records(
        &mut bytes,
        b"static-bank",
        std::slice::from_ref(&assets.static_world_tmap_bank),
    );
    append_source_render_records(
        &mut bytes,
        b"economy-bank",
        std::slice::from_ref(&assets.economy_tmap_bank),
    );
    append_source_render_records(
        &mut bytes,
        b"source-bank",
        std::slice::from_ref(source_bank),
    );
    append_source_render_records(&mut bytes, b"particles", &world.presentation().particles);
    Ok(sim_core::presentation_frame::byte_hash(&bytes))
}

impl SessionRecord {
    fn set_game_paused(&mut self, paused: bool) {
        if paused && !self.game_paused {
            // Capture the current live player state once at the pause
            // boundary so presentation may continue while entity action
            // dispatch remains held.
            self.camera_history = CameraHistory::capture(
                &self.state,
                self.source_turn,
                self.camera_fov,
                self.camera_look_yaw,
                self.camera_look_pitch,
            );
            self.camera_slope_sample = self.camera_slope.update(
                self.terrain.ground_heights(),
                self.camera_history.x,
                self.camera_history.y,
            );
        }
        self.game_paused = paused;
    }

    fn apply_presentation_input(&mut self, input: McLevel1PresentationInputV2) -> AbiResult<()> {
        validate_reserved(&input.reserved, "Level 1 presentation input")?;
        if input.reserved_0 != 0 || input.reserved_1 != 0 || input.reserved_2 != 0 {
            return Err(AbiError::new(
                MC_STATUS_RESERVED_NONZERO,
                "Level 1 presentation input reserved fields must be zero",
            ));
        }
        if input.authority_mask & !LIVE_MENU_AUTH_ALL != 0 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                format!(
                    "Level 1 presentation authority mask {:#x} has unknown bits",
                    input.authority_mask
                ),
            ));
        }
        if input.debug_heap_size != 0
            || input.debug_memory_used != 0
            || input.debug_memory_free != 0
        {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "Level 1 source allocator metrics are simulation-owned and input fields must be zero",
            ));
        }
        for (name, value) in [
            ("pause_menu_suppressed", input.pause_menu_suppressed),
            ("pause_text_suppressed", input.pause_text_suppressed),
            ("level_end_text_suppressed", input.level_end_text_suppressed),
            ("recursive_pause_writer", input.recursive_pause_writer),
            ("load_available", input.load_available),
            ("sound_available", input.sound_available),
            ("music_available", input.music_available),
            ("cursor_visible", input.cursor_visible),
        ] {
            if value > 1 {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("{name} must be zero or one"),
                ));
            }
        }
        let volume_preview = self.mission.player_menu_state == 10
            && input.volume_target == 1
            && self.presentation_ui.sound_volume != input.sound_volume;
        let chat_lines = std::array::from_fn(|index| {
            let start = index * 64;
            input.chat_lines[start..start + 64]
                .try_into()
                .expect("fixed chat line extent")
        });
        let chat_names = std::array::from_fn(|index| {
            let start = index * 16;
            input.chat_names[start..start + 16]
                .try_into()
                .expect("fixed chat name extent")
        });
        self.presentation_ui = LiveMenuState {
            authority_mask: input.authority_mask,
            selected_menu_item: input.selected_menu_item,
            pause_menu_suppressed: input.pause_menu_suppressed != 0,
            pause_text_suppressed: input.pause_text_suppressed != 0,
            level_end_text_suppressed: input.level_end_text_suppressed != 0,
            recursive_pause_writer: input.recursive_pause_writer != 0,
            load_available: input.load_available != 0,
            sound_available: input.sound_available != 0,
            music_available: input.music_available != 0,
            options_enabled_mask: input.options_enabled_mask,
            options_selected_mask: input.options_selected_mask,
            sound_volume: input.sound_volume,
            music_volume: input.music_volume,
            volume_target: input.volume_target,
            chat_recipient: input.chat_recipient,
            chat_mode: input.chat_mode,
            chat_player_mask: input.chat_player_mask,
            local_player_index: input.local_player_index,
            frame_timing_index: input.frame_timing_index,
            chat_lines,
            chat_names,
            notification_player_name: input.notification_player_name,
            cursor_visible: input.cursor_visible != 0,
            mouse_x: input.mouse_x,
            mouse_y: input.mouse_y,
            joystick: input.joystick,
            text_colour_phase: input.text_colour_phase,
        };
        if input.authority_mask & LIVE_MENU_AUTH_DEBUG_METRICS != 0 {
            if self.debug_clock_bucket != Some(input.debug_clock_bucket) {
                if self.debug_clock_bucket.is_some() {
                    self.debug_fps = self.source_turn.wrapping_sub(self.debug_previous_turn);
                }
                self.debug_clock_bucket = Some(input.debug_clock_bucket);
                self.debug_previous_turn = self.source_turn;
            }
            self.sound_number = input.sound_number;
            self.debug_heap_size = LEVEL1_SOURCE_HEAP_SIZE;
            self.debug_memory_used = LEVEL1_SOURCE_MEMORY_USED;
            self.debug_memory_free = LEVEL1_SOURCE_MEMORY_FREE;
        } else {
            self.debug_clock_bucket = None;
            self.debug_previous_turn = self.source_turn;
            self.debug_fps = 0;
            self.sound_number = 0;
            self.debug_heap_size = 0;
            self.debug_memory_used = 0;
            self.debug_memory_free = 0;
        }
        if volume_preview {
            self.emit_local_flight_sound(sim_core::volume_preview_audio_event(
                self.state.entity_index,
                i16::from(self.world.runtime().local_player_slot),
            ));
            // `AdjustVolume_1A070` synchronously calls PlayEntitySounds after
            // preparing its preview, including while byte_38591 selects the
            // sound slider during pause.
            self.drain_pending_audio_commands();
        }
        Ok(())
    }

    fn emit_local_flight_sound(&mut self, event: FlightEvent) {
        let record = self.encode_flight_event(event, state_hash(&self.state));
        self.append_event(record);
    }

    fn live_world_hash_v2(&self) -> u64 {
        self.live_world.hash_v2(self.mission.global_rng())
    }

    #[cfg(test)]
    fn source_render_input_hash(&self) -> AbiResult<u64> {
        let map_right_viewport = matches!(self.mission.player_menu_state, 6 | 7 | 8 | 11 | 12 | 14);
        let inputs = self.source_render_inputs(map_right_viewport)?;
        self.source_render_input_hash_for(&inputs)
    }

    fn source_render_input_hash_for(&self, inputs: &SourceRenderInputs) -> AbiResult<u64> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"mc2-source-render-input-v1\0");
        bytes.extend_from_slice(&self.terrain.deterministic_hash().to_le_bytes());
        bytes.extend_from_slice(&self.live_world_hash_v2().to_le_bytes());
        bytes.push(u8::from(inputs.map_right_viewport));
        bytes.extend_from_slice(&inputs.camera.render_x.to_le_bytes());
        bytes.extend_from_slice(&inputs.camera.render_y.to_le_bytes());
        bytes.extend_from_slice(&inputs.camera.eye_z.to_le_bytes());
        bytes.extend_from_slice(&inputs.camera.yaw.to_le_bytes());
        bytes.extend_from_slice(&inputs.camera.pitch.to_le_bytes());
        bytes.extend_from_slice(&inputs.camera.roll.to_le_bytes());
        bytes.extend_from_slice(&inputs.camera.fov.to_le_bytes());
        bytes.extend_from_slice(&inputs.camera.source_turn.to_le_bytes());
        bytes.push(u8::from(inputs.camera.reflections));
        bytes.extend_from_slice(&inputs.generation_asset_digest.to_le_bytes());
        bytes.extend_from_slice(&inputs.local_player_entity_index.to_le_bytes());
        for value in [
            self.camera_slope_sample.target_x,
            self.camera_slope_sample.target_y,
            self.camera_slope_sample.offset_x,
            self.camera_slope_sample.offset_y,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(
            &u32::try_from(inputs.frozen_actors.len())
                .expect("frozen render count fits u32")
                .to_le_bytes(),
        );
        for actor in &inputs.frozen_actors {
            bytes.extend_from_slice(&actor.source_entity_index.to_le_bytes());
            bytes.extend_from_slice(&actor.position.x.to_le_bytes());
            bytes.extend_from_slice(&actor.position.y.to_le_bytes());
            bytes.extend_from_slice(&actor.position.z.to_le_bytes());
            bytes.extend_from_slice(&actor.yaw.to_le_bytes());
            bytes.extend_from_slice(&actor.render_parameter_index.to_le_bytes());
        }
        append_source_render_records(&mut bytes, b"goats", &inputs.goats);
        append_source_render_records(&mut bytes, b"ambient", &inputs.ambient_entities);
        append_source_render_records(&mut bytes, b"actors", &inputs.mission_actors);
        append_source_render_records(&mut bytes, b"residents", &inputs.dynamic_residents);
        append_source_render_records(&mut bytes, b"segments", &inputs.mission_segments);
        append_source_render_records(&mut bytes, b"castle", inputs.castle_root.as_slice());
        append_source_render_records(&mut bytes, b"spheres", &inputs.mana_spheres);
        append_source_render_records(&mut bytes, b"carrier", inputs.mana_carrier.as_slice());
        append_source_render_records(&mut bytes, b"projectiles", &inputs.projectiles);
        append_source_render_records(&mut bytes, b"impacts", &inputs.impacts);
        append_source_render_records(&mut bytes, b"possessed", &inputs.possessed_buildings);
        append_source_render_records(&mut bytes, b"source-slots", &inputs.source_slots);
        append_source_render_records(&mut bytes, b"dispositions", &inputs.dispositions);
        let topology = inputs.map_chains.canonical_topology_bytes();
        bytes.extend_from_slice(
            &u32::try_from(topology.len())
                .expect("source map topology fits u32")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&topology);
        bytes.push(u8::from(inputs.night));
        let mut frame = |selector: u16,
                         state: u8,
                         frame_index: u8,
                         cursor: u32,
                         width: u16,
                         height: u16,
                         pixels: &[u8]| {
            bytes.extend_from_slice(&selector.to_le_bytes());
            bytes.push(state);
            bytes.push(frame_index);
            bytes.extend_from_slice(&cursor.to_le_bytes());
            bytes.extend_from_slice(&width.to_le_bytes());
            bytes.extend_from_slice(&height.to_le_bytes());
            bytes.extend_from_slice(
                &u32::try_from(pixels.len())
                    .expect("TMAP payload fits u32")
                    .to_le_bytes(),
            );
            bytes.extend_from_slice(pixels);
        };
        for clock in self
            .selector_frames
            .as_ref()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "source-render input has no Goat TMAP clocks",
                )
            })?
            .states()
        {
            frame(
                clock.selector(),
                clock.state_ordinal(),
                clock.frame_index(),
                clock.stream_cursor(),
                clock.width(),
                clock.height(),
                clock.pixels(),
            );
        }
        for clock in self
            .archer_selector_frames
            .as_ref()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "source-render input has no Archer TMAP clocks",
                )
            })?
            .states()
        {
            frame(
                clock.selector(),
                clock.state_ordinal(),
                clock.frame_index(),
                clock.stream_cursor(),
                clock.width(),
                clock.height(),
                clock.pixels(),
            );
        }
        for clock in self
            .firefly_selector_frames
            .as_ref()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "source-render input has no Firefly TMAP clocks",
                )
            })?
            .states()
        {
            frame(
                clock.selector(),
                clock.state_ordinal(),
                clock.frame_index(),
                clock.stream_cursor(),
                clock.width(),
                clock.height(),
                clock.pixels(),
            );
        }
        for clock in self
            .villager_selector_frames
            .as_ref()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "source-render input has no villager TMAP clocks",
                )
            })?
            .states()
        {
            frame(
                clock.selector(),
                clock.state_ordinal(),
                0,
                0,
                clock.width(),
                clock.height(),
                clock.pixels(),
            );
        }
        for animated in [
            self.fireball_selector_frame
                .as_ref()
                .map(FireballTmapClock::frame),
            self.possession_selector_frame
                .as_ref()
                .map(PossessionTmapClock::frame),
        ] {
            let animated = animated.ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "source-render input has no combat TMAP clock",
                )
            })?;
            frame(
                animated.selector,
                animated.state_ordinal,
                animated.frame_index,
                animated.stream_cursor,
                animated.width,
                animated.height,
                &animated.pixels,
            );
        }
        let castle = self.castle_selector_frame.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "source-render input has no Castle TMAP clock",
            )
        })?;
        let castle_frame = castle.frame();
        frame(
            castle_frame.selector,
            castle.state_ordinal(),
            0,
            0,
            castle_frame.width,
            castle_frame.height,
            &castle_frame.pixels,
        );
        Ok(sim_core::presentation_frame::byte_hash(&bytes))
    }

    fn new(world: NativeWorld) -> AbiResult<Self> {
        let terrain_frame_assets = load_optional_terrain_frame_assets()?.map(Arc::new);
        Self::new_with_terrain_frame_assets(world, terrain_frame_assets)
    }

    fn new_with_terrain_frame_assets(
        world: NativeWorld,
        terrain_frame_assets: Option<Arc<TerrainFrameAssets>>,
    ) -> AbiResult<Self> {
        let world = Arc::new(world);
        let archer_selector_frames = terrain_frame_assets
            .as_ref()
            .map(|assets| ArcherTmapClocks::from_bank(&assets.archer_tmap_bank));
        let firefly_selector_frames = terrain_frame_assets
            .as_ref()
            .map(|assets| FireflyTmapClocks::from_bank(&assets.firefly_tmap_bank));
        let villager_selector_frames = terrain_frame_assets
            .as_ref()
            .map(|assets| VillagerTmapClocks::from_bank(&assets.frozen_actor_tmap_bank));
        let fireball_selector_frame = terrain_frame_assets
            .as_ref()
            .map(|assets| FireballTmapClock::from_bank(&assets.combat_tmap_bank));
        let possession_selector_frame = terrain_frame_assets
            .as_ref()
            .map(|assets| PossessionTmapClock::from_bank(&assets.combat_tmap_bank));
        let castle_selector_frame = terrain_frame_assets
            .as_ref()
            .map(|assets| CastleTmapClock::from_bank(&assets.economy_tmap_bank));
        let frozen_actors = level1_frozen_actor_renders(world.as_ref())?;
        let renderable_map_chains =
            reset_renderable_map_chains(world.as_ref(), frozen_actors.as_ref())?;
        let player = &world.players()[usize::from(world.runtime().local_player_slot)];
        let record = player.record_bytes();
        let source_turn = player.turn;
        let camera_look_yaw = i16::from_le_bytes([record[28], record[29]]);
        let camera_look_pitch = i16::from_le_bytes([record[30], record[31]]);
        let state = *world.reset_state();
        let mut mission = world.level1_mission().session(
            world.entities()[usize::from(state.entity_index)].life,
            1_000,
        );
        mission.set_source_turn_counter(source_turn.cast_unsigned());
        mission.set_reset_source_map_chains(world.reset_source_map_chains().clone());
        let camera_fov = 128;
        let camera_history = CameraHistory::capture(
            &state,
            source_turn,
            camera_fov,
            camera_look_yaw,
            camera_look_pitch,
        );
        let mut camera_slope = TerrainSlopeCamera::default();
        let camera_slope_sample = camera_slope.update(
            world.terrain().ground_heights(),
            camera_history.x,
            camera_history.y,
        );
        let terrain = SessionTerrain::from_world(world.as_ref());
        let lighting_receipt = mission
            .dynamic_lighting_execution_receipt(&terrain.planes().shading)
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
        let source_render_asset_digest = terrain_frame_assets
            .as_ref()
            .map(|assets| source_render_asset_digest(world.as_ref(), assets))
            .transpose()?;
        Ok(Self {
            terrain,
            state,
            context: world.session_context(),
            live_world: world.reset_live_world().clone(),
            mission,
            frozen_actors,
            selector_frames: world.goat_tmap_bank().map(GoatTmapClocks::from_bank),
            archer_selector_frames,
            firefly_selector_frames,
            villager_selector_frames,
            fireball_selector_frame,
            possession_selector_frame,
            castle_selector_frame,
            renderable_map_chains,
            selector_draw_feedback: None,
            notification_draw_feedback: None,
            goat_draw_admission: None,
            world,
            generation: 1,
            event_head: 0,
            events: VecDeque::new(),
            audio_commands: BTreeMap::new(),
            audio_voices: [None; AUDIO_VOICE_COUNT],
            music_start_armed: true,
            fire_sequence: 0,
            last_resolution: 0,
            source_turn,
            camera_fov,
            camera_look_yaw,
            camera_look_pitch,
            camera_history,
            camera_slope,
            camera_slope_sample,
            terrain_frame_assets,
            source_render_asset_digest,
            presentation_frame: PresentationFrameState::new(1),
            lighting_receipt,
            presentation_ui: LiveMenuState::default(),
            debug_clock_bucket: None,
            debug_previous_turn: source_turn,
            debug_fps: 0,
            sound_number: 0,
            game_timer_turn: 0,
            debug_heap_size: 0,
            debug_memory_used: 0,
            debug_memory_free: 0,
            game_paused: false,
            pending_house_effects: Vec::new(),
            #[cfg(test)]
            immediate_released_holder_handoff: Vec::new(),
        })
    }

    fn reset(&mut self) {
        self.terrain = SessionTerrain::from_world(self.world.as_ref());
        self.state = *self.world.reset_state();
        self.context = self.world.session_context();
        self.live_world = self.world.reset_live_world().clone();
        self.mission = self.world.level1_mission().session(
            self.world.entities()[usize::from(self.state.entity_index)].life,
            1_000,
        );
        self.mission
            .set_reset_source_map_chains(self.world.reset_source_map_chains().clone());
        self.selector_frames = self.world.goat_tmap_bank().map(GoatTmapClocks::from_bank);
        self.archer_selector_frames = self
            .terrain_frame_assets
            .as_ref()
            .map(|assets| ArcherTmapClocks::from_bank(&assets.archer_tmap_bank));
        self.firefly_selector_frames = self
            .terrain_frame_assets
            .as_ref()
            .map(|assets| FireflyTmapClocks::from_bank(&assets.firefly_tmap_bank));
        self.villager_selector_frames = self
            .terrain_frame_assets
            .as_ref()
            .map(|assets| VillagerTmapClocks::from_bank(&assets.frozen_actor_tmap_bank));
        self.fireball_selector_frame = self
            .terrain_frame_assets
            .as_ref()
            .map(|assets| FireballTmapClock::from_bank(&assets.combat_tmap_bank));
        self.possession_selector_frame = self
            .terrain_frame_assets
            .as_ref()
            .map(|assets| PossessionTmapClock::from_bank(&assets.combat_tmap_bank));
        self.castle_selector_frame = self
            .terrain_frame_assets
            .as_ref()
            .map(|assets| CastleTmapClock::from_bank(&assets.economy_tmap_bank));
        self.frozen_actors = level1_frozen_actor_renders(self.world.as_ref())
            .expect("validated world reset actor renders remain valid");
        self.renderable_map_chains =
            reset_renderable_map_chains(self.world.as_ref(), self.frozen_actors.as_ref())
                .expect("validated world reset renderable chains remain valid");
        self.selector_draw_feedback = None;
        self.notification_draw_feedback = None;
        self.goat_draw_admission = None;
        self.generation = next_nonzero(self.generation);
        self.presentation_frame.reset(self.generation);
        self.event_head = 0;
        self.events.clear();
        self.audio_commands.clear();
        self.audio_voices = [None; AUDIO_VOICE_COUNT];
        self.music_start_armed = true;
        self.fire_sequence = 0;
        self.last_resolution = 0;
        self.presentation_ui = LiveMenuState::default();
        self.debug_clock_bucket = None;
        self.debug_previous_turn = self.source_turn;
        self.debug_fps = 0;
        self.sound_number = 0;
        self.game_timer_turn = 0;
        self.debug_heap_size = 0;
        self.debug_memory_used = 0;
        self.debug_memory_free = 0;
        self.game_paused = false;
        let player = &self.world.players()[usize::from(self.world.runtime().local_player_slot)];
        let record = player.record_bytes();
        self.source_turn = player.turn;
        self.mission
            .set_source_turn_counter(self.source_turn.cast_unsigned());
        self.lighting_receipt = self
            .mission
            .dynamic_lighting_execution_receipt(&self.terrain.planes().shading)
            .expect("reset dynamic-light state and authored shading have fixed extents");
        self.camera_fov = 128;
        self.camera_look_yaw = i16::from_le_bytes([record[28], record[29]]);
        self.camera_look_pitch = i16::from_le_bytes([record[30], record[31]]);
        self.camera_history = CameraHistory::capture(
            &self.state,
            self.source_turn,
            self.camera_fov,
            self.camera_look_yaw,
            self.camera_look_pitch,
        );
        self.camera_slope = TerrainSlopeCamera::default();
        self.camera_slope_sample = self.camera_slope.update(
            self.terrain.ground_heights(),
            self.camera_history.x,
            self.camera_history.y,
        );
        self.pending_house_effects.clear();
        #[cfg(test)]
        self.immediate_released_holder_handoff.clear();
    }

    fn presentation_state(&self) -> McPresentationStateV2 {
        let camera = self.camera_history;
        McPresentationStateV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McPresentationStateV2>(),
            generation: self.generation,
            tick: camera.tick,
            source_turn: camera.source_turn,
            camera_x: camera.x,
            camera_y: camera.y,
            camera_z: camera.z,
            camera_yaw: camera.yaw,
            camera_pitch: camera.pitch,
            camera_roll: camera.roll,
            camera_fov: camera.fov,
            camera_authority: MC_AUTHORITY_PLAYER_FLIGHT_LIVE,
            camera_flags: MC_PRESENTATION_CAMERA_EXACT_V2_INPUTS
                | MC_PRESENTATION_CAMERA_EYE_Z_PLUS_128,
            reserved_0: 0,
            reserved: [0; 4],
        }
    }

    fn render_camera_state(&self) -> McRenderCameraStateV2 {
        let camera = self.camera_history;
        let slope = self.camera_slope_sample;
        McRenderCameraStateV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McRenderCameraStateV2>(),
            generation: self.generation,
            tick: camera.tick,
            source_turn: camera.source_turn,
            history_x: camera.x,
            history_y: camera.y,
            eye_z: camera.z,
            yaw: camera.yaw & 0x07ff,
            pitch: camera.pitch,
            roll: camera.roll,
            fov: camera.fov,
            slope_target_x: slope.target_x,
            slope_target_y: slope.target_y,
            render_offset_x: slope.offset_x,
            render_offset_y: slope.offset_y,
            authority: MC_AUTHORITY_PLAYER_FLIGHT_LIVE,
            flags: MC_RENDER_CAMERA_TERRAIN_SLOPE_XY,
            reserved_0: 0,
            reserved: [0; 4],
        }
    }

    fn terrain_grid(&self) -> [McTerrainGridVertexV2; OUTDOOR_GRID_VERTICES] {
        let camera = self.camera_history;
        let slope = self.camera_slope_sample;
        let planes = self.terrain.planes();
        let grid = outdoor_grid(OutdoorGridInput {
            render_x: camera.x.wrapping_add(slope.offset_x.cast_unsigned()),
            render_y: camera.y.wrapping_add(slope.offset_y.cast_unsigned()),
            yaw: camera.yaw,
            turn: camera.source_turn,
            shading: &planes.shading,
            angles: &planes.angles,
        });
        std::array::from_fn(|index| {
            let source = grid[index];
            McTerrainGridVertexV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McTerrainGridVertexV2>(),
                index: u16::try_from(index).expect("terrain grid index fits u16"),
                row: u8::try_from(index / 40).expect("terrain grid row fits u8"),
                column: u8::try_from(index % 40).expect("terrain grid column fits u8"),
                tile_x: source.tile_x,
                tile_y: source.tile_y,
                shade: source.shade,
                angle: source.angle,
                view_x: source.view_x,
                view_depth: source.view_depth,
                distance_squared: source.distance_squared,
                pnt5: source.pnt5.unwrap_or(0),
                raw_wave: source.raw_wave,
                surface_displacement: source.surface_displacement,
                lighting_wave: source.lighting_wave,
                feature_flags: source.feature_flags,
                tablesn_row: source.vertex_tablesn_row.unwrap_or(0),
                pnt5_written: u8::from(source.pnt5.is_some()),
                reserved: [0; 4],
            }
        })
    }

    fn terrain_frame(&self) -> AbiResult<Level1TerrainFrame> {
        let assets = self.terrain_frame_assets.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "isolated terrain frame unavailable: MC_ASSET_CACHE_PATH was not supplied when the session opened",
            )
        })?;
        let camera = self.camera_history;
        let slope = self.camera_slope_sample;
        let planes = self.terrain.planes();
        render_level1_terrain_frame(
            Level1TerrainFrameCamera {
                render_x: camera.x.wrapping_add(slope.offset_x.cast_unsigned()),
                render_y: camera.y.wrapping_add(slope.offset_y.cast_unsigned()),
                eye_z: i32::from(camera.z),
                yaw: camera.yaw,
                pitch: camera.pitch,
                roll: camera.roll,
                fov: camera.fov,
                source_turn: camera.source_turn,
                reflections: true,
            },
            Level1TerrainFrameResources {
                terrain_types: &planes.terrain_types,
                ground_heights: &planes.ground_heights,
                shading: &planes.shading,
                angles: &planes.angles,
                terrain_atlas: &assets.terrain_atlas,
                tablesn: &assets.tablesn,
                sky: &assets.sky,
            },
        )
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))
    }

    fn level1_palette_rgb8(&self) -> AbiResult<[u8; 768]> {
        let assets = self.terrain_frame_assets.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "Level 1 palette unavailable: MC_ASSET_CACHE_PATH was not supplied when the session opened",
            )
        })?;
        let original: &[u8; 768] = assets.palette_rgb6.as_ref().try_into().map_err(|_| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "qualified PALN RGB6 palette must contain exactly 768 bytes",
            )
        })?;
        let rgb6 = self.mission.effective_palette_rgb6(original);
        Ok(rgb6.map(|channel| channel.saturating_mul(4)))
    }

    fn render_world_frame(
        &self,
        ambient_entities: &[Level1AmbientEntity],
        map_chains: &RenderableMapChains,
    ) -> AbiResult<Level1TerrainFrame> {
        let unavailable = |message| AbiError::new(MC_STATUS_VALIDATION_FAILED, message);
        let assets = self.terrain_frame_assets.as_ref().ok_or_else(|| {
            unavailable(
                "world frame unavailable: MC_ASSET_CACHE_PATH was not supplied when the session opened",
            )
        })?;
        let tmap_clocks = self.selector_frames.as_ref().ok_or_else(|| {
            unavailable("world frame unavailable without qualified Goat TMAP descriptors")
        })?;
        let archer_tmap_clocks = self.archer_selector_frames.as_ref().ok_or_else(|| {
            unavailable("world frame unavailable without qualified Archer TMAP descriptors")
        })?;
        let firefly_tmap_clocks = self.firefly_selector_frames.as_ref().ok_or_else(|| {
            unavailable("world frame unavailable without qualified Firefly TMAP descriptors")
        })?;
        let villager_tmap_clocks = self.villager_selector_frames.as_ref().ok_or_else(|| {
            unavailable("world frame unavailable without qualified villager TMAP descriptors")
        })?;
        let fireball_tmap_clock = self.fireball_selector_frame.as_ref().ok_or_else(|| {
            unavailable("world frame unavailable without qualified Fireball TMAP descriptor")
        })?;
        let possession_tmap_clock = self.possession_selector_frame.as_ref().ok_or_else(|| {
            unavailable("world frame unavailable without qualified Possession TMAP descriptor")
        })?;
        let castle_tmap_clock = self.castle_selector_frame.as_ref().ok_or_else(|| {
            unavailable("world frame unavailable without qualified Castle TMAP descriptor")
        })?;
        let camera = self.camera_history;
        let slope = self.camera_slope_sample;
        let planes = self.terrain.planes();
        let dynamic_residents = self
            .mission
            .dynamic_residents()
            .copied()
            .collect::<Vec<_>>();
        render_level1_world_frame(
            Level1TerrainFrameCamera {
                render_x: camera.x.wrapping_add(slope.offset_x.cast_unsigned()),
                render_y: camera.y.wrapping_add(slope.offset_y.cast_unsigned()),
                eye_z: i32::from(camera.z),
                yaw: camera.yaw,
                pitch: camera.pitch,
                roll: camera.roll,
                fov: camera.fov,
                source_turn: camera.source_turn,
                reflections: true,
            },
            Level1TerrainFrameResources {
                terrain_types: &planes.terrain_types,
                ground_heights: &planes.ground_heights,
                shading: &planes.shading,
                angles: &planes.angles,
                terrain_atlas: &assets.terrain_atlas,
                tablesn: &assets.tablesn,
                sky: &assets.sky,
            },
            Level1WorldFrameResources {
                local_player_entity_index: self.state.entity_index,
                goats: self.live_world.goats(),
                frozen_actors: &self.frozen_actors,
                ambient_entities,
                frozen_actor_tmap_bank: Some(&assets.frozen_actor_tmap_bank),
                villager_tmap_clocks: Some(villager_tmap_clocks),
                mission_actors: self.mission.actors(),
                dynamic_residents: &dynamic_residents,
                mission_segments: self.mission.multipart_segments(),
                castle_root: self
                    .mission
                    .castle_exists
                    .then_some(Level1CastleRootRender {
                        source_entity_index: self.mission.castle_source_entity_index,
                        position: self.mission.castle_position,
                        active: true,
                    }),
                mana_spheres: self.mission.mana_spheres(),
                mana_carrier: self.mission.mana_carrier(),
                map_chains,
                goat_tmap_clocks: tmap_clocks,
                archer_tmap_bank: &assets.archer_tmap_bank,
                archer_tmap_clocks,
                firefly_tmap_bank: &assets.firefly_tmap_bank,
                firefly_tmap_clocks,
                projectiles: self.mission.projectiles(),
                impacts: self.mission.impacts(),
                possessed_buildings: self.mission.possessed_buildings(),
                combat_tmap_bank: &assets.combat_tmap_bank,
                static_world_tmap_bank: &assets.static_world_tmap_bank,
                economy_tmap_bank: &assets.economy_tmap_bank,
                fireball_tmap_clock,
                possession_tmap_clock,
                castle_tmap_clock,
                particle_parameters: &self.world.presentation().particles,
                source_slots: self.mission.source_slots(),
                night: self.world.metadata().map_type == sim_core::native_world::MapType::Night,
            },
        )
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))
    }

    fn source_sprite_dispositions(&self) -> AbiResult<Vec<SourceSpriteDisposition>> {
        let mut dispositions = self
            .mission
            .source_sprite_dispositions(&self.state)
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
        let mut qualified = dispositions
            .iter()
            .map(source_sprite_disposition_index)
            .collect::<BTreeSet<_>>();
        let legacy = self
            .mission
            .source_sprite_legacy_dispositions(&qualified)
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
        qualified.extend(legacy.iter().map(source_sprite_disposition_index));
        dispositions.extend(legacy);

        let mut imported_rows = Vec::new();
        for source_entity_index in self.renderable_map_chains.linked_entity_indices() {
            if qualified.contains(&source_entity_index) {
                continue;
            }
            let entity = self
                .world
                .entities()
                .get(usize::from(source_entity_index))
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        unknown_source_sprite_tuple(source_entity_index, 0, 0, 0, 0),
                    )
                })?;
            let source = self
                .mission
                .source_slots()
                .get(usize::from(source_entity_index))
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        unknown_source_sprite_tuple(source_entity_index, 0, 0, 0, 0),
                    )
                })?;
            let route = if entity.authority == EntityAuthority::GoatLive {
                if (source.class, source.model) != (GOAT_CLASS, GOAT_MODEL)
                    || !matches!(source.action, 9 | 12 | 13 | 14 | GOAT_PATROL_ACTION)
                {
                    return Err(AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        format!(
                            "live Goat source row mismatch: slot={source_entity_index} source={}/{}/{} phase={} rng={} flags={:#x}",
                            source.class,
                            source.model,
                            source.action,
                            source.phase,
                            source.entity_rng,
                            source.control_flags,
                        ),
                    ));
                }
                SourceSpriteContractRoute::Legacy(SourceSpriteLegacyFamily::Goat)
            } else if entity.authority == EntityAuthority::VillagerLive {
                if (source.class, source.model) != (VILLAGER_CLASS, VILLAGER_MODEL)
                    || !matches!(source.action, 104..=111)
                {
                    return Err(AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        format!(
                            "live Villager source row mismatch: slot={source_entity_index} source={}/{}/{} phase={} rng={} flags={:#x}",
                            source.class,
                            source.model,
                            source.action,
                            source.phase,
                            source.entity_rng,
                            source.control_flags,
                        ),
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
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        unknown_source_sprite_tuple(
                            source_entity_index,
                            source.class,
                            source.model,
                            source.action,
                            entity.particle_parameter_index.unwrap_or(0),
                        ),
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
        let imported = source_sprite_dispositions(imported_rows)
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
        qualified.extend(imported.iter().map(source_sprite_disposition_index));
        dispositions.extend(imported);
        dispositions.sort_unstable_by_key(source_sprite_disposition_index);
        for source_entity_index in self.renderable_map_chains.linked_entity_indices() {
            if !qualified.contains(&source_entity_index) {
                let (class, model, action) = self
                    .mission
                    .source_slots()
                    .get(usize::from(source_entity_index))
                    .map_or((0, 0, 0), |source| {
                        (source.class, source.model, source.action)
                    });
                return Err(AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    unknown_source_sprite_tuple(source_entity_index, class, model, action, 0),
                ));
            }
        }
        Ok(dispositions)
    }

    fn source_render_inputs(&self, map_right_viewport: bool) -> AbiResult<SourceRenderInputs> {
        let dispositions = self.source_sprite_dispositions()?;
        let mut map_chains = self.renderable_map_chains.clone();
        for disposition in &dispositions {
            let SourceSpriteDisposition::Render(submission) = disposition else {
                continue;
            };
            if submission.source_entity_index == self.state.entity_index {
                map_chains
                    .sync_entity(RenderableMapSeed {
                        entity_index: submission.source_entity_index,
                        active: true,
                        x: submission.position.x,
                        y: submission.position.y,
                    })
                    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
            }
        }
        let assets = self.terrain_frame_assets.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "source-render generation has no qualified asset banks",
            )
        })?;
        let generation_asset_digest = source_render_asset_digest(self.world.as_ref(), assets)?;
        if Some(generation_asset_digest) != self.source_render_asset_digest {
            return Err(AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "source-render asset banks changed within the active generation",
            ));
        }
        let camera = Level1TerrainFrameCamera {
            render_x: self
                .camera_history
                .x
                .wrapping_add(self.camera_slope_sample.offset_x.cast_unsigned()),
            render_y: self
                .camera_history
                .y
                .wrapping_add(self.camera_slope_sample.offset_y.cast_unsigned()),
            eye_z: i32::from(self.camera_history.z).wrapping_add(if map_right_viewport {
                128
            } else {
                0
            }),
            yaw: i16::try_from(self.camera_history.yaw.cast_unsigned() & 0x07ff)
                .expect("masked source yaw fits i16"),
            pitch: self.camera_history.pitch,
            roll: self.camera_history.roll,
            fov: self.camera_history.fov,
            source_turn: self.camera_history.source_turn,
            reflections: true,
        };
        Ok(SourceRenderInputs {
            map_right_viewport,
            camera,
            generation_asset_digest,
            local_player_entity_index: self.state.entity_index,
            goats: self.live_world.goats().to_vec(),
            frozen_actors: self.frozen_actors.to_vec(),
            ambient_entities: self.mission.ambient_entities().copied().collect(),
            mission_actors: self.mission.actors().to_vec(),
            dynamic_residents: self.mission.dynamic_residents().copied().collect(),
            mission_segments: self.mission.multipart_segments().to_vec(),
            castle_root: self
                .mission
                .castle_exists
                .then_some(Level1CastleRootRender {
                    source_entity_index: self.mission.castle_source_entity_index,
                    position: self.mission.castle_position,
                    active: true,
                }),
            mana_spheres: self.mission.mana_spheres().to_vec(),
            mana_carrier: self.mission.mana_carrier().copied(),
            map_chains,
            projectiles: self.mission.projectiles().to_vec(),
            impacts: self.mission.impacts().to_vec(),
            possessed_buildings: self.mission.possessed_buildings().to_vec(),
            source_slots: self.mission.source_slots().to_vec(),
            dispositions,
            night: self.world.metadata().map_type == sim_core::native_world::MapType::Night,
        })
    }

    fn render_source_world_frame(
        &self,
        inputs: &SourceRenderInputs,
    ) -> AbiResult<Level1TerrainFrame> {
        let unavailable = |message| AbiError::new(MC_STATUS_VALIDATION_FAILED, message);
        let assets = self.terrain_frame_assets.as_ref().ok_or_else(|| {
            unavailable(
                "source world frame unavailable: MC_ASSET_CACHE_PATH was not supplied when the session opened",
            )
        })?;
        let source_bank = self.world.source_sprite_tmap_bank().ok_or_else(|| {
            unavailable("source world frame unavailable without the CP1 source TMAP bank")
        })?;
        let tmap_clocks = self.selector_frames.as_ref().ok_or_else(|| {
            unavailable("source world frame unavailable without Goat TMAP clocks")
        })?;
        let archer_tmap_clocks = self.archer_selector_frames.as_ref().ok_or_else(|| {
            unavailable("source world frame unavailable without Archer TMAP clocks")
        })?;
        let firefly_tmap_clocks = self.firefly_selector_frames.as_ref().ok_or_else(|| {
            unavailable("source world frame unavailable without Firefly TMAP clocks")
        })?;
        let villager_tmap_clocks = self.villager_selector_frames.as_ref().ok_or_else(|| {
            unavailable("source world frame unavailable without villager TMAP clocks")
        })?;
        let fireball_tmap_clock = self.fireball_selector_frame.as_ref().ok_or_else(|| {
            unavailable("source world frame unavailable without Fireball TMAP clock")
        })?;
        let possession_tmap_clock = self.possession_selector_frame.as_ref().ok_or_else(|| {
            unavailable("source world frame unavailable without Possession TMAP clock")
        })?;
        let castle_tmap_clock = self.castle_selector_frame.as_ref().ok_or_else(|| {
            unavailable("source world frame unavailable without Castle TMAP clock")
        })?;
        let planes = self.terrain.planes();
        let render = if inputs.map_right_viewport {
            render_level1_source_world_map_frame_composed
        } else {
            render_level1_source_world_frame
        };
        render(
            inputs.camera,
            Level1TerrainFrameResources {
                terrain_types: &planes.terrain_types,
                ground_heights: &planes.ground_heights,
                shading: &planes.shading,
                angles: &planes.angles,
                terrain_atlas: &assets.terrain_atlas,
                tablesn: &assets.tablesn,
                sky: &assets.sky,
            },
            Level1WorldFrameResources {
                local_player_entity_index: inputs.local_player_entity_index,
                goats: &inputs.goats,
                frozen_actors: &inputs.frozen_actors,
                ambient_entities: &inputs.ambient_entities,
                frozen_actor_tmap_bank: Some(&assets.frozen_actor_tmap_bank),
                villager_tmap_clocks: Some(villager_tmap_clocks),
                mission_actors: &inputs.mission_actors,
                dynamic_residents: &inputs.dynamic_residents,
                mission_segments: &inputs.mission_segments,
                castle_root: inputs.castle_root,
                mana_spheres: &inputs.mana_spheres,
                mana_carrier: inputs.mana_carrier.as_ref(),
                map_chains: &inputs.map_chains,
                goat_tmap_clocks: tmap_clocks,
                archer_tmap_bank: &assets.archer_tmap_bank,
                archer_tmap_clocks,
                firefly_tmap_bank: &assets.firefly_tmap_bank,
                firefly_tmap_clocks,
                projectiles: &inputs.projectiles,
                impacts: &inputs.impacts,
                possessed_buildings: &inputs.possessed_buildings,
                combat_tmap_bank: &assets.combat_tmap_bank,
                static_world_tmap_bank: &assets.static_world_tmap_bank,
                economy_tmap_bank: &assets.economy_tmap_bank,
                fireball_tmap_clock,
                possession_tmap_clock,
                castle_tmap_clock,
                particle_parameters: &self.world.presentation().particles,
                source_slots: &inputs.source_slots,
                night: inputs.night,
            },
            SourceSpriteWorldResources {
                dispositions: &inputs.dispositions,
                tmap_bank: source_bank,
            },
        )
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))
    }

    fn world_frame(&mut self) -> AbiResult<Level1TerrainFrame> {
        let ambient_entities = self.mission.ambient_entities().copied().collect::<Vec<_>>();
        let frame = self.render_world_frame(&ambient_entities, &self.renderable_map_chains)?;
        self.commit_world_frame_feedback(&frame)?;
        Ok(frame)
    }

    fn commit_world_frame_feedback(&mut self, frame: &Level1TerrainFrame) -> AbiResult<()> {
        let mask = frame.goat_selector_mask;
        self.selector_frames
            .as_mut()
            .expect("world-frame selector clocks were validated")
            .latch_draw_attempts(mask)
            .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
        self.archer_selector_frames
            .as_mut()
            .expect("world-frame Archer selector clocks were validated")
            .latch_draw_attempts(frame.archer_selector_mask)
            .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
        self.firefly_selector_frames
            .as_mut()
            .expect("world-frame Firefly selector clocks were validated")
            .latch_draw_attempts(frame.firefly_selector_mask);
        self.villager_selector_frames
            .as_mut()
            .expect("world-frame villager selector clocks were validated")
            .latch_draw_attempts(frame.frozen_actor_selector_mask)
            .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
        self.fireball_selector_frame
            .as_mut()
            .expect("world-frame Fireball selector clock was validated")
            .latch_draw_attempt(frame.fireball_draw_attempted);
        self.possession_selector_frame
            .as_mut()
            .expect("world-frame Possession selector clock was validated")
            .latch_draw_attempt(frame.possession_draw_attempted);
        self.castle_selector_frame
            .as_mut()
            .expect("world-frame Castle selector clock was validated")
            .latch_draw_attempt(frame.castle_draw_attempted);
        let feedback_tick = self.state.tick;
        let combined_mask = self
            .selector_draw_feedback
            .filter(|(tick, _)| *tick == feedback_tick)
            .map_or(mask, |(_, prior_mask)| prior_mask | mask);
        self.selector_draw_feedback = Some((feedback_tick, combined_mask));
        Ok(())
    }

    fn presentation_frame(
        &mut self,
        query: McPresentationFrameQueryV2,
    ) -> AbiResult<sim_core::presentation_frame::PresentationFrameResult> {
        let assets = Arc::clone(self.terrain_frame_assets.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "presentation frame unavailable without qualified owner cache",
            )
        })?);
        if query.menu_overlay > 6 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                format!(
                    "presentation menu overlay {} is not supported",
                    query.menu_overlay
                ),
            ));
        }
        let hud = level1_hud_snapshot(self);
        let spell_menu = level1_spell_menu_snapshot(self);
        let minimap_entities_abi = level1_minimap_entity_commands(self);
        let minimap_entities = minimap_entities_abi
            .iter()
            .map(|command| Level1MinimapEntityCommand {
                source_slot: command.source_slot,
                x: command.x,
                y: command.y,
                selector: command.selector,
                kind: command.kind,
                palette_index: command.palette_index,
                size: command.size,
                anchor: command.anchor,
                help_selector: command.help_selector,
            })
            .collect::<Vec<_>>();
        let map_entities = level1_map_entity_commands(self);
        let spell_slot = |slot: McLevel1HudSpellSlotV2| LiveSpellSlot {
            holder_entity_index: slot.holder_entity_index,
            model: slot.model,
            tier: slot.tier,
            enabled_flags: slot.enabled_flags,
            active_value: slot.active_value,
            max_mana: slot.max_mana,
            mana_regen: slot.mana_regen,
            caster_mana: slot.caster_mana,
            frame_selector: slot.frame_selector,
            icon_selector: slot.icon_selector,
        };
        let fire = self.context.spell_fire.as_ref();
        let mut family_available = std::array::from_fn(|family| {
            u8::from(
                assets
                    .indexed_overlays
                    .spell_menu_profile(family)
                    .expect("qualified cache has all 26 spell families")
                    .family_header_tier_count
                    != 0,
            )
        });
        // DrawBottomSpellsMenu suppresses the cave-only family in the outdoor
        // Level 1 profile even though the source descriptor itself is live.
        family_available[25] = 0;
        let holder_entity_index = fire.map_or([0_u16; 26], |fire| fire.enabled_holder_indices);
        let family_unlocked_tier = fire.map_or([0_i8; 26], |fire| fire.spell_levels);
        let family_selected_tier = fire.map_or([0_i8; 26], |fire| fire.family_selected_tiers);
        let family_hand_eligibility = fire.map_or([0_u8; 26], |fire| fire.spell_hand_eligibility);
        let family_experience = fire.map_or([0_i32; 26], |fire| fire.spell_experience);
        let family_enabled_flags = std::array::from_fn(|family| {
            assets
                .indexed_overlays
                .spell_menu_profile(family)
                .expect("qualified cache has all 26 spell families")
                .family_flags
        });
        let family_active_value = std::array::from_fn(|family| {
            let holder = holder_entity_index[family];
            fire.and_then(|fire| {
                fire.holders
                    .iter()
                    .find(|candidate| candidate.entity_index == holder)
            })
            .map_or(0, |holder| holder.active_value)
        });
        let family_mana_cost = std::array::from_fn(|family| {
            assets
                .indexed_overlays
                .spell_menu_profile(family)
                .expect("qualified cache has all 26 spell families")
                .mana_cost
        });
        let family_max_mana_limit = std::array::from_fn(|family| {
            assets
                .indexed_overlays
                .spell_menu_profile(family)
                .expect("qualified cache has all 26 spell families")
                .max_mana_limit
        });
        let debug_source = &self.world.entities()[usize::from(self.state.entity_index)];
        let debug_record = debug_source.record_bytes();
        let read_i16 =
            |offset: usize| i16::from_le_bytes([debug_record[offset], debug_record[offset + 1]]);
        let read_u16 =
            |offset: usize| u16::from_le_bytes([debug_record[offset], debug_record[offset + 1]]);
        let read_i32 = |offset: usize| {
            i32::from_le_bytes([
                debug_record[offset],
                debug_record[offset + 1],
                debug_record[offset + 2],
                debug_record[offset + 3],
            ])
        };
        let map_mode = matches!(self.mission.player_menu_state, 6 | 7 | 8 | 11 | 12 | 14);
        let bridge = build_level1_final_main(
            &assets.indexed_overlays,
            Level1PresentationLive {
                flight: self.state,
                menu_state: self.mission.player_menu_state,
                player_life: self.mission.player_life,
                player_immobilize_ticks: self.state.immobilize_counter.cast_unsigned().into(),
                terrain: self.terrain.planes(),
                hud: LiveHud {
                    mana_color_index: hud.mana_color_index,
                    mana_max_color_index: hud.mana_max_color_index,
                    life_color_index: hud.life_color_index,
                    color_phase_1: hud.color_phase_1,
                    color_phase_2: hud.color_phase_2,
                    player_hit_frame_ticks: hud.player_hit_frame_ticks,
                    castle_hit_frame_ticks: hud.castle_hit_frame_ticks,
                    global_mana_denominator: hud.global_mana_denominator,
                    secured_mana_component: hud.secured_mana_component,
                    goal_percent: hud.goal_percent,
                    player_life: self.mission.player_life,
                    player_max_life: self.mission.player_max_life,
                    player_mana: self.mission.player_mana,
                    player_max_mana: self.mission.player_max_mana,
                    castle_active: self.mission.castle_exists,
                    castle_level: self.mission.castle_level,
                    castle_life: self.mission.castle_life,
                    castle_max_life: self.mission.castle_max_life,
                    castle_mana: self.mission.castle_mana,
                    castle_mana_capacity: self.mission.castle_mana_capacity,
                    player_status_flags: hud.player_status_flags,
                    player_status_word: u16::from(self.mission.exit_player_status_flags),
                    balloon_level: 0,
                    balloon_life: [0; 3],
                    balloon_max_life: [0; 3],
                    balloon_mana: [0; 3],
                    balloon_max_mana: [0; 3],
                    left: spell_slot(hud.left),
                    right: spell_slot(hud.right),
                },
                spell_menu: LiveSpellMenu {
                    selected_ui_index: spell_menu.selected_ui_index,
                    selected_tier: spell_menu.selected_tier,
                    left_family: spell_menu.left_family,
                    right_family: spell_menu.right_family,
                    family_available,
                    holder_entity_index,
                    family_unlocked_tier,
                    family_selected_tier,
                    family_hand_eligibility,
                    family_enabled_flags,
                    family_active_value,
                    family_experience,
                    family_mana_cost,
                    family_max_mana_limit,
                    castle_mana: self.mission.castle_mana,
                    caster_mana: self.mission.player_mana,
                    player_colour_0: hud.mana_color_index,
                    player_colour_1: hud.mana_max_color_index,
                    active_suppression_phase: hud.color_phase_1 != 0,
                    cursor_visible: self.presentation_ui.cursor_visible
                        && matches!(self.presentation_ui.joystick, 4 | 5 | 6 | 8 | 11 | 12 | 13),
                    cursor_x: self.presentation_ui.mouse_x / 2,
                    cursor_y: self.presentation_ui.mouse_y / 2,
                },
                menu: self.presentation_ui,
                debug_entity: LiveDebugEntity {
                    pointer_value: u32::from_le_bytes([
                        debug_record[0],
                        debug_record[1],
                        debug_record[2],
                        debug_record[3],
                    ]),
                    source_slot: debug_source.index,
                    class: debug_source.class,
                    model: debug_source.model,
                    action: debug_source.action,
                    life: debug_source.life,
                    max_life: debug_source.max_life,
                    actual_speed: debug_source.actual_speed,
                    min_speed: read_i16(132),
                    max_speed: read_i16(134),
                    position: self.state.position,
                    source_id: debug_source.source_id,
                    parent_id: read_u16(40),
                    mana: debug_source.mana,
                    max_mana: read_i32(140),
                },
                minimap_entities: &minimap_entities,
                minimap_marks: self.mission.minimap_marks(&self.state),
                map_entities: &map_entities,
                map_marks: self.mission.map_marks(&self.state),
                objective_selector: self.mission.current_text_selector,
                objective_ticks: u16::from(self.mission.objective_textbox_ticks),
                level_completed: self.mission.level_completed,
                exit_sequence_active: self.mission.exit_sequence_active,
                exit_display_blank: self.mission.exit_display_blank,
                exit_blur_mode: self.mission.exit_blur_mode,
                notification_active: self.mission.player_notification_active,
                notification_id: self.mission.player_notification_id,
                notification_state: self.mission.player_notification_state,
                notification_timer: self.mission.player_notification_timer,
                notification_player_name: self.presentation_ui.notification_player_name,
                level_number: 1,
                player_turn: self.source_turn,
                sound_number: self.sound_number,
                game_timer_turn: self.game_timer_turn,
                debug_fps: self.debug_fps,
                active_entity_count: u16::try_from(
                    self.mission
                        .source_slots()
                        .iter()
                        .filter(|slot| slot.active)
                        .count(),
                )
                .expect("source arena count fits u16"),
                heap_size: self.debug_heap_size,
                memory_used: self.debug_memory_used,
                memory_free: self.debug_memory_free,
            },
            PresentationBridgeQuery {
                minimap_enabled: query.minimap_enabled != 0,
                top_bar_enabled: query.top_bar_enabled != 0,
                menu_overlay: query.menu_overlay,
                game_paused: query.game_paused != 0,
                level_end: query.level_end != 0,
                objective_selector: (query.objective_selector != u8::MAX)
                    .then_some(query.objective_selector),
                objective_textbox: query.objective_textbox != 0,
                tutorial_or_briefing: query.tutorial_or_briefing != 0,
                help_popup: query.help_popup != 0,
                debug_flags: query.debug_text,
            },
        )
        .map_err(|error| AbiError::new(MC_STATUS_GENERATION_MISMATCH, error.0))?;
        let current_lighting_receipt = self
            .mission
            .dynamic_lighting_execution_receipt(&self.terrain.planes().shading)
            .map_err(|error| AbiError::new(MC_STATUS_GENERATION_MISMATCH, error))?;
        if current_lighting_receipt != self.lighting_receipt {
            return Err(AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "dynamic-light state/shading changed without an executed scheduler receipt",
            ));
        }
        let source_render_inputs = self.source_render_inputs(map_mode)?;
        let notification_feedback = bridge.notification_feedback;
        let input = PresentationFrameInput {
            generation: query.generation,
            tick: query.tick,
            presentation_revision: query.presentation_revision,
            state_hash: state_hash(&self.state),
            source_arena_hash: self.mission.source_arena_hash_v2(),
            source_render_input_hash: self.source_render_input_hash_for(&source_render_inputs)?,
            lighting_hash: self.mission.dynamic_lighting_hash_v1(),
            camera_hash: camera_history_hash(self.camera_history),
            source_reflections_expected: i32::from(self.camera_history.z)
                < sim_core::terrain_reflection::REFLECTION_EYE_Z_LIMIT,
            postprocess: IndexedPostprocessInput {
                configured_mode: if self.mission.exit_sequence_active {
                    self.mission.exit_blur_mode
                } else {
                    query.configured_blur_mode
                },
                blur_capability: query.blur_capability != 0,
                high_speed_system: query.high_speed_system != 0,
                scratch_present: query.scratch_present != 0,
                vga_resolution: query.vga_resolution,
                menu_state: self.mission.player_menu_state,
                viewport_width: query.viewport_width,
                viewport_height: query.viewport_height,
                screen_stride: query.screen_stride,
                actual_speed: self.state.actual_speed,
                spatial_smoothing: query.spatial_smoothing != 0,
            },
            overlay: bridge.overlay,
            lighting_receipt: Some(self.lighting_receipt),
        };
        let mut goat_candidate = self.selector_frames.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "presentation source renderer has no Goat selector clocks",
            )
        })?;
        let mut archer_candidate = self.archer_selector_frames.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "presentation source renderer has no Archer selector clocks",
            )
        })?;
        let mut firefly_candidate = self.firefly_selector_frames.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "presentation source renderer has no Firefly selector clocks",
            )
        })?;
        let mut villager_candidate = self.villager_selector_frames.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "presentation source renderer has no villager selector clocks",
            )
        })?;
        let mut fireball_candidate = self.fireball_selector_frame.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "presentation source renderer has no Fireball selector clock",
            )
        })?;
        let mut possession_candidate = self.possession_selector_frame.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "presentation source renderer has no Possession selector clock",
            )
        })?;
        let mut castle_candidate = self.castle_selector_frame.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "presentation source renderer has no Castle selector clock",
            )
        })?;
        let mut presentation =
            std::mem::replace(&mut self.presentation_frame, PresentationFrameState::new(0));
        let mut feedback_candidate = None;
        let result =
            presentation.finalize(input, &assets.tablesn, &assets.indexed_overlays, || {
                let frame = self
                    .render_source_world_frame(&source_render_inputs)
                    .map_err(|error| PresentationFrameError(error.message))?;
                let receipt = frame
                    .source_execution_receipt()
                    .map_err(|error| PresentationFrameError(error.0))?;
                goat_candidate
                    .latch_draw_attempts(frame.goat_selector_mask)
                    .map_err(PresentationFrameError)?;
                archer_candidate
                    .latch_draw_attempts(frame.archer_selector_mask)
                    .map_err(PresentationFrameError)?;
                firefly_candidate.latch_draw_attempts(frame.firefly_selector_mask);
                villager_candidate
                    .latch_draw_attempts(frame.frozen_actor_selector_mask)
                    .map_err(PresentationFrameError)?;
                fireball_candidate.latch_draw_attempt(frame.fireball_draw_attempted);
                possession_candidate.latch_draw_attempt(frame.possession_draw_attempted);
                castle_candidate.latch_draw_attempt(frame.castle_draw_attempted);
                feedback_candidate = Some((
                    goat_candidate,
                    archer_candidate,
                    firefly_candidate,
                    villager_candidate,
                    fireball_candidate,
                    possession_candidate,
                    castle_candidate,
                    frame.goat_selector_mask,
                ));
                Ok(SourceRenderedFrame {
                    map_world: map_mode.then(|| frame.indices.clone()),
                    indices: frame.indices,
                    receipt,
                })
            });
        self.presentation_frame = presentation;
        let result =
            result.map_err(|error| AbiError::new(MC_STATUS_GENERATION_MISMATCH, error.0))?;
        if result.newly_committed {
            let (goat, archer, firefly, villager, fireball, possession, castle, goat_mask) =
                feedback_candidate
                    .expect("newly committed presentation owns validated clock candidates");
            self.selector_frames = Some(goat);
            self.archer_selector_frames = Some(archer);
            self.firefly_selector_frames = Some(firefly);
            self.villager_selector_frames = Some(villager);
            self.fireball_selector_frame = Some(fireball);
            self.possession_selector_frame = Some(possession);
            self.castle_selector_frame = Some(castle);
            self.selector_draw_feedback = Some((self.state.tick, goat_mask));
            if let Some((active, state, timer)) = notification_feedback {
                self.notification_draw_feedback = Some((self.state.tick, active, state, timer));
            }
        }
        Ok(result)
    }

    fn selector_frame_batch(
        &self,
        start_ordinal: u32,
        written: u32,
    ) -> AbiResult<McSelectorFrameBatchV2> {
        let frames = self.selector_frames.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "selector-frame state unavailable: MC_ASSET_CACHE_PATH was not supplied when the session opened",
            )
        })?;
        let (last_draw_tick, last_draw_mask, has_draw_feedback) = self
            .selector_draw_feedback
            .map_or((0, 0, 0), |(tick, mask)| (tick, u64::from(mask), 1));
        let written_usize = usize::try_from(written)
            .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "selector written count exceeds usize"))?;
        let start_usize = usize::try_from(start_ordinal)
            .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "selector start exceeds usize"))?;
        Ok(McSelectorFrameBatchV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McSelectorFrameBatchV2>(),
            generation: self.generation,
            tick: self.state.tick,
            selector_state_hash: selector_state_hash_v1(frames, self.selector_draw_feedback),
            last_draw_tick,
            last_draw_mask,
            valid_draw_mask: u64::from(GOAT_TMAP_DRAW_MASK),
            start_ordinal,
            written,
            total_count: GOAT_TMAP_SELECTORS.len() as u32,
            has_more: u8::from(start_usize + written_usize < GOAT_TMAP_SELECTORS.len()),
            has_draw_feedback,
            reserved_0: 0,
            reserved: [0; 4],
        })
    }

    fn evaluate_goat_draw_admission(&mut self) -> AbiResult<McSelectorFrameBatchV2> {
        if self.world.goat_tmap_bank().is_none() {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "Goat CPU draw admission unavailable without qualified owner-cache descriptors",
            ));
        }
        let camera = self.camera_history;
        let slope = self.camera_slope_sample;
        let planes = self.terrain.planes();
        let render_x = camera.x.wrapping_add(slope.offset_x.cast_unsigned());
        let render_y = camera.y.wrapping_add(slope.offset_y.cast_unsigned());
        let lattice = outdoor_grid(OutdoorGridInput {
            render_x,
            render_y,
            yaw: camera.yaw,
            turn: camera.source_turn,
            shading: &planes.shading,
            angles: &planes.angles,
        });
        let visited = outdoor_billboard_visit_map(&lattice, camera.yaw);
        let admission = evaluate_goat_draw_admission(
            self.live_world.goats(),
            GoatDrawCamera {
                render_x,
                render_y,
                yaw: camera.yaw,
            },
            &visited,
            true,
        );
        let mask = admission.selector_mask;
        let frames = self.selector_frames.as_mut().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "selector-frame state unavailable without qualified owner cache",
            )
        })?;
        frames
            .latch_draw_attempts(mask)
            .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
        let combined_mask = self
            .selector_draw_feedback
            .filter(|(tick, _)| *tick == self.state.tick)
            .map_or(mask, |(_, prior_mask)| prior_mask | mask);
        self.selector_draw_feedback = Some((self.state.tick, combined_mask));
        self.goat_draw_admission = Some(admission);
        let mut receipt = self.selector_frame_batch(0, 0)?;
        receipt.has_more = 0;
        Ok(receipt)
    }

    fn snapshot(&self) -> McSessionSnapshotV2 {
        let player = &self.world.players()[usize::from(self.world.runtime().local_player_slot)];
        let fire = self.context.spell_fire.as_ref();
        let selected = fire.map_or(0, |fire| fire.selected_spell_ui_index);
        let left_holder = fire.map_or(0, |fire| fire.left_holder_index);
        let right_holder = fire.map_or(0, |fire| fire.right_holder_index);
        let selected_holder = fire.map_or(0, |fire| fire.selected_holder_index);
        let holder_model = |holder_index| {
            fire.and_then(|fire| {
                fire.holders
                    .iter()
                    .find(|holder| holder.entity_index == holder_index)
                    .map(|holder| holder.model)
            })
        };
        let left_spell_slot = holder_model(left_holder).unwrap_or(player.left_spell_slot);
        let right_spell_slot = holder_model(right_holder).unwrap_or(player.right_spell_slot);
        let caster_mana = fire.map_or(0, |fire| fire.caster.mana);
        McSessionSnapshotV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McSessionSnapshotV2>(),
            generation: self.generation,
            tick: self.state.tick,
            event_head: self.event_head,
            state_hash: state_hash(&self.state),
            conformance_hash: conformance_hash_v2(&self.state, &self.context),
            entity_index: self.state.entity_index,
            x: self.state.position.x,
            y: self.state.position.y,
            z: self.state.position.z,
            yaw: self.state.yaw,
            display_pitch: self.state.display_pitch,
            effective_pitch: self.state.effective_pitch,
            roll_accumulator: self.state.roll_accumulator,
            target_speed: self.state.target_speed,
            actual_speed: self.state.actual_speed,
            strafe_speed: self.state.strafe_speed,
            phase: self.state.entity_phase,
            last_resolution: self.last_resolution,
            profile: profile_code(self.world.profile().cave_mode),
            local_player_slot: self.world.runtime().local_player_slot,
            left_spell_slot,
            right_spell_slot,
            selected_spell_ui_index: selected,
            objective_textbox_ticks: self.world.runtime().objective_textbox_ticks,
            objective_count: u8::try_from(self.world.runtime().objectives.len())
                .expect("validated objective count fits u8"),
            objectives_authority: MC_AUTHORITY_FROZEN_OBSERVED,
            reserved_1: [0; 2],
            left_spell_holder: left_holder,
            right_spell_holder: right_holder,
            selected_spell_holder: selected_holder,
            reserved_0: 0,
            caster_mana,
            maptype_music: self.world.runtime().maptype_music,
            fire_sequence: self.fire_sequence,
            reserved: [0; 4],
        }
    }

    fn append_event(&mut self, event: McEventV2) {
        if self.events.len() == EVENT_CAPACITY
            && let Some(expired) = self.events.pop_front()
        {
            // Audio sidecars have an explicit client release lifecycle. They
            // may outlive the bounded event journal while paused or while a
            // client drains a multi-step backlog.
            let _ = expired;
        }
        self.events.push_back(event);
    }

    fn next_event_base(&mut self, kind: u32, state_hash: u64) -> McEventV2 {
        self.event_head = next_nonzero(self.event_head);
        McEventV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McEventV2>(),
            generation: self.generation,
            sequence: self.event_head,
            state_hash,
            kind,
            code: 0,
            entity_index: 0,
            player_index: -1,
            value_0: 0,
            value_1: 0,
            asset_ref_index: MC_ASSET_REF_NONE,
            reserved: [0; 4],
        }
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

    fn step(&mut self, command: PlayerCommand) {
        #[cfg(test)]
        self.immediate_released_holder_handoff.clear();
        if self.game_paused {
            self.mission.begin_paused_outer_presentation();
            // `UpdateEntities_57730` stores its global-RNG draw before testing
            // GAME_PAUSED. Preserve that source-visible prefix while keeping
            // every post-gate entity action and unrelated turn clock frozen.
            self.mission.advance_paused_update_rng();
            self.mission.finish_paused_outer_presentation(&self.state);
            let state_hash = state_hash(&self.state);
            for event in self.mission.events().to_vec() {
                let record = self.encode_mission_event(event, state_hash);
                self.append_event(record);
            }
            // `PlayEntitySounds_6E150` is gated while the selected
            // single-player/no-CD profile is paused (`byte_38591 == 0`).
            // Keep prepared rows unresolved so the first unpaused flush
            // coalesces them with that frame's rows using the source pool.
            return;
        }
        if let Some((_, active, state, timer)) = self.notification_draw_feedback.take() {
            self.mission.player_notification_active = active;
            self.mission.player_notification_state = state;
            self.mission.player_notification_timer = timer;
            if !active {
                self.mission.player_notification_id = 0;
            }
        }
        self.mission.finish_hud_draw(self.state.tick);
        if let (Some(frames), Some(bank)) =
            (self.selector_frames.as_mut(), self.world.goat_tmap_bank())
        {
            frames.service_unpaused_outer_turn(bank);
        }
        if let (Some(frames), Some(assets)) = (
            self.archer_selector_frames.as_mut(),
            self.terrain_frame_assets.as_ref(),
        ) {
            frames.service_unpaused_outer_turn(&assets.archer_tmap_bank);
        }
        if let (Some(frames), Some(assets)) = (
            self.firefly_selector_frames.as_mut(),
            self.terrain_frame_assets.as_ref(),
        ) {
            frames.service_unpaused_outer_turn(&assets.firefly_tmap_bank);
        }
        if let (Some(frames), Some(assets)) = (
            self.villager_selector_frames.as_mut(),
            self.terrain_frame_assets.as_ref(),
        ) {
            // remc2's C++ transcription omits the clear-store at the common
            // texture clock service. We retain the already accepted
            // executable-order clear-before-advance policy used by Goat and
            // Archer clocks.
            frames.service_unpaused_outer_turn(&assets.frozen_actor_tmap_bank);
        }
        if let (Some(frame), Some(assets)) = (
            self.fireball_selector_frame.as_mut(),
            self.terrain_frame_assets.as_ref(),
        ) {
            frame.service_unpaused_outer_turn(&assets.combat_tmap_bank);
        }
        if let (Some(frame), Some(assets)) = (
            self.possession_selector_frame.as_mut(),
            self.terrain_frame_assets.as_ref(),
        ) {
            frame.service_unpaused_outer_turn(&assets.combat_tmap_bank);
        }
        if let (Some(frame), Some(assets)) = (
            self.castle_selector_frame.as_mut(),
            self.terrain_frame_assets.as_ref(),
        ) {
            frame.service_unpaused_outer_turn(&assets.economy_tmap_bank);
        }
        self.mission.advance_exit_palette_frame();
        self.source_turn = self.source_turn.wrapping_add(1);
        self.game_timer_turn = self.game_timer_turn.wrapping_add(1);
        if !self.mission.exit_sequence_owns_player_action()
            && self.mission.player_action == sim_core::level1_mission::PLAYER_ACTION_ACTIVE
        {
            if command.opcode == 8 {
                self.camera_fov = self.camera_fov.wrapping_add(i16::from(command.argument_1));
            }
            self.camera_look_yaw = command.argument_6.cast_signed();
            self.camera_look_pitch = command.argument_8.cast_signed();
        }
        // `PlayerEvents_51BB0` stores this camera record after applying the
        // input/view fields but before `UpdateEntities_57730` advances flight.
        self.camera_history = CameraHistory::capture(
            &self.state,
            self.source_turn,
            self.camera_fov,
            self.camera_look_yaw,
            self.camera_look_pitch,
        );
        self.camera_slope_sample = self.camera_slope.update(
            self.terrain.ground_heights(),
            self.camera_history.x,
            self.camera_history.y,
        );
        self.mission.sync_collected_spell_holders(&mut self.context);
        self.mission
            .sync_speed_holder_projection(&mut self.state, &mut self.context);
        let holders_before = self
            .context
            .spell_fire
            .as_ref()
            .map_or_else(Vec::new, |fire| fire.holders.clone());
        self.context.events.clear();
        let world = Arc::clone(&self.world);
        let player_control_flags = world
            .entities()
            .get(usize::from(self.state.entity_index))
            .map_or(0, |entity| entity.control_flags);
        let initial_target = [GoatPlayerTarget {
            entity_index: self.state.entity_index,
            position: self.state.position,
            control_flags: player_control_flags,
        }];
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
                )
                .chain(self.live_world.houses().iter().map(house_collision_target)),
        );
        {
            let (_, shading) = self.terrain.dynamic_lighting_planes();
            self.mission
                .subtract_dynamic_lighting_before_updates(shading);
        }
        self.mission.begin_source_turn_at(self.state.position);
        let mut goat_events = self
            .reclaim_imported_source_slots()
            .into_iter()
            .map(|event| (event, self.mission.global_rng()))
            .collect::<Vec<_>>();
        if self.mission.take_group_7_villager_release() {
            let released = self.live_world.release_villager_stage_group_7();
            debug_assert_eq!(released, 28);
        }
        let begin_goat_events = self.live_world.begin_source_turn(&initial_target);
        self.apply_goat_event_side_effects(world.as_ref(), &begin_goat_events);
        goat_events.extend(
            begin_goat_events
                .into_iter()
                .map(|event| (event, self.mission.global_rng())),
        );
        let mut source_views =
            level1_turn_start_source_views(&self.mission, &self.state, &self.live_world);
        let frozen_house_source_views = source_views.clone();
        let mut evidence = Vec::new();
        let mut released_holder_handoff = false;
        for source_slot in 1_u16..1_000 {
            if let Some((owner_entity_index, payload)) =
                self.mission.take_building_possession_mailbox(source_slot)
            {
                self.live_world
                    .submit_house_possession(source_slot, owner_entity_index, payload)
                    .expect("possession mailbox targets a live eligible house");
            }
            let action_51_blocked = self
                .mission
                .current_house_action_51_blocked(&frozen_house_source_views, source_slot);
            let house_events = self.live_world.step_house_source_slot_with_action_51_block(
                source_slot,
                &self.terrain,
                action_51_blocked,
            );
            self.apply_house_event_side_effects(&house_events, &frozen_house_source_views);
            let player_target = [GoatPlayerTarget {
                entity_index: self.state.entity_index,
                position: self.state.position,
                control_flags: player_control_flags,
            }];
            let slot_goat_events =
                self.live_world
                    .step_source_slot(source_slot, &self.terrain, &player_target);
            let villager_effects = self.live_world.step_villager_source_slot(
                source_slot,
                &self.terrain,
                &source_views,
            );
            self.mission
                .apply_imported_villager_effects(&villager_effects);
            self.apply_goat_event_side_effects(world.as_ref(), &slot_goat_events);
            goat_events.extend(
                slot_goat_events
                    .into_iter()
                    .map(|event| (event, self.mission.global_rng())),
            );
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
            refresh_level1_current_source_view(&mut source_views, &self.live_world, source_slot);
            if let Some(dolmen) = world
                .entities()
                .get(usize::from(source_slot))
                .filter(|entity| {
                    entity.active && entity.class == 2 && entity.model == 2 && entity.action == 6
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
                evidence = if self.mission.exit_sequence_owns_player_action() {
                    self.state.tick = self.state.tick.wrapping_add(1);
                    Vec::new()
                } else {
                    match self.mission.player_action {
                        sim_core::level1_mission::PLAYER_ACTION_ACTIVE => {
                            step_outer_turn_with_context(
                                &mut self.state,
                                command,
                                0,
                                world.profile(),
                                &self.terrain,
                                &mut self.context,
                            )
                        }
                        sim_core::level1_mission::PLAYER_ACTION_DYING => {
                            vec![step_entity_body_with_context(
                                &mut self.state,
                                world.profile(),
                                &self.terrain,
                                &mut self.context,
                            )]
                        }
                        _ => Vec::new(),
                    }
                };
                if !self.mission.exit_sequence_owns_player_action() {
                    let flight_proposed = evidence
                        .last()
                        .map_or(self.state.position, |pass| pass.proposed);
                    let lifecycle_action = self.mission.player_action;
                    self.mission
                        .advance_player_lifecycle_after_flight_with_proposed(
                            &self.terrain,
                            &mut self.state,
                            &mut self.context,
                            flight_proposed,
                        );
                    let released_holder_this_pass = lifecycle_action
                        == sim_core::level1_mission::PLAYER_ACTION_DYING
                        && self.mission.player_lifecycle_trace().iter().any(|entry| {
                            entry.step
                                == sim_core::level1_mission::PLAYER_LIFECYCLE_TRACE_HOLDER_FAMILY
                        });
                    released_holder_handoff |= released_holder_this_pass;
                    if released_holder_this_pass {
                        for released in self.mission.released_spell_holders().iter().flatten() {
                            let tile = (
                                released.position.x.to_be_bytes()[0],
                                released.position.y.to_be_bytes()[0],
                            );
                            self.renderable_map_chains
                                .sync_entity(RenderableMapSeed {
                                    entity_index: released.source_entity_index,
                                    active: true,
                                    x: released.position.x,
                                    y: released.position.y,
                                })
                                .expect("qualified immediate spell holder map insertion");
                            debug_assert!(
                                self.renderable_map_chains
                                    .entity_chain(tile.0, tile.1)
                                    .contains(&released.source_entity_index),
                                "released holder must be visible before later source slots dispatch"
                            );
                            #[cfg(test)]
                            self.immediate_released_holder_handoff.push((
                                released.family,
                                released.source_entity_index,
                                tile.0,
                                tile.1,
                            ));
                        }
                    }
                    if let Some(marker) = self
                        .mission
                        .player_death_marker()
                        .filter(|marker| marker.active)
                    {
                        self.renderable_map_chains
                            .sync_entity(RenderableMapSeed {
                                entity_index: marker.source_entity_index,
                                active: true,
                                x: marker.position.x,
                                y: marker.position.y,
                            })
                            .expect("qualified player death marker map insertion");
                    }
                    for burst in self
                        .mission
                        .death_bursts()
                        .iter()
                        .filter(|burst| burst.active)
                    {
                        self.renderable_map_chains
                            .sync_entity(RenderableMapSeed {
                                entity_index: burst.source_entity_index,
                                active: true,
                                x: burst.position.x,
                                y: burst.position.y,
                            })
                            .expect("qualified death burst map insertion");
                    }
                }
                // A multi-pass outer turn clears `accepted_models` when each
                // pass latches the command. Detect the source-visible holder
                // transition instead, matching the direct Rust session.
                let activated_model = |model| {
                    self.context.spell_fire.as_ref().is_some_and(|fire| {
                        fire.holders.iter().any(|holder| {
                            holder.model == model
                                && holder.activation_value != 0
                                && holder.active_value == holder.activation_value.cast_signed()
                                && holders_before
                                    .iter()
                                    .find(|before| before.entity_index == holder.entity_index)
                                    .is_none_or(|before| before.active_value != holder.active_value)
                        })
                    })
                };
                let fireball = activated_model(0);
                let possession = activated_model(1);
                let castle = activated_model(CASTLE_SPELL_FAMILY);
                self.mission
                    .capture_spell_holder_active_values(&self.context);
                (
                    fireball,
                    possession,
                    castle.then_some(self.state.yaw.wrapping_add(self.camera_look_yaw) & 0x07ff),
                )
            } else {
                (false, false, None)
            };
            // remc2 updates a moving entity's terrain-cell link during that
            // entity's source action. Goats/villagers have already acted
            // above, so expose their current chain positions before the Rust
            // mission dispatches the same source slot. Mission-owned rows are
            // synchronized again after their action below.
            self.sync_source_slot_renderable_records(source_slot)
                .expect("validated pre-dispatch source map update");
            let mission_event_start = self.mission.events().len();
            if !self.mission.exit_sequence_owns_player_action()
                && self.mission.player_action == sim_core::level1_mission::PLAYER_ACTION_ACTIVE
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
                self.camera_look_yaw,
                self.camera_look_pitch,
            );
            self.mission
                .apply_pending_player_transients(&mut self.state);
            for seed in apply_level1_source_effects(
                &mut self.mission,
                &mut self.live_world,
                &source_effects,
            )
            .expect("resident source effects apply after slot dispatch")
            {
                self.renderable_map_chains
                    .sync_entity(seed)
                    .expect("resident source effect updates presentation map chain");
            }
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
            self.sync_source_slot_renderable_records(source_slot)
                .expect("validated source renderable map update");
            goat_events.extend(
                self.reclaim_imported_source_slots()
                    .into_iter()
                    .map(|event| (event, self.mission.global_rng())),
            );
        }
        self.live_world.finish_source_turn();
        {
            let (ground_heights, shading) = self.terrain.dynamic_lighting_planes();
            self.mission.service_dynamic_lighting_after_updates(
                &self.state,
                &source_views,
                ground_heights,
                shading,
            );
            self.lighting_receipt = self
                .mission
                .dynamic_lighting_execution_receipt(shading)
                .expect("qualified C ABI dynamic-light execution has fixed shading extent");
        }
        self.mission.finish_source_turn(&self.state);
        self.sync_supported_renderable_records()
            .expect("validated final source renderable map update");
        if released_holder_handoff {
            for released in self.mission.released_spell_holders().iter().flatten() {
                self.renderable_map_chains
                    .sync_entity(RenderableMapSeed {
                        entity_index: released.source_entity_index,
                        active: true,
                        x: released.position.x,
                        y: released.position.y,
                    })
                    .expect("qualified released spell holder map insertion");
                debug_assert!(
                    self.renderable_map_chains
                        .entity_chain(
                            released.position.x.to_be_bytes()[0],
                            released.position.y.to_be_bytes()[0],
                        )
                        .contains(&released.source_entity_index),
                    "final released holder insertion must restore current map membership"
                );
            }
        }
        self.finalize_supported_renderable_records()
            .expect("validated source renderable active set");
        if let Some(fire) = self.context.spell_fire.as_mut() {
            // The class-15 handlers consume one active tick after their
            // same-turn projectile factories: model 0 in `sub_693F0` and
            // model 1 in `sub_69640`. This lets Fireball and Possession return
            // to their inactive, retriggerable state on the source clock.
            for holder in &mut fire.holders {
                if matches!(holder.model, 0 | 1) && holder.active_value > 0 {
                    holder.active_value -= 1;
                }
            }
        }
        self.mission.sync_collected_spell_holders(&mut self.context);
        self.mission
            .sync_speed_holder_projection(&mut self.state, &mut self.context);
        let state_hash = state_hash(&self.state);
        let typed_events = self.context.events.clone();
        for event in typed_events {
            let record = self.encode_flight_event(event, state_hash);
            self.append_event(record);
        }
        let live_world_hash = self.live_world_hash_v2();
        for (event, random_state) in goat_events {
            let record = self.encode_goat_event(event, live_world_hash, random_state);
            self.append_event(record);
        }
        let mission_events = self.mission.events().to_vec();
        for event in mission_events {
            let record = self.encode_mission_event(event, state_hash);
            self.append_event(record);
        }
        self.drain_pending_audio_commands();
        for pass in evidence {
            let mut record = self.next_event_base(MC_EVENT_MOVEMENT_RESOLUTION, pass.state_hash);
            record.code = resolution_code(&pass);
            self.last_resolution = u8::try_from(record.code).unwrap_or(u8::MAX);
            self.append_event(record);
        }
        // The original loop stops music after its first DrawAndEvents pass,
        // then starts Night logical track 1 after the second pass while its
        // local `gameTurn` is 1. Keep that process-loop event distinct from
        // the per-player UpdateMusic envelope command.
        if self.music_start_armed && self.source_turn == 2 {
            let mut record = self.next_event_base(MC_EVENT_START_MUSIC, state_hash);
            record.code = u32::try_from(self.world.runtime().maptype_music)
                .expect("qualified Level 1 music selector is non-negative");
            record.value_0 = 0;
            record.value_1 = 1;
            record.asset_ref_index = self.world.music_asset_ref().unwrap_or(MC_ASSET_REF_NONE);
            self.append_event(record);
            self.music_start_armed = false;
        }
    }

    fn apply_goat_event_side_effects(&mut self, _world: &NativeWorld, events: &[GoatStepEvent]) {
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

    fn apply_house_event_side_effects(
        &mut self,
        events: &[HouseStepEvent],
        frozen_source_views: &[Level1SourceEntityView],
    ) {
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
                    for resident in outcome.residents {
                        self.mission
                            .spawn_house_resident(
                                resident.position,
                                resident.model,
                                resident.action,
                                attacker_entity_index,
                            )
                            .expect("qualified action-53 resident constructor");
                        self.pending_house_effects
                            .push(HouseStepEvent::ResidentRequested {
                                entity_index,
                                attacker_entity_index,
                                position: resident.position,
                                model: resident.model,
                                action: resident.action,
                            });
                    }
                    self.mission.remove_imported_building(entity_index);
                    self.pending_house_effects.push(*event);
                }
                HouseStepEvent::ResidentRequested {
                    attacker_entity_index,
                    position,
                    model,
                    action,
                    ..
                } => {
                    self.mission
                        .spawn_house_resident(position, model, action, attacker_entity_index)
                        .expect("qualified action-52 resident constructor");
                    self.pending_house_effects.push(*event);
                }
                HouseStepEvent::Damaged { .. } | HouseStepEvent::DestructionStarted { .. } => {
                    self.pending_house_effects.push(*event);
                }
            }
        }
    }

    fn reclaim_imported_source_slots(&mut self) -> Vec<GoatStepEvent> {
        self.mission
            .take_reclaimed_imported_source_slots()
            .into_iter()
            .filter_map(|source_slot| self.live_world.reclaim_source_slot(source_slot))
            .collect()
    }

    fn sync_supported_renderable_records(&mut self) -> Result<(), String> {
        for actor in &mut self.frozen_actors {
            if let Some(villager) = self
                .live_world
                .villagers()
                .iter()
                .find(|villager| villager.entity_index == actor.source_entity_index)
            {
                actor.position = villager.position;
                actor.yaw = villager.yaw;
            } else if (130..=132).contains(&actor.source_entity_index) {
                actor.position.z = self.terrain.ground_height(actor.position);
            }
        }
        for goat in self.live_world.goats() {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: goat.entity_index,
                active: goat.active,
                x: goat.position.x,
                y: goat.position.y,
            })?;
        }
        for villager in self.live_world.villagers() {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: villager.entity_index,
                active: villager.active,
                x: villager.position.x,
                y: villager.position.y,
            })?;
        }
        for actor in self.mission.actors().iter().filter(|actor| {
            native_composited_actor_kind(actor.kind) && actor.source_entity_index != 0
        }) {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: actor.source_entity_index,
                active: actor.active,
                x: actor.position.x,
                y: actor.position.y,
            })?;
        }
        for resident in self.mission.dynamic_residents() {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: resident.source_entity_index,
                active: resident.active,
                x: resident.position.x,
                y: resident.position.y,
            })?;
        }
        for segment in self.mission.multipart_segments() {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: segment.source_entity_index,
                active: segment.active,
                x: segment.position.x,
                y: segment.position.y,
            })?;
        }
        for ambient in self.mission.ambient_entities() {
            if ambient.map_linked {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: ambient.source_entity_index,
                    active: ambient.visible(),
                    x: ambient.position.x,
                    y: ambient.position.y,
                })?;
            }
        }
        for projectile in self
            .mission
            .projectiles()
            .iter()
            .filter(|projectile| projectile.source_entity_index() != 0)
        {
            let position = match projectile {
                Level1Projectile::PlayerFireball(value) => value.position,
                Level1Projectile::ArcherArrow(value) => value.position,
                Level1Projectile::FireflyFireball(value) => value.position,
                Level1Projectile::CastleBuild(value) => value.position,
                Level1Projectile::Possession(value) => value.position,
            };
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: projectile.source_entity_index(),
                active: projectile.active(),
                x: position.x,
                y: position.y,
            })?;
        }
        for impact in self
            .mission
            .impacts()
            .iter()
            .filter(|impact| impact.model != POSSESSION_ATTRACTION_MODEL)
        {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: impact.source_entity_index,
                active: impact.active,
                x: impact.position.x,
                y: impact.position.y,
            })?;
        }
        for burst in self
            .mission
            .death_bursts()
            .iter()
            .filter(|burst| burst.active)
        {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: burst.source_entity_index,
                active: true,
                x: burst.position.x,
                y: burst.position.y,
            })?;
        }
        if let Some(marker) = self
            .mission
            .player_death_marker()
            .filter(|marker| marker.active)
        {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: marker.source_entity_index,
                active: true,
                x: marker.position.x,
                y: marker.position.y,
            })?;
        }
        for building in self.mission.possessed_buildings() {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: building.entity_index,
                active: building.active,
                x: building.position.x,
                y: building.position.y,
            })?;
        }
        if self.mission.castle_exists && self.mission.castle_source_entity_index != 0 {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: self.mission.castle_source_entity_index,
                active: true,
                x: self.mission.castle_position.x,
                y: self.mission.castle_position.y,
            })?;
        }
        for sphere in self
            .mission
            .mana_spheres()
            .iter()
            .filter(|sphere| sphere.source_entity_index != 0)
        {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: sphere.source_entity_index,
                active: sphere.active && !sphere.pending_delete,
                x: sphere.position.x,
                y: sphere.position.y,
            })?;
        }
        if let Some(carrier) = self
            .mission
            .mana_carrier()
            .filter(|carrier| carrier.source_entity_index != 0)
        {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: carrier.source_entity_index,
                active: carrier.active,
                x: carrier.position.x,
                y: carrier.position.y,
            })?;
        }
        Ok(())
    }

    fn sync_source_slot_renderable_records(&mut self, source_slot: u16) -> Result<(), String> {
        if self.renderable_map_chains.tracks(source_slot) {
            if source_slot == self.state.entity_index {
                let seed = RenderableMapSeed {
                    entity_index: source_slot,
                    active: true,
                    x: self.state.position.x,
                    y: self.state.position.y,
                };
                self.mission.sync_source_map_entity(seed)?;
                self.renderable_map_chains.sync_entity(seed)?;
            } else if let Some(goat) = self
                .live_world
                .goats()
                .iter()
                .find(|goat| goat.entity_index == source_slot)
            {
                let seed = RenderableMapSeed {
                    entity_index: goat.entity_index,
                    active: goat.active,
                    x: goat.position.x,
                    y: goat.position.y,
                };
                self.mission.sync_source_map_entity(seed)?;
                self.renderable_map_chains.sync_entity(seed)?;
            } else if let Some(villager) = self
                .live_world
                .villagers()
                .iter()
                .find(|villager| villager.entity_index == source_slot)
            {
                if let Some(actor) = self
                    .frozen_actors
                    .iter_mut()
                    .find(|actor| actor.source_entity_index == source_slot)
                {
                    actor.position = villager.position;
                    actor.yaw = villager.yaw;
                }
                let seed = RenderableMapSeed {
                    entity_index: villager.entity_index,
                    active: villager.active,
                    x: villager.position.x,
                    y: villager.position.y,
                };
                self.mission.sync_source_map_entity(seed)?;
                self.renderable_map_chains.sync_entity(seed)?;
            } else if let Some(actor) = self.mission.actors().iter().find(|actor| {
                native_composited_actor_kind(actor.kind) && actor.source_entity_index == source_slot
            }) {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: actor.source_entity_index,
                    active: actor.active,
                    x: actor.position.x,
                    y: actor.position.y,
                })?;
            } else if let Some(resident) = self
                .mission
                .dynamic_residents()
                .find(|resident| resident.source_entity_index == source_slot)
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: resident.source_entity_index,
                    active: resident.active,
                    x: resident.position.x,
                    y: resident.position.y,
                })?;
            } else if let Some(segment) = self
                .mission
                .multipart_segments()
                .iter()
                .find(|segment| segment.source_entity_index == source_slot)
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: segment.source_entity_index,
                    active: segment.active,
                    x: segment.position.x,
                    y: segment.position.y,
                })?;
            } else if let Some(ambient) = self
                .mission
                .ambient_entities()
                .find(|ambient| ambient.source_entity_index == source_slot)
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: ambient.source_entity_index,
                    active: ambient.visible(),
                    x: ambient.position.x,
                    y: ambient.position.y,
                })?;
            } else if let Some(building) = self
                .mission
                .possessed_buildings()
                .iter()
                .find(|building| building.entity_index == source_slot)
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: building.entity_index,
                    active: building.active,
                    x: building.position.x,
                    y: building.position.y,
                })?;
            } else if self.mission.castle_exists
                && self.mission.castle_source_entity_index == source_slot
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: true,
                    x: self.mission.castle_position.x,
                    y: self.mission.castle_position.y,
                })?;
            } else if let Some(sphere) = self
                .mission
                .mana_spheres()
                .iter()
                .find(|sphere| sphere.source_entity_index == source_slot)
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: sphere.active && !sphere.pending_delete,
                    x: sphere.position.x,
                    y: sphere.position.y,
                })?;
            } else if let Some(carrier) = self
                .mission
                .mana_carrier()
                .filter(|carrier| carrier.source_entity_index == source_slot)
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: carrier.active,
                    x: carrier.position.x,
                    y: carrier.position.y,
                })?;
            }
        }
        // Replay each constructor's AddEventToMap order. Ordinary actors map
        // immediately after allocation. The multipart constructor is the
        // exception: it maps segment 1..16 first and its already-allocated
        // head last, yielding head -> segment16 -> ... -> segment1.
        let mut new_actors = self
            .mission
            .actors()
            .iter()
            .filter(|actor| {
                native_composited_actor_kind(actor.kind)
                    && actor.source_entity_index != 0
                    && !self.renderable_map_chains.tracks(actor.source_entity_index)
            })
            .collect::<Vec<_>>();
        new_actors.sort_unstable_by_key(|actor| actor.source_entity_index);
        let mut newly_tracked = Vec::new();
        for actor in new_actors {
            if actor.kind == Level1ActorKind::MultipartHead {
                let mut segments = self
                    .mission
                    .multipart_segments()
                    .iter()
                    .filter(|segment| {
                        segment.head_source_entity_index == actor.source_entity_index
                            && !self
                                .renderable_map_chains
                                .tracks(segment.source_entity_index)
                    })
                    .collect::<Vec<_>>();
                segments.sort_unstable_by_key(|segment| segment.ordinal);
                newly_tracked.extend(segments.into_iter().map(|segment| RenderableMapSeed {
                    entity_index: segment.source_entity_index,
                    active: segment.active,
                    x: segment.position.x,
                    y: segment.position.y,
                }));
            }
            newly_tracked.push(RenderableMapSeed {
                entity_index: actor.source_entity_index,
                active: actor.active,
                x: actor.position.x,
                y: actor.position.y,
            });
        }
        // Preserve partial-construction prefixes if the source arena filled
        // before all children could be allocated.
        let orphan_segments = self
            .mission
            .multipart_segments()
            .iter()
            .filter(|segment| {
                !self
                    .renderable_map_chains
                    .tracks(segment.source_entity_index)
                    && !newly_tracked
                        .iter()
                        .any(|seed| seed.entity_index == segment.source_entity_index)
            })
            .map(|segment| RenderableMapSeed {
                entity_index: segment.source_entity_index,
                active: segment.active,
                x: segment.position.x,
                y: segment.position.y,
            })
            .collect::<Vec<_>>();
        newly_tracked.extend(orphan_segments);
        for seed in newly_tracked {
            self.renderable_map_chains.sync_entity(seed)?;
        }
        let newly_tracked_residents = self
            .mission
            .dynamic_residents()
            .filter(|resident| {
                resident.source_entity_index != 0
                    && !self
                        .renderable_map_chains
                        .tracks(resident.source_entity_index)
            })
            .map(|resident| RenderableMapSeed {
                entity_index: resident.source_entity_index,
                active: resident.active,
                x: resident.position.x,
                y: resident.position.y,
            })
            .collect::<Vec<_>>();
        for seed in newly_tracked_residents {
            self.renderable_map_chains.sync_entity(seed)?;
        }
        let newly_tracked_ambient = self
            .mission
            .ambient_entities()
            .filter(|ambient| {
                ambient.map_linked
                    && !self
                        .renderable_map_chains
                        .tracks(ambient.source_entity_index)
            })
            .map(|ambient| RenderableMapSeed {
                entity_index: ambient.source_entity_index,
                active: ambient.visible(),
                x: ambient.position.x,
                y: ambient.position.y,
            })
            .collect::<Vec<_>>();
        for seed in newly_tracked_ambient {
            self.renderable_map_chains.sync_entity(seed)?;
        }
        if self.mission.castle_exists
            && self.mission.castle_source_entity_index != 0
            && !self
                .renderable_map_chains
                .tracks(self.mission.castle_source_entity_index)
        {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: self.mission.castle_source_entity_index,
                active: true,
                x: self.mission.castle_position.x,
                y: self.mission.castle_position.y,
            })?;
        }
        let newly_tracked_spheres = self
            .mission
            .mana_spheres()
            .iter()
            .filter(|sphere| {
                sphere.source_entity_index != 0
                    && !self
                        .renderable_map_chains
                        .tracks(sphere.source_entity_index)
            })
            .map(|sphere| RenderableMapSeed {
                entity_index: sphere.source_entity_index,
                active: sphere.active,
                x: sphere.position.x,
                y: sphere.position.y,
            })
            .collect::<Vec<_>>();
        for seed in newly_tracked_spheres {
            self.renderable_map_chains.sync_entity(seed)?;
        }
        if let Some(carrier) = self.mission.mana_carrier().filter(|carrier| {
            carrier.source_entity_index != 0
                && !self
                    .renderable_map_chains
                    .tracks(carrier.source_entity_index)
        }) {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: carrier.source_entity_index,
                active: carrier.active,
                x: carrier.position.x,
                y: carrier.position.y,
            })?;
        }
        Ok(())
    }

    fn finalize_supported_renderable_records(&mut self) -> Result<(), String> {
        let mut active = self
            .world
            .entities()
            .iter()
            .filter(|entity| {
                entity.active
                    && entity.control_flags & 8 != 0
                    && self.mission.imported_source_slot_is_live(entity.index)
            })
            .map(|entity| entity.index)
            .collect::<Vec<_>>();
        active.extend(
            self.live_world
                .goats()
                .iter()
                .filter(|goat| goat.active)
                .map(|goat| goat.entity_index),
        );
        active.extend(
            self.live_world
                .villagers()
                .iter()
                .filter(|villager| villager.active)
                .map(|villager| villager.entity_index),
        );
        active.extend(
            self.frozen_actors
                .iter()
                .filter(|actor| {
                    actor.source_entity_index >= 130
                        && self
                            .mission
                            .imported_source_slot_is_live(actor.source_entity_index)
                })
                .map(|actor| actor.source_entity_index),
        );
        active.extend(
            self.mission
                .actors()
                .iter()
                .filter(|actor| {
                    native_composited_actor_kind(actor.kind)
                        && actor.source_entity_index != 0
                        && actor.active
                })
                .map(|actor| actor.source_entity_index),
        );
        active.extend(
            self.mission
                .dynamic_residents()
                .filter(|resident| resident.active)
                .map(|resident| resident.source_entity_index),
        );
        active.extend(
            self.mission
                .multipart_segments()
                .iter()
                .filter(|segment| segment.active)
                .map(|segment| segment.source_entity_index),
        );
        active.extend(
            self.mission
                .ambient_entities()
                .filter(|ambient| ambient.visible())
                .map(|ambient| ambient.source_entity_index),
        );
        active.extend(
            self.mission
                .projectiles()
                .iter()
                .filter(|projectile| projectile.active())
                .map(|projectile| projectile.source_entity_index()),
        );
        active.extend(
            self.mission
                .impacts()
                .iter()
                .filter(|impact| impact.active && impact.model != POSSESSION_ATTRACTION_MODEL)
                .map(|impact| impact.source_entity_index),
        );
        active.extend(
            self.mission
                .death_bursts()
                .iter()
                .filter(|burst| burst.active)
                .map(|burst| burst.source_entity_index),
        );
        active.extend(
            self.mission
                .released_spell_holders()
                .iter()
                .flatten()
                .map(|holder| holder.source_entity_index),
        );
        if let Some(marker) = self
            .mission
            .player_death_marker()
            .filter(|marker| marker.active)
        {
            active.push(marker.source_entity_index);
        }
        active.extend(
            self.mission
                .possessed_buildings()
                .iter()
                .filter(|building| building.active)
                .map(|building| building.entity_index),
        );
        if self.mission.castle_exists && self.mission.castle_source_entity_index != 0 {
            active.push(self.mission.castle_source_entity_index);
        }
        active.extend(
            self.mission
                .mana_spheres()
                .iter()
                .filter(|sphere| {
                    sphere.active && !sphere.pending_delete && sphere.source_entity_index != 0
                })
                .map(|sphere| sphere.source_entity_index),
        );
        if let Some(carrier) = self
            .mission
            .mana_carrier()
            .filter(|carrier| carrier.active && carrier.source_entity_index != 0)
        {
            active.push(carrier.source_entity_index);
        }
        // Hidden class-10/model-45 houses remain live collision rows even
        // before Possession reveals their coloured marker. Keeping them in
        // the filtered chain preserves their source painter/collision order.
        active.extend(
            self.mission
                .possession_collision_targets()
                .iter()
                .filter(|target| target.active)
                .map(|target| target.entity_index),
        );
        active.sort_unstable();
        active.dedup();
        self.renderable_map_chains.retain_entities(&active)
    }

    fn inject_new_fireball_impacts(&mut self, event_start: usize) {
        let impacts = self.mission.events()[event_start..]
            .iter()
            .filter_map(|event| {
                let Level1MissionEvent::FireballImpacted {
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
                            || sim_core::level1_mission::source_entities_overlap(
                                position,
                                sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                                sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_VERTICAL_OFFSET,
                                sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_HEIGHT,
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
                                    || sim_core::level1_mission::source_entities_overlap(
                                        position,
                                        sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                                        sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_VERTICAL_OFFSET,
                                        sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_HEIGHT,
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
                                    || sim_core::level1_mission::source_entities_overlap_xy(
                                        position,
                                        sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                                        sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
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

    fn drain_pending_audio_commands(&mut self) {
        let unresolved = self
            .audio_commands
            .iter()
            .filter_map(|(sequence, command)| {
                (command.action == 0).then_some((*sequence, *command))
            })
            .collect::<Vec<_>>();
        let pooled_flush_id = unresolved
            .iter()
            .filter(|(_, command)| !matches!(command.sound_id, 1 | 2 | 5 | 31))
            .map(|(sequence, _)| *sequence)
            .min()
            .unwrap_or(0);
        let mut pending = BTreeMap::<u16, u64>::new();
        for (sequence, command) in unresolved {
            if matches!(command.sound_id, 1 | 2 | 5 | 31) {
                let mut command = command;
                let (action, voice_index) = self.allocate_audio_voice(command);
                command.action = if action == MC_AUDIO_ACTION_SUPPRESS_EXISTING {
                    MC_AUDIO_ACTION_UPDATE_EXISTING
                } else {
                    action
                };
                command.voice_index = voice_index;
                command.pooled_flush_rank = 0;
                self.audio_commands.insert(sequence, command);
                continue;
            }
            if let Some(current_sequence) = pending.get(&command.sound_id).copied() {
                let current = self.audio_commands[&current_sequence];
                if positional_sound_should_replace(
                    i32::from(command.volume_q15),
                    i32::from(current.volume_q15),
                ) {
                    self.audio_commands
                        .get_mut(&current_sequence)
                        .expect("pending command remains present")
                        .action = MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED;
                    pending.insert(command.sound_id, sequence);
                } else {
                    self.audio_commands
                        .get_mut(&sequence)
                        .expect("new command remains present")
                        .action = MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED;
                }
            } else {
                pending.insert(command.sound_id, sequence);
            }
        }
        let mut pooled_flush_rank = 1_u8;
        for sequence in pending.into_values() {
            let mut command = self.audio_commands[&sequence];
            let (action, voice_index) = self.allocate_audio_voice(command);
            command.action = action;
            command.voice_index = voice_index;
            if matches!(
                command.action,
                MC_AUDIO_ACTION_START | MC_AUDIO_ACTION_RESTART | MC_AUDIO_ACTION_UPDATE_EXISTING
            ) {
                command.pooled_flush_rank = pooled_flush_rank;
                command.reserved[0] = pooled_flush_id as u32;
                command.reserved[1] = (pooled_flush_id >> 32) as u32;
                pooled_flush_rank = pooled_flush_rank
                    .checked_add(1)
                    .expect("ordinary sound pool has at most 70 winners");
            } else {
                command.pooled_flush_rank = 0;
            }
            self.audio_commands.insert(sequence, command);
        }
    }

    fn insert_prepared_audio_command(&mut self, sequence: u64, mut command: McAudioCommandV2) {
        if self.game_paused
            && command.action == 0
            && !matches!(command.sound_id, 1 | 2 | 5 | 31)
            && let Some(current_sequence) =
                self.audio_commands
                    .iter()
                    .find_map(|(candidate_sequence, candidate)| {
                        (candidate.action == 0 && candidate.sound_id == command.sound_id)
                            .then_some(*candidate_sequence)
                    })
        {
            let current = self.audio_commands[&current_sequence];
            if positional_sound_should_replace(
                i32::from(command.volume_q15),
                i32::from(current.volume_q15),
            ) {
                self.audio_commands
                    .get_mut(&current_sequence)
                    .expect("paused pending command remains present")
                    .action = MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED;
            } else {
                command.action = MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED;
            }
        }
        self.audio_commands.insert(sequence, command);
    }

    fn allocate_audio_voice(&mut self, command: McAudioCommandV2) -> (u8, u16) {
        let matching = self.audio_voices.iter().position(|voice| {
            voice.is_some_and(|voice| {
                voice.sound_id == command.sound_id && voice.sound_flags == command.sound_flags
            })
        });
        // `play_type` is the queued `EntitySounds.playType_0` value for
        // PrepareEventSound events (1 = restart-or-start, 3 = if-not-playing).
        // The four direct ambience loops retain the lower PlaySample enum
        // value 2 for if-not-playing.
        let is_direct_ambience_if_not_playing = matches!(command.sound_id, 1 | 2 | 5 | 31)
            && command.play_type == POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING;
        if (command.play_type == 3 || is_direct_ambience_if_not_playing)
            && let Some(index) = matching
        {
            return (
                MC_AUDIO_ACTION_SUPPRESS_EXISTING,
                u16::try_from(index).expect("ten voices fit u16"),
            );
        }
        if command.play_type == 1
            && let Some(index) = matching
        {
            self.audio_voices[index] = Some(AudioVoice {
                sound_id: command.sound_id,
                sound_flags: command.sound_flags,
                event_sequence: command.event_sequence,
            });
            return (
                MC_AUDIO_ACTION_RESTART,
                u16::try_from(index).expect("ten voices fit u16"),
            );
        }
        let Some(index) = self.audio_voices.iter().position(Option::is_none) else {
            return (MC_AUDIO_ACTION_SUPPRESS_NO_VOICE, MC_AUDIO_VOICE_NONE);
        };
        self.audio_voices[index] = Some(AudioVoice {
            sound_id: command.sound_id,
            sound_flags: command.sound_flags,
            event_sequence: command.event_sequence,
        });
        (
            MC_AUDIO_ACTION_START,
            u16::try_from(index).expect("ten voices fit u16"),
        )
    }

    fn encode_flight_event(&mut self, event: FlightEvent, hash: u64) -> McEventV2 {
        match event {
            FlightEvent::PrepareSound {
                entity_index,
                player_index,
                sound_id,
            } => {
                let mut record = self.next_event_base(MC_EVENT_PREPARE_SOUND, hash);
                record.entity_index = entity_index;
                record.player_index = player_index;
                record.code = u32::from(sound_id);
                record.asset_ref_index = self
                    .world
                    .sound_asset_ref(sound_id)
                    .unwrap_or(MC_ASSET_REF_NONE);
                if let Some(command) = self.prepare_audio_command(&record) {
                    self.insert_prepared_audio_command(record.sequence, command);
                }
                record
            }
            FlightEvent::EndLoop {
                entity_index,
                player_index,
                sound_id,
            } => {
                let mut record = self.next_event_base(MC_EVENT_END_LOOP, hash);
                record.entity_index = entity_index;
                record.player_index = player_index;
                record.code = u32::from(sound_id);
                record.asset_ref_index = self
                    .world
                    .sound_asset_ref(sound_id)
                    .unwrap_or(MC_ASSET_REF_NONE);
                if matches!(sound_id, 1 | 2 | 5 | 31) {
                    let matching = self.audio_voices.iter().position(|voice| {
                        voice.is_some_and(|voice| {
                            voice.sound_id == sound_id && voice.sound_flags == 0
                        })
                    });
                    let mut command = local_audio_command_record(
                        &record,
                        sound_id,
                        0,
                        POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING,
                    );
                    if let Some(index) = matching {
                        command.action = MC_AUDIO_ACTION_END_LOOP;
                        command.voice_index = u16::try_from(index).expect("ten voices fit u16");
                    } else {
                        command.action = MC_AUDIO_ACTION_SUPPRESS_NO_EXISTING;
                    }
                    self.audio_commands.insert(record.sequence, command);
                }
                record
            }
            FlightEvent::UpdateMusic {
                mode,
                timer_multiplier,
            } => {
                let mut record = self.next_event_base(MC_EVENT_UPDATE_MUSIC, hash);
                record.code = u32::from(mode);
                record.value_0 = i32::from(timer_multiplier);
                record.value_1 = self.source_turn;
                record.asset_ref_index = self.world.music_asset_ref().unwrap_or(MC_ASSET_REF_NONE);
                record
            }
            FlightEvent::RequestSpellActivation {
                source,
                holder_entity_index,
                direction_mask,
            } => {
                self.fire_sequence = self.fire_sequence.wrapping_add(1);
                let mut record = self.next_event_base(MC_EVENT_REQUEST_SPELL_ACTIVATION, hash);
                record.entity_index = holder_entity_index;
                record.code = match source {
                    sim_core::SpellFireSource::Left => 0,
                    sim_core::SpellFireSource::Right => 1,
                    sim_core::SpellFireSource::Selected => 2,
                };
                record.value_0 = i32::from(direction_mask);
                record
            }
            FlightEvent::ShowHelpHint { hint_index, state } => {
                let mut record = self.next_event_base(MC_EVENT_SHOW_HELP_HINT, hash);
                record.code = u32::from(hint_index);
                record.value_0 = i32::from(state);
                record
            }
        }
    }

    fn encode_goat_event(
        &mut self,
        event: GoatStepEvent,
        live_hash: u64,
        random_state: u32,
    ) -> McEventV2 {
        match event {
            GoatStepEvent::Bleat {
                entity_index,
                sound_id,
                position,
                action,
            } => {
                let mut record = self.next_event_base(MC_EVENT_GOAT_SOUND_REQUEST, live_hash);
                record.entity_index = entity_index;
                record.code = u32::from(sound_id);
                record.asset_ref_index = self
                    .world
                    .sound_asset_ref(sound_id)
                    .unwrap_or(MC_ASSET_REF_NONE);
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    entity_index,
                    position,
                    action,
                    random_state,
                );
                record
            }
            GoatStepEvent::ManaConverted { entity_index, mana } => {
                let mut record = self.next_event_base(MC_EVENT_GOAT_MANA_CONVERTED, live_hash);
                record.entity_index = entity_index;
                record.value_0 = mana;
                record
            }
            GoatStepEvent::ExtraManaSphereRequested { entity_index } => {
                let mut record =
                    self.next_event_base(MC_EVENT_GOAT_EXTRA_MANA_SPHERE_REQUEST, live_hash);
                record.entity_index = entity_index;
                record
            }
            GoatStepEvent::RemovalQueued { entity_index } => {
                let mut record = self.next_event_base(MC_EVENT_GOAT_REMOVAL_QUEUED, live_hash);
                record.entity_index = entity_index;
                record
            }
            GoatStepEvent::Reclaimed { entity_index } => {
                let mut record = self.next_event_base(MC_EVENT_GOAT_RECLAIMED, live_hash);
                record.entity_index = entity_index;
                record
            }
            GoatStepEvent::EngagementBoundaryReached {
                entity_index,
                target_entity_index,
            } => {
                let mut record = self.next_event_base(MC_EVENT_GOAT_ENGAGEMENT_BOUNDARY, live_hash);
                record.entity_index = entity_index;
                record.value_0 = i32::from(target_entity_index);
                record
            }
        }
    }

    fn encode_mission_event(&mut self, event: Level1MissionEvent, hash: u64) -> McEventV2 {
        match event {
            Level1MissionEvent::ObjectiveAdvanced {
                completed_stage,
                text_selector,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_OBJECTIVE_ADVANCED, hash);
                record.code = u32::from(text_selector);
                record.value_0 = i32::from(completed_stage);
                record
            }
            Level1MissionEvent::AbilityImproved {
                source_family,
                notification_template_selector,
                ability_name_selector,
                notification_state,
                notification_timer,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_ABILITY_IMPROVED, hash);
                record.code = u32::from(source_family);
                record.entity_index = ability_name_selector;
                record.value_0 = i32::from(notification_state);
                record.value_1 = i32::from(notification_timer);
                record.reserved[0] = u32::from(notification_template_selector);
                record
            }
            Level1MissionEvent::SpellCollected {
                family,
                authored_index,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_SPELL_COLLECTED, hash);
                record.code = u32::from(family);
                record.entity_index = authored_index;
                record
            }
            Level1MissionEvent::FireballSpawned { projectile_id } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_FIREBALL_SPAWNED, hash);
                record.code = projectile_id;
                record
            }
            Level1MissionEvent::PossessionSpawned { projectile_id } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_POSSESSION_SPAWNED, hash);
                record.code = projectile_id;
                record
            }
            Level1MissionEvent::ManaSphereSpawned {
                sphere_id,
                source_entity_index,
                mana,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_MANA_SPHERE_SPAWNED, hash);
                record.code = sphere_id;
                record.entity_index = source_entity_index;
                record.value_0 = mana;
                record
            }
            Level1MissionEvent::ManaSphereClaimed {
                sphere_id,
                owner_entity_index,
                sound_id,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_MANA_SPHERE_CLAIMED, hash);
                record.code = u32::from(sound_id);
                record.entity_index = owner_entity_index;
                record.value_0 = i32::try_from(sphere_id).unwrap_or(i32::MAX);
                let sphere = self
                    .mission
                    .mana_spheres()
                    .iter()
                    .find(|sphere| sphere.id == sphere_id)
                    .copied();
                let position = if owner_entity_index == self.state.entity_index {
                    self.state.position
                } else {
                    self.mission
                        .actors()
                        .iter()
                        .find(|actor| actor.source_entity_index == owner_entity_index)
                        .map(|actor| actor.position)
                        .or_else(|| {
                            self.live_world
                                .goats()
                                .iter()
                                .find(|goat| goat.entity_index == owner_entity_index)
                                .map(|goat| goat.position)
                        })
                        .or_else(|| sphere.map(|sphere| sphere.position))
                        .unwrap_or(self.state.position)
                };
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    owner_entity_index,
                    position,
                    10,
                    self.mission.global_rng(),
                );
                record
            }
            Level1MissionEvent::ManaSphereCollected {
                sphere_id,
                carrier_id,
                mana,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_MANA_SPHERE_COLLECTED, hash);
                record.code = carrier_id;
                record.value_0 = i32::try_from(sphere_id).unwrap_or(i32::MAX);
                record.value_1 = mana;
                record
            }
            Level1MissionEvent::ArcherArrowSpawned {
                projectile_id,
                archer_authored_index,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_ARCHER_ARROW_SPAWNED, hash);
                record.code = projectile_id;
                record.entity_index = archer_authored_index;
                record
            }
            Level1MissionEvent::ArcherArrowSound {
                projectile_id,
                sound_id,
                source_entity_index,
                position,
                random_state,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_ARCHER_ARROW_SOUND, hash);
                record.code = u32::from(sound_id);
                record.value_0 = i32::try_from(projectile_id).unwrap_or(i32::MAX);
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    source_entity_index,
                    position,
                    13,
                    random_state,
                );
                record
            }
            Level1MissionEvent::FireflyFireballSpawned {
                projectile_id,
                firefly_authored_index,
            } => {
                let mut record =
                    self.next_event_base(MC_EVENT_LEVEL1_FIREFLY_FIREBALL_SPAWNED, hash);
                record.code = projectile_id;
                record.entity_index = firefly_authored_index;
                record
            }
            Level1MissionEvent::FireflyChargeSound {
                firefly_authored_index,
                sound_id,
                random_state,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_FIREFLY_CHARGE_SOUND, hash);
                record.code = u32::from(sound_id);
                record.entity_index = firefly_authored_index;
                if let Some(actor) = self
                    .mission
                    .actors()
                    .iter()
                    .find(|actor| actor.authored_index == firefly_authored_index)
                    .copied()
                {
                    self.attach_mission_sound(
                        &mut record,
                        sound_id,
                        actor.source_entity_index,
                        actor.position,
                        actor.action,
                        random_state,
                    );
                }
                record
            }
            Level1MissionEvent::FireballImpacted {
                projectile_id,
                impact_id,
                position,
                sound_id,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_FIREBALL_IMPACTED, hash);
                record.code = impact_id;
                record.value_0 = i32::try_from(projectile_id).unwrap_or(i32::MAX);
                record.value_1 = i32::from(position.z);
                let source_entity_index = self
                    .mission
                    .impacts()
                    .iter()
                    .find(|impact| impact.id == impact_id)
                    .map_or(0, |impact| impact.source_entity_index);
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    source_entity_index,
                    position,
                    0,
                    self.mission.global_rng(),
                );
                record
            }
            Level1MissionEvent::WaterSplashed {
                projectile_id,
                splash_id,
                position,
                sound_id,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_WATER_SPLASHED, hash);
                record.code = u32::from(sound_id);
                record.value_0 = i32::try_from(projectile_id).unwrap_or(i32::MAX);
                record.value_1 = i32::from(position.z);
                record.entity_index = u16::try_from(splash_id).unwrap_or(u16::MAX);
                let source_entity_index = self
                    .mission
                    .impacts()
                    .iter()
                    .find(|impact| impact.id == splash_id)
                    .map_or(0, |impact| impact.source_entity_index);
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    source_entity_index,
                    position,
                    5,
                    self.mission.global_rng(),
                );
                record
            }
            Level1MissionEvent::ActorDamaged {
                authored_index,
                attacker_entity_index,
                damage,
                life,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_ACTOR_DAMAGED, hash);
                record.entity_index = authored_index;
                record.code = u32::from(attacker_entity_index);
                record.value_0 = damage;
                record.value_1 = life;
                record
            }
            Level1MissionEvent::ActorKilled {
                authored_index,
                kind,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_ACTOR_KILLED, hash);
                record.entity_index = authored_index;
                record.code = u32::from(kind as u8);
                record
            }
            Level1MissionEvent::CastleBuilt { level } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_CASTLE_BUILT, hash);
                record.code = u32::from(level);
                record
            }
            Level1MissionEvent::CastleProjectileSpawned {
                projectile_id,
                sound_id,
                source_entity_index,
                position,
                random_state,
            } => {
                let mut record =
                    self.next_event_base(MC_EVENT_LEVEL1_CASTLE_PROJECTILE_SPAWNED, hash);
                record.code = u32::from(sound_id);
                record.value_0 = i32::try_from(projectile_id).unwrap_or(i32::MAX);
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    source_entity_index,
                    position,
                    20,
                    random_state,
                );
                record
            }
            Level1MissionEvent::CastlePlacementRejected { projectile_id } => {
                let mut record =
                    self.next_event_base(MC_EVENT_LEVEL1_CASTLE_PLACEMENT_REJECTED, hash);
                record.code = projectile_id;
                record
            }
            Level1MissionEvent::CastleConstructionSound {
                sound_id,
                source_entity_index,
                position,
            } => {
                let mut record =
                    self.next_event_base(MC_EVENT_LEVEL1_CASTLE_CONSTRUCTION_SOUND, hash);
                record.code = u32::from(sound_id);
                record.entity_index = source_entity_index;
                record.value_0 = i32::from(position.x) | (i32::from(position.y) << 16);
                record.value_1 = i32::from(position.z);
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    source_entity_index,
                    position,
                    44,
                    self.mission.global_rng(),
                );
                record
            }
            Level1MissionEvent::CastleDefenseShot {
                controller_entity_index,
                target_entity_index,
                projectile_entity_index,
                spell_family,
                subspell,
                yaw,
                pitch,
                first_shot,
            } => {
                let mut record =
                    self.next_event_base(MC_EVENT_LEVEL1_CASTLE_PROJECTILE_SPAWNED, hash);
                record.entity_index = projectile_entity_index;
                record.code = u32::from(spell_family)
                    | (u32::from(subspell) << 8)
                    | (u32::from(first_shot) << 16);
                record.value_0 = i32::from(target_entity_index);
                record.value_1 = i32::from(controller_entity_index);
                record.reserved[0] = u32::from(yaw.cast_unsigned());
                record.reserved[1] = u32::from(pitch.cast_unsigned());
                record
            }
            Level1MissionEvent::CastleManaChanged { mana } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_CASTLE_MANA_CHANGED, hash);
                record.value_0 = mana;
                record
            }
            Level1MissionEvent::PlayerDamaged { damage, life } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_PLAYER_DAMAGED, hash);
                record.entity_index = self.state.entity_index;
                record.value_0 = damage;
                record.value_1 = life;
                record
            }
            Level1MissionEvent::PlayerDied => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_PLAYER_DIED, hash);
                record.entity_index = self.state.entity_index;
                record
            }
            Level1MissionEvent::MissionSound {
                sound_id,
                source_entity_index,
                position,
                random_state,
            } => {
                let mut record = self.next_event_base(MC_EVENT_PREPARE_SOUND, hash);
                record.code = u32::from(sound_id);
                let source_action = self
                    .mission
                    .source_slots()
                    .get(usize::from(source_entity_index))
                    .map_or(0, |source| source.action);
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    source_entity_index,
                    position,
                    source_action,
                    random_state,
                );
                record
            }
            Level1MissionEvent::ExitSequenceStarted => {
                self.next_event_base(MC_EVENT_LEVEL1_EXIT_SEQUENCE_STARTED, hash)
            }
            Level1MissionEvent::ExitSequenceCompleted { result_code } => {
                let mut record =
                    self.next_event_base(MC_EVENT_LEVEL1_EXIT_SEQUENCE_COMPLETED, hash);
                record.code = u32::from(result_code);
                record
            }
            Level1MissionEvent::LevelCompleted => {
                self.next_event_base(MC_EVENT_LEVEL1_COMPLETED, hash)
            }
        }
    }

    fn attach_mission_sound(
        &mut self,
        record: &mut McEventV2,
        sound_id: u16,
        source_entity_index: u16,
        source_position: Position,
        source_action: u8,
        random_state: u32,
    ) {
        record.entity_index = source_entity_index;
        record.asset_ref_index = self
            .world
            .sound_asset_ref(sound_id)
            .unwrap_or(MC_ASSET_REF_NONE);
        let source_id = self
            .world
            .entities()
            .get(usize::from(source_entity_index))
            .map_or(source_entity_index, |source| source.source_id);
        if let Some(prepared) = prepare_event_sound_command(
            self.state.position,
            self.state.yaw,
            source_position,
            sound_id,
            random_state,
            source_id,
            source_action,
        ) {
            self.insert_prepared_audio_command(
                record.sequence,
                audio_command_record(
                    record.generation,
                    record.sequence,
                    record.state_hash,
                    record.asset_ref_index,
                    sound_id,
                    source_entity_index,
                    record.player_index,
                    prepared.command,
                    prepared.play_type,
                ),
            );
        }
    }

    fn prepare_audio_command(&self, event: &McEventV2) -> Option<McAudioCommandV2> {
        let sound_id = u16::try_from(event.code).ok()?;
        if matches!(sound_id, 1 | 2 | 5 | 31) {
            return Some(local_audio_command_record(
                event,
                sound_id,
                0,
                POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING,
            ));
        }
        if matches!(sound_id, 14 | 29) {
            // PrepareEventSound's local LevelIndex/-1 branch stores type 1
            // for menu select and can't-use feedback.
            return Some(local_audio_command_record(event, sound_id, 0, 1));
        }
        let source = self.world.entities().get(usize::from(event.entity_index))?;
        let source_position = if event.entity_index == self.state.entity_index {
            self.state.position
        } else {
            source.position
        };
        let random_state = self
            .context
            .effects
            .as_ref()
            .map_or(self.world.runtime().random_state, |effects| {
                effects.global_rng
            });
        let prepared = prepare_event_sound_command(
            self.state.position,
            self.state.yaw,
            source_position,
            sound_id,
            random_state,
            source.source_id,
            source.action,
        )?;
        Some(audio_command_record(
            event.generation,
            event.sequence,
            event.state_hash,
            event.asset_ref_index,
            sound_id,
            event.entity_index,
            event.player_index,
            prepared.command,
            prepared.play_type,
        ))
    }
}

fn selector_state_hash_v1(frames: &GoatTmapClocks, feedback: Option<(u64, u8)>) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for byte in b"mc2-selector-frame-session-v1\0"
        .iter()
        .copied()
        .chain(frames.hash_v1().to_le_bytes())
        .chain(u8::from(feedback.is_some()).to_le_bytes())
        .chain(feedback.map_or(0, |value| value.0).to_le_bytes())
        .chain(feedback.map_or(0, |value| value.1).to_le_bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

fn camera_history_hash(camera: CameraHistory) -> u64 {
    let mut bytes = Vec::with_capacity(64);
    bytes.extend_from_slice(b"mc2-presentation-camera-v1\0");
    bytes.extend_from_slice(&camera.tick.to_le_bytes());
    bytes.extend_from_slice(&camera.source_turn.to_le_bytes());
    bytes.extend_from_slice(&camera.x.to_le_bytes());
    bytes.extend_from_slice(&camera.y.to_le_bytes());
    bytes.extend_from_slice(&camera.z.to_le_bytes());
    bytes.extend_from_slice(&camera.yaw.to_le_bytes());
    bytes.extend_from_slice(&camera.pitch.to_le_bytes());
    bytes.extend_from_slice(&camera.roll.to_le_bytes());
    bytes.extend_from_slice(&camera.fov.to_le_bytes());
    sim_core::presentation_frame::byte_hash(&bytes)
}

fn selector_frame_state(
    ordinal: usize,
    clock: &GoatTmapClock,
    bank: &GoatTmapBank,
) -> McSelectorFrameStateV2 {
    let selector = &bank.selectors()[ordinal];
    McSelectorFrameStateV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McSelectorFrameStateV2>(),
        selector: clock.selector(),
        mask_bit: ordinal as u8,
        authority: MC_AUTHORITY_SELECTOR_FRAME_LIVE,
        width: clock.width(),
        height: clock.height(),
        source_frame_index: u16::from(clock.frame_index()),
        frame_count: 4,
        stream_start: selector.stream_start,
        stream_cursor: clock.stream_cursor(),
        payload_bytes: selector.payload_bytes,
        pixel_bytes: u32::try_from(clock.pixels().len())
            .expect("qualified Goat TMAP pixel count fits u32"),
        flags: (MC_SELECTOR_FRAME_ANIMATED | MC_SELECTOR_FRAME_PIXELS_MUTABLE)
            & !MC_SELECTOR_FRAME_GROUPED_UPDATE
            | if clock.used_by_preceding_draw() {
                MC_SELECTOR_FRAME_USED_BY_PRECEDING_DRAW
            } else {
                0
            },
        pixel_hash: clock.pixel_hash_v1(),
        reserved: [0; 4],
    }
}

fn local_audio_command_record(
    event: &McEventV2,
    sound_id: u16,
    sound_flags: i16,
    play_type: u8,
) -> McAudioCommandV2 {
    let loop_count = if matches!(sound_id, 1 | 2 | 5 | 31) {
        -1
    } else {
        0
    };
    McAudioCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McAudioCommandV2>(),
        generation: event.generation,
        event_sequence: event.sequence,
        state_hash: event.state_hash,
        asset_ref_index: event.asset_ref_index,
        playback_rate_hz: 22_050,
        sound_id,
        entity_index: event.entity_index,
        player_index: event.player_index,
        sound_flags,
        volume_q15: if matches!(sound_id, 14 | 29) {
            0x7fff
        } else {
            0
        },
        pan_q16: 0x7fff,
        voice_index: MC_AUDIO_VOICE_NONE,
        play_type,
        loop_count,
        action: 0,
        reserved: [0; 4],
        ..McAudioCommandV2::default()
    }
}

#[allow(clippy::too_many_arguments)]
fn audio_command_record(
    generation: u64,
    event_sequence: u64,
    state_hash: u64,
    asset_ref_index: u32,
    sound_id: u16,
    entity_index: u16,
    player_index: i16,
    command: PositionalSoundCommand,
    play_type: u8,
) -> McAudioCommandV2 {
    let playback_rate_hz = if command.audible {
        let percent = u32::try_from(100 + i32::from(command.rate_delta_percent))
            .expect("bounded playback percentage is positive");
        22_050 * percent / 100
    } else {
        0
    };
    McAudioCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McAudioCommandV2>(),
        generation,
        event_sequence,
        state_hash,
        asset_ref_index,
        relative_x: command.relative_x,
        relative_y: command.relative_y,
        relative_z: command.relative_z,
        playback_rate_hz,
        sound_id,
        entity_index,
        player_index,
        sound_flags: command.sound_flags,
        volume_q15: command.volume_q15,
        pan_q16: command.pan_q16,
        rate_delta_percent: command.rate_delta_percent,
        voice_index: MC_AUDIO_VOICE_NONE,
        play_type,
        loop_count: 0,
        action: if command.audible {
            0
        } else {
            MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME
        },
        pooled_flush_rank: 0,
        reserved: [0; 4],
    }
}

fn positional_qualification_command(index: u32) -> AbiResult<McAudioCommandV2> {
    let (dx, dy, sound_id, random_state) = match index {
        0 | 22 => (0, -1_024, 43, 28),
        1 => (512, -512, 37, 0),
        2 => (1_024, 0, 39, 0),
        3 => (512, 512, 37, 0),
        4 => (0, 1_024, 37, 0),
        5 => (-512, 512, 37, 0),
        6 => (-1_024, 0, 37, 0),
        7 => (-512, -512, 37, 0),
        8 => (320, 0, 37, 0),
        9 => (321, 0, 37, 0),
        10 => (0, -12_095, 37, 0),
        11 => (0, -12_096, 37, 0),
        12 => (9_071, 0, 37, 0),
        13 => (9_072, 0, 37, 0),
        14 => (0, 6_047, 37, 0),
        15 => (0, 6_048, 37, 0),
        16 => (12_288, 0, 37, 0),
        17 => (12_288, 1, 37, 0),
        18 => (0, 0, 43, 13),
        19 => (0, 0, 43, 28),
        20 => (0, 0, 43, 20),
        21 => (0, -6_000, 43, 28),
        _ => {
            return Err(AbiError::new(
                MC_STATUS_BOUNDS,
                format!("positional qualification vector {index} is out of bounds"),
            ));
        }
    };
    let listener = Position {
        x: 32_768,
        y: 32_768,
        z: 1_000,
    };
    let source = Position {
        x: u16::try_from(i32::from(listener.x) + dx).expect("qualification x remains in range"),
        y: u16::try_from(i32::from(listener.y) + dy).expect("qualification y remains in range"),
        z: listener.z,
    };
    let prepared = prepare_event_sound_command(listener, 0, source, sound_id, random_state, 321, 0)
        .expect("qualification vector uses a proven Level 1 positional sound");
    let asset_ref_index = match sound_id {
        37 => 95,
        39 => 96,
        43 => 99,
        _ => unreachable!("qualification sound is bounded"),
    };
    let mut record = audio_command_record(
        1,
        u64::from(index) + 1,
        0,
        asset_ref_index,
        sound_id,
        321,
        -1,
        prepared.command,
        prepared.play_type,
    );
    if record.action == 0 {
        record.action = MC_AUDIO_ACTION_START;
        record.voice_index = 0;
    }
    if index == 22 {
        record.action = MC_AUDIO_ACTION_SUPPRESS_EXISTING;
        record.voice_index = 0;
    }
    Ok(record)
}

#[derive(Default)]
struct Registry {
    next_handle: u64,
    sessions: BTreeMap<u64, Arc<Mutex<SessionRecord>>>,
}

impl Registry {
    fn insert(&mut self, session: SessionRecord) -> AbiResult<u64> {
        for _ in 0..u16::MAX {
            self.next_handle = next_nonzero(self.next_handle);
            if !self.sessions.contains_key(&self.next_handle) {
                let handle = self.next_handle;
                self.sessions.insert(handle, Arc::new(Mutex::new(session)));
                return Ok(handle);
            }
        }
        Err(AbiError::new(
            MC_STATUS_LOCK_FAILED,
            "session handle space is unavailable",
        ))
    }
}

static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();

fn registry() -> &'static Mutex<Registry> {
    REGISTRY.get_or_init(|| Mutex::new(Registry::default()))
}

fn boundary(operation: impl FnOnce() -> AbiResult<()>) -> i32 {
    LAST_ERROR.with(|error| error.borrow_mut().clear());
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => MC_STATUS_OK,
        Ok(Err(error)) => {
            set_last_error(&error.message);
            error.status
        }
        Err(_) => {
            set_last_error("panic contained at mc_native ABI boundary");
            MC_STATUS_INTERNAL_PANIC
        }
    }
}

fn boundary_value<T: Copy>(
    output: *mut T,
    output_size: u32,
    operation: impl FnOnce() -> AbiResult<T>,
) -> i32 {
    boundary(|| {
        require_struct_output::<T>(output, output_size, "output")?;
        let value = operation()?;
        // SAFETY: the caller supplied a non-null writable output pointer per
        // the ABI contract; unaligned writes support conservative P/Invoke.
        unsafe { output.write_unaligned(value) };
        Ok(())
    })
}

fn set_last_error(message: &str) {
    LAST_ERROR.with(|error| {
        let mut error = error.borrow_mut();
        error.clear();
        error.push_str(message);
    });
}

fn with_registry<T>(operation: impl FnOnce(&Registry) -> AbiResult<T>) -> AbiResult<T> {
    let guard = registry()
        .lock()
        .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "session registry lock is poisoned"))?;
    operation(&guard)
}

fn with_registry_mut<T>(operation: impl FnOnce(&mut Registry) -> AbiResult<T>) -> AbiResult<T> {
    let mut guard = registry()
        .lock()
        .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "session registry lock is poisoned"))?;
    operation(&mut guard)
}

fn session_arc(registry: &Registry, handle: u64) -> AbiResult<Arc<Mutex<SessionRecord>>> {
    if handle == 0 {
        return Err(AbiError::new(
            MC_STATUS_UNKNOWN_HANDLE,
            "session handle zero is invalid",
        ));
    }
    registry.sessions.get(&handle).cloned().ok_or_else(|| {
        AbiError::new(
            MC_STATUS_UNKNOWN_HANDLE,
            format!("unknown session handle {handle}"),
        )
    })
}

fn with_session<T>(
    handle: u64,
    operation: impl FnOnce(&SessionRecord) -> AbiResult<T>,
) -> AbiResult<T> {
    let entry = with_registry(|registry| session_arc(registry, handle))?;
    let guard = entry
        .lock()
        .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "session lock is poisoned"))?;
    operation(&guard)
}

fn with_session_mut<T>(
    handle: u64,
    operation: impl FnOnce(&mut SessionRecord) -> AbiResult<T>,
) -> AbiResult<T> {
    let entry = with_registry(|registry| session_arc(registry, handle))?;
    let mut guard = entry
        .lock()
        .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "session lock is poisoned"))?;
    operation(&mut guard)
}

#[derive(Debug)]
enum CampaignSessionKind {
    Level1 {
        world: Arc<NativeWorld>,
    },
    Level2 {
        inspection: Arc<NativeWorldInspection>,
        reset: Level2ResetSession,
    },
}

#[derive(Debug)]
struct CampaignSessionRecord {
    kind: CampaignSessionKind,
    generation: u64,
}

#[derive(Default)]
struct CampaignRegistry {
    next_handle: u64,
    sessions: BTreeMap<u64, Arc<Mutex<CampaignSessionRecord>>>,
}

impl CampaignRegistry {
    fn insert(&mut self, session: CampaignSessionRecord) -> AbiResult<u64> {
        for _ in 0..u16::MAX {
            self.next_handle = next_nonzero(self.next_handle);
            if !self.sessions.contains_key(&self.next_handle) {
                let handle = self.next_handle;
                self.sessions.insert(handle, Arc::new(Mutex::new(session)));
                return Ok(handle);
            }
        }
        Err(AbiError::new(
            MC_STATUS_LOCK_FAILED,
            "campaign session handle space is unavailable",
        ))
    }
}

static CAMPAIGN_REGISTRY: OnceLock<Mutex<CampaignRegistry>> = OnceLock::new();

fn campaign_registry() -> &'static Mutex<CampaignRegistry> {
    CAMPAIGN_REGISTRY.get_or_init(|| Mutex::new(CampaignRegistry::default()))
}

fn with_campaign_registry<T>(
    operation: impl FnOnce(&CampaignRegistry) -> AbiResult<T>,
) -> AbiResult<T> {
    let guard = campaign_registry().lock().map_err(|_| {
        AbiError::new(
            MC_STATUS_LOCK_FAILED,
            "campaign session registry lock is poisoned",
        )
    })?;
    operation(&guard)
}

fn with_campaign_registry_mut<T>(
    operation: impl FnOnce(&mut CampaignRegistry) -> AbiResult<T>,
) -> AbiResult<T> {
    let mut guard = campaign_registry().lock().map_err(|_| {
        AbiError::new(
            MC_STATUS_LOCK_FAILED,
            "campaign session registry lock is poisoned",
        )
    })?;
    operation(&mut guard)
}

fn campaign_session_arc(
    registry: &CampaignRegistry,
    handle: u64,
) -> AbiResult<Arc<Mutex<CampaignSessionRecord>>> {
    if handle == 0 {
        return Err(AbiError::new(
            MC_STATUS_UNKNOWN_HANDLE,
            "campaign session handle zero is invalid",
        ));
    }
    registry.sessions.get(&handle).cloned().ok_or_else(|| {
        AbiError::new(
            MC_STATUS_UNKNOWN_HANDLE,
            format!("unknown campaign session handle {handle}"),
        )
    })
}

fn with_campaign_session<T>(
    handle: u64,
    operation: impl FnOnce(&CampaignSessionRecord) -> AbiResult<T>,
) -> AbiResult<T> {
    let entry = with_campaign_registry(|registry| campaign_session_arc(registry, handle))?;
    let guard = entry
        .lock()
        .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "campaign session lock is poisoned"))?;
    operation(&guard)
}

fn with_campaign_session_mut<T>(
    handle: u64,
    operation: impl FnOnce(&mut CampaignSessionRecord) -> AbiResult<T>,
) -> AbiResult<T> {
    let entry = with_campaign_registry(|registry| campaign_session_arc(registry, handle))?;
    let mut guard = entry
        .lock()
        .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "campaign session lock is poisoned"))?;
    operation(&mut guard)
}

#[unsafe(no_mangle)]
/// Mechanically infallible constant expression: this export touches no memory,
/// performs no allocation and invokes no panic-capable operation.
pub extern "C" fn mc_native_abi_version() -> u32 {
    MC_NATIVE_ABI_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_open(
    package_dir: *const u8,
    package_dir_len: u32,
    closure_path: *const u8,
    closure_path_len: u32,
    out_handle: *mut u64,
) -> i32 {
    boundary_value(out_handle, struct_size::<u64>(), || {
        let package = utf8_input(package_dir, package_dir_len, "package_dir")?;
        let closure = utf8_input(closure_path, closure_path_len, "closure_path")?;
        if package.is_empty() || closure.is_empty() {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "package_dir and closure_path must be non-empty",
            ));
        }
        let world = if let Some(cache_root) = std::env::var_os("MC_ASSET_CACHE_PATH") {
            NativeWorld::load_with_owner_cache(&package, &closure, PathBuf::from(cache_root))
        } else {
            NativeWorld::load(&package, &closure)
        }
        .map_err(|error| {
            let message = error.to_string();
            let status = if message.starts_with("cannot read ") {
                MC_STATUS_IO_ERROR
            } else {
                MC_STATUS_VALIDATION_FAILED
            };
            AbiError::new(status, message)
        })?;
        let session = SessionRecord::new(world)?;
        with_registry_mut(|registry| registry.insert(session))
    })
}

/// Open a validated common campaign session. Level 1 uses the same loader as
/// the legacy call; Level 2 uses the typed inspection/content seam and never
/// falls through to the Level 1 mission dispatcher.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_session_open(
    package_dir: *const u8,
    package_dir_len: u32,
    closure_path: *const u8,
    closure_path_len: u32,
    selection: *const McCampaignSelectionV2,
    out_handle: *mut u64,
) -> i32 {
    boundary(|| {
        require_output(out_handle, "out_handle")?;
        write_output(out_handle, 0);
        let selection = read_versioned_input(selection, "campaign selection")?;
        validate_reserved(&selection.reserved, "campaign selection")?;
        if selection.reserved_0 != 0 {
            return Err(AbiError::new(
                MC_STATUS_RESERVED_NONZERO,
                "campaign selection reserved_0 must be zero",
            ));
        }
        let package = utf8_input(package_dir, package_dir_len, "package_dir")?;
        if package.is_empty() {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "package_dir must be non-empty",
            ));
        }
        let expected = sim_core::mission_content::MissionContentProfile::from_metadata(
            selection.external_level,
            selection.internal_level,
            selection.map_type,
        )
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))?;
        let session = if expected.is_level2() {
            let inspection = NativeWorld::inspect_level2(&package)
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))?;
            let reset = inspection
                .level2_reset_session()
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))?;
            CampaignSessionRecord {
                kind: CampaignSessionKind::Level2 {
                    inspection: Arc::new(inspection),
                    reset,
                },
                generation: 0,
            }
        } else {
            let closure = utf8_input(closure_path, closure_path_len, "closure_path")?;
            if closure.is_empty() {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "Level 1 campaign sessions require closure_path",
                ));
            }
            let world = NativeWorld::load(&package, &closure)
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))?;
            CampaignSessionRecord {
                kind: CampaignSessionKind::Level1 {
                    world: Arc::new(world),
                },
                generation: 0,
            }
        };
        let handle = with_campaign_registry_mut(|registry| registry.insert(session))?;
        write_output(out_handle, handle);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_session_destroy(handle: u64) -> i32 {
    boundary(|| {
        with_campaign_registry_mut(|registry| {
            if registry.sessions.remove(&handle).is_none() {
                return Err(AbiError::new(
                    MC_STATUS_UNKNOWN_HANDLE,
                    format!("unknown campaign session handle {handle}"),
                ));
            }
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_session_reset(handle: u64) -> i32 {
    boundary(|| {
        with_campaign_session_mut(handle, |session| {
            if let CampaignSessionKind::Level2 { reset, .. } = &mut session.kind {
                reset.reset();
                session.generation = session.generation.wrapping_add(1);
            }
            Ok(())
        })
    })
}

/// Execute one additive Level 2 outer turn.  This path deliberately owns a
/// separate campaign handle and never enters the legacy Level 1 session
/// dispatcher.  The candidate session is stepped on a clone first so a short
/// event buffer or invalid output cannot partially mutate the live handle.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_session_step(
    handle: u64,
    input: *const McCampaignStepInputV2,
    out_snapshot: *mut McCampaignStepSnapshotV2,
    out_snapshot_size: u32,
    events: *mut McCampaignEventV2,
    event_capacity: u32,
    event_element_size: u32,
    out_event_count: *mut u32,
) -> i32 {
    boundary(|| {
        require_struct_output::<McCampaignStepSnapshotV2>(
            out_snapshot,
            out_snapshot_size,
            "campaign step snapshot",
        )?;
        require_output(out_event_count, "campaign step event count")?;
        require_element_size::<McCampaignEventV2>(event_element_size, "campaign step event")?;
        let input = read_versioned_input(input, "campaign step input")?;
        validate_reserved(&input.reserved, "campaign step input")?;
        if input.reserved_0 != 0 {
            return Err(AbiError::new(
                MC_STATUS_RESERVED_NONZERO,
                "campaign step input reserved_0 must be zero",
            ));
        }
        if input.speed_index > 2 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "campaign step speed_index must be 0, 1, or 2",
            ));
        }
        validate_embedded_command(&input.command)?;
        let command = player_command(input.command)?;
        with_campaign_session_mut(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &mut session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign step requires a Level 2 campaign session",
                ));
            };
            if input.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign step generation {} does not match {}",
                        input.generation, session.generation
                    ),
                ));
            }
            let player = reset
                .players()
                .get(usize::from(input.player_slot))
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!(
                            "campaign player slot {} is out of bounds",
                            input.player_slot
                        ),
                    )
                })?;
            if player.active_player_index < 0 {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("campaign player slot {} is inactive", input.player_slot),
                ));
            }
            if input.player_slot != reset.runtime().local_player_slot {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!(
                        "campaign step player slot {} is not the local slot {}",
                        input.player_slot,
                        reset.runtime().local_player_slot
                    ),
                ));
            }

            let mut candidate = reset.clone();
            candidate.state.entity_index = player.entity_index;
            candidate.state.position = Position {
                x: input.player_x,
                y: input.player_y,
                z: input.player_z,
            };
            candidate.step_outer_turn(command, input.speed_index);

            let event_rows = candidate
                .dispatch_events()
                .iter()
                .enumerate()
                .map(|(ordinal, event)| campaign_event_snapshot(session.generation, ordinal, event))
                .collect::<Vec<_>>();
            copy_slice(&event_rows, events, event_capacity, out_event_count)?;
            let mut event_kind_mask = 0_u32;
            for event in &event_rows {
                if (1..=32).contains(&event.kind) {
                    event_kind_mask |= 1_u32 << u32::from(event.kind - 1);
                }
            }
            let snapshot = McCampaignStepSnapshotV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McCampaignStepSnapshotV2>(),
                generation: session.generation,
                revision: candidate.state.tick,
                state_hash: candidate.campaign_state_hash(),
                terrain_hash: candidate.terrain_hash(),
                dispatch_hash: decode_sha256_hex(&candidate.dispatch_observability_hash()),
                event_count: u32::try_from(event_rows.len()).map_err(|_| {
                    AbiError::new(MC_STATUS_BOUNDS, "campaign event count exceeds u32")
                })?,
                event_kind_mask,
                player_slot: input.player_slot,
                reserved_0: [0; 3],
                player_entity_index: candidate.state.entity_index,
                reserved_1: 0,
                x: candidate.state.position.x,
                y: candidate.state.position.y,
                z: candidate.state.position.z,
                reserved: [0; 4],
            };
            *reset = candidate;
            Ok(snapshot)
        })
        .and_then(|snapshot| {
            write_output(out_snapshot, snapshot);
            Ok(())
        })
    })
}

/// Copy the detailed Level 2 economy sidecars from the most recent outer
/// turn.  Generic `McCampaignEventV2` rows remain the stable 48-byte event
/// stream; callers join these rows by `generation + ordinal`.  Level 1
/// campaign handles and reset/initial states have no kind-10 sidecars.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_economy_events(
    handle: u64,
    events: *mut McCampaignEconomyEventV2,
    event_capacity: u32,
    event_element_size: u32,
    out_event_count: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_event_count, "campaign economy event count")?;
        require_element_size::<McCampaignEconomyEventV2>(
            event_element_size,
            "campaign economy event",
        )?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign economy events require a Level 2 campaign session",
                ));
            };
            let rows = reset
                .dispatch_events()
                .iter()
                .enumerate()
                .filter_map(|(ordinal, dispatch)| match dispatch {
                    Level2DispatchEvent::Economy {
                        pass,
                        source_slot,
                        event,
                    } => Some(campaign_economy_event_snapshot(
                        session.generation,
                        ordinal,
                        *pass,
                        *source_slot,
                        *event,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            copy_slice(&rows, events, event_capacity, out_event_count)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_world_info(
    handle: u64,
    out_info: *mut McCampaignWorldInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        with_campaign_session(handle, |session| {
            let (metadata, entity_count, active_count, player_count, objective_count, hash) =
                match &session.kind {
                    CampaignSessionKind::Level1 { world } => (
                        world.metadata(),
                        world.entities().len(),
                        world
                            .entities()
                            .iter()
                            .filter(|entity| entity.active)
                            .count(),
                        world.players().len(),
                        world.runtime().objectives.len(),
                        state_hash(world.reset_state()),
                    ),
                    CampaignSessionKind::Level2 { inspection, reset } => (
                        inspection.metadata(),
                        inspection.entities().len(),
                        inspection
                            .entities()
                            .iter()
                            .filter(|entity| entity.active)
                            .count(),
                        inspection.players().len(),
                        inspection.runtime().objectives.len(),
                        reset.campaign_state_hash(),
                    ),
                };
            Ok(McCampaignWorldInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McCampaignWorldInfoV2>(),
                external_level: metadata.external_level,
                internal_level: metadata.internal_level,
                map_type: metadata.map_type.id(),
                profile: MC_PROFILE_OUTDOOR,
                reserved_0: 0,
                entity_count: u32::try_from(entity_count).expect("entity count fits u32"),
                active_entity_count: u32::try_from(active_count).expect("active count fits u32"),
                player_count: u32::try_from(player_count).expect("player count fits u32"),
                objective_count: u32::try_from(objective_count).expect("objective count fits u32"),
                terrain_width: 256,
                terrain_height: 256,
                reset_state_hash: hash,
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_player_count(handle: u64, out_count: *mut u32) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_campaign_session(handle, |session| {
            let count = match &session.kind {
                CampaignSessionKind::Level1 { world } => world.players().len(),
                CampaignSessionKind::Level2 { inspection, .. } => inspection.players().len(),
            };
            u32::try_from(count)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "player count exceeds u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_objective_count(handle: u64, out_count: *mut u32) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_campaign_session(handle, |session| {
            let count = match &session.kind {
                CampaignSessionKind::Level1 { world } => world.runtime().objectives.len(),
                CampaignSessionKind::Level2 { inspection, .. } => {
                    inspection.runtime().objectives.len()
                }
            };
            u32::try_from(count)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "objective count exceeds u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_entity_count(handle: u64, out_count: *mut u32) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_campaign_session(handle, |session| {
            let count = match &session.kind {
                CampaignSessionKind::Level1 { world } => world.entities().len(),
                CampaignSessionKind::Level2 { inspection, .. } => inspection.entities().len(),
            };
            u32::try_from(count)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "entity count exceeds u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_players_copy(
    handle: u64,
    start_slot: u32,
    buffer: *mut McCampaignPlayerSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McCampaignPlayerSnapshotV2>(
            element_size,
            "campaign player element",
        )?;
        with_campaign_session(handle, |session| {
            let (players, participant_count) = match &session.kind {
                CampaignSessionKind::Level1 { world } => {
                    (world.players(), world.runtime().player_count)
                }
                CampaignSessionKind::Level2 { inspection, .. } => {
                    (inspection.players(), inspection.runtime().player_count)
                }
            };
            let start = usize::try_from(start_slot).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign player start exceeds usize")
            })?;
            if start > players.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign player start out of bounds",
                ));
            }
            let count = (players.len() - start).min(usize::try_from(capacity).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign player capacity exceeds usize")
            })?);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign player buffer is null",
                ));
            }
            for (offset, player) in players.iter().skip(start).take(count).enumerate() {
                let value = campaign_player_snapshot(player, player.slot < participant_count);
                unsafe { buffer.add(offset).write_unaligned(value) };
            }
            write_output(
                out_written,
                u32::try_from(count).expect("player count fits u32"),
            );
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_objectives_copy(
    handle: u64,
    start_slot: u32,
    buffer: *mut McObjectiveSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McObjectiveSnapshotV2>(element_size, "campaign objective element")?;
        with_campaign_session(handle, |session| {
            let objectives = match &session.kind {
                CampaignSessionKind::Level1 { world } => &world.runtime().objectives,
                CampaignSessionKind::Level2 { inspection, .. } => &inspection.runtime().objectives,
            };
            let start = usize::try_from(start_slot).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign objective start exceeds usize")
            })?;
            if start > objectives.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign objective start out of bounds",
                ));
            }
            let count =
                (objectives.len() - start).min(usize::try_from(capacity).map_err(|_| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        "campaign objective capacity exceeds usize",
                    )
                })?);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign objective buffer is null",
                ));
            }
            for (offset, objective) in objectives.iter().skip(start).take(count).enumerate() {
                unsafe {
                    buffer.add(offset).write_unaligned(objective_snapshot(
                        u8::try_from(start + offset).expect("objective slot fits u8"),
                        objective,
                    ))
                };
            }
            write_output(
                out_written,
                u32::try_from(count).expect("objective count fits u32"),
            );
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_entities_copy(
    handle: u64,
    start_index: u32,
    buffer: *mut McCampaignEntitySnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McCampaignEntitySnapshotV2>(
            element_size,
            "campaign entity element",
        )?;
        with_campaign_session(handle, |session| {
            let entities = match &session.kind {
                CampaignSessionKind::Level1 { world } => world.entities(),
                CampaignSessionKind::Level2 { inspection, .. } => inspection.entities(),
            };
            let start = usize::try_from(start_index).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign entity start exceeds usize")
            })?;
            if start > entities.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign entity start out of bounds",
                ));
            }
            let count = (entities.len() - start).min(usize::try_from(capacity).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign entity capacity exceeds usize")
            })?);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign entity buffer is null",
                ));
            }
            for (offset, entity) in entities.iter().skip(start).take(count).enumerate() {
                unsafe {
                    buffer
                        .add(offset)
                        .write_unaligned(campaign_entity_snapshot(entity))
                };
            }
            write_output(
                out_written,
                u32::try_from(count).expect("entity count fits u32"),
            );
            Ok(())
        })
    })
}

/// Copy the live Level 2 Goat family from the campaign reset session.  The
/// legacy `mc_native_goats_copy` remains Level 1-only; this additive route
/// exposes the authored Level 2 rows after each production campaign step.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_goats_copy(
    handle: u64,
    generation: u64,
    start_ordinal: u32,
    buffer: *mut McGoatSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_batch: *mut McGoatBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        require_element_size::<McGoatSnapshotV2>(element_size, "campaign Goat element")?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Goat snapshots require a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Goat generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let goats = reset.goats();
            let start = usize::try_from(start_ordinal).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign Goat start exceeds usize")
            })?;
            if start > goats.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign Goat start is beyond family count",
                ));
            }
            let capacity = usize::try_from(capacity).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign Goat capacity exceeds usize")
            })?;
            let count = (goats.len() - start).min(capacity);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign Goat output buffer is null",
                ));
            }
            for (offset, goat) in goats.iter().skip(start).take(count).enumerate() {
                unsafe {
                    buffer.add(offset).write_unaligned(goat_snapshot(goat));
                }
            }
            Ok(McGoatBatchV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McGoatBatchV2>(),
                generation: session.generation,
                tick: u64::from(reset.goat_outer_tick()),
                live_world_hash: reset.campaign_state_hash(),
                start_ordinal,
                written: u32::try_from(count).expect("Level 2 Goat count fits u32"),
                total_count: u32::try_from(LEVEL_TWO_GOAT_INDICES.len())
                    .expect("Level 2 Goat count fits u32"),
                has_more: u8::from(start + count < goats.len()),
                reserved_0: [0; 3],
                reserved: [0; 4],
            })
        })
    })
}

/// Copy the live Level 2 mana-sphere arena.  This is deliberately separate
/// from the legacy Level 1 mission endpoint: the rows come from the mutable
/// `Level2EconomyState` arena after the source `sub_5FD00`/`TransformEntityToManaSphere`
/// paths, including retained inactive rows awaiting the reclaim pass.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_mana_spheres_copy(
    handle: u64,
    generation: u64,
    start_ordinal: u32,
    buffer: *mut McManaSphereSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_batch: *mut McCampaignManaBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        require_element_size::<McManaSphereSnapshotV2>(
            element_size,
            "campaign mana sphere element",
        )?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign mana spheres require a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign mana sphere generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let spheres = reset.economy().spheres();
            let start = usize::try_from(start_ordinal).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign mana sphere start exceeds usize")
            })?;
            if start > spheres.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign mana sphere start is beyond arena count",
                ));
            }
            let capacity = usize::try_from(capacity).map_err(|_| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign mana sphere capacity exceeds usize",
                )
            })?;
            let count = (spheres.len() - start).min(capacity);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign mana sphere output buffer is null",
                ));
            }
            for (offset, sphere) in spheres.iter().skip(start).take(count).enumerate() {
                unsafe {
                    buffer
                        .add(offset)
                        .write_unaligned(mana_sphere_snapshot(sphere));
                }
            }
            Ok(McCampaignManaBatchV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McCampaignManaBatchV2>(),
                generation: session.generation,
                tick: reset.state.tick,
                live_world_hash: reset.campaign_state_hash(),
                start_ordinal,
                written: u32::try_from(count).expect("Level 2 mana sphere count fits u32"),
                total_count: u32::try_from(spheres.len())
                    .expect("Level 2 mana sphere count fits u32"),
                has_more: u8::from(start + count < spheres.len()),
                reserved_0: [0; 3],
                reserved: [0; 4],
            })
        })
    })
}

/// Copy every retained Level 2 mana-carrier row in source allocation order.
/// Do not collapse this to `economy().carrier()`: remc2 can retain multiple
/// active/inactive carriers in the shared arena until the reclaim pass.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_mana_carriers_copy(
    handle: u64,
    generation: u64,
    start_ordinal: u32,
    buffer: *mut McManaCarrierSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_batch: *mut McCampaignManaBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        require_element_size::<McManaCarrierSnapshotV2>(
            element_size,
            "campaign mana carrier element",
        )?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign mana carriers require a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign mana carrier generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let carriers = reset.economy().carriers();
            let start = usize::try_from(start_ordinal).map_err(|_| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign mana carrier start exceeds usize",
                )
            })?;
            if start > carriers.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign mana carrier start is beyond arena count",
                ));
            }
            let capacity = usize::try_from(capacity).map_err(|_| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign mana carrier capacity exceeds usize",
                )
            })?;
            let count = (carriers.len() - start).min(capacity);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign mana carrier output buffer is null",
                ));
            }
            for (offset, carrier) in carriers.iter().skip(start).take(count).enumerate() {
                unsafe {
                    buffer
                        .add(offset)
                        .write_unaligned(mana_carrier_snapshot_value(
                            carrier,
                            session.generation,
                            reset.state.tick,
                        ));
                }
            }
            Ok(McCampaignManaBatchV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McCampaignManaBatchV2>(),
                generation: session.generation,
                tick: reset.state.tick,
                live_world_hash: reset.campaign_state_hash(),
                start_ordinal,
                written: u32::try_from(count).expect("Level 2 mana carrier count fits u32"),
                total_count: u32::try_from(carriers.len())
                    .expect("Level 2 mana carrier count fits u32"),
                has_more: u8::from(start + count < carriers.len()),
                reserved_0: [0; 3],
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_destroy(handle: u64) -> i32 {
    boundary(|| {
        with_registry_mut(|registry| {
            if handle == 0 || registry.sessions.remove(&handle).is_none() {
                Err(AbiError::new(
                    MC_STATUS_UNKNOWN_HANDLE,
                    format!("unknown session handle {handle}"),
                ))
            } else {
                Ok(())
            }
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_reset(
    handle: u64,
    out_snapshot: *mut McSessionSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session_mut(handle, |session| {
            session.reset();
            Ok(session.snapshot())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_snapshot(
    handle: u64,
    out_snapshot: *mut McSessionSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| Ok(session.snapshot()))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_source_projection_hash(handle: u64, out_hash: *mut u64) -> i32 {
    boundary_value(out_hash, struct_size::<u64>(), || {
        with_session(handle, |session| Ok(source_projection_hash(&session.state)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_source_arena_hash(handle: u64, out_hash: *mut u64) -> i32 {
    boundary_value(out_hash, struct_size::<u64>(), || {
        with_session(handle, |session| Ok(session.mission.source_arena_hash_v1()))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_action_51_fallback_state_hash_v1(
    handle: u64,
    out_hash: *mut u64,
) -> i32 {
    boundary_value(out_hash, struct_size::<u64>(), || {
        with_session(handle, |session| {
            Ok(session.mission.action_51_fallback_state_hash_v1())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_resident_observation_v1(
    handle: u64,
    out_observation: *mut McLevel1ResidentObservationV1,
    out_observation_size: u32,
) -> i32 {
    boundary_value(out_observation, out_observation_size, || {
        with_session(handle, |session| {
            Ok(McLevel1ResidentObservationV1 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McLevel1ResidentObservationV1>(),
                required_mask: session.mission.required_resident_behavior_mask(),
                observed_mask: session.mission.observed_resident_behavior_mask(),
                remaining_mask: session.mission.remaining_resident_behavior_mask(),
                reserved_0: 0,
                digest: session.mission.resident_behavior_observation_digest_v1(),
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_presentation_state(
    handle: u64,
    out_state: *mut McPresentationStateV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_session(handle, |session| Ok(session.presentation_state()))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_render_camera_state(
    handle: u64,
    out_state: *mut McRenderCameraStateV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_session(handle, |session| Ok(session.render_camera_state()))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_terrain_pnt5_vector_count() -> u32 {
    TERRAIN_PNT5_QUALIFICATION_VECTOR_COUNT
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_terrain_pnt5_vector(
    index: u32,
    out_vector: *mut McTerrainPnt5VectorV2,
    out_vector_size: u32,
) -> i32 {
    boundary_value(out_vector, out_vector_size, || {
        let (turn, tile_x, tile_y, shade, angle, view_y, distance_squared) = match index {
            0 => (0, 4, 4, 0, 8, 128, 0),
            1 => (0, 12, 4, 40, 8, 128, 0),
            2 => (1, 0, 0, 40, 8, 128, 0),
            3 => (1, 1, 1, 40, 8, 128, 14_745_600),
            4 => (1, 2, 3, 40, 8, 128, 14_745_601),
            5 => (1, 5, 7, 40, 8, 128, 19_202_048),
            6 => (1, 255, 255, 40, 8, 128, 23_658_495),
            7 => (1, 255, 255, 40, 8, 128, 23_658_496),
            8 => (1, 255, 255, 40, 8, 128, 26_214_399),
            9 => (1, 255, 255, 40, 8, 128, 26_214_400),
            10 => (1, 42, 99, 56, 8, 128, 0),
            11 => (65_535, 256, -1, 55, 8, 128, 14_757_945),
            12 => (1, 0, 0, 40, 8, -256, 0),
            _ => {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    format!("terrain pnt5 qualification vector {index} is out of bounds"),
                ));
            }
        };
        let value =
            outdoor_terrain_pnt5(turn, tile_x, tile_y, shade, angle, view_y, distance_squared);
        Ok(McTerrainPnt5VectorV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McTerrainPnt5VectorV2>(),
            turn,
            tile_x,
            tile_y,
            distance_squared,
            shade,
            angle,
            visible: u8::from(value.visible),
            tablesn_row: value.tablesn_row,
            wave: value.wave,
            land_displacement: value.land_displacement,
            pnt5: value.pnt5,
            view_y,
            lighting_wave: value.lighting_wave,
            reserved: [0; 2],
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_terrain_grid_vertex_count() -> u32 {
    u32::try_from(OUTDOOR_GRID_VERTICES).expect("terrain grid vertex count fits u32")
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_terrain_grid_copy(
    handle: u64,
    buffer: *mut McTerrainGridVertexV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McTerrainGridVertexV2>(element_size, "terrain grid vertex")?;
        with_session(handle, |session| {
            let grid = session.terrain_grid();
            copy_slice(&grid, buffer, capacity, out_written)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_step(
    handle: u64,
    command: *const McCommandV2,
    out_snapshot: *mut McSessionSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        let command = read_versioned_input(command, "command")?;
        validate_command(&command)?;
        let command = player_command(command)?;
        with_session_mut(handle, |session| {
            session.step(command);
            Ok(session.snapshot())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_step_with_camera(
    handle: u64,
    command: *const McCommandV2,
    camera_input: *const McCameraInputV2,
    out_snapshot: *mut McSessionSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        let command = read_versioned_input(command, "command")?;
        validate_command(&command)?;
        let camera_input = read_versioned_input(camera_input, "camera input")?;
        validate_camera_input(&camera_input)?;
        let mut command = player_command(command)?;
        command.argument_6 = camera_input.look_yaw;
        command.argument_8 = camera_input.look_pitch;
        with_session_mut(handle, |session| {
            session.step(command);
            Ok(session.snapshot())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_terrain_frame_copy(
    handle: u64,
    query: *const McTerrainFrameQueryV2,
    index_buffer: *mut u8,
    index_capacity: u32,
    coverage_buffer: *mut u8,
    coverage_capacity: u32,
    out_info: *mut McTerrainFrameInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        let query = read_versioned_input(query, "terrain frame query")?;
        validate_reserved(&query.reserved, "terrain frame query")?;
        with_session(handle, |session| {
            if query.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "terrain frame generation {} does not match {}",
                        query.generation, session.generation
                    ),
                ));
            }
            if query.tick != session.camera_history.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "terrain frame presentation tick {} does not match {}",
                        query.tick, session.camera_history.tick
                    ),
                ));
            }
            let required =
                u32::try_from(LEVEL1_VIEW_PIXELS).expect("qualified terrain frame extent fits u32");
            if index_capacity < required || coverage_capacity < required {
                return Err(AbiError::new(
                    MC_STATUS_BUFFER_TOO_SMALL,
                    format!(
                        "terrain frame requires {required} index and coverage bytes, got {index_capacity} and {coverage_capacity}"
                    ),
                ));
            }
            if index_buffer.is_null() || coverage_buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "terrain frame index and coverage buffers must be non-null",
                ));
            }
            let frame = session.terrain_frame()?;
            // SAFETY: both caller buffers were checked non-null and have at
            // least the exact 64,000-byte frame capacity.
            unsafe {
                index_buffer.copy_from_nonoverlapping(frame.indices.as_ptr(), LEVEL1_VIEW_PIXELS);
                coverage_buffer
                    .copy_from_nonoverlapping(frame.terrain_coverage.as_ptr(), LEVEL1_VIEW_PIXELS);
            }
            Ok(McTerrainFrameInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McTerrainFrameInfoV2>(),
                generation: session.generation,
                tick: session.camera_history.tick,
                width: LEVEL1_VIEW_WIDTH as u16,
                height: LEVEL1_VIEW_HEIGHT as u16,
                stride: LEVEL1_VIEW_WIDTH as u16,
                viewport_x: 0,
                viewport_y: 0,
                viewport_width: LEVEL1_VIEW_WIDTH as u16,
                viewport_height: LEVEL1_VIEW_HEIGHT as u16,
                reserved_0: 0,
                index_bytes: required,
                coverage_bytes: required,
                flags: MC_TERRAIN_FRAME_ISOLATED_SKY_TERRAIN
                    | MC_TERRAIN_FRAME_SKY_SEEDED
                    | MC_TERRAIN_FRAME_HAS_COVERAGE
                    | MC_TERRAIN_FRAME_NON_TERRAIN_OMITTED,
                reflected_triangles: frame.stats.reflected_triangles,
                primary_mode5_triangles: frame.stats.primary_mode5_triangles,
                primary_mode26_triangles: frame.stats.primary_mode26_triangles,
                index_hash: frame.index_hash,
                coverage_hash: frame.coverage_hash,
                terrain_pixel_writes: frame.stats.terrain_pixel_writes,
                reserved: [0; 2],
            })
        })
    })
}

/// Copy the bounded source-ordered Level 1 world frame.
///
/// This preserves the stable terrain-only endpoint while adding the currently
/// qualified mode-20 Goat family. Other non-terrain writers remain explicitly
/// omitted in the returned flags.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_world_frame_copy(
    handle: u64,
    query: *const McTerrainFrameQueryV2,
    index_buffer: *mut u8,
    index_capacity: u32,
    coverage_buffer: *mut u8,
    coverage_capacity: u32,
    out_info: *mut McTerrainFrameInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        let query = read_versioned_input(query, "world frame query")?;
        validate_reserved(&query.reserved, "world frame query")?;
        with_session_mut(handle, |session| {
            if query.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "world frame generation {} does not match {}",
                        query.generation, session.generation
                    ),
                ));
            }
            if query.tick != session.camera_history.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "world frame presentation tick {} does not match {}",
                        query.tick, session.camera_history.tick
                    ),
                ));
            }
            let required =
                u32::try_from(LEVEL1_VIEW_PIXELS).expect("qualified world frame extent fits u32");
            if index_capacity < required || coverage_capacity < required {
                return Err(AbiError::new(
                    MC_STATUS_BUFFER_TOO_SMALL,
                    format!(
                        "world frame requires {required} index and coverage bytes, got {index_capacity} and {coverage_capacity}"
                    ),
                ));
            }
            if index_buffer.is_null() || coverage_buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "world frame index and coverage buffers must be non-null",
                ));
            }
            let frame = session.world_frame()?;
            // SAFETY: both caller buffers were checked non-null and have at
            // least the exact 64,000-byte frame capacity.
            unsafe {
                index_buffer.copy_from_nonoverlapping(frame.indices.as_ptr(), LEVEL1_VIEW_PIXELS);
                coverage_buffer
                    .copy_from_nonoverlapping(frame.terrain_coverage.as_ptr(), LEVEL1_VIEW_PIXELS);
            }
            Ok(McTerrainFrameInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McTerrainFrameInfoV2>(),
                generation: session.generation,
                tick: session.camera_history.tick,
                width: LEVEL1_VIEW_WIDTH as u16,
                height: LEVEL1_VIEW_HEIGHT as u16,
                stride: LEVEL1_VIEW_WIDTH as u16,
                viewport_x: 0,
                viewport_y: 0,
                viewport_width: LEVEL1_VIEW_WIDTH as u16,
                viewport_height: LEVEL1_VIEW_HEIGHT as u16,
                reserved_0: 0,
                index_bytes: required,
                coverage_bytes: required,
                flags: world_frame_flags(
                    frame.reflected_sprites_composited,
                    frame.sprite_shadows_composited,
                ),
                reflected_triangles: frame.stats.reflected_triangles,
                primary_mode5_triangles: frame.stats.primary_mode5_triangles,
                primary_mode26_triangles: frame.stats.primary_mode26_triangles,
                index_hash: frame.index_hash,
                coverage_hash: frame.coverage_hash,
                terrain_pixel_writes: frame.stats.terrain_pixel_writes,
                reserved: [0; 2],
            })
        })
    })
}

/// Copy the revisioned compositor-complete fixed-profile FINAL MAIN.
///
/// Unlike the stable 96-byte terrain/world endpoints, this additive endpoint
/// owns temporal MAIN history, indexed overlays, and per-byte provenance.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_presentation_frame_copy(
    handle: u64,
    query: *const McPresentationFrameQueryV2,
    index_buffer: *mut u8,
    index_capacity: u32,
    provenance_buffer: *mut u8,
    provenance_capacity: u32,
    out_info: *mut McPresentationFrameInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        let query = read_versioned_input(query, "presentation frame query")?;
        validate_reserved(&query.reserved, "presentation frame query")?;
        if query.reserved_0 != 0 {
            return Err(AbiError::new(
                MC_STATUS_RESERVED_NONZERO,
                "presentation frame query reserved_0 must be zero",
            ));
        }
        for (name, value) in [
            ("blur_capability", query.blur_capability),
            ("high_speed_system", query.high_speed_system),
            ("scratch_present", query.scratch_present),
            ("spatial_smoothing", query.spatial_smoothing),
            ("minimap_enabled", query.minimap_enabled),
            ("top_bar_enabled", query.top_bar_enabled),
            ("game_paused", query.game_paused),
            ("level_end", query.level_end),
            ("objective_textbox", query.objective_textbox),
            ("tutorial_or_briefing", query.tutorial_or_briefing),
            ("help_popup", query.help_popup),
        ] {
            if value > 1 {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("presentation frame {name} must be 0 or 1"),
                ));
            }
        }
        if query.debug_text & !0x48 != 0 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "presentation frame debug_text accepts source flags 0x08 and 0x40 only",
            ));
        }
        if query.configured_blur_mode > 2
            || query.viewport_width > LEVEL1_VIEW_WIDTH as u16
            || query.viewport_height > LEVEL1_VIEW_HEIGHT as u16
            || query.screen_stride < query.viewport_width
        {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "presentation blur mode or viewport/stride is invalid",
            ));
        }
        let required =
            u32::try_from(LEVEL1_VIEW_PIXELS).expect("qualified presentation extent fits u32");
        if index_capacity < required || provenance_capacity < required {
            return Err(AbiError::new(
                MC_STATUS_BUFFER_TOO_SMALL,
                format!(
                    "presentation frame requires {required} index and provenance bytes, got {index_capacity} and {provenance_capacity}"
                ),
            ));
        }
        if index_buffer.is_null() || provenance_buffer.is_null() {
            return Err(AbiError::new(
                MC_STATUS_NULL_POINTER,
                "presentation index and provenance buffers must be non-null",
            ));
        }
        with_session_mut(handle, |session| {
            if query.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "presentation generation {} does not match {}",
                        query.generation, session.generation
                    ),
                ));
            }
            if query.tick != session.camera_history.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "presentation tick {} does not match camera history {}",
                        query.tick, session.camera_history.tick
                    ),
                ));
            }
            if (query.game_paused != 0) != session.game_paused {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "presentation paused query does not match authoritative session state",
                ));
            }
            let result = session.presentation_frame(query)?;
            let frame = &result.frame;
            // SAFETY: both caller buffers were checked non-null with the
            // exact fixed-profile capacity.
            unsafe {
                index_buffer.copy_from_nonoverlapping(frame.indices.as_ptr(), LEVEL1_VIEW_PIXELS);
                provenance_buffer
                    .copy_from_nonoverlapping(frame.provenance.as_ptr(), LEVEL1_VIEW_PIXELS);
            }
            let effective_postprocess_mode = match frame.postprocess_branch {
                IndexedPostprocessBranch::Direct { .. } => 0,
                IndexedPostprocessBranch::Temporal { mode } => mode,
            };
            let source = frame.source_receipt;
            let family = source.family_mask;
            let source_flags = if source.sky_seeded {
                MC_TERRAIN_FRAME_SKY_SEEDED
            } else {
                0
            } | if source.has_coverage {
                MC_TERRAIN_FRAME_HAS_COVERAGE
            } else {
                0
            } | if family & 1 != 0 {
                MC_TERRAIN_FRAME_GOATS_COMPOSITED
            } else {
                0
            } | if family & (1 << 1) != 0 {
                MC_TERRAIN_FRAME_FROZEN_ACTORS_COMPOSITED
            } else {
                0
            } | if family & (1 << 3) != 0 {
                MC_TERRAIN_FRAME_ARCHERS_COMPOSITED
            } else {
                0
            } | if family & (1 << 4) != 0 {
                MC_TERRAIN_FRAME_FIREFLIES_COMPOSITED
            } else {
                0
            } | if family & (1 << 5) != 0 {
                MC_TERRAIN_FRAME_COMBAT_COMPOSITED
            } else {
                0
            } | if family & (1 << 6) != 0 {
                MC_TERRAIN_FRAME_OBJECTIVE_PROPS_COMPOSITED
            } else {
                0
            } | if family & (1 << 7) != 0 {
                MC_TERRAIN_FRAME_ECONOMY_COMPOSITED
            } else {
                0
            } | if family & (1 << 8) != 0 {
                MC_TERRAIN_FRAME_SPRITE_SHADOWS_COMPOSITED
            } else {
                0
            } | if source.reflected_sprites_composited {
                MC_TERRAIN_FRAME_REFLECTED_SPRITES_COMPOSITED
            } else {
                0
            };
            let presentation_flags = if frame.has_provenance() {
                MC_PRESENTATION_FRAME_HAS_PROVENANCE
            } else {
                0
            } | if frame.compositor_complete() {
                MC_PRESENTATION_FRAME_COMPOSITOR_COMPLETE
            } else {
                0
            } | if frame.overlays_complete() {
                MC_PRESENTATION_FRAME_OVERLAYS_COMPOSITED
            } else {
                0
            } | if frame.postprocess_complete() {
                MC_PRESENTATION_FRAME_POSTPROCESS_COMPLETE
            } else {
                0
            } | if frame.lighting_complete() {
                MC_PRESENTATION_FRAME_LIGHTING_COMPLETE
            } else {
                0
            };
            Ok(McPresentationFrameInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McPresentationFrameInfoV2>(),
                generation: frame.generation,
                tick: frame.tick,
                presentation_revision: frame.presentation_revision,
                immutable_input_hash: frame.immutable_input_hash,
                width: LEVEL1_VIEW_WIDTH as u16,
                height: LEVEL1_VIEW_HEIGHT as u16,
                stride: LEVEL1_VIEW_WIDTH as u16,
                viewport_width: query.viewport_width,
                index_bytes: required,
                provenance_bytes: required,
                flags: source_flags | presentation_flags,
                effective_postprocess_mode,
                history_state: match session.presentation_frame.history() {
                    MainHistory::Unprimed => 0,
                    MainHistory::Primed => 1,
                },
                newly_committed: u8::from(result.newly_committed),
                reserved_0: 0,
                index_hash: frame.index_hash,
                provenance_hash: frame.provenance_hash,
                lighting_hash: session.mission.dynamic_lighting_hash_v1(),
                source_arena_hash: session.mission.source_arena_hash_v2(),
                overlay_pixels_written: frame.overlay_pixels_written,
                overlay_stage_count: u16::try_from(frame.overlay_receipts.len())
                    .expect("fixed overlay stage count fits u16"),
                reserved_1: 0,
                reserved: [0; 6],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_world_info(
    handle: u64,
    out_info: *mut McWorldInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        with_session(handle, |session| {
            let world = &session.world;
            let metadata = world.metadata();
            Ok(McWorldInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McWorldInfoV2>(),
                external_level: metadata.external_level,
                internal_level: metadata.internal_level,
                map_type: metadata.map_type.id(),
                profile: profile_code(world.profile().cave_mode),
                reserved_0: 0,
                entity_count: u32::try_from(world.entities().len())
                    .expect("validated entity count fits u32"),
                active_entity_count: u32::try_from(
                    world
                        .entities()
                        .iter()
                        .filter(|entity| entity.active)
                        .count(),
                )
                .expect("validated active count fits u32"),
                player_count: u32::try_from(world.players().len())
                    .expect("validated player count fits u32"),
                asset_ref_count: u32::try_from(world.asset_refs().len())
                    .expect("validated asset count fits u32"),
                terrain_width: 256,
                terrain_height: 256,
                reset_state_hash: state_hash(world.reset_state()),
                reset_conformance_hash: conformance_hash(
                    world.reset_state(),
                    world.reset_context(),
                ),
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_world_string_length(
    handle: u64,
    kind: u32,
    out_length: *mut u32,
) -> i32 {
    boundary_value(out_length, struct_size::<u32>(), || {
        with_session(handle, |session| {
            let value = world_string(&session.world, kind)?;
            string_length(value)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_world_string_copy(
    handle: u64,
    kind: u32,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_length, "out_length")?;
        with_session(handle, |session| {
            let value = world_string(&session.world, kind)?;
            copy_string(value, buffer, capacity, out_length)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_terrain_plane_length(
    handle: u64,
    kind: u32,
    out_count: *mut u32,
) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_session(handle, |session| {
            validate_plane_kind(session.terrain.planes(), kind)?;
            Ok(u32::try_from(TERRAIN_CELLS).expect("terrain count fits u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_terrain_state(
    handle: u64,
    out_state: *mut McTerrainStateV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_session(handle, |session| Ok(terrain_state(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_terrain_plane_copy_u8(
    handle: u64,
    kind: u32,
    buffer: *mut u8,
    capacity: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        with_session(handle, |session| {
            let values = terrain_u8_plane(session.terrain.planes(), kind)?;
            copy_slice(values, buffer, capacity, out_written)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_terrain_entity_indices_copy(
    handle: u64,
    buffer: *mut i16,
    capacity: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        with_session(handle, |session| {
            let values = &session.terrain.planes().entity_indices;
            copy_slice(values, buffer, capacity, out_written)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_entity_count(handle: u64, out_count: *mut u32) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_session(handle, |session| {
            Ok(u32::try_from(session.world.entities().len())
                .expect("validated entity count fits u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_objective_count(handle: u64, out_count: *mut u32) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_session(handle, |session| {
            Ok(u32::try_from(session.world.runtime().objectives.len())
                .expect("validated objective count fits u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_objectives_copy(
    handle: u64,
    start_slot: u32,
    buffer: *mut McObjectiveSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McObjectiveSnapshotV2>(element_size, "objective element")?;
        with_session(handle, |session| {
            let objectives = &session.world.runtime().objectives;
            let start = usize::try_from(start_slot)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "objective start exceeds usize"))?;
            if start > objectives.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "objective start is beyond objective count",
                ));
            }
            let remaining = &objectives[start..];
            let count = remaining.len().min(
                usize::try_from(capacity)
                    .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "capacity exceeds usize"))?,
            );
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "objective output buffer is null",
                ));
            }
            for (offset, objective) in remaining.iter().take(count).enumerate() {
                let slot = start + offset;
                let snapshot = objective_snapshot(
                    u8::try_from(slot).expect("validated objective slot fits u8"),
                    objective,
                );
                // SAFETY: buffer is non-null for count records and the caller
                // promises capacity writable records of the exact ABI stride.
                unsafe { buffer.add(offset).write_unaligned(snapshot) };
            }
            write_output(
                out_written,
                u32::try_from(count).expect("objective count fits u32"),
            );
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_entities_copy(
    handle: u64,
    start_index: u32,
    buffer: *mut McEntitySnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McEntitySnapshotV2>(element_size, "entity element")?;
        with_session(handle, |session| {
            let entities = session.world.entities();
            let start = usize::try_from(start_index)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "entity start does not fit usize"))?;
            if start > entities.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "entity start is beyond entity count",
                ));
            }
            let remaining = &entities[start..];
            let count = remaining.len().min(
                usize::try_from(capacity)
                    .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "capacity does not fit usize"))?,
            );
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "entity output buffer is null",
                ));
            }
            for (index, entity) in remaining.iter().take(count).enumerate() {
                // SAFETY: buffer is non-null for count records and the caller
                // promises capacity writable records.
                unsafe {
                    buffer.add(index).write_unaligned(entity_snapshot(
                        entity,
                        &session.state,
                        &session.live_world,
                        &session.mission,
                    ));
                };
            }
            write_output(
                out_written,
                u32::try_from(count).expect("entity count fits u32"),
            );
            Ok::<(), AbiError>(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_goats_copy(
    handle: u64,
    generation: u64,
    start_ordinal: u32,
    buffer: *mut McGoatSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_batch: *mut McGoatBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        require_element_size::<McGoatSnapshotV2>(element_size, "Goat element")?;
        with_session(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "Goat generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let goats = session.live_world.goats();
            let start = usize::try_from(start_ordinal)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "Goat start exceeds usize"))?;
            if start > goats.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "Goat start is beyond family count",
                ));
            }
            let capacity = usize::try_from(capacity)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "Goat capacity exceeds usize"))?;
            let count = (goats.len() - start).min(capacity);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "Goat output buffer is null",
                ));
            }
            for (offset, goat) in goats.iter().skip(start).take(count).enumerate() {
                // SAFETY: the caller supplied exact element stride and
                // writable capacity for `count` records.
                unsafe {
                    buffer.add(offset).write_unaligned(goat_snapshot(goat));
                }
            }
            Ok(McGoatBatchV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McGoatBatchV2>(),
                generation: session.generation,
                tick: session.live_world.outer_tick().into(),
                live_world_hash: session.live_world_hash_v2(),
                start_ordinal,
                written: u32::try_from(count).expect("33 Goats fit u32"),
                total_count: u32::try_from(LEVEL_ONE_GOAT_INDICES.len()).expect("33 Goats fit u32"),
                has_more: u8::from(start + count < goats.len()),
                reserved_0: [0; 3],
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_mission_snapshot(
    handle: u64,
    out_snapshot: *mut McLevel1MissionSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| Ok(level1_mission_snapshot(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_player_lifecycle_snapshot(
    handle: u64,
    out_snapshot: *mut McLevel1PlayerLifecycleSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| {
            Ok(level1_player_lifecycle_snapshot(session))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_player_menu_state(handle: u64, menu_state: u8) -> i32 {
    boundary(|| {
        with_session_mut(handle, |session| {
            let current = session.mission.player_menu_state;
            session.mission.set_player_menu_state(menu_state);
            if current != menu_state && menu_state != 8 && !matches!(current, 5 | 8) {
                session.emit_local_flight_sound(sim_core::menu_cursor_transition_audio_event(
                    session.state.entity_index,
                    i16::from(session.world.runtime().local_player_slot),
                ));
            }
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_game_paused(handle: u64, paused: u8) -> i32 {
    boundary(|| {
        if paused > 1 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "Level 1 paused state must be zero or one",
            ));
        }
        with_session_mut(handle, |session| {
            session.set_game_paused(paused != 0);
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_presentation_input(
    handle: u64,
    input: *const McLevel1PresentationInputV2,
) -> i32 {
    boundary(|| {
        let input = read_versioned_input(input, "Level 1 presentation input")?;
        with_session_mut(handle, |session| session.apply_presentation_input(input))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_player_inject_damage_for_qualification(
    handle: u64,
    damage: i32,
    attacker_entity_index: u16,
) -> i32 {
    boundary(|| {
        with_session_mut(handle, |session| {
            session
                .mission
                .submit_player_damage(damage, attacker_entity_index);
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_action_51_blocker_setup_for_qualification(
    handle: u64,
    house_entity_index: u16,
    house_action: u8,
) -> i32 {
    boundary(|| {
        if !matches!(
            house_action,
            sim_core::house::HOUSE_TERRAIN_TO_LIVE_ACTION | u8::MAX
        ) {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "action-51 qualification house action must be 51 or 255",
            ));
        }
        with_session_mut(handle, |session| {
            let mut mission = session.mission.clone();
            let mut live_world = session.live_world.clone();
            let mut house = live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == house_entity_index)
                .copied()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!("action-51 qualification house {house_entity_index} is absent"),
                    )
                })?;
            house.action = house_action;
            house.max_life = 2;
            house.life = 2;
            house.owner_entity_index = session.state.entity_index;
            house.control_flags |= 1;
            live_world
                .initialize_builder_house(house)
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
            mission
                .inject_action_51_blocker_for_qualification(
                    house.position,
                    house.collision_half_width,
                    house.collision_half_depth,
                )
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "action-51 qualification blocker allocation failed",
                    )
                })?;
            session.mission = mission;
            session.live_world = live_world;
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_action_51_completion_setup_for_qualification(
    handle: u64,
    house_entity_index: u16,
    parent_entity_index: u16,
    construction_selector: u8,
) -> i32 {
    boundary(|| {
        with_session_mut(handle, |session| {
            if session
                .world
                .house_build_footprint(construction_selector)
                .is_none()
            {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("action-51 qualification BUILD {construction_selector} is absent"),
                ));
            }
            let mut mission = session.mission.clone();
            let mut live_world = session.live_world.clone();
            let views = level1_turn_start_source_views(&mission, &session.state, &live_world);
            let parent = views
                .iter()
                .copied()
                .find(|view| view.source_entity_index == parent_entity_index)
                .filter(|view| view.class == 3 && view.life >= 0)
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!(
                            "action-51 qualification parent {parent_entity_index} is not a live class-3 row"
                        ),
                    )
                })?;
            let construction_axis = Position {
                x: parent.position.x.wrapping_add(0x0101),
                y: parent.position.y.wrapping_add(0x0202),
                z: parent.position.z.wrapping_add(33),
            };
            if !mission.configure_action_51_completion_parent_for_qualification(
                parent_entity_index,
                construction_selector,
                construction_axis,
            ) {
                return Err(AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "action-51 qualification parent source fields could not be configured",
                ));
            }
            let mut house = live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == house_entity_index)
                .copied()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!("action-51 qualification house {house_entity_index} is absent"),
                    )
                })?;
            house.action = sim_core::house::HOUSE_TERRAIN_TO_LIVE_ACTION;
            house.max_life = 1;
            house.life = 1;
            house.owner_entity_index = session.state.entity_index;
            house.control_flags |= 1;
            house.position = parent.position;
            live_world
                .initialize_builder_house(house)
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
            session.mission = mission;
            session.live_world = live_world;
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_hud_snapshot(
    handle: u64,
    out_snapshot: *mut McLevel1HudSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| Ok(level1_hud_snapshot(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_spell_menu_snapshot(
    handle: u64,
    out_snapshot: *mut McLevel1SpellMenuSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| Ok(level1_spell_menu_snapshot(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_minimap_marks(
    handle: u64,
    out_state: *mut McLevel1MinimapMarksV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_session(handle, |session| Ok(level1_minimap_marks(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_minimap_entities_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    buffer: *mut McLevel1MinimapEntityCommandV2,
    capacity: u32,
    element_size: u32,
    out_info: *mut McLevel1MinimapEntitiesInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        require_element_size::<McLevel1MinimapEntityCommandV2>(
            element_size,
            "Level 1 minimap entity command",
        )?;
        with_session(handle, |session| {
            validate_mission_snapshot_cursor(session, generation, expected_tick)?;
            let commands = level1_minimap_entity_commands(session);
            let capacity = usize::try_from(capacity).map_err(|_| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    "Level 1 minimap command capacity exceeds usize",
                )
            })?;
            if capacity < commands.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "Level 1 minimap command capacity is smaller than the atomic stream",
                ));
            }
            if !commands.is_empty() && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "Level 1 minimap command buffer is null",
                ));
            }
            for (offset, command) in commands.iter().enumerate() {
                // SAFETY: element size and caller capacity were validated.
                unsafe {
                    buffer.add(offset).write_unaligned(*command);
                }
            }
            Ok(McLevel1MinimapEntitiesInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McLevel1MinimapEntitiesInfoV2>(),
                generation: session.generation,
                tick: session.state.tick,
                total_count: u32::try_from(commands.len())
                    .expect("Level 1 minimap command count fits u32"),
                cross_x: 63,
                cross_y: 64,
                cross_colour: 176,
                cross_arm_length: 10,
                reveal_active: 0,
                authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
                reserved_0: [0; 4],
                command_hash: level1_minimap_command_hash(&commands),
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_palette_copy(
    handle: u64,
    rgb8_buffer: *mut u8,
    rgb8_capacity: u32,
    out_info: *mut McLevel1PaletteInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        with_session(handle, |session| {
            if rgb8_buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "Level 1 effective palette buffer must be non-null",
                ));
            }
            if rgb8_capacity < 768 {
                return Err(AbiError::new(
                    MC_STATUS_BUFFER_TOO_SMALL,
                    format!("Level 1 effective palette requires 768 bytes, got {rgb8_capacity}"),
                ));
            }
            let rgb8 = session.level1_palette_rgb8()?;
            // SAFETY: the caller supplied a non-null buffer with at least the
            // checked 768-byte capacity.
            unsafe {
                rgb8_buffer.copy_from_nonoverlapping(rgb8.as_ptr(), rgb8.len());
            }
            let digest = Sha256::digest(rgb8);
            let mut rgb8_sha256 = [0_u8; 32];
            rgb8_sha256.copy_from_slice(&digest);
            Ok(McLevel1PaletteInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McLevel1PaletteInfoV2>(),
                generation: session.generation,
                tick: session.state.tick,
                submode: session.mission.exit_palette_submode,
                effect: session.mission.exit_palette_effect,
                weight: session.mission.exit_palette_weight,
                denominator: session.mission.exit_palette_denominator,
                palette_count: session.mission.palette_count(),
                flags: 1,
                rgb8_sha256,
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_exit_presentation(
    handle: u64,
    out_state: *mut McLevel1ExitPresentationV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_session(handle, |session| Ok(level1_exit_presentation(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_castle_snapshot(
    handle: u64,
    out_snapshot: *mut McLevel1CastleSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| Ok(level1_castle_snapshot(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_castle_lifecycle_snapshot(
    handle: u64,
    out_snapshot: *mut McLevel1CastleLifecycleSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| {
            Ok(level1_castle_lifecycle_snapshot(session))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_house_completion_effect_count(
    handle: u64,
    out_count: *mut u32,
) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_session(handle, |session| {
            Ok(
                u32::try_from(level1_house_completion_effects(session).len())
                    .expect("completion effect count fits u32"),
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_house_completion_effect_snapshot(
    handle: u64,
    effect_ordinal: u32,
    out_snapshot: *mut McLevel1HouseCompletionEffectV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| {
            level1_house_completion_effects(session)
                .get(usize::try_from(effect_ordinal).expect("u32 ordinal fits usize"))
                .copied()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!(
                            "Level 1 house-completion effect ordinal {effect_ordinal} is out of bounds"
                        ),
                    )
                })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_mana_carrier_snapshot(
    handle: u64,
    out_snapshot: *mut McManaCarrierSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| Ok(mana_carrier_snapshot(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_mana_spheres_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McManaSphereSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McManaSphereSnapshotV2>(element_size, "mana sphere element")?;
        with_session(handle, |session| {
            validate_mission_snapshot_cursor(session, generation, expected_tick)?;
            copy_mission_slice(
                session.mission.mana_spheres(),
                start_ordinal,
                buffer,
                capacity,
                out_written,
                mana_sphere_snapshot,
                "mana sphere",
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_actors_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McLevel1ActorSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McLevel1ActorSnapshotV2>(element_size, "Level 1 actor element")?;
        with_session(handle, |session| {
            validate_mission_snapshot_cursor(session, generation, expected_tick)?;
            copy_mission_slice(
                session.mission.actors(),
                start_ordinal,
                buffer,
                capacity,
                out_written,
                level1_actor_snapshot,
                "Level 1 actor",
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_multipart_segments_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McLevel1MultipartSegmentSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McLevel1MultipartSegmentSnapshotV2>(
            element_size,
            "Level 1 multipart segment element",
        )?;
        with_session(handle, |session| {
            validate_mission_snapshot_cursor(session, generation, expected_tick)?;
            copy_mission_slice(
                session.mission.multipart_segments(),
                start_ordinal,
                buffer,
                capacity,
                out_written,
                level1_multipart_segment_snapshot,
                "Level 1 multipart segment",
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_fireball_projectiles_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McFireballProjectileSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McFireballProjectileSnapshotV2>(
            element_size,
            "Fireball projectile element",
        )?;
        with_session(handle, |session| {
            validate_mission_snapshot_cursor(session, generation, expected_tick)?;
            copy_mission_slice(
                session.mission.projectiles(),
                start_ordinal,
                buffer,
                capacity,
                out_written,
                fireball_projectile_snapshot,
                "Fireball projectile",
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_fireball_impacts_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McFireballImpactSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McFireballImpactSnapshotV2>(
            element_size,
            "Fireball impact element",
        )?;
        with_session(handle, |session| {
            validate_mission_snapshot_cursor(session, generation, expected_tick)?;
            copy_mission_slice(
                session.mission.impacts(),
                start_ordinal,
                buffer,
                capacity,
                out_written,
                fireball_impact_snapshot,
                "Fireball impact",
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_selector_frame_states_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McSelectorFrameStateV2,
    capacity: u32,
    element_size: u32,
    out_batch: *mut McSelectorFrameBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        require_element_size::<McSelectorFrameStateV2>(element_size, "selector-frame element")?;
        with_session(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "selector-frame generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            if expected_tick != session.state.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "selector-frame tick {expected_tick} does not match {}",
                        session.state.tick
                    ),
                ));
            }
            let frames = session.selector_frames.as_ref().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "selector-frame state unavailable without qualified owner cache",
                )
            })?;
            let bank = session.world.goat_tmap_bank().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "selector-frame bank unavailable without qualified owner cache",
                )
            })?;
            let start = usize::try_from(start_ordinal).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "selector-frame start exceeds usize")
            })?;
            if start > GOAT_TMAP_SELECTORS.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "selector-frame start is beyond selector count",
                ));
            }
            let capacity = usize::try_from(capacity).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "selector-frame capacity exceeds usize")
            })?;
            let count = (GOAT_TMAP_SELECTORS.len() - start).min(capacity);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "selector-frame output buffer is null",
                ));
            }
            for (offset, clock) in frames.states().iter().skip(start).take(count).enumerate() {
                // SAFETY: exact element size and writable capacity were
                // supplied for `count` rows.
                unsafe {
                    buffer.add(offset).write_unaligned(selector_frame_state(
                        start + offset,
                        clock,
                        bank,
                    ));
                }
            }
            session.selector_frame_batch(
                start_ordinal,
                u32::try_from(count).expect("three selector frames fit u32"),
            )
        })
    })
}

/// Run the qualified original CPU Goat draw-admission pass for the completed
/// session tick and OR its three-bit result into the selector used latches.
///
/// This function consumes only authoritative Rust simulation, camera-history,
/// terrain-lattice, and owner-cache state.  Managed/GPU visibility is not an
/// input.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_goat_draw_admission_evaluate(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    out_batch: *mut McSelectorFrameBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        with_session_mut(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "Goat draw-admission generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            if expected_tick != session.state.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "Goat draw-admission tick {expected_tick} does not match {}",
                        session.state.tick
                    ),
                ));
            }
            session.evaluate_goat_draw_admission()
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_selector_draw_feedback_submit(
    handle: u64,
    command: *const McSelectorDrawFeedbackV2,
    out_batch: *mut McSelectorFrameBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        let command = read_versioned_input(command, "selector draw feedback")?;
        validate_reserved(&command.reserved, "selector draw feedback")?;
        if command.selector_mask & !u64::from(GOAT_TMAP_DRAW_MASK) != 0 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                format!(
                    "selector draw mask {:#x} has unsupported bits",
                    command.selector_mask
                ),
            ));
        }
        with_session_mut(handle, |session| {
            if command.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "selector draw generation {} does not match {}",
                        command.generation, session.generation
                    ),
                ));
            }
            if command.draw_tick != session.state.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "selector draw tick {} does not match {}",
                        command.draw_tick, session.state.tick
                    ),
                ));
            }
            let mask =
                u8::try_from(command.selector_mask).expect("validated selector draw mask fits u8");
            if let Some((tick, prior_mask)) = session.selector_draw_feedback
                && tick == command.draw_tick
            {
                let combined_mask = prior_mask | mask;
                if combined_mask == prior_mask {
                    let mut receipt = session.selector_frame_batch(0, 0)?;
                    receipt.has_more = 0;
                    return Ok(receipt);
                }
                let frames = session.selector_frames.as_mut().ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "selector-frame state unavailable without qualified owner cache",
                    )
                })?;
                frames
                    .latch_draw_attempts(mask)
                    .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
                session.selector_draw_feedback = Some((command.draw_tick, combined_mask));
                let mut receipt = session.selector_frame_batch(0, 0)?;
                receipt.has_more = 0;
                return Ok(receipt);
            }
            let frames = session.selector_frames.as_mut().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "selector-frame state unavailable without qualified owner cache",
                )
            })?;
            frames
                .latch_draw_attempts(mask)
                .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
            session.selector_draw_feedback = Some((command.draw_tick, mask));
            let mut receipt = session.selector_frame_batch(0, 0)?;
            receipt.has_more = 0;
            Ok(receipt)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_goat_inject_damage(
    handle: u64,
    command: *const McGoatDamageCommandV2,
    out_batch: *mut McGoatBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        let command = read_versioned_input(command, "Goat damage command")?;
        validate_reserved(&command.reserved, "Goat damage command")?;
        if command.reserved_0 != 0 {
            return Err(AbiError::new(
                MC_STATUS_RESERVED_NONZERO,
                "Goat damage command reserved_0 must be zero",
            ));
        }
        with_session_mut(handle, |session| {
            if command.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "Goat damage generation {} does not match {}",
                        command.generation, session.generation
                    ),
                ));
            }
            session
                .live_world
                .inject_damage(
                    command.entity_index,
                    command.damage,
                    session.state.entity_index.cast_signed(),
                )
                .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
            Ok(McGoatBatchV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McGoatBatchV2>(),
                generation: session.generation,
                tick: session.live_world.outer_tick().into(),
                live_world_hash: session.live_world_hash_v2(),
                start_ordinal: 0,
                written: 0,
                total_count: u32::try_from(LEVEL_ONE_GOAT_INDICES.len()).expect("33 Goats fit u32"),
                has_more: 0,
                reserved_0: [0; 3],
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_asset_ref_count(handle: u64, out_count: *mut u32) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_session(handle, |session| {
            Ok(u32::try_from(session.world.asset_refs().len())
                .expect("validated asset count fits u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_asset_ref_info(
    handle: u64,
    asset_index: u32,
    out_info: *mut McAssetRefInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        with_session(handle, |session| {
            let asset = asset_ref(&session.world, asset_index)?;
            Ok(McAssetRefInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McAssetRefInfoV2>(),
                asset_index,
                trace_ordinal: asset.observation.ordinal,
                owner_asset_count: u16::try_from(asset.owner_asset_ids.len()).map_err(|_| {
                    AbiError::new(MC_STATUS_BOUNDS, "owner asset count exceeds u16")
                })?,
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_asset_string_length(
    handle: u64,
    asset_index: u32,
    kind: u32,
    owner_index: u32,
    out_length: *mut u32,
) -> i32 {
    boundary_value(out_length, struct_size::<u32>(), || {
        with_session(handle, |session| {
            let asset = asset_ref(&session.world, asset_index)?;
            string_length(asset_string(asset, kind, owner_index)?)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_asset_string_copy(
    handle: u64,
    asset_index: u32,
    kind: u32,
    owner_index: u32,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_length, "out_length")?;
        with_session(handle, |session| {
            let asset = asset_ref(&session.world, asset_index)?;
            copy_string(
                asset_string(asset, kind, owner_index)?,
                buffer,
                capacity,
                out_length,
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_events_copy_since(
    handle: u64,
    query: *const McEventQueryV2,
    buffer: *mut McEventV2,
    capacity: u32,
    element_size: u32,
    out_batch: *mut McEventBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        require_element_size::<McEventV2>(element_size, "event element")?;
        let query = read_versioned_input(query, "event query")?;
        validate_reserved(&query.reserved, "event query")?;
        with_session(handle, |session| {
            if query.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "event query generation {} does not match {}",
                        query.generation, session.generation
                    ),
                ));
            }
            if query.cursor > session.event_head {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "event cursor is beyond current event head",
                ));
            }
            if let Some(oldest) = session.events.front()
                && query.cursor.saturating_add(1) < oldest.sequence
            {
                return Err(AbiError::new(
                    MC_STATUS_CURSOR_EXPIRED,
                    "event cursor predates the retained journal",
                ));
            }
            let available = session
                .events
                .iter()
                .filter(|event| event.sequence > query.cursor)
                .collect::<Vec<_>>();
            let count = available.len().min(
                usize::try_from(capacity)
                    .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "capacity does not fit usize"))?,
            );
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "event output buffer is null",
                ));
            }
            for (index, event) in available.iter().take(count).enumerate() {
                // SAFETY: buffer is non-null for count records and the caller
                // promises capacity writable records.
                unsafe { buffer.add(index).write_unaligned(**event) };
            }
            let next_cursor = if count == 0 {
                query.cursor
            } else {
                available[count - 1].sequence
            };
            Ok(McEventBatchV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McEventBatchV2>(),
                generation: session.generation,
                next_cursor,
                event_head: session.event_head,
                written: u32::try_from(count).expect("event batch count fits u32"),
                has_more: u8::from(count < available.len()),
                reserved_0: [0; 3],
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_audio_command_for_event(
    handle: u64,
    generation: u64,
    event_sequence: u64,
    out_command: *mut McAudioCommandV2,
    out_command_size: u32,
) -> i32 {
    boundary_value(out_command, out_command_size, || {
        with_session(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "audio command generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            session
                .audio_commands
                .get(&event_sequence)
                .copied()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!("event sequence {event_sequence} has no positional audio command"),
                    )
                })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_audio_command_release(
    handle: u64,
    generation: u64,
    event_sequence: u64,
) -> i32 {
    boundary(|| {
        with_session_mut(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "audio release generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let command = session.audio_commands.get(&event_sequence).ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    format!("event sequence {event_sequence} has no positional audio command"),
                )
            })?;
            if command.action == 0 {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("event sequence {event_sequence} is still pending"),
                ));
            }
            session.audio_commands.remove(&event_sequence);
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_audio_command_discard(
    handle: u64,
    generation: u64,
    event_sequence: u64,
) -> i32 {
    boundary(|| {
        with_session_mut(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "audio discard generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let command = session
                .audio_commands
                .get(&event_sequence)
                .copied()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!("event sequence {event_sequence} has no positional audio command"),
                    )
                })?;
            if command.action == MC_AUDIO_ACTION_PREPARED_PENDING {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("event sequence {event_sequence} is still pending"),
                ));
            }
            if matches!(
                command.action,
                MC_AUDIO_ACTION_START
                    | MC_AUDIO_ACTION_RESTART
                    | MC_AUDIO_ACTION_UPDATE_EXISTING
                    | MC_AUDIO_ACTION_END_LOOP
            ) {
                let index = usize::from(command.voice_index);
                let voice = session.audio_voices.get_mut(index).ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!("audio voice {} is out of bounds", command.voice_index),
                    )
                })?;
                let owns_voice = voice.is_some_and(|voice| {
                    voice.sound_id == command.sound_id
                        && voice.sound_flags == command.sound_flags
                        && if matches!(
                            command.action,
                            MC_AUDIO_ACTION_UPDATE_EXISTING | MC_AUDIO_ACTION_END_LOOP
                        ) {
                            voice.event_sequence <= command.event_sequence
                        } else {
                            voice.event_sequence == command.event_sequence
                        }
                });
                if owns_voice {
                    *voice = None;
                }
            }
            session.audio_commands.remove(&event_sequence);
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_audio_voice_complete(
    handle: u64,
    generation: u64,
    voice_index: u16,
    event_sequence: u64,
) -> i32 {
    boundary(|| {
        with_session_mut(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "audio completion generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let index = usize::from(voice_index);
            let voice = session.audio_voices.get_mut(index).ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    format!("audio voice {voice_index} is out of bounds"),
                )
            })?;
            if !voice.is_some_and(|voice| voice.event_sequence == event_sequence) {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    format!("audio voice {voice_index} does not own event {event_sequence}"),
                ));
            }
            *voice = None;
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub const extern "C" fn mc_native_audio_qualification_vector_count() -> u32 {
    POSITIONAL_QUALIFICATION_VECTOR_COUNT
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_audio_qualification_vector(
    index: u32,
    out_command: *mut McAudioCommandV2,
    out_command_size: u32,
) -> i32 {
    boundary_value(out_command, out_command_size, || {
        positional_qualification_command(index)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_audio_qualification_submit(
    handle: u64,
    event_kind: u32,
    sound_id: u16,
    sound_flags: i16,
    volume_q15: u16,
    pan_q16: u16,
    play_type: u8,
    out_event: *mut McEventV2,
    out_event_size: u32,
    out_command: *mut McAudioCommandV2,
    out_command_size: u32,
) -> i32 {
    boundary(|| {
        require_struct_output::<McEventV2>(out_event, out_event_size, "audio event")?;
        require_struct_output::<McAudioCommandV2>(out_command, out_command_size, "audio command")?;
        if !matches!(event_kind, MC_EVENT_PREPARE_SOUND | MC_EVENT_END_LOOP)
            || !matches!(sound_id, 1 | 2 | 5 | 29 | 31 | 37 | 39 | 43)
            || !matches!(play_type, POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING | 3)
            || volume_q15 > 0x7fff
        {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "qualification audio submission is outside the proven effect contract",
            ));
        }
        let (event, command) = with_session_mut(handle, |session| {
            let mut event = session.next_event_base(event_kind, state_hash(&session.state));
            event.code = u32::from(sound_id);
            event.entity_index = session.state.entity_index;
            event.player_index = -1;
            event.asset_ref_index = session.world.sound_asset_ref(sound_id).ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    format!("sound {sound_id} has no qualified owner asset"),
                )
            })?;
            let mut command = local_audio_command_record(&event, sound_id, sound_flags, play_type);
            command.volume_q15 = volume_q15;
            command.pan_q16 = pan_q16;
            if event_kind == MC_EVENT_END_LOOP {
                let matching = session.audio_voices.iter().position(|voice| {
                    voice.is_some_and(|voice| {
                        voice.sound_id == sound_id && voice.sound_flags == sound_flags
                    })
                });
                if let Some(index) = matching {
                    command.action = MC_AUDIO_ACTION_END_LOOP;
                    command.voice_index = u16::try_from(index).expect("ten voices fit u16");
                } else {
                    command.action = MC_AUDIO_ACTION_SUPPRESS_NO_EXISTING;
                }
            } else {
                let (action, voice_index) = session.allocate_audio_voice(command);
                command.action = if matches!(sound_id, 1 | 2 | 5 | 31)
                    && action == MC_AUDIO_ACTION_SUPPRESS_EXISTING
                {
                    MC_AUDIO_ACTION_UPDATE_EXISTING
                } else {
                    action
                };
                command.voice_index = voice_index;
            }
            session.audio_commands.insert(event.sequence, command);
            session.append_event(event);
            Ok((event, command))
        })?;
        write_output(out_event, event);
        write_output(out_command, command);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_last_error_length(out_length: *mut u32) -> i32 {
    match catch_unwind(AssertUnwindSafe(|| {
        require_output(out_length, "out_length")?;
        LAST_ERROR.with(|error| {
            let length = string_length(&error.borrow())?;
            write_output(out_length, length);
            Ok::<(), AbiError>(())
        })
    })) {
        Ok(Ok(())) => MC_STATUS_OK,
        Ok(Err(error)) => {
            set_last_error(&error.message);
            error.status
        }
        Err(_) => MC_STATUS_INTERNAL_PANIC,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_last_error_copy(
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    match catch_unwind(AssertUnwindSafe(|| {
        let message = LAST_ERROR.with(|error| error.borrow().clone());
        require_output(out_length, "out_length")?;
        copy_string(&message, buffer, capacity, out_length)
    })) {
        Ok(Ok(())) => MC_STATUS_OK,
        // Error retrieval must not destroy the diagnostic being retrieved.
        Ok(Err(error)) => error.status,
        Err(_) => MC_STATUS_INTERNAL_PANIC,
    }
}

fn validate_command(command: &McCommandV2) -> AbiResult<()> {
    validate_reserved(&command.reserved, "command")?;
    if command.requested_spell != MC_REQUESTED_SPELL_NONE
        && !(0..=28).contains(&command.requested_spell)
    {
        return Err(AbiError::new(
            MC_STATUS_BOUNDS,
            "requested spell must be -1 or 0..=28",
        ));
    }
    Ok(())
}

fn validate_embedded_command(command: &McCommandV2) -> AbiResult<()> {
    if command.abi_version != MC_NATIVE_ABI_VERSION {
        return Err(AbiError::new(
            MC_STATUS_UNSUPPORTED_VERSION,
            format!(
                "campaign step command ABI version {} is unsupported",
                command.abi_version
            ),
        ));
    }
    let required = struct_size::<McCommandV2>();
    if command.struct_size < required {
        return Err(AbiError::new(
            MC_STATUS_STRUCT_TOO_SMALL,
            format!(
                "campaign step command struct_size {} is smaller than {required}",
                command.struct_size
            ),
        ));
    }
    validate_command(command)
}

fn campaign_event_snapshot(
    generation: u64,
    ordinal: usize,
    event: &Level2DispatchEvent,
) -> McCampaignEventV2 {
    let (pass, projected_kind, family, flags, source_slot) = event.campaign_projection();
    let kind = match event {
        Level2DispatchEvent::Economy { .. } => MC_CAMPAIGN_EVENT_ECONOMY,
        Level2DispatchEvent::SpellAcquisition { .. } => MC_CAMPAIGN_EVENT_SPELL_ACQUISITION,
        _ => projected_kind,
    };
    McCampaignEventV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignEventV2>(),
        generation,
        ordinal: u32::try_from(ordinal).expect("bounded campaign event ordinal fits u32"),
        pass,
        kind,
        family,
        flags,
        source_slot,
        reserved_0: 0,
        reserved: [0; 4],
    }
}

fn campaign_economy_event_snapshot(
    generation: u64,
    ordinal: usize,
    pass: u8,
    source_slot: u16,
    event: Level2EconomyEvent,
) -> McCampaignEconomyEventV2 {
    let (code, primary_id, secondary_id, entity_0, entity_1, value_0, value_1, flags) = match event
    {
        Level2EconomyEvent::ManaSphereSpawned {
            sphere_id,
            source_entity_index,
            mana,
        } => (
            MC_CAMPAIGN_ECONOMY_MANA_SPHERE_SPAWNED,
            sphere_id,
            0,
            source_entity_index,
            0,
            mana,
            0,
            0,
        ),
        Level2EconomyEvent::ManaSphereClaimed {
            sphere_id,
            owner_entity_index,
        } => (
            MC_CAMPAIGN_ECONOMY_MANA_SPHERE_CLAIMED,
            sphere_id,
            0,
            owner_entity_index,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::ManaSphereCollected {
            sphere_id,
            carrier_id,
            mana,
        } => (
            MC_CAMPAIGN_ECONOMY_MANA_SPHERE_COLLECTED,
            sphere_id,
            carrier_id,
            0,
            0,
            mana,
            0,
            0,
        ),
        Level2EconomyEvent::ManaSpheresMerged {
            target_sphere_id,
            source_sphere_id,
            owner_entity_index,
            mana,
        } => (
            MC_CAMPAIGN_ECONOMY_MANA_SPHERES_MERGED,
            target_sphere_id,
            source_sphere_id,
            owner_entity_index,
            0,
            mana,
            0,
            0,
        ),
        Level2EconomyEvent::ManaCarrierSpawned {
            source_entity_index,
            owner_entity_index,
        } => (
            MC_CAMPAIGN_ECONOMY_MANA_CARRIER_SPAWNED,
            u32::from(source_entity_index),
            0,
            source_entity_index,
            owner_entity_index,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::ManaCarrierArrived {
            carrier_id,
            castle_id,
            mana,
        } => (
            MC_CAMPAIGN_ECONOMY_MANA_CARRIER_ARRIVED,
            carrier_id,
            0,
            castle_id,
            0,
            mana,
            0,
            0,
        ),
        Level2EconomyEvent::CastleDefenderArrived {
            defender_slot,
            castle_id,
            mana,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_ARRIVED,
            u32::from(defender_slot),
            0,
            castle_id,
            0,
            mana,
            0,
            0,
        ),
        Level2EconomyEvent::CastleDefenderCollectedSphere {
            defender_slot,
            sphere_id,
            mana,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_COLLECTED_SPHERE,
            u32::from(defender_slot),
            sphere_id,
            defender_slot,
            0,
            mana,
            0,
            0,
        ),
        Level2EconomyEvent::DeathBurstSpawned {
            source_entity_index,
            parent_entity_index,
            id_entity_index,
        } => (
            MC_CAMPAIGN_ECONOMY_DEATH_BURST_SPAWNED,
            u32::from(source_entity_index),
            0,
            parent_entity_index,
            id_entity_index,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::DeathBurstExpired {
            source_entity_index,
            parent_entity_index,
        } => (
            MC_CAMPAIGN_ECONOMY_DEATH_BURST_EXPIRED,
            u32::from(source_entity_index),
            0,
            parent_entity_index,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::CastleBuilt {
            source_entity_index,
            owner_entity_index,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_BUILT,
            u32::from(source_entity_index),
            0,
            owner_entity_index,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::CastleDestroyed {
            source_entity_index,
            killer_entity_index,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_DESTROYED,
            u32::from(source_entity_index),
            0,
            killer_entity_index,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::CastleConstructionSound {
            source_entity_index,
            sound_id,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_CONSTRUCTION_SOUND,
            u32::from(source_entity_index),
            0,
            sound_id,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::CastleDestructionSound {
            source_entity_index,
            sound_id,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_DESTRUCTION_SOUND,
            u32::from(source_entity_index),
            0,
            sound_id,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::CastleDefenderCaveSound {
            defender_slot,
            sound_id,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_CAVE_SOUND,
            u32::from(defender_slot),
            0,
            sound_id,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::CastleStageDestroyed {
            source_entity_index,
            previous_level,
            next_level,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_STAGE_DESTROYED,
            u32::from(source_entity_index),
            0,
            0,
            0,
            i32::from(previous_level),
            i32::from(next_level),
            0,
        ),
        Level2EconomyEvent::DamageApplied {
            target_entity_index,
            attacker_entity_index,
            damage,
            lethal,
        } => (
            MC_CAMPAIGN_ECONOMY_DAMAGE_APPLIED,
            u32::from(target_entity_index),
            0,
            attacker_entity_index,
            0,
            damage,
            0,
            u8::from(lethal),
        ),
    };
    McCampaignEconomyEventV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignEconomyEventV2>(),
        generation,
        ordinal: u32::try_from(ordinal).expect("bounded campaign event ordinal fits u32"),
        pass,
        family: sim_core::native_world::level2_economy_event_family(event),
        code,
        flags,
        source_slot,
        reserved_0: 0,
        primary_id,
        secondary_id,
        entity_0,
        entity_1,
        value_0,
        value_1,
        reserved: [0; 4],
    }
}

fn decode_sha256_hex(value: &str) -> [u8; 32] {
    let bytes = value.as_bytes();
    let mut digest = [0_u8; 32];
    if bytes.len() != 64 {
        return digest;
    }
    let mut index = 0;
    while index < 32 {
        let high = hex_nibble(bytes[index * 2]);
        let low = hex_nibble(bytes[index * 2 + 1]);
        let Some(high) = high else { return [0; 32] };
        let Some(low) = low else { return [0; 32] };
        digest[index] = (high << 4) | low;
        index += 1;
    }
    digest
}

const fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn validate_camera_input(camera_input: &McCameraInputV2) -> AbiResult<()> {
    validate_reserved(&camera_input.reserved, "camera input")
}

fn validate_reserved(reserved: &[u32], name: &str) -> AbiResult<()> {
    if reserved.iter().any(|value| *value != 0) {
        return Err(AbiError::new(
            MC_STATUS_RESERVED_NONZERO,
            format!("{name} reserved fields must be zero"),
        ));
    }
    Ok(())
}

fn player_command(command: McCommandV2) -> AbiResult<PlayerCommand> {
    let (opcode, argument_1) = if command.requested_spell == MC_REQUESTED_SPELL_NONE {
        (command.opcode, command.argument_1)
    } else {
        (
            6,
            i8::try_from(command.requested_spell)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "requested spell exceeds i8"))?,
        )
    };
    Ok(PlayerCommand {
        opcode,
        argument_1,
        argument_2: command.argument_2,
        roll: command.roll,
        pitch: command.pitch,
        flags: command.flags,
        ..PlayerCommand::default()
    })
}

fn world_string(world: &NativeWorld, kind: u32) -> AbiResult<&str> {
    match kind {
        MC_WORLD_STRING_SCHEMA => Ok("mc2-owner-level-export/v2"),
        MC_WORLD_STRING_PACKAGE_SHA256 => Ok(world.package_sha256()),
        MC_WORLD_STRING_CLOSURE_SHA256 => Ok(world.closure_sha256()),
        MC_WORLD_STRING_STATE_HASH_SCHEMA => Ok("mc2-flight-state-hash/fnv1a64-v1"),
        MC_WORLD_STRING_CONFORMANCE_HASH_SCHEMA => Ok("mc2-flight-conformance-hash/fnv1a64-v1"),
        MC_WORLD_STRING_CONTEXT_SEAM => {
            Ok("Level1 control_mode=0; regression-proven, not serialized by package v2")
        }
        _ => Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!("unknown world string kind {kind}"),
        )),
    }
}

fn validate_plane_kind(planes: &TerrainPlanes, kind: u32) -> AbiResult<()> {
    match kind {
        MC_TERRAIN_TYPES_U8
        | MC_TERRAIN_HEIGHT_U8
        | MC_TERRAIN_SHADE_U8
        | MC_TERRAIN_ANGLE_U8
        | MC_TERRAIN_SECONDARY_U8 => {
            terrain_u8_plane(planes, kind)?;
            Ok(())
        }
        MC_TERRAIN_ENTITY_INDEX_I16 => Ok(()),
        _ => Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!("unknown terrain plane kind {kind}"),
        )),
    }
}

fn terrain_u8_plane(planes: &TerrainPlanes, kind: u32) -> AbiResult<&[u8]> {
    match kind {
        MC_TERRAIN_TYPES_U8 => Ok(&planes.terrain_types),
        MC_TERRAIN_HEIGHT_U8 => Ok(&planes.ground_heights),
        MC_TERRAIN_SHADE_U8 => Ok(&planes.shading),
        MC_TERRAIN_ANGLE_U8 => Ok(&planes.angles),
        MC_TERRAIN_SECONDARY_U8 => Ok(&planes.secondary_heights),
        MC_TERRAIN_ENTITY_INDEX_I16 => Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "entity-index terrain plane requires the i16 copy API",
        )),
        _ => Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!("unknown u8 terrain plane kind {kind}"),
        )),
    }
}

fn entity_snapshot(
    entity: &EntitySnapshot,
    state: &FlightState,
    live_world: &LiveWorldState,
    mission: &Level1MissionState,
) -> McEntitySnapshotV2 {
    let (has_parameter_record, parameter_record_index) = entity
        .parameter_relation
        .map_or((0, MC_PARAMETER_RECORD_NONE), |index| (1, index));
    let mut snapshot = McEntitySnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McEntitySnapshotV2>(),
        entity_index: entity.index,
        parameter_record_index,
        authority: match entity.authority {
            EntityAuthority::Inactive => MC_AUTHORITY_INACTIVE,
            EntityAuthority::FrozenObserved => MC_AUTHORITY_FROZEN_OBSERVED,
            EntityAuthority::PlayerFlightLive => MC_AUTHORITY_PLAYER_FLIGHT_LIVE,
            EntityAuthority::SpellActivationLive => MC_AUTHORITY_SPELL_ACTIVATION_LIVE,
            EntityAuthority::GoatLive => MC_AUTHORITY_GOAT_LIVE,
            EntityAuthority::VillagerLive => MC_AUTHORITY_VILLAGER_LIVE,
        },
        active: u8::from(entity.active),
        class_id: entity.class,
        model: entity.model,
        action: entity.action,
        phase: entity.phase,
        has_parameter_record,
        reserved_0: 0,
        x: entity.position.x,
        y: entity.position.y,
        z: entity.position.z,
        yaw: entity.yaw,
        pitch: entity.pitch,
        fov: entity.fov,
        actual_speed: entity.actual_speed,
        reserved: [0; 4],
    };
    if snapshot.active != 0 && !mission.imported_source_slot_is_live(entity.index) {
        snapshot.authority = MC_AUTHORITY_INACTIVE;
        snapshot.active = 0;
    }
    if entity.authority == EntityAuthority::PlayerFlightLive && entity.index == state.entity_index {
        snapshot.x = state.position.x;
        snapshot.y = state.position.y;
        snapshot.z = state.position.z;
        snapshot.yaw = state.yaw;
        snapshot.pitch = state.display_pitch;
        snapshot.fov = state.entity_fov;
        snapshot.actual_speed = state.actual_speed;
        snapshot.phase = state.entity_phase;
    } else if entity.authority == EntityAuthority::GoatLive
        && let Some(goat) = live_world
            .goats()
            .iter()
            .find(|goat| goat.entity_index == entity.index)
    {
        snapshot.authority = MC_AUTHORITY_GOAT_LIVE;
        snapshot.active = u8::from(goat.active);
        snapshot.action = goat.action;
        snapshot.phase = goat.phase;
        snapshot.x = goat.position.x;
        snapshot.y = goat.position.y;
        snapshot.z = goat.position.z;
        snapshot.yaw = goat.yaw;
        snapshot.pitch = goat.pitch;
        snapshot.actual_speed = goat.actual_speed;
    } else if entity.authority == EntityAuthority::VillagerLive
        && mission.imported_source_slot_is_live(entity.index)
        && let Some(villager) = live_world
            .villagers()
            .iter()
            .find(|villager| villager.entity_index == entity.index)
    {
        snapshot.authority = MC_AUTHORITY_VILLAGER_LIVE;
        snapshot.active = u8::from(villager.active);
        snapshot.action = villager.action;
        snapshot.phase = villager.phase;
        snapshot.x = villager.position.x;
        snapshot.y = villager.position.y;
        snapshot.z = villager.position.z;
        snapshot.yaw = villager.yaw;
        snapshot.pitch = villager.pitch;
        snapshot.actual_speed = villager.actual_speed;
    }
    if let Some(resident) = mission
        .dynamic_residents()
        .find(|resident| resident.source_entity_index == entity.index)
    {
        snapshot.authority = MC_AUTHORITY_LEVEL1_MISSION_LIVE;
        snapshot.active = u8::from(resident.active);
        snapshot.class_id = resident.class;
        snapshot.model = resident.model;
        snapshot.action = resident.action;
        snapshot.phase = resident.phase;
        snapshot.has_parameter_record = 1;
        snapshot.parameter_record_index = match resident.kind {
            sim_core::level1_mission::DynamicResidentKind::Archer => 75,
            sim_core::level1_mission::DynamicResidentKind::Builder => 101,
            sim_core::level1_mission::DynamicResidentKind::Villager
            | sim_core::level1_mission::DynamicResidentKind::Trader => 100,
        };
        snapshot.x = resident.position.x;
        snapshot.y = resident.position.y;
        snapshot.z = resident.position.z;
        snapshot.yaw = resident.yaw;
        snapshot.pitch = 0;
        snapshot.actual_speed = resident.actual_speed;
    }
    if let Some(ambient) = mission
        .ambient_entities()
        .find(|ambient| ambient.source_entity_index == entity.index)
    {
        snapshot.authority = MC_AUTHORITY_LEVEL1_MISSION_LIVE;
        snapshot.active = u8::from(ambient.active);
        snapshot.class_id = ambient.class;
        snapshot.model = ambient.model;
        snapshot.action = ambient.action;
        snapshot.phase = ambient.phase;
        snapshot.has_parameter_record = 1;
        snapshot.parameter_record_index = LEVEL1_NEW_EVENT_ENTITY_PARAMETER_RECORD_INDEX;
        snapshot.x = ambient.position.x;
        snapshot.y = ambient.position.y;
        snapshot.z = ambient.position.z;
        snapshot.yaw = ambient.yaw;
        snapshot.pitch = ambient.pitch;
        snapshot.actual_speed = ambient.actual_speed;
    }
    snapshot
}

fn goat_snapshot(goat: &GoatState) -> McGoatSnapshotV2 {
    McGoatSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McGoatSnapshotV2>(),
        entity_index: goat.entity_index,
        target_entity_index: goat.target_entity_index.unwrap_or(MC_ENTITY_INDEX_NONE),
        render_parameter_index: goat.render_parameter_index,
        base_tmap: GOAT_BASE_TMAP,
        authority: MC_AUTHORITY_GOAT_LIVE,
        active: u8::from(goat.active),
        class_id: GOAT_CLASS,
        model: GOAT_MODEL,
        action: goat.action,
        phase: goat.phase,
        stage_var_1: goat.stage_var_1,
        stage_var_2: goat.stage_var_2,
        proximity_countdown: goat.proximity_countdown,
        proximity_subcounter: goat.proximity_subcounter,
        animation_frame: goat.animation_frame,
        reserved_0: 0,
        x: goat.position.x,
        y: goat.position.y,
        z: goat.position.z,
        yaw: goat.yaw,
        pitch: goat.pitch,
        roll: goat.roll,
        actual_speed: goat.actual_speed,
        minimum_speed: goat.minimum_speed,
        maximum_speed: goat.maximum_speed,
        rng: goat.rng,
        life: goat.life,
        max_life: goat.max_life,
        mana: goat.mana,
        pending_damage: goat.pending_damage,
        control_flags: goat.control_flags,
        reserved: [0; 4],
    }
}

fn level1_mission_snapshot(session: &SessionRecord) -> McLevel1MissionSnapshotV2 {
    let mission = &session.mission;
    let possessed_buildings = || {
        mission
            .possessed_buildings()
            .iter()
            .filter(|building| building.active && building.owner_entity_index != 0)
    };
    McLevel1MissionSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1MissionSnapshotV2>(),
        generation: session.generation,
        tick: session.state.tick,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        current_stage: mission.current_stage,
        text_selector: mission.current_text_selector,
        objective_textbox_ticks: mission.objective_textbox_ticks,
        tasks_completed: u8::from(mission.level_completed),
        player_dead: u8::from(mission.player_dead()),
        has_fireball: u8::from(mission.has_fireball),
        has_possession: u8::from(mission.has_possession),
        has_castle_spell: u8::from(mission.has_castle_spell),
        has_speed_spell: u8::from(mission.has_speed_spell),
        castle_exists: u8::from(mission.castle_exists),
        castle_level: mission.castle_level,
        fidelity_gap_flags: MC_LEVEL1_CURRENT_FIDELITY_GAPS,
        exit_sequence_active: u8::from(mission.exit_sequence_active),
        exit_sequence_complete: u8::from(mission.exit_sequence_complete),
        exit_result_code: mission.exit_result_code,
        player_life: mission.player_life,
        player_max_life: mission.player_max_life,
        player_mana: mission.player_mana,
        player_max_mana: mission.player_max_mana,
        castle_mana: mission.castle_mana,
        secured_mana_percent: mission.secured_mana_percent(),
        actor_count: u32::try_from(mission.actors().len()).expect("Level 1 actor count fits u32"),
        projectile_count: u32::try_from(mission.projectiles().len())
            .expect("Level 1 projectile count fits u32"),
        impact_count: u32::try_from(mission.impacts().len())
            .expect("Level 1 impact count fits u32"),
        exit_sequence_phase: u32::from(mission.exit_sequence_phase),
        exit_sequence_timer: i32::from(mission.exit_sequence_timer),
        possession_experience: mission.possession_experience,
        last_possessed_building_entity: possessed_buildings()
            .last()
            .map_or(0, |building| building.entity_index),
        last_possessed_building_owner: possessed_buildings()
            .last()
            .map_or(0, |building| building.owner_entity_index),
        possessed_building_count: u16::try_from(possessed_buildings().count())
            .expect("Level 1 possessed building count fits u16"),
        possession_tier: u8::try_from(mission.possession_tier)
            .expect("Level 1 possession tier is non-negative"),
        speed_trail_count: u8::try_from(mission.speed_trails().len()).unwrap_or(u8::MAX),
    }
}

fn level1_player_lifecycle_snapshot(session: &SessionRecord) -> McLevel1PlayerLifecycleSnapshotV2 {
    let mission = &session.mission;
    let marker = mission.player_death_marker();
    let player_entity_index = session.state.entity_index;
    let player_control_flags = mission.player_source_control_flags(player_entity_index);
    McLevel1PlayerLifecycleSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1PlayerLifecycleSnapshotV2>(),
        generation: session.generation,
        tick: session.state.tick,
        lifecycle_hash: mission.player_lifecycle_hash_v2(&session.state, &session.context),
        source_arena_hash: mission.source_arena_hash_v1(),
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        player_action: mission.player_action,
        player_dead: u8::from(mission.player_dead()),
        input_suppressed: u8::from(
            mission.player_action != sim_core::level1_mission::PLAYER_ACTION_ACTIVE,
        ),
        player_active: u8::from(mission.player_source_active(player_entity_index)),
        player_visible: u8::from(player_control_flags & 0x20 == 0),
        player_targetable: u8::from(
            mission.player_source_active(player_entity_index) && mission.player_life >= 0,
        ),
        menu_close_requested: u8::from(mission.player_menu_close_requested),
        notification_active: u8::from(mission.player_notification_active),
        notification_state: mission.player_notification_state,
        banished_camera_active: u8::from(mission.player_banished_camera_active),
        marker_present: u8::from(marker.is_some_and(|marker| marker.active)),
        fall_velocity: mission.player_fall_velocity,
        notification_timer: mission.player_notification_timer,
        action_timer: mission.player_action_timer,
        replaceable_cursor: mission.source_pool_replaceable_cursor(),
        killer_entity_index: mission.player_killer_entity_index(),
        killer_score_awards: mission.player_score_for(
            mission.player_killer_entity_index(),
            session.state.player_color_index,
        ),
        notification_id: mission.player_notification_id,
        player_entity_rng: mission.player_source_entity_rng(player_entity_index),
        player_control_flags,
        player_entity_roll: mission.player_entity_roll,
        player_yaw: session.state.yaw,
        controller_pitch: session.state.pitch_accumulator,
        controller_roll: session.state.roll_accumulator,
        recoil_speed: session.state.recoil_speed,
        marker_source_entity_index: marker.map_or(0, |marker| marker.source_entity_index),
        marker_x: marker.map_or(0, |marker| marker.position.x),
        marker_y: marker.map_or(0, |marker| marker.position.y),
        marker_z: marker.map_or(0, |marker| marker.position.z),
        marker_trigger_entity_index: marker.map_or(0, |marker| marker.trigger_player_entity_index),
        marker_particle_parameter_index: marker.map_or(0, |marker| marker.particle_parameter_index),
        marker_life: marker.map_or(0, |marker| marker.life),
        marker_max_life: marker.map_or(0, |marker| marker.max_life),
        source_pool_rebuild_count: mission.source_pool_rebuild_count(),
        lifecycle_trace_hash: mission.player_lifecycle_trace_hash(),
        lifecycle_trace_count: u16::try_from(mission.player_lifecycle_trace().len())
            .unwrap_or(u16::MAX),
        marker_dispatch_phase: marker.map_or(0, |marker| marker.dispatch_phase),
        marker_class: marker.map_or(0, |_| sim_core::level1_mission::PLAYER_DEATH_MARKER_CLASS),
        marker_model: marker.map_or(0, |_| sim_core::level1_mission::PLAYER_DEATH_MARKER_MODEL),
        marker_action: marker.map_or(0, |_| sim_core::level1_mission::PLAYER_DEATH_MARKER_ACTION),
        marker_byte_38: marker.map_or(0, |marker| marker.byte_38),
        source_color_phase_6: u8::from(mission.source_color_phase_6()),
        source_turn_counter: mission.source_turn_counter(),
        menu_state: mission.player_menu_state,
        killer_score_victim_color: session.state.player_color_index.min(7),
        effective_pitch: session.state.effective_pitch,
        marker_constructor_phase: marker.map_or(0, |marker| marker.constructor_phase),
        reserved: [0; 4],
    }
}

fn level1_hud_spell_slot(session: &SessionRecord, holder_index: u16) -> McLevel1HudSpellSlotV2 {
    let Some(fire) = session.context.spell_fire.as_ref() else {
        return McLevel1HudSpellSlotV2::default();
    };
    let Some(holder) = fire
        .holders
        .iter()
        .find(|holder| holder.entity_index == holder_index)
    else {
        return McLevel1HudSpellSlotV2::default();
    };
    let enabled_flags = if matches!(holder.model, 3 | 4 | 6 | 8 | 11 | 12 | 14) {
        4
    } else {
        0
    };
    McLevel1HudSpellSlotV2 {
        holder_entity_index: holder.entity_index,
        parent_entity_index: fire.caster.entity_index,
        model: holder.model,
        // DrawSpellIcon_2E260 reads the shared holder level gate, not the
        // selected tier stored independently for each hand.
        tier: holder.level_gate,
        enabled_flags,
        action_marker: holder.action_marker,
        active_value: holder.active_value,
        activation_value: holder.activation_value,
        max_mana: holder.mana_cost,
        // All four holders reachable in Level 1 have zero source manaRegen.
        mana_regen: 0,
        caster_mana: fire.caster.mana,
        frame_selector: if holder.active_value == 0 { 1 } else { 2 },
        icon_selector: 123 + u16::from(holder.model),
        reserved: [0; 2],
    }
}

fn level1_hud_snapshot(session: &SessionRecord) -> McLevel1HudSnapshotV2 {
    let mission = &session.mission;
    let fire = session.context.spell_fire.as_ref();
    let left_index = fire.map_or(0, |fire| fire.left_holder_index);
    let right_index = fire.map_or(0, |fire| fire.right_holder_index);
    let selected_index = fire.map_or(0, |fire| fire.selected_holder_index);
    let selected = level1_hud_spell_slot(session, selected_index);
    let player_color = usize::from(session.state.player_color_index.min(7));
    const NIGHT_COLORS: [[u8; 2]; 8] = [
        [0xA4, 0xAA],
        [0x77, 0x7D],
        [0xC0, 0xC6],
        [0x58, 0x5D],
        [0x97, 0x9D],
        [0xD7, 0xDD],
        [0x69, 0x62],
        [0xC9, 0xCF],
    ];
    McLevel1HudSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1HudSnapshotV2>(),
        generation: session.generation,
        tick: session.state.tick,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        mana_color_index: NIGHT_COLORS[player_color][0],
        mana_max_color_index: NIGHT_COLORS[player_color][1],
        life_color_index: 0x7B,
        color_phase_1: u8::from((session.state.tick & 1) != 0),
        color_phase_2: u8::from(((session.state.tick / 2) & 1) != 0),
        player_hit_frame_ticks: mission.player_hit_frame_ticks(),
        castle_hit_frame_ticks: mission.castle_hit_frame_ticks(),
        global_mana_denominator: mission.global_mana_denominator,
        secured_mana_component: mission.secured_mana_component,
        goal_percent: mission.secured_mana_percent(),
        player_status_flags: session
            .context
            .spell_fire
            .as_ref()
            .map_or(0, |fire| fire.caster.control_flags),
        left: level1_hud_spell_slot(session, left_index),
        right: level1_hud_spell_slot(session, right_index),
        selected_holder_entity_index: selected.holder_entity_index,
        selected_icon_selector: selected.icon_selector,
        selected_model: selected.model,
        selected_tier: selected.tier,
        selected_enabled_flags: selected.enabled_flags,
        selected_frame_selector: u8::try_from(selected.frame_selector)
            .expect("spell frame selector is 1 or 2"),
    }
}

fn level1_spell_menu_snapshot(session: &SessionRecord) -> McLevel1SpellMenuSnapshotV2 {
    let Some(fire) = session.context.spell_fire.as_ref() else {
        return McLevel1SpellMenuSnapshotV2::default();
    };
    let holder_model = |holder_index| {
        fire.holders
            .iter()
            .find(|holder| holder.entity_index == holder_index)
            .map_or(0, |holder| holder.model)
    };
    let mut family_enabled = [0_u8; 4];
    let mut family_unlocked_tier = [0_i8; 4];
    let mut family_selected_tier = [0_i8; 4];
    let mut holder_entity_index = [0_u16; 4];
    for family in 0..4 {
        let holder_index = fire.enabled_holder_indices[family];
        holder_entity_index[family] = holder_index;
        family_enabled[family] = u8::from(holder_index != 0);
        family_unlocked_tier[family] = fire.spell_levels[family].clamp(0, 2);
        family_selected_tier[family] = fire.family_selected_tiers[family];
    }
    McLevel1SpellMenuSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1SpellMenuSnapshotV2>(),
        generation: session.generation,
        tick: session.state.tick,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        selected_ui_index: fire.selected_spell_ui_index,
        selected_tier: fire.selected_subspell_index,
        left_family: holder_model(fire.left_holder_index),
        left_tier: fire.left_subspell_index,
        right_family: holder_model(fire.right_holder_index),
        right_tier: fire.right_subspell_index,
        reserved_0: 0,
        family_enabled,
        family_unlocked_tier,
        family_selected_tier,
        family_hand_eligibility: std::array::from_fn(|family| fire.spell_hand_eligibility[family]),
        holder_entity_index,
        reserved: [0; 2],
    }
}

fn level1_minimap_marks(session: &SessionRecord) -> McLevel1MinimapMarksV2 {
    let Level1MinimapMarks {
        mark_count,
        mark_pulse,
        arrow_valid,
        arrow_visible,
        mark_x,
        mark_y,
        mark_colour,
        mark_help_selector,
        arrow_xy,
    } = session.mission.minimap_marks(&session.state);
    McLevel1MinimapMarksV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1MinimapMarksV2>(),
        generation: session.generation,
        tick: session.state.tick,
        mark_count,
        mark_pulse: u8::from(mark_pulse),
        arrow_valid: u8::from(arrow_valid),
        arrow_visible: u8::from(arrow_visible),
        mark_x,
        mark_y,
        mark_colour,
        mark_help_selector,
        arrow_xy,
        reserved: [0; 4],
    }
}

fn level1_exit_presentation(session: &SessionRecord) -> McLevel1ExitPresentationV2 {
    let mission = &session.mission;
    McLevel1ExitPresentationV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1ExitPresentationV2>(),
        generation: session.generation,
        tick: session.state.tick,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        player_action: mission.exit_player_action,
        action_phase: mission.exit_sequence_phase,
        palette_submode: mission.exit_palette_submode,
        palette_effect: mission.exit_palette_effect,
        palette_weight: mission.exit_palette_weight,
        palette_denominator: mission.exit_palette_denominator,
        movement_resolution: mission.exit_last_movement_resolution,
        target_source_entity_index: mission.exit_target_source_entity_index,
        target_valid: u8::from(mission.exit_target_valid),
        input_suppressed: u8::from(mission.exit_sequence_owns_player_action()),
        camera_fov: session.camera_fov,
        action_timer: mission.exit_sequence_timer,
        player_status_flags: mission.exit_player_status_flags,
        blur_mode: mission.exit_blur_mode,
        display_blank: u8::from(mission.exit_display_blank),
        result_code: mission.exit_result_code,
        active: u8::from(mission.exit_sequence_active),
        complete: u8::from(mission.exit_sequence_complete),
        flags: MC_LEVEL1_EXIT_FLAG_PALETTE_VISUAL_CURRENT
            | MC_LEVEL1_EXIT_FLAG_FOV_POST_ACTION_NEXT_HISTORY
            | if mission.exit_end_sequence_flag {
                MC_LEVEL1_EXIT_FLAG_END_SEQUENCE
            } else {
                0
            },
        reserved_0: 0,
        reserved: [0; 4],
    }
}

fn record_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn level1_minimap_entity_inputs(session: &SessionRecord) -> [Level1MinimapEntityInput; 1_000] {
    const SOURCE_EXTENT: usize = 1_000;
    let local_player = session.state.entity_index;
    let mut inputs = [Level1MinimapEntityInput::default(); SOURCE_EXTENT];
    let relation_is_player = |entity_index: u16| {
        session
            .world
            .entities()
            .get(usize::from(entity_index))
            .is_some_and(|entity| entity.active && entity.class == 3)
    };

    for source_slot in 1..SOURCE_EXTENT {
        let slot = session.mission.source_slots()[source_slot];
        let reset = &session.world.entities()[source_slot];
        let bytes = reset.record_bytes();
        let source_id = reset.source_id;
        let player_link = record_u16(bytes, 0x94);
        inputs[source_slot] = Level1MinimapEntityInput {
            source_slot: source_slot as u16,
            class: slot.class,
            model: slot.model,
            action: slot.action,
            phase: slot.phase,
            control_flags: slot.control_flags,
            source_id,
            source_id_is_player: relation_is_player(source_id),
            source_id_is_local_player: source_id == local_player,
            player_link,
            player_link_is_player: relation_is_player(player_link),
            player_link_is_local_player: player_link == local_player,
            parent_id: record_u16(bytes, 0x28),
            stage_var_2: bytes[0x49] as i8,
            word_32: record_u16(bytes, 0x32),
            position: reset.position,
            active: slot.active,
        };
    }

    let mut resolve = |source_slot: u16,
                       position: Position,
                       source_id: u16,
                       player_link: u16,
                       parent_id: u16,
                       stage_var_2: i8| {
        if source_slot == 0 || usize::from(source_slot) >= SOURCE_EXTENT {
            return;
        }
        let input = &mut inputs[usize::from(source_slot)];
        input.position = position;
        input.source_id = source_id;
        input.source_id_is_player = relation_is_player(source_id);
        input.source_id_is_local_player = source_id == local_player;
        input.player_link = player_link;
        input.player_link_is_player = relation_is_player(player_link);
        input.player_link_is_local_player = player_link == local_player;
        input.parent_id = parent_id;
        input.stage_var_2 = stage_var_2;
    };

    resolve(
        local_player,
        session.state.position,
        local_player,
        local_player,
        0,
        0,
    );
    for goat in session.live_world.goats() {
        resolve(
            goat.entity_index,
            goat.position,
            goat.source_id,
            0,
            0,
            goat.stage_var_2,
        );
    }
    for actor in session.mission.actors() {
        resolve(
            actor.source_entity_index,
            actor.position,
            actor.owner_id.cast_unsigned(),
            0,
            0,
            actor.stage_var_2,
        );
    }
    for segment in session.mission.multipart_segments() {
        resolve(
            segment.source_entity_index,
            segment.position,
            segment.owner_id.cast_unsigned(),
            0,
            segment.head_source_entity_index,
            0,
        );
    }
    for projectile in session.mission.projectiles() {
        match projectile {
            Level1Projectile::PlayerFireball(projectile) => resolve(
                projectile.source_entity_index,
                projectile.position,
                projectile.owner_entity_index,
                0,
                0,
                0,
            ),
            Level1Projectile::ArcherArrow(projectile) => resolve(
                projectile.source_entity_index,
                projectile.position,
                projectile.owner_entity_index,
                projectile.target_entity_index,
                0,
                0,
            ),
            Level1Projectile::FireflyFireball(projectile) => resolve(
                projectile.source_entity_index,
                projectile.position,
                projectile.owner_entity_index,
                projectile.target_entity_index,
                0,
                0,
            ),
            Level1Projectile::CastleBuild(projectile) => resolve(
                projectile.source_entity_index,
                projectile.position,
                projectile.owner_entity_index,
                projectile.owner_entity_index,
                0,
                0,
            ),
            Level1Projectile::Possession(projectile) => resolve(
                projectile.source_entity_index,
                projectile.position,
                projectile.owner_entity_index,
                projectile.owner_entity_index,
                0,
                0,
            ),
        }
    }
    for impact in session.mission.impacts() {
        resolve(
            impact.source_entity_index,
            impact.position,
            impact.owner_entity_index,
            impact.external_target_entity_index,
            0,
            0,
        );
    }
    for burst in session.mission.death_bursts() {
        resolve(
            burst.source_entity_index,
            burst.position,
            burst.parent_entity_index,
            0,
            burst.parent_entity_index,
            0,
        );
    }
    for sphere in session.mission.mana_spheres() {
        resolve(
            sphere.source_entity_index,
            sphere.position,
            sphere.source_entity_index,
            sphere.owner_entity_index,
            0,
            0,
        );
    }
    if let Some(carrier) = session.mission.mana_carrier() {
        resolve(
            carrier.source_entity_index,
            carrier.position,
            carrier.owner_entity_index,
            carrier.owner_entity_index,
            0,
            0,
        );
    }
    if session.mission.castle_exists {
        resolve(
            session.mission.castle_source_entity_index,
            session.mission.castle_position,
            session.mission.castle_owner_entity_index,
            session.mission.castle_owner_entity_index,
            0,
            0,
        );
    }
    if let Some(construction) = session.mission.castle_construction() {
        if construction.active {
            resolve(
                construction.source_entity_index,
                construction.position,
                construction.parent_entity_index,
                construction.owner_entity_index,
                construction.parent_entity_index,
                0,
            );
        }
    }
    inputs
}

fn level1_minimap_entity_commands(session: &SessionRecord) -> Vec<McLevel1MinimapEntityCommandV2> {
    let inputs = level1_minimap_entity_inputs(session);
    let mut commands = Vec::with_capacity(LEVEL1_MINIMAP_MAX_ENTITY_COMMANDS);
    if session.mission.castle_exists {
        for (order, (x, y)) in
            level1_minimap_castle_rope_pixels(&session.state, session.mission.castle_position)
                .into_iter()
                .enumerate()
        {
            commands.push(McLevel1MinimapEntityCommandV2 {
                source_slot: session.mission.castle_source_entity_index,
                x,
                y,
                kind: LEVEL1_MINIMAP_COMMAND_BLEND_PIXEL,
                palette_index: 176,
                anchor: sim_core::level1_mission::LEVEL1_MINIMAP_ANCHOR_TOP_LEFT,
                order: u16::try_from(order).expect("castle rope command order fits u16"),
                ..McLevel1MinimapEntityCommandV2::default()
            });
        }
    }
    for input in inputs.into_iter().skip(1) {
        let Some(command) = level1_minimap_entity_command(&session.state, input) else {
            continue;
        };
        commands.push(McLevel1MinimapEntityCommandV2 {
            source_slot: command.source_slot,
            x: command.x,
            y: command.y,
            selector: command.selector,
            kind: command.kind,
            palette_index: command.palette_index,
            size: command.size,
            anchor: command.anchor,
            help_selector: command.help_selector,
            flags: 0x07,
            order: u16::try_from(commands.len()).expect("minimap command order fits u16"),
            reserved: [0; 2],
        });
    }
    assert!(
        commands.len() <= LEVEL1_MINIMAP_MAX_ENTITY_COMMANDS,
        "Level 1 minimap command extent"
    );
    commands
}

fn level1_map_entity_commands(session: &SessionRecord) -> Vec<Level1MinimapEntityCommand> {
    let inputs = level1_minimap_entity_inputs(session);
    let mut commands = Vec::with_capacity(LEVEL1_MINIMAP_MAX_ENTITY_COMMANDS);
    if session.mission.castle_exists {
        commands.extend(
            level1_map_castle_rope_pixels(&session.state, session.mission.castle_position)
                .into_iter()
                .map(|(x, y)| Level1MinimapEntityCommand {
                    source_slot: session.mission.castle_source_entity_index,
                    x,
                    y,
                    selector: 0,
                    kind: LEVEL1_MINIMAP_COMMAND_BLEND_PIXEL,
                    palette_index: 176,
                    size: 1,
                    anchor: sim_core::level1_mission::LEVEL1_MINIMAP_ANCHOR_TOP_LEFT,
                    help_selector: 0,
                }),
        );
    }
    commands.extend(
        inputs
            .into_iter()
            .skip(1)
            .filter_map(|input| level1_map_entity_command(&session.state, input)),
    );
    assert!(
        commands.len() <= LEVEL1_MINIMAP_MAX_ENTITY_COMMANDS,
        "Level 1 map entity command extent"
    );
    commands
}

fn level1_minimap_command_hash(commands: &[McLevel1MinimapEntityCommandV2]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut append = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    append(b"mc2-level1-minimap-entities-v1\0");
    append(&(commands.len() as u32).to_le_bytes());
    for command in commands {
        append(&command.source_slot.to_le_bytes());
        append(&command.x.to_le_bytes());
        append(&command.y.to_le_bytes());
        append(&command.selector.to_le_bytes());
        append(&[
            command.kind,
            command.palette_index,
            command.size,
            command.anchor,
            command.help_selector,
            command.flags,
        ]);
        append(&command.order.to_le_bytes());
    }
    hash
}

fn level1_castle_snapshot(session: &SessionRecord) -> McLevel1CastleSnapshotV2 {
    let mission = &session.mission;
    let carrier = mission.mana_carrier();
    McLevel1CastleSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1CastleSnapshotV2>(),
        generation: session.generation,
        tick: session.state.tick,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        active: u8::from(mission.castle_exists),
        level: mission.castle_level,
        class_id: CASTLE_ROOT_CLASS,
        model: CASTLE_ROOT_MODEL,
        action: CASTLE_ROOT_ACTION,
        // TMAP 96 header `03 00` supplies the runtime mode.
        render_mode: 0,
        reserved_0: 0,
        owner_entity_index: carrier.map_or(0, |carrier| carrier.owner_entity_index),
        render_parameter_index: CASTLE_ROOT_RENDER_INDEX,
        x: mission.castle_position.x,
        y: mission.castle_position.y,
        z: mission.castle_position.z,
        base_tmap: CASTLE_ROOT_BASE_TMAP,
        logical_height: CASTLE_ROOT_LOGICAL_HEIGHT,
        reserved_1: 0,
        life: mission.castle_life,
        max_life: mission.castle_max_life,
        mana: mission.castle_mana,
        mana_capacity: mission.castle_mana_capacity,
        carrier_id: carrier.map_or(0, |carrier| carrier.id),
        reserved: [
            u32::from(mission.castle_source_entity_index),
            mission.castle_presentation_id,
            0,
        ],
    }
}

fn terrain_state(session: &SessionRecord) -> McTerrainStateV2 {
    McTerrainStateV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McTerrainStateV2>(),
        generation: session.generation,
        tick: session.state.tick,
        mutation_sequence: session.terrain.mutation_sequence(),
        terrain_hash: session.terrain.deterministic_hash(),
        width: 256,
        height: 256,
        plane_count: 6,
        hash_schema: 1,
        authority: MC_AUTHORITY_SESSION_TERRAIN_LIVE,
        reserved_0: 0,
        reserved: [0; 4],
    }
}

fn level1_castle_lifecycle_snapshot(session: &SessionRecord) -> McLevel1CastleLifecycleSnapshotV2 {
    let mission = &session.mission;
    let construction = mission.castle_construction();
    McLevel1CastleLifecycleSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1CastleLifecycleSnapshotV2>(),
        generation: session.generation,
        tick: session.state.tick,
        root_presentation_id: mission.castle_presentation_id,
        root_source_entity_index: mission.castle_source_entity_index,
        root_owner_entity_index: mission.castle_owner_entity_index,
        root_x: mission.castle_position.x,
        root_y: mission.castle_position.y,
        root_z: mission.castle_position.z,
        root_build_state: mission.castle_build_state,
        root_action: mission.castle_action,
        root_level: mission.castle_level,
        root_active: u8::from(mission.castle_exists),
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        root_life: mission.castle_life,
        construction_id: construction.map_or(0, |child| child.id),
        construction_source_entity_index: construction.map_or(0, |child| child.source_entity_index),
        construction_parent_entity_index: construction.map_or(0, |child| child.parent_entity_index),
        construction_owner_entity_index: construction.map_or(0, |child| child.owner_entity_index),
        construction_x: construction.map_or(0, |child| child.position.x),
        construction_y: construction.map_or(0, |child| child.position.y),
        construction_z: construction.map_or(0, |child| child.position.z),
        construction_timer: construction.map_or(0, |child| child.timer),
        construction_stage: construction.map_or(0, |child| child.stage),
        construction_present: u8::from(construction.is_some()),
        construction_active: u8::from(construction.is_some_and(|child| child.active)),
        construction_initialized: u8::from(construction.is_some_and(|child| child.initialized)),
        reserved_0: [0; 2],
        terrain_mutation_sequence: session.terrain.mutation_sequence(),
    }
}

fn level1_house_completion_effects(
    session: &SessionRecord,
) -> Vec<McLevel1HouseCompletionEffectV2> {
    let completion_effects = session
        .mission
        .house_construction_effects()
        .iter()
        .copied()
        .filter(|effect| {
            matches!(
                effect,
                HouseConstructionSourceEffect::ConstructionAllocated { .. }
                    | HouseConstructionSourceEffect::ConstructionAllocationFailed { .. }
            )
        })
        .collect::<Vec<_>>();
    let effect_count =
        u32::try_from(completion_effects.len()).expect("completion effect count fits u32");
    completion_effects
        .into_iter()
        .enumerate()
        .map(|(ordinal, effect)| {
            let (kind, house_entity_index, parent_entity_index, source_entity_index, selector) =
                match effect {
                    HouseConstructionSourceEffect::ConstructionAllocated {
                        house_entity_index,
                        parent_entity_index,
                        source_entity_index,
                        building_index,
                    } => (
                        MC_LEVEL1_HOUSE_COMPLETION_ALLOCATED,
                        house_entity_index,
                        parent_entity_index,
                        source_entity_index,
                        building_index,
                    ),
                    HouseConstructionSourceEffect::ConstructionAllocationFailed {
                        house_entity_index,
                        parent_entity_index,
                        building_index,
                    } => (
                        MC_LEVEL1_HOUSE_COMPLETION_ALLOCATION_FAILED,
                        house_entity_index,
                        parent_entity_index,
                        0,
                        building_index,
                    ),
                    _ => unreachable!("filtered completion effect"),
                };
            let construction = (source_entity_index != 0)
                .then(|| {
                    session
                        .mission
                        .house_completion_constructions()
                        .iter()
                        .find(|construction| {
                            construction.source_entity_index == source_entity_index
                        })
                })
                .flatten();
            let source = (source_entity_index != 0)
                .then(|| {
                    session
                        .mission
                        .source_slots()
                        .get(usize::from(source_entity_index))
                })
                .flatten();
            let parent_word_2e = session
                .mission
                .raw_house_construction_state(parent_entity_index)
                .map_or(0, |state| state.4);
            let parent_construction_selector = session
                .mission
                .raw_source_construction_selector(parent_entity_index)
                .unwrap_or_default();
            let house_building_index = session
                .live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == house_entity_index)
                .map(|house| house.building_index)
                .or_else(|| {
                    session
                        .mission
                        .authored_houses()
                        .find(|house| house.entity_index == house_entity_index)
                        .map(|house| house.building_index)
                })
                .unwrap_or_default();
            McLevel1HouseCompletionEffectV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McLevel1HouseCompletionEffectV2>(),
                generation: session.generation,
                tick: session.state.tick,
                kind,
                house_entity_index,
                parent_entity_index,
                source_entity_index,
                source_id: construction.map_or(0, |value| value.owner_entity_index),
                class_id: source.map_or(0, |value| value.class),
                model: source.map_or(0, |value| value.model),
                action: source.map_or(0, |value| value.action),
                construction_selector: selector,
                house_building_index,
                constructor_latch: construction.map_or(0, |value| value.constructor_latch),
                active: u8::from(construction.is_some_and(|value| value.active)),
                phase: source.map_or(0, |value| value.phase),
                parent_word_2e,
                parent_construction_selector,
                x: construction.map_or(0, |value| value.position.x),
                y: construction.map_or(0, |value| value.position.y),
                z: construction.map_or(0, |value| value.position.z),
                collision_half_width: construction.map_or(0, |value| value.collision_half_width),
                collision_half_depth: construction.map_or(0, |value| value.collision_half_depth),
                yaw: construction.map_or(0, |value| value.yaw),
                fov: construction.map_or(0, |value| value.fov),
                effect_ordinal: u16::try_from(ordinal).expect("completion effect ordinal fits u16"),
                max_life: construction.map_or(0, |value| value.max_life),
                life: construction.map_or(0, |value| value.life),
                control_flags: construction.map_or(0, |value| value.control_flags),
                effect_count,
            }
        })
        .collect()
}

fn mana_sphere_snapshot(sphere: &ManaSphere) -> McManaSphereSnapshotV2 {
    let size = usize::from(mana_sphere_size_ordinal(sphere.mana));
    let render_base = if sphere.owner_entity_index == 0 {
        MANA_SPHERE_NEUTRAL_RENDER_BASE
    } else {
        MANA_SPHERE_PLAYER_ONE_RENDER_BASE
    };
    McManaSphereSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McManaSphereSnapshotV2>(),
        sphere_id: sphere.id,
        source_entity_index: sphere.source_entity_index,
        owner_entity_index: sphere.owner_entity_index,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        active: u8::from(sphere.active && !sphere.pending_delete),
        class_id: MANA_SPHERE_CLASS,
        model: MANA_SPHERE_MODEL,
        action: MANA_SPHERE_ACTION,
        phase: sphere.phase,
        // TMAPs 58/67 both carry runtime mode 21.
        render_mode: 21,
        reserved_0: 0,
        x: sphere.position.x,
        y: sphere.position.y,
        z: sphere.position.z,
        yaw: sphere.yaw,
        velocity_x: sphere.velocity_x,
        velocity_y: sphere.velocity_y,
        vertical_velocity: sphere.vertical_velocity,
        render_parameter_index: render_base
            + u16::try_from(size).expect("eight mana sphere sizes fit u16"),
        base_tmap: if sphere.owner_entity_index == 0 {
            MANA_SPHERE_NEUTRAL_BASE_TMAP
        } else {
            MANA_SPHERE_PLAYER_ONE_BASE_TMAP
        },
        logical_height: u16::try_from((size + 1) * 50)
            .expect("mana sphere logical height fits u16"),
        mana: sphere.mana,
        assigned_carrier_id: sphere.assigned_carrier_id,
        reserved: [0; 3],
    }
}

fn mana_carrier_snapshot(session: &SessionRecord) -> McManaCarrierSnapshotV2 {
    let carrier = session
        .mission
        .mana_carrier()
        .copied()
        .unwrap_or(ManaCarrier {
            id: 0,
            source_entity_index: 0,
            entity_rng: 0,
            owner_entity_index: 0,
            position: Position::default(),
            yaw: 0,
            pitch: 0,
            speed: MANA_CARRIER_SPEED,
            life: 0,
            max_life: MANA_CARRIER_MAX_LIFE,
            max_mana: MANA_CARRIER_MAX_MANA,
            target_sphere_id: 0,
            carried_mana: 0,
            returning_to_castle: false,
            pending_damage: 0,
            pending_attacker_entity_index: 0,
            active: false,
        });
    mana_carrier_snapshot_value(&carrier, session.generation, session.state.tick)
}

fn mana_carrier_snapshot_value(
    carrier: &ManaCarrier,
    generation: u64,
    tick: u64,
) -> McManaCarrierSnapshotV2 {
    McManaCarrierSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McManaCarrierSnapshotV2>(),
        generation,
        tick,
        carrier_id: carrier.id,
        target_sphere_id: carrier.target_sphere_id,
        owner_entity_index: carrier.owner_entity_index,
        render_parameter_index: MANA_CARRIER_RENDER_INDEX,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        active: u8::from(carrier.active),
        class_id: MANA_CARRIER_CLASS,
        model: MANA_CARRIER_MODEL,
        action: MANA_CARRIER_ACTION,
        returning_to_castle: u8::from(carrier.returning_to_castle),
        // TMAP 88 header `02 00` supplies the runtime mode. This family is
        // fixed-facing; it does not use Firefly's mode-17 yaw selection.
        render_mode: 0,
        reserved_0: 0,
        x: carrier.position.x,
        y: carrier.position.y,
        z: carrier.position.z,
        yaw: carrier.yaw,
        speed: carrier.speed,
        base_tmap: MANA_CARRIER_BASE_TMAP,
        logical_height: MANA_CARRIER_LOGICAL_HEIGHT,
        reserved_1: 0,
        carried_mana: carrier.carried_mana,
        // Carrier identity and every carrier/sphere link are source slots.
        // Keep the redundant reserved projection until the next ABI version
        // so existing desktop clients can enforce exact equality.
        reserved: [u32::from(carrier.source_entity_index), 0, 0],
    }
}

fn level1_actor_snapshot(actor: &Level1Actor) -> McLevel1ActorSnapshotV2 {
    McLevel1ActorSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1ActorSnapshotV2>(),
        authored_index: actor.authored_index,
        activation_group: actor.activation_group,
        kind: actor.kind as u8,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        active: u8::from(actor.active),
        collected: u8::from(actor.collected),
        class_id: actor.class,
        model: actor.model,
        action: actor.action,
        render_mode: actor.render_mode.unwrap_or(0),
        x: actor.position.x,
        y: actor.position.y,
        z: actor.position.z,
        render_parameter_index: actor.render_parameter_index.unwrap_or(u16::MAX),
        max_life: actor.max_life,
        life: actor.life,
        pending_damage: actor.pending_damage,
        base_tmap: actor.base_tmap.unwrap_or(u16::MAX),
        logical_height: actor.logical_height.unwrap_or(0),
        yaw: actor.yaw,
        animation_frame: actor.animation_frame,
        visible: u8::from(actor.visible),
        state_value: actor.state_value,
        reserved: 0,
    }
}

fn level1_multipart_segment_snapshot(
    segment: &Level1MultipartSegment,
) -> McLevel1MultipartSegmentSnapshotV2 {
    McLevel1MultipartSegmentSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1MultipartSegmentSnapshotV2>(),
        source_entity_index: segment.source_entity_index,
        head_authored_index: segment.head_authored_index,
        head_source_entity_index: segment.head_source_entity_index,
        predecessor_source_entity_index: segment.predecessor_source_entity_index,
        successor_source_entity_index: segment.successor_source_entity_index,
        ordinal: segment.ordinal,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        active: u8::from(segment.active),
        visible: u8::from(segment.visible),
        class_id: segment.class,
        model: segment.model,
        action: segment.action,
        render_mode: segment.render_mode,
        x: segment.position.x,
        y: segment.position.y,
        z: segment.position.z,
        yaw: segment.yaw,
        pitch: segment.pitch,
        phase: segment.phase,
        reserved_0: 0,
        owner_id: segment.owner_id,
        life: segment.life,
        pending_damage: segment.pending_damage,
        render_parameter_index: segment.render_parameter_index,
        base_tmap: segment.base_tmap,
        logical_height: segment.logical_height,
        collision_half_width: segment.collision_half_width,
        collision_half_height: segment.collision_half_height,
        trailing_distance: segment.trailing_distance,
        reserved: 0,
    }
}

fn fireball_projectile_snapshot(projectile: &Level1Projectile) -> McFireballProjectileSnapshotV2 {
    let (
        projectile_kind,
        id,
        source_entity_index,
        owner_entity_index,
        render_index,
        class_id,
        model,
        position,
        yaw,
        pitch,
        speed,
        life_ticks,
        base_tmap,
        payload,
        logical_height,
        render_mode,
        active,
    ) = match projectile {
        Level1Projectile::PlayerFireball(projectile) => (
            1,
            projectile.id,
            projectile.source_entity_index,
            projectile.owner_entity_index,
            FIREBALL_RENDER_INDEX,
            FIREBALL_PROJECTILE_CLASS,
            FIREBALL_PROJECTILE_MODEL,
            projectile.position,
            projectile.yaw,
            projectile.pitch,
            projectile.speed,
            projectile.life_ticks,
            FIREBALL_BASE_TMAP,
            projectile.payload,
            FIREBALL_LOGICAL_HEIGHT,
            FIREBALL_RENDER_MODE,
            projectile.active,
        ),
        Level1Projectile::ArcherArrow(projectile) => (
            2,
            projectile.id,
            projectile.source_entity_index,
            projectile.owner_entity_index,
            ARCHER_ARROW_RENDER_INDEX,
            ARCHER_ARROW_PROJECTILE_CLASS,
            ARCHER_ARROW_PROJECTILE_MODEL,
            projectile.position,
            projectile.yaw,
            projectile.pitch,
            projectile.speed,
            projectile.life_ticks,
            ARCHER_ARROW_BASE_TMAP,
            projectile.payload,
            ARCHER_ARROW_LOGICAL_HEIGHT,
            ARCHER_ARROW_RENDER_MODE,
            projectile.active,
        ),
        Level1Projectile::FireflyFireball(projectile) => (
            3,
            projectile.id,
            projectile.source_entity_index,
            projectile.owner_entity_index,
            FIREBALL_RENDER_INDEX,
            FIREBALL_PROJECTILE_CLASS,
            FIREBALL_PROJECTILE_MODEL,
            projectile.position,
            projectile.yaw,
            projectile.pitch,
            projectile.speed,
            projectile.life_ticks,
            FIREBALL_BASE_TMAP,
            projectile.payload,
            FIREBALL_LOGICAL_HEIGHT,
            FIREBALL_RENDER_MODE,
            projectile.active,
        ),
        Level1Projectile::CastleBuild(projectile) => (
            4,
            projectile.id,
            projectile.source_entity_index,
            projectile.owner_entity_index,
            CASTLE_PROJECTILE_RENDER_INDEX,
            CASTLE_PROJECTILE_CLASS,
            CASTLE_PROJECTILE_MODEL,
            projectile.position,
            projectile.yaw,
            projectile.pitch,
            projectile.speed,
            projectile.life_ticks,
            CASTLE_PROJECTILE_BASE_TMAP,
            0,
            CASTLE_PROJECTILE_LOGICAL_HEIGHT,
            CASTLE_PROJECTILE_RENDER_MODE,
            projectile.active,
        ),
        Level1Projectile::Possession(projectile) => (
            5,
            projectile.id,
            projectile.source_entity_index,
            projectile.owner_entity_index,
            POSSESSION_PROJECTILE_RENDER_INDEX,
            POSSESSION_PROJECTILE_CLASS,
            POSSESSION_PROJECTILE_MODEL,
            projectile.position,
            projectile.yaw,
            projectile.pitch,
            projectile.speed,
            projectile.life_ticks,
            POSSESSION_PROJECTILE_BASE_TMAP,
            0,
            POSSESSION_PROJECTILE_LOGICAL_HEIGHT,
            POSSESSION_PROJECTILE_RENDER_MODE,
            projectile.active,
        ),
    };
    McFireballProjectileSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McFireballProjectileSnapshotV2>(),
        projectile_id: id,
        owner_entity_index,
        render_index,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        active: u8::from(active),
        class_id,
        model,
        x: position.x,
        y: position.y,
        z: position.z,
        yaw,
        pitch,
        speed,
        life_ticks,
        base_tmap,
        payload,
        logical_height,
        render_mode,
        animation_frame: 0,
        projectile_kind,
        // Reserved word 0 carries the authoritative remc2 source slot while
        // `projectile_id` remains a presentation-stable identifier.
        reserved: [u32::from(source_entity_index), 0],
    }
}

fn fireball_impact_snapshot(impact: &FireballImpact) -> McFireballImpactSnapshotV2 {
    let (base_tmap, render_parameter_index, render_mode, logical_height) =
        if impact.model == WATER_SPLASH_MODEL {
            (
                WATER_SPLASH_BASE_TMAP,
                WATER_SPLASH_RENDER_INDEX,
                WATER_SPLASH_RENDER_MODE,
                WATER_SPLASH_LOGICAL_HEIGHT,
            )
        } else if impact.model == GROUND_FIRE_MODEL {
            let (render_parameter_index, logical_height) = ground_fire_presentation(impact.phase);
            (
                GROUND_FIRE_BASE_TMAP,
                render_parameter_index,
                GROUND_FIRE_RENDER_MODE,
                logical_height,
            )
        } else if impact.model == POSSESSION_ATTRACTION_MODEL {
            (
                POSSESSION_ATTRACTION_BASE_TMAP,
                POSSESSION_ATTRACTION_RENDER_INDEX,
                POSSESSION_ATTRACTION_RENDER_MODE,
                POSSESSION_ATTRACTION_LOGICAL_HEIGHT,
            )
        } else if matches!(
            impact.model,
            POSSESSION_IMPACT_MODEL | POSSESSION_STRONG_IMPACT_MODEL
        ) {
            (
                POSSESSION_IMPACT_BASE_TMAP,
                POSSESSION_IMPACT_RENDER_INDEX,
                POSSESSION_IMPACT_RENDER_MODE,
                POSSESSION_IMPACT_LOGICAL_HEIGHT,
            )
        } else if impact.model == CHECKPOINT_SMOKE_MODEL {
            let render_parameter_index = u16::from(impact.animation_frame);
            (
                CHECKPOINT_SMOKE_BASE_TMAP,
                render_parameter_index,
                CHECKPOINT_SMOKE_RENDER_MODE,
                render_parameter_index
                    .saturating_sub(CHECKPOINT_SMOKE_FIRST_RENDER_INDEX)
                    .saturating_add(1)
                    * 50,
            )
        } else {
            (
                FIREBALL_IMPACT_BASE_TMAP,
                FIREBALL_IMPACT_RENDER_INDEX,
                FIREBALL_IMPACT_RENDER_MODE,
                FIREBALL_IMPACT_LOGICAL_HEIGHT,
            )
        };
    McFireballImpactSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McFireballImpactSnapshotV2>(),
        impact_id: impact.id,
        source_projectile_id: impact.source_projectile_id,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        active: u8::from(impact.active),
        damage_submitted: u8::from(impact.damage_submitted),
        class_id: if matches!(
            impact.model,
            POSSESSION_IMPACT_MODEL | POSSESSION_STRONG_IMPACT_MODEL
        ) {
            POSSESSION_IMPACT_CLASS
        } else if impact.model == CHECKPOINT_SMOKE_MODEL {
            CHECKPOINT_SMOKE_CLASS
        } else {
            FIREBALL_IMPACT_CLASS
        },
        model: impact.model,
        render_mode,
        animation_frame: i8::try_from(impact.animation_frame).unwrap_or(i8::MAX),
        reserved_0: 0,
        x: impact.position.x,
        y: impact.position.y,
        z: impact.position.z,
        life_ticks: impact.life_ticks,
        payload: impact.payload,
        base_tmap,
        render_parameter_index,
        logical_height,
        reserved_1: 0,
        // Reserved word 0 carries the authoritative remc2 source slot while
        // `impact_id` remains a presentation-stable identifier.
        reserved: [u32::from(impact.source_entity_index), 0],
    }
}

fn validate_mission_snapshot_cursor(
    session: &SessionRecord,
    generation: u64,
    expected_tick: u64,
) -> AbiResult<()> {
    if generation != session.generation {
        return Err(AbiError::new(
            MC_STATUS_GENERATION_MISMATCH,
            format!(
                "Level 1 mission generation {generation} does not match {}",
                session.generation
            ),
        ));
    }
    if expected_tick != session.state.tick {
        return Err(AbiError::new(
            MC_STATUS_GENERATION_MISMATCH,
            format!(
                "Level 1 mission tick {expected_tick} does not match {}",
                session.state.tick
            ),
        ));
    }
    Ok(())
}

fn copy_mission_slice<T, U>(
    source: &[T],
    start_ordinal: u32,
    buffer: *mut U,
    capacity: u32,
    out_written: *mut u32,
    convert: impl Fn(&T) -> U,
    label: &str,
) -> AbiResult<()> {
    let start = usize::try_from(start_ordinal)
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, format!("{label} start exceeds usize")))?;
    if start > source.len() {
        return Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!("{label} start is beyond count"),
        ));
    }
    let capacity = usize::try_from(capacity)
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, format!("{label} capacity exceeds usize")))?;
    let count = (source.len() - start).min(capacity);
    if count > 0 && buffer.is_null() {
        return Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            format!("{label} output buffer is null"),
        ));
    }
    for (offset, item) in source.iter().skip(start).take(count).enumerate() {
        // SAFETY: the caller supplied exact element stride and writable
        // capacity for `count` records.
        unsafe { buffer.add(offset).write_unaligned(convert(item)) };
    }
    write_output(
        out_written,
        u32::try_from(count).expect("mission slice count fits u32"),
    );
    Ok(())
}

fn objective_snapshot(
    slot: u8,
    objective: &sim_core::native_world::RuntimeObjective,
) -> McObjectiveSnapshotV2 {
    McObjectiveSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McObjectiveSnapshotV2>(),
        slot,
        is_level_end: objective.is_level_end,
        text_selector: objective.text_selector,
        done: objective.done,
        authority: MC_AUTHORITY_FROZEN_OBSERVED,
        stage_selectors: objective.stage_selectors,
        reserved_0: [0; 3],
        reserved: [0; 4],
    }
}

fn campaign_player_snapshot(
    player: &sim_core::native_world::PlayerSnapshot,
    active: bool,
) -> McCampaignPlayerSnapshotV2 {
    McCampaignPlayerSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignPlayerSnapshotV2>(),
        slot: player.slot,
        is_ai: u8::from(player.is_ai),
        active: u8::from(active),
        reserved_0: 0,
        entity_index: player.entity_index,
        active_player_index: player.active_player_index,
        turn: player.turn,
        left_spell_slot_raw: player.left_spell_slot_raw,
        right_spell_slot_raw: player.right_spell_slot_raw,
        left_subspell_index: player.left_subspell_index,
        right_subspell_index: player.right_subspell_index,
        selected_spell_ui_index: player.selected_spell_ui_index,
        selected_subspell_index: player.selected_subspell_index,
        reserved_1: [0; 2],
        reserved: [0; 4],
    }
}

fn campaign_entity_snapshot(entity: &EntitySnapshot) -> McCampaignEntitySnapshotV2 {
    McCampaignEntitySnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignEntitySnapshotV2>(),
        entity_index: entity.index,
        source_id: entity.source_id,
        next_relation: entity.next_relation.unwrap_or(u16::MAX),
        parameter_relation: entity.parameter_relation.unwrap_or(u16::MAX),
        authority: match entity.authority {
            EntityAuthority::Inactive => MC_AUTHORITY_INACTIVE,
            EntityAuthority::FrozenObserved => MC_AUTHORITY_FROZEN_OBSERVED,
            EntityAuthority::PlayerFlightLive => MC_AUTHORITY_PLAYER_FLIGHT_LIVE,
            EntityAuthority::SpellActivationLive => MC_AUTHORITY_SPELL_ACTIVATION_LIVE,
            EntityAuthority::GoatLive => MC_AUTHORITY_GOAT_LIVE,
            EntityAuthority::VillagerLive => MC_AUTHORITY_VILLAGER_LIVE,
        },
        active: u8::from(entity.active),
        class_id: entity.class,
        model: entity.model,
        action: entity.action,
        phase: entity.phase,
        x: entity.position.x,
        y: entity.position.y,
        z: entity.position.z,
        yaw: entity.yaw,
        pitch: entity.pitch,
        fov: entity.fov,
        actual_speed: entity.actual_speed,
        reserved: [0; 4],
    }
}

fn asset_ref(world: &NativeWorld, index: u32) -> AbiResult<&AssetRef> {
    let index = usize::try_from(index)
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "asset index does not fit usize"))?;
    world.asset_refs().get(index).ok_or_else(|| {
        AbiError::new(
            MC_STATUS_BOUNDS,
            format!("asset index {index} is out of bounds"),
        )
    })
}

fn asset_string(asset: &AssetRef, kind: u32, owner_index: u32) -> AbiResult<&str> {
    match kind {
        MC_ASSET_STRING_ID => Ok(&asset.asset_id),
        MC_ASSET_STRING_CONTENT_ADDRESS => Ok(&asset.content_address),
        MC_ASSET_STRING_CATEGORY => Ok(&asset.observation.category),
        MC_ASSET_STRING_HOOK => Ok(&asset.observation.hook),
        MC_ASSET_STRING_STAGE => Ok(asset.observation.stage.as_deref().unwrap_or("")),
        MC_ASSET_STRING_OWNER_ID => {
            let owner_index = usize::try_from(owner_index)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "owner index does not fit usize"))?;
            asset
                .owner_asset_ids
                .get(owner_index)
                .map(String::as_str)
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!("owner asset index {owner_index} is out of bounds"),
                    )
                })
        }
        _ => Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!("unknown asset string kind {kind}"),
        )),
    }
}

fn resolution_code(evidence: &StepEvidence) -> u32 {
    match evidence.movement_resolution {
        "accepted" => 0,
        "redirected" => 1,
        "rejected" => 2,
        "rejected-and-stopped" => 3,
        "recovered-after-reject" => 4,
        "skipped" => 5,
        _ => u32::MAX,
    }
}

fn profile_code(cave_mode: bool) -> u8 {
    if cave_mode {
        MC_PROFILE_CAVE
    } else {
        MC_PROFILE_OUTDOOR
    }
}

fn next_nonzero(value: u64) -> u64 {
    let next = value.wrapping_add(1);
    if next == 0 { 1 } else { next }
}

fn load_optional_terrain_frame_assets() -> AbiResult<Option<TerrainFrameAssets>> {
    let Some(root) = std::env::var_os("MC_ASSET_CACHE_PATH") else {
        return Ok(None);
    };
    load_terrain_frame_assets(&PathBuf::from(root)).map(Some)
}

fn load_terrain_frame_assets(root: &Path) -> AbiResult<TerrainFrameAssets> {
    let terrain_atlas = read_qualified_cache_file(
        root,
        "terrain/bl32n.atlas.indices.u8",
        LEVEL1_TERRAIN_ATLAS_BYTES,
        TERRAIN_ATLAS_SHA256,
    )?;
    let tablesn = read_qualified_cache_file(
        root,
        "palette/tablesn.u8",
        sim_core::mode26_raster::NIGHT_TABLES_BYTES,
        TABLESN_SHA256,
    )?;
    let sky =
        read_qualified_cache_file(root, "sky/skyn.indices.u8", LEVEL1_SKY_BYTES, SKYN_SHA256)?;
    let palette_rgb6 = read_qualified_cache_file(root, "palette/paln.rgb6", 768, PALN_RGB6_SHA256)?;
    let archer_tmap_bank = load_qualified_archer_tmap_bank(root)?;
    let firefly_tmap_bank = load_qualified_firefly_tmap_bank(root)?;
    let frozen_actor_tmap_bank = load_qualified_frozen_actor_tmap_bank(root)?;
    let combat_tmap_bank = load_qualified_combat_tmap_bank(root)?;
    let static_world_tmap_bank = load_qualified_static_world_tmap_bank(root)?;
    let economy_tmap_bank = load_qualified_economy_tmap_bank(root)?;
    let indexed_overlays = IndexedOverlayAssets::load(root)
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))?;
    Ok(TerrainFrameAssets {
        terrain_atlas: terrain_atlas.into_boxed_slice(),
        tablesn: tablesn.into_boxed_slice(),
        sky: sky.into_boxed_slice(),
        palette_rgb6: palette_rgb6.into_boxed_slice(),
        archer_tmap_bank,
        firefly_tmap_bank,
        frozen_actor_tmap_bank,
        combat_tmap_bank,
        static_world_tmap_bank,
        economy_tmap_bank,
        indexed_overlays,
    })
}

fn load_qualified_frozen_actor_tmap_bank(root: &Path) -> AbiResult<FrozenActorTmapBank> {
    let dolmen = read_qualified_cache_file(
        root,
        "terrain/tmaps1/079.decoded.bin",
        DOLMEN_PAYLOAD_BYTES,
        DOLMEN_PAYLOAD_SHA256,
    )?;
    let payloads = VILLAGER_ANIMATED_SELECTORS
        .iter()
        .enumerate()
        .map(|(ordinal, selector)| {
            read_qualified_cache_file(
                root,
                &format!("terrain/tmaps1/{selector:03}.decoded.bin"),
                VILLAGER_PAYLOAD_BYTES[ordinal],
                VILLAGER_PAYLOAD_SHA256[ordinal],
            )
        })
        .collect::<AbiResult<Vec<_>>>()?;
    FrozenActorTmapBank::decode_qualified(
        &dolmen,
        std::array::from_fn(|ordinal| payloads[ordinal].as_slice()),
    )
    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))
}

fn load_qualified_economy_tmap_bank(root: &Path) -> AbiResult<EconomyTmapBank> {
    let neutral_mana = read_qualified_cache_file(
        root,
        "terrain/tmaps1/058.decoded.bin",
        MANA_SPHERE_NEUTRAL_PAYLOAD_BYTES,
        MANA_SPHERE_NEUTRAL_PAYLOAD_SHA256,
    )?;
    let player_one_mana = read_qualified_cache_file(
        root,
        "terrain/tmaps1/067.decoded.bin",
        MANA_SPHERE_PLAYER_ONE_PAYLOAD_BYTES,
        MANA_SPHERE_PLAYER_ONE_PAYLOAD_SHA256,
    )?;
    let carrier = read_qualified_cache_file(
        root,
        "terrain/tmaps1/088.decoded.bin",
        MANA_CARRIER_PAYLOAD_BYTES,
        MANA_CARRIER_PAYLOAD_SHA256,
    )?;
    let castle = read_qualified_cache_file(
        root,
        "terrain/tmaps1/096.decoded.bin",
        CASTLE_ROOT_PAYLOAD_BYTES,
        CASTLE_ROOT_PAYLOAD_SHA256,
    )?;
    EconomyTmapBank::decode_qualified(&neutral_mana, &player_one_mana, &carrier, &castle)
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))
}

fn load_qualified_static_world_tmap_bank(root: &Path) -> AbiResult<StaticWorldTmapBank> {
    let spell_jar = read_qualified_cache_file(
        root,
        "terrain/tmaps1/059.decoded.bin",
        SPELL_JAR_PAYLOAD_BYTES,
        SPELL_JAR_PAYLOAD_SHA256,
    )?;
    let exit_target = read_qualified_cache_file(
        root,
        "terrain/tmaps1/461.decoded.bin",
        EXIT_TARGET_PAYLOAD_BYTES,
        EXIT_TARGET_PAYLOAD_SHA256,
    )?;
    let secondary = [30_u16, 31, 32, 33, 34, 35, 36, 37, 66, 259]
        .map(|selector| {
            let path = root.join(format!("terrain/tmaps1/{selector:03}.decoded.bin"));
            fs::read(&path).map_err(|error| {
                AbiError::new(
                    MC_STATUS_IO_ERROR,
                    format!("failed reading {}: {error}", path.display()),
                )
            })
        })
        .into_iter()
        .collect::<AbiResult<Vec<_>>>()?;
    let scenery = LEVEL1_SCENERY_SELECTORS
        .map(|selector| {
            let path = root.join(format!("terrain/tmaps1/{selector:03}.decoded.bin"));
            fs::read(&path).map_err(|error| {
                AbiError::new(
                    MC_STATUS_IO_ERROR,
                    format!("failed reading {}: {error}", path.display()),
                )
            })
        })
        .into_iter()
        .collect::<AbiResult<Vec<_>>>()?;
    let ambient = LEVEL1_AMBIENT_SELECTORS
        .map(|selector| {
            let path = root.join(format!("terrain/tmaps1/{selector:03}.decoded.bin"));
            fs::read(&path).map_err(|error| {
                AbiError::new(
                    MC_STATUS_IO_ERROR,
                    format!("failed reading {}: {error}", path.display()),
                )
            })
        })
        .into_iter()
        .collect::<AbiResult<Vec<_>>>()?;
    StaticWorldTmapBank::decode_with_level1_secondary_scenery_and_ambient(
        &spell_jar,
        &exit_target,
        std::array::from_fn(|index| secondary[index].as_slice()),
        std::array::from_fn(|index| scenery[index].as_slice()),
        std::array::from_fn(|index| ambient[index].as_slice()),
    )
    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))
}

fn load_qualified_combat_tmap_bank(root: &Path) -> AbiResult<CombatTmapBank> {
    let fireball = read_qualified_cache_file(
        root,
        "terrain/tmaps1/463.decoded.bin",
        4_346,
        FIREBALL_PAYLOAD_SHA256,
    )?;
    let possession = read_qualified_cache_file(
        root,
        "terrain/tmaps1/144.decoded.bin",
        7_186,
        POSSESSION_PAYLOAD_SHA256,
    )?;
    let mut static_pixels = Vec::with_capacity(COMBAT_STATIC_SELECTORS.len());
    for ordinal in 0..COMBAT_STATIC_SELECTORS.len() {
        let selector = COMBAT_STATIC_SELECTORS[ordinal];
        let (width, height) = COMBAT_STATIC_EXTENTS[ordinal];
        static_pixels.push(read_qualified_cache_file(
            root,
            &format!("terrain/tmaps1/{selector:03}.indices.u8"),
            usize::from(width) * usize::from(height),
            COMBAT_STATIC_SHA256[ordinal],
        )?);
    }
    let references: Vec<&[u8]> = static_pixels.iter().map(Vec::as_slice).collect();
    let references: [&[u8]; COMBAT_STATIC_SELECTORS.len()] = references
        .try_into()
        .map_err(|_| AbiError::new(MC_STATUS_VALIDATION_FAILED, "combat TMAP count changed"))?;
    CombatTmapBank::decode_qualified(&fireball, &possession, references)
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))
}

fn load_qualified_archer_tmap_bank(root: &Path) -> AbiResult<ArcherTmapBank> {
    let archer_animated = [
        (
            "terrain/tmaps1/008.decoded.bin",
            6_300,
            "084b65352a3123fa3d5dd8fb344ba56bb4e78f387b1026dd3cdfe590b56c0691",
        ),
        (
            "terrain/tmaps1/009.decoded.bin",
            6_958,
            "96747ee124997f694b384bdd1acf7d896f609ef5c5ef6f1eca9fa9875c57c4f1",
        ),
        (
            "terrain/tmaps1/010.decoded.bin",
            6_048,
            "f07373bd03faec45604b1ad16ae016c005cec11584d39fa76a4769e04c940cf7",
        ),
    ]
    .map(|(path, bytes, hash)| read_qualified_cache_file(&root, path, bytes, hash))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    let archer_static = [
        (
            "terrain/tmaps1/011.indices.u8",
            1_938,
            "e41cac6d975a7cd02e50a1f63aba2ab51fbf54587f2ddb5af89e9d4257385a1a",
        ),
        (
            "terrain/tmaps1/012.indices.u8",
            2_090,
            "82ad42cd719d21c2422632a44361b6c918a805efbd4bca8bffc998099048f77a",
        ),
        (
            "terrain/tmaps1/013.indices.u8",
            1_760,
            "d81fe313bcc95ea110fee6bfd4e0716a8eef91737d964be3dee173292f87b44e",
        ),
        (
            "terrain/tmaps1/121.indices.u8",
            1_440,
            "da5633de0e9f4a0591c5f11a6648bb0f6c5caf1b586ed1b13b11d00480c35501",
        ),
        (
            "terrain/tmaps1/122.indices.u8",
            1_710,
            "7b0a2db4ae60b748857a356deed89a6698950371ad564fc5b51ff1985da58fc9",
        ),
        (
            "terrain/tmaps1/123.indices.u8",
            1_440,
            "ce6eb45e6e8d07c45de984c1cb78d574a5ae2f4357df0318f3f3ce9bf37bd3df",
        ),
    ]
    .map(|(path, bytes, hash)| read_qualified_cache_file(&root, path, bytes, hash))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    let archer_tmap_bank = ArcherTmapBank::decode_qualified(
        [
            &archer_animated[0],
            &archer_animated[1],
            &archer_animated[2],
        ],
        [
            &archer_static[0],
            &archer_static[1],
            &archer_static[2],
            &archer_static[3],
            &archer_static[4],
            &archer_static[5],
        ],
    )
    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
    Ok(archer_tmap_bank)
}

fn load_qualified_firefly_tmap_bank(root: &Path) -> AbiResult<FireflyTmapBank> {
    let payloads = [
        (
            "terrain/tmaps1/279.decoded.bin",
            8_378,
            "d4ffec8ae18edb62f68441ecd29cf375b6873af3cf98a780bc3539f11b5b1387",
        ),
        (
            "terrain/tmaps1/280.decoded.bin",
            9_828,
            "c334d13b8adf0d274452643760d58a2bcb5527241cd5d8a9ed0ae3550a77b7c9",
        ),
        (
            "terrain/tmaps1/281.decoded.bin",
            10_290,
            "fbfb64916d1d8e7b3c460b0235d47f227603e434d8b6bed808d2d4079986a9e7",
        ),
        (
            "terrain/tmaps1/282.decoded.bin",
            10_292,
            "f91014fba453529488ca0e49f3643622eb8485c1e2d45e631a8bfd1d18d0c720",
        ),
        (
            "terrain/tmaps1/283.decoded.bin",
            10_902,
            "3566b01e55bc04862eb40cfd20150e7eb03cf04f842d37078a245521d225df8d",
        ),
        (
            "terrain/tmaps1/284.decoded.bin",
            11_018,
            "8f75ffc75929f75364445a32c59e04a9b335e3f4d9d96388a106939987f0b4d0",
        ),
        (
            "terrain/tmaps1/285.decoded.bin",
            11_344,
            "ae00740ed6c6d188d369aad1ef95d062ec3d301ecdf7f0413d8b24c9086c367b",
        ),
        (
            "terrain/tmaps1/286.decoded.bin",
            11_294,
            "8095182d4842a4b04f2f02e3937225599bcd2fc50d73ef0ea1170024d5b6b283",
        ),
    ]
    .map(|(path, bytes, hash)| read_qualified_cache_file(root, path, bytes, hash))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    FireflyTmapBank::decode_qualified([
        &payloads[0],
        &payloads[1],
        &payloads[2],
        &payloads[3],
        &payloads[4],
        &payloads[5],
        &payloads[6],
        &payloads[7],
    ])
    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))
}

fn read_qualified_cache_file(
    root: &Path,
    relative: &str,
    expected_len: usize,
    expected_sha256: &str,
) -> AbiResult<Vec<u8>> {
    let canonical_root = fs::canonicalize(root).map_err(|error| {
        AbiError::new(
            MC_STATUS_IO_ERROR,
            format!(
                "cannot resolve owner asset cache {}: {error}",
                root.display()
            ),
        )
    })?;
    if fs::symlink_metadata(&canonical_root)
        .map_err(|error| {
            AbiError::new(
                MC_STATUS_IO_ERROR,
                format!(
                    "cannot inspect owner asset cache {}: {error}",
                    canonical_root.display()
                ),
            )
        })?
        .file_type()
        .is_symlink()
    {
        return Err(AbiError::new(
            MC_STATUS_VALIDATION_FAILED,
            "owner asset cache root must not be a symlink",
        ));
    }
    let candidate = canonical_root.join(relative);
    let canonical_candidate = fs::canonicalize(&candidate).map_err(|error| {
        AbiError::new(
            MC_STATUS_IO_ERROR,
            format!("cannot resolve owner cache file {relative}: {error}"),
        )
    })?;
    if !canonical_candidate.starts_with(&canonical_root)
        || fs::symlink_metadata(&candidate)
            .map_err(|error| {
                AbiError::new(
                    MC_STATUS_IO_ERROR,
                    format!("cannot inspect owner cache file {relative}: {error}"),
                )
            })?
            .file_type()
            .is_symlink()
    {
        return Err(AbiError::new(
            MC_STATUS_VALIDATION_FAILED,
            format!("owner cache file {relative} escapes the cache or is a symlink"),
        ));
    }
    let bytes = fs::read(&canonical_candidate).map_err(|error| {
        AbiError::new(
            MC_STATUS_IO_ERROR,
            format!("cannot read owner cache file {relative}: {error}"),
        )
    })?;
    if bytes.len() != expected_len {
        return Err(AbiError::new(
            MC_STATUS_VALIDATION_FAILED,
            format!(
                "owner cache file {relative} has {} bytes, expected {expected_len}",
                bytes.len()
            ),
        ));
    }
    let actual_sha256 = format!("{:x}", Sha256::digest(&bytes));
    if actual_sha256 != expected_sha256 {
        return Err(AbiError::new(
            MC_STATUS_VALIDATION_FAILED,
            format!("owner cache file {relative} hash mismatch"),
        ));
    }
    Ok(bytes)
}

fn struct_size<T>() -> u32 {
    u32::try_from(size_of::<T>()).expect("ABI struct size fits u32")
}

fn string_length(value: &str) -> AbiResult<u32> {
    u32::try_from(value.len())
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "string length exceeds u32"))
}

fn copy_string(value: &str, buffer: *mut u8, capacity: u32, out_length: *mut u32) -> AbiResult<()> {
    let payload_length = string_length(value)?;
    write_output(out_length, payload_length);
    let required = payload_length
        .checked_add(1)
        .ok_or_else(|| AbiError::new(MC_STATUS_BOUNDS, "string capacity overflow"))?;
    if capacity < required {
        return Err(AbiError::new(
            MC_STATUS_BUFFER_TOO_SMALL,
            format!("string buffer requires {required} bytes, got {capacity}"),
        ));
    }
    if buffer.is_null() {
        return Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            "string output buffer is null",
        ));
    }
    // SAFETY: the caller supplied `capacity >= value.len() + 1` writable bytes.
    unsafe {
        buffer.copy_from_nonoverlapping(value.as_ptr(), value.len());
        *buffer.add(value.len()) = 0;
    }
    Ok(())
}

fn copy_slice<T: Copy>(
    values: &[T],
    buffer: *mut T,
    capacity: u32,
    out_written: *mut u32,
) -> AbiResult<()> {
    let required = u32::try_from(values.len())
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "slice length exceeds u32"))?;
    if capacity < required {
        write_output(out_written, 0);
        return Err(AbiError::new(
            MC_STATUS_BUFFER_TOO_SMALL,
            format!("copy requires {required} elements, got {capacity}"),
        ));
    }
    if !values.is_empty() && buffer.is_null() {
        return Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            "copy output buffer is null",
        ));
    }
    // SAFETY: the caller supplied enough writable T elements and source and
    // destination cannot overlap across the FFI ownership boundary.
    unsafe { buffer.copy_from_nonoverlapping(values.as_ptr(), values.len()) };
    write_output(out_written, required);
    Ok(())
}

fn utf8_input(pointer: *const u8, length: u32, name: &str) -> AbiResult<String> {
    let length = usize::try_from(length)
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, format!("{name} length exceeds usize")))?;
    if length > 0 && pointer.is_null() {
        return Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            format!("{name} pointer is null"),
        ));
    }
    if length == 0 {
        return Ok(String::new());
    }
    // SAFETY: the caller promises `length` readable bytes at the non-null
    // pointer for the duration of this call.
    let bytes = unsafe { slice::from_raw_parts(pointer, length) };
    str::from_utf8(bytes).map(str::to_owned).map_err(|error| {
        AbiError::new(
            MC_STATUS_INVALID_UTF8,
            format!("{name} is not valid UTF-8: {error}"),
        )
    })
}

fn read_versioned_input<T: Copy>(pointer: *const T, name: &str) -> AbiResult<T> {
    if pointer.is_null() {
        return Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            format!("{name} pointer is null"),
        ));
    }
    // SAFETY: every versioned ABI input begins with two u32 fields. Reading
    // only that fixed header lets us reject short callers without first
    // reading the full current Rust structure.
    let bytes = pointer.cast::<u8>();
    let abi_version = unsafe { bytes.cast::<u32>().read_unaligned() };
    let declared_size = unsafe { bytes.add(size_of::<u32>()).cast::<u32>().read_unaligned() };
    if abi_version != MC_NATIVE_ABI_VERSION {
        return Err(AbiError::new(
            MC_STATUS_UNSUPPORTED_VERSION,
            format!("{name} ABI version {abi_version} is unsupported"),
        ));
    }
    let required = struct_size::<T>();
    if declared_size < required {
        return Err(AbiError::new(
            MC_STATUS_STRUCT_TOO_SMALL,
            format!("{name} struct_size {declared_size} is smaller than {required}"),
        ));
    }
    // SAFETY: after declaring at least the current prefix size, the caller is
    // required by the ABI contract to provide that readable prefix.
    Ok(unsafe { pointer.read_unaligned() })
}

fn require_output<T>(pointer: *mut T, name: &str) -> AbiResult<()> {
    if pointer.is_null() {
        Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            format!("{name} pointer is null"),
        ))
    } else {
        Ok(())
    }
}

fn require_struct_output<T>(pointer: *mut T, output_size: u32, name: &str) -> AbiResult<()> {
    require_output(pointer, name)?;
    let required = struct_size::<T>();
    if output_size < required {
        return Err(AbiError::new(
            MC_STATUS_STRUCT_TOO_SMALL,
            format!("{name} size {output_size} is smaller than {required}"),
        ));
    }
    Ok(())
}

fn require_element_size<T>(element_size: u32, name: &str) -> AbiResult<()> {
    let required = struct_size::<T>();
    if element_size < required {
        return Err(AbiError::new(
            MC_STATUS_STRUCT_TOO_SMALL,
            format!("{name} size {element_size} is smaller than {required}"),
        ));
    }
    if element_size > required {
        return Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            format!("{name} size {element_size} must equal {required}"),
        ));
    }
    Ok(())
}

fn write_output<T>(pointer: *mut T, value: T) {
    // SAFETY: all callers validated the output pointer before reaching this
    // helper. Unaligned writes support conservative P/Invoke marshaling.
    unsafe { pointer.write_unaligned(value) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::{
        FlightWorld,
        level1_mission::{
            ABILITY_IMPROVED_NAME_SELECTOR_BASE, ABILITY_IMPROVED_NOTIFICATION_STATE,
            ABILITY_IMPROVED_NOTIFICATION_TIMER, ABILITY_IMPROVED_TEMPLATE_SELECTOR,
        },
    };
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    #[repr(C, align(8))]
    struct AlignedAbiHeader {
        abi_version: u32,
        struct_size: u32,
    }

    fn source_presentation_input() -> McLevel1PresentationInputV2 {
        let mut input = McLevel1PresentationInputV2 {
            authority_mask: LIVE_MENU_AUTH_ALL,
            load_available: 1,
            sound_available: 1,
            music_available: 1,
            options_enabled_mask: 0x1ffff,
            sound_volume: 127,
            music_volume: 127,
            volume_target: 1,
            text_colour_phase: 1,
            debug_clock_bucket: 17,
            sound_number: 46,
            debug_heap_size: 0,
            debug_memory_used: 0,
            debug_memory_free: 0,
            ..McLevel1PresentationInputV2::default()
        };
        for (index, name) in [
            b"Zanzamar".as_slice(),
            b"Nyphur",
            b"Rahn",
            b"Belix",
            b"Jark",
            b"Elyssia",
            b"Yragore",
            b"Prish",
        ]
        .into_iter()
        .enumerate()
        {
            let start = index * 16;
            input.chat_names[start..start + name.len()].copy_from_slice(name);
        }
        input.notification_player_name[..8].copy_from_slice(b"Zanzamar");
        input
    }

    fn submit_source_presentation_input(handle: u64, input: &McLevel1PresentationInputV2) {
        assert_eq!(
            mc_native_level1_presentation_input(handle, input),
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
    }

    #[test]
    fn world_frame_capability_flags_follow_the_passes_that_actually_ran() {
        let without_reflection = world_frame_flags(false, true);
        assert_eq!(
            without_reflection & MC_TERRAIN_FRAME_REFLECTED_SPRITES_COMPOSITED,
            0
        );
        assert_ne!(
            without_reflection & MC_TERRAIN_FRAME_SPRITE_SHADOWS_COMPOSITED,
            0
        );

        let without_shadows = world_frame_flags(true, false);
        assert_ne!(
            without_shadows & MC_TERRAIN_FRAME_REFLECTED_SPRITES_COMPOSITED,
            0
        );
        assert_eq!(
            without_shadows & MC_TERRAIN_FRAME_SPRITE_SHADOWS_COMPOSITED,
            0
        );
    }

    #[test]
    fn paused_native_steps_store_only_the_pre_gate_update_rng_draw() {
        let handle = open_fixture();
        let command = valid_command();
        let mut before = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_snapshot(
                handle,
                &raw mut before,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let (start_rng, start_turn, emitter_life) = with_session(handle, |session| {
            Ok((
                session.mission.global_rng(),
                session.mission.source_turn_counter(),
                session
                    .mission
                    .ambient_entities()
                    .find(|entity| entity.source_entity_index == 113)
                    .expect("reset model-60 source slot 113")
                    .life,
            ))
        })
        .unwrap();
        assert_eq!(mc_native_level1_game_paused(handle, 1), MC_STATUS_OK);

        let mut expected_rng = start_rng;
        for paused_index in 0..3 {
            let mut paused = McSessionSnapshotV2::default();
            assert_eq!(
                mc_native_session_step(
                    handle,
                    &raw const command,
                    &raw mut paused,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
            expected_rng = expected_rng.wrapping_mul(9_377).wrapping_add(9_439);
            assert_eq!(
                (paused.tick, paused.state_hash, paused.event_head),
                (before.tick, before.state_hash, before.event_head)
            );
            with_session(handle, |session| {
                assert_eq!(session.mission.global_rng(), expected_rng);
                assert_eq!(session.mission.source_turn_counter(), start_turn);
                let expected_presentation = match paused_index {
                    0 => (2, 1, 0),
                    1 => (3, 0, 200),
                    2 => (3, 0, 199),
                    _ => unreachable!(),
                };
                assert_eq!(
                    (
                        session.mission.objective_presentation_palette_mod,
                        session.mission.objective_presentation_phase,
                        session.mission.objective_textbox_ticks,
                    ),
                    expected_presentation,
                    "paused outer presentation remains live exactly once per frame"
                );
                assert_eq!(
                    session
                        .mission
                        .ambient_entities()
                        .find(|entity| entity.source_entity_index == 113)
                        .unwrap()
                        .life,
                    emitter_life,
                    "paused UpdateEntities must not dispatch the later model-60 row"
                );
                Ok(())
            })
            .unwrap();
        }

        assert_eq!(mc_native_level1_game_paused(handle, 0), MC_STATUS_OK);
        let mut resumed = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut resumed,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        for _ in 0..10 {
            expected_rng = expected_rng.wrapping_mul(9_377).wrapping_add(9_439);
        }
        with_session(handle, |session| {
            assert_eq!(session.mission.global_rng(), expected_rng);
            assert_eq!(session.mission.source_turn_counter(), start_turn + 1);
            Ok(())
        })
        .unwrap();
        assert_eq!(resumed.tick, before.tick + 1);
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn paused_objective_sound_stays_prepared_until_the_first_unpaused_pool_flush() {
        let handle = open_fixture();
        let command = valid_command();
        with_session_mut(handle, |session| {
            session.mission.current_stage = 1;
            session.mission.objective_presentation_phase = 1;
            session.mission.objective_presentation_palette_mod = 2;
            Ok(())
        })
        .unwrap();
        assert_eq!(mc_native_level1_game_paused(handle, 1), MC_STATUS_OK);
        let mut paused = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut paused,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let (sequence, prepared) = with_session(handle, |session| {
            let (sequence, command) = session
                .audio_commands
                .iter()
                .find(|(_, command)| command.sound_id == 61)
                .map(|(sequence, command)| (*sequence, *command))
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "paused stage-one presentation did not prepare sound61",
                    )
                })?;
            Ok((sequence, command))
        })
        .unwrap();
        assert_eq!(
            prepared.action, 0,
            "paused PlayEntitySounds gate retains the row"
        );
        assert_eq!(prepared.pooled_flush_rank, 0);
        assert_eq!(prepared.reserved, [0; 4]);

        assert_eq!(mc_native_level1_game_paused(handle, 0), MC_STATUS_OK);
        let mut resumed = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut resumed,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        with_session(handle, |session| {
            let resolved = session.audio_commands[&sequence];
            assert_ne!(resolved.action, 0);
            assert_eq!(
                resolved.pooled_flush_rank, 2,
                "retained sound61 follows the lower-ID current-frame winner"
            );
            assert_ne!(resolved.reserved[0], 0);
            assert_eq!(resolved.reserved[1..], [0; 3]);
            Ok(())
        })
        .unwrap();
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn playable_menu_transition_and_sound_slider_preview_use_distinct_source_flushes() {
        let handle = open_fixture();
        let initial = source_presentation_input();
        submit_source_presentation_input(handle, &initial);
        assert_eq!(mc_native_level1_player_menu_state(handle, 6), MC_STATUS_OK);
        let menu_sequence = with_session(handle, |session| {
            let (sequence, command) = session
                .audio_commands
                .iter()
                .find(|(_, command)| command.sound_id == 14)
                .map(|(sequence, command)| (*sequence, *command))
                .expect("menu transition prepares local sound14");
            assert_eq!(command.action, 0);
            Ok(sequence)
        })
        .unwrap();
        with_session_mut(handle, |session| {
            session.drain_pending_audio_commands();
            assert_ne!(session.audio_commands[&menu_sequence].action, 0);
            Ok(())
        })
        .unwrap();

        assert_eq!(mc_native_level1_player_menu_state(handle, 10), MC_STATUS_OK);
        let mut changed = initial;
        changed.sound_volume = 126;
        submit_source_presentation_input(handle, &changed);
        with_session(handle, |session| {
            let preview = session
                .audio_commands
                .values()
                .filter(|command| command.sound_id == 8)
                .max_by_key(|command| command.event_sequence)
                .expect("sound slider change prepares preview sound8");
            assert_ne!(preview.action, 0);
            assert_ne!(preview.pooled_flush_rank, 0);
            assert_ne!(preview.reserved[0], 0);
            Ok(())
        })
        .unwrap();
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn presentation_frame_abi_is_revisioned_complete_idempotent_and_fail_closed() {
        assert_eq!(size_of::<McPresentationFrameQueryV2>(), 64);
        assert_eq!(size_of::<McLevel1PresentationInputV2>(), 740);
        assert_eq!(size_of::<McPresentationFrameInfoV2>(), 128);
        assert_eq!(
            std::mem::offset_of!(McPresentationFrameQueryV2, configured_blur_mode),
            40
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1PresentationInputV2, mouse_x),
            38
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1PresentationInputV2, debug_clock_bucket),
            704
        );
        assert_eq!(
            std::mem::offset_of!(McPresentationFrameInfoV2, immutable_input_hash),
            32
        );
        assert_eq!(
            std::mem::offset_of!(McPresentationFrameInfoV2, index_hash),
            64
        );
        assert_eq!(
            std::mem::offset_of!(McPresentationFrameInfoV2, reserved),
            104
        );

        let handle = open_fixture_with_presentation_cache();
        let mut query = McPresentationFrameQueryV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McPresentationFrameQueryV2>(),
            generation: 1,
            tick: 0,
            presentation_revision: 1,
            viewport_width: LEVEL1_VIEW_WIDTH as u16,
            viewport_height: LEVEL1_VIEW_HEIGHT as u16,
            screen_stride: LEVEL1_VIEW_WIDTH as u16,
            reserved_0: 0,
            configured_blur_mode: 0,
            blur_capability: 0,
            high_speed_system: 1,
            scratch_present: 1,
            vga_resolution: 1,
            spatial_smoothing: 0,
            minimap_enabled: 1,
            top_bar_enabled: 1,
            menu_overlay: 0,
            game_paused: 0,
            level_end: 0,
            objective_selector: u8::MAX,
            objective_textbox: 0,
            tutorial_or_briefing: 0,
            help_popup: 0,
            debug_text: 0,
            reserved: [0; 2],
        };
        let mut indices = vec![0_u8; LEVEL1_VIEW_PIXELS];
        let mut provenance = vec![0_u8; LEVEL1_VIEW_PIXELS];
        let mut info = McPresentationFrameInfoV2::default();
        let copy = |query: &McPresentationFrameQueryV2,
                    indices: &mut [u8],
                    provenance: &mut [u8],
                    info: &mut McPresentationFrameInfoV2| {
            mc_native_session_presentation_frame_copy(
                handle,
                query,
                indices.as_mut_ptr(),
                indices.len() as u32,
                provenance.as_mut_ptr(),
                provenance.len() as u32,
                info,
                struct_size::<McPresentationFrameInfoV2>(),
            )
        };
        let capture = |capture_handle: u64, capture_query: &McPresentationFrameQueryV2| {
            let mut capture_indices = vec![0_u8; LEVEL1_VIEW_PIXELS];
            let mut capture_provenance = vec![0_u8; LEVEL1_VIEW_PIXELS];
            let mut capture_info = McPresentationFrameInfoV2::default();
            let status = mc_native_session_presentation_frame_copy(
                capture_handle,
                capture_query,
                capture_indices.as_mut_ptr(),
                capture_indices.len() as u32,
                capture_provenance.as_mut_ptr(),
                capture_provenance.len() as u32,
                &raw mut capture_info,
                struct_size::<McPresentationFrameInfoV2>(),
            );
            (status, capture_indices, capture_provenance, capture_info)
        };
        let semantic_vectors = with_session(handle, |session| {
            crate::presentation_bridge::assert_independent_source_vectors(
                &session
                    .terrain_frame_assets
                    .as_ref()
                    .expect("qualified presentation cache")
                    .indexed_overlays,
            );
            Ok(session
                .terrain_frame_assets
                .as_ref()
                .expect("qualified presentation cache")
                .indexed_overlays
                .source_semantic_vectors()
                .clone())
        })
        .expect("source-semantic vector ledger");
        assert_eq!(
            semantic_vectors["palette_indirection"]["indexed_values"]
                .as_array()
                .expect("palette vector")
                .iter()
                .map(|value| value.as_i64().expect("palette integer"))
                .collect::<Vec<_>>(),
            [61, 23, 171, 87, 177, 119, 176]
        );
        assert_eq!(
            semantic_vectors["pause"]["selector_order"]
                .as_array()
                .expect("pause vector")
                .iter()
                .map(|value| value.as_i64().expect("pause selector integer"))
                .collect::<Vec<_>>(),
            [174, 175, 176, 177, 178]
        );
        let button_rows = semantic_vectors["chat"]["button_order"]
            .as_array()
            .expect("button vector rows");
        assert_eq!(
            button_rows
                .iter()
                .flat_map(|row| {
                    row.as_array()
                        .expect("button tuple")
                        .iter()
                        .map(|value| value.as_i64().expect("button integer"))
                })
                .collect::<Vec<_>>(),
            [259, 192, 166, 261, 237, 166, 260, 282, 166]
        );
        assert_eq!(
            semantic_vectors["options"]["on_ids"]
                .as_array()
                .map(Vec::len),
            Some(17)
        );
        assert_eq!(
            semantic_vectors["volume"]["source_bar"]
                .as_array()
                .expect("volume vector")
                .iter()
                .map(|value| value.as_i64().expect("volume integer"))
                .collect::<Vec<_>>(),
            [274, 210, 90, 12]
        );
        assert_eq!(
            semantic_vectors["ok_cancel"]["fixed_prompts"]
                .as_array()
                .expect("prompt vector")
                .iter()
                .map(|value| value.as_str().expect("prompt string"))
                .collect::<Vec<_>>(),
            ["Abandon level.?", "Load Level?", "Save Level?"]
        );
        assert_eq!(
            semantic_vectors["pause_or_end"]["end_text"],
            "Tasks completed. Fly to the exit point.."
        );
        assert_eq!(
            semantic_vectors["map"]["divider_order"]
                .as_array()
                .expect("divider vector")
                .iter()
                .map(|value| value.as_i64().expect("divider integer"))
                .collect::<Vec<_>>(),
            [192, 191]
        );
        assert_eq!(semantic_vectors["bottom"]["tier_spacing"], 33);
        assert_eq!(semantic_vectors["debug"]["version"], "Beta");
        assert_eq!(semantic_vectors["cursor"]["active_policy"], "fail_closed");
        assert_eq!(
            copy(&query, &mut indices, &mut provenance, &mut info),
            MC_STATUS_OK
        );
        let complete_flags = MC_PRESENTATION_FRAME_COMPOSITOR_COMPLETE
            | MC_PRESENTATION_FRAME_HAS_PROVENANCE
            | MC_PRESENTATION_FRAME_OVERLAYS_COMPOSITED
            | MC_PRESENTATION_FRAME_POSTPROCESS_COMPLETE
            | MC_PRESENTATION_FRAME_LIGHTING_COMPLETE;
        assert_eq!(info.flags & complete_flags, complete_flags);
        assert_eq!(
            info.flags
                & (MC_TERRAIN_FRAME_ISOLATED_SKY_TERRAIN | MC_TERRAIN_FRAME_NON_TERRAIN_OMITTED),
            0
        );
        assert_eq!((info.history_state, info.newly_committed), (1, 1));
        assert_eq!(
            sim_core::presentation_frame::byte_hash(&indices),
            info.index_hash
        );
        assert_eq!(
            sim_core::presentation_frame::byte_hash(&provenance),
            info.provenance_hash
        );
        assert!(provenance.iter().any(|value| value & 1 != 0));
        assert!(provenance.iter().any(|value| value & 8 != 0));
        assert_ne!(info.immutable_input_hash, 0);
        let first_indices = indices.clone();
        let first_provenance = provenance.clone();
        let first_info = info;

        assert_eq!(
            copy(&query, &mut indices, &mut provenance, &mut info),
            MC_STATUS_OK
        );
        assert_eq!(info.newly_committed, 0);
        assert_eq!(indices, first_indices);
        assert_eq!(provenance, first_provenance);
        assert_eq!(info.immutable_input_hash, first_info.immutable_input_hash);

        let baseline_source_render_hash =
            with_session(handle, |session| session.source_render_input_hash())
                .expect("baseline source-render hash");
        with_session(handle, |session| {
            let baseline = session.source_render_inputs(false)?;
            let assert_changed = |label: &str, changed: SourceRenderInputs| -> AbiResult<()> {
                assert_ne!(
                    session.source_render_input_hash_for(&changed)?,
                    baseline_source_render_hash,
                    "{label} participates in the complete source-render key"
                );
                Ok(())
            };

            let mut changed = baseline.clone();
            changed.map_right_viewport = !changed.map_right_viewport;
            assert_changed("map render branch", changed)?;
            for (label, mutate) in [
                ("composed camera render_x", 0_u8),
                ("composed camera render_y", 1),
                ("composed camera eye_z", 2),
                ("composed camera yaw", 3),
                ("composed camera pitch", 4),
                ("composed camera roll", 5),
                ("composed camera fov", 6),
                ("composed camera source_turn", 7),
                ("composed camera reflections", 8),
            ] {
                let mut changed = baseline.clone();
                match mutate {
                    0 => changed.camera.render_x ^= 1,
                    1 => changed.camera.render_y ^= 1,
                    2 => changed.camera.eye_z ^= 1,
                    3 => changed.camera.yaw ^= 1,
                    4 => changed.camera.pitch ^= 1,
                    5 => changed.camera.roll ^= 1,
                    6 => changed.camera.fov ^= 1,
                    7 => changed.camera.source_turn ^= 1,
                    8 => changed.camera.reflections = !changed.camera.reflections,
                    _ => unreachable!(),
                }
                assert_changed(label, changed)?;
            }
            let mut changed = baseline.clone();
            changed.generation_asset_digest ^= 1;
            assert_changed("generation renderer-asset digest", changed)?;
            let mut changed = baseline.clone();
            changed.local_player_entity_index ^= 1;
            assert_changed("local player identity", changed)?;
            let mut changed = baseline.clone();
            changed.goats[0].position.x ^= 1;
            assert_changed("Goat record", changed)?;
            let mut changed = baseline.clone();
            changed.frozen_actors[0].position.y ^= 1;
            assert_changed("frozen actor record", changed)?;
            let mut changed = baseline.clone();
            changed.ambient_entities[0].phase ^= 1;
            assert_changed("ambient record", changed)?;
            let mut changed = baseline.clone();
            changed.dynamic_residents.push(DynamicResidentState {
                source_entity_index: 983,
                source_id: 983,
                kind: sim_core::level1_mission::DynamicResidentKind::Builder,
                class: 5,
                model: 4,
                action: 96,
                position: Position {
                    x: 0x1212,
                    y: 0x3434,
                    z: 56,
                },
                max_life: 100,
                life: 90,
                pending_damage: 0,
                pending_attacker: 0,
                current_attacker: 0,
                kill_credit_entity: 0,
                entity_rng: 17,
                yaw: 18,
                desired_heading: 19,
                minimum_speed: 20,
                actual_speed: 21,
                maximum_speed: 22,
                target_entity_index: 0,
                linked_entity_index: 0,
                owner_entity_index: 1,
                reset_counter: 23,
                mana: 24,
                stage_var_2: 0,
                builder_placement_latch: 0,
                movement_blocked: false,
                suppress_death_burst: false,
                phase: 25,
                collision_half_width: 26,
                collision_vertical_offset: 27,
                collision_half_height: 28,
                render_parameter_index: 83,
                base_tmap: 80,
                render_mode: 20,
                logical_height: 200,
                active: true,
                visible: true,
            });
            assert_changed("dynamic resident record", changed)?;

            let mut changed = baseline.clone();
            changed.mission_actors[0].position.x ^= 1;
            changed.mission_actors[0].yaw ^= 1;
            changed.mission_actors[0].visible = !changed.mission_actors[0].visible;
            assert_changed("actor position/yaw/visibility", changed)?;

            let mut changed = baseline.clone();
            changed.mission_segments.push(Level1MultipartSegment {
                source_entity_index: 991,
                head_authored_index: 9,
                head_source_entity_index: 990,
                predecessor_source_entity_index: 990,
                successor_source_entity_index: 0,
                ordinal: 1,
                class: 5,
                model: 3,
                action: 0,
                owner_id: 1,
                position: Position {
                    x: 0x1234,
                    y: 0x5678,
                    z: 90,
                },
                life: 100,
                pending_damage: 0,
                pending_attacker: 0,
                yaw: 321,
                pitch: -17,
                phase: 4,
                trailing_distance: 64,
                collision_half_width: 30,
                collision_half_height: 40,
                render_parameter_index: 83,
                base_tmap: 80,
                render_mode: 20,
                logical_height: 200,
                active: true,
                visible: true,
            });
            assert_changed("multipart segment", changed)?;

            let mut changed = baseline.clone();
            changed.castle_root = Some(Level1CastleRootRender {
                source_entity_index: 989,
                position: Position {
                    x: 0x4321,
                    y: 0x8765,
                    z: 111,
                },
                active: true,
            });
            assert_changed("castle root", changed)?;

            let mut changed = baseline.clone();
            changed.mana_spheres.push(ManaSphere {
                id: 988,
                source_entity_index: 988,
                entity_rng: 17,
                position: Position {
                    x: 0x1111,
                    y: 0x2222,
                    z: 33,
                },
                velocity_x: 1,
                velocity_y: 2,
                vertical_velocity: 3,
                yaw: 4,
                mana: 5,
                owner_entity_index: 0,
                player_entity_index: 0,
                incoming_owner_entity_index: 0,
                incoming_possession_payload: 0,
                assigned_carrier_id: 0,
                assigned_defender_slot: 0,
                control_flags: 0,
                phase: 6,
                active: true,
                pending_delete: false,
            });
            assert_changed("mana sphere", changed)?;

            let mut changed = baseline.clone();
            changed.mana_carrier = Some(ManaCarrier {
                id: 987,
                source_entity_index: 987,
                entity_rng: 18,
                owner_entity_index: 1,
                position: Position {
                    x: 0x3333,
                    y: 0x4444,
                    z: 55,
                },
                yaw: 6,
                pitch: 7,
                speed: 8,
                life: 9,
                max_life: 10,
                max_mana: 11,
                target_sphere_id: 988,
                carried_mana: 12,
                returning_to_castle: false,
                pending_damage: 13,
                pending_attacker_entity_index: 14,
                active: true,
            });
            assert_changed("mana carrier", changed)?;

            let mut changed = baseline.clone();
            changed.projectiles.push(Level1Projectile::PlayerFireball(
                sim_core::level1_mission::FireballProjectile {
                    id: 1,
                    source_entity_index: 986,
                    owner_entity_index: 1,
                    position: Position {
                        x: 0x5555,
                        y: 0x6666,
                        z: 77,
                    },
                    yaw: 8,
                    pitch: 9,
                    speed: 10,
                    life_ticks: 11,
                    payload: 12,
                    target_entity_index: 0,
                    acquisition_initialized: true,
                    active: true,
                },
            ));
            assert_changed("projectile", changed)?;

            let mut changed = baseline.clone();
            changed.impacts.push(FireballImpact {
                id: 2,
                source_entity_index: 985,
                source_projectile_id: 1,
                owner_entity_index: 1,
                external_target_entity_index: 0,
                model: FIREBALL_IMPACT_MODEL,
                position: Position {
                    x: 0x7777,
                    y: 0x8888,
                    z: 99,
                },
                payload: 14,
                life_ticks: 15,
                entity_rng: 16,
                vertical_velocity: 17,
                phase: 18,
                animation_frame: 19,
                damage_submitted: false,
                active: true,
            });
            assert_changed("impact", changed)?;

            let mut changed = baseline.clone();
            changed.possessed_buildings.push(Level1PossessedBuilding {
                entity_index: 984,
                position: Position {
                    x: 0x9999,
                    y: 0xaaaa,
                    z: 121,
                },
                owner_entity_index: 1,
                player_color_index: 2,
                render_parameter_index: 177,
                base_tmap: 96,
                render_mode: 20,
                logical_height: 250,
                active: true,
            });
            assert_changed("possessed building", changed)?;

            let mut changed = baseline.clone();
            changed.source_slots[0].phase ^= 1;
            assert_changed("source-slot record", changed)?;

            let mut changed = baseline.clone();
            changed.dispositions.push(SourceSpriteDisposition::Legacy {
                source_entity_index: 983,
                family: SourceSpriteLegacyFamily::ImportedReset,
            });
            assert_changed("normalized source disposition", changed)?;

            let mut ordered = baseline.clone();
            for entity_index in [980_u16, 981] {
                ordered
                    .map_chains
                    .sync_entity(RenderableMapSeed {
                        entity_index,
                        active: true,
                        x: 0x2222,
                        y: 0x3333,
                    })
                    .expect("synthetic same-tile source-map insert");
            }
            let ordered_hash = session.source_render_input_hash_for(&ordered)?;
            let mut reordered = ordered.clone();
            reordered
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: 980,
                    active: true,
                    x: 0x2322,
                    y: 0x3333,
                })
                .expect("temporary adjacent-tile move");
            reordered
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: 980,
                    active: true,
                    x: 0x2222,
                    y: 0x3333,
                })
                .expect("same-tile head reinsert");
            assert_ne!(
                session.source_render_input_hash_for(&reordered)?,
                ordered_hash,
                "same membership and positions with different source-map head order changes the key"
            );
            assert_changed("same-tile source-map painter order", reordered)?;

            let mut changed = baseline;
            changed.night = !changed.night;
            assert_changed("night palette profile", changed)?;
            Ok(())
        })
        .expect("complete source-render projection mutation vectors");
        let generation_asset_digest = with_session_mut(handle, |session| {
            let digest = session
                .source_render_asset_digest
                .expect("qualified generation binds its renderer assets");
            session.source_render_asset_digest = Some(digest ^ 1);
            Ok(digest)
        })
        .expect("mutate only the generation-bound renderer digest");
        assert_eq!(
            copy(&query, &mut indices, &mut provenance, &mut info),
            MC_STATUS_GENERATION_MISMATCH,
            "renderer asset-bank drift rejects before cached return"
        );
        with_session(handle, |session| {
            assert_eq!(
                session
                    .presentation_frame
                    .cached()
                    .expect("asset drift preserves the prior COMPLETE cache")
                    .immutable_input_hash,
                first_info.immutable_input_hash
            );
            Ok(())
        })
        .expect("asset drift is atomic");
        with_session_mut(handle, |session| {
            session.source_render_asset_digest = Some(generation_asset_digest);
            Ok(())
        })
        .expect("restore generation-bound renderer digest");
        assert_eq!(
            copy(&query, &mut indices, &mut provenance, &mut info),
            MC_STATUS_OK,
            "restored renderer identity permits the exact cached retry"
        );
        assert_eq!(info.newly_committed, 0);
        with_session_mut(handle, |session| {
            let baseline = session.terrain.clone();
            session.terrain.dynamic_lighting_planes().1[0] ^= 1;
            assert_ne!(
                session.source_render_input_hash()?,
                baseline_source_render_hash,
                "terrain content participates in the source-render key"
            );
            session.terrain = baseline;

            let baseline = session.live_world.clone();
            session.live_world.release_villager_stage_group_7();
            assert_ne!(
                session.source_render_input_hash()?,
                baseline_source_render_hash,
                "live-world content participates in the source-render key"
            );
            session.live_world = baseline;

            let baseline = session.camera_slope_sample;
            session.camera_slope_sample.offset_x ^= 1;
            assert_ne!(
                session.source_render_input_hash()?,
                baseline_source_render_hash,
                "camera-slope input participates in the source-render key"
            );
            session.camera_slope_sample = baseline;

            let baseline = session.frozen_actors.clone();
            session.frozen_actors[0].position.x ^= 1;
            assert_ne!(
                session.source_render_input_hash()?,
                baseline_source_render_hash,
                "frozen render records participate in the source-render key"
            );
            session.frozen_actors = baseline;

            let assets = Arc::clone(
                session
                    .terrain_frame_assets
                    .as_ref()
                    .expect("qualified source banks"),
            );
            let goat_bank = session
                .world
                .goat_tmap_bank()
                .expect("qualified Goat bank")
                .clone();
            let baseline = session.selector_frames.clone();
            session
                .selector_frames
                .as_mut()
                .expect("Goat clocks")
                .latch_draw_attempts(GOAT_TMAP_DRAW_MASK)
                .expect("valid Goat draw mask");
            session
                .selector_frames
                .as_mut()
                .expect("Goat clocks")
                .service_unpaused_outer_turn(&goat_bank);
            assert_ne!(
                session.source_render_input_hash()?,
                baseline_source_render_hash,
                "Goat current TMAP participates in the source-render key"
            );
            session.selector_frames = baseline;

            let baseline = session.archer_selector_frames.clone();
            session
                .archer_selector_frames
                .as_mut()
                .expect("Archer clocks")
                .latch_draw_attempts(sim_core::archer_tmap::ARCHER_TMAP_DRAW_MASK)
                .expect("valid Archer draw mask");
            session
                .archer_selector_frames
                .as_mut()
                .expect("Archer clocks")
                .service_unpaused_outer_turn(&assets.archer_tmap_bank);
            assert_ne!(
                session.source_render_input_hash()?,
                baseline_source_render_hash,
                "Archer current TMAP participates in the source-render key"
            );
            session.archer_selector_frames = baseline;

            let baseline = session.firefly_selector_frames.clone();
            session
                .firefly_selector_frames
                .as_mut()
                .expect("Firefly clocks")
                .latch_draw_attempts(sim_core::firefly_tmap::FIREFLY_TMAP_DRAW_MASK);
            session
                .firefly_selector_frames
                .as_mut()
                .expect("Firefly clocks")
                .service_unpaused_outer_turn(&assets.firefly_tmap_bank);
            assert_ne!(
                session.source_render_input_hash()?,
                baseline_source_render_hash,
                "Firefly current TMAP participates in the source-render key"
            );
            session.firefly_selector_frames = baseline;

            let baseline = session.villager_selector_frames.clone();
            session
                .villager_selector_frames
                .as_mut()
                .expect("villager clocks")
                .latch_draw_attempts(sim_core::frozen_actor_tmap::VILLAGER_TMAP_DRAW_MASK)
                .expect("valid villager draw mask");
            session
                .villager_selector_frames
                .as_mut()
                .expect("villager clocks")
                .service_unpaused_outer_turn(&assets.frozen_actor_tmap_bank);
            assert_ne!(
                session.source_render_input_hash()?,
                baseline_source_render_hash,
                "villager current TMAP participates in the source-render key"
            );
            session.villager_selector_frames = baseline;

            let baseline = session.fireball_selector_frame.clone();
            session
                .fireball_selector_frame
                .as_mut()
                .expect("Fireball clock")
                .latch_draw_attempt(true);
            session
                .fireball_selector_frame
                .as_mut()
                .expect("Fireball clock")
                .service_unpaused_outer_turn(&assets.combat_tmap_bank);
            assert_ne!(
                session.source_render_input_hash()?,
                baseline_source_render_hash,
                "Fireball current TMAP participates in the source-render key"
            );
            session.fireball_selector_frame = baseline;

            let baseline = session.possession_selector_frame.clone();
            session
                .possession_selector_frame
                .as_mut()
                .expect("Possession clock")
                .latch_draw_attempt(true);
            session
                .possession_selector_frame
                .as_mut()
                .expect("Possession clock")
                .service_unpaused_outer_turn(&assets.combat_tmap_bank);
            assert_ne!(
                session.source_render_input_hash()?,
                baseline_source_render_hash,
                "Possession current TMAP participates in the source-render key"
            );
            session.possession_selector_frame = baseline;

            let baseline = session.castle_selector_frame.clone();
            session
                .castle_selector_frame
                .as_mut()
                .expect("Castle clock")
                .latch_draw_attempt(true);
            session
                .castle_selector_frame
                .as_mut()
                .expect("Castle clock")
                .service_unpaused_outer_turn(&assets.economy_tmap_bank);
            assert_ne!(
                session.source_render_input_hash()?,
                baseline_source_render_hash,
                "Castle current TMAP participates in the source-render key"
            );
            session.castle_selector_frame = baseline;
            assert_eq!(
                session.source_render_input_hash()?,
                baseline_source_render_hash,
                "restoring every renderer input restores the exact key"
            );
            Ok(())
        })
        .expect("source-render domain mutation witnesses");

        let baseline_slope = with_session(handle, |session| Ok(session.camera_slope_sample))
            .expect("baseline slope");
        with_session_mut(handle, |session| {
            session.camera_slope_sample.offset_y ^= 1;
            Ok(())
        })
        .expect("same-tick slope mutation");
        assert_eq!(
            copy(&query, &mut indices, &mut provenance, &mut info),
            MC_STATUS_GENERATION_MISMATCH,
            "same-tick source-render drift rejects before cached reuse"
        );
        with_session(handle, |session| {
            assert_eq!(
                session
                    .presentation_frame
                    .cached()
                    .expect("prior COMPLETE frame remains cached")
                    .immutable_input_hash,
                first_info.immutable_input_hash
            );
            Ok(())
        })
        .expect("same-tick rejection preserves the prior cache");
        with_session_mut(handle, |session| {
            session.camera_slope_sample = baseline_slope;
            Ok(())
        })
        .expect("restore slope input");
        assert_eq!(
            copy(&query, &mut indices, &mut provenance, &mut info),
            MC_STATUS_OK,
            "restored same-tick source input permits exact cached retry"
        );
        assert_eq!(info.newly_committed, 0);

        let chain_handle = open_fixture_with_presentation_cache();
        let mut ordered_chains = with_session(chain_handle, |session| {
            Ok(session.renderable_map_chains.clone())
        })
        .expect("isolated baseline painter-map topology");
        for entity_index in [113_u16, 114] {
            ordered_chains
                .sync_entity(RenderableMapSeed {
                    entity_index,
                    active: true,
                    x: 0x2222,
                    y: 0x3333,
                })
                .expect("live same-tile source-map insert");
        }
        with_session_mut(chain_handle, |session| {
            session.renderable_map_chains = ordered_chains.clone();
            Ok(())
        })
        .expect("install ordered live painter map");
        let mut chain_indices = vec![0_u8; LEVEL1_VIEW_PIXELS];
        let mut chain_provenance = vec![0_u8; LEVEL1_VIEW_PIXELS];
        let mut chain_info = McPresentationFrameInfoV2::default();
        let chain_copy =
            |indices: &mut [u8], provenance: &mut [u8], info: &mut McPresentationFrameInfoV2| {
                mc_native_session_presentation_frame_copy(
                    chain_handle,
                    &raw const query,
                    indices.as_mut_ptr(),
                    indices.len() as u32,
                    provenance.as_mut_ptr(),
                    provenance.len() as u32,
                    info,
                    struct_size::<McPresentationFrameInfoV2>(),
                )
            };
        assert_eq!(
            chain_copy(&mut chain_indices, &mut chain_provenance, &mut chain_info),
            MC_STATUS_OK,
            "ordered same-tile live map establishes the immutable baseline"
        );
        let ordered_info = chain_info;
        let mut reordered_chains = ordered_chains.clone();
        reordered_chains
            .sync_entity(RenderableMapSeed {
                entity_index: 113,
                active: true,
                x: 0x2322,
                y: 0x3333,
            })
            .expect("temporary live adjacent-tile move");
        reordered_chains
            .sync_entity(RenderableMapSeed {
                entity_index: 113,
                active: true,
                x: 0x2222,
                y: 0x3333,
            })
            .expect("live same-tile head reinsert");
        with_session_mut(chain_handle, |session| {
            session.renderable_map_chains = reordered_chains;
            Ok(())
        })
        .expect("install reordered live painter map");
        assert_eq!(
            chain_copy(&mut chain_indices, &mut chain_provenance, &mut chain_info),
            MC_STATUS_GENERATION_MISMATCH,
            "same-tick painter order drift rejects before cached return"
        );
        with_session(chain_handle, |session| {
            assert_eq!(
                session
                    .presentation_frame
                    .cached()
                    .expect("prior frame survives chain rejection")
                    .immutable_input_hash,
                ordered_info.immutable_input_hash
            );
            Ok(())
        })
        .expect("chain rejection preserves history/cache/clocks");
        with_session_mut(chain_handle, |session| {
            session.renderable_map_chains = ordered_chains;
            Ok(())
        })
        .expect("restore painter-map order");
        assert_eq!(
            chain_copy(&mut chain_indices, &mut chain_provenance, &mut chain_info),
            MC_STATUS_OK,
            "restored same-tile order returns the exact cached frame"
        );
        assert_eq!(chain_info.newly_committed, 0);
        assert_eq!(
            mc_native_session_destroy(chain_handle),
            MC_STATUS_OK,
            "isolated source-map reorder session closes"
        );

        let baseline_castle = with_session(handle, |session| {
            Ok((
                session.mission.castle_exists,
                session.mission.castle_source_entity_index,
                session.mission.castle_position,
            ))
        })
        .expect("baseline castle root");
        with_session_mut(handle, |session| {
            session.mission.castle_exists = true;
            session.mission.castle_source_entity_index = 989;
            session.mission.castle_position = Position {
                x: 0x3456,
                y: 0x789a,
                z: 88,
            };
            Ok(())
        })
        .expect("mutate a typed castle render record");
        assert_eq!(
            copy(&query, &mut indices, &mut provenance, &mut info),
            MC_STATUS_GENERATION_MISMATCH,
            "same-tick typed castle record drift rejects before cached return"
        );
        with_session_mut(handle, |session| {
            (
                session.mission.castle_exists,
                session.mission.castle_source_entity_index,
                session.mission.castle_position,
            ) = baseline_castle;
            Ok(())
        })
        .expect("restore typed castle render record");
        assert_eq!(
            copy(&query, &mut indices, &mut provenance, &mut info),
            MC_STATUS_OK
        );
        assert_eq!(info.newly_committed, 0);

        let isolated_handle = open_fixture_with_presentation_cache();
        let mut isolated_indices = vec![0_u8; LEVEL1_VIEW_PIXELS];
        let mut isolated_provenance = vec![0_u8; LEVEL1_VIEW_PIXELS];
        let mut isolated_info = McPresentationFrameInfoV2::default();
        assert_eq!(
            mc_native_session_presentation_frame_copy(
                isolated_handle,
                &raw const query,
                isolated_indices.as_mut_ptr(),
                isolated_indices.len() as u32,
                isolated_provenance.as_mut_ptr(),
                isolated_provenance.len() as u32,
                &raw mut isolated_info,
                struct_size::<McPresentationFrameInfoV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(isolated_indices, first_indices);
        assert_eq!(isolated_provenance, first_provenance);
        assert_eq!(
            isolated_info.immutable_input_hash,
            first_info.immutable_input_hash
        );

        let unauthoritative_debug_handle = open_fixture_with_presentation_cache();
        let mut unauthoritative_debug_query = query;
        unauthoritative_debug_query.debug_text = 8;
        assert_eq!(
            capture(unauthoritative_debug_handle, &unauthoritative_debug_query).0,
            MC_STATUS_GENERATION_MISMATCH,
            "active debug metrics cannot earn COMPLETE from reset zeros"
        );
        assert_eq!(
            mc_native_session_destroy(unauthoritative_debug_handle),
            MC_STATUS_OK
        );

        let debug_handle = open_fixture_with_presentation_cache();
        let mut debug_input = source_presentation_input();
        submit_source_presentation_input(debug_handle, &debug_input);
        with_session_mut(debug_handle, |session| {
            session.source_turn = session.source_turn.wrapping_add(37);
            Ok(())
        })
        .expect("advance authoritative source turn within the next clock bucket");
        debug_input.debug_clock_bucket += 1;
        submit_source_presentation_input(debug_handle, &debug_input);
        with_session(debug_handle, |session| {
            assert_eq!(
                session.debug_fps, 37,
                "source clock bucket transition records turn delta as FPS"
            );
            assert_eq!(session.debug_heap_size, 0x0140_0000);
            assert_eq!(session.debug_memory_used, 0);
            assert_eq!(session.debug_memory_free, 0);
            Ok(())
        })
        .expect("debug FPS transition is authoritative");
        let mut debug_query = query;
        debug_query.debug_text = 8;
        let (status, debug_indices, debug_provenance, debug_info) =
            capture(debug_handle, &debug_query);
        assert_eq!(
            status,
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_ne!(
            debug_info.immutable_input_hash,
            first_info.immutable_input_hash
        );
        assert_ne!(debug_info.index_hash, first_info.index_hash);
        assert_ne!(debug_indices, first_indices);
        assert_ne!(debug_provenance, first_provenance);
        assert_eq!(mc_native_session_destroy(debug_handle), MC_STATUS_OK);

        let entity_debug_handle = open_fixture_with_presentation_cache();
        let mut entity_debug_query = query;
        entity_debug_query.debug_text = 0x40;
        let (entity_debug_status, entity_debug_indices, _, _) =
            capture(entity_debug_handle, &entity_debug_query);
        assert_eq!(
            entity_debug_status,
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_ne!(
            entity_debug_indices, first_indices,
            "debug flag0x40 traverses the selected live entity rows"
        );
        assert_eq!(mc_native_session_destroy(entity_debug_handle), MC_STATUS_OK);

        let hud_handle = open_fixture_with_presentation_cache();
        with_session_mut(hud_handle, |session| {
            session.mission.player_mana -= 16_384;
            Ok(())
        })
        .expect("mutate live HUD mana before the production bridge");
        let (status, hud_indices, _, hud_info) = capture(hud_handle, &query);
        assert_eq!(status, MC_STATUS_OK);
        assert_ne!(
            hud_info.immutable_input_hash,
            first_info.immutable_input_hash
        );
        assert_ne!(hud_info.index_hash, first_info.index_hash);
        assert_ne!(hud_indices, first_indices);
        assert_eq!(mc_native_session_destroy(hud_handle), MC_STATUS_OK);

        let objective_48 = open_fixture_with_presentation_cache();
        let objective_49 = open_fixture_with_presentation_cache();
        for (handle, selector) in [(objective_48, 48_u8), (objective_49, 49_u8)] {
            with_session_mut(handle, |session| {
                session.mission.current_text_selector = selector;
                session.mission.objective_textbox_ticks = 33;
                Ok(())
            })
            .expect("configure a live objective textbox");
        }
        let (status_48, objective_indices_48, objective_provenance_48, info_48) =
            capture(objective_48, &query);
        let (status_49, objective_indices_49, objective_provenance_49, info_49) =
            capture(objective_49, &query);
        assert_eq!(
            (status_48, status_49),
            (MC_STATUS_OK, MC_STATUS_OK),
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_ne!(info_48.immutable_input_hash, info_49.immutable_input_hash);
        assert_ne!(info_48.index_hash, info_49.index_hash);
        assert_ne!(objective_indices_48, objective_indices_49);
        assert_ne!(objective_provenance_48, objective_provenance_49);
        assert_eq!(mc_native_session_destroy(objective_48), MC_STATUS_OK);
        assert_eq!(mc_native_session_destroy(objective_49), MC_STATUS_OK);

        let menu_handle = open_fixture_with_presentation_cache();
        assert_eq!(
            mc_native_level1_player_menu_state(menu_handle, 5),
            MC_STATUS_OK
        );
        let (menu_status, menu_indices, _, menu_info) = capture(menu_handle, &query);
        assert_eq!(menu_status, MC_STATUS_OK);
        assert_ne!(
            menu_info.immutable_input_hash,
            first_info.immutable_input_hash
        );
        assert_ne!(menu_info.index_hash, first_info.index_hash);
        assert_ne!(menu_indices, first_indices);
        assert_eq!(mc_native_session_destroy(menu_handle), MC_STATUS_OK);

        let unauthoritative_chat_handle = open_fixture_with_presentation_cache();
        assert_eq!(
            mc_native_level1_player_menu_state(unauthoritative_chat_handle, 3),
            MC_STATUS_OK
        );
        assert_eq!(
            capture(unauthoritative_chat_handle, &query).0,
            MC_STATUS_GENERATION_MISMATCH,
            "active chat cannot earn COMPLETE from the inert reset UI state"
        );
        assert_eq!(
            mc_native_session_destroy(unauthoritative_chat_handle),
            MC_STATUS_OK
        );

        let acquired_handle = open_fixture_with_presentation_cache();
        assert_eq!(
            mc_native_level1_player_menu_state(acquired_handle, 5),
            MC_STATUS_OK
        );
        with_session_mut(acquired_handle, |session| {
            let fire = session
                .context
                .spell_fire
                .as_mut()
                .expect("Level 1 spell registry");
            fire.selected_spell_ui_index = 4;
            fire.spell_experience[4] = 1;
            fire.enabled_holder_indices[4] = 0;
            Ok(())
        })
        .expect("configure acquired family without a holder");
        let (acquired_status, _, _, _) = capture(acquired_handle, &query);
        assert_eq!(
            acquired_status,
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_eq!(mc_native_session_destroy(acquired_handle), MC_STATUS_OK);

        let cannot_handle = open_fixture_with_presentation_cache();
        assert_eq!(
            mc_native_level1_player_menu_state(cannot_handle, 5),
            MC_STATUS_OK
        );
        with_session_mut(cannot_handle, |session| {
            let fire = session
                .context
                .spell_fire
                .as_mut()
                .expect("Level 1 spell registry");
            let mut holder = fire.holders[0];
            holder.entity_index = 990;
            holder.model = 4;
            holder.active_value = 0;
            fire.holders.push(holder);
            fire.enabled_holder_indices[4] = holder.entity_index;
            fire.selected_spell_ui_index = 4;
            fire.selected_subspell_index = 0;
            fire.family_selected_tiers[4] = 0;
            fire.spell_levels[4] = 2;
            session.mission.castle_mana = 0;
            Ok(())
        })
        .expect("configure existing holder below its Castle-mana gate");
        let (cannot_status, _, _, _) = capture(cannot_handle, &query);
        assert_eq!(
            cannot_status,
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_eq!(mc_native_session_destroy(cannot_handle), MC_STATUS_OK);

        for (menu_state, expected_stages) in [
            (3_u8, 13_u16),
            (9, 13),
            (10, 13),
            (13, 13),
            (6, 14),
            (8, 15),
            (11, 14),
            (12, 14),
            (14, 14),
        ] {
            let branch_handle = open_fixture_with_presentation_cache();
            assert_eq!(
                mc_native_level1_player_menu_state(branch_handle, menu_state),
                MC_STATUS_OK
            );
            submit_source_presentation_input(branch_handle, &source_presentation_input());
            let (status, _, _, branch_info) = capture(branch_handle, &query);
            assert_eq!(
                status,
                MC_STATUS_OK,
                "menu state {menu_state}: {}",
                LAST_ERROR.with(|value| value.borrow().clone())
            );
            assert_eq!(
                branch_info.overlay_stage_count, expected_stages,
                "menu state {menu_state} consumes its source-ledger receipt census"
            );
            assert_eq!(mc_native_session_destroy(branch_handle), MC_STATUS_OK);
        }
        let map_frame = |pitch_delta: i16, fov_delta: i16| {
            let map_handle = open_fixture_with_presentation_cache();
            assert_eq!(
                mc_native_level1_player_menu_state(map_handle, 6),
                MC_STATUS_OK
            );
            with_session_mut(map_handle, |session| {
                session.camera_history.pitch =
                    session.camera_history.pitch.wrapping_add(pitch_delta);
                session.camera_history.fov = session.camera_history.fov.wrapping_add(fov_delta);
                Ok(())
            })
            .expect("configure the real map DrawWorld camera");
            let result = capture(map_handle, &query);
            assert_eq!(
                result.0,
                MC_STATUS_OK,
                "map pitch {pitch_delta} fov {fov_delta}: {}",
                LAST_ERROR.with(|value| value.borrow().clone())
            );
            assert_eq!(mc_native_session_destroy(map_handle), MC_STATUS_OK);
            result.1
        };
        let map_base = map_frame(0, 0);
        for (label, changed) in [("pitch", map_frame(64, 0)), ("fov", map_frame(0, 16))] {
            let base_left = map_base
                .chunks_exact(LEVEL1_VIEW_WIDTH)
                .flat_map(|row| &row[..191])
                .copied()
                .collect::<Vec<_>>();
            let changed_left = changed
                .chunks_exact(LEVEL1_VIEW_WIDTH)
                .flat_map(|row| &row[..191])
                .copied()
                .collect::<Vec<_>>();
            let base_right = map_base
                .chunks_exact(LEVEL1_VIEW_WIDTH)
                .flat_map(|row| &row[192..])
                .copied()
                .collect::<Vec<_>>();
            let changed_right = changed
                .chunks_exact(LEVEL1_VIEW_WIDTH)
                .flat_map(|row| &row[192..])
                .copied()
                .collect::<Vec<_>>();
            assert_eq!(
                changed_left, base_left,
                "map {label} changes only the real second-world viewport"
            );
            assert_ne!(
                changed_right, base_right,
                "map {label} must reach the real 128x200 DrawWorld projection"
            );
        }
        let scores_handle = open_fixture_with_presentation_cache();
        assert_eq!(
            mc_native_level1_player_menu_state(scores_handle, 7),
            MC_STATUS_OK
        );
        let (scores_status, _, _, _) = capture(scores_handle, &query);
        assert_eq!(
            scores_status, MC_STATUS_GENERATION_MISMATCH,
            "scores fail closed until the live all-player/help-RNG matrix is typed"
        );
        assert_eq!(mc_native_session_destroy(scores_handle), MC_STATUS_OK);

        let chat_handle = open_fixture_with_presentation_cache();
        assert_eq!(
            mc_native_level1_player_menu_state(chat_handle, 3),
            MC_STATUS_OK
        );
        let mut chat_input = source_presentation_input();
        chat_input.chat_lines[5 * 64..5 * 64 + 5].copy_from_slice(b"HELLO");
        chat_input.chat_mode = 3;
        chat_input.chat_player_mask = 1 << 1;
        // Source coordinates (20,376) enter fixed recipient cell 5 after the
        // VGA1 divide, overriding the stored recipient.
        chat_input.mouse_x = 20;
        chat_input.mouse_y = 376;
        submit_source_presentation_input(chat_handle, &chat_input);
        let (chat_status, chat_indices, _, _) = capture(chat_handle, &query);
        assert_eq!(
            chat_status,
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_ne!(
            chat_indices, first_indices,
            "the selected live chat buffer traverses the exact source writer"
        );
        assert_eq!(mc_native_session_destroy(chat_handle), MC_STATUS_OK);

        let cursor_handle = open_fixture_with_presentation_cache();
        assert_eq!(
            mc_native_level1_player_menu_state(cursor_handle, 3),
            MC_STATUS_OK
        );
        let mut cursor_input = source_presentation_input();
        cursor_input.cursor_visible = 1;
        cursor_input.joystick = 8;
        submit_source_presentation_input(cursor_handle, &cursor_input);
        let (cursor_status, _, _, _) = capture(cursor_handle, &query);
        assert_eq!(
            cursor_status, MC_STATUS_GENERATION_MISMATCH,
            "active selector9 must fail closed until its source bank is proven"
        );
        with_session(cursor_handle, |session| {
            assert_eq!(session.presentation_frame.history(), MainHistory::Unprimed);
            assert!(session.presentation_frame.cached().is_none());
            Ok(())
        })
        .expect("cursor provenance rejection is atomic");
        assert_eq!(mc_native_session_destroy(cursor_handle), MC_STATUS_OK);

        let options_handle = open_fixture_with_presentation_cache();
        assert_eq!(
            mc_native_level1_player_menu_state(options_handle, 9),
            MC_STATUS_OK
        );
        let mut options_input = source_presentation_input();
        options_input.options_selected_mask = 1;
        options_input.options_enabled_mask &= !1;
        options_input.mouse_x = 2 * 120;
        options_input.mouse_y = 2 * 36;
        submit_source_presentation_input(options_handle, &options_input);
        let (options_status, _, _, _) = capture(options_handle, &query);
        assert_eq!(
            options_status,
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_eq!(mc_native_session_destroy(options_handle), MC_STATUS_OK);

        let volume_handle = open_fixture_with_presentation_cache();
        assert_eq!(
            mc_native_level1_player_menu_state(volume_handle, 10),
            MC_STATUS_OK
        );
        let mut volume_input = source_presentation_input();
        volume_input.volume_target = 2;
        volume_input.music_volume = 63;
        submit_source_presentation_input(volume_handle, &volume_input);
        let (volume_status, _, _, _) = capture(volume_handle, &query);
        assert_eq!(
            volume_status,
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_eq!(mc_native_session_destroy(volume_handle), MC_STATUS_OK);

        let invalid_volume_handle = open_fixture_with_presentation_cache();
        assert_eq!(
            mc_native_level1_player_menu_state(invalid_volume_handle, 10),
            MC_STATUS_OK
        );
        let mut invalid_volume_input = source_presentation_input();
        invalid_volume_input.volume_target = 0;
        submit_source_presentation_input(invalid_volume_handle, &invalid_volume_input);
        let (invalid_volume_status, _, _, _) = capture(invalid_volume_handle, &query);
        assert_eq!(
            invalid_volume_status, MC_STATUS_GENERATION_MISMATCH,
            "volume target outside sound/music fails before history mutation"
        );
        with_session(invalid_volume_handle, |session| {
            assert_eq!(session.presentation_frame.history(), MainHistory::Unprimed);
            assert!(session.presentation_frame.cached().is_none());
            Ok(())
        })
        .expect("invalid volume rejection is atomic");
        assert_eq!(
            mc_native_session_destroy(invalid_volume_handle),
            MC_STATUS_OK
        );

        for selected_item in 1..=3 {
            let ok_handle = open_fixture_with_presentation_cache();
            assert_eq!(
                mc_native_level1_player_menu_state(ok_handle, 13),
                MC_STATUS_OK
            );
            let mut ok_input = source_presentation_input();
            ok_input.selected_menu_item = selected_item;
            submit_source_presentation_input(ok_handle, &ok_input);
            let (ok_status, _, _, _) = capture(ok_handle, &query);
            assert_eq!(
                ok_status,
                MC_STATUS_OK,
                "OK/cancel item {selected_item}: {}",
                LAST_ERROR.with(|value| value.borrow().clone())
            );
            assert_eq!(mc_native_session_destroy(ok_handle), MC_STATUS_OK);
        }

        let pause_handle = open_fixture_with_presentation_cache();
        let mut pause_query = query;
        let mut before_pause_step = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_snapshot(
                pause_handle,
                &raw mut before_pause_step,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(mc_native_level1_game_paused(pause_handle, 1), MC_STATUS_OK);
        let mut paused_step = McSessionSnapshotV2::default();
        let paused_command = valid_command();
        assert_eq!(
            mc_native_session_step(
                pause_handle,
                &raw const paused_command,
                &raw mut paused_step,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(paused_step.tick, before_pause_step.tick);
        assert_eq!(paused_step.state_hash, before_pause_step.state_hash);
        assert_eq!(paused_step.event_head, before_pause_step.event_head);
        pause_query.game_paused = 1;
        let mut pause_input = source_presentation_input();
        pause_input.load_available = 0;
        pause_input.sound_available = 0;
        pause_input.music_available = 0;
        submit_source_presentation_input(pause_handle, &pause_input);
        let (pause_status, pause_indices, _, pause_info) = capture(pause_handle, &pause_query);
        assert_eq!(pause_status, MC_STATUS_OK);
        assert_eq!(pause_info.overlay_stage_count, 13);
        assert_ne!(pause_indices, first_indices, "Pause writes its source text");
        assert_eq!(mc_native_level1_game_paused(pause_handle, 0), MC_STATUS_OK);
        let mut resumed_step = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                pause_handle,
                &raw const paused_command,
                &raw mut resumed_step,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(resumed_step.tick, paused_step.tick + 1);
        assert_eq!(mc_native_session_destroy(pause_handle), MC_STATUS_OK);

        let recursive_pause_handle = open_fixture_with_presentation_cache();
        assert_eq!(
            mc_native_level1_game_paused(recursive_pause_handle, 1),
            MC_STATUS_OK
        );
        let mut recursive_pause_input = source_presentation_input();
        recursive_pause_input.recursive_pause_writer = 1;
        submit_source_presentation_input(recursive_pause_handle, &recursive_pause_input);
        let (recursive_pause_status, _, _, _) = capture(recursive_pause_handle, &pause_query);
        assert_eq!(
            recursive_pause_status, MC_STATUS_GENERATION_MISMATCH,
            "recursive sub_30870 state fails closed rather than fabricating output"
        );
        with_session(recursive_pause_handle, |session| {
            assert_eq!(session.presentation_frame.history(), MainHistory::Unprimed);
            assert!(session.presentation_frame.cached().is_none());
            Ok(())
        })
        .expect("recursive pause rejection is atomic");
        assert_eq!(
            mc_native_session_destroy(recursive_pause_handle),
            MC_STATUS_OK
        );

        let fabricated_end_handle = open_fixture_with_presentation_cache();
        let mut level_end_query = query;
        level_end_query.level_end = 1;
        let (fabricated_status, _, _, _) = capture(fabricated_end_handle, &level_end_query);
        assert_eq!(
            fabricated_status, MC_STATUS_GENERATION_MISMATCH,
            "a render-side query cannot fabricate authoritative completion"
        );
        with_session(fabricated_end_handle, |session| {
            assert_eq!(session.presentation_frame.history(), MainHistory::Unprimed);
            assert!(session.presentation_frame.cached().is_none());
            Ok(())
        })
        .expect("fabricated completion rejection is atomic");
        assert_eq!(
            mc_native_session_destroy(fabricated_end_handle),
            MC_STATUS_OK
        );

        let completed_handle = open_fixture_with_presentation_cache();
        with_session_mut(completed_handle, |session| {
            session.mission.level_completed = true;
            Ok(())
        })
        .expect("configure authoritative completion");
        submit_source_presentation_input(completed_handle, &source_presentation_input());
        let (completed_status, completed_indices, _, completed_info) =
            capture(completed_handle, &level_end_query);
        assert_eq!(completed_status, MC_STATUS_OK);
        assert_eq!(completed_info.overlay_stage_count, 13);
        assert_ne!(
            completed_indices, first_indices,
            "authoritative completion writes the two source strings"
        );
        assert_eq!(mc_native_session_destroy(completed_handle), MC_STATUS_OK);

        let notification_handle = open_fixture_with_presentation_cache();
        with_session_mut(notification_handle, |session| {
            session.mission.player_notification_active = true;
            session.mission.player_notification_id =
                sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_ID;
            session.mission.player_notification_state =
                sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_STATE;
            session.mission.player_notification_timer =
                sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_TIMER;
            Ok(())
        })
        .expect("configure the source player notification");
        submit_source_presentation_input(notification_handle, &source_presentation_input());
        let (notification_status, notification_indices, _, notification_info) =
            capture(notification_handle, &query);
        assert_eq!(notification_status, MC_STATUS_OK);
        assert_eq!(notification_info.overlay_stage_count, 13);
        assert_ne!(
            notification_indices, first_indices,
            "the live source notification writes its fixed-profile text"
        );
        with_session(notification_handle, |session| {
            assert_eq!(
                session.notification_draw_feedback,
                Some((
                    session.state.tick,
                    true,
                    sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_STATE,
                    sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_TIMER - 1,
                )),
                "the source text writer stages one decrement only after COMPLETE"
            );
            assert_eq!(
                (
                    session.mission.player_notification_state,
                    session.mission.player_notification_timer
                ),
                (
                    sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_STATE,
                    sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_TIMER,
                ),
                "same-tick immutable state remains stable until the next outer turn"
            );
            Ok(())
        })
        .expect("notification feedback committed");
        let (repeat_status, _, _, repeat_info) = capture(notification_handle, &query);
        assert_eq!(repeat_status, MC_STATUS_OK);
        assert_eq!(repeat_info.newly_committed, 0);
        with_session(notification_handle, |session| {
            assert_eq!(
                session.notification_draw_feedback,
                Some((
                    session.state.tick,
                    true,
                    sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_STATE,
                    sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_TIMER - 1,
                )),
                "same-key cached retry cannot stage a second decrement"
            );
            Ok(())
        })
        .expect("notification retry is atomic");
        with_session_mut(notification_handle, |session| {
            session.step(player_command(valid_command()).expect("valid native command"));
            assert_eq!(
                session.mission.player_notification_timer,
                sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_TIMER - 1,
                "the next outer turn applies the single accepted writer feedback"
            );
            Ok(())
        })
        .expect("notification feedback applies once");
        assert_eq!(mc_native_session_destroy(notification_handle), MC_STATUS_OK);

        for state in 1..=5 {
            let state_handle = open_fixture_with_presentation_cache();
            with_session_mut(state_handle, |session| {
                session.mission.player_notification_active = true;
                session.mission.player_notification_id =
                    sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_ID;
                session.mission.player_notification_state = state;
                session.mission.player_notification_timer = 1;
                Ok(())
            })
            .expect("configure one exact notification state");
            submit_source_presentation_input(state_handle, &source_presentation_input());
            let (state_status, _, _, _) = capture(state_handle, &query);
            assert_eq!(
                state_status,
                MC_STATUS_OK,
                "notification state {state}: {}",
                LAST_ERROR.with(|value| value.borrow().clone())
            );
            with_session(state_handle, |session| {
                assert_eq!(
                    session.notification_draw_feedback,
                    Some((session.state.tick, true, state, 0)),
                    "old timer 1 draws once and stages post-decrement zero"
                );
                Ok(())
            })
            .expect("notification edge feedback");
            assert_eq!(mc_native_session_destroy(state_handle), MC_STATUS_OK);
        }

        let expired_notification_handle = open_fixture_with_presentation_cache();
        with_session_mut(expired_notification_handle, |session| {
            session.mission.player_notification_active = true;
            session.mission.player_notification_id =
                sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_ID;
            session.mission.player_notification_state = 1;
            session.mission.player_notification_timer = 0;
            Ok(())
        })
        .expect("configure notification timer zero");
        submit_source_presentation_input(expired_notification_handle, &source_presentation_input());
        let (expired_status, _, _, _) = capture(expired_notification_handle, &query);
        assert_eq!(expired_status, MC_STATUS_OK);
        with_session(expired_notification_handle, |session| {
            assert_eq!(
                session.notification_draw_feedback,
                Some((session.state.tick, false, 0, -1)),
                "old timer zero clears the source state without drawing"
            );
            Ok(())
        })
        .expect("expired notification feedback");
        assert_eq!(
            mc_native_session_destroy(expired_notification_handle),
            MC_STATUS_OK
        );

        let phase_notification = |timer: i16| {
            let phase_handle = open_fixture_with_presentation_cache();
            with_session_mut(phase_handle, |session| {
                session.mission.player_notification_active = true;
                session.mission.player_notification_id =
                    sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_ID;
                session.mission.player_notification_state = 2;
                session.mission.player_notification_timer = timer;
                Ok(())
            })
            .expect("configure state2 timer/phase edge");
            submit_source_presentation_input(phase_handle, &source_presentation_input());
            let result = capture(phase_handle, &query);
            assert_eq!(
                result.0,
                MC_STATUS_OK,
                "state2 timer {timer}: {}",
                LAST_ERROR.with(|value| value.borrow().clone())
            );
            assert_eq!(mc_native_session_destroy(phase_handle), MC_STATUS_OK);
            result
        };
        let (_, timer_101_indices, _, _) = phase_notification(101);
        let (_, timer_102_phase_indices, _, _) = phase_notification(102);
        assert_ne!(
            timer_101_indices, timer_102_phase_indices,
            "state2 uses CLRN3840 through post-decrement 100 then the phase colour above it"
        );

        let help_handle = open_fixture_with_presentation_cache();
        let mut help_query = query;
        help_query.help_popup = 1;
        let (help_status, _, _, _) = capture(help_handle, &help_query);
        assert_eq!(
            help_status, MC_STATUS_GENERATION_MISMATCH,
            "help cannot earn COMPLETE without cached E2A74 recursive state"
        );
        with_session(help_handle, |session| {
            assert_eq!(session.presentation_frame.history(), MainHistory::Unprimed);
            assert!(session.presentation_frame.cached().is_none());
            Ok(())
        })
        .expect("help prerequisite rejection is atomic");
        let (help_retry_status, _, _, help_retry_info) = capture(help_handle, &query);
        assert_eq!(
            help_retry_status, MC_STATUS_OK,
            "clearing the public help query retries the rejected tick"
        );
        assert_eq!(
            (
                help_retry_info.history_state,
                help_retry_info.newly_committed
            ),
            (1, 1),
            "a failed help request does not mutate its sentinel/history before the valid retry"
        );
        assert_eq!(mc_native_session_destroy(help_handle), MC_STATUS_OK);

        for label in ["tutorial sentinel stream", "spider sprite sheet"] {
            let prerequisite_handle = open_fixture_with_presentation_cache();
            let mut prerequisite_query = query;
            if label == "tutorial sentinel stream" {
                prerequisite_query.tutorial_or_briefing = 1;
            } else {
                with_session_mut(prerequisite_handle, |session| {
                    session.state.immobilize_counter = 1;
                    Ok(())
                })
                .expect(label);
            }
            let (status, _, _, _) = capture(prerequisite_handle, &prerequisite_query);
            assert_eq!(
                status, MC_STATUS_GENERATION_MISMATCH,
                "{label} must fail closed until the independent prerequisite is cached"
            );
            with_session(prerequisite_handle, |session| {
                assert_eq!(session.presentation_frame.history(), MainHistory::Unprimed);
                assert!(session.presentation_frame.cached().is_none());
                Ok(())
            })
            .expect("missing prerequisite is atomic");
            assert_eq!(mc_native_session_destroy(prerequisite_handle), MC_STATUS_OK);
        }

        let missing_source_handle = open_fixture_with_presentation_cache();
        with_session_mut(missing_source_handle, |session| {
            session.selector_frames = None;
            Ok(())
        })
        .expect("remove one current source compositor producer");
        let (missing_status, _, _, _) = capture(missing_source_handle, &query);
        assert_eq!(missing_status, MC_STATUS_GENERATION_MISMATCH);
        with_session(missing_source_handle, |session| {
            assert_eq!(session.presentation_frame.history(), MainHistory::Unprimed);
            assert!(session.presentation_frame.cached().is_none());
            Ok(())
        })
        .expect("missing producer fails before committing history");
        assert_eq!(
            mc_native_session_destroy(missing_source_handle),
            MC_STATUS_OK
        );

        query.presentation_revision = 2;
        assert_eq!(
            copy(&query, &mut indices, &mut provenance, &mut info),
            MC_STATUS_GENERATION_MISMATCH,
            "same finalized tick with a changed revision rejects"
        );
        query.presentation_revision = 1;
        query.tick = 2;
        assert_eq!(
            copy(&query, &mut indices, &mut provenance, &mut info),
            MC_STATUS_GENERATION_MISMATCH,
            "out-of-order presentation rejects before rendering"
        );

        let command = valid_command();
        let mut stepped = McSessionSnapshotV2::default();
        for _ in 0..2 {
            assert_eq!(
                mc_native_session_step(
                    handle,
                    &raw const command,
                    &raw mut stepped,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
        }
        with_session(handle, |session| {
            assert!(
                session
                    .world
                    .raw_source_entity_indices()
                    .iter()
                    .copied()
                    .any(|entity_index| {
                        session.mission.source_map_chains().tracks(entity_index)
                            && !session.renderable_map_chains.tracks(entity_index)
                    }),
                "FINAL MAIN keeps its filtered painter chain separate from the durable raw gameplay topology"
            );
            Ok(())
        })
        .expect("registered C API session");
        query.tick = 1;
        query.configured_blur_mode = 2;
        query.blur_capability = 1;
        let temporal_status = copy(&query, &mut indices, &mut provenance, &mut info);
        assert_eq!(
            temporal_status,
            MC_STATUS_OK,
            "the next contiguous stored camera tick consumes prior finalized MAIN: {}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_eq!(
            (info.effective_postprocess_mode, info.newly_committed),
            (2, 1)
        );
        assert!(
            provenance.iter().any(|value| value & 2 != 0),
            "temporal provenance records prior finalized MAIN"
        );
        query.tick = 0;
        assert_eq!(
            copy(&query, &mut indices, &mut provenance, &mut info),
            MC_STATUS_GENERATION_MISMATCH,
            "a stale tick rejects after contiguous advancement"
        );

        let mut snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_reset(
                handle,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        query.generation = snapshot.generation;
        query.tick = 0;
        query.configured_blur_mode = 2;
        query.blur_capability = 1;
        assert_eq!(
            copy(&query, &mut indices, &mut provenance, &mut info),
            MC_STATUS_GENERATION_MISMATCH,
            "reset restores Unprimed and rejects a temporal first frame"
        );
        query.configured_blur_mode = 0;
        query.blur_capability = 0;
        assert_eq!(
            copy(&query, &mut indices, &mut provenance, &mut info),
            MC_STATUS_OK
        );
        assert_eq!((info.history_state, info.newly_committed), (1, 1));
        let mut isolated_snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_reset(
                isolated_handle,
                &raw mut isolated_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_level1_player_menu_state(isolated_handle, 6),
            MC_STATUS_OK
        );
        let mut incomplete_query = query;
        incomplete_query.generation = isolated_snapshot.generation;
        incomplete_query.configured_blur_mode = 0;
        incomplete_query.blur_capability = 0;
        assert_eq!(
            mc_native_session_presentation_frame_copy(
                isolated_handle,
                &raw const incomplete_query,
                isolated_indices.as_mut_ptr(),
                isolated_indices.len() as u32,
                isolated_provenance.as_mut_ptr(),
                isolated_provenance.len() as u32,
                &raw mut isolated_info,
                struct_size::<McPresentationFrameInfoV2>(),
            ),
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_eq!(isolated_info.overlay_stage_count, 14);
        with_session(isolated_handle, |session| {
            assert_eq!(session.presentation_frame.history(), MainHistory::Primed);
            assert!(session.presentation_frame.cached().is_some());
            Ok(())
        })
        .expect("map source branch commits only after all typed receipts");

        let zero_life_handle = open_fixture_with_presentation_cache();
        with_session_mut(zero_life_handle, |session| {
            session.mission.player_life = 0;
            Ok(())
        })
        .expect("configure exact source alive boundary");
        let mut boundary_query = query;
        boundary_query.generation = 1;
        boundary_query.tick = 0;
        let (zero_status, _, _, zero_info) = capture(zero_life_handle, &boundary_query);
        assert_eq!(
            zero_status,
            MC_STATUS_OK,
            "life zero remains alive: {}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_eq!(zero_info.overlay_stage_count, 13);
        assert_eq!(mc_native_session_destroy(zero_life_handle), MC_STATUS_OK);

        let dead_handle = open_fixture_with_presentation_cache();
        with_session_mut(dead_handle, |session| {
            session.mission.player_life = -1;
            Ok(())
        })
        .expect("configure dead source branch");
        let (dead_status, _, _, dead_info) = capture(dead_handle, &boundary_query);
        assert_eq!(dead_status, MC_STATUS_OK);
        assert_eq!(
            dead_info.overlay_stage_count, 5,
            "dead ordinary branch is menu then objective/tutorial/help/debug only"
        );
        assert_eq!(mc_native_session_destroy(dead_handle), MC_STATUS_OK);
        for menu_state in [9_u8, 10, 13] {
            let dead_menu_handle = open_fixture_with_presentation_cache();
            with_session_mut(dead_menu_handle, |session| {
                session.mission.player_life = -1;
                session.mission.player_menu_state = menu_state;
                Ok(())
            })
            .expect("configure dead source menu branch");
            submit_source_presentation_input(dead_menu_handle, &source_presentation_input());
            let (status, _, _, dead_menu_info) = capture(dead_menu_handle, &boundary_query);
            assert_eq!(
                status,
                MC_STATUS_OK,
                "dead menu state {menu_state}: {}",
                LAST_ERROR.with(|value| value.borrow().clone())
            );
            assert_eq!(
                dead_menu_info.overlay_stage_count, 5,
                "dead menu state {menu_state} closes its source-ledger receipt sequence"
            );
            assert_eq!(mc_native_session_destroy(dead_menu_handle), MC_STATUS_OK);
        }
        assert_eq!(mc_native_session_destroy(isolated_handle), MC_STATUS_OK);
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn player_hit_bridge_installs_only_next_turn_directional_recoil() {
        let handle = open_fixture();
        let expected_life = with_session_mut(handle, |session| {
            let attacker = 10;
            session.mission.submit_player_damage(500, attacker);
            Ok(session.mission.player_life - 500)
        })
        .expect("registered C API session");
        let mut snapshot = McSessionSnapshotV2::default();
        let command = valid_command();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let control_handle = open_fixture();
        let mut control_snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                control_handle,
                &raw const command,
                &raw mut control_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let control_position =
            with_session(control_handle, |session| Ok(session.state.position)).unwrap();
        with_session(handle, |session| {
            assert_eq!(session.mission.player_life, expected_life);
            assert_eq!(
                session.state.recoil_speed, 46,
                "the same-turn eight-pass flight consumes four units of the installed boost"
            );
            assert_ne!(
                session.state.position, control_position,
                "sub_5EFA0 recoil participates in this same production flight turn"
            );
            Ok(())
        })
        .expect("registered C API session");
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_session_step(
                control_handle,
                &raw const command,
                &raw mut control_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        with_session(handle, |session| {
            assert_eq!(session.mission.player_life, expected_life);
            assert_eq!(
                session.state.recoil_speed, 42,
                "the next eight-pass turn decays the one-shot boost instead of replaying 50"
            );
            Ok(())
        })
        .expect("registered C API session");
        assert_eq!(mc_native_session_destroy(control_handle), MC_STATUS_OK);
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    fn fixture_paths() -> (PathBuf, PathBuf) {
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        (
            private.join("generated/level1-export-m"),
            private.join("generated/level1-asset-closure.json"),
        )
    }

    fn open_fixture() -> u64 {
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load_without_owner_cache(package, closure)
            .expect("package M opens without an owner cache");
        let session = SessionRecord::new_with_terrain_frame_assets(world, None)
            .expect("cache-free fixture session constructs");
        with_registry_mut(|registry| registry.insert(session))
            .expect("cache-free fixture session registers")
    }

    fn configured_goat_pitch_handle(retain_action_14: bool) -> u64 {
        const BLEATING_GOAT: u16 = 102;
        const BLEATING_GOAT_RESET_RNG: u16 = 32_616;

        let handle = open_fixture();
        with_session_mut(handle, |session| {
            let mut entities = session.world.entities().to_vec();
            let goat = &mut entities[usize::from(BLEATING_GOAT)];
            goat.action = 14;
            goat.position = session.state.position;
            session.live_world = LiveWorldState::from_entities(
                &entities,
                session.world.goat_stage_point(),
                session.world.goat_config(),
            )
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
            let configured = session
                .live_world
                .goats()
                .iter()
                .find(|goat| goat.entity_index == BLEATING_GOAT)
                .expect("source-derived Goat slot 102");
            assert_eq!(configured.rng, BLEATING_GOAT_RESET_RNG);
            assert_eq!(configured.action, 14);
            assert_eq!(session.mission.global_rng(), 0xad6e_67d9);
            Ok(())
        })
        .expect("configure the source-derived action-14 pitch discriminator");

        let mut before = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_snapshot(
                handle,
                &raw mut before,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        if retain_action_14 {
            let damage = McGoatDamageCommandV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McGoatDamageCommandV2>(),
                generation: before.generation,
                entity_index: BLEATING_GOAT,
                reserved_0: 0,
                damage: 1,
                reserved: [0; 4],
            };
            let mut batch = McGoatBatchV2::default();
            assert_eq!(
                mc_native_session_goat_inject_damage(
                    handle,
                    &raw const damage,
                    &raw mut batch,
                    struct_size::<McGoatBatchV2>(),
                ),
                MC_STATUS_OK
            );
        }
        handle
    }

    fn goat_pitch_audio_after_step(handle: u64) -> McAudioCommandV2 {
        const BLEATING_GOAT: u16 = 102;
        let mut after = McSessionSnapshotV2::default();
        let idle = valid_command();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const idle,
                &raw mut after,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let query = McEventQueryV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McEventQueryV2>(),
            generation: after.generation,
            cursor: 0,
            reserved: [0; 4],
        };
        let mut events = vec![McEventV2::default(); 256];
        let mut event_batch = McEventBatchV2::default();
        assert_eq!(
            mc_native_events_copy_since(
                handle,
                &raw const query,
                events.as_mut_ptr(),
                u32::try_from(events.len()).unwrap(),
                struct_size::<McEventV2>(),
                &raw mut event_batch,
                struct_size::<McEventBatchV2>(),
            ),
            MC_STATUS_OK
        );
        events.truncate(usize::try_from(event_batch.written).unwrap());
        let bleat = events
            .iter()
            .find(|event| {
                event.kind == MC_EVENT_GOAT_SOUND_REQUEST
                    && event.entity_index == BLEATING_GOAT
                    && event.code == 46
            })
            .expect("production Goat action emits sound 46 before slot 113");
        let mut audio = McAudioCommandV2::default();
        assert_eq!(
            mc_native_audio_command_for_event(
                handle,
                after.generation,
                bleat.sequence,
                &raw mut audio,
                struct_size::<McAudioCommandV2>(),
            ),
            MC_STATUS_OK
        );
        audio
    }

    fn production_goat_pitch_case(retain_action_14: bool) -> McAudioCommandV2 {
        const BLEATING_GOAT: u16 = 102;
        let handle = configured_goat_pitch_handle(retain_action_14);
        let audio = goat_pitch_audio_after_step(handle);
        with_session(handle, |session| {
            let goat = session
                .live_world
                .goats()
                .iter()
                .find(|goat| goat.entity_index == BLEATING_GOAT)
                .expect("source-derived Goat slot 102 remains projected");
            assert_eq!(session.mission.global_rng(), 0x43e9_ccaf);
            assert_eq!(goat.action == 14, retain_action_14);
            Ok(())
        })
        .expect("inspect the post-turn source discriminator");
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
        audio
    }

    #[test]
    fn goat_sound_46_peeks_the_slot_102_stored_rng_without_consuming_it() {
        const BLEATING_GOAT: u16 = 102;
        let handle = open_fixture();
        let (audio, stored_rng) = with_session_mut(handle, |session| {
            let listener = session.state.position;
            let mut entities = session.world.entities().to_vec();
            let goat = &mut entities[usize::from(BLEATING_GOAT)];
            goat.action = 14;
            goat.position = listener;
            session.live_world = LiveWorldState::from_entities(
                &entities,
                session.world.goat_stage_point(),
                session.world.goat_config(),
            )
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
            session
                .live_world
                .inject_damage(
                    BLEATING_GOAT,
                    1,
                    i16::try_from(session.state.entity_index).unwrap(),
                )
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;

            session.mission.begin_source_turn_at(listener);
            let stored_rng = session.mission.global_rng();
            assert_eq!(stored_rng, 0x9909_f858);
            let events = session.live_world.step_source_slot(
                BLEATING_GOAT,
                &session.terrain,
                &[GoatPlayerTarget {
                    entity_index: session.state.entity_index,
                    position: listener,
                    control_flags: 0,
                }],
            );
            assert_eq!(session.mission.global_rng(), stored_rng);
            let bleat = events
                .into_iter()
                .find(|event| matches!(event, GoatStepEvent::Bleat { sound_id: 46, .. }))
                .ok_or_else(|| {
                    AbiError::new(MC_STATUS_VALIDATION_FAILED, "slot 102 did not bleat")
                })?;
            let event = session.encode_goat_event(bleat, session.live_world_hash_v2(), stored_rng);
            let audio = *session.audio_commands.get(&event.sequence).ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "slot 102 bleat has no audio command",
                )
            })?;
            assert_eq!(session.mission.global_rng(), stored_rng);
            Ok((audio, stored_rng))
        })
        .unwrap();
        assert_eq!(audio.rate_delta_percent, 25);
        let nonstored = stored_rng.wrapping_mul(9_377).wrapping_add(9_439);
        assert_eq!(nonstored, 0xa631_b437);
        let other_action = prepare_event_sound_command(
            Position::default(),
            0,
            Position::default(),
            46,
            stored_rng,
            BLEATING_GOAT,
            0,
        )
        .unwrap();
        assert_eq!(other_action.command.rate_delta_percent, 5);
        assert_eq!(stored_rng, 0x9909_f858, "the pitch peek is never stored");
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn dynamic_house_resident_overlays_the_source_entity_projection() {
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("package M loads");
        let mut session = SessionRecord::new(world).expect("fixture session constructs");
        let position = Position {
            x: 12_345,
            y: 23_456,
            z: 640,
        };
        let source_slot = session
            .mission
            .spawn_house_resident(position, 4, 33, session.state.entity_index)
            .expect("qualified Archer resident constructor")
            .expect("fixture source pool has one allocatable row");
        let entity = &session.world.entities()[usize::from(source_slot)];
        let snapshot = entity_snapshot(
            entity,
            &session.state,
            &session.live_world,
            &session.mission,
        );
        assert_eq!(snapshot.entity_index, source_slot);
        assert_eq!(snapshot.authority, MC_AUTHORITY_LEVEL1_MISSION_LIVE);
        assert_eq!(snapshot.active, 1);
        assert_eq!(
            (snapshot.class_id, snapshot.model, snapshot.action),
            (5, 4, 33)
        );
        assert_eq!((snapshot.x, snapshot.y, snapshot.z), (12_345, 23_456, 640));
        assert_eq!(snapshot.parameter_record_index, 75);
    }

    fn open_fixture_with_terrain_frame_assets() -> u64 {
        let world = presentation_test_world();
        let mut session = SessionRecord::new(world).expect("fixture session constructs");
        let assets = Arc::new(terrain_frame_assets_fixture());
        session.archer_selector_frames =
            Some(ArcherTmapClocks::from_bank(&assets.archer_tmap_bank));
        session.firefly_selector_frames =
            Some(FireflyTmapClocks::from_bank(&assets.firefly_tmap_bank));
        session.villager_selector_frames = Some(VillagerTmapClocks::from_bank(
            &assets.frozen_actor_tmap_bank,
        ));
        session.fireball_selector_frame =
            Some(FireballTmapClock::from_bank(&assets.combat_tmap_bank));
        session.possession_selector_frame =
            Some(PossessionTmapClock::from_bank(&assets.combat_tmap_bank));
        session.castle_selector_frame = Some(CastleTmapClock::from_bank(&assets.economy_tmap_bank));
        session.source_render_asset_digest = Some(
            source_render_asset_digest(session.world.as_ref(), &assets)
                .expect("fixture source-render asset digest"),
        );
        session.terrain_frame_assets = Some(assets);
        with_registry_mut(|registry| registry.insert(session)).expect("fixture session registers")
    }

    fn presentation_test_cache() -> PathBuf {
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        resolve_level1_presentation_cache_root(private)
    }

    fn presentation_test_world() -> NativeWorld {
        let (package, closure) = fixture_paths();
        NativeWorld::load_with_owner_cache(package, closure, presentation_test_cache())
            .expect("package M and manifest/hash-qualified presentation cache load")
    }

    fn terrain_frame_assets_fixture() -> TerrainFrameAssets {
        load_terrain_frame_assets(&presentation_test_cache())
            .expect("manifest/hash-qualified presentation assets load")
    }

    fn open_fixture_with_owner_cache() -> u64 {
        let world = presentation_test_world();
        let session = SessionRecord::new(world).expect("owner-cache fixture session constructs");
        with_registry_mut(|registry| registry.insert(session))
            .expect("owner-cache fixture session registers")
    }

    fn open_fixture_with_presentation_cache() -> u64 {
        let world = presentation_test_world();
        let mut session = SessionRecord::new(world).expect("presentation session constructs");
        let assets = Arc::new(terrain_frame_assets_fixture());
        session.selector_frames = session
            .world
            .goat_tmap_bank()
            .map(GoatTmapClocks::from_bank);
        session.archer_selector_frames =
            Some(ArcherTmapClocks::from_bank(&assets.archer_tmap_bank));
        session.firefly_selector_frames =
            Some(FireflyTmapClocks::from_bank(&assets.firefly_tmap_bank));
        session.villager_selector_frames = Some(VillagerTmapClocks::from_bank(
            &assets.frozen_actor_tmap_bank,
        ));
        session.fireball_selector_frame =
            Some(FireballTmapClock::from_bank(&assets.combat_tmap_bank));
        session.possession_selector_frame =
            Some(PossessionTmapClock::from_bank(&assets.combat_tmap_bank));
        session.castle_selector_frame = Some(CastleTmapClock::from_bank(&assets.economy_tmap_bank));
        session.source_render_asset_digest = Some(
            source_render_asset_digest(session.world.as_ref(), &assets)
                .expect("fixture source-render asset digest"),
        );
        session.terrain_frame_assets = Some(assets);
        with_registry_mut(|registry| registry.insert(session))
            .expect("presentation session registers")
    }

    fn open_fixture_with_owner_cache_and_terrain_frame_assets() -> u64 {
        let world = presentation_test_world();
        let mut session =
            SessionRecord::new(world).expect("owner-cache fixture session constructs");
        let assets = Arc::new(terrain_frame_assets_fixture());
        session.archer_selector_frames =
            Some(ArcherTmapClocks::from_bank(&assets.archer_tmap_bank));
        session.firefly_selector_frames =
            Some(FireflyTmapClocks::from_bank(&assets.firefly_tmap_bank));
        session.villager_selector_frames = Some(VillagerTmapClocks::from_bank(
            &assets.frozen_actor_tmap_bank,
        ));
        session.fireball_selector_frame =
            Some(FireballTmapClock::from_bank(&assets.combat_tmap_bank));
        session.possession_selector_frame =
            Some(PossessionTmapClock::from_bank(&assets.combat_tmap_bank));
        session.castle_selector_frame = Some(CastleTmapClock::from_bank(&assets.economy_tmap_bank));
        session.source_render_asset_digest = Some(
            source_render_asset_digest(session.world.as_ref(), &assets)
                .expect("fixture source-render asset digest"),
        );
        session.terrain_frame_assets = Some(assets);
        with_registry_mut(|registry| registry.insert(session))
            .expect("complete owner-cache fixture session registers")
    }

    fn open_fixture_with_world_frame_assets() -> u64 {
        let world = presentation_test_world();
        let mut session =
            SessionRecord::new(world).expect("owner-cache fixture session constructs");
        let assets = Arc::new(terrain_frame_assets_fixture());
        session.archer_selector_frames =
            Some(ArcherTmapClocks::from_bank(&assets.archer_tmap_bank));
        session.firefly_selector_frames =
            Some(FireflyTmapClocks::from_bank(&assets.firefly_tmap_bank));
        session.villager_selector_frames = Some(VillagerTmapClocks::from_bank(
            &assets.frozen_actor_tmap_bank,
        ));
        session.fireball_selector_frame =
            Some(FireballTmapClock::from_bank(&assets.combat_tmap_bank));
        session.possession_selector_frame =
            Some(PossessionTmapClock::from_bank(&assets.combat_tmap_bank));
        session.castle_selector_frame = Some(CastleTmapClock::from_bank(&assets.economy_tmap_bank));
        session.source_render_asset_digest = Some(
            source_render_asset_digest(session.world.as_ref(), &assets)
                .expect("fixture source-render asset digest"),
        );
        session.terrain_frame_assets = Some(assets);
        with_registry_mut(|registry| registry.insert(session))
            .expect("world-frame fixture session registers")
    }

    fn open_paths(package: &Path, closure: &Path) -> Result<u64, i32> {
        let package = package.to_string_lossy();
        let closure = closure.to_string_lossy();
        let mut handle = 0_u64;
        let status = mc_native_session_open(
            package.as_ptr(),
            u32::try_from(package.len()).unwrap(),
            closure.as_ptr(),
            u32::try_from(closure.len()).unwrap(),
            &raw mut handle,
        );
        if status == MC_STATUS_OK {
            Ok(handle)
        } else {
            Err(status)
        }
    }

    fn valid_command() -> McCommandV2 {
        McCommandV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCommandV2>(),
            requested_spell: MC_REQUESTED_SPELL_NONE,
            ..McCommandV2::default()
        }
    }

    fn valid_camera_input() -> McCameraInputV2 {
        McCameraInputV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCameraInputV2>(),
            ..McCameraInputV2::default()
        }
    }

    #[test]
    fn layouts_and_versions_are_stable() {
        assert_eq!(mc_native_abi_version(), 2);
        assert_eq!(size_of::<McWorldInfoV2>(), 72);
        assert_eq!(size_of::<McCommandV2>(), 32);
        assert_eq!(size_of::<McCameraInputV2>(), 28);
        assert_eq!(size_of::<McSessionSnapshotV2>(), 120);
        assert_eq!(size_of::<McObjectiveSnapshotV2>(), 40);
        assert_eq!(size_of::<McEntitySnapshotV2>(), 52);
        assert_eq!(size_of::<McCampaignSelectionV2>(), 32);
        assert_eq!(size_of::<McCampaignStepInputV2>(), 80);
        assert_eq!(size_of::<McCampaignEventV2>(), 48);
        assert_eq!(size_of::<McCampaignEconomyEventV2>(), 64);
        assert_eq!(size_of::<McCampaignStepSnapshotV2>(), 112);
        assert_eq!(std::mem::offset_of!(McCampaignStepInputV2, command), 16);
        assert_eq!(std::mem::offset_of!(McCampaignEventV2, source_slot), 24);
        assert_eq!(
            std::mem::offset_of!(McCampaignEconomyEventV2, primary_id),
            28
        );
        assert_eq!(std::mem::offset_of!(McCampaignEconomyEventV2, value_0), 40);
        assert_eq!(
            std::mem::offset_of!(McCampaignStepSnapshotV2, dispatch_hash),
            40
        );
        assert_eq!(size_of::<McCampaignWorldInfoV2>(), 64);
        assert_eq!(size_of::<McCampaignPlayerSnapshotV2>(), 48);
        assert_eq!(size_of::<McCampaignEntitySnapshotV2>(), 52);
        assert_eq!(size_of::<McGoatSnapshotV2>(), 84);
        assert_eq!(size_of::<McGoatBatchV2>(), 64);
        assert_eq!(size_of::<McCampaignManaBatchV2>(), 64);
        assert_eq!(size_of::<McLevel1MissionSnapshotV2>(), 96);
        assert_eq!(size_of::<McLevel1PlayerLifecycleSnapshotV2>(), 144);
        assert_eq!(size_of::<McLevel1HudSpellSlotV2>(), 36);
        assert_eq!(size_of::<McLevel1HudSnapshotV2>(), 128);
        assert_eq!(size_of::<McLevel1SpellMenuSnapshotV2>(), 64);
        assert_eq!(size_of::<McLevel1MinimapMarksV2>(), 168);
        assert_eq!(size_of::<McLevel1MinimapEntityCommandV2>(), 24);
        assert_eq!(size_of::<McLevel1MinimapEntitiesInfoV2>(), 64);
        assert_eq!(size_of::<McLevel1PaletteInfoV2>(), 80);
        assert_eq!(size_of::<McLevel1ExitPresentationV2>(), 64);
        assert_eq!(size_of::<McLevel1CastleSnapshotV2>(), 80);
        assert_eq!(size_of::<McTerrainStateV2>(), 64);
        assert_eq!(size_of::<McLevel1CastleLifecycleSnapshotV2>(), 80);
        assert_eq!(size_of::<McLevel1HouseCompletionEffectV2>(), 80);
        assert_eq!(size_of::<McManaSphereSnapshotV2>(), 64);
        assert_eq!(size_of::<McManaCarrierSnapshotV2>(), 80);
        assert_eq!(size_of::<McLevel1ActorSnapshotV2>(), 56);
        assert_eq!(size_of::<McLevel1MultipartSegmentSnapshotV2>(), 64);
        assert_eq!(size_of::<McFireballProjectileSnapshotV2>(), 56);
        assert_eq!(size_of::<McFireballImpactSnapshotV2>(), 52);
        assert_eq!(size_of::<McSelectorFrameStateV2>(), 64);
        assert_eq!(size_of::<McSelectorFrameBatchV2>(), 88);
        assert_eq!(size_of::<McSelectorDrawFeedbackV2>(), 48);
        assert_eq!(size_of::<McGoatDamageCommandV2>(), 40);
        assert_eq!(size_of::<McAssetRefInfoV2>(), 32);
        assert_eq!(size_of::<McEventQueryV2>(), 40);
        assert_eq!(size_of::<McEventBatchV2>(), 56);
        assert_eq!(size_of::<McEventV2>(), 72);
        assert_eq!(size_of::<McLevel1ResidentObservationV1>(), 40);
        assert_eq!(
            std::mem::offset_of!(McLevel1ResidentObservationV1, digest),
            16
        );
        assert_eq!(size_of::<McAudioCommandV2>(), 88);
        assert_eq!(size_of::<McPresentationStateV2>(), 64);
        assert_eq!(size_of::<McRenderCameraStateV2>(), 72);
        assert_eq!(size_of::<McTerrainPnt5VectorV2>(), 56);
        assert_eq!(size_of::<McTerrainGridVertexV2>(), 64);
        assert_eq!(size_of::<McTerrainFrameQueryV2>(), 40);
        assert_eq!(size_of::<McTerrainFrameInfoV2>(), 96);
        assert_eq!(size_of::<McPresentationFrameQueryV2>(), 64);
        assert_eq!(size_of::<McPresentationFrameInfoV2>(), 128);
        assert_eq!(std::mem::offset_of!(McWorldInfoV2, reset_state_hash), 40);
        assert_eq!(std::mem::offset_of!(McCommandV2, requested_spell), 14);
        assert_eq!(std::mem::offset_of!(McCameraInputV2, look_yaw), 8);
        assert_eq!(std::mem::offset_of!(McCameraInputV2, reserved), 12);
        assert_eq!(std::mem::offset_of!(McSessionSnapshotV2, generation), 8);
        assert_eq!(std::mem::offset_of!(McSessionSnapshotV2, entity_index), 48);
        assert_eq!(
            std::mem::offset_of!(McSessionSnapshotV2, left_spell_holder),
            82
        );
        assert_eq!(
            std::mem::offset_of!(McSessionSnapshotV2, objective_textbox_ticks),
            77
        );
        assert_eq!(
            std::mem::offset_of!(McSessionSnapshotV2, objectives_authority),
            79
        );
        assert_eq!(std::mem::offset_of!(McSessionSnapshotV2, reserved), 104);
        assert_eq!(
            std::mem::offset_of!(McObjectiveSnapshotV2, stage_selectors),
            13
        );
        assert_eq!(std::mem::offset_of!(McObjectiveSnapshotV2, reserved), 24);
        assert_eq!(std::mem::offset_of!(McEntitySnapshotV2, reserved), 36);
        assert_eq!(std::mem::offset_of!(McCampaignSelectionV2, reserved), 16);
        assert_eq!(
            std::mem::offset_of!(McCampaignWorldInfoV2, reset_state_hash),
            40
        );
        assert_eq!(std::mem::offset_of!(McCampaignPlayerSnapshotV2, turn), 16);
        assert_eq!(
            std::mem::offset_of!(McCampaignPlayerSnapshotV2, reserved),
            32
        );
        assert_eq!(
            std::mem::offset_of!(McCampaignEntitySnapshotV2, reserved),
            36
        );
        assert_eq!(std::mem::offset_of!(McGoatSnapshotV2, life), 48);
        assert_eq!(std::mem::offset_of!(McGoatSnapshotV2, reserved), 68);
        assert_eq!(std::mem::offset_of!(McGoatBatchV2, reserved), 48);
        assert_eq!(
            std::mem::offset_of!(McLevel1MissionSnapshotV2, player_life),
            40
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1PlayerLifecycleSnapshotV2, player_action),
            41
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1PlayerLifecycleSnapshotV2, action_timer),
            56
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1PlayerLifecycleSnapshotV2, player_control_flags),
            72
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1PlayerLifecycleSnapshotV2, source_pool_rebuild_count),
            108
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1PlayerLifecycleSnapshotV2, marker_dispatch_phase),
            122
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1PlayerLifecycleSnapshotV2, source_turn_counter),
            128
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1PlayerLifecycleSnapshotV2, effective_pitch),
            134
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1PlayerLifecycleSnapshotV2, marker_constructor_phase),
            136
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1MissionSnapshotV2, actor_count),
            64
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1MissionSnapshotV2, possession_experience),
            84
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1MissionSnapshotV2, last_possessed_building_entity),
            88
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1MissionSnapshotV2, possessed_building_count),
            92
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1MissionSnapshotV2, speed_trail_count),
            95
        );
        assert_eq!(std::mem::offset_of!(McLevel1HudSpellSlotV2, max_mana), 12);
        assert_eq!(
            std::mem::offset_of!(McLevel1HudSnapshotV2, global_mana_denominator),
            32
        );
        assert_eq!(std::mem::offset_of!(McLevel1HudSnapshotV2, left), 48);
        assert_eq!(std::mem::offset_of!(McLevel1HudSnapshotV2, right), 84);
        assert_eq!(
            std::mem::offset_of!(McLevel1HudSnapshotV2, selected_holder_entity_index),
            120
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1SpellMenuSnapshotV2, family_enabled),
            32
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1SpellMenuSnapshotV2, holder_entity_index),
            48
        );
        assert_eq!(std::mem::offset_of!(McLevel1MinimapMarksV2, mark_x), 28);
        assert_eq!(std::mem::offset_of!(McLevel1MinimapMarksV2, arrow_xy), 124);
        assert_eq!(std::mem::offset_of!(McLevel1MinimapMarksV2, reserved), 152);
        assert_eq!(
            std::mem::offset_of!(McLevel1MinimapEntityCommandV2, reserved),
            16
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1MinimapEntitiesInfoV2, command_hash),
            40
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1MinimapEntitiesInfoV2, reserved),
            48
        );
        assert_eq!(std::mem::offset_of!(McLevel1PaletteInfoV2, rgb8_sha256), 32);
        assert_eq!(std::mem::offset_of!(McLevel1PaletteInfoV2, reserved), 64);
        assert_eq!(
            std::mem::offset_of!(McLevel1ExitPresentationV2, player_action),
            25
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1ExitPresentationV2, camera_fov),
            36
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1ExitPresentationV2, reserved),
            48
        );
        assert_eq!(std::mem::offset_of!(McLevel1CastleSnapshotV2, life), 48);
        assert_eq!(std::mem::offset_of!(McLevel1CastleSnapshotV2, reserved), 68);
        assert_eq!(
            std::mem::offset_of!(McTerrainStateV2, mutation_sequence),
            24
        );
        assert_eq!(std::mem::offset_of!(McTerrainStateV2, width), 40);
        assert_eq!(std::mem::offset_of!(McTerrainStateV2, reserved), 48);
        assert_eq!(
            std::mem::offset_of!(McLevel1CastleLifecycleSnapshotV2, root_life),
            44
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1CastleLifecycleSnapshotV2, construction_timer),
            64
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1CastleLifecycleSnapshotV2, terrain_mutation_sequence),
            72
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1HouseCompletionEffectV2, parent_word_2e),
            44
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1HouseCompletionEffectV2, max_life),
            64
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1HouseCompletionEffectV2, effect_count),
            76
        );
        assert_eq!(std::mem::offset_of!(McManaSphereSnapshotV2, mana), 44);
        assert_eq!(std::mem::offset_of!(McManaSphereSnapshotV2, reserved), 52);
        assert_eq!(
            std::mem::offset_of!(McManaCarrierSnapshotV2, carried_mana),
            60
        );
        assert_eq!(std::mem::offset_of!(McManaCarrierSnapshotV2, reserved), 64);
        assert_eq!(
            std::mem::offset_of!(McLevel1ActorSnapshotV2, authored_index),
            8
        );
        assert_eq!(std::mem::offset_of!(McLevel1ActorSnapshotV2, max_life), 28);
        assert_eq!(std::mem::offset_of!(McLevel1ActorSnapshotV2, base_tmap), 40);
        assert_eq!(std::mem::offset_of!(McLevel1ActorSnapshotV2, yaw), 44);
        assert_eq!(
            std::mem::offset_of!(McLevel1ActorSnapshotV2, state_value),
            48
        );
        assert_eq!(std::mem::offset_of!(McLevel1ActorSnapshotV2, reserved), 52);
        assert_eq!(
            std::mem::offset_of!(McLevel1MultipartSegmentSnapshotV2, x),
            26
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1MultipartSegmentSnapshotV2, life),
            40
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1MultipartSegmentSnapshotV2, base_tmap),
            50
        );
        assert_eq!(
            std::mem::offset_of!(McLevel1MultipartSegmentSnapshotV2, reserved),
            60
        );
        assert_eq!(std::mem::offset_of!(McFireballProjectileSnapshotV2, x), 20);
        assert_eq!(
            std::mem::offset_of!(McFireballProjectileSnapshotV2, payload),
            36
        );
        assert_eq!(
            std::mem::offset_of!(McFireballProjectileSnapshotV2, logical_height),
            40
        );
        assert_eq!(
            std::mem::offset_of!(McFireballProjectileSnapshotV2, reserved),
            48
        );
        assert_eq!(std::mem::offset_of!(McFireballImpactSnapshotV2, x), 24);
        assert_eq!(
            std::mem::offset_of!(McFireballImpactSnapshotV2, payload),
            32
        );
        assert_eq!(
            std::mem::offset_of!(McFireballImpactSnapshotV2, base_tmap),
            36
        );
        assert_eq!(
            std::mem::offset_of!(McFireballImpactSnapshotV2, reserved),
            44
        );
        assert_eq!(std::mem::offset_of!(McSelectorFrameStateV2, pixel_hash), 40);
        assert_eq!(std::mem::offset_of!(McSelectorFrameStateV2, reserved), 48);
        assert_eq!(
            std::mem::offset_of!(McSelectorFrameBatchV2, start_ordinal),
            56
        );
        assert_eq!(std::mem::offset_of!(McSelectorFrameBatchV2, reserved), 72);
        assert_eq!(std::mem::offset_of!(McSelectorDrawFeedbackV2, reserved), 32);
        assert_eq!(std::mem::offset_of!(McGoatDamageCommandV2, reserved), 24);
        assert_eq!(std::mem::offset_of!(McEventV2, state_hash), 24);
        assert_eq!(std::mem::offset_of!(McEventV2, reserved), 56);
        assert_eq!(std::mem::offset_of!(McAudioCommandV2, playback_rate_hz), 48);
        assert_eq!(std::mem::offset_of!(McAudioCommandV2, sound_id), 52);
        assert_eq!(std::mem::offset_of!(McAudioCommandV2, volume_q15), 60);
        assert_eq!(std::mem::offset_of!(McAudioCommandV2, play_type), 68);
        assert_eq!(std::mem::offset_of!(McAudioCommandV2, reserved), 72);
        assert_eq!(std::mem::offset_of!(McPresentationStateV2, source_turn), 24);
        assert_eq!(std::mem::offset_of!(McPresentationStateV2, camera_x), 28);
        assert_eq!(
            std::mem::offset_of!(McPresentationStateV2, camera_authority),
            42
        );
        assert_eq!(std::mem::offset_of!(McPresentationStateV2, reserved), 48);
        assert_eq!(std::mem::offset_of!(McRenderCameraStateV2, history_x), 28);
        assert_eq!(
            std::mem::offset_of!(McRenderCameraStateV2, slope_target_x),
            42
        );
        assert_eq!(
            std::mem::offset_of!(McRenderCameraStateV2, render_offset_x),
            46
        );
        assert_eq!(std::mem::offset_of!(McRenderCameraStateV2, reserved), 56);
        assert_eq!(std::mem::offset_of!(McTerrainPnt5VectorV2, pnt5), 36);
        assert_eq!(std::mem::offset_of!(McTerrainGridVertexV2, view_x), 16);
        assert_eq!(
            std::mem::offset_of!(McTerrainGridVertexV2, feature_flags),
            44
        );
        assert_eq!(std::mem::offset_of!(McTerrainGridVertexV2, reserved), 48);
        assert_eq!(std::mem::offset_of!(McTerrainFrameQueryV2, reserved), 24);
        assert_eq!(std::mem::offset_of!(McTerrainFrameInfoV2, width), 24);
        assert_eq!(std::mem::offset_of!(McTerrainFrameInfoV2, index_bytes), 40);
        assert_eq!(std::mem::offset_of!(McTerrainFrameInfoV2, index_hash), 64);
        assert_eq!(std::mem::offset_of!(McTerrainFrameInfoV2, reserved), 88);
        assert_eq!(
            std::mem::offset_of!(McPresentationFrameQueryV2, configured_blur_mode),
            40
        );
        assert_eq!(
            std::mem::offset_of!(McPresentationFrameInfoV2, immutable_input_hash),
            32
        );
        assert_eq!(
            std::mem::offset_of!(McPresentationFrameInfoV2, index_hash),
            64
        );
        assert_eq!(
            std::mem::offset_of!(McPresentationFrameInfoV2, reserved),
            104
        );

        let weak_marker = FireballImpact {
            id: 1,
            source_entity_index: 2,
            source_projectile_id: 3,
            owner_entity_index: 152,
            external_target_entity_index: 0,
            model: POSSESSION_IMPACT_MODEL,
            position: Position {
                x: 20_000,
                y: 19_616,
                z: 128,
            },
            payload: 0,
            life_ticks: 8,
            entity_rng: 0,
            vertical_velocity: 0,
            phase: 77,
            animation_frame: 1,
            damage_submitted: false,
            active: true,
        };
        let strong_marker = FireballImpact {
            model: POSSESSION_STRONG_IMPACT_MODEL,
            payload: 1,
            ..weak_marker
        };
        let weak_snapshot = fireball_impact_snapshot(&weak_marker);
        let strong_snapshot = fireball_impact_snapshot(&strong_marker);
        let ground_fire_rows = [0_u32, 1, 2, 3, 4, 5, 6, 7, 6, 5, 4, 3, 2, 1, 0]
            .into_iter()
            .map(|phase| {
                let snapshot = fireball_impact_snapshot(&FireballImpact {
                    model: GROUND_FIRE_MODEL,
                    payload: 50,
                    phase,
                    animation_frame: 0,
                    ..weak_marker
                });
                (
                    snapshot.render_parameter_index,
                    snapshot.logical_height,
                    snapshot.base_tmap,
                    snapshot.render_mode,
                    snapshot.animation_frame,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            ground_fire_rows,
            [
                (228, 50, 77, 1, 0),
                (229, 150, 77, 1, 0),
                (230, 300, 77, 1, 0),
                (231, 450, 77, 1, 0),
                (232, 600, 77, 1, 0),
                (233, 700, 77, 1, 0),
                (234, 800, 77, 1, 0),
                (235, 950, 77, 1, 0),
                (234, 800, 77, 1, 0),
                (233, 700, 77, 1, 0),
                (232, 600, 77, 1, 0),
                (231, 450, 77, 1, 0),
                (230, 300, 77, 1, 0),
                (229, 150, 77, 1, 0),
                (228, 50, 77, 1, 0),
            ],
            "model 6 projects sub_31760 expansion and contraction through rows 228..235"
        );
        assert_eq!(
            (
                strong_snapshot.class_id,
                strong_snapshot.base_tmap,
                strong_snapshot.render_parameter_index,
                strong_snapshot.render_mode,
                strong_snapshot.logical_height,
            ),
            (
                weak_snapshot.class_id,
                weak_snapshot.base_tmap,
                weak_snapshot.render_parameter_index,
                weak_snapshot.render_mode,
                weak_snapshot.logical_height,
            ),
            "model 70 exports the same possession marker tuple as model 12"
        );
        assert_eq!(
            (
                strong_snapshot.model,
                strong_snapshot.payload,
                weak_snapshot.animation_frame,
                strong_snapshot.animation_frame,
            ),
            (POSSESSION_STRONG_IMPACT_MODEL, 1, 1, 1),
            "CAPI projects the bounded render frame, never class-10 dword phase"
        );
        let attraction_snapshot = fireball_impact_snapshot(&FireballImpact {
            model: POSSESSION_ATTRACTION_MODEL,
            payload: i32::try_from(
                sim_core::level1_mission::POSSESSION_ATTRACTION_TIER_TWO_RADIUS_SQUARED,
            )
            .unwrap(),
            phase: 128,
            animation_frame: 0,
            ..weak_marker
        });
        assert_eq!(
            (
                attraction_snapshot.model,
                attraction_snapshot.class_id,
                attraction_snapshot.render_parameter_index,
                attraction_snapshot.base_tmap,
                attraction_snapshot.logical_height,
                attraction_snapshot.render_mode,
                attraction_snapshot.animation_frame,
                attraction_snapshot.reserved,
            ),
            (
                POSSESSION_ATTRACTION_MODEL,
                FIREBALL_IMPACT_CLASS,
                POSSESSION_ATTRACTION_RENDER_INDEX,
                POSSESSION_ATTRACTION_BASE_TMAP,
                POSSESSION_ATTRACTION_LOGICAL_HEIGHT,
                POSSESSION_ATTRACTION_RENDER_MODE,
                0,
                [2, 0],
            ),
            "model 54 remains in gameplay inventory with its literal row-zero tuple"
        );
    }

    #[test]
    fn campaign_economy_event_kind_is_publicly_bound() {
        let event = Level2DispatchEvent::Economy {
            pass: 2,
            source_slot: 157,
            event: Level2EconomyEvent::ManaSphereSpawned {
                sphere_id: 157,
                source_entity_index: 39,
                mana: 321,
            },
        };
        let snapshot = campaign_event_snapshot(4, 7, &event);
        assert_eq!(MC_CAMPAIGN_EVENT_ECONOMY, 10);
        assert_eq!(snapshot.kind, MC_CAMPAIGN_EVENT_ECONOMY);
        assert_eq!(snapshot.family, 1);
        assert_eq!(snapshot.source_slot, 157);
        assert_eq!(snapshot.ordinal, 7);
    }

    #[test]
    fn campaign_economy_snapshot_binds_castle_defender_sphere_collection() {
        let snapshot = campaign_economy_event_snapshot(
            11,
            19,
            3,
            157,
            Level2EconomyEvent::CastleDefenderCollectedSphere {
                defender_slot: 157,
                sphere_id: 39,
                mana: 321,
            },
        );

        assert_eq!(
            snapshot.code,
            MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_COLLECTED_SPHERE
        );
        assert_eq!(snapshot.family, 3);
        assert_eq!(snapshot.source_slot, 157);
        assert_eq!(snapshot.primary_id, 157);
        assert_eq!(snapshot.secondary_id, 39);
        assert_eq!(snapshot.entity_0, 157);
        assert_eq!(snapshot.entity_1, 0);
        assert_eq!(snapshot.value_0, 321);
        assert_eq!(snapshot.value_1, 0);
        assert_eq!(snapshot.flags, 0);
        assert_eq!(snapshot.reserved, [0; 4]);
    }

    #[test]
    fn campaign_economy_snapshot_binds_castle_defender_cave_sound() {
        let snapshot = campaign_economy_event_snapshot(
            11,
            20,
            3,
            157,
            Level2EconomyEvent::CastleDefenderCaveSound {
                defender_slot: 157,
                sound_id: 22,
            },
        );
        assert_eq!(
            snapshot.code,
            MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_CAVE_SOUND
        );
        assert_eq!(snapshot.family, 3);
        assert_eq!(snapshot.source_slot, 157);
        assert_eq!(snapshot.primary_id, 157);
        assert_eq!(snapshot.secondary_id, 0);
        assert_eq!(snapshot.entity_0, 22);
        assert_eq!(snapshot.value_0, 0);
        assert_eq!(snapshot.value_1, 0);
        assert_eq!(snapshot.flags, 0);
        assert_eq!(snapshot.reserved, [0; 4]);
    }

    #[test]
    fn campaign_api_selects_level_profiles_and_preserves_old_level1_values() {
        let (level1_package, level1_closure) = fixture_paths();
        let package = level1_package.to_string_lossy();
        let closure = level1_closure.to_string_lossy();
        let selection = McCampaignSelectionV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCampaignSelectionV2>(),
            external_level: 1,
            internal_level: 0,
            map_type: 1,
            ..McCampaignSelectionV2::default()
        };
        let mut level1_handle = 0;
        assert_eq!(
            mc_native_campaign_session_open(
                package.as_ptr(),
                package.len() as u32,
                closure.as_ptr(),
                closure.len() as u32,
                &raw const selection,
                &raw mut level1_handle,
            ),
            MC_STATUS_OK
        );
        assert_ne!(level1_handle, 0);
        let mut info = McCampaignWorldInfoV2::default();
        assert_eq!(
            mc_native_campaign_world_info(
                level1_handle,
                &raw mut info,
                struct_size::<McCampaignWorldInfoV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            (info.external_level, info.internal_level, info.map_type),
            (1, 0, 1)
        );
        assert_eq!(info.player_count, 8);
        let mut players = [McCampaignPlayerSnapshotV2::default(); 8];
        let mut written = 0;
        assert_eq!(
            mc_native_campaign_players_copy(
                level1_handle,
                0,
                players.as_mut_ptr(),
                players.len() as u32,
                struct_size::<McCampaignPlayerSnapshotV2>(),
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 8);
        assert_eq!(players[0].active, 1);
        assert_eq!(players[1].active, 0);
        let old_handle = open_paths(&level1_package, &level1_closure).expect("Level 1 open");
        let mut old_info = McWorldInfoV2::default();
        assert_eq!(
            mc_native_world_info(
                old_handle,
                &raw mut old_info,
                struct_size::<McWorldInfoV2>()
            ),
            MC_STATUS_OK
        );
        assert_eq!(info.entity_count, old_info.entity_count);
        assert_eq!(info.active_entity_count, old_info.active_entity_count);
        assert_eq!(info.player_count, old_info.player_count);
        assert_eq!(info.reset_state_hash, old_info.reset_state_hash);
        let mut old_objectives = [McObjectiveSnapshotV2::default(); 8];
        let mut old_objective_written = 0;
        assert_eq!(
            mc_native_objectives_copy(
                old_handle,
                0,
                old_objectives.as_mut_ptr(),
                8,
                struct_size::<McObjectiveSnapshotV2>(),
                &raw mut old_objective_written,
            ),
            MC_STATUS_OK
        );
        let mut generic_objectives = [McObjectiveSnapshotV2::default(); 8];
        let mut generic_objective_written = 0;
        assert_eq!(
            mc_native_campaign_objectives_copy(
                level1_handle,
                0,
                generic_objectives.as_mut_ptr(),
                8,
                struct_size::<McObjectiveSnapshotV2>(),
                &raw mut generic_objective_written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(old_objective_written, generic_objective_written);
        assert_eq!(old_objectives, generic_objectives);
        let mut old_entities = [McEntitySnapshotV2::default(); 4];
        let mut old_entity_written = 0;
        assert_eq!(
            mc_native_entities_copy(
                old_handle,
                0,
                old_entities.as_mut_ptr(),
                4,
                struct_size::<McEntitySnapshotV2>(),
                &raw mut old_entity_written,
            ),
            MC_STATUS_OK
        );
        let mut generic_entities = [McCampaignEntitySnapshotV2::default(); 4];
        let mut generic_entity_written = 0;
        assert_eq!(
            mc_native_campaign_entities_copy(
                level1_handle,
                0,
                generic_entities.as_mut_ptr(),
                4,
                struct_size::<McCampaignEntitySnapshotV2>(),
                &raw mut generic_entity_written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(old_entity_written, generic_entity_written);
        for (old, generic) in old_entities.iter().zip(generic_entities.iter()) {
            assert_eq!(old.entity_index, generic.entity_index);
            assert_eq!(old.class_id, generic.class_id);
            assert_eq!(old.model, generic.model);
            assert_eq!(old.action, generic.action);
            assert_eq!(old.phase, generic.phase);
            assert_eq!((old.x, old.y, old.z), (generic.x, generic.y, generic.z));
        }
        let mut level1_mana_batch = McCampaignManaBatchV2::default();
        let mut level1_mana_rows = [McManaSphereSnapshotV2::default(); 1];
        assert_eq!(
            mc_native_campaign_mana_spheres_copy(
                level1_handle,
                0,
                0,
                level1_mana_rows.as_mut_ptr(),
                1,
                struct_size::<McManaSphereSnapshotV2>(),
                &raw mut level1_mana_batch,
                struct_size::<McCampaignManaBatchV2>(),
            ),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(mc_native_session_destroy(old_handle), MC_STATUS_OK);
        assert_eq!(
            mc_native_campaign_session_destroy(level1_handle),
            MC_STATUS_OK
        );

        let level2_package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/l2-debug3.fJ3PaM/level2");
        let package = level2_package.to_string_lossy();
        let selection = McCampaignSelectionV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCampaignSelectionV2>(),
            external_level: 2,
            internal_level: 1,
            map_type: 0,
            ..McCampaignSelectionV2::default()
        };
        let mut level2_handle = 0;
        assert_eq!(
            mc_native_campaign_session_open(
                package.as_ptr(),
                package.len() as u32,
                std::ptr::null(),
                0,
                &raw const selection,
                &raw mut level2_handle,
            ),
            MC_STATUS_OK
        );
        let mut level2_info = McCampaignWorldInfoV2::default();
        assert_eq!(
            mc_native_campaign_world_info(
                level2_handle,
                &raw mut level2_info,
                struct_size::<McCampaignWorldInfoV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            (
                level2_info.external_level,
                level2_info.internal_level,
                level2_info.map_type
            ),
            (2, 1, 0)
        );
        assert_eq!(level2_info.active_entity_count, 114);
        assert_eq!(level2_info.player_count, 8);
        // GOLDEN-CHANGE: authored Level 2 group-8/22/27 source construction
        // is now part of the reset-state projection (aa07b82).
        assert_eq!(level2_info.reset_state_hash, 0x722c_e919_153b_78d8);
        let mut level2_players = [McCampaignPlayerSnapshotV2::default(); 8];
        assert_eq!(
            mc_native_campaign_players_copy(
                level2_handle,
                0,
                level2_players.as_mut_ptr(),
                8,
                struct_size::<McCampaignPlayerSnapshotV2>(),
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(level2_players[0].active + level2_players[1].active, 2);
        let mut entities = [McCampaignEntitySnapshotV2::default(); 4];
        assert_eq!(
            mc_native_campaign_entities_copy(
                level2_handle,
                0,
                entities.as_mut_ptr(),
                4,
                struct_size::<McCampaignEntitySnapshotV2>(),
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 4);
        assert!(
            entities
                .iter()
                .any(|entity| entity.next_relation != u16::MAX)
        );
        assert_eq!(
            mc_native_campaign_session_reset(level2_handle),
            MC_STATUS_OK
        );
        let mut level2_reset_info = McCampaignWorldInfoV2::default();
        assert_eq!(
            mc_native_campaign_world_info(
                level2_handle,
                &raw mut level2_reset_info,
                struct_size::<McCampaignWorldInfoV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            level2_reset_info.reset_state_hash,
            level2_info.reset_state_hash
        );
        assert_eq!(
            mc_native_campaign_session_destroy(level2_handle),
            MC_STATUS_OK
        );
    }

    #[test]
    fn campaign_api_rejects_unknown_profile_and_short_or_reserved_selection_without_handle() {
        let (package, closure) = fixture_paths();
        let package = package.to_string_lossy();
        let closure = closure.to_string_lossy();
        let mut selection = McCampaignSelectionV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCampaignSelectionV2>(),
            external_level: 9,
            internal_level: 9,
            map_type: 9,
            ..McCampaignSelectionV2::default()
        };
        let mut handle = 42;
        assert_eq!(
            mc_native_campaign_session_open(
                package.as_ptr(),
                package.len() as u32,
                closure.as_ptr(),
                closure.len() as u32,
                &raw const selection,
                &raw mut handle,
            ),
            MC_STATUS_VALIDATION_FAILED
        );
        assert_eq!(handle, 0);
        selection.external_level = 1;
        selection.internal_level = 0;
        selection.map_type = 1;
        selection.reserved_0 = 1;
        handle = 42;
        assert_eq!(
            mc_native_campaign_session_open(
                package.as_ptr(),
                package.len() as u32,
                closure.as_ptr(),
                closure.len() as u32,
                &raw const selection,
                &raw mut handle,
            ),
            MC_STATUS_RESERVED_NONZERO
        );
        assert_eq!(handle, 0);
        selection.reserved_0 = 0;
        selection.struct_size = 8;
        handle = 42;
        assert_eq!(
            mc_native_campaign_session_open(
                package.as_ptr(),
                package.len() as u32,
                closure.as_ptr(),
                closure.len() as u32,
                &raw const selection,
                &raw mut handle,
            ),
            MC_STATUS_STRUCT_TOO_SMALL
        );
        assert_eq!(handle, 0);
    }

    #[test]
    fn level2_campaign_step_is_revisioned_reset_safe_and_keeps_level1_abi_untouched() {
        let level2_package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/l2-debug3.fJ3PaM/level2");
        let package = level2_package.to_string_lossy();
        let selection = McCampaignSelectionV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCampaignSelectionV2>(),
            external_level: 2,
            internal_level: 1,
            map_type: 0,
            ..McCampaignSelectionV2::default()
        };
        let mut handle = 0;
        assert_eq!(
            mc_native_campaign_session_open(
                package.as_ptr(),
                package.len() as u32,
                std::ptr::null(),
                0,
                &raw const selection,
                &raw mut handle,
            ),
            MC_STATUS_OK
        );
        let mut initial_info = McCampaignWorldInfoV2::default();
        assert_eq!(
            mc_native_campaign_world_info(
                handle,
                &raw mut initial_info,
                struct_size::<McCampaignWorldInfoV2>(),
            ),
            MC_STATUS_OK
        );
        let mut initial_goats = [McGoatSnapshotV2::default(); 36];
        let mut initial_goat_batch = McGoatBatchV2::default();
        assert_eq!(
            mc_native_campaign_goats_copy(
                handle,
                0,
                0,
                initial_goats.as_mut_ptr(),
                initial_goats.len() as u32,
                struct_size::<McGoatSnapshotV2>(),
                &raw mut initial_goat_batch,
                struct_size::<McGoatBatchV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(initial_goat_batch.written, 36);
        assert_eq!(initial_goat_batch.total_count, 36);
        assert_eq!(initial_goat_batch.has_more, 0);
        let initial_action11_slots = initial_goats
            .iter()
            .filter(|goat| goat.action == 11)
            .map(|goat| goat.entity_index)
            .collect::<BTreeSet<_>>();
        assert_eq!(initial_action11_slots, BTreeSet::from([39, 71]));
        let mut initial_spheres = [McManaSphereSnapshotV2::default(); 64];
        let mut initial_sphere_batch = McCampaignManaBatchV2::default();
        assert_eq!(
            mc_native_campaign_mana_spheres_copy(
                handle,
                0,
                0,
                initial_spheres.as_mut_ptr(),
                initial_spheres.len() as u32,
                struct_size::<McManaSphereSnapshotV2>(),
                &raw mut initial_sphere_batch,
                struct_size::<McCampaignManaBatchV2>(),
            ),
            MC_STATUS_OK
        );
        // The qualified reset fixture has no authored model-39 sphere row;
        // the positive route below allocates one through the production
        // `TransformEntityToManaSphere` projection before copying again.
        assert_eq!(initial_sphere_batch.written, 0);
        assert_eq!(initial_sphere_batch.total_count, 0);
        assert_eq!(initial_sphere_batch.generation, 0);
        assert_eq!(initial_sphere_batch.tick, 0);
        let (sphere_id, carrier_slots) = with_campaign_session_mut(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &mut session.kind else {
                unreachable!("campaign fixture opened as Level 2")
            };
            let sphere_id = reset
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
                    111,
                )
                .expect("Level 2 mana sphere allocation");
            let first = reset
                .economy_mut()
                .spawn_mana_carrier(Position::default(), 111, 0)
                .expect("first Level 2 carrier allocation");
            let second = reset
                .economy_mut()
                .spawn_mana_carrier(Position::default(), 112, 0)
                .expect("second Level 2 carrier allocation");
            Ok((sphere_id, [first, second]))
        })
        .expect("install Level 2 sphere and carrier rows");
        assert_eq!(
            mc_native_campaign_mana_spheres_copy(
                handle,
                0,
                0,
                initial_spheres.as_mut_ptr(),
                initial_spheres.len() as u32,
                struct_size::<McManaSphereSnapshotV2>(),
                &raw mut initial_sphere_batch,
                struct_size::<McCampaignManaBatchV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(initial_sphere_batch.written, 1);
        assert_eq!(initial_sphere_batch.total_count, 1);
        assert_eq!(initial_spheres[0].sphere_id, sphere_id as u32);
        assert_eq!(
            (
                initial_spheres[0].class_id,
                initial_spheres[0].model,
                initial_spheres[0].action,
                initial_spheres[0].mana,
                initial_spheres[0].active,
            ),
            (10, 39, 41, 321, 1)
        );
        let mut carriers = [McManaCarrierSnapshotV2::default(); 4];
        let mut carrier_batch = McCampaignManaBatchV2::default();
        assert_eq!(
            mc_native_campaign_mana_carriers_copy(
                handle,
                0,
                0,
                carriers.as_mut_ptr(),
                carriers.len() as u32,
                struct_size::<McManaCarrierSnapshotV2>(),
                &raw mut carrier_batch,
                struct_size::<McCampaignManaBatchV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(carrier_batch.written, 2);
        assert_eq!(carrier_batch.total_count, 2);
        assert_eq!(
            carriers[..2]
                .iter()
                .map(|carrier| carrier.reserved[0] as u16)
                .collect::<Vec<_>>(),
            carrier_slots.to_vec()
        );
        assert!(carriers[..2].iter().all(|carrier| carrier.active != 0));
        let mut carrier_page = [McManaCarrierSnapshotV2::default(); 1];
        let mut first_carrier_page = McCampaignManaBatchV2::default();
        assert_eq!(
            mc_native_campaign_mana_carriers_copy(
                handle,
                0,
                0,
                carrier_page.as_mut_ptr(),
                1,
                struct_size::<McManaCarrierSnapshotV2>(),
                &raw mut first_carrier_page,
                struct_size::<McCampaignManaBatchV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(first_carrier_page.written, 1);
        assert_eq!(first_carrier_page.total_count, 2);
        assert_eq!(first_carrier_page.has_more, 1);
        let first_carrier_tick = first_carrier_page.tick;
        let first_carrier_hash = first_carrier_page.live_world_hash;
        let mut second_carrier_page = McCampaignManaBatchV2::default();
        assert_eq!(
            mc_native_campaign_mana_carriers_copy(
                handle,
                0,
                1,
                carrier_page.as_mut_ptr(),
                1,
                struct_size::<McManaCarrierSnapshotV2>(),
                &raw mut second_carrier_page,
                struct_size::<McCampaignManaBatchV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(second_carrier_page.written, 1);
        assert_eq!(second_carrier_page.has_more, 0);
        assert_eq!(second_carrier_page.tick, first_carrier_tick);
        assert_eq!(second_carrier_page.live_world_hash, first_carrier_hash);
        let mut pre_step_info = McCampaignWorldInfoV2::default();
        assert_eq!(
            mc_native_campaign_world_info(
                handle,
                &raw mut pre_step_info,
                struct_size::<McCampaignWorldInfoV2>(),
            ),
            MC_STATUS_OK
        );
        let input_command = McCommandV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCommandV2>(),
            flags: 1,
            requested_spell: MC_REQUESTED_SPELL_NONE,
            ..McCommandV2::default()
        };
        let mut input = McCampaignStepInputV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCampaignStepInputV2>(),
            generation: 0,
            command: input_command,
            player_slot: 0,
            speed_index: 0,
            player_x: 123,
            player_y: 456,
            player_z: 7,
            ..McCampaignStepInputV2::default()
        };
        let mut snapshot = McCampaignStepSnapshotV2::default();
        let mut events = [McCampaignEventV2::default(); 1024];
        let mut event_count = 0;
        let mut rejected_snapshot = McCampaignStepSnapshotV2::default();
        let mut rejected_event_count = 99;
        assert_eq!(
            mc_native_campaign_session_step(
                handle,
                &raw const input,
                &raw mut rejected_snapshot,
                struct_size::<McCampaignStepSnapshotV2>(),
                std::ptr::null_mut(),
                0,
                struct_size::<McCampaignEventV2>(),
                &raw mut rejected_event_count,
            ),
            MC_STATUS_BUFFER_TOO_SMALL
        );
        assert_eq!(rejected_event_count, 0);
        let mut after_rejected_info = McCampaignWorldInfoV2::default();
        assert_eq!(
            mc_native_campaign_world_info(
                handle,
                &raw mut after_rejected_info,
                struct_size::<McCampaignWorldInfoV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            after_rejected_info.reset_state_hash, pre_step_info.reset_state_hash,
            "short event output must not commit the candidate turn"
        );
        assert_eq!(
            mc_native_campaign_session_step(
                handle,
                &raw const input,
                &raw mut snapshot,
                struct_size::<McCampaignStepSnapshotV2>(),
                events.as_mut_ptr(),
                events.len() as u32,
                struct_size::<McCampaignEventV2>(),
                &raw mut event_count,
            ),
            MC_STATUS_OK
        );
        assert_eq!(snapshot.generation, 0);
        assert!(snapshot.revision > 0);
        assert_eq!(snapshot.event_count, event_count);
        assert!(event_count > 0, "Level 2 source pass must expose events");
        assert_ne!(snapshot.state_hash, initial_info.reset_state_hash);
        assert!(
            events[..event_count as usize]
                .iter()
                .all(|event| event.abi_version == MC_NATIVE_ABI_VERSION)
        );
        let goat_slots = events[..event_count as usize]
            .iter()
            .filter(|event| event.kind == MC_CAMPAIGN_EVENT_GOAT)
            .map(|event| event.source_slot)
            .collect::<BTreeSet<_>>();
        assert!(
            goat_slots.contains(&39) && goat_slots.contains(&71),
            "public Level 2 C ABI must expose Goat source slots 39 and 71"
        );
        let mut economy_events = [McCampaignEconomyEventV2::default(); 1024];
        let mut economy_event_count = 0;
        assert_eq!(
            mc_native_campaign_economy_events(
                handle,
                economy_events.as_mut_ptr(),
                economy_events.len() as u32,
                struct_size::<McCampaignEconomyEventV2>(),
                &raw mut economy_event_count,
            ),
            MC_STATUS_OK
        );
        for row in &economy_events[..economy_event_count as usize] {
            assert_eq!(row.abi_version, MC_NATIVE_ABI_VERSION);
            assert_eq!(row.generation, snapshot.generation);
            let generic = events
                .get(row.ordinal as usize)
                .expect("economy sidecar ordinal joins generic event stream");
            assert_eq!(generic.kind, MC_CAMPAIGN_EVENT_ECONOMY);
            assert_eq!(generic.pass, row.pass);
            assert_eq!(generic.family, row.family);
            assert_eq!(generic.source_slot, row.source_slot);
            assert!((1..=16).contains(&row.code));
        }
        let mut canonical_events = Sha256::new();
        canonical_events.update(sim_core::mission_content::REMC2_SPEC_COMMIT.as_bytes());
        canonical_events.update(b"level2-campaign-events-v1");
        for (ordinal, event) in events[..event_count as usize].iter().enumerate() {
            canonical_events.update((ordinal as u32).to_le_bytes());
            canonical_events.update([event.pass, event.kind, event.family, event.flags]);
            canonical_events.update(event.source_slot.to_le_bytes());
        }
        assert_eq!(
            snapshot.dispatch_hash,
            canonical_events.finalize().as_slice()
        );
        let composite_hash = with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                unreachable!("campaign fixture opened as Level 2")
            };
            Ok(reset.campaign_state_hash())
        })
        .expect("read composite Level 2 state hash");
        assert_eq!(snapshot.state_hash, composite_hash);
        let stepped_hash = snapshot.state_hash;

        assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
        let mut reset_info = McCampaignWorldInfoV2::default();
        assert_eq!(
            mc_native_campaign_world_info(
                handle,
                &raw mut reset_info,
                struct_size::<McCampaignWorldInfoV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(reset_info.reset_state_hash, initial_info.reset_state_hash);
        let mut stale_goat_batch = McGoatBatchV2::default();
        assert_eq!(
            mc_native_campaign_goats_copy(
                handle,
                0,
                0,
                initial_goats.as_mut_ptr(),
                initial_goats.len() as u32,
                struct_size::<McGoatSnapshotV2>(),
                &raw mut stale_goat_batch,
                struct_size::<McGoatBatchV2>(),
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        let mut reset_carrier_batch = McCampaignManaBatchV2::default();
        assert_eq!(
            mc_native_campaign_mana_carriers_copy(
                handle,
                1,
                0,
                carriers.as_mut_ptr(),
                carriers.len() as u32,
                struct_size::<McManaCarrierSnapshotV2>(),
                &raw mut reset_carrier_batch,
                struct_size::<McCampaignManaBatchV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(reset_carrier_batch.total_count, 0);
        let mut stale_sphere_batch = McCampaignManaBatchV2::default();
        assert_eq!(
            mc_native_campaign_mana_spheres_copy(
                handle,
                0,
                0,
                initial_spheres.as_mut_ptr(),
                initial_spheres.len() as u32,
                struct_size::<McManaSphereSnapshotV2>(),
                &raw mut stale_sphere_batch,
                struct_size::<McCampaignManaBatchV2>(),
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        let mut stale_carrier_batch = McCampaignManaBatchV2::default();
        assert_eq!(
            mc_native_campaign_mana_carriers_copy(
                handle,
                0,
                0,
                carriers.as_mut_ptr(),
                carriers.len() as u32,
                struct_size::<McManaCarrierSnapshotV2>(),
                &raw mut stale_carrier_batch,
                struct_size::<McCampaignManaBatchV2>(),
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        input.generation = 0;
        event_count = 0;
        assert_eq!(
            mc_native_campaign_session_step(
                handle,
                &raw const input,
                &raw mut snapshot,
                struct_size::<McCampaignStepSnapshotV2>(),
                events.as_mut_ptr(),
                events.len() as u32,
                struct_size::<McCampaignEventV2>(),
                &raw mut event_count,
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        input.generation = 1;
        assert_eq!(
            mc_native_campaign_session_step(
                handle,
                &raw const input,
                &raw mut snapshot,
                struct_size::<McCampaignStepSnapshotV2>(),
                events.as_mut_ptr(),
                events.len() as u32,
                struct_size::<McCampaignEventV2>(),
                &raw mut event_count,
            ),
            MC_STATUS_OK
        );
        assert_ne!(snapshot.state_hash, 0);
        // The first step ran after installing a sphere and two carriers; the
        // post-reset generation starts from the qualified empty arena, so its
        // deterministic state hash must differ while the reset baseline above
        // remains unchanged.
        assert_ne!(snapshot.state_hash, stepped_hash);
        assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);

        let (level1_package, level1_closure) = fixture_paths();
        let level1_handle = open_paths(&level1_package, &level1_closure).expect("Level 1 open");
        let legacy_command = valid_command();
        let mut legacy_snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                level1_handle,
                &raw const legacy_command,
                &raw mut legacy_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(mc_native_session_destroy(level1_handle), MC_STATUS_OK);
    }

    #[test]
    fn level2_campaign_step_projects_family12_source_slot38() {
        let level2_package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/l2-debug3.fJ3PaM/level2");
        let package = level2_package.to_string_lossy();
        let selection = McCampaignSelectionV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCampaignSelectionV2>(),
            external_level: 2,
            internal_level: 1,
            map_type: 0,
            ..McCampaignSelectionV2::default()
        };
        let mut handle = 0;
        assert_eq!(
            mc_native_campaign_session_open(
                package.as_ptr(),
                package.len() as u32,
                std::ptr::null(),
                0,
                &raw const selection,
                &raw mut handle,
            ),
            MC_STATUS_OK
        );
        let pickup_position = with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                unreachable!("campaign fixture opened as Level 2")
            };
            let pickup = reset
                .shared_spells()
                .pickups()
                .iter()
                .find(|pickup| pickup.index == 38)
                .expect("family-12 source slot 38 is present");
            assert_eq!(pickup.action, 37);
            Ok(pickup.position)
        })
        .expect("read family-12 source position");

        let input_command = McCommandV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCommandV2>(),
            flags: 1,
            requested_spell: MC_REQUESTED_SPELL_NONE,
            ..McCommandV2::default()
        };
        let mut input = McCampaignStepInputV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCampaignStepInputV2>(),
            generation: 0,
            command: input_command,
            player_slot: 0,
            player_x: pickup_position.x,
            player_y: pickup_position.y,
            player_z: pickup_position.z,
            ..McCampaignStepInputV2::default()
        };
        let mut snapshot = McCampaignStepSnapshotV2::default();
        let mut events = [McCampaignEventV2::default(); 1024];
        let mut event_count;
        let mut acquired = None;
        // The source cadence is part of the reset-visible row. Walk the
        // minimum bounded number of turns needed to reach its phase gate
        // instead of mutating private source bytes through the C ABI.
        for _ in 0..4 {
            event_count = 0;
            assert_eq!(
                mc_native_campaign_session_step(
                    handle,
                    &raw const input,
                    &raw mut snapshot,
                    struct_size::<McCampaignStepSnapshotV2>(),
                    events.as_mut_ptr(),
                    events.len() as u32,
                    struct_size::<McCampaignEventV2>(),
                    &raw mut event_count,
                ),
                MC_STATUS_OK
            );
            acquired = events[..event_count as usize]
                .iter()
                .find(|event| {
                    event.kind == MC_CAMPAIGN_EVENT_SPELL_ACQUISITION
                        && event.family == 12
                        && event.source_slot == 38
                        && event.flags == 1
                })
                .copied()
                .or(acquired);
            if acquired.is_some() {
                break;
            }
        }
        let acquired = acquired.expect("C ABI exposes family-12 slot-38 acquisition");
        assert_eq!(acquired.pass, 0);
        assert_eq!(acquired.generation, 0);
        assert_eq!(acquired.abi_version, MC_NATIVE_ABI_VERSION);
        let acquired_snapshot = snapshot;

        assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
        input.generation = 1;
        let mut replay = None;
        for _ in 0..4 {
            event_count = 0;
            assert_eq!(
                mc_native_campaign_session_step(
                    handle,
                    &raw const input,
                    &raw mut snapshot,
                    struct_size::<McCampaignStepSnapshotV2>(),
                    events.as_mut_ptr(),
                    events.len() as u32,
                    struct_size::<McCampaignEventV2>(),
                    &raw mut event_count,
                ),
                MC_STATUS_OK
            );
            replay = events[..event_count as usize]
                .iter()
                .find(|event| {
                    event.kind == MC_CAMPAIGN_EVENT_SPELL_ACQUISITION
                        && event.family == 12
                        && event.source_slot == 38
                        && event.flags == 1
                })
                .copied()
                .or(replay);
            if replay.is_some() {
                break;
            }
        }
        let replay = replay.expect("reset replay exposes family-12 slot-38 acquisition");
        assert_eq!(replay.generation, 1);
        assert_eq!(replay.pass, acquired.pass);
        assert_eq!(replay.kind, acquired.kind);
        assert_eq!(replay.family, acquired.family);
        assert_eq!(replay.flags, acquired.flags);
        assert_eq!(replay.source_slot, acquired.source_slot);
        assert_eq!(snapshot.state_hash, acquired_snapshot.state_hash);
        assert_eq!(snapshot.terrain_hash, acquired_snapshot.terrain_hash);
        assert_eq!(snapshot.dispatch_hash, acquired_snapshot.dispatch_hash);
        assert_eq!(snapshot.event_count, acquired_snapshot.event_count);
        assert_eq!(snapshot.event_kind_mask, acquired_snapshot.event_kind_mask);
        assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn level2_campaign_step_projects_persistent_replacement_success_and_no_free_slot_failure() {
        let open_level2 = || {
            let level2_package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/l2-debug3.fJ3PaM/level2");
            let package = level2_package.to_string_lossy();
            let selection = McCampaignSelectionV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McCampaignSelectionV2>(),
                external_level: 2,
                internal_level: 1,
                map_type: 0,
                ..McCampaignSelectionV2::default()
            };
            let mut handle = 0;
            assert_eq!(
                mc_native_campaign_session_open(
                    package.as_ptr(),
                    package.len() as u32,
                    std::ptr::null(),
                    0,
                    &raw const selection,
                    &raw mut handle,
                ),
                MC_STATUS_OK
            );
            handle
        };
        let command = McCommandV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCommandV2>(),
            flags: 1,
            requested_spell: MC_REQUESTED_SPELL_NONE,
            ..McCommandV2::default()
        };

        let success_handle = open_level2();
        let (success_source_slot, success_position, success_player_slot) =
            with_campaign_session_mut(success_handle, |session| {
                let CampaignSessionKind::Level2 { reset, .. } = &mut session.kind else {
                    unreachable!("campaign fixture opened as Level 2")
                };
                let player_entity_index = reset.state.entity_index;
                let position = reset
                    .entities()
                    .get(usize::from(player_entity_index))
                    .expect("qualified Level 2 player entity")
                    .position;
                let replacement = reset
                    .allocate_level2_spell_replacement(11, player_entity_index, position)
                    .expect("qualified persistent replacement allocation");
                Ok((
                    replacement.source_entity_index,
                    replacement.position,
                    reset.runtime().local_player_slot,
                ))
            })
            .expect("install success-vector replacement");
        let success_count_before = with_campaign_session(success_handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                unreachable!("campaign fixture opened as Level 2")
            };
            Ok(reset.economy().spell_replacements().len())
        })
        .expect("read success-vector baseline");
        let mut success_input = McCampaignStepInputV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCampaignStepInputV2>(),
            generation: 0,
            command,
            player_slot: success_player_slot,
            player_x: success_position.x,
            player_y: success_position.y,
            player_z: success_position.z,
            ..McCampaignStepInputV2::default()
        };
        let mut success_snapshot = McCampaignStepSnapshotV2::default();
        let mut success_events = [McCampaignEventV2::default(); 4096];
        let mut success_acquired = false;
        for _ in 0..4 {
            let mut success_event_count = 0;
            assert_eq!(
                mc_native_campaign_session_step(
                    success_handle,
                    &raw const success_input,
                    &raw mut success_snapshot,
                    struct_size::<McCampaignStepSnapshotV2>(),
                    success_events.as_mut_ptr(),
                    success_events.len() as u32,
                    struct_size::<McCampaignEventV2>(),
                    &raw mut success_event_count,
                ),
                MC_STATUS_OK
            );
            success_acquired |=
                success_events[..success_event_count as usize]
                    .iter()
                    .any(|event| {
                        event.kind == MC_CAMPAIGN_EVENT_SPELL_ACQUISITION
                            && event.family == 11
                            && event.source_slot == success_source_slot
                            && event.flags == 1
                    });
            if success_acquired {
                break;
            }
        }
        assert!(success_acquired);
        let success_count_after = with_campaign_session(success_handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                unreachable!("campaign fixture opened as Level 2")
            };
            Ok(reset.economy().spell_replacements().len())
        })
        .expect("read success-vector result");
        assert_eq!(success_count_after, success_count_before + 1);
        assert_eq!(
            mc_native_campaign_session_destroy(success_handle),
            MC_STATUS_OK
        );

        let failure_handle = open_level2();
        let (failure_source_slot, failure_position, failure_player_slot, allocated_count) =
            with_campaign_session_mut(failure_handle, |session| {
                let CampaignSessionKind::Level2 { reset, .. } = &mut session.kind else {
                    unreachable!("campaign fixture opened as Level 2")
                };
                let player_entity_index = reset.state.entity_index;
                let position = reset
                    .entities()
                    .get(usize::from(player_entity_index))
                    .expect("qualified Level 2 player entity")
                    .position;
                let first = reset
                    .allocate_level2_spell_replacement(11, player_entity_index, position)
                    .expect("qualified failure-vector seed replacement");
                let mut allocated = 1_usize;
                while reset.economy().free_slot_count() != 0 {
                    reset
                        .allocate_level2_spell_replacement(
                            11,
                            player_entity_index,
                            Position::default(),
                        )
                        .expect("shared arena should materialize every qualified slot");
                    allocated += 1;
                }
                Ok((
                    first.source_entity_index,
                    first.position,
                    reset.runtime().local_player_slot,
                    allocated,
                ))
            })
            .expect("exhaust shared arena for failure vector");
        success_input.generation = 0;
        success_input.player_slot = failure_player_slot;
        success_input.player_x = failure_position.x;
        success_input.player_y = failure_position.y;
        success_input.player_z = failure_position.z;
        let mut failure_snapshot = McCampaignStepSnapshotV2::default();
        let mut failure_events = [McCampaignEventV2::default(); 4096];
        let mut failure_acquired = false;
        for _ in 0..4 {
            let mut failure_event_count = 0;
            assert_eq!(
                mc_native_campaign_session_step(
                    failure_handle,
                    &raw const success_input,
                    &raw mut failure_snapshot,
                    struct_size::<McCampaignStepSnapshotV2>(),
                    failure_events.as_mut_ptr(),
                    failure_events.len() as u32,
                    struct_size::<McCampaignEventV2>(),
                    &raw mut failure_event_count,
                ),
                MC_STATUS_OK
            );
            failure_acquired |=
                failure_events[..failure_event_count as usize]
                    .iter()
                    .any(|event| {
                        event.kind == MC_CAMPAIGN_EVENT_SPELL_ACQUISITION
                            && event.family == 11
                            && event.source_slot == failure_source_slot
                            && event.flags == 1
                    });
            if failure_acquired {
                break;
            }
        }
        assert!(failure_acquired);
        with_campaign_session(failure_handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                unreachable!("campaign fixture opened as Level 2")
            };
            assert_eq!(reset.economy().free_slot_count(), 0);
            assert_eq!(reset.economy().spell_replacements().len(), allocated_count);
            assert_eq!(
                reset
                    .economy()
                    .spell_replacements()
                    .iter()
                    .find(|replacement| replacement.source_entity_index == failure_source_slot)
                    .expect("failure-vector source remains live")
                    .action,
                33
            );
            Ok(())
        })
        .expect("read no-free replacement result");
        assert_eq!(
            mc_native_campaign_session_destroy(failure_handle),
            MC_STATUS_OK
        );
    }

    #[test]
    fn level2_campaign_economy_sidecar_copies_typed_payload_without_touching_legacy_rows() {
        let level2_package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/l2-debug3.fJ3PaM/level2");
        let package = level2_package.to_string_lossy();
        let selection = McCampaignSelectionV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCampaignSelectionV2>(),
            external_level: 2,
            internal_level: 1,
            map_type: 0,
            ..McCampaignSelectionV2::default()
        };
        let mut handle = 0;
        assert_eq!(
            mc_native_campaign_session_open(
                package.as_ptr(),
                package.len() as u32,
                std::ptr::null(),
                0,
                &raw const selection,
                &raw mut handle,
            ),
            MC_STATUS_OK
        );
        let (sphere_id, carrier_slot) = with_campaign_session_mut(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &mut session.kind else {
                unreachable!("campaign fixture opened as Level 2")
            };
            let sphere_id = reset
                .economy_mut()
                .spawn_mana_sphere_from_death(901, Position::default(), 0, 0x3456, 111, 222)
                .expect("test sphere allocation");
            let carrier_slot = reset
                .economy_mut()
                .spawn_mana_carrier(Position::default(), 222, sphere_id)
                .expect("test carrier allocation");
            Ok((sphere_id, carrier_slot))
        })
        .expect("install source-derived carrier target before outer turn");

        let input_command = McCommandV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCommandV2>(),
            flags: 1,
            requested_spell: MC_REQUESTED_SPELL_NONE,
            ..McCommandV2::default()
        };
        let input = McCampaignStepInputV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCampaignStepInputV2>(),
            command: input_command,
            player_x: 123,
            player_y: 456,
            player_z: 7,
            ..McCampaignStepInputV2::default()
        };
        let mut snapshot = McCampaignStepSnapshotV2::default();
        let mut generic_events = [McCampaignEventV2::default(); 1024];
        let mut generic_count = 0;
        assert_eq!(
            mc_native_campaign_session_step(
                handle,
                &raw const input,
                &raw mut snapshot,
                struct_size::<McCampaignStepSnapshotV2>(),
                generic_events.as_mut_ptr(),
                generic_events.len() as u32,
                struct_size::<McCampaignEventV2>(),
                &raw mut generic_count,
            ),
            MC_STATUS_OK
        );
        let mut sidecars = [McCampaignEconomyEventV2::default(); 1024];
        let mut sidecar_count = 0;
        assert_eq!(
            mc_native_campaign_economy_events(
                handle,
                sidecars.as_mut_ptr(),
                sidecars.len() as u32,
                struct_size::<McCampaignEconomyEventV2>(),
                &raw mut sidecar_count,
            ),
            MC_STATUS_OK
        );
        let row = sidecars[..sidecar_count as usize]
            .iter()
            .find(|row| {
                row.code == MC_CAMPAIGN_ECONOMY_MANA_SPHERE_COLLECTED && row.primary_id == sphere_id
            })
            .copied()
            .expect("source carrier collection is published through sidecar");
        assert_eq!(row.generation, snapshot.generation);
        assert_eq!(row.family, 1);
        assert_eq!(row.source_slot, carrier_slot);
        assert_eq!(row.secondary_id, u32::from(carrier_slot));
        assert_eq!(row.entity_0, 0);
        assert_eq!(row.value_0, 111);
        let generic = generic_events
            .get(row.ordinal as usize)
            .expect("sidecar ordinal is in generic event stream");
        assert_eq!(generic.kind, MC_CAMPAIGN_EVENT_ECONOMY);
        assert_eq!(generic.source_slot, row.source_slot);

        assert_eq!(
            mc_native_campaign_economy_events(
                handle,
                sidecars.as_mut_ptr(),
                sidecars.len() as u32,
                struct_size::<McCampaignEconomyEventV2>() - 1,
                &raw mut sidecar_count,
            ),
            MC_STATUS_STRUCT_TOO_SMALL
        );
        assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
        sidecar_count = 99;
        assert_eq!(
            mc_native_campaign_economy_events(
                handle,
                sidecars.as_mut_ptr(),
                sidecars.len() as u32,
                struct_size::<McCampaignEconomyEventV2>(),
                &raw mut sidecar_count,
            ),
            MC_STATUS_OK
        );
        assert_eq!(sidecar_count, 0, "reset clears the retained sidecar stream");
        assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn camera_history_preserves_raw_wrapping_yaw_until_render_mapping() {
        let mut state = FlightState {
            yaw: 2_040,
            ..FlightState::default()
        };
        let positive = CameraHistory::capture(&state, 0, 128, 32, 0);
        assert_eq!(positive.yaw, 2_072);
        assert_eq!(positive.yaw & 0x07ff, 24);

        state.yaw = 8;
        let negative = CameraHistory::capture(&state, 0, 128, -32, 0);
        assert_eq!(negative.yaw, -24);
        assert_eq!(negative.yaw & 0x07ff, 2_024);

        state.yaw = i16::MAX;
        let signed_overflow = CameraHistory::capture(&state, 0, 128, 1, 0);
        assert_eq!(signed_overflow.yaw, i16::MIN);
        assert_eq!(signed_overflow.yaw & 0x07ff, 0);
    }

    #[test]
    fn render_camera_sidecar_exposes_stateful_slope_offset_and_reset() {
        let handle = open_fixture();
        let read = || {
            let mut render = McRenderCameraStateV2::default();
            assert_eq!(
                mc_native_render_camera_state(
                    handle,
                    &raw mut render,
                    struct_size::<McRenderCameraStateV2>(),
                ),
                MC_STATUS_OK
            );
            render
        };
        let initial = read();
        let (initial_history, initial_slope) = with_session(handle, |session| {
            Ok((session.camera_history, session.camera_slope_sample))
        })
        .unwrap();
        assert_eq!(
            (
                initial.history_x,
                initial.history_y,
                initial.eye_z,
                initial.slope_target_x,
                initial.slope_target_y,
                initial.render_offset_x,
                initial.render_offset_y,
            ),
            (
                initial_history.x,
                initial_history.y,
                initial_history.z,
                initial_slope.target_x,
                initial_slope.target_y,
                initial_slope.offset_x,
                initial_slope.offset_y,
            )
        );
        assert_eq!(initial.authority, MC_AUTHORITY_PLAYER_FLIGHT_LIVE);
        assert_eq!(initial.flags, MC_RENDER_CAMERA_TERRAIN_SLOPE_XY);

        let command = McCommandV2 {
            roll: 38,
            flags: 1,
            ..valid_command()
        };
        let camera_input = valid_camera_input();
        let mut snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step_with_camera(
                handle,
                &raw const command,
                &raw const camera_input,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let advanced = read();
        let advanced_slope =
            with_session(handle, |session| Ok(session.camera_slope_sample)).unwrap();
        assert_eq!(
            (
                advanced.slope_target_x,
                advanced.slope_target_y,
                advanced.render_offset_x,
                advanced.render_offset_y,
            ),
            (
                advanced_slope.target_x,
                advanced_slope.target_y,
                advanced_slope.offset_x,
                advanced_slope.offset_y,
            )
        );

        assert_eq!(
            mc_native_session_reset(
                handle,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let reset = read();
        assert_eq!(
            (
                reset.history_x,
                reset.history_y,
                reset.eye_z,
                reset.slope_target_x,
                reset.slope_target_y,
                reset.render_offset_x,
                reset.render_offset_y,
            ),
            (
                initial.history_x,
                initial.history_y,
                initial.eye_z,
                initial.slope_target_x,
                initial.slope_target_y,
                initial.render_offset_x,
                initial.render_offset_y,
            ),
            "reset zeros the persistent accumulators before the initial draw update"
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn presentation_camera_is_the_pre_entity_update_history_record() {
        let handle = open_fixture();
        let (before_state, before_turn) =
            with_session(handle, |session| Ok((session.state, session.source_turn))).unwrap();
        let command = McCommandV2 {
            roll: 38,
            flags: 1,
            ..valid_command()
        };
        let camera_input = McCameraInputV2 {
            look_yaw: 64,
            look_pitch: (-32_i16).cast_unsigned(),
            ..valid_camera_input()
        };
        let expected =
            CameraHistory::capture(&before_state, before_turn.wrapping_add(1), 128, 64, -32);
        let mut snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step_with_camera(
                handle,
                &raw const command,
                &raw const camera_input,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let mut presentation = McPresentationStateV2::default();
        assert_eq!(
            mc_native_presentation_state(
                handle,
                &raw mut presentation,
                struct_size::<McPresentationStateV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(presentation.tick, expected.tick);
        assert_eq!(presentation.source_turn, expected.source_turn);
        assert_eq!(
            (
                presentation.camera_x,
                presentation.camera_y,
                presentation.camera_z,
                presentation.camera_yaw,
                presentation.camera_pitch,
                presentation.camera_roll,
                presentation.camera_fov,
            ),
            (
                expected.x,
                expected.y,
                expected.z,
                expected.yaw,
                expected.pitch,
                expected.roll,
                expected.fov,
            )
        );
        assert_eq!(snapshot.tick, before_state.tick + 1);
        assert_ne!(
            (snapshot.x, snapshot.y, snapshot.z, snapshot.yaw),
            (
                presentation.camera_x,
                presentation.camera_y,
                presentation.camera_z.wrapping_sub(128),
                presentation.camera_yaw,
            ),
            "movement advances after the stored camera record"
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn presentation_camera_history_tracks_consecutive_turns_and_reset() {
        let handle = open_fixture();
        let first_command = McCommandV2 {
            roll: 38,
            flags: 1,
            ..valid_command()
        };
        let first_camera = McCameraInputV2 {
            look_yaw: 64,
            look_pitch: (-32_i16).cast_unsigned(),
            ..valid_camera_input()
        };
        let mut first_snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step_with_camera(
                handle,
                &raw const first_command,
                &raw const first_camera,
                &raw mut first_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );

        let (before_second, before_second_turn) =
            with_session(handle, |session| Ok((session.state, session.source_turn))).unwrap();
        let second_command = McCommandV2 {
            roll: -19,
            flags: 1,
            ..valid_command()
        };
        let second_camera = McCameraInputV2 {
            look_yaw: 21,
            look_pitch: 9,
            ..valid_camera_input()
        };
        let expected_second = CameraHistory::capture(
            &before_second,
            before_second_turn.wrapping_add(1),
            128,
            21,
            9,
        );
        let mut second_snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step_with_camera(
                handle,
                &raw const second_command,
                &raw const second_camera,
                &raw mut second_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let mut presentation = McPresentationStateV2::default();
        assert_eq!(
            mc_native_presentation_state(
                handle,
                &raw mut presentation,
                struct_size::<McPresentationStateV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            (
                presentation.tick,
                presentation.source_turn,
                presentation.camera_x,
                presentation.camera_y,
                presentation.camera_z,
                presentation.camera_yaw,
                presentation.camera_pitch,
                presentation.camera_roll,
            ),
            (
                expected_second.tick,
                expected_second.source_turn,
                expected_second.x,
                expected_second.y,
                expected_second.z,
                expected_second.yaw,
                expected_second.pitch,
                expected_second.roll,
            )
        );
        assert_eq!(presentation.tick, first_snapshot.tick);
        assert_eq!(
            (
                presentation.camera_x,
                presentation.camera_y,
                presentation.camera_z.wrapping_sub(128),
            ),
            (first_snapshot.x, first_snapshot.y, first_snapshot.z)
        );
        assert_eq!(second_snapshot.tick, first_snapshot.tick + 1);

        let mut reset_snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_reset(
                handle,
                &raw mut reset_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_presentation_state(
                handle,
                &raw mut presentation,
                struct_size::<McPresentationStateV2>(),
            ),
            MC_STATUS_OK
        );
        let reset_history = with_session(handle, |session| Ok(session.camera_history)).unwrap();
        assert_eq!(presentation.generation, reset_snapshot.generation);
        assert_eq!(presentation.tick, reset_snapshot.tick);
        assert_eq!(
            (
                presentation.camera_x,
                presentation.camera_y,
                presentation.camera_z.wrapping_sub(128),
            ),
            (reset_snapshot.x, reset_snapshot.y, reset_snapshot.z)
        );
        assert_eq!(
            (
                presentation.source_turn,
                presentation.camera_yaw,
                presentation.camera_pitch,
                presentation.camera_roll,
                presentation.camera_fov,
            ),
            (
                reset_history.source_turn,
                reset_history.yaw,
                reset_history.pitch,
                reset_history.roll,
                reset_history.fov,
            )
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn presentation_camera_base_preserves_source_ground_clearance() {
        let handle = open_fixture();
        for turn in 0..192 {
            let command = McCommandV2 {
                roll: match turn % 4 {
                    0 => 38,
                    1 => 0,
                    2 => -38,
                    _ => 0,
                },
                flags: 1,
                ..valid_command()
            };
            let camera_input = valid_camera_input();
            let mut snapshot = McSessionSnapshotV2::default();
            assert_eq!(
                mc_native_session_step_with_camera(
                    handle,
                    &raw const command,
                    &raw const camera_input,
                    &raw mut snapshot,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
            let mut presentation = McPresentationStateV2::default();
            assert_eq!(
                mc_native_presentation_state(
                    handle,
                    &raw mut presentation,
                    struct_size::<McPresentationStateV2>(),
                ),
                MC_STATUS_OK
            );
            let base = Position {
                x: presentation.camera_x,
                y: presentation.camera_y,
                z: presentation.camera_z.wrapping_sub(128),
            };
            let (ground, clearance) = with_session(handle, |session| {
                Ok((
                    session.world.ground_height(base),
                    session.world.profile().ground_clearance,
                ))
            })
            .unwrap();
            assert!(
                i32::from(base.z) >= i32::from(ground) + i32::from(clearance),
                "turn {turn}: camera base z={} is below ground {} + clearance {}",
                base.z,
                ground,
                clearance
            );
        }
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    #[cfg(not(windows))]
    fn c_header_compiles_and_locks_the_rust_layout() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("mc-sim-header-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let source = directory.join("layout.c");
        let object = directory.join("layout.o");
        fs::write(
            &source,
            r#"
#include <stddef.h>
#include "mc_sim_level2.h"

_Static_assert(sizeof(McWorldInfoV2) == 72, "McWorldInfoV2 size");
_Static_assert(sizeof(McCommandV2) == 32, "McCommandV2 size");
_Static_assert(sizeof(McCameraInputV2) == 28, "McCameraInputV2 size");
_Static_assert(sizeof(McSessionSnapshotV2) == 120, "McSessionSnapshotV2 size");
_Static_assert(sizeof(McObjectiveSnapshotV2) == 40, "McObjectiveSnapshotV2 size");
_Static_assert(sizeof(McEntitySnapshotV2) == 52, "McEntitySnapshotV2 size");
_Static_assert(sizeof(McCampaignSelectionV2) == 32, "McCampaignSelectionV2 size");
_Static_assert(sizeof(McCampaignStepInputV2) == 80, "McCampaignStepInputV2 size");
_Static_assert(sizeof(McCampaignEventV2) == 48, "McCampaignEventV2 size");
_Static_assert(sizeof(McCampaignEconomyEventV2) == 64, "McCampaignEconomyEventV2 size");
_Static_assert(sizeof(McCampaignStepSnapshotV2) == 112, "McCampaignStepSnapshotV2 size");
_Static_assert(offsetof(McCampaignStepInputV2, command) == 16, "campaign step command offset");
_Static_assert(offsetof(McCampaignEventV2, source_slot) == 24, "campaign event source offset");
_Static_assert(offsetof(McCampaignEconomyEventV2, primary_id) == 28, "campaign economy primary offset");
_Static_assert(offsetof(McCampaignEconomyEventV2, value_0) == 40, "campaign economy value offset");
_Static_assert(offsetof(McCampaignStepSnapshotV2, dispatch_hash) == 40, "campaign step hash offset");
_Static_assert(sizeof(McCampaignWorldInfoV2) == 64, "McCampaignWorldInfoV2 size");
_Static_assert(sizeof(McCampaignPlayerSnapshotV2) == 48, "McCampaignPlayerSnapshotV2 size");
_Static_assert(sizeof(McCampaignEntitySnapshotV2) == 52, "McCampaignEntitySnapshotV2 size");
_Static_assert(sizeof(McGoatSnapshotV2) == 84, "McGoatSnapshotV2 size");
_Static_assert(sizeof(McGoatBatchV2) == 64, "McGoatBatchV2 size");
_Static_assert(sizeof(McCampaignManaBatchV2) == 64, "McCampaignManaBatchV2 size");
_Static_assert(sizeof(McLevel1MissionSnapshotV2) == 96, "McLevel1MissionSnapshotV2 size");
_Static_assert(sizeof(McLevel1PlayerLifecycleSnapshotV2) == 144, "McLevel1PlayerLifecycleSnapshotV2 size");
_Static_assert(sizeof(McLevel1HudSpellSlotV2) == 36, "McLevel1HudSpellSlotV2 size");
_Static_assert(sizeof(McLevel1HudSnapshotV2) == 128, "McLevel1HudSnapshotV2 size");
_Static_assert(sizeof(McLevel1SpellMenuSnapshotV2) == 64, "McLevel1SpellMenuSnapshotV2 size");
_Static_assert(sizeof(McLevel1MinimapMarksV2) == 168, "McLevel1MinimapMarksV2 size");
_Static_assert(sizeof(McLevel1MinimapEntityCommandV2) == 24, "McLevel1MinimapEntityCommandV2 size");
_Static_assert(sizeof(McLevel1MinimapEntitiesInfoV2) == 64, "McLevel1MinimapEntitiesInfoV2 size");
_Static_assert(sizeof(McLevel1PaletteInfoV2) == 80, "McLevel1PaletteInfoV2 size");
_Static_assert(sizeof(McLevel1ExitPresentationV2) == 64, "McLevel1ExitPresentationV2 size");
_Static_assert(sizeof(McLevel1CastleSnapshotV2) == 80, "McLevel1CastleSnapshotV2 size");
_Static_assert(sizeof(McTerrainStateV2) == 64, "McTerrainStateV2 size");
_Static_assert(sizeof(McLevel1CastleLifecycleSnapshotV2) == 80, "McLevel1CastleLifecycleSnapshotV2 size");
_Static_assert(sizeof(McLevel1HouseCompletionEffectV2) == 80, "McLevel1HouseCompletionEffectV2 size");
_Static_assert(sizeof(McManaSphereSnapshotV2) == 64, "McManaSphereSnapshotV2 size");
_Static_assert(sizeof(McManaCarrierSnapshotV2) == 80, "McManaCarrierSnapshotV2 size");
_Static_assert(sizeof(McLevel1ActorSnapshotV2) == 56, "McLevel1ActorSnapshotV2 size");
_Static_assert(sizeof(McLevel1MultipartSegmentSnapshotV2) == 64, "McLevel1MultipartSegmentSnapshotV2 size");
_Static_assert(sizeof(McFireballProjectileSnapshotV2) == 56, "McFireballProjectileSnapshotV2 size");
_Static_assert(sizeof(McFireballImpactSnapshotV2) == 52, "McFireballImpactSnapshotV2 size");
_Static_assert(sizeof(McSelectorFrameStateV2) == 64, "McSelectorFrameStateV2 size");
_Static_assert(sizeof(McSelectorFrameBatchV2) == 88, "McSelectorFrameBatchV2 size");
_Static_assert(sizeof(McSelectorDrawFeedbackV2) == 48, "McSelectorDrawFeedbackV2 size");
_Static_assert(sizeof(McGoatDamageCommandV2) == 40, "McGoatDamageCommandV2 size");
_Static_assert(sizeof(McAssetRefInfoV2) == 32, "McAssetRefInfoV2 size");
_Static_assert(sizeof(McEventQueryV2) == 40, "McEventQueryV2 size");
_Static_assert(sizeof(McEventBatchV2) == 56, "McEventBatchV2 size");
_Static_assert(sizeof(McEventV2) == 72, "McEventV2 size");
_Static_assert(sizeof(McLevel1ResidentObservationV1) == 40, "McLevel1ResidentObservationV1 size");
_Static_assert(offsetof(McLevel1ResidentObservationV1, digest) == 16, "resident observation digest offset");
_Static_assert(sizeof(McAudioCommandV2) == 88, "McAudioCommandV2 size");
_Static_assert(sizeof(McPresentationStateV2) == 64, "McPresentationStateV2 size");
_Static_assert(sizeof(McRenderCameraStateV2) == 72, "McRenderCameraStateV2 size");
_Static_assert(sizeof(McTerrainPnt5VectorV2) == 56, "McTerrainPnt5VectorV2 size");
_Static_assert(sizeof(McTerrainGridVertexV2) == 64, "McTerrainGridVertexV2 size");
_Static_assert(sizeof(McTerrainFrameQueryV2) == 40, "McTerrainFrameQueryV2 size");
_Static_assert(sizeof(McTerrainFrameInfoV2) == 96, "McTerrainFrameInfoV2 size");
_Static_assert(sizeof(McPresentationFrameQueryV2) == 64, "McPresentationFrameQueryV2 size");
_Static_assert(sizeof(McLevel1PresentationInputV2) == 740, "McLevel1PresentationInputV2 size");
_Static_assert(sizeof(McPresentationFrameInfoV2) == 128, "McPresentationFrameInfoV2 size");
_Static_assert(offsetof(McWorldInfoV2, reset_state_hash) == 40, "world hash offset");
_Static_assert(offsetof(McCommandV2, requested_spell) == 14, "spell offset");
_Static_assert(offsetof(McCameraInputV2, look_yaw) == 8, "camera yaw offset");
_Static_assert(offsetof(McCameraInputV2, reserved) == 12, "camera reserved offset");
_Static_assert(offsetof(McSessionSnapshotV2, entity_index) == 48, "entity offset");
_Static_assert(offsetof(McSessionSnapshotV2, objective_textbox_ticks) == 77, "objective ticks offset");
_Static_assert(offsetof(McSessionSnapshotV2, objectives_authority) == 79, "objective authority offset");
_Static_assert(offsetof(McSessionSnapshotV2, left_spell_holder) == 82, "holder offset");
_Static_assert(offsetof(McSessionSnapshotV2, reserved) == 104, "snapshot reserved offset");
_Static_assert(offsetof(McObjectiveSnapshotV2, stage_selectors) == 13, "objective stages offset");
_Static_assert(offsetof(McObjectiveSnapshotV2, reserved) == 24, "objective reserved offset");
_Static_assert(offsetof(McEntitySnapshotV2, reserved) == 36, "entity reserved offset");
_Static_assert(offsetof(McCampaignSelectionV2, reserved) == 16, "campaign selection reserved offset");
_Static_assert(offsetof(McCampaignWorldInfoV2, reset_state_hash) == 40, "campaign world hash offset");
_Static_assert(offsetof(McCampaignPlayerSnapshotV2, turn) == 16, "campaign player turn offset");
_Static_assert(offsetof(McCampaignPlayerSnapshotV2, reserved) == 32, "campaign player reserved offset");
_Static_assert(offsetof(McCampaignEntitySnapshotV2, reserved) == 36, "campaign entity reserved offset");
_Static_assert(offsetof(McGoatSnapshotV2, life) == 48, "Goat life offset");
_Static_assert(offsetof(McGoatSnapshotV2, reserved) == 68, "Goat reserved offset");
_Static_assert(offsetof(McGoatBatchV2, reserved) == 48, "Goat batch reserved offset");
_Static_assert(offsetof(McLevel1MissionSnapshotV2, player_life) == 40, "Level 1 mission life offset");
_Static_assert(offsetof(McLevel1PlayerLifecycleSnapshotV2, player_action) == 41, "player lifecycle action offset");
_Static_assert(offsetof(McLevel1PlayerLifecycleSnapshotV2, action_timer) == 56, "player lifecycle timer offset");
_Static_assert(offsetof(McLevel1PlayerLifecycleSnapshotV2, player_control_flags) == 72, "player lifecycle flags offset");
_Static_assert(offsetof(McLevel1PlayerLifecycleSnapshotV2, source_pool_rebuild_count) == 108, "player lifecycle rebuild offset");
_Static_assert(offsetof(McLevel1PlayerLifecycleSnapshotV2, marker_dispatch_phase) == 122, "player lifecycle marker dispatch offset");
_Static_assert(offsetof(McLevel1PlayerLifecycleSnapshotV2, source_turn_counter) == 128, "player lifecycle source turn offset");
_Static_assert(offsetof(McLevel1PlayerLifecycleSnapshotV2, effective_pitch) == 134, "player lifecycle effective pitch offset");
_Static_assert(offsetof(McLevel1PlayerLifecycleSnapshotV2, marker_constructor_phase) == 136, "player lifecycle marker constructor offset");
_Static_assert(offsetof(McLevel1MissionSnapshotV2, actor_count) == 64, "Level 1 mission actor count offset");
_Static_assert(offsetof(McLevel1MissionSnapshotV2, exit_sequence_phase) == 76, "Level 1 mission exit phase offset");
_Static_assert(offsetof(McLevel1MissionSnapshotV2, exit_sequence_timer) == 80, "Level 1 mission exit timer offset");
_Static_assert(offsetof(McLevel1MissionSnapshotV2, possession_experience) == 84, "Level 1 possession experience offset");
_Static_assert(offsetof(McLevel1MissionSnapshotV2, last_possessed_building_entity) == 88, "Level 1 last possessed building offset");
_Static_assert(offsetof(McLevel1MissionSnapshotV2, possessed_building_count) == 92, "Level 1 possessed building count offset");
_Static_assert(offsetof(McLevel1MissionSnapshotV2, speed_trail_count) == 95, "Level 1 speed trail count offset");
_Static_assert(offsetof(McLevel1HudSpellSlotV2, max_mana) == 12, "Level 1 HUD spell max mana offset");
_Static_assert(offsetof(McLevel1HudSnapshotV2, global_mana_denominator) == 32, "Level 1 HUD denominator offset");
_Static_assert(offsetof(McLevel1HudSnapshotV2, left) == 48, "Level 1 HUD left offset");
_Static_assert(offsetof(McLevel1HudSnapshotV2, right) == 84, "Level 1 HUD right offset");
_Static_assert(offsetof(McLevel1HudSnapshotV2, selected_holder_entity_index) == 120, "Level 1 HUD selected holder offset");
_Static_assert(offsetof(McLevel1SpellMenuSnapshotV2, family_enabled) == 32, "Level 1 spell menu families offset");
_Static_assert(offsetof(McLevel1SpellMenuSnapshotV2, holder_entity_index) == 48, "Level 1 spell menu holders offset");
_Static_assert(offsetof(McLevel1MinimapMarksV2, mark_x) == 28, "Level 1 minimap mark X offset");
_Static_assert(offsetof(McLevel1MinimapMarksV2, arrow_xy) == 124, "Level 1 minimap arrow offset");
_Static_assert(offsetof(McLevel1MinimapMarksV2, reserved) == 152, "Level 1 minimap reserved offset");
_Static_assert(offsetof(McLevel1MinimapEntityCommandV2, reserved) == 16, "Level 1 minimap entity command reserved offset");
_Static_assert(offsetof(McLevel1MinimapEntitiesInfoV2, command_hash) == 40, "Level 1 minimap entities hash offset");
_Static_assert(offsetof(McLevel1MinimapEntitiesInfoV2, reserved) == 48, "Level 1 minimap entities reserved offset");
_Static_assert(offsetof(McLevel1PaletteInfoV2, rgb8_sha256) == 32, "Level 1 palette hash offset");
_Static_assert(offsetof(McLevel1PaletteInfoV2, reserved) == 64, "Level 1 palette reserved offset");
_Static_assert(offsetof(McLevel1ExitPresentationV2, player_action) == 25, "Level 1 exit action offset");
_Static_assert(offsetof(McLevel1ExitPresentationV2, camera_fov) == 36, "Level 1 exit FOV offset");
_Static_assert(offsetof(McLevel1ExitPresentationV2, reserved) == 48, "Level 1 exit reserved offset");
_Static_assert(offsetof(McLevel1CastleSnapshotV2, life) == 48, "Level 1 castle life offset");
_Static_assert(offsetof(McLevel1CastleSnapshotV2, reserved) == 68, "Level 1 castle reserved offset");
_Static_assert(offsetof(McTerrainStateV2, mutation_sequence) == 24, "terrain state mutation offset");
_Static_assert(offsetof(McTerrainStateV2, width) == 40, "terrain state dimensions offset");
_Static_assert(offsetof(McTerrainStateV2, reserved) == 48, "terrain state reserved offset");
_Static_assert(offsetof(McLevel1CastleLifecycleSnapshotV2, root_life) == 44, "castle lifecycle life offset");
_Static_assert(offsetof(McLevel1CastleLifecycleSnapshotV2, construction_timer) == 64, "castle lifecycle timer offset");
_Static_assert(offsetof(McLevel1CastleLifecycleSnapshotV2, terrain_mutation_sequence) == 72, "castle lifecycle terrain sequence offset");
_Static_assert(offsetof(McLevel1HouseCompletionEffectV2, parent_word_2e) == 44, "house completion parent state offset");
_Static_assert(offsetof(McLevel1HouseCompletionEffectV2, max_life) == 64, "house completion life offset");
_Static_assert(offsetof(McLevel1HouseCompletionEffectV2, effect_count) == 76, "house completion count offset");
_Static_assert(offsetof(McManaSphereSnapshotV2, mana) == 44, "mana sphere mana offset");
_Static_assert(offsetof(McManaSphereSnapshotV2, reserved) == 52, "mana sphere reserved offset");
_Static_assert(offsetof(McManaCarrierSnapshotV2, carried_mana) == 60, "mana carrier payload offset");
_Static_assert(offsetof(McManaCarrierSnapshotV2, reserved) == 64, "mana carrier reserved offset");
_Static_assert(offsetof(McLevel1ActorSnapshotV2, max_life) == 28, "Level 1 actor life offset");
_Static_assert(offsetof(McLevel1ActorSnapshotV2, base_tmap) == 40, "Level 1 actor base TMAP offset");
_Static_assert(offsetof(McLevel1ActorSnapshotV2, yaw) == 44, "Level 1 actor yaw offset");
_Static_assert(offsetof(McLevel1ActorSnapshotV2, state_value) == 48, "Level 1 actor state value offset");
_Static_assert(offsetof(McLevel1ActorSnapshotV2, reserved) == 52, "Level 1 actor reserved offset");
_Static_assert(offsetof(McLevel1MultipartSegmentSnapshotV2, x) == 26, "multipart segment x offset");
_Static_assert(offsetof(McLevel1MultipartSegmentSnapshotV2, life) == 40, "multipart segment life offset");
_Static_assert(offsetof(McLevel1MultipartSegmentSnapshotV2, base_tmap) == 50, "multipart segment TMAP offset");
_Static_assert(offsetof(McLevel1MultipartSegmentSnapshotV2, reserved) == 60, "multipart segment reserved offset");
_Static_assert(offsetof(McFireballProjectileSnapshotV2, payload) == 36, "Fireball projectile payload offset");
_Static_assert(offsetof(McFireballProjectileSnapshotV2, logical_height) == 40, "Fireball projectile height offset");
_Static_assert(offsetof(McFireballProjectileSnapshotV2, reserved) == 48, "Fireball projectile reserved offset");
_Static_assert(offsetof(McFireballImpactSnapshotV2, payload) == 32, "Fireball impact payload offset");
_Static_assert(offsetof(McFireballImpactSnapshotV2, base_tmap) == 36, "Fireball impact base TMAP offset");
_Static_assert(offsetof(McFireballImpactSnapshotV2, reserved) == 44, "Fireball impact reserved offset");
_Static_assert(offsetof(McSelectorFrameStateV2, pixel_hash) == 40, "selector frame pixel hash offset");
_Static_assert(offsetof(McSelectorFrameStateV2, reserved) == 48, "selector frame reserved offset");
_Static_assert(offsetof(McSelectorFrameBatchV2, start_ordinal) == 56, "selector batch start offset");
_Static_assert(offsetof(McSelectorFrameBatchV2, reserved) == 72, "selector batch reserved offset");
_Static_assert(offsetof(McSelectorDrawFeedbackV2, reserved) == 32, "selector feedback reserved offset");
_Static_assert(offsetof(McGoatDamageCommandV2, reserved) == 24, "Goat damage reserved offset");
_Static_assert(offsetof(McEventV2, state_hash) == 24, "event hash offset");
_Static_assert(offsetof(McEventV2, reserved) == 56, "event reserved offset");
_Static_assert(offsetof(McAudioCommandV2, playback_rate_hz) == 48, "audio rate offset");
_Static_assert(offsetof(McAudioCommandV2, sound_id) == 52, "audio sound offset");
_Static_assert(offsetof(McAudioCommandV2, volume_q15) == 60, "audio volume offset");
_Static_assert(offsetof(McAudioCommandV2, play_type) == 68, "audio type offset");
_Static_assert(offsetof(McAudioCommandV2, reserved) == 72, "audio reserved offset");
_Static_assert(offsetof(McPresentationStateV2, source_turn) == 24, "presentation turn offset");
_Static_assert(offsetof(McPresentationStateV2, camera_x) == 28, "presentation camera offset");
_Static_assert(offsetof(McPresentationStateV2, camera_authority) == 42, "presentation authority offset");
_Static_assert(offsetof(McPresentationStateV2, reserved) == 48, "presentation reserved offset");
_Static_assert(offsetof(McRenderCameraStateV2, history_x) == 28, "render camera history offset");
_Static_assert(offsetof(McRenderCameraStateV2, slope_target_x) == 42, "render camera slope target offset");
_Static_assert(offsetof(McRenderCameraStateV2, render_offset_x) == 46, "render camera offset");
_Static_assert(offsetof(McRenderCameraStateV2, reserved) == 56, "render camera reserved offset");
_Static_assert(offsetof(McTerrainPnt5VectorV2, pnt5) == 36, "terrain pnt5 offset");
_Static_assert(offsetof(McTerrainGridVertexV2, view_x) == 16, "terrain grid view offset");
_Static_assert(offsetof(McTerrainGridVertexV2, feature_flags) == 44, "terrain grid flags offset");
_Static_assert(offsetof(McTerrainGridVertexV2, reserved) == 48, "terrain grid reserved offset");
_Static_assert(offsetof(McTerrainFrameQueryV2, reserved) == 24, "terrain frame query reserved offset");
_Static_assert(offsetof(McTerrainFrameInfoV2, width) == 24, "terrain frame width offset");
_Static_assert(offsetof(McTerrainFrameInfoV2, index_bytes) == 40, "terrain frame bytes offset");
_Static_assert(offsetof(McTerrainFrameInfoV2, index_hash) == 64, "terrain frame hash offset");
_Static_assert(offsetof(McTerrainFrameInfoV2, reserved) == 88, "terrain frame reserved offset");
_Static_assert(offsetof(McPresentationFrameQueryV2, configured_blur_mode) == 40, "presentation frame blur offset");
_Static_assert(offsetof(McLevel1PresentationInputV2, mouse_x) == 38, "presentation input mouse offset");
_Static_assert(offsetof(McLevel1PresentationInputV2, debug_clock_bucket) == 704, "presentation input debug clock offset");
_Static_assert(offsetof(McPresentationFrameInfoV2, immutable_input_hash) == 32, "presentation frame input hash offset");
_Static_assert(offsetof(McPresentationFrameInfoV2, index_hash) == 64, "presentation frame index hash offset");
_Static_assert(offsetof(McPresentationFrameInfoV2, reserved) == 104, "presentation frame reserved offset");
"#,
        )
        .unwrap();
        let include = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("include");
        let output = std::process::Command::new("cc")
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-I"])
            .arg(include)
            .arg("-c")
            .arg(&source)
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap();
        let _ = fs::remove_dir_all(&directory);
        assert!(
            output.status.success(),
            "C header layout probe failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn terrain_pnt5_qualification_vectors_cross_the_typed_abi_exactly() {
        assert_eq!(
            mc_native_terrain_pnt5_vector_count(),
            TERRAIN_PNT5_QUALIFICATION_VECTOR_COUNT
        );
        let inputs = [
            (0, 4, 4, 0, 8, 128, 0),
            (0, 12, 4, 40, 8, 128, 0),
            (1, 0, 0, 40, 8, 128, 0),
            (1, 1, 1, 40, 8, 128, 14_745_600),
            (1, 2, 3, 40, 8, 128, 14_745_601),
            (1, 5, 7, 40, 8, 128, 19_202_048),
            (1, 255, 255, 40, 8, 128, 23_658_495),
            (1, 255, 255, 40, 8, 128, 23_658_496),
            (1, 255, 255, 40, 8, 128, 26_214_399),
            (1, 255, 255, 40, 8, 128, 26_214_400),
            (1, 42, 99, 56, 8, 128, 0),
            (65_535, 256, -1, 55, 8, 128, 14_757_945),
            (1, 0, 0, 40, 8, -256, 0),
        ];
        for (index, (turn, tile_x, tile_y, shade, angle, view_y, distance_squared)) in
            inputs.into_iter().enumerate()
        {
            let expected =
                outdoor_terrain_pnt5(turn, tile_x, tile_y, shade, angle, view_y, distance_squared);
            let mut vector = McTerrainPnt5VectorV2::default();
            assert_eq!(
                mc_native_terrain_pnt5_vector(
                    u32::try_from(index).unwrap(),
                    &raw mut vector,
                    struct_size::<McTerrainPnt5VectorV2>(),
                ),
                MC_STATUS_OK
            );
            assert_eq!(vector.abi_version, MC_NATIVE_ABI_VERSION);
            assert_eq!(
                (
                    vector.struct_size,
                    vector.turn,
                    vector.tile_x,
                    vector.tile_y,
                    vector.shade,
                    vector.angle,
                    vector.view_y,
                    vector.distance_squared,
                ),
                (
                    56,
                    turn,
                    tile_x,
                    tile_y,
                    shade,
                    angle,
                    view_y,
                    distance_squared,
                )
            );
            assert_eq!(vector.visible, u8::from(expected.visible));
            assert_eq!(vector.tablesn_row, expected.tablesn_row);
            assert_eq!(vector.wave, expected.wave);
            assert_eq!(vector.land_displacement, expected.land_displacement);
            assert_eq!(vector.pnt5, expected.pnt5);
            assert_eq!(vector.lighting_wave, expected.lighting_wave);
            assert_eq!(vector.reserved, [0; 2]);
        }
        let mut out_of_bounds = McTerrainPnt5VectorV2::default();
        assert_eq!(
            mc_native_terrain_pnt5_vector(
                TERRAIN_PNT5_QUALIFICATION_VECTOR_COUNT,
                &raw mut out_of_bounds,
                struct_size::<McTerrainPnt5VectorV2>(),
            ),
            MC_STATUS_BOUNDS
        );
    }

    #[test]
    fn owner_level_one_terrain_grid_crosses_the_bulk_abi_exactly() {
        let handle = open_fixture();
        assert_eq!(mc_native_terrain_grid_vertex_count(), 840);

        let mut too_small = vec![McTerrainGridVertexV2::default(); 839];
        let mut written = 99;
        assert_eq!(
            mc_native_session_terrain_grid_copy(
                handle,
                too_small.as_mut_ptr(),
                839,
                struct_size::<McTerrainGridVertexV2>(),
                &raw mut written,
            ),
            MC_STATUS_BUFFER_TOO_SMALL
        );
        assert_eq!(written, 0);

        let mut vertices = vec![McTerrainGridVertexV2::default(); 840];
        assert_eq!(
            mc_native_session_terrain_grid_copy(
                handle,
                vertices.as_mut_ptr(),
                840,
                struct_size::<McTerrainGridVertexV2>(),
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 840);
        let direct = with_session(handle, |session| Ok(session.terrain_grid())).unwrap();
        assert_eq!(vertices.as_slice(), direct.as_slice());
        for (index, vertex) in vertices.iter().enumerate() {
            assert_eq!(vertex.abi_version, MC_NATIVE_ABI_VERSION);
            assert_eq!(vertex.struct_size, 64);
            assert_eq!(usize::from(vertex.index), index);
            assert_eq!(usize::from(vertex.row), index / 40);
            assert_eq!(usize::from(vertex.column), index % 40);
            assert_eq!(vertex.pnt5_written, u8::from(vertex.feature_flags & 2 == 0));
            if vertex.pnt5_written != 0 {
                assert_eq!(vertex.tablesn_row, ((vertex.pnt5 >> 16) & 0xff) as u8);
            }
            assert_eq!(vertex.reserved, [0; 4]);
        }
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn isolated_terrain_frame_crosses_generation_tick_checked_bulk_abi() {
        let handle = open_fixture_with_terrain_frame_assets();
        let mut camera = McRenderCameraStateV2::default();
        assert_eq!(
            mc_native_render_camera_state(
                handle,
                &raw mut camera,
                struct_size::<McRenderCameraStateV2>(),
            ),
            MC_STATUS_OK
        );
        let query = McTerrainFrameQueryV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McTerrainFrameQueryV2>(),
            generation: camera.generation,
            tick: camera.tick,
            reserved: [0; 4],
        };
        let mut indices = vec![0_u8; LEVEL1_VIEW_PIXELS];
        let mut coverage = vec![0_u8; LEVEL1_VIEW_PIXELS];
        let mut info = McTerrainFrameInfoV2::default();
        assert_eq!(
            mc_native_session_terrain_frame_copy(
                handle,
                &raw const query,
                indices.as_mut_ptr(),
                indices.len() as u32,
                coverage.as_mut_ptr(),
                coverage.len() as u32,
                &raw mut info,
                struct_size::<McTerrainFrameInfoV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            (info.width, info.height, info.stride),
            (
                LEVEL1_VIEW_WIDTH as u16,
                LEVEL1_VIEW_HEIGHT as u16,
                LEVEL1_VIEW_WIDTH as u16,
            )
        );
        assert_eq!(
            (info.index_bytes, info.coverage_bytes),
            (LEVEL1_VIEW_PIXELS as u32, LEVEL1_VIEW_PIXELS as u32)
        );
        assert_eq!(
            info.flags,
            MC_TERRAIN_FRAME_ISOLATED_SKY_TERRAIN
                | MC_TERRAIN_FRAME_SKY_SEEDED
                | MC_TERRAIN_FRAME_HAS_COVERAGE
                | MC_TERRAIN_FRAME_NON_TERRAIN_OMITTED
        );
        assert_eq!(
            sim_core::terrain_frame::byte_hash(&indices),
            info.index_hash
        );
        assert_eq!(
            sim_core::terrain_frame::byte_hash(&coverage),
            info.coverage_hash
        );
        assert!(coverage.contains(&255));

        let first_indices = indices.clone();
        let first_coverage = coverage.clone();
        let mut repeated = McTerrainFrameInfoV2::default();
        assert_eq!(
            mc_native_session_terrain_frame_copy(
                handle,
                &raw const query,
                indices.as_mut_ptr(),
                indices.len() as u32,
                coverage.as_mut_ptr(),
                coverage.len() as u32,
                &raw mut repeated,
                struct_size::<McTerrainFrameInfoV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(info, repeated);
        assert_eq!(indices, first_indices);
        assert_eq!(coverage, first_coverage);

        assert_eq!(
            mc_native_session_terrain_frame_copy(
                handle,
                &raw const query,
                indices.as_mut_ptr(),
                (LEVEL1_VIEW_PIXELS - 1) as u32,
                coverage.as_mut_ptr(),
                coverage.len() as u32,
                &raw mut repeated,
                struct_size::<McTerrainFrameInfoV2>(),
            ),
            MC_STATUS_BUFFER_TOO_SMALL
        );
        let stale_tick = McTerrainFrameQueryV2 {
            tick: query.tick.wrapping_add(1),
            ..query
        };
        assert_eq!(
            mc_native_session_terrain_frame_copy(
                handle,
                &raw const stale_tick,
                indices.as_mut_ptr(),
                indices.len() as u32,
                coverage.as_mut_ptr(),
                coverage.len() as u32,
                &raw mut repeated,
                struct_size::<McTerrainFrameInfoV2>(),
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn reset_world_chain_contains_all_frozen_actors_in_source_relative_order() {
        let handle = open_fixture_with_world_frame_assets();
        with_session(handle, |session| {
            let frozen = LEVEL1_FROZEN_ACTOR_ENTITY_INDICES
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(session.frozen_actors.len(), frozen.len());
            assert!(session.frozen_actors.iter().all(|actor| {
                frozen.contains(&actor.source_entity_index)
                    && session
                        .renderable_map_chains
                        .tracks(actor.source_entity_index)
            }));

            let supported = session
                .world
                .reset_live_world()
                .goats()
                .iter()
                .map(|goat| goat.entity_index)
                .chain(frozen.iter().copied())
                .collect::<std::collections::BTreeSet<_>>();
            let planes = session.world.terrain_planes();
            let mut seen_frozen = std::collections::BTreeSet::new();
            for (tile, raw_head) in planes.entity_indices.iter().copied().enumerate() {
                if raw_head <= 0 {
                    continue;
                }
                let mut expected = Vec::new();
                let mut source_entity = raw_head.cast_unsigned();
                while source_entity != 0 {
                    if supported.contains(&source_entity) {
                        expected.push(source_entity);
                        if frozen.contains(&source_entity) {
                            seen_frozen.insert(source_entity);
                        }
                    }
                    source_entity =
                        session.world.entities()[usize::from(source_entity)].old_map_entity();
                }
                if expected.is_empty() {
                    continue;
                }
                let tile_x = u8::try_from(tile & 0xff).expect("tile X");
                let tile_y = u8::try_from(tile >> 8).expect("tile Y");
                assert_eq!(
                    session.renderable_map_chains.entity_chain(tile_x, tile_y),
                    expected,
                    "filtered source-relative order changed at tile {tile:#06x}"
                );
            }
            assert_eq!(seen_frozen, frozen);
            Ok(())
        })
        .expect("qualified reset renderable chain");
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn reset_restores_live_villager_draw_records_before_rebuilding_map_chains() {
        let handle = open_fixture_with_world_frame_assets();
        with_session_mut(handle, |session| {
            let reset_actors = level1_frozen_actor_renders(session.world.as_ref())?;
            for _ in 0..16 {
                session.step(PlayerCommand::default());
            }
            assert!(
                session
                    .frozen_actors
                    .iter()
                    .zip(reset_actors.iter())
                    .any(|(live, reset)| {
                        live.source_entity_index < 130 && live.position != reset.position
                    }),
                "at least one live Villager moved before reset"
            );

            session.reset();

            assert_eq!(session.frozen_actors, reset_actors);
            let expected =
                reset_renderable_map_chains(session.world.as_ref(), reset_actors.as_ref())?;
            assert_eq!(session.renderable_map_chains, expected);
            Ok(())
        })
        .expect("stepped live Villager reset");
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn source_ordered_world_frame_exposes_supported_families_without_changing_terrain_contract() {
        let handle = open_fixture_with_world_frame_assets();
        let native_frame = with_session_mut(handle, |session| {
            assert_eq!(session.frozen_actors.len(), 31);
            assert!(session.frozen_actors.iter().all(|actor| {
                session
                    .renderable_map_chains
                    .tracks(actor.source_entity_index)
            }));
            assert_eq!(session.mission.ambient_entities().count(), 40);
            let reset_ambient = session
                .mission
                .ambient_entities()
                .copied()
                .collect::<Vec<_>>();
            let render_ambient_ablation =
                |ambient_entities: &[Level1AmbientEntity]| -> AbiResult<Level1TerrainFrame> {
                    let mut map_chains = session.renderable_map_chains.clone();
                    for ambient in &reset_ambient {
                        if ambient_entities.iter().any(|selected| {
                            selected.source_entity_index == ambient.source_entity_index
                        }) {
                            continue;
                        }
                        map_chains
                            .sync_entity(RenderableMapSeed {
                                entity_index: ambient.source_entity_index,
                                active: false,
                                x: ambient.position.x,
                                y: ambient.position.y,
                            })
                            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
                    }
                    session.render_world_frame(ambient_entities, &map_chains)
                };
            let reset_full = render_ambient_ablation(&reset_ambient)?;
            let reset_without_ambient = render_ambient_ablation(&[])?;
            assert_eq!(reset_full.index_hash, 0xaca8_5f81_ab89_d6ef);
            assert_eq!(
                reset_full.terrain_coverage,
                reset_without_ambient.terrain_coverage
            );
            assert_eq!(
                reset_full.coverage_hash,
                reset_without_ambient.coverage_hash
            );
            assert_eq!(reset_full.stats, reset_without_ambient.stats);

            let changed_summary = |frame: &Level1TerrainFrame,
                                   baseline: &Level1TerrainFrame|
             -> (usize, Option<(usize, usize, usize, usize)>) {
                assert_eq!(frame.terrain_coverage, baseline.terrain_coverage);
                assert_eq!(frame.coverage_hash, baseline.coverage_hash);
                assert_eq!(frame.stats, baseline.stats);
                assert_eq!(frame.indices.len(), baseline.indices.len());
                let mut count = 0;
                let mut unchanged_count = 0;
                let mut bounds: Option<(usize, usize, usize, usize)> = None;
                for (index, (&actual, &without_ambient)) in frame
                    .indices
                    .iter()
                    .zip(baseline.indices.iter())
                    .enumerate()
                {
                    if actual == without_ambient {
                        unchanged_count += 1;
                        continue;
                    }
                    let x = index % LEVEL1_VIEW_WIDTH;
                    let y = index / LEVEL1_VIEW_WIDTH;
                    count += 1;
                    bounds = Some(bounds.map_or((x, y, x, y), |(min_x, min_y, max_x, max_y)| {
                        (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y))
                    }));
                }
                assert_eq!(count + unchanged_count, frame.indices.len());
                (count, bounds)
            };

            assert_eq!(reset_without_ambient.index_hash, 0x2f38_beef_8e4a_62fb);
            assert_eq!(
                reset_without_ambient
                    .ambient_raster_stats
                    .framebuffer_writes,
                0
            );
            assert_eq!(reset_full.ambient_raster_stats.framebuffer_writes, 16_000);
            assert_eq!(
                changed_summary(&reset_full, &reset_without_ambient),
                (10_989, Some((0, 114, 252, 199)))
            );

            for (model, row_count, index_hash, framebuffer_writes, delta) in [
                (
                    1,
                    8,
                    0xfba7_2c67_ec5b_9bac,
                    10_627,
                    (8_894, Some((0, 128, 242, 199))),
                ),
                (
                    14,
                    9,
                    0x734e_f184_f4c0_cdaa,
                    259,
                    (256, Some((66, 114, 211, 168))),
                ),
                (
                    0,
                    14,
                    0x4831_3b7f_97f3_30ed,
                    5_114,
                    (4_539, Some((25, 134, 252, 199))),
                ),
            ] {
                let family = reset_ambient
                    .iter()
                    .copied()
                    .filter(|ambient| ambient.model == model)
                    .collect::<Vec<_>>();
                let frame = render_ambient_ablation(&family)?;
                assert_eq!(
                    (
                        family.len(),
                        frame.index_hash,
                        frame.ambient_raster_stats.framebuffer_writes,
                        changed_summary(&frame, &reset_without_ambient),
                    ),
                    (row_count, index_hash, framebuffer_writes, delta),
                    "reset ambient family model {model}"
                );
            }
            let emitter_113 = reset_ambient
                .iter()
                .copied()
                .filter(|ambient| ambient.source_entity_index == 113)
                .collect::<Vec<_>>();
            assert_eq!(emitter_113.len(), 1);
            assert!(!session.renderable_map_chains.tracks(113));
            let emitter_frame = render_ambient_ablation(&emitter_113)?;
            assert_eq!(
                (
                    emitter_frame.index_hash,
                    emitter_frame.ambient_raster_stats.framebuffer_writes,
                    changed_summary(&emitter_frame, &reset_without_ambient),
                ),
                (0x2f38_beef_8e4a_62fb, 0, (0, None))
            );
            let fire = session
                .mission
                .ambient_entities()
                .find(|ambient| ambient.source_entity_index == 122)
                .copied()
                .expect("frame-zero village fire");
            let child_before = session
                .mission
                .ambient_entities()
                .find(|ambient| ambient.source_entity_index == 164)
                .copied()
                .expect("frame-zero village fire child");
            session.camera_slope_sample = TerrainSlopeCameraSample::default();
            let mut ambient_before = None;
            let mut ambient_camera = None;
            for yaw in (0_i16..2_048_i16).step_by(256) {
                let camera = CameraHistory {
                    tick: session.state.tick,
                    source_turn: session.source_turn,
                    x: fire.position.x.wrapping_sub(768),
                    y: fire.position.y,
                    z: fire.position.z.wrapping_add(128),
                    yaw,
                    pitch: 0,
                    roll: 0,
                    fov: 128,
                };
                session.camera_history = camera;
                let frame = session.world_frame()?;
                if frame.ambient_raster_stats.framebuffer_writes > 0 {
                    ambient_camera = Some(camera);
                    ambient_before = Some(frame);
                    break;
                }
            }
            let ambient_before = ambient_before.ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "no cardinal test view rasterized village fire entity 122",
                )
            })?;
            assert!(ambient_before.ambient_entities_composited);
            assert!(ambient_before.ambient_raster_stats.framebuffer_writes > 0);
            session.step(PlayerCommand::default());
            session.camera_history = ambient_camera.expect("visible ambient camera");
            let ambient_after = session.world_frame()?;
            assert_ne!(
                ambient_before.index_hash, ambient_after.index_hash,
                "ambient action/reclamation must change production render pixels"
            );
            assert!(
                session
                    .mission
                    .ambient_entities()
                    .all(|entity| entity.model != 14)
            );
            assert!(
                session
                    .mission
                    .ambient_entities()
                    .find(|entity| entity.source_entity_index == 164)
                    .is_some_and(|entity| {
                        entity.life == 6
                            && entity.animation_frame == 2
                            && entity.position != child_before.position
                    })
            );
            session.reset();
            assert_eq!(session.mission.ambient_entities().count(), 40);
            let actor = session.frozen_actors[0];
            session.camera_slope_sample = TerrainSlopeCameraSample::default();
            let mut visible = None;
            for yaw in (0_i16..2_048_i16).step_by(256) {
                session.camera_history = CameraHistory {
                    tick: session.state.tick,
                    source_turn: session.source_turn,
                    x: actor.position.x.wrapping_sub(1_024),
                    y: actor.position.y,
                    z: actor.position.z.wrapping_add(256),
                    yaw,
                    pitch: 0,
                    roll: 0,
                    fov: 128,
                };
                let frame = session.world_frame()?;
                if frame.frozen_actor_raster_stats.framebuffer_writes > 0
                    && frame.frozen_actor_selector_mask != 0
                {
                    visible = Some(frame);
                    break;
                }
            }
            visible.ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "no cardinal test view rasterized frozen villager entity 51",
                )
            })
        })
        .expect("native frame");
        assert!(native_frame.frozen_actors_composited);
        assert!(
            native_frame.frozen_actor_raster_stats.framebuffer_writes > 0,
            "the reset view must raster at least one of the 31 frozen actors"
        );
        assert_ne!(
            native_frame.frozen_actor_selector_mask, 0,
            "the reset view must latch at least one villager facing"
        );
        assert_eq!(native_frame.index_hash, 0x3a86_3b45_c134_1d6c);
        assert_eq!(
            native_frame.frozen_actor_raster_stats,
            sim_core::goat_sprite_raster::Mode20SpriteRasterStats {
                candidate_pixels: 6_439,
                transparent_pixels: 1_277,
                clipped_pixels: 3_575,
                framebuffer_writes: 960,
            }
        );
        assert_eq!(
            native_frame.reflected_sprite_raster_stats,
            sim_core::goat_sprite_raster::Mode20SpriteRasterStats {
                candidate_pixels: 6_439,
                transparent_pixels: 1_172,
                clipped_pixels: 3_750,
                framebuffer_writes: 890,
            }
        );
        assert_eq!(
            native_frame.shadow_sprite_raster_stats,
            sim_core::goat_sprite_raster::Mode20SpriteRasterStats {
                candidate_pixels: 1_519,
                transparent_pixels: 287,
                clipped_pixels: 767,
                framebuffer_writes: 186,
            }
        );
        let camera =
            with_session(handle, |session| Ok(session.render_camera_state())).expect("camera");
        let query = McTerrainFrameQueryV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McTerrainFrameQueryV2>(),
            generation: camera.generation,
            tick: camera.tick,
            reserved: [0; 4],
        };
        let mut terrain_indices = vec![0_u8; LEVEL1_VIEW_PIXELS];
        let mut terrain_coverage = vec![0_u8; LEVEL1_VIEW_PIXELS];
        let mut terrain_info = McTerrainFrameInfoV2::default();
        assert_eq!(
            mc_native_session_terrain_frame_copy(
                handle,
                &raw const query,
                terrain_indices.as_mut_ptr(),
                terrain_indices.len() as u32,
                terrain_coverage.as_mut_ptr(),
                terrain_coverage.len() as u32,
                &raw mut terrain_info,
                struct_size::<McTerrainFrameInfoV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(terrain_info.flags & MC_TERRAIN_FRAME_GOATS_COMPOSITED, 0);
        assert_eq!(terrain_info.flags & MC_TERRAIN_FRAME_ARCHERS_COMPOSITED, 0);
        assert_eq!(
            terrain_info.flags & MC_TERRAIN_FRAME_FIREFLIES_COMPOSITED,
            0
        );
        assert_eq!(terrain_info.flags & MC_TERRAIN_FRAME_COMBAT_COMPOSITED, 0);
        assert_eq!(
            terrain_info.flags & MC_TERRAIN_FRAME_OBJECTIVE_PROPS_COMPOSITED,
            0
        );
        assert_eq!(terrain_info.flags & MC_TERRAIN_FRAME_ECONOMY_COMPOSITED, 0);
        assert_eq!(
            terrain_info.flags & MC_TERRAIN_FRAME_REFLECTED_SPRITES_COMPOSITED,
            0
        );
        assert_eq!(
            terrain_info.flags & MC_TERRAIN_FRAME_SPRITE_SHADOWS_COMPOSITED,
            0
        );
        assert_eq!(
            terrain_info.flags & MC_TERRAIN_FRAME_FROZEN_ACTORS_COMPOSITED,
            0
        );

        let mut world_indices = vec![0_u8; LEVEL1_VIEW_PIXELS];
        let mut world_coverage = vec![0_u8; LEVEL1_VIEW_PIXELS];
        let mut world_info = McTerrainFrameInfoV2::default();
        let world_status = mc_native_session_world_frame_copy(
            handle,
            &raw const query,
            world_indices.as_mut_ptr(),
            world_indices.len() as u32,
            world_coverage.as_mut_ptr(),
            world_coverage.len() as u32,
            &raw mut world_info,
            struct_size::<McTerrainFrameInfoV2>(),
        );
        assert_eq!(
            world_status,
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|error| error.borrow().clone())
        );
        assert_ne!(world_info.flags & MC_TERRAIN_FRAME_GOATS_COMPOSITED, 0);
        assert_ne!(world_info.flags & MC_TERRAIN_FRAME_ARCHERS_COMPOSITED, 0);
        assert_ne!(world_info.flags & MC_TERRAIN_FRAME_FIREFLIES_COMPOSITED, 0);
        assert_ne!(world_info.flags & MC_TERRAIN_FRAME_COMBAT_COMPOSITED, 0);
        assert_ne!(
            world_info.flags & MC_TERRAIN_FRAME_OBJECTIVE_PROPS_COMPOSITED,
            0
        );
        assert_ne!(world_info.flags & MC_TERRAIN_FRAME_ECONOMY_COMPOSITED, 0);
        assert_eq!(
            world_info.flags & MC_TERRAIN_FRAME_REFLECTED_SPRITES_COMPOSITED != 0,
            camera.eye_z < sim_core::terrain_reflection::REFLECTION_EYE_Z_LIMIT as i16
        );
        assert_ne!(
            world_info.flags & MC_TERRAIN_FRAME_SPRITE_SHADOWS_COMPOSITED,
            0
        );
        assert_ne!(
            world_info.flags & MC_TERRAIN_FRAME_FROZEN_ACTORS_COMPOSITED,
            0
        );
        assert_eq!(world_coverage, terrain_coverage);
        assert_eq!(
            world_info.index_hash,
            sim_core::terrain_frame::byte_hash(&world_indices)
        );
        assert_eq!(
            world_info.coverage_hash,
            sim_core::terrain_frame::byte_hash(&world_coverage)
        );
        let feedback = with_session(handle, |session| Ok(session.selector_draw_feedback))
            .expect("draw feedback");
        assert!(feedback.is_some());

        let first_world_indices = world_indices.clone();
        let first_world_info = world_info;
        assert_eq!(
            mc_native_session_world_frame_copy(
                handle,
                &raw const query,
                world_indices.as_mut_ptr(),
                world_indices.len() as u32,
                world_coverage.as_mut_ptr(),
                world_coverage.len() as u32,
                &raw mut world_info,
                struct_size::<McTerrainFrameInfoV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(world_info, first_world_info);
        assert_eq!(world_indices, first_world_indices);
        assert_eq!(
            with_session(handle, |session| Ok(session.selector_draw_feedback))
                .expect("repeated draw feedback"),
            feedback
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn positional_audio_qualification_vectors_cross_the_typed_sidecar_exactly() {
        assert_eq!(
            mc_native_audio_qualification_vector_count(),
            POSITIONAL_QUALIFICATION_VECTOR_COUNT
        );
        let expected_octants = [
            (43, 1_024, 30_036, 32_767, 0, 22_050),
            (37, 724, 30_560, 49_151, 0, 22_050),
            (39, 1_024, 29_126, 65_535, 0, 22_050),
            (37, 724, 29_678, 49_151, 0, 22_050),
            (37, 1_024, 27_305, 32_767, 0, 22_050),
            (37, 724, 29_678, 16_383, 0, 22_050),
            (37, 1_024, 29_126, 0, 0, 22_050),
            (37, 724, 30_560, 16_383, 0, 22_050),
        ];
        for (index, (sound, distance, volume, pan, rate_delta, rate_hz)) in
            expected_octants.into_iter().enumerate()
        {
            let mut command = McAudioCommandV2::default();
            assert_eq!(
                mc_native_audio_qualification_vector(
                    u32::try_from(index).unwrap(),
                    &raw mut command,
                    struct_size::<McAudioCommandV2>(),
                ),
                MC_STATUS_OK
            );
            assert_eq!(command.abi_version, MC_NATIVE_ABI_VERSION);
            assert_eq!(command.struct_size, 88);
            assert_eq!(command.sound_id, sound);
            assert_eq!(
                sim_core::positional_sound_command(
                    Position {
                        x: 32_768,
                        y: 32_768,
                        z: 1_000,
                    },
                    0,
                    Position {
                        x: u16::try_from(32_768_i32 + command.relative_x).unwrap(),
                        y: u16::try_from(32_768_i32 + command.relative_y).unwrap(),
                        z: i16::try_from(1_000_i32 + command.relative_z).unwrap(),
                    },
                    command.sound_id,
                    if index == 0 { 28 } else { 0 },
                    321,
                )
                .unwrap()
                .distance_xy,
                distance
            );
            assert_eq!(command.volume_q15, volume);
            assert_eq!(command.pan_q16, pan);
            assert_eq!(command.rate_delta_percent, rate_delta);
            assert_eq!(command.playback_rate_hz, rate_hz);
            assert_eq!(command.play_type, 3);
            assert_eq!(command.loop_count, 0);
            assert_eq!(command.action, MC_AUDIO_ACTION_START);
            assert_eq!(command.voice_index, 0);
            assert_eq!(command.reserved, [0; 4]);
        }

        for (index, expected_volume, expected_action) in [
            (10, 514, MC_AUDIO_ACTION_START),
            (11, 511, MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME),
            (12, 515, MC_AUDIO_ACTION_START),
            (13, 511, MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME),
            (14, 517, MC_AUDIO_ACTION_START),
            (15, 511, MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME),
            (16, 0, MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME),
            (17, 0, MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME),
        ] {
            let mut command = McAudioCommandV2::default();
            assert_eq!(
                mc_native_audio_qualification_vector(
                    index,
                    &raw mut command,
                    struct_size::<McAudioCommandV2>(),
                ),
                MC_STATUS_OK
            );
            assert_eq!(command.volume_q15, expected_volume);
            assert_eq!(command.action, expected_action);
        }

        for (index, delta, hz) in [(18, -15, 18_742), (19, 0, 22_050), (20, 14, 25_137)] {
            let mut command = McAudioCommandV2::default();
            assert_eq!(
                mc_native_audio_qualification_vector(
                    index,
                    &raw mut command,
                    struct_size::<McAudioCommandV2>(),
                ),
                MC_STATUS_OK
            );
            assert_eq!(command.sound_flags, 0);
            assert_eq!(command.rate_delta_percent, delta);
            assert_eq!(command.playback_rate_hz, hz);
        }

        let mut duplicate = McAudioCommandV2::default();
        assert_eq!(
            mc_native_audio_qualification_vector(
                22,
                &raw mut duplicate,
                struct_size::<McAudioCommandV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(duplicate.sound_id, 43);
        assert_eq!(duplicate.action, MC_AUDIO_ACTION_SUPPRESS_EXISTING);
        assert_eq!(duplicate.voice_index, 0);

        let mut command = McAudioCommandV2::default();
        assert_eq!(
            mc_native_audio_qualification_vector(
                POSITIONAL_QUALIFICATION_VECTOR_COUNT,
                &raw mut command,
                struct_size::<McAudioCommandV2>(),
            ),
            MC_STATUS_BOUNDS
        );
    }

    #[test]
    fn audio_sidecars_encode_original_loop_count_by_sound_class() {
        let handle = open_fixture();
        with_session_mut(handle, |session| {
            let entity_index = session.state.entity_index;
            let hash = state_hash(&session.state);
            for sound_id in [1, 2, 5, 31] {
                session.audio_commands.clear();
                session.audio_voices.fill(None);

                let start_event = session.encode_flight_event(
                    FlightEvent::PrepareSound {
                        entity_index,
                        player_index: -1,
                        sound_id,
                    },
                    hash,
                );
                session.drain_pending_audio_commands();
                let start = session.audio_commands[&start_event.sequence];
                assert_eq!(start.loop_count, -1);
                assert_eq!(start.action, MC_AUDIO_ACTION_START);

                let update_event = session.encode_flight_event(
                    FlightEvent::PrepareSound {
                        entity_index,
                        player_index: -1,
                        sound_id,
                    },
                    hash,
                );
                session.drain_pending_audio_commands();
                let update = session.audio_commands[&update_event.sequence];
                assert_eq!(update.loop_count, -1);
                assert_eq!(update.action, MC_AUDIO_ACTION_UPDATE_EXISTING);
                assert_eq!(update.voice_index, start.voice_index);

                let end_event = session.encode_flight_event(
                    FlightEvent::EndLoop {
                        entity_index,
                        player_index: -1,
                        sound_id,
                    },
                    hash,
                );
                let end = session.audio_commands[&end_event.sequence];
                assert_eq!(end.loop_count, -1);
                assert_eq!(end.action, MC_AUDIO_ACTION_END_LOOP);
                assert_eq!(end.voice_index, start.voice_index);
            }

            let synthetic_event = McEventV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McEventV2>(),
                ..McEventV2::default()
            };
            assert_eq!(
                local_audio_command_record(&synthetic_event, 29, 0, 1,).loop_count,
                0
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn audio_command_discard_reclaims_resolved_sidecar_and_logical_voice() {
        let handle = open_fixture();
        let generation = with_session(handle, |session| Ok(session.generation)).unwrap();
        for sequence in 100..112 {
            with_session_mut(handle, |session| {
                let mut command = McAudioCommandV2 {
                    generation,
                    event_sequence: sequence,
                    sound_id: 37,
                    sound_flags: 0,
                    play_type: 3,
                    ..McAudioCommandV2::default()
                };
                let (action, voice_index) = session.allocate_audio_voice(command);
                command.action = action;
                command.voice_index = voice_index;
                assert_eq!(command.action, MC_AUDIO_ACTION_START);
                assert_eq!(command.voice_index, 0);
                session.audio_commands.insert(sequence, command);
                Ok(())
            })
            .unwrap();
            assert_eq!(
                mc_native_audio_command_discard(handle, generation, sequence),
                MC_STATUS_OK
            );
            with_session(handle, |session| {
                assert!(!session.audio_commands.contains_key(&sequence));
                assert!(session.audio_voices.iter().all(Option::is_none));
                Ok(())
            })
            .unwrap();
        }

        with_session_mut(handle, |session| {
            session.audio_voices[0] = Some(AudioVoice {
                sound_id: 2,
                sound_flags: 0,
                event_sequence: 200,
            });
            for (sequence, action) in [
                (201, MC_AUDIO_ACTION_UPDATE_EXISTING),
                (202, MC_AUDIO_ACTION_END_LOOP),
            ] {
                session.audio_commands.insert(
                    sequence,
                    McAudioCommandV2 {
                        generation,
                        event_sequence: sequence,
                        sound_id: 2,
                        sound_flags: 0,
                        action,
                        voice_index: 0,
                        ..McAudioCommandV2::default()
                    },
                );
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(
            mc_native_audio_command_discard(handle, generation, 201),
            MC_STATUS_OK
        );
        with_session_mut(handle, |session| {
            assert!(session.audio_voices[0].is_none());
            session.audio_voices[0] = Some(AudioVoice {
                sound_id: 2,
                sound_flags: 0,
                event_sequence: 200,
            });
            Ok(())
        })
        .unwrap();
        assert_eq!(
            mc_native_audio_command_discard(handle, generation, 202),
            MC_STATUS_OK
        );
        with_session(handle, |session| {
            assert!(session.audio_voices[0].is_none());
            Ok(())
        })
        .unwrap();

        for (sequence, action) in [
            (300, MC_AUDIO_ACTION_START),
            (301, MC_AUDIO_ACTION_UPDATE_EXISTING),
            (302, MC_AUDIO_ACTION_END_LOOP),
        ] {
            with_session_mut(handle, |session| {
                session.audio_voices[0] = Some(AudioVoice {
                    sound_id: 2,
                    sound_flags: 0,
                    event_sequence: 400,
                });
                session.audio_commands.insert(
                    sequence,
                    McAudioCommandV2 {
                        generation,
                        event_sequence: sequence,
                        sound_id: 2,
                        sound_flags: 0,
                        action,
                        voice_index: 0,
                        ..McAudioCommandV2::default()
                    },
                );
                Ok(())
            })
            .unwrap();
            assert_eq!(
                mc_native_audio_command_discard(handle, generation, sequence),
                MC_STATUS_OK
            );
            with_session(handle, |session| {
                assert_eq!(
                    session.audio_voices[0].map(|voice| voice.event_sequence),
                    Some(400),
                    "discarding stale start/update/end preserves a newer voice owner"
                );
                Ok(())
            })
            .unwrap();
        }
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn session_audio_sidecar_is_keyed_to_event_generation_and_cleared_on_reset() {
        let handle = open_fixture();
        let snapshot = with_session_mut(handle, |session| {
            session.state.entity_phase = 64;
            session
                .context
                .effects
                .as_mut()
                .expect("session has effects")
                .periodic_spell_effect = Some(0x13);
            session.step(PlayerCommand::default());
            Ok(session.snapshot())
        })
        .unwrap();
        let sequence = with_session(handle, |session| {
            session
                .events
                .iter()
                .find(|event| event.kind == MC_EVENT_PREPARE_SOUND && event.code == 43)
                .map(|event| event.sequence)
                .ok_or_else(|| AbiError::new(MC_STATUS_BOUNDS, "missing positional event"))
        })
        .unwrap();
        let mut command = McAudioCommandV2::default();
        assert_eq!(
            mc_native_audio_command_for_event(
                handle,
                snapshot.generation,
                sequence,
                &raw mut command,
                struct_size::<McAudioCommandV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(command.event_sequence, sequence);
        assert_eq!(command.state_hash, snapshot.state_hash);
        assert_eq!(command.sound_id, 43);
        assert_eq!(command.entity_index, 152);
        assert_eq!(command.sound_flags, 0);
        assert_eq!(
            (command.relative_x, command.relative_y, command.relative_z),
            (0, 0, 0)
        );
        assert_eq!(command.volume_q15, 0x7fff);
        assert_eq!(command.pan_q16, 0x7fff);
        assert_eq!(command.loop_count, 0);
        assert_eq!(command.action, MC_AUDIO_ACTION_START);
        assert_eq!(command.voice_index, 2);
        with_session(handle, |session| {
            let event_for = |kind, code| {
                session
                    .events
                    .iter()
                    .find(|event| event.kind == kind && event.code == code)
                    .copied()
                    .expect("source-ledger event is present")
            };
            let periodic_43 = event_for(MC_EVENT_PREPARE_SOUND, 43);
            let land_2 = event_for(MC_EVENT_PREPARE_SOUND, 2);
            let land_end_1 = event_for(MC_EVENT_END_LOOP, 1);
            let ambient_fire_3 = session
                .audio_commands
                .iter()
                .find_map(|(sequence, audio)| {
                    (audio.sound_id == 3
                        && audio.voice_index == 1
                        && audio.action == MC_AUDIO_ACTION_START)
                        .then(|| {
                            session
                                .events
                                .iter()
                                .find(|event| event.sequence == *sequence)
                                .copied()
                                .expect("winning sound-3 command retains its source event")
                        })
                })
                .expect("one ambient-fire source occurrence wins pooled sound 3");
            assert!(
                periodic_43.sequence < land_2.sequence
                    && land_2.sequence < land_end_1.sequence
                    && land_end_1.sequence < ambient_fire_3.sequence,
                "flight emits synthetic periodic 43 then pinned SurfaceLandLoop 2/1 before the pinned ambient-fire mission occurrence"
            );
            // Pinned source occurrence identities:
            // sound 2 SurfaceLandLoop = a8fdf305...3f1e8;
            // sound 3 AmbientFireAction01FirstEligibleSound = ac35f9a0...a9a9.
            let allocated = [land_2.sequence, ambient_fire_3.sequence, periodic_43.sequence]
                .map(|sequence| session.audio_commands[&sequence]);
            assert_eq!(
                allocated.map(|audio| {
                    (
                        audio.sound_id,
                        audio.voice_index,
                        audio.pooled_flush_rank,
                        audio.action,
                    )
                }),
                [
                    (2, 0, 0, MC_AUDIO_ACTION_START),
                    (3, 1, 1, MC_AUDIO_ACTION_START),
                    (43, 2, 2, MC_AUDIO_ACTION_START),
                ],
                "direct sound 2 claims voice 0 before sound-id-ordered pooled 3 and synthetic/excluded 43 claim voices 1 and 2"
            );
            assert_eq!(
                session.audio_voices[..3]
                    .iter()
                    .map(|voice| voice.map(|voice| voice.sound_id))
                    .collect::<Vec<_>>(),
                [Some(2), Some(3), Some(43)],
                "source-ordered land ambience and ambient-fire sound claim voices zero and one before pooled sound 43"
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(
            mc_native_audio_voice_complete(
                handle,
                snapshot.generation.wrapping_add(1),
                command.voice_index,
                command.event_sequence,
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(
            mc_native_audio_voice_complete(handle, snapshot.generation, 0, command.event_sequence,),
            MC_STATUS_BOUNDS
        );
        assert_eq!(
            mc_native_audio_voice_complete(
                handle,
                snapshot.generation,
                command.voice_index,
                command.event_sequence.wrapping_add(1),
            ),
            MC_STATUS_BOUNDS
        );
        assert_eq!(
            mc_native_audio_voice_complete(
                handle,
                snapshot.generation,
                command.voice_index,
                command.event_sequence,
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_audio_voice_complete(
                handle,
                snapshot.generation,
                command.voice_index,
                command.event_sequence,
            ),
            MC_STATUS_BOUNDS
        );

        let old_generation = snapshot.generation;
        let mut reset = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_reset(handle, &raw mut reset, struct_size::<McSessionSnapshotV2>(),),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_audio_command_for_event(
                handle,
                old_generation,
                sequence,
                &raw mut command,
                struct_size::<McAudioCommandV2>(),
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(
            mc_native_audio_command_for_event(
                handle,
                reset.generation,
                sequence,
                &raw mut command,
                struct_size::<McAudioCommandV2>(),
            ),
            MC_STATUS_BOUNDS
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);

        let stored_rng = 0x9909_f858_u32;
        let nonstored_rng = stored_rng.wrapping_mul(9_377).wrapping_add(9_439);
        assert_eq!(nonstored_rng, 0xa631_b437);
        for (source_action, expected_delta) in [(0, 5), (14, 25)] {
            let prepared = prepare_event_sound_command(
                Position::default(),
                0,
                Position::default(),
                46,
                stored_rng,
                102,
                source_action,
            )
            .expect("sound46 has a source PrepareEventSound route");
            assert_eq!(
                prepared.command.rate_delta_percent, expected_delta,
                "sound46 derives its pitch from the nonstored next LCG value"
            );
            assert_eq!(
                stored_rng, 0x9909_f858,
                "pitch derivation never stores the derived LCG value"
            );
        }

        let snapshot_handle = open_fixture();
        let (snapshot_audio, listener, sound_position, mutated_position, captured_rng) =
            with_session_mut(snapshot_handle, |session| {
                const BLEATING_GOAT: u16 = 102;
                let listener = session.state.position;
                let mut entities = session.world.entities().to_vec();
                let goat = &mut entities[usize::from(BLEATING_GOAT)];
                goat.action = 14;
                goat.position = listener;
                session.live_world = LiveWorldState::from_entities(
                    &entities,
                    session.world.goat_stage_point(),
                    session.world.goat_config(),
                )
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
                session
                    .live_world
                    .inject_damage(
                        BLEATING_GOAT,
                        1,
                        i16::try_from(session.state.entity_index)
                            .expect("qualified player index fits signed source relation"),
                    )
                    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
                session.mission.begin_source_turn_at(listener);
                let player = [GoatPlayerTarget {
                    entity_index: session.state.entity_index,
                    position: listener,
                    control_flags: 0,
                }];
                let events =
                    session
                        .live_world
                        .step_source_slot(BLEATING_GOAT, &session.terrain, &player);
                let (bleat, sound_position, sound_action) = events
                    .into_iter()
                    .find_map(|event| match event {
                        event @ GoatStepEvent::Bleat {
                            entity_index: BLEATING_GOAT,
                            sound_id: 46,
                            position,
                            action,
                        } => Some((event, position, action)),
                        _ => None,
                    })
                    .ok_or_else(|| {
                        AbiError::new(
                            MC_STATUS_VALIDATION_FAILED,
                            "source-derived Goat slot 102 did not emit sound46",
                        )
                    })?;
                assert_eq!(sound_action, 14);
                let captured_rng = session.mission.global_rng();
                assert_eq!(captured_rng, 0x9909_f858);

                let mutated_position = Position {
                    x: sound_position.x.wrapping_add(4_096),
                    y: sound_position.y.wrapping_add(2_048),
                    z: sound_position.z.wrapping_add(512),
                };
                let goat = &mut entities[usize::from(BLEATING_GOAT)];
                goat.action = 0;
                goat.position = mutated_position;
                session.live_world = LiveWorldState::from_entities(
                    &entities,
                    session.world.goat_stage_point(),
                    session.world.goat_config(),
                )
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;

                let event =
                    session.encode_goat_event(bleat, session.live_world_hash_v2(), captured_rng);
                let audio = session
                    .audio_commands
                    .get(&event.sequence)
                    .copied()
                    .ok_or_else(|| {
                        AbiError::new(
                            MC_STATUS_VALIDATION_FAILED,
                            "snapshotted sound46 event has no audio sidecar",
                        )
                    })?;
                assert_eq!(session.mission.global_rng(), captured_rng);
                Ok((
                    audio,
                    listener,
                    sound_position,
                    mutated_position,
                    captured_rng,
                ))
            })
            .expect("production Goat sound snapshot survives later live-row mutation");
        assert_eq!(captured_rng, 0x9909_f858);
        assert_eq!(snapshot_audio.rate_delta_percent, 25);
        assert_eq!(
            (
                snapshot_audio.relative_x,
                snapshot_audio.relative_y,
                snapshot_audio.relative_z,
            ),
            (
                i32::from(sound_position.x) - i32::from(listener.x),
                i32::from(sound_position.y) - i32::from(listener.y),
                i32::from(sound_position.z) - i32::from(listener.z),
            )
        );
        assert_ne!(sound_position, mutated_position);
        assert_eq!(mc_native_session_destroy(snapshot_handle), MC_STATUS_OK);

        let other_action = production_goat_pitch_case(false);
        let action_14 = production_goat_pitch_case(true);
        assert_eq!(
            (
                other_action.sound_id,
                other_action.rate_delta_percent,
                other_action.playback_rate_hz,
            ),
            (46, 5, 23_152),
            "stored 0x9909f858 derives nonstored 0xa631b437 and the other-action +5 family"
        );
        assert_eq!(
            (
                action_14.sound_id,
                action_14.rate_delta_percent,
                action_14.playback_rate_hz,
            ),
            (46, 25, 27_562),
            "stored 0x9909f858 derives nonstored 0xa631b437 and action-14 +25"
        );
    }

    #[test]
    fn pending_slot_and_ten_voice_lifecycle_match_original_arbitration() {
        fn command(
            sequence: u64,
            sound_id: u16,
            sound_flags: i16,
            volume_q15: u16,
        ) -> McAudioCommandV2 {
            McAudioCommandV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: 88,
                generation: 1,
                event_sequence: sequence,
                sound_id,
                sound_flags,
                volume_q15,
                play_type: 3,
                voice_index: MC_AUDIO_VOICE_NONE,
                ..McAudioCommandV2::default()
            }
        }

        let handle = open_fixture();
        with_session_mut(handle, |session| {
            session.audio_commands.insert(1, command(1, 37, 20, 1_000));
            session.audio_commands.insert(2, command(2, 37, 21, 992));
            session.drain_pending_audio_commands();
            assert_eq!(
                session.audio_commands[&1].action,
                MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED
            );
            assert_eq!(session.audio_commands[&2].action, MC_AUDIO_ACTION_START);
            assert_eq!(session.audio_commands[&2].voice_index, 0);
            assert_eq!(session.audio_commands[&1].pooled_flush_rank, 0);
            assert_eq!(session.audio_commands[&2].pooled_flush_rank, 1);
            assert_eq!(session.audio_commands[&2].reserved[..2], [1, 0]);

            session.audio_commands.insert(3, command(3, 39, 30, 1_000));
            session.audio_commands.insert(4, command(4, 39, 31, 991));
            session.drain_pending_audio_commands();
            assert_eq!(session.audio_commands[&3].action, MC_AUDIO_ACTION_START);
            assert_eq!(
                session.audio_commands[&4].action,
                MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED
            );
            assert_eq!(session.audio_commands[&3].pooled_flush_rank, 1);
            assert_eq!(session.audio_commands[&4].pooled_flush_rank, 0);
            assert_eq!(session.audio_commands[&3].reserved[..2], [3, 0]);

            for index in 2..AUDIO_VOICE_COUNT {
                let sequence = u64::try_from(index + 10).unwrap();
                let flags = i16::try_from(index + 100).unwrap();
                session
                    .audio_commands
                    .insert(sequence, command(sequence, 37, flags, 2_000));
                session.drain_pending_audio_commands();
                assert_eq!(
                    session.audio_commands[&sequence].voice_index,
                    u16::try_from(index).unwrap()
                );
            }
            assert!(session.audio_voices.iter().all(Option::is_some));

            let full_sequence = 100;
            session
                .audio_commands
                .insert(full_sequence, command(full_sequence, 37, 999, 2_000));
            session.drain_pending_audio_commands();
            assert_eq!(
                session.audio_commands[&full_sequence].action,
                MC_AUDIO_ACTION_SUPPRESS_NO_VOICE
            );
            assert_eq!(
                session.audio_commands[&full_sequence].voice_index,
                MC_AUDIO_VOICE_NONE
            );

            let duplicate_sequence = 101;
            session.audio_commands.insert(
                duplicate_sequence,
                command(duplicate_sequence, 37, 21, 2_000),
            );
            session.drain_pending_audio_commands();
            assert_eq!(
                session.audio_commands[&duplicate_sequence].action,
                MC_AUDIO_ACTION_SUPPRESS_EXISTING
            );
            assert_eq!(session.audio_commands[&duplicate_sequence].voice_index, 0);

            session.audio_voices[0] = None;
            let reused_sequence = 102;
            session
                .audio_commands
                .insert(reused_sequence, command(reused_sequence, 37, 999, 2_000));
            session.drain_pending_audio_commands();
            assert_eq!(
                session.audio_commands[&reused_sequence].action,
                MC_AUDIO_ACTION_START
            );
            assert_eq!(session.audio_commands[&reused_sequence].voice_index, 0);

            let restart_sequence = 103;
            let mut restart = command(restart_sequence, 37, 999, 2_000);
            restart.play_type = 1;
            session.audio_commands.insert(restart_sequence, restart);
            session.drain_pending_audio_commands();
            assert_eq!(
                session.audio_commands[&restart_sequence].action,
                MC_AUDIO_ACTION_RESTART
            );
            assert_eq!(session.audio_commands[&restart_sequence].voice_index, 0);

            session.audio_commands.clear();
            session.audio_voices.fill(None);
            session
                .audio_commands
                .insert(200, command(200, 63, 63, 0x7fff));
            session
                .audio_commands
                .insert(201, command(201, 61, 61, 0x7fff));
            session.drain_pending_audio_commands();
            assert_eq!(session.audio_commands[&201].pooled_flush_rank, 1);
            assert_eq!(session.audio_commands[&200].pooled_flush_rank, 2);
            assert_eq!(session.audio_commands[&201].reserved[..2], [200, 0]);
            assert_eq!(session.audio_commands[&200].reserved[..2], [200, 0]);

            session.audio_commands.clear();
            session.audio_voices.fill(None);
            session.audio_commands.insert(210, command(210, 1, 0, 100));
            session.audio_commands.insert(211, command(211, 1, 0, 200));
            session.drain_pending_audio_commands();
            assert_eq!(session.audio_commands[&210].pooled_flush_rank, 0);
            assert_eq!(session.audio_commands[&211].pooled_flush_rank, 0);
            assert_ne!(
                session.audio_commands[&210].action,
                MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED
            );
            assert_ne!(
                session.audio_commands[&211].action,
                MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED
            );

            session.reset();
            assert!(session.audio_voices.iter().all(Option::is_none));
            assert!(session.audio_commands.is_empty());
            Ok(())
        })
        .unwrap();
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn paused_pending_audio_survives_ring_and_step_backlog_until_explicit_release() {
        let handle = open_fixture();
        let (generation, sequence) = with_session_mut(handle, |session| {
            session.game_paused = true;
            let mut event = session.next_event_base(MC_EVENT_PREPARE_SOUND, 1);
            event.code = 61;
            let command = local_audio_command_record(&event, 61, 0, 1);
            let sequence = event.sequence;
            session.insert_prepared_audio_command(sequence, command);
            session.append_event(event);
            for _ in 0..EVENT_CAPACITY {
                let event = session.next_event_base(MC_EVENT_MOVEMENT_RESOLUTION, 1);
                session.append_event(event);
            }
            assert!(
                !session
                    .events
                    .iter()
                    .any(|event| event.sequence == sequence)
            );
            assert_eq!(session.audio_commands[&sequence].action, 0);
            assert!(session.audio_commands.len() <= EVENT_CAPACITY + 1);
            Ok((session.generation, sequence))
        })
        .unwrap();
        assert_eq!(
            mc_native_audio_command_release(handle, generation, sequence),
            MC_STATUS_INVALID_ARGUMENT,
            "clients cannot release an unresolved source-pool row"
        );

        with_session_mut(handle, |session| {
            session.game_paused = false;
            session.drain_pending_audio_commands();
            assert_ne!(session.audio_commands[&sequence].action, 0);
            session.step(PlayerCommand::default());
            session.step(PlayerCommand::default());
            assert!(session.audio_commands.contains_key(&sequence));
            Ok(())
        })
        .unwrap();
        assert_eq!(
            mc_native_audio_command_release(handle, generation, sequence),
            MC_STATUS_OK
        );
        let mut released = McAudioCommandV2::default();
        assert_eq!(
            mc_native_audio_command_for_event(
                handle,
                generation,
                sequence,
                &raw mut released,
                struct_size::<McAudioCommandV2>(),
            ),
            MC_STATUS_BOUNDS
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn versioned_struct_boundaries_reject_short_buffers_and_bad_strides() {
        let handle = open_fixture();
        let mut snapshot = McSessionSnapshotV2 {
            abi_version: 77,
            ..McSessionSnapshotV2::default()
        };
        let unchanged = snapshot;
        assert_eq!(
            mc_native_session_snapshot(
                handle,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>() - 1,
            ),
            MC_STATUS_STRUCT_TOO_SMALL
        );
        assert_eq!(snapshot, unchanged);

        let mut terrain_state = McTerrainStateV2 {
            abi_version: 77,
            ..McTerrainStateV2::default()
        };
        let unchanged_terrain_state = terrain_state;
        assert_eq!(
            mc_native_session_terrain_state(
                handle,
                &raw mut terrain_state,
                struct_size::<McTerrainStateV2>() - 1,
            ),
            MC_STATUS_STRUCT_TOO_SMALL
        );
        assert_eq!(terrain_state, unchanged_terrain_state);

        let mut lifecycle = McLevel1CastleLifecycleSnapshotV2 {
            abi_version: 77,
            ..McLevel1CastleLifecycleSnapshotV2::default()
        };
        let unchanged_lifecycle = lifecycle;
        assert_eq!(
            mc_native_level1_castle_lifecycle_snapshot(
                handle,
                &raw mut lifecycle,
                struct_size::<McLevel1CastleLifecycleSnapshotV2>() - 1,
            ),
            MC_STATUS_STRUCT_TOO_SMALL
        );
        assert_eq!(lifecycle, unchanged_lifecycle);

        let short_command_header = [MC_NATIVE_ABI_VERSION, 8];
        assert_eq!(
            mc_native_session_step(
                handle,
                short_command_header.as_ptr().cast::<McCommandV2>(),
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_STRUCT_TOO_SMALL
        );

        let mut written = 99;
        assert_eq!(
            mc_native_entities_copy(
                handle,
                0,
                std::ptr::null_mut(),
                0,
                struct_size::<McEntitySnapshotV2>() - 1,
                &raw mut written,
            ),
            MC_STATUS_STRUCT_TOO_SMALL
        );
        assert_eq!(
            mc_native_entities_copy(
                handle,
                0,
                std::ptr::null_mut(),
                0,
                struct_size::<McEntitySnapshotV2>() + 1,
                &raw mut written,
            ),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            mc_native_session_terrain_grid_copy(
                handle,
                std::ptr::null_mut(),
                0,
                struct_size::<McTerrainGridVertexV2>() - 1,
                &raw mut written,
            ),
            MC_STATUS_STRUCT_TOO_SMALL
        );
        assert_eq!(
            mc_native_session_terrain_grid_copy(
                handle,
                std::ptr::null_mut(),
                0,
                struct_size::<McTerrainGridVertexV2>() + 1,
                &raw mut written,
            ),
            MC_STATUS_INVALID_ARGUMENT
        );

        let short_query_header = AlignedAbiHeader {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: 8,
        };
        let mut batch = McEventBatchV2::default();
        assert_eq!(
            mc_native_events_copy_since(
                handle,
                (&raw const short_query_header).cast::<McEventQueryV2>(),
                std::ptr::null_mut(),
                0,
                struct_size::<McEventV2>(),
                &raw mut batch,
                struct_size::<McEventBatchV2>(),
            ),
            MC_STATUS_STRUCT_TOO_SMALL
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn bounded_event_journal_reports_expired_cursors() {
        let handle = open_fixture();
        let command = valid_command();
        let mut snapshot = McSessionSnapshotV2::default();
        for _ in 0..=EVENT_CAPACITY {
            assert_eq!(
                mc_native_session_step(
                    handle,
                    &raw const command,
                    &raw mut snapshot,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
        }
        let query = McEventQueryV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McEventQueryV2>(),
            generation: snapshot.generation,
            cursor: 0,
            reserved: [0; 4],
        };
        let mut batch = McEventBatchV2::default();
        assert_eq!(
            mc_native_events_copy_since(
                handle,
                &raw const query,
                std::ptr::null_mut(),
                0,
                struct_size::<McEventV2>(),
                &raw mut batch,
                struct_size::<McEventBatchV2>(),
            ),
            MC_STATUS_CURSOR_EXPIRED
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn panic_is_contained_at_the_native_boundary() {
        assert_eq!(
            boundary(|| -> AbiResult<()> { panic!("intentional ABI containment probe") }),
            MC_STATUS_INTERNAL_PANIC
        );
        LAST_ERROR.with(|error| {
            assert!(error.borrow().contains("panic contained"));
        });
    }

    #[test]
    fn zero_capacity_event_read_preserves_the_cursor() {
        let handle = open_fixture();
        let mut command = valid_command();
        command.flags = 0x10;
        let mut snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let query = McEventQueryV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McEventQueryV2>(),
            generation: snapshot.generation,
            cursor: 0,
            reserved: [0; 4],
        };
        let mut batch = McEventBatchV2::default();
        assert_eq!(
            mc_native_events_copy_since(
                handle,
                &raw const query,
                std::ptr::null_mut(),
                0,
                struct_size::<McEventV2>(),
                &raw mut batch,
                struct_size::<McEventBatchV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(batch.written, 0);
        assert_eq!(batch.next_cursor, query.cursor);
        assert_eq!(batch.has_more, 1);
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn null_utf8_version_size_reserved_and_unknown_handle_are_distinct() {
        let mut handle = 0_u64;
        assert_eq!(
            mc_native_session_open(std::ptr::null(), 1, std::ptr::null(), 1, &raw mut handle),
            MC_STATUS_NULL_POINTER
        );
        let bad_utf8 = [0xff_u8];
        assert_eq!(
            mc_native_session_open(bad_utf8.as_ptr(), 1, bad_utf8.as_ptr(), 1, &raw mut handle),
            MC_STATUS_INVALID_UTF8
        );
        let handle = open_fixture();
        let mut snapshot = McSessionSnapshotV2::default();
        let mut command = valid_command();
        command.abi_version = 99;
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_UNSUPPORTED_VERSION
        );
        command = valid_command();
        command.struct_size = 4;
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_STRUCT_TOO_SMALL
        );
        command = valid_command();
        command.reserved[0] = 1;
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_RESERVED_NONZERO
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
        assert_eq!(
            mc_native_session_snapshot(
                handle,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_UNKNOWN_HANDLE
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_UNKNOWN_HANDLE);
    }

    #[test]
    fn level_one_exit_sidecar_holds_completed_action_and_resets_atomically() {
        let handle = open_fixture();
        let mut initial = McLevel1ExitPresentationV2::default();
        assert_eq!(
            mc_native_level1_exit_presentation(
                handle,
                &raw mut initial,
                struct_size::<McLevel1ExitPresentationV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(initial.authority, MC_AUTHORITY_LEVEL1_MISSION_LIVE);
        assert_eq!(
            initial.flags,
            MC_LEVEL1_EXIT_FLAG_PALETTE_VISUAL_CURRENT
                | MC_LEVEL1_EXIT_FLAG_FOV_POST_ACTION_NEXT_HISTORY
        );
        assert_eq!(
            (
                initial.player_action,
                initial.action_phase,
                initial.palette_submode,
                initial.palette_effect,
                initial.palette_weight,
                initial.palette_denominator,
                initial.camera_fov,
                initial.active,
                initial.complete,
            ),
            (0, 0, 0, 0, 0, 0, 128, 0, 0)
        );

        with_session_mut(handle, |session| {
            session.camera_fov = 121;
            session.mission.exit_player_action = 12;
            session.mission.exit_sequence_phase = 12;
            session.mission.exit_sequence_active = false;
            session.mission.exit_sequence_complete = true;
            session.mission.exit_result_code = 10;
            session.mission.exit_player_status_flags = 10;
            session.mission.exit_palette_effect = 2;
            session.mission.exit_palette_weight = 28;
            session.mission.exit_palette_denominator = 28;
            session.mission.exit_end_sequence_flag = true;
            session.mission.exit_display_blank = true;
            session.mission.player_mana = 123;
            Ok(())
        })
        .unwrap();

        let mut exit = McLevel1ExitPresentationV2::default();
        assert_eq!(
            mc_native_level1_exit_presentation(
                handle,
                &raw mut exit,
                struct_size::<McLevel1ExitPresentationV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            (
                exit.player_action,
                exit.action_phase,
                exit.palette_effect,
                exit.palette_weight,
                exit.palette_denominator,
                exit.camera_fov,
                exit.player_status_flags,
                exit.input_suppressed,
                exit.display_blank,
                exit.result_code,
                exit.active,
                exit.complete,
            ),
            (12, 12, 2, 28, 28, 121, 10, 1, 1, 10, 0, 1)
        );
        assert_eq!(
            exit.flags,
            MC_LEVEL1_EXIT_FLAG_PALETTE_VISUAL_CURRENT
                | MC_LEVEL1_EXIT_FLAG_FOV_POST_ACTION_NEXT_HISTORY
                | MC_LEVEL1_EXIT_FLAG_END_SEQUENCE
        );

        let mut before = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_snapshot(
                handle,
                &raw mut before,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let command = McCommandV2 {
            flags: 1,
            roll: i8::MAX,
            pitch: i8::MAX,
            ..valid_command()
        };
        let mut after = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut after,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            (after.x, after.y, after.yaw, after.actual_speed),
            (before.x, before.y, before.yaw, before.actual_speed)
        );
        assert_eq!(after.tick, before.tick + 1);
        let held_mana = with_session(handle, |session| Ok(session.mission.player_mana)).unwrap();
        assert_eq!(held_mana, 123);

        let mut reset = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_reset(handle, &raw mut reset, struct_size::<McSessionSnapshotV2>(),),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_level1_exit_presentation(
                handle,
                &raw mut exit,
                struct_size::<McLevel1ExitPresentationV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(exit.generation, reset.generation);
        assert_eq!(exit.tick, reset.tick);
        assert_eq!(
            (
                exit.player_action,
                exit.action_phase,
                exit.palette_effect,
                exit.camera_fov,
                exit.input_suppressed,
                exit.result_code,
                exit.complete,
            ),
            (0, 0, 0, 128, 0, 0, 0)
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn level_one_exit_sidecar_marks_post_action_fov_one_history_tick_ahead() {
        let handle = open_fixture();
        with_session_mut(handle, |session| {
            session.mission.exit_player_action = 12;
            session.mission.exit_sequence_active = true;
            session.mission.exit_sequence_phase = 5;
            session.mission.exit_sequence_timer = 12;
            session.mission.exit_player_status_flags = 0x20;
            Ok(())
        })
        .unwrap();
        let command = valid_command();
        let mut snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let mut history = McPresentationStateV2::default();
        let mut exit = McLevel1ExitPresentationV2::default();
        assert_eq!(
            mc_native_presentation_state(
                handle,
                &raw mut history,
                struct_size::<McPresentationStateV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_level1_exit_presentation(
                handle,
                &raw mut exit,
                struct_size::<McLevel1ExitPresentationV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(history.camera_fov, 128);
        assert_eq!(exit.camera_fov, 127);
        assert_ne!(
            exit.flags & MC_LEVEL1_EXIT_FLAG_FOV_POST_ACTION_NEXT_HISTORY,
            0
        );

        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_presentation_state(
                handle,
                &raw mut history,
                struct_size::<McPresentationStateV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_level1_exit_presentation(
                handle,
                &raw mut exit,
                struct_size::<McLevel1ExitPresentationV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(history.camera_fov, 127);
        assert_eq!(exit.camera_fov, 126);
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn level_one_exit_sidecar_isolated_sessions_replay_independently() {
        let first = open_fixture();
        let second = open_fixture();
        with_session_mut(first, |session| {
            session.mission.exit_player_action = 12;
            session.mission.exit_sequence_active = true;
            session.mission.exit_sequence_phase = 5;
            session.mission.exit_sequence_timer = 12;
            session.mission.exit_player_status_flags = 0x20;
            Ok(())
        })
        .unwrap();
        let command = valid_command();
        let mut snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                first,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let mut first_exit = McLevel1ExitPresentationV2::default();
        let mut second_exit = McLevel1ExitPresentationV2::default();
        assert_eq!(
            mc_native_level1_exit_presentation(
                first,
                &raw mut first_exit,
                struct_size::<McLevel1ExitPresentationV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_level1_exit_presentation(
                second,
                &raw mut second_exit,
                struct_size::<McLevel1ExitPresentationV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            (
                first_exit.player_action,
                first_exit.action_phase,
                first_exit.camera_fov,
                first_exit.active,
            ),
            (12, 5, 127, 1)
        );
        assert_eq!(
            (
                second_exit.player_action,
                second_exit.action_phase,
                second_exit.camera_fov,
                second_exit.active,
            ),
            (0, 0, 128, 0)
        );

        assert_eq!(
            mc_native_session_reset(
                first,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_level1_exit_presentation(
                first,
                &raw mut first_exit,
                struct_size::<McLevel1ExitPresentationV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            (
                first_exit.player_action,
                first_exit.action_phase,
                first_exit.camera_fov,
                first_exit.active,
            ),
            (0, 0, 128, 0)
        );
        assert_eq!(mc_native_session_destroy(first), MC_STATUS_OK);
        assert_eq!(mc_native_session_destroy(second), MC_STATUS_OK);
    }

    #[test]
    fn level_one_mission_and_fireball_cross_the_generation_checked_abi() {
        let handle = open_fixture();
        let mut mission = McLevel1MissionSnapshotV2::default();
        assert_eq!(
            mc_native_level1_mission_snapshot(
                handle,
                &raw mut mission,
                struct_size::<McLevel1MissionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(mission.authority, MC_AUTHORITY_LEVEL1_MISSION_LIVE);
        assert_eq!((mission.current_stage, mission.text_selector), (0, 48));
        assert_eq!((mission.player_life, mission.player_mana), (10_000, 1_000));
        assert_eq!(mission.actor_count, 45);
        assert_eq!((mission.projectile_count, mission.impact_count), (0, 0));
        assert_eq!(mission.tasks_completed, 0);
        assert_eq!(mission.exit_sequence_active, 0);
        assert_eq!(mission.exit_sequence_complete, 0);
        assert_eq!(mission.exit_result_code, 0);
        assert_eq!(mission.exit_sequence_phase, 0);
        assert_eq!(mission.exit_sequence_timer, 0);
        let mut hud = McLevel1HudSnapshotV2::default();
        assert_eq!(
            mc_native_level1_hud_snapshot(
                handle,
                &raw mut hud,
                struct_size::<McLevel1HudSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            (hud.generation, hud.tick),
            (mission.generation, mission.tick)
        );
        assert_eq!(hud.authority, MC_AUTHORITY_LEVEL1_MISSION_LIVE);
        assert_eq!(
            (
                hud.mana_color_index,
                hud.mana_max_color_index,
                hud.life_color_index,
                hud.color_phase_1,
                hud.color_phase_2,
            ),
            (0xA4, 0xAA, 0x7B, 0, 0)
        );
        let mut spell_menu = McLevel1SpellMenuSnapshotV2::default();
        assert_eq!(
            mc_native_level1_spell_menu_snapshot(
                handle,
                &raw mut spell_menu,
                struct_size::<McLevel1SpellMenuSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            (spell_menu.generation, spell_menu.tick),
            (mission.generation, mission.tick)
        );
        assert_eq!(spell_menu.authority, MC_AUTHORITY_LEVEL1_MISSION_LIVE);
        assert_eq!(
            (
                spell_menu.selected_ui_index,
                spell_menu.selected_tier,
                spell_menu.left_family,
                spell_menu.left_tier,
                spell_menu.right_family,
                spell_menu.right_tier,
            ),
            (0, 0, 0, 0, 1, 0)
        );
        assert_eq!(spell_menu.family_enabled, [1, 1, 0, 0]);
        assert_eq!(spell_menu.family_unlocked_tier, [0; 4]);
        assert_eq!(spell_menu.family_selected_tier, [0; 4]);
        assert_eq!(spell_menu.family_hand_eligibility, [0; 4]);
        assert_ne!(spell_menu.holder_entity_index[0], 0);
        assert_ne!(spell_menu.holder_entity_index[1], 0);
        assert!(hud.global_mana_denominator > 0);
        assert_eq!((hud.secured_mana_component, hud.goal_percent), (0, 0));
        assert_eq!(
            (
                hud.left.holder_entity_index,
                hud.left.parent_entity_index,
                hud.left.model,
                hud.left.tier,
                hud.left.active_value,
                hud.left.max_mana,
                hud.left.caster_mana,
                hud.left.frame_selector,
                hud.left.icon_selector,
            ),
            (153, 152, 0, 0, 0, 100, 1_000, 1, 123)
        );
        assert_eq!(
            (
                hud.right.holder_entity_index,
                hud.right.model,
                hud.right.active_value,
                hud.right.max_mana,
                hud.right.frame_selector,
                hud.right.icon_selector,
            ),
            (154, 1, 0, 100, 1, 124)
        );
        assert_eq!(
            (
                hud.selected_holder_entity_index,
                hud.selected_model,
                hud.selected_tier,
                hud.selected_frame_selector,
                hud.selected_icon_selector,
            ),
            (153, 0, 0, 1, 123)
        );
        let mut minimap = McLevel1MinimapMarksV2::default();
        assert_eq!(
            mc_native_level1_minimap_marks(
                handle,
                &raw mut minimap,
                struct_size::<McLevel1MinimapMarksV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            (minimap.generation, minimap.tick),
            (mission.generation, mission.tick)
        );
        assert_eq!(
            (
                minimap.mark_count,
                minimap.mark_pulse,
                minimap.arrow_valid,
                minimap.arrow_visible,
                minimap.mark_colour[0],
                minimap.mark_help_selector[0],
            ),
            (1, 1, 1, 0, 184, 0x51)
        );
        assert_eq!(minimap.reserved, [0; 4]);
        let expected_minimap_commands = with_session(handle, |session| {
            Ok(level1_minimap_entity_commands(session))
        })
        .unwrap();
        assert!(!expected_minimap_commands.is_empty());
        assert!(expected_minimap_commands.windows(2).all(|pair| {
            pair[0].kind == LEVEL1_MINIMAP_COMMAND_BLEND_PIXEL
                || pair[0].source_slot <= pair[1].source_slot
        }));
        let civilian_slots = with_session(handle, |session| {
            Ok(session
                .mission
                .source_slots()
                .iter()
                .filter(|slot| slot.active && slot.class == 5 && (12..=14).contains(&slot.model))
                .map(|slot| slot.index)
                .collect::<Vec<_>>())
        })
        .unwrap();
        assert_eq!(civilian_slots.len(), 28);
        for source_slot in civilian_slots {
            assert!(expected_minimap_commands.iter().any(|command| {
                command.source_slot == source_slot
                    && command.kind == sim_core::level1_mission::LEVEL1_MINIMAP_COMMAND_DOT
                    && command.palette_index == 23
            }));
        }
        let mut entity_commands =
            [McLevel1MinimapEntityCommandV2::default(); LEVEL1_MINIMAP_MAX_ENTITY_COMMANDS];
        let mut entity_info = McLevel1MinimapEntitiesInfoV2::default();
        assert_eq!(
            mc_native_level1_minimap_entities_copy(
                handle,
                mission.generation,
                mission.tick,
                entity_commands.as_mut_ptr(),
                entity_commands.len() as u32,
                struct_size::<McLevel1MinimapEntityCommandV2>(),
                &raw mut entity_info,
                struct_size::<McLevel1MinimapEntitiesInfoV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            entity_info.total_count as usize,
            expected_minimap_commands.len()
        );
        assert_eq!(
            entity_info.command_hash,
            level1_minimap_command_hash(&expected_minimap_commands)
        );
        assert_eq!(
            (
                entity_info.cross_x,
                entity_info.cross_y,
                entity_info.cross_colour,
                entity_info.cross_arm_length,
                entity_info.authority,
            ),
            (63, 64, 176, 10, MC_AUTHORITY_LEVEL1_MISSION_LIVE)
        );
        assert_eq!(
            &entity_commands[..expected_minimap_commands.len()],
            expected_minimap_commands.as_slice()
        );
        assert_eq!(entity_info.reserved_0, [0; 4]);
        assert_eq!(entity_info.reserved, [0; 4]);
        assert_eq!(
            mission.fidelity_gap_flags & MC_LEVEL1_GAP_CASTLE_BUILD_FOOTPRINT,
            0
        );
        assert_eq!(
            mission.fidelity_gap_flags & MC_LEVEL1_GAP_CASTLE_CONSTRUCTION_TIMING,
            0
        );
        assert_eq!(
            mission.fidelity_gap_flags & MC_LEVEL1_GAP_MANA_SPHERE_ALLOCATION_RNG,
            0
        );
        assert_eq!(
            mission.fidelity_gap_flags & MC_LEVEL1_GAP_MANA_CARRIER_FLIGHT,
            0
        );
        assert_eq!(mission.fidelity_gap_flags, 0);

        let mut actors = [McLevel1ActorSnapshotV2::default(); 45];
        let mut written = 0;
        assert_eq!(
            mc_native_level1_actors_copy(
                handle,
                mission.generation,
                mission.tick,
                0,
                actors.as_mut_ptr(),
                actors.len() as u32,
                struct_size::<McLevel1ActorSnapshotV2>(),
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 45);
        assert_eq!(
            actors
                .iter()
                .filter(|actor| {
                    actor.kind == sim_core::level1_mission::Level1ActorKind::Archer as u8
                })
                .count(),
            4
        );
        assert_eq!(
            actors
                .iter()
                .filter(|actor| {
                    actor.kind == sim_core::level1_mission::Level1ActorKind::Firefly as u8
                })
                .count(),
            6
        );
        assert_eq!(
            actors
                .iter()
                .filter(|actor| {
                    actor.kind == sim_core::level1_mission::Level1ActorKind::MultipartHead as u8
                })
                .count(),
            4
        );
        assert_eq!(
            actors
                .iter()
                .filter(|actor| {
                    actor.kind == sim_core::level1_mission::Level1ActorKind::ExperienceScroll as u8
                })
                .count(),
            5
        );
        assert_eq!(
            actors
                .iter()
                .filter(|actor| {
                    actor.kind == sim_core::level1_mission::Level1ActorKind::Tree as u8
                        && actor.render_parameter_index
                            == sim_core::level1_mission::TREE_RENDER_INDEX_A
                        && actor.base_tmap == sim_core::level1_mission::TREE_BASE_TMAP_A
                })
                .count(),
            18
        );
        assert_eq!(
            actors
                .iter()
                .filter(|actor| {
                    actor.kind == sim_core::level1_mission::Level1ActorKind::Stone as u8
                        && actor.render_parameter_index
                            == sim_core::level1_mission::STONE_RENDER_INDEX
                        && actor.base_tmap == sim_core::level1_mission::STONE_BASE_TMAP
                })
                .count(),
            1
        );
        assert_eq!(
            actors
                .iter()
                .filter(|actor| {
                    actor.kind
                        == sim_core::level1_mission::Level1ActorKind::CheckpointSmokeEmitter as u8
                        && actor.visible == 0
                        && actor.render_parameter_index == u16::MAX
                })
                .count(),
            3
        );
        assert_eq!(
            actors
                .iter()
                .filter(|actor| {
                    actor.kind == sim_core::level1_mission::Level1ActorKind::ExitTarget as u8
                        && actor.visible == 0
                        && actor.base_tmap == 461
                })
                .count(),
            1
        );
        assert_eq!(
            actors
                .iter()
                .filter(|actor| {
                    actor.kind == sim_core::level1_mission::Level1ActorKind::ExitSwitch as u8
                        && actor.visible == 0
                        && actor.render_parameter_index == u16::MAX
                })
                .count(),
            1
        );
        assert!(actors.iter().all(|actor| actor.active == 0));

        let fire = McCommandV2 {
            flags: 0x10,
            ..valid_command()
        };
        let mut flight = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const fire,
                &raw mut flight,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_level1_mission_snapshot(
                handle,
                &raw mut mission,
                struct_size::<McLevel1MissionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(mission.tick, flight.tick);
        assert!(mission.projectile_count + mission.impact_count > 0);

        if mission.projectile_count != 0 {
            let mut projectile = McFireballProjectileSnapshotV2::default();
            assert_eq!(
                mc_native_fireball_projectiles_copy(
                    handle,
                    mission.generation,
                    mission.tick,
                    0,
                    &raw mut projectile,
                    1,
                    struct_size::<McFireballProjectileSnapshotV2>(),
                    &raw mut written,
                ),
                MC_STATUS_OK
            );
            assert_eq!(written, 1);
            assert_eq!(projectile.authority, MC_AUTHORITY_LEVEL1_MISSION_LIVE);
            assert_eq!(projectile.projectile_kind, 1);
            assert_eq!(projectile.render_index, FIREBALL_RENDER_INDEX);
            assert_ne!(
                projectile.reserved[0], 0,
                "reserved word 0 preserves source-slot identity independently of projectile_id"
            );
        } else {
            let mut impact = McFireballImpactSnapshotV2::default();
            assert_eq!(
                mc_native_fireball_impacts_copy(
                    handle,
                    mission.generation,
                    mission.tick,
                    0,
                    &raw mut impact,
                    1,
                    struct_size::<McFireballImpactSnapshotV2>(),
                    &raw mut written,
                ),
                MC_STATUS_OK
            );
            assert_eq!(written, 1);
            assert_eq!(impact.authority, MC_AUTHORITY_LEVEL1_MISSION_LIVE);
            assert_ne!(
                impact.reserved[0], 0,
                "reserved word 0 preserves source-slot identity independently of impact_id"
            );
        }

        assert_eq!(
            mc_native_level1_actors_copy(
                handle,
                mission.generation,
                mission.tick.saturating_sub(1),
                0,
                std::ptr::null_mut(),
                0,
                struct_size::<McLevel1ActorSnapshotV2>(),
                &raw mut written,
            ),
            MC_STATUS_GENERATION_MISMATCH
        );

        let mut reset = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_reset(handle, &raw mut reset, struct_size::<McSessionSnapshotV2>(),),
            MC_STATUS_OK
        );
        let idle = valid_command();
        let (tree_source_slot, tree_authored_index, tree_position) =
            with_session_mut(handle, |session| {
                let activation_target = session
                    .world
                    .level1_mission()
                    .possession_targets()
                    .first()
                    .map(|target| target.entity_index)
                    .ok_or_else(|| {
                        AbiError::new(
                            MC_STATUS_VALIDATION_FAILED,
                            "qualified Level 1 has no imported house activation target",
                        )
                    })?;
                session
                    .mission
                    .commit_imported_house_demolition(activation_target, 1, &[])
                    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?
                    .ok_or_else(|| {
                        AbiError::new(
                            MC_STATUS_VALIDATION_FAILED,
                            "production xtype-one activation failed",
                        )
                    })?;
                session
                    .mission
                    .actors()
                    .iter()
                    .find(|actor| actor.kind == Level1ActorKind::Tree && actor.active)
                    .map(|actor| {
                        (
                            actor.source_entity_index,
                            actor.authored_index,
                            actor.position,
                        )
                    })
                    .ok_or_else(|| {
                        AbiError::new(
                            MC_STATUS_VALIDATION_FAILED,
                            "group-one activation produced no live tree",
                        )
                    })
            })
            .unwrap();

        for shot in 0..2 {
            with_session_mut(handle, |session| {
                session.state.position = Position {
                    x: tree_position.x.wrapping_sub(512),
                    y: tree_position.y,
                    z: tree_position.z.wrapping_add(100),
                };
                session.state.yaw = 512;
                session.state.pitch_accumulator = 0;
                session.state.effective_pitch = 0;
                session.state.target_speed = 0;
                session.state.actual_speed = 0;
                session.state.strafe_speed = 0;
                Ok(())
            })
            .unwrap();
            assert_eq!(
                mc_native_session_step(
                    handle,
                    &raw const fire,
                    &raw mut flight,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
            let mut reached = false;
            for _ in 0..24 {
                reached = with_session(handle, |session| {
                    let tree = session
                        .mission
                        .actors()
                        .iter()
                        .find(|actor| actor.source_entity_index == tree_source_slot)
                        .ok_or_else(|| {
                            AbiError::new(
                                MC_STATUS_VALIDATION_FAILED,
                                "production tree source row disappeared",
                            )
                        })?;
                    Ok(if shot == 0 {
                        tree.life < sim_core::level1_mission::TREE_LIFE
                    } else {
                        session
                            .mission
                            .impacts()
                            .iter()
                            .any(|impact| impact.model == GROUND_FIRE_MODEL && impact.active)
                    })
                })
                .unwrap();
                if reached {
                    break;
                }
                assert_eq!(
                    mc_native_session_step(
                        handle,
                        &raw const idle,
                        &raw mut flight,
                        struct_size::<McSessionSnapshotV2>(),
                    ),
                    MC_STATUS_OK
                );
            }
            assert!(
                reached,
                "production shot {shot} reaches the real tree lifecycle"
            );
        }

        let query = McEventQueryV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McEventQueryV2>(),
            generation: flight.generation,
            cursor: 0,
            reserved: [0; 4],
        };
        let mut events = vec![McEventV2::default(); 256];
        let mut batch = McEventBatchV2::default();
        assert_eq!(
            mc_native_events_copy_since(
                handle,
                &raw const query,
                events.as_mut_ptr(),
                u32::try_from(events.len()).unwrap(),
                struct_size::<McEventV2>(),
                &raw mut batch,
                struct_size::<McEventBatchV2>(),
            ),
            MC_STATUS_OK
        );
        events.truncate(usize::try_from(batch.written).unwrap());
        assert!(events.iter().any(|event| {
            event.kind == MC_EVENT_LEVEL1_ACTOR_DAMAGED
                && event.entity_index == tree_authored_index
                && event.code == u32::from(flight.entity_index)
        }));

        let first_ground_fire = read_level1_impacts(handle)
            .into_iter()
            .find(|impact| impact.model == GROUND_FIRE_MODEL && impact.active != 0)
            .expect("the second production hit creates live class-10/model-6 ground fire");
        assert_eq!(
            (
                first_ground_fire.class_id,
                first_ground_fire.render_parameter_index,
                first_ground_fire.base_tmap,
                first_ground_fire.render_mode,
                first_ground_fire.logical_height,
                first_ground_fire.animation_frame,
                first_ground_fire.authority,
            ),
            (
                FIREBALL_IMPACT_CLASS,
                228,
                77,
                1,
                50,
                0,
                MC_AUTHORITY_LEVEL1_MISSION_LIVE,
            )
        );
        assert!(
            first_ground_fire.reserved[0] > 0 && first_ground_fire.reserved[0] < 1_000,
            "the production ground fire retains its real source-arena slot"
        );
        with_session_mut(handle, |session| {
            assert!(
                session
                    .renderable_map_chains
                    .tracks(u16::try_from(first_ground_fire.reserved[0]).unwrap()),
                "model 6 follows NewAdd0A06's AddEventToMap path"
            );
            session.state.position = Position {
                x: tree_position.x.wrapping_add(16_384),
                y: tree_position.y.wrapping_add(16_384),
                z: tree_position.z.wrapping_add(2_000),
            };
            session.state.target_speed = 0;
            session.state.actual_speed = 0;
            session.state.strafe_speed = 0;
            session.mission.player_life = 1_000_000;
            Ok(())
        })
        .unwrap();

        let mut changed_rows = Vec::new();
        for _ in 0..256 {
            let Some(ground_fire) = read_level1_impacts(handle).into_iter().find(|impact| {
                impact.impact_id == first_ground_fire.impact_id && impact.active != 0
            }) else {
                break;
            };
            let phase = usize::from(ground_fire.render_parameter_index.saturating_sub(228));
            let expected_heights = [50, 150, 300, 450, 600, 700, 800, 950];
            assert!(
                phase < expected_heights.len(),
                "ground-fire row {} lies outside 228..235",
                ground_fire.render_parameter_index
            );
            assert_eq!(
                (
                    ground_fire.base_tmap,
                    ground_fire.render_mode,
                    ground_fire.logical_height,
                    ground_fire.animation_frame,
                ),
                (77, 1, expected_heights[phase], 0)
            );
            if changed_rows.last().copied() != Some(ground_fire.render_parameter_index) {
                changed_rows.push(ground_fire.render_parameter_index);
            }
            assert_eq!(
                mc_native_session_step(
                    handle,
                    &raw const idle,
                    &raw mut flight,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
        }
        assert_eq!(
            changed_rows,
            [
                228, 229, 230, 231, 232, 233, 234, 235, 234, 233, 232, 231, 230, 229, 228
            ],
            "the real tree fire expands and contracts through the source phase table"
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    fn configure_shared_possession_holder(handle: u64) {
        with_session_mut(handle, |session| {
            session.mission.possession_experience = 4_800;
            session.mission.possession_tier = 2;
            let fire =
                session.context.spell_fire.as_mut().ok_or_else(|| {
                    AbiError::new(MC_STATUS_VALIDATION_FAILED, "spell state absent")
                })?;
            fire.selected_holder_index = 153;
            fire.selected_spell_ui_index = 0;
            fire.selected_subspell_index = 0;
            fire.spell_levels[1] = 2;
            fire.family_selected_tiers[1] = 2;
            fire.holders
                .iter_mut()
                .find(|holder| holder.entity_index == 154)
                .ok_or_else(|| {
                    AbiError::new(MC_STATUS_VALIDATION_FAILED, "possession holder absent")
                })?
                .level_gate = 2;
            Ok(())
        })
        .unwrap();
    }

    fn submit_spell_menu_command(
        handle: u64,
        snapshot: &mut McSessionSnapshotV2,
        opcode: i8,
        argument_1: i8,
        argument_2: i8,
    ) {
        let command = McCommandV2 {
            opcode,
            argument_1,
            argument_2,
            ..valid_command()
        };
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut *snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
    }

    fn read_level1_hud(handle: u64) -> McLevel1HudSnapshotV2 {
        let mut hud = McLevel1HudSnapshotV2::default();
        assert_eq!(
            mc_native_level1_hud_snapshot(
                handle,
                &raw mut hud,
                struct_size::<McLevel1HudSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        hud
    }

    fn read_level1_spell_menu(handle: u64) -> McLevel1SpellMenuSnapshotV2 {
        let mut menu = McLevel1SpellMenuSnapshotV2::default();
        assert_eq!(
            mc_native_level1_spell_menu_snapshot(
                handle,
                &raw mut menu,
                struct_size::<McLevel1SpellMenuSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        menu
    }

    fn read_level1_impacts(handle: u64) -> Vec<McFireballImpactSnapshotV2> {
        let mut mission = McLevel1MissionSnapshotV2::default();
        assert_eq!(
            mc_native_level1_mission_snapshot(
                handle,
                &raw mut mission,
                struct_size::<McLevel1MissionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let mut impacts = vec![
            McFireballImpactSnapshotV2::default();
            usize::try_from(mission.impact_count).unwrap()
        ];
        let mut written = 0;
        assert_eq!(
            mc_native_fireball_impacts_copy(
                handle,
                mission.generation,
                mission.tick,
                0,
                impacts.as_mut_ptr(),
                mission.impact_count,
                struct_size::<McFireballImpactSnapshotV2>(),
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, mission.impact_count);
        impacts
    }

    fn assert_raw_tier_and_family_minus_one(handle: u64, snapshot: &mut McSessionSnapshotV2) {
        submit_spell_menu_command(handle, snapshot, 40, 0, 1);
        let cursor_before_raw = with_session(handle, |session| {
            let fire =
                session.context.spell_fire.as_ref().ok_or_else(|| {
                    AbiError::new(MC_STATUS_VALIDATION_FAILED, "spell state absent")
                })?;
            Ok((
                fire.selected_spell_ui_index,
                fire.selected_subspell_index,
                fire.selected_holder_index,
            ))
        })
        .unwrap();
        submit_spell_menu_command(handle, snapshot, 31, 1, 7);
        let menu = read_level1_spell_menu(handle);
        assert_eq!((menu.left_family, menu.left_tier), (1, 7));
        assert_eq!(menu.family_selected_tier[1], 7);
        let (cursor_after_raw, holder_tier) = with_session(handle, |session| {
            let fire =
                session.context.spell_fire.as_ref().ok_or_else(|| {
                    AbiError::new(MC_STATUS_VALIDATION_FAILED, "spell state absent")
                })?;
            let holder_tier = fire
                .holders
                .iter()
                .find(|holder| holder.entity_index == 154)
                .ok_or_else(|| {
                    AbiError::new(MC_STATUS_VALIDATION_FAILED, "possession holder absent")
                })?
                .level_gate;
            Ok((
                (
                    fire.selected_spell_ui_index,
                    fire.selected_subspell_index,
                    fire.selected_holder_index,
                ),
                holder_tier,
            ))
        })
        .unwrap();
        assert_eq!(cursor_after_raw, cursor_before_raw);
        assert_eq!(holder_tier, 2);

        submit_spell_menu_command(handle, snapshot, 40, 1, 1);
        let reopened = read_level1_spell_menu(handle);
        assert_eq!(
            (
                reopened.selected_ui_index,
                reopened.selected_tier,
                reopened.family_selected_tier[1],
            ),
            (1, 7, 7),
            "family press exposes the raw current tier before held selection clamps it"
        );
        submit_spell_menu_command(handle, snapshot, 41, 2, 0);
        let before_no_op = read_level1_spell_menu(handle);
        assert_eq!(
            (
                before_no_op.selected_tier,
                before_no_op.family_selected_tier[1],
            ),
            (2, 2)
        );
        submit_spell_menu_command(handle, snapshot, 32, -1, 7);
        let menu = read_level1_spell_menu(handle);
        assert_eq!(
            (
                menu.selected_ui_index,
                menu.selected_tier,
                menu.left_family,
                menu.left_tier,
                menu.right_family,
                menu.right_tier,
                menu.family_selected_tier,
                menu.holder_entity_index,
            ),
            (
                before_no_op.selected_ui_index,
                before_no_op.selected_tier,
                before_no_op.left_family,
                before_no_op.left_tier,
                before_no_op.right_family,
                before_no_op.right_tier,
                before_no_op.family_selected_tier,
                before_no_op.holder_entity_index,
            )
        );
    }

    fn assert_spell_menu_cast_routes(handle: u64, snapshot: &mut McSessionSnapshotV2) {
        submit_spell_menu_command(handle, snapshot, 31, 1, 0);
        submit_spell_menu_command(handle, snapshot, 32, 0, 0);
        submit_spell_menu_command(handle, snapshot, 38, 0, 2);
        let menu = read_level1_spell_menu(handle);
        assert_eq!(menu.family_hand_eligibility, [2, 0, 0, 0]);
        for (flags, expected_mask) in [(0x10_i8, 256_u32), (0x20_i8, 512_u32)] {
            let command = McCommandV2 {
                flags,
                ..valid_command()
            };
            assert_eq!(
                mc_native_session_step(
                    handle,
                    &raw const command,
                    &raw mut *snapshot,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
            let (left_holder, right_holder, direction_mask) = with_session(handle, |session| {
                let fire = session.context.spell_fire.as_ref().ok_or_else(|| {
                    AbiError::new(MC_STATUS_VALIDATION_FAILED, "spell state absent")
                })?;
                Ok((
                    fire.left_holder_index,
                    fire.right_holder_index,
                    fire.caster.control_flags & 0x0300,
                ))
            })
            .unwrap();
            assert_eq!((left_holder, right_holder), (154, 153));
            assert_eq!(direction_mask, expected_mask);
        }
    }

    #[test]
    fn top_hud_uses_shared_holder_tier_while_menu_preserves_per_hand_tiers() {
        let handle = open_fixture();
        configure_shared_possession_holder(handle);

        let mut snapshot = McSessionSnapshotV2::default();
        assert_eq!(read_level1_hud(handle).selected_model, 0);
        submit_spell_menu_command(handle, &mut snapshot, 40, 1, 1);
        let pressed_hud = read_level1_hud(handle);
        assert_eq!(
            (pressed_hud.selected_model, pressed_hud.selected_tier),
            (1, 2)
        );
        let selected_fire = McCommandV2 {
            flags: 0x40,
            ..valid_command()
        };
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const selected_fire,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let selected_route = with_session(handle, |session| {
            let fire =
                session.context.spell_fire.as_ref().ok_or_else(|| {
                    AbiError::new(MC_STATUS_VALIDATION_FAILED, "spell state absent")
                })?;
            Ok((
                fire.selected_holder_index,
                fire.accepted_models.clone(),
                fire.caster.control_flags & 0x0300,
            ))
        })
        .unwrap();
        assert_eq!(selected_route, (154, vec![1], 256));
        submit_spell_menu_command(handle, &mut snapshot, 41, 2, 0);
        submit_spell_menu_command(handle, &mut snapshot, 31, 1, 2);
        submit_spell_menu_command(handle, &mut snapshot, 40, 1, 2);
        submit_spell_menu_command(handle, &mut snapshot, 41, 1, 0);
        submit_spell_menu_command(handle, &mut snapshot, 32, 1, 1);

        let hud = read_level1_hud(handle);
        let menu = read_level1_spell_menu(handle);
        assert_eq!((hud.left.model, hud.left.tier), (1, 1));
        assert_eq!((hud.right.model, hud.right.tier), (1, 1));
        assert_eq!((hud.selected_model, hud.selected_tier), (1, 1));
        assert_eq!((menu.left_family, menu.left_tier), (1, 2));
        assert_eq!((menu.right_family, menu.right_tier), (1, 1));
        assert_eq!(menu.family_selected_tier[1], 1);

        assert_raw_tier_and_family_minus_one(handle, &mut snapshot);
        assert_spell_menu_cast_routes(handle, &mut snapshot);
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn selected_fireball_retriggers_through_the_public_command_path() {
        let handle = open_fixture();
        let mut snapshot = McSessionSnapshotV2::default();
        let left_fire = McCommandV2 {
            flags: 0x10,
            ..valid_command()
        };
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const left_fire,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let fireball_launch_count = |session: &SessionRecord| {
            session
                .mission
                .projectiles()
                .iter()
                .filter(|projectile| matches!(projectile, Level1Projectile::PlayerFireball(_)))
                .count()
                + session
                    .mission
                    .impacts()
                    .iter()
                    .filter(|impact| impact.model == FIREBALL_IMPACT_MODEL)
                    .count()
        };
        let first_count =
            with_session(handle, |session| Ok(fireball_launch_count(session))).unwrap();
        assert_eq!(first_count, 1);

        let selected_fire = McCommandV2 {
            flags: 0x40,
            requested_spell: 0,
            ..valid_command()
        };
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const selected_fire,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let second_count =
            with_session(handle, |session| Ok(fireball_launch_count(session))).unwrap();
        assert_eq!(second_count, 2);
        assert_eq!(snapshot.selected_spell_ui_index, 0);
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn right_hand_possession_countdown_retriggers_through_the_public_command_path() {
        let handle = open_fixture();
        let mut snapshot = McSessionSnapshotV2::default();
        let right_possession = McCommandV2 {
            flags: 0x20,
            ..valid_command()
        };
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const right_possession,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let holder_active = |expected| {
            with_session(handle, |session| {
                let holder = session
                    .context
                    .spell_fire
                    .as_ref()
                    .and_then(|fire| {
                        fire.holders
                            .iter()
                            .find(|holder| holder.entity_index == 154)
                    })
                    .ok_or_else(|| {
                        AbiError::new(
                            MC_STATUS_VALIDATION_FAILED,
                            "Possession holder 154 is absent",
                        )
                    })?;
                Ok(holder.active_value == expected)
            })
            .unwrap()
        };
        assert!(
            holder_active(2),
            "sub_69640 consumes the activation turn from 3 to 2"
        );

        let idle = valid_command();
        for expected in [1, 0] {
            assert_eq!(
                mc_native_session_step(
                    handle,
                    &raw const idle,
                    &raw mut snapshot,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
            assert!(holder_active(expected));
        }

        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const right_possession,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert!(
            holder_active(2),
            "the second right-hand cast starts and consumes a fresh activation"
        );

        let query = McEventQueryV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McEventQueryV2>(),
            generation: snapshot.generation,
            cursor: 0,
            reserved: [0; 4],
        };
        let mut events = vec![McEventV2::default(); 256];
        let mut batch = McEventBatchV2::default();
        assert_eq!(
            mc_native_events_copy_since(
                handle,
                &raw const query,
                events.as_mut_ptr(),
                u32::try_from(events.len()).unwrap(),
                struct_size::<McEventV2>(),
                &raw mut batch,
                struct_size::<McEventBatchV2>(),
            ),
            MC_STATUS_OK
        );
        events.truncate(usize::try_from(batch.written).unwrap());
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == MC_EVENT_LEVEL1_POSSESSION_SPAWNED)
                .count(),
            2,
            "both public right-hand casts reach the Possession projectile factory"
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn real_level_one_right_hand_possession_claims_and_composites_a_building() {
        let handle = open_fixture_with_owner_cache_and_terrain_frame_assets();
        let target = with_session_mut(handle, |session| {
            session.mission.possession_experience = 600;
            session.mission.possession_tier = 1;
            let fire =
                session.context.spell_fire.as_mut().ok_or_else(|| {
                    AbiError::new(MC_STATUS_VALIDATION_FAILED, "spell state absent")
                })?;
            fire.spell_levels[1] = 1;
            fire.family_selected_tiers[1] = 1;
            fire.holders
                .iter_mut()
                .find(|holder| holder.entity_index == 154)
                .ok_or_else(|| {
                    AbiError::new(MC_STATUS_VALIDATION_FAILED, "possession holder absent")
                })?
                .level_gate = 1;
            let target = *session
                .world
                .level1_mission()
                .possession_targets()
                .first()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "qualified Level 1 has no possession target",
                    )
                })?;
            session.state.position = Position {
                x: target.position.x,
                y: target.position.y.wrapping_add(384),
                z: target.position.z.wrapping_sub(50),
            };
            session.state.yaw = 0;
            session.state.pitch_accumulator = 0;
            session.state.effective_pitch = 0;
            session.state.target_speed = 0;
            session.state.actual_speed = 0;
            session.state.strafe_speed = 0;
            session.camera_history = CameraHistory::capture(
                &session.state,
                session.source_turn,
                session.camera_fov,
                0,
                0,
            );
            Ok(target)
        })
        .unwrap();

        let mut snapshot = McSessionSnapshotV2::default();
        let right_possession = McCommandV2 {
            flags: 0x20,
            ..valid_command()
        };
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const right_possession,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let idle = valid_command();
        let mut mission = McLevel1MissionSnapshotV2::default();
        for _ in 0..12 {
            assert_eq!(
                mc_native_level1_mission_snapshot(
                    handle,
                    &raw mut mission,
                    struct_size::<McLevel1MissionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
            if mission.possessed_building_count != 0 {
                break;
            }
            assert_eq!(
                mc_native_session_step(
                    handle,
                    &raw const idle,
                    &raw mut snapshot,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
        }
        assert_eq!(mission.possessed_building_count, 1);
        assert_eq!(mission.last_possessed_building_entity, target.entity_index);
        assert_eq!(mission.last_possessed_building_owner, snapshot.entity_index);
        assert_eq!(mission.possession_experience, 601);
        assert_eq!(mission.possession_tier, 1);
        let mut impacts = vec![
            McFireballImpactSnapshotV2::default();
            usize::try_from(mission.impact_count).unwrap()
        ];
        let mut impact_written = 0;
        assert_eq!(
            mc_native_fireball_impacts_copy(
                handle,
                mission.generation,
                mission.tick,
                0,
                impacts.as_mut_ptr(),
                mission.impact_count,
                struct_size::<McFireballImpactSnapshotV2>(),
                &raw mut impact_written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(impact_written, mission.impact_count);
        let attraction = impacts
            .iter()
            .find(|impact| impact.model == POSSESSION_ATTRACTION_MODEL)
            .expect("tier-one production collision retains its model-54 gameplay row");
        assert_eq!(
            (
                attraction.class_id,
                attraction.render_parameter_index,
                attraction.base_tmap,
                attraction.render_mode,
                attraction.logical_height,
                attraction.animation_frame,
                attraction.authority,
                attraction.active,
            ),
            (
                FIREBALL_IMPACT_CLASS,
                POSSESSION_ATTRACTION_RENDER_INDEX,
                POSSESSION_ATTRACTION_BASE_TMAP,
                POSSESSION_ATTRACTION_RENDER_MODE,
                POSSESSION_ATTRACTION_LOGICAL_HEIGHT,
                0,
                MC_AUTHORITY_LEVEL1_MISSION_LIVE,
                1,
            )
        );
        with_session(handle, |session| {
            let house = session
                .live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == target.entity_index)
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "possessed house is absent from the canonical house family",
                    )
                })?;
            assert_eq!(house.owner_entity_index, snapshot.entity_index);
            assert_eq!(house.pending_possession_owner, 0);
            assert_eq!(house.pending_possession_payload, 0);
            assert!(
                !session
                    .renderable_map_chains
                    .tracks(u16::try_from(attraction.reserved[0]).unwrap()),
                "model 54 remains in gameplay snapshots without an AddEventToMap chain"
            );
            Ok(())
        })
        .unwrap();

        // Let the temporary model-12 impact finish so any combat raster writes
        // below can only come from the persistent parameter-177 building.
        for _ in 0..12 {
            assert_eq!(
                mc_native_session_step(
                    handle,
                    &raw const idle,
                    &raw mut snapshot,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
        }
        let writes =
            with_session_mut(handle, |session| {
                assert!(
                    session
                        .mission
                        .projectiles()
                        .iter()
                        .all(|projectile| !projectile.active())
                );
                assert!(
                    session
                        .mission
                        .impacts()
                        .iter()
                        .filter(|impact| impact.model != POSSESSION_ATTRACTION_MODEL)
                        .all(|impact| !impact.active)
                );
                assert!(
                    session.mission.impacts().iter().any(|impact| {
                        impact.model == POSSESSION_ATTRACTION_MODEL && impact.active
                    }),
                    "the invisible model-54 gameplay auxiliary outlives its short model-12 sibling"
                );
                let building = session
                    .mission
                    .possessed_buildings()
                    .iter()
                    .find(|building| building.entity_index == target.entity_index)
                    .copied()
                    .ok_or_else(|| {
                        AbiError::new(
                            MC_STATUS_VALIDATION_FAILED,
                            "possessed Level 1 building disappeared",
                        )
                    })?;
                assert_eq!(building.owner_entity_index, snapshot.entity_index);
                assert_eq!(building.render_parameter_index, 177);
                assert_eq!(building.base_tmap, 96);
                assert_eq!(building.render_mode, 0);
                assert_eq!(building.logical_height, 400);

                session.state.position = Position {
                    x: target.position.x,
                    y: target.position.y.wrapping_add(1_000),
                    z: target.position.z,
                };
                session.state.yaw = 0;
                session.state.pitch_accumulator = 0;
                session.state.effective_pitch = 0;
                session.camera_history = CameraHistory::capture(
                    &session.state,
                    session.source_turn,
                    session.camera_fov,
                    0,
                    0,
                );
                session
                    .sync_supported_renderable_records()
                    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
                let frame = session.world_frame()?;
                Ok(frame.combat_raster_stats.framebuffer_writes)
            })
            .unwrap();
        assert!(
            writes > 0,
            "the persistent remc2 parameter-177/TMAP-96 building reaches the indexed compositor"
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn requested_spell_without_cast_flag_preserves_selection_and_spawns_no_projectile() {
        let handle = open_fixture();
        let mut snapshot = McSessionSnapshotV2::default();
        let select_possession = McCommandV2 {
            flags: 0,
            requested_spell: 1,
            ..valid_command()
        };
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const select_possession,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(snapshot.selected_spell_ui_index, 0);
        assert_eq!(snapshot.selected_spell_holder, 153);
        let projectile_count =
            with_session(handle, |session| Ok(session.mission.projectiles().len())).unwrap();
        assert_eq!(projectile_count, 0);
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn ability_improved_event_encodes_the_exact_notification_contract() {
        let handle = open_fixture();
        with_session_mut(handle, |session| {
            let source_family = 16;
            let event = session.encode_mission_event(
                Level1MissionEvent::AbilityImproved {
                    source_family,
                    notification_template_selector: ABILITY_IMPROVED_TEMPLATE_SELECTOR,
                    ability_name_selector: ABILITY_IMPROVED_NAME_SELECTOR_BASE
                        + u16::from(source_family),
                    notification_state: ABILITY_IMPROVED_NOTIFICATION_STATE,
                    notification_timer: ABILITY_IMPROVED_NOTIFICATION_TIMER,
                },
                state_hash(&session.state),
            );
            assert_eq!(event.kind, MC_EVENT_LEVEL1_ABILITY_IMPROVED);
            assert_eq!(event.code, u32::from(source_family));
            assert_eq!(
                event.entity_index,
                ABILITY_IMPROVED_NAME_SELECTOR_BASE + u16::from(source_family)
            );
            assert_eq!(
                (event.value_0, event.value_1),
                (
                    i32::from(ABILITY_IMPROVED_NOTIFICATION_STATE),
                    i32::from(ABILITY_IMPROVED_NOTIFICATION_TIMER),
                )
            );
            assert_eq!(
                event.reserved,
                [u32::from(ABILITY_IMPROVED_TEMPLATE_SELECTOR), 0, 0, 0]
            );
            Ok(())
        })
        .expect("ability-improved C ABI event");
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn selected_castle_sound_event_and_sidecar_share_the_owner_asset_reference() {
        let handle = open_fixture_with_owner_cache();
        with_session_mut(handle, |session| {
            let position = session.state.position;
            let source_entity_index = session.state.entity_index;
            let random_state = session.mission.global_rng();
            // The projectile may already be gone when the end-of-turn event
            // journal is encoded (for example, an immediate blocked
            // placement). The spawn event must therefore carry its complete
            // audio source rather than looking the projectile up again.
            assert!(session.mission.projectiles().is_empty());
            let event = session.encode_mission_event(
                Level1MissionEvent::CastleProjectileSpawned {
                    projectile_id: 1,
                    sound_id: CASTLE_PROJECTILE_SOUND,
                    source_entity_index,
                    position,
                    random_state,
                },
                state_hash(&session.state),
            );
            let expected_asset_ref = session
                .world
                .sound_asset_ref(CASTLE_PROJECTILE_SOUND)
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "Castle sound has no owner asset reference",
                    )
                })?;
            assert_eq!(event.code, u32::from(CASTLE_PROJECTILE_SOUND));
            assert_eq!(expected_asset_ref, 86);
            assert_eq!(event.asset_ref_index, expected_asset_ref);
            let command = session
                .audio_commands
                .get(&event.sequence)
                .copied()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "Castle sound event has no audio sidecar",
                    )
                })?;
            assert_eq!(command.sound_id, CASTLE_PROJECTILE_SOUND);
            assert_eq!(command.asset_ref_index, expected_asset_ref);
            assert_eq!(command.event_sequence, event.sequence);
            Ok(())
        })
        .expect("selected Castle audio contract");
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn package_m_world_reset_and_copy_surfaces_are_exact() {
        let handle = open_fixture();
        assert_ne!(handle, 0);
        let mut info = McWorldInfoV2::default();
        assert_eq!(
            mc_native_world_info(handle, &raw mut info, struct_size::<McWorldInfoV2>()),
            MC_STATUS_OK
        );
        assert_eq!((info.external_level, info.internal_level), (1, 0));
        assert_eq!(info.map_type, 1);
        assert_eq!(info.profile, MC_PROFILE_OUTDOOR);
        assert_eq!(info.entity_count, 1_000);
        assert_eq!(info.active_entity_count, 177);
        assert_eq!(info.player_count, 8);
        assert_eq!(info.asset_ref_count, 76);
        assert_eq!(info.reset_state_hash, 0x9da4_bc91_af58_f3ce);
        assert_eq!(info.reset_conformance_hash, 0x77b9_6b82_492d_6f4b);

        let mut snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_snapshot(
                handle,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(snapshot.state_hash, info.reset_state_hash);
        assert_eq!(
            snapshot.conformance_hash, 0x36bd_7f1d_df05_37b5,
            "live v2 conformance includes spell tiers and the additive Level 1 effects journal"
        );
        assert_ne!(snapshot.conformance_hash, info.reset_conformance_hash);
        assert_eq!(
            (snapshot.x, snapshot.y, snapshot.z),
            (19_840, 56_960, 5_024)
        );
        assert_eq!(
            snapshot.objective_textbox_ticks, 0,
            "package M captures remc2 before PresentObjective installs its 200-tick textbox"
        );
        assert_eq!(snapshot.objective_count, 8);
        assert_eq!(snapshot.objectives_authority, MC_AUTHORITY_FROZEN_OBSERVED);

        let mut terrain_state = McTerrainStateV2::default();
        assert_eq!(
            mc_native_session_terrain_state(
                handle,
                &raw mut terrain_state,
                struct_size::<McTerrainStateV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(terrain_state.generation, snapshot.generation);
        assert_eq!(terrain_state.tick, snapshot.tick);
        assert_eq!(terrain_state.mutation_sequence, 0);
        assert_eq!(
            (
                terrain_state.width,
                terrain_state.height,
                terrain_state.plane_count,
                terrain_state.hash_schema,
                terrain_state.authority,
            ),
            (256, 256, 6, 1, MC_AUTHORITY_SESSION_TERRAIN_LIVE)
        );
        assert_eq!(terrain_state.terrain_hash, 0x1cb0_31cc_76e2_140e);

        let mut lifecycle = McLevel1CastleLifecycleSnapshotV2::default();
        assert_eq!(
            mc_native_level1_castle_lifecycle_snapshot(
                handle,
                &raw mut lifecycle,
                struct_size::<McLevel1CastleLifecycleSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(lifecycle.generation, snapshot.generation);
        assert_eq!(lifecycle.tick, snapshot.tick);
        assert_eq!(lifecycle.root_active, 0);
        assert_eq!(lifecycle.construction_present, 0);
        assert_eq!(lifecycle.terrain_mutation_sequence, 0);

        let mut count = 0;
        assert_eq!(
            mc_native_terrain_plane_length(handle, MC_TERRAIN_TYPES_U8, &raw mut count),
            MC_STATUS_OK
        );
        assert_eq!(count, 65_536);
        let mut too_small = vec![0_u8; 1];
        let mut written = 99;
        assert_eq!(
            mc_native_terrain_plane_copy_u8(
                handle,
                MC_TERRAIN_TYPES_U8,
                too_small.as_mut_ptr(),
                1,
                &raw mut written,
            ),
            MC_STATUS_BUFFER_TOO_SMALL
        );
        assert_eq!(written, 0);
        let mut terrain = vec![0_u8; 65_536];
        assert_eq!(
            mc_native_terrain_plane_copy_u8(
                handle,
                MC_TERRAIN_TYPES_U8,
                terrain.as_mut_ptr(),
                65_536,
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 65_536);
        let mut entity_indices = vec![0_i16; 65_536];
        assert_eq!(
            mc_native_terrain_entity_indices_copy(
                handle,
                entity_indices.as_mut_ptr(),
                65_536,
                &raw mut written,
            ),
            MC_STATUS_OK
        );

        let mut entities = vec![McEntitySnapshotV2::default(); 1_000];
        assert_eq!(
            mc_native_entities_copy(
                handle,
                0,
                entities.as_mut_ptr(),
                1_000,
                struct_size::<McEntitySnapshotV2>(),
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 1_000);
        assert_eq!(entities[152].authority, MC_AUTHORITY_PLAYER_FLIGHT_LIVE);
        assert_eq!(entities[153].authority, MC_AUTHORITY_SPELL_ACTIVATION_LIVE);
        for index in LEVEL_ONE_GOAT_INDICES {
            assert_eq!(
                entities[usize::from(index)].authority,
                MC_AUTHORITY_GOAT_LIVE
            );
        }
        for index in 51_usize..=78 {
            assert_eq!(entities[index].authority, MC_AUTHORITY_VILLAGER_LIVE);
        }
        assert_eq!(
            entities
                .iter()
                .filter(|entity| entity.authority == MC_AUTHORITY_FROZEN_OBSERVED)
                .count(),
            74
        );
        let fire = entities[122];
        assert_eq!(
            (
                fire.authority,
                fire.active,
                fire.class_id,
                fire.model,
                fire.action,
                fire.phase,
                fire.has_parameter_record,
                fire.parameter_record_index,
            ),
            (
                MC_AUTHORITY_LEVEL1_MISSION_LIVE,
                1,
                10,
                1,
                1,
                123,
                1,
                LEVEL1_NEW_EVENT_ENTITY_PARAMETER_RECORD_INDEX,
            )
        );
        let emitter = entities[113];
        assert_eq!(
            (
                emitter.authority,
                emitter.active,
                emitter.class_id,
                emitter.model,
                emitter.action,
                emitter.phase,
                emitter.has_parameter_record,
                emitter.parameter_record_index,
            ),
            (
                MC_AUTHORITY_LEVEL1_MISSION_LIVE,
                1,
                10,
                60,
                65,
                114,
                1,
                LEVEL1_NEW_EVENT_ENTITY_PARAMETER_RECORD_INDEX,
            )
        );
        let smoke = entities[155];
        assert_eq!(
            (
                smoke.authority,
                smoke.active,
                smoke.class_id,
                smoke.model,
                smoke.action,
                smoke.phase,
                smoke.has_parameter_record,
                smoke.parameter_record_index,
            ),
            (
                MC_AUTHORITY_LEVEL1_MISSION_LIVE,
                1,
                10,
                14,
                14,
                156,
                1,
                LEVEL1_NEW_EVENT_ENTITY_PARAMETER_RECORD_INDEX,
            )
        );
        let child = entities[164];
        assert_eq!(
            (
                child.authority,
                child.active,
                child.class_id,
                child.model,
                child.action,
                child.phase,
                child.has_parameter_record,
                child.parameter_record_index,
            ),
            (
                MC_AUTHORITY_LEVEL1_MISSION_LIVE,
                1,
                10,
                0,
                0,
                165,
                1,
                LEVEL1_NEW_EVENT_ENTITY_PARAMETER_RECORD_INDEX,
            )
        );

        with_session_mut(handle, |session| {
            session.mission.begin_source_turn_at(session.state.position);
            session.mission.dispatch_source_slot(
                &session.terrain,
                &session.state,
                113,
                false,
                false,
                None,
            );
            assert_eq!(
                session.world.entities()[178].parameter_relation,
                None,
                "newly allocated ambient slot has no frozen/static entity-parameter relation"
            );
            let dynamic_smoke = session
                .mission
                .ambient_entities()
                .find(|ambient| ambient.source_entity_index == 178)
                .copied()
                .expect("emitter 113 allocates smoke slot 178");
            assert_eq!(dynamic_smoke.render_parameter_index, 9);
            Ok(())
        })
        .expect("source-turn emitter dispatch");
        assert_eq!(
            mc_native_entities_copy(
                handle,
                0,
                entities.as_mut_ptr(),
                1_000,
                struct_size::<McEntitySnapshotV2>(),
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 1_000);
        let dynamic_smoke = entities[178];
        assert_eq!(
            (
                dynamic_smoke.authority,
                dynamic_smoke.active,
                dynamic_smoke.class_id,
                dynamic_smoke.model,
                dynamic_smoke.action,
                dynamic_smoke.has_parameter_record,
                dynamic_smoke.parameter_record_index,
            ),
            (
                MC_AUTHORITY_LEVEL1_MISSION_LIVE,
                1,
                10,
                14,
                14,
                1,
                LEVEL1_NEW_EVENT_ENTITY_PARAMETER_RECORD_INDEX,
            )
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn goat_bulk_hash_damage_lifecycle_reset_and_session_isolation_are_atomic() {
        fn session_snapshot(handle: u64) -> McSessionSnapshotV2 {
            let mut snapshot = McSessionSnapshotV2::default();
            assert_eq!(
                mc_native_session_snapshot(
                    handle,
                    &raw mut snapshot,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
            snapshot
        }

        fn goats(handle: u64, generation: u64) -> (Vec<McGoatSnapshotV2>, McGoatBatchV2) {
            let mut rows = vec![McGoatSnapshotV2::default(); LEVEL_ONE_GOAT_INDICES.len()];
            let mut batch = McGoatBatchV2::default();
            assert_eq!(
                mc_native_goats_copy(
                    handle,
                    generation,
                    0,
                    rows.as_mut_ptr(),
                    u32::try_from(rows.len()).unwrap(),
                    struct_size::<McGoatSnapshotV2>(),
                    &raw mut batch,
                    struct_size::<McGoatBatchV2>(),
                ),
                MC_STATUS_OK
            );
            assert_eq!(batch.written, 33);
            assert_eq!(batch.total_count, 33);
            assert_eq!(batch.has_more, 0);
            (rows, batch)
        }

        let first = open_fixture();
        let second = open_fixture();
        let rng_only = open_fixture();
        let first_session = session_snapshot(first);
        let second_session = session_snapshot(second);
        let rng_only_session = session_snapshot(rng_only);
        let (reset_rows, reset_batch) = goats(first, first_session.generation);
        let (second_rows, second_batch) = goats(second, second_session.generation);
        let (rng_only_reset_rows, rng_only_reset_batch) =
            goats(rng_only, rng_only_session.generation);
        assert_eq!(reset_rows, second_rows);
        assert_eq!(reset_batch.live_world_hash, second_batch.live_world_hash);
        assert_eq!(
            reset_batch.live_world_hash, 0xc5f2_8129_5e04_4de5,
            "C ABI publishes the explicit v2 reset hash"
        );
        assert_eq!(rng_only_reset_rows, reset_rows);
        assert_eq!(
            rng_only_reset_batch.live_world_hash,
            reset_batch.live_world_hash
        );
        assert_eq!(reset_batch.tick, 0);
        assert_eq!(reset_rows[0].entity_index, 50);
        assert_eq!(reset_rows[0].authority, MC_AUTHORITY_GOAT_LIVE);
        assert_eq!(reset_rows[0].render_parameter_index, 238);
        assert_eq!(reset_rows[0].base_tmap, 152);
        assert_eq!(reset_rows[0].life, 600);
        assert_eq!(reset_rows[0].max_life, 600);

        with_session_mut(rng_only, |session| {
            assert_eq!(session.mission.global_rng(), 0xad6e_67d9);
            session.mission.begin_source_turn();
            assert_eq!(session.mission.global_rng(), 0x9909_f858);
            Ok(())
        })
        .expect("production UpdateEntities turn-start RNG draw");
        let (rng_only_rows, rng_only_batch) = goats(rng_only, rng_only_session.generation);
        assert_eq!(
            rng_only_rows, rng_only_reset_rows,
            "mission RNG changes while every C ABI Goat row remains identical"
        );
        assert_eq!(rng_only_batch.tick, rng_only_reset_batch.tick);
        assert_ne!(
            rng_only_batch.live_world_hash, rng_only_reset_batch.live_world_hash,
            "C ABI live_world_hash must admit an RNG-only mutation"
        );
        let mut rng_only_restored = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_reset(
                rng_only,
                &raw mut rng_only_restored,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let (restored_rng_rows, restored_rng_batch) = goats(rng_only, rng_only_restored.generation);
        assert_eq!(
            restored_rng_rows, second_rows,
            "reset restores every row after an RNG-only mutation"
        );
        assert_eq!(restored_rng_batch.tick, second_batch.tick);
        assert_eq!(
            restored_rng_batch.live_world_hash, second_batch.live_world_hash,
            "reset restores the v2 RNG composite to an untouched session"
        );
        assert_eq!(mc_native_session_destroy(rng_only), MC_STATUS_OK);

        let mut stepped = McSessionSnapshotV2::default();
        let idle_command = valid_command();
        assert_eq!(
            mc_native_session_step(
                first,
                &raw const idle_command,
                &raw mut stepped,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let (_, advanced_batch) = goats(first, stepped.generation);
        let (_, untouched_batch) = goats(second, second_session.generation);
        assert_eq!(advanced_batch.tick, 1);
        assert_ne!(advanced_batch.live_world_hash, reset_batch.live_world_hash);
        assert_eq!(untouched_batch.live_world_hash, reset_batch.live_world_hash);

        let damage = McGoatDamageCommandV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McGoatDamageCommandV2>(),
            generation: stepped.generation,
            entity_index: 50,
            reserved_0: 0,
            damage: 601,
            reserved: [0; 4],
        };
        let mut damage_batch = McGoatBatchV2::default();
        let mut invalid_damage = damage;
        invalid_damage.reserved[0] = 1;
        assert_eq!(
            mc_native_session_goat_inject_damage(
                first,
                &raw const invalid_damage,
                &raw mut damage_batch,
                struct_size::<McGoatBatchV2>(),
            ),
            MC_STATUS_RESERVED_NONZERO
        );
        assert_eq!(
            goats(first, stepped.generation).1.live_world_hash,
            advanced_batch.live_world_hash
        );
        invalid_damage = damage;
        invalid_damage.reserved_0 = 1;
        assert_eq!(
            mc_native_session_goat_inject_damage(
                first,
                &raw const invalid_damage,
                &raw mut damage_batch,
                struct_size::<McGoatBatchV2>(),
            ),
            MC_STATUS_RESERVED_NONZERO
        );
        assert_eq!(
            goats(first, stepped.generation).1.live_world_hash,
            advanced_batch.live_world_hash
        );
        invalid_damage = damage;
        invalid_damage.damage = 0;
        assert_eq!(
            mc_native_session_goat_inject_damage(
                first,
                &raw const invalid_damage,
                &raw mut damage_batch,
                struct_size::<McGoatBatchV2>(),
            ),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            goats(first, stepped.generation).1.live_world_hash,
            advanced_batch.live_world_hash
        );
        invalid_damage = damage;
        invalid_damage.entity_index = 999;
        assert_eq!(
            mc_native_session_goat_inject_damage(
                first,
                &raw const invalid_damage,
                &raw mut damage_batch,
                struct_size::<McGoatBatchV2>(),
            ),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            goats(first, stepped.generation).1.live_world_hash,
            advanced_batch.live_world_hash
        );
        invalid_damage = damage;
        invalid_damage.generation = invalid_damage.generation.wrapping_add(1);
        assert_eq!(
            mc_native_session_goat_inject_damage(
                first,
                &raw const invalid_damage,
                &raw mut damage_batch,
                struct_size::<McGoatBatchV2>(),
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(
            goats(first, stepped.generation).1.live_world_hash,
            advanced_batch.live_world_hash
        );
        assert_eq!(
            mc_native_session_goat_inject_damage(
                first,
                &raw const damage,
                &raw mut damage_batch,
                struct_size::<McGoatBatchV2>(),
            ),
            MC_STATUS_OK
        );
        assert_ne!(damage_batch.live_world_hash, advanced_batch.live_world_hash);

        for _ in 0..12 {
            assert_eq!(
                mc_native_session_step(
                    first,
                    &raw const idle_command,
                    &raw mut stepped,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
            let (rows, _) = goats(first, stepped.generation);
            if rows[0].active == 0 {
                break;
            }
            if rows[0].action == 12 {
                let kill_credit = with_session(first, |session| {
                    Ok(session.live_world.goats()[0].kill_credit_entity)
                })
                .unwrap();
                assert_eq!(
                    kill_credit,
                    i16::try_from(stepped.entity_index).unwrap(),
                    "qualification damage is always attributed to the local player"
                );
            }
        }
        let (dead_rows, _) = goats(first, stepped.generation);
        assert!(
            dead_rows[0].active == 0,
            "Goat must reach next-pass reclamation"
        );

        let query = McEventQueryV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McEventQueryV2>(),
            generation: stepped.generation,
            cursor: 0,
            reserved: [0; 4],
        };
        let mut events = vec![McEventV2::default(); 256];
        let mut event_batch = McEventBatchV2::default();
        assert_eq!(
            mc_native_events_copy_since(
                first,
                &raw const query,
                events.as_mut_ptr(),
                u32::try_from(events.len()).unwrap(),
                struct_size::<McEventV2>(),
                &raw mut event_batch,
                struct_size::<McEventBatchV2>(),
            ),
            MC_STATUS_OK
        );
        events.truncate(usize::try_from(event_batch.written).unwrap());
        assert!(events.iter().any(|event| {
            event.kind == MC_EVENT_GOAT_MANA_CONVERTED
                && event.entity_index == 50
                && event.value_0 == 300
        }));
        assert!(events.iter().any(|event| {
            event.kind == MC_EVENT_GOAT_REMOVAL_QUEUED && event.entity_index == 50
        }));
        assert!(
            events
                .iter()
                .any(|event| { event.kind == MC_EVENT_GOAT_RECLAIMED && event.entity_index == 50 })
        );

        let mut reset_snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_reset(
                first,
                &raw mut reset_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let mut rejected_batch = McGoatBatchV2::default();
        assert_eq!(
            mc_native_goats_copy(
                first,
                stepped.generation,
                0,
                std::ptr::null_mut(),
                0,
                struct_size::<McGoatSnapshotV2>(),
                &raw mut rejected_batch,
                struct_size::<McGoatBatchV2>(),
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        let (restored_rows, restored_batch) = goats(first, reset_snapshot.generation);
        assert_eq!(restored_rows, reset_rows);
        assert_eq!(restored_batch.live_world_hash, reset_batch.live_world_hash);
        assert_eq!(restored_batch.tick, 0);

        assert_eq!(mc_native_session_destroy(first), MC_STATUS_OK);
        assert_eq!(mc_native_session_destroy(second), MC_STATUS_OK);
    }

    #[test]
    fn selector_frames_latch_advance_freeze_reset_and_isolate_exactly() {
        fn snapshot(handle: u64) -> McSessionSnapshotV2 {
            let mut snapshot = McSessionSnapshotV2::default();
            assert_eq!(
                mc_native_session_snapshot(
                    handle,
                    &raw mut snapshot,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
            snapshot
        }

        fn frames(
            handle: u64,
            generation: u64,
            tick: u64,
        ) -> ([McSelectorFrameStateV2; 3], McSelectorFrameBatchV2) {
            let mut rows = [McSelectorFrameStateV2::default(); 3];
            let mut batch = McSelectorFrameBatchV2::default();
            assert_eq!(
                mc_native_selector_frame_states_copy(
                    handle,
                    generation,
                    tick,
                    0,
                    rows.as_mut_ptr(),
                    3,
                    struct_size::<McSelectorFrameStateV2>(),
                    &raw mut batch,
                    struct_size::<McSelectorFrameBatchV2>(),
                ),
                MC_STATUS_OK
            );
            (rows, batch)
        }

        fn submit(
            handle: u64,
            generation: u64,
            tick: u64,
            mask: u64,
        ) -> (i32, McSelectorFrameBatchV2) {
            let command = McSelectorDrawFeedbackV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McSelectorDrawFeedbackV2>(),
                generation,
                draw_tick: tick,
                selector_mask: mask,
                reserved: [0; 4],
            };
            let mut batch = McSelectorFrameBatchV2::default();
            let status = mc_native_session_selector_draw_feedback_submit(
                handle,
                &raw const command,
                &raw mut batch,
                struct_size::<McSelectorFrameBatchV2>(),
            );
            (status, batch)
        }

        let first = open_fixture_with_owner_cache();
        let second = open_fixture_with_owner_cache();
        let first_snapshot = snapshot(first);
        let second_snapshot = snapshot(second);
        let (reset_rows, reset_batch) =
            frames(first, first_snapshot.generation, first_snapshot.tick);
        let (second_rows, second_batch) =
            frames(second, second_snapshot.generation, second_snapshot.tick);
        assert_eq!(reset_rows, second_rows);
        assert_eq!(
            reset_batch.selector_state_hash,
            second_batch.selector_state_hash
        );
        assert_eq!(reset_rows.map(|row| row.selector), GOAT_TMAP_SELECTORS);
        assert_eq!(reset_rows.map(|row| row.source_frame_index), [1, 1, 1]);
        assert_eq!(reset_rows.map(|row| row.stream_cursor), [490, 710, 490]);
        assert_eq!(reset_rows.map(|row| row.pixel_bytes), [484, 704, 484]);
        assert!(
            reset_rows
                .iter()
                .all(|row| row.flags & MC_SELECTOR_FRAME_USED_BY_PRECEDING_DRAW == 0)
        );
        assert_eq!(reset_batch.valid_draw_mask, 7);
        assert_eq!(reset_batch.has_draw_feedback, 0);

        let (status, submitted) = submit(first, first_snapshot.generation, 0, 1);
        assert_eq!(status, MC_STATUS_OK);
        assert_eq!(submitted.last_draw_tick, 0);
        assert_eq!(submitted.last_draw_mask, 1);
        assert_eq!(submitted.has_draw_feedback, 1);
        let (latched_rows, latched_batch) = frames(first, first_snapshot.generation, 0);
        assert_ne!(
            latched_batch.selector_state_hash,
            reset_batch.selector_state_hash
        );
        assert_ne!(
            latched_rows[0].flags & MC_SELECTOR_FRAME_USED_BY_PRECEDING_DRAW,
            0
        );
        assert_eq!(
            latched_rows[1].flags & MC_SELECTOR_FRAME_USED_BY_PRECEDING_DRAW,
            0
        );

        let (status, duplicate) = submit(first, first_snapshot.generation, 0, 1);
        assert_eq!(status, MC_STATUS_OK);
        assert_eq!(
            duplicate.selector_state_hash,
            latched_batch.selector_state_hash
        );
        let (status, combined) = submit(first, first_snapshot.generation, 0, 2);
        assert_eq!(status, MC_STATUS_OK);
        assert_eq!(combined.last_draw_mask, 3);
        let (combined_rows, combined_batch) = frames(first, first_snapshot.generation, 0);
        assert_ne!(
            combined_rows[1].flags & MC_SELECTOR_FRAME_USED_BY_PRECEDING_DRAW,
            0
        );
        let (status, _) = submit(first, first_snapshot.generation, 0, 8);
        assert_eq!(status, MC_STATUS_INVALID_ARGUMENT);
        assert_eq!(
            frames(first, first_snapshot.generation, 0)
                .1
                .selector_state_hash,
            combined_batch.selector_state_hash
        );

        let mut stepped = McSessionSnapshotV2::default();
        let idle = valid_command();
        assert_eq!(
            mc_native_session_step(
                first,
                &raw const idle,
                &raw mut stepped,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let (advanced_rows, advanced_batch) = frames(first, stepped.generation, stepped.tick);
        assert_eq!(advanced_rows.map(|row| row.source_frame_index), [2, 2, 1]);
        assert_eq!(advanced_rows.map(|row| row.stream_cursor), [818, 1196, 490]);
        assert!(
            advanced_rows
                .iter()
                .all(|row| row.flags & MC_SELECTOR_FRAME_USED_BY_PRECEDING_DRAW == 0)
        );
        assert_ne!(
            advanced_batch.selector_state_hash,
            latched_batch.selector_state_hash
        );
        assert_eq!(
            submit(first, stepped.generation, stepped.tick - 1, 4).0,
            MC_STATUS_GENERATION_MISMATCH,
            "feedback binds the completed session/Goat tick, not the lagging camera-history tick"
        );
        assert_eq!(
            frames(first, stepped.generation, stepped.tick)
                .1
                .selector_state_hash,
            advanced_batch.selector_state_hash
        );
        assert_eq!(
            frames(second, second_snapshot.generation, second_snapshot.tick).0,
            reset_rows
        );

        assert_eq!(
            mc_native_session_step(
                first,
                &raw const idle,
                &raw mut stepped,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let (frozen_rows, _) = frames(first, stepped.generation, stepped.tick);
        assert_eq!(
            frozen_rows.map(|row| row.source_frame_index),
            [2, 2, 1],
            "no feedback freezes every selector"
        );

        let (status, zero) = submit(first, stepped.generation, stepped.tick, 0);
        assert_eq!(status, MC_STATUS_OK);
        assert_eq!(zero.last_draw_mask, 0);
        let (status, accumulated) = submit(first, stepped.generation, stepped.tick, 4);
        assert_eq!(status, MC_STATUS_OK);
        assert_eq!(accumulated.last_draw_mask, 4);
        assert_eq!(
            mc_native_session_step(
                first,
                &raw const idle,
                &raw mut stepped,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            frames(first, stepped.generation, stepped.tick)
                .0
                .map(|row| row.source_frame_index),
            [2, 2, 2],
            "zero then a later same-tick draw accumulates the new selector"
        );

        let (status, zero_only) = submit(first, stepped.generation, stepped.tick, 0);
        assert_eq!(status, MC_STATUS_OK);
        assert_eq!(zero_only.last_draw_mask, 0);
        assert_eq!(
            mc_native_session_step(
                first,
                &raw const idle,
                &raw mut stepped,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            frames(first, stepped.generation, stepped.tick)
                .0
                .map(|row| row.source_frame_index),
            [2, 2, 2],
            "an explicit zero-only feedback epoch is frozen"
        );

        let stale_generation = stepped.generation;
        let mut reset_snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_reset(
                first,
                &raw mut reset_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_ne!(reset_snapshot.generation, stale_generation);
        let (restored_rows, restored_batch) =
            frames(first, reset_snapshot.generation, reset_snapshot.tick);
        assert_eq!(restored_rows, reset_rows);
        assert_eq!(
            restored_batch.selector_state_hash,
            reset_batch.selector_state_hash
        );
        assert_eq!(
            submit(first, stale_generation, stepped.tick, 1).0,
            MC_STATUS_GENERATION_MISMATCH
        );

        assert_eq!(
            submit(first, reset_snapshot.generation, reset_snapshot.tick, 1).0,
            MC_STATUS_OK
        );
        assert_eq!(
            submit(first, reset_snapshot.generation, reset_snapshot.tick, 2).0,
            MC_STATUS_OK
        );
        let mut replay_snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                first,
                &raw const idle,
                &raw mut replay_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let (replay_rows, replay_batch) =
            frames(first, replay_snapshot.generation, replay_snapshot.tick);
        assert_eq!(replay_rows, advanced_rows);
        assert_eq!(
            replay_batch.selector_state_hash, advanced_batch.selector_state_hash,
            "reset plus identical feedback/step replay restores the full selector state"
        );
        assert_eq!(mc_native_session_destroy(first), MC_STATUS_OK);
        assert_eq!(mc_native_session_destroy(second), MC_STATUS_OK);
    }

    #[test]
    fn native_level_one_palette_is_atomic_hashed_and_owner_cache_backed() {
        let handle = open_fixture_with_world_frame_assets();
        let mut rgb8 = [0_u8; 768];
        let mut info = McLevel1PaletteInfoV2::default();
        assert_eq!(
            mc_native_level1_palette_copy(
                handle,
                rgb8.as_mut_ptr(),
                rgb8.len() as u32,
                &raw mut info,
                struct_size::<McLevel1PaletteInfoV2>(),
            ),
            MC_STATUS_OK
        );
        let expected = fs::read(presentation_test_cache().join("palette/paln.rgb8")).unwrap();
        assert_eq!(rgb8.as_slice(), expected);
        assert_eq!(info.rgb8_sha256.as_slice(), Sha256::digest(rgb8).as_slice());
        assert_eq!(
            (
                info.submode,
                info.effect,
                info.weight,
                info.denominator,
                info.palette_count,
                info.flags,
            ),
            (0, 0, 0, 0, 0, 1)
        );
        assert_eq!(info.reserved, [0; 4]);
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn qualified_cpu_goat_draw_admission_is_authoritative_idempotent_and_tick_bound() {
        let handle = open_fixture_with_owner_cache();
        let mut reset = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_snapshot(
                handle,
                &raw mut reset,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let mut receipt = McSelectorFrameBatchV2::default();
        assert_eq!(
            mc_native_session_goat_draw_admission_evaluate(
                handle,
                reset.generation,
                reset.tick,
                &raw mut receipt,
                struct_size::<McSelectorFrameBatchV2>(),
            ),
            MC_STATUS_OK
        );
        let exact_mask = with_session(handle, |session| {
            Ok(session
                .goat_draw_admission
                .as_ref()
                .expect("evaluation stores diagnostic rows")
                .selector_mask)
        })
        .unwrap();
        assert_eq!(receipt.has_draw_feedback, 1);
        assert_eq!(receipt.last_draw_tick, reset.tick);
        assert_eq!(receipt.last_draw_mask, u64::from(exact_mask));
        assert_eq!(receipt.valid_draw_mask, u64::from(GOAT_TMAP_DRAW_MASK));

        let first_hash = receipt.selector_state_hash;
        assert_eq!(
            mc_native_session_goat_draw_admission_evaluate(
                handle,
                reset.generation,
                reset.tick,
                &raw mut receipt,
                struct_size::<McSelectorFrameBatchV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(receipt.selector_state_hash, first_hash);
        assert_eq!(receipt.last_draw_mask, u64::from(exact_mask));

        let idle = valid_command();
        let mut stepped = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const idle,
                &raw mut stepped,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let mut rows = [McSelectorFrameStateV2::default(); 3];
        assert_eq!(
            mc_native_selector_frame_states_copy(
                handle,
                stepped.generation,
                stepped.tick,
                0,
                rows.as_mut_ptr(),
                3,
                struct_size::<McSelectorFrameStateV2>(),
                &raw mut receipt,
                struct_size::<McSelectorFrameBatchV2>(),
            ),
            MC_STATUS_OK
        );
        for (ordinal, row) in rows.iter().enumerate() {
            let was_admitted = exact_mask & (1 << ordinal) != 0;
            assert_eq!(row.source_frame_index, if was_admitted { 2 } else { 1 });
            assert_eq!(
                row.flags & MC_SELECTOR_FRAME_USED_BY_PRECEDING_DRAW,
                0,
                "the preceding CPU draw is serviced exactly once"
            );
        }
        assert_eq!(
            mc_native_session_goat_draw_admission_evaluate(
                handle,
                stepped.generation,
                stepped.tick - 1,
                &raw mut receipt,
                struct_size::<McSelectorFrameBatchV2>(),
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);

        let ownerless = open_fixture();
        let mut ownerless_snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_snapshot(
                ownerless,
                &raw mut ownerless_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_session_goat_draw_admission_evaluate(
                ownerless,
                ownerless_snapshot.generation,
                ownerless_snapshot.tick,
                &raw mut receipt,
                struct_size::<McSelectorFrameBatchV2>(),
            ),
            MC_STATUS_VALIDATION_FAILED
        );
        assert_eq!(mc_native_session_destroy(ownerless), MC_STATUS_OK);
    }

    #[test]
    fn objective_catalog_is_complete_frozen_and_bounds_checked() {
        let handle = open_fixture();
        let mut count = 0;
        assert_eq!(
            mc_native_objective_count(handle, &raw mut count),
            MC_STATUS_OK
        );
        assert_eq!(count, 8);

        let mut objectives = [McObjectiveSnapshotV2::default(); 8];
        let mut written = 0;
        assert_eq!(
            mc_native_objectives_copy(
                handle,
                0,
                objectives.as_mut_ptr(),
                8,
                struct_size::<McObjectiveSnapshotV2>(),
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 8);
        for (slot, objective) in objectives.iter().enumerate() {
            assert_eq!(usize::from(objective.slot), slot);
            assert_eq!(objective.authority, MC_AUTHORITY_FROZEN_OBSERVED);
            assert_eq!(objective.abi_version, MC_NATIVE_ABI_VERSION);
            assert_eq!(
                objective.struct_size,
                struct_size::<McObjectiveSnapshotV2>()
            );
        }
        assert_eq!(
            (
                objectives[0].is_level_end,
                objectives[0].text_selector,
                objectives[0].done,
            ),
            (0, 0, 0)
        );
        assert_eq!(objectives[0].stage_selectors, [1, 1, 1, 1, 1, 0, 0, 0]);

        assert_eq!(
            mc_native_objectives_copy(
                handle,
                0,
                std::ptr::null_mut(),
                0,
                struct_size::<McObjectiveSnapshotV2>() - 1,
                &raw mut written,
            ),
            MC_STATUS_STRUCT_TOO_SMALL
        );
        assert_eq!(
            mc_native_objectives_copy(
                handle,
                0,
                std::ptr::null_mut(),
                0,
                struct_size::<McObjectiveSnapshotV2>() + 1,
                &raw mut written,
            ),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            mc_native_objectives_copy(
                handle,
                9,
                std::ptr::null_mut(),
                0,
                struct_size::<McObjectiveSnapshotV2>(),
                &raw mut written,
            ),
            MC_STATUS_BOUNDS
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn two_sessions_are_independent_and_repeat_deterministically() {
        let first = open_fixture();
        let second = open_fixture();
        assert_ne!(first, second);
        let mut first_terrain = McTerrainStateV2::default();
        let mut second_terrain = McTerrainStateV2::default();
        assert_eq!(
            mc_native_session_terrain_state(
                first,
                &raw mut first_terrain,
                struct_size::<McTerrainStateV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_session_terrain_state(
                second,
                &raw mut second_terrain,
                struct_size::<McTerrainStateV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(first_terrain.terrain_hash, second_terrain.terrain_hash);
        assert_eq!(first_terrain.mutation_sequence, 0);
        assert_eq!(second_terrain.mutation_sequence, 0);
        let mut command = valid_command();
        command.flags = 1;
        let mut first_snapshot = McSessionSnapshotV2::default();
        let mut second_snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                first,
                &raw const command,
                &raw mut first_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_session_snapshot(
                second,
                &raw mut second_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_ne!(first_snapshot.state_hash, second_snapshot.state_hash);
        let mut player_entity = McEntitySnapshotV2::default();
        let mut written = 0;
        assert_eq!(
            mc_native_entities_copy(
                first,
                u32::from(first_snapshot.entity_index),
                &raw mut player_entity,
                1,
                struct_size::<McEntitySnapshotV2>(),
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 1);
        assert_eq!(
            (player_entity.x, player_entity.y, player_entity.z),
            (first_snapshot.x, first_snapshot.y, first_snapshot.z)
        );
        assert_eq!(player_entity.yaw, first_snapshot.yaw);
        assert_eq!(player_entity.pitch, first_snapshot.display_pitch);
        assert_eq!(player_entity.actual_speed, first_snapshot.actual_speed);
        assert_eq!(player_entity.phase, first_snapshot.phase);
        let mut reset = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_reset(first, &raw mut reset, struct_size::<McSessionSnapshotV2>(),),
            MC_STATUS_OK
        );
        assert_eq!(reset.state_hash, second_snapshot.state_hash);
        assert_ne!(reset.generation, second_snapshot.generation);
        let mut reset_terrain = McTerrainStateV2::default();
        assert_eq!(
            mc_native_session_terrain_state(
                first,
                &raw mut reset_terrain,
                struct_size::<McTerrainStateV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(reset_terrain.generation, reset.generation);
        assert_eq!(reset_terrain.mutation_sequence, 0);
        assert_eq!(reset_terrain.terrain_hash, second_terrain.terrain_hash);
        assert_eq!(mc_native_session_destroy(first), MC_STATUS_OK);
        assert_eq!(mc_native_session_destroy(second), MC_STATUS_OK);
    }

    #[test]
    fn source_projection_hash_has_complete_preimage_without_action_51_terrain_and_normalizes_tick()
    {
        let handle = open_fixture();
        let command = valid_command();
        let mut snapshot = McSessionSnapshotV2::default();
        for _ in 0..300 {
            assert_eq!(
                mc_native_session_step(
                    handle,
                    &raw const command,
                    &raw mut snapshot,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
        }
        let mut projected_hash = 0_u64;
        assert_eq!(
            mc_native_session_source_projection_hash(handle, &raw mut projected_hash),
            MC_STATUS_OK
        );
        with_session(handle, |session| {
            assert_eq!(
                session.state,
                FlightState {
                    entity_index: 152,
                    position: Position {
                        x: 19_840,
                        y: 56_960,
                        z: 5_024,
                    },
                    yaw: 0,
                    display_pitch: 0,
                    effective_pitch: 0,
                    roll_accumulator: 0,
                    pitch_accumulator: 0,
                    target_speed: 0,
                    speed_changed_flag: 0,
                    actual_speed: 0,
                    strafe_speed: 0,
                    roll_delta: 0,
                    pitch_delta: 0,
                    recoil_speed: 0,
                    recoil_yaw: 0,
                    impulse_x: 0,
                    impulse_y: 0,
                    impulse_z: 0,
                    entity_fov: 0,
                    water_counter: 0,
                    speed_modifier: 0,
                    speed_modifier_counter: 0,
                    immobilize_counter: 0,
                    immobilize_subcounter: 0,
                    tether_prior_distance: 0,
                    tether_target_index: 0,
                    tether_holder_index: 0,
                    tether_tier: 0,
                    speed_spell_holder_index: 0,
                    entity_phase: 197,
                    ambient_loop_5_distance: 2_048,
                    ambient_loop_31_distance: 1_746,
                    player_color_index: 0,
                    music_countdown: 0,
                    recovery_latch: false,
                    skip_flight_once: false,
                    tick: 300,
                },
                "all 39 fixed-width state-hash fields have an explicit preimage"
            );
            assert_eq!(
                (
                    session.terrain.deterministic_hash(),
                    session.terrain.mutation_sequence(),
                ),
                (0x5acb_822a_a12a_962f, 2),
                "the paired terrain generation is independently identified"
            );
            assert!(session.mission.house_construction_effects().is_empty());
            assert!(session.pending_house_effects.iter().all(|effect| !matches!(
                effect,
                HouseStepEvent::ConstructionPass { .. }
                    | HouseStepEvent::ConstructionCompleted { .. }
            )));
            Ok(())
        })
        .expect("complete 300-turn hash preimage");
        assert_eq!(
            (snapshot.state_hash, projected_hash),
            (0x5fec_9398_db9c_e681, 0x372c_5311_579c_f80a)
        );
        with_session(handle, |session| {
            let shifted = FlightState {
                tick: session.state.tick.wrapping_add(10_000),
                ..session.state
            };
            assert_ne!(state_hash(&shifted), snapshot.state_hash);
            assert_eq!(source_projection_hash(&shifted), projected_hash);
            Ok(())
        })
        .expect("source-projection tick normalization");
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn source_arena_hash_replays_resets_and_isolates_sessions() {
        let first = open_fixture();
        let second = open_fixture();
        let mut first_initial = 0_u64;
        let mut second_initial = 0_u64;
        assert_eq!(
            mc_native_session_source_arena_hash(first, &raw mut first_initial),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_session_source_arena_hash(second, &raw mut second_initial),
            MC_STATUS_OK
        );
        assert_eq!(first_initial, second_initial);
        assert_ne!(first_initial, 0);

        let command = valid_command();
        let mut snapshot = McSessionSnapshotV2::default();
        for _ in 0..8 {
            assert_eq!(
                mc_native_session_step(
                    first,
                    &raw const command,
                    &raw mut snapshot,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
            assert_eq!(
                mc_native_session_step(
                    second,
                    &raw const command,
                    &raw mut snapshot,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
        }
        let mut first_replay = 0_u64;
        let mut second_replay = 0_u64;
        assert_eq!(
            mc_native_session_source_arena_hash(first, &raw mut first_replay),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_session_source_arena_hash(second, &raw mut second_replay),
            MC_STATUS_OK
        );
        assert_eq!(first_replay, second_replay);
        assert_ne!(first_replay, first_initial);

        assert_eq!(
            mc_native_session_step(
                first,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let mut first_diverged = 0_u64;
        assert_eq!(
            mc_native_session_source_arena_hash(first, &raw mut first_diverged),
            MC_STATUS_OK
        );
        assert_ne!(first_diverged, second_replay);

        assert_eq!(
            mc_native_session_reset(
                first,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let mut first_reset = 0_u64;
        assert_eq!(
            mc_native_session_source_arena_hash(first, &raw mut first_reset),
            MC_STATUS_OK
        );
        assert_eq!(first_reset, first_initial);
        assert_eq!(mc_native_session_destroy(first), MC_STATUS_OK);
        assert_eq!(mc_native_session_destroy(second), MC_STATUS_OK);
    }

    #[test]
    fn direct_core_and_capi_source_dispatch_remain_in_lockstep() {
        const DISTANCE_WINNER_C_PUBLIC_AIM: (i16, i16) = (0, 0);
        const NUMERIC_FIRST_A_PUBLIC_AIM: (i16, i16) = (16, 0);

        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("package M loads");
        let mut direct = world.session();
        let handle = open_fixture();
        let command = valid_command();
        let mut snapshot = McSessionSnapshotV2::default();

        // The native host supplies one complete typed turn-start snapshot to
        // every source dispatch. Make all relevant orders disagree:
        // numeric A/B/C/D, reversed host D/C/B/A, model-major B/A/C/D, nearest
        // C, plus a still-nearer same-turn insertion E that must remain absent
        // from the frozen guidance lists until the next rebuild.
        let exercise_targeting_vector =
            |mission: &mut Level1MissionState,
             state: &mut FlightState,
             terrain: &SessionTerrain,
             live_world: &LiveWorldState| {
                state.position = Position {
                    x: 40_000,
                    y: 40_000,
                    z: 2_000,
                };
                state.yaw = 0;
                state.effective_pitch = 0;
                state.actual_speed = 0;
                mission.has_fireball = true;
                mission.player_mana = 10_000;
                let origin = state.position;
                let spawn_resident = |mission: &mut Level1MissionState,
                                      model,
                                      action,
                                      x_offset: u16,
                                      y_distance: u16,
                                      z_offset: i16| {
                    mission
                        .spawn_house_resident(
                            Position {
                                x: origin.x.wrapping_add(x_offset),
                                y: origin.y.wrapping_sub(y_distance),
                                z: origin.z.wrapping_add(z_offset),
                            },
                            model,
                            action,
                            0,
                        )
                        .expect("qualified resident family")
                        .expect("resident source slot")
                };
                let numeric_first = spawn_resident(mission, 12, 97, 64, 1_000, 100);
                let model_first = spawn_resident(mission, 4, 33, 128, 1_000, 164);
                let distance_first = spawn_resident(mission, 12, 97, 0, 512, 100);
                let host_first = spawn_resident(mission, 13, 105, 256, 1_000, 36);
                assert!(
                    numeric_first < model_first
                        && model_first < distance_first
                        && distance_first < host_first
                );

                mission.begin_source_turn_at(origin);
                let mut turn_start = level1_turn_start_source_views(mission, state, live_world);
                turn_start.reverse();
                mission.dispatch_source_slot_with_views_and_look(
                    terrain,
                    state,
                    &turn_start,
                    0,
                    false,
                    false,
                    None,
                    0,
                    0,
                );

                let same_turn_insertion = spawn_resident(mission, 4, 33, 100, 283, 100);
                let mut later_views = level1_turn_start_source_views(mission, state, live_world);
                later_views.reverse();
                assert!(mission.spawn_fireball_with_look(state, 0, 0));
                let projectile_offset = mission.projectiles().len() - 1;
                let projectile_slot =
                    mission.projectiles()[projectile_offset].source_entity_index();
                mission.dispatch_source_slot_with_views_and_look(
                    terrain,
                    state,
                    &later_views,
                    projectile_slot,
                    false,
                    false,
                    None,
                    0,
                    0,
                );
                let Level1Projectile::PlayerFireball(projectile) =
                    mission.projectiles()[projectile_offset]
                else {
                    panic!("targeting vector spawns a player Fireball");
                };
                assert_eq!(
                    projectile.target_entity_index, distance_first,
                    "distance winner C replaces model-list head B, while same-turn insertion E remains frozen out"
                );
                (
                    [
                        numeric_first,
                        model_first,
                        distance_first,
                        host_first,
                        same_turn_insertion,
                    ],
                    projectile_offset,
                    projectile.source_entity_index,
                    projectile.target_entity_index,
                    projectile.yaw,
                    projectile.pitch,
                )
            };
        let mut direct_vector = world.session();
        let vector_handle = open_fixture();
        let direct_outcome = exercise_targeting_vector(
            &mut direct_vector.mission,
            &mut direct_vector.state,
            &direct_vector.terrain,
            &direct_vector.live_world,
        );
        let capi_outcome = with_session_mut(vector_handle, |session| {
            Ok(exercise_targeting_vector(
                &mut session.mission,
                &mut session.state,
                &session.terrain,
                &session.live_world,
            ))
        })
        .expect("registered C API targeting-vector session");
        assert_eq!(direct_outcome, capi_outcome);
        assert_eq!(
            direct_outcome.3, direct_outcome.0[2],
            "direct core target identity proves that distance winner C defeated numeric A, model-list B, and reversed-host D"
        );
        let capi_vector_snapshot = with_session(vector_handle, |session| Ok(session.snapshot()))
            .expect("registered C API targeting-vector snapshot");
        let mut visible_projectile = McFireballProjectileSnapshotV2::default();
        let mut vector_written = 0;
        assert_eq!(
            mc_native_fireball_projectiles_copy(
                vector_handle,
                capi_vector_snapshot.generation,
                capi_vector_snapshot.tick,
                u32::try_from(capi_outcome.1).expect("projectile offset fits u32"),
                &raw mut visible_projectile,
                1,
                struct_size::<McFireballProjectileSnapshotV2>(),
                &raw mut vector_written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(vector_written, 1);
        assert_eq!(
            (visible_projectile.yaw, visible_projectile.pitch),
            DISTANCE_WINNER_C_PUBLIC_AIM,
            "C API projection independently exposes C's source-derived straight-ahead yaw/pitch"
        );
        assert_ne!(
            (visible_projectile.yaw, visible_projectile.pitch),
            NUMERIC_FIRST_A_PUBLIC_AIM,
            "candidate A's distinct in-cone yaw would hit the source first-turn +16 yaw limit"
        );
        assert_eq!(
            visible_projectile.reserved[0],
            u32::from(capi_outcome.2),
            "reserved[0] is the projectile source slot, not the selected target"
        );
        assert_eq!(mc_native_session_destroy(vector_handle), MC_STATUS_OK);

        let reset_villagers = direct.live_world.villagers().to_vec();
        assert_eq!(
            reset_villagers
                .iter()
                .map(|villager| villager.entity_index)
                .collect::<Vec<_>>(),
            (51_u16..=78).collect::<Vec<_>>()
        );

        for turn in 1..=24 {
            direct.step(PlayerCommand::default());
            assert_eq!(
                mc_native_session_step(
                    handle,
                    &raw const command,
                    &raw mut snapshot,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
            with_session(handle, |capi| {
                assert_eq!(
                    state_hash(&direct.state),
                    state_hash(&capi.state),
                    "flight state diverged on turn {turn}"
                );
                assert_eq!(
                    direct.live_world_hash_v2(),
                    capi.live_world_hash_v2(),
                    "typed imported world diverged on turn {turn}"
                );
                assert_eq!(
                    direct.mission.source_arena_hash_v1(),
                    capi.mission.source_arena_hash_v1(),
                    "source arena diverged on turn {turn}"
                );
                Ok(())
            })
            .expect("registered C API session");
        }
        assert!(
            direct
                .live_world
                .villagers()
                .iter()
                .zip(&reset_villagers)
                .all(|(live, reset)| {
                    live.active
                        && live.phase == reset.phase.wrapping_add(24)
                        && live.position != reset.position
                        && direct.mission.source_slots()[usize::from(live.entity_index)].action
                            == live.action
                }),
            "all 28 frame-zero Villagers execute live action-111 bodies"
        );
        with_session(handle, |capi| {
            assert_eq!(
                direct.live_world.villagers(),
                capi.live_world.villagers(),
                "direct and C API Villager bodies diverged"
            );
            for villager in capi.live_world.villagers() {
                let projection = capi
                    .frozen_actors
                    .iter()
                    .find(|actor| actor.source_entity_index == villager.entity_index)
                    .expect("live Villager presentation projection");
                assert_eq!(
                    (projection.position, projection.yaw),
                    (villager.position, villager.yaw),
                    "Villager presentation must follow live simulation"
                );
                assert!(capi.renderable_map_chains.tracks(villager.entity_index));
            }
            Ok(())
        })
        .expect("registered C API session");

        let lethal_damage = direct
            .live_world
            .villagers()
            .iter()
            .find(|villager| villager.entity_index == 51)
            .unwrap()
            .life
            + 1;
        direct
            .live_world
            .inject_damage(51, lethal_damage, 152)
            .expect("direct Villager lethal mailbox");
        with_session_mut(handle, |capi| {
            capi.live_world
                .inject_damage(51, lethal_damage, 152)
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))
        })
        .expect("C API Villager lethal mailbox");
        let mut removed_turn = None;
        for death_turn in 1..=10 {
            direct.step(PlayerCommand::default());
            assert_eq!(
                mc_native_session_step(
                    handle,
                    &raw const command,
                    &raw mut snapshot,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
            with_session(handle, |capi| {
                assert_eq!(direct.live_world.villagers(), capi.live_world.villagers());
                assert_eq!(
                    direct.mission.source_arena_hash_v1(),
                    capi.mission.source_arena_hash_v1()
                );
                Ok(())
            })
            .expect("registered C API session");
            let villager = direct
                .live_world
                .villagers()
                .iter()
                .find(|villager| villager.entity_index == 51)
                .unwrap();
            if death_turn == 1 {
                assert_eq!(villager.action, 108);
                assert_eq!(villager.pending_damage, lethal_damage);
                assert_eq!(villager.pending_attacker, 0);
                assert_eq!(villager.kill_credit_entity, 152);
            }
            if !villager.active {
                removed_turn = Some(death_turn);
                break;
            }
        }
        assert_eq!(
            removed_turn,
            Some(2),
            "frame-zero reset_counter=2 takes action 108's immediate-disable branch"
        );
        assert!(
            direct
                .mission
                .death_bursts()
                .iter()
                .all(|burst| burst.parent_entity_index != 51),
            "the immediate-disable branch does not enter action 109"
        );
        assert!(direct.mission.imported_source_slot_is_live(51));
        with_session(handle, |capi| {
            let dead = capi
                .live_world
                .villagers()
                .iter()
                .find(|villager| villager.entity_index == 51)
                .unwrap();
            assert!(!dead.active);
            assert!(
                !capi
                    .renderable_map_chains
                    .entity_chain(
                        dead.position.x.to_be_bytes()[0],
                        dead.position.y.to_be_bytes()[0]
                    )
                    .contains(&51),
                "death unlinks the live Villager presentation immediately"
            );
            Ok(())
        })
        .expect("registered C API session");
        direct.step(PlayerCommand::default());
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert!(
            !direct.mission.imported_source_slot_is_live(51),
            "the next turn reclaims the disabled imported source slot"
        );
        with_session(handle, |capi| {
            assert!(!capi.mission.imported_source_slot_is_live(51));
            assert_eq!(direct.live_world_hash_v2(), capi.live_world_hash_v2());
            assert_eq!(
                direct.mission.source_arena_hash_v1(),
                capi.mission.source_arena_hash_v1()
            );
            Ok(())
        })
        .expect("registered C API session");
        let retired_projection = with_session(handle, |capi| {
            Ok(entity_snapshot(
                &capi.world.entities()[51],
                &capi.state,
                &capi.live_world,
                &capi.mission,
            ))
        })
        .expect("retired imported Villager projection");
        assert_eq!(
            (
                retired_projection.entity_index,
                retired_projection.authority,
                retired_projection.active,
            ),
            (51, MC_AUTHORITY_LEVEL1_MISSION_LIVE, 1),
            "an existing MissionLive payload owns the projection as soon as the imported row retires"
        );

        let resident_position = direct.live_world.houses()[0].position;
        let resident_slot = direct
            .mission
            .spawn_house_resident(resident_position, 4, 33, 0)
            .expect("direct resident constructor")
            .expect("direct source arena resident slot");
        let capi_resident_slot = with_session_mut(handle, |capi| {
            Ok(capi
                .mission
                .spawn_house_resident(resident_position, 4, 33, 0)
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?
                .expect("C API source arena resident slot"))
        })
        .expect("registered C API session");
        assert_eq!(resident_slot, capi_resident_slot);
        let replacement_projection = with_session(handle, |capi| {
            Ok(entity_snapshot(
                &capi.world.entities()[usize::from(capi_resident_slot)],
                &capi.state,
                &capi.live_world,
                &capi.mission,
            ))
        })
        .expect("MissionLive replacement projection");
        assert_eq!(
            (
                replacement_projection.entity_index,
                replacement_projection.authority,
                replacement_projection.active,
                replacement_projection.class_id,
                replacement_projection.model,
                replacement_projection.action,
            ),
            (resident_slot, MC_AUTHORITY_LEVEL1_MISSION_LIVE, 1, 5, 4, 33,),
            "the allocated row belongs exclusively to its MissionLive resident"
        );
        for (model, action) in [(12, 97), (13, 105), (14, 113)] {
            let direct_slot = direct
                .mission
                .spawn_house_resident(resident_position, model, action, 0)
                .expect("direct resident-family constructor")
                .expect("direct source arena resident-family slot");
            let capi_slot = with_session_mut(handle, |capi| {
                Ok(capi
                    .mission
                    .spawn_house_resident(resident_position, model, action, 0)
                    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?
                    .expect("C API source arena resident-family slot"))
            })
            .expect("registered C API session");
            assert_eq!(direct_slot, capi_slot);
        }
        direct.step(PlayerCommand::default());
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let mut observation = McLevel1ResidentObservationV1::default();
        assert_eq!(
            mc_native_level1_resident_observation_v1(
                handle,
                &raw mut observation,
                struct_size::<McLevel1ResidentObservationV1>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            (
                observation.required_mask,
                observation.observed_mask,
                observation.remaining_mask,
                observation.digest,
            ),
            (
                direct.mission.required_resident_behavior_mask(),
                direct.mission.observed_resident_behavior_mask(),
                direct.mission.remaining_resident_behavior_mask(),
                direct.mission.resident_behavior_observation_digest_v1(),
            ),
            "the public C ABI observation receipt must exactly match direct Rust state"
        );
        with_session(handle, |capi| {
            assert!(direct.mission.resident_projection_is_linked(resident_slot));
            assert!(capi.mission.resident_projection_is_linked(resident_slot));
            assert!(
                capi.renderable_map_chains.tracks(resident_slot),
                "C API applies the resident render projection in the dispatch slot"
            );
            assert_eq!(
                direct.live_world_hash_v2(),
                capi.live_world_hash_v2(),
                "resident effect live-world hashes diverged"
            );
            assert_eq!(
                direct.mission.source_arena_hash_v1(),
                capi.mission.source_arena_hash_v1(),
                "resident effect source-arena hashes diverged"
            );
            assert_eq!(direct.mission.remaining_resident_behavior_mask(), 0);
            assert_eq!(capi.mission.remaining_resident_behavior_mask(), 0);
            Ok(())
        })
        .expect("registered C API session");

        direct.mission.has_fireball = true;
        direct.mission.player_mana = 10_000;
        with_session_mut(handle, |capi| {
            capi.mission.has_fireball = true;
            capi.mission.player_mana = 10_000;
            Ok(())
        })
        .expect("registered C API session");
        let mut fire_command = valid_command();
        fire_command.flags = 0x10;
        let mut saw_dynamic_family = false;
        for turn in 25..=36 {
            let command = if turn % 2 == 1 {
                fire_command
            } else {
                valid_command()
            };
            direct.step(player_command(command).expect("valid parity command"));
            saw_dynamic_family |=
                !direct.mission.projectiles().is_empty() || !direct.mission.impacts().is_empty();
            assert_eq!(
                mc_native_session_step(
                    handle,
                    &raw const command,
                    &raw mut snapshot,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
            with_session(handle, |capi| {
                assert_eq!(
                    state_hash(&direct.state),
                    state_hash(&capi.state),
                    "flight state diverged on dynamic turn {turn}"
                );
                assert_eq!(
                    direct.live_world_hash_v2(),
                    capi.live_world_hash_v2(),
                    "typed imported world diverged on dynamic turn {turn}"
                );
                assert_eq!(
                    direct.mission.source_arena_hash_v1(),
                    capi.mission.source_arena_hash_v1(),
                    "source arena diverged on dynamic turn {turn}"
                );
                assert_eq!(
                    direct.mission.projectiles().len(),
                    capi.mission.projectiles().len(),
                    "dynamic family count diverged on turn {turn}"
                );
                Ok(())
            })
            .expect("registered C API session");
        }
        assert!(
            saw_dynamic_family,
            "dynamic parity phase must allocate and dispatch a Fireball family"
        );
        direct.reset();
        assert_eq!(
            mc_native_session_reset(
                handle,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let reset_projection = with_session(handle, |capi| {
            Ok(entity_snapshot(
                &capi.world.entities()[51],
                &capi.state,
                &capi.live_world,
                &capi.mission,
            ))
        })
        .expect("reset Villager projection");
        assert_eq!(
            (
                reset_projection.entity_index,
                reset_projection.authority,
                reset_projection.active,
                reset_projection.class_id,
                reset_projection.model,
            ),
            (51, MC_AUTHORITY_VILLAGER_LIVE, 1, 5, 13),
            "reset alone restores the exact imported Villager identity"
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn replaceable_fallback_hides_the_stale_generic_entity_projection() {
        let handle = open_fixture();
        let replaced_slot = with_session_mut(handle, |session| {
            let replaced_slot = 70;
            let stale_villager = session
                .live_world
                .villagers()
                .iter()
                .find(|villager| villager.entity_index == replaced_slot)
                .expect("row 70 imported Villager");
            assert!(stale_villager.active);
            assert!(
                session.mission.disable_imported_source_slot(replaced_slot),
                "production disable marks the imported source row"
            );
            let player = session.state;
            session.mission.begin_source_turn();
            session.mission.finish_source_turn(&player);
            assert!(
                !session.mission.imported_source_slot_is_live(replaced_slot),
                "production turn-start cleanup retires the imported source row"
            );
            Ok(replaced_slot)
        })
        .expect("replaceable fallback Villager slot");

        assert!(
            (51..=78).contains(&replaced_slot),
            "the discriminating fallback must retire one of the 28 Villager rows"
        );
        with_session(handle, |session| {
            let entity = &session.world.entities()[usize::from(replaced_slot)];
            assert!(
                entity.authority == EntityAuthority::VillagerLive
                    && entity.active
                    && !session.mission.imported_source_slot_is_live(replaced_slot),
                "the stale typed Villager remains present behind a retired source slot"
            );
            Ok(())
        })
        .expect("registered C API session");

        let mut snapshot = McEntitySnapshotV2::default();
        let mut written = 0;
        assert_eq!(
            mc_native_entities_copy(
                handle,
                u32::from(replaced_slot),
                &raw mut snapshot,
                1,
                struct_size::<McEntitySnapshotV2>(),
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 1);
        assert_eq!(snapshot.entity_index, replaced_slot);
        assert_eq!(snapshot.active, 0);
        assert_eq!(snapshot.authority, MC_AUTHORITY_INACTIVE);
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn dead_player_ignores_flight_view_and_spell_input_until_reset() {
        let handle = open_fixture();
        with_session_mut(handle, |session| {
            session.mission.objective_textbox_ticks = 4;
            let fire = session
                .context
                .spell_fire
                .get_or_insert_with(Default::default);
            fire.enabled_holder_indices[0] = 1;
            session
                .mission
                .capture_spell_holder_active_values(&session.context);
            let lethal_damage = session.mission.player_max_life.saturating_add(1);
            session.mission.submit_player_damage(lethal_damage, 10);
            Ok(())
        })
        .unwrap();
        assert_eq!(mc_native_level1_player_menu_state(handle, 5), MC_STATUS_OK);

        let mut retained_command = valid_command();
        retained_command.flags = 1;
        retained_command.roll = 72;
        retained_command.pitch = -48;
        let mut dying_entry = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const retained_command,
                &raw mut dying_entry,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let (retained_roll_delta, retained_pitch_delta, retained_target_speed) =
            with_session(handle, |session| {
                assert_eq!(
                    session.mission.player_action,
                    sim_core::level1_mission::PLAYER_ACTION_DYING
                );
                Ok((
                    session.state.roll_delta,
                    session.state.pitch_delta,
                    session.state.target_speed,
                ))
            })
            .unwrap();

        let mut lifecycle = McLevel1PlayerLifecycleSnapshotV2::default();
        assert_eq!(
            mc_native_level1_player_lifecycle_snapshot(
                handle,
                &raw mut lifecycle,
                struct_size::<McLevel1PlayerLifecycleSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(lifecycle.player_action, 2);
        assert_eq!(
            (
                lifecycle.player_active,
                lifecycle.player_visible,
                lifecycle.player_targetable,
                lifecycle.input_suppressed,
            ),
            (1, 1, 0, 1)
        );

        let mut suppressed = valid_command();
        suppressed.flags = 0x12;
        suppressed.roll = -101;
        suppressed.pitch = 99;
        suppressed.requested_spell = 0;
        let fire_sequence_before = dying_entry.fire_sequence;
        let expected_burst_position = with_session(handle, |session| {
            let mut expected_state = session.state;
            let mut expected_context = session.context.clone();
            Ok(step_entity_body_with_context(
                &mut expected_state,
                session.world.profile(),
                &session.terrain,
                &mut expected_context,
            )
            .proposed)
        })
        .unwrap();
        let mut after_direct_body = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const suppressed,
                &raw mut after_direct_body,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        with_session(handle, |session| {
            assert_eq!(
                (
                    session.state.roll_delta,
                    session.state.pitch_delta,
                    session.state.target_speed,
                ),
                (
                    retained_roll_delta,
                    retained_pitch_delta,
                    retained_target_speed,
                ),
                "action 2 runs the direct retained flight body without relatching command axes"
            );
            assert!(
                !session.mission.death_bursts().is_empty(),
                "the production action-2 pass emits its per-tick burst"
            );
            assert_eq!(
                session.mission.death_bursts().last().unwrap().position,
                expected_burst_position,
                "the C ABI production route retains predictedAxis before the separate fall"
            );
            let burst = session.mission.death_bursts().last().unwrap();
            assert!(
                session
                    .renderable_map_chains
                    .entity_chain(
                        burst.position.x.to_be_bytes()[0],
                        burst.position.y.to_be_bytes()[0],
                    )
                    .contains(&burst.source_entity_index),
                "a live action-1 burst survives the production CAPI chain handoff before marker replacement"
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(after_direct_body.fire_sequence, fire_sequence_before);
        assert_eq!(
            mc_native_level1_player_lifecycle_snapshot(
                handle,
                &raw mut lifecycle,
                struct_size::<McLevel1PlayerLifecycleSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(lifecycle.menu_close_requested, 1);
        assert_eq!(lifecycle.menu_state, 0);

        let mut mission = McLevel1MissionSnapshotV2::default();
        const DYING_OUTER_FRAMES: u8 = 78;
        with_session(handle, |session| {
            assert_eq!(
                (
                    session.state.position,
                    session.mission.player_fall_velocity,
                    session
                        .terrain
                        .ground_height(session.state.position)
                        .wrapping_add(100),
                    session.terrain.mutation_sequence(),
                ),
                (
                    Position {
                        x: 19_684,
                        y: 56_918,
                        z: 5_020,
                    },
                    -2,
                    4_864,
                    2,
                ),
                "fixed owner-data position, source fall velocity, +100 collision floor, and terrain generation form the independent action-2 preimage"
            );
            Ok(())
        })
        .unwrap();
        for dying_step in 0..DYING_OUTER_FRAMES {
            assert_eq!(
                mc_native_level1_mission_snapshot(
                    handle,
                    &raw mut mission,
                    struct_size::<McLevel1MissionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
            assert_eq!(
                mc_native_level1_player_lifecycle_snapshot(
                    handle,
                    &raw mut lifecycle,
                    struct_size::<McLevel1PlayerLifecycleSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
            assert_eq!(lifecycle.player_action, 2);
            let (expected_position, expected_velocity, expected_contact) =
                with_session(handle, |session| {
                    let mut expected_state = session.state;
                    let mut expected_context = session.context.clone();
                    step_entity_body_with_context(
                        &mut expected_state,
                        session.world.profile(),
                        &session.terrain,
                        &mut expected_context,
                    );
                    expected_state.position.z = expected_state
                        .position
                        .z
                        .wrapping_add(session.mission.player_fall_velocity);
                    let mut expected_velocity = session
                        .mission
                        .player_fall_velocity
                        .wrapping_sub(2)
                        .max(-256);
                    if expected_velocity > 0 {
                        expected_velocity = 0;
                    }
                    let ground = session
                        .terrain
                        .ground_height(expected_state.position)
                        .wrapping_add(100);
                    if expected_state.position.z < ground {
                        expected_state.position.z = ground;
                    }
                    Ok((
                        expected_state.position,
                        expected_velocity,
                        expected_state.position.z == ground,
                    ))
                })
                .unwrap();
            assert_eq!(
                expected_contact,
                dying_step == DYING_OUTER_FRAMES - 1,
                "the source recurrence independently reaches contact only on outer frame 78"
            );
            let mut step = McSessionSnapshotV2::default();
            assert_eq!(
                mc_native_session_step(
                    handle,
                    &raw const suppressed,
                    &raw mut step,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
            with_session(handle, |session| {
                assert_eq!(session.state.position, expected_position);
                assert_eq!(session.mission.player_fall_velocity, expected_velocity);
                assert_eq!(
                    session.mission.player_action,
                    if expected_contact { 3 } else { 2 }
                );
                Ok(())
            })
            .unwrap();
        }
        assert_eq!(
            mc_native_level1_mission_snapshot(
                handle,
                &raw mut mission,
                struct_size::<McLevel1MissionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_level1_player_lifecycle_snapshot(
                handle,
                &raw mut lifecycle,
                struct_size::<McLevel1PlayerLifecycleSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(lifecycle.player_action, 3);
        assert_eq!(mission.player_dead, 1);
        assert_eq!(
            mission.objective_textbox_ticks,
            200_u8 - DYING_OUTER_FRAMES,
            "PresentObjective remains frame-driven while dead-player input is suppressed"
        );
        assert_eq!(
            (
                lifecycle.player_active,
                lifecycle.player_visible,
                lifecycle.player_targetable,
                lifecycle.menu_close_requested,
                lifecycle.notification_id,
                lifecycle.notification_state,
                lifecycle.notification_timer,
                lifecycle.marker_present,
                lifecycle.marker_class,
                lifecycle.marker_model,
                lifecycle.marker_action,
                lifecycle.marker_byte_38,
            ),
            (1, 0, 0, 0, 374, 1, 100, 1, 10, 40, 42, 2)
        );
        assert_eq!(
            (lifecycle.marker_life, lifecycle.marker_max_life),
            (300, 300)
        );
        assert_eq!(
            lifecycle.marker_dispatch_phase,
            lifecycle.marker_source_entity_index.to_le_bytes()[0]
        );
        assert_eq!(
            lifecycle.marker_constructor_phase,
            u32::from(lifecycle.marker_source_entity_index % 11)
        );
        assert_eq!(
            lifecycle.marker_trigger_entity_index,
            dying_entry.entity_index
        );
        assert_eq!(lifecycle.marker_particle_parameter_index, 65);
        assert!(lifecycle.lifecycle_trace_count >= 33);
        assert_ne!(lifecycle.lifecycle_hash, lifecycle.source_arena_hash);
        assert_eq!(lifecycle.killer_score_awards, 0);
        with_session(handle, |session| {
            let marker = session
                .mission
                .player_death_marker()
                .expect("production action 2 allocated the death marker");
            assert!(
                session
                    .renderable_map_chains
                    .tracks(marker.source_entity_index),
                "the production C ABI retains model 40 in its live source map"
            );
            assert!(
                session
                    .renderable_map_chains
                    .entity_chain(
                        marker.position.x.to_le_bytes()[1],
                        marker.position.y.to_le_bytes()[1],
                    )
                    .contains(&marker.source_entity_index),
                "sub_501D0 map-chain insertion survives the C ABI presentation sync"
            );
            Ok(())
        })
        .unwrap();

        let (released_slot, released_position, moved_position) =
            with_session_mut(handle, |session| {
                let released = session
                    .mission
                    .released_spell_holders()
                    .iter()
                    .flatten()
                    .find(|released| released.source_entity_index == 1)
                    .copied()
                    .expect("literal holder one survives the production contact path");
                let moved = Position {
                    x: released.position.x.wrapping_add(1_024),
                    ..released.position
                };
                session
                    .renderable_map_chains
                    .sync_entity(RenderableMapSeed {
                        entity_index: released.source_entity_index,
                        active: true,
                        x: moved.x,
                        y: moved.y,
                    })
                    .expect("simulate the later live actor CopyPosition update");
                Ok((released.source_entity_index, released.position, moved))
            })
            .unwrap();
        let action_timer = lifecycle.action_timer;
        let mut banished = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const suppressed,
                &raw mut banished,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_level1_player_lifecycle_snapshot(
                handle,
                &raw mut lifecycle,
                struct_size::<McLevel1PlayerLifecycleSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(lifecycle.player_action, 3);
        assert_eq!(lifecycle.action_timer, action_timer);
        assert_eq!(lifecycle.banished_camera_active, 1);
        with_session(handle, |session| {
            let moved_tile = (
                moved_position.x.to_be_bytes()[0],
                moved_position.y.to_be_bytes()[0],
            );
            assert!(
                session
                    .renderable_map_chains
                    .entity_chain(moved_tile.0, moved_tile.1)
                    .contains(&released_slot),
                "a later live holder position owns the action-3 CAPI chain"
            );
            let released_tile = (
                released_position.x.to_be_bytes()[0],
                released_position.y.to_be_bytes()[0],
            );
            if moved_tile != released_tile {
                assert!(
                    !session
                        .renderable_map_chains
                        .entity_chain(released_tile.0, released_tile.1)
                        .contains(&released_slot),
                    "historical release coordinates are not replayed on action-3 turns"
                );
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(
            (
                lifecycle.controller_pitch,
                lifecycle.controller_roll,
                lifecycle.recoil_speed,
            ),
            (0, 0, 0)
        );

        let mut reset = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_reset(handle, &raw mut reset, struct_size::<McSessionSnapshotV2>()),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_level1_mission_snapshot(
                handle,
                &raw mut mission,
                struct_size::<McLevel1MissionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(mission.player_dead, 0);
        assert_eq!(mission.player_life, mission.player_max_life);
        assert_eq!(
            mc_native_level1_player_lifecycle_snapshot(
                handle,
                &raw mut lifecycle,
                struct_size::<McLevel1PlayerLifecycleSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(lifecycle.player_action, 0);
        assert_eq!(lifecycle.marker_present, 0);
        assert_eq!(lifecycle.notification_id, 0);

        let (player_entity_index, victim_color, lethal_damage) =
            with_session_mut(handle, |session| {
                let position = session.state.position;
                session.state.position.z =
                    session.terrain.ground_height(position).wrapping_add(100);
                Ok((
                    session.state.entity_index,
                    session.state.player_color_index,
                    session.mission.player_max_life.saturating_add(1),
                ))
            })
            .unwrap();
        assert_eq!(
            mc_native_level1_player_inject_damage_for_qualification(
                handle,
                lethal_damage,
                player_entity_index,
            ),
            MC_STATUS_OK
        );
        let mut lethal_entry = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const suppressed,
                &raw mut lethal_entry,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        with_session_mut(handle, |session| {
            assert_eq!(
                session.mission.player_action,
                sim_core::level1_mission::PLAYER_ACTION_DYING
            );
            let position = session.state.position;
            session.state.position.z = session.terrain.ground_height(position).wrapping_add(100);
            session.state.recoil_speed = 0;
            session.state.skip_flight_once = true;
            Ok(())
        })
        .unwrap();
        for _ in 0..8 {
            let mut step = McSessionSnapshotV2::default();
            assert_eq!(
                mc_native_session_step(
                    handle,
                    &raw const suppressed,
                    &raw mut step,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
            assert_eq!(
                mc_native_level1_player_lifecycle_snapshot(
                    handle,
                    &raw mut lifecycle,
                    struct_size::<McLevel1PlayerLifecycleSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
            if lifecycle.player_action == sim_core::level1_mission::PLAYER_ACTION_BANISHED {
                break;
            }
        }
        assert_eq!(
            lifecycle.player_action,
            sim_core::level1_mission::PLAYER_ACTION_BANISHED
        );
        assert_eq!(lifecycle.killer_entity_index, player_entity_index);
        assert_eq!(lifecycle.killer_score_victim_color, victim_color);
        assert_eq!(
            lifecycle.killer_score_awards, 1,
            "the C ABI projects the qualifying killer's own victim-color score"
        );
        with_session_mut(handle, |session| {
            session.state.effective_pitch = 321;
            Ok(())
        })
        .unwrap();
        let mut banished_pitch_step = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const suppressed,
                &raw mut banished_pitch_step,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_level1_player_lifecycle_snapshot(
                handle,
                &raw mut lifecycle,
                struct_size::<McLevel1PlayerLifecycleSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            lifecycle.effective_pitch, 321,
            "the lifecycle ABI distinguishes retained effective pitch from cleared controller pitch"
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);

        let exhausted_handle = open_fixture();
        let (exhausted_player, exhausted_damage, exhausted_holder, allocated_residents) =
            with_session_mut(exhausted_handle, |session| {
                let holder = session
                    .world
                    .entities()
                    .iter()
                    .find(|entity| {
                        entity.active
                            && entity.class == 10
                            && entity.model == 45
                            && session.renderable_map_chains.tracks(entity.index)
                    })
                    .expect("Level 1 reset exposes a source-mapped static holder row")
                    .index;
                let fire = session
                    .context
                    .spell_fire
                    .get_or_insert_with(Default::default);
                fire.enabled_holder_indices[0] = holder;
                session
                    .mission
                    .capture_spell_holder_active_values(&session.context);
                let resident_position = session.state.position;
                let mut allocated_residents = 0_u16;
                while session
                    .mission
                    .spawn_house_resident(resident_position, 13, 105, 0)
                    .expect("qualified resident constructor")
                    .is_some()
                {
                    allocated_residents = allocated_residents.wrapping_add(1);
                }
                assert!(
                    allocated_residents > 0,
                    "the production source arena exposes allocatable rows before exhaustion"
                );
                Ok((
                    session.state.entity_index,
                    session.mission.player_max_life.saturating_add(1),
                    holder,
                    allocated_residents,
                ))
            })
            .unwrap();
        assert!(
            allocated_residents > 100,
            "the regression exhausts the real production source arena, not a one-slot stub"
        );
        assert_eq!(
            mc_native_level1_player_inject_damage_for_qualification(
                exhausted_handle,
                exhausted_damage,
                exhausted_player,
            ),
            MC_STATUS_OK
        );
        let mut exhausted_step = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                exhausted_handle,
                &raw const suppressed,
                &raw mut exhausted_step,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let force_contact = |handle| {
            with_session_mut(handle, |session| {
                let position = session.state.position;
                session.state.position.z =
                    session.terrain.ground_height(position).wrapping_add(100);
                session.state.recoil_speed = 0;
                session.state.skip_flight_once = true;
                Ok(())
            })
            .unwrap();
        };
        force_contact(exhausted_handle);
        assert_eq!(
            mc_native_session_step(
                exhausted_handle,
                &raw const suppressed,
                &raw mut exhausted_step,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let first_failed_release = with_session(exhausted_handle, |session| {
            assert_eq!(
                session.mission.player_action,
                sim_core::level1_mission::PLAYER_ACTION_DYING,
                "failed marker allocation retains action 2"
            );
            assert!(session.mission.player_death_marker().is_none());
            let released = session.mission.released_spell_holders()[0]
                .expect("failed marker allocation still releases the live holder row");
            assert_eq!(released.source_entity_index, exhausted_holder);
            let tile = (
                released.position.x.to_be_bytes()[0],
                released.position.y.to_be_bytes()[0],
            );
            assert!(
                session
                    .renderable_map_chains
                    .entity_chain(tile.0, tile.1)
                    .contains(&released.source_entity_index),
                "the CAPI handoff retains holder relinking when the marker allocator fails"
            );
            Ok(released)
        })
        .unwrap();
        with_session_mut(exhausted_handle, |session| {
            session.state.position.x = session.state.position.x.wrapping_add(4_096);
            Ok(())
        })
        .unwrap();
        let literal_one_prior_tile = with_session(exhausted_handle, |session| {
            Ok((0..=u8::MAX)
                .find_map(|tile_y| {
                    (0..=u8::MAX)
                        .find(|tile_x| {
                            session
                                .renderable_map_chains
                                .entity_chain(*tile_x, tile_y)
                                .contains(&1)
                        })
                        .map(|tile_x| (tile_x, tile_y))
                })
                .expect("exhaustion leaves literal source row one map-linked"))
        })
        .unwrap();
        force_contact(exhausted_handle);
        assert_eq!(
            mc_native_session_step(
                exhausted_handle,
                &raw const suppressed,
                &raw mut exhausted_step,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        with_session(exhausted_handle, |session| {
            assert_eq!(
                session.mission.player_action,
                sim_core::level1_mission::PLAYER_ACTION_DYING
            );
            assert!(session.mission.player_death_marker().is_none());
            let retried = session.mission.released_spell_holders()[0]
                .expect("the failed marker constructor retries holder release");
            assert_ne!(
                retried.position, first_failed_release.position,
                "a retry records its current production release position"
            );
            assert_eq!(
                retried.source_entity_index, 1,
                "the source retry consumes the literal-one holder registry sentinel"
            );
            let retried_tile = (
                retried.position.x.to_be_bytes()[0],
                retried.position.y.to_be_bytes()[0],
            );
            assert_ne!(retried_tile, literal_one_prior_tile);
            assert!(
                session
                    .immediate_released_holder_handoff
                    .contains(&(0, retried.source_entity_index, retried_tile.0, retried_tile.1)),
                "the current action-2 invocation publishes CopyEntityPosition immediately before later source slots dispatch"
            );
            let final_alias = session
                .mission
                .released_spell_holders()
                .iter()
                .flatten()
                .filter(|released| released.source_entity_index == retried.source_entity_index)
                .last()
                .copied()
                .expect("at least one literal-one release");
            let final_tile = (
                final_alias.position.x.to_be_bytes()[0],
                final_alias.position.y.to_be_bytes()[0],
            );
            assert!(
                session
                    .renderable_map_chains
                    .entity_chain(final_tile.0, final_tile.1)
                    .contains(&retried.source_entity_index),
                "the last family writing the aliased literal-one row owns its final map position"
            );
            if final_tile != retried_tile {
                assert!(
                    !session
                        .renderable_map_chains
                        .entity_chain(retried_tile.0, retried_tile.1)
                        .contains(&retried.source_entity_index),
                    "an earlier family alias cannot retain duplicate raw map membership"
                );
            }
            assert!(
                !session
                    .renderable_map_chains
                    .entity_chain(literal_one_prior_tile.0, literal_one_prior_tile.1)
                    .contains(&retried.source_entity_index),
                "the failed-contact retry unlinks literal row one from its prior tile"
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(mc_native_session_destroy(exhausted_handle), MC_STATUS_OK);
    }

    #[test]
    fn step_event_order_and_copy_since_survive_reset_generation() {
        let handle = open_fixture();
        let mut before = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_snapshot(
                handle,
                &raw mut before,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let mut command = valid_command();
        command.flags = 0x10;
        let mut after = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut after,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert!(after.event_head >= 2);
        let query = McEventQueryV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McEventQueryV2>(),
            generation: after.generation,
            cursor: 0,
            reserved: [0; 4],
        };
        let mut events = vec![McEventV2::default(); 32];
        let mut batch = McEventBatchV2::default();
        assert_eq!(
            mc_native_events_copy_since(
                handle,
                &raw const query,
                events.as_mut_ptr(),
                32,
                struct_size::<McEventV2>(),
                &raw mut batch,
                struct_size::<McEventBatchV2>(),
            ),
            MC_STATUS_OK
        );
        assert!(batch.written >= 2);
        let written = usize::try_from(batch.written).unwrap();
        for pair in events[..written].windows(2) {
            assert_eq!(pair[1].sequence, pair[0].sequence + 1);
        }
        assert!(
            events[..written]
                .iter()
                .any(|event| event.kind == MC_EVENT_REQUEST_SPELL_ACTIVATION)
        );
        assert!(
            events[..written - 1]
                .iter()
                .any(|event| event.kind == MC_EVENT_MOVEMENT_RESOLUTION)
        );
        let music_start = events[written - 1];
        assert_eq!(music_start.kind, MC_EVENT_START_MUSIC);
        assert_eq!(music_start.code, 1);
        assert_eq!(music_start.value_0, 0);
        assert_eq!(music_start.value_1, 1);

        let next_query = McEventQueryV2 {
            cursor: batch.next_cursor,
            ..query
        };
        assert_eq!(
            mc_native_events_copy_since(
                handle,
                &raw const next_query,
                events.as_mut_ptr(),
                32,
                struct_size::<McEventV2>(),
                &raw mut batch,
                struct_size::<McEventBatchV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(batch.written, 0);

        let mut reset = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_reset(handle, &raw mut reset, struct_size::<McSessionSnapshotV2>(),),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_events_copy_since(
                handle,
                &raw const query,
                events.as_mut_ptr(),
                32,
                struct_size::<McEventV2>(),
                &raw mut batch,
                struct_size::<McEventBatchV2>(),
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn level_one_music_starts_once_after_the_second_source_pass_and_rearms_on_reset() {
        let first = open_fixture();
        let second = open_fixture();

        for handle in [first, second] {
            with_session_mut(handle, |session| {
                assert_eq!(session.source_turn, 1);
                assert!(session.music_start_armed);
                assert!(
                    !session
                        .events
                        .iter()
                        .any(|event| event.kind == MC_EVENT_START_MUSIC)
                );
                session.step(PlayerCommand::default());
                assert_eq!(session.source_turn, 2);
                let starts = session
                    .events
                    .iter()
                    .filter(|event| event.kind == MC_EVENT_START_MUSIC)
                    .collect::<Vec<_>>();
                assert_eq!(starts.len(), 1);
                let start = starts[0];
                assert_eq!(start.code, 1);
                assert_eq!(start.value_0, 0);
                assert_eq!(start.value_1, 1);
                assert_eq!(start.asset_ref_index, MC_ASSET_REF_NONE);
                assert_eq!(start.generation, 1);
                assert_eq!(session.events.back().unwrap().kind, MC_EVENT_START_MUSIC);
                assert!(!session.music_start_armed);
                session.step(PlayerCommand::default());
                assert_eq!(
                    session
                        .events
                        .iter()
                        .filter(|event| event.kind == MC_EVENT_START_MUSIC)
                        .count(),
                    1
                );
                Ok(())
            })
            .unwrap();
        }

        with_session_mut(first, |session| {
            session.reset();
            assert_eq!(session.generation, 2);
            assert!(session.music_start_armed);
            assert!(session.events.is_empty());
            session.step(PlayerCommand::default());
            let start = session
                .events
                .iter()
                .find(|event| event.kind == MC_EVENT_START_MUSIC)
                .unwrap();
            assert_eq!(start.generation, 2);
            assert_eq!(start.value_1, 1);
            Ok(())
        })
        .unwrap();

        assert_eq!(mc_native_session_destroy(first), MC_STATUS_OK);
        assert_eq!(mc_native_session_destroy(second), MC_STATUS_OK);
    }

    #[test]
    fn capi_terrain_projection_exposes_target_list_block_and_action_51_planes() {
        let world = presentation_test_world();
        let mut direct = world.session();
        let handle = open_fixture_with_owner_cache();
        let configure = |live_world: &mut LiveWorldState, owner_entity_index: u16| {
            let mut house = live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == 11)
                .copied()
                .expect("house 11");
            house.action = sim_core::house::HOUSE_TERRAIN_TO_LIVE_ACTION;
            house.max_life = 2;
            house.life = 2;
            house.owner_entity_index = owner_entity_index;
            house.control_flags |= 1;
            live_world.initialize_builder_house(house).unwrap();
        };
        configure(&mut direct.live_world, direct.state.entity_index);
        with_session_mut(handle, |session| {
            configure(&mut session.live_world, session.state.entity_index);
            Ok(())
        })
        .expect("configure registered C API session");

        let command = valid_command();
        let mut snapshot = McSessionSnapshotV2::default();
        let copy_plane_for = |session_handle: u64, kind: u32| {
            let mut plane = vec![0_u8; 65_536];
            let mut written = 0;
            assert_eq!(
                mc_native_terrain_plane_copy_u8(
                    session_handle,
                    kind,
                    plane.as_mut_ptr(),
                    65_536,
                    &raw mut written,
                ),
                MC_STATUS_OK
            );
            assert_eq!(written, 65_536);
            plane
        };
        let copy_plane = |kind: u32| copy_plane_for(handle, kind);
        let copy_entity_indices = || {
            let mut plane = vec![0_i16; 65_536];
            let mut written = 0;
            assert_eq!(
                mc_native_terrain_entity_indices_copy(
                    handle,
                    plane.as_mut_ptr(),
                    65_536,
                    &raw mut written,
                ),
                MC_STATUS_OK
            );
            assert_eq!(written, 65_536);
            plane
        };

        let blocked_handle = open_fixture_with_owner_cache();
        let blocked_control_handle = open_fixture_with_owner_cache();
        assert_eq!(
            mc_native_level1_action_51_blocker_setup_for_qualification(
                blocked_handle,
                11,
                sim_core::house::HOUSE_TERRAIN_TO_LIVE_ACTION,
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_level1_action_51_blocker_setup_for_qualification(
                blocked_control_handle,
                11,
                u8::MAX,
            ),
            MC_STATUS_OK
        );
        let blocked_before = with_session(blocked_handle, |session| {
            let house = session
                .live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == 11)
                .expect("blocked house 11");
            let views = level1_turn_start_source_views(
                &session.mission,
                &session.state,
                &session.live_world,
            );
            assert!(
                Level1MissionState::house_action_51_blocked(&views, 11),
                "public setup must materialize a real overlapping model-42 source row: house={:?}, blockers={:?}, constructions={:?}",
                views.iter().find(|view| view.source_entity_index == 11),
                views
                    .iter()
                    .filter(|view| view.class == 10 && view.model == 42)
                    .collect::<Vec<_>>(),
                session.mission.house_completion_constructions()
            );
            Ok((house.action, house.life, house.phase))
        })
        .expect("public blocked-session setup");
        let mut blocked_snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                blocked_handle,
                &raw const command,
                &raw mut blocked_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let mut blocked_control_snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                blocked_control_handle,
                &raw const command,
                &raw mut blocked_control_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let blocked_after = with_session(blocked_handle, |session| {
            let house = session
                .live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == 11)
                .expect("blocked house 11 after public step");
            Ok((house.action, house.life, house.phase))
        })
        .expect("public blocked-session result");
        assert_eq!(
            blocked_before.0,
            sim_core::house::HOUSE_TERRAIN_TO_LIVE_ACTION
        );
        assert_eq!(blocked_before.1, 2);
        assert_eq!(blocked_after.0, blocked_before.0);
        assert_eq!(blocked_after.1, blocked_before.1);
        assert_eq!(blocked_after.2, blocked_before.2.wrapping_add(1));
        for (kind, before) in [
            MC_TERRAIN_HEIGHT_U8,
            MC_TERRAIN_TYPES_U8,
            MC_TERRAIN_SHADE_U8,
            MC_TERRAIN_ANGLE_U8,
            MC_TERRAIN_SECONDARY_U8,
        ]
        .into_iter()
        .map(|kind| (kind, copy_plane_for(blocked_control_handle, kind)))
        {
            assert!(
                copy_plane_for(blocked_handle, kind) == before,
                "blocked action 51 changed terrain plane kind {kind}"
            );
        }
        assert_eq!(mc_native_session_destroy(blocked_handle), MC_STATUS_OK);
        assert_eq!(
            mc_native_session_destroy(blocked_control_handle),
            MC_STATUS_OK
        );

        direct.step(PlayerCommand::default());
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            copy_plane(MC_TERRAIN_HEIGHT_U8),
            direct.terrain.planes().ground_heights.as_ref()
        );
        assert_eq!(
            copy_plane(MC_TERRAIN_TYPES_U8),
            direct.terrain.planes().terrain_types.as_ref()
        );
        assert_eq!(
            copy_plane(MC_TERRAIN_ANGLE_U8),
            direct.terrain.planes().angles.as_ref()
        );
        assert_eq!(
            copy_plane(MC_TERRAIN_SHADE_U8),
            direct.terrain.planes().shading.as_ref()
        );
        assert_eq!(
            copy_plane(MC_TERRAIN_SECONDARY_U8),
            direct.terrain.planes().secondary_heights.as_ref()
        );
        assert_eq!(
            copy_entity_indices(),
            direct.terrain.planes().entity_indices.as_ref()
        );
        assert_eq!(
            direct
                .live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == 11)
                .unwrap()
                .action,
            sim_core::house::HOUSE_TERRAIN_TO_LIVE_ACTION
        );

        direct.step(PlayerCommand::default());
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            copy_plane(MC_TERRAIN_HEIGHT_U8),
            direct.terrain.planes().ground_heights.as_ref()
        );
        assert_eq!(
            copy_plane(MC_TERRAIN_TYPES_U8),
            direct.terrain.planes().terrain_types.as_ref()
        );
        assert_eq!(
            copy_plane(MC_TERRAIN_ANGLE_U8),
            direct.terrain.planes().angles.as_ref()
        );
        assert_eq!(
            copy_plane(MC_TERRAIN_SHADE_U8),
            direct.terrain.planes().shading.as_ref()
        );
        assert_eq!(
            copy_plane(MC_TERRAIN_SECONDARY_U8),
            direct.terrain.planes().secondary_heights.as_ref()
        );
        assert_eq!(
            copy_entity_indices(),
            direct.terrain.planes().entity_indices.as_ref()
        );
        with_session(handle, |session| {
            assert_eq!(session.terrain, direct.terrain);
            assert_eq!(session.live_world, direct.live_world);
            assert_eq!(
                session.mission.source_arena_hash_v2(),
                direct.mission.source_arena_hash_v2()
            );
            assert_eq!(
                session.mission.house_construction_effects(),
                direct.mission.house_construction_effects()
            );
            assert_eq!(
                session.mission.house_completion_constructions(),
                direct.mission.house_completion_constructions()
            );
            assert_eq!(session.pending_house_effects, direct.pending_house_effects);
            Ok(())
        })
        .expect("direct/C ABI action-51 full-state equality");
        let capi_action = with_session(handle, |session| {
            Ok(session
                .live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == 11)
                .expect("C API house 11")
                .action)
        })
        .expect("registered C API house action");
        assert_eq!(capi_action, sim_core::house::HOUSE_LIVE_ACTION);
        assert_eq!(
            direct
                .live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == 11)
                .unwrap()
                .action,
            capi_action
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn capi_action_51_completion_exposes_rich_model_42_effect() {
        let handle = open_fixture_with_owner_cache();
        let parent_entity_index = 152;
        let house_building_index = with_session(handle, |session| {
            Ok(session
                .live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == 11)
                .expect("qualification house 11")
                .building_index)
        })
        .expect("qualification house selector");
        let construction_selector = [17, 26]
            .into_iter()
            .find(|selector| *selector != house_building_index)
            .expect("two distinct qualified construction selectors");
        let (parent_source_id, parent_position, expected_half_width, expected_half_depth) =
            with_session(handle, |session| {
                let parent = level1_turn_start_source_views(
                    &session.mission,
                    &session.state,
                    &session.live_world,
                )
                .into_iter()
                .find(|view| view.source_entity_index == parent_entity_index)
                .expect("qualification class-3 parent");
                let footprint = session
                    .world
                    .house_build_footprint(construction_selector)
                    .expect("qualification construction footprint");
                Ok((
                    parent.source_id,
                    parent.position,
                    i16::try_from(((i32::from(footprint.width) << 8) + 1_280) >> 1)
                        .expect("qualification width shift fits i16"),
                    i16::try_from(((i32::from(footprint.height) << 8) + 1_280) >> 1)
                        .expect("qualification depth shift fits i16"),
                ))
            })
            .expect("qualification parent state");
        let mut effect_count = u32::MAX;
        assert_eq!(
            mc_native_level1_house_completion_effect_count(handle, &raw mut effect_count),
            MC_STATUS_OK
        );
        assert_eq!(effect_count, 0);
        let mut hash_before = 0;
        assert_eq!(
            mc_native_session_source_arena_hash(handle, &raw mut hash_before),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_level1_action_51_completion_setup_for_qualification(
                handle,
                11,
                parent_entity_index,
                construction_selector,
            ),
            MC_STATUS_OK
        );
        let command = valid_command();
        let mut session_snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut session_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_level1_house_completion_effect_count(handle, &raw mut effect_count),
            MC_STATUS_OK
        );
        assert!(effect_count >= 1);
        let mut effect = McLevel1HouseCompletionEffectV2::default();
        assert_eq!(
            mc_native_level1_house_completion_effect_snapshot(
                handle,
                0,
                &raw mut effect,
                struct_size::<McLevel1HouseCompletionEffectV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(effect.abi_version, MC_NATIVE_ABI_VERSION);
        assert_eq!(effect.struct_size, 80);
        assert_eq!(effect.kind, MC_LEVEL1_HOUSE_COMPLETION_ALLOCATED);
        assert_eq!(effect.house_entity_index, 11);
        assert_eq!(effect.parent_entity_index, parent_entity_index);
        assert_ne!(effect.source_entity_index, 0);
        assert_eq!(effect.source_id, parent_source_id);
        assert_eq!((effect.class_id, effect.model, effect.action), (10, 42, 44));
        assert_eq!(effect.construction_selector, construction_selector);
        assert_ne!(
            effect.house_building_index, effect.construction_selector,
            "house byte 0x46 must remain distinct from the parent dword-0x10 selector"
        );
        assert_eq!(effect.constructor_latch, 1);
        assert_eq!(effect.active, 1);
        assert_eq!(
            effect.phase,
            effect.source_entity_index.to_le_bytes()[0].wrapping_add(1),
            "the newly allocated higher slot receives its ordinary outer phase increment later in the same source pass"
        );
        assert_eq!(effect.parent_word_2e, 4);
        assert_eq!(
            effect.parent_construction_selector,
            i16::from(construction_selector)
        );
        assert_eq!(
            (effect.x, effect.y, effect.z),
            (
                parent_position.x.wrapping_add(0x0101),
                parent_position.y.wrapping_add(0x0202),
                parent_position.z.wrapping_add(33),
            )
        );
        assert_eq!(effect.collision_half_width, expected_half_width);
        assert_eq!(effect.collision_half_depth, expected_half_depth);
        assert_eq!((effect.yaw, effect.fov), (0, 256));
        assert_eq!(
            (effect.max_life, effect.life, effect.control_flags),
            (0, 0, 1)
        );
        assert_eq!(effect.effect_ordinal, 0);
        assert_eq!(effect.effect_count, effect_count);
        let mut hash_after = 0;
        assert_eq!(
            mc_native_session_source_arena_hash(handle, &raw mut hash_after),
            MC_STATUS_OK
        );
        assert_ne!(hash_after, hash_before);
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut session_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_level1_house_completion_effect_count(handle, &raw mut effect_count),
            MC_STATUS_OK
        );
        assert_eq!(
            effect_count, 0,
            "completion-effect snapshots are scoped to their source turn"
        );
        assert_eq!(
            mc_native_level1_house_completion_effect_snapshot(
                handle,
                0,
                &raw mut effect,
                struct_size::<McLevel1HouseCompletionEffectV2>(),
            ),
            MC_STATUS_BOUNDS
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn action_51_raw_only_first_pass_survives_prior_public_finalization() {
        let world = presentation_test_world();
        let mut direct = world.session();
        let handle = open_fixture_with_owner_cache();
        let command = valid_command();
        let mut snapshot = McSessionSnapshotV2::default();

        direct.step(PlayerCommand::default());
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );

        let (
            class_2_slot,
            class_5_slot,
            building_index,
            house_position,
            house_source_id,
            raw_tile_before,
        ) = with_session_mut(handle, |session| {
            let raw_only = session
                .mission
                .source_map_chains()
                .linked_entity_indices()
                .into_iter()
                .filter(|entity_index| {
                    *entity_index > 11
                        && session.mission.imported_source_slot_is_live(*entity_index)
                        && !session
                            .live_world
                            .houses()
                            .iter()
                            .any(|house| house.entity_index == *entity_index)
                        && !session
                            .live_world
                            .goats()
                            .iter()
                            .any(|goat| goat.entity_index == *entity_index)
                        && !session
                            .live_world
                            .villagers()
                            .iter()
                            .any(|villager| villager.entity_index == *entity_index)
                })
                .take(2)
                .collect::<Vec<_>>();
            assert_eq!(raw_only.len(), 2, "two raw-only qualification rows");
            let retained_presentation = session
                .renderable_map_chains
                .linked_entity_indices()
                .into_iter()
                .filter(|entity_index| !raw_only.contains(entity_index))
                .collect::<Vec<_>>();
            session
                .renderable_map_chains
                .retain_entities(&retained_presentation)
                .expect("remove qualification rows from presentation only");
            let footprint = (0..=u8::MAX)
                .filter_map(|index| session.world.house_build_footprint(index))
                .find(|footprint| footprint.cells().iter().any(|cell| cell[0] == 0xff))
                .cloned()
                .expect("qualification BUILD contains a 0xff material cell");
            let (start_x, start_y) = (0..=u8::MAX)
                .flat_map(|y| (0..=u8::MAX).map(move |x| (x, y)))
                .find(|(start_x, start_y)| {
                    (0..footprint.height).all(|row| {
                        (0..footprint.width).all(|column| {
                            session
                                .mission
                                .source_map_chains()
                                .entity_chain(
                                    start_x.wrapping_add(column),
                                    start_y.wrapping_add(row),
                                )
                                .is_empty()
                        })
                    })
                })
                .expect("empty wrapped BUILD footprint");
            let ff_ordinal = footprint
                .cells()
                .iter()
                .position(|cell| cell[0] == 0xff)
                .expect("0xff footprint cell");
            let ff_x = start_x.wrapping_add(
                u8::try_from(ff_ordinal % usize::from(footprint.width))
                    .expect("footprint x fits u8"),
            );
            let ff_y = start_y.wrapping_add(
                u8::try_from(ff_ordinal / usize::from(footprint.width))
                    .expect("footprint y fits u8"),
            );
            let raw_position = Position {
                x: u16::from(ff_x) << 8,
                y: u16::from(ff_y) << 8,
                z: 0,
            };
            let house_position = Position {
                x: u16::from(start_x.wrapping_add(footprint.width / 2)) << 8,
                y: u16::from(start_y.wrapping_add(footprint.height / 2)) << 8,
                z: 0,
            };
            assert!(
                session
                    .mission
                    .configure_action_51_raw_entity_for_qualification(
                        raw_only[0],
                        2,
                        1,
                        raw_only[0],
                        raw_position,
                    )
            );
            assert!(
                session
                    .mission
                    .configure_action_51_raw_entity_for_qualification(
                        raw_only[1],
                        5,
                        25,
                        raw_only[1],
                        raw_position,
                    )
            );
            assert!(!session.renderable_map_chains.tracks(raw_only[0]));
            assert!(!session.renderable_map_chains.tracks(raw_only[1]));

            let mut house = session
                .live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == 11)
                .copied()
                .expect("qualification house 11");
            house.action = sim_core::house::HOUSE_TERRAIN_TO_LIVE_ACTION;
            house.max_life = 2;
            house.life = 2;
            house.owner_entity_index = session.state.entity_index;
            house.control_flags |= 1;
            house.building_index = footprint.index;
            house.position = house_position;
            let house_source_id = house.source_id;
            session.live_world.initialize_builder_house(house).unwrap();
            Ok((
                raw_only[0],
                raw_only[1],
                footprint.index,
                house_position,
                house_source_id,
                session.mission.source_map_chains().entity_chain(ff_x, ff_y),
            ))
        })
        .expect("configure public raw-only action-51 pass");

        let footprint = world
            .house_build_footprint(building_index)
            .expect("direct qualification BUILD");
        let ff_ordinal = footprint
            .cells()
            .iter()
            .position(|cell| cell[0] == 0xff)
            .expect("direct 0xff footprint cell");
        let center_x = house_position.x.wrapping_add(128).to_be_bytes()[0];
        let center_y = house_position.y.wrapping_add(128).to_be_bytes()[0];
        let ff_x = center_x
            .wrapping_sub(footprint.width / 2)
            .wrapping_add(u8::try_from(ff_ordinal % usize::from(footprint.width)).unwrap());
        let ff_y = center_y
            .wrapping_sub(footprint.height / 2)
            .wrapping_add(u8::try_from(ff_ordinal / usize::from(footprint.width)).unwrap());
        let raw_position = Position {
            x: u16::from(ff_x) << 8,
            y: u16::from(ff_y) << 8,
            z: 0,
        };
        assert!(
            direct
                .mission
                .configure_action_51_raw_entity_for_qualification(
                    class_2_slot,
                    2,
                    1,
                    class_2_slot,
                    raw_position,
                )
        );
        assert!(
            direct
                .mission
                .configure_action_51_raw_entity_for_qualification(
                    class_5_slot,
                    5,
                    25,
                    class_5_slot,
                    raw_position,
                )
        );
        let mut direct_house = direct
            .live_world
            .houses()
            .iter()
            .find(|house| house.entity_index == 11)
            .copied()
            .expect("direct qualification house 11");
        direct_house.action = sim_core::house::HOUSE_TERRAIN_TO_LIVE_ACTION;
        direct_house.max_life = 2;
        direct_house.life = 2;
        direct_house.owner_entity_index = direct.state.entity_index;
        direct_house.control_flags |= 1;
        direct_house.building_index = building_index;
        direct_house.position = house_position;
        direct
            .live_world
            .initialize_builder_house(direct_house)
            .unwrap();
        assert_eq!(
            direct.mission.source_map_chains().entity_chain(ff_x, ff_y),
            raw_tile_before
        );

        direct.step(PlayerCommand::default());
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        with_session(handle, |session| {
            assert_eq!(
                session.mission.house_construction_effects(),
                direct.mission.house_construction_effects(),
                "direct and public C ABI preserve identical raw-chain effect order"
            );
            assert_eq!(
                session.mission.source_map_chains().entity_chain(ff_x, ff_y),
                raw_tile_before,
                "sub_57390 leaves the raw head and old links untouched"
            );
            assert_eq!(
                session.mission.raw_house_construction_state(class_2_slot),
                direct.mission.raw_house_construction_state(class_2_slot)
            );
            assert_eq!(
                session.mission.raw_house_construction_state(class_5_slot),
                direct.mission.raw_house_construction_state(class_5_slot)
            );
            Ok(())
        })
        .expect("public raw-only action-51 result");
        let class_2 = direct
            .mission
            .raw_house_construction_state(class_2_slot)
            .unwrap();
        let class_5 = direct
            .mission
            .raw_house_construction_state(class_5_slot)
            .unwrap();
        assert!(
            !direct.mission.house_construction_effects().is_empty(),
            "raw action-51 emitted no effects: house={:?} pending={:?} raw_chain={:?}",
            direct
                .live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == 11),
            direct.pending_house_effects,
            direct.mission.source_map_chains().entity_chain(ff_x, ff_y),
        );
        assert_ne!(
            class_2.0 & sim_core::level1_mission::SOURCE_DISABLED_FLAG,
            0
        );
        assert_eq!(
            (class_5.1, class_5.2, class_5.3),
            (-1, house_source_id, house_source_id)
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn public_turn_preserves_raw_map_rows_excluded_from_final_presentation() {
        let handle = open_fixture_with_owner_cache();
        let raw_only_entity = with_session_mut(handle, |session| {
            let seeds = session
                .world
                .raw_source_entity_indices()
                .iter()
                .copied()
                .map(|entity_index| {
                    let entity = &session.world.entities()[usize::from(entity_index)];
                    RenderableMapSeed {
                        entity_index,
                        active: true,
                        x: entity.position.x,
                        y: entity.position.y,
                    }
                })
                .collect::<Vec<_>>();
            let raw = RenderableMapChains::from_reset(
                &seeds,
                &session.world.terrain_planes().entity_indices,
                |entity_index| {
                    session
                        .world
                        .entities()
                        .get(usize::from(entity_index))
                        .map(EntitySnapshot::old_map_entity)
                },
            )
            .expect("qualified raw reset map topology");
            let raw_only_entity = raw
                .linked_entity_indices()
                .into_iter()
                .find(|entity_index| !session.renderable_map_chains.tracks(*entity_index))
                .expect("qualification fixture has a raw row excluded from presentation");
            session.mission.set_source_map_chains(raw);
            Ok(raw_only_entity)
        })
        .expect("install complete raw gameplay map");

        let command = valid_command();
        let mut snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        with_session(handle, |session| {
            assert!(
                session
                    .mission
                    .source_map_chains()
                    .entity_chain(
                        session.world.entities()[usize::from(raw_only_entity)]
                            .position
                            .x
                            .to_be_bytes()[0],
                        session.world.entities()[usize::from(raw_only_entity)]
                            .position
                            .y
                            .to_be_bytes()[0],
                    )
                    .contains(&raw_only_entity),
                "public presentation finalization must not replace the raw gameplay chain"
            );
            Ok(())
        })
        .expect("raw gameplay topology after public turn");
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn public_action_51_ordinary_then_fallback_then_exhaustion_matches_direct_exact_state() {
        let world = presentation_test_world();
        let mut direct = world.session();
        let handle = open_fixture_with_owner_cache();
        let (first, second, last, ordinary, replacement) =
            (900_u16, 901_u16, 902_u16, 903_u16, 904_u16);
        let construction_selector = (0..=u8::MAX)
            .find(|index| world.house_build_footprint(*index).is_some())
            .expect("qualified construction selector");
        let construction_axis = Position { x: 0, y: 0, z: 33 };
        assert!(
            direct
                .mission
                .configure_action_51_ordinary_fallback_exhaustion_for_qualification(
                    [first, second, last],
                    ordinary,
                    replacement,
                    construction_selector,
                    construction_axis,
                )
        );
        with_session_mut(handle, |session| {
            assert!(
                session
                    .mission
                    .configure_action_51_ordinary_fallback_exhaustion_for_qualification(
                        [first, second, last],
                        ordinary,
                        replacement,
                        construction_selector,
                        construction_axis,
                    )
            );
            Ok(())
        })
        .expect("configure public fallback arena");

        let configure_house = |house: &mut sim_core::house::HouseState, owner: u16| {
            house.action = sim_core::house::HOUSE_TERRAIN_TO_LIVE_ACTION;
            house.max_life = 1;
            house.life = 1;
            house.owner_entity_index = owner;
            house.control_flags |= 1;
            house.building_index = construction_selector;
            house.position = Position::default();
            house.collision_half_width = 40;
            house.collision_half_depth = 40;
        };
        let mut direct_house = direct
            .live_world
            .houses()
            .iter()
            .find(|house| house.entity_index == 11)
            .copied()
            .expect("direct fallback house");
        configure_house(&mut direct_house, direct.state.entity_index);
        direct
            .live_world
            .initialize_builder_house(direct_house)
            .unwrap();
        with_session_mut(handle, |session| {
            let mut house = session
                .live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == 11)
                .copied()
                .expect("public fallback house");
            configure_house(&mut house, session.state.entity_index);
            session.live_world.initialize_builder_house(house).unwrap();
            Ok(())
        })
        .expect("configure public fallback house");

        direct.step(PlayerCommand::default());
        let command = valid_command();
        let mut snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert!(matches!(
            direct.mission.house_construction_effects(),
            [
                HouseConstructionSourceEffect::ConstructionAllocated {
                    parent_entity_index,
                    source_entity_index,
                    ..
                },
                HouseConstructionSourceEffect::ConstructionAllocated {
                    parent_entity_index: fallback_parent,
                    source_entity_index: fallback_source,
                    ..
                },
                HouseConstructionSourceEffect::ConstructionAllocationFailed {
                    parent_entity_index: failed_parent,
                    ..
                }
            ] if *parent_entity_index == first
                && *source_entity_index == ordinary
                && *fallback_parent == second
                && *fallback_source == replacement
                && *failed_parent == last
        ));
        assert_eq!(
            direct
                .mission
                .house_completion_constructions()
                .iter()
                .map(|child| (child.source_entity_index, child.parent_entity_index))
                .collect::<Vec<_>>(),
            [(ordinary, first), (replacement, second)]
        );
        assert_eq!(
            (
                direct
                    .mission
                    .raw_house_construction_state(first)
                    .unwrap()
                    .4,
                direct
                    .mission
                    .raw_house_construction_state(second)
                    .unwrap()
                    .4,
                direct.mission.raw_house_construction_state(last).unwrap().4,
                direct
                    .mission
                    .raw_house_construction_state(ordinary)
                    .unwrap()
                    .4,
                direct
                    .mission
                    .raw_house_construction_state(replacement)
                    .unwrap()
                    .4,
            ),
            (4, 4, 9, 0, 0)
        );

        let mut effect_count = 0;
        assert_eq!(
            mc_native_level1_house_completion_effect_count(handle, &raw mut effect_count),
            MC_STATUS_OK
        );
        assert_eq!(effect_count, 3);
        let mut ordinary_effect = McLevel1HouseCompletionEffectV2::default();
        let mut fallback_effect = McLevel1HouseCompletionEffectV2::default();
        let mut failed = McLevel1HouseCompletionEffectV2::default();
        assert_eq!(
            mc_native_level1_house_completion_effect_snapshot(
                handle,
                0,
                &raw mut ordinary_effect,
                struct_size::<McLevel1HouseCompletionEffectV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_level1_house_completion_effect_snapshot(
                handle,
                1,
                &raw mut fallback_effect,
                struct_size::<McLevel1HouseCompletionEffectV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_level1_house_completion_effect_snapshot(
                handle,
                2,
                &raw mut failed,
                struct_size::<McLevel1HouseCompletionEffectV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            (
                ordinary_effect.kind,
                ordinary_effect.parent_entity_index,
                ordinary_effect.source_entity_index,
                ordinary_effect.parent_word_2e,
            ),
            (MC_LEVEL1_HOUSE_COMPLETION_ALLOCATED, first, ordinary, 4,)
        );
        assert_eq!(
            (
                fallback_effect.kind,
                fallback_effect.parent_entity_index,
                fallback_effect.source_entity_index,
                fallback_effect.parent_word_2e,
            ),
            (MC_LEVEL1_HOUSE_COMPLETION_ALLOCATED, second, replacement, 4,)
        );
        assert_eq!(
            (
                failed.kind,
                failed.parent_entity_index,
                failed.source_entity_index,
                failed.parent_word_2e,
            ),
            (MC_LEVEL1_HOUSE_COMPLETION_ALLOCATION_FAILED, last, 0, 9,)
        );
        let direct_digest = direct.mission.action_51_fallback_state_hash_v1();
        let mut public_digest = 0_u64;
        assert_eq!(
            mc_native_level1_action_51_fallback_state_hash_v1(handle, &raw mut public_digest,),
            MC_STATUS_OK
        );
        assert_eq!(public_digest, direct_digest);
        with_session(handle, |session| {
            assert_eq!(
                session.mission.house_construction_effects(),
                direct.mission.house_construction_effects()
            );
            assert_eq!(
                session.mission.house_completion_constructions(),
                direct.mission.house_completion_constructions()
            );
            assert_eq!(
                session.mission.action_51_fallback_state_hash_v1(),
                direct_digest,
                "the public digest covers exact direct/C-ABI allocator, raw-list, map, and construction state"
            );
            let views = level1_turn_start_source_views(
                &session.mission,
                &session.state,
                &session.live_world,
            );
            assert!(
                !session.mission.current_house_action_51_blocked(&views, 11),
                "fallback clears the later dword_38535 head as well as dword_38519"
            );
            Ok(())
        })
        .expect("public fallback result");
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn asset_strings_last_error_and_bounds_have_copy_contracts() {
        let handle = open_fixture();
        let mut count = 0;
        assert_eq!(
            mc_native_asset_ref_count(handle, &raw mut count),
            MC_STATUS_OK
        );
        assert_eq!(count, 76);
        let mut info = McAssetRefInfoV2::default();
        assert_eq!(
            mc_native_asset_ref_info(handle, 0, &raw mut info, struct_size::<McAssetRefInfoV2>(),),
            MC_STATUS_OK
        );
        assert_eq!(info.asset_index, 0);
        let mut length = 0;
        assert_eq!(
            mc_native_asset_string_length(
                handle,
                0,
                MC_ASSET_STRING_CONTENT_ADDRESS,
                0,
                &raw mut length,
            ),
            MC_STATUS_OK
        );
        assert_eq!(length, 71);
        let mut tiny = [0_u8; 1];
        assert_eq!(
            mc_native_asset_string_copy(
                handle,
                0,
                MC_ASSET_STRING_CONTENT_ADDRESS,
                0,
                tiny.as_mut_ptr(),
                1,
                &raw mut length,
            ),
            MC_STATUS_BUFFER_TOO_SMALL
        );
        let mut error_length = 0;
        assert_eq!(
            mc_native_last_error_length(&raw mut error_length),
            MC_STATUS_OK
        );
        assert!(error_length > 0);
        let mut error = vec![0_u8; usize::try_from(error_length).unwrap() + 1];
        assert_eq!(
            mc_native_last_error_copy(
                error.as_mut_ptr(),
                u32::try_from(error.len()).unwrap(),
                &raw mut error_length,
            ),
            MC_STATUS_OK
        );
        assert!(
            str::from_utf8(&error[..usize::try_from(error_length).unwrap()])
                .unwrap()
                .contains("requires")
        );
        assert_eq!(
            mc_native_asset_ref_info(handle, 76, &raw mut info, struct_size::<McAssetRefInfoV2>(),),
            MC_STATUS_BOUNDS
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn last_error_copy_can_resize_and_retry_without_losing_the_diagnostic() {
        let mut info = McWorldInfoV2::default();
        assert_eq!(
            mc_native_world_info(u64::MAX, &raw mut info, struct_size::<McWorldInfoV2>(),),
            MC_STATUS_UNKNOWN_HANDLE
        );
        let mut original_length = 0;
        assert_eq!(
            mc_native_last_error_length(&raw mut original_length),
            MC_STATUS_OK
        );
        assert!(original_length > 0);

        let mut tiny = [0_u8; 1];
        let mut reported_length = 0;
        assert_eq!(
            mc_native_last_error_copy(tiny.as_mut_ptr(), 1, &raw mut reported_length),
            MC_STATUS_BUFFER_TOO_SMALL
        );
        assert_eq!(reported_length, original_length);

        assert_eq!(
            mc_native_last_error_copy(
                std::ptr::null_mut(),
                original_length + 1,
                &raw mut reported_length,
            ),
            MC_STATUS_NULL_POINTER
        );
        assert_eq!(reported_length, original_length);

        let mut length_after_failure = 0;
        assert_eq!(
            mc_native_last_error_length(&raw mut length_after_failure),
            MC_STATUS_OK
        );
        assert_eq!(length_after_failure, original_length);
        let mut message = vec![0_u8; usize::try_from(original_length).unwrap() + 1];
        assert_eq!(
            mc_native_last_error_copy(
                message.as_mut_ptr(),
                u32::try_from(message.len()).unwrap(),
                &raw mut reported_length,
            ),
            MC_STATUS_OK
        );
        assert_eq!(reported_length, original_length);
        let message =
            str::from_utf8(&message[..usize::try_from(reported_length).unwrap()]).unwrap();
        assert!(message.contains("unknown session handle"));
        assert!(!message.contains("requires"));
    }

    #[test]
    fn corrupt_package_propagates_validation_status_and_useful_error() {
        let (package, closure) = fixture_paths();
        let temp = TempPackage::copy_from(&package, &closure);
        let path = temp.package.join("entities.bin");
        let mut bytes = fs::read(&path).unwrap();
        bytes[0] ^= 0xff;
        fs::write(&path, bytes).unwrap();
        assert_eq!(
            open_paths(&temp.package, &temp.closure),
            Err(MC_STATUS_VALIDATION_FAILED)
        );
        let mut length = 0;
        assert_eq!(mc_native_last_error_length(&raw mut length), MC_STATUS_OK);
        let mut error = vec![0_u8; usize::try_from(length).unwrap() + 1];
        assert_eq!(
            mc_native_last_error_copy(
                error.as_mut_ptr(),
                u32::try_from(error.len()).unwrap(),
                &raw mut length,
            ),
            MC_STATUS_OK
        );
        let error = str::from_utf8(&error[..usize::try_from(length).unwrap()]).unwrap();
        assert!(error.contains("hash mismatch"));
    }

    struct TempPackage {
        root: PathBuf,
        package: PathBuf,
        closure: PathBuf,
    }

    impl TempPackage {
        fn copy_from(package: &Path, closure: &Path) -> Self {
            let root = std::env::temp_dir().join(format!(
                "mc2-capi-v2-{}-{}",
                std::process::id(),
                unique_suffix()
            ));
            let output = root.join("package");
            fs::create_dir_all(&output).unwrap();
            fs::copy(package.join("manifest.json"), output.join("manifest.json")).unwrap();
            for name in [
                "authored-level.bin",
                "entities.bin",
                "players.bin",
                "presentation.bin",
                "runtime.bin",
                "terrain.bin",
            ] {
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
