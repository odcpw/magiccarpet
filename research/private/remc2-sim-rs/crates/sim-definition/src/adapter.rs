//! Narrow bridge for private engine adapters.
//!
//! This module is public only because workspace crate boundaries require it.
//! `sim-core` deliberately does not re-export it as part of its public API.

use std::path::Path;

use crate::{
    ArtifactKind, ArtifactNode, DefinitionError, DefinitionErrorCode, GameDefinition, GamePackage,
    PackageProvenance, RequiredCapability, SemanticId, SemanticValue,
    level_definition::{
        ActivationGroupId, LevelDefinition, LevelEntityDefinition, LevelEntityId, LevelEntityKind,
        LevelObjectiveDefinition, ObjectiveCondition, TilePosition,
    },
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

#[must_use]
pub fn entity_id(kind: LevelEntityKind, ordinal: u16) -> LevelEntityId {
    match kind {
        LevelEntityKind::Creature(kind) => LevelEntityId::creature(kind, ordinal),
        LevelEntityKind::SpellPickup(kind) => LevelEntityId::spell_pickup(kind, ordinal),
        LevelEntityKind::Scenery(kind) => LevelEntityId::scenery(kind, ordinal),
        LevelEntityKind::Item(kind) => LevelEntityId::item(kind, ordinal),
        LevelEntityKind::Building(kind) => LevelEntityId::building(kind, ordinal),
        LevelEntityKind::Switch(kind) => LevelEntityId::switch(kind, ordinal),
        LevelEntityKind::Mechanic(kind) => LevelEntityId::mechanic(kind, ordinal),
    }
}

#[must_use]
pub const fn entity_definition(
    id: LevelEntityId,
    kind: LevelEntityKind,
    position: TilePosition,
    activation_group: Option<ActivationGroupId>,
) -> LevelEntityDefinition {
    LevelEntityDefinition::new(id, kind, position, activation_group)
}

#[must_use]
pub const fn objective_definition(condition: ObjectiveCondition) -> LevelObjectiveDefinition {
    LevelObjectiveDefinition::new(condition)
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
