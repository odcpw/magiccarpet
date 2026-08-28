//! Definition-to-runtime bindings owned by the neutral Rust domain.

use super::{
    DefinitionErrorCode, FIREBALL_MANA_COST_CAPABILITY_ID, GameDefinition, Result, SemanticId,
    SemanticValue,
};
use crate::FlightTuning;
use sim_definition::adapter::definition_error;

pub const PLAYER_MAX_FORWARD_SPEED_ID: &str = "flight.player.max_forward_speed";
pub const FIREBALL_MANA_COST_ID: &str = "spell.fireball.mana_cost";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefinitionFlightBinding {
    tuning: FlightTuning,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefinitionFireballBinding {
    mana_cost: i32,
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
