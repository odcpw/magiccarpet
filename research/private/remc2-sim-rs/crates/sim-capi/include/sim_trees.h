#ifndef MC_SIM_TREES_H
#define MC_SIM_TREES_H
#include "mc_sim.h"
#ifdef __cplusplus
extern "C" {
#endif
/* Read-only current source Tree occupants (class 2/model 0). generation and
 * tick must match the descriptor campaign; slot_generation identifies reuse.
 * authored_index is UINT16_MAX for a reset-imported occupant. drawing follows
 * the source disable/hide flags; a hidden occupant may persist until reclaim.
 * render_height is in source units (64 per world unit), and render_parameter
 * is the source particle selector, not a TMAP number. */
typedef struct McCampaignAuthoredTreeV1 {
    uint32_t abi_version, struct_size;
    uint64_t generation, tick;
    uint32_t slot_generation;
    uint16_t entity_index, source_id, authored_index, render_parameter, render_height;
    uint16_t x, y;
    int16_t z;
    int32_t life;
    uint32_t control_flags;
    uint8_t class_id, model, action, drawing;
} McCampaignAuthoredTreeV1;
int32_t mc_native_campaign_authored_trees_copy_v1(uint64_t handle,
    uint64_t generation, uint64_t tick, McCampaignAuthoredTreeV1 *buffer,
    uint32_t capacity, uint32_t element_size, uint32_t *out_written);
#ifdef __cplusplus
}
#endif
#endif
