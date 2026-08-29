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
    /// Frame-local palette cadence used by the objective-presentation gate.
    /// The source initializes this normalized carrier at one and saturates it
    /// at three before presenting a pending objective.
    pub objective_presentation_palette_mod: u8,
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
            objective_presentation_palette_mod: 1,
            objective_textbox_ticks,
            objectives,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{PlayerCommand, level_definition::ObjectiveCondition};

    use crate::campaign::{CampaignSelection, CampaignSession};

    fn level_two_session() -> crate::native_world::Level2ResetSession {
        let package = std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
            },
            PathBuf::from,
        );
        CampaignSession::open(package, CampaignSelection::LEVEL2)
            .expect("qualified Level 2 package")
            .level2_compatibility()
            .clone()
    }

    #[test]
    fn level_two_rival_wizard_semantic_target_drives_following_turn_objective_gate() {
        // No-Claim: this gate stages Nyphur's already-proven terminal player
        // state and the row-11 trigger position. It proves the qualified
        // semantic target binding, ordinary objective pass, and following-turn
        // model-32 consumer only; it does not prove organic combat/banishment,
        // alternate rival authoring, later objectives, transition, or clients.
        let mut session = level_two_session();
        let objective_id =
            sim_definition::adapter::stable_objective_id("objective.defeat.nyphur").unwrap();
        let target =
            sim_definition::adapter::stable_rival_wizard_id("rival_wizard.nyphur").unwrap();
        assert_eq!(
            session
                .semantic_level_definition()
                .unwrap()
                .objective(&objective_id)
                .unwrap()
                .condition(),
            ObjectiveCondition::DefeatRivalWizard(target.clone())
        );
        let target_index = match session.runtime().stages[1].relation {
            crate::native_world::RuntimeRelation::LiveEntity(index) => index,
            _ => panic!("qualified objective stage 1 binds a live player"),
        };
        assert_eq!(target_index, 112, "qualified Nyphur entity binding");
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

        let mut mismatched = level_two_session();
        mismatched
            .players
            .iter_mut()
            .find(|player| player.entity_index == target_index)
            .expect("objective target player")
            .banishment_state_0x006 = Some(0);
        let local_player_index = mismatched.players[0].entity_index;
        mismatched.replace_rival_wizard_binding_for_test(target, local_player_index);
        mismatched.state.position = row11_position;
        mismatched.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(
            mismatched.runtime().objectives[0].stage_selectors[1],
            1,
            "the raw stage target cannot override the semantic Nyphur binding"
        );
    }
}
