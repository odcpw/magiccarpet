//! Closed, agent-facing game-definition operations.
//!
//! The operation vocabulary names only engine-declared semantic concepts.
//! Raw source selectors and arbitrary semantic paths are intentionally not
//! representable here. More structural operations join this enum only with a
//! stable author identity and a real runtime consumer.

use crate::level_definition::{LevelEntityKind, LevelObjectiveId, ObjectiveCondition};
use crate::{
    ARCHER_MAX_LIFE_CAPABILITY_ID, Access, BEE_MAX_LIFE_CAPABILITY_ID,
    CREATURE_POLICY_CAPABILITY_ID, DRAGON_MAX_LIFE_CAPABILITY_ID, DefinitionError,
    DefinitionErrorCode, FIREBALL_MANA_COST_CAPABILITY_ID, FIREFLY_MAX_LIFE_CAPABILITY_ID,
    GOAT_MAX_LIFE_CAPABILITY_ID, GameDefinition, SKELETON_MAX_LIFE_CAPABILITY_ID, SemanticId,
    SemanticValue, TRADER_MAX_LIFE_CAPABILITY_ID, VILLAGER_MAX_LIFE_CAPABILITY_ID,
    creature_max_life_capability_id,
    level_definition::{LevelEntityId, TilePosition, is_private_source_identity_segment},
};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};

/// Stable identifier of an artifact already retained by the immutable package.
/// Source selectors are deliberately not valid artifact vocabulary.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ArtifactId(SemanticId);

impl ArtifactId {
    /// Parse one semantic retained-artifact identifier.
    ///
    /// # Errors
    ///
    /// Rejects non-semantic identifiers and source-selector vocabulary.
    pub fn parse(value: &str) -> crate::Result<Self> {
        let semantic = SemanticId::parse(value)?;
        let source_shaped = semantic.as_str().split('.').any(is_source_selector_segment);
        if source_shaped {
            return Err(DefinitionError::new(
                DefinitionErrorCode::InvalidSemanticId,
                value,
                "artifact ID must not expose source class/model/action/selector identity",
            ));
        }
        Ok(Self(semantic))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl<'de> Deserialize<'de> for ArtifactId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegerParameter {
    #[serde(rename = "flight.player.max_forward_speed")]
    PlayerMaxForwardSpeed,
    #[serde(rename = "spell.fireball.mana_cost")]
    FireballManaCost,
}

impl IntegerParameter {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PlayerMaxForwardSpeed => "flight.player.max_forward_speed",
            Self::FireballManaCost => "spell.fireball.mana_cost",
        }
    }

    pub(crate) fn semantic_id(self) -> SemanticId {
        SemanticId::parse(self.as_str()).expect("static integer parameter ID is valid")
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionParameter {
    #[serde(rename = "ai.enemy.default.policy")]
    EnemyDefaultPolicy,
}

impl OptionParameter {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        "ai.enemy.default.policy"
    }

    pub(crate) fn semantic_id(self) -> SemanticId {
        SemanticId::parse(self.as_str()).expect("static option parameter ID is valid")
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationAssetSlot {
    #[serde(rename = "presentation.spell.fireball.projectile_asset")]
    FireballProjectile,
}

impl PresentationAssetSlot {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        "presentation.spell.fireball.projectile_asset"
    }

    pub(crate) fn semantic_id(self) -> SemanticId {
        SemanticId::parse(self.as_str()).expect("static presentation slot ID is valid")
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum DefinitionOperation {
    SetInteger {
        parameter: IntegerParameter,
        value: i64,
        #[serde(default)]
        expected_value: Option<i64>,
    },
    SelectOption {
        parameter: OptionParameter,
        option: String,
        #[serde(default)]
        expected_option: Option<String>,
    },
    BindAsset {
        slot: PresentationAssetSlot,
        artifact: ArtifactId,
        #[serde(default)]
        expected_artifact: Option<ArtifactId>,
    },
    MoveEntity {
        entity: LevelEntityId,
        position: TilePosition,
        #[serde(default)]
        expected_position: Option<TilePosition>,
    },
    SetEntityMaxLife {
        entity: LevelEntityId,
        max_life: u32,
        #[serde(default)]
        expected_max_life: Option<u32>,
    },
    PutObjective {
        objective: LevelObjectiveId,
        condition: ObjectiveCondition,
        #[serde(default)]
        expected_condition: Option<ObjectiveCondition>,
    },
    SetMechanicCountdown {
        entity: LevelEntityId,
        arming_ticks: u32,
        #[serde(default)]
        expected_arming_ticks: Option<u32>,
    },
}

impl DefinitionOperation {
    /// Machine-readable closed authoring vocabulary for discovery clients.
    #[must_use]
    pub fn schema_json() -> Value {
        json!({
            "deny_unknown_fields": true,
            "tag_field": "operation",
            "variants": [
                {
                    "operation": "set_integer",
                    "required_fields": ["parameter", "value"],
                    "optional_fields": ["expected_value"],
                    "parameter_ids": [
                        IntegerParameter::PlayerMaxForwardSpeed.as_str(),
                        IntegerParameter::FireballManaCost.as_str(),
                    ],
                    "parameter_requirements": {
                        IntegerParameter::FireballManaCost.as_str(): {
                            "required_capability": {
                                "id": FIREBALL_MANA_COST_CAPABILITY_ID,
                                "exact_version": 1,
                            },
                            "writable_when": "the active definition requires this exact capability",
                        },
                    },
                },
                {
                    "operation": "select_option",
                    "required_fields": ["parameter", "option"],
                    "optional_fields": ["expected_option"],
                    "parameter_ids": [OptionParameter::EnemyDefaultPolicy.as_str()],
                    "required_capability": {
                        "id": CREATURE_POLICY_CAPABILITY_ID,
                        "exact_version": 1,
                    },
                    "writable_when": "the active definition requires this exact capability and contains package-present Goats",
                    "mutation_timing": "session_reset",
                    "describe": "game describe --id ai.enemy.default.policy",
                    "read_back": "game get --id ai.enemy.default.policy",
                },
                {
                    "operation": "bind_asset",
                    "required_fields": ["slot", "artifact"],
                    "optional_fields": ["expected_artifact"],
                    "slot_ids": [PresentationAssetSlot::FireballProjectile.as_str()],
                    "required_capability": {
                        "id": "engine.presentation.indexed_asset",
                        "exact_version": 1,
                    },
                    "artifact_ids": "target descriptor -> compatible_artifacts[].artifact_id",
                    "expected_artifact": "target descriptor -> effective_artifact_id",
                    "writable_when": "the active definition retains at least two compatible typed artifacts",
                    "mutation_timing": "package_reload",
                    "describe": "game describe --id presentation.spell.fireball.projectile_asset",
                    "read_back": "game get --id presentation.spell.fireball.projectile_asset",
                },
                {
                    "operation": "move_entity",
                    "required_fields": ["entity", "position"],
                    "optional_fields": ["expected_position"],
                    "entity_ids": "game entities -> data.entities[].id",
                    "target_id": "<entity-id>.position",
                    "position": {
                        "shape": {"x": "integer", "y": "integer"},
                        "unit": "level_tile",
                        "bounds": {
                            "x": {"minimum": 0, "maximum": 255},
                            "y": {"minimum": 0, "maximum": 255},
                        },
                    },
                    "expected_position": "same shape, unit, and bounds as position",
                    "writable_when": "entity.position_access == session_reset",
                    "mutation_timing": "session_reset",
                    "describe": "game describe --id <entity-id>.position",
                    "read_back": "game get --id <entity-id>.position",
                },
                {
                    "operation": "set_entity_max_life",
                    "required_fields": ["entity", "max_life"],
                    "optional_fields": ["expected_max_life"],
                    "entity_ids": "game entities -> data.entities[].id",
                    "target_id": "<entity-id>.max_life",
                    "max_life": {
                        "type": "integer",
                        "unit": "life_points",
                        "minimum": 1,
                        "maximum": "target descriptor -> maximum",
                    },
                    "expected_max_life": "same type, unit, and bounds as max_life",
                    "required_capability_by_creature_kind": {
                        "archer": {
                            "id": ARCHER_MAX_LIFE_CAPABILITY_ID,
                            "exact_version": 1,
                        },
                        "bee": {
                            "id": BEE_MAX_LIFE_CAPABILITY_ID,
                            "exact_version": 1,
                        },
                        "dragon": {
                            "id": DRAGON_MAX_LIFE_CAPABILITY_ID,
                            "exact_version": 1,
                        },
                        "firefly": {
                            "id": FIREFLY_MAX_LIFE_CAPABILITY_ID,
                            "exact_version": 1,
                        },
                        "goat": {
                            "id": GOAT_MAX_LIFE_CAPABILITY_ID,
                            "exact_version": 1,
                        },
                        "skeleton": {
                            "id": SKELETON_MAX_LIFE_CAPABILITY_ID,
                            "exact_version": 1,
                        },
                        "trader": {
                            "id": TRADER_MAX_LIFE_CAPABILITY_ID,
                            "exact_version": 1,
                        },
                        "villager": {
                            "id": VILLAGER_MAX_LIFE_CAPABILITY_ID,
                            "exact_version": 1,
                        },
                    },
                    "writable_when": "entity carries a session-reset health configuration and its creature family has the exact compiled maximum-life capability",
                    "mutation_timing": "session_reset",
                    "describe": "game describe --id <entity-id>.max_life",
                    "read_back": "game get --id <entity-id>.max_life",
                },
                {
                    "operation": "put_objective",
                    "required_fields": ["objective", "condition"],
                    "optional_fields": ["expected_condition"],
                    "objective_ids": "game objectives -> data.objectives[].id",
                    "target_id": "<objective-id>.condition",
                    "condition": {
                        "shapes": [
                            {
                                "reach_location": {"x": "integer", "y": "integer"},
                                "unit": "level_tile",
                                "bounds": {
                                    "x": {"minimum": 0, "maximum": 255},
                                    "y": {"minimum": 0, "maximum": 255}
                                }
                            },
                            {
                                "castle_mana_percent": "integer",
                                "unit": "percent",
                                "minimum": 0,
                                "maximum": 100
                            },
                            {
                                "no_living_creatures": "semantic_creature_kind",
                                "unit": "creature_kind",
                                "values": "target descriptor -> bounds_or_enum.values"
                            }
                        ]
                    },
                    "expected_condition": "same shape, unit, and bounds as condition",
                    "writable_when": "objective.condition_access == session_reset",
                    "mutation_timing": "session_reset",
                    "describe": "game describe --id <objective-id>.condition",
                    "read_back": "game get --id <objective-id>.condition"
                },
                {
                    "operation": "set_mechanic_countdown",
                    "required_fields": ["entity", "arming_ticks"],
                    "optional_fields": ["expected_arming_ticks"],
                    "entity_ids": "game entities -> data.entities[].id",
                    "target_id": "<entity-id>.arming_ticks",
                    "arming_ticks": {
                        "type": "integer",
                        "unit": "entity_visits",
                        "minimum": 1,
                        "maximum": i32::MAX,
                    },
                    "expected_arming_ticks": "same type, unit, and bounds as arming_ticks",
                    "writable_when": "entity carries a session-reset absence-countdown configuration",
                    "mutation_timing": "session_reset",
                    "describe": "game describe --id <entity-id>.arming_ticks",
                    "read_back": "game get --id <entity-id>.arming_ticks",
                },
            ],
        })
    }

    pub(crate) fn target_id(&self) -> SemanticId {
        match self {
            Self::SetInteger { parameter, .. } => parameter.semantic_id(),
            Self::SelectOption { parameter, .. } => parameter.semantic_id(),
            Self::BindAsset { slot, .. } => slot.semantic_id(),
            Self::MoveEntity { entity, .. } => {
                SemanticId::parse(&format!("{}.position", entity.as_str()))
                    .expect("validated entity ID plus position is a semantic target")
            }
            Self::SetEntityMaxLife { entity, .. } => {
                SemanticId::parse(&format!("{}.max_life", entity.as_str()))
                    .expect("validated entity ID plus max_life is a semantic target")
            }
            Self::PutObjective { objective, .. } => {
                SemanticId::parse(&format!("{}.condition", objective.as_str()))
                    .expect("validated objective ID plus condition is a semantic target")
            }
            Self::SetMechanicCountdown { entity, .. } => {
                SemanticId::parse(&format!("{}.arming_ticks", entity.as_str()))
                    .expect("validated entity ID plus arming_ticks is a semantic target")
            }
        }
    }

    pub(crate) fn resolve(&self, definition: &GameDefinition) -> crate::Result<ResolvedOperation> {
        let fireball_cost_is_bound = definition
            .required_capabilities()
            .iter()
            .any(|requirement| {
                requirement.id.as_str() == FIREBALL_MANA_COST_CAPABILITY_ID
                    && requirement.exact_version == 1
            });
        let is_fireball_cost_operation = matches!(
            self,
            Self::SetInteger {
                parameter: IntegerParameter::FireballManaCost,
                ..
            }
        );
        if is_fireball_cost_operation && !fireball_cost_is_bound {
            return Err(DefinitionError::new(
                DefinitionErrorCode::DependencyFailed,
                self.target_id().as_str(),
                "definition has no package-qualified Fireball mana-cost runtime consumer",
            ));
        }
        let policy_descriptor = matches!(self, Self::SelectOption { .. }).then(|| {
            crate::default_catalog().and_then(|catalog| {
                definition
                    .semantic_descriptor(
                        &catalog,
                        &OptionParameter::EnemyDefaultPolicy.semantic_id(),
                    )
                    .ok_or_else(|| {
                        DefinitionError::new(
                            DefinitionErrorCode::UnknownSemanticId,
                            self.target_id().as_str(),
                            "definition has no enemy-policy descriptor",
                        )
                    })
            })
        });
        if matches!(policy_descriptor.transpose()?, Some(descriptor) if descriptor.access != Access::Writable)
        {
            return Err(DefinitionError::new(
                DefinitionErrorCode::DependencyFailed,
                self.target_id().as_str(),
                "definition has no package-qualified Goat policy runtime consumer",
            ));
        }
        let asset_descriptor = matches!(self, Self::BindAsset { .. }).then(|| {
            crate::default_catalog().and_then(|catalog| {
                definition
                    .semantic_descriptor(
                        &catalog,
                        &PresentationAssetSlot::FireballProjectile.semantic_id(),
                    )
                    .ok_or_else(|| {
                        DefinitionError::new(
                            DefinitionErrorCode::UnknownSemanticId,
                            self.target_id().as_str(),
                            "definition has no presentation-asset descriptor",
                        )
                    })
            })
        });
        if matches!(asset_descriptor.transpose()?, Some(descriptor) if descriptor.access != Access::Writable)
        {
            return Err(DefinitionError::new(
                DefinitionErrorCode::DependencyFailed,
                self.target_id().as_str(),
                "definition has no package-qualified compatible presentation-asset choice",
            ));
        }
        let playable_operation_is_supported = matches!(
            self,
            Self::SetInteger {
                parameter: IntegerParameter::PlayerMaxForwardSpeed,
                ..
            } | Self::MoveEntity { .. }
                | Self::SetEntityMaxLife { .. }
                | Self::PutObjective { .. }
                | Self::SetMechanicCountdown { .. }
        ) || is_fireball_cost_operation
            || matches!(self, Self::BindAsset { .. } | Self::SelectOption { .. });
        if definition.level_definition().is_some() && !playable_operation_is_supported {
            return Err(DefinitionError::new(
                DefinitionErrorCode::DependencyFailed,
                self.target_id().as_str(),
                "playable campaign definitions have no runtime consumer for this operation yet",
            ));
        }
        if let Self::MoveEntity {
            entity,
            position,
            expected_position,
        } = self
        {
            let id = self.target_id();
            let descriptor = definition
                .level_entity_position_descriptor(&id)
                .ok_or_else(|| {
                    DefinitionError::new(
                        DefinitionErrorCode::UnknownSemanticId,
                        entity.as_str(),
                        "move names no entity in the active level definition",
                    )
                })?;
            if descriptor.access != Access::Writable {
                return Err(DefinitionError::new(
                    DefinitionErrorCode::DependencyFailed,
                    entity.as_str(),
                    "entity position has no runtime consumer in this package",
                ));
            }
            descriptor.validate_value(&SemanticValue::Position(*position), id.as_str())?;
            if let Some(expected_position) = expected_position {
                descriptor.validate_value(
                    &SemanticValue::Position(*expected_position),
                    "expected_position",
                )?;
            }
            return Ok(ResolvedOperation::EntityMove {
                id,
                entity: entity.clone(),
                position: *position,
                expected_position: *expected_position,
            });
        }
        if let Self::SetEntityMaxLife {
            entity,
            max_life,
            expected_max_life,
        } = self
        {
            let id = self.target_id();
            let descriptor = definition
                .level_creature_max_life_descriptor(&id)
                .ok_or_else(|| {
                    DefinitionError::new(
                        DefinitionErrorCode::UnknownSemanticId,
                        entity.as_str(),
                        "maximum-life update names no package-bound creature health target",
                    )
                })?;
            if descriptor.access != Access::Writable {
                return Err(DefinitionError::new(
                    DefinitionErrorCode::DependencyFailed,
                    entity.as_str(),
                    "creature maximum life has no runtime consumer in this package",
                ));
            }
            let creature_kind = definition
                .level_definition()
                .and_then(|level| level.entity(entity))
                .and_then(|candidate| match candidate.kind() {
                    LevelEntityKind::Creature(kind) => Some(kind),
                    _ => None,
                })
                .ok_or_else(|| {
                    DefinitionError::new(
                        DefinitionErrorCode::DependencyFailed,
                        entity.as_str(),
                        "maximum-life target is not a package-bound creature",
                    )
                })?;
            let required_capability =
                creature_max_life_capability_id(creature_kind).ok_or_else(|| {
                    DefinitionError::new(
                        DefinitionErrorCode::DependencyFailed,
                        entity.as_str(),
                        "creature family has no compiled maximum-life consumer",
                    )
                })?;
            let has_capability = definition
                .required_capabilities()
                .iter()
                .any(|requirement| {
                    requirement.id.as_str() == required_capability && requirement.exact_version == 1
                });
            if !has_capability {
                return Err(DefinitionError::new(
                    DefinitionErrorCode::DependencyFailed,
                    entity.as_str(),
                    "definition lacks the exact creature-family maximum-life capability",
                ));
            }
            descriptor
                .validate_value(&SemanticValue::Integer(i64::from(*max_life)), id.as_str())?;
            if let Some(expected_max_life) = expected_max_life {
                descriptor.validate_value(
                    &SemanticValue::Integer(i64::from(*expected_max_life)),
                    "expected_max_life",
                )?;
            }
            return Ok(ResolvedOperation::EntityMaxLifeSet {
                id,
                entity: entity.clone(),
                max_life: *max_life,
                expected_max_life: *expected_max_life,
            });
        }
        if let Self::PutObjective {
            objective,
            condition,
            expected_condition,
        } = self
        {
            let id = self.target_id();
            let descriptor = definition
                .level_objective_condition_descriptor(&id)
                .ok_or_else(|| {
                    DefinitionError::new(
                        DefinitionErrorCode::UnknownSemanticId,
                        objective.as_str(),
                        "put names no objective in the active level definition",
                    )
                })?;
            if descriptor.access != Access::Writable {
                return Err(DefinitionError::new(
                    DefinitionErrorCode::DependencyFailed,
                    objective.as_str(),
                    "objective condition has no authoring consumer in this package",
                ));
            }
            descriptor.validate_value(
                &SemanticValue::ObjectiveCondition(condition.clone()),
                id.as_str(),
            )?;
            if let Some(expected_condition) = expected_condition {
                descriptor.validate_value(
                    &SemanticValue::ObjectiveCondition(expected_condition.clone()),
                    "expected_condition",
                )?;
            }
            return Ok(ResolvedOperation::ObjectivePut {
                id,
                objective: objective.clone(),
                condition: condition.clone(),
                expected_condition: expected_condition.clone(),
            });
        }
        if let Self::SetMechanicCountdown {
            entity,
            arming_ticks,
            expected_arming_ticks,
        } = self
        {
            let id = self.target_id();
            let descriptor = definition
                .level_countdown_arming_ticks_descriptor(&id)
                .ok_or_else(|| {
                    DefinitionError::new(
                        DefinitionErrorCode::UnknownSemanticId,
                        entity.as_str(),
                        "countdown update names no absence mechanic in the active level definition",
                    )
                })?;
            if descriptor.access != Access::Writable {
                return Err(DefinitionError::new(
                    DefinitionErrorCode::DependencyFailed,
                    entity.as_str(),
                    "absence-countdown delay has no runtime consumer in this package",
                ));
            }
            descriptor.validate_value(
                &SemanticValue::Integer(i64::from(*arming_ticks)),
                id.as_str(),
            )?;
            if let Some(expected_arming_ticks) = expected_arming_ticks {
                descriptor.validate_value(
                    &SemanticValue::Integer(i64::from(*expected_arming_ticks)),
                    "expected_arming_ticks",
                )?;
            }
            return Ok(ResolvedOperation::MechanicCountdownSet {
                id,
                entity: entity.clone(),
                arming_ticks: *arming_ticks,
                expected_arming_ticks: *expected_arming_ticks,
            });
        }
        let (id, value, expected_value) = match self {
            Self::SetInteger {
                parameter,
                value,
                expected_value,
            } => (
                parameter.semantic_id(),
                SemanticValue::Integer(*value),
                expected_value.map(SemanticValue::Integer),
            ),
            Self::SelectOption {
                parameter,
                option,
                expected_option,
            } => (
                parameter.semantic_id(),
                SemanticValue::Enum(option.clone()),
                expected_option.clone().map(SemanticValue::Enum),
            ),
            Self::BindAsset {
                slot,
                artifact,
                expected_artifact,
            } => (
                slot.semantic_id(),
                SemanticValue::Reference(content_address(definition, artifact)?),
                expected_artifact
                    .as_ref()
                    .map(|artifact| content_address(definition, artifact))
                    .transpose()?
                    .map(SemanticValue::Reference),
            ),
            Self::MoveEntity { .. } => unreachable!("entity moves return above"),
            Self::SetEntityMaxLife { .. } => {
                unreachable!("entity maximum-life updates return above")
            }
            Self::PutObjective { .. } => unreachable!("objective puts return above"),
            Self::SetMechanicCountdown { .. } => {
                unreachable!("mechanic countdown updates return above")
            }
        };
        Ok(ResolvedOperation::CatalogReplacement {
            id,
            value,
            expected_value,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResolvedOperation {
    CatalogReplacement {
        id: SemanticId,
        value: SemanticValue,
        expected_value: Option<SemanticValue>,
    },
    EntityMove {
        id: SemanticId,
        entity: LevelEntityId,
        position: TilePosition,
        expected_position: Option<TilePosition>,
    },
    EntityMaxLifeSet {
        id: SemanticId,
        entity: LevelEntityId,
        max_life: u32,
        expected_max_life: Option<u32>,
    },
    ObjectivePut {
        id: SemanticId,
        objective: LevelObjectiveId,
        condition: ObjectiveCondition,
        expected_condition: Option<ObjectiveCondition>,
    },
    MechanicCountdownSet {
        id: SemanticId,
        entity: LevelEntityId,
        arming_ticks: u32,
        expected_arming_ticks: Option<u32>,
    },
}

fn content_address(definition: &GameDefinition, artifact: &ArtifactId) -> crate::Result<String> {
    definition
        .artifacts()
        .iter()
        .find(|candidate| candidate.id == artifact.as_str())
        .map(|candidate| format!("sha256:{}", candidate.sha256))
        .ok_or_else(|| {
            DefinitionError::new(
                DefinitionErrorCode::DependencyFailed,
                artifact.as_str(),
                "asset binding names no retained package artifact",
            )
        })
}

fn is_source_selector_segment(segment: &str) -> bool {
    is_private_source_identity_segment(segment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{
        build_level_definition, creature_entity_definition, entity_definition, stable_entity_id,
        with_level_definition,
    };
    use crate::level_definition::{
        CreatureKind, LevelEntityKind, LevelEntityParameterAccess, LevelEntityPositionAccess,
        SceneryKind, TilePosition,
    };
    use crate::{GamePackage, default_catalog, engine_capabilities};
    use std::path::PathBuf;

    #[test]
    fn operation_json_is_closed_against_raw_selectors_and_unimplemented_structure() {
        for value in [
            serde_json::json!({
                "operation": "set_integer",
                "parameter": "class5.archer",
                "value": 16
            }),
            serde_json::json!({
                "operation": "set_integer",
                "parameter": "flight.player.max_forward_speed",
                "value": 16,
                "source_slot": 111
            }),
            serde_json::json!({
                "operation": "move_entity",
                "entity": "creature.goat.1",
                "x": 10,
                "y": 20
            }),
            serde_json::json!({
                "operation": "bind_asset",
                "slot": "presentation.spell.fireball.projectile_asset",
                "artifact": "class5.archer"
            }),
            serde_json::json!({
                "operation": "bind_asset",
                "slot": "presentation.spell.fireball.projectile_asset",
                "artifact": "action17.firefly"
            }),
            serde_json::json!({
                "operation": "bind_asset",
                "slot": "presentation.spell.fireball.projectile_asset",
                "artifact": "asset.source48"
            }),
            serde_json::json!({
                "operation": "bind_asset",
                "slot": "presentation.spell.fireball.projectile_asset",
                "artifact": "asset.raw48"
            }),
            serde_json::json!({
                "operation": "bind_asset",
                "slot": "presentation.spell.fireball.projectile_asset",
                "artifact": "asset.level_1"
            }),
            serde_json::json!({
                "operation": "put_objective",
                "objective": "objective.source48",
                "condition": {"reach_location": {"x": 10, "y": 20}}
            }),
            serde_json::json!({
                "operation": "put_objective",
                "objective": "objective.reach.sight_beacon",
                "condition": {"reach_location": {"x": 10, "y": 20}},
                "source_slot": 48
            }),
        ] {
            assert!(serde_json::from_value::<DefinitionOperation>(value).is_err());
        }
    }

    #[test]
    fn goat_policy_requires_a_package_present_goat_not_only_the_compiled_capability() {
        let root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/custom-synthetic-v1");
        let catalog = default_catalog().unwrap();
        let definition = GameDefinition::from_package(
            GamePackage::load(root).unwrap(),
            &engine_capabilities().unwrap(),
            &catalog,
        )
        .unwrap();
        let definition = with_level_definition(
            definition,
            build_level_definition(
                vec![entity_definition(
                    stable_entity_id("scenery.policy_witness_tree").unwrap(),
                    LevelEntityKind::Scenery(SceneryKind::Tree),
                    TilePosition { x: 1, y: 1 },
                    None,
                )],
                Vec::new(),
                Vec::new(),
            )
            .unwrap(),
        );
        let id = OptionParameter::EnemyDefaultPolicy.semantic_id();
        assert_eq!(
            definition
                .semantic_descriptor(&catalog, &id)
                .unwrap()
                .access,
            Access::ReadOnly
        );
        let operation = DefinitionOperation::SelectOption {
            parameter: OptionParameter::EnemyDefaultPolicy,
            option: "vigilant_goat_v1".to_owned(),
            expected_option: Some("classic_goat_v1".to_owned()),
        };
        let error = operation.resolve(&definition).unwrap_err();
        assert_eq!(error.code, DefinitionErrorCode::DependencyFailed);
    }

    #[test]
    fn goat_max_life_requires_the_exact_package_capability_not_only_a_writable_shape() {
        let root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/custom-synthetic-v1");
        let catalog = default_catalog().unwrap();
        let definition = GameDefinition::from_package(
            GamePackage::load(root).unwrap(),
            &engine_capabilities().unwrap(),
            &catalog,
        )
        .unwrap();
        let goat = stable_entity_id("creature.goat.synthetic").unwrap();
        let definition = with_level_definition(
            definition,
            build_level_definition(
                vec![creature_entity_definition(
                    goat.clone(),
                    CreatureKind::Goat,
                    TilePosition { x: 1, y: 1 },
                    None,
                    LevelEntityPositionAccess::SessionReset,
                    600,
                    LevelEntityParameterAccess::SessionReset,
                )],
                Vec::new(),
                Vec::new(),
            )
            .unwrap(),
        );
        let target = SemanticId::parse("creature.goat.synthetic.max_life").unwrap();
        assert_eq!(
            definition
                .level_creature_max_life_descriptor(&target)
                .unwrap()
                .access,
            Access::ReadOnly
        );
        let operation = DefinitionOperation::SetEntityMaxLife {
            entity: goat,
            max_life: 1_200,
            expected_max_life: Some(600),
        };
        let error = operation.resolve(&definition).unwrap_err();
        assert_eq!(error.code, DefinitionErrorCode::DependencyFailed);
    }
}
