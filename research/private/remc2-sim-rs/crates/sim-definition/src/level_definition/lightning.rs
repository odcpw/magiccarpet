//! MC2 family7 Lightning, distinct from family9 Meteor.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LightningTier {
    pub damage: u16,
    pub mana_cost: i32,
    pub castle_mana_required: i32,
    pub cooldown: u16,
    /// Source life byte: direct ray, storm shot, or linked storm shots.
    pub product: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LightningProperties {
    pub tiers: [LightningTier; 3],
    pub projectile_speed: i16,
    pub ray_range: i32,
    pub storm_shot_range: i32,
    pub maximum_launch_speed: i16,
    pub aim_distance: i16,
}

/// Loaded owner SPELLS.DAT family7; constructors4D860/4DA20 and holder6A5C0.
pub const LIGHTNING_PROPERTIES: LightningProperties = LightningProperties {
    tiers: [
        LightningTier {
            damage: 200,
            mana_cost: 1000,
            castle_mana_required: 30000,
            cooldown: 5,
            product: 0,
        },
        LightningTier {
            damage: 300,
            mana_cost: 10000,
            castle_mana_required: 90000,
            cooldown: 35,
            product: 1,
        },
        LightningTier {
            damage: 800,
            mana_cost: 20000,
            castle_mana_required: 120000,
            cooldown: 45,
            product: 2,
        },
    ],
    projectile_speed: 384,
    ray_range: 3584,
    storm_shot_range: 2048,
    maximum_launch_speed: 8192,
    aim_distance: 16384,
};
