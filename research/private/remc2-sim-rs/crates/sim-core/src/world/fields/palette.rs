//! Source frame-level status/transport palette, consumed by indexed rendering.
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
            1 => {
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
                _ => [red, green, blue],
            };
            for (channel, (base, changed)) in color
                .iter_mut()
                .zip([red, green, blue].into_iter().zip(modified))
            {
                *channel = ((base + (changed - base) * i32::from(self.weight) / 4) as u8) << 2;
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
