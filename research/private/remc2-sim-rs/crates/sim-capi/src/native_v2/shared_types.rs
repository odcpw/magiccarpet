//! Shared ABI constants and revisioned C-layout marshalling types.
//!
//! This family is a pure module move from the native_v2 facade. It owns no
//! runtime state and preserves the public ABI Rust paths through the parent
//! re-export.

use super::*;

pub const MC_NATIVE_ABI_VERSION: u32 = 2;

// Pinned remc2 sub_83B50 fixes the owned game heap at 0x400000 plus
// 0x1000000. Its retained Level-1 source has no writer for the two allocator
// diagnostic globals, so their exact simulation-domain values remain zero.
pub(crate) const LEVEL1_SOURCE_HEAP_SIZE: u32 = 0x0140_0000;
pub(crate) const LEVEL1_SOURCE_MEMORY_USED: u32 = 0;
pub(crate) const LEVEL1_SOURCE_MEMORY_FREE: u32 = 0;

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
/// A source action was valid but temporarily blocked by its gameplay state.
pub const MC_STATUS_SOURCE_ACTION_BLOCKED: i32 = 16;

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
/// One source-slot row projected from the canonical descriptor-backed
/// campaign arena.  This authority is generic: class/model/action remain
/// compatibility payload inside the MC2 adapter and do not select a public
/// Level-N session or ABI.
pub const MC_AUTHORITY_CAMPAIGN_LIVE: u8 = 9;
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
pub const MC_EVENT_CLOSE_HELP_POPUP: u32 = 39;

/// Campaign event kinds (`mc_sim_campaign.h`).
pub const MC_CAMPAIGN_EVENT_SPELL_ACQUISITION: u8 = 2;
pub const MC_CAMPAIGN_EVENT_GOAT: u8 = 9;
/// Campaign economy events (`mc_sim_campaign.h`, kind 10).
pub const MC_CAMPAIGN_EVENT_ECONOMY: u8 = 10;
/// Authored-group activation events (`mc_sim_campaign.h`, kind 11).
pub const MC_CAMPAIGN_EVENT_AUTHORED_GROUP: u8 = 11;

pub const MC_CAMPAIGN_GOAT_BLEAT: u8 = 1;
pub const MC_CAMPAIGN_GOAT_MANA_CONVERTED: u8 = 2;
pub const MC_CAMPAIGN_GOAT_EXTRA_MANA_SPHERE_REQUEST: u8 = 3;
pub const MC_CAMPAIGN_GOAT_REMOVAL_QUEUED: u8 = 4;
pub const MC_CAMPAIGN_GOAT_RECLAIMED: u8 = 5;
pub const MC_CAMPAIGN_GOAT_ENGAGEMENT_BOUNDARY: u8 = 6;

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
pub const MC_CAMPAIGN_ECONOMY_FIRE_SPHERE_COLLISION_SOUND: u8 = 18;
/// Typed source-audio handoff for class-5/model-3 action callbacks.  This
/// remains distinct from the Castle and fire-sphere sound projections.
pub const MC_CAMPAIGN_ECONOMY_AUTHORED_CLASS5_MODEL3_SOUND: u8 = 19;

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
/// The selected owner-bank record is NULL.WAV; no voice or completion.
pub const MC_AUDIO_ACTION_SUPPRESS_NULL_SAMPLE: u8 = 10;

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
/// Classic HD-only receipt bits. These are deliberately outside the legacy
/// 320x200 FINAL MAIN flag namespace: a caller can distinguish a profile-sized
/// rerender from a copied Exact frame without changing the old ABI contract.
pub const MC_CLASSIC_HD_FRAME_PROFILE_RERENDER: u32 = 0x4_0000;
pub const MC_CLASSIC_HD_FRAME_TYPED_SPRITES: u32 = 0x8_0000;
pub const MC_CLASSIC_HD_FRAME_REFLECTIONS: u32 = 0x10_0000;
pub const MC_CLASSIC_HD_FRAME_SHADOWS: u32 = 0x20_0000;
/// The caller requested only the profile-sized, owner-authentic HUD overlay.
/// The native world graph is intentionally not rendered for this receipt.
pub const MC_CLASSIC_HD_FRAME_OWNER_OVERLAY_ONLY: u32 = 0x40_0000;

pub(crate) fn world_frame_flags(
    reflected_sprites_composited: bool,
    sprite_shadows_composited: bool,
) -> u32 {
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

pub(crate) fn source_sprite_disposition_index(disposition: &SourceSpriteDisposition) -> u16 {
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

pub(crate) const EVENT_CAPACITY: usize = 4_096;
pub(crate) const AUDIO_VOICE_COUNT: usize = 10;
pub(crate) const TERRAIN_CELLS: usize = 256 * 256;
pub(crate) const POSITIONAL_QUALIFICATION_VECTOR_COUNT: u32 = 23;
pub(crate) const TERRAIN_PNT5_QUALIFICATION_VECTOR_COUNT: u32 = 13;
// Pinned remc2 6806c720 `Events.cpp:561-605` assigns the NewEvent entity
// parameter row at lines 573 and 599, independently of its particle selector.
pub(crate) const LEVEL1_NEW_EVENT_ENTITY_PARAMETER_RECORD_INDEX: u16 = 59;
pub(crate) const TERRAIN_ATLAS_SHA256: &str =
    "d4d69babd648008aac40413e6089e27bbe32e6b2a5be4311eb10b1aaa33fc5de";
pub(crate) const TABLESN_SHA256: &str =
    "f88974baf68f728fb534a4dab35397b9cb3ca2f424019bbde36858eebbf97495";
pub(crate) const SKYN_SHA256: &str =
    "944112ef6eb54e042889c9cc841ed1d7bdb129c38c1c61ce4c8b8e81589f466b";
pub(crate) const PALN_RGB6_SHA256: &str =
    "6b55d0714b939e8bd25cef89466806299dbae933f2f2f96e0cf8402fc607d16f";

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

/// Generation-bound Level 2 objective stage state.  The legacy objective
/// catalog remains frozen-observed; this additive surface reads the mutable
/// runtime stage and selector state owned by the campaign reset session.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignObjectiveStageSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub objective_slot: u8,
    pub stage_slot: u8,
    pub is_level_end: u8,
    pub done: u8,
    pub text_selector: u8,
    pub stage_state: u8,
    pub stage_kind: i8,
    pub stage_flags: i8,
    pub axis_x: i16,
    pub axis_y: i16,
    pub relation_domain: u8,
    pub reserved_0: u8,
    pub relation_value: u16,
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

/// Additive detailed payload for a kind-11 `McCampaignEventV2` row.  One
/// sidecar row is emitted for each authored index, preserving the source
/// order without placing a variable-length array in the C ABI.  `ordinal`
/// joins the sidecar to the generic event, while `event_index` identifies the
/// authored-index position within that activation.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignAuthoredGroupEventV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub ordinal: u32,
    pub pass: u8,
    pub flags: u8,
    pub group: u16,
    pub source_slot: u16,
    pub event_index: u16,
    pub authored_index: u16,
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

/// Additive detailed payload for a kind-9 `McCampaignEventV2` row. `ordinal`
/// joins the sidecar to the generic Goat dispatch row, and `event_index`
/// identifies which `GoatStepEvent` within that row produced this payload.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignGoatEventV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub ordinal: u32,
    pub pass: u8,
    pub family: u8,
    pub code: u8,
    pub flags: u8,
    pub source_slot: u16,
    pub event_index: u16,
    pub entity_0: u16,
    pub entity_1: u16,
    pub value_0: i32,
    pub value_1: i32,
    pub sound_id: u16,
    pub action: u8,
    pub reserved_0: u8,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub asset_ref_index: u32,
    pub reserved: [u32; 2],
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

/// Generation-bound mutable terrain metadata for a Level 2 campaign handle.
/// This is deliberately a distinct record from the sealed Level 1 terrain
/// state: clients must not accidentally pair a Level 2 reset with the legacy
/// session path.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignTerrainStateV2 {
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

/// Generation/tick-bound qualified Level 2 Day indexed world-frame metadata.
/// The two SHA-256 fields bind the returned bytes to the validated package and
/// Day material cache; the remaining hashes bind the deterministic render
/// inputs and output without exposing a second renderer to managed callers.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignDayIndexedFrameInfoV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub width: u16,
    pub height: u16,
    pub stride: u16,
    pub reserved_0: u16,
    pub index_bytes: u32,
    pub palette_bytes: u32,
    pub index_hash: u64,
    pub palette_hash: u64,
    pub terrain_input_hash: u64,
    pub terrain_command_hash: u64,
    pub package_sha256: [u8; 32],
    pub cache_content_sha256: [u8; 32],
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

/// Read-only source/package sprite identity for one live campaign entity.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignEntityPresentationV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub entity_index: u16,
    pub source_id: u16,
    pub render_parameter_index: u16,
    pub base_tmap: u16,
    pub selected_tmap: u16,
    pub logical_height: u16,
    pub render_mode: u8,
    pub orientation_sector: u8,
    pub mirrored: u8,
    pub reserved_0: u8,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub yaw: i16,
    pub pixel_mode: u8,
    pub owner_color: u8,
    pub brightness: u16,
    pub draw_order: u32,
    pub control_flags: u32,
    pub primary_depth_adjustment: i16,
    pub reserved: u16,
}

/// Additive family-6 Shield state projection.  The row is generation-bound
/// like the other Level 2 campaign snapshots; `active == 0` is the qualified
/// reset/no-holder state rather than an error.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignShieldSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub active: u8,
    pub tier: u8,
    pub action: u8,
    pub reserved_0: u8,
    pub holder_entity_index: u16,
    pub parent_entity_index: u16,
    pub active_value: i16,
    pub activation_value: i16,
    pub countdown: i16,
    pub reserved_1: i16,
    pub mana_cost: i32,
    pub parent_mana: i32,
    pub parent_life: i32,
    pub parent_mana_regen: i32,
    pub parent_status_flags: u32,
    pub experience_awarded: i32,
    pub reserved: [u32; 4],
}

/// Additive family-11 Invisible state projection for Level 2 campaign
/// clients.  `active == 0` is the no-holder reset state.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignInvisibleSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub active: u8,
    pub tier: u8,
    pub action: u8,
    pub parent_visibility_mode: u8,
    pub holder_entity_index: u16,
    pub parent_entity_index: u16,
    pub active_value: i16,
    pub activation_value: i16,
    pub countdown: i16,
    pub reserved_0: i16,
    pub mana_cost: i32,
    pub mana_regen_requirement: i32,
    pub parent_mana: i32,
    pub parent_life: i32,
    pub parent_mana_regen: i32,
    pub parent_status_flags: u32,
    pub spell_status_flags: u32,
    pub experience_awarded: i32,
    pub cleanup_requested: u8,
    pub reserved_1: [u8; 3],
    pub reserved: [u32; 3],
}

/// Additive family-12 Beyond Sight state projection for Level 2 campaign
/// clients. `active == 0` is the no-holder reset state. Beyond Sight owns no
/// parent status bits; its pulse/effect arena remains Rust-owned.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignBeyondSightSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub active: u8,
    pub tier: u8,
    pub action: u8,
    pub reserved_0: u8,
    pub holder_entity_index: u16,
    pub parent_entity_index: u16,
    pub active_value: i16,
    pub activation_value: i16,
    pub countdown: i16,
    pub reserved_1: i16,
    pub mana_cost: i32,
    pub mana_regen_requirement: i32,
    pub parent_mana: i32,
    pub parent_life: i32,
    pub parent_mana_regen: i32,
    pub experience_awarded: i32,
    pub cleanup_requested: u8,
    pub reserved_2: [u8; 3],
    pub reserved: [u32; 5],
}

/// Additive Level 2 Nyphur participant/holder-chain projection. The snapshot
/// is generation-bound and reset-owned; live opponent AI decisions remain a
/// separate dispatcher surface.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignOpponentSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub active: u8,
    pub player_slot: u8,
    pub is_ai: u8,
    pub color_index: u8,
    pub entity_index: u16,
    pub entity_class: u8,
    pub entity_model: u8,
    pub entity_action: u8,
    pub reserved_0: u8,
    pub entity_source_id: u16,
    pub entity_next_relation: u16,
    pub entity_next_entity: u16,
    pub reserved_1: u16,
    pub entity_control_flags: u32,
    pub entity_life: i32,
    pub entity_max_life: i32,
    pub entity_mana: i32,
    pub entity_max_mana: i32,
    pub fireball_holder_index: u16,
    pub fireball_action: u8,
    pub reserved_2: u8,
    pub fireball_parent_entity_index: u16,
    pub fireball_next_entity: u16,
    pub possession_holder_index: u16,
    pub possession_action: u8,
    pub reserved_3: u8,
    pub possession_parent_entity_index: u16,
    pub possession_next_entity: u16,
    /// RuntimeRelation domain: 2 means LiveEntity, 0 means no relation.
    pub objective_relation_domain: u8,
    pub reserved_4: [u8; 3],
    pub objective_relation_value: u16,
    pub reserved_5: u16,
    /// Bit 0: visible to the local player. Bit 1: owner Invisible mode is
    /// nonzero. Bit 2: a local Beyond Sight holder is present. Bit 3: that
    /// holder is active. Bit 4: the target's family-4 holder gate is active.
    pub visibility_flags: u32,
    pub reserved: [u32; 4],
}

/// Additive generation/tick-bound Level 2 Day HUD spell-hand row. The
/// selectors are source-qualified values; managed presentation validates them
/// against the owner-data roots before drawing.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignDayHudSpellSlotV2 {
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
    pub draw_admitted: u8,
    pub help_unavailable: u8,
    pub reserved_padding: u8,
    pub reserved: [u32; 2],
}

/// Minimal authoritative Day status-bar, spell-hand, and Nyphur visibility
/// projection. The nested opponent row remains the existing typed Nyphur
/// participant contract; this outer record binds it and the HUD rows to one
/// campaign generation and simulation tick.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignDayHudSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub local_player_slot: u8,
    pub local_player_active: u8,
    pub local_player_color_index: u8,
    pub hit_frame_time_pre_draw: u8,
    pub local_player_entity_index: u16,
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
    pub reserved_0: u8,
    pub nyphur: McCampaignOpponentSnapshotV2,
    pub left: McCampaignDayHudSpellSlotV2,
    pub right: McCampaignDayHudSpellSlotV2,
}

/// Generation/tick-bound logical Day music lifecycle. The qualified owner
/// cache carries XMI, not PCM; this is the source control state and never an
/// assertion that the native client can decode the track.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignDayMusicSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub asset_slot: u32,
    pub start_count: u32,
    pub update_count: u32,
    pub fade_callback_count: u32,
    pub stop_count: u32,
    pub end_count: u32,
    pub initialized: u8,
    pub loaded: u8,
    pub active: u8,
    pub playing: u8,
    pub track: u8,
    pub volume: u8,
    pub updates_enabled: u8,
    pub timer_active: u8,
    pub target_mode: u8,
    pub direction: i8,
    pub current_units: u8,
    pub timer_multiplier: u8,
    pub timer_hz: u16,
    pub reserved_0: u16,
    pub state_hash: u64,
    pub reserved: [u32; 2],
}

/// Generation/tick-bound summary of the authoritative Level 2 minimap
/// projection. Entity and mark rows are copied through the two typed bulk
/// functions below; this record binds both arrays to one Rust plan.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignMinimapStateV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub local_player_entity_index: u16,
    pub reserved_0: u16,
    pub entity_count: u32,
    pub mark_count: u32,
    pub entity_hash: u64,
    pub mark_hash: u64,
    pub visibility_flags: u32,
    pub reserved: [u32; 3],
}

/// One source-ordered entity admitted to the Level 2 minimap.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignMinimapEntityV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub source_order: u16,
    pub subject_entity_index: u16,
    pub x: i16,
    pub y: i16,
    pub selector: u16,
    pub palette_index: u8,
    pub kind: u8,
    pub size: u8,
    pub anchor: u8,
    pub help_selector: u8,
    pub visible: u8,
    pub reserved: [u32; 3],
}

/// Campaign minimap command: kind 2 rectangle, kind 3 arrow-outline vertex.
/// An arrow has seven consecutive rows, joined and closed. Source order remains
/// strictly increasing across all commands.
/// Coordinates use the existing 128-unit map space; visibility is Rust-owned.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignMinimapMarkV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub source_order: u16,
    pub subject_entity_index: u16,
    pub x: i16,
    pub y: i16,
    pub palette_index: u8,
    pub kind: u8,
    pub help_selector: u8,
    pub visible: u8,
    pub reserved: [u32; 3],
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

/// Additive Level 2 campaign Goat row. This preserves the sealed 84-byte Goat
/// ABI while reusing the first reserved slot for the authored action-11
/// partner link.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignGoatSnapshotV2 {
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
    pub partner_entity_index: u16,
    pub reserved_1: u16,
    pub reserved_2: [u32; 3],
}

/// Additive Level 2 authored spell-pickup row. The row is generation-bound
/// by the accompanying `McCampaignManaBatchV2` header and exposes the native
/// source position needed to drive acquisition through the real campaign
/// step route. Active Shield/Invisible/Beyond Sight lifecycles remain
/// separate state projections.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCampaignSpellPickupSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub source_entity_index: u16,
    pub authored_index: u16,
    pub family: u8,
    pub action: u8,
    pub phase: u8,
    pub active: u8,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub entity_rng: u16,
    pub reserved_0: u16,
    pub reserved: [u32; 9],
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
    /// Attraction source, packed radial words, and current source word94
    /// player relation. The latter can differ from `owner_entity_index`.
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
