//! Exact outdoor reflected-terrain helpers used by the Level 1 render path.
//!
//! With reflections enabled, the original renderer forms both the ordinary
//! terrain altitude and a wave-scaled inverse altitude for every eligible
//! vertex. It projects both surfaces, draws the inverse surface first when the
//! eye is below the source altitude gate, and then draws the primary terrain.
//! This module records that behavior without depending on Unity or a GPU.

use crate::{
    SIN_16_16,
    terrain_grid::{OutdoorGridVertex, TERRAIN_FEATURE_CULLED, TERRAIN_FEATURE_SEAM_SIDE},
};

pub const TERRAIN_FEATURE_PRIMARY_LEFT: u16 = 0x0008;
pub const TERRAIN_FEATURE_PRIMARY_RIGHT: u16 = 0x0010;
pub const TERRAIN_FEATURE_PRIMARY_ABOVE: u16 = 0x0020;
pub const TERRAIN_FEATURE_PRIMARY_BELOW: u16 = 0x0040;
/// The source sets this from the second half of `x_BYTE_D41D8`, indexed by
/// the cell texture. On the primary pass it selects raster mode 26.
pub const TERRAIN_FEATURE_REFLECTIVE_MATERIAL: u16 = 0x0080;
pub const TERRAIN_FEATURE_REFLECTED_LEFT: u16 = 0x0100;
pub const TERRAIN_FEATURE_REFLECTED_RIGHT: u16 = 0x0200;
pub const TERRAIN_FEATURE_REFLECTED_ABOVE: u16 = 0x0400;
pub const TERRAIN_FEATURE_REFLECTED_BELOW: u16 = 0x0800;
pub const TERRAIN_FEATURE_FLAT_SHADE: u16 = 0x1000;

pub const PRIMARY_OUTSIDE_MASK: u16 = TERRAIN_FEATURE_PRIMARY_LEFT
    | TERRAIN_FEATURE_PRIMARY_RIGHT
    | TERRAIN_FEATURE_PRIMARY_ABOVE
    | TERRAIN_FEATURE_PRIMARY_BELOW;
pub const REFLECTED_OUTSIDE_MASK: u16 = TERRAIN_FEATURE_REFLECTED_LEFT
    | TERRAIN_FEATURE_REFLECTED_RIGHT
    | TERRAIN_FEATURE_REFLECTED_ABOVE
    | TERRAIN_FEATURE_REFLECTED_BELOW;

pub const MODE5_TERRAIN_SELECTOR: u8 = 5;
pub const MODE7_FLAT_TERRAIN_SELECTOR: u8 = 7;
pub const MODE26_REFLECTIVE_TERRAIN_SELECTOR: u8 = 26;
pub const REFLECTION_EYE_Z_LIMIT: i32 = 4_096;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScreenPoint {
    pub x: i32,
    pub y: i32,
}

/// Source projection values already recovered by the render-camera path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutdoorProjection {
    pub scale: i32,
    pub pitch_offset: i32,
    pub roll: i16,
    pub center_x: i32,
    pub center_y: i32,
    pub viewport_width: i32,
    pub viewport_height: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutdoorReflectionVertex {
    pub primary_altitude: i32,
    pub inverse_altitude: i32,
    pub primary_screen: ScreenPoint,
    pub reflected_screen: ScreenPoint,
    pub feature_flags: u16,
    pub texture_index: u8,
    pub texture_uv_row: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerrainPass {
    Reflected,
    Primary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainTriangleSubmission {
    pub pass: TerrainPass,
    /// Lower-left lattice owner of the source terrain cell.
    pub owner_index: usize,
    pub vertex_indices: [usize; 3],
    pub texture_index: u8,
    pub texture_uv_row: u8,
    pub raster_selector: u8,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TerrainQuadSubmission {
    pub reflected: Vec<TerrainTriangleSubmission>,
    pub primary: Vec<TerrainTriangleSubmission>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TerrainSubmissionPlan {
    /// Every reflected terrain triangle in exact painter order.
    pub reflected: Vec<TerrainTriangleSubmission>,
    /// Every primary terrain triangle in exact painter order. This complete
    /// pass follows the complete reflected pass in the source framebuffer.
    pub primary: Vec<TerrainTriangleSubmission>,
}

/// Reproduce the row-local painter-order split driven by feature bit `0x4`.
///
/// The source first submits cell owners from the left edge up to (but not
/// including) the split, then submits the remaining owners from right to left.
/// Bit `0x4` is not a cell visibility gate.
#[must_use]
pub fn terrain_row_anchor_order(row_start: usize, row_feature_flags: &[u16; 40]) -> Vec<usize> {
    assert!(row_start >= 40);
    assert!(row_start + 39 < 840);
    let split = (0..39)
        .find(|column| row_feature_flags[column + 1] & TERRAIN_FEATURE_SEAM_SIDE != 0)
        .unwrap_or(39);
    let mut owners = Vec::with_capacity(39);
    owners.extend((0..split).map(|column| row_start + column));
    owners.extend((split..39).rev().map(|column| row_start + column));
    owners
}

/// Schedule the complete 20×39 outdoor terrain surface without interleaving
/// reflected and primary cells.
#[must_use]
pub fn submit_outdoor_grid(
    eye_z: i32,
    lattice: &[OutdoorGridVertex; 840],
    projected: &[Option<OutdoorReflectionVertex>; 840],
) -> TerrainSubmissionPlan {
    let mut plan = TerrainSubmissionPlan::default();
    for row in (1..=20).rev() {
        let row_start = row * 40;
        let row_flags = std::array::from_fn(|column| lattice[row_start + column].feature_flags);
        for anchor in terrain_row_anchor_order(row_start, &row_flags) {
            let quad = submit_outdoor_quad(
                anchor,
                eye_z,
                [
                    projected[anchor],
                    projected[anchor + 1],
                    projected[anchor - 39],
                    projected[anchor - 40],
                ],
            );
            plan.reflected.extend(quad.reflected);
            plan.primary.extend(quad.primary);
        }
    }
    plan
}

/// The wave-scaled inverse altitude used only by the outdoor reflection path.
#[must_use]
pub fn reflected_inverse_altitude(ground_height: u8, wave: i32, eye_z: i32) -> i32 {
    let scale = (wave >> 4).wrapping_add(0x8000);
    let reflected_height = i32::from(ground_height).wrapping_mul(scale) >> 10;
    reflected_height.wrapping_neg().wrapping_sub(eye_z)
}

/// The reflective-material bit is the second 164-byte half of the source
/// material table. It is distinct from terrain angle bit 3: the angle controls
/// the height wave, while this table controls primary-pass raster mode 26.
#[must_use]
pub const fn terrain_material_is_reflective(texture_index: u8) -> bool {
    matches!(
        texture_index,
        0 | 35
            | 36
            | 37
            | 38
            | 39
            | 44
            | 45
            | 46
            | 47
            | 80
            | 81
            | 82
            | 83
            | 102
            | 103
            | 104
            | 105
            | 106
            | 107
            | 108
            | 109
            | 110
            | 111
            | 112
            | 113
            | 114
            | 115
            | 116
            | 117
            | 118
            | 119
            | 126
            | 127
    )
}

/// Texture lookup is offset one cell toward the view-sector corner. The UV
/// row combines the four-way yaw sector with the texture cell's angle bits.
#[must_use]
pub fn reflection_texture_cell(
    tile_x: u8,
    tile_y: u8,
    yaw: i16,
    terrain_types: &[u8],
    angles: &[u8],
) -> (u8, u8) {
    assert_eq!(terrain_types.len(), 65_536);
    assert_eq!(angles.len(), 65_536);
    let quadrant = (((i32::from(yaw) & 0x07ff) + 256) >> 9) & 3;
    let (offset_x, offset_y) = match quadrant {
        0 => (0_i8, 0_i8),
        1 => (-1, 0),
        2 => (-1, -1),
        3 => (0, -1),
        _ => unreachable!("quadrant is masked to two bits"),
    };
    let texture_x = tile_x.wrapping_add_signed(offset_x);
    let texture_y = tile_y.wrapping_add_signed(offset_y);
    let index = usize::from(texture_x) + usize::from(texture_y) * 256;
    let texture_index = terrain_types[index];
    let texture_uv_row =
        u8::try_from(quadrant).expect("quadrant fits u8") + ((angles[index] >> 2) & 0x1c);
    (texture_index, texture_uv_row)
}

/// Project both Level 1 terrain surfaces and attach the source feature bits.
///
/// Returns `None` when the lattice stage rejected the vertex; the original
/// skipped all altitude, projection, material and pnt5 writes in that case.
#[must_use]
pub fn project_outdoor_reflection_vertex(
    lattice: OutdoorGridVertex,
    ground_height: u8,
    eye_z: i32,
    texture_index: u8,
    texture_uv_row: u8,
    flat_shade: bool,
    projection: OutdoorProjection,
) -> Option<OutdoorReflectionVertex> {
    if lattice.feature_flags & TERRAIN_FEATURE_CULLED != 0 {
        return None;
    }
    assert!(projection.viewport_width >= 0);
    assert!(projection.viewport_height >= 0);

    let divisor = lattice.view_depth.max(128);
    let primary_altitude = i32::from(ground_height)
        .wrapping_mul(32)
        .wrapping_sub(eye_z)
        .wrapping_add(lattice.surface_displacement);
    let inverse_altitude = reflected_inverse_altitude(ground_height, lattice.raw_wave, eye_z);
    let projected_x = projection.scale.wrapping_mul(lattice.view_x) / divisor;
    let projected_primary_y = projection
        .pitch_offset
        .wrapping_add(projection.scale.wrapping_mul(primary_altitude) / divisor);
    let projected_reflected_y = projection
        .pitch_offset
        .wrapping_add(projection.scale.wrapping_mul(inverse_altitude) / divisor);

    let roll = i32::from(projection.roll) & 0x07ff;
    let sin = SIN_16_16[usize::try_from(roll).expect("masked roll fits usize")];
    let cos = SIN_16_16[usize::try_from(512 + roll).expect("cosine index fits usize")];
    let rotate = |vertical: i32| {
        let x = cos
            .wrapping_mul(projected_x)
            .wrapping_sub(sin.wrapping_mul(vertical))
            >> 16;
        let y = sin
            .wrapping_mul(projected_x)
            .wrapping_add(cos.wrapping_mul(vertical))
            >> 16;
        ScreenPoint {
            x: x.wrapping_add(projection.center_x),
            y: projection.center_y.wrapping_sub(y),
        }
    };
    let primary_screen = rotate(projected_primary_y);
    let reflected_screen = rotate(projected_reflected_y);

    let mut feature_flags = lattice.feature_flags;
    add_outside_flags(
        primary_screen,
        projection.viewport_width,
        projection.viewport_height,
        &mut feature_flags,
        [
            TERRAIN_FEATURE_PRIMARY_LEFT,
            TERRAIN_FEATURE_PRIMARY_RIGHT,
            TERRAIN_FEATURE_PRIMARY_ABOVE,
            TERRAIN_FEATURE_PRIMARY_BELOW,
        ],
    );
    add_outside_flags(
        reflected_screen,
        projection.viewport_width,
        projection.viewport_height,
        &mut feature_flags,
        [
            TERRAIN_FEATURE_REFLECTED_LEFT,
            TERRAIN_FEATURE_REFLECTED_RIGHT,
            TERRAIN_FEATURE_REFLECTED_ABOVE,
            TERRAIN_FEATURE_REFLECTED_BELOW,
        ],
    );
    if terrain_material_is_reflective(texture_index) {
        feature_flags |= TERRAIN_FEATURE_REFLECTIVE_MATERIAL;
    }
    if flat_shade {
        feature_flags |= TERRAIN_FEATURE_FLAT_SHADE;
    }

    Some(OutdoorReflectionVertex {
        primary_altitude,
        inverse_altitude,
        primary_screen,
        reflected_screen,
        feature_flags,
        texture_index,
        texture_uv_row,
    })
}

/// Build the exact triangle call order for one 40-column lattice cell.
///
/// `anchor` is the lower-left source vertex (`row * 40 + column`), so valid
/// cells have row 1..=20 and column 0..=38. The caller supplies the four
/// already projected vertices at `[anchor, anchor+1, anchor-39, anchor-40]`.
#[must_use]
pub fn submit_outdoor_quad(
    anchor: usize,
    eye_z: i32,
    vertices: [Option<OutdoorReflectionVertex>; 4],
) -> TerrainQuadSubmission {
    assert!((40..840).contains(&anchor));
    assert!(anchor % 40 < 39);
    let [lower_left, lower_right, upper_right, upper_left] = vertices;
    let Some(lower_left) = lower_left else {
        return TerrainQuadSubmission::default();
    };
    let Some(lower_right) = lower_right else {
        return TerrainQuadSubmission::default();
    };
    let Some(upper_right) = upper_right else {
        return TerrainQuadSubmission::default();
    };
    let Some(upper_left) = upper_left else {
        return TerrainQuadSubmission::default();
    };
    let indices = [anchor, anchor + 1, anchor - 39, anchor - 40];
    let flags = [
        lower_left.feature_flags,
        lower_right.feature_flags,
        upper_right.feature_flags,
        upper_left.feature_flags,
    ];
    let any = flags
        .into_iter()
        .fold(0, |combined, value| combined | value);
    let all = flags
        .into_iter()
        .fold(u16::MAX, |combined, value| combined & value);
    let parity_one = lower_left.feature_flags & 1 != 0;
    let mut reflected = Vec::with_capacity(2);
    let mut primary = Vec::with_capacity(2);

    // The inverse surface is gated by eye height, a non-zero texture index,
    // and the ordinary bit-2 vertex rejection. It deliberately does not use
    // the shared reflected-outside mask; mode 5 performs final clipping.
    if eye_z < REFLECTION_EYE_Z_LIMIT
        && lower_left.texture_index != 0
        && any & TERRAIN_FEATURE_CULLED == 0
    {
        let pairs = if parity_one {
            [
                [indices[0], indices[3], indices[1]],
                [indices[3], indices[2], indices[1]],
            ]
        } else {
            [
                [indices[0], indices[2], indices[1]],
                [indices[0], indices[3], indices[2]],
            ]
        };
        reflected.extend(
            pairs
                .into_iter()
                .map(|vertex_indices| TerrainTriangleSubmission {
                    pass: TerrainPass::Reflected,
                    owner_index: anchor,
                    vertex_indices,
                    texture_index: lower_left.texture_index,
                    texture_uv_row: lower_left.texture_uv_row,
                    raster_selector: MODE5_TERRAIN_SELECTOR,
                }),
        );
    }

    if any & TERRAIN_FEATURE_CULLED == 0 && all & PRIMARY_OUTSIDE_MASK == 0 {
        let selector = if lower_left.feature_flags & TERRAIN_FEATURE_REFLECTIVE_MATERIAL != 0 {
            MODE26_REFLECTIVE_TERRAIN_SELECTOR
        } else if lower_left.feature_flags & TERRAIN_FEATURE_FLAT_SHADE != 0 {
            MODE7_FLAT_TERRAIN_SELECTOR
        } else {
            MODE5_TERRAIN_SELECTOR
        };
        let pairs = if parity_one {
            [
                [indices[0], indices[1], indices[3]],
                [indices[3], indices[1], indices[2]],
            ]
        } else {
            [
                [indices[0], indices[1], indices[2]],
                [indices[0], indices[2], indices[3]],
            ]
        };
        primary.extend(
            pairs
                .into_iter()
                .map(|vertex_indices| TerrainTriangleSubmission {
                    pass: TerrainPass::Primary,
                    owner_index: anchor,
                    vertex_indices,
                    texture_index: lower_left.texture_index,
                    texture_uv_row: lower_left.texture_uv_row,
                    raster_selector: selector,
                }),
        );
    }

    TerrainQuadSubmission { reflected, primary }
}

fn add_outside_flags(
    point: ScreenPoint,
    width: i32,
    height: i32,
    flags: &mut u16,
    [left, right, above, below]: [u16; 4],
) {
    if point.x < 0 {
        *flags |= left;
    } else if point.x >= width {
        *flags |= right;
    }
    if point.y < 0 {
        *flags |= above;
    } else if point.y >= height {
        *flags |= below;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        native_world::NativeWorld,
        terrain_grid::{OutdoorGridInput, outdoor_grid},
    };
    use std::path::PathBuf;

    fn lattice() -> OutdoorGridVertex {
        OutdoorGridVertex {
            view_x: 256,
            view_depth: 256,
            raw_wave: 0,
            surface_displacement: 0,
            pnt5: Some(0x0020_0000),
            ..OutdoorGridVertex::default()
        }
    }

    fn projection() -> OutdoorProjection {
        OutdoorProjection {
            scale: 1_000,
            pitch_offset: 10,
            roll: 0,
            center_x: 160,
            center_y: 100,
            viewport_width: 320,
            viewport_height: 200,
        }
    }

    #[test]
    fn inverse_altitude_pins_wave_scale_and_eye_offset() {
        assert_eq!(reflected_inverse_altitude(100, 0, 2_000), -5_200);
        assert_eq!(reflected_inverse_altitude(100, 65_536, 2_000), -5_600);
        assert_eq!(reflected_inverse_altitude(100, -65_536, 2_000), -4_800);
    }

    #[test]
    fn material_table_second_half_is_distinct_from_angle_water() {
        assert!(terrain_material_is_reflective(0));
        assert!(terrain_material_is_reflective(35));
        assert!(terrain_material_is_reflective(127));
        assert!(!terrain_material_is_reflective(8));
        assert!(!terrain_material_is_reflective(34));
        assert!(!terrain_material_is_reflective(128));
    }

    #[test]
    fn complete_reflective_material_half_table_is_pinned() {
        let actual = (0_u8..164)
            .filter(|selector| terrain_material_is_reflective(*selector))
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            [
                0, 35, 36, 37, 38, 39, 44, 45, 46, 47, 80, 81, 82, 83, 102, 103, 104, 105, 106,
                107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 126, 127,
            ]
        );
    }

    #[test]
    fn texture_cell_wraps_and_uv_row_combines_sector_and_angle() {
        let mut terrain = vec![0; 65_536];
        let mut angles = vec![0; 65_536];
        let index = 255 + 255 * 256;
        terrain[index] = 47;
        angles[index] = 0x70;
        assert_eq!(
            reflection_texture_cell(0, 0, 1_024, &terrain, &angles),
            (47, 30)
        );
    }

    #[test]
    fn projection_forms_primary_and_reflected_points_before_screen_flags() {
        let vertex =
            project_outdoor_reflection_vertex(lattice(), 100, 1_000, 35, 6, false, projection())
                .expect("eligible vertex");
        assert_eq!(vertex.primary_altitude, 2_200);
        assert_eq!(vertex.inverse_altitude, -4_200);
        assert_eq!(
            vertex.primary_screen,
            ScreenPoint {
                x: 1_160,
                y: -8_503
            }
        );
        assert_eq!(
            vertex.reflected_screen,
            ScreenPoint {
                x: 1_160,
                y: 16_496
            }
        );
        assert_ne!(vertex.feature_flags & TERRAIN_FEATURE_PRIMARY_RIGHT, 0);
        assert_ne!(vertex.feature_flags & TERRAIN_FEATURE_PRIMARY_ABOVE, 0);
        assert_ne!(vertex.feature_flags & TERRAIN_FEATURE_REFLECTED_RIGHT, 0);
        assert_ne!(vertex.feature_flags & TERRAIN_FEATURE_REFLECTED_BELOW, 0);
        assert_ne!(
            vertex.feature_flags & TERRAIN_FEATURE_REFLECTIVE_MATERIAL,
            0
        );
    }

    #[test]
    fn quarter_turn_roll_uses_the_source_fixed_point_rotation_order() {
        let mut rolled = projection();
        rolled.roll = 512;
        let vertex = project_outdoor_reflection_vertex(lattice(), 100, 1_000, 34, 0, false, rolled)
            .expect("eligible vertex");
        assert_eq!(vertex.primary_screen, ScreenPoint { x: -8_443, y: -900 });
        assert_eq!(vertex.reflected_screen, ScreenPoint { x: 16_556, y: -900 });
    }

    #[test]
    fn culled_vertex_skips_all_reflection_writes() {
        let mut source = lattice();
        source.feature_flags |= TERRAIN_FEATURE_CULLED;
        assert_eq!(
            project_outdoor_reflection_vertex(source, 100, 0, 1, 0, false, projection()),
            None
        );
    }

    fn submitted_vertex(flags: u16, texture: u8) -> Option<OutdoorReflectionVertex> {
        Some(OutdoorReflectionVertex {
            primary_altitude: 0,
            inverse_altitude: 0,
            primary_screen: ScreenPoint::default(),
            reflected_screen: ScreenPoint::default(),
            feature_flags: flags,
            texture_index: texture,
            texture_uv_row: 12,
        })
    }

    #[test]
    fn parity_zero_submits_reflection_first_then_primary_in_source_order() {
        let result = submit_outdoor_quad(
            800,
            2_000,
            [
                submitted_vertex(TERRAIN_FEATURE_REFLECTIVE_MATERIAL, 35),
                submitted_vertex(0, 35),
                submitted_vertex(0, 35),
                submitted_vertex(0, 35),
            ],
        );
        assert_eq!(result.reflected.len(), 2);
        assert_eq!(result.primary.len(), 2);
        assert_eq!(result.reflected[0].pass, TerrainPass::Reflected);
        assert_eq!(result.reflected[0].vertex_indices, [800, 761, 801]);
        assert_eq!(result.reflected[1].vertex_indices, [800, 760, 761]);
        assert_eq!(result.primary[0].pass, TerrainPass::Primary);
        assert_eq!(result.primary[0].vertex_indices, [800, 801, 761]);
        assert_eq!(result.primary[1].vertex_indices, [800, 761, 760]);
        assert_eq!(
            result.primary[0].raster_selector,
            MODE26_REFLECTIVE_TERRAIN_SELECTOR
        );
    }

    #[test]
    fn parity_one_reverses_reflection_winding_and_uses_primary_source_order() {
        let result = submit_outdoor_quad(
            800,
            2_000,
            [
                submitted_vertex(1, 34),
                submitted_vertex(0, 34),
                submitted_vertex(0, 34),
                submitted_vertex(0, 34),
            ],
        );
        assert_eq!(result.reflected[0].vertex_indices, [800, 760, 801]);
        assert_eq!(result.reflected[1].vertex_indices, [760, 761, 801]);
        assert_eq!(result.primary[0].vertex_indices, [800, 801, 760]);
        assert_eq!(result.primary[1].vertex_indices, [760, 801, 761]);
        assert_eq!(result.primary[0].raster_selector, MODE5_TERRAIN_SELECTOR);
    }

    #[test]
    fn flat_primary_uses_mode7_while_reflection_is_forced_to_mode5() {
        let result = submit_outdoor_quad(
            800,
            0,
            [
                submitted_vertex(TERRAIN_FEATURE_FLAT_SHADE, 1),
                submitted_vertex(0, 1),
                submitted_vertex(0, 1),
                submitted_vertex(0, 1),
            ],
        );
        assert_eq!(result.reflected[0].raster_selector, MODE5_TERRAIN_SELECTOR);
        assert_eq!(
            result.primary[0].raster_selector,
            MODE7_FLAT_TERRAIN_SELECTOR
        );
    }

    #[test]
    fn reflection_gate_and_primary_shared_outside_gate_are_independent() {
        let all_right = TERRAIN_FEATURE_PRIMARY_RIGHT | TERRAIN_FEATURE_REFLECTED_RIGHT;
        let vertices = [
            submitted_vertex(all_right, 1),
            submitted_vertex(all_right, 1),
            submitted_vertex(all_right, 1),
            submitted_vertex(all_right, 1),
        ];
        let below = submit_outdoor_quad(800, REFLECTION_EYE_Z_LIMIT - 1, vertices);
        assert_eq!(below.reflected.len(), 2);
        assert!(below.primary.is_empty());

        let at_gate = submit_outdoor_quad(800, REFLECTION_EYE_Z_LIMIT, vertices);
        assert!(at_gate.reflected.is_empty());
        assert!(at_gate.primary.is_empty());
    }

    #[test]
    fn zero_texture_suppresses_only_the_inverse_pass() {
        let zero = submit_outdoor_quad(
            800,
            0,
            [
                submitted_vertex(0, 0),
                submitted_vertex(0, 0),
                submitted_vertex(0, 0),
                submitted_vertex(0, 0),
            ],
        );
        assert!(zero.reflected.is_empty());
        assert_eq!(zero.primary.len(), 2);
    }

    #[test]
    fn seam_side_partitions_row_order_without_hiding_its_cell() {
        let mut row = [0_u16; 40];
        row[13] |= TERRAIN_FEATURE_SEAM_SIDE;
        let order = terrain_row_anchor_order(800, &row);
        assert_eq!(&order[..12], &(800..812).collect::<Vec<_>>());
        assert_eq!(&order[12..], &(812..839).rev().collect::<Vec<_>>());

        let seam_cell = submit_outdoor_quad(
            812,
            0,
            [
                submitted_vertex(0, 1),
                submitted_vertex(TERRAIN_FEATURE_SEAM_SIDE, 1),
                submitted_vertex(TERRAIN_FEATURE_SEAM_SIDE, 1),
                submitted_vertex(0, 1),
            ],
        );
        assert_eq!(seam_cell.reflected.len(), 2);
        assert_eq!(seam_cell.primary.len(), 2);
    }

    #[test]
    fn row_without_seam_split_submits_left_to_right() {
        let row = [0_u16; 40];
        assert_eq!(
            terrain_row_anchor_order(800, &row),
            (800..839).collect::<Vec<_>>()
        );
    }

    #[test]
    fn full_grid_plan_keeps_complete_reflection_before_complete_primary_pass() {
        let lattice = [OutdoorGridVertex::default(); 840];
        let vertex = submitted_vertex(0, 1);
        let projected = [vertex; 840];
        let plan = submit_outdoor_grid(0, &lattice, &projected);
        assert_eq!(plan.reflected.len(), 20 * 39 * 2);
        assert_eq!(plan.primary.len(), 20 * 39 * 2);
        assert!(
            plan.reflected
                .iter()
                .all(|triangle| triangle.pass == TerrainPass::Reflected)
        );
        assert!(
            plan.primary
                .iter()
                .all(|triangle| triangle.pass == TerrainPass::Primary)
        );
        assert_eq!(plan.reflected[0].vertex_indices, [800, 761, 801]);
        assert_eq!(plan.primary[0].vertex_indices, [800, 801, 761]);
        assert_eq!(
            plan.reflected
                .last()
                .expect("last reflection")
                .vertex_indices,
            [78, 38, 39]
        );
        assert_eq!(
            plan.primary.last().expect("last primary").vertex_indices,
            [78, 39, 38]
        );
    }

    #[test]
    fn culled_vertex_before_seam_does_not_hide_reverse_side_anchors() {
        let mut lattice = [OutdoorGridVertex::default(); 840];
        lattice[805].feature_flags |= TERRAIN_FEATURE_CULLED;
        lattice[813].feature_flags |= TERRAIN_FEATURE_SEAM_SIDE;
        let mut projected = [None; 840];
        let vertex = submitted_vertex(0, 1);
        for index in [838, 839, 799, 798] {
            projected[index] = vertex;
        }

        let plan = submit_outdoor_grid(0, &lattice, &projected);
        assert_eq!(plan.reflected.len(), 2);
        assert_eq!(plan.primary.len(), 2);
        assert_eq!(plan.reflected[0].vertex_indices, [838, 799, 839]);
        assert_eq!(plan.primary[0].vertex_indices, [838, 839, 799]);
    }

    #[test]
    fn preserved_level1_rows_pin_owner_heights_materials_and_inverse_altitudes() {
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load(
            private.join("generated/level1-export-m"),
            private.join("generated/level1-asset-closure.json"),
        )
        .expect("validated Level 1 owner package");
        let planes = world.terrain_planes();
        let grid = outdoor_grid(OutdoorGridInput {
            render_x: 19_841,
            render_y: 56_958,
            yaw: 0,
            turn: 8,
            shading: &planes.shading,
            angles: &planes.angles,
        });
        let projection = OutdoorProjection {
            scale: 164,
            pitch_offset: 0,
            roll: 0,
            center_x: 160,
            center_y: 100,
            viewport_width: 320,
            viewport_height: 200,
        };

        for (
            owner,
            tile,
            selector,
            height,
            shade,
            angle,
            wave,
            primary_altitude,
            inverse_altitude,
            uv,
            primary_selector,
        ) in [
            (
                94,
                (72, 221),
                46,
                149,
                34,
                20,
                -24_832,
                -384,
                -9_694,
                4,
                MODE26_REFLECTIVE_TERRAIN_SELECTOR,
            ),
            (
                135,
                (73, 220),
                0,
                139,
                41,
                64,
                0,
                -704,
                -9_600,
                16,
                MODE26_REFLECTIVE_TERRAIN_SELECTOR,
            ),
            (
                138,
                (76, 220),
                1,
                143,
                58,
                49,
                0,
                -576,
                -9_728,
                12,
                MODE5_TERRAIN_SELECTOR,
            ),
        ] {
            let source = grid[owner];
            assert_eq!((source.tile_x, source.tile_y), tile);
            assert_eq!(
                (source.shade, source.angle, source.raw_wave),
                (shade, angle, wave)
            );
            let map_index = usize::from(source.tile_x) + usize::from(source.tile_y) * 256;
            assert_eq!(planes.ground_heights[map_index], height);
            let (texture_index, texture_uv_row) = reflection_texture_cell(
                source.tile_x,
                source.tile_y,
                0,
                &planes.terrain_types,
                &planes.angles,
            );
            assert_eq!((texture_index, texture_uv_row), (selector, uv));
            let projected = project_outdoor_reflection_vertex(
                source,
                height,
                5_152,
                texture_index,
                texture_uv_row,
                false,
                projection,
            )
            .expect("selected preserved row is eligible");
            assert_eq!(
                (projected.primary_altitude, projected.inverse_altitude),
                (primary_altitude, inverse_altitude)
            );
            let mut submission_vertex = projected;
            submission_vertex.feature_flags &=
                TERRAIN_FEATURE_REFLECTIVE_MATERIAL | TERRAIN_FEATURE_FLAT_SHADE | 1;
            let quad =
                submit_outdoor_quad(800, REFLECTION_EYE_Z_LIMIT, [Some(submission_vertex); 4]);
            assert!(quad.reflected.is_empty());
            assert_eq!(quad.primary[0].raster_selector, primary_selector);
        }
    }
}
