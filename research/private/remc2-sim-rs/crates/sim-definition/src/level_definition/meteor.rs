//! Source-authored Meteor (spell family9), exercised by Mission4's group6 reward.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MeteorTier {
    pub damage: u16,
    pub mana_cost: i32,
    pub castle_mana_required: i32,
    pub cooldown: u16,
    pub impact_duration: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MeteorProperties {
    pub tiers: [MeteorTier; 3],
    pub projectile_speed: i16,
    pub projectile_range: i32,
    pub maximum_launch_speed: i16,
    pub aim_distance: i16,
    pub projectile_yaw_turn: i16,
    pub projectile_pitch_turn: i16,
}

/// Loaded owner SPELLS.DAT family9, sub_4D500, sub_6AB00 and sub_6DCA0.
pub const METEOR_PROPERTIES: MeteorProperties = MeteorProperties {
    tiers: [
        MeteorTier {
            damage: 4000,
            mana_cost: 6000,
            castle_mana_required: 40000,
            cooldown: 3,
            impact_duration: 2,
        },
        MeteorTier {
            damage: 8000,
            mana_cost: 10000,
            castle_mana_required: 80000,
            cooldown: 7,
            impact_duration: 5,
        },
        MeteorTier {
            damage: 16000,
            mana_cost: 20000,
            castle_mana_required: 150000,
            cooldown: 11,
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
