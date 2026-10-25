//! Original Cave floor/ceiling material pass. Actor interleaving is separate.
//!
//! `DrawTerrainAndParticles_3C080` uses the same yaw lattice as outdoors, but
//! clears to key colour 1 and submits ceiling texture 1 before each floor cell.
//! Its water displacement and phase are different from the outdoor branch.

use crate::{
    SIN_16_16,
    classic_hd::{ClassicHdFrameWriter, ClassicHdRenderProfile, byte_hash},
    classic_hd_raster::{
        CLASSIC_HD_TABLESN_BYTES, CLASSIC_HD_TERRAIN_ATLAS_BYTES, ClassicHdRasterError,
        rasterize_classic_hd_mode5_triangle, source_uv_row, terrain_texture_base,
    },
    classic_hd_terrain::ClassicHdProjection,
    mode5_raster::Mode5Vertex,
    render_camera::terrain_projection_divisor,
    terrain_frame::Level1TerrainFrameCamera,
    terrain_grid::{
        OUTDOOR_GRID_COLUMNS, OUTDOOR_GRID_ROWS, OutdoorGridInput, OutdoorGridVertex,
        TERRAIN_FEATURE_CHECKERBOARD, TERRAIN_FEATURE_CULLED, outdoor_grid,
    },
    terrain_presentation::{
        TERRAIN_FOG_END_SQUARED, TERRAIN_FOG_START_SQUARED, TERRAIN_FOG_THICKNESS_SQUARED,
    },
    terrain_reflection::{reflection_texture_cell, terrain_row_anchor_order},
};

const SEALED: u16 = 0x80;
const OUTSIDE: u16 = 0x78;

#[derive(Clone, Copy)]
pub struct CaveTerrainResources<'a> {
    pub terrain_types: &'a [u8],
    pub ground_heights: &'a [u8],
    pub ceiling_heights: &'a [u8],
    pub shading: &'a [u8],
    pub angles: &'a [u8],
    pub clear_index: u8,
}

#[derive(Clone, Copy)]
struct Vertex {
    floor: Mode5Vertex,
    ceiling: Mode5Vertex,
    flags: u16,
    texture: u8,
    uv: u8,
}

#[derive(Clone)]
struct Triangle {
    vertices: [Mode5Vertex; 3],
    texture: u8,
    ceiling: bool,
}

#[derive(Clone)]
pub struct CaveTerrainPlan {
    profile_revision: u64,
    clear_index: u8,
    triangles: Vec<Triangle>,
    pub input_hash: u64,
    pub command_hash: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CaveRasterReceipt {
    pub floor_pixels_written: u64,
    pub ceiling_pixels_written: u64,
}

impl CaveTerrainPlan {
    #[cfg(test)]
    pub(crate) fn without_ceiling_submissions_for_test(&self) -> Self {
        let mut plan = self.clone();
        plan.triangles.retain(|triangle| !triangle.ceiling);
        plan.command_hash = hash_commands(&plan.triangles);
        plan
    }

    pub fn initial_indices(&self, profile: &ClassicHdRenderProfile) -> Vec<u8> {
        vec![self.clear_index; profile.extent()]
    }

    /// Material-only pass: no sky, outdoor reflections, or actor-occlusion claim.
    pub fn rasterize(
        &self,
        writer: &mut ClassicHdFrameWriter<'_>,
        atlas: &[u8],
        tables: &[u8],
        provenance: u8,
    ) -> Result<CaveRasterReceipt, ClassicHdRasterError> {
        if writer.profile().profile_revision != self.profile_revision
            || atlas.len() != CLASSIC_HD_TERRAIN_ATLAS_BYTES
            || tables.len() != 83_456
        {
            return Err(ClassicHdRasterError(
                "Cave raster profile or material extent differs".into(),
            ));
        }
        for triangle in &self.triangles {
            terrain_texture_base(triangle.texture)?;
        }
        let mut receipt = CaveRasterReceipt::default();
        for triangle in &self.triangles {
            let stage = rasterize_classic_hd_mode5_triangle(
                writer,
                triangle.vertices,
                atlas,
                terrain_texture_base(triangle.texture)?,
                &tables[..CLASSIC_HD_TABLESN_BYTES],
                provenance | if triangle.ceiling { 4 } else { 2 },
            )?;
            if triangle.ceiling {
                receipt.ceiling_pixels_written += stage.written_pixels;
            } else {
                receipt.floor_pixels_written += stage.written_pixels;
            }
        }
        Ok(receipt)
    }
}

pub fn build_cave_terrain(
    profile: &ClassicHdRenderProfile,
    camera: Level1TerrainFrameCamera,
    resources: CaveTerrainResources<'_>,
) -> Result<CaveTerrainPlan, ClassicHdRasterError> {
    if !profile.is_compatibility_profile() {
        return Err(ClassicHdRasterError(
            "Cave terrain requires the original 320x200 profile".into(),
        ));
    }
    for plane in [
        resources.terrain_types,
        resources.ground_heights,
        resources.ceiling_heights,
        resources.shading,
        resources.angles,
    ] {
        if plane.len() != 65_536 {
            return Err(ClassicHdRasterError(
                "Cave terrain requires five complete live planes".into(),
            ));
        }
    }
    let projection = ClassicHdProjection::from_profile(profile, camera);
    // Only tile/view coordinates and source seam/parity bits are shared.
    // Every Cave altitude, wave, shade, and clipping bit is computed below.
    let grid = outdoor_grid(OutdoorGridInput {
        render_x: camera.render_x,
        render_y: camera.render_y,
        yaw: camera.yaw,
        turn: camera.source_turn,
        shading: resources.shading,
        angles: resources.angles,
    });
    let vertices: Vec<_> = grid
        .iter()
        .map(|vertex| project(*vertex, camera, projection, resources))
        .collect();
    let mut triangles = Vec::new();
    for row in (1..OUTDOOR_GRID_ROWS).rev() {
        let start = row * OUTDOOR_GRID_COLUMNS;
        let flags = std::array::from_fn(|column| grid[start + column].feature_flags);
        for owner in terrain_row_anchor_order(start, &flags) {
            let indices = [owner, owner + 1, owner - 39, owner - 40];
            let Some(cell) = indices
                .map(|index| vertices[index])
                .into_iter()
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            let any = cell.iter().fold(0, |flags, vertex| flags | vertex.flags);
            let all = cell
                .iter()
                .fold(u16::MAX, |flags, vertex| flags & vertex.flags);
            if all & SEALED != 0 || any & TERRAIN_FEATURE_CULLED != 0 {
                continue;
            }
            let uv = source_uv_row(cell[0].uv)?;
            let parity = cell[0].flags & TERRAIN_FEATURE_CHECKERBOARD != 0;
            let ceiling = if parity {
                [[0, 3, 1], [3, 2, 1]]
            } else {
                [[0, 2, 1], [0, 3, 2]]
            };
            append_triangles(&mut triangles, &cell, uv, ceiling, 1, true);
            if all & OUTSIDE == 0 {
                let floor = if parity {
                    [[0, 1, 3], [3, 1, 2]]
                } else {
                    [[0, 1, 2], [0, 2, 3]]
                };
                append_triangles(&mut triangles, &cell, uv, floor, cell[0].texture, false);
            }
        }
    }
    let mut input = b"mc2-cave-terrain/v1".to_vec();
    input.extend_from_slice(&profile.profile_revision.to_le_bytes());
    input.extend_from_slice(&camera.render_x.to_le_bytes());
    input.extend_from_slice(&camera.render_y.to_le_bytes());
    input.extend_from_slice(&camera.eye_z.to_le_bytes());
    for value in [camera.yaw, camera.pitch, camera.roll, camera.fov] {
        input.extend_from_slice(&value.to_le_bytes());
    }
    input.extend_from_slice(&camera.source_turn.to_le_bytes());
    input.push(resources.clear_index);
    for plane in [
        resources.terrain_types,
        resources.ground_heights,
        resources.ceiling_heights,
        resources.shading,
        resources.angles,
    ] {
        input.extend_from_slice(plane);
    }
    let command_hash = hash_commands(&triangles);
    Ok(CaveTerrainPlan {
        profile_revision: profile.profile_revision,
        clear_index: resources.clear_index,
        triangles,
        input_hash: byte_hash(&input),
        command_hash,
    })
}

fn hash_commands(triangles: &[Triangle]) -> u64 {
    let mut commands = Vec::new();
    for triangle in triangles {
        commands.extend_from_slice(&[triangle.texture, u8::from(triangle.ceiling)]);
        for v in triangle.vertices {
            for value in [v.x, v.y, v.u, v.v, v.pnt5] {
                commands.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    byte_hash(&commands)
}

fn append_triangles(
    out: &mut Vec<Triangle>,
    cell: &[Vertex],
    uv: [[i32; 2]; 4],
    pairs: [[usize; 3]; 2],
    texture: u8,
    ceiling: bool,
) {
    for corners in pairs {
        out.push(Triangle {
            texture,
            ceiling,
            vertices: corners.map(|corner| {
                let mut vertex = if ceiling {
                    cell[corner].ceiling
                } else {
                    cell[corner].floor
                };
                [vertex.u, vertex.v] = uv[corner];
                vertex
            }),
        });
    }
}

fn project(
    grid: OutdoorGridVertex,
    camera: Level1TerrainFrameCamera,
    projection: ClassicHdProjection,
    resources: CaveTerrainResources<'_>,
) -> Option<Vertex> {
    let divisor = terrain_projection_divisor(
        grid.view_depth,
        grid.distance_squared,
        projection.render_cutoff_squared,
    )?;
    let index = usize::from(grid.tile_x) + usize::from(grid.tile_y) * 256;
    let wave = if resources.terrain_types[index] == 0 {
        let phase = camera.source_turn.wrapping_mul(32);
        let x = (i32::from(grid.tile_x) << 7).wrapping_add(phase) & 2047;
        let y = (i32::from(grid.tile_y) << 7).wrapping_add(phase) & 2047;
        (SIN_16_16[x as usize] >> 8).wrapping_mul(SIN_16_16[y as usize] >> 8)
    } else {
        0
    };
    let floor_altitude = (i32::from(resources.ground_heights[index]) * 32)
        .wrapping_sub(camera.eye_z)
        .wrapping_sub(wave >> 13);
    let ceiling_altitude =
        (i32::from(resources.ceiling_heights[index]) * 32).wrapping_sub(camera.eye_z);
    let brightness = ((i32::from(grid.shade) << 8) + 128) << 8;
    let brightness = brightness.wrapping_add(if grid.shade < 56 { 8 * wave } else { 0 });
    let pnt5 = if grid.distance_squared <= TERRAIN_FOG_START_SQUARED {
        brightness
    } else if grid.distance_squared < TERRAIN_FOG_END_SQUARED {
        (i64::from(brightness) * i64::from(TERRAIN_FOG_END_SQUARED - grid.distance_squared)
            / TERRAIN_FOG_THICKNESS_SQUARED) as i32
    } else {
        0
    };
    let x = projection.scale.wrapping_mul(grid.view_x) / divisor;
    let roll = (i32::from(camera.roll) & 2047) as usize;
    let sin = SIN_16_16[roll];
    let cos = SIN_16_16[512 + roll];
    let point = |altitude: i32| {
        let y = projection
            .pitch_offset
            .wrapping_add(projection.scale.wrapping_mul(altitude) / divisor);
        Mode5Vertex {
            x: ((cos.wrapping_mul(x).wrapping_sub(sin.wrapping_mul(y))) >> 16)
                .wrapping_add(projection.center_x),
            y: projection
                .center_y
                .wrapping_sub((sin.wrapping_mul(x).wrapping_add(cos.wrapping_mul(y))) >> 16),
            u: 0,
            v: 0,
            pnt5,
        }
    };
    let floor = point(floor_altitude);
    let ceiling = point(ceiling_altitude);
    let mut flags = grid.feature_flags;
    if grid.angle & 8 != 0 {
        flags |= SEALED;
    }
    if floor.x < projection.viewport_x {
        flags |= 8;
    } else if floor.x >= projection.viewport_right() {
        flags |= 16;
    }
    if floor.y < projection.viewport_y {
        flags |= 32;
    } else if floor.y >= projection.viewport_bottom() {
        flags |= 64;
    }
    let (texture, uv) = reflection_texture_cell(
        grid.tile_x,
        grid.tile_y,
        camera.yaw,
        resources.terrain_types,
        resources.angles,
    );
    Some(Vertex {
        floor,
        ceiling,
        flags,
        texture,
        uv,
    })
}
