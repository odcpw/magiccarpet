//! Canonical campaign objective state.
//!
//! Objective rows used to be copied out of `RuntimeSnapshot` into the
//! campaign carrier.  Keeping the state embedded here lets the runtime and
//! campaign views share one mutable representation while preserving the
//! existing field-style access through `RuntimeSnapshot`'s `Deref` impl.

use crate::native_world::RuntimeObjective;

/// `sub_58F00_game_objectives` owns one objective row in single-player,
/// regardless of how many AI players the mission retains. Only its explicit
/// multiplayer setting admits `NumberOfPlayers` objective owners.
#[must_use]
pub(crate) const fn objective_owner_count(multiplayer: bool, source_player_count: u8) -> usize {
    if multiplayer {
        source_player_count as usize
    } else {
        1
    }
}

/// Source objective condition 8 reads the declared player-array extent and
/// excludes its current owner. Actor life, player colour and records beyond
/// `NumberOfPlayers` do not participate in this predicate. The caller retains
/// the source current-objective gate and common stage-completion epilogue.
#[must_use]
pub(crate) fn all_other_players_banished(
    current_player_slot: usize,
    source_player_count: u8,
    player_states: impl IntoIterator<Item = Option<u8>>,
) -> bool {
    let player_count = usize::from(source_player_count);
    if player_count == 0 || current_player_slot >= player_count {
        return false;
    }
    let mut player_states = player_states.into_iter();
    for slot in 0..player_count {
        let Some(state) = player_states.next() else {
            return false;
        };
        if slot != current_player_slot && state != Some(0) {
            return false;
        }
    }
    true
}

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

impl crate::native_world::Mc2Simulation {
    /// The keyboard producer precedes PresentObjective for both playing and
    /// paused outer frames. Keep the source byte carrier and typed state in
    /// the runtime owner together; clients only submit a held input.
    pub(crate) fn apply_objective_input(&mut self, held: bool) {
        self.session
            .state
            .live
            .core
            .runtime
            .apply_objective_input(held);
    }

    pub(crate) fn clear_objective_textbox(&mut self) {
        self.session
            .state
            .live
            .core
            .runtime
            .clear_objective_textbox();
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{PlayerCommand, level_definition::ObjectiveCondition};

    use crate::campaign::{CampaignSelection, CampaignSession};

    #[test]
    fn objective_authority_uses_mode_instead_of_ai_player_count() {
        assert_eq!(super::objective_owner_count(false, 2), 1);
        assert_eq!(super::objective_owner_count(false, 8), 1);
        assert_eq!(super::objective_owner_count(true, 2), 2);
        assert_eq!(super::objective_owner_count(true, 8), 8);
        assert_eq!(super::objective_owner_count(true, 0), 0);
    }

    #[test]
    fn condition_eight_uses_only_declared_other_player_flags() {
        use super::all_other_players_banished as achieved;

        assert!(achieved(1, 3, [Some(0), Some(7), Some(0), Some(255)]));
        assert!(achieved(0, 1, [None, Some(1)]));
        assert!(!achieved(0, 2, [Some(0), Some(1)]));
        assert!(!achieved(0, 2, [Some(0), None]));
        assert!(!achieved(0, 3, [Some(0), Some(0)]));
        assert!(!achieved(2, 2, [Some(0), Some(0)]));
        assert!(!achieved(0, 0, []));
    }

    #[test]
    fn owner_seven_condition_eight_has_a_read_only_semantic_binding() {
        use crate::campaign::{
            mc2_level_definition::Mc2LevelDefinition, source_facts::decode_building_parameters,
            spec::DecodedCampaignPackage,
        };
        use sim_definition::level_definition::LevelObjectiveConditionAccess;

        let root = std::env::var_os("MC_ORDINARY_CAMPAIGN_PACKAGES").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj")
            },
            PathBuf::from,
        );
        let selection = CampaignSelection::ordinary(7).unwrap();
        let package = DecodedCampaignPackage::decode(root.join("level7"), selection).unwrap();
        let parameters =
            decode_building_parameters(package.member("building-parameters.bin").unwrap()).unwrap();
        let binding = Mc2LevelDefinition::from_authored_bytes(
            package.authored_bytes(),
            &package.campaign_spec.objectives,
            selection,
            Some(&parameters),
        )
        .unwrap();
        let objectives = binding.semantic.objectives();
        assert_eq!(objectives.len(), 1);
        assert_eq!(
            objectives[0].condition(),
            ObjectiveCondition::AllOtherPlayersBanished
        );
        assert_eq!(
            objectives[0].condition_access(),
            LevelObjectiveConditionAccess::ReadOnly
        );
    }

    fn level_two_session() -> crate::native_world::Mc2Simulation {
        let package = std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
            },
            PathBuf::from,
        );
        CampaignSession::open(package, CampaignSelection::LEVEL2)
            .expect("qualified Level 2 package")
            .simulation()
            .clone()
    }

    #[test]
    fn single_player_reach_objective_does_not_advance_rival_objectives() {
        let mut session = level_two_session();
        assert_eq!(session.runtime().player_count, 2);
        let local_color = usize::from(session.players()[0].player_color_index());
        let rival = &session.players()[1];
        assert!(rival.is_ai);
        assert!(rival.active_player_index >= 0);
        let rival_color = usize::from(rival.player_color_index());
        assert_ne!(local_color, rival_color);
        let rival_before = session.runtime().objectives[rival_color];
        assert_eq!(rival_before.stage_selectors[0], 1);
        let target = &session.runtime().stages[0];
        assert_eq!(target.kind, 5, "ordinary first reach-location producer");
        let (target_x, target_y) = (target.axis_x as u16, target.axis_y as u16);
        session.state.position.x = target_x;
        session.state.position.y = target_y;

        session.step_outer_turn(PlayerCommand::default(), 0);

        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. }))
        );
        assert_eq!(
            session.runtime().objectives[local_color].stage_selectors[0],
            2
        );
        assert_eq!(session.runtime().objectives[local_color].text_selector, 1);
        assert_eq!(session.runtime().objectives[rival_color], rival_before);
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
