//! Receipts for guarded entry into and return from a secret mission.
//!
//! This contract deliberately has no ordinary completion payload. The runtime
//! producer creates return receipts only after its guarded secret-return path
//! has selected the retained campaign origin.

use super::super::{CampaignMenuPhase, GAME_RUNTIME_CONTRACT, RuntimeIdentity, RuntimeMode};
use crate::{game_definition::SemanticLevelId, game_session::SessionIdentity};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{error::Error, fmt};

pub const SECRET_TRANSITION_RECEIPT_CONTRACT: &str = "carpet-flight-secret-transition-receipt/v1";

/// Direction of one accepted secret-mission boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretTransitionKind {
    Entry,
    CompletedReturn,
    AbandonedReturn,
}

/// Immutable, digest-bound identity change for secret entry or return.
///
/// `receipt_sha256` hashes the serialized receipt payload in field order,
/// excluding only the digest field itself. No ordinary completion data is
/// carried by this contract.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SecretTransitionReceipt {
    pub receipt_contract: &'static str,
    pub kind: SecretTransitionKind,
    pub before: RuntimeIdentity,
    pub after: RuntimeIdentity,
    pub source_session: SessionIdentity,
    pub origin_level_id: SemanticLevelId,
    pub secret_level_id: SemanticLevelId,
    pub receipt_sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretTransitionReceiptError {
    InvalidRuntimeContract,
    InvalidRuntimeId,
    RuntimeIdentityMismatch,
    RuntimeGenerationDidNotAdvance,
    SourceSessionMismatch,
    InvalidEntryTransition,
    InvalidReturnTransition,
}

impl fmt::Display for SecretTransitionReceiptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidRuntimeContract => "runtime contract does not match this engine",
            Self::InvalidRuntimeId => "runtime ID is zero",
            Self::RuntimeIdentityMismatch => "runtime ID changed across the transition",
            Self::RuntimeGenerationDidNotAdvance => {
                "runtime generation did not advance by exactly one"
            }
            Self::SourceSessionMismatch => {
                "source session does not match the guarded before identity"
            }
            Self::InvalidEntryTransition => "identity pair is not a valid secret entry",
            Self::InvalidReturnTransition => "identity pair is not a valid secret return",
        };
        formatter.write_str(message)
    }
}

impl Error for SecretTransitionReceiptError {}

impl SecretTransitionReceipt {
    /// Construct a receipt after the owning runtime has committed a secret
    /// boundary. Runtime-owned construction keeps clients from supplying a
    /// completion result as transition authority.
    pub(crate) fn new(
        kind: SecretTransitionKind,
        before: RuntimeIdentity,
        after: RuntimeIdentity,
        source_session: SessionIdentity,
        origin_level_id: SemanticLevelId,
        secret_level_id: SemanticLevelId,
    ) -> Result<Self, SecretTransitionReceiptError> {
        let mut receipt = Self {
            receipt_contract: SECRET_TRANSITION_RECEIPT_CONTRACT,
            kind,
            before,
            after,
            source_session,
            origin_level_id,
            secret_level_id,
            receipt_sha256: String::new(),
        };
        receipt.validate_transition()?;
        receipt.receipt_sha256 = receipt.expected_sha256();
        Ok(receipt)
    }

    /// Verify both the transition identities and the exact serialized payload
    /// digest.
    #[must_use]
    pub fn verify(&self) -> bool {
        self.receipt_contract == SECRET_TRANSITION_RECEIPT_CONTRACT
            && self.validate_transition().is_ok()
            && self.receipt_sha256 == self.expected_sha256()
    }

    fn validate_transition(&self) -> Result<(), SecretTransitionReceiptError> {
        if self.before.runtime_contract != GAME_RUNTIME_CONTRACT
            || self.after.runtime_contract != GAME_RUNTIME_CONTRACT
        {
            return Err(SecretTransitionReceiptError::InvalidRuntimeContract);
        }
        if self.before.runtime_id == 0 || self.after.runtime_id == 0 {
            return Err(SecretTransitionReceiptError::InvalidRuntimeId);
        }
        if self.before.runtime_id != self.after.runtime_id {
            return Err(SecretTransitionReceiptError::RuntimeIdentityMismatch);
        }
        if self.before.generation.checked_add(1) != Some(self.after.generation) {
            return Err(SecretTransitionReceiptError::RuntimeGenerationDidNotAdvance);
        }
        if self.source_session.session_id == 0
            || self.before.session.as_ref() != Some(&self.source_session)
        {
            return Err(SecretTransitionReceiptError::SourceSessionMismatch);
        }

        match self.kind {
            SecretTransitionKind::Entry => self.validate_entry(),
            SecretTransitionKind::CompletedReturn | SecretTransitionKind::AbandonedReturn => {
                self.validate_return()
            }
        }
    }

    fn validate_entry(&self) -> Result<(), SecretTransitionReceiptError> {
        let before_flow_matches = self
            .before
            .active_runtime_flow
            .as_ref()
            .is_some_and(|flow| {
                flow.bound_level_id() == &self.origin_level_id
                    && has_secret_origin(flow, &self.origin_level_id, &self.secret_level_id)
            });
        let after_flow_matches = self.after.active_runtime_flow.as_ref().is_some_and(|flow| {
            flow.bound_level_id() == &self.secret_level_id
                && has_secret_origin(flow, &self.origin_level_id, &self.secret_level_id)
        });
        let catalog_lineage_matches = self
            .after
            .active_runtime_flow
            .as_ref()
            .zip(self.before.active_runtime_flow.as_ref())
            .is_some_and(|(after_flow, before_flow)| after_flow.continues_catalog(before_flow));
        let after_session_advances = self.after.session.as_ref().is_some_and(|session| {
            session.session_id == self.source_session.session_id
                && self.source_session.generation.checked_add(1) == Some(session.generation)
        });

        if self.before.mode == RuntimeMode::Playing
            && self.after.mode == RuntimeMode::Playing
            && before_flow_matches
            && after_flow_matches
            && catalog_lineage_matches
            && after_session_advances
        {
            Ok(())
        } else {
            Err(SecretTransitionReceiptError::InvalidEntryTransition)
        }
    }

    fn validate_return(&self) -> Result<(), SecretTransitionReceiptError> {
        let before_flow_matches = self
            .before
            .active_runtime_flow
            .as_ref()
            .is_some_and(|flow| {
                flow.bound_level_id() == &self.secret_level_id
                    && has_secret_origin(flow, &self.origin_level_id, &self.secret_level_id)
            });
        let after_flow_matches = self.after.active_runtime_flow.as_ref().is_some_and(|flow| {
            has_secret_origin(flow, &self.origin_level_id, &self.secret_level_id)
        });
        let catalog_lineage_matches = self
            .before
            .active_runtime_flow
            .as_ref()
            .zip(self.after.active_runtime_flow.as_ref())
            .is_some_and(|(before_flow, after_flow)| before_flow.continues_catalog(after_flow));
        let after_campaign_matches = self.after.campaign_menu.as_ref().is_some_and(|menu| {
            menu.phase == CampaignMenuPhase::MissionSelect
                && menu.selected_level_id.as_ref() == Some(&self.origin_level_id)
        });

        let source_mode_matches = self.before.mode == RuntimeMode::Playing
            || (self.kind == SecretTransitionKind::AbandonedReturn
                && self.before.mode == RuntimeMode::Paused);
        if source_mode_matches
            && self.after.mode == RuntimeMode::Campaign
            && self.after.session.is_none()
            && before_flow_matches
            && after_flow_matches
            && catalog_lineage_matches
            && after_campaign_matches
        {
            Ok(())
        } else {
            Err(SecretTransitionReceiptError::InvalidReturnTransition)
        }
    }

    fn expected_sha256(&self) -> String {
        let payload = SecretTransitionReceiptPayload {
            receipt_contract: self.receipt_contract,
            kind: self.kind,
            before: &self.before,
            after: &self.after,
            source_session: &self.source_session,
            origin_level_id: &self.origin_level_id,
            secret_level_id: &self.secret_level_id,
        };
        let bytes = serde_json::to_vec(&payload)
            .expect("secret transition receipt payload is serializable");
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

fn has_secret_origin(
    flow: &crate::game_definition::RuntimeFlowDefinition,
    origin_level_id: &SemanticLevelId,
    secret_level_id: &SemanticLevelId,
) -> bool {
    flow.secret_entry(secret_level_id)
        .is_some_and(|entry| entry.origin_level_id() == origin_level_id)
}

#[derive(Serialize)]
struct SecretTransitionReceiptPayload<'a> {
    receipt_contract: &'static str,
    kind: SecretTransitionKind,
    before: &'a RuntimeIdentity,
    after: &'a RuntimeIdentity,
    source_session: &'a SessionIdentity,
    origin_level_id: &'a SemanticLevelId,
    secret_level_id: &'a SemanticLevelId,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_definition::{
        RuntimeFlowDefinition, RuntimeFlowDestination, RuntimeFlowEntry, RuntimeFlowNodeId,
        RuntimeFlowPresentationDefinition, RuntimeFlowPresentationEntry, SemanticGameId,
        SemanticPresentationCueId, TITLE_PRESENTATION_CUE_ID,
    };
    use crate::game_runtime::preferences::{PlayerPreferences, PreferencesSnapshot};
    use crate::game_runtime::{
        CampaignMenuSnapshot, RuntimePresentationCue, RuntimePresentationPhase,
    };
    use sim_definition::runtime_flow::RuntimeFlowSecretEntry;

    fn flow_fixture() -> (
        SemanticLevelId,
        SemanticLevelId,
        RuntimeFlowDefinition,
        RuntimeFlowDefinition,
        RuntimeFlowDefinition,
        RuntimeFlowDefinition,
    ) {
        let previous = SemanticLevelId::parse("level.magic_carpet_2.receipt_previous").unwrap();
        let origin = SemanticLevelId::parse("level.magic_carpet_2.receipt_origin").unwrap();
        let secret = SemanticLevelId::parse("level.magic_carpet_2.receipt_secret").unwrap();
        let presentations = [&previous, &origin, &secret]
            .into_iter()
            .map(|level| {
                let stem = level.as_str().strip_prefix("level.").unwrap();
                RuntimeFlowPresentationEntry::new(
                    level.clone(),
                    SemanticPresentationCueId::parse(&format!("presentation.{stem}.loading"))
                        .unwrap(),
                    SemanticPresentationCueId::parse(&format!("presentation.{stem}.entry"))
                        .unwrap(),
                    SemanticPresentationCueId::parse(&format!("presentation.{stem}.completion"))
                        .unwrap(),
                )
            })
            .collect();
        let presentation = RuntimeFlowPresentationDefinition::new(
            SemanticPresentationCueId::parse(TITLE_PRESENTATION_CUE_ID).unwrap(),
            presentations,
        );
        let entries = vec![
            RuntimeFlowEntry::new(
                previous.clone(),
                RuntimeFlowDestination::Level {
                    level_id: origin.clone(),
                },
            ),
            RuntimeFlowEntry::new(
                origin.clone(),
                RuntimeFlowDestination::Title {
                    node_id: RuntimeFlowNodeId::parse("flow.magic_carpet_2.receipt_return")
                        .unwrap(),
                },
            ),
        ];
        let secrets = vec![RuntimeFlowSecretEntry::new(origin.clone(), secret.clone())];
        let game_id = SemanticGameId::parse("game.magic_carpet_2").unwrap();
        let retained_parent_flow = RuntimeFlowDefinition::new_with_secret_entries(
            game_id.clone(),
            previous.clone(),
            previous.clone(),
            entries.clone(),
            secrets.clone(),
            presentation.clone(),
        )
        .unwrap();
        let origin_flow = RuntimeFlowDefinition::new_with_secret_entries(
            game_id.clone(),
            origin.clone(),
            previous.clone(),
            entries.clone(),
            secrets.clone(),
            presentation.clone(),
        )
        .unwrap();
        let secret_flow = RuntimeFlowDefinition::new_with_secret_entries(
            game_id,
            secret.clone(),
            previous.clone(),
            entries.clone(),
            secrets.clone(),
            presentation.clone(),
        )
        .unwrap();
        let mismatched_parent_flow = RuntimeFlowDefinition::new_with_secret_entries(
            SemanticGameId::parse("game.magic_carpet_2.other_catalog").unwrap(),
            previous.clone(),
            previous,
            entries,
            secrets,
            presentation,
        )
        .unwrap();
        (
            origin,
            secret,
            origin_flow,
            secret_flow,
            retained_parent_flow,
            mismatched_parent_flow,
        )
    }

    fn session(session_id: u64, generation: u64, definition_hash: &str) -> SessionIdentity {
        SessionIdentity {
            engine_contract: "carpet-flight-game-session/v1",
            package_graph_hash: "package-graph".to_owned(),
            definition_hash: definition_hash.to_owned(),
            definition_revision: 1,
            session_id,
            generation,
            tick: 0,
        }
    }

    fn identity(
        runtime_generation: u64,
        mode: RuntimeMode,
        flow: RuntimeFlowDefinition,
        session: Option<SessionIdentity>,
        selected_level: Option<SemanticLevelId>,
    ) -> RuntimeIdentity {
        let level_id = flow.bound_level_id().clone();
        let campaign_menu = selected_level.map(|selected| CampaignMenuSnapshot {
            phase: CampaignMenuPhase::MissionSelect,
            completed_level_id: None,
            completed_levels: Vec::new(),
            available_levels: vec![selected.clone()],
            selected_level_id: Some(selected.clone()),
            briefing_level_id: Some(selected),
            completion_statistics: None,
            level_statistics: Vec::new(),
        });
        RuntimeIdentity {
            source_speed_index: 0,
            source_player_names_visible: true,
            runtime_contract: GAME_RUNTIME_CONTRACT,
            runtime_id: 42,
            generation: runtime_generation,
            mode,
            display_settings: crate::game_runtime::DisplaySettings::default(),
            frontend_media: None,
            player_preferences: PreferencesSnapshot {
                preferences: PlayerPreferences::default(),
                installed: false,
                load_status: "not_installed".to_owned(),
                assistance_sequence: 0,
                installed_locales: Vec::new(),
                locale_binding: None,
                language_status: "not_installed".to_owned(),
                source_brightness_admitted: None,
                source_audio_admitted: None,
                source_flight_assistance_admitted: None,
                source_help_mode_admitted: None,
            },
            presentation: RuntimePresentationCue {
                cue_id: SemanticPresentationCueId::parse("presentation.receipt.current").unwrap(),
                phase: if mode == RuntimeMode::Campaign {
                    RuntimePresentationPhase::CampaignMap
                } else {
                    RuntimePresentationPhase::LevelEntry
                },
                level_id: Some(level_id),
                completion_destination: None,
            },
            campaign_menu,
            pending_definition: None,
            active_runtime_flow: Some(flow),
            session,
            secret_campaign: None,
        }
    }

    fn entry_receipt() -> SecretTransitionReceipt {
        let (origin, secret, origin_flow, secret_flow, _, _) = flow_fixture();
        let source = session(73, 8, "origin-definition");
        SecretTransitionReceipt::new(
            SecretTransitionKind::Entry,
            identity(
                5,
                RuntimeMode::Playing,
                origin_flow,
                Some(source.clone()),
                None,
            ),
            identity(
                6,
                RuntimeMode::Playing,
                secret_flow,
                Some(session(
                    source.session_id,
                    source.generation + 1,
                    "secret-definition",
                )),
                None,
            ),
            source,
            origin,
            secret,
        )
        .unwrap()
    }

    fn return_receipt(kind: SecretTransitionKind) -> SecretTransitionReceipt {
        let (origin, secret, _, secret_flow, retained_parent_flow, _) = flow_fixture();
        assert_ne!(retained_parent_flow.bound_level_id(), &origin);
        let source = session(73, 9, "secret-definition");
        SecretTransitionReceipt::new(
            kind,
            identity(
                6,
                RuntimeMode::Playing,
                secret_flow,
                Some(source.clone()),
                None,
            ),
            identity(
                7,
                RuntimeMode::Campaign,
                retained_parent_flow,
                None,
                Some(origin.clone()),
            ),
            source,
            origin,
            secret,
        )
        .unwrap()
    }

    #[test]
    fn verifies_entry_and_both_secret_return_kinds() {
        let entry = entry_receipt();
        assert!(entry.verify());
        let serialized = serde_json::to_value(&entry).unwrap();
        assert_eq!(
            serialized["receipt_contract"],
            SECRET_TRANSITION_RECEIPT_CONTRACT
        );
        assert_eq!(serialized["kind"], "entry");
        assert!(serialized.get("presentation_sequence").is_none());

        let completed = return_receipt(SecretTransitionKind::CompletedReturn);
        assert!(completed.verify());
        assert_ne!(
            completed
                .after
                .active_runtime_flow
                .as_ref()
                .unwrap()
                .bound_level_id(),
            &completed.origin_level_id
        );
        assert_eq!(
            serde_json::to_value(&completed).unwrap()["kind"],
            "completed_return"
        );
        let abandoned = return_receipt(SecretTransitionKind::AbandonedReturn);
        assert!(abandoned.verify());
        assert_eq!(
            serde_json::to_value(&abandoned).unwrap()["kind"],
            "abandoned_return"
        );
    }

    #[test]
    fn paused_secret_can_only_produce_an_abandoned_return() {
        let mut abandoned = return_receipt(SecretTransitionKind::AbandonedReturn);
        abandoned.before.mode = RuntimeMode::Paused;
        let accepted = SecretTransitionReceipt::new(
            abandoned.kind,
            abandoned.before.clone(),
            abandoned.after.clone(),
            abandoned.source_session.clone(),
            abandoned.origin_level_id.clone(),
            abandoned.secret_level_id.clone(),
        )
        .unwrap();
        assert!(accepted.verify());
        assert_eq!(accepted.before.mode, RuntimeMode::Paused);
        assert!(
            SecretTransitionReceipt::new(
                SecretTransitionKind::CompletedReturn,
                abandoned.before,
                abandoned.after,
                abandoned.source_session,
                abandoned.origin_level_id,
                abandoned.secret_level_id,
            )
            .is_err()
        );
        let mut entry = entry_receipt();
        entry.before.mode = RuntimeMode::Paused;
        assert!(
            SecretTransitionReceipt::new(
                entry.kind,
                entry.before,
                entry.after,
                entry.source_session,
                entry.origin_level_id,
                entry.secret_level_id,
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_payload_tampering() {
        let mut receipt = entry_receipt();
        receipt.receipt_sha256.replace_range(
            0..1,
            if receipt.receipt_sha256.starts_with('0') {
                "1"
            } else {
                "0"
            },
        );
        assert!(!receipt.verify());

        let mut payload_tamper = entry_receipt();
        payload_tamper.before.presentation.cue_id =
            SemanticPresentationCueId::parse("presentation.receipt.tampered").unwrap();
        assert!(!payload_tamper.verify());
    }

    #[test]
    fn rejects_wrong_guard_runtime_identity_and_ordinary_completion_route() {
        let (origin, secret, origin_flow, secret_flow, retained_parent_flow, _) = flow_fixture();
        let source = session(73, 8, "origin-definition");
        let before = identity(
            5,
            RuntimeMode::Playing,
            origin_flow,
            Some(source.clone()),
            None,
        );
        let after = identity(
            6,
            RuntimeMode::Playing,
            secret_flow,
            Some(session(
                source.session_id,
                source.generation + 1,
                "secret-definition",
            )),
            None,
        );
        let wrong_source = session(
            source.session_id,
            source.generation + 1,
            "origin-definition",
        );
        assert_eq!(
            SecretTransitionReceipt::new(
                SecretTransitionKind::Entry,
                before.clone(),
                after.clone(),
                wrong_source,
                origin.clone(),
                secret.clone(),
            ),
            Err(SecretTransitionReceiptError::SourceSessionMismatch)
        );

        let mut wrong_runtime = after.clone();
        wrong_runtime.runtime_id += 1;
        assert_eq!(
            SecretTransitionReceipt::new(
                SecretTransitionKind::Entry,
                before.clone(),
                wrong_runtime,
                source.clone(),
                origin.clone(),
                secret.clone(),
            ),
            Err(SecretTransitionReceiptError::RuntimeIdentityMismatch)
        );

        let mut skipped_generation = after;
        skipped_generation.generation += 1;
        assert_eq!(
            SecretTransitionReceipt::new(
                SecretTransitionKind::Entry,
                before.clone(),
                skipped_generation,
                source.clone(),
                origin.clone(),
                secret.clone(),
            ),
            Err(SecretTransitionReceiptError::RuntimeGenerationDidNotAdvance)
        );

        let ordinary_source = session(73, 8, "origin-definition");
        let ordinary_before = identity(
            8,
            RuntimeMode::Playing,
            before.active_runtime_flow.clone().unwrap(),
            Some(ordinary_source.clone()),
            None,
        );
        let ordinary_after = identity(
            9,
            RuntimeMode::Campaign,
            retained_parent_flow,
            None,
            Some(origin.clone()),
        );
        assert_eq!(
            SecretTransitionReceipt::new(
                SecretTransitionKind::CompletedReturn,
                ordinary_before,
                ordinary_after,
                ordinary_source,
                origin,
                secret,
            ),
            Err(SecretTransitionReceiptError::InvalidReturnTransition)
        );
    }

    #[test]
    fn rejects_return_with_mismatched_catalog_lineage() {
        let (origin, secret, _, secret_flow, _, mismatched_parent_flow) = flow_fixture();
        assert!(has_secret_origin(&mismatched_parent_flow, &origin, &secret));
        assert!(!secret_flow.continues_catalog(&mismatched_parent_flow));
        let source = session(73, 9, "secret-definition");
        assert_eq!(
            SecretTransitionReceipt::new(
                SecretTransitionKind::AbandonedReturn,
                identity(
                    6,
                    RuntimeMode::Playing,
                    secret_flow,
                    Some(source.clone()),
                    None,
                ),
                identity(
                    7,
                    RuntimeMode::Campaign,
                    mismatched_parent_flow,
                    None,
                    Some(origin.clone()),
                ),
                source,
                origin,
                secret,
            ),
            Err(SecretTransitionReceiptError::InvalidReturnTransition)
        );
    }
}
