//! Guarded outer runtime ownership above one definition-bound gameplay session.
//!
//! The runtime owns guarded transitions into gameplay and between completed
//! semantic levels. [`GameSession`] remains the sole gameplay authority while
//! a level is playing.

use crate::game_definition::GameDefinition;
use crate::game_definition::{RuntimeFlowDefinition, RuntimeFlowDestination, SemanticLevelId};
use crate::game_session::{
    GameAdapter, GameCompletion, GameCompletionStatus, GameSession, GameSessionBackend,
    OpenSessionError, SessionGuard, SessionGuardError, SessionIdentity,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{error::Error, fmt, num::NonZeroU64, sync::Arc};

pub const GAME_RUNTIME_CONTRACT: &str = "carpet-flight-game-runtime/v4";
pub const COMPLETION_TRANSITION_RECEIPT: &str = "carpet-flight-completion-transition-receipt/v1";

fn serialize_u64_decimal<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&value.to_string())
}

/// Runtime modes established by the guarded outer owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMode {
    Title,
    Loading,
    Playing,
    Paused,
}

impl RuntimeMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Loading => "loading",
            Self::Playing => "playing",
            Self::Paused => "paused",
        }
    }
}

/// Identity of the immutable definition waiting to be opened.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimeDefinitionIdentity {
    pub package_graph_hash: String,
    pub definition_hash: String,
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub definition_revision: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_flow: Option<RuntimeFlowDefinition>,
}

impl RuntimeDefinitionIdentity {
    fn from_definition(definition: &GameDefinition) -> Self {
        Self {
            package_graph_hash: definition.package_graph_hash().to_owned(),
            definition_hash: definition.definition_hash().to_owned(),
            definition_revision: definition.revision(),
            runtime_flow: definition.runtime_flow().cloned(),
        }
    }
}

/// Guard supplied with every state-changing runtime transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimeGuard {
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub runtime_id: u64,
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub generation: u64,
}

/// Canonical observation of the outer runtime and its current binding.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimeIdentity {
    pub runtime_contract: &'static str,
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub runtime_id: u64,
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub generation: u64,
    pub mode: RuntimeMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_definition: Option<RuntimeDefinitionIdentity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_runtime_flow: Option<RuntimeFlowDefinition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionIdentity>,
}

/// Immutable receipt for one accepted semantic level-completion boundary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompletionTransitionReceipt {
    pub receipt_contract: &'static str,
    pub before: RuntimeIdentity,
    pub after: RuntimeIdentity,
    pub source_session: SessionIdentity,
    pub completed_level_id: SemanticLevelId,
    pub completion_destination: RuntimeFlowDestination,
    pub completion: GameCompletion,
    pub receipt_sha256: String,
}

impl CompletionTransitionReceipt {
    /// Verify that the receipt digest still matches every bound transition
    /// field.
    #[must_use]
    pub fn verify(&self) -> bool {
        self.receipt_contract == COMPLETION_TRANSITION_RECEIPT
            && self.receipt_sha256
                == completion_receipt_sha256(
                    &self.before,
                    &self.after,
                    &self.source_session,
                    &self.completed_level_id,
                    &self.completion_destination,
                    &self.completion,
                )
    }
}

#[derive(Serialize)]
struct CompletionTransitionReceiptPayload<'a> {
    receipt_contract: &'static str,
    before: &'a RuntimeIdentity,
    after: &'a RuntimeIdentity,
    source_session: &'a SessionIdentity,
    completed_level_id: &'a SemanticLevelId,
    completion_destination: &'a RuntimeFlowDestination,
    completion: &'a GameCompletion,
}

impl RuntimeIdentity {
    #[must_use]
    pub const fn guard(&self) -> RuntimeGuard {
        RuntimeGuard {
            runtime_id: self.runtime_id,
            generation: self.generation,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeGuardErrorCode {
    InvalidRuntimeId,
    RuntimeMismatch,
    GenerationMismatch,
}

impl RuntimeGuardErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRuntimeId => "INVALID_RUNTIME_ID",
            Self::RuntimeMismatch => "RUNTIME_MISMATCH",
            Self::GenerationMismatch => "GENERATION_MISMATCH",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeGuardError {
    pub code: RuntimeGuardErrorCode,
    pub expected: RuntimeIdentity,
    pub actual: RuntimeGuard,
}

impl fmt::Display for RuntimeGuardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: expected runtime/generation {}/{}, received {}/{}",
            self.code.as_str(),
            self.expected.runtime_id,
            self.expected.generation,
            self.actual.runtime_id,
            self.actual.generation
        )
    }
}

impl Error for RuntimeGuardError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeTransitionError {
    InvalidRuntimeId,
    Guard(Box<RuntimeGuardError>),
    SessionGuard(Box<SessionGuardError>),
    InvalidMode {
        expected: RuntimeMode,
        actual: RuntimeMode,
    },
    InvalidRuntimeFlow {
        message: String,
    },
    InvalidNextDefinition {
        message: String,
    },
    CompletionNotReady,
    InvalidCompletion {
        message: String,
    },
    GenerationExhausted,
}

impl fmt::Display for RuntimeTransitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRuntimeId => formatter.write_str("INVALID_RUNTIME_ID: runtime ID is zero"),
            Self::Guard(error) => error.fmt(formatter),
            Self::SessionGuard(error) => error.fmt(formatter),
            Self::InvalidMode { expected, actual } => write!(
                formatter,
                "INVALID_RUNTIME_MODE: expected {}, found {}",
                expected.as_str(),
                actual.as_str()
            ),
            Self::InvalidRuntimeFlow { message } => {
                write!(formatter, "INVALID_RUNTIME_FLOW: {message}")
            }
            Self::InvalidNextDefinition { message } => {
                write!(formatter, "INVALID_NEXT_DEFINITION: {message}")
            }
            Self::CompletionNotReady => formatter.write_str(
                "COMPLETION_NOT_READY: gameplay has not reached its authoritative completion carrier",
            ),
            Self::InvalidCompletion { message } => {
                write!(formatter, "INVALID_COMPLETION: {message}")
            }
            Self::GenerationExhausted => {
                formatter.write_str("GENERATION_EXHAUSTED: runtime generation overflow")
            }
        }
    }
}

impl Error for RuntimeTransitionError {}

#[derive(Debug)]
pub enum OpenLevelError<E> {
    Transition(RuntimeTransitionError),
    Session(OpenSessionError<E>),
}

impl<E: fmt::Display> fmt::Display for OpenLevelError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transition(error) => error.fmt(formatter),
            Self::Session(error) => error.fmt(formatter),
        }
    }
}

impl<E: Error + 'static> Error for OpenLevelError<E> {}

#[derive(Debug)]
enum RuntimeState<B> {
    Title,
    Loading {
        definition: Arc<GameDefinition>,
        session_generation: NonZeroU64,
    },
    Playing {
        session: GameSession<B>,
        paused: bool,
    },
}

/// Rust-owned outer runtime state above the existing gameplay session.
#[derive(Debug)]
pub struct GameRuntime<B> {
    runtime_id: u64,
    generation: u64,
    state: RuntimeState<B>,
}

impl<B: GameSessionBackend> GameRuntime<B> {
    /// Construct a runtime at the title boundary.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeTransitionError::InvalidRuntimeId`] for identifier
    /// zero.
    pub fn title(runtime_id: u64) -> Result<Self, RuntimeTransitionError> {
        if runtime_id == 0 {
            return Err(RuntimeTransitionError::InvalidRuntimeId);
        }
        Ok(Self {
            runtime_id,
            generation: 1,
            state: RuntimeState::Title,
        })
    }

    #[must_use]
    pub fn identity(&self) -> RuntimeIdentity {
        let (mode, pending_definition, active_runtime_flow, session) = match &self.state {
            RuntimeState::Title => (RuntimeMode::Title, None, None, None),
            RuntimeState::Loading { definition, .. } => (
                RuntimeMode::Loading,
                Some(RuntimeDefinitionIdentity::from_definition(definition)),
                None,
                None,
            ),
            RuntimeState::Playing { session, paused } => (
                if *paused {
                    RuntimeMode::Paused
                } else {
                    RuntimeMode::Playing
                },
                None,
                session.definition().runtime_flow().cloned(),
                Some(session.identity()),
            ),
        };
        RuntimeIdentity {
            runtime_contract: GAME_RUNTIME_CONTRACT,
            runtime_id: self.runtime_id,
            generation: self.generation,
            mode,
            pending_definition,
            active_runtime_flow,
            session,
        }
    }

    /// Enter the loading boundary with one immutable definition.
    ///
    /// # Errors
    ///
    /// Rejects stale guards, non-title callers, and generation overflow
    /// without changing the runtime.
    pub fn new_game(
        &mut self,
        guard: RuntimeGuard,
        definition: Arc<GameDefinition>,
    ) -> Result<RuntimeIdentity, RuntimeTransitionError> {
        self.validate_guard(guard)
            .map_err(Box::new)
            .map_err(RuntimeTransitionError::Guard)?;
        if !matches!(self.state, RuntimeState::Title) {
            return Err(RuntimeTransitionError::InvalidMode {
                expected: RuntimeMode::Title,
                actual: self.identity().mode,
            });
        }
        let runtime_flow = definition.runtime_flow().ok_or_else(|| {
            RuntimeTransitionError::InvalidRuntimeFlow {
                message: "new-game definition has no semantic runtime flow".to_owned(),
            }
        })?;
        if runtime_flow.bound_level_id() != runtime_flow.entry_level_id() {
            return Err(RuntimeTransitionError::InvalidRuntimeFlow {
                message: format!(
                    "new-game definition binds {}, but flow entry is {}",
                    runtime_flow.bound_level_id().as_str(),
                    runtime_flow.entry_level_id().as_str()
                ),
            });
        }
        let next_generation = self.next_generation()?;
        self.state = RuntimeState::Loading {
            definition,
            session_generation: NonZeroU64::MIN,
        };
        self.generation = next_generation;
        Ok(self.identity())
    }

    /// Open the pending definition through a concrete gameplay adapter.
    ///
    /// # Errors
    ///
    /// Rejects stale guards, non-loading callers, generation overflow, and
    /// adapter/session failures without changing the runtime.
    pub fn open_level<A>(
        &mut self,
        guard: RuntimeGuard,
        adapter: &A,
        input: A::OpenInput,
        session_id: u64,
    ) -> Result<RuntimeIdentity, OpenLevelError<A::Error>>
    where
        A: GameAdapter<Backend = B>,
    {
        self.validate_guard(guard)
            .map_err(Box::new)
            .map_err(RuntimeTransitionError::Guard)
            .map_err(OpenLevelError::Transition)?;
        let (definition, session_generation) = match &self.state {
            RuntimeState::Loading {
                definition,
                session_generation,
            } => (Arc::clone(definition), *session_generation),
            _ => {
                return Err(OpenLevelError::Transition(
                    RuntimeTransitionError::InvalidMode {
                        expected: RuntimeMode::Loading,
                        actual: self.identity().mode,
                    },
                ));
            }
        };
        let next_generation = self.next_generation().map_err(OpenLevelError::Transition)?;
        let session = GameSession::open_with_generation(
            adapter,
            definition,
            session_id,
            session_generation,
            input,
        )
        .map_err(OpenLevelError::Session)?;
        self.state = RuntimeState::Playing {
            session,
            paused: false,
        };
        self.generation = next_generation;
        Ok(self.identity())
    }

    /// Accept one authoritative level completion and bind the exact next
    /// semantic definition at the existing loading boundary.
    ///
    /// This is an engine-internal transition seam until the native resolver
    /// can supply the next validated package without letting a client choose
    /// a raw package path. The current gameplay session remains unchanged on
    /// every rejection. A successful transition discards it exactly once and
    /// carries a monotonically increasing session generation into the next
    /// [`Self::open_level`].
    #[doc(hidden)]
    pub fn accept_transition(
        &mut self,
        guard: RuntimeGuard,
        session_guard: SessionGuard,
        next_definition: Arc<GameDefinition>,
    ) -> Result<CompletionTransitionReceipt, RuntimeTransitionError> {
        self.validate_guard(guard)
            .map_err(Box::new)
            .map_err(RuntimeTransitionError::Guard)?;
        let before = self.identity();
        if before.mode != RuntimeMode::Playing {
            return Err(RuntimeTransitionError::InvalidMode {
                expected: RuntimeMode::Playing,
                actual: before.mode,
            });
        }
        let RuntimeState::Playing {
            session,
            paused: false,
        } = &self.state
        else {
            unreachable!("validated Playing mode owns an unpaused gameplay session")
        };
        let completion = match session
            .completion_status(session_guard)
            .map_err(Box::new)
            .map_err(RuntimeTransitionError::SessionGuard)?
            .value
        {
            GameCompletionStatus::Unsupported => {
                return Err(RuntimeTransitionError::InvalidCompletion {
                    message: "gameplay backend does not expose semantic completion".to_owned(),
                });
            }
            GameCompletionStatus::InProgress => {
                return Err(RuntimeTransitionError::CompletionNotReady);
            }
            GameCompletionStatus::Completed { completion } => completion,
            GameCompletionStatus::Invalid { message } => {
                return Err(RuntimeTransitionError::InvalidCompletion { message });
            }
        };
        let source_session = session.identity();
        let current_flow = session.definition().runtime_flow().ok_or_else(|| {
            RuntimeTransitionError::InvalidRuntimeFlow {
                message: "playing definition has no semantic runtime flow".to_owned(),
            }
        })?;
        let current_entry = current_flow
            .entries()
            .iter()
            .find(|entry| entry.level_id() == current_flow.bound_level_id())
            .expect("validated runtime flow contains its bound level");
        let completed_level_id = current_entry.level_id().clone();
        let completion_destination = current_entry.completion_destination().clone();
        let RuntimeFlowDestination::Level {
            level_id: next_level,
        } = &completion_destination
        else {
            return Err(RuntimeTransitionError::InvalidRuntimeFlow {
                message: "terminal Title transition is not yet supported by this runtime seam"
                    .to_owned(),
            });
        };
        let candidate_flow = next_definition.runtime_flow().ok_or_else(|| {
            RuntimeTransitionError::InvalidRuntimeFlow {
                message: "next definition has no semantic runtime flow".to_owned(),
            }
        })?;
        next_definition.verify_identity().map_err(|error| {
            RuntimeTransitionError::InvalidNextDefinition {
                message: error.to_string(),
            }
        })?;
        if candidate_flow.game_id() != current_flow.game_id()
            || candidate_flow.entry_level_id() != current_flow.entry_level_id()
            || candidate_flow.entries() != current_flow.entries()
        {
            return Err(RuntimeTransitionError::InvalidRuntimeFlow {
                message:
                    "next definition does not retain the active semantic game and ordered flow"
                        .to_owned(),
            });
        }
        if candidate_flow.bound_level_id() != next_level {
            return Err(RuntimeTransitionError::InvalidRuntimeFlow {
                message: format!(
                    "completion of {} requires {}, but candidate binds {}",
                    completed_level_id.as_str(),
                    next_level.as_str(),
                    candidate_flow.bound_level_id().as_str(),
                ),
            });
        }
        let next_runtime_generation = self.next_generation()?;
        let next_session_generation = source_session
            .generation
            .checked_add(1)
            .and_then(NonZeroU64::new)
            .ok_or(RuntimeTransitionError::GenerationExhausted)?;

        self.state = RuntimeState::Loading {
            definition: next_definition,
            session_generation: next_session_generation,
        };
        self.generation = next_runtime_generation;
        let after = self.identity();
        let receipt_sha256 = completion_receipt_sha256(
            &before,
            &after,
            &source_session,
            &completed_level_id,
            &completion_destination,
            &completion,
        );
        Ok(CompletionTransitionReceipt {
            receipt_contract: COMPLETION_TRANSITION_RECEIPT,
            before,
            after,
            source_session,
            completed_level_id,
            completion_destination,
            completion,
            receipt_sha256,
        })
    }

    /// Enter a quiescent pause boundary without executing a gameplay or
    /// package-specific paused frame.
    ///
    /// The same definition-bound session remains owned by this runtime. Only
    /// the outer and nested lifecycle generations advance; backend state and
    /// the gameplay tick remain unchanged.
    pub fn pause(
        &mut self,
        guard: RuntimeGuard,
    ) -> Result<RuntimeIdentity, RuntimeTransitionError> {
        self.set_paused(guard, false, true)
    }

    /// Leave the quiescent pause boundary without executing a gameplay step.
    pub fn resume(
        &mut self,
        guard: RuntimeGuard,
    ) -> Result<RuntimeIdentity, RuntimeTransitionError> {
        self.set_paused(guard, true, false)
    }

    /// Observationally borrow the sole gameplay authority while the runtime is
    /// playing or paused.
    #[must_use]
    pub fn session(&self) -> Option<&GameSession<B>> {
        match &self.state {
            RuntimeState::Playing { session, .. } => Some(session),
            RuntimeState::Title | RuntimeState::Loading { .. } => None,
        }
    }

    /// Mutably borrow the sole gameplay authority only while the runtime is
    /// playing. The session's own generation/tick guards remain mandatory for
    /// every gameplay mutation.
    pub fn session_mut(&mut self) -> Option<&mut GameSession<B>> {
        match &mut self.state {
            RuntimeState::Playing {
                session,
                paused: false,
            } => Some(session),
            RuntimeState::Playing { paused: true, .. }
            | RuntimeState::Title
            | RuntimeState::Loading { .. } => None,
        }
    }

    fn set_paused(
        &mut self,
        guard: RuntimeGuard,
        expected_paused: bool,
        next_paused: bool,
    ) -> Result<RuntimeIdentity, RuntimeTransitionError> {
        self.validate_guard(guard)
            .map_err(Box::new)
            .map_err(RuntimeTransitionError::Guard)?;
        let expected_mode = if expected_paused {
            RuntimeMode::Paused
        } else {
            RuntimeMode::Playing
        };
        let actual_mode = self.identity().mode;
        if actual_mode != expected_mode {
            return Err(RuntimeTransitionError::InvalidMode {
                expected: expected_mode,
                actual: actual_mode,
            });
        }
        let next_runtime_generation = self.next_generation()?;
        let RuntimeState::Playing { session, paused } = &mut self.state else {
            unreachable!("validated runtime mode owns a gameplay session")
        };
        let session_guard = session.identity().guard();
        session
            .advance_lifecycle_generation(session_guard)
            .map_err(Box::new)
            .map_err(RuntimeTransitionError::SessionGuard)?;
        *paused = next_paused;
        self.generation = next_runtime_generation;
        Ok(self.identity())
    }

    fn next_generation(&self) -> Result<u64, RuntimeTransitionError> {
        self.generation
            .checked_add(1)
            .ok_or(RuntimeTransitionError::GenerationExhausted)
    }

    fn validate_guard(&self, guard: RuntimeGuard) -> Result<(), RuntimeGuardError> {
        let expected = self.identity();
        let code = if guard.runtime_id == 0 {
            Some(RuntimeGuardErrorCode::InvalidRuntimeId)
        } else if guard.runtime_id != expected.runtime_id {
            Some(RuntimeGuardErrorCode::RuntimeMismatch)
        } else if guard.generation != expected.generation {
            Some(RuntimeGuardErrorCode::GenerationMismatch)
        } else {
            None
        };
        match code {
            Some(code) => Err(RuntimeGuardError {
                code,
                expected,
                actual: guard,
            }),
            None => Ok(()),
        }
    }
}

fn completion_receipt_sha256(
    before: &RuntimeIdentity,
    after: &RuntimeIdentity,
    source_session: &SessionIdentity,
    completed_level_id: &SemanticLevelId,
    completion_destination: &RuntimeFlowDestination,
    completion: &GameCompletion,
) -> String {
    let payload = CompletionTransitionReceiptPayload {
        receipt_contract: COMPLETION_TRANSITION_RECEIPT,
        before,
        after,
        source_session,
        completed_level_id,
        completion_destination,
        completion,
    };
    let bytes = serde_json::to_vec(&payload)
        .expect("completion transition receipt payload is serializable");
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_definition::{
        RuntimeFlowDefinition, RuntimeFlowDestination, RuntimeFlowEntry, RuntimeFlowNodeId,
        SemanticGameId, SemanticLevelId,
    };
    use crate::{
        campaign::{
            backend::{
                CampaignBackendError, Mc2CampaignAdapter, Mc2CampaignBackend, Mc2CampaignInputs,
                load_mc2_campaign,
            },
            spec::CampaignSelection,
        },
        game_definition::{DefinitionErrorCode, default_catalog},
        game_session::{GameCommand, GameSessionError, SessionGuardErrorCode},
    };
    use std::path::PathBuf;

    fn level_one_package() -> PathBuf {
        std::env::var_os("MC_CAMPAIGN_LEVEL1_PACKAGE_V1").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf15-level1-package-current")
            },
            PathBuf::from,
        )
    }

    fn level_two_package() -> PathBuf {
        std::env::var_os("MC_CAMPAIGN_PACKAGE_V1").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf02-campaign-package-v1-a/qualified-level2")
            },
            PathBuf::from,
        )
    }

    fn level_three_package() -> PathBuf {
        std::env::var_os("MC_CAMPAIGN_PACKAGE_LEVEL3_V1").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf14-campaign-package-v1-a/qualified-level3")
            },
            PathBuf::from,
        )
    }

    #[test]
    fn runtime_loading_rejects_wrong_package_then_opens_real_level_one() {
        let catalog = default_catalog().unwrap();
        let level_one = level_one_package();
        let definition = Arc::new(load_mc2_campaign(&level_one, &catalog).unwrap());
        let expected_definition = RuntimeDefinitionIdentity::from_definition(&definition);

        let mut runtime = GameRuntime::<Mc2CampaignBackend>::title(41).unwrap();
        let title = runtime.identity();
        assert_eq!(title.mode, RuntimeMode::Title);
        assert_eq!(title.generation, 1);
        assert!(runtime.session().is_none());

        let stale = RuntimeGuard {
            generation: title.generation + 1,
            ..title.guard()
        };
        let stale_result = runtime.new_game(stale, Arc::clone(&definition));
        assert!(matches!(
            stale_result,
            Err(RuntimeTransitionError::Guard(error))
                if error.code == RuntimeGuardErrorCode::GenerationMismatch
        ));
        assert_eq!(runtime.identity(), title);

        let later_definition = Arc::new(load_mc2_campaign(level_two_package(), &catalog).unwrap());
        let later_result = runtime.new_game(title.guard(), later_definition);
        assert!(matches!(
            later_result,
            Err(RuntimeTransitionError::InvalidRuntimeFlow { .. })
        ));
        assert_eq!(runtime.identity(), title);

        let semantic_level = definition.level_definition().unwrap().clone();
        let spire = SemanticLevelId::parse("level.magic_carpet_2.night_spire").unwrap();
        let payahandra = SemanticLevelId::parse("level.magic_carpet_2.payahandra_ruins").unwrap();
        let revealed = SemanticLevelId::parse("level.magic_carpet_2.revealed_way").unwrap();
        let reordered = RuntimeFlowDefinition::new(
            SemanticGameId::parse("game.magic_carpet_2").unwrap(),
            spire.clone(),
            spire.clone(),
            vec![
                RuntimeFlowEntry::new(
                    spire,
                    RuntimeFlowDestination::Level {
                        level_id: revealed.clone(),
                    },
                ),
                RuntimeFlowEntry::new(
                    revealed,
                    RuntimeFlowDestination::Level {
                        level_id: payahandra.clone(),
                    },
                ),
                RuntimeFlowEntry::new(
                    payahandra,
                    RuntimeFlowDestination::Title {
                        node_id: RuntimeFlowNodeId::parse("flow.magic_carpet_2.return_to_title")
                            .unwrap(),
                    },
                ),
            ],
        )
        .unwrap();
        let reordered_definition = Arc::new(sim_definition::adapter::with_level_and_runtime_flow(
            definition.as_ref().clone(),
            semantic_level,
            reordered,
        ));
        let mut tampered_runtime = GameRuntime::<Mc2CampaignBackend>::title(42).unwrap();
        let tampered_loading = tampered_runtime
            .new_game(tampered_runtime.identity().guard(), reordered_definition)
            .unwrap();
        let rejected = tampered_runtime.open_level(
            tampered_loading.guard(),
            &Mc2CampaignAdapter,
            Mc2CampaignInputs::new(&level_one),
            74,
        );
        let Err(OpenLevelError::Session(OpenSessionError::Adapter(
            CampaignBackendError::Definition(error),
        ))) = rejected
        else {
            panic!("real L1 open must reject a reordered semantic flow")
        };
        assert_eq!(error.code, DefinitionErrorCode::HashMismatch);
        assert_eq!(error.path_or_id, "definition.runtime_flow");
        assert_eq!(tampered_runtime.identity(), tampered_loading);
        assert!(tampered_runtime.session().is_none());

        let loading = runtime
            .new_game(title.guard(), Arc::clone(&definition))
            .unwrap();
        assert_eq!(loading.mode, RuntimeMode::Loading);
        assert_eq!(loading.generation, 2);
        assert_eq!(loading.pending_definition, Some(expected_definition));
        let loading_flow = loading
            .pending_definition
            .as_ref()
            .and_then(|identity| identity.runtime_flow.as_ref())
            .expect("loading identity carries the immutable semantic runtime flow");
        assert_eq!(loading_flow.game_id().as_str(), "game.magic_carpet_2");
        assert_eq!(
            loading_flow.bound_level_id().as_str(),
            "level.magic_carpet_2.night_spire"
        );
        assert!(loading.active_runtime_flow.is_none());
        assert!(loading.session.is_none());

        let before_mismatch = runtime.identity();
        let mismatch = runtime.open_level(
            before_mismatch.guard(),
            &Mc2CampaignAdapter,
            Mc2CampaignInputs::new(level_two_package()),
            73,
        );
        let Err(OpenLevelError::Session(OpenSessionError::Adapter(
            CampaignBackendError::Definition(error),
        ))) = mismatch
        else {
            panic!("L1 definition paired with L2 package must fail semantic identity validation")
        };
        assert_eq!(error.code, DefinitionErrorCode::HashMismatch);
        assert_eq!(error.path_or_id, "definition.package_graph_hash");
        assert_eq!(runtime.identity(), before_mismatch);
        assert!(runtime.session().is_none());

        let playing = runtime
            .open_level(
                before_mismatch.guard(),
                &Mc2CampaignAdapter,
                Mc2CampaignInputs::new(&level_one),
                73,
            )
            .unwrap();
        assert_eq!(playing.mode, RuntimeMode::Playing);
        assert_eq!(playing.generation, 3);
        assert!(playing.pending_definition.is_none());
        assert_eq!(
            playing.active_runtime_flow.as_ref(),
            definition.runtime_flow()
        );
        let session_identity = playing.session.as_ref().unwrap();
        assert_eq!(session_identity.session_id, 73);
        assert_eq!(session_identity.generation, 1);
        assert_eq!(session_identity.tick, 0);
        assert_eq!(
            session_identity.definition_hash,
            definition.definition_hash()
        );

        let session = runtime.session().unwrap();
        assert_eq!(session.identity(), *session_identity);
        assert_eq!(
            session
                .backend()
                .compatibility_campaign()
                .definition()
                .identity,
            CampaignSelection::LEVEL1
        );
    }

    #[test]
    fn runtime_pause_resume_freezes_real_level_one_and_invalidates_session_guards() {
        let catalog = default_catalog().unwrap();
        let level_one = level_one_package();
        let definition = Arc::new(load_mc2_campaign(&level_one, &catalog).unwrap());
        let mut runtime = GameRuntime::<Mc2CampaignBackend>::title(91).unwrap();
        let loading = runtime
            .new_game(runtime.identity().guard(), Arc::clone(&definition))
            .unwrap();
        let playing = runtime
            .open_level(
                loading.guard(),
                &Mc2CampaignAdapter,
                Mc2CampaignInputs::new(level_one),
                91,
            )
            .unwrap();
        let before = runtime.session().unwrap().snapshot();
        let pre_pause_session_guard = before.identity.guard();

        let paused = runtime.pause(playing.guard()).unwrap();
        assert_eq!(paused.mode, RuntimeMode::Paused);
        assert_eq!(paused.generation, 4);
        let paused_session = paused.session.as_ref().unwrap();
        assert_eq!(paused_session.session_id, before.identity.session_id);
        assert_eq!(paused_session.generation, 2);
        assert_eq!(paused_session.tick, before.identity.tick);
        assert_eq!(paused.active_runtime_flow, playing.active_runtime_flow);
        assert_eq!(
            paused_session.definition_hash,
            before.identity.definition_hash
        );
        assert!(runtime.session_mut().is_none());
        let paused_snapshot = runtime.session().unwrap().snapshot();
        assert_eq!(paused_snapshot.value, before.value);
        assert_eq!(paused_snapshot.identity, *paused_session);
        assert_eq!(runtime.session().unwrap().snapshot(), paused_snapshot);

        let before_stale_pause = runtime.identity();
        let stale_pause = runtime.pause(playing.guard());
        assert!(matches!(
            stale_pause,
            Err(RuntimeTransitionError::Guard(error))
                if error.code == RuntimeGuardErrorCode::GenerationMismatch
        ));
        assert_eq!(runtime.identity(), before_stale_pause);
        assert_eq!(runtime.session().unwrap().snapshot(), paused_snapshot);

        let resumed = runtime.pause(paused.guard());
        assert!(matches!(
            resumed,
            Err(RuntimeTransitionError::InvalidMode {
                expected: RuntimeMode::Playing,
                actual: RuntimeMode::Paused,
            })
        ));
        assert_eq!(runtime.identity(), before_stale_pause);

        let resumed = runtime.resume(paused.guard()).unwrap();
        assert_eq!(resumed.active_runtime_flow, playing.active_runtime_flow);
        assert_eq!(resumed.mode, RuntimeMode::Playing);
        assert_eq!(resumed.generation, 5);
        let resumed_session = resumed.session.as_ref().unwrap();
        assert_eq!(resumed_session.generation, 3);
        assert_eq!(resumed_session.tick, before.identity.tick);
        let after_resume = runtime.session().unwrap().snapshot();
        assert_eq!(after_resume.value, before.value);

        let stale_session_step = runtime
            .session_mut()
            .unwrap()
            .step(pre_pause_session_guard, GameCommand::default());
        assert!(matches!(
            stale_session_step,
            Err(GameSessionError::Guard(error))
                if error.code == SessionGuardErrorCode::GenerationMismatch
        ));
        assert_eq!(runtime.session().unwrap().snapshot(), after_resume);

        let current_guard = runtime.session().unwrap().identity().guard();
        let stepped = runtime
            .session_mut()
            .unwrap()
            .step(current_guard, GameCommand::default())
            .unwrap();
        assert_eq!(stepped.identity.tick, before.identity.tick + 1);

        // No-Claim: this gate proves only the quiescent runtime boundary and
        // guard invalidation. There is no paused_frame operation here, so it
        // does not prove the L1 pre-gate RNG/objective/presentation/audio
        // prefix, replay-visible paused time, L2/L3 pause policy, or pause UI.
    }

    #[test]
    fn runtime_completed_real_level_one_accepts_only_level_two_into_loading_once() {
        let catalog = default_catalog().unwrap();
        let level_one = level_one_package();
        let level_two = level_two_package();
        let level_three = level_three_package();
        let level_one_definition = Arc::new(load_mc2_campaign(&level_one, &catalog).unwrap());
        let level_two_definition = Arc::new(load_mc2_campaign(&level_two, &catalog).unwrap());
        let level_three_definition = Arc::new(load_mc2_campaign(&level_three, &catalog).unwrap());
        let mut runtime = GameRuntime::<Mc2CampaignBackend>::title(111).unwrap();
        let loading = runtime
            .new_game(runtime.identity().guard(), level_one_definition)
            .unwrap();
        let playing = runtime
            .open_level(
                loading.guard(),
                &Mc2CampaignAdapter,
                Mc2CampaignInputs::new(&level_one),
                111,
            )
            .unwrap();
        let stale_level_one_tick_zero_guard = playing.session.as_ref().unwrap().guard();

        // Stage only the already-qualified real group-4 boundary. The real
        // package portal/trigger, numeric scheduler, player action-12 phases,
        // journal, persistent completion carrier, and next definition open
        // remain production-owned.
        {
            let campaign = runtime
                .session_mut()
                .unwrap()
                .backend_mut()
                .compatibility_campaign_mut_for_test();
            let session = campaign.level2_compatibility_mut();
            session
                .activate_authored_group_prerequisite_for_test(4)
                .expect("real L1 group 4 materializes the exit boundary");
            let trigger = session
                .economy()
                .authored_switch_producers()
                .iter()
                .find(|source| source.authored_index == 137)
                .copied()
                .expect("real L1 group 4 contains its exit trigger");
            let player = session.state.entity_index;
            session
                .set_level2_entity_position(player, trigger.position)
                .expect("stage the real player inside the real exit trigger");
        }

        let before_start = runtime.identity();
        let before_start_snapshot = runtime.session().unwrap().snapshot();
        let premature = runtime.accept_transition(
            before_start.guard(),
            before_start.session.as_ref().unwrap().guard(),
            Arc::clone(&level_two_definition),
        );
        assert!(matches!(
            premature,
            Err(RuntimeTransitionError::CompletionNotReady)
        ));
        assert_eq!(runtime.identity(), before_start);
        assert_eq!(runtime.session().unwrap().snapshot(), before_start_snapshot);

        for _ in 0..16 {
            let guard = runtime.session().unwrap().identity().guard();
            runtime
                .session_mut()
                .unwrap()
                .step(guard, GameCommand::default())
                .unwrap();
            if runtime
                .session()
                .unwrap()
                .backend()
                .compatibility_campaign()
                .level2_compatibility()
                .level_exit_sequence()
                .active
            {
                break;
            }
        }
        let started = runtime
            .session()
            .unwrap()
            .backend()
            .compatibility_campaign()
            .level2_compatibility()
            .level_exit_sequence();
        assert!(started.active);
        assert!(!started.complete);
        let started_identity = runtime.identity();
        let started_snapshot = runtime.session().unwrap().snapshot();
        let started_rejection = runtime.accept_transition(
            started_identity.guard(),
            started_identity.session.as_ref().unwrap().guard(),
            Arc::clone(&level_two_definition),
        );
        assert!(matches!(
            started_rejection,
            Err(RuntimeTransitionError::CompletionNotReady)
        ));
        assert_eq!(runtime.identity(), started_identity);
        assert_eq!(runtime.session().unwrap().snapshot(), started_snapshot);

        for _ in 0..700 {
            if matches!(
                runtime.session().unwrap().backend().completion_status(),
                GameCompletionStatus::Completed { .. }
            ) {
                break;
            }
            let guard = runtime.session().unwrap().identity().guard();
            runtime
                .session_mut()
                .unwrap()
                .step(guard, GameCommand::default())
                .unwrap();
        }
        let completed_identity = runtime.identity();
        let completed_snapshot = runtime.session().unwrap().snapshot();
        let completion = runtime
            .session()
            .unwrap()
            .completion_status(completed_identity.session.as_ref().unwrap().guard())
            .unwrap()
            .value;
        let GameCompletionStatus::Completed { completion } = completion else {
            panic!("real phase-12 exit completion is projected neutrally")
        };
        assert_eq!(completion.state_hash, completed_snapshot.value.state_hash);

        let event_guard = completed_identity.session.as_ref().unwrap().guard();
        let retained_cursor = completed_snapshot.value.effect_head.saturating_sub(1);
        let empty_page = runtime
            .session()
            .unwrap()
            .events_after(event_guard, retained_cursor, 0)
            .unwrap();
        let first_page = runtime
            .session()
            .unwrap()
            .events_after(event_guard, retained_cursor, 1)
            .unwrap();
        let full_page = runtime
            .session()
            .unwrap()
            .events_after(event_guard, retained_cursor, usize::MAX)
            .unwrap();
        assert!(empty_page.value.events.is_empty());
        assert_eq!(empty_page.value.next_cursor, retained_cursor);
        assert!(empty_page.value.has_more);
        assert_eq!(first_page.value.events.len(), 1);
        assert_eq!(
            first_page.value.next_cursor,
            completed_snapshot.value.effect_head
        );
        assert!(!first_page.value.has_more);
        assert_eq!(full_page.value.events, first_page.value.events);
        assert_eq!(full_page.value.next_cursor, first_page.value.next_cursor);
        assert!(!full_page.value.has_more);
        assert_eq!(
            full_page.value.event_head,
            completed_snapshot.value.effect_head
        );
        assert_eq!(runtime.identity(), completed_identity);
        assert_eq!(runtime.session().unwrap().snapshot(), completed_snapshot);
        assert_eq!(
            runtime
                .session()
                .unwrap()
                .completion_status(event_guard)
                .unwrap()
                .value,
            GameCompletionStatus::Completed {
                completion: completion.clone(),
            }
        );

        {
            runtime
                .session_mut()
                .unwrap()
                .backend_mut()
                .compatibility_campaign_mut_for_test()
                .level2_compatibility_mut()
                .corrupt_completed_exit_result_for_test(9);
        }
        let invalid_identity = runtime.identity();
        let invalid_snapshot = runtime.session().unwrap().snapshot();
        let invalid_completion = runtime.accept_transition(
            invalid_identity.guard(),
            invalid_identity.session.as_ref().unwrap().guard(),
            Arc::clone(&level_two_definition),
        );
        assert!(matches!(
            invalid_completion,
            Err(RuntimeTransitionError::InvalidCompletion { .. })
        ));
        assert_eq!(runtime.identity(), invalid_identity);
        assert_eq!(runtime.session().unwrap().snapshot(), invalid_snapshot);
        runtime
            .session_mut()
            .unwrap()
            .backend_mut()
            .compatibility_campaign_mut_for_test()
            .level2_compatibility_mut()
            .corrupt_completed_exit_result_for_test(10);
        assert_eq!(runtime.identity(), completed_identity);
        assert_eq!(runtime.session().unwrap().snapshot(), completed_snapshot);

        let mut tampered_json = serde_json::to_value(level_two_definition.as_ref()).unwrap();
        tampered_json["definition_hash"] = serde_json::Value::String("sha256:00".to_owned());
        let tampered_definition = Arc::new(serde_json::from_value(tampered_json).unwrap());
        let tampered_next = runtime.accept_transition(
            completed_identity.guard(),
            completed_identity.session.as_ref().unwrap().guard(),
            tampered_definition,
        );
        assert!(matches!(
            tampered_next,
            Err(RuntimeTransitionError::InvalidNextDefinition { .. })
        ));
        assert_eq!(runtime.identity(), completed_identity);
        assert_eq!(runtime.session().unwrap().snapshot(), completed_snapshot);

        let wrong_next = runtime.accept_transition(
            completed_identity.guard(),
            completed_identity.session.as_ref().unwrap().guard(),
            level_three_definition,
        );
        assert!(matches!(
            wrong_next,
            Err(RuntimeTransitionError::InvalidRuntimeFlow { .. })
        ));
        assert_eq!(runtime.identity(), completed_identity);
        assert_eq!(runtime.session().unwrap().snapshot(), completed_snapshot);

        let receipt = runtime
            .accept_transition(
                completed_identity.guard(),
                completed_identity.session.as_ref().unwrap().guard(),
                level_two_definition,
            )
            .unwrap();
        assert_eq!(receipt.receipt_contract, COMPLETION_TRANSITION_RECEIPT);
        assert_eq!(receipt.before, completed_identity);
        assert_eq!(receipt.after, runtime.identity());
        assert_eq!(receipt.source_session, completed_snapshot.identity);
        assert_eq!(receipt.completion, completion);
        assert_eq!(receipt.receipt_sha256.len(), 64);
        assert!(receipt.verify());
        let mut tampered_receipt = receipt.clone();
        tampered_receipt.completion.state_hash ^= 1;
        assert!(!tampered_receipt.verify());
        assert_eq!(
            receipt.completed_level_id.as_str(),
            "level.magic_carpet_2.night_spire"
        );
        assert!(matches!(
            receipt.completion_destination,
            RuntimeFlowDestination::Level { ref level_id }
                if level_id.as_str() == "level.magic_carpet_2.payahandra_ruins"
        ));
        assert_eq!(runtime.identity().mode, RuntimeMode::Loading);
        assert!(runtime.session().is_none());

        let duplicate = runtime.accept_transition(
            completed_identity.guard(),
            completed_identity.session.as_ref().unwrap().guard(),
            Arc::new(load_mc2_campaign(&level_two, &catalog).unwrap()),
        );
        assert!(matches!(
            duplicate,
            Err(RuntimeTransitionError::Guard(error))
                if error.code == RuntimeGuardErrorCode::GenerationMismatch
        ));

        let level_two_playing = runtime
            .open_level(
                runtime.identity().guard(),
                &Mc2CampaignAdapter,
                Mc2CampaignInputs::new(level_two),
                111,
            )
            .unwrap();
        assert_eq!(level_two_playing.mode, RuntimeMode::Playing);
        assert_eq!(level_two_playing.session.as_ref().unwrap().generation, 2);
        assert_eq!(
            level_two_playing
                .active_runtime_flow
                .as_ref()
                .unwrap()
                .bound_level_id()
                .as_str(),
            "level.magic_carpet_2.payahandra_ruins"
        );
        let stale_level_one = runtime
            .session_mut()
            .unwrap()
            .step(stale_level_one_tick_zero_guard, GameCommand::default());
        assert!(matches!(
            stale_level_one,
            Err(GameSessionError::Guard(error))
                if error.code == SessionGuardErrorCode::GenerationMismatch
        ));

        // No-Claim: this focused gate stages the already-separated real L1
        // exit-group prerequisite. It proves authoritative phase-12 capture,
        // exact semantic L1->L2 selection, one guarded Loading transition,
        // real L2 open, and cross-level ABA rejection. It does not prove
        // organic L1 objective progression, L2 completion, L3->Title,
        // native/Unity transition exposure, results UI, save/load, or replay.
    }
}
