//! Gravity Well and Whirlwind tiers retained from the installed SPELLS.DAT.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FieldSpellTier {
    pub power: u16,
    pub mana_cost: i32,
    pub castle_mana_required: i32,
    pub cooldown: u16,
    pub effect_parameter: u8,
    pub experience_threshold: i32,
}

pub const GRAVITY_WELL_TIERS: [FieldSpellTier; 3] = [
    FieldSpellTier {
        power: 750,
        mana_cost: 15000,
        castle_mana_required: 300000,
        cooldown: 11,
        effect_parameter: 16,
        experience_threshold: 0,
    },
    FieldSpellTier {
        power: 1500,
        mana_cost: 20000,
        castle_mana_required: 350000,
        cooldown: 21,
        effect_parameter: 26,
        experience_threshold: 1500,
    },
    FieldSpellTier {
        power: 3000,
        mana_cost: 30000,
        castle_mana_required: 400000,
        cooldown: 27,
        effect_parameter: 40,
        experience_threshold: 3000,
    },
];

pub const WHIRLWIND_TIERS: [FieldSpellTier; 3] = [
    FieldSpellTier {
        power: 100,
        mana_cost: 2000,
        castle_mana_required: 60000,
        cooldown: 7,
        effect_parameter: 5,
        experience_threshold: 0,
    },
    FieldSpellTier {
        power: 180,
        mana_cost: 2500,
        castle_mana_required: 90000,
        cooldown: 11,
        effect_parameter: 10,
        experience_threshold: 400,
    },
    FieldSpellTier {
        power: 240,
        mana_cost: 5000,
        castle_mana_required: 120000,
        cooldown: 21,
        effect_parameter: 10,
        experience_threshold: 1200,
    },
];
