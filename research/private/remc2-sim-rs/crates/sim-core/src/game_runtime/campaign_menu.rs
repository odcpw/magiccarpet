//! Campaign menu ownership after gameplay relinquishes its session.
//!
//! MC2 source chain: sub_46830_main_loop -> NewGameDialog_77350 ->
//! PortalsUpdate_7DD70 -> NewGameDraw_7EAE0 (results state 4, selection state 3).
//! Presentation reads are observational; only guarded commands advance it.

use super::*;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
    ResetCampaign,
    ReturnTitle,
}

/// The source's five-value per-mission results row, retained after the
/// gameplay session is released and overwritten when that mission is replayed.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompletedLevelStatistics {
    pub level_id: SemanticLevelId,
    pub statistics: GameCompletionStatistics,
}

/// Source map saves contain frontier, spell carry and retained results rather
/// than an invented active gameplay session. The package adapter owns the
/// outer receipt and spell carry; this is the runtime's exact menu state.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CampaignSaveState {
    pub definition: GameDefinition,
    pub phase: CampaignMenuPhase,
    pub completed_level_id: Option<SemanticLevelId>,
    pub selected_level_id: Option<SemanticLevelId>,
    pub completed_levels: Vec<SemanticLevelId>,
    pub level_statistics: Vec<CompletedLevelStatistics>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CampaignMenuSnapshot {
    pub phase: CampaignMenuPhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_level_id: Option<SemanticLevelId>,
    pub completed_levels: Vec<SemanticLevelId>,
    pub available_levels: Vec<SemanticLevelId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_level_id: Option<SemanticLevelId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub briefing_level_id: Option<SemanticLevelId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_statistics: Option<GameCompletionStatistics>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub level_statistics: Vec<CompletedLevelStatistics>,
}

pub(super) fn presentation(
    definition: &GameDefinition,
    selected: Option<&GameDefinition>,
    phase: CampaignMenuPhase,
    completed: Option<&SemanticLevelId>,
) -> RuntimePresentationCue {
    let flow = definition
        .runtime_flow()
        .expect("campaign retains its validated flow");
    match phase {
        CampaignMenuPhase::Results => {
            let completed = completed.expect("Results is entered only with an earned completion");
            RuntimePresentationCue {
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
            }
        }
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
            completion_statistics: completed_level.as_ref().and_then(|level| {
                self.level_statistics
                    .iter()
                    .find(|row| &row.level_id == level)
                    .map(|row| row.statistics.clone())
            }),
            level_statistics: self.level_statistics.clone(),
        })
    }

    pub(crate) fn campaign_save_state(&self) -> Option<CampaignSaveState> {
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
        Some(CampaignSaveState {
            definition: definition.as_ref().clone(),
            phase: *phase,
            completed_level_id: completed_level.clone(),
            selected_level_id: selected_definition
                .as_ref()
                .and_then(|definition| definition.runtime_flow())
                .map(|flow| flow.bound_level_id().clone()),
            completed_levels: self.completed_levels.clone(),
            level_statistics: self.level_statistics.clone(),
        })
    }

    pub(crate) fn campaign_session_generation(&self) -> Option<u64> {
        match &self.state {
            RuntimeState::Campaign {
                session_generation, ..
            } => Some(session_generation.get()),
            _ => None,
        }
    }

    /// Publish a validated map/results save directly at the source menu
    /// boundary. Candidate resolution and all validation precede mutation.
    pub(crate) fn restore_campaign_resolved<F>(
        &mut self,
        guard: RuntimeGuard,
        saved: CampaignSaveState,
        resolve_selected: F,
    ) -> Result<RuntimeIdentity, RuntimeTransitionError>
    where
        F: FnOnce(
            &GameDefinition,
            &SemanticLevelId,
        ) -> Result<Arc<GameDefinition>, RuntimeTransitionError>,
    {
        self.validate_guard(guard)
            .map_err(Box::new)
            .map_err(RuntimeTransitionError::Guard)?;
        let RuntimeState::Title {
            session_generation, ..
        } = &self.state
        else {
            return Err(RuntimeTransitionError::InvalidMode {
                expected: RuntimeMode::Title,
                actual: self.identity().mode,
            });
        };
        let session_generation = *session_generation;
        let next_generation = self.next_generation()?;
        saved.definition.verify_identity().map_err(|error| {
            RuntimeTransitionError::InvalidReplay {
                message: error.to_string(),
            }
        })?;
        let flow = saved.definition.runtime_flow().ok_or_else(|| {
            RuntimeTransitionError::InvalidReplay {
                message: "saved campaign menu has no runtime flow".to_owned(),
            }
        })?;
        let mut seen = std::collections::BTreeSet::new();
        if saved.completed_levels.iter().any(|level| {
            !flow.entries().iter().any(|entry| entry.level_id() == level) || !seen.insert(level)
        }) {
            return Err(RuntimeTransitionError::InvalidReplay {
                message: "saved campaign menu has unknown or repeated completed missions"
                    .to_owned(),
            });
        }
        validate_statistics(flow, &saved.completed_levels, &saved.level_statistics)?;
        if saved.completed_level_id.as_ref().is_some_and(|level| {
            !saved.completed_levels.contains(level) || flow.bound_level_id() != level
        }) || (saved.phase == CampaignMenuPhase::Results && saved.completed_level_id.is_none())
        {
            return Err(RuntimeTransitionError::InvalidReplay {
                message: "saved results do not bind an earned completed mission".to_owned(),
            });
        }
        if saved.phase == CampaignMenuPhase::MissionSelect {
            let frontier = flow
                .entries()
                .iter()
                .position(|entry| !saved.completed_levels.contains(entry.level_id()))
                .map_or(flow.entries().len(), |index| index + 1);
            if saved.selected_level_id.as_ref().is_some_and(|level| {
                !flow
                    .entries()
                    .iter()
                    .take(frontier)
                    .any(|entry| entry.level_id() == level)
            }) {
                return Err(RuntimeTransitionError::InvalidReplay {
                    message: "saved selected mission is beyond the earned frontier".to_owned(),
                });
            }
        } else {
            let destination = flow
                .entries()
                .iter()
                .find(|entry| entry.level_id() == flow.bound_level_id())
                .expect("validated flow binds its current mission")
                .completion_destination();
            let matches_destination = match (destination, &saved.selected_level_id) {
                (RuntimeFlowDestination::Level { level_id }, Some(selected)) => {
                    level_id == selected
                }
                (RuntimeFlowDestination::Title { .. }, None) => true,
                _ => false,
            };
            if !matches_destination {
                return Err(RuntimeTransitionError::InvalidReplay {
                    message: "saved results bind the wrong completion destination".to_owned(),
                });
            }
        }
        let selected_definition = saved
            .selected_level_id
            .as_ref()
            .map(|level| {
                let selected = resolve_selected(&saved.definition, level)?;
                validate_selected(&saved.definition, &selected, level)?;
                Ok::<_, RuntimeTransitionError>(selected)
            })
            .transpose()?;
        self.state = RuntimeState::Campaign {
            definition: Arc::new(saved.definition),
            selected_definition,
            session_generation,
            phase: saved.phase,
            completed_level: saved.completed_level_id,
        };
        self.completed_levels = saved.completed_levels;
        self.level_statistics = saved.level_statistics;
        self.generation = next_generation;
        Ok(self.identity())
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
            completed_level,
        } = &mut self.state
        else {
            unreachable!()
        };
        match command {
            CampaignMenuCommand::ResetCampaign if *phase == CampaignMenuPhase::MissionSelect => {
                let entry = definition.runtime_flow().unwrap().entry_level_id().clone();
                let selected = resolve(&entry)?;
                validate_selected(definition, &selected, &entry)?;
                *definition = Arc::clone(&selected);
                *selected_definition = Some(selected);
                *completed_level = None;
                self.completed_levels.clear();
                self.level_statistics.clear();
            }
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
                    origin: LoadingOrigin::Campaign {
                        definition: Arc::clone(definition),
                        selected_definition: selected_definition.clone(),
                        phase: *phase,
                        completed_level: completed_level.clone(),
                    },
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

pub(super) fn validate_statistics(
    flow: &RuntimeFlowDefinition,
    completed_levels: &[SemanticLevelId],
    records: &[CompletedLevelStatistics],
) -> Result<(), RuntimeTransitionError> {
    let mut seen = std::collections::BTreeSet::new();
    for record in records {
        let stats = &record.statistics;
        if !completed_levels.contains(&record.level_id)
            || !flow
                .entries()
                .iter()
                .any(|entry| entry.level_id() == &record.level_id)
            || !seen.insert(&record.level_id)
            || !valid_percentages(stats)
        {
            return Err(RuntimeTransitionError::InvalidReplay {
                message:
                    "saved completion statistics have an invalid mission, duplicate or percentage"
                        .to_owned(),
            });
        }
    }
    Ok(())
}

pub(super) fn valid_percentages(stats: &GameCompletionStatistics) -> bool {
    [
        stats.spells_found_percent,
        stats.hit_accuracy_percent,
        stats.creatures_killed_percent,
        stats.mana_collected_percent,
    ]
    .iter()
    .all(|value| (0..=100).contains(value))
}

pub(super) fn validate_selected(
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
    if !candidate.continues_catalog(flow) || candidate.bound_level_id() != level_id {
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
            "mc-bhq8-level1-collision-v5-night",
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
            level_statistics: vec![CompletedLevelStatistics {
                level_id: first.clone(),
                statistics: GameCompletionStatistics {
                    spells_found_percent: 50,
                    hit_accuracy_percent: 60,
                    creatures_killed_percent: 70,
                    mana_collected_percent: 80,
                    time_taken_seconds: 90,
                },
            }],
            state: RuntimeState::Campaign {
                definition: Arc::clone(&definitions[0]),
                selected_definition: Some(Arc::clone(&definitions[1])),
                session_generation: NonZeroU64::new(2).unwrap(),
                phase: CampaignMenuPhase::Results,
                completed_level: Some(first.clone()),
            },
        };
        let results = runtime.identity();
        let saved = runtime.campaign_save_state().unwrap();
        let saved: CampaignSaveState =
            serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        let mut restored = GameRuntime::<Mc2CampaignBackend>::title(772).unwrap();
        let restore_guard = restored.identity().guard();
        restored
            .restore_campaign_resolved(restore_guard, saved.clone(), |_, level| {
                Ok(definitions
                    .iter()
                    .find(|definition| definition.runtime_flow().unwrap().bound_level_id() == level)
                    .unwrap()
                    .clone())
            })
            .unwrap();
        assert!(
            restored.session().is_none(),
            "map restore must not create gameplay"
        );
        assert_eq!(restored.identity().mode, RuntimeMode::Campaign);
        assert_eq!(restored.campaign_save_state().unwrap(), saved);
        assert_eq!(
            restored
                .campaign_menu_snapshot()
                .unwrap()
                .completion_statistics,
            Some(runtime.level_statistics[0].statistics.clone())
        );
        assert_eq!(restored.campaign_session_generation(), Some(1));
        let mut duplicate = saved;
        duplicate
            .level_statistics
            .push(duplicate.level_statistics[0].clone());
        let mut rejected = GameRuntime::<Mc2CampaignBackend>::title(773).unwrap();
        let before_rejection = rejected.identity();
        assert!(
            rejected
                .restore_campaign_resolved(
                    before_rejection.guard(),
                    duplicate,
                    |_, _| unreachable!("invalid statistics must reject before package resolution")
                )
                .is_err()
        );
        assert_eq!(
            rejected.identity(),
            before_rejection,
            "invalid map saves publish no state"
        );
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
        let stale_cancel = runtime.cancel_loading(map.guard());
        assert!(matches!(
            stale_cancel,
            Err(RuntimeTransitionError::Guard(_))
        ));
        assert_eq!(runtime.identity(), loading);
        let restored_map = runtime.cancel_loading(loading.guard()).unwrap();
        assert_eq!(restored_map.mode, RuntimeMode::Campaign);
        assert_eq!(restored_map.campaign_menu, map.campaign_menu);
        assert!(runtime.session().is_none());
        assert!(runtime.cancel_loading(loading.guard()).is_err());
        assert_eq!(runtime.identity(), restored_map);
        let loading = resolver
            .campaign_menu_command(
                &mut runtime,
                restored_map.guard(),
                CampaignMenuCommand::StartMission,
            )
            .unwrap();
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
            level_statistics: Vec::new(),
            state: RuntimeState::Campaign {
                definition: Arc::clone(&definitions[2]),
                selected_definition: None,
                session_generation: NonZeroU64::new(4).unwrap(),
                phase: CampaignMenuPhase::Results,
                completed_level: Some(third),
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

        // The retained three-mission save adopts the installed fourth mission
        // at its ordinary interlevel selection. This stages the menu boundary,
        // not a claim that preceding gameplay objectives were earned here.
        let expanded = Mc2RuntimePackageResolver::load(
            paths
                .iter()
                .cloned()
                .chain(std::iter::once(
                    generated.join("mc-bhq6-ordinary-20261025-zclu5fjj/level4"),
                ))
                .map(Mc2CampaignInputs::new),
        )
        .unwrap();
        let first = definitions[0]
            .runtime_flow()
            .unwrap()
            .bound_level_id()
            .clone();
        let mut retained = GameRuntime::<Mc2CampaignBackend> {
            frontend_media: super::media::FrontendMedia::default(),
            player_preferences: super::preferences::PreferenceStore::default(),
            runtime_id: 773,
            generation: 4,
            completed_levels: vec![first.clone()],
            level_statistics: Vec::new(),
            state: RuntimeState::Campaign {
                definition: Arc::clone(&definitions[0]),
                selected_definition: None,
                session_generation: NonZeroU64::new(2).unwrap(),
                phase: CampaignMenuPhase::Results,
                completed_level: Some(first),
            },
        };
        let guard = retained.identity().guard();
        let map = expanded
            .campaign_menu_command(&mut retained, guard, CampaignMenuCommand::DismissResults)
            .unwrap();
        let loading = expanded
            .campaign_menu_command(
                &mut retained,
                map.guard(),
                CampaignMenuCommand::StartMission,
            )
            .unwrap();
        expanded
            .open_pending(&mut retained, loading.guard(), 773)
            .unwrap();
        let active = retained
            .session()
            .unwrap()
            .definition()
            .runtime_flow()
            .unwrap();
        assert_eq!(
            active.bound_level_id().as_str(),
            "level.magic_carpet_2.payahandra_ruins"
        );
        assert_eq!(active.entries().len(), 4);
    }
}
