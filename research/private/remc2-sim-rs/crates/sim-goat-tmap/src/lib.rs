//! Exact selector-global Goat TMAP clocks recovered from the owner-data oracle.
//!
//! MC2 mutates one shared texture descriptor for each of selectors 152, 153,
//! and 154.  The clocks are not per-Goat entity.  Rendering only sets a
//! one-bit "used" latch; the following unpaused outer turn clears that latch
//! and advances the corresponding descriptor once.

pub const GOAT_TMAP_SELECTORS: [u16; 3] = [152, 153, 154];
pub const GOAT_TMAP_DRAW_MASK: u8 = 0b0000_0111;
pub const GOAT_TMAP_STATE_COUNT: usize = 5;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GoatTmapFrame {
    pub state_ordinal: u8,
    pub frame_index: u8,
    pub stream_cursor: u32,
    pub width: u16,
    pub height: u16,
    pub sha256: String,
    pub pixels: Box<[u8]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GoatTmapSelectorBank {
    pub selector: u16,
    pub stream_start: u32,
    pub payload_bytes: u32,
    pub states: [GoatTmapFrame; GOAT_TMAP_STATE_COUNT],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GoatTmapBank {
    selectors: [GoatTmapSelectorBank; GOAT_TMAP_SELECTORS.len()],
}

impl GoatTmapBank {
    /// Construct a bank whose selector ordering and state ordinals have
    /// already been validated against the private owner-data cache.
    ///
    /// # Errors
    ///
    /// Returns an error if selector order, state ordinals, dimensions, or
    /// payload sizes are inconsistent.
    pub fn new(
        selectors: [GoatTmapSelectorBank; GOAT_TMAP_SELECTORS.len()],
    ) -> Result<Self, String> {
        for (selector_ordinal, (selector, expected_selector)) in
            selectors.iter().zip(GOAT_TMAP_SELECTORS).enumerate()
        {
            if selector.selector != expected_selector {
                return Err(format!(
                    "Goat TMAP selector ordinal {selector_ordinal} is {}, expected {expected_selector}",
                    selector.selector
                ));
            }
            if selector.stream_start != selector.states[0].stream_cursor {
                return Err(format!(
                    "Goat TMAP {expected_selector} stream start does not match reset cursor"
                ));
            }
            for (state_ordinal, state) in selector.states.iter().enumerate() {
                if usize::from(state.state_ordinal) != state_ordinal {
                    return Err(format!(
                        "Goat TMAP {expected_selector} state ordinal is {}, expected {state_ordinal}",
                        state.state_ordinal
                    ));
                }
                let expected_bytes = usize::from(state.width) * usize::from(state.height);
                if state.pixels.len() != expected_bytes {
                    return Err(format!(
                        "Goat TMAP {expected_selector} state {state_ordinal} has {} pixels, expected {expected_bytes}",
                        state.pixels.len()
                    ));
                }
            }
        }
        Ok(Self { selectors })
    }

    #[must_use]
    pub const fn selectors(&self) -> &[GoatTmapSelectorBank; GOAT_TMAP_SELECTORS.len()] {
        &self.selectors
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GoatTmapClock {
    selector: u16,
    state_ordinal: u8,
    frame_index: u8,
    stream_cursor: u32,
    width: u16,
    height: u16,
    used_by_preceding_draw: bool,
    pixels: Box<[u8]>,
}

impl GoatTmapClock {
    #[must_use]
    pub const fn selector(&self) -> u16 {
        self.selector
    }

    #[must_use]
    pub const fn state_ordinal(&self) -> u8 {
        self.state_ordinal
    }

    #[must_use]
    pub const fn frame_index(&self) -> u8 {
        self.frame_index
    }

    #[must_use]
    pub const fn stream_cursor(&self) -> u32 {
        self.stream_cursor
    }

    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    #[must_use]
    pub const fn used_by_preceding_draw(&self) -> bool {
        self.used_by_preceding_draw
    }

    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    #[must_use]
    pub fn pixel_hash_v1(&self) -> u64 {
        let mut hash = Fnv1a64::new();
        hash.bytes(b"mc2-goat-tmap-pixels-v1\0");
        hash.bytes(&self.pixels);
        hash.finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GoatTmapClocks {
    clocks: [GoatTmapClock; GOAT_TMAP_SELECTORS.len()],
}

impl GoatTmapClocks {
    #[must_use]
    pub fn from_bank(bank: &GoatTmapBank) -> Self {
        Self {
            clocks: std::array::from_fn(|ordinal| {
                let selector = &bank.selectors[ordinal];
                let reset = &selector.states[0];
                GoatTmapClock {
                    selector: selector.selector,
                    state_ordinal: reset.state_ordinal,
                    frame_index: reset.frame_index,
                    stream_cursor: reset.stream_cursor,
                    width: reset.width,
                    height: reset.height,
                    used_by_preceding_draw: false,
                    pixels: reset.pixels.clone(),
                }
            }),
        }
    }

    pub fn reset(&mut self, bank: &GoatTmapBank) {
        *self = Self::from_bank(bank);
    }

    /// OR exact post-draw selector feedback into the three source latches.
    ///
    /// # Errors
    ///
    /// Returns an error when bits outside selectors 152..154 are set.
    pub fn latch_draw_attempts(&mut self, draw_attempt_mask: u8) -> Result<(), String> {
        if draw_attempt_mask & !GOAT_TMAP_DRAW_MASK != 0 {
            return Err(format!(
                "Goat TMAP draw-attempt mask {draw_attempt_mask:#04x} has unsupported bits"
            ));
        }
        for (ordinal, clock) in self.clocks.iter_mut().enumerate() {
            clock.used_by_preceding_draw |= draw_attempt_mask & (1 << ordinal) != 0;
        }
        Ok(())
    }

    /// Service the latches once at the start of an unpaused outer turn.
    ///
    /// Selectors are visited in original ascending order.  Clearing before
    /// advance deliberately restores the executable sequence beginning at
    /// 0x715F0, whose write-back store at 0x715F3 is omitted by remc2.
    pub fn service_unpaused_outer_turn(&mut self, bank: &GoatTmapBank) {
        for (ordinal, clock) in self.clocks.iter_mut().enumerate() {
            if !clock.used_by_preceding_draw {
                continue;
            }
            clock.used_by_preceding_draw = false;
            let next_ordinal = if clock.state_ordinal >= 4 {
                1
            } else {
                clock.state_ordinal + 1
            };
            let next = &bank.selectors[ordinal].states[usize::from(next_ordinal)];
            clock.state_ordinal = next.state_ordinal;
            clock.frame_index = next.frame_index;
            clock.stream_cursor = next.stream_cursor;
            clock.width = next.width;
            clock.height = next.height;
            clock.pixels.clone_from(&next.pixels);
        }
    }

    #[must_use]
    pub const fn states(&self) -> &[GoatTmapClock; GOAT_TMAP_SELECTORS.len()] {
        &self.clocks
    }

    #[must_use]
    pub fn hash_v1(&self) -> u64 {
        let mut hash = Fnv1a64::new();
        hash.bytes(b"mc2-goat-tmap-clocks-v1\0");
        for clock in &self.clocks {
            hash.u16(clock.selector);
            hash.u8(clock.state_ordinal);
            hash.u8(clock.frame_index);
            hash.u32(clock.stream_cursor);
            hash.u16(clock.width);
            hash.u16(clock.height);
            hash.u8(u8::from(clock.used_by_preceding_draw));
            hash.u32(u32::try_from(clock.pixels.len()).expect("Goat TMAP pixels fit u32"));
            hash.bytes(&clock.pixels);
        }
        hash.finish()
    }
}

struct Fnv1a64(u64);

impl Fnv1a64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    const fn new() -> Self {
        Self(Self::OFFSET_BASIS)
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    fn u8(&mut self, value: u8) {
        self.bytes(&[value]);
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bank() -> GoatTmapBank {
        let cursors = [
            [490, 818, 1146, 1472, 1812],
            [710, 1196, 1700, 2202, 2724],
            [490, 822, 1152, 1482, 1816],
        ];
        GoatTmapBank::new(std::array::from_fn(|selector_ordinal| {
            let selector = GOAT_TMAP_SELECTORS[selector_ordinal];
            GoatTmapSelectorBank {
                selector,
                stream_start: cursors[selector_ordinal][0],
                payload_bytes: [1818, 2730, 1822][selector_ordinal],
                states: std::array::from_fn(|state_ordinal| GoatTmapFrame {
                    state_ordinal: state_ordinal as u8,
                    frame_index: state_ordinal as u8 + 1,
                    stream_cursor: cursors[selector_ordinal][state_ordinal],
                    width: if selector == 153 { 32 } else { 22 },
                    height: 22,
                    sha256: format!("{selector}-{state_ordinal}"),
                    pixels: vec![
                        state_ordinal as u8;
                        if selector == 153 { 32 * 22 } else { 22 * 22 }
                    ]
                    .into_boxed_slice(),
                }),
            }
        }))
        .expect("synthetic exact-shape bank")
    }

    #[test]
    fn independent_latches_advance_once_and_wrap_to_frame_one() {
        let bank = bank();
        let mut clocks = GoatTmapClocks::from_bank(&bank);
        clocks.latch_draw_attempts(0b001).unwrap();
        clocks.latch_draw_attempts(0b001).unwrap();
        clocks.service_unpaused_outer_turn(&bank);
        assert_eq!(
            std::array::from_fn(|ordinal| clocks.states()[ordinal].state_ordinal()),
            [1, 0, 0]
        );
        assert!(!clocks.states()[0].used_by_preceding_draw());

        for expected in [2, 3, 4, 1] {
            clocks.latch_draw_attempts(0b001).unwrap();
            clocks.service_unpaused_outer_turn(&bank);
            assert_eq!(clocks.states()[0].state_ordinal(), expected);
        }
        assert_eq!(clocks.states()[0].frame_index(), 2);
        assert_eq!(clocks.states()[0].stream_cursor(), 818);
    }

    #[test]
    fn invisible_selectors_freeze_and_invalid_mask_does_not_mutate() {
        let bank = bank();
        let mut clocks = GoatTmapClocks::from_bank(&bank);
        let reset = clocks.clone();
        clocks.service_unpaused_outer_turn(&bank);
        assert_eq!(clocks, reset);
        assert!(clocks.latch_draw_attempts(0x80).is_err());
        assert_eq!(clocks, reset);
    }

    #[test]
    fn pause_is_zero_services_and_resume_consumes_once() {
        let bank = bank();
        let mut clocks = GoatTmapClocks::from_bank(&bank);
        clocks.latch_draw_attempts(0b110).unwrap();
        let paused = clocks.clone();
        assert_eq!(clocks, paused);
        clocks.service_unpaused_outer_turn(&bank);
        assert_eq!(
            std::array::from_fn(|ordinal| clocks.states()[ordinal].state_ordinal()),
            [0, 1, 1]
        );
    }

    #[test]
    fn selector_drift_and_simultaneous_draws_remain_independent() {
        let bank = bank();
        let mut clocks = GoatTmapClocks::from_bank(&bank);
        for (mask, expected) in [
            (0b001, [1, 0, 0]),
            (0b010, [1, 1, 0]),
            (0b100, [1, 1, 1]),
            (0b000, [1, 1, 1]),
            (0b111, [2, 2, 2]),
        ] {
            clocks.latch_draw_attempts(mask).unwrap();
            clocks.service_unpaused_outer_turn(&bank);
            assert_eq!(
                std::array::from_fn(|ordinal| clocks.states()[ordinal].state_ordinal()),
                expected
            );
        }
    }
}
