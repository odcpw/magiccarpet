//! Shared decoder for remc2's mutable indexed SS2 sprite transitions.
//!
//! Animated TMAPs keep one mutable indexed base image followed by a sequence
//! of F1FA records with type-7 word-packet deltas and optional skipped type-4
//! children. The caller owns selector identity,
//! header/frame-count qualification, state clocks, and closure policy.

use sha2::{Digest, Sha256};

pub fn apply_ss2_delta(
    decoded: &[u8],
    pixels: &mut [u8],
    family: &str,
    selector: u16,
    width: u16,
    height: u16,
    cursor: u32,
    stream_end: usize,
) -> Result<u32, String> {
    // Decode transactionally: callers may retain the previous frame when an
    // owner stream is malformed or no longer matches its qualified extent.
    let mut candidate = pixels.to_vec();
    let next = apply_ss2_delta_inner(
        decoded,
        &mut candidate,
        family,
        selector,
        width,
        height,
        cursor,
        stream_end,
    )?;
    pixels.copy_from_slice(&candidate);
    Ok(next)
}

fn apply_ss2_delta_inner(
    decoded: &[u8],
    pixels: &mut [u8],
    family: &str,
    selector: u16,
    width: u16,
    height: u16,
    cursor: u32,
    stream_end: usize,
) -> Result<u32, String> {
    let decoded = decoded
        .get(..stream_end)
        .ok_or_else(|| format!("{family} TMAP {selector} stream extent exceeds payload"))?;
    let outer_start = 6_usize
        .checked_add(
            usize::try_from(cursor)
                .map_err(|_| format!("{family} TMAP {selector} cursor exceeds usize"))?,
        )
        .ok_or_else(|| format!("{family} TMAP {selector} outer offset overflow"))?;
    let outer_size = usize::try_from(read_u32(decoded, outer_start, "SS2 outer size")?)
        .map_err(|_| format!("{family} TMAP {selector} outer size exceeds usize"))?;
    let outer_tag = read_u16(decoded, outer_start + 4, "SS2 outer tag")?;
    if outer_tag != 0xf1fa || outer_size < 16 {
        return Err(format!(
            "{family} TMAP {selector} invalid F1FA outer record"
        ));
    }
    let outer_end = outer_start
        .checked_add(outer_size)
        .ok_or_else(|| format!("{family} TMAP {selector} outer extent overflow"))?;
    if outer_end > stream_end {
        return Err(format!(
            "{family} TMAP {selector} outer record exceeds stream"
        ));
    }
    let child_count = read_u16(decoded, outer_start + 6, "SS2 child count")?;
    if decoded
        .get(outer_start + 8..outer_start + 16)
        .is_none_or(|reserved| reserved != [0; 8])
    {
        return Err(format!(
            "{family} TMAP {selector} outer reserved fields changed"
        ));
    }
    if pixels.len() != usize::from(width) * usize::from(height) {
        return Err(format!(
            "{family} TMAP {selector} destination extent changed"
        ));
    }
    let outer = &decoded[..outer_end];
    let mut child_start = outer_start + 16;
    for _ in 0..child_count {
        let child_size = usize::try_from(read_u32(outer, child_start, "SS2 child size")?)
            .map_err(|_| format!("{family} TMAP {selector} child size exceeds usize"))?;
        let child_type = read_u16(outer, child_start + 4, "SS2 child type")?;
        let child_end = child_start
            .checked_add(child_size)
            .ok_or_else(|| format!("{family} TMAP {selector} child extent overflow"))?;
        if child_size < 6 || child_end > outer_end {
            return Err(format!("{family} TMAP {selector} invalid child extent"));
        }
        match child_type {
            // sub_766A4 calls sub_7677C for type 4; it skips the payload
            // without applying the intro decoder's palette operation.
            4 => {}
            7 if child_size >= 8 => apply_ss2_child(
                &outer[..child_end],
                pixels,
                family,
                selector,
                width,
                height,
                child_start,
            )?,
            _ => {
                return Err(format!(
                    "{family} TMAP {selector} unsupported child type {child_type}"
                ));
            }
        }
        child_start = child_end;
    }
    if child_start != outer_end {
        return Err(format!(
            "{family} TMAP {selector} outer child count or extent changed"
        ));
    }
    u32::try_from(outer_end - 6)
        .map_err(|_| format!("{family} TMAP {selector} next cursor exceeds u32"))
}

fn apply_ss2_child(
    decoded: &[u8],
    pixels: &mut [u8],
    family: &str,
    selector: u16,
    width: u16,
    height: u16,
    child_start: usize,
) -> Result<(), String> {
    let line_count = read_i16(decoded, child_start + 6, "SS2 line count")?;
    if line_count < 0 || usize::try_from(line_count).unwrap_or(usize::MAX) > usize::from(height) {
        return Err(format!(
            "{family} TMAP {selector} invalid SS2 line count {line_count}"
        ));
    }
    let width = usize::from(width);
    let expected_pixels = width * usize::from(height);
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
                    .map_err(|_| format!("{family} TMAP {selector} invalid line skip"))?;
                destination = destination
                    .checked_add(width * skip_lines)
                    .ok_or_else(|| format!("{family} TMAP {selector} line skip overflow"))?;
                if destination >= expected_pixels {
                    return Err(format!(
                        "{family} TMAP {selector} line {line} skips beyond image"
                    ));
                }
                continue;
            }
            let last = row_start
                .checked_add(width)
                .and_then(|value| value.checked_sub(1))
                .ok_or_else(|| format!("{family} TMAP {selector} last-pixel overflow"))?;
            if last >= expected_pixels {
                return Err(format!(
                    "{family} TMAP {selector} line {line} last pixel is outside image"
                ));
            }
            pixels[last] = control.to_le_bytes()[0];
            destination = row_start;
        };
        let row_start = destination;
        let row_end = row_start
            .checked_add(width)
            .ok_or_else(|| format!("{family} TMAP {selector} row extent overflow"))?;
        if row_end > expected_pixels {
            return Err(format!(
                "{family} TMAP {selector} line {line} starts outside image"
            ));
        }
        for packet in 0..packet_count {
            let skip = usize::from(
                *decoded
                    .get(position)
                    .ok_or_else(|| format!("{family} TMAP {selector} packet skip is truncated"))?,
            );
            let span = i8::from_le_bytes([*decoded
                .get(position + 1)
                .ok_or_else(|| format!("{family} TMAP {selector} packet span is truncated"))?]);
            position += 2;
            destination = destination
                .checked_add(skip)
                .ok_or_else(|| format!("{family} TMAP {selector} packet skip overflow"))?;
            if span == 0 {
                return Err(format!(
                    "{family} TMAP {selector} line {line} packet {packet} has zero span"
                ));
            }
            let byte_count = usize::from(span.unsigned_abs()) * 2;
            let write_end = destination
                .checked_add(byte_count)
                .ok_or_else(|| format!("{family} TMAP {selector} packet extent overflow"))?;
            if destination < row_start || write_end > row_end {
                return Err(format!(
                    "{family} TMAP {selector} line {line} packet {packet} crosses row"
                ));
            }
            if span > 0 {
                let literal_end = position
                    .checked_add(byte_count)
                    .ok_or_else(|| format!("{family} TMAP {selector} literal extent overflow"))?;
                let literal = decoded.get(position..literal_end).ok_or_else(|| {
                    format!("{family} TMAP {selector} literal packet is truncated")
                })?;
                pixels[destination..write_end].copy_from_slice(literal);
                position = literal_end;
            } else {
                let pair = decoded
                    .get(position..position + 2)
                    .ok_or_else(|| format!("{family} TMAP {selector} repeat pair is truncated"))?;
                for chunk in pixels[destination..write_end].chunks_exact_mut(2) {
                    chunk.copy_from_slice(pair);
                }
                position += 2;
            }
            destination = write_end;
        }
        destination = row_end;
    }
    if position != decoded.len() {
        return Err(format!(
            "{family} TMAP {selector} SS2 child has trailing or missing bytes"
        ));
    }
    Ok(())
}

pub fn read_u16(bytes: &[u8], offset: usize, label: &str) -> Result<u16, String> {
    let value = bytes
        .get(
            offset
                ..offset
                    .checked_add(2)
                    .ok_or_else(|| format!("{label} offset overflow"))?,
        )
        .ok_or_else(|| format!("{label} is truncated"))?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn read_i16(bytes: &[u8], offset: usize, label: &str) -> Result<i16, String> {
    Ok(read_u16(bytes, offset, label)?.cast_signed())
}

pub fn read_u32(bytes: &[u8], offset: usize, label: &str) -> Result<u32, String> {
    let value = bytes
        .get(
            offset
                ..offset
                    .checked_add(4)
                    .ok_or_else(|| format!("{label} offset overflow"))?,
        )
        .ok_or_else(|| format!("{label} is truncated"))?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

pub fn require_hash(bytes: &[u8], expected: &str, label: &str) -> Result<(), String> {
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
    fn source_word_packets_decode_literal_repeat_last_pixel_and_line_skip() {
        let payload = synthetic_ss2_payload();
        let mut pixels = payload[6..18].to_vec();
        let cursor = apply_ss2_delta(
            &payload,
            &mut pixels,
            "synthetic",
            8,
            4,
            3,
            18,
            payload.len(),
        )
        .expect("synthetic SS2 transition");
        assert_eq!(usize::try_from(cursor).unwrap() + 6, payload.len());
        assert_eq!(pixels, [9, 8, 3, 4, 10, 11, 12, 13, 5, 6, 22, 7]);
    }

    #[test]
    fn empty_frame_and_skipped_palette_child_follow_tmap_source() {
        let payload = synthetic_ss2_payload();
        let mut multi = payload.clone();
        // A type-4 chunk in the TMAP path is skipped, not applied to pixels.
        multi.splice(40..40, [10, 0, 0, 0, 4, 0, 11, 22, 33, 44]);
        let size = read_u32(&multi, 24, "outer").unwrap() + 10;
        multi[24..28].copy_from_slice(&size.to_le_bytes());
        multi[30..32].copy_from_slice(&2_u16.to_le_bytes());
        let mut pixels = multi[6..18].to_vec();
        let cursor =
            apply_ss2_delta(&multi, &mut pixels, "synthetic", 8, 4, 3, 18, multi.len()).unwrap();
        assert_eq!(cursor as usize + 6, multi.len());
        assert_eq!(pixels, [9, 8, 3, 4, 10, 11, 12, 13, 5, 6, 22, 7]);

        let mut empty = payload[..40].to_vec();
        empty[24..28].copy_from_slice(&16_u32.to_le_bytes());
        empty[30..32].copy_from_slice(&0_u16.to_le_bytes());
        let mut pixels = empty[6..18].to_vec();
        let original = pixels.clone();
        assert_eq!(
            apply_ss2_delta(&empty, &mut pixels, "synthetic", 8, 4, 3, 18, empty.len()).unwrap()
                as usize
                + 6,
            empty.len()
        );
        assert_eq!(pixels, original);
    }

    #[test]
    fn invalid_later_child_is_atomic() {
        let mut payload = synthetic_ss2_payload();
        let outer_size = read_u32(&payload, 24, "outer").unwrap() + 6;
        payload[24..28].copy_from_slice(&outer_size.to_le_bytes());
        payload[30..32].copy_from_slice(&2_u16.to_le_bytes());
        payload.extend_from_slice(&[6, 0, 0, 0, 15, 0]);
        let original = payload[6..18].to_vec();
        let mut pixels = original.clone();
        assert!(
            apply_ss2_delta(
                &payload,
                &mut pixels,
                "synthetic",
                8,
                4,
                3,
                18,
                payload.len()
            )
            .is_err()
        );
        assert_eq!(pixels, original);
    }

    #[test]
    fn rejected_late_packet_and_stream_extent_leave_pixels_unchanged() {
        let mut payload = synthetic_ss2_payload();
        let original = payload[6..18].to_vec();
        // The first packet writes successfully; the final repeat then lacks a byte.
        payload.pop();
        let outer_size = read_u32(&payload, 24, "outer").unwrap() - 1;
        let child_size = read_u32(&payload, 40, "child").unwrap() - 1;
        payload[24..28].copy_from_slice(&outer_size.to_le_bytes());
        payload[40..44].copy_from_slice(&child_size.to_le_bytes());
        let mut pixels = original.clone();
        assert!(
            apply_ss2_delta(
                &payload,
                &mut pixels,
                "synthetic",
                8,
                4,
                3,
                18,
                payload.len()
            )
            .is_err()
        );
        assert_eq!(pixels, original);

        let mut pixels = original.clone();
        assert!(
            apply_ss2_delta(
                &payload,
                &mut pixels,
                "synthetic",
                8,
                4,
                3,
                18,
                payload.len() + 1
            )
            .is_err()
        );
        assert_eq!(pixels, original);
    }

    /// Run explicitly with MC2_TMAP_ORACLE_DIR pointing at the Night cache's
    /// terrain/tmaps1 directory. Owner assets remain local and untracked.
    #[test]
    #[ignore = "requires the private qualified Night owner cache"]
    fn night_goat_and_archer_real_streams_match_qualified_frame_hashes() {
        let root = std::path::PathBuf::from(
            std::env::var_os("MC2_TMAP_ORACLE_DIR").expect("MC2_TMAP_ORACLE_DIR"),
        );
        let cases = [
            (
                152,
                22,
                22,
                "a5660e5a142d4179c1b3bb4328b4875c6621daf45c6c8dfcaa9a7a2fd3cc74db",
                [
                    "cd5e773ab7c4e4e88536c5a690a8a1794a1b67361b402ab8365e9df3e59a1e9d",
                    "502c7e18c327f09578c7ebcd31c351cd2c537bae9482c3f7630a7035be130190",
                    "b22359a1aebc1f7afea7e5aa7a4e8c5bdfc5b238ca9a7f162e110470b3350103",
                    "2011b3fb136f072ca7dcdae7199b2e8b9dbbdb71bdaa812b1bd659b695c51d12",
                    "cd5e773ab7c4e4e88536c5a690a8a1794a1b67361b402ab8365e9df3e59a1e9d",
                ],
            ),
            (
                8,
                30,
                56,
                "084b65352a3123fa3d5dd8fb344ba56bb4e78f387b1026dd3cdfe590b56c0691",
                [
                    "eb2f4bf66d88760d1f74d55a40c1267bb3d26de6af807931d695dcae52af6d8e",
                    "6624fa0869d6c535383f274febd07c72dd81a5b04308d46df43329f5df66c489",
                    "a1d723e40291f340043a7522befdcfe4c144d151d5ecee0ff3f475eae87b028f",
                    "6f0c8509da1670508858009ff4758764b76e0443e4dd2d56938716b5c44dd90c",
                    "eb2f4bf66d88760d1f74d55a40c1267bb3d26de6af807931d695dcae52af6d8e",
                ],
            ),
        ];
        for (selector, width, height, payload_hash, states) in cases {
            let decoded = std::fs::read(root.join(format!("{selector:03}.decoded.bin"))).unwrap();
            require_hash(&decoded, payload_hash, "owner payload").unwrap();
            assert_eq!(
                (
                    read_u16(&decoded, 0, "header").unwrap(),
                    read_u16(&decoded, 2, "width").unwrap(),
                    read_u16(&decoded, 4, "height").unwrap()
                ),
                (5123, width, height)
            );
            let base_end = 6 + usize::from(width) * usize::from(height);
            assert_eq!(read_u16(&decoded, base_end, "frame count").unwrap(), 4);
            assert_eq!(
                usize::try_from(read_u32(&decoded, base_end + 2, "stream size").unwrap()).unwrap()
                    + base_end
                    + 6,
                decoded.len()
            );
            let mut pixels = decoded[6..base_end].to_vec();
            let mut cursor = u32::try_from(base_end).unwrap();
            require_hash(&pixels, states[0], "base pixels").unwrap();
            for expected in &states[1..] {
                cursor = apply_ss2_delta(
                    &decoded,
                    &mut pixels,
                    "Night",
                    selector,
                    width,
                    height,
                    cursor,
                    decoded.len(),
                )
                .unwrap();
                require_hash(&pixels, expected, "frame pixels").unwrap();
            }
            assert_eq!(usize::try_from(cursor).unwrap() + 6, decoded.len());
        }
    }

    #[test]
    fn child_cannot_read_following_record() {
        let mut payload = synthetic_ss2_payload();
        let end = payload.len();
        // Keep the missing byte in the surrounding payload but exclude it from
        // both declared record sizes, as if it belonged to the next frame.
        let outer = 24;
        let child = outer + 16;
        let outer_size = read_u32(&payload, outer, "outer").unwrap() - 1;
        let child_size = read_u32(&payload, child, "child").unwrap() - 1;
        payload[outer..outer + 4].copy_from_slice(&outer_size.to_le_bytes());
        payload[child..child + 4].copy_from_slice(&child_size.to_le_bytes());
        let original = payload[6..18].to_vec();
        let mut pixels = original.clone();
        assert!(apply_ss2_delta(&payload, &mut pixels, "synthetic", 8, 4, 3, 18, end).is_err());
        assert_eq!(pixels, original);
    }
}
