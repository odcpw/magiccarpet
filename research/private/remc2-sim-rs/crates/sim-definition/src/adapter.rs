//! Narrow bridge for private engine adapters.
//!
//! This module is public only because workspace crate boundaries require it.
//! `sim-core` deliberately does not re-export it as part of its public API.

use std::{collections::BTreeMap, path::Path};

use crate::{
    ArtifactKind, ArtifactNode, DefinitionError, DefinitionErrorCode, GameDefinition, GamePackage,
    PackageProvenance, RequiredCapability, SemanticId, SemanticValue,
    level_definition::{
        AbsenceCountdownDefinition, ActivationGroupId, CreatureHealthDefinition, CreatureKind,
        ExperienceScrollDefinition, LevelDefinition, LevelEntityDefinition, LevelEntityId,
        LevelEntityKind, LevelEntityParameterAccess, LevelEntityPositionAccess,
        LevelObjectiveConditionAccess, LevelObjectiveDefinition, LevelObjectiveId,
        ObjectiveCondition, RivalWizardId, TilePosition,
    },
    runtime_flow::RuntimeFlowDefinition,
};

#[must_use]
pub fn definition_error(
    code: DefinitionErrorCode,
    path_or_id: impl Into<String>,
    message: impl Into<String>,
) -> DefinitionError {
    DefinitionError::new(code, path_or_id, message)
}

#[must_use]
pub fn with_level_definition(
    definition: GameDefinition,
    level_definition: LevelDefinition,
) -> GameDefinition {
    definition.with_level_definition(level_definition)
}

#[must_use]
pub fn with_level_and_runtime_flow(
    definition: GameDefinition,
    level_definition: LevelDefinition,
    runtime_flow: RuntimeFlowDefinition,
) -> GameDefinition {
    definition.with_level_and_runtime_flow(level_definition, runtime_flow)
}

pub fn with_replacement(
    definition: &GameDefinition,
    id: &SemanticId,
    value: SemanticValue,
    revision: u64,
) -> crate::Result<GameDefinition> {
    definition.with_replacement(id, value, revision)
}

pub fn reissue_at_revision(
    definition: &GameDefinition,
    revision: u64,
) -> crate::Result<GameDefinition> {
    definition.reissue_at_revision(revision)
}

#[must_use]
pub const fn activation_group_id(value: u16) -> ActivationGroupId {
    ActivationGroupId::new(value)
}

pub fn stable_entity_id(value: &str) -> crate::Result<LevelEntityId> {
    LevelEntityId::parse(value)
}

pub fn stable_objective_id(value: &str) -> crate::Result<LevelObjectiveId> {
    LevelObjectiveId::parse(value)
}

pub fn stable_rival_wizard_id(value: &str) -> crate::Result<RivalWizardId> {
    RivalWizardId::parse(value)
}

#[must_use]
pub const fn entity_definition(
    id: LevelEntityId,
    kind: LevelEntityKind,
    position: TilePosition,
    activation_group: Option<ActivationGroupId>,
) -> LevelEntityDefinition {
    LevelEntityDefinition::new(
        id,
        kind,
        position,
        activation_group,
        LevelEntityPositionAccess::ReadOnly,
        None,
        None,
        None,
    )
}

#[must_use]
pub const fn entity_definition_with_position_access(
    id: LevelEntityId,
    kind: LevelEntityKind,
    position: TilePosition,
    activation_group: Option<ActivationGroupId>,
    position_access: LevelEntityPositionAccess,
) -> LevelEntityDefinition {
    LevelEntityDefinition::new(
        id,
        kind,
        position,
        activation_group,
        position_access,
        None,
        None,
        None,
    )
}

#[must_use]
pub const fn creature_entity_definition(
    id: LevelEntityId,
    kind: CreatureKind,
    position: TilePosition,
    activation_group: Option<ActivationGroupId>,
    position_access: LevelEntityPositionAccess,
    max_life: u32,
    max_life_access: LevelEntityParameterAccess,
) -> LevelEntityDefinition {
    LevelEntityDefinition::new(
        id,
        LevelEntityKind::Creature(kind),
        position,
        activation_group,
        position_access,
        None,
        Some(CreatureHealthDefinition::new(max_life, max_life_access)),
        None,
    )
}

#[must_use]
pub const fn absence_countdown_definition(
    arming_ticks: u32,
    arming_ticks_access: LevelEntityParameterAccess,
) -> AbsenceCountdownDefinition {
    AbsenceCountdownDefinition::new(arming_ticks, arming_ticks_access)
}

#[must_use]
pub const fn absence_countdown_entity_definition(
    id: LevelEntityId,
    kind: LevelEntityKind,
    position: TilePosition,
    activation_group: Option<ActivationGroupId>,
    position_access: LevelEntityPositionAccess,
    countdown: AbsenceCountdownDefinition,
) -> LevelEntityDefinition {
    LevelEntityDefinition::new(
        id,
        kind,
        position,
        activation_group,
        position_access,
        Some(countdown),
        None,
        None,
    )
}

#[must_use]
pub const fn experience_scroll_entity_definition(
    id: LevelEntityId,
    position: TilePosition,
    activation_group: Option<ActivationGroupId>,
    position_access: LevelEntityPositionAccess,
    experience_award: u32,
    experience_award_access: LevelEntityParameterAccess,
) -> LevelEntityDefinition {
    LevelEntityDefinition::new(
        id,
        LevelEntityKind::Item(crate::level_definition::ItemKind::ExperienceScroll),
        position,
        activation_group,
        position_access,
        None,
        None,
        Some(ExperienceScrollDefinition::new(
            experience_award,
            experience_award_access,
        )),
    )
}

#[must_use]
pub fn objective_definition(
    id: LevelObjectiveId,
    condition: ObjectiveCondition,
    condition_access: LevelObjectiveConditionAccess,
) -> LevelObjectiveDefinition {
    LevelObjectiveDefinition::new(id, condition, condition_access)
}

pub fn build_level_definition(
    entities: Vec<LevelEntityDefinition>,
    objectives: Vec<LevelObjectiveDefinition>,
    activation_groups: Vec<ActivationGroupId>,
) -> Result<LevelDefinition, String> {
    LevelDefinition::new(entities, objectives, activation_groups)
}

pub fn validate_lower_sha256(value: &str, path: &str) -> crate::Result<()> {
    crate::lower_sha256(value, path)
}

pub fn read_confined_file(root: &Path, relative: &str) -> crate::Result<Vec<u8>> {
    crate::package::read_confined_file(root, relative)
}

pub fn artifact_node(
    id: &str,
    kind: ArtifactKind,
    logical_path: &str,
    content_hash: &str,
    byte_length: u64,
    dependencies: &[&str],
) -> crate::Result<ArtifactNode> {
    ArtifactNode::adapter(
        id,
        kind,
        logical_path,
        content_hash,
        byte_length,
        dependencies,
    )
}

pub fn owner_provenance(source_sha256: &str) -> crate::Result<PackageProvenance> {
    PackageProvenance::owner_derived(source_sha256)
}

pub fn game_package(
    package_id: &str,
    package_version: u64,
    required_capabilities: Vec<RequiredCapability>,
    artifacts: Vec<ArtifactNode>,
    bindings: Vec<(SemanticId, SemanticValue)>,
    provenance: PackageProvenance,
) -> crate::Result<GamePackage> {
    GamePackage::materialize(
        package_id,
        package_version,
        required_capabilities,
        artifacts,
        bindings,
        provenance,
    )
}

pub fn game_package_with_payloads(
    package_id: &str,
    package_version: u64,
    required_capabilities: Vec<RequiredCapability>,
    artifacts: Vec<ArtifactNode>,
    bindings: Vec<(SemanticId, SemanticValue)>,
    provenance: PackageProvenance,
    artifact_payloads: BTreeMap<String, Vec<u8>>,
) -> crate::Result<GamePackage> {
    GamePackage::materialize_with_payloads(
        package_id,
        package_version,
        required_capabilities,
        artifacts,
        bindings,
        provenance,
        artifact_payloads,
    )
}
