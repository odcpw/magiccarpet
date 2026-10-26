//! Steal Mana and Duel's three retained owner SPELLS.DAT tiers.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DirectionalSpellTier {
    pub power: u16,
    pub mana_cost: i32,
    pub castle_mana_required: i32,
    pub cooldown: u16,
    pub product: u8,
    pub single_player_experience: i32,
    pub multiplayer_experience: i32,
}

pub const STEAL_MANA_TIERS: [DirectionalSpellTier; 3] = [
    DirectionalSpellTier {
        power: 2000,
        mana_cost: 10000,
        castle_mana_required: 10000,
        cooldown: 5,
        product: 0,
        single_player_experience: 0,
        multiplayer_experience: 0,
    },
    DirectionalSpellTier {
        power: 4000,
        mana_cost: 20000,
        castle_mana_required: 16000,
        cooldown: 7,
        product: 0,
        single_player_experience: 400,
        multiplayer_experience: 250,
    },
    DirectionalSpellTier {
        power: 10,
        mana_cost: 30000,
        castle_mana_required: 32000,
        cooldown: 21,
        product: 1,
        single_player_experience: 1200,
        multiplayer_experience: 1000,
    },
];

pub const DUEL_TIERS: [DirectionalSpellTier; 3] = [
    DirectionalSpellTier {
        power: 5170,
        mana_cost: 10000,
        castle_mana_required: 10000,
        cooldown: 195,
        product: 0,
        single_player_experience: 0,
        multiplayer_experience: 0,
    },
    DirectionalSpellTier {
        power: 7720,
        mana_cost: 20000,
        castle_mana_required: 16000,
        cooldown: 395,
        product: 1,
        single_player_experience: 400,
        multiplayer_experience: 250,
    },
    DirectionalSpellTier {
        power: 7720,
        mana_cost: 40000,
        castle_mana_required: 36000,
        cooldown: 603,
        product: 2,
        single_player_experience: 1200,
        multiplayer_experience: 750,
    },
];
