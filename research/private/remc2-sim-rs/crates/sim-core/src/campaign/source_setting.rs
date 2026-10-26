//! Typed carrier for the source global `setting_30` byte.
//!
//! `PlayerEvents` advances this once per invocation, after frame timing and
//! outside the player loop. It is independent of per-player entity passes and
//! the Rust frame-timing index. Keep it outside the imported MCR1 byte layout;
//! the source owns its initialization and reset sites.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SourceSetting30(u8);

impl SourceSetting30 {
    /// Source initialization and all observed reset sites set this byte to 0.
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self(0)
    }

    /// Read the current source value for deterministic actor RNG consumers.
    #[must_use]
    pub(crate) const fn value(self) -> u8 {
        self.0
    }

    /// Apply a source initialization or reset site.
    pub(crate) fn reset(&mut self) {
        self.0 = 0;
    }

    /// Advance at the source `PlayerEvents` boundary with u8 wraparound.
    pub(crate) fn advance(&mut self) -> u8 {
        self.0 = self.0.wrapping_add(1);
        self.0
    }
}
