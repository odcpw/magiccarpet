//! Exact camera-oriented 40 by 21 outdoor terrain lattice.
//!
//! `GameRenderOriginal::DrawTerrainAndParticles_3C080` does not evaluate fog
//! from a generic world-space radius. It first constructs this fixed lattice,
//! rotates each coordinate through independently truncated 16.16 products,
//! and only then squares the resulting view coordinates. Keeping that order
//! is necessary at fog and cull boundaries.

use crate::{
    SIN_16_16,
    render_camera::terrain_projection_divisor,
    terrain_presentation::{TERRAIN_RENDER_CUTOFF_SQUARED, outdoor_terrain_pnt5},
};

pub const OUTDOOR_GRID_COLUMNS: usize = 40;
pub const OUTDOOR_GRID_ROWS: usize = 21;
pub const OUTDOOR_GRID_VERTICES: usize = OUTDOOR_GRID_COLUMNS * OUTDOOR_GRID_ROWS;

pub const TERRAIN_FEATURE_CHECKERBOARD: u16 = 0x0001;
pub const TERRAIN_FEATURE_CULLED: u16 = 0x0002;
pub const TERRAIN_FEATURE_SEAM_SIDE: u16 = 0x0004;

#[derive(Clone, Copy, Debug)]
pub struct OutdoorGridInput<'a> {
    /// Slope-adjusted render position, narrowed to the source 16-bit domain.
    pub render_x: u16,
    pub render_y: u16,
    pub yaw: i16,
    pub turn: i32,
    pub shading: &'a [u8],
    pub angles: &'a [u8],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OutdoorGridVertex {
    pub tile_x: u8,
    pub tile_y: u8,
    pub view_x: i32,
    pub view_depth: i32,
    pub distance_squared: u32,
    pub feature_flags: u16,
    pub shade: u8,
    pub angle: u8,
    pub raw_wave: i32,
    pub surface_displacement: i32,
    pub lighting_wave: i32,
    /// `None` means the source rejected the vertex before assigning pnt5.
    pub pnt5: Option<i32>,
    pub vertex_tablesn_row: Option<u8>,
}

/// Fill the original outdoor terrain lattice in row-major order.
///
/// # Panics
///
/// Panics unless both source planes contain exactly 65,536 bytes.
#[must_use]
pub fn outdoor_grid(input: OutdoorGridInput<'_>) -> [OutdoorGridVertex; OUTDOOR_GRID_VERTICES] {
    assert_eq!(
        input.shading.len(),
        256 * 256,
        "outdoor grid requires one 256x256 shading plane"
    );
    assert_eq!(
        input.angles.len(),
        256 * 256,
        "outdoor grid requires one 256x256 angle plane"
    );

    let yaw = i32::from(input.yaw) & 0x07ff;
    let quadrant = usize::try_from(((yaw + 256) >> 9) & 3).expect("quadrant fits usize");
    let residual = (((yaw + 256) & 0x01ff) - 256) & 0x07ff;
    let sin = SIN_16_16[usize::try_from(residual).expect("masked sine index fits usize")];
    let cos = SIN_16_16[usize::try_from(512 + residual).expect("cosine index fits usize")];
    let [x_low, x_high] = input.render_x.to_le_bytes();
    let [y_low, y_high] = input.render_y.to_le_bytes();

    let (a1_start, a2_start) = match quadrant {
        0 => (-i32::from(x_low) - 4_864, i32::from(y_low) - 256),
        1 => (-i32::from(y_low) - 4_864, -i32::from(x_low)),
        2 => (i32::from(x_low) - 4_864, -i32::from(y_low)),
        3 => (i32::from(y_low) - 4_864, i32::from(x_low) - 256),
        _ => unreachable!("quadrant is masked to two bits"),
    };

    let mut out = [OutdoorGridVertex::default(); OUTDOOR_GRID_VERTICES];
    for row in 0..OUTDOOR_GRID_ROWS {
        let a2 = a2_start + i32::try_from(row * 256).expect("row lattice offset fits i32");
        for column in 0..OUTDOOR_GRID_COLUMNS {
            let a1 =
                a1_start + i32::try_from(column * 256).expect("column lattice offset fits i32");
            let view_x = (a1.wrapping_mul(cos) >> 16) - (a2.wrapping_mul(sin) >> 16);
            let view_depth = (a1.wrapping_mul(sin) >> 16) + (a2.wrapping_mul(cos) >> 16);
            let distance_squared = view_x
                .wrapping_mul(view_x)
                .wrapping_add(view_depth.wrapping_mul(view_depth))
                .cast_unsigned();
            let (tile_x, tile_y) = tile_for(
                quadrant,
                x_high,
                y_high,
                u8::try_from(row).expect("row fits u8"),
                u8::try_from(column).expect("column fits u8"),
            );
            let map_index = usize::from(tile_x) + usize::from(tile_y) * 256;
            let shade = input.shading[map_index];
            let angle = input.angles[map_index];
            let value = outdoor_terrain_pnt5(
                input.turn,
                i32::from(tile_x),
                i32::from(tile_y),
                shade,
                angle,
                view_depth,
                distance_squared,
            );
            let eligible = terrain_projection_divisor(
                view_depth,
                distance_squared,
                TERRAIN_RENDER_CUTOFF_SQUARED,
            )
            .is_some();
            debug_assert_eq!(eligible, value.visible);

            let mut feature_flags = if a1 >= 0 {
                TERRAIN_FEATURE_SEAM_SIDE
            } else {
                0
            };
            feature_flags |= u16::from(tile_x.wrapping_add(tile_y) & 1);
            if !eligible {
                feature_flags |= TERRAIN_FEATURE_CULLED;
            }

            out[row * OUTDOOR_GRID_COLUMNS + column] = OutdoorGridVertex {
                tile_x,
                tile_y,
                view_x,
                view_depth,
                distance_squared,
                feature_flags,
                shade,
                angle,
                raw_wave: value.wave,
                surface_displacement: value.land_displacement,
                lighting_wave: value.lighting_wave,
                pnt5: eligible.then_some(value.pnt5),
                vertex_tablesn_row: eligible.then_some(value.tablesn_row),
            };
        }
    }
    out
}

const fn tile_for(
    quadrant: usize,
    camera_x_high: u8,
    camera_y_high: u8,
    row: u8,
    column: u8,
) -> (u8, u8) {
    match quadrant {
        0 => (
            camera_x_high.wrapping_sub(19).wrapping_add(column),
            camera_y_high.wrapping_add(1).wrapping_sub(row),
        ),
        1 => (
            camera_x_high.wrapping_add(row),
            camera_y_high.wrapping_sub(19).wrapping_add(column),
        ),
        2 => (
            camera_x_high.wrapping_add(19).wrapping_sub(column),
            camera_y_high.wrapping_add(row),
        ),
        3 => (
            camera_x_high.wrapping_add(1).wrapping_sub(row),
            camera_y_high.wrapping_add(19).wrapping_sub(column),
        ),
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn planes(shade: u8, angle: u8) -> (Vec<u8>, Vec<u8>) {
        (vec![shade; 256 * 256], vec![angle; 256 * 256])
    }

    #[test]
    fn cardinal_quadrants_pin_map_wrapping_and_rotated_coordinates() {
        let (shading, angles) = planes(40, 0);
        for (yaw, tile, view) in [
            (0, (255, 87), (-4_916, -85)),
            (512, (18, 67), (-5_035, -52)),
            (1_024, (37, 86), (-4_812, -171)),
            (1_536, (19, 105), (-4_693, -204)),
        ] {
            let grid = outdoor_grid(OutdoorGridInput {
                render_x: 0x1234,
                render_y: 0x56ab,
                yaw,
                turn: 0,
                shading: &shading,
                angles: &angles,
            });
            assert_eq!((grid[0].tile_x, grid[0].tile_y), tile);
            assert_eq!((grid[0].view_x, grid[0].view_depth), view);
        }
    }

    #[test]
    fn independent_fixed_point_shifts_change_the_non_cardinal_distance() {
        let (shading, angles) = planes(40, 0);
        let grid = outdoor_grid(OutdoorGridInput {
            render_x: 0x0100,
            render_y: 0x00ff,
            yaw: 128,
            turn: 0,
            shading: &shading,
            angles: &angles,
        });
        let vertex = grid[19];
        assert_eq!((vertex.view_x, vertex.view_depth), (1, -1));
        assert_eq!(vertex.distance_squared, 2);
    }

    #[test]
    fn water_height_precedes_bright_shade_lighting_suppression() {
        let (mut shading, mut angles) = planes(0, 0);
        let index = 1 + 256;
        shading[index] = 32;
        angles[index] = 8;
        let water = outdoor_grid(OutdoorGridInput {
            render_x: 0x0100,
            render_y: 0x00ff,
            yaw: 0,
            turn: 0,
            shading: &shading,
            angles: &angles,
        })[19];
        assert_eq!((water.tile_x, water.tile_y), (1, 1));
        assert_eq!((water.view_x, water.view_depth), (0, -1));
        assert_eq!(water.raw_wave, 9_409);
        assert_eq!(water.surface_displacement, -9);
        assert_eq!(water.lighting_wave, 9_409);
        assert_eq!(water.pnt5, Some(2_205_192));
        assert_eq!(water.vertex_tablesn_row, Some(33));

        shading[index] = 56;
        let bright = outdoor_grid(OutdoorGridInput {
            render_x: 0x0100,
            render_y: 0x00ff,
            yaw: 0,
            turn: 0,
            shading: &shading,
            angles: &angles,
        })[19];
        assert_eq!(bright.surface_displacement, -9);
        assert_eq!(bright.lighting_wave, 0);
        assert_eq!(bright.pnt5, Some(3_702_784));
    }

    #[test]
    fn fog_and_cull_boundaries_match_the_source_lattice() {
        let (shading, angles) = planes(32, 0);
        let grid = outdoor_grid(OutdoorGridInput {
            render_x: 0,
            render_y: 0,
            yaw: 0,
            turn: 0,
            shading: &shading,
            angles: &angles,
        });
        let at = |row: usize, column: usize| grid[row * OUTDOOR_GRID_COLUMNS + column];

        assert_eq!(at(16, 19).distance_squared, 14_745_600);
        assert_eq!(at(16, 19).pnt5, Some(2_129_920));
        assert_eq!(at(17, 19).distance_squared, 16_777_216);
        assert_eq!(at(17, 19).pnt5, Some(1_644_423));
        assert_eq!(at(20, 19).distance_squared, 23_658_496);
        assert_eq!(at(20, 19).pnt5, Some(0));

        let far = at(1, 39);
        assert_eq!((far.view_x, far.view_depth), (5_120, 0));
        assert_eq!(far.distance_squared, 26_214_400);
        assert_eq!(far.pnt5, None);
        assert_ne!(far.feature_flags & TERRAIN_FEATURE_CULLED, 0);

        let behind = at(0, 19);
        assert_eq!(behind.view_depth, -256);
        assert_eq!(behind.pnt5, None);
        assert_ne!(behind.feature_flags & TERRAIN_FEATURE_CULLED, 0);
    }
}
