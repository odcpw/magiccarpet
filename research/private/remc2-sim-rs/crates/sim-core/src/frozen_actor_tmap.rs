//! Exact owner-data TMAPs and selector-global clocks for the immutable
//! Level 1 villager/dolmen actor slice.
//!
//! The 28 villagers retain frozen reset poses, but their selected mode-20
//! texture descriptors are global mutable SS2 streams. The three dolmens use
//! one immutable mode-0 frame.

use crate::ss2_tmap::{apply_ss2_delta, read_u16, read_u32, require_hash};

pub const DOLMEN_SELECTOR: u16 = 79;
pub const DOLMEN_RENDER_PARAMETER_INDEX: u16 = 39;
pub const DOLMEN_LOGICAL_HEIGHT: u16 = 400;
pub const DOLMEN_RENDER_MODE: u8 = 0;
pub const LEVEL1_FROZEN_ACTOR_ENTITY_INDICES: [u16; 31] = [
    51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74,
    75, 76, 77, 78, 130, 131, 132,
];

pub const VILLAGER_ANIMATED_SELECTORS: [u16; 12] =
    [155, 156, 157, 180, 181, 182, 183, 184, 185, 199, 200, 201];
pub const VILLAGER_TMAP_STATE_COUNT: usize = 5;
pub const VILLAGER_TMAP_DRAW_MASK: u16 = 0x0fff;

pub const DOLMEN_PAYLOAD_BYTES: usize = 7_266;
pub const DOLMEN_PAYLOAD_SHA256: &str =
    "f52541878b53270ffe0d84101396d3b4493ce2a0b777ec7c500a7bb9033ac167";
pub const VILLAGER_PAYLOAD_BYTES: [usize; VILLAGER_ANIMATED_SELECTORS.len()] = [
    3_288, 3_316, 3_246, 6_406, 6_170, 5_860, 6_396, 6_172, 5_854, 7_160, 7_364, 7_570,
];
pub const VILLAGER_PAYLOAD_SHA256: [&str; VILLAGER_ANIMATED_SELECTORS.len()] = [
    "1f94f8122e6efeae777de34e3a84459774f3d831212b3fbdd0554567165af1ea",
    "533fce163751e81ed0f4ffca83ab0d340e08f3bbc3482a94f9dc9fab69950a03",
    "1c98fb0e690a9aec12e0eacdd30bc1e20acc1d144019f2a724d2c4c542aa2064",
    "927b7c1abf129939770424def6318046bc59b667769162fb6d2baf743e0dec47",
    "5cfe773843501489d946471cc00c30676e1cdda1a293f9c4da2e3aac9580fa29",
    "f8ca03bbd940455c4f94f185184e0a45f5c3b545d8cdb1364f2b83dd04fdc5a5",
    "f528a49f0be70375ebc3faaacb46817139d855edf2123988aadd757f662bf4ad",
    "3b45176d758b018d5741bbd4952960ab6992ff7b984e3f2b7e613989fa122318",
    "d23a11d2942d003ff56e48939b18223fd295ff37ad1414134f42928041472a75",
    "c93285d847b170324f0352e1ba71a18530d9d99465202adee4c8f6cbbb0b8298",
    "c1838b5fd0d19a52af05865b32cb1f968609faa8b0de3aa5dc5a9d9271a7480e",
    "6ebe435e00b54a30878de31523bd44e5a537f6b2441b64ada498a09ab5d528b8",
];
const VILLAGER_EXTENTS: [(u16, u16); VILLAGER_ANIMATED_SELECTORS.len()] = [
    (18, 40),
    (24, 38),
    (18, 39),
    (26, 56),
    (32, 54),
    (24, 53),
    (26, 56),
    (32, 54),
    (24, 53),
    (24, 74),
    (24, 74),
    (24, 75),
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrozenActorTmapFrame {
    pub selector: u16,
    pub state_ordinal: u8,
    pub frame_index: u8,
    pub stream_cursor: u32,
    pub width: u16,
    pub height: u16,
    pub pixels: Box<[u8]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrozenActorTmapSelectorBank {
    pub selector: u16,
    pub payload_bytes: u32,
    pub states: [FrozenActorTmapFrame; VILLAGER_TMAP_STATE_COUNT],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DolmenTmapFrame {
    pub width: u16,
    pub height: u16,
    pub pixels: Box<[u8]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrozenActorTmapBank {
    dolmen: DolmenTmapFrame,
    villagers: [FrozenActorTmapSelectorBank; VILLAGER_ANIMATED_SELECTORS.len()],
}

impl FrozenActorTmapBank {
    /// Decode and identity-pin the exact Level 1 dolmen/villager payloads.
    ///
    /// # Errors
    ///
    /// Rejects changed hashes, headers, dimensions, SS2 records, transition
    /// counts, or streams whose fourth transition does not restore reset.
    pub fn decode_qualified(
        dolmen_payload: &[u8],
        villager_payloads: [&[u8]; VILLAGER_ANIMATED_SELECTORS.len()],
    ) -> Result<Self, String> {
        let dolmen = decode_dolmen(dolmen_payload)?;
        let mut villagers = Vec::with_capacity(VILLAGER_ANIMATED_SELECTORS.len());
        for ordinal in 0..VILLAGER_ANIMATED_SELECTORS.len() {
            let selector = VILLAGER_ANIMATED_SELECTORS[ordinal];
            let payload = villager_payloads[ordinal];
            if payload.len() != VILLAGER_PAYLOAD_BYTES[ordinal] {
                return Err(format!(
                    "villager TMAP {selector} has {} bytes, expected {}",
                    payload.len(),
                    VILLAGER_PAYLOAD_BYTES[ordinal]
                ));
            }
            require_hash(
                payload,
                VILLAGER_PAYLOAD_SHA256[ordinal],
                &format!("villager TMAP {selector} payload"),
            )?;
            villagers.push(decode_villager(
                selector,
                payload,
                VILLAGER_EXTENTS[ordinal],
            )?);
        }
        Ok(Self {
            dolmen,
            villagers: villagers
                .try_into()
                .map_err(|_| "villager selector count changed".to_owned())?,
        })
    }

    #[must_use]
    pub const fn dolmen(&self) -> &DolmenTmapFrame {
        &self.dolmen
    }

    #[must_use]
    pub const fn villagers(
        &self,
    ) -> &[FrozenActorTmapSelectorBank; VILLAGER_ANIMATED_SELECTORS.len()] {
        &self.villagers
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VillagerTmapClock {
    selector: u16,
    state_ordinal: u8,
    frame_index: u8,
    stream_cursor: u32,
    width: u16,
    height: u16,
    used_by_preceding_draw: bool,
    pixels: Box<[u8]>,
}

impl VillagerTmapClock {
    #[must_use]
    pub const fn selector(&self) -> u16 {
        self.selector
    }

    #[must_use]
    pub const fn state_ordinal(&self) -> u8 {
        self.state_ordinal
    }

    #[must_use]
    pub const fn used_by_preceding_draw(&self) -> bool {
        self.used_by_preceding_draw
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
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VillagerTmapClocks {
    clocks: [VillagerTmapClock; VILLAGER_ANIMATED_SELECTORS.len()],
}

impl VillagerTmapClocks {
    #[must_use]
    pub fn from_bank(bank: &FrozenActorTmapBank) -> Self {
        Self {
            clocks: std::array::from_fn(|ordinal| {
                let reset = &bank.villagers[ordinal].states[0];
                VillagerTmapClock {
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

    pub fn latch_draw_attempts(&mut self, mask: u16) -> Result<(), String> {
        if mask & !VILLAGER_TMAP_DRAW_MASK != 0 {
            return Err(format!(
                "villager TMAP draw-attempt mask {mask:#06x} has unsupported bits"
            ));
        }
        for (ordinal, clock) in self.clocks.iter_mut().enumerate() {
            clock.used_by_preceding_draw |= mask & (1_u16 << ordinal) != 0;
        }
        Ok(())
    }

    pub fn service_unpaused_outer_turn(&mut self, bank: &FrozenActorTmapBank) {
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
            let next = &bank.villagers[ordinal].states[usize::from(next_ordinal)];
            clock.state_ordinal = next.state_ordinal;
            clock.frame_index = next.frame_index;
            clock.stream_cursor = next.stream_cursor;
            clock.width = next.width;
            clock.height = next.height;
            clock.pixels.clone_from(&next.pixels);
        }
    }

    #[must_use]
    pub fn frame(&self, selector: u16) -> Option<&VillagerTmapClock> {
        self.clocks.iter().find(|clock| clock.selector == selector)
    }

    #[must_use]
    pub const fn states(&self) -> &[VillagerTmapClock; VILLAGER_ANIMATED_SELECTORS.len()] {
        &self.clocks
    }
}

#[must_use]
pub fn villager_selector_mask(selector: u16) -> Option<u16> {
    VILLAGER_ANIMATED_SELECTORS
        .iter()
        .position(|candidate| *candidate == selector)
        .map(|ordinal| 1_u16 << ordinal)
}

fn decode_dolmen(payload: &[u8]) -> Result<DolmenTmapFrame, String> {
    if payload.len() != DOLMEN_PAYLOAD_BYTES {
        return Err(format!(
            "dolmen TMAP 79 has {} bytes, expected {DOLMEN_PAYLOAD_BYTES}",
            payload.len()
        ));
    }
    require_hash(payload, DOLMEN_PAYLOAD_SHA256, "dolmen TMAP 79 payload")?;
    let header = read_u16(payload, 0, "dolmen header")?;
    let width = read_u16(payload, 2, "dolmen width")?;
    let height = read_u16(payload, 4, "dolmen height")?;
    if (header, width, height) != (2, 110, 66) {
        return Err(format!(
            "dolmen TMAP 79 header/extent changed: {header}, {width}x{height}"
        ));
    }
    let pixels = payload
        .get(6..)
        .ok_or_else(|| "dolmen TMAP 79 pixels are absent".to_owned())?;
    if pixels.len() != usize::from(width) * usize::from(height) {
        return Err("dolmen TMAP 79 pixel extent changed".to_owned());
    }
    Ok(DolmenTmapFrame {
        width,
        height,
        pixels: pixels.into(),
    })
}

fn decode_villager(
    selector: u16,
    payload: &[u8],
    expected_extent: (u16, u16),
) -> Result<FrozenActorTmapSelectorBank, String> {
    let header = read_u16(payload, 0, "villager header")?;
    let width = read_u16(payload, 2, "villager width")?;
    let height = read_u16(payload, 4, "villager height")?;
    if header != 5123 || (width, height) != expected_extent {
        return Err(format!(
            "villager TMAP {selector} header/extent changed: {header}, {width}x{height}"
        ));
    }
    let first_frame_end = 6_usize
        .checked_add(usize::from(width) * usize::from(height))
        .ok_or_else(|| format!("villager TMAP {selector} base extent overflow"))?;
    let frame_count = read_u16(payload, first_frame_end, "villager frame count")?;
    let stream_size = read_u32(payload, first_frame_end + 2, "villager stream size")?;
    if frame_count != 4 {
        return Err(format!(
            "villager TMAP {selector} frame count is {frame_count}, expected 4"
        ));
    }
    let stream_start = first_frame_end + 6;
    let stream_end = stream_start
        .checked_add(
            usize::try_from(stream_size)
                .map_err(|_| format!("villager TMAP {selector} stream size exceeds usize"))?,
        )
        .ok_or_else(|| format!("villager TMAP {selector} stream extent overflow"))?;
    if stream_end != payload.len() {
        return Err(format!(
            "villager TMAP {selector} declared stream ends at {stream_end}, payload has {} bytes",
            payload.len()
        ));
    }
    let mut pixels = payload
        .get(6..first_frame_end)
        .ok_or_else(|| format!("villager TMAP {selector} base frame is truncated"))?
        .to_vec();
    let mut cursor = u32::try_from(first_frame_end)
        .map_err(|_| format!("villager TMAP {selector} cursor exceeds u32"))?;
    let mut states = Vec::with_capacity(VILLAGER_TMAP_STATE_COUNT);
    states.push(FrozenActorTmapFrame {
        selector,
        state_ordinal: 0,
        frame_index: 1,
        stream_cursor: cursor,
        width,
        height,
        pixels: pixels.clone().into_boxed_slice(),
    });
    for state_ordinal in 1..VILLAGER_TMAP_STATE_COUNT {
        cursor = apply_ss2_delta(
            payload,
            &mut pixels,
            "villager",
            selector,
            width,
            height,
            cursor,
            stream_end,
        )?;
        states.push(FrozenActorTmapFrame {
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
        .and_then(|value| value.checked_add(6))
        != Some(stream_end)
    {
        return Err(format!(
            "villager TMAP {selector} has missing or extra transition records"
        ));
    }
    if states[4].pixels != states[0].pixels {
        return Err(format!(
            "villager TMAP {selector} fourth transition does not restore reset"
        ));
    }
    Ok(FrozenActorTmapSelectorBank {
        selector,
        payload_bytes: u32::try_from(payload.len())
            .map_err(|_| format!("villager TMAP {selector} payload exceeds u32"))?,
        states: states
            .try_into()
            .map_err(|_| format!("villager TMAP {selector} state count changed"))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner_cache() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level1-unity-asset-cache/terrain/tmaps1")
    }

    fn bank() -> FrozenActorTmapBank {
        let root = owner_cache();
        let dolmen = std::fs::read(root.join("079.decoded.bin")).expect("dolmen payload");
        let payloads = VILLAGER_ANIMATED_SELECTORS.map(|selector| {
            std::fs::read(root.join(format!("{selector:03}.decoded.bin")))
                .expect("villager payload")
        });
        FrozenActorTmapBank::decode_qualified(
            &dolmen,
            std::array::from_fn(|ordinal| payloads[ordinal].as_slice()),
        )
        .expect("qualified frozen actor bank")
    }

    #[test]
    fn qualified_payloads_pin_dolmen_and_all_villager_streams() {
        let bank = bank();
        assert_eq!((bank.dolmen.width, bank.dolmen.height), (110, 66));
        assert_eq!(bank.villagers.len(), 12);
        for (ordinal, selector) in VILLAGER_ANIMATED_SELECTORS.into_iter().enumerate() {
            assert_eq!(bank.villagers[ordinal].selector, selector);
            assert_eq!(
                bank.villagers[ordinal].states[4].pixels,
                bank.villagers[ordinal].states[0].pixels
            );
        }
    }

    #[test]
    fn selector_global_clocks_advance_once_freeze_and_wrap_to_one() {
        let bank = bank();
        let mut clocks = VillagerTmapClocks::from_bank(&bank);
        let reset = clocks.clone();
        clocks.service_unpaused_outer_turn(&bank);
        assert_eq!(clocks, reset);

        let mask = villager_selector_mask(155).expect("selector 155 mask");
        clocks.latch_draw_attempts(mask).expect("valid mask");
        clocks.latch_draw_attempts(mask).expect("idempotent latch");
        clocks.service_unpaused_outer_turn(&bank);
        assert_eq!(clocks.frame(155).expect("clock").state_ordinal(), 1);
        assert_eq!(clocks.frame(156).expect("clock").state_ordinal(), 0);

        for expected in [2, 3, 4, 1] {
            clocks.latch_draw_attempts(mask).expect("valid mask");
            clocks.service_unpaused_outer_turn(&bank);
            assert_eq!(clocks.frame(155).expect("clock").state_ordinal(), expected);
        }
        assert!(!clocks.frame(155).expect("clock").used_by_preceding_draw());
        assert!(clocks.latch_draw_attempts(0x8000).is_err());
    }
}
