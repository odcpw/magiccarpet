//! Immutable definition transactions and one-file atomic workspace publication.
//!
//! The durable state file is the sole commit point. It contains the active
//! definition pointer, retained immutable definitions, active package/artifact
//! identities, and the receipt journal. Live sessions are intentionally absent.

use super::{
    Access, ArtifactId, DefinitionError, DefinitionOperation, GameDefinition, IntegerParameter,
    MutationTiming, OptionParameter, PresentationAssetSlot, SemanticCatalog, SemanticId,
    SemanticValue,
};
use crate::operation::ResolvedOperation;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub const CHANGE_SET_SCHEMA: &str = "carpet-flight-change-set/v3";
pub const WORKSPACE_SCHEMA: &str = "carpet-flight-definition-workspace/v1";
pub const VALIDATION_RECEIPT_SCHEMA: &str = "carpet-flight-validation-receipt/v1";
pub const APPLY_RECEIPT_SCHEMA: &str = "carpet-flight-apply-receipt/v1";

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransactionErrorCode {
    InvalidRequest,
    WorkspaceExists,
    WorkspaceMissing,
    WorkspaceCorrupt,
    ReadOnly,
    DefinitionMismatch,
    Conflict,
    ValidationRejected,
    ReceiptNotFound,
    RetainedDefinitionMissing,
    InterruptedAfterCommit,
    Io,
}

impl TransactionErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::WorkspaceExists => "WORKSPACE_EXISTS",
            Self::WorkspaceMissing => "WORKSPACE_MISSING",
            Self::WorkspaceCorrupt => "WORKSPACE_CORRUPT",
            Self::ReadOnly => "READ_ONLY",
            Self::DefinitionMismatch => "DEFINITION_MISMATCH",
            Self::Conflict => "CONFLICT",
            Self::ValidationRejected => "VALIDATION_REJECTED",
            Self::ReceiptNotFound => "RECEIPT_NOT_FOUND",
            Self::RetainedDefinitionMissing => "RETAINED_DEFINITION_MISSING",
            Self::InterruptedAfterCommit => "INTERRUPTED_AFTER_COMMIT",
            Self::Io => "IO",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionError {
    pub code: TransactionErrorCode,
    pub path_or_id: String,
    pub message: String,
    pub state_changed: bool,
    pub retryable: bool,
}

impl TransactionError {
    fn new(
        code: TransactionErrorCode,
        path_or_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            path_or_id: path_or_id.into(),
            message: message.into(),
            state_changed: false,
            retryable: false,
        }
    }

    fn interrupted(idempotency_key: &str) -> Self {
        Self {
            code: TransactionErrorCode::InterruptedAfterCommit,
            path_or_id: idempotency_key.to_owned(),
            message: "publication committed; recover the receipt with apply-status".to_owned(),
            state_changed: true,
            retryable: true,
        }
    }

    fn after_commit(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: TransactionErrorCode::InterruptedAfterCommit,
            path_or_id: path.into(),
            message: message.into(),
            state_changed: true,
            retryable: true,
        }
    }

    #[must_use]
    pub fn to_json(&self, operation: &str) -> Value {
        json!({
            "code": self.code.as_str(),
            "message": self.message,
            "path_or_id": self.path_or_id,
            "operation": operation,
            "retryable": self.retryable,
            "state_changed": self.state_changed,
            "remediation": if self.code == TransactionErrorCode::InterruptedAfterCommit {
                "call apply-status with the same workspace and idempotency key"
            } else if self.code == TransactionErrorCode::DefinitionMismatch {
                "inspect the workspace and build a new guarded change set"
            } else {
                "repair the named input and validate again"
            },
            "safe_alternative": if self.code == TransactionErrorCode::InterruptedAfterCommit {
                "apply-status"
            } else {
                "preview"
            },
            "expected_identity": Value::Null,
            "actual_identity": Value::Null,
        })
    }
}

impl fmt::Display for TransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at {}: {}",
            self.code.as_str(),
            self.path_or_id,
            self.message
        )
    }
}

impl std::error::Error for TransactionError {}

impl From<DefinitionError> for TransactionError {
    fn from(error: DefinitionError) -> Self {
        let code = match error.message.as_str() {
            "semantic descriptor is read-only at this engine revision" => {
                TransactionErrorCode::ReadOnly
            }
            _ => TransactionErrorCode::InvalidRequest,
        };
        Self::new(code, error.path_or_id, error.message)
    }
}

type Result<T> = std::result::Result<T, TransactionError>;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestedTiming {
    SessionReset,
    PackageReload,
}

impl RequestedTiming {
    fn permits(self, required: MutationTiming) -> bool {
        matches!(
            (self, required),
            (Self::SessionReset, MutationTiming::SessionReset)
                | (
                    Self::PackageReload,
                    MutationTiming::SessionReset | MutationTiming::PackageReload
                )
        )
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::SessionReset => "session_reset",
            Self::PackageReload => "package_reload",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeSet {
    pub schema_version: String,
    pub target_workspace: String,
    pub expected_definition_hash: String,
    pub expected_definition_revision: u64,
    pub idempotency_key: String,
    pub operations: Vec<DefinitionOperation>,
    pub requested_timing: RequestedTiming,
    #[serde(default)]
    pub intent: Option<String>,
}

impl ChangeSet {
    /// Canonicalize and validate the request-level shape without consulting or
    /// mutating a workspace.
    pub fn canonicalized(mut self) -> Result<Self> {
        if self.schema_version != CHANGE_SET_SCHEMA {
            return Err(TransactionError::new(
                TransactionErrorCode::InvalidRequest,
                "schema_version",
                "unsupported change-set schema",
            ));
        }
        validate_token(&self.target_workspace, "target_workspace")?;
        validate_token(&self.idempotency_key, "idempotency_key")?;
        validate_content_hash(&self.expected_definition_hash, "expected_definition_hash")?;
        if self.expected_definition_revision == 0 {
            return Err(TransactionError::new(
                TransactionErrorCode::InvalidRequest,
                "expected_definition_revision",
                "definition revision must be positive",
            ));
        }
        if self.operations.is_empty() {
            return Err(TransactionError::new(
                TransactionErrorCode::InvalidRequest,
                "operations",
                "change set must contain at least one typed operation",
            ));
        }
        if self.operations.len() > 100 {
            return Err(TransactionError::new(
                TransactionErrorCode::InvalidRequest,
                "operations",
                "change set exceeds the hard limit of 100 operations",
            ));
        }
        if self
            .intent
            .as_ref()
            .is_some_and(|intent| intent.len() > 1_024)
        {
            return Err(TransactionError::new(
                TransactionErrorCode::InvalidRequest,
                "intent",
                "non-authoritative intent exceeds the hard limit of 1024 bytes",
            ));
        }
        self.operations.sort_by_key(DefinitionOperation::target_id);
        if self
            .operations
            .windows(2)
            .any(|pair| pair[0].target_id() == pair[1].target_id())
        {
            return Err(TransactionError::new(
                TransactionErrorCode::InvalidRequest,
                "operations",
                "change set repeats a semantic target",
            ));
        }
        Ok(self)
    }

    #[must_use]
    pub fn canonical_hash(&self) -> String {
        content_hash(self)
    }
}

#[derive(Clone, Debug)]
pub struct ChangeSetBuilder {
    change_set: ChangeSet,
}

impl ChangeSetBuilder {
    #[must_use]
    pub fn new(
        workspace: impl Into<String>,
        definition: &GameDefinition,
        idempotency_key: impl Into<String>,
        requested_timing: RequestedTiming,
    ) -> Self {
        Self {
            change_set: ChangeSet {
                schema_version: CHANGE_SET_SCHEMA.to_owned(),
                target_workspace: workspace.into(),
                expected_definition_hash: definition.definition_hash().to_owned(),
                expected_definition_revision: definition.revision(),
                idempotency_key: idempotency_key.into(),
                operations: Vec::new(),
                requested_timing,
                intent: None,
            },
        }
    }

    #[must_use]
    pub fn set_integer(mut self, parameter: IntegerParameter, value: i64) -> Self {
        self.change_set
            .operations
            .push(DefinitionOperation::SetInteger {
                parameter,
                value,
                expected_value: None,
            });
        self
    }

    #[must_use]
    pub fn select_option(mut self, parameter: OptionParameter, option: impl Into<String>) -> Self {
        self.change_set
            .operations
            .push(DefinitionOperation::SelectOption {
                parameter,
                option: option.into(),
                expected_option: None,
            });
        self
    }

    #[must_use]
    pub fn bind_asset(mut self, slot: PresentationAssetSlot, artifact: ArtifactId) -> Self {
        self.change_set
            .operations
            .push(DefinitionOperation::BindAsset {
                slot,
                artifact,
                expected_artifact: None,
            });
        self
    }

    #[must_use]
    pub fn move_entity(
        mut self,
        entity: crate::level_definition::LevelEntityId,
        position: crate::level_definition::TilePosition,
    ) -> Self {
        self.change_set
            .operations
            .push(DefinitionOperation::MoveEntity {
                entity,
                position,
                expected_position: None,
            });
        self
    }

    #[must_use]
    pub fn move_entity_from(
        mut self,
        entity: crate::level_definition::LevelEntityId,
        expected_position: crate::level_definition::TilePosition,
        position: crate::level_definition::TilePosition,
    ) -> Self {
        self.change_set
            .operations
            .push(DefinitionOperation::MoveEntity {
                entity,
                position,
                expected_position: Some(expected_position),
            });
        self
    }

    #[must_use]
    pub fn operation(mut self, operation: DefinitionOperation) -> Self {
        self.change_set.operations.push(operation);
        self
    }

    #[must_use]
    pub fn intent(mut self, intent: impl Into<String>) -> Self {
        self.change_set.intent = Some(intent.into());
        self
    }

    pub fn build(self) -> Result<ChangeSet> {
        self.change_set.canonicalized()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub path_or_id: String,
    pub message: String,
}

impl From<DefinitionError> for Diagnostic {
    fn from(error: DefinitionError) -> Self {
        Self {
            code: error.code.as_str().to_owned(),
            path_or_id: error.path_or_id,
            message: error.message,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ValidationReceipt {
    pub schema: String,
    pub receipt_hash: String,
    pub change_set_hash: String,
    pub target_workspace: String,
    pub expected_definition_hash: String,
    pub expected_definition_revision: u64,
    pub candidate_definition_hash: Option<String>,
    pub candidate_definition_revision: Option<u64>,
    pub affected_ids: Vec<String>,
    pub required_timing: Option<RequestedTiming>,
    pub accepted: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub validator_identity: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DefinitionDiff {
    pub id: String,
    pub before: SemanticValue,
    pub after: SemanticValue,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ApplyReceipt {
    pub schema: String,
    pub receipt_hash: String,
    pub transaction_id: String,
    pub operation: String,
    pub workspace_id: String,
    pub idempotency_key: String,
    pub change_set_hash: String,
    pub validation_receipt_hash: String,
    pub prior_definition_hash: String,
    pub prior_definition_revision: u64,
    pub resulting_definition_hash: String,
    pub resulting_definition_revision: u64,
    pub prior_package_graph_hash: String,
    pub resulting_package_graph_hash: String,
    pub prior_artifact_graph_hash: String,
    pub resulting_artifact_graph_hash: String,
    pub commit_point: String,
    pub mutation_timing: RequestedTiming,
    pub affected_existing_sessions: Vec<String>,
    pub applied_ids: Vec<String>,
    pub read_back_hash: String,
    pub rollback_of_transaction: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplyStatus {
    NotStarted,
    Committed(ApplyReceipt),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkspaceInspection {
    pub workspace_id: String,
    pub active_definition_hash: String,
    pub active_definition_revision: u64,
    pub package_graph_hash: String,
    pub artifact_graph_hash: String,
    pub retained_definition_count: usize,
    pub committed_receipt_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationFault {
    None,
    BeforeWrite,
    BeforeCommit,
    #[doc(hidden)]
    PauseBeforeCommit,
    AfterRename,
    AfterCommit,
    #[doc(hidden)]
    PauseAfterCommit,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceState {
    schema: String,
    workspace_id: String,
    active_definition_hash: String,
    definitions: BTreeMap<String, GameDefinition>,
    receipts: BTreeMap<String, ApplyReceipt>,
}

impl WorkspaceState {
    fn active(&self) -> Result<&GameDefinition> {
        self.definitions
            .get(&self.active_definition_hash)
            .ok_or_else(|| {
                TransactionError::new(
                    TransactionErrorCode::WorkspaceCorrupt,
                    "active_definition_hash",
                    "active pointer does not name a retained definition",
                )
            })
    }

    fn verify(&self) -> Result<()> {
        if self.schema != WORKSPACE_SCHEMA {
            return Err(TransactionError::new(
                TransactionErrorCode::WorkspaceCorrupt,
                "workspace.schema",
                "unsupported workspace schema",
            ));
        }
        validate_token(&self.workspace_id, "workspace_id")?;
        for (hash, definition) in &self.definitions {
            definition.verify_identity().map_err(|error| {
                TransactionError::new(
                    TransactionErrorCode::WorkspaceCorrupt,
                    error.path_or_id,
                    error.message,
                )
            })?;
            if hash != definition.definition_hash() {
                return Err(TransactionError::new(
                    TransactionErrorCode::WorkspaceCorrupt,
                    hash,
                    "retained definition map key does not match definition identity",
                ));
            }
        }
        let _ = self.active()?;
        for (key, receipt) in &self.receipts {
            if key != &receipt.idempotency_key || receipt.receipt_hash != receipt_hash(receipt) {
                return Err(TransactionError::new(
                    TransactionErrorCode::WorkspaceCorrupt,
                    key,
                    "receipt journal identity is inconsistent",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct WorkspaceStore {
    root: PathBuf,
}

impl WorkspaceStore {
    /// Create a durable workspace whose first active definition is already
    /// validated. Existing workspace state is never overwritten.
    pub fn create(
        root: impl AsRef<Path>,
        workspace_id: &str,
        definition: GameDefinition,
    ) -> Result<Self> {
        validate_token(workspace_id, "workspace_id")?;
        definition
            .verify_identity()
            .map_err(TransactionError::from)?;
        let store = Self {
            root: root.as_ref().to_path_buf(),
        };
        fs::create_dir_all(&store.root).map_err(|error| store.io_error("workspace", error))?;
        let _lock = store.lock()?;
        if store.state_path().exists() {
            return Err(TransactionError::new(
                TransactionErrorCode::WorkspaceExists,
                store.state_path().display().to_string(),
                "workspace state already exists",
            ));
        }
        let active_definition_hash = definition.definition_hash().to_owned();
        let mut definitions = BTreeMap::new();
        definitions.insert(active_definition_hash.clone(), definition);
        let state = WorkspaceState {
            schema: WORKSPACE_SCHEMA.to_owned(),
            workspace_id: workspace_id.to_owned(),
            active_definition_hash,
            definitions,
            receipts: BTreeMap::new(),
        };
        store.write_state(&state, PublicationFault::None)?;
        Ok(store)
    }

    #[must_use]
    pub fn open(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn inspect(&self) -> Result<WorkspaceInspection> {
        let _lock = self.lock()?;
        let state = self.read_state()?;
        let active = state.active()?;
        Ok(WorkspaceInspection {
            workspace_id: state.workspace_id.clone(),
            active_definition_hash: active.definition_hash().to_owned(),
            active_definition_revision: active.revision(),
            package_graph_hash: active.package_graph_hash().to_owned(),
            artifact_graph_hash: active.artifact_graph_hash().to_owned(),
            retained_definition_count: state.definitions.len(),
            committed_receipt_count: state.receipts.len(),
        })
    }

    pub fn active_definition(&self) -> Result<GameDefinition> {
        let _lock = self.lock()?;
        Ok(self.read_state()?.active()?.clone())
    }

    pub fn state_bytes(&self) -> Result<Vec<u8>> {
        let _lock = self.lock()?;
        fs::read(self.state_path()).map_err(|error| self.io_error("workspace.state", error))
    }

    pub fn validate(
        &self,
        catalog: &SemanticCatalog,
        change_set: ChangeSet,
    ) -> Result<ValidationReceipt> {
        let _lock = self.lock()?;
        let state = self.read_state()?;
        let (receipt, _) = validate_against(&state, catalog, change_set)?;
        Ok(receipt)
    }

    pub fn diff(
        &self,
        catalog: &SemanticCatalog,
        change_set: ChangeSet,
    ) -> Result<(ValidationReceipt, Vec<DefinitionDiff>)> {
        let _lock = self.lock()?;
        let state = self.read_state()?;
        let active = state.active()?;
        let (receipt, candidate) = validate_against(&state, catalog, change_set)?;
        let diffs = candidate.map_or_else(Vec::new, |candidate| {
            receipt
                .affected_ids
                .iter()
                .filter_map(|id| {
                    let id = SemanticId::parse(id).ok()?;
                    Some(DefinitionDiff {
                        id: id.as_str().to_owned(),
                        before: active.semantic_target_value(&id)?,
                        after: candidate.semantic_target_value(&id)?,
                    })
                })
                .collect()
        });
        Ok((receipt, diffs))
    }

    pub fn apply(
        &self,
        catalog: &SemanticCatalog,
        change_set: ChangeSet,
        validation_receipt_hash: &str,
        fault: PublicationFault,
    ) -> Result<ApplyReceipt> {
        let _lock = self.lock()?;
        let mut state = self.read_state()?;
        let change_set = change_set.canonicalized()?;
        let change_set_hash = change_set.canonical_hash();
        if let Some(existing) = state.receipts.get(&change_set.idempotency_key) {
            if existing.change_set_hash == change_set_hash {
                return Ok(existing.clone());
            }
            return Err(TransactionError::new(
                TransactionErrorCode::Conflict,
                change_set.idempotency_key,
                "idempotency key is already committed for a different change-set hash",
            ));
        }
        let (validation, candidate) = validate_against(&state, catalog, change_set.clone())?;
        if validation.receipt_hash != validation_receipt_hash {
            return Err(TransactionError::new(
                TransactionErrorCode::ValidationRejected,
                "validation_receipt_hash",
                "apply receipt does not match a fresh validation of this change set",
            ));
        }
        let candidate = candidate.ok_or_else(|| {
            TransactionError::new(
                TransactionErrorCode::ValidationRejected,
                "validation",
                "validation rejected the candidate definition",
            )
        })?;
        let prior = state.active()?.clone();
        let receipt = make_apply_receipt(
            "apply",
            &state.workspace_id,
            &change_set,
            &validation,
            &prior,
            &candidate,
            None,
        );
        state.active_definition_hash = candidate.definition_hash().to_owned();
        state
            .definitions
            .insert(candidate.definition_hash().to_owned(), candidate);
        state
            .receipts
            .insert(change_set.idempotency_key.clone(), receipt.clone());
        self.write_state(&state, fault)?;
        if fault == PublicationFault::PauseAfterCommit {
            std::thread::sleep(std::time::Duration::from_secs(30));
        }
        if fault == PublicationFault::AfterCommit {
            return Err(TransactionError::interrupted(&change_set.idempotency_key));
        }
        let readback = self.read_state().map_err(|error| {
            TransactionError::after_commit(
                change_set.idempotency_key.clone(),
                format!(
                    "publication committed but read-back failed; resolve with apply-status: {}",
                    error.message
                ),
            )
        })?;
        let committed = readback
            .receipts
            .get(&change_set.idempotency_key)
            .ok_or_else(|| {
                TransactionError::after_commit(
                    change_set.idempotency_key,
                    "committed receipt is absent on read-back",
                )
            })?;
        if committed != &receipt
            || readback.active_definition_hash != receipt.resulting_definition_hash
        {
            return Err(TransactionError::after_commit(
                "workspace.read_back",
                "publication read-back does not match committed receipt",
            ));
        }
        Ok(receipt)
    }

    pub fn apply_status(&self, idempotency_key: &str) -> Result<ApplyStatus> {
        validate_token(idempotency_key, "idempotency_key")?;
        let _lock = self.lock()?;
        let state = self.read_state()?;
        Ok(state
            .receipts
            .get(idempotency_key)
            .cloned()
            .map_or(ApplyStatus::NotStarted, ApplyStatus::Committed))
    }

    pub fn rollback(
        &self,
        expected_definition_hash: &str,
        expected_definition_revision: u64,
        source_transaction_id: &str,
        idempotency_key: &str,
        fault: PublicationFault,
    ) -> Result<ApplyReceipt> {
        validate_content_hash(expected_definition_hash, "expected_definition_hash")?;
        validate_token(source_transaction_id, "source_transaction_id")?;
        validate_token(idempotency_key, "idempotency_key")?;
        let _lock = self.lock()?;
        let mut state = self.read_state()?;
        let change_set_hash = content_hash(&json!({
            "operation": "rollback",
            "workspace": state.workspace_id,
            "expected_definition_hash": expected_definition_hash,
            "expected_definition_revision": expected_definition_revision,
            "source_transaction_id": source_transaction_id,
            "idempotency_key": idempotency_key,
        }));
        if let Some(existing) = state.receipts.get(idempotency_key) {
            if existing.operation == "rollback"
                && existing.rollback_of_transaction.as_deref() == Some(source_transaction_id)
                && existing.change_set_hash == change_set_hash
            {
                return Ok(existing.clone());
            }
            return Err(TransactionError::new(
                TransactionErrorCode::Conflict,
                idempotency_key,
                "idempotency key is already committed for a different transaction",
            ));
        }
        let active = state.active()?.clone();
        guard_definition(
            &active,
            expected_definition_hash,
            expected_definition_revision,
        )?;
        let source = state
            .receipts
            .values()
            .find(|receipt| receipt.transaction_id == source_transaction_id)
            .cloned()
            .ok_or_else(|| {
                TransactionError::new(
                    TransactionErrorCode::ReceiptNotFound,
                    source_transaction_id,
                    "rollback source transaction is not retained",
                )
            })?;
        let retained = state
            .definitions
            .get(&source.prior_definition_hash)
            .cloned()
            .ok_or_else(|| {
                TransactionError::new(
                    TransactionErrorCode::RetainedDefinitionMissing,
                    &source.prior_definition_hash,
                    "receipt-bound prior definition is not retained",
                )
            })?;
        let candidate = retained
            .reissue_at_revision(active.revision().saturating_add(1))
            .map_err(TransactionError::from)?;
        let validation_receipt_hash = content_hash(&json!({
            "accepted": true,
            "change_set_hash": change_set_hash,
            "candidate_definition_hash": candidate.definition_hash(),
        }));
        let receipt = make_rollback_receipt(
            &state.workspace_id,
            idempotency_key,
            &change_set_hash,
            &validation_receipt_hash,
            &active,
            &candidate,
            source_transaction_id,
            source.mutation_timing,
            source.applied_ids.clone(),
        );
        state.active_definition_hash = candidate.definition_hash().to_owned();
        state
            .definitions
            .insert(candidate.definition_hash().to_owned(), candidate);
        state
            .receipts
            .insert(idempotency_key.to_owned(), receipt.clone());
        self.write_state(&state, fault)?;
        if fault == PublicationFault::PauseAfterCommit {
            std::thread::sleep(std::time::Duration::from_secs(30));
        }
        if fault == PublicationFault::AfterCommit {
            return Err(TransactionError::interrupted(idempotency_key));
        }
        let readback = self.read_state().map_err(|error| {
            TransactionError::after_commit(
                idempotency_key,
                format!(
                    "rollback committed but read-back failed; resolve with apply-status: {}",
                    error.message
                ),
            )
        })?;
        if readback.active_definition_hash != receipt.resulting_definition_hash
            || readback.receipts.get(idempotency_key) != Some(&receipt)
        {
            return Err(TransactionError::after_commit(
                idempotency_key,
                "rollback read-back does not match committed receipt",
            ));
        }
        Ok(receipt)
    }

    fn state_path(&self) -> PathBuf {
        self.root.join("workspace-state.json")
    }

    fn lock(&self) -> Result<WorkspaceLock> {
        fs::create_dir_all(&self.root).map_err(|error| self.io_error("workspace", error))?;
        let path = self.root.join(".workspace-lock");
        reject_symlink(&path)?;
        let owner_identity = LockOwner::current();
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = self.root.join(format!(
            ".workspace-lock.{}.{}.{}.candidate",
            owner_identity.process_id,
            owner_identity.process_start_ticks.unwrap_or_default(),
            sequence
        ));
        let owner_bytes = serde_json::to_vec(&owner_identity).expect("lock owner serializes");
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&candidate)
            .and_then(|mut file| {
                file.write_all(&owner_bytes)?;
                file.sync_all()
            })
            .map_err(|error| self.io_error("workspace.lock_candidate", error))?;
        let mut acquired = false;
        for attempt in 0..500 {
            match fs::hard_link(&candidate, &path) {
                Ok(()) => {
                    acquired = true;
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if let Err(error) = reject_symlink(&path) {
                        let _ = fs::remove_file(&candidate);
                        return Err(error);
                    }
                    if attempt >= 50 && LockOwner::read(&path).is_none_or(|owner| !owner.is_live())
                    {
                        match fs::remove_file(&path) {
                            Ok(()) => {}
                            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                            Err(error) => {
                                let _ = fs::remove_file(&candidate);
                                return Err(self.io_error("workspace.stale_lock", error));
                            }
                        }
                        continue;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                Err(error) => {
                    let _ = fs::remove_file(&candidate);
                    return Err(self.io_error("workspace.lock", error));
                }
            }
        }
        let _ = fs::remove_file(&candidate);
        if !acquired {
            return Err(TransactionError::new(
                TransactionErrorCode::Conflict,
                path.display().to_string(),
                "another writer owns the workspace publication lock",
            ));
        }
        Ok(WorkspaceLock {
            path,
            owner: owner_identity,
        })
    }

    fn read_state(&self) -> Result<WorkspaceState> {
        let path = self.state_path();
        reject_symlink(&path)?;
        let mut bytes = Vec::new();
        File::open(&path)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    TransactionError::new(
                        TransactionErrorCode::WorkspaceMissing,
                        path.display().to_string(),
                        "workspace state does not exist",
                    )
                } else {
                    self.io_error("workspace.state", error)
                }
            })?
            .read_to_end(&mut bytes)
            .map_err(|error| self.io_error("workspace.state", error))?;
        let state: WorkspaceState = serde_json::from_slice(&bytes).map_err(|error| {
            TransactionError::new(
                TransactionErrorCode::WorkspaceCorrupt,
                path.display().to_string(),
                format!("workspace state is not canonical schema JSON: {error}"),
            )
        })?;
        state.verify()?;
        Ok(state)
    }

    fn write_state(&self, state: &WorkspaceState, fault: PublicationFault) -> Result<()> {
        state.verify()?;
        if fault == PublicationFault::BeforeWrite {
            return Err(TransactionError::new(
                TransactionErrorCode::Io,
                "workspace.publish.before_write",
                "injected failure before staged state write",
            ));
        }
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = self.root.join(format!(
            ".workspace-state.{}.{}.next",
            std::process::id(),
            sequence
        ));
        let result = (|| {
            let bytes = serde_json::to_vec(state).expect("workspace state is serializable");
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)
                .map_err(|error| self.io_error("workspace.stage", error))?;
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(|error| self.io_error("workspace.stage", error))?;
            if fault == PublicationFault::BeforeCommit {
                return Err(TransactionError::new(
                    TransactionErrorCode::Io,
                    "workspace.publish.before_commit",
                    "injected failure after durable staging and before commit",
                ));
            }
            if fault == PublicationFault::PauseBeforeCommit {
                std::thread::sleep(std::time::Duration::from_secs(30));
                return Err(TransactionError::new(
                    TransactionErrorCode::Io,
                    "workspace.publish.pause_before_commit",
                    "fault-injection pause completed before commit",
                ));
            }
            reject_symlink(&self.state_path())?;
            fs::rename(&temporary, self.state_path())
                .map_err(|error| self.io_error("workspace.commit", error))?;
            if fault == PublicationFault::AfterRename {
                return Err(TransactionError::after_commit(
                    "workspace.publish.after_rename",
                    "injected interruption after atomic rename and before parent-directory sync",
                ));
            }
            sync_directory(&self.root).map_err(|error| {
                TransactionError::after_commit(
                    "workspace.commit",
                    format!(
                        "atomic rename completed but parent-directory durability is uncertain: {error}"
                    ),
                )
            })?;
            Ok(())
        })();
        if temporary.exists() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    fn io_error(&self, path: &str, error: std::io::Error) -> TransactionError {
        TransactionError::new(
            TransactionErrorCode::Io,
            path,
            format!("workspace I/O failed: {error}"),
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct LockOwner {
    schema: String,
    process_id: u32,
    process_start_ticks: Option<u64>,
    boot_id: Option<String>,
}

impl LockOwner {
    fn current() -> Self {
        Self {
            schema: "carpet-flight-workspace-lock/v1".to_owned(),
            process_id: std::process::id(),
            process_start_ticks: process_start_ticks(std::process::id()),
            boot_id: boot_id(),
        }
    }

    fn read(path: &Path) -> Option<Self> {
        reject_symlink(path).ok()?;
        serde_json::from_slice(&fs::read(path).ok()?).ok()
    }

    fn is_live(&self) -> bool {
        if self.schema != "carpet-flight-workspace-lock/v1" {
            return false;
        }
        match (
            self.process_start_ticks,
            process_start_ticks(self.process_id),
            self.boot_id.as_deref(),
            boot_id().as_deref(),
        ) {
            (Some(expected_start), Some(actual_start), Some(expected_boot), Some(actual_boot)) => {
                expected_start == actual_start && expected_boot == actual_boot
            }
            _ => self.process_id == std::process::id(),
        }
    }
}

struct WorkspaceLock {
    path: PathBuf,
    owner: LockOwner,
}

impl Drop for WorkspaceLock {
    fn drop(&mut self) {
        if LockOwner::read(&self.path).as_ref() == Some(&self.owner) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn process_start_ticks(process_id: u32) -> Option<u64> {
    let stat = fs::read_to_string(format!("/proc/{process_id}/stat")).ok()?;
    let command_end = stat.rfind(')')?;
    stat.get(command_end.saturating_add(1)..)?
        .split_whitespace()
        .nth(19)?
        .parse()
        .ok()
}

fn boot_id() -> Option<String> {
    fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn validate_against(
    state: &WorkspaceState,
    catalog: &SemanticCatalog,
    change_set: ChangeSet,
) -> Result<(ValidationReceipt, Option<GameDefinition>)> {
    let change_set = change_set.canonicalized()?;
    if change_set.target_workspace != state.workspace_id {
        return Err(TransactionError::new(
            TransactionErrorCode::InvalidRequest,
            "target_workspace",
            "change set targets a different workspace",
        ));
    }
    let active = state.active()?;
    guard_definition(
        active,
        &change_set.expected_definition_hash,
        change_set.expected_definition_revision,
    )?;
    let mut diagnostics = Vec::new();
    let mut required_timing = None;
    let mut proposed = active.clone();
    let mut preliminarily_valid = BTreeSet::new();
    let candidate_revision = active.revision().saturating_add(1);
    let mut resolved_operations = Vec::with_capacity(change_set.operations.len());
    for operation in &change_set.operations {
        match operation.resolve(active) {
            Ok(resolved) => resolved_operations.push(resolved),
            Err(error) => diagnostics.push(error.into()),
        }
    }
    for resolved in &resolved_operations {
        match resolved {
            ResolvedOperation::CatalogReplacement {
                id,
                value,
                expected_value,
            } => {
                let descriptor = catalog.descriptor(id);
                if let Some(descriptor) = descriptor {
                    required_timing = Some(match (required_timing, descriptor.mutation_timing) {
                        (Some(RequestedTiming::PackageReload), _)
                        | (_, MutationTiming::PackageReload) => RequestedTiming::PackageReload,
                        _ => RequestedTiming::SessionReset,
                    });
                    if !change_set
                        .requested_timing
                        .permits(descriptor.mutation_timing)
                    {
                        diagnostics.push(Diagnostic {
                            code: "UNSAFE_TIMING".to_owned(),
                            path_or_id: id.as_str().to_owned(),
                            message: format!(
                                "requested {} is earlier than required {}",
                                change_set.requested_timing.as_str(),
                                descriptor.mutation_timing.as_str()
                            ),
                        });
                        continue;
                    }
                    if descriptor.access == Access::ReadOnly {
                        diagnostics.push(Diagnostic {
                            code: "READ_ONLY".to_owned(),
                            path_or_id: id.as_str().to_owned(),
                            message: "semantic descriptor is read-only at this engine revision"
                                .to_owned(),
                        });
                        continue;
                    }
                }
                if let Some(expected) = expected_value
                    && active.value(id) != Some(expected)
                {
                    diagnostics.push(Diagnostic {
                        code: "CONFLICT".to_owned(),
                        path_or_id: id.as_str().to_owned(),
                        message: "replacement expected value does not match the active definition"
                            .to_owned(),
                    });
                    continue;
                }
                if active.value(id) == Some(value) {
                    diagnostics.push(Diagnostic {
                        code: "INVALID_REQUEST".to_owned(),
                        path_or_id: id.as_str().to_owned(),
                        message: "replacement is identical to the active definition value"
                            .to_owned(),
                    });
                    continue;
                }
                match catalog.validate_replacement_shape(active, id, value) {
                    Ok(()) => {
                        if let SemanticValue::Reference(reference) = value {
                            let present = active.artifacts().iter().any(|artifact| {
                                reference
                                    .strip_prefix("sha256:")
                                    .is_some_and(|hash| hash == artifact.sha256)
                            });
                            if !present {
                                diagnostics.push(Diagnostic {
                                    code: "DEPENDENCY_FAILED".to_owned(),
                                    path_or_id: id.as_str().to_owned(),
                                    message: "replacement asset is absent from the retained artifact graph"
                                        .to_owned(),
                                });
                                continue;
                            }
                        }
                        match proposed.with_replacement(id, value.clone(), candidate_revision) {
                            Ok(updated) => {
                                proposed = updated;
                                preliminarily_valid.insert(id.clone());
                            }
                            Err(error) => diagnostics.push(error.into()),
                        }
                    }
                    Err(error) => diagnostics.push(error.into()),
                }
            }
            ResolvedOperation::EntityMove {
                id,
                entity,
                position,
                expected_position,
            } => {
                required_timing = Some(required_timing.unwrap_or(RequestedTiming::SessionReset));
                if !change_set
                    .requested_timing
                    .permits(MutationTiming::SessionReset)
                {
                    diagnostics.push(Diagnostic {
                        code: "UNSAFE_TIMING".to_owned(),
                        path_or_id: id.as_str().to_owned(),
                        message: "entity movement requires session_reset timing".to_owned(),
                    });
                    continue;
                }
                let current = active
                    .level_definition()
                    .and_then(|level| level.entity(entity))
                    .map(|candidate| candidate.position());
                if expected_position.is_some() && current != *expected_position {
                    diagnostics.push(Diagnostic {
                        code: "CONFLICT".to_owned(),
                        path_or_id: id.as_str().to_owned(),
                        message: "move expected position does not match the active definition"
                            .to_owned(),
                    });
                    continue;
                }
                if current == Some(*position) {
                    diagnostics.push(Diagnostic {
                        code: "INVALID_REQUEST".to_owned(),
                        path_or_id: id.as_str().to_owned(),
                        message: "entity already has the requested position".to_owned(),
                    });
                    continue;
                }
                match proposed.with_moved_level_entity(entity, *position, candidate_revision) {
                    Ok(updated) => {
                        proposed = updated;
                        preliminarily_valid.insert(id.clone());
                    }
                    Err(error) => diagnostics.push(error.into()),
                }
            }
        }
    }
    for resolved in &resolved_operations {
        if let ResolvedOperation::CatalogReplacement { id, value, .. } = resolved
            && preliminarily_valid.contains(id)
            && let Err(error) = catalog.validate_replacement(&proposed, id, value)
        {
            diagnostics.push(error.into());
        }
    }
    let mut candidate = active.clone();
    if diagnostics.is_empty() {
        for resolved in &resolved_operations {
            let applied = match resolved {
                ResolvedOperation::CatalogReplacement { id, value, .. } => {
                    catalog.apply_replacement(&candidate, id, value, candidate_revision)
                }
                ResolvedOperation::EntityMove {
                    entity, position, ..
                } => candidate.with_moved_level_entity(entity, *position, candidate_revision),
            };
            match applied {
                Ok(updated) => candidate = updated,
                Err(error) => diagnostics.push(error.into()),
            }
        }
    }
    let accepted = diagnostics.is_empty();
    let change_set_hash = change_set.canonical_hash();
    let affected_ids = change_set
        .operations
        .iter()
        .map(|operation| operation.target_id().as_str().to_owned())
        .collect::<Vec<_>>();
    let mut receipt = ValidationReceipt {
        schema: VALIDATION_RECEIPT_SCHEMA.to_owned(),
        receipt_hash: String::new(),
        change_set_hash,
        target_workspace: state.workspace_id.clone(),
        expected_definition_hash: active.definition_hash().to_owned(),
        expected_definition_revision: active.revision(),
        candidate_definition_hash: accepted.then(|| candidate.definition_hash().to_owned()),
        candidate_definition_revision: accepted.then_some(candidate.revision()),
        affected_ids,
        required_timing,
        accepted,
        diagnostics,
        validator_identity: "sim-core/semantic-transaction-v1".to_owned(),
    };
    receipt.receipt_hash = validation_receipt_hash(&receipt);
    Ok((receipt, accepted.then_some(candidate)))
}

fn guard_definition(
    active: &GameDefinition,
    expected_hash: &str,
    expected_revision: u64,
) -> Result<()> {
    if active.definition_hash() != expected_hash || active.revision() != expected_revision {
        return Err(TransactionError::new(
            TransactionErrorCode::DefinitionMismatch,
            "expected_definition",
            format!(
                "expected revision {expected_revision} hash {expected_hash}; active revision {} hash {}",
                active.revision(),
                active.definition_hash()
            ),
        ));
    }
    Ok(())
}

fn make_apply_receipt(
    operation: &str,
    workspace_id: &str,
    change_set: &ChangeSet,
    validation: &ValidationReceipt,
    prior: &GameDefinition,
    candidate: &GameDefinition,
    rollback_of_transaction: Option<String>,
) -> ApplyReceipt {
    let applied_ids = change_set
        .operations
        .iter()
        .map(|operation| operation.target_id().as_str().to_owned())
        .collect::<Vec<_>>();
    let read_back_hash = read_back_hash(candidate, &applied_ids);
    make_receipt(
        operation,
        workspace_id,
        &change_set.idempotency_key,
        &validation.change_set_hash,
        &validation.receipt_hash,
        prior,
        candidate,
        change_set.requested_timing,
        applied_ids,
        read_back_hash,
        rollback_of_transaction,
    )
}

fn make_rollback_receipt(
    workspace_id: &str,
    idempotency_key: &str,
    change_set_hash: &str,
    validation_receipt_hash: &str,
    prior: &GameDefinition,
    candidate: &GameDefinition,
    rollback_of_transaction: &str,
    mutation_timing: RequestedTiming,
    applied_ids: Vec<String>,
) -> ApplyReceipt {
    let read_back_hash = read_back_hash(candidate, &applied_ids);
    make_receipt(
        "rollback",
        workspace_id,
        idempotency_key,
        change_set_hash,
        validation_receipt_hash,
        prior,
        candidate,
        mutation_timing,
        applied_ids,
        read_back_hash,
        Some(rollback_of_transaction.to_owned()),
    )
}

#[allow(clippy::too_many_arguments)]
fn make_receipt(
    operation: &str,
    workspace_id: &str,
    idempotency_key: &str,
    change_set_hash: &str,
    validation_receipt_hash: &str,
    prior: &GameDefinition,
    candidate: &GameDefinition,
    mutation_timing: RequestedTiming,
    applied_ids: Vec<String>,
    read_back_hash: String,
    rollback_of_transaction: Option<String>,
) -> ApplyReceipt {
    let transaction_id = format!(
        "tx-{}",
        &content_hash(&json!({
            "workspace": workspace_id,
            "idempotency_key": idempotency_key,
            "change_set_hash": change_set_hash,
            "result": candidate.definition_hash(),
        }))[7..23]
    );
    let mut receipt = ApplyReceipt {
        schema: APPLY_RECEIPT_SCHEMA.to_owned(),
        receipt_hash: String::new(),
        transaction_id,
        operation: operation.to_owned(),
        workspace_id: workspace_id.to_owned(),
        idempotency_key: idempotency_key.to_owned(),
        change_set_hash: change_set_hash.to_owned(),
        validation_receipt_hash: validation_receipt_hash.to_owned(),
        prior_definition_hash: prior.definition_hash().to_owned(),
        prior_definition_revision: prior.revision(),
        resulting_definition_hash: candidate.definition_hash().to_owned(),
        resulting_definition_revision: candidate.revision(),
        prior_package_graph_hash: prior.package_graph_hash().to_owned(),
        resulting_package_graph_hash: candidate.package_graph_hash().to_owned(),
        prior_artifact_graph_hash: prior.artifact_graph_hash().to_owned(),
        resulting_artifact_graph_hash: candidate.artifact_graph_hash().to_owned(),
        commit_point: "workspace-state.json:atomic-rename-v1".to_owned(),
        mutation_timing,
        affected_existing_sessions: Vec::new(),
        applied_ids,
        read_back_hash,
        rollback_of_transaction,
    };
    receipt.receipt_hash = receipt_hash(&receipt);
    receipt
}

fn validation_receipt_hash(receipt: &ValidationReceipt) -> String {
    let mut canonical = receipt.clone();
    canonical.receipt_hash.clear();
    content_hash(&canonical)
}

fn receipt_hash(receipt: &ApplyReceipt) -> String {
    let mut canonical = receipt.clone();
    canonical.receipt_hash.clear();
    content_hash(&canonical)
}

fn read_back_hash(definition: &GameDefinition, ids: &[String]) -> String {
    let values = ids
        .iter()
        .filter_map(|id| SemanticId::parse(id).ok())
        .filter_map(|id| {
            definition
                .semantic_target_value(&id)
                .map(|value| json!({"id": id.as_str(), "value": value.to_json()}))
        })
        .collect::<Vec<_>>();
    content_hash(&json!({
        "definition_hash": definition.definition_hash(),
        "definition_revision": definition.revision(),
        "values": values,
    }))
}

fn content_hash(value: &impl Serialize) -> String {
    format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("canonical transaction value serializes"))
    )
}

fn validate_token(value: &str, path: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
    {
        return Err(TransactionError::new(
            TransactionErrorCode::InvalidRequest,
            path,
            "identifier must be 1..128 ASCII letters, digits, dot, underscore, colon, or hyphen",
        ));
    }
    Ok(())
}

fn validate_content_hash(value: &str, path: &str) -> Result<()> {
    let valid = value.strip_prefix("sha256:").is_some_and(|hash| {
        hash.len() == 64
            && hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    });
    if !valid {
        return Err(TransactionError::new(
            TransactionErrorCode::InvalidRequest,
            path,
            "identity must be sha256:<64 lowercase hexadecimal digits>",
        ));
    }
    Ok(())
}

fn reject_symlink(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(TransactionError::new(
            TransactionErrorCode::Io,
            path.display().to_string(),
            "workspace state and lock paths must not be symlinks",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(TransactionError::new(
            TransactionErrorCode::Io,
            path.display().to_string(),
            format!("cannot inspect workspace path: {error}"),
        )),
    }
}

fn sync_directory(path: &Path) -> std::io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{
        build_level_definition, entity_definition_with_position_access, stable_entity_id,
        with_level_definition,
    };
    use crate::level_definition::{
        LevelEntityKind, LevelEntityPositionAccess, SceneryKind, SwitchKind, TilePosition,
    };
    use crate::{
        GamePackage, Result as DefinitionResult, SemanticBinding, default_catalog,
        engine_capabilities,
    };

    fn apply_value(
        definition: &GameDefinition,
        id: &SemanticId,
        value: &SemanticValue,
        revision: u64,
    ) -> DefinitionResult<GameDefinition> {
        definition.with_replacement(id, value.clone(), revision)
    }

    fn validate_even(
        _: &GameDefinition,
        _: &SemanticId,
        value: &SemanticValue,
    ) -> DefinitionResult<()> {
        match value {
            SemanticValue::Integer(value) if value.trailing_zeros() > 0 => Ok(()),
            _ => Err(crate::DefinitionError::new(
                crate::DefinitionErrorCode::OutOfRange,
                "flight.player.max_forward_speed",
                "conformance validator requires an even value",
            )),
        }
    }

    fn validate_any(_: &GameDefinition, _: &SemanticId, _: &SemanticValue) -> DefinitionResult<()> {
        Ok(())
    }

    fn read_value(definition: &GameDefinition, id: &SemanticId) -> DefinitionResult<SemanticValue> {
        definition.value(id).cloned().ok_or_else(|| {
            crate::DefinitionError::new(
                crate::DefinitionErrorCode::UnknownSemanticId,
                id.as_str(),
                "missing value",
            )
        })
    }

    fn writable_catalog() -> SemanticCatalog {
        let base = default_catalog().expect("base catalog");
        let mut descriptor = base
            .descriptors()
            .find(|row| row.id.as_str() == "flight.player.max_forward_speed")
            .expect("flight descriptor")
            .clone();
        descriptor.access = Access::Writable;
        SemanticCatalog::try_from_parts(
            vec![descriptor],
            vec![SemanticBinding {
                id: SemanticId::parse("flight.player.max_forward_speed").unwrap(),
                read: Some(read_value),
                validate: Some(validate_even),
                apply: Some(apply_value),
                read_back: Some(read_value),
            }],
        )
        .expect("writable conformance catalog")
    }

    fn writable_asset_catalog() -> SemanticCatalog {
        let base = default_catalog().expect("base catalog");
        let mut descriptor = base
            .descriptors()
            .find(|row| row.id.as_str() == "presentation.spell.fireball.projectile_asset")
            .expect("asset descriptor")
            .clone();
        descriptor.access = Access::Writable;
        let id = descriptor.id.clone();
        SemanticCatalog::try_from_parts(
            vec![descriptor],
            vec![SemanticBinding {
                id,
                read: Some(read_value),
                validate: Some(validate_any),
                apply: Some(apply_value),
                read_back: Some(read_value),
            }],
        )
        .expect("writable asset conformance catalog")
    }

    fn validate_flight_below_mana(
        definition: &GameDefinition,
        id: &SemanticId,
        value: &SemanticValue,
    ) -> DefinitionResult<()> {
        let mana_id = SemanticId::parse("spell.fireball.mana_cost")?;
        match (value, definition.value(&mana_id)) {
            (SemanticValue::Integer(speed), Some(SemanticValue::Integer(mana)))
                if speed <= mana =>
            {
                Ok(())
            }
            _ => Err(crate::DefinitionError::new(
                crate::DefinitionErrorCode::DependencyFailed,
                id.as_str(),
                "conformance cross-field rule requires speed not to exceed mana",
            )),
        }
    }

    fn cross_field_catalog() -> SemanticCatalog {
        let base = default_catalog().expect("base catalog");
        let mut flight = base
            .descriptors()
            .find(|row| row.id.as_str() == "flight.player.max_forward_speed")
            .unwrap()
            .clone();
        flight.access = Access::Writable;
        flight.dependencies = vec!["spell.fireball.mana_cost".to_owned()];
        let mut mana = base
            .descriptors()
            .find(|row| row.id.as_str() == "spell.fireball.mana_cost")
            .unwrap()
            .clone();
        mana.access = Access::ReadOnly;
        SemanticCatalog::try_from_parts(
            vec![flight.clone(), mana.clone()],
            vec![
                SemanticBinding {
                    id: flight.id,
                    read: Some(read_value),
                    validate: Some(validate_flight_below_mana),
                    apply: Some(apply_value),
                    read_back: Some(read_value),
                },
                SemanticBinding {
                    id: mana.id,
                    read: Some(read_value),
                    validate: None,
                    apply: None,
                    read_back: None,
                },
            ],
        )
        .expect("cross-field conformance catalog")
    }

    fn fixture_definition() -> GameDefinition {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../sim-core/tests/fixtures/custom-synthetic-v1");
        GameDefinition::from_package(
            GamePackage::load(root).expect("load fixture"),
            &engine_capabilities().expect("capabilities"),
            &default_catalog().expect("catalog"),
        )
        .expect("definition")
    }

    fn fixture_definition_with_movable_entity() -> GameDefinition {
        let movable = entity_definition_with_position_access(
            stable_entity_id("switch.start_area_exit").unwrap(),
            LevelEntityKind::Switch(SwitchKind::AreaExitTrigger),
            TilePosition { x: 74, y: 212 },
            None,
            LevelEntityPositionAccess::SessionReset,
        );
        let read_only = entity_definition_with_position_access(
            stable_entity_id("scenery.old_oak").unwrap(),
            LevelEntityKind::Scenery(SceneryKind::Tree),
            TilePosition { x: 80, y: 210 },
            None,
            LevelEntityPositionAccess::ReadOnly,
        );
        with_level_definition(
            fixture_definition(),
            build_level_definition(vec![movable, read_only], Vec::new(), Vec::new()).unwrap(),
        )
    }

    fn temporary_workspace(label: &str) -> PathBuf {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "carpet-flight-f3-{label}-{}-{sequence}",
            std::process::id()
        ))
    }

    fn change(definition: &GameDefinition, key: &str, value: i64) -> ChangeSet {
        ChangeSetBuilder::new(
            "custom-workspace",
            definition,
            key,
            RequestedTiming::SessionReset,
        )
        .set_integer(IntegerParameter::PlayerMaxForwardSpeed, value)
        .build()
        .expect("change set")
    }

    #[test]
    fn move_entity_publication_diff_readback_and_rollback_are_typed_and_atomic() {
        let root = temporary_workspace("move-entity");
        let definition = fixture_definition_with_movable_entity();
        let store = WorkspaceStore::create(&root, "move-workspace", definition.clone()).unwrap();
        let catalog = default_catalog().unwrap();
        let entity = stable_entity_id("switch.start_area_exit").unwrap();
        let target_id = SemanticId::parse("switch.start_area_exit.position").unwrap();
        let original = TilePosition { x: 74, y: 212 };
        let moved = TilePosition { x: 150, y: 100 };
        let base_descriptor = definition
            .level_entity_position_descriptor(&target_id)
            .unwrap();
        assert_eq!(base_descriptor.kind, crate::DescriptorKind::Placement);
        assert_eq!(base_descriptor.access, Access::Writable);
        assert_eq!(
            base_descriptor.default_value,
            SemanticValue::Position(original)
        );
        assert_eq!(
            definition.semantic_target_value(&target_id),
            Some(SemanticValue::Position(original))
        );
        let request = ChangeSetBuilder::new(
            "move-workspace",
            &definition,
            "move-start-trigger",
            RequestedTiming::SessionReset,
        )
        .move_entity_from(entity.clone(), original, moved)
        .build()
        .unwrap();
        let before = store.state_bytes().unwrap();
        let validation = store.validate(&catalog, request.clone()).unwrap();
        assert!(validation.accepted, "{:?}", validation.diagnostics);
        assert_eq!(
            validation.affected_ids,
            vec!["switch.start_area_exit.position"]
        );
        let (_, diff) = store.diff(&catalog, request.clone()).unwrap();
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].before, SemanticValue::Position(original));
        assert_eq!(diff[0].after, SemanticValue::Position(moved));
        assert_eq!(store.state_bytes().unwrap(), before);

        let applied = store
            .apply(
                &catalog,
                request,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        assert_eq!(applied.applied_ids, vec!["switch.start_area_exit.position"]);
        let active = store.active_definition().unwrap();
        assert_eq!(
            active
                .level_definition()
                .unwrap()
                .entity(&entity)
                .unwrap()
                .position(),
            moved
        );
        assert_ne!(active.definition_hash(), definition.definition_hash());
        let moved_descriptor = active.level_entity_position_descriptor(&target_id).unwrap();
        assert_eq!(
            moved_descriptor.default_value,
            SemanticValue::Position(original)
        );
        assert_eq!(
            active.semantic_target_value(&target_id),
            Some(SemanticValue::Position(moved))
        );

        let rolled = store
            .rollback(
                active.definition_hash(),
                active.revision(),
                &applied.transaction_id,
                "rollback-start-trigger",
                PublicationFault::None,
            )
            .unwrap();
        assert_eq!(rolled.resulting_definition_revision, 3);
        assert_eq!(
            store
                .active_definition()
                .unwrap()
                .level_definition()
                .unwrap()
                .entity(&entity)
                .unwrap()
                .position(),
            original
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn mixed_parameter_and_entity_move_publish_atomically_or_not_at_all() {
        let root = temporary_workspace("mixed-entity-move");
        let definition = fixture_definition_with_movable_entity();
        let store = WorkspaceStore::create(&root, "mixed-workspace", definition.clone()).unwrap();
        let catalog = default_catalog().unwrap();
        let entity = stable_entity_id("switch.start_area_exit").unwrap();
        let original = TilePosition { x: 74, y: 212 };
        let moved = TilePosition { x: 150, y: 100 };
        let before = store.state_bytes().unwrap();

        let invalid = ChangeSetBuilder::new(
            "mixed-workspace",
            &definition,
            "mixed-invalid",
            RequestedTiming::SessionReset,
        )
        .set_integer(IntegerParameter::PlayerMaxForwardSpeed, 20)
        .move_entity_from(entity.clone(), original, moved)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, invalid).unwrap();
        assert!(!validation.accepted);
        assert_eq!(store.state_bytes().unwrap(), before);

        let request = ChangeSetBuilder::new(
            "mixed-workspace",
            &definition,
            "mixed-valid",
            RequestedTiming::SessionReset,
        )
        .set_integer(IntegerParameter::PlayerMaxForwardSpeed, 96)
        .move_entity_from(entity.clone(), original, moved)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, request.clone()).unwrap();
        assert!(validation.accepted, "{:?}", validation.diagnostics);
        assert_eq!(
            validation.affected_ids,
            [
                "flight.player.max_forward_speed",
                "switch.start_area_exit.position",
            ]
        );
        let (_, diff) = store.diff(&catalog, request.clone()).unwrap();
        assert_eq!(diff.len(), 2);
        assert_eq!(
            diff.iter()
                .map(|entry| (entry.id.as_str(), &entry.before, &entry.after))
                .collect::<Vec<_>>(),
            vec![
                (
                    "flight.player.max_forward_speed",
                    &SemanticValue::Integer(80),
                    &SemanticValue::Integer(96),
                ),
                (
                    "switch.start_area_exit.position",
                    &SemanticValue::Position(original),
                    &SemanticValue::Position(moved),
                ),
            ]
        );
        assert_eq!(store.state_bytes().unwrap(), before);

        let applied = store
            .apply(
                &catalog,
                request,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        assert_eq!(applied.applied_ids, validation.affected_ids);
        let active = store.active_definition().unwrap();
        assert_eq!(
            active.semantic_target_value(
                &SemanticId::parse("flight.player.max_forward_speed").unwrap()
            ),
            Some(SemanticValue::Integer(96))
        );
        assert_eq!(
            active
                .level_definition()
                .unwrap()
                .entity(&entity)
                .unwrap()
                .position(),
            moved
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn move_entity_rejections_preserve_workspace_and_raw_identity_is_unrepresentable() {
        let root = temporary_workspace("move-entity-rejections");
        let definition = fixture_definition_with_movable_entity();
        let store = WorkspaceStore::create(&root, "move-workspace", definition.clone()).unwrap();
        let catalog = default_catalog().unwrap();
        let before = store.state_bytes().unwrap();
        let movable = stable_entity_id("switch.start_area_exit").unwrap();

        let rejected = [
            DefinitionOperation::MoveEntity {
                entity: stable_entity_id("scenery.old_oak").unwrap(),
                position: TilePosition { x: 81, y: 210 },
                expected_position: None,
            },
            DefinitionOperation::MoveEntity {
                entity: stable_entity_id("switch.unknown_trigger").unwrap(),
                position: TilePosition { x: 150, y: 100 },
                expected_position: None,
            },
            DefinitionOperation::MoveEntity {
                entity: movable.clone(),
                position: TilePosition { x: 150, y: 100 },
                expected_position: Some(TilePosition { x: 75, y: 212 }),
            },
            DefinitionOperation::MoveEntity {
                entity: movable.clone(),
                position: TilePosition { x: 256, y: 100 },
                expected_position: None,
            },
            DefinitionOperation::MoveEntity {
                entity: movable.clone(),
                position: TilePosition { x: 150, y: 100 },
                expected_position: Some(TilePosition { x: 256, y: 100 }),
            },
        ];
        for (index, operation) in rejected.into_iter().enumerate() {
            let request = ChangeSetBuilder::new(
                "move-workspace",
                &definition,
                format!("rejected-move-{index}"),
                RequestedTiming::SessionReset,
            )
            .operation(operation)
            .build()
            .unwrap();
            let validation = store.validate(&catalog, request).unwrap();
            assert!(!validation.accepted);
            assert_eq!(store.state_bytes().unwrap(), before);
        }

        let duplicate = ChangeSetBuilder::new(
            "move-workspace",
            &definition,
            "duplicate-move",
            RequestedTiming::SessionReset,
        )
        .move_entity(movable.clone(), TilePosition { x: 150, y: 100 })
        .move_entity(movable, TilePosition { x: 151, y: 100 })
        .build()
        .expect_err("duplicate semantic target must reject");
        assert_eq!(duplicate.code, TransactionErrorCode::InvalidRequest);

        for raw_identity in [
            "action17.firefly",
            "creature.class5_model3_action17",
            "switch.source48",
            "switch.raw48",
            "switch.level_1",
        ] {
            assert!(stable_entity_id(raw_identity).is_err());
        }
        let raw_field = serde_json::json!({
            "operation": "move_entity",
            "entity": "switch.start_area_exit",
            "position": {"x": 150, "y": 100},
            "source_slot": 48
        });
        assert!(serde_json::from_value::<DefinitionOperation>(raw_field).is_err());
        for nested_raw_field in [
            serde_json::json!({
                "operation": "move_entity",
                "entity": "switch.start_area_exit",
                "position": {"x": 150, "y": 100, "source_slot": 48}
            }),
            serde_json::json!({
                "operation": "move_entity",
                "entity": "switch.start_area_exit",
                "position": {"x": 150, "y": 100},
                "expected_position": {"x": 74, "y": 212, "raw": 48}
            }),
        ] {
            assert!(serde_json::from_value::<DefinitionOperation>(nested_raw_field).is_err());
        }
        assert_eq!(store.state_bytes().unwrap(), before);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn validate_diff_and_precommit_faults_preserve_every_durable_root() {
        let root = temporary_workspace("preserve");
        let definition = fixture_definition();
        let store = WorkspaceStore::create(&root, "custom-workspace", definition.clone()).unwrap();
        let catalog = writable_catalog();
        let request = change(&definition, "apply-one", 82);
        let before = store.state_bytes().unwrap();
        let validation = store.validate(&catalog, request.clone()).unwrap();
        assert!(validation.accepted);
        let (_, diff) = store.diff(&catalog, request.clone()).unwrap();
        assert_eq!(diff.len(), 1);
        assert_eq!(store.state_bytes().unwrap(), before);
        for fault in [
            PublicationFault::BeforeWrite,
            PublicationFault::BeforeCommit,
        ] {
            let error = store
                .apply(&catalog, request.clone(), &validation.receipt_hash, fault)
                .expect_err("fault must reject");
            assert!(!error.state_changed);
            assert_eq!(store.state_bytes().unwrap(), before);
            assert_eq!(
                store.apply_status("apply-one").unwrap(),
                ApplyStatus::NotStarted
            );
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn apply_is_atomic_idempotent_recoverable_and_guarded_by_definition_identity() {
        let root = temporary_workspace("apply");
        let definition = fixture_definition();
        let store = WorkspaceStore::create(&root, "custom-workspace", definition.clone()).unwrap();
        let catalog = writable_catalog();
        let request = change(&definition, "apply-one", 82);
        let validation = store.validate(&catalog, request.clone()).unwrap();
        let interrupted = store
            .apply(
                &catalog,
                request.clone(),
                &validation.receipt_hash,
                PublicationFault::AfterCommit,
            )
            .expect_err("post-commit interruption");
        assert_eq!(
            interrupted.code,
            TransactionErrorCode::InterruptedAfterCommit
        );
        assert!(interrupted.state_changed);
        let status = store.apply_status("apply-one").unwrap();
        assert!(matches!(status, ApplyStatus::Committed(_)));
        let ApplyStatus::Committed(receipt) = status else {
            return;
        };
        assert_eq!(receipt.resulting_definition_revision, 2);
        let retry = store
            .apply(
                &catalog,
                request.clone(),
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        assert_eq!(retry, receipt);

        let mut reused = request.clone();
        let DefinitionOperation::SetInteger { value, .. } = &mut reused.operations[0] else {
            panic!("test request remains an integer operation");
        };
        *value = 84;
        let error = store
            .apply(
                &catalog,
                reused,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .expect_err("key reuse must conflict");
        assert_eq!(error.code, TransactionErrorCode::Conflict);

        let stale = change(&definition, "stale", 86);
        let error = store
            .validate(&catalog, stale)
            .expect_err("stale CAS must fail");
        assert_eq!(error.code, TransactionErrorCode::DefinitionMismatch);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn interruptions_after_rename_and_after_durable_commit_recover_the_exact_receipt() {
        for fault in [PublicationFault::AfterRename, PublicationFault::AfterCommit] {
            let root = temporary_workspace(match fault {
                PublicationFault::AfterRename => "after-rename",
                PublicationFault::AfterCommit => "after-commit",
                _ => unreachable!("test enumerates only post-publication faults"),
            });
            let definition = fixture_definition();
            let store =
                WorkspaceStore::create(&root, "custom-workspace", definition.clone()).unwrap();
            let catalog = writable_catalog();
            let request = change(&definition, "recover-key", 82);
            let validation = store.validate(&catalog, request.clone()).unwrap();
            let interrupted = store
                .apply(&catalog, request, &validation.receipt_hash, fault)
                .expect_err("post-publication interruption must require status recovery");
            assert_eq!(
                interrupted.code,
                TransactionErrorCode::InterruptedAfterCommit
            );
            assert!(interrupted.state_changed);
            let status = store.apply_status("recover-key").unwrap();
            assert!(matches!(status, ApplyStatus::Committed(_)));
            let ApplyStatus::Committed(receipt) = status else {
                return;
            };
            assert_eq!(receipt.resulting_definition_revision, 2);
            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    fn apply_status_reclaims_a_crashed_writer_lock_by_process_identity() {
        let root = temporary_workspace("stale-lock");
        let definition = fixture_definition();
        let store = WorkspaceStore::create(&root, "custom-workspace", definition).unwrap();
        let before = store.state_bytes().unwrap();
        fs::write(root.join(".workspace-state.crashed.next"), b"partial-stage").unwrap();
        let stale_lock = root.join(".workspace-lock");
        let stale_owner = LockOwner {
            schema: "carpet-flight-workspace-lock/v1".to_owned(),
            process_id: u32::MAX,
            process_start_ticks: Some(1),
            boot_id: boot_id(),
        };
        fs::write(&stale_lock, serde_json::to_vec(&stale_owner).unwrap()).unwrap();
        assert_eq!(
            store.apply_status("never-started").unwrap(),
            ApplyStatus::NotStarted
        );
        assert_eq!(store.state_bytes().unwrap(), before);
        assert!(!stale_lock.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn concurrent_writers_serialize_and_the_loser_observes_a_stale_definition_guard() {
        let root = temporary_workspace("concurrent");
        let definition = fixture_definition();
        let store = WorkspaceStore::create(&root, "custom-workspace", definition.clone()).unwrap();
        let catalog = writable_catalog();
        let first = change(&definition, "concurrent-first", 82);
        let second = change(&definition, "concurrent-second", 84);
        let first_validation = store.validate(&catalog, first.clone()).unwrap();
        let second_validation = store.validate(&catalog, second.clone()).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let first_handle = {
            let barrier = barrier.clone();
            let root = root.clone();
            std::thread::spawn(move || {
                barrier.wait();
                WorkspaceStore::open(root).apply(
                    &writable_catalog(),
                    first,
                    &first_validation.receipt_hash,
                    PublicationFault::None,
                )
            })
        };
        let second_handle = {
            let barrier = barrier.clone();
            let root = root.clone();
            std::thread::spawn(move || {
                barrier.wait();
                WorkspaceStore::open(root).apply(
                    &writable_catalog(),
                    second,
                    &second_validation.receipt_hash,
                    PublicationFault::None,
                )
            })
        };
        barrier.wait();
        let results = [first_handle.join().unwrap(), second_handle.join().unwrap()];
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        let loser = results
            .iter()
            .find_map(|result| result.as_ref().err())
            .expect("one stale writer");
        assert_eq!(loser.code, TransactionErrorCode::DefinitionMismatch);
        assert_eq!(store.inspect().unwrap().committed_receipt_count, 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rollback_is_a_guarded_forward_publication_to_a_receipt_bound_graph() {
        let root = temporary_workspace("rollback");
        let definition = fixture_definition();
        let store = WorkspaceStore::create(&root, "custom-workspace", definition.clone()).unwrap();
        let catalog = writable_catalog();
        let request = change(&definition, "apply-one", 82);
        let validation = store.validate(&catalog, request.clone()).unwrap();
        let applied = store
            .apply(
                &catalog,
                request,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let rolled = store
            .rollback(
                &applied.resulting_definition_hash,
                applied.resulting_definition_revision,
                &applied.transaction_id,
                "rollback-one",
                PublicationFault::None,
            )
            .unwrap();
        assert_eq!(rolled.resulting_definition_revision, 3);
        let active = store.active_definition().unwrap();
        assert_eq!(
            active.value(&SemanticId::parse("flight.player.max_forward_speed").unwrap()),
            Some(&SemanticValue::Integer(80))
        );
        assert_ne!(active.definition_hash(), definition.definition_hash());
        let error = store
            .rollback(
                &applied.resulting_definition_hash,
                applied.resulting_definition_revision,
                &applied.transaction_id,
                "rollback-stale",
                PublicationFault::None,
            )
            .expect_err("stale rollback guard must fail");
        assert_eq!(error.code, TransactionErrorCode::DefinitionMismatch);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rollback_refuses_a_missing_retained_graph_and_persisted_artifact_tampering_fails_closed() {
        let root = temporary_workspace("retention");
        let definition = fixture_definition();
        let store = WorkspaceStore::create(&root, "custom-workspace", definition.clone()).unwrap();
        let catalog = writable_catalog();
        let request = change(&definition, "retained-apply", 82);
        let validation = store.validate(&catalog, request.clone()).unwrap();
        let applied = store
            .apply(
                &catalog,
                request,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        {
            let _lock = store.lock().unwrap();
            let mut state = store.read_state().unwrap();
            state.definitions.remove(&applied.prior_definition_hash);
            store.write_state(&state, PublicationFault::None).unwrap();
        }
        let before = store.state_bytes().unwrap();
        let error = store
            .rollback(
                &applied.resulting_definition_hash,
                applied.resulting_definition_revision,
                &applied.transaction_id,
                "missing-retained",
                PublicationFault::None,
            )
            .expect_err("missing receipt-bound graph must fail");
        assert_eq!(error.code, TransactionErrorCode::RetainedDefinitionMissing);
        assert_eq!(store.state_bytes().unwrap(), before);

        let mut state: serde_json::Value = serde_json::from_slice(&before).unwrap();
        let active_hash = state["active_definition_hash"].as_str().unwrap().to_owned();
        state["definitions"][&active_hash]["artifacts"][0]["bytes"] = json!(999_999_u64);
        fs::write(
            store.state_path(),
            serde_json::to_vec(&state).expect("serialize tampered state"),
        )
        .unwrap();
        let error = store
            .inspect()
            .expect_err("artifact tampering must fail closed");
        assert_eq!(error.code, TransactionErrorCode::WorkspaceCorrupt);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejected_values_unsafe_timing_partial_assets_and_unknown_fields_do_not_mutate() {
        let root = temporary_workspace("reject");
        let definition = fixture_definition();
        let store = WorkspaceStore::create(&root, "custom-workspace", definition.clone()).unwrap();
        let catalog = writable_catalog();
        let before = store.state_bytes().unwrap();
        let odd = change(&definition, "odd", 81);
        let rejected = store.validate(&catalog, odd).unwrap();
        assert!(!rejected.accepted);
        assert_eq!(rejected.diagnostics[0].code, "OUT_OF_RANGE");
        assert_eq!(store.state_bytes().unwrap(), before);

        let cross_field = change(&definition, "cross-field", 120);
        let rejected = store.validate(&cross_field_catalog(), cross_field).unwrap();
        assert!(!rejected.accepted);
        assert_eq!(rejected.diagnostics[0].code, "DEPENDENCY_FAILED");
        assert_eq!(store.state_bytes().unwrap(), before);

        let partial_asset = ChangeSetBuilder::new(
            "custom-workspace",
            &definition,
            "partial-asset",
            RequestedTiming::PackageReload,
        )
        .bind_asset(
            PresentationAssetSlot::FireballProjectile,
            ArtifactId::parse("presentation.missing_asset").unwrap(),
        )
        .build()
        .unwrap();
        let rejected = store
            .validate(&writable_asset_catalog(), partial_asset)
            .unwrap();
        assert!(!rejected.accepted);
        assert_eq!(rejected.diagnostics[0].code, "DEPENDENCY_FAILED");
        assert_eq!(store.state_bytes().unwrap(), before);

        let unsafe_timing = ChangeSetBuilder::new(
            "custom-workspace",
            &definition,
            "unsafe-asset-timing",
            RequestedTiming::SessionReset,
        )
        .bind_asset(
            PresentationAssetSlot::FireballProjectile,
            ArtifactId::parse("presentation.projectile_asset_variant").unwrap(),
        )
        .build()
        .unwrap();
        let rejected = store
            .validate(&default_catalog().unwrap(), unsafe_timing)
            .unwrap();
        assert_eq!(rejected.diagnostics[0].code, "UNSAFE_TIMING");
        assert_eq!(store.state_bytes().unwrap(), before);

        for (idempotency_key, value, expected_code) in
            [("unknown-goat-policy", "berserk_goat_v1", "OUT_OF_RANGE")]
        {
            let invalid_policy = ChangeSetBuilder::new(
                "custom-workspace",
                &definition,
                idempotency_key,
                RequestedTiming::SessionReset,
            )
            .select_option(OptionParameter::EnemyDefaultPolicy, value)
            .build()
            .unwrap();
            let rejected = store
                .validate(&default_catalog().unwrap(), invalid_policy)
                .unwrap();
            assert!(!rejected.accepted);
            assert_eq!(rejected.diagnostics[0].code, expected_code);
            assert_eq!(store.state_bytes().unwrap(), before);
        }

        let negative_fireball_cost = ChangeSetBuilder::new(
            "custom-workspace",
            &definition,
            "negative-fireball-cost",
            RequestedTiming::SessionReset,
        )
        .set_integer(IntegerParameter::FireballManaCost, -1)
        .build()
        .unwrap();
        let rejected = store
            .validate(&default_catalog().unwrap(), negative_fireball_cost)
            .unwrap();
        assert_eq!(rejected.diagnostics[0].code, "OUT_OF_RANGE");
        assert_eq!(store.state_bytes().unwrap(), before);

        let misaligned_speed = ChangeSetBuilder::new(
            "custom-workspace",
            &definition,
            "misaligned-flight-speed",
            RequestedTiming::SessionReset,
        )
        .set_integer(IntegerParameter::PlayerMaxForwardSpeed, 20)
        .build()
        .unwrap();
        let rejected = store
            .validate(&default_catalog().unwrap(), misaligned_speed)
            .unwrap();
        assert!(!rejected.accepted);
        assert_eq!(rejected.diagnostics[0].code, "OUT_OF_RANGE");
        assert_eq!(store.state_bytes().unwrap(), before);

        let malformed = serde_json::json!({
            "schema_version": CHANGE_SET_SCHEMA,
            "target_workspace": "custom-workspace",
            "expected_definition_hash": definition.definition_hash(),
            "expected_definition_revision": 1,
            "idempotency_key": "bad-schema",
            "operations": [{"operation": "set_integer", "parameter": "flight.player.max_forward_speed", "value": 82}],
            "requested_timing": "session_reset",
            "intent": null,
            "session_generation": 7
        });
        assert!(serde_json::from_value::<ChangeSet>(malformed).is_err());
        let wrong_unit = serde_json::json!({
            "schema_version": CHANGE_SET_SCHEMA,
            "target_workspace": "custom-workspace",
            "expected_definition_hash": definition.definition_hash(),
            "expected_definition_revision": 1,
            "idempotency_key": "wrong-unit",
            "operations": [{
                "operation": "set_integer",
                "parameter": "spell.fireball.mana_cost",
                "value": 200,
                "unit": "source_velocity_units_per_entity_pass"
            }],
            "requested_timing": "session_reset",
            "intent": null
        });
        assert!(serde_json::from_value::<ChangeSet>(wrong_unit).is_err());
        let unsafe_timing = serde_json::json!({
            "schema_version": CHANGE_SET_SCHEMA,
            "target_workspace": "custom-workspace",
            "expected_definition_hash": definition.definition_hash(),
            "expected_definition_revision": 1,
            "idempotency_key": "unsafe-timing",
            "operations": [{
                "operation": "set_integer",
                "parameter": "spell.fireball.mana_cost",
                "value": 200
            }],
            "requested_timing": "next_tick",
            "intent": null
        });
        assert!(serde_json::from_value::<ChangeSet>(unsafe_timing).is_err());
        let legacy_replacement = serde_json::json!({
            "schema_version": CHANGE_SET_SCHEMA,
            "target_workspace": "custom-workspace",
            "expected_definition_hash": definition.definition_hash(),
            "expected_definition_revision": 1,
            "idempotency_key": "legacy-replacement",
            "changes": [{"id": "flight.player.max_forward_speed", "value": {"type": "integer", "value": 82}}],
            "requested_timing": "session_reset"
        });
        assert!(serde_json::from_value::<ChangeSet>(legacy_replacement).is_err());
        let legacy_v1 = serde_json::json!({
            "schema_version": "carpet-flight-change-set/v1",
            "target_workspace": "custom-workspace",
            "expected_definition_hash": definition.definition_hash(),
            "expected_definition_revision": 1,
            "idempotency_key": "legacy-v1",
            "operations": [{
                "operation": "set_integer",
                "parameter": "flight.player.max_forward_speed",
                "value": 80
            }],
            "requested_timing": "session_reset"
        });
        let legacy_v1 = serde_json::from_value::<ChangeSet>(legacy_v1).unwrap();
        let error = legacy_v1.canonicalized().unwrap_err();
        assert_eq!(error.code, TransactionErrorCode::InvalidRequest);
        assert_eq!(error.path_or_id, "schema_version");
        assert_eq!(store.state_bytes().unwrap(), before);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn typed_asset_publication_and_forward_rollback_are_atomic() {
        let root = temporary_workspace("f9-asset-atomic");
        let definition = fixture_definition();
        let store = WorkspaceStore::create(&root, "f9-asset", definition.clone()).unwrap();
        let catalog = default_catalog().unwrap();
        let change_set = ChangeSetBuilder::new(
            "f9-asset",
            &definition,
            "publish-asset-variant",
            RequestedTiming::PackageReload,
        )
        .bind_asset(
            PresentationAssetSlot::FireballProjectile,
            ArtifactId::parse("presentation.projectile_asset_variant").unwrap(),
        )
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted);
        let before = store.state_bytes().unwrap();
        let error = store
            .apply(
                &catalog,
                change_set.clone(),
                &validation.receipt_hash,
                PublicationFault::BeforeCommit,
            )
            .expect_err("pre-commit asset fault must reject");
        assert_eq!(error.code, TransactionErrorCode::Io);
        assert_eq!(store.state_bytes().unwrap(), before);
        assert_eq!(
            store.apply_status("publish-asset-variant").unwrap(),
            ApplyStatus::NotStarted
        );

        let applied = store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let active = store.active_definition().unwrap();
        assert_eq!(active.revision(), 2);
        assert_eq!(active.package_graph_hash(), definition.package_graph_hash());
        assert_eq!(
            active.artifact_graph_hash(),
            definition.artifact_graph_hash()
        );
        assert_eq!(
            active
                .presentation_asset(
                    &SemanticId::parse("presentation.spell.fireball.projectile_asset").unwrap()
                )
                .unwrap()
                .content_address,
            "sha256:c924ac200c84b9983374d1abcacff27414c3f565ac82792394353e0358ee0b34"
        );
        let variant_bytes = store.state_bytes().unwrap();
        let rollback_error = store
            .rollback(
                active.definition_hash(),
                active.revision(),
                &applied.transaction_id,
                "rollback-asset-variant",
                PublicationFault::BeforeCommit,
            )
            .expect_err("pre-commit rollback fault must reject");
        assert_eq!(rollback_error.code, TransactionErrorCode::Io);
        assert_eq!(store.state_bytes().unwrap(), variant_bytes);

        let rolled = store
            .rollback(
                active.definition_hash(),
                active.revision(),
                &applied.transaction_id,
                "rollback-asset-variant",
                PublicationFault::None,
            )
            .unwrap();
        assert_eq!(rolled.resulting_definition_revision, 3);
        let restored = store.active_definition().unwrap();
        assert_eq!(
            restored.package_graph_hash(),
            definition.package_graph_hash()
        );
        assert_eq!(
            restored.artifact_graph_hash(),
            definition.artifact_graph_hash()
        );
        assert_eq!(
            restored
                .presentation_asset(
                    &SemanticId::parse("presentation.spell.fireball.projectile_asset").unwrap()
                )
                .unwrap()
                .content_address,
            "sha256:11200749e51e49d4a9f4be95ffa8f6c0636be87e36781481c89e6a0825232508"
        );
        let _ = fs::remove_dir_all(root);
    }
}
