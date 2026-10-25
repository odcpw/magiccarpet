//! Source frame-level status/transport/objective palette for indexed rendering.
//!
//! A pending source request and the visible fade are different temporal
//! carriers: PaletteChanges consumes the former once before entity dispatch.

use super::PaletteRequest;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Palette {
    pending: u8,
    count: i16,
    effect: u8,
    weight: u8,
    fading: bool,
}

impl Palette {
    /// Current source paletteSubMod, including an unconsumed request.
    pub(crate) fn mode(&self) -> u8 {
        self.pending
    }

    pub(crate) fn request(&mut self, request: PaletteRequest) {
        self.pending = request.mode;
        if request.mode == 3 {
            self.count = request.count;
        }
    }

    /// Returns moveSpeedFlag for the same following player upkeep. Repeating
    /// upkeep may request the current slowing stage again, as in sub_5ED50.
    pub(crate) fn advance_frame(&mut self) -> bool {
        match self.pending {
            // Case 5 clears the target palette, then case 10 performs a
            // separate 28-step fade. Neither the request nor the first
            // case-10 call changes the visible start palette.
            5 => {
                self.effect = 5;
                self.weight = 0;
                self.fading = false;
                self.pending = 10;
                true
            }
            10 => {
                if self.fading {
                    self.weight = self.weight.saturating_add(1).min(28);
                } else {
                    self.fading = true;
                }
                if self.weight == 28 {
                    self.pending = 0;
                    self.fading = false;
                    // The installed black palette survives completion;
                    // clearing the submode must not restore the base colors.
                }
                true
            }
            mode @ (3 | 6) => {
                self.effect = mode;
                self.weight = 4;
                self.fading = false;
                self.pending = 1;
                true
            }
            // PresentObjective requests source paletteSubMod 8 after this
            // frame's PaletteChanges. Consume it on the following frame;
            // case 8 installs the bright palette and selects case 9's longer
            // fade, independently of entity-pass count and pause state.
            8 => {
                self.effect = 8;
                self.weight = 16;
                self.fading = false;
                self.pending = 9;
                true
            }
            1 | 9 => {
                if self.fading {
                    self.weight = self.weight.saturating_sub(1);
                } else {
                    self.fading = true;
                }
                if self.weight == 0 {
                    self.effect = 0;
                    self.pending = 0;
                    self.fading = false;
                }
                true
            }
            _ => false,
        }
    }

    /// The native indexed frame already uploads this RGB8 palette. Source
    /// math remains RGB6. Flashes retain index0; source black fade covers
    /// every installed VGA color, including the background at index0.
    pub fn apply_rgb8(&self, palette: &mut [u8]) {
        if self.effect == 0 || self.weight == 0 {
            return;
        }
        let denominator = match self.effect {
            5 => 28,
            8 => 16,
            _ => 4,
        };
        let first_color = usize::from(self.effect != 5);
        for color in palette.chunks_exact_mut(3).take(256).skip(first_color) {
            let red = i32::from(color[0] >> 2);
            let green = i32::from(color[1] >> 2);
            let blue = i32::from(color[2] >> 2);
            let modified = match self.effect {
                5 => [0, 0, 0],
                3 => {
                    let offset = (-56 * i32::from(self.count)) >> 8;
                    [
                        (red + offset).clamp(0, 63),
                        green,
                        (blue + offset).clamp(0, 63),
                    ]
                }
                6 => [
                    (blue + 48).clamp(0, 63),
                    (green + 32).clamp(0, 63),
                    (blue + 32).clamp(0, 63),
                ],
                8 => [
                    (red + 48).clamp(0, 63),
                    (green + 48).clamp(0, 63),
                    (blue + 48).clamp(0, 63),
                ],
                _ => [red, green, blue],
            };
            for (channel, (base, changed)) in color
                .iter_mut()
                .zip([red, green, blue].into_iter().zip(modified))
            {
                *channel =
                    ((base + (changed - base) * i32::from(self.weight) / denominator) as u8) << 2;
            }
        }
    }

    pub(crate) fn hash(&self, hasher: &mut sha2::Sha256) {
        use sha2::Digest;
        if *self == Self::default() {
            return;
        }
        hasher.update(b"field-palette-v1");
        hasher.update([
            self.pending,
            self.effect,
            self.weight,
            u8::from(self.fading),
        ]);
        hasher.update(self.count.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::{Palette, PaletteRequest};
    use sha2::{Digest, Sha256};

    fn hash(palette: &Palette) -> Vec<u8> {
        let mut hasher = Sha256::new();
        palette.hash(&mut hasher);
        hasher.finalize().to_vec()
    }

    #[test]
    fn exit_black_fade_waits_for_source_setup_then_completes_twenty_eight_steps() {
        let mut original = [252_u8; 768];
        original[..6].copy_from_slice(&[4, 8, 12, 252, 160, 252]);
        let mut palette = Palette::default();
        palette.request(PaletteRequest { mode: 5, count: 0 });
        assert_eq!(palette.mode(), 5);
        let mut colors = original;
        palette.apply_rgb8(&mut colors);
        assert_eq!(colors, original, "request waits for PaletteChanges");

        assert!(palette.advance_frame());
        assert_eq!(palette.mode(), 10, "case 5 selects case 10, not step 1");
        colors = original;
        palette.apply_rgb8(&mut colors);
        assert_eq!(colors, original);
        let setup_hash = hash(&palette);

        // sub_90B27's first single-step call installs its captured palette
        // at step 0. Only later calls advance the interpolation counter.
        assert!(palette.advance_frame());
        assert_eq!(palette.mode(), 10);
        colors = original;
        palette.apply_rgb8(&mut colors);
        assert_eq!(colors, original);
        assert_ne!(
            hash(&palette),
            setup_hash,
            "step-0 initialization is retained"
        );
        for step in 1..=28 {
            assert!(palette.advance_frame());
            assert_eq!(palette.mode(), if step == 28 { 0 } else { 10 });
            colors = original;
            palette.apply_rgb8(&mut colors);
            match step {
                1 => assert_eq!(&colors[3..6], &[244, 156, 244]),
                14 => {
                    assert_eq!(&colors[3..6], &[128, 80, 128]);
                    assert_eq!(&colors[..3], &[4, 4, 8], "fade includes VGA index 0");
                }
                28 => assert_eq!(colors, [0; 768]),
                _ => {}
            }
        }
        let black_hash = hash(&palette);
        assert_ne!(black_hash, hash(&Palette::default()));
        assert!(!palette.advance_frame());
        assert_eq!(palette.mode(), 0);
        colors = original;
        palette.apply_rgb8(&mut colors);
        assert_eq!(
            colors, [0; 768],
            "completion retains the installed black palette"
        );
        assert_eq!(hash(&palette), black_hash);
    }

    #[test]
    fn objective_white_flash_waits_for_frame_then_fades_on_source_sixteen_step_cadence() {
        let mut original = [0_u8; 768];
        original[..9].copy_from_slice(&[0, 4, 8, 4, 8, 12, 240, 160, 252]);
        let mut palette = Palette::default();
        let initial_hash = hash(&palette);
        palette.request(PaletteRequest { mode: 8, count: 0 });
        let mut colors = original;
        palette.apply_rgb8(&mut colors);
        assert_eq!(
            colors, original,
            "post-objective request waits for PaletteChanges"
        );
        assert_ne!(
            hash(&palette),
            initial_hash,
            "pending flash is authoritative state"
        );

        assert!(palette.advance_frame());
        palette.apply_rgb8(&mut colors);
        assert_eq!(&colors[..9], &[0, 4, 8, 196, 200, 204, 252, 252, 252]);
        let flash_hash = hash(&palette);
        assert_ne!(flash_hash, initial_hash);

        // The first fade call installs its source start palette; the second
        // consumes the first of sixteen interpolation steps, as in the
        // existing sealed Level 1 objective palette implementation.
        assert!(palette.advance_frame());
        colors = original;
        palette.apply_rgb8(&mut colors);
        assert_eq!(&colors[3..6], &[196, 200, 204]);
        assert!(palette.advance_frame());
        colors = original;
        palette.apply_rgb8(&mut colors);
        assert_eq!(&colors[3..9], &[184, 188, 192, 248, 244, 252]);
        assert_ne!(hash(&palette), flash_hash);

        // A following objective phase may request case 8 again before the
        // previous fade ends: it replaces the request rather than stacking.
        palette.request(PaletteRequest { mode: 8, count: 0 });
        assert!(palette.advance_frame());
        colors = original;
        palette.apply_rgb8(&mut colors);
        assert_eq!(&colors[3..6], &[196, 200, 204]);
        for _ in 0..17 {
            assert!(palette.advance_frame());
        }
        colors = original;
        palette.apply_rgb8(&mut colors);
        assert_eq!(colors, original);
        assert!(!palette.advance_frame());
        assert_eq!(hash(&palette), initial_hash);
    }
}
