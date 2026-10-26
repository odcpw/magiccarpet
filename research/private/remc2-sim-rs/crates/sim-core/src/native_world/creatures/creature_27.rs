//! Source model27's one-callback, five-head, fifty-part creature.
//!
//! The numeric source rows remain authoritative. A root callback validates
//! the complete linked chain and updates its parts directly; parts 233/234 are
//! constructed but never scheduled as independent callbacks.

use crate::campaign::source_arena::SourceEntityHandle;

mod combat;
pub(crate) mod mc2;
mod multipart;
#[cfg(test)]
mod tests;

pub(crate) const SOURCE_MODEL: u8 = 27;
pub(crate) const ACTION_BASE: u8 = 216;
pub(crate) const HEAD_ACTION: u8 = 233;
pub(crate) const BODY_ACTION: u8 = 234;
pub(crate) const ROOT_PARTICLE: u16 = 315;
pub(crate) const HEAD_PARTICLE: u16 = 316;
pub(crate) const BODY_PARTICLE: u16 = 317;
pub(crate) const ROOT_DESCRIPTOR: u16 = 97;
pub(crate) const HEAD_DESCRIPTOR: u16 = 103;
pub(crate) const PROJECTILE_DESCRIPTOR: u16 = 106;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Properties {
    pub roots_require_free_slots: usize,
    pub head_count: usize,
    pub body_parts_per_head: usize,
    pub initial_action: u8,
    pub root_minimum_speed: i16,
    pub root_maximum_speed: i16,
    pub root_initial_speed: i16,
    pub root_max_life: i32,
    pub root_initial_life: i32,
    pub root_initial_mana: i32,
    pub root_initial_source_cadence: u8,
    pub root_turn_limit: i16,
    pub head_initial_speed: i16,
    pub head_base_life: i32,
    pub head_life_per_index: i32,
    pub body_spacing: i16,
}

pub(crate) const PROPERTIES: Properties = Properties {
    roots_require_free_slots: 51,
    head_count: 5,
    body_parts_per_head: 9,
    initial_action: ACTION_BASE + 1,
    root_minimum_speed: 64,
    root_maximum_speed: 0,
    root_initial_speed: 30,
    root_max_life: 36_000,
    root_initial_life: 1_000_000,
    root_initial_mana: 20_000,
    root_initial_source_cadence: 17,
    root_turn_limit: 0x400,
    head_initial_speed: 16,
    head_base_life: 920,
    head_life_per_index: 460,
    body_spacing: 96,
};

/// The constructor's generation-stamped result. The source word-0x34 chain is
/// the persistent topology; callers use these handles for the immediate
/// producer commit and never keep them as a second live family table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Allocation {
    pub root: SourceEntityHandle,
    pub heads: [SourceEntityHandle; 5],
    pub bodies: [[SourceEntityHandle; 9]; 5],
}

/// The action table has real callbacks for 216 through 223. Child actions
/// 233/234 are intentionally excluded: the root's 29A90 callback advances
/// them in the same pass.
pub(crate) const fn enabled_root_action(action: u8) -> bool {
    action >= ACTION_BASE && action <= ACTION_BASE + 7
}
