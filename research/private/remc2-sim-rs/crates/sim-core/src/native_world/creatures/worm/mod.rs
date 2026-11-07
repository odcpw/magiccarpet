//! Worm behavior across the authored/dynamic and imported package carriers.
//!
//! The enclosing campaign retains the sole source arena, numeric scheduler,
//! effect journal, reset authority, and following-pass cleanup. These modules
//! own only the carrier-specific Worm constructor and transition callbacks.

pub(crate) mod dynamic;
pub(in crate::native_world) mod imported;
pub(crate) mod mc2;
pub(crate) mod projectile;
pub(crate) mod stage;

#[cfg(test)]
mod dynamic_tests;

#[cfg(test)]
mod encounter_tests;
