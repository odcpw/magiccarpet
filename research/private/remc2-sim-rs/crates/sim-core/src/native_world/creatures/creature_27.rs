//! Source model27's one-callback, five-head, fifty-part creature.
//!
//! The numeric source rows remain authoritative. A root callback validates
//! the complete linked chain and updates its parts directly; parts 233/234 are
//! constructed but never scheduled as independent callbacks.

use crate::campaign::source_arena::SourceEntityHandle;
use crate::level_definition::{CREATURE27_CLASS_PROPERTIES, Creature27ClassProperties};

mod combat;
pub(crate) mod mc2;
mod multipart;
mod stage;
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

pub(crate) const PROPERTIES: Creature27ClassProperties = CREATURE27_CLASS_PROPERTIES;

/// Distinct source call sites for Hydra head sounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SoundSite {
    HeadRegrowth,
    AfterAttack,
    Projectile,
    Relocation,
}

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
