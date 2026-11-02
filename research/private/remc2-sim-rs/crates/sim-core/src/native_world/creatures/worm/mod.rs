//! Worm behavior across the authored/dynamic and imported package carriers.
//!
//! The enclosing campaign retains the sole source arena, numeric scheduler,
//! effect journal, reset authority, and following-pass cleanup. These modules
//! own only the carrier-specific Worm constructor and transition callbacks.

pub(crate) mod dynamic;
pub(in crate::native_world) mod imported;

#[cfg(test)]
mod dynamic_tests;
