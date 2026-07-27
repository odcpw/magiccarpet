//! Exact selector-global TMAP state for the Level 1 Firefly mode-17 family.
//!
//! Selectors 279..286 are independent mutable six-transition SS2 streams.
//! Rendering one facing latches only that selector; the following unpaused
//! outer turn advances its shared descriptor. Transition six restores the
//! reset pixels, after which remc2 restarts from transition one.

use crate::ss2_tmap::{apply_ss2_delta, read_u16, read_u32, require_hash};

pub const FIREFLY_ANIMATED_SELECTORS: [u16; 8] = [279, 280, 281, 282, 283, 284, 285, 286];
pub const FIREFLY_TMAP_DRAW_MASK: u8 = u8::MAX;
pub const FIREFLY_TMAP_STATE_COUNT: usize = 7;

const QUALIFIED_PAYLOAD_SHA256: [&str; 8] = [
    "d4ffec8ae18edb62f68441ecd29cf375b6873af3cf98a780bc3539f11b5b1387",
    "c334d13b8adf0d274452643760d58a2bcb5527241cd5d8a9ed0ae3550a77b7c9",
    "fbfb64916d1d8e7b3c460b0235d47f227603e434d8b6bed808d2d4079986a9e7",
    "f91014fba453529488ca0e49f3643622eb8485c1e2d45e631a8bfd1d18d0c720",
    "3566b01e55bc04862eb40cfd20150e7eb03cf04f842d37078a245521d225df8d",
    "8f75ffc75929f75364445a32c59e04a9b335e3f4d9d96388a106939987f0b4d0",
    "ae00740ed6c6d188d369aad1ef95d062ec3d301ecdf7f0413d8b24c9086c367b",
    "8095182d4842a4b04f2f02e3937225599bcd2fc50d73ef0ea1170024d5b6b283",
];
const QUALIFIED_EXTENTS: [(u16, u16); 8] = [
    (66, 54),
    (62, 57),
    (60, 59),
    (64, 61),
    (62, 61),
    (62, 62),
    (64, 64),
    (72, 63),
];
const QUALIFIED_STATE_SHA256: [[&str; FIREFLY_TMAP_STATE_COUNT]; 8] = [
    [
        "5f463bb2badb27777af022f03c75183063cb309263619879ee8263b3f29a4ff3",
        "14aa044e58329d76f3b39b85a785d0f6ce6c51d019b86648a7334b3436c8f21b",
        "0587dede5bfc076a4060ecd926d780db9221a3263a01412ef37325c628fc3930",
        "84a6361650987920f5ea0913ee5eb556d2fad94af31c1cc891c4d906bcd1d5e5",
        "ab5b08041c824b6f9e3c1873e16919d448c46315f7cc470b93a3de4ecf8fe44e",
        "8c40e5729d75f4e7e59ae667ea3bdc4e8806256b86ffa3aa56552ff1424cfffe",
        "5f463bb2badb27777af022f03c75183063cb309263619879ee8263b3f29a4ff3",
    ],
    [
        "c01a8f9ee104e32b1417fdbc1cea62a91315bd6c7d1856722640001d22aad3f3",
        "4f6e32367b516100b6224856971cea9c90444fc02f6c0447fad651b479ff9f51",
        "6804e54d4ce1f597f444ab6b281ddb093578bb183f50788643da284bb636d3c7",
        "175936a8f3b1ee1677a766b5a240ab71b5dea89ca469de755ba25c6e4ed7dc44",
        "8fee37f94664fb036dda3cc4a297abc09891fb52ca52259ef42141576074c5aa",
        "80c5f81054ecbd17f68fa290600e87b6042ffa2fae0fd6b46d04414c6e5cf4bf",
        "c01a8f9ee104e32b1417fdbc1cea62a91315bd6c7d1856722640001d22aad3f3",
    ],
    [
        "a84c0606f5627192da972f71985286445c7ffc62538982c701a41c414968218b",
        "a222342186fa442ae9406de7cf7d753c559e422b339d5bda70fd530549255269",
        "e1f9243d9e545f17e0a0d64e848b24a2d9fc294085bb167677dcbce49f4cf026",
        "4fce96a951556aa84b19f30b458768e72f2efe998e617c6d1c307f5a92e06b5a",
        "63049e84d7f4a713faf6acd02b2ce74dbc7efb5aa1fe48fec8514f90d2b22b46",
        "b0c42e06da40b7c6069bb8208c3634bc5a6e81c68c187dacf70d6611f750ebe7",
        "a84c0606f5627192da972f71985286445c7ffc62538982c701a41c414968218b",
    ],
    [
        "adbc08a6e9f80bd0e0eeda3cd4067fa2eebe6847496195ef13d1361c5f94179c",
        "419d4e08828f130be9ed14f0a7a0382abc2364ec2ba3eb5fea2920316b25ff7f",
        "ec1a6afa5b41e9c734212d6d35a9c72628659f36df03f4209c64f0a599dbd787",
        "0cb3d1b01f5e9d2f7d898f9b8ef3f80b4361d01c658ea5d3f8d00802fa8f9f50",
        "8c29b780ba066df2ce098127c28d1b5ce0553a07b688cd5b0b0e23c315f4ad1f",
        "56e814b6acb32c70e8e9155abc62a56879b4a377b2c9d2c6d97d06dfc1c57a6c",
        "adbc08a6e9f80bd0e0eeda3cd4067fa2eebe6847496195ef13d1361c5f94179c",
    ],
    [
        "dc9229c38f55549556ae0494a9a7dba48f12498ff5fc4eefd4a437a034a7c980",
        "fdfe5c7fffbf40a76b3c48b7f722ec7918277804b27c3de91d9d96ad1ecc02e1",
        "3d9c039f0f3e2ee5c0052d5c21f1766c21f496effaa7980b3cf2f426f3aa9a0d",
        "e49f09deb993eed292ddf40689055d91210220c13cf6e6acfe6d66d6df4a9ed4",
        "c2a3d6b58c4549dff58e9a281041e377938e935ee1477adf50274df84188e14f",
        "df3b7c514ee47a8f1a05388cb4e2462bca41b67cf66d5ff64b8d077eed5bc2c2",
        "dc9229c38f55549556ae0494a9a7dba48f12498ff5fc4eefd4a437a034a7c980",
    ],
    [
        "084786718e6d98a99bd499b0ed7d93f18cc177c4a03e52263717b4f6003dd77a",
        "6fe5d79b3c7d58ab1ccd09e79833b4eae4a24e24f89b6e836b9ad1e8a5ceaa14",
        "c0120b8a368467e6190e97028b6dac7b4f83f6fa06c3ce5674df98d665ec91ff",
        "5bea72efbd1f88fe6659e2eaed38d6282b46b6b3e11f13af76f4dc7a82566a59",
        "5a06eee895e494818b5bb33f63daaa0508c71e24cfbf7f5e978ffadc002003e1",
        "e7cd5570dcd3c4d96cdbd9b117043348a06929f06aec69f8d090f28251e958f5",
        "084786718e6d98a99bd499b0ed7d93f18cc177c4a03e52263717b4f6003dd77a",
    ],
    [
        "a26eecdd4432122934584f8672cbca9757e9abef9d081953e025a9c634c38af0",
        "391f9acbf3de0d0cf1d328805dbc755eecbbcd6985c6b55b86f5cda000aae6b4",
        "0c31cd49742bc71835616f535bedbd1097f4e677c90fa2c3ff3a9f2099536cf4",
        "7d8e6027f385748beff1f2e21c2d0d86c5a558aec7d90055e784bc63abce99c1",
        "5402147e0b4000f61ed3d3f791be1b817b3d0ad47a3ebe953948429f6ed38619",
        "74979077254e81efb1e4ecaa735db36623338159247f612d5a7a4fdeaa22b23f",
        "a26eecdd4432122934584f8672cbca9757e9abef9d081953e025a9c634c38af0",
    ],
    [
        "f5e292252d0a463778d6d31b9cac384b781cfa5dc170e0df7726540288e1be39",
        "5de4db24f1be605f9eade675442fcc5f37c09e65d7b4538f2b658180ae3640f0",
        "3da952f2760b26b9068c012a19915378baf8bd65186804417b906b5bb6b0b360",
        "0a8cf6e97244cb8a234df778dad9a08b121b3728f422cf95b95fbcfef96464c1",
        "ea70dc781e1a096c2351aee85d1d4a8ec1702e1c9f4e2eae8d9b2c03d41d3544",
        "1dc4b6ece0fac97a65fe2d23dc13f3c552a7fba6f40d68ee9a3d5b431ae92888",
        "f5e292252d0a463778d6d31b9cac384b781cfa5dc170e0df7726540288e1be39",
    ],
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FireflyTmapFrame {
    pub selector: u16,
    pub state_ordinal: u8,
    pub frame_index: u8,
    pub stream_cursor: u32,
    pub width: u16,
    pub height: u16,
    pub pixels: Box<[u8]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FireflyTmapSelectorBank {
    pub selector: u16,
    pub payload_bytes: u32,
    pub states: [FireflyTmapFrame; FIREFLY_TMAP_STATE_COUNT],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FireflyTmapBank {
    animated: [FireflyTmapSelectorBank; FIREFLY_ANIMATED_SELECTORS.len()],
}

impl FireflyTmapBank {
    /// Decode the eight exact qualified owner-cache Firefly payloads.
    ///
    /// # Errors
    ///
    /// Rejects any identity, extent, SS2 opcode, transition, or closure that
    /// differs from the qualified Level 1 assets.
    pub fn decode_qualified(
        payloads: [&[u8]; FIREFLY_ANIMATED_SELECTORS.len()],
    ) -> Result<Self, String> {
        let mut animated = Vec::with_capacity(FIREFLY_ANIMATED_SELECTORS.len());
        for ordinal in 0..FIREFLY_ANIMATED_SELECTORS.len() {
            let selector = FIREFLY_ANIMATED_SELECTORS[ordinal];
            require_hash(
                payloads[ordinal],
                QUALIFIED_PAYLOAD_SHA256[ordinal],
                &format!("Firefly animated TMAP {selector} payload"),
            )?;
            let selector_bank =
                decode_animated_selector(selector, payloads[ordinal], QUALIFIED_EXTENTS[ordinal])?;
            for (state, expected_hash) in selector_bank
                .states
                .iter()
                .zip(QUALIFIED_STATE_SHA256[ordinal])
            {
                require_hash(
                    &state.pixels,
                    expected_hash,
                    &format!(
                        "Firefly TMAP {} state {}",
                        state.selector, state.state_ordinal
                    ),
                )?;
            }
            animated.push(selector_bank);
        }
        Ok(Self {
            animated: animated
                .try_into()
                .map_err(|_| "Firefly animated selector count changed".to_owned())?,
        })
    }

    #[must_use]
    pub const fn animated(&self) -> &[FireflyTmapSelectorBank; FIREFLY_ANIMATED_SELECTORS.len()] {
        &self.animated
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FireflyTmapClock {
    selector: u16,
    state_ordinal: u8,
    frame_index: u8,
    stream_cursor: u32,
    width: u16,
    height: u16,
    used_by_preceding_draw: bool,
    pixels: Box<[u8]>,
}

impl FireflyTmapClock {
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FireflyTmapClocks {
    clocks: [FireflyTmapClock; FIREFLY_ANIMATED_SELECTORS.len()],
}

impl FireflyTmapClocks {
    #[must_use]
    pub fn from_bank(bank: &FireflyTmapBank) -> Self {
        Self {
            clocks: std::array::from_fn(|ordinal| {
                let reset = &bank.animated[ordinal].states[0];
                FireflyTmapClock {
                    selector: reset.selector,
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

    pub fn latch_draw_attempts(&mut self, mask: u8) {
        for (ordinal, clock) in self.clocks.iter_mut().enumerate() {
            clock.used_by_preceding_draw |= mask & (1 << ordinal) != 0;
        }
    }

    pub fn service_unpaused_outer_turn(&mut self, bank: &FireflyTmapBank) {
        for (ordinal, clock) in self.clocks.iter_mut().enumerate() {
            if !clock.used_by_preceding_draw {
                continue;
            }
            clock.used_by_preceding_draw = false;
            let next_ordinal = if clock.state_ordinal >= 6 {
                1
            } else {
                clock.state_ordinal + 1
            };
            let next = &bank.animated[ordinal].states[usize::from(next_ordinal)];
            clock.state_ordinal = next.state_ordinal;
            clock.frame_index = next.frame_index;
            clock.stream_cursor = next.stream_cursor;
            clock.width = next.width;
            clock.height = next.height;
            clock.pixels.clone_from(&next.pixels);
        }
    }

    #[must_use]
    pub fn frame(&self, selector: u16) -> Option<&FireflyTmapClock> {
        self.clocks.iter().find(|clock| clock.selector == selector)
    }

    #[must_use]
    pub const fn states(&self) -> &[FireflyTmapClock; FIREFLY_ANIMATED_SELECTORS.len()] {
        &self.clocks
    }
}

fn decode_animated_selector(
    selector: u16,
    decoded: &[u8],
    expected_extent: (u16, u16),
) -> Result<FireflyTmapSelectorBank, String> {
    let header_word = read_u16(decoded, 0, "TMAP header")?;
    let width = read_u16(decoded, 2, "TMAP width")?;
    let height = read_u16(decoded, 4, "TMAP height")?;
    if header_word != 4355 || (width, height) != expected_extent {
        return Err(format!(
            "Firefly TMAP {selector} header/extent changed: {header_word}, {width}x{height}"
        ));
    }
    let first_frame_end = 6_usize
        .checked_add(usize::from(width) * usize::from(height))
        .ok_or_else(|| format!("Firefly TMAP {selector} base extent overflow"))?;
    let frame_count = read_u16(decoded, first_frame_end, "TMAP frame count")?;
    let stream_size = read_u32(decoded, first_frame_end + 2, "TMAP stream size")?;
    if frame_count != 6 {
        return Err(format!(
            "Firefly TMAP {selector} frame count is {frame_count}, expected 6"
        ));
    }
    let stream_start = first_frame_end + 6;
    let stream_end = stream_start
        .checked_add(
            usize::try_from(stream_size)
                .map_err(|_| format!("Firefly TMAP {selector} stream size exceeds usize"))?,
        )
        .ok_or_else(|| format!("Firefly TMAP {selector} stream extent overflow"))?;
    if stream_end != decoded.len() {
        return Err(format!(
            "Firefly TMAP {selector} declared stream ends at {stream_end}, payload has {} bytes",
            decoded.len()
        ));
    }

    let mut pixels = decoded
        .get(6..first_frame_end)
        .ok_or_else(|| format!("Firefly TMAP {selector} base frame is truncated"))?
        .to_vec();
    let mut cursor = u32::try_from(first_frame_end)
        .map_err(|_| format!("Firefly TMAP {selector} cursor exceeds u32"))?;
    let mut states = Vec::with_capacity(FIREFLY_TMAP_STATE_COUNT);
    states.push(FireflyTmapFrame {
        selector,
        state_ordinal: 0,
        frame_index: 1,
        stream_cursor: cursor,
        width,
        height,
        pixels: pixels.clone().into_boxed_slice(),
    });
    for state_ordinal in 1..FIREFLY_TMAP_STATE_COUNT {
        cursor = apply_ss2_delta(
            decoded,
            &mut pixels,
            "Firefly",
            selector,
            width,
            height,
            cursor,
            stream_end,
        )?;
        states.push(FireflyTmapFrame {
            selector,
            state_ordinal: state_ordinal as u8,
            frame_index: state_ordinal as u8 + 1,
            stream_cursor: cursor,
            width,
            height,
            pixels: pixels.clone().into_boxed_slice(),
        });
    }
    if usize::try_from(cursor)
        .ok()
        .and_then(|cursor| cursor.checked_add(6))
        != Some(stream_end)
    {
        return Err(format!(
            "Firefly TMAP {selector} has missing or extra transition records"
        ));
    }
    if states[6].pixels != states[0].pixels {
        return Err(format!(
            "Firefly TMAP {selector} sixth transition does not restore reset"
        ));
    }
    Ok(FireflyTmapSelectorBank {
        selector,
        payload_bytes: u32::try_from(decoded.len())
            .map_err(|_| format!("Firefly TMAP {selector} payload exceeds u32"))?,
        states: states
            .try_into()
            .map_err(|_| format!("Firefly TMAP {selector} state count changed"))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_bank() -> FireflyTmapBank {
        FireflyTmapBank {
            animated: std::array::from_fn(|selector_ordinal| FireflyTmapSelectorBank {
                selector: FIREFLY_ANIMATED_SELECTORS[selector_ordinal],
                payload_bytes: 1,
                states: std::array::from_fn(|state_ordinal| FireflyTmapFrame {
                    selector: FIREFLY_ANIMATED_SELECTORS[selector_ordinal],
                    state_ordinal: state_ordinal as u8,
                    frame_index: state_ordinal as u8 + 1,
                    stream_cursor: state_ordinal as u32,
                    width: 1,
                    height: 1,
                    pixels: vec![state_ordinal as u8].into_boxed_slice(),
                }),
            }),
        }
    }

    #[test]
    fn selector_clocks_latch_independently_and_wrap_from_closure_to_one() {
        let bank = synthetic_bank();
        let mut clocks = FireflyTmapClocks::from_bank(&bank);
        for expected in [1, 2, 3, 4, 5, 6, 1] {
            clocks.latch_draw_attempts(0b1000_0001);
            clocks.service_unpaused_outer_turn(&bank);
            assert_eq!(clocks.states()[0].state_ordinal(), expected);
            assert_eq!(clocks.states()[7].state_ordinal(), expected);
            assert_eq!(clocks.states()[1].state_ordinal(), 0);
        }
    }

    #[test]
    fn repeated_draws_before_service_latch_only_once() {
        let bank = synthetic_bank();
        let mut clocks = FireflyTmapClocks::from_bank(&bank);
        clocks.latch_draw_attempts(0b0000_0100);
        clocks.latch_draw_attempts(0b0000_0100);
        clocks.service_unpaused_outer_turn(&bank);
        assert_eq!(clocks.states()[2].state_ordinal(), 1);
        clocks.service_unpaused_outer_turn(&bank);
        assert_eq!(clocks.states()[2].state_ordinal(), 1);
    }
}
