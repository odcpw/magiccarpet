//! MC2's indexed sprite pixel operations, independent of a level or renderer.
//!
//! `DrawSprites_3E360` selects these operations before `DrawSprite_41BD3`.
//! A sprite's image selector and its pixel operation are separate state.

use crate::{SIN_16_16, goat_sprite_raster::Mode20SpriteProjectionLimits};

const NEAR_PIXEL_MODES: [u8; 6] = [0, 0, 2, 3, 4, 5];
const FOG_PIXEL_MODES: [u8; 6] = [1, 0, 6, 7, 4, 5];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceSpritePixelMetadata {
    pub(crate) pixel_mode: u8,
    pub(crate) brightness: u16,
}

/// The same signed map wrapping and fixed-point rotation as the existing
/// source sprite projector, including its truncation before squaring.
pub(crate) fn source_sprite_distance_squared(
    camera_x: u16,
    camera_y: u16,
    camera_yaw: i16,
    entity_x: u16,
    entity_y: u16,
) -> u32 {
    let yaw = (i32::from(camera_yaw) & 2047) as usize;
    let sin = SIN_16_16[yaw];
    let cos = SIN_16_16[512 + yaw];
    let dx = i32::from(entity_x.wrapping_sub(camera_x).cast_signed());
    let dy = i32::from(camera_y.wrapping_sub(entity_y).cast_signed());
    let forward = sin.wrapping_mul(dx).wrapping_add(cos.wrapping_mul(dy)) >> 16;
    let lateral = cos.wrapping_mul(dx).wrapping_sub(sin.wrapping_mul(dy)) >> 16;
    lateral
        .wrapping_mul(lateral)
        .wrapping_add(forward.wrapping_mul(forward))
        .cast_unsigned()
}

/// Source `x_BYTE_D4750[byte_10]` or `[6 + byte_10]`, with the original
/// squared-distance fog row. Parameter value 1 deliberately remains unshaded.
pub(crate) fn source_sprite_pixel_metadata(
    particle_byte_10: u8,
    distance_squared: u32,
) -> Result<SourceSpritePixelMetadata, &'static str> {
    let limits = Mode20SpriteProjectionLimits::exact();
    let brightness = if distance_squared <= limits.fog_start_squared {
        0x2000
    } else if distance_squared < limits.fog_end_squared {
        ((32_i64 * i64::from(limits.fog_end_squared - distance_squared)
            / limits.fog_thickness_squared)
            << 8) as u16
    } else {
        0
    };
    let modes = if brightness == 0x2000 {
        &NEAR_PIXEL_MODES
    } else {
        &FOG_PIXEL_MODES
    };
    let pixel_mode = *modes
        .get(usize::from(particle_byte_10))
        .ok_or("source sprite particle byte_10 exceeds the pixel-mode table")?;
    Ok(SourceSpritePixelMetadata {
        pixel_mode,
        brightness,
    })
}

/// Literal behavior of the pinned remc2 primary-sprite flag branch. Its
/// `dw_w_b.byte` is unsigned: `byte[2] & 0x380` tests bit 7, while the later
/// `byte[2] >= 0` condition always succeeds. An unmatched flagged row retains
/// the previous operation; it does not reset to ordinary copy.
pub(crate) const fn source_sprite_pixel_mode(
    base_pixel_mode: u8,
    control_flags: u32,
    previous_pixel_mode: u8,
) -> u8 {
    if control_flags & 0x0080_0000 == 0 {
        return base_pixel_mode;
    }
    let byte_3 = (control_flags >> 24) as u8;
    if byte_3 & 2 != 0 {
        4
    } else if byte_3 & 4 != 0 {
        5
    } else if byte_3 & 1 != 0 {
        3
    } else {
        previous_pixel_mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_pixel_table_retains_unshaded_and_both_blend_directions() {
        let limits = Mode20SpriteProjectionLimits::exact();
        for parameter in 0..6 {
            assert_eq!(
                source_sprite_pixel_metadata(parameter, limits.fog_start_squared)
                    .unwrap()
                    .pixel_mode,
                NEAR_PIXEL_MODES[usize::from(parameter)]
            );
            let fog = source_sprite_pixel_metadata(parameter, limits.fog_end_squared).unwrap();
            assert_eq!(fog.pixel_mode, FOG_PIXEL_MODES[usize::from(parameter)]);
            assert_eq!(fog.brightness, 0);
        }
        assert!(source_sprite_pixel_metadata(6, 0).is_err());
    }

    #[test]
    fn sprite_distance_matches_the_admitting_source_projector() {
        for yaw in [0, 17, 511, 1024, 1700] {
            for (x, y) in [(100, 100), (65530, 400), (1200, 65000)] {
                if let Ok((_, _, distance)) =
                    crate::goat_sprite_raster::source_sprite_view(400, 200, yaw, x, y, u32::MAX)
                {
                    assert_eq!(
                        source_sprite_distance_squared(400, 200, yaw, x, y),
                        distance
                    );
                }
            }
        }
    }

    #[test]
    fn unsigned_source_flags_preserve_the_previous_operation_when_unmatched() {
        assert_eq!(source_sprite_pixel_mode(7, 0, 5), 7);
        assert_eq!(source_sprite_pixel_mode(7, 0x0080_0000, 5), 5);
        assert_eq!(source_sprite_pixel_mode(7, 0x0180_0000, 5), 3);
        assert_eq!(source_sprite_pixel_mode(7, 0x0280_0000, 5), 4);
        assert_eq!(source_sprite_pixel_mode(7, 0x0480_0000, 5), 5);
    }
}
