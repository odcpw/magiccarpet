//! Secret-mission parent retention and return-state ownership.
//!
//! The source controller enters a secret level during an ordinary mission,
//! then returns to the campaign map. This state retains the selected parent
//! and ordinary frontier across that detour; it never treats a secret result
//! as completion of the parent mission.

use super::{
    CompletedLevelStatistics, RuntimeTransitionError,
    campaign_menu::{self, CampaignMenuPhase, CampaignSaveState},
};
use crate::game_definition::{RuntimeFlowDefinition, SemanticLevelId};
use serde::{Deserialize, Serialize};

pub(crate) mod receipt;

/// Source-backed outcome from the actual secret gameplay session.
///
/// The owner must derive this from the live session's completion/return
/// result; it is not an input command or a client-settable completion flag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SecretMissionOutcome {
    Completed,
    Abandoned,
}

/// Parent campaign state retained while a secret mission is open.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SecretMissionReturnFrame {
    parent_campaign: CampaignSaveState,
    origin_level_id: SemanticLevelId,
    secret_level_id: SemanticLevelId,
}

impl SecretMissionReturnFrame {
    #[must_use]
    pub(crate) const fn parent_campaign(&self) -> &CampaignSaveState {
        &self.parent_campaign
    }

    #[must_use]
    pub(crate) const fn origin_level_id(&self) -> &SemanticLevelId {
        &self.origin_level_id
    }

    #[must_use]
    pub(crate) const fn secret_level_id(&self) -> &SemanticLevelId {
        &self.secret_level_id
    }
}

/// Campaign-owned secret progress plus the one active parent return frame.
///
/// The frame is included in save receipts by the package/save owner. Its
/// semantic IDs and retained parent state let a resumed secret session return
/// to the correct origin without reconstructing or granting frontier state.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SecretFlowState {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    discovered_secret_levels: Vec<SemanticLevelId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    completed_secret_levels: Vec<SemanticLevelId>,
    /// sub_82AB0's separate five-value row per secret portal. A successful
    /// replay replaces its row; failure does not create or erase one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    completed_secret_statistics: Vec<CompletedLevelStatistics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    return_frame: Option<SecretMissionReturnFrame>,
}

impl SecretFlowState {
    #[must_use]
    pub(crate) fn discovered_secret_levels(&self) -> &[SemanticLevelId] {
        &self.discovered_secret_levels
    }

    #[must_use]
    pub(crate) fn completed_secret_levels(&self) -> &[SemanticLevelId] {
        &self.completed_secret_levels
    }

    #[must_use]
    pub(crate) fn completed_secret_statistics(&self) -> &[CompletedLevelStatistics] {
        &self.completed_secret_statistics
    }

    /// Record only an actual completion's statistics after its secret result
    /// has been accepted. `sub_82AB0` overwrites the same source portal row.
    pub(crate) fn record_completed_statistics(
        &mut self,
        level: &SemanticLevelId,
        statistics: super::GameCompletionStatistics,
    ) -> Result<(), RuntimeTransitionError> {
        if !self.completed_secret_levels.contains(level)
            || !campaign_menu::valid_percentages(&statistics)
        {
            return Err(secret_replay_error(
                "secret completion statistics have an invalid mission or percentage",
            ));
        }
        let record = CompletedLevelStatistics {
            level_id: level.clone(),
            statistics,
        };
        if let Some(previous) = self
            .completed_secret_statistics
            .iter_mut()
            .find(|row| &row.level_id == level)
        {
            *previous = record;
        } else {
            self.completed_secret_statistics.push(record);
        }
        Ok(())
    }

    /// An unfinished discovery is eligible for its game's parent-replay rule.
    /// The adapter still resolves the selected parent and destination in its
    /// validated catalog; active secret sessions cannot start a second route.
    #[must_use]
    pub(crate) fn is_discovered_unfinished(&self, level: &SemanticLevelId) -> bool {
        self.return_frame.is_none()
            && self.discovered_secret_levels.contains(level)
            && !self.completed_secret_levels.contains(level)
    }

    #[must_use]
    pub(crate) fn return_frame(&self) -> Option<&SecretMissionReturnFrame> {
        self.return_frame.as_ref()
    }

    /// Re-enter a secret portal already discovered on the campaign map.
    ///
    /// The compatibility catalog may be bound to a different ordinary
    /// mission than the selected portal origin. Rebind an exact clone of that
    /// catalog to the origin before reusing the guarded entry checks.
    pub(crate) fn enter_discovered_secret(
        &mut self,
        parent_catalog: &RuntimeFlowDefinition,
        parent_campaign: CampaignSaveState,
        secret_level_id: SemanticLevelId,
    ) -> Result<(), RuntimeTransitionError> {
        if self.return_frame.is_some() {
            return Err(secret_flow_error(
                "a secret mission return frame is already active",
            ));
        }
        if !self.discovered_secret_levels.contains(&secret_level_id) {
            return Err(secret_flow_error(format!(
                "secret mission {} has not been discovered",
                secret_level_id.as_str()
            )));
        }

        let active_entry = parent_catalog
            .secret_entry(&secret_level_id)
            .ok_or_else(|| {
                secret_flow_error(format!(
                    "{} is not an installed secret destination",
                    secret_level_id.as_str()
                ))
            })?;
        let origin_level_id = active_entry.origin_level_id().clone();
        if parent_campaign.selected_level_id.as_ref() != Some(&origin_level_id) {
            return Err(secret_flow_error(
                "selected campaign mission does not match the discovered secret origin",
            ));
        }

        let origin_catalog = RuntimeFlowDefinition::new_with_secret_entries(
            parent_catalog.game_id().clone(),
            origin_level_id,
            parent_catalog.entry_level_id().clone(),
            parent_catalog.entries().to_vec(),
            parent_catalog.secret_entries().to_vec(),
            parent_catalog.presentation().clone(),
        )
        .map_err(|error| {
            secret_flow_error(format!(
                "secret parent catalog could not be rebound: {error}"
            ))
        })?;

        self.enter_secret(&origin_catalog, parent_campaign, secret_level_id)
    }

    /// Retain the exact selected parent campaign before loading an observed
    /// secret destination. Re-entering a completed secret is permitted, as in
    /// the source map, while a second concurrent secret frame is rejected.
    pub(crate) fn enter_secret(
        &mut self,
        active_parent_flow: &RuntimeFlowDefinition,
        parent_campaign: CampaignSaveState,
        secret_level_id: SemanticLevelId,
    ) -> Result<(), RuntimeTransitionError> {
        if self.return_frame.is_some() {
            return Err(secret_flow_error(
                "a secret mission return frame is already active",
            ));
        }
        self.validate_for_flow(active_parent_flow)?;

        let active_entry = active_parent_flow
            .secret_entry(&secret_level_id)
            .ok_or_else(|| {
                secret_flow_error(format!(
                    "{} is not an installed secret destination",
                    secret_level_id.as_str()
                ))
            })?;
        let parent_catalog = parent_campaign
            .definition
            .runtime_flow()
            .ok_or_else(|| secret_flow_error("retained parent has no runtime flow"))?;
        validate_parent_campaign(&parent_campaign, parent_catalog)?;

        if !active_parent_flow.continues_catalog(parent_catalog)
            || parent_catalog.secret_entry(&secret_level_id) != Some(active_entry)
            || active_parent_flow.bound_level_id() != active_entry.origin_level_id()
            || parent_campaign.selected_level_id.as_ref() != Some(active_entry.origin_level_id())
        {
            return Err(secret_flow_error(
                "secret destination does not match the live parent and retained map selection",
            ));
        }

        let frame = SecretMissionReturnFrame {
            parent_campaign,
            origin_level_id: active_entry.origin_level_id().clone(),
            secret_level_id: secret_level_id.clone(),
        };
        if !self.discovered_secret_levels.contains(&secret_level_id) {
            self.discovered_secret_levels.push(secret_level_id);
        }
        self.return_frame = Some(frame);
        Ok(())
    }

    /// Validate persisted progress against the package catalog bound by the
    /// active runtime. A bound secret requires its retained parent frame; an
    /// ordinary package cannot carry an unrelated active secret frame.
    pub(crate) fn validate_for_flow(
        &self,
        flow: &RuntimeFlowDefinition,
    ) -> Result<(), RuntimeTransitionError> {
        let mut discovered = std::collections::BTreeSet::new();
        if self
            .discovered_secret_levels
            .iter()
            .any(|level| flow.secret_entry(level).is_none() || !discovered.insert(level))
        {
            return Err(secret_replay_error(
                "saved secret discovery contains an unknown or repeated mission",
            ));
        }

        let mut completed = std::collections::BTreeSet::new();
        if self.completed_secret_levels.iter().any(|level| {
            flow.secret_entry(level).is_none()
                || !discovered.contains(level)
                || !completed.insert(level)
        }) {
            return Err(secret_replay_error(
                "saved secret completion is unknown, undiscovered or repeated",
            ));
        }

        let mut statistics_levels = std::collections::BTreeSet::new();
        if self.completed_secret_statistics.iter().any(|record| {
            !self.completed_secret_levels.contains(&record.level_id)
                || flow.secret_entry(&record.level_id).is_none()
                || !statistics_levels.insert(&record.level_id)
                || !campaign_menu::valid_percentages(&record.statistics)
        }) {
            return Err(secret_replay_error(
                "saved secret statistics have an invalid mission, duplicate or percentage",
            ));
        }

        match (
            flow.secret_entry(flow.bound_level_id()),
            self.return_frame.as_ref(),
        ) {
            (Some(_), Some(frame)) => {
                if !self
                    .discovered_secret_levels
                    .contains(&frame.secret_level_id)
                {
                    return Err(secret_replay_error(
                        "saved secret return frame is missing its discovered portal state",
                    ));
                }
                validate_return_frame(frame, flow)?;
            }
            (Some(_), None) => {
                return Err(secret_replay_error(
                    "saved secret session has no retained parent return frame",
                ));
            }
            (None, Some(_)) => {
                return Err(secret_replay_error(
                    "saved ordinary session contains an unrelated secret return frame",
                ));
            }
            (None, None) => {}
        }
        Ok(())
    }

    /// Consume the active frame after the runtime owner has resolved the
    /// parent map state. The caller stages mutations on a cloned state so
    /// failure while resolving the parent leaves the live state untouched.
    pub(crate) fn return_to_parent(
        &mut self,
        active_secret_flow: &RuntimeFlowDefinition,
        outcome: SecretMissionOutcome,
    ) -> Result<CampaignSaveState, RuntimeTransitionError> {
        self.validate_for_flow(active_secret_flow)?;
        let frame = self
            .return_frame
            .as_ref()
            .ok_or_else(|| secret_flow_error("no secret mission is active for this return"))?;
        let parent_campaign = frame.parent_campaign.clone();
        let secret_level_id = frame.secret_level_id.clone();

        if outcome == SecretMissionOutcome::Completed
            && !self.completed_secret_levels.contains(&secret_level_id)
        {
            self.completed_secret_levels.push(secret_level_id);
        }

        self.return_frame = None;
        Ok(parent_campaign)
    }
}

fn validate_parent_campaign(
    campaign: &CampaignSaveState,
    flow: &RuntimeFlowDefinition,
) -> Result<(), RuntimeTransitionError> {
    if campaign.phase != CampaignMenuPhase::MissionSelect || campaign.selected_level_id.is_none() {
        return Err(secret_flow_error(
            "secret entry requires a retained mission-selection state",
        ));
    }
    validate_earned_parent_marker(campaign, flow)?;

    let mut completed = std::collections::BTreeSet::new();
    if campaign.completed_levels.iter().any(|level| {
        !flow.entries().iter().any(|entry| entry.level_id() == level) || !completed.insert(level)
    }) {
        return Err(secret_flow_error(
            "retained parent frontier contains an unknown or repeated ordinary mission",
        ));
    }
    campaign_menu::validate_statistics(flow, &campaign.completed_levels, &campaign.level_statistics)
}

fn validate_earned_parent_marker(
    campaign: &CampaignSaveState,
    flow: &RuntimeFlowDefinition,
) -> Result<(), RuntimeTransitionError> {
    if campaign
        .completed_level_id
        .as_ref()
        .is_some_and(|completed_level| {
            !campaign.completed_levels.contains(completed_level)
                || flow.bound_level_id() != completed_level
        })
    {
        return Err(secret_flow_error(
            "retained results marker is not earned by its bound parent mission",
        ));
    }
    Ok(())
}

fn validate_return_frame(
    frame: &SecretMissionReturnFrame,
    active_secret_flow: &RuntimeFlowDefinition,
) -> Result<(), RuntimeTransitionError> {
    let parent_flow = frame
        .parent_campaign
        .definition
        .runtime_flow()
        .ok_or_else(|| secret_replay_error("saved parent return frame has no runtime flow"))?;
    let active_entry = active_secret_flow
        .secret_entry(&frame.secret_level_id)
        .ok_or_else(|| secret_replay_error("saved secret return destination is not installed"))?;
    if active_secret_flow.bound_level_id() != &frame.secret_level_id
        || active_entry.origin_level_id() != &frame.origin_level_id
        || !active_secret_flow.continues_catalog(parent_flow)
        || parent_flow.secret_entry(&frame.secret_level_id) != Some(active_entry)
        || frame.parent_campaign.phase != CampaignMenuPhase::MissionSelect
        || frame.parent_campaign.selected_level_id.as_ref() != Some(&frame.origin_level_id)
    {
        return Err(secret_replay_error(
            "saved secret return frame does not retain its declared parent mission",
        ));
    }
    validate_parent_campaign(&frame.parent_campaign, parent_flow)
        .map_err(|_| secret_replay_error("saved secret parent campaign state is invalid"))
}

fn secret_flow_error(message: impl Into<String>) -> RuntimeTransitionError {
    RuntimeTransitionError::InvalidRuntimeFlow {
        message: message.into(),
    }
}

fn secret_replay_error(message: impl Into<String>) -> RuntimeTransitionError {
    RuntimeTransitionError::InvalidReplay {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        campaign::backend::load_mc2_campaign,
        game_definition::{
            RuntimeFlowPresentationDefinition, RuntimeFlowPresentationEntry,
            SemanticPresentationCueId, TITLE_PRESENTATION_CUE_ID, default_catalog,
        },
    };
    use sim_definition::runtime_flow::RuntimeFlowSecretEntry;
    use std::path::Path;

    fn fixture() -> (
        CampaignSaveState,
        RuntimeFlowDefinition,
        RuntimeFlowDefinition,
        SemanticLevelId,
    ) {
        let package_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let package = package_root.join("mc-bhq8-level1-collision-v5-night");
        let definition = load_mc2_campaign(&package, &default_catalog().unwrap()).unwrap();
        let parent_level = definition.runtime_flow().unwrap().bound_level_id().clone();
        let secret_level = SemanticLevelId::parse("level.magic_carpet_2.secret_fixture").unwrap();
        let previous_flow = definition.runtime_flow().unwrap();
        let secret_entry = RuntimeFlowSecretEntry::new(parent_level.clone(), secret_level.clone());
        let mut presentations = previous_flow.presentation().entries().to_vec();
        let cue_stem = secret_level.as_str().strip_prefix("level.").unwrap();
        presentations.push(RuntimeFlowPresentationEntry::new(
            secret_level.clone(),
            SemanticPresentationCueId::parse(&format!("presentation.{cue_stem}.loading")).unwrap(),
            SemanticPresentationCueId::parse(&format!("presentation.{cue_stem}.entry")).unwrap(),
            SemanticPresentationCueId::parse(&format!("presentation.{cue_stem}.completion"))
                .unwrap(),
        ));
        let presentation = RuntimeFlowPresentationDefinition::new(
            SemanticPresentationCueId::parse(TITLE_PRESENTATION_CUE_ID).unwrap(),
            presentations,
        );
        let parent_flow = RuntimeFlowDefinition::new_with_secret_entries(
            previous_flow.game_id().clone(),
            parent_level.clone(),
            previous_flow.entry_level_id().clone(),
            previous_flow.entries().to_vec(),
            vec![secret_entry],
            presentation.clone(),
        )
        .unwrap();
        let secret_flow = RuntimeFlowDefinition::new_with_secret_entries(
            parent_flow.game_id().clone(),
            secret_level.clone(),
            parent_flow.entry_level_id().clone(),
            parent_flow.entries().to_vec(),
            parent_flow.secret_entries().to_vec(),
            presentation,
        )
        .unwrap();
        let definition = sim_definition::adapter::with_level_and_runtime_flow(
            definition.clone(),
            definition.level_definition().unwrap().clone(),
            parent_flow.clone(),
        );
        let parent_campaign = CampaignSaveState {
            definition,
            phase: CampaignMenuPhase::MissionSelect,
            completed_level_id: None,
            selected_level_id: Some(parent_level.clone()),
            completed_levels: Vec::new(),
            level_statistics: Vec::new(),
        };
        (parent_campaign, parent_flow, secret_flow, secret_level)
    }

    fn compatibility_catalog_bound_elsewhere(
        flow: &RuntimeFlowDefinition,
        selected_origin: &SemanticLevelId,
    ) -> RuntimeFlowDefinition {
        let other_level = flow
            .entries()
            .iter()
            .map(|entry| entry.level_id())
            .find(|level| *level != selected_origin)
            .expect("the campaign fixture contains another ordinary level")
            .clone();
        RuntimeFlowDefinition::new_with_secret_entries(
            flow.game_id().clone(),
            other_level,
            flow.entry_level_id().clone(),
            flow.entries().to_vec(),
            flow.secret_entries().to_vec(),
            flow.presentation().clone(),
        )
        .unwrap()
    }

    fn earned_marker_fixture() -> (
        CampaignSaveState,
        RuntimeFlowDefinition,
        RuntimeFlowDefinition,
        SemanticLevelId,
        SemanticLevelId,
        SemanticLevelId,
    ) {
        let (mut parent, source_flow, _, secret_level) = fixture();
        let completed_level = source_flow.bound_level_id().clone();
        let selected_origin = source_flow
            .entries()
            .get(1)
            .expect("the campaign fixture has a next mission")
            .level_id()
            .clone();
        let secret_entry =
            RuntimeFlowSecretEntry::new(selected_origin.clone(), secret_level.clone());
        let parent_flow = RuntimeFlowDefinition::new_with_secret_entries(
            source_flow.game_id().clone(),
            completed_level.clone(),
            source_flow.entry_level_id().clone(),
            source_flow.entries().to_vec(),
            vec![secret_entry],
            source_flow.presentation().clone(),
        )
        .unwrap();
        let secret_flow = RuntimeFlowDefinition::new_with_secret_entries(
            parent_flow.game_id().clone(),
            secret_level.clone(),
            parent_flow.entry_level_id().clone(),
            parent_flow.entries().to_vec(),
            parent_flow.secret_entries().to_vec(),
            parent_flow.presentation().clone(),
        )
        .unwrap();
        let level_definition = parent.definition.level_definition().unwrap().clone();
        parent.definition = sim_definition::adapter::with_level_and_runtime_flow(
            parent.definition.clone(),
            level_definition,
            parent_flow.clone(),
        );
        parent.phase = CampaignMenuPhase::MissionSelect;
        parent.completed_level_id = Some(completed_level.clone());
        parent.selected_level_id = Some(selected_origin.clone());
        parent.completed_levels = vec![completed_level.clone()];
        parent.level_statistics.clear();
        (
            parent,
            parent_flow,
            secret_flow,
            completed_level,
            selected_origin,
            secret_level,
        )
    }

    #[test]
    fn secret_return_retains_parent_frontier_and_round_trips_active_save_state() {
        let (parent, parent_flow, secret_flow, secret_level) = fixture();
        let parent_level = parent.selected_level_id.clone().unwrap();
        let mut state = SecretFlowState::default();
        state
            .enter_secret(&parent_flow, parent.clone(), secret_level.clone())
            .unwrap();
        let encoded = serde_json::to_vec(&state).unwrap();
        let mut resumed: SecretFlowState = serde_json::from_slice(&encoded).unwrap();
        resumed.validate_for_flow(&secret_flow).unwrap();
        let returned = resumed
            .return_to_parent(&secret_flow, SecretMissionOutcome::Completed)
            .unwrap();
        assert_eq!(returned, parent);
        assert_eq!(resumed.discovered_secret_levels(), &[secret_level.clone()]);
        assert!(!resumed.is_discovered_unfinished(&secret_level));
        assert_eq!(resumed.completed_secret_levels(), &[secret_level]);
        assert_eq!(returned.selected_level_id.as_ref(), Some(&parent_level));
        assert_eq!(returned.completed_levels, parent.completed_levels);
        assert_eq!(returned.level_statistics, parent.level_statistics);
        assert!(resumed.return_frame().is_none());
        resumed.validate_for_flow(&parent_flow).unwrap();
    }

    #[test]
    fn unknown_secret_result_leaves_parent_and_progress_untouched() {
        let (parent, parent_flow, _, _) = fixture();
        let mut state = SecretFlowState::default();
        let before = state.clone();
        let unknown = SemanticLevelId::parse("level.magic_carpet_2.not_a_secret").unwrap();
        assert!(state.enter_secret(&parent_flow, parent, unknown).is_err());
        assert_eq!(state, before);
    }

    #[test]
    fn abandoning_a_secret_discovers_it_without_granting_completion() {
        let (parent, parent_flow, secret_flow, secret_level) = fixture();
        let mut state = SecretFlowState::default();
        assert!(!state.is_discovered_unfinished(&secret_level));
        state
            .enter_secret(&parent_flow, parent, secret_level.clone())
            .unwrap();
        assert!(!state.is_discovered_unfinished(&secret_level));
        let _parent = state
            .return_to_parent(&secret_flow, SecretMissionOutcome::Abandoned)
            .unwrap();
        assert!(state.is_discovered_unfinished(&secret_level));
        assert_eq!(state.discovered_secret_levels(), &[secret_level]);
        assert!(state.completed_secret_levels().is_empty());
    }

    #[test]
    fn undiscovered_secret_cannot_be_reentered() {
        let (parent, parent_flow, _, secret_level) = fixture();
        let selected_origin = parent.selected_level_id.clone().unwrap();
        let compatibility_catalog =
            compatibility_catalog_bound_elsewhere(&parent_flow, &selected_origin);
        let mut state = SecretFlowState::default();
        let before = state.clone();

        assert!(
            state
                .enter_discovered_secret(&compatibility_catalog, parent, secret_level)
                .is_err()
        );
        assert_eq!(state, before);
    }

    #[test]
    fn completed_secret_statistics_survive_save_abandon_and_replace_on_replay() {
        let (parent, parent_flow, secret_flow, secret_level) = fixture();
        let stats = super::super::GameCompletionStatistics {
            spells_found_percent: 11,
            hit_accuracy_percent: 22,
            creatures_killed_percent: 33,
            mana_collected_percent: 44,
            time_taken_seconds: 55,
        };
        let mut state = SecretFlowState::default();
        state
            .enter_secret(&parent_flow, parent.clone(), secret_level.clone())
            .unwrap();
        assert!(
            state
                .record_completed_statistics(&secret_level, stats.clone())
                .is_err(),
            "an active but unfinished secret cannot acquire a result row"
        );
        state
            .return_to_parent(&secret_flow, SecretMissionOutcome::Completed)
            .unwrap();
        state
            .record_completed_statistics(&secret_level, stats.clone())
            .unwrap();
        state.validate_for_flow(&parent_flow).unwrap();
        let bytes = serde_json::to_vec(&state).unwrap();
        let mut restored: SecretFlowState = serde_json::from_slice(&bytes).unwrap();
        restored.validate_for_flow(&parent_flow).unwrap();
        assert_eq!(restored.completed_secret_statistics()[0].statistics, stats);

        restored
            .enter_discovered_secret(&parent_flow, parent.clone(), secret_level.clone())
            .unwrap();
        restored
            .return_to_parent(&secret_flow, SecretMissionOutcome::Abandoned)
            .unwrap();
        assert_eq!(
            restored.completed_secret_statistics()[0].statistics,
            stats,
            "abandonment cannot erase or replace an earlier source row"
        );

        let mut replay_stats = stats.clone();
        replay_stats.time_taken_seconds += 1;
        restored
            .enter_discovered_secret(&parent_flow, parent, secret_level.clone())
            .unwrap();
        restored
            .return_to_parent(&secret_flow, SecretMissionOutcome::Completed)
            .unwrap();
        restored
            .record_completed_statistics(&secret_level, replay_stats.clone())
            .unwrap();
        assert_eq!(restored.completed_secret_statistics().len(), 1);
        assert_eq!(
            restored.completed_secret_statistics()[0].statistics,
            replay_stats
        );
        restored.validate_for_flow(&parent_flow).unwrap();

        let mut invalid = restored.clone();
        invalid
            .completed_secret_statistics
            .push(invalid.completed_secret_statistics[0].clone());
        assert!(
            invalid.validate_for_flow(&parent_flow).is_err(),
            "duplicate rows are rejected"
        );
        let mut invalid = restored.clone();
        invalid.completed_secret_statistics[0].level_id = parent_flow.bound_level_id().clone();
        assert!(
            invalid.validate_for_flow(&parent_flow).is_err(),
            "ordinary and secret rows cannot be spliced"
        );
        let mut invalid = restored.clone();
        invalid.completed_secret_levels.clear();
        assert!(
            invalid.validate_for_flow(&parent_flow).is_err(),
            "unfinished secrets cannot own a row"
        );
        let mut invalid = restored;
        invalid.completed_secret_statistics[0]
            .statistics
            .hit_accuracy_percent = 101;
        assert!(
            invalid.validate_for_flow(&parent_flow).is_err(),
            "invalid source percentages are rejected"
        );
    }

    #[test]
    fn discovered_unfinished_and_completed_secrets_reenter_without_advancing_ordinary_frontier() {
        for completed in [false, true] {
            let (parent, parent_flow, _, secret_level) = fixture();
            let selected_origin = parent.selected_level_id.clone().unwrap();
            let compatibility_catalog =
                compatibility_catalog_bound_elsewhere(&parent_flow, &selected_origin);
            assert_ne!(compatibility_catalog.bound_level_id(), &selected_origin);
            let ordinary_frontier = parent.completed_levels.clone();
            let mut state = SecretFlowState {
                discovered_secret_levels: vec![secret_level.clone()],
                completed_secret_levels: completed
                    .then(|| secret_level.clone())
                    .into_iter()
                    .collect(),
                completed_secret_statistics: Vec::new(),
                return_frame: None,
            };

            state
                .enter_discovered_secret(
                    &compatibility_catalog,
                    parent.clone(),
                    secret_level.clone(),
                )
                .unwrap();

            let frame = state.return_frame().unwrap();
            assert_eq!(frame.origin_level_id(), &selected_origin);
            assert_eq!(frame.secret_level_id(), &secret_level);
            assert_eq!(frame.parent_campaign().completed_levels, ordinary_frontier);
            assert_eq!(
                state.completed_secret_levels().contains(&secret_level),
                completed
            );
        }
    }

    #[test]
    fn discovered_secret_cannot_be_reentered_from_the_wrong_origin() {
        let (mut parent, parent_flow, _, secret_level) = fixture();
        let selected_origin = parent.selected_level_id.clone().unwrap();
        let wrong_origin = parent_flow
            .entries()
            .iter()
            .map(|entry| entry.level_id())
            .find(|level| *level != &selected_origin)
            .expect("the campaign fixture contains another ordinary level")
            .clone();
        let compatibility_catalog =
            compatibility_catalog_bound_elsewhere(&parent_flow, &selected_origin);
        parent.selected_level_id = Some(wrong_origin);
        let mut state = SecretFlowState {
            discovered_secret_levels: vec![secret_level.clone()],
            completed_secret_levels: Vec::new(),
            completed_secret_statistics: Vec::new(),
            return_frame: None,
        };
        let before = state.clone();

        assert!(
            state
                .enter_discovered_secret(&compatibility_catalog, parent, secret_level)
                .is_err()
        );
        assert_eq!(state, before);
        assert!(state.return_frame().is_none());
    }

    #[test]
    fn earned_dismissed_results_marker_round_trips_through_secret_parent_frame() {
        let (parent, parent_flow, secret_flow, completed_level, selected_origin, secret_level) =
            earned_marker_fixture();
        assert_eq!(parent.completed_level_id.as_ref(), Some(&completed_level));
        assert_eq!(parent.completed_levels, vec![completed_level.clone()]);
        assert_eq!(parent.selected_level_id.as_ref(), Some(&selected_origin));

        let mut state = SecretFlowState {
            discovered_secret_levels: vec![secret_level.clone()],
            completed_secret_levels: Vec::new(),
            completed_secret_statistics: Vec::new(),
            return_frame: None,
        };
        state
            .enter_discovered_secret(&parent_flow, parent.clone(), secret_level.clone())
            .unwrap();
        let encoded = serde_json::to_vec(&state).unwrap();
        let mut resumed: SecretFlowState = serde_json::from_slice(&encoded).unwrap();
        resumed.validate_for_flow(&secret_flow).unwrap();

        let returned = resumed
            .return_to_parent(&secret_flow, SecretMissionOutcome::Abandoned)
            .unwrap();
        assert_eq!(returned, parent);
        assert_eq!(returned.completed_level_id.as_ref(), Some(&completed_level));
        assert_eq!(returned.completed_levels, vec![completed_level]);
        assert_eq!(returned.selected_level_id.as_ref(), Some(&selected_origin));
        assert!(resumed.completed_secret_levels().is_empty());
        resumed.validate_for_flow(&parent_flow).unwrap();
    }

    #[test]
    fn rejects_unearned_and_wrong_owner_parent_results_markers() {
        let (parent, parent_flow, secret_flow, completed_level, selected_origin, secret_level) =
            earned_marker_fixture();
        let mut state = SecretFlowState {
            discovered_secret_levels: vec![secret_level.clone()],
            completed_secret_levels: Vec::new(),
            completed_secret_statistics: Vec::new(),
            return_frame: None,
        };

        let mut unearned_marker = parent.clone();
        unearned_marker.completed_levels.clear();
        let before = state.clone();
        assert!(
            state
                .enter_discovered_secret(&parent_flow, unearned_marker, secret_level.clone(),)
                .is_err()
        );
        assert_eq!(state, before);
        assert!(state.return_frame().is_none());

        let mut wrong_owner_marker = parent;
        wrong_owner_marker.completed_level_id = Some(selected_origin.clone());
        wrong_owner_marker
            .completed_levels
            .push(selected_origin.clone());
        assert!(
            wrong_owner_marker
                .completed_levels
                .contains(&wrong_owner_marker.completed_level_id.clone().unwrap())
        );
        assert_ne!(
            parent_flow.bound_level_id(),
            wrong_owner_marker.completed_level_id.as_ref().unwrap()
        );
        assert!(
            state
                .enter_discovered_secret(
                    &parent_flow,
                    wrong_owner_marker.clone(),
                    secret_level.clone(),
                )
                .is_err()
        );
        assert_eq!(state, before);
        assert_eq!(parent_flow.bound_level_id(), &completed_level);

        let wrong_owner_frame = SecretFlowState {
            discovered_secret_levels: vec![secret_level.clone()],
            completed_secret_levels: Vec::new(),
            completed_secret_statistics: Vec::new(),
            return_frame: Some(SecretMissionReturnFrame {
                parent_campaign: wrong_owner_marker,
                origin_level_id: selected_origin,
                secret_level_id: secret_level,
            }),
        };
        assert!(wrong_owner_frame.validate_for_flow(&secret_flow).is_err());
    }
}
