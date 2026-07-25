//! Exact isolated Level 1 outdoor terrain frame.
//!
//! This pipeline initializes the original 320×200 indexed sky, forms and
//! projects the 40×21 outdoor lattice, submits the complete reflected pass
//! before the complete primary pass, and executes mode 5 or mode 26 into the
//! indexed framebuffer. It deliberately does not claim a complete original
//! world frame: MC2 interleaves sprites and particles with terrain, while this
//! module returns their terrain-only coverage separately.

use crate::{
    SIN_16_16, legacy_sqrt,
    mode5_raster::{Mode5Vertex, rasterize_mode5_triangle_from_atlas},
    mode26_raster::{NIGHT_TABLES_BYTES, rasterize_mode26_triangle_from_atlas_into_framebuffer},
    terrain_grid::{OutdoorGridInput, OutdoorGridVertex, outdoor_grid},
    terrain_reflection::{
        MODE5_TERRAIN_SELECTOR, MODE26_REFLECTIVE_TERRAIN_SELECTOR, OutdoorProjection,
        OutdoorReflectionVertex, TERRAIN_FEATURE_REFLECTIVE_MATERIAL, TerrainPass,
        TerrainTriangleSubmission, project_outdoor_reflection_vertex, reflection_texture_cell,
        submit_outdoor_grid,
    },
};

pub const LEVEL1_VIEW_WIDTH: usize = 320;
pub const LEVEL1_VIEW_HEIGHT: usize = 200;
pub const LEVEL1_VIEW_PIXELS: usize = LEVEL1_VIEW_WIDTH * LEVEL1_VIEW_HEIGHT;
pub const LEVEL1_TERRAIN_ATLAS_WIDTH: usize = 256;
pub const LEVEL1_TERRAIN_TILE_SIZE: usize = 32;
pub const LEVEL1_TERRAIN_TILE_COLUMNS: usize = 8;
pub const LEVEL1_TERRAIN_TILE_COUNT: usize = 152;
pub const LEVEL1_TERRAIN_ATLAS_BYTES: usize =
    LEVEL1_TERRAIN_TILE_COUNT * LEVEL1_TERRAIN_TILE_SIZE * LEVEL1_TERRAIN_TILE_SIZE;
pub const LEVEL1_SKY_BYTES: usize = 256 * 256;

const UV_EDGE: i32 = 0x001f_ffff;
const BASE_UV_ROWS: [[[i32; 2]; 4]; 8] = [
    [[0, 0], [UV_EDGE, 0], [UV_EDGE, UV_EDGE], [0, UV_EDGE]],
    [[UV_EDGE, 0], [UV_EDGE, UV_EDGE], [0, UV_EDGE], [0, 0]],
    [[UV_EDGE, UV_EDGE], [0, UV_EDGE], [0, 0], [UV_EDGE, 0]],
    [[0, UV_EDGE], [0, 0], [UV_EDGE, 0], [UV_EDGE, UV_EDGE]],
    [[UV_EDGE, 0], [0, 0], [0, UV_EDGE], [UV_EDGE, UV_EDGE]],
    [[0, 0], [0, UV_EDGE], [UV_EDGE, UV_EDGE], [UV_EDGE, 0]],
    [[0, UV_EDGE], [UV_EDGE, UV_EDGE], [UV_EDGE, 0], [0, 0]],
    [[UV_EDGE, UV_EDGE], [UV_EDGE, 0], [0, 0], [0, UV_EDGE]],
];
const UV_ROW_ORDER: [usize; 32] = [
    0, 1, 2, 3, 4, 5, 6, 7, 6, 7, 4, 5, 2, 3, 0, 1, 5, 6, 7, 4, 3, 0, 1, 2, 1, 2, 3, 0, 7, 4, 5, 6,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level1TerrainFrameCamera {
    pub render_x: u16,
    pub render_y: u16,
    pub eye_z: i32,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub fov: i16,
    pub source_turn: i32,
    pub reflections: bool,
}

pub struct Level1TerrainFrameResources<'a> {
    pub terrain_types: &'a [u8],
    pub ground_heights: &'a [u8],
    pub shading: &'a [u8],
    pub angles: &'a [u8],
    /// Original 256-stride BL32N atlas, 8 tiles per row.
    pub terrain_atlas: &'a [u8],
    pub tablesn: &'a [u8],
    pub sky: &'a [u8],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerrainFrameStats {
    pub reflected_triangles: u32,
    pub primary_mode5_triangles: u32,
    pub primary_mode26_triangles: u32,
    pub terrain_pixel_writes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level1TerrainFrame {
    pub width: u16,
    pub height: u16,
    /// Complete indexed sky plus isolated terrain result.
    pub indices: Vec<u8>,
    /// 255 where this isolated terrain pipeline wrote at least once.
    pub terrain_coverage: Vec<u8>,
    pub index_hash: u64,
    pub coverage_hash: u64,
    pub stats: TerrainFrameStats,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerrainFrameError(pub String);

impl std::fmt::Display for TerrainFrameError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for TerrainFrameError {}

/// Render one exact isolated Level 1 terrain frame.
///
/// The only intentional omission is non-terrain framebuffer writers
/// (particles, sprites, carpet, HUD and effects). The returned coverage makes
/// that boundary explicit to clients.
pub fn render_level1_terrain_frame(
    camera: Level1TerrainFrameCamera,
    resources: Level1TerrainFrameResources<'_>,
) -> Result<Level1TerrainFrame, TerrainFrameError> {
    validate_resources(&resources)?;
    let projection = level1_projection(camera);
    let lattice = outdoor_grid(OutdoorGridInput {
        render_x: camera.render_x,
        render_y: camera.render_y,
        yaw: camera.yaw,
        turn: camera.source_turn,
        shading: resources.shading,
        angles: resources.angles,
    });
    let projected: [Option<OutdoorReflectionVertex>; 840] = std::array::from_fn(|index| {
        let source = lattice[index];
        let map_index = usize::from(source.tile_x) + usize::from(source.tile_y) * 256;
        let (texture_index, texture_uv_row) = reflection_texture_cell(
            source.tile_x,
            source.tile_y,
            camera.yaw,
            resources.terrain_types,
            resources.angles,
        );
        let mut projected = project_outdoor_reflection_vertex(
            source,
            resources.ground_heights[map_index],
            camera.eye_z,
            texture_index,
            texture_uv_row,
            false,
            projection,
        );
        if !camera.reflections
            && let Some(vertex) = projected.as_mut()
        {
            vertex.feature_flags &= !TERRAIN_FEATURE_REFLECTIVE_MATERIAL;
        }
        projected
    });
    let eye_for_submission = if camera.reflections {
        camera.eye_z
    } else {
        i32::MAX
    };
    let plan = submit_outdoor_grid(eye_for_submission, &lattice, &projected);
    let mut indices = draw_level1_sky(camera, resources.sky)?;
    let mut terrain_coverage = vec![0; LEVEL1_VIEW_PIXELS];
    let mut stats = TerrainFrameStats::default();

    for submission in plan.reflected.iter().chain(plan.primary.iter()) {
        raster_submission(
            *submission,
            &lattice,
            &projected,
            &resources,
            &mut indices,
            &mut terrain_coverage,
            &mut stats,
        )?;
    }

    Ok(Level1TerrainFrame {
        width: LEVEL1_VIEW_WIDTH as u16,
        height: LEVEL1_VIEW_HEIGHT as u16,
        index_hash: byte_hash(&indices),
        coverage_hash: byte_hash(&terrain_coverage),
        indices,
        terrain_coverage,
        stats,
    })
}

#[must_use]
pub fn level1_projection(camera: Level1TerrainFrameCamera) -> OutdoorProjection {
    let diagonal_squared =
        (LEVEL1_VIEW_WIDTH * LEVEL1_VIEW_WIDTH + LEVEL1_VIEW_HEIGHT * LEVEL1_VIEW_HEIGHT) as u32;
    let diagonal = legacy_sqrt(diagonal_squared) as i32;
    OutdoorProjection {
        scale: 7_i32
            .wrapping_mul(diagonal)
            .wrapping_mul(i32::from(camera.fov))
            >> 11,
        pitch_offset: i32::from(camera.pitch).wrapping_mul(LEVEL1_VIEW_WIDTH as i32) >> 8,
        roll: camera.roll,
        center_x: LEVEL1_VIEW_WIDTH as i32 / 2,
        center_y: LEVEL1_VIEW_HEIGHT as i32 / 2,
        viewport_width: LEVEL1_VIEW_WIDTH as i32,
        viewport_height: LEVEL1_VIEW_HEIGHT as i32,
    }
}

/// Reproduce `DrawSky_40950` for the qualified 320×200 Level 1 viewport.
pub fn draw_level1_sky(
    camera: Level1TerrainFrameCamera,
    sky: &[u8],
) -> Result<Vec<u8>, TerrainFrameError> {
    if sky.len() != LEVEL1_SKY_BYTES {
        return Err(TerrainFrameError(format!(
            "Level 1 sky must contain {LEVEL1_SKY_BYTES} bytes, got {}",
            sky.len()
        )));
    }
    let projection = level1_projection(camera);
    let roll = i32::from(camera.roll) & 0x07ff;
    let cos = SIN_16_16[usize::try_from(512 + roll).expect("masked sky cosine index")];
    let sin = SIN_16_16[usize::try_from(roll).expect("masked sky sine index")];
    let cos_step = cos.wrapping_shl(8) / LEVEL1_VIEW_WIDTH as i32;
    let sin_step = sin.wrapping_shl(8) / LEVEL1_VIEW_WIDTH as i32;

    let mut x_deltas = [0_u8; LEVEL1_VIEW_WIDTH];
    let mut y_deltas = [0_u8; LEVEL1_VIEW_WIDTH];
    let mut error_x = 0_i32;
    let mut error_y = 0_i32;
    let mut previous_x = 0_u8;
    let mut previous_y = 0_u8;
    for column in 0..LEVEL1_VIEW_WIDTH {
        let current_x = byte2(error_x);
        let current_y = byte2(error_y);
        x_deltas[column] = current_x.wrapping_sub(previous_x);
        y_deltas[column] = current_y.wrapping_sub(previous_y);
        previous_x = current_x;
        previous_y = current_y;
        error_x = error_x.wrapping_add(cos_step);
        error_y = error_y.wrapping_add(sin_step);
    }

    let origin_x = (sin.wrapping_mul(projection.pitch_offset).wrapping_neg() >> 16)
        .wrapping_add(projection.center_x);
    let origin_y = projection
        .center_y
        .wrapping_sub(cos.wrapping_mul(projection.pitch_offset) >> 16);
    let rotated_x = origin_x
        .wrapping_mul(cos_step)
        .wrapping_sub(origin_y.wrapping_mul(sin_step));
    let rotated_y = cos_step
        .wrapping_mul(origin_y)
        .wrapping_add(sin_step.wrapping_mul(origin_x));
    let mut row_u = (i32::from(camera.yaw as u16) << 15).wrapping_sub(rotated_x);
    let mut row_v = rotated_y.wrapping_neg();
    let mut frame = vec![0; LEVEL1_VIEW_PIXELS];

    for row in 0..LEVEL1_VIEW_HEIGHT {
        let mut u = byte2(row_u);
        let mut v = byte2(row_v);
        for column in 0..LEVEL1_VIEW_WIDTH {
            frame[row * LEVEL1_VIEW_WIDTH + column] = sky[(usize::from(v) << 8) | usize::from(u)];
            u = u.wrapping_add(x_deltas[column]);
            v = v.wrapping_add(y_deltas[column]);
        }
        row_u = row_u.wrapping_sub(sin_step);
        row_v = row_v.wrapping_add(cos_step);
    }
    Ok(frame)
}

fn raster_submission(
    submission: TerrainTriangleSubmission,
    lattice: &[OutdoorGridVertex; 840],
    projected: &[Option<OutdoorReflectionVertex>; 840],
    resources: &Level1TerrainFrameResources<'_>,
    indices: &mut [u8],
    coverage: &mut [u8],
    stats: &mut TerrainFrameStats,
) -> Result<(), TerrainFrameError> {
    let vertices = triangle_vertices(submission, lattice, projected)?;
    let texture_base = terrain_texture_base(submission.texture_index)?;
    match submission.raster_selector {
        MODE5_TERRAIN_SELECTOR => {
            let pixels = rasterize_mode5_triangle_from_atlas(
                vertices,
                LEVEL1_VIEW_WIDTH as i32,
                LEVEL1_VIEW_HEIGHT as i32,
                resources.terrain_atlas,
                texture_base,
                &resources.tablesn[..65_536],
            );
            for pixel in pixels {
                let index = usize::try_from(pixel.y).expect("mode-5 Y is clipped")
                    * LEVEL1_VIEW_WIDTH
                    + usize::try_from(pixel.sample.x).expect("mode-5 X is clipped");
                indices[index] = pixel.sample.output_index;
                coverage[index] = 255;
                stats.terrain_pixel_writes = stats.terrain_pixel_writes.wrapping_add(1);
            }
            match submission.pass {
                TerrainPass::Reflected => {
                    stats.reflected_triangles = stats.reflected_triangles.wrapping_add(1);
                }
                TerrainPass::Primary => {
                    stats.primary_mode5_triangles = stats.primary_mode5_triangles.wrapping_add(1);
                }
            }
        }
        MODE26_REFLECTIVE_TERRAIN_SELECTOR => {
            if submission.pass != TerrainPass::Primary {
                return Err(TerrainFrameError(
                    "mode 26 is valid only for the Level 1 primary terrain pass".to_owned(),
                ));
            }
            let pixels = rasterize_mode26_triangle_from_atlas_into_framebuffer(
                vertices,
                LEVEL1_VIEW_WIDTH as i32,
                LEVEL1_VIEW_HEIGHT as i32,
                resources.terrain_atlas,
                texture_base,
                resources.tablesn,
                indices,
            );
            for pixel in pixels {
                let index = usize::try_from(pixel.y).expect("mode-26 Y is clipped")
                    * LEVEL1_VIEW_WIDTH
                    + usize::try_from(pixel.sample.x).expect("mode-26 X is clipped");
                coverage[index] = 255;
                stats.terrain_pixel_writes = stats.terrain_pixel_writes.wrapping_add(1);
            }
            stats.primary_mode26_triangles = stats.primary_mode26_triangles.wrapping_add(1);
        }
        selector => {
            return Err(TerrainFrameError(format!(
                "unsupported Level 1 terrain raster selector {selector}; no approximation allowed"
            )));
        }
    }
    Ok(())
}

fn triangle_vertices(
    submission: TerrainTriangleSubmission,
    lattice: &[OutdoorGridVertex; 840],
    projected: &[Option<OutdoorReflectionVertex>; 840],
) -> Result<[Mode5Vertex; 3], TerrainFrameError> {
    let uv = source_uv_row(submission.texture_uv_row)?;
    let mut result = [Mode5Vertex::default(); 3];
    for (destination, source_index) in result.iter_mut().zip(submission.vertex_indices) {
        let projected_vertex = projected[source_index].ok_or_else(|| {
            TerrainFrameError(format!(
                "submitted terrain vertex {source_index} has no projected value"
            ))
        })?;
        let corner = cell_corner(submission.owner_index, source_index)?;
        let screen = match submission.pass {
            TerrainPass::Reflected => projected_vertex.reflected_screen,
            TerrainPass::Primary => projected_vertex.primary_screen,
        };
        *destination = Mode5Vertex {
            x: screen.x,
            y: screen.y,
            u: uv[corner][0],
            v: uv[corner][1],
            pnt5: lattice[source_index].pnt5.ok_or_else(|| {
                TerrainFrameError(format!(
                    "submitted terrain vertex {source_index} has no pnt5"
                ))
            })?,
        };
    }
    Ok(result)
}

fn cell_corner(owner: usize, vertex: usize) -> Result<usize, TerrainFrameError> {
    if vertex == owner {
        Ok(0)
    } else if vertex == owner + 1 {
        Ok(1)
    } else if vertex + 39 == owner {
        Ok(2)
    } else if vertex + 40 == owner {
        Ok(3)
    } else {
        Err(TerrainFrameError(format!(
            "terrain vertex {vertex} does not belong to owner cell {owner}"
        )))
    }
}

fn source_uv_row(row: u8) -> Result<[[i32; 2]; 4], TerrainFrameError> {
    let index = usize::from(row);
    let base = *UV_ROW_ORDER
        .get(index)
        .ok_or_else(|| TerrainFrameError(format!("terrain UV row {row} is out of bounds")))?;
    Ok(BASE_UV_ROWS[base])
}

fn terrain_texture_base(selector: u8) -> Result<usize, TerrainFrameError> {
    let selector = usize::from(selector);
    if selector >= LEVEL1_TERRAIN_TILE_COUNT {
        return Err(TerrainFrameError(format!(
            "terrain selector {selector} is outside the qualified Level 1 BL32N bank"
        )));
    }
    Ok((selector / LEVEL1_TERRAIN_TILE_COLUMNS)
        * LEVEL1_TERRAIN_TILE_SIZE
        * LEVEL1_TERRAIN_ATLAS_WIDTH
        + (selector % LEVEL1_TERRAIN_TILE_COLUMNS) * LEVEL1_TERRAIN_TILE_SIZE)
}

fn validate_resources(
    resources: &Level1TerrainFrameResources<'_>,
) -> Result<(), TerrainFrameError> {
    for (name, values) in [
        ("terrain types", resources.terrain_types),
        ("ground heights", resources.ground_heights),
        ("terrain shading", resources.shading),
        ("terrain angles", resources.angles),
    ] {
        if values.len() != 65_536 {
            return Err(TerrainFrameError(format!(
                "{name} must contain 65,536 bytes, got {}",
                values.len()
            )));
        }
    }
    if resources.terrain_atlas.len() != LEVEL1_TERRAIN_ATLAS_BYTES {
        return Err(TerrainFrameError(format!(
            "Level 1 BL32N atlas must contain {LEVEL1_TERRAIN_ATLAS_BYTES} bytes, got {}",
            resources.terrain_atlas.len()
        )));
    }
    if resources.tablesn.len() != NIGHT_TABLES_BYTES {
        return Err(TerrainFrameError(format!(
            "Level 1 TABLESN must contain {NIGHT_TABLES_BYTES} bytes, got {}",
            resources.tablesn.len()
        )));
    }
    if resources.sky.len() != LEVEL1_SKY_BYTES {
        return Err(TerrainFrameError(format!(
            "Level 1 SKYN must contain {LEVEL1_SKY_BYTES} bytes, got {}",
            resources.sky.len()
        )));
    }
    Ok(())
}

#[must_use]
pub fn byte_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

const fn byte2(value: i32) -> u8 {
    ((value >> 16) & 0xff) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_world::NativeWorld;
    use std::path::PathBuf;

    #[test]
    fn source_uv_table_preserves_all_32_rows_and_inclusive_fixed_edges() {
        assert_eq!(source_uv_row(0).unwrap(), BASE_UV_ROWS[0]);
        assert_eq!(source_uv_row(8).unwrap(), BASE_UV_ROWS[6]);
        assert_eq!(source_uv_row(16).unwrap(), BASE_UV_ROWS[5]);
        assert_eq!(source_uv_row(24).unwrap(), BASE_UV_ROWS[1]);
        assert_eq!(source_uv_row(31).unwrap(), BASE_UV_ROWS[6]);
        assert_eq!(source_uv_row(0).unwrap()[1][0], 0x001f_ffff);
        assert!(source_uv_row(32).is_err());
    }

    #[test]
    fn atlas_pointer_uses_source_256_stride_and_reaches_last_tile() {
        assert_eq!(terrain_texture_base(0).unwrap(), 0);
        assert_eq!(terrain_texture_base(7).unwrap(), 224);
        assert_eq!(terrain_texture_base(8).unwrap(), 8_192);
        assert_eq!(terrain_texture_base(151).unwrap(), 147_680);
        assert_eq!(
            terrain_texture_base(151).unwrap() + 31 * 256 + 31,
            LEVEL1_TERRAIN_ATLAS_BYTES - 1
        );
        assert!(terrain_texture_base(152).is_err());
    }

    #[test]
    fn qualified_projection_is_164_at_default_fov() {
        let camera = Level1TerrainFrameCamera {
            render_x: 0,
            render_y: 0,
            eye_z: 0,
            yaw: 0,
            pitch: 0,
            roll: 0,
            fov: 128,
            source_turn: 0,
            reflections: true,
        };
        assert_eq!(level1_projection(camera).scale, 164);
    }

    #[test]
    fn zero_sky_is_stable_and_complete() {
        let camera = Level1TerrainFrameCamera {
            render_x: 0,
            render_y: 0,
            eye_z: 0,
            yaw: 0,
            pitch: 0,
            roll: 0,
            fov: 128,
            source_turn: 0,
            reflections: true,
        };
        let frame = draw_level1_sky(camera, &[0; LEVEL1_SKY_BYTES]).unwrap();
        assert_eq!(frame.len(), LEVEL1_VIEW_PIXELS);
        assert!(frame.iter().all(|value| *value == 0));
    }

    #[test]
    fn qualified_owner_reset_and_low_water_frames_are_deterministic() {
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load(
            private.join("generated/level1-export-m"),
            private.join("generated/level1-asset-closure.json"),
        )
        .expect("validated Level 1 owner package");
        let atlas = std::fs::read(
            private.join("generated/level1-unity-asset-cache/terrain/bl32n.atlas.indices.u8"),
        )
        .expect("qualified Level 1 source-layout terrain atlas");
        let tablesn =
            std::fs::read(private.join("generated/level1-unity-asset-cache/palette/tablesn.u8"))
                .expect("qualified Level 1 TABLESN");
        let sky =
            std::fs::read(private.join("generated/level1-unity-asset-cache/sky/skyn.indices.u8"))
                .expect("qualified Level 1 SKYN");
        let planes = world.terrain_planes();
        let camera = Level1TerrainFrameCamera {
            render_x: 19_841,
            render_y: 56_958,
            eye_z: 5_152,
            yaw: 0,
            pitch: 0,
            roll: 0,
            fov: 128,
            source_turn: 8,
            reflections: true,
        };
        let reset_lattice = outdoor_grid(OutdoorGridInput {
            render_x: camera.render_x,
            render_y: camera.render_y,
            yaw: camera.yaw,
            turn: camera.source_turn,
            shading: &planes.shading,
            angles: &planes.angles,
        });
        let mut visible = 0;
        let mut culled = 0;
        let mut near = 0;
        let mut fog_ramp = 0;
        let mut fog_zero = 0;
        let mut parity = [0; 2];
        let mut d4_by_band = [0_u8; 3];
        for vertex in reset_lattice {
            if vertex.feature_flags & 0x0002 != 0 {
                culled += 1;
                continue;
            }
            visible += 1;
            parity[usize::from(vertex.feature_flags & 1)] += 1;
            let band = if vertex.distance_squared <= 14_745_600 {
                near += 1;
                0
            } else if vertex.distance_squared < 23_658_496 {
                fog_ramp += 1;
                1
            } else {
                fog_zero += 1;
                2
            };
            let map_index = usize::from(vertex.tile_x) + usize::from(vertex.tile_y) * 256;
            d4_by_band[band] |= 1 << ((planes.angles[map_index] >> 4) & 7);
        }
        assert_eq!((visible, culled), (672, 168));
        assert_eq!((near, fog_ramp, fog_zero), (388, 212, 72));
        assert_eq!(parity, [336, 336]);
        assert_eq!(d4_by_band, [0xff, 0xff, 0xff]);
        let render = || {
            render_level1_terrain_frame(
                camera,
                Level1TerrainFrameResources {
                    terrain_types: &planes.terrain_types,
                    ground_heights: &planes.ground_heights,
                    shading: &planes.shading,
                    angles: &planes.angles,
                    terrain_atlas: &atlas,
                    tablesn: &tablesn,
                    sky: &sky,
                },
            )
            .expect("qualified isolated Level 1 terrain frame")
        };
        let first = render();
        let second = render();
        assert_eq!(first, second);
        assert_eq!(first.index_hash, 0x2f38_beef_8e4a_62fb);
        assert_eq!(first.coverage_hash, 0x65fa_674a_e4af_efec);
        assert_eq!(
            first.stats,
            TerrainFrameStats {
                reflected_triangles: 0,
                primary_mode5_triangles: 578,
                primary_mode26_triangles: 0,
                terrain_pixel_writes: 37_174,
            }
        );
        assert!(first.stats.primary_mode5_triangles > 0);
        assert!(first.terrain_coverage.contains(&255));

        let low_camera = Level1TerrainFrameCamera {
            render_x: 16 << 8,
            render_y: 0,
            eye_z: 2_000,
            ..camera
        };
        let low = render_level1_terrain_frame(
            low_camera,
            Level1TerrainFrameResources {
                terrain_types: &planes.terrain_types,
                ground_heights: &planes.ground_heights,
                shading: &planes.shading,
                angles: &planes.angles,
                terrain_atlas: &atlas,
                tablesn: &tablesn,
                sky: &sky,
            },
        )
        .expect("qualified low-altitude reflection frame");
        assert_eq!(low.index_hash, 0x9721_91f6_b54d_1e2a);
        assert_eq!(low.coverage_hash, 0x5abf_682e_9728_07ca);
        assert_eq!(
            low.stats,
            TerrainFrameStats {
                reflected_triangles: 36,
                primary_mode5_triangles: 0,
                primary_mode26_triangles: 300,
                terrain_pixel_writes: 7_885,
            }
        );
        assert!(low.stats.reflected_triangles > 0);
        assert!(low.stats.primary_mode26_triangles > 0);

        let moving_low = render_level1_terrain_frame(
            Level1TerrainFrameCamera {
                source_turn: low_camera.source_turn + 1,
                ..low_camera
            },
            Level1TerrainFrameResources {
                terrain_types: &planes.terrain_types,
                ground_heights: &planes.ground_heights,
                shading: &planes.shading,
                angles: &planes.angles,
                terrain_atlas: &atlas,
                tablesn: &tablesn,
                sky: &sky,
            },
        )
        .expect("qualified moving low-altitude reflection frame");
        assert_eq!(moving_low.index_hash, 0x9992_4581_ea78_82b0);
        assert_eq!(moving_low.coverage_hash, 0x1424_ba7c_1c11_d071);
        assert_eq!(
            moving_low.stats,
            TerrainFrameStats {
                reflected_triangles: 36,
                primary_mode5_triangles: 0,
                primary_mode26_triangles: 300,
                terrain_pixel_writes: 7_932,
            }
        );

        for seam_y in [u16::MAX, 0] {
            let seam = render_level1_terrain_frame(
                Level1TerrainFrameCamera {
                    render_y: seam_y,
                    ..camera
                },
                Level1TerrainFrameResources {
                    terrain_types: &planes.terrain_types,
                    ground_heights: &planes.ground_heights,
                    shading: &planes.shading,
                    angles: &planes.angles,
                    terrain_atlas: &atlas,
                    tablesn: &tablesn,
                    sky: &sky,
                },
            )
            .expect("qualified map-seam terrain frame");
            let seam_lattice = outdoor_grid(OutdoorGridInput {
                render_x: camera.render_x,
                render_y: seam_y,
                yaw: camera.yaw,
                turn: camera.source_turn,
                shading: &planes.shading,
                angles: &planes.angles,
            });
            assert!(seam_lattice.iter().any(|vertex| vertex.tile_y == 0));
            assert!(seam_lattice.iter().any(|vertex| vertex.tile_y == 255));
            let expected = if seam_y == u16::MAX {
                (0xe519_7093_29e4_23e8, 0xd33b_c3da_61c8_1217, 68_931)
            } else {
                (0x0025_703f_712a_3129, 0x8210_0660_1a3c_8f55, 68_908)
            };
            assert_eq!(
                (
                    seam.index_hash,
                    seam.coverage_hash,
                    seam.stats.terrain_pixel_writes
                ),
                expected
            );
            assert_eq!(seam.stats.reflected_triangles, 0);
            assert_eq!(seam.stats.primary_mode5_triangles, 604);
            assert_eq!(seam.stats.primary_mode26_triangles, 0);
        }

        let no_reflections = render_level1_terrain_frame(
            Level1TerrainFrameCamera {
                reflections: false,
                ..low_camera
            },
            Level1TerrainFrameResources {
                terrain_types: &planes.terrain_types,
                ground_heights: &planes.ground_heights,
                shading: &planes.shading,
                angles: &planes.angles,
                terrain_atlas: &atlas,
                tablesn: &tablesn,
                sky: &sky,
            },
        )
        .expect("qualified reflection-disabled terrain frame");
        assert_eq!(no_reflections.stats.reflected_triangles, 0);
        assert_eq!(no_reflections.stats.primary_mode26_triangles, 0);
        assert!(no_reflections.stats.primary_mode5_triangles > 0);
    }
}
