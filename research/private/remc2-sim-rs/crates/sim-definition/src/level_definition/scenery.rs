//! Immutable scenery properties consumed by the world constructors.

use serde::{Deserialize, Serialize};

/// Stationary scenery whose scheduled rule leaves its source packet intact.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StaticSceneryDefinition {
    pub max_life: i32,
}

impl Default for StaticSceneryDefinition {
    fn default() -> Self {
        Self { max_life: 300 }
    }
}

impl StaticSceneryDefinition {
    pub fn validate(self) -> Result<(), String> {
        if self.max_life <= 0 {
            return Err("static scenery maximum life must be positive".into());
        }
        Ok(())
    }
}

/// The source Dolmen's collision extents; its sprite remains a retained asset binding.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DolmenDefinition {
    pub horizontal_half_extent: i16,
    pub vertical_half_extent: i16,
}

impl Default for DolmenDefinition {
    fn default() -> Self {
        Self { horizontal_half_extent: 1024, vertical_half_extent: 1024 }
    }
}

impl DolmenDefinition {
    pub fn validate(self) -> Result<(), String> {
        if self.horizontal_half_extent <= 0 || self.vertical_half_extent <= 0 {
            return Err("dolmen collision extents must be positive".into());
        }
        Ok(())
    }
}

/// Shared life rule for moving scenery; appearance remains an asset binding.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BallisticSceneryDefinition {
    pub minimum_life: i32,
    pub life_variation: u16,
}

impl Default for BallisticSceneryDefinition {
    fn default() -> Self {
        Self {
            minimum_life: 400,
            life_variation: 2_000,
        }
    }
}

impl BallisticSceneryDefinition {
    pub fn validate(self) -> Result<(), String> {
        if self.minimum_life <= 0
            || self.life_variation == 0
            || self
                .minimum_life
                .checked_add(i32::from(self.life_variation) - 1)
                .is_none()
        {
            return Err("ballistic scenery requires a positive, representable life range".into());
        }
        Ok(())
    }
}

/// A mushroom's life is minimum_life + entity_rng % life_variation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MushroomDefinition {
    pub minimum_life: i32,
    pub life_variation: u16,
}

impl Default for MushroomDefinition {
    fn default() -> Self {
        Self {
            minimum_life: 100,
            life_variation: 80,
        }
    }
}

impl MushroomDefinition {
    pub fn validate(self) -> Result<(), String> {
        if self.minimum_life <= 0
            || self.life_variation == 0
            || self
                .minimum_life
                .checked_add(i32::from(self.life_variation - 1))
                .is_none()
        {
            return Err("mushroom life range must be positive and fit i32".into());
        }
        Ok(())
    }
}
