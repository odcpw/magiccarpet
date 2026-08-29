//! Neutral, immutable game-package and definition domain.
//!
//! This module owns definition identity, semantic discovery, and atomic
//! definition transactions. It does not own a live gameplay session,
//! transport, or gameplay scheduler.

mod catalog;
mod operation;
mod package;
mod presentation_asset;
mod transaction;

#[doc(hidden)]
pub mod adapter;
pub mod level_definition;
pub mod runtime_flow;

pub use catalog::{
    Access, ApplyBinding, DescriptorKind, MutationTiming, ReadBackBinding, ReadBinding,
    SemanticBinding, SemanticCatalog, SemanticDescriptor, SemanticId, SemanticValue,
    TemporalContract, ValidateBinding, ValueType, default_catalog,
};
pub use operation::{
    ArtifactId, DefinitionOperation, IntegerParameter, OptionParameter, PresentationAssetSlot,
};
pub use package::{
    ARCHER_MAX_LIFE_CAPABILITY_ID, ArtifactKind, ArtifactNode, BEE_MAX_LIFE_CAPABILITY_ID,
    CREATURE_POLICY_CAPABILITY_ID, CompiledCapability, CompiledCapabilityRegistry,
    DRAGON_MAX_LIFE_CAPABILITY_ID, EXPERIENCE_SCROLL_AWARD_CAPABILITY_ID,
    FIREBALL_MANA_COST_CAPABILITY_ID, FIREFLY_MAX_LIFE_CAPABILITY_ID, GOAT_MAX_LIFE_CAPABILITY_ID,
    GamePackage, PackageProvenance, PackageProvenanceKind, RequiredCapability,
    SKELETON_MAX_LIFE_CAPABILITY_ID, TRADER_MAX_LIFE_CAPABILITY_ID,
    VILLAGER_MAX_LIFE_CAPABILITY_ID, WORM_MAX_LIFE_CAPABILITY_ID, creature_max_life_capability_id,
    engine_capabilities,
};
pub use presentation_asset::{
    BoundPresentationAsset, CompatiblePresentationArtifact, INDEXED_SPRITE_SCHEMA,
    IndexedPresentationAsset, PresentationAssetBudgets,
};
pub use transaction::{
    APPLY_RECEIPT_SCHEMA, ApplyReceipt, ApplyStatus, CHANGE_SET_SCHEMA, ChangeSet,
    ChangeSetBuilder, DefinitionDiff, Diagnostic, PublicationFault, RequestedTiming,
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

use level_definition::{CreatureKind, LevelDefinition, LevelEntityKind};
use runtime_flow::RuntimeFlowDefinition;

pub const GAME_PACKAGE_SCHEMA: &str = "carpet-flight-game-package/v1";
pub const GAME_DEFINITION_SCHEMA: &str = "carpet-flight-game-definition/v8";
pub const SEMANTIC_CONTRACT_VERSION: &str = "29.0";

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    level_definition: Option<LevelDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    runtime_flow: Option<RuntimeFlowDefinition>,
    provenance: PackageProvenance,
}

impl GameDefinition {
    /// Project one catalog descriptor through the capabilities of this
    /// immutable definition.
    ///
    /// Engine-wide catalog entries can be writable for one package while any
    /// definition without the matching runtime consumer must advertise the
    /// same semantic value as read-only.
    #[must_use]
    pub fn semantic_descriptor(
        &self,
        catalog: &SemanticCatalog,
        id: &SemanticId,
    ) -> Option<SemanticDescriptor> {
        let mut descriptor = catalog.descriptor(id)?.clone();
        if id.as_str() == OptionParameter::EnemyDefaultPolicy.as_str() {
            let capability_present = self.required_capabilities.iter().any(|requirement| {
                requirement.id.as_str() == CREATURE_POLICY_CAPABILITY_ID
                    && requirement.exact_version == 1
            });
            let package_has_goats = self.level_definition.as_ref().is_none_or(|level| {
                level
                    .entities()
                    .iter()
                    .any(|entity| entity.kind() == LevelEntityKind::Creature(CreatureKind::Goat))
            });
            if !capability_present || !package_has_goats {
                descriptor.access = Access::ReadOnly;
            }
        }
        if id.as_str() == IntegerParameter::FireballManaCost.as_str()
            && !self.required_capabilities.iter().any(|requirement| {
                requirement.id.as_str() == FIREBALL_MANA_COST_CAPABILITY_ID
                    && requirement.exact_version == 1
            })
        {
            descriptor.access = Access::ReadOnly;
        }
        if id.as_str() == PresentationAssetSlot::FireballProjectile.as_str() {
            let capability_present = self.required_capabilities.iter().any(|requirement| {
                requirement.id.as_str() == "engine.presentation.indexed_asset"
                    && requirement.exact_version == 1
            });
            let has_choice = matches!(
                self.compatible_presentation_artifacts(id),
                Ok(artifacts) if artifacts.len() >= 2
            );
            if !capability_present || !has_choice {
                descriptor.access = Access::ReadOnly;
            }
        }
        Some(descriptor)
    }

    /// Project every engine catalog descriptor through this definition's
    /// package-specific runtime capabilities.
    #[must_use]
    pub fn semantic_descriptors(&self, catalog: &SemanticCatalog) -> Vec<SemanticDescriptor> {
        catalog
            .descriptors()
            .filter_map(|descriptor| self.semantic_descriptor(catalog, &descriptor.id))
            .collect()
    }

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
            level_definition: None,
            runtime_flow: None,
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

    /// Package-authored semantic level content, when the package defines a
    /// playable level rather than only reusable engine values or assets.
    #[must_use]
    pub const fn level_definition(&self) -> Option<&LevelDefinition> {
        self.level_definition.as_ref()
    }

    /// Package-bound semantic identity and declared completion destination for
    /// this playable definition, when its adapter supplies one.
    #[must_use]
    pub const fn runtime_flow(&self) -> Option<&RuntimeFlowDefinition> {
        self.runtime_flow.as_ref()
    }

    pub(crate) fn with_level_definition(mut self, level_definition: LevelDefinition) -> Self {
        self.level_definition = Some(level_definition);
        self.definition_hash = self.compute_hash();
        self
    }

    pub(crate) fn with_level_and_runtime_flow(
        mut self,
        level_definition: LevelDefinition,
        runtime_flow: RuntimeFlowDefinition,
    ) -> Self {
        self.level_definition = Some(level_definition);
        self.runtime_flow = Some(runtime_flow);
        self.definition_hash = self.compute_hash();
        self
    }

    /// Resolve and validate one typed presentation asset bound by semantic ID.
    /// Adapter-backed owner assets may be external; portable package assets
    /// must carry their verified payload in the immutable definition.
    pub fn presentation_asset(&self, id: &SemanticId) -> Result<BoundPresentationAsset> {
        presentation_asset::resolve(self, id)
    }

    /// Enumerate package-retained, typed alternatives compatible with the
    /// current presentation slot. No path or source selector crosses this
    /// definition-owned discovery surface.
    pub fn compatible_presentation_artifacts(
        &self,
        id: &SemanticId,
    ) -> Result<Vec<CompatiblePresentationArtifact>> {
        presentation_asset::compatible_artifacts(self, id)
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
        if !self
            .compatible_presentation_artifacts(id)?
            .iter()
            .any(|artifact| artifact.content_address == *reference)
        {
            return Err(DefinitionError::new(
                DefinitionErrorCode::TypeMismatch,
                id.as_str(),
                "presentation replacement is not structurally compatible with the active slot",
            ));
        }
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
        if self.runtime_flow != base.runtime_flow {
            return Err(DefinitionError::new(
                DefinitionErrorCode::HashMismatch,
                "definition.runtime_flow",
                "semantic variant cannot substitute game, level, or completion-flow identity",
            ));
        }
        match (&self.level_definition, &base.level_definition) {
            (Some(candidate), Some(original)) => candidate
                .verify_session_reset_variant_of(original)
                .map_err(|message| {
                    DefinitionError::new(
                        DefinitionErrorCode::HashMismatch,
                        "definition.level_definition",
                        message,
                    )
                })?,
            (None, None) => {}
            _ => {
                return Err(DefinitionError::new(
                    DefinitionErrorCode::HashMismatch,
                    "definition.level_definition",
                    "semantic variant cannot add or remove a level definition",
                ));
            }
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

    pub(crate) fn with_moved_level_entity(
        &self,
        id: &level_definition::LevelEntityId,
        position: level_definition::TilePosition,
        revision: u64,
    ) -> Result<Self> {
        if revision < self.revision {
            return Err(DefinitionError::new(
                DefinitionErrorCode::InvalidRequest,
                "definition.revision",
                "candidate definition revision must not move backward",
            ));
        }
        let level = self.level_definition.as_ref().ok_or_else(|| {
            DefinitionError::new(
                DefinitionErrorCode::DependencyFailed,
                id.as_str(),
                "definition has no semantic level placement consumer",
            )
        })?;
        let mut candidate = self.clone();
        candidate.level_definition =
            Some(level.with_moved_entity(id, position).map_err(|message| {
                DefinitionError::new(DefinitionErrorCode::InvalidRequest, id.as_str(), message)
            })?);
        candidate.revision = revision;
        candidate.definition_hash = candidate.compute_hash();
        Ok(candidate)
    }

    pub(crate) fn with_put_level_objective(
        &self,
        id: &level_definition::LevelObjectiveId,
        condition: level_definition::ObjectiveCondition,
        revision: u64,
    ) -> Result<Self> {
        if revision < self.revision {
            return Err(DefinitionError::new(
                DefinitionErrorCode::InvalidRequest,
                "definition.revision",
                "candidate definition revision must not move backward",
            ));
        }
        let level = self.level_definition.as_ref().ok_or_else(|| {
            DefinitionError::new(
                DefinitionErrorCode::DependencyFailed,
                id.as_str(),
                "definition has no semantic objective consumer",
            )
        })?;
        let mut candidate = self.clone();
        candidate.level_definition =
            Some(level.with_put_objective(id, condition).map_err(|message| {
                DefinitionError::new(DefinitionErrorCode::InvalidRequest, id.as_str(), message)
            })?);
        candidate.revision = revision;
        candidate.definition_hash = candidate.compute_hash();
        Ok(candidate)
    }

    pub(crate) fn with_countdown_arming_ticks(
        &self,
        id: &level_definition::LevelEntityId,
        arming_ticks: u32,
        revision: u64,
    ) -> Result<Self> {
        if revision < self.revision {
            return Err(DefinitionError::new(
                DefinitionErrorCode::InvalidRequest,
                "definition.revision",
                "candidate definition revision must not move backward",
            ));
        }
        let level = self.level_definition.as_ref().ok_or_else(|| {
            DefinitionError::new(
                DefinitionErrorCode::DependencyFailed,
                id.as_str(),
                "definition has no semantic mechanic consumer",
            )
        })?;
        let mut candidate = self.clone();
        candidate.level_definition = Some(
            level
                .with_countdown_arming_ticks(id, arming_ticks)
                .map_err(|message| {
                    DefinitionError::new(DefinitionErrorCode::InvalidRequest, id.as_str(), message)
                })?,
        );
        candidate.revision = revision;
        candidate.definition_hash = candidate.compute_hash();
        Ok(candidate)
    }

    pub(crate) fn with_creature_max_life(
        &self,
        id: &level_definition::LevelEntityId,
        max_life: u32,
        revision: u64,
    ) -> Result<Self> {
        if revision < self.revision {
            return Err(DefinitionError::new(
                DefinitionErrorCode::InvalidRequest,
                "definition.revision",
                "candidate definition revision must not move backward",
            ));
        }
        let level = self.level_definition.as_ref().ok_or_else(|| {
            DefinitionError::new(
                DefinitionErrorCode::DependencyFailed,
                id.as_str(),
                "definition has no semantic creature-health consumer",
            )
        })?;
        let mut candidate = self.clone();
        candidate.level_definition = Some(level.with_creature_max_life(id, max_life).map_err(
            |message| {
                DefinitionError::new(DefinitionErrorCode::InvalidRequest, id.as_str(), message)
            },
        )?);
        candidate.revision = revision;
        candidate.definition_hash = candidate.compute_hash();
        Ok(candidate)
    }

    pub(crate) fn with_experience_scroll_award(
        &self,
        id: &level_definition::LevelEntityId,
        experience_award: u32,
        revision: u64,
    ) -> Result<Self> {
        if revision < self.revision {
            return Err(DefinitionError::new(
                DefinitionErrorCode::InvalidRequest,
                "definition.revision",
                "candidate definition revision must not move backward",
            ));
        }
        let level = self.level_definition.as_ref().ok_or_else(|| {
            DefinitionError::new(
                DefinitionErrorCode::DependencyFailed,
                id.as_str(),
                "definition has no semantic Experience Scroll consumer",
            )
        })?;
        let mut candidate = self.clone();
        candidate.level_definition = Some(
            level
                .with_experience_scroll_award(id, experience_award)
                .map_err(|message| {
                    DefinitionError::new(DefinitionErrorCode::InvalidRequest, id.as_str(), message)
                })?,
        );
        candidate.revision = revision;
        candidate.definition_hash = candidate.compute_hash();
        Ok(candidate)
    }

    /// Read one semantic authoring target from the immutable definition.
    #[must_use]
    pub fn semantic_target_value(&self, id: &SemanticId) -> Option<SemanticValue> {
        if let Some(value) = self.value(id) {
            return Some(value.clone());
        }
        if let Some(entity) = id.as_str().strip_suffix(".position") {
            let entity_id = level_definition::LevelEntityId::parse(entity).ok()?;
            return self
                .level_definition
                .as_ref()?
                .entity(&entity_id)
                .map(|entity| SemanticValue::Position(entity.position()));
        }
        if let Some(entity) = id.as_str().strip_suffix(".arming_ticks") {
            let entity_id = level_definition::LevelEntityId::parse(entity).ok()?;
            return self
                .level_definition
                .as_ref()?
                .entity(&entity_id)
                .and_then(level_definition::LevelEntityDefinition::absence_countdown)
                .map(|config| SemanticValue::Integer(i64::from(config.arming_ticks())));
        }
        if let Some(entity) = id.as_str().strip_suffix(".max_life") {
            let entity_id = level_definition::LevelEntityId::parse(entity).ok()?;
            return self
                .level_definition
                .as_ref()?
                .entity(&entity_id)
                .and_then(level_definition::LevelEntityDefinition::creature_health)
                .map(|config| SemanticValue::Integer(i64::from(config.max_life())));
        }
        if let Some(entity) = id.as_str().strip_suffix(".experience_award") {
            let entity_id = level_definition::LevelEntityId::parse(entity).ok()?;
            return self
                .level_definition
                .as_ref()?
                .entity(&entity_id)
                .and_then(level_definition::LevelEntityDefinition::experience_scroll)
                .map(|config| SemanticValue::Integer(i64::from(config.experience_award())));
        }
        if let Some(objective) = id.as_str().strip_suffix(".condition") {
            let objective_id = level_definition::LevelObjectiveId::parse(objective).ok()?;
            return self
                .level_definition
                .as_ref()?
                .objective(&objective_id)
                .map(|objective| SemanticValue::ObjectiveCondition(objective.condition()));
        }
        None
    }

    /// Describe one stable semantic level-entity position target.
    #[must_use]
    pub fn level_entity_position_descriptor(&self, id: &SemanticId) -> Option<SemanticDescriptor> {
        let entity_id = id.as_str().strip_suffix(".position")?;
        let entity_id = level_definition::LevelEntityId::parse(entity_id).ok()?;
        let entity = self.level_definition.as_ref()?.entity(&entity_id)?;
        let writable = matches!(
            entity.position_access(),
            level_definition::LevelEntityPositionAccess::SessionReset
        );
        let target_id = id.as_str().to_owned();
        Some(SemanticDescriptor {
            id: id.clone(),
            descriptor_version: 1,
            kind: DescriptorKind::Placement,
            value_type: ValueType::Position,
            unit: Some("level_tile".to_owned()),
            default_value: SemanticValue::Position(entity.default_position()),
            minimum: Some(0),
            maximum: Some(i64::from(u8::MAX)),
            enum_values: Vec::new(),
            access: if writable {
                Access::Writable
            } else {
                Access::ReadOnly
            },
            mutation_timing: MutationTiming::SessionReset,
            determinism_role: "immutable package placement authoring target".to_owned(),
            dependencies: vec![
                entity.id().as_str().to_owned(),
                "package runtime carrier binding".to_owned(),
            ],
            side_effects: if writable {
                vec![
                    "explicit reopen/reset materializes a new runtime position".to_owned(),
                    "new runtime generation updates map membership and semantic projections"
                        .to_owned(),
                ]
            } else {
                Vec::new()
            },
            scope: format!(
                "stable level entity {} ({:?})",
                entity.id().as_str(),
                entity.kind()
            ),
            provenance_origin: "package semantic level definition".to_owned(),
            provenance_content_hash: self.package_graph_hash.clone(),
            temporal_contract: Some(TemporalContract {
                dispatch_phase: "definition_publication".to_owned(),
                dispatch_key: target_id.clone(),
                ordinary_producer: "validated MoveEntity candidate".to_owned(),
                handler: "guarded atomic definition compare-and-swap".to_owned(),
                same_pass_epilogue:
                    "receipt/read-back commit; already-open sessions remain unchanged".to_owned(),
                following_pass_cleanup: "explicit reset/reopen creates a new runtime generation and materializes position, map membership, and semantic projections before gameplay".to_owned(),
                authoritative_reads: vec![target_id],
                authoritative_writes: vec!["immutable LevelDefinition position".to_owned()],
                rng_and_event_effects:
                    "none during definition publication or placement materialization".to_owned(),
            }),
            summary: "Tile position of one stable semantic level entity.".to_owned(),
        })
    }

    /// Describe every position target in stable level-definition order.
    #[must_use]
    pub fn level_entity_position_descriptors(&self) -> Vec<SemanticDescriptor> {
        self.level_definition
            .as_ref()
            .into_iter()
            .flat_map(level_definition::LevelDefinition::entities)
            .filter_map(|entity| {
                SemanticId::parse(&format!("{}.position", entity.id().as_str()))
                    .ok()
                    .and_then(|id| self.level_entity_position_descriptor(&id))
            })
            .collect()
    }

    /// Describe one package-present absence-countdown arming delay.
    #[must_use]
    pub fn level_countdown_arming_ticks_descriptor(
        &self,
        id: &SemanticId,
    ) -> Option<SemanticDescriptor> {
        let entity_id = id.as_str().strip_suffix(".arming_ticks")?;
        let entity_id = level_definition::LevelEntityId::parse(entity_id).ok()?;
        let entity = self.level_definition.as_ref()?.entity(&entity_id)?;
        let config = entity.absence_countdown()?;
        let writable = matches!(
            config.arming_ticks_access(),
            level_definition::LevelEntityParameterAccess::SessionReset
        );
        let target_id = id.as_str().to_owned();
        Some(SemanticDescriptor {
            id: id.clone(),
            descriptor_version: 1,
            kind: DescriptorKind::Mechanic,
            value_type: ValueType::Integer,
            unit: Some("entity_visits".to_owned()),
            default_value: SemanticValue::Integer(i64::from(config.default_arming_ticks())),
            minimum: Some(1),
            maximum: Some(i64::from(i32::MAX)),
            enum_values: Vec::new(),
            access: if writable {
                Access::Writable
            } else {
                Access::ReadOnly
            },
            mutation_timing: MutationTiming::SessionReset,
            determinism_role: "immutable per-instance mechanic configuration".to_owned(),
            dependencies: vec![
                entity.id().as_str().to_owned(),
                "package absence-countdown runtime binding".to_owned(),
            ],
            side_effects: if writable {
                vec![
                    "already-open sessions remain unchanged until explicit reopen/reset".to_owned(),
                    "ordinary countdown callbacks use the authored arming delay".to_owned(),
                ]
            } else {
                Vec::new()
            },
            scope: format!("stable absence countdown {}", entity.id().as_str()),
            provenance_origin: "package semantic level definition".to_owned(),
            provenance_content_hash: self.package_graph_hash.clone(),
            temporal_contract: Some(TemporalContract {
                dispatch_phase: "definition_publication".to_owned(),
                dispatch_key: target_id.clone(),
                ordinary_producer: "validated SetMechanicCountdown candidate".to_owned(),
                handler: "guarded atomic definition compare-and-swap".to_owned(),
                same_pass_epilogue:
                    "receipt/read-back commit; already-open sessions remain unchanged".to_owned(),
                following_pass_cleanup: "explicit reset/reopen creates a new runtime generation; the package-bound countdown reads the authored delay on its next arming visit"
                    .to_owned(),
                authoritative_reads: vec![target_id],
                authoritative_writes: vec![
                    "immutable LevelDefinition absence-countdown configuration".to_owned(),
                ],
                rng_and_event_effects:
                    "none during publication; countdown audio/group effects remain scheduler-owned"
                        .to_owned(),
            }),
            summary: "Clear-family visits loaded when this absence countdown arms.".to_owned(),
        })
    }

    /// Describe every absence-countdown arming target in level order.
    #[must_use]
    pub fn level_countdown_arming_ticks_descriptors(&self) -> Vec<SemanticDescriptor> {
        self.level_definition
            .as_ref()
            .into_iter()
            .flat_map(level_definition::LevelDefinition::entities)
            .filter(|entity| entity.absence_countdown().is_some())
            .filter_map(|entity| {
                SemanticId::parse(&format!("{}.arming_ticks", entity.id().as_str()))
                    .ok()
                    .and_then(|id| self.level_countdown_arming_ticks_descriptor(&id))
            })
            .collect()
    }

    /// Describe one package-present creature maximum-life target.
    #[must_use]
    pub fn level_creature_max_life_descriptor(
        &self,
        id: &SemanticId,
    ) -> Option<SemanticDescriptor> {
        let entity_id = id.as_str().strip_suffix(".max_life")?;
        let entity_id = level_definition::LevelEntityId::parse(entity_id).ok()?;
        let entity = self.level_definition.as_ref()?.entity(&entity_id)?;
        let config = entity.creature_health()?;
        let creature = match entity.kind() {
            level_definition::LevelEntityKind::Creature(kind) => kind,
            _ => return None,
        };
        let capability_id = creature_max_life_capability_id(creature)?;
        let writable = matches!(
            config.max_life_access(),
            level_definition::LevelEntityParameterAccess::SessionReset
        ) && self.required_capabilities.iter().any(|requirement| {
            requirement.id.as_str() == capability_id && requirement.exact_version == 1
        });
        let target_id = id.as_str().to_owned();
        let mut side_effects = Vec::new();
        if writable {
            side_effects.push(format!(
                "already-open sessions retain their original {} health",
                creature.as_str()
            ));
            side_effects.push(
                "explicit reopen/reset initializes both life and maximum life from this value"
                    .to_owned(),
            );
            if creature == level_definition::CreatureKind::Bee {
                side_effects.push(
                    "the authored Bee constructor derives its death-conversion mana as half this value"
                        .to_owned(),
                );
            }
        }
        Some(SemanticDescriptor {
            id: id.clone(),
            descriptor_version: 1,
            kind: DescriptorKind::EntityParameter,
            value_type: ValueType::Integer,
            unit: Some("life_points".to_owned()),
            default_value: SemanticValue::Integer(i64::from(config.default_max_life())),
            minimum: Some(1),
            maximum: Some(i64::from(
                level_definition::creature_max_life_upper_bound(creature)?,
            )),
            enum_values: Vec::new(),
            access: if writable {
                Access::Writable
            } else {
                Access::ReadOnly
            },
            mutation_timing: MutationTiming::SessionReset,
            determinism_role: "immutable per-instance creature constructor parameter".to_owned(),
            dependencies: vec![
                entity.id().as_str().to_owned(),
                format!("{capability_id}@1"),
                format!(
                    "package {} constructor and complete damage/death lifecycle",
                    creature.as_str()
                ),
            ],
            side_effects,
            scope: format!("stable {} instance {}", creature.as_str(), entity.id().as_str()),
            provenance_origin: "package semantic level definition".to_owned(),
            provenance_content_hash: self.package_graph_hash.clone(),
            temporal_contract: Some(TemporalContract {
                dispatch_phase: "definition_publication".to_owned(),
                dispatch_key: target_id.clone(),
                ordinary_producer: "validated SetEntityMaxLife candidate".to_owned(),
                handler: "guarded atomic definition compare-and-swap".to_owned(),
                same_pass_epilogue:
                    "receipt/read-back commit; already-open sessions remain unchanged".to_owned(),
                following_pass_cleanup: format!(
                    "explicit reopen/reset binds the authored value before the package group constructs this {}; ordinary damage, pre-kill, hide, and reclaim consume that carrier",
                    creature.as_str()
                ),
                authoritative_reads: vec![target_id],
                authoritative_writes: vec![
                    format!(
                        "immutable LevelDefinition {} max_life configuration",
                        creature.as_str()
                    ),
                ],
                rng_and_event_effects:
                    "none during publication or construction; damage and death event order remains scheduler-owned"
                        .to_owned(),
            }),
            summary: format!(
                "Initial and maximum life of one package-bound {}.",
                creature.as_str()
            ),
        })
    }

    /// Describe every package-bound creature maximum-life target in level order.
    #[must_use]
    pub fn level_creature_max_life_descriptors(&self) -> Vec<SemanticDescriptor> {
        self.level_definition
            .as_ref()
            .into_iter()
            .flat_map(level_definition::LevelDefinition::entities)
            .filter(|entity| entity.creature_health().is_some())
            .filter_map(|entity| {
                SemanticId::parse(&format!("{}.max_life", entity.id().as_str()))
                    .ok()
                    .and_then(|id| self.level_creature_max_life_descriptor(&id))
            })
            .collect()
    }

    /// Describe one package-bound Experience Scroll reward target.
    #[must_use]
    pub fn level_experience_scroll_award_descriptor(
        &self,
        id: &SemanticId,
    ) -> Option<SemanticDescriptor> {
        let entity_id = id.as_str().strip_suffix(".experience_award")?;
        let entity_id = level_definition::LevelEntityId::parse(entity_id).ok()?;
        let entity = self.level_definition.as_ref()?.entity(&entity_id)?;
        let config = entity.experience_scroll()?;
        let writable = matches!(
            config.experience_award_access(),
            level_definition::LevelEntityParameterAccess::SessionReset
        ) && self.required_capabilities.iter().any(|requirement| {
            requirement.id.as_str() == EXPERIENCE_SCROLL_AWARD_CAPABILITY_ID
                && requirement.exact_version == 1
        });
        let target_id = id.as_str().to_owned();
        Some(SemanticDescriptor {
            id: id.clone(),
            descriptor_version: 1,
            kind: DescriptorKind::EntityParameter,
            value_type: ValueType::Integer,
            unit: Some("experience_points".to_owned()),
            default_value: SemanticValue::Integer(i64::from(config.default_experience_award())),
            minimum: Some(1),
            maximum: Some(i64::from(i32::MAX)),
            enum_values: Vec::new(),
            access: if writable { Access::Writable } else { Access::ReadOnly },
            mutation_timing: MutationTiming::SessionReset,
            determinism_role: "immutable per-instance Experience Scroll reward".to_owned(),
            dependencies: vec![
                entity.id().as_str().to_owned(),
                format!("{EXPERIENCE_SCROLL_AWARD_CAPABILITY_ID}@1"),
                "package Experience Scroll constructor and collector".to_owned(),
            ],
            side_effects: if writable {
                vec![
                    "already-open sessions retain the package reward".to_owned(),
                    "explicit reopen/reset binds the authored award before collection".to_owned(),
                ]
            } else { Vec::new() },
            scope: format!("stable Experience Scroll {}", entity.id().as_str()),
            provenance_origin: "package semantic level definition".to_owned(),
            provenance_content_hash: self.package_graph_hash.clone(),
            temporal_contract: Some(TemporalContract {
                dispatch_phase: "definition_publication".to_owned(),
                dispatch_key: target_id.clone(),
                ordinary_producer: "validated SetExperienceScrollAward candidate".to_owned(),
                handler: "guarded atomic definition compare-and-swap".to_owned(),
                same_pass_epilogue: "receipt/read-back commit; already-open sessions remain unchanged".to_owned(),
                following_pass_cleanup: "explicit reopen/reset binds the reward into the authored row; ordinary overlap awards XP, hides the row, and following-boundary reclaim removes it".to_owned(),
                authoritative_reads: vec![target_id],
                authoritative_writes: vec!["immutable LevelDefinition Experience Scroll reward".to_owned()],
                rng_and_event_effects: "none during publication; collection event order remains scheduler-owned".to_owned(),
            }),
            summary: "Experience awarded by one package-bound Experience Scroll.".to_owned(),
        })
    }

    #[must_use]
    pub fn level_experience_scroll_award_descriptors(&self) -> Vec<SemanticDescriptor> {
        self.level_definition
            .as_ref()
            .into_iter()
            .flat_map(level_definition::LevelDefinition::entities)
            .filter(|entity| entity.experience_scroll().is_some())
            .filter_map(|entity| {
                SemanticId::parse(&format!("{}.experience_award", entity.id().as_str()))
                    .ok()
                    .and_then(|id| self.level_experience_scroll_award_descriptor(&id))
            })
            .collect()
    }

    /// Describe one stable semantic objective-condition target.
    #[must_use]
    pub fn level_objective_condition_descriptor(
        &self,
        id: &SemanticId,
    ) -> Option<SemanticDescriptor> {
        let objective_id = id.as_str().strip_suffix(".condition")?;
        let objective_id = level_definition::LevelObjectiveId::parse(objective_id).ok()?;
        let objective = self.level_definition.as_ref()?.objective(&objective_id)?;
        let writable = matches!(
            objective.condition_access(),
            level_definition::LevelObjectiveConditionAccess::SessionReset
        );
        let target_id = id.as_str().to_owned();
        let (unit, minimum, maximum, enum_values, runtime_effect) = match objective
            .default_condition()
        {
            level_definition::ObjectiveCondition::ReachLocation(_) => (
                Some("level_tile".to_owned()),
                Some(0),
                Some(i64::from(u8::MAX)),
                Vec::new(),
                "new runtime generations evaluate the authored location in the ordinary objective pass",
            ),
            level_definition::ObjectiveCondition::CastleManaPercent(_) => (
                Some("percent".to_owned()),
                Some(0),
                Some(100),
                Vec::new(),
                "new runtime generations evaluate the authored Castle-mana threshold in the ordinary objective pass",
            ),
            level_definition::ObjectiveCondition::NoLivingCreatures(_) => (
                Some("creature_kind".to_owned()),
                None,
                None,
                self.level_definition
                    .as_ref()?
                    .no_living_creatures_objective_kinds()
                    .into_iter()
                    .map(|kind| kind.as_str().to_owned())
                    .collect(),
                "new runtime generations evaluate the authored package-present creature absence in the ordinary objective pass",
            ),
            _ => (
                None,
                None,
                None,
                Vec::new(),
                "read-only package objective condition",
            ),
        };
        Some(SemanticDescriptor {
            id: id.clone(),
            descriptor_version: 1,
            kind: DescriptorKind::Objective,
            value_type: ValueType::ObjectiveCondition,
            unit,
            default_value: SemanticValue::ObjectiveCondition(objective.default_condition()),
            minimum,
            maximum,
            enum_values,
            access: if writable {
                Access::Writable
            } else {
                Access::ReadOnly
            },
            mutation_timing: MutationTiming::SessionReset,
            determinism_role: "immutable package objective authoring target".to_owned(),
            dependencies: vec![
                objective.id().as_str().to_owned(),
                "ordered runtime objective-stage binding".to_owned(),
            ],
            side_effects: if writable {
                vec![
                    "already-open sessions remain unchanged until explicit reopen/reset".to_owned(),
                    runtime_effect.to_owned(),
                ]
            } else {
                Vec::new()
            },
            scope: format!("stable level objective {}", objective.id().as_str()),
            provenance_origin: "package semantic level definition".to_owned(),
            provenance_content_hash: self.package_graph_hash.clone(),
            temporal_contract: Some(TemporalContract {
                dispatch_phase: "definition_publication".to_owned(),
                dispatch_key: target_id.clone(),
                ordinary_producer: "validated PutObjective candidate".to_owned(),
                handler: "guarded atomic definition compare-and-swap".to_owned(),
                same_pass_epilogue:
                    "receipt/read-back commit; already-open sessions remain unchanged".to_owned(),
                following_pass_cleanup: "explicit reset/reopen creates a new runtime generation; the ordinary objective evaluator reads the authored condition"
                    .to_owned(),
                authoritative_reads: vec![target_id],
                authoritative_writes: vec!["immutable LevelDefinition objective condition".to_owned()],
                rng_and_event_effects:
                    "none during publication; gameplay effects remain owned by the objective scheduler"
                        .to_owned(),
            }),
            summary: "Condition of one stable semantic level objective.".to_owned(),
        })
    }

    /// Describe every objective-condition target in stable stage order.
    #[must_use]
    pub fn level_objective_condition_descriptors(&self) -> Vec<SemanticDescriptor> {
        self.level_definition
            .as_ref()
            .into_iter()
            .flat_map(level_definition::LevelDefinition::objectives)
            .filter_map(|objective| {
                SemanticId::parse(&format!("{}.condition", objective.id().as_str()))
                    .ok()
                    .and_then(|id| self.level_objective_condition_descriptor(&id))
            })
            .collect()
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

    /// Verify the complete persisted schema, hash, capability, artifact, and
    /// semantic-flow identity without mutating this definition.
    ///
    /// # Errors
    ///
    /// Returns the first typed identity or schema violation.
    pub fn verify_identity(&self) -> Result<()> {
        if self.schema != GAME_DEFINITION_SCHEMA {
            return Err(DefinitionError::new(
                DefinitionErrorCode::InvalidSchema,
                "definition.schema",
                "unsupported persisted definition schema",
            ));
        }
        if self.runtime_flow.is_some() && self.level_definition.is_none() {
            return Err(DefinitionError::new(
                DefinitionErrorCode::InvalidSchema,
                "definition.runtime_flow",
                "runtime flow identity requires a playable level definition",
            ));
        }
        if let Some(runtime_flow) = &self.runtime_flow {
            runtime_flow.verify()?;
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
        let mut definition_payload = json!({
            "schema": self.schema,
            "revision": self.revision,
            "package_id": self.package_id,
            "package_version": self.package_version,
            "package_graph_hash": self.package_graph_hash,
            "artifact_graph_hash": self.artifact_graph_hash,
            "required_capabilities": self.required_capabilities.iter().map(RequiredCapability::to_json).collect::<Vec<_>>(),
            "values": self.values.iter().map(|(id, value)| json!({"id": id.as_str(), "value": value.to_json()})).collect::<Vec<_>>(),
        });
        if let Some(level_definition) = &self.level_definition {
            definition_payload
                .as_object_mut()
                .expect("definition identity payload is an object")
                .insert(
                    "level_definition".to_owned(),
                    serde_json::to_value(level_definition)
                        .expect("semantic level definition is serializable"),
                );
        }
        if let Some(runtime_flow) = &self.runtime_flow {
            definition_payload
                .as_object_mut()
                .expect("definition identity payload is an object")
                .insert(
                    "runtime_flow".to_owned(),
                    serde_json::to_value(runtime_flow).expect("runtime flow is serializable"),
                );
        }
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
