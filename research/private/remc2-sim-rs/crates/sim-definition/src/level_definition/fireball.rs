//! Fireball's owner spell rows and source constructor defaults.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FireballTierProperties {
    pub power: u16,
    pub cooldown: u16,
    pub castle_mana_required: i32,
    pub mana_cost: i32,
    pub font_type: u8,
    pub source_life: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FireballProperties {
    pub tiers: [FireballTierProperties; 3],
    pub projectile_speed: i16,
    pub projectile_lifetime: i16,
    pub aim_distance: i16,
    pub render_index: u16,
    pub local_render_index: u16,
}

pub const FIREBALL_PROPERTIES: FireballProperties = FireballProperties {
    tiers: [
        FireballTierProperties {
            power: 250,
            cooldown: 5,
            castle_mana_required: 0,
            mana_cost: 100,
            font_type: 0,
            source_life: 0,
        },
        FireballTierProperties {
            power: 160,
            cooldown: 11,
            castle_mana_required: 0,
            mana_cost: 250,
            font_type: 1,
            source_life: 1,
        },
        FireballTierProperties {
            power: 80,
            cooldown: 15,
            castle_mana_required: 160_000,
            mana_cost: 1_500,
            font_type: 0,
            source_life: 2,
        },
    ],
    projectile_speed: 384,
    projectile_lifetime: 0x2000 / 384,
    aim_distance: 0x4000,
    render_index: 340,
    local_render_index: 42,
};
