//! Qualified immutable TMAPs for source-ordered Level 1 mission props.
//!
//! These records are fixed-selector sprites. They deliberately do not enter
//! the mutable selector-frame ABI used by SS2 animation clocks.

use crate::ss2_tmap::{read_u16, require_hash};

pub const SPELL_JAR_SELECTOR: u16 = 59;
pub const EXIT_TARGET_SELECTOR: u16 = 461;
pub const LEVEL1_SECONDARY_SELECTORS: [u16; 10] = [30, 31, 32, 33, 34, 35, 36, 37, 66, 259];
pub const LEVEL1_SCENERY_SELECTORS: [u16; 6] = [57, 75, 78, 87, 178, 179];
pub const LEVEL1_AMBIENT_SELECTORS: [u16; 3] = [38, 54, 63];

pub const SPELL_JAR_PAYLOAD_BYTES: usize = 838;
pub const EXIT_TARGET_PAYLOAD_BYTES: usize = 6_516;

pub const SPELL_JAR_PAYLOAD_SHA256: &str =
    "2361483b4b57ded90bbe62de9ad4e7ef55d7939c0395a8afbf7f0f6e7faace99";
pub const EXIT_TARGET_PAYLOAD_SHA256: &str =
    "15a28ac12ed08f0cdc38b3c9243d979aabe2ddbf95636ad9c810e6096f564eed";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticWorldTmapFrame {
    pub selector: u16,
    pub render_mode: u8,
    pub width: u16,
    pub height: u16,
    pub pixels: Box<[u8]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticWorldTmapBank {
    frames: Vec<StaticWorldTmapFrame>,
}

impl StaticWorldTmapBank {
    pub fn decode_qualified(
        spell_jar_payload: &[u8],
        exit_target_payload: &[u8],
    ) -> Result<Self, String> {
        Ok(Self {
            frames: vec![
                decode_static(
                    spell_jar_payload,
                    SPELL_JAR_SELECTOR,
                    0x0102,
                    1,
                    32,
                    26,
                    SPELL_JAR_PAYLOAD_BYTES,
                    SPELL_JAR_PAYLOAD_SHA256,
                )?,
                decode_static(
                    exit_target_payload,
                    EXIT_TARGET_SELECTOR,
                    0x0002,
                    0,
                    70,
                    93,
                    EXIT_TARGET_PAYLOAD_BYTES,
                    EXIT_TARGET_PAYLOAD_SHA256,
                )?,
            ],
        })
    }

    /// Decode the established objective props plus the exact owner payloads
    /// needed by the delayed Level 1 group-33/34 chain. Animated head TMAPs
    /// expose their reset image here; their mutable stream clock remains an
    /// explicit fidelity gap until it is ported.
    pub fn decode_with_level1_secondary(
        spell_jar_payload: &[u8],
        exit_target_payload: &[u8],
        secondary_payloads: [&[u8]; LEVEL1_SECONDARY_SELECTORS.len()],
    ) -> Result<Self, String> {
        let mut bank = Self::decode_qualified(spell_jar_payload, exit_target_payload)?;
        const HEAD_EXTENTS: [(u16, u16); 8] = [
            (96, 25),
            (86, 26),
            (76, 29),
            (70, 32),
            (70, 34),
            (76, 35),
            (88, 36),
            (92, 35),
        ];
        const PAYLOAD_BYTES: [usize; 10] = [
            4_554, 4_396, 4_682, 5_620, 6_540, 7_568, 8_518, 8_364, 838, 390,
        ];
        const PAYLOAD_SHA256: [&str; 10] = [
            "7bfe13fdf90e9b918978e71e6005f47023f02fc442092d3fc529b07d72aeaa61",
            "6a8477b50f3e94952d8bcd12e5d94c231bb2cf7dad6a79160bd4a009943dc1da",
            "c0888ec7eb007505164ff023d14d15965f7c8daf4dc4c57e79d2f77d5ecf0f2d",
            "1122f71dfccff095374437ee3ebde1c02264ccc8d9ab73656f04e615a9fa54c4",
            "b151b32a7c1a91da6eef5d0c367b6b6ac4077ca0dc985da1a3fef5cb695ccb16",
            "46e16ca7306522daa7342a757bf2cae1b113bb12366fd7b2119f67a2bdaca532",
            "58161104b22bf8cc65f34f43e0f20621afec960ca51e20f34f41fa7af095a308",
            "70ff354f99edd18057045db7de7bcaa355b46a7d600c40318b671e10d5078518",
            "b4bea807840a76ea6d1bfb7437d1f8d98440421775f2d82d7e48c7792fa279ec",
            "440d25091028633628f18cc68e4baeb6b8f05ad8f11a5ea553265913bd360504",
        ];
        for ordinal in 0..LEVEL1_SECONDARY_SELECTORS.len() {
            let selector = LEVEL1_SECONDARY_SELECTORS[ordinal];
            let (header, mode, width, height) = if ordinal < HEAD_EXTENTS.len() {
                let (width, height) = HEAD_EXTENTS[ordinal];
                (0x1103, 17, width, height)
            } else if selector == 66 {
                (0x0002, 0, 32, 26)
            } else {
                (0x0102, 1, 32, 12)
            };
            bank.frames.push(decode_base_frame(
                secondary_payloads[ordinal],
                selector,
                header,
                mode,
                width,
                height,
                PAYLOAD_BYTES[ordinal],
                PAYLOAD_SHA256[ordinal],
            )?);
        }
        Ok(bank)
    }

    /// Add the exact static frames selected by Level 1's authored tree and
    /// stone constructors, including the two persistent stump selectors.
    pub fn decode_with_level1_secondary_and_scenery(
        spell_jar_payload: &[u8],
        exit_target_payload: &[u8],
        secondary_payloads: [&[u8]; LEVEL1_SECONDARY_SELECTORS.len()],
        scenery_payloads: [&[u8]; LEVEL1_SCENERY_SELECTORS.len()],
    ) -> Result<Self, String> {
        let mut bank = Self::decode_with_level1_secondary(
            spell_jar_payload,
            exit_target_payload,
            secondary_payloads,
        )?;
        const EXTENTS: [(u16, u16); LEVEL1_SCENERY_SELECTORS.len()] =
            [(64, 53), (80, 75), (50, 144), (80, 75), (80, 75), (80, 75)];
        const PAYLOAD_BYTES: [usize; LEVEL1_SCENERY_SELECTORS.len()] =
            [3_398, 6_006, 7_206, 6_006, 6_006, 6_006];
        const PAYLOAD_SHA256: [&str; LEVEL1_SCENERY_SELECTORS.len()] = [
            "d2d90a67c3d2be88d61835c25bff8fd0a655bc133987873337440843b1d626dc",
            "eb71fccffbca8a5fb4195f013ad4374b3e245e7c5941d73b9fe60824cd62b2a6",
            "263d77437976605dbb797c03ab59591e77fae2e93fce0be32861989982d3adde",
            "eb71fccffbca8a5fb4195f013ad4374b3e245e7c5941d73b9fe60824cd62b2a6",
            "9d68fd94c343efbe9656dddcbfa2ff726d2a9625779c8889876efc138b52b8ca",
            "9d68fd94c343efbe9656dddcbfa2ff726d2a9625779c8889876efc138b52b8ca",
        ];
        for ordinal in 0..LEVEL1_SCENERY_SELECTORS.len() {
            let selector = LEVEL1_SCENERY_SELECTORS[ordinal];
            let (width, height) = EXTENTS[ordinal];
            let (header, render_mode) = if selector == 57 {
                (0x1502, 21)
            } else {
                (0x0002, 0)
            };
            bank.frames.push(decode_base_frame(
                scenery_payloads[ordinal],
                selector,
                header,
                render_mode,
                width,
                height,
                PAYLOAD_BYTES[ordinal],
                PAYLOAD_SHA256[ordinal],
            )?);
        }
        Ok(bank)
    }

    /// Add the three exact fixed frames selected indirectly by ambient
    /// particle parameter rows 7, 9, and 41.
    pub fn decode_with_level1_secondary_scenery_and_ambient(
        spell_jar_payload: &[u8],
        exit_target_payload: &[u8],
        secondary_payloads: [&[u8]; LEVEL1_SECONDARY_SELECTORS.len()],
        scenery_payloads: [&[u8]; LEVEL1_SCENERY_SELECTORS.len()],
        ambient_payloads: [&[u8]; LEVEL1_AMBIENT_SELECTORS.len()],
    ) -> Result<Self, String> {
        let mut bank = Self::decode_with_level1_secondary_and_scenery(
            spell_jar_payload,
            exit_target_payload,
            secondary_payloads,
            scenery_payloads,
        )?;
        const EXTENTS: [(u16, u16); 3] = [(40, 39), (36, 34), (64, 53)];
        const HEADERS: [u16; 3] = [0x2402, 0x1502, 0x1502];
        const BYTES: [usize; 3] = [1_566, 1_230, 3_398];
        const HASHES: [&str; 3] = [
            "1dd8a48d785dac1dee3a5c012360cc4c7aab03687b4febb298a9f1fb8b7b284f",
            "40de3c4958c5ffc65c3a57d6b37e5d7d1d8b6cdf1f2da3b941820b388d4e3eca",
            "a2a5cf4d1ace2b0b025d02c5b9a14cf08d1d583eeae2bb06e8228a1347004ad7",
        ];
        for ordinal in 0..LEVEL1_AMBIENT_SELECTORS.len() {
            let selector = LEVEL1_AMBIENT_SELECTORS[ordinal];
            let (width, height) = EXTENTS[ordinal];
            bank.frames.push(decode_base_frame(
                ambient_payloads[ordinal],
                selector,
                HEADERS[ordinal],
                HEADERS[ordinal].to_le_bytes()[1],
                width,
                height,
                BYTES[ordinal],
                HASHES[ordinal],
            )?);
        }
        Ok(bank)
    }

    #[must_use]
    pub fn frame(&self, selector: u16) -> Option<&StaticWorldTmapFrame> {
        self.frames.iter().find(|frame| frame.selector == selector)
    }
}

#[allow(clippy::too_many_arguments)]
fn decode_base_frame(
    payload: &[u8],
    selector: u16,
    expected_header: u16,
    expected_render_mode: u8,
    expected_width: u16,
    expected_height: u16,
    expected_bytes: usize,
    expected_hash: &str,
) -> Result<StaticWorldTmapFrame, String> {
    if payload.len() != expected_bytes {
        return Err(format!(
            "Level 1 secondary TMAP {selector} has {} bytes, expected {expected_bytes}",
            payload.len()
        ));
    }
    require_hash(
        payload,
        expected_hash,
        &format!("Level 1 secondary TMAP {selector} payload"),
    )?;
    let header = read_u16(payload, 0, "secondary TMAP header")?;
    let width = read_u16(payload, 2, "secondary TMAP width")?;
    let height = read_u16(payload, 4, "secondary TMAP height")?;
    if (header, header.to_le_bytes()[1], width, height)
        != (
            expected_header,
            expected_render_mode,
            expected_width,
            expected_height,
        )
    {
        return Err(format!(
            "Level 1 secondary TMAP {selector} header/extent changed"
        ));
    }
    let pixel_end = 6 + usize::from(width) * usize::from(height);
    let pixels = payload
        .get(6..pixel_end)
        .ok_or_else(|| format!("Level 1 secondary TMAP {selector} base image is truncated"))?;
    Ok(StaticWorldTmapFrame {
        selector,
        render_mode: expected_render_mode,
        width,
        height,
        pixels: pixels.to_vec().into_boxed_slice(),
    })
}

#[allow(clippy::too_many_arguments)]
fn decode_static(
    payload: &[u8],
    selector: u16,
    expected_header: u16,
    expected_render_mode: u8,
    expected_width: u16,
    expected_height: u16,
    expected_bytes: usize,
    expected_hash: &str,
) -> Result<StaticWorldTmapFrame, String> {
    if payload.len() != expected_bytes {
        return Err(format!(
            "static world TMAP {selector} has {} bytes, expected {expected_bytes}",
            payload.len()
        ));
    }
    require_hash(
        payload,
        expected_hash,
        &format!("static world TMAP {selector} payload"),
    )?;
    let header = read_u16(payload, 0, "static world TMAP header")?;
    let width = read_u16(payload, 2, "static world TMAP width")?;
    let height = read_u16(payload, 4, "static world TMAP height")?;
    if header != expected_header
        || header.to_le_bytes()[1] != expected_render_mode
        || width != expected_width
        || height != expected_height
    {
        return Err(format!(
            "static world TMAP {selector} header/extent changed: {header:#06x}, mode {}, {width}x{height}",
            header.to_le_bytes()[1]
        ));
    }
    let expected_pixels = usize::from(width) * usize::from(height);
    let pixels = payload
        .get(6..)
        .ok_or_else(|| format!("static world TMAP {selector} has no pixel payload"))?;
    if pixels.len() != expected_pixels {
        return Err(format!(
            "static world TMAP {selector} has {} pixels, expected {expected_pixels}",
            pixels.len()
        ));
    }
    Ok(StaticWorldTmapFrame {
        selector,
        render_mode: expected_render_mode,
        width,
        height,
        pixels: pixels.to_vec().into_boxed_slice(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner_cache() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level1-unity-asset-cache/terrain/tmaps1")
    }

    #[test]
    fn qualified_owner_payloads_pin_fixed_modes_and_extents() {
        let root = owner_cache();
        let jars = std::fs::read(root.join("059.decoded.bin")).expect("jar owner payload");
        let exit = std::fs::read(root.join("461.decoded.bin")).expect("exit owner payload");
        let bank = StaticWorldTmapBank::decode_qualified(&jars, &exit).expect("qualified bank");

        let jar = bank.frame(SPELL_JAR_SELECTOR).expect("jar frame");
        assert_eq!((jar.render_mode, jar.width, jar.height), (1, 32, 26));
        assert_eq!(jar.pixels.len(), 832);

        let exit = bank.frame(EXIT_TARGET_SELECTOR).expect("exit frame");
        assert_eq!((exit.render_mode, exit.width, exit.height), (0, 70, 93));
        assert_eq!(exit.pixels.len(), 6_510);
    }

    #[test]
    fn wrong_header_or_payload_is_rejected() {
        let root = owner_cache();
        let mut jars = std::fs::read(root.join("059.decoded.bin")).expect("jar owner payload");
        let exit = std::fs::read(root.join("461.decoded.bin")).expect("exit owner payload");
        jars[1] = 0;
        assert!(StaticWorldTmapBank::decode_qualified(&jars, &exit).is_err());
    }

    #[test]
    fn qualified_level_one_scenery_payloads_pin_every_tree_stone_and_stump_frame() {
        let root = owner_cache();
        let jars = std::fs::read(root.join("059.decoded.bin")).expect("jar owner payload");
        let exit = std::fs::read(root.join("461.decoded.bin")).expect("exit owner payload");
        let secondary = LEVEL1_SECONDARY_SELECTORS.map(|selector| {
            std::fs::read(root.join(format!("{selector:03}.decoded.bin")))
                .expect("secondary payload")
        });
        let scenery = LEVEL1_SCENERY_SELECTORS.map(|selector| {
            std::fs::read(root.join(format!("{selector:03}.decoded.bin"))).expect("scenery payload")
        });
        let bank = StaticWorldTmapBank::decode_with_level1_secondary_and_scenery(
            &jars,
            &exit,
            std::array::from_fn(|ordinal| secondary[ordinal].as_slice()),
            std::array::from_fn(|ordinal| scenery[ordinal].as_slice()),
        )
        .expect("qualified scenery bank");
        assert_eq!(
            LEVEL1_SCENERY_SELECTORS.map(|selector| {
                let frame = bank.frame(selector).expect("scenery frame");
                (frame.selector, frame.render_mode, frame.width, frame.height)
            }),
            [
                (57, 21, 64, 53),
                (75, 0, 80, 75),
                (78, 0, 50, 144),
                (87, 0, 80, 75),
                (178, 0, 80, 75),
                (179, 0, 80, 75),
            ]
        );
    }
}
