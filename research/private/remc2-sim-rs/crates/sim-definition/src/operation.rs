//! Closed, agent-facing game-definition operations.
//!
//! The operation vocabulary names only engine-declared semantic concepts.
//! Raw source selectors and arbitrary semantic paths are intentionally not
//! representable here. More structural operations join this enum only with a
//! stable author identity and a real runtime consumer.

use crate::{DefinitionError, DefinitionErrorCode, GameDefinition, SemanticId, SemanticValue};
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
                },
                {
                    "operation": "select_option",
                    "required_fields": ["parameter", "option"],
                    "optional_fields": ["expected_option"],
                    "parameter_ids": [OptionParameter::EnemyDefaultPolicy.as_str()],
                },
                {
                    "operation": "bind_asset",
                    "required_fields": ["slot", "artifact"],
                    "optional_fields": ["expected_artifact"],
                    "slot_ids": [PresentationAssetSlot::FireballProjectile.as_str()],
                },
            ],
        })
    }

    pub(crate) fn target_id(&self) -> SemanticId {
        match self {
            Self::SetInteger { parameter, .. } => parameter.semantic_id(),
            Self::SelectOption { parameter, .. } => parameter.semantic_id(),
            Self::BindAsset { slot, .. } => slot.semantic_id(),
        }
    }

    pub(crate) fn resolve(&self, definition: &GameDefinition) -> crate::Result<ResolvedOperation> {
        if definition.level_definition().is_some()
            && !matches!(
                self,
                Self::SetInteger {
                    parameter: IntegerParameter::PlayerMaxForwardSpeed,
                    ..
                }
            )
        {
            return Err(DefinitionError::new(
                DefinitionErrorCode::DependencyFailed,
                self.target_id().as_str(),
                "playable campaign definitions have no runtime consumer for this operation yet",
            ));
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
        };
        Ok(ResolvedOperation {
            id,
            value,
            expected_value,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedOperation {
    pub id: SemanticId,
    pub value: SemanticValue,
    pub expected_value: Option<SemanticValue>,
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
    if matches!(segment, "source" | "source_slot" | "raw") || segment.starts_with("source_") {
        return true;
    }
    ["class", "model", "action", "selector", "family", "slot"]
        .into_iter()
        .any(|prefix| {
            segment == prefix
                || segment.strip_prefix(prefix).is_some_and(|suffix| {
                    !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit())
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

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
        ] {
            assert!(serde_json::from_value::<DefinitionOperation>(value).is_err());
        }
    }
}
