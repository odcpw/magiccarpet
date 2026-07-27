//! Shared decoder for remc2's mutable indexed SS2 sprite transitions.
//!
//! Animated TMAPs keep one mutable indexed base image followed by a sequence
//! of F1FA/type-7 word-packet deltas. The caller owns selector identity,
//! header/frame-count qualification, state clocks, and closure policy.

use sha2::{Digest, Sha256};

pub(crate) fn apply_ss2_delta(
    decoded: &[u8],
    pixels: &mut [u8],
    family: &str,
    selector: u16,
    width: u16,
    height: u16,
    cursor: u32,
    stream_end: usize,
) -> Result<u32, String> {
    let outer_start = 6_usize
        .checked_add(
            usize::try_from(cursor)
                .map_err(|_| format!("{family} TMAP {selector} cursor exceeds usize"))?,
        )
        .ok_or_else(|| format!("{family} TMAP {selector} outer offset overflow"))?;
    let outer_size = usize::try_from(read_u32(decoded, outer_start, "SS2 outer size")?)
        .map_err(|_| format!("{family} TMAP {selector} outer size exceeds usize"))?;
    let outer_tag = read_u16(decoded, outer_start + 4, "SS2 outer tag")?;
    if outer_tag != 0xf1fa || outer_size < 24 {
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
    if child_count != 1
        || decoded
            .get(outer_start + 8..outer_start + 16)
            .is_none_or(|reserved| reserved != [0; 8])
    {
        return Err(format!(
            "{family} TMAP {selector} outer child/reserved fields changed"
        ));
    }
    let child_start = outer_start + 16;
    let child_size = usize::try_from(read_u32(decoded, child_start, "SS2 child size")?)
        .map_err(|_| format!("{family} TMAP {selector} child size exceeds usize"))?;
    let child_type = read_u16(decoded, child_start + 4, "SS2 child type")?;
    let child_end = child_start
        .checked_add(child_size)
        .ok_or_else(|| format!("{family} TMAP {selector} child extent overflow"))?;
    if child_type != 7 || child_size < 8 || child_end != outer_end {
        return Err(format!(
            "{family} TMAP {selector} invalid type-7 child record"
        ));
    }
    let line_count = read_i16(decoded, child_start + 6, "SS2 line count")?;
    if line_count < 0 || usize::try_from(line_count).unwrap_or(usize::MAX) > usize::from(height) {
        return Err(format!(
            "{family} TMAP {selector} invalid SS2 line count {line_count}"
        ));
    }
    let width = usize::from(width);
    let expected_pixels = width * usize::from(height);
    if pixels.len() != expected_pixels {
        return Err(format!(
            "{family} TMAP {selector} destination extent changed"
        ));
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
    if position != child_end {
        return Err(format!(
            "{family} TMAP {selector} SS2 child has trailing or missing bytes"
        ));
    }
    u32::try_from(outer_end - 6)
        .map_err(|_| format!("{family} TMAP {selector} next cursor exceeds u32"))
}

pub(crate) fn read_u16(bytes: &[u8], offset: usize, label: &str) -> Result<u16, String> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| format!("{label} is truncated"))?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn read_i16(bytes: &[u8], offset: usize, label: &str) -> Result<i16, String> {
    Ok(read_u16(bytes, offset, label)?.cast_signed())
}

pub(crate) fn read_u32(bytes: &[u8], offset: usize, label: &str) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("{label} is truncated"))?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

pub(crate) fn require_hash(bytes: &[u8], expected: &str, label: &str) -> Result<(), String> {
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != expected {
        return Err(format!("{label} hash mismatch: {actual}"));
    }
    Ok(())
}
