//! Exact stateful terrain-slope camera offset from `DrawWorld_411A0`.
//!
//! The game first stores a pre-entity-update camera row, then the outdoor
//! renderer samples four height-map points around that row. Two bounded slope
//! targets are eased with persistent integer accumulators and added to camera
//! X/Y before terrain and particles are drawn.

/// One source-renderer camera-offset result.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerrainSlopeCameraSample {
    pub target_x: i16,
    pub target_y: i16,
    pub offset_x: i16,
    pub offset_y: i16,
}

/// Persistent `x_DWORD_D4794`/`x_DWORD_D4798` renderer state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerrainSlopeCamera {
    offset_x: i32,
    offset_y: i32,
}

/// Original terrain projection rejection and divisor-floor rule.
///
/// This is not a geometric near plane: points behind the camera remain
/// eligible until depth `-256`, and all eligible depths below `128` share the
/// divisor `128`.
#[must_use]
pub const fn terrain_projection_divisor(
    forward_depth: i32,
    horizontal_distance_squared: u32,
    render_cutoff_squared: u32,
) -> Option<i32> {
    if forward_depth <= -256 || horizontal_distance_squared >= render_cutoff_squared {
        None
    } else if forward_depth < 128 {
        Some(128)
    } else {
        Some(forward_depth)
    }
}

/// Original entity/billboard forward-depth and far-distance eligibility rule.
#[must_use]
pub const fn object_projection_eligible(
    forward_depth: i32,
    horizontal_distance_squared: u32,
    render_cutoff_squared: u32,
) -> bool {
    forward_depth > 64 && horizontal_distance_squared < render_cutoff_squared
}

impl TerrainSlopeCamera {
    /// Apply one original `DrawWorld_411A0` terrain-slope update.
    ///
    /// # Panics
    ///
    /// Panics when `ground_heights` is not the validated 256 by 256 source
    /// plane.
    #[must_use]
    pub fn update(
        &mut self,
        ground_heights: &[u8],
        camera_x: u16,
        camera_y: u16,
    ) -> TerrainSlopeCameraSample {
        assert_eq!(
            ground_heights.len(),
            256 * 256,
            "terrain-slope camera requires one 256x256 height plane"
        );

        let [x_low, x_high] = camera_x.to_le_bytes();
        let [y_low, y_high] = camera_y.to_le_bytes();
        let tile_x = if x_low < 128 {
            x_high.wrapping_sub(1)
        } else {
            x_high
        };
        let tile_y = if y_low < 128 {
            y_high.wrapping_sub(1)
        } else {
            y_high
        };
        let tile_x_2 = tile_x.wrapping_add(2);
        let tile_y_2 = tile_y.wrapping_add(2);
        let height =
            |x: u8, y: u8| i32::from(ground_heights[usize::from(x) + usize::from(y) * 256]);
        let h00 = height(tile_x, tile_y);
        let h20 = height(tile_x_2, tile_y);
        let h22 = height(tile_x_2, tile_y_2);
        let h02 = height(tile_x, tile_y_2);
        let target_x = (2 * (h02 + h00 - h20 - h22)).clamp(-100, 100);
        let target_y = (2 * (h00 + h20 - h22 - h02)).clamp(-100, 100);

        // Signed right shift is deliberately arithmetic, matching the original
        // x86/C++ build for negative deltas.
        self.offset_x += (target_x - self.offset_x) >> 3;
        self.offset_y += (target_y - self.offset_y) >> 3;

        TerrainSlopeCameraSample {
            target_x: target_x as i16,
            target_y: target_y as i16,
            offset_x: self.offset_x as i16,
            offset_y: self.offset_y as i16,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_height_plane_has_no_render_camera_offset() {
        let heights = vec![37; 256 * 256];
        let mut camera = TerrainSlopeCamera::default();
        assert_eq!(
            camera.update(&heights, 0x8080, 0x8080),
            TerrainSlopeCameraSample::default()
        );
    }

    #[test]
    fn source_four_sample_equations_and_stateful_easing_are_exact() {
        let mut heights = vec![0; 256 * 256];
        let index = |x: usize, y: usize| x + y * 256;
        heights[index(0x80, 0x80)] = 10;
        heights[index(0x82, 0x80)] = 20;
        heights[index(0x82, 0x82)] = 30;
        heights[index(0x80, 0x82)] = 40;
        let mut camera = TerrainSlopeCamera::default();
        assert_eq!(
            camera.update(&heights, 0x8080, 0x8080),
            TerrainSlopeCameraSample {
                target_x: 0,
                target_y: -80,
                offset_x: 0,
                offset_y: -10,
            }
        );
        assert_eq!(
            camera.update(&heights, 0x8080, 0x8080),
            TerrainSlopeCameraSample {
                target_x: 0,
                target_y: -80,
                offset_x: 0,
                offset_y: -19,
            }
        );
    }

    #[test]
    fn half_tile_selection_wrap_and_target_clamp_match_source_rules() {
        let mut heights = vec![0; 256 * 256];
        let index = |x: usize, y: usize| x + y * 256;
        // Low coordinate bytes below 128 select the preceding tile. At tile
        // zero that wraps to 255; the +2 samples then wrap to tile one.
        heights[index(255, 255)] = 255;
        heights[index(1, 255)] = 0;
        heights[index(1, 1)] = 0;
        heights[index(255, 1)] = 255;
        let mut camera = TerrainSlopeCamera::default();
        assert_eq!(
            camera.update(&heights, 0x0000, 0x0000),
            TerrainSlopeCameraSample {
                target_x: 100,
                target_y: 0,
                offset_x: 12,
                offset_y: 0,
            }
        );
    }

    #[test]
    fn terrain_projection_boundaries_pin_rejection_and_divisor_floor() {
        const CUTOFF: u32 = 26_214_400;
        assert_eq!(terrain_projection_divisor(-257, 0, CUTOFF), None);
        assert_eq!(terrain_projection_divisor(-256, 0, CUTOFF), None);
        assert_eq!(terrain_projection_divisor(-255, 0, CUTOFF), Some(128));
        assert_eq!(terrain_projection_divisor(0, 0, CUTOFF), Some(128));
        assert_eq!(terrain_projection_divisor(127, 0, CUTOFF), Some(128));
        assert_eq!(terrain_projection_divisor(128, 0, CUTOFF), Some(128));
        assert_eq!(terrain_projection_divisor(129, 0, CUTOFF), Some(129));
        assert_eq!(terrain_projection_divisor(129, CUTOFF, CUTOFF), None);
    }

    #[test]
    fn object_projection_boundary_is_strictly_greater_than_64() {
        const CUTOFF: u32 = 26_214_400;
        assert!(!object_projection_eligible(64, 0, CUTOFF));
        assert!(object_projection_eligible(65, 0, CUTOFF));
        assert!(!object_projection_eligible(65, CUTOFF, CUTOFF));
    }
}
