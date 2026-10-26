//! Typed behavior for the two targeted Level 2 spells.

pub mod duel;
pub mod steal_mana;

pub use duel::{DuelActor, DuelTetherBinding, DuelTetherRule, bind_tether, tether_rule};
pub use steal_mana::{CastleMana, ManaActor, ManaSphereGrant, StealManaOutcome, apply_steal_mana};

pub use crate::level_definition::{DUEL_TIERS, DirectionalSpellTier, STEAL_MANA_TIERS};
