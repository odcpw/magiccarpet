//! Qualified Level 1 castle and mana-economy TMAPs.
//!
//! Mana spheres and the carrier are immutable indexed sprites. Castle selector
//! 96 carries remc2's mutable-descriptor bit, but its only F1FA transition has
//! zero children and therefore changes no pixels. The clock remains explicit
//! so the Rust state preserves the source scheduler's sticky post-draw path.

use sim_ss2_tmap::{read_u16, read_u32, require_hash};

pub const MANA_SPHERE_NEUTRAL_SELECTOR: u16 = 58;
pub const MANA_SPHERE_PLAYER_ONE_SELECTOR: u16 = 67;
pub const MANA_CARRIER_SELECTOR: u16 = 88;
pub const CASTLE_ROOT_SELECTOR: u16 = 96;

pub const MANA_SPHERE_NEUTRAL_PAYLOAD_BYTES: usize = 838;
pub const MANA_SPHERE_PLAYER_ONE_PAYLOAD_BYTES: usize = 838;
pub const MANA_CARRIER_PAYLOAD_BYTES: usize = 4_878;
pub const CASTLE_ROOT_PAYLOAD_BYTES: usize = 1_432;

pub const MANA_SPHERE_NEUTRAL_PAYLOAD_SHA256: &str =
    "bdf9844a6ce126dc9f71048c76433e15e6fe8f88e6b8e2534e3221abf972c085";
pub const MANA_SPHERE_PLAYER_ONE_PAYLOAD_SHA256: &str =
    "244e98ceab342e5f12a6260a72a3968d2b030b5f1f4024b287ad8964f0940648";
pub const MANA_CARRIER_PAYLOAD_SHA256: &str =
    "714cf9e449baa90eb7a273e1f7443af8babc8b6fd73d4a7002cb82bee69d89bc";
pub const CASTLE_ROOT_PAYLOAD_SHA256: &str =
    "89bb8d3624427b3042dac2c1f95b18485f0a82bbc4508f4a8981d6ec901ee9ef";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EconomyTmapFrame {
    pub selector: u16,
    pub render_mode: u8,
    pub width: u16,
    pub height: u16,
    pub pixels: Box<[u8]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EconomyTmapBank {
    static_frames: [EconomyTmapFrame; 3],
    castle_frame: EconomyTmapFrame,
}

impl EconomyTmapBank {
    pub fn decode_qualified(
        neutral_mana: &[u8],
        player_one_mana: &[u8],
        carrier: &[u8],
        castle: &[u8],
    ) -> Result<Self, String> {
        let static_frames = [
            decode_static(
                neutral_mana,
                MANA_SPHERE_NEUTRAL_SELECTOR,
                0x1502,
                21,
                32,
                26,
                MANA_SPHERE_NEUTRAL_PAYLOAD_BYTES,
                MANA_SPHERE_NEUTRAL_PAYLOAD_SHA256,
            )?,
            decode_static(
                player_one_mana,
                MANA_SPHERE_PLAYER_ONE_SELECTOR,
                0x1502,
                21,
                32,
                26,
                MANA_SPHERE_PLAYER_ONE_PAYLOAD_BYTES,
                MANA_SPHERE_PLAYER_ONE_PAYLOAD_SHA256,
            )?,
            decode_static(
                carrier,
                MANA_CARRIER_SELECTOR,
                0x0002,
                0,
                58,
                84,
                MANA_CARRIER_PAYLOAD_BYTES,
                MANA_CARRIER_PAYLOAD_SHA256,
            )?,
        ];
        Ok(Self {
            static_frames,
            castle_frame: decode_castle(castle)?,
        })
    }

    #[must_use]
    pub fn frame(&self, selector: u16) -> Option<&EconomyTmapFrame> {
        if selector == CASTLE_ROOT_SELECTOR {
            Some(&self.castle_frame)
        } else {
            self.static_frames
                .iter()
                .find(|frame| frame.selector == selector)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CastleTmapClock {
    state_ordinal: u8,
    started_by_draw: bool,
    frame: EconomyTmapFrame,
}

impl CastleTmapClock {
    #[must_use]
    pub fn from_bank(bank: &EconomyTmapBank) -> Self {
        Self {
            state_ordinal: 0,
            started_by_draw: false,
            frame: bank.castle_frame.clone(),
        }
    }

    #[must_use]
    pub const fn state_ordinal(&self) -> u8 {
        self.state_ordinal
    }

    #[must_use]
    pub const fn started_by_draw(&self) -> bool {
        self.started_by_draw
    }

    #[must_use]
    pub const fn frame(&self) -> &EconomyTmapFrame {
        &self.frame
    }

    pub fn latch_draw_attempt(&mut self, attempted: bool) {
        self.started_by_draw |= attempted;
    }

    pub fn service_unpaused_outer_turn(&mut self, bank: &EconomyTmapBank) {
        if self.started_by_draw {
            // Selector 96 is non-group and its descriptor draw bit is sticky.
            // Its one transition is a zero-child F1FA record, so every service
            // after the first admitted draw remains in the same visual state.
            self.state_ordinal = 1;
            self.frame.clone_from(&bank.castle_frame);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn decode_static(
    payload: &[u8],
    selector: u16,
    expected_header: u16,
    expected_mode: u8,
    expected_width: u16,
    expected_height: u16,
    expected_bytes: usize,
    expected_hash: &str,
) -> Result<EconomyTmapFrame, String> {
    validate_identity(
        payload,
        selector,
        expected_header,
        expected_width,
        expected_height,
        expected_bytes,
        expected_hash,
    )?;
    let pixel_count = usize::from(expected_width) * usize::from(expected_height);
    let pixels = payload
        .get(6..6 + pixel_count)
        .ok_or_else(|| format!("economy TMAP {selector} pixels are truncated"))?;
    if payload.len() != 6 + pixel_count {
        return Err(format!("static economy TMAP {selector} has trailing bytes"));
    }
    Ok(EconomyTmapFrame {
        selector,
        render_mode: expected_mode,
        width: expected_width,
        height: expected_height,
        pixels: pixels.to_vec().into_boxed_slice(),
    })
}

fn decode_castle(payload: &[u8]) -> Result<EconomyTmapFrame, String> {
    validate_identity(
        payload,
        CASTLE_ROOT_SELECTOR,
        0x0003,
        36,
        39,
        CASTLE_ROOT_PAYLOAD_BYTES,
        CASTLE_ROOT_PAYLOAD_SHA256,
    )?;
    let pixel_count = 36 * 39;
    let first_frame_end = 6 + pixel_count;
    let frame_count = read_u16(payload, first_frame_end, "castle TMAP frame count")?;
    let stream_size = read_u32(payload, first_frame_end + 2, "castle TMAP stream size")?;
    let stream_start = first_frame_end + 6;
    if frame_count != 1 || stream_size != 16 || stream_start + 16 != payload.len() {
        return Err(format!(
            "castle TMAP 96 animation envelope changed: frames={frame_count}, bytes={stream_size}"
        ));
    }
    let outer_size = read_u32(payload, stream_start, "castle TMAP no-op outer size")?;
    let outer_tag = read_u16(payload, stream_start + 4, "castle TMAP no-op outer tag")?;
    let child_count = read_u16(payload, stream_start + 6, "castle TMAP no-op child count")?;
    if outer_size != 16
        || outer_tag != 0xf1fa
        || child_count != 0
        || payload[stream_start + 8..stream_start + 16] != [0; 8]
    {
        return Err("castle TMAP 96 no-op F1FA transition changed".to_owned());
    }
    Ok(EconomyTmapFrame {
        selector: CASTLE_ROOT_SELECTOR,
        render_mode: 0,
        width: 36,
        height: 39,
        pixels: payload[6..first_frame_end].to_vec().into_boxed_slice(),
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_identity(
    payload: &[u8],
    selector: u16,
    expected_header: u16,
    expected_width: u16,
    expected_height: u16,
    expected_bytes: usize,
    expected_hash: &str,
) -> Result<(), String> {
    if payload.len() != expected_bytes {
        return Err(format!(
            "economy TMAP {selector} has {} bytes, expected {expected_bytes}",
            payload.len()
        ));
    }
    require_hash(
        payload,
        expected_hash,
        &format!("economy TMAP {selector} payload"),
    )?;
    let header = read_u16(payload, 0, "economy TMAP header")?;
    let width = read_u16(payload, 2, "economy TMAP width")?;
    let height = read_u16(payload, 4, "economy TMAP height")?;
    if (header, width, height) != (expected_header, expected_width, expected_height) {
        return Err(format!(
            "economy TMAP {selector} header/extent changed: {header:#06x}, {width}x{height}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner_cache() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level1-unity-asset-cache/terrain/tmaps1")
    }

    fn qualified_bank() -> EconomyTmapBank {
        let root = owner_cache();
        EconomyTmapBank::decode_qualified(
            &std::fs::read(root.join("058.decoded.bin")).expect("neutral mana"),
            &std::fs::read(root.join("067.decoded.bin")).expect("owned mana"),
            &std::fs::read(root.join("088.decoded.bin")).expect("carrier"),
            &std::fs::read(root.join("096.decoded.bin")).expect("castle"),
        )
        .expect("qualified economy bank")
    }

    #[test]
    fn owner_payloads_pin_all_level_one_economy_selectors() {
        let bank = qualified_bank();
        assert_eq!(
            bank.frame(58)
                .map(|frame| (frame.render_mode, frame.width, frame.height)),
            Some((21, 32, 26))
        );
        assert_eq!(
            bank.frame(67)
                .map(|frame| (frame.render_mode, frame.width, frame.height)),
            Some((21, 32, 26))
        );
        assert_eq!(
            bank.frame(88)
                .map(|frame| (frame.render_mode, frame.width, frame.height)),
            Some((0, 58, 84))
        );
        assert_eq!(
            bank.frame(96)
                .map(|frame| (frame.render_mode, frame.width, frame.height)),
            Some((0, 36, 39))
        );
    }

    #[test]
    fn castle_clock_stays_reset_until_draw_then_services_sticky_no_op() {
        let bank = qualified_bank();
        let mut clock = CastleTmapClock::from_bank(&bank);
        clock.service_unpaused_outer_turn(&bank);
        assert_eq!(clock.state_ordinal(), 0);
        clock.latch_draw_attempt(true);
        clock.service_unpaused_outer_turn(&bank);
        assert_eq!(clock.state_ordinal(), 1);
        let pixels = clock.frame().pixels.clone();
        clock.service_unpaused_outer_turn(&bank);
        assert_eq!(clock.state_ordinal(), 1);
        assert_eq!(clock.frame().pixels, pixels);
    }

    #[test]
    fn castle_transition_envelope_is_not_silently_treated_as_static() {
        let root = owner_cache();
        let neutral = std::fs::read(root.join("058.decoded.bin")).expect("neutral mana");
        let owned = std::fs::read(root.join("067.decoded.bin")).expect("owned mana");
        let carrier = std::fs::read(root.join("088.decoded.bin")).expect("carrier");
        let mut castle = std::fs::read(root.join("096.decoded.bin")).expect("castle");
        let last = castle.len() - 1;
        castle[last] = 1;
        assert!(EconomyTmapBank::decode_qualified(&neutral, &owned, &carrier, &castle).is_err());
    }
}
