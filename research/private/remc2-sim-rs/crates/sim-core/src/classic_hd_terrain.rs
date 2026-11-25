//! Resolution-aware Classic HD sky, camera, and outdoor-terrain planning.
//!
//! This adapter deliberately does not replace `terrain_frame`.  The latter
//! remains the sealed 320x200 Exact path.  Instead, this module consumes the
//! same typed camera and owner planes and expands the source lattice into a
//! profile-sized, indexed presentation plan.  The raster/postprocess stages
//! consume the returned sky, projected vertices, and painter submissions.

use crate::{
    SIN_16_16,
    classic_hd::{ClassicHdError, ClassicHdRenderProfile, Q16_ONE, byte_hash},
    render_camera::terrain_projection_divisor,
    terrain_frame::{Level1TerrainFrameCamera, level1_projection},
    terrain_grid::{
        TERRAIN_FEATURE_CHECKERBOARD, TERRAIN_FEATURE_CULLED, TERRAIN_FEATURE_SEAM_SIDE,
    },
    terrain_presentation::{
        TERRAIN_FOG_END_SQUARED, TERRAIN_FOG_START_SQUARED, TERRAIN_RENDER_CUTOFF_SQUARED,
        outdoor_terrain_pnt5,
    },
    terrain_reflection::{
        MODE5_TERRAIN_SELECTOR, MODE7_FLAT_TERRAIN_SELECTOR, MODE26_REFLECTIVE_TERRAIN_SELECTOR,
        PRIMARY_OUTSIDE_MASK, REFLECTION_EYE_Z_LIMIT, TERRAIN_FEATURE_FLAT_SHADE,
        TERRAIN_FEATURE_PRIMARY_ABOVE, TERRAIN_FEATURE_PRIMARY_BELOW, TERRAIN_FEATURE_PRIMARY_LEFT,
        TERRAIN_FEATURE_PRIMARY_RIGHT, TERRAIN_FEATURE_REFLECTED_ABOVE,
        TERRAIN_FEATURE_REFLECTED_BELOW, TERRAIN_FEATURE_REFLECTED_LEFT,
        TERRAIN_FEATURE_REFLECTED_RIGHT, TERRAIN_FEATURE_REFLECTIVE_MATERIAL, TerrainPass,
        reflected_inverse_altitude, reflection_texture_cell, terrain_material_is_reflective,
    },
};

const BASE_GRID_COLUMNS: usize = 40;
const BASE_GRID_ROWS: usize = 21;
const MAX_CLASSIC_HD_GRID_VERTICES: usize = 160 * 84;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassicHdSkyBank {
    Base256,
    HighResolution1024,
}

impl ClassicHdSkyBank {
    #[must_use]
    pub const fn side(self) -> usize {
        match self {
            Self::Base256 => 256,
            Self::HighResolution1024 => 1_024,
        }
    }

    #[must_use]
    pub const fn required_bytes(self) -> usize {
        self.side() * self.side()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdProjection {
    pub profile_revision: u64,
    pub scale: i32,
    pub pitch_offset: i32,
    pub roll: i16,
    pub center_x: i32,
    pub center_y: i32,
    pub viewport_x: i32,
    pub viewport_y: i32,
    pub viewport_width: i32,
    pub viewport_height: i32,
    pub view_distance_scale_q16: u32,
    pub render_cutoff_squared: u32,
    pub fog_start_squared: u32,
    pub fog_end_squared: u32,
}

impl ClassicHdProjection {
    /// Exact source DrawWorld rectangle within the shared indexed target.
    #[must_use]
    pub(crate) fn source_viewport(
        profile: &ClassicHdRenderProfile,
        camera: Level1TerrainFrameCamera,
        viewport: crate::classic_hd::ClassicHdViewport,
    ) -> Self {
        let geometry = crate::terrain_frame::source_viewport_projection(camera, viewport);
        let mut projection = Self::from_profile(profile, camera);
        projection.scale = geometry.scale;
        projection.pitch_offset = geometry.pitch_offset;
        projection.center_x = geometry.center_x;
        projection.center_y = geometry.center_y;
        projection.viewport_x = viewport.x as i32;
        projection.viewport_y = viewport.y as i32;
        projection.viewport_width = viewport.width as i32;
        projection.viewport_height = viewport.height as i32;
        projection
    }

    #[must_use]
    pub fn from_profile(
        profile: &ClassicHdRenderProfile,
        camera: Level1TerrainFrameCamera,
    ) -> Self {
        let exact = level1_projection(camera);
        let scale = if profile.is_compatibility_profile() {
            exact.scale
        } else {
            scale_i32(exact.scale, profile.projection_scale_q16)
        };
        let viewport = profile.viewport;
        let width = i32::try_from(viewport.width).expect("Classic HD width fits i32");
        let height = i32::try_from(viewport.height).expect("Classic HD height fits i32");
        let viewport_x = i32::try_from(viewport.x).expect("Classic HD viewport x fits i32");
        let viewport_y = i32::try_from(viewport.y).expect("Classic HD viewport y fits i32");
        let scale_squared = q16_square(profile.view_distance_scale_q16);
        Self {
            profile_revision: profile.profile_revision,
            scale,
            pitch_offset: i32::from(camera.pitch).wrapping_mul(width) >> 8,
            roll: camera.roll,
            center_x: viewport_x + width / 2,
            center_y: viewport_y + height / 2,
            viewport_x,
            viewport_y,
            viewport_width: width,
            viewport_height: height,
            view_distance_scale_q16: profile.view_distance_scale_q16,
            render_cutoff_squared: scale_u32(TERRAIN_RENDER_CUTOFF_SQUARED, scale_squared),
            fog_start_squared: scale_u32(TERRAIN_FOG_START_SQUARED, scale_squared),
            fog_end_squared: scale_u32(TERRAIN_FOG_END_SQUARED, scale_squared),
        }
    }

    #[must_use]
    pub const fn viewport_right(self) -> i32 {
        self.viewport_x + self.viewport_width
    }

    #[must_use]
    pub const fn viewport_bottom(self) -> i32 {
        self.viewport_y + self.viewport_height
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdTerrainResources<'a> {
    pub terrain_types: &'a [u8],
    pub ground_heights: &'a [u8],
    pub shading: &'a [u8],
    pub angles: &'a [u8],
    pub sky: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdTerrainVertex {
    pub tile_x: u8,
    pub tile_y: u8,
    pub row: u16,
    pub column: u16,
    pub view_x: i32,
    pub view_depth: i32,
    pub distance_squared: u32,
    pub feature_flags: u16,
    pub shade: u8,
    pub angle: u8,
    pub raw_wave: i32,
    pub surface_displacement: i32,
    pub lighting_wave: i32,
    pub pnt5: Option<i32>,
    pub tablesn_row: Option<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassicHdTerrainLattice {
    pub columns: usize,
    pub rows: usize,
    pub vertices: Vec<ClassicHdTerrainVertex>,
    pub command_hash: u64,
}

impl ClassicHdTerrainLattice {
    #[must_use]
    pub fn vertex(&self, row: usize, column: usize) -> Option<&ClassicHdTerrainVertex> {
        (row < self.rows && column < self.columns)
            .then(|| &self.vertices[row * self.columns + column])
    }

    #[must_use]
    pub const fn vertex_count(&self) -> usize {
        self.vertices.len()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClassicHdScreenPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdProjectedVertex {
    pub primary_altitude: i32,
    pub inverse_altitude: i32,
    pub primary_screen: ClassicHdScreenPoint,
    pub reflected_screen: ClassicHdScreenPoint,
    pub feature_flags: u16,
    pub texture_index: u8,
    pub texture_uv_row: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdTerrainTriangle {
    pub pass: TerrainPass,
    pub owner_index: usize,
    pub vertex_indices: [usize; 3],
    pub texture_index: u8,
    pub texture_uv_row: u8,
    pub raster_selector: u8,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ClassicHdTerrainSubmissionPlan {
    pub reflected: Vec<ClassicHdTerrainTriangle>,
    pub primary: Vec<ClassicHdTerrainTriangle>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassicHdTerrainRender {
    pub profile_revision: u64,
    pub projection: ClassicHdProjection,
    pub sky: Vec<u8>,
    pub lattice: ClassicHdTerrainLattice,
    pub projected: Vec<Option<ClassicHdProjectedVertex>>,
    pub submissions: ClassicHdTerrainSubmissionPlan,
    pub input_hash: u64,
    pub command_hash: u64,
}

impl ClassicHdTerrainRender {
    /// Apply source GAME flat_2199 before rasterization. Reflected terrain
    /// remains mode5; reflective primary material keeps mode26 precedence.
    /// An off setting leaves every existing command/hash byte untouched.
    pub(crate) fn apply_flat_shading(&mut self, enabled: bool) {
        if !enabled { return; }
        for vertex in self.projected.iter_mut().flatten() {
            vertex.feature_flags |= TERRAIN_FEATURE_FLAT_SHADE;
        }
        for triangle in &mut self.submissions.primary {
            if triangle.raster_selector == MODE5_TERRAIN_SELECTOR {
                triangle.raster_selector = MODE7_FLAT_TERRAIN_SELECTOR;
            }
        }
        let mut input = self.input_hash.to_le_bytes().to_vec();
        input.extend_from_slice(b"source-flat-terrain-mode7");
        self.input_hash = byte_hash(&input);
        self.command_hash = hash_submissions(&self.submissions);
    }

    /// Source painter ranks for terrain anchors and their billboard buckets.
    /// A bucket follows its own cell, even when that cell's material is clipped.
    pub fn source_painter_orders(&self, yaw: i16) -> (Vec<u16>, Vec<u16>) {
        let mut owners = vec![0; self.lattice.vertices.len()];
        let mut cells = vec![0; 256 * 256];
        let mut rank = 0_u16;
        let quadrant = ((i32::from(yaw) & 2047) + 256) >> 9 & 3;
        for row in (1..self.lattice.rows).rev() {
            for owner in row_anchor_order(&self.lattice, row) {
                rank += 1;
                owners[owner] = rank;
                if self.projected[owner].is_none() {
                    continue;
                }
                let point = self.lattice.vertices[owner];
                // GameRenderOriginal.h unk_D4328x texture offset [2,3]
                // followed by billboard offset [4,5], after quadrant rotation.
                let x = point.tile_x.wrapping_sub(u8::from(quadrant >= 2));
                let y = point
                    .tile_y
                    .wrapping_sub(u8::from(quadrant == 0 || quadrant == 3));
                cells[usize::from(y) * 256 + usize::from(x)] = rank;
            }
        }
        (owners, cells)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassicHdTerrainError(pub String);

impl std::fmt::Display for ClassicHdTerrainError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ClassicHdTerrainError {}

impl From<ClassicHdError> for ClassicHdTerrainError {
    fn from(error: ClassicHdError) -> Self {
        Self(error.0)
    }
}

/// Build the resolution-aware sky, lattice, projection, and painter plan.
pub fn build_classic_hd_terrain(
    profile: &ClassicHdRenderProfile,
    camera: Level1TerrainFrameCamera,
    resources: ClassicHdTerrainResources<'_>,
    sky_bank: ClassicHdSkyBank,
) -> Result<ClassicHdTerrainRender, ClassicHdTerrainError> {
    validate_resources(profile, resources, sky_bank)?;
    let projection = ClassicHdProjection::from_profile(profile, camera);
    let sky = render_classic_hd_sky(profile, camera, resources.sky, sky_bank)?;
    build_classic_hd_terrain_projected(profile, camera, resources, sky_bank, projection, sky)
}

/// Reuse the source terrain owner for a real DrawWorld viewport, such as
/// the full map's narrow right-hand world view. Geometry is projected at that
/// viewport's own scale; the caller supplies its matching source sky plane.
pub(crate) fn build_classic_hd_terrain_projected(
    profile: &ClassicHdRenderProfile,
    camera: Level1TerrainFrameCamera,
    resources: ClassicHdTerrainResources<'_>,
    sky_bank: ClassicHdSkyBank,
    projection: ClassicHdProjection,
    sky: Vec<u8>,
) -> Result<ClassicHdTerrainRender, ClassicHdTerrainError> {
    validate_resources(profile, resources, sky_bank)?;
    if sky.len() != profile.extent() || projection.profile_revision != profile.profile_revision {
        return Err(ClassicHdTerrainError(
            "source world viewport differs from its frame owner".into(),
        ));
    }
    let lattice = build_lattice(profile, camera, resources.shading, resources.angles)?;
    let projected = lattice
        .vertices
        .iter()
        .map(|vertex| {
            let map_index = usize::from(vertex.tile_x) + usize::from(vertex.tile_y) * 256;
            let (texture_index, texture_uv_row) = reflection_texture_cell(
                vertex.tile_x,
                vertex.tile_y,
                camera.yaw,
                resources.terrain_types,
                resources.angles,
            );
            project_vertex(
                *vertex,
                resources.ground_heights[map_index],
                camera.eye_z,
                texture_index,
                texture_uv_row,
                projection,
            )
        })
        .collect::<Vec<_>>();
    let submissions = build_submission_plan(camera, &lattice, &projected);
    let mut input_hash = hash_inputs(profile, camera, resources, sky_bank);
    if projection != ClassicHdProjection::from_profile(profile, camera) {
        let mut input = input_hash.to_le_bytes().to_vec();
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
        input_hash = byte_hash(&input);
    }
    let command_hash = hash_submissions(&submissions);
    Ok(ClassicHdTerrainRender {
        profile_revision: profile.profile_revision,
        projection,
        sky,
        lattice,
        projected,
        submissions,
        input_hash,
        command_hash,
    })
}

/// Render an indexed sky into the profile-sized stride.  The base bank uses
/// the same source-shaped integer lookup as Exact; the optional 1024 bank
/// changes only the bank coordinate granularity, never the palette model.
pub fn render_classic_hd_sky(
    profile: &ClassicHdRenderProfile,
    camera: Level1TerrainFrameCamera,
    sky: &[u8],
    bank: ClassicHdSkyBank,
) -> Result<Vec<u8>, ClassicHdTerrainError> {
    if sky.len() != bank.required_bytes() {
        return Err(ClassicHdTerrainError(format!(
            "Classic HD {:?} sky requires {} bytes, got {}",
            bank,
            bank.required_bytes(),
            sky.len()
        )));
    }
    if profile.viewport.x != 0
        || profile.viewport.y != 0
        || profile.viewport.width != profile.width
        || profile.viewport.height != profile.height
    {
        return Err(ClassicHdTerrainError(
            "Classic HD sky requires a full profile viewport".to_owned(),
        ));
    }
    let projection = ClassicHdProjection::from_profile(profile, camera);
    let viewport_width = usize::try_from(projection.viewport_width)
        .map_err(|_| ClassicHdTerrainError("Classic HD sky width is negative".to_owned()))?;
    let viewport_height = usize::try_from(projection.viewport_height)
        .map_err(|_| ClassicHdTerrainError("Classic HD sky height is negative".to_owned()))?;
    let stride = usize::try_from(profile.stride).map_err(|_| {
        ClassicHdTerrainError("Classic HD sky stride does not fit usize".to_owned())
    })?;
    let cos = SIN_16_16[usize::from((512 + (i32::from(camera.roll) & 0x07ff)) as u16)];
    let sin = SIN_16_16[usize::from((i32::from(camera.roll) & 0x07ff) as u16)];
    let cos_step = cos.wrapping_shl(8) / i32::try_from(viewport_width).unwrap();
    let sin_step = sin.wrapping_shl(8) / i32::try_from(viewport_width).unwrap();
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
    let mut x_deltas = vec![0_u8; viewport_width];
    let mut y_deltas = vec![0_u8; viewport_width];
    let mut error_x = 0_i32;
    let mut error_y = 0_i32;
    let mut previous_x = 0_u8;
    let mut previous_y = 0_u8;
    for column in 0..viewport_width {
        let current_x = sky_byte(error_x);
        let current_y = sky_byte(error_y);
        x_deltas[column] = current_x.wrapping_sub(previous_x);
        y_deltas[column] = current_y.wrapping_sub(previous_y);
        previous_x = current_x;
        previous_y = current_y;
        error_x = error_x.wrapping_add(cos_step);
        error_y = error_y.wrapping_add(sin_step);
    }
    let mut frame = vec![0_u8; profile.extent()];
    for row in 0..viewport_height {
        let mut u = sky_byte(row_u);
        let mut v = sky_byte(row_v);
        for column in 0..viewport_width {
            let offset = row * stride + column;
            frame[offset] = sky[sky_index(v, u, bank)];
            u = u.wrapping_add(x_deltas[column]);
            v = v.wrapping_add(y_deltas[column]);
        }
        row_u = row_u.wrapping_sub(sin_step);
        row_v = row_v.wrapping_add(cos_step);
    }
    Ok(frame)
}

fn validate_resources(
    profile: &ClassicHdRenderProfile,
    resources: ClassicHdTerrainResources<'_>,
    sky_bank: ClassicHdSkyBank,
) -> Result<(), ClassicHdTerrainError> {
    let expected = 256 * 256;
    for (name, length) in [
        ("terrain_types", resources.terrain_types.len()),
        ("ground_heights", resources.ground_heights.len()),
        ("shading", resources.shading.len()),
        ("angles", resources.angles.len()),
    ] {
        if length != expected {
            return Err(ClassicHdTerrainError(format!(
                "Classic HD {name} plane requires {expected} bytes, got {length}"
            )));
        }
    }
    if resources.sky.len() != sky_bank.required_bytes() {
        return Err(ClassicHdTerrainError(format!(
            "Classic HD sky bank requires {} bytes, got {}",
            sky_bank.required_bytes(),
            resources.sky.len()
        )));
    }
    if profile.extent() == 0 {
        return Err(ClassicHdTerrainError(
            "Classic HD profile has an empty presentation extent".to_owned(),
        ));
    }
    Ok(())
}

fn build_lattice(
    profile: &ClassicHdRenderProfile,
    camera: Level1TerrainFrameCamera,
    shading: &[u8],
    angles: &[u8],
) -> Result<ClassicHdTerrainLattice, ClassicHdTerrainError> {
    let columns = scaled_grid_count(BASE_GRID_COLUMNS, profile.view_distance_scale_q16);
    let rows = scaled_grid_count(BASE_GRID_ROWS, profile.view_distance_scale_q16);
    let vertex_count = columns
        .checked_mul(rows)
        .ok_or_else(|| ClassicHdTerrainError("Classic HD terrain lattice overflow".to_owned()))?;
    if vertex_count > MAX_CLASSIC_HD_GRID_VERTICES {
        return Err(ClassicHdTerrainError(
            "Classic HD terrain lattice exceeds the safety bound".to_owned(),
        ));
    }
    let yaw = i32::from(camera.yaw) & 0x07ff;
    let quadrant = usize::try_from(((yaw + 256) >> 9) & 3).unwrap();
    let residual = (((yaw + 256) & 0x01ff) - 256) & 0x07ff;
    let sin = SIN_16_16[usize::try_from(residual).unwrap()];
    let cos = SIN_16_16[usize::try_from(512 + residual).unwrap()];
    let [x_low, x_high] = camera.render_x.to_le_bytes();
    let [y_low, y_high] = camera.render_y.to_le_bytes();
    let half_columns = columns / 2 - 1;
    let half_columns_i32 = i32::try_from(half_columns)
        .map_err(|_| ClassicHdTerrainError("Classic HD terrain columns overflow i32".to_owned()))?;
    let (a1_start, a2_start) = match quadrant {
        0 => (
            -i32::from(x_low) - half_columns_i32 * 256,
            i32::from(y_low) - 256,
        ),
        1 => (
            -i32::from(y_low) - half_columns_i32 * 256,
            -i32::from(x_low),
        ),
        2 => (i32::from(x_low) - half_columns_i32 * 256, -i32::from(y_low)),
        3 => (
            i32::from(y_low) - half_columns_i32 * 256,
            i32::from(x_low) - 256,
        ),
        _ => unreachable!("quadrant is masked"),
    };
    let distance_scale_squared = q16_square(profile.view_distance_scale_q16);
    let mut vertices = Vec::with_capacity(vertex_count);
    for row in 0..rows {
        let a2 = a2_start + i32::try_from(row * 256).unwrap();
        for column in 0..columns {
            let a1 = a1_start + i32::try_from(column * 256).unwrap();
            let view_x = (a1.wrapping_mul(cos) >> 16) - (a2.wrapping_mul(sin) >> 16);
            let view_depth = (a1.wrapping_mul(sin) >> 16) + (a2.wrapping_mul(cos) >> 16);
            let distance_squared = view_x
                .wrapping_mul(view_x)
                .wrapping_add(view_depth.wrapping_mul(view_depth))
                .cast_unsigned();
            let (tile_x, tile_y) = terrain_tile_for(quadrant, x_high, y_high, row, column, columns);
            let map_index = usize::from(tile_x) + usize::from(tile_y) * 256;
            let shade = shading[map_index];
            let angle = angles[map_index];
            let source_distance = normalize_distance(distance_squared, distance_scale_squared);
            let value = outdoor_terrain_pnt5(
                camera.source_turn,
                i32::from(tile_x),
                i32::from(tile_y),
                shade,
                angle,
                view_depth,
                source_distance,
            );
            let eligible = value.visible
                && terrain_projection_divisor(
                    view_depth,
                    distance_squared,
                    scale_u32(TERRAIN_RENDER_CUTOFF_SQUARED, distance_scale_squared),
                )
                .is_some();
            let mut feature_flags = if a1 >= 0 {
                TERRAIN_FEATURE_SEAM_SIDE
            } else {
                0
            };
            feature_flags |= u16::from(tile_x.wrapping_add(tile_y) & 1);
            if !eligible {
                feature_flags |= TERRAIN_FEATURE_CULLED;
            }
            vertices.push(ClassicHdTerrainVertex {
                tile_x,
                tile_y,
                row: u16::try_from(row).unwrap(),
                column: u16::try_from(column).unwrap(),
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
                tablesn_row: eligible.then_some(value.tablesn_row),
            });
        }
    }
    let command_hash = hash_lattice(profile.profile_revision, columns, rows, &vertices);
    Ok(ClassicHdTerrainLattice {
        columns,
        rows,
        vertices,
        command_hash,
    })
}

fn project_vertex(
    lattice: ClassicHdTerrainVertex,
    ground_height: u8,
    eye_z: i32,
    texture_index: u8,
    texture_uv_row: u8,
    projection: ClassicHdProjection,
) -> Option<ClassicHdProjectedVertex> {
    if lattice.feature_flags & TERRAIN_FEATURE_CULLED != 0 {
        return None;
    }
    let divisor = terrain_projection_divisor(
        lattice.view_depth,
        lattice.distance_squared,
        projection.render_cutoff_squared,
    )?;
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
    let sin = SIN_16_16[usize::try_from(roll).unwrap()];
    let cos = SIN_16_16[usize::try_from(512 + roll).unwrap()];
    let rotate = |vertical: i32| {
        let x = cos
            .wrapping_mul(projected_x)
            .wrapping_sub(sin.wrapping_mul(vertical))
            >> 16;
        let y = sin
            .wrapping_mul(projected_x)
            .wrapping_add(cos.wrapping_mul(vertical))
            >> 16;
        ClassicHdScreenPoint {
            x: x.wrapping_add(projection.center_x),
            y: projection.center_y.wrapping_sub(y),
        }
    };
    let primary_screen = rotate(projected_primary_y);
    let reflected_screen = rotate(projected_reflected_y);
    let mut feature_flags = lattice.feature_flags;
    add_outside_flags(
        primary_screen,
        projection,
        [
            TERRAIN_FEATURE_PRIMARY_LEFT,
            TERRAIN_FEATURE_PRIMARY_RIGHT,
            TERRAIN_FEATURE_PRIMARY_ABOVE,
            TERRAIN_FEATURE_PRIMARY_BELOW,
        ],
        &mut feature_flags,
    );
    add_outside_flags(
        reflected_screen,
        projection,
        [
            TERRAIN_FEATURE_REFLECTED_LEFT,
            TERRAIN_FEATURE_REFLECTED_RIGHT,
            TERRAIN_FEATURE_REFLECTED_ABOVE,
            TERRAIN_FEATURE_REFLECTED_BELOW,
        ],
        &mut feature_flags,
    );
    if terrain_material_is_reflective(texture_index) {
        feature_flags |= TERRAIN_FEATURE_REFLECTIVE_MATERIAL;
    }
    Some(ClassicHdProjectedVertex {
        primary_altitude,
        inverse_altitude,
        primary_screen,
        reflected_screen,
        feature_flags,
        texture_index,
        texture_uv_row,
    })
}

fn row_anchor_order(lattice: &ClassicHdTerrainLattice, row: usize) -> Vec<usize> {
    let row_start = row * lattice.columns;
    let split = (0..lattice.columns.saturating_sub(1))
        .find(|column| {
            lattice.vertices[row_start + column + 1].feature_flags & TERRAIN_FEATURE_SEAM_SIDE != 0
        })
        .unwrap_or(lattice.columns.saturating_sub(1));
    (0..split)
        .chain((split..lattice.columns.saturating_sub(1)).rev())
        .map(|column| row_start + column)
        .collect()
}

fn build_submission_plan(
    camera: Level1TerrainFrameCamera,
    lattice: &ClassicHdTerrainLattice,
    projected: &[Option<ClassicHdProjectedVertex>],
) -> ClassicHdTerrainSubmissionPlan {
    let mut plan = ClassicHdTerrainSubmissionPlan::default();
    for row in (1..lattice.rows).rev() {
        for anchor in row_anchor_order(lattice, row) {
            let upper_left = anchor - lattice.columns;
            let upper_right = upper_left + 1;
            let indices = [anchor, anchor + 1, upper_right, upper_left];
            let Some(vertices) = indices
                .map(|index| projected[index])
                .into_iter()
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            let flags = vertices
                .iter()
                .fold(0_u16, |combined, vertex| combined | vertex.feature_flags);
            let all = vertices
                .iter()
                .fold(u16::MAX, |combined, vertex| combined & vertex.feature_flags);
            let lower_left = vertices[0];
            let parity_one = lower_left.feature_flags & TERRAIN_FEATURE_CHECKERBOARD != 0;
            if camera.reflections
                && camera.eye_z < REFLECTION_EYE_Z_LIMIT
                && lower_left.texture_index != 0
                && flags & TERRAIN_FEATURE_CULLED == 0
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
                plan.reflected
                    .extend(
                        pairs
                            .into_iter()
                            .map(|vertex_indices| ClassicHdTerrainTriangle {
                                pass: TerrainPass::Reflected,
                                owner_index: anchor,
                                vertex_indices,
                                texture_index: lower_left.texture_index,
                                texture_uv_row: lower_left.texture_uv_row,
                                raster_selector: MODE5_TERRAIN_SELECTOR,
                            }),
                    );
            }
            if flags & TERRAIN_FEATURE_CULLED == 0 && all & PRIMARY_OUTSIDE_MASK == 0 {
                let selector = if camera.reflections
                    && lower_left.feature_flags & TERRAIN_FEATURE_REFLECTIVE_MATERIAL != 0
                {
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
                plan.primary.extend(pairs.into_iter().map(|vertex_indices| {
                    ClassicHdTerrainTriangle {
                        pass: TerrainPass::Primary,
                        owner_index: anchor,
                        vertex_indices,
                        texture_index: lower_left.texture_index,
                        texture_uv_row: lower_left.texture_uv_row,
                        raster_selector: selector,
                    }
                }));
            }
        }
    }
    plan
}

fn terrain_tile_for(
    quadrant: usize,
    camera_x_high: u8,
    camera_y_high: u8,
    row: usize,
    column: usize,
    columns: usize,
) -> (u8, u8) {
    let half = columns / 2 - 1;
    let row = u8::try_from(row).expect("Classic HD terrain rows fit u8");
    let column = u8::try_from(column).expect("Classic HD terrain columns fit u8");
    let half = u8::try_from(half).expect("Classic HD terrain half-width fits u8");
    match quadrant {
        0 => (
            camera_x_high.wrapping_sub(half).wrapping_add(column),
            camera_y_high.wrapping_add(1).wrapping_sub(row),
        ),
        1 => (
            camera_x_high.wrapping_add(row),
            camera_y_high.wrapping_sub(half).wrapping_add(column),
        ),
        2 => (
            camera_x_high.wrapping_add(half).wrapping_sub(column),
            camera_y_high.wrapping_add(row),
        ),
        3 => (
            camera_x_high.wrapping_add(1).wrapping_sub(row),
            camera_y_high.wrapping_add(half).wrapping_sub(column),
        ),
        _ => unreachable!("quadrant is masked"),
    }
}

fn add_outside_flags(
    point: ClassicHdScreenPoint,
    projection: ClassicHdProjection,
    flags: [u16; 4],
    target: &mut u16,
) {
    if point.x < projection.viewport_x {
        *target |= flags[0];
    } else if point.x >= projection.viewport_right() {
        *target |= flags[1];
    }
    if point.y < projection.viewport_y {
        *target |= flags[2];
    } else if point.y >= projection.viewport_bottom() {
        *target |= flags[3];
    }
}

fn scaled_grid_count(base: usize, scale_q16: u32) -> usize {
    usize::try_from(
        (u64::try_from(base).unwrap() * u64::from(scale_q16) + u64::from(Q16_ONE / 2))
            / u64::from(Q16_ONE),
    )
    .unwrap()
}

fn q16_square(scale_q16: u32) -> u64 {
    (u64::from(scale_q16) * u64::from(scale_q16)) / u64::from(Q16_ONE) / u64::from(Q16_ONE)
}

fn scale_i32(value: i32, scale_q16: u32) -> i32 {
    i32::try_from((i64::from(value) * i64::from(scale_q16)) / i64::from(Q16_ONE)).unwrap_or_else(
        |_| {
            if value.is_negative() {
                i32::MIN
            } else {
                i32::MAX
            }
        },
    )
}

fn scale_u32(value: u32, factor: u64) -> u32 {
    u32::try_from(u64::from(value).saturating_mul(factor)).unwrap_or(u32::MAX)
}

fn normalize_distance(value: u32, factor: u64) -> u32 {
    if factor == 0 {
        value
    } else {
        u32::try_from(u64::from(value) / factor).unwrap_or(u32::MAX)
    }
}

fn sky_byte(value: i32) -> u8 {
    // DrawSky_40950 reads BYTE2 of its 16.16 accumulators, not
    // the upper byte of the fractional component (BYTE1).
    (value >> 16) as u8
}

fn sky_index(v: u8, u: u8, bank: ClassicHdSkyBank) -> usize {
    match bank {
        ClassicHdSkyBank::Base256 => (usize::from(v) << 8) | usize::from(u),
        ClassicHdSkyBank::HighResolution1024 => {
            ((usize::from(v) << 2) << 10) | (usize::from(u) << 2)
        }
    }
}

fn hash_inputs(
    profile: &ClassicHdRenderProfile,
    camera: Level1TerrainFrameCamera,
    resources: ClassicHdTerrainResources<'_>,
    sky_bank: ClassicHdSkyBank,
) -> u64 {
    let mut bytes = Vec::with_capacity(128);
    bytes.extend_from_slice(&profile.profile_revision.to_le_bytes());
    bytes.extend_from_slice(&profile.width.to_le_bytes());
    bytes.extend_from_slice(&profile.height.to_le_bytes());
    bytes.extend_from_slice(&camera.render_x.to_le_bytes());
    bytes.extend_from_slice(&camera.render_y.to_le_bytes());
    bytes.extend_from_slice(&camera.eye_z.to_le_bytes());
    bytes.extend_from_slice(&camera.yaw.to_le_bytes());
    bytes.extend_from_slice(&camera.pitch.to_le_bytes());
    bytes.extend_from_slice(&camera.roll.to_le_bytes());
    bytes.extend_from_slice(&camera.fov.to_le_bytes());
    bytes.extend_from_slice(&camera.source_turn.to_le_bytes());
    bytes.push(u8::from(camera.reflections));
    bytes.push(match sky_bank {
        ClassicHdSkyBank::Base256 => 0,
        ClassicHdSkyBank::HighResolution1024 => 1,
    });
    for plane in [
        resources.terrain_types,
        resources.ground_heights,
        resources.shading,
        resources.angles,
        resources.sky,
    ] {
        bytes.extend_from_slice(&byte_hash(plane).to_le_bytes());
    }
    byte_hash(&bytes)
}

fn hash_lattice(
    profile_revision: u64,
    columns: usize,
    rows: usize,
    vertices: &[ClassicHdTerrainVertex],
) -> u64 {
    let mut bytes = Vec::with_capacity(vertices.len() * 16 + 24);
    bytes.extend_from_slice(&profile_revision.to_le_bytes());
    bytes.extend_from_slice(&(columns as u64).to_le_bytes());
    bytes.extend_from_slice(&(rows as u64).to_le_bytes());
    for vertex in vertices {
        bytes.extend_from_slice(&[vertex.tile_x, vertex.tile_y]);
        bytes.extend_from_slice(&vertex.row.to_le_bytes());
        bytes.extend_from_slice(&vertex.column.to_le_bytes());
        bytes.extend_from_slice(&vertex.view_x.to_le_bytes());
        bytes.extend_from_slice(&vertex.view_depth.to_le_bytes());
        bytes.extend_from_slice(&vertex.distance_squared.to_le_bytes());
        bytes.extend_from_slice(&vertex.feature_flags.to_le_bytes());
        bytes.extend_from_slice(&[vertex.shade, vertex.angle]);
        bytes.extend_from_slice(&vertex.raw_wave.to_le_bytes());
        bytes.extend_from_slice(&vertex.surface_displacement.to_le_bytes());
        bytes.extend_from_slice(&vertex.lighting_wave.to_le_bytes());
        bytes.extend_from_slice(&vertex.pnt5.unwrap_or(i32::MIN).to_le_bytes());
        bytes.push(vertex.tablesn_row.unwrap_or(u8::MAX));
    }
    byte_hash(&bytes)
}

fn hash_submissions(plan: &ClassicHdTerrainSubmissionPlan) -> u64 {
    let mut bytes = Vec::with_capacity((plan.reflected.len() + plan.primary.len()) * 32);
    for triangle in plan.reflected.iter().chain(plan.primary.iter()) {
        bytes.push(match triangle.pass {
            TerrainPass::Reflected => 0,
            TerrainPass::Primary => 1,
        });
        bytes.extend_from_slice(&(triangle.owner_index as u64).to_le_bytes());
        for index in triangle.vertex_indices {
            bytes.extend_from_slice(&(index as u64).to_le_bytes());
        }
        bytes.extend_from_slice(&[
            triangle.texture_index,
            triangle.texture_uv_row,
            triangle.raster_selector,
        ]);
    }
    byte_hash(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        terrain_frame::{LEVEL1_SKY_BYTES, draw_level1_sky, level1_projection},
        terrain_grid::{OutdoorGridInput, outdoor_grid},
    };

    fn camera() -> Level1TerrainFrameCamera {
        Level1TerrainFrameCamera {
            render_x: 0x1234,
            render_y: 0x56ab,
            eye_z: 1_024,
            yaw: 173,
            pitch: 29,
            roll: 11,
            fov: 256,
            source_turn: 7,
            reflections: true,
        }
    }

    fn resources<'a>(
        terrain_types: &'a [u8],
        ground_heights: &'a [u8],
        shading: &'a [u8],
        angles: &'a [u8],
        sky: &'a [u8],
    ) -> ClassicHdTerrainResources<'a> {
        ClassicHdTerrainResources {
            terrain_types,
            ground_heights,
            shading,
            angles,
            sky,
        }
    }

    #[test]
    fn compatibility_projection_and_sky_match_exact_path() {
        let profile = ClassicHdRenderProfile::compatibility_320x200();
        let camera = camera();
        let projection = ClassicHdProjection::from_profile(&profile, camera);
        let exact = level1_projection(camera);
        assert_eq!(projection.scale, exact.scale);
        assert_eq!(projection.pitch_offset, exact.pitch_offset);
        assert_eq!(projection.center_x, exact.center_x);
        assert_eq!(projection.center_y, exact.center_y);
        let sky = vec![0_u8; LEVEL1_SKY_BYTES];
        assert_eq!(
            render_classic_hd_sky(&profile, camera, &sky, ClassicHdSkyBank::Base256).unwrap(),
            draw_level1_sky(camera, &sky).unwrap()
        );
        let full = crate::terrain_frame::source_viewport(40).unwrap();
        assert_eq!(ClassicHdProjection::source_viewport(&profile, camera, full), projection);
        let patterned: Vec<u8> = (0..LEVEL1_SKY_BYTES)
            .map(|index| ((index >> 8) ^ index) as u8).collect();
        assert_eq!(
            crate::terrain_frame::draw_source_viewport_sky(camera, &patterned, full, 77).unwrap(),
            draw_level1_sky(camera, &patterned).unwrap(),
            "size 40 preserves the entire existing sky"
        );
        for (size, expected) in [
            (39, (4, 2, 312, 195)),
            (16, (96, 60, 128, 80)),
        ] {
            let view = crate::terrain_frame::source_viewport(size).unwrap();
            assert_eq!((view.x, view.y, view.width, view.height), expected);
            let projected = ClassicHdProjection::source_viewport(&profile, camera, view);
            assert_eq!(projected.center_x, (view.x + view.width / 2) as i32);
            assert_eq!(projected.center_y, (view.y + view.height / 2) as i32);
            assert_eq!(projected.pitch_offset, i32::from(camera.pitch) * view.width as i32 >> 8);
            assert_eq!(projected.scale, 7 * crate::legacy_sqrt(view.width * view.width + view.height * view.height) as i32 * i32::from(camera.fov) >> 11);
            assert_ne!(projected.scale, projection.scale, "a cropped full projection is not the source view");
            let rendered = crate::terrain_frame::draw_source_viewport_sky(camera, &patterned, view, 77).unwrap();
            for y in 0..200_u32 {
                for x in 0..320_u32 {
                    if x < view.x || x >= view.x + view.width || y < view.y || y >= view.y + view.height {
                        assert_eq!(rendered[(y * 320 + x) as usize], 77, "outside source sky viewport");
                    }
                }
            }
        }
        assert!(crate::terrain_frame::source_viewport(15).is_none());
        assert!(crate::terrain_frame::source_viewport(41).is_none());
    }

    #[test]
    fn patterned_sky_uses_source_integer_coordinate_byte() {
        assert_eq!(sky_byte(0x0012_3456), 0x12);
        assert_eq!(sky_byte(-0x0001_0000), 255);
        let profile = ClassicHdRenderProfile::compatibility_320x200();
        let sky: Vec<u8> = (0..LEVEL1_SKY_BYTES)
            .map(|index| ((index >> 8) ^ (index & 255)) as u8)
            .collect();
        for (yaw, pitch, roll) in [(0, 0, 0), (173, 29, 11), (2047, -83, 1999)] {
            let camera = Level1TerrainFrameCamera {
                yaw,
                pitch,
                roll,
                ..camera()
            };
            assert_eq!(
                render_classic_hd_sky(&profile, camera, &sky, ClassicHdSkyBank::Base256).unwrap(),
                draw_level1_sky(camera, &sky).unwrap(),
                "nonuniform pixels must match the source-qualified sky path"
            );
        }
    }

    #[test]
    fn compatibility_lattice_preserves_exact_grid_coordinates_and_coverage() {
        let profile = ClassicHdRenderProfile::compatibility_320x200();
        let camera = camera();
        let shading = vec![40; 256 * 256];
        let angles = vec![8; 256 * 256];
        let lattice = build_lattice(&profile, camera, &shading, &angles).unwrap();
        let exact = outdoor_grid(OutdoorGridInput {
            render_x: camera.render_x,
            render_y: camera.render_y,
            yaw: camera.yaw,
            turn: camera.source_turn,
            shading: &shading,
            angles: &angles,
        });
        assert_eq!((lattice.columns, lattice.rows), (40, 21));
        for (actual, expected) in lattice.vertices.iter().zip(exact) {
            assert_eq!(
                (actual.tile_x, actual.tile_y),
                (expected.tile_x, expected.tile_y)
            );
            assert_eq!(
                (actual.view_x, actual.view_depth),
                (expected.view_x, expected.view_depth)
            );
            assert_eq!(actual.distance_squared, expected.distance_squared);
            assert_eq!(actual.feature_flags, expected.feature_flags);
            assert_eq!(actual.pnt5, expected.pnt5);
            assert_eq!(actual.tablesn_row, expected.vertex_tablesn_row);
        }
    }

    #[test]
    fn hd_profile_expands_lattice_and_scales_projection_and_cutoffs() {
        let profile = ClassicHdRenderProfile::hd_640x480();
        let projection = ClassicHdProjection::from_profile(&profile, camera());
        let exact = level1_projection(camera());
        assert_eq!(projection.scale, exact.scale * 2);
        assert_eq!(projection.center_x, 320);
        assert_eq!(projection.center_y, 240);
        assert_eq!(
            projection.render_cutoff_squared,
            TERRAIN_RENDER_CUTOFF_SQUARED * 4
        );
        let shading = vec![40; 256 * 256];
        let angles = vec![0; 256 * 256];
        let lattice = build_lattice(&profile, camera(), &shading, &angles).unwrap();
        assert_eq!((lattice.columns, lattice.rows), (80, 42));
        assert_eq!(lattice.vertex_count(), 80 * 42);
        assert_ne!(lattice.command_hash, 0);
    }

    #[test]
    fn wrapped_map_edges_and_alternating_diagonals_are_stable() {
        let profile = ClassicHdRenderProfile::hd_640x480();
        let mut camera = camera();
        camera.render_x = 0;
        camera.render_y = 0;
        camera.yaw = 0;
        let shading = vec![40; 256 * 256];
        let angles = vec![0; 256 * 256];
        let terrain_types = vec![0; 256 * 256];
        let ground_heights = vec![32; 256 * 256];
        let sky = vec![0; 256 * 256];
        let first = build_classic_hd_terrain(
            &profile,
            camera,
            resources(&terrain_types, &ground_heights, &shading, &angles, &sky),
            ClassicHdSkyBank::Base256,
        )
        .unwrap();
        let second = build_classic_hd_terrain(
            &profile,
            camera,
            resources(&terrain_types, &ground_heights, &shading, &angles, &sky),
            ClassicHdSkyBank::Base256,
        )
        .unwrap();
        assert_eq!(first.input_hash, second.input_hash);
        assert_eq!(first.command_hash, second.command_hash);
        assert!(
            first
                .lattice
                .vertices
                .iter()
                .any(|vertex| vertex.tile_x == 255)
        );
        assert!(
            first
                .lattice
                .vertices
                .iter()
                .any(|vertex| vertex.tile_y == 255)
        );
        assert!(
            first
                .submissions
                .reflected
                .iter()
                .all(|triangle| triangle.pass == TerrainPass::Reflected)
        );
        assert!(
            first
                .submissions
                .primary
                .iter()
                .all(|triangle| triangle.pass == TerrainPass::Primary)
        );
    }

    #[test]
    fn high_resolution_sky_bank_is_deterministic_and_resources_fail_closed() {
        let profile = ClassicHdRenderProfile::hd_1280x960();
        let camera = camera();
        let sky = vec![17; ClassicHdSkyBank::HighResolution1024.required_bytes()];
        let first =
            render_classic_hd_sky(&profile, camera, &sky, ClassicHdSkyBank::HighResolution1024)
                .unwrap();
        let second =
            render_classic_hd_sky(&profile, camera, &sky, ClassicHdSkyBank::HighResolution1024)
                .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.len(), profile.extent());
        assert!(first.iter().all(|value| *value == 17));
        let bad = vec![0; 256 * 256];
        assert!(
            render_classic_hd_sky(&profile, camera, &bad, ClassicHdSkyBank::HighResolution1024)
                .is_err()
        );
        let bad_planes = vec![0; 7];
        let resources = resources(&bad_planes, &bad_planes, &bad_planes, &bad_planes, &bad);
        assert!(
            build_classic_hd_terrain(&profile, camera, resources, ClassicHdSkyBank::Base256)
                .is_err()
        );
    }
}
