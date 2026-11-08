#ifndef MC_SIM_H
#define MC_SIM_H

/*
 * Stable C surface for the private Magic Carpet simulation lab.
 *
 * mc_sim_* is the legacy ABI v1 scalar/single-thread adapter. Production
 * builds contain no owner fixture bytes: callers must set
 * MC_LEGACY_STATE_SEQUENCE and MC_LEGACY_TERRAIN_SEQUENCE to explicit private
 * fixture paths before first use. Missing, unreadable, or invalid fixtures are
 * contained at the ABI boundary and return each function's typed zero
 * sentinel.
 *
 * mc_native_* is ABI v2. It loads caller-supplied, validated owner packages,
 * returns opaque session handles, and supports independent concurrent
 * sessions. UTF-8 path arguments are byte spans and need not be NUL
 * terminated. Returned strings are UTF-8 and are NUL terminated by copy APIs.
 */

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define MC_NATIVE_ABI_VERSION 2u

#define MC_STATUS_OK 0
#define MC_STATUS_NULL_POINTER 1
#define MC_STATUS_INVALID_UTF8 2
#define MC_STATUS_INVALID_ARGUMENT 3
#define MC_STATUS_UNSUPPORTED_VERSION 4
#define MC_STATUS_STRUCT_TOO_SMALL 5
#define MC_STATUS_RESERVED_NONZERO 6
#define MC_STATUS_IO_ERROR 7
#define MC_STATUS_VALIDATION_FAILED 8
#define MC_STATUS_UNKNOWN_HANDLE 9
#define MC_STATUS_BOUNDS 10
#define MC_STATUS_BUFFER_TOO_SMALL 11
#define MC_STATUS_GENERATION_MISMATCH 12
#define MC_STATUS_CURSOR_EXPIRED 13
#define MC_STATUS_INTERNAL_PANIC 14
#define MC_STATUS_LOCK_FAILED 15

#define MC_PROFILE_OUTDOOR 0u
#define MC_PROFILE_CAVE 1u

#define MC_AUTHORITY_INACTIVE 0u
#define MC_AUTHORITY_FROZEN_OBSERVED 1u
#define MC_AUTHORITY_PLAYER_FLIGHT_LIVE 2u
#define MC_AUTHORITY_SPELL_ACTIVATION_LIVE 3u
#define MC_AUTHORITY_GOAT_LIVE 4u
#define MC_AUTHORITY_SELECTOR_FRAME_LIVE 5u
#define MC_AUTHORITY_LEVEL1_MISSION_LIVE 6u
#define MC_AUTHORITY_SESSION_TERRAIN_LIVE 7u
#define MC_AUTHORITY_VILLAGER_LIVE 8u
#define MC_LEVEL1_HOUSE_COMPLETION_ALLOCATED 1u
#define MC_LEVEL1_HOUSE_COMPLETION_ALLOCATION_FAILED 2u
#define MC_LEVEL1_EXIT_FLAG_PALETTE_VISUAL_CURRENT 0x01u
#define MC_LEVEL1_EXIT_FLAG_FOV_POST_ACTION_NEXT_HISTORY 0x02u
#define MC_LEVEL1_EXIT_FLAG_END_SEQUENCE 0x04u
#define MC_LEVEL1_GAP_ALLOCATION_ONLY_ACTORS (1u << 0)
#define MC_LEVEL1_GAP_CASTLE_BUILD_FOOTPRINT (1u << 1)
#define MC_LEVEL1_GAP_CASTLE_CONSTRUCTION_TIMING (1u << 2)
#define MC_LEVEL1_GAP_MANA_SPHERE_ALLOCATION_RNG (1u << 3)
#define MC_LEVEL1_GAP_MANA_CARRIER_FLIGHT (1u << 4)
#define MC_LEVEL1_GAP_MISSION_SOUND_POSITIONING (1u << 5)
#define MC_LEVEL1_GAP_EXIT_COLLISION_AND_FADE (1u << 6)
#define MC_LEVEL1_GAP_SECONDARY_GROUP_33_34_CREATURES (1u << 7)
#define MC_LEVEL1_CURRENT_FIDELITY_GAPS 0u

#define MC_SELECTOR_FRAME_ANIMATED 0x0001u
#define MC_SELECTOR_FRAME_GROUPED_UPDATE 0x0002u
#define MC_SELECTOR_FRAME_USED_BY_PRECEDING_DRAW 0x0004u
#define MC_SELECTOR_FRAME_PIXELS_MUTABLE 0x0008u

#define MC_TERRAIN_TYPES_U8 0u
#define MC_TERRAIN_HEIGHT_U8 1u
#define MC_TERRAIN_SHADE_U8 2u
#define MC_TERRAIN_ANGLE_U8 3u
#define MC_TERRAIN_SECONDARY_U8 4u
#define MC_TERRAIN_ENTITY_INDEX_I16 5u

#define MC_WORLD_STRING_SCHEMA 0u
#define MC_WORLD_STRING_PACKAGE_SHA256 1u
#define MC_WORLD_STRING_CLOSURE_SHA256 2u
#define MC_WORLD_STRING_STATE_HASH_SCHEMA 3u
#define MC_WORLD_STRING_CONFORMANCE_HASH_SCHEMA 4u
#define MC_WORLD_STRING_CONTEXT_SEAM 5u

#define MC_ASSET_STRING_ID 0u
#define MC_ASSET_STRING_CONTENT_ADDRESS 1u
#define MC_ASSET_STRING_CATEGORY 2u
#define MC_ASSET_STRING_HOOK 3u
#define MC_ASSET_STRING_STAGE 4u
#define MC_ASSET_STRING_OWNER_ID 5u

#define MC_EVENT_MOVEMENT_RESOLUTION 0u
#define MC_EVENT_PREPARE_SOUND 1u
#define MC_EVENT_END_LOOP 2u
#define MC_EVENT_UPDATE_MUSIC 3u
#define MC_EVENT_REQUEST_SPELL_ACTIVATION 4u
#define MC_EVENT_SHOW_HELP_HINT 5u
#define MC_EVENT_START_MUSIC 6u
#define MC_EVENT_GOAT_SOUND_REQUEST 7u
#define MC_EVENT_GOAT_MANA_CONVERTED 8u
#define MC_EVENT_GOAT_EXTRA_MANA_SPHERE_REQUEST 9u
#define MC_EVENT_GOAT_REMOVAL_QUEUED 10u
#define MC_EVENT_GOAT_RECLAIMED 11u
#define MC_EVENT_GOAT_ENGAGEMENT_BOUNDARY 12u
#define MC_EVENT_LEVEL1_OBJECTIVE_ADVANCED 13u
#define MC_EVENT_LEVEL1_SPELL_COLLECTED 14u
#define MC_EVENT_LEVEL1_FIREBALL_SPAWNED 15u
#define MC_EVENT_LEVEL1_FIREBALL_IMPACTED 16u
#define MC_EVENT_LEVEL1_ACTOR_DAMAGED 17u
#define MC_EVENT_LEVEL1_ACTOR_KILLED 18u
#define MC_EVENT_LEVEL1_CASTLE_BUILT 19u
#define MC_EVENT_LEVEL1_CASTLE_MANA_CHANGED 20u
#define MC_EVENT_LEVEL1_PLAYER_DAMAGED 21u
#define MC_EVENT_LEVEL1_PLAYER_DIED 22u
#define MC_EVENT_LEVEL1_COMPLETED 23u
#define MC_EVENT_LEVEL1_ARCHER_ARROW_SPAWNED 24u
#define MC_EVENT_LEVEL1_ARCHER_ARROW_SOUND 25u
#define MC_EVENT_LEVEL1_FIREFLY_FIREBALL_SPAWNED 26u
#define MC_EVENT_LEVEL1_FIREFLY_CHARGE_SOUND 27u
#define MC_EVENT_LEVEL1_CASTLE_PROJECTILE_SPAWNED 28u
#define MC_EVENT_LEVEL1_CASTLE_PLACEMENT_REJECTED 29u
#define MC_EVENT_LEVEL1_CASTLE_CONSTRUCTION_SOUND 30u
#define MC_EVENT_LEVEL1_POSSESSION_SPAWNED 31u
#define MC_EVENT_LEVEL1_MANA_SPHERE_SPAWNED 32u
#define MC_EVENT_LEVEL1_MANA_SPHERE_CLAIMED 33u
#define MC_EVENT_LEVEL1_MANA_SPHERE_COLLECTED 34u
#define MC_EVENT_LEVEL1_EXIT_SEQUENCE_STARTED 35u
#define MC_EVENT_LEVEL1_EXIT_SEQUENCE_COMPLETED 36u
#define MC_EVENT_LEVEL1_WATER_SPLASHED 37u
#define MC_EVENT_LEVEL1_ABILITY_IMPROVED 38u

#define MC_AUDIO_ACTION_PREPARED_PENDING 0u
#define MC_AUDIO_ACTION_START 1u
#define MC_AUDIO_ACTION_RESTART 2u
#define MC_AUDIO_ACTION_SUPPRESS_EXISTING 3u
#define MC_AUDIO_ACTION_SUPPRESS_NO_VOICE 4u
#define MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME 5u
#define MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED 6u
#define MC_AUDIO_ACTION_UPDATE_EXISTING 7u
#define MC_AUDIO_ACTION_END_LOOP 8u
#define MC_AUDIO_ACTION_SUPPRESS_NO_EXISTING 9u
/* The selected owner-bank record is NULL.WAV; no voice or completion. */
#define MC_AUDIO_ACTION_SUPPRESS_NULL_SAMPLE 10u
#define MC_AUDIO_VOICE_NONE UINT16_MAX
#define MC_PRESENTATION_CAMERA_EXACT_V2_INPUTS 0x01u
#define MC_PRESENTATION_CAMERA_EYE_Z_PLUS_128 0x02u
#define MC_TERRAIN_FRAME_ISOLATED_SKY_TERRAIN 0x0001u
#define MC_TERRAIN_FRAME_SKY_SEEDED 0x0002u
#define MC_TERRAIN_FRAME_HAS_COVERAGE 0x0004u
#define MC_TERRAIN_FRAME_NON_TERRAIN_OMITTED 0x0008u
#define MC_TERRAIN_FRAME_GOATS_COMPOSITED 0x0010u
#define MC_TERRAIN_FRAME_ARCHERS_COMPOSITED 0x0020u
#define MC_TERRAIN_FRAME_FIREFLIES_COMPOSITED 0x0040u
#define MC_TERRAIN_FRAME_COMBAT_COMPOSITED 0x0080u
#define MC_TERRAIN_FRAME_OBJECTIVE_PROPS_COMPOSITED 0x0100u
#define MC_TERRAIN_FRAME_ECONOMY_COMPOSITED 0x0200u
#define MC_TERRAIN_FRAME_REFLECTED_SPRITES_COMPOSITED 0x0400u
#define MC_TERRAIN_FRAME_SPRITE_SHADOWS_COMPOSITED 0x0800u
#define MC_TERRAIN_FRAME_FROZEN_ACTORS_COMPOSITED 0x1000u
#define MC_PRESENTATION_FRAME_COMPOSITOR_COMPLETE 0x2000u
#define MC_PRESENTATION_FRAME_HAS_PROVENANCE 0x4000u
#define MC_PRESENTATION_FRAME_OVERLAYS_COMPOSITED 0x8000u
#define MC_PRESENTATION_FRAME_POSTPROCESS_COMPLETE 0x10000u
#define MC_PRESENTATION_FRAME_LIGHTING_COMPLETE 0x20000u

#define MC_ASSET_REF_NONE UINT32_MAX
#define MC_PARAMETER_RECORD_NONE UINT16_MAX
#define MC_ENTITY_INDEX_NONE UINT16_MAX
#define MC_REQUESTED_SPELL_NONE (-1)

typedef struct McWorldInfoV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint16_t external_level;
    uint16_t internal_level;
    uint8_t map_type;
    uint8_t profile;
    uint16_t reserved_0;
    uint32_t entity_count;
    uint32_t active_entity_count;
    uint32_t player_count;
    uint32_t asset_ref_count;
    uint16_t terrain_width;
    uint16_t terrain_height;
    uint64_t reset_state_hash;
    uint64_t reset_conformance_hash;
    uint32_t reserved[4];
} McWorldInfoV2;

typedef struct McCommandV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    int8_t opcode;
    int8_t argument_1;
    int8_t argument_2;
    int8_t roll;
    int8_t pitch;
    int8_t flags;
    int16_t requested_spell;
    uint32_t reserved[4];
} McCommandV2;

/*
 * Additive tail for the two 16-bit view fields in the original ten-byte
 * player input record. McCommandV2 remains byte-for-byte stable.
 */
typedef struct McCameraInputV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint16_t look_yaw;
    uint16_t look_pitch;
    uint32_t reserved[4];
} McCameraInputV2;

typedef struct McSessionSnapshotV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t tick;
    uint64_t event_head;
    uint64_t state_hash;
    uint64_t conformance_hash;
    uint16_t entity_index;
    uint16_t x;
    uint16_t y;
    int16_t z;
    int16_t yaw;
    int16_t display_pitch;
    int16_t effective_pitch;
    int16_t roll_accumulator;
    int16_t target_speed;
    int16_t actual_speed;
    int16_t strafe_speed;
    uint8_t phase;
    uint8_t last_resolution;
    uint8_t profile;
    uint8_t local_player_slot;
    uint8_t left_spell_slot;
    uint8_t right_spell_slot;
    uint8_t selected_spell_ui_index;
    /*
     * Reset-observed remaining objective-textbox display ticks; not an
     * objective selector and not advanced by the current ABI v2 simulation.
     */
    uint8_t objective_textbox_ticks;
    uint8_t objective_count;
    /* Governs the countdown and all rows; FrozenObserved in ABI v2. */
    uint8_t objectives_authority;
    uint8_t reserved_1[2];
    uint16_t left_spell_holder;
    uint16_t right_spell_holder;
    uint16_t selected_spell_holder;
    uint16_t reserved_0;
    int32_t caster_mana;
    /* Frame-zero map music selector; not live music-system state. */
    int32_t maptype_music;
    uint32_t fire_sequence;
    uint32_t reserved[4];
} McSessionSnapshotV2;

typedef struct McObjectiveSnapshotV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint8_t slot;
    uint8_t is_level_end;
    uint8_t text_selector;
    uint8_t done;
    /* Always MC_AUTHORITY_FROZEN_OBSERVED in ABI v2. */
    uint8_t authority;
    uint8_t stage_selectors[8];
    uint8_t reserved_0[3];
    uint32_t reserved[4];
} McObjectiveSnapshotV2;

typedef struct McEntitySnapshotV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint16_t entity_index;
    uint16_t parameter_record_index;
    /*
     * Rows are validated reset observations except the PlayerFlightLive row
     * (Level 1 package M entity 152): pose, yaw, display pitch, FOV, speed and
     * phase are projected from the live session. The SpellActivationLive row
     * (package M entity 153) identifies live ownership, but its changing
     * holder timer/action-marker fields are not exposed by this struct;
     * spell events and the atomic session snapshot are the public live view.
     */
    uint8_t authority;
    uint8_t active;
    uint8_t class_id;
    uint8_t model;
    uint8_t action;
    uint8_t phase;
    uint8_t has_parameter_record;
    uint8_t reserved_0;
    uint16_t x;
    uint16_t y;
    int16_t z;
    int16_t yaw;
    int16_t pitch;
    int16_t fov;
    int16_t actual_speed;
    uint32_t reserved[4];
} McEntitySnapshotV2;

typedef struct McGoatSnapshotV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint16_t entity_index;
    uint16_t target_entity_index;
    uint16_t render_parameter_index;
    uint16_t base_tmap;
    uint8_t authority;
    uint8_t active;
    uint8_t class_id;
    uint8_t model;
    uint8_t action;
    uint8_t phase;
    int8_t stage_var_1;
    int8_t stage_var_2;
    int8_t proximity_countdown;
    int8_t proximity_subcounter;
    /* Per-entity reset byte; live selector-global state uses the sidecar. */
    int8_t animation_frame;
    uint8_t reserved_0;
    uint16_t x;
    uint16_t y;
    int16_t z;
    int16_t yaw;
    int16_t pitch;
    int16_t roll;
    int16_t actual_speed;
    int16_t minimum_speed;
    int16_t maximum_speed;
    uint16_t rng;
    int32_t life;
    int32_t max_life;
    int32_t mana;
    int32_t pending_damage;
    uint32_t control_flags;
    uint32_t reserved[4];
} McGoatSnapshotV2;

typedef struct McGoatBatchV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t tick;
    uint64_t live_world_hash;
    uint32_t start_ordinal;
    uint32_t written;
    uint32_t total_count;
    uint8_t has_more;
    uint8_t reserved_0[3];
    uint32_t reserved[4];
} McGoatBatchV2;

typedef struct McLevel1MissionSnapshotV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t tick;
    uint8_t authority;
    uint8_t current_stage;
    uint8_t text_selector;
    uint8_t objective_textbox_ticks;
    uint8_t tasks_completed;
    uint8_t player_dead;
    uint8_t has_fireball;
    uint8_t has_possession;
    uint8_t has_castle_spell;
    uint8_t has_speed_spell;
    uint8_t castle_exists;
    uint8_t castle_level;
    uint8_t fidelity_gap_flags;
    uint8_t exit_sequence_active;
    uint8_t exit_sequence_complete;
    uint8_t exit_result_code;
    int32_t player_life;
    int32_t player_max_life;
    int32_t player_mana;
    int32_t player_max_mana;
    int32_t castle_mana;
    int32_t secured_mana_percent;
    uint32_t actor_count;
    uint32_t projectile_count;
    uint32_t impact_count;
    uint32_t exit_sequence_phase;
    int32_t exit_sequence_timer;
    int32_t possession_experience;
    uint16_t last_possessed_building_entity;
    uint16_t last_possessed_building_owner;
    uint16_t possessed_building_count;
    uint8_t possession_tier;
    uint8_t speed_trail_count;
} McLevel1MissionSnapshotV2;

typedef struct McLevel1PlayerLifecycleSnapshotV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t tick;
    uint64_t lifecycle_hash;
    uint64_t source_arena_hash;
    uint8_t authority;
    uint8_t player_action;
    uint8_t player_dead;
    uint8_t input_suppressed;
    uint8_t player_active;
    uint8_t player_visible;
    uint8_t player_targetable;
    uint8_t menu_close_requested;
    uint8_t notification_active;
    uint8_t notification_state;
    uint8_t banished_camera_active;
    uint8_t marker_present;
    int16_t fall_velocity;
    int16_t notification_timer;
    int32_t action_timer;
    int32_t replaceable_cursor;
    uint16_t killer_entity_index;
    uint16_t killer_score_awards;
    uint16_t notification_id;
    uint16_t player_entity_rng;
    uint32_t player_control_flags;
    int16_t player_entity_roll;
    int16_t player_yaw;
    int16_t controller_pitch;
    int16_t controller_roll;
    int16_t recoil_speed;
    uint16_t marker_source_entity_index;
    uint16_t marker_x;
    uint16_t marker_y;
    int16_t marker_z;
    uint16_t marker_trigger_entity_index;
    uint16_t marker_particle_parameter_index;
    int32_t marker_life;
    int32_t marker_max_life;
    uint32_t source_pool_rebuild_count;
    uint64_t lifecycle_trace_hash;
    uint16_t lifecycle_trace_count;
    uint8_t marker_dispatch_phase;
    uint8_t marker_class;
    uint8_t marker_model;
    uint8_t marker_action;
    uint8_t marker_byte_38;
    uint8_t source_color_phase_6;
    uint32_t source_turn_counter;
    uint8_t menu_state;
    uint8_t killer_score_victim_color;
    int16_t effective_pitch;
    uint32_t marker_constructor_phase;
    uint8_t reserved[4];
} McLevel1PlayerLifecycleSnapshotV2;

typedef struct McLevel1HudSpellSlotV2 {
    uint16_t holder_entity_index;
    uint16_t parent_entity_index;
    uint8_t model;
    int8_t tier;
    uint8_t enabled_flags;
    int8_t action_marker;
    int16_t active_value;
    uint16_t activation_value;
    int32_t max_mana;
    int32_t mana_regen;
    int32_t caster_mana;
    uint16_t frame_selector;
    uint16_t icon_selector;
    uint32_t reserved[2];
} McLevel1HudSpellSlotV2;

typedef struct McLevel1HudSnapshotV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t tick;
    uint8_t authority;
    uint8_t mana_color_index;
    uint8_t mana_max_color_index;
    uint8_t life_color_index;
    uint8_t color_phase_1;
    uint8_t color_phase_2;
    uint8_t player_hit_frame_ticks;
    uint8_t castle_hit_frame_ticks;
    int32_t global_mana_denominator;
    int32_t secured_mana_component;
    int32_t goal_percent;
    uint32_t player_status_flags;
    McLevel1HudSpellSlotV2 left;
    McLevel1HudSpellSlotV2 right;
    uint16_t selected_holder_entity_index;
    uint16_t selected_icon_selector;
    uint8_t selected_model;
    int8_t selected_tier;
    uint8_t selected_enabled_flags;
    uint8_t selected_frame_selector;
} McLevel1HudSnapshotV2;

typedef struct McLevel1SpellMenuSnapshotV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t tick;
    uint8_t authority;
    uint8_t selected_ui_index;
    int8_t selected_tier;
    uint8_t left_family;
    int8_t left_tier;
    uint8_t right_family;
    int8_t right_tier;
    uint8_t reserved_0;
    uint8_t family_enabled[4];
    int8_t family_unlocked_tier[4];
    int8_t family_selected_tier[4];
    uint8_t family_hand_eligibility[4];
    uint16_t holder_entity_index[4];
    uint32_t reserved[2];
} McLevel1SpellMenuSnapshotV2;

typedef struct McLevel1MinimapMarksV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t tick;
    uint8_t mark_count;
    uint8_t mark_pulse;
    uint8_t arrow_valid;
    uint8_t arrow_visible;
    int16_t mark_x[16];
    int16_t mark_y[16];
    uint8_t mark_colour[16];
    uint8_t mark_help_selector[16];
    int16_t arrow_xy[14];
    uint32_t reserved[4];
} McLevel1MinimapMarksV2;

typedef struct McLevel1MinimapEntityCommandV2 {
    uint16_t source_slot;
    int16_t x;
    int16_t y;
    uint16_t selector;
    uint8_t kind;
    uint8_t palette_index;
    uint8_t size;
    uint8_t anchor;
    uint8_t help_selector;
    uint8_t flags;
    uint16_t order;
    uint32_t reserved[2];
} McLevel1MinimapEntityCommandV2;

typedef struct McLevel1MinimapEntitiesInfoV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t tick;
    uint32_t total_count;
    uint16_t cross_x;
    uint16_t cross_y;
    uint8_t cross_colour;
    uint8_t cross_arm_length;
    uint8_t reveal_active;
    uint8_t authority;
    uint8_t reserved_0[4];
    uint64_t command_hash;
    uint32_t reserved[4];
} McLevel1MinimapEntitiesInfoV2;

typedef struct McLevel1PaletteInfoV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t tick;
    uint8_t submode;
    uint8_t effect;
    uint8_t weight;
    uint8_t denominator;
    int16_t palette_count;
    uint16_t flags;
    uint8_t rgb8_sha256[32];
    uint32_t reserved[4];
} McLevel1PaletteInfoV2;

typedef struct McLevel1ExitPresentationV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t tick;
    uint8_t authority;
    uint8_t player_action;
    uint8_t action_phase;
    uint8_t palette_submode;
    uint8_t palette_effect;
    uint8_t palette_weight;
    uint8_t palette_denominator;
    uint8_t movement_resolution;
    uint16_t target_source_entity_index;
    uint8_t target_valid;
    uint8_t input_suppressed;
    int16_t camera_fov;
    int16_t action_timer;
    uint8_t player_status_flags;
    uint8_t blur_mode;
    uint8_t display_blank;
    uint8_t result_code;
    uint8_t active;
    uint8_t complete;
    uint8_t flags;
    uint8_t reserved_0;
    uint32_t reserved[4];
} McLevel1ExitPresentationV2;

typedef struct McLevel1CastleSnapshotV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t tick;
    uint8_t authority;
    uint8_t active;
    uint8_t level;
    uint8_t class_id;
    uint8_t model;
    uint8_t action;
    uint8_t render_mode;
    uint8_t reserved_0;
    uint16_t owner_entity_index;
    uint16_t render_parameter_index;
    uint16_t x;
    uint16_t y;
    int16_t z;
    uint16_t base_tmap;
    uint16_t logical_height;
    uint16_t reserved_1;
    int32_t life;
    int32_t max_life;
    int32_t mana;
    int32_t mana_capacity;
    /* Active class-3/model-3 carrier source slot, or zero. */
    uint32_t carrier_id;
    /* reserved[0] = authoritative castle-root remc2 source slot;
       reserved[1] = castle-root presentation identity. */
    uint32_t reserved[3];
} McLevel1CastleSnapshotV2;

typedef struct McTerrainStateV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t tick;
    uint64_t mutation_sequence;
    uint64_t terrain_hash;
    uint16_t width;
    uint16_t height;
    uint8_t plane_count;
    uint8_t hash_schema;
    uint8_t authority;
    uint8_t reserved_0;
    uint32_t reserved[4];
} McTerrainStateV2;

typedef struct McLevel1CastleLifecycleSnapshotV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t tick;
    uint32_t root_presentation_id;
    uint16_t root_source_entity_index;
    uint16_t root_owner_entity_index;
    uint16_t root_x;
    uint16_t root_y;
    int16_t root_z;
    int16_t root_build_state;
    uint8_t root_action;
    uint8_t root_level;
    uint8_t root_active;
    uint8_t authority;
    int32_t root_life;
    uint32_t construction_id;
    uint16_t construction_source_entity_index;
    uint16_t construction_parent_entity_index;
    uint16_t construction_owner_entity_index;
    uint16_t construction_x;
    uint16_t construction_y;
    int16_t construction_z;
    int16_t construction_timer;
    uint8_t construction_stage;
    uint8_t construction_present;
    uint8_t construction_active;
    uint8_t construction_initialized;
    uint8_t reserved_0[2];
    uint64_t terrain_mutation_sequence;
} McLevel1CastleLifecycleSnapshotV2;

typedef struct McLevel1HouseCompletionEffectV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t tick;
    uint32_t kind;
    uint16_t house_entity_index;
    uint16_t parent_entity_index;
    uint16_t source_entity_index;
    uint16_t source_id;
    uint8_t class_id;
    uint8_t model;
    uint8_t action;
    uint8_t construction_selector;
    uint8_t house_building_index;
    uint8_t constructor_latch;
    uint8_t active;
    uint8_t phase;
    int16_t parent_word_2e;
    int16_t parent_construction_selector;
    uint16_t x;
    uint16_t y;
    int16_t z;
    int16_t collision_half_width;
    int16_t collision_half_depth;
    int16_t yaw;
    int16_t fov;
    uint16_t effect_ordinal;
    int32_t max_life;
    int32_t life;
    uint32_t control_flags;
    uint32_t effect_count;
} McLevel1HouseCompletionEffectV2;

typedef struct McManaSphereSnapshotV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint32_t sphere_id;
    uint16_t source_entity_index;
    uint16_t owner_entity_index;
    uint8_t authority;
    uint8_t active;
    uint8_t class_id;
    uint8_t model;
    uint8_t action;
    uint8_t phase;
    uint8_t render_mode;
    uint8_t reserved_0;
    uint16_t x;
    uint16_t y;
    int16_t z;
    int16_t yaw;
    int16_t velocity_x;
    int16_t velocity_y;
    int16_t vertical_velocity;
    uint16_t render_parameter_index;
    uint16_t base_tmap;
    uint16_t logical_height;
    int32_t mana;
    /* Assigned class-3/model-3 carrier source slot, or zero. */
    uint32_t assigned_carrier_id;
    uint32_t reserved[3];
} McManaSphereSnapshotV2;

typedef struct McManaCarrierSnapshotV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t tick;
    uint32_t carrier_id;
    uint32_t target_sphere_id;
    uint16_t owner_entity_index;
    uint16_t render_parameter_index;
    uint8_t authority;
    uint8_t active;
    uint8_t class_id;
    uint8_t model;
    uint8_t action;
    uint8_t returning_to_castle;
    uint8_t render_mode;
    uint8_t reserved_0;
    uint16_t x;
    uint16_t y;
    int16_t z;
    int16_t yaw;
    int16_t speed;
    uint16_t base_tmap;
    uint16_t logical_height;
    uint16_t reserved_1;
    int32_t carried_mana;
    /* carrier_id and reserved[0] are the same authoritative remc2 source
       slot. The duplicate projection is retained for ABI v2 compatibility.
       target_sphere_id is also a source-slot link: zero, a mana sphere, or
       the owning castle while returning. */
    uint32_t reserved[3];
} McManaCarrierSnapshotV2;

typedef struct McLevel1ActorSnapshotV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint16_t authored_index;
    int16_t activation_group;
    uint8_t kind;
    uint8_t authority;
    uint8_t active;
    uint8_t collected;
    uint8_t class_id;
    uint8_t model;
    uint8_t action;
    uint8_t render_mode;
    uint16_t x;
    uint16_t y;
    int16_t z;
    uint16_t render_parameter_index;
    int32_t max_life;
    int32_t life;
    int32_t pending_damage;
    uint16_t base_tmap;
    uint16_t logical_height;
    int16_t yaw;
    int8_t animation_frame;
    uint8_t visible;
    int32_t state_value;
    uint32_t reserved;
} McLevel1ActorSnapshotV2;

typedef struct McLevel1MultipartSegmentSnapshotV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint16_t source_entity_index;
    uint16_t head_authored_index;
    uint16_t head_source_entity_index;
    uint16_t predecessor_source_entity_index;
    uint16_t successor_source_entity_index;
    uint8_t ordinal;
    uint8_t authority;
    uint8_t active;
    uint8_t visible;
    uint8_t class_id;
    uint8_t model;
    uint8_t action;
    uint8_t render_mode;
    uint16_t x;
    uint16_t y;
    int16_t z;
    int16_t yaw;
    int16_t pitch;
    uint8_t phase;
    uint8_t reserved_0;
    int16_t owner_id;
    int32_t life;
    int32_t pending_damage;
    uint16_t render_parameter_index;
    uint16_t base_tmap;
    uint16_t logical_height;
    int16_t collision_half_width;
    int16_t collision_half_height;
    int16_t trailing_distance;
    uint32_t reserved;
} McLevel1MultipartSegmentSnapshotV2;

typedef struct McFireballProjectileSnapshotV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint32_t projectile_id;
    uint16_t owner_entity_index;
    uint16_t render_index;
    uint8_t authority;
    uint8_t active;
    uint8_t class_id;
    uint8_t model;
    uint16_t x;
    uint16_t y;
    int16_t z;
    int16_t yaw;
    int16_t pitch;
    int16_t speed;
    int16_t life_ticks;
    uint16_t base_tmap;
    int32_t payload;
    uint16_t logical_height;
    uint8_t render_mode;
    int8_t animation_frame;
    /* 1 = player Fireball; 2 = Archer arrow; 3 = Firefly fireball;
       4 = Castle build projectile; 5 = Possession projectile. */
    uint32_t projectile_kind;
    /* reserved[0] = authoritative remc2 source slot;
       projectile_id remains presentation identity. */
    uint32_t reserved[2];
} McFireballProjectileSnapshotV2;

typedef struct McFireballImpactSnapshotV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint32_t impact_id;
    uint32_t source_projectile_id;
    uint8_t authority;
    uint8_t active;
    uint8_t damage_submitted;
    uint8_t class_id;
    uint8_t model;
    uint8_t render_mode;
    int8_t animation_frame;
    uint8_t reserved_0;
    uint16_t x;
    uint16_t y;
    int16_t z;
    int16_t life_ticks;
    int32_t payload;
    uint16_t base_tmap;
    uint16_t render_parameter_index;
    uint16_t logical_height;
    uint16_t reserved_1;
    /* reserved[0] = authoritative remc2 source slot;
       impact_id remains presentation identity. */
    uint32_t reserved[2];
} McFireballImpactSnapshotV2;

typedef struct McSelectorFrameStateV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint16_t selector;
    uint8_t mask_bit;
    uint8_t authority;
    uint16_t width;
    uint16_t height;
    uint16_t source_frame_index;
    uint16_t frame_count;
    uint32_t stream_start;
    uint32_t stream_cursor;
    uint32_t payload_bytes;
    uint32_t pixel_bytes;
    uint32_t flags;
    uint64_t pixel_hash;
    uint32_t reserved[4];
} McSelectorFrameStateV2;

typedef struct McSelectorFrameBatchV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t tick;
    uint64_t selector_state_hash;
    uint64_t last_draw_tick;
    uint64_t last_draw_mask;
    uint64_t valid_draw_mask;
    uint32_t start_ordinal;
    uint32_t written;
    uint32_t total_count;
    uint8_t has_more;
    uint8_t has_draw_feedback;
    uint16_t reserved_0;
    uint32_t reserved[4];
} McSelectorFrameBatchV2;

typedef struct McSelectorDrawFeedbackV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t draw_tick;
    uint64_t selector_mask;
    uint32_t reserved[4];
} McSelectorDrawFeedbackV2;

typedef struct McGoatDamageCommandV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint16_t entity_index;
    uint16_t reserved_0;
    int32_t damage;
    uint32_t reserved[4];
} McGoatDamageCommandV2;

typedef struct McAssetRefInfoV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint32_t asset_index;
    uint16_t trace_ordinal;
    uint16_t owner_asset_count;
    uint32_t reserved[4];
} McAssetRefInfoV2;

typedef struct McEventQueryV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t cursor;
    uint32_t reserved[4];
} McEventQueryV2;

typedef struct McLevel1ResidentObservationV1 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint8_t required_mask;
    uint8_t observed_mask;
    uint8_t remaining_mask;
    uint8_t reserved_0;
    uint64_t digest;
    uint32_t reserved[4];
} McLevel1ResidentObservationV1;

typedef struct McEventBatchV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t next_cursor;
    uint64_t event_head;
    uint32_t written;
    uint8_t has_more;
    uint8_t reserved_0[3];
    uint32_t reserved[4];
} McEventBatchV2;

typedef struct McEventV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t sequence;
    uint64_t state_hash;
    uint32_t kind;
    uint32_t code;
    uint16_t entity_index;
    int16_t player_index;
    int32_t value_0;
    int32_t value_1;
    uint32_t asset_ref_index;
    uint32_t reserved[4];
} McEventV2;

/*
 * Additive original-stereo sidecar keyed by generation + event_sequence.
 * McEventV2 remains byte-for-byte stable.
 */
typedef struct McAudioCommandV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t event_sequence;
    uint64_t state_hash;
    uint32_t asset_ref_index;
    int32_t relative_x;
    int32_t relative_y;
    int32_t relative_z;
    uint32_t playback_rate_hz;
    uint16_t sound_id;
    uint16_t entity_index;
    int16_t player_index;
    int16_t sound_flags;
    uint16_t volume_q15;
    uint16_t pan_q16;
    int16_t rate_delta_percent;
    uint16_t voice_index;
    uint8_t play_type;
    int8_t loop_count;
    uint8_t action;
    /* One-based ordinary pooled playback order. Direct/immediate, pending,
       and suppressed commands are zero. ABI offset/size is unchanged. */
    uint8_t pooled_flush_rank;
    /* reserved[0..1] = the non-zero little-endian u64 pooled flush ID (the
       minimum participating event sequence) when pooled_flush_rank is
       non-zero; reserved[2..3] remain zero. Action zero
       is prepared/pending and must not be rendered. Sidecars remain queryable
       across event-ring expiry until the client releases a resolved row. */
    uint32_t reserved[4];
} McAudioCommandV2;

/*
 * Additive source-presentation sidecar. Existing snapshots and their hashes
 * remain stable. source_turn is the local player Turn, not the MCP1 header
 * count: players.bin has a 12-byte header and Turn is record offset 8.
 */
typedef struct McPresentationStateV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t tick;
    int32_t source_turn;
    uint16_t camera_x;
    uint16_t camera_y;
    int16_t camera_z;
    int16_t camera_yaw;
    int16_t camera_pitch;
    int16_t camera_roll;
    int16_t camera_fov;
    uint8_t camera_authority;
    uint8_t camera_flags;
    uint32_t reserved_0;
    uint32_t reserved[4];
} McPresentationStateV2;

/*
 * Additive final outdoor-render camera sidecar. The raw pre-update history
 * remains in McPresentationStateV2; these fields expose DrawWorld_411A0's
 * stateful terrain-slope X/Y target and filtered offset.
 */
typedef struct McRenderCameraStateV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t tick;
    int32_t source_turn;
    uint16_t history_x;
    uint16_t history_y;
    int16_t eye_z;
    int16_t yaw;
    int16_t pitch;
    int16_t roll;
    int16_t fov;
    int16_t slope_target_x;
    int16_t slope_target_y;
    int16_t render_offset_x;
    int16_t render_offset_y;
    uint8_t authority;
    uint8_t flags;
    uint16_t reserved_0;
    uint32_t reserved[4];
} McRenderCameraStateV2;

typedef struct McTerrainPnt5VectorV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    int32_t turn;
    int32_t tile_x;
    int32_t tile_y;
    uint32_t distance_squared;
    uint8_t shade;
    uint8_t angle;
    uint8_t visible;
    uint8_t tablesn_row;
    int32_t wave;
    int32_t land_displacement;
    int32_t pnt5;
    int32_t view_y;
    int32_t lighting_wave;
    uint32_t reserved[2];
} McTerrainPnt5VectorV2;

typedef struct McTerrainGridVertexV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint16_t index;
    uint8_t row;
    uint8_t column;
    uint8_t tile_x;
    uint8_t tile_y;
    uint8_t shade;
    uint8_t angle;
    int32_t view_x;
    int32_t view_depth;
    uint32_t distance_squared;
    int32_t pnt5;
    int32_t raw_wave;
    int32_t surface_displacement;
    int32_t lighting_wave;
    uint16_t feature_flags;
    uint8_t tablesn_row;
    uint8_t pnt5_written;
    uint32_t reserved[4];
} McTerrainGridVertexV2;

typedef struct McTerrainFrameQueryV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    /* Presentation/render-camera tick, not post-simulation snapshot tick. */
    uint64_t tick;
    uint32_t reserved[4];
} McTerrainFrameQueryV2;

typedef struct McTerrainFrameInfoV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t tick;
    uint16_t width;
    uint16_t height;
    uint16_t stride;
    uint16_t viewport_x;
    uint16_t viewport_y;
    uint16_t viewport_width;
    uint16_t viewport_height;
    uint16_t reserved_0;
    uint32_t index_bytes;
    uint32_t coverage_bytes;
    uint32_t flags;
    uint32_t reflected_triangles;
    uint32_t primary_mode5_triangles;
    uint32_t primary_mode26_triangles;
    uint64_t index_hash;
    uint64_t coverage_hash;
    uint64_t terrain_pixel_writes;
    uint32_t reserved[2];
} McTerrainFrameInfoV2;

typedef struct McPresentationFrameQueryV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t tick;
    uint64_t presentation_revision;
    uint16_t viewport_width;
    uint16_t viewport_height;
    uint16_t screen_stride;
    uint16_t reserved_0;
    uint8_t configured_blur_mode;
    uint8_t blur_capability;
    uint8_t high_speed_system;
    uint8_t scratch_present;
    uint8_t vga_resolution;
    uint8_t spatial_smoothing;
    uint8_t minimap_enabled;
    uint8_t top_bar_enabled;
    uint8_t menu_overlay;
    uint8_t game_paused;
    uint8_t level_end;
    uint8_t objective_selector;
    uint8_t objective_textbox;
    uint8_t tutorial_or_briefing;
    uint8_t help_popup;
    uint8_t debug_text;
    uint32_t reserved[2];
} McPresentationFrameQueryV2;

typedef struct McLevel1PresentationInputV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint32_t authority_mask;
    uint8_t selected_menu_item;
    uint8_t pause_menu_suppressed;
    uint8_t pause_text_suppressed;
    uint8_t level_end_text_suppressed;
    uint8_t recursive_pause_writer;
    uint8_t load_available;
    uint8_t sound_available;
    uint8_t music_available;
    uint32_t options_enabled_mask;
    uint32_t options_selected_mask;
    uint8_t sound_volume;
    uint8_t music_volume;
    uint8_t volume_target;
    uint8_t chat_recipient;
    uint8_t chat_mode;
    uint8_t chat_player_mask;
    uint8_t local_player_index;
    uint8_t frame_timing_index;
    uint8_t cursor_visible;
    uint8_t reserved_0;
    int16_t mouse_x;
    int16_t mouse_y;
    int16_t joystick;
    uint8_t text_colour_phase;
    uint8_t reserved_1;
    uint8_t chat_lines[8 * 64];
    uint8_t chat_names[8 * 16];
    uint8_t notification_player_name[16];
    uint32_t debug_clock_bucket;
    int16_t sound_number;
    uint16_t reserved_2;
    uint32_t debug_heap_size;
    uint32_t debug_memory_used;
    uint32_t debug_memory_free;
    uint32_t reserved[4];
} McLevel1PresentationInputV2;

typedef struct McPresentationFrameInfoV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t tick;
    uint64_t presentation_revision;
    uint64_t immutable_input_hash;
    uint16_t width;
    uint16_t height;
    uint16_t stride;
    uint16_t viewport_width;
    uint32_t index_bytes;
    uint32_t provenance_bytes;
    uint32_t flags;
    uint8_t effective_postprocess_mode;
    uint8_t history_state;
    uint8_t newly_committed;
    uint8_t reserved_0;
    uint64_t index_hash;
    uint64_t provenance_hash;
    uint64_t lighting_hash;
    uint64_t source_arena_hash;
    uint32_t overlay_pixels_written;
    uint16_t overlay_stage_count;
    uint16_t reserved_1;
    uint32_t reserved[6];
} McPresentationFrameInfoV2;

/*
 * Legacy ABI v1. Each call contains Rust panics and returns a type-appropriate
 * zero sentinel if containment is needed. Successful signatures and values
 * are unchanged.
 */
uint32_t mc_sim_abi_version(void);
uint64_t mc_sim_reset(void);
uint64_t mc_sim_step(int8_t opcode, int8_t argument_1, int8_t roll,
                     int8_t pitch, int8_t flags);
int16_t mc_sim_ground_height(uint16_t x, uint16_t y);
uint16_t mc_sim_x(void);
uint16_t mc_sim_y(void);
int16_t mc_sim_z(void);
int16_t mc_sim_yaw(void);
int16_t mc_sim_display_pitch(void);
int16_t mc_sim_effective_pitch(void);
int16_t mc_sim_roll_accumulator(void);
int16_t mc_sim_target_speed(void);
int16_t mc_sim_actual_speed(void);
uint8_t mc_sim_phase(void);
uint32_t mc_sim_fire_sequence(void);
uint8_t mc_sim_last_resolution(void);

/*
 * Rust-owned outer game runtime ABI. The same handle moves through Title,
 * Campaign (results/mission selection), Loading, Playing and Paused. Once Playing, the
 * mc_game_session_*_v1 functions below address the nested authoritative
 * GameSession through this handle. Completion acceptance is guarded by both
 * outer and nested identity and returns to Campaign results in Rust.
 * finish_mission_if_ready is an explicit frame boundary: unfinished gameplay
 * returns OK without advancing its tick or generation; finished gameplay
 * releases the session and publishes a completion receipt. Observational
 * identity/receipt reads never accept completion or start another mission.
 * Replay saves are opaque UTF-8 JSON under
 * `carpet-flight-mc2-campaign-replay-save/v7`; older save contracts fail
 * closed during load.
 */
int32_t mc_game_runtime_contract_json_v1(
    uint8_t *buffer, uint32_t capacity, uint32_t *out_length);
int32_t mc_game_runtime_create_v1(uint64_t *out_handle);
int32_t mc_game_runtime_identity_json_v1(
    uint64_t handle, uint8_t *buffer, uint32_t capacity,
    uint32_t *out_length);
/* Identity-bound render fields without diagnostic hashes. The terrain
 * mutation sequence is scoped to identity.generation. Playing and paused
 * runtimes are accepted; the usual JSON buffer sizing convention applies. */
int32_t mc_game_runtime_render_observation_json_v1(
    uint64_t handle, uint8_t *buffer, uint32_t capacity,
    uint32_t *out_length);
/* Coherent little-endian MCPO v1 render packet. On a short buffer, no
 * payload is written and out_written receives the required byte count.
 * Stale outer generation, session generation or tick is rejected. */
int32_t mc_game_runtime_render_observation_copy_v1(
    uint64_t handle, uint64_t expected_runtime_generation,
    uint64_t expected_session_generation, uint64_t expected_tick,
    uint8_t *buffer, uint32_t capacity, uint32_t *out_written);
/* Read-only, generation/tick-guarded live source BUILD owners. */
typedef struct McRuntimeBuildingV1 {
    uint32_t abi_version, struct_size;
    uint64_t generation, tick;
    uint32_t slot_generation;
    uint16_t source_slot, source_id;
    uint8_t building_index, action, active, drawing;
    uint16_t x, y;
    int16_t z;
    uint16_t reserved[3];
} McRuntimeBuildingV1;
int32_t mc_game_runtime_buildings_copy_v1(
    uint64_t handle, uint64_t generation, uint64_t expected_tick,
    McRuntimeBuildingV1 *buffer, uint32_t capacity, uint32_t element_size,
    uint32_t *out_written);
int32_t mc_game_runtime_bootstrap_package_set_json_v1(
    uint64_t handle, uint64_t generation,
    const uint8_t *request_json, uint32_t request_json_len);
int32_t mc_game_runtime_save_replay_json_v1(
    uint64_t handle, uint64_t runtime_generation,
    uint64_t session_generation, uint64_t expected_tick,
    uint8_t *buffer, uint32_t capacity, uint32_t *out_length);
int32_t mc_game_runtime_load_replay_json_v1(
    uint64_t handle, uint64_t generation,
    const uint8_t *package_set_json, uint32_t package_set_json_len,
    const uint8_t *save_json, uint32_t save_json_len);
/* Menu Load preserves the guarded old runtime's nonserialized MC2 process
 * context. The old owner remains untouched if verification/loading fails. */
int32_t mc_game_runtime_load_replay_from_runtime_json_v1(
    uint64_t handle, uint64_t generation,
    uint64_t source_handle, uint64_t source_generation,
    const uint8_t *package_set_json, uint32_t package_set_json_len,
    const uint8_t *save_json, uint32_t save_json_len);
int32_t mc_game_runtime_open_level_v1(
    uint64_t handle, uint64_t generation);
/* Observes the exact pending installed definition; never opens a level.
 * Cancellation restores its original Title/Campaign owner with a new outer
 * generation. Failed opens retain the same Loading guard for retry. */
int32_t mc_game_runtime_loading_presentation_json_v1(
    uint64_t handle, uint64_t generation,
    uint8_t *buffer, uint32_t capacity, uint32_t *out_length);
/* Reads the exact installed source presentation binding of the live Playing
 * session after direct secret entry. Does not create a Loading owner. */
int32_t mc_game_runtime_active_entry_presentation_json_v1(
    uint64_t handle, uint64_t generation,
    uint8_t *buffer, uint32_t capacity, uint32_t *out_length);

int32_t mc_game_runtime_cancel_loading_v1(
    uint64_t handle, uint64_t generation);
int32_t mc_game_runtime_restart_new_game_v1(
    uint64_t handle, uint64_t generation);
int32_t mc_game_runtime_pause_v1(
    uint64_t handle, uint64_t generation);
int32_t mc_game_runtime_paused_frame_v1(
    uint64_t handle,
    uint64_t runtime_generation,
    uint64_t session_generation,
    uint64_t expected_tick);
int32_t mc_game_runtime_paused_frame_timed_v1(
    uint64_t handle, uint64_t runtime_generation, uint64_t session_generation,
    uint64_t expected_tick, uint32_t elapsed_centiseconds, uint8_t show_objective);
/* One admitted UI frame in Playing or Paused. Does not advance gameplay/RNG.
 * Call once per rendered frame; JSON observation/capacity queries are read-only. */
int32_t mc_game_runtime_presentation_frame_v1(
    uint64_t handle, uint64_t runtime_generation, uint64_t session_generation,
    uint64_t expected_tick, uint8_t notifications_visible);
int32_t mc_game_runtime_participant_notifications_json_v1(
    uint64_t handle, uint8_t *buffer, uint32_t capacity, uint32_t *out_length);
int32_t mc_game_runtime_resume_v1(
    uint64_t handle, uint64_t generation);
int32_t mc_game_runtime_accept_transition_v1(
    uint64_t handle,
    uint64_t runtime_generation,
    uint64_t session_generation,
    uint64_t expected_tick);
int32_t mc_game_runtime_completion_transition_receipt_json_v1(
    uint64_t handle, uint8_t *buffer, uint32_t capacity,
    uint32_t *out_length);
/* Tagged accepted boundary: ordinary_completion or secret_transition, with
 * exactly one typed receipt payload. The ordinary-only query stays strict. */
int32_t mc_game_runtime_boundary_transition_receipt_json_v1(
    uint64_t handle, uint8_t *buffer, uint32_t capacity,
    uint32_t *out_length);
int32_t mc_game_runtime_campaign_menu_command_json_v1(
    uint64_t handle, uint64_t generation,
    const uint8_t *request, uint32_t request_len);
/* Bounded JSON commands; observations are in runtime identity. */
int32_t mc_game_runtime_media_command_json_v1(
    uint64_t handle, uint64_t generation,
    const uint8_t *request, uint32_t request_len);
int32_t mc_game_runtime_preferences_command_json_v1(
    uint64_t handle, uint64_t generation,
    const uint8_t *request, uint32_t request_len);
int32_t mc_game_runtime_finish_mission_if_ready_v1(
    uint64_t handle, uint64_t runtime_generation,
    uint64_t session_generation, uint64_t expected_tick);
/* As above, additionally reports 1 when the boundary transitioned; 0 when
 * it was a no-op or rejected. The output pointer must be non-null. */
int32_t mc_game_runtime_finish_mission_if_ready_result_v1(
    uint64_t handle, uint64_t runtime_generation,
    uint64_t session_generation, uint64_t expected_tick, uint8_t *out_changed);
int32_t mc_game_runtime_close_v1(
    uint64_t handle,
    uint64_t runtime_generation,
    uint64_t session_generation,
    uint64_t expected_tick);
int32_t mc_game_runtime_destroy_v1(uint64_t handle);

/*
 * Definition-bound neutral GameSession ABI v1. JSON outputs are UTF-8 byte
 * spans without a trailing NUL. A zero-capacity read returns MC_STATUS_BOUNDS
 * and writes the required byte count without changing session state.
 */
int32_t mc_game_session_contract_json_v1(
    uint8_t *buffer, uint32_t capacity, uint32_t *out_length);
int32_t mc_game_session_open_v1(
    const uint8_t *package_dir, uint32_t package_dir_len,
    const uint8_t *closure_path, uint32_t closure_path_len,
    const uint8_t *asset_cache_path, uint32_t asset_cache_path_len,
    uint64_t *out_handle);
int32_t mc_game_session_open_workspace_v1(
    const uint8_t *package_dir, uint32_t package_dir_len,
    const uint8_t *closure_path, uint32_t closure_path_len,
    const uint8_t *asset_cache_path, uint32_t asset_cache_path_len,
    const uint8_t *workspace_path, uint32_t workspace_path_len,
    uint64_t *out_handle);
int32_t mc_game_session_destroy_v1(uint64_t handle);
int32_t mc_game_session_snapshot_json_v1(
    uint64_t handle, uint8_t *buffer, uint32_t capacity,
    uint32_t *out_length);
int32_t mc_game_session_step_json_v1(
    uint64_t handle, uint64_t generation, uint64_t expected_tick,
    const uint8_t *command_json, uint32_t command_json_len);
int32_t mc_game_session_reset_v1(
    uint64_t handle, uint64_t generation, uint64_t expected_tick);
int32_t mc_game_session_events_json_v1(
    uint64_t handle, uint64_t generation, uint64_t expected_tick,
    uint64_t cursor, uint32_t limit, uint8_t *buffer, uint32_t capacity,
    uint32_t *out_length);
int32_t mc_game_session_audio_json_v1(
    uint64_t handle, uint64_t generation, uint64_t expected_tick,
    uint64_t event_sequence, uint8_t *buffer, uint32_t capacity,
    uint32_t *out_length);
int32_t mc_game_session_presentation_asset_json_v1(
    uint64_t handle, uint64_t generation, uint64_t expected_tick,
    const uint8_t *semantic_id, uint32_t semantic_id_len, uint8_t *buffer,
    uint32_t capacity, uint32_t *out_length);

/* Compatibility package-backed handle ABI v2. */
uint32_t mc_native_abi_version(void);
int32_t mc_native_session_open(const uint8_t *package_dir,
                               uint32_t package_dir_len,
                               const uint8_t *closure_path,
                               uint32_t closure_path_len,
                               uint64_t *out_handle);
int32_t mc_native_session_destroy(uint64_t handle);
int32_t mc_native_session_reset(uint64_t handle,
                                McSessionSnapshotV2 *out_snapshot,
                                uint32_t out_snapshot_size);
int32_t mc_native_session_snapshot(uint64_t handle,
                                   McSessionSnapshotV2 *out_snapshot,
                                   uint32_t out_snapshot_size);
/* Hashes only the original-game FlightState projection. The additive native
 * session tick is normalized to zero; mc_native_session_snapshot.state_hash
 * remains the live lifecycle-sensitive hash. */
int32_t mc_native_session_source_projection_hash(uint64_t handle,
                                                 uint64_t *out_hash);
int32_t mc_native_session_source_arena_hash(uint64_t handle,
                                            uint64_t *out_hash);
int32_t mc_native_level1_action_51_fallback_state_hash_v1(
    uint64_t handle, uint64_t *out_hash);
int32_t mc_native_level1_resident_observation_v1(
    uint64_t handle, McLevel1ResidentObservationV1 *out_observation,
    uint32_t out_observation_size);
int32_t mc_native_level1_player_menu_state(uint64_t handle,
                                           uint8_t menu_state);
int32_t mc_native_level1_game_paused(uint64_t handle, uint8_t paused);
int32_t mc_native_level1_presentation_input(
    uint64_t handle, const McLevel1PresentationInputV2 *input);
int32_t mc_native_level1_player_inject_damage_for_qualification(
    uint64_t handle, int32_t damage, uint16_t attacker_entity_index);
int32_t mc_native_level1_action_51_blocker_setup_for_qualification(
    uint64_t handle, uint16_t house_entity_index, uint8_t house_action);
int32_t mc_native_level1_action_51_completion_setup_for_qualification(
    uint64_t handle, uint16_t house_entity_index,
    uint16_t parent_entity_index, uint8_t construction_selector);
int32_t mc_native_presentation_state(uint64_t handle,
                                     McPresentationStateV2 *out_state,
                                     uint32_t out_state_size);
int32_t mc_native_render_camera_state(uint64_t handle,
                                      McRenderCameraStateV2 *out_state,
                                      uint32_t out_state_size);
uint32_t mc_native_terrain_pnt5_vector_count(void);
int32_t mc_native_terrain_pnt5_vector(
    uint32_t index, McTerrainPnt5VectorV2 *out_vector,
    uint32_t out_vector_size);
uint32_t mc_native_terrain_grid_vertex_count(void);
int32_t mc_native_session_terrain_grid_copy(
    uint64_t handle, McTerrainGridVertexV2 *buffer, uint32_t capacity,
    uint32_t element_size, uint32_t *out_written);
int32_t mc_native_session_terrain_frame_copy(
    uint64_t handle, const McTerrainFrameQueryV2 *query,
    uint8_t *index_buffer, uint32_t index_capacity,
    uint8_t *coverage_buffer, uint32_t coverage_capacity,
    McTerrainFrameInfoV2 *out_info, uint32_t out_info_size);
int32_t mc_native_session_world_frame_copy(
    uint64_t handle, const McTerrainFrameQueryV2 *query,
    uint8_t *index_buffer, uint32_t index_capacity,
    uint8_t *coverage_buffer, uint32_t coverage_capacity,
    McTerrainFrameInfoV2 *out_info, uint32_t out_info_size);
int32_t mc_native_session_presentation_frame_copy(
    uint64_t handle, const McPresentationFrameQueryV2 *query,
    uint8_t *index_buffer, uint32_t index_capacity,
    uint8_t *provenance_buffer, uint32_t provenance_capacity,
    McPresentationFrameInfoV2 *out_info, uint32_t out_info_size);
int32_t mc_native_session_step(uint64_t handle, const McCommandV2 *command,
                               McSessionSnapshotV2 *out_snapshot,
                               uint32_t out_snapshot_size);
int32_t mc_native_session_step_with_camera(
    uint64_t handle, const McCommandV2 *command,
    const McCameraInputV2 *camera_input,
    McSessionSnapshotV2 *out_snapshot, uint32_t out_snapshot_size);
int32_t mc_native_world_info(uint64_t handle, McWorldInfoV2 *out_info,
                             uint32_t out_info_size);
int32_t mc_native_world_string_length(uint64_t handle, uint32_t kind,
                                      uint32_t *out_length);
int32_t mc_native_world_string_copy(uint64_t handle, uint32_t kind,
                                    uint8_t *buffer, uint32_t capacity,
                                    uint32_t *out_length);
int32_t mc_native_terrain_plane_length(uint64_t handle, uint32_t kind,
                                       uint32_t *out_count);
int32_t mc_native_session_terrain_state(
    uint64_t handle, McTerrainStateV2 *out_state,
    uint32_t out_state_size);
int32_t mc_native_terrain_plane_copy_u8(uint64_t handle, uint32_t kind,
                                        uint8_t *buffer, uint32_t capacity,
                                        uint32_t *out_written);
int32_t mc_native_terrain_entity_indices_copy(uint64_t handle,
                                              int16_t *buffer,
                                              uint32_t capacity,
                                              uint32_t *out_written);
int32_t mc_native_entity_count(uint64_t handle, uint32_t *out_count);
int32_t mc_native_objective_count(uint64_t handle, uint32_t *out_count);
int32_t mc_native_objectives_copy(uint64_t handle, uint32_t start_slot,
                                  McObjectiveSnapshotV2 *buffer,
                                  uint32_t capacity, uint32_t element_size,
                                  uint32_t *out_written);
int32_t mc_native_entities_copy(uint64_t handle, uint32_t start_index,
                                McEntitySnapshotV2 *buffer, uint32_t capacity,
                                uint32_t element_size, uint32_t *out_written);
int32_t mc_native_goats_copy(uint64_t handle, uint64_t generation,
                             uint32_t start_ordinal,
                             McGoatSnapshotV2 *buffer, uint32_t capacity,
                             uint32_t element_size, McGoatBatchV2 *out_batch,
                             uint32_t out_batch_size);
int32_t mc_native_level1_mission_snapshot(
    uint64_t handle, McLevel1MissionSnapshotV2 *out_snapshot,
    uint32_t out_snapshot_size);
int32_t mc_native_level1_player_lifecycle_snapshot(
    uint64_t handle, McLevel1PlayerLifecycleSnapshotV2 *out_snapshot,
    uint32_t out_snapshot_size);
int32_t mc_native_level1_hud_snapshot(
    uint64_t handle, McLevel1HudSnapshotV2 *out_snapshot,
    uint32_t out_snapshot_size);
int32_t mc_native_level1_spell_menu_snapshot(
    uint64_t handle, McLevel1SpellMenuSnapshotV2 *out_snapshot,
    uint32_t out_snapshot_size);
int32_t mc_native_level1_minimap_marks(
    uint64_t handle, McLevel1MinimapMarksV2 *out_state,
    uint32_t out_state_size);
int32_t mc_native_level1_minimap_entities_copy(
    uint64_t handle, uint64_t generation, uint64_t expected_tick,
    McLevel1MinimapEntityCommandV2 *buffer, uint32_t capacity,
    uint32_t element_size, McLevel1MinimapEntitiesInfoV2 *out_info,
    uint32_t out_info_size);
int32_t mc_native_level1_palette_copy(
    uint64_t handle, uint8_t *rgb8_buffer, uint32_t rgb8_capacity,
    McLevel1PaletteInfoV2 *out_info, uint32_t out_info_size);
int32_t mc_native_level1_exit_presentation(
    uint64_t handle, McLevel1ExitPresentationV2 *out_state,
    uint32_t out_state_size);
int32_t mc_native_level1_castle_snapshot(
    uint64_t handle, McLevel1CastleSnapshotV2 *out_snapshot,
    uint32_t out_snapshot_size);
int32_t mc_native_level1_castle_lifecycle_snapshot(
    uint64_t handle, McLevel1CastleLifecycleSnapshotV2 *out_snapshot,
    uint32_t out_snapshot_size);
int32_t mc_native_level1_house_completion_effect_count(
    uint64_t handle, uint32_t *out_count);
int32_t mc_native_level1_house_completion_effect_snapshot(
    uint64_t handle, uint32_t effect_ordinal,
    McLevel1HouseCompletionEffectV2 *out_snapshot,
    uint32_t out_snapshot_size);
int32_t mc_native_level1_mana_carrier_snapshot(
    uint64_t handle, McManaCarrierSnapshotV2 *out_snapshot,
    uint32_t out_snapshot_size);
int32_t mc_native_level1_mana_spheres_copy(
    uint64_t handle, uint64_t generation, uint64_t expected_tick,
    uint32_t start_ordinal, McManaSphereSnapshotV2 *buffer,
    uint32_t capacity, uint32_t element_size, uint32_t *out_written);
int32_t mc_native_level1_actors_copy(
    uint64_t handle, uint64_t generation, uint64_t expected_tick,
    uint32_t start_ordinal, McLevel1ActorSnapshotV2 *buffer,
    uint32_t capacity, uint32_t element_size, uint32_t *out_written);
int32_t mc_native_level1_multipart_segments_copy(
    uint64_t handle, uint64_t generation, uint64_t expected_tick,
    uint32_t start_ordinal, McLevel1MultipartSegmentSnapshotV2 *buffer,
    uint32_t capacity, uint32_t element_size, uint32_t *out_written);
int32_t mc_native_fireball_projectiles_copy(
    uint64_t handle, uint64_t generation, uint64_t expected_tick,
    uint32_t start_ordinal, McFireballProjectileSnapshotV2 *buffer,
    uint32_t capacity, uint32_t element_size, uint32_t *out_written);
int32_t mc_native_fireball_impacts_copy(
    uint64_t handle, uint64_t generation, uint64_t expected_tick,
    uint32_t start_ordinal, McFireballImpactSnapshotV2 *buffer,
    uint32_t capacity, uint32_t element_size, uint32_t *out_written);
int32_t mc_native_selector_frame_states_copy(
    uint64_t handle, uint64_t generation, uint64_t expected_tick,
    uint32_t start_ordinal, McSelectorFrameStateV2 *buffer, uint32_t capacity,
    uint32_t element_size, McSelectorFrameBatchV2 *out_batch,
    uint32_t out_batch_size);
int32_t mc_native_session_goat_draw_admission_evaluate(
    uint64_t handle, uint64_t generation, uint64_t expected_tick,
    McSelectorFrameBatchV2 *out_batch, uint32_t out_batch_size);
int32_t mc_native_session_selector_draw_feedback_submit(
    uint64_t handle, const McSelectorDrawFeedbackV2 *command,
    McSelectorFrameBatchV2 *out_batch, uint32_t out_batch_size);
int32_t mc_native_session_goat_inject_damage(
    uint64_t handle, const McGoatDamageCommandV2 *command,
    McGoatBatchV2 *out_batch, uint32_t out_batch_size);
int32_t mc_native_asset_ref_count(uint64_t handle, uint32_t *out_count);
int32_t mc_native_asset_ref_info(uint64_t handle, uint32_t asset_index,
                                 McAssetRefInfoV2 *out_info,
                                 uint32_t out_info_size);
int32_t mc_native_asset_string_length(uint64_t handle, uint32_t asset_index,
                                      uint32_t kind, uint32_t owner_index,
                                      uint32_t *out_length);
int32_t mc_native_asset_string_copy(uint64_t handle, uint32_t asset_index,
                                    uint32_t kind, uint32_t owner_index,
                                    uint8_t *buffer, uint32_t capacity,
                                    uint32_t *out_length);
int32_t mc_native_events_copy_since(uint64_t handle,
                                    const McEventQueryV2 *query,
                                    McEventV2 *buffer, uint32_t capacity,
                                    uint32_t element_size,
                                    McEventBatchV2 *out_batch,
                                    uint32_t out_batch_size);
int32_t mc_native_audio_command_for_event(
    uint64_t handle, uint64_t generation, uint64_t event_sequence,
    McAudioCommandV2 *out_command, uint32_t out_command_size);
int32_t mc_native_audio_command_release(
    uint64_t handle, uint64_t generation, uint64_t event_sequence);
int32_t mc_native_audio_command_discard(
    uint64_t handle, uint64_t generation, uint64_t event_sequence);
int32_t mc_native_audio_voice_complete(
    uint64_t handle, uint64_t generation, uint16_t voice_index,
    uint64_t event_sequence);
uint32_t mc_native_audio_qualification_vector_count(void);
int32_t mc_native_audio_qualification_vector(
    uint32_t index, McAudioCommandV2 *out_command,
    uint32_t out_command_size);
int32_t mc_native_audio_qualification_submit(
    uint64_t handle, uint32_t event_kind,
    uint16_t sound_id, int16_t sound_flags,
    uint16_t volume_q15, uint16_t pan_q16, uint8_t play_type,
    McEventV2 *out_event, uint32_t out_event_size,
    McAudioCommandV2 *out_command, uint32_t out_command_size);
int32_t mc_native_last_error_length(uint32_t *out_length);
/* Copy failures preserve the diagnostic so callers may resize and retry. */
int32_t mc_native_last_error_copy(uint8_t *buffer, uint32_t capacity,
                                  uint32_t *out_length);

#ifdef __cplusplus
}
#endif

#endif /* MC_SIM_H */
