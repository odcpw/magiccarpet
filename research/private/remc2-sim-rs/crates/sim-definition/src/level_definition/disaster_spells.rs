//! Ordinary terrain spell rules loaded from owner SPELLS.DAT families15–18.
//! All three local owner copies agree (SHA256 7b426f0104f282d91bf7da904ed978e8
//! 09135e12e0dfebd845e4a582d52b51cc). The compiled Spells.cpp table is a
//! fallback; startup loads the owner table before SetDefaultSpells_5C0A0.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DisasterSpellTier {
    pub damage: u16,
    pub mana_cost: i32,
    pub castle_mana_required: i32,
    pub cooldown: u16,
    pub impact_parameter: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DisasterSpellProperties {
    pub tiers: [DisasterSpellTier; 3],
    pub projectile_speed: i16,
    pub projectile_range: i32,
    pub maximum_launch_speed: i16,
    pub aim_distance: i16,
    pub projectile_yaw_turn: i16,
    pub projectile_pitch_turn: i16,
}

const fn properties(tiers: [DisasterSpellTier; 3]) -> DisasterSpellProperties {
    DisasterSpellProperties {
        tiers,
        projectile_speed: 384,
        projectile_range: 8192,
        maximum_launch_speed: 8192,
        aim_distance: 4096,
        projectile_yaw_turn: 22,
        projectile_pitch_turn: 22,
    }
}

pub const TREMOR_PROPERTIES: DisasterSpellProperties = properties([
    DisasterSpellTier {
        damage: 200,
        mana_cost: 4000,
        castle_mana_required: 80000,
        cooldown: 31,
        impact_parameter: 60,
    },
    DisasterSpellTier {
        damage: 300,
        mana_cost: 6000,
        castle_mana_required: 140000,
        cooldown: 41,
        impact_parameter: 80,
    },
    DisasterSpellTier {
        damage: 800,
        mana_cost: 10000,
        castle_mana_required: 180000,
        cooldown: 61,
        impact_parameter: 120,
    },
]);

pub const CRATER_PROPERTIES: DisasterSpellProperties = properties([
    DisasterSpellTier {
        damage: 250,
        mana_cost: 6000,
        castle_mana_required: 100000,
        cooldown: 31,
        impact_parameter: 6,
    },
    DisasterSpellTier {
        damage: 400,
        mana_cost: 9000,
        castle_mana_required: 150000,
        cooldown: 41,
        impact_parameter: 12,
    },
    DisasterSpellTier {
        damage: 900,
        mana_cost: 12000,
        castle_mana_required: 200000,
        cooldown: 51,
        impact_parameter: 24,
    },
]);

pub const EARTHQUAKE_PROPERTIES: DisasterSpellProperties = properties([
    DisasterSpellTier {
        damage: 300,
        mana_cost: 10000,
        castle_mana_required: 150000,
        cooldown: 21,
        impact_parameter: 16,
    },
    DisasterSpellTier {
        damage: 500,
        mana_cost: 12000,
        castle_mana_required: 200000,
        cooldown: 31,
        impact_parameter: 32,
    },
    DisasterSpellTier {
        damage: 1000,
        mana_cost: 15000,
        castle_mana_required: 250000,
        cooldown: 41,
        impact_parameter: 64,
    },
]);

pub const VOLCANO_PROPERTIES: DisasterSpellProperties = properties([
    DisasterSpellTier {
        damage: 400,
        mana_cost: 12000,
        castle_mana_required: 200000,
        cooldown: 23,
        impact_parameter: 7,
    },
    DisasterSpellTier {
        damage: 800,
        mana_cost: 15000,
        castle_mana_required: 250000,
        cooldown: 33,
        impact_parameter: 9,
    },
    DisasterSpellTier {
        damage: 1200,
        mana_cost: 18000,
        castle_mana_required: 350000,
        cooldown: 43,
        impact_parameter: 11,
    },
]);
