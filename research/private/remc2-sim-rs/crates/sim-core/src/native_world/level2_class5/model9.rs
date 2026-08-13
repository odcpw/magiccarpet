//! Existing class-5/model-9 action-72 free-movement body.

use super::super::*;

impl Level2EconomyState {
    #[inline]
    pub(super) fn step_authored_class5_model9(&self, source: &mut Level2AuthoredClass5Source) {
        // `sub_2AF10(a1, 1)` first advances the model-9 root, resnaps Z
        // to terrain, and then lets `sub_29A90` settle its linked rows.
        // The qualified Level 2 source seam has no imported collision
        // relation for this root, so this bounded path commits the
        // source-backed free-movement branch. Collision/alternate-yaw
        // selection remains an explicit residual until that relation is
        // available.
        let mut predicted = source.position;
        crate::move_entity(&mut predicted, source.yaw, 0, source.actual_speed);
        predicted.z = self.terrain.ground_height(predicted);
        source.position = predicted;
        source.position.z = self.terrain.ground_height(source.position);
    }
}
