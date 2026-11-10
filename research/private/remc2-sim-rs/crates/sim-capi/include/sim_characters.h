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
/* 96-byte V2: V1 families plus Firefly and real linked Worm parts. The prefix
 * struct_size is 96. render_height is the particle-table projected height,
 * in original source units (64 per world unit). source_half_width is the
 * Worm collision-array extent, NOT a visual mesh width; zero for other
 * families. Raw pitch/roll may be steering fields, not physical Euler angles.
 * Worm head parameter 88; parts 89..104. A part's
 * has_parent is false after its original head disappears. No synthetic motion
 * or gameplay time is produced. lifecycle_flags bit0 identifies a living
 * model13 action108 house entry; all other bits are zero. */
typedef struct McCampaignCharacterStateV2 {
    McCampaignCharacterStateV1 base;
    int16_t roll;
    uint16_t source_half_width, render_height, previous_index, next_index, part_index;
    uint32_t lifecycle_flags;
} McCampaignCharacterStateV2;
int32_t mc_native_campaign_character_states_copy_v2(uint64_t handle,
    uint64_t generation, uint64_t tick, McCampaignCharacterStateV2 *buffer,
    uint32_t capacity, uint32_t element_size, uint32_t *out_written);
/* The source Castle's class-3/model-3 balloon holders. These are live arena
 * occupants, including a hidden holder until the following reclaim pass.
 * castle_index/generation identify the current matching owner Castle, not
 * the holder's raw (zero) parent word. target_index is the raw sphere word;
 * relation_flags bit0 means owner_player_color is known, bit1 means castle
 * is current, bit2 means target_index resolves to a current ManaSphere.
 * drawing is zero when the source draw-disable flag is set. */
typedef struct McCampaignCastleChildV1 {
    uint32_t abi_version, struct_size;
    uint64_t generation, tick;
    uint32_t slot_generation, castle_generation, target_generation;
    uint16_t entity_index, source_id, owner_entity_index, player_entity_index;
    uint16_t castle_index, target_index, x, y;
    int16_t z, yaw, speed;
    uint16_t render_parameter, render_height, reserved_0;
    int32_t life, max_life, mana, max_mana;
    uint32_t control_flags;
    uint8_t class_id, model, action, phase;
    uint8_t active, drawing, owner_player_color, relation_flags;
} McCampaignCastleChildV1;
int32_t mc_native_campaign_castle_children_copy_v1(uint64_t handle,
    uint64_t generation, uint64_t tick, McCampaignCastleChildV1 *buffer,
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
