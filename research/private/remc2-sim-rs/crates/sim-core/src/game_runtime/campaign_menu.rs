//! Campaign menu ownership after gameplay relinquishes its session.
//!
//! MC2 source chain: sub_46830_main_loop -> NewGameDialog_77350 ->
//! PortalsUpdate_7DD70 -> NewGameDraw_7EAE0 (results state 4, selection state 3).
//! Presentation reads are observational; only guarded commands advance it.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignMenuPhase {
    Results,
    MissionSelect,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum CampaignMenuCommand {
    DismissResults,
    SelectMission { level_id: SemanticLevelId },
    StartMission,
    ReturnTitle,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CampaignMenuSnapshot {
    pub phase: CampaignMenuPhase,
    pub completed_level_id: SemanticLevelId,
    pub completed_levels: Vec<SemanticLevelId>,
    pub available_levels: Vec<SemanticLevelId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_level_id: Option<SemanticLevelId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub briefing_level_id: Option<SemanticLevelId>,
}

pub(super) fn presentation(
    definition: &GameDefinition,
    selected: Option<&GameDefinition>,
    phase: CampaignMenuPhase,
    completed: &SemanticLevelId,
) -> RuntimePresentationCue {
    let flow = definition
        .runtime_flow()
        .expect("campaign retains its validated flow");
    match phase {
        CampaignMenuPhase::Results => RuntimePresentationCue {
            cue_id: flow
                .presentation()
                .entry(completed)
                .unwrap()
                .completion_cue()
                .clone(),
            phase: RuntimePresentationPhase::LevelCompletion,
            level_id: Some(completed.clone()),
            completion_destination: Some(
                flow.entries()
                    .iter()
                    .find(|entry| entry.level_id() == completed)
                    .unwrap()
                    .completion_destination()
                    .clone(),
            ),
        },
        CampaignMenuPhase::MissionSelect => RuntimePresentationCue {
            cue_id: SemanticPresentationCueId::parse("presentation.campaign.map").unwrap(),
            phase: RuntimePresentationPhase::CampaignMap,
            level_id: selected
                .and_then(GameDefinition::runtime_flow)
                .map(|flow| flow.bound_level_id().clone()),
            completion_destination: None,
        },
    }
}

impl<B: GameSessionBackend> GameRuntime<B> {
    #[must_use]
    pub fn campaign_menu_snapshot(&self) -> Option<CampaignMenuSnapshot> {
        let RuntimeState::Campaign {
            definition,
            selected_definition,
            phase,
            completed_level,
            ..
        } = &self.state
        else {
            return None;
        };
        let flow = definition.runtime_flow().unwrap();
        let briefing_level_id = flow
            .entries()
            .iter()
            .find(|entry| !self.completed_levels.contains(entry.level_id()))
            .map(|entry| entry.level_id().clone());
        // NewGameDraw_7EAE0 stops portal hit testing at the first active
        // (unfinished) portal. Do not expose later portals just because a
        // direct-entry diagnostic happened to complete a later mission.
        let frontier = briefing_level_id
            .as_ref()
            .and_then(|level| {
                flow.entries()
                    .iter()
                    .position(|entry| entry.level_id() == level)
            })
            .map_or(flow.entries().len(), |index| index + 1);
        let available_levels = flow
            .entries()
            .iter()
            .take(frontier)
            .map(|entry| entry.level_id().clone())
            .collect();
        Some(CampaignMenuSnapshot {
            phase: *phase,
            completed_level_id: completed_level.clone(),
            completed_levels: self.completed_levels.clone(),
            available_levels,
            selected_level_id: (*phase == CampaignMenuPhase::MissionSelect)
                .then(|| {
                    selected_definition
                        .as_ref()
                        .and_then(|definition| definition.runtime_flow())
                        .map(|flow| flow.bound_level_id().clone())
                })
                .flatten(),
            briefing_level_id,
        })
    }

    /// A semantic mission choice is resolved by the installed package owner;
    /// clients never provide a definition or owner path.
    pub(crate) fn campaign_menu_command_resolved<F>(
        &mut self,
        guard: RuntimeGuard,
        command: CampaignMenuCommand,
        resolve: F,
    ) -> Result<RuntimeIdentity, RuntimeTransitionError>
    where
        F: FnOnce(&SemanticLevelId) -> Result<Arc<GameDefinition>, RuntimeTransitionError>,
    {
        self.validate_guard(guard)
            .map_err(Box::new)
            .map_err(RuntimeTransitionError::Guard)?;
        let snapshot =
            self.campaign_menu_snapshot()
                .ok_or_else(|| RuntimeTransitionError::InvalidMode {
                    expected: RuntimeMode::Campaign,
                    actual: self.identity().mode,
                })?;
        let next_generation = self.next_generation()?;
        let RuntimeState::Campaign {
            definition,
            selected_definition,
            session_generation,
            phase,
            ..
        } = &mut self.state
        else {
            unreachable!()
        };
        match command {
            CampaignMenuCommand::DismissResults if *phase == CampaignMenuPhase::Results => {
                // The map focuses its first unfinished portal, independently
                // of which completed portal the results were showing.
                *selected_definition = match &snapshot.briefing_level_id {
                    Some(level_id) => {
                        let selected = resolve(level_id)?;
                        validate_selected(definition, &selected, level_id)?;
                        Some(selected)
                    }
                    None => None,
                };
                *phase = CampaignMenuPhase::MissionSelect;
            }
            CampaignMenuCommand::SelectMission { level_id }
                if *phase == CampaignMenuPhase::MissionSelect =>
            {
                if !snapshot.available_levels.contains(&level_id) {
                    return Err(RuntimeTransitionError::InvalidRuntimeFlow {
                        message: "mission is not unlocked in this campaign".to_owned(),
                    });
                }
                let selected = resolve(&level_id)?;
                validate_selected(definition, &selected, &level_id)?;
                *selected_definition = Some(selected);
            }
            CampaignMenuCommand::StartMission if *phase == CampaignMenuPhase::MissionSelect => {
                let selected = selected_definition.as_ref().ok_or_else(|| {
                    RuntimeTransitionError::InvalidRuntimeFlow {
                        message: "campaign has no selected installed mission".to_owned(),
                    }
                })?;
                self.state = RuntimeState::Loading {
                    definition: Arc::clone(selected),
                    session_generation: *session_generation,
                };
            }
            CampaignMenuCommand::ReturnTitle => {
                self.state = RuntimeState::Title {
                    session_generation: *session_generation,
                    presentation_cue: definition
                        .runtime_flow()
                        .unwrap()
                        .presentation()
                        .title_cue()
                        .clone(),
                };
                self.frontend_media.return_to_title();
            }
            _ => {
                return Err(RuntimeTransitionError::InvalidRuntimeFlow {
                    message: "campaign action is unavailable in this menu phase".to_owned(),
                });
            }
        }
        self.generation = next_generation;
        Ok(self.identity())
    }
}

fn validate_selected(
    definition: &GameDefinition,
    selected: &GameDefinition,
    level_id: &SemanticLevelId,
) -> Result<(), RuntimeTransitionError> {
    selected
        .verify_identity()
        .map_err(|error| RuntimeTransitionError::InvalidNextDefinition {
            message: error.to_string(),
        })?;
    let flow = definition.runtime_flow().unwrap();
    let candidate =
        selected
            .runtime_flow()
            .ok_or_else(|| RuntimeTransitionError::InvalidRuntimeFlow {
                message: "selected mission has no runtime flow".to_owned(),
            })?;
    if candidate.game_id() != flow.game_id()
        || candidate.entry_level_id() != flow.entry_level_id()
        || candidate.entries() != flow.entries()
        || candidate.presentation() != flow.presentation()
        || candidate.bound_level_id() != level_id
    {
        return Err(RuntimeTransitionError::InvalidRuntimeFlow {
            message: "selected mission does not belong to the active campaign".to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign::backend::{
        Mc2CampaignBackend, Mc2CampaignInputs, Mc2RuntimePackageResolver, load_mc2_campaign,
    };
    use crate::game_definition::default_catalog;

    #[test]
    fn results_require_map_selection_before_loading_and_terminal_results_remain_on_map() {
        let generated = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let paths = [
            "mc-cf15-level1-package-v3-current",
            "level2-day-block32-campaign-v2",
            "level3-day-block32-campaign-recipe-v2",
        ]
        .map(|name| generated.join(name));
        let resolver =
            Mc2RuntimePackageResolver::load(paths.iter().map(Mc2CampaignInputs::new)).unwrap();
        let catalog = default_catalog().unwrap();
        let definitions = paths
            .iter()
            .map(|path| Arc::new(load_mc2_campaign(path, &catalog).unwrap()))
            .collect::<Vec<_>>();
        let first = definitions[0]
            .runtime_flow()
            .unwrap()
            .bound_level_id()
            .clone();
        let second = definitions[1]
            .runtime_flow()
            .unwrap()
            .bound_level_id()
            .clone();
        let third = definitions[2]
            .runtime_flow()
            .unwrap()
            .bound_level_id()
            .clone();
        // Stage only the menu's input boundary. This test exercises real
        // package resolution/loading, not preceding gameplay completion.
        let mut runtime = GameRuntime::<Mc2CampaignBackend> {
            frontend_media: super::media::FrontendMedia::default(),
            player_preferences: super::preferences::PreferenceStore::default(),
            runtime_id: 771,
            generation: 4,
            completed_levels: vec![first.clone()],
            state: RuntimeState::Campaign {
                definition: Arc::clone(&definitions[0]),
                selected_definition: Some(Arc::clone(&definitions[1])),
                session_generation: NonZeroU64::new(2).unwrap(),
                phase: CampaignMenuPhase::Results,
                completed_level: first.clone(),
            },
        };
        let results = runtime.identity();
        assert_eq!(
            runtime.identity(),
            results,
            "polling does not dismiss results"
        );
        assert!(
            resolver
                .open_pending(&mut runtime, results.guard(), 771)
                .is_err()
        );
        assert!(
            resolver
                .campaign_menu_command(
                    &mut runtime,
                    results.guard(),
                    CampaignMenuCommand::StartMission
                )
                .is_err()
        );
        assert_eq!(runtime.identity(), results);
        let map = resolver
            .campaign_menu_command(
                &mut runtime,
                results.guard(),
                CampaignMenuCommand::DismissResults,
            )
            .unwrap();
        assert_eq!(map.mode, RuntimeMode::Campaign);
        assert_eq!(
            map.campaign_menu.as_ref().unwrap().available_levels,
            vec![first, second.clone()]
        );
        assert!(
            resolver
                .campaign_menu_command(
                    &mut runtime,
                    map.guard(),
                    CampaignMenuCommand::SelectMission {
                        level_id: third.clone()
                    }
                )
                .is_err()
        );
        assert_eq!(
            runtime.identity(),
            map,
            "locked mission rejection is atomic"
        );
        assert!(
            resolver
                .campaign_menu_command(
                    &mut runtime,
                    results.guard(),
                    CampaignMenuCommand::StartMission
                )
                .is_err()
        );
        let loading = resolver
            .campaign_menu_command(&mut runtime, map.guard(), CampaignMenuCommand::StartMission)
            .unwrap();
        assert_eq!(loading.mode, RuntimeMode::Loading);
        assert_eq!(
            loading
                .pending_definition
                .as_ref()
                .unwrap()
                .runtime_flow
                .as_ref()
                .unwrap()
                .bound_level_id(),
            &second
        );
        let playing = resolver
            .open_pending(&mut runtime, loading.guard(), 771)
            .unwrap();
        assert_eq!(playing.session.as_ref().unwrap().generation, 2);
        assert_eq!(
            playing
                .active_runtime_flow
                .as_ref()
                .unwrap()
                .bound_level_id(),
            &second
        );

        let mut terminal = GameRuntime::<Mc2CampaignBackend> {
            frontend_media: super::media::FrontendMedia::default(),
            player_preferences: super::preferences::PreferenceStore::default(),
            runtime_id: 772,
            generation: 8,
            completed_levels: resolver
                .flow()
                .entries()
                .iter()
                .map(|entry| entry.level_id().clone())
                .collect(),
            state: RuntimeState::Campaign {
                definition: Arc::clone(&definitions[2]),
                selected_definition: None,
                session_generation: NonZeroU64::new(4).unwrap(),
                phase: CampaignMenuPhase::Results,
                completed_level: third,
            },
        };
        let guard = terminal.identity().guard();
        let map = resolver
            .campaign_menu_command(&mut terminal, guard, CampaignMenuCommand::DismissResults)
            .unwrap();
        assert_eq!(
            map.mode,
            RuntimeMode::Campaign,
            "Level 3 does not invent a game ending"
        );
        assert_eq!(
            map.campaign_menu.as_ref().unwrap().available_levels.len(),
            3
        );
        assert!(
            map.campaign_menu
                .as_ref()
                .unwrap()
                .briefing_level_id
                .is_none()
        );
        assert!(
            resolver
                .campaign_menu_command(
                    &mut terminal,
                    map.guard(),
                    CampaignMenuCommand::StartMission
                )
                .is_err()
        );
        let title = resolver
            .campaign_menu_command(&mut terminal, map.guard(), CampaignMenuCommand::ReturnTitle)
            .unwrap();
        assert_eq!(title.mode, RuntimeMode::Title);
        assert!(title.session.is_none());
    }
}
