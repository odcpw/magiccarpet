//! Source-authored Lightning content, exercised by Mission4's group6 reward.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LightningTier {
    pub damage: u16,
    pub mana_cost: i32,
    pub castle_mana_required: i32,
    pub cooldown: u16,
    pub impact_duration: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LightningProperties {
    pub tiers: [LightningTier; 3],
    pub projectile_speed: i16,
    pub projectile_range: i32,
    pub maximum_launch_speed: i16,
    pub aim_distance: i16,
    pub projectile_yaw_turn: i16,
    pub projectile_pitch_turn: i16,
}

/// SPELLS_BEGIN_BUFFER_str[9], sub_4D500, sub_6AB00 and sub_6DCA0.
pub const LIGHTNING_PROPERTIES: LightningProperties = LightningProperties {
    tiers: [
        LightningTier {
            damage: 1750,
            mana_cost: 1000,
            castle_mana_required: 5000,
            cooldown: 9,
            impact_duration: 2,
        },
        LightningTier {
            damage: 3500,
            mana_cost: 1500,
            castle_mana_required: 9000,
            cooldown: 17,
            impact_duration: 5,
        },
        LightningTier {
            damage: 7000,
            mana_cost: 2000,
            castle_mana_required: 18000,
            cooldown: 25,
            impact_duration: 10,
        },
    ],
    projectile_speed: 384,
    projectile_range: 8192,
    maximum_launch_speed: 8192,
    aim_distance: 10240,
    projectile_yaw_turn: 22,
    projectile_pitch_turn: 22,
};
