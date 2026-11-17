//! Original Cave floor/ceiling pass and source-cell sprite occlusion order.
//!
//! `DrawTerrainAndParticles_3C080` uses the same yaw lattice as outdoors, but
//! clears to key colour 1 and submits ceiling texture 1 before each floor cell.
//! Its water displacement and phase are different from the outdoor branch.

use crate::{
    SIN_16_16,
    classic_hd::{ClassicHdFrameWriter, ClassicHdRenderProfile, byte_hash},
    classic_hd_raster::{
        CLASSIC_HD_TABLESN_BYTES, CLASSIC_HD_TERRAIN_ATLAS_BYTES, ClassicHdRasterError,
        rasterize_classic_hd_mode5_triangle, rasterize_classic_hd_mode7_triangle,
        source_uv_row, terrain_texture_base,
    },
    classic_hd_terrain::ClassicHdProjection,
    mode5_raster::{Mode5Vertex, rasterize_mode5_triangle_from_tile_atlas,
        rasterize_mode7_triangle_from_tile_atlas, source_flat_terrain_shade},
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
    cell_order: u16,
    flat_shade: u8,
}

#[derive(Clone)]
pub struct CaveTerrainPlan {
    profile_revision: u64,
    clear_index: u8,
    flat_shading: bool,
    triangles: Vec<Triangle>,
    cell_orders: Vec<u16>,
    cell_textures: Vec<u8>,
    pub input_hash: u64,
    pub command_hash: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CaveRasterReceipt {
    pub floor_pixels_written: u64,
    pub ceiling_pixels_written: u64,
}

impl CaveTerrainPlan {
    pub(crate) fn apply_flat_shading(&mut self, enabled: bool) {
        if !enabled { return; }
        self.flat_shading = true;
        // Both Cave traversal branches explicitly select mode 5 for the
        // ceiling even when flat terrain selects mode 7 for the floor.
        for triangle in self.triangles.iter_mut().filter(|triangle| !triangle.ceiling) {
            for vertex in &mut triangle.vertices {
                vertex.pnt5 = i32::from(triangle.flat_shade) << 16;
            }
        }
        let mut input = self.input_hash.to_le_bytes().to_vec();
        input.extend_from_slice(b"source-flat-terrain-mode7");
        self.input_hash = byte_hash(&input);
        self.command_hash = hash_commands(&self.triangles);
    }
    #[cfg(test)]
    pub(crate) fn without_ceiling_submissions_for_test(&self) -> Self {
        let mut plan = self.clone();
        plan.triangles.retain(|triangle| !triangle.ceiling);
        plan.command_hash = hash_commands(&plan.triangles);
        plan
    }

    #[cfg(test)]
    pub(crate) fn without_floor_submissions_for_test(&self) -> Self {
        let mut plan = self.clone();
        plan.triangles.retain(|triangle| triangle.ceiling);
        plan.command_hash = hash_commands(&plan.triangles);
        plan
    }

    pub fn initial_indices(&self, profile: &ClassicHdRenderProfile) -> Vec<u8> {
        vec![self.clear_index; profile.extent()]
    }

    /// Source sprite-bucket order, indexed by wrapped map tile y*256+x.
    /// Zero means the source did not submit that bucket in this view.
    pub fn cell_orders(&self) -> &[u16] {
        &self.cell_orders
    }

    pub fn cell_order(&self, tile_x: u8, tile_y: u8) -> u16 {
        self.cell_orders[usize::from(tile_y) * 256 + usize::from(tile_x)]
    }

    /// Material-only compatibility path, retaining the existing frame bytes.
    pub fn rasterize(
        &self,
        writer: &mut ClassicHdFrameWriter<'_>,
        atlas: &[u8],
        tables: &[u8],
        provenance: u8,
    ) -> Result<CaveRasterReceipt, ClassicHdRasterError> {
        self.rasterize_inner(writer, atlas, tables, provenance, None, None)
    }

    /// Stamp the source cell's order at each pixel actually written by its
    /// exact mode5 spans. Later terrain clips earlier sprites at fragment
    /// granularity; a sprite follows its own cell's ceiling/floor submissions.
    pub fn rasterize_with_occlusion(
        &self,
        writer: &mut ClassicHdFrameWriter<'_>,
        atlas: &[u8],
        tables: &[u8],
        provenance: u8,
        orders: &mut [u16],
    ) -> Result<CaveRasterReceipt, ClassicHdRasterError> {
        if orders.len() != writer.profile().extent() {
            return Err(ClassicHdRasterError(
                "Cave occlusion extent differs from frame".into(),
            ));
        }
        orders.fill(0);
        self.rasterize_inner(writer, atlas, tables, provenance, Some(orders), None)
    }

    /// Source cell order is ceiling, floor, then that cell's shadow/primary
    /// billboard chain. Clipped materials do not discard a live bucket.
    #[allow(clippy::type_complexity)]
    pub fn rasterize_composed(
        &self,
        writer: &mut ClassicHdFrameWriter<'_>,
        atlas: &[u8],
        tables: &[u8],
        provenance: u8,
        orders: &mut [u16],
        after_cell: &mut dyn FnMut(
            u16,
            u8,
            &mut ClassicHdFrameWriter<'_>,
        ) -> Result<(), ClassicHdRasterError>,
    ) -> Result<CaveRasterReceipt, ClassicHdRasterError> {
        if orders.len() != writer.profile().extent() {
            return Err(ClassicHdRasterError(
                "Cave composed source extent differs".into(),
            ));
        }
        orders.fill(0);
        self.rasterize_inner(
            writer,
            atlas,
            tables,
            provenance,
            Some(orders),
            Some(after_cell),
        )
    }

    #[allow(clippy::type_complexity)]
    fn rasterize_inner(
        &self,
        writer: &mut ClassicHdFrameWriter<'_>,
        atlas: &[u8],
        tables: &[u8],
        provenance: u8,
        mut orders: Option<&mut [u16]>,
        mut after_cell: Option<
            &mut dyn FnMut(
                u16,
                u8,
                &mut ClassicHdFrameWriter<'_>,
            ) -> Result<(), ClassicHdRasterError>,
        >,
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
        let mut completed_cell = 0_u16;
        for triangle in &self.triangles {
            while after_cell.is_some() && completed_cell + 1 < triangle.cell_order {
                completed_cell += 1;
                if let Some(consumer) = after_cell.as_mut() {
                    consumer(
                        completed_cell,
                        self.cell_textures[usize::from(completed_cell)],
                        writer,
                    )?;
                }
            }
            let provenance = provenance | if triangle.ceiling { 4 } else { 2 };
            let written_pixels = if let Some(orders) = orders.as_deref_mut() {
                let width = writer.width();
                let base = terrain_texture_base(triangle.texture)?;
                let pixels = if self.flat_shading && !triangle.ceiling {
                    rasterize_mode7_triangle_from_tile_atlas(triangle.vertices, width as i32,
                        writer.height() as i32, atlas, base, &tables[..CLASSIC_HD_TABLESN_BYTES], triangle.flat_shade)
                } else {
                    rasterize_mode5_triangle_from_tile_atlas(triangle.vertices, width as i32,
                        writer.height() as i32, atlas, base, &tables[..CLASSIC_HD_TABLESN_BYTES])
                };
                let mut written = 0;
                for pixel in pixels {
                    if writer.set_pixel(
                        pixel.sample.x,
                        pixel.y,
                        pixel.sample.output_index,
                        provenance,
                    ) {
                        orders[pixel.y as usize * width as usize + pixel.sample.x as usize] =
                            triangle.cell_order;
                        written += 1;
                    }
                }
                written
            } else if self.flat_shading && !triangle.ceiling {
                rasterize_classic_hd_mode7_triangle(writer, triangle.vertices, atlas,
                    terrain_texture_base(triangle.texture)?, &tables[..CLASSIC_HD_TABLESN_BYTES],
                    provenance, triangle.flat_shade)?.written_pixels
            } else {
                rasterize_classic_hd_mode5_triangle(
                    writer,
                    triangle.vertices,
                    atlas,
                    terrain_texture_base(triangle.texture)?,
                    &tables[..CLASSIC_HD_TABLESN_BYTES],
                    provenance,
                )?
                .written_pixels
            };
            if triangle.ceiling {
                receipt.ceiling_pixels_written += written_pixels;
            } else {
                receipt.floor_pixels_written += written_pixels;
            }
            // A source cell can have ceiling and floor triangles. Its
            // bucket follows all of them, never the first floor span.
            // The loop below drains it when the following cell begins.
        }
        if let Some(consumer) = after_cell.as_mut() {
            for cell in completed_cell + 1..self.cell_textures.len() as u16 {
                consumer(cell, self.cell_textures[usize::from(cell)], writer)?;
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
    build_cave_terrain_projected(
        profile,
        camera,
        resources,
        ClassicHdProjection::from_profile(profile, camera),
    )
}

pub(crate) fn build_cave_terrain_projected(
    profile: &ClassicHdRenderProfile,
    camera: Level1TerrainFrameCamera,
    resources: CaveTerrainResources<'_>,
    projection: ClassicHdProjection,
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
    let mut cell_orders = vec![0; 65_536];
    let mut cell_order = 0_u16;
    let mut cell_textures = vec![0_u8];
    for row in (1..OUTDOOR_GRID_ROWS).rev() {
        let start = row * OUTDOOR_GRID_COLUMNS;
        let flags = std::array::from_fn(|column| grid[start + column].feature_flags);
        for owner in terrain_row_anchor_order(start, &flags) {
            cell_order += 1;
            cell_textures.push(vertices[owner].map_or(0, |vertex| vertex.texture));
            let indices = [owner, owner + 1, owner - 39, owner - 40];
            // haveBillboard is set from the owner's accepted lattice point,
            // independently of an adjacent vertex rejecting a material quad.
            let sealed = indices
                .iter()
                .all(|&index| vertices[index].is_some_and(|vertex| vertex.flags & SEALED != 0));
            if !sealed && vertices[owner].is_some() {
                let (x, y) = sprite_cell(grid[owner].tile_x, grid[owner].tile_y, camera.yaw);
                cell_orders[usize::from(y) * 256 + usize::from(x)] = cell_order;
            }
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
            append_triangles(&mut triangles, &cell, uv, ceiling, 1, true, cell_order);
            if all & OUTSIDE == 0 {
                let floor = if parity {
                    [[0, 1, 3], [3, 1, 2]]
                } else {
                    [[0, 1, 2], [0, 2, 3]]
                };
                append_triangles(
                    &mut triangles,
                    &cell,
                    uv,
                    floor,
                    cell[0].texture,
                    false,
                    cell_order,
                );
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
    if projection != ClassicHdProjection::from_profile(profile, camera) {
        for value in [
            projection.scale,
            projection.pitch_offset,
            projection.center_x,
            projection.center_y,
            projection.viewport_x,
            projection.viewport_y,
            projection.viewport_width,
            projection.viewport_height,
        ] {
            input.extend_from_slice(&value.to_le_bytes());
        }
    }
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
        flat_shading: false,
        triangles,
        cell_orders,
        cell_textures,
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
    cell_order: u16,
) {
    for corners in pairs {
        out.push(Triangle {
            texture,
            ceiling,
            cell_order,
            flat_shade: source_flat_terrain_shade(std::array::from_fn(|index| cell[index].floor.pnt5)),
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

/// GameRenderOriginal.h unk_D4328x: texture offset[2,3], followed by
/// billboard offset[4,5]. The sprite bucket is NOT the lattice-point tile.
fn sprite_cell(x: u8, y: u8, yaw: i16) -> (u8, u8) {
    let quadrant = ((i32::from(yaw) & 2047) + 256) >> 9 & 3;
    match quadrant {
        0 => (x, y.wrapping_sub(1)),
        1 => (x, y),
        2 => (x.wrapping_sub(1), y),
        3 => (x.wrapping_sub(1), y.wrapping_sub(1)),
        _ => unreachable!("quadrant is masked"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_floor_preserves_interpolated_ceiling_in_both_raster_paths() {
        let profile = ClassicHdRenderProfile::compatibility_320x200();
        let triangle = |x, ceiling| Triangle {
            vertices: [
                Mode5Vertex { x, y: 2, u: 0, v: 0, pnt5: 4 << 16 },
                Mode5Vertex { x: x + 40, y: 2, u: 0, v: 0, pnt5: 20 << 16 },
                Mode5Vertex { x, y: 42, u: 0, v: 0, pnt5: 12 << 16 },
            ],
            texture: if ceiling { 1 } else { 2 },
            ceiling,
            cell_order: if ceiling { 1 } else { 2 },
            flat_shade: 12,
        };
        let smooth = CaveTerrainPlan {
            profile_revision: profile.profile_revision,
            clear_index: 254,
            flat_shading: false,
            triangles: vec![triangle(2, true), triangle(64, false)],
            cell_orders: vec![0; 65_536],
            cell_textures: vec![0, 1, 2],
            input_hash: 7,
            command_hash: 9,
        };
        let mut flat = smooth.clone();
        flat.apply_flat_shading(true);
        let atlas = vec![17; CLASSIC_HD_TERRAIN_ATLAS_BYTES];
        let mut tables = vec![0; 83_456];
        for (shade, row) in tables[..CLASSIC_HD_TABLESN_BYTES].chunks_exact_mut(256).enumerate() {
            row.fill(shade as u8);
        }
        let render = |plan: &CaveTerrainPlan, composed: bool| {
            let mut indices = plan.initial_indices(&profile);
            let mut provenance = vec![0; profile.extent()];
            let mut orders = vec![0; profile.extent()];
            let mut writer = ClassicHdFrameWriter::from_planes(
                &profile, &mut indices, &mut provenance, 1, 0, plan.input_hash, 0,
            ).unwrap();
            let receipt = if composed {
                plan.rasterize_with_occlusion(&mut writer, &atlas, &tables, 1, &mut orders).unwrap()
            } else {
                plan.rasterize(&mut writer, &atlas, &tables, 1).unwrap()
            };
            let _ = writer.finish();
            assert!(receipt.ceiling_pixels_written > 0 && receipt.floor_pixels_written > 0);
            indices
        };
        for composed in [false, true] {
            let before = render(&smooth, composed);
            let after = render(&flat, composed);
            let roof = |pixels: &[u8]| (0..44).flat_map(|y|
                pixels[y * 320..y * 320 + 48].iter().copied()).collect::<Vec<_>>();
            let floor = |pixels: &[u8]| (0..44).flat_map(|y|
                pixels[y * 320 + 64..y * 320 + 112].iter().copied()).collect::<Vec<_>>();
            assert_eq!(roof(&before), roof(&after), "ceiling keeps mode5 shades");
            assert_ne!(floor(&before), floor(&after), "floor switches to mode7");
        }
    }

    #[test]
    fn cave_occlusion_uses_exact_spans_and_preserves_partial_sprite_visibility() {
        let profile = ClassicHdRenderProfile::compatibility_320x200();
        let point = |x, y| Mode5Vertex {
            x,
            y,
            u: 0,
            v: 0,
            pnt5: 3 << 16,
        };
        let plan = CaveTerrainPlan {
            profile_revision: profile.profile_revision,
            clear_index: 254,
            flat_shading: false,
            cell_textures: vec![0, 0, 0],
            triangles: vec![
                Triangle {
                    vertices: [point(0, 0), point(20, 0), point(0, 20)],
                    texture: 0,
                    ceiling: true,
                    cell_order: 1,
                    flat_shade: 3,
                },
                Triangle {
                    vertices: [point(8, 0), point(20, 0), point(8, 12)],
                    texture: 0,
                    ceiling: false,
                    cell_order: 2,
                    flat_shade: 3,
                },
            ],
            cell_orders: vec![0; 65_536],
            input_hash: 7,
            command_hash: 9,
        };
        let atlas = vec![17; CLASSIC_HD_TERRAIN_ATLAS_BYTES];
        let mut tables = vec![0; 83_456];
        for row in tables[..CLASSIC_HD_TABLESN_BYTES].chunks_exact_mut(256) {
            for (index, value) in row.iter_mut().enumerate() {
                *value = index as u8;
            }
        }
        let render = |occlusion| {
            let mut indices = plan.initial_indices(&profile);
            let mut provenance = vec![0; profile.extent()];
            let mut orders = vec![0; profile.extent()];
            let mut writer = ClassicHdFrameWriter::from_planes(
                &profile,
                &mut indices,
                &mut provenance,
                1,
                0,
                plan.input_hash,
                0,
            )
            .unwrap();
            if occlusion {
                plan.rasterize_with_occlusion(&mut writer, &atlas, &tables, 1, &mut orders)
                    .unwrap();
            } else {
                plan.rasterize(&mut writer, &atlas, &tables, 1).unwrap();
            }
            writer.finish();
            (indices, provenance, orders)
        };
        let plain = render(false);
        let ordered = render(true);
        assert_eq!(ordered.0, plain.0, "source indexed samples unchanged");
        assert_eq!(
            ordered.1, plain.1,
            "source floor/ceiling coverage unchanged"
        );
        assert_eq!(ordered.2[320 + 1], 1);
        assert_eq!(ordered.2[320 + 9], 2);
        assert_eq!(ordered.2[31 * 320 + 31], 0);
        // The shader keeps equal/earlier terrain and clips later terrain.
        // One source bucket therefore has both visible and hidden pixels.
        assert!(1 >= ordered.2[320 + 1]);
        assert!(1 < ordered.2[320 + 9]);
    }

    #[test]
    fn cave_sprite_bucket_uses_source_quadrant_offsets_and_wrapping() {
        assert_eq!(sprite_cell(0, 0, 0), (0, 255));
        assert_eq!(sprite_cell(0, 0, 512), (0, 0));
        assert_eq!(sprite_cell(0, 0, 1024), (255, 0));
        assert_eq!(sprite_cell(0, 0, 1536), (255, 255));
    }
}
