#ifndef MC_SIM_CHARACTERS_H
#define MC_SIM_CHARACTERS_H
#include "mc_sim.h"
#ifdef __cplusplus
extern "C" {
#endif
/* Read-only live animation observations. Layout:80 bytes. Generation/tick
 * queries must match. All slots carry native allocation generations (zero for
 * never-replaced imported occupants). Arrow rows prove actual native
 * allocation; age_ticks is the consumed native lifetime, not wall time. */
typedef struct McCampaignCharacterStateV1 {
    uint32_t abi_version, struct_size;
    uint64_t generation, tick;
    uint32_t slot_generation, parent_generation;
    uint16_t entity_index, source_id, parent_index, parent_id;
    /* stance: 0 standing/default, 1 kneeling; independent of draw admission. */
    uint8_t class_id, model, action, phase, active, has_target, has_parent, stance;
    uint16_t x, y;
    int16_t z, yaw, pitch, speed;
    int32_t life;
    uint16_t target_index, target_x, target_y;
    int16_t target_z;
    uint32_t control_flags;
    /* render_parameter is the original appearance selector for settlement humans (class 5/models 12–14);
     * it is zero for all other admitted families. */
    uint16_t age_ticks, render_parameter;
} McCampaignCharacterStateV1;
int32_t mc_native_campaign_character_states_copy(uint64_t handle,
    uint64_t generation, uint64_t tick, McCampaignCharacterStateV1 *buffer,
    uint32_t capacity, uint32_t element_size, uint32_t *out_written);
/* Retained once-per-incarnation deaths. Row generation is the stable voxel
 * world epoch; the request generation is the current outer session identity.
 * Cursor zero starts at the oldest retained row; an expired nonzero cursor
 * returns bounds. */
typedef struct McCampaignVoxelDeathV1 {
    uint32_t abi_version, struct_size;
    uint64_t generation, sequence;
    uint32_t slot_generation;
    uint16_t entity_index, source_id;
} McCampaignVoxelDeathV1;
int32_t mc_native_campaign_voxel_deaths_copy(uint64_t handle,
    uint64_t generation, uint64_t tick, uint64_t after_sequence,
    McCampaignVoxelDeathV1 *buffer, uint32_t capacity,
    uint32_t element_size, uint32_t *out_written);
#ifdef __cplusplus
}
#endif
#endif
