//! Package-backed, handle-based ABI v2 for native presentation clients.
//!
//! All paths and data are supplied explicitly by the caller. This module does
//! not use the embedded owner regression bytes retained by the legacy v1 ABI.

use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    fs,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
    slice, str,
    sync::{Arc, Mutex, OnceLock},
};

use sha2::{Digest, Sha256};
use sim_core::{
    FlightContext, FlightEvent, FlightState, FlightWorld, POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING,
    PlayerCommand, Position, PositionalSoundCommand, StepEvidence,
    archer_tmap::{ArcherTmapBank, ArcherTmapClocks},
    combat_tmap::{
        COMBAT_STATIC_EXTENTS, COMBAT_STATIC_SELECTORS, COMBAT_STATIC_SHA256, CombatTmapBank,
        FIREBALL_PAYLOAD_SHA256, FireballTmapClock, POSSESSION_PAYLOAD_SHA256, PossessionTmapClock,
    },
    conformance_hash, conformance_hash_v2,
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
        GOAT_BASE_TMAP, GOAT_CLASS, GOAT_MODEL, GoatPlayerTarget, GoatState, GoatStepEvent,
        LEVEL_ONE_GOAT_INDICES, LiveWorldState,
    },
    goat_draw_admission::{
        GoatDrawAdmission, GoatDrawCamera, evaluate_goat_draw_admission,
        outdoor_billboard_visit_map,
    },
    goat_tmap::{
        GOAT_TMAP_DRAW_MASK, GOAT_TMAP_SELECTORS, GoatTmapBank, GoatTmapClock, GoatTmapClocks,
    },
    house::{HouseState, HouseStepEvent},
    level1_mission::{
        ARCHER_ARROW_BASE_TMAP, ARCHER_ARROW_LOGICAL_HEIGHT, ARCHER_ARROW_PROJECTILE_CLASS,
        ARCHER_ARROW_PROJECTILE_MODEL, ARCHER_ARROW_RENDER_INDEX, ARCHER_ARROW_RENDER_MODE,
        CASTLE_PROJECTILE_BASE_TMAP, CASTLE_PROJECTILE_CLASS, CASTLE_PROJECTILE_LOGICAL_HEIGHT,
        CASTLE_PROJECTILE_MODEL, CASTLE_PROJECTILE_RENDER_INDEX, CASTLE_PROJECTILE_RENDER_MODE,
        CASTLE_ROOT_ACTION, CASTLE_ROOT_BASE_TMAP, CASTLE_ROOT_CLASS, CASTLE_ROOT_LOGICAL_HEIGHT,
        CASTLE_ROOT_MODEL, CASTLE_ROOT_RENDER_INDEX, CASTLE_SPELL_FAMILY,
        CHECKPOINT_SMOKE_BASE_TMAP, CHECKPOINT_SMOKE_CLASS, CHECKPOINT_SMOKE_FIRST_RENDER_INDEX,
        CHECKPOINT_SMOKE_MODEL, CHECKPOINT_SMOKE_RENDER_MODE, FIREBALL_BASE_TMAP,
        FIREBALL_IMPACT_BASE_TMAP, FIREBALL_IMPACT_CLASS, FIREBALL_IMPACT_LOGICAL_HEIGHT,
        FIREBALL_IMPACT_RENDER_INDEX, FIREBALL_IMPACT_RENDER_MODE, FIREBALL_LOGICAL_HEIGHT,
        FIREBALL_PROJECTILE_CLASS, FIREBALL_PROJECTILE_MODEL, FIREBALL_RENDER_INDEX,
        FIREBALL_RENDER_MODE, FireballImpact, LEVEL1_MINIMAP_COMMAND_BLEND_PIXEL,
        LEVEL1_MINIMAP_MAX_ENTITY_COMMANDS, LEVEL1_MINIMAP_MAX_MARKS, Level1Actor, Level1ActorKind,
        Level1CollisionTarget, Level1MinimapEntityInput, Level1MinimapMarks, Level1MissionEvent,
        Level1MissionState, Level1MultipartSegment, Level1Projectile, MANA_CARRIER_ACTION,
        MANA_CARRIER_BASE_TMAP, MANA_CARRIER_CLASS, MANA_CARRIER_LOGICAL_HEIGHT,
        MANA_CARRIER_MAX_LIFE, MANA_CARRIER_MAX_MANA, MANA_CARRIER_MODEL,
        MANA_CARRIER_RENDER_INDEX, MANA_CARRIER_SPEED, MANA_SPHERE_ACTION, MANA_SPHERE_CLASS,
        MANA_SPHERE_MODEL, MANA_SPHERE_NEUTRAL_BASE_TMAP, MANA_SPHERE_NEUTRAL_RENDER_BASE,
        MANA_SPHERE_PLAYER_ONE_BASE_TMAP, MANA_SPHERE_PLAYER_ONE_RENDER_BASE, ManaCarrier,
        ManaSphere, POSSESSION_IMPACT_BASE_TMAP, POSSESSION_IMPACT_CLASS,
        POSSESSION_IMPACT_LOGICAL_HEIGHT, POSSESSION_IMPACT_MODEL, POSSESSION_IMPACT_RENDER_INDEX,
        POSSESSION_IMPACT_RENDER_MODE, POSSESSION_PROJECTILE_BASE_TMAP,
        POSSESSION_PROJECTILE_CLASS, POSSESSION_PROJECTILE_LOGICAL_HEIGHT,
        POSSESSION_PROJECTILE_MODEL, POSSESSION_PROJECTILE_RENDER_INDEX,
        POSSESSION_PROJECTILE_RENDER_MODE, PlayerHitReaction, WATER_SPLASH_BASE_TMAP,
        WATER_SPLASH_LOGICAL_HEIGHT, WATER_SPLASH_MODEL, WATER_SPLASH_RENDER_INDEX,
        WATER_SPLASH_RENDER_MODE, level1_minimap_castle_rope_pixels, level1_minimap_entity_command,
        mana_sphere_size_ordinal,
    },
    native_world::{
        AssetRef, EntityAuthority, EntitySnapshot, NativeWorld, ParticleParameter, SessionTerrain,
        TerrainPlanes,
    },
    positional_sound_should_replace, prepare_event_sound_command,
    render_camera::{TerrainSlopeCamera, TerrainSlopeCameraSample},
    renderable_map_chain::{RenderableMapChains, RenderableMapSeed},
    source_projection_hash, state_hash,
    static_world_tmap::{
        EXIT_TARGET_PAYLOAD_BYTES, EXIT_TARGET_PAYLOAD_SHA256, LEVEL1_SCENERY_SELECTORS,
        SPELL_JAR_PAYLOAD_BYTES, SPELL_JAR_PAYLOAD_SHA256, StaticWorldTmapBank,
    },
    step_outer_turn_with_context,
    terrain_frame::{
        LEVEL1_SKY_BYTES, LEVEL1_TERRAIN_ATLAS_BYTES, LEVEL1_VIEW_HEIGHT, LEVEL1_VIEW_PIXELS,
        LEVEL1_VIEW_WIDTH, Level1CastleRootRender, Level1FrozenActorRender, Level1TerrainFrame,
        Level1TerrainFrameCamera, Level1TerrainFrameResources, Level1WorldFrameResources,
        render_level1_terrain_frame, render_level1_world_frame,
    },
    terrain_grid::{OUTDOOR_GRID_VERTICES, OutdoorGridInput, outdoor_grid},
    terrain_presentation::outdoor_terrain_pnt5,
    villager::VillagerState,
};

#[cfg(test)]
use sim_core::level1_mission::FIREBALL_IMPACT_MODEL;

#[cfg(test)]
use sim_core::level1_mission::CASTLE_PROJECTILE_SOUND;

pub const MC_NATIVE_ABI_VERSION: u32 = 2;

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

fn apply_player_hit_reaction(state: &mut FlightState, reaction: PlayerHitReaction) {
    state.recoil_yaw = reaction.recoil_yaw;
    state.recoil_speed = reaction.recoil_speed;
}
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

const EVENT_CAPACITY: usize = 4_096;
const AUDIO_VOICE_COUNT: usize = 10;
const TERRAIN_CELLS: usize = 256 * 256;
const POSITIONAL_QUALIFICATION_VECTOR_COUNT: u32 = 23;
const TERRAIN_PNT5_QUALIFICATION_VECTOR_COUNT: u32 = 13;
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
    pub reserved_1: [u8; 4],
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
    pub reserved_0: u8,
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
    pending_house_effects: Vec<HouseStepEvent>,
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

impl SessionRecord {
    fn new(world: NativeWorld) -> AbiResult<Self> {
        let terrain_frame_assets = load_optional_terrain_frame_assets()?.map(Arc::new);
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
        let mission = world.level1_mission().session(
            world.entities()[usize::from(state.entity_index)].life,
            1_000,
        );
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
        Ok(Self {
            terrain: SessionTerrain::from_world(world.as_ref()),
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
            pending_house_effects: Vec::new(),
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
        self.goat_draw_admission = None;
        self.generation = next_nonzero(self.generation);
        self.event_head = 0;
        self.events.clear();
        self.audio_commands.clear();
        self.audio_voices = [None; AUDIO_VOICE_COUNT];
        self.music_start_armed = true;
        self.fire_sequence = 0;
        self.last_resolution = 0;
        let player = &self.world.players()[usize::from(self.world.runtime().local_player_slot)];
        let record = player.record_bytes();
        self.source_turn = player.turn;
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

    fn world_frame(&mut self) -> AbiResult<Level1TerrainFrame> {
        let frame = {
            let assets = self.terrain_frame_assets.as_ref().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "world frame unavailable: MC_ASSET_CACHE_PATH was not supplied when the session opened",
                )
            })?;
            let tmap_clocks = self.selector_frames.as_ref().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "world frame unavailable without qualified Goat TMAP descriptors",
                )
            })?;
            let archer_tmap_clocks = self.archer_selector_frames.as_ref().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "world frame unavailable without qualified Archer TMAP descriptors",
                )
            })?;
            let firefly_tmap_clocks = self.firefly_selector_frames.as_ref().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "world frame unavailable without qualified Firefly TMAP descriptors",
                )
            })?;
            let villager_tmap_clocks = self.villager_selector_frames.as_ref().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "world frame unavailable without qualified villager TMAP descriptors",
                )
            })?;
            let fireball_tmap_clock = self.fireball_selector_frame.as_ref().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "world frame unavailable without qualified Fireball TMAP descriptor",
                )
            })?;
            let possession_tmap_clock =
                self.possession_selector_frame.as_ref().ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "world frame unavailable without qualified Possession TMAP descriptor",
                    )
                })?;
            let castle_tmap_clock = self.castle_selector_frame.as_ref().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "world frame unavailable without qualified Castle TMAP descriptor",
                )
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
                    map_chains: &self.renderable_map_chains,
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
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))?
        };
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
        Ok(frame)
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
            self.audio_commands.remove(&expired.sequence);
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
        let footprint = self
            .mission
            .castle_stage_one_footprint()
            .cloned()
            .expect("Castle terrain updates require the validated BUILD[1] footprint");
        for update in updates {
            self.terrain
                .apply_castle_update(update, &footprint)
                .expect("qualified Level 1 Castle terrain update");
        }
    }

    fn step(&mut self, command: PlayerCommand) {
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
        if !self.mission.exit_sequence_owns_player_action() && !self.mission.player_dead {
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
        let mut evidence = Vec::new();
        for source_slot in 1_u16..1_000 {
            if let Some((owner_entity_index, payload)) =
                self.mission.take_building_possession_mailbox(source_slot)
            {
                self.live_world
                    .submit_house_possession(source_slot, owner_entity_index, payload)
                    .expect("possession mailbox targets a live eligible house");
            }
            let house_events = self
                .live_world
                .step_house_source_slot(source_slot, &self.terrain);
            self.apply_house_event_side_effects(&house_events);
            let player_target = [GoatPlayerTarget {
                entity_index: self.state.entity_index,
                position: self.state.position,
                control_flags: player_control_flags,
            }];
            let slot_goat_events =
                self.live_world
                    .step_source_slot(source_slot, &self.terrain, &player_target);
            self.live_world
                .step_villager_source_slot(source_slot, &self.terrain);
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
                evidence = if self.mission.exit_sequence_owns_player_action()
                    || self.mission.player_dead
                {
                    self.state.tick = self.state.tick.wrapping_add(1);
                    Vec::new()
                } else {
                    step_outer_turn_with_context(
                        &mut self.state,
                        command,
                        0,
                        world.profile(),
                        &self.terrain,
                        &mut self.context,
                    )
                };
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
            self.mission
                .set_source_map_chains(self.renderable_map_chains.clone());
            let mission_event_start = self.mission.events().len();
            if !self.mission.exit_sequence_owns_player_action() && !self.mission.player_dead {
                self.mission.dispatch_speed_holder_slot(
                    source_slot,
                    &mut self.state,
                    &mut self.context,
                );
            }
            self.mission.dispatch_source_slot_with_look(
                &self.terrain,
                &self.state,
                source_slot,
                fireball_activated,
                possession_activated,
                castle_cast_yaw,
                self.camera_look_yaw,
                self.camera_look_pitch,
            );
            if source_slot == self.state.entity_index && self.mission.exit_sequence_active {
                self.mission.advance_exit_sequence(
                    &self.terrain,
                    &mut self.state,
                    &mut self.context,
                    &mut self.camera_fov,
                );
            }
            self.apply_castle_terrain_updates();
            if let Some(reaction) = self.mission.take_player_hit_reaction() {
                // `sub_5EFA0` runs after this turn's player flight pass. The
                // installed directional boost is consumed by `sub_5F380` on
                // the next turn, not retroactively in the impact turn.
                apply_player_hit_reaction(&mut self.state, reaction);
            }
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
        self.mission.finish_source_turn(&self.state);
        self.sync_supported_renderable_records()
            .expect("validated final source renderable map update");
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
        let live_world_hash = self.live_world.hash_v1();
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

    fn apply_house_event_side_effects(&mut self, events: &[HouseStepEvent]) {
        for event in events {
            match *event {
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
        for impact in self.mission.impacts() {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: impact.source_entity_index,
                active: impact.active,
                x: impact.position.x,
                y: impact.position.y,
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
            if let Some(goat) = self
                .live_world
                .goats()
                .iter()
                .find(|goat| goat.entity_index == source_slot)
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: goat.entity_index,
                    active: goat.active,
                    x: goat.position.x,
                    y: goat.position.y,
                })?;
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
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: villager.entity_index,
                    active: villager.active,
                    x: villager.position.x,
                    y: villager.position.y,
                })?;
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
                .projectiles()
                .iter()
                .filter(|projectile| projectile.active())
                .map(|projectile| projectile.source_entity_index()),
        );
        active.extend(
            self.mission
                .impacts()
                .iter()
                .filter(|impact| impact.active)
                .map(|impact| impact.source_entity_index),
        );
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
        let mut pending = BTreeMap::<u16, u64>::new();
        for (sequence, command) in unresolved {
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
        for sequence in pending.into_values() {
            let mut command = self.audio_commands[&sequence];
            let (action, voice_index) = self.allocate_audio_voice(command);
            command.action = if matches!(command.sound_id, 1 | 2 | 5 | 31)
                && action == MC_AUDIO_ACTION_SUPPRESS_EXISTING
            {
                MC_AUDIO_ACTION_UPDATE_EXISTING
            } else {
                action
            };
            command.voice_index = voice_index;
            self.audio_commands.insert(sequence, command);
        }
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
                    self.audio_commands.insert(record.sequence, command);
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
            } => {
                let mut record = self.next_event_base(MC_EVENT_GOAT_SOUND_REQUEST, live_hash);
                record.entity_index = entity_index;
                record.code = u32::from(sound_id);
                record.asset_ref_index = self
                    .world
                    .sound_asset_ref(sound_id)
                    .unwrap_or(MC_ASSET_REF_NONE);
                if let Some(goat) = self
                    .live_world
                    .goats()
                    .iter()
                    .find(|goat| goat.entity_index == entity_index)
                    .map(|goat| (goat.position, goat.action))
                {
                    self.attach_mission_sound(
                        &mut record,
                        sound_id,
                        entity_index,
                        goat.0,
                        goat.1,
                        random_state,
                    );
                }
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
                damage,
                life,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_ACTOR_DAMAGED, hash);
                record.entity_index = authored_index;
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
            self.audio_commands.insert(
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
            return Some(local_audio_command_record(event, sound_id, 0, 3));
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
        reserved_0: 0,
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
        let world = NativeWorld::load(&package, &closure).map_err(|error| {
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
                live_world_hash: session.live_world.hash_v1(),
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
                live_world_hash: session.live_world.hash_v1(),
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
        player_dead: u8::from(mission.player_dead),
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
        last_possessed_building_entity: mission
            .possessed_buildings()
            .last()
            .map_or(0, |building| building.entity_index),
        last_possessed_building_owner: mission
            .possessed_buildings()
            .last()
            .map_or(0, |building| building.owner_entity_index),
        possessed_building_count: u16::try_from(mission.possessed_buildings().len())
            .expect("Level 1 possessed building count fits u16"),
        possession_tier: u8::try_from(mission.possession_tier)
            .expect("Level 1 possession tier is non-negative"),
        speed_trail_count: u8::try_from(mission.speed_trails().len()).unwrap_or(u8::MAX),
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
        family_selected_tier[family] = fire
            .holders
            .iter()
            .find(|holder| holder.entity_index == holder_index)
            .map_or(0, |holder| holder.level_gate);
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
        reserved_1: [0; 4],
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

fn level1_minimap_entity_commands(session: &SessionRecord) -> Vec<McLevel1MinimapEntityCommandV2> {
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
    McManaCarrierSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McManaCarrierSnapshotV2>(),
        generation: session.generation,
        tick: session.state.tick,
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
        } else if impact.model == POSSESSION_IMPACT_MODEL {
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
        class_id: if impact.model == POSSESSION_IMPACT_MODEL {
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
    let root = PathBuf::from(root);
    let terrain_atlas = read_qualified_cache_file(
        &root,
        "terrain/bl32n.atlas.indices.u8",
        LEVEL1_TERRAIN_ATLAS_BYTES,
        TERRAIN_ATLAS_SHA256,
    )?;
    let tablesn = read_qualified_cache_file(
        &root,
        "palette/tablesn.u8",
        sim_core::mode26_raster::NIGHT_TABLES_BYTES,
        TABLESN_SHA256,
    )?;
    let sky =
        read_qualified_cache_file(&root, "sky/skyn.indices.u8", LEVEL1_SKY_BYTES, SKYN_SHA256)?;
    let palette_rgb6 =
        read_qualified_cache_file(&root, "palette/paln.rgb6", 768, PALN_RGB6_SHA256)?;
    let archer_tmap_bank = load_qualified_archer_tmap_bank(&root)?;
    let firefly_tmap_bank = load_qualified_firefly_tmap_bank(&root)?;
    let frozen_actor_tmap_bank = load_qualified_frozen_actor_tmap_bank(&root)?;
    let combat_tmap_bank = load_qualified_combat_tmap_bank(&root)?;
    let static_world_tmap_bank = load_qualified_static_world_tmap_bank(&root)?;
    let economy_tmap_bank = load_qualified_economy_tmap_bank(&root)?;
    Ok(Some(TerrainFrameAssets {
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
    }))
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
    StaticWorldTmapBank::decode_with_level1_secondary_and_scenery(
        &spell_jar,
        &exit_target,
        std::array::from_fn(|index| secondary[index].as_slice()),
        std::array::from_fn(|index| scenery[index].as_slice()),
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
    use sim_core::FlightWorld;
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    #[repr(C, align(8))]
    struct AlignedAbiHeader {
        abi_version: u32,
        struct_size: u32,
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
    fn player_hit_bridge_installs_only_next_turn_directional_recoil() {
        let mut state = FlightState {
            recoil_yaw: 17,
            recoil_speed: 3,
            ..FlightState::default()
        };
        let reaction = PlayerHitReaction {
            attacker_entity_index: 10,
            raw_damage: 500,
            applied_damage: 500,
            recoil_yaw: 512,
            recoil_pitch: 1_792,
            recoil_speed: 50,
            pain_sound_id: 56,
            hit_frame_ticks: 4,
            regen_block_ticks: 16,
            hit_aux_ticks: 64,
            survived_feedback: 100,
            killer_entity_index: 0,
            lethal: false,
        };

        apply_player_hit_reaction(&mut state, reaction);

        assert_eq!(state.recoil_yaw, 512);
        assert_eq!(state.recoil_speed, 50);
        assert_eq!(state.position, FlightState::default().position);
        assert_eq!(state.tick, 0);
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
        open_paths(&package, &closure).expect("package M opens")
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
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("package M loads");
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
        session.terrain_frame_assets = Some(assets);
        with_registry_mut(|registry| registry.insert(session)).expect("fixture session registers")
    }

    fn terrain_frame_assets_fixture() -> TerrainFrameAssets {
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let cache = private.join("generated/level1-unity-asset-cache");
        TerrainFrameAssets {
            terrain_atlas: fs::read(cache.join("terrain/bl32n.atlas.indices.u8"))
                .unwrap()
                .into_boxed_slice(),
            tablesn: fs::read(cache.join("palette/tablesn.u8"))
                .unwrap()
                .into_boxed_slice(),
            sky: fs::read(cache.join("sky/skyn.indices.u8"))
                .unwrap()
                .into_boxed_slice(),
            palette_rgb6: fs::read(cache.join("palette/paln.rgb6"))
                .unwrap()
                .into_boxed_slice(),
            archer_tmap_bank: load_qualified_archer_tmap_bank(&cache)
                .expect("qualified Archer TMAP fixture"),
            firefly_tmap_bank: load_qualified_firefly_tmap_bank(&cache)
                .expect("qualified Firefly TMAP fixture"),
            frozen_actor_tmap_bank: load_qualified_frozen_actor_tmap_bank(&cache)
                .expect("qualified frozen actor TMAP fixture"),
            combat_tmap_bank: load_qualified_combat_tmap_bank(&cache)
                .expect("qualified combat TMAP fixture"),
            static_world_tmap_bank: load_qualified_static_world_tmap_bank(&cache)
                .expect("qualified static world TMAP fixture"),
            economy_tmap_bank: load_qualified_economy_tmap_bank(&cache)
                .expect("qualified economy TMAP fixture"),
        }
    }

    fn open_fixture_with_owner_cache() -> u64 {
        let (package, closure) = fixture_paths();
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load_with_owner_cache(
            package,
            closure,
            private.join("generated/level1-unity-asset-cache"),
        )
        .expect("package M and qualified owner cache load");
        let session = SessionRecord::new(world).expect("owner-cache fixture session constructs");
        with_registry_mut(|registry| registry.insert(session))
            .expect("owner-cache fixture session registers")
    }

    fn open_fixture_with_owner_cache_and_terrain_frame_assets() -> u64 {
        let (package, closure) = fixture_paths();
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load_with_owner_cache(
            package,
            closure,
            private.join("generated/level1-unity-asset-cache"),
        )
        .expect("package M and qualified owner cache load");
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
        session.terrain_frame_assets = Some(assets);
        with_registry_mut(|registry| registry.insert(session))
            .expect("complete owner-cache fixture session registers")
    }

    fn open_fixture_with_world_frame_assets() -> u64 {
        let (package, closure) = fixture_paths();
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load_with_owner_cache(
            package,
            closure,
            private.join("generated/level1-unity-asset-cache"),
        )
        .expect("package M and qualified owner cache load");
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
        assert_eq!(size_of::<McGoatSnapshotV2>(), 84);
        assert_eq!(size_of::<McGoatBatchV2>(), 64);
        assert_eq!(size_of::<McLevel1MissionSnapshotV2>(), 96);
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
        assert_eq!(size_of::<McAudioCommandV2>(), 88);
        assert_eq!(size_of::<McPresentationStateV2>(), 64);
        assert_eq!(size_of::<McRenderCameraStateV2>(), 72);
        assert_eq!(size_of::<McTerrainPnt5VectorV2>(), 56);
        assert_eq!(size_of::<McTerrainGridVertexV2>(), 64);
        assert_eq!(size_of::<McTerrainFrameQueryV2>(), 40);
        assert_eq!(size_of::<McTerrainFrameInfoV2>(), 96);
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
        assert_eq!(std::mem::offset_of!(McGoatSnapshotV2, life), 48);
        assert_eq!(std::mem::offset_of!(McGoatSnapshotV2, reserved), 68);
        assert_eq!(std::mem::offset_of!(McGoatBatchV2, reserved), 48);
        assert_eq!(
            std::mem::offset_of!(McLevel1MissionSnapshotV2, player_life),
            40
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
#include "mc_sim.h"

_Static_assert(sizeof(McWorldInfoV2) == 72, "McWorldInfoV2 size");
_Static_assert(sizeof(McCommandV2) == 32, "McCommandV2 size");
_Static_assert(sizeof(McCameraInputV2) == 28, "McCameraInputV2 size");
_Static_assert(sizeof(McSessionSnapshotV2) == 120, "McSessionSnapshotV2 size");
_Static_assert(sizeof(McObjectiveSnapshotV2) == 40, "McObjectiveSnapshotV2 size");
_Static_assert(sizeof(McEntitySnapshotV2) == 52, "McEntitySnapshotV2 size");
_Static_assert(sizeof(McGoatSnapshotV2) == 84, "McGoatSnapshotV2 size");
_Static_assert(sizeof(McGoatBatchV2) == 64, "McGoatBatchV2 size");
_Static_assert(sizeof(McLevel1MissionSnapshotV2) == 96, "McLevel1MissionSnapshotV2 size");
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
_Static_assert(sizeof(McAudioCommandV2) == 88, "McAudioCommandV2 size");
_Static_assert(sizeof(McPresentationStateV2) == 64, "McPresentationStateV2 size");
_Static_assert(sizeof(McRenderCameraStateV2) == 72, "McRenderCameraStateV2 size");
_Static_assert(sizeof(McTerrainPnt5VectorV2) == 56, "McTerrainPnt5VectorV2 size");
_Static_assert(sizeof(McTerrainGridVertexV2) == 64, "McTerrainGridVertexV2 size");
_Static_assert(sizeof(McTerrainFrameQueryV2) == 40, "McTerrainFrameQueryV2 size");
_Static_assert(sizeof(McTerrainFrameInfoV2) == 96, "McTerrainFrameInfoV2 size");
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
_Static_assert(offsetof(McGoatSnapshotV2, life) == 48, "Goat life offset");
_Static_assert(offsetof(McGoatSnapshotV2, reserved) == 68, "Goat reserved offset");
_Static_assert(offsetof(McGoatBatchV2, reserved) == 48, "Goat batch reserved offset");
_Static_assert(offsetof(McLevel1MissionSnapshotV2, player_life) == 40, "Level 1 mission life offset");
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
                local_audio_command_record(&synthetic_event, 29, 0, 3,).loop_count,
                0
            );
            Ok(())
        })
        .unwrap();
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
        assert_eq!(command.voice_index, 1);
        with_session(handle, |session| {
            assert_eq!(
                session.audio_voices[0].map(|voice| voice.sound_id),
                Some(2),
                "the local ambient loop owns shared voice zero first"
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

            session.audio_commands.insert(3, command(3, 39, 30, 1_000));
            session.audio_commands.insert(4, command(4, 39, 31, 991));
            session.drain_pending_audio_commands();
            assert_eq!(session.audio_commands[&3].action, MC_AUDIO_ACTION_START);
            assert_eq!(
                session.audio_commands[&4].action,
                MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED
            );

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

            session.reset();
            assert!(session.audio_voices.iter().all(Option::is_none));
            assert!(session.audio_commands.is_empty());
            Ok(())
        })
        .unwrap();
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
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn top_hud_uses_shared_holder_tier_while_menu_preserves_per_hand_tiers() {
        let handle = open_fixture();
        with_session_mut(handle, |session| {
            let fire =
                session.context.spell_fire.as_mut().ok_or_else(|| {
                    AbiError::new(MC_STATUS_VALIDATION_FAILED, "spell state absent")
                })?;
            fire.left_holder_index = 154;
            fire.right_holder_index = 154;
            fire.selected_holder_index = 154;
            fire.selected_spell_ui_index = 1;
            fire.left_subspell_index = 2;
            fire.right_subspell_index = 1;
            fire.selected_subspell_index = 1;
            fire.spell_levels[1] = 2;
            fire.holders
                .iter_mut()
                .find(|holder| holder.entity_index == 154)
                .ok_or_else(|| {
                    AbiError::new(MC_STATUS_VALIDATION_FAILED, "possession holder absent")
                })?
                .level_gate = 1;
            Ok(())
        })
        .unwrap();

        let mut hud = McLevel1HudSnapshotV2::default();
        let mut menu = McLevel1SpellMenuSnapshotV2::default();
        assert_eq!(
            mc_native_level1_hud_snapshot(
                handle,
                &raw mut hud,
                struct_size::<McLevel1HudSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_level1_spell_menu_snapshot(
                handle,
                &raw mut menu,
                struct_size::<McLevel1SpellMenuSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!((hud.left.model, hud.left.tier), (1, 1));
        assert_eq!((hud.right.model, hud.right.tier), (1, 1));
        assert_eq!((hud.selected_model, hud.selected_tier), (1, 1));
        assert_eq!((menu.left_family, menu.left_tier), (1, 2));
        assert_eq!((menu.right_family, menu.right_tier), (1, 1));
        assert_eq!(menu.family_selected_tier[1], 1);
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
        assert_eq!(mission.possession_experience, 1);
        assert_eq!(mission.possession_tier, 0);
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
        let writes = with_session_mut(handle, |session| {
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
                    .all(|impact| !impact.active)
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
        assert_eq!(snapshot.objective_textbox_ticks, 33);
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
            114
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
        let first_session = session_snapshot(first);
        let second_session = session_snapshot(second);
        let (reset_rows, reset_batch) = goats(first, first_session.generation);
        let (second_rows, second_batch) = goats(second, second_session.generation);
        assert_eq!(reset_rows, second_rows);
        assert_eq!(reset_batch.live_world_hash, second_batch.live_world_hash);
        assert_eq!(reset_batch.tick, 0);
        assert_eq!(reset_rows[0].entity_index, 50);
        assert_eq!(reset_rows[0].authority, MC_AUTHORITY_GOAT_LIVE);
        assert_eq!(reset_rows[0].render_parameter_index, 238);
        assert_eq!(reset_rows[0].base_tmap, 152);
        assert_eq!(reset_rows[0].life, 600);
        assert_eq!(reset_rows[0].max_life, 600);

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
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let expected =
            fs::read(private.join("generated/level1-unity-asset-cache/palette/paln.rgb8")).unwrap();
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
    fn source_projection_hash_normalizes_only_the_additive_session_tick() {
        let handle = open_fixture();
        let command = valid_command();
        let mut snapshot = McSessionSnapshotV2::default();
        for _ in 0..8 {
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
        assert_eq!(snapshot.state_hash, 0x061b_7ffc_8213_1a7e);
        assert_eq!(projected_hash, 0x0e45_47b4_2a98_c976);
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
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("package M loads");
        let mut direct = world.session();
        let handle = open_fixture();
        let command = valid_command();
        let mut snapshot = McSessionSnapshotV2::default();

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
                    direct.live_world.hash_v1(),
                    capi.live_world.hash_v1(),
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
                    direct.live_world.hash_v1(),
                    capi.live_world.hash_v1(),
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
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn replaceable_fallback_hides_the_stale_generic_entity_projection() {
        let handle = open_fixture();
        let replaced_slot = with_session_mut(handle, |session| {
            session.mission.has_fireball = true;
            session.mission.player_mana = i32::MAX;
            let free_count = session
                .world
                .entities()
                .iter()
                .skip(1)
                .filter(|entity| !entity.active)
                .count();
            for _ in 0..=free_count {
                assert!(session.mission.spawn_fireball(&session.state));
            }
            session
                .world
                .entities()
                .iter()
                .find(|entity| {
                    entity.active
                        && entity.control_flags & sim_core::level1_mission::SOURCE_REPLACEABLE_FLAG
                            != 0
                        && !session.mission.imported_source_slot_is_live(entity.index)
                })
                .map(|entity| entity.index)
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "fallback allocation did not replace an imported source row",
                    )
                })
        })
        .expect("replaceable fallback slot");

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
        let before = with_session_mut(handle, |session| {
            session.mission.player_dead = true;
            session.mission.objective_textbox_ticks = 2;
            Ok(session.snapshot())
        })
        .unwrap();
        let (before_camera, before_look) = with_session(handle, |session| {
            Ok((
                session.camera_history,
                (session.camera_look_yaw, session.camera_look_pitch),
            ))
        })
        .unwrap();

        let mut command = valid_command();
        command.flags = 0x11;
        command.roll = 96;
        command.pitch = -96;
        command.requested_spell = 0;
        let camera = McCameraInputV2 {
            look_yaw: 777,
            look_pitch: 333,
            ..valid_camera_input()
        };
        let mut after = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step_with_camera(
                handle,
                &raw const command,
                &raw const camera,
                &raw mut after,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let (after_camera, after_look) = with_session(handle, |session| {
            Ok((
                session.camera_history,
                (session.camera_look_yaw, session.camera_look_pitch),
            ))
        })
        .unwrap();
        assert_eq!(after.tick, before.tick + 1);
        assert_eq!(
            (after.x, after.y, after.z, after.yaw),
            (before.x, before.y, before.z, before.yaw)
        );
        assert_eq!(
            (
                after.display_pitch,
                after.effective_pitch,
                after.roll_accumulator,
                after.target_speed,
                after.actual_speed,
                after.strafe_speed,
            ),
            (
                before.display_pitch,
                before.effective_pitch,
                before.roll_accumulator,
                before.target_speed,
                before.actual_speed,
                before.strafe_speed,
            )
        );
        assert_eq!(after.fire_sequence, before.fire_sequence);
        assert_eq!(after_camera.x, before_camera.x);
        assert_eq!(after_camera.y, before_camera.y);
        assert_eq!(after_camera.z, before_camera.z);
        assert_eq!(after_camera.yaw, before_camera.yaw);
        assert_eq!(after_camera.pitch, before_camera.pitch);
        assert_eq!(after_camera.fov, before_camera.fov);
        assert_eq!(after_look, before_look);

        let mut mission = McLevel1MissionSnapshotV2::default();
        assert_eq!(
            mc_native_level1_mission_snapshot(
                handle,
                &raw mut mission,
                struct_size::<McLevel1MissionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(mission.player_dead, 1);
        assert_eq!(mission.objective_textbox_ticks, 1);
        assert_eq!(mission.projectile_count, 0);

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
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
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
