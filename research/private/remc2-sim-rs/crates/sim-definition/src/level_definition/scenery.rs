//! Immutable scenery properties consumed by the world constructors.

use serde::{Deserialize, Serialize};

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
