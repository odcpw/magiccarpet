#ifndef MC_SIM_LEVEL2_H
#define MC_SIM_LEVEL2_H

/*
 * Additive Level 2 campaign declarations.
 *
 * Keep the sealed Level 1 header (`mc_sim.h`) byte-for-byte stable.  A caller
 * that needs the Level 2 campaign or economy declarations includes this
 * header after (or instead of) `mc_sim.h`; the legacy and Level 1 declarations
 * remain exactly those of the accepted Level 1 contract.
 */

#include "mc_sim.h"

#ifdef __cplusplus
extern "C" {
#endif

/* Additive generic campaign selection; legacy v2 layouts are unchanged. */
typedef struct McCampaignSelectionV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint16_t external_level;
    uint16_t internal_level;
    uint8_t map_type;
    uint8_t reserved_0;
    uint32_t reserved[4];
} McCampaignSelectionV2;

/* Additive Level 2 outer-turn input. The embedded McCommandV2 preserves the
 * legacy command projection; player_slot/position identify the active flight
 * row for the campaign handle. */
typedef struct McCampaignStepInputV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    McCommandV2 command;
    uint8_t player_slot;
    uint8_t speed_index;
    uint16_t reserved_0;
    uint16_t player_x;
    uint16_t player_y;
    int16_t player_z;
    uint32_t reserved[4];
} McCampaignStepInputV2;

enum {
    MC_CAMPAIGN_EVENT_SHARED_PICKUP = 1,
    MC_CAMPAIGN_EVENT_SPELL_ACQUISITION = 2,
    MC_CAMPAIGN_EVENT_BAD_STONE = 3,
    MC_CAMPAIGN_EVENT_HOUSE = 4,
    MC_CAMPAIGN_EVENT_VILLAGER = 5,
    MC_CAMPAIGN_EVENT_DOLMEN = 6,
    MC_CAMPAIGN_EVENT_CASTLE = 7,
    MC_CAMPAIGN_EVENT_DYNAMIC_ARENA = 8,
    MC_CAMPAIGN_EVENT_GOAT = 9,
    MC_CAMPAIGN_EVENT_ERROR = 255
};

/* One bounded, explicitly typed Level 2 source-pass event. */
typedef struct McCampaignEventV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint32_t ordinal;
    uint8_t pass;
    uint8_t kind;
    uint8_t family;
    uint8_t flags;
    uint16_t source_slot;
    uint16_t reserved_0;
    uint32_t reserved[4];
} McCampaignEventV2;

/* Revisioned Level 2 outer-turn snapshot. state_hash is the composite
 * Level 2 campaign fingerprint (flight plus mutable mission subsystems), and
 * dispatch_hash is the SHA-256 digest of the canonical scalar event
 * projection returned by the same call. */
typedef struct McCampaignStepSnapshotV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint64_t revision;
    uint64_t state_hash;
    uint64_t terrain_hash;
    uint8_t dispatch_hash[32];
    uint32_t event_count;
    uint32_t event_kind_mask;
    uint8_t player_slot;
    uint8_t reserved_0[3];
    uint16_t player_entity_index;
    uint16_t reserved_1;
    uint16_t x;
    uint16_t y;
    int16_t z;
    uint32_t reserved[4];
} McCampaignStepSnapshotV2;

typedef struct McCampaignWorldInfoV2 {
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
    uint32_t objective_count;
    uint16_t terrain_width;
    uint16_t terrain_height;
    uint64_t reset_state_hash;
    uint32_t reserved[4];
} McCampaignWorldInfoV2;

typedef struct McCampaignPlayerSnapshotV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint8_t slot;
    uint8_t is_ai;
    uint8_t active;
    uint8_t reserved_0;
    uint16_t entity_index;
    int16_t active_player_index;
    int32_t turn;
    int16_t left_spell_slot_raw;
    int16_t right_spell_slot_raw;
    int8_t left_subspell_index;
    int8_t right_subspell_index;
    uint8_t selected_spell_ui_index;
    int8_t selected_subspell_index;
    uint8_t reserved_1[2];
    uint32_t reserved[4];
} McCampaignPlayerSnapshotV2;

typedef struct McCampaignEntitySnapshotV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint16_t entity_index;
    uint16_t source_id;
    uint16_t next_relation;
    uint16_t parameter_relation;
    uint8_t authority;
    uint8_t active;
    uint8_t class_id;
    uint8_t model;
    uint8_t action;
    uint8_t phase;
    uint16_t x;
    uint16_t y;
    int16_t z;
    int16_t yaw;
    int16_t pitch;
    int16_t fov;
    int16_t actual_speed;
    uint32_t reserved[4];
} McCampaignEntitySnapshotV2;

/* Kind 10 is additive; existing McCampaignEventV2 rows are unchanged. */
enum {
    MC_CAMPAIGN_EVENT_ECONOMY = 10
};

/* Stable per-variant codes for the additive Level 2 economy sidecar.  The
 * generic McCampaignEventV2 row remains unchanged and clients join these
 * rows by generation + ordinal. */
enum {
    MC_CAMPAIGN_ECONOMY_MANA_SPHERE_SPAWNED = 1,
    MC_CAMPAIGN_ECONOMY_MANA_SPHERE_CLAIMED = 2,
    MC_CAMPAIGN_ECONOMY_MANA_SPHERE_COLLECTED = 3,
    MC_CAMPAIGN_ECONOMY_MANA_SPHERES_MERGED = 4,
    MC_CAMPAIGN_ECONOMY_MANA_CARRIER_SPAWNED = 5,
    MC_CAMPAIGN_ECONOMY_MANA_CARRIER_ARRIVED = 6,
    MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_ARRIVED = 7,
    MC_CAMPAIGN_ECONOMY_DEATH_BURST_SPAWNED = 8,
    MC_CAMPAIGN_ECONOMY_DEATH_BURST_EXPIRED = 9,
    MC_CAMPAIGN_ECONOMY_CASTLE_BUILT = 10,
    MC_CAMPAIGN_ECONOMY_CASTLE_DESTROYED = 11,
    MC_CAMPAIGN_ECONOMY_CASTLE_CONSTRUCTION_SOUND = 12,
    MC_CAMPAIGN_ECONOMY_CASTLE_DESTRUCTION_SOUND = 13,
    MC_CAMPAIGN_ECONOMY_CASTLE_STAGE_DESTROYED = 14,
    MC_CAMPAIGN_ECONOMY_DAMAGE_APPLIED = 15
};

/* Detailed payload for a kind-10 McCampaignEventV2 row.  `ordinal` joins the
 * sidecar to the generic event returned by mc_native_campaign_session_step;
 * non-economy rows have no sidecar.  The two ids, two entity indices, and two
 * values have code-specific meanings documented by the Rust projection and
 * are never overloaded through the generic event's reserved bytes. */
typedef struct McCampaignEconomyEventV2 {
    uint32_t abi_version;
    uint32_t struct_size;
    uint64_t generation;
    uint32_t ordinal;
    uint8_t pass;
    uint8_t family;
    uint8_t code;
    uint8_t flags;
    uint16_t source_slot;
    uint16_t reserved_0;
    uint32_t primary_id;
    uint32_t secondary_id;
    uint16_t entity_0;
    uint16_t entity_1;
    int32_t value_0;
    int32_t value_1;
    uint32_t reserved[4];
} McCampaignEconomyEventV2;

int32_t mc_native_campaign_session_open(
    const uint8_t *package_dir, uint32_t package_dir_len,
    const uint8_t *closure_path, uint32_t closure_path_len,
    const McCampaignSelectionV2 *selection, uint64_t *out_handle);
int32_t mc_native_campaign_session_destroy(uint64_t handle);
int32_t mc_native_campaign_session_reset(uint64_t handle);
int32_t mc_native_campaign_session_step(
    uint64_t handle, const McCampaignStepInputV2 *input,
    McCampaignStepSnapshotV2 *out_snapshot, uint32_t out_snapshot_size,
    McCampaignEventV2 *events, uint32_t event_capacity,
    uint32_t event_element_size, uint32_t *out_event_count);
int32_t mc_native_campaign_world_info(
    uint64_t handle, McCampaignWorldInfoV2 *out_info, uint32_t out_info_size);
int32_t mc_native_campaign_player_count(uint64_t handle, uint32_t *out_count);
int32_t mc_native_campaign_objective_count(uint64_t handle, uint32_t *out_count);
int32_t mc_native_campaign_entity_count(uint64_t handle, uint32_t *out_count);
int32_t mc_native_campaign_players_copy(
    uint64_t handle, uint32_t start_slot, McCampaignPlayerSnapshotV2 *buffer,
    uint32_t capacity, uint32_t element_size, uint32_t *out_written);
int32_t mc_native_campaign_objectives_copy(
    uint64_t handle, uint32_t start_slot, McObjectiveSnapshotV2 *buffer,
    uint32_t capacity, uint32_t element_size, uint32_t *out_written);
int32_t mc_native_campaign_entities_copy(
    uint64_t handle, uint32_t start_index, McCampaignEntitySnapshotV2 *buffer,
    uint32_t capacity, uint32_t element_size, uint32_t *out_written);

int32_t mc_native_campaign_economy_events(
    uint64_t handle, McCampaignEconomyEventV2 *events,
    uint32_t event_capacity, uint32_t event_element_size,
    uint32_t *out_event_count);

#ifdef __cplusplus
}
#endif

#endif /* MC_SIM_LEVEL2_H */
