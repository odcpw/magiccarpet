//! Qualified indexed TMAP resources for the live Level 1 combat families.
//!
//! Fireball selector 463 is one selector-global mutable SS2 stream. Arrow,
//! castle-projectile, impact, and splash selectors are immutable frames.

use crate::ss2_tmap::{apply_ss2_delta, read_u16, read_u32, require_hash};

pub const FIREBALL_SELECTOR: u16 = 463;
pub const FIREBALL_STATE_COUNT: usize = 5;
pub const POSSESSION_SELECTOR: u16 = 144;
pub const POSSESSION_STATE_COUNT: usize = 9;
pub const COMBAT_STATIC_SELECTORS: [u16; 35] = [
    61, 105, 106, 107, 108, 109, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 186, 187, 188, 189, 190,
    191, 192, 193, 194, 195, 54, 96, 97, 98, 99, 100, 101, 102, 103,
];
pub const COMBAT_STATIC_EXTENTS: [(u16, u16); 35] = [
    (32, 31),
    (18, 24),
    (35, 14),
    (50, 12),
    (38, 18),
    (40, 31),
    (40, 39),
    (40, 39),
    (40, 39),
    (40, 39),
    (40, 39),
    (40, 39),
    (40, 39),
    (40, 39),
    (40, 39),
    (40, 39),
    (28, 29),
    (28, 29),
    (28, 29),
    (28, 29),
    (28, 29),
    (28, 29),
    (28, 29),
    (28, 29),
    (28, 29),
    (28, 29),
    (36, 34),
    (36, 39),
    (36, 39),
    (36, 39),
    (36, 39),
    (36, 39),
    (36, 39),
    (36, 39),
    (36, 39),
];
pub const COMBAT_STATIC_SHA256: [&str; 35] = [
    "c4ab0a77ef7d7813cbafd46e39fe38dfa7106cfe0fae7e367ac76dff38143cc8",
    "53d228896686b080487f4b4c5535ac3181d5d29d26216eeb340ae4bad97d8832",
    "cf196e0ddbf8729f72a47cf2528aea639fe1feaa8ab8f11fac24da34c4f583f9",
    "c32401d5811d0d5801d88523b3f365a3c15b05d233a989547c3bfd490338ef48",
    "a100907c4a55b9f55dfa14ec9af4e109322776a647b746cd0d1ab9dccbfaa4e2",
    "0cf1c40320544ced140ed3df962fe3887616421f132889cac8c4dfc862594efc",
    "7ae480541a3847e7857010c8cdcb88efddbbf1910d3dde79c23aec12e2e86132",
    "8b07df7e6e883a04e1c4a5800fe792db83d44ad6880b074e33be1f5587639df0",
    "0d80b2a3059fdfc6d954a7444445da1ed7eddb564a00f64dc18eb65addc42ba7",
    "21edbdc033ef2ae7b7c11454b72e925f725f4bb3db140e0b1f8c837f92437580",
    "dfb1eb8f6b194a5c15e6b84f2fa3c6456dd91c3d3e1e284ac05da2ce39401972",
    "32968986931d7943759a4f9ad8fdd6fbf4eee520e15a937898841fc2b4febc98",
    "7064a0ea3cc4f8d268d48171bf964145b79f15db97bafd0c4d34bf6108f3d8d0",
    "b5c48fcd8e037bddc50d9d823312aed9d343b7cafdea243374bd4909b8acac87",
    "886083f15d38fbe664167b1939aac9702c9b95012def18b60611b2e1553207fe",
    "f89d6803a793b830603596473da27196d62e2a978173caaeeaef8e9e9ebd0233",
    "1e39d371ba9c20bd80764187364c3060495df2890dce579cd87dd2ee42e044a8",
    "9411ca81f21968805c43bd08dee0280e979e323e170dc2e78393afa7d8c436da",
    "cb4d10ee334bc20fd47e11028de424eb108e0d1161c8316fcac626e33b6823e0",
    "09b5d659287236605b513536cf52daae8bac9986368717c7ea3fd4475929c55f",
    "9dacceba89351e0622022c3f439e0bedd6fc324efa8b3d611f8c6b29f598cd8c",
    "080300030ccf89fc08177e30c6c4b2325491fce65987dd71ef5d2a31bb1d34d4",
    "4a564e421f5992fb8846324cf37e28dfad84348fa3c63eed5d6a246841cfc8fd",
    "21697e30753676503136215415b234bca3d592945977bd5f1ec5f14c9292830f",
    "92d4fd0c565ded09bae1f521ae0f773345eb81e6b0b82e2dfe2f4f596ee326db",
    "da72e2efcc793d8f78582e34b0719c78b5bbd3b522a2b5135ee1e7f65fbb04c4",
    "601e0703adf89d54d07c0e0dac2293e4a961e7b8d772300d1274b5d964b71e1f",
    "a743ae0be3456600f88fedadbf6462b9d5d808a1ac004e2be41e543a685d3d5c",
    "af8c3d1957907ad7577b2ce60fe774d820efd2f308cf073833d197753b663b22",
    "2c7f0e84962713229317adf0a22c7762c3c1b177b181203279788e326f3dca3c",
    "b0ab9f5b1e2f1a6d6e0e50e30f993fce2287e60d4113b61c3a679906b9922b47",
    "fa1cab2587daf3b5147f1736f151eb34a40d1201b21eb9afcc64dd061b0cc777",
    "266d7a0b496215e356d42e61c15a992b5750ac5920f30f4f09ca3d8d6491776d",
    "32688bcf566b98cf2f8d8fc4d6dbc02508d0befbd37070a00886e2a93fe6a60e",
    "68936134230e17e15f27e2e4e4075caaf9e48de28f13ac07cfd7bd39a0c9bdeb",
];
pub const FIREBALL_PAYLOAD_SHA256: &str =
    "cf185f7e35f9103be4b4ecfa2573fc9332ac8ade346b564be3971cbe73fb35ec";
pub const POSSESSION_PAYLOAD_SHA256: &str =
    "b7579df03c4ba78dc595b57ebb3c32188015bf1ad9b1dbfd24c2d2efb0d20fb1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CombatTmapFrame {
    pub selector: u16,
    pub state_ordinal: u8,
    pub frame_index: u8,
    pub stream_cursor: u32,
    pub width: u16,
    pub height: u16,
    pub pixels: Box<[u8]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CombatStaticTmapFrame {
    pub selector: u16,
    pub width: u16,
    pub height: u16,
    pub pixels: Box<[u8]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CombatTmapBank {
    fireball: [CombatTmapFrame; FIREBALL_STATE_COUNT],
    possession: [CombatTmapFrame; POSSESSION_STATE_COUNT],
    static_frames: [CombatStaticTmapFrame; COMBAT_STATIC_SELECTORS.len()],
}

impl CombatTmapBank {
    pub fn decode_qualified(
        fireball_payload: &[u8],
        possession_payload: &[u8],
        static_pixels: [&[u8]; COMBAT_STATIC_SELECTORS.len()],
    ) -> Result<Self, String> {
        require_hash(
            fireball_payload,
            FIREBALL_PAYLOAD_SHA256,
            "Fireball TMAP 463 payload",
        )?;
        let fireball = decode_fireball(fireball_payload)?;
        require_hash(
            possession_payload,
            POSSESSION_PAYLOAD_SHA256,
            "Possession TMAP 144 payload",
        )?;
        let possession = decode_possession(possession_payload)?;
        let mut frames = Vec::with_capacity(COMBAT_STATIC_SELECTORS.len());
        for ordinal in 0..COMBAT_STATIC_SELECTORS.len() {
            let selector = COMBAT_STATIC_SELECTORS[ordinal];
            let (width, height) = COMBAT_STATIC_EXTENTS[ordinal];
            let pixels = static_pixels[ordinal];
            let expected = usize::from(width) * usize::from(height);
            if pixels.len() != expected {
                return Err(format!(
                    "combat TMAP {selector} has {} pixels, expected {expected}",
                    pixels.len()
                ));
            }
            require_hash(
                pixels,
                COMBAT_STATIC_SHA256[ordinal],
                &format!("combat TMAP {selector}"),
            )?;
            frames.push(CombatStaticTmapFrame {
                selector,
                width,
                height,
                pixels: pixels.to_vec().into_boxed_slice(),
            });
        }
        Ok(Self {
            fireball,
            possession,
            static_frames: frames
                .try_into()
                .map_err(|_| "combat static selector count changed".to_owned())?,
        })
    }

    #[must_use]
    pub fn static_frame(&self, selector: u16) -> Option<&CombatStaticTmapFrame> {
        self.static_frames
            .iter()
            .find(|frame| frame.selector == selector)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PossessionTmapClock {
    state_ordinal: u8,
    started_by_draw: bool,
    frame: CombatTmapFrame,
}

impl PossessionTmapClock {
    #[must_use]
    pub fn from_bank(bank: &CombatTmapBank) -> Self {
        Self {
            state_ordinal: 0,
            started_by_draw: false,
            frame: bank.possession[0].clone(),
        }
    }

    #[must_use]
    pub const fn frame(&self) -> &CombatTmapFrame {
        &self.frame
    }

    pub fn latch_draw_attempt(&mut self, attempted: bool) {
        self.started_by_draw |= attempted;
    }

    pub fn service_unpaused_outer_turn(&mut self, bank: &CombatTmapBank) {
        if !self.started_by_draw {
            return;
        }
        // Selector 144 is a non-group TMAP. remc2 computes `word_0 & 0xf7`
        // without storing it, so descriptor bit 0x08 remains sticky after the
        // first admitted draw. State zero is therefore reset-only; after the
        // closure delta the next service reapplies delta one.
        self.state_ordinal = if usize::from(self.state_ordinal) + 1 >= POSSESSION_STATE_COUNT {
            1
        } else {
            self.state_ordinal + 1
        };
        self.frame = bank.possession[usize::from(self.state_ordinal)].clone();
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FireballTmapClock {
    state_ordinal: u8,
    started_by_draw: bool,
    frame: CombatTmapFrame,
}

impl FireballTmapClock {
    #[must_use]
    pub fn from_bank(bank: &CombatTmapBank) -> Self {
        Self {
            state_ordinal: 0,
            started_by_draw: false,
            frame: bank.fireball[0].clone(),
        }
    }

    #[must_use]
    pub const fn frame(&self) -> &CombatTmapFrame {
        &self.frame
    }

    pub fn latch_draw_attempt(&mut self, attempted: bool) {
        self.started_by_draw |= attempted;
    }

    pub fn service_unpaused_outer_turn(&mut self, bank: &CombatTmapBank) {
        if !self.started_by_draw {
            return;
        }
        // Selector 463 follows the same non-group sticky-descriptor path.
        self.state_ordinal = if usize::from(self.state_ordinal) + 1 >= FIREBALL_STATE_COUNT {
            1
        } else {
            self.state_ordinal + 1
        };
        self.frame = bank.fireball[usize::from(self.state_ordinal)].clone();
    }
}

fn decode_fireball(decoded: &[u8]) -> Result<[CombatTmapFrame; FIREBALL_STATE_COUNT], String> {
    let header = read_u16(decoded, 0, "Fireball TMAP header")?;
    let width = read_u16(decoded, 2, "Fireball TMAP width")?;
    let height = read_u16(decoded, 4, "Fireball TMAP height")?;
    if header != 5379 || (width, height) != (34, 35) {
        return Err(format!(
            "Fireball TMAP 463 header/extent changed: {header}, {width}x{height}"
        ));
    }
    let first_frame_end = 6 + usize::from(width) * usize::from(height);
    let frame_count = read_u16(decoded, first_frame_end, "Fireball TMAP frame count")?;
    let stream_size = read_u32(decoded, first_frame_end + 2, "Fireball TMAP stream size")?;
    if frame_count != 4 {
        return Err(format!(
            "Fireball TMAP 463 frame count is {frame_count}, expected 4"
        ));
    }
    let stream_end = first_frame_end
        + 6
        + usize::try_from(stream_size)
            .map_err(|_| "Fireball TMAP 463 stream size exceeds usize".to_owned())?;
    if stream_end != decoded.len() {
        return Err("Fireball TMAP 463 declared stream extent changed".to_owned());
    }
    let mut pixels = decoded[6..first_frame_end].to_vec();
    let mut cursor =
        u32::try_from(first_frame_end).map_err(|_| "Fireball cursor exceeds u32".to_owned())?;
    let mut states = Vec::with_capacity(FIREBALL_STATE_COUNT);
    states.push(CombatTmapFrame {
        selector: FIREBALL_SELECTOR,
        state_ordinal: 0,
        frame_index: 1,
        stream_cursor: cursor,
        width,
        height,
        pixels: pixels.clone().into_boxed_slice(),
    });
    for ordinal in 1..FIREBALL_STATE_COUNT {
        cursor = apply_ss2_delta(
            decoded,
            &mut pixels,
            "Fireball",
            FIREBALL_SELECTOR,
            width,
            height,
            cursor,
            stream_end,
        )?;
        states.push(CombatTmapFrame {
            selector: FIREBALL_SELECTOR,
            state_ordinal: ordinal as u8,
            frame_index: ordinal as u8 + 1,
            stream_cursor: cursor,
            width,
            height,
            pixels: pixels.clone().into_boxed_slice(),
        });
    }
    if usize::try_from(cursor)
        .ok()
        .and_then(|value| value.checked_add(6))
        != Some(stream_end)
        || states[4].pixels != states[0].pixels
    {
        return Err("Fireball TMAP 463 transition closure changed".to_owned());
    }
    states
        .try_into()
        .map_err(|_| "Fireball state count changed".to_owned())
}

fn decode_possession(decoded: &[u8]) -> Result<[CombatTmapFrame; POSSESSION_STATE_COUNT], String> {
    let header = read_u16(decoded, 0, "Possession TMAP header")?;
    let width = read_u16(decoded, 2, "Possession TMAP width")?;
    let height = read_u16(decoded, 4, "Possession TMAP height")?;
    if header != 5379 || (width, height) != (36, 30) {
        return Err(format!(
            "Possession TMAP 144 header/extent changed: {header}, {width}x{height}"
        ));
    }
    let first_frame_end = 6 + usize::from(width) * usize::from(height);
    let frame_count = read_u16(decoded, first_frame_end, "Possession TMAP frame count")?;
    let stream_size = read_u32(decoded, first_frame_end + 2, "Possession TMAP stream size")?;
    if frame_count != 8 {
        return Err(format!(
            "Possession TMAP 144 frame count is {frame_count}, expected 8"
        ));
    }
    let stream_end = first_frame_end
        + 6
        + usize::try_from(stream_size)
            .map_err(|_| "Possession TMAP 144 stream size exceeds usize".to_owned())?;
    if stream_end != decoded.len() {
        return Err("Possession TMAP 144 declared stream extent changed".to_owned());
    }
    let mut pixels = decoded[6..first_frame_end].to_vec();
    let mut cursor =
        u32::try_from(first_frame_end).map_err(|_| "Possession cursor exceeds u32".to_owned())?;
    let mut states = Vec::with_capacity(POSSESSION_STATE_COUNT);
    states.push(CombatTmapFrame {
        selector: POSSESSION_SELECTOR,
        state_ordinal: 0,
        frame_index: 1,
        stream_cursor: cursor,
        width,
        height,
        pixels: pixels.clone().into_boxed_slice(),
    });
    for ordinal in 1..POSSESSION_STATE_COUNT {
        cursor = apply_ss2_delta(
            decoded,
            &mut pixels,
            "Possession",
            POSSESSION_SELECTOR,
            width,
            height,
            cursor,
            stream_end,
        )?;
        states.push(CombatTmapFrame {
            selector: POSSESSION_SELECTOR,
            state_ordinal: ordinal as u8,
            frame_index: ordinal as u8 + 1,
            stream_cursor: cursor,
            width,
            height,
            pixels: pixels.clone().into_boxed_slice(),
        });
    }
    if usize::try_from(cursor)
        .ok()
        .and_then(|value| value.checked_add(6))
        != Some(stream_end)
        || states[8].pixels != states[0].pixels
    {
        return Err("Possession TMAP 144 transition closure changed".to_owned());
    }
    states
        .try_into()
        .map_err(|_| "Possession state count changed".to_owned())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn qualified_bank() -> CombatTmapBank {
        let cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level1-unity-asset-cache/terrain/tmaps1");
        let fireball =
            std::fs::read(cache.join("463.decoded.bin")).expect("qualified Fireball payload");
        let possession =
            std::fs::read(cache.join("144.decoded.bin")).expect("qualified Possession payload");
        let statics = COMBAT_STATIC_SELECTORS.map(|selector| {
            std::fs::read(cache.join(format!("{selector:03}.indices.u8")))
                .expect("qualified static combat frame")
        });
        CombatTmapBank::decode_qualified(
            &fireball,
            &possession,
            statics.each_ref().map(Vec::as_slice),
        )
        .expect("qualified combat bank")
    }

    #[test]
    fn possession_clock_freezes_until_first_draw_then_advances_stickily_and_wraps() {
        let bank = qualified_bank();
        let mut clock = PossessionTmapClock::from_bank(&bank);
        let base = clock.frame().clone();
        clock.service_unpaused_outer_turn(&bank);
        assert_eq!(clock.frame(), &base);

        clock.latch_draw_attempt(true);
        clock.service_unpaused_outer_turn(&bank);
        assert_eq!(clock.frame().state_ordinal, 1);
        for expected in 2..POSSESSION_STATE_COUNT {
            clock.service_unpaused_outer_turn(&bank);
            assert_eq!(usize::from(clock.frame().state_ordinal), expected);
        }
        assert_eq!(clock.frame().pixels, base.pixels);
        clock.service_unpaused_outer_turn(&bank);
        assert_eq!(clock.frame().state_ordinal, 1);
    }

    #[test]
    fn fireball_clock_uses_the_same_non_group_sticky_descriptor_bit() {
        let bank = qualified_bank();
        let mut clock = FireballTmapClock::from_bank(&bank);
        clock.latch_draw_attempt(true);
        for expected in 1..FIREBALL_STATE_COUNT {
            clock.service_unpaused_outer_turn(&bank);
            assert_eq!(usize::from(clock.frame().state_ordinal), expected);
        }
        clock.service_unpaused_outer_turn(&bank);
        assert_eq!(clock.frame().state_ordinal, 1);
    }
}
