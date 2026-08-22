//! Neutral, immutable game-package and definition domain.
//!
//! This module owns definition identity and semantic discovery. It does not
//! own a live session, mutation transaction, transport, or gameplay scheduler.

mod catalog;
mod mc2_owned;
mod package;
mod presentation_asset;
mod transaction;

pub use catalog::{
    Access, ApplyBinding, DescriptorKind, MutationTiming, ReadBackBinding, ReadBinding,
    SemanticBinding, SemanticCatalog, SemanticDescriptor, SemanticId, SemanticValue,
    TemporalContract, ValidateBinding, ValueType, default_catalog,
};
pub use mc2_owned::{Mc2OwnedInputs, load_mc2_owned};
pub use package::{
    ArtifactKind, ArtifactNode, CompiledCapability, CompiledCapabilityRegistry, GamePackage,
    PackageProvenance, PackageProvenanceKind, RequiredCapability, engine_capabilities,
};
pub use presentation_asset::{
    BoundPresentationAsset, INDEXED_SPRITE_SCHEMA, IndexedPresentationAsset,
    PresentationAssetBudgets,
};
pub use transaction::{
    APPLY_RECEIPT_SCHEMA, ApplyReceipt, ApplyStatus, CHANGE_SET_SCHEMA, ChangeSet,
    ChangeSetBuilder, DefinitionDiff, Diagnostic, PublicationFault, Replacement, RequestedTiming,
    TransactionError, TransactionErrorCode, VALIDATION_RECEIPT_SCHEMA, ValidationReceipt,
    WORKSPACE_SCHEMA, WorkspaceInspection, WorkspaceStore,
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

pub const GAME_PACKAGE_SCHEMA: &str = "carpet-flight-game-package/v1";
pub const GAME_DEFINITION_SCHEMA: &str = "carpet-flight-game-definition/v1";
pub const SEMANTIC_CONTRACT_VERSION: &str = "1.0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefinitionErrorCode {
    InvalidRequest,
    InvalidSchema,
    UnknownMember,
    PathEscape,
    HashMismatch,
    InvalidProvenance,
    ExecutableCapabilityForbidden,
    CapabilityUnavailable,
    DependencyCycle,
    ArtifactCollision,
    InvalidSemanticId,
    SemanticIdCollision,
    DuplicateDescriptor,
    DuplicateBinding,
    CatalogBindingMismatch,
    UndiscoverableSetter,
    IncompleteWritableBinding,
    UnknownSemanticId,
    TypeMismatch,
    OutOfRange,
    DependencyFailed,
    Io,
}

impl DefinitionErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::InvalidSchema => "PACKAGE_INVALID",
            Self::UnknownMember => "PACKAGE_UNKNOWN_MEMBER",
            Self::PathEscape => "PACKAGE_PATH_ESCAPE",
            Self::HashMismatch => "HASH_MISMATCH",
            Self::InvalidProvenance => "PROVENANCE_INVALID",
            Self::ExecutableCapabilityForbidden => "PACKAGE_EXECUTABLE_CAPABILITY_FORBIDDEN",
            Self::CapabilityUnavailable => "CAPABILITY_UNAVAILABLE",
            Self::DependencyCycle => "PACKAGE_DEPENDENCY_CYCLE",
            Self::ArtifactCollision => "PACKAGE_ARTIFACT_COLLISION",
            Self::InvalidSemanticId => "INVALID_SEMANTIC_ID",
            Self::SemanticIdCollision => "SEMANTIC_ID_COLLISION",
            Self::DuplicateDescriptor => "DUPLICATE_DESCRIPTOR",
            Self::DuplicateBinding => "DUPLICATE_BINDING",
            Self::CatalogBindingMismatch => "CATALOG_BINDING_MISMATCH",
            Self::UndiscoverableSetter => "UNDISCOVERABLE_SETTER",
            Self::IncompleteWritableBinding => "INCOMPLETE_WRITABLE_BINDING",
            Self::UnknownSemanticId => "UNKNOWN_ID",
            Self::TypeMismatch => "TYPE_MISMATCH",
            Self::OutOfRange => "OUT_OF_RANGE",
            Self::DependencyFailed => "DEPENDENCY_FAILED",
            Self::Io => "IO",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefinitionError {
    pub code: DefinitionErrorCode,
    pub path_or_id: String,
    pub message: String,
}

impl DefinitionError {
    pub(crate) fn new(
        code: DefinitionErrorCode,
        path_or_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            path_or_id: path_or_id.into(),
            message: message.into(),
        }
    }

    #[must_use]
    pub fn to_json(&self, operation: &str) -> Value {
        json!({
            "code": self.code.as_str(),
            "message": self.message,
            "path_or_id": self.path_or_id,
            "operation": operation,
            "retryable": false,
            "state_changed": false,
            "remediation": "repair the named input, then rerun the same read-only operation",
            "safe_alternative": "capabilities",
            "expected_identity": Value::Null,
            "actual_identity": Value::Null,
        })
    }
}

impl fmt::Display for DefinitionError {
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

impl Error for DefinitionError {}

pub type Result<T> = std::result::Result<T, DefinitionError>;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GameDefinition {
    schema: String,
    revision: u64,
    definition_hash: String,
    package_id: String,
    package_version: u64,
    package_graph_hash: String,
    artifact_graph_hash: String,
    required_capabilities: Vec<RequiredCapability>,
    values: BTreeMap<SemanticId, SemanticValue>,
    artifacts: Vec<ArtifactNode>,
    #[serde(default)]
    artifact_payloads: BTreeMap<String, Vec<u8>>,
    provenance: PackageProvenance,
}

impl GameDefinition {
    /// Materialize an immutable definition from one validated package.
    ///
    /// # Errors
    ///
    /// Returns an error before construction if a required capability, binding,
    /// semantic value, or catalog relationship is invalid.
    pub fn from_package(
        package: GamePackage,
        capabilities: &CompiledCapabilityRegistry,
        catalog: &SemanticCatalog,
    ) -> Result<Self> {
        capabilities.require_all(&package.required_capabilities)?;
        let mut values = catalog
            .descriptors()
            .map(|descriptor| (descriptor.id.clone(), descriptor.default_value.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut observed = BTreeSet::new();
        for (id, value) in &package.bindings {
            if !observed.insert(id.clone()) {
                return Err(DefinitionError::new(
                    DefinitionErrorCode::SemanticIdCollision,
                    id.as_str(),
                    "package binds the same semantic ID more than once",
                ));
            }
            let descriptor = catalog.descriptor(id).ok_or_else(|| {
                DefinitionError::new(
                    DefinitionErrorCode::UnknownSemanticId,
                    id.as_str(),
                    "package binding is not declared by the engine catalog",
                )
            })?;
            descriptor.validate_value(value, id.as_str())?;
            values.insert(id.clone(), value.clone());
        }

        let mut definition = Self {
            schema: GAME_DEFINITION_SCHEMA.to_owned(),
            revision: 1,
            definition_hash: String::new(),
            package_id: package.package_id,
            package_version: package.package_version,
            package_graph_hash: package.package_graph_hash,
            artifact_graph_hash: package.artifact_graph_hash,
            required_capabilities: package.required_capabilities,
            values,
            artifacts: package.artifacts,
            artifact_payloads: package.artifact_payloads,
            provenance: package.provenance,
        };
        definition.verify_bound_presentation_assets(catalog)?;
        definition.definition_hash = definition.compute_hash();
        Ok(definition)
    }

    #[must_use]
    pub fn schema(&self) -> &str {
        &self.schema
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn definition_hash(&self) -> &str {
        &self.definition_hash
    }

    #[must_use]
    pub fn package_id(&self) -> &str {
        &self.package_id
    }

    #[must_use]
    pub const fn package_version(&self) -> u64 {
        self.package_version
    }

    #[must_use]
    pub fn package_graph_hash(&self) -> &str {
        &self.package_graph_hash
    }

    #[must_use]
    pub fn artifact_graph_hash(&self) -> &str {
        &self.artifact_graph_hash
    }

    #[must_use]
    pub fn required_capabilities(&self) -> &[RequiredCapability] {
        &self.required_capabilities
    }

    #[must_use]
    pub fn artifacts(&self) -> &[ArtifactNode] {
        &self.artifacts
    }

    /// Resolve and validate one typed presentation asset bound by semantic ID.
    /// Adapter-backed owner assets may be external; portable package assets
    /// must carry their verified payload in the immutable definition.
    pub fn presentation_asset(&self, id: &SemanticId) -> Result<BoundPresentationAsset> {
        presentation_asset::resolve(self, id)
    }

    pub(crate) fn validate_presentation_asset_value(
        &self,
        id: &SemanticId,
        value: &SemanticValue,
    ) -> Result<()> {
        let SemanticValue::Reference(reference) = value else {
            return Err(DefinitionError::new(
                DefinitionErrorCode::TypeMismatch,
                id.as_str(),
                "presentation replacement is not a content reference",
            ));
        };
        let _ = presentation_asset::resolve_reference(self, id, reference)?;
        Ok(())
    }

    pub(crate) fn artifact_payload(&self, artifact_id: &str) -> Option<&[u8]> {
        self.artifact_payloads.get(artifact_id).map(Vec::as_slice)
    }

    #[must_use]
    pub const fn provenance(&self) -> &PackageProvenance {
        &self.provenance
    }

    #[must_use]
    pub fn value(&self, id: &SemanticId) -> Option<&SemanticValue> {
        self.values.get(id)
    }

    /// Verify that this definition is a transaction-published semantic
    /// variant of `base`, with no package, artifact, provenance, capability,
    /// or read-only value substitution.
    ///
    /// # Errors
    ///
    /// Returns a typed error for identity corruption, incompatible package
    /// roots, undeclared values, or any change outside a writable binding.
    pub fn verify_semantic_variant_of(&self, base: &Self, catalog: &SemanticCatalog) -> Result<()> {
        self.verify_identity()?;
        base.verify_identity()?;
        if self.package_id != base.package_id
            || self.package_version != base.package_version
            || self.package_graph_hash != base.package_graph_hash
            || self.artifact_graph_hash != base.artifact_graph_hash
            || self.required_capabilities != base.required_capabilities
            || self.artifacts != base.artifacts
            || self.artifact_payloads != base.artifact_payloads
            || self.provenance != base.provenance
        {
            return Err(DefinitionError::new(
                DefinitionErrorCode::HashMismatch,
                "definition.package_graph_hash",
                "semantic variant cannot substitute package, artifact, capability, or provenance roots",
            ));
        }
        if self.values.keys().ne(base.values.keys()) {
            return Err(DefinitionError::new(
                DefinitionErrorCode::CatalogBindingMismatch,
                "definition.values",
                "semantic variant value set differs from its package base",
            ));
        }
        for (id, value) in &self.values {
            let descriptor = catalog.descriptor(id).ok_or_else(|| {
                DefinitionError::new(
                    DefinitionErrorCode::UnknownSemanticId,
                    id.as_str(),
                    "semantic variant contains a value outside the engine catalog",
                )
            })?;
            descriptor.validate_value(value, id.as_str())?;
            if base.values.get(id) != Some(value) {
                catalog.validate_replacement(base, id, value)?;
            }
        }
        Ok(())
    }

    pub(crate) fn with_replacement(
        &self,
        id: &SemanticId,
        value: SemanticValue,
        revision: u64,
    ) -> Result<Self> {
        if revision < self.revision {
            return Err(DefinitionError::new(
                DefinitionErrorCode::InvalidRequest,
                "definition.revision",
                "candidate definition revision must not move backward",
            ));
        }
        if !self.values.contains_key(id) {
            return Err(DefinitionError::new(
                DefinitionErrorCode::UnknownSemanticId,
                id.as_str(),
                "definition contains no value for semantic replacement",
            ));
        }
        let mut candidate = self.clone();
        candidate.revision = revision;
        candidate.values.insert(id.clone(), value);
        candidate.definition_hash = candidate.compute_hash();
        Ok(candidate)
    }

    pub(crate) fn reissue_at_revision(&self, revision: u64) -> Result<Self> {
        if revision <= self.revision {
            return Err(DefinitionError::new(
                DefinitionErrorCode::InvalidRequest,
                "definition.revision",
                "forward publication must advance the definition revision",
            ));
        }
        let mut candidate = self.clone();
        candidate.revision = revision;
        candidate.definition_hash = candidate.compute_hash();
        Ok(candidate)
    }

    pub(crate) fn verify_identity(&self) -> Result<()> {
        if self.schema != GAME_DEFINITION_SCHEMA {
            return Err(DefinitionError::new(
                DefinitionErrorCode::InvalidSchema,
                "definition.schema",
                "unsupported persisted definition schema",
            ));
        }
        if self.definition_hash != self.compute_hash() {
            return Err(DefinitionError::new(
                DefinitionErrorCode::HashMismatch,
                "definition.definition_hash",
                "persisted definition identity does not match its canonical payload",
            ));
        }
        let artifact_payload =
            Value::Array(self.artifacts.iter().map(ArtifactNode::to_json).collect());
        let actual_artifact_graph_hash = format!(
            "sha256:{:x}",
            Sha256::digest(
                serde_json::to_vec(&artifact_payload)
                    .expect("artifact identity payload is serializable")
            )
        );
        if self.artifact_graph_hash != actual_artifact_graph_hash {
            return Err(DefinitionError::new(
                DefinitionErrorCode::HashMismatch,
                "definition.artifact_graph_hash",
                "persisted artifact nodes do not match the retained artifact graph identity",
            ));
        }
        for (artifact_id, payload) in &self.artifact_payloads {
            let artifact = self
                .artifacts
                .iter()
                .find(|artifact| &artifact.id == artifact_id)
                .ok_or_else(|| {
                    DefinitionError::new(
                        DefinitionErrorCode::HashMismatch,
                        artifact_id,
                        "retained payload has no artifact node",
                    )
                })?;
            if payload.len() as u64 != artifact.bytes
                || format!("{:x}", Sha256::digest(payload)) != artifact.sha256
            {
                return Err(DefinitionError::new(
                    DefinitionErrorCode::HashMismatch,
                    artifact_id,
                    "retained artifact payload does not match its content identity",
                ));
            }
        }
        Ok(())
    }

    fn verify_bound_presentation_assets(&self, catalog: &SemanticCatalog) -> Result<()> {
        for descriptor in catalog
            .descriptors()
            .filter(|descriptor| descriptor.kind == DescriptorKind::AssetSlot)
        {
            let Some(SemanticValue::Reference(reference)) = self.value(&descriptor.id) else {
                continue;
            };
            let Some(artifact) = self
                .artifacts
                .iter()
                .find(|artifact| format!("sha256:{}", artifact.sha256) == *reference)
            else {
                continue;
            };
            if self.artifact_payload(&artifact.id).is_some() {
                let _ = self.presentation_asset(&descriptor.id)?;
            }
        }
        Ok(())
    }

    fn compute_hash(&self) -> String {
        let definition_payload = json!({
            "schema": self.schema,
            "revision": self.revision,
            "package_id": self.package_id,
            "package_version": self.package_version,
            "package_graph_hash": self.package_graph_hash,
            "artifact_graph_hash": self.artifact_graph_hash,
            "required_capabilities": self.required_capabilities.iter().map(RequiredCapability::to_json).collect::<Vec<_>>(),
            "values": self.values.iter().map(|(id, value)| json!({"id": id.as_str(), "value": value.to_json()})).collect::<Vec<_>>(),
        });
        format!(
            "sha256:{:x}",
            Sha256::digest(
                serde_json::to_vec(&definition_payload)
                    .expect("definition identity payload is serializable")
            )
        )
    }
}

pub(crate) fn exact_object<'a>(
    value: &'a Value,
    expected: &[&str],
    path: &str,
) -> Result<&'a serde_json::Map<String, Value>> {
    let object = value.as_object().ok_or_else(|| {
        DefinitionError::new(
            DefinitionErrorCode::InvalidSchema,
            path,
            "value is not a JSON object",
        )
    })?;
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        let unknown = actual.difference(&expected).copied().collect::<Vec<_>>();
        return Err(DefinitionError::new(
            DefinitionErrorCode::UnknownMember,
            path,
            format!("object member set differs; unknown={unknown:?}"),
        ));
    }
    Ok(object)
}

pub(crate) fn required_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a str> {
    object[key].as_str().ok_or_else(|| {
        DefinitionError::new(
            DefinitionErrorCode::InvalidSchema,
            format!("{path}.{key}"),
            "value is not a string",
        )
    })
}

pub(crate) fn lower_sha256(value: &str, path: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(DefinitionError::new(
            DefinitionErrorCode::HashMismatch,
            path,
            "value is not a lowercase SHA-256",
        ));
    }
    Ok(())
}

pub(crate) fn lower_content_hash(value: &str, path: &str) -> Result<()> {
    let Some(hash) = value.strip_prefix("sha256:") else {
        return Err(DefinitionError::new(
            DefinitionErrorCode::HashMismatch,
            path,
            "content reference must use sha256:<lowercase-hex>",
        ));
    };
    lower_sha256(hash, path)
}
