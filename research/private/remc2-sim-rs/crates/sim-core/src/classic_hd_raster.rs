//! Resolution-independent Classic HD indexed raster and postprocess adapters.
//!
//! The Exact mode-5/mode-26 kernels remain unchanged and continue to own the
//! sealed 320x200 path.  This module supplies the missing presentation seam:
//! it validates a Classic HD profile, invokes those kernels at the active
//! stride, records provenance, and exposes deterministic indexed lighting,
//! spatial smoothing, and temporal-history helpers for the larger buffer.

use crate::{
    classic_hd::{ClassicHdError, ClassicHdFrameWriter, ClassicHdRenderProfile, byte_hash},
    classic_hd_terrain::{
        ClassicHdTerrainRender, ClassicHdTerrainSubmissionPlan, ClassicHdTerrainTriangle,
    },
    indexed_postprocess::{
        PRIOR_MAIN_PROVENANCE, SOURCE_WORLD_PROVENANCE, TABLESN_BLEND_OFFSET,
        TABLESN_REQUIRED_BYTES,
    },
    mode5_raster::{Mode5Vertex, rasterize_mode5_triangle_from_atlas},
    mode26_raster::{NIGHT_TABLES_BYTES, rasterize_mode26_triangle_from_atlas_into_framebuffer},
    terrain_reflection::TerrainPass,
};

pub const CLASSIC_HD_TABLESN_BYTES: usize = 256 * 256;
pub const CLASSIC_HD_TERRAIN_TILE_SIZE: usize = 32;
pub const CLASSIC_HD_TERRAIN_TILE_COLUMNS: usize = 8;
pub const CLASSIC_HD_TERRAIN_TILE_COUNT: usize = 152;
pub const CLASSIC_HD_TERRAIN_ATLAS_BYTES: usize =
    CLASSIC_HD_TERRAIN_TILE_COUNT * CLASSIC_HD_TERRAIN_TILE_SIZE * CLASSIC_HD_TERRAIN_TILE_SIZE;
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
pub struct ClassicHdHistoryKey {
    pub generation: u64,
    pub profile_revision: u64,
    pub tick: u64,
    pub input_hash: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdRasterReceipt {
    pub mode5_triangles: u32,
    pub mode26_triangles: u32,
    pub source_pixels: u64,
    pub written_pixels: u64,
    pub clipped_pixels: u64,
    pub output_hash: u64,
    pub provenance_hash: u64,
}

impl ClassicHdRasterReceipt {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            mode5_triangles: 0,
            mode26_triangles: 0,
            source_pixels: 0,
            written_pixels: 0,
            clipped_pixels: 0,
            output_hash: 0,
            provenance_hash: 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassicHdRasterError(pub String);

impl std::fmt::Display for ClassicHdRasterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ClassicHdRasterError {}

impl From<ClassicHdError> for ClassicHdRasterError {
    fn from(error: ClassicHdError) -> Self {
        Self(error.0)
    }
}

/// The source TABLESN address is always shade-row high byte, palette index
/// low byte.  Keeping this helper public makes maximum-address tests explicit
/// without exposing a raw table pointer to Unity.
#[must_use]
pub const fn palette_address(shade: u8, source_index: u8) -> usize {
    ((shade as usize) << 8) | source_index as usize
}

pub fn rasterize_classic_hd_mode5_triangle(
    writer: &mut ClassicHdFrameWriter<'_>,
    vertices: [Mode5Vertex; 3],
    texture_atlas: &[u8],
    texture_base: usize,
    tablesn: &[u8],
    provenance: u8,
) -> Result<ClassicHdRasterReceipt, ClassicHdRasterError> {
    validate_writer(writer)?;
    validate_texture(texture_atlas, texture_base)?;
    if tablesn.len() != CLASSIC_HD_TABLESN_BYTES {
        return Err(ClassicHdRasterError(format!(
            "Classic HD mode-5 TABLESN requires {CLASSIC_HD_TABLESN_BYTES} bytes, got {}",
            tablesn.len()
        )));
    }
    let width = i32::try_from(writer.width())
        .map_err(|_| ClassicHdRasterError("Classic HD mode-5 width does not fit i32".to_owned()))?;
    let height = i32::try_from(writer.height()).map_err(|_| {
        ClassicHdRasterError("Classic HD mode-5 height does not fit i32".to_owned())
    })?;
    let pixels = rasterize_mode5_triangle_from_atlas(
        vertices,
        width,
        height,
        texture_atlas,
        texture_base,
        tablesn,
    );
    let source_pixels = u64::try_from(pixels.len()).unwrap_or(u64::MAX);
    let mut written_pixels = 0_u64;
    let mut clipped_pixels = 0_u64;
    let mut hash_bytes = Vec::with_capacity(pixels.len() * 4);
    for pixel in pixels {
        hash_bytes.extend_from_slice(&pixel.y.to_le_bytes());
        hash_bytes.extend_from_slice(&pixel.sample.x.to_le_bytes());
        hash_bytes.push(pixel.sample.output_index);
        if writer.set_pixel(
            pixel.sample.x,
            pixel.y,
            pixel.sample.output_index,
            provenance,
        ) {
            written_pixels = written_pixels.wrapping_add(1);
        } else {
            clipped_pixels = clipped_pixels.wrapping_add(1);
        }
    }
    Ok(ClassicHdRasterReceipt {
        mode5_triangles: 1,
        mode26_triangles: 0,
        source_pixels,
        written_pixels,
        clipped_pixels,
        output_hash: byte_hash(&hash_bytes),
        provenance_hash: writer_provenance_hash(writer),
    })
}

pub fn rasterize_classic_hd_mode26_triangle(
    writer: &mut ClassicHdFrameWriter<'_>,
    vertices: [Mode5Vertex; 3],
    texture_atlas: &[u8],
    texture_base: usize,
    tablesn: &[u8],
    provenance: u8,
) -> Result<ClassicHdRasterReceipt, ClassicHdRasterError> {
    validate_writer(writer)?;
    validate_texture(texture_atlas, texture_base)?;
    if tablesn.len() != NIGHT_TABLES_BYTES {
        return Err(ClassicHdRasterError(format!(
            "Classic HD mode-26 TABLESN requires {NIGHT_TABLES_BYTES} bytes, got {}",
            tablesn.len()
        )));
    }
    let width = i32::try_from(writer.width()).map_err(|_| {
        ClassicHdRasterError("Classic HD mode-26 width does not fit i32".to_owned())
    })?;
    let height = i32::try_from(writer.height()).map_err(|_| {
        ClassicHdRasterError("Classic HD mode-26 height does not fit i32".to_owned())
    })?;
    let pixels = rasterize_mode26_triangle_from_atlas_into_framebuffer(
        vertices,
        width,
        height,
        texture_atlas,
        texture_base,
        tablesn,
        writer.indices_mut(),
    );
    let source_pixels = u64::try_from(pixels.len()).unwrap_or(u64::MAX);
    let mut written_pixels = 0_u64;
    let mut clipped_pixels = 0_u64;
    let mut hash_bytes = Vec::with_capacity(pixels.len() * 5);
    for pixel in pixels {
        hash_bytes.extend_from_slice(&pixel.y.to_le_bytes());
        hash_bytes.extend_from_slice(&pixel.sample.x.to_le_bytes());
        hash_bytes.push(pixel.sample.output_index);
        if writer.set_provenance(pixel.sample.x, pixel.y, provenance) {
            written_pixels = written_pixels.wrapping_add(1);
        } else {
            clipped_pixels = clipped_pixels.wrapping_add(1);
        }
    }
    Ok(ClassicHdRasterReceipt {
        mode5_triangles: 0,
        mode26_triangles: 1,
        source_pixels,
        written_pixels,
        clipped_pixels,
        output_hash: byte_hash(&hash_bytes),
        provenance_hash: writer_provenance_hash(writer),
    })
}

/// Rasterize the terrain adapter's complete reflection-then-primary plan.
/// All submissions are preflighted before the first write, so malformed
/// topology, profile changes, unsupported selectors, and undersized banks
/// fail without partially mutating the current frame.
pub fn rasterize_classic_hd_terrain(
    writer: &mut ClassicHdFrameWriter<'_>,
    terrain: &ClassicHdTerrainRender,
    terrain_atlas: &[u8],
    tablesn: &[u8],
    provenance: u8,
) -> Result<ClassicHdRasterReceipt, ClassicHdRasterError> {
    validate_writer(writer)?;
    if writer.profile().profile_revision != terrain.profile_revision {
        return Err(ClassicHdRasterError(
            "Classic HD terrain profile revision does not match frame writer".to_owned(),
        ));
    }
    validate_texture_bank(terrain_atlas)?;
    if tablesn.len() != NIGHT_TABLES_BYTES {
        return Err(ClassicHdRasterError(format!(
            "Classic HD terrain TABLESN requires {NIGHT_TABLES_BYTES} bytes, got {}",
            tablesn.len()
        )));
    }
    validate_submission_plan(&terrain.submissions, terrain)?;
    let mut receipt = ClassicHdRasterReceipt::empty();
    for triangle in terrain
        .submissions
        .reflected
        .iter()
        .chain(terrain.submissions.primary.iter())
    {
        let vertices = triangle_vertices(triangle, terrain)?;
        let texture_base = terrain_texture_base(triangle.texture_index)?;
        let stage = match triangle.raster_selector {
            5 => rasterize_classic_hd_mode5_triangle(
                writer,
                vertices,
                terrain_atlas,
                texture_base,
                &tablesn[..CLASSIC_HD_TABLESN_BYTES],
                provenance_for(triangle.pass, provenance),
            )?,
            26 if triangle.pass == TerrainPass::Primary => rasterize_classic_hd_mode26_triangle(
                writer,
                vertices,
                terrain_atlas,
                texture_base,
                tablesn,
                provenance_for(triangle.pass, provenance),
            )?,
            selector => {
                return Err(ClassicHdRasterError(format!(
                    "Classic HD terrain selector {selector} is unsupported; no approximation allowed"
                )));
            }
        };
        receipt.mode5_triangles = receipt.mode5_triangles.wrapping_add(stage.mode5_triangles);
        receipt.mode26_triangles = receipt
            .mode26_triangles
            .wrapping_add(stage.mode26_triangles);
        receipt.source_pixels = receipt.source_pixels.wrapping_add(stage.source_pixels);
        receipt.written_pixels = receipt.written_pixels.wrapping_add(stage.written_pixels);
        receipt.clipped_pixels = receipt.clipped_pixels.wrapping_add(stage.clipped_pixels);
    }
    receipt.output_hash = writer_output_hash(writer);
    receipt.provenance_hash = writer_provenance_hash(writer);
    Ok(receipt)
}

/// Apply the source indexed palette table to a profile-sized frame using a
/// 256x256 dynamic-light shading plane.  The map-to-frame lookup is nearest
/// and deterministic; it is presentation-only and never mutates the
/// authoritative `DynamicLightingState`.
pub fn apply_classic_hd_dynamic_lighting(
    profile: &ClassicHdRenderProfile,
    indices: &mut [u8],
    shading: &[u8],
    tablesn: &[u8],
) -> Result<u64, ClassicHdRasterError> {
    validate_plane(profile, indices, "indexed frame")?;
    if shading.len() != CLASSIC_HD_TABLESN_BYTES {
        return Err(ClassicHdRasterError(format!(
            "Classic HD dynamic-light shading requires {CLASSIC_HD_TABLESN_BYTES} bytes, got {}",
            shading.len()
        )));
    }
    if tablesn.len() != CLASSIC_HD_TABLESN_BYTES {
        return Err(ClassicHdRasterError(format!(
            "Classic HD dynamic-light TABLESN requires {CLASSIC_HD_TABLESN_BYTES} bytes, got {}",
            tablesn.len()
        )));
    }
    let width = usize::try_from(profile.width).unwrap();
    let height = usize::try_from(profile.height).unwrap();
    let stride = usize::try_from(profile.stride).unwrap();
    for y in 0..height {
        let map_y = y * 256 / height;
        for x in 0..width {
            let map_x = x * 256 / width;
            let offset = y * stride + x;
            let shade = shading[map_y * 256 + map_x];
            indices[offset] = tablesn[palette_address(shade, indices[offset])];
        }
    }
    Ok(byte_hash(indices))
}

/// Deterministic cross-shaped indexed smoothing.  This is opt-in Classic HD
/// presentation work; Exact never calls it.
pub fn smooth_classic_hd_indexed(
    profile: &ClassicHdRenderProfile,
    input: &[u8],
    output: &mut [u8],
    radius: u32,
) -> Result<u64, ClassicHdRasterError> {
    validate_plane(profile, input, "smoothing input")?;
    validate_plane(profile, output, "smoothing output")?;
    if radius == 0 || radius > 4 {
        return Err(ClassicHdRasterError(
            "Classic HD smoothing radius must be between 1 and 4".to_owned(),
        ));
    }
    let width = usize::try_from(profile.width).unwrap();
    let height = usize::try_from(profile.height).unwrap();
    let stride = usize::try_from(profile.stride).unwrap();
    let radius = usize::try_from(radius).unwrap();
    for y in 0..height {
        for x in 0..width {
            let mut sum = u32::from(input[y * stride + x]);
            let mut count = 1_u32;
            for distance in 1..=radius {
                for (nx, ny) in [
                    (x.saturating_sub(distance), y),
                    ((x + distance).min(width - 1), y),
                    (x, y.saturating_sub(distance)),
                    (x, (y + distance).min(height - 1)),
                ] {
                    sum += u32::from(input[ny * stride + nx]);
                    count += 1;
                }
            }
            output[y * stride + x] = u8::try_from(sum / count).unwrap();
        }
    }
    Ok(byte_hash(output))
}

/// Apply the source 2x2 indexed smoothing kernel to a profile-sized plane.
/// `radius` repeats the source pass, which gives the HD presentation mode a
/// bounded strength control while keeping every colour decision in TABLESN.
pub fn smooth_classic_hd_indexed_with_tablesn(
    profile: &ClassicHdRenderProfile,
    input: &[u8],
    output: &mut [u8],
    input_provenance: &[u8],
    output_provenance: &mut [u8],
    tablesn: &[u8],
    radius: u32,
) -> Result<u64, ClassicHdRasterError> {
    validate_plane(profile, input, "TABLESN smoothing input")?;
    validate_plane(profile, output, "TABLESN smoothing output")?;
    validate_plane(
        profile,
        input_provenance,
        "TABLESN smoothing input provenance",
    )?;
    validate_plane(
        profile,
        output_provenance,
        "TABLESN smoothing output provenance",
    )?;
    if radius == 0 || radius > 4 {
        return Err(ClassicHdRasterError(
            "Classic HD TABLESN smoothing radius must be between 1 and 4".to_owned(),
        ));
    }
    if tablesn.len() < TABLESN_REQUIRED_BYTES {
        return Err(ClassicHdRasterError(format!(
            "Classic HD TABLESN smoothing requires at least {TABLESN_REQUIRED_BYTES} bytes, got {}",
            tablesn.len()
        )));
    }
    output.copy_from_slice(input);
    output_provenance.copy_from_slice(input_provenance);
    let width = usize::try_from(profile.width).unwrap();
    let height = usize::try_from(profile.height).unwrap();
    let stride = usize::try_from(profile.stride).unwrap();
    for _ in 0..radius {
        if width < 2 || height < 2 {
            break;
        }
        for row in 0..height - 1 {
            for column in 0..width - 1 {
                let top_left = row * stride + column;
                let top_right = top_left + 1;
                let bottom_left = top_left + stride;
                let bottom_right = bottom_left + 1;
                let upper = tablesn[TABLESN_BLEND_OFFSET
                    + (usize::from(output[bottom_left]) << 8)
                    + usize::from(output[top_left])];
                let lower = tablesn[TABLESN_BLEND_OFFSET
                    + (usize::from(output[bottom_right]) << 8)
                    + usize::from(output[top_right])];
                output[top_left] =
                    tablesn[TABLESN_BLEND_OFFSET + (usize::from(lower) << 8) + usize::from(upper)];
                output_provenance[top_left] |= crate::indexed_postprocess::SPATIAL_PROVENANCE;
            }
        }
    }
    Ok(byte_hash(output))
}

/// Blend a frame against a prior presentation history only when the full
/// generation/profile/tick/input key proves that the history is compatible.
pub fn blend_classic_hd_history(
    profile: &ClassicHdRenderProfile,
    current: &mut [u8],
    history: &[u8],
    current_key: ClassicHdHistoryKey,
    history_key: ClassicHdHistoryKey,
    history_weight: u8,
) -> Result<u64, ClassicHdRasterError> {
    validate_plane(profile, current, "history current")?;
    validate_plane(profile, history, "history previous")?;
    validate_history_keys(current_key, history_key, profile.profile_revision)?;
    let width = usize::try_from(profile.width).unwrap();
    let height = usize::try_from(profile.height).unwrap();
    let stride = usize::try_from(profile.stride).unwrap();
    let history_weight = u32::from(history_weight);
    let current_weight = 255_u32 - history_weight;
    for y in 0..height {
        for x in 0..width {
            let offset = y * stride + x;
            let value = u32::from(current[offset]) * current_weight
                + u32::from(history[offset]) * history_weight
                + 127;
            current[offset] = u8::try_from(value / 255).unwrap();
        }
    }
    Ok(byte_hash(current))
}

/// Blend a Classic HD frame through the source indexed TABLESN pair table.
///
/// Mode 1 keeps the source's first-dword operand reversal while mode 2 uses
/// the ordinary current/high-byte and prior/low-byte order. The helper is
/// separate from the arithmetic fallback above so existing H-level callers
/// remain source-compatible while the graph can prove the real postprocess
/// table was used.
pub fn blend_classic_hd_history_with_tablesn(
    profile: &ClassicHdRenderProfile,
    current: &mut [u8],
    current_provenance: &mut [u8],
    history: &[u8],
    current_key: ClassicHdHistoryKey,
    history_key: ClassicHdHistoryKey,
    tablesn: &[u8],
    mode: u8,
) -> Result<u64, ClassicHdRasterError> {
    validate_plane(profile, current, "TABLESN history current")?;
    validate_plane(profile, current_provenance, "TABLESN current provenance")?;
    validate_plane(profile, history, "TABLESN history previous")?;
    validate_history_keys(current_key, history_key, profile.profile_revision)?;
    if !matches!(mode, 1 | 2) {
        return Err(ClassicHdRasterError(format!(
            "Classic HD TABLESN history mode must be 1 or 2, got {mode}"
        )));
    }
    if tablesn.len() < TABLESN_REQUIRED_BYTES {
        return Err(ClassicHdRasterError(format!(
            "Classic HD TABLESN history requires at least {TABLESN_REQUIRED_BYTES} bytes, got {}",
            tablesn.len()
        )));
    }
    let width = usize::try_from(profile.width).unwrap();
    let height = usize::try_from(profile.height).unwrap();
    let stride = usize::try_from(profile.stride).unwrap();
    let dword_width = width / 4 * 4;
    for y in 0..height {
        let row_start = y * stride;
        for column in 0..dword_width {
            let offset = row_start + column;
            let current_index = usize::from(current[offset]);
            let prior_index = usize::from(history[offset]);
            let reverse = mode == 1 && y == 0 && column < 4;
            let pair = if reverse {
                (prior_index << 8) | current_index
            } else {
                (current_index << 8) | prior_index
            };
            current[offset] = tablesn[TABLESN_BLEND_OFFSET + pair];
            current_provenance[offset] = SOURCE_WORLD_PROVENANCE | PRIOR_MAIN_PROVENANCE;
        }
    }
    Ok(byte_hash(current))
}

fn validate_history_keys(
    current: ClassicHdHistoryKey,
    history: ClassicHdHistoryKey,
    profile_revision: u64,
) -> Result<(), ClassicHdRasterError> {
    if current.profile_revision != profile_revision || history.profile_revision != profile_revision
    {
        return Err(ClassicHdRasterError(
            "Classic HD history profile revision mismatch".to_owned(),
        ));
    }
    if current.generation != history.generation {
        return Err(ClassicHdRasterError(
            "Classic HD history generation mismatch".to_owned(),
        ));
    }
    if history.tick >= current.tick {
        return Err(ClassicHdRasterError(
            "Classic HD history tick is stale or out of order".to_owned(),
        ));
    }
    if current.input_hash != history.input_hash {
        return Err(ClassicHdRasterError(
            "Classic HD history input hash mismatch".to_owned(),
        ));
    }
    Ok(())
}

fn validate_writer(writer: &ClassicHdFrameWriter<'_>) -> Result<(), ClassicHdRasterError> {
    if writer.stride() != writer.width() {
        return Err(ClassicHdRasterError(
            "Classic HD raster kernels require stride equal to width".to_owned(),
        ));
    }
    Ok(())
}

fn validate_plane(
    profile: &ClassicHdRenderProfile,
    plane: &[u8],
    name: &str,
) -> Result<(), ClassicHdRasterError> {
    if plane.len() != profile.extent() {
        return Err(ClassicHdRasterError(format!(
            "Classic HD {name} requires {} bytes, got {}",
            profile.extent(),
            plane.len()
        )));
    }
    Ok(())
}

fn validate_texture(texture_atlas: &[u8], texture_base: usize) -> Result<(), ClassicHdRasterError> {
    let end = texture_base
        .checked_add(256 * 256)
        .ok_or_else(|| ClassicHdRasterError("Classic HD texture address overflow".to_owned()))?;
    if end > texture_atlas.len() {
        return Err(ClassicHdRasterError(format!(
            "Classic HD texture bank is too small for base {texture_base:#x}"
        )));
    }
    Ok(())
}

fn validate_texture_bank(texture_atlas: &[u8]) -> Result<(), ClassicHdRasterError> {
    if texture_atlas.len() != CLASSIC_HD_TERRAIN_ATLAS_BYTES {
        return Err(ClassicHdRasterError(format!(
            "Classic HD terrain atlas requires {CLASSIC_HD_TERRAIN_ATLAS_BYTES} bytes, got {}",
            texture_atlas.len()
        )));
    }
    Ok(())
}

fn validate_submission_plan(
    plan: &ClassicHdTerrainSubmissionPlan,
    terrain: &ClassicHdTerrainRender,
) -> Result<(), ClassicHdRasterError> {
    for triangle in plan.reflected.iter().chain(plan.primary.iter()) {
        if !matches!(triangle.raster_selector, 5 | 26) {
            return Err(ClassicHdRasterError(format!(
                "Classic HD terrain selector {} is unsupported",
                triangle.raster_selector
            )));
        }
        if triangle.raster_selector == 26 && triangle.pass != TerrainPass::Primary {
            return Err(ClassicHdRasterError(
                "Classic HD mode 26 is valid only for primary terrain".to_owned(),
            ));
        }
        let _ = triangle_vertices(triangle, terrain)?;
        terrain_texture_base(triangle.texture_index)?;
    }
    Ok(())
}

fn triangle_vertices(
    triangle: &ClassicHdTerrainTriangle,
    terrain: &ClassicHdTerrainRender,
) -> Result<[Mode5Vertex; 3], ClassicHdRasterError> {
    let uv = source_uv_row(triangle.texture_uv_row)?;
    let columns = terrain.lattice.columns;
    let mut result = [Mode5Vertex::default(); 3];
    for (destination, source_index) in result.iter_mut().zip(triangle.vertex_indices) {
        let projected = terrain
            .projected
            .get(source_index)
            .and_then(Option::as_ref)
            .ok_or_else(|| {
                ClassicHdRasterError(format!(
                    "Classic HD submitted vertex {source_index} has no projected value"
                ))
            })?;
        let lattice = terrain.lattice.vertices.get(source_index).ok_or_else(|| {
            ClassicHdRasterError(format!(
                "Classic HD submitted vertex {source_index} is out of bounds"
            ))
        })?;
        let corner = cell_corner(triangle.owner_index, source_index, columns)?;
        let screen = match triangle.pass {
            TerrainPass::Reflected => projected.reflected_screen,
            TerrainPass::Primary => projected.primary_screen,
        };
        *destination = Mode5Vertex {
            x: screen.x,
            y: screen.y,
            u: uv[corner][0],
            v: uv[corner][1],
            pnt5: lattice.pnt5.ok_or_else(|| {
                ClassicHdRasterError(format!(
                    "Classic HD submitted vertex {source_index} has no pnt5"
                ))
            })?,
        };
    }
    Ok(result)
}

fn cell_corner(owner: usize, vertex: usize, columns: usize) -> Result<usize, ClassicHdRasterError> {
    if vertex == owner {
        Ok(0)
    } else if vertex == owner + 1 {
        Ok(1)
    } else if vertex.checked_add(columns - 1) == Some(owner) {
        Ok(2)
    } else if vertex.checked_add(columns) == Some(owner) {
        Ok(3)
    } else {
        Err(ClassicHdRasterError(format!(
            "Classic HD vertex {vertex} does not belong to owner cell {owner}"
        )))
    }
}

fn source_uv_row(row: u8) -> Result<[[i32; 2]; 4], ClassicHdRasterError> {
    let base = *UV_ROW_ORDER
        .get(usize::from(row))
        .ok_or_else(|| ClassicHdRasterError(format!("Classic HD UV row {row} is out of bounds")))?;
    Ok(BASE_UV_ROWS[base])
}

fn terrain_texture_base(selector: u8) -> Result<usize, ClassicHdRasterError> {
    let selector = usize::from(selector);
    if selector >= CLASSIC_HD_TERRAIN_TILE_COUNT {
        return Err(ClassicHdRasterError(format!(
            "Classic HD terrain selector {selector} is outside the indexed bank"
        )));
    }
    Ok(
        (selector / CLASSIC_HD_TERRAIN_TILE_COLUMNS) * CLASSIC_HD_TERRAIN_TILE_SIZE * 256
            + (selector % CLASSIC_HD_TERRAIN_TILE_COLUMNS) * CLASSIC_HD_TERRAIN_TILE_SIZE,
    )
}

fn provenance_for(pass: TerrainPass, base: u8) -> u8 {
    match pass {
        TerrainPass::Reflected => base | 0x01,
        TerrainPass::Primary => base | 0x02,
    }
}

fn writer_output_hash(writer: &ClassicHdFrameWriter<'_>) -> u64 {
    byte_hash(writer.profile().id.as_str().as_bytes()) ^ byte_hash(writer.indices())
}

fn writer_provenance_hash(writer: &ClassicHdFrameWriter<'_>) -> u64 {
    byte_hash(writer.provenance())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        classic_hd::{ClassicHdFrameBuffers, ClassicHdRenderProfile},
        mode5_raster::rasterize_mode5_triangle_from_atlas,
        mode26_raster::MODE26_BLEND_OFFSET,
    };

    fn vertices() -> [Mode5Vertex; 3] {
        [
            Mode5Vertex {
                x: 2,
                y: 0,
                u: 0,
                v: 0,
                pnt5: 12 << 16,
            },
            Mode5Vertex {
                x: 10,
                y: 2,
                u: 8 << 16,
                v: 8 << 16,
                pnt5: 20 << 16,
            },
            Mode5Vertex {
                x: 4,
                y: 8,
                u: 4 << 16,
                v: 12 << 16,
                pnt5: 16 << 16,
            },
        ]
    }

    fn tables() -> (Vec<u8>, Vec<u8>) {
        let mut texture = vec![0_u8; 256 * 256];
        let mut tablesn = vec![0_u8; NIGHT_TABLES_BYTES];
        for address in 0..=u16::MAX {
            texture[usize::from(address)] = address.to_le_bytes()[0];
            let [source, row] = address.to_le_bytes();
            tablesn[usize::from(address)] = source.wrapping_add(row);
        }
        for framebuffer in 0..256_usize {
            for shaded in 0..256_usize {
                tablesn[MODE26_BLEND_OFFSET + (framebuffer << 8) + shaded] =
                    (framebuffer as u8).wrapping_sub(shaded as u8);
            }
        }
        (texture, tablesn)
    }

    #[test]
    fn compatibility_mode5_wrapper_matches_exact_kernel_and_stamps_provenance() {
        let profile = ClassicHdRenderProfile::compatibility_320x200();
        let (texture, tablesn) = tables();
        let expected = rasterize_mode5_triangle_from_atlas(
            vertices(),
            320,
            200,
            &texture,
            0,
            &tablesn[..CLASSIC_HD_TABLESN_BYTES],
        );
        let mut buffers = ClassicHdFrameBuffers::new(profile.clone(), 4).unwrap();
        let mut frame = buffers
            .begin_frame(4, 0, profile.profile_revision, 9)
            .unwrap();
        let receipt = rasterize_classic_hd_mode5_triangle(
            &mut frame,
            vertices(),
            &texture,
            0,
            &tablesn[..CLASSIC_HD_TABLESN_BYTES],
            0x20,
        )
        .unwrap();
        assert_eq!(receipt.source_pixels, expected.len() as u64);
        assert_eq!(receipt.written_pixels, expected.len() as u64);
        let identity = frame.finish();
        assert_ne!(identity.index_hash, byte_hash(&vec![0; profile.extent()]));
        assert_ne!(
            identity.provenance_hash,
            byte_hash(&vec![0; profile.extent()])
        );
    }

    #[test]
    fn mode26_wrapper_preserves_framebuffer_dependent_water_order() {
        let profile = ClassicHdRenderProfile::compatibility_320x200();
        let (mut texture, tablesn) = tables();
        texture[0] = 11;
        let mut buffers = ClassicHdFrameBuffers::new(profile.clone(), 4).unwrap();
        let mut frame = buffers
            .begin_frame(4, 0, profile.profile_revision, 9)
            .unwrap();
        frame.set_pixel(0, 0, 231, 0);
        let receipt = rasterize_classic_hd_mode26_triangle(
            &mut frame,
            [
                Mode5Vertex {
                    x: 0,
                    y: 0,
                    u: 0,
                    v: 0,
                    pnt5: 3 << 16,
                },
                Mode5Vertex {
                    x: 1,
                    y: 0,
                    u: 0,
                    v: 0,
                    pnt5: 3 << 16,
                },
                Mode5Vertex {
                    x: 0,
                    y: 1,
                    u: 0,
                    v: 0,
                    pnt5: 3 << 16,
                },
            ],
            &texture,
            0,
            &tablesn,
            0x40,
        )
        .unwrap();
        assert_eq!(receipt.mode26_triangles, 1);
        assert!(receipt.written_pixels >= 1);
        assert_eq!(frame.row_mut(0).unwrap()[0], 217);
        assert_eq!(frame.provenance_row_mut(0).unwrap()[0], 0x40);
    }

    #[test]
    fn indexed_postprocesses_are_bounded_and_history_is_fail_closed() {
        let profile = ClassicHdRenderProfile::hd_640x480();
        let mut indices = vec![10_u8; profile.extent()];
        let shading = vec![2_u8; 256 * 256];
        let tablesn = vec![3_u8; CLASSIC_HD_TABLESN_BYTES];
        let hash =
            apply_classic_hd_dynamic_lighting(&profile, &mut indices, &shading, &tablesn).unwrap();
        assert_ne!(hash, 0);
        assert!(indices.iter().all(|value| *value == 3));
        let mut smoothed = vec![0_u8; profile.extent()];
        assert!(smooth_classic_hd_indexed(&profile, &indices, &mut smoothed, 1).is_ok());
        assert!(smoothed.iter().all(|value| *value == 3));
        let before = smoothed.clone();
        let current = ClassicHdHistoryKey {
            generation: 1,
            profile_revision: profile.profile_revision,
            tick: 4,
            input_hash: 99,
        };
        let stale = ClassicHdHistoryKey {
            generation: 1,
            profile_revision: profile.profile_revision,
            tick: 4,
            input_hash: 99,
        };
        assert!(
            blend_classic_hd_history(&profile, &mut smoothed, &indices, current, stale, 128)
                .is_err()
        );
        assert_eq!(smoothed, before);
        let history = ClassicHdHistoryKey {
            generation: 1,
            profile_revision: profile.profile_revision,
            tick: 3,
            input_hash: 99,
        };
        assert!(
            blend_classic_hd_history(&profile, &mut smoothed, &indices, current, history, 128)
                .is_ok()
        );
        assert_eq!(palette_address(u8::MAX, u8::MAX), 65_535);
    }

    #[test]
    fn malformed_profile_planes_and_table_lengths_fail_before_mutation() {
        let profile = ClassicHdRenderProfile::hd_640x480();
        let mut indices = vec![7_u8; profile.extent()];
        let before = indices.clone();
        assert!(
            apply_classic_hd_dynamic_lighting(&profile, &mut indices, &[0; 7], &[0; 7]).is_err()
        );
        assert_eq!(indices, before);
        let mut buffers = ClassicHdFrameBuffers::new(profile.clone(), 1).unwrap();
        let mut frame = buffers
            .begin_frame(1, 0, profile.profile_revision, 3)
            .unwrap();
        let (texture, _tablesn) = tables();
        assert!(
            rasterize_classic_hd_mode5_triangle(&mut frame, vertices(), &texture, 0, &[0; 7], 1)
                .is_err()
        );
        assert!(frame.indices_mut().iter().all(|value| *value == 0));
    }
}
