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
    // None retains verified pre-policy replay history exactly. Runtime frame
    // admission installs the source startup policy at a recorded boundary.
    source: Option<SourcePalette>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourcePalette { phase: u8, brightness: u8, fade: Option<BrightnessFade> }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BrightnessFade { from: Option<u8>, step: u8, running: bool }

impl Palette {
    pub(crate) fn source_brightness(&self) -> Option<u8> { self.source.map(|source| source.brightness) }
    pub(crate) fn source_phase(&self) -> Option<u8> { self.source.map(|source| source.phase) }

    pub(crate) fn brightness_admitted(&self) -> bool {
        self.source.is_some_and(|source| source.phase >= 3) && self.pending == 0
    }

    pub(crate) fn bind_source_brightness(&mut self, brightness: u8) {
        if let Some(source) = self.source {
            if source.brightness != brightness { self.request_brightness(brightness); }
        } else {
            *self = Self { source: Some(SourcePalette { phase: 0, brightness, fade: None }), ..Self::default() };
        }
    }

    /// sub_47650 changes the target and requests case1, including a clamped
    /// endpoint. The installed VGA palette stays at the old value until the
    /// ordinary PaletteChanges prefix performs its single-step fade.
    pub(crate) fn request_brightness(&mut self, brightness: u8) {
        let source = self.source.as_mut().expect("source brightness requires an admitted palette policy");
        let old = source.brightness;
        source.brightness = brightness;
        if source.phase >= 3 {
            source.fade = Some(BrightnessFade { from: Some(old), step: 0, running: false });
            self.pending = 1;
            self.effect = 0;
            self.weight = 0;
            self.fading = false;
        }
    }

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
        if let Some(source) = &mut self.source {
            if source.phase < 3 {
                source.phase += 1;
                if source.phase == 3 {
                    // PaletteChanges case2 installs black, loads the selected
                    // base palette and calls sub_47650 before the case1 fade.
                    source.fade = Some(BrightnessFade { from: None, step: 0, running: false });
                    self.pending = 1;
                }
                return false;
            }
            if self.pending == 1 && let Some(fade) = &mut source.fade {
                if fade.running { fade.step += 1; } else { fade.running = true; }
                if fade.step == 4 { source.fade = None; self.pending = 0; }
                return true;
            }
            // A new effect request changes only the pending submode. It
            // supersedes the installed brightness fade when consumed here,
            // not when an objective/actor publishes the request later in a frame.
            if self.pending != 1 { source.fade = None; }
        }
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
            mode @ (2 | 3 | 4 | 6 | 7) => {
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
        if let Some(source) = self.source {
            if source.phase < 3 { palette.fill(0); return; }
            for channel in palette.iter_mut() {
                let base = i32::from(*channel >> 2);
                let target = (base + 6 * i32::from(source.brightness)).min(63);
                let value = if let Some(fade) = source.fade {
                    let from = fade.from.map_or(0, |old| (base + 6 * i32::from(old)).min(63));
                    from + i32::from(fade.step) * (target - from) / 4
                } else { target };
                *channel = (value as u8) << 2;
            }
        }
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
                2 => [(red + 40).min(63), green, blue],
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
                4 => [red, green, 63],
                7 => {
                    let mean = (red + green + blue) / 3;
                    [mean, mean, mean]
                }
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
        if let Some(source) = self.source {
            hasher.update(b"source-brightness-palette-v1");
            hasher.update([source.phase, source.brightness]);
            match source.fade {
                None => hasher.update([0]),
                Some(fade) => hasher.update([1, fade.from.unwrap_or(255), fade.step, u8::from(fade.running)]),
            }
        }
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
    fn source_brightness_startup_refade_and_endpoint_follow_actual_palette_prefix() {
        let base = [4, 80, 240];
        let mut palette = Palette::default();
        palette.bind_source_brightness(3);
        for phase in 1..=3 {
            assert!(!palette.brightness_admitted());
            assert!(!palette.advance_frame());
            assert_eq!(palette.source_phase(), Some(phase));
            let mut rgb = base;
            palette.apply_rgb8(&mut rgb);
            assert_eq!(rgb, [0; 3], "loading prefix installs black before case1");
        }
        for step in 0..=4 {
            assert!(palette.advance_frame());
            let mut rgb = base;
            palette.apply_rgb8(&mut rgb);
            assert_eq!(rgb, [(19 * step / 4 * 4) as u8, (38 * step / 4 * 4) as u8, (63 * step / 4 * 4) as u8]);
            assert_eq!(palette.brightness_admitted(), step == 4);
        }
        palette.request_brightness(4);
        assert_eq!(palette.mode(), 1);
        let before = hash(&palette);
        for _ in 0..10 { let mut rgb = base; palette.apply_rgb8(&mut rgb); assert_eq!(rgb, [76, 152, 252]); }
        assert_eq!(hash(&palette), before, "reads cannot advance the fade");
        for (step, expected) in [[76, 152, 252], [80, 156, 252], [88, 164, 252], [92, 168, 252], [100, 176, 252]].into_iter().enumerate() {
            palette.advance_frame();
            let mut rgb = base;
            palette.apply_rgb8(&mut rgb);
            assert_eq!(rgb, expected);
            assert_eq!(palette.brightness_admitted(), step == 4);
        }
        palette.request_brightness(4);
        assert!(!palette.brightness_admitted(), "clamped endpoints still request source submode1");
        for _ in 0..5 { palette.advance_frame(); }
        assert!(palette.brightness_admitted());
        palette.request(PaletteRequest { mode: 2, count: 0 });
        assert!(!palette.brightness_admitted(), "hit/transport/objective palette requests block case6 too");
        for _ in 0..6 { palette.advance_frame(); }
        assert!(palette.brightness_admitted());
        palette.request_brightness(0);
        for expected in [[100, 176, 252], [76, 152, 252], [52, 128, 248], [28, 104, 244], [4, 80, 240]] {
            palette.advance_frame();
            let mut rgb = base;
            palette.apply_rgb8(&mut rgb);
            assert_eq!(rgb, expected, "negative RGB6 deltas truncate toward zero before VGA scaling");
        }
    }

    #[test]
    fn hit_blue_and_grey_requests_use_source_rgb6_then_release_the_base_palette() {
        let mut original = [0_u8; 768];
        original[..6].copy_from_slice(&[4, 8, 12, 120, 80, 40]);
        for (mode, changed) in [(2, [252, 80, 40]), (4, [120, 80, 252]), (7, [80, 80, 80])] {
            let mut palette = Palette::default();
            palette.request(PaletteRequest { mode, count: 0 });
            let mut colors = original;
            palette.apply_rgb8(&mut colors);
            assert_eq!(
                colors, original,
                "source request waits for frame palette phase"
            );
            assert!(palette.advance_frame());
            colors = original;
            palette.apply_rgb8(&mut colors);
            assert_eq!(&colors[..3], &original[..3]);
            assert_eq!(&colors[3..6], &changed);
            for _ in 0..5 {
                assert!(palette.advance_frame());
            }
            colors = original;
            palette.apply_rgb8(&mut colors);
            assert_eq!(colors, original);
            assert_eq!(palette.mode(), 0);
        }
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
