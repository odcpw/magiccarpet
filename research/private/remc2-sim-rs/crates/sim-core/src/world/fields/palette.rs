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
    /// math remains RGB6; index0 keeps the retained package's background.
    pub fn apply_rgb8(&self, palette: &mut [u8]) {
        if self.effect == 0 || self.weight == 0 {
            return;
        }
        let denominator = if self.effect == 8 { 16 } else { 4 };
        for color in palette.chunks_exact_mut(3).take(256).skip(1) {
            let red = i32::from(color[0] >> 2);
            let green = i32::from(color[1] >> 2);
            let blue = i32::from(color[2] >> 2);
            let modified = match self.effect {
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
