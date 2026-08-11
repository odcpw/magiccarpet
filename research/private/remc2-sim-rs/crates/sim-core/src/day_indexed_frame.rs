//! Qualified Level 2 Day indexed terrain-frame production.
//!
//! This module is the renderer-facing Rust boundary for the bounded Day
//! profile.  It consumes the already validated Level 2 package/session and a
//! separately qualified owner material cache; it does not construct a second
//! simulation state or fall back to the sealed Level 1 cache.

use std::{
    collections::BTreeSet,
    fmt, fs,
    path::{Component, Path},
};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    FlightState,
    classic_hd::{ClassicHdFrameWriter, ClassicHdRenderProfile, byte_hash},
    classic_hd_raster::rasterize_classic_hd_terrain,
    classic_hd_terrain::{ClassicHdSkyBank, ClassicHdTerrainResources, build_classic_hd_terrain},
    mission_content::REMC2_SPEC_COMMIT,
    native_world::{Level2ResetSession, MapType, NativeWorldInspection, OwnerPackageProfile},
    terrain_frame::Level1TerrainFrameCamera,
};

const MATERIAL_SCHEMA: &str = "mc2-level2-day-material-cache/v1";
const MATERIAL_DESCRIPTOR_SCHEMA: &str = "mc2-level2-day-materials/v1";
const MATERIAL_GENERATOR: &str = "build_cache.py";
const MATERIAL_PROFILE_NAME: &str = "level2-day";
const MATERIAL_PACKAGE_LEVEL: u64 = 2;
const MATERIAL_INTERNAL_LEVEL: u64 = 1;
const MATERIAL_MAP_TYPE_ID: u64 = 0;
const MATERIAL_VGA_RESOLUTION_SELECTOR: u64 = 1;
const MATERIAL_TEXTURE_SIZE: u64 = 32;
const MATERIAL_TERRAIN_GRAPHICS_FLAGS: u64 = 0;
const MATERIAL_TERRAIN_BANK: &str = "TMAPS0-0";
const MATERIAL_SPRITE_BANK: &str = "MSPRD0-0";
const MATERIAL_WEB_BANK: &str = "MWEBD0-0";
const MATERIAL_BLOCK_BANK: &str = "BL32N0-0";
const MATERIAL_PALETTE: &str = "PALD-0";
const MATERIAL_SKY_BANK: &str = "SKYD0-0";

const PALETTE_RGB8_PATH: &str = "palette/pald.rgb8";
const TABLESD_PATH: &str = "palette/tablesd.u8";
const SKY_PATH: &str = "sky/skyd.indices.u8";
const TERRAIN_ATLAS_PATH: &str = "terrain/bl32n.atlas.indices.u8";
const TERRAIN_TILES_PATH: &str = "terrain/bl32n.indices.u8";
const MATERIAL_DESCRIPTOR_PATH: &str = "materials/descriptor.json";

const PALETTE_RGB8_BYTES: usize = 256 * 3;
const TABLESD_BYTES: usize = 83_456;
const TABLESD_SHADE_LUT_BYTES: usize = 256 * 256;
const SKY_BYTES: usize = 256 * 256;
const TERRAIN_ATLAS_BYTES: usize = 256 * 608;
const TERRAIN_TILE_BANK_BYTES: usize = 152 * 32 * 32;
const TERRAIN_PLANE_BYTES: usize = 256 * 256;
const DAY_FRAME_WIDTH: u16 = 320;
const DAY_FRAME_HEIGHT: u16 = 200;
const DAY_SOURCE_FOV: i16 = 128;
const DAY_TERRAIN_PROVENANCE: u8 = 1;

/// The validated owner-data profile carried by [`DayIndexedFrameAssets`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DayIndexedProfile {
    pub external_level: u16,
    pub internal_level: u16,
    pub map_type_id: u8,
    pub vga_resolution_selector: u8,
    pub texture_size: u8,
}

impl DayIndexedProfile {
    #[must_use]
    pub const fn qualified() -> Self {
        Self {
            external_level: 2,
            internal_level: 1,
            map_type_id: 0,
            vga_resolution_selector: 1,
            texture_size: 32,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DayIndexedFrameError(String);

impl DayIndexedFrameError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for DayIndexedFrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for DayIndexedFrameError {}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OutputRecord {
    path: String,
    sha256: String,
    bytes: usize,
    kind: String,
    source_asset_ids: Vec<String>,
}

/// Decoded owner products needed by the Day indexed world pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DayIndexedFrameAssets {
    profile: DayIndexedProfile,
    package_sha256: String,
    cache_content_sha256: String,
    palette_rgb8: Box<[u8]>,
    tablesd: Box<[u8]>,
    sky: Box<[u8]>,
    terrain_atlas: Box<[u8]>,
}

impl DayIndexedFrameAssets {
    /// Load and fully validate one owner-generated Level 2 Day material cache.
    ///
    /// The manifest payload, every listed output hash/size, content-tree
    /// identity, profile selectors, descriptor bounds, and source roots are
    /// checked before any decoded product is returned.  Unknown or tampered
    /// caches therefore fail closed instead of falling back to Level 1.
    pub fn load(cache_dir: impl AsRef<Path>) -> Result<Self, DayIndexedFrameError> {
        let cache_dir = cache_dir.as_ref();
        let manifest_path = cache_dir.join("manifest.json");
        let manifest_bytes = fs::read(&manifest_path).map_err(|error| {
            DayIndexedFrameError::new(format!(
                "cannot read Level 2 Day material manifest {}: {error}",
                manifest_path.display()
            ))
        })?;
        let manifest: Value = serde_json::from_slice(&manifest_bytes).map_err(|error| {
            DayIndexedFrameError::new(format!(
                "invalid Level 2 Day material manifest JSON: {error}"
            ))
        })?;
        let manifest_object = object(&manifest, "material manifest")?;

        let claimed_manifest_hash = string_field(
            manifest_object,
            "manifest_payload_sha256",
            "material manifest",
        )?;
        let mut unsigned_manifest = manifest.clone();
        unsigned_manifest
            .as_object_mut()
            .ok_or_else(|| DayIndexedFrameError::new("material manifest is not an object"))?
            .remove("manifest_payload_sha256");
        let actual_manifest_hash = sha256_hex(canonical_json(&unsigned_manifest)?.as_bytes());
        if actual_manifest_hash != claimed_manifest_hash {
            return Err(DayIndexedFrameError::new(
                "Level 2 Day material manifest metadata hash mismatch",
            ));
        }
        if string_field(manifest_object, "schema", "material manifest")? != MATERIAL_SCHEMA {
            return Err(DayIndexedFrameError::new(
                "unsupported Level 2 Day material cache schema",
            ));
        }
        let generator = object_field(manifest_object, "generator", "material manifest")?;
        if string_field(generator, "name", "material generator")? != MATERIAL_GENERATOR {
            return Err(DayIndexedFrameError::new(
                "Level 2 Day material generator identity mismatch",
            ));
        }

        let profile_value = manifest_object
            .get("profile")
            .ok_or_else(|| DayIndexedFrameError::new("material manifest profile is missing"))?;
        let expected_profile = qualified_profile_json();
        if profile_value != &expected_profile {
            return Err(DayIndexedFrameError::new(
                "Level 2 Day material profile identity mismatch",
            ));
        }
        let profile_object = object(profile_value, "material profile")?;
        let package_sha256 =
            string_field(profile_object, "package_sha256", "material profile")?.to_owned();
        let closure_sha256 =
            string_field(profile_object, "closure_sha256", "material profile")?.to_owned();
        let bindings = object_field(manifest_object, "bindings", "material manifest")?;
        for (name, expected) in [
            ("level_export_package_sha256", package_sha256.as_str()),
            ("level_asset_closure_sha256", closure_sha256.as_str()),
            (
                "owner_catalog_sha256",
                string_field(profile_object, "catalog_sha256", "material profile")?,
            ),
            (
                "owner_catalog_file_sha256",
                string_field(profile_object, "catalog_file_sha256", "material profile")?,
            ),
            ("remc2_commit", REMC2_SPEC_COMMIT),
        ] {
            if string_field(bindings, name, "material bindings")? != expected {
                return Err(DayIndexedFrameError::new(format!(
                    "Level 2 Day material binding {name} does not match the qualified profile"
                )));
            }
        }

        let output_values = manifest_object
            .get("outputs")
            .and_then(Value::as_array)
            .ok_or_else(|| DayIndexedFrameError::new("Level 2 Day material outputs are missing"))?;
        if output_values.is_empty() {
            return Err(DayIndexedFrameError::new(
                "Level 2 Day material cache has no outputs",
            ));
        }
        let mut outputs = Vec::with_capacity(output_values.len());
        let mut seen_paths = BTreeSet::new();
        for (ordinal, value) in output_values.iter().enumerate() {
            let row = object(value, "material output")?;
            let path = string_field(row, "path", "material output")?.to_owned();
            let relative = Path::new(&path);
            if relative.is_absolute()
                || relative
                    .components()
                    .any(|component| component == Component::ParentDir)
                || !seen_paths.insert(path.clone())
            {
                return Err(DayIndexedFrameError::new(format!(
                    "invalid or duplicate Level 2 material output path at ordinal {ordinal}"
                )));
            }
            let sha256 = string_field(row, "sha256", "material output")?.to_owned();
            if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(DayIndexedFrameError::new(format!(
                    "invalid Level 2 material output hash for {path}"
                )));
            }
            let bytes =
                usize_from_value(row.get("bytes"), &format!("material output {path}.bytes"))?;
            let kind = string_field(row, "kind", "material output")?.to_owned();
            let source_asset_ids = row
                .get("source_asset_ids")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    DayIndexedFrameError::new(format!(
                        "material output {path} has no source_asset_ids"
                    ))
                })?
                .iter()
                .map(|value| {
                    value.as_str().map(str::to_owned).ok_or_else(|| {
                        DayIndexedFrameError::new(format!(
                            "material output {path} has a non-string source_asset_id"
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            outputs.push(OutputRecord {
                path,
                sha256,
                bytes,
                kind,
                source_asset_ids,
            });
        }

        let claimed_content_hash =
            string_field(manifest_object, "cache_content_sha256", "material manifest")?;
        let actual_content_hash = output_identity(&outputs);
        if actual_content_hash != claimed_content_hash {
            return Err(DayIndexedFrameError::new(
                "Level 2 Day material cache content identity mismatch",
            ));
        }
        validate_output_tree(cache_dir, &outputs)?;
        validate_output_files(cache_dir, &outputs)?;

        let descriptor_bytes = read_output(
            cache_dir,
            &outputs,
            MATERIAL_DESCRIPTOR_PATH,
            "canonical_json_day_material_descriptor",
            64_695,
            &[
                "owner_data:DATA/PALD-0.DAT",
                "owner_data:DATA/TABLESD.DAT",
                "owner_data:DATA/CLRD-0.DAT",
                "owner_data:DATA/BL32N0-0.DAT",
                "owner_data:DATA/SKYD0-0.DAT",
            ],
        )?;
        validate_descriptor(&descriptor_bytes, profile_value)?;

        let palette_rgb8 = read_output(
            cache_dir,
            &outputs,
            PALETTE_RGB8_PATH,
            "ordered_rgb8_palette_exact_vga_scale",
            PALETTE_RGB8_BYTES,
            &["owner_data:DATA/PALD-0.DAT"],
        )?;
        let tablesd = read_output(
            cache_dir,
            &outputs,
            TABLESD_PATH,
            "remc2_day_renderer_tables_u8",
            TABLESD_BYTES,
            &["owner_data:DATA/TABLESD.DAT"],
        )?;
        let sky = read_output(
            cache_dir,
            &outputs,
            SKY_PATH,
            "indexed_image_u8",
            SKY_BYTES,
            &["owner_data:DATA/SKYD0-0.DAT"],
        )?;
        let terrain_atlas = read_output(
            cache_dir,
            &outputs,
            TERRAIN_ATLAS_PATH,
            "indexed_atlas_u8",
            TERRAIN_ATLAS_BYTES,
            &["owner_data:DATA/BL32N0-0.DAT"],
        )?;
        let _ = read_output(
            cache_dir,
            &outputs,
            TERRAIN_TILES_PATH,
            "indexed_tile_bank_u8",
            TERRAIN_TILE_BANK_BYTES,
            &["owner_data:DATA/BL32N0-0.DAT"],
        )?;

        Ok(Self {
            profile: DayIndexedProfile::qualified(),
            package_sha256,
            cache_content_sha256: claimed_content_hash.to_owned(),
            palette_rgb8: palette_rgb8.into_boxed_slice(),
            tablesd: tablesd.into_boxed_slice(),
            sky: sky.into_boxed_slice(),
            terrain_atlas: terrain_atlas.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn profile(&self) -> DayIndexedProfile {
        self.profile
    }

    #[must_use]
    pub fn package_sha256(&self) -> &str {
        &self.package_sha256
    }

    #[must_use]
    pub fn cache_content_sha256(&self) -> &str {
        &self.cache_content_sha256
    }

    #[must_use]
    pub fn palette_rgb8(&self) -> &[u8] {
        &self.palette_rgb8
    }

    #[must_use]
    pub fn tablesd(&self) -> &[u8] {
        &self.tablesd
    }

    #[must_use]
    pub fn sky(&self) -> &[u8] {
        &self.sky
    }

    #[must_use]
    pub fn terrain_atlas(&self) -> &[u8] {
        &self.terrain_atlas
    }
}

/// One deterministic indexed Day world frame and its source-bound identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DayIndexedFrame {
    pub profile: DayIndexedProfile,
    pub generation: u64,
    pub tick: u64,
    pub width: u16,
    pub height: u16,
    pub indices: Vec<u8>,
    pub palette_rgb8: Vec<u8>,
    pub index_hash: u64,
    pub terrain_input_hash: u64,
    pub terrain_command_hash: u64,
    pub cache_content_sha256: String,
    pub package_sha256: String,
}

/// Transactional generation/tick carrier for the frame producer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DayIndexedFrameState {
    generation: u64,
    current_tick: Option<u64>,
    current_frame: Option<DayIndexedFrame>,
}

impl DayIndexedFrameState {
    #[must_use]
    pub const fn new(generation: u64) -> Self {
        Self {
            generation,
            current_tick: None,
            current_frame: None,
        }
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn current_tick(&self) -> Option<u64> {
        self.current_tick
    }

    #[must_use]
    pub fn current_frame(&self) -> Option<&DayIndexedFrame> {
        self.current_frame.as_ref()
    }

    pub fn reset(&mut self, generation: u64) {
        self.generation = generation;
        self.current_tick = None;
        self.current_frame = None;
    }

    /// Render the next frame without mutating this carrier until the complete
    /// qualified terrain pass succeeds.
    pub fn render(
        &mut self,
        inspection: &NativeWorldInspection,
        session: &Level2ResetSession,
        assets: &DayIndexedFrameAssets,
        generation: u64,
    ) -> Result<DayIndexedFrame, DayIndexedFrameError> {
        if generation != self.generation {
            return Err(DayIndexedFrameError::new(format!(
                "Day indexed frame generation {generation} does not match {}",
                self.generation
            )));
        }
        let expected_tick = match self.current_tick {
            Some(current) => current
                .checked_add(1)
                .ok_or_else(|| DayIndexedFrameError::new("Day indexed frame tick overflow"))?,
            None => 0,
        };
        let tick = session.state.tick;
        if tick != expected_tick {
            return Err(DayIndexedFrameError::new(format!(
                "Day indexed frame tick {tick} is out of order; expected {expected_tick}"
            )));
        }
        let frame = render_level2_day_indexed_frame(inspection, session, assets, generation)?;
        self.current_tick = Some(tick);
        self.current_frame = Some(frame.clone());
        Ok(frame)
    }
}

/// Render one ordinary Level 2 frame from the public package/session route.
pub fn render_level2_day_indexed_frame(
    inspection: &NativeWorldInspection,
    session: &Level2ResetSession,
    assets: &DayIndexedFrameAssets,
    generation: u64,
) -> Result<DayIndexedFrame, DayIndexedFrameError> {
    validate_level2_route(inspection, session, assets)?;
    let tick = session.state.tick;
    let camera = camera_from_state(&session.state)?;
    let profile = ClassicHdRenderProfile::compatibility_320x200();
    let planes = session.terrain.planes();
    validate_terrain_planes(
        planes.terrain_types.as_ref(),
        planes.ground_heights.as_ref(),
        planes.shading.as_ref(),
        planes.angles.as_ref(),
    )?;
    let terrain = build_classic_hd_terrain(
        &profile,
        camera,
        ClassicHdTerrainResources {
            terrain_types: &planes.terrain_types,
            ground_heights: &planes.ground_heights,
            shading: &planes.shading,
            angles: &planes.angles,
            sky: assets.sky(),
        },
        ClassicHdSkyBank::Base256,
    )
    .map_err(|error| DayIndexedFrameError::new(format!("Day terrain build failed: {error}")))?;
    let extent = profile.extent();
    let mut indices = terrain.sky.clone();
    if indices.len() != extent {
        return Err(DayIndexedFrameError::new(format!(
            "Day sky extent {} does not match profile extent {extent}",
            indices.len()
        )));
    }
    let mut provenance = vec![0_u8; extent];
    let identity = {
        let mut writer = ClassicHdFrameWriter::from_planes(
            &profile,
            &mut indices,
            &mut provenance,
            generation,
            tick,
            terrain.input_hash,
            0,
        )
        .map_err(|error| {
            DayIndexedFrameError::new(format!("Day frame allocation failed: {error}"))
        })?;
        rasterize_classic_hd_terrain(
            &mut writer,
            &terrain,
            assets.terrain_atlas(),
            assets.tablesd(),
            DAY_TERRAIN_PROVENANCE,
        )
        .map_err(|error| {
            DayIndexedFrameError::new(format!("Day terrain raster failed: {error}"))
        })?;
        writer.finish()
    };
    if identity.index_hash != byte_hash(&indices) {
        return Err(DayIndexedFrameError::new(
            "Day indexed frame hash does not match its output bytes",
        ));
    }
    Ok(DayIndexedFrame {
        profile: assets.profile(),
        generation,
        tick,
        width: DAY_FRAME_WIDTH,
        height: DAY_FRAME_HEIGHT,
        indices,
        palette_rgb8: assets.palette_rgb8().to_vec(),
        index_hash: identity.index_hash,
        terrain_input_hash: terrain.input_hash,
        terrain_command_hash: terrain.command_hash,
        cache_content_sha256: assets.cache_content_sha256.clone(),
        package_sha256: assets.package_sha256.clone(),
    })
}

fn validate_level2_route(
    inspection: &NativeWorldInspection,
    session: &Level2ResetSession,
    assets: &DayIndexedFrameAssets,
) -> Result<(), DayIndexedFrameError> {
    if inspection.profile() != OwnerPackageProfile::Level2 {
        return Err(DayIndexedFrameError::new(
            "Day indexed frame requires the Level 2 owner package",
        ));
    }
    let metadata = inspection.metadata();
    if metadata.external_level != 2
        || metadata.internal_level != 1
        || metadata.map_type != MapType::Day
    {
        return Err(DayIndexedFrameError::new(
            "Day indexed frame metadata is not the qualified external-2/internal-1 Day profile",
        ));
    }
    if assets.package_sha256() != inspection.package_sha256()
        || session.package_sha256() != inspection.package_sha256()
    {
        return Err(DayIndexedFrameError::new(
            "Day indexed frame inspection, session, and material cache owner identities differ",
        ));
    }
    if session.profile().cave_mode {
        return Err(DayIndexedFrameError::new(
            "Day indexed frame cannot consume a cave flight profile",
        ));
    }
    Ok(())
}

fn validate_terrain_planes(
    terrain_types: &[u8],
    ground_heights: &[u8],
    shading: &[u8],
    angles: &[u8],
) -> Result<(), DayIndexedFrameError> {
    for (name, plane) in [
        ("terrain_types", terrain_types),
        ("ground_heights", ground_heights),
        ("shading", shading),
        ("angles", angles),
    ] {
        if plane.len() != TERRAIN_PLANE_BYTES {
            return Err(DayIndexedFrameError::new(format!(
                "Level 2 Day {name} plane requires {TERRAIN_PLANE_BYTES} bytes, got {}",
                plane.len()
            )));
        }
    }
    Ok(())
}

fn camera_from_state(
    state: &FlightState,
) -> Result<Level1TerrainFrameCamera, DayIndexedFrameError> {
    let source_turn = i32::try_from(state.tick)
        .map_err(|_| DayIndexedFrameError::new("Level 2 Day source tick does not fit i32"))?;
    Ok(Level1TerrainFrameCamera {
        render_x: state.position.x,
        render_y: state.position.y,
        eye_z: i32::from(state.position.z),
        yaw: state.yaw,
        pitch: state.display_pitch,
        roll: state.roll_accumulator,
        fov: DAY_SOURCE_FOV,
        source_turn,
        reflections: true,
    })
}

fn validate_descriptor(bytes: &[u8], profile: &Value) -> Result<(), DayIndexedFrameError> {
    let descriptor: Value = serde_json::from_slice(bytes).map_err(|error| {
        DayIndexedFrameError::new(format!(
            "invalid Level 2 Day material descriptor JSON: {error}"
        ))
    })?;
    let object = object(&descriptor, "material descriptor")?;
    if string_field(object, "schema", "material descriptor")? != MATERIAL_DESCRIPTOR_SCHEMA
        || object.get("profile") != Some(profile)
    {
        return Err(DayIndexedFrameError::new(
            "Level 2 Day material descriptor profile identity mismatch",
        ));
    }
    let palette = object_field(object, "palette", "material descriptor")?;
    require_u64(palette, "entries", 256, "material descriptor palette")?;
    let tablesd = object_field(object, "tablesd", "material descriptor")?;
    require_u64(
        tablesd,
        "bytes",
        TABLESD_BYTES as u64,
        "material descriptor TABLESD",
    )?;
    require_u64(
        tablesd,
        "entries",
        TABLESD_SHADE_LUT_BYTES as u64,
        "material descriptor TABLESD",
    )?;
    let bl32n = object_field(object, "bl32n", "material descriptor")?;
    require_u64(bl32n, "tile_count", 152, "material descriptor BL32N")?;
    require_u64(bl32n, "tile_width", 32, "material descriptor BL32N")?;
    require_u64(bl32n, "tile_height", 32, "material descriptor BL32N")?;
    require_u64(bl32n, "atlas_width", 256, "material descriptor BL32N")?;
    require_u64(bl32n, "atlas_height", 608, "material descriptor BL32N")?;
    let skyd = object_field(object, "skyd", "material descriptor")?;
    require_u64(skyd, "width", 256, "material descriptor SKYD")?;
    require_u64(skyd, "height", 256, "material descriptor SKYD")?;
    let gpu_ready = object_field(object, "gpu_ready", "material descriptor")?;
    if string_field(gpu_ready, "indexed_format", "material descriptor gpu_ready")? != "R8_UNORM"
        || string_field(gpu_ready, "row_order", "material descriptor gpu_ready")? != "top_to_bottom"
        || string_field(gpu_ready, "origin", "material descriptor gpu_ready")? != "top_left"
    {
        return Err(DayIndexedFrameError::new(
            "Level 2 Day material descriptor GPU indexed contract mismatch",
        ));
    }
    Ok(())
}

fn validate_output_tree(
    cache_dir: &Path,
    outputs: &[OutputRecord],
) -> Result<(), DayIndexedFrameError> {
    let expected = outputs
        .iter()
        .map(|output| output.path.clone())
        .chain(std::iter::once("manifest.json".to_owned()))
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    collect_tree(cache_dir, cache_dir, &mut actual)?;
    if actual != expected {
        return Err(DayIndexedFrameError::new(
            "Level 2 Day material cache tree has missing or extra files",
        ));
    }
    Ok(())
}

fn collect_tree(
    root: &Path,
    directory: &Path,
    files: &mut BTreeSet<String>,
) -> Result<(), DayIndexedFrameError> {
    let entries = fs::read_dir(directory).map_err(|error| {
        DayIndexedFrameError::new(format!(
            "cannot read Level 2 Day material cache directory {}: {error}",
            directory.display()
        ))
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| DayIndexedFrameError::new(error.to_string()))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            DayIndexedFrameError::new(format!("cannot inspect {}: {error}", path.display()))
        })?;
        if metadata.file_type().is_symlink() {
            return Err(DayIndexedFrameError::new(format!(
                "Level 2 Day material cache contains symlink {}",
                path.display()
            )));
        }
        if metadata.is_dir() {
            collect_tree(root, &path, files)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| DayIndexedFrameError::new(error.to_string()))?
                .to_str()
                .ok_or_else(|| DayIndexedFrameError::new("material cache path is not UTF-8"))?
                .replace(std::path::MAIN_SEPARATOR, "/");
            files.insert(relative);
        } else {
            return Err(DayIndexedFrameError::new(format!(
                "Level 2 Day material cache contains unsupported filesystem entry {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn validate_output_files(
    cache_dir: &Path,
    outputs: &[OutputRecord],
) -> Result<(), DayIndexedFrameError> {
    for output in outputs {
        let path = cache_dir.join(&output.path);
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            DayIndexedFrameError::new(format!(
                "cannot inspect Level 2 Day material output {}: {error}",
                output.path
            ))
        })?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || usize::try_from(metadata.len()).ok() != Some(output.bytes)
        {
            return Err(DayIndexedFrameError::new(format!(
                "Level 2 Day material output missing or truncated: {}",
                output.path
            )));
        }
        let bytes = fs::read(&path).map_err(|error| {
            DayIndexedFrameError::new(format!(
                "cannot read Level 2 Day material output {}: {error}",
                output.path
            ))
        })?;
        if sha256_hex(&bytes) != output.sha256 {
            return Err(DayIndexedFrameError::new(format!(
                "Level 2 Day material output hash mismatch: {}",
                output.path
            )));
        }
    }
    Ok(())
}

fn read_output(
    cache_dir: &Path,
    outputs: &[OutputRecord],
    path: &str,
    expected_kind: &str,
    expected_bytes: usize,
    expected_sources: &[&str],
) -> Result<Vec<u8>, DayIndexedFrameError> {
    let output = outputs
        .iter()
        .find(|output| output.path == path)
        .ok_or_else(|| {
            DayIndexedFrameError::new(format!(
                "required Level 2 Day material output is missing: {path}"
            ))
        })?;
    if output.kind != expected_kind
        || output.bytes != expected_bytes
        || output
            .source_asset_ids
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            != expected_sources
    {
        return Err(DayIndexedFrameError::new(format!(
            "Level 2 Day material output contract mismatch: {path}"
        )));
    }
    let bytes = fs::read(cache_dir.join(path)).map_err(|error| {
        DayIndexedFrameError::new(format!(
            "cannot read required Level 2 Day output {path}: {error}"
        ))
    })?;
    if bytes.len() != expected_bytes || sha256_hex(&bytes) != output.sha256 {
        return Err(DayIndexedFrameError::new(format!(
            "required Level 2 Day material output bytes/hash mismatch: {path}"
        )));
    }
    Ok(bytes)
}

fn output_identity(outputs: &[OutputRecord]) -> String {
    let mut hasher = Sha256::new();
    for output in outputs {
        hasher.update(output.path.as_bytes());
        hasher.update([0]);
        hasher.update(output.sha256.as_bytes());
        hasher.update([0]);
        hasher.update(output.bytes.to_string().as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

fn qualified_profile_json() -> Value {
    json!({
        "catalog_file_sha256": "a25488be0933e5b4499665d37440a9860c8614649d9f695080da511c11d75b44",
        "catalog_sha256": "ea9a735c598f2cbed81541366e1fdf8a5fcbc8cef7aecd086bb8a6a9e7c2429e",
        "closure_sha256": "a4a7a3d4086c09404294fb90d53e7f34e05d58a406fb025e15f032cca18793f1",
        "external_level": MATERIAL_PACKAGE_LEVEL,
        "internal_level": MATERIAL_INTERNAL_LEVEL,
        "map_type": {"id": MATERIAL_MAP_TYPE_ID, "name": "Day"},
        "name": MATERIAL_PROFILE_NAME,
        "package_sha256": "4b8c245c268d9644d5911285c4b4da5dacb78d02034fec7d2f05366a4cc48c6a",
        "selectors": {
            "block_bank": MATERIAL_BLOCK_BANK,
            "palette": MATERIAL_PALETTE,
            "sky_bank": MATERIAL_SKY_BANK,
            "sprite_bank": MATERIAL_SPRITE_BANK,
            "terrain_bank": MATERIAL_TERRAIN_BANK,
            "terrain_graphics_flag_2": false,
            "terrain_graphics_flags": MATERIAL_TERRAIN_GRAPHICS_FLAGS,
            "texture_size": MATERIAL_TEXTURE_SIZE,
            "vga_resolution_selector": MATERIAL_VGA_RESOLUTION_SELECTOR,
            "web_bank": MATERIAL_WEB_BANK,
        },
        "spec_commit": REMC2_SPEC_COMMIT,
    })
}

fn object<'a>(
    value: &'a Value,
    context: &str,
) -> Result<&'a serde_json::Map<String, Value>, DayIndexedFrameError> {
    value
        .as_object()
        .ok_or_else(|| DayIndexedFrameError::new(format!("{context} is not an object")))
}

fn object_field<'a>(
    map: &'a serde_json::Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<&'a serde_json::Map<String, Value>, DayIndexedFrameError> {
    object(
        map.get(name)
            .ok_or_else(|| DayIndexedFrameError::new(format!("{context}.{name} is missing")))?,
        &format!("{context}.{name}"),
    )
}

fn string_field<'a>(
    object: &'a serde_json::Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<&'a str, DayIndexedFrameError> {
    object
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| DayIndexedFrameError::new(format!("{context}.{name} is not a string")))
}

fn usize_from_value(value: Option<&Value>, context: &str) -> Result<usize, DayIndexedFrameError> {
    value
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| DayIndexedFrameError::new(format!("{context} is not a bounded integer")))
}

fn require_u64(
    object: &serde_json::Map<String, Value>,
    name: &str,
    expected: u64,
    context: &str,
) -> Result<(), DayIndexedFrameError> {
    let actual = object
        .get(name)
        .and_then(Value::as_u64)
        .ok_or_else(|| DayIndexedFrameError::new(format!("{context}.{name} is not an integer")))?;
    if actual != expected {
        return Err(DayIndexedFrameError::new(format!(
            "{context}.{name} must be {expected}, got {actual}"
        )));
    }
    Ok(())
}

fn canonical_json(value: &Value) -> Result<String, DayIndexedFrameError> {
    let encoded = serde_json::to_string_pretty(value).map_err(|error| {
        DayIndexedFrameError::new(format!("cannot encode canonical JSON: {error}"))
    })?;
    Ok(format!("{encoded}\n"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_world::NativeWorld;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct TempDirectory(PathBuf);

    impl Drop for TempDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn generated_path(relative: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join("generated")
            .join(relative)
    }

    fn qualified_package() -> PathBuf {
        generated_path("l2-debug3.fJ3PaM/level2")
    }

    fn qualified_cache() -> PathBuf {
        generated_path("level2-day-material-cache")
    }

    fn copy_tree(source: &Path, destination: &Path) {
        fs::create_dir_all(destination).expect("create temporary cache directory");
        for entry in fs::read_dir(source).expect("read qualified cache") {
            let entry = entry.expect("read qualified cache entry");
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            let metadata =
                fs::symlink_metadata(&source_path).expect("inspect qualified cache entry");
            if metadata.is_dir() {
                copy_tree(&source_path, &destination_path);
            } else {
                assert!(
                    metadata.is_file(),
                    "qualified cache contains a special entry"
                );
                fs::copy(&source_path, &destination_path).expect("copy qualified cache entry");
            }
        }
    }

    fn temporary_cache() -> TempDirectory {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("mc-exzm-day-cache-{nonce}"));
        fs::create_dir(&path).expect("create unique temporary cache directory");
        copy_tree(&qualified_cache(), &path);
        TempDirectory(path)
    }

    fn refresh_manifest_claim(manifest: &mut Value) {
        let object = manifest.as_object_mut().expect("manifest object");
        object.remove("manifest_payload_sha256");
        let unsigned = Value::Object(object.clone());
        let claim = sha256_hex(
            canonical_json(&unsigned)
                .expect("canonical tampered manifest")
                .as_bytes(),
        );
        object.insert("manifest_payload_sha256".to_owned(), Value::String(claim));
    }

    #[test]
    fn level_two_day_indexed_frame_uses_qualified_profile() {
        let package = qualified_package();
        let inspection = NativeWorld::inspect_level2(&package).expect("qualified Level 2 package");
        let assets = DayIndexedFrameAssets::load(qualified_cache()).expect("qualified Day cache");
        assert_eq!(inspection.profile(), OwnerPackageProfile::Level2);
        assert_eq!(inspection.metadata().external_level, 2);
        assert_eq!(inspection.metadata().internal_level, 1);
        assert_eq!(inspection.metadata().map_type, MapType::Day);
        assert_eq!(assets.profile(), DayIndexedProfile::qualified());
        assert_eq!(assets.package_sha256(), inspection.package_sha256());
        assert_eq!(assets.palette_rgb8().len(), PALETTE_RGB8_BYTES);
        assert_eq!(assets.tablesd().len(), TABLESD_BYTES);
        assert_eq!(assets.sky().len(), SKY_BYTES);
        assert_eq!(assets.terrain_atlas().len(), TERRAIN_ATLAS_BYTES);

        let session = inspection
            .level2_reset_session()
            .expect("ordinary Level 2 reset session");
        assert_eq!(session.package_sha256(), inspection.package_sha256());
        let mut frame_state = DayIndexedFrameState::new(41);
        let first = frame_state
            .render(&inspection, &session, &assets, 41)
            .expect("ordinary Level 2 Day indexed frame");
        assert_eq!(first.width, DAY_FRAME_WIDTH);
        assert_eq!(first.height, DAY_FRAME_HEIGHT);
        assert_eq!(
            first.indices.len(),
            usize::from(DAY_FRAME_WIDTH) * usize::from(DAY_FRAME_HEIGHT)
        );
        assert_eq!(first.palette_rgb8.len(), PALETTE_RGB8_BYTES);
        assert!(first.indices.iter().any(|index| *index != 0));
        assert_eq!(first.index_hash, byte_hash(&first.indices));
        assert_eq!(first.tick, session.state.tick);
        assert_eq!(first.package_sha256, session.package_sha256());
        assert_eq!(frame_state.current_tick(), Some(0));
        assert_eq!(frame_state.current_frame(), Some(&first));

        let replay_session = inspection
            .level2_reset_session()
            .expect("replay Level 2 reset session");
        let mut replay_state = DayIndexedFrameState::new(41);
        let replay = replay_state
            .render(&inspection, &replay_session, &assets, 41)
            .expect("replay Day indexed frame");
        assert_eq!(first.indices, replay.indices);
        assert_eq!(first.index_hash, replay.index_hash);
        assert_eq!(first.terrain_input_hash, replay.terrain_input_hash);
        assert_eq!(first.terrain_command_hash, replay.terrain_command_hash);
        assert_eq!(first.cache_content_sha256, replay.cache_content_sha256);
    }

    #[test]
    fn level_two_day_indexed_frame_rejects_unknown_profile_asset_stale_generation_and_reset() {
        let package = qualified_package();
        let inspection = NativeWorld::inspect_level2(&package).expect("qualified Level 2 package");
        let assets = DayIndexedFrameAssets::load(qualified_cache()).expect("qualified Day cache");
        let tampered_cache = temporary_cache();
        let manifest_path = tampered_cache.0.join("manifest.json");
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).expect("read temporary manifest"))
                .expect("parse temporary manifest");
        manifest["profile"]["map_type"] = json!({"id": 1, "name": "Night"});
        refresh_manifest_claim(&mut manifest);
        fs::write(
            &manifest_path,
            canonical_json(&manifest).expect("encode tampered manifest"),
        )
        .expect("write tampered manifest");
        let error = DayIndexedFrameAssets::load(&tampered_cache.0)
            .expect_err("unknown profile must fail closed");
        assert!(error.to_string().contains("profile identity mismatch"));

        let output_tampered_cache = temporary_cache();
        let sky_path = output_tampered_cache.0.join(SKY_PATH);
        let mut sky = fs::read(&sky_path).expect("read temporary sky output");
        sky[0] ^= 1;
        fs::write(&sky_path, sky).expect("write tampered sky output");
        let error = DayIndexedFrameAssets::load(&output_tampered_cache.0)
            .expect_err("tampered decoded output must fail before rendering");
        assert!(error.to_string().contains(SKY_PATH));
        assert!(error.to_string().contains("hash mismatch"));

        let mut state = DayIndexedFrameState::new(9);
        let first = state
            .render(
                &inspection,
                &inspection.level2_reset_session().expect("reset session"),
                &assets,
                9,
            )
            .expect("first frame");
        let stale_generation = state
            .render(
                &inspection,
                &inspection.level2_reset_session().expect("reset session"),
                &assets,
                10,
            )
            .expect_err("stale generation must fail closed");
        assert!(stale_generation.to_string().contains("generation"));
        assert_eq!(state.current_tick(), Some(0));
        assert_eq!(state.current_frame(), Some(&first));

        let stale_tick = state
            .render(
                &inspection,
                &inspection.level2_reset_session().expect("reset session"),
                &assets,
                9,
            )
            .expect_err("out-of-order tick must fail closed");
        assert!(stale_tick.to_string().contains("out of order"));
        assert_eq!(state.current_tick(), Some(0));

        state.reset(12);
        assert_eq!(state.current_tick(), None);
        assert_eq!(state.current_frame(), None);
        let reset_session = inspection.level2_reset_session().expect("reset session");
        let after_reset = state
            .render(&inspection, &reset_session, &assets, 12)
            .expect("reset frame");
        assert_eq!(after_reset.indices, first.indices);
        assert_eq!(after_reset.generation, 12);
        assert_eq!(after_reset.tick, 0);
        assert_eq!(after_reset.tick, reset_session.state.tick);
    }
}
