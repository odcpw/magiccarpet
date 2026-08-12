//! Source-exact MSPRD0-0 occupancy masks used by remc2 model-45 collision.
//!
//! The exporter carries the raw signed 16-bit pixels from the source TAB/DAT
//! pair. -1 is remc2's transparent pixel; every other value is occupied.
//! The binary header and records are bounded so malformed owner data cannot
//! become an implicit collision authority.

use sha2::{Digest, Sha256};

const MAGIC: &[u8; 4] = b"MCM1";
const VERSION: u16 = 1;
const HEADER_BYTES: usize = 20;
const RECORD_HEADER_BYTES: usize = 12;

pub const MSPRD0_0_VGA_RESOLUTION_SELECTOR: u16 = 1;
pub const LEVEL2_COLLISION_MASK_BANK_BYTES: usize = 4_638;
pub const LEVEL2_COLLISION_MASK_BANK_SHA256: &str =
    "cd438a156fc7738c8221ac403be2c5efc1a4ffd96dbad5d9a53b2a75036ca432";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollisionMask {
    selector: u16,
    width: u16,
    height: u16,
    pixels: Box<[i16]>,
}

impl CollisionMask {
    #[must_use]
    pub const fn selector(&self) -> u16 {
        self.selector
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
    pub fn pixels(&self) -> &[i16] {
        &self.pixels
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollisionMaskBank {
    vga_resolution_selector: u16,
    masks: Box<[CollisionMask]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollisionMaskError {
    HeaderTooShort,
    BadMagic,
    UnsupportedVersion(u16),
    HeaderExtentMismatch,
    VgaSelectorMismatch(u16),
    ReservedNonZero,
    InvalidMaskCount,
    RecordHeaderExtentMismatch,
    TruncatedRecord,
    InvalidExtent,
    PixelExtentMismatch,
    UnsortedSelectors,
    TrailingBytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollisionMaskLookupError {
    SelectorOutOfRange(u16),
    PixelOutOfBounds { selector: u16, x: i32, y: i32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollisionMaskSourceIdentityError {
    ByteLengthMismatch { expected: usize, actual: usize },
    Sha256Mismatch,
    Malformed(CollisionMaskError),
}

impl CollisionMaskBank {
    pub fn parse(bytes: &[u8]) -> Result<Self, CollisionMaskError> {
        if bytes.len() < HEADER_BYTES {
            return Err(CollisionMaskError::HeaderTooShort);
        }
        if bytes[..4] != *MAGIC {
            return Err(CollisionMaskError::BadMagic);
        }
        let mut cursor = 4;
        let version = read_u16(bytes, &mut cursor, CollisionMaskError::HeaderTooShort)?;
        if version != VERSION {
            return Err(CollisionMaskError::UnsupportedVersion(version));
        }
        let header_bytes = read_u16(bytes, &mut cursor, CollisionMaskError::HeaderTooShort)?;
        if usize::from(header_bytes) != HEADER_BYTES {
            return Err(CollisionMaskError::HeaderExtentMismatch);
        }
        let vga_resolution_selector =
            read_u16(bytes, &mut cursor, CollisionMaskError::HeaderTooShort)?;
        if vga_resolution_selector != MSPRD0_0_VGA_RESOLUTION_SELECTOR {
            return Err(CollisionMaskError::VgaSelectorMismatch(
                vga_resolution_selector,
            ));
        }
        if read_u16(bytes, &mut cursor, CollisionMaskError::HeaderTooShort)? != 0 {
            return Err(CollisionMaskError::ReservedNonZero);
        }
        let mask_count = read_u32(bytes, &mut cursor, CollisionMaskError::HeaderTooShort)?;
        if mask_count == 0 {
            return Err(CollisionMaskError::InvalidMaskCount);
        }
        let record_header_bytes = read_u32(bytes, &mut cursor, CollisionMaskError::HeaderTooShort)?;
        if usize::try_from(record_header_bytes).ok() != Some(RECORD_HEADER_BYTES) {
            return Err(CollisionMaskError::RecordHeaderExtentMismatch);
        }
        let available_record_slots = (bytes.len() - HEADER_BYTES) / RECORD_HEADER_BYTES;
        if u64::from(mask_count) > available_record_slots as u64 {
            return Err(CollisionMaskError::TruncatedRecord);
        }

        let mut masks = Vec::with_capacity(mask_count as usize);
        let mut previous_selector = None;
        for _ in 0..mask_count {
            let selector = read_u16(bytes, &mut cursor, CollisionMaskError::TruncatedRecord)?;
            let width = read_u16(bytes, &mut cursor, CollisionMaskError::TruncatedRecord)?;
            let height = read_u16(bytes, &mut cursor, CollisionMaskError::TruncatedRecord)?;
            if read_u16(bytes, &mut cursor, CollisionMaskError::TruncatedRecord)? != 0 {
                return Err(CollisionMaskError::ReservedNonZero);
            }
            let pixel_bytes = read_u32(bytes, &mut cursor, CollisionMaskError::TruncatedRecord)?;
            if width == 0 || height == 0 {
                return Err(CollisionMaskError::InvalidExtent);
            }
            let expected_pixel_bytes = usize::from(width)
                .checked_mul(usize::from(height))
                .and_then(|pixels| pixels.checked_mul(2))
                .ok_or(CollisionMaskError::InvalidExtent)?;
            if usize::try_from(pixel_bytes).ok() != Some(expected_pixel_bytes) {
                return Err(CollisionMaskError::PixelExtentMismatch);
            }
            if previous_selector.is_some_and(|previous| selector <= previous) {
                return Err(CollisionMaskError::UnsortedSelectors);
            }
            let payload = take(
                bytes,
                &mut cursor,
                expected_pixel_bytes,
                CollisionMaskError::TruncatedRecord,
            )?;
            let pixels = payload
                .chunks_exact(2)
                .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
                .collect::<Vec<_>>()
                .into_boxed_slice();
            masks.push(CollisionMask {
                selector,
                width,
                height,
                pixels,
            });
            previous_selector = Some(selector);
        }
        if cursor != bytes.len() {
            return Err(CollisionMaskError::TrailingBytes);
        }
        Ok(Self {
            vga_resolution_selector,
            masks: masks.into_boxed_slice(),
        })
    }

    /// Validate and parse the complete qualified Level 2 MSPRD0-0 bank.
    ///
    /// Manifest member and aggregate hashes are self-consistency claims. This
    /// source-derived identity is the immutable admission check that the
    /// authoritative package-open path must use before constructing a live
    /// session.
    pub fn parse_source_exact(bytes: &[u8]) -> Result<Self, CollisionMaskSourceIdentityError> {
        Self::validate_source_exact(bytes)?;
        Self::parse(bytes).map_err(CollisionMaskSourceIdentityError::Malformed)
    }

    /// Validate the immutable byte identity of the qualified Level 2 bank.
    pub fn validate_source_exact(bytes: &[u8]) -> Result<(), CollisionMaskSourceIdentityError> {
        if bytes.len() != LEVEL2_COLLISION_MASK_BANK_BYTES {
            return Err(CollisionMaskSourceIdentityError::ByteLengthMismatch {
                expected: LEVEL2_COLLISION_MASK_BANK_BYTES,
                actual: bytes.len(),
            });
        }
        if format!("{:x}", Sha256::digest(bytes)) != LEVEL2_COLLISION_MASK_BANK_SHA256 {
            return Err(CollisionMaskSourceIdentityError::Sha256Mismatch);
        }
        Ok(())
    }

    #[must_use]
    pub const fn vga_resolution_selector(&self) -> u16 {
        self.vga_resolution_selector
    }

    #[must_use]
    pub fn masks(&self) -> &[CollisionMask] {
        &self.masks
    }

    #[must_use]
    pub fn mask(&self, selector: u16) -> Option<&CollisionMask> {
        self.masks.iter().find(|mask| mask.selector == selector)
    }

    /// Return remc2's model-45 occupancy result for an exact source pixel.
    pub fn model45_occupancy(
        &self,
        selector: u16,
        x: i32,
        y: i32,
    ) -> Result<bool, CollisionMaskLookupError> {
        let mask = self
            .mask(selector)
            .ok_or(CollisionMaskLookupError::SelectorOutOfRange(selector))?;
        let original_x = x;
        let original_y = y;
        let x = usize::try_from(x).ok();
        let y = usize::try_from(y).ok();
        let Some((x, y)) = x.zip(y) else {
            return Err(CollisionMaskLookupError::PixelOutOfBounds {
                selector,
                x: original_x,
                y: original_y,
            });
        };
        if x >= usize::from(mask.width) || y >= usize::from(mask.height) {
            return Err(CollisionMaskLookupError::PixelOutOfBounds {
                selector,
                x: original_x,
                y: original_y,
            });
        }
        Ok(mask.pixels[y * usize::from(mask.width) + x] != -1)
    }
}

fn take<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    length: usize,
    error: CollisionMaskError,
) -> Result<&'a [u8], CollisionMaskError> {
    let end = cursor.checked_add(length).ok_or(error)?;
    if end > bytes.len() {
        return Err(error);
    }
    let result = &bytes[*cursor..end];
    *cursor = end;
    Ok(result)
}

fn read_u16(
    bytes: &[u8],
    cursor: &mut usize,
    error: CollisionMaskError,
) -> Result<u16, CollisionMaskError> {
    let raw = take(bytes, cursor, 2, error)?;
    Ok(u16::from_le_bytes([raw[0], raw[1]]))
}

fn read_u32(
    bytes: &[u8],
    cursor: &mut usize,
    error: CollisionMaskError,
) -> Result<u32, CollisionMaskError> {
    let raw = take(bytes, cursor, 4, error)?;
    Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::{
        CollisionMaskBank, CollisionMaskError, CollisionMaskLookupError,
        CollisionMaskSourceIdentityError, LEVEL2_COLLISION_MASK_BANK_BYTES,
        LEVEL2_COLLISION_MASK_BANK_SHA256, MSPRD0_0_VGA_RESOLUTION_SELECTOR,
    };

    fn sample_bank() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"MCM1");
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&20u16.to_le_bytes());
        bytes.extend_from_slice(&MSPRD0_0_VGA_RESOLUTION_SELECTOR.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&12u32.to_le_bytes());
        bytes.extend_from_slice(&3u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend_from_slice(&(-1i16).to_le_bytes());
        bytes.extend_from_slice(&7i16.to_le_bytes());
        bytes.extend_from_slice(&7u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&8u32.to_le_bytes());
        for pixel in [-2i16, -1, 0, 3] {
            bytes.extend_from_slice(&pixel.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn model45_occupancy_preserves_signed_source_pixels_and_order() {
        let bytes = sample_bank();
        let bank = CollisionMaskBank::parse(&bytes).expect("sample carrier parses");
        let replay = CollisionMaskBank::parse(&bytes).expect("replay carrier parses");
        assert_eq!(bank, replay);
        assert_eq!(
            bank.masks()
                .iter()
                .map(|mask| mask.selector())
                .collect::<Vec<_>>(),
            vec![3, 7]
        );
        assert!(!bank.model45_occupancy(3, 0, 0).expect("transparent pixel"));
        assert!(bank.model45_occupancy(3, 1, 0).expect("occupied pixel"));
        assert!(
            bank.model45_occupancy(7, 0, 0)
                .expect("negative non-transparent pixel")
        );
        assert!(!bank.model45_occupancy(7, 1, 0).expect("transparent pixel"));
        assert!(bank.model45_occupancy(7, 0, 1).expect("zero pixel"));
    }

    #[test]
    fn malformed_masks_fail_closed_at_header_record_and_tail_boundaries() {
        let mut selector_mismatch = sample_bank();
        selector_mismatch[8..10].copy_from_slice(&2u16.to_le_bytes());
        assert_eq!(
            CollisionMaskBank::parse(&selector_mismatch),
            Err(CollisionMaskError::VgaSelectorMismatch(2))
        );

        let mut extent_mismatch = sample_bank();
        extent_mismatch[28..32].copy_from_slice(&6u32.to_le_bytes());
        assert_eq!(
            CollisionMaskBank::parse(&extent_mismatch),
            Err(CollisionMaskError::PixelExtentMismatch)
        );

        let mut unsorted = sample_bank();
        unsorted[36..38].copy_from_slice(&2u16.to_le_bytes());
        assert_eq!(
            CollisionMaskBank::parse(&unsorted),
            Err(CollisionMaskError::UnsortedSelectors)
        );

        let mut truncated = sample_bank();
        truncated.pop();
        assert_eq!(
            CollisionMaskBank::parse(&truncated),
            Err(CollisionMaskError::TruncatedRecord)
        );

        let mut trailing = sample_bank();
        trailing.push(0);
        assert_eq!(
            CollisionMaskBank::parse(&trailing),
            Err(CollisionMaskError::TrailingBytes)
        );
    }

    #[test]
    fn model45_occupancy_rejects_selector_and_pixel_out_of_range() {
        let bank = CollisionMaskBank::parse(&sample_bank()).expect("sample carrier parses");
        assert_eq!(
            bank.model45_occupancy(99, 0, 0),
            Err(CollisionMaskLookupError::SelectorOutOfRange(99))
        );
        assert_eq!(
            bank.model45_occupancy(3, -1, 0),
            Err(CollisionMaskLookupError::PixelOutOfBounds {
                selector: 3,
                x: -1,
                y: 0
            })
        );
        assert_eq!(
            bank.model45_occupancy(3, 2, 0),
            Err(CollisionMaskLookupError::PixelOutOfBounds {
                selector: 3,
                x: 2,
                y: 0
            })
        );
    }

    #[test]
    fn source_exact_identity_rejects_wrong_full_bank_before_parse() {
        let wrong_bank = vec![0_u8; LEVEL2_COLLISION_MASK_BANK_BYTES];
        assert_eq!(wrong_bank.len(), LEVEL2_COLLISION_MASK_BANK_BYTES);
        assert_ne!(
            format!("{:x}", Sha256::digest(&wrong_bank)),
            LEVEL2_COLLISION_MASK_BANK_SHA256
        );
        assert_eq!(
            CollisionMaskBank::validate_source_exact(&wrong_bank),
            Err(CollisionMaskSourceIdentityError::Sha256Mismatch)
        );
        assert_eq!(
            CollisionMaskBank::parse_source_exact(&wrong_bank),
            Err(CollisionMaskSourceIdentityError::Sha256Mismatch)
        );
    }
}
