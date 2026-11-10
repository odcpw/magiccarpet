//! Immutable properties for passive world items.

use serde::{Deserialize, Serialize};

/// A stationary item with no collection, movement or reward callback.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PassiveItemDefinition {
    pub max_life: i32,
    pub horizontal_half_extent: i16,
    pub vertical_half_extent: i16,
}

impl Default for PassiveItemDefinition {
    fn default() -> Self {
        Self {
            max_life: 300,
            horizontal_half_extent: 384,
            vertical_half_extent: 384,
        }
    }
}

impl PassiveItemDefinition {
    pub fn validate(self) -> Result<(), String> {
        if self.max_life <= 0 || self.horizontal_half_extent <= 0 || self.vertical_half_extent <= 0
        {
            return Err("passive items require positive life and collision extents".into());
        }
        Ok(())
    }
}

/// The source reward controller's construction health. Its emission cadence
/// and child construction remain behavior owned by the item family.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RewardJarDefinition {
    pub max_life: i32,
}

impl Default for RewardJarDefinition {
    fn default() -> Self {
        Self { max_life: 10_000 }
    }
}

impl RewardJarDefinition {
    pub fn validate(self) -> Result<(), String> {
        if self.max_life <= 0 {
            return Err("reward jars require positive construction health".into());
        }
        Ok(())
    }
}
