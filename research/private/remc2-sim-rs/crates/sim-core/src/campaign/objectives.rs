//! Canonical campaign objective state.
//!
//! Objective rows used to be copied out of `RuntimeSnapshot` into the
//! campaign carrier.  Keeping the state embedded here lets the runtime and
//! campaign views share one mutable representation while preserving the
//! existing field-style access through `RuntimeSnapshot`'s `Deref` impl.

use crate::native_world::RuntimeObjective;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectiveState {
    pub stage_index: u8,
    /// `byte_0x36E02`, consumed by remc2's objective-presentation service.
    pub objective_presentation_phase: u8,
    /// Remaining objective-textbox presentation ticks.
    pub objective_textbox_ticks: u8,
    pub objectives: Box<[RuntimeObjective]>,
}

impl ObjectiveState {
    #[must_use]
    pub const fn new(
        stage_index: u8,
        objective_presentation_phase: u8,
        objective_textbox_ticks: u8,
        objectives: Box<[RuntimeObjective]>,
    ) -> Self {
        Self {
            stage_index,
            objective_presentation_phase,
            objective_textbox_ticks,
            objectives,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{PlayerCommand, native_world::NativeWorld};

    fn level_two_session() -> crate::native_world::Level2ResetSession {
        let package = std::env::var_os("MC_LEVEL2_PACKAGE_V5").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/l2-debug3.fJ3PaM/level2")
            },
            PathBuf::from,
        );
        NativeWorld::inspect_level2(package)
            .expect("qualified Level 2 package")
            .level2_reset_session()
            .expect("qualified Level 2 session")
    }

    #[test]
    fn cf08_following_turn_visibility_matches_omega() {
        let mut session = level_two_session();
        let target_index = match session.runtime().stages[1].relation {
            crate::native_world::RuntimeRelation::LiveEntity(index) => index,
            _ => panic!("qualified objective stage 1 binds a live player"),
        };
        session
            .players
            .iter_mut()
            .find(|player| player.entity_index == target_index)
            .expect("objective target player")
            .banishment_state_0x006 = Some(0);
        let row11_position = session
            .mission()
            .authored_features()
            .iter()
            .find(|feature| feature.index == 11)
            .expect("qualified row-11 model-32 producer")
            .world_position(0);
        session.state.position = row11_position;

        assert_eq!(session.runtime().objectives[0].stage_selectors[1], 1);
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(session.runtime().objectives[0].stage_selectors[1], 2);
        let row206 = session
            .economy()
            .authored_model32()
            .iter()
            .find(|source| source.authored_index == 206)
            .expect("ordinary row-11 route materializes row 206");
        assert!(
            !row206.fired,
            "same-turn model-32 cannot see the epilogue write"
        );
        assert_eq!(session.runtime().objectives[0].done, 0);

        session.step_outer_turn(PlayerCommand::default(), 0);
        let row206 = session
            .economy()
            .authored_model32()
            .iter()
            .find(|source| source.authored_index == 206)
            .expect("row 206 remains available at its following-turn callback");
        assert!(row206.fired);
        assert_eq!(
            session.runtime().objectives[0].done,
            0,
            "the post-UpdateEntities objective phase consumes the frame-local pause"
        );

        let mut early = level_two_session();
        early.state.position = row11_position;
        early.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(early.runtime().objectives[0].stage_selectors[1], 1);
        assert!(
            !early
                .economy()
                .authored_model32()
                .iter()
                .any(|source| source.authored_index == 206 && source.fired)
        );
    }
}
