//! Exact integer helpers used by the original outdoor terrain presentation.
//!
//! This is deliberately renderer-independent.  The source renderer computed
//! `pnt5` at terrain-grid vertices, then the mode-5 rasterizer interpolated it
//! in screen space and used bits 16..23 as the TABLESN row.

use crate::SIN_16_16;

pub const TERRAIN_RENDER_CUTOFF_SQUARED: u32 = 26_214_400;
pub const TERRAIN_FOG_START_SQUARED: u32 = 14_745_600;
pub const TERRAIN_FOG_END_SQUARED: u32 = 23_658_496;
pub const TERRAIN_FOG_THICKNESS_SQUARED: i64 = 8_912_896;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainPnt5 {
    /// The vertex survives the original behind-camera and horizontal cutoffs.
    pub visible: bool,
    /// Raw product of the two source sine-table samples after each is shifted by 8.
    pub wave: i32,
    /// Wave contribution retained for pnt5 after the angle and shade gates.
    pub lighting_wave: i32,
    /// Land-height displacement applied by the outdoor water branch.
    pub land_displacement: i32,
    /// Pre-raster fixed-point brightness value.
    pub pnt5: i32,
    /// TABLESN row at the vertex. Raster pixels use the same bits after
    /// mode-5 screen-linear interpolation of `pnt5`.
    pub tablesn_row: u8,
}

/// Reproduce the outdoor `GameRenderOriginal` pnt5 vertex calculation.
///
/// `tile_x` and `tile_y` intentionally accept unwrapped coordinates: the
/// source's `& 0x7ff` sine indices provide the exact 256-tile map wrap.
/// `view_y` is the source renderer's rotated forward coordinate. The source
/// rejects vertices at or behind -256 before projecting them.
/// `distance_squared` is the source renderer's horizontal x/y product.
#[must_use]
pub fn outdoor_terrain_pnt5(
    turn: i32,
    tile_x: i32,
    tile_y: i32,
    shade: u8,
    angle: u8,
    view_y: i32,
    distance_squared: u32,
) -> TerrainPnt5 {
    let phase = turn.wrapping_shl(6);
    let sine_y = SIN_16_16[usize::try_from((phase.wrapping_add(tile_y.wrapping_shl(7))) & 0x07ff)
        .expect("masked sine index fits usize")]
        >> 8;
    let sine_x = SIN_16_16[usize::try_from((tile_x.wrapping_shl(7).wrapping_add(phase)) & 0x07ff)
        .expect("masked sine index fits usize")]
        >> 8;
    let wave = sine_y.wrapping_mul(sine_x);
    let water = angle & 8 != 0;
    // The source subtracts the height wave before the shade >= 56 lighting
    // suppression. Bright water therefore still moves vertically.
    let land_displacement = if water { -(wave >> 10) } else { 0 };
    let lighting_wave = if water && shade < 56 { wave } else { 0 };

    let static_brightness = (i32::from(shade) << 8) + 128;
    let base = (static_brightness << 8).wrapping_add(lighting_wave.wrapping_mul(8));
    let pnt5 = if distance_squared <= TERRAIN_FOG_START_SQUARED {
        base
    } else if distance_squared < TERRAIN_FOG_END_SQUARED {
        // The signed 64-bit product is required: replacing this with an
        // unsigned or 32-bit product changes transition rows. Rust integer
        // division, like C/C++, truncates toward zero.
        i32::try_from(
            i64::from(base) * i64::from(TERRAIN_FOG_END_SQUARED - distance_squared)
                / TERRAIN_FOG_THICKNESS_SQUARED,
        )
        .expect("fog-scaled pnt5 fits i32")
    } else {
        0
    };

    TerrainPnt5 {
        visible: view_y > -256 && distance_squared < TERRAIN_RENDER_CUTOFF_SQUARED,
        wave,
        lighting_wave,
        land_displacement,
        pnt5,
        tablesn_row: ((pnt5 >> 16) & 0xff) as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_boundary_vectors_pin_wave_fog_and_cutoff() {
        let cases = [
            // turn, x, y, shade, angle, view_y, distance, wave, displacement, pnt5, row, visible
            (1, 0, 0, 40, 8, 128, 0, 2_401, -2, 2_673_416, 40, true),
            (
                1,
                1,
                1,
                40,
                8,
                128,
                TERRAIN_FOG_START_SQUARED,
                20_164,
                -19,
                2_815_520,
                42,
                true,
            ),
            (
                1,
                2,
                3,
                40,
                8,
                128,
                TERRAIN_FOG_START_SQUARED + 1,
                53_212,
                -51,
                3_079_903,
                46,
                true,
            ),
            (
                1, 5, 7, 40, 8, 128, 19_202_048, 10_388, -10, 1_368_656, 20, true,
            ),
            (
                1,
                255,
                255,
                40,
                8,
                128,
                TERRAIN_FOG_END_SQUARED - 1,
                2_500,
                -2,
                0,
                0,
                true,
            ),
            (
                1,
                255,
                255,
                40,
                8,
                128,
                TERRAIN_FOG_END_SQUARED,
                2_500,
                -2,
                0,
                0,
                true,
            ),
            (
                1,
                255,
                255,
                40,
                8,
                128,
                TERRAIN_RENDER_CUTOFF_SQUARED - 1,
                2_500,
                -2,
                0,
                0,
                true,
            ),
            (
                1,
                255,
                255,
                40,
                8,
                128,
                TERRAIN_RENDER_CUTOFF_SQUARED,
                2_500,
                -2,
                0,
                0,
                false,
            ),
        ];
        for (turn, x, y, shade, angle, view_y, distance, wave, displacement, pnt5, row, visible) in
            cases
        {
            assert_eq!(
                outdoor_terrain_pnt5(turn, x, y, shade, angle, view_y, distance),
                TerrainPnt5 {
                    visible,
                    wave,
                    lighting_wave: wave,
                    land_displacement: displacement,
                    pnt5,
                    tablesn_row: row,
                }
            );
        }
    }

    #[test]
    fn source_suppression_and_wrapping_vectors_are_exact() {
        assert_eq!(
            outdoor_terrain_pnt5(1, 42, 99, 56, 8, 128, 0),
            TerrainPnt5 {
                visible: true,
                wave: -53_463,
                lighting_wave: 0,
                land_displacement: 53,
                pnt5: 3_702_784,
                tablesn_row: 56,
            }
        );
        assert_eq!(
            outdoor_terrain_pnt5(1, 42, 99, 55, 0, 128, 0),
            TerrainPnt5 {
                visible: true,
                wave: -53_463,
                lighting_wave: 0,
                land_displacement: 0,
                pnt5: 3_637_248,
                tablesn_row: 55,
            }
        );
        assert_eq!(
            outdoor_terrain_pnt5(65_535, 256, -1, 55, 8, 128, 14_757_945),
            TerrainPnt5 {
                visible: true,
                wave: 7_150,
                lighting_wave: 7_150,
                land_displacement: -6,
                pnt5: 3_689_330,
                tablesn_row: 56,
            }
        );
        assert_eq!(
            outdoor_terrain_pnt5(0, 4, 4, 0, 8, 128, 0),
            TerrainPnt5 {
                visible: true,
                wave: 65_536,
                lighting_wave: 65_536,
                land_displacement: -64,
                pnt5: 557_056,
                tablesn_row: 8,
            }
        );
    }

    #[test]
    fn source_visibility_rejects_the_exact_behind_camera_boundary() {
        let visible = outdoor_terrain_pnt5(0, 0, 0, 40, 0, -255, 0);
        let culled = outdoor_terrain_pnt5(0, 0, 0, 40, 0, -256, 0);
        assert!(visible.visible);
        assert!(!culled.visible);
        assert_eq!(visible.pnt5, culled.pnt5);
    }
}
