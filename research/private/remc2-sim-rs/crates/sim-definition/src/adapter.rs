//! Narrow bridge for private engine adapters.
//!
//! This module is public only because workspace crate boundaries require it.
//! `sim-core` deliberately does not re-export it as part of its public API.

use std::{collections::BTreeMap, path::Path};

use crate::{
    ArtifactKind, ArtifactNode, DefinitionError, DefinitionErrorCode, GameDefinition, GamePackage,
    PackageProvenance, RequiredCapability, SemanticId, SemanticValue,
    level_definition::{
        AbsenceCountdownDefinition, ActivationGroupId, CaveCorridorDefinition,
        CaveFeatureDefinition, CorridorOrientation, CreatureHealthDefinition, CreatureKind,
        EffectDefinition, EffectKind, ExperienceScrollDefinition, HouseConstructionDefinition,
        LevelDefinition, LevelEntityDefinition, LevelEntityId, LevelEntityKind,
        LevelEntityParameterAccess, LevelEntityPositionAccess, LevelObjectiveConditionAccess,
        LevelObjectiveDefinition, LevelObjectiveId, MechanicKind, ObjectiveCondition,
        RivalWizardId, SwitchOverlapDefinition, TilePosition,
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
        None,
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
pub const fn cave_corridor_definition(
    orientation: CorridorOrientation,
    extent: i16,
) -> CaveCorridorDefinition {
    CaveCorridorDefinition::new(orientation, extent)
}

#[must_use]
pub const fn cave_feature_entity_definition(
    id: LevelEntityId,
    position: TilePosition,
    activation_group: Option<ActivationGroupId>,
    position_access: LevelEntityPositionAccess,
    config: CaveFeatureDefinition,
) -> LevelEntityDefinition {
    LevelEntityDefinition::new(
        id,
        LevelEntityKind::Mechanic(MechanicKind::CaveFeature {
            kind: config.kind(),
        }),
        position,
        activation_group,
        position_access,
        None,
        None,
        None,
        None,
        None,
    )
    .with_cave_feature(config)
}

/// Pinned MC2 class defaults. Pure presentation effects retain their source
/// payload word, but their behavior never submits a damage operation.
#[must_use]
pub const fn mc2_effect_definition(kind: EffectKind) -> EffectDefinition {
    let (life, damage) = match kind {
        EffectKind::Impact => (8, 400),
        EffectKind::RadialExplosion => (1, 400),
        EffectKind::Expiry => (8, 100),
        EffectKind::ShortVisual => (7, 0),
        EffectKind::DormantGroundMarker => (100, 0),
        EffectKind::Ash => (8, 0),
        EffectKind::GroundFire => (240, 50),
        EffectKind::ImmediateVisual => (12, 100),
        EffectKind::Smoke | EffectKind::FadingSmoke => (17, 100),
        EffectKind::DarkSmoke => (28, 100),
        EffectKind::BurningDebris => (100, 200),
        EffectKind::Eruption => (10000, 200),
        EffectKind::EruptionSmoke => (240, 200),
        EffectKind::AmbientSmoke => (800, 100),
        EffectKind::PossessionContact => (8, 64000),
        EffectKind::FireSpheres => (80, 70),
    };
    EffectDefinition::new(kind, life, damage)
}

#[must_use]
pub const fn effect_entity_definition(
    id: LevelEntityId,
    position: TilePosition,
    activation_group: Option<ActivationGroupId>,
    position_access: LevelEntityPositionAccess,
    config: EffectDefinition,
) -> LevelEntityDefinition {
    LevelEntityDefinition::new(
        id,
        LevelEntityKind::Mechanic(MechanicKind::EnvironmentalEffect {
            kind: config.kind(),
        }),
        position,
        activation_group,
        position_access,
        None,
        None,
        None,
        None,
        None,
    )
    .with_effect(config)
}

#[must_use]
pub const fn cave_corridor_entity_definition(
    id: LevelEntityId,
    position: TilePosition,
    activation_group: Option<ActivationGroupId>,
    position_access: LevelEntityPositionAccess,
    config: CaveCorridorDefinition,
) -> LevelEntityDefinition {
    LevelEntityDefinition::new(
        id,
        LevelEntityKind::Mechanic(MechanicKind::CaveCorridor),
        position,
        activation_group,
        position_access,
        None,
        None,
        None,
        None,
        None,
    )
    .with_cave_corridor(config)
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
        None,
        None,
    )
}

/// Bind an activation-family rule and its live overlap/countdown properties.
/// Level validation rejects missing or incompatible configuration by kind.
#[must_use]
pub const fn activation_switch_entity_definition(
    id: LevelEntityId,
    kind: crate::level_definition::SwitchKind,
    position: TilePosition,
    activation_group: Option<ActivationGroupId>,
    position_access: LevelEntityPositionAccess,
    overlap_extent_tiles: Option<u16>,
    countdown: Option<AbsenceCountdownDefinition>,
) -> LevelEntityDefinition {
    let overlap = match overlap_extent_tiles {
        Some(extent) => Some(SwitchOverlapDefinition::new(
            extent,
            LevelEntityParameterAccess::SessionReset,
        )),
        None => None,
    };
    LevelEntityDefinition::new(
        id,
        LevelEntityKind::Switch(kind),
        position,
        activation_group,
        position_access,
        countdown,
        None,
        None,
        None,
        overlap,
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
        None,
        None,
    )
}

#[must_use]
pub const fn house_construction_entity_definition(
    id: LevelEntityId,
    position: TilePosition,
    activation_group: ActivationGroupId,
    position_access: LevelEntityPositionAccess,
    life_mana_scalar: u16,
    life_mana_scalar_access: LevelEntityParameterAccess,
    construction_visits: u16,
    construction_visits_access: LevelEntityParameterAccess,
) -> LevelEntityDefinition {
    LevelEntityDefinition::new(
        id,
        LevelEntityKind::Building(crate::level_definition::BuildingKind::House),
        position,
        Some(activation_group),
        position_access,
        None,
        None,
        None,
        Some(HouseConstructionDefinition::new(
            life_mana_scalar,
            life_mana_scalar_access,
            construction_visits,
            construction_visits_access,
        )),
        None,
    )
}

#[must_use]
pub const fn exit_sequence_trigger_entity_definition(
    id: LevelEntityId,
    position: TilePosition,
    activation_group: ActivationGroupId,
    position_access: LevelEntityPositionAccess,
    overlap_extent_tiles: u16,
    overlap_extent_tiles_access: LevelEntityParameterAccess,
) -> LevelEntityDefinition {
    LevelEntityDefinition::new(
        id,
        LevelEntityKind::Switch(crate::level_definition::SwitchKind::ExitSequenceTrigger),
        position,
        Some(activation_group),
        position_access,
        None,
        None,
        None,
        None,
        Some(SwitchOverlapDefinition::new(
            overlap_extent_tiles,
            overlap_extent_tiles_access,
        )),
    )
}

#[must_use]
pub const fn area_entry_trigger_entity_definition(
    id: LevelEntityId,
    position: TilePosition,
    activation_group: Option<ActivationGroupId>,
    position_access: LevelEntityPositionAccess,
    overlap_extent_tiles: u16,
    overlap_extent_tiles_access: LevelEntityParameterAccess,
) -> LevelEntityDefinition {
    LevelEntityDefinition::new(
        id,
        LevelEntityKind::Switch(crate::level_definition::SwitchKind::AreaEntryTrigger),
        position,
        activation_group,
        position_access,
        None,
        None,
        None,
        None,
        Some(SwitchOverlapDefinition::new(
            overlap_extent_tiles,
            overlap_extent_tiles_access,
        )),
    )
}

#[must_use]
pub const fn area_exit_trigger_entity_definition(
    id: LevelEntityId,
    position: TilePosition,
    activation_group: Option<ActivationGroupId>,
    position_access: LevelEntityPositionAccess,
    overlap_extent_tiles: u16,
    overlap_extent_tiles_access: LevelEntityParameterAccess,
) -> LevelEntityDefinition {
    LevelEntityDefinition::new(
        id,
        LevelEntityKind::Switch(crate::level_definition::SwitchKind::AreaExitTrigger),
        position,
        activation_group,
        position_access,
        None,
        None,
        None,
        None,
        Some(SwitchOverlapDefinition::new(
            overlap_extent_tiles,
            overlap_extent_tiles_access,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn house_construction_preserves_validated_position_access() {
        for access in [
            LevelEntityPositionAccess::SessionReset,
            LevelEntityPositionAccess::ReadOnly,
        ] {
            let entity = house_construction_entity_definition(
                stable_entity_id("building.house.at_156_215").unwrap(),
                TilePosition { x: 156, y: 215 },
                activation_group_id(13),
                access,
                16,
                LevelEntityParameterAccess::SessionReset,
                32,
                LevelEntityParameterAccess::SessionReset,
            );
            assert_eq!(entity.position_access(), access);
            assert_eq!(entity.position(), TilePosition { x: 156, y: 215 });
            assert_eq!(entity.activation_group(), Some(activation_group_id(13)));
            let construction = entity.house_construction().unwrap();
            assert_eq!(construction.life_mana_scalar(), 16);
            assert_eq!(construction.construction_visits(), 32);
        }
    }
}
