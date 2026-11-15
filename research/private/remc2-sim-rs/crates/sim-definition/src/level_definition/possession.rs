//! Possession's three source table rows and ordinary flight properties.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PossessionTierProperties {
    pub power: u16,
    pub cooldown: u16,
    pub castle_mana_required: i32,
    pub mana_cost: i32,
    /// The table's product selector, distinct from the holder's entity life.
    pub source_life: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PossessionProperties {
    pub tiers: [PossessionTierProperties; 3],
    pub projectile_speed: i16,
    pub projectile_lifetime: i16,
    pub basic_aim_distance: i16,
    pub tiered_aim_distance: i16,
}

/// Owner SPELLS.DAT family1, plus AddPossession flight defaults.
pub const POSSESSION_PROPERTIES: PossessionProperties = PossessionProperties {
    tiers: [
        PossessionTierProperties {
            power: 10,
            cooldown: 3,
            castle_mana_required: 0,
            mana_cost: 100,
            source_life: 0,
        },
        PossessionTierProperties {
            power: 15,
            cooldown: 41,
            castle_mana_required: 1_000,
            mana_cost: 250,
            source_life: 1,
        },
        PossessionTierProperties {
            power: 20,
            cooldown: 51,
            castle_mana_required: 20_000,
            mana_cost: 1_000,
            source_life: 2,
        },
    ],
    projectile_speed: 384,
    projectile_lifetime: 4_096 / 384,
    basic_aim_distance: 10_240,
    tiered_aim_distance: 16_384,
};
