//! Existing class-5/model-3 action-25 transition body.

use super::super::*;

impl Level2EconomyState {
    #[inline]
    pub(super) fn step_authored_class5_model3(
        &self,
        source: &mut Level2AuthoredClass5Source,
        target: Option<EntitySnapshot>,
    ) {
        // `sub_29710` calls `sub_2A6B0` before the movement callback.
        // Keep the mailbox transfer and byte-0x3B-zero fallback exact.
        // The model-3 descriptor is `str_D7BD6[74]`: its target wrapper
        // uses a 5120-unit range and a 682-unit yaw cone.
        const MODEL3_TARGET_RANGE: u32 = 0x1400;
        const MODEL3_TARGET_HALF_ANGLE: i16 = 0x02aa;
        let mut run_terrain_resnap = true;
        if source.pending_word_0x62 != 0 {
            let pending = source.pending_word_0x62;
            source.word_0x26 = pending;
            source.pending_word_0x62 = 0;
            if source.byte_0x3b == 0 {
                source.action = 220;
                source.word_0x24 = pending;
                run_terrain_resnap = false;
            } else {
                // `sub_29710`'s return-1 pre-pass promotes the copied
                // target and refreshes life before `sub_2AF10` runs.
                source.word_0x96_150 = pending;
                source.life = 1_000_000;
            }
        }
        // `sub_29710` reaches `sub_2AF10` for this model-3/action-25
        // family.  When the pre-pass does not take the action-220
        // fallback, this bounded projection retains the callback's
        // terrain resnap even when movement is disabled.
        // Its `sub_2A6B0` pre-pass, action 216/217 transitions, target
        // selection, collision turns, and deferred child chain remain
        // explicit creature-family residuals until their source state is
        // projected into this arena record.
        if run_terrain_resnap {
            source.position.z = self.terrain.ground_height(source.position);
        }
        if source.action == 25 {
            let target_valid = target.as_ref().is_some_and(|target| {
                target.active
                    && target.life >= 0
                    && target.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
            });
            let mut target_lost = !target_valid;
            if let Some(target) = target.as_ref().filter(|_| target_valid) {
                // `sub_2AF10` has no imported collision result in this
                // bounded seam, so the source's v2 != 3 branch is the
                // ordinary target-tracking path.
                if source.source_byte_3e & 3 == 0 && source.byte_0x46_70 == 0 {
                    let heading = heading_for_positions(source.position, target.position);
                    source.roll = i16::try_from(heading).expect("target heading fits i16");
                }
                // The angle/sprite update and range cutoff are both
                // gated by the source's 0x1f cadence mask. Between those
                // callbacks remc2 retains the current engagement state.
                if source.source_byte_3e & 0x1f == 0 {
                    let heading = heading_for_positions(source.position, target.position);
                    if angle_distance(i32::from(source.yaw), heading) <= MODEL3_TARGET_HALF_ANGLE {
                        source.byte_0x46_70 = 1;
                        source.particle_index = 337;
                    } else {
                        source.byte_0x46_70 = 0;
                        source.particle_index = 315;
                    }
                    target_lost = crate::legacy_sqrt(level2_wrapping_source_distance_3d(
                        source.position,
                        target.position,
                    )) >= MODEL3_TARGET_RANGE;
                }
            }
            if target_lost {
                // `sub_29710`'s target wrapper treats an invalid or
                // distant target as lost: action 217, idle sprite 315,
                // and cleared relation/selector.
                source.action = 217;
                source.particle_index = 315;
                source.byte_0x46_70 = 0;
                source.word_0x96_150 = 0;
            }
        }
    }
}
