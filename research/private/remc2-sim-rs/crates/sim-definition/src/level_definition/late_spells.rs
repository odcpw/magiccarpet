//! Owner `DATA/SPELLS.DAT` rows 22–24 after the live SetDefaultSpells pass.
//!
//! `SPELLS.DAT` is loaded first. `SetDefaultSpells_5C0A0` then overrides only
//! family 23's `maxManaLimit_A`, so each tier keeps both the raw owner value
//! and the value that the running game actually checks.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LateSpellTier {
    /// `subSpellIndex_2`, the family's effect payload.
    pub subspell_index: i32,
    /// `manaCost_6`, charged through the caster's ordinary mana-regeneration
    /// lane on the holder pulse.
    pub mana_cost: i32,
    /// Raw owner-table `maxManaLimit_A` before SetDefaultSpells overrides.
    pub owner_castle_mana_limit: i32,
    /// Effective live `maxManaLimit_A` after SetDefaultSpells.
    pub runtime_castle_mana_limit: i32,
    /// Owner `xpos1_E` and `xpos2_0x12` fields.
    pub xpos1: i32,
    pub xpos2: i32,
    /// `hintText_0x16x`, `word_0x18`, `life_0x1A`, and `fontType_0x1B`.
    pub hint_text: i16,
    pub cooldown: i16,
    pub life: u8,
    pub font_type: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LateSpellProperties {
    pub family: u8,
    pub tiers: [LateSpellTier; 3],
}

pub const FOOLS_MANA_PROPERTIES: LateSpellProperties = LateSpellProperties {
    family: 22,
    tiers: [
        LateSpellTier {
            subspell_index: 500,
            mana_cost: 15_000,
            owner_castle_mana_limit: 30_000,
            runtime_castle_mana_limit: 30_000,
            xpos1: 0,
            xpos2: 0,
            hint_text: 254,
            cooldown: 25,
            life: 0,
            font_type: 0,
        },
        LateSpellTier {
            subspell_index: 1_000,
            mana_cost: 20_000,
            owner_castle_mana_limit: 40_000,
            runtime_castle_mana_limit: 40_000,
            xpos1: 360,
            xpos2: 160,
            hint_text: 255,
            cooldown: 25,
            life: 1,
            font_type: 0,
        },
        LateSpellTier {
            subspell_index: 2_000,
            mana_cost: 25_000,
            owner_castle_mana_limit: 50_000,
            runtime_castle_mana_limit: 50_000,
            xpos1: 720,
            xpos2: 320,
            hint_text: 256,
            cooldown: 25,
            life: 2,
            font_type: 0,
        },
    ],
};

pub const MAGIC_MINE_PROPERTIES: LateSpellProperties = LateSpellProperties {
    family: 23,
    tiers: [
        LateSpellTier {
            subspell_index: 1_000,
            mana_cost: 10_000,
            owner_castle_mana_limit: 300_000,
            runtime_castle_mana_limit: 50_000,
            xpos1: 0,
            xpos2: 0,
            hint_text: 257,
            cooldown: 47,
            life: 0,
            font_type: 0,
        },
        LateSpellTier {
            subspell_index: 5_000,
            mana_cost: 12_000,
            owner_castle_mana_limit: 350_000,
            runtime_castle_mana_limit: 70_000,
            xpos1: 400,
            xpos2: 240,
            hint_text: 258,
            cooldown: 67,
            life: 1,
            font_type: 0,
        },
        LateSpellTier {
            subspell_index: 10_000,
            mana_cost: 24_000,
            owner_castle_mana_limit: 400_000,
            runtime_castle_mana_limit: 90_000,
            xpos1: 1_200,
            xpos2: 720,
            hint_text: 259,
            cooldown: 87,
            life: 2,
            font_type: 0,
        },
    ],
};

pub const ALLIANCE_PROPERTIES: LateSpellProperties = LateSpellProperties {
    family: 24,
    tiers: [
        LateSpellTier {
            subspell_index: 610,
            mana_cost: 10_000,
            owner_castle_mana_limit: 200_000,
            runtime_castle_mana_limit: 200_000,
            xpos1: 0,
            xpos2: 0,
            hint_text: 260,
            cooldown: 23,
            life: 16,
            font_type: 0,
        },
        LateSpellTier {
            subspell_index: 1_100,
            mana_cost: 18_000,
            owner_castle_mana_limit: 300_000,
            runtime_castle_mana_limit: 300_000,
            xpos1: 800,
            xpos2: 400,
            hint_text: 261,
            cooldown: 29,
            life: 26,
            font_type: 0,
        },
        LateSpellTier {
            subspell_index: 2_710,
            mana_cost: 30_000,
            owner_castle_mana_limit: 400_000,
            runtime_castle_mana_limit: 400_000,
            xpos1: 1_600,
            xpos2: 800,
            hint_text: 262,
            cooldown: 63,
            life: 32,
            font_type: 0,
        },
    ],
};
