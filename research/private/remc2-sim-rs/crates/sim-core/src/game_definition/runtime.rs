//! Definition-to-runtime bindings owned by the neutral Rust domain.

use super::{
    CREATURE_POLICY_CAPABILITY_ID, DefinitionErrorCode, FIREBALL_MANA_COST_CAPABILITY_ID,
    GameDefinition, Result, SemanticId, SemanticValue,
};
use crate::{FlightTuning, goat::GoatBehaviorConfig};
use sim_definition::adapter::definition_error;
use sim_definition::level_definition::{CreatureKind, LevelEntityKind};

pub const PLAYER_MAX_FORWARD_SPEED_ID: &str = "flight.player.max_forward_speed";
pub const FIREBALL_MANA_COST_ID: &str = "spell.fireball.mana_cost";
pub const ENEMY_DEFAULT_POLICY_ID: &str = "ai.enemy.default.policy";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefinitionFlightBinding {
    tuning: FlightTuning,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefinitionFireballBinding {
    mana_cost: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompiledGoatPolicy {
    Classic,
    Vigilant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefinitionGoatPolicyBinding {
    policy: CompiledGoatPolicy,
}

impl DefinitionGoatPolicyBinding {
    /// Resolve a package-present Goat policy consumed by the ordinary
    /// source-slot scheduler.
    ///
    /// # Errors
    ///
    /// Returns a typed definition error when a Goat-bearing package declares
    /// an absent, mistyped, or unknown compiled policy.
    pub fn from_definition(definition: &GameDefinition) -> Result<Option<Self>> {
        let capability_present = definition
            .required_capabilities()
            .iter()
            .any(|requirement| {
                requirement.id.as_str() == CREATURE_POLICY_CAPABILITY_ID
                    && requirement.exact_version == 1
            });
        let package_has_goats = definition.level_definition().is_some_and(|level| {
            level
                .entities()
                .iter()
                .any(|entity| entity.kind() == LevelEntityKind::Creature(CreatureKind::Goat))
        });
        if !capability_present || !package_has_goats {
            return Ok(None);
        }
        let id = SemanticId::parse(ENEMY_DEFAULT_POLICY_ID)?;
        let Some(SemanticValue::Enum(value)) = definition.value(&id) else {
            return Err(definition_error(
                DefinitionErrorCode::TypeMismatch,
                ENEMY_DEFAULT_POLICY_ID,
                "enemy default policy is not a compiled-policy enum",
            ));
        };
        let policy = match value.as_str() {
            "classic_goat_v1" => CompiledGoatPolicy::Classic,
            "vigilant_goat_v1" => CompiledGoatPolicy::Vigilant,
            _ => {
                return Err(definition_error(
                    DefinitionErrorCode::OutOfRange,
                    ENEMY_DEFAULT_POLICY_ID,
                    format!("unsupported compiled Goat policy {value}"),
                ));
            }
        };
        Ok(Some(Self { policy }))
    }

    /// Apply the selected compiled policy to the package-decoded source
    /// parameter row without replacing unrelated movement or terrain fields.
    #[must_use]
    pub fn apply(self, mut package_config: GoatBehaviorConfig) -> GoatBehaviorConfig {
        if self.policy == CompiledGoatPolicy::Vigilant {
            package_config.perception_cadence = 1;
            package_config.perception_radius = u16::MAX;
            package_config.perception_half_angle = 1_025;
            package_config.behavior_flags |= 0x10;
        }
        package_config
    }

    #[must_use]
    pub fn consumes(id: &SemanticId) -> bool {
        id.as_str() == ENEMY_DEFAULT_POLICY_ID
    }
}

impl DefinitionFireballBinding {
    /// Resolve a package-present local-player Fireball cost binding.
    ///
    /// # Errors
    ///
    /// Returns a typed definition error when the package declares the
    /// capability but the semantic value is absent, mistyped, or cannot fit
    /// the source-width runtime carrier.
    pub fn from_definition(definition: &GameDefinition) -> Result<Option<Self>> {
        let is_required = definition
            .required_capabilities()
            .iter()
            .any(|requirement| {
                requirement.id.as_str() == FIREBALL_MANA_COST_CAPABILITY_ID
                    && requirement.exact_version == 1
            });
        if !is_required {
            return Ok(None);
        }
        let id = SemanticId::parse(FIREBALL_MANA_COST_ID)?;
        let Some(SemanticValue::Integer(value)) = definition.value(&id) else {
            return Err(definition_error(
                DefinitionErrorCode::TypeMismatch,
                FIREBALL_MANA_COST_ID,
                "Fireball mana cost is not an integer",
            ));
        };
        let mana_cost = i32::try_from(*value).map_err(|_| {
            definition_error(
                DefinitionErrorCode::OutOfRange,
                FIREBALL_MANA_COST_ID,
                "Fireball mana cost does not fit the source i32 carrier",
            )
        })?;
        if mana_cost < 0 {
            return Err(definition_error(
                DefinitionErrorCode::OutOfRange,
                FIREBALL_MANA_COST_ID,
                "Fireball mana cost must be nonnegative",
            ));
        }
        Ok(Some(Self { mana_cost }))
    }

    #[must_use]
    pub const fn mana_cost(self) -> i32 {
        self.mana_cost
    }

    #[must_use]
    pub fn consumes(id: &SemanticId) -> bool {
        id.as_str() == FIREBALL_MANA_COST_ID
    }
}

impl DefinitionFlightBinding {
    /// Resolve the immutable flight values consumed by an ordinary campaign
    /// player callback.
    ///
    /// # Errors
    ///
    /// Returns a typed definition error when the required value is absent,
    /// has the wrong type, or cannot fit the source-width runtime carrier.
    pub fn from_definition(definition: &GameDefinition) -> Result<Self> {
        let id = SemanticId::parse(PLAYER_MAX_FORWARD_SPEED_ID)?;
        let Some(SemanticValue::Integer(value)) = definition.value(&id) else {
            return Err(definition_error(
                DefinitionErrorCode::TypeMismatch,
                PLAYER_MAX_FORWARD_SPEED_ID,
                "player maximum forward speed is not an integer",
            ));
        };
        let max_forward_speed = i16::try_from(*value).map_err(|_| {
            definition_error(
                DefinitionErrorCode::OutOfRange,
                PLAYER_MAX_FORWARD_SPEED_ID,
                "player maximum forward speed does not fit the source i16 carrier",
            )
        })?;
        if max_forward_speed < 0
            || max_forward_speed % crate::SPEED_STEP != 0
            || max_forward_speed > 32_752
        {
            return Err(definition_error(
                DefinitionErrorCode::OutOfRange,
                PLAYER_MAX_FORWARD_SPEED_ID,
                "player maximum forward speed must be a nonnegative multiple of 16 through 32752",
            ));
        }
        Ok(Self {
            tuning: FlightTuning { max_forward_speed },
        })
    }

    #[must_use]
    pub const fn tuning(self) -> FlightTuning {
        self.tuning
    }

    #[must_use]
    pub fn consumes(id: &SemanticId) -> bool {
        id.as_str() == PLAYER_MAX_FORWARD_SPEED_ID
    }
}
