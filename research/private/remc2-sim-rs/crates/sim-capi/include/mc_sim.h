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

#define MC_AUDIO_ACTION_START 1u
#define MC_AUDIO_ACTION_RESTART 2u
#define MC_AUDIO_ACTION_SUPPRESS_EXISTING 3u
#define MC_AUDIO_ACTION_SUPPRESS_NO_VOICE 4u
#define MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME 5u
#define MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED 6u
#define MC_AUDIO_ACTION_UPDATE_EXISTING 7u
#define MC_AUDIO_ACTION_END_LOOP 8u
#define MC_AUDIO_ACTION_SUPPRESS_NO_EXISTING 9u
#define MC_AUDIO_VOICE_NONE UINT16_MAX
#define MC_PRESENTATION_CAMERA_EXACT_V2_INPUTS 0x01u
#define MC_PRESENTATION_CAMERA_EYE_Z_PLUS_128 0x02u
#define MC_TERRAIN_FRAME_ISOLATED_SKY_TERRAIN 0x0001u
#define MC_TERRAIN_FRAME_SKY_SEEDED 0x0002u
#define MC_TERRAIN_FRAME_HAS_COVERAGE 0x0004u
#define MC_TERRAIN_FRAME_NON_TERRAIN_OMITTED 0x0008u

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
    /* Frozen until the selector-global animation clock is qualified. */
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
    uint8_t reserved_0;
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

/* Package-backed handle ABI v2. */
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
