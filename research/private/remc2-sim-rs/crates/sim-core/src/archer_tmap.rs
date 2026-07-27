//! Exact selector-global TMAP state for the Level 1 Archer mode-20 family.
//!
//! Selectors 8..10 are mutable four-transition SS2 streams. The attack-pose
//! selectors 11..13 and 121..123 are immutable indexed frames. Like the Goat
//! textures, each animated selector owns one shared descriptor clock: a draw
//! latches the selector and the following unpaused outer turn advances it.

use sha2::{Digest, Sha256};

pub const ARCHER_ANIMATED_SELECTORS: [u16; 3] = [8, 9, 10];
pub const ARCHER_STATIC_SELECTORS: [u16; 6] = [11, 12, 13, 121, 122, 123];
pub const ARCHER_TMAP_DRAW_MASK: u8 = 0b0000_0111;
pub const ARCHER_TMAP_STATE_COUNT: usize = 5;

const QUALIFIED_ANIMATED_PAYLOAD_SHA256: [&str; 3] = [
    "084b65352a3123fa3d5dd8fb344ba56bb4e78f387b1026dd3cdfe590b56c0691",
    "96747ee124997f694b384bdd1acf7d896f609ef5c5ef6f1eca9fa9875c57c4f1",
    "f07373bd03faec45604b1ad16ae016c005cec11584d39fa76a4769e04c940cf7",
];
const QUALIFIED_ANIMATED_EXTENTS: [(u16, u16); 3] = [(30, 56), (34, 56), (26, 56)];
const QUALIFIED_ANIMATED_STATE_SHA256: [[&str; ARCHER_TMAP_STATE_COUNT]; 3] = [
    [
        "eb2f4bf66d88760d1f74d55a40c1267bb3d26de6af807931d695dcae52af6d8e",
        "6624fa0869d6c535383f274febd07c72dd81a5b04308d46df43329f5df66c489",
        "a1d723e40291f340043a7522befdcfe4c144d151d5ecee0ff3f475eae87b028f",
        "6f0c8509da1670508858009ff4758764b76e0443e4dd2d56938716b5c44dd90c",
        "eb2f4bf66d88760d1f74d55a40c1267bb3d26de6af807931d695dcae52af6d8e",
    ],
    [
        "3bbd5b70057ef674e73002ae56e8a8865d59d4cf4ba19c8a0fe024c38c2d8b63",
        "9247efcbc6319667cbecc1eb72aa3c7e44106919501e0d1a6fe49f0eda8b64ba",
        "55330f5867904fd75d79aea58f72b7cdb3fecea41b22db213a4f9c453c4ea39d",
        "e8792e6187b0851401b8c2316c3c19dcaa47eb099b3b06b25947debc259f3544",
        "3bbd5b70057ef674e73002ae56e8a8865d59d4cf4ba19c8a0fe024c38c2d8b63",
    ],
    [
        "beca0f2e694017356b43841f1c256efedf7579905253a77831f81914189f0c8f",
        "f30615e38cdb6ca0cc0605ae893279f1edbc083cc3e8120d55a02ea495ce07ae",
        "d74f89659fb04baa124c12072392d3691e5cdcf35a8de5d19056f4a982e6e12b",
        "5489783e9f36b97a582ffde4ac45e918d9e195472159d694bd0bba3297ab60d0",
        "beca0f2e694017356b43841f1c256efedf7579905253a77831f81914189f0c8f",
    ],
];
const QUALIFIED_STATIC_EXTENTS: [(u16, u16); 6] =
    [(34, 57), (38, 55), (32, 55), (32, 45), (38, 45), (32, 45)];
const QUALIFIED_STATIC_SHA256: [&str; 6] = [
    "e41cac6d975a7cd02e50a1f63aba2ab51fbf54587f2ddb5af89e9d4257385a1a",
    "82ad42cd719d21c2422632a44361b6c918a805efbd4bca8bffc998099048f77a",
    "d81fe313bcc95ea110fee6bfd4e0716a8eef91737d964be3dee173292f87b44e",
    "da5633de0e9f4a0591c5f11a6648bb0f6c5caf1b586ed1b13b11d00480c35501",
    "7b0a2db4ae60b748857a356deed89a6698950371ad564fc5b51ff1985da58fc9",
    "ce6eb45e6e8d07c45de984c1cb78d574a5ae2f4357df0318f3f3ce9bf37bd3df",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArcherTmapFrame {
    pub selector: u16,
    pub state_ordinal: u8,
    pub frame_index: u8,
    pub stream_cursor: u32,
    pub width: u16,
    pub height: u16,
    pub pixels: Box<[u8]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArcherTmapSelectorBank {
    pub selector: u16,
    pub payload_bytes: u32,
    pub states: [ArcherTmapFrame; ARCHER_TMAP_STATE_COUNT],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArcherStaticTmapFrame {
    pub selector: u16,
    pub width: u16,
    pub height: u16,
    pub pixels: Box<[u8]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArcherTmapBank {
    animated: [ArcherTmapSelectorBank; ARCHER_ANIMATED_SELECTORS.len()],
    static_frames: [ArcherStaticTmapFrame; ARCHER_STATIC_SELECTORS.len()],
}

impl ArcherTmapBank {
    /// Decode the exact qualified owner-cache Archer payloads.
    ///
    /// # Errors
    ///
    /// Rejects any identity, extent, SS2 opcode, transition, closure, or
    /// immutable-frame hash that differs from the qualified Level 1 assets.
    pub fn decode_qualified(
        animated_payloads: [&[u8]; ARCHER_ANIMATED_SELECTORS.len()],
        static_pixels: [&[u8]; ARCHER_STATIC_SELECTORS.len()],
    ) -> Result<Self, String> {
        let mut animated = Vec::with_capacity(ARCHER_ANIMATED_SELECTORS.len());
        for ordinal in 0..ARCHER_ANIMATED_SELECTORS.len() {
            let payload = animated_payloads[ordinal];
            require_hash(
                payload,
                QUALIFIED_ANIMATED_PAYLOAD_SHA256[ordinal],
                &format!(
                    "Archer animated TMAP {} payload",
                    ARCHER_ANIMATED_SELECTORS[ordinal]
                ),
            )?;
            let selector_bank = decode_animated_selector(
                ARCHER_ANIMATED_SELECTORS[ordinal],
                payload,
                QUALIFIED_ANIMATED_EXTENTS[ordinal],
            )?;
            for (state, expected_hash) in selector_bank
                .states
                .iter()
                .zip(QUALIFIED_ANIMATED_STATE_SHA256[ordinal])
            {
                require_hash(
                    &state.pixels,
                    expected_hash,
                    &format!(
                        "Archer TMAP {} state {}",
                        state.selector, state.state_ordinal
                    ),
                )?;
            }
            animated.push(selector_bank);
        }

        let mut static_frames = Vec::with_capacity(ARCHER_STATIC_SELECTORS.len());
        for ordinal in 0..ARCHER_STATIC_SELECTORS.len() {
            let selector = ARCHER_STATIC_SELECTORS[ordinal];
            let (width, height) = QUALIFIED_STATIC_EXTENTS[ordinal];
            let pixels = static_pixels[ordinal];
            let expected_len = usize::from(width) * usize::from(height);
            if pixels.len() != expected_len {
                return Err(format!(
                    "Archer static TMAP {selector} has {} bytes, expected {expected_len}",
                    pixels.len()
                ));
            }
            require_hash(
                pixels,
                QUALIFIED_STATIC_SHA256[ordinal],
                &format!("Archer static TMAP {selector}"),
            )?;
            static_frames.push(ArcherStaticTmapFrame {
                selector,
                width,
                height,
                pixels: pixels.to_vec().into_boxed_slice(),
            });
        }
        Ok(Self {
            animated: animated
                .try_into()
                .map_err(|_| "Archer animated selector count changed".to_owned())?,
            static_frames: static_frames
                .try_into()
                .map_err(|_| "Archer static selector count changed".to_owned())?,
        })
    }

    #[must_use]
    pub const fn animated(&self) -> &[ArcherTmapSelectorBank; ARCHER_ANIMATED_SELECTORS.len()] {
        &self.animated
    }

    #[must_use]
    pub fn static_frame(&self, selector: u16) -> Option<&ArcherStaticTmapFrame> {
        self.static_frames
            .iter()
            .find(|frame| frame.selector == selector)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArcherTmapClock {
    selector: u16,
    state_ordinal: u8,
    frame_index: u8,
    stream_cursor: u32,
    width: u16,
    height: u16,
    used_by_preceding_draw: bool,
    pixels: Box<[u8]>,
}

impl ArcherTmapClock {
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
pub struct ArcherTmapClocks {
    clocks: [ArcherTmapClock; ARCHER_ANIMATED_SELECTORS.len()],
}

impl ArcherTmapClocks {
    #[must_use]
    pub fn from_bank(bank: &ArcherTmapBank) -> Self {
        Self {
            clocks: std::array::from_fn(|ordinal| {
                let reset = &bank.animated[ordinal].states[0];
                ArcherTmapClock {
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

    pub fn latch_draw_attempts(&mut self, mask: u8) -> Result<(), String> {
        if mask & !ARCHER_TMAP_DRAW_MASK != 0 {
            return Err(format!(
                "Archer TMAP draw-attempt mask {mask:#04x} has unsupported bits"
            ));
        }
        for (ordinal, clock) in self.clocks.iter_mut().enumerate() {
            clock.used_by_preceding_draw |= mask & (1 << ordinal) != 0;
        }
        Ok(())
    }

    pub fn service_unpaused_outer_turn(&mut self, bank: &ArcherTmapBank) {
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
    pub fn animated_frame(&self, selector: u16) -> Option<&ArcherTmapClock> {
        self.clocks.iter().find(|clock| clock.selector == selector)
    }

    #[must_use]
    pub const fn states(&self) -> &[ArcherTmapClock; ARCHER_ANIMATED_SELECTORS.len()] {
        &self.clocks
    }
}

fn decode_animated_selector(
    selector: u16,
    decoded: &[u8],
    expected_extent: (u16, u16),
) -> Result<ArcherTmapSelectorBank, String> {
    let header_word = read_u16(decoded, 0, "TMAP header")?;
    let width = read_u16(decoded, 2, "TMAP width")?;
    let height = read_u16(decoded, 4, "TMAP height")?;
    if header_word != 5123 || (width, height) != expected_extent {
        return Err(format!(
            "Archer TMAP {selector} header/extent changed: {header_word}, {width}x{height}"
        ));
    }
    let first_frame_end = 6_usize
        .checked_add(usize::from(width) * usize::from(height))
        .ok_or_else(|| format!("Archer TMAP {selector} base extent overflow"))?;
    let frame_count = read_u16(decoded, first_frame_end, "TMAP frame count")?;
    let stream_size = read_u32(decoded, first_frame_end + 2, "TMAP stream size")?;
    if frame_count != 4 {
        return Err(format!(
            "Archer TMAP {selector} frame count is {frame_count}, expected 4"
        ));
    }
    let stream_start = first_frame_end + 6;
    let stream_end = stream_start
        .checked_add(
            usize::try_from(stream_size)
                .map_err(|_| format!("Archer TMAP {selector} stream size exceeds usize"))?,
        )
        .ok_or_else(|| format!("Archer TMAP {selector} stream extent overflow"))?;
    if stream_end != decoded.len() {
        return Err(format!(
            "Archer TMAP {selector} declared stream ends at {stream_end}, payload has {} bytes",
            decoded.len()
        ));
    }

    let mut pixels = decoded
        .get(6..first_frame_end)
        .ok_or_else(|| format!("Archer TMAP {selector} base frame is truncated"))?
        .to_vec();
    let mut cursor = u32::try_from(first_frame_end)
        .map_err(|_| format!("Archer TMAP {selector} cursor exceeds u32"))?;
    let mut states = Vec::with_capacity(ARCHER_TMAP_STATE_COUNT);
    states.push(ArcherTmapFrame {
        selector,
        state_ordinal: 0,
        frame_index: 1,
        stream_cursor: cursor,
        width,
        height,
        pixels: pixels.clone().into_boxed_slice(),
    });
    for state_ordinal in 1..ARCHER_TMAP_STATE_COUNT {
        cursor = apply_ss2_delta(
            decoded,
            &mut pixels,
            selector,
            width,
            height,
            cursor,
            stream_end,
        )?;
        states.push(ArcherTmapFrame {
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
            "Archer TMAP {selector} has missing or extra transition records"
        ));
    }
    if states[4].pixels != states[0].pixels {
        return Err(format!(
            "Archer TMAP {selector} fourth transition does not restore reset"
        ));
    }
    let states: [ArcherTmapFrame; ARCHER_TMAP_STATE_COUNT] = states
        .try_into()
        .map_err(|_| format!("Archer TMAP {selector} state count changed"))?;
    Ok(ArcherTmapSelectorBank {
        selector,
        payload_bytes: u32::try_from(decoded.len())
            .map_err(|_| format!("Archer TMAP {selector} payload exceeds u32"))?,
        states,
    })
}

fn apply_ss2_delta(
    decoded: &[u8],
    pixels: &mut [u8],
    selector: u16,
    width: u16,
    height: u16,
    cursor: u32,
    stream_end: usize,
) -> Result<u32, String> {
    let outer_start = 6_usize
        .checked_add(
            usize::try_from(cursor)
                .map_err(|_| format!("Archer TMAP {selector} cursor exceeds usize"))?,
        )
        .ok_or_else(|| format!("Archer TMAP {selector} outer offset overflow"))?;
    let outer_size = usize::try_from(read_u32(decoded, outer_start, "SS2 outer size")?)
        .map_err(|_| format!("Archer TMAP {selector} outer size exceeds usize"))?;
    let outer_tag = read_u16(decoded, outer_start + 4, "SS2 outer tag")?;
    if outer_tag != 0xf1fa || outer_size < 24 {
        return Err(format!("Archer TMAP {selector} invalid F1FA outer record"));
    }
    let outer_end = outer_start
        .checked_add(outer_size)
        .ok_or_else(|| format!("Archer TMAP {selector} outer extent overflow"))?;
    if outer_end > stream_end {
        return Err(format!(
            "Archer TMAP {selector} outer record exceeds stream"
        ));
    }
    let child_count = read_u16(decoded, outer_start + 6, "SS2 child count")?;
    if child_count != 1
        || decoded
            .get(outer_start + 8..outer_start + 16)
            .is_none_or(|reserved| reserved != [0; 8])
    {
        return Err(format!(
            "Archer TMAP {selector} outer child/reserved fields changed"
        ));
    }
    let child_start = outer_start + 16;
    let child_size = usize::try_from(read_u32(decoded, child_start, "SS2 child size")?)
        .map_err(|_| format!("Archer TMAP {selector} child size exceeds usize"))?;
    let child_type = read_u16(decoded, child_start + 4, "SS2 child type")?;
    let child_end = child_start
        .checked_add(child_size)
        .ok_or_else(|| format!("Archer TMAP {selector} child extent overflow"))?;
    if child_type != 7 || child_size < 8 || child_end != outer_end {
        return Err(format!(
            "Archer TMAP {selector} invalid type-7 child record"
        ));
    }
    let line_count = read_i16(decoded, child_start + 6, "SS2 line count")?;
    if line_count < 0 || usize::try_from(line_count).unwrap_or(usize::MAX) > usize::from(height) {
        return Err(format!(
            "Archer TMAP {selector} invalid SS2 line count {line_count}"
        ));
    }
    let width = usize::from(width);
    let expected_pixels = width * usize::from(height);
    if pixels.len() != expected_pixels {
        return Err(format!("Archer TMAP {selector} destination extent changed"));
    }
    let mut position = child_start + 8;
    let mut destination = 0_usize;
    for line in 0..usize::try_from(line_count).expect("validated non-negative line count") {
        let packet_count = loop {
            let control = read_i16(decoded, position, "SS2 row control")?;
            position += 2;
            let row_start = destination;
            if control >= 0 {
                break usize::try_from(control).expect("non-negative packet count");
            }
            if control.cast_unsigned() & 0x4000 != 0 {
                let skip_lines = usize::try_from(-i32::from(control))
                    .map_err(|_| format!("Archer TMAP {selector} invalid line skip"))?;
                destination = destination
                    .checked_add(width * skip_lines)
                    .ok_or_else(|| format!("Archer TMAP {selector} line skip overflow"))?;
                if destination >= expected_pixels {
                    return Err(format!(
                        "Archer TMAP {selector} line {line} skips beyond image"
                    ));
                }
                continue;
            }
            let last = row_start
                .checked_add(width)
                .and_then(|value| value.checked_sub(1))
                .ok_or_else(|| format!("Archer TMAP {selector} last-pixel overflow"))?;
            if last >= expected_pixels {
                return Err(format!(
                    "Archer TMAP {selector} line {line} last pixel is outside image"
                ));
            }
            pixels[last] = control.to_le_bytes()[0];
            destination = row_start;
        };
        let row_start = destination;
        let row_end = row_start
            .checked_add(width)
            .ok_or_else(|| format!("Archer TMAP {selector} row extent overflow"))?;
        if row_end > expected_pixels {
            return Err(format!(
                "Archer TMAP {selector} line {line} starts outside image"
            ));
        }
        for packet in 0..packet_count {
            let skip = usize::from(
                *decoded
                    .get(position)
                    .ok_or_else(|| format!("Archer TMAP {selector} packet skip is truncated"))?,
            );
            let span = i8::from_le_bytes([*decoded
                .get(position + 1)
                .ok_or_else(|| format!("Archer TMAP {selector} packet span is truncated"))?]);
            position += 2;
            destination = destination
                .checked_add(skip)
                .ok_or_else(|| format!("Archer TMAP {selector} packet skip overflow"))?;
            if span == 0 {
                return Err(format!(
                    "Archer TMAP {selector} line {line} packet {packet} has zero span"
                ));
            }
            let word_count = usize::from(span.unsigned_abs());
            let byte_count = word_count * 2;
            let write_end = destination
                .checked_add(byte_count)
                .ok_or_else(|| format!("Archer TMAP {selector} packet extent overflow"))?;
            if destination < row_start || write_end > row_end {
                return Err(format!(
                    "Archer TMAP {selector} line {line} packet {packet} crosses row"
                ));
            }
            if span > 0 {
                let literal_end = position
                    .checked_add(byte_count)
                    .ok_or_else(|| format!("Archer TMAP {selector} literal extent overflow"))?;
                let literal = decoded
                    .get(position..literal_end)
                    .ok_or_else(|| format!("Archer TMAP {selector} literal packet is truncated"))?;
                pixels[destination..write_end].copy_from_slice(literal);
                position = literal_end;
            } else {
                let pair = decoded
                    .get(position..position + 2)
                    .ok_or_else(|| format!("Archer TMAP {selector} repeat pair is truncated"))?;
                for chunk in pixels[destination..write_end].chunks_exact_mut(2) {
                    chunk.copy_from_slice(pair);
                }
                position += 2;
            }
            destination = write_end;
        }
        destination = row_end;
    }
    if position != child_end {
        return Err(format!(
            "Archer TMAP {selector} SS2 child has trailing or missing bytes"
        ));
    }
    u32::try_from(outer_end - 6)
        .map_err(|_| format!("Archer TMAP {selector} next cursor exceeds u32"))
}

fn read_u16(bytes: &[u8], offset: usize, label: &str) -> Result<u16, String> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| format!("{label} is truncated"))?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn read_i16(bytes: &[u8], offset: usize, label: &str) -> Result<i16, String> {
    Ok(read_u16(bytes, offset, label)?.cast_signed())
}

fn read_u32(bytes: &[u8], offset: usize, label: &str) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("{label} is truncated"))?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn require_hash(bytes: &[u8], expected: &str, label: &str) -> Result<(), String> {
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != expected {
        return Err(format!("{label} hash mismatch: {actual}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_ss2_payload() -> Vec<u8> {
        // 4x3 base, one transition. It exercises a literal word, a repeated
        // word, the last-pixel opcode, and a skipped source line.
        let mut child = Vec::new();
        child.extend_from_slice(&0_u32.to_le_bytes());
        child.extend_from_slice(&7_u16.to_le_bytes());
        child.extend_from_slice(&2_i16.to_le_bytes());
        child.extend_from_slice(&1_i16.to_le_bytes());
        child.extend_from_slice(&[0, 1, 9, 8]);
        child.extend_from_slice(&(-1_i16).to_le_bytes());
        child.extend_from_slice(&(0x8007_u16 as i16).to_le_bytes());
        child.extend_from_slice(&1_i16.to_le_bytes());
        child.extend_from_slice(&[0, -1_i8 as u8, 5, 6]);
        let child_size = u32::try_from(child.len()).unwrap();
        child[..4].copy_from_slice(&child_size.to_le_bytes());

        let mut outer = Vec::new();
        outer.extend_from_slice(&0_u32.to_le_bytes());
        outer.extend_from_slice(&0xf1fa_u16.to_le_bytes());
        outer.extend_from_slice(&1_u16.to_le_bytes());
        outer.extend_from_slice(&[0; 8]);
        outer.extend_from_slice(&child);
        let outer_size = u32::try_from(outer.len()).unwrap();
        outer[..4].copy_from_slice(&outer_size.to_le_bytes());

        let mut payload = Vec::new();
        payload.extend_from_slice(&5123_u16.to_le_bytes());
        payload.extend_from_slice(&4_u16.to_le_bytes());
        payload.extend_from_slice(&3_u16.to_le_bytes());
        payload.extend_from_slice(&[1, 2, 3, 4, 10, 11, 12, 13, 20, 21, 22, 23]);
        payload.extend_from_slice(&1_u16.to_le_bytes());
        payload.extend_from_slice(&(outer.len() as u32).to_le_bytes());
        payload.extend_from_slice(&outer);
        payload
    }

    #[test]
    fn ss2_decoder_matches_source_word_packets_last_pixel_and_line_skip() {
        let payload = synthetic_ss2_payload();
        let mut pixels = payload[6..18].to_vec();
        let cursor = apply_ss2_delta(&payload, &mut pixels, 8, 4, 3, 18, payload.len())
            .expect("synthetic SS2 transition");
        assert_eq!(usize::try_from(cursor).unwrap() + 6, payload.len());
        assert_eq!(pixels, [9, 8, 3, 4, 10, 11, 12, 13, 5, 6, 22, 7]);
    }

    #[test]
    fn selector_clocks_latch_once_and_wrap_from_state_four_to_one() {
        let frames = std::array::from_fn(|selector_ordinal| ArcherTmapSelectorBank {
            selector: ARCHER_ANIMATED_SELECTORS[selector_ordinal],
            payload_bytes: 1,
            states: std::array::from_fn(|state_ordinal| ArcherTmapFrame {
                selector: ARCHER_ANIMATED_SELECTORS[selector_ordinal],
                state_ordinal: state_ordinal as u8,
                frame_index: state_ordinal as u8 + 1,
                stream_cursor: state_ordinal as u32,
                width: 1,
                height: 1,
                pixels: vec![state_ordinal as u8].into_boxed_slice(),
            }),
        });
        let static_frames = std::array::from_fn(|ordinal| ArcherStaticTmapFrame {
            selector: ARCHER_STATIC_SELECTORS[ordinal],
            width: 1,
            height: 1,
            pixels: vec![ordinal as u8].into_boxed_slice(),
        });
        let bank = ArcherTmapBank {
            animated: frames,
            static_frames,
        };
        let mut clocks = ArcherTmapClocks::from_bank(&bank);
        for expected in [1, 2, 3, 4, 1] {
            clocks.latch_draw_attempts(0b001).unwrap();
            clocks.service_unpaused_outer_turn(&bank);
            assert_eq!(clocks.states()[0].state_ordinal(), expected);
            assert_eq!(clocks.states()[1].state_ordinal(), 0);
        }
    }
}
