#ifndef MC_SIM_VOXELS_H
#define MC_SIM_VOXELS_H
#include "mc_sim.h"
#ifdef __cplusplus
extern "C" {
#endif
/* Optional native voxel world; observations and enable require the current
 * playing-session generation and simulation tick. Layout: 64 bytes.
 * Configuration contains selected_slot and 82277 IEEE-f32 modern_heights
 * bit patterns. Native BUILD/terrain sources are independently validated.
 * Configuration is retained at its exact semantic replay operation boundary. */
typedef struct McVoxelStateV1 {
    uint32_t abi_version, struct_size;
    uint64_t generation, tick, revision, content_hash;
    uint32_t enabled, removed_count;
    uint64_t impact_head;
    /* Stable world/journal identity, retained across pause/resume command
     * generation changes; distinct from the generation guard above. */
    uint64_t world_generation;
} McVoxelStateV1;
/* Current exact impact lifetimes only; refresh at each observed tick. */
typedef struct McVoxelImpactChildV1 {
    uint32_t abi_version, struct_size;
    uint64_t generation, sequence;
    uint32_t slot_generation;
    uint16_t entity_index, reserved;
} McVoxelImpactChildV1;
int32_t mc_game_runtime_voxel_impact_children_copy(uint64_t handle,
    uint64_t generation, uint64_t tick, McVoxelImpactChildV1 *buffer,
    uint32_t capacity, uint32_t element_size, uint32_t *out_written);
int32_t mc_game_runtime_voxel_enable_json_v1(uint64_t handle,
    uint64_t generation, uint64_t tick, const uint8_t *request, uint32_t length);
int32_t mc_game_runtime_voxel_state_v1(uint64_t handle,
    uint64_t generation, uint64_t tick, McVoxelStateV1 *out, uint32_t size);
/* include_base is 0/1; impacts strictly after after_impact. JSON contains
 * cumulative removed cells plus a bounded impact journal. Callers validate
 * sequence continuity. Initial size probe uses null buffer/capacity0 and
 * returns the required byte count with MC_STATUS_BOUNDS. */
int32_t mc_game_runtime_voxel_snapshot_json_v1(uint64_t handle,
    uint64_t generation, uint64_t tick, uint32_t include_base,
    uint64_t after_impact, uint8_t *buffer, uint32_t capacity,
    uint32_t *out_length);
#ifdef __cplusplus
}
#endif
#endif
