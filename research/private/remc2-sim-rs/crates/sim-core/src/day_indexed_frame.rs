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
    native_world::{Level2ResetSession, MapType, NativeWorldInspection},
    presentation::environment_materials::{CACHE_SCHEMA, EnvironmentResources},
    terrain_frame::Level1TerrainFrameCamera,
};

const MATERIAL_SCHEMA: &str = "mc2-level2-day-material-cache/v1";
const MATERIAL_DESCRIPTOR_SCHEMA: &str = "mc2-level2-day-materials/v1";
const NEUTRAL_MATERIAL_SCHEMA: &str = "mc2-day-material-cache/v1";
const NEUTRAL_MATERIAL_DESCRIPTOR_SCHEMA: &str = "mc2-day-materials/v1";
const NEUTRAL_MATERIAL_SCHEMA_V2: &str = "mc2-day-material-cache/v2";
const NEUTRAL_MATERIAL_DESCRIPTOR_SCHEMA_V2: &str = "mc2-day-materials/v2";
const NEUTRAL_MATERIAL_SCHEMA_V3: &str = "mc2-day-material-cache/v3";
const NEUTRAL_MATERIAL_DESCRIPTOR_SCHEMA_V3: &str = "mc2-day-materials/v3";
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
const LEVEL2_OWNER_MANIFEST_SCHEMA_V5: &str = "mc2-owner-level-export/v5";
const LEVEL2_OWNER_PLAYERS_COMPONENT_MAGIC: &str = "MCP1";
const LEVEL2_OWNER_PLAYERS_COMPONENT_VERSION: u64 = 4;
const LEVEL2_OWNER_PLAYERS_COMPONENT_RECORD_BYTES: u64 = 663;
const LEVEL2_OWNER_PLAYERS_COMPONENT_COUNT: u64 = 8;

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
    source_minimap: bool,
    profile: DayIndexedProfile,
    package_sha256: String,
    campaign_package_sha256: Option<String>,
    cache_content_sha256: String,
    materials: EnvironmentResources,
}

impl DayIndexedFrameAssets {
    /// Reuse the sealed Night texture/sky/palette products for the Level 1
    /// descriptor campaign. Geometry is supplied solely by the live session;
    /// the retained cache's package hash identifies its original export.
    pub fn load_night(cache_dir: impl AsRef<Path>) -> Result<Self, DayIndexedFrameError> {
        if is_environment_cache(cache_dir.as_ref())? {
            let assets = Self::load_environment(cache_dir)?;
            if assets.profile.map_type_id != 1 {
                return Err(DayIndexedFrameError::new(
                    "Night cache has a different environment map type",
                ));
            }
            return Ok(assets);
        }
        let cache = crate::native_world::QualifiedOwnerCacheReader::open(cache_dir)
            .map_err(|error| DayIndexedFrameError::new(error.to_string()))?;
        let manifest = cache
            .manifest_object()
            .map_err(|error| DayIndexedFrameError::new(error.to_string()))?;
        let bindings = object_field(manifest, "bindings", "Night cache")?;
        let package_sha256 =
            string_field(bindings, "level_export_package_sha256", "Night cache")?.to_owned();
        let cache_content_sha256 =
            string_field(manifest, "cache_content_sha256", "Night cache")?.to_owned();
        let read = |path: &str, extent: usize| -> Result<Box<[u8]>, DayIndexedFrameError> {
            let bytes = cache
                .read(path)
                .map_err(|error| DayIndexedFrameError::new(error.to_string()))?;
            if bytes.len() != extent {
                return Err(DayIndexedFrameError::new(format!(
                    "Night {path} extent mismatch"
                )));
            }
            Ok(bytes.into_boxed_slice())
        };
        Ok(Self {
            source_minimap: true,
            profile: DayIndexedProfile {
                external_level: 1,
                internal_level: 0,
                map_type_id: 1,
                vga_resolution_selector: 1,
                texture_size: 32,
            },
            package_sha256,
            campaign_package_sha256: None,
            cache_content_sha256,
            materials: EnvironmentResources::retained(
                read("palette/paln.rgb8", PALETTE_RGB8_BYTES)?,
                read("palette/tablesn.u8", TABLESD_BYTES)?,
                read("sky/skyn.indices.u8", SKY_BYTES)?,
                read("terrain/bl32n.atlas.indices.u8", TERRAIN_ATLAS_BYTES)?,
            ),
        })
    }

    /// Load and fully validate one owner-generated Level 2 Day material cache.
    ///
    /// The manifest payload, every listed output hash/size, content-tree
    /// identity, profile selectors, descriptor bounds, and source roots are
    /// checked before any decoded product is returned.  Unknown or tampered
    /// caches therefore fail closed instead of falling back to Level 1.
    pub fn load(cache_dir: impl AsRef<Path>) -> Result<Self, DayIndexedFrameError> {
        let cache_dir = cache_dir.as_ref();
        if is_environment_cache(cache_dir)? {
            return Self::load_environment(cache_dir);
        }
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
        let schema = string_field(manifest_object, "schema", "material manifest")?;
        let corrected_blocks = matches!(
            schema,
            NEUTRAL_MATERIAL_SCHEMA_V2 | NEUTRAL_MATERIAL_SCHEMA_V3
        );
        let block_asset = if corrected_blocks {
            "owner_data:DATA/BLOCK32.DAT"
        } else {
            "owner_data:DATA/BL32N0-0.DAT"
        };
        let neutral_dialect = match schema {
            MATERIAL_SCHEMA => false,
            NEUTRAL_MATERIAL_SCHEMA | NEUTRAL_MATERIAL_SCHEMA_V2 | NEUTRAL_MATERIAL_SCHEMA_V3 => {
                true
            }
            _ => {
                return Err(DayIndexedFrameError::new(
                    "unsupported Day material cache schema",
                ));
            }
        };
        let generator = object_field(manifest_object, "generator", "material manifest")?;
        if string_field(generator, "name", "material generator")? != MATERIAL_GENERATOR {
            return Err(DayIndexedFrameError::new(
                "Level 2 Day material generator identity mismatch",
            ));
        }

        let profile_value = manifest_object
            .get("profile")
            .ok_or_else(|| DayIndexedFrameError::new("material manifest profile is missing"))?;
        let profile_object = object(profile_value, "material profile")?;
        let expected_profile = qualified_profile_json();
        let historical_v4_profile = historical_v4_profile_json();
        let qualified_v5 = !neutral_dialect && profile_value == &expected_profile;
        let profile = if neutral_dialect {
            validate_neutral_profile(profile_object, corrected_blocks)?
        } else {
            if !qualified_v5 && profile_value != &historical_v4_profile {
                return Err(DayIndexedFrameError::new(
                    "Level 2 Day material profile identity mismatch",
                ));
            }
            DayIndexedProfile::qualified()
        };
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
        let campaign_package_sha256 = if neutral_dialect {
            let campaign_package = string_field(
                profile_object,
                "campaign_package_sha256",
                "material profile",
            )?;
            if string_field(bindings, "campaign_package_sha256", "material bindings")?
                != campaign_package
            {
                return Err(DayIndexedFrameError::new(
                    "Day material campaign-package binding mismatch",
                ));
            }
            validate_v5_owner_identity(profile_object, bindings)?;
            Some(campaign_package.to_owned())
        } else if qualified_v5 {
            validate_v5_owner_identity(profile_object, bindings)?;
            None
        } else {
            // v4 remains a decode-only historical inspection artifact. A
            // hybrid artifact must not gain qualified status through an
            // incomplete owner-identity projection.
            for name in [
                "owner_manifest_schema",
                "players_component_magic",
                "players_component_version",
                "players_component_record_bytes",
                "players_component_count",
            ] {
                if bindings.contains_key(name) {
                    return Err(DayIndexedFrameError::new(format!(
                        "historical v4 material bindings must not contain {name}"
                    )));
                }
            }
            None
        };

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
            // GOLDEN-CHANGE: the qualified v5 descriptor now carries 20
            // additional rooted class-5 TMAP rows; v4 remains historical.
            if neutral_dialect {
                outputs
                    .iter()
                    .find(|output| output.path == MATERIAL_DESCRIPTOR_PATH)
                    .map(|output| output.bytes)
                    .ok_or_else(|| {
                        DayIndexedFrameError::new("Day material descriptor output is missing")
                    })?
            } else if qualified_v5 {
                74_421
            } else {
                64_695
            },
            &[
                "owner_data:DATA/PALD-0.DAT",
                "owner_data:DATA/TABLESD.DAT",
                "owner_data:DATA/CLRD-0.DAT",
                block_asset,
                "owner_data:DATA/SKYD0-0.DAT",
            ],
        )?;
        validate_descriptor(
            &descriptor_bytes,
            profile_value,
            if schema == NEUTRAL_MATERIAL_SCHEMA_V3 {
                NEUTRAL_MATERIAL_DESCRIPTOR_SCHEMA_V3
            } else if corrected_blocks {
                NEUTRAL_MATERIAL_DESCRIPTOR_SCHEMA_V2
            } else if neutral_dialect {
                NEUTRAL_MATERIAL_DESCRIPTOR_SCHEMA
            } else {
                MATERIAL_DESCRIPTOR_SCHEMA
            },
        )?;

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
            if corrected_blocks {
                "terrain/block32.atlas.indices.u8"
            } else {
                TERRAIN_ATLAS_PATH
            },
            "indexed_atlas_u8",
            TERRAIN_ATLAS_BYTES,
            &[block_asset],
        )?;
        let _ = read_output(
            cache_dir,
            &outputs,
            if corrected_blocks {
                "terrain/block32.indices.u8"
            } else {
                TERRAIN_TILES_PATH
            },
            "indexed_tile_bank_u8",
            TERRAIN_TILE_BANK_BYTES,
            &[block_asset],
        )?;

        Ok(Self {
            source_minimap: corrected_blocks,
            profile,
            package_sha256,
            campaign_package_sha256,
            cache_content_sha256: claimed_content_hash.to_owned(),
            materials: EnvironmentResources::retained(
                palette_rgb8.into_boxed_slice(),
                tablesd.into_boxed_slice(),
                sky.into_boxed_slice(),
                terrain_atlas.into_boxed_slice(),
            ),
        })
    }

    fn load_environment(cache_dir: impl AsRef<Path>) -> Result<Self, DayIndexedFrameError> {
        let materials = EnvironmentResources::load(cache_dir)
            .map_err(|e| DayIndexedFrameError::new(e.to_string()))?;
        let profile = materials
            .profile
            .as_ref()
            .ok_or_else(|| DayIndexedFrameError::new("environment profile missing"))?;
        Ok(Self {
            source_minimap: true,
            profile: DayIndexedProfile {
                external_level: profile.external_level,
                internal_level: profile.internal_level,
                map_type_id: profile.map_type.id,
                vga_resolution_selector: profile.vga_resolution_selector,
                texture_size: profile.texture_size,
            },
            package_sha256: profile.owner_export_package_sha256.clone(),
            campaign_package_sha256: Some(profile.campaign_package_sha256.clone()),
            cache_content_sha256: materials.content_sha256.clone(),
            materials,
        })
    }

    pub fn environment_resources(&self) -> &EnvironmentResources {
        &self.materials
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
    pub fn campaign_package_sha256(&self) -> Option<&str> {
        self.campaign_package_sha256.as_deref()
    }

    #[must_use]
    pub fn cache_content_sha256(&self) -> &str {
        &self.cache_content_sha256
    }

    #[must_use]
    pub fn palette_rgb8(&self) -> &[u8] {
        self.materials.palette_rgb8()
    }

    #[must_use]
    pub fn tablesd(&self) -> &[u8] {
        self.materials.tables()
    }

    #[must_use]
    pub fn sky(&self) -> &[u8] {
        self.materials.sky().unwrap_or_default()
    }

    #[must_use]
    pub fn terrain_atlas(&self) -> &[u8] {
        self.materials.blocks()
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
        let frame = render_environment_indexed_frame(
            inspection,
            session,
            assets,
            generation,
            self.current_frame.as_ref(),
        )?;
        self.current_tick = Some(tick);
        self.current_frame = Some(frame.clone());
        Ok(frame)
    }

    /// Render the authority's current revision for an observational client.
    ///
    /// Unlike the retained Level-2 compatibility endpoint, a generic client
    /// is not required to poll every intervening presentation tick. This does
    /// not advance simulation or presentation history; the Day producer is a
    /// pure projection of the current authoritative state.
    pub fn render_current(
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
        let tick = session.state.tick;
        if self.current_tick.is_some_and(|current| tick < current) {
            return Err(DayIndexedFrameError::new(format!(
                "Day indexed frame tick {tick} predates the committed presentation revision"
            )));
        }
        if self.current_tick == Some(tick) {
            if let Some(frame) = self.current_frame.as_ref() {
                if frame.cache_content_sha256 != assets.cache_content_sha256() {
                    return Err(DayIndexedFrameError::new(
                        "environment cache changed within a live frame generation",
                    ));
                }
                return Ok(frame.clone());
            }
        }
        let frame = render_environment_indexed_frame(
            inspection,
            session,
            assets,
            generation,
            self.current_frame.as_ref(),
        )?;
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
    render_environment_indexed_frame(inspection, session, assets, generation, None)
}

fn render_environment_indexed_frame(
    inspection: &NativeWorldInspection,
    session: &Level2ResetSession,
    assets: &DayIndexedFrameAssets,
    generation: u64,
    prior: Option<&DayIndexedFrame>,
) -> Result<DayIndexedFrame, DayIndexedFrameError> {
    validate_day_route(inspection, session, assets)?;
    if prior.is_some_and(|frame| {
        frame.generation != generation
            || frame.cache_content_sha256 != assets.cache_content_sha256()
    }) {
        return Err(DayIndexedFrameError::new(
            "environment history belongs to a different package/generation",
        ));
    }
    let exit = session.level_exit_sequence();
    let tables = assets.materials.renderer_tables(exit);
    let tick = session.state.tick;
    let camera = camera_from_state(&session.state)?;
    let profile = ClassicHdRenderProfile::compatibility_320x200();
    let planes = session.canonical_terrain().planes();
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
            tables,
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
    if let Some(prior) = prior {
        assets
            .materials
            .composite_exit(&mut indices, &prior.indices, exit)
            .map_err(|e| DayIndexedFrameError::new(e.to_string()))?;
    }
    // The approved v2 presentation enables the source map layer. Retained v1
    // material captures keep their terrain-only contract.
    if assets.source_minimap {
        draw_minimap_terrain(
            &mut indices,
            usize::from(DAY_FRAME_WIDTH),
            camera,
            &planes.terrain_types,
            &planes.shading,
            tables,
        );
    }
    let index_hash = byte_hash(&indices);
    let mut palette_rgb8 = assets.palette_rgb8().to_vec();
    session.field_palette().apply_rgb8(&mut palette_rgb8);
    Ok(DayIndexedFrame {
        profile: assets.profile(),
        generation,
        tick,
        width: DAY_FRAME_WIDTH,
        height: DAY_FRAME_HEIGHT,
        indices,
        palette_rgb8,
        index_hash,
        terrain_input_hash: terrain.input_hash,
        terrain_command_hash: terrain.command_hash,
        cache_content_sha256: assets.cache_content_sha256.clone(),
        package_sha256: assets.package_sha256.clone(),
    })
}

/// Source `GameUI.cpp::sub_63670_draw_minimap_a`, non-cave opaque branch.
/// The 128-source-pixel map is sampled onto the fixed 320x200 presentation.
/// All world coordinates wrap through BYTE1 exactly as the source map index.
fn draw_minimap_terrain(
    target: &mut [u8],
    stride: usize,
    camera: Level1TerrainFrameCamera,
    terrain_types: &[u8],
    shading: &[u8],
    tables: &[u8],
) {
    const SOURCE_SIDE: i32 = 128;
    const SIDE: usize = 64;
    let angle = usize::from(camera.yaw.cast_unsigned() & 0x7ff);
    let sin = ((256_i64 * i64::from(crate::SIN_16_16[angle])) >> 16) as i32;
    let cos = ((256_i64 * i64::from(crate::SIN_16_16[angle + 512])) >> 16) as i32;
    let origin_x = i32::from(camera.render_x) - SOURCE_SIDE * (cos - sin) / 2;
    let origin_y = i32::from(camera.render_y) - SOURCE_SIDE * (cos + sin) / 2;
    for y in 0..SIDE {
        let source_y = (y * 2) as i32;
        let phase = if source_y < 64 {
            63 - source_y
        } else {
            source_y - 64
        };
        let half_span = (64 * i32::from(tables[0x14300 + (phase as usize * 4)])) >> 8;
        for x in 0..SIDE {
            let source_x = (x * 2) as i32;
            if source_x < 64 - half_span || source_x >= 64 + half_span {
                continue;
            }
            let world_x = origin_x - source_y * sin + source_x * cos;
            let world_y = origin_y + source_y * cos + source_x * sin;
            let tile = (((world_y >> 8) & 255) * 256 + ((world_x >> 8) & 255)) as usize;
            let base = usize::from(tables[0x14000 + usize::from(terrain_types[tile])]);
            target[y * stride + x] = tables[(usize::from(shading[tile]) << 8) + base];
        }
    }
}

fn validate_day_route(
    inspection: &NativeWorldInspection,
    session: &Level2ResetSession,
    assets: &DayIndexedFrameAssets,
) -> Result<(), DayIndexedFrameError> {
    let metadata = inspection.metadata();
    let material = assets.profile();
    if metadata.external_level != material.external_level
        || metadata.internal_level != material.internal_level
        || metadata.map_type as u8 != material.map_type_id
        || !matches!(metadata.map_type, MapType::Day | MapType::Night)
    {
        return Err(DayIndexedFrameError::new(
            "Day indexed frame metadata and material selection differ",
        ));
    }
    if ((material.map_type_id == 0 || assets.campaign_package_sha256().is_some())
        && assets.package_sha256() != inspection.package_sha256())
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

fn is_environment_cache(root: &Path) -> Result<bool, DayIndexedFrameError> {
    let bytes = fs::read(root.join("manifest.json"))
        .map_err(|e| DayIndexedFrameError::new(e.to_string()))?;
    let manifest: Value =
        serde_json::from_slice(&bytes).map_err(|e| DayIndexedFrameError::new(e.to_string()))?;
    Ok(manifest.get("schema").and_then(Value::as_str) == Some(CACHE_SCHEMA))
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

fn validate_descriptor(
    bytes: &[u8],
    profile: &Value,
    descriptor_schema: &str,
) -> Result<(), DayIndexedFrameError> {
    let descriptor: Value = serde_json::from_slice(bytes).map_err(|error| {
        DayIndexedFrameError::new(format!(
            "invalid Level 2 Day material descriptor JSON: {error}"
        ))
    })?;
    let object = object(&descriptor, "material descriptor")?;
    if string_field(object, "schema", "material descriptor")? != descriptor_schema
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
    let bl32n = object_field(
        object,
        if matches!(
            descriptor_schema,
            NEUTRAL_MATERIAL_DESCRIPTOR_SCHEMA_V2 | NEUTRAL_MATERIAL_DESCRIPTOR_SCHEMA_V3
        ) {
            "blocks"
        } else {
            "bl32n"
        },
        "material descriptor",
    )?;
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
        // GOLDEN-CHANGE: only rooted class-5 constructor TMAP reachability was
        // added; the qualified simulation and event behavior did not change.
        "closure_sha256": "5f1935977111a911f8c0b954214fe89d3e9e6fa949a37df21be7668800ee101e",
        "external_level": MATERIAL_PACKAGE_LEVEL,
        "internal_level": MATERIAL_INTERNAL_LEVEL,
        "map_type": {"id": MATERIAL_MAP_TYPE_ID, "name": "Day"},
        "name": MATERIAL_PROFILE_NAME,
        "owner_package": {
            "manifest_schema": LEVEL2_OWNER_MANIFEST_SCHEMA_V5,
            "players_component": {
                "magic": LEVEL2_OWNER_PLAYERS_COMPONENT_MAGIC,
                "version": LEVEL2_OWNER_PLAYERS_COMPONENT_VERSION,
                "record_bytes": LEVEL2_OWNER_PLAYERS_COMPONENT_RECORD_BYTES,
                "count": LEVEL2_OWNER_PLAYERS_COMPONENT_COUNT,
            },
        },
        "package_sha256": "dc8f973ee9a2f53863a2e2e3743b6fa4b9ef947d3ce0b27b79d3ba86de318b80",
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

fn historical_v4_profile_json() -> Value {
    json!({
        "catalog_file_sha256": "a25488be0933e5b4499665d37440a9860c8614649d9f695080da511c11d75b44",
        "catalog_sha256": "ea9a735c598f2cbed81541366e1fdf8a5fcbc8cef7aecd086bb8a6a9e7c2429e",
        "closure_sha256": "66505e6c3691cce6c730682047c7d8634c682172daa017d4840d1a4a5be4661c",
        "external_level": MATERIAL_PACKAGE_LEVEL,
        "internal_level": MATERIAL_INTERNAL_LEVEL,
        "map_type": {"id": MATERIAL_MAP_TYPE_ID, "name": "Day"},
        "name": MATERIAL_PROFILE_NAME,
        "package_sha256": "1d73e81bc897933d62621b53a1c8c1cf890bd5dffe17f77de43f8d38f3854488",
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

fn validate_neutral_profile(
    profile: &serde_json::Map<String, Value>,
    corrected_blocks: bool,
) -> Result<DayIndexedProfile, DayIndexedFrameError> {
    if string_field(profile, "name", "material profile")? != "day-material"
        || string_field(profile, "spec_commit", "material profile")? != REMC2_SPEC_COMMIT
    {
        return Err(DayIndexedFrameError::new(
            "neutral Day material profile identity mismatch",
        ));
    }
    let external = u16::try_from(
        profile
            .get("external_level")
            .and_then(Value::as_u64)
            .ok_or_else(|| DayIndexedFrameError::new("Day external level is invalid"))?,
    )
    .map_err(|_| DayIndexedFrameError::new("Day external level exceeds u16"))?;
    let internal = u16::try_from(
        profile
            .get("internal_level")
            .and_then(Value::as_u64)
            .ok_or_else(|| DayIndexedFrameError::new("Day internal level is invalid"))?,
    )
    .map_err(|_| DayIndexedFrameError::new("Day internal level exceeds u16"))?;
    if external != internal.saturating_add(1) {
        return Err(DayIndexedFrameError::new(
            "Day external/internal selection mismatch",
        ));
    }
    let map_type = object_field(profile, "map_type", "material profile")?;
    require_u64(
        map_type,
        "id",
        MATERIAL_MAP_TYPE_ID,
        "material profile map type",
    )?;
    if string_field(map_type, "name", "material profile map type")? != "Day" {
        return Err(DayIndexedFrameError::new(
            "neutral material profile is not Day",
        ));
    }
    let selectors = object_field(profile, "selectors", "material profile")?;
    let mut expected = qualified_profile_json();
    if corrected_blocks {
        expected["selectors"]["block_bank"] = Value::String("BLOCK32".to_owned());
    }
    if selectors
        != object_field(
            object(&expected, "qualified profile")?,
            "selectors",
            "qualified profile",
        )?
    {
        return Err(DayIndexedFrameError::new(
            "neutral Day material selector identity mismatch",
        ));
    }
    let package = string_field(profile, "package_sha256", "material profile")?;
    let owner_export = string_field(profile, "owner_export_package_sha256", "material profile")?;
    if package != owner_export {
        return Err(DayIndexedFrameError::new(
            "neutral Day owner-export binding mismatch",
        ));
    }
    for value in [
        package,
        string_field(profile, "campaign_package_sha256", "material profile")?,
        string_field(profile, "closure_sha256", "material profile")?,
        string_field(profile, "catalog_sha256", "material profile")?,
        string_field(profile, "catalog_file_sha256", "material profile")?,
    ] {
        if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(DayIndexedFrameError::new(
                "neutral Day material identity is not a SHA-256 digest",
            ));
        }
    }
    Ok(DayIndexedProfile {
        external_level: external,
        internal_level: internal,
        map_type_id: MATERIAL_MAP_TYPE_ID as u8,
        vga_resolution_selector: MATERIAL_VGA_RESOLUTION_SELECTOR as u8,
        texture_size: MATERIAL_TEXTURE_SIZE as u8,
    })
}

fn validate_v5_owner_identity(
    profile: &serde_json::Map<String, Value>,
    bindings: &serde_json::Map<String, Value>,
) -> Result<(), DayIndexedFrameError> {
    let owner_package = object_field(profile, "owner_package", "material profile")?;
    if string_field(owner_package, "manifest_schema", "material owner package")?
        != LEVEL2_OWNER_MANIFEST_SCHEMA_V5
    {
        return Err(DayIndexedFrameError::new(
            "Level 2 Day material owner package schema mismatch",
        ));
    }
    let players_component =
        object_field(owner_package, "players_component", "material owner package")?;
    if string_field(
        players_component,
        "magic",
        "material owner players component",
    )? != LEVEL2_OWNER_PLAYERS_COMPONENT_MAGIC
    {
        return Err(DayIndexedFrameError::new(
            "Level 2 Day material owner players magic mismatch",
        ));
    }
    for (name, expected) in [
        ("version", LEVEL2_OWNER_PLAYERS_COMPONENT_VERSION),
        ("record_bytes", LEVEL2_OWNER_PLAYERS_COMPONENT_RECORD_BYTES),
        ("count", LEVEL2_OWNER_PLAYERS_COMPONENT_COUNT),
    ] {
        require_u64(
            players_component,
            name,
            expected,
            "material owner players component",
        )?;
    }
    if string_field(bindings, "owner_manifest_schema", "material bindings")?
        != LEVEL2_OWNER_MANIFEST_SCHEMA_V5
        || string_field(bindings, "players_component_magic", "material bindings")?
            != LEVEL2_OWNER_PLAYERS_COMPONENT_MAGIC
    {
        return Err(DayIndexedFrameError::new(
            "Level 2 Day material owner bindings schema mismatch",
        ));
    }
    for (name, expected) in [
        (
            "players_component_version",
            LEVEL2_OWNER_PLAYERS_COMPONENT_VERSION,
        ),
        (
            "players_component_record_bytes",
            LEVEL2_OWNER_PLAYERS_COMPONENT_RECORD_BYTES,
        ),
        (
            "players_component_count",
            LEVEL2_OWNER_PLAYERS_COMPONENT_COUNT,
        ),
    ] {
        require_u64(bindings, name, expected, "material bindings")?;
    }
    Ok(())
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
    use crate::native_world::{NativeWorld, OwnerPackageProfile};
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn package_environment_binds_real_night_and_day_frames() {
        for (level, package, selection, retained) in [
            (
                1,
                "mc-bhq8-level1-collision-v5-night",
                crate::campaign::spec::CampaignSelection::LEVEL1,
                "level1-unity-asset-cache-plan29-final",
            ),
            (
                2,
                "level2-day-block32-campaign-v2",
                crate::campaign::spec::CampaignSelection::LEVEL2,
                "level2-day-minimap-owner-cache-v3/components/material",
            ),
        ] {
            let path = generated_path(&format!("mc-oi62-environment-level{level}-v2"));
            let mut campaign = crate::campaign::CampaignSession::open_with_owner_cache(
                generated_path(package),
                &path,
                selection,
            )
            .expect("actual package and environment open");
            let assets = DayIndexedFrameAssets::load(&path).expect("package-selected materials");
            let legacy = if level == 1 {
                DayIndexedFrameAssets::load_night(generated_path(retained))
            } else {
                DayIndexedFrameAssets::load(generated_path(retained))
            }
            .expect("retained known default");
            assert_eq!(assets.palette_rgb8(), legacy.palette_rgb8());
            assert_eq!(assets.terrain_atlas(), legacy.terrain_atlas());
            assert_eq!(assets.tablesd(), legacy.tablesd());
            let mut state = DayIndexedFrameState::new(campaign.generation());
            let first = state
                .render_current(
                    campaign.inspection(),
                    campaign.level2_compatibility(),
                    &assets,
                    campaign.generation(),
                )
                .expect("actual runtime frame");
            assert!(first.indices.iter().any(|&pixel| pixel != 0));
            campaign.step(
                crate::PlayerCommand {
                    roll: 24,
                    pitch: -8,
                    ..crate::PlayerCommand::default()
                },
                0,
            );
            let moved = state
                .render_current(
                    campaign.inspection(),
                    campaign.level2_compatibility(),
                    &assets,
                    campaign.generation(),
                )
                .expect("following runtime frame");
            assert_eq!(moved.tick, first.tick + 1);
            assert_ne!(moved.index_hash, first.index_hash);
        }
        assert!(
            DayIndexedFrameAssets::load_night(generated_path("mc-oi62-environment-level2-v2"))
                .is_err()
        );
    }

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
        std::env::var_os("MC_LEVEL2_PACKAGE_V5")
            .map_or_else(|| generated_path("l2-debug3.fJ3PaM/level2"), PathBuf::from)
    }

    fn qualified_cache() -> PathBuf {
        std::env::var_os("MC_LEVEL2_MATERIAL_CACHE_V5").map_or_else(
            || generated_path("level2-day-material-cache"),
            PathBuf::from,
        )
    }

    fn historical_v4_cache() -> PathBuf {
        let migrated = generated_path("level2-day-material-cache-v4");
        if migrated.exists() {
            migrated
        } else {
            generated_path("level2-day-material-cache")
        }
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
    fn qualified_night_campaign_renders_live_terrain_and_rejects_day_substitution() {
        let mut campaign = crate::campaign::CampaignSession::open(
            generated_path("mc-bhq8-level1-collision-v5-night"),
            crate::campaign::spec::CampaignSelection::LEVEL1,
        )
        .expect("real Night campaign opens");
        let assets = DayIndexedFrameAssets::load_night(generated_path(
            "level1-unity-asset-cache-plan29-final",
        ))
        .expect("sealed Night assets validate without rebuilding owner products");
        let initial_hash = campaign.state_hash();
        let render = |campaign: &crate::campaign::CampaignSession| {
            render_level2_day_indexed_frame(
                campaign.inspection(),
                campaign.level2_compatibility(),
                &assets,
                campaign.generation(),
            )
            .unwrap()
        };
        let first = render(&campaign);
        assert_eq!(first.indices.len(), 320 * 200);
        assert_eq!(first.palette_rgb8.len(), 768);
        assert!(first.indices.iter().any(|pixel| *pixel != 0));
        assert_eq!(first, render(&campaign));
        assert_eq!(
            campaign.state_hash(),
            initial_hash,
            "render polling is observational"
        );
        campaign.step(
            crate::PlayerCommand {
                roll: 24,
                pitch: -8,
                ..crate::PlayerCommand::default()
            },
            0,
        );
        let stepped = render(&campaign);
        assert_eq!(stepped.tick, 1);
        assert_ne!(stepped.terrain_input_hash, first.terrain_input_hash);
        let day = crate::campaign::CampaignSession::open(
            generated_path("level2-day-block32-campaign-v2"),
            crate::campaign::spec::CampaignSelection::LEVEL2,
        )
        .unwrap();
        assert!(
            render_level2_day_indexed_frame(
                day.inspection(),
                day.level2_compatibility(),
                &assets,
                0
            )
            .is_err()
        );
        campaign.reset();
        let reset = render(&campaign);
        assert_eq!(reset.indices, first.indices);
        assert_eq!(reset.palette_rgb8, first.palette_rgb8);
    }

    #[test]
    fn corrected_level_two_owner_identity_includes_its_building_parameters() {
        let campaign = crate::campaign::CampaignSession::open(
            generated_path("level2-day-block32-campaign-v2"),
            crate::campaign::spec::CampaignSelection::LEVEL2,
        )
        .expect("corrected Level 2 campaign opens");
        let assets = DayIndexedFrameAssets::load(generated_path(
            "level2-day-minimap-owner-cache-v3/components/material",
        ))
        .expect("the matching owner cache opens");
        let manifest: Value = serde_json::from_slice(
            &fs::read(generated_path("level2-day-block32-owner-v2/manifest.json")).unwrap(),
        )
        .unwrap();
        assert!(manifest["files"].get("building-parameters.bin").is_some());
        assert_eq!(
            campaign.inspection().package_sha256(),
            manifest["package_sha256"].as_str().unwrap()
        );
        assert_eq!(
            assets.package_sha256(),
            campaign.inspection().package_sha256()
        );
        let frame = render_level2_day_indexed_frame(
            campaign.inspection(),
            campaign.level2_compatibility(),
            &assets,
            0,
        )
        .expect("cache binding and real terrain render accept the exact owner identity");
        assert!(frame.indices.iter().any(|pixel| *pixel != 0));
    }

    #[test]
    fn day_v3_symbol_cache_preserves_the_qualified_day_frame() {
        let previous =
            DayIndexedFrameAssets::load(generated_path("level3-day-block32-material-cache-v2"))
                .expect("qualified Day v2 cache");
        let current = DayIndexedFrameAssets::load(generated_path(
            "level3-day-minimap-owner-cache-v3/components/material",
        ))
        .expect("Day v3 material component consumed by the Unity symbol presenter");
        let campaign = crate::campaign::CampaignSession::open(
            generated_path("level3-day-block32-campaign-recipe-v2"),
            crate::campaign::spec::CampaignSelection::LEVEL3,
        )
        .expect("real corrected Day campaign");
        let before = campaign.state_hash();
        let render = |assets| {
            render_level2_day_indexed_frame(
                campaign.inspection(),
                campaign.level2_compatibility(),
                assets,
                0,
            )
            .unwrap()
        };
        let old_frame = render(&previous);
        let frame = render(&current);
        assert_eq!(frame.indices, old_frame.indices);
        assert_eq!(current.palette_rgb8(), previous.palette_rgb8());
        assert!(current.source_minimap);
        assert_eq!(campaign.state_hash(), before);
    }

    #[test]
    fn day_v2_material_loads_source_day_bank_and_rejects_night_selector() {
        let cache = generated_path("level3-day-block32-material-cache-v2");
        let mut assets = DayIndexedFrameAssets::load(&cache).expect("qualified Day v2 cache");
        let source = generated_path("../remc2-native-runtime/CD_Files/DATA/BLOCK32.DAT");
        assert_eq!(assets.terrain_atlas(), fs::read(source).unwrap());
        let night = generated_path("../remc2-native-runtime/CD_Files/DATA/BL32N0-0.DAT");
        assert_ne!(assets.terrain_atlas(), fs::read(night).unwrap());
        let campaign = crate::campaign::CampaignSession::open(
            generated_path("level3-day-block32-campaign-recipe-v2"),
            crate::campaign::spec::CampaignSelection::LEVEL3,
        )
        .expect("corrected campaign recipe opens through the shared authority");
        let before = campaign.state_hash();
        let frame = render_level2_day_indexed_frame(
            campaign.inspection(),
            campaign.level2_compatibility(),
            &assets,
            0,
        )
        .expect("corrected Day assets render the real campaign terrain");
        assert_eq!(frame.indices.len(), 320 * 200);
        assert!(frame.indices.iter().any(|index| *index != 0));
        let session = campaign.level2_compatibility();
        let planes = session.canonical_terrain().planes();
        let position = session.state.position;
        let tile = usize::from(position.y >> 8) * 256 + usize::from(position.x >> 8);
        let base = assets.tablesd()[0x14000 + usize::from(planes.terrain_types[tile])];
        assert_eq!(
            frame.indices[32 * 320 + 32],
            assets.tablesd()[(usize::from(planes.shading[tile]) << 8) + usize::from(base)],
            "map center samples the authoritative player's terrain tile"
        );
        assets.source_minimap = false;
        let terrain_only =
            render_level2_day_indexed_frame(campaign.inspection(), session, &assets, 0).unwrap();
        assert_eq!(
            frame.indices[0], terrain_only.indices[0],
            "aperture clips the corner"
        );
        assert_eq!(
            &frame.indices[64 * 320..],
            &terrain_only.indices[64 * 320..]
        );
        assert_ne!(
            frame.indices, terrain_only.indices,
            "map pixels reach the real frame"
        );
        assert_eq!(
            campaign.state_hash(),
            before,
            "presentation is observational"
        );
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(cache.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["schema"], NEUTRAL_MATERIAL_SCHEMA_V2);
        let profile = manifest["profile"].as_object_mut().unwrap();
        validate_neutral_profile(profile, true).expect("Day v2 profile accepted");
        profile.get_mut("selectors").unwrap()["block_bank"] = Value::String("BL32N0-0".to_owned());
        assert!(
            validate_neutral_profile(profile, true).is_err(),
            "v2 cannot relabel Night indices as Day"
        );
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
    fn level_two_v4_material_cache_remains_inspectable() {
        let assets = DayIndexedFrameAssets::load(historical_v4_cache())
            .expect("historical v4 material cache remains parseable for inspection");
        assert_eq!(assets.profile(), DayIndexedProfile::qualified());
        assert_eq!(
            assets.package_sha256(),
            "1d73e81bc897933d62621b53a1c8c1cf890bd5dffe17f77de43f8d38f3854488"
        );
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

        let owner_tampered_cache = temporary_cache();
        let manifest_path = owner_tampered_cache.0.join("manifest.json");
        let mut manifest: Value = serde_json::from_slice(
            &fs::read(&manifest_path).expect("read owner-identity temporary manifest"),
        )
        .expect("parse owner-identity temporary manifest");
        manifest["bindings"]["players_component_version"] = json!(3);
        refresh_manifest_claim(&mut manifest);
        fs::write(
            &manifest_path,
            canonical_json(&manifest).expect("encode owner-identity temporary manifest"),
        )
        .expect("write owner-identity temporary manifest");
        let error = DayIndexedFrameAssets::load(&owner_tampered_cache.0)
            .expect_err("v5 material cache with v3 players binding must fail closed");
        assert!(error.to_string().contains("players_component_version"));

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
